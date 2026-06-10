use crate::css_parser::source::SourceInput;
use crate::css_parser::token::{Token, TokenKind};

/// CSS tokenizer per CSS Syntax Level 3 §4.
///
/// Consumes a CSS input string and produces a stream of tokens.
/// Implements `Iterator<Item = Token<'a>>` for ergonomic usage.
pub struct Tokenizer<'a> {
    input: SourceInput<'a>,
    finished: bool,
}

impl<'a> Tokenizer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            input: SourceInput::new(input),
            finished: false,
        }
    }

    /// Consume the next token (§4.3.1 "Consume a token").
    pub fn next_token(&mut self) -> Token<'a> {
        self.consume_comments();

        let loc = self.input.location();

        match self.input.next_char() {
            None => Token {
                kind: TokenKind::Eof,
                loc,
            },

            Some(ch) => {
                let kind = match ch {
                    // Whitespace
                    ' ' | '\t' | '\n' => {
                        self.consume_while(|c| c == ' ' || c == '\t' || c == '\n');
                        TokenKind::Whitespace
                    }

                    // String
                    '"' => self.consume_string('"'),
                    '\'' => self.consume_string('\''),

                    // Hash
                    '#' => {
                        let next = self.input.current_char();
                        let next2 = self.input.peek_char(1);
                        if is_name_char(next) || is_valid_escape(next, next2) {
                            let is_id = would_start_ident(
                                self.input.current_char(),
                                self.input.peek_char(1),
                                self.input.peek_char(2),
                            );
                            let start = self.input.pos();
                            self.consume_name_chars();
                            let value = self.input.slice(start, self.input.pos());
                            TokenKind::Hash { value, is_id }
                        } else {
                            TokenKind::Delim('#')
                        }
                    }

                    // Left paren
                    '(' => TokenKind::OpenParen,
                    ')' => TokenKind::CloseParen,

                    // Plus: could be number
                    '+' => {
                        if would_start_number(
                            Some('+'),
                            self.input.current_char(),
                            self.input.peek_char(1),
                        ) {
                            self.input.reconsume();
                            self.consume_numeric()
                        } else {
                            TokenKind::Delim('+')
                        }
                    }

                    ',' => TokenKind::Comma,

                    // Minus: could be number, ident, or CDC
                    '-' => {
                        if would_start_number(
                            Some('-'),
                            self.input.current_char(),
                            self.input.peek_char(1),
                        ) {
                            self.input.reconsume();
                            self.consume_numeric()
                        } else if self.input.current_char() == Some('-')
                            && self.input.peek_char(1) == Some('>')
                        {
                            self.input.next_char(); // -
                            self.input.next_char(); // >
                            TokenKind::Cdc
                        } else if would_start_ident(
                            Some('-'),
                            self.input.current_char(),
                            self.input.peek_char(1),
                        ) {
                            self.input.reconsume();
                            self.consume_ident_like()
                        } else {
                            TokenKind::Delim('-')
                        }
                    }

                    // Period: could be number
                    '.' => {
                        if would_start_number(
                            Some('.'),
                            self.input.current_char(),
                            self.input.peek_char(1),
                        ) {
                            self.input.reconsume();
                            self.consume_numeric()
                        } else {
                            TokenKind::Delim('.')
                        }
                    }

                    ':' => TokenKind::Colon,
                    ';' => TokenKind::Semicolon,

                    // Less-than: CDO?
                    '<' => {
                        if self.input.current_char() == Some('!')
                            && self.input.peek_char(1) == Some('-')
                            && self.input.peek_char(2) == Some('-')
                        {
                            self.input.next_char(); // !
                            self.input.next_char(); // -
                            self.input.next_char(); // -
                            TokenKind::Cdo
                        } else {
                            TokenKind::Delim('<')
                        }
                    }

                    // At-keyword
                    '@' => {
                        if would_start_ident(
                            self.input.current_char(),
                            self.input.peek_char(1),
                            self.input.peek_char(2),
                        ) {
                            let start = self.input.pos();
                            self.consume_name_chars();
                            let name = self.input.slice(start, self.input.pos());
                            TokenKind::AtKeyword(name)
                        } else {
                            TokenKind::Delim('@')
                        }
                    }

                    '[' => TokenKind::OpenSquare,
                    ']' => TokenKind::CloseSquare,

                    // Backslash: escaped code point or delim
                    '\\' => {
                        if is_valid_escape(Some('\\'), self.input.current_char()) {
                            self.input.reconsume();
                            self.consume_ident_like()
                        } else {
                            // Parse error, but return delim
                            TokenKind::Delim('\\')
                        }
                    }

                    '{' => TokenKind::OpenCurly,
                    '}' => TokenKind::CloseCurly,

                    // Digit: numeric token
                    '0'..='9' => {
                        self.input.reconsume();
                        self.consume_numeric()
                    }

                    // Name-start: ident-like token
                    c if is_name_start(c) => {
                        self.input.reconsume();
                        self.consume_ident_like()
                    }

                    // Anything else
                    other => TokenKind::Delim(other),
                };

                Token { kind, loc }
            }
        }
    }

    /// Consume comments (§4.3.2). Called before each token.
    fn consume_comments(&mut self) {
        loop {
            if self.input.current_char() == Some('/') && self.input.peek_char(1) == Some('*') {
                self.input.next_char(); // /
                self.input.next_char(); // *
                loop {
                    match self.input.next_char() {
                        None => return, // EOF in comment
                        Some('*') if self.input.current_char() == Some('/') => {
                            self.input.next_char(); // /
                            break;
                        }
                        _ => {}
                    }
                }
            } else {
                return;
            }
        }
    }

    /// Consume a string token (§4.3.5).
    fn consume_string(&mut self, ending: char) -> TokenKind<'a> {
        let start = self.input.pos();
        loop {
            match self.input.next_char() {
                None => {
                    // EOF: return what we have (parse error, but valid token)
                    let value = self.input.slice(start, self.input.pos());
                    return TokenKind::String(value);
                }
                Some(c) if c == ending => {
                    let value = self.input.slice(start, self.input.pos() - 1);
                    return TokenKind::String(value);
                }
                Some('\n') => {
                    // Unescaped newline in string: bad string
                    self.input.reconsume();
                    return TokenKind::BadString;
                }
                Some('\\') => {
                    match self.input.current_char() {
                        None => {} // EOF after backslash, ignore
                        Some('\n') => {
                            self.input.next_char(); // consume the newline (line continuation)
                        }
                        _ => {
                            self.consume_escape();
                        }
                    }
                }
                Some(_) => {}
            }
        }
    }

    /// Consume a numeric token (§4.3.3).
    fn consume_numeric(&mut self) -> TokenKind<'a> {
        let (value, int_value, has_sign) = self.consume_number();

        // Check if followed by ident → dimension
        if would_start_ident(
            self.input.current_char(),
            self.input.peek_char(1),
            self.input.peek_char(2),
        ) {
            let start = self.input.pos();
            self.consume_name_chars();
            let unit = self.input.slice(start, self.input.pos());
            return TokenKind::Dimension {
                value,
                int_value,
                unit,
            };
        }

        // Check if followed by % → percentage
        if self.input.current_char() == Some('%') {
            self.input.next_char();
            return TokenKind::Percentage { value, int_value };
        }

        TokenKind::Number {
            value,
            int_value,
            has_sign,
        }
    }

    /// Consume a number (§4.3.12). Returns (value, int_value, has_sign).
    fn consume_number(&mut self) -> (f64, Option<i64>, bool) {
        let start = self.input.pos();
        let mut is_integer = true;
        let mut has_sign = false;

        // Optional sign
        match self.input.current_char() {
            Some('+') | Some('-') => {
                has_sign = true;
                self.input.next_char();
            }
            _ => {}
        }

        // Digits before decimal point
        self.consume_while(|c| c.is_ascii_digit());

        // Decimal point + digits
        if self.input.current_char() == Some('.')
            && self.input.peek_char(1).is_some_and(|c| c.is_ascii_digit())
        {
            is_integer = false;
            self.input.next_char(); // .
            self.consume_while(|c| c.is_ascii_digit());
        }

        // Exponent
        if matches!(self.input.current_char(), Some('e') | Some('E')) {
            let next = self.input.peek_char(1);
            if next.is_some_and(|c| c.is_ascii_digit())
                || (matches!(next, Some('+') | Some('-'))
                    && self.input.peek_char(2).is_some_and(|c| c.is_ascii_digit()))
            {
                is_integer = false;
                self.input.next_char(); // e/E
                if matches!(self.input.current_char(), Some('+') | Some('-')) {
                    self.input.next_char(); // sign
                }
                self.consume_while(|c| c.is_ascii_digit());
            }
        }

        let repr = self.input.slice(start, self.input.pos());
        let value: f64 = repr.parse().unwrap_or(0.0);
        let int_value = if is_integer {
            repr.parse::<i64>().ok()
        } else {
            None
        };

        (value, int_value, has_sign)
    }

    /// Consume an ident-like token (§4.3.4).
    fn consume_ident_like(&mut self) -> TokenKind<'a> {
        let start = self.input.pos();
        self.consume_name_chars();
        let name = self.input.slice(start, self.input.pos());

        // Check for url( special case
        if name.eq_ignore_ascii_case("url") && self.input.current_char() == Some('(') {
            self.input.next_char(); // (
                                    // Skip whitespace
            self.consume_while(|c| c == ' ' || c == '\t' || c == '\n');
            // If next is quote, it's a regular function (not url token)
            match self.input.current_char() {
                Some('"') | Some('\'') => {
                    return TokenKind::Function(name);
                }
                _ => {
                    return self.consume_url();
                }
            }
        }

        // Check for function
        if self.input.current_char() == Some('(') {
            self.input.next_char(); // (
            return TokenKind::Function(name);
        }

        TokenKind::Ident(name)
    }

    /// Consume a url token (§4.3.6).
    fn consume_url(&mut self) -> TokenKind<'a> {
        self.consume_while(|c| c == ' ' || c == '\t' || c == '\n');
        let start = self.input.pos();

        loop {
            match self.input.next_char() {
                None => {
                    let value = self.input.slice(start, self.input.pos());
                    return TokenKind::Url(value.trim_end());
                }
                Some(')') => {
                    let end = self.input.pos() - 1;
                    let value = self.input.slice(start, end).trim_end();
                    return TokenKind::Url(value);
                }
                Some(' ') | Some('\t') | Some('\n') => {
                    let end = self.input.pos() - 1;
                    self.consume_while(|c| c == ' ' || c == '\t' || c == '\n');
                    if self.input.current_char() == Some(')') || self.input.is_eof() {
                        self.input.next_char(); // )
                        let value = self.input.slice(start, end);
                        return TokenKind::Url(value);
                    }
                    self.consume_bad_url_remnants();
                    return TokenKind::BadUrl;
                }
                Some('"') | Some('\'') | Some('(') => {
                    self.consume_bad_url_remnants();
                    return TokenKind::BadUrl;
                }
                Some('\\') => {
                    if is_valid_escape(Some('\\'), self.input.current_char()) {
                        self.consume_escape();
                    } else {
                        self.consume_bad_url_remnants();
                        return TokenKind::BadUrl;
                    }
                }
                Some(c) if is_non_printable(c) => {
                    self.consume_bad_url_remnants();
                    return TokenKind::BadUrl;
                }
                Some(_) => {}
            }
        }
    }

    /// Consume remnants of a bad url (§4.3.14).
    fn consume_bad_url_remnants(&mut self) {
        loop {
            match self.input.next_char() {
                None | Some(')') => return,
                Some('\\') if is_valid_escape(Some('\\'), self.input.current_char()) => {
                    self.consume_escape();
                }
                _ => {}
            }
        }
    }

    /// Consume an escape sequence, returning the code point (§4.3.7).
    fn consume_escape(&mut self) -> char {
        match self.input.next_char() {
            None => '\u{FFFD}',
            Some(c) if c.is_ascii_hexdigit() => {
                let mut hex = String::with_capacity(6);
                hex.push(c);
                for _ in 0..5 {
                    match self.input.current_char() {
                        Some(h) if h.is_ascii_hexdigit() => {
                            hex.push(h);
                            self.input.next_char();
                        }
                        _ => break,
                    }
                }
                // Consume optional trailing whitespace
                if matches!(
                    self.input.current_char(),
                    Some(' ') | Some('\t') | Some('\n')
                ) {
                    self.input.next_char();
                }
                u32::from_str_radix(&hex, 16)
                    .ok()
                    .and_then(char::from_u32)
                    .map(|c| if c == '\0' { '\u{FFFD}' } else { c })
                    .unwrap_or('\u{FFFD}')
            }
            Some(c) => c,
        }
    }

    /// Consume name characters (§4.3.11).
    fn consume_name_chars(&mut self) {
        loop {
            match self.input.current_char() {
                Some(c) if is_name_char(Some(c)) => {
                    self.input.next_char();
                }
                Some('\\') if is_valid_escape(Some('\\'), self.input.peek_char(1)) => {
                    self.input.next_char(); // backslash
                    self.consume_escape();
                }
                _ => return,
            }
        }
    }

    /// Consume characters while a predicate holds.
    fn consume_while(&mut self, predicate: impl Fn(char) -> bool) {
        while let Some(c) = self.input.current_char() {
            if predicate(c) {
                self.input.next_char();
            } else {
                break;
            }
        }
    }
}

