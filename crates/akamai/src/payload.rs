//! Akamai sensor_data v2 cleartext field assembler.
//!
//! Builds the 58-element `tAD` array that becomes the input to
//! `crypto::shuffle_tokens` + `crypto::substitute_chars`. The array
//! is 29 (marker, data) pairs — one pair per fingerprint slot.
//!
//! Field structure ported from DalphanDev/akamai-sensor `gen_tAD`,
//! itself a manual deobfuscation of Akamai's `bd-l-loader` script.
//! Markers (`-100`, `-105`, `-108`, …) are stable across deployments;
//! data values are mostly stable but a few (mouse trajectory, _abck
//! echo, page URL, request counter) vary per session.
//!
//! ## Field summary
//!
//! | Marker | Var name | Content | Source in oxide |
//! |---|---|---|---|
//! | -100 | vAD | UA + gecko + lang + screen + tz + flags | `StealthProfile` |
//! | -105 | n8D | counters `-1,-1,0,0,-1,-1,0;` | static |
//! | -108 | fdD | empty | static |
//! | -101 | wAD | event listeners `do_en,dm_en,t_en` | static |
//! | -110 | zdD | mouse trajectory `i,1,t,x,y;…` | `AkamaiSession.mouse_buf` |
//! | -117 | WdD | empty | static |
//! | -109 | Z8D | timing `0,t,-1,-1,-1,-1,-1,-1,-1,-1,-1;` | timer |
//! | -102 | QAD | counters `-1,-1,0,0,-1,-1,0;` | static |
//! | -111 | l8D | timing `0,t,-1,-1,-1;` | timer |
//! | -114 | kdD | empty | static |
//! | -103 | X8D | varies (per-session string) | static placeholder |
//! | -106 | NAD | `10,2` | static |
//! | -115 | VRD | _abck echo + per-session counters | session |
//! | -112 | cAD | page URL | session |
//! | -119 | j8D | `-1` | static |
//! | -122 | TRD | `0,0,0,0,1,0,0` | static |
//! | -123 | CRD | empty | static |
//! | -124 | hRD | empty | static |
//! | -126 | IRD | empty | static |
//! | -127 | g8D | random digit string | per-session |
//! | -128 | NRD | `,1,<sha256-hex>` | per-session |
//! | -131 | A8D | integer tuple | static |
//! | -132 | P8D | `-1` | static |
//! | -133 | U8D | empty | static |
//! | -70  | BdD.fpValStr | tz + hardwareConcurrency + plugins flags | `StealthProfile` |
//! | -80  | XRD | `4961` | static |
//! | -90  | wdD | `1,4588905,3,1529635,5,917781\|4588905` | static |
//! | -116 | Y8D | `0` | static |
//! | -129 | LRD | canvas-FP-hex + WebGL-renderer-hex | `StealthProfile` |
//!
//! ## Status
//!
//! T3A-A3: structural completeness. Static-placeholder fields are
//! ported verbatim from DalphanDev's reference (bestbuy may demand
//! variant values; A6 verifies). Dynamic fields wire to
//! `StealthProfile` and `AkamaiSession`. The mouse-trajectory format
//! is shipped here but the buffer is filled in A4.

use crate::session::AkamaiSession;
use crate::MouseEvent;
use rand::Rng;
use stealth::StealthProfile;

