# 15 — Open questions / research backlog

Living document. Add to it as questions surface. Resolve by removing + linking to the resolution doc.

## Unresolved questions blocking v0.1.0

### Q1 — Why does reddit's challenge handler not trigger iter 1?

**Status:** Investigated 2026-05-24; **likely resolved by chapter 17 finding** (HTMLFormElement.elements is missing).  
**Owner:** TBD (Phase 1, see `05_SPA_HYDRATION_CLUSTER.md` + `17_WEB_API_PARITY_MATRIX.md`)

Reddit's 8326-byte body contains a `<script>` that on DOMContentLoaded calls `form.requestSubmit()`. Our `requestSubmit` impl (`dom_bootstrap.js:1108`) calls `submit()` which sets `__pendingNavigation` (`dom_bootstrap.js:1098`). The outer nav loop reads `__pendingNavigation` at `page.rs:1944` and `2367`.

But: sweep_metrics shows reddit completing in 316-448 ms with only iter=0. **Chapter 17 § 2 found the root cause**: `HTMLFormElement.prototype.elements` does not exist. Reddit's challenge calls `e.elements.namedItem('solution')` → `undefined.namedItem(...)` throws TypeError → silently caught by the top-level trap at `page.rs:3406` → `__pendingNavigation` never set → no iter 1.

**Likely fix**: implement `HTMLFormElement.prototype.elements` returning an HTMLFormControlsCollection-like wrapper. Chapter 43 § 3 has this as MUST-HAVE fix #11 (0.5 day). Validation: reddit body should flip from 8326 → 100+ KB.

### Q5 — Worker-context fingerprint identity check (HIGHEST RISK unknown)

**Status:** Surfaced by chapter 41 § 4.4 + chapter 42 § 3 Pattern 5; **not audited**.  
**Owner:** TBD (Phase 0 prerequisite)

Per chapter 42 matrix: 9 of 12 vendors check that the WORKER context returns the SAME fingerprint as the main thread. If main says "Chrome 148 Linux" but Worker says "stub" → bot.

**BO status**: chapter 16 documented main-thread coverage; Worker-context coverage NOT audited. Single failing check on a single API in Worker context could explain WHY duolingo/recaptcha fails despite our main-thread API surface looking complete.

**Next step**: audit `crates/js_runtime/src/js/worker_bootstrap.js` (or wherever Worker-context shims live) against `window_bootstrap.js`. Each prototype method in window should have an equivalent in worker; values must match.

### Q6 — homedepot inversion: why does Playwright/Patchright PASS but BO + Camoufox FAIL?

**Status:** Surfaced by chapter 27 § 4 + chapter 42 § 4.  
**Owner:** TBD (research; informs chapter 26)

Per chapter 27 measured data: Playwright + Patchright + PW-Stealth ALL pass homedepot.com with 1+ MB body. BO + Camoufox both fail at ~2638 bytes (Akamai-CHL). Akamai's homedepot tenant evidently trusts real Chromium TLS+UA+headers enough that even CDP-detected Playwright passes. This INVERTS the usual narrative ("CDP-driver tier is worst").

**Hypothesis**: real Chromium chrome_for_testing build sends a header or TLS extension that boring2's chrome_147 codename doesn't include, AND Akamai's homedepot tenant checks specifically for it. Camoufox loses because it's Firefox-class, not Chromium-class.

**Next step**: capture both real Chromium (via Playwright + CDP intercept) and BO chrome_148_macos on homedepot. Diff at TLS layer (ClientHello bytes), HTTP layer (header set + order), and JS layer. Identify the differentiator. Could be a free win.

### Q7 — adidas firefox-only win: what specifically makes BO firefox uniquely pass Akamai BMP?

**Status:** Surfaced by chapter 26 § 4.1 + chapter 27 § 2.  
**Owner:** TBD (research; informs chapter 26 + 11)

Per chapter 26: BO firefox profile uniquely flips adidas 2494 → 1.3 MB. Camoufox FAILS adidas (2384 bytes). Other BO profiles (chrome/pixel/iphone) all fail. **Camoufox is NOT strictly better than us** — this is the proof.

