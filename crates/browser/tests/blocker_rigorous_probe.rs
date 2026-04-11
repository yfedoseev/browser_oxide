//! Rigorous re-probe of the known-problematic sites after the T1.3 + Workers
//! + humanize work, using content markers (not just status codes) to decide
//! pass/fail. The anti_bot_sites.rs probe is too lenient — it labels
//! `200 + 2351-byte interstitial` as PASS. This suite uses:
//!
//! - **Size threshold**: the real page body must be >= min_body_size (bot
//!   interstitials are always < 5 KB for these sites; real pages are 100 KB+).
//! - **Positive marker**: a string that only appears on the real page
//!   (product-listing HTML, brand name in title, etc.).
//! - **Negative marker**: strings that only appear on the challenge page
//!   (`sec-if-cpt-container`, `Pardon Our Interruption`, `Reference Error`).
//!
//! Runs against both the rquest-only path (`client.get`) and the full
//! `Page::navigate_with_challenges` path so we can see whether the challenge
//! solver helps on top of the baseline.
//!
//! Run:
//!   cargo test -p browser --test blocker_rigorous_probe -- \
//!     --ignored --test-threads=1 --nocapture

use browser::Page;
use net::HttpClient;
use stealth::StealthProfile;

#[derive(Debug)]
struct BlockerProbeResult {
    name: &'static str,
    url: &'static str,
    protection: &'static str,
    baseline_status: u16,
    baseline_size: usize,
    baseline_verdict: Verdict,
    solver_status: u16,
    solver_size: usize,
    solver_verdict: Verdict,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Verdict {
    /// Real page loaded (positive marker present, no negative markers).
    Pass,
    /// Challenge page / interstitial.
    Interstitial,
    /// Hard block / WAF error page.
    Block,
    /// Network/client error.
    Error,
}

impl Verdict {
    fn symbol(self) -> &'static str {
        match self {
            Verdict::Pass => "PASS",
            Verdict::Interstitial => "INTR",
            Verdict::Block => "BLOCK",
            Verdict::Error => "ERR ",
        }
    }
}

impl BlockerProbeResult {
    fn print(&self) {
        println!(
            "[{:<5}] baseline={} ({}b) solver={} ({}b)  {:<14} {} — {}",
            if matches!(self.solver_verdict, Verdict::Pass) {
                "WIN"
            } else if matches!(self.baseline_verdict, Verdict::Pass) {
                "BASE"
            } else {
                "FAIL"
            },
            self.baseline_verdict.symbol(),
            self.baseline_size,
            self.solver_verdict.symbol(),
            self.solver_size,
            self.protection,
            self.name,
            self.url
        );
    }
}

fn classify(body: &str, status: u16, positive: &[&str], negative: &[&str], min_size: usize) -> Verdict {
    if status == 0 {
        return Verdict::Error;
    }
    // Fast path: a body much smaller than the min_size and containing
    // known interstitial/block markers is definitely a challenge page.
    let small_body = body.len() < min_size.min(10_000);
    let has_negative = negative.iter().any(|m| body.contains(m));
    if small_body && has_negative {
        // Hard blocks vs interstitials — the former have reference errors.
        for marker in negative {
            if body.contains(marker)
                && (marker.contains("Reference Error")
                    || marker.contains("WAFfailover")
                    || marker.contains("Access Denied"))
            {
                return Verdict::Block;
            }
        }
        return Verdict::Interstitial;
    }
    if small_body {
        return Verdict::Interstitial;
    }
    // Body is large enough. Check for negative markers first — a large
    // body can still be an interstitial that includes wrapper DOM.
    if has_negative {
        return Verdict::Interstitial;
    }
    // Large body, no negative markers. If any positive marker matches,
    // definite PASS. Otherwise still likely PASS because interstitials
    // are always small — just mark it.
    if positive.iter().any(|m| body.contains(m)) {
        return Verdict::Pass;
    }
    // Large body (>min_size), no negative markers, no positive markers.
    // Most likely a real page that uses different terminology than our
    // markers. Trust the size.
    if body.len() > 50_000 {
        return Verdict::Pass;
    }
    Verdict::Interstitial
}

