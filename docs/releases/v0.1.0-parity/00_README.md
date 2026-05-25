# v0.1.0-parity — Beat-Camoufox release plan

**Status:** planning complete; v0.1.0 execution-ready  
**Created:** 2026-05-24  
**Last expanded:** 2026-05-24 (16 → 44 chapters after the deep cross-vendor research arc)  
**Target metric:** routed best-of-N strict-pass on 126-site holistic corpus  
**Bar to clear:** ≥ 115 (Camoufox best measured = 113)

This directory is a self-contained engineering plan. Any contributor should be able to pick up any chapter and execute without further interviews. Each chapter is structured as: **Context → Concrete file:line pointers → Acceptance criteria → How to validate**.

## What "parity" means here

- **Pass rate**: ≥ Camoufox 113 strict (`L3-RENDERED` AND body ≥ 15 KB) routed best-of-4 across BO profiles, with a ≥ 2-site margin.
- **Per-profile**: ≥ 110 strict on at least one single profile (Camoufox single = 113).
- **Memory**: peak RSS comparable to Camoufox's *real* tree RSS (200-400 MB measured correctly, NOT the buggy 48 MB number); below Playwright/Patchright by ≥ 5×.
- **Throughput**: pool path ≥ Patchright (currently within ~10% per `docs/PERFORMANCE_2026_05_24.md`).
- **Stability**: no panics; pool path fixes `wellsfargo.com` DOM-cycle regression (see `docs/BENCHMARK_2026_05_24.md §7`).

## How to navigate 44 chapters

If you only have **30 minutes**: read **00, 01, 02, 42, 43** (this README + state + gap + synthesis pair). That's the whole story.

If you only have **2 hours**: add **04, 11, 14** (tooling + profile strategy + testing).

If you're **picking up engine work**: read the relevant per-cluster chapter (05/06/07/08/25/26) + the relevant detection-category chapter (38/39/40/41) + chapter 43 for prioritization.

## Doc index (44 chapters / ~30,000 lines / 1.5 MB)

### Structural (read first)

| # | File | Lines | What |
|---|---|--:|---|
| 00 | `00_README.md` | this | Index + success scorecard + how to navigate |
| 01 | `01_CURRENT_STATE.md` | 125 | Headline numbers per profile + methodology delta vs 90a7ed5 |
| 02 | `02_GAP_ANALYSIS.md` | 261 | Per-site root cause for all 10 Camoufox-only-pass sites |
| 13 | `13_FILE_LOCATIONS_INDEX.md` | 181 | One-page lookup for every file:line in the plan |
| 15 | `15_OPEN_QUESTIONS.md` | — | Research backlog + resolved-items log |

### Methodology + tooling

| # | File | Lines | What |
|---|---|--:|---|
| 03 | `03_BENCHMARK_METHODOLOGY.md` | 648 | Sweep harness, classifier, profiles, ±5 noise floor, multi-run aggregation |
| 04 | `04_TOOLING_SPEC.md` | 840 | `sweep_metrics --capture` mode + BO↔Camoufox auto-diff (BLOCKING for Phases 1-3) |
| 14 | `14_TESTING_VALIDATION.md` | 495 | L1-L5 validation layers + CI workflow + A/B harness |
| 34 | `34_PER_VENDOR_TEST_HARNESS.md` | 664 | Per-vendor deterministic regression tests |

### Per-cluster fix playbooks (the recoverable surface)

| # | File | Lines | What |
|---|---|--:|---|
| 05 | `05_SPA_HYDRATION_CLUSTER.md` | 785 | reddit / duolingo / booking / douyin with debug commands |
| 06 | `06_AWS_WAF_SOLVER.md` | 573 | challenge.js → token; 3 solver alternatives |
| 07 | `07_DATADOME_PRIMITIVES.md` | 667 | Restore as engine-internal primitives (no vendor names) |
| 08 | `08_KASADA_FRONTIER.md` | 219 | canadagoose / hyatt / realtor — research-bound (post-v0.1.0) |
| 25 | `25_CLOUDFLARE_DEEP.md` | 1360 | Managed Challenge + Turnstile + Bot Fight + JSC (4 products) |
| 26 | `26_AKAMAI_BMP_DEEP.md` | 788 | sensor_data v2/v3 + sec-cpt + _abck; aecdf19 restoration |

### Profile + competitive strategy

| # | File | Lines | What |
|---|---|--:|---|
| 11 | `11_PER_PROFILE_STRATEGY.md` | 642 | chrome_148 / pixel / iphone / firefox routing decision tree |
| 12 | `12_COMPETITIVE_LANDSCAPE.md` | 445 | Camoufox / Playwright / Patchright / PW-Stealth — what each does |
| 19 | `19_PROFILE_EXPANSION_PLAN.md` | 408 | Add safari_18_macos / chrome_148_windows etc. (already coded!) |
| 27 | `27_VENDOR_COMPETITIVE_MATRIX.md` | 574 | Per-vendor BO vs Camoufox vs PW head-to-head |

