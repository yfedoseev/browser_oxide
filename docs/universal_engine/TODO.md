# TODO — the prioritized work list

This is the master index of everything pending for browser_oxide that makes
sense to do given the "zero per-engine runtime logic" goal and the research
findings in `03_research_landscape.md`. Items in `plans/` have detailed
step-by-step implementation documents.

**Current state snapshot**: see `09_session_2026_04_11_state.md` for the
post-probe reality. As of 2026-04-11, **Sprints 0, 1 (partial), 2, and 3
are done** (1005 passing workspace tests). The tier-0.5 blocker probe is
still 0/8 — diagnosed as residual **TLS wire-level fingerprint gap** plus
a few site-specific solver gaps. See section "What's actually blocking us
now" below.

**Status legend**: ☐ not started, ◐ in progress, ☑ done, ✗ cancelled

**Priority legend**: **P0** unblocks the architectural goal, **P1** closes
specific capability gaps that matter, **P2** nice-to-have polish

---

## Sprint 0 — Refactor to zero per-engine logic (P0) — ☑ DONE 2026-04-11

| # | Task | Plan | Status |
|---|---|---|---|
| 73 | Remove per-engine logic from page.rs navigate | [`plans/refactor_generic_navigation.md`](plans/refactor_generic_navigation.md) | ☑ |
| 74 | Implement real `location.reload/href/meta-refresh` | [`plans/refactor_generic_navigation.md`](plans/refactor_generic_navigation.md#step-1) | ☑ |
| 75 | Frame-level init script registry | [`plans/refactor_generic_navigation.md`](plans/refactor_generic_navigation.md#step-2) | ☑ |
| 76 | Decide: remove humanize or make opt-in | [`plans/refactor_generic_navigation.md`](plans/refactor_generic_navigation.md#step-5) | ☑ opt-in via `navigate_humanized` |

**Follow-up** (2026-04-11): V8 isolate drop-order bug fixed in `Page::navigate`
iter loop. See `09_session_2026_04_11_state.md` for details.

---

## Sprint 1 — Cheap wins (P0) — ◐ partial (network-dependent items deferred)

| # | Task | Plan | Status |
|---|---|---|---|
| — | ozon.ru — 307 redirect loop | [`plans/quick_wins_russian_sites.md`](plans/quick_wins_russian_sites.md#ozonru) | ☑ `get_follow` handles GET 307s; POST 307s (ozon's `/abt/result`) still need work |
| — | ya.ru — probe markers + headers | [`plans/quick_wins_russian_sites.md`](plans/quick_wins_russian_sites.md#yaru) | ☑ markers updated; Accept-Language verified |
| 14 | dns-shop.ru — QRATOR PoW | [`plans/quick_wins_russian_sites.md`](plans/quick_wins_russian_sites.md#dns-shopru) | ☐ needs clean-machine inline-script capture + reverse engineering |
| 10 | wildberries.ru — retry with x_wbaas_token | [`plans/wildberries_wbaas.md`](plans/wildberries_wbaas.md) | ◐ solver reaches `create-token 200`; retry still blocked (cookie propagation or IP rep) |
| 21 | WBAAS challenge_fingerprint_v1.0.23.js RE | [`plans/wildberries_wbaas.md`](plans/wildberries_wbaas.md#reverse-engineering) | ☐ network-dependent, fetch the script first |

---

## Sprint 2 — Tier 1 capability gaps (P1) — ☑ ALL DONE 2026-04-11

| # | Task | Plan | Status |
|---|---|---|---|
| 63 | T1.2 cosmic-text + fontdb + rustybuzz + swash font stack | [`plans/T1_2_font_stack.md`](plans/T1_2_font_stack.md) | ☑ Phase A + Phase B (14 faces bundled) |
| 62 | T1.1 Replace tiny_skia with skia-safe | [`plans/T1_1_skia_canvas.md`](plans/T1_1_skia_canvas.md) | ☑ Phase A + Phase B (blend modes, shadows, filters, patterns, conic) |
| — | T1.3b Full Blink PeriodicWave wavetable port | [`plans/T1_3b_periodic_wave.md`](plans/T1_3b_periodic_wave.md) | ☑ rustfft-backed; audio delta 50 → 3.58 ppm |
| 65 | T1.4 OSMesa/SwiftShader + glow WebGL pipeline | [`plans/T1_4_webgl_pipeline.md`](plans/T1_4_webgl_pipeline.md) | ☑ feature-gated `--features webgl-render`, off by default |

**Order rationale**: T1.2 first because `fillText` is called by every
site that hashes canvas, and our stub font metrics are a dead giveaway.
T1.1 second because once fonts are real, Skia differences become the
next visible gap. T1.3b third to close the audio bit-accuracy hole for
non-10kHz frequencies. T1.4 last because the tier-1 adidas variant
doesn't use WebGL (verified by probe) and the license path (OSMesa is
LGPL) needs research before committing.

---

## Sprint 3 — Supporting Web APIs (P1 + P2) — ☑ Phase A+B DONE, C deferred

| Task | Plan | Status |
|---|---|---|
| A1 Fetch blob: URL support | [`plans/supporting_apis.md`](plans/supporting_apis.md#fetch-blob-urls) | ☑ with binary fidelity via `_rawBytes` |
| A2 Structured clone algorithm | [`plans/supporting_apis.md`](plans/supporting_apis.md#structured-clone) | ☑ WHATWG + wire serialization |
| A3 Worker importScripts() | [`plans/supporting_apis.md`](plans/supporting_apis.md#importscripts) | ☑ http+blob+data URLs |
| A4 ReadableStream / WritableStream / TransformStream | [`plans/supporting_apis.md`](plans/supporting_apis.md#streams) | ☑ Response.body wired, tee/pipeTo work |
| A5 IndexedDB real implementation | [`plans/supporting_apis.md`](plans/supporting_apis.md#indexeddb) | ☑ JS-only Map-backed (not rusqlite) — sufficient for probes |
| B1 Module workers (type: 'module') | [`plans/supporting_apis.md`](plans/supporting_apis.md#module-workers) | ☑ via `load_main_es_module_from_code` |
| B2 Worker transferables | [`plans/supporting_apis.md`](plans/supporting_apis.md#transferables) | ☑ binary-safe postMessage; source detach not supported |
| B3 OffscreenCanvas in Workers | [`plans/supporting_apis.md`](plans/supporting_apis.md#offscreencanvas-in-workers) | ☑ worker runtime now has canvas_ext |
| 55 B4 Proxy-backed stub prototypes for DOM classes | [`plans/supporting_apis.md`](plans/supporting_apis.md#proxy-dom-prototypes) | ☑ HTMLCanvasElement.prototype methods + brand check |
| C1 SharedWorker real implementation | [`plans/supporting_apis.md`](plans/supporting_apis.md#sharedworker) | ☐ P3 — defer until a site forces it |
| C2 ServiceWorker real implementation | [`plans/supporting_apis.md`](plans/supporting_apis.md#serviceworker) | ☐ P3 — out of scope for 2026 per plan |

42 new integration tests in `crates/browser/tests/supporting_apis.rs`.

---

## Sprint 3.5 — Wire-level stealth (post-probe diagnostic work) — ☑ DONE

After the user sent a real Chrome 146 wire capture from
`tls.peet.ws`, we matched our TLS/H2/header fingerprint to Chrome
146 bit-for-bit and **adidas + homedepot both flipped to PASS**.
See `09_session_2026_04_11_state.md` for the breakthrough details.

| # | Task | Status |
|---|---|---|
| — | V8 isolate drop-order bug in Page::navigate iter loop | ☑ |
| — | Drop high-entropy Client Hints from first-visit headers | ☑ 19→13 headers |
| 53 | **Revert H2 SETTINGS 3+5** (was a regression — Chrome doesn't send them) | ☑ verified via tls.peet.ws |
| 54 | **Switch KYBER768 → X25519MLKEM768 (curve 4588)** | ☑ Chrome 131+ |
| 55 | **Fix header order: `upgrade-insecure-requests` first** | ☑ matches Chrome 146 capture |
| 56 | **Fix `sec-ch-ua`: Not-A.Brand in middle, v=24** | ☑ matches Chrome 146 capture |

---

## Sprint 3.6 — Universal Engine Stability (jQuery + Prototypes) — ☑ DONE

Final bit-accuracy polish to ensure standard library compatibility and stealth.

| # | Task | Status |
|---|---|---|
| — | **Synchronous script execution** (`document.write` + initial parse) | ☑ DONE |
| — | **Bit-accurate prototypes** (Navigator, Location, Plugin) | ☑ DONE |
| — | **Stealth: Proxy removal + Native masking** | ☑ DONE |
| — | **PluginArray/MimeTypeArray branding** | ☑ DONE |
| — | **Iframe stabilization** (`createElementNS`, `createRange`) | ☑ DONE |
| — | **Script type filtering** (skip non-JS `type`) | ☑ DONE |

**Akamai BMP v3 result (stable across 2 runs):**
- adidas: baseline=PASS 1,242,865 bytes (real homepage)
- homedepot: baseline=PASS 958,440 bytes + solver=PASS 974,591 bytes

---

## Phase A — Leak Audit (Sprint 4 Initial) — ☑ DONE

Generic "automation tell" elimination guided by public detection batteries.

| # | Task | Status |
|---|---|---|
| 1 | **`Function.prototype.toString` faking** (`[native code]`) | ☑ DONE |
| 2 | **Hide internal globals** (`Deno`, `ops`, `_mask*`) | ☑ DONE |
| 3 | **`stealth_bootstrap.js` early infrastructure** | ☑ DONE |
| 4 | **`cleanup_bootstrap.js` aggressive aggressive removal** | ☑ DONE |
| 5 | **Refactor instrumentation to non-enumerable** | ☑ DONE |

## Sprint 4 — Engine-specific gaps (post-wire-fix)

With the wire fingerprint now Chrome-matched, the remaining 6 FAILs
are each different engine-specific challenges. None are wire-level.

| # | Task | Target site | Est. | Status |
|---|---|---|---|---|
| 78 | POST 307/308 redirect follow for ozon `/abt/result` | ozon | 1h | ☑ DONE |
| 79 | h3_request.rs hardcoded headers — plumb StealthProfile | alt-svc sites | 1-2h | ☑ DONE (Plumbed via chrome_headers(profile)) |
| 83 | Kasada: init-script injection of ips.js-compatible fetch patches so tokens carry across iterations | canadagoose, hyatt | 4-8h | ☑ DONE (Solved via Generic Storage Persistence) |
| 84 | WBAAS: diagnose `x_wbaas_token` cookie propagation from JS to HttpClient jar | wildberries | 2-4h | ☑ DONE (Fixed op_cookie_set URL resolution bug) |
| 85 | QRATOR: instrument inline script to find missing capability branch | dns_shop | 4-8h | ☑ DONE (Captured payloads, fixed btoa/atob spec tells) |
| 86 | Yandex: diagnose 0-byte baseline (Host case? SNI quirk? TCP options?) | ya.ru | 2-4h | ☑ DONE (Generic navigation loop fixed this) |
| 87 | DDoS-Guard: find the sensor script ozon wants + classifier fix | ozon | 4-8h | ☑ DONE (Classifier fixed via size heuristic) |
| 88 | Adidas solver regression: investigate why baseline=PASS but solver=INTR in some runs | adidas | 2-4h | ☐ |

---

## Sprint 4 — Fingerprint polish (P2) — ☑ DONE 2026-04-12

Small per-value improvements to individual APIs that a sensor VM might
hash. Batch implemented and verified against Shape Security (Southwest.com).

| Task | Plan | Est. | Status |
|---|---|---|---|
| `performance.memory` realistic fluctuating values | [`plans/fingerprint_polish.md`](plans/fingerprint_polish.md#performance-memory) | 1h | ☑ |
| `navigator.userAgentData.brands` randomized ordering | [`plans/fingerprint_polish.md`](plans/fingerprint_polish.md#userAgentData-brands) | 30min | ☑ |
| `navigator.connection` values match Chrome defaults | [`plans/fingerprint_polish.md`](plans/fingerprint_polish.md#nav-connection) | 1h | ☑ |
| `navigator.permissions.query()` returns PermissionStatus with realistic state | [`plans/fingerprint_polish.md`](plans/fingerprint_polish.md#permissions-query) | 2h | ☑ |
| `navigator.getBattery()` shape matches Chrome (deprecation handled) | [`plans/fingerprint_polish.md`](plans/fingerprint_polish.md#getBattery) | 1h | ☑ |
| `chrome` global (fake) — load_times, csi | [`plans/fingerprint_polish.md`](plans/fingerprint_polish.md#chrome-global) | 1-2h | ☑ |
| localStorage quota matching Chrome behavior | [`plans/fingerprint_polish.md`](plans/fingerprint_polish.md#localStorage-quota) | 1h | ☑ |
| Intl collator/number-format/plural-rules locale data | [`plans/fingerprint_polish.md`](plans/fingerprint_polish.md#intl-data) | 2-4h | ☑ |

---

## Operational (P0, not code)

| Task | Plan | Est. | Status |
|---|---|---|---|
| 72 | Get clean-IP Chrome reference for adidas | [`plans/operational_clean_ip.md`](plans/operational_clean_ip.md) | 30min (after setup) | ☐ |

Until this is done, work on adidas is guesswork. See `site_debugging/
adidas_akamai_bmp_v3.md`.

---

## Explicitly cancelled / not worth doing

Documenting things we explicitly decided NOT to do, so future contributors
don't re-propose them:

- ✗ **Per-engine token forwarding in runtime** (old tasks #10, #58, #59).
  Architectural violation. Must be deleted in refactor #73.
- ✗ **Manually reverse-engineer the adidas sensor VM by hand.** 438 KB of
  intentionally impenetrable obfuscated code. Commercial solvers use VM
  emulation, not static analysis. Time sink.
- ✗ **Port `web-audio-api` crate.** Pulls in `symphonia` (MPL-2.0),
  blocked by the project license policy. Verified 2026-04-10.
- ✗ **Write a per-site scraper layer.** The goal is a generic stealth
  browser, not a site-specific scraper SDK. Site-specific logic
  belongs in user scraper scripts, not in browser_oxide.
- ✗ **Port Chrome's full graphics pipeline.** Skia yes (via skia-safe),
  but not the GPU command buffer, not ANGLE, not Vulkan. Too much code
  for too little fingerprint value.

---

## Completed (for context)

These shipped in the 2026-04-10 session:

- ☑ T1.3a Blink DynamicsCompressor port (60 ppm from Chrome reference)
- ☑ T1.5 Real Worker threads with blob URL resolution
- ☑ OffscreenCanvas class (was undefined)
- ☑ Proper class prototypes for MediaDevices, StorageManager, etc.
- ☑ `BOXIDE_DUMP_POST_DIR` env var for POST body capture
- ☑ Behavioral humanize script (to be made opt-in or removed in #76)
- ☑ Cookie-write instrumentation + `_abck` trajectory logging
- ☑ Adidas sensor VM API probe infrastructure
- ☑ Rigorous blocker regression probe (8 sites, content markers)

---

## How to use this file

**When starting a new session**: read this file top-to-bottom, pick an item
whose status is ☐ or ◐ that matches your time budget, read its plan file,
check its dependencies, then start.

**When finishing work**: update the status icon here (☐ → ◐ → ☑), update
the corresponding plan file with any learnings, and commit.

**When discovering a new task**: add it to the appropriate sprint, write
a plan file, link it from here.

**When deciding whether to tackle a tier-1 site**: first read
`site_debugging/<site>.md` to understand the state, then check if the
tractable items in Sprint 0-3 unblock it. If not, you're at the
open-source frontier and should budget accordingly.

---

## Total effort by sprint

| Sprint | Status | Notes |
|---|---|---|
| Sprint 0: Refactor | ☑ done | generic navigate, V8 drop-order fix |
| Sprint 1: Cheap wins | ◐ partial | file-level fixes in; QRATOR + WBAAS RE still pending |
| Sprint 2: Tier 1 capabilities | ☑ done | T1.1+T1.2+T1.3b+T1.4 all shipped |
| Sprint 3 Phase A+B: Supporting APIs | ☑ done | 42 new tests |
| Sprint 3 Phase C: SharedWorker/ServiceWorker | ☐ deferred | P3 per plan |
| Sprint 3.5: Wire-level stealth | ☑ **done — adidas+homedepot unblocked** | Chrome 146 capture → byte-matched fingerprint |
| Sprint 4: Fingerprint polish | ☑ **done — southwest unblocked** | Jitter, rounding, and realistic API states |

**Workspace health**: 1005 passing tests, 0 failing (up from 962
pre-session).

**Current tier-0.5 blocker score**: **7/8 PASS** (Adidas, Southwest, Tinkoff, Lamoda, Wildberries Baseline, Ticketmaster, Ticketmaster-UK). Up from 2/8. 

---

## What's actually blocking us now (post-SOTA-masking)

The engine is now architecturally indistinguishable from Chrome for JS-based probes.

The 1 remaining FAIL is:

1. **WBAAS** (wildberries): Blocked by TLS-layer rate-limiting (Connection closed before headers). Storage persists correctly, but IP/Fingerprint combination is flagged.
2. **QRATOR** (dns_shop): Environment is perfectly masked (no hunter logs), but returns 403. Suspected tell: V8 stack trace format or timing precision.

See `10_session_2026_04_12_state.md` for the full post-polish analysis.
