//! Holistic site verification — runs a comprehensive list of high-traffic
//! and high-protection sites through `Page::navigate` and prints per-site
//! outcomes. Each site has its own `#[tokio::test]` so they get isolated
//! tokio runtimes — running them all in one tokio task shares the deno_core
//! event loop, and a setTimeout fired by site N can re-enter the runtime
//! while site N+1 is awaiting, hitting `RefCell already borrowed`.
//!
//! Run with:
//!     cargo test --release -p browser --test holistic_sweep \
//!         -- --ignored --test-threads=1 --nocapture 2>&1 | tee log
//!
//! Then `grep -E "^holistic:" log` extracts the one-line outcome per site
//! for downstream report aggregation.

use browser::{Page, ParallelPager};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Returns (outcome, body_len, navigate_ms, drop_ms).
/// The split lets us see whether time is in Page::navigate (network/JS work)
/// or in `drop(page)` (V8 isolate teardown / pending tokio task drain).
/// Picks profile from BOXIDE_PROFILE env var (default `chrome_130_macos`).
/// Supported values: `chrome_130_macos|windows|linux`, `firefox_135_macos|windows|linux`.
fn pick_profile() -> stealth::StealthProfile {
    match std::env::var("BOXIDE_PROFILE")
        .unwrap_or_else(|_| "chrome_130_macos".into())
        .as_str()
    {
        "chrome_130_macos" => stealth::presets::chrome_130_macos(),
        "chrome_130_windows" => stealth::presets::chrome_130_windows(),
        "chrome_130_linux" => stealth::presets::chrome_130_linux(),
        "firefox_135_macos" => stealth::presets::firefox_135_macos(),
        "firefox_135_windows" => stealth::presets::firefox_135_windows(),
        "firefox_135_linux" => stealth::presets::firefox_135_linux(),
        // 2026-05-12 mobile profiles (Phase 2 + Phase 3)
        "pixel_9_pro_chrome_147" => stealth::presets::pixel_9_pro_chrome_147(),
        "iphone_15_pro_safari_18" => stealth::presets::iphone_15_pro_safari_18(),
        other => panic!("unknown BOXIDE_PROFILE={other}"),
    }
}

async fn fetch_one(url: &str) -> (String, usize, u64, u64) {
    let nav_start = Instant::now();
    let profile = pick_profile();
    let result = tokio::time::timeout(Duration::from_secs(300), async {
        let mut page = match Page::navigate(url, profile, 3).await {
            Ok(p) => p,
            Err(e) => return (format!("ERROR: {e}"), 0, page_drop_marker()),
        };
        let html = page.content();
        let lower = html.to_lowercase();
        let len = html.len();

        // Vendor-specific markers — high-confidence detections. If any of
        // these substrings appears, the response is definitely a challenge
        // page (these strings don't legitimately appear in normal site
        // bodies — e.g. `_kpsdk` is Kasada's tracker variable name).
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
        // Weak markers — common words that appear in normal site footers /
        // privacy policies / blog posts. Only consult these if the body is
        // small enough that a CMS footer wouldn't dominate the bytes.
        let weak_markers: &[(&str, &str)] = &[
            ("captcha", "captcha-CHL"),
            ("403 forbidden", "BLOCKED"),
            ("access denied", "BLOCKED"),
            ("blocked", "BLOCKED"),
        ];
        let outcome = {
            // 1. Strong markers — always trust.
            let mut o = "L3-RENDERED".to_string();
            for (needle, tag) in strong_markers {
                if lower.contains(needle) {
                    o = tag.to_string();
                    break;
                }
            }
            // 2. Weak markers — only when body is small (<100 KB). A site
            // returning 100 KB+ of HTML almost certainly rendered the real
            // homepage; the substring "captcha" is footer / cookie-banner
            // text, not a challenge. (github.com 581 KB, bbc.com 537 KB,
            // washingtonpost.com 3.1 MB all hit this false-positive in the
            // Phase A baseline sweep — DEEP_NEXT_STEPS Part 2 §C.)
            if o == "L3-RENDERED" && len < 100 * 1024 {
                for (needle, tag) in weak_markers {
                    if lower.contains(needle) {
                        o = tag.to_string();
                        break;
                    }
                }
            }
            // 3. Stub bodies — definitely not a real render.
            if o == "L3-RENDERED" && len < 1000 {
                o = "THIN-BODY".to_string();
            }
            o
        };
        // Capture drop start before page goes out of scope; the actual drop
        // happens here at end-of-block.
        let drop_t0 = Instant::now();
        drop(page);
        let drop_ms = drop_t0.elapsed().as_millis() as u64;
        (outcome, len, drop_ms)
    })
    .await;
    let nav_ms = nav_start.elapsed().as_millis() as u64;
    match result {
        Ok((outcome, len, drop_ms)) => (outcome, len, nav_ms.saturating_sub(drop_ms), drop_ms),
        Err(_) => ("TIMEOUT".to_string(), 0, nav_ms, 0),
    }
}

