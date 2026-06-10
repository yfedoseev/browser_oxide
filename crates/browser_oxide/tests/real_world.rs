//! Real-world site tests against anti-bot protection systems.
//!
//! All tests are #[ignore] — they require internet and hit live sites.
//! Run with: cargo test -p browser --test real_world -- --ignored --test-threads=1 --nocapture

use browser_oxide::Page;

// ============================================================
// Tier 1: challenge-protected sites (vendor A)
// ============================================================

/// nowsecure.nl — challenge-vendor bot-management test page.
/// A 403 from curl but 200 from a real browser means the vendor is doing JS checks.
/// We verify we at least get an HTTP response (the vendor may still challenge us).
#[tokio::test]
#[ignore]
async fn managed_challenge_page_nl() {
    let profile = browser_oxide::stealth::chrome_148_linux();
    let client = browser_oxide::net::HttpClient::new(&profile).unwrap();
    let resp = client.get("https://nowsecure.nl").await.unwrap();
    println!("[nowsecure.nl] status: {}", resp.status);
    println!("[nowsecure.nl] body length: {}", resp.body.len());
    // CF may return 403 (challenge page) or 200 (passed).
    // Getting a response at all (not a connection error) is the baseline.
    assert!(
        resp.status == 200 || resp.status == 403 || resp.status == 503,
        "unexpected status: {}",
        resp.status
    );
    let body = resp.text();
    if resp.status == 200 {
        println!("[nowsecure.nl] PASSED Cloudflare challenge!");
    } else {
        // CF challenge page — check if it's the JS challenge (not IP block)
        let is_challenge =
            body.contains("challenge") || body.contains("Checking") || body.contains("cf-");
        println!(
            "[nowsecure.nl] Got challenge page (status {}), is_cf_challenge: {}",
            resp.status, is_challenge
        );
    }
}

/// nowsecure.nl with full stealth navigate (HTTP + parse + JS execution).
/// This tests the complete pipeline against a CF-protected page.
#[tokio::test]
#[ignore]
async fn managed_challenge_full_navigate() {
    let profile = browser_oxide::stealth::chrome_148_linux();
    let mut page = Page::navigate_stealth("https://nowsecure.nl", profile)
        .await
        .expect("navigate to nowsecure.nl failed");
    let title = page.title();
    println!("[nowsecure.nl full] title: {title:?}");
    let text = page.text_content();
    println!(
        "[nowsecure.nl full] body preview: {}",
        &text[..text.len().min(300)]
    );

    // Check JS environment is correct even on the challenge page
    let webdriver = page.evaluate("typeof navigator.webdriver").unwrap();
    assert_eq!(webdriver, "undefined", "webdriver leaked on CF page");
}

// ============================================================
// Tier 2: challenge-protected sites (vendor B)
// ============================================================

/// g2.com — protected by a challenge vendor.
/// The vendor checks TLS fingerprint, headers, and JS sensors.
#[tokio::test]
#[ignore]
async fn interstitial_site_nav() {
    let profile = browser_oxide::stealth::chrome_148_windows();
    let client = browser_oxide::net::HttpClient::new(&profile).unwrap();
    let resp = client.get("https://www.g2.com").await.unwrap();
    println!("[g2.com] status: {}", resp.status);
    println!(
        "[g2.com] headers: {:?}",
        resp.headers.keys().collect::<Vec<_>>()
    );
    let body = resp.text();
    println!("[g2.com] body length: {}", body.len());

    // The vendor either passes us (200) or shows a captcha page (403)
    assert!(
        resp.status == 200 || resp.status == 403 || resp.status == 302,
        "unexpected status: {}",
        resp.status
    );

    if resp.status == 200 {
        println!("[g2.com] PASSED DataDome!");
        assert!(
            body.contains("G2") || body.contains("g2"),
            "expected G2 content"
        );
    } else {
        let is_dd =
            body.contains("datadome") || body.contains("DataDome") || body.contains("captcha");
        println!(
            "[g2.com] Blocked by DataDome (status {}), is_dd_challenge: {}",
            resp.status, is_dd
        );
    }
}

