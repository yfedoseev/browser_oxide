# 01 — MASTER ROADMAP: PATH TO 126

> **Synthesis of 27 sub-agent workflow docs** under `docs/v0.1.0-parity-workflows/{external,sites,api}/`.
> Trustworthy baseline: **same-IP delta head-to-head (12 contested sites) = BO 5 / Camoufox v150 8 → BO behind by 3.**
> Source of truth for the gating root cause: `docs/HANDOFF_2026_05_28b.md` §4–§5.
> Date: 2026-05-28. Branch: `fix/v0.1.0-fix4-canvas-parity`.

---

## 1. EXECUTIVE SCORECARD

### 1.1 Headline

| Metric | Value |
|---|---|
| Corpus size | 126 |
| Diagnostic-excluded (by design) | 1 (`areyouheadless`, 3653 B body, sub-15 KB + flag) |
| Honest production denominator | **125** |
| BO routed-best-of-4 passing | ~107 |
| Non-passing (engine-addressable surface) | **~18** (19 incl. areyouheadless) |
| Same-IP contested delta (12 sites) | **BO 5 / v150 8** |
| Deficit to close to MATCH v150 | **-3** |
| Deficit to OUTPERFORM v150 | -3 then +N |

### 1.2 The single most important finding (all 27 docs converge here)

**v150 does NOT beat BO on fingerprint fidelity. BO's fingerprint is accepted.** Camoufox wins because it is a real Firefox whose native event loop runs anti-bot challenge JS (AWS PoW worker, SPA hydration, reCAPTCHA worker, Kasada/DataDome self-solve) **to async completion**, whereas BO's live `navigate` path drains only **50 ms inter-script** (`page.rs:1678`, warm-rebuild `:3566`) + **500 ms final** (`page.rs:1730`) — versus the offline oracle / cold path's **8 s** (`page.rs:611`, `:3643`). The page's own challenge script starts, then BO tears the page down before its deferred `.then()` → `new Worker(blobUrl)` → PoW → token-POST → reload chain advances.

This **one lever** — a live-nav async-completeness drain — is independently named as the #1 fix by **9 of 27 sub-docs** (AWS, booking, homedepot, Kasada, wildberries, bestbuy, timing, workers/crypto, camoufox-engine). It is the spine of Phase 1.

### 1.3 The 18 non-passing sites by stratum

**Stratum A — Async self-solve gated (the live-nav drain class). v150 passes, BO can flip.**
| Site | Vendor | BO today | Root cause |
|---|---|---|---|
| imdb | AWS-WAF | 0/5 | challenge.js stub sticks; PoW worker never POSTs token |
| amazon-in | AWS-WAF | 0/5 | same |
| amazon-fr / jp / com-au | AWS-WAF | flaky | drain budget race (build-phase half-consumes 15 s) |
| amazon-ca / com | AWS-WAF | flaky/0 | same cluster |
| booking | AWS-WAF (misclassified as SPA) | fail | byte-identical 1.37 MB challenge.js; same drain lever |
| duolingo | reCAPTCHA Enterprise | fail | invisible v3 worker in script-created cross-origin iframe never instantiated as realm |

**Stratum B — BO already wins or is at parity; needs durability/reliability hardening.**
| Site | Vendor | Status | Risk |
|---|---|---|---|
| homedepot | Akamai sec-cpt | BO ~3/5, v150 ?? | long-timer unref race; "flip" does not reproduce 5/5 |
| x-com | shared-cookie | BO 5/5 (delta) | band-aid only on cold path; PagePool path silently unpatched |

**Stratum C — both BO and v150 fail; forward progress = outright win, or honest frontier.**
| Site | Vendor | BO vs v150 | Reachability |
|---|---|---|---|
| wildberries | wbaas (in-house) | BO 7900 B (iphone) > v150 THIN-39 | drain lever → outright win |
| etsy | DataDome rt:'i' | both fail | child-iframe cookie isolation + daily-key (vendor) |
| canadagoose / hyatt / realtor | Kasada | both fail | drain + JS-fidelity tail; needs K2-DIFF to bound |
| bestbuy | Akamai BMP | both fail (incl. Patchright) | long-shot drain; else vendor+proxy |
| douyin | acrawler L1 | both inconclusive (v150 driver crash) | signature VALUE; native-builtin + audio tells |

