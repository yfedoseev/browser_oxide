//! W4.3 — 5-run sweep variance characterization.
//!
//! Reads all sweep logs matching `/tmp/sweep_<profile>*.log` (or a path
//! supplied via the `BOXIDE_VARIANCE_LOGS` env var as a colon-separated
//! list) and reports per-(profile, site) pass-rate distribution.
//!
//! Output classifies each site into one of four buckets per profile:
//!   - STABLE_L3   — 100% L3-RENDERED across all runs
//!   - STABLE_FAIL — 100% non-L3 (deterministic block)
//!   - NOISY_PASS  — >50% pass rate but not 100% (flaky-towards-pass)
//!   - NOISY_FAIL  — ≤50% pass rate but not 0% (flaky-towards-fail)
//!
//! Single-run sweep diffs are contaminated by ~±2 sites of variance per
//! profile (~±8 across the 4-profile union); this lets every subsequent
//! patch decision separate signal from noise. Per PLAN.md §6 W4.3.
//!
//! Run: `cargo test -p browser --test sweep_variance variance_report --
//!       --ignored --nocapture`

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

#[derive(Default, Clone)]
struct Tally {
    pass: u32,
    fail: u32,
}

impl Tally {
    fn total(&self) -> u32 {
        self.pass + self.fail
    }
    fn pass_rate(&self) -> f64 {
        if self.total() == 0 {
            0.0
        } else {
            self.pass as f64 / self.total() as f64
        }
    }
}

fn collect_log_paths() -> Vec<PathBuf> {
    if let Ok(env) = std::env::var("BOXIDE_VARIANCE_LOGS") {
        return env
            .split(':')
            .map(PathBuf::from)
            .filter(|p| p.exists())
            .collect();
    }
    let mut paths = Vec::new();
    let tmp = std::path::Path::new("/tmp");
    if let Ok(entries) = fs::read_dir(tmp) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("sweep_") && name.ends_with(".log") && !name.contains("runner") {
                paths.push(e.path());
            }
        }
    }
    paths.sort();
    paths
}

/// Extract the profile name from `sweep_<profile>[_suffix].log`. The
/// suffix (e.g. `_pre_classifier_fix`) is preserved so multiple sweep
/// runs against the same profile are counted as distinct "runs" for
/// that profile under its base name.
fn profile_from_path(p: &std::path::Path) -> Option<String> {
    let stem = p.file_stem()?.to_string_lossy();
    let after = stem.strip_prefix("sweep_")?;
    // Drop optional trailing suffix after the recognized profile root.
    for known in [
        "chrome_130_macos",
        "pixel_9_pro_chrome_147",
        "iphone_15_pro_safari_18",
        "firefox_135_macos",
    ] {
        if after == known || after.starts_with(&format!("{known}_")) {
            return Some(known.to_string());
        }
    }
    None
}

fn classify_outcome(out: &str) -> bool {
    out == "L3-RENDERED"
}

/// Parse a single sweep log into `(profile_run_id, site) -> pass`. Each
/// log file is one full sweep, so we treat the file path as the run id.
fn parse_log(path: &std::path::Path) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    let Ok(content) = fs::read_to_string(path) else {
        return out;
    };
    for line in content.lines() {
        if !line.starts_with("holistic-end:") {
            continue;
        }
        // Format: holistic-end: <ts> <cat> <name> <outcome> len=... nav_ms=... drop_ms=... url=...
        let mut fields = line.split_whitespace();
        let _ = fields.next(); // "holistic-end:"
        let _ = fields.next(); // ts
        let _ = fields.next(); // cat
        let Some(name) = fields.next() else { continue };
        let Some(outcome) = fields.next() else {
            continue;
        };
        out.push((name.to_string(), classify_outcome(outcome)));
    }
    out
}

/// Aggregator. Returns BTreeMap<(profile, site), Tally>.
fn aggregate(paths: &[PathBuf]) -> BTreeMap<(String, String), Tally> {
    let mut agg: BTreeMap<(String, String), Tally> = BTreeMap::new();
    for path in paths {
        let Some(profile) = profile_from_path(path) else {
            continue;
        };
        for (site, pass) in parse_log(path) {
            let entry = agg.entry((profile.clone(), site)).or_default();
            if pass {
                entry.pass += 1;
            } else {
                entry.fail += 1;
            }
        }
    }
    agg
}

fn classify(tally: &Tally) -> &'static str {
    if tally.total() < 2 {
        return "SINGLE_RUN";
    }
    let r = tally.pass_rate();
    if r >= 0.999 {
        "STABLE_L3"
    } else if r <= 0.001 {
        "STABLE_FAIL"
    } else if r > 0.5 {
        "NOISY_PASS"
    } else {
        "NOISY_FAIL"
    }
}

