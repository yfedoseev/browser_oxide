//! Layout engine for browser_oxide.
//!
//! Uses taffy for CSS Block, Flexbox, and Grid layout computation.
//! Provides getBoundingClientRect(), offsetWidth, getComputedStyle, etc.

pub mod engine;
pub mod layout_unit;
pub mod query;
pub mod resolve;
pub mod style_map;
pub mod viewport;

pub use engine::LayoutEngine;
pub use layout_unit::LayoutUnit;
pub use query::DOMRect;
pub use viewport::Viewport;
