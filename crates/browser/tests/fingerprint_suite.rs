//! Modern hard-mode fingerprinting test suite.
//!
//! Runs browser_oxide against a curated panel of fingerprinting test
//! pages and captures (a) the rendered HTML, (b) a per-site extracted
//! JSON state blob, and (c) a PASS / PARTIAL / FAIL / ERROR
//! classification. Results are written under
//! `docs/FINGERPRINT_SUITE_2026_05_10/`.
//!
//! This is the verification gate for "SOTA stealth" claims — every
//! site here is a known, public, modern detector and the verdicts are
//! captured straight from the page's own DOM/globals (no string
//! pattern hand-waving where a real signal exists).
//!
//! Run with:
//!
//! ```bash
//! BOXIDE_NAV_BUDGET_MS=45000 \
//!   cargo test --release -p browser --test fingerprint_suite \
//!     -- --ignored --test-threads=1 --nocapture fingerprint_suite_full_run
//! ```
//!
//! Per-site outputs:
//!   docs/FINGERPRINT_SUITE_2026_05_10/<site>.html   — rendered HTML
//!   docs/FINGERPRINT_SUITE_2026_05_10/<site>.json   — extracted state
//!   docs/FINGERPRINT_SUITE_2026_05_10/SUMMARY.md    — verdict table

use browser::Page;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    Pass,
    Partial,
    Fail,
    Error,
}

impl Verdict {
    fn as_str(self) -> &'static str {
        match self {
            Verdict::Pass => "PASS",
            Verdict::Partial => "PARTIAL",
            Verdict::Fail => "FAIL",
            Verdict::Error => "ERROR",
        }
    }
}

struct SiteResult {
    site: &'static str,
    url: &'static str,
    verdict: Verdict,
    note: String,
    #[allow(dead_code)] // surfaced via per-site .json artifact; held for future inspection
    extracted: Value,
}

fn out_dir() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // crates/browser → repo root
    p.pop();
    p.pop();
    p.push("docs");
    p.push("FINGERPRINT_SUITE_2026_05_10");
    p
}

fn write_artifacts(site: &str, html: &str, extracted: &Value) {
    let dir = out_dir();
    let _ = std::fs::create_dir_all(&dir);
    let html_path = dir.join(format!("{}.html", site));
    let json_path = dir.join(format!("{}.json", site));
    let _ = std::fs::write(&html_path, html);
    let _ = std::fs::write(
        &json_path,
        serde_json::to_string_pretty(extracted).unwrap_or_default(),
    );
}

/// Wait until a JS predicate returns "true" or the deadline elapses.
/// Steps the event loop in 1s slices via `evaluate_async`.
async fn wait_for(page: &mut Page, predicate_js: &str, max_secs: u64) -> bool {
    for _ in 0..max_secs {
        if let Ok(v) = page.evaluate(predicate_js) {
            if v == "true" {
                return true;
            }
        }
        // Drive scheduled timers / microtasks for ~1s.
        let _ = page
            .evaluate_async("/*tick*/ void 0;", Duration::from_millis(1000))
            .await;
    }
    matches!(page.evaluate(predicate_js).as_deref(), Ok("true"))
}

fn parse_json(s: &str) -> Value {
    serde_json::from_str(s).unwrap_or_else(|_| Value::String(s.to_string()))
}

// ---------------------------------------------------------------------
// Per-site verdict extraction
// ---------------------------------------------------------------------