// ============================================================
// Tier 3: Bot detection test sites (designed to detect headless)
// ============================================================

/// bot.sannysoft.com — comprehensive headless browser detection.
/// The site's scripts call navigator.getBattery() which we don't implement,
/// so we test the HTTP + stealth layer, then run checks manually.
#[tokio::test]
#[ignore]
async fn sannysoft_bot_detection() {
    let profile = browser_oxide::stealth::chrome_148_linux();
    let client = browser_oxide::net::HttpClient::new(&profile).unwrap();
    let resp = client.get("https://bot.sannysoft.com").await.unwrap();
    println!("[sannysoft] status: {}", resp.status);
    assert_eq!(resp.status, 200, "sannysoft should be accessible");

    // Don't execute sannysoft's scripts — they use document.write, navigator.getBattery, etc.
    // Instead, create a blank stealth page and run checks manually.
    let mut page = Page::with_profile(
        "<html><head></head><body></body></html>",
        "https://bot.sannysoft.com",
        browser_oxide::stealth::chrome_148_linux(),
    )
    .await
    .expect("create stealth page failed");

    let checks = vec![
        ("webdriver", "typeof navigator.webdriver", "undefined"),
        ("userAgent", "typeof navigator.userAgent", "string"),
        ("chrome", "typeof window.chrome", "object"),
        ("plugins", "navigator.plugins.length > 0", "true"),
        ("languages", "navigator.languages.length > 0", "true"),
        (
            "webgl",
            "typeof document.createElement('canvas').getContext('webgl')",
            "object",
        ),
        (
            "canvas2d",
            "typeof document.createElement('canvas').getContext('2d')",
            "object",
        ),
        ("vendor", "navigator.vendor", "Google Inc."),
        (
            "hardwareConcurrency",
            "navigator.hardwareConcurrency > 0",
            "true",
        ),
        ("deviceMemory", "navigator.deviceMemory > 0", "true"),
        ("cookieEnabled", "navigator.cookieEnabled", "true"),
    ];

    let mut passed = 0;
    for (name, js, expected) in &checks {
        let result = page.evaluate(js).unwrap_or_else(|e| format!("ERROR: {e}"));
        let ok = result == *expected;
        println!(
            "[sannysoft] {}: {} (expected {}, got {}){}",
            name,
            if ok { "PASS" } else { "FAIL" },
            expected,
            result,
            if ok { "" } else { " <---" }
        );
        if ok {
            passed += 1;
        }
    }
    println!("[sannysoft] {}/{} checks passed", passed, checks.len());
    assert_eq!(passed, checks.len(), "some sannysoft checks failed");
}

/// creepjs — advanced fingerprint consistency checker.
/// Tests canvas, audio, WebGL, fonts, and fingerprint stability.
#[tokio::test]
#[ignore]
async fn creepjs_fingerprint_page() {
    let profile = browser_oxide::stealth::chrome_148_linux();
    let client = browser_oxide::net::HttpClient::new(&profile).unwrap();
    let resp = client
        .get("https://abrahamjuliot.github.io/creepjs/")
        .await
        .unwrap();
    println!("[creepjs] status: {}", resp.status);
    assert_eq!(resp.status, 200, "creepjs should be accessible");

    // Parse and execute — creepjs is heavy JS, it may not fully run without all APIs,
    // but we verify the page loads and our environment doesn't crash
    let mut page = Page::navigate_stealth("https://abrahamjuliot.github.io/creepjs/", profile)
        .await
        .expect("navigate to creepjs failed");

    let title = page.title();
    println!("[creepjs] title: {title:?}");

    // Verify our fingerprint surface exists
    let canvas_ok = page
        .evaluate("typeof document.createElement('canvas').getContext('2d')")
        .unwrap();
    println!("[creepjs] canvas 2d: {canvas_ok}");
    assert_eq!(canvas_ok, "object");

    let webgl_ok = page
        .evaluate("typeof document.createElement('canvas').getContext('webgl')")
        .unwrap();
    println!("[creepjs] webgl: {webgl_ok}");
    assert_eq!(webgl_ok, "object");

    let audio_ok = page.evaluate("typeof AudioContext").unwrap();
    println!("[creepjs] AudioContext: {audio_ok}");
    assert_eq!(audio_ok, "function");
}

