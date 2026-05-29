# CHANGES MANIFEST — every gap, the change, and why (v0.1.0-match)

> Single authoritative checklist consolidating ALL changes identified in this
> workstream, across three tracks: **A) per-profile consistency**, **B) engine
> performance**, **C) benchmark harness/methodology**. Each row links to the
> detailed root-cause doc. Nothing here is implemented yet — this is the
> execute-later plan. Status legend: ⬜ not started · 🔬 documented/root-caused.
>
> Baseline at time of writing (2026-05-29 gate): BO **115/125 routed**
> (chrome 110 / pixel 108 / iphone 108 / firefox 106); leads Camoufox v150
> baseline ~112-113. Goal: each profile individually → ~115 (per-profile parity)
> + faster engine.

---

## TRACK A — Per-profile consistency (close the 20 gap sites)

Why: a site in the routed-115 should pass on ALL 4 profiles, and each profile
should match v150 single-engine. Root cause (clusters 01/02/07): **per-profile
TLS/JA4-and-HTTP2 fingerprint doesn't match the profile's UA** — `tls.rs:241` /
`h2_client.rs:85` branch only on `device_class`, never `browser_name`, so the
Safari and Firefox profiles ship Chrome-ish wire bytes that CF/DataDome/PX flag.
Full ranked table: **`08_MASTER_CONSISTENCY_ROADMAP.md` §4** (17 rows). Headlines:

| ID | Change | File:line | Why (gap) | Profiles | Gain | Effort | Doc |
|---|---|---|---|---|---|---|---|
| 🔬 A1 `D3` | Fix iOS-Safari cipher SET: drop 3DES `0xC008`/`0xC012`, add `0xC00A`/`0xC014` before `0x000A` | `net/src/tls.rs:111-132` | iphone JA4 cipher-hash matches NO real Safari → CF JA4-Signals challenges economist/ecosia/ft/openai/quora/udemy | iphone | **+4-6** | 1-2h | 01 |
| 🔬 A2 `D1/FF-WIRE` | Real Firefox-135 TLS+H2 class (`browser_name=='Firefox'` branch: NSS ciphers, no MLKEM lead, fixed ext order, no ALPS/ECH-grease/Brotli; H2 SETTINGS 65536/0/131072/16384, pseudo m,p,a,s) | `net/src/tls.rs:233`, `h2_client.rs:78` | firefox ships Chrome wire under FF UA → DataDome/PX flag reuters/wsj/tripadvisor/zillow | firefox | **+3-4** | 1-2wk | 02,07 |
| 🔬 A3 `D2` | Author `android_mali_g715` + `ios_apple_a17_pro` GpuProfiles; wire into mobile presets + cross-API test | `stealth/src/gpu.rs`, `presets.rs:931,1035` | mobile profiles use apple_m3 desktop GPU → WebGL/UA incoherence | iphone, pixel | +3-5 iphone (overlaps A1) | 1-2d | 01,05,07 |
| 🔬 A4 `PX1` | `chrome_headers_fetch` device-aware `sec-ch-ua-mobile ?1` for mobile (mirror `chrome_headers_impl` is_mobile) | `net/src/headers.rs:250` | pixel sends desktop mobile-hint → airbnb/yandex THIN-BODY/error | pixel | +2-3 | ~10 LOC | 03 |
| 🔬 A5 `UBER-BUDGET` | Add `uber.com` to the 90s SPA-shell budget arm | `browser/src/page.rs:1962` | uber heavy SPA times out on desktop/pixel (iphone lighter, passes) | chrome,ff,pixel | +2-3 | 1 line | 04 |
| 🔬 A6 `DRAIN-B` | Challenge-aware inter-script drain (≥1-2s burst when `doc_is_challenge`, V8DeadlineWatcher-capped) | `browser/src/page.rs:3661,3594` | per-profile sec-cpt solve variance (homedepot) | iphone (+AWS/booking) | +1 | 1-2d | 05 |
| 🔬 A7 `FF-GUARD` | Interim: don't route Firefox UA to JA4-cross-checking vendors until A2 ships | routing | prevents guaranteed firefox losses pre-A2 | firefox | 0 (prevents loss) | 0.5-1d | 02,07 |
| 🔬 A8 tests | JA4 byte-regression (`JA4-REG`) + capture corrected per-profile JA4/akamai-H2 from tls.peet.ws (`JA4-CAP`) | `net/tests/` | lock A1/A2/A3 against drift; validate the wire fixes | — | regression insurance | 1-4h | 01,07 |

