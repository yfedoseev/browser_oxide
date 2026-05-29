# ENGINE: Camoufox v150 — how it wins 8/12 (≈113/126), reverse-engineered to copy-or-beat

**Author:** research agent, 2026-05-28
**Scope:** Reverse-engineer the *mechanisms* behind Camoufox v150's passes that BO lacks, at file:line depth, and produce a ranked, ROI-ordered fix list (public-engine vs vendor_solvers tagged).
**Baseline (same IP, 12 contested sites, `audit/17_DELTA_HEADTOHEAD`):** BO 5 / v150 8 — BO is behind by 3 (imdb, booking, amazon-in are the hard losses; amazon-fr/jp/com-au are reliability gaps).

---

## 0. Executive summary — the single most load-bearing finding

**v150 does NOT beat BO on fingerprint fidelity. It beats BO on *runtime completeness*.** Two independent lines of evidence converge:

1. **BO's own AWS root-cause** (`HANDOFF_2026_05_28b.md` §4): challenge.js *proceeds* with BO's fingerprint, reads `navigator.plugins`/`webdriver`/WebGL unmasked/`chrome.csi`/`performance.now`, and calls `forceRefreshToken` **in the offline oracle**. It only fails in the **live navigate path** because BO's `build_page_with_scripts_init_and_storage` drains the event loop for only **50 ms between scripts + 500 ms final** (`crates/browser/src/page.rs:1678`, `:1730`), so challenge.js's `checkForceRefresh().then(...)` promise chain — which *defers* the blob-URL PoW Web Worker spawn + token POST — never advances before the nav loop re-fetches the stub.

2. **External corroboration** (ZenRows, "How to Bypass AWS WAF", 2026): Camoufox itself scores only **~17% against AWS WAF cold** (fresh profile, no token) but **86% with a pre-warmed session** that already holds the `aws-waf-token` cookie. So even v150 does not "fingerprint past" AWS WAF — it survives because **Firefox's real event loop runs challenge.js's async self-solve to completion**, the PoW worker computes, the token POSTs, the cookie is set, and the reload returns real content. BO's deficiency is that its navigate loop short-circuits before that async chain finishes.

This reframes the whole gap. The 67-macOS-preset diversity, WebGL renderer format, audio variance, and RAF jitter that the prior audits (`audit/02`, `audit/03`) catalogued are **real but second-order**. The first-order lever is **the live-nav async drain** (`HANDOFF_2026_05_28b.md` §5.1) — confirmed here from a fresh, independent angle.

