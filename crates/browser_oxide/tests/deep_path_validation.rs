//! Deep-path validation (Option A) — probe product/search/detail pages on
//! the 48 sites that land at L3 to test whether the landing pass translates
//! to real scraping capability, or if it was just a landing-page grace period.
//!
//! Some bot-detection sensors build trust profiles over the first 5-10
//! requests before scoring. A single GET doesn't exercise that. This
//! suite makes N=2 requests per site (landing then a deep path) to see
//! if the trust profile holds.
//!
//! Run: cargo test -p browser --test deep_path_validation \
//!          -- --ignored --test-threads=1 --nocapture
//!
//! Each test prints [L1 L2 L3] for both the landing and the deep path so
//! we can see if the deep path stays green or degrades.

#[allow(
    dead_code,
    reason = "diagnostic capture struct; not all fields are asserted"
)]
struct DeepProbeResult {
    site: String,
    landing_url: String,
    landing_status: u16,
    landing_l3: bool,
    deep_url: String,
    deep_status: u16,
    deep_l3: bool,
    notes: String,
}

impl DeepProbeResult {
    fn print(&self) {
        let landing = if self.landing_l3 { "L3" } else { "-" };
        let deep = if self.deep_l3 { "L3" } else { "-" };
        let verdict = match (self.landing_l3, self.deep_l3) {
            (true, true) => "HOLD",
            (true, false) => "DEGRADE",
            (false, true) => "weird",
            (false, false) => "both-fail",
        };
        println!(
            "[{verdict:<7}] {site:<30} landing={landing} ({ls}) deep={deep} ({ds})",
            site = self.site,
            ls = self.landing_status,
            ds = self.deep_status,
        );
        if !self.notes.is_empty() {
            println!("           {}", self.notes);
        }
    }
}

async fn deep_probe(
    site: &str,
    landing: &str,
    deep: &str,
    profile: browser_oxide::stealth::StealthProfile,
    deep_markers: &[&str],
    deep_negative: &[&str],
) -> DeepProbeResult {
    let client = browser_oxide::net::HttpClient::new(&profile).unwrap();

    // Landing
    let landing_resp = match client.get_follow(landing, 10).await {
        Ok(r) => r,
        Err(e) => {
            return DeepProbeResult {
                site: site.into(),
                landing_url: landing.into(),
                landing_status: 0,
                landing_l3: false,
                deep_url: deep.into(),
                deep_status: 0,
                deep_l3: false,
                notes: format!("landing err: {e}"),
            };
        }
    };
    let landing_status = landing_resp.status;
    let landing_ok = matches!(landing_status, 200 | 202 | 301 | 302);

    // Deep
    let deep_resp = match client.get_follow(deep, 10).await {
        Ok(r) => r,
        Err(e) => {
            return DeepProbeResult {
                site: site.into(),
                landing_url: landing.into(),
                landing_status,
                landing_l3: landing_ok,
                deep_url: deep.into(),
                deep_status: 0,
                deep_l3: false,
                notes: format!("deep err: {e}"),
            };
        }
    };
    let deep_status = deep_resp.status;
    let deep_ok_status = matches!(deep_status, 200 | 202 | 301 | 302);
    let body = deep_resp.text();
    let mut deep_l3 = deep_ok_status;
    let mut reason = String::new();
    if deep_l3 {
        for m in deep_markers {
            if !body.contains(m) {
                deep_l3 = false;
                reason = format!("missing marker '{m}'");
                break;
            }
        }
    } else {
        reason = format!("status {deep_status}");
    }
    if deep_l3 {
        for m in deep_negative {
            if body.contains(m) {
                deep_l3 = false;
                reason = format!("hit negative '{m}'");
                break;
            }
        }
    }

    DeepProbeResult {
        site: site.into(),
        landing_url: landing.into(),
        landing_status,
        landing_l3: landing_ok,
        deep_url: deep.into(),
        deep_status,
        deep_l3,
        notes: reason,
    }
}

