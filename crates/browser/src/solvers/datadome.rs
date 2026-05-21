//! `DataDomeSolver` — trait-shaped wrapper around the existing
//! `crate::datadome_handler::*` detection helpers.
//!
//! Detection:
//!   - `is_datadome_challenge_doc(html)` (the `var dd={…}` interstitial
//!     shape used on etsy/tripadvisor/wsj/reuters) → sub_kind "rt-i"
//!   - body contains `captcha-delivery.com` / `ddcaptchaencoded`
//!     → sub_kind "marker"
//!
//! Solve: returns `InProgress`. DataDome's `i.js` runs inside our V8
//! from the page build; the outer navigate loop pumps the event loop
//! and the cookie-diff retry path re-issues the original URL once
//! i.js lands a fresh `datadome=` cookie. (The 45-s self-solve poll
//! from the old engine code stays in `Page::navigate_with_html` until
//! E2 routes it through this solver.)
//!
//! `relax_response_csp` returns `true` on a DataDome challenge doc —
//! the origin's restrictive 403 CSP must be suspended so i.js can
//! reach `geo.captcha-delivery.com`.
//!
//! `solved_signal` consults `datadome_handler::datadome_solved` so the
//! per-iter cookie-watch loop can break when a `datadome=` cookie
//! lands on a non-challenge body.

use crate::challenge::{ChallengeKind, ChallengeSolver, SolveOutcome};
use crate::Page;
use async_trait::async_trait;

#[derive(Debug, Default, Clone, Copy)]
pub struct DataDomeSolver;

impl DataDomeSolver {
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait(?Send)]
impl ChallengeSolver for DataDomeSolver {
    fn name(&self) -> &'static str {
        "datadome"
    }

    fn detect(&self, _resp: &net::Response, html: &str) -> Option<ChallengeKind> {
        if crate::datadome_handler::is_datadome_challenge_doc(html) {
            return Some(ChallengeKind::new("datadome", "rt-i"));
        }
        let lower = html.to_ascii_lowercase();
        if lower.contains("captcha-delivery.com") || lower.contains("ddcaptchaencoded") {
            return Some(ChallengeKind::new("datadome", "marker"));
        }
        None
    }

    async fn solve(
        &self,
        _page: &mut Page,
        _client: &net::HttpClient,
        _kind: ChallengeKind,
    ) -> SolveOutcome {
        // i.js runs inside V8 from the page build; the outer navigate
        // loop pumps the event loop and the cookie-diff retry re-issues
        // the original URL once the `datadome=` cookie lands.
        SolveOutcome::InProgress
    }

    fn relax_response_csp(&self, _resp: &net::Response, html: &str) -> bool {
        crate::datadome_handler::is_datadome_challenge_doc(html)
    }

    fn solved_signal(&self, cookies: &str, body: &str) -> bool {
        crate::datadome_handler::datadome_solved(cookies, body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn empty_resp() -> net::Response {
        net::Response {
            url: String::new(),
            status: 200,
            status_text: String::new(),
            headers: HashMap::new(),
            set_cookies: Vec::new(),
            body: Vec::new(),
            accept_ch_upgrade: false,
            timings: Default::default(),
        }
    }

    /// Canonical `rt:'i'` interstitial body shape.
    const DD_RTI_BODY: &str = r#"<html><body><script>
        var dd={'rt':'i','cid':'ABC','hsh':'XYZ','t':'fe','s':1,
        'host':'geo.captcha-delivery.com','cookie':'datadome'};
    </script><script src="https://geo.captcha-delivery.com/c.js"></script>
    </body></html>"#;

    #[test]
    fn detects_rti_interstitial() {
        let s = DataDomeSolver::new();
        let resp = empty_resp();
        let k = s.detect(&resp, DD_RTI_BODY);
        assert_eq!(k.as_ref().map(|k| k.sub_kind), Some("rt-i"));
    }

    #[test]
    fn detects_marker_only() {
        let s = DataDomeSolver::new();
        let resp = empty_resp();
        let html = r#"<script src="https://geo.captcha-delivery.com/x.js"></script>"#;
        let k = s.detect(&resp, html);
        assert!(k.is_some(), "marker-only body should detect");
    }

    #[test]
    fn relax_csp_only_for_challenge() {
        let s = DataDomeSolver::new();
        let resp = empty_resp();
        assert!(s.relax_response_csp(&resp, DD_RTI_BODY));
        assert!(!s.relax_response_csp(&resp, "<html><body>hi</body></html>"));
    }
}
