use browser::Page;
use std::time::Duration;
use stealth::StealthProfile;

#[derive(Debug, PartialEq)]
enum Verdict {
    Pass,
    Intr,
    Block,
    Error,
}

struct BlockerProbeResult {
    name: String,
    url: String,
    protection: String,
    baseline_status: u16,
    baseline_size: usize,
    baseline_verdict: Verdict,
    solver_status: u16,
    solver_size: usize,
    solver_verdict: Verdict,
}

impl BlockerProbeResult {
    fn print(&self) {
        let baseline_str = format!("{:?} ({}b)", self.baseline_verdict, self.baseline_size);
        let solver_str = format!("{:?} ({}b)", self.solver_verdict, self.solver_size);
        let status = if self.solver_verdict == Verdict::Pass {
            "[WIN  ]"
        } else {
            "[FAIL ]"
        };
        println!(
            "{} baseline={:<12} solver={:<12}  {:<14} {} — {}",
            status, baseline_str, solver_str, self.protection, self.name, self.url
        );
    }
}

fn classify(
    body: &str,
    status: u16,
    positive: &[&str],
    negative: &[&str],
    min_size: usize,
) -> Verdict {
    if status >= 400 && status != 403 && status != 429 {
        return Verdict::Error;
    }

    // Priority 1: Large body size usually means we reached the real content,
    // even if some challenge keywords are still present in the DOM.
    if body.len() >= min_size {
        return Verdict::Pass;
    }

    // Priority 2: Negative markers on small bodies
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
        return Verdict::Intr;
    }

    // Priority 3: Positive markers
    // If we are small and don't have negative markers, check
    // positive markers. If any positive marker matches,
    // definite PASS. Otherwise still likely PASS because interstitials
    // are always small — just mark it.
    if positive.iter().any(|m| body.contains(m)) {
        return Verdict::Pass;
    }

    if body.is_empty() {
        return Verdict::Error;
    }

    Verdict::Intr
}

async fn probe_site(
    name: &str,
    url: &str,
    protection: &str,
    profile: StealthProfile,
    positive: &[&str],
    negative: &[&str],
    min_size: usize,
) -> BlockerProbeResult {
    // Baseline path:navigate_simple does not execute JS.
    // It only gets the raw HTML from the server.
    let client = net::HttpClient::new(&profile).unwrap();
    let (baseline_status, baseline_size, baseline_verdict) = match client.get_follow(url, 10).await
    {
        Ok(resp) => {
            let body = resp.text();
            let size = body.len();
            let v = classify(&body, resp.status, positive, negative, min_size);
            (resp.status, size, v)
        }
        Err(_) => (0, 0, Verdict::Error),
    };

    // Solver path: Page::navigate runs the full challenge flow,
    // following `__pendingNavigation` set by challenge scripts for
    // up to 5 iterations.
    let (solver_status, solver_size, solver_verdict) = match Page::navigate(url, profile, 5).await {
        Ok(mut page) => {
            let body = page.content();
            let size = body.len();
            let v = classify(&body, 200, positive, negative, min_size);
            (200, size, v)
        }
        Err(_) => (0, 0, Verdict::Error),
    };

    BlockerProbeResult {
        name: name.to_string(),
        url: url.to_string(),
        protection: protection.to_string(),
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
            &[
                "adidas-us",
                "product-card",
                "utag_data",
                "Sneakers and Activewear",
            ],
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
