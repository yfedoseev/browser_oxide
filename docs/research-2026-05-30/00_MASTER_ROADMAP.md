# 00 — MASTER ROADMAP: Path to v150 Parity + Profile Convergence

**Date:** 2026-05-30
**Branch context:** `fix/v0.1.0-fix4-canvas-parity`
**Synthesizes:** `01_THIN_RENDER.md`, `02_AKAMAI.md`, `03_DATADOME.md`, `04_FIREFOX_WIRE.md`, `05_PROFILE_CONSISTENCY.md`, `06_ENGINE_CORRECTNESS.md`

---

## 1. Executive Summary

### The goal
Two coupled objectives:

1. **Beat Camoufox v150 on site-pass rate.** BO currently loses the same-IP head-to-head on a small contested set. The 7 routed-gap ground-truth sites are: **douyin, duolingo, adidas, ozon, wildberries, homedepot, etsy**. On 5 of 7 (douyin/duolingo/adidas/ozon/wildberries) BO serves a **1.8–13 KB shell** where v150 renders 100 KB–1.5 MB.
2. **Converge all four stealth profiles** (chrome, pixel, iphone, firefox) so they show the **same pass set** — eliminating profile-dependent flakiness in the benchmark and in production.

### The profile-consistency problem (now precisely scoped)
BO has **exactly one** per-profile coherence defect, and it is the **Firefox profile**: it emits a Firefox UA + correct Firefox request headers **over a Chrome-147 BoringSSL ClientHello and Chrome H2 SETTINGS**, because the net layer (`tls.rs`, `h2_client.rs`) branches **only on `device_class`** (Desktop/MobileAndroid/MobileIOS) — there is **no Firefox arm**. `tls_impersonate:"firefox_135"` is a **dead string** the wire layer never reads (`presets.rs:466–474` admits "informational only"). Result: any vendor running a JA4-vs-UA coherence check (DataDome, Cloudflare, AWS WAF, Akamai) sees `Firefox/135 UA + Chrome-class JA4` → instant high-risk bucket. This single defect is the root cause of **every firefox-only loss** (reuters/zillow/wsj/airbnb/spotify/tripadvisor — 6 of the 13 inconsistent sites). chrome+pixel are wire-identical (`CURVES_ANDROID == CURVES_DESKTOP`); iphone is fully coherent (Safari TLS + Safari headers + iOS JS surface). The remaining chrome/pixel/iphone gaps are **content/engine** gaps that fail all 4 profiles equally — they are **not** a coherence problem and converge together when the engine gap closes.

### Corrected facts (do not regress these)
- **ozon and wildberries are NOT IP blocks.** They are the **thin-render / ES-module-execution gap** — Vite/React/Vue SPAs ship `<script type=module>` entries that BO drops with `SyntaxError: Cannot use import statement outside a module` because the document script path uses classic `v8::Script::compile` only. The module-loading capability **already exists** in the codebase (used for module *workers*) — this is a **wiring gap, not a missing capability**.
- **homedepot is NOT a hard `_abck~-1~` 403 anymore.** The historical Akamai H2-fingerprint edge gate is **resolved in code** (`h2_client.rs` now emits Chrome-exact 15663105 WINDOW_UPDATE + weight-256 exclusive priority); BO reaches an **in-band sec-cpt interstitial**. The residual failure is **non-deterministic budget timing**, not a fingerprint gap.
- **bestbuy is NOT Akamai.** It is the "Choose a country" i18n splash that **real Chrome from this IP also gets** — a classifier-FP / geo-cookie problem (`?intl=nosplash`).
- **adidas IS a genuine holistic-ML `_abck~-1~` tail** with no cheap public-engine lever — explicitly de-scoped.
- **etsy is a DataDome WASM token-producer gap**, not a child-iframe cookie-jar bug (jar is verified shared end-to-end). The token signer lives in the private `vendor_solvers` crate and `Page::navigate` registers an empty solver set.

---

## 2. ROI-Ranked Fix Table (all clusters, single view)

Ordered by ROI = (expected flips × confidence) ÷ effort. "Flips" are *expected* and contingent where noted.