**Stratum D — out of ledger / unflippable.**
| Site | Why |
|---|---|
| areyouheadless | diagnostic by design; sub-15 KB; v150 also fails (3668 B). Do NOT open a stealth ticket. |

---

## 2. CONSOLIDATED FIX TABLE — ranked by ROI = (sites × confidence) / effort

Deduplicated across all 27 docs. The drain lever appears once (M-1) and is referenced by all its aliases. Tag: **[P]** public engine, **[V]** vendor_solvers/frontier, **[G]** guard/regression (0 flips, protects work).

| # | Fix | Sites moved | Effort | Conf | Tag | file:line anchor |
|---|---|---|---|---|---|---|
| **M-1** | **Live-nav async-completeness drain.** Replace byte/CHL-marker gate + fixed 50 ms inter-script (`page.rs:1678`, warm `:3566`) + 500 ms final (`:1730`) with an *async-in-flight* predicate: keep draining (up to the existing 13–50 s budget, like cold 8 s at `:611`/`:3643`) while any owned worker is alive (`worker_ext` registry), in-flight fetch counter > 0 (add to `fetch_ext`), or a pending timer-owning macrotask exists. Instrument first (`BROWSER_OXIDE_EVENT_LOOP_PROFILE`, `event_loop/lib.rs:316-376`). | **6–9** (AWS cluster 7 + duolingo + booking) | 3–5 d | high | [P] | `page.rs:1678,1730,3566,3643,611`; `event_loop/lib.rs:365` |
| **M-2** | **AWS-WAF challenge predicate + poll arming.** Add `is_awswaf_challenge(html)` (len<4096 && `AwsWafIntegration` && `gokuProps`) + `started_as_awswaf_challenge` at `page.rs:1871`, mirroring `is_datadome_challenge`(`:208`)/`started_as_dd`(`:1845`). OR it into the poll gate (`:2177`) and cookie-delta re-fetch gate. Add `is_awswaf_solved(cookies,body)` (`aws-waf-token=` && !challenge), mirroring `is_seccpt_solved`(`:242`). **Confirmed missing today** — AWS markers only appear inline at `page.rs:2521-2522`, no predicate/flag/poll exists. | enables/stabilizes all M-1 AWS flips | 1 d | high | [P] | `page.rs:208,221,242,1845,1871,2177,2215,2521` |
| **M-3** | **Keep long timers REFED on challenge navs.** Thread `__keepLongTimersRefed` (set when `started_as_{seccpt,dd,cf,awswaf}_challenge`) into `timer_bootstrap.js:57-59` so the 5–30 s `chlg_duration` wait pins the loop instead of unref at `UNREF_THRESHOLD_MS=2000`. **Verified**: unref-at-2000ms is the homedepot daily-difficulty race. | homedepot 3/5→5/5; reinforces AWS/booking | 0.5–1 d | high | [P] | `js_runtime/.../timer_bootstrap.js:57` |
| **M-4** | **Host nav-budget arm for AWS.** Add `amazon.*`/`imdb.com`/`*.token.awswaf.com`-seen to the host budget table at the 25 s Akamai-BMP tier (`page.rs:1902-1949`) so PoW+token+reload fit vs the 15 s default the build-phase half-consumes. Same mechanism as existing Kasada/Akamai arms. | AWS fr/jp/com-au reliability | 0.5 d | high | [P] | `page.rs:1902-1949` |
| **D2-W** | **Worker `WorkerNavigator` class + fp coherence.** Replace worker navigator object-literal with a real class exposing props as prototype getters (match window shape). Add chrome_compat test: worker OffscreenCanvas/audio/WebGL/navigator hashes == window (same seed). | 0 direct (diagnostic) | 1–2 d | high | [P] | `worker_bootstrap.js:139-160` |
| **F3** | **Worker-thread fetch identity fix.** `op_worker_sync_fetch` runs on a `std::thread::spawn`'d thread where thread-local `FETCH_CLIENT` is None → falls back to `chrome_148_linux()` (confirmed live Linux UA leak on a macOS page). Seed helper-thread client from the worker's profile. | duolingo prereq; AWS blob workers; window↔worker coherence | 1–2 d | high | [P] | `worker_ext.rs` (op_worker_sync_fetch) |
| **MASK-1** | **Sweep getters/setters.** Add `desc.get`/`desc.set` to the universal prototype-sweep collection loop (`cleanup_bootstrap.js:529-534`); `_maskAsNative` already names them. Clears ~15 leaking accessors (Request.signal, Response.*, Streams.locked, MessagePort.onmessage, URLSearchParams.size, WebSocket.*). | 0 alone; compounds Kasada/DataDome | 1–2 h | high | [P] | `cleanup_bootstrap.js:529-534` |
| **NAV-1** | **Clamp `navigator.deviceMemory` ≤ 8.** Getter returns raw `_pInt("device_memory",8)` (`window_bootstrap.js:979`) but sampler sets 16/18/24/36/48 (`presets.rs:797`) — **confirmed deterministic tell** (real Chrome caps at 8). Clamp in main + worker getter; tighten `validate()` to reject JS deviceMemory>8; keep physical RAM for non-JS coherence. | AWS cluster tell-removal | 0.5 d | high | [P] | `window_bootstrap.js:979`; `worker_bootstrap.js:148`; `presets.rs:797`; `profile.rs:302` |
| **NAV-2** | **`navigator.vendor` profile-read.** Hard-coded `"Google Inc."` at `window_bootstrap.js:963` → leaks vendor=Google under Firefox UA (100 % impersonation breaker) AND mismatches worker (already reads `_p("vendor")`). Change to `_p("vendor")`. | any firefox_135_* preset site | 1 h | high | [P] | `window_bootstrap.js:963` |
| **X1** | **x-com cookie-scrub on warm path.** Mirror the `383c64a` scrub block from `navigate_with_init_solvers` (`page.rs:1088-1142`) into a shared helper, also call from `navigate_warm` (`page.rs:1426`); add pool-path regression test. Makes the 5/5 trustworthy across nav paths. | x-com durability (prod hole) | 0.5 d | high | [P] | `page.rs:1088-1142,1426` |
| **MASK-2** | **Event-subclass name+toString.** Explicit `_maskFunction` loop for the 23 Event subclasses (`event_bootstrap.js:512`) + add names to Layer B `_toMask`. Fixes `MouseEvent.name==='Event'`. Canonical Kasada `sdt` leak. | 0 alone; Kasada×3 | 0.5 d | high | [P] | `event_bootstrap.js:512` |
| **MASK-4** | **Real `NamedNodeMap` for `Element.attributes`.** Replace `Proxy([])` (`dom_bootstrap.js:877`) with a real class backed by attribute ops. Fixes `toString.call(el.attributes)` `[object Array]`→`[object NamedNodeMap]`. Akamai BMP + CreepJS tell. | 0 alone; Akamai/CreepJS | 1 d | high | [P] | `dom_bootstrap.js:877` |
| **D1** | **Function.toString mask sweep (16_AUDIT §5).** Add Event ctors, Headers/Request/Response, XHR, WebGL methods, Worker.postMessage, Observers, Streams, History, Storage to `dom_bootstrap.js:3032` sweep + golden `native_code_mask_audit` test. | 0 guaranteed; Kasada/CreepJS tail | 3–5 d | high | [P] | `dom_bootstrap.js:3032` |
| **CS-1** | **rebrowser-bot-detector as standing offline gate.** Wire runtimeEnableLeak/dummyFn/exposeFunctionLeak/sourceUrlLeak/mainWorldExecution into the in-VM probe harness. Locks the structural CDP-immunity lead vs deno_core/V8 drift. | 0 (guard, protects +15 lead) | 0.5–1 d | high | [G] | chrome_compat harness |
| **AWS-J/seccpt-oracle** | **Fork `awswaf_probe.rs`→`seccpt_probe.rs`.** Load captured homedepot 428, pre-inject instrumentation Proxy, `run_until_idle(30s)`, dump access trace/cookies/worker activity, decode base64 `challenge` provider field (crypto/behavioral/adaptive). | 0 direct; validates M-3, classifies homedepot days | 1–2 d | high | [P] | `crates/browser/examples/awswaf_probe.rs` |
| **DD-1** | **etsy live-nav trace experiment.** Use `[datadome-trace]` hooks (`page.rs:3149`); log materialized-iframe POST + `Set-Cookie: datadome=`. Disambiguate iframe-never-ran (drain) vs ran-but-fresh-challenge (fp/behavior). | 0; gates DD-2/3/4 | 1 d | high | [P] | `page.rs:3149` |
| **ETSY-1** | **Propagate child-iframe Set-Cookie to parent jar.** `ChildIframe::from_url` (`iframe.rs:173`) mints a fresh client (`runtime.rs:84-90`); share parent HttpClient/jar or merge child-jar deltas for the registrable domain so `is_datadome_solved`(`page.rs:221`) + cookie-diff retry (`:2397`) see the clearance cookie. | etsy/tripadvisor precondition; CF Turnstile | 2–3 d | high | [P] | `iframe.rs:173`; `runtime.rs:84-90`; `page.rs:221,2397` |
| **R1-douyin** | **acrawler offline oracle trace.** Point awswaf_probe-style oracle at douyin gate HTML; determine if `byted_acrawler.sign("",nonce)` THROWS (caught `page.rs:3406`→undefined sig) or returns server-rejected value; trace navigator/crypto/AudioContext/toString access. | 0; unblocks R2-R4 | 1 d | high | [P] | `page.rs:3406` |
| **G1** | **WebGPU adapter → null.** `requestAdapter()` resolves null (headless Chrome) instead of contradictory deviceless adapter w/ non-spec `name`, empty features, `isFallbackAdapter:false`, rejecting `requestDevice()`. | 0; CreepJS lie removal | 1 d | high | [P] | `window_bootstrap.js:6038-6056` |
| **AUDIO-2** | **Reconcile Apple-Silicon sampleRate.** External consensus macOS=44100 not 48000 (unsourced). Capture real Chrome 148; revert macOS presets if confirmed; wire `BiquadFilterNode._sampleRate` from profile. | 0; OS cross-signal | 2–3 h | high | [P] | `profile.rs:122-129`; `presets.rs:178-181`; `canvas_bootstrap.js:746` |
| **F1-duolingo** | **Instantiate script-created cross-origin iframes as executing realms.** Extend `iframe::find_iframes` + `page.rs:613-668` beyond srcdoc; wire to parent via postMessage. recaptcha anchor/bframe (and its Worker) run here. FP-E1 root. | duolingo + etsy/tripadvisor + some CF | 2–3 wk | medium | [P] | `page.rs:613-668` |
| **F2-duolingo** | **Real transferable MessagePort across Worker boundary.** Accept MessagePort in transfer lists (`window_bootstrap.js:1983-1994`, `worker_bootstrap.js:191-199`, postMessage `:4218`), proxy over mpsc, populate inbound `MessageEvent.ports`. Fix 8 only wired page↔page. | duolingo (once F1 lands) | 1 wk | medium | [P] | `window_bootstrap.js:1983-1994`; `worker_bootstrap.js:191-199` |
| **FIX-A-mouse** | **Route live mouse cycle through Sigma-Lambda.** `humanize.js:309-326` linear `_lerp` → `op_behavior_mouse_trajectory` (Rust generator already used by pre-pop). Fixes straightness 1.0→~0.4, tremor, count. | 0 confirmed; bestbuy/Kasada holistic | 1–2 d | high | [P] | `humanize.js:309-326` |
| **K-1 (K2-DIFF)** | **In-VM plaintext /tl sensor dump + field-diff.** Intercept XHR/fetch pre-XOR, capture live BO `/tl` for hyatt+canadagoose, decrypt via `omgtopkek`, diff vs captured real-Chrome ref. Converts the open tail into a finite bug list; decides public-vs-vendor. | 0 direct; bounds Kasada reachability | 3–5 d | high | [P] | `dom_ext.rs:1137` (child realm); ab_harness/tl |
| **G2** | **WebGL2-only methods.** Add ~60 stubs (createVertexArray/texImage3D/drawArraysInstanced/createQuery/fenceSync…) on `WebGL2RenderingContext.prototype` BEFORE `_maskAllProtoFns` sweep. Today empty subclass. | 0; CreepJS+AWS method-enum | 1–2 d | medium | [P] | `canvas_bootstrap.js:661` |
| **DD-2** | **Live-nav iframe drain parity.** Give `rematerialize_iframes` children (geo.captcha-delivery.com) full `run_until_idle` budget on the LIVE path (mirrors M-1). | etsy+tripadvisor (with ETSY-1) | 2–4 d | medium | [P] | `page.rs` rematerialize_iframes |
| **T2** | **IntersectionObserver real layout + re-fire.** Route isIntersecting/ratio/rect through layout op; re-fire on scroll/mutation; report false/0 off-screen instead of always true/1.0. | booking/douyin/SPA cluster | 2–3 d | medium | [P] | `window_bootstrap.js:3521-3547` |
| **D4-stack** | **Error.prepareStackTrace filter.** Drop `<anonymous>` + BO-internal frames; present Chrome-shaped frames for page code. CreepJS L8 + Kasada `esd` + Castle. No scrub today. | 0; CreepJS+Kasada | 1–2 d | medium | [P] | (no existing scrub) |
| **R2-douyin** | **Native-builtin integrity for acrawler.** From R1 trace, fix every BO builtin whose toString≠`[native code]` or descriptor shape diverges; extend `_maskAsNative`. Canonical Chromium-mimic-vs-Firefox discriminator. | douyin (1) | 2–4 d | medium | [P] | per R1 trace |
| **NAV-3** | **screen.orientation from w/h.** `window_bootstrap.js:1433` frozen landscape-primary → mobile presets report landscape on portrait. Derive from width vs height; nudge `screenY` off 0. | mobile presets; CreepJS | 0.5 d | high | [P] | `window_bootstrap.js:1433,1529` |
| **MASK-3** | **Mask WebGL ctx constructor objects.** Add WebGL/WebGL2RenderingContext to `_sfcNames` (`cleanup_bootstrap.js:463`); methods already masked. | 0 alone; AWS+CreepJS | 1 h | high | [P] | `cleanup_bootstrap.js:463` |
| **CANVAS-noise** | **Rewrite `to_data_url_with_jitter` to v150 model.** ±1 on first non-zero channel deterministic from seed; skip zero channels (preserves clearRect). Or A/B disable per FIX-G (`BROWSER_OXIDE_DISABLE_CANVAS_NOISE=1`); v150 defaults OFF. | 0–3 (cross-vendor) | 1 d | medium | [P] | `canvas2d.rs:1110-1131,1093-1144`; `webgl_render.rs:407-445` |
| **AUDIO-1** | **DynamicsCompressor -50 dB math.** Re-derive makeup-gain/static-curve (`audio.rs:385-505`) to match Chrome at -24 AND -50 dB; delete empirical 0.6+0.0739 patch; add -50 dB golden. CreepJS + Kasada ips.js send -50; BO 16 % off. | 0; Kasada coherence | ~1 wk | medium | [P] | `audio.rs:385-505` |
| **N3-firefox** | **Real Firefox 135 TLS+H2 class.** Build in `tls.rs`+`h2_client.rs`. BO leaks Firefox UA over Chrome TLS/H2 (JA4-vs-UA catch). | 0 corpus; Firefox-class parity | 1–2 wk | medium | [P] | `tls.rs`; `h2_client.rs` |
| **HD-vendor** | **sec-cpt v2/v3 sensor encoder (private).** Port pre-strip sensor_data + behavioral/adaptive flow into `vendor_solvers`; register via `Page::with_solvers`. Public seam exists. | homedepot behavioral/adaptive days (~40 %) | 3–5 d | medium | [V] | `vendor_solvers`; `challenge.rs:55` |
| **DD-vendor** | **DataDome interstitial daily-key solver (private).** 6-char daily key + canvas/audio/behavioral payload + encrypted POST to geo.captcha-delivery.com/interstitial/, via ChallengeSolver. | etsy+tripadvisor (after ETSY-1) | 1–2 wk + maint | medium | [V] | `challenge.rs:55`; `page.rs:2235` |
| **TAIL-PIN** | **CI regression pin.** Diagnostic exclusion == `{areyouheadless}`, `production_n==125`; snapshot thin-shell sizes (duolingo ~13.3k, bestbuy ~7.9k, spotify ~9.6k, booking ~8k). | 0; protects 107 passes | 0.5 d | high | [G] | classify.rs |
| **DUOLINGO-ASSERT** | **Token-gated PASS assertion.** A duolingo PASS must coincide with a solved reCAPTCHA worker token, not a >15 KB shell (sits 1.7 KB under gate). Prevents M-1 manufacturing a measurement-artifact win. | 1 verdict integrity | 0.5 d | medium | [G] | classify gate |
| **INVERSE-CHL** | **Guard 30 KB *-CHL gate vs growing bodies.** Extend any-size origin-token pattern to AWS (gokuProps/awsWafCookieDomainList) so a >30 KB unsolved AWS shell can't false-PASS during M-1 refactor. | AWS verdict integrity (7) | 1 d | medium | [G] | classify.rs |