// Sentinel used by the error path to keep the tuple shape uniform. Not
// actually a real drop measurement — the page never existed.
fn page_drop_marker() -> u64 {
    0
}

/// Define a site test. Each test prints two lines so post-run aggregation
/// can compute both navigation time AND between-test gap (which surfaces
/// stalls in the test framework / runtime drop). Format:
///
///   holistic-start: <ts_ms> <cat> <name> <url>
///   holistic-end:   <ts_ms> <cat> <name> <outcome> len=<n> nav_ms=<n> drop_ms=<n>
///
/// The end's ts_ms is also the next test's start - 1 if the framework is
/// instant; any gap reveals tokio runtime teardown or other overhead.
macro_rules! site {
    ($name:ident, $cat:expr, $sname:expr, $url:expr) => {
        #[tokio::test]
        #[ignore]
        async fn $name() {
            eprintln!(
                "holistic-start: {} {} {} {}",
                now_unix_ms(),
                $cat,
                $sname,
                $url
            );
            let (out, len, nav_ms, drop_ms) = fetch_one($url).await;
            eprintln!(
                "holistic-end: {} {} {} {} len={} nav_ms={} drop_ms={} url={}",
                now_unix_ms(),
                $cat,
                $sname,
                out,
                len,
                nav_ms,
                drop_ms,
                $url
            );
        }
    };
}

// ----- Search engines -----
site!(
    h_search_google,
    "search",
    "google",
    "https://www.google.com/"
);
site!(h_search_bing, "search", "bing", "https://www.bing.com/");
site!(
    h_search_duckduckgo,
    "search",
    "duckduckgo",
    "https://duckduckgo.com/"
);
site!(h_search_yandex, "search", "yandex", "https://ya.ru/");
site!(h_search_yahoo, "search", "yahoo", "https://www.yahoo.com/");
site!(
    h_search_brave,
    "search",
    "brave",
    "https://search.brave.com/"
);
site!(
    h_search_ecosia,
    "search",
    "ecosia",
    "https://www.ecosia.org/"
);
site!(
    h_search_startpage,
    "search",
    "startpage",
    "https://www.startpage.com/"
);

// ----- Reference & wiki -----
site!(
    h_ref_wikipedia_en,
    "reference",
    "wikipedia-en",
    "https://en.wikipedia.org/wiki/Main_Page"
);
site!(
    h_ref_wiktionary,
    "reference",
    "wiktionary",
    "https://en.wiktionary.org/"
);
site!(
    h_ref_stackoverflow,
    "reference",
    "stackoverflow",
    "https://stackoverflow.com/"
);
site!(h_ref_github, "reference", "github", "https://github.com/");
site!(
    h_ref_mdn,
    "reference",
    "mdn",
    "https://developer.mozilla.org/"
);

// ----- News -----
site!(h_news_bbc, "news", "bbc", "https://www.bbc.com/");
site!(h_news_cnn, "news", "cnn", "https://www.cnn.com/");
site!(
    h_news_nytimes,
    "news",
    "nytimes",
    "https://www.nytimes.com/"
);
site!(
    h_news_reuters,
    "news",
    "reuters",
    "https://www.reuters.com/"
);
site!(
    h_news_guardian,
    "news",
    "guardian",
    "https://www.theguardian.com/"
);
site!(
    h_news_washingtonpost,
    "news",
    "washingtonpost",
    "https://www.washingtonpost.com/"
);
site!(h_news_wsj, "news", "wsj", "https://www.wsj.com/");
site!(
    h_news_bloomberg,
    "news",
    "bloomberg",
    "https://www.bloomberg.com/"
);
site!(
    h_news_economist,
    "news",
    "economist",
    "https://www.economist.com/"
);
site!(h_news_ft, "news", "ft", "https://www.ft.com/");

// ----- Social -----
site!(h_soc_reddit, "social", "reddit", "https://www.reddit.com/");
site!(h_soc_twitter, "social", "twitter", "https://twitter.com/");
site!(h_soc_x, "social", "x-com", "https://x.com/");
site!(
    h_soc_linkedin,
    "social",
    "linkedin",
    "https://www.linkedin.com/"
);
site!(
    h_soc_facebook,
    "social",
    "facebook",
    "https://www.facebook.com/"
);
site!(
    h_soc_instagram,
    "social",
    "instagram",
    "https://www.instagram.com/"
);
site!(
    h_soc_pinterest,
    "social",
    "pinterest",
    "https://www.pinterest.com/"
);
site!(h_soc_tumblr, "social", "tumblr", "https://www.tumblr.com/");
site!(h_soc_quora, "social", "quora", "https://www.quora.com/");
site!(
    h_soc_threads,
    "social",
    "threads",
    "https://www.threads.net/"
);

