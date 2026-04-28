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

    Ok(FetchResponse {
        status: resp.status,
        status_text: resp.status_text.clone(),
        headers: resp.headers.clone(),
        body: body_text,
        url: resp.url.clone(),
        ok,
    })
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
    let profile = FETCH_CLIENT
        .get()
        .map(|c| c.profile().clone())
        .unwrap_or_else(stealth::presets::chrome_130_ru);
    let client = match net::HttpClient::new(&profile) {
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
                std::time::Duration::from_secs(5),
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
    let profile = FETCH_CLIENT
        .get()
        .map(|c| c.profile().clone())
        .unwrap_or_else(stealth::presets::chrome_130_ru);

    // Parse extra headers provided by JS.
    let extra_headers: Vec<(String, String)> = serde_json::from_str(&headers_json)
        .unwrap_or_default();

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
    let origin_str = if origin.is_empty() { None } else { Some(origin) };

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
            let client = match net::HttpClient::new(&profile) {
                Ok(c) => c,
                Err(_) => return "{}".to_string(),
            };
            // Transfer cookies from FETCH_CLIENT into this fresh client.
            if let Some(main) = FETCH_CLIENT.get() {
                if let Ok(parsed) = url::Url::parse(&url_clone) {
                    if let Some(ck) = main.cookies_for_url(&parsed).await {
                        if !ck.is_empty() {
                            let _ = client.set_cookie_str(&parsed, &ck).await;
                        }
                    }
                }
            }

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
                    client.post_bytes_exact_headers(&url_clone, &body_bytes, &merged).await
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
    ops = [op_fetch, op_cookie_get, op_cookie_set, op_net_fetch_sync, op_net_xhr_sync],
);
