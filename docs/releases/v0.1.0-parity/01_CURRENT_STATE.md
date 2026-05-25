# 01 — Current state (2026-05-24 baseline)

## Headline numbers — 126-site holistic corpus

All measurements 2026-05-24, single-run, same machine, same IP, same hour. Source: `/tmp/full_sweep_2026_05_24/*.json`.

### Pass rate (the customer-facing metric)

| Engine | **Pass** (L3 ≥15 KB) | ThinShell | CHL | ThinBody | Error | L3 loose |
|---|--:|--:|--:|--:|--:|--:|
| Camoufox | **113** | 7 | 6 | 2 | 1 | 120 |
| **BO routed best-of-4** | **108** | 12 | 5 | 3 | 0 | **120** |
| BO pixel_9_pro_chrome_148 | 102 | 16 | 6 | 4 | 0 | 118 |
| BO firefox_135_macos | 101 | 14 | 9 | 4 | 0 | 115 |
| BO chrome_148_macos | 99 | 17 | 7 | 5 | 0 | 116 |
| BO iphone_15_pro_safari_18 | 98 | 15 | 11 | 4 | 0 | 113 |
| Playwright | 88 | 9 | 25 | 7 | 4 | 97 |
| Patchright | 88 | 9 | 25 | 7 | 3 | 97 |
| Playwright + Stealth | 87 | 10 | 25 | 7 | 5 | 97 |

**Key reads:**
- BO routed matches Camoufox on **loose L3** (120 = 120) — same WAF defeat rate.
- The 5-site gap is entirely in **strict body ≥ 15 KB**: BO L3-renders 12 sites that ship < 15 KB; Camoufox only 7.
- BO **leads CDP-driver tier by ~15 Pass** (vs Playwright 88).

### Per-category pass rate

| Category | n | Camoufox | BO chrome | BO pixel | BO iphone | BO firefox | Notes |
|---|--:|--:|--:|--:|--:|--:|---|
| search | 8 | 8 | 8 | 8 | 8 | 8 | parity |
| reference | 5 | 5 | 5 | 5 | 5 | 5 | parity |
| gov-bank | 6 | 6 | 6 | 6 | 6 | 6 | parity |
| tech | 9 | 9 | 9 | 9 | 9 | 9 | parity |
| news | 10 | 10 | 10 | 10 | 10 | 10 | parity |
| antibot (probes) | 10 | 10 | 9 | 9 | 9 | 9 | areyouheadless is the loss |
| misc | 12 | 12 | 9 | 9 | 9 | 9 | imdb/yelp/duolingo cluster |
| streaming | 8 | 8 | 7 | 7 | 6 | 7 | hulu/netflix dependent |
| ru | 6 | 6 | 5 | 5 | 4 | 5 | wildberries |
| stores | 17 | 14 | 13 | 14 | 13 | 14 | etsy DataDome |
| social | 10 | 9 | 8 | 8 | 9 | 9 | reddit |
| amazon | 8 | **6** | 1 | 1 | 1 | 1 | **biggest single category gap** |
| travel | 8 | 8 | 5 | 5 | 4 | 5 | booking, tripadvisor |
| realestate | 4 | 4 | 3 | 3 | 4 | 3 | realtor Kasada |
| chl-known | 5 | 4 | 1 | 3 | 1 | 1 | canadagoose/hyatt/realtor + douyin |

## Memory (peak RSS, MB)

| Engine | RSS peak (reported) | RSS peak (corrected) |
|---|--:|--:|
| Camoufox | 48 | **~200-400** (measurement bug — see `09_MEMORY_OPTIMIZATION.md`) |
| BO pixel_9_pro_chrome_148 | 388 | 388 |
| BO chrome_148_macos | 419 | 419 |
| BO iphone_15_pro_safari_18 | 445 | 445 |
| BO firefox_135_macos | 472 | 472 |
| BO chrome POOL (97 sites, partial) | 1365 | 1365 |
| Playwright + Stealth | 5011 | 5011 |
| Playwright | 5618 | 5618 |
| Patchright | 5681 | 5681 |

The 48 MB Camoufox number was wrong. `benchmarks/bench_corpus_v2.py:256-267` picked the first /proc child whose `comm` contains "fox" and walked only its descendants — Firefox e10s content processes weren't captured. Fix applied 2026-05-24 (uncommitted); real Camoufox tree RSS is 200-400 MB on a 126-site sweep.

## Timing (per-page wall-clock, ms)

