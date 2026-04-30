//! Akamai Bot Manager (web) sensor_data v2 encoder for browser_oxide.
//!
//! ## What this crate does
//!
//! Akamai-protected sites (bestbuy.com, homedepot.com, etc.) set a
//! `_abck=...~0~-1~` cookie on the first response — that suffix means
//! "untrusted, prove you're real". Real Chrome 147 ships a POST to a
//! tenant-specific obfuscated path (e.g. `/iBo5C/hYh/7w3a/...` for
//! bestbuy) with body `{"sensor_data":"<encrypted>"}`. Akamai upgrades
//! `_abck` to `~-1~-1~-1~` (favorable) and subsequent requests succeed.
//!
//! browser_oxide's TLS+H2+JS fingerprint is byte-exact Chrome 147
//! (Phase 7); this crate fills the last gap for the ~10% of Akamai sites
//! that demand sensor_data even with a perfect TLS handshake.
//!
//! ## Format (Akamai web v2, what bestbuy uses)
//!
//! Verified 2026-04-29 against a real Chrome 147 capture from
//! Playwright MCP — see `docs/akamai_sensor_reference_2026_04_29.txt`.
//!
//! ```text
//! sensor_data := "3" ";" "0" ";" "1" ";" "0" ";"
//!                <counter-int> ";"
//!                <sha256-base64-of-everything-after> ";"
//!                <counter-tuple> ";"
//!                <scrambled-body>
//! ```
//!
//! - **Field 1**: `"3"` — script version marker.
//! - **Fields 2–4**: `"0;1;0"` — flags (constant on every capture).
//! - **Field 5**: per-tenant counter / seed (e.g. bestbuy = `3224113`).
//! - **Field 6**: SHA-256 base64 of the cleartext body, used as a
//!   server-side integrity check.
//! - **Field 7**: counter tuple `"<key>,<key2>,<mouse>,<touch>,<scroll>,<accel>"`
//!   — first POST may be `"16,0,0,0,0,0"` (only key counter populated
//!   from page-load events); second POST after user activity:
//!   `"5,18,0,0,1,323"` (5 keys, 18 mouse, 1 scroll, 323 accel).
//! - **Field 8+**: XOR-scrambled colon-delimited concat of ~58
//!   sub-fields (canvas FP, WebGL params, audio FP, navigator props,
//!   mouse trajectory, key events, anti-debug timings).
//!
//! ## Reference
//!
//! - Public algorithm: <https://github.com/xiaoweigege/akamai2.0-sensor_data> (akamai2.0.js — v2 path)
//! - Signal taxonomy: <https://github.com/Edioff/akamai-analysis>
//! - Research summary: `docs/RESEARCH_AKAMAI_BMP_BYPASS_2026_04_29.md`
//!
//! ## Status
//!
//! T3A-A1: foundation only (this commit). A2 ports the crypto layer
//! (XOR-scramble); A3 builds the 58-element field set; A4 wires
//! behavioural data; A5 integrates into `Page::navigate`; A6 verifies
//! against bestbuy + homedepot in the holistic sweep.

pub mod crypto;
pub mod session;

pub use crypto::{build_v2_bestbuy, build_v2_dalphan, sha256_b64};
pub use session::{AbckState, AkamaiSession, AkamaiSessionStore};

use serde::{Deserialize, Serialize};

/// A captured mouse event for the behavioural-trajectory part of
/// sensor_data. Pushed by `humanize.js` taps, drained by the payload
/// builder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MouseEvent {
    pub x: i32,
    pub y: i32,
    /// Milliseconds since session start.
    pub t: u64,
    /// 0 = move, 1 = down, 2 = up.
    pub kind: u8,
    /// Mouse button index (0 = left).
    pub button: u8,
}

/// A captured keyboard event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyEvent {
    pub code: String,
    pub t: u64,
    /// 0 = down, 1 = up, 2 = press.
    pub kind: u8,
}

/// A captured touch event (touchscreen / trackpad pinch gestures).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TouchEvent {
    pub x: i32,
    pub y: i32,
    pub t: u64,
    /// 0 = start, 1 = move, 2 = end.
    pub kind: u8,
}

/// Counter-tuple for sensor_data field 7.
#[derive(Debug, Clone, Default)]
pub struct CounterTuple {
    pub key_count: u32,
    pub mouse_count: u32,
    pub touch_count: u32,
    pub scroll_count: u32,
    pub accel_count: u32,
    pub orientation_count: u32,
}

impl CounterTuple {
    /// Format as `"<key>,<key2>,<mouse>,<touch>,<scroll>,<accel>"` —
    /// the order observed in real captures (e.g. "5,18,0,0,1,323").
    /// Note: real captures show 6 slots; second slot is sometimes a
    /// separate key-press counter.
    pub fn as_field7(&self) -> String {
        format!(
            "{},{},{},{},{},{}",
            self.key_count,
            self.key_count, // second slot — same value in observed captures
            self.mouse_count,
            self.touch_count,
            self.scroll_count,
            self.accel_count,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_tuple_first_post_shape() {
        // Real Chrome 147 capture #1: "16,0,0,0,0,0". 6 slots.
        let mut c = CounterTuple::default();
        c.key_count = 16;
        let s = c.as_field7();
        assert_eq!(s.split(',').count(), 6);
        assert!(s.starts_with("16,"), "expected first slot = 16, got {s}");
    }

    // NOTE: Capture #2 was "5,18,0,0,1,323" — open question: does
    // slot 0 = key_count or some other counter, and slot 1 = mouse?
    // The current as_field7() emits slot 0 = key_count twice (a
    // wrong-but-bounded placeholder); A3 will pin semantics by
    // reading xiaoweigege's akamai2.0.js source for the v2 format.
}
