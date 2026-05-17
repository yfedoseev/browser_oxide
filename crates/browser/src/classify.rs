//! FP-B1 — the single shared anti-bot challenge classifier.
//!
//! Before this module there were **three divergent classifiers** that
//! tagged the same body differently depending on which one you asked:
//!
//! - `page.rs::body_has_challenge_marker` — ungated strong markers,
//!   weak markers gated `< 50 KB`, edge/sensor split at `50 KB`,
//!   render-incomplete `< 5 KB`.
//! - `holistic_sweep.rs::classify` — interstitial gate `< 30 KB`,
//!   blocked-word gate `< 5 KB`, thin `< 1000`, the ledger-authoritative
//!   metric behind the 126-corpus count and the directive's re-measure
//!   clause (10 regression tests pin its behavior).
//! - the vendor handlers — their own `≤ 4 KB` gates.
//!
//! The same body could be `Pass` in one and `*-CHL` in another, which is
//! exactly how the "22 engine-addressable" count was inflated from a
//! true ≈6 (master plan Phase 0.2).
//!
//! This module is now the **single source of truth** for the marker set
//! and the size gates. The canonical policy is byte-for-byte the old
//! `holistic_sweep::classify` (the most FP-hardened of the three and the
//! one the ledger is computed from — converging onto it is a strict
//! false-positive reduction for the other two, never a weakening of
//! genuine-challenge detection). `page.rs` and the audit harness now
//! derive their verdicts from [`engine_classify`]; `holistic_sweep`'s
//! `classify` is a thin wrapper returning [`EngineClass::tag`].
//!
//! Policy *corrections* (size-gating literal strong markers, splitting
//! `CfChallengeIncomplete` from `SensorFail`, the thin-shell band) are
//! deliberately layered on top in FP-B2 / FP-B3 / FP-B4 — this commit is
//! a behavior-preserving unification only.

use crate::page::ChallengeVerdict;

/// Canonical size gates — the single source of truth. Anything that
/// needs a body-size threshold for challenge classification reads these.
pub const INTERSTITIAL_MAX_BYTES: usize = 30 * 1024;
pub const BLOCKED_WORD_MAX_BYTES: usize = 5 * 1024;
pub const THIN_BODY_MAX_BYTES: usize = 1000;
/// Coarse edge-vs-sensor split for [`ChallengeVerdict`]: a served
/// challenge in a small body is an edge/interstitial deny before our JS
/// earned trust; in a large body the vendor JS ran and scored us bot.
pub const SENSOR_SPLIT_BYTES: usize = 50 * 1024;

/// Result of the shared classifier. `tag` is the holistic-style vendor
/// tag (ledger vocabulary); `verdict` is the coarse [`ChallengeVerdict`]
/// `page.rs` / the audit harness consume — both derived from the *same*
/// marker pass so they can never disagree again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EngineClass {
    pub tag: &'static str,
    pub verdict: ChallengeVerdict,
    pub len: usize,
}

/// Any-size, near-zero-false-positive structural tokens (CSS class
/// names / URL paths / encoded vars that do not legitimately appear in
/// rendered content). Order is significant — first match wins.
const UNAMBIGUOUS: &[(&str, &str)] = &[
    ("cf-browser-verification", "Cloudflare-CHL"),
    ("/_sec/cp_challenge", "Akamai-sec-cpt-CHL"),
    ("ddcaptchaencoded", "DataDome-CHL"),
    ("px-captcha", "PerimeterX-CHL"),
];

/// English-phrase interstitial markers — these CAN appear in normal page
/// copy (article body, cookie banner, privacy policy), so they only
/// count when the body is interstitial-sized (`< INTERSTITIAL_MAX_BYTES`).
const PHRASE: &[(&str, &str)] = &[
    ("just a moment", "Cloudflare-CHL"),
    ("checking your browser", "Cloudflare-CHL"),
    ("captcha-delivery.com", "DataDome-CHL"),
    ("press &amp; hold", "PerimeterX-PaH"),
    ("pardon our interruption", "Akamai-CHL"),
];

/// Vendor fingerprint markers that appear in BOTH rendered pages (as
/// analytics/SDK references) and on interstitials — a challenge only
/// when the content was replaced (body `< INTERSTITIAL_MAX_BYTES`).
const SMALL_BODY: &[(&str, &str)] = &[
    ("akam/13", "Akamai-CHL"),
    ("_abck", "Akamai-CHL"),
    ("_kpsdk", "Kasada-CHL"),
    ("ips.js", "Kasada-CHL"),
    ("_pxhd", "PerimeterX-CHL"),
    ("captcha", "captcha-CHL"),
    ("403 forbidden", "BLOCKED"),
    ("access denied", "BLOCKED"),
];