impl<'a> Iterator for Tokenizer<'a> {
    type Item = Token<'a>;

    fn next(&mut self) -> Option<Token<'a>> {
        if self.finished {
            return None;
        }
        let token = self.next_token();
        if token.kind == TokenKind::Eof {
            self.finished = true;
            return None;
        }
        Some(token)
    }
}

// --- Helper predicates (§4.3.8–4.3.10) ---

fn is_name_start(c: char) -> bool {
    c.is_ascii_alphabetic() || !c.is_ascii() || c == '_'
}

fn is_name_char(c: Option<char>) -> bool {
    match c {
        Some(c) => is_name_start(c) || c.is_ascii_digit() || c == '-',
        None => false,
    }
}

fn is_non_printable(c: char) -> bool {
    matches!(c, '\x00'..='\x08' | '\x0B' | '\x0E'..='\x1F' | '\x7F')
}

fn is_valid_escape(first: Option<char>, second: Option<char>) -> bool {
    first == Some('\\') && second != Some('\n')
}

/// Would three code points start an identifier? (§4.3.9)
fn would_start_ident(first: Option<char>, second: Option<char>, third: Option<char>) -> bool {
    match first {
        Some('-') => {
            matches!(second, Some(c) if is_name_start(c) || c == '-')
                || is_valid_escape(second, third)
        }
        Some(c) if is_name_start(c) => true,
        Some('\\') => is_valid_escape(first, second),
        _ => false,
    }
}

