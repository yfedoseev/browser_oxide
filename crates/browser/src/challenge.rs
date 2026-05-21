//! `ChallengeSolver` trait — the engine's pluggable hook for handling
//! per-vendor anti-bot challenges (Akamai BMP sensor_data, Kasada
//! `x-kpsdk-*` PoW, DataDome interstitial round-trip, Cloudflare
//! orchestrator runner, etc.).
//!
//! The public engine ships the trait + the dispatch wiring; concrete
//! solver implementations live alongside (today, in
//! `crates/akamai`, `crates/stealth/src/kasada.rs`, `crates/browser/
//! src/datadome_handler.rs`, `crates/stealth/src/cloudflare.rs`) and
//! will move out as the public/private split lands. Embedders register
//! the set of solvers they want when constructing a `Page` /
//! `HttpClient`; with an empty set the engine still navigates and
//! renders, it just won't auto-clear vendor challenges.
//!
//! Lifecycle from the engine's perspective, per navigation iteration:
//!
//! 1. `observe_response(host, resp)` — every response from `HttpClient`
//!    is broadcast to every registered solver so they can learn session
//!    state (Akamai `_abck`, Kasada `x-kpsdk-cr/st`, DataDome cookies).
//! 2. `prepare_request(host, headers)` — every outgoing request is
//!    broadcast so each solver can inject vendor headers
//!    (Kasada `x-kpsdk-cd/ct/fc/h/im/dt/v/r`, Akamai `bm_sz`, …).
//! 3. After the page has been built and JS has run, `Page::navigate`
//!    walks the solvers and calls `detect(&resp, &html)`. The first
//!    solver returning `Some(ChallengeKind)` owns this iteration.
//! 4. That solver's `solve(page, client, kind)` is called. It has full
//!    access to `Page` (event loop, content, evaluate) and the
//!    `HttpClient` (for sensor_data POST, /tl, /mfc, …). Outcome drives
//!    whether the navigate loop retries, breaks, or surfaces an
//!    incomplete-challenge verdict.
//! 5. `relax_response_csp(&resp, &html)` is consulted before installing
//!    the origin's CSP — DataDome `rt:'i'` interstitials need the
//!    origin's 403 CSP suspended so `i.js` can reach
//!    `geo.captcha-delivery.com`.
//! 6. `solved_signal(&cookies, &body)` is consulted by the per-iteration
//!    poll — Akamai sec-cpt cookie flipping to `~3~` lets the engine
//!    break out of the poll early instead of burning the full budget.
//!
//! Solvers MUST be `Send + Sync` because they are shared across the
//! tokio runtime via `Arc<dyn ChallengeSolver>`. They SHOULD hold their
//! per-host session state internally (e.g. an `Arc<RwLock<HashMap<…>>>`
//! field on the impl) so the public surface stays the same regardless
//! of which solvers are registered.

use async_trait::async_trait;

/// Opaque per-vendor challenge context returned by [`ChallengeSolver::
/// detect`]. The engine treats it as a marker; the solver pulls real
/// state out via its own internal store keyed by the same host.
///
/// Carries only the vendor name + an opaque kind discriminant so a
/// solver can distinguish e.g. Cloudflare's Managed challenge from
/// JS challenge from Turnstile, without exposing the full vendor type
/// surface to the engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChallengeKind {
    /// Short telemetry name of the vendor: "akamai-bmp", "kasada",
    /// "datadome", "cloudflare-managed", "cloudflare-turnstile", …
    pub vendor: &'static str,
    /// Vendor-internal sub-kind, e.g. "sec-cpt" vs "sensor-data" for
    /// Akamai; "managed" vs "jsch" vs "non-interactive" vs "turnstile"
    /// for Cloudflare. Free-form; the engine only uses it for telemetry.
    pub sub_kind: &'static str,
}

impl ChallengeKind {
    pub const fn new(vendor: &'static str, sub_kind: &'static str) -> Self {
        Self { vendor, sub_kind }
    }
}

/// Result of [`ChallengeSolver::solve`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SolveOutcome {
    /// Solver re-examined and decided the response is NOT its vendor's
    /// challenge after all (e.g. a body-marker FP). The engine moves on
    /// to the next registered solver.
    NotApplicable,
    /// Solver is partway through (e.g. Cloudflare orchestrator pumping
    /// the event loop, waiting for `cf_clearance`). The engine should
    /// continue the navigation iteration as if the solver hadn't fired
    /// — the solver's side effects (cookie jar mutation, event loop
    /// advancement) will be picked up by the next pass through the
    /// outer retry loop.
    InProgress,
    /// Solver cleared the challenge — caller should refetch the
    /// original URL on the next iteration.
    Solved,
    /// Solver failed (network error, exhausted budget, unsolvable
    /// vendor variant like a human-gated captcha). Engine should
    /// surface `ChallengeIncomplete` and stop trying.
    Unsolvable,
}