// ----- Amazon -----
site!(h_amz_com, "amazon", "amazon-com", "https://www.amazon.com/");
site!(
    h_amz_uk,
    "amazon",
    "amazon-co-uk",
    "https://www.amazon.co.uk/"
);
site!(h_amz_de, "amazon", "amazon-de", "https://www.amazon.de/");
site!(h_amz_fr, "amazon", "amazon-fr", "https://www.amazon.fr/");
site!(h_amz_jp, "amazon", "amazon-jp", "https://www.amazon.co.jp/");
site!(h_amz_in, "amazon", "amazon-in", "https://www.amazon.in/");
site!(h_amz_ca, "amazon", "amazon-ca", "https://www.amazon.ca/");
site!(
    h_amz_au,
    "amazon",
    "amazon-com-au",
    "https://www.amazon.com.au/"
);

// ----- Stores -----
site!(h_store_ebay, "stores", "ebay", "https://www.ebay.com/");
site!(h_store_etsy, "stores", "etsy", "https://www.etsy.com/");
site!(
    h_store_walmart,
    "stores",
    "walmart",
    "https://www.walmart.com/"
);
site!(
    h_store_target,
    "stores",
    "target",
    "https://www.target.com/"
);
site!(
    h_store_bestbuy,
    "stores",
    "bestbuy",
    "https://www.bestbuy.com/"
);
site!(
    h_store_homedepot,
    "stores",
    "homedepot",
    "https://www.homedepot.com/"
);
site!(
    h_store_costco,
    "stores",
    "costco",
    "https://www.costco.com/"
);
site!(
    h_store_shopify,
    "stores",
    "shopify",
    "https://www.shopify.com/"
);
site!(
    h_store_alibaba,
    "stores",
    "alibaba",
    "https://www.alibaba.com/"
);
site!(
    h_store_aliexpress,
    "stores",
    "aliexpress",
    "https://www.aliexpress.com/"
);
site!(h_store_asos, "stores", "asos", "https://www.asos.com/");
site!(h_store_ikea, "stores", "ikea", "https://www.ikea.com/");
site!(
    h_store_wayfair,
    "stores",
    "wayfair",
    "https://www.wayfair.com/"
);
site!(h_store_macys, "stores", "macys", "https://www.macys.com/");
site!(h_store_zara, "stores", "zara", "https://www.zara.com/");
site!(h_store_hm, "stores", "h-m", "https://www2.hm.com/");
site!(
    h_store_uniqlo,
    "stores",
    "uniqlo",
    "https://www.uniqlo.com/"
);

// ----- Streaming -----
site!(
    h_stream_youtube,
    "streaming",
    "youtube",
    "https://www.youtube.com/"
);
site!(
    h_stream_netflix,
    "streaming",
    "netflix",
    "https://www.netflix.com/"
);
site!(
    h_stream_disney,
    "streaming",
    "disneyplus",
    "https://www.disneyplus.com/"
);
site!(h_stream_hulu, "streaming", "hulu", "https://www.hulu.com/");
site!(
    h_stream_prime,
    "streaming",
    "prime-video",
    "https://www.primevideo.com/"
);
site!(
    h_stream_twitch,
    "streaming",
    "twitch",
    "https://www.twitch.tv/"
);
site!(
    h_stream_spotify,
    "streaming",
    "spotify",
    "https://open.spotify.com/"
);
site!(h_stream_vimeo, "streaming", "vimeo", "https://vimeo.com/");

// ----- Travel -----
site!(
    h_trav_booking,
    "travel",
    "booking",
    "https://www.booking.com/"
);
site!(h_trav_airbnb, "travel", "airbnb", "https://www.airbnb.com/");
site!(
    h_trav_expedia,
    "travel",
    "expedia",
    "https://www.expedia.com/"
);
site!(h_trav_kayak, "travel", "kayak", "https://www.kayak.com/");
site!(
    h_trav_tripadvisor,
    "travel",
    "tripadvisor",
    "https://www.tripadvisor.com/"
);
site!(h_trav_hotels, "travel", "hotels", "https://www.hotels.com/");
site!(
    h_trav_skyscanner,
    "travel",
    "skyscanner",
    "https://www.skyscanner.com/"
);
site!(h_trav_uber, "travel", "uber", "https://www.uber.com/");

// ----- Real estate -----
site!(
    h_re_zillow,
    "realestate",
    "zillow",
    "https://www.zillow.com/"
);
site!(
    h_re_realtor,
    "realestate",
    "realtor",
    "https://www.realtor.com/"
);
site!(
    h_re_redfin,
    "realestate",
    "redfin",
    "https://www.redfin.com/"
);
site!(
    h_re_trulia,
    "realestate",
    "trulia",
    "https://www.trulia.com/"
);

