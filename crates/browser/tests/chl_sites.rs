//! Live CHL site validation — the 7 sites from research_2026.md that
//! Playwright MCP succeeds against from this same machine but
//! browser_oxide previously failed. All `#[ignore]` (network-dependent).
//!
//! Run with: cargo test -p browser --test chl_sites -- --ignored --test-threads=1 --nocapture
//!
//! Each test fetches the site landing page through browser_oxide and
//! checks for one of:
//!   - L3 PASS:  page rendered, no challenge markers in body
//!   - CHL:      challenge response detected (cookies, JS challenge body)
//!   - BLOCKED:  outright 403 / WAF block / no useful content
//!
//! After the engine-coherence work in this session, sites that previously
//! served challenges should be more likely to render. If a site still
//! serves a challenge with our improved engine, the gap is most likely
//! either (a) IP/cookie warm-up — we don't carry a warmed profile here, or
//! (b) byte-equality hashes (PNG / audio) that the libpng-sys / WebAudio
//! port would close.

use browser::Page;
use stealth;

async fn fetch_and_classify(url: &str) -> String {
    let profile = stealth::presets::chrome_130_macos();
    let mut page = match Page::navigate(url, profile, 3).await {
        Ok(p) => p,
        Err(e) => return format!("ERROR: {e}"),
    };
    let html = page.content();

    // Same two-tier classifier as `holistic_sweep.rs` — strong (vendor)
    // markers always trusted; weak (generic words) only consulted if body
    // is small enough that a CMS footer wouldn't dominate the bytes.
    let lower = html.to_lowercase();
    let len = html.len();
    let strong_markers: &[(&str, &str)] = &[
        ("just a moment", "Cloudflare-CHL"),
        ("checking your browser", "Cloudflare-CHL"),
        ("cf-browser-verification", "Cloudflare-CHL"),
        ("_kpsdk", "Kasada-CHL"),
        ("ips.js", "Kasada-CHL"),
        ("/_sec/cp_challenge", "Akamai-sec-cpt-CHL"),
        ("akam/13", "Akamai-CHL"),
        ("_abck", "Akamai-CHL"),
        ("captcha-delivery", "DataDome-CHL"),
        ("ddcaptchaencoded", "DataDome-CHL"),
        ("press &amp; hold", "PerimeterX-PaH"),
        ("_pxhd", "PerimeterX-CHL"),
    ];
    let weak_markers: &[(&str, &str)] = &[
        ("captcha", "captcha-CHL"),
        ("403 forbidden", "BLOCKED"),
        ("access denied", "BLOCKED"),
        ("blocked", "BLOCKED"),
    ];
    for (needle, tag) in strong_markers {
        if lower.contains(needle) {
            return format!("{tag} (body len={len})");
        }
    }
    if len < 100 * 1024 {
        for (needle, tag) in weak_markers {
            if lower.contains(needle) {
                return format!("{tag} (body len={len})");
            }
        }
    }
    if len < 1000 {
        return format!("THIN-BODY (len={len})");
    }
    format!("L3-RENDERED (len={len})")
}

// ================================================================
// The 7 CHL sites from research_2026.md
// ================================================================

#[tokio::test]
#[ignore]
async fn chl_canadagoose() {
    let r = fetch_and_classify("https://www.canadagoose.com/").await;
    eprintln!("canadagoose: {r}");
}

#[tokio::test]
#[ignore]
async fn chl_hyatt() {
    let r = fetch_and_classify("https://www.hyatt.com/").await;
    eprintln!("hyatt: {r}");
}

#[tokio::test]
#[ignore]
async fn chl_adidas() {
    let r = fetch_and_classify("https://www.adidas.com/us").await;
    eprintln!("adidas: {r}");
}

#[tokio::test]
#[ignore]
async fn chl_zillow() {
    let r = fetch_and_classify("https://www.zillow.com/").await;
    eprintln!("zillow: {r}");
}

#[tokio::test]
#[ignore]
async fn chl_wildberries() {
    let r = fetch_and_classify("https://www.wildberries.ru/").await;
    eprintln!("wildberries: {r}");
}

#[tokio::test]
#[ignore]
async fn chl_ozon() {
    let r = fetch_and_classify("https://www.ozon.ru/").await;
    eprintln!("ozon: {r}");
}

#[tokio::test]
#[ignore]
async fn chl_douyin() {
    let r = fetch_and_classify("https://www.douyin.com/").await;
    eprintln!("douyin: {r}");
}

// ================================================================
// Public anti-bot test pages (also from research_2026.md)
// ================================================================

#[tokio::test]
#[ignore]
async fn fp_creepjs() {
    let r = fetch_and_classify("https://abrahamjuliot.github.io/creepjs/").await;
    eprintln!("creepjs: {r}");
}

#[tokio::test]
#[ignore]
async fn fp_sannysoft() {
    let r = fetch_and_classify("https://bot.sannysoft.com/").await;
    eprintln!("sannysoft: {r}");
}

#[tokio::test]
#[ignore]
async fn fp_browserleaks_canvas() {
    let r = fetch_and_classify("https://browserleaks.com/canvas").await;
    eprintln!("browserleaks-canvas: {r}");
}

#[tokio::test]
#[ignore]
async fn fp_pixelscan() {
    let r = fetch_and_classify("https://pixelscan.net/").await;
    eprintln!("pixelscan: {r}");
}

#[tokio::test]
#[ignore]
async fn fp_areyouheadless() {
    let r = fetch_and_classify("https://arh.antoinevastel.com/bots/areyouheadless").await;
    eprintln!("areyouheadless: {r}");
}

#[tokio::test]
#[ignore]
async fn fp_botd() {
    let r = fetch_and_classify("https://fingerprint.com/products/bot-detection/").await;
    eprintln!("botd: {r}");
}

#[tokio::test]
#[ignore]
async fn fp_fingerprintscan() {
    let r = fetch_and_classify("https://fingerprintscan.com/").await;
    eprintln!("fingerprintscan: {r}");
}

#[tokio::test]
#[ignore]
async fn fp_nowsecure() {
    let r = fetch_and_classify("https://www.nowsecure.nl/").await;
    eprintln!("nowsecure: {r}");
}
