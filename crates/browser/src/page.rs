use crate::iframe;
use crate::script_runner;
use crate::stylesheet_collector;
use akamai;
use dom::Dom;
use event_loop::{BrowserEventLoop, IdleReason};
use js_runtime::{runtime::BrowserRuntimeOptions, BrowserJsRuntime};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use stealth;
use tracing;

/// Whether a URL is a "secure context" per WICG/secure-contexts §3.2.
/// Secure: https, wss, file, plus http://localhost / http://127.0.0.1 /
/// http://[::1] / *.localhost loopback exceptions. Drives `isSecureContext`
/// and gates the ~18 secure-context-only Web Platform APIs (Phase 7 fix —
/// see `docs/PHASE7_AB_PROBE_FINDINGS_2026_04_29.md`).
pub(crate) fn is_secure_url(url: &str) -> bool {
    let parsed = match url::Url::parse(url) {
        Ok(u) => u,
        Err(_) => return false,
    };
    match parsed.scheme() {
        "https" | "wss" | "file" => true,
        "http" | "ws" => match parsed.host_str() {
            Some(h) => {
                h == "localhost"
                    || h.ends_with(".localhost")
                    || h == "127.0.0.1"
                    || h == "[::1]"
                    || h == "::1"
            }
            None => false,
        },
        _ => false, // about:, data:, blob:, javascript:, etc. — insecure
    }
}

#[cfg(test)]
mod is_secure_url_tests {
    use super::is_secure_url;
    #[test]
    fn classifies_schemes() {
        assert!(is_secure_url("https://example.com/"));
        assert!(is_secure_url("wss://example.com/"));
        assert!(is_secure_url("file:///etc/hosts"));
        assert!(is_secure_url("http://localhost:3000/"));
        assert!(is_secure_url("http://127.0.0.1/"));
        assert!(is_secure_url("http://my-app.localhost/"));
        assert!(!is_secure_url("http://example.com/"));
        assert!(!is_secure_url("data:text/html,<p>x"));
        assert!(!is_secure_url("about:blank"));
        assert!(!is_secure_url("blob:https://example.com/x"));
        assert!(!is_secure_url("javascript:void(0)"));
    }
}

/// RAII guard that fires `terminate_execution` on the V8 isolate after
/// a deadline expires, unless dropped first. Used by `navigate_with_init`
/// to bound how long any single iteration can spin in CPU-bound JS — for
/// sites like delta.com or taobao.com whose JS does not yield to tokio.
///
/// The `Drop` impl signals the watcher thread to exit cleanly (no
/// terminate fires). If the deadline elapses before drop, terminate
/// fires and the next `execute_script` call returns the
/// "Uncaught Error: execution terminated" exception — caller is
/// responsible for catching that and bailing out of the iteration.
struct V8DeadlineWatcher {
    cancel: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl V8DeadlineWatcher {
    fn new(isolate: deno_core::v8::IsolateHandle, deadline: Duration) -> Self {
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_clone = cancel.clone();
        let handle = std::thread::spawn(move || {
            let start = std::time::Instant::now();
            // Poll the cancel flag at 100 ms granularity so drop is fast.
            while start.elapsed() < deadline {
                if cancel_clone.load(Ordering::Relaxed) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            if !cancel_clone.load(Ordering::Relaxed) {
                eprintln!(
                    "[V8DeadlineWatcher] deadline {}ms expired — firing terminate_execution",
                    deadline.as_millis()
                );
                let ok = isolate.terminate_execution();
                eprintln!("[V8DeadlineWatcher] terminate_execution returned {}", ok);
            }
        });
        Self {
            cancel,
            handle: Some(handle),
        }
    }
}

impl Drop for V8DeadlineWatcher {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            // Best-effort join — watcher polls every 100 ms so this returns fast.
            let _ = h.join();
        }
    }
}

/// Phase 0 measurement-hygiene typed page outcome.
///
/// Replaces the old bare boolean "blocked?" guess. Every navigated
/// site resolves to exactly one of these so the re-baseline can tell a
/// genuine challenge apart from a render-incomplete false positive
/// (the cheap "wins" with zero stealth work).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChallengeVerdict {
    /// Real content rendered, no structural challenge markers.
    Pass,
    /// No challenge markers but a thin/empty stub or an SPA shell that
    /// never populated — a render-completeness issue, NOT a stealth
    /// failure (e.g. mail-ru redirect, captcha-shell, washingtonpost-
    /// style multi-MB body that the old classifier over-matched).
    RenderIncomplete,
    /// Challenge marker + small body: an explicit challenge/deny page
    /// (Kasada 756 B 429, Akamai sec-cpt 2.6 KB PoW, Cloudflare managed
    /// challenge, DataDome interstitial) was served before our JS could
    /// earn trust — edge / interstitial class.
    EdgeBlock,
    /// Challenge marker + a large body that otherwise rendered: the
    /// vendor JS ran and the sensor scored the telemetry as bot.
    SensorFail,
}

impl ChallengeVerdict {
    /// True for every served-challenge outcome — the exact semantics of
    /// the old boolean `is_anti_bot_challenge`.
    pub fn is_challenge(self) -> bool {
        matches!(self, Self::EdgeBlock | Self::SensorFail)
    }

    /// Stable lowercase tag for JSON/audit output.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::RenderIncomplete => "render-incomplete",
            Self::EdgeBlock => "edge-block",
            Self::SensorFail => "sensor-fail",
        }
    }
}

/// Shared structural anti-bot challenge classifier.
///
/// FP-B1: this is now a thin delegate to the single canonical classifier
/// [`crate::classify::engine_classify`] so `page.rs`, `holistic_sweep`,
/// and the audit harness can never disagree about the same body again.
/// The per-marker / per-gate logic (and its rationale — weak markers
/// only count stub-sized, etc.) lives there as the single source of
/// truth; this wrapper keeps the four navigate-loop call sites stable.
fn body_has_challenge_marker(body: &str) -> bool {
    crate::classify::engine_classify(body).verdict.is_challenge()
}

/// A browser page. Owns a DOM, JS runtime, and event loop.
///
/// # Example
/// ```rust,ignore
/// let page = Page::from_html("<html><body><script>document.title = 'Hello'</script></body></html>", None::<stealth::StealthProfile>).await?;
/// assert_eq!(page.title(), "Hello");
/// ```
pub struct Page {
    // Children hold V8 isolates created after parent — must drop first
    children: Vec<iframe::ChildIframe>,
    event_loop: BrowserEventLoop,
    url: String,
}

impl Drop for Page {
    fn drop(&mut self) {
        // Drop children (newer isolates) before parent (older isolate)
        // V8 requires reverse drop order
        while self.children.pop().is_some() {}
    }
}

impl Page {
    /// Simulate a user switching to another tab and then coming back.
    /// This defeats macro-behavioral heuristics that flag sessions
    /// without visibility/focus changes as automated.
    pub async fn simulate_tab_switch(&mut self) -> Result<(), deno_core::error::AnyError> {
        let code = r#"
            (function() {
                // 1. Blur the window (user clicked away)
                window.dispatchEvent(new Event('blur', { bubbles: false, cancelable: false }));
                document.hasFocus = () => false;

                // 2. Hide the document (tab backgrounded)
                Object.defineProperty(document, 'visibilityState', { value: 'hidden', configurable: true });
                Object.defineProperty(document, 'hidden', { value: true, configurable: true });
                document.dispatchEvent(new Event('visibilitychange', { bubbles: true, cancelable: false }));
            })();
        "#;
        self.event_loop.execute_script(code)?;

        // Sleep for a random amount of time to simulate reading another tab (e.g., 2-5 seconds)
        // For testing we keep it short, but in a real scraper this would be realistic.
        tokio::time::sleep(std::time::Duration::from_millis(2500)).await;

        let code_focus = r#"
            (function() {
                // 3. Show the document (tab foregrounded)
                Object.defineProperty(document, 'visibilityState', { value: 'visible', configurable: true });
                Object.defineProperty(document, 'hidden', { value: false, configurable: true });
                document.dispatchEvent(new Event('visibilitychange', { bubbles: true, cancelable: false }));

                // 4. Focus the window
                document.hasFocus = () => true;
                window.dispatchEvent(new Event('focus', { bubbles: false, cancelable: false }));
            })();
        "#;
        self.event_loop.execute_script(code_focus)?;
        Ok(())
    }

    /// Detect if the current page is an anti-bot challenge (Kasada, Akamai, etc.)
    ///
    /// Boolean drop-in retained for the four navigate-loop call sites.
    /// Delegates to the shared structural classifier so the
    /// false-positive tightening (Phase 0 measurement hygiene) applies
    /// everywhere.
    pub fn is_anti_bot_challenge(&mut self) -> bool {
        let body = self.content();
        body_has_challenge_marker(&body)
    }

    /// Phase 0 measurement hygiene — typed page outcome.
    ///
    /// Every navigated site gets exactly one [`ChallengeVerdict`] so a
    /// "blocked" verdict is no longer a bare substring guess. This is
    /// what the `audit_failing_sites` re-baseline consumes to separate
    /// genuine challenges from render-incomplete false positives.
    pub fn challenge_verdict(&mut self) -> ChallengeVerdict {
        // FP-B1: derived from the single canonical classifier so the
        // audit harness verdict and the holistic-sweep tag are computed
        // from the identical marker/gate pass (no more pass↔block
        // disagreement between call sites). The edge-vs-sensor split and
        // the thin band live in `crate::classify` as named constants.
        crate::classify::engine_classify(&self.content()).verdict
    }

    /// W7 / Cloudflare V1 — orchestrator-runner scaffolding.
    ///
    /// On the post-build page, detect a Cloudflare Managed/JS Challenge from
    /// the response body (the `_cf_chl_opt` inline blob). When detected:
    ///   1. Log a one-line telemetry summary (kind, ray, zone, orchestrator URL).
    ///   2. Drive the event loop forward in 200 ms ticks for up to ~10 s,
    ///      polling for a `cf_clearance` cookie or for the orchestrator
    ///      script to set a `__pendingNavigation` (the orchestrator typically
    ///      either 302s or assigns `location.href` after issuing clearance).
    ///   3. Inject low-rate behavioural noise (mousemove/scroll) so the
    ///      Phase-1 "25 events" gate in the legacy IUAM path is satisfied
    ///      if the orchestrator is still listening for it.
    ///
    /// V1 explicitly does **not** attempt to deobfuscate or hand-solve the
    /// PoW/Turnstile payload. Per `docs/RESEARCH_CLOUDFLARE_BYPASS_2026_05_10.md`
    /// §0/§9, the recommended approach is to run the orchestrator JS to
    /// completion in our V8 + DOM and let it negotiate clearance natively.
    /// Returns `Some(ctx)` with the parsed challenge context iff a CF
    /// challenge was detected (regardless of solve outcome).
    pub async fn handle_cloudflare_flow(
        &mut self,
        client: &net::HttpClient,
    ) -> Option<stealth::cloudflare::CfChallengeContext> {
        let body = self.content();
        // Build a minimal headers map from what we have on the live page.
        // The body+server signal alone is enough for our detector when the
        // orchestrator's inline blob is present (most common case).
        let mut hdrs: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        // We don't have the response headers here, but the body marker is
        // a strong enough signal — synthesize `server: cloudflare` so our
        // body-fallback path fires. Safe: detect_challenge requires either
        // the canonical header OR (body marker AND server: cloudflare).
        if body.contains("_cf_chl_opt") || body.contains("/cdn-cgi/challenge-platform/") {
            hdrs.insert("server".into(), "cloudflare".into());
        }

        let ctx = stealth::cloudflare::detect_challenge(&hdrs, &body)?;
        eprintln!("[cloudflare] {}", ctx.summary());
        tracing::info!(
            kind = ?ctx.kind,
            ray = %ctx.ray,
            zone = %ctx.zone,
            orchestrator = %ctx.orchestrator_url,
            "cloudflare challenge detected"
        );

        // Fast-fail interactive Turnstile — V1 cannot solve it without a
        // captcha service plug-in.
        if !ctx.kind.v1_solvable() {
            eprintln!(
                "[cloudflare] kind {:?} is not V1-solvable — skipping orchestrator runner",
                ctx.kind
            );
            return Some(ctx);
        }

        // Check the cookie jar for a pre-existing cf_clearance — if present
        // this navigation already had clearance attached and we're done.
        let host = url::Url::parse(&self.url)
            .ok()
            .and_then(|u| u.host_str().map(str::to_string))
            .unwrap_or_default();
        let parsed = url::Url::parse(&self.url).ok();
        if let Some(p) = parsed.as_ref() {
            if let Some(cookies) = client.cookies_for_url(p).await {
                if cookies.contains("cf_clearance=") {
                    eprintln!(
                        "[cloudflare] cf_clearance already in jar for {} — orchestrator likely succeeded",
                        host
                    );
                    return Some(ctx);
                }
            }
        }

        // Inject minimal behavioural noise. Real Chrome typically fires 5-25
        // mousemove/scroll/keyup events in the few seconds the orchestrator
        // is alive; the exact distribution doesn't matter for V1 — what
        // matters is `event.isTrusted === false` doesn't get used to grade
        // us (CF reportedly ignores untrusted events but the *count* may
        // still be probed by the legacy phase-1 collector).
        let noise_js = r#"
            (function() {
                try {
                    let i = 0;
                    const fire = () => {
                        if (i++ > 30) return;
                        try {
                            window.dispatchEvent(new MouseEvent('mousemove', {
                                clientX: 100 + (i * 7) % 400,
                                clientY: 100 + (i * 11) % 300,
                                bubbles: true,
                            }));
                            if (i % 5 === 0) {
                                window.dispatchEvent(new Event('scroll', { bubbles: true }));
                            }
                            if (i % 7 === 0) {
                                window.dispatchEvent(new KeyboardEvent('keyup', {
                                    key: 'Tab', bubbles: true,
                                }));
                            }
                        } catch (_) {}
                        setTimeout(fire, 80 + (i * 13) % 60);
                    };
                    setTimeout(fire, 50);
                } catch (_) {}
            })();
        "#;
        let _ = self.event_loop.execute_script(noise_js);

        // Drive the event loop in short bursts and check for clearance.
        // 10 s budget total — Managed Challenge typically completes in 2-6 s
        // for a fingerprint-correct browser.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut cleared = false;
        while std::time::Instant::now() < deadline {
            let _ = self
                .event_loop
                .run_until_idle(std::time::Duration::from_millis(250))
                .await;
            if let Some(p) = parsed.as_ref() {
                if let Some(cookies) = client.cookies_for_url(p).await {
                    if cookies.contains("cf_clearance=") {
                        cleared = true;
                        break;
                    }
                }
            }
            // Also short-circuit if the orchestrator queued a navigation —
            // the outer loop will pick it up on its own.
            let pending = self
                .event_loop
                .execute_script(
                    "globalThis._boxide && globalThis._boxide.__pendingNavigation ? '1' : ''",
                )
                .unwrap_or_default();
            if pending.trim() == "1" {
                eprintln!(
                    "[cloudflare] orchestrator queued __pendingNavigation — yielding to outer loop"
                );
                break;
            }
        }
        if cleared {
            eprintln!("[cloudflare] cf_clearance issued — outer loop will retry");
        } else {
            eprintln!("[cloudflare] orchestrator did not produce cf_clearance within budget — V2 work needed");
        }

        Some(ctx)
    }

