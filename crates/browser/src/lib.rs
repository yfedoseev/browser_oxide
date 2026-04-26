//! Top-level headless browser API.
//!
//! Provides `Page` — parse HTML, execute JavaScript, extract content.

pub mod iframe;
mod page;
pub mod pool;
mod script_runner;
pub mod stylesheet_collector;

pub use page::Page;
pub use pool::PagePool;
