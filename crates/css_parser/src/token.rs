use crate::source::SourceLocation;
use std::borrow::Cow;

/// A CSS token with its source location.
#[derive(Debug, Clone, PartialEq)]
pub struct Token<'a> {
    pub kind: TokenKind<'a>,
    pub loc: SourceLocation,
}

/// All CSS token types per CSS Syntax Level 3 §4.
///
/// String slices borrow from the original input (zero-copy).
/// When CSS escapes are present in identifiers/strings, use
/// `resolve_escapes()` to get the decoded value.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind<'a> {
    /// An identifier: `color`, `--custom-prop`, `webkit`
    Ident(&'a str),

    /// A function token: `rgb`, `var`, `calc` (the `(` is consumed but not in the name)
    Function(&'a str),

    /// An at-keyword: `media`, `import`, `layer` (without the `@`)
    AtKeyword(&'a str),

    /// A hash token: `fff`, `my-id` (without the `#`)
    Hash {
        value: &'a str,
        /// True if this hash would be a valid ID selector (starts with ident)
        is_id: bool,
    },

    /// A quoted string (without the quotes, escapes NOT resolved)
    String(&'a str),

    /// An unterminated or otherwise bad string
    BadString,

    /// A `url()` token value (without `url(` and `)`, escapes NOT resolved)
    Url(&'a str),

    /// A malformed url() token
    BadUrl,

    /// A numeric value
    Number {
        value: f64,
        /// Some if the original representation was integer (no `.` or `e`)
        int_value: Option<i64>,
        /// Whether a `+` or `-` sign was present
        has_sign: bool,
    },

    /// A percentage value (e.g., `50%`)
    Percentage { value: f64, int_value: Option<i64> },

    /// A dimension value (e.g., `10px`, `2em`)
    Dimension {
        value: f64,
        int_value: Option<i64>,
        unit: &'a str,
    },

    /// One or more whitespace characters (collapsed to a single token)
    Whitespace,

    /// A single code point delimiter not matched by any other token
    Delim(char),

    /// `:`
    Colon,
    /// `;`
    Semicolon,
    /// `,`
    Comma,
    /// `[`
    OpenSquare,
    /// `]`
    CloseSquare,
    /// `(`
    OpenParen,
    /// `)`
    CloseParen,
    /// `{`
    OpenCurly,
    /// `}`
    CloseCurly,

    /// `<!--` (legacy CDO)
    Cdo,
    /// `-->` (legacy CDC)
    Cdc,

    /// End of input
    Eof,
}

impl<'a> TokenKind<'a> {
    /// Returns true if this is a whitespace token.
    pub fn is_whitespace(&self) -> bool {
        matches!(self, TokenKind::Whitespace)
    }

    /// Returns true if this is an ident token.
    pub fn is_ident(&self) -> bool {
        matches!(self, TokenKind::Ident(_))
    }
}

/// Resolve CSS escape sequences in a raw string slice.
///
/// For the common case (no backslashes), returns `Cow::Borrowed`.
/// When escapes are present, allocates and returns `Cow::Owned`.
pub fn resolve_escapes(raw: &str) -> Cow<'_, str> {
    if !raw.contains('\\') {
        return Cow::Borrowed(raw);
    }

    let mut result = String::with_capacity(raw.len());
    let mut chars = raw.chars();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                None => {
                    // Trailing backslash (shouldn't happen in valid CSS, keep it)
                    result.push(ch);
                }
                Some(next) if next == '\n' => {
                    // Escaped newline: continuation, skip both
                }
                Some(next) if next.is_ascii_hexdigit() => {
                    // Hex escape: up to 6 hex digits
                    let mut hex = String::with_capacity(6);
                    hex.push(next);
                    for _ in 0..5 {
                        match chars.clone().next() {
                            Some(h) if h.is_ascii_hexdigit() => {
                                hex.push(h);
                                chars.next();
                            }
                            _ => break,
                        }
                    }
                    // Optional trailing whitespace after hex escape
                    if let Some(&ws) = chars.as_str().as_bytes().first() {
                        if ws == b' ' || ws == b'\t' || ws == b'\n' {
                            chars.next();
                        }
                    }
                    if let Ok(code) = u32::from_str_radix(&hex, 16) {
                        if let Some(c) = char::from_u32(code) {
                            if c == '\0' {
                                result.push('\u{FFFD}');
                            } else {
                                result.push(c);
                            }
                        } else {
                            result.push('\u{FFFD}');
                        }
                    } else {
                        result.push('\u{FFFD}');
                    }
                }
                Some(next) => {
                    // Any other escaped character
                    result.push(next);
                }
            }
        } else {
            result.push(ch);
        }
    }

    Cow::Owned(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_no_escapes() {
        let result = resolve_escapes("hello");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "hello");
    }

    #[test]
    fn resolve_hex_escape() {
        assert_eq!(resolve_escapes("\\26 "), "&");
        assert_eq!(resolve_escapes("\\000026 "), "&");
    }

    #[test]
    fn resolve_simple_escape() {
        assert_eq!(resolve_escapes("\\("), "(");
        assert_eq!(resolve_escapes("hello\\.world"), "hello.world");
    }

    #[test]
    fn resolve_null_escape() {
        assert_eq!(resolve_escapes("\\0 "), "\u{FFFD}");
    }

    #[test]
    fn resolve_newline_continuation() {
        assert_eq!(resolve_escapes("hel\\\nlo"), "hello");
    }
}
