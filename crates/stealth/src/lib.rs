//! Stealth fingerprint profiles for browser_oxide.
//!
//! Provides consistent browser identities with 50+ properties
//! that pass anti-bot detection (Cloudflare, DataDome, Akamai, HUMAN).

pub mod aliyun;
pub mod behavior;
pub mod cloudflare;
pub mod douyin;
pub mod gpu;
pub mod kasada;
pub mod ngenix;
pub mod presets;
pub mod profile;
pub mod qrator;

pub use behavior::{BehaviorProfile, Handedness, MousePoint, ScrollStyle, WheelTick};
pub use gpu::GpuProfile;
pub use presets::*;
pub use profile::StealthProfile;
