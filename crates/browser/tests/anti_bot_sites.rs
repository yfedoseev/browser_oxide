//! Anti-bot site test suite — tests against 100+ protected real-world sites.
//!
//! All tests are #[ignore] — they require internet and hit live sites.
//! Run with: cargo test -p browser --test anti_bot_sites -- --ignored --test-threads=1 --nocapture
//!
//! These tests verify HTTP-level access (TLS fingerprint + headers).
//! A 200 means we passed the initial bot check. A 403/503 means we got challenged.
//! Even challenged responses are useful — they confirm connectivity and tell us
//! what protection system responded.

use browser::Page;

/// Test result for a site probe.
struct ProbeResult {
    url: String,
    status: u16,
    protection: String,
    passed: bool,
    body_len: usize,
    notes: String,
}

impl ProbeResult {
    fn print(&self) {
        let icon = if self.passed { "PASS" } else { "BLOCK" };
        println!(
            "[{icon}] {url} — {status} ({protection}) body={body_len}b {notes}",
            icon = icon,
            url = self.url,
            status = self.status,
            protection = self.protection,
            body_len = self.body_len,
            notes = self.notes,
        );
    }
}

async fn probe(url: &str, profile: stealth::StealthProfile, protection: &str) -> ProbeResult {
    let client = match net::HttpClient::new(&profile) {
        Ok(c) => c,
        Err(e) => {
            return ProbeResult {
                url: url.into(),
                status: 0,
                protection: protection.into(),
                passed: false,
                body_len: 0,
                notes: format!("client error: {e}"),
            };
        }
    };
    match client.get(url).await {
        Ok(resp) => {
            let body = resp.text();
            let passed = resp.status == 200 || resp.status == 301 || resp.status == 302;
            let notes = if !passed {
                detect_protection(&body, &resp.headers)
            } else {
                String::new()
            };
            ProbeResult {
                url: url.into(),
                status: resp.status,
                protection: protection.into(),
                passed,
                body_len: body.len(),
                notes,
            }
        }
        Err(e) => ProbeResult {
            url: url.into(),
            status: 0,
            protection: protection.into(),
            passed: false,
            body_len: 0,
            notes: format!("request error: {e}"),
        },
    }
}

fn detect_protection(body: &str, headers: &std::collections::HashMap<String, String>) -> String {
    let mut signals = Vec::new();
    if headers.contains_key("cf-ray") {
        signals.push("cloudflare");
    }
    if headers.contains_key("x-datadome") || headers.contains_key("x-dd-b") {
        signals.push("datadome");
    }
    if body.contains("_abck") || body.contains("akamai") {
        signals.push("akamai");
    }
    if headers.contains_key("x-px")
        || body.contains("perimeterx")
        || body.contains("human security")
    {
        signals.push("perimeterx");
    }
    if body.contains("kasada") || headers.contains_key("x-kpsdk-ct") {
        signals.push("kasada");
    }
    if body.contains("challenge") {
        signals.push("challenge-page");
    }
    if body.contains("captcha") || body.contains("CAPTCHA") {
        signals.push("captcha");
    }
    if body.contains("Just a moment") {
        signals.push("cf-interstitial");
    }
    signals.join(", ")
}

// ================================================================
// Cloudflare Bot Management
// ================================================================

