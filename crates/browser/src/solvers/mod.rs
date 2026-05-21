//! Default [`crate::ChallengeSolver`] implementations for the
//! per-vendor anti-bot challenges the engine ships with today
//! (Akamai BMP, Kasada, DataDome, Cloudflare). Each wrapper here
//! delegates to existing pure-functions / per-host stores living in
//! the corresponding vendor crate (`akamai`, `stealth::kasada`, …)
//! plus the existing `Page::handle_akamai_flow` /
//! `Page::handle_cloudflare_flow` methods.
//!
//! Wrappers are intentionally THIN — Stage 2 of the refactor is
//! additive (vendor logic stays in place, the wrapper provides only
//! the trait-shaped surface). Stage 3 will move the wrapper +
//! underlying vendor logic together into a buildable internal-repo
//! plugin crate.

pub mod akamai;

pub use akamai::AkamaiSolver;