Four hypotheses in chapter 26 § 4.1 (all unverified):
- H1: TLS class — Firefox-class TLS impersonation acceptable to Akamai's adidas tenant
- H2: `WebGLRenderingContext.getParameter(VENDOR)` returns `""` for Firefox (per Mozilla spec), masked WebGL
- H3: No UA-CH (sec-ch-ua-*) headers on Firefox profile — Akamai may distrust UA-CH presence
- H4: Combination of the above

**Next step**: capture diff per chapter 04 between BO chrome (fails adidas) and BO firefox (passes adidas). Bisect to find the load-bearing field. Bake the learning into other profiles where possible.

### Q8 — Castle `__cuid` cookie: real or third-party rumor?

**Status:** Surfaced by chapter 36 § 2.3 honest-uncertainty note.

Castle's own device-fingerprinting docs reference "first-party cookies" without naming them. The `__cuid` cookie name comes from third-party reports (scrapfly, capsolver writeups), not Castle docs. May not be the actual cookie.

**Next step (if customer brings Castle-protected site)**: capture real request via Playwright + CDP, identify Castle's actual cookie name.

### Q9 — Reblaze "Mc Cohen module" / `mc.cohen.io`: unverified

**Status:** Surfaced by chapter 37 § 1.3 honest-uncertainty note.

The "Mc Cohen module" reference (which I included in the prompt for Reblaze research) is not verifiable in any public source. Agent searched Tracxn, Crunchbase, all Reblaze/Link11 docs subdomains, press releases — zero hits.

**Status: research-item-not-confirmed.** Reblaze's bot decision engine appears to be unnamed in public docs. If a customer brings a Reblaze-protected site, capture the actual JS asset URL.

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

## v0.1.0-parity Fix-list residuals (2026-05-26)

Nine of the twelve EXECUTION_PLAN.md fixes landed in-session (pre-flight + Fixes 1, 3, 5, 6, 7, 8, 9, 10, 11) on stacked branches under `fix/v0.1.0-fixN-*`. The remaining three are blocked on out-of-session infrastructure:

### R-FIX-2 — WebGL per-profile golden snapshot needs real-Chrome captures

EXECUTION_PLAN.md Fix 2 step 1: "Capture real Chrome 148 macOS WebGL output via Playwright + CDP". The capture requires a working Playwright install pointing at real Chrome 148 (per 4 profiles: chrome_148_macos, chrome_148_windows, iphone_15_pro_safari_18, firefox_135_macos). Not runnable in this session — no Playwright/Chrome on the box; outputs would be needed under `crates/browser/tests/captures/`. **Owner action:** run the capture, commit the four JSON snapshots, then the engine-side `webgl_param_golden_snapshot` test + any `webgl_ext.rs` fixes are mechanical.

### R-FIX-4 — Canvas toDataURL parity needs real-Chrome captures

EXECUTION_PLAN.md Fix 4 step 1: "Capture real Chrome 148 canvas output for the FingerprintJS + browserleaks + thumbmarkjs standard draw sequences". Same external dependency as R-FIX-2. Once `crates/browser/tests/captures/canvas_chrome_148.json` exists, the `canvas_todataurl_parity` test + per-divergence fixes in `crates/canvas/` are mechanical.

### R-FIX-12 — 3-run aggregated baseline + acceptance gate

EXECUTION_PLAN.md Fix 12 needs ~12 h sweep wall-clock (3 runs × 4 profiles × 126 sites in pool mode). Cannot run in-session for time + the need to merge Fixes 1, 3, 5-11 to a single branch before sweeping. **Suggested next steps**:
  1. Cherry-pick / merge all `fix/v0.1.0-fixN-*` branches onto a single `release/v0.1.0-parity` branch.
  2. Run the sweep command listed in EXECUTION_PLAN.md Fix 12 **wrapped in a per-site wall-clock watchdog** (see R-V8-TERM below).
  3. Aggregate per 14_TESTING_VALIDATION.md §L5; compare to the 2026-05-24 internal baseline.
  4. If median routed best-of-4 ≥ 115: tag `v0.1.0-parity-rc1`. If 113-114: tag `v0.1.0-parity`.

