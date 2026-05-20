use crate::state::DomState;
use deno_core::op2;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;
use url::Url;

/// Per-page sync-fetch chain ceiling. Without this, sites like
/// delta.com and taobao.com cascade nested document.write(<script src>)
/// + setTimeout-driven JSONP polls indefinitely, holding the V8 worker
/// thread for minutes and starving tokio of yield points.
///
/// 30 is comfortable: any anti-bot vendor's solver chain fits in
/// <10 sync fetches, leaving headroom for legitimate inline scripts.
const MAX_SYNC_FETCH_PER_PAGE: usize = 30;
static SYNC_FETCH_COUNT: AtomicUsize = AtomicUsize::new(0);

/// Reset the per-page sync-fetch counter. Called by Page::navigate_with_init
/// at the start of each navigation iteration.
pub fn reset_sync_fetch_count() {
    SYNC_FETCH_COUNT.store(0, Ordering::Relaxed);
}

pub fn record_resource_timing(state: &mut deno_core::OpState, timings: net::TimingStats) {
    if let Some(dom_state) = state.try_borrow_mut::<DomState>() {
        dom_state.resource_timings.push(timings);
    }
}

/// HTTP client state stored in OpState.
pub struct FetchState {
    pub client: Option<net::HttpClient>,
}

impl FetchState {
    pub fn new(client: Option<net::HttpClient>) -> Self {
        Self { client }
    }

    pub fn with_profile(profile: &stealth::StealthProfile) -> Self {
        Self {
            client: net::HttpClient::new(profile).ok(),
        }
    }
}

/// Shared fetch client, initialized once from the stealth profile.
/// Used by the async op_fetch since deno_core async ops can't borrow OpState.
static FETCH_CLIENT: OnceLock<net::HttpClient> = OnceLock::new();

/// Active CSP policy + origin, mirrored from `DomState` so async
/// `op_fetch` can enforce without borrowing `OpState`. Updated per
/// top-level navigation by `set_csp_policy`. Reset (set to None-style
/// empty) by `clear_csp_policy` between navigations.
///
/// We use `std::sync::RwLock<Option<...>>` instead of `OnceLock` here
/// because the policy genuinely changes per navigation; OnceLock would
/// strand stale walmart policy on the next site we visit.
static ACTIVE_CSP: std::sync::OnceLock<std::sync::RwLock<Option<ActiveCsp>>> =
    std::sync::OnceLock::new();

#[derive(Clone)]
struct ActiveCsp {
    policy: std::sync::Arc<net::csp::PolicySet>,
    origin: Url,
    enforce: bool,
}

fn active_csp_lock() -> &'static std::sync::RwLock<Option<ActiveCsp>> {
    ACTIVE_CSP.get_or_init(|| std::sync::RwLock::new(None))
}

/// Install a CSP policy + origin for the current navigation. `enforce`
/// is wired from `profile.enforce_csp` and the `BOXIDE_CSP_BYPASS=1`
/// escape hatch. Called by `Page::navigate_with_init` after parsing
/// the response headers + meta tags.
///
/// Drains the violation queue at install time — violations from the
/// previous document are no longer relevant once a new navigation
/// installs its own policy. Real Chrome resets the violation list per
/// top-level navigation; this matches that behaviour.
pub fn set_csp_policy(policy: std::sync::Arc<net::csp::PolicySet>, origin: Url, enforce: bool) {
    if let Ok(mut q) = violations_lock().lock() {
        q.clear();
    }
    let mut w = active_csp_lock().write().expect("CSP lock poisoned");
    *w = Some(ActiveCsp {
        policy,
        origin,
        enforce,
    });
}

/// Clear any active CSP. Called between top-level navigations so a
/// strict policy from site A doesn't leak into site B. Also drains
/// any queued violations — they belong to the previous document.
pub fn clear_csp_policy() {
    if let Ok(mut q) = violations_lock().lock() {
        q.clear();
    }
    let mut w = active_csp_lock().write().expect("CSP lock poisoned");
    *w = None;
}