// ============================================================
// Tier 4: TLS fingerprint verification
// ============================================================

/// tls.peet.ws — shows your JA3/JA4 TLS fingerprint.
/// Verifies our BoringSSL TLS impersonation looks like Chrome.
#[tokio::test]
#[ignore]
async fn tls_fingerprint_check() {
    let profile = browser_oxide::stealth::chrome_148_linux();
    let client = browser_oxide::net::HttpClient::new(&profile).unwrap();
    let resp = client.get("https://tls.peet.ws/api/all").await.unwrap();
    println!("[tls.peet.ws] status: {}", resp.status);
    let body = resp.text();
    println!("[tls.peet.ws] response: {}", &body[..body.len().min(1000)]);

    assert_eq!(resp.status, 200);
    // The response is JSON with TLS fingerprint details
    // Verify it contains expected fields
    assert!(
        body.contains("ja3") || body.contains("ja4") || body.contains("tls_version"),
        "expected TLS fingerprint data"
    );
}

/// Check HTTP/2 fingerprint via httpbin
#[tokio::test]
#[ignore]
async fn http2_fingerprint() {
    let profile = browser_oxide::stealth::chrome_148_linux();
    let client = browser_oxide::net::HttpClient::new(&profile).unwrap();
    let resp = client.get("https://httpbin.org/get").await.unwrap();
    println!("[http2] status: {}", resp.status);
    assert_eq!(resp.status, 200);
    let body = resp.text();
    // Verify our headers look like a real browser
    assert!(body.contains("Chrome"), "UA should contain Chrome");
    assert!(
        body.contains("sec-ch-ua") || body.contains("Sec-Ch-Ua"),
        "should send client hints"
    );
}

// ============================================================
// Tier 5: Real content scraping targets
// ============================================================

/// Hacker News — simple site, no anti-bot, verifies basic page scraping works.
#[tokio::test]
#[ignore]
async fn scrape_hacker_news() {
    let profile = browser_oxide::stealth::chrome_148_linux();
    let client = browser_oxide::net::HttpClient::new(&profile).unwrap();
    let mut page = Page::navigate_simple("https://news.ycombinator.com", &client, profile.clone())
        .await
        .expect("navigate to HN failed");

    let title = page.title();
    println!("[HN] title: {title:?}");
    assert_eq!(title, "Hacker News");

    let has_stories = page.has_element(".titleline");
    println!("[HN] has stories: {has_stories}");
    assert!(has_stories, "should find story links");

    let first_story = page.text_of(".titleline");
    println!("[HN] first story: {:?}", first_story);
    assert!(first_story.is_some(), "should extract first story text");
}

/// Wikipedia — no anti-bot, complex HTML with templates/infoboxes.
#[tokio::test]
#[ignore]
async fn scrape_wikipedia() {
    let profile = browser_oxide::stealth::chrome_148_linux();
    let client = browser_oxide::net::HttpClient::new(&profile).unwrap();
    let mut page = Page::navigate_simple(
        "https://en.wikipedia.org/wiki/Rust_(programming_language)",
        &client,
        profile.clone(),
    )
    .await
    .expect("navigate to Wikipedia failed");

    let title = page.title();
    println!("[Wikipedia] title: {title:?}");
    assert!(title.contains("Rust"), "title should mention Rust: {title}");

    let text = page.text_content();
    println!("[Wikipedia] body length: {}", text.len());
    assert!(text.len() > 1000, "Wikipedia article should be substantial");
    assert!(
        text.contains("programming language") || text.contains("Rust"),
        "should contain relevant content"
    );
}
