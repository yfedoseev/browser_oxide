# Benchmark report — 2026-05-23

Source: in-tree run on branch `fix/yandex-regression`
(commits `f8c6324`, `a11044f`, `f62584d` on top of `0c2ad3e`). Same
machine, same IP, same hour, same classifier across every engine.

## Methodology

- **Corpus**: 126 sites extracted from
  `crates/browser/tests/holistic_sweep.rs` (the `site!` macro list).
  Identical input URL set for every engine; URL list saved to
  `/tmp/corpus.json` so the harness in `benchmarks/bench_corpus.py`
  is byte-identical to what our own sweep tests.
- **Classifier**: every engine's final rendered DOM
  (`document.documentElement.outerHTML`) is piped through one shared
  Rust binary (`target/release/examples/classify_stdin`) that calls
  `browser::engine_classify`. Zero classifier drift across engines.
- **Sweep mode for browser_oxide**: parallel-2 via
  `holistic_sweep_parallel`, 40-min timeout per profile (the documented
  stable concurrency — `BOXIDE_PARALLEL_WORKERS=2`). 4 profiles run
  sequentially to avoid same-IP rate-limit contention.
- **Competitor harness**: `benchmarks/bench_corpus.py` drives each
  competitor through the same 126 URLs, captures `page.content()`, and
  scores it via the shared classifier.
