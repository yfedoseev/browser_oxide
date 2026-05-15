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
///
/// **Wire format (verified bestbuy / homedepot / macys captures
/// 2026-05-13):**
/// ```text
/// <hex-id>~<stop-signal>~<opaque-base64-blob>~<inv-signal>~<reserved>~<reserved>~<reserved>~<reserved>
/// ```
/// - slot 0: hex session ID
/// - slot 1: stop-signal — `-1` means Akamai hasn't issued a threshold
///   yet; positive `N` means "POST `N` sensor_data envelopes total".
/// - slot 2: opaque base64 blob (not numeric)
/// - slot 3: invalidation signal — `-1` valid; any other value = killed.
/// - slots 4-7: reserved.
///
/// Pre-2026-05-14 our parser treated slot 1 as a binary trust flag
/// (`-1` = Favorable, `0` = NeedsSensor). That's wrong: the very first
/// response is always `<hex>~-1~<blob>~-1~-1~-1~-1~-1` (slot 1 still
/// undecided) and we silently classified it Favorable → never POSTed a
/// sensor. Fixed parser uses Hyper SDK's `IsCookieValid` semantics:
/// favorable iff `sensor_counter >= slot1 && slot3 == -1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbckState {
    /// Stop-signal satisfied (we've POSTed enough sensors) AND
    /// invalidation slot is `-1`. Safe to proceed.
    Favorable,
    /// Either slot 1 == `-1` (no threshold yet — must POST first sensor)
    /// or `sensor_counter < slot1` (POST more sensors).
    NeedsSensor,
    /// `~-1~0~-1~` — Akamai demands a sec-cpt PoW challenge. Detected
    /// from response status (428) rather than cookie shape — kept here
    /// for back-compat with consumers but never returned by the parser.
    NeedsSecCpt,
    /// Pixel-challenge required. Same as above — surfaced by response
    /// path, not cookie parsing.
    NeedsPixel,
    /// Cookie present, slot 3 != -1 — Akamai killed our session.
    /// Caller should drop the session and start fresh.
    Invalidated,
    /// No cookie observed yet (first request hasn't completed) or
    /// malformed cookie value.
    Unknown,
}

/// Parsed slots from an `_abck` cookie. Carries the raw numeric fields;
/// `evaluate()` converts to `AbckState` given the session's sensor counter.
#[derive(Debug, Clone, Copy)]
pub struct ParsedAbck {
    pub stop_signal: i32,
    pub inv_signal: i32,
    pub well_formed: bool,
}

impl ParsedAbck {
    pub fn parse(value: &str) -> Self {
        // Skip the hex session id (everything before the first `~`).
        let suffix = match value.find('~') {
            Some(i) => &value[i + 1..],
            None => {
                return Self {
                    stop_signal: -1,
                    inv_signal: 0,
                    well_formed: false,
                };
            }
        };
        let parts: Vec<&str> = suffix.split('~').collect();
        // parts[0] = stop-signal, parts[1] = base64 blob, parts[2] = inv-signal
        let stop = parts
            .first()
            .and_then(|s| s.parse::<i32>().ok())
            .unwrap_or(-1);
        // Note: parts[1] is the opaque blob, parts[2] is the inv signal.
        let inv = parts
            .get(2)
            .and_then(|s| s.parse::<i32>().ok())
            .unwrap_or(-1);
        Self {
            stop_signal: stop,
            inv_signal: inv,
            well_formed: parts.len() >= 3,
        }
    }

    /// Evaluate against the session's sensor counter — Hyper SDK
    /// `IsCookieValid` semantics: cookie is favorable iff stop_signal is
    /// non-negative AND sensor_counter >= stop_signal AND inv_signal == -1.
    pub fn evaluate(&self, sensor_counter: u32) -> AbckState {
        if !self.well_formed {
            return AbckState::Unknown;
        }
        if self.inv_signal != -1 {
            return AbckState::Invalidated;
        }
        if self.stop_signal == -1 {
            // Akamai hasn't issued a threshold yet — we MUST POST a sensor.
            return AbckState::NeedsSensor;
        }
        if (sensor_counter as i32) >= self.stop_signal {
            AbckState::Favorable
        } else {
            AbckState::NeedsSensor
        }
    }
}

