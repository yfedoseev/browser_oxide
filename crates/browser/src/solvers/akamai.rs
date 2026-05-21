//! `AkamaiSolver` — trait-shaped wrapper around the existing
//! `Page::handle_akamai_flow` method + `akamai::*` primitives.
//!
//! Detection:
//!   - `sec-if-cpt-container` / `sec-cpt-if` in body → sub_kind "sec-cpt"
//!     (Akamai sec-cpt PoW interstitial; the bundle self-solves so the
//!     solver just lets the navigation iterate while the per-iter poll
//!     watches the `sec_cpt` cookie via [`Self::solved_signal`])
//!   - `_abck` Set-Cookie OR `akam/13` body marker → sub_kind
//!     "sensor-data" (vanilla BMP, drives `Page::handle_akamai_flow`)
//!
//! Solve: delegates to `Page::handle_akamai_flow(client)`.
//!
//! `solved_signal` consults `akamai::sec_cpt::sec_cpt_solved` so the
//! per-iteration cookie-watch loop can break out the instant the
//! sec-cpt cookie transitions to the `~3~` (solved) marker.

use crate::challenge::{ChallengeKind, ChallengeSolver, SolveOutcome};
use crate::Page;
use async_trait::async_trait;

#[derive(Debug, Default, Clone, Copy)]
pub struct AkamaiSolver;

impl AkamaiSolver {
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait(?Send)]
impl ChallengeSolver for AkamaiSolver {
    fn name(&self) -> &'static str {
        "akamai-bmp"
    }

    fn detect(&self, resp: &net::Response, html: &str) -> Option<ChallengeKind> {
        // sec-cpt is the more specific variant — check first.
        if html.contains("sec-if-cpt-container") || html.contains("sec-cpt-if") {
            return Some(ChallengeKind::new("akamai-bmp", "sec-cpt"));
        }
        // _abck Set-Cookie. Stored in resp.set_cookies (HashMap-headers
        // would collapse duplicate Set-Cookie headers).
        let has_abck = resp
            .set_cookies
            .iter()
            .any(|v| v.trim_start().starts_with("_abck="));
        // `akam/13` is the BMP bootstrap script; alone it's not a
        // challenge signal (rendered pages embed it too). Treat as a
        // challenge only when paired with the trust-state cookie.
        if has_abck {
            return Some(ChallengeKind::new("akamai-bmp", "sensor-data"));
        }
        None
    }

    async fn solve(
        &self,
        page: &mut Page,
        client: &net::HttpClient,
        kind: ChallengeKind,
    ) -> SolveOutcome {
        // sec-cpt bundle self-solves; BMP sensor_data POST against a
        // sec-cpt verify endpoint loops forever (doc-20 anti-pattern).
        // Yield to let the bundle's own JS clear the cookie; the
        // per-iter cookie-watch loop will break on solved_signal().
        if kind.sub_kind == "sec-cpt" {
            return SolveOutcome::InProgress;
        }
        match page.handle_akamai_flow(client).await {
            Ok(akamai::AbckState::Favorable) | Ok(akamai::AbckState::Invalidated) => {
                SolveOutcome::Solved
            }
            Ok(akamai::AbckState::NeedsSensor)
            | Ok(akamai::AbckState::NeedsSecCpt)
            | Ok(akamai::AbckState::NeedsPixel) => SolveOutcome::InProgress,
            Ok(akamai::AbckState::Unknown) => SolveOutcome::NotApplicable,
            Err(_) => SolveOutcome::Unsolvable,
        }
    }

    fn solved_signal(&self, cookies: &str, _body: &str) -> bool {
        akamai::sec_cpt::sec_cpt_solved(cookies)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_resp() -> net::Response {
        net::Response {
            url: String::new(),
            status: 200,
            status_text: String::new(),
            headers: std::collections::HashMap::new(),
            set_cookies: Vec::new(),
            body: Vec::new(),
            accept_ch_upgrade: false,
            timings: Default::default(),
        }
    }

    #[test]
    fn detects_sec_cpt_in_body() {
        let s = AkamaiSolver::new();
        let resp = empty_resp();
        let k = s.detect(&resp, r#"<div id="sec-if-cpt-container"></div>"#);
        assert_eq!(k.as_ref().map(|k| k.sub_kind), Some("sec-cpt"));
    }

    #[test]
    fn detects_abck_set_cookie() {
        let s = AkamaiSolver::new();
        let mut resp = empty_resp();
        resp.set_cookies
            .push("_abck=ABCD~0~-1~; Path=/".to_string());
        let k = s.detect(&resp, "");
        assert_eq!(k.as_ref().map(|k| k.sub_kind), Some("sensor-data"));
    }

    #[test]
    fn ignores_benign_body() {
        let s = AkamaiSolver::new();
        let resp = empty_resp();
        let k = s.detect(&resp, "<html><body>hello world</body></html>");
        assert!(k.is_none());
    }

    #[test]
    fn solved_signal_delegates_to_sec_cpt() {
        let s = AkamaiSolver::new();
        assert!(s.solved_signal("sec_cpt=abc~3~deadbeef", ""));
        assert!(!s.solved_signal("sec_cpt=abc~1~xyz", ""));
        assert!(!s.solved_signal("foo=bar", ""));
    }
}
