//! Phase 3 audit — capture cookies + network detail + response shape
//! for each of the 29 sites that don't reach L3-RENDERED on the
//! holistic sweep. Output is a structured JSON-per-site dump under
//! `/tmp/audit_failing_sites/` that lets us bucket failures by gate
//! type without re-running the full sweep.
//!
//! Run with `--ignored` (live network) and `--nocapture` to see each
//! site's summary line. Each capture saves a file with cookie names,
//! Set-Cookie raw values, response headers of the top-level response,
//! and final body classification.
//!
//! Buckets we are looking to confirm:
//! - **Edge-blocked**: tiny body (<5 KB), challenge cookie set
//!   "blocked" — TLS/HTTP-2/header layer issue.
//! - **Sensor-failed**: body > 100 KB rendered but classifier hits a
//!   challenge marker — Akamai/PX JS sensor returned unfavorable score.
//! - **Captcha-shell**: SPA shell (10-100 KB), framework didn't
//!   populate — render-completeness, not stealth.
//! - **Outright BLOCKED**: tier-1 deny list / IP-tuple block.
//! - **THIN-BODY**: redirect chain didn't terminate at content.

use browser::Page;
use stealth::presets::chrome_130_macos;

const FAILING_SITES: &[(&str, &str, &str)] = &[
    // (key, vendor-bucket, url)
    ("bestbuy", "Akamai-CHL", "https://www.bestbuy.com/"),
    ("brave", "BLOCKED", "https://search.brave.com/"),
    ("canadagoose", "Kasada-CHL", "https://www.canadagoose.com/"),
    ("costco", "Akamai-CHL", "https://www.costco.com/"),
    ("disneyplus", "Akamai-CHL", "https://www.disneyplus.com/"),
    ("duolingo", "captcha-CHL", "https://www.duolingo.com/"),
    ("etsy", "DataDome-CHL", "https://www.etsy.com/"),
    ("expedia", "Akamai-CHL", "https://www.expedia.com/"),
    ("h-m", "Akamai-CHL", "https://www2.hm.com/"),
    ("homedepot", "Akamai-CHL", "https://www.homedepot.com/"),
    ("hulu", "Akamai-CHL", "https://www.hulu.com/"),
    ("hyatt", "Kasada-CHL", "https://www.hyatt.com/"),
    ("leboncoin", "DataDome-CHL", "https://www.leboncoin.fr/"),
    ("macys", "Kasada-CHL", "https://www.macys.com/"),
    ("mail-ru", "THIN-BODY", "https://mail.ru/"),
    ("medium", "captcha-CHL", "https://medium.com/"),
    ("quora", "captcha-CHL", "https://www.quora.com/"),
    ("realtor", "Kasada-CHL", "https://www.realtor.com/"),
    ("skyscanner", "BLOCKED", "https://www.skyscanner.com/"),
    ("spotify", "captcha-CHL", "https://open.spotify.com/"),
    ("substack", "captcha-CHL", "https://substack.com/"),
    (
        "tripadvisor",
        "DataDome-CHL",
        "https://www.tripadvisor.com/",
    ),
    ("udemy", "Cloudflare-CHL", "https://www.udemy.com/"),
    ("uniqlo", "Akamai-CHL", "https://www.uniqlo.com/"),
    ("walmart", "Akamai-CHL", "https://www.walmart.com/"),
    (
        "washingtonpost",
        "Akamai-CHL",
        "https://www.washingtonpost.com/",
    ),
    ("wayfair", "PerimeterX-CHL", "https://www.wayfair.com/"),
    ("weather", "Akamai-CHL", "https://weather.com/"),
    ("yelp", "DataDome-CHL", "https://www.yelp.com/"),
];

/// Focused capture on the multi-MB sites we suspect are classifier
/// false positives — much faster than re-running all 29.
#[tokio::test]
#[ignore = "network: Phase 3 — focused capture for false-positive verification"]
async fn audit_suspected_false_positives() {
    let suspects: &[(&str, &str, &str)] = &[
        ("walmart", "Akamai-CHL", "https://www.walmart.com/"),
        ("costco", "Akamai-CHL", "https://www.costco.com/"),
        ("disneyplus", "Akamai-CHL", "https://www.disneyplus.com/"),
        ("hulu", "Akamai-CHL", "https://www.hulu.com/"),
        ("uniqlo", "Akamai-CHL", "https://www.uniqlo.com/"),
        ("weather", "Akamai-CHL", "https://weather.com/"),
        (
            "washingtonpost",
            "Akamai-CHL",
            "https://www.washingtonpost.com/",
        ),
        ("expedia", "Akamai-CHL", "https://www.expedia.com/"),
    ];
    capture_sites(suspects, "/tmp/audit_false_positives").await;
}