/// Returns `Err(blocked_directive)` when the active policy denies the
/// fetch; `Ok(())` when allowed (no policy installed, or matched).
/// On block, pushes a record onto the per-runtime violation queue so
/// JS can later dispatch `securitypolicyviolation` events for each.
pub fn check_csp(
    directive: net::csp::Directive,
    url: &Url,
    nonce: Option<&str>,
    parser_inserted: bool,
) -> Result<(), &'static str> {
    let guard = active_csp_lock().read().expect("CSP lock poisoned");
    let Some(active) = guard.as_ref() else {
        return Ok(());
    };
    if !active.enforce {
        return Ok(());
    }
    let ctx = net::csp::CheckCtx {
        directive,
        url,
        page_origin: &active.origin,
        nonce,
        parser_inserted,
    };
    let decision = active.policy.allows(&ctx);
    if decision.allowed {
        Ok(())
    } else {
        let dir_name = decision.matched_directive.as_str();
        push_csp_violation(CspViolation {
            blocked_uri: url.as_str().to_string(),
            effective_directive: dir_name.to_string(),
            violated_directive: dir_name.to_string(),
            disposition: "enforce".to_string(),
        });
        Err(dir_name)
    }
}

// ---------------------------------------------------------------------
// Violation queue — the gates push, JS drains via `op_drain_csp_violations`
// and dispatches `securitypolicyviolation` events. We keep the queue
// process-global next to ACTIVE_CSP because the gates run from
// non-op call sites (page.rs build_page_with_scripts) where there's no
// OpState handle.
// ---------------------------------------------------------------------

#[derive(Clone, serde::Serialize)]
pub struct CspViolation {
    #[serde(rename = "blockedURI")]
    pub blocked_uri: String,
    #[serde(rename = "effectiveDirective")]
    pub effective_directive: String,
    #[serde(rename = "violatedDirective")]
    pub violated_directive: String,
    pub disposition: String,
}

static CSP_VIOLATIONS: std::sync::OnceLock<std::sync::Mutex<Vec<CspViolation>>> =
    std::sync::OnceLock::new();

fn violations_lock() -> &'static std::sync::Mutex<Vec<CspViolation>> {
    CSP_VIOLATIONS.get_or_init(|| std::sync::Mutex::new(Vec::new()))
}

fn push_csp_violation(v: CspViolation) {
    if let Ok(mut q) = violations_lock().lock() {
        // Cap queue at 256 to avoid unbounded growth on pathological
        // scripts that retry blocked fetches in a loop.
        if q.len() < 256 {
            q.push(v);
        }
    }
}

/// JS-callable drain. Returns the queue contents and clears it.
/// Caller iterates and dispatches one `securitypolicyviolation` event
/// per item on `document` and `window`.
#[op2]
#[serde]
pub fn op_drain_csp_violations() -> Vec<CspViolation> {
    if let Ok(mut q) = violations_lock().lock() {
        std::mem::take(&mut *q)
    } else {
        Vec::new()
    }
}

/// Initialize the shared fetch client from a profile.
/// Call this once during runtime setup.
pub fn init_fetch_client(profile: &stealth::StealthProfile) {
    if let Ok(client) = net::HttpClient::new(profile) {
        let _ = FETCH_CLIENT.set(client);
    }
}

/// Set the shared fetch client to an existing HttpClient.
/// Used by navigate_with_challenges to share cookies between
/// the navigation client and the JS fetch() calls.
pub fn set_fetch_client(client: net::HttpClient) {
    let _ = FETCH_CLIENT.set(client);
}

/// Clone of the shared fetch client, if one has been installed.
/// Used by the worker `importScripts` synchronous fetch path in
/// `worker_ext::op_worker_sync_fetch`.
pub fn fetch_client() -> Option<net::HttpClient> {
    FETCH_CLIENT.get().cloned()
}

#[derive(Serialize)]
pub struct FetchResponse {
    pub status: u16,
    pub status_text: String,
    pub headers: HashMap<String, String>,
    pub body: String,
    pub url: String,
    pub ok: bool,
}

