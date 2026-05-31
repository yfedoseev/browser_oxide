//! Raw HTTP fetch probe — dumps status, final URL, key headers, body length
//! for a single URL through browser_oxide's net stack (BoringSSL TLS + H2).
//!
//!   cargo run --release -p browser --example fetch_probe -- <url> [profile]

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let mut args = std::env::args().skip(1);
    let url = args.next().expect("usage: fetch_probe <url> [profile]");
    let profile_name = args
        .next()
        .unwrap_or_else(|| "chrome_148_macos".to_string());
    let profile = match profile_name.as_str() {
        "chrome_148_macos" => stealth::presets::chrome_148_macos(),
        "chrome_148_windows" => stealth::presets::chrome_148_windows(),
        "firefox_135_macos" => stealth::presets::firefox_135_macos(),
        "iphone_15_pro_safari_18" => stealth::presets::iphone_15_pro_safari_18(),
        "pixel_9_pro_chrome_148" => stealth::presets::pixel_9_pro_chrome_148(),
        other => panic!("unknown profile {other}"),
    };

    let local = tokio::task::LocalSet::new();
    local
        .run_until(async move {
            let client = net::HttpClient::shared(&profile).expect("client");
            println!("== fetch_probe {url} (profile={profile_name}) ==");

            // 1) Plain GET (no redirect follow) — see the first-hop status.
            match client.get(&url).await {
                Ok(r) => {
                    println!(
                        "[get]        status={} final_url={} body_len={}",
                        r.status,
                        r.url,
                        r.text().len()
                    );
                }
                Err(e) => println!("[get]        ERROR: {e}"),
            }

            // 2) get_follow (redirect chain) — what the navigate path actually uses.
            match client.get_follow(&url, 10).await {
                Ok(r) => {
                    let body = r.text();
                    println!(
                        "[get_follow] status={} final_url={} body_len={}",
                        r.status,
                        r.url,
                        body.len()
                    );
                    println!("[headers]");
                    for (k, v) in r.headers.iter() {
                        let kl = k.to_ascii_lowercase();
                        if matches!(
                            kl.as_str(),
                            "location"
                                | "content-type"
                                | "content-length"
                                | "content-encoding"
                                | "content-security-policy"
                                | "set-cookie"
                                | "server"
                                | "cf-mitigated"
                                | "x-frame-options"
                                | "retry-after"
                                | "transfer-encoding"
                                | "vary"
                        ) {
                            let vs: String = v.chars().take(180).collect();
                            println!("    {kl}: {vs}");
                        }
                    }
                    let head: String = body.chars().take(400).collect();
                    println!("[body head 400] {head:?}");
                }
                Err(e) => println!("[get_follow] ERROR: {e}"),
            }
        })
        .await;
}
