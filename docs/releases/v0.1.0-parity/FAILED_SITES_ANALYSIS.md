# Failed Sites — Root Cause + What To Do (2026-05-27)

Companion to [`VERIFICATION.md`](VERIFICATION.md). **19 sites** the browser_oxide routed best-of-4 median doesn't pass, classified by *what other engine passes them* (= what would be sufficient to fix), with concrete action items. **For the next-developer handoff that turns these into actionable PRs, see** [`HANDOFF_v0.2.0_CLOSE_V150_GAP.md`](HANDOFF_v0.2.0_CLOSE_V150_GAP.md).

Sources: `/tmp/fix12_gate/*.{json,log}` (BO), `/tmp/full_sweep_2026_05_27/comp_camoufox{,_v150}.json` (Camoufox v135 / v150), `/tmp/full_sweep_2026_05_27/comp_{patchright,playwright,playwright_stealth}.json` (Chromium tier).

## TL;DR

| Stratum | Definition | Count | Concentration |
|---|---|--:|---|
| **A** | Camoufox v150 passes; BO doesn't | **11** | 7 AWS WAF, 2 SPA-hydration (booking + douyin), 1 reCAPTCHA Worker (duolingo), 1 TLS isolation (x-com) |
| **B** | Patchright passes; neither BO nor v150 (rare) | **1** | homedepot (Akamai sec-cpt — chromium-stealth solves it) |
| **C** | No engine tested passes — true open-source frontier | **7** | 3 Kasada, 1 Akamai bestbuy, 1 DataDome etsy, 1 SPA wildberries, 1 diagnostic areyouheadless |

**Strategic implication:** Stratum A (58% of the gap) is **engine-addressable** — Camoufox v150's "hardware spoofing" lineage proves these are solvable without vendor solvers. The fix surface is **fingerprint/stealth-class**, not vendor-WAF-class. This realigns where BO's next-cycle effort should go.

---

## The data table

| site | BO chrome | BO pixel | BO iphone | BO firefox | Cf v135 | Cf v150 | Patchright | cluster |
|---|---|---|---|---|---|---|---|---|
| amazon-ca | 2011b | 2011b | 2014b | 2011b | 5524b | **PASS** 215k | **PASS** 1138k | AWS WAF |
| amazon-com | 2014b | 2014b | 2011b | 2014b | 2008b | **PASS** 1286k | **PASS** 987k | AWS WAF |
| amazon-com-au | 2011b | 2014b | 2014b | 2011b | **PASS** 511k | **PASS** 945k | **PASS** 1037k | AWS WAF |
| amazon-fr | 2011b | 2011b | 2011b | 2011b | **PASS** 559k | **PASS** 871k | **PASS** 980k | AWS WAF |
| amazon-in | 2011b | 2011b | 2011b | 2011b | 2005b | **PASS** 708k | **PASS** 1060k | AWS WAF |
| amazon-jp | 2011b | 2011b | 2011b | 2011b | 2008b | **PASS** 850k | **PASS** 925k | AWS WAF |
| imdb | 1995b | 1995b | 1995b | 1995b | **PASS** 1057k | **PASS** 1068k | BLOCKED 117 | AWS WAF |
| booking | 8478b | 8473b | 3891b | 8473b | 8403b | **PASS** 513k | **PASS** 465k | SPA hydration |
| douyin | 6327b | 6327b | 6327b | 6327b | **PASS** 1036k | **PASS** 1020k | 8601b | SPA hydration |
| duolingo | 13327b | 13566b | 13554b | — | **PASS** 775k | **PASS** 697k | **PASS** 781k | reCAPTCHA Worker |
| x-com | THIN 69 | THIN 69 | THIN 69 | THIN 69 | **PASS** 379k | **PASS** 379k | **PASS** 379k | TLS / SharedSession |
| homedepot | Akamai-CHL | Akamai-CHL | Akamai-CHL | Akamai-CHL | Akamai-CHL | Akamai-CHL | **PASS** 1246k | Akamai sec-cpt |
| etsy | DataDome-CHL | DataDome-CHL | DataDome-CHL | DataDome-CHL | 7913b | DataDome-CHL | DataDome-CHL | DataDome |
| bestbuy | 7943b | 7889b | 7943b | 7943b | 7467b | 7467b | 7105b | Akamai SPA shell |
| wildberries | 1883b | ERR | 7900b | 7900b | 2710b | THIN 39 | 1818b | SPA hydration |
| canadagoose | Kasada-CHL | Kasada-CHL | Kasada-CHL | Kasada-CHL | Kasada-CHL | Kasada-CHL | Kasada-CHL | Kasada |
| hyatt | Kasada-CHL | Kasada-CHL | Kasada-CHL | Kasada-CHL | Kasada-CHL | Kasada-CHL | 13228b | Kasada |
| realtor | Kasada-CHL | Kasada-CHL | Kasada-CHL | Kasada-CHL | Kasada-CHL | Kasada-CHL | Kasada-CHL | Kasada |
| areyouheadless | 3653b | 3653b | 3653b | 3653b | 3668b | 3668b | 3662b | antibot diagnostic |

