use deno_core::op2;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::OnceLock;
use url::Url;

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

    // Convert JS headers to ordered pairs for the net layer.
    let extra_headers: Vec<(String, String)> = headers
        .into_iter()
        .map(|(k, v)| (k.to_ascii_lowercase(), v))
        .collect();

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

    let method_upper = method.to_uppercase();
    let resp = match method_upper.as_str() {
        "POST" | "PUT" | "PATCH" => {
            client
                .post_bytes_with_headers(&url, &body_bytes, &extra_headers)
                .await
        }
        _ => client.get_with_headers(&url, &extra_headers).await,
    };

    let resp = match resp {
        Ok(r) => r,
        Err(e) => return Err(deno_core::error::AnyError::msg(e.to_string())),
    };

    let ok = resp.ok();

    Ok(FetchResponse {
        status: resp.status,
        status_text: resp.status_text.clone(),
        headers: resp.headers.clone(),
        body: resp.text(),
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
pub fn op_net_fetch_sync(#[string] url: String) -> String {
    eprintln!("[op_net_fetch_sync] fetching {}", url);
    
    // 1. Get a client instance
    let client = if let Some(c) = FETCH_CLIENT.get() {
        c.clone()
    } else {
        // Fallback: create a temporary client with default profile
        let profile = stealth::presets::chrome_130_ru();
        match net::HttpClient::new(&profile) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[op_net_fetch_sync] FAILED to create client: {}", e);
                return String::new();
            }
        }
    };

    // Use a temporary thread and runtime to perform the blocking fetch.
    // This is safe because it's only used for synchronous script fetching
    // which MUST block the V8 thread anyway.
    let url_clone = url.clone();
    let result = std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async move {
            client.get(&url_clone).await.map(|r| r.text()).unwrap_or_else(|e| {
                eprintln!("[op_net_fetch_sync] FAILED fetch {}: {}", url_clone, e);
                String::new()
            })
        })
    }).join().unwrap_or_default();

    eprintln!("[op_net_fetch_sync] fetched {} bytes from {}", result.len(), url);
    if url.contains("qauth") {
        let _ = std::fs::write("oxide_dump/qauth.js", &result);
    }
    result
}

deno_core::extension!(
    fetch_extension,
    ops = [op_fetch, op_cookie_get, op_cookie_set, op_net_fetch_sync],
);
