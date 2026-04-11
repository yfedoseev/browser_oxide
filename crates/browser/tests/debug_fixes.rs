//! Tests for specific fix attempts.
//! Run: cargo test -p browser --test debug_fixes -- --ignored --test-threads=1 --nocapture

#[tokio::test]
#[ignore]
async fn ozon_with_redirect_follow() {
    let profile = stealth::presets::chrome_130_ru();
    let client = net::HttpClient::new(&profile).unwrap();
    let resp = client.get_follow("https://www.ozon.ru", 5).await.unwrap();
    println!("[ozon follow] status: {}", resp.status);
    println!("[ozon follow] url: {}", resp.url);
    println!("[ozon follow] body length: {}", resp.body.len());
    let body = resp.text();
    println!("[ozon follow] preview: {}", &body[..body.len().min(300)]);
}

#[tokio::test]
#[ignore]
async fn airbnb_header_check() {
    // Check what headers we're actually sending
    let profile = stealth::chrome_130_windows();
    let client = net::HttpClient::new(&profile).unwrap();
    let resp = client.get("https://httpbin.org/headers").await.unwrap();
    println!("[our headers]:\n{}", resp.text());
}

#[tokio::test]
#[ignore]
async fn yandex_http11() {
    // Try ya.ru with explicit HTTP/1.1 fallback
    let profile = stealth::presets::chrome_130_ru();
    let client = net::HttpClient::new(&profile).unwrap();
    // Try the search page instead of root
    let resp = client.get("https://yandex.ru/search/?text=test").await;
    match resp {
        Ok(r) => {
            println!("[yandex search] status: {}", r.status);
            println!("[yandex search] body length: {}", r.body.len());
        }
        Err(e) => println!("[yandex search] error: {e}"),
    }
}

#[tokio::test]
#[ignore]
async fn amazon_challenge_content() {
    // Get Amazon's challenge page and examine the JS
    let profile = stealth::chrome_130_windows();
    let client = net::HttpClient::new(&profile).unwrap();
    let resp = client.get("https://www.amazon.com").await.unwrap();
    println!("[amazon] status: {}", resp.status);
    println!("[amazon] full body:\n{}", resp.text());
}