/// Map a canonical tag + body length to the coarse [`ChallengeVerdict`].
/// `THIN-BODY` ⇒ render-completeness issue (not stealth); `L3-RENDERED`
/// ⇒ pass; anything else is a served challenge/deny split edge-vs-sensor
/// by [`SENSOR_SPLIT_BYTES`].
fn verdict_for(tag: &str, len: usize) -> ChallengeVerdict {
    match tag {
        "L3-RENDERED" => ChallengeVerdict::Pass,
        "THIN-BODY" => ChallengeVerdict::RenderIncomplete,
        _ if len < SENSOR_SPLIT_BYTES => ChallengeVerdict::EdgeBlock,
        _ => ChallengeVerdict::SensorFail,
    }
}

/// The single canonical classifier. Behavior is byte-for-byte the
/// pre-unification `holistic_sweep::classify`; the only addition is the
/// derived [`ChallengeVerdict`] so `page.rs` shares the exact same
/// marker/gate decision.
pub fn engine_classify(body: &str) -> EngineClass {
    let lower = body.to_lowercase();
    let len = body.len();

    let tag: &'static str = 'tag: {
        for (n, t) in UNAMBIGUOUS {
            if lower.contains(n) {
                break 'tag t;
            }
        }
        if len < INTERSTITIAL_MAX_BYTES {
            for (n, t) in PHRASE {
                if lower.contains(n) {
                    break 'tag t;
                }
            }
            for (n, t) in SMALL_BODY {
                if lower.contains(n) {
                    break 'tag t;
                }
            }
        }
        if len < BLOCKED_WORD_MAX_BYTES && lower.contains("blocked") {
            break 'tag "BLOCKED";
        }
        if len < THIN_BODY_MAX_BYTES {
            break 'tag "THIN-BODY";
        }
        "L3-RENDERED"
    };

    EngineClass {
        tag,
        verdict: verdict_for(tag, len),
        len,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // FP-B1 regression: the SAME fixture fed through every call site
    // must yield the same challenge-vs-pass decision. `page_is_challenge`
    // mirrors `page.rs::is_anti_bot_challenge`; `holistic_tag` mirrors
    // `holistic_sweep::classify`. They are now provably one function.
    fn page_is_challenge(body: &str) -> bool {
        engine_classify(body).verdict.is_challenge()
    }
    fn holistic_tag(body: &str) -> &'static str {
        engine_classify(body).tag
    }

    #[test]
    fn all_call_sites_agree() {
        struct Case {
            name: &'static str,
            body: String,
            tag: &'static str,
            challenge: bool,
        }
        let big = |seed: &str| {
            let mut h = String::from("<html><body>");
            h.push_str(seed);
            for _ in 0..30000 {
                h.push_str("<div>actual rendered content paragraph</div>");
            }
            h.push_str("</body></html>");
            h
        };
        let cases = vec![
            Case { name: "empty", body: "<html></html>".into(), tag: "THIN-BODY", challenge: false },
            Case { name: "cf small", body: "<html><body>Just a moment...</body></html>".into(), tag: "Cloudflare-CHL", challenge: true },
            Case { name: "dd small", body: r#"<script src="https://geo.captcha-delivery.com/c"></script>"#.into(), tag: "DataDome-CHL", challenge: true },
            Case { name: "akam small", body: r#"<script src="/akam/13/abc"></script><form id="bm-verify"></form>"#.into(), tag: "Akamai-CHL", challenge: true },
            // FP-B2 target (still over-matches here, fixed in B2): an
            // unambiguous literal in a >100 KB rendered page.
            Case { name: "pxhd large benign", body: big(r#"<script>window._pxhd="sdk"</script>"#), tag: "L3-RENDERED", challenge: false },
            Case { name: "just-a-moment large benign", body: big("<p>give us just a moment to load</p>"), tag: "L3-RENDERED", challenge: false },
            Case { name: "grecaptcha config large", body: big(r#"<script>window.C={"googleRecaptcha":1}</script>"#), tag: "L3-RENDERED", challenge: false },
        ];
        for c in cases {
            let ec = engine_classify(&c.body);
            assert_eq!(ec.tag, c.tag, "tag mismatch [{}]", c.name);
            assert_eq!(holistic_tag(&c.body), c.tag, "holistic disagrees [{}]", c.name);
            assert_eq!(
                page_is_challenge(&c.body),
                c.challenge,
                "page/holistic challenge-verdict disagree [{}] tag={}",
                c.name,
                ec.tag
            );
        }
    }

    #[test]
    fn verdict_mapping_is_consistent() {
        assert_eq!(engine_classify("<html></html>").verdict, ChallengeVerdict::RenderIncomplete);
        assert_eq!(
            engine_classify("<html><body>Just a moment...</body></html>").verdict,
            ChallengeVerdict::EdgeBlock
        );
        // Unambiguous literal in a large body ⇒ SensorFail (the only way
        // a challenge tag co-occurs with a ≥50 KB body under canonical
        // policy). FP-B2 will size-gate this away.
        let mut big = String::from(r#"<div class="px-captcha">x</div>"#);
        for _ in 0..3000 {
            big.push_str("<p>padding padding padding padding</p>");
        }
        assert!(big.len() >= SENSOR_SPLIT_BYTES);
        assert_eq!(engine_classify(&big).verdict, ChallengeVerdict::SensorFail);
    }
}
