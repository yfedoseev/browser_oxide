//! Fingerprint reference scorers — probe browser_oxide against CreepJS,
//! bot.incolumitas, pixelscan, amiunique, and Vastel's areyouheadless test.
//!
//! These sites render scores via JS, so we use `navigate_with_challenges`
//! which runs the full challenge/script pipeline, then extract the score
//! from the rendered DOM via `page.evaluate()`.
//!
//! Run with:
//!   cargo test -p browser --test fingerprint_scorers \
//!       -- --ignored --test-threads=1 --nocapture
//!
//! The assertions here are intentionally loose — our first run is a
//! measurement, not a regression test. The ACTUAL VALUE is logged so we
//! can establish baseline scores and track them over time.

use browser::Page;
use std::time::Duration;

/// Load a URL through the full browser pipeline (network fetch + JS
/// execution + challenge handling) and wait a bit for scripts to run.
async fn load_and_wait(url: &str, profile: stealth::StealthProfile, wait_ms: u64) -> Page {
    let mut page = match Page::navigate(url, profile, 2).await {
        Ok(p) => p,
        Err(e) => panic!("navigate_with_challenges failed: {e}"),
    };
    // Pump the event loop to let async scoring run.
    let _ = page
        .evaluate_async("void 0", Duration::from_millis(wait_ms))
        .await;
    page
}

// ================================================================
// Scorer probes — each one loads the scorer and extracts its verdict.
// ================================================================

#[tokio::test]
#[ignore]
async fn scorer_vastel_fetch_script() {
    // Fetch areuheadless.js directly to see what test it runs
    let profile = stealth::chrome_130_windows();
    let client = net::HttpClient::new(&profile).unwrap();
    let resp = client
        .get_follow(
            "https://arh.antoinevastel.com/javascripts/areuheadless.js",
            3,
        )
        .await
        .unwrap();
    println!("=== areuheadless.js ===");
    println!("status: {} body_len: {}", resp.status, resp.body.len());
    let body = resp.text();
    println!("{body}");
}

#[tokio::test]
#[ignore]
async fn scorer_vastel_raw_html() {
    // Fetch the raw HTML to understand the default vs JS-updated text.
    let profile = stealth::chrome_130_windows();
    let client = net::HttpClient::new(&profile).unwrap();
    let resp = client
        .get_follow("https://arh.antoinevastel.com/bots/areyouheadless", 5)
        .await
        .unwrap();
    println!("=== Vastel raw HTML ===");
    println!("status: {} body_len: {}", resp.status, resp.body.len());
    let body = resp.text();
    // Look for the verdict element
    for line in body.lines() {
        let l = line.trim();
        if l.contains("headless") || l.contains("You are") {
            if l.len() < 300 {
                println!("  line: {l}");
            }
        }
    }
    // Grep for the detection script
    if let Some(idx) = body.find("<script") {
        let end = body[idx..]
            .find("</script>")
            .map(|e| idx + e)
            .unwrap_or(idx + 500);
        println!("first script tag:\n{}", &body[idx..end.min(body.len())]);
    }
}