/// Build the 58-element tAD array as a `,`-joined cleartext string.
///
/// `request_url` is the URL of the page that triggered the
/// sensor_data POST (field -112 / cAD).
pub fn build_cleartext(
    profile: &StealthProfile,
    session: &AkamaiSession,
    request_url: &str,
) -> String {
    let mut tad: Vec<String> = Vec::with_capacity(58);

    // -100 / vAD — primary device-data blob
    tad.push("-100".into());
    tad.push(field_vad(profile));

    // -105 / n8D
    tad.push("-105".into());
    tad.push("-1,-1,0,0,-1,-1,0;".into());

    // -108 / fdD
    tad.push("-108".into());
    tad.push("".into());

    // -101 / wAD
    tad.push("-101".into());
    tad.push("do_en,dm_en,t_en".into());

    // -110 / zdD — mouse trajectory
    tad.push("-110".into());
    tad.push(field_mouse_trajectory(&session.mouse_buf));

    // -117 / WdD
    tad.push("-117".into());
    tad.push("".into());

    // -109 / Z8D
    tad.push("-109".into());
    tad.push(field_z8d(session));

    // -102 / QAD
    tad.push("-102".into());
    tad.push("-1,-1,0,0,-1,-1,0;".into());

    // -111 / l8D
    tad.push("-111".into());
    tad.push(field_l8d(session));

    // -114 / kdD
    tad.push("-114".into());
    tad.push("".into());

    // -103 / X8D — opaque per-session string (placeholder)
    tad.push("-103".into());
    tad.push("".into());

    // -106 / NAD
    tad.push("-106".into());
    tad.push("10,2".into());

    // -115 / VRD — _abck echo + per-session counters
    tad.push("-115".into());
    tad.push(field_vrd(session));

    // -112 / cAD — page URL (without fragment)
    tad.push("-112".into());
    let url_str = if let Ok(mut u) = url::Url::parse(request_url) {
        u.set_fragment(None);
        u.to_string()
    } else {
        request_url.to_string()
    };
    tad.push(url_str);

    // -119 / j8D
    tad.push("-119".into());
    tad.push("-1".into());

    // -122 / TRD
    tad.push("-122".into());
    tad.push("0,0,0,0,1,0,0".into());

    // -123 / CRD
    tad.push("-123".into());
    tad.push("".into());

    // -124 / hRD
    tad.push("-124".into());
    tad.push("".into());

    // -126 / IRD
    tad.push("-126".into());
    tad.push("".into());

    // -127 / g8D — random digit string
    tad.push("-127".into());
    let mut g8d = String::new();
    let mut rng = rand::thread_rng();
    use rand::Rng;
    for _ in 0..20 {
        g8d.push_str(&rng.gen_range(0..10).to_string());
    }
    tad.push(g8d);

    // -128 / NRD
    tad.push("-128".into());
    tad.push(field_nrd(session));

    // -131 / A8D
    tad.push("-131".into());
    tad.push("4294705152,131011631,123954963,50,22,1,1,1,0,1".into());

    // -132 / P8D
    tad.push("-132".into());
    tad.push("-1".into());

    // -133 / U8D
    tad.push("-133".into());
    tad.push("".into());

    // -70 / BdD.fpValStr — fingerprint values
    tad.push("-70".into());
    tad.push(field_fp_val_str(profile));

    // -80 / XRD
    tad.push("-80".into());
    tad.push("4961".into());

    // -90 / wdD
    tad.push("-90".into());
    tad.push("1,4588905,3,1529635,5,917781|4588905".into());

    // -116 / Y8D
    tad.push("-116".into());
    tad.push("0".into());

    // -129 / LRD — canvas + WebGL hashes
    tad.push("-129".into());
    tad.push(field_lrd(profile));

    debug_assert_eq!(tad.len(), 58, "tAD must be 58 elements (29 marker/data pairs)");
    tad.join(",")
}

/// Field -100 / vAD: UA + product + sub + locale + screen + flags.
/// Real captures vary in last segment ("loc:" suffix is empty if no
/// page interaction yet).
fn field_vad(p: &StealthProfile) -> String {
    let plugins_flag = if p.pdf_viewer_enabled { 1 } else { 0 };
    format!(
        "{ua},uaend,12147,20030107,{lang},Gecko,5,0,0,0,415091,0,{sw},{sah},{sw2},{sh},533,969,{sw3},,cpen:0,i1:0,dm:0,cwen:0,non:1,opc:0,fc:0,sc:0,wrc:1,isc:0,vib:1,bat:1,x11:0,x12:1,8103,0.18373239291,843518179527.5,{plugins},loc:",
        ua = p.user_agent,
        lang = p.language,
        sw = p.screen_width,
        sah = p.screen_avail_height,
        sw2 = p.screen_width,
        sh = p.screen_height,
        sw3 = p.screen_width,
        plugins = plugins_flag,
    )
}

/// Field -110 / zdD: mouse trajectory, semicolon-delimited tuples
/// `<i>,1,<t_ms>,<x>,<y>;` per event. Empty trajectory → empty string.
fn field_mouse_trajectory(events: &[MouseEvent]) -> String {
    let mut s = String::new();
    for (i, e) in events.iter().enumerate() {
        s.push_str(&format!("{i},1,{},{},{};", e.t, e.x, e.y));
    }
    s
}

/// Field -109 / Z8D: page-load timing tuple
/// `0,<load_t>,-1,-1,-1,-1,-1,-1,-1,-1,-1;`. Currently a static
/// placeholder; A4 will wire the real timer.
fn field_z8d(_s: &AkamaiSession) -> String {
    let load_t = rand::thread_rng().gen_range(200000..500000);
    format!("0,{load_t},-1,-1,-1,-1,-1,-1,-1,-1,-1;")
}

/// Field -111 / l8D: page-load timing tuple
/// `0,<load_t>,-1,-1,-1;`. Static placeholder pending A4.
fn field_l8d(_s: &AkamaiSession) -> String {
    let load_t = rand::thread_rng().gen_range(200000..500000);
    format!("0,{load_t},-1,-1,-1;")
}

/// Field -115 / VRD: _abck echo + per-session counters. Real captures
/// embed the server-issued _abck cookie and a barrel of counters; for
/// our cold-start case we ship a stub matching the DalphanDev shape
/// with our own session's _abck if known.
fn field_vrd(s: &AkamaiSession) -> String {
    let abck_echo = s.last_abck.as_deref().unwrap_or("");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let load_t = rand::thread_rng().gen_range(200000..500000);
    format!(
        "1,4016774,32,{load_t},{load_t},0,4588905,23468869,0,{now},7,18047,0,14,3007,0,0,23468870,4577207,0,{}",
        abck_echo
    )
}

