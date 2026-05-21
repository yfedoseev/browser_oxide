//! `KasadaSolver` — trait-shaped wrapper around Kasada handling.
//!
//! Unlike Akamai (which has the dedicated `Page::handle_akamai_flow`
//! method), Kasada is mostly PASSIVE on the engine side: the page's
//! own `ips.js` runs the SHA-256 proof-of-work loop inside our V8,
//! and the net crate quietly learns/injects `x-kpsdk-*` headers and
//! the per-origin session store on every request/response.
//!
//! So this solver's role today is:
//!   - `detect`: recognise a Kasada-served response (body markers
//!     `_kpsdk` / `ips.js`, OR per-host learned session state).
//!   - `solve`: return `InProgress` and let `Page::navigate`'s outer
//!     loop keep driving the event loop until `ips.js` completes.
//!   - `prepare_request` / `observe_response`: no-op for Stage 2 —
//!     the existing net-level wiring still does it. In Stage 3 these
//!     hooks become the canonical path and the net-level Kasada
//!     knowledge is removed.

use crate::challenge::{ChallengeKind, ChallengeSolver, SolveOutcome};
use crate::Page;
use async_trait::async_trait;

#[derive(Debug, Default, Clone, Copy)]
pub struct KasadaSolver;

impl KasadaSolver {
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait(?Send)]
impl ChallengeSolver for KasadaSolver {
    fn name(&self) -> &'static str {
        "kasada"
    }

    fn detect(&self, resp: &net::Response, html: &str) -> Option<ChallengeKind> {
        // x-kpsdk-* response headers — Kasada always sets at least
        // `x-kpsdk-st` on the initial protected response.
        let has_kpsdk_header = resp
            .headers
            .keys()
            .any(|k| k.to_ascii_lowercase().starts_with("x-kpsdk-"));
        if has_kpsdk_header {
            return Some(ChallengeKind::new("kasada", "header"));
        }
        // Body markers: `_kpsdk` global, or `ips.js` script path.
        let lower = html.to_ascii_lowercase();
        if lower.contains("_kpsdk") || lower.contains("ips.js") {
            return Some(ChallengeKind::new("kasada", "ips-js"));
        }
        None
    }

    async fn solve(
        &self,
        _page: &mut Page,
        _client: &net::HttpClient,
        _kind: ChallengeKind,
    ) -> SolveOutcome {
        // ips.js is already running inside our V8 from the initial
        // page build; nothing to actively drive here. The outer
        // navigate loop will pump the event loop and watch for the
        // session-token cookie to land.
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
    fn detects_kpsdk_header() {
        let mut resp = empty_resp();
        resp.headers
            .insert("X-Kpsdk-St".to_string(), "12345".to_string());
        let k = KasadaSolver::new().detect(&resp, "");
        assert_eq!(k.as_ref().map(|k| k.sub_kind), Some("header"));
    }

    #[test]
    fn detects_ips_js_in_body() {
        let resp = empty_resp();
        let html = r#"<script src="/ips.js?x-kpsdk-im=abc"></script>"#;
        let k = KasadaSolver::new().detect(&resp, html);
        assert_eq!(k.as_ref().map(|k| k.sub_kind), Some("ips-js"));
    }

    #[test]
    fn detects_kpsdk_global_in_body() {
        let resp = empty_resp();
        let html = "<html><script>window._kpsdk={}</script></html>";
        let k = KasadaSolver::new().detect(&resp, html);
        assert_eq!(k.as_ref().map(|k| k.sub_kind), Some("ips-js"));
    }

    #[test]
    fn ignores_benign_body() {
        let resp = empty_resp();
        let k = KasadaSolver::new().detect(&resp, "<html><body>hi</body></html>");
        assert!(k.is_none());
    }
}
