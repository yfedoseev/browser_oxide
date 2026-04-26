use net::HttpClient;
use stealth::presets;

#[tokio::test]
#[ignore] // Requires internet
async fn test_tls_fingerprint_peet() {
    let profile = presets::chrome_130_ru();
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