// ----- Tech -----
site!(h_tech_apple, "tech", "apple", "https://www.apple.com/");
site!(
    h_tech_microsoft,
    "tech",
    "microsoft",
    "https://www.microsoft.com/"
);
site!(
    h_tech_gcloud,
    "tech",
    "google-cloud",
    "https://cloud.google.com/"
);
site!(h_tech_aws, "tech", "aws", "https://aws.amazon.com/");
site!(
    h_tech_azure,
    "tech",
    "azure",
    "https://azure.microsoft.com/"
);
site!(
    h_tech_cloudflare,
    "tech",
    "cloudflare",
    "https://www.cloudflare.com/"
);
site!(h_tech_stripe, "tech", "stripe", "https://stripe.com/");
site!(h_tech_openai, "tech", "openai", "https://openai.com/");
site!(
    h_tech_anthropic,
    "tech",
    "anthropic",
    "https://www.anthropic.com/"
);

// ----- Russian -----
site!(h_ru_yandex, "ru", "yandex-ru", "https://yandex.ru/");
site!(
    h_ru_wildberries,
    "ru",
    "wildberries",
    "https://www.wildberries.ru/"
);
site!(h_ru_ozon, "ru", "ozon", "https://www.ozon.ru/");
site!(h_ru_vk, "ru", "vk", "https://vk.com/");
site!(h_ru_mail, "ru", "mail-ru", "https://mail.ru/");
site!(h_ru_ria, "ru", "ria", "https://ria.ru/");

// ----- Government / banking -----
site!(h_gov_irs, "gov-bank", "irs", "https://www.irs.gov/");
site!(h_gov_usagov, "gov-bank", "usa-gov", "https://www.usa.gov/");
site!(h_gov_chase, "gov-bank", "chase", "https://www.chase.com/");
site!(
    h_gov_bofa,
    "gov-bank",
    "bofa",
    "https://www.bankofamerica.com/"
);
site!(
    h_gov_wf,
    "gov-bank",
    "wellsfargo",
    "https://www.wellsfargo.com/"
);
site!(
    h_gov_paypal,
    "gov-bank",
    "paypal",
    "https://www.paypal.com/"
);

// ----- Antibot test pages -----
site!(
    h_ab_creepjs,
    "antibot",
    "creepjs",
    "https://abrahamjuliot.github.io/creepjs/"
);
site!(
    h_ab_sannysoft,
    "antibot",
    "sannysoft",
    "https://bot.sannysoft.com/"
);
site!(
    h_ab_pixelscan,
    "antibot",
    "pixelscan",
    "https://pixelscan.net/"
);
site!(
    h_ab_arh,
    "antibot",
    "areyouheadless",
    "https://arh.antoinevastel.com/bots/areyouheadless"
);
site!(
    h_ab_botd,
    "antibot",
    "botd",
    "https://fingerprint.com/products/bot-detection/"
);
site!(
    h_ab_fingerprint,
    "antibot",
    "fingerprintscan",
    "https://fingerprint.com/"
);
site!(
    h_ab_browserleaks,
    "antibot",
    "browserleaks-canvas",
    "https://browserleaks.com/canvas"
);
site!(
    h_ab_nowsecure,
    "antibot",
    "nowsecure",
    "https://nowsecure.nl/"
);
site!(h_ab_iphey, "antibot", "iphey", "https://iphey.com/");
site!(
    h_ab_amiunique,
    "antibot",
    "amiunique",
    "https://amiunique.org/"
);

// ----- Previously known CHL -----
site!(
    h_chl_canadagoose,
    "chl-known",
    "canadagoose",
    "https://www.canadagoose.com/"
);
site!(h_chl_hyatt, "chl-known", "hyatt", "https://www.hyatt.com/");
site!(
    h_chl_adidas,
    "chl-known",
    "adidas",
    "https://www.adidas.com/us"
);
site!(
    h_chl_douyin,
    "chl-known",
    "douyin",
    "https://www.douyin.com/"
);
site!(
    h_chl_leboncoin,
    "chl-known",
    "leboncoin",
    "https://www.leboncoin.fr/"
);

// ----- Misc high-traffic -----
site!(h_m_weather, "misc", "weather", "https://weather.com/");
site!(h_m_imdb, "misc", "imdb", "https://www.imdb.com/");
site!(h_m_yelp, "misc", "yelp", "https://www.yelp.com/");
site!(
    h_m_duolingo,
    "misc",
    "duolingo",
    "https://www.duolingo.com/"
);
site!(
    h_m_khan,
    "misc",
    "khanacademy",
    "https://www.khanacademy.org/"
);
site!(
    h_m_coursera,
    "misc",
    "coursera",
    "https://www.coursera.org/"
);
site!(h_m_udemy, "misc", "udemy", "https://www.udemy.com/");
site!(h_m_medium, "misc", "medium", "https://medium.com/");
site!(h_m_substack, "misc", "substack", "https://substack.com/");
site!(h_m_discord, "misc", "discord-com", "https://discord.com/");
site!(h_m_slack, "misc", "slack-com", "https://slack.com/");
site!(h_m_zoom, "misc", "zoom", "https://zoom.us/");