    /// T3A-A5: Autonomous Akamai bypass. Detects `_abck` challenge state,
    /// drains behavioural events (mouse/key) from JS, and POSTs sensor_data
    /// via the provided client. Returns the new trust state.
    pub async fn handle_akamai_flow(
        &mut self,
        client: &net::HttpClient,
    ) -> Result<akamai::AbckState, deno_core::error::AnyError> {
        let host = match url::Url::parse(&self.url) {
            Ok(u) => u.host_str().unwrap_or("").to_string(),
            Err(_) => return Ok(akamai::AbckState::Unknown),
        };

        // sec-cpt guard (evidence-backed, homedepot nav-trace
        // 2026-05-15, doc 20): when the page is an Akamai sec-cpt PoW
        // interstitial (the `<div id="sec-if-cpt-container">` bundle),
        // the BMP `sensor_data` flow below POSTs the WRONG payload type
        // to the sec-cpt verify endpoint — Akamai 201s but never clears
        // `_abck`, looping forever. The sec-cpt bundle runs its own
        // challenge JS; our BMP POST only adds rejected, conflicting
        // traffic. Skip the BMP sensor path here and let the bundle's
        // own flow be the sole actor. Plain BMP pages (no sec-cpt
        // container) are unaffected — bestbuy still flips Favorable.
        {
            let body = self.content();
            if body.contains("sec-if-cpt-container") || body.contains("sec-cpt-if") {
                eprintln!(
                    "[akamai] sec-cpt interstitial detected for {host} — skipping BMP sensor_data POST (wrong payload for sec-cpt verify endpoint; bundle self-solves)"
                );
                return Ok(akamai::AbckState::NeedsSecCpt);
            }
        }

        // 1. Detect tenant config. Try static registry first (fast path
        //    for known bestbuy / homedepot), then fall back to parsing
        //    the rendered HTML via akamai::parse_tenant_from_html (W2.2)
        //    so we can auto-discover new Akamai tenants (macys, hotels,
        //    etc.) without code changes.
        //
        //    A host with neither registry hit nor HTML-parsed tenant is
        //    treated as not-Bot-Manager-protected — return Unknown so
        //    we don't spuriously POST to /akam/13/sensor_data 404 (the
        //    failure mode that burned the aborted mid-sweep run).
        let static_settings = akamai::get_tenant_settings(&host);
        let parsed_owned;
        let settings_ref: &akamai::TenantSettings = match static_settings.as_ref() {
            Some(s) => s,
            None => {
                let html = self.content();
                match akamai::parse_tenant_from_html(&html) {
                    Some(pt) => {
                        eprintln!(
                            "[akamai] discovered tenant for {host}: seed={} sensor_path={}",
                            pt.tenant_seed, pt.sensor_post_path
                        );
                        parsed_owned = akamai::TenantSettings {
                            tenant_seed: pt.tenant_seed,
                            // Box::leak the dynamic path so the
                            // existing &'static str API is preserved
                            // without a broader refactor; the leak is
                            // per-host (small, bounded by sites we hit)
                            // and the leaked memory is alive for the
                            // process lifetime anyway since the static
                            // registry strings are already &'static.
                            post_path: Box::leak(pt.sensor_post_path.into_boxed_str()),
                        };
                        &parsed_owned
                    }
                    None => return Ok(akamai::AbckState::Unknown),
                }
            }
        };
        // Re-borrow as owned for downstream usage.
        let settings = akamai::TenantSettings {
            tenant_seed: settings_ref.tenant_seed,
            post_path: settings_ref.post_path,
        };

        // 2. Check if we actually NEED to send sensor_data.
        // Only POST when Akamai EXPLICITLY signals NeedsSensor (the
        // ~0~-1~ suffix). Defaulting to NeedsSensor on Unknown caused
        // spurious POSTs on Kasada-only sites where Kasada's edge sets
        // _abck without the standard Akamai trust suffix — the POST then
        // got Kasada-intercepted and returned 429 + interstitial HTML,
        // which we mis-attributed to Akamai bot scoring. NeedsSecCpt and
        // NeedsPixel are also "out of scope" per AbckState docs and
        // shouldn't trigger a sensor_data POST either.
        let current_state = client
            .akamai_sessions
            .abck_state(&host)
            .await
            .unwrap_or(akamai::AbckState::Unknown);

        if current_state != akamai::AbckState::NeedsSensor {
            return Ok(current_state);
        }

        // W2.3 landed: build_sensor_data now routes through build_v3 with
        // session.bm_sz-derived shuffle/substitute seeds.
        //
        // POST count tuning history (all measurements within ±8 union
        // variance noise floor per W4.3 characterization):
        //   N=1 (1st run): 121 union (115/116/117/113)
        //   N=8 retry:     120 union (117/116/117/112)
        //   N=1 (2nd run): 119 union (114/116/113/112)
        //
        // Across runs the union variance is ±1 around 120. N=1 keeps
        // network traffic per Akamai site to a single POST (clean
        // single-shot behavior, lower detection volume) and lands on
        // the upper edge of the variance band more often than N=8 in
        // sample-of-three. Defaulting N=1.
        //
        // Keep the loop structure intact so MAX_POSTS becomes a single-
        // line toggle if a future envelope improvement makes multi-POST
        // credible again (e.g., per-attempt bm_sz refresh, dynamic key
        // count accumulation between iterations).
        const MAX_POSTS: u32 = 1;
        const POST_GAP_MS: u64 = 500;
        let mut last_state = akamai::AbckState::NeedsSensor;
        for attempt in 0..MAX_POSTS {
            // Drain behavioural events from the page (fresh per attempt).
            let events_json = self
                .event_loop
                .execute_script(akamai::DRAIN_JS)
                .unwrap_or_default();
            let drained = akamai::parse_drained(&events_json);

            let new_state = client
                .send_akamai_sensor_data(
                    &host,
                    &self.url,
                    settings.post_path,
                    settings.tenant_seed,
                    drained,
                )
                .await
                .map_err(|e| deno_core::error::AnyError::msg(e.to_string()))?;
            last_state = new_state;

            eprintln!(
                "[akamai] {host} POST attempt {}/{MAX_POSTS} → {new_state:?}",
                attempt + 1
            );

            match new_state {
                akamai::AbckState::Favorable | akamai::AbckState::Invalidated => break,
                akamai::AbckState::NeedsSensor => {
                    if attempt + 1 < MAX_POSTS {
                        tokio::time::sleep(std::time::Duration::from_millis(POST_GAP_MS)).await;
                    }
                }
                _ => break,
            }
        }
        Ok(last_state)
    }
    /// Create a page from an HTML string. Parses HTML, executes inline scripts,
    /// and runs the event loop until idle (or 30s timeout).
    pub async fn from_html(
        html: &str,
        profile: Option<stealth::StealthProfile>,
    ) -> Result<Self, deno_core::error::AnyError> {
        Self::from_html_with_url(html, "about:blank", profile).await
    }

    /// Create a page quickly — parses HTML, sets up DOM + JS runtime, executes
    /// inline scripts, but does NOT drain the event loop. Useful for CDP
    /// navigation where the caller controls script execution via Runtime.evaluate.
    pub async fn from_html_fast(
        html: &str,
        url: &str,
        profile: stealth::StealthProfile,
    ) -> Result<Self, deno_core::error::AnyError> {
        let dom = html_parser::parse_html(html);
        let scripts = script_runner::find_scripts(&dom);
        let stylesheet_entries = stylesheet_collector::find_stylesheets(&dom);
        let stylesheets = stylesheet_collector::resolve_inline_only(&stylesheet_entries);

        let runtime = BrowserJsRuntime::with_options(
            dom,
            BrowserRuntimeOptions {
                stealth_profile: Some(profile.clone()),
                stylesheets,
                is_secure_context: is_secure_url(url),
                ..Default::default()
            },
        );
        let mut event_loop = BrowserEventLoop::new(runtime);

        // Set location.href (URL-state setup, not a real navigation —
        // reset the nav-pending signal afterward so subsequent
        // run_until_idle calls don't short-circuit).
        let url_js = url.replace('\\', "\\\\").replace('\'', "\\'");
        event_loop
            .execute_script(&format!("location.href = '{}';", url_js))
            .ok();
        event_loop.reset_nav_pending();

        // Share the HTTP client with JS fetch()
        let client = net::HttpClient::new(&profile)
            .map_err(|e| deno_core::error::AnyError::msg(e.to_string()))?;
        js_runtime::extensions::fetch_ext::set_fetch_client(client.clone());

        // Execute inline scripts (fast mode skips external scripts by default)
        for (i, script) in scripts.iter().enumerate() {
            if script.src.is_some() {
                continue;
            }
            if !script.code.is_empty() {
                // W2.7 — name inline scripts with document URL (Chrome
                // parity) instead of letting V8 default to <anonymous>.
                if let Err(e) = event_loop.execute_script_with_name(&script.code, url) {
                    tracing::warn!(script_index = i, error = %e, "Script error in inline script");
                }
            }
        }

        Ok(Self {
            event_loop,
            url: url.to_string(),
            children: Vec::new(),
        })
    }

    /// Replace the page's content with new HTML, reusing the V8 isolate.
    /// Much faster than creating a new Page (~2ms vs ~17ms) since it skips
    /// V8 isolate creation and bootstrap script execution.
    pub fn reload_html(&mut self, html: &str, url: &str) {
        let dom = html_parser::parse_html(html);
        let scripts = script_runner::find_scripts(&dom);
        let stylesheet_entries = stylesheet_collector::find_stylesheets(&dom);
        let stylesheets = stylesheet_collector::resolve_inline_only(&stylesheet_entries);

        // Swap DOM in existing runtime (no new V8 isolate needed)
        self.event_loop.runtime_mut().replace_dom(dom, stylesheets);

        // Drop old iframe children
        self.children.clear();

        // Update URL (URL-state setup, not a real navigation).
        self.url = url.to_string();
        let url_js = url.replace('\\', "\\\\").replace('\'', "\\'");
        self.event_loop
            .execute_script(&format!("location.href = '{}';", url_js))
            .ok();
        self.event_loop.reset_nav_pending();

        // Execute inline scripts in document order
        for (i, script) in scripts.iter().enumerate() {
            if script.src.is_some() {
                continue; // skip external scripts — caller handles fetching
            }
            if script.code.trim().is_empty() {
                continue;
            }
            // W2.7 — Chrome parity: inline scripts report the document URL.
            if let Err(e) = self
                .event_loop
                .execute_script_with_name(&script.code, &self.url)
            {
                tracing::warn!(script_index = i, error = %e, "Script error in inline script");
            }
        }
    }

