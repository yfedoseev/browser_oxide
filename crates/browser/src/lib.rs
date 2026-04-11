//! Top-level headless browser API.
//!
//! Provides `Page` — parse HTML, execute JavaScript, extract content.

pub mod iframe;
mod page;
mod script_runner;
pub mod stylesheet_collector;

pub use page::Page;