---

## 3. PHASED EXECUTION PLAN

### PHASE 1 — OUTPERFORM v150 (close the -3, then go positive)

The -3 deficit is **entirely Stratum A** (AWS cluster + duolingo + booking), all the same async-drain class. v150 fails worse on wildberries, so progress there is a free outright win.

**P1.1 — Instrument & gate (de-risk before building).** seccpt-oracle fork, `BROWSER_OXIDE_EVENT_LOOP_PROFILE` instrumentation on the AWS path (does `op_worker_spawn` fire live? does `checkForceRefresh().then` resolve?), INVERSE-CHL + DUOLINGO-ASSERT + TAIL-PIN guards FIRST so a measurement artifact can't masquerade as a flip. *(M-1 instrumentation, AWS-J, the 3 guards.)*

**P1.2 — The drain lever + AWS arming.** M-1 (live-nav async drain), M-2 (AWS predicate + poll arming — confirmed entirely missing), M-3 (keep long timers refed), M-4 (host budget arm), F3 (worker-thread fetch identity). **Expected: imdb + amazon-in flip; amazon-ca/com/fr/jp/com-au reliability up; booking flip; wildberries grows past v150's THIN-39.** This alone should erase the -3 and likely add wildberries as a +1.

**P1.3 — duolingo (the hard one in Stratum A).** Needs F1 (script-created cross-origin iframe realms — 2–3 wk) + F2 (transferable MessagePort) + F3. This is the long pole of Phase 1; do P1.2 first and bank those flips while F1 lands.