#[tokio::test]
#[ignore]
async fn scorer_vastel_are_you_headless() {
    let profile = stealth::chrome_130_windows();
    let mut page = load_and_wait(
        "https://arh.antoinevastel.com/bots/areyouheadless",
        profile,
        10000,
    )
    .await;
    let title = page.title();
    // Query the #res div specifically — that's where Vastel's JS writes the
    // actual verdict. The default innerHTML is "You are Chrome headless".
    let res_text = page
        .evaluate(
            r#"(() => {
            const el = document.querySelector('#res');
            return el ? el.textContent : '(no #res)';
        })()"#,
        )
        .unwrap_or_default();
    println!("\n=== Vastel AreYouHeadless ===");
    println!("#res content: {res_text}");
    // Also check if the script ran by looking at window.FpCollect or similar
    let script_ran = page
        .evaluate(r#"JSON.stringify({
            fpCollect: typeof fpCollect,
            hasIsHeadless: typeof window.isHeadless,
            hasGlobalResult: typeof window.result,
            scripts: Array.from(document.getElementsByTagName('script')).map(s => s.src || '(inline)'),
            errors: window.__scriptErrors || [],
        })"#)
        .unwrap_or_default();
    println!("script state: {script_ran}");
    let text = page.text_content();
    let verdict = if text.contains("You are not Chrome headless") {
        "NOT HEADLESS ✓"
    } else if text.contains("You are Chrome headless") {
        "DETECTED ✗"
    } else if text.is_empty() {
        "EMPTY_BODY"
    } else {
        "UNKNOWN"
    };
    println!("\n=== Vastel AreYouHeadless ===");
    println!("title: {title}");
    println!("verdict: {verdict}");
    let preview: String = text.chars().take(500).collect();
    println!("body first 500 chars: {preview}");
    // Run the same checks Vastel's script runs so we can see which one fails
    let checks = page
        .evaluate(
            r#"JSON.stringify({
            webdriver_undefined: typeof navigator.webdriver === 'undefined',
            webdriver_false: navigator.webdriver === false,
            webdriver_value: String(navigator.webdriver),
            chrome_present: typeof window.chrome !== 'undefined',
            chrome_runtime: typeof window.chrome?.runtime !== 'undefined',
            languages_len: navigator.languages?.length || 0,
            plugins_len: navigator.plugins?.length || 0,
            permissions_obj: typeof navigator.permissions,
            notification_perm: (() => {
                try { return Notification.permission; } catch(e) { return 'ERR'; }
            })(),
            permission_check: typeof navigator.permissions?.query === 'function',
            platform: navigator.platform,
        })"#,
        )
        .unwrap_or_default();
    println!("checks: {checks}");
}

#[tokio::test]
#[ignore]
async fn scorer_vastel_bot_tests() {
    let profile = stealth::chrome_130_windows();
    let mut page = load_and_wait("https://arh.antoinevastel.com/bots", profile, 3000).await;
    let title = page.title();
    let text = page.text_content();
    println!("\n=== Vastel BotTests ===");
    println!("title: {title}");
    println!("body len: {}", text.len());
    // Extract any "PASS"/"FAIL" text rows
    for line in text.lines() {
        let l = line.trim();
        if !l.is_empty() && (l.contains("PASS") || l.contains("FAIL")) {
            println!("  row: {l}");
        }
    }
}

#[tokio::test]
#[ignore]
async fn scorer_bot_incolumitas() {
    // bot.incolumitas is a comprehensive bot detection harness with dozens
    // of individual checks. It renders a score of the form "X / Y" in the
    // page body.
    let profile = stealth::chrome_130_windows();
    let mut page = load_and_wait("https://bot.incolumitas.com/", profile, 5000).await;
    let title = page.title();
    let text = page.text_content();
    println!("\n=== bot.incolumitas.com ===");
    println!("title: {title}");
    println!("body len: {}", text.len());
    // Look for the score pattern
    let score = page
        .evaluate(
            r#"(() => {
                // Common selectors the page uses to render scores
                const el = document.querySelector('#botScore, .bot-score, [data-testid="score"], .score');
                return el ? el.textContent : null;
            })()"#,
        )
        .unwrap_or_default();
    println!("score element: {score}");
    // Fallback: grep the body text for a "score" number
    for line in text.lines() {
        let l = line.trim();
        if l.to_lowercase().contains("score") || l.to_lowercase().contains("bot") {
            if l.len() < 200 && !l.is_empty() {
                println!("  {l}");
            }
        }
    }
}

