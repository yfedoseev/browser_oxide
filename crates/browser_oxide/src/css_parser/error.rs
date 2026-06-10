use crate::css_parser::source::SourceLocation;

/// A CSS parse error. CSS parsing is error-tolerant: these are collected
/// but do not stop parsing.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ParseError {
    #[error("unexpected token at {loc:?}: expected {expected}, found {found}")]
    UnexpectedToken {
        loc: SourceLocation,
        expected: String,
        found: String,
    },

    #[error("unexpected end of input at {loc:?}")]
    UnexpectedEof { loc: SourceLocation },

    #[error("invalid CSS at {loc:?}: {message}")]
    Invalid {
        loc: SourceLocation,
        message: String,
    },
}
