/// A comma-separated list of complex selectors.
pub type SelectorList = Vec<Selector>;

/// A complete selector (one entry in a comma-separated list).
/// Components are stored **right-to-left** for efficient matching.
#[derive(Debug, Clone, PartialEq)]
pub struct Selector {
    pub(crate) components: Vec<Component>,
    pub(crate) specificity: Specificity,
}

impl Selector {
    pub fn new(components: Vec<Component>, specificity: Specificity) -> Self {
        Self {
            components,
            specificity,
        }
    }

    pub fn components(&self) -> &[Component] {
        &self.components
    }

    pub fn specificity(&self) -> Specificity {
        self.specificity
    }
}

/// A component of a selector: either a combinator or a simple selector.
#[derive(Debug, Clone, PartialEq)]
pub enum Component {
    Combinator(Combinator),
    Simple(SimpleSelector),
}

/// Relationship between compound selectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Combinator {
    /// Whitespace: descendant
    Descendant,
    /// `>`: direct child
    Child,
    /// `+`: immediately following sibling
    NextSibling,
    /// `~`: subsequent sibling
    SubsequentSibling,
}

/// A single simple selector (part of a compound selector).
#[derive(Debug, Clone, PartialEq)]
pub enum SimpleSelector {
    /// Type selector: `div`, `span`
    Type(String),
    /// Universal selector: `*`
    Universal,
    /// ID selector: `#foo`
    Id(String),
    /// Class selector: `.bar`
    Class(String),
    /// Attribute selector: `[href]`, `[type="text" i]`
    Attribute {
        name: String,
        operator: Option<AttributeOperator>,
        value: Option<String>,
        case_sensitivity: CaseSensitivity,
    },
    /// Pseudo-class: `:hover`, `:nth-child(2n+1)`, `:is(.foo, .bar)`
    PseudoClass(PseudoClass),
    /// Pseudo-element: `::before`, `::after`
    PseudoElement(PseudoElement),
    /// `&` nesting selector
    Nesting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributeOperator {
    /// `=` exact match
    Exact,
    /// `~=` whitespace-separated word
    Includes,
    /// `|=` exact or prefix with `-`
    DashMatch,
    /// `^=` prefix
    Prefix,
    /// `$=` suffix
    Suffix,
    /// `*=` substring
    Substring,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseSensitivity {
    Default,
    CaseInsensitive,
    CaseSensitive,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PseudoClass {
    // Location
    AnyLink,
    Link,
    Visited,
    Target,

    // User action
    Hover,
    Active,
    Focus,
    FocusWithin,
    FocusVisible,

    // Input
    Enabled,
    Disabled,
    ReadWrite,
    ReadOnly,
    Checked,
    Default,
    Indeterminate,
    Required,
    Optional,
    Valid,
    Invalid,
    InRange,
    OutOfRange,
    PlaceholderShown,

    // Tree-structural
    Root,
    Empty,
    FirstChild,
    LastChild,
    OnlyChild,
    FirstOfType,
    LastOfType,
    OnlyOfType,
    NthChild(NthExpr, Option<SelectorList>),
    NthLastChild(NthExpr, Option<SelectorList>),
    NthOfType(NthExpr),
    NthLastOfType(NthExpr),

    // Linguistic
    Lang(Vec<String>),

    // Functional
    Is(SelectorList),
    Not(SelectorList),
    Where(SelectorList),
    Has(Vec<RelativeSelector>),
}

/// An+B expression for `:nth-child`, `:nth-of-type`, etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NthExpr {
    pub a: i32,
    pub b: i32,
}

impl NthExpr {
    /// Check if index (1-based) matches this An+B expression.
    pub fn matches(&self, index: i32) -> bool {
        if self.a == 0 {
            return index == self.b;
        }
        let diff = index - self.b;
        if self.a > 0 {
            diff >= 0 && diff % self.a == 0
        } else {
            diff <= 0 && diff % self.a == 0
        }
    }
}

/// A relative selector (used in `:has()`).
#[derive(Debug, Clone, PartialEq)]
pub struct RelativeSelector {
    pub combinator: Option<Combinator>,
    pub selector: Selector,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PseudoElement {
    Before,
    After,
    FirstLine,
    FirstLetter,
    Placeholder,
    Selection,
    Part(Vec<String>),
    Slotted(Box<Selector>),
    Custom(String),
}

/// Specificity as (a, b, c) per Selectors §17.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Specificity {
    pub a: u32,
    pub b: u32,
    pub c: u32,
}

impl Specificity {
    pub fn new(a: u32, b: u32, c: u32) -> Self {
        Self { a, b, c }
    }

    pub fn max(self, other: Self) -> Self {
        if self >= other {
            self
        } else {
            other
        }
    }
}

impl std::ops::Add for Specificity {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self {
            a: self.a + rhs.a,
            b: self.b + rhs.b,
            c: self.c + rhs.c,
        }
    }
}

impl std::ops::AddAssign for Specificity {
    fn add_assign(&mut self, rhs: Self) {
        self.a += rhs.a;
        self.b += rhs.b;
        self.c += rhs.c;
    }
}