#[tokio::test]
#[ignore]
async fn scorer_creepjs() {
    // CreepJS is the most comprehensive fingerprint scorer. It renders
    // a "Trust Score" as a percentage in a prominent div. The site also
    // checks ~200 fingerprint signals and flags "lies" (patched APIs).
    let profile = stealth::chrome_130_windows();
    let mut page = load_and_wait(
        "https://abrahamjuliot.github.io/creepjs/",
        profile,
        8000, // CreepJS takes a while to compute
    )
    .await;
    let title = page.title();
    println!("\n=== CreepJS ===");
    println!("title: {title}");

    // Try several likely score selectors
    let trust_score = page
        .evaluate(
            r#"(() => {
                const candidates = [
                    '.trust-score',
                    '#trust-score',
                    '[data-trust-score]',
                    '.trust .score',
                    '.creep',
                    '.fp-hash',
                    '.trusted',
                ];
                const found = {};
                for (const sel of candidates) {
                    const el = document.querySelector(sel);
                    if (el) found[sel] = el.textContent.trim().substring(0, 100);
                }
                return JSON.stringify(found);
            })()"#,
        )
        .unwrap_or_default();
    println!("candidates: {trust_score}");

    // Also extract any "lies" count (CreepJS reports this prominently)
    let lies = page
        .evaluate(
            r#"(() => {
                const all = document.body ? document.body.textContent : '';
                const matches = all.match(/(\d+)\s*(?:lies|lie)/i);
                return matches ? matches[1] : 'none';
            })()"#,
        )
        .unwrap_or_default();
    println!("lies count: {lies}");

    // Look for the FP hash
    let body_len = page
        .evaluate("document.body ? document.body.textContent.length : 0")
        .unwrap_or_default();
    println!("body text len: {body_len}");

    // Look for any text containing a percentage
    let percentage_texts = page
        .evaluate(
            r#"(() => {
                const matches = [];
                const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
                let n;
                while ((n = walker.nextNode())) {
                    const t = n.textContent.trim();
                    if (/\d+\s*%/.test(t) && t.length < 50) matches.push(t);
                    if (matches.length >= 10) break;
                }
                return JSON.stringify(matches);
            })()"#,
        )
        .unwrap_or_default();
    println!("percentage texts: {percentage_texts}");
}

#[tokio::test]
#[ignore]
async fn scorer_pixelscan() {
    let profile = stealth::chrome_130_windows();
    let mut page = load_and_wait("https://pixelscan.net/", profile, 5000).await;
    let title = page.title();
    println!("\n=== pixelscan.net ===");
    println!("title: {title}");
    let consistent = page
        .evaluate(
            r#"(() => {
                const text = document.body ? document.body.textContent : '';
                if (text.includes('Consistent') && !text.includes('Inconsistent')) return 'CONSISTENT';
                if (text.includes('Inconsistent')) return 'INCONSISTENT';
                return 'unknown';
            })()"#,
        )
        .unwrap_or_default();
    println!("verdict: {consistent}");
    let automation = page
        .evaluate(
            r#"(() => {
                const text = document.body ? document.body.textContent.toLowerCase() : '';
                return text.includes('automation detected') ? 'FLAGGED' : 'OK';
            })()"#,
        )
        .unwrap_or_default();
    println!("automation: {automation}");
}

#[tokio::test]
#[ignore]
async fn scorer_amiunique() {
    let profile = stealth::chrome_130_windows();
    let mut page = load_and_wait("https://amiunique.org/fingerprint", profile, 6000).await;
    let title = page.title();
    println!("\n=== amiunique.org ===");
    println!("title: {title}");
    // amiunique renders the uniqueness score prominently
    let uniqueness = page
        .evaluate(
            r#"(() => {
                const text = document.body ? document.body.textContent : '';
                // Look for "Your fingerprint appears to be unique among..."
                const m = text.match(/(\d+\.\d+)\s*%|(\d+)\s*%/);
                return m ? (m[1] || m[2]) : 'none';
            })()"#,
        )
        .unwrap_or_default();
    println!("uniqueness-related percentages: {uniqueness}");
}