impl AbckState {
    /// Parse the trust state from a `_abck=` cookie value, assuming zero
    /// sensors posted so far. Prefer the per-session evaluation via
    /// `AkamaiSession::observe_abck` / `AkamaiSessionStore::abck_state`
    /// (both pass the real sensor counter).
    pub fn from_cookie_value(value: &str) -> Self {
        ParsedAbck::parse(value).evaluate(0)
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
        ParsedAbck::parse(raw).evaluate(self.sensor_counter)
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
    pub async fn with_session<R>(&self, host: &str, f: impl FnOnce(&mut AkamaiSession) -> R) -> R {
        let mut guard = self.inner.write().await;
        let session = guard
            .entry(host.to_string())
            .or_insert_with(AkamaiSession::new);
        f(session)
    }

    /// Snapshot the current `_abck` trust state for a host without
    /// taking ownership of the session. Evaluates against the session's
    /// actual `sensor_counter` (Hyper SDK `IsCookieValid` semantics).
    pub async fn abck_state(&self, host: &str) -> Option<AbckState> {
        let guard = self.inner.read().await;
        guard.get(host).and_then(|s| {
            s.last_abck
                .as_deref()
                .map(|raw| ParsedAbck::parse(raw).evaluate(s.sensor_counter))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_response_needs_sensor() {
        // The canonical first-response shape: slot 1 = -1 (no stop-signal
        // yet), slot 3 = -1 (not invalidated). Pre-fix this mapped to
        // Favorable and we silently never POSTed a sensor.
        let raw = "D5DBF1BE64D4716D17D9837F6FC3810E~-1~base64blobplaceholder~-1~-1~-1~-1~-1";
        assert_eq!(AbckState::from_cookie_value(raw), AbckState::NeedsSensor);
    }

    #[test]
    fn parsed_carries_slots() {
        let raw = "TOK==~7~someBase64Blob~-1~-1~-1~-1~-1";
        let parsed = ParsedAbck::parse(raw);
        assert_eq!(parsed.stop_signal, 7);
        assert_eq!(parsed.inv_signal, -1);
        assert!(parsed.well_formed);
    }

    #[test]
    fn stop_signal_unsatisfied_needs_more_sensors() {
        let raw = "TOK==~7~someBase64Blob~-1~-1~-1~-1~-1";
        // Sent 3 sensors, threshold is 7 → still NeedsSensor.
        assert_eq!(ParsedAbck::parse(raw).evaluate(3), AbckState::NeedsSensor);
    }

    #[test]
    fn stop_signal_satisfied_is_favorable() {
        let raw = "TOK==~7~someBase64Blob~-1~-1~-1~-1~-1";
        // Sent 7 sensors, threshold satisfied → Favorable.
        assert_eq!(ParsedAbck::parse(raw).evaluate(7), AbckState::Favorable);
        // And 8 (more than threshold) is still Favorable.
        assert_eq!(ParsedAbck::parse(raw).evaluate(8), AbckState::Favorable);
    }

    #[test]
    fn invalidated_when_slot3_nonneg() {
        // slot 3 = 0 = invalidated (Akamai killed the session).
        let raw = "TOK==~7~someBase64Blob~0~-1~-1~-1~-1";
        assert_eq!(ParsedAbck::parse(raw).evaluate(10), AbckState::Invalidated);
    }

    #[test]
    fn unknown_shapes() {
        assert_eq!(AbckState::from_cookie_value(""), AbckState::Unknown);
        assert_eq!(AbckState::from_cookie_value("notoken"), AbckState::Unknown);
    }

    #[tokio::test]
    async fn store_round_trip_first_response_needs_sensor() {
        let store = AkamaiSessionStore::new();
        store
            .with_session("bestbuy.com", |s| {
                s.tenant_seed = Some(1_647_451_213);
                s.observe_abck("HEXID~-1~base64blob~-1~-1~-1~-1~-1");
            })
            .await;
        // sensor_counter = 0 (no posts), slot1 = -1 → must POST.
        assert_eq!(
            store.abck_state("bestbuy.com").await,
            Some(AbckState::NeedsSensor)
        );
    }

    #[tokio::test]
    async fn store_evaluates_with_sensor_counter() {
        let store = AkamaiSessionStore::new();
        store
            .with_session("bestbuy.com", |s| {
                s.sensor_counter = 5;
                s.observe_abck("HEXID~5~base64blob~-1~-1~-1~-1~-1");
            })
            .await;
        // sensor_counter = 5 >= slot1 = 5 → Favorable.
        assert_eq!(
            store.abck_state("bestbuy.com").await,
            Some(AbckState::Favorable)
        );
    }
}
