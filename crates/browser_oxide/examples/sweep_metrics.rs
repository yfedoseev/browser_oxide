//! Full-corpus sweep with rich, customer-facing metrics.
//!
//! Emits a JSON whose per-site records (timing, tag, body len)
//! line up field-for-field. Emits per-site: timing, classifier tag, body
//! length, error; and per-sweep: cold-start (= first page), peak RSS,
//! total wall-clock, median/p95/p99, throughput, per-category pass-rate.
//!
//! Usage:
//!   cargo run --release -p browser --example sweep_metrics -- \
//!       <profile> <corpus.json> <out.json>
//!
//! `<profile>` ∈ { chrome_148_macos, chrome_148_windows, firefox_135_macos,
//!                 iphone_15_pro_safari_18, pixel_9_pro_chrome_148 }
//!
//! Set `BROWSER_OXIDE_SWEEP_POOL=1` to use the pool path
//! (`PagePool::navigate`) instead of the cold `Page::navigate` per URL.
//! Both paths are interesting customer metrics so we publish them
//! separately rather than as a single number.

use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::time::Instant;

#[derive(serde::Deserialize, Clone)]
struct Site {
    cat: String,
    name: String,
    url: String,
    /// True iff this site exists in the corpus as a DIAGNOSTIC probe — i.e.
    /// it's INTENDED to fail (e.g. `areyouheadless` returns
    /// "Chrome Headless detected" by design). Diagnostic probes drag the
    /// raw pass-rate down equally on every engine; reporting them
    /// separately lets the project make honest "% of real sites we can
    /// browse" claims without methodology games.
    /// Defaults to false via serde so existing corpus JSONs keep working.
    #[serde(default)]
    diagnostic: bool,
}

#[derive(Serialize)]
struct SiteResult {
    cat: String,
    name: String,
    url: String,
    tag: String,
    len: usize,
    ms: u64,
    rss_mb: f64,
    err: Option<String>,
}

#[derive(Serialize)]
struct Summary {
    engine: String,
    profile: String,
    mode: String,
    n: usize,
    pass: usize,
    thin_shell: usize,
    chl: usize,
    thin_body: usize,
    error: usize,
    pass_pct: f64,
    /// Sites in the corpus tagged `diagnostic: true` (intended to fail).
    /// e.g. `areyouheadless`.
    diagnostic_n: usize,
    /// Sites NOT marked diagnostic = real-browsable sites.
    /// `production_n = n - diagnostic_n`.
    production_n: usize,
    /// Passes among non-diagnostic sites only.
    production_pass: usize,
    /// production_pass / production_n × 100.
    production_pass_pct: f64,
    t_launch_ms: u64,
    t_first_page_ready_ms: u64,
    rss_peak_mb: f64,
    ms_median: u64,
    ms_p95: u64,
    ms_p99: u64,
    wall_total_ms: u64,
    throughput_pages_per_min: f64,
    by_category: HashMap<String, CategoryStats>,
}

#[derive(Serialize, Default)]
struct CategoryStats {
    n: usize,
    pass: usize,
}