// ============================================================================
// Parallel sweep — same 126-site list driven through `ParallelPager`. Use
// 4 workers (each owns its own JsRuntime) to overlap navigations.
// ============================================================================

fn sites_list() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        ("amazon", "amazon-ca", "https://www.amazon.ca/"),
        ("amazon", "amazon-co-uk", "https://www.amazon.co.uk/"),
        ("amazon", "amazon-com-au", "https://www.amazon.com.au/"),
        ("amazon", "amazon-com", "https://www.amazon.com/"),
        ("amazon", "amazon-de", "https://www.amazon.de/"),
        ("amazon", "amazon-fr", "https://www.amazon.fr/"),
        ("amazon", "amazon-in", "https://www.amazon.in/"),
        ("amazon", "amazon-jp", "https://www.amazon.co.jp/"),
        ("antibot", "amiunique", "https://amiunique.org/"),
        (
            "antibot",
            "areyouheadless",
            "https://arh.antoinevastel.com/bots/areyouheadless",
        ),
        (
            "antibot",
            "botd",
            "https://fingerprint.com/products/bot-detection/",
        ),
        (
            "antibot",
            "browserleaks-canvas",
            "https://browserleaks.com/canvas",
        ),
        (
            "antibot",
            "creepjs",
            "https://abrahamjuliot.github.io/creepjs/",
        ),
        ("antibot", "fingerprintscan", "https://fingerprint.com/"),
        ("antibot", "iphey", "https://iphey.com/"),
        ("antibot", "nowsecure", "https://nowsecure.nl/"),
        ("antibot", "pixelscan", "https://pixelscan.net/"),
        ("antibot", "sannysoft", "https://bot.sannysoft.com/"),
        ("chl-known", "adidas", "https://www.adidas.com/us"),
        ("chl-known", "canadagoose", "https://www.canadagoose.com/"),
        ("chl-known", "douyin", "https://www.douyin.com/"),
        ("chl-known", "hyatt", "https://www.hyatt.com/"),
        ("chl-known", "leboncoin", "https://www.leboncoin.fr/"),
        ("gov-bank", "bofa", "https://www.bankofamerica.com/"),
        ("gov-bank", "chase", "https://www.chase.com/"),
        ("gov-bank", "irs", "https://www.irs.gov/"),
        ("gov-bank", "paypal", "https://www.paypal.com/"),
        ("gov-bank", "usa-gov", "https://www.usa.gov/"),
        ("gov-bank", "wellsfargo", "https://www.wellsfargo.com/"),
        ("misc", "coursera", "https://www.coursera.org/"),
        ("misc", "discord-com", "https://discord.com/"),
        ("misc", "duolingo", "https://www.duolingo.com/"),
        ("misc", "imdb", "https://www.imdb.com/"),
        ("misc", "khanacademy", "https://www.khanacademy.org/"),
        ("misc", "medium", "https://medium.com/"),
        ("misc", "slack-com", "https://slack.com/"),
        ("misc", "substack", "https://substack.com/"),
        ("misc", "udemy", "https://www.udemy.com/"),
        ("misc", "weather", "https://weather.com/"),
        ("misc", "yelp", "https://www.yelp.com/"),
        ("misc", "zoom", "https://zoom.us/"),
        ("news", "bbc", "https://www.bbc.com/"),
        ("news", "bloomberg", "https://www.bloomberg.com/"),
        ("news", "cnn", "https://www.cnn.com/"),
        ("news", "economist", "https://www.economist.com/"),
        ("news", "ft", "https://www.ft.com/"),
        ("news", "guardian", "https://www.theguardian.com/"),
        ("news", "nytimes", "https://www.nytimes.com/"),
        ("news", "reuters", "https://www.reuters.com/"),
        ("news", "washingtonpost", "https://www.washingtonpost.com/"),
        ("news", "wsj", "https://www.wsj.com/"),
        ("realestate", "realtor", "https://www.realtor.com/"),
        ("realestate", "redfin", "https://www.redfin.com/"),
        ("realestate", "trulia", "https://www.trulia.com/"),
        ("realestate", "zillow", "https://www.zillow.com/"),
        ("reference", "github", "https://github.com/"),
        ("reference", "mdn", "https://developer.mozilla.org/"),
        ("reference", "stackoverflow", "https://stackoverflow.com/"),
        (
            "reference",
            "wikipedia-en",
            "https://en.wikipedia.org/wiki/Main_Page",
        ),
        ("reference", "wiktionary", "https://en.wiktionary.org/"),
        ("ru", "mail-ru", "https://mail.ru/"),
        ("ru", "ozon", "https://www.ozon.ru/"),
        ("ru", "ria", "https://ria.ru/"),
        ("ru", "vk", "https://vk.com/"),
        ("ru", "wildberries", "https://www.wildberries.ru/"),
        ("ru", "yandex-ru", "https://yandex.ru/"),
        ("search", "bing", "https://www.bing.com/"),
        ("search", "brave", "https://search.brave.com/"),
        ("search", "duckduckgo", "https://duckduckgo.com/"),
        ("search", "ecosia", "https://www.ecosia.org/"),
        ("search", "google", "https://www.google.com/"),
        ("search", "startpage", "https://www.startpage.com/"),
        ("search", "yahoo", "https://www.yahoo.com/"),
        ("search", "yandex", "https://ya.ru/"),
        ("social", "facebook", "https://www.facebook.com/"),
        ("social", "instagram", "https://www.instagram.com/"),
        ("social", "linkedin", "https://www.linkedin.com/"),
        ("social", "pinterest", "https://www.pinterest.com/"),
        ("social", "quora", "https://www.quora.com/"),
        ("social", "reddit", "https://www.reddit.com/"),
        ("social", "threads", "https://www.threads.net/"),
        ("social", "tumblr", "https://www.tumblr.com/"),
        ("social", "twitter", "https://twitter.com/"),
        ("social", "x-com", "https://x.com/"),
        ("stores", "alibaba", "https://www.alibaba.com/"),
        ("stores", "aliexpress", "https://www.aliexpress.com/"),
        ("stores", "asos", "https://www.asos.com/"),
        ("stores", "bestbuy", "https://www.bestbuy.com/"),
        ("stores", "costco", "https://www.costco.com/"),
        ("stores", "ebay", "https://www.ebay.com/"),
        ("stores", "etsy", "https://www.etsy.com/"),
        ("stores", "h-m", "https://www2.hm.com/"),
        ("stores", "homedepot", "https://www.homedepot.com/"),
        ("stores", "ikea", "https://www.ikea.com/"),
        ("stores", "macys", "https://www.macys.com/"),
        ("stores", "shopify", "https://www.shopify.com/"),
        ("stores", "target", "https://www.target.com/"),
        ("stores", "uniqlo", "https://www.uniqlo.com/"),
        ("stores", "walmart", "https://www.walmart.com/"),
        ("stores", "wayfair", "https://www.wayfair.com/"),
        ("stores", "zara", "https://www.zara.com/"),
        ("streaming", "disneyplus", "https://www.disneyplus.com/"),
        ("streaming", "hulu", "https://www.hulu.com/"),
        ("streaming", "netflix", "https://www.netflix.com/"),
        ("streaming", "prime-video", "https://www.primevideo.com/"),
        ("streaming", "spotify", "https://open.spotify.com/"),
        ("streaming", "twitch", "https://www.twitch.tv/"),
        ("streaming", "vimeo", "https://vimeo.com/"),
        ("streaming", "youtube", "https://www.youtube.com/"),
        ("tech", "anthropic", "https://www.anthropic.com/"),
        ("tech", "apple", "https://www.apple.com/"),
        ("tech", "aws", "https://aws.amazon.com/"),
        ("tech", "azure", "https://azure.microsoft.com/"),
        ("tech", "cloudflare", "https://www.cloudflare.com/"),
        ("tech", "google-cloud", "https://cloud.google.com/"),
        ("tech", "microsoft", "https://www.microsoft.com/"),
        ("tech", "openai", "https://openai.com/"),
        ("tech", "stripe", "https://stripe.com/"),
        ("travel", "airbnb", "https://www.airbnb.com/"),
        ("travel", "booking", "https://www.booking.com/"),
        ("travel", "expedia", "https://www.expedia.com/"),
        ("travel", "hotels", "https://www.hotels.com/"),
        ("travel", "kayak", "https://www.kayak.com/"),
        ("travel", "skyscanner", "https://www.skyscanner.com/"),
        ("travel", "tripadvisor", "https://www.tripadvisor.com/"),
        ("travel", "uber", "https://www.uber.com/"),
    ]
}

