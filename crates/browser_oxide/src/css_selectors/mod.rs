//! CSS Selectors Level 4 parser and matching engine.
//!
//! MIT/Apache-2.0 licensed. Part of the browser_oxide project.
//!
//! # Features
//! - Parses all Selectors Level 4 syntax including `:is()`, `:not()`, `:where()`, `:has()`
//! - Generic `Element` trait works with any DOM implementation
//! - Right-to-left matching for efficiency
//! - Specificity computation per §17
//!
//! # Example
//!
//! ```rust,ignore
//! use css_selectors::{parse_selector_list, matches_selector};
//!
//! let selectors = parse_selector_list("div.content > p:first-child").unwrap();
//! assert!(matches_selector(&some_element, &selectors[0]));
//! ```

pub mod ast;
pub mod element;
pub mod error;
pub mod matching;
pub mod nth;
pub mod parser;
pub mod specificity;

pub use ast::*;
pub use element::Element;
pub use error::SelectorParseError;
pub use matching::{matches_any, matches_selector, query_selector, query_selector_all};
pub use parser::{parse_selector_list, parse_selector_list_forgiving};
pub use specificity::compute_specificity;
