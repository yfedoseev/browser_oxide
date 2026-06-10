use crate::css_parser::ast::*;
use crate::css_parser::error::ParseError;
use crate::css_parser::source::SourceLocation;
use crate::css_parser::token::{Token, TokenKind};
use crate::css_parser::tokenizer::Tokenizer;

/// CSS parser. Buffers all tokens for nesting disambiguation.
pub struct Parser<'a> {
    tokens: Vec<Token<'a>>,
    pos: usize,
    errors: Vec<ParseError>,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        let tokens: Vec<Token<'a>> = {
            let mut tokenizer = Tokenizer::new(input);
            let mut tokens = Vec::new();
            loop {
                let token = tokenizer.next_token();
                let is_eof = token.kind == TokenKind::Eof;
                tokens.push(token);
                if is_eof {
                    break;
                }
            }
            tokens
        };

        Self {
            tokens,
            pos: 0,
            errors: Vec::new(),
        }
    }

    /// Parse a complete stylesheet (§5.3.3).
    pub fn parse_stylesheet(input: &'a str) -> (Stylesheet<'a>, Vec<ParseError>) {
        let mut parser = Self::new(input);
        let loc = parser.current_location();
        let rules = parser.consume_rule_list(true);
        let errors = parser.errors;
        (Stylesheet { rules, loc }, errors)
    }

    /// Parse a list of declarations (§5.3.5). For `style=""` attributes.
    pub fn parse_declaration_list(input: &'a str) -> (Vec<Declaration<'a>>, Vec<ParseError>) {
        let mut parser = Self::new(input);
        let (declarations, _rules) = parser.consume_block_contents();
        let errors = parser.errors;
        (declarations, errors)
    }

    // --- Token access ---

    fn current(&self) -> &Token<'a> {
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    fn current_kind(&self) -> &TokenKind<'a> {
        &self.current().kind
    }

    fn current_location(&self) -> SourceLocation {
        self.current().loc
    }

    fn advance(&mut self) -> &Token<'a> {
        let token = &self.tokens[self.pos.min(self.tokens.len() - 1)];
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
        token
    }

    fn is_eof(&self) -> bool {
        matches!(self.current_kind(), TokenKind::Eof)
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.current_kind(), TokenKind::Whitespace) {
            self.advance();
        }
    }

    // --- Parsing algorithms (§5.4) ---

    /// Consume a list of rules (§5.4.1).
    fn consume_rule_list(&mut self, top_level: bool) -> Vec<Rule<'a>> {
        let mut rules = Vec::new();
        loop {
            self.skip_whitespace();
            match self.current_kind() {
                TokenKind::Eof => return rules,
                TokenKind::CloseCurly => return rules,
                TokenKind::AtKeyword(_) => {
                    if let Some(rule) = self.consume_at_rule() {
                        rules.push(Rule::At(rule));
                    }
                }
                TokenKind::Cdo | TokenKind::Cdc if top_level => {
                    self.advance();
                }
                _ => {
                    if let Some(rule) = self.consume_qualified_rule() {
                        rules.push(Rule::Qualified(rule));
                    }
                }
            }
        }
    }

    /// Consume an at-rule (§5.4.2).
    fn consume_at_rule(&mut self) -> Option<AtRule<'a>> {
        let loc = self.current_location();
        let name = match self.current_kind() {
            TokenKind::AtKeyword(n) => *n,
            _ => return None,
        };
        self.advance();

        let mut prelude = Vec::new();

        loop {
            match self.current_kind() {
                TokenKind::Semicolon => {
                    self.advance();
                    return Some(AtRule {
                        name,
                        prelude,
                        block: None,
                        loc,
                    });
                }
                TokenKind::Eof => {
                    return Some(AtRule {
                        name,
                        prelude,
                        block: None,
                        loc,
                    });
                }
                TokenKind::OpenCurly => {
                    self.advance(); // {
                    let block = self.consume_at_rule_block(name);
                    // Consume closing }
                    if matches!(self.current_kind(), TokenKind::CloseCurly) {
                        self.advance();
                    }
                    return Some(AtRule {
                        name,
                        prelude,
                        block: Some(block),
                        loc,
                    });
                }
                _ => {
                    prelude.push(self.consume_component_value());
                }
            }
        }
    }

    /// Determine block type for an at-rule and consume accordingly.
    fn consume_at_rule_block(&mut self, name: &str) -> Block<'a> {
        // At-rules that contain rules (not declarations)
        let is_rule_list = matches!(
            name.to_ascii_lowercase().as_str(),
            "media" | "supports" | "layer" | "container" | "scope" | "document" | "keyframes"
        );

        if is_rule_list {
            Block::RuleList(self.consume_rule_list(false))
        } else {
            let (declarations, rules) = self.consume_block_contents();
            Block::DeclarationBlock {
                declarations,
                rules,
            }
        }
    }

    /// Consume a qualified rule (§5.4.3).
    fn consume_qualified_rule(&mut self) -> Option<QualifiedRule<'a>> {
        let loc = self.current_location();
        let mut prelude = Vec::new();

        loop {
            match self.current_kind() {
                TokenKind::Eof => {
                    self.errors.push(ParseError::UnexpectedEof {
                        loc: self.current_location(),
                    });
                    return None;
                }
                TokenKind::OpenCurly => {
                    self.advance(); // {
                    let (declarations, rules) = self.consume_block_contents();
                    // Consume closing }
                    if matches!(self.current_kind(), TokenKind::CloseCurly) {
                        self.advance();
                    }
                    return Some(QualifiedRule {
                        prelude,
                        declarations,
                        rules,
                        loc,
                    });
                }
                _ => {
                    prelude.push(self.consume_component_value());
                }
            }
        }
    }

    /// Consume block contents: interleaved declarations and nested rules (CSS Nesting §5.4.4).
    ///
    /// This is the nesting-aware algorithm. Inside a `{ ... }`, we must
    /// distinguish declarations (`property: value;`) from nested rules
    /// (`selector { ... }`).
    fn consume_block_contents(&mut self) -> (Vec<Declaration<'a>>, Vec<Rule<'a>>) {
        let mut declarations = Vec::new();
        let mut rules = Vec::new();

        loop {
            self.skip_whitespace();
            match self.current_kind() {
                TokenKind::Eof | TokenKind::CloseCurly => {
                    return (declarations, rules);
                }
                TokenKind::Semicolon => {
                    self.advance();
                }
                TokenKind::AtKeyword(_) => {
                    if let Some(at_rule) = self.consume_at_rule() {
                        rules.push(Rule::At(at_rule));
                    }
                }
                // Ident might be a declaration or a type selector
                TokenKind::Ident(_) => {
                    // Try to parse as declaration first (save position for backtrack)
                    let saved_pos = self.pos;
                    if let Some(decl) = self.try_consume_declaration() {
                        declarations.push(decl);
                    } else {
                        // Backtrack and parse as nested qualified rule
                        self.pos = saved_pos;
                        if let Some(rule) = self.consume_qualified_rule() {
                            rules.push(Rule::Qualified(rule));
                        }
                    }
                }
                // `&` nesting selector or other selector-starting tokens → nested rule
                _ => {
                    if let Some(rule) = self.consume_qualified_rule() {
                        rules.push(Rule::Qualified(rule));
                    }
                }
            }
        }
    }

    /// Try to consume a declaration. Returns None if this doesn't look like one.
    fn try_consume_declaration(&mut self) -> Option<Declaration<'a>> {
        let loc = self.current_location();

        // Must start with ident
        let name = match self.current_kind() {
            TokenKind::Ident(n) => *n,
            _ => return None,
        };
        self.advance();

        self.skip_whitespace();

        // Must be followed by `:`
        if !matches!(self.current_kind(), TokenKind::Colon) {
            return None;
        }
        self.advance(); // :

        self.skip_whitespace();

        // Consume value tokens until `;`, `}`, or EOF
        let mut value = Vec::new();
        loop {
            match self.current_kind() {
                TokenKind::Semicolon => {
                    self.advance();
                    break;
                }
                TokenKind::CloseCurly | TokenKind::Eof => {
                    break;
                }
                _ => {
                    value.push(self.consume_component_value());
                }
            }
        }

        // Check for !important at the end of value
        let important = check_and_strip_important(&mut value);

        Some(Declaration {
            name,
            value,
            important,
            loc,
        })
    }

    /// Consume a component value (§5.4.6).
    fn consume_component_value(&mut self) -> ComponentValue<'a> {
        match self.current_kind() {
            TokenKind::OpenCurly | TokenKind::OpenSquare | TokenKind::OpenParen => {
                ComponentValue::SimpleBlock(self.consume_simple_block())
            }
            TokenKind::Function(_) => ComponentValue::Function(self.consume_function()),
            _ => {
                let token = self.advance().clone();
                ComponentValue::Token(token)
            }
        }
    }

    /// Consume a simple block (§5.4.7).
    fn consume_simple_block(&mut self) -> SimpleBlock<'a> {
        let loc = self.current_location();
        let opening = match self.current_kind() {
            TokenKind::OpenCurly => '{',
            TokenKind::OpenSquare => '[',
            TokenKind::OpenParen => '(',
            _ => unreachable!(),
        };
        let closing = match opening {
            '{' => TokenKind::CloseCurly,
            '[' => TokenKind::CloseSquare,
            '(' => TokenKind::CloseParen,
            _ => unreachable!(),
        };
        self.advance(); // opening token

        let mut value = Vec::new();
        loop {
            if *self.current_kind() == closing {
                self.advance();
                return SimpleBlock {
                    token: opening,
                    value,
                    loc,
                };
            }
            if self.is_eof() {
                return SimpleBlock {
                    token: opening,
                    value,
                    loc,
                };
            }
            value.push(self.consume_component_value());
        }
    }

    /// Consume a function (§5.4.8).
    fn consume_function(&mut self) -> CssFunction<'a> {
        let loc = self.current_location();
        let name = match self.current_kind() {
            TokenKind::Function(n) => *n,
            _ => unreachable!(),
        };
        self.advance(); // function token (includes consumed `(`)

        let mut arguments = Vec::new();
        loop {
            match self.current_kind() {
                TokenKind::CloseParen => {
                    self.advance();
                    return CssFunction {
                        name,
                        arguments,
                        loc,
                    };
                }
                TokenKind::Eof => {
                    return CssFunction {
                        name,
                        arguments,
                        loc,
                    };
                }
                _ => {
                    arguments.push(self.consume_component_value());
                }
            }
        }
    }
}

