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
pub mod drain;
pub mod payload;
pub mod session;

pub use crypto::{build_v2_bestbuy, build_v2_dalphan, sha256_b64};
pub use drain::{parse_drained, Drained, DRAIN_JS};
pub use payload::build_cleartext;
pub use session::{AbckState, AkamaiSession, AkamaiSessionStore};

/// Static registry of known Akamai tenants and their magic constants.
/// T3A-A6 milestone: autonomous bypass for BestBuy.
pub struct TenantSettings {
    pub tenant_seed: i64,
    pub post_path: &'static str,
}

pub fn get_tenant_settings(host: &str) -> Option<TenantSettings> {
    if host.contains("bestbuy.com") {
        Some(TenantSettings {
            tenant_seed: 3_224_113,
            post_path: "/iBo5C/hYh/7w3a/LoSr/yK3l/muuXcz9SiLaEkpiw1u/QRgwWis/cgtYQ/RktbE8B",
        })
    } else {
        // Per-tenant config table is intentionally minimal. Adding a host
        // here without its real `tenant_seed` + obfuscated `post_path` is
        // strictly harmful: we POST a malformed v2 sensor body to the
        // wrong endpoint and the CDN returns 429 (which we mis-attribute
        // to bot scoring). The previous homedepot.com placeholder did
        // exactly this.
        //
        // To add homedepot.com (and other Akamai-protected sites), capture
        // the challenge bootstrap via Playwright MCP:
        //
        //   1. browser_navigate to https://www.homedepot.com/, let the
        //      Akamai challenge run.
        //   2. Read the obfuscated bootstrap script Akamai serves at
        //      <script src="/akam/13/<hash>">. Look for:
        //        - a big numeric constant (analogous to bestbuy's
        //          `3_224_113`) — this is the per-tenant seed.
        //        - a `fetch("/<rand1>/<rand2>/.../<randN>")` call —
        //          this is the obfuscated POST path.
        //   3. Add a new branch here:
        //        } else if host.contains("homedepot.com") {
        //            Some(TenantSettings { tenant_seed: <captured>,
        //                                  post_path: "<captured>" })
        //   4. Verify Page::navigate flips _abck to ~-1~-1~-1~ on
        //      a live request, then re-run the holistic sweep.
        //
        // Without these, returning None is the correct behaviour — the
        // page navigates without our sensor_data POST, which still
        // produces the Akamai-CHL outcome but doesn't pollute the engine
        // signal with a known-wrong POST.
        None
    }
}

/// High-level entry point: produce a complete sensor_data POST body
/// for `host` ready to wrap in `{"sensor_data": "<v>"}`.
///
/// Tenant_seed is the seed observed in the challenge JS for this
/// host (e.g. 3_224_113 for bestbuy). If unknown, pass 0 — Akamai
/// may reject but we'll still see a parseable response.
pub fn build_sensor_data(
    profile: &stealth::StealthProfile,
    session: &AkamaiSession,
    request_url: &str,
    tenant_seed: i64,
) -> String {
    let cleartext = build_cleartext(profile, session, request_url);
    let counter = CounterTuple {
        key_count: session.key_count,
        mouse_count: session.mouse_count,
        touch_count: session.touch_count,
        scroll_count: session.scroll_count,
        accel_count: session.accel_count,
        orientation_count: 0,
    };
    build_v2_bestbuy(
        &cleartext,
        tenant_seed,
        &counter.as_field7(),
        3_289_904, // shuffle seed (DalphanDev default)
        3_683_632, // substitute seed (DalphanDev default)
    )
}

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

    #[test]
    fn end_to_end_build_produces_bestbuy_envelope() {
        // Top-level integration: build_sensor_data() with the bestbuy
        // tenant seed should produce a `3;0;1;0;<seed>;<sha>;<counter>;<body>`
        // envelope that can be wrapped as `{"sensor_data": "<v>"}`.
        let profile = stealth::presets::chrome_130_macos();
        let session = AkamaiSession::default();
        let body = crate::build_sensor_data(
            &profile,
            &session,
            "https://www.bestbuy.com/?intl=nosplash",
            3_224_113, // bestbuy tenant seed (from A0 capture)
        );
        let prefix_parts: Vec<&str> = body.splitn(8, ';').collect();
        assert_eq!(prefix_parts.len(), 8, "envelope is 8 fields (3;0;1;0;seed;sha;counter;body)");
        assert_eq!(prefix_parts[0], "3");
        assert_eq!(prefix_parts[4], "3224113", "tenant seed in field 5");
        // Field 5 is base64 SHA-256 (44 chars + '=')
        assert_eq!(prefix_parts[5].len(), 44);
        assert!(prefix_parts[5].ends_with('='));
        // Body is non-empty
        assert!(!prefix_parts[7].is_empty());
        // Wrap as Akamai expects
        let wrapped = format!("{{\"sensor_data\":\"{}\"}}", body);
        assert!(wrapped.starts_with("{\"sensor_data\":\""));
    }
}
