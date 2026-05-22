//! Debug test for blocked sites — dumps full response details.
//! Run: cargo test -p browser --test debug_blocked -- --ignored --test-threads=1 --nocapture

async fn debug_probe(url: &str, profile: stealth::StealthProfile) {
    let client = net::HttpClient::new(&profile).unwrap();
    println!("\n======================================================================");
    println!(" PROBING: {url}");
    println!(
        " Profile: {} / {} / tz={}",
        profile.language, profile.os_name, profile.timezone
    );
    println!("======================================================================");

    match client.get(url).await {
        Ok(resp) => {
            println!("  Status: {}", resp.status);
            println!("  URL after redirects: {}", resp.url);
            println!("  Headers ({}):", resp.headers.len());
            let mut sorted: Vec<_> = resp.headers.iter().collect();
            sorted.sort_by_key(|(k, _)| k.clone());
            for (k, v) in &sorted {
                println!("    {k}: {}", &v[..v.len().min(120)]);
            }
            let body = resp.text();
            println!("  Body length: {}", body.len());
            println!("  Body preview (first 2000 chars):");
            println!(
                "    {}",
                &body[..body.len().min(2000)].replace('\n', "\n    ")
            );

            // Detection signals
            if body.contains("captcha") || body.contains("CAPTCHA") {
                println!("  !! CAPTCHA detected");
            }
            if body.contains("challenge") {
                println!("  !! Challenge page detected");
            }
            if body.contains("Just a moment") {
                println!("  !! Cloudflare interstitial");
            }
            if body.contains("datadome") || body.contains("DataDome") {
                println!("  !! DataDome detected");
            }
            if body.contains("_abck") {
                println!("  !! Akamai sensor detected");
            }
            if body.contains("px-captcha") || body.contains("perimeterx") {
                println!("  !! PerimeterX detected");
            }
            if body.contains("blocked") || body.contains("denied") {
                println!("  !! Access blocked/denied");
            }
            if body.contains("robot") || body.contains("bot") {
                println!("  !! Bot detection keyword");
            }
        }
        Err(e) => {
            println!("  ERROR: {e}");
        }
    }
}

#[tokio::test]
#[ignore]
async fn debug_tripadvisor() {
    debug_probe("https://www.tripadvisor.com", stealth::chrome_148_windows()).await;
}

#[tokio::test]
#[ignore]
async fn debug_airbnb() {
    debug_probe("https://www.airbnb.com", stealth::chrome_148_windows()).await;
}

#[tokio::test]
#[ignore]
async fn debug_amazon() {
    debug_probe("https://www.amazon.com", stealth::chrome_148_windows()).await;
}

#[tokio::test]
#[ignore]
async fn debug_ozon() {
    debug_probe("https://www.ozon.ru", stealth::presets::chrome_148_ru()).await;
}

#[tokio::test]
#[ignore]
async fn debug_ozon_rr1() {
    debug_probe(
        "https://www.ozon.ru/?__rr=1",
        stealth::presets::chrome_148_ru(),
    )
    .await;
}

#[tokio::test]
#[ignore]
async fn debug_yandex() {
    debug_probe("https://ya.ru", stealth::presets::chrome_148_ru()).await;
}

#[tokio::test]
#[ignore]
async fn debug_dns_shop() {
    debug_probe("https://www.dns-shop.ru", stealth::presets::chrome_148_ru()).await;
}

// Also test with different profiles to see if locale matters
#[tokio::test]
#[ignore]
async fn debug_amazon_linux() {
    debug_probe("https://www.amazon.com", stealth::chrome_148_linux()).await;
}

#[tokio::test]
#[ignore]
async fn debug_tripadvisor_macos() {
    debug_probe("https://www.tripadvisor.com", stealth::chrome_148_macos()).await;
}