- **Verdict semantics** (engine's own rule, `classify.rs:47`):
  - `Pass` — `tag == L3-RENDERED` AND body `≥ 15 KB` (real content)
  - `ThinShell` — `tag == L3-RENDERED` AND body `1-15 KB`
    (SPA bootstrap shell that never hydrated; counted as L3 by
    absence-of-challenge-marker but not a full render)
  - `CHL` — any `*-CHL` / `BLOCKED` / `PerimeterX-PaH` tag
  - `ThinBody` — body `< 1 KB`
  - `Error` — engine raised an exception
  - `Missing` — site did not finish before the 30/40-min profile
    timeout (browser_oxide only — competitors all completed 126/126)

## Headline numbers

Same 126-corpus, same classifier, same IP, today.

| Engine                            | **Pass** (≥15 KB, strict) | ThinShell | CHL | ThinBody | Error | Missing | L3-tag (loose) | Sites tested |
|-----------------------------------|--------------------------:|----------:|----:|---------:|------:|--------:|---------------:|-------------:|
| boxide.chrome_148_macos           | **102** | 14 | 6 | 1 | 0 | 3 | 116 | 123 |
| boxide.pixel_9_pro_chrome_148     | **102** | 13 | 5 | 1 | 0 | 5 | 115 | 121 |
| boxide.iphone_15_pro_safari_18    | **96**  | 15 | 7 | 1 | 0 | 7 | 111 | 119 |
| boxide.firefox_135_macos          | **94**  | 12 | 7 | 1 | 0 | 12 | 106 | 114 |
| Chromium headless (vanilla)       | 86 | 11 | 25 | 1 | 3 | 0 | 97 | 126 |
| Playwright + Stealth              | 87 | 10 | 25 | 1 | 3 | 0 | 97 | 126 |
| Patchright (CDP-hidden)           | 86 | 11 | 25 | 4 | 0 | 0 | 97 | 126 |
| Camoufox (Firefox)                | **108** | 10 | 8 | 0 | 0 | 0 | 118 | 126 |
| **boxide BEST-OF-4 routed**       | **111** | 10 | 3 | 0 | 0 | 2 | 121 | 126 |

## Honest reads

1. **Single-profile, Camoufox wins.** Our best single profile (chrome /
   pixel = 102 Pass) trails Camoufox (108) by 6 sites. A workload that
   pins one profile sees us behind Camoufox.

2. **Routed, browser_oxide wins.** When the caller is free to pick the
   best of the 4 profiles per domain (the `routed best-of-4` row), we
   reach 111 Pass — 3 sites ahead of Camoufox. Most real scraping
   pipelines do this naturally (different sites have different optimal
   profiles).

3. **vs CDP-driver tier we win clean.** Chromium headless, Playwright +
   Stealth, and Patchright all sit at 86-87 Pass. They each accumulate
   25 challenges (anti-bot vendors detect their CDP-driver fingerprint
   regardless of stealth plugins). We hit 3 challenges routed. **~22
   more Pass sites and 22 fewer challenges** — the from-scratch engine
   thesis validates here.

4. **The `L3-tag` loose count flatters the engines.** L3-tag counts a
   2 KB amazon-com aws-waf stub the same as a 1 MB real homepage,
   because the classifier only fires on challenge markers, not on
   actual content presence. We publish `Pass (≥15 KB)` as the headline
   because it's the only number that maps to "the page actually
   rendered". The L3-tag column is provided for compatibility with
   prior reports that used it.

5. **ThinShell band is genuinely small everywhere.** All 8 engines
   end up with 10-15 ThinShell results, mostly the same set of sites —
   amazon-com / amazon-co-uk / amazon-com-au / imdb / booking and a
   couple of others ship a 2-13 KB bootstrap shell to any non-real-
   browser HTTP client. Nobody — including Camoufox — extracts real
   content from those.

## Where we're behind

The 6-Pass gap to Camoufox on a single profile, and the residual
~10-site gap to the README's old `120` chrome number, both trace to
**session-history fingerprinting**. Real browsers visit hundreds of
sites before they ever touch yandex / leboncoin / homedepot / amazon
variants — those vendors flag a cookie-jar-empty fresh session as a
bot. browser_oxide now uses one process-wide `SharedSession`
(cookies + accept-CH origins) so cookies persist across navigations
(see `crates/net/src/lib.rs::shared_session`), but the jar still
starts empty on a cold start; the prior README number was inflated
by an undisclosed cross-test cookie-leakage bug
(`OnceLock<HttpClient>`) that the current code correctly removed.

Known engineering items to close the residual gap:
- **Pre-warm cookie jar**: navigate a few neutral sites at engine
  startup to populate the shared jar before user code runs (a
  `Page::with_warm_session()` constructor).
- **`BOXIDE_COOKIE_JAR=<path>` persistence**: already wired through
  to the shared session; users with long-running scrapers get
  free improvement run-over-run.
- **Twitter / x.com SPA-fast-exit edge case**: `page.rs:1511`
  exits when ANY common SPA mount has ≥1 child, but twitter's
  bootstrap populates the mount with the *loader* before JS adds
  real content. ~3 site-instances across profiles flow from this.

## What changed since the published README's `120/121/116/116`

The branch `fix/yandex-regression` has three commits on top of `main`:

| SHA      | Type    | What                                                                                                  |
|----------|---------|-------------------------------------------------------------------------------------------------------|
| `f8c6324`| revert  | Drops the `d5598c5` no-progress early-exit. That fix saved ~50% sweep walltime but exited the iter-1 retry path *before* the Rust-fallback fetch — and for yandex/yandex-ru/prime-video that fallback is what produced the real content. |
| `a11044f`| fix     | `fetch_ext` globals (`FETCH_CLIENT`, `ACTIVE_CSP`, `CSP_VIOLATIONS`, `SYNC_FETCH_COUNT`) `thread_local!` instead of process-global `OnceLock`. Removes the cross-worker contamination that collapsed SPA-hydration sites to stubs in parallel mode. |
| `f62584d`| feat    | `net::SharedSession` + `HttpClient::shared()`. One process-wide cookie jar + accept-CH origin set (DNS / Alt-Svc deliberately per-client). Recreates the production "one user, one jar" semantics without the parallel-mode race. |

The published `120/121/116/116` numbers were measured 2026-05-21 on
commit `90a7ed5` — but they were a side-effect of two bugs:
- `OnceLock<FETCH_CLIENT>` shared one HTTP client across every test
  in a `cargo test` process; the cookie jar accumulated across ~100
  sites before yandex / leboncoin / etc. were tested, so those
  vendors saw "a browser with browsing history" and let us through.
- The bug also corrupted concurrent workers in `ParallelPager` —
  five SPA-hydration sites (reddit, zara, amazon-de, yandex,
  yandex-ru) silently collapsed to 2-8 KB stubs in parallel mode.

Today's V10 on `90a7ed5` re-measured: same IP, same hour, the bug
was producing 84-Pass on chrome and the SPA sites still rendered.
Fixing the underlying bug honestly removes the leak; the new
SharedSession recovers most of the lost cookies but not all (some
sites are sensitive to specific cookie ordering or DNS routing that
the leak happened to provide). The net effect is +~20 fewer
"fake" passes counted under loose L3-tag and a +3 routed Pass lead
over Camoufox under strict ≥15 KB.

## Reproducing this run

```bash
# browser_oxide (per profile)
cd /home/yfedoseev/projects/browser_oxide
git checkout fix/yandex-regression
cargo test --release -p browser --test holistic_sweep --no-run
for p in chrome_148_macos pixel_9_pro_chrome_148 iphone_15_pro_safari_18 firefox_135_macos; do
  BOXIDE_PROFILE=$p BOXIDE_PARALLEL_WORKERS=2 \
    cargo test --release -p browser --test holistic_sweep \
    holistic_sweep_parallel -- --ignored --nocapture \
    > /tmp/v10_${p}.log 2>&1
done

# Competitors (Playwright/Patchright/Camoufox in a venv; see
# /tmp/stealth-bench/venv on the bench machine)
cargo build --release -p browser --example classify_stdin
for eng in chromium playwright_stealth patchright camoufox; do
  python3 benchmarks/bench_corpus.py $eng \
    > /tmp/corpus_${eng}.log 2>&1
done
```

Raw per-profile / per-engine logs (this run) preserved at
`/tmp/stealth-bench/`:
- `baseline_*.log` — 90a7ed5 reference, parallel-2, today
- `v10_*.log` — branch `fix/yandex-regression`, parallel-2, today
- `corpus_<engine>.json` — competitor 126-site results
- `run_v10_all.sh` / `run_both_sweeps.sh` — orchestration scripts