#[tokio::test]
#[ignore]
async fn scorer_sannysoft() {
    // bot.sannysoft.com renders a table with 18 rows, each either "pass"
    // (green cell) or "fail" (red cell). Extract the pass/fail counts.
    let profile = stealth::chrome_130_windows();
    let mut page = load_and_wait("https://bot.sannysoft.com/", profile, 3000).await;
    println!("\n=== bot.sannysoft.com ===");
    let counts = page
        .evaluate(
            r#"(() => {
                const greens = document.querySelectorAll('td.passed, td.pass, td[style*="green"]').length;
                const reds = document.querySelectorAll('td.failed, td.fail, td[style*="red"]').length;
                // Also count by textContent
                const rows = Array.from(document.querySelectorAll('tr'));
                let passed = 0, failed = 0, unknown = 0;
                for (const row of rows) {
                    const text = row.textContent.toLowerCase();
                    if (text.includes('passed') || text.includes('ok')) passed++;
                    else if (text.includes('failed') || text.includes('fail')) failed++;
                }
                return JSON.stringify({greens, reds, passed, failed, totalRows: rows.length});
            })()"#,
        )
        .unwrap_or_default();
    println!("counts: {counts}");
}

#[tokio::test]
#[ignore]
async fn scorer_browserleaks_canvas() {
    // browserleaks/canvas renders a specific canvas fingerprint hash
    // that we want to check is STABLE across runs (per profile).
    let profile = stealth::chrome_130_windows();
    let mut page = load_and_wait("https://browserleaks.com/canvas", profile, 4000).await;
    println!("\n=== browserleaks.com/canvas ===");
    let hash = page
        .evaluate(
            r#"(() => {
                const text = document.body ? document.body.textContent : '';
                const m = text.match(/Signature[:\s]+([0-9a-fA-F]+)/i);
                return m ? m[1] : 'none';
            })()"#,
        )
        .unwrap_or_default();
    println!("canvas signature: {hash}");
    let uniqueness = page
        .evaluate(
            r#"(() => {
                const text = document.body ? document.body.textContent : '';
                const m = text.match(/Uniqueness[:\s]+([0-9.]+\s*%|\d+\s*in\s*\d+)/i);
                return m ? m[1] : 'none';
            })()"#,
        )
        .unwrap_or_default();
    println!("uniqueness: {uniqueness}");
}

#[tokio::test]
#[ignore]
async fn scorer_browserleaks_webgl() {
    let profile = stealth::chrome_130_windows();
    let mut page = load_and_wait("https://browserleaks.com/webgl", profile, 4000).await;
    println!("\n=== browserleaks.com/webgl ===");
    let report = page
        .evaluate(
            r#"(() => {
                const text = document.body ? document.body.textContent : '';
                const vendor = text.match(/Vendor[:\s]+([^\n]{1,100})/i)?.[1] || 'none';
                const renderer = text.match(/Renderer[:\s]+([^\n]{1,150})/i)?.[1] || 'none';
                const unmasked = text.match(/Unmasked\s*Renderer[:\s]+([^\n]{1,150})/i)?.[1] || 'none';
                return JSON.stringify({vendor, renderer, unmasked});
            })()"#,
        )
        .unwrap_or_default();
    println!("webgl report: {report}");
}