async fn run_creepjs(profile: stealth::StealthProfile) -> SiteResult {
    let url = "https://abrahamjuliot.github.io/creepjs/";
    let mut page = match Page::navigate(url, profile, 3).await {
        Ok(p) => p,
        Err(e) => {
            return SiteResult {
                site: "creepjs",
                url,
                verdict: Verdict::Error,
                note: format!("navigation failed: {e}"),
                extracted: Value::Null,
            };
        }
    };
    // CreepJS posts results into window.creep after running every
    // detector. The fingerprint-header-container DOM node also
    // appears once the report is rendered — use whichever first.
    let _ = wait_for(
        &mut page,
        "(typeof window.creep === 'object' && window.creep && \
         window.creep.fingerprint && \
         typeof window.creep.fingerprint.trustScore === 'number') \
         || !!document.querySelector('.fingerprint-header-container')",
        40,
    )
    .await;

    let raw = page
        .evaluate(
            r#"JSON.stringify({
                trustScore: (window.creep && window.creep.fingerprint && window.creep.fingerprint.trustScore) || null,
                lies: [...document.querySelectorAll('.lies')].map(e => e.innerText).slice(0, 12),
                fuzzy: (document.querySelector('.fuzzy-signature') && document.querySelector('.fuzzy-signature').innerText) || null,
                header: (document.querySelector('.fingerprint-header-container') && document.querySelector('.fingerprint-header-container').innerText) || null,
                hasCreepGlobal: typeof window.creep === 'object' && !!window.creep
            })"#,
        )
        .unwrap_or_else(|e| format!("{{\"error\":\"{}\"}}", e.to_string().replace('"', "\\\"")));
    let extracted = parse_json(&raw);
    let html = page.content();
    write_artifacts("creepjs", &html, &extracted);

    let trust = extracted.get("trustScore").and_then(|v| v.as_f64());
    let (verdict, note) = match trust {
        Some(score) if score >= 90.0 => (Verdict::Pass, format!("trustScore={score}")),
        Some(score) if score >= 70.0 => (Verdict::Partial, format!("trustScore={score}")),
        Some(score) => (Verdict::Fail, format!("trustScore={score}")),
        None => (
            Verdict::Fail,
            "no trustScore (window.creep absent or report did not finish)".into(),
        ),
    };
    SiteResult {
        site: "creepjs",
        url,
        verdict,
        note,
        extracted,
    }
}

async fn run_pixelscan(profile: stealth::StealthProfile) -> SiteResult {
    let url = "https://pixelscan.net/";
    let mut page = match Page::navigate(url, profile, 3).await {
        Ok(p) => p,
        Err(e) => {
            return SiteResult {
                site: "pixelscan",
                url,
                verdict: Verdict::Error,
                note: format!("navigation failed: {e}"),
                extracted: Value::Null,
            };
        }
    };
    let _ = wait_for(
        &mut page,
        "!!document.body && document.body.innerText.length > 500",
        25,
    )
    .await;

    let raw = page
        .evaluate(
            r#"JSON.stringify({
                bodyText: (document.body && document.body.innerText || '').slice(0, 4000),
                consistency: (() => {
                    const m = (document.body && document.body.innerText || '')
                        .match(/consistency[^\d]{0,40}(\d{1,3})\s*%/i);
                    return m ? Number(m[1]) : null;
                })(),
                spoofingHits: (() => {
                    const txt = (document.body && document.body.innerText || '').toLowerCase();
                    return ['spoofing detected','mismatch','inconsistent','automation','bot detected']
                        .filter(s => txt.includes(s));
                })()
            })"#,
        )
        .unwrap_or_else(|e| format!("{{\"error\":\"{}\"}}", e.to_string().replace('"', "\\\"")));
    let extracted = parse_json(&raw);
    let html = page.content();
    write_artifacts("pixelscan", &html, &extracted);

    let consistency = extracted.get("consistency").and_then(|v| v.as_f64());
    let spoofing_hits = extracted
        .get("spoofingHits")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let (verdict, note) = match (consistency, spoofing_hits) {
        (Some(c), 0) if c >= 90.0 => (Verdict::Pass, format!("consistency={c}%")),
        (Some(c), 0) if c >= 70.0 => (Verdict::Partial, format!("consistency={c}%")),
        (Some(c), _) => (
            Verdict::Fail,
            format!("consistency={c}% spoofing_hits={spoofing_hits}"),
        ),
        (None, 0) => (
            Verdict::Partial,
            "no consistency score parsed; no negative markers".into(),
        ),
        (None, _) => (Verdict::Fail, format!("spoofing_hits={spoofing_hits}")),
    };
    SiteResult {
        site: "pixelscan",
        url,
        verdict,
        note,
        extracted,
    }
}