#[tokio::test]
#[ignore]
async fn variance_report() {
    let paths = collect_log_paths();
    if paths.is_empty() {
        println!(
            "no sweep logs found (looked at /tmp/sweep_*.log or BOXIDE_VARIANCE_LOGS env var)"
        );
        return;
    }

    println!("\n=== sweep variance over {} log files ===", paths.len());
    for p in &paths {
        println!("  {}", p.display());
    }
    println!();

    let agg = aggregate(&paths);

    // Per-profile noisy-site count + stable totals.
    let mut by_profile: BTreeMap<String, (u32, u32, u32, u32)> = BTreeMap::new(); // stable_l3, stable_fail, noisy_pass, noisy_fail
    let mut noisy_lines: Vec<String> = Vec::new();
    for ((profile, site), tally) in &agg {
        let cls = classify(tally);
        let entry = by_profile.entry(profile.clone()).or_default();
        match cls {
            "STABLE_L3" => entry.0 += 1,
            "STABLE_FAIL" => entry.1 += 1,
            "NOISY_PASS" => entry.2 += 1,
            "NOISY_FAIL" => entry.3 += 1,
            _ => {}
        }
        if matches!(cls, "NOISY_PASS" | "NOISY_FAIL") {
            noisy_lines.push(format!(
                "  {profile} {site} pass={}/{} rate={:.1}% [{cls}]",
                tally.pass,
                tally.total(),
                tally.pass_rate() * 100.0
            ));
        }
    }

    println!("=== per-profile classification ===");
    println!(
        "{:<32} {:>10} {:>10} {:>10} {:>10}",
        "profile", "STABLE_L3", "STABLE_FAIL", "NOISY_PASS", "NOISY_FAIL"
    );
    for (profile, (s_l3, s_fail, n_pass, n_fail)) in &by_profile {
        println!("{profile:<32} {s_l3:>10} {s_fail:>10} {n_pass:>10} {n_fail:>10}");
    }

    if !noisy_lines.is_empty() {
        println!("\n=== noisy sites (per-profile) ===");
        noisy_lines.sort();
        for l in noisy_lines {
            println!("{l}");
        }
    }

    // Routing-union noise floor — count of sites where the union L3
    // outcome flips between runs depending on which run is picked.
    // (For each site, does it have AT LEAST one profile-run pass?)
    // This characterizes the union-level variance the user observes.
    let mut union_by_site: BTreeMap<String, (u32, u32)> = BTreeMap::new(); // (any_pass_across_runs, total_runs_observed)
                                                                           // Group runs by (path basename) so we can compute per-run union.
    let mut runs_by_path: Vec<(PathBuf, Vec<(String, String, bool)>)> = Vec::new();
    for p in &paths {
        let Some(profile) = profile_from_path(p) else {
            continue;
        };
        let parsed = parse_log(p);
        runs_by_path.push((
            p.clone(),
            parsed
                .into_iter()
                .map(|(s, b)| (profile.clone(), s, b))
                .collect(),
        ));
    }
    // Bucket runs into sweep-batches by (suffix-after-profile) so each
    // 4-profile batch becomes one "run" of the routing union.
    let mut batches: BTreeMap<String, Vec<(String, String, bool)>> = BTreeMap::new();
    for (p, rows) in runs_by_path {
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let suffix = {
            let mut s = stem.to_string();
            for known in [
                "chrome_130_macos",
                "pixel_9_pro_chrome_147",
                "iphone_15_pro_safari_18",
                "firefox_135_macos",
            ] {
                let needle = format!("sweep_{known}");
                if let Some(rest) = stem.strip_prefix(&needle) {
                    s = rest.trim_start_matches('_').to_string();
                    break;
                }
            }
            if s.is_empty() {
                "default".to_string()
            } else {
                s
            }
        };
        batches.entry(suffix).or_default().extend(rows);
    }
    for (batch_name, rows) in &batches {
        let mut union: std::collections::BTreeMap<String, bool> = std::collections::BTreeMap::new();
        for (_profile, site, pass) in rows {
            let entry = union.entry(site.clone()).or_insert(false);
            *entry = *entry || *pass;
        }
        let l3_count = union.values().filter(|v| **v).count();
        let total = union.len();
        println!(
            "\nunion[{batch_name}]: {l3_count}/{total} L3 across {} per-profile rows",
            rows.len()
        );
    }

    let total_sites = agg.len();
    println!("\ntotal (profile, site) pairs observed: {total_sites}");
}
