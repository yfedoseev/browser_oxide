#[derive(Debug, Clone, thiserror::Error)]
pub enum ValueError {
    #[error("unknown property: {0}")]
    UnknownProperty(String),

    #[error("invalid value: {0}")]
    InvalidValue(String),

    #[error("var() resolution error: {0}")]
    VarResolution(String),
}
