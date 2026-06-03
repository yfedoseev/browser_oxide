use net::HttpClient;
use stealth::presets;

#[tokio::test]
#[ignore] // Requires internet
async fn test_tls_fingerprint_peet() {
    let profile = presets::chrome_148_ru();
    let client = HttpClient::new(&profile).unwrap();

    // tls.peet.ws returns JSON with fingerprint details
    let resp = client.get("https://tls.peet.ws/api/all").await.unwrap();
    let json: serde_json::Value = serde_json::from_str(&resp.text()).unwrap();

    println!("TLS Fingerprint Result: {:#?}", json);

    // Check JA4
    if let Some(ja4) = json["ja4"].as_str() {
        println!("JA4: {}", ja4);
        // Expect something like t13d1516h2_...
        assert!(ja4.starts_with("t13d"));
    }
}

/// Capture the live JA4 / JA4_o (ordered) / JA4H / peetprint for the gated
/// profiles so we can field-diff against authoritative references:
///   * iOS-18 Safari  — lexiforest safari_18.0_iOS (ciphers verified byte-exact
///     in-repo; this confirms the on-the-wire ORDERED variant + padding/GREASE).
///   * Firefox 135    — for the firefox-profile wire-class work (#27 / cluster 02).
///   * Chrome 148     — known-good control (huge corpus).
///
/// Run: `cargo test -p net --test tls_fingerprint capture_profiles_ja4
///       -- --ignored --nocapture --test-threads=1`
#[tokio::test]
#[ignore] // Requires internet; IP-safe (tls.peet.ws is not an anti-bot vendor).
async fn capture_profiles_ja4() {
    let profiles: &[(&str, stealth::StealthProfile)] = &[
        (
            "iphone_15_pro_safari_18",
            presets::iphone_15_pro_safari_18(),
        ),
        ("firefox_135_macos", presets::firefox_135_macos()),
        ("chrome_148_macos", presets::chrome_148_macos()),
    ];
    for (name, profile) in profiles {
        let client = match HttpClient::new(profile) {
            Ok(c) => c,
            Err(e) => {
                println!("[{name}] client build FAILED: {e}");
                continue;
            }
        };
        match client.get("https://tls.peet.ws/api/all").await {
            Ok(resp) => {
                let body = resp.text();
                match serde_json::from_str::<serde_json::Value>(&body) {
                    Ok(j) => {
                        let tls = &j["tls"];
                        println!("==================== {name} ====================");
                        println!("  ja4    = {}", tls["ja4"].as_str().unwrap_or("?"));
                        println!("  ja4_r  = {}", tls["ja4_r"].as_str().unwrap_or("?"));
                        // JA3 for cross-check against lexiforest JA3 hash.
                        println!("  ja3    = {}", tls["ja3"].as_str().unwrap_or("?"));
                        println!("  ja3_hash = {}", tls["ja3_hash"].as_str().unwrap_or("?"));
                        println!("  peetprint = {}", tls["peetprint"].as_str().unwrap_or("?"));
                        println!(
                            "  ja4h   = {}",
                            j["http2"]["akamai_fingerprint"]
                                .as_str()
                                .or(j["ja4h"].as_str())
                                .unwrap_or("?")
                        );
                    }
                    Err(_) => println!(
                        "[{name}] non-JSON body ({} bytes): {}",
                        body.len(),
                        &body[..body.len().min(200)]
                    ),
                }
            }
            Err(e) => println!("[{name}] request FAILED: {e}"),
        }
    }
}