**Exit criterion:** BO ≥ v150 on the 12-site delta, with AWS cluster + booking + wildberries flipped. Realistically **+5 to +7 sites**, taking BO from -3 to **ahead by +2 to +4**.

### PHASE 2 — PUSH PAST v150 (durability + the next tier)

- **homedepot durability** (M-3 already in P1; add seccpt-oracle classification + cookie-only fast path): 3/5 → 5/5 reliable. Locks a site BO beats v150 on.
- **x-com durability** (X1 warm-path scrub; then FIX-X2 per-Page cookie jar, 3–7 d, for the durable model). Closes a production hole.
- **etsy/tripadvisor** (DD-1 trace → ETSY-1 child-iframe cookie propagation → DD-2 iframe drain). Public-engine reaches v135's partial-completion; full flip needs DD-vendor (Stratum frontier).
- **douyin** (R1 oracle → R2 native-builtin integrity → R3 audio variance → R4 crypto.subtle). 1 site if the VM bails on an integrity probe.
- **Cross-cutting mask sweep** (D1, MASK-1/2/3/4, D2-W worker class, D4 stack scrub): compounds across Kasada/DataDome/CreepJS without any single guaranteed flip but raises the holistic-ML floor everywhere.

### PHASE 3 — FRONTIER ATTEMPTS (honest odds)

