//! Stealth fingerprint profiles for browser_oxide.
//!
//! Provides consistent browser identities — UA string, screen, locale,
//! GPU vendor/renderer, TLS impersonation label, behavioural input model
//! — so the engine reports a coherent "I am Chrome 148 on macOS" surface
//! rather than a default headless fingerprint.

pub mod behavior;
pub mod config;
pub mod gpu;
pub mod presets;
pub mod profile;

pub use behavior::{BehaviorProfile, Handedness, MousePoint, ScrollStyle, WheelTick};
pub use config::{ConfigError, ConfigFormat};
pub use gpu::GpuProfile;
pub use presets::*;
pub use profile::{DeviceClass, StealthProfile};
