//! Cloudflare challenge detection — V1 scaffolding.
//!
//! Pure-Rust module that recognises a Cloudflare challenge response and
//! extracts the inline `_cf_chl_opt` configuration that the orchestrator
//! script reads. Mirrors the layout of `kasada.rs`: detection +
//! per-host context only, no V8 dependency.
//!
//! V1 scope (per `docs/RESEARCH_CLOUDFLARE_BYPASS_2026_05_10.md` §0/§9):
//! - Detect `cf-mitigated: challenge` header and the body-level
//!   `window._cf_chl_opt = { ... }` blob.
//! - Extract `cType`, `cRay`, `cZone`, `cN` (CSP nonce), the orchestrator
//!   script `src=`, `fa`, `mdrd` so the engine can log telemetry and
//!   choose follow-up behaviour.
//! - **Not** a homemade IUAM PoW solver. The PoW algorithm rotates
//!   per-Ray-ID; the right answer is to run the orchestrator JS to
//!   completion in our V8 + DOM (see `Page::handle_cloudflare_flow`).
//!
//! V2 (out of scope here):
//! - Turnstile widget answer collection.
//! - Privacy Pass token redemption (we cannot mint these — Cloudflare
//!   holds the issuer key).
//! - Per-host PoW solver (pointless without algorithm extraction every
//!   30 min key-rotation window).

use std::collections::HashMap;

/// The four challenge UI types Cloudflare exposes today.
///
/// `cType` lives inside the inline `_cf_chl_opt` blob; values observed in
/// the wild are `'managed'`, `'jsch'`, `'non-interactive'`,
/// `'interactive'`. Anything else is bucketed as `Unknown`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CfChallengeKind {
    /// `cType: 'managed'` — Cloudflare's adaptive challenge. Picks
    /// invisible / non-interactive / interactive Turnstile based on bot
    /// score. udemy.com 2026-05-10 case.
    Managed,
    /// `cType: 'jsch'` — legacy JS Challenge (small client-side
    /// computation). Rare in 2026.
    Jsch,
    /// `cType: 'non-interactive'` — Turnstile widget that completes
    /// without user input.
    NonInteractive,
    /// `cType: 'interactive'` — Turnstile widget with mandatory user
    /// gesture. Headless-only solvers cannot complete this; needs a
    /// captcha service.
    Interactive,
    /// Detected as a CF challenge (header or body marker present) but
    /// `cType` was missing or unrecognized.
    Unknown,
}

impl CfChallengeKind {
    fn from_str(s: &str) -> Self {
        match s {
            "managed" => Self::Managed,
            "jsch" => Self::Jsch,
            "non-interactive" => Self::NonInteractive,
            "interactive" => Self::Interactive,
            _ => Self::Unknown,
        }
    }

    /// Whether the V1 orchestrator-runner is expected to succeed on this
    /// challenge type. Interactive Turnstile requires a captcha service
    /// and is documented out of scope.
    pub fn v1_solvable(&self) -> bool {
        matches!(
            self,
            Self::Managed | Self::Jsch | Self::NonInteractive | Self::Unknown
        )
    }
}

