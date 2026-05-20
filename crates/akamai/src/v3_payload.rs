//! Akamai BMP v3 cleartext payload — JSON object schema.
//!
//! Our existing `build_cleartext` produces the v2 DalphanDev 58-element
//! CSV (`-100,-105,...`). Akamai's v3 edge expects a JSON object with
//! ~30 named keys. The edge decrypts the envelope body, runs
//! `JSON.parse(decoded)`, and scores the parsed fields. If `JSON.parse`
//! fails — which is our current behavior — the response is 201 with
//! `_abck` unchanged (envelope shape accepted, payload structurally
//! invalid).
//!
//! This module ships the v3 JSON shape so that, paired with the
//! `bm_sz`-derived shuffle/substitute seeds (W2.3) and the
//! `parse_tenant_from_html` discovery (W2.2), our POST body will
//! decrypt cleanly to a JSON-parseable value. Final scoring then
//! depends on the per-field `wsl` canaries (the 10 plugin / heap /
//! voice probes Akamai uses for "is this a real browser?").
//!
//! See `wsl` slot semantics below — the highest scoring vector.

use crate::AkamaiSession;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use stealth::StealthProfile;

/// Build the v3 cleartext JSON payload that will be substituted-then-
/// shuffled into the envelope body. Returns a `String` of the
/// serialized JSON ready to feed to `build_v2_bestbuy` (which will
/// scramble it).
///
/// `session` carries the accumulated per-page state (mouse/key/touch
/// buffers, counters, bm_sz cookie value, etc.). `profile` provides
/// the static per-stealth-profile fingerprint values
/// (UA, languages, screen, plugins, etc.). `page_url` is the URL
/// our engine is on (matches `pur` field, fragment stripped).
pub fn build_cleartext_v3_json(
    profile: &StealthProfile,
    session: &AkamaiSession,
    page_url: &str,
) -> String {
    let payload = V3Payload::from_state(profile, session, page_url);
    serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string())
}

/// The 30-key v3 cleartext. Keys order-stable per insertion to match
/// the real Chrome 147 capture in
/// `glizzykingdreko/akamai-v3-sensor-data-helper/src/test.js`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V3Payload {
    pub ver: String,
    pub fpt: String,
    pub fpc: String,
    pub ajr: String,
    pub din: Vec<Value>,
    pub eem: String,
    pub ffs: String,
    pub vev: String,
    pub inf: String,
    pub ajt: String,
    pub kev: String,
    pub dme: String,
    pub mev: String,
    pub doe: String,
    pub pur: String,
    pub pev: String,
    pub mst: Vec<Value>,
    pub o9: i64,
    pub tev: String,
    pub sde: String,
    pub pmo: String,
    pub dpw: String,
    pub pac: String,
    pub per: String,
    pub dsi: Vec<Value>,
    pub wsl: String,
    pub hls: String,
    pub pde: String,
    pub oev: String,
    #[serde(rename = "if")]
    pub if_field: String,
    pub fwd: Vec<Value>,
}