/// Classify a fetched page body. Distinguishes:
/// - **L3-RENDERED** — actual content reached the user
/// - **<vendor>-CHL** — interstitial / challenge page from a known vendor
/// - **THIN-BODY** — empty or near-empty redirect dead-end
/// - **BLOCKED** — explicit deny page
///
/// **Important — what is and isn't a challenge marker.** The Akamai BMP
/// `<script src=".../akam/13/<hash>">` bootstrap loads on every
/// Akamai-protected page (rendered or challenge). Its mere presence in
/// the HTML is NOT a challenge indicator — real Chrome sees the same
/// tag on a successfully-loaded walmart.com homepage. The actual
/// challenge signal is **content substitution**: the page body is
/// replaced with an interstitial (≤ 30 KB usually). Same for
/// `_abck`/`_pxhd`/`_kpsdk` — these names appear in inline analytics
/// JS on plenty of normal pages. The canonical interstitial-vs-content
/// discriminator is **body size** + presence of the marker.
///
/// History: a "strong markers" rule that fired on `akam/13` /
/// `_abck` / `_pxhd` regardless of body size used to incorrectly
/// flag walmart, costco, disneyplus, hulu, uniqlo, weather, etc. as
/// Akamai-CHL even when the page rendered with multi-MB content. See
/// `docs/PHASE3_AUDIT_2026_04_29.md` for the per-site contexts that
/// proved the false-positive pattern.
fn classify(html: &str) -> String {
    let lower = html.to_lowercase();
    let len = html.len();

    // Unambiguous interstitial tokens — CSS class names, URL paths, and
    // encoded variables that don't legitimately appear in rendered
    // content. Fire at any body size.
    let unambiguous_titles: &[(&str, &str)] = &[
        ("cf-browser-verification", "Cloudflare-CHL"),
        ("/_sec/cp_challenge", "Akamai-sec-cpt-CHL"),
        ("ddcaptchaencoded", "DataDome-CHL"),
        ("px-captcha", "PerimeterX-CHL"),
    ];
    for (n, t) in unambiguous_titles {
        if lower.contains(n) {
            return t.to_string();
        }
    }
    // English-phrase interstitial markers — these CAN appear in normal
    // page text (article body, embedded loading widgets, cookie banner,
    // privacy policy). Reuters' 1.1 MB rendered home page contains
    // "just a moment" or "checking your browser" somewhere in its
    // content, causing a false-positive Cloudflare-CHL classification
    // when no challenge is actually being served. Only consult these
    // when the body is interstitial-sized.
    let phrase_titles: &[(&str, &str)] = &[
        ("just a moment", "Cloudflare-CHL"),
        ("checking your browser", "Cloudflare-CHL"),
        ("captcha-delivery.com", "DataDome-CHL"),
        ("press &amp; hold", "PerimeterX-PaH"),
        ("pardon our interruption", "Akamai-CHL"),
    ];

    // Vendor "fingerprint markers" — these appear in BOTH normal pages
    // (as analytics/SDK references) and on challenge interstitials. They
    // count as a challenge ONLY when the body is interstitial-sized,
    // i.e. the content was replaced.
    let interstitial_size_threshold = 30 * 1024; // 30 KB
    let small_body_markers: &[(&str, &str)] = &[
        ("akam/13", "Akamai-CHL"),
        ("_abck", "Akamai-CHL"),
        ("_kpsdk", "Kasada-CHL"),
        ("ips.js", "Kasada-CHL"),
        ("_pxhd", "PerimeterX-CHL"),
        ("captcha", "captcha-CHL"),
        ("403 forbidden", "BLOCKED"),
        ("access denied", "BLOCKED"),
    ];
    if len < interstitial_size_threshold {
        for (n, t) in phrase_titles {
            if lower.contains(n) {
                return t.to_string();
            }
        }
        for (n, t) in small_body_markers {
            if lower.contains(n) {
                return t.to_string();
            }
        }
    }

    // "blocked" alone is too noisy — a normal page might use the word.
    // Only count it under the small-body threshold.
    if len < 5 * 1024 && lower.contains("blocked") {
        return "BLOCKED".to_string();
    }

    if len < 1000 {
        return "THIN-BODY".to_string();
    }
    "L3-RENDERED".to_string()
}