/// Parsed Cloudflare challenge state for one response.
///
/// All fields are optional except `kind` and `ray` (which we always
/// recover from at least the `cf-ray` response header). Use
/// [`detect_challenge`] to construct.
#[derive(Debug, Clone)]
pub struct CfChallengeContext {
    pub kind: CfChallengeKind,
    /// `cf-ray` header value (`<id>-<colo>`). Used as the dynamic
    /// decryption key for the orchestrator's stage-2 payload.
    pub ray: String,
    /// `cZone` from `_cf_chl_opt` — the host the challenge is gating.
    pub zone: String,
    /// `cN` — the CSP nonce. The challenge platform script must be
    /// loaded with this nonce or the page's CSP rejects it.
    pub csp_nonce: String,
    /// `<script src="...">` attached at the bottom of the inline blob.
    /// Path looks like
    /// `/cdn-cgi/challenge-platform/h/{b,g}/orchestrate/{managed,chl_page,jsch}/v1?ray=...`.
    pub orchestrator_url: String,
    /// `fa` — full-action token URL fragment, used by the orchestrator
    /// when posting back the final clearance payload.
    pub fa_url: String,
    /// `mdrd` — metadata-redirect token. Some flows attach it as a query
    /// parameter on the clearance redirect.
    pub mdrd: String,
    /// `cFPWv` — platform variant ('b' or 'g'). Selects the
    /// `/cdn-cgi/challenge-platform/h/{b,g}/...` path family.
    pub platform_variant: String,
    /// Did the response carry the canonical `cf-mitigated: challenge`
    /// header? If false we recovered the challenge from body markers
    /// only — still actionable, but the header is the authoritative
    /// signal per Cloudflare docs.
    pub cf_mitigated_header: bool,
}

impl CfChallengeContext {
    /// One-line summary suitable for a tracing/eprintln log.
    pub fn summary(&self) -> String {
        format!(
            "cf={:?} ray={} zone={} variant={} orchestrator={} mitigated_hdr={}",
            self.kind,
            self.ray,
            self.zone,
            self.platform_variant,
            self.orchestrator_url,
            self.cf_mitigated_header,
        )
    }
}

/// Detect a Cloudflare challenge response.
///
/// `headers` should be a header-name (lowercase) → value map; pass the
/// top-level response headers (the same map our [`net`] crate already
/// surfaces). `body` is the response body as a `&str`.
///
/// Returns `Some(ctx)` if **either** the `cf-mitigated: challenge`
/// header is present **or** the body contains a recognisable
/// `_cf_chl_opt` blob. Returns `None` otherwise.
///
/// We deliberately do **not** require the header — downstream proxies
/// occasionally strip non-standard headers and we still want to fire on
/// the body marker. This matches the false-positive-tolerant approach in
/// `Page::is_anti_bot_challenge`.
pub fn detect_challenge(
    headers: &HashMap<String, String>,
    body: &str,
) -> Option<CfChallengeContext> {
    let cf_mitigated = headers
        .get("cf-mitigated")
        .map(|v| v.eq_ignore_ascii_case("challenge"))
        .unwrap_or(false);

    let has_chl_opt = body.contains("_cf_chl_opt");
    let has_chl_platform = body.contains("/cdn-cgi/challenge-platform/");
    let server_is_cf = headers
        .get("server")
        .map(|v| v.to_ascii_lowercase().contains("cloudflare"))
        .unwrap_or(false);

    // Require either the canonical header OR (one body marker AND
    // server: cloudflare). Bare body markers without `server: cloudflare`
    // would over-match on the research notes themselves.
    let is_challenge = cf_mitigated
        || ((has_chl_opt || has_chl_platform) && server_is_cf);
    if !is_challenge {
        return None;
    }

    let ray = headers
        .get("cf-ray")
        .cloned()
        .or_else(|| extract_field(body, "cRay"))
        .unwrap_or_default();
    let zone = extract_field(body, "cZone").unwrap_or_default();
    let csp_nonce = extract_field(body, "cN").unwrap_or_default();
    let fa_url = extract_field(body, "fa").unwrap_or_default();
    let mdrd = extract_field(body, "mdrd").unwrap_or_default();
    let platform_variant = extract_field(body, "cFPWv").unwrap_or_default();
    let kind = extract_field(body, "cType")
        .map(|s| CfChallengeKind::from_str(&s))
        .unwrap_or(CfChallengeKind::Unknown);
    let orchestrator_url = extract_orchestrator_url(body).unwrap_or_default();

    Some(CfChallengeContext {
        kind,
        ray,
        zone,
        csp_nonce,
        orchestrator_url,
        fa_url,
        mdrd,
        platform_variant,
        cf_mitigated_header: cf_mitigated,
    })
}

