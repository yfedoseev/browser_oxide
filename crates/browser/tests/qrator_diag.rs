use browser::Page;
use std::time::Duration;
use stealth::presets::chrome_130_ru;

#[tokio::test]
async fn debug_qrator_dns_shop() {
    let profile = chrome_130_ru();
    let url = "https://www.dns-shop.ru";

    println!("\n=== QRATOR DIAGNOSTIC: {} ===\n", url);

    // We use a high iteration count to follow the challenge redirect
    let result = Page::navigate(url, profile, 5).await;

    match result {
        Ok(mut page) => {
            println!("Navigation finished. Final URL: {}", page.url());
            println!("Title: {}", page.title());

            // Extract all script logs/errors
            let logs_json: String = page
                .evaluate("JSON.stringify(globalThis.__scriptErrors || [])")
                .unwrap_or_else(|_| "[]".to_string());
            let logs: Vec<String> = deno_core::serde_json::from_str(&logs_json).unwrap_or_default();
            println!("\n[SCRIPT LOGS]:");
            for log in logs {
                println!("  {}", log);
            }

            // Extract missing APIs
            let missing: String = page
                .evaluate("JSON.stringify(window.__missingApis || [])")
                .unwrap_or_else(|_| "[]".to_string());
            println!("\n[MISSING APIs]:\n{}", missing.replace("},{", "}\n{"));

            let wasm: String = page
                .evaluate("typeof WebAssembly")
                .unwrap_or_else(|_| "error".to_string());
            println!("[WebAssembly]: {}", wasm);

            let intl: String = page
                .evaluate(
                    r#"
                JSON.stringify({
                    timezone: Intl.DateTimeFormat().resolvedOptions().timeZone,
                    locale: Intl.DateTimeFormat().resolvedOptions().locale,
                    platform: navigator.platform,
                    appVersion: navigator.appVersion,
                    vendor: navigator.vendor,
                    colorDepth: screen.colorDepth
                })
            "#,
                )
                .unwrap_or_else(|_| "{}".to_string());
            println!("[INTL/ENV]: {}", intl);

            // Extract fetch log
            let fetches: String = page
                .evaluate(
                    r#"
                JSON.stringify((window.__fetchLog || []).map(f => ({
                    m: f.method,
                    u: f.url,
                    s: f.status
                })))
            "#,
                )
                .unwrap_or_else(|_| "[]".to_string());
            println!("\n[PAGE FETCHES]: {}", fetches);

            let content = page.content();
            println!("\nContent length: {} bytes", content.len());
            if content.contains("qrator")
                || content.contains("401")
                || content.contains("Unauthorized")
            {
                println!("STILL BLOCKED by QRATOR (or 401)");
            } else {
                println!(
                    "SUCCESS? content preview: {}",
                    content.chars().take(200).collect::<String>()
                );
            }
        }
        Err(e) => {
            println!("Navigation failed: {}", e);
        }
    }
}