### Engine surface (fingerprint + API coverage)

| # | File | Lines | What |
|---|---|--:|---|
| 16 | `16_STEALTH_FINGERPRINT_AUDIT.md` | 693 | Every JS API mask status + which vendor probes each |
| 17 | `17_WEB_API_PARITY_MATRIX.md` | 893 | Implemented / stubbed / missing per Web Platform spec area |
| 18 | `18_ANTI_BOT_VENDOR_COOKBOOK.md` | 1165 | Encyclopedia: 14 vendors with markers + mechanism |
| 23 | `23_TLS_HTTP_FINGERPRINT_REFERENCE.md` | 529 | boring2 4.15.15, JA3/JA4, HTTP/2 SETTINGS, quarterly refresh |

### Cross-cutting detection categories (the synthesis material)

| # | File | Lines | What |
|---|---|--:|---|
| 38 | `38_VISUAL_AUDIO_FINGERPRINTING.md` | 1083 | Canvas / WebGL / audio — 11/10/8 of 12 vendors |
| 39 | `39_NETWORK_LAYER_FINGERPRINTING.md` | 1005 | TLS / HTTP/2 / WebRTC — 12/11/5 of 12 vendors |
| 40 | `40_TIMING_BEHAVIORAL.md` | 1173 | performance.now / RAF / mouse / keystroke — wiring gaps |
| 41 | `41_POW_WASM_WORKER_PATTERNS.md` | 1403 | Convergent vendor patterns; MessageChannel blocker |

### Vendor encyclopedia (customer onboarding + cross-vendor synthesis)

| # | File | Lines | What |
|---|---|--:|---|
| 28 | `28_AWS_WAF_EXTENDED.md` | 1087 | Full product family (4 distinct), 40+ signal inventory, WASM PoW |
| 29 | `29_F5_SHAPE_SECURITY.md` | 709 | Financial/airline; predicted partial pass |
| 30 | `30_ARKOSE_LABS.md` | 715 | FunCaptcha + MatchKey; visible vs invisible |
| 31 | `31_FASTLY_NGWAF.md` | 641 | Signal Sciences; "we win" — boring2 matches |
| 32 | `32_RADWARE_BOT_MANAGER.md` | 702 | ShieldSquare; IDBA punishes humanize.js gaps |
| 35 | `35_IMPERVA_ABP.md` | 571 | Now Thales (2023); Reese84 mechanism |
| 36 | `36_ATO_SPECIALISTS.md` | 711 | Castle / Sift / Forter / Akamai AP |
| 37 | `37_REBLAZE_SUCURI_DDAP.md` | 645 | Mid-tier + ATO product cousins |

### Memory / timing / perf

| # | File | Lines | What |
|---|---|--:|---|
| 09 | `09_MEMORY_OPTIMIZATION.md` | 638 | Worker leak fix + Camoufox measurement bug post-mortem |
| 10 | `10_TIMING_OPTIMIZATION.md` | 722 | Cold-vs-pool + drain fix + wellsfargo panic |
| 20 | `20_MEMORY_BUDGET.md` | 712 | Per-subsystem MB budget breakdown |
| 21 | `21_V8_SNAPSHOT_PARALLEL_COLD.md` | 719 | Snapshot warming + parallel sweep detailed impl |

### Operations + risk

| # | File | Lines | What |
|---|---|--:|---|
| 22 | `22_PRODUCTION_DEPLOYMENT.md` | 1030 | k8s/Lambda/Cloud Run patterns; 11 env vars; cost model |
| 24 | `24_RISK_REGISTER.md` | 904 | 17 risks + dependency map; deno_core 90 versions behind |
| 33 | `33_QUARTERLY_PROBE_ROTATION_LOG.md` | 743 | Per-vendor rotation cadence + CI automation |

### Synthesis (read after the per-cluster + per-category chapters)

| # | File | Lines | What |
|---|---|--:|---|
| 42 | `42_HOLISTIC_VISION.md` | 322 | Vendor × technique matrix; top-12 surfaces by Σ; 7 recurring patterns |
| 43 | `43_STRATEGIC_GAP_ASSESSMENT.md` | 328 | Top-10 fixes ranked by ROI; v0.1.0/v0.2.0/v0.3.0 phasing; 2027 forecast |

## Execution order (chapter 43 § 3)

The recommended order, with measured effort:

```
v0.1.0 MUST-HAVE (13-16 engineering-days) — close the gap to Camoufox 113 + go ahead
─────────────────────────────────────────────────────────────────────────────────
[1] WebGL prototype mask sweep              1-2d   moves 11 vendors    chapter 38
[2] WebGL per-profile golden snapshot       1d     moves 11 vendors    chapter 38
[3] Function.toString mass mask sweep       2d     moves 11 vendors    chapter 16
[4] Canvas toDataURL parity test            2d     moves 10 vendors    chapter 38
[5] Wire keystroke generator (exists!)      1-2d   moves 8+ vendors    chapter 40
[6] Wire two-level seed (exists!)           1d     moves 8+ vendors    chapter 40
[7] Wire performance.timeOrigin             0.5d   moves 8 vendors     chapter 40
[8] MessageChannel + MessagePort impl       3-5d   moves 6+ + duolingo chapter 17/41
[9] RAF jitter                              1d     moves 7+ vendors    chapter 40
[10] Vendor-detect markers extension        1h     detection coverage  chapter 18
[11] reddit HTMLFormElement.elements        0.5d   reddit flip         chapter 05/17
[12] 3-run aggregated baseline + validate   ~12h sweep + 2d analysis   chapter 14

v0.1.0 SHOULD-HAVE (5-10 days additional)
─────────────────────────────────────────
Touch event synthesis (iPhone/Pixel), Cloudflare cf-mitigated detection,
Canvas emoji per-profile snapshot, JA4 ground-truth capture.

v0.2.0 (3-6 months — chapter 43 §4)
───────────────────────────────────
Theme A: vendor_solvers crate (AWS WAF / DataDome / Akamai) → private repo
Theme B: measurement infra (capture mode shipped, multi-run wired to CI)
Theme C: perf (V8 snapshot warming, parallel cold sweep)
Theme D: production (k8s YAMLs, Lambda handler, healthz/readyz)

v0.3.0 (6-12 months — chapter 43 §5)
────────────────────────────────────
Kasada K2-DIFF push (chapter 08), profile expansion (chapter 19),
behavioral biometrics depth (Σ-Λ Plamondon mouse curves).
```

## Success scorecard (must be true before declaring v0.1.0)

- [ ] **Pass**: ≥ 115 strict routed best-of-4 — 3-run median across all 4 profiles
- [ ] **Pass**: ≥ 110 strict on at least one single profile
- [ ] **Pass**: zero functional regressions vs HEAD on existing `≥ 15 KB` sites
- [ ] **WebGL**: prototype mask sweep complete; per-profile golden snapshots in tree
- [ ] **Stealth**: `Function.prototype.toString` mass mask sweep complete
- [ ] **humanize.js**: keystroke generator wired; two-level seed wired; timeOrigin wired; RAF jitter added
- [ ] **MessageChannel**: paired-port routing implemented; duolingo flips
- [ ] **reddit**: HTMLFormElement.elements implemented; site flips
- [ ] **Detection**: vendor-detect markers extended to 11 headers + 20 body markers
- [ ] **Memory**: peak RSS < 1.5× honestly-measured Camoufox on 126-site sweep
- [ ] **Memory**: no monotonic leak (RSS at site 126 within 2× of RSS at site 1, with worker reaper enabled)
- [ ] **Throughput**: pool path ≥ 13.5 pages/min (= Patchright) on the same corpus
- [ ] **Stability**: `wellsfargo.com` panic in pool mode fixed (see `crates/dom/src/arena.rs` 100k-node cycle detector)
- [ ] **Docs**: all 44 chapters in this directory complete + reviewed
- [ ] **CI**: regression-gate CI job that re-runs the 126 sweep weekly with 3-run aggregation and fails on > -3 site drop from baseline

## Source-of-truth artifacts

Public:
- `crates/browser/tests/holistic_sweep.rs` — the 126-site corpus definition
- `crates/browser/src/classify.rs` — the verdict classifier
- `docs/BENCHMARK_2026_05_24.md` — narrative report of the baseline sweep
- `docs/PERFORMANCE_2026_05_24.md` — per-page perf investigation
- `docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md` — WAF variance characterization (±5 sites is noise)
- `CLAUDE.md` — workspace conventions; scope rules on vendor solvers

Private (in `~/projects/browser_oxide_internal/`):
- `benchmarks/baselines/2026-05-24/` — raw sweep JSONs + logs (canonical baseline)
- `ab_harness/tl/` — captured real-Chrome Kasada `/tl` sensor reference (chapter 08 K2-DIFF input)
- `docs/kasada_ips_analysis/` — Kasada VM analysis + decryption tools
- `vendor_solvers/` — Akamai/Kasada/DataDome/Cloudflare solver impls (per CLAUDE.md scope)

## What's committed vs uncommitted

All 44 chapters are committed and pushed to `origin/main`. Three engine fixes from this planning effort are also committed:
- **Fix A** — Camoufox RSS measurement (`benchmarks/bench_corpus_v2.py:256`) — commit `816a06b`
- **Fix B** — `build_page` drain restore 200ms → 8s + humanize `__bgSetTimeout` — commit `3091460`
- **Fix C** — Worker reap on `Page::drop` — commit `3091460`

Next step per chapter 43 § 3: execute the v0.1.0 MUST-HAVE list.
