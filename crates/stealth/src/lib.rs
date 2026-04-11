//! Stealth fingerprint profiles for browser_oxide.
//!
//! Provides consistent browser identities with 50+ properties
//! that pass anti-bot detection (Cloudflare, DataDome, Akamai, HUMAN).

pub mod gpu;
pub mod presets;
pub mod profile;

pub use gpu::GpuProfile;
pub use presets::*;
pub use profile::StealthProfile;