`PASS` = `L3-RENDERED` AND `len ≥ 15000`. Numbers in `b` = body bytes (loose L3-RENDERED, doesn't meet strict). `CHL` = vendor challenge interstitial. `THIN` = sub-100-byte body (TLS-drop or rate-limit). `ERR` = nav error.

---

## Stratum A — Engine-addressable: Camoufox v150 passes (11 sites)

This is the **real v0.2.0 priority list**. Camoufox v150 is open-source firefox + stealth patches; it passes these without any vendor-specific solver. The fix surface is observable fingerprint behaviour.

### A.1 — AWS WAF cluster (7 sites: amazon-ca/com/com-au/fr/in/jp + imdb)

**What we see:** 2011-byte (amazon) / 1995-byte (imdb) static challenge stub. Body contains:
```js
window.awsWafCookieDomainList = [];
window.gokuProps = { "key":"AQIDAHj...", "iv":"A6wUCgAhZgAB...", "context":"glDLQwA7..." };
<script src="https://*.awswaf.com/.../challenge.js"></script>
AwsWafIntegration.checkForceRefresh().then((forceRefresh) => { ... AwsWafIntegration.getToken().then(() => window.location.reload(true)); });
```

**What v150 does differently:** challenge.js runs to completion and calls `getToken()`. The token POST to `awswaf.com/.../verify` succeeds, sets `aws-waf-token` cookie, reload yields real Amazon. Patchright also makes this work, so the threshold is in the *initial fingerprint observation* (font list, WebAssembly subtle, hardware concurrency, RAF cadence, AudioContext sample-rate, screen, …) that challenge.js does before deciding whether to call `getToken()`.

**Why BO fails:** challenge.js *runs* (the `[vendor-detect] aws-waf` marker fires per `page.rs:1050`, the `/report` telemetry endpoint POSTs) but `getToken()` doesn't fire — `silently not called`. challenge.js detected something fingerprint-wise and bailed.

**v150 lineage hint:** `v146-hardware` release named "Hardware Spoofing". Likely shipped: per-context hardware concurrency / device memory / WebGL renderer / GPU vendor strings + matching AudioContext sample-rate variance + RAF jitter envelope. We have *some* of these (Fix 2 WebGL golden, Fix 9 RAF jitter) but at lower fidelity.

**Action items:**
1. **Capture challenge.js for a 2026-05-27 amazon-de hit + a corresponding v150 hit** (instrument both with the same harness, log everywhere `getToken` is called from). Diff what trips the v150 fork. Filed as **R-AWSWAF-FINGERPRINT-DIFF**.
2. **Audit BO's hardware-fingerprint surface** vs v150's. Concretely: enumerate every `navigator.*` / `screen.*` / `WebGLRenderingContext.*` / `AudioContext.*` / `AnalyserNode.*` getter and identify which ones are still leaking (a) the engine identity, (b) the wrong per-profile value, (c) the wrong cross-API correlation (e.g. UA says iPhone but `deviceMemory` says 32). Filed as **R-FP-AUDIT-2026Q3**.
3. **Investigate AWS WAF's prober as a unit-test fixture** — capture challenge.js, run it inside our V8 with our `navigator`/`window` stubs in a jest-like harness, instrument `getToken`. This gives a reproducible fingerprint-vs-decision oracle without going through the live IP. Filed as **R-AWSWAF-OFFLINE-PROBE**.
4. **Defer the actual AWS WAF token solver to `vendor_solvers`** per `CLAUDE.md` scope — but the *fingerprint fixes* that flip 7 sites belong in the public engine.

**Estimated effort:** R-FP-AUDIT-2026Q3 is the high-leverage item. ~2-3 weeks scoped: enumerate, prioritize by Camoufox-v150-source-diff, ship 5-10 targeted prototype-mask + getter-value fixes. Expected per-fix yield: 1-3 sites each from this cluster.

### A.2 — SPA hydration: booking + douyin (2 sites)

> **2026-05-28 reclassification note (parity-workflows):** the master roadmap
> §1.3 hypothesizes booking is actually **AWS-WAF** (not pure SPA hydration),
> sharing the same 1.37 MB `challenge.js`. If so, the live-verified AWS
> root-cause applies — `challenge.js` loads + executes but bails inside its
> own `checkForceRefresh().then()` chain before creating the blob PoW worker
> (zero `op_blob_register`/`op_worker_spawn`); see
> `docs/v0.1.0-parity-workflows/sites/SITE_awswaf_cluster.md` addendum +
> parity-workflows task #21. VERIFY with a live booking capture (grep the
> initial body for `gokuProps`/`AwsWafIntegration`) before treating it as a
> generic SPA fetch-chain bug; `R-SPA-BOOKING-FETCH-CHAIN` may be the wrong frame.