| # | Change | File(s) | Effort | Conf. | Expected flips |
|---|--------|---------|--------|-------|----------------|
| **1** | **Verify delta harness calls `navigate(url, profile, 3)`** — post-solve reload is hard-gated `iter+1<iterations` (page.rs:2736); a 1-iter run *cannot* flip sec-cpt and reports a false engine-gap | `benchmarks/run_delta_headtohead.py` | S (assert) | high | removes false-fail (homedepot); prereq for #6/#7 to be measurable |
| **2** | **INTERIM firefox convergence:** route the 6 firefox-loss sites through `chrome_148` (or drop firefox from rotation for JA4-cross-checking vendors). Firefox profile is strictly worse than Chrome on every cross-check site | `page.rs` profile selection / benchmark picker | trivial | high | collapses 13-site inconsistency to chrome pass set; no *new* passes but kills firefox-unique divergence (reuters/zillow/wsj/airbnb/spotify/tripadvisor) |
| **3** | **bestbuy geo-cookie follow:** on i18n-splash body (title "Select your Country", zero `_abck`/`sec_cpt`/`sensor_data`), re-issue `?intl=nosplash`; confirm classify.rs:175–186 de-attributes splash from Akamai-CHL | `page.rs` nav loop + `classify.rs:175–186` | S | high | bestbuy THIN/Akamai-CHL → rendered |
| **4** | **De-scope adidas** from flip list (ML-tail, no cheap lever; T1.3/T1.5/canvas/humanize all closed, slot unchanged). Tag "needs synchronized clean-IP sensor diff" | doc scoping (`02_AKAMAI.md`) | N/A | high | none — prevents wasted sessions |
| **5** | **Tighten `is_datadome_challenge`** to require interstitial structure (`rt:'i'` / dd config object) in addition to `captcha-delivery.com` + 50KB cap | `page.rs:208` | low | high | none (robustness; prevents real-page misclassification) |
| **6** | **sec-cpt budget arm on solve:** in the poll early-break (page.rs:2324–2346), when `is_seccpt_solved` fires, do `nav_budget += 45s` (floor remaining at MIN_RETRY_BUDGET) **before** break, so cookie-only `~3~` solve gets guaranteed build+drain budget — not only the lucky JS-pending-nav branch. Gated by `started_as_seccpt_challenge` ⇒ zero non-seccpt regression | `page.rs:2324–2346` | S (~6 lines) | high | **homedepot** chrome/pixel/iphone (firefox blocked separately by missing FF-TLS) |
| **7** | **Raise homedepot/sec-cpt host budget** 45_000 → 60_000 ms (must cover build + ~1MB bundle + sha256 PoW + server `chlg_duration` 5–30s + reload + drain). b623d5d needed ~119s | `page.rs:1984–1993` | S (1 line) | medium | stabilizes homedepot across Akamai config rotations |
| **8** | **Route document `type=module` scripts** through existing `load_main_es_module_from_code` + `mod_evaluate` (proven in worker_ext.rs:347–372) + host ModuleLoader (relative-specifier resolve vs doc URL, recursive import-graph prefetch) + dynamic `import()` host callback; gate `find_scripts` + `dom_bootstrap` to dispatch module scripts there | `lib.rs:122`, `script_runner.rs:44`, `dom_bootstrap.js:146` (+ new loader) | medium (capability exists) | high | **duolingo, adidas, ozon, wildberries, douyin** (5 of 7) — contingent on #9/#10 |
| **9** | **Fix `run_until_idle` idle semantics:** AllWorkDone is terminal ONLY when no timers/intervals scheduled before deadline; else sleep until `min(next_timer_deadline, deadline)`. Un-ignore the `timeout_respected` regression test | `event_loop/lib.rs:316–376` (test :524) | small | high | deferred-hydration SPAs; lets the now-executing module app finish |
| **10** | **Content-adaptive SPA nav-budget:** grant 90s tier when a `type=module` entry or React/Vue global is detected, instead of hardcoded host allowlist | `page.rs:1953–2005` | small | high | douyin, duolingo, ozon, wildberries (currently capped at 15s) |
| **11** | **IntersectionObserver re-fire** on DOM mutation / synthetic scroll (currently one-shot `Promise.resolve().then` per `observe()`) | `window_bootstrap.js:3543–3570` | medium | medium | douyin, wildberries (feed/grid hydration) |
| **12** | **Firefox TLS arm** in `chrome_connector`, gated on browser family (thread `browser_name` / make `tls_impersonate` load-bearing): NSS cipher order, `record_size_limit` ext28, `delegated_credentials` ext34, FFDHE2048/3072 in supported_groups, real ECH (not GREASE), fixed (non-shuffled) extension order. boring2 4.15 can express all via the same `SslConnector::builder` as the Safari-iOS arm | `tls.rs:247–262` | high | high | **firefox cohort** (reuters/zillow/wsj/airbnb/spotify/tripadvisor); SIMULTANEOUSLY unblocks **DataDome etsy/tripadvisor** (Firefox-NSS = documented only DD bypass) |
| **13** | **Firefox H2 arm:** third branch in handshake — keep SETTINGS 0x3 (Chrome omits it), Firefox SETTINGS wire order + pseudo-header order, RFC7540 priority TREE instead of Chrome's single weight-255 exclusive HEADERS dependency | `h2_client.rs:110–175` | medium | high | completes FF wire coherence; required alongside #12 for full JA4+H2 cross-check parity |
| **14** | **Firefox JA4 coherence test** mirroring the Chrome assert (tls.rs:552–557) + byte-capture test (tls.rs:568) — the missing canary that lets a Chrome wire silently ride under a Firefox UA | `tls.rs` test module | low | high | none (prevents regression of #12/#13) |
| **15** | **Investigate DataDome WASM live-nav drain:** does `run_until_idle(200ms)` starve the DD bundle's blob-worker / requestIdleCallback PoW chain (same class as AWS-WAF M-1)? If the daily-key WASM can be driven to POST a valid token from-scratch, etsy renders with **no vendor solver** — the only public-scope etsy path | `page.rs:2244–2311` (+ timer/window bootstrap drain) | medium | medium | etsy (only if DD verification is pure-client PoW, not server-ML-gated) |
| **16** | **elementFromPoint** return `null` for OOB coords (full fix = layout hit-test); replace worker `setInterval(_,5)` with self-rescheduling `setTimeout` that yields when queue drains | `dom_bootstrap.js:1524`, `worker_bootstrap.js:261` | small | medium | FP parity polish (CreepJS/PX layout lie-detector) |
| **—** | **(scope-policy, not public)** Register byte-verified `DataDomeSolver`/`DdEncryptor` behind `--features vendor_solvers`; all plumbing already wired | `page.rs` default_solvers + `datadome_crypto.rs:371` | low | high | etsy (private/vendor build only — **declined for public engine per CLAUDE.md**) |

---

## 3. Phased Plan

### Phase 0 — Measurement hygiene (do FIRST, gates everything)
- **#1** Confirm delta harness uses `navigate(url, profile, 3)`. A tight-iteration or tight-budget run makes homedepot a false engine-gap and makes #6/#7 unmeasurable.
- **#4** De-scope adidas in the docs to stop session burn on a known dead-end.

### Phase 1 — Quick wins (trivial/small, high confidence, low blast radius)
- **#2** Interim firefox→chrome routing — collapses the profile-inconsistency table immediately (no wire work).
- **#3** bestbuy geo-cookie follow → rendered.
- **#5** Tighten `is_datadome_challenge` (robustness).
- **#6 + #7** sec-cpt budget arm + budget tier raise → **homedepot deterministic** on chrome/pixel/iphone.

### Phase 2 — Wire the ES-module class (highest-ROI single capability)
- **#8** Route `type=module` document scripts to the existing module-eval path. This is the load-bearing fix for **5 of 7** ground-truth sites.
- **#9** Fix `run_until_idle` idle semantics (lets the now-executing app finish).
- **#10** Content-adaptive SPA budget (lets the app *get* the time it needs).
- **#11** IntersectionObserver re-fire (feed/grid hydration).
- These four are interdependent: #8 without #9/#10 will execute the module but starve/timeout it. Land and measure as a set.

### Phase 3 — Challenge / render endgame (medium effort, contingent)
- **#15** DataDome WASM live-nav drain investigation — the only public-scope etsy path; medium confidence, gated on whether DD is pure-client PoW.
- homedepot residual stabilization rides on Phase 1 #6/#7 + harness #1.

### Phase 4 — Profile convergence (high effort, biggest single coherence payoff)
- **#12** Firefox TLS arm — converges the firefox cohort AND is the documented **only** DataDome bypass (unblocks etsy/tripadvisor on the firefox profile).
- **#13** Firefox H2 arm — required alongside #12 for full JA4+H2 cross-check parity.
- **#14** Firefox JA4 coherence test — locks the fix against silent regression.
- Once #12–#14 land, retire the interim #2 routing: the firefox profile becomes genuinely coherent rather than masked.

---

## 4. Prior Research to Reuse

| Asset | Path | Reuse for |
|-------|------|-----------|
| Akamai sec-cpt unblock analysis | `docs/research/engines/UNBLOCK_akamai_seccpt.md` §2.3 | Already flagged budget as "implicit and brittle" — direct support for #6/#7 |
| adidas BMP dead-end record | `docs/universal_engine/site_debugging/adidas_akamai_bmp_v3.md` | Proof adidas is ML-tail (POST→201, `_abck~-1~` unchanged across audio/worker/canvas/humanize) — supports #4 de-scope |
| Real-Chrome adidas sensor capture | `docs/akamai_sensor_analysis/scratch/adidas-network.json` | Synchronized clean-IP sensor diff if adidas is ever re-opened |
| Firefox-NSS bypass + JA4 string | `browser_oxide_internal/docs/GAP_DEEP_ANALYSIS_2026_04_28.md:206` (JA4 `t13d1715h2_5b57614c22b0_3d5424432f57`), `:163` (Akamai H2 WINDOW_UPDATE bucketing) | Target JA4 for #12; H2 context for #13 |
| Byte-verified DD crypto (dead code) | `datadome_crypto.rs:371` (`DdEncryptor`) | Vendor-build etsy flip; confirms gap is wiring+policy, not crypto |
| Byte-verified Akamai sec-cpt solver (dead code) | `crates/akamai/src/sec_cpt.rs` | Reference math (note: un-wireable for homedepot — obfuscated bundle, not parseable 428 JSON) |
| Module-eval capability (proof) | `worker_ext.rs:347–372` (`load_main_es_module_from_code` + `mod_evaluate`) | Template for #8 document-module wiring |
| Now-resolved edge-fp baseline | `GAP_DEEP_ANALYSIS_2026_04_28.md` (homedepot H2) | Confirms homedepot is past the fingerprint gate — don't re-hunt it |
| AWS-WAF live-nav drain blocker | `docs/HANDOFF_2026_05_28b.md` §5.1 | Same class as #15 (DD WASM drain) — reuse the drain instrumentation |
| Stale-doc verification | `06_ENGINE_CORRECTNESS.md` (BatteryManager/VisualViewport/AudioContext/MediaSession now FIXED) | Don't re-do the 2026-04-29 FP inventory; only `elementFromPoint` survives |

---

## 5. Honest Risks & Unknowns

1. **#8 module-exec is high-ROL but NOT a guaranteed 5-site flip.** The 5 flips are *contingent* on #9 (idle) + #10 (budget) + possibly #11 (observer). A module that executes but starves on a 15s budget or an early `AllWorkDone` still renders a shell. Land Phase 2 as a unit and measure together; do not claim individual flips before the set lands.
2. **Module import-graph prefetch is non-trivial.** Relative-specifier resolution vs document URL, recursive graph fetch, and dynamic `import()` host callbacks are new surface. CSP / cross-origin module fetches may hit the same script-created-iframe-never-fetched class (FP-E1). Risk of partial graphs → partial hydration.
3. **homedepot remains stochastic by nature.** Even with #6/#7, a high-end server-enforced `chlg_duration` (30s) rotation can still pressure the budget. #6+#7 *reduce* dependence on budget luck but the site is documented borderline; expect occasional rotation-dependent misses.
4. **etsy has no in-scope deterministic path.** #15 (WASM drain) is medium-confidence and **fails entirely if DD verification is server-ML-gated rather than pure-client PoW**. The only certain etsy flip is the vendor-solver build, which is **out of public scope**. Do not promise etsy on the public engine.
5. **Firefox wire (#12/#13) is the largest effort and has hidden boring2 risk.** Findings claim boring2 4.15 can express NSS ciphers / FFDHE / record_size_limit / delegated_credentials / real ECH via the same builder — but the "boring2-cannot-emit-NSS" blocker was *previously* believed true and is now called stale. Validate each primitive emits correct bytes via a captured real-Firefox ClientHello **before** assuming the full arm lands.
6. **adidas (and the broader holistic-ML tail) has no public lever.** De-scoped on purpose. A future synchronized clean-IP sensor diff is the only lead, and it requires infrastructure not currently in place.
7. **Interim #2 masks rather than fixes.** Routing firefox→chrome makes the benchmark converge but hides the real defect; it must be retired once #12–#14 land or it will paper over a future firefox regression.
8. **Measurement coupling:** several "flips" are only observable if Phase 0 #1 is correct. If the harness silently uses <3 iterations, homedepot, and any sec-cpt-class site, will under-report regardless of code correctness.

---

## 6. One-Line Bottom Line

The highest-ROI work is **Phase 1 quick wins (homedepot deterministic, bestbuy geo-follow, firefox interim convergence)** followed by **Phase 2 wiring the already-present ES-module capability** (5-of-7 sites, contingent on idle+budget fixes landing together) — with the **Firefox TLS/H2 wire (Phase 4)** as the single highest-effort lever that converges the profile set AND unblocks DataDome. ozon/wildberries are module-exec gaps (not IP blocks), adidas is a de-scoped ML-tail, and etsy has no deterministic public-engine path.
