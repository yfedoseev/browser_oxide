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

/// Decoded Akamai `Server-Timing: ak_p` BotScoreVector.
///
/// Akamai's edge appends a `Server-Timing: ak_p; desc="..."` header to
/// every response from a Bot Manager-protected origin. The `desc`
/// value carries six underscore-separated risk sub-scores per
/// 02_AKAMAI.md §10:
///
/// ```text
/// desc="<request_id>_<timestamp>_<score_a>_<score_b>_<score_c>_<score_d>_<score_e>_<score_f>-"
/// ```
///
/// Lower scores → more human. A jump in any sub-score across runs is a
/// regression signal that pinpoints which engine fingerprint we just
/// broke. Used as a passive diagnostic; never as a gating condition.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BotScoreVector {
    pub request_id: Option<String>,
    pub timestamp: Option<u64>,
    pub score_a: u32,
    pub score_b: u32,
    pub score_c: u32,
    pub score_d: u32,
    pub score_e: u32,
    pub score_f: u32,
}

impl BotScoreVector {
    /// Parse a raw `Server-Timing` header value containing `ak_p; desc="..."`.
    /// Returns `None` if the header doesn't contain an `ak_p` entry or the
    /// `desc` value isn't the expected `_`-separated tuple.
    ///
    /// A `Server-Timing` header may concatenate multiple metrics with
    /// commas; we scan for the `ak_p` entry specifically. Quoted `desc=`
    /// values may or may not include the trailing dash sentinel.
    pub fn parse(server_timing: &str) -> Option<Self> {
        for entry in server_timing.split(',') {
            let entry = entry.trim();
            if !entry.starts_with("ak_p") {
                continue;
            }
            // Find `desc=` segment (case-insensitive).
            let desc_idx = entry
                .to_ascii_lowercase()
                .find("desc=")
                .map(|i| i + "desc=".len());
            let desc_value = match desc_idx {
                Some(i) => entry[i..].trim_matches('"').trim_matches('\''),
                None => continue,
            };
            // The trailing `-` is a sentinel; strip if present.
            let stripped = desc_value.trim_end_matches('-');
            let parts: Vec<&str> = stripped.split('_').collect();
            if parts.len() < 8 {
                return None;
            }
            return Some(BotScoreVector {
                request_id: parts.first().filter(|s| !s.is_empty()).map(|s| s.to_string()),
                timestamp: parts.get(1).and_then(|s| s.parse().ok()),
                score_a: parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0),
                score_b: parts.get(3).and_then(|s| s.parse().ok()).unwrap_or(0),
                score_c: parts.get(4).and_then(|s| s.parse().ok()).unwrap_or(0),
                score_d: parts.get(5).and_then(|s| s.parse().ok()).unwrap_or(0),
                score_e: parts.get(6).and_then(|s| s.parse().ok()).unwrap_or(0),
                score_f: parts.get(7).and_then(|s| s.parse().ok()).unwrap_or(0),
            });
        }
        None
    }

    /// Sum of all six sub-scores. A useful single-number proxy for
    /// "how bot-shaped did Akamai think this request was".
    pub fn total(&self) -> u32 {
        self.score_a + self.score_b + self.score_c + self.score_d + self.score_e + self.score_f
    }
}

/// Static registry of known Akamai tenants and their magic constants.
/// T3A-A6 milestone: autonomous bypass for BestBuy.
pub struct TenantSettings {
    pub tenant_seed: i64,
    pub post_path: &'static str,
}

/// Parsed Akamai tenant configuration extracted from a live HTML
/// response. Per 02_AKAMAI.md §3.4: per-tenant seeds rotate (verified
/// 2026-05-13: bestbuy 3_224_113 → 1_647_451_213, homedepot
/// 3_420_213 → 534_393_124). Static registries can't keep up. We must
/// parse the seed + obfuscated paths from each rendered page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedTenant {
    /// The integer in `bazadebezolkohpepadr="<digits>"`. Field 5 of the
    /// v2/v3 sensor_data envelope.
    pub tenant_seed: i64,
    /// The deep obfuscated POST path Akamai bootstraps for sensor_data.
    /// Format examples (per 02_AKAMAI.md §3 table):
    ///   /iBo5C/hYh/7w3a/LoSr/yK3l/...
    ///   /R8CjSca6_7i6/TepMG7/yyZyaB/...
    pub sensor_post_path: String,
    /// `/akam/<version>/<hash>` pixel bootstrap path.
    pub pixel_bootstrap_path: Option<String>,
}