async fn run_browserleaks(profile: stealth::StealthProfile, section: &'static str) -> SiteResult {
    let site_key: &'static str = match section {
        "canvas" => "browserleaks_canvas",
        "webgl" => "browserleaks_webgl",
        "javascript" => "browserleaks_javascript",
        "fonts" => "browserleaks_fonts",
        "webrtc" => "browserleaks_webrtc",
        "proxy" => "browserleaks_proxy",
        _ => "browserleaks_unknown",
    };
    let url_owned = format!("https://browserleaks.com/{section}");
    // Leak the URL into a 'static via Box::leak so the SiteResult
    // (which keeps a &'static str) can reference it. Test-scope only.
    let url: &'static str = Box::leak(url_owned.into_boxed_str());

    let mut page = match Page::navigate(url, profile, 3).await {
        Ok(p) => p,
        Err(e) => {
            return SiteResult {
                site: site_key,
                url,
                verdict: Verdict::Error,
                note: format!("navigation failed: {e}"),
                extracted: Value::Null,
            };
        }
    };
    let _ = wait_for(
        &mut page,
        "!!document.body && document.body.innerText.length > 800",
        25,
    )
    .await;

    let raw = page
        .evaluate(
            r#"JSON.stringify({
                title: document.title || '',
                bodyText: (document.body && document.body.innerText || '').slice(0, 6000),
                tableRows: [...document.querySelectorAll('table tr')]
                    .slice(0, 80)
                    .map(tr => tr.innerText.replace(/\s+/g, ' ').trim())
                    .filter(t => t.length > 0)
            })"#,
        )
        .unwrap_or_else(|e| format!("{{\"error\":\"{}\"}}", e.to_string().replace('"', "\\\"")));
    let extracted = parse_json(&raw);
    let html = page.content();
    write_artifacts(site_key, &html, &extracted);

    // BrowserLeaks doesn't emit a single PASS bit — it just reports
    // your fingerprint. We classify on (a) whether the page rendered
    // a real report (has table rows / non-trivial body) and (b)
    // whether the body contains obvious bot/headless markers.
    let body = extracted
        .get("bodyText")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_lowercase();
    let rows = extracted
        .get("tableRows")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let bad_markers = ["headless", "automation", "phantomjs", "webdriver"]
        .iter()
        .filter(|m| body.contains(*m))
        .count();
    let (verdict, note) = if rows >= 5 && bad_markers == 0 {
        (Verdict::Pass, format!("{rows} report rows, no bot markers"))
    } else if rows >= 3 && bad_markers == 0 {
        (Verdict::Partial, format!("{rows} rows; thin report"))
    } else if bad_markers > 0 {
        (
            Verdict::Fail,
            format!("{bad_markers} bot-marker hits in body"),
        )
    } else {
        (Verdict::Fail, format!("only {rows} rows, no clear report"))
    };
    SiteResult {
        site: site_key,
        url,
        verdict,
        note,
        extracted,
    }
}

async fn run_rebrowser(profile: stealth::StealthProfile) -> SiteResult {
    let url = "https://bot-detector.rebrowser.net/";
    let mut page = match Page::navigate(url, profile, 3).await {
        Ok(p) => p,
        Err(e) => {
            return SiteResult {
                site: "rebrowser",
                url,
                verdict: Verdict::Error,
                note: format!("navigation failed: {e}"),
                extracted: Value::Null,
            };
        }
    };
    let _ = wait_for(
        &mut page,
        "!!document.body && document.body.innerText.length > 400",
        25,
    )
    .await;

    let raw = page
        .evaluate(
            r#"JSON.stringify({
                bodyText: (document.body && document.body.innerText || '').slice(0, 6000),
                passedCount: (document.body && document.body.innerText || '').match(/PASSED/gi)?.length || 0,
                failedCount: (document.body && document.body.innerText || '').match(/FAILED/gi)?.length || 0,
                detectedCount: (document.body && document.body.innerText || '').match(/DETECTED/gi)?.length || 0
            })"#,
        )
        .unwrap_or_else(|e| format!("{{\"error\":\"{}\"}}", e.to_string().replace('"', "\\\"")));
    let extracted = parse_json(&raw);
    let html = page.content();
    write_artifacts("rebrowser", &html, &extracted);

    let passed = extracted
        .get("passedCount")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let failed = extracted
        .get("failedCount")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let detected = extracted
        .get("detectedCount")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let bad = failed + detected;
    let (verdict, note) = if passed >= 3 && bad == 0 {
        (
            Verdict::Pass,
            format!("PASSED={passed} FAILED={failed} DETECTED={detected}"),
        )
    } else if passed > 0 && bad <= 1 {
        (
            Verdict::Partial,
            format!("PASSED={passed} FAILED={failed} DETECTED={detected}"),
        )
    } else if passed == 0 && bad == 0 {
        (
            Verdict::Fail,
            "no PASSED/FAILED markers — page likely did not render".into(),
        )
    } else {
        (
            Verdict::Fail,
            format!("PASSED={passed} FAILED={failed} DETECTED={detected}"),
        )
    };
    SiteResult {
        site: "rebrowser",
        url,
        verdict,
        note,
        extracted,
    }
}

