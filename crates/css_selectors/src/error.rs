use css_parser::SourceLocation;

#[derive(Debug, Clone, thiserror::Error)]
pub enum SelectorParseError {
    #[error("unexpected token at {loc:?}: {message}")]
    UnexpectedToken {
        loc: SourceLocation,
        message: String,
    },

    #[error("unexpected end of selector")]
    UnexpectedEof,

    #[error("empty selector")]
    EmptySelector,

    #[error("invalid An+B expression: {0}")]
    InvalidNth(String),

    #[error("unsupported pseudo-class: {0}")]
    UnsupportedPseudoClass(String),

    #[error("unsupported pseudo-element: {0}")]
    UnsupportedPseudoElement(String),
}
