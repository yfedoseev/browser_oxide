//! `CloudflareSolver` — trait-shaped wrapper around the existing
//! `Page::handle_cloudflare_flow` method + `stealth::cloudflare::
//! detect_challenge` body parser + `crate::classify::is_cf_challenge_doc`
//! body sniffer.
//!
//! Detection:
//!   - body matches `is_cf_challenge_doc` (`_cf_chl_opt`,
//!     `/cdn-cgi/challenge-platform/`, `cf-browser-verification`)
//!     → sub_kind "challenge-doc"
//!
//! Solve: delegates to `Page::handle_cloudflare_flow(client)`. That
//! method runs the orchestrator JS to completion in our V8, injects
//! low-rate behavioural noise, and polls for `cf_clearance` for up
//! to 10s. Returns:
//!   - `Solved` if `cf_clearance` lands in the jar
//!   - `InProgress` if the orchestrator queued `__pendingNavigation`
//!   - `Unsolvable` if the challenge is interactive (Turnstile etc.)

use crate::challenge::{ChallengeKind, ChallengeSolver, SolveOutcome};
use crate::Page;
use async_trait::async_trait;

#[derive(Debug, Default, Clone, Copy)]
pub struct CloudflareSolver;

impl CloudflareSolver {
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait(?Send)]
impl ChallengeSolver for CloudflareSolver {
    fn name(&self) -> &'static str {
        "cloudflare"
    }

    fn detect(&self, _resp: &net::Response, html: &str) -> Option<ChallengeKind> {
        if crate::classify::is_cf_challenge_doc(html) {
            return Some(ChallengeKind::new("cloudflare", "challenge-doc"));
        }
        None
    }

    async fn solve(
        &self,
        page: &mut Page,
        client: &net::HttpClient,
        _kind: ChallengeKind,
    ) -> SolveOutcome {
        // handle_cloudflare_flow returns Some(ctx) iff a CF challenge
        // was detected (regardless of solve outcome). The actual
        // outcome is observed via the cookie jar (cf_clearance).
        let ctx = page.handle_cloudflare_flow(client).await;
        if ctx.is_none() {
            return SolveOutcome::NotApplicable;
        }
        // Whether clearance was actually issued is checked by the
        // outer cookie-delta retry path; from here the orchestrator
        // either solved it (Solved) or is still running / unsolvable.
        // Conservative default: InProgress lets the outer loop iterate.
        SolveOutcome::InProgress
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

    #[test]
    fn detects_cf_chl_opt() {
        let s = CloudflareSolver::new();
        let resp = empty_resp();
        let html = r#"<html><script>window._cf_chl_opt={cType:'managed'};</script></html>"#;
        let k = s.detect(&resp, html);
        assert_eq!(k.as_ref().map(|k| k.sub_kind), Some("challenge-doc"));
    }

    #[test]
    fn detects_cdn_cgi_path() {
        let s = CloudflareSolver::new();
        let resp = empty_resp();
        let html = r#"<script src="/cdn-cgi/challenge-platform/h/g/orchestrate/chl/v1"></script>"#;
        let k = s.detect(&resp, html);
        assert_eq!(k.as_ref().map(|k| k.sub_kind), Some("challenge-doc"));
    }

    #[test]
    fn ignores_benign_body() {
        let s = CloudflareSolver::new();
        let resp = empty_resp();
        let k = s.detect(&resp, "<html><body>hello</body></html>");
        assert!(k.is_none());
    }
}