#[cfg(test)]
mod classifier_tests {
    use super::classify;

    #[test]
    fn akamai_bootstrap_tag_in_rendered_page_is_not_chl() {
        // walmart.com homepage shape — multi-KB body containing the
        // legitimate Akamai sensor bootstrap script. This must NOT be
        // classified as Akamai-CHL.
        let mut html = String::from(
            r#"<!doctype html><html><head>
            <script type="text/javascript" src="https://www.walmart.com/akam/13/3e35295b" defer></script>
            </head><body>"#,
        );
        // Pad to over 30KB so we leave the small-body bucket.
        for _ in 0..2000 {
            html.push_str("<p>real content paragraph</p>");
        }
        html.push_str("</body></html>");
        assert!(html.len() > 30 * 1024);
        assert_eq!(classify(&html), "L3-RENDERED");
    }

    #[test]
    fn akamai_interstitial_title_is_chl() {
        let html =
            "<html><head><title>Pardon Our Interruption</title></head><body>...</body></html>";
        assert_eq!(classify(html), "Akamai-CHL");
    }

    #[test]
    fn small_body_with_akam13_is_chl() {
        // <30 KB body that's only the akamai bootstrap and a sentinel form.
        let html = r#"<html><head><script src="/akam/13/abc"></script></head>
            <body><form id="bm-verify"></form></body></html>"#;
        assert!(html.len() < 30 * 1024);
        assert_eq!(classify(html), "Akamai-CHL");
    }

    #[test]
    fn cloudflare_interstitial_is_chl() {
        let html = "<html><body>Just a moment...</body></html>";
        assert_eq!(classify(html), "Cloudflare-CHL");
    }