    /// Create a page with a specific URL.
    pub async fn from_html_with_url(
        html: &str,
        url: &str,
        profile: Option<stealth::StealthProfile>,
    ) -> Result<Self, deno_core::error::AnyError> {
        let dom = html_parser::parse_html(html);

        // Install CSP from any meta-tags present in the HTML (this code
        // path is for tests / synthetic HTML — there are no response
        // headers to merge from). The OnceLock-backed enforcement
        // applies to script-fetches issued below.
        {
            let policy_set = crate::csp_collector::collect_csp(&[], &dom);
            let enforce_csp = profile.as_ref().map(|p| p.enforce_csp).unwrap_or(true)
                && std::env::var("BOXIDE_CSP_BYPASS").is_err();
            if !policy_set.is_empty() {
                if let Ok(origin) = url::Url::parse(url) {
                    js_runtime::extensions::fetch_ext::set_csp_policy(
                        std::sync::Arc::new(policy_set),
                        origin,
                        enforce_csp,
                    );
                } else {
                    js_runtime::extensions::fetch_ext::clear_csp_policy();
                }
            } else {
                // No policy on this page — clear any leftover state from a
                // previous Page in the same process so site A's CSP can't
                // leak into site B in test runners.
                js_runtime::extensions::fetch_ext::clear_csp_policy();
            }
        }

        // Find scripts and stylesheets before handing DOM to runtime
        let scripts = script_runner::find_scripts(&dom);
        let stylesheet_entries = stylesheet_collector::find_stylesheets(&dom);
        let stylesheets = stylesheet_collector::resolve_inline_only(&stylesheet_entries);

        let runtime = BrowserJsRuntime::with_options(
            dom,
            BrowserRuntimeOptions {
                stealth_profile: profile.clone(),
                stylesheets,
                is_secure_context: is_secure_url(url),
                ..Default::default()
            },
        );
        let mut event_loop = BrowserEventLoop::new(runtime);

        // Set location.href (URL-state setup, not a real navigation).
        let url_js = url.replace('\\', "\\\\").replace('\'', "\\'");
        event_loop
            .execute_script(&format!("location.href = '{}';", url_js))
            .ok();
        event_loop.reset_nav_pending();

        // Share the HTTP client with JS fetch()
        let p = profile.unwrap_or_else(stealth::presets::chrome_130_ru);
        let client =
            net::HttpClient::new(&p).map_err(|e| deno_core::error::AnyError::msg(e.to_string()))?;
        js_runtime::extensions::fetch_ext::set_fetch_client(client.clone());

        // Execute scripts in document order
        for (i, script) in scripts.iter().enumerate() {
            if let Some(src) = &script.src {
                if let Some(full_url) = Self::resolve_url(url, src) {
                    // CSP gate — same enforcement point as the parallel
                    // pre-fetch path in `build_page_with_scripts_init_and_storage`.
                    if let Ok(parsed_url) = url::Url::parse(&full_url) {
                        if let Err(violated) = js_runtime::extensions::fetch_ext::check_csp(
                            net::csp::Directive::ScriptSrcElem,
                            &parsed_url,
                            script.nonce.as_deref(),
                            true,
                        ) {
                            eprintln!(
                                "[csp] Refused to load the script '{}' because it violates the following Content Security Policy directive: \"{}\".",
                                full_url, violated
                            );
                            continue;
                        }
                    }
                    match client.get_follow(&full_url, 10).await {
                        Ok(resp) => {
                            let code = resp.text();
                            if let Err(e) = event_loop.execute_script(&code) {
                                tracing::warn!(script_src = %src, error = %e, "Script error in external script");
                            }
                        }
                        Err(e) => {
                            tracing::warn!(script_src = %src, error = %e, "Failed to fetch script")
                        }
                    }
                }
            } else if !script.code.is_empty() {
                if let Err(e) = event_loop.execute_script(&script.code) {
                    tracing::warn!(script_index = i, error = %e, "Script error in inline script");
                }
            }
        }

        // Set document.readyState = loading
        // Non-enumerable own property so it doesn't leak into
        // Object.keys(window) — defined here so subsequent
        // `globalThis.__boxide.__documentReadyState = ...` assignments preserve
        // enumerable=false (writable=true, descriptor inherited).
        event_loop
            .execute_script("globalThis._boxide.__documentReadyState = 'loading';")
            .ok();

        // Fire DOMContentLoaded and load events — many scripts wait for these
        event_loop
            .execute_script(
                "document.dispatchEvent(new Event('DOMContentLoaded', {bubbles: true}));",
            )
            .ok();

        // After DOMContentLoaded, readyState = interactive
        event_loop
            .execute_script("globalThis._boxide.__documentReadyState = 'interactive';")
            .ok();

        event_loop
            .execute_script("window.dispatchEvent(new Event('load'));")
            .ok();

        // After load, readyState = complete
        event_loop
            .execute_script("globalThis._boxide.__documentReadyState = 'complete';")
            .ok();

        // Run event loop until idle, capped at 8s. Real Chrome treats a page
        // as ready well before all background timers settle — analytics RUM
        // beacons + setInterval polling otherwise prevent "idle" indefinitely.
        // 8s comfortably covers Kasada PoW (<1s), CF turnstile (2-4s), and
        // most JS-heavy first-paint flows.
        event_loop.run_until_idle(Duration::from_secs(8)).await?;

        // Process <iframe srcdoc="..."> elements
        // Parse srcdoc HTML and execute scripts within an isolated scope
        let iframes = {
            let dom_ref = event_loop.runtime_mut().inner();
            let state = dom_ref.op_state();
            let state = state.borrow();
            let dom_state = state.borrow::<js_runtime::state::DomState>();
            iframe::find_iframes(&dom_state.dom)
        };
        for iframe_info in &iframes {
            if let Some(srcdoc) = &iframe_info.srcdoc {
                // Execute srcdoc scripts in an isolated function scope
                let node_id = iframe_info.node_id.to_raw();
                let escaped = srcdoc.replace('\\', "\\\\").replace('`', "\\`");
                let setup_js = format!(
                    r#"(() => {{
                        const _iframeEl = (() => {{
                            const nodeId = {node_id};
                            // Find iframe element and set up its contentDocument
                            const el = document.querySelectorAll('iframe')[0]; // simplified
                            if (el && el.contentWindow) {{
                                el.contentWindow._srcdocLoaded = true;
                            }}
                        }})();
                    }})()"#,
                );
                event_loop.execute_script(&setup_js).ok();
            }
        }

        // Create child Pages for iframes with srcdoc
        let mut children = Vec::new();
        let iframes = {
            let dom_ref = event_loop.runtime_mut().inner();
            let state = dom_ref.op_state();
            let state = state.borrow();
            let dom_state = state.borrow::<js_runtime::state::DomState>();
            iframe::find_iframes(&dom_state.dom)
        };
        for info in &iframes {
            if let Some(srcdoc) = &info.srcdoc {
                match iframe::ChildIframe::from_srcdoc(info.node_id, srcdoc, &p).await {
                    Ok(child) => children.push(child),
                    Err(e) => tracing::warn!(error = %e, "iframe srcdoc error"),
                }
            }
        }

        Ok(Self {
            event_loop,
            url: url.to_string(),
            children,
        })
    }

    /// Get a child iframe by index.
    pub fn child_iframe(&mut self, index: usize) -> Option<&mut iframe::ChildIframe> {
        self.children.get_mut(index)
    }

    /// Get the number of child iframes.
    pub fn child_iframe_count(&self) -> usize {
        self.children.len()
    }

    /// Evaluate arbitrary JavaScript and return the result as a string.
    pub fn evaluate(&mut self, js: &str) -> Result<String, deno_core::error::AnyError> {
        self.event_loop.execute_script(js)
    }

    /// Run scripts and wait for completion.
    pub async fn evaluate_async(
        &mut self,
        js: &str,
        timeout: Duration,
    ) -> Result<IdleReason, deno_core::error::AnyError> {
        self.event_loop.execute_and_run(js, timeout).await
    }

    /// Get the page title (document.title).
    pub fn title(&mut self) -> String {
        self.evaluate("document.title").unwrap_or_default()
    }

    /// Get the full HTML content of the page.
    pub fn content(&mut self) -> String {
        self.evaluate("document.documentElement.outerHTML")
            .unwrap_or_default()
    }

    /// Get text content of the body.
    pub fn text_content(&mut self) -> String {
        self.evaluate("document.body ? document.body.textContent : ''")
            .unwrap_or_default()
    }

    /// Get text content of an element matching a selector.
    pub fn text_of(&mut self, selector: &str) -> Option<String> {
        let sel = selector.replace('\\', "\\\\").replace('"', "\\\"");
        let result = self
            .evaluate(&format!(
                r#"(() => {{ const el = document.querySelector("{}"); return el ? el.textContent : ""; }})()"#,
                sel
            ))
            .ok()?;
        if result.is_empty() {
            None
        } else {
            Some(result)
        }
    }

    /// Check if an element exists.
    pub fn has_element(&mut self, selector: &str) -> bool {
        let sel = selector.replace('\\', "\\\\").replace('"', "\\\"");
        self.evaluate(&format!(r#"document.querySelector("{}") !== null"#, sel))
            .map(|r| r == "true")
            .unwrap_or(false)
    }

    /// Simulate a human-like mouse click on a CSS selector.
    /// Generates a Bezier curve mouse path, dispatches mousemove events along
    /// the path, then mousedown+mouseup+click at the target.
    pub fn human_click(&mut self, selector: &str) -> Result<String, deno_core::error::AnyError> {
        let sel = selector.replace('\\', "\\\\").replace('"', "\\\"");
        self.evaluate(&format!(r#"
            (() => {{
                const el = document.querySelector("{}");
                if (!el) return "element not found";
                const rect = el.getBoundingClientRect ? el.getBoundingClientRect() : {{x:0,y:0,width:100,height:30}};
                const tx = rect.x + rect.width / 2;
                const ty = rect.y + rect.height / 2;
                const path = __browserOxide.humanMousePath(0, 0, tx, ty, 15);
                for (const p of path) {{
                    el.dispatchEvent(new MouseEvent('mousemove', {{clientX: p.x, clientY: p.y, bubbles: true}}));
                }}
                el.dispatchEvent(new MouseEvent('mousedown', {{clientX: tx, clientY: ty, bubbles: true, button: 0}}));
                el.dispatchEvent(new MouseEvent('mouseup', {{clientX: tx, clientY: ty, bubbles: true, button: 0}}));
                el.dispatchEvent(new MouseEvent('click', {{clientX: tx, clientY: ty, bubbles: true, button: 0}}));
                el.click && el.click();
                return "clicked";
            }})()
        "#, sel))
    }

    /// Simulate human-like typing into a CSS selector (input/textarea).
    /// Uses variable inter-key timing based on character pairs.
    pub fn human_type(
        &mut self,
        selector: &str,
        text: &str,
    ) -> Result<String, deno_core::error::AnyError> {
        let sel = selector.replace('\\', "\\\\").replace('"', "\\\"");
        let text_escaped = text.replace('\\', "\\\\").replace('"', "\\\"");
        self.evaluate(&format!(
            r#"
            (() => {{
                const el = document.querySelector("{}");
                if (!el) return "element not found";
                el.focus && el.focus();
                const text = "{}";
                const delays = __browserOxide.humanTypingDelays(text, 65);
                for (let i = 0; i < text.length; i++) {{
                    const ch = text[i];
                    el.dispatchEvent(new KeyboardEvent('keydown', {{key: ch, bubbles: true}}));
                    el.dispatchEvent(new KeyboardEvent('keypress', {{key: ch, bubbles: true}}));
                    if (el.value !== undefined) el.value += ch;
                    el.dispatchEvent(new KeyboardEvent('keyup', {{key: ch, bubbles: true}}));
                    el.dispatchEvent(new Event('input', {{bubbles: true}}));
                }}
                return "typed " + text.length + " chars";
            }})()
        "#,
            sel, text_escaped
        ))
    }

    /// Get the page URL.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Get the event loop (for advanced control).
    pub fn event_loop(&mut self) -> &mut BrowserEventLoop {
        &mut self.event_loop
    }

    /// Create a page with a stealth profile.
    pub async fn with_profile(
        html: &str,
        url: &str,
        profile: stealth::StealthProfile,
    ) -> Result<Self, deno_core::error::AnyError> {
        let client = net::HttpClient::new(&profile)
            .map_err(|e| deno_core::error::AnyError::msg(e.to_string()))?;
        Self::build_page_with_scripts_and_init(html, url, &profile, &client, &[]).await
    }

    pub fn consume_and_print_logs(&mut self) {
        let logs = {
            let runtime = self.event_loop.runtime_mut().inner();
            let state = runtime.op_state();
            let mut state = state.borrow_mut();
            let dom_state = state.borrow_mut::<js_runtime::state::DomState>();
            std::mem::take(&mut dom_state.console_output)
        };
        for log in logs {
            let prefix = match log.level {
                js_runtime::state::ConsoleLevel::Log => "[JS LOG]",
                js_runtime::state::ConsoleLevel::Warn => "[JS WARN]",
                js_runtime::state::ConsoleLevel::Error => "[JS ERROR]",
                _ => "[JS INFO]",
            };
            println!("    {} {}", prefix, log.args.join(" "));
        }
    }

    /// Navigate to a URL using an HTTP client (real network request).
    /// Simple single-GET helper used by tests that don't need stealth or
    /// challenge handling. For production flows use [`Page::navigate`].
    pub async fn navigate_simple(
        url: &str,
        client: &net::HttpClient,
        profile: stealth::StealthProfile,
    ) -> Result<Self, deno_core::error::AnyError> {
        let resp = client
            .get(url)
            .await
            .map_err(|e| deno_core::error::AnyError::msg(e.to_string()))?;
        let html = resp.text();
        Self::from_html_with_url(&html, url, Some(profile)).await
    }

    /// Navigate with a stealth profile.
    pub async fn navigate_stealth(
        url: &str,
        profile: stealth::StealthProfile,
    ) -> Result<Self, deno_core::error::AnyError> {
        let client = net::HttpClient::new(&profile)
            .map_err(|e| deno_core::error::AnyError::msg(e.to_string()))?;
        let resp = client
            .get_follow(url, 10)
            .await
            .map_err(|e| deno_core::error::AnyError::msg(e.to_string()))?;
        let html = resp.text();
        let _ = std::fs::write("oxide_dump/page_html.html", &html);
        let resp_url = resp.url.clone();
        Self::with_profile(&html, &resp_url, profile).await
    }

    /// Generic navigation entry point.
    ///
    /// Loops by re-fetching whenever a script sets
    /// `globalThis.__pendingNavigation` (via `location.reload`,
    /// `location.href = ...`, `location.assign/replace`, or a
    /// `<meta http-equiv="refresh">` tag). Each iteration drops the
    /// previous V8 isolate and builds a fresh one — identical to how a
    /// real browser does a top-level navigation. Zero per-engine logic.
    ///
    /// `max_iterations` caps the loop to prevent infinite reload cycles.
    /// `5` is a reasonable default for challenge flows (interstitial →
    /// solver → real page is the common case, so even 3 is enough).
    pub async fn navigate(
        url: &str,
        profile: stealth::StealthProfile,
        max_iterations: u8,
    ) -> Result<Self, deno_core::error::AnyError> {
        // Phase F — humanization is default-on. Sensor-based detectors
        // (Akamai BMP, DataDome, PerimeterX) score "zero user input in
        // first 2 s" as a strong bot signal. Real Chrome sessions
        // accumulate dozens of mousemove/scroll/click events before any
        // sensor VM posts; headless runs see none unless we synthesize
        // them. The cost is ~30 setTimeout dispatches over ~2 s of page
        // life — negligible compared to the navigation budget — and the
        // benefit is a measurable PASS bump on behaviorally-fingerprinted
        // sites.
        //
        // Opt-out via `navigate_pure(url, profile, max_iter)` for
        // deterministic snapshot tests where synthetic input would skew
        // results.
        let humanize = include_str!("js/humanize.js").to_string();
        Self::navigate_with_init(url, profile, max_iterations, vec![humanize]).await
    }

    /// Pure navigation — no humanization, no synthetic events. Use for
    /// deterministic snapshot tests, layout dump captures, or any
    /// scenario where the test wants to assert about the page's *own*
    /// behavior without injected mousemove/click activity.
    pub async fn navigate_pure(
        url: &str,
        profile: stealth::StealthProfile,
        max_iterations: u8,
    ) -> Result<Self, deno_core::error::AnyError> {
        Self::navigate_with_init(url, profile, max_iterations, Vec::new()).await
    }

    /// Alias of [`Page::navigate`] preserved for backward compatibility.
    /// `Page::navigate` is already humanized as of Phase F.
    pub async fn navigate_humanized(
        url: &str,
        profile: stealth::StealthProfile,
        max_iterations: u8,
    ) -> Result<Self, deno_core::error::AnyError> {
        Self::navigate(url, profile, max_iterations).await
    }

    /// Like [`Page::navigate`], but installs caller-supplied init scripts on
    /// every iteration's fresh runtime. Used by [`Page::navigate_humanized`]
    /// and any future feature that wants to carry JS across navigations
    /// within a single frame (equivalent to Chromium's
    /// `Page.addScriptToEvaluateOnNewDocument`).
    /// Like [`Page::navigate`], but installs caller-supplied init scripts on
    /// every iteration's fresh runtime. Used by [`Page::navigate_humanized`]
    /// and any future feature that wants to carry JS across navigations
    /// within a single frame (equivalent to Chromium's
    /// `Page.addScriptToEvaluateOnNewDocument`).
    pub async fn navigate_with_init(
        url: &str,
        profile: stealth::StealthProfile,
        max_iterations: u8,
        init_scripts: Vec<String>,
    ) -> Result<Self, deno_core::error::AnyError> {
        let client = net::HttpClient::new(&profile)
            .map_err(|e| deno_core::error::AnyError::msg(e.to_string()))?;

        // Share the HTTP client with JS fetch() so scripts running inside
        // the V8 isolate hit the same cookie jar as the Rust driver.
        js_runtime::extensions::fetch_ext::set_fetch_client(client.clone());

        let iterations = max_iterations.max(1);
        let debug_nav = std::env::var("BOXIDE_DEBUG_NAV").is_ok();

        tracing::debug!(url = %url, "navigate initial fetch");
        let resp = client
            .get_follow(url, 10)
            .await
            .map_err(|e| deno_core::error::AnyError::msg(e.to_string()))?;

        if resp.status == 498 || resp.status == 403 || resp.status == 429 {
            eprintln!(
                "[navigate] Initial challenge response headers ({}):",
                resp.status
            );
            for (k, v) in &resp.headers {
                eprintln!("  {}: {}", k, v);
            }
        }
        // G.3 — log known anti-bot vendor response markers so post-run
        // analysis can split CHL outcomes by protocol. Each marker also
        // hints whether the site needs a vendor-specific solver
        // (`memory/open_tasks.md#64` Akamai, etc.). No flow change yet —
        // the adaptive budget from Phase A.1 already short-circuits when
        // the body is small + readyState complete, so we don't burn the
        // full 75 s on a 2 KB challenge stub.
        if let Some(waf) = resp.headers.get("x-amzn-waf-action") {
            eprintln!("[vendor-detect] aws-waf {} on {}", waf, resp.url);
        }
        if resp.headers.contains_key("x-datadome") {
            eprintln!("[vendor-detect] datadome on {}", resp.url);
        }
        if resp.headers.contains_key("x-wbaas-token") {
            eprintln!("[vendor-detect] wbaas on {}", resp.url);
        }
        // Akamai BMP marker: _abck cookie set. Use header parse.
        if resp
            .headers
            .iter()
            .any(|(k, v)| k.eq_ignore_ascii_case("set-cookie") && v.starts_with("_abck="))
        {
            eprintln!("[vendor-detect] akamai-bmp _abck set on {}", resp.url);
        }
        // Collect response-header CSP value(s) before consuming `resp`.
        // CSP3 §3.2 allows the header to repeat (multiple Policy values
        // applied conjunctively); we keep each instance separate so the
        // "all must allow" matcher semantic stays correct.
        let csp_headers: Vec<String> = resp
            .headers
            .iter()
            .filter(|(k, _)| k.eq_ignore_ascii_case("content-security-policy"))
            .map(|(_, v)| v.clone())
            .collect();
        let csp_headers_ro: Vec<String> = resp
            .headers
            .iter()
            .filter(|(k, _)| k.eq_ignore_ascii_case("content-security-policy-report-only"))
            .map(|(_, v)| v.clone())
            .collect();
        let html = resp.text();
        // W3.8 — surface DataDome interstitials as a structured signal.
        // The `var dd={…}` 403 body (etsy/tripadvisor/wsj/reuters) was
        // previously only visible as a downstream CSP-refusal symptom
        // (we mis-route the document-level challenge as a child iframe
        // and frame-src correctly refuses geo.captcha-delivery.com).
        // Detection is wired here (log-only, no flow change yet —
        // matches the vendor-detect convention above); the solver needs
        // Chrome-correct audio+canvas FP (see docs/research_2026_05_14/
        // 16_AUDIO_BLINK_PARITY) before it can round-trip the cookie.
        if let Some(dd) = crate::datadome_handler::detect_datadome_interstitial(&html) {
            // Phase 5 (doc 05 §2d): replace the old log-only telemetry
            // with the typed in-engine self-solve plan. The behavioral
            // round-trip is performed by the existing pieces — i.js runs
            // in our V8, the shared cookie jar captures the resulting
            // `datadome=` Set-Cookie, and the cookie-diff retry below
            // re-issues the original URL. What was missing (and is fixed
            // at the CSP-install site in navigate_loop_internal) is that
            // the origin's restrictive 403 CSP must NOT be enforced on
            // the DataDome challenge document, or i.js cannot reach
            // geo.captcha-delivery.com (doc 05 §2c symptom).
            match crate::datadome_handler::plan_datadome_solve(&dd, &resp.url) {
                Some(plan) => eprintln!(
                    "[datadome] interstitial rt={} cid={} — in-engine self-solve planned: \
                     challenge_hosts={:?} renav={} (cookie-diff retry re-issues)",
                    plan.rt, dd.cid, plan.challenge_hosts, plan.renav_url
                ),
                None => eprintln!(
                    "[datadome] interstitial rt={} cid={} on {} — not auto-solvable \
                     (human slider / out of stealth scope)",
                    dd.rt, dd.cid, resp.url
                ),
            }
        }
        let resp_url = resp.url.clone();
        let timings = resp.timings.clone();
        let mut page = Self::navigate_loop_internal(
            html,
            resp_url,
            profile,
            client,
            iterations,
            0,
            init_scripts,
            debug_nav,
            csp_headers,
            csp_headers_ro,
            resp.accept_ch_upgrade,
        )
        .await?;

        page.event_loop()
            .runtime_mut()
            .record_resource_timing(timings);
        Ok(page)
    }

    /// For tests: start a navigation loop with a provided HTML instead of
    /// fetching from URL. Subsequent iterations (if any) will fetch from the URL.
    pub async fn navigate_with_html(
        html: &str,
        url: &str,
        profile: stealth::StealthProfile,
        max_iterations: u8,
    ) -> Result<Self, deno_core::error::AnyError> {
        let client = net::HttpClient::new(&profile)
            .map_err(|e| deno_core::error::AnyError::msg(e.to_string()))?;
        js_runtime::extensions::fetch_ext::set_fetch_client(client.clone());

        let iterations = max_iterations.max(1);
        let debug_nav = std::env::var("BOXIDE_DEBUG_NAV").is_ok();

        // Iteration 0 uses provided HTML — no headers means no CSP
        // (this entry point is for tests that hand us synthetic HTML).
        Self::navigate_loop_internal(
            html.to_string(),
            url.to_string(),
            profile,
            client,
            iterations,
            0,
            vec![],
            debug_nav,
            Vec::new(),
            Vec::new(),
            false,
        )
        .await
    }

    async fn navigate_loop_internal(
        html: String,
        resp_url: String,
        profile: stealth::StealthProfile,
        client: net::HttpClient,
        iterations: u8,
        start_iter: u8,
        init_scripts: Vec<String>,
        debug_nav: bool,
        csp_headers: Vec<String>,
        csp_headers_ro: Vec<String>,
        accept_ch_upgrade: bool,
    ) -> Result<Self, deno_core::error::AnyError> {
        // Install CSP for this navigation. Headers + meta-tag sources
        // both contribute. The fetch_ext layer reads from a per-process
        // RwLock so async ops can enforce without borrowing OpState.
        // `enforce_csp` controls whether matches actually block; off
        // means CSP is parsed and reported but doesn't gate fetches —
        // useful for A/B comparison on the holistic sweep.
        {
            let csp_dom = html_parser::parse_html(&html);
            let header_refs: Vec<&str> = csp_headers.iter().map(|s| s.as_str()).collect();
            let report_refs: Vec<&str> = csp_headers_ro.iter().map(|s| s.as_str()).collect();
            let policy_set = crate::csp_collector::collect_csp_with_report_only(
                &header_refs,
                &report_refs,
                &csp_dom,
            );
            // Bypass switch — useful to compare engine behaviour with
            // and without enforcement on the same site without rebuild.
            let env_bypass = std::env::var("BOXIDE_CSP_BYPASS").is_ok();
            // Phase 5 (doc 05 §2c/§2d): a DataDome `rt:'i'` interstitial
            // is a DataDome-served challenge document, NOT the origin's
            // page — enforcing the origin's restrictive 403-response CSP
            // on it refuses geo.captcha-delivery.com and kills the i.js
            // self-solve round-trip. Narrowly gated to the <4 KB
            // interstitial shape (detect_datadome_interstitial), so it
            // cannot affect normal pages or the 10 passing Akamai sites
            // (their bodies don't match). The cookie-diff retry then
            // re-issues the original URL once i.js lands `datadome=`.
            let dd_challenge_doc = crate::datadome_handler::is_datadome_challenge_doc(&html);
            let enforce = profile.enforce_csp && !env_bypass && !dd_challenge_doc;
            if dd_challenge_doc {
                if debug_nav {
                    eprintln!(
                        "[datadome] challenge document — origin CSP not enforced \
                         (i.js round-trip to captcha-delivery.com permitted)"
                    );
                }
            }
            if let Ok(origin) = url::Url::parse(&resp_url) {
                if !policy_set.is_empty() {
                    if debug_nav {
                        eprintln!(
                            "[csp] installed {} policies from headers={} meta={} enforce={}",
                            policy_set.policies.len(),
                            csp_headers.len(),
                            policy_set.policies.len() - csp_headers.len(),
                            enforce
                        );
                    }
                    js_runtime::extensions::fetch_ext::set_csp_policy(
                        std::sync::Arc::new(policy_set),
                        origin,
                        enforce,
                    );
                } else {
                    js_runtime::extensions::fetch_ext::clear_csp_policy();
                }
            } else {
                js_runtime::extensions::fetch_ext::clear_csp_policy();
            }
        }

        const PENDING_NAV_JS: &str = "(function(){\
                const boxide = globalThis._boxide;\
                const p = boxide && boxide.__pendingNavigation;\
                if (p) boxide.__pendingNavigation = null;\
                return p ? JSON.stringify({url: p.url, method: p.method || 'GET', body: p.body, kind: p.kind}) : '';\
            })()";

        // Phase 5 (doc 05 §2d) — did THIS navigation start as a DataDome
        // `rt:'i'` interstitial? Computed once from the original response
        // body, *before* the loop mutates `current_html`. Load-bearing:
        // i.js mutates the live DOM (removes its own inline `var dd` +
        // `<script src=i.js>`), so the post-i.js `self.content()` no
        // longer trips `body_has_challenge_marker` → `is_anti_bot_
        // challenge()` flips false → the engine wrongly skips BOTH the
        // pending-nav poll AND the cookie-diff retry, bailing before
        // i.js's round-trip can land the `datadome=` cookie. This flag
        // keeps the challenge-resolution path active for the DataDome
        // nav specifically; it is false for every non-DataDome site, so
        // it cannot regress any other flow / the §4 gate.
        let started_as_dd_challenge =
            crate::datadome_handler::is_datadome_challenge_doc(&html);
        // Akamai sec-cpt analog (master plan §4 Phase 3 / §8.5): homedepot
        // serves the rotating-obfuscated-bundle sec-cpt variant
        // (`<div id="sec-if-cpt-container">` + `<script src="/Wjv3…">`).
        // The bundle self-solves in our V8 and sets the `sec_cpt` cookie,
        // but it mutates the DOM the same way DataDome's i.js does, so the
        // post-exec `is_anti_bot_challenge()` can flip false and skip the
        // poll + cookie-diff retry before the bundle's round-trip lands.
        // Same narrow gating ⇒ false for every non-sec-cpt site ⇒ zero
        // regression / §4 gate unaffected. (If the marker happens to
        // persist post-exec this OR-in is simply a harmless no-op.)
        let started_as_seccpt_challenge =
            html.contains("sec-if-cpt-container") || html.contains("sec-cpt-if");
        let mut current_html = html;
        let mut current_url = resp_url;
        let mut current_storage: Option<
            std::collections::HashMap<String, std::collections::HashMap<String, String>>,
        > = None;
        let mut last_accept_ch_upgrade = accept_ch_upgrade;
        let mut accept_ch_retry_done = false;

        // Wall-clock budget for this entire navigate_with_init call.
        // Default 50 s leaves headroom under the antibot_smoke 60 s wrapper.
        // Override via BOXIDE_NAV_BUDGET_MS for slow-link or debugging runs.
        // The budget is mutable: if iter=0 returns a *real-content* page
        // (no challenge marker AND body > 50 KB), we extend the budget by
        // BOXIDE_NAV_BUDGET_EXTEND_MS (default 25 s) to allow heavy
        // legitimate sites (footlocker, walmart) to fully render.
        // Default budget aggressively low (15 s) — most pages render their
        // primary body well under that. Sites that legitimately need more
        // time get the per-iteration extension below; sites that hit a CHL
        // marker get retried with a fresh budget per iteration. The old
        // 50 s default left 35 s on the table for fast sites, which
        // dominated the holistic-sweep wall-clock (96 min for 126 sites).
        // Host-aware default. Three classes of sites need more than the
        // 15s baseline:
        //   - Kasada-protected: ~530KB ips.js VM that runs PoW + sensor
        //     POST + token negotiation; 25-40s on our V8. Bump to 45s.
        //   - Akamai BMP-protected: sensor_data flow ~20s. Bump to 25s.
        //   - SPA shells (twitter, x.com, hulu, yandex.ru, h&m,
        //     khanacademy): main bundle is 1-5MB; React/Vue hydration
        //     in our V8 takes 60-90s vs ~5s on headed Chrome. Without
        //     the bump, body=0/69 bytes after deadline. Per W5 Tier A
        //     in PLAN_2026_05_10_UPDATE.md.
        let host_budget_default_ms = match url::Url::parse(&current_url)
            .ok()
            .and_then(|u| u.host_str().map(str::to_string))
            .as_deref()
        {
            // Kasada-protected (high-tier).
            Some(h)
                if h.ends_with("canadagoose.com")
                    || h.ends_with("hyatt.com")
                    || h.ends_with("ticketmaster.com")
                    || h.ends_with("footlocker.com")
                    || h.ends_with("veve.me") =>
            {
                45_000
            }
            // SPA shells — heavy React/Vue hydration.
            Some(h)
                if h.ends_with("twitter.com")
                    || h.ends_with("x.com")
                    || h.ends_with("hulu.com")
                    || h.ends_with("yandex.ru")
                    || h.ends_with("hm.com")
                    || h.ends_with("khanacademy.org")
                    || h.ends_with("spotify.com") =>
            {
                90_000
            }
            // Akamai BMP-protected.
            Some(h)
                if h.ends_with("bestbuy.com")
                    || h.ends_with("homedepot.com")
                    || h.ends_with("nike.com")
                    || h.ends_with("adidas.com")
                    || h.ends_with("samsclub.com")
                    || h.ends_with("walmart.com") =>
            {
                25_000
            }
            _ => 15_000,
        };
        let mut nav_budget = Duration::from_millis(
            std::env::var("BOXIDE_NAV_BUDGET_MS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(host_budget_default_ms),
        );
        let nav_budget_extend = Duration::from_millis(
            std::env::var("BOXIDE_NAV_BUDGET_EXTEND_MS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(25_000),
        );
        let mut budget_extended = false;
        let nav_t0 = std::time::Instant::now();

        for iter in start_iter..iterations {
            // Bail before starting a new iteration if the wall-clock
            // budget is already exhausted — no point doing build_page
            // and drain if we have no time to react to the result.
            if nav_t0.elapsed() >= nav_budget {
                eprintln!(
                    "[navigate] budget exhausted ({}ms used / {}ms budget) before iter={} — bailing",
                    nav_t0.elapsed().as_millis(),
                    nav_budget.as_millis(),
                    iter
                );
                break;
            }

            // If we are re-using a page from a previous iteration that
            // was terminated, cancel the termination now so we can run
            // init scripts and other logic.
            // Note: we don't have a 'page' yet on iter 0, but we do on retries.
            // (The build_page_with_scripts... call below creates a fresh
            // runtime anyway, but the loop logic might eventually change).

            tracing::info!(iter = iter, url = %current_url, "navigation loop start");
            if debug_nav {
                eprintln!(
                    "[navigate] iter={} url={} html_len={}",
                    iter,
                    current_url,
                    current_html.len()
                );
            }

            // Reset the per-page sync-fetch counter at the start of each
            // iteration so MAX_SYNC_FETCH_PER_PAGE bounds *this* page's
            // chain, not the cumulative across iterations.
            js_runtime::extensions::fetch_ext::reset_sync_fetch_count();

            // Snapshot cookies for this URL before the page runs. If the jar
            // gains new cookies during script execution (e.g. Kasada /tl
            // response set a session cookie), we treat that as the
            // "challenge-solved" signal and retry — Kasada's ips.js solves
            // the PoW but never calls location.reload(), relying on the user
            // to hit F5. This primitive is that F5.
            let parsed_current = url::Url::parse(&current_url).ok();
            let cookies_before: String = if let Some(p) = parsed_current.as_ref() {
                client.cookies_for_url(p).await.unwrap_or_default()
            } else {
                String::new()
            };
            if debug_nav {
                tracing::debug!(cookies = %cookies_before, "navigate jar cookies (before)");
            }

            let mut page = Self::build_page_with_scripts_init_and_storage(
                &current_html,
                &current_url,
                &profile,
                &client,
                &init_scripts,
                current_storage.take(),
            )
            .await?;

            // Install the V8 deadline watcher for the remainder of the
            // wall-clock budget — but always with a minimum 5s floor so
            // even iterations past the nominal budget have a safety net.
            // Without the floor, a budget-exhausted iteration could spin
            // forever in V8 (no watcher → tokio::time::timeout can't
            // preempt CPU-bound JS).
            let remaining = nav_budget
                .saturating_sub(nav_t0.elapsed())
                .max(Duration::from_secs(5));
            eprintln!(
                "[navigate] iter={} installing V8DeadlineWatcher with {}ms remaining",
                iter,
                remaining.as_millis()
            );
            let _watcher =
                V8DeadlineWatcher::new(page.event_loop().runtime_mut().isolate_handle(), remaining);

            // Drain the event loop. Use the remaining nav budget (floored at 8s)
            // so that heavy PoW challenges (Kasada KPSDK takes 30+ seconds) can
            // complete their /tl POST AFTER the PoW finishes. The V8DeadlineWatcher
            // installed above provides the hard kill for analytics loops that never
            // reach idle on their own — once V8 is terminated, run_event_loop()
            // returns and the drain exits naturally.
            let drain_timeout = {
                let remaining = nav_budget.saturating_sub(nav_t0.elapsed());
                remaining.max(Duration::from_secs(8))
            };
            if let Err(e) = page.event_loop().run_until_idle(drain_timeout).await {
                tracing::warn!(error = %e, "navigate event loop error");
            }

            // If the watcher fired (or if we reached idle naturally), ensure
            // the isolate is ready for further script execution (draining
            // events, classification, etc.).
            page.event_loop().runtime_mut().cancel_terminate_execution();

            // Phase A.1 — Adaptive budget. Two paths after the first iteration:
            //
            // 1. FAST-EXIT — body > 50 KB AND no CHL marker AND readyState
            //    "complete" → the site rendered cleanly, return it now.
            //    Skips iter 1 and iter 2 entirely. Closes the dominant
            //    fast-site stall in the holistic sweep (where every fast
            //    page used to wait the full 50 s budget for nothing).
            //
            // 2. EXTEND — body > 50 KB but readyState still "loading"
            //    (e.g. footlocker, walmart pre-paint). Give one extension.
            //
            // CHL markers always continue iterating (cookie-delta retry path
            // below kicks in). Tiny-body responses also continue (challenges
            // often start as <50 KB stubs).
            if iter == start_iter {
                let body_len: usize = page
                    .event_loop()
                    .execute_script("document.body ? document.body.outerHTML.length : 0")
                    .unwrap_or_default()
                    .parse()
                    .unwrap_or(0);
                let is_chl = page.is_anti_bot_challenge();
                if !is_chl && body_len > 50 * 1024 {
                    let ready_state = page
                        .event_loop()
                        .execute_script("document.readyState")
                        .unwrap_or_default();
                    let ready_state = ready_state.trim().trim_matches('"');
                    if ready_state == "complete" {
                        eprintln!(
                            "[navigate] fast-exit on iter={} (body={}KB, no CHL, readyState=complete)",
                            iter,
                            body_len / 1024
                        );
                        return Ok(page);
                    }
                    if !budget_extended {
                        nav_budget += nav_budget_extend;
                        budget_extended = true;
                        eprintln!(
                            "[navigate] budget extended +{}ms (body={}KB, no CHL marker, readyState={})",
                            nav_budget_extend.as_millis(),
                            body_len / 1024,
                            ready_state
                        );
                    }
                }
                // SPA hydration early-exit (W5b A3 + W5b-PLUS expansion).
                // For React/Vue/Next.js sites the `<body>` outerHTML may be
                // tiny (a 69-byte <noscript> + a single mount div) OR
                // moderately sized (twitter ships a 241KB shell of inline
                // state + script tags) — but the user-visible content is
                // always under one of the well-known mount-point IDs. If
                // ANY common SPA mount has ≥1 child element, the app is
                // alive and the loop's continued spinning is just noise
                // we'd terminate at drop time anyway. Without this signal,
                // twitter/x/hulu (heavy shells, slow hydration) burn the
                // full nav budget waiting for is_pending=false — which
                // never arrives because React's scheduler keeps queuing
                // setTimeout work forever. Per W5b research + W5b-PLUS
                // profile (2026-05-10): pending state in steady-state is
                // 33 op_timer_sleep, cycling 33→18→1→33 driven by React.
                //
                // The mount-populated check is intentionally cheap (single
                // querySelector chain, fail-fast) so it adds <1ms per
                // iteration. Removed the prior `body_len <= 50KB` gate
                // because twitter's 241KB shell was tripping the wrong
                // branch — the mount-children count is the single source
                // of truth for "is this app rendered."
                if !is_chl {
                    let mount_populated: usize = page
                        .event_loop()
                        .execute_script(
                            "(function(){\
                                var sels = ['#react-root','#__next','#app','#root','[data-reactroot]','#main-app','#mount-point'];\
                                for (var i = 0; i < sels.length; i++) {\
                                    var el = document.querySelector(sels[i]);\
                                    if (el && el.children && el.children.length > 0) return el.children.length;\
                                }\
                                return 0;\
                            })()",
                        )
                        .unwrap_or_default()
                        .parse()
                        .unwrap_or(0);
                    if mount_populated > 0 {
                        eprintln!(
                            "[navigate] SPA-fast-exit on iter={} (body={}KB, mount has {} children)",
                            iter,
                            body_len / 1024,
                            mount_populated
                        );
                        return Ok(page);
                    }
                }
            }

            // Did a script request a re-navigation?
            let mut pending_info = page
                .event_loop()
                .execute_script(PENDING_NAV_JS)
                .unwrap_or_default();

            if !pending_info.is_empty() {
                tracing::info!(pending = %pending_info, "initial pending navigation found");
            }

            // Bounded poll for deferred navigation signals (auto-submitted forms,
            // PoW completions, challenge-driven assigns). Replaces the previous
            // fixed 2s wait. Checks every 200ms for up to 10s total; exits early
            // on first hit.
            if pending_info.is_empty()
                && (page.is_anti_bot_challenge()
                    || started_as_dd_challenge
                    || started_as_seccpt_challenge)
            {
                let deadline = std::time::Instant::now() + Duration::from_secs(90);
                while std::time::Instant::now() < deadline {
                    let _ = page
                        .event_loop()
                        .run_until_idle(Duration::from_millis(200))
                        .await;
                    pending_info = page
                        .event_loop()
                        .execute_script(PENDING_NAV_JS)
                        .unwrap_or_default();
                    if !pending_info.is_empty() {
                        break;
                    }
                    // Phase 5: for a DataDome nav, i.js's round-trip
                    // typically lands a fresh `datadome=` cookie WITHOUT
                    // setting a pending nav — break as soon as it does so
                    // the cookie-diff retry below re-issues the original
                    // URL (instead of burning the full 90 s deadline).
                    if started_as_dd_challenge {
                        if let Some(p) = parsed_current.as_ref() {
                            let now =
                                client.cookies_for_url(p).await.unwrap_or_default();
                            if crate::datadome_handler::cookies_have_datadome(&now)
                                && !crate::datadome_handler::cookies_have_datadome(
                                    &cookies_before,
                                )
                            {
                                break;
                            }
                        }
                    }
                }
            }

            // 3. Selective CSP bypass for known anti-bot challenge domains.
            // Walmart / Canada Goose / Hyatt / Realtor / Foot Locker etc. CSPs
            // often block their own Akamai/Kasada trackers in emulated
            // environments due to origin/nonce mismatches. Without the bypass
            // we get body=0 because the ips.js script we'd LOAD to solve the
            // challenge is the very thing CSP refuses (caught on hyatt.com
            // 2026-05-10 round-3 sweep — went Kasada-CHL → THIN-BODY when
            // body=0 because CSP refused to load the ips.js script).
            if current_url.contains("walmart.com")
                || current_url.contains("canadagoose.com")
                || current_url.contains("hyatt.com")
                || current_url.contains("realtor.com")
                || current_url.contains("footlocker.com")
                || current_url.contains("ticketmaster.com")
                || current_url.contains("udemy.com")
            {
                tracing::info!(url = %current_url, "applying selective CSP bypass for anti-bot domain");
                let mut rt = page.event_loop().runtime_mut();
                let op_state = rt.op_state();
                let mut state = op_state.borrow_mut();
                if let Some(stealth_state) =
                    state.try_borrow_mut::<js_runtime::extensions::stealth_ext::StealthState>()
                {
                    if let Some(profile) = &mut stealth_state.profile {
                        profile.enforce_csp = false;
                    }
                }
            }

            // Phase A.5 — Akamai sensor_data POST.
            // Phase 5 homedepot fix (measured doc-20 anti-pattern):
            // `handle_akamai_flow`'s sec-cpt guard checks `self.content()`
            // (the *mutable current DOM*). After the `/Wjv3…` sec-cpt
            // bundle runs and mutates the DOM, `sec-if-cpt-container` is
            // gone, so the guard misses and the engine fires the WRONG
            // BMP `sensor_data` POST to the sec-cpt verify endpoint —
            // Akamai 201s `NeedsSensor` forever (`_abck=…~-1~` never
            // clears) AND, per doc 20, that conflicting traffic actively
            // prevents the bundle's own self-solve flow. Use the
            // persistent "this nav started as sec-cpt" signal so the BMP
            // path is suppressed for the WHOLE nav, letting the bundle be
            // the sole actor. Narrowly gated (false for every non-sec-cpt
            // site incl. the entire §4 gate) ⇒ zero regression; the 10
            // passing plain-BMP Akamai sites never serve sec-if-cpt.
            let akamai_state = if started_as_seccpt_challenge {
                akamai::AbckState::NeedsSecCpt
            } else {
                page.handle_akamai_flow(&client)
                    .await
                    .unwrap_or(akamai::AbckState::Unknown)
            };

            // W7 / Cloudflare V1 — orchestrator runner. Detect Managed/JS
            // Challenge from the rendered body and drive the event loop until
            // cf_clearance is issued (or the 10 s budget expires). The outer
            // cookie-delta retry path picks up cf_clearance and re-fetches.
            let _cf_ctx = page.handle_cloudflare_flow(&client).await;

            if pending_info.is_empty() {
                // Post-settle cookie-delta retry: if the cookie jar gained new
                // values during this iteration AND the page still looks like a
                // challenge AND we have iterations left, retry the same URL
                // once. Covers engines whose solver sets a session cookie and
                // expects the NEXT top-level nav to carry it (Kasada; some
                // Akamai variants). Universal primitive — no per-engine code.
                //
                // PHASE J: also retry if the origin just upgraded to Accept-CH
                // (Wildberries parity). Only retry ONCE for the upgrade.
                if (page.is_anti_bot_challenge()
                    || started_as_dd_challenge
                    || started_as_seccpt_challenge
                    || (last_accept_ch_upgrade && !accept_ch_retry_done))
                    && iter + 1 < iterations
                {
                    let cookies_after: String = if let Some(p) = parsed_current.as_ref() {
                        client.cookies_for_url(p).await.unwrap_or_default()
                    } else {
                        String::new()
                    };

                    // Phase 5 instrumentation: at the exact decision point
                    // where the cookie-diff retry would re-issue the
                    // original URL, record whether the DataDome i.js
                    // round-trip actually landed a `datadome=` cookie.
                    // `cookie_gained=false` here ⇒ the bundle's VM/WASM
                    // did not complete (the next increment's target);
                    // `true` ⇒ the existing retry already re-issues.
                    // debug_nav-gated ⇒ zero §4-gate impact.
                    if debug_nav && started_as_dd_challenge {
                        eprintln!(
                            "{}",
                            crate::datadome_handler::dd_flow_summary(
                                page.is_anti_bot_challenge(),
                                crate::datadome_handler::cookies_have_datadome(&cookies_before),
                                crate::datadome_handler::cookies_have_datadome(&cookies_after),
                            )
                        );
                        // What did i.js actually do? Dump its post-exec
                        // fetch log so the next increment sees whether the
                        // 15 KB loader fired its verification POST to
                        // geo.captcha-delivery.com and what came back.
                        let fl = page
                            .event_loop()
                            .execute_script(
                                "JSON.stringify((globalThis._boxide&&globalThis._boxide.__fetchLog)||[])",
                            )
                            .unwrap_or_default();
                        eprintln!("[datadome-trace] i.js __fetchLog={fl}");
                    }
                    // Phase 5 (homedepot): same diagnostic for the
                    // sec-cpt bundle — did `/Wjv3…` actually run and fire
                    // its PoW-answer verify POST? debug_nav-gated ⇒ zero
                    // §4 gate impact.
                    if debug_nav && started_as_seccpt_challenge {
                        let fl = page
                            .event_loop()
                            .execute_script(
                                "JSON.stringify((globalThis._boxide&&globalThis._boxide.__fetchLog)||[])",
                            )
                            .unwrap_or_default();
                        let secck = page
                            .event_loop()
                            .execute_script(
                                "(function(){try{return /sec_cpt=/.test(document.cookie)?'sec_cpt-present':'no-sec_cpt'}catch(e){return 'err'}})()",
                            )
                            .unwrap_or_default();
                        eprintln!(
                            "[seccpt-trace] post-bundle cookie={secck} __fetchLog={fl}"
                        );
                    }

                    let mut should_retry = (cookies_after != cookies_before
                        && !cookies_after.is_empty())
                        || (last_accept_ch_upgrade && !accept_ch_retry_done);

                    // Special case for Akamai: if we are already favorable, DON'T retry
                    // just because the cookie value changed (it always does).
                    if akamai_state == akamai::AbckState::Favorable
                        && !(last_accept_ch_upgrade && !accept_ch_retry_done)
                    {
                        should_retry = false;
                    }

                    if should_retry {
                        if last_accept_ch_upgrade {
                            accept_ch_retry_done = true;
                        }
                        // Before launching the retry, check we have at least
                        // ~15s of nav budget left — a retry requires a fresh
                        // build + drain (~10-15s minimum). If the budget is
                        // too tight, return the current iter=0 page instead
                        // of blowing the budget and returning nothing
                        // (regression we hit on nike.com — Akamai bm_sz
                        // cookie triggers the retry path even on real
                        // homepages).
                        const MIN_RETRY_BUDGET: Duration = Duration::from_secs(15);
                        if nav_budget.saturating_sub(nav_t0.elapsed()) < MIN_RETRY_BUDGET {
                            eprintln!(
                                "[navigate] iter={} skip cookie-delta retry: only {}ms left of {}ms budget",
                                iter,
                                nav_budget.saturating_sub(nav_t0.elapsed()).as_millis(),
                                nav_budget.as_millis()
                            );
                            return Ok(page);
                        }
                        if debug_nav {
                            eprintln!(
                                "[navigate] iter={} POST-SETTLE RETRY firing for {}",
                                iter, current_url
                            );
                        }
                        tracing::info!(
                            before_len = cookies_before.len(),
                            after_len = cookies_after.len(),
                            "cookie delta after challenge scripts — retrying"
                        );

                        // Option A: try an in-V8 refetch first. If a challenge
                        // engine (Kasada/PerimeterX/etc.) patched window.fetch
                        // during script execution to inject session headers
                        // (x-kpsdk-ct and friends), those headers ride along
                        // on this fetch — which a fresh Rust-side GET would
                        // not carry. The page stays alive while we refetch so
                        // the patched fetch state is preserved.
                        let refetch_js = r#"
                            (async () => {
                                globalThis.__psrHtml = null;
                                globalThis.__psrStatus = 0;
                                globalThis.__psrErr = null;
                                try {
                                    const resp = await fetch(location.href, {
                                        method: 'GET',
                                        credentials: 'include',
                                        headers: {
                                            'accept': 'text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8',
                                        },
                                    });
                                    globalThis.__psrStatus = resp.status;
                                    globalThis.__psrHtml = await resp.text();
                                } catch (e) {
                                    globalThis.__psrErr = String((e && e.message) || e);
                                }
                            })();
                        "#;
                        let _ = page
                            .event_loop()
                            .execute_and_run(refetch_js, Duration::from_secs(15))
                            .await;
                        let status_str = page
                            .event_loop()
                            .execute_script("String(globalThis.__psrStatus || 0)")
                            .unwrap_or_default();
                        let err_str = page
                            .event_loop()
                            .execute_script("String(globalThis.__psrErr || '')")
                            .unwrap_or_default();
                        let v8_html = page
                            .event_loop()
                            .execute_script("String(globalThis.__psrHtml || '')")
                            .unwrap_or_default();
                        if debug_nav {
                            eprintln!(
                                "[navigate] iter={} in-V8 refetch status={} err={} html_len={}",
                                iter,
                                status_str,
                                err_str,
                                v8_html.len()
                            );
                        }

                        // Accept the V8-fetched body if it's larger than the
                        // current challenge page AND doesn't re-trigger our
                        // anti-bot content markers. Otherwise fall back to a
                        // Rust-side GET (cookie-only flow — works for simpler
                        // engines that upgrade on any authenticated request).
                        // Coverage of the marker set must mirror
                        // is_anti_bot_challenge() — otherwise a vendor we
                        // detect at the top of the loop is silently accepted
                        // here, breaking the retry chain. Caught for
                        // DataDome (yelp/etsy/leboncoin/wsj) on 2026-05-10.
                        let v8_html_is_real = !v8_html.is_empty()
                            && v8_html.len() > current_html.len()
                            && !v8_html.contains("/ips.js")
                            && !v8_html.contains("/149e9513-")
                            && !v8_html.contains("kpsdk")
                            && !v8_html.contains("_abck")
                            && !v8_html.contains("bm_sz")
                            && !v8_html.contains("captcha-delivery.com")
                            && !v8_html.contains("dd-script")
                            && !v8_html.contains("dd_engagement")
                            && !v8_html.contains("/cdn-cgi/challenge-platform/");

                        // Extract any challenge-engine session headers that
                        // scripts collected during solves. For Kasada: the
                        // last successful POST /tl response carried a fresh
                        // x-kpsdk-ct that the retry GET must forward AS A
                        // REQUEST HEADER (Hyper-Solutions' Go SDK docs it
                        // explicitly). Cookies alone are not enough.
                        let kpsdk_headers_js = r#"
                            JSON.stringify((() => {
                                const log = globalThis.__fetchLog || [];
                                const out = {};
                                for (const entry of log) {
                                    const resp = entry.respHeaders || {};
                                    for (const k of Object.keys(resp)) {
                                        if (k.toLowerCase().startsWith('x-kpsdk')) {
                                            out[k.toLowerCase()] = resp[k];
                                        }
                                    }
                                    const req = entry.reqHeaders || {};
                                    for (const k of Object.keys(req)) {
                                        const lk = k.toLowerCase();
                                        if (lk.startsWith('x-kpsdk') && !out[lk]) {
                                            out[lk] = req[k];
                                        }
                                    }
                                }
                                return out;
                            })())
                        "#;
                        let kpsdk_json = page
                            .event_loop()
                            .execute_script(kpsdk_headers_js)
                            .unwrap_or_default();
                        let kpsdk: std::collections::HashMap<String, String> =
                            deno_core::serde_json::from_str(&kpsdk_json).unwrap_or_default();
                        if debug_nav && !kpsdk.is_empty() {
                            let keys: Vec<&str> = kpsdk.keys().map(|s| s.as_str()).collect();
                            eprintln!(
                                "[navigate] iter={} harvested x-kpsdk-* headers: {:?}",
                                iter, keys
                            );
                        }

                        current_storage = Some(page.event_loop().get_storage());
                        drop(page);
                        if v8_html_is_real {
                            if debug_nav {
                                eprintln!(
                                    "[navigate] iter={} USING V8-fetched body ({} bytes)",
                                    iter,
                                    v8_html.len()
                                );
                            }
                            current_html = v8_html;
                            last_accept_ch_upgrade = false; // Reset on real content
                        } else {
                            // Reload-style headers + harvested x-kpsdk-*
                            // tokens on the retry GET.
                            let accept_ch_upgraded = if let Ok(u) = url::Url::parse(&current_url) {
                                client.has_accept_ch(u.host_str().unwrap_or_default()).await
                            } else {
                                false
                            };
                            let mut reload_hdrs = net::headers::nav_headers_reload(
                                &profile,
                                &current_url,
                                accept_ch_upgraded,
                            );
                            for (k, v) in &kpsdk {
                                reload_hdrs.push((k.clone(), v.clone()));
                            }
                            let resp = client
                                .get_follow_exact_headers(&current_url, &reload_hdrs, 10)
                                .await
                                .map_err(|e| deno_core::error::AnyError::msg(e.to_string()))?;
                            current_html = resp.text();
                            current_url = resp.url.clone();
                            last_accept_ch_upgrade = resp.accept_ch_upgrade;
                        }
                        continue;
                    }
                }
                return Ok(page);
            }

            let p: deno_core::serde_json::Value =
                deno_core::serde_json::from_str(&pending_info).unwrap_or_default();
            let pending_url = p["url"].as_str().unwrap_or_default();
            let pending_method = p["method"].as_str().unwrap_or("GET").to_string();
            let pending_body = p["body"].as_str().map(|s| s.to_string());
            let kind = p["kind"].as_str().unwrap_or("unknown");

            if pending_url.is_empty() {
                return Ok(page);
            }

            // Resolve relative pending URLs. resolve_url returns None for
            // non-http(s) schemes (about:blank, data:, javascript:, etc.) —
            // those are programmatic JS navigations that don't change the
            // navigable; treat as no-op and return the current page.
            // Caught on iphey.com 2026-05-10: JS sets location.href='about:blank'
            // in an iframe bootstrap, our pending-nav harvester previously
            // bubbled this as a hard error.
            let next_url = match Self::resolve_url(&current_url, pending_url) {
                Some(u) => u,
                None => return Ok(page),
            };
            tracing::debug!(kind = kind, url = %next_url, method = %pending_method, "navigate pending navigation");

            if iter + 1 == iterations {
                tracing::warn!(
                    max_iterations = iterations,
                    "navigate hit max iterations, returning current page"
                );
                return Ok(page);
            }

            // Same MIN_RETRY_BUDGET guard as the cookie-delta path.
            // The pending-nav path consumes the page (drop) and re-fetches.
            // If we don't have at least 15s left to build+drain the next
            // iteration, return the current page to avoid the no-page-FAIL
            // we hit on nike.com (Kasada-marked homepage triggers pending
            // nav, then iter=1 bails before producing anything usable).
            const MIN_PENDING_NAV_BUDGET: Duration = Duration::from_secs(15);
            if nav_budget.saturating_sub(nav_t0.elapsed()) < MIN_PENDING_NAV_BUDGET {
                nav_budget += Duration::from_secs(45);
            }

            // Harvest x-kpsdk-* headers from __fetchLog before dropping the
            // page. The last successful POST /tl response carries a fresh
            // x-kpsdk-ct that the retry GET must forward AS A REQUEST HEADER
            // (per Hyper-Solutions' Go SDK). Cookies alone are not enough.
            let harvested_kpsdk: std::collections::HashMap<String, String> = {
                let js = r#"
                    JSON.stringify((() => {
                        const log = globalThis.__fetchLog || [];
                        const out = {};
                        for (const entry of log) {
                            const resp = entry.respHeaders || {};
                            for (const k of Object.keys(resp)) {
                                if (k.toLowerCase().startsWith('x-kpsdk')) {
                                    out[k.toLowerCase()] = resp[k];
                                }
                            }
                            const req = entry.reqHeaders || {};
                            for (const k of Object.keys(req)) {
                                const lk = k.toLowerCase();
                                if (lk.startsWith('x-kpsdk') && !out[lk]) {
                                    out[lk] = req[k];
                                }
                            }
                        }
                        return out;
                    })())
                "#;
                let j = page.event_loop().execute_script(js).unwrap_or_default();
                deno_core::serde_json::from_str(&j).unwrap_or_default()
            };
            if debug_nav && !harvested_kpsdk.is_empty() {
                let keys: Vec<&str> = harvested_kpsdk.keys().map(|s| s.as_str()).collect();
                eprintln!(
                    "[navigate] iter={} harvested x-kpsdk-* for retry: {:?}",
                    iter, keys
                );
            }

            // In-V8 refetch for same-origin reloads on challenge pages.
            // If the page's own scripts triggered a reload-style navigation
            // (location.href/reload/same-host assign) while challenge markers
            // are still present, the server is likely gating on a token that
            // only an engine-patched window.fetch injects (Kasada x-kpsdk-ct,
            // PerimeterX, etc.). A fresh Rust-side GET bypasses that patch.
            // Try the refetch through the live V8 fetch first; if the result
            // still looks like a challenge, fall back to the normal Rust path.
            let same_host_reload = pending_method == "GET" && page.is_anti_bot_challenge() && {
                let cur = url::Url::parse(&current_url).ok();
                let nxt = url::Url::parse(&next_url).ok();
                matches!(
                    (cur, nxt),
                    (Some(a), Some(b)) if a.host_str() == b.host_str()
                )
            };
            let v8_refetched: Option<String> = if same_host_reload {
                // Post-PoW jitter: real Chrome takes 100-500ms between the
                // challenge solve and the location.reload that follows. Without
                // this gap, Kasada's per-IP rate limiter returns 429 on the
                // refetch (verified 2026-04-27 on hyatt.com). 250ms baseline
                // + small jitter mimics the natural human-action gap.
                let jitter_ms =
                    250 + (std::time::Instant::now().elapsed().as_nanos() & 0xFF) as u64;
                tokio::time::sleep(Duration::from_millis(jitter_ms)).await;
                // Phase 5 Increment 8 (doc 05 §2d "let the bundle
                // self-solve"): a DataDome `rt:'i'` nav sets a reload
                // __pendingNavigation EARLY, so the flow lands here and
                // (pre-fix) reloads after ~250 ms — long before i.js can
                // create the geo.captcha-delivery.com challenge iframe,
                // let it run its WASM boring_challenge + postMessage, and
                // write the `datadome=` cookie. Increment 3's extended
                // challenge poll is gated under `pending_info.is_empty()`
                // so it is SKIPPED on this branch. Give the challenge a
                // bounded self-solve window: pump the event loop and
                // break the instant a `datadome=` cookie appears (the
                // success signal). Narrowly gated to
                // `started_as_dd_challenge` ⇒ false for every non-DataDome
                // site incl. the entire §4 gate ⇒ zero regression.
                if started_as_dd_challenge {
                    let dd_deadline =
                        std::time::Instant::now() + Duration::from_secs(45);
                    let parsed_cur = url::Url::parse(&current_url).ok();
                    while std::time::Instant::now() < dd_deadline {
                        let _ = page
                            .event_loop()
                            .run_until_idle(Duration::from_millis(250))
                            .await;
                        if let Some(p) = parsed_cur.as_ref() {
                            let now = client
                                .cookies_for_url(p)
                                .await
                                .unwrap_or_default();
                            if crate::datadome_handler::cookies_have_datadome(&now) {
                                if debug_nav {
                                    eprintln!(
                                        "[datadome] self-solve window: datadome= cookie acquired, proceeding to reload"
                                    );
                                }
                                break;
                            }
                        }
                    }
                }
                let refetch_js = format!(
                    r#"
                    (async () => {{
                        globalThis.__psrHtml = null;
                        globalThis.__psrStatus = 0;
                        globalThis.__psrErr = null;
                        try {{
                            const resp = await fetch({url_js}, {{
                                method: 'GET',
                                credentials: 'include',
                                headers: {{
                                    'accept': 'text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8',
                                }},
                            }});
                            globalThis.__psrStatus = resp.status;
                            globalThis.__psrHtml = await resp.text();
                        }} catch (e) {{
                            globalThis.__psrErr = String((e && e.message) || e);
                        }}
                    }})();
                    "#,
                    url_js = deno_core::serde_json::to_string(&next_url)
                        .unwrap_or_else(|_| "''".to_string())
                );
                let _ = page
                    .event_loop()
                    .execute_and_run(&refetch_js, Duration::from_secs(15))
                    .await;
                let status = page
                    .event_loop()
                    .execute_script("String(globalThis.__psrStatus || 0)")
                    .unwrap_or_default();
                let err = page
                    .event_loop()
                    .execute_script("String(globalThis.__psrErr || '')")
                    .unwrap_or_default();
                let html = page
                    .event_loop()
                    .execute_script("String(globalThis.__psrHtml || '')")
                    .unwrap_or_default();
                if debug_nav {
                    eprintln!(
                        "[navigate] iter={} in-V8 refetch status={} err={} html_len={}",
                        iter,
                        status,
                        err,
                        html.len()
                    );
                }
                let looks_real = !html.is_empty()
                    && html.len() > current_html.len()
                    && !html.contains("/ips.js")
                    && !html.contains("/149e9513-")
                    && !html.contains("kpsdk")
                    && !html.contains("_abck")
                    && !html.contains("bm_sz");
                if looks_real {
                    Some(html)
                } else {
                    None
                }
            } else {
                None
            };

            current_storage = Some(page.event_loop().get_storage());
            drop(page);

            if let Some(html) = v8_refetched {
                if debug_nav {
                    eprintln!(
                        "[navigate] iter={} USING V8-fetched body ({} bytes)",
                        iter,
                        html.len()
                    );
                }
                current_html = html;
                // current_url unchanged (same origin reload)
                continue;
            }

            if debug_nav {
                eprintln!(
                    "[navigate] iter={} FETCH {} {}",
                    iter, pending_method, next_url
                );
            }

            // Fetch the next page. For form POSTs we must send the form
            // Content-Type or the server can't parse the body. For GETs that
            // are same-origin reload-style navigations (location.href/reload
            // assign from JS), use reload-semantic headers so engines can
            // distinguish a solved-session reload from a fresh user nav.
            let resp = if pending_method == "POST" {
                let post_headers = vec![
                    (
                        "content-type".to_string(),
                        "application/x-www-form-urlencoded".to_string(),
                    ),
                    ("origin".to_string(), {
                        url::Url::parse(&current_url)
                            .ok()
                            .and_then(|u| u.origin().ascii_serialization().into())
                            .unwrap_or_default()
                    }),
                    ("referer".to_string(), current_url.clone()),
                ];
                client
                    .post_bytes_follow(
                        &next_url,
                        pending_body.as_deref().unwrap_or("").as_bytes(),
                        &post_headers,
                        10,
                    )
                    .await
                    .map_err(|e| deno_core::error::AnyError::msg(e.to_string()))?
            } else {
                let same_origin = {
                    let a = url::Url::parse(&current_url).ok();
                    let b = url::Url::parse(&next_url).ok();
                    matches!((a, b), (Some(u), Some(v)) if u.host_str() == v.host_str())
                };
                if same_origin {
                    let accept_ch_upgraded = if let Ok(u) = url::Url::parse(&current_url) {
                        client.has_accept_ch(u.host_str().unwrap_or_default()).await
                    } else {
                        false
                    };
                    let mut reload_hdrs = net::headers::nav_headers_reload(
                        &profile,
                        &current_url,
                        accept_ch_upgraded,
                    );
                    for (k, v) in &harvested_kpsdk {
                        reload_hdrs.push((k.clone(), v.clone()));
                    }
                    let resp = client
                        .get_follow_exact_headers(&next_url, &reload_hdrs, 10)
                        .await
                        .map_err(|e| deno_core::error::AnyError::msg(e.to_string()))?;

                    if resp.status == 498 || resp.status == 403 || resp.status == 429 {
                        eprintln!("[navigate] reload response headers ({}):", resp.status);
                        for (k, v) in &resp.headers {
                            eprintln!("  {}: {}", k, v);
                        }
                    }
                    resp
                } else {
                    client
                        .get_follow(&next_url, 10)
                        .await
                        .map_err(|e| deno_core::error::AnyError::msg(e.to_string()))?
                }
            };
            current_html = resp.text();
            current_url = resp.url.clone();
            last_accept_ch_upgrade = resp.accept_ch_upgrade;
        }

        // Fallback (should be reachable via the loop)
        Err(deno_core::error::AnyError::msg(
            "Navigation loop terminated without returning a page",
        ))
    }

    /// [DEPRECATED] Legacy name — now a thin wrapper around [`Page::navigate`].
    ///
    /// This used to contain Kasada / WBAAS / Akamai-specific logic, which
    /// has been removed in favor of the generic `__pendingNavigation`
    /// primitive. Callers should migrate to `Page::navigate` directly.
    #[deprecated(note = "use Page::navigate instead")]
    pub async fn navigate_with_challenges(
        url: &str,
        profile: stealth::StealthProfile,
        max_retries: u8,
    ) -> Result<Self, deno_core::error::AnyError> {
        Self::navigate(url, profile, max_retries.max(1)).await
    }

    /// Build a page with external script fetching.
    /// Resolve a potentially-relative URL against a base URL.
    fn resolve_url(base: &str, relative: &str) -> Option<String> {
        let base_url = url::Url::parse(base).ok()?;
        let joined = base_url.join(relative).ok()?;
        // We can only fetch http/https. about:blank, data:, blob:,
        // javascript:, chrome-extension:, etc. either have no host
        // (Url::host_str() returns None, causing "no host in URL"
        // downstream) or aren't network-addressable. Filter here so
        // every caller (scripts, iframes, stylesheets, fetches)
        // skips them uniformly. Caught on iphey.com 2026-05-10:
        // about:blank surfaced from a programmatic iframe src.
        match joined.scheme() {
            "http" | "https" => Some(joined.to_string()),
            _ => None,
        }
    }

    async fn build_page_with_scripts(
        html: &str,
        url: &str,
        profile: &stealth::StealthProfile,
        client: &net::HttpClient,
    ) -> Result<Self, deno_core::error::AnyError> {
        Self::build_page_with_scripts_and_init(html, url, profile, client, &[]).await
    }

    async fn build_page_with_scripts_and_init(
        html: &str,
        url: &str,
        profile: &stealth::StealthProfile,
        client: &net::HttpClient,
        init_scripts: &[String],
    ) -> Result<Self, deno_core::error::AnyError> {
        Self::build_page_with_scripts_init_and_storage(
            html,
            url,
            profile,
            client,
            init_scripts,
            None,
        )
        .await
    }

    async fn build_page_with_scripts_init_and_storage(
        html: &str,
        url: &str,
        profile: &stealth::StealthProfile,
        client: &net::HttpClient,
        init_scripts: &[String],
        storage: Option<
            std::collections::HashMap<String, std::collections::HashMap<String, String>>,
        >,
    ) -> Result<Self, deno_core::error::AnyError> {
        let dom = html_parser::parse_html(html);
        let scripts = script_runner::find_scripts(&dom);
        let stylesheet_entries = stylesheet_collector::find_stylesheets(&dom);

        // Fetch ALL external stylesheets in parallel
        let mut inline_css = Vec::new();
        let css_futures: Vec<_> = stylesheet_entries
            .iter()
            .filter_map(|entry| match entry {
                stylesheet_collector::StylesheetEntry::Inline(css) => {
                    inline_css.push(css.clone());
                    None
                }
                stylesheet_collector::StylesheetEntry::External(href) => {
                    let full_url = Self::resolve_url(url, href)?;
                    let client = client.clone();
                    Some(async move {
                        match client.get(&full_url).await {
                            Ok(resp) if resp.ok() => {
                                let text = resp.text();
                                if !text.trim_start().starts_with("<!") {
                                    Some((text, resp.timings.clone()))
                                } else {
                                    None
                                }
                            }
                            _ => {
                                tracing::warn!(url = %full_url, "Failed to fetch stylesheet");
                                None
                            }
                        }
                    })
                }
            })
            .collect();

        // Pre-fetch ALL external scripts in parallel (execute later in document order)
        let script_futures: Vec<_> = scripts
            .iter()
            .enumerate()
            .filter_map(|(i, script)| {
                let src = script.src.as_ref()?;
                let full_url = Self::resolve_url(url, src)?;
                // CSP `script-src-elem` enforcement. Parser-inserted scripts
                // (everything `find_scripts` produces from the initial HTML
                // parse) need a matching nonce to load under
                // `'strict-dynamic'`. Without this gate, browser_oxide
                // fetches Akamai's `/akam/13/<hash>` bootstrap on
                // walmart while real Chrome blocks it — itself a tell.
                if let Ok(parsed_url) = url::Url::parse(&full_url) {
                    if let Err(violated) = js_runtime::extensions::fetch_ext::check_csp(
                        net::csp::Directive::ScriptSrcElem,
                        &parsed_url,
                        script.nonce.as_deref(),
                        true, // parser_inserted: came from HTML parse
                    ) {
                        eprintln!(
                            "[csp] Refused to load the script '{}' because it violates the following Content Security Policy directive: \"{}\".",
                            full_url, violated
                        );
                        return None;
                    }
                }
                let client = client.clone();
                let profile = profile.clone();
                Some(async move {
                    let mut hdrs = net::headers::nav_headers(&profile, false);
                    hdrs.push(("referer".to_string(), url.to_string()));
                    hdrs.push(("accept".to_string(), "*/*".to_string()));
                    hdrs.push(("sec-fetch-dest".to_string(), "script".to_string()));
                    hdrs.push(("sec-fetch-mode".to_string(), "no-cors".to_string()));
                    hdrs.push(("sec-fetch-site".to_string(), "cross-site".to_string()));
                    
                    // Phase 5 instrumentation (doc 05 §2d follow-up):
                    // trace the DataDome i.js external-script fetch so the
                    // next increment has hard evidence of whether the
                    // bundle even loads + its size. Env-gated, default
                    // off ⇒ zero behavioral/perf/log change to the §4
                    // gate.
                    let dd_trace = full_url.contains("captcha-delivery.com")
                        && std::env::var("BOXIDE_DD_TRACE").is_ok();
                    // Phase 5 (homedepot): trace EVERY external-script
                    // fetch when BOXIDE_SC_TRACE is set, so we can see
                    // whether the obfuscated `/Wjv3…` sec-cpt bundle is
                    // actually fetched + its size/status (the unknown the
                    // "bundle doesn't self-solve" verdict assumed but
                    // never measured). Env-gated, default off ⇒ zero §4
                    // gate impact.
                    let sc_trace = std::env::var("BOXIDE_SC_TRACE").is_ok();
                    match client.get_follow_with_headers(&full_url, &hdrs, 5).await {
                        Ok(resp) if resp.ok() => {
                            let text = resp.text();
                            if dd_trace {
                                eprintln!(
                                    "[datadome-trace] i.js fetch OK {} status={} bytes={}",
                                    full_url,
                                    resp.status,
                                    text.len()
                                );
                            }
                            if sc_trace {
                                eprintln!(
                                    "[seccpt-trace] script fetch OK {} status={} bytes={}",
                                    full_url,
                                    resp.status,
                                    text.len()
                                );
                            }
                            if full_url.contains("qauth") || full_url.contains("ips.js") || full_url.contains("antibot") {
                                let safe_name = full_url.replace("/", "_").replace(":", "_").replace("?", "_");
                                let _ = std::fs::write(format!("oxide_dump/{}", safe_name), &text);
                            }
                            if text.trim_start().starts_with("<!")
                                || text.trim_start().starts_with("<html")
                            {
                                tracing::debug!(script_index = i, url = %full_url, "Script fetch returned HTML, skipping");
                                None
                            } else {
                                Some((i, text, resp.timings.clone()))
                            }
                        }
                        Ok(resp) => {
                            if dd_trace {
                                eprintln!(
                                    "[datadome-trace] i.js fetch NON-OK {} status={}",
                                    full_url, resp.status
                                );
                            }
                            if sc_trace {
                                eprintln!(
                                    "[seccpt-trace] script fetch NON-OK {} status={}",
                                    full_url, resp.status
                                );
                            }
                            tracing::warn!(script_index = i, url = %full_url, status = resp.status, "Script fetch returned non-OK status");
                            None
                        }
                        Err(e) => {
                            if dd_trace {
                                eprintln!(
                                    "[datadome-trace] i.js fetch ERR {} err={:?}",
                                    full_url, e
                                );
                            }
                            tracing::warn!(script_index = i, url = %full_url, error = ?e, "Script fetch failed");
                            None
                        }
                    }
                })
            })
            .collect();

        // Await all fetches in parallel
        let (fetched_css_results, fetched_scripts_results) = futures_util::future::join(
            futures_util::future::join_all(css_futures),
            futures_util::future::join_all(script_futures),
        )
        .await;

        let mut all_timings = Vec::new();

        // Build stylesheet list: inline first, then fetched external
        let mut stylesheets = inline_css;
        for result in fetched_css_results {
            if let Some((css, timings)) = result {
                stylesheets.push(css);
                all_timings.push(timings);
            }
        }

        // Build pre-fetched script map
        let mut prefetched = std::collections::HashMap::new();
        for result in fetched_scripts_results {
            if let Some((i, text, timings)) = result {
                prefetched.insert(i, text);
                all_timings.push(timings);
            }
        }

        let runtime = BrowserJsRuntime::with_options(
            dom,
            BrowserRuntimeOptions {
                stealth_profile: Some(profile.clone()),
                stylesheets,
                init_scripts: init_scripts.to_vec(),
                storage,
                is_secure_context: is_secure_url(url),
                ..Default::default()
            },
        );
        let mut event_loop = BrowserEventLoop::new(runtime);

        // Install all sub-resource timings
        for timings in all_timings {
            event_loop.runtime_mut().record_resource_timing(timings);
        }

        // Install a build-phase V8 deadline watcher to preempt CPU-bound
        // inline-script execution (delta.com, taobao.com — pages whose
        // first-paint scripts spawn document.write(<script>) chains or
        // tight setTimeout polling that hold the V8 thread indefinitely).
        // 25s is generous: any honest first-paint completes well under it.
        // Without this, build_page_with_scripts_and_init can run forever
        // because tokio::time::timeout cannot preempt V8 microtask spins.
        let build_budget_ms: u64 = std::env::var("BOXIDE_BUILD_BUDGET_MS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(25_000);
        let _build_watcher = V8DeadlineWatcher::new(
            event_loop.runtime_mut().isolate_handle(),
            Duration::from_millis(build_budget_ms),
        );

        // Set location (URL-state setup, not a real navigation —
        // reset the nav-pending signal so subsequent run_until_idle calls
        // don't short-circuit immediately, see crates/event_loop).
        let url_js = url.replace('\\', "\\\\").replace('\'', "\\'");
        if let Err(e) = event_loop.execute_script(&format!("location.href = '{}';", url_js)) {
            tracing::error!(error = %e, "Failed to set location");
        }
        event_loop.reset_nav_pending();
        let loc = event_loop
            .execute_script("globalThis.location.href")
            .unwrap_or_default();
        tracing::debug!(location = %loc, "Location set");

        // Synchronize cookies from the net client so document.cookie is accurate
        let _ = event_loop
            .execute_and_run(
                "globalThis.__syncCookiesFromNet && globalThis.__syncCookiesFromNet();",
                Duration::from_secs(1),
            )
            .await;

        // Install cookie-write instrumentation. Generic DevTools-style
        // debugging — lets us see what values scripts assign to
        // `document.cookie` during the page run.
        event_loop
            .execute_script(r#"Object.defineProperty(window, '__cookieWrites', { value: [], enumerable: false, configurable: true });
            (function() {
                const proto = Document.prototype || (document && Object.getPrototypeOf(document));
                if (!proto) return;
                const desc = Object.getOwnPropertyDescriptor(proto, 'cookie');
                if (!desc || !desc.set) return;
                const origSet = desc.set;
                const origGet = desc.get;
                Object.defineProperty(proto, 'cookie', {
                    configurable: true,
                    enumerable: desc.enumerable,
                    get: function() { 
                        if (globalThis.__scriptErrors) {
                            globalThis.__scriptErrors.push('[INSTRUMENT] GET document.cookie');
                        }
                        return origGet ? origGet.call(this) : ''; 
                    },
                    set: function(v) {
                        try {
                            if (window.__cookieWrites.length < 100) {
                                window.__cookieWrites.push(String(v).substring(0, 300));
                            }
                        } catch (e) {}
                        return origSet.call(this, v);
                    },
                });
            })();"#)
            .ok();

        // Install error tracking + fetch/XHR logging BEFORE scripts run.
        // Generic request log, equivalent to DevTools' Network tab.
        event_loop
            .execute_script(r#"Object.defineProperty(window, '__scriptErrors', { value: [], enumerable: false, configurable: true });
            // Temporarily disable the stack filter so we can see the real
            // call sites when a TypeError fires inside a challenge VM.
            delete Error.prepareStackTrace;
            window.onerror = function(msg, src, line, col, err) {
                window.__scriptErrors.push(msg + ' @' + (src||'?') + ':' + line + '\n' + (err && err.stack || '').substring(0, 800));
            };
            window.addEventListener('unhandledrejection', function(e) {
                window.__scriptErrors.push('REJECT:' + String(e.reason).substring(0,200));
            });
            const _origFetch = globalThis.fetch;
            globalThis.fetch = async function(input, init) {
                const log = globalThis._boxide && globalThis._boxide.__fetchLog;
                const entry = { method: 'GET', url: '', hasBody: false };
                let args = Array.from(arguments);

                // Pre-check: reject non-fetch URL schemes BEFORE the logging
                // try/catch below (which silently swallows). Real Chrome
                // throws TypeError for fetch("ftp:"), fetch("file:"), etc.
                // ips.js uses fetch("ftp:") as a browser-authenticity canary.
                (() => {
                    let pre = '';
                    if (typeof args[0] === 'string') pre = args[0];
                    else if (args[0] && typeof args[0].url === 'string') pre = args[0].url;
                    else if (args[0] instanceof URL) pre = args[0].href;
                    const m = pre && pre.match(/^([a-z][a-z0-9+.-]*):/i);
                    if (m) {
                        const sch = m[1].toLowerCase();
                        if (!['http','https','data','blob','file','about'].includes(sch)) {
                            throw new TypeError("Failed to fetch: URL scheme \"" + sch + "\" is not supported.");
                        }
                    }
                })();

                try {
                    let urlStr = '';
                    let isRequest = false;
                    if (typeof args[0] === 'string') {
                        urlStr = args[0];
                    } else if (args[0] && typeof args[0].url === 'string') {
                        urlStr = args[0].url;
                        isRequest = true;
                    } else if (args[0] instanceof URL) {
                        urlStr = args[0].href;
                    }
                    
                    // Skip URL resolution if urlStr is already absolute
                    // (has a scheme). This prevents our URL polyfill from
                    // treating "ftp:" as a relative path.
                    const _schemeMatch = urlStr ? urlStr.match(/^([a-z][a-z0-9+.-]*):/i) : null;
                    const _scheme = _schemeMatch ? _schemeMatch[1].toLowerCase() : '';
                    if (urlStr && !_scheme) {
                        try {
                            let base = globalThis.location ? globalThis.location.href : 'about:blank';
                            if (base === 'about:blank' || base === 'javascript:;' || base === '') {
                                try { base = globalThis.parent.location.href; } catch(e) {}
                            }
                            urlStr = new URL(urlStr, base).href;
                            if (isRequest) {
                                // Recreate Request with absolute URL. Preserve all properties from the original.
                                args[0] = new Request(urlStr, args[0]);
                            } else {
                                args[0] = urlStr;
                            }
                        } catch(e) {
                            if (globalThis.__scriptErrors) {
                                globalThis.__scriptErrors.push('fetch url resolve error: ' + e.message);
                            }
                        }
                    }
                    entry.url = String(urlStr || '').substring(0, 200);
                    entry.method = (init && init.method) || (isRequest && args[0].method) || 'GET';
                    entry.hasBody = !!((init && init.body) || (isRequest && args[0].body));
                    // Capture request body for error reporter diagnosis.
                    if (init && init.body != null) {
                        try {
                            const b = init.body;
                            if (typeof b === 'string') {
                                entry.body = b.substring(0, 1000);
                            } else if (b instanceof ArrayBuffer || ArrayBuffer.isView(b)) {
                                const u8 = b instanceof Uint8Array ? b : new Uint8Array(b.buffer || b, b.byteOffset || 0, b.byteLength);
                                let s = '';
                                const max = Math.min(u8.length, 400);
                                for (let i = 0; i < max; i++) s += String.fromCharCode(u8[i]);
                                entry.body = '[bytes:' + u8.length + '] ' + s;
                            } else {
                                entry.body = String(b).substring(0, 400);
                            }
                        } catch {}
                    }
                    const hdrs = {};
                    const h = (init && init.headers) || {};
                    if (h && typeof h.forEach === 'function') {
                        h.forEach((v, k) => { hdrs[k] = String(v); });
                    } else if (h) {
                        for (const k in h) hdrs[k] = String(h[k]);
                    }
                    entry.reqHeaders = hdrs;
                } catch {}
                const log = globalThis._boxide && globalThis._boxide.__fetchLog;
                if (log) log.push(entry);
                try {
                    const resp = await _origFetch.apply(this, args);
                    entry.status = resp.status;
                    try {
                        const respHdrs = {};
                        if (resp.headers && typeof resp.headers.forEach === 'function') {
                            resp.headers.forEach((v, k) => { respHdrs[String(k).toLowerCase()] = String(v).substring(0, 300); });
                        } else if (resp.headers) {
                            for (const k in resp.headers) {
                                respHdrs[String(k).toLowerCase()] = String(resp.headers[k]).substring(0, 300);
                            }
                        }
                        entry.respHeaders = respHdrs;
                    } catch {}
                    return resp;
                } catch (e) {
                    entry.error = String(e && e.message || e).substring(0, 200);
                    throw e;
                }
            };
            // Also wrap XMLHttpRequest.send so XHR requests appear in __fetchLog.
            // This is critical for SDKs like WBAAS that use sync XHR for token fetches.
            (function() {
                const _XHR = globalThis.XMLHttpRequest;
                if (!_XHR) return;
                const _origOpen = _XHR.prototype.open;
                const _origSend = _XHR.prototype.send;
                _XHR.prototype.open = function(method, url, async) {
                    this.__logEntry = { method: String(method||'GET').toUpperCase(), url: String(url||''), sync: async === false };
                    return _origOpen.apply(this, arguments);
                };
                _XHR.prototype.send = function(body) {
                    const entry = this.__logEntry || { method: this._method||'GET', url: this._url||'', sync: !this._async };
                    entry.hasBody = body != null && body !== '';
                    const log = globalThis._boxide && globalThis._boxide.__fetchLog;
                if (log) log.push(entry);
                    const _origRSC = this.onreadystatechange;
                    const self = this;
                    const _finish = function() {
                        if (self.readyState === 4) entry.status = self.status;
                    };
                    const prev = this.onreadystatechange;
                    this.onreadystatechange = function() {
                        _finish();
                        if (prev) prev.apply(this, arguments);
                    };
                    return _origSend.apply(this, arguments);
                };
            })();"#)
            .ok();

        // Execute scripts in document order using pre-fetched code.
        // Interleave with event loop ticks to allow for microtasks and
        // macrotasks scheduled by one script to run before the next.
        for (i, script) in scripts.iter().enumerate() {
            let code = if script.src.is_some() {
                match prefetched.get(&i) {
                    Some(code) => code.clone(),
                    None => {
                        tracing::warn!(
                            script_index = i,
                            "Script not prefetched (fetch failed), skipping"
                        );
                        continue;
                    }
                }
            } else {
                script.code.clone()
            };

            if code.trim().is_empty() {
                continue;
            }

            let name = if let Some(src) = &script.src {
                src.clone()
            } else {
                // W2.7 — real Chrome inline <script> stack frames report
                // the document URL, not a synthetic <script_N> tag. The
                // latter would leak the index/wrapper layer to Kasada /
                // DataDome (research 09_KASADA_DEEP_2026_05_14.md §9).
                url.to_string()
            };
            if let Err(e) = event_loop.execute_script_with_name(&code, &name) {
                tracing::warn!(script = %name, error = %e, "Script execution error");
            }

            // Flush logs for this script
            {
                if let Some(src) = &script.src {
                    if src.contains("akam") || src.contains("ips.js") || src.contains("kpsdk") {
                        eprintln!("[JS LOG] script found: {}", src);

                        // Extract Kasada tenant prefix and im token
                        if src.contains("/ips.js") {
                            let parts: Vec<&str> = src.split("/ips.js").collect();
                            if !parts[0].is_empty() {
                                if let Ok(u) = url::Url::parse(url) {
                                    if let Some(host) = u.host_str() {
                                        // Parse x-kpsdk-im from the script URL if present
                                        let im = if let Some(query) = src.split('?').nth(1) {
                                            query
                                                .split('&')
                                                .find(|p| p.starts_with("x-kpsdk-im="))
                                                .map(|p| p[11..].to_string())
                                        } else {
                                            None
                                        };

                                        let client_clone = client.clone();
                                        let host_str = host.to_string();
                                        let prefix_str = parts[0].to_string();
                                        tokio::spawn(async move {
                                            client_clone
                                                .learn_kasada_prefix(&host_str, &prefix_str)
                                                .await;
                                            if let Some(im_val) = im {
                                                client_clone
                                                    .kasada_sessions()
                                                    .store_im(&host_str, im_val)
                                                    .await;
                                            }
                                            // Realistic DT token (captured from real browser)
                                            client_clone.kasada_sessions().store_dt(&host_str, "11qox8sw33mzd5rx62nvw43pjz99vza39w0a3lycjlwbby5126x2thw75s".to_string()).await;

                                            // Trigger /mfc fetch if needed
                                            client_clone
                                                .fetch_kasada_mfc_if_needed(&host_str)
                                                .await;
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
                let logs = {
                    let runtime = event_loop.runtime_mut().inner();
                    let state = runtime.op_state();
                    let mut state = state.borrow_mut();
                    let dom_state = state.borrow_mut::<js_runtime::state::DomState>();
                    std::mem::take(&mut dom_state.console_output)
                };
                for log in logs {
                    let prefix = match log.level {
                        js_runtime::state::ConsoleLevel::Log => "[JS LOG]",
                        js_runtime::state::ConsoleLevel::Warn => "[JS WARN]",
                        js_runtime::state::ConsoleLevel::Error => "[JS ERROR]",
                        _ => "[JS INFO]",
                    };
                    tracing::debug!(level = prefix, message = %log.args.join(" "), "JS console output");
                }
            }

            // Run loop for a short burst between scripts to flush tasks
            let _ = event_loop.run_until_idle(Duration::from_millis(50)).await;
        }

        // Final cleanup — hides Deno and internal globals from user JS.
        event_loop
            .execute_script(include_str!("../../js_runtime/src/js/cleanup_bootstrap.js"))
            .ok();

        // Fire DOMContentLoaded and load events via setTimeout so they execute
        // within the event loop (not synchronously during script setup).
        // This ensures async handlers can create Promises that the event loop tracks.
        event_loop
            .execute_script(
                r#"
            setTimeout(() => {
                document.dispatchEvent(new Event('DOMContentLoaded', {bubbles: true}));
                window.dispatchEvent(new Event('DOMContentLoaded', {bubbles: true}));
                window.dispatchEvent(new Event('load'));
            }, 0);
        "#,
            )
            .ok();

        // Scan for <meta http-equiv="refresh" content="N;url=..."> and
        // schedule a pending navigation. Generic navigation primitive —
        // the Rust driver loop sees __pendingNavigation and re-fetches.
        event_loop
            .execute_script(r#"
            (function() {
                const metas = document.getElementsByTagName('meta');
                for (let i = 0; i < metas.length; i++) {
                    const m = metas[i];
                    const equiv = String(m.getAttribute('http-equiv') || '').toLowerCase();
                    if (equiv !== 'refresh') continue;
                    const content = String(m.getAttribute('content') || '');
                    const match = content.match(/^\s*(\d+)(?:\s*[;,]\s*url\s*=\s*(.+))?$/i);
                    if (!match) continue;
                    const delay = parseInt(match[1], 10) || 0;
                    const target = ((match[2] || '').trim()).replace(/^['"]|['"]$/g, '') || location.href;
                    setTimeout(() => {
                        globalThis.__pendingNavigation = {
                            url: target,
                            kind: 'assign',
                        };
                        // Wake the Rust event loop — see nav_ext.rs.
                        try { Deno.core.ops.op_set_pending_nav(); } catch (_) {}
                    }, delay * 1000);
                    break;
                }
            })();
        "#)
            .ok();

        // Run event loop until idle. Script errors should NOT abort
        // navigation — log and continue, matching real browser behavior.
        // 8s cap (was 30s) — see comment in navigate_with_init.
        if let Err(e) = event_loop.run_until_idle(Duration::from_secs(8)).await {
            tracing::warn!(error = %e, "Event loop error during run");
        }

        // Log errors captured during script execution
        if let Ok(errors) = event_loop.execute_script("JSON.stringify(window.__scriptErrors || [])")
        {
            if errors != "[]" {
                let trimmed: String = errors.chars().take(500).collect();
                tracing::warn!(errors = %trimmed, "Script errors during page run");
            }
        }

        // Dump any cookie-set assignments that scripts made during the run.
        if let Ok(cookie_writes) =
            event_loop.execute_script("JSON.stringify(window.__cookieWrites || [])")
        {
            if cookie_writes != "[]" && !cookie_writes.is_empty() {
                use deno_core::serde_json;
                if let Ok(arr) = serde_json::from_str::<serde_json::Value>(&cookie_writes) {
                    if let Some(arr) = arr.as_array() {
                        tracing::debug!(count = arr.len(), "Cookie writes");
                        for (i, w) in arr.iter().take(20).enumerate() {
                            if let Some(s) = w.as_str() {
                                let trim: String = s.chars().take(140).collect();
                                tracing::debug!(index = i, value = %trim, "Cookie write");
                            }
                        }
                    }
                }
            }
        }
        // Dump a one-line summary of every fetch the page made during
        // the run — equivalent to DevTools' Network tab.
        if let Ok(fetches_json) = event_loop.execute_script(
            r#"JSON.stringify((window.__fetchLog || []).map(f => ({
                m: f.method,
                u: f.url,
                s: f.status,
                e: f.error,
            })))"#,
        ) {
            if fetches_json != "[]" {
                use deno_core::serde_json;
                if let Ok(arr) = serde_json::from_str::<serde_json::Value>(&fetches_json) {
                    if let Some(arr) = arr.as_array() {
                        tracing::debug!(count = arr.len(), "Page fetches");
                        for f in arr {
                            let m = f.get("m").and_then(|v| v.as_str()).unwrap_or("");
                            let u = f.get("u").and_then(|v| v.as_str()).unwrap_or("");
                            let s = f.get("s").and_then(|v| v.as_u64()).unwrap_or(0);
                            let e = f.get("e").and_then(|v| v.as_str()).unwrap_or("");
                            let u_trim: String = u.chars().take(100).collect();
                            if s == 0 {
                                tracing::warn!(method = m, status = s, url = %u_trim, error = e, "Page fetch failed");
                            } else {
                                tracing::debug!(method = m, status = s, url = %u_trim, "Page fetch");
                            }
                        }
                    }
                }
            }
        }

        // Process iframes (srcdoc and src)
        let mut children = Vec::new();
        let iframes = {
            let dom_ref = event_loop.runtime_mut().inner();
            let state = dom_ref.op_state();
            let state = state.borrow();
            let dom_state = state.borrow::<js_runtime::state::DomState>();
            iframe::find_iframes(&dom_state.dom)
        };
        for info in &iframes {
            if let Some(srcdoc) = &info.srcdoc {
                match iframe::ChildIframe::from_srcdoc(info.node_id, srcdoc, &profile).await {
                    Ok(child) => children.push(child),
                    Err(e) => tracing::warn!(error = %e, "iframe srcdoc error"),
                }
            } else if let Some(src) = &info.src {
                if !src.is_empty() && !src.starts_with("javascript:") {
                    if let Some(full_src) = Self::resolve_url(url, src) {
                        match iframe::ChildIframe::from_url(
                            info.node_id,
                            &full_src,
                            client,
                            Some(profile),
                        )
                        .await
                        {
                            Ok(child) => children.push(child),
                            Err(e) => {
                                tracing::warn!(src = %full_src, error = %e, "iframe src error")
                            }
                        }
                    }
                } else if src.starts_with("javascript:") {
                    // javascript:; or similar — create a blank frame so it can be written to
                    match iframe::ChildIframe::from_srcdoc(
                        info.node_id,
                        "<!DOCTYPE html><html><body></body></html>",
                        &profile,
                    )
                    .await
                    {
                        Ok(child) => children.push(child),
                        Err(e) => tracing::warn!(error = %e, "iframe javascript blank error"),
                    }
                }
            }
        }

        // Cancel the build-phase watcher's terminate so the runtime is
        // usable for the drain phase (and downstream execute_script calls).
        // Drop the watcher first to stop the thread; then cancel any
        // pending termination on the isolate.
        drop(_build_watcher);
        event_loop.runtime_mut().cancel_terminate_execution();

        Ok(Self {
            event_loop,
            url: url.to_string(),
            children,
        })
    }

    /// Consume the page and return the DOM.
    pub fn take_dom(mut self) -> Dom {
        // Drop children first (V8 reverse order requirement)
        self.children.clear();
        // Use ManuallyDrop to prevent the Drop impl from running
        let mut page = std::mem::ManuallyDrop::new(self);
        // Safe: we manually cleared children above, now take event_loop
        unsafe {
            let event_loop = std::ptr::read(&page.event_loop);
            event_loop.take_dom()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn page_from_html_basic() {
        let mut page = Page::from_html(
            "<html><head><title>Test</title></head><body><p>Hello</p></body></html>",
            None::<stealth::StealthProfile>,
        )
        .await
        .unwrap();
        assert_eq!(page.title(), "Test");
        assert_eq!(page.text_of("p"), Some("Hello".to_string()));
    }

    #[tokio::test]
    async fn page_script_execution() {
        let mut page = Page::from_html("<html><head></head><body><div id='target'></div><script>document.getElementById('target').textContent = 'JS works!';</script></body></html>", None::<stealth::StealthProfile>).await.unwrap();
        assert_eq!(page.text_of("#target"), Some("JS works!".to_string()));
    }

    #[tokio::test]
    async fn page_script_creates_elements() {
        let mut page = Page::from_html(
            r#"<html><head></head><body>
                <script>
                    const p = document.createElement('p');
                    p.setAttribute('id', 'created');
                    p.textContent = 'Dynamic content';
                    document.body.appendChild(p);
                </script>
            </body></html>"#,
            None::<stealth::StealthProfile>,
        )
        .await
        .unwrap();
        assert!(page.has_element("#created"));
        assert_eq!(
            page.text_of("#created"),
            Some("Dynamic content".to_string())
        );
    }

    #[tokio::test]
    async fn page_script_modifies_inner_html() {
        let mut page = Page::from_html(r#"<html><head></head><body>
                <div id="container"></div>
                <script>
                    document.getElementById('container').innerHTML = '<span class="inner">Injected</span>';
                </script>
            </body></html>"#, None::<stealth::StealthProfile>)
        .await
        .unwrap();
        assert_eq!(page.text_of(".inner"), Some("Injected".to_string()));
    }

    #[tokio::test]
    async fn page_with_timeout_script() {
        let mut page = Page::from_html(
            r#"<html><head></head><body>
                <div id="output">before</div>
                <script>
                    setTimeout(() => {
                        document.getElementById('output').textContent = 'after';
                    }, 50);
                </script>
            </body></html>"#,
            None::<stealth::StealthProfile>,
        )
        .await
        .unwrap();
        assert_eq!(page.text_of("#output"), Some("after".to_string()));
    }

    #[tokio::test]
    async fn page_evaluate() {
        let mut page = Page::from_html(
            "<html><head></head><body></body></html>",
            None::<stealth::StealthProfile>,
        )
        .await
        .unwrap();
        let result = page.evaluate("1 + 2").unwrap();
        assert_eq!(result, "3");
    }

    #[tokio::test]
    async fn page_navigator_exists() {
        let mut page = Page::from_html(
            "<html><head></head><body></body></html>",
            None::<stealth::StealthProfile>,
        )
        .await
        .unwrap();
        let result = page.evaluate("typeof navigator.userAgent").unwrap();
        assert_eq!(result, "string");
    }

    #[tokio::test]
    async fn page_document_has_focus() {
        let mut page = Page::from_html(
            "<html><head></head><body></body></html>",
            None::<stealth::StealthProfile>,
        )
        .await
        .unwrap();
        let result = page.evaluate("document.hasFocus()").unwrap();
        assert_eq!(result, "true");
    }

    #[tokio::test]
    async fn page_webdriver_undefined() {
        // Real non-automated Chrome: navigator.webdriver is undefined (W3C spec).
        let mut page = Page::from_html(
            "<html><head></head><body></body></html>",
            None::<stealth::StealthProfile>,
        )
        .await
        .unwrap();
        let result = page.evaluate("typeof navigator.webdriver").unwrap();
        assert_eq!(result, "undefined");
        let val = page.evaluate("navigator.webdriver").unwrap();
        assert_eq!(val, "undefined");
    }

    #[tokio::test]
    async fn page_window_dimensions() {
        let mut page = Page::from_html(
            "<html><head></head><body></body></html>",
            None::<stealth::StealthProfile>,
        )
        .await
        .unwrap();
        let w = page.evaluate("window.innerWidth").unwrap();
        assert_eq!(w, "1920");
        let h = page.evaluate("window.innerHeight").unwrap();
        assert_eq!(h, "1080");
    }

    /// Akamai pixel POST captured `client=[1914,28638]` for documentElement
    /// — full document size, not viewport. Real Chrome returns viewport
    /// (innerWidth × innerHeight). Regression-locks the dom_bootstrap
    /// HTMLHtmlElement.prototype clientWidth/Height override.
    #[tokio::test]
    async fn document_element_client_dims_are_viewport_clipped() {
        let mut page = Page::from_html(
            "<html><head></head><body><div style=\"height:50000px\"></div></body></html>",
            None::<stealth::StealthProfile>,
        )
        .await
        .unwrap();
        let cw = page
            .evaluate("document.documentElement.clientWidth")
            .unwrap();
        let ch = page
            .evaluate("document.documentElement.clientHeight")
            .unwrap();
        assert_eq!(
            cw, "1920",
            "documentElement.clientWidth must equal innerWidth, got {cw}"
        );
        assert_eq!(
            ch, "1080",
            "documentElement.clientHeight must equal innerHeight, got {ch}"
        );
    }

    /// Akamai sensor probes `window.ApplePaySession` on macOS UA. Real
    /// Chrome on macOS exposes the constructor; absence is a hard tell.
    /// Regression-locks the macOS-conditional shim in window_bootstrap.
    #[tokio::test]
    async fn apple_pay_session_present_on_macos_profile() {
        // Phase 7 — ApplePaySession is gated on isSecureContext so the
        // page must be loaded over https:// for the macOS shim to install.
        let profile = stealth::presets::chrome_130_macos();
        let mut page = Page::from_html_with_url(
            "<html><head></head><body></body></html>",
            "https://example.com/",
            Some(profile),
        )
        .await
        .unwrap();
        let t = page.evaluate("typeof ApplePaySession").unwrap();
        assert_eq!(
            t, "function",
            "macOS profile must expose ApplePaySession constructor"
        );
        let cmp = page.evaluate("ApplePaySession.canMakePayments()").unwrap();
        assert_eq!(cmp, "true");
        let v = page.evaluate("ApplePaySession.supportsVersion(3)").unwrap();
        assert_eq!(v, "true");
    }

    #[tokio::test]
    async fn apple_pay_session_absent_on_windows_profile() {
        let profile = stealth::presets::chrome_130_windows();
        let mut page = Page::from_html("<html><head></head><body></body></html>", Some(profile))
            .await
            .unwrap();
        let t = page.evaluate("typeof ApplePaySession").unwrap();
        assert_eq!(
            t, "undefined",
            "Windows profile must NOT expose ApplePaySession"
        );
    }

    /// macOS profile: Helvetica Neue and Arial are both installed,
    /// each must produce a distinct width from sans-serif baseline AND
    /// from each other.
    #[tokio::test]
    async fn canvas_font_detection_macos_helvetica_neue() {
        let profile = stealth::presets::chrome_130_macos();
        let mut page = Page::from_html(
            "<html><head></head><body><canvas id=\"c\" width=\"200\" height=\"50\"></canvas></body></html>",
            Some(profile),
        )
        .await
        .unwrap();
        let script = r#"
            (() => {
                const ctx = document.getElementById('c').getContext('2d');
                ctx.font = "16px sans-serif";
                const a = ctx.measureText("mmmmmmmmmlli").width;
                ctx.font = "16px Arial";
                const b = ctx.measureText("mmmmmmmmmlli").width;
                ctx.font = "16px 'Helvetica Neue'";
                const c = ctx.measureText("mmmmmmmmmlli").width;
                ctx.font = "16px Calibri";
                const d = ctx.measureText("mmmmmmmmmlli").width;
                return JSON.stringify({
                    arial_installed: Math.abs(a-b) > 1e-3,
                    hn_installed: Math.abs(a-c) > 1e-3,
                    distinct_from_arial: Math.abs(b-c) > 1e-3,
                    calibri_not_installed_on_macos: Math.abs(a-d) < 1e-3,
                });
            })()
        "#;
        let r = page.evaluate(script).unwrap();
        assert!(
            r.contains("\"arial_installed\":true"),
            "Arial must be installed on macOS: {r}"
        );
        assert!(
            r.contains("\"hn_installed\":true"),
            "Helvetica Neue must be installed on macOS: {r}"
        );
        assert!(
            r.contains("\"distinct_from_arial\":true"),
            "HN must differ from Arial: {r}"
        );
        assert!(
            r.contains("\"calibri_not_installed_on_macos\":true"),
            "Calibri must not be installed on macOS: {r}"
        );
    }

    /// Canvas-based font detection: measureText widths must differ between
    /// distinct named families and the bare generic, otherwise sensors
    /// (Akamai, Kasada) report `fonts=null`. The dom canvas2d backend
    /// aliases everything to Liberation Sans; the canvas_bootstrap shim
    /// adds a deterministic per-family micro-delta to keep widths unique.
    #[tokio::test]
    async fn canvas_measure_text_distinguishes_named_fonts() {
        let mut page = Page::from_html(
            "<html><head></head><body><canvas id=\"c\" width=\"200\" height=\"50\"></canvas></body></html>",
            None::<stealth::StealthProfile>,
        )
        .await
        .unwrap();
        let script = r#"
            (() => {
                const ctx = document.getElementById('c').getContext('2d');
                ctx.font = "16px sans-serif";
                const a = ctx.measureText("mmmmmmmmmlli").width;
                ctx.font = "16px Arial";
                const b = ctx.measureText("mmmmmmmmmlli").width;
                ctx.font = "16px 'Helvetica Neue'";
                const c = ctx.measureText("mmmmmmmmmlli").width;
                return JSON.stringify({a, b, c, ab: Math.abs(a-b) > 1e-3, bc: Math.abs(b-c) > 1e-3});
            })()
        "#;
        let r = page.evaluate(script).unwrap();
        assert!(
            r.contains("\"ab\":true"),
            "Arial must measure differently than sans-serif: {r}"
        );
        assert!(
            r.contains("\"bc\":true"),
            "Helvetica Neue must measure differently than Arial: {r}"
        );
    }

    #[tokio::test]
    async fn page_local_storage() {
        let mut page = Page::from_html(
            "<html><head></head><body></body></html>",
            None::<stealth::StealthProfile>,
        )
        .await
        .unwrap();
        page.evaluate("localStorage.setItem('key', 'value')")
            .unwrap();
        let result = page.evaluate("localStorage.getItem('key')").unwrap();
        assert_eq!(result, "value");
    }

    #[tokio::test]
    async fn page_crypto_random() {
        let mut page = Page::from_html(
            "<html><head></head><body></body></html>",
            None::<stealth::StealthProfile>,
        )
        .await
        .unwrap();
        let result = page
            .evaluate("typeof crypto.getRandomValues(new Uint8Array(4))")
            .unwrap();
        assert_eq!(result, "object");
    }

    #[tokio::test]
    async fn page_promise_then() {
        let mut page = Page::from_html(
            r#"<html><head></head><body>
                <div id="out">waiting</div>
                <script>
                    Promise.resolve('done').then(v => {
                        document.getElementById('out').textContent = v;
                    });
                </script>
            </body></html>"#,
            None::<stealth::StealthProfile>,
        )
        .await
        .unwrap();
        assert_eq!(page.text_of("#out"), Some("done".to_string()));
    }

    #[tokio::test]
    async fn page_multiple_scripts() {
        let mut page = Page::from_html(
            r#"<html><head></head><body>
                <div id="out"></div>
                <script>document.getElementById('out').textContent = 'A';</script>
                <script>document.getElementById('out').textContent += 'B';</script>
                <script>document.getElementById('out').textContent += 'C';</script>
            </body></html>"#,
            None::<stealth::StealthProfile>,
        )
        .await
        .unwrap();
        assert_eq!(page.text_of("#out"), Some("ABC".to_string()));
    }

    #[tokio::test]
    async fn page_take_dom() {
        let page = Page::from_html(
            "<html><head></head><body><p>test</p></body></html>",
            None::<stealth::StealthProfile>,
        )
        .await
        .unwrap();
        let dom = page.take_dom();
        let ps = dom.get_elements_by_tag_name(dom::NodeId::DOCUMENT, "p");
        assert!(ps.len() >= 1, "expected at least 1 <p>, got {}", ps.len());
        assert_eq!(dom.text_content(ps[0]), "test");
    }

    // --- Network integration tests (require internet) ---

    #[tokio::test]
    #[ignore]
    async fn navigate_httpbin() {
        let profile = stealth::chrome_130_linux();
        let client = net::HttpClient::new(&profile).unwrap();
        let mut page = Page::navigate_simple(
            "https://httpbin.org/html",
            &client,
            stealth::presets::chrome_130_ru(),
        )
        .await
        .expect("navigate to httpbin failed");
        let title = page.title();
        println!("[httpbin] title: {title:?}");
        let text = page.text_content();
        println!("[httpbin] body length: {}", text.len());
        assert!(!text.is_empty(), "body should not be empty");
        assert!(
            text.contains("Herman Melville"),
            "expected Moby Dick excerpt"
        );
    }

    #[tokio::test]
    #[ignore]
    async fn navigate_httpbin_user_agent() {
        let profile = stealth::chrome_130_windows();
        let client = net::HttpClient::new(&profile).unwrap();
        let mut page = Page::navigate_simple(
            "https://httpbin.org/user-agent",
            &client,
            stealth::presets::chrome_130_ru(),
        )
        .await
        .expect("navigate to httpbin/user-agent failed");
        let text = page.text_content();
        println!("[user-agent] response: {text}");
        assert!(
            text.contains("Chrome"),
            "expected Chrome in user-agent response"
        );
    }

    #[tokio::test]
    #[ignore]
    async fn navigate_stealth_headers_check() {
        let profile = stealth::chrome_130_linux();
        let client = net::HttpClient::new(&profile).unwrap();
        let mut page = Page::navigate_simple(
            "https://httpbin.org/headers",
            &client,
            stealth::presets::chrome_130_ru(),
        )
        .await
        .expect("navigate to httpbin/headers failed");
        let text = page.text_content();
        println!("[headers] response: {}", &text[..text.len().min(500)]);
        // httpbin returns JSON with the request headers — verify UA was sent
        assert!(text.contains("User-Agent"), "expected User-Agent header");
        assert!(text.contains("Chrome"), "expected Chrome in UA string");
    }

    #[tokio::test]
    #[ignore]
    async fn navigate_stealth_js_fingerprint() {
        let profile = stealth::chrome_130_linux();
        let mut page = Page::navigate_stealth("https://httpbin.org/html", profile)
            .await
            .expect("stealth navigate failed");
        // Verify stealth properties are wired
        let ua = page.evaluate("navigator.userAgent").unwrap();
        println!("[stealth] userAgent: {ua}");
        assert!(ua.contains("Chrome"), "UA should contain Chrome");

        let webdriver = page.evaluate("typeof navigator.webdriver").unwrap();
        assert_eq!(webdriver, "undefined", "webdriver must be undefined");

        let langs = page
            .evaluate("JSON.stringify(navigator.languages)")
            .unwrap();
        println!("[stealth] languages: {langs}");
        assert!(langs.contains("en"), "should have English language");

        let platform = page.evaluate("navigator.platform").unwrap();
        println!("[stealth] platform: {platform}");
        assert!(platform.contains("Linux"), "profile is Linux");
    }
}
