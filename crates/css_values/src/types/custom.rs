/// A `var()` reference.
#[derive(Debug, Clone, PartialEq)]
pub struct VarReference {
    /// Custom property name (e.g. `"--spacing"`).
    pub name: String,
    /// Fallback value (raw CSS text) if the property is not defined.
    pub fallback: Option<String>,
}

/// An `env()` reference.
#[derive(Debug, Clone, PartialEq)]
pub struct EnvReference {
    pub name: String,
    pub fallback: Option<String>,
}