/// Field -128 / NRD: `,1,<sha256-hex>`. Real captures embed a
/// session-specific SHA-256 over [unknown input]. Static placeholder
/// for now.
fn field_nrd(_s: &AkamaiSession) -> String {
    ",1,e79ca373a5d6562e4be163953f8f9a57367140e11d550d1767780b9dd547f5bf".into()
}

/// Field -70 / BdD.fpValStr: fingerprint flags. Each segment encodes
/// a navigator/device property: timezone offset, hardware concurrency,
/// touch points, color depth, plus boolean flags.
fn field_fp_val_str(p: &StealthProfile) -> String {
    // Real capture format: `420217769;-1;dis;,7;true;true;true;240;true;24;24;true;false;-1`.
    // Without per-deployment field reverse-engineering we ship a
    // shape-correct value with our profile's color depth / cores.
    let cd = p.screen_color_depth;
    format!(
        "420217769;-1;dis;,{cores};true;true;true;240;true;{cd};{cd};true;false;-1",
        cores = p.cpu_cores,
    )
}

/// Field -129 / LRD: canvas-FP hex + WebGL renderer + audio FP +
/// shading_language hash. We derive deterministic hashes from
/// canvas_seed / audio_seed for the canvas / audio components, and
/// embed StealthProfile.gpu_profile fields directly.
fn field_lrd(p: &StealthProfile) -> String {
    let canvas_hash = hex64(p.canvas_seed);
    let audio_hash = hex64(p.audio_seed);
    let shader_hash = hex64(p.canvas_seed.wrapping_mul(0x9e37_79b9));
    let renderer = &p.gpu_profile.unmasked_renderer;
    let unmasked_vendor = &p.gpu_profile.unmasked_vendor;
    format!(
        "{canvas},1,{shader},{vendor},{renderer},{audio},29",
        canvas = canvas_hash,
        shader = shader_hash,
        vendor = unmasked_vendor,
        renderer = renderer,
        audio = audio_hash,
    )
}

/// Format a u64 as a 64-char hex string (left-pad with zeros, repeat
/// the seed twice). Used to produce 64-char hash placeholders for
/// canvas / audio / shader fingerprints — same shape as a real
/// SHA-256 digest in hex.
fn hex64(seed: u64) -> String {
    format!("{:016x}{:016x}{:016x}{:016x}", seed, !seed, seed.wrapping_mul(31), !seed.wrapping_mul(31))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_profile() -> StealthProfile {
        stealth::presets::chrome_130_macos()
    }

    #[test]
    fn cleartext_has_58_elements() {
        let p = synthetic_profile();
        let s = AkamaiSession::default();
        let ct = build_cleartext(&p, &s, "https://www.bestbuy.com/");
        let n = ct.split(',').count();
        // 58 array slots, each separated by ','. The vAD field itself
        // contains commas internally, so the split count is HIGHER
        // than 58. We just sanity-check the count is meaningful.
        assert!(n >= 58, "split count {n} should be ≥ 58 (extra commas inside fields)");
    }

    #[test]
    fn cleartext_includes_url_and_ua() {
        let p = synthetic_profile();
        let s = AkamaiSession::default();
        let ct = build_cleartext(&p, &s, "https://www.bestbuy.com/");
        assert!(ct.contains("https://www.bestbuy.com/"));
        assert!(ct.contains(&p.user_agent[..40])); // partial UA contained
    }

    #[test]
    fn cleartext_starts_with_minus_100_marker() {
        let p = synthetic_profile();
        let s = AkamaiSession::default();
        let ct = build_cleartext(&p, &s, "https://www.bestbuy.com/?intl=nosplash");
        println!("CLEARTEXT_START:{}CLEARTEXT_END", ct);
        assert!(
            ct.starts_with("-100,"),
            "cleartext must start with the -100 marker; got: {}",
            &ct[..50]
        );
    }

    #[test]
    fn mouse_trajectory_emits_per_event_tuples() {
        let events = vec![
            MouseEvent { x: 100, y: 200, t: 1000, kind: 0, button: 0 },
            MouseEvent { x: 110, y: 205, t: 1010, kind: 0, button: 0 },
        ];
        let s = field_mouse_trajectory(&events);
        assert_eq!(s, "0,1,1000,100,200;1,1,1010,110,205;");
    }

    #[test]
    fn empty_mouse_trajectory_emits_empty_string() {
        let s = field_mouse_trajectory(&[]);
        assert_eq!(s, "");
    }

    #[test]
    fn lrd_includes_renderer_and_seed_hashes() {
        let p = synthetic_profile();
        let s = field_lrd(&p);
        assert!(s.contains(&p.gpu_profile.unmasked_renderer));
        assert!(s.contains(&p.gpu_profile.unmasked_vendor));
        // hex64 seed produces 64 chars
        let parts: Vec<&str> = s.split(',').collect();
        assert_eq!(parts[0].len(), 64, "canvas hash is 64 hex chars");
    }

    #[test]
    fn vad_contains_screen_dims() {
        let p = synthetic_profile();
        let s = field_vad(&p);
        assert!(s.contains(&format!("{},{}", p.screen_width, p.screen_avail_height)));
    }
}
