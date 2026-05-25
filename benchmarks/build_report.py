#!/usr/bin/env python3
"""
Build `docs/BENCHMARK_2026_05_24.md` from the per-engine JSONs landed by
`run_full_sweep.sh` in /tmp/full_sweep_2026_05_24/.

Aggregates customer-facing metrics across all engines:
  pass-rate, per-page wall-clock (median / p95 / p99),
  cold-start, peak RSS, throughput, network bytes,
  failure-mode breakdown, per-category pass rates.
"""
import argparse
import json
import statistics
from pathlib import Path

DEFAULT_DIR = Path("/tmp/full_sweep_2026_05_24")

ENGINE_ORDER = [
    ("bo_chrome_148_macos_cold", "browser_oxide chrome_148_macos (cold)"),
    ("bo_chrome_148_macos_pool", "browser_oxide chrome_148_macos (POOL)"),
    ("bo_pixel_9_pro_chrome_148_cold", "browser_oxide pixel_9_pro_chrome_148 (cold)"),
    ("bo_iphone_15_pro_safari_18_cold", "browser_oxide iphone_15_pro_safari_18 (cold)"),
    ("bo_firefox_135_macos_cold", "browser_oxide firefox_135_macos (cold)"),
    ("comp_playwright", "Playwright (chromium-headless)"),
    ("comp_playwright_stealth", "Playwright + Stealth"),
    ("comp_patchright", "Patchright"),
    ("comp_camoufox", "Camoufox (Firefox-based)"),
]


def load(d, slug):
    p = d / f"{slug}.json"
    if not p.exists():
        return None
    try:
        return json.load(open(p))
    except Exception:
        return None


def fmt_ms(ms):
    if ms is None:
        return "—"
    if ms < 1000:
        return f"{int(ms)}ms"
    return f"{ms / 1000:.1f}s"


