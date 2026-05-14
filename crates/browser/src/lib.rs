//! Top-level headless browser API.
//!
//! Provides `Page` — parse HTML, execute JavaScript, extract content.

pub mod csp_collector;
pub mod datadome_handler;
pub mod iframe;
mod page;
pub mod parallel;
pub mod pool;
mod script_runner;
pub mod stylesheet_collector;

pub use page::Page;
pub use parallel::{NavigateResult, ParallelPager};
pub use pool::PagePool;