async fn probe_site(
    name: &'static str,
    url: &'static str,
    protection: &'static str,
    profile: StealthProfile,
    positive: &[&str],
    negative: &[&str],
    min_size: usize,
) -> BlockerProbeResult {
    // Baseline: rquest-only path, no JS challenge execution.
    let client = HttpClient::new(&profile).unwrap();
    let (baseline_status, baseline_size, baseline_verdict) = match client.get(url).await {
        Ok(resp) => {
            let body = resp.text();
            let status = resp.status;
            let size = body.len();
            let v = classify(&body, status, positive, negative, min_size);
            (status, size, v)
        }
        Err(_) => (0, 0, Verdict::Error),
    };

    // Solver path: Page::navigate runs the full challenge flow,
    // following `__pendingNavigation` set by challenge scripts for
    // up to 5 iterations (initial fetch + solver run + re-navigate
    // + headroom for chained challenges). The old value of `1` was
    // a stale migration from `navigate_with_challenges(url, 1)`
    // which meant "1 retry on top of initial fetch"; the new API's
    // `1` means "1 total fetch" and caps out before the solver can
    // re-navigate to the real page.
    let (solver_status, solver_size, solver_verdict) =
        match Page::navigate(url, profile, 5).await {
            Ok(mut page) => {
                let body = page.content();
                let size = body.len();
                let v = classify(&body, 200, positive, negative, min_size);
                (200, size, v)
            }
            Err(_) => (0, 0, Verdict::Error),
        };

    BlockerProbeResult {
        name,
        url,
        protection,
        baseline_status,
        baseline_size,
        baseline_verdict,
        solver_status,
        solver_size,
        solver_verdict,
    }
}

#[tokio::test]
#[ignore]
async fn tier05_blockers_all() {
    let mut results: Vec<BlockerProbeResult> = Vec::new();

    // Akamai BMP v3 blockers
    results.push(
        probe_site(
            "adidas",
            "https://www.adidas.com/us",
            "akamai-bmp-v3",
            stealth::chrome_130_macos(),
            &["adidas-us", "product-card", "utag_data", "Sneakers and Activewear"],
            &[
                "sec-if-cpt-container",
                "Pardon Our Interruption",
                "Reference Error",
                "WAFfailover",
            ],
            50_000,
        )
        .await,
    );
    results.push(
        probe_site(
            "homedepot",
            "https://www.homedepot.com/",
            "akamai-bmp-v3",
            stealth::chrome_130_windows(),
            &["homedepot", "product", "Home Depot"],
            &[
                "sec-if-cpt-container",
                "Pardon Our Interruption",
                "Reference Error",
                "Access Denied",
            ],
            50_000,
        )
        .await,
    );

    // Kasada blockers
    results.push(
        probe_site(
            "canadagoose",
            "https://www.canadagoose.com/us/en/home-page",
            "kasada",
            stealth::chrome_130_windows(),
            &["Canada Goose", "product", "shop"],
            &["x-kpsdk", "KPSDK", "403", "ips.js"],
            50_000,
        )
        .await,
    );
    results.push(
        probe_site(
            "hyatt",
            "https://www.hyatt.com/",
            "kasada",
            stealth::chrome_130_windows(),
            &["Hyatt", "hotel", "book"],
            &["x-kpsdk", "KPSDK", "Access denied"],
            50_000,
        )
        .await,
    );

    // Russian sites / QRATOR / WBAAS
    results.push(
        probe_site(
            "wildberries",
            "https://www.wildberries.ru/",
            "wbaas",
            stealth::presets::chrome_130_ru(),
            &["wildberries", "Wildberries", "товар"],
            &["challenge_fingerprint", "x-wbaas-token", "QRATOR"],
            80_000,
        )
        .await,
    );
    results.push(
        probe_site(
            "dns_shop",
            "https://www.dns-shop.ru/",
            "qrator",
            stealth::presets::chrome_130_ru(),
            &["dns-shop", "DNS", "каталог"],
            &["QRATOR", "Rate limit", "blocked"],
            80_000,
        )
        .await,
    );
    results.push(
        probe_site(
            "ozon",
            "https://www.ozon.ru/",
            "ddos-guard",
            stealth::presets::chrome_130_ru(),
            &["ozon", "Ozon", "товар"],
            &["ddos-guard", "challenge", "cf-chl"],
            80_000,
        )
        .await,
    );
    results.push(
        probe_site(
            "yandex",
            "https://ya.ru/",
            "smartcaptcha",
            stealth::presets::chrome_130_ru(),
            // Yandex-specific markers that appear on the real search
            // homepage but NOT on the SmartCaptcha interstitial. These
            // are more stable than the raw "ya.ru"/"yandex" literals,
            // which can collide with the interstitial's own branding.
            //
            // Yandex uses BEM class conventions — `data-bem` is a
            // reliable signal that the server returned the real
            // component-rendered page rather than an error stub.
            &["data-bem", "yandex-verification", "homer"],
            &["SmartCaptcha", "smart-captcha", "\"captcha\""],
            30_000,
        )
        .await,
    );

    println!("\n=== Tier 0.5 Blocker Re-Probe Results ===\n");
    let total = results.len();
    let mut wins = 0usize;
    let mut base_only = 0usize;
    let mut fails = 0usize;
    for r in &results {
        r.print();
        match r.solver_verdict {
            Verdict::Pass => wins += 1,
            _ if r.baseline_verdict == Verdict::Pass => base_only += 1,
            _ => fails += 1,
        }
    }
    println!();
    println!("Summary: {wins}/{total} solver-PASS, {base_only} baseline-only PASS, {fails} FAIL");
}