/// Check if the last tokens in a value are `!` `important` and strip them.
fn check_and_strip_important(value: &mut Vec<ComponentValue<'_>>) -> bool {
    // Walk backwards past whitespace
    let mut i = value.len();
    while i > 0 {
        i -= 1;
        match &value[i] {
            ComponentValue::Token(Token {
                kind: TokenKind::Whitespace,
                ..
            }) => continue,
            ComponentValue::Token(Token {
                kind: TokenKind::Ident(name),
                ..
            }) if name.eq_ignore_ascii_case("important") => {
                // Look for `!` before it (past whitespace)
                while i > 0 {
                    i -= 1;
                    match &value[i] {
                        ComponentValue::Token(Token {
                            kind: TokenKind::Whitespace,
                            ..
                        }) => continue,
                        ComponentValue::Token(Token {
                            kind: TokenKind::Delim('!'),
                            ..
                        }) => {
                            value.truncate(i);
                            // Trim trailing whitespace
                            while value
                                .last()
                                .map(|v| {
                                    matches!(
                                        v,
                                        ComponentValue::Token(Token {
                                            kind: TokenKind::Whitespace,
                                            ..
                                        })
                                    )
                                })
                                .unwrap_or(false)
                            {
                                value.pop();
                            }
                            return true;
                        }
                        _ => return false,
                    }
                }
                return false;
            }
            _ => return false,
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_rule() {
        let (stylesheet, errors) = Parser::parse_stylesheet("h1 { color: red; }");
        assert!(errors.is_empty());
        assert_eq!(stylesheet.rules.len(), 1);
        match &stylesheet.rules[0] {
            Rule::Qualified(qr) => {
                assert_eq!(qr.declarations.len(), 1);
                assert_eq!(qr.declarations[0].name, "color");
                assert!(!qr.declarations[0].important);
            }
            _ => panic!("Expected qualified rule"),
        }
    }

    #[test]
    fn parse_important() {
        let (stylesheet, _) = Parser::parse_stylesheet("h1 { color: red !important; }");
        match &stylesheet.rules[0] {
            Rule::Qualified(qr) => {
                assert!(qr.declarations[0].important);
            }
            _ => panic!(),
        }
    }

    #[test]
    fn parse_at_rule_media() {
        let (stylesheet, errors) = Parser::parse_stylesheet("@media screen { h1 { color: red; } }");
        assert!(errors.is_empty());
        assert_eq!(stylesheet.rules.len(), 1);
        match &stylesheet.rules[0] {
            Rule::At(at) => {
                assert_eq!(at.name, "media");
                assert!(at.block.is_some());
                match at.block.as_ref().unwrap() {
                    Block::RuleList(rules) => {
                        assert_eq!(rules.len(), 1);
                    }
                    _ => panic!("Expected rule list"),
                }
            }
            _ => panic!("Expected at-rule"),
        }
    }

    #[test]
    fn parse_at_rule_import() {
        let (stylesheet, _) = Parser::parse_stylesheet("@import url(\"style.css\");");
        match &stylesheet.rules[0] {
            Rule::At(at) => {
                assert_eq!(at.name, "import");
                assert!(at.block.is_none()); // statement at-rule
            }
            _ => panic!(),
        }
    }

    #[test]
    fn parse_nested_rule() {
        let (stylesheet, errors) =
            Parser::parse_stylesheet(".card { color: black; &:hover { color: blue; } }");
        assert!(errors.is_empty(), "errors: {:?}", errors);
        match &stylesheet.rules[0] {
            Rule::Qualified(qr) => {
                assert_eq!(qr.declarations.len(), 1);
                assert_eq!(qr.declarations[0].name, "color");
                assert_eq!(qr.rules.len(), 1);
                match &qr.rules[0] {
                    Rule::Qualified(nested) => {
                        assert_eq!(nested.declarations.len(), 1);
                        assert_eq!(nested.declarations[0].name, "color");
                    }
                    _ => panic!("Expected nested qualified rule"),
                }
            }
            _ => panic!(),
        }
    }

    #[test]
    fn parse_nested_media() {
        let (stylesheet, _) = Parser::parse_stylesheet(
            ".card { color: black; @media (width > 768px) { padding: 2rem; } }",
        );
        match &stylesheet.rules[0] {
            Rule::Qualified(qr) => {
                assert_eq!(qr.declarations.len(), 1);
                assert_eq!(qr.rules.len(), 1);
                match &qr.rules[0] {
                    Rule::At(at) => {
                        assert_eq!(at.name, "media");
                    }
                    _ => panic!(),
                }
            }
            _ => panic!(),
        }
    }

    #[test]
    fn parse_multiple_declarations() {
        let (stylesheet, _) =
            Parser::parse_stylesheet("h1 { color: red; font-size: 2em; margin: 0; }");
        match &stylesheet.rules[0] {
            Rule::Qualified(qr) => {
                assert_eq!(qr.declarations.len(), 3);
                assert_eq!(qr.declarations[0].name, "color");
                assert_eq!(qr.declarations[1].name, "font-size");
                assert_eq!(qr.declarations[2].name, "margin");
            }
            _ => panic!(),
        }
    }

    #[test]
    fn parse_declaration_list_inline() {
        let (decls, _) = Parser::parse_declaration_list("color: red; font-size: 16px");
        assert_eq!(decls.len(), 2);
        assert_eq!(decls[0].name, "color");
        assert_eq!(decls[1].name, "font-size");
    }

    #[test]
    fn parse_at_layer() {
        let (stylesheet, _) = Parser::parse_stylesheet("@layer base, components, utilities;");
        match &stylesheet.rules[0] {
            Rule::At(at) => {
                assert_eq!(at.name, "layer");
                assert!(at.block.is_none()); // statement form
            }
            _ => panic!(),
        }
    }

    #[test]
    fn parse_empty_stylesheet() {
        let (stylesheet, errors) = Parser::parse_stylesheet("");
        assert!(errors.is_empty());
        assert!(stylesheet.rules.is_empty());
    }

    #[test]
    fn parse_empty_rule() {
        let (stylesheet, _) = Parser::parse_stylesheet("h1 {}");
        match &stylesheet.rules[0] {
            Rule::Qualified(qr) => {
                assert!(qr.declarations.is_empty());
                assert!(qr.rules.is_empty());
            }
            _ => panic!(),
        }
    }

    #[test]
    fn error_recovery_unclosed_rule() {
        let (stylesheet, _) = Parser::parse_stylesheet("h1 { color: red; h2 { font-size: 1em; }");
        // Should still parse something without panicking
        assert!(!stylesheet.rules.is_empty());
    }

    #[test]
    fn parse_custom_property() {
        let (stylesheet, _) = Parser::parse_stylesheet(":root { --color: red; }");
        match &stylesheet.rules[0] {
            Rule::Qualified(qr) => {
                assert_eq!(qr.declarations.len(), 1);
                assert_eq!(qr.declarations[0].name, "--color");
            }
            _ => panic!(),
        }
    }

    #[test]
    fn parse_function_in_value() {
        let (stylesheet, _) = Parser::parse_stylesheet("h1 { color: rgb(255, 0, 0); }");
        match &stylesheet.rules[0] {
            Rule::Qualified(qr) => {
                assert_eq!(qr.declarations.len(), 1);
                // The value should contain a function
                let has_fn = qr.declarations[0]
                    .value
                    .iter()
                    .any(|v| matches!(v, ComponentValue::Function(f) if f.name == "rgb"));
                assert!(has_fn, "Expected rgb function in value");
            }
            _ => panic!(),
        }
    }
}