Sources: [ZenRows — Bypass AWS WAF](https://www.zenrows.com/blog/bypass-aws-waf), [daijro/camoufox](https://github.com/daijro/camoufox), [ScrapingBee — Camoufox](https://www.scrapingbee.com/blog/how-to-scrape-with-camoufox-to-bypass-antibot-technology/).

---

## 1. What the existing repo docs already concluded (cited)

| Doc | Conclusion this doc builds on |
|---|---|
| `audit/02_CAMOUFOX_V150_OVERVIEW.md` | v150 = Firefox 149-152 + 32 surgical C++ patches; spoofing per-`userContextId`; ~70 config keys; 67 macOS presets from BrowserForge; **does NOT pretend to be Chrome** (UA stays `Firefox/150.0`). The bypass is *hardware coherence*, not UA-spoofing. WebGL renderers anonymized (`Apple M1, or similar`) — BO must NOT copy this (Chrome leak). |
| `audit/03_HARDWARE_SPOOFING_DIFF.md` | 5 plausible deltas: (1) BO 1 preset vs v150 67; (2) WebGL renderer format; (3) v150 stays Firefox so fewer cross-API surfaces to leak; (4) v146 bug #443 = worker-navigator vs main-navigator mismatch; (5) v150 disabled canvas noise (`e4528a2`). Flagged BO's per-session-random `AudioContext.sampleRate` as an anomaly (now fixed, FIX-C). |
| `audit/15_FIX_PRIORITY_RANKED.md` | FIX-A (Sec-CH-UA-Arch from profile), FIX-C (audio seed), FIX-D/D2 (WebGL surface), FIX-J (FileReader.readAsDataURL — THE bug that first flipped amazon-ca/de), FIX-E/E2 (profile sampler). FIX-J is the precedent: a *runtime-completeness* bug, not fingerprint, was the lever. |
| `HANDOFF_2026_05_28b.md` §4-5 | AWS = self-solve-execution gap. Worker `crypto.subtle` prereq shipped (`5216336`); WebGL1/2 split shipped (FIX-D2, `07491f9`); **neither flipped a site**. The remaining lever = live-nav async drain (§5.1). |
| `FAILED_SITES_ANALYSIS.md` | Cluster A = 11 sites: 7 AWS WAF + booking + douyin (SPA hydration) + duolingo (reCAPTCHA worker) + x-com (TLS, now solved). douyin = `__ac_signature`, **Firefox-only solve, not transferable to BO's Chrome identity** (`vNext/05`). |
| `vNext/05_R-SPA-DOUYIN-SIG.md` | douyin's win is "Firefox naturally produces the right value distribution" — explicitly *not a transferable lesson*. Deprioritized to v0.3.0+. |

**Net of prior work:** the fingerprint surface has been audited to near-parity for the Chrome identity BO claims. The unresolved frontier is *runtime async completeness* (AWS/booking/duolingo) and *preset diversity for anti-clustering* (reliability gaps).

---

## 2. New external findings (cited)

### 2.1 Camoufox's worker / async model = native, not a "drain"
Via deepwiki on `daijro/camoufox`: Camoufox does **nothing special for `requestAnimationFrame`/`setTimeout`** — there is no async-completion trick. It runs the **real Firefox event loop**, and the *only* worker-specific code is **fingerprint-value synchronization** so workers read coherent spoofed values:
- `cross-process-storage.patch` — synchronous IPDL `RoverfoxStoragePut`/`RoverfoxStorageGet` so per-context values exist *before* any worker process starts (no race).
- Workers resolve `userContextId` via `WorkerPrivate::GetOriginAttributes()` (inherited from creator's `BrowsingContext`).
- `dom.ipc.processPrelaunch.enabled = false` so a fresh content process never inherits stale overrides.

**Implication for BO:** the reason v150 solves AWS/duolingo is structural — Firefox keeps the page alive and pumping its event loop until the PoW worker finishes and the token POSTs. BO must **emulate that by extending its drain on challenge pages**, not by adding a fingerprint patch. This is the single highest-ROI item and it is **public-engine-addressable** (no per-vendor code: a generic "if the page is still doing async work, keep draining" rule).

### 2.2 v150 cross-API coherence is enforced at the *CSS media-query* layer too
deepwiki (`daijro/camoufox`, navigator/screen patches): `ScreenDimensionManager` hooks **`nsMediaFeatures.cpp`** so `matchMedia('(device-width: 1920px)')` returns results consistent with spoofed `screen.width`, "preventing detection of inconsistencies between JavaScript APIs and CSS media queries." Navigator spoofing uses a **three-tier fallback** (per-context → global `CAMOU_CONFIG` → vanilla) and explicitly hooks `WorkerNavigator::GetPlatform/HardwareConcurrency/GetUserAgent` (the bug #443 fix) so main-window == worker.

**BO status:** BO *does* implement a media-query evaluator (`window_bootstrap.js:4060-4087`) — but **it answers `device-width`/`device-height` from `inner_width`/`inner_height`, not `screen.width`/`screen.height`** (`:4065`, `:4068`). Real Chrome resolves `device-width` to the *screen* dimension (in CSS px), not the viewport. This is a **subtle coherence leak**: a probe comparing `screen.width` (e.g. 1512) to `matchMedia('(device-width:1512px)')` will find BO returns the inner width (1512 here happens to match because the sampler sets `inner_width = screen_width`, but on windowed/zoomed profiles they diverge). Low-frequency but a clusterable inconsistency v150 explicitly closes.

### 2.3 v150 canvas spoofing = deterministic ±1 on *first non-zero channel*; BO uses biased ±1 on a 5% pixel subset
deepwiki (`canvas-spoofing.patch`): `ApplyCanvasNoise` "iterates RGB channels and modifying the first non-zero channel by +/-1 … deterministic … Zero-value channels are skipped to preserve `clearRect()` transparency." Per-context seed; **no global noise distribution** to cluster on (this is why `audit/03` noted v150 *disabled* the old global canvas noise).

**BO status:** `crates/canvas/src/canvas2d.rs:1110-1131` (`to_data_url_with_jitter`): perturbs **5% of pixels** (`val % 100 < 5`) and **biases the direction by luminance** (`if pixels[i] > 128 { sub } else { add }`). Two problems vs v150:
- The **luminance-biased direction** is a *fixed, detectable transform*: every pixel >128 always goes down, every pixel ≤128 always goes up. A vendor that renders a known reference image and diffs sees a deterministic, non-physical pattern (real hardware noise is symmetric). This is **more clusterable than v150's approach**, not less.
- It skips the "first non-zero channel only / preserve transparency" rule, so `clearRect` regions can get perturbed → alpha-channel artifacts.

### 2.4 v150 anti-font-fingerprinting = HarfBuzz letter-spacing seed; BO has no measureText jitter
deepwiki (`anti-font-fingerprinting.patch`): per-context seed adds ~0.0-0.1 em spacing via an LCG inside HarfBuzz shaping; `WordCacheKey` gets `mUserContextId` to prevent cross-context cache hits. BO's `measureText` (`canvas_bootstrap.js:179`) returns deterministic widths from its font metrics with no per-profile jitter — so two BO instances produce *identical* measureText fingerprints (clusterable across the datacenter IP). v150 makes each context unique.

### 2.5 BrowserForge = Bayesian-network coherent sampling (the source of the 67 presets)
Local read of `browserforge/bayesian_network.py` + `fingerprints/generator.py`:
- The fingerprint network is a **conditional Bayesian network** (`generate_consistent_sample_when_possible`, `bayesian_network.py:121-161`) sampled in topological order; `recursively_generate_consistent_sample_when_possible` backtracks (bans values) until it finds a globally consistent assignment.
- It is **seeded by a real User-Agent first** (`generator.py:197-209`): generate headers → extract UA → sample the fingerprint *conditioned on that UA*. So `screen`, `navigator`, `videoCard`, `fonts`, `audioCodecs`, `pluginsData` are all mutually consistent for the chosen UA — by *construction*, from real-browser frequency data (`apify_fingerprint_datapoints`).
- Camoufox then maps BrowserForge output → its config keys via `browserforge.yml` (`camoufox/fingerprints.py:from_browserforge`), with `handle_screenXY` deriving `window.screenY` from `screenX` coherently.

**Implication:** v150's "67 presets" are not hand-picked — they are **samples from a coherent joint distribution of real Firefox fingerprints**. BO's `chrome_148_macos_sampled_with_rng` (`presets.rs:750`) is a *hand-built* approximation: it picks a chip (M3/Pro/Max) then constrains cores/RAM/screen/GPU to that chip. This is good (it already learned the "vary independently → regression" lesson, `presets.rs:736-738`), but it covers **3 chips × ~2 screens** ≈ a handful of points vs v150's distribution-sampled diversity. For same-IP anti-clustering this is the gap on the *reliability* sites (amazon-fr/jp/com-au).

---

## 3. BO code-level analysis (file:line) — where each mechanism lives and what's missing

### 3.1 The live-nav drain (THE lever) — `crates/browser/src/page.rs`
- **Inter-script drain:** `:1678` `run_until_idle(Duration::from_millis(50))` inside the document-order script loop (`:1660-1680`).
- **Final drain:** `:1730` `run_until_idle(Duration::from_millis(500))`.
- **Offline oracle (works):** `Page::from_html_with_url` path uses `run_until_idle(Duration::from_secs(8))` (`:611`) and the awswaf oracle uses a 5 s idle — that is why challenge.js completes there.
- **Navigate-loop budget:** `:1881-2129` — the loop has a 50 s default budget with a V8DeadlineWatcher (`:56-86`), FAST-EXIT when `body > 50 KB && !is_chl` (`:2086`), and a per-iteration `run_until_idle(drain_timeout)` (`:2055`). The bug: for an AWS stub (1995-2011 bytes, *below* 50 KB, marked `is_chl`-adjacent), the loop re-fetches before the async self-solve lands. The drain is gated by *byte size + CHL marker*, not by *"is there outstanding async work (a live worker, a pending fetch, an unsettled promise that owns a timer)"*.

**Fix shape (public engine):** add an "async-in-flight" predicate to the navigate loop's continue/break decision: keep draining (up to the existing 13-50 s budget) while (a) any owned worker is alive (`worker_ext::worker_registry()` non-empty / not terminated), OR (b) there is an in-flight `fetch` (track a counter in `fetch_ext`), OR (c) an AWS-WAF/PoW marker was seen in challenge.js. This is generic — it does not encode any vendor's algorithm; it just refuses to give up on a page that is still computing. Mirrors what Firefox's event loop does for free.

### 3.2 Worker runtime — already structurally correct
- `crates/js_runtime/src/extensions/worker_ext.rs:333-357`: each worker runs on its **own thread with a real `run_event_loop`** in a 25 ms-tick loop until terminated. Blob-URL workers round-trip (verified in handoff). `crypto.subtle` now inherited (`5216336`).
- **So the worker side is NOT the blocker** — the blocker is that the *main* page (where `new Worker(blobURL)` is called inside a `.then()`) stops pumping before the `.then()` runs. Confirm by instrumenting whether `op_worker_spawn` is ever called in the live path (handoff says it is NOT — consistent with the main-thread drain ending first).

### 3.3 Audio — at parity (FIX-C landed)
`canvas_bootstrap.js:839-889`: `sampleRate` from `profile.audio_sample_rate` (48000 Apple / 44100 else); `baseLatency`/`outputLatency` derived from `audio_seed` bits — stable per profile, matches v150's `AudioContext:sampleRate`/`outputLatency` + `audio:seed` model. **No action.**

### 3.4 RAF jitter — at parity (good)
`timer_bootstrap.js:174` `_rafDelayMs = max(1, _RAF_MEAN_MS + gauss()*_RAF_SIGMA_MS)` fired via real `setTimeout` (`:193-199`), explicitly defeating the Kasada `set(diffs).size===1` perfect-grid probe. **No action** — BO is ahead of a naive RAF here.

### 3.5 Canvas noise — REGRESSION RISK vs v150 (§2.3)
`canvas2d.rs:1110-1131`: luminance-biased ±1 on 5% of pixels. Should switch to v150's symmetric, deterministic "first-non-zero-channel ±1, skip zero channels" to (a) preserve transparency and (b) remove the detectable directional bias. Or, per `audit/03` finding (v150 *disabled* global canvas noise on `e4528a2`), evaluate **disabling** it and relying on per-profile `canvas_seed`-driven deterministic rendering instead (FIX-G is already "research" status, `audit/15` row 7).

### 3.6 matchMedia device-width coherence — minor leak (§2.2)
`window_bootstrap.js:4063-4068`: `device-width`→`inner_width`, `device-height`→`inner_height`. Real Chrome resolves these to `screen.width`/`screen.height` (CSS px). Diverges on windowed profiles. One-line-ish fix to read `screen_width`/`screen_height` for `device-*` and keep `inner_*` for plain `width`/`height`.

### 3.7 Profile diversity — hand-built vs distribution-sampled (§2.5)
`presets.rs:750-825` (`chrome_148_macos_sampled_with_rng`): 3 chips × ~2 screens × small core/RAM pools + random canvas/audio seeds. Covers the *coherence* requirement (chip ↔ cores ↔ GPU ↔ RAM) but the *cardinality* is far below v150's 67. For same-IP anti-clustering on the reliability sites, widen the pool (more screens per chip, add M1/M2 families with matching renderer strings) — keeping the coherence invariant `presets.rs:823 validate()`.

---

## 4. Why v150 specifically wins booking / douyin / duolingo / imdb (mechanism per site)

| Site | v150's winning mechanism | Transferable to BO? |
|---|---|---|
| **imdb / amazon-in (hard AWS)** | Firefox event loop runs challenge.js → blob-worker PoW → `aws-waf-token` POST → reload returns content. NOT fingerprint (BO's fp is accepted). | **YES — §3.1 live-nav drain.** Public engine. Highest ROI. |
| **amazon-fr/jp/com-au (reliability AWS)** | Same self-solve + preset diversity reduces same-IP clustering rejections. | **YES — §3.1 + §3.7.** Public engine. |
| **booking (SPA hydration)** | Body 8.5 KB shell → v150 reaches 465-513 KB. The SPA's `/api/...` hydration fetch chain only fires after the framework's async bootstrap (likely RAF/idle-callback gated + a fetch chain). v150's real event loop lets the chain complete; BO's 50 ms+500 ms drain ends before the SPA hydrates. **Same class as AWS** (async completeness), not a fingerprint bail. | **LIKELY YES — §3.1.** Re-test booking immediately after the drain fix (handoff §5.4 predicted this). Public engine. |
| **duolingo (reCAPTCHA enterprise worker)** | reCAPTCHA enterprise spins a Web Worker that reads `self.location.origin` (FIX-W shipped `self.location` in worker, `audit/15` row 23) and computes a token asynchronously. v150's event loop completes it; BO stops draining first. | **YES — §3.1** (the worker prereqs are in place; needs the main-thread to keep pumping). Public engine. |
| **douyin (`__ac_signature`)** | Firefox's *V8-vs-SpiderMonkey value distribution* produces a signature douyin's server accepts. Patchright (Chromium) also fails → **Firefox-only solve.** | **NO** — contradicts BO's Chrome identity (`vNext/05`). Pursuing it means emulating a Firefox signature inside a Chrome-claiming engine = inconsistency. Skip / vendor_solvers only. |

**Pattern:** 3 of the 4 BO can plausibly take with **one public-engine change** (live-nav async drain). douyin is the outlier and is explicitly not worth chasing under the Chrome positioning.

---

## 5. Ranked fix list (ROI order)

> All effort estimates assume the existing `run_delta_headtohead.py` loop for validation. "Public engine" = fixable in MIT/Apache crates per CLAUDE.md; vendor-algorithm code stays out.

### FIX-1 — Live-nav async-completeness drain (THE lever)
- **What:** In `page.rs` navigate loop (`:1881-2129`), replace the byte-size/CHL-marker gate on the continue/drain decision with an **"async-in-flight" predicate**: keep draining (up to the existing 13-50 s budget) while any owned worker is alive (`worker_ext::worker_registry`), an in-flight fetch counter > 0 (add to `fetch_ext`), or a pending macrotask owns a timer. Mirror the offline oracle's behaviour (`run_until_idle(5-8 s)`) for pages that are still computing. Instrument first (handoff §5.1 step 1): does `op_worker_spawn` ever fire live? does `checkForceRefresh().then` resolve?
- **Effort:** 3-5 days (instrument 1 day; predicate + plumb fetch counter 1-2 days; tune budget + regression 1-2 days).
- **Expected impact:** imdb + amazon-in flip; amazon-fr/jp/com-au reliability up; **booking + duolingo likely flip** (same async class). Up to **~6-7 of the 8-site gap.**
- **Confidence:** high (two independent evidence lines: BO's own oracle-vs-live diff + ZenRows' 17%-cold-vs-86%-warm AWS data).
- **Public engine:** yes.

### FIX-2 — Widen the coherent profile pool (anti-clustering)
- **What:** Extend `chrome_148_macos_sampled_with_rng` (`presets.rs:750`) from 3 chips × ~2 screens to a larger coherent pool: add M1/M2/M2 Pro/M2 Max families with matching `unmasked_renderer` strings + correct cores/RAM/screen, more logical resolutions per chip. Keep the `validate()` invariant (`:823`). Optionally port BrowserForge's *idea* (sample from a frequency-weighted joint distribution) without copying its MPL data — build a small Rust table from real-Chrome-macOS capture frequencies.
- **Effort:** 2-3 days (data gathering + per-chip GpuProfile variants like the existing `apple_m3_pro_macos`).
- **Expected impact:** amazon-fr/jp/com-au reliability (flip flaky → consistent); defends against same-IP clustering generally. 1-3 sites' *reliability*.
- **Confidence:** medium (helps reliability, not the hard 0/5 sites; those need FIX-1 first).
- **Public engine:** yes.

### FIX-3 — Canvas noise: match v150's symmetric deterministic model (or disable)
- **What:** Rewrite `to_data_url_with_jitter` (`canvas2d.rs:1110-1131`) to v150's rule — modify the **first non-zero channel by ±1 deterministically from `canvas_seed`, skip zero channels** (preserves `clearRect` transparency, removes the detectable luminance-biased direction). Alternatively evaluate disabling per FIX-G (`audit/15` row 7 / v150 commit `e4528a2`).
- **Effort:** 1 day.
- **Expected impact:** removes a clusterable canvas tell; 0-3 sites (cross-vendor, hard to attribute), but it is a *correctness* fix that can't hurt and de-risks DataDome/CreepJS-style probes.
- **Confidence:** medium (defensive; unlikely to flip a hard site alone).
- **Public engine:** yes.

### FIX-4 — measureText / font-metric per-profile jitter
- **What:** Add a `spacing_seed`-driven sub-pixel jitter to `measureText` widths (`canvas_bootstrap.js:179`), analogous to v150's HarfBuzz LCG (~0-0.1 em), so two BO instances don't share an identical font fingerprint. Seed from `canvas_seed`/a new `font_spacing_seed` profile field.
- **Effort:** 1-2 days.
- **Expected impact:** anti-clustering on font fingerprint; 0-2 sites; pairs with FIX-2.
- **Confidence:** medium-low (defensive).
- **Public engine:** yes.

### FIX-5 — matchMedia `device-width`/`device-height` → screen dimensions
- **What:** `window_bootstrap.js:4063-4068`: resolve `device-width`/`device-height` from `screen_width`/`screen_height` (CSS px), keep `inner_*` only for plain `width`/`height`. Closes the JS-vs-CSS coherence leak v150 explicitly patches (`nsMediaFeatures.cpp`).
- **Effort:** 0.5 day.
- **Expected impact:** closes one cross-API inconsistency; 0-1 sites; cheap correctness.
- **Confidence:** medium-low.
- **Public engine:** yes.

### FIX-6 (do NOT do) — douyin `__ac_signature`
- **What:** Firefox-only signature solve. Contradicts BO's Chrome identity. Skip; if ever needed, vendor_solvers.
- **Public engine:** no (vendor / open frontier).

---

## 6. Open questions

- Does `op_worker_spawn` *ever* fire in the live imdb path, or does the main-thread drain end before challenge.js's `.then()` even runs? (handoff §5.1 instrumentation step — gates whether FIX-1's predicate should watch workers or also force a longer pre-worker drain.)
- After FIX-1, does booking hydrate (does its `/api` chain fire under a longer drain), confirming it is the same async class as AWS rather than a separate SPA-framework gate?
- Is BO's `chrome_148_macos` WebGL `getParameter`/extension list still byte-matching a *fresh* 2026 real-Chrome-148-macOS capture (OPEN-2, `audit/15`)? Not the AWS lever, but a latent cross-API check.
- Quantify FIX-3/FIX-4 with a CreepJS/`automation-detector` run before/after (scrapfly automation-detector cited) to attribute the defensive fixes.

Sources: [ZenRows — Bypass AWS WAF](https://www.zenrows.com/blog/bypass-aws-waf), [daijro/camoufox (GitHub)](https://github.com/daijro/camoufox), [ScrapingBee — Camoufox](https://www.scrapingbee.com/blog/how-to-scrape-with-camoufox-to-bypass-antibot-technology/), [scrapfly automation-detector](https://scrapfly.io/web-scraping-tools/automation-detector); deepwiki `daijro/camoufox` (cross-process-storage / canvas-spoofing / anti-font-fingerprinting / navigator-spoofing / screen-spoofing patches); local `browserforge` (bayesian_network.py, fingerprints/generator.py) + `camoufox` (fingerprints.py, browserforge.yml).