**booking observed:** body 8473 bytes (chrome/pixel/firefox), 3891 (iphone). Time = 15s (full nav budget). Final state: SPA shell, no `/api/...` hydration.
**douyin observed:** body 6327 bytes — *identical across all 4 profiles*. Deterministic detection. ttwid / `__ac_signature` cookie not set.

**What v150 does differently:** for booking, gets full 513KB hydrated React tree → the SPA's initial `/api/...` fetch is firing successfully. For douyin, gets 1MB content → ttwid signature computed correctly.

**Note: Patchright passes booking but NOT douyin** (8601b). douyin is firefox-specific stealth-class — possibly RAF jitter or AudioContext fingerprint quirks that Firefox has and Chromium-stealth doesn't.

**Why BO fails:**
- booking: Fix 8 (MessageChannel) targeted this cluster but didn't crack it. Per `02_GAP_ANALYSIS.md §booking`, the hypothesis is "SPA bootstrap requires a fetch chain that fails". Need to capture `fetches.json` diff between BO and v150.
- douyin: deterministic 6327 = identical bytes across BO profiles AND across BO ≠ Camoufox-v135. This is a fingerprint-driven server-side response, not a download-failure. ttwid / `__ac_signature` JS is in the 6327 bytes; need to instrument it to see why our V8 doesn't compute the right signature.

**Action items:**
5. **booking fetches.json diff (BO vs v150 vs Patchright)** — all 3 should be captured + diff'd. The missing fetch chain in BO is the smoking gun. Filed as **R-SPA-BOOKING-FETCH-CHAIN**.
6. **douyin in-VM instrumentation** — pull the 6327-byte response, identify the `__ac_signature` JS, run it with BO's V8 + verbose JS console, capture every CRYPTO/AudioContext call. Compare to a real-Chrome / v150 trace. Filed as **R-SPA-DOUYIN-SIG**.

**Estimated effort:** booking = 3-5 day investigation, fix probably 1-2 PRs. douyin = 1-2 week investigation (signature reverse-engineering), fix could be a JS surface gap.

### A.3 — duolingo (reCAPTCHA Worker)

**What we see:** body 13327-13566 bytes (1.7 KB shy of 15KB threshold per `02_GAP_ANALYSIS.md`). reCAPTCHA invisible JS loads `recaptcha/enterprise/webworker.js`. The SPA shell doesn't hydrate because `grecaptcha.execute()` never resolves with a token.

**What v150 does differently:** passes with 697KB → recaptcha Worker is producing the token. Patchright also passes.

**Why BO fails:** Fix 8 (MessageChannel) targeted this but didn't crack it. The Worker's view of `navigator`, `Worker.prototype.postMessage`, and the structured-clone surface is the suspect; `crates/js_runtime/src/extensions/worker_ext.rs` is the engine entry point.

**Action items:**
7. **reCAPTCHA Worker in-VM oracle** — spawn just `webworker.js` in BO's worker context and instrument every `postMessage`. Compare to v150's. The diff is the leak. Filed as **R-DUO-WORKER** (extends prior filing).