/// Parse Akamai tenant config out of a freshly fetched HTML body.
///
/// Returns `Some(ParsedTenant)` iff both the `bazadebezolkohpepadr`
/// seed AND a deep obfuscated `<script src="...">` POST path are
/// present. Returns `None` if either is missing — the absence of one
/// means this host either isn't Akamai-protected or is serving us a
/// Bot-or-Not / SBSD interstitial that doesn't carry the bootstrap.
pub fn parse_tenant_from_html(html: &str) -> Option<ParsedTenant> {
    let tenant_seed = parse_tenant_seed(html)?;
    let sensor_post_path = parse_sensor_post_path(html)?;
    let pixel_bootstrap_path = parse_pixel_bootstrap_path(html);
    Some(ParsedTenant {
        tenant_seed,
        sensor_post_path,
        pixel_bootstrap_path,
    })
}

fn parse_tenant_seed(html: &str) -> Option<i64> {
    let key = "bazadebezolkohpepadr=";
    let start = html.find(key)? + key.len();
    let rest = html.get(start..)?;
    // Accept either `"<digits>"` or `'<digits>'`.
    let q = rest.chars().next()?;
    if q != '"' && q != '\'' {
        return None;
    }
    let close = rest[1..].find(q)?;
    let digits = &rest[1..1 + close];
    digits.parse::<i64>().ok()
}

fn parse_sensor_post_path(html: &str) -> Option<String> {
    // Walk every `<script ... src="..."` occurrence and pick the first
    // path that:
    //   - starts with `/`
    //   - has ≥4 slash-separated segments (the bestbuy capture has 5+)
    //   - each segment uses [A-Za-z0-9_-]
    //   - is NOT `/akam/...` (that's the pixel bootstrap, not the
    //     sensor_data POST path)
    let mut cursor = 0;
    while let Some(rel) = html[cursor..].find("<script") {
        let abs = cursor + rel;
        cursor = abs + "<script".len();
        // Find src="..."
        let after_tag_end = html[cursor..].find('>')?;
        let tag = &html[cursor..cursor + after_tag_end];
        let Some(src_idx) = find_attr(tag, "src") else {
            continue;
        };
        let attr = &tag[src_idx..];
        let q = attr.chars().next()?;
        if q != '"' && q != '\'' {
            continue;
        }
        let close = attr[1..].find(q)?;
        let path = &attr[1..1 + close];
        if !path.starts_with('/') {
            continue;
        }
        if path.starts_with("/akam/") {
            continue;
        }
        let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        if segments.len() < 4 {
            continue;
        }
        if !segments
            .iter()
            .all(|seg| seg.chars().all(is_obfuscated_seg_char))
        {
            continue;
        }
        return Some(path.to_string());
    }
    None
}

fn parse_pixel_bootstrap_path(html: &str) -> Option<String> {
    let mut cursor = 0;
    while let Some(rel) = html[cursor..].find("<script") {
        let abs = cursor + rel;
        cursor = abs + "<script".len();
        let after_tag_end = html[cursor..].find('>')?;
        let tag = &html[cursor..cursor + after_tag_end];
        let Some(src_idx) = find_attr(tag, "src") else {
            continue;
        };
        let attr = &tag[src_idx..];
        let q = attr.chars().next()?;
        if q != '"' && q != '\'' {
            continue;
        }
        let close = attr[1..].find(q)?;
        let path = &attr[1..1 + close];
        if !path.starts_with("/akam/") {
            continue;
        }
        // /akam/<version>/<hash> — exactly 3 segments after the leading /
        let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        if segments.len() != 3 {
            continue;
        }
        if !segments[1].chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        if !segments[2].chars().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }
        return Some(path.to_string());
    }
    None
}

/// Locate the start of an attribute value (skipping the attribute name
/// and `=`). Returns the byte offset of the opening quote relative to
/// `tag`, or `None` if the attribute isn't present.
fn find_attr(tag: &str, name: &str) -> Option<usize> {
    let needle = format!(" {name}=");
    let pos = tag.find(&needle)?;
    Some(pos + needle.len())
}

