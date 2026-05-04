//! Per-host Akamai session state, mirroring `KasadaSessionStore` shape
//! at `crates/net/src/kasada_session.rs`.
//!
//! Lifecycle:
//! 1. First request to an Akamai-protected host returns
//!    `Set-Cookie: _abck=...~0~-1~`. `HttpClient` calls `learn_abck()`
//!    which mints an `AkamaiSession` (or refreshes existing) with the
//!    suffix state.
//! 2. `humanize.js` taps push mouse/key/touch events into the
//!    session's behavioural buffers via op_akamai_record_* (T3A-A4).
//! 3. After ~500ms post-load (or on retry), `payload::build()` (T3A-A3)
//!    consumes the buffers + profile state to produce a sensor_data
//!    string. The session's `sensor_counter` is incremented.
//! 4. The HTTP client POSTs `{"sensor_data": "<v>"}` to the
//!    tenant-obfuscated path; response refreshes `_abck`.
//!
//! Until A2/A3 land, `build_sensor_data()` is a stub.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::{KeyEvent, MouseEvent, TouchEvent};

/// Trust state encoded by Akamai's `_abck` cookie suffix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbckState {
    /// `~-1~-1~-1~` — passed, sensor_data accepted.
    Favorable,
    /// `~0~-1~` or `~0~-1~-1~` — sensor_data missing/invalid; we need
    /// to POST one (or POST a better one).
    NeedsSensor,
    /// `~-1~0~-1~` — Akamai demands a sec-cpt PoW challenge. Out of
    /// scope for T3A; we'll detect it and bail noisily for now.
    NeedsSecCpt,
    /// `~-1~-1~0~` — pixel-challenge required (rare). Out of scope.
    NeedsPixel,
    /// Anything else / unparseable.
    Unknown,
}

impl AbckState {
    /// Parse the trust state from a `_abck=` cookie value (everything
    /// after the leading token, i.e. starting at the first `~`).
    /// The suffix format Akamai uses is delimited by `~`; trust slots
    /// are integer fields where `-1` = neutral and `0` = challenge
    /// requested.
    pub fn from_cookie_value(value: &str) -> Self {
        // Split off the leading session token; suffix begins at the
        // first `~`.
        let suffix = match value.find('~') {
            Some(i) => &value[i..],
            None => return Self::Unknown,
        };
        // Tokenise on `~`, drop empty leading split.
        let parts: Vec<&str> = suffix.trim_start_matches('~').split('~').collect();
        // Common shapes (best-effort; Akamai rotates):
        //   ~-1~-1~-1~     — favorable
        //   ~0~-1~-1~      — needs sensor (challenge0)
        //   ~-1~0~-1~      — needs sec-cpt
        //   ~-1~-1~0~      — needs pixel
        // Take the first three numeric slots.
        let nums: Vec<i32> = parts
            .iter()
            .take(4)
            .filter_map(|s| s.parse::<i32>().ok())
            .collect();
        match nums.as_slice() {
            [-1, -1, -1, ..] => Self::Favorable,
            [0, _, _, ..] => Self::NeedsSensor,
            [-1, 0, _, ..] => Self::NeedsSecCpt,
            [-1, -1, 0, ..] => Self::NeedsPixel,
            _ => Self::Unknown,
        }
    }
}

/// Per-host Akamai session state.
#[derive(Debug, Clone, Default)]
pub struct AkamaiSession {
    /// Number of sensor_data POSTs sent so far on this session.
    pub sensor_counter: u32,
    /// Per-tenant counter / seed (`3224113` in the bestbuy capture).
    /// Extracted from the Akamai challenge JS the first time we see it.
    pub tenant_seed: Option<i64>,
    /// Last observed `_abck` raw cookie value (for trust-state polling).
    pub last_abck: Option<String>,
    /// Last observed `bm_sz` cookie value (the v3 PRNG seed; v2 may
    /// also use it as part of body XOR-key derivation).
    pub bm_sz: Option<String>,
    /// Tenant-specific obfuscated POST path observed in the page's
    /// challenge JS, e.g. `/iBo5C/hYh/7w3a/LoSr/yK3l/...`. Set by the
    /// HTML/JS sniffer; falls back to a default if not yet known.
    pub post_path: Option<String>,
    /// Behavioural buffers pushed by `humanize.js` taps (T3A-A4).
    pub mouse_buf: Vec<MouseEvent>,
    pub key_buf: Vec<KeyEvent>,
    pub touch_buf: Vec<TouchEvent>,
    /// Coarse counters for sensor_data field 7.
    pub key_count: u32,
    pub mouse_count: u32,
    pub touch_count: u32,
    pub scroll_count: u32,
    pub accel_count: u32,
}