/// Pluggable hook for handling a per-vendor anti-bot challenge.
///
/// See the module-level docs for the lifecycle. Default implementations
/// are provided for every method except [`Self::name`] so a solver only
/// has to implement what it cares about (e.g. a passive sniffer that
/// only observes responses can leave `solve` defaulted to
/// `SolveOutcome::NotApplicable`).
#[async_trait(?Send)]
pub trait ChallengeSolver: Send + Sync {
    /// Short telemetry name — same string as [`ChallengeKind::vendor`].
    /// Stable; used in log lines and not user-visible.
    fn name(&self) -> &'static str;

    /// Observe a response. May mutate the solver's internal per-host
    /// session store. Called for every response received by
    /// `HttpClient`, including subresources. Should be cheap; heavy
    /// work belongs in [`Self::solve`].
    #[allow(unused_variables)]
    async fn observe_response(&self, host: &str, resp: &net::Response) {}

    /// Prepare an outgoing request. May append/replace request headers
    /// (e.g. inject `x-kpsdk-*`, `bm_sz`). Called for every outgoing
    /// request including subresources. Must be synchronous because it
    /// runs on the hot request-encoding path.
    #[allow(unused_variables)]
    fn prepare_request(&self, host: &str, headers: &mut Vec<(String, String)>) {}

    /// Detect whether the rendered response is this solver's vendor's
    /// challenge. Pure function over `(response, html)`; no I/O. Cheap.
    /// Return `Some(kind)` if recognised, else `None`.
    #[allow(unused_variables)]
    fn detect(&self, resp: &net::Response, html: &str) -> Option<ChallengeKind> {
        None
    }

    /// Drive the challenge to resolution. Called when `detect` returned
    /// `Some(kind)`. May call `page.evaluate`, drive the event loop,
    /// POST sensor data via `client`, etc. Return the [`SolveOutcome`].
    #[allow(unused_variables)]
    async fn solve(
        &self,
        page: &mut crate::Page,
        client: &net::HttpClient,
        kind: ChallengeKind,
    ) -> SolveOutcome {
        SolveOutcome::NotApplicable
    }

    /// Should the origin's response CSP be SUSPENDED for this nav?
    /// Used by DataDome `rt:'i'` interstitials whose JS reaches
    /// `geo.captcha-delivery.com` (refused by the origin's 403 CSP).
    /// Decision is body-based. Default: false. Pure inspection — no I/O.
    #[allow(unused_variables)]
    fn relax_response_csp(&self, html: &str) -> bool {
        false
    }

    /// Has this nav's challenge been observed as solved? Used by the
    /// per-iteration event-loop poll to break out early (e.g. Akamai
    /// sec-cpt cookie flipping to the `~3~` solved marker, DataDome
    /// `datadome=` cookie appearing on a non-challenge body).
    /// Default: false. Pure inspection — no I/O.
    #[allow(unused_variables)]
    fn solved_signal(&self, cookies: &str, body: &str) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Default-impl smoke: a solver that overrides only `name()` should
    /// compile and have safe no-op defaults for every other method.
    struct PassiveSolver;
    #[async_trait(?Send)]
    impl ChallengeSolver for PassiveSolver {
        fn name(&self) -> &'static str {
            "passive"
        }
    }

    #[test]
    fn challenge_kind_basic() {
        let k = ChallengeKind::new("akamai-bmp", "sensor-data");
        assert_eq!(k.vendor, "akamai-bmp");
        assert_eq!(k.sub_kind, "sensor-data");
        assert_eq!(k.clone(), k);
    }

    #[test]
    fn passive_solver_has_safe_defaults() {
        let s = PassiveSolver;
        assert_eq!(s.name(), "passive");
        // detect() / solved_signal() / relax_response_csp() all default
        // to "not applicable" so a passive solver doesn't interfere
        // with the engine's normal flow.
        let mut headers: Vec<(String, String)> = Vec::new();
        s.prepare_request("example.com", &mut headers);
        assert!(headers.is_empty());
        assert_eq!(s.solved_signal("foo=bar", "<html></html>"), false);
    }

    #[test]
    fn solver_object_safety() {
        // Trait must be object-safe so Page/HttpClient can hold
        // `Arc<dyn ChallengeSolver>` for runtime polymorphism.
        let _v: Vec<std::sync::Arc<dyn ChallengeSolver>> = vec![std::sync::Arc::new(PassiveSolver)];
    }
}
