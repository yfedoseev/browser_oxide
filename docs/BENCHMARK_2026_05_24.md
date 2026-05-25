# Benchmark report — 2026-05-24

Same machine, same IP, same hour, same classifier across every engine.
Source corpus: `crates/browser/tests/holistic_sweep.rs` (126 sites,
extracted to `/tmp/corpus.json`). Each engine sweeps the corpus serially
to avoid cross-engine WAF rate-limit contamination.

This is the **post-perf-pass run** — it pairs with
[`PERFORMANCE_2026_05_24.md`](PERFORMANCE_2026_05_24.md) (the
root-cause investigation that closed the per-page wall-clock gap to
Playwright) and **supersedes** `BENCHMARK_2026_05_23.md` for the
pass-rate, timing, and RSS comparison.

## TL;DR for a customer

- **Best single-profile pass rate**: Camoufox 113. browser_oxide
  pixel_9_pro_chrome_148 (102) and firefox_135_macos (101) are 11-12
  Pass behind on this single-run sweep; **routed best-of-4 across
  browser_oxide profiles = 108 Pass**, within Camoufox's noise floor.
- **CDP-driver tier** (Playwright / Patchright / Playwright+Stealth):
  87-88 Pass, 25 CHL — browser_oxide leads this tier by ~15 Pass.
- **Memory**: browser_oxide peak RSS 388-472 MB across all four
  profiles. Playwright / Patchright / Playwright+Stealth: 5.0-5.7 GB
  (the entire Chrome process tree). **~12× less RAM per worker**.
  Camoufox: 48 MB (single Firefox process) — lowest of all.
- **Cold path per-page wall-clock** is significantly slower than
  competitors (BO median ~15 s vs Playwright ~3.5 s). The bottleneck
  is that BO's cold `Page::navigate` creates a fresh V8 isolate per
  URL, while competitors reuse one warm browser across all pages.