impl V3Payload {
    /// Construct a v3 payload from current session + profile + URL.
    /// All fields populated to plausible values; empty strings where
    /// the real payload often has them empty (`ffs`, `inf`, `pev`,
    /// `tev`, `pmo`, `dpw`, `pac`, `pde`, `oev`, `if`).
    pub fn from_state(profile: &StealthProfile, session: &AkamaiSession, page_url: &str) -> Self {
        // `ver` should be SHA-256(bmak.js content) base64. Without a
        // captured bmak.js extraction (W2 patch #2, separate effort) we
        // emit a static placeholder — Akamai's edge cross-checks this
        // against envelope field 6, which is also static for now.
        // When the fileHash extraction lands, both should populate from
        // the same source.
        let ver = "wS5KmeE4vP5vBcKRIM2pPQlq4qZivf0B53dgMqmUH4E=".to_string();

        // `fpt` — fingerprint tuple. Static slots from profile.
        //   `;<-1>;<plugins-tag>;<sessionStorage>;<localStorage>;<idb>;<tz>;
        //    <rtc>;<colorDepth>;<pixelDepth>;<cookieEnabled>;<javaEnabled>;<dnt>`
        let fpt = format!(
            ";-1;dis;,7;true;true;true;{};true;{};{};true;false;-1",
            -(chrono_utc_offset_minutes_or(420)),
            profile_color_depth(profile),
            profile_color_depth(profile),
        );

        // `fpc` — fingerprint count. Static placeholder.
        let fpc = "4488".to_string();

        // `ajr` — comma-list of UA + screen tuple + Gecko flags.
        let ajr = build_ajr(profile);

        // `din` — device info array of `{key:val}` objects.
        let din = build_din(profile);

        // `eem` — event listener registry. Static minimal: document/dm/touch.
        let eem = "do_en,dm_en,t_en".to_string();

        // Session-derived empties / streams.
        let ffs = String::new();
        let vev = String::new(); // visibilitychange events — empty for first POST
        let inf = String::new();
        let ajt = format!(
            "{},{}",
            session.scroll_count, // approximate xhr_count
            session.scroll_count, // approximate fetch_count
        );
        let kev = build_kev(session);
        let dme = String::new(); // DOM mutation buffer — wire up MutationObserver tap (TODO)
        let mev = build_mev(session);
        let doe = "0,1000,-1,-1,-1;".to_string(); // DOMContentLoaded ~1s after start
        let pur = page_url.split('#').next().unwrap_or(page_url).to_string();
        let pev = String::new();

        // `mst` — master state counters.
        let mst = build_mst(session);

        let o9 = 0i64;
        let tev = String::new();
        let sde = "0,0,0,0,1,0,0".to_string();
        let pmo = String::new();
        let dpw = String::new();
        let pac = String::new();
        let per = generate_per();
        let dsi = build_dsi(profile, session);

        // `wsl` — TOP scoring vector. Real Chrome canaries:
        //   wsl[0] performance.memory.jsHeapSizeLimit (~4 GB)
        //   wsl[1] totalJSHeapSize (30-50 MB)
        //   wsl[2] usedJSHeapSize (10-25 MB)
        //   wsl[3] navigator.connection.rtt (50-300)
        //   wsl[4] speechSynthesis.getVoices().length (~8 voices)
        //   wsl[5] !!navigator.plugins[0][0].enabledPlugin (1)
        //   wsl[6] !!navigator.plugins.refresh (1)
        //   wsl[7] !!navigator.plugins.item(4294967296) (1 — UINT32 wrap)
        //   wsl[8] !!File.prototype.path (0 — Electron-only)
        //   wsl[9] !!SharedArrayBuffer (1 in COI Chrome)
        let wsl = format!(
            "{},{},{},{},{},1,1,1,0,1,,,,,,0,,,1,1",
            4_294_705_152u64, // jsHeapSizeLimit ≈ 4 GB
            42_000_000u32,    // totalJSHeapSize ≈ 42 MB
            18_000_000u32,    // usedJSHeapSize ≈ 18 MB
            100u32,           // connection.rtt 100ms
            8u32,             // 8 voices typical Chrome desktop
        );

        let hls = "-1,,,1,1".to_string();
        let pde = String::new();
        let oev = String::new();
        let if_field = String::new();
        let fwd = build_fwd();

        V3Payload {
            ver,
            fpt,
            fpc,
            ajr,
            din,
            eem,
            ffs,
            vev,
            inf,
            ajt,
            kev,
            dme,
            mev,
            doe,
            pur,
            pev,
            mst,
            o9,
            tev,
            sde,
            pmo,
            dpw,
            pac,
            per,
            dsi,
            wsl,
            hls,
            pde,
            oev,
            if_field,
            fwd,
        }
    }
}

fn chrono_utc_offset_minutes_or(default: i32) -> i32 {
    // Without pulling in chrono just for this, return a plausible
    // offset that matches our typical profile. -420 = UTC-7 (PDT).
    // Real implementation should derive from profile.timezone.
    default
}

fn profile_color_depth(_profile: &StealthProfile) -> u32 {
    24
}

fn build_ajr(profile: &StealthProfile) -> String {
    // `<UA>,<inner_w>,<inner_h>,<outer_w>,<outer_h>,<screen_w>,<screen_h>,
    //  <screen_avail_w>,<screen_avail_h>,<dpr>,cpen:0,i1:0,dm:0,cwen:0,non:1,
    //  opc:0,fc:0,sc:0,wrc:1,isc:0,vib:1,bat:1,x11:0,x12:1`
    format!(
        "{},1280,720,1280,720,1920,1080,1920,1040,1,cpen:0,i1:0,dm:0,cwen:0,non:1,opc:0,fc:0,sc:0,wrc:1,isc:0,vib:1,bat:1,x11:0,x12:1",
        profile.user_agent
    )
}