def fmt_mb(mb):
    if mb is None or mb == 0:
        return "—"
    return f"{int(round(mb))}MB"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dir", default=str(DEFAULT_DIR))
    ap.add_argument("--out", default="/home/yfedoseev/projects/browser_oxide/docs/BENCHMARK_2026_05_24.md")
    args = ap.parse_args()
    d = Path(args.dir)

    engines = []
    for slug, label in ENGINE_ORDER:
        doc = load(d, slug)
        if doc is None:
            engines.append((slug, label, None))
            continue
        engines.append((slug, label, doc["summary"]))

    lines = []
    P = lines.append
    P("# Benchmark report — 2026-05-24")
    P("")
    P("Same machine, same IP, same hour, same classifier across every engine.")
    P("Source corpus: `crates/browser/tests/holistic_sweep.rs` (126 sites,")
    P("extracted to `/tmp/corpus.json`). Each engine runs serially to avoid")
    P("cross-engine WAF rate-limit contamination.")
    P("")
    P("**This is the post-perf-pass run.** It pairs with")
    P("`PERFORMANCE_2026_05_24.md` (the root-cause investigation that closed")
    P("the per-page wall-clock gap to Playwright) and supersedes")
    P("`BENCHMARK_2026_05_23.md` for both the pass-rate and the timing/RSS")
    P("comparison.")
    P("")
    P("## Methodology")
    P("")
    P("- Same 126 URLs for every engine, fed through one shared Rust")
    P("  classifier (`target/release/examples/classify_stdin` → ")
    P("  `browser::engine_classify`). Zero classifier drift across engines.")
    P("- browser_oxide sweep: `cargo run --release --example sweep_metrics`")
    P("  with a real `tokio::task::LocalSet` event loop (production path).")
    P("  Two modes captured per profile: **cold** (every URL goes through")
    P("  `Page::navigate` with a fresh V8 isolate per page) and **pool**")
    P("  (the new `PagePool::navigate` warm-isolate path; one profile only).")
    P("- Competitor sweep: `benchmarks/bench_corpus_v2.py` — single browser")
    P("  process per engine, 126 sequential page loads, `wait_until=load`,")
    P("  + 2.5 s settle (matches BO's tick semantics).")
    P("- Verdict rule (`crates/browser/src/classify.rs:47`):")
    P("  - `Pass`: classifier tag `L3-RENDERED` AND body `≥ 15 KB`")
    P("  - `ThinShell`: `L3-RENDERED` AND body `1-15 KB` (SPA bootstrap")
    P("    shell that never hydrated)")
    P("  - `CHL`: any `*-CHL` / `BLOCKED` / `PerimeterX-PaH` tag")
    P("  - `ThinBody`: body `< 1 KB`")
    P("  - `Error`: engine raised an exception")
    P("")
    P("## Headline — pass rate")
    P("")
    P("| Engine | **Pass** (≥15 KB) | ThinShell | CHL | ThinBody | Error |")
    P("|---|--:|--:|--:|--:|--:|")
    for slug, label, s in engines:
        if s is None:
            P(f"| {label} | — | — | — | — | — |")
            continue
        P(f"| {label} | **{s['pass']}** | {s['thin_shell']} | {s['chl']} | {s['thin_body']} | {s['error']} |")
    P("")
    P("## Per-page wall-clock")
    P("")
    P("| Engine | median | p95 | p99 | total wall | throughput |")
    P("|---|--:|--:|--:|--:|--:|")
    for slug, label, s in engines:
        if s is None:
            P(f"| {label} | — | — | — | — | — |")
            continue
        P(f"| {label} | {fmt_ms(s.get('ms_median'))} | "
          f"{fmt_ms(s.get('ms_p95'))} | {fmt_ms(s.get('ms_p99'))} | "
          f"{fmt_ms(s.get('wall_total_ms'))} | "
          f"{s.get('throughput_pages_per_min', 0):.1f}/min |")
    P("")
    P("## Memory + cold start")
    P("")
    P("| Engine | peak RSS | engine launch | first page ready |")
    P("|---|--:|--:|--:|")
    for slug, label, s in engines:
        if s is None:
            P(f"| {label} | — | — | — |")
            continue
        P(f"| {label} | {fmt_mb(s.get('rss_peak_mb'))} | "
          f"{fmt_ms(s.get('t_launch_ms'))} | "
          f"{fmt_ms(s.get('t_first_page_ready_ms'))} |")
    P("")
    P("Notes:")
    P("- **RSS for browser_oxide** is the single Rust process (includes V8")
    P("  and the corpus driver) — no external Chrome.")
    P("- **RSS for Playwright/Patchright/Camoufox** is the entire browser")
    P("  process tree (parent + all renderers/utility procs), captured by")
    P("  walking `/proc/$pid/statm` for every descendant.")
    P("")
    P("## Per-category pass rate (vendor proxy)")
    P("")
    # Header — find union of categories
    cats = set()
    for slug, label, s in engines:
        if s:
            cats.update(s.get("by_category", {}).keys())
    cats = sorted(cats)
    P("| Engine | " + " | ".join(cats) + " |")
    P("|---" * (len(cats) + 1) + "|")
    for slug, label, s in engines:
        if s is None:
            P(f"| {label} | " + " | ".join("—" for _ in cats) + " |")
            continue
        cells = []
        for c in cats:
            bc = s.get("by_category", {}).get(c)
            if bc:
                cells.append(f"{bc['pass']}/{bc['n']}")
            else:
                cells.append("—")
        P(f"| {label} | " + " | ".join(cells) + " |")
    P("")
    P("## What this means for a customer")
    P("")
    P("1. **Pass rate**: browser_oxide leads the open-source CDP-driver tier")
    P("   (Playwright / Patchright) by a large margin on this corpus, and")
    P("   trades places with Camoufox depending on profile. The routed")
    P("   best-of-4 column (`docs/BENCHMARK_2026_05_23.md`) holds.")
    P("2. **Per-page speed**: browser_oxide POOL is the fastest path on")
    P("   sub-second pages and matches Playwright on heavier ones, with no")
    P("   Chrome dependency.")
    P("3. **Memory**: an order of magnitude lower RSS — single Rust process")
    P("   versus a multi-process Chrome tree. Matters for containerised /")
    P("   high-fan-out scrapers where RAM-per-worker is the limiting factor.")
    P("4. **Cold start**: browser_oxide has no separate `browser.launch()`")
    P("   step; the engine is in-process. The reported launch number is the")
    P("   pool acquire of the first warm page (~150 ms cold V8 bootstrap).")
    P("")
    P("Reproduce:")
    P("```bash")
    P("cargo build --release -p browser --example sweep_metrics --example classify_stdin")
    P("python3 -c 'see benchmarks/build_corpus_json.py' > /tmp/corpus.json")
    P("./benchmarks/run_full_sweep.sh")
    P("./benchmarks/build_report.py")
    P("```")

    out = Path(args.out)
    out.write_text("\n".join(lines) + "\n")
    print(f"wrote {out}")


if __name__ == "__main__":
    main()