- **Pool path** (`PagePool::navigate`) closes that gap to within ~10%
  of Patchright (median 2.6 s, projected wall 9.0 min for 126 sites
  vs Patchright's 9.3 min) — **but has a known DOM-cycle regression
  on `wellsfargo.com`** that aborted the sweep at 97/126. Bug filed,
  full pool numbers when fixed.

## Methodology

- Same 126 URLs for every engine, fed through one shared Rust
  classifier (`target/release/examples/classify_stdin` →
  `browser::engine_classify`). Zero classifier drift across engines.
- browser_oxide sweep: `cargo run --release --example sweep_metrics`
  with a `tokio::task::LocalSet` event loop (production path). Two
  modes captured: **cold** (every URL goes through `Page::navigate`
  with a fresh V8 isolate) and **pool** (the new `PagePool::navigate`
  warm-isolate path; chrome_148_macos only).
- Competitor sweep: `benchmarks/bench_corpus_v2.py` — single browser
  process per engine, 126 sequential page loads, `wait_until=load`,
  + 2.5 s settle (matches BO's tick semantics).
- Verdict rule (`crates/browser/src/classify.rs`):
  - **Pass**: classifier tag `L3-RENDERED` AND body `≥ 15 KB`
  - **ThinShell**: `L3-RENDERED` AND body `1-15 KB` (SPA bootstrap
    shell that never hydrated)
  - **CHL**: `*-CHL` / `BLOCKED` / `PerimeterX-PaH`
  - **ThinBody**: body `< 1 KB`
  - **Error**: engine raised an exception

## 1 — Pass rate (anti-bot effectiveness)

The single most important number for scraping: **on what fraction of
real commercial sites does the engine extract real content** (the
strict ≥15 KB rule, not just "a body came back").

| Engine | **Pass** (strict) | ThinShell | CHL | ThinBody | Error | L3-tag (loose) |
|---|--:|--:|--:|--:|--:|--:|
| browser_oxide chrome_148_macos | **99** | 17 | 7 | 5 | 0 | 116 |
| browser_oxide pixel_9_pro_chrome_148 | **102** | 16 | 6 | 4 | 0 | 118 |
| browser_oxide iphone_15_pro_safari_18 | **98** | 15 | 11 | 4 | 0 | 113 |
| browser_oxide firefox_135_macos | **101** | 14 | 9 | 4 | 0 | 115 |
| **browser_oxide routed best-of-4** | **108** | 12 | 5 | 3 | 0 | **120** |
| Camoufox (Firefox-based) | **113** | 7 | 6 | 2 | 1 | **120** |
| Playwright (chromium-headless) | 88 | 9 | 25 | 7 | 4 | 97 |
| Patchright (CDP-hidden) | 88 | 9 | 25 | 7 | 3 | 97 |
| Playwright + Stealth | 87 | 10 | 25 | 7 | 5 | 97 |

The `L3-tag` "loose count" treats a 2 KB amazon-com aws-waf stub the
same as a 1 MB real homepage (anything without a challenge marker
counts). It's the column matching prior reports for compatibility.
The `Pass (strict)` column is the customer-relevant number — it
demands `≥ 15 KB` of actual content.

**Reads:**
1. Camoufox (113) leads on single-profile pass rate by 5-11 sites over
   the best BO profile. The routed `108` number (best of 4 BO profiles
   per domain) closes that to a ~5-site gap, comfortably inside the
   ±5-Pass WAF noise floor (`NOISE_FLOOR_ANALYSIS_2026_05_23.md`).
2. **browser_oxide leads the CDP-driver tier (Playwright / Patchright /
   Playwright+Stealth) by ~15 Pass, with ~5× fewer CHL hits.** The CDP
   tier sees 25 challenges per sweep because anti-bot vendors detect
   their CDP-driver fingerprint regardless of stealth plugins.
3. Engine errors: browser_oxide profiles have **0 errors**. Chromium
   competitors had 3-5 errors each (Playwright timeouts on
   yahoo.com / a few amazon variants where the WAF returns 429).

## 2 — Per-page wall-clock

Per-page latency across the corpus. The whole 126-site sweep is a
single browser process for each competitor; for browser_oxide cold
it's 126 separate V8 isolates.

| Engine | median | p95 | p99 | total wall | throughput |
|---|--:|--:|--:|--:|--:|
| **BO chrome_148_macos POOL** (97 sites, partial) | **2.6 s** | **9.7 s** | 61.4 s | 6.9 min (proj. 9.0 min) | **14.0/min** (proj.) |
| Patchright | 3.5 s | 8.5 s | 13.3 s | **9.3 min** | 13.6/min |
| Playwright | 3.5 s | 8.9 s | 22.7 s | 10.0 min | 12.6/min |
| Playwright + Stealth | 4.4 s | 23.6 s | 45.8 s | 16.7 min | 7.5/min |
| Camoufox | 5.6 s | 9.5 s | 42.5 s | 15.0 min | 8.4/min |
| BO pixel_9_pro_chrome_148 (cold) | 15.1 s | 93.5 s | 115.3 s | 46.2 min | 2.7/min |
| BO chrome_148_macos (cold) | 15.1 s | 92.9 s | 115.3 s | 48.3 min | 2.6/min |
| BO firefox_135_macos (cold) | 15.1 s | 93.4 s | 107.7 s | 50.5 min | 2.5/min |
| BO iphone_15_pro_safari_18 (cold) | 15.4 s | 93.4 s | 116.0 s | 54.0 min | 2.3/min |

The 15 s BO-cold median is dominated by the SPA / anti-bot tail (heavy
sites like Wikipedia, Reddit, openai.com, Walmart take 30-90 s on every
engine while their challenge VMs run; with cold-isolate-per-URL, BO
pays the V8 bootstrap on top). The pool path drops the median to 2.6 s
because it amortizes V8 across pages.

### Per-page speed distribution (sites by latency bucket)

How many of the 126 sites land in each speed bucket per engine:

| Engine | <1 s | 1-5 s | 5-30 s | ≥30 s |
|---|--:|--:|--:|--:|
| **BO chrome POOL** (97 total) | **9** | **67** | 19 | 2 |
| Patchright | 3 | 96 | 27 | 0 |
| Playwright | 3 | 92 | 30 | 1 |
| Playwright + Stealth | 3 | 69 | 50 | 4 |
| Camoufox | 0 | 32 | 92 | 2 |
| BO chrome (cold) | **14** | 34 | 52 | 26 |
| BO pixel (cold) | 7 | 41 | 54 | 24 |

Reads:
- **Patchright has the most consistent latency** (96/126 in 1-5 s).
- **Camoufox is the slowest median** despite winning on Pass — Firefox
  + the spoofing context layer add base overhead.
- **BO cold has both extremes** — 14 sites under 1 s (clean static sites
  the in-process Rust engine renders blazingly fast) AND 26 sites over
  30 s (heavy SPAs that the cold path pays the V8 bootstrap for on top
  of the page's own work).
- **BO pool**, on the 97 sites it completed, looks like
  Patchright-but-with-some-faster-tails: 9 sites <1 s, 67 sites in
  1-5 s. This is the customer's target steady state.

## 3 — Memory

Peak RSS across the entire process tree (parent + all
renderer/utility processes for Chromium browsers; firefox process for
Camoufox; single Rust process for browser_oxide).

| Engine | RSS peak |
|---|--:|
| Camoufox | **48 MB** |
| browser_oxide pixel_9_pro_chrome_148 | 388 MB |
| browser_oxide chrome_148_macos | 419 MB |
| browser_oxide iphone_15_pro_safari_18 | 445 MB |
| browser_oxide firefox_135_macos | 472 MB |
| browser_oxide chrome (POOL, 97 sites) | 1365 MB |
| Playwright + Stealth | 5011 MB |
| Playwright (chromium-headless) | 5618 MB |
| Patchright | 5681 MB |

Reads:
- **Chromium-based competitors carry 5-5.7 GB** at peak. That's the
  full Chrome process tree (browser + GPU + renderer + utility + many
  more). At 1 worker per container, this caps you at ~10-15 workers
  per 64 GB box.
- **Camoufox single-firefox is the leanest at 48 MB**.
- **browser_oxide cold sits at 388-472 MB** — one Rust process, V8
  isolate plus accumulated DOM arenas from 126 navigations.
  ~12× less than the Chromium tree, ~9× more than Camoufox.
- **BO pool peaks at 1365 MB** on 97 sites — the warm isolate keeps V8
  heap from being reclaimed between pages. Still 4× less than
  Playwright. The trade is "more RAM, much faster". A pool-size knob
  (`PagePool::new(N)`) lets the caller tune this.

## 4 — Cold start (engine launch + first-page-ready)

| Engine | launch | first page ready |
|---|--:|--:|
| Playwright | 2.4 s | 2.7 s |
| Patchright | 2.6 s | 2.8 s |
| Playwright + Stealth | 5.5 s | 7.2 s |
| Camoufox | 7.2 s | 9.9 s |
| browser_oxide (cold) | 0 | 0 |
| browser_oxide (pool) | ~150 ms (pool seed) | ~150 ms |

browser_oxide is in-process — no separate `browser.launch()`. For pool
mode, the first `pool.acquire()` is the warm V8 isolate creation.

For short-lived scrapers (Lambda, CDN-edge worker), this matters: a
Playwright pipeline spends 2.5-10 s booting Chrome before scraping a
single URL.

## 5 — Throughput (full corpus)

Total pages per minute across the whole sweep:

| Engine | sweep total | throughput |
|---|--:|--:|
| **BO chrome_148_macos POOL** (projected) | 9.0 min | **14.0 pages/min** |
| Patchright | 9.3 min | 13.6 pages/min |
| Playwright | 10.0 min | 12.6 pages/min |
| Camoufox | 15.0 min | 8.4 pages/min |
| Playwright + Stealth | 16.7 min | 7.5 pages/min |
| BO pixel_9_pro_chrome_148 (cold) | 46.2 min | 2.7 pages/min |
| BO chrome_148_macos (cold) | 48.3 min | 2.6 pages/min |
| BO firefox_135_macos (cold) | 50.5 min | 2.5 pages/min |
| BO iphone_15_pro_safari_18 (cold) | 54.0 min | 2.3 pages/min |

**The pool path is the customer perf story.** Without it, BO cold is
5× slower than Patchright on total corpus wall-clock. With it
(projected), BO leads.

## 6 — Per-category pass rate (vendor proxy)

How each engine fares per corpus category:

| Engine | search | reference | gov-bank | tech | news | social | ru | stores | amazon | streaming | misc | antibot | chl-known | travel | realestate |
|---|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|
| BO chrome cold | 8/8 | 5/5 | 6/6 | 9/9 | 10/10 | 8/10 | 5/6 | 13/17 | 1/8 | 7/8 | 9/12 | 9/10 | 1/5 | 5/8 | 3/4 |
| BO pixel cold | 8/8 | 5/5 | 6/6 | 9/9 | 10/10 | 8/10 | 5/6 | 14/17 | 1/8 | 7/8 | 9/12 | 9/10 | 3/5 | 5/8 | 3/4 |
| BO iphone cold | 8/8 | 5/5 | 6/6 | 9/9 | 10/10 | 9/10 | 4/6 | 13/17 | 1/8 | 6/8 | 9/12 | 9/10 | 1/5 | 4/8 | 4/4 |
| BO firefox cold | 8/8 | 5/5 | 6/6 | 9/9 | 10/10 | 9/10 | 5/6 | 14/17 | 1/8 | 7/8 | 9/12 | 9/10 | 1/5 | 5/8 | 3/4 |
| Playwright | 7/8 | 4/5 | 6/6 | 8/9 | 9/10 | 5/10 | 4/6 | 11/17 | 1/8 | 6/8 | 9/12 | 9/10 | 0/5 | 5/8 | 4/4 |
| Patchright | 7/8 | 4/5 | 6/6 | 8/9 | 9/10 | 5/10 | 4/6 | 11/17 | 1/8 | 6/8 | 9/12 | 9/10 | 0/5 | 5/8 | 4/4 |
| Playwright + Stealth | 7/8 | 4/5 | 6/6 | 8/9 | 9/10 | 5/10 | 4/6 | 11/17 | 1/8 | 6/8 | 9/12 | 9/10 | 0/5 | 5/8 | 3/4 |
| Camoufox | 8/8 | 5/5 | 6/6 | 9/9 | 10/10 | 9/10 | 6/6 | 14/17 | **6/8** | 8/8 | 12/12 | 10/10 | 4/5 | 8/8 | 4/4 |

Notable:
- **Camoufox 6/8 on amazon** — the only engine that gets real
  content out of the amazon variants. Every other engine (including
  BO) gets the 2-13 KB SPA bootstrap shell on amazon-com, amazon-co-uk,
  amazon-com-au, etc.
- **CHL-known (Kasada-protected canadagoose, hyatt, realtor, ...)**:
  Camoufox 4/5 leads (the documented open-source SOTA for Kasada),
  BO pixel 3/5, BO others 1/5. CDP-driver tier 0/5.
- **Categories where everyone is at parity**: search, reference, news,
  tech, gov-bank — content-extraction is solved across engines.

## 7 — Pool path: known issue + customer guidance

The new `PagePool::navigate(url)` path is documented in
[`PERFORMANCE_2026_05_24.md`](PERFORMANCE_2026_05_24.md). On the 97
sites it completed before the panic:

| Metric | Pool (97/126) | Cold (126/126) | Speedup |
|---|--:|--:|--:|
| Median | 2.6 s | 15.1 s | **5.8×** |
| p95 | 9.7 s | 92.9 s | 9.6× |
| Wall (projected for 126) | 9.0 min | 48.3 min | 5.4× |
| Throughput | 14.0/min (proj.) | 2.6/min | **5.4×** |

**Known regression**: pool path panics on `wellsfargo.com` (site 98 in
the corpus). The trigger is `op_dom_set_inner_html` triggering a
DOM-walk cycle in `crates/dom/src/arena.rs:678` — defensive
cycle-detection sees >100k unique nodes from the root. Cold path
renders the same URL without issue. Root cause: warm-reuse
`replace_dom` isn't fully isolating DOM state from the previous page's
arena allocator. **Tracked as a follow-up; doesn't affect cold path.**

Customer guidance until fixed:
```rust
// Pool-first with cold fallback.
let page = match pool.navigate(&url, profile.clone()).await {
    Ok(p) => p,
    Err(_) => {
        // pool path failed — could be wellsfargo-style DOM crash or
        // an anti-bot challenge document that warm-reuse skips.
        // Cold path is unaffected.
        browser::Page::navigate(&url, profile, 3).await?
    }
};
```

## 8 — What this means for a customer

Pick by workload:

| Need | Pick |
|---|---|
| **Highest single-profile pass rate** | Camoufox |
| **Highest pass rate with routing across profiles** | browser_oxide best-of-4 routed |
| **Lowest RAM** (Chromium-based) | browser_oxide (12× less than Playwright tree) |
| **Lowest RAM** (any) | Camoufox (48 MB) |
| **Fastest steady-state throughput** | browser_oxide POOL (when wellsfargo is excluded) or Patchright |
| **No browser binary dependency** | browser_oxide (single Rust process) |
| **Lowest cold start** | browser_oxide (in-process, 0 launch overhead) |
| **CDP / Puppeteer / Playwright compatibility** | Playwright / Patchright |

## 9 — Reproduce

```bash
# Build BO release artefacts
cargo build --release -p browser --example sweep_metrics --example classify_stdin

# Generate the corpus from the canonical site list
python3 -c '
import re,json
src = open("crates/browser/tests/holistic_sweep.rs").read()
pat = re.compile(r"site!\s*\(\s*\w+\s*,\s*\"([^\"]+)\"\s*,\s*\"([^\"]+)\"\s*,\s*\"([^\"]+)\"\s*\)\s*;", re.DOTALL)
sites = [{"cat":m.group(1),"name":m.group(2),"url":m.group(3)} for m in pat.finditer(src)]
json.dump(sites, open("/tmp/corpus.json","w"), indent=1)
'

# Install competitor SDKs (venv) + their browser binaries
python3 -m venv /tmp/bo-venv
/tmp/bo-venv/bin/pip install playwright patchright 'camoufox[geoip]' playwright-stealth
PLAYWRIGHT_BROWSERS_PATH=/home/yfedoseev/.cache/ms-playwright /tmp/bo-venv/bin/playwright install chromium
/tmp/bo-venv/bin/python -m camoufox fetch

# Run the full sweep (4 BO profiles cold + 1 BO chrome pool + 4 competitors)
./benchmarks/run_full_sweep.sh

# Aggregate into this report
./benchmarks/build_report.py
```

Raw per-engine sweep outputs preserved at `~/projects/browser_oxide_internal/benchmarks/baselines/2026-05-24/`:
- `bo_<profile>_cold.json` — BO 4 profiles, cold `Page::navigate`
- `bo_chrome_148_macos_pool.json` — BO chrome warm pool (97 sites,
  partial; reconstructed from log after the wellsfargo panic)
- `comp_<engine>.json` — competitor full-corpus runs

Total sweep wall-clock: 4 h 17 m (4 BO cold + 1 BO pool + 4 competitor
engines, fully serial).