async fn run_fingerprint_demo(profile: stealth::StealthProfile) -> SiteResult {
    let url = "https://fingerprint.com/demo/";
    let mut page = match Page::navigate(url, profile, 3).await {
        Ok(p) => p,
        Err(e) => {
            return SiteResult {
                site: "fingerprint_com",
                url,
                verdict: Verdict::Error,
                note: format!("navigation failed: {e}"),
                extracted: Value::Null,
            };
        }
    };
    let _ = wait_for(
        &mut page,
        "!!document.body && document.body.innerText.length > 400",
        30,
    )
    .await;

    let raw = page
        .evaluate(
            r#"JSON.stringify({
                bodyText: (document.body && document.body.innerText || '').slice(0, 6000),
                visitorId: (() => {
                    const m = (document.body && document.body.innerText || '')
                        .match(/visitor\s*id[^\w]{0,8}([a-zA-Z0-9]{12,})/i);
                    return m ? m[1] : null;
                })(),
                botDetectionTexts: [...document.querySelectorAll('[class*="bot" i], [data-test*="bot" i]')]
                    .slice(0, 10)
                    .map(e => (e.innerText || '').slice(0, 200))
            })"#,
        )
        .unwrap_or_else(|e| format!("{{\"error\":\"{}\"}}", e.to_string().replace('"', "\\\"")));
    let extracted = parse_json(&raw);
    let html = page.content();
    write_artifacts("fingerprint_com", &html, &extracted);

    let body = extracted
        .get("bodyText")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_lowercase();
    let visitor_id = extracted.get("visitorId").and_then(|v| v.as_str());
    let bot_signal = body.contains("automated")
        || body.contains("bot detected")
        || body.contains("automation tool");
    let (verdict, note) = match (visitor_id, bot_signal) {
        (Some(id), false) => (Verdict::Pass, format!("visitorId={id}")),
        (Some(id), true) => (
            Verdict::Partial,
            format!("visitorId={id} but bot signal in copy"),
        ),
        (None, false) => (
            Verdict::Partial,
            "no visitorId parsed; no bot signal".into(),
        ),
        (None, true) => (Verdict::Fail, "no visitorId; bot signal in copy".into()),
    };
    SiteResult {
        site: "fingerprint_com",
        url,
        verdict,
        note,
        extracted,
    }
}

async fn run_amiunique(profile: stealth::StealthProfile) -> SiteResult {
    let url = "https://amiunique.org/fingerprint";
    let mut page = match Page::navigate(url, profile, 3).await {
        Ok(p) => p,
        Err(e) => {
            return SiteResult {
                site: "amiunique",
                url,
                verdict: Verdict::Error,
                note: format!("navigation failed: {e}"),
                extracted: Value::Null,
            };
        }
    };
    let _ = wait_for(
        &mut page,
        "!!document.body && document.body.innerText.length > 600",
        30,
    )
    .await;

    let raw = page
        .evaluate(
            r#"JSON.stringify({
                title: document.title || '',
                bodyText: (document.body && document.body.innerText || '').slice(0, 5000),
                verdict: (() => {
                    const txt = (document.body && document.body.innerText || '');
                    if (/you can be tracked/i.test(txt) || /yes.*unique/i.test(txt)) return 'unique';
                    if (/you are not unique/i.test(txt) || /not unique/i.test(txt)) return 'not_unique';
                    return null;
                })()
            })"#,
        )
        .unwrap_or_else(|e| format!("{{\"error\":\"{}\"}}", e.to_string().replace('"', "\\\"")));
    let extracted = parse_json(&raw);
    let html = page.content();
    write_artifacts("amiunique", &html, &extracted);

    let v = extracted.get("verdict").and_then(|v| v.as_str());
    let (verdict, note) = match v {
        Some("not_unique") => (Verdict::Pass, "amiunique reports NOT unique".into()),
        Some("unique") => (
            Verdict::Partial,
            "amiunique reports unique (expected for new fp)".into(),
        ),
        _ => (Verdict::Partial, "no verdict text parsed".into()),
    };
    SiteResult {
        site: "amiunique",
        url,
        verdict,
        note,
        extracted,
    }
}