**Estimated effort:** 1 week of Worker context plumbing. High-confidence root cause (we know it's a Worker delta), unknown fix complexity until oracle runs.

### A.4 — x-com (TLS / SharedSession)

**What we see:** `THIN-BODY` 69 bytes uniformly across all 4 profiles, mid-sweep. In isolation: `L3-RENDERED` 274 KB. So it's the *cumulative sweep state* that breaks it, not the engine.

**What v150 does differently:** passes mid-sweep (379KB). So does Patchright. Camoufox doesn't have BO's SharedSession architecture.

**Why BO fails:** per `02_GAP_ANALYSIS.md §10`, the `f62584d` `SharedSession` rev introduced a process-wide `accept_ch` set that picks up `Accept-CH` headers from earlier sites. Twitter's WAF heuristic eventually rejects the connection.

**Action items:**
8. **A/B test SharedSession vs HttpClient::new across the 126-corpus** — already designed in §10. ~1 hour wall to confirm. If confirmed, the fix is per-site (or per-host) session isolation, not process-wide. Filed as **R-SHAREDSESSION-X-COM** (extends prior R-X-COM).

**Estimated effort:** A/B is 1 hour. Fix: 2-3 day refactor of session sharing scope.

---

## Stratum B — Chromium-only-solvable: homedepot (1 site)

**What we see:** all 4 BO profiles + Camoufox v135 + v150 → `Akamai-CHL`. Patchright → `PASS 1246KB`.

**What Patchright does differently:** the sec-cpt challenge resolves on Chromium-with-CDP-hidden. v150 (Firefox 150) doesn't pass — same as BO. So this is a **Chromium/Firefox split**: Akamai's sec-cpt has a Firefox-rejecting heuristic.

**Action items:**
9. **homedepot is currently filed in `memory/state_2026_05_16_phase5_datadome.md`** as having had a fix (Inc 7 `b623d5d` persistent sec-cpt BMP-suppression) that flipped chrome but not consistently for iphone. Re-check what regressed. The flake might be the BMP/canvas suppression order. Filed as **R-AKAMAI-SECCPT-FLAKE**.

**Estimated effort:** 2-3 days bisect + small fix. Patchright path is the reference.

---

## Stratum C — Universal hard frontier: 7 sites

No engine tested passes these. They define the true 2026-05-27 open-source SOTA ceiling.

### C.1 — Kasada (canadagoose + hyatt + realtor — 3 sites)

All 3 give `Kasada-CHL` across every engine including Camoufox v150. Patchright on hyatt shows 13228 bytes (loose L3 but sub-15KB — partial progress, possibly the unsolved interstitial). On canadagoose + realtor: full Kasada-CHL block.

Per `memory/state_2026_05_16_phase0_rebaseline.md`, the Kasada hunt's previous outcome was: **realm/sentinel/identity line CLOSED as not-the-bug**. The remainder is "holistic ML tail, no single lever". Camoufox v150 with hardware spoofing didn't move this — confirms ML-classifier nature.

**Action items:**
10. **Document as v0.3.0+ research target** — needs Kasada K2-DIFF approach + cross-engine fingerprint corpus. Belongs to `vendor_solvers` private repo per `CLAUDE.md` scope. Filed as **R-KASADA-FRONTIER** (existing).

**Estimated effort:** open-ended research, multiple month scale.

### C.2 — Akamai bestbuy (1 site)

`L3-RENDERED 7943` across BO + Camoufox v135 + v150. Patchright 7105b. SPA shell that all engines fail to hydrate. This is a **server-side conditional hydration**: Akamai serves the same minimal shell regardless of fingerprint until some trust signal is satisfied.

**Action items:**
11. **bestbuy interactive probe** — drive a manual Playwright run and see if the SPA hydrates after a click/scroll. If yes → behavioural signal needed. If no → trust signal is something we haven't identified. Filed as **R-BESTBUY-AKAMAI**.

**Estimated effort:** 2 days investigation, fix unknown.

### C.3 — DataDome etsy (1 site)

BO + v150 + Patchright all get `DataDome-CHL`. Camoufox v135 actually gets 7913b (loose L3, not strict — partial interstitial completion). So v135 had been able to progress past the initial CHL, but neither v150 nor BO. Some DataDome change broke v135's old behaviour too.

Per `memory/state_2026_05_16_phase5_datadome.md`, etsy is the daily-key endgame for DataDome WASM-iframe.

**Action items:**
12. **DataDome WASM-iframe key rotation tracker** — instrument once to capture the daily key + investigate if there's a relative-day arithmetic gap. Belongs to `vendor_solvers`. Filed as **R-DATADOME-DAILY-KEY**.

### C.4 — wildberries (universal SPA shell)

BO chrome 1883b, BO pixel ERR, BO iphone 7900b, Camoufox v135 2710b, v150 THIN 39, Patchright 1818b. **Different engines see different responses** — this is a unstable site or geo-blocked. Not a stealth problem.

**Action items:**
13. **wildberries — drop from canonical corpus?** It's a Russian retailer; possibly geo-blocked from this datacenter. Verify with a manual curl from datacenter IP — if 4xx/5xx, it's an unreachable site, not an engine miss. Filed as **R-CORPUS-WILDBERRIES**.

**Estimated effort:** 30 minutes investigation. Likely outcome: remove from corpus.

### C.5 — areyouheadless (diagnostic probe — designed to fail)

`areyouheadless.com` is an open-source bot-detection demo. It's *designed* to detect headless browsers via cross-correlated fingerprint signals; passing it cleanly is essentially solving every stealth surface at once. By construction, every engine including v150 gets the same 3653-3668 byte interstitial.

**Action items:**
14. **areyouheadless — exclude from headline pass-rate metric** — it's a known-impossible probe; including it drags every engine's score equally. Move to a separate "probe" bucket. Filed as **R-CORPUS-PROBE-BUCKET**.

---

## Suggested execution order (by leverage × effort)

| Rank | Action | Sites recovered | Effort | Where |
|---|---|---:|---|---|
| 1 | **R-FP-AUDIT-2026Q3** — diff BO fingerprint surface vs Camoufox v150 source | up to 7 | 2-3 weeks | public engine |
| 2 | **R-SHAREDSESSION-X-COM** — A/B test + session-isolation refactor | 1 | 3 days | public engine |
| 3 | **R-DUO-WORKER** — reCAPTCHA Worker in-VM oracle | 1 | 1 week | public engine |
| 4 | **R-SPA-BOOKING-FETCH-CHAIN** — fetches.json diff + fix | 1 | 3-5 days | public engine |
| 5 | **R-AKAMAI-SECCPT-FLAKE** — homedepot Inc-7 regression bisect | 1 | 2-3 days | public engine |
| 6 | **R-SPA-DOUYIN-SIG** — `__ac_signature` reverse-engineering | 1 | 1-2 weeks | public engine |
| 7 | **R-CORPUS-WILDBERRIES** — drop from corpus if geo-blocked | 1 (corpus-cleanup) | 30 min | corpus |
| 8 | **R-CORPUS-PROBE-BUCKET** — separate diagnostic sites | 1 (metric-cleanup) | 30 min | corpus |
| 9 | **R-BESTBUY-AKAMAI** — interactive probe | 1 | 2 days | public engine |
| 10 | **R-AWSWAF-OFFLINE-PROBE** — challenge.js in-VM oracle | 0 (enabler) | 1 week | public engine |
| 11 | **R-DATADOME-DAILY-KEY** | 1 | unknown | `vendor_solvers` |
| 12 | **R-KASADA-FRONTIER** | 3 | months | `vendor_solvers` |

**If items 1-9 land: routed median lifts from 107 → estimated 115-118**, putting BO at or above Camoufox v150's 115. Stratum C residuals (Kasada + etsy + bestbuy + corpus cleanup) are out-of-public-engine-scope or research-grade.

**If only items 1+2 land** (the highest-leverage pair): +7-8 sites → routed median 114-115. Already at parity with v150.

---

## What this changes about the v0.1.0-parity verdict

The original target was "beat Camoufox 113". On 2026-05-27 the realtime Camoufox bar is v150 = 115. BO's 107 is -8.

The framing shifts from "did we hit 115" (we didn't) to "**did our 12 fixes recover the right things and is the residual gap a tractable list?**" — yes and yes.

- Fix 11 (reddit `HTMLFormElement.elements`) is a confirmed measurable target-site flip — 1 site recovered, deterministic.
- The 19-site residual gap is **64% engine-addressable** (Strata A + B = 12 sites) and **36% out-of-scope** (Stratum C, of which 3 Kasada sites + DataDome belong to `vendor_solvers`, 1 is a known-impossible probe, 1 is likely geo-blocked).
- The Camoufox v150 advantage is concentrated in fingerprint-class wins (AWS WAF cluster), which is **exactly the surface area browser_oxide's `SCOPE.md` calls "stealth by design"** — not vendor solvers, but engine-side stealth fidelity. **This is the v0.2.0 work-list, perfectly aligned with project scope.**

No tag for v0.1.0-parity (per the 6c user decision), but the residual gap is well-mapped and actionable.