fn is_obfuscated_seg_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

pub fn get_tenant_settings(host: &str) -> Option<TenantSettings> {
    if host.contains("bestbuy.com") {
        Some(TenantSettings {
            tenant_seed: 3_224_113,
            post_path: "/iBo5C/hYh/7w3a/LoSr/yK3l/muuXcz9SiLaEkpiw1u/QRgwWis/cgtYQ/RktbE8B",
        })
    } else if host.contains("homedepot.com") {
        // Captured 2026-05-10 via Playwright MCP (W17 in PLAN_2026_05_10).
        // Real Chrome 147 from a residential macOS profile navigates
        // homedepot.com → Akamai sensor_data POST goes to the obfuscated
        // path below with `{"sensor_data":"3;0;1;0;3420213;..."}` body.
        // Tenant seed (field 5) = 3_420_213. Verified across 2 captured
        // POSTs in the same session.
        Some(TenantSettings {
            tenant_seed: 3_420_213,
            post_path: "/R8CjSca6_7i6/TepMG7/yyZyaB/1z5kQJkkNz4V0tS1fY/IjUxRBpiDAI/KRkJCEx/PelsB",
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
    // Derive key_down / key_up from the session's drained key buffer.
    // kind=0 → down, kind=1 → up, kind=2 → press (counted on neither side
    // per spec — synthetic keypress events are deprecated and absent in
    // real Chrome 147 captures).
    let key_down_count = session.key_buf.iter().filter(|e| e.kind == 0).count() as u32;
    let key_up_count = session.key_buf.iter().filter(|e| e.kind == 1).count() as u32;
    let counter = CounterTuple {
        key_down_count,
        key_up_count,
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
///
/// Real Chrome 147 captures emit 6 distinct counters per field 7:
/// `"<key_down>,<key_up>,<mouse>,<touch>,<scroll>,<accel>"`. Pre-W2.4
/// our `key_count` was duplicated into slots 0 and 1, producing
/// `"K,K,M,T,S,A"` which Akamai scored as bot-shaped because real
/// keyboards generate `key_down >= key_up` (every keyup is preceded
/// by a keydown, but a keydown without a subsequent keyup signals
/// long-hold keys / repeats / focus loss). Two-counter shape is the
/// canonical capture `"5,18,0,0,1,323"`.
#[derive(Debug, Clone, Default)]
pub struct CounterTuple {
    pub key_down_count: u32,
    pub key_up_count: u32,
    pub mouse_count: u32,
    pub touch_count: u32,
    pub scroll_count: u32,
    pub accel_count: u32,
    pub orientation_count: u32,
}

impl CounterTuple {
    /// Format as `"<key_down>,<key_up>,<mouse>,<touch>,<scroll>,<accel>"`
    /// per real Chrome 147 capture order.
    pub fn as_field7(&self) -> String {
        format!(
            "{},{},{},{},{},{}",
            self.key_down_count,
            self.key_up_count,
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
        // Real Chrome 147 capture #1: "16,0,0,0,0,0". 6 slots — typing
        // 16 keys without releasing (sticky-key hold or window focus
        // lost between down and up).
        let c = CounterTuple {
            key_down_count: 16,
            ..Default::default()
        };
        let s = c.as_field7();
        assert_eq!(s.split(',').count(), 6);
        assert_eq!(s, "16,0,0,0,0,0");
    }

    #[test]
    fn counter_tuple_capture_2_shape() {
        // Real Chrome 147 capture #2: "5,18,0,0,1,323". W2.4 — first
        // two slots are distinct counters, not duplicated key_count.
        // 5 keys down, 18 keys up (long-hold release flurry), 1 scroll,
        // 323 accel events.
        let c = CounterTuple {
            key_down_count: 5,
            key_up_count: 18,
            mouse_count: 0,
            touch_count: 0,
            scroll_count: 1,
            accel_count: 323,
            ..Default::default()
        };
        assert_eq!(c.as_field7(), "5,18,0,0,1,323");
    }

    #[test]
    fn bot_score_vector_parses_real_capture() {
        // From 02_AKAMAI.md §10: hotels.com response shows
        // `ak_p; desc="0.75951eb8.1778739141_1778739141_14_144_0_0_0_0-"`
        // → request_id="0.75951eb8.1778739141", timestamp=1778739141,
        //   scores = 14, 144, 0, 0, 0, 0. The `144` is the elevated
        //   detection score that drove the 429.
        let h = r#"ak_p; desc="0.75951eb8.1778739141_1778739141_14_144_0_0_0_0-""#;
        let v = BotScoreVector::parse(h).expect("parse");
        assert_eq!(v.request_id.as_deref(), Some("0.75951eb8.1778739141"));
        assert_eq!(v.timestamp, Some(1778739141));
        assert_eq!(v.score_a, 14);
        assert_eq!(v.score_b, 144);
        assert_eq!(v.score_c, 0);
        assert_eq!(v.score_d, 0);
        assert_eq!(v.score_e, 0);
        assert_eq!(v.score_f, 0);
        assert_eq!(v.total(), 158);
    }

    #[test]
    fn bot_score_vector_handles_multi_metric_header() {
        let h = r#"edge; dur=1, origin; dur=44, ak_p; desc="rid_1234567890_28_257_0_0_0_0-""#;
        let v = BotScoreVector::parse(h).expect("parse");
        assert_eq!(v.score_a, 28);
        assert_eq!(v.score_b, 257);
    }

    #[test]
    fn bot_score_vector_returns_none_without_ak_p() {
        assert!(BotScoreVector::parse("edge; dur=1, origin; dur=44").is_none());
    }

    #[test]
    fn parse_tenant_bestbuy_shape() {
        let html = r#"<!doctype html>
<html><head>
<script type="text/javascript" src="/iBo5C/hYh/7w3a/LoSr/yK3l/muuXcz9SiLaEkpiw1u/QRgwWis/cgtYQ/RktbE8B"></script>
<script src="/akam/13/3e35295b"></script>
<meta name="bazadebezolkohpepadr" content="...">
</head><body bazadebezolkohpepadr="1647451213"></body></html>"#;
        let t = parse_tenant_from_html(html).expect("parsed");
        assert_eq!(t.tenant_seed, 1_647_451_213);
        assert_eq!(t.sensor_post_path, "/iBo5C/hYh/7w3a/LoSr/yK3l/muuXcz9SiLaEkpiw1u/QRgwWis/cgtYQ/RktbE8B");
        assert_eq!(t.pixel_bootstrap_path.as_deref(), Some("/akam/13/3e35295b"));
    }

    #[test]
    fn parse_tenant_homedepot_shape() {
        let html = r##"<html><head>
<script src="/R8CjSca6_7i6/TepMG7/yyZyaB/1z5kQJkkNz4V0tS1fY/IjUxRBpiDAI/KRkJCEx/PelsB"></script>
<script defer src="/akam/13/8a0fbc"></script>
</head><body bazadebezolkohpepadr="534393124"></body></html>"##;
        let t = parse_tenant_from_html(html).expect("parsed");
        assert_eq!(t.tenant_seed, 534_393_124);
        assert!(t.sensor_post_path.starts_with("/R8CjSca6_7i6/"));
        assert_eq!(t.pixel_bootstrap_path.as_deref(), Some("/akam/13/8a0fbc"));
    }

    #[test]
    fn parse_tenant_returns_none_without_seed() {
        let html = r#"<html><script src="/foo/bar/baz/qux"></script></html>"#;
        assert!(parse_tenant_from_html(html).is_none());
    }

    #[test]
    fn parse_tenant_skips_akam13_path_for_sensor() {
        // Only /akam/13/... present; no deep obfuscated path → no
        // sensor_post_path → None.
        let html = r#"<html bazadebezolkohpepadr="123"><script src="/akam/13/abc"></script></html>"#;
        assert!(parse_tenant_from_html(html).is_none());
    }

    #[test]
    fn parse_tenant_skips_short_paths() {
        // 3-segment path is too short to be an Akamai sensor POST path.
        let html = r#"<html bazadebezolkohpepadr="42">
<script src="/a/b/c"></script>
<script src="/x/y/z/w/v/u/t"></script>
</html>"#;
        let t = parse_tenant_from_html(html).expect("parsed");
        assert_eq!(t.sensor_post_path, "/x/y/z/w/v/u/t");
    }

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