/// Async fetch op. Uses the profile-configured client (with proxy, TLS
/// emulation, cookies) when available, falling back to a default Chrome 130.
///
/// The JS side sends the body as a base64 string in the `body` parameter to
/// preserve binary data (Kasada's challenge solution POST uses
/// `application/octet-stream` with a raw byte payload). The first character
/// of `body` is a marker: 's' for plain UTF-8 string bodies, 'b' for
/// base64-encoded binary bodies. This keeps the op signature stable as a
/// `#[string]` while supporting binary POSTs.
#[op2(async)]
#[serde]
pub async fn op_fetch(
    #[string] url: String,
    #[string] method: String,
    #[serde] headers: HashMap<String, String>,
    #[string] body: String,
) -> Result<FetchResponse, deno_core::error::AnyError> {
    // CSP `connect-src` enforcement — `window.fetch()` and XHR both
    // route through this op. Real Chrome blocks fetches that violate
    // the active policy by returning a 0-status, opaque, network-error
    // response. We mirror that shape so JS code's
    // `try { await fetch(...) } catch (e) { ... }` path fires the same
    // way it would in Chrome.
    if let Ok(parsed) = Url::parse(&url) {
        if let Err(violated) = check_csp(net::csp::Directive::ConnectSrc, &parsed, None, false) {
            eprintln!(
                "[csp] Refused to connect to '{}' because it violates the following Content Security Policy directive: \"{}\".",
                url, violated
            );
            return Ok(FetchResponse {
                status: 0,
                status_text: "".to_string(),
                headers: HashMap::new(),
                body: String::new(),
                url: url.clone(),
                ok: false,
            });
        }
    }

    // Resource blocker — short-circuit ad/tracker requests before TLS+JS.
    // Empty source_url is OK; the JS layer doesn't currently pass the page
    // origin here, but adblock's first-party rules degrade gracefully.
    let request_type = net::blocker::classify_request_type(
        &url,
        headers.get("x-boxide-request-type").map(|s| s.as_str()),
    );
    if net::blocker::should_block(&url, "", request_type) {
        return Ok(FetchResponse {
            status: 200,
            status_text: "OK".to_string(),
            headers: HashMap::new(),
            body: String::new(),
            url: url.clone(),
            ok: true,
        });
    }

    let default_client;
    let client = match FETCH_CLIENT.get() {
        Some(c) => c,
        None => {
            let profile = stealth::chrome_130_linux();
            default_client = net::HttpClient::new(&profile)
                .map_err(|e| deno_core::error::AnyError::msg(e.to_string()))?;
            &default_client
        }
    };

    // Pull JS-provided headers. JS may pass "x-boxide-origin" as a pseudo
    // header carrying the page's origin; strip it here and forward as the
    // origin context so the net layer can compute sec-fetch-site correctly.
    let mut extra_headers: Vec<(String, String)> = Vec::with_capacity(headers.len());
    let mut origin: Option<String> = None;
    for (k, v) in headers.into_iter() {
        let lk = k.to_ascii_lowercase();
        if lk == "x-boxide-origin" {
            origin = Some(v);
            continue;
        }
        extra_headers.push((lk, v));
    }

    // Decode the body marker. Legacy callers that don't set a marker send
    // plain UTF-8 strings; we treat those as 's' by default.
    let body_bytes: Vec<u8> = if let Some(rest) = body.strip_prefix("b:") {
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD
            .decode(rest.as_bytes())
            .unwrap_or_default()
    } else if let Some(rest) = body.strip_prefix("s:") {
        rest.as_bytes().to_vec()
    } else {
        body.as_bytes().to_vec()
    };

    // Use fetch-API-style headers (accept: */*, sec-fetch-dest: empty, no
    // upgrade-insecure-requests) — this is a JS fetch() call, not a navigation.
    // Kasada and similar engines use the nav-vs-fetch header distinction as a
    // strong bot signal.
    let method_upper = method.to_uppercase();

    // Apply a 30-second hard timeout so hanging connections (e.g. Kasada /tl
    // where the server black-holes requests with invalid solutions) don't hold
    // the V8 event loop open indefinitely.
    let fetch_timeout = std::time::Duration::from_secs(30);
    let resp_result = tokio::time::timeout(fetch_timeout, async {
        match method_upper.as_str() {
            "POST" | "PUT" | "PATCH" => {
                client
                    .fetch_post_bytes(&url, &body_bytes, &extra_headers, origin.as_deref())
                    .await
            }
            _ => {
                client
                    .fetch_get(&url, &extra_headers, origin.as_deref())
                    .await
            }
        }
    })
    .await;
    let resp = match resp_result {
        Ok(r) => r,
        Err(_) => {
            return Err(deno_core::error::AnyError::msg(format!(
                "fetch timeout after {}s: {}",
                fetch_timeout.as_secs(),
                url
            )));
        }
    };

    let resp = match resp {
        Ok(r) => r,
        Err(e) => return Err(deno_core::error::AnyError::msg(e.to_string())),
    };

    let ok = resp.ok();
    let body_text = resp.text();

    let final_resp = FetchResponse {
        status: resp.status,
        status_text: resp.status_text.clone(),
        headers: resp.headers.clone(),
        body: body_text,
        url: resp.url.clone(),
        ok,
    };

    // record_resource_timing is sync (uses try_borrow_mut), so it's safe to call here.
    // However, op_fetch is an async op; we need access to OpState.
    // In deno_core 0.311, op2(async) can't easily borrow &mut OpState from its future.
    // Instead, we use the process-global DomState if accessible, or we'll just return it.
    // For now, let's keep it simple: we need to find where the OpState is for this isolate.

    Ok(final_resp)
}