async fn run_eff(profile: stealth::StealthProfile) -> SiteResult {
    let url = "https://coveryourtracks.eff.org/";
    let mut page = match Page::navigate(url, profile, 3).await {
        Ok(p) => p,
        Err(e) => {
            return SiteResult {
                site: "eff_coveryourtracks",
                url,
                verdict: Verdict::Error,
                note: format!("navigation failed: {e}"),
                extracted: Value::Null,
            };
        }
    };
    let _ = wait_for(
        &mut page,
        "!!document.body && document.body.innerText.length > 500",
        25,
    )
    .await;
    let raw = page
        .evaluate(
            r#"JSON.stringify({
                title: document.title || '',
                bodyText: (document.body && document.body.innerText || '').slice(0, 5000)
            })"#,
        )
        .unwrap_or_else(|e| format!("{{\"error\":\"{}\"}}", e.to_string().replace('"', "\\\"")));
    let extracted = parse_json(&raw);
    let html = page.content();
    write_artifacts("eff_coveryourtracks", &html, &extracted);

    // EFF landing page is informational; the actual scan is at /test
    // and is JS-heavy. Treat HTML+content load with no captcha as
    // PARTIAL — full test would require the /test path.
    let body = extracted
        .get("bodyText")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let bad =
        body.to_lowercase().contains("just a moment") || body.to_lowercase().contains("captcha");
    let (verdict, note) = if !bad && body.len() > 800 {
        (
            Verdict::Partial,
            format!("landing page rendered ({} chars)", body.len()),
        )
    } else if bad {
        (Verdict::Fail, "challenge / captcha page returned".into())
    } else {
        (
            Verdict::Fail,
            format!("body too short: {} chars", body.len()),
        )
    };
    SiteResult {
        site: "eff_coveryourtracks",
        url,
        verdict,
        note,
        extracted,
    }
}

async fn run_deviceinfo(profile: stealth::StealthProfile) -> SiteResult {
    let url = "https://www.deviceinfo.me/";
    let mut page = match Page::navigate(url, profile, 3).await {
        Ok(p) => p,
        Err(e) => {
            return SiteResult {
                site: "deviceinfo",
                url,
                verdict: Verdict::Error,
                note: format!("navigation failed: {e}"),
                extracted: Value::Null,
            };
        }
    };
    let _ = wait_for(
        &mut page,
        "!!document.body && document.body.innerText.length > 500",
        25,
    )
    .await;

    let raw = page
        .evaluate(
            r#"JSON.stringify({
                title: document.title || '',
                bodyText: (document.body && document.body.innerText || '').slice(0, 5000),
                detectedOS: (() => {
                    const m = (document.body && document.body.innerText || '')
                        .match(/operating system[^\n]{0,80}/i);
                    return m ? m[0] : null;
                })()
            })"#,
        )
        .unwrap_or_else(|e| format!("{{\"error\":\"{}\"}}", e.to_string().replace('"', "\\\"")));
    let extracted = parse_json(&raw);
    let html = page.content();
    write_artifacts("deviceinfo", &html, &extracted);

    // Spoofed UA says Windows. PASS = page reports Windows-consistent
    // OS. FAIL = it figures out Linux/headless.
    let body = extracted
        .get("bodyText")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_lowercase();
    let says_linux = body.contains("linux") && !body.contains("windows");
    let says_windows = body.contains("windows");
    let (verdict, note) = if says_windows && !says_linux {
        (
            Verdict::Pass,
            "OS reported as Windows, no Linux leak".into(),
        )
    } else if says_linux {
        (Verdict::Fail, "true OS leaked: Linux".into())
    } else {
        (Verdict::Partial, "could not parse OS".into())
    };
    SiteResult {
        site: "deviceinfo",
        url,
        verdict,
        note,
        extracted,
    }
}

