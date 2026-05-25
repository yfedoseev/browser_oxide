# 15 — Open questions / research backlog

Living document. Add to it as questions surface. Resolve by removing + linking to the resolution doc.

## Unresolved questions blocking v0.1.0

### Q1 — Why does reddit's challenge handler not trigger iter 1?

**Status:** Investigated 2026-05-24, not resolved.  
**Owner:** TBD (Phase 1, see `05_SPA_HYDRATION_CLUSTER.md`)

Reddit's 8326-byte body contains a `<script>` that on DOMContentLoaded calls `form.requestSubmit()`. Our `requestSubmit` impl (`dom_bootstrap.js:1108`) calls `submit()` which sets `__pendingNavigation` (`dom_bootstrap.js:1098`). The outer nav loop reads `__pendingNavigation` at `page.rs:1944` and `2367`.

But: sweep_metrics shows reddit completing in 316-448 ms with only iter=0. Something in the chain doesn't fire. Hypotheses (in `05_SPA_HYDRATION_CLUSTER.md`):
- H1: DOMContentLoaded never fires before drain exits
- H2: `document.forms[0]` returns wrong element
- H3: `e.elements.namedItem("solution")` returns null
- H4: requestSubmit throws (prototype not properly registered)

**Next step:** add temporary logging in `dom_bootstrap.js` submit() to confirm it fires + log to `globalThis.__scriptErrors` if anything throws. Run reddit in isolation with `RUST_LOG=js_runtime=trace,browser=debug` and grep for the pending_nav signal.

### Q2 — What fingerprint signal does AWS WAF challenge.js check?

**Status:** Hypothesis-only, not investigated.  
**Owner:** TBD (Phase 2, see `06_AWS_WAF_SOLVER.md`)

The AWS WAF stub loads `challenge.js` from `*.token.awswaf.com`. Our V8 executes it; it POSTs to `/report` (telemetry) but never to `/verify` (token). Conclusion: a fingerprint check fails silently and `getToken()` short-circuits.

Candidates (none ruled out yet):
- navigator.webdriver / navigator.userAgentData
- WebGL vendor + renderer + extensions list
- Canvas/audio context hash
- Worker behavior
- WebAssembly.compile() timing
- Hardware concurrency, deviceMemory
- Touch events (mobile profiles)
- Permissions API responses

**Next step:** capture challenge.js, deobfuscate, identify the gate. See `06_AWS_WAF_SOLVER.md` Section 2.

### Q3 — Is the SharedSession bleed responsible for x-com THIN-BODY in full sweep?

**Status:** Hypothesis, A/B not run.  
**Owner:** TBD

In the full 2026-05-24 sweep, x-com returned 69 bytes (THIN-BODY) — after 24 prior nav requests had populated the process-wide `accept_ch` + cookie jar.  
In an isolated single-site run on 2026-05-24 (after fix B), x-com returned 274 KB (L3-RENDERED).

The difference suggests `f62584d` SharedSession is leaking some signal to Twitter's WAF. Hypotheses:
- An `Accept-CH` header from another site is now applied to the x.com request and Twitter's WAF flags it
- A cookie bleed sends a session ID from another origin (unlikely — net should partition by host)
- Rate limit on our IP (would NOT be a SharedSession issue)

**Next step:** A/B test — full 126-sweep with `HttpClient::shared` vs `HttpClient::new`, observe whether x-com flips. If yes, gate SharedSession behind `BROWSER_OXIDE_SHARED_SESSION` env var (default off for benchmark, on for production). See `14_TESTING_VALIDATION.md` for the A/B harness spec.

### Q4 — Should `default_solvers()` re-register engine-internal primitives?

**Status:** Plan documented in `07_DATADOME_PRIMITIVES.md`; decision pending.

The vendor strip (`aecdf19`) set `default_solvers()` to empty. Three behaviours it removed are NOT vendor-specific bypass — they're protocol-correct browser behaviour (CSP relaxation on challenge docs, cross-origin iframe materialization, solved-cookie retry). 07_DATADOME_PRIMITIVES.md proposes restoring them as engine-internal primitives (no vendor name in code).

Decision question: do these belong in the public engine, or should the public engine remain minimal and the customer benchmark plug in private solvers?

**Recommended:** public engine. Reason: real Chrome does these. The strip was about removing vendor-specific MITM code (Akamai sensor encoders, Kasada `x-kpsdk-ct`), not about removing browser-correct behaviour.

### Q5 — Pool path wellsfargo panic root cause

**Status:** Identified, not root-caused.  
**Owner:** TBD (Phase 4, see `10_TIMING_OPTIMIZATION.md`)

`op_dom_set_inner_html` triggers `crates/dom/src/arena.rs:678` cycle detector (sees >100k unique nodes from root). Pool mode only — cold mode renders the same URL fine. Hypothesis: `reset_warm_state` clears DOM via `replace_dom` (`lib.rs:170`) but doesn't fully isolate from previous page's arena allocator state.

**Next step:** add per-page arena snapshot before/after warm reuse. Repro: pool sweep stops at site 98 of the 126 corpus.

## Deferred decisions

### D1 — V8 HEAP_INITIAL: stay at 1 GB or reduce to 256 MB?

**Status:** Deferred from 2026-05-24 — see `09_MEMORY_OPTIMIZATION.md` Section 5.