/// Get the cookie string for a URL from the shared HTTP client's cookie jar.
/// Returns "name=value; name2=value2" — the format document.cookie expects.
#[op2(async)]
#[string]
pub async fn op_cookie_get(#[string] url: String) -> String {
    let Some(client) = FETCH_CLIENT.get() else {
        return String::new();
    };
    let Ok(parsed) = Url::parse(&url) else {
        return String::new();
    };
    client.cookies_for_url(&parsed).await.unwrap_or_default()
}

/// Set a cookie via a raw "name=value; path=/; ..." string, scoped to the URL's origin.
#[op2(async)]
pub async fn op_cookie_set(#[string] url: String, #[string] cookie: String) {
    let Some(client) = FETCH_CLIENT.get() else {
        return;
    };
    let Ok(parsed) = Url::parse(&url) else { return };
    client.set_cookie_str(&parsed, &cookie).await;
}

/// Synchronous fetch op. Blocks the V8 thread until the request completes.
/// Used by document.write and appendChild(script) when synchronous execution
/// is required.
#[op2]
#[string]
pub fn op_net_fetch_sync(#[string] url: String, #[string] referer: String) -> String {
    // CSP `script-src-elem` enforcement. Sync-fetch is the path
    // `document.write('<script src=...>')` and dynamic
    // `appendChild(script)` use. Real Chrome enforces CSP on these
    // identically to parser-injected scripts. Without a nonce on the
    // dynamically-inserted script (we don't track them today), under
    // strict-dynamic this fetch will block.
    if let Ok(parsed) = Url::parse(&url) {
        if let Err(violated) = check_csp(net::csp::Directive::ScriptSrcElem, &parsed, None, false) {
            eprintln!(
                "[csp] Refused to load the script '{}' (sync-fetch) — violates: \"{}\".",
                url, violated
            );
            return String::new();
        }
    }

    // Resource blocker — return empty body for ad/tracker URLs without
    // doing any HTTP work. Tracker JS that loads via <script src=…>
    // (gtm.js, gpt.js, doubleclick) is the dominant time sink on
    // news/store sites; blocking these saves 1-3 s per site on average.
    if net::blocker::should_block(
        &url,
        &referer,
        net::blocker::classify_request_type(&url, Some("script")),
    ) {
        return String::new();
    }

    // Per-page chain ceiling — see MAX_SYNC_FETCH_PER_PAGE.
    let n = SYNC_FETCH_COUNT.fetch_add(1, Ordering::Relaxed);
    if n >= MAX_SYNC_FETCH_PER_PAGE {
        eprintln!(
            "[op_net_fetch_sync] CHAIN LIMIT ({}) exceeded — returning empty for {}",
            MAX_SYNC_FETCH_PER_PAGE, url
        );
        return String::new();
    }

    tracing::debug!("[op_net_fetch_sync] fetching {}", url);

    // 1. Get a client instance.
    //
    // NOTE: we deliberately build a FRESH client here rather than reuse
    // FETCH_CLIENT. Reason: the V8 op runs on the main tokio runtime's
    // thread (synchronous from JS's perspective). It then std::thread::spawn
    // a new tokio runtime to do the await. If we used the shared
    // FETCH_CLIENT, its pooled HTTP/2 connections — whose reader/writer
    // tasks live on the MAIN runtime — would deadlock because the main
    // runtime is blocked waiting for this op to return. A fresh client
    // with its own connection pool fully owned by the spawned runtime
    // sidesteps the deadlock. We DO read the profile from FETCH_CLIENT
    // so cookies + stealth settings are consistent.
    let (_profile, client_res) = match FETCH_CLIENT.get() {
        Some(main) => (
            main.profile().clone(),
            net::HttpClient::new_with_shared_state(
                main.profile(),
                main.cookies(),
                main.kasada_sessions(),
                main.akamai_sessions(),
                main.accept_ch_origins(),
                main.dns_cache(),
                main.alt_svc_cache(),
            ),
        ),
        None => {
            let p = stealth::presets::chrome_130_ru();
            (p.clone(), net::HttpClient::new(&p))
        }
    };
    let client = match client_res {
        Ok(c) => c,
        Err(_) => return String::new(),
    };

    // 2. Build browser-native headers for a script fetch
    let mut extra_headers = vec![
        ("referer".to_string(), referer.clone()),
        ("sec-fetch-dest".to_string(), "script".to_string()),
        ("sec-fetch-mode".to_string(), "no-cors".to_string()),
        ("sec-fetch-site".to_string(), "same-origin".to_string()),
    ];
    if let Ok(parsed) = Url::parse(&referer) {
        if let Some(origin) = parsed.origin().ascii_serialization().into() {
            extra_headers.push(("origin".to_string(), origin));
        }
    }

    let url_clone = url.clone();
    let result = std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                eprintln!("[op_net_fetch_sync] runtime build error: {e}");
                return String::new();
            }
        };
        rt.block_on(async move {
            match tokio::time::timeout(
                std::time::Duration::from_secs(30),
                client.get_with_headers(&url_clone, &extra_headers),
            )
            .await
            {
                Ok(Ok(resp)) => {
                    let body = resp.text();
                    if body.is_empty() {
                        eprintln!(
                            "[op_net_fetch_sync] empty body for {} (status={})",
                            url_clone, resp.status
                        );
                    } else if url_clone.ends_with(".js") && body.len() > 10000 {
                        let filename = format!("/tmp/fetched_script_{}.js", body.len());
                        let _ = std::fs::write(&filename, &body);
                        eprintln!("[op_net_fetch_sync] saved script to {}", filename);
                    }
                    body
                }
                Ok(Err(e)) => {
                    eprintln!("[op_net_fetch_sync] FAILED fetch {}: {}", url_clone, e);
                    String::new()
                }
                Err(_) => {
                    eprintln!("[op_net_fetch_sync] TIMEOUT fetching {}", url_clone);
                    String::new()
                }
            }
        })
    })
    .join()
    .unwrap_or_default();

    eprintln!(
        "[op_net_fetch_sync] fetched {} bytes from {}",
        result.len(),
        url
    );
    result
}