Lower-ranked / diagnostic rows (PX3, PX2, D3-PAD, PX-GPU/H2, SECCPT-ORACLE,
SPOTIFY-DIAG, DD-VERIFY) are in 08 §4 rows 7-17. **yelp = unwinnable** (DataDome
`rt:'c'` interactive captcha; Camoufox also fails — do not chase).

**Projected per-profile after Track A** (08 §6): iphone 113-114, firefox 111-112,
pixel 112-114, chrome 112-113 → routed 118+.

---

## TRACK B — Engine performance (init + reuse)

Why: BO pays ~1.5s cold init per process and can't reuse a process across navs.
Full analysis: **`GATE_PERFORMANCE.md`**.

| ID | Change | File:line | Why (gap) | Impact | Effort | Doc |
|---|---|---|---|---|---|---|
| 🔬 B1 compile-time snapshot | Move `snapshot::get_snapshot()` from runtime `OnceLock` build to a **`build.rs` compile-time snapshot** + `include_bytes!` | `js_runtime/build.rs` (new), `js_runtime/src/snapshot.rs`, `lib.rs:53` | every fresh process REBUILDS the V8 snapshot (executes 12 bootstraps) = **1543ms**; per-site processes rebuild it 126× | cold init **1.5s→~0.1s (~15×)**; biggest **production** cold-start win + ~3min/profile gate | 0.5-1d | GATE_PERFORMANCE §3b |
| 🔬 B2 per-nav resource leak | Reap owned Web Workers (`worker_ext` registry), cancel/unref timers, drop V8 isolate/DOM arena on `Page` drop | `browser/src/page.rs` Page::drop, `worker_ext.rs`, `timer_bootstrap.js` | single process accumulates → **runaway** (1.7GB RSS, 100% CPU, stuck ~site 104 @7h); forces per-site isolation + bloats prod RSS | unlocks process reuse (pool/`navigate_warm`) + lower steady-state RSS | 2-4d | GATE_PERFORMANCE §5 |
| 🔬 B3 process reuse (pool) | After B2, use `PagePool`/`navigate_warm` in the gate + production to pay init ONCE per process, reuse across navs | `browser/src/page.rs:navigate_warm`, `parallel.rs` | reuse path exists but blocked by B2 leak | amortizes init across navs (the "reuse" win) | (gated on B2) | GATE_PERFORMANCE §5 |

---

## TRACK C — Benchmark harness / methodology

Why: fair, fast, reproducible same-IP measurement. Docs: `CAMOUFOX_INSTALL.md`,
`GATE_PERFORMANCE.md`, `06_AMAZONCA_GATE_CLUSTERING.md`.

| ID | Change | File | Why (gap) | Effort | Doc |
|---|---|---|---|---|---|
| 🔬 C1 AWS spacing | Wall-clock ≥150s same-vendor gap in the BO gate runner (not adjacency-only 30s) | `benchmarks/run_bo_isolated.py` | back-to-back AWS sites token-cluster → false fails (amazon-ca 5KB gate vs 1.03MB spaced; redfin CHL@392KB) | 0.5d | 06 |
| 🔬 C2 parallel BO gate | `--parallel N` + vendor-aware scheduler (≤N concurrent, never 2 same-vendor in flight / within 150s) | `benchmarks/run_bo_isolated.py` | BO gate is serial nav-bound (~60min/profile, 1 of 8 cores used) | BO gate **~4h→~1h** | 1-2d | GATE_PERFORMANCE §4 |
| ✅ C3 camoufox v150 install | Documented working trick: separate venv + `sed MIN_VERSION→alpha.1` + manual v150 asset download (`alpha.26` lin.x86_64) + `XDG_CACHE_HOME` cache | `CAMOUFOX_INSTALL.md` §3b | v150 tagged beta.25 but asset is `alpha.26`; launcher MIN='beta.19' rejects `alpha<beta` → fetch silently falls back to v135 | done (doc) | CAMOUFOX_INSTALL |
| ✅ C4 competitor harness | Chromium-tier via shared-browser `bench_corpus_v2` (stable+fast); camoufox via per-site `run_competitor_isolated.py` (driver crashes shared) + retry | benchmarks/ | per-site Chromium relaunch = 10h; shared = 3h. Camoufox driver unstable shared | done (harness) | GATE_PERFORMANCE §2 |