Per-isolate baseline drops -30 to -50 MB if reduced. Risk: creepjs (which is in our 126 corpus and currently passes) allocates well past 256 MB during fingerprint pass; without the reserved heap, V8 compacts repeatedly → slower (per the existing comment at `runtime.rs:107`).

**Decision criteria:** A/B test on creepjs only — if creepjs stays Pass with ≤ 5% time regression, ship 256 MB. Else stay at 1 GB and find memory wins elsewhere.

### D2 — Restore vendor solver impls in public engine?

**Status:** Resolved: NO. Per CLAUDE.md.

Vendor-specific bypass (Akamai sensor v2, Kasada `x-kpsdk-ct`, DataDome WASM-iframe-daily-key) stays in the private `vendor_solvers` crate. Public engine ships the engine + primitives only.

The customer's internal benchmark registers `vendor_solvers::default_solvers()` via `Page::with_solvers(...)`. That's the supported path.

### D3 — Add chrome_148_windows profile?

**Status:** Deferred. See `11_PER_PROFILE_STRATEGY.md`.

Some sites WAF Linux-allowlisted; macOS / iOS / Android / Firefox-macOS may miss them. Adding Windows would help on a measurable subset. Estimate from data: +1-3 routing wins.

**Decision criteria:** if v0.1.0 lands under target (≥ 115), add the profile. Else defer to v0.2.0.

## Research backlog (not blocking v0.1.0)

### R1 — V8 snapshot warming

Pre-bake stealth bootstrap + all per-page-static JS into the V8 startup snapshot. Currently bootstrap re-runs on every cold isolate. Saves ~100 ms per cold nav.

Reference: `crates/js_runtime/src/snapshot.rs` already produces a snapshot; the stealth bootstrap currently runs OUTSIDE the snapshot path (see `runtime.rs:341`).

### R2 — Parallel cold sweep across profiles

V8 isolates are per-thread (per CLAUDE.md). Could spawn 4 threads, one per profile, sweep in parallel. Throughput potential 4× on cold path. Trade-off: 4× memory (since pool path has the worker leak, cold doesn't accumulate across threads).

### R3 — Per-site retry budget tuning

Current default 15 s nav budget + 25 s extend. Some sites (etsy, tripadvisor, yelp — all DataDome) burn 90 s with no progress. Adaptive budget that early-cancels CHL paths would save a lot of sweep wall-clock.

### R4 — Kasada cracking (canadagoose / hyatt / realtor)

Open SOTA frontier. See `08_KASADA_FRONTIER.md`. Camoufox only solves 4/5. Out of scope for v0.1.0.

### R5 — Real Chrome A/B for AWS WAF

Use Playwright + CDP intercept to capture exactly what real Chrome sends to `awswaf.com/.../verify`. Diff against what BO sends (nothing). Inform the fingerprint hypothesis.

### R6 — Stealth profile drift detection

When Chrome ships 149, our chrome_148_macos profile is stale. WAF detection scores get noisier. A monthly auto-bump tool (auto-update the brand version list, sec-ch-ua-full-version-list, TLS impersonate codename) would prevent drift.

### R7 — Why does iPhone profile underperform pixel?

iPhone 98 Pass, pixel 102. iPhone has 11 CHL vs pixel's 6. Some WAFs treat iOS Safari more strictly. Worth a deeper per-category analysis (see `11_PER_PROFILE_STRATEGY.md`).

### R8 — booking / douyin specific mechanism

Both 6-8 KB SSR shells. Not investigated past hypothesis. Need capture + diff vs Camoufox (`04_TOOLING_SPEC.md`).

## Resolved questions (for posterity)

### RES-1 — Was the "121 → 108" a real regression?

**Answer:** No, mostly methodology. Apples-to-apples (loose L3) went 121 → 118 single-best, 123 → 120 routed — within ±5 noise floor. The 108 number is a different metric (strict ≥ 15 KB body gate added). See `01_CURRENT_STATE.md`.

### RES-2 — Is Camoufox really 48 MB?

**Answer:** No, measurement bug. Real Camoufox tree RSS is 200-400 MB on a 126-site sweep. `benchmarks/bench_corpus_v2.py:256-267` walked only the first /proc child matching "fox" and missed Firefox e10s content processes. Fix applied 2026-05-24 (uncommitted). See `09_MEMORY_OPTIMIZATION.md` Section 3.

### RES-3 — Did the 200 ms build_page drain cap break async challenges?

**Answer:** Yes, partially. Fix B (200 ms → 8 s) validated: adidas flipped 2.5 KB → 1.3 MB deterministically; amazon-jp + x-com flipped on isolated runs. Spot-check shows no regressions. See `10_TIMING_OPTIMIZATION.md` Section 5.

### RES-4 — Is the worker leak real?

**Answer:** Yes. 13 sites (cnn / bloomberg / youtube / costco / asos / discord / udemy + 6 others) trigger > 15 MB step-ups that never reclaim. Each leaked worker = 64 MB stack OS thread + child JsRuntime. Fix C applied 2026-05-24 (uncommitted): WorkerOwnership state in OpState, drain_owned_workers in Page::drop. See `09_MEMORY_OPTIMIZATION.md` Section 4.

## How to use this doc

- Add new questions at the top of the appropriate section.
- When you investigate a question, link the investigation result (which doc it's now covered in).
- When a question is resolved, move it to "Resolved questions" with the answer.
- This doc should never grow unboundedly — convert resolved items into reference text in the appropriate chapter and link from here.
