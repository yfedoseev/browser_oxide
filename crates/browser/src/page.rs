use crate::iframe;
use crate::script_runner;
use crate::stylesheet_collector;
use dom::Dom;
use event_loop::{BrowserEventLoop, IdleReason};
use js_runtime::{runtime::BrowserRuntimeOptions, BrowserJsRuntime};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Whether a URL is a "secure context" per WICG/secure-contexts §3.2.
/// Secure: https, wss, file, plus http://localhost / http://127.0.0.1 /
/// http://[::1] / *.localhost loopback exceptions. Drives `isSecureContext`
/// and gates the ~18 secure-context-only Web Platform APIs.
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
    /// FP-B3: rendered, no challenge marker, but the body is below a
    /// real-content floor (above the THIN-BODY noise floor) — a thin
    /// shell / SPA pre-hydration stub, not the full content. Distinct
    /// from [`Self::Pass`] so a small shell is not over-counted as a
    /// full win (the bestbuy 7.8 KB / spotify 9.6 KB class). NOT a
    /// challenge (`is_challenge()==false`) — it is a content-depth
    /// caveat, like [`Self::RenderIncomplete`] but above the thin-body
    /// floor.
    ThinShell,
    /// FP-B4: a challenge is structurally present in a *large* body but
    /// the vendor flow never *completed* (no clearance / no nav) — e.g.
    /// a Cloudflare Managed-Challenge orchestrator shell that ran but
    /// did not issue `cf_clearance`. Distinct from [`Self::SensorFail`]
    /// (which implies the sensor *scored us bot* ⇒ misdirects work to
    /// fingerprint tuning) and from [`Self::Pass`] (the page never
    /// rendered real content). udemy's 476 KB CF shell is the motivating
    /// case — it must not read as either a sensor-fail or a pass.
    ChallengeIncomplete,
}

impl ChallengeVerdict {
    /// True for every served-challenge outcome — the exact semantics of
    /// the old boolean `is_anti_bot_challenge`.
    pub fn is_challenge(self) -> bool {
        // ChallengeIncomplete IS an unsolved challenge (not a pass) — it
        // must keep the navigate-loop retry/poll paths active.
        matches!(
            self,
            Self::EdgeBlock | Self::SensorFail | Self::ChallengeIncomplete
        )
    }

    /// Stable lowercase tag for JSON/audit output.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::RenderIncomplete => "render-incomplete",
            Self::EdgeBlock => "edge-block",
            Self::SensorFail => "sensor-fail",
            Self::ChallengeIncomplete => "challenge-incomplete",
            Self::ThinShell => "thin-shell",
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
    crate::classify::engine_classify(body)
        .verdict
        .is_challenge()
}

/// Public-engine DataDome interstitial detector (R-DATADOME-DAILY-KEY).
///
/// Identifies a `rt:'i'` DataDome interstitial document by the
/// `captcha-delivery.com` substring — DD's CDN that serves the daily-
/// rotated WASM challenge bundle. The interstitial is small (typically
/// 1-5 KB) and loads `captcha-delivery.com` as a `<script src=…>`; a
/// rendered real page mentioning the CDN in passing would be much
/// larger. The 50 KB size gate guards against that false positive while
/// safely covering every shipped DD interstitial.
///
/// Used at three navigate-loop points to make the engine's DD-aware
/// behaviour (CSP relaxation, iframe materialization, solved-cookie
/// retry) fire on ANY DD challenge document, not just navs where a
/// registered `DataDomeSolver` claims the response. Per `CLAUDE.md`,
/// the WASM solver itself stays in `vendor_solvers`; these three
/// defensive primitives are public-engine scope (they enable the
/// bundle's OWN self-solve flow to run).
fn is_datadome_challenge(html: &str) -> bool {
    html.len() < 50_000 && html.contains("captcha-delivery.com")
}

/// Public-engine DataDome solve detector (R-DATADOME-DAILY-KEY).
///
/// A genuine solve = the cookie jar has `datadome=` AND the current
/// body is no longer a DataDome interstitial (post-solve body is the
/// real page content, not the captcha-delivery.com challenge document).
/// Per FP-D3: a `datadome=` cookie is set on EVERY DD nav including
/// the failing 403, so the cookie alone is not a solve marker — the
/// body-shape transition is what differentiates "still bouncing on the
/// interstitial" from "passed through".
fn is_datadome_solved(cookies: &str, body: &str) -> bool {
    cookies.contains("datadome=") && !is_datadome_challenge(body)
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
    /// Registered [`crate::ChallengeSolver`]s. Populated by
    /// `Page::navigate` with the four default vendor solvers
    /// (AkamaiSolver, KasadaSolver, DataDomeSolver, CloudflareSolver)
    /// so existing call paths see the same behaviour they did
    /// pre-refactor. `Page::from_html` / direct construction leave
    /// this empty (those paths don't run the challenge loop). Stage 2
    /// only consults this list inside the navigate iteration; Stage 3
    /// moves the underlying vendor impls to an internal crate.
    solvers: std::sync::Arc<[std::sync::Arc<dyn crate::ChallengeSolver>]>,
}

