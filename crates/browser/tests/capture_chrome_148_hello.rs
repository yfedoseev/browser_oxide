//! W4.4 — Capture our engine's TLS ClientHello shape via the public
//! `tls.peet.ws/api/all` analyzer, for byte-perfect comparison against
//! real Chrome 148 from the same machine.
//!
//! tls.peet.ws (lwthiker/tls.peet.ws) echoes back a structured JSON
//! description of every TLS+H2 fingerprint signal it observed: JA3 +
//! JA3N + JA4 hashes, cipher suite list, extension order, signature
//! algorithms, supported versions, supported groups, ALPN, HTTP/2
//! pseudo-header order, SETTINGS frames, WINDOW_UPDATE deltas, plus
//! HEADERS-frame order via Akamai's `akamai_fingerprint`.
//!
//! Procedure to use this diagnostic for parity work:
//!
//! 1. Run this test against each profile. Capture output to a file:
//!    `cargo test -p browser --test capture_chrome_148_hello -- \
//!       --ignored --nocapture --test-threads=1 > /tmp/oxide_tls.json`
//!
//! 2. From the same machine, drive real Chrome 148 (headed, via
//!    `google-chrome --user-data-dir=/tmp/cdrun https://tls.peet.ws/api/all`
//!    or via Playwright MCP). Save the response as
//!    `/tmp/real_chrome_tls.json`.
//!
//! 3. `diff <(jq -S . /tmp/oxide_tls.json) <(jq -S . /tmp/real_chrome_tls.json)`.
//!    Any non-empty diff is an engine fingerprint gap.
//!
//! Test is `#[ignore]` because it requires internet access and is
//! intended for manual diagnostic runs, not CI.
//!
//! Per PLAN.md §3 W4.4.

#[tokio::test]
#[ignore = "network: capture our TLS ClientHello via tls.peet.ws/api/all"]
async fn capture_chrome_148_hello_desktop() {
    let profile = stealth::presets::chrome_148_macos();
    capture_tls_fingerprint("chrome_148_macos", &profile).await;
}

#[tokio::test]
#[ignore = "network: capture Pixel Android TLS via tls.peet.ws/api/all"]
async fn capture_chrome_148_hello_pixel() {
    let profile = stealth::presets::pixel_9_pro_chrome_148();
    capture_tls_fingerprint("pixel_9_pro_chrome_148", &profile).await;
}

#[tokio::test]
#[ignore = "network: capture Safari iOS TLS via tls.peet.ws/api/all"]
async fn capture_chrome_148_hello_iphone() {
    let profile = stealth::presets::iphone_15_pro_safari_18();
    capture_tls_fingerprint("iphone_15_pro_safari_18", &profile).await;
}

#[tokio::test]
#[ignore = "network: capture Firefox TLS via tls.peet.ws/api/all"]
async fn capture_chrome_148_hello_firefox() {
    let profile = stealth::presets::firefox_135_macos();
    capture_tls_fingerprint("firefox_135_macos", &profile).await;
}

async fn capture_tls_fingerprint(label: &str, profile: &stealth::StealthProfile) {
    let client = match net::HttpClient::new(profile) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[{label}] HttpClient init failed: {e}");
            return;
        }
    };
    let url = "https://tls.peet.ws/api/all";
    let result = client.get(url).await;
    match result {
        Ok(resp) => {
            let body = String::from_utf8_lossy(&resp.body);
            // Stringify minimal context + body. The fingerprint analyzer
            // returns JSON; print as-is for downstream `jq` comparison.
            println!("--- TLS-FP-CAPTURE BEGIN [{label}] ---");
            println!("status: {}", resp.status);
            println!("{body}");
            println!("--- TLS-FP-CAPTURE END [{label}] ---");
        }
        Err(e) => {
            eprintln!("[{label}] tls.peet.ws fetch failed: {e}");
        }
    }
}