- **Kasada (canadagoose/hyatt/realtor)** — K-1/K2-DIFF first (bounds the problem); K-2 native-mask regression; then K-3 decode bot1225 identifier. **Odds: 1 of 3 if the drain + one named JS-fidelity bug is the dominant fail; v150 also fails all three, so any flip is a frontier win.** Honest: may be a holistic-ML tail with no single lever.
- **bestbuy** — BB-1 3-question probe FIRST (R-ticket never ran it; the Patchright-passes premise was a data misread — 1246k was homedepot's row). Only if probe Q2 shows fetched-but-never-posts does the drain (BB-2) flip it. Otherwise vendor+proxy. **Odds: low.**
- **DataDome daily-key (etsy/tripadvisor full)** — DD-vendor in vendor_solvers after ETSY-1. **+1–2 frontier sites BEYOND v150.** Ongoing daily-key maintenance cost.
- **homedepot behavioral/adaptive days** — HD-vendor encoder for the ~40 % of days the public engine structurally cannot reach. Beats v150 (no Akamai encoder either).

---

## 4. CAN WE HIT 126? — REALISTIC VERDICT

| Stratum | Sites | Ceiling | Verdict |
|---|---|---|---|
| A (async drain) | imdb, amazon-ca/com/com-au/fr/in/jp, booking, duolingo | **9/9 reachable** in public engine | M-1+M-2+M-3 flip the AWS 7 + booking with high confidence; duolingo reachable via F1/F2 (multi-week). **Realistic: 8–9.** |
| B (durability) | homedepot, x-com | **2/2 reachable** | M-3 + X1/FIX-X2. **Realistic: 2.** |
| C-public-winnable | wildberries | **1/1 reachable** | drain lever; outright win vs v150. **Realistic: 1.** |
| C-frontier-public | etsy, tripadvisor, douyin | partial | etsy/tripadvisor need ETSY-1 + DD-vendor for a true flip (public reaches partial-completion only); douyin needs the right integrity fix. **Realistic: 1–2.** |
| C-frontier-hard | canadagoose, hyatt, realtor, bestbuy | unknown until K2-DIFF/BB-1 | **Realistic: 0–2.** Likely holistic-ML tail / real-Chrome-trust / proxy. |
| D (diagnostic) | areyouheadless | **0 — unflippable by any engine incl. v150** | Correctly excluded. NOT a competitive gap. |

**Honest bottom line.** A literal **126/126 is NOT realistic** — `areyouheadless` is unflippable by design (so the true ceiling is 125), and the Kasada/bestbuy frontier (4 sites) plus the DataDome daily-key (vendor-only, maintenance treadmill) are genuinely uncertain. **What IS reachable in the public engine: ~12–14 of the 18 non-passing sites** (all of Stratum A+B+wildberries, plus etsy/tripadvisor partial and douyin if the integrity fix lands). That takes BO from ~107 to **~119–121 routed passes and decisively ahead of v150.** The residual hard tail (Kasada×3, bestbuy, full DataDome) is where vendor_solvers + honest "may be out of reach" applies. **Genuinely out of reach today:** any PAT/Private-Access-Token / device-attestation gate and pure-ML behavioral frontiers where no single field is load-bearing — none of which are confirmed in the current corpus, so the honest near-term target is **~121 / 125, ahead of v150 by a clear margin**, not 126.

---

## 5. CROSS-CUTTING WINS (compound across many sites)

1. **M-1 live-nav async drain** — the spine. Unlocks AWS×7 + booking + wildberries + (with realms) duolingo + reinforces homedepot/Kasada/etsy/bestbuy. Implement once, harvest 9+ sites. The single highest-ROI change in the entire program.
2. **Function.toString + accessor mask sweep (D1, MASK-1/2/3/4)** — one sweep cleans CreepJS L1, Kasada `sdt`/`sfc`/`sdt`, DataDome fetch-trio, Akamai attribute-audit. ~15 accessors + 23 Event subclasses + NamedNodeMap + WebGL ctor in a few days. No single flip; raises the ML floor under the whole frontier tier.
3. **Worker↔window coherence (D2-W, F3, NAV-1, D3 hashes)** — Camoufox's entire cross-process-storage patch exists for this. Removes worker-vs-window lies that fire on every CreepJS/DataDome/Kasada worker probe; F3's Linux-UA leak is a confirmed hard tell.
4. **Coherence/clamp fixes (NAV-1 deviceMemory≤8, NAV-2 vendor, NAV-3 orientation, AUDIO-2 sampleRate, G1 WebGPU)** — each removes a deterministic cross-signal contradiction that any ML scorer weights; cheap (hours each), no regression risk.
5. **Standing regression gates (CS-1 rebrowser, TAIL-PIN, INVERSE-CHL, K-2/K-4 native-mask, T4 timeOrigin)** — protect the measured +15-pass CDP-immunity lead and the honest 125-denominator against deno_core/V8 drift and measurement artifacts. Zero flips, high process ROI.
6. **MessageChannel/MessagePort + crypto.subtle completeness (F2, W-3)** — transferable ports + HMAC/derive/AES round out the PoW-worker substrate that AWS/reCAPTCHA/future challenges depend on.

> **Process note:** several prior "highest-leverage" items are already DONE — MessageChannel paired-routing (`f3ea599`), worker secure-context crypto.subtle (`5216336`), blob: protocol (`worker.rs:132-141`), WebGL1/2 split (FIX-D2). Docs `17_WEB_API_PARITY_MATRIX §2.5`, `41_POW §4.3/§6` still call these NO-OP stubs — **correct them and close `vNext/10_URL-polyfill-blob` to stop re-implementing solved work.** Likewise **reclassify booking SPA→AWS-WAF** in `02_GAP_ANALYSIS`/`05_SPA_HYDRATION_CLUSTER`/`FAILED_SITES_ANALYSIS` and retire `R-SPA-BOOKING-FETCH-CHAIN`.