async fn capture_sites(sites: &[(&str, &str, &str)], out_dir: &str) {
    std::fs::create_dir_all(out_dir).expect("mkdir");
    let mut summary = Vec::new();
    for (key, bucket, url) in sites {
        eprintln!("\n=== AUDIT {key} ({bucket}) — {url}");
        let t0 = std::time::Instant::now();
        let result = Page::navigate(url, chrome_130_macos(), 1).await;
        let elapsed_ms = t0.elapsed().as_millis() as u64;
        let mut record = build_record(key, bucket, url, elapsed_ms, result).await;
        eprintln!(
            "  ok={} body_len={} cookies={} markers={:?} ms={}",
            record.ok,
            record.body_len,
            record.cookie_names.len(),
            record.html_markers,
            record.elapsed_ms
        );
        for ctx in &record.marker_contexts {
            eprintln!("    ctx: {ctx}");
        }
        let json = serde_json::to_string_pretty(&record).unwrap();
        std::fs::write(format!("{out_dir}/{key}.json"), &json).expect("write");
        record.marker_contexts.clear(); // keep summary lean
        summary.push(record);
    }
    let index_json = serde_json::to_string_pretty(&summary).unwrap();
    std::fs::write(format!("{out_dir}/_index.json"), index_json).expect("write index");
    eprintln!(
        "\n=== AUDIT DONE — wrote {} records to {out_dir}/",
        summary.len()
    );
}

async fn build_record(
    key: &str,
    bucket: &str,
    url: &str,
    elapsed_ms: u64,
    result: Result<Page, deno_core::error::AnyError>,
) -> SiteRecord {
    let mut record = SiteRecord {
        key: key.to_string(),
        vendor_bucket: bucket.to_string(),
        url: url.to_string(),
        elapsed_ms,
        ok: false,
        body_len: 0,
        final_url: String::new(),
        cookie_names: Vec::new(),
        cookie_summaries: Vec::new(),
        html_markers: Vec::new(),
        marker_contexts: Vec::new(),
    };
    match result {
        Ok(mut page) => {
            record.ok = true;
            let body = page
                .evaluate("document.documentElement.outerHTML")
                .unwrap_or_default();
            let body_clean = body.trim_matches('"');
            record.body_len = body_clean.len();
            record.final_url = page
                .evaluate("location.href")
                .unwrap_or_default()
                .trim_matches('"')
                .to_string();
            let cookies = page.evaluate("document.cookie").unwrap_or_default();
            let cookies_clean = cookies.trim_matches('"');
            for c in cookies_clean.split(';') {
                let c = c.trim();
                if let Some(name) = c.split('=').next() {
                    if !name.is_empty() {
                        record.cookie_names.push(name.to_string());
                    }
                }
            }
            for marker in [
                "_abck",
                "bm_sz",
                "bm_sv",
                "ak_bmsc",
                "_px3",
                "_pxhd",
                "datadome",
                "kpsdk",
                "cf_clearance",
                "__cf_bm",
                "incap_ses_",
                "__wbauid",
            ] {
                for c in cookies_clean.split(';') {
                    let c = c.trim();
                    if c.starts_with(&format!("{marker}=")) || c.starts_with(marker) {
                        let head: String = c.chars().take(80).collect();
                        record.cookie_summaries.push(head);
                        break;
                    }
                }
            }
            let lower = body_clean.to_lowercase();
            for marker in [
                "px-captcha",
                "_pxhd",
                "_px3",
                "ips.js",
                "kpsdk",
                "_abck",
                "bm_sz",
                "bot/captcha",
                "datadome",
                "just a moment",
                "checking your browser",
                "are you a robot",
                "press &amp; hold",
                "x-amzn-waf-action",
                "g-recaptcha",
                "akam/13",
                "captcha",
            ] {
                if let Some(idx) = lower.find(marker) {
                    record.html_markers.push(marker.to_string());
                    let start = idx.saturating_sub(40);
                    let end = (idx + marker.len() + 40).min(body_clean.len());
                    let snippet: String = body_clean[start..end].chars().take(80).collect();
                    record
                        .marker_contexts
                        .push(format!("{marker}: …{snippet}…"));
                }
            }
        }
        Err(e) => {
            eprintln!("  navigate ERROR: {e}");
        }
    }
    record
}

