use crate::iframe;
use crate::script_runner;
use crate::stylesheet_collector;
use dom::Dom;
use event_loop::{BrowserEventLoop, IdleReason};
use js_runtime::{runtime::BrowserRuntimeOptions, BrowserJsRuntime};
use std::time::Duration;
use stealth;
use tracing;

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
    /// Detect if the current page is an anti-bot challenge (Kasada, Akamai, etc.)
    pub fn is_anti_bot_challenge(&mut self) -> bool {
        let body = self.content();
        body.contains("ips.js") || 
        body.contains("kpsdk") || 
        body.contains("_abck") || 
        body.contains("bm_sz") ||
        body.contains("perimeterx") ||
        body.contains("human security") ||
        body.contains("smartcaptcha") ||
        body.contains("checkbox-captcha")
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
    pub async fn from_html_fast(html: &str, url: &str, profile: stealth::StealthProfile) -> Result<Self, deno_core::error::AnyError> {
        let dom = html_parser::parse_html(html);
        let scripts = script_runner::find_scripts(&dom);
        let stylesheet_entries = stylesheet_collector::find_stylesheets(&dom);
        let stylesheets = stylesheet_collector::resolve_inline_only(&stylesheet_entries);

        let runtime = BrowserJsRuntime::with_options(
            dom,
            BrowserRuntimeOptions {
                stealth_profile: Some(profile.clone()),
                stylesheets,
                ..Default::default()
            },
        );
        let mut event_loop = BrowserEventLoop::new(runtime);

        // Set location.href
        let url_js = url.replace('\\', "\\\\").replace('\'', "\\'");
        event_loop
            .execute_script(&format!("location.href = '{}';", url_js))
            .ok();

        // Share the HTTP client with JS fetch()
        let client = net::HttpClient::new(&profile)
            .map_err(|e| deno_core::error::AnyError::msg(e.to_string()))?;
        js_runtime::extensions::fetch_ext::set_fetch_client(client.clone());

        // Execute inline scripts (fast mode skips external scripts by default)
        for (i, script) in scripts.iter().enumerate() {
            if script.src.is_some() { continue; }
            if !script.code.is_empty() {
                if let Err(e) = event_loop.execute_script(&script.code) {
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

        // Update URL
        self.url = url.to_string();
        let url_js = url.replace('\\', "\\\\").replace('\'', "\\'");
        self.event_loop
            .execute_script(&format!("location.href = '{}';", url_js))
            .ok();

        // Execute inline scripts in document order
        for (i, script) in scripts.iter().enumerate() {
            if script.src.is_some() {
                continue; // skip external scripts — caller handles fetching
            }
            if script.code.trim().is_empty() {
                continue;
            }
            if let Err(e) = self.event_loop.execute_script(&script.code) {
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

        // Find scripts and stylesheets before handing DOM to runtime
        let scripts = script_runner::find_scripts(&dom);
        let stylesheet_entries = stylesheet_collector::find_stylesheets(&dom);
        let stylesheets = stylesheet_collector::resolve_inline_only(&stylesheet_entries);

        let runtime = BrowserJsRuntime::with_options(
            dom,
            BrowserRuntimeOptions {
                stealth_profile: profile.clone(),
                stylesheets,
                ..Default::default()
            },
        );
        let mut event_loop = BrowserEventLoop::new(runtime);

        // Set location.href
        let url_js = url.replace('\\', "\\\\").replace('\'', "\\'");
        event_loop
            .execute_script(&format!("location.href = '{}';", url_js))
            .ok();

        // Share the HTTP client with JS fetch()
        let p = profile.unwrap_or_else(stealth::presets::chrome_130_ru);
        let client = net::HttpClient::new(&p)
            .map_err(|e| deno_core::error::AnyError::msg(e.to_string()))?;
        js_runtime::extensions::fetch_ext::set_fetch_client(client.clone());

        // Execute scripts in document order
        for (i, script) in scripts.iter().enumerate() {
            if let Some(src) = &script.src {
                if let Some(full_url) = Self::resolve_url(url, src) {
                    match client.get_follow(&full_url, 10).await {
                        Ok(resp) => {
                            let code = resp.text();
                            if let Err(e) = event_loop.execute_script(&code) {
                                tracing::warn!(script_src = %src, error = %e, "Script error in external script");
                            }
                        }
                        Err(e) => tracing::warn!(script_src = %src, error = %e, "Failed to fetch script"),
                    }
                }
            } else if !script.code.is_empty() {
                if let Err(e) = event_loop.execute_script(&script.code) {
                    tracing::warn!(script_index = i, error = %e, "Script error in inline script");
                }
            }
        }

        // Set document.readyState = loading
        event_loop.execute_script("globalThis.__documentReadyState = 'loading';").ok();

        // Fire DOMContentLoaded and load events — many scripts wait for these
        event_loop
            .execute_script("document.dispatchEvent(new Event('DOMContentLoaded', {bubbles: true}));")
            .ok();
        
        // After DOMContentLoaded, readyState = interactive
        event_loop.execute_script("globalThis.__documentReadyState = 'interactive';").ok();

        event_loop
            .execute_script("window.dispatchEvent(new Event('load'));")
            .ok();

        // After load, readyState = complete
        event_loop.execute_script("globalThis.__documentReadyState = 'complete';").ok();

        // Run event loop until idle (max 30s)
        event_loop.run_until_idle(Duration::from_secs(30)).await?;

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
        Self::navigate_with_init(url, profile, max_iterations, Vec::new()).await
    }

    /// Like [`Page::navigate`], but also installs an input-humanizer init
    /// script that dispatches mousemove/click/focus events on every
    /// navigation. Opt-in because synthetic input is a workaround for
    /// sensor-based detectors, not a semantic part of page loading.
    pub async fn navigate_humanized(
        url: &str,
        profile: stealth::StealthProfile,
        max_iterations: u8,
    ) -> Result<Self, deno_core::error::AnyError> {
        let humanize = include_str!("js/humanize.js").to_string();
        Self::navigate_with_init(url, profile, max_iterations, vec![humanize]).await
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
        let html = resp.text();
        let resp_url = resp.url.clone();
        Self::navigate_loop_internal(html, resp_url, profile, client, iterations, 0, init_scripts, debug_nav).await
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

        // Iteration 0 uses provided HTML
        Self::navigate_loop_internal(html.to_string(), url.to_string(), profile, client, iterations, 0, vec![], debug_nav).await
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
    ) -> Result<Self, deno_core::error::AnyError> {
        const PENDING_NAV_JS: &str = "(function(){\
                const p = globalThis.__pendingNavigation;\
                return p ? JSON.stringify({url: p.url, method: p.method || 'GET', body: p.body, kind: p.kind}) : '';\
            })()";

        let mut current_html = html;
        let mut current_url = resp_url;

        for iter in start_iter..iterations {
            tracing::info!(iter = iter, url = %current_url, "navigation loop start");
            if debug_nav {
                eprintln!("[navigate] iter={} url={} html_len={}", iter, current_url, current_html.len());
            }

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

            let mut page = Self::build_page_with_scripts_and_init(
                &current_html,
                &current_url,
                &profile,
                &client,
                &init_scripts,
            )
            .await?;

            // Drain the event loop
            if let Err(e) = page
                .event_loop()
                .run_until_idle(Duration::from_secs(30))
                .await
            {
                tracing::warn!(error = %e, "navigate event loop error");
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
            if pending_info.is_empty() && page.is_anti_bot_challenge() {
                tracing::info!("anti-bot challenge detected with no pending navigation, polling up to 10s...");
                let deadline = std::time::Instant::now() + Duration::from_secs(10);
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
                        tracing::info!(pending = %pending_info, "pending navigation found during poll");
                        break;
                    }
                }
            }

            if pending_info.is_empty() {
                // Post-settle cookie-delta retry: if the cookie jar gained new
                // values during this iteration AND the page still looks like a
                // challenge AND we have iterations left, retry the same URL
                // once. Covers engines whose solver sets a session cookie and
                // expects the NEXT top-level nav to carry it (Kasada; some
                // Akamai variants). Universal primitive — no per-engine code.
                if page.is_anti_bot_challenge() && iter + 1 < iterations {
                    let cookies_after: String = if let Some(p) = parsed_current.as_ref() {
                        client.cookies_for_url(p).await.unwrap_or_default()
                    } else {
                        String::new()
                    };
                    if debug_nav {
                        eprintln!(
                            "[navigate] iter={} cookies before={}b after={}b delta={}",
                            iter,
                            cookies_before.len(),
                            cookies_after.len(),
                            cookies_after != cookies_before
                        );
                    }
                    if cookies_after != cookies_before && !cookies_after.is_empty() {
                        if debug_nav {
                            eprintln!("[navigate] iter={} POST-SETTLE RETRY firing for {}", iter, current_url);
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
                        let v8_html_is_real = !v8_html.is_empty()
                            && v8_html.len() > current_html.len()
                            && !v8_html.contains("/ips.js")
                            && !v8_html.contains("/149e9513-")
                            && !v8_html.contains("kpsdk")
                            && !v8_html.contains("_abck")
                            && !v8_html.contains("bm_sz");

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
                            eprintln!("[navigate] iter={} harvested x-kpsdk-* headers: {:?}", iter, keys);
                        }

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
                        } else {
                            // Reload-style headers + harvested x-kpsdk-*
                            // tokens on the retry GET.
                            let mut reload_hdrs = net::headers::chrome_headers_reload(&profile, &current_url);
                            for (k, v) in &kpsdk {
                                reload_hdrs.push((k.clone(), v.clone()));
                            }
                            let resp = client
                                .get_follow_exact_headers(&current_url, &reload_hdrs, 10)
                                .await
                                .map_err(|e| deno_core::error::AnyError::msg(e.to_string()))?;
                            current_html = resp.text();
                            current_url = resp.url.clone();
                        }
                        continue;
                    }
                }
                return Ok(page);
            }

            let p: deno_core::serde_json::Value = deno_core::serde_json::from_str(&pending_info).unwrap_or_default();
            let pending_url = p["url"].as_str().unwrap_or_default();
            let pending_method = p["method"].as_str().unwrap_or("GET").to_string();
            let pending_body = p["body"].as_str().map(|s| s.to_string());
            let kind = p["kind"].as_str().unwrap_or("unknown");

            if pending_url.is_empty() {
                return Ok(page);
            }

            // Resolve relative pending URLs
            let next_url = Self::resolve_url(&current_url, pending_url)
                .ok_or_else(|| deno_core::error::AnyError::msg("Failed to resolve pending URL"))?;
            tracing::debug!(kind = kind, url = %next_url, method = %pending_method, "navigate pending navigation");

            if iter + 1 == iterations {
                tracing::warn!(max_iterations = iterations, "navigate hit max iterations, returning current page");
                return Ok(page);
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
            let same_host_reload = pending_method == "GET"
                && page.is_anti_bot_challenge()
                && {
                    let cur = url::Url::parse(&current_url).ok();
                    let nxt = url::Url::parse(&next_url).ok();
                    matches!(
                        (cur, nxt),
                        (Some(a), Some(b)) if a.host_str() == b.host_str()
                    )
                };
            let v8_refetched: Option<String> = if same_host_reload {
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
                    url_js = deno_core::serde_json::to_string(&next_url).unwrap_or_else(|_| "''".to_string())
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
                eprintln!("[navigate] iter={} FETCH {} {}", iter, pending_method, next_url);
            }

            // Fetch the next page. For form POSTs we must send the form
            // Content-Type or the server can't parse the body. For GETs that
            // are same-origin reload-style navigations (location.href/reload
            // assign from JS), use reload-semantic headers so engines can
            // distinguish a solved-session reload from a fresh user nav.
            let resp = if pending_method == "POST" {
                let post_headers = vec![
                    ("content-type".to_string(), "application/x-www-form-urlencoded".to_string()),
                    ("origin".to_string(), {
                        url::Url::parse(&current_url)
                            .ok()
                            .and_then(|u| u.origin().ascii_serialization().into())
                            .unwrap_or_default()
                    }),
                    ("referer".to_string(), current_url.clone()),
                ];
                client
                    .post_bytes_follow(&next_url, pending_body.as_deref().unwrap_or("").as_bytes(), &post_headers, 10)
                    .await
                    .map_err(|e| deno_core::error::AnyError::msg(e.to_string()))?
            } else {
                let same_origin = {
                    let a = url::Url::parse(&current_url).ok();
                    let b = url::Url::parse(&next_url).ok();
                    matches!((a, b), (Some(u), Some(v)) if u.host_str() == v.host_str())
                };
                if same_origin {
                    let mut reload_hdrs = net::headers::chrome_headers_reload(&profile, &current_url);
                    for (k, v) in &harvested_kpsdk {
                        reload_hdrs.push((k.clone(), v.clone()));
                    }
                    client
                        .get_follow_exact_headers(&next_url, &reload_hdrs, 10)
                        .await
                        .map_err(|e| deno_core::error::AnyError::msg(e.to_string()))?
                } else {
                    client
                        .get_follow(&next_url, 10)
                        .await
                        .map_err(|e| deno_core::error::AnyError::msg(e.to_string()))?
                }
            };
            current_html = resp.text();
            current_url = resp.url.clone();
        }

        // Fallback (should be reachable via the loop)
        Err(deno_core::error::AnyError::msg("Navigation loop terminated without returning a page"))
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
        base_url.join(relative).ok().map(|u| u.to_string())
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
                                    Some(text)
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
                let client = client.clone();
                let profile = profile.clone();
                Some(async move {
                    let mut hdrs = net::headers::chrome_headers(&profile);
                    hdrs.push(("referer".to_string(), url.to_string()));
                    hdrs.push(("accept".to_string(), "*/*".to_string()));
                    hdrs.push(("sec-fetch-dest".to_string(), "script".to_string()));
                    hdrs.push(("sec-fetch-mode".to_string(), "no-cors".to_string()));
                    hdrs.push(("sec-fetch-site".to_string(), "cross-site".to_string()));
                    
                    match client.get_follow_with_headers(&full_url, &hdrs, 5).await {
                        Ok(resp) if resp.ok() => {
                            let text = resp.text();
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
                                Some((i, text))
                            }
                        }
                        Ok(resp) => {
                            tracing::warn!(script_index = i, url = %full_url, status = resp.status, "Script fetch returned non-OK status");
                            None
                        }
                        Err(e) => {
                            tracing::warn!(script_index = i, url = %full_url, error = ?e, "Script fetch failed");
                            None
                        }
                    }
                })
            })
            .collect();

        // Await all fetches in parallel
        let (fetched_css, fetched_scripts) = futures_util::future::join(
            futures_util::future::join_all(css_futures),
            futures_util::future::join_all(script_futures),
        )
        .await;

        // Build stylesheet list: inline first, then fetched external
        let mut stylesheets = inline_css;
        for css in fetched_css.into_iter().flatten() {
            stylesheets.push(css);
        }

        // Build pre-fetched script map
        let prefetched: std::collections::HashMap<usize, String> =
            fetched_scripts.into_iter().flatten().collect();

        let runtime = BrowserJsRuntime::with_options(
            dom,
            BrowserRuntimeOptions {
                stealth_profile: Some(profile.clone()),
                stylesheets,
                init_scripts: init_scripts.to_vec(),
                ..Default::default()
            },
        );
        let mut event_loop = BrowserEventLoop::new(runtime);

        // Set location
        let url_js = url.replace('\\', "\\\\").replace('\'', "\\'");
        if let Err(e) = event_loop.execute_script(&format!("location.href = '{}';", url_js)) {
            tracing::error!(error = %e, "Failed to set location");
        }
        let loc = event_loop.execute_script("globalThis.location.href").unwrap_or_default();
        tracing::debug!(location = %loc, "Location set");
        
        // Synchronize cookies from the net client so document.cookie is accurate
        let _ = event_loop.execute_and_run("globalThis.__syncCookiesFromNet && globalThis.__syncCookiesFromNet();", Duration::from_secs(1)).await;

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
            Object.defineProperty(window, '__fetchLog', { value: [], enumerable: false, configurable: true });
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
                if (!window.__fetchLog) window.__fetchLog = [];
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
                window.__fetchLog.push(entry);
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
            };"#)
            .ok();

        // Execute scripts in document order using pre-fetched code.
        // Interleave with event loop ticks to allow for microtasks and
        // macrotasks scheduled by one script to run before the next.
        for (i, script) in scripts.iter().enumerate() {
            let code = if script.src.is_some() {
                match prefetched.get(&i) {
                    Some(code) => code.clone(),
                    None => {
                        tracing::warn!(script_index = i, "Script not prefetched (fetch failed), skipping");
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
                format!("<script_{}>", i)
            };
            if let Err(e) = event_loop.execute_script_with_name(&code, &name) {
                tracing::warn!(script = %name, error = %e, "Script execution error");
            }

            // Flush logs for this script
            {
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
            .execute_script(r#"
            setTimeout(() => {
                document.dispatchEvent(new Event('DOMContentLoaded', {bubbles: true}));
                window.dispatchEvent(new Event('DOMContentLoaded', {bubbles: true}));
                window.dispatchEvent(new Event('load'));
            }, 0);
        "#)
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
                    }, delay * 1000);
                    break;
                }
            })();
        "#)
            .ok();

        // Run event loop until idle. Script errors should NOT abort
        // navigation — log and continue, matching real browser behavior.
        if let Err(e) = event_loop.run_until_idle(Duration::from_secs(30)).await {
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
            })))"#) {
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
                if !src.is_empty() {
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
                            Err(e) => tracing::warn!(src = %full_src, error = %e, "iframe src error"),
                        }
                    }
                }
            }
        }

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
        let mut page = Page::from_html("<html><head><title>Test</title></head><body><p>Hello</p></body></html>", None::<stealth::StealthProfile>)
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
        let mut page = Page::from_html(r#"<html><head></head><body>
                <script>
                    const p = document.createElement('p');
                    p.setAttribute('id', 'created');
                    p.textContent = 'Dynamic content';
                    document.body.appendChild(p);
                </script>
            </body></html>"#, None::<stealth::StealthProfile>)
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
        let mut page = Page::from_html(r#"<html><head></head><body>
                <div id="output">before</div>
                <script>
                    setTimeout(() => {
                        document.getElementById('output').textContent = 'after';
                    }, 50);
                </script>
            </body></html>"#, None::<stealth::StealthProfile>)
        .await
        .unwrap();
        assert_eq!(page.text_of("#output"), Some("after".to_string()));
    }

    #[tokio::test]
    async fn page_evaluate() {
        let mut page = Page::from_html("<html><head></head><body></body></html>", None::<stealth::StealthProfile>)
            .await
            .unwrap();
        let result = page.evaluate("1 + 2").unwrap();
        assert_eq!(result, "3");
    }

    #[tokio::test]
    async fn page_navigator_exists() {
        let mut page = Page::from_html("<html><head></head><body></body></html>", None::<stealth::StealthProfile>)
            .await
            .unwrap();
        let result = page.evaluate("typeof navigator.userAgent").unwrap();
        assert_eq!(result, "string");
    }

    #[tokio::test]
    async fn page_document_has_focus() {
        let mut page = Page::from_html("<html><head></head><body></body></html>", None::<stealth::StealthProfile>)
            .await
            .unwrap();
        let result = page.evaluate("document.hasFocus()").unwrap();
        assert_eq!(result, "true");
    }

    #[tokio::test]
    async fn page_webdriver_false() {
        let mut page = Page::from_html("<html><head></head><body></body></html>", None::<stealth::StealthProfile>)
            .await
            .unwrap();
        let result = page.evaluate("typeof navigator.webdriver").unwrap();
        assert_eq!(result, "boolean");
        let val = page.evaluate("navigator.webdriver").unwrap();
        assert_eq!(val, "false");
    }

    #[tokio::test]
    async fn page_window_dimensions() {
        let mut page = Page::from_html("<html><head></head><body></body></html>", None::<stealth::StealthProfile>)
            .await
            .unwrap();
        let w = page.evaluate("window.innerWidth").unwrap();
        assert_eq!(w, "1920");
        let h = page.evaluate("window.innerHeight").unwrap();
        assert_eq!(h, "1080");
    }

    #[tokio::test]
    async fn page_local_storage() {
        let mut page = Page::from_html("<html><head></head><body></body></html>", None::<stealth::StealthProfile>)
            .await
            .unwrap();
        page.evaluate("localStorage.setItem('key', 'value')")
            .unwrap();
        let result = page.evaluate("localStorage.getItem('key')").unwrap();
        assert_eq!(result, "value");
    }

    #[tokio::test]
    async fn page_crypto_random() {
        let mut page = Page::from_html("<html><head></head><body></body></html>", None::<stealth::StealthProfile>)
            .await
            .unwrap();
        let result = page
            .evaluate("typeof crypto.getRandomValues(new Uint8Array(4))")
            .unwrap();
        assert_eq!(result, "object");
    }

    #[tokio::test]
    async fn page_promise_then() {
        let mut page = Page::from_html(r#"<html><head></head><body>
                <div id="out">waiting</div>
                <script>
                    Promise.resolve('done').then(v => {
                        document.getElementById('out').textContent = v;
                    });
                </script>
            </body></html>"#, None::<stealth::StealthProfile>)
        .await
        .unwrap();
        assert_eq!(page.text_of("#out"), Some("done".to_string()));
    }

    #[tokio::test]
    async fn page_multiple_scripts() {
        let mut page = Page::from_html(r#"<html><head></head><body>
                <div id="out"></div>
                <script>document.getElementById('out').textContent = 'A';</script>
                <script>document.getElementById('out').textContent += 'B';</script>
                <script>document.getElementById('out').textContent += 'C';</script>
            </body></html>"#, None::<stealth::StealthProfile>)
        .await
        .unwrap();
        assert_eq!(page.text_of("#out"), Some("ABC".to_string()));
    }

    #[tokio::test]
    async fn page_take_dom() {
        let page = Page::from_html("<html><head></head><body><p>test</p></body></html>", None::<stealth::StealthProfile>)
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
        let mut page = Page::navigate_simple("https://httpbin.org/html", &client, stealth::presets::chrome_130_ru())
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
        let mut page = Page::navigate_simple("https://httpbin.org/user-agent", &client, stealth::presets::chrome_130_ru())
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
        let mut page = Page::navigate_simple("https://httpbin.org/headers", &client, stealth::presets::chrome_130_ru())
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