fn build_din(profile: &StealthProfile) -> Vec<Value> {
    vec![
        json!({"nal": "en-US"}),
        json!({"nps": "20030107"}),
        json!({"ucs": 8753}),
        json!({"she": 1080}),
        json!({"tsd": 0}),
        json!({"ran": format!("{}", rand_decimal_under_1())}),
        json!({"xag": 12147}),
        json!({"ibr": 0}),
        json!({"ua": profile.user_agent.clone()}),
        json!({"swi": 1280}),
        json!({"dau": 0}),
        json!({"asw": 1280}),
        json!({"wdr": 0}),
        json!({"wow": 1280}),
        json!({"pha": 0}),
        json!({"hal": 871_866_924_183i64}),
        json!({"hz1": 429_041}),
        json!({"wih": 720}),
        json!({"wiw": 1280}),
        json!({"ash": 1040}),
        json!({"nap": "Gecko"}),
        json!({"adp": "cpen:0,i1:0,dm:0,cwen:0,non:1,opc:0,fc:0,sc:0,wrc:1,isc:0,vib:1,bat:1,x11:0,x12:1"}),
        json!({"npl": 5}),
    ]
}

fn build_kev(_session: &AkamaiSession) -> String {
    String::new()
}

fn build_mev(session: &AkamaiSession) -> String {
    let mut out = String::new();
    for (i, ev) in session.mouse_buf.iter().enumerate().take(100) {
        let kind = match ev.kind {
            0 => 1, // mousemove → mev kind=1
            1 => 4, // mousedown → mev kind=4
            2 => 3, // mouseup → mev kind=3 (close enough for now)
            _ => 1,
        };
        out.push_str(&format!("{},{},{},{},{};", i, kind, ev.t, ev.x, ev.y));
    }
    out
}

fn build_mst(session: &AkamaiSession) -> Vec<Value> {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(1_743_733_848_366);
    vec![
        json!({"kevl": 1}),
        json!({"mevl": 21_193_143}),
        json!({"tevl": 32}),
        json!({"devl": 45}),
        json!({"dmvl": 5_526_574}),
        json!({"pevl": 0}),
        json!({"tovl": 26_719_730}),
        json!({"delt": 5_526_529}),
        json!({"it": 0}),
        json!({"sts": now_ms}),
        json!({"fct": 1}),
        json!({"dd2": 18_653}),
        json!({"kc": session.key_count}),
        json!({"mc": session.mouse_count}),
        json!({"ww8": 3108}),
        json!({"pc": 2}),
        json!({"tc": session.touch_count}),
        json!({"ssts": 5_526_533}),
        json!({"tst": 26_665_496}),
        json!({"rval": "-1"}),
        json!({"rcfp": "-1"}),
        json!({"nfas": 30_261_693}),
        json!({"jsrf": "PiZtE"}),
        json!({"jsrf1": 39_064}),
        json!({"jsrf2": 80}),
        json!({"signals": "0"}),
        json!({"mwd": "0"}),
        json!({"hea": ""}),
        json!({"dvc": "93h9dhdYdh9iYeveufko,13,f+b+l+g+i+j+e+k+c+"}),
        json!({"srd": "0"}),
    ]
}

fn build_dsi(_profile: &StealthProfile, _session: &AkamaiSession) -> Vec<Value> {
    vec![
        json!({"get": ""}),
        json!({"set": "0"}),
        json!({"ico": "070f409b82df3bdd2f51a6415c7895353c153c47fe6dd8a0f87f3d14c46ccb2b"}),
        json!({"ift": "3"}),
        json!({"xof": "8,5,1,1,8"}),
        json!({"xot": "8,5,1,1,8"}),
        json!({"wev": "NA;wev;NA"}),
        json!({"wre": "NA;wre;NA"}),
        json!({"wdr": "0"}),
        json!({"iks": ""}),
        json!({"lds": "1"}),
        json!({"sst": ""}),
    ]
}

fn build_fwd() -> Vec<Value> {
    vec![
        json!({"fmh": ""}),
        json!({"fmz": "2"}),
        json!({"ssh": "6d9faae3a85b2727ec5b802ee76b75a2c8a736774ef9c0024c9b875de06f1fb0"}),
    ]
}