| Engine | median | p95 | p99 | total wall | throughput |
|---|--:|--:|--:|--:|--:|
| BO chrome POOL (97 sites) | 2.6 s | 9.7 s | 61.4 s | 6.9 min (proj 9.0) | **14.0/min** (proj) |
| Patchright | 3.5 s | 8.5 s | 13.3 s | 9.3 min | 13.6/min |
| Playwright | 3.5 s | 8.9 s | 22.7 s | 10.0 min | 12.6/min |
| Playwright + Stealth | 4.4 s | 23.6 s | 45.8 s | 16.7 min | 7.5/min |
| Camoufox | 5.6 s | 9.5 s | 42.5 s | 15.0 min | 8.4/min |
| BO cold (any profile) | 15.1 s | 92.9 s | 115.3 s | 46-54 min | 2.3-2.7/min |

Cold path is bottlenecked by per-URL V8 isolate creation (deno_core JsRuntime is ~50-70 MB and 200-300 ms to spin up). Pool path amortizes the isolate across pages.

## Methodology — what changed between 90a7ed5 and HEAD

The headline "121 → 108" perception is a methodology change, not a real regression.

| Aspect | 90a7ed5 | HEAD (2026-05-24) |
|---|---|---|
| Metric | "L3-RENDERED" count (loose) | "Pass" = L3-RENDERED AND body ≥ 15 KB (strict) |
| Single profile best | pixel_9_pro_chrome_147 = **121** | pixel_9_pro_chrome_148 loose = 118, strict = 102 |
| Routed | 123 | loose = 120, strict = 108 |
| Δ apples-to-apples (loose) | — | **-3** (within ±5 noise floor) |

The strict gate is the customer-relevant metric (a 2 KB AWS WAF stub shouldn't count as "we got the page"). It exposes 10-16 sites where the engine "renders" the WAF challenge document but never extracts real content.

See `docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md` for the ±5 noise floor characterization.

## Commits that nudged loose L3 down -3 since 90a7ed5

Each within noise but additive:

1. **`aecdf19`** — vendor strip. `default_solvers()` now returns empty. Lost: `relax_response_csp`, sec-cpt cookie-flip break, `_abck` learning, Kasada `x-kpsdk` learning. Mostly affects: etsy (DataDome), homedepot (Akamai sec-cpt), canadagoose/hyatt/realtor (Kasada).
2. **`f62584d`** — `SharedSession` (process-wide cookies + accept_ch). Per its own commit message: Δ = -1 to -3 vs 90a7ed5 baseline. Cookie/accept_ch bleed across origins during the sweep.
3. **`a11044f`** — `fetch_ext` thread-local. Real correctness fix (was a parallel bug). Side effect: removed an accidental cookie-jar-leak across tests that had been inflating loose counts (yandex/reddit/zara/amazon-de).

Plus uncommitted perf-pass changes (now partially reverted in fix B — see `04_TOOLING_SPEC.md` and `10_TIMING_OPTIMIZATION.md`):
- `page.rs:3389` build_page final drain 8 s → 200 ms (REVERTED to 8 s 2026-05-24)
- `humanize.js` `_sched` → `__bgSetTimeout` (kept — fixes the drain-pinning issue B addressed)
- `__syncCookiesFromNet` drain 1 s → 50 ms (kept; documented at `page.rs:3046` as "one async op ≪ 1 ms")

## The 8 hard-residual sites (no engine passes)

These are NOT in the recoverable surface for v0.1.0; they are the documented frontier.

| Site | Block | Camoufox result | Notes |
|---|---|---|---|
| amazon-jp | AWS WAF | 5635 bytes (also fails) | When the WAF rolls hard — Camoufox doesn't pass either |
| bestbuy | Akamai SPA shell | 7465 bytes | Cross-engine |
| homedepot | Akamai-CHL | 2638 bytes | sec-cpt; was passing on iPhone profile pre-strip |
| realtor | Kasada-CHL | 1772 bytes | Open SOTA frontier |
| canadagoose | Kasada-CHL | 740 bytes | Open SOTA frontier |
| hyatt | Kasada-CHL | 745 bytes | Open SOTA frontier |
| wildberries | SPA shell | 8924 bytes | Cross-engine |
| areyouheadless | antibot probe | 3668 bytes | Cross-engine probe (intentional fingerprint test) |

## File pointers for this section

- Raw sweep JSONs: `/tmp/full_sweep_2026_05_24/{bo,comp}_*.json`
- Corpus definition: `crates/browser/tests/holistic_sweep.rs:1-700`
- Classifier rules: `crates/browser/src/classify.rs`
- Benchmark report: `docs/BENCHMARK_2026_05_24.md`
- Performance investigation: `docs/PERFORMANCE_2026_05_24.md`
- Noise floor analysis: `docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md`