/// Extract a single quoted value from the inline `_cf_chl_opt` literal.
///
/// The blob is JS, not strict JSON — it is delimited by `{ ... }` and
/// fields look like `<key>: '<value>'` (single quotes) or `<key>: <num>`
/// (unquoted numerics, irrelevant for V1). We only need the quoted
/// strings, so a small tolerant scanner is sufficient.
fn extract_field(body: &str, key: &str) -> Option<String> {
    // Match `key: '...'` or `key:'...'`. Fields ride a leading `,` or
    // `{` so we anchor on those plus optional whitespace.
    let needles = [format!("{}:", key), format!("{} :", key)];
    for needle in &needles {
        let mut search_from = 0;
        while let Some(idx) = body[search_from..].find(needle.as_str()) {
            let abs = search_from + idx;
            // Reject mid-identifier matches: previous char must be
            // whitespace, `,`, or `{`.
            let before_ok = abs == 0
                || matches!(
                    body.as_bytes()[abs - 1],
                    b' ' | b'\t' | b'\n' | b'\r' | b',' | b'{'
                );
            if !before_ok {
                search_from = abs + needle.len();
                continue;
            }
            let after = &body[abs + needle.len()..];
            let after = after.trim_start_matches([' ', '\t', '\n', '\r']);
            if let Some(rest) = after.strip_prefix('\'') {
                if let Some(end) = rest.find('\'') {
                    return Some(rest[..end].to_string());
                }
            }
            if let Some(rest) = after.strip_prefix('"') {
                if let Some(end) = rest.find('"') {
                    return Some(rest[..end].to_string());
                }
            }
            search_from = abs + needle.len();
        }
    }
    None
}

