//! Validation experiment for GAPS §Q7: is the adidas Akamai gate a fingerprint
//! gate or a timing/behavior gate?
//!
//! Run:
//!   cargo test -p browser --test adidas_cookie_replay -- \
//!     --ignored --test-threads=1 --nocapture
//!
//! Reads /tmp/adidas-cookies.txt (Netscape cookie file captured from a real
//! Playwright Chrome 130 session via CDP Network.getAllCookies) and replays a
//! GET through our Chrome-131 TLS-impersonating HttpClient.
//!
//! Interpretation:
//!   - status 200 + >500 KB body + product markers ("Sneakers", "adidas.com")
//!     → cookies are portable across Chrome-shaped TLS. The gate is per-session
//!       fingerprint state that the sensor posts produce. Tier 1 (real canvas,
//!       fonts, audio, workers) is on the critical path.
//!   - status 403 or interstitial ("Pardon Our Interruption", "Akamai")
//!     → cookies are bound to something we can't spoof in curl-alike fetches
//!       (IP+UA+TLS hash, or cookies require continuous sensor refresh).
//!       Tier 1 alone will not fix adidas; pivot research.

use std::fs;

async fn cookie_header_from_file(path: &str) -> String {
    let text = fs::read_to_string(path).expect("adidas cookies file");
    let mut parts: Vec<String> = Vec::new();
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        // Netscape: domain \t include_subdomains \t path \t secure \t expiry \t name \t value
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 7 {
            continue;
        }
        let name = cols[5];
        let value = cols[6];
        if name.is_empty() {
            continue;
        }
        parts.push(format!("{name}={value}"));
    }
    parts.join("; ")
}

#[tokio::test]
#[ignore]
async fn adidas_cookie_replay() {
    let cookie = cookie_header_from_file("/tmp/adidas-cookies.txt").await;
    println!("Cookie header length: {}", cookie.len());
    println!("First 200 bytes: {}", &cookie[..cookie.len().min(200)]);

    let profile = stealth::chrome_130_macos();
    let client = net::HttpClient::new(&profile).expect("HttpClient");

    // No cookies — baseline.
    let baseline = client.get("https://www.adidas.com/us").await;
    match &baseline {
        Ok(r) => {
            println!("[baseline] status={} body={} bytes", r.status, r.body.len());
        }
        Err(e) => println!("[baseline] ERROR: {e}"),
    }

    // With captured cookies.
    let extras = vec![("cookie".to_string(), cookie)];
    let with_cookies = client
        .get_with_headers("https://www.adidas.com/us", &extras)
        .await;
    match &with_cookies {
        Ok(r) => {
            let body = r.text();
            println!(
                "[w/cookies] status={} body={} bytes",
                r.status,
                r.body.len()
            );
            let snippet = &body[..body.len().min(400)];
            println!("[w/cookies] snippet: {snippet}");

            let real_markers = [
                "Sneakers and Activewear",
                "adidas-us",
                "product-card",
                "pdp_product_title",
            ];
            let block_markers = [
                "Pardon Our Interruption",
                "Reference #",
                "Access Denied",
                "Bad Gateway",
            ];
            let has_real = real_markers.iter().any(|m| body.contains(m));
            let has_block = block_markers.iter().any(|m| body.contains(m));
            println!("[w/cookies] real={has_real} blocked={has_block}");

            if has_real && r.status == 200 {
                println!("VERDICT: cookies are portable → gate is fingerprint/sensor state.");
                println!("         Tier 1 (skia-safe, fonts, audio, workers) is the right path.");
            } else {
                println!(
                    "VERDICT: cookies NOT portable → gate is IP/TLS binding or live sensor refresh."
                );
                println!("         Tier 1 alone will not unlock adidas. Pivot required.");
            }
        }
        Err(e) => println!("[w/cookies] ERROR: {e}"),
    }
}
