use browser_oxide::stealth::presets;
use browser_oxide::Page;
use std::time::Duration;

#[tokio::test]
#[ignore] // Requires internet
async fn probe_sannysoft_bot_detection() {
    let profile = presets::chrome_148_ru();
    // navigate() uses the full loop with 5 iterations, including __pendingNavigation follow
    let mut page = Page::navigate("https://bot.sannysoft.com", profile, 5)
        .await
        .unwrap();

    // Wait for the page to finish all its async checks
    tokio::time::sleep(Duration::from_secs(15)).await;

    let result: Result<String, _> = page.evaluate(r#"
        (function() {
            const results = {};
            const tables = document.querySelectorAll('table');
            if (tables.length === 0) {
                return JSON.stringify({error: "No tables found", html: document.body.innerHTML.substring(0, 1000)});
            }
            tables.forEach(table => {
                const rows = table.querySelectorAll('tr');
                rows.forEach(row => {
                    const cells = row.querySelectorAll('td, th');
                    if (cells.length >= 2) {
                        const name = cells[0].innerText.trim().replace(/\n/g, ' ');
                        const val = cells[1].innerText.trim().replace(/\n/g, ' ');
                        if (name && name !== "Test name") {
                            results[name] = val;
                        }
                    }
                });
            });
            const statusItems = document.querySelectorAll('.fp-status-item');
            statusItems.forEach(item => {
                const name = item.querySelector('.fp-status-name')?.innerText.trim();
                const val = item.querySelector('.fp-status-value')?.innerText.trim();
                if (name) results["STATUS_" + name] = val;
            });
            return JSON.stringify(results);
        })()
    "#);

    match result {
        Ok(json) => println!("Sannysoft Results: {}", json),
        Err(e) => {
            let html: String = page.evaluate("document.body.innerHTML").unwrap_or_default();
            let errors: String = page
                .evaluate("JSON.stringify(window.__errors || [])")
                .unwrap_or_default();
            println!(
                "Sannysoft extraction failed: {}. Captured Errors: {}. Body HTML: {}",
                e, errors, html
            );
        }
    }
}

#[tokio::test]
#[ignore] // Requires internet
async fn probe_creepjs_score() {
    let profile = presets::chrome_148_ru();
    // CreepJS is heavy and takes time to compute
    let mut page = Page::with_profile("", "https://abrahamjuliot.github.io/creepjs/", profile)
        .await
        .unwrap();

    println!("Waiting for CreepJS to finish (30s)...");
    tokio::time::sleep(Duration::from_secs(30)).await;

    let result: String = page
        .evaluate(
            r#"
        (function() {
            const results = {};
            try {
                results.trustScore = document.querySelector('.trust-score')?.innerText;
                results.botStatus = document.querySelector('.bot-status')?.innerText;
                results.fingerprint = document.querySelector('.fingerprint-header')?.innerText;
                
                // Get all "failed" or "warning" points
                const alerts = [];
                document.querySelectorAll('.alert, .warning').forEach(el => {
                    alerts.push(el.innerText.trim());
                });
                results.alerts = alerts;
            } catch (e) {
                results.error = e.message;
            }
            return JSON.stringify(results);
        })()
    "#,
        )
        .unwrap();

    println!("CreepJS Results: {}", result);
}

#[tokio::test]
#[ignore] // Requires internet
async fn probe_browser_scan() {
    let profile = presets::chrome_148_ru();
    let mut page = Page::with_profile("", "https://www.browserscan.net/bot-detection", profile)
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_secs(10)).await;

    let result: String = page
        .evaluate(
            r#"
        (function() {
            const results = {};
            // Look for bot detection indicators
            const items = document.querySelectorAll('.detection-item');
            items.forEach(item => {
                const label = item.querySelector('.label')?.innerText;
                const status = item.querySelector('.status')?.innerText;
                if (label) results[label] = status;
            });
            return JSON.stringify(results);
        })()
    "#,
        )
        .unwrap();

    println!("BrowserScan Results: {}", result);
}