**Partial in-session sweep data** (single profile, single run on `fix/v0.1.0-fix4-canvas-parity`, killed mid-sweep at site 73 by R-V8-TERM): 57 / 73 strict-pass (78% on first 73 sites). 4 CHL: etsy/homedepot/tripadvisor/skyscanner (all known-hard). Extrapolation to 126 is unsafe — the unhit second half is enriched for easy SaaS/news/reference sites, but also contains the canadagoose/hyatt/realtor Kasada cluster. Anchor decisions on the full re-run.

### R-V8-TERM — V8 `terminate_execution` returning `true` but JS continuing for hours

Discovered while running the partial Fix 12 sweep on `fix/v0.1.0-fix4-canvas-parity` (2026-05-26): after `[73/126] travel skyscanner PerimeterX-CHL`, the engine entered uber.com (site 74). The log shows `[V8DeadlineWatcher] deadline 25000ms expired — firing terminate_execution / terminate_execution returned true` then `[op_net_fetch_sync] fetched 2 bytes from https://tags.tiqcdn.com/utag/uber/main/prod/utag.v.js?...`, then **3.5 hours of 94% CPU with zero log output** before being killed externally. The sweep_metrics output JSON was never written.

V8's `terminate_execution` is documented as "may not have effect until the next V8 entry point" — but our V8DeadlineWatcher already reports it returned `true`. The hang reproduced on uber-alone-x2 once during initial diagnosis but did NOT reproduce on three subsequent retries on the same binary, so it's transient — likely Tealium's `utag.v.js` returning a degenerate response (the 2-byte fetch is suspicious) that V8 enters an uninterruptible native-op spin on.

**Reproducer recipe** (not always-fires):
```bash
# Build the v0.1.0-parity union branch
cargo build --release -p browser --example sweep_metrics
# Run uber twice in a row — sometimes hangs on the second nav
echo '[{"cat":"travel","name":"uber","url":"https://www.uber.com/"},{"cat":"travel","name":"uber","url":"https://www.uber.com/"}]' > /tmp/u2.json
target/release/examples/sweep_metrics chrome_148_macos /tmp/u2.json /tmp/u2_out.json
```

**Mitigations / next steps**:
  1. Wrap sweep_metrics in an external watchdog that kills the process if a single site exceeds N minutes (e.g., 3× the per-site budget). Recommended for any unattended Fix 12 run.
  2. Investigate which native op is blocking the V8 deadline — candidates: synchronous fetch (`op_net_fetch_sync`), CSS layout, or DOM mutation in the V8DeadlineWatcher's "after-terminate" path.
  3. Predates the v0.1.0 work — present in `main` HEAD `385d70a`. The 11 landed fixes neither caused nor cure it; just want to surface it as a load-bearing gap for the Fix 12 gate.

### R-FIX-WINDOWS-RTX — chrome_148_windows preset drift

The Fix 2 engine-side test (`webgl_param_golden_snapshot_chrome_148_windows`) surfaced an existing preset/catalog drift: `crates/stealth/src/presets.rs:65` declares `webgl_renderer: "...RTX 3080..."` but `:106` selects `gpu_profile: nvidia_rtx_3060_windows()`. The engine reads from `gpu_profile` so the user-facing `webgl_renderer` declaration is dead in this code path; tests anchor on `gpu_profile.unmasked_*`. Fix is a one-line `webgl_renderer` correction OR removal of the dead field — defer until owner decision on which is canonical.

### R-FIX-pre — Pre-flight HEAD breakage at `385d70a`

main HEAD was not gate-green when the v0.1.0 work started: (a) 7 test compile errors from a missed `chrome_130_*` → `chrome_148_*` rename in `c3ec0ed`; (b) 14 clippy `-D warnings` errors (redundant imports, unused mut, dead code, etc. in `page.rs` + 2 unused doc comments in `fetch_ext.rs`); (c) fmt drift across 10+ files. All fixed mechanically on branch `fix/pre-flight-head` @ `2a373e2`. The single remaining test failure (`humanize_mouse_intervals_are_right_skewed`) pre-existed at HEAD and is the Σ-Λ signal Fixes 5/6/9 are designed to flip — verify post-Fix-12 sweep.

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
