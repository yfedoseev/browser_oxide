use crate::css_parser::ast::ComponentValue;
use crate::css_parser::token::TokenKind;

/// Convert CSS component values back to a string.
pub fn tokens_to_string(values: &[ComponentValue]) -> String {
    let mut s = String::new();
    for v in values {
        match v {
            ComponentValue::Token(t) => match &t.kind {
                TokenKind::Ident(name) => s.push_str(name),
                TokenKind::String(val) => {
                    s.push('"');
                    s.push_str(val);
                    s.push('"');
                }
                TokenKind::Hash { value, .. } => {
                    s.push('#');
                    s.push_str(value);
                }
                TokenKind::Number { value, .. } => s.push_str(&value.to_string()),
                TokenKind::Percentage { value, .. } => {
                    s.push_str(&value.to_string());
                    s.push('%');
                }
                TokenKind::Dimension { value, unit, .. } => {
                    s.push_str(&value.to_string());
                    s.push_str(unit);
                }
                TokenKind::Whitespace => s.push(' '),
                TokenKind::Delim(c) => s.push(*c),
                TokenKind::Colon => s.push(':'),
                TokenKind::Semicolon => s.push(';'),
                TokenKind::Comma => s.push(','),
                TokenKind::OpenParen => s.push('('),
                TokenKind::CloseParen => s.push(')'),
                TokenKind::OpenSquare => s.push('['),
                TokenKind::CloseSquare => s.push(']'),
                TokenKind::Function(name) => {
                    s.push_str(name);
                    s.push('(');
                }
                _ => {}
            },
            ComponentValue::Function(f) => {
                s.push_str(f.name);
                s.push('(');
                s.push_str(&tokens_to_string(&f.arguments));
                s.push(')');
            }
            ComponentValue::SimpleBlock(b) => {
                s.push_str(&tokens_to_string(&b.value));
            }
        }
    }
    s
}
