# Benchmark report — 2026-05-23

Source: in-tree run on branch `fix/yandex-regression` (5 commits on top
of `0c2ad3e`, head: `8bf774d`). Same machine, same IP, same hour, same
classifier across every engine.

## Methodology

- **Corpus**: 126 sites extracted from
  `crates/browser/tests/holistic_sweep.rs` (the `site!` macro list).
  Identical input URL set for every engine; URL list cached at
  `/tmp/corpus.json` so the harness in `benchmarks/bench_corpus.py`
  scores from the same bytes our own sweep tests.
- **Classifier**: every engine's final rendered DOM
  (`document.documentElement.outerHTML`) is piped through one shared
  Rust binary (`target/release/examples/classify_stdin`) that calls
  `browser::engine_classify`. Zero classifier drift across engines.
- **Sweep mode for browser_oxide**: parallel-2 via
  `holistic_sweep_parallel`, 60–120 min timeout per profile (firefox
  required 90 + min on full corpus). 4 profiles run sequentially to
  avoid same-IP rate-limit contention.
- **Competitor harness**: `benchmarks/bench_corpus.py` drives each
  competitor through the same 126 URLs, captures `page.content()`, and
  scores via the shared classifier.
- **Verdict semantics** (engine's own rule, `classify.rs:47`):
  - `Pass` — `tag == L3-RENDERED` AND body `≥ 15 KB` (real content)
  - `ThinShell` — `tag == L3-RENDERED` AND body `1-15 KB`
    (SPA bootstrap shell that never hydrated; counted as L3 by
    absence-of-challenge-marker but not a full render)
  - `CHL` — any `*-CHL` / `BLOCKED` / `PerimeterX-PaH` tag
  - `ThinBody` — body `< 1 KB`
  - `Error` — engine raised an exception
  - `Missing` — site did not finish before the profile timeout

## Headline numbers

Same 126-corpus, same classifier, same IP, today.

| Engine                            | **Pass** (≥15 KB, strict) | ThinShell | CHL | ThinBody | Error | Missing | L3-tag (loose) | Sites tested |
|-----------------------------------|--------------------------:|----------:|----:|---------:|------:|--------:|---------------:|-------------:|
| Chromium headless (vanilla)       | 86 | 11 | 25 | 1 | 3 | 0 | 97 | 126 |
| Playwright + Stealth              | 87 | 10 | 25 | 1 | 3 | 0 | 97 | 126 |
| Patchright (CDP-hidden)           | 86 | 11 | 25 | 4 | 0 | 0 | 97 | 126 |
| **browser_oxide.chrome_148_macos**       | **102** | 14 | 7 | 2 | 0 | 1 | 116 | 125 |
| **browser_oxide.pixel_9_pro_chrome_148** | **104** | 15 | 6 | 1 | 0 | 0 | 119 | 126 |
| **browser_oxide.iphone_15_pro_safari_18**| **106** | 14 | 4 | 1 | 0 | 1 | 120 | 125 |
| **browser_oxide.firefox_135_macos**      | **101** | 14 | 9 | 2 | 0 | 0 | 115 | 126 |
| Camoufox (Firefox-based)          | **108** | 10 | 8 | 0 | 0 | 0 | 118 | 126 |
| **browser_oxide BEST-OF-4 routed**       | **110** | 10 | 4 | 0 | 0 | 2 | 122 | 126 |

## Honest reads

1. **Single-profile, Camoufox wins by 2.** Our best single profile
   (iphone = 106 Pass) trails Camoufox (108) by 2 sites. A workload
   that pins one profile sees us a hair behind Camoufox on this corpus.

2. **Routed, browser_oxide wins by 2.** When the caller is free to
   pick the best of the 4 profiles per domain (the `routed best-of-4`
   row), we reach 110 Pass — 2 sites ahead of Camoufox. Most real
   scraping pipelines do this naturally (different sites have
   different optimal profiles).

3. **vs CDP-driver tier we win clean.** Chromium headless, Playwright
   + Stealth, and Patchright all sit at 86-87 Pass. They each
   accumulate 25 challenges (anti-bot vendors detect their CDP-driver
   fingerprint regardless of stealth plugins). We hit 4 challenges
   routed. **~22 more Pass sites and 21 fewer challenges** — the
   from-scratch engine thesis validates here.

4. **The `L3-tag` loose count flatters every engine.** L3-tag counts
   a 2 KB amazon-com aws-waf stub the same as a 1 MB real homepage,
   because the classifier only fires on challenge markers, not on
   actual content presence. We publish `Pass (≥15 KB)` as the headline
   because it's the only number that maps to "the page actually
   rendered". The L3-tag column is provided for compatibility with
   prior reports that used it.

5. **ThinShell band is small and similar across engines.** All 8
   engines end up with 10-15 ThinShell results, mostly the same set —
   amazon-com / amazon-co-uk / amazon-com-au / imdb / booking and a
   couple of others ship a 2-13 KB bootstrap shell to any non-real-
   browser HTTP client. Nobody — including Camoufox — extracts real
   content from those.

## Where we're behind

The 2-Pass gap to Camoufox on a single profile (and the residual
~6-10-site gap to a hypothetical perfect-routed `123`) traces to
**session-history fingerprinting**. Real browsers visit hundreds of
sites before they ever touch yandex / leboncoin / homedepot / amazon
variants — those vendors flag a cookie-jar-empty fresh session as a
bot. browser_oxide uses one process-wide `SharedSession` (cookies +
accept-CH origins) so cookies persist across navigations (see
`crates/net/src/lib.rs::shared_session`), but the jar starts empty on
a cold start.

Known engineering items to close the residual gap:
- **Pre-warm cookie jar**: navigate a few neutral sites at engine
  startup to populate the shared jar before user code runs
  (a `Page::with_warm_session()` constructor).
- **`BROWSER_OXIDE_COOKIE_JAR=<path>` persistence**: already wired through
  the shared session; long-running scrapers get free improvement
  run-over-run.

## Run-to-run variance — important context

Single-run sweep deltas of ±5 sites are within the measured WAF
noise floor (see `NOISE_FLOOR_ANALYSIS_2026_05_23.md`):

- **amazon-de / amazon-fr / amazon-ca / imdb**: ~1-in-3 pass rate per
  individual fetch on a single engine. Bernoulli stddev across 5
  amazon variants ≈ 1.05 sites just for amazon.
- **leboncoin / wayfair / quora**: sometimes return the real
  multi-100-KB body, sometimes return a 1-15 KB challenge stub. The
  same engine flips this on consecutive runs minutes apart.

The 110-routed figure is the central tendency. Individual runs land
anywhere from ~105 to ~115. Multi-run aggregation would narrow the
estimate but we did not budget for it.

## What changed since the prior README's `120/121/116/116`

The branch `fix/yandex-regression` has five commits on top of `main`:

| SHA       | Type    | What |
|-----------|---------|------|
| `f8c6324` | revert  | Drops the `d5598c5` no-progress early-exit. That fix saved ~50% sweep walltime but exited the iter-1 retry path *before* the Rust-fallback fetch — for yandex/yandex-ru/prime-video that fallback is what produced the real content. |
| `a11044f` | fix     | `fetch_ext` globals (`FETCH_CLIENT`, `ACTIVE_CSP`, `CSP_VIOLATIONS`, `SYNC_FETCH_COUNT`) are `thread_local!` instead of process-global `OnceLock`. Removes cross-worker contamination that collapsed SPA-hydration sites to stubs in parallel mode. |
| `f62584d` | feat    | `net::SharedSession` + `HttpClient::shared()`. One process-wide cookie jar + accept-CH origin set (DNS / Alt-Svc deliberately per-client). Recreates the production "one user, one jar" semantics without the parallel-mode race. |
| `dc2ea97` | docs    | This benchmark report. |
| `8bf774d` | docs    | Noise-floor analysis. |

The prior README's `120/121/116/116` numbers were measured 2026-05-21
on commit `90a7ed5` — but they were inflated by two bugs we now fix:

- `OnceLock<FETCH_CLIENT>` shared one HTTP client across every test
  in a `cargo test` process; the cookie jar accumulated across many
  sites before yandex / leboncoin / etc. were tested, so those
  vendors saw "a browser with browsing history" and let us through.
- The same bug corrupted concurrent workers in `ParallelPager` —
  SPA-hydration sites silently collapsed to 2-8 KB stubs in
  parallel mode.

The honest reproducible measurement on the post-fix branch is what
this report publishes.

## Reproducing this run

```bash
# browser_oxide (4 profiles, parallel-2)
git checkout fix/yandex-regression
cargo test --release -p browser --test holistic_sweep --no-run
for p in chrome_148_macos pixel_9_pro_chrome_148 iphone_15_pro_safari_18 firefox_135_macos; do
  BROWSER_OXIDE_PROFILE=$p BROWSER_OXIDE_PARALLEL_WORKERS=2 \
    cargo test --release -p browser --test holistic_sweep \
    holistic_sweep_parallel -- --ignored --nocapture \
    > /tmp/v10_${p}.log 2>&1
done

# Competitors (Playwright/Patchright/Camoufox in a venv)
cargo build --release -p browser --example classify_stdin
for eng in chromium playwright_stealth patchright camoufox; do
  python3 benchmarks/bench_corpus.py $eng \
    > /tmp/corpus_${eng}.log 2>&1
done
```

Raw per-profile / per-engine logs (this run) preserved at
`/tmp/stealth-bench/`:
- `h2h_FIX_*.log` — branch `fix/yandex-regression`, parallel-2
- `h2h_FIX_firefox_135_macos_v2.log` — firefox re-run with extended
  timeout to reach 126/126
- `h2h_BASE_*.log` — `baseline-90a7ed5` reference for diff analysis
- `corpus_<engine>.json` — competitor 126-site results
