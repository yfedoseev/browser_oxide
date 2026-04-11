//! Live TLS / HTTP2 fingerprint probe against public endpoints that
//! echo back the JA3/JA4/Akamai-H2 hash. Use this to diagnose which
//! wire-level signal is outing us as a bot.
//!
//! Run:
//!   cargo test -p browser --test tls_fingerprint_probe \
//!     -- --ignored --test-threads=1 --nocapture

use net::HttpClient;
use stealth::chrome_130_macos;

#[tokio::test]
#[ignore]
async fn probe_tls_peet_ws() {
    let client = HttpClient::new(&chrome_130_macos()).unwrap();
    let resp = client
        .get("https://tls.peet.ws/api/all")
        .await
        .expect("fetch");
    let body = resp.text();
    eprintln!("status={} size={}", resp.status, body.len());
    // Extract only the hash fields — much less noise than dumping JSON.
    for field in &[
        "\"ja3\"",
        "\"ja3_hash\"",
        "\"ja4\"",
        "\"peetprint_hash\"",
        "\"akamai_fingerprint\"",
        "\"akamai_fingerprint_hash\"",
    ] {
        if let Some(idx) = body.find(field) {
            let end = (idx + 300).min(body.len());
            eprintln!("  {}", &body[idx..end]);
        }
    }
    // Print the whole sent_frames block — that's where HPACK / header
    // ordering tells us whether our HEADERS frame matches Chrome.
    if let Some(idx) = body.find("\"sent_frames\"") {
        // Find matching bracket to bound the block.
        let end = body[idx..]
            .find("\"tcp_ip_fingerprint\"")
            .map(|e| idx + e)
            .unwrap_or(body.len().min(idx + 4000));
        eprintln!("\n=== sent_frames ===\n{}", &body[idx..end]);
    }
}

#[tokio::test]
#[ignore]
async fn probe_example_com_sanity() {
    let client = HttpClient::new(&chrome_130_macos()).unwrap();
    let resp = client.get("https://example.com").await.expect("fetch");
    let body = resp.text();
    eprintln!("status={} size={}", resp.status, body.len());
    eprintln!(
        "\n=== example.com first 500 chars ===\n{}",
        body.chars().take(500).collect::<String>()
    );
}

#[tokio::test]
#[ignore]
async fn probe_httpbin_headers() {
    let client = HttpClient::new(&chrome_130_macos()).unwrap();
    let resp = client
        .get("https://httpbin.org/headers")
        .await
        .expect("fetch");
    let body = resp.text();
    eprintln!("status={} size={}", resp.status, body.len());
    eprintln!("\n=== httpbin.org/headers response ===\n{body}");
}