/// Would three code points start a number? (§4.3.10)
fn would_start_number(first: Option<char>, second: Option<char>, third: Option<char>) -> bool {
    match first {
        Some('+') | Some('-') => match second {
            Some(c) if c.is_ascii_digit() => true,
            Some('.') => third.is_some_and(|c| c.is_ascii_digit()),
            _ => false,
        },
        Some('.') => second.is_some_and(|c| c.is_ascii_digit()),
        Some(c) if c.is_ascii_digit() => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokenize(input: &str) -> Vec<TokenKind<'_>> {
        Tokenizer::new(input).map(|t| t.kind).collect()
    }

    #[test]
    fn simple_ident() {
        assert_eq!(tokenize("color"), vec![TokenKind::Ident("color")]);
    }

    #[test]
    fn ident_with_dash() {
        assert_eq!(
            tokenize("--custom-prop"),
            vec![TokenKind::Ident("--custom-prop")]
        );
    }

    #[test]
    fn function_token() {
        assert_eq!(
            tokenize("rgb("),
            vec![TokenKind::Function("rgb"), TokenKind::Eof][..1],
            // Actually the ( is consumed as part of Function
        );
        let tokens = tokenize("rgb(255)");
        assert_eq!(tokens[0], TokenKind::Function("rgb"));
        assert!(matches!(tokens[1], TokenKind::Number { value, .. } if value == 255.0));
        assert_eq!(tokens[2], TokenKind::CloseParen);
    }

    #[test]
    fn at_keyword() {
        assert_eq!(tokenize("@media"), vec![TokenKind::AtKeyword("media")]);
        assert_eq!(tokenize("@layer"), vec![TokenKind::AtKeyword("layer")]);
    }

    #[test]
    fn hash_id() {
        let tokens = tokenize("#foo");
        assert_eq!(
            tokens,
            vec![TokenKind::Hash {
                value: "foo",
                is_id: true
            }]
        );
    }

    #[test]
    fn hash_unrestricted() {
        let tokens = tokenize("#123");
        assert_eq!(
            tokens,
            vec![TokenKind::Hash {
                value: "123",
                is_id: false
            }]
        );
    }

    #[test]
    fn string_double_quotes() {
        assert_eq!(
            tokenize("\"hello world\""),
            vec![TokenKind::String("hello world")]
        );
    }

    #[test]
    fn string_single_quotes() {
        assert_eq!(tokenize("'hello'"), vec![TokenKind::String("hello")]);
    }

    #[test]
    fn bad_string_newline() {
        let tokens = tokenize("\"unterminated\n");
        assert_eq!(tokens[0], TokenKind::BadString);
    }

    #[test]
    fn number_integer() {
        let tokens = tokenize("42");
        assert_eq!(
            tokens,
            vec![TokenKind::Number {
                value: 42.0,
                int_value: Some(42),
                has_sign: false,
            }]
        );
    }

    #[test]
    #[allow(
        clippy::approx_constant,
        reason = "3.14 is the literal under test, not pi"
    )]
    fn number_float() {
        let tokens = tokenize("3.14");
        assert_eq!(
            tokens,
            vec![TokenKind::Number {
                value: 3.14,
                int_value: None,
                has_sign: false,
            }]
        );
    }

    #[test]
    fn number_scientific() {
        let tokens = tokenize("1e10");
        assert!(matches!(
            &tokens[0],
            TokenKind::Number { value, int_value: None, .. } if (*value - 1e10).abs() < 1.0
        ));
    }

    #[test]
    fn number_signed() {
        let tokens = tokenize("+5");
        assert_eq!(
            tokens,
            vec![TokenKind::Number {
                value: 5.0,
                int_value: Some(5),
                has_sign: true,
            }]
        );
    }

    #[test]
    fn percentage() {
        let tokens = tokenize("50%");
        assert_eq!(
            tokens,
            vec![TokenKind::Percentage {
                value: 50.0,
                int_value: Some(50),
            }]
        );
    }

    #[test]
    fn dimension() {
        let tokens = tokenize("10px");
        assert_eq!(
            tokens,
            vec![TokenKind::Dimension {
                value: 10.0,
                int_value: Some(10),
                unit: "px",
            }]
        );
    }

    #[test]
    fn dimension_em() {
        let tokens = tokenize("2em");
        assert_eq!(
            tokens,
            vec![TokenKind::Dimension {
                value: 2.0,
                int_value: Some(2),
                unit: "em",
            }]
        );
    }

    #[test]
    fn whitespace_collapsed() {
        let tokens = tokenize("  \t\n  ");
        assert_eq!(tokens, vec![TokenKind::Whitespace]);
    }

    #[test]
    fn delimiters() {
        assert_eq!(tokenize("*"), vec![TokenKind::Delim('*')]);
        assert_eq!(tokenize(">"), vec![TokenKind::Delim('>')]);
        assert_eq!(tokenize("~"), vec![TokenKind::Delim('~')]);
    }

    #[test]
    fn punctuation() {
        assert_eq!(tokenize(":"), vec![TokenKind::Colon]);
        assert_eq!(tokenize(";"), vec![TokenKind::Semicolon]);
        assert_eq!(tokenize(","), vec![TokenKind::Comma]);
        assert_eq!(tokenize("{"), vec![TokenKind::OpenCurly]);
        assert_eq!(tokenize("}"), vec![TokenKind::CloseCurly]);
        assert_eq!(tokenize("["), vec![TokenKind::OpenSquare]);
        assert_eq!(tokenize("]"), vec![TokenKind::CloseSquare]);
        assert_eq!(tokenize("("), vec![TokenKind::OpenParen]);
        assert_eq!(tokenize(")"), vec![TokenKind::CloseParen]);
    }

    #[test]
    fn cdo_cdc() {
        assert_eq!(tokenize("<!--"), vec![TokenKind::Cdo]);
        assert_eq!(tokenize("-->"), vec![TokenKind::Cdc]);
    }

    #[test]
    fn url_token() {
        let tokens = tokenize("url(image.png)");
        assert_eq!(tokens, vec![TokenKind::Url("image.png")]);
    }

    #[test]
    fn url_with_whitespace() {
        let tokens = tokenize("url(  image.png  )");
        assert_eq!(tokens, vec![TokenKind::Url("image.png")]);
    }

    #[test]
    fn url_with_quotes_becomes_function() {
        let tokens = tokenize("url(\"image.png\")");
        assert_eq!(tokens[0], TokenKind::Function("url"));
        assert_eq!(tokens[1], TokenKind::String("image.png"));
        assert_eq!(tokens[2], TokenKind::CloseParen);
    }

    #[test]
    fn comment_skipped() {
        let tokens = tokenize("a /* comment */ b");
        assert_eq!(
            tokens,
            vec![
                TokenKind::Ident("a"),
                TokenKind::Whitespace,
                TokenKind::Whitespace,
                TokenKind::Ident("b"),
            ]
        );
    }

    #[test]
    fn full_rule() {
        let tokens = tokenize("h1 { color: red; }");
        assert_eq!(
            tokens,
            vec![
                TokenKind::Ident("h1"),
                TokenKind::Whitespace,
                TokenKind::OpenCurly,
                TokenKind::Whitespace,
                TokenKind::Ident("color"),
                TokenKind::Colon,
                TokenKind::Whitespace,
                TokenKind::Ident("red"),
                TokenKind::Semicolon,
                TokenKind::Whitespace,
                TokenKind::CloseCurly,
            ]
        );
    }

    #[test]
    fn empty_input() {
        assert_eq!(tokenize(""), Vec::<TokenKind>::new());
    }

    #[test]
    fn negative_dimension() {
        let tokens = tokenize("-10px");
        assert_eq!(
            tokens,
            vec![TokenKind::Dimension {
                value: -10.0,
                int_value: Some(-10),
                unit: "px",
            }]
        );
    }
}