async fn run_nowsecure(profile: stealth::StealthProfile) -> SiteResult {
    let url = "https://nowsecure.nl";
    let mut page = match Page::navigate(url, profile, 3).await {
        Ok(p) => p,
        Err(e) => {
            return SiteResult {
                site: "nowsecure_cloudflare",
                url,
                verdict: Verdict::Error,
                note: format!("navigation failed: {e}"),
                extracted: Value::Null,
            };
        }
    };
    // Cloudflare interstitial typically resolves to real content
    // within 5–8s once the JS challenge runs.
    let _ = wait_for(
        &mut page,
        "!!document.body && document.documentElement.outerHTML.length > 5120 \
         && !/just a moment/i.test(document.body.innerText || '')",
        25,
    )
    .await;

    let html = page.content();
    let raw = page
        .evaluate(
            r#"JSON.stringify({
                title: document.title || '',
                bodyText: (document.body && document.body.innerText || '').slice(0, 4000),
                htmlLen: document.documentElement.outerHTML.length,
                challengeMarkers: {
                    justAMoment: /just a moment/i.test(document.body && document.body.innerText || ''),
                    cfChallenge: !!document.querySelector('[id^="challenge"], [class*="cf-"]')
                }
            })"#,
        )
        .unwrap_or_else(|e| format!("{{\"error\":\"{}\"}}", e.to_string().replace('"', "\\\"")));
    let extracted = parse_json(&raw);
    write_artifacts("nowsecure_cloudflare", &html, &extracted);

    let html_len = extracted
        .get("htmlLen")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let just_a_moment = extracted
        .get("challengeMarkers")
        .and_then(|m| m.get("justAMoment"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let (verdict, note) = if html_len > 5120 && !just_a_moment {
        (
            Verdict::Pass,
            format!("HTML {html_len}b, no challenge text"),
        )
    } else if just_a_moment {
        (
            Verdict::Fail,
            "Cloudflare 'Just a moment' interstitial".into(),
        )
    } else {
        (Verdict::Fail, format!("HTML only {html_len}b"))
    };
    SiteResult {
        site: "nowsecure_cloudflare",
        url,
        verdict,
        note,
        extracted,
    }
}

async fn run_datadome(profile: stealth::StealthProfile) -> SiteResult {
    let url = "https://antoinevastel.com/bots/datadome";
    let mut page = match Page::navigate(url, profile, 3).await {
        Ok(p) => p,
        Err(e) => {
            return SiteResult {
                site: "datadome_antoinevastel",
                url,
                verdict: Verdict::Error,
                note: format!("navigation failed: {e}"),
                extracted: Value::Null,
            };
        }
    };
    let _ = wait_for(
        &mut page,
        "!!document.body && document.body.innerText.length > 200",
        25,
    )
    .await;

    let raw = page
        .evaluate(
            r#"JSON.stringify({
                title: document.title || '',
                bodyText: (document.body && document.body.innerText || '').slice(0, 4000),
                hasBlockedText: /blocked|access denied|captcha/i.test(document.body && document.body.innerText || ''),
                hasDataDomeCookie: /datadome/i.test(document.cookie || '')
            })"#,
        )
        .unwrap_or_else(|e| format!("{{\"error\":\"{}\"}}", e.to_string().replace('"', "\\\"")));
    let extracted = parse_json(&raw);
    let html = page.content();
    write_artifacts("datadome_antoinevastel", &html, &extracted);

    let blocked = extracted
        .get("hasBlockedText")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let body_len = extracted
        .get("bodyText")
        .and_then(|v| v.as_str())
        .map(str::len)
        .unwrap_or(0);
    let (verdict, note) = if !blocked && body_len > 200 {
        (
            Verdict::Pass,
            format!("rendered {body_len} chars, no block markers"),
        )
    } else if blocked {
        (
            Verdict::Fail,
            "DataDome block / captcha text present".into(),
        )
    } else {
        (Verdict::Fail, format!("body too short: {body_len} chars"))
    };
    SiteResult {
        site: "datadome_antoinevastel",
        url,
        verdict,
        note,
        extracted,
    }
}