#[tokio::test]
#[ignore = "network: Phase 3 audit — captures cookies + network detail per failing site"]
async fn audit_all_failing_sites() {
    let out_dir = "/tmp/audit_failing_sites";
    std::fs::create_dir_all(out_dir).expect("mkdir");

    let mut summary = Vec::new();

    for (key, bucket, url) in FAILING_SITES {
        eprintln!("\n=== AUDIT {key} ({bucket}) — {url}");
        let t0 = std::time::Instant::now();

        let result = Page::navigate(url, chrome_130_macos(), 1).await;
        let elapsed_ms = t0.elapsed().as_millis() as u64;

        let mut record = SiteRecord {
            key: key.to_string(),
            vendor_bucket: bucket.to_string(),
            url: url.to_string(),
            elapsed_ms,
            ok: false,
            body_len: 0,
            final_url: String::new(),
            cookie_names: Vec::new(),
            cookie_summaries: Vec::new(),
            html_markers: Vec::new(),
            marker_contexts: Vec::new(),
        };

        match result {
            Ok(mut page) => {
                record.ok = true;
                let body = page
                    .evaluate("document.documentElement.outerHTML")
                    .unwrap_or_default();
                let body_clean = body.trim_matches('"');
                record.body_len = body_clean.len();
                record.final_url = page
                    .evaluate("location.href")
                    .unwrap_or_default()
                    .trim_matches('"')
                    .to_string();
                let cookies = page.evaluate("document.cookie").unwrap_or_default();
                let cookies_clean = cookies.trim_matches('"');
                for c in cookies_clean.split(';') {
                    let c = c.trim();
                    if let Some(name) = c.split('=').next() {
                        if !name.is_empty() {
                            record.cookie_names.push(name.to_string());
                        }
                    }
                }
                // Vendor-cookie summaries — head of value lets us see e.g.
                // `_abck=...~-1~...` (challenged) vs `_abck=0~0~...` (passed).
                for marker in [
                    "_abck",
                    "bm_sz",
                    "bm_sv",
                    "ak_bmsc",
                    "_px3",
                    "_pxhd",
                    "datadome",
                    "kpsdk",
                    "cf_clearance",
                    "__cf_bm",
                    "incap_ses_",
                    "__wbauid",
                ] {
                    for c in cookies_clean.split(';') {
                        let c = c.trim();
                        if c.starts_with(&format!("{marker}=")) || c.starts_with(marker) {
                            let head: String = c.chars().take(80).collect();
                            record.cookie_summaries.push(head);
                            break;
                        }
                    }
                }
                // HTML body markers that signal vendor challenge frames.
                // For each that hits, capture 80 chars of context so we can
                // tell whether it's a real challenge frame (`<iframe src="...akam/13...">`)
                // or an inline-JS reference (`var foo = "...akam/13...";` —
                // false positive for challenge classification).
                let lower = body_clean.to_lowercase();
                for marker in [
                    "px-captcha",
                    "_pxhd",
                    "_px3",
                    "ips.js",
                    "kpsdk",
                    "_abck",
                    "bm_sz",
                    "bot/captcha",
                    "datadome",
                    "just a moment",
                    "checking your browser",
                    "are you a robot",
                    "press &amp; hold",
                    "x-amzn-waf-action",
                    "g-recaptcha",
                    "akam/13",
                    "captcha",
                ] {
                    if let Some(idx) = lower.find(marker) {
                        record.html_markers.push(marker.to_string());
                        let start = idx.saturating_sub(40);
                        let end = (idx + marker.len() + 40).min(body_clean.len());
                        let snippet: String = body_clean[start..end].chars().take(80).collect();
                        record
                            .marker_contexts
                            .push(format!("{marker}: …{snippet}…"));
                    }
                }
            }
            Err(e) => {
                eprintln!("  navigate ERROR: {e}");
            }
        }

        eprintln!(
            "  ok={} body_len={} cookies={} markers={:?} ms={}",
            record.ok,
            record.body_len,
            record.cookie_names.len(),
            record.html_markers,
            record.elapsed_ms
        );

        let json = serde_json::to_string_pretty(&record).unwrap();
        std::fs::write(format!("{out_dir}/{key}.json"), &json).expect("write");
        summary.push(record);
    }

    // Write the summary index.
    let index_json = serde_json::to_string_pretty(&summary).unwrap();
    std::fs::write(format!("{out_dir}/_index.json"), index_json).expect("write index");

    eprintln!(
        "\n=== AUDIT DONE — wrote {} site records to {out_dir}/",
        summary.len()
    );

    // Print a 2-line summary per site to stderr for at-a-glance review.
    for r in &summary {
        eprintln!(
            "  {:<18} {:<18} body={:>8} cookies={:?}",
            r.key,
            r.vendor_bucket,
            r.body_len,
            vendor_cookies(&r.cookie_names)
        );
        if !r.html_markers.is_empty() {
            eprintln!(
                "                                       markers={:?}",
                r.html_markers
            );
        }
    }
}

fn vendor_cookies(names: &[String]) -> Vec<String> {
    let interesting = [
        "_abck",
        "bm_sz",
        "bm_sv",
        "ak_bmsc",
        "_px3",
        "_pxhd",
        "_pxvid",
        "datadome",
        "kpsdk",
        "cf_clearance",
        "__cf_bm",
    ];
    names
        .iter()
        .filter(|n| {
            interesting
                .iter()
                .any(|m| n.starts_with(m) || n.contains(m))
        })
        .cloned()
        .collect()
}

#[derive(serde::Serialize)]
struct SiteRecord {
    key: String,
    vendor_bucket: String,
    url: String,
    elapsed_ms: u64,
    ok: bool,
    body_len: usize,
    final_url: String,
    cookie_names: Vec<String>,
    cookie_summaries: Vec<String>,
    html_markers: Vec<String>,
    marker_contexts: Vec<String>,
}