/// Find the `<script src="/cdn-cgi/challenge-platform/...">` tag the
/// inline blob appends to `<head>`. Falls back to grepping the raw
/// path if the tag form is not present.
fn extract_orchestrator_url(body: &str) -> Option<String> {
    // Direct grep for the canonical path prefix.
    let needle = "/cdn-cgi/challenge-platform/";
    let idx = body.find(needle)?;
    // Walk forward until we hit a quote, `<`, `>`, whitespace, or `;`.
    let tail = &body[idx..];
    let end = tail
        .find(|c: char| {
            matches!(c, '"' | '\'' | '<' | '>' | ' ' | '\t' | '\n' | '\r' | ';')
        })
        .unwrap_or(tail.len());
    Some(tail[..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Synthetic body modelled on the udemy.com 2026-05-10 capture in
    /// `docs/RESEARCH_CLOUDFLARE_BYPASS_2026_05_10.md` §6. We elide the
    /// long opaque base64 fields but keep the structure intact.
    const UDEMY_BODY: &str = r#"
<html><head><title>Just a moment...</title></head><body>
<script>
window._cf_chl_opt = {
    cFPWv: 'g',
    cH: 'EwhK3.EvWCoWNMFp08JGZPYK4Kgq.KINXsAzHRc2JtI-1778431871-1.2.1.1-xxx',
    cITimeS: '1778431871',
    cN: 'VMXi8tUab5dnlKwLSWc0p3',
    cRay: '9f9a727a2808cb95',
    cType: 'managed',
    cUPMDTk: '/?__cf_chl_tk=xxx',
    cvId: '3',
    cZone: 'www.udemy.com',
    fa: '/?__cf_chl_f_tk=yyy',
    md: '1wf1ARxfTUw',
    mdrd: 'PGcLxGtOfF6NY'
};
var a = document.createElement('script');
a.nonce = 'VMXi8tUab5dnlKwLSWc0p3';
a.src = '/cdn-cgi/challenge-platform/h/g/orchestrate/managed/v1?ray=9f9a727a2808cb95';
document.head.appendChild(a);
</script></body></html>
"#;

    fn udemy_headers() -> HashMap<String, String> {
        let mut h = HashMap::new();
        h.insert("cf-mitigated".into(), "challenge".into());
        h.insert("cf-ray".into(), "9f9a7259de914d45-YVR".into());
        h.insert("server".into(), "cloudflare".into());
        h
    }

    #[test]
    fn detects_managed_challenge_from_udemy() {
        let ctx = detect_challenge(&udemy_headers(), UDEMY_BODY)
            .expect("should detect challenge");
        assert_eq!(ctx.kind, CfChallengeKind::Managed);
        assert_eq!(ctx.ray, "9f9a7259de914d45-YVR");
        assert_eq!(ctx.zone, "www.udemy.com");
        assert_eq!(ctx.csp_nonce, "VMXi8tUab5dnlKwLSWc0p3");
        assert_eq!(ctx.platform_variant, "g");
        assert_eq!(ctx.fa_url, "/?__cf_chl_f_tk=yyy");
        assert_eq!(ctx.mdrd, "PGcLxGtOfF6NY");
        assert!(ctx.cf_mitigated_header);
        assert!(ctx
            .orchestrator_url
            .starts_with("/cdn-cgi/challenge-platform/h/g/orchestrate/managed/v1"));
    }

    #[test]
    fn falls_back_to_body_when_header_missing() {
        let mut h = HashMap::new();
        h.insert("server".into(), "cloudflare".into());
        // No `cf-mitigated` header — body markers should still trigger.
        let ctx = detect_challenge(&h, UDEMY_BODY).expect("body fallback");
        assert!(!ctx.cf_mitigated_header);
        assert_eq!(ctx.kind, CfChallengeKind::Managed);
    }

    #[test]
    fn ignores_non_cloudflare_responses() {
        let mut h = HashMap::new();
        h.insert("server".into(), "nginx".into());
        // Body looks vaguely CF-shaped but no server: cloudflare and no
        // cf-mitigated → should NOT trigger (would be a false positive
        // against e.g. a research blog post that quotes _cf_chl_opt).
        let body = "see _cf_chl_opt example /cdn-cgi/challenge-platform/...";
        assert!(detect_challenge(&h, body).is_none());
    }

    #[test]
    fn detects_jsch_kind() {
        let mut h = HashMap::new();
        h.insert("cf-mitigated".into(), "challenge".into());
        let body = "var _cf_chl_opt = { cType: 'jsch', cRay: 'abc' };";
        let ctx = detect_challenge(&h, body).unwrap();
        assert_eq!(ctx.kind, CfChallengeKind::Jsch);
    }

    #[test]
    fn detects_interactive_kind() {
        let mut h = HashMap::new();
        h.insert("cf-mitigated".into(), "challenge".into());
        let body = "_cf_chl_opt = { cType: 'interactive' }";
        let ctx = detect_challenge(&h, body).unwrap();
        assert_eq!(ctx.kind, CfChallengeKind::Interactive);
        assert!(!ctx.kind.v1_solvable());
    }

    #[test]
    fn extracts_orchestrator_url_from_inline_script() {
        let body = r#"a.src = '/cdn-cgi/challenge-platform/h/b/orchestrate/jsch/v1?ray=xxx';"#;
        let url = extract_orchestrator_url(body).unwrap();
        assert_eq!(
            url,
            "/cdn-cgi/challenge-platform/h/b/orchestrate/jsch/v1?ray=xxx"
        );
    }

    #[test]
    fn extract_field_handles_double_quotes() {
        let body = r#"{ cType: "managed", cRay: "xyz" }"#;
        assert_eq!(extract_field(body, "cType").as_deref(), Some("managed"));
        assert_eq!(extract_field(body, "cRay").as_deref(), Some("xyz"));
    }

    #[test]
    fn cf_mitigated_only_no_body_still_detects() {
        let mut h = HashMap::new();
        h.insert("cf-mitigated".into(), "challenge".into());
        let ctx = detect_challenge(&h, "").expect("header alone is canonical");
        assert!(ctx.cf_mitigated_header);
        assert_eq!(ctx.kind, CfChallengeKind::Unknown);
    }
}