---

## TRACK D — Frontier (pass-everything via the no-CDP moat)

Why: the "frontier 10" are NOT all out of scope. **5-6 are engine-addressable**
in the public engine on this IP, leveraging BO's no-CDP advantage (proven:
no-CDP real Chrome passes Kasada from this IP; CDP Playwright gets 429 same IP).
Full analysis: **`../v0.1.0-frontier-workflows/`** (01-08).

| ID | Change | File:line | Why (gap) | Site(s) | Effort | Doc |
|---|---|---|---|---|---|---|
| 🔬 D0 no-CDP oracle | Build `nocdp_oracle.rs`/`nocdp_capture.rs` (from awswaf_probe/aws_capture) + move nocdp.sh/tl_capture to `tools/oracle/`; the capture+diff enabler for all frontier work | `crates/browser/examples/` | can't fix what we can't see a passing trace of; Playwright/MCP are CDP-detected (invalid oracle) | ALL | 1-2d | frontier 06 |
| 🔬 D1 moat guardrail | Gate `crates/protocol` CDP server behind an **off-by-default `cdp-server` Cargo feature**; standing rebrowser-bot-detector test (clean by construction; FAIL if CdpServer bound) | `crates/browser/Cargo.toml:36`, `crates/protocol` | the no-CDP moat is conditional — CDP server is a dep, must never reach the navigate path | ALL (protects moat) | 0.5d | frontier 07 |
| 🔬 D2 Kasada child-realm | Populate the near-empty iframe child global (document/navigator/constructors/timers/fetch/storage, realm-distinct) | `js_runtime/src/extensions/dom_ext.rs:1217-1247` | child realm only has Window/self/globals → Kasada `contentWindow` probe (bot1225) hard-fails | hyatt/canadagoose/realtor | 2-4d | frontier 01 |
| 🔬 D3 Kasada K2-DIFF | In-VM `/tl` plaintext dump (hook fetch/XHR pre-XOR, env-gated) + field-diff vs no-CDP real `/tl` | `js_runtime/src/js/fetch_bootstrap.js` | bounds the Kasada residual to a named field list | Kasada×3 | 3-5d | frontier 01 |
| 🔬 D4 DataDome cookie-jar | `ChildIframe::from_url` child V8 must use the **shared** session jar, not a fresh isolated one | `net/lib.rs:308` vs `363-368`, `runtime.rs:84-90`, `page.rs:2474` | child-iframe DataDome clearance cookie never reaches parent → etsy/tripadvisor stay CHL | etsy, tripadvisor (+CF Turnstile) | 1-2d | frontier 02 |
| 🔬 D5 douyin probe | R1 offline acrawler trace (sign() throws vs returns-rejected) → R2 builtin-integrity diff vs no-CDP real Chrome (fix via `_maskAsNative`) | `stealth_bootstrap.js:25-104` | reclassified UP from Firefox-only — may be a fixable Chrome integrity leak | douyin | 1-2d | frontier 05 |

**Genuinely NOT engine-addressable (do not chase as engine bugs):** bestbuy
(IP/ASN — datacenter can't reach Favorable `_abck`; no passing reference engine),
ozon + wildberries-trust (need RU/residential IP), yelp (human slider captcha —
even vendor_solvers can't pass). See frontier 03/04/02.

---

## Execution order (recommended)

1. **B1 (compile-time snapshot)** — self-contained, big production win, fast.
2. **A1 (Safari cipher) + A8 (JA4 tests)** — 1-2h, +4-6 iphone, highest ROI consistency fix.
3. **C1 + C2 (gate spacing + parallel)** — makes re-measurement fast/fair for validating the rest.
4. **A4 (pixel headers) + A5 (uber budget)** — cheap, +4-6 across pixel/desktop.
5. **A3 (mobile GPUs) + A6 (drain)** — medium, lifts iphone/pixel.
6. **A2 (Firefox wire class)** — the big firefox lever (1-2wk); gate behind A7 until shipped.
7. **B2/B3 (leak fix + reuse)** — production scaling + enables shared-process gate.

Re-run the full gate (4 profiles × 126 + all 5 competitors, spaced) after each
phase via `benchmarks/run_full_gate.sh` + `run_all_competitors.sh`;
regenerate `../v0.1.0-parity-workflows/02_FULL_GATE_VERIFICATION.md` with
`build_gate_report.py`.

— 2026-05-29