fn generate_per() -> String {
    // 20-digit pseudo-random number string per session.
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..20)
        .map(|_| char::from(b'0' + rng.gen_range(0..10)))
        .collect()
}

fn rand_decimal_under_1() -> f64 {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    rng.gen_range(0.0..1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_profile() -> StealthProfile {
        stealth::presets::chrome_130_macos()
    }

    #[test]
    fn v3_payload_serializes_to_valid_json() {
        let profile = dummy_profile();
        let session = AkamaiSession::default();
        let serialized =
            build_cleartext_v3_json(&profile, &session, "https://www.example.com/path");
        // Must parse back as JSON object.
        let parsed: Value = serde_json::from_str(&serialized).expect("parses as JSON");
        assert!(parsed.is_object(), "v3 cleartext must be a JSON object");
        // All 30 keys present.
        let obj = parsed.as_object().unwrap();
        for key in [
            "ver", "fpt", "fpc", "ajr", "din", "eem", "ffs", "vev", "inf", "ajt", "kev", "dme",
            "mev", "doe", "pur", "pev", "mst", "o9", "tev", "sde", "pmo", "dpw", "pac", "per",
            "dsi", "wsl", "hls", "pde", "oev", "if", "fwd",
        ] {
            assert!(obj.contains_key(key), "missing v3 key: {key}");
        }
    }

    #[test]
    fn v3_per_is_20_digits() {
        let profile = dummy_profile();
        let session = AkamaiSession::default();
        let s = build_cleartext_v3_json(&profile, &session, "/");
        let v: Value = serde_json::from_str(&s).unwrap();
        let per = v["per"].as_str().unwrap();
        assert_eq!(per.len(), 20, "per must be 20 digits, got {per}");
        assert!(
            per.chars().all(|c| c.is_ascii_digit()),
            "per must be all digits"
        );
    }

    #[test]
    fn v3_wsl_canaries_match_real_chrome_shape() {
        let profile = dummy_profile();
        let session = AkamaiSession::default();
        let s = build_cleartext_v3_json(&profile, &session, "/");
        let v: Value = serde_json::from_str(&s).unwrap();
        let wsl = v["wsl"].as_str().unwrap();
        let parts: Vec<&str> = wsl.split(',').collect();
        // Real-Chrome shape: 20 comma-separated slots with specific canaries
        // at positions 5-9 — the bot-detection canaries — must be exactly:
        //   wsl[5..=7] = 1 (plugin probes pass)
        //   wsl[8]     = 0 (File.prototype.path absent — Electron-only)
        //   wsl[9]     = 1 (SharedArrayBuffer present)
        assert!(
            parts.len() >= 10,
            "wsl needs ≥10 slots, got {}",
            parts.len()
        );
        assert_eq!(
            parts[5], "1",
            "wsl[5] plugins[0][0].enabledPlugin must be 1"
        );
        assert_eq!(parts[6], "1", "wsl[6] plugins.refresh must be 1");
        assert_eq!(parts[7], "1", "wsl[7] plugins.item(UINT32) must be 1");
        assert_eq!(parts[8], "0", "wsl[8] File.prototype.path must be 0");
        assert_eq!(parts[9], "1", "wsl[9] SharedArrayBuffer must be 1");
    }

    #[test]
    fn v3_pur_strips_fragment() {
        let profile = dummy_profile();
        let session = AkamaiSession::default();
        let s =
            build_cleartext_v3_json(&profile, &session, "https://www.example.com/path#fragment");
        let v: Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["pur"].as_str().unwrap(), "https://www.example.com/path");
    }

    #[test]
    fn v3_din_array_has_24_entries() {
        let profile = dummy_profile();
        let session = AkamaiSession::default();
        let s = build_cleartext_v3_json(&profile, &session, "/");
        let v: Value = serde_json::from_str(&s).unwrap();
        let din = v["din"].as_array().unwrap();
        // Real captures show 23 entries in the din device-info array
        // (nal, nps, ucs, she, tsd, ran, xag, ibr, ua, swi, dau, asw,
        // wdr, wow, pha, hal, hz1, wih, wiw, ash, nap, adp, npl).
        assert_eq!(din.len(), 23, "din array must have 23 entries");
    }
}