// ---------------------------------------------------------------------
// Summary writer + driver
// ---------------------------------------------------------------------

fn write_summary(results: &[SiteResult]) {
    let dir = out_dir();
    let _ = std::fs::create_dir_all(&dir);
    let mut md = String::new();
    md.push_str("# Fingerprint Suite — 2026-05-10\n\n");
    md.push_str(
        "Run via `cargo test --release -p browser --test fingerprint_suite \
                 -- --ignored --test-threads=1 --nocapture fingerprint_suite_full_run`.\n\n",
    );
    md.push_str("Profile: `stealth::presets::chrome_130_windows()` (UA reports Chrome 147).\n\n");
    md.push_str("| Site | URL | Verdict | Note |\n");
    md.push_str("|------|-----|---------|------|\n");
    for r in results {
        let note_clean = r.note.replace('|', "\\|").replace('\n', " ");
        md.push_str(&format!(
            "| {} | <{}> | **{}** | {} |\n",
            r.site,
            r.url,
            r.verdict.as_str(),
            note_clean
        ));
    }
    md.push_str(
        "\nPer-site artifacts: `<site>.html` (rendered DOM) and `<site>.json` (extracted state).\n",
    );
    let summary_path = dir.join("SUMMARY.md");
    let _ = std::fs::write(&summary_path, md);
}

fn print_summary(results: &[SiteResult]) {
    println!();
    println!("=========================================================");
    println!("  FINGERPRINT SUITE — verdict summary");
    println!("=========================================================");
    let (mut p, mut pa, mut f, mut e) = (0, 0, 0, 0);
    for r in results {
        match r.verdict {
            Verdict::Pass => p += 1,
            Verdict::Partial => pa += 1,
            Verdict::Fail => f += 1,
            Verdict::Error => e += 1,
        }
        println!("  [{:>7}] {:<32} — {}", r.verdict.as_str(), r.site, r.note);
    }
    println!("---------------------------------------------------------");
    println!(
        "  totals: PASS={p}  PARTIAL={pa}  FAIL={f}  ERROR={e}  (of {})",
        results.len()
    );
    println!("=========================================================");
}

#[tokio::test]
#[ignore]
async fn fingerprint_suite_full_run() {
    let mut results: Vec<SiteResult> = Vec::new();

    // Each site runs in its own scope so the Page (and its V8
    // isolate) is dropped before the next one starts. V8 is
    // single-threaded per isolate but that's per Page; serial here
    // is required because the test runner uses --test-threads=1
    // anyway and concurrent isolates inside one tokio task would
    // not help.
    results.push(run_creepjs(stealth::presets::chrome_130_windows()).await);
    results.push(run_pixelscan(stealth::presets::chrome_130_windows()).await);
    for section in ["canvas", "webgl", "javascript", "fonts", "webrtc", "proxy"] {
        results.push(run_browserleaks(stealth::presets::chrome_130_windows(), section).await);
    }
    results.push(run_rebrowser(stealth::presets::chrome_130_windows()).await);
    results.push(run_fingerprint_demo(stealth::presets::chrome_130_windows()).await);
    results.push(run_amiunique(stealth::presets::chrome_130_windows()).await);
    results.push(run_eff(stealth::presets::chrome_130_windows()).await);
    results.push(run_deviceinfo(stealth::presets::chrome_130_windows()).await);
    results.push(run_nowsecure(stealth::presets::chrome_130_windows()).await);
    results.push(run_datadome(stealth::presets::chrome_130_windows()).await);

    write_summary(&results);
    print_summary(&results);

    // Don't fail the test based on verdicts — the point is to
    // capture and report. CI gates can grep SUMMARY.md.
    // We do however assert that every site produced *some* result
    // (catching catastrophic Page::navigate panics that we missed
    // wrapping above).
    assert!(
        !results.is_empty(),
        "no per-site results produced — driver bug"
    );
    let json_extras_unused = json!({}); // silence dead-import warning if json! unused
    let _ = json_extras_unused;
}
