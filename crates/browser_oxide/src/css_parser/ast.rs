use crate::css_parser::source::SourceLocation;
use crate::css_parser::token::Token;

/// A parsed CSS stylesheet.
#[derive(Debug, Clone)]
pub struct Stylesheet<'a> {
    pub rules: Vec<Rule<'a>>,
    pub loc: SourceLocation,
}

/// A CSS rule: either a qualified rule (selector + block) or an at-rule.
#[derive(Debug, Clone)]
pub enum Rule<'a> {
    Qualified(QualifiedRule<'a>),
    At(AtRule<'a>),
}

/// A qualified rule: `selector { declarations; nested-rules }`.
///
/// With CSS Nesting, qualified rules can contain both declarations
/// and nested rules interleaved.
#[derive(Debug, Clone)]
pub struct QualifiedRule<'a> {
    /// The selector prelude (unparsed tokens — css_selectors parses these).
    pub prelude: Vec<ComponentValue<'a>>,
    /// Property declarations inside the block.
    pub declarations: Vec<Declaration<'a>>,
    /// Nested rules (CSS Nesting).
    pub rules: Vec<Rule<'a>>,
    pub loc: SourceLocation,
}

/// An at-rule: `@name prelude { block }` or `@name prelude;`.
#[derive(Debug, Clone)]
pub struct AtRule<'a> {
    /// The at-rule name (without `@`), e.g. `"media"`, `"layer"`.
    pub name: &'a str,
    /// Tokens between `@name` and `{` or `;`.
    pub prelude: Vec<ComponentValue<'a>>,
    /// The block contents, or None for statement at-rules (`@import ...;`).
    pub block: Option<Block<'a>>,
    pub loc: SourceLocation,
}

/// The contents of an at-rule block.
#[derive(Debug, Clone)]
pub enum Block<'a> {
    /// Contains rules only (e.g. `@media`, `@layer`, `@supports`).
    RuleList(Vec<Rule<'a>>),
    /// Contains declarations and possibly nested rules (e.g. `@font-face`, `@property`).
    DeclarationBlock {
        declarations: Vec<Declaration<'a>>,
        rules: Vec<Rule<'a>>,
    },
}

/// A CSS declaration: `property: value !important?`.
#[derive(Debug, Clone)]
pub struct Declaration<'a> {
    /// The property name (zero-copy from input).
    pub name: &'a str,
    /// The value tokens (everything between `:` and `;` / `}`).
    pub value: Vec<ComponentValue<'a>>,
    /// Whether `!important` was present.
    pub important: bool,
    pub loc: SourceLocation,
}

/// A CSS component value (§5.4.6): a token, function, or simple block.
#[derive(Debug, Clone)]
pub enum ComponentValue<'a> {
    /// A preserved token.
    Token(Token<'a>),
    /// A function: `name(arguments)`.
    Function(CssFunction<'a>),
    /// A block: `{...}`, `[...]`, or `(...)`.
    SimpleBlock(SimpleBlock<'a>),
}

/// A CSS function: `name(arg1, arg2, ...)`.
#[derive(Debug, Clone)]
pub struct CssFunction<'a> {
    pub name: &'a str,
    pub arguments: Vec<ComponentValue<'a>>,
    pub loc: SourceLocation,
}

/// A simple block delimited by `{}`, `[]`, or `()`.
#[derive(Debug, Clone)]
pub struct SimpleBlock<'a> {
    /// The opening token: `{`, `[`, or `(`.
    pub token: char,
    /// The block contents.
    pub value: Vec<ComponentValue<'a>>,
    pub loc: SourceLocation,
}