impl Drop for Page {
    fn drop(&mut self) {
        // Reap any Workers this page's V8 isolate spawned but never
        // explicitly `worker.terminate()`'d from JS. Without this,
        // each orphan worker keeps a 64 MB stack OS thread + child
        // JsRuntime heap alive for the rest of the process — the
        // dominant memory leak observed in the cold-sweep RSS curve
        // (cnn / bloomberg / youtube / discord / udemy and ~8 others
        // produce >15 MB step-ups that never reclaim).
        {
            let op_state = self.event_loop.runtime_mut().op_state();
            let mut state = op_state.borrow_mut();
            js_runtime::extensions::worker_ext::drain_owned_workers(&mut state);
        }
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

    // Vendor-specific challenge resolution (Cloudflare orchestrator
    // runner, Akamai BMP sensor_data flow) is NOT part of the
    // open-source engine. It lives in the private `vendor_solvers`
    // crate as `ChallengeSolver` implementations. The engine's
    // navigate loop dispatches through whatever solvers are
    // registered (empty by default); see `crate::challenge`.

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
        let client = net::HttpClient::shared(&profile)
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
            solvers: std::sync::Arc::from(Vec::<std::sync::Arc<dyn crate::ChallengeSolver>>::new()),
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
                && std::env::var("BROWSER_OXIDE_CSP_BYPASS").is_err();
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
        let p = profile.unwrap_or_else(stealth::presets::chrome_148_ru);
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
        // `globalThis.__browser_oxide.__documentReadyState = ...` assignments preserve
        // enumerable=false (writable=true, descriptor inherited).
        event_loop
            .execute_script("globalThis._browser_oxide.__documentReadyState = 'loading';")
            .ok();

        // Fire DOMContentLoaded and load events — many scripts wait for these
        event_loop
            .execute_script(
                "document.dispatchEvent(new Event('DOMContentLoaded', {bubbles: true}));",
            )
            .ok();

        // After DOMContentLoaded, readyState = interactive
        event_loop
            .execute_script("globalThis._browser_oxide.__documentReadyState = 'interactive';")
            .ok();

        event_loop
            .execute_script("window.dispatchEvent(new Event('load'));")
            .ok();

        // After load, readyState = complete
        event_loop
            .execute_script("globalThis._browser_oxide.__documentReadyState = 'complete';")
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
                let _escaped = srcdoc.replace('\\', "\\\\").replace('`', "\\`");
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
            solvers: std::sync::Arc::from(Vec::<std::sync::Arc<dyn crate::ChallengeSolver>>::new()),
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

    /// FP-E1: post-JS DOM rescan — materialize **script-injected**
    /// iframes.
    ///
    /// `find_iframes` runs only at *build time* over the parsed DOM
    /// (`build_page_with_scripts_init_and_storage`). When a vendor
    /// challenge script `appendChild`s `<iframe src="https://
    /// geo.captcha-delivery.com/…">` (DataDome) or `challenges.
    /// cloudflare.com` (Cloudflare Turnstile) *after* build, the
    /// `dom_bootstrap.js` hook only fabricates a synthetic
    /// `contentWindow` — the challenge document is **never fetched or
    /// executed**, which structurally blocks DataDome challenge iframes
    /// and modern CF Managed Challenge. This is currently the single
    /// highest-leverage rendering gap.
    ///
    /// This rescans the *current* (post-JS) DOM and, for every iframe
    /// whose `node_id` is not already materialized in `self.children`,
    /// performs the SAME real cross-origin fetch + child-context
    /// execution the build-time path does (`ChildIframe::from_url`,
    /// CSP-`frame-src`-gated identically to build time). Returns the
    /// number of newly materialized iframes. Idempotent: re-running
    /// only picks up iframes injected since the last call.
    ///
    /// Caller MUST gate this on a challenge-origin flag (it is invoked
    /// only inside the challenge poll) so it never runs for a benign
    /// nav ⇒ zero §4-gate regression risk, same narrow-gating
    /// discipline as `started_as_dd/cf/seccpt_challenge`.
    pub async fn rematerialize_iframes(
        &mut self,
        base_url: &str,
        client: &net::HttpClient,
        profile: &stealth::StealthProfile,
    ) -> usize {
        // Snapshot the current DOM's iframes (scoped borrow, dropped
        // before any await / before touching self.children).
        let iframes = {
            let dom_ref = self.event_loop.runtime_mut().inner();
            let state = dom_ref.op_state();
            let state = state.borrow();
            let dom_state = state.borrow::<js_runtime::state::DomState>();
            iframe::find_iframes(&dom_state.dom)
        };
        let already: Vec<_> = self.children.iter().map(|c| c.node_id).collect();
        let mut materialized = 0usize;
        for info in &iframes {
            if already.contains(&info.node_id) {
                continue; // already a real child context — not script-new
            }
            if let Some(srcdoc) = &info.srcdoc {
                match iframe::ChildIframe::from_srcdoc(info.node_id, srcdoc, profile).await {
                    Ok(child) => {
                        self.children.push(child);
                        materialized += 1;
                    }
                    Err(e) => tracing::warn!(error = %e, "rematerialize srcdoc error"),
                }
            } else if let Some(src) = &info.src {
                if src.is_empty() || src.starts_with("javascript:") {
                    continue; // blank/JS frames are handled at build time
                }
                if let Some(full_src) = Self::resolve_url(base_url, src) {
                    match iframe::ChildIframe::from_url(
                        info.node_id,
                        &full_src,
                        client,
                        Some(profile),
                    )
                    .await
                    {
                        Ok(child) => {
                            self.children.push(child);
                            materialized += 1;
                        }
                        Err(e) => tracing::warn!(
                            src = %full_src, error = %e,
                            "rematerialize src-iframe error (CSP-blocked or fetch failed)"
                        ),
                    }
                }
            }
        }
        materialized
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

    /// Replace this page's [`crate::ChallengeSolver`] list. Returns the
    /// modified `Self` so it can be used builder-style. Default (set by
    /// `Page::navigate`) is the four built-in vendor solvers; embedders
    /// who want a vanilla engine can call `page.with_solvers(&[])` or
    /// build their own list of trait objects.
    pub fn with_solvers(
        mut self,
        solvers: impl Into<std::sync::Arc<[std::sync::Arc<dyn crate::ChallengeSolver>]>>,
    ) -> Self {
        self.solvers = solvers.into();
        self
    }

    /// Read-only view of the currently-registered solvers.
    pub fn solvers(&self) -> &[std::sync::Arc<dyn crate::ChallengeSolver>] {
        &self.solvers
    }

    /// Default solver set wired by `Page::navigate`. The open-source
    /// engine ships NO per-vendor solvers — the measured 126-corpus
    /// pass rate comes entirely from the from-scratch TLS + fingerprint
    /// + V8 engine, not from active challenge-solving (verified: empty
    /// vs full solvers both render the same sites). Per-vendor solver
    /// implementations (Akamai BMP sensor_data, Kasada PoW, DataDome
    /// i.js, Cloudflare orchestrator) live in the private companion
    /// `vendor_solvers` crate; embedders that want them register via
    /// `Page::with_solvers(vendor_solvers::default_solvers())`.
    pub fn default_solvers() -> std::sync::Arc<[std::sync::Arc<dyn crate::ChallengeSolver>]> {
        std::sync::Arc::from(Vec::<std::sync::Arc<dyn crate::ChallengeSolver>>::new())
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
        let client = net::HttpClient::shared(&profile)
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
        let client = net::HttpClient::shared(&profile)
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

    /// Like [`Page::navigate`] but with a caller-supplied
    /// [`crate::ChallengeSolver`] list. The open-source engine ships no
    /// solvers (`default_solvers()` is empty); embedders that have the
    /// private `vendor_solvers` crate register them here:
    /// `Page::navigate_with_solvers(url, profile, n, vendor_solvers::default_solvers())`.
    pub async fn navigate_with_solvers(
        url: &str,
        profile: stealth::StealthProfile,
        max_iterations: u8,
        solvers: std::sync::Arc<[std::sync::Arc<dyn crate::ChallengeSolver>]>,
    ) -> Result<Self, deno_core::error::AnyError> {
        let humanize = include_str!("js/humanize.js").to_string();
        Self::navigate_with_init_solvers(url, profile, max_iterations, vec![humanize], solvers)
            .await
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
        Self::navigate_with_init_solvers(
            url,
            profile,
            max_iterations,
            init_scripts,
            Self::default_solvers(),
        )
        .await
    }

    /// [`Self::navigate_with_init`] + an explicit solver list. The
    /// 4-arg `navigate_with_init` forwards here with `default_solvers()`.
    pub async fn navigate_with_init_solvers(
        url: &str,
        profile: stealth::StealthProfile,
        max_iterations: u8,
        init_scripts: Vec<String>,
        solvers: std::sync::Arc<[std::sync::Arc<dyn crate::ChallengeSolver>]>,
    ) -> Result<Self, deno_core::error::AnyError> {
        let client = net::HttpClient::shared(&profile)
            .map_err(|e| deno_core::error::AnyError::msg(e.to_string()))?;

        // Share the HTTP client with JS fetch() so scripts running inside
        // the V8 isolate hit the same cookie jar as the Rust driver.
        js_runtime::extensions::fetch_ext::set_fetch_client(client.clone());

        let iterations = max_iterations.max(1);
        let debug_nav = std::env::var("BROWSER_OXIDE_DEBUG_NAV").is_ok();

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
        // v0.1.0-parity Fix 10: extended vendor-detect markers per
        // 18_ANTI_BOT_VENDOR_COOKBOOK.md §4.1. Pure observability —
        // post-run analysis splits CHL outcomes by protocol.
        if let Some(v) = resp.headers.get("cf-mitigated") {
            eprintln!("[vendor-detect] cloudflare-mitigated {} on {}", v, resp.url);
        }
        if let Some(v) = resp.headers.get("cf-ray") {
            if matches!(resp.status, 403 | 429 | 498 | 503) {
                eprintln!(
                    "[vendor-detect] cloudflare cf-ray={} status={} on {}",
                    v, resp.status, resp.url
                );
            }
        }
        if let Some(v) = resp.headers.get("x-iinfo") {
            eprintln!("[vendor-detect] imperva-incapsula {} on {}", v, resp.url);
        }
        if resp
            .headers
            .get("x-cdn")
            .map(|v| v.to_ascii_lowercase().contains("imperva"))
            .unwrap_or(false)
        {
            eprintln!("[vendor-detect] imperva-cdn on {}", resp.url);
        }
        if let Some(v) = resp.headers.get("x-perimeterx-id") {
            eprintln!("[vendor-detect] perimeterx {} on {}", v, resp.url);
        }
        if let Some(v) = resp.headers.get("x-sucuri-id") {
            eprintln!("[vendor-detect] sucuri {} on {}", v, resp.url);
        }
        if let Some(v) = resp.headers.get("x-akamai-transformed") {
            eprintln!("[vendor-detect] akamai-edge {} on {}", v, resp.url);
        }
        if resp
            .headers
            .iter()
            .any(|(k, _)| k.to_ascii_lowercase().starts_with("x-kpsdk"))
        {
            eprintln!("[vendor-detect] kasada (x-kpsdk-*) on {}", resp.url);
        }
        if let Some(v) = resp.headers.get("x-armor-shield-zone") {
            eprintln!("[vendor-detect] reblaze {} on {}", v, resp.url);
        }
        if resp
            .headers
            .get("server")
            .map(|v| v.to_ascii_lowercase().contains("cloudflare"))
            .unwrap_or(false)
            && !resp.headers.contains_key("cf-mitigated")
            && !resp.headers.contains_key("cf-ray")
        {
            // Server: cloudflare with no cf-mitigated/ray = passive
            // CF edge, not active bot management. Logged only when
            // we don't already have a more specific signal.
            eprintln!("[vendor-detect] cloudflare-edge on {}", resp.url);
        }
        if resp
            .headers
            .get("via")
            .map(|v| v.to_ascii_lowercase().contains("varnish"))
            .unwrap_or(false)
        {
            eprintln!("[vendor-detect] fastly (via: varnish) on {}", resp.url);
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
            solvers,
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
        let client = net::HttpClient::shared(&profile)
            .map_err(|e| deno_core::error::AnyError::msg(e.to_string()))?;
        js_runtime::extensions::fetch_ext::set_fetch_client(client.clone());

        let iterations = max_iterations.max(1);
        let debug_nav = std::env::var("BROWSER_OXIDE_DEBUG_NAV").is_ok();

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
            Self::default_solvers(),
        )
        .await
    }

    /// Reset all cross-navigation JS state on this Page so its V8 isolate
    /// can be safely reused for a different URL. Called by
    /// [`Page::navigate_warm`] and [`crate::pool::PagePool::navigate`].
    ///
    /// What gets cleared:
    /// - In-flight `setTimeout` / `setInterval` callbacks (via
    ///   `__cancelAllTimers()` generation bump in `timer_bootstrap.js`).
    ///   Without this, the previous page's `humanize.js` 30 pending
    ///   timers + recurring 4 s setInterval would fire on the new DOM
    ///   and dispatch synthetic mouse events into the wrong document.
    /// - `_browser_oxide.__pendingNavigation` — spurious value left by the
    ///   `location.href = …` setter inside the previous build.
    /// - `_browser_oxide.__fetchLog` — DevTools-style network log; must
    ///   reset so the new page's fetches aren't mixed with the old's.
    /// - `window.__cookieWrites`, `window.__scriptErrors` —
    ///   instrumentation buffers re-initialised per page.
    /// - `globalThis.__akamai_events` (mouse / key / touch / scroll
    ///   buffers + counters) — humanize.js re-installs into these and
    ///   sensors read them on POST, so stale values would skew detection.
    /// - `globalThis.__jsCookies` — cookie cache snapshot (the real
    ///   source of truth is the HTTP client's jar, re-synced below).
    ///
    /// What stays:
    /// - V8 isolate, bootstrap scripts (`window_bootstrap.js`,
    ///   `dom_bootstrap.js`, …), and the page-instrumentation wrappers
    ///   on `globalThis.fetch` / `document.cookie` / `XMLHttpRequest`.
    ///   These are the expensive bits we're reusing.
    fn reset_warm_state(&mut self) {
        let _ = self.event_loop.execute_script(
            r#"(function() {
                const g = globalThis;
                try { g.__cancelAllTimers && g.__cancelAllTimers(); } catch (_) {}
                if (g._browser_oxide) {
                    g._browser_oxide.__pendingNavigation = null;
                    if (Array.isArray(g._browser_oxide.__fetchLog)) {
                        g._browser_oxide.__fetchLog.length = 0;
                    }
                }
                const w = (g.window && g.window !== g) ? g.window : g;
                try { if (Array.isArray(w.__cookieWrites)) w.__cookieWrites.length = 0; } catch (_) {}
                try { if (Array.isArray(w.__scriptErrors)) w.__scriptErrors.length = 0; } catch (_) {}
                if (g.__akamai_events) {
                    g.__akamai_events.mouse.length = 0;
                    g.__akamai_events.key.length = 0;
                    g.__akamai_events.touch.length = 0;
                    g.__akamai_events.scroll.length = 0;
                    if (g.__akamai_events.counters) {
                        g.__akamai_events.counters.key = 0;
                        g.__akamai_events.counters.mouse = 0;
                        g.__akamai_events.counters.touch = 0;
                        g.__akamai_events.counters.scroll = 0;
                        g.__akamai_events.counters.accel = 0;
                    }
                }
                if (g.__jsCookies) g.__jsCookies = {};
            })();"#,
        );
    }

    /// Navigate this *warm* Page to a new URL by reusing its V8 isolate
    /// and bootstrap. Saves ~150 ms vs the cold [`Page::navigate`] path
    /// by skipping isolate creation, bootstrap-script execution, and
    /// the page-instrumentation install (cookie-write / fetch wrapper /
    /// error-tracking wrappers stay across navigations).
    ///
    /// Use via [`crate::pool::PagePool::navigate`] for the canonical
    /// "scrape many URLs with one warm engine" pattern. The Page's
    /// existing stealth profile is reused — call sites that need a
    /// different profile should `pool.release` this Page and acquire a
    /// fresh one.
    ///
    /// **Scope**: warm reuse handles the benign content-extraction case.
    /// It does NOT run the cookie-diff / pending-nav iteration loop that
    /// [`Page::navigate`] does for anti-bot pages — challenge scripts
    /// keep V8 busy on their own so the savings from a warm isolate are
    /// negligible there anyway, and reproducing the iteration loop on
    /// the warm path is a known follow-up (see the comment above
    /// [`Self::navigate_loop_internal`]).
    pub async fn navigate_warm(&mut self, url: &str) -> Result<(), deno_core::error::AnyError> {
        let warm_trace = std::env::var("BROWSER_OXIDE_WARM_PROFILE").is_ok();
        let warm_t0 = std::time::Instant::now();
        macro_rules! wmark {
            ($label:expr) => {
                if warm_trace {
                    eprintln!("[warm] {:>5}ms {}", warm_t0.elapsed().as_millis(), $label);
                }
            };
        }

        // Pull the stealth profile out of the runtime's `StealthState`.
        // The pool stores Pages by profile; this guards against silent
        // misuse where a caller hand-builds a Page without a profile and
        // tries to warm-navigate it.
        let profile: stealth::StealthProfile = {
            let op_state = self.event_loop.runtime_mut().op_state();
            let state = op_state.borrow();
            state
                .try_borrow::<js_runtime::extensions::stealth_ext::StealthState>()
                .and_then(|s| s.profile.clone())
                .ok_or_else(|| {
                    deno_core::error::AnyError::msg(
                        "navigate_warm requires a Page built with a stealth profile",
                    )
                })?
        };
        let client = net::HttpClient::shared(&profile)
            .map_err(|e| deno_core::error::AnyError::msg(e.to_string()))?;
        js_runtime::extensions::fetch_ext::set_fetch_client(client.clone());
        wmark!("profile + shared client");

        // Initial fetch (redirect-follow, same as cold path).
        let resp = client
            .get_follow(url, 10)
            .await
            .map_err(|e| deno_core::error::AnyError::msg(e.to_string()))?;
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
        let resp_url = resp.url.clone();
        let timings = resp.timings.clone();
        drop(resp);
        wmark!("fetch done");

        // Install CSP for this navigation (same logic as the cold build).
        {
            let csp_dom = html_parser::parse_html(&html);
            let header_refs: Vec<&str> = csp_headers.iter().map(|s| s.as_str()).collect();
            let report_refs: Vec<&str> = csp_headers_ro.iter().map(|s| s.as_str()).collect();
            let policy_set = crate::csp_collector::collect_csp_with_report_only(
                &header_refs,
                &report_refs,
                &csp_dom,
            );
            let env_bypass = std::env::var("BROWSER_OXIDE_CSP_BYPASS").is_ok();
            let enforce = profile.enforce_csp && !env_bypass;
            if let Ok(origin) = url::Url::parse(&resp_url) {
                if !policy_set.is_empty() {
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
        wmark!("CSP set");

        // Parse + find subresources. Parsing the same DOM twice (once
        // for CSP, once here) is cheap (~µs for typical pages) and keeps
        // the CSP block lifted from the cold path without a refactor.
        let dom = html_parser::parse_html(&html);
        let scripts_meta = script_runner::find_scripts(&dom);
        let stylesheet_entries = stylesheet_collector::find_stylesheets(&dom);

        // Parallel fetch external CSS + external scripts. Mirrors the
        // cold build path; we only inline the bits we actually need
        // here so this method stays self-contained.
        let mut inline_css: Vec<String> = Vec::new();
        let css_futures: Vec<_> = stylesheet_entries
            .iter()
            .filter_map(|entry| match entry {
                stylesheet_collector::StylesheetEntry::Inline(css) => {
                    inline_css.push(css.clone());
                    None
                }
                stylesheet_collector::StylesheetEntry::External(href) => {
                    let full_url = Self::resolve_url(&resp_url, href)?;
                    let client = client.clone();
                    Some(async move {
                        match client.get(&full_url).await {
                            Ok(r) if r.ok() => {
                                let text = r.text();
                                if !text.trim_start().starts_with("<!") {
                                    Some((text, r.timings.clone()))
                                } else {
                                    None
                                }
                            }
                            _ => None,
                        }
                    })
                }
            })
            .collect();
        let script_futures: Vec<_> = scripts_meta
            .iter()
            .enumerate()
            .filter_map(|(i, script)| {
                let src = script.src.as_ref()?;
                let full_url = Self::resolve_url(&resp_url, src)?;
                if let Ok(parsed_url) = url::Url::parse(&full_url) {
                    if js_runtime::extensions::fetch_ext::check_csp(
                        net::csp::Directive::ScriptSrcElem,
                        &parsed_url,
                        script.nonce.as_deref(),
                        true,
                    )
                    .is_err()
                    {
                        return None;
                    }
                }
                let client = client.clone();
                let profile = profile.clone();
                let referer = resp_url.clone();
                Some(async move {
                    let mut hdrs = net::headers::nav_headers(&profile, false);
                    hdrs.push(("referer".to_string(), referer));
                    hdrs.push(("accept".to_string(), "*/*".to_string()));
                    hdrs.push(("sec-fetch-dest".to_string(), "script".to_string()));
                    hdrs.push(("sec-fetch-mode".to_string(), "no-cors".to_string()));
                    hdrs.push(("sec-fetch-site".to_string(), "cross-site".to_string()));
                    match client.get_follow_with_headers(&full_url, &hdrs, 5).await {
                        Ok(r) if r.ok() => {
                            let text = r.text();
                            if text.trim_start().starts_with("<!")
                                || text.trim_start().starts_with("<html")
                            {
                                None
                            } else {
                                Some((i, text, r.timings.clone()))
                            }
                        }
                        _ => None,
                    }
                })
            })
            .collect();
        let (fetched_css, fetched_scripts) = futures_util::future::join(
            futures_util::future::join_all(css_futures),
            futures_util::future::join_all(script_futures),
        )
        .await;
        wmark!("subresources fetched");

        let mut all_timings = vec![timings];
        let mut stylesheets = inline_css;
        for r in fetched_css.into_iter().flatten() {
            stylesheets.push(r.0);
            all_timings.push(r.1);
        }
        let mut prefetched: std::collections::HashMap<usize, String> =
            std::collections::HashMap::new();
        for r in fetched_scripts.into_iter().flatten() {
            prefetched.insert(r.0, r.1);
            all_timings.push(r.2);
        }

        // Cancel all in-flight timers from the previous page and clear
        // cross-nav JS buffers BEFORE swapping the DOM, so any straggler
        // callbacks that try to fire don't see a half-installed state.
        self.reset_warm_state();
        wmark!("reset_warm_state");

        // Swap DOM (also resets `TimerState` Rust-side).
        self.event_loop.runtime_mut().replace_dom(dom, stylesheets);
        self.children.clear();
        self.url = resp_url.clone();
        for t in all_timings {
            self.event_loop.runtime_mut().record_resource_timing(t);
        }
        wmark!("replace_dom");

        // Build-phase deadline watcher — preempts CPU-bound inline-script
        // spins on the warm isolate the same way the cold build does.
        let build_budget_ms: u64 = std::env::var("BROWSER_OXIDE_BUILD_BUDGET_MS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(25_000);
        let _build_watcher = V8DeadlineWatcher::new(
            self.event_loop.runtime_mut().isolate_handle(),
            Duration::from_millis(build_budget_ms),
        );

        // Seed `location` for this navigation. The `reset_nav_pending` now
        // scrubs the spurious `__pendingNavigation` the setter writes, so
        // downstream `PENDING_NAV_JS` reads see a clean empty value.
        let url_js = resp_url.replace('\\', "\\\\").replace('\'', "\\'");
        let _ = self
            .event_loop
            .execute_script(&format!("location.href = '{}';", url_js));
        self.event_loop.reset_nav_pending();

        // Refresh `document.cookie` for the new origin. Fast op call,
        // 50 ms cap is plenty (same as the cold path).
        let _ = self
            .event_loop
            .execute_and_run(
                "globalThis.__syncCookiesFromNet && globalThis.__syncCookiesFromNet();",
                Duration::from_millis(50),
            )
            .await;

        // Run inline + external scripts in document order, draining
        // between each so microtasks land before the next script reads
        // them. Mirrors the cold path's script loop.
        for (i, script) in scripts_meta.iter().enumerate() {
            let code = if script.src.is_some() {
                match prefetched.get(&i) {
                    Some(c) => c.clone(),
                    None => continue,
                }
            } else {
                script.code.clone()
            };
            if code.trim().is_empty() {
                continue;
            }
            let name = script.src.clone().unwrap_or_else(|| resp_url.clone());
            if let Err(e) = self.event_loop.execute_script_with_name(&code, &name) {
                tracing::warn!(script = %name, error = %e, "warm script error");
            }
            let _ = self
                .event_loop
                .run_until_idle(Duration::from_millis(50))
                .await;
        }
        wmark!("scripts executed");

        // Re-install `humanize.js` on the fresh DOM. The previous page's
        // humanize closure captured the old `document.body`; its setInterval
        // has been cancelled by the generation bump, so we install fresh.
        let _ = self
            .event_loop
            .execute_script(include_str!("js/humanize.js"));

        // DOMContentLoaded + load events — same setTimeout(0) trick the
        // cold build uses so dispatched handlers run inside the event
        // loop (not synchronously during setup).
        let _ = self.event_loop.execute_script(
            r#"setTimeout(() => {
                document.dispatchEvent(new Event('DOMContentLoaded', {bubbles: true}));
                window.dispatchEvent(new Event('DOMContentLoaded', {bubbles: true}));
                window.dispatchEvent(new Event('load'));
            }, 0);"#,
        );

        // Meta-refresh scanner — sets `__pendingNavigation` if the page
        // has one. The caller doesn't loop on it (warm path skips the
        // iter retry loop) but downstream `Page::content()` / classifier
        // calls won't see a half-applied refresh either way.
        let _ = self.event_loop.execute_script(
            r#"(function() {
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
                        globalThis.__pendingNavigation = { url: target, kind: 'assign' };
                        try { Deno.core.ops.op_set_pending_nav(); } catch (_) {}
                    }, delay * 1000);
                    break;
                }
            })();"#,
        );

        // Final drain to let async work settle. Same cap as the cold
        // build phase — humanize timers are unref'd so they don't pin.
        let _ = self
            .event_loop
            .run_until_idle(Duration::from_millis(500))
            .await;
        self.event_loop.runtime_mut().cancel_terminate_execution();
        drop(_build_watcher);
        wmark!("drain done [READY]");

        Ok(())
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
        solvers: std::sync::Arc<[std::sync::Arc<dyn crate::ChallengeSolver>]>,
    ) -> Result<Self, deno_core::error::AnyError> {
        // The challenge-solver dispatch below iterates over `solvers`.
        // The open-source engine passes an empty list (per-vendor solvers
        // live in the private `vendor_solvers` crate); embedders register
        // their own via `Page::navigate_with_solvers`. An empty list makes
        // the dispatch a clean no-op.
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
            let env_bypass = std::env::var("BROWSER_OXIDE_CSP_BYPASS").is_ok();
            // Phase 5 (doc 05 §2c/§2d): a DataDome `rt:'i'` interstitial
            // is a DataDome-served challenge document, NOT the origin's
            // page — enforcing the origin's restrictive 403-response CSP
            // on it refuses geo.captcha-delivery.com and kills the i.js
            // self-solve round-trip. Narrowly gated to the <4 KB
            // interstitial shape (detect_datadome_interstitial), so it
            // cannot affect normal pages or the 10 passing Akamai sites
            // (their bodies don't match). The cookie-diff retry then
            // re-issues the original URL once i.js lands `datadome=`.
            // A registered solver may request the origin CSP be
            // suspended for this nav (DataDomeSolver does this for
            // `rt:'i'` interstitials so i.js can reach
            // captcha-delivery.com). Empty solver list ⇒ never relaxed.
            // R-DATADOME-DAILY-KEY: relax CSP on any DataDome interstitial,
            // not just when a registered solver claims it. The interstitial
            // loads `captcha-delivery.com` scripts that the origin's own
            // CSP refuses; without relaxation the bundle never runs and
            // the daily-key WASM never gets a chance to land `datadome=`.
            let relax_csp =
                solvers.iter().any(|s| s.relax_response_csp(&html)) || is_datadome_challenge(&html);
            let enforce = profile.enforce_csp && !env_bypass && !relax_csp;
            if relax_csp && debug_nav {
                eprintln!("[solver] origin CSP not enforced for this challenge document");
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
                const browser_oxide = globalThis._browser_oxide;\
                const p = browser_oxide && browser_oxide.__pendingNavigation;\
                if (p) browser_oxide.__pendingNavigation = null;\
                return p ? JSON.stringify({url: p.url, method: p.method || 'GET', body: p.body, kind: p.kind}) : '';\
            })()";

        // Did this nav start as a challenge document that a registered
        // solver wants to keep the resolution path active for? (DataDome
        // `rt:'i'` interstitials mutate the DOM via i.js, dropping the
        // body marker; without this pre-mutation flag the engine would
        // skip the pending-nav poll + cookie-diff retry before i.js
        // lands `datadome=`.) Reuses `relax_response_csp` as the
        // "is this my challenge doc" signal. Empty solver list ⇒ false.
        //
        // R-DATADOME-DAILY-KEY public-engine primitive: also fire on a
        // raw DataDome interstitial shape (i.e. a small body that loads
        // a `captcha-delivery.com` script) so the iframe-materialization
        // poll runs even without a registered DataDomeSolver. The bundle
        // is the actor; the engine just needs to NOT interfere and to
        // re-fetch the original URL once `datadome=` is in the jar.
        let started_as_dd_challenge =
            solvers.iter().any(|s| s.relax_response_csp(&html)) || is_datadome_challenge(&html);
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
        // FP-C2: the last live doc-20 mutable-state guard. The
        // cookie-diff retry / pending-nav poll below gate on
        // `page.is_anti_bot_challenge()` (the *post-mutation* DOM). A
        // Cloudflare orchestrator mutates the body, so the `_cf_chl_opt`
        // / `/cdn-cgi/challenge-platform/` marker can drop from the live
        // DOM while `cf_clearance` was NEVER issued ⇒ a
        // body-mutated-but-unsolved CF page silently slips past the
        // retry gate. DD and sec-cpt already have persistent
        // origin-flags; Cloudflare did not. Capture it from the
        // *initial* response `html` (pre-mutation), mirroring the
        // existing detector at `handle_cloudflare_flow`. Narrow ⇒ false
        // for every non-CF site ⇒ §4 gate unaffected.
        let started_as_cf_challenge = crate::classify::is_cf_challenge_doc(&html);
        let mut current_html = html;
        let mut current_url = resp_url;
        let mut current_storage: Option<
            std::collections::HashMap<String, std::collections::HashMap<String, String>>,
        > = None;
        let mut last_accept_ch_upgrade = accept_ch_upgrade;
        let mut accept_ch_retry_done = false;

        // Wall-clock budget for this entire navigate_with_init call.
        // Default 50 s leaves headroom under the antibot_smoke 60 s wrapper.
        // Override via BROWSER_OXIDE_NAV_BUDGET_MS for slow-link or debugging runs.
        // The budget is mutable: if iter=0 returns a *real-content* page
        // (no challenge marker AND body > 50 KB), we extend the budget by
        // BROWSER_OXIDE_NAV_BUDGET_EXTEND_MS (default 25 s) to allow heavy
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
        //     the bump, body=0/69 bytes after deadline.
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
            // Akamai sec-cpt PoW (Task#3): the obfuscated sec-cpt
            // bundle runs a heavy in-page VM PoW comparable to Kasada
            // and needs ≥2 iterations (build → bundle self-solve →
            // `sec_cpt=…~3~` → post-solve reload). 25 s (the plain-BMP
            // tier) is too tight — the b623d5d flip was observed at
            // nav_ms≈119 s, surviving only on budget-extend stacking.
            // Give it the Kasada heavy-PoW tier so the flip is
            // deterministic, not budget-luck. (bestbuy is the benign
            // i18n splash — Task#1 — so it stays in the plain-BMP tier.)
            Some(h) if h.ends_with("homedepot.com") => 45_000,
            // Akamai BMP-protected (plain sensor_data, no sec-cpt PoW).
            Some(h)
                if h.ends_with("bestbuy.com")
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
            std::env::var("BROWSER_OXIDE_NAV_BUDGET_MS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(host_budget_default_ms),
        );
        let nav_budget_extend = Duration::from_millis(
            std::env::var("BROWSER_OXIDE_NAV_BUDGET_EXTEND_MS")
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
                    || started_as_seccpt_challenge
                    || started_as_cf_challenge)
            {
                let deadline = std::time::Instant::now() + Duration::from_secs(90);
                while std::time::Instant::now() < deadline {
                    let _ = page
                        .event_loop()
                        .run_until_idle(Duration::from_millis(200))
                        .await;
                    // FP-E1: a challenge script may have appendChild'd a
                    // cross-origin challenge iframe (DataDome
                    // geo.captcha-delivery.com / Cloudflare
                    // challenges.cloudflare.com) during the tick above.
                    // `find_iframes` ran only at build time, so such a
                    // script-injected iframe otherwise gets ONLY a
                    // synthetic contentWindow shim and its challenge
                    // document is never fetched/executed (the structural
                    // blocker for etsy/tripadvisor + every modern CF
                    // Managed Challenge). Materialize it for real here.
                    // Idempotent + cheap (DOM walk only) when nothing new
                    // appeared; gated by this poll's challenge condition
                    // ⇒ never runs for a benign nav (zero §4 regression).
                    let _ = page
                        .rematerialize_iframes(&current_url, &client, &profile)
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
                            let now = client.cookies_for_url(p).await.unwrap_or_default();
                            // FP-D3: a `datadome=` cookie is set on every
                            // nav incl. the failing 403 — break only on a
                            // genuine solve (cookie present AND the body
                            // is no longer a DD challenge document), not
                            // on a bare/gained cookie (false success).
                            // E2 trait dispatch: any registered solver
                            // reporting solved on this (cookies, body) pair
                            // breaks the poll. Replaces the direct
                            // `datadome_handler::datadome_solved` call;
                            // DataDomeSolver.solved_signal internally calls
                            // the same fn so behaviour is preserved.
                            //
                            // R-DATADOME-DAILY-KEY public primitive: also
                            // break on the engine-side `is_datadome_solved`
                            // check so the cookie-diff retry fires even
                            // without a registered DataDomeSolver.
                            let body = page.content();
                            if solvers.iter().any(|s| s.solved_signal(&now, &body))
                                || is_datadome_solved(&now, &body)
                            {
                                break;
                            }
                        }
                    }
                    // Task#3 (homedepot deterministic sec-cpt): the
                    // Akamai sec-cpt bundle self-solves in our V8 and
                    // transitions the `sec_cpt` cookie to the `~3~`
                    // (solved) state WITHOUT setting a pending nav —
                    // exactly analogous to the DataDome break above.
                    // Pre-fix, the b623d5d flip survived only on
                    // incidental budget-stacking (observed nav_ms≈119s);
                    // break the instant the documented `~3~` success
                    // marker appears so the post-sec-cpt reload is
                    // deterministic, not budget-luck. Gated by
                    // `started_as_seccpt_challenge` ⇒ false for every
                    // non-sec-cpt site ⇒ zero §4 regression.
                    if started_as_seccpt_challenge {
                        if let Some(p) = parsed_current.as_ref() {
                            let now = client.cookies_for_url(p).await.unwrap_or_default();
                            // E2 trait dispatch: AkamaiSolver.solved_signal
                            // delegates to sec_cpt::sec_cpt_solved.
                            let body = page.content();
                            if solvers.iter().any(|s| s.solved_signal(&now, &body)) {
                                if debug_nav {
                                    eprintln!(
                                        "[solver] solved_signal fired (likely sec-cpt ~3~) — breaking poll"
                                    );
                                }
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
                let rt = page.event_loop().runtime_mut();
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

            // E2 trait dispatch: iterate registered solvers and let each
            // try to clear its vendor's challenge. The four built-in
            // wrappers (AkamaiSolver, KasadaSolver, DataDomeSolver,
            // CloudflareSolver) internally bail out on non-matching
            // bodies / cookies, so unconditional iteration is
            // equivalent to the pre-refactor unconditional inline calls.
            //
            // We track whether *any* solver reported Solved this
            // iteration so the cookie-delta retry below can suppress
            // the retry-on-cookie-change for the previously
            // `akamai_state == Favorable` special case.
            //
            // sec-cpt guard preserved: when this nav started as
            // sec-cpt, the Akamai BMP POST path is wrong (per doc-20
            // anti-pattern). AkamaiSolver's `solve` already short-
            // circuits on the "sec-cpt" sub_kind, returning
            // InProgress, so the unconditional iter is safe.
            let mut any_solved = false;
            for s in solvers.iter() {
                // Pre-iteration sec-cpt guard: when this nav started as
                // sec-cpt, the Akamai BMP sensor_data POST is the wrong
                // payload for the verify endpoint (doc-20 anti-pattern),
                // so signal the solver via sub_kind to short-circuit.
                let sub = if s.name() == "akamai-bmp" && started_as_seccpt_challenge {
                    "sec-cpt"
                } else {
                    ""
                };
                let kind = crate::challenge::ChallengeKind::new(s.name(), sub);
                if matches!(
                    s.solve(&mut page, &client, kind).await,
                    crate::challenge::SolveOutcome::Solved
                ) {
                    any_solved = true;
                }
            }

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
                    || started_as_cf_challenge
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
                    // Phase 5 (homedepot): same diagnostic for the
                    // sec-cpt bundle — did `/Wjv3…` actually run and fire
                    // its PoW-answer verify POST? debug_nav-gated ⇒ zero
                    // §4 gate impact.
                    if debug_nav && started_as_seccpt_challenge {
                        let fl = page
                            .event_loop()
                            .execute_script(
                                "JSON.stringify((globalThis._browser_oxide&&globalThis._browser_oxide.__fetchLog)||[])",
                            )
                            .unwrap_or_default();
                        let secck = page
                            .event_loop()
                            .execute_script(
                                "(function(){try{return /sec_cpt=/.test(document.cookie)?'sec_cpt-present':'no-sec_cpt'}catch(e){return 'err'}})()",
                            )
                            .unwrap_or_default();
                        eprintln!("[seccpt-trace] post-bundle cookie={secck} __fetchLog={fl}");
                    }

                    let mut should_retry = (cookies_after != cookies_before
                        && !cookies_after.is_empty())
                        || (last_accept_ch_upgrade && !accept_ch_retry_done);

                    // If a solver already reported the challenge solved
                    // this iteration, DON'T retry just because a cookie
                    // value changed (challenge cookies always rotate).
                    if (accept_ch_retry_done || !last_accept_ch_upgrade) && any_solved {
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
                        // v0.1.0-parity Fix 10: extended marker set per
                        // 18_ANTI_BOT_VENDOR_COOKBOOK.md §4.2. Each new
                        // string is the same one classify.rs already keys
                        // on for the vendor — keeping this guard in sync
                        // with the verdict logic.
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
                            && !v8_html.contains("/cdn-cgi/challenge-platform/")
                            && !v8_html.contains("AwsWafIntegration")
                            && !v8_html.contains("gokuProps")
                            && !v8_html.contains("_Incapsula_Resource")
                            && !v8_html.contains("visid_incap")
                            && !v8_html.contains("reese84")
                            && !v8_html.contains("_px3")
                            && !v8_html.contains("_pxhd")
                            && !v8_html.contains("px-captcha")
                            && !v8_html.contains("press &amp; hold")
                            && !v8_html.contains("sucuri_cloudproxy_js")
                            && !v8_html.contains("Incapsula incident ID");

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
                //
                // FP-D1 (reachability, verify-don't-assume): this Inc-8
                // window is the *pending-nav* (homedepot-class) DD path.
                // The *etsy-class* `rt:'i'` flow (no early pending nav)
                // is NOT served here — it is served by the
                // `pending_info.is_empty() && started_as_dd_challenge`
                // poll above, which now also pumps `rematerialize_iframes`
                // (FP-E1) and breaks on `datadome_solved` (FP-D3). So the
                // DD self-solve window is reachable on BOTH branches; the
                // poll-entry invariant (`started_as_dd_challenge ==
                // is_datadome_challenge_doc(initial html)`) is pinned by
                // `datadome_handler::tests::etsy_rt_i_body_enters_dd_self_solve_path`.
                if started_as_dd_challenge {
                    let dd_deadline = std::time::Instant::now() + Duration::from_secs(45);
                    let parsed_cur = url::Url::parse(&current_url).ok();
                    while std::time::Instant::now() < dd_deadline {
                        let _ = page
                            .event_loop()
                            .run_until_idle(Duration::from_millis(250))
                            .await;
                        if let Some(p) = parsed_cur.as_ref() {
                            let now = client.cookies_for_url(p).await.unwrap_or_default();
                            // FP-D3: require a genuine solve (cookie +
                            // body no longer a DD challenge doc) — the
                            // bare `datadome=` cookie is set on the 403
                            // fail too, so the old check broke the
                            // self-solve window on a false success.
                            // E2 trait dispatch: DataDomeSolver.solved_signal
                            // delegates to datadome_handler::datadome_solved.
                            let body = page.content();
                            if solvers.iter().any(|s| s.solved_signal(&now, &body)) {
                                if debug_nav {
                                    eprintln!("[solver] self-solve signal — proceeding to reload");
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

    /// **DEPRECATED**: Legacy name — now a thin wrapper around [`Page::navigate`].
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
        // Defence against the iphey.com regression: a JS-side
        // `location.href = 'about:blank'` (or `'data:...'`, `'javascript:'`,
        // etc.) can reach Rust as the literal `https://host/about:blank` if
        // the JS URL polyfill mis-joined a special-scheme URL against the
        // current http(s) base. Treat any path that begins with a known
        // special-scheme literal as a no-op pending navigation (return None
        // and let the caller keep the current page).
        if let Some(idx) = relative.find('/') {
            let tail = &relative[idx + 1..];
            for sch in &[
                "about:",
                "data:",
                "javascript:",
                "blob:",
                "mailto:",
                "tel:",
                "view-source:",
            ] {
                if tail.starts_with(sch) {
                    return None;
                }
            }
        }
        for sch in &[
            "about:",
            "data:",
            "javascript:",
            "blob:",
            "mailto:",
            "tel:",
            "view-source:",
        ] {
            if relative.starts_with(sch) {
                return None;
            }
        }
        let base_url = url::Url::parse(base).ok()?;
        let joined = base_url.join(relative).ok()?;
        // Reject any joined URL whose path begins with a special-scheme
        // literal (catches the rare case where the input was a clean
        // relative path but contained an embedded `about:blank` segment).
        if let Some(path) = joined.path().strip_prefix('/') {
            for sch in &[
                "about:",
                "data:",
                "javascript:",
                "blob:",
                "mailto:",
                "tel:",
                "view-source:",
            ] {
                if path.starts_with(sch) {
                    return None;
                }
            }
        }
        // We can only fetch http/https. about:blank, data:, blob:,
        // javascript:, chrome-extension:, etc. either have no host
        // (Url::host_str() returns None, causing "no host in URL"
        // downstream) or aren't network-addressable.
        match joined.scheme() {
            "http" | "https" => Some(joined.to_string()),
            _ => None,
        }
    }

    #[allow(dead_code)]
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
        let bp_trace = std::env::var("BROWSER_OXIDE_BUILD_PROFILE").is_ok();
        let bp_t0 = std::time::Instant::now();
        macro_rules! mark {
            ($label:expr) => {
                if bp_trace {
                    eprintln!("[bp] {:>5}ms {}", bp_t0.elapsed().as_millis(), $label);
                }
            };
        }
        let dom = html_parser::parse_html(html);
        let scripts = script_runner::find_scripts(&dom);
        let stylesheet_entries = stylesheet_collector::find_stylesheets(&dom);
        mark!("parse_html + find_scripts + find_stylesheets");

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
                        && std::env::var("BROWSER_OXIDE_DD_TRACE").is_ok();
                    // Phase 5 (homedepot): trace EVERY external-script
                    // fetch when BROWSER_OXIDE_SC_TRACE is set, so we can see
                    // whether the obfuscated `/Wjv3…` sec-cpt bundle is
                    // actually fetched + its size/status (the unknown the
                    // "bundle doesn't self-solve" verdict assumed but
                    // never measured). Env-gated, default off ⇒ zero §4
                    // gate impact.
                    let sc_trace = std::env::var("BROWSER_OXIDE_SC_TRACE").is_ok();
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
        mark!("subresource fetch join (css + scripts)");

        let mut all_timings = Vec::new();

        // Build stylesheet list: inline first, then fetched external
        let mut stylesheets = inline_css;
        for (css, timings) in fetched_css_results.into_iter().flatten() {
            stylesheets.push(css);
            all_timings.push(timings);
        }

        // Build pre-fetched script map
        let mut prefetched = std::collections::HashMap::new();
        for (i, text, timings) in fetched_scripts_results.into_iter().flatten() {
            prefetched.insert(i, text);
            all_timings.push(timings);
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
        mark!("BrowserJsRuntime::with_options (V8 isolate + bootstrap)");

        // Install all sub-resource timings
        for timings in all_timings {
            event_loop.runtime_mut().record_resource_timing(timings);
        }
        mark!("record_resource_timing");

        // Install a build-phase V8 deadline watcher to preempt CPU-bound
        // inline-script execution (delta.com, taobao.com — pages whose
        // first-paint scripts spawn document.write(<script>) chains or
        // tight setTimeout polling that hold the V8 thread indefinitely).
        // 25s is generous: any honest first-paint completes well under it.
        // Without this, build_page_with_scripts_and_init can run forever
        // because tokio::time::timeout cannot preempt V8 microtask spins.
        let build_budget_ms: u64 = std::env::var("BROWSER_OXIDE_BUILD_BUDGET_MS")
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
        mark!("location.href setup + reset_nav_pending");

        // Synchronize cookies from the net client so document.cookie is accurate.
        // This is one async op (`op_cookie_get`) — it returns in ≪1 ms. The drain
        // here only needs to flush that one microtask. The previous 1 s cap was
        // pessimistic: with `humanize.js` already installed (~30 pending
        // setTimeouts that don't resolve for ~2 s), the drain hits its full
        // timeout on every navigation regardless of how trivial the page is.
        // 50 ms is more than enough for the one cookie microtask to land, and
        // the humanize timers continue firing during the outer nav-loop drain
        // where the budget is correctly allocated.
        let _ = event_loop
            .execute_and_run(
                "globalThis.__syncCookiesFromNet && globalThis.__syncCookiesFromNet();",
                Duration::from_millis(50),
            )
            .await;
        mark!("__syncCookiesFromNet");

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
        mark!("install cookie-write instrumentation");

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
                const log = globalThis._browser_oxide && globalThis._browser_oxide.__fetchLog;
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
                const log = globalThis._browser_oxide && globalThis._browser_oxide.__fetchLog;
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
                    const log = globalThis._browser_oxide && globalThis._browser_oxide.__fetchLog;
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
        mark!("install error + fetch/XHR instrumentation");

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
                // Real Chrome inline <script> stack frames report the
                // document URL, not a synthetic <script_N> tag. The
                // latter would leak the index/wrapper layer to Kasada /
                // DataDome.
                url.to_string()
            };
            if let Err(e) = event_loop.execute_script_with_name(&code, &name) {
                tracing::warn!(script = %name, error = %e, "Script execution error");
            }

            // Flush logs for this script
            {
                let _ = &script.src;
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
        mark!("inline scripts + interleaved drains");

        // Final cleanup — hides Deno and internal globals from user JS.
        event_loop
            .execute_script(include_str!("../../js_runtime/src/js/cleanup_bootstrap.js"))
            .ok();
        mark!("cleanup_bootstrap.js");

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

        mark!("DOMContentLoaded/load setTimeout install");

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

        mark!("meta-refresh scanner install");

        // Run event loop until idle. Script errors should NOT abort
        // navigation — log and continue, matching real browser behavior.
        //
        // 8 s cap. This drain flushes microtasks, zero-delay setTimeouts
        // (DOMContentLoaded / load handlers, meta-refresh scanner), and
        // any short async chains kicked off by inline scripts.
        //
        // Important: `humanize.js` now schedules its synthetic mouse /
        // scroll timers via `globalThis.__bgSetTimeout` (timer_bootstrap.js)
        // which is `.unref()`'d — so the humanize setTimeouts no longer
        // pin this drain to its full ceiling. Benign pages exit idle in
        // milliseconds; anti-bot challenge pages (AWS WAF / reddit verify /
        // recaptcha invisible / DataDome) get the full 8 s they need for
        // their async POST+reload chain to complete, which sets
        // `__pendingNavigation` and triggers iter 1 with the proper token
        // cookie. Cutting this below ~5 s causes those chains to never
        // complete and the outer loop returns the challenge stub as the
        // "rendered" page.
        if let Err(e) = event_loop.run_until_idle(Duration::from_secs(8)).await {
            tracing::warn!(error = %e, "Event loop error during run");
        }
        mark!("build-phase run_until_idle");

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
                match iframe::ChildIframe::from_srcdoc(info.node_id, srcdoc, profile).await {
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
                        profile,
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
        mark!("post-drain summary + iframes + watcher cleanup [DONE]");

        Ok(Self {
            event_loop,
            url: url.to_string(),
            children,
            solvers: std::sync::Arc::from(Vec::<std::sync::Arc<dyn crate::ChallengeSolver>>::new()),
        })
    }

    /// Consume the page and return the DOM.
    pub fn take_dom(mut self) -> Dom {
        // Drop children first (V8 reverse order requirement)
        self.children.clear();
        // Use ManuallyDrop to prevent the Drop impl from running
        let page = std::mem::ManuallyDrop::new(self);
        // SAFETY: `page` is `ManuallyDrop`, so its destructor will not
        // run and won't double-drop the bytes we read out of it.
        // `event_loop` is read by value exactly once via `ptr::read`,
        // and nothing else touches it after this — the surrounding
        // `ManuallyDrop` ensures the original location is never used
        // again (no aliasing, no double-free). The `children` field
        // that the event loop depends on was already cleared above
        // per V8's reverse-drop-order requirement.
        unsafe {
            let event_loop = std::ptr::read(&page.event_loop);
            event_loop.take_dom()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// R-DATADOME-DAILY-KEY: `is_datadome_challenge` must catch a typical
    /// `rt:'i'` interstitial (small body + captcha-delivery.com script).
    #[test]
    fn datadome_challenge_detects_interstitial() {
        let interstitial = r#"<html><head><script src="https://geo.captcha-delivery.com/c"></script></head><body></body></html>"#;
        assert!(is_datadome_challenge(interstitial));
    }

    /// Legitimate page that mentions captcha-delivery.com in a benign
    /// context (e.g. CSP report-uri or text content) must NOT be flagged.
    /// We rely on the 50 KB size gate to differentiate the small
    /// interstitial from a full rendered page.
    #[test]
    fn datadome_challenge_size_gates_false_positive() {
        let mut big = String::from("<html><body>");
        // 60 KB filler text containing the substring
        big.push_str(&"x".repeat(60_000));
        big.push_str("captcha-delivery.com (mentioned in passing)");
        big.push_str("</body></html>");
        assert!(!is_datadome_challenge(&big));
    }

    /// A vanilla rendered page with no DD substring is not a challenge.
    #[test]
    fn datadome_challenge_rejects_non_dd_body() {
        let html =
            r#"<html><head><title>Real Site</title></head><body><h1>Welcome</h1></body></html>"#;
        assert!(!is_datadome_challenge(html));
    }

    /// `is_datadome_solved` requires BOTH the `datadome=` cookie AND a
    /// body that is no longer a DD interstitial. The cookie alone is
    /// not a solve marker (FP-D3: DD sets the cookie on every nav incl.
    /// the failing 403).
    #[test]
    fn datadome_solved_requires_cookie_and_clean_body() {
        let real_body = "<html><body><h1>Real Page</h1></body></html>";
        let interstitial = r#"<html><body><script src="https://geo.captcha-delivery.com/c"></script></body></html>"#;

        // Cookie + real body → solved.
        assert!(is_datadome_solved("datadome=abc123", real_body));

        // Cookie + interstitial body → NOT solved (still on the challenge).
        assert!(!is_datadome_solved("datadome=abc123", interstitial));

        // No cookie + real body → NOT solved (we never saw a token).
        assert!(!is_datadome_solved("session=x; other=y", real_body));
    }

    /// Regression: a JS-side `location.href = 'about:blank'` (or any
    /// other special-scheme URL) must NOT cause the navigate loop to
    /// fetch `https://host/about:blank`. Caught on iphey.com where the
    /// URL polyfill mis-joined the special scheme and broke same-page
    /// rendering (THIN-BODY 901b instead of L3-RENDERED 29 KB).
    #[test]
    fn resolve_url_rejects_special_scheme_relative() {
        // Bare special-scheme strings → no-op
        assert_eq!(Page::resolve_url("https://iphey.com/", "about:blank"), None);
        assert_eq!(
            Page::resolve_url("https://iphey.com/", "data:text/html,<p>x</p>"),
            None
        );
        assert_eq!(
            Page::resolve_url("https://iphey.com/", "javascript:void(0)"),
            None
        );
        assert_eq!(
            Page::resolve_url("https://iphey.com/", "blob:https://x/y"),
            None
        );
        // Path-encoded special schemes (the iphey symptom — pending URL
        // arrives as the literal "https://iphey.com/about:blank") → no-op
        assert_eq!(
            Page::resolve_url("https://iphey.com/", "https://iphey.com/about:blank"),
            None
        );
        assert_eq!(
            Page::resolve_url("https://iphey.com/", "/about:blank"),
            None
        );
        // Normal navigations are unaffected
        assert_eq!(
            Page::resolve_url("https://iphey.com/", "/page2"),
            Some("https://iphey.com/page2".to_string())
        );
        assert_eq!(
            Page::resolve_url("https://iphey.com/", "https://other.example/foo"),
            Some("https://other.example/foo".to_string())
        );
    }

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
    async fn page_webdriver_false() {
        // Modern Chrome (>=89, incl. Chrome-148): navigator.webdriver
        // === false for normal browsing (`undefined` is the old/headless
        // tell). K2-DIFF wdt fix — evidence-backed (Kasada flagged
        // wdt.r="undefined"); supersedes the prior W3C-spec misreading.
        let mut page = Page::from_html(
            "<html><head></head><body></body></html>",
            None::<stealth::StealthProfile>,
        )
        .await
        .unwrap();
        let result = page.evaluate("typeof navigator.webdriver").unwrap();
        assert_eq!(result, "boolean");
        let val = page.evaluate("navigator.webdriver").unwrap();
        assert_eq!(val, "false");
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
        let profile = stealth::presets::chrome_148_macos();
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
        let profile = stealth::presets::chrome_148_windows();
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
    ///
    /// Ignored: needs real `canvas.getContext('2d')` font-metrics — the
    /// `Page::from_html` test harness initialises a context that can't
    /// resolve named font families, so the assertion fails in the
    /// default test env even though the behaviour is correct against a
    /// real browser. Run with `--ignored` after wiring a fuller canvas
    /// context into the unit-test harness.
    #[tokio::test]
    #[ignore = "needs real canvas getContext in the test harness"]
    async fn canvas_font_detection_macos_helvetica_neue() {
        let profile = stealth::presets::chrome_148_macos();
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
    ///
    /// Ignored: same canvas-getContext harness limitation as
    /// `canvas_font_detection_macos_helvetica_neue` above.
    #[tokio::test]
    #[ignore = "needs real canvas getContext in the test harness"]
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
        assert!(!ps.is_empty(), "expected at least 1 <p>, got {}", ps.len());
        assert_eq!(dom.text_content(ps[0]), "test");
    }

    // --- Network integration tests (require internet) ---

    #[tokio::test]
    #[ignore]
    async fn navigate_httpbin() {
        let profile = stealth::presets::chrome_148_linux();
        let client = net::HttpClient::new(&profile).unwrap();
        let mut page = Page::navigate_simple(
            "https://httpbin.org/html",
            &client,
            stealth::presets::chrome_148_ru(),
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
        let profile = stealth::presets::chrome_148_windows();
        let client = net::HttpClient::new(&profile).unwrap();
        let mut page = Page::navigate_simple(
            "https://httpbin.org/user-agent",
            &client,
            stealth::presets::chrome_148_ru(),
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
        let profile = stealth::presets::chrome_148_linux();
        let client = net::HttpClient::new(&profile).unwrap();
        let mut page = Page::navigate_simple(
            "https://httpbin.org/headers",
            &client,
            stealth::presets::chrome_148_ru(),
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
        let profile = stealth::presets::chrome_148_linux();
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