#[tokio::test]
#[ignore]
async fn cf_nowsecure() {
    let r = probe(
        "https://nowsecure.nl",
        stealth::chrome_130_linux(),
        "cloudflare",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn cf_discord() {
    let r = probe(
        "https://discord.com",
        stealth::chrome_130_windows(),
        "cloudflare",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn cf_medium() {
    let r = probe(
        "https://medium.com",
        stealth::chrome_130_windows(),
        "cloudflare",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn cf_indeed() {
    let r = probe(
        "https://www.indeed.com",
        stealth::chrome_130_windows(),
        "cloudflare",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn cf_coinbase() {
    let r = probe(
        "https://www.coinbase.com",
        stealth::chrome_130_windows(),
        "cloudflare",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn cf_glassdoor() {
    let r = probe(
        "https://www.glassdoor.com",
        stealth::chrome_130_windows(),
        "cloudflare",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn cf_patreon() {
    let r = probe(
        "https://www.patreon.com",
        stealth::chrome_130_windows(),
        "cloudflare",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn cf_chatgpt() {
    let r = probe(
        "https://chatgpt.com",
        stealth::chrome_130_windows(),
        "cloudflare",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn cf_bet365() {
    let r = probe(
        "https://www.bet365.com",
        stealth::chrome_130_windows(),
        "cloudflare",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn cf_mercadolibre() {
    let r = probe(
        "https://www.mercadolibre.com",
        stealth::chrome_130_windows(),
        "cloudflare",
    )
    .await;
    r.print();
}

// ================================================================
// DataDome
// ================================================================

#[tokio::test]
#[ignore]
async fn dd_reddit() {
    let r = probe(
        "https://www.reddit.com",
        stealth::chrome_130_windows(),
        "datadome",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn dd_footlocker() {
    let r = probe(
        "https://www.footlocker.com",
        stealth::chrome_130_windows(),
        "datadome",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn dd_hermes() {
    let r = probe(
        "https://www.hermes.com",
        stealth::chrome_130_windows(),
        "datadome",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn dd_soundcloud() {
    let r = probe(
        "https://soundcloud.com",
        stealth::chrome_130_windows(),
        "datadome",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn dd_tripadvisor() {
    let r = probe(
        "https://www.tripadvisor.com",
        stealth::chrome_130_windows(),
        "datadome",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn dd_crunchbase() {
    let r = probe(
        "https://www.crunchbase.com",
        stealth::chrome_130_windows(),
        "datadome",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn dd_leboncoin() {
    let r = probe(
        "https://www.leboncoin.fr",
        stealth::presets::chrome_130_de(),
        "datadome",
    )
    .await;
    r.print();
}

// ================================================================
// Akamai Bot Manager
// ================================================================

#[tokio::test]
#[ignore]
async fn akamai_nike() {
    let r = probe(
        "https://www.nike.com",
        stealth::chrome_130_windows(),
        "akamai",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn akamai_adidas() {
    let r = probe(
        "https://www.adidas.com",
        stealth::chrome_130_windows(),
        "akamai",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn akamai_homedepot() {
    let r = probe(
        "https://www.homedepot.com",
        stealth::chrome_130_windows(),
        "akamai",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn akamai_costco() {
    let r = probe(
        "https://www.costco.com",
        stealth::chrome_130_windows(),
        "akamai",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn akamai_delta() {
    let r = probe(
        "https://www.delta.com",
        stealth::chrome_130_windows(),
        "akamai",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn akamai_united() {
    let r = probe(
        "https://www.united.com",
        stealth::chrome_130_windows(),
        "akamai",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn akamai_marriott() {
    let r = probe(
        "https://www.marriott.com",
        stealth::chrome_130_windows(),
        "akamai",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn akamai_airbnb() {
    let r = probe(
        "https://www.airbnb.com",
        stealth::chrome_130_windows(),
        "akamai",
    )
    .await;
    r.print();
}

// ================================================================
// PerimeterX / HUMAN Security
// ================================================================

#[tokio::test]
#[ignore]
async fn px_walmart() {
    let r = probe(
        "https://www.walmart.com",
        stealth::chrome_130_windows(),
        "perimeterx",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn px_craigslist() {
    let r = probe(
        "https://www.craigslist.org",
        stealth::chrome_130_windows(),
        "perimeterx",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn px_stockx() {
    let r = probe(
        "https://stockx.com",
        stealth::chrome_130_windows(),
        "perimeterx",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn px_nordstrom() {
    let r = probe(
        "https://www.nordstrom.com",
        stealth::chrome_130_windows(),
        "perimeterx",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn px_instacart() {
    let r = probe(
        "https://www.instacart.com",
        stealth::chrome_130_windows(),
        "perimeterx",
    )
    .await;
    r.print();
}

// ================================================================
// Kasada
// ================================================================

#[tokio::test]
#[ignore]
async fn kasada_ticketmaster() {
    let r = probe(
        "https://www.ticketmaster.com",
        stealth::chrome_130_windows(),
        "kasada",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn kasada_ticketmaster_uk() {
    let r = probe(
        "https://www.ticketmaster.co.uk",
        stealth::chrome_130_windows(),
        "kasada",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn kasada_seatgeek() {
    let r = probe(
        "https://seatgeek.com",
        stealth::chrome_130_windows(),
        "kasada",
    )
    .await;
    r.print();
}

// ================================================================
// Shape Security (F5)
// ================================================================

#[tokio::test]
#[ignore]
async fn shape_southwest() {
    let r = probe(
        "https://www.southwest.com",
        stealth::chrome_130_windows(),
        "shape",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn shape_iherb() {
    let r = probe(
        "https://www.iherb.com",
        stealth::chrome_130_windows(),
        "shape",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn shape_gap() {
    let r = probe(
        "https://www.gap.com",
        stealth::chrome_130_windows(),
        "shape",
    )
    .await;
    r.print();
}

// ================================================================
// Fingerprint-heavy sites
// ================================================================

#[tokio::test]
#[ignore]
async fn fp_amazon() {
    let r = probe(
        "https://www.amazon.com",
        stealth::chrome_130_windows(),
        "custom",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn fp_linkedin() {
    let r = probe(
        "https://www.linkedin.com",
        stealth::chrome_130_windows(),
        "custom",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn fp_facebook() {
    let r = probe(
        "https://www.facebook.com",
        stealth::chrome_130_windows(),
        "custom",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn fp_twitter() {
    let r = probe("https://x.com", stealth::chrome_130_windows(), "custom").await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn fp_google() {
    let r = probe(
        "https://www.google.com/search?q=test",
        stealth::chrome_130_windows(),
        "custom",
    )
    .await;
    r.print();
}

// ================================================================
// Chinese sites (with Chinese locale profile)
// ================================================================

#[tokio::test]
#[ignore]
async fn cn_taobao() {
    let r = probe(
        "https://world.taobao.com",
        stealth::presets::chrome_130_cn(),
        "alibaba-waf",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn cn_jd() {
    let r = probe(
        "https://www.jd.com",
        stealth::presets::chrome_130_cn(),
        "jd-waf",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn cn_bilibili() {
    let r = probe(
        "https://www.bilibili.com",
        stealth::presets::chrome_130_cn(),
        "custom",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn cn_douyin() {
    let r = probe(
        "https://www.douyin.com",
        stealth::presets::chrome_130_cn(),
        "bytedance-waf",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn cn_xiaohongshu() {
    let r = probe(
        "https://www.xiaohongshu.com",
        stealth::presets::chrome_130_cn(),
        "custom",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn cn_zhihu() {
    let r = probe(
        "https://www.zhihu.com",
        stealth::presets::chrome_130_cn(),
        "custom",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn cn_baidu() {
    let r = probe(
        "https://www.baidu.com",
        stealth::presets::chrome_130_cn(),
        "baidu-waf",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn cn_weibo() {
    let r = probe(
        "https://weibo.com",
        stealth::presets::chrome_130_cn(),
        "custom",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn cn_tmall() {
    let r = probe(
        "https://www.tmall.com",
        stealth::presets::chrome_130_cn(),
        "alibaba-waf",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn cn_trip() {
    let r = probe(
        "https://www.trip.com",
        stealth::presets::chrome_130_cn(),
        "custom",
    )
    .await;
    r.print();
}

// ================================================================
// Russian sites (with Russian locale profile)
// ================================================================

#[tokio::test]
#[ignore]
async fn ru_yandex() {
    let r = probe(
        "https://ya.ru",
        stealth::presets::chrome_130_ru(),
        "yandex-smartcaptcha",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn ru_ozon() {
    let r = probe(
        "https://www.ozon.ru",
        stealth::presets::chrome_130_ru(),
        "custom",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn ru_wildberries() {
    let r = probe(
        "https://www.wildberries.ru",
        stealth::presets::chrome_130_ru(),
        "custom",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn ru_avito() {
    let r = probe(
        "https://www.avito.ru",
        stealth::presets::chrome_130_ru(),
        "qrator",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn ru_vk() {
    let r = probe(
        "https://vk.com",
        stealth::presets::chrome_130_ru(),
        "custom",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn ru_cian() {
    let r = probe(
        "https://cian.ru",
        stealth::presets::chrome_130_ru(),
        "custom",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn ru_lamoda() {
    let r = probe(
        "https://www.lamoda.ru",
        stealth::presets::chrome_130_ru(),
        "custom",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn ru_dns_shop() {
    let r = probe(
        "https://www.dns-shop.ru",
        stealth::presets::chrome_130_ru(),
        "custom",
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn ru_tinkoff() {
    let r = probe(
        "https://www.tinkoff.ru",
        stealth::presets::chrome_130_ru(),
        "custom",
    )
    .await;
    r.print();
}

// ================================================================
// Fingerprint verification sites
// ================================================================

#[tokio::test]
#[ignore]
async fn verify_tls_fingerprint() {
    let profile = stealth::chrome_130_linux();
    let client = net::HttpClient::new(&profile).unwrap();
    let resp = client.get("https://tls.peet.ws/api/all").await.unwrap();
    let body = resp.text();
    println!("[TLS] status: {}", resp.status);
    println!("[TLS] fingerprint: {}", &body[..body.len().min(500)]);
    assert_eq!(resp.status, 200);
    assert!(
        body.contains("tls") || body.contains("ja"),
        "expected TLS data"
    );
}

#[tokio::test]
#[ignore]
async fn verify_httpbin_headers() {
    let profile = stealth::chrome_130_windows();
    let client = net::HttpClient::new(&profile).unwrap();
    let resp = client.get("https://httpbin.org/headers").await.unwrap();
    let body = resp.text();
    println!("[headers] {}", &body[..body.len().min(500)]);
    assert!(body.contains("Chrome"), "UA should contain Chrome");
    assert!(
        body.contains("Sec-Ch-Ua") || body.contains("sec-ch-ua"),
        "should send client hints"
    );
}

#[tokio::test]
#[ignore]
async fn verify_sannysoft() {
    let profile = stealth::chrome_130_linux();
    let client = net::HttpClient::new(&profile).unwrap();
    let resp = client.get("https://bot.sannysoft.com").await.unwrap();
    println!("[sannysoft] status: {}", resp.status);
    assert_eq!(resp.status, 200);
}

#[tokio::test]
#[ignore]
async fn verify_creepjs() {
    let profile = stealth::chrome_130_linux();
    let client = net::HttpClient::new(&profile).unwrap();
    let resp = client
        .get("https://abrahamjuliot.github.io/creepjs/")
        .await
        .unwrap();
    println!("[creepjs] status: {}", resp.status);
    assert_eq!(resp.status, 200);
}

#[tokio::test]
#[ignore]
async fn verify_browserleaks() {
    let profile = stealth::chrome_130_windows();
    let client = net::HttpClient::new(&profile).unwrap();
    let resp = client.get("https://browserleaks.com").await.unwrap();
    println!("[browserleaks] status: {}", resp.status);
    assert_eq!(resp.status, 200);
}

#[tokio::test]
#[ignore]
async fn verify_pixelscan() {
    let profile = stealth::chrome_130_windows();
    let client = net::HttpClient::new(&profile).unwrap();
    let resp = client.get("https://pixelscan.net").await.unwrap();
    println!("[pixelscan] status: {}", resp.status);
    // pixelscan may return 200 or redirect
    assert!(resp.status == 200 || resp.status == 301 || resp.status == 302);
}

// ================================================================
// Geo-locale verification — Russian profile
// ================================================================

#[tokio::test]
#[ignore]
async fn verify_ru_locale_headers() {
    let profile = stealth::presets::chrome_130_ru();
    let client = net::HttpClient::new(&profile).unwrap();
    let resp = client.get("https://httpbin.org/headers").await.unwrap();
    let body = resp.text();
    println!("[ru-locale] headers: {}", &body[..body.len().min(500)]);
    // Accept-Language should contain Russian
    assert!(
        body.contains("ru"),
        "Accept-Language should contain 'ru': {}",
        body
    );
}

#[tokio::test]
#[ignore]
async fn verify_ru_locale_js() {
    let profile = stealth::presets::chrome_130_ru();
    let mut page = Page::with_profile(
        "<html><head></head><body></body></html>",
        "https://ya.ru",
        profile,
    )
    .await
    .unwrap();
    let lang = page.evaluate("navigator.language").unwrap();
    assert_eq!(lang, "ru-RU");
    let langs = page
        .evaluate("JSON.stringify(navigator.languages)")
        .unwrap();
    assert!(langs.contains("ru"), "languages should contain ru: {langs}");
    let tz = page
        .evaluate("Intl.DateTimeFormat().resolvedOptions().timeZone")
        .unwrap();
    println!("[ru-locale] timezone from Intl: {tz}");
}

// ================================================================
// Geo-locale verification — Chinese profile
// ================================================================

#[tokio::test]
#[ignore]
async fn verify_cn_locale_headers() {
    let profile = stealth::presets::chrome_130_cn();
    let client = net::HttpClient::new(&profile).unwrap();
    let resp = client.get("https://httpbin.org/headers").await.unwrap();
    let body = resp.text();
    println!("[cn-locale] headers: {}", &body[..body.len().min(500)]);
    assert!(
        body.contains("zh"),
        "Accept-Language should contain 'zh': {}",
        body
    );
}

#[tokio::test]
#[ignore]
async fn verify_cn_locale_js() {
    let profile = stealth::presets::chrome_130_cn();
    let mut page = Page::with_profile(
        "<html><head></head><body></body></html>",
        "https://baidu.com",
        profile,
    )
    .await
    .unwrap();
    let lang = page.evaluate("navigator.language").unwrap();
    assert_eq!(lang, "zh-CN");
    let langs = page
        .evaluate("JSON.stringify(navigator.languages)")
        .unwrap();
    assert!(langs.contains("zh"), "languages should contain zh: {langs}");
}

// ================================================================
// Summary runner — probe all sites and print scorecard
// ================================================================

#[tokio::test]
#[ignore]
async fn scorecard_all_sites() {
    let sites: Vec<(&str, &str, fn() -> stealth::StealthProfile)> = vec![
        // Cloudflare
        (
            "https://nowsecure.nl",
            "cloudflare",
            stealth::chrome_130_linux as fn() -> _,
        ),
        (
            "https://discord.com",
            "cloudflare",
            stealth::chrome_130_windows,
        ),
        (
            "https://medium.com",
            "cloudflare",
            stealth::chrome_130_windows,
        ),
        (
            "https://www.coinbase.com",
            "cloudflare",
            stealth::chrome_130_windows,
        ),
        // DataDome
        (
            "https://www.reddit.com",
            "datadome",
            stealth::chrome_130_windows,
        ),
        (
            "https://www.footlocker.com",
            "datadome",
            stealth::chrome_130_windows,
        ),
        (
            "https://www.tripadvisor.com",
            "datadome",
            stealth::chrome_130_windows,
        ),
        // Akamai
        (
            "https://www.nike.com",
            "akamai",
            stealth::chrome_130_windows,
        ),
        (
            "https://www.homedepot.com",
            "akamai",
            stealth::chrome_130_windows,
        ),
        (
            "https://www.airbnb.com",
            "akamai",
            stealth::chrome_130_windows,
        ),
        // PerimeterX
        (
            "https://www.walmart.com",
            "perimeterx",
            stealth::chrome_130_windows,
        ),
        (
            "https://stockx.com",
            "perimeterx",
            stealth::chrome_130_windows,
        ),
        // Kasada
        (
            "https://www.ticketmaster.com",
            "kasada",
            stealth::chrome_130_windows,
        ),
        // Shape
        (
            "https://www.southwest.com",
            "shape",
            stealth::chrome_130_windows,
        ),
        // Chinese
        (
            "https://www.baidu.com",
            "baidu-waf",
            stealth::presets::chrome_130_cn,
        ),
        (
            "https://www.bilibili.com",
            "custom-cn",
            stealth::presets::chrome_130_cn,
        ),
        (
            "https://www.jd.com",
            "jd-waf",
            stealth::presets::chrome_130_cn,
        ),
        // Russian
        ("https://ya.ru", "yandex", stealth::presets::chrome_130_ru),
        (
            "https://www.ozon.ru",
            "custom-ru",
            stealth::presets::chrome_130_ru,
        ),
        (
            "https://vk.com",
            "custom-ru",
            stealth::presets::chrome_130_ru,
        ),
        // Big tech
        (
            "https://www.amazon.com",
            "custom",
            stealth::chrome_130_windows,
        ),
        (
            "https://www.google.com/search?q=test",
            "custom",
            stealth::chrome_130_windows,
        ),
        (
            "https://www.linkedin.com",
            "custom",
            stealth::chrome_130_windows,
        ),
    ];

    let mut passed = 0;
    let mut failed = 0;
    let mut errors = 0;

    println!("\n============================================================");
    println!(" ANTI-BOT SCORECARD — {} sites", sites.len());
    println!("============================================================\n");

    for (url, protection, profile_fn) in &sites {
        let r = probe(url, profile_fn(), protection).await;
        r.print();
        if r.status == 0 {
            errors += 1;
        } else if r.passed {
            passed += 1;
        } else {
            failed += 1;
        }
    }

    println!("\n============================================================");
    println!(
        " RESULTS: {} passed / {} blocked / {} errors  ({}%)",
        passed,
        failed,
        errors,
        if passed + failed > 0 {
            passed * 100 / (passed + failed)
        } else {
            0
        }
    );
    println!("============================================================\n");
}
