//! JA4 capture — fetches tls.peet.ws/api/all with a given profile and prints
//! the JA3/JA4/peetprint TLS fingerprints + the Akamai HTTP/2 fingerprint, so a
//! Firefox profile's wire class can be verified against the target JA4.
//!
//!   cargo run --release -p browser --example ja4_capture -- [profile]

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let profile_name = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "firefox_135_macos".to_string());
    let profile = match profile_name.as_str() {
        "chrome_148_macos" => browser_oxide::stealth::presets::chrome_148_macos(),
        "chrome_148_windows" => browser_oxide::stealth::presets::chrome_148_windows(),
        "firefox_135_macos" => browser_oxide::stealth::presets::firefox_135_macos(),
        "iphone_15_pro_safari_18" => browser_oxide::stealth::presets::iphone_15_pro_safari_18(),
        "pixel_9_pro_chrome_148" => browser_oxide::stealth::presets::pixel_9_pro_chrome_148(),
        other => panic!("unknown profile {other}"),
    };
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async move {
            let client = browser_oxide::net::HttpClient::shared(&profile).expect("client");
            let resp = client
                .get_follow("https://tls.peet.ws/api/all", 10)
                .await
                .expect("fetch tls.peet.ws");
            let body = resp.text();
            let v: deno_core::serde_json::Value =
                deno_core::serde_json::from_str(&body).expect("parse json");
            println!("== ja4_capture profile={profile_name} ==");
            let tls = &v["tls"];
            println!("ja3        = {}", tls["ja3"].as_str().unwrap_or("?"));
            println!("ja3_hash   = {}", tls["ja3_hash"].as_str().unwrap_or("?"));
            println!("ja4        = {}", tls["ja4"].as_str().unwrap_or("?"));
            println!("peetprint  = {}", tls["peetprint"].as_str().unwrap_or("?"));
            println!(
                "akamai_h2  = {}",
                v["http2"]["akamai_fingerprint"].as_str().unwrap_or("?")
            );
            println!("http_ver   = {}", v["http_version"].as_str().unwrap_or("?"));
            // ciphers + extensions count sanity
            if let Some(ciphers) = tls["ciphers"].as_array() {
                println!("n_ciphers  = {}", ciphers.len());
            }
            if let Some(exts) = tls["extensions"].as_array() {
                println!("n_exts     = {}", exts.len());
            }
        })
        .await;
}