fn self_rss_mb() -> f64 {
    // Read /proc/self/statm — column 2 is RSS in pages.
    if let Ok(s) = fs::read_to_string("/proc/self/statm") {
        if let Some(pages) = s.split_whitespace().nth(1) {
            if let Ok(pages) = pages.parse::<u64>() {
                return pages as f64 * 4.0 / 1024.0;
            }
        }
    }
    0.0
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let mut args = std::env::args().skip(1);
    let profile_name = args
        .next()
        .expect("usage: sweep_metrics <profile> <corpus.json> <out.json>");
    let corpus_path = args.next().expect("missing corpus.json");
    let out_path = args.next().expect("missing out.json");

    let profile = match profile_name.as_str() {
        "chrome_148_macos" => browser_oxide::stealth::presets::chrome_148_macos(),
        "chrome_148_windows" => browser_oxide::stealth::presets::chrome_148_windows(),
        "firefox_135_macos" => browser_oxide::stealth::presets::firefox_135_macos(),
        "iphone_15_pro_safari_18" => browser_oxide::stealth::presets::iphone_15_pro_safari_18(),
        "pixel_9_pro_chrome_148" => browser_oxide::stealth::presets::pixel_9_pro_chrome_148(),
        other => panic!("unknown profile {other}"),
    };

    let use_pool = std::env::var("BROWSER_OXIDE_SWEEP_POOL").is_ok();
    let mode = if use_pool { "pool" } else { "cold" }.to_string();

    // FIX-E: per-site profile sampling for IP-clustering defence.
    // When `BROWSER_OXIDE_SAMPLE_PROFILE=1` is set AND the base profile
    // is chrome_148_macos, each navigate() gets a freshly-sampled variant
    // (screen / cores / RAM / canvas+audio seeds vary). For non-macos
    // profiles this currently no-ops — extend to other-OS samplers as
    // they ship.
    let sample_per_site =
        std::env::var("BROWSER_OXIDE_SAMPLE_PROFILE").is_ok() && profile_name == "chrome_148_macos";

    let corpus_bytes = fs::read(&corpus_path).expect("read corpus");
    let corpus: Vec<Site> = serde_json::from_slice(&corpus_bytes).expect("parse corpus");
    let total = corpus.len();

    let local = tokio::task::LocalSet::new();
    local
        .run_until(async move {
            let sweep_t0 = Instant::now();

            // Cold-start: time-to-first-page-ready. For the pool mode, we
            // pre-warm one Page; for the cold mode it's just the first
            // Page::navigate.
            let pool = if use_pool {
                Some(browser_oxide::PagePool::new(4))
            } else {
                None
            };

            let t_launch_ms;
            let t_first_page_ready_ms;
            if let Some(pool) = pool.as_ref() {
                let t0 = Instant::now();
                let seed = pool.acquire(Some(profile.clone())).await.expect("seed");
                t_launch_ms = t0.elapsed().as_millis() as u64;
                pool.release(seed);
                t_first_page_ready_ms = t_launch_ms; // pool acquire = first-page-ready
            } else {
                t_launch_ms = 0;
                t_first_page_ready_ms = 0;
            };

            let mut results: Vec<SiteResult> = Vec::with_capacity(total);
            let mut rss_peak: f64 = 0.0;
            // Atomic-checkpoint writer: every site appends to
            // `<out_path>.partial` so a cap-truncated kill (SIGTERM /
            // SIGKILL from a wrapping `timeout`) leaves a per-site log
            // readable by the aggregator. The
            // final summary still writes `out_path` atomically on
            // 126/126 completion.
            let partial_path = format!("{out_path}.partial");
            // Clear any stale partial from a prior crashed run.
            let _ = fs::remove_file(&partial_path);
            for (i, site) in corpus.iter().enumerate() {
                let t0 = Instant::now();
                let mut err: Option<String> = None;
                // Per-site profile (sampled or shared)
                let site_profile = if sample_per_site {
                    browser_oxide::stealth::presets::chrome_148_macos_sampled()
                } else {
                    profile.clone()
                };
                let (tag, body_len): (String, usize) = if use_pool {
                    let pool = pool.as_ref().unwrap();
                    match pool.navigate(&site.url, site_profile.clone()).await {
                        Ok(mut page) => {
                            let body = page.content();
                            let ec = browser_oxide::engine_classify(&body);
                            let r = (ec.tag.to_string(), ec.len);
                            pool.release(page);
                            r
                        }
                        Err(e) => {
                            err = Some(format!("{}", e).chars().take(200).collect());
                            ("ERROR".to_string(), 0)
                        }
                    }
                } else {
                    match browser_oxide::Page::navigate(&site.url, site_profile, 3).await {
                        Ok(mut page) => {
                            let body = page.content();
                            let ec = browser_oxide::engine_classify(&body);
                            (ec.tag.to_string(), ec.len)
                        }
                        Err(e) => {
                            err = Some(format!("{}", e).chars().take(200).collect());
                            ("ERROR".to_string(), 0)
                        }
                    }
                };
                let ms = t0.elapsed().as_millis() as u64;
                let rss = self_rss_mb();
                if rss > rss_peak {
                    rss_peak = rss;
                }
                let line = format!(
                    "sweep: [{}/{}] {} {} {} len={} ms={} rss={:.0}{}",
                    i + 1,
                    total,
                    site.cat,
                    site.name,
                    tag,
                    body_len,
                    ms,
                    rss,
                    err.as_ref().map(|e| format!(" err={}", e)).unwrap_or_default()
                );
                println!("{}", line);
                let sr = SiteResult {
                    cat: site.cat.clone(),
                    name: site.name.clone(),
                    url: site.url.clone(),
                    tag,
                    len: body_len,
                    ms,
                    rss_mb: (rss * 10.0).round() / 10.0,
                    err,
                };
                // Checkpoint to `<out>.partial` (one JSON line per site)
                // so cap-truncated runs leave a usable result trace.
                if let Ok(json_line) = serde_json::to_string(&sr) {
                    use std::io::Write;
                    if let Ok(mut f) = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&partial_path)
                    {
                        let _ = writeln!(f, "{json_line}");
                    }
                }
                results.push(sr);
            }

            let wall_total_ms = sweep_t0.elapsed().as_millis() as u64;

            // Aggregate
            let pass_count = results
                .iter()
                .filter(|r| r.tag == "L3-RENDERED" && r.len >= 15000)
                .count();
            let thin_shell = results
                .iter()
                .filter(|r| r.tag == "L3-RENDERED" && r.len >= 1000 && r.len < 15000)
                .count();
            let chl = results
                .iter()
                .filter(|r| r.tag.contains("CHL") || r.tag == "BLOCKED" || r.tag.contains("PaH"))
                .count();
            let thin_body = results
                .iter()
                .filter(|r| r.tag != "L3-RENDERED" && r.len < 1000 && r.err.is_none())
                .count();
            let error = results.iter().filter(|r| r.err.is_some()).count();

            // Dual-metric: production (non-diagnostic) sites only.
            // Diagnostic probes (`areyouheadless` etc.) are INTENDED to fail
            // and drag every engine's pass-rate down equally; the production
            // metric is the honest "% of real sites we can browse" number.
            let diagnostic_n = corpus.iter().filter(|s| s.diagnostic).count();
            let production_n = total - diagnostic_n;
            let production_pass = corpus
                .iter()
                .zip(results.iter())
                .filter(|(s, r)| {
                    !s.diagnostic && r.tag == "L3-RENDERED" && r.len >= 15000
                })
                .count();
            let production_pass_pct = if production_n > 0 {
                (100.0 * production_pass as f64 / production_n as f64 * 10.0).round() / 10.0
            } else {
                0.0
            };

            let mut timings: Vec<u64> = results.iter().map(|r| r.ms).collect();
            timings.sort_unstable();
            let ms_median = timings.get(timings.len() / 2).copied().unwrap_or(0);
            let ms_p95 = timings
                .get((timings.len() as f64 * 0.95) as usize)
                .copied()
                .unwrap_or(0);
            let ms_p99 = timings
                .get((timings.len() as f64 * 0.99) as usize)
                .copied()
                .unwrap_or(0);

            let mut by_category: HashMap<String, CategoryStats> = HashMap::new();
            for r in &results {
                let entry = by_category.entry(r.cat.clone()).or_default();
                entry.n += 1;
                if r.tag == "L3-RENDERED" && r.len >= 15000 {
                    entry.pass += 1;
                }
            }

            let throughput =
                60_000.0 * total as f64 / wall_total_ms.max(1) as f64;
            let summary = Summary {
                engine: "browser_oxide".to_string(),
                profile: profile_name.clone(),
                mode: mode.clone(),
                n: total,
                pass: pass_count,
                thin_shell,
                chl,
                thin_body,
                error,
                pass_pct: (100.0 * pass_count as f64 / total as f64 * 10.0).round() / 10.0,
                diagnostic_n,
                production_n,
                production_pass,
                production_pass_pct,
                t_launch_ms,
                t_first_page_ready_ms,
                rss_peak_mb: (rss_peak * 10.0).round() / 10.0,
                ms_median,
                ms_p95,
                ms_p99,
                wall_total_ms,
                throughput_pages_per_min: (throughput * 100.0).round() / 100.0,
                by_category,
            };

            let json = serde_json::json!({
                "summary": summary,
                "results": results,
            });
            fs::write(&out_path, serde_json::to_vec_pretty(&json).expect("serialize"))
                .expect("write out.json");

            eprintln!(
                "\n=== browser_oxide [{} / {}]: pass={}/{} ({}%) | production={}/{} ({}%) wall={}s rss_peak={}MB median={}ms p95={}ms ===",
                profile_name,
                mode,
                summary.pass,
                summary.n,
                summary.pass_pct,
                summary.production_pass,
                summary.production_n,
                summary.production_pass_pct,
                wall_total_ms / 1000,
                summary.rss_peak_mb,
                summary.ms_median,
                summary.ms_p95,
            );
            eprintln!("  -> {}", out_path);
        })
        .await;
}