impl AkamaiSession {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn observe_abck(&mut self, raw: &str) -> AbckState {
        self.last_abck = Some(raw.to_string());
        AbckState::from_cookie_value(raw)
    }

    /// Drain the behavioural buffers (mouse/key/touch). Called by the
    /// payload builder; resets buffers but keeps coarse counters.
    pub fn drain_buffers(&mut self) -> (Vec<MouseEvent>, Vec<KeyEvent>, Vec<TouchEvent>) {
        let m = std::mem::take(&mut self.mouse_buf);
        let k = std::mem::take(&mut self.key_buf);
        let t = std::mem::take(&mut self.touch_buf);
        (m, k, t)
    }
}

/// Per-host Akamai session store. Mirrors `KasadaSessionStore`'s
/// `Arc<RwLock<HashMap<String, …>>>` shape so the integration in
/// `crates/net/src/lib.rs::HttpClient` follows the existing pattern.
#[derive(Debug, Clone, Default)]
pub struct AkamaiSessionStore {
    inner: Arc<RwLock<HashMap<String, AkamaiSession>>>,
}

impl AkamaiSessionStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get-or-create a session for `host`, then call `f` with mutable
    /// access. Returns `f`'s result.
    pub async fn with_session<R>(
        &self,
        host: &str,
        f: impl FnOnce(&mut AkamaiSession) -> R,
    ) -> R {
        let mut guard = self.inner.write().await;
        let session = guard.entry(host.to_string()).or_insert_with(AkamaiSession::new);
        f(session)
    }

    /// Snapshot the current `_abck` trust state for a host without
    /// taking ownership of the session.
    pub async fn abck_state(&self, host: &str) -> Option<AbckState> {
        let guard = self.inner.read().await;
        guard
            .get(host)
            .and_then(|s| s.last_abck.as_deref())
            .map(AbckState::from_cookie_value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_favorable_abck() {
        let raw = "ABCDEF~-1~-1~-1~-1~-1";
        assert_eq!(AbckState::from_cookie_value(raw), AbckState::Favorable);
    }

    #[test]
    fn parse_needs_sensor_abck() {
        // Real-world unfavorable suffix from the holistic sweep logs.
        let raw = "TOK123==~0~-1~-1~-1~-1";
        assert_eq!(AbckState::from_cookie_value(raw), AbckState::NeedsSensor);
    }

    #[test]
    fn parse_needs_sec_cpt() {
        let raw = "TOK==~-1~0~-1~-1";
        assert_eq!(AbckState::from_cookie_value(raw), AbckState::NeedsSecCpt);
    }

    #[test]
    fn parse_needs_pixel() {
        let raw = "TOK==~-1~-1~0~-1";
        assert_eq!(AbckState::from_cookie_value(raw), AbckState::NeedsPixel);
    }

    #[test]
    fn unknown_shapes() {
        assert_eq!(AbckState::from_cookie_value(""), AbckState::Unknown);
        assert_eq!(AbckState::from_cookie_value("notoken"), AbckState::Unknown);
    }

    #[tokio::test]
    async fn store_round_trip() {
        let store = AkamaiSessionStore::new();
        store
            .with_session("bestbuy.com", |s| {
                s.tenant_seed = Some(3_224_113);
                s.observe_abck("TOK==~0~-1~-1~-1~-1");
            })
            .await;
        assert_eq!(
            store.abck_state("bestbuy.com").await,
            Some(AbckState::NeedsSensor)
        );
    }
}