/// Synchronous XHR op: makes a network request (GET or POST) synchronously,
/// returning a JSON string `{status, headers, body, url}`.
///
/// Used by the XHR polyfill so that KPSDK's /tl POST (and similar anti-bot
/// challenge POSTs) complete even when V8 is busy with a PoW computation
/// loop that starves the async event loop. Cookies set by the response are
/// written back to the shared FETCH_CLIENT cookie jar.
///
/// Body is marker-prefixed: "s:<utf8>" or "b:<base64>". Empty string = no body.
#[op2]
#[string]
pub fn op_net_xhr_sync(
    #[string] url: String,
    #[string] method: String,
    #[string] headers_json: String,
    #[string] body: String,
    #[string] origin: String,
) -> String {
    // Parse extra headers provided by JS.
    let extra_headers: Vec<(String, String)> =
        serde_json::from_str(&headers_json).unwrap_or_default();

    // Decode the body.
    let body_bytes: Vec<u8> = if let Some(rest) = body.strip_prefix("b:") {
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD
            .decode(rest.as_bytes())
            .unwrap_or_default()
    } else if let Some(rest) = body.strip_prefix("s:") {
        rest.as_bytes().to_vec()
    } else if body.is_empty() {
        Vec::new()
    } else {
        body.as_bytes().to_vec()
    };

    let url_clone = url.clone();
    let method_upper = method.to_uppercase();
    let origin_str = if origin.is_empty() {
        None
    } else {
        Some(origin)
    };

    let result = std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(_) => return "{}".to_string(),
        };
        rt.block_on(async move {
            // Fresh client for sync execution (avoids H2 deadlock on FETCH_CLIENT).
            // Shares all state (cookies, tokens, cache) with the main client.
            let client = match FETCH_CLIENT.get() {
                Some(main) => {
                    net::HttpClient::new_with_shared_state(
                        main.profile(),
                        main.cookies(),
                        main.kasada_sessions(),
                        main.akamai_sessions(),
                        main.accept_ch_origins(),
                        main.dns_cache(),
                        main.alt_svc_cache(),
                    ).unwrap_or_else(|_| net::HttpClient::new(main.profile()).unwrap())
                }
                None => {
                    let p = stealth::presets::chrome_130_ru();
                    net::HttpClient::new(&p).unwrap()
                }
            };

            let resp_result = match method_upper.as_str() {
                "GET" | "HEAD" => {
                    client.get_with_headers(&url_clone, &extra_headers).await
                }
                _ => {
                    let hdrs = net::headers::chrome_headers_fetch(
                        &client.profile(),
                        &url_clone,
                        origin_str.as_deref(),
                    );
                    let mut merged = hdrs;
                    for h in &extra_headers { merged.push(h.clone()); }
                    client.post_bytes_with_exact_headers(&url_clone, &body_bytes, &merged).await
                }
            };

            match tokio::time::timeout(
                std::time::Duration::from_secs(15),
                async { resp_result },
            ).await {
                Ok(Ok(resp)) => {
                    // Write response cookies back to FETCH_CLIENT.
                    if let Some(main) = FETCH_CLIENT.get() {
                        if let Ok(parsed) = url::Url::parse(&url_clone) {
                            for ck in &resp.set_cookies {
                                main.set_cookie_str(&parsed, ck).await;
                            }
                        }
                    }
                    let status = resp.status;
                    let resp_url = resp.url.clone();
                    let body_text = resp.text();
                    // Serialize headers as [[k,v],...] for JS.
                    let headers_arr: Vec<[String; 2]> = resp.headers
                        .into_iter()
                        .map(|(k, v)| [k, v])
                        .collect();
                    serde_json::json!({
                        "status": status,
                        "url": resp_url,
                        "headers": headers_arr,
                        "body": body_text,
                    }).to_string()
                }
                Ok(Err(e)) => {
                    eprintln!("[op_net_xhr_sync] error {}: {e}", url_clone);
                    serde_json::json!({"status": 0, "url": url_clone, "headers": [], "body": "", "error": e.to_string()}).to_string()
                }
                Err(_) => {
                    eprintln!("[op_net_xhr_sync] timeout {}", url_clone);
                    serde_json::json!({"status": 0, "url": url_clone, "headers": [], "body": "", "error": "timeout"}).to_string()
                }
            }
        })
    })
    .join()
    .unwrap_or_else(|_| "{}".to_string());

    result
}

deno_core::extension!(
    fetch_extension,
    ops = [
        op_fetch,
        op_cookie_get,
        op_cookie_set,
        op_net_fetch_sync,
        op_net_xhr_sync,
        op_drain_csp_violations
    ],
);
