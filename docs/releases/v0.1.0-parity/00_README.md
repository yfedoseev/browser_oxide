# v0.1.0-parity — Beat-Camoufox release plan

**Status:** planning  
**Created:** 2026-05-24  
**Target metric:** routed best-of-N strict-pass on 126-site holistic corpus  
**Bar to clear:** ≥ 115 (Camoufox best measured = 113)

This directory is a self-contained engineering plan. Any contributor should be able to pick up any chapter and execute without further interviews. Each chapter is structured as: **Context → Concrete file:line pointers → Acceptance criteria → How to validate**.

## What "parity" means here

- **Pass rate**: ≥ Camoufox 113 strict (`L3-RENDERED` AND body ≥ 15 KB) routed best-of-4 across BO profiles, with a ≥ 2-site margin.
- **Per-profile**: ≥ 110 strict on at least one single profile (Camoufox single = 113).
- **Memory**: peak RSS comparable to Camoufox's *real* tree RSS (200-400 MB measured correctly, NOT the buggy 48 MB number); below Playwright/Patchright by ≥ 5×.
- **Throughput**: pool path ≥ Patchright (currently within ~10% per `docs/PERFORMANCE_2026_05_24.md`).
- **Stability**: no panics; pool path fixes `wellsfargo.com` DOM-cycle regression (see `docs/BENCHMARK_2026_05_24.md §7`).

## Doc index

| # | File | What it covers | Owner agent / scope |
|---|---|---|---|
| 00 | `00_README.md` | This file. Plan overview, success criteria, doc index. | structural |
| 01 | `01_CURRENT_STATE.md` | Headline numbers per profile, what's measured, methodology change between 90a7ed5 and HEAD. | structural |
| 02 | `02_GAP_ANALYSIS.md` | Every Camoufox-only-pass site categorized with root cause, evidence, and reference data. | structural |
| 03 | `03_BENCHMARK_METHODOLOGY.md` | How sweeps run. Corpus, classifier, profiles, noise floor, multi-run aggregation requirements. | tooling |
| 04 | `04_TOOLING_SPEC.md` | Per-site capture mode for `sweep_metrics`, BO↔Camoufox auto-diff tool. Specs + acceptance. | tooling |
| 05 | `05_SPA_HYDRATION_CLUSTER.md` | reddit / duolingo / booking / douyin. Per-site debug playbook with known evidence. | SPA work |
| 06 | `06_AWS_WAF_SOLVER.md` | challenge.js reverse, token mechanism, solver design (amazon-de/in/com-au/jp + imdb). | solver work |
| 07 | `07_DATADOME_PRIMITIVES.md` | Restore the post-`aecdf19` DataDome behaviour as engine-internal primitives. etsy/tripadvisor/yelp. | primitives work |
| 08 | `08_KASADA_FRONTIER.md` | Hard residual — canadagoose / hyatt / realtor. Research-bound. | research |
| 09 | `09_MEMORY_OPTIMIZATION.md` | Worker leak fix, V8 heap, per-profile RSS breakdown, measurement-bug post-mortem. | memory work |
| 10 | `10_TIMING_OPTIMIZATION.md` | Cold path vs pool, isolate reuse, per-profile latency targets, wellsfargo pool bug. | perf work |
| 11 | `11_PER_PROFILE_STRATEGY.md` | chrome_148 / pixel / iphone / firefox. Strengths, weaknesses, routing rules. | structural |
| 12 | `12_COMPETITIVE_LANDSCAPE.md` | Camoufox, Playwright, Patchright, PW-Stealth — what each does technically. | research |
| 13 | `13_FILE_LOCATIONS_INDEX.md` | Every important file:line referenced in this plan. One-page lookup. | structural |
| 14 | `14_TESTING_VALIDATION.md` | Regression gates, CI integration, multi-run sweep aggregation, A/B harness. | tooling |
| 15 | `15_OPEN_QUESTIONS.md` | Known unknowns, deferred decisions, research backlog. | structural |

## Execution order (recommended)

```
┌─── Phase 0 (3-5 days) ───┐
│  04 Tooling — capture    │  Blocking dep for everything else
│  03 Methodology — multi  │  Multi-run aggregation
│  14 Testing — A/B harness│
└───────────┬──────────────┘
            │
            ├──► Phase 1 (1-2 wk) ── 05 SPA cluster ───┐
            ├──► Phase 2 (2-4 wk) ── 06 AWS WAF ───────┼─► Phase 4 ── 09 Memory (parallel)
            └──► Phase 3 (1-2 wk) ── 07 DataDome ──────┘     10 Timing (parallel)
                                                              08 Kasada (research)
```

## Success scorecard (must be true before declaring v0.1.0)

- [ ] **Pass**: ≥ 115 strict routed best-of-4 — 3-run median across all 4 profiles
- [ ] **Pass**: ≥ 110 strict on at least one single profile
- [ ] **Pass**: zero functional regressions vs HEAD on existing `≥ 15 KB` sites (defined by `crates/browser/tests/holistic_sweep.rs`)
- [ ] **Memory**: peak RSS < 1.5× honestly-measured Camoufox on 126-site sweep
- [ ] **Memory**: no monotonic leak (RSS at site 126 within 2× of RSS at site 1, with worker reaper enabled)
- [ ] **Throughput**: pool path ≥ 13.5 pages/min (= Patchright) on the same corpus
- [ ] **Stability**: `wellsfargo.com` panic in pool mode fixed (see `crates/dom/src/arena.rs` 100k-node cycle detector)
- [ ] **Docs**: all 16 chapters in this directory complete + reviewed
- [ ] **CI**: regression-gate CI job that re-runs the 126 sweep weekly with 3-run aggregation and fails on > -3 site drop from baseline

## How a contributor should use this

1. Read `00`, `01`, `02`, `11` (these four — about 30 min) to understand state + bar.
2. Read `03` + `04` (methodology + tooling) before changing any engine code, so you can validate.
3. Pick a chapter from 05/06/07/08/09/10. Each chapter is self-contained: context, concrete pointers, acceptance criteria.
4. Use `13_FILE_LOCATIONS_INDEX.md` as a quick lookup.
5. Log open questions in `15_OPEN_QUESTIONS.md` so they're not lost.

## Source-of-truth artifacts (read these before believing this doc)

- `~/projects/browser_oxide_internal/benchmarks/baselines/2026-05-24/` — last full 4-profile sweep raw JSON outputs (BO + competitors)
- `docs/BENCHMARK_2026_05_24.md` — narrative report of the 2026-05-24 sweep
- `docs/PERFORMANCE_2026_05_24.md` — per-page perf investigation that motivated the pool path
- `docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md` — WAF variance characterization (±5 sites is noise)
- `crates/browser/tests/holistic_sweep.rs` — the 126-site corpus definition
- `crates/browser/src/classify.rs` — the verdict classifier (Pass / ThinShell / CHL / ThinBody / Error rules)
- `CLAUDE.md` — workspace conventions; scope rules on vendor solvers

## Memory-mode notes for the next contributor

This planning effort recovered three concrete fixes (A/B/C, see `09` + `04` + `10`). Two of them (B and C) are uncommitted in the working tree as of 2026-05-24. They are validated but not yet committed:
- `crates/browser/src/page.rs:3389` — drain restored 200 ms → 8 s
- `crates/browser/src/page.rs:216` + `crates/js_runtime/src/extensions/worker_ext.rs` + `crates/js_runtime/src/runtime.rs` — worker reap on `Page::drop`
- `benchmarks/bench_corpus_v2.py:256` — Camoufox RSS measurement fix

Commit them as three separate logical commits before starting Phase 0.