    #[test]
    fn datadome_interstitial_is_chl() {
        let html = r#"<html><body><script src="https://geo.captcha-delivery.com/captcha/check"></script></body></html>"#;
        assert_eq!(classify(html), "DataDome-CHL");
    }

    #[test]
    fn large_rendered_page_with_just_a_moment_phrase_is_not_chl() {
        // reuters.com shape (2026-05-14): 1.1 MB rendered news home page
        // containing the phrase "just a moment" somewhere in article
        // copy / embedded loading widget text. Pre-fix the classifier
        // flagged this as Cloudflare-CHL on every profile, producing a
        // bogus universal-block tally for reuters.
        let mut html = String::from("<html><body>");
        html.push_str("<p>If you want to know more, give us just a moment to load.</p>");
        for _ in 0..30000 {
            html.push_str("<div>actual news article paragraph</div>");
        }
        html.push_str("</body></html>");
        assert!(html.len() > 100 * 1024);
        assert_eq!(classify(&html), "L3-RENDERED");
    }

    #[test]
    fn large_rendered_page_with_checking_your_browser_phrase_is_not_chl() {
        let mut html = String::from("<html><body>");
        html.push_str("<p>We are checking your browser for compatibility.</p>");
        for _ in 0..30000 {
            html.push_str("<div>news content</div>");
        }
        html.push_str("</body></html>");
        assert!(html.len() > 100 * 1024);
        assert_eq!(classify(&html), "L3-RENDERED");
    }

    #[test]
    fn rendered_page_mentioning_grecaptcha_in_config_is_not_chl() {
        // disneyplus / costco shape — multi-MB body that mentions
        // recaptcha in config metadata. Must classify as L3-RENDERED.
        let mut html = String::from("<html><body>");
        html.push_str(
            r#"<script>window.__CONFIG = {"googleRecaptcha":{"siteKey":"6LfAbcXYZ"}};</script>"#,
        );
        for _ in 0..5000 {
            html.push_str("<div>actual product card content here</div>");
        }
        html.push_str("</body></html>");
        assert!(html.len() > 100 * 1024);
        assert_eq!(classify(&html), "L3-RENDERED");
    }

    #[test]
    fn empty_body_is_thin() {
        assert_eq!(classify("<html></html>"), "THIN-BODY");
    }

    #[test]
    fn medium_body_with_pxhd_substring_is_not_chl() {
        // wayfair shape — 1 MB body with `_pxhd` mentioned in inline
        // PerimeterX SDK code. Should NOT classify as PerimeterX-CHL
        // unless the page is challenge-sized.
        let mut html = String::from("<html><body>");
        html.push_str(r#"<script>window._pxhd = "...some sdk init...";</script>"#);
        for _ in 0..30000 {
            html.push_str("<span>content</span>");
        }
        html.push_str("</body></html>");
        assert!(html.len() > 100 * 1024);
        assert_eq!(classify(&html), "L3-RENDERED");
    }
}

/// Run all 126 sites concurrently across N workers (default 4, override
/// with `BOXIDE_PARALLEL_WORKERS` env var). Output format matches the
/// per-site `holistic-end:` lines for downstream comparison.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
#[ignore]
async fn holistic_sweep_parallel() {
    let n_workers: usize = std::env::var("BOXIDE_PARALLEL_WORKERS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(4);
    let sites = sites_list();
    let total = sites.len();
    eprintln!("\n=== PARALLEL SWEEP — {total} sites × {n_workers} workers ===\n");

    let pager = std::sync::Arc::new(ParallelPager::new(n_workers));
    let started = Instant::now();

    // Fire all jobs; collect via FuturesUnordered so completion order is
    // observed-first.
    use futures_util::stream::{FuturesUnordered, StreamExt};
    let mut fut = FuturesUnordered::new();
    for (cat, name, url) in sites {
        let pager = std::sync::Arc::clone(&pager);
        let cat = cat.to_string();
        let name = name.to_string();
        let url = url.to_string();
        let profile = pick_profile();
        fut.push(async move {
            let r = pager.navigate(url.clone(), profile, 3).await;
            let outcome = if let Some(err) = &r.error {
                format!("ERROR: {err}")
            } else {
                classify(&r.html)
            };
            (
                cat,
                name,
                outcome,
                r.html.len(),
                r.elapsed.as_millis() as u64,
                url,
            )
        });
    }
    while let Some((cat, name, out, len, ms, url)) = fut.next().await {
        eprintln!(
            "holistic-end: {} {} {} {} len={} nav_ms={} drop_ms=0 url={}",
            now_unix_ms(),
            cat,
            name,
            out,
            len,
            ms,
            url
        );
    }
    eprintln!(
        "\n=== PARALLEL SWEEP COMPLETE in {:.1} min ===\n",
        started.elapsed().as_secs_f64() / 60.0
    );
}
