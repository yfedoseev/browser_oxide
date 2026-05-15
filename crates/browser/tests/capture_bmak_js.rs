//! Capture bmak.js from Akamai-protected hosts via our TLS-impersonating
//! HttpClient. Curl from datacenter IPs is hard-blocked at the edge;
//! our rquest-based engine with full Chrome 148 TLS fingerprint may get
//! through where curl doesn't.
//!
//! Captured files are written to /tmp/bmak_<host>.js for later
//! extraction via glizzy's helper:
//!
//!   cat /tmp/bmak_bestbuy.js | node \
//!     /tmp/akamai-v3-sensor-data-helper/src/extract_hash/index.js
//!
//! Each successful capture yields a 5-7 digit fileHash for the
//! `known_file_hash` registry in `crates/akamai/src/lib.rs`.
//!
//! Run: `cargo test -p browser --test capture_bmak_js -- --ignored \
//!       --nocapture --test-threads=1`

use stealth;

#[tokio::test]
#[ignore = "network: capture bmak.js from bestbuy"]
async fn capture_bestbuy_bmak() {
    capture_via_http_client("https://www.bestbuy.com/", "bestbuy").await;
}

#[tokio::test]
#[ignore = "network: capture bmak.js from homedepot"]
async fn capture_homedepot_bmak() {
    capture_via_http_client("https://www.homedepot.com/", "homedepot").await;
}

#[tokio::test]
#[ignore = "network: capture bmak.js from macys"]
async fn capture_macys_bmak() {
    capture_via_http_client("https://www.macys.com/", "macys").await;
}

#[tokio::test]
#[ignore = "network: capture bmak.js from hotels"]
async fn capture_hotels_bmak() {
    capture_via_http_client("https://www.hotels.com/", "hotels").await;
}

#[tokio::test]
#[ignore = "network: capture bmak.js from h-m"]
async fn capture_hm_bmak() {
    capture_via_http_client("https://www2.hm.com/", "hm").await;
}

/// homedepot returns a sec-cpt challenge page (~2.6 KB) on direct
/// GET / which contains TWO script tags: a challenge script with
/// `?v=<uuid>&t=<token>` query and a clean bmak.js. The default
/// capture_via_http_client picks the FIRST deep-obfuscated path,
/// which is the challenge script. This variant fetches the second
/// one (no query string) directly — that's the real bmak.js.
#[tokio::test]
#[ignore = "network: capture bmak.js from homedepot challenge page"]
async fn capture_homedepot_bmak_from_challenge() {
    let profile = stealth::presets::chrome_130_macos();
    let client = match net::HttpClient::new(&profile) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("HttpClient init failed: {e}");
            return;
        }
    };
    let resp = client.get("https://www.homedepot.com/").await.unwrap();
    let body = String::from_utf8_lossy(&resp.body).to_string();
    // Find ALL <script src="..."> paths.
    let mut paths: Vec<String> = Vec::new();
    for frag in body.split("<script") {
        let Some(attr_start) = frag.find(" src=") else { continue };
        let after = &frag[attr_start + " src=".len()..];
        let Some(q) = after.chars().next() else { continue };
        if q != '"' && q != '\'' { continue }
        let Some(close) = after[1..].find(q) else { continue };
        let path = &after[1..1 + close];
        if !path.starts_with('/') && !path.starts_with("https://www.homedepot.com/") { continue }
        let path_only = path.trim_start_matches("https://www.homedepot.com");
        if path_only.starts_with("/akam/") { continue }
        // We want paths WITHOUT `?` query — those are bmak.js, not challenge scripts.
        if path_only.contains('?') { continue }
        let segments: Vec<&str> = path_only.split('/').filter(|s| !s.is_empty()).collect();
        if segments.len() < 4 { continue }
        if !segments.iter().all(|s| s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')) {
            continue;
        }
        paths.push(path_only.to_string());
    }
    eprintln!("[homedepot] found {} candidate bmak paths: {paths:?}", paths.len());
    let Some(bmak_path) = paths.first() else {
        eprintln!("[homedepot] no candidate bmak.js path");
        return;
    };
    let bmak_url = format!("https://www.homedepot.com{bmak_path}");
    eprintln!("[homedepot] fetching bmak.js: {bmak_url}");
    let resp = client.get(&bmak_url).await.unwrap();
    eprintln!("[homedepot] status={} length={}", resp.status, resp.body.len());
    std::fs::write("/tmp/bmak_homedepot.js", &resp.body).expect("save");
    eprintln!("[homedepot] saved /tmp/bmak_homedepot.js");
}

async fn capture_via_http_client(url: &str, label: &str) {
    let profile = stealth::presets::chrome_130_macos();
    let client = match net::HttpClient::new(&profile) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[{label}] HttpClient init failed: {e}");
            return;
        }
    };
    // Use get_follow so 302 redirects (hotels.com, www2.hm.com)
    // resolve to the real homepage where bmak.js lives.
    let resp = match client.get_follow(url, 5).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[{label}] GET {url} failed: {e}");
            return;
        }
    };
    eprintln!("[{label}] GET {url} → status {}", resp.status);
    let body = String::from_utf8_lossy(&resp.body).to_string();
    eprintln!("[{label}] body length: {}", body.len());

    // Extract the deep-obfuscated <script src="..."> path from HTML.
    // Per parse_tenant_from_html in akamai/src/lib.rs: ≥4-segment path,
    // not starting with /akam/, [A-Za-z0-9_-] chars in segments.
    let deep_path = body.split("<script").find_map(|frag| {
        let attr_start = frag.find(" src=")?;
        let after = &frag[attr_start + " src=".len()..];
        let q = after.chars().next()?;
        if q != '"' && q != '\'' {
            return None;
        }
        let close = after[1..].find(q)?;
        let path = &after[1..1 + close];
        if !path.starts_with('/') {
            return None;
        }
        if path.starts_with("/akam/") {
            return None;
        }
        let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        if segments.len() < 4 {
            return None;
        }
        if !segments.iter().all(|s| s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')) {
            return None;
        }
        Some(path.to_string())
    });

    let bmak_path = match deep_path {
        Some(p) => p,
        None => {
            eprintln!("[{label}] no deep-obfuscated script found in HTML — host may not be Akamai-protected or is on Bot-or-Not");
            // Save the body for inspection
            let path = format!("/tmp/bmak_capture_{label}_body.html");
            std::fs::write(&path, &body).ok();
            eprintln!("[{label}] body saved to {path}");
            return;
        }
    };

    let bmak_url = format!(
        "{}://{}{}",
        if url.starts_with("https") { "https" } else { "http" },
        url::Url::parse(url).unwrap().host_str().unwrap_or("unknown"),
        bmak_path
    );
    eprintln!("[{label}] fetching bmak.js: {bmak_url}");

    let resp = match client.get(&bmak_url).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[{label}] bmak.js fetch failed: {e}");
            return;
        }
    };
    eprintln!("[{label}] bmak.js status: {}, body length: {}", resp.status, resp.body.len());
    let path = format!("/tmp/bmak_{label}.js");
    std::fs::write(&path, &resp.body).expect("save bmak.js");
    eprintln!("[{label}] saved to {path}");
    eprintln!("[{label}] next: cat {path} | node /tmp/akamai-v3-sensor-data-helper/src/extract_hash/index.js");
}
