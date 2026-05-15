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

async fn capture_via_http_client(url: &str, label: &str) {
    let profile = stealth::presets::chrome_130_macos();
    let client = match net::HttpClient::new(&profile) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[{label}] HttpClient init failed: {e}");
            return;
        }
    };
    let resp = match client.get(url).await {
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
