//! CSS Syntax Level 3 tokenizer and parser with CSS Nesting support.
//!
//! MIT/Apache-2.0 licensed. Part of the browser_oxide project.
//!
//! This crate provides:
//! - A zero-copy **tokenizer** per CSS Syntax §4
//! - A **parser** per CSS Syntax §5 with CSS Nesting Module support
//! - Source location tracking on all AST nodes
//! - Error-tolerant parsing (collects errors, always produces a best-effort AST)
//!
//! # Example
//!
//! ```
//! use css_parser::{parse_stylesheet, parse_declaration_list};
//!
//! let (stylesheet, errors) = parse_stylesheet("h1 { color: red; }");
//! assert!(errors.is_empty());
//! assert_eq!(stylesheet.rules.len(), 1);
//!
//! let (decls, _) = parse_declaration_list("color: red; font-size: 16px");
//! assert_eq!(decls.len(), 2);
//! ```

pub mod ast;
pub mod error;
pub mod parser;
pub mod source;
pub mod token;
pub mod tokenizer;

pub use ast::*;
pub use error::ParseError;
pub use parser::Parser;
pub use source::SourceLocation;
pub use token::{resolve_escapes, Token, TokenKind};
pub use tokenizer::Tokenizer;

/// Parse a complete CSS stylesheet.
///
/// Returns the parsed stylesheet and any non-fatal parse errors.
pub fn parse_stylesheet(input: &str) -> (Stylesheet<'_>, Vec<ParseError>) {
    Parser::parse_stylesheet(input)
}

/// Parse a list of CSS declarations (e.g., from a `style` attribute).
///
/// Returns the parsed declarations and any non-fatal parse errors.
pub fn parse_declaration_list(input: &str) -> (Vec<Declaration<'_>>, Vec<ParseError>) {
    Parser::parse_declaration_list(input)
}