// §6.6 item 1 — navigator.plugins / mimeTypes parity with fixture §10.1.
// Verifies: shape, count driven by profile, stable identity, item/namedItem,
// enabledPlugin linking, iteration, numeric indexing.
#[tokio::test]
async fn test_plugins_parity() {
    let profile = stealth::chrome_130_windows();
    let mut page = browser::Page::with_profile("", "about:blank", profile)
        .await
        .unwrap();

    let probe = r#"
        (() => {
            const arr = [];
            for (const p of navigator.plugins) arr.push(p.name);
            const mimeArr = [];
            for (const m of navigator.mimeTypes) mimeArr.push(m.type);
            const p0 = navigator.plugins[0];
            return JSON.stringify({
                plugins_length: navigator.plugins.length,
                mimeTypes_length: navigator.mimeTypes.length,
                identity_stable: navigator.plugins === navigator.plugins,
                mime_identity_stable: navigator.mimeTypes === navigator.mimeTypes,
                item_matches_index: navigator.plugins.item(0) === navigator.plugins[0],
                namedItem_matches: navigator.plugins.namedItem('PDF Viewer') === navigator.plugins[0],
                namedItem_missing: navigator.plugins.namedItem('Not A Real Plugin'),
                mime_item_matches: navigator.mimeTypes.item(0) === navigator.mimeTypes[0],
                p0_name: p0 && p0.name,
                p0_filename: p0 && p0.filename,
                p0_description: p0 && p0.description,
                p0_length: p0 && p0.length,
                p0_mime0_type: p0 && p0[0] && p0[0].type,
                mime0_enabledPlugin_is_p0: navigator.mimeTypes[0].enabledPlugin === p0,
                mime0_type: navigator.mimeTypes[0] && navigator.mimeTypes[0].type,
                mime1_type: navigator.mimeTypes[1] && navigator.mimeTypes[1].type,
                iter_names: arr,
                iter_mime_types: mimeArr,
                plugins_is_PluginArray: navigator.plugins instanceof PluginArray,
                mimeTypes_is_MimeTypeArray: navigator.mimeTypes instanceof MimeTypeArray,
                p0_is_Plugin: p0 instanceof Plugin,
            });
        })()
    "#;
    let raw = page.evaluate(probe).unwrap();
    let obj: serde_json::Value = serde_json::from_str(&raw)
        .unwrap_or_else(|_| panic!("plugins probe result was not JSON: {}", raw));

    assert_eq!(obj["plugins_length"], 5, "default preset ships 5 plugins");
    assert_eq!(
        obj["mimeTypes_length"], 2,
        "default preset ships 2 mime types"
    );
    assert_eq!(obj["identity_stable"], true);
    assert_eq!(obj["mime_identity_stable"], true);
    assert_eq!(obj["item_matches_index"], true);
    assert_eq!(obj["namedItem_matches"], true);
    assert!(obj["namedItem_missing"].is_null());
    assert_eq!(obj["mime_item_matches"], true);
    assert_eq!(obj["p0_name"], "PDF Viewer");
    assert_eq!(obj["p0_filename"], "internal-pdf-viewer");
    assert_eq!(obj["p0_description"], "Portable Document Format");
    assert_eq!(
        obj["p0_length"], 2,
        "Plugin exposes its mime types by length"
    );
    assert_eq!(obj["p0_mime0_type"], "application/pdf");
    assert_eq!(obj["mime0_enabledPlugin_is_p0"], true);
    assert_eq!(obj["mime0_type"], "application/pdf");
    assert_eq!(obj["mime1_type"], "text/pdf");
    assert_eq!(obj["plugins_is_PluginArray"], true);
    assert_eq!(obj["mimeTypes_is_MimeTypeArray"], true);
    assert_eq!(obj["p0_is_Plugin"], true);

    let names: Vec<&str> = obj["iter_names"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        vec![
            "PDF Viewer",
            "Chrome PDF Viewer",
            "Chromium PDF Viewer",
            "Microsoft Edge PDF Viewer",
            "WebKit built-in PDF",
        ]
    );
    let mimes: Vec<&str> = obj["iter_mime_types"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(mimes, vec!["application/pdf", "text/pdf"]);
}

// §6.6 item 1 — profile.plugins_count is respected. Build a profile that
// claims only 3 plugins / 1 mime type and verify the JS surface slices.
#[tokio::test]
async fn test_plugins_count_from_profile() {
    let mut profile = stealth::chrome_130_windows();
    profile.plugins_count = 3;
    profile.mime_types_count = 1;
    let mut page = browser::Page::with_profile("", "about:blank", profile)
        .await
        .unwrap();

    assert_eq!(page.evaluate("navigator.plugins.length").unwrap(), "3");
    assert_eq!(page.evaluate("navigator.mimeTypes.length").unwrap(), "1");
    // Last plugin in the 5-array is out of range now
    assert_eq!(
        page.evaluate("navigator.plugins[3] === undefined || navigator.plugins[3] === null")
            .unwrap(),
        "true"
    );
}
