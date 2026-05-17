//! Top-level headless browser API.
//!
//! Provides `Page` — parse HTML, execute JavaScript, extract content.

pub mod classify;
pub mod csp_collector;
pub mod datadome_handler;
pub mod iframe;
mod page;
pub mod parallel;
pub mod pool;
mod script_runner;
pub mod stylesheet_collector;

pub use classify::{engine_classify, EngineClass};
pub use page::{ChallengeVerdict, Page};
pub use parallel::{NavigateResult, ParallelPager};
pub use pool::PagePool;
