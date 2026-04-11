//! CSS property value parsing and typed representations.
//!
//! MIT/Apache-2.0 licensed. Part of the browser_oxide project.
//!
//! Parses CSS component values into typed Rust structs for every known property.

pub mod error;
pub mod parse;
pub mod property;
pub mod types;

pub use error::ValueError;
pub use parse::parse_property;
pub use property::*;
pub use types::color::Color;
pub use types::display::*;
pub use types::font::*;
pub use types::length::*;