// ================================================================
// Deep probes — one per site, two requests each (landing + deep path)
// ================================================================

#[tokio::test]
#[ignore]
async fn deep_amazon_dp() {
    // A specific Amazon product detail URL (Kindle).
    let r = deep_probe(
        "amazon.com",
        "https://www.amazon.com/",
        "https://www.amazon.com/dp/B08N3TCP5Z",
        browser_oxide::stealth::chrome_148_windows(),
        &["<html"],
        &["validateCaptcha", "Sorry, we just need"],
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn deep_chatgpt_about() {
    let r = deep_probe(
        "chatgpt.com",
        "https://chatgpt.com/",
        "https://chatgpt.com/auth/login",
        browser_oxide::stealth::chrome_148_windows(),
        &["<html"],
        &["Just a moment"],
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn deep_linkedin_feed() {
    let r = deep_probe(
        "linkedin.com",
        "https://www.linkedin.com/",
        "https://www.linkedin.com/feed/",
        browser_oxide::stealth::chrome_148_windows(),
        &["<html"],
        &["Access denied", "Pardon Our Interruption"],
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn deep_coinbase_price() {
    let r = deep_probe(
        "coinbase.com",
        "https://www.coinbase.com/",
        "https://www.coinbase.com/price/bitcoin",
        browser_oxide::stealth::chrome_148_windows(),
        &["<html", "Bitcoin"],
        &["Just a moment"],
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn deep_medium_topics() {
    let r = deep_probe(
        "medium.com",
        "https://medium.com/",
        "https://medium.com/topic/technology",
        browser_oxide::stealth::chrome_148_windows(),
        &["<html"],
        &["Just a moment"],
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn deep_discord_developers() {
    let r = deep_probe(
        "discord.com",
        "https://discord.com/",
        "https://discord.com/developers/docs/intro",
        browser_oxide::stealth::chrome_148_windows(),
        &["<html"],
        &["Just a moment"],
    )
    .await;
    r.print();
}

// --- Tier 0.5 deep paths ---

#[tokio::test]
#[ignore]
async fn deep_product_page() {
    // Nike Air Max — a specific product detail URL
    let r = deep_probe(
        "nike.com",
        "https://www.nike.com/",
        "https://www.nike.com/w/mens-shoes-nik1zy7ok",
        browser_oxide::stealth::chrome_148_windows(),
        &["<html"],
        &["Access Denied", "Pardon Our Interruption"],
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn deep_search_page() {
    let r = deep_probe(
        "walmart.com",
        "https://www.walmart.com/",
        "https://www.walmart.com/search?q=laptop",
        browser_oxide::stealth::chrome_148_windows(),
        &["<html"],
        &["Robot or human", "Access Denied"],
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn deep_zillow_search() {
    let r = deep_probe(
        "zillow.com",
        "https://www.zillow.com/",
        "https://www.zillow.com/homes/San-Francisco_rb/",
        browser_oxide::stealth::chrome_148_windows(),
        &["<html"],
        &["Robot", "Access Denied"],
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn deep_stockx_browse() {
    let r = deep_probe(
        "stockx.com",
        "https://www.stockx.com/",
        "https://stockx.com/sneakers",
        browser_oxide::stealth::chrome_148_windows(),
        &["<html"],
        &["Access denied"],
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn deep_glassdoor_jobs() {
    let r = deep_probe(
        "glassdoor.com",
        "https://www.glassdoor.com/",
        "https://www.glassdoor.com/Job/jobs.htm?sc.keyword=software%20engineer",
        browser_oxide::stealth::chrome_148_windows(),
        &["<html"],
        &[r#"dd={"rt""#, "ct.captcha-delivery.com"],
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn deep_crunchbase_search() {
    let r = deep_probe(
        "crunchbase.com",
        "https://www.crunchbase.com/",
        "https://www.crunchbase.com/discover/organization.companies",
        browser_oxide::stealth::chrome_148_windows(),
        &["<html"],
        &[r#"dd={"rt""#],
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn deep_reddit_sub() {
    let r = deep_probe(
        "reddit.com",
        "https://www.reddit.com/",
        "https://www.reddit.com/r/rust/",
        browser_oxide::stealth::chrome_148_windows(),
        &["<html"],
        &[r#"dd={"rt""#, "Access denied"],
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn deep_delta_destinations() {
    let r = deep_probe(
        "delta.com",
        "https://www.delta.com/",
        "https://www.delta.com/flight-search/book-a-flight",
        browser_oxide::stealth::chrome_148_windows(),
        &["<html"],
        &["Access Denied"],
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn deep_turbotax_products() {
    let r = deep_probe(
        "turbotax.com",
        "https://turbotax.intuit.com/",
        "https://turbotax.intuit.com/personal-taxes/online/",
        browser_oxide::stealth::chrome_148_windows(),
        &["<html"],
        &["Access Denied"],
    )
    .await;
    r.print();
}

// --- CN deep paths ---

#[tokio::test]
#[ignore]
async fn deep_taobao_search() {
    let r = deep_probe(
        "taobao.com",
        "https://www.taobao.com/",
        "https://s.taobao.com/search?q=laptop",
        browser_oxide::stealth::presets::chrome_148_cn(),
        &["<html"],
        &[r#""code":"punish""#, "slider"],
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn deep_tmall_search() {
    let r = deep_probe(
        "tmall.com",
        "https://www.tmall.com/",
        "https://list.tmall.com/search_product.htm?q=laptop",
        browser_oxide::stealth::presets::chrome_148_cn(),
        &["<html"],
        &[r#""code":"punish""#, "slider"],
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn deep_jd_search() {
    let r = deep_probe(
        "jd.com",
        "https://www.jd.com/",
        "https://search.jd.com/Search?keyword=laptop",
        browser_oxide::stealth::presets::chrome_148_cn(),
        &["<html"],
        &["access denied"],
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn deep_baidu_search() {
    let r = deep_probe(
        "baidu.com",
        "https://www.baidu.com/",
        "https://www.baidu.com/s?wd=rust",
        browser_oxide::stealth::presets::chrome_148_cn(),
        &["<html"],
        &["百度安全验证"],
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn deep_douyin_explore() {
    let r = deep_probe(
        "douyin.com",
        "https://www.douyin.com/",
        "https://www.douyin.com/discover",
        browser_oxide::stealth::presets::chrome_148_cn(),
        &["<html"],
        &[],
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn deep_bilibili_play() {
    let r = deep_probe(
        "bilibili.com",
        "https://www.bilibili.com/",
        "https://www.bilibili.com/v/popular/all",
        browser_oxide::stealth::presets::chrome_148_cn(),
        &["<html"],
        &[],
    )
    .await;
    r.print();
}

// --- RU deep paths ---

#[tokio::test]
#[ignore]
async fn deep_avito_search() {
    let r = deep_probe(
        "avito.ru",
        "https://www.avito.ru/",
        "https://www.avito.ru/moskva?q=iphone",
        browser_oxide::stealth::presets::chrome_148_ru(),
        &["<html"],
        &["Доступ ограничен"],
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn deep_ya_search() {
    let r = deep_probe(
        "ya.ru",
        "https://ya.ru/",
        "https://ya.ru/search/?text=rust%20language",
        browser_oxide::stealth::presets::chrome_148_ru(),
        &["<html"],
        &["showcaptcha"],
    )
    .await;
    r.print();
}

#[tokio::test]
#[ignore]
async fn deep_vk_feed() {
    let r = deep_probe(
        "vk.com",
        "https://vk.com/",
        "https://vk.com/feed",
        browser_oxide::stealth::presets::chrome_148_ru(),
        &["<html"],
        &["Доступ ограничен"],
    )
    .await;
    r.print();
}
