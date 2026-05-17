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
/// FP-B3: a rendered, challenge-free body below this floor (but above
/// [`THIN_BODY_MAX_BYTES`]) is a thin shell / SPA pre-hydration stub —
/// flagged `ThinShell`, not over-counted as a full `Pass`. Sized to
/// cover the known small "pass" shells (bestbuy ~7.8 KB, spotify
/// ~9.6 KB, duolingo ~13 KB). Only affects the coarse
/// [`ChallengeVerdict`]; the holistic `tag` stays `L3-RENDERED`
/// (≥ 1000 B) so the 126-corpus ledger metric is unchanged.
pub const THIN_SHELL_MAX_BYTES: usize = 15 * 1024;
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

/// Any-size, near-zero-false-positive structural tokens. FP-B2: every
/// entry here must be a token that does NOT legitimately appear in
/// rendered content at any size — a unique URL path, an encoded
/// variable, or a challenge-only CSS hook. `px-captcha` was removed
/// from this table (it is a bare CSS class / cookie-consent manifest
/// key string that occurs verbatim in fully-rendered pages — the
/// historical wayfair false positive) and is now size-gated in
/// [`SMALL_BODY`]. `captcha-delivery.com` is and stays phrase-gated in
/// [`PHRASE`]. The three below are structural URL/var tokens that a
/// real Chrome never sees on a passed page. Order is significant —
/// first match wins. FP-B4 adds `_cf_chl_opt` — the inline Cloudflare
/// challenge-options JS object that is present *only* on the CF
/// challenge / Managed-Challenge orchestrator shell and is gone once
/// the challenge clears and real content is served (so it is the
/// any-size structural signal that catches the large udemy CF shell
/// `/cdn-cgi/challenge-platform/` alone cannot — that JSD URL also
/// stays on *passed* CF pages, so it is deliberately NOT used here).
const UNAMBIGUOUS: &[(&str, &str)] = &[
    ("cf-browser-verification", "Cloudflare-CHL"),
    ("_cf_chl_opt", "Cloudflare-CHL"),
    ("/_sec/cp_challenge", "Akamai-sec-cpt-CHL"),
    ("ddcaptchaencoded", "DataDome-CHL"),
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
    // FP-B2: `px-captcha` relocated here from UNAMBIGUOUS. It is a real
    // PerimeterX interstitial hook, but those interstitials are small —
    // gating it `< INTERSTITIAL_MAX_BYTES` keeps true detection while
    // killing the wayfair-class FP (literal `px-captcha` in a multi-MB
    // rendered page's CSS / cookie-consent manifest). MUST precede the
    // bare `captcha` row so PerimeterX attribution wins over captcha-CHL.
    ("px-captcha", "PerimeterX-CHL"),
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
        // FP-B3: rendered + challenge-free but below the content floor
        // ⇒ ThinShell (a small shell, not a full win). The holistic
        // `tag` is still "L3-RENDERED" (ledger unchanged) — only this
        // coarse verdict distinguishes the band.
        "L3-RENDERED" if len < THIN_SHELL_MAX_BYTES => ChallengeVerdict::ThinShell,
        "L3-RENDERED" => ChallengeVerdict::Pass,
        "THIN-BODY" => ChallengeVerdict::RenderIncomplete,
        // FP-B4: a Cloudflare challenge in a *large* body is the
        // orchestrator shell that ran but never cleared (udemy class) —
        // ChallengeIncomplete, NOT SensorFail (the body was never
        // replaced with real content, so the sensor did not "score us
        // bot"; mislabeling it SensorFail misdirects work to fingerprint
        // tuning). A small CF stub stays EdgeBlock (classic edge deny).
        "Cloudflare-CHL" if len >= SENSOR_SPLIT_BYTES => {
            ChallengeVerdict::ChallengeIncomplete
        }
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

/// FP-C2: does this *initial response body* indicate a Cloudflare
/// challenge / Managed-Challenge orchestrator? Mirrors the
/// `datadome_handler::is_datadome_challenge_doc` pattern so the navigate
/// loop can set a *persistent* `started_as_cf_challenge` origin flag.
/// Without it, the cookie-diff retry / pending-nav poll gate on the
/// *post-mutation* DOM; CF's orchestrator rewrites the body, so the
/// marker drops while `cf_clearance` was never issued and a
/// body-mutated-but-unsolved CF page silently slips the retry gate
/// (the doc-20 mutable-state-guard class). Single source of truth for
/// the CF-origin substrings.
pub fn is_cf_challenge_doc(body: &str) -> bool {
    body.contains("_cf_chl_opt")
        || body.contains("/cdn-cgi/challenge-platform/")
        || body.contains("cf-browser-verification")
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

    // FP-B2 regression: a fully-rendered multi-MB page that merely
    // *contains the literal substring* `px-captcha` / `captcha-delivery.com`
    // (CSS class, analytics key, cookie-consent JSON manifest — the
    // historical wayfair FP root) must classify as a pass, while a real
    // small interstitial with the same token must still be detected.
    #[test]
    fn fp_b2_literal_strong_markers_size_gated() {
        let big = |seed: &str| {
            let mut h = String::from("<html><body>");
            h.push_str(seed);
            for _ in 0..40000 {
                h.push_str("<div>actual rendered product card content</div>");
            }
            h.push_str("</body></html>");
            assert!(h.len() > 1_000_000);
            h
        };
        // wayfair shape: px-captcha only in a cookie-consent manifest.
        let wf = big(r#"<script>window.__CONSENT={"_px3":"NECESSARY","px-captcha":"NECESSARY"};</script>"#);
        assert_eq!(engine_classify(&wf).tag, "L3-RENDERED");
        assert_eq!(engine_classify(&wf).verdict, ChallengeVerdict::Pass);
        // captcha-delivery.com literal in a large rendered page.
        let dd = big(r#"<img src="https://x.captcha-delivery.com/pixel.gif">"#);
        assert_eq!(engine_classify(&dd).tag, "L3-RENDERED");
        assert_eq!(engine_classify(&dd).verdict, ChallengeVerdict::Pass);
        // True detection preserved: a real small PerimeterX interstitial
        // whose only signal is the `px-captcha` hook is still detected as
        // a PerimeterX challenge via the relocated SMALL_BODY row (and
        // the bare-`captcha` row does not steal the attribution). No
        // "press & hold" text here — that is a separate, also-valid
        // PerimeterX-PaH phrase and would mask the row this test pins.
        let px_chl = r#"<html><body><div id="px-captcha"></div><p>verifying</p></body></html>"#;
        assert_eq!(engine_classify(px_chl).tag, "PerimeterX-CHL");
        assert!(engine_classify(px_chl).verdict.is_challenge());
        // And the PaH phrase still classifies as a PerimeterX challenge.
        let pah = r#"<html><body><p>Press &amp; Hold to confirm</p></body></html>"#;
        assert_eq!(engine_classify(pah).tag, "PerimeterX-PaH");
        assert!(engine_classify(pah).verdict.is_challenge());
    }

    // FP-B4 regression: a large Cloudflare orchestrator shell that ran
    // but never cleared (udemy class) classifies ChallengeIncomplete —
    // NOT SensorFail (no fingerprint scoring happened) and NOT Pass (no
    // real content). A small CF stub stays EdgeBlock. A genuinely
    // passed CF page that merely retains the always-on
    // `/cdn-cgi/challenge-platform/` JSD URL (but no `_cf_chl_opt`)
    // stays a pass — proving the discriminator is the challenge-only
    // `_cf_chl_opt`, not the JSD URL.
    #[test]
    fn fp_b4_cf_incomplete_split_from_sensorfail() {
        let mut shell = String::from(
            r#"<html><head><title>Just a moment...</title></head><body>
               <script>window._cf_chl_opt={cvId:'3',cType:'managed'};</script>"#,
        );
        for _ in 0..2000 {
            shell.push_str("<div>cf challenge orchestrator shell padding</div>");
        }
        shell.push_str("</body></html>");
        assert!(shell.len() >= SENSOR_SPLIT_BYTES);
        let ec = engine_classify(&shell);
        assert_eq!(ec.tag, "Cloudflare-CHL");
        assert_eq!(ec.verdict, ChallengeVerdict::ChallengeIncomplete);
        assert_ne!(ec.verdict, ChallengeVerdict::SensorFail);
        assert_ne!(ec.verdict, ChallengeVerdict::Pass);
        assert!(ec.verdict.is_challenge(), "incomplete CF is still an unsolved challenge");

        // Small CF stub ⇒ EdgeBlock (unchanged classic edge deny).
        let stub = "<html><body><script>window._cf_chl_opt={}</script>Just a moment...</body></html>";
        assert_eq!(engine_classify(stub).verdict, ChallengeVerdict::EdgeBlock);

        // Passed CF page: real content + the always-on JSD URL but NO
        // `_cf_chl_opt` ⇒ must remain a pass (no false ChallengeIncomplete).
        let mut passed = String::from(
            r#"<html><body><script src="/cdn-cgi/challenge-platform/h/b/jsd"></script>"#,
        );
        for _ in 0..3000 {
            passed.push_str("<article>real rendered course catalog content</article>");
        }
        passed.push_str("</body></html>");
        assert!(passed.len() >= SENSOR_SPLIT_BYTES);
        assert_eq!(engine_classify(&passed).tag, "L3-RENDERED");
        assert_eq!(engine_classify(&passed).verdict, ChallengeVerdict::Pass);
    }

    // FP-C2 regression: the persistent CF-origin predicate fires on the
    // CF challenge / orchestrator shell (so the navigate-loop retry/poll
    // stays active even after the orchestrator mutates the DOM and the
    // marker drops from the live body) and does NOT fire on benign
    // rendered content.
    #[test]
    fn fp_c2_cf_challenge_doc_predicate() {
        assert!(is_cf_challenge_doc(
            r#"<script>window._cf_chl_opt={cvId:'3'};</script>"#
        ));
        assert!(is_cf_challenge_doc(
            r#"<script src="/cdn-cgi/challenge-platform/h/b/jsd/r/x"></script>"#
        ));
        assert!(is_cf_challenge_doc(
            r#"<html class="cf-browser-verification"><body>...</body></html>"#
        ));
        assert!(!is_cf_challenge_doc(
            "<html><body>fully rendered course catalog, no CF challenge</body></html>"
        ));
    }

    // FP-B3 regression: a small rendered, challenge-free body is
    // ThinShell (not over-counted as a full Pass); a large one is Pass;
    // the holistic `tag` stays L3-RENDERED for BOTH (ledger unchanged);
    // ThinShell is not a challenge.
    #[test]
    fn fp_b3_thin_shell_band() {
        // ~3 KB rendered shell, no challenge marker.
        let mut shell = String::from("<html><body>");
        for _ in 0..60 {
            shell.push_str("<div>spa hydration placeholder</div>");
        }
        shell.push_str("</body></html>");
        assert!(
            shell.len() > THIN_BODY_MAX_BYTES && shell.len() < THIN_SHELL_MAX_BYTES
        );
        let ec = engine_classify(&shell);
        assert_eq!(ec.tag, "L3-RENDERED", "ledger tag unchanged");
        assert_eq!(ec.verdict, ChallengeVerdict::ThinShell);
        assert!(!ec.verdict.is_challenge(), "ThinShell is not a challenge");
        // A large rendered body is a full Pass (same tag).
        let mut full = String::from("<html><body>");
        for _ in 0..1000 {
            full.push_str("<article>real rendered content paragraph here</article>");
        }
        full.push_str("</body></html>");
        assert!(full.len() >= THIN_SHELL_MAX_BYTES);
        let fc = engine_classify(&full);
        assert_eq!(fc.tag, "L3-RENDERED");
        assert_eq!(fc.verdict, ChallengeVerdict::Pass);
        // Below the thin-body floor stays RenderIncomplete (unchanged).
        assert_eq!(
            engine_classify("<html></html>").verdict,
            ChallengeVerdict::RenderIncomplete
        );
    }

    // FP-D2 regression: the dead `cf_clearance` success path means the
    // engine can never *fabricate* a CF pass. Any CF challenge body
    // (small stub OR large orchestrator shell) must classify as a
    // challenge (is_challenge==true) and NEVER as Pass — the verdict
    // invariant that documents the structurally-unreachable success
    // branch until FP-E1's iframe interception lands.
    #[test]
    fn fp_d2_cf_unsolved_never_passes() {
        // Small CF stub.
        let stub = "<html><body>Just a moment...<script>window._cf_chl_opt={}</script></body></html>";
        let s = engine_classify(stub);
        assert!(s.verdict.is_challenge());
        assert_ne!(s.verdict, ChallengeVerdict::Pass);
        // Large CF orchestrator shell (udemy class).
        let mut shell = String::from(r#"<script>window._cf_chl_opt={cvId:'3'}</script>"#);
        for _ in 0..2500 {
            shell.push_str("<div>cf shell padding padding padding</div>");
        }
        assert!(shell.len() >= SENSOR_SPLIT_BYTES);
        let l = engine_classify(&shell);
        assert!(l.verdict.is_challenge());
        assert_ne!(l.verdict, ChallengeVerdict::Pass);
        assert_eq!(l.verdict, ChallengeVerdict::ChallengeIncomplete);
    }

    #[test]
    fn verdict_mapping_is_consistent() {
        assert_eq!(engine_classify("<html></html>").verdict, ChallengeVerdict::RenderIncomplete);
        assert_eq!(
            engine_classify("<html><body>Just a moment...</body></html>").verdict,
            ChallengeVerdict::EdgeBlock
        );
        // A still-any-size structural UNAMBIGUOUS token in a ≥50 KB body
        // ⇒ SensorFail (post-FP-B2 only the genuinely-structural URL/var
        // tokens remain any-size; `px-captcha` is now size-gated and
        // would instead be a pass here — see fp_b2_* test).
        let mut big = String::from(r#"<script>var ddcaptchaEncoded="z";</script>"#);
        for _ in 0..3000 {
            big.push_str("<p>padding padding padding padding</p>");
        }
        assert!(big.len() >= SENSOR_SPLIT_BYTES);
        assert_eq!(engine_classify(&big).verdict, ChallengeVerdict::SensorFail);
    }
}
