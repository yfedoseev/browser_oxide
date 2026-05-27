# R-FP-AUDIT-2026Q3 — Plan and methodology

**Goal:** close the routed-best-of-4 median gap from 107/126 (current BO HEAD) to ≥115 (Camoufox v150 baseline) by enumerating, diffing, and shipping fixes for every fingerprint surface where BO leaks engine identity, returns a wrong per-profile value, or fails cross-API correlation.

**Scope marker.** Engine-side stealth surface only. Per-vendor token computation (AWS WAF WASM PoW, DataDome WASM daily-key, Kasada `/tl` POST, Akamai sensor_data) is `vendor_solvers` per [`CLAUDE.md`](../../../CLAUDE.md) and [`SCOPE.md`](../../../SCOPE.md). Engine work this audit covers is what *enables* those vendors to issue tokens — fingerprint surface, JS API parity, cross-API consistency, and `[native code]` masking integrity.

**Standing context.** Read these in order before opening a per-surface file in this directory:

1. [`../16_STEALTH_FINGERPRINT_AUDIT.md`](../16_STEALTH_FINGERPRINT_AUDIT.md) — exhaustive table of every JS API surface BO exposes today, masking status, vendor-probe matrix. The "what we have" baseline.
2. [`../FAILED_SITES_ANALYSIS.md`](../FAILED_SITES_ANALYSIS.md) — per-site root-cause map.  Stratum A (Camoufox v150 passes; engine-addressable) is the focus of this audit.
3. [`../HANDOFF_v0.2.0_CLOSE_V150_GAP.md`](../HANDOFF_v0.2.0_CLOSE_V150_GAP.md) §1.1 — the v150 deltas: 8 sites v150 gained over v135, 11-site target list, file pointers.
4. [`../17_WEB_API_PARITY_MATRIX.md`](../17_WEB_API_PARITY_MATRIX.md) — implemented-vs-missing JS API matrix.
5. [`../28_AWS_WAF_EXTENDED.md`](../28_AWS_WAF_EXTENDED.md) + [`../06_AWS_WAF_SOLVER.md`](../06_AWS_WAF_SOLVER.md) — AWS WAF challenge.js probe inventory.

**Camoufox v150 reference.** Beta release 2026-05-11. Source: `https://github.com/daijro/camoufox/tree/v150.0.2-beta.25`, key directories `additions/` (Firefox source patches), `assets/` (fingerprint catalogue), `pythonlib/` (CLI). MPL-2.0 — read for "what to look at", do not copy-paste into BO (MIT/Apache).

## Methodology

### Step 1 — Establish BO's current baseline

The audit script proposed in `16_STEALTH_FINGERPRINT_AUDIT.md §5.2` (`native_code_mask_audit`) gives us the `[native code]` masking baseline. Step 1.a adds a parallel script `fingerprint_value_audit` that captures every fingerprint *value* (not just whether the function is masked) — UA, navigator.* getters, screen.*, WebGL parameters, AudioContext sampleRate, MediaDevices output. Output: golden JSON committed at `crates/browser/tests/fixtures/fingerprint_baseline_chrome_148_macos.json` (per profile).

### Step 2 — Establish Camoufox v150's surface

Same script run against Camoufox v150 via the Python harness. Output: `fingerprint_baseline_camoufox_v150.json` (per OS-profile v150 supports). Run programmatically:

```python
from camoufox.async_api import AsyncCamoufox
async with AsyncCamoufox(headless=True, os='macos') as b:
    page = await b.new_page()
    await page.goto('about:blank')
    fp = await page.evaluate(open('audit_probe.js').read())
    json.dump(fp, open('fingerprint_baseline_camoufox_v150.json','w'))
```

The probe script is the same JS executed inside both engines so the values are directly comparable.

### Step 3 — Diff

For each surface (navigator.userAgent, screen.width, …), emit a 4-column row:
`| surface | BO chrome_148_macos | Camoufox v150 macos | observed diff |`. File: `06_*.md`-style per-surface deep-dive with the diff + remediation.

### Step 4 — Prioritize

Three criteria:
- **Yield:** how many of the 11 target sites does fixing this surface plausibly unblock?
- **Effort:** stealth-profile-only (1-2 hours) vs engine code (days)?
- **Risk:** does the fix have regression surface on currently-passing sites?

Output: ordered fix list in `15_FIX_PRIORITY_RANKED.md` with the top-N as actionable v0.2.0 PRs.

### Step 5 — Land fixes

Each fix:
1. Open a per-fix doc (template at `EXECUTION_PLAN.md` in the parent dir).
2. Add a test in `crates/browser/tests/chrome_compat.rs` that goldens the new fingerprint value.
3. Implement.
4. L1-L4 gate green (build / clippy / fmt / workspace tests).
5. L5 single-profile sweep on target site(s) — confirm flip.
6. Commit. Record outcome in `16_DECISION_LOG.md`.

After every 3 fixes, run the full 3-run × 4-profile gate to confirm the routed median is climbing toward 115.

## Directory layout

```
audit/
  00_PLAN.md                       — this file
  01_BO_BASELINE.md                — BO's current fingerprint surface (extends 16_*)
  02_CAMOUFOX_V150_OVERVIEW.md     — v150 architecture, what changed v135→v150
  03_HARDWARE_SPOOFING_DIFF.md     — navigator.hardwareConcurrency / deviceMemory / screen / WebGL VENDOR-RENDERER (THE headline cluster)
  04_NAVIGATOR_DIFF.md             — every navigator.* getter
  05_SCREEN_WINDOW_DIFF.md         — screen + window.outer*/inner*/devicePixelRatio
  06_WEBGL_DIFF.md                 — WebGL params + extensions + getParameter source masking (AWS WAF principal target)
  07_AUDIO_DIFF.md                 — AudioContext.sampleRate + DynamicsCompressor fingerprint
  08_CANVAS2D_DIFF.md              — CanvasRenderingContext2D fingerprint (CreepJS/BotD)
  09_PERFORMANCE_TIMING_DIFF.md    — performance.now granularity + drift
  10_FONTS_DIFF.md                 — font enumeration + measureText + FontFace API
  11_MEDIA_DEVICES_DIFF.md         — enumerateDevices output shape
  12_CLIENT_HINTS_DIFF.md          — sec-ch-ua-* headers + navigator.userAgentData (DataDome ClientHints cross-check)
  13_CROSS_CORRELATION.md          — UA ↔ deviceMemory ↔ hardwareConcurrency ↔ screen ↔ WebGL VENDOR consistency
  14_AWS_WAF_CORRELATION.md        — which surfaces challenge.js reads + which we leak
  15_FIX_PRIORITY_RANKED.md        — yield × effort ranked v0.2.0 PR list
  16_DECISION_LOG.md               — running log; what shipped, what was dropped, why
```

Each per-surface file follows the same template:

```
# NN — <surface>

**Sites in scope:** <subset of the 11 target sites>
**Vendor probes reading this:** <AWS WAF / DataDome / etc.>

## 1. Current BO behaviour
   <values, file:line citations, masking status>

## 2. Real Chrome 148 behaviour
   <from captured browser fingerprint or canonical reference>

## 3. Camoufox v150 behaviour
   <from probe script run against v150>

## 4. Diff
   <table>

## 5. Fix
   <file:line, test name, validation, effort, risk>
```

## Acceptance criteria

Same as `16_STEALTH_FINGERPRINT_AUDIT.md §7` plus:

- [ ] `01_BO_BASELINE.md` enumerates every fingerprint surface in BO (programmatically generated).
- [ ] `02_CAMOUFOX_V150_OVERVIEW.md` lists v146→v150's hardware-spoofing patch lineage with file:line into Camoufox's `additions/`.
- [ ] Per-surface diff files (03-13) exist for every surface where BO ≠ v150.
- [ ] `15_FIX_PRIORITY_RANKED.md` has a yield × effort ranked PR list with at least the top-5 actionable items.
- [ ] At least the top-3 fixes from §15 are landed and the routed best-of-4 median ≥ 110 (intermediate gate).
- [ ] All fixes ship behind the existing stealth-profile mechanism — `crates/stealth/profiles/*.yaml` for value-class fixes, `crates/js_runtime/src/js/*_bootstrap.js` for surface-class fixes. No new `unsafe`, no new dependencies, no license-incompatible code.

## Out of scope for v0.2.0 specifically

- WASM PoW solvers (AWS WAF, DataDome, Kasada). Engine-side fingerprint fixes must enable these to *issue* tokens; the *solving* is `vendor_solvers`.
- Behavioural-signal work (mouse/keyboard humanization). Tracked separately.
- Kasada `sfc`/`sdt` deep dive beyond what the masking sweep covers — that's R-KASADA-FRONTIER, deferred.

## Status legend

- ⬜ not started
- 🔵 in progress
- ✅ done
- ❌ dropped (with reason in `16_DECISION_LOG.md`)
- ⏸️ paused (with reason)

## Current status

| § | File | Status |
|---|------|--------|
| 00 | 00_PLAN.md | ✅ |
| 01 | 01_BO_BASELINE.md | ✅ |
| 02 | 02_CAMOUFOX_V150_OVERVIEW.md | ✅ |
| 03 | 03_HARDWARE_SPOOFING_DIFF.md | ✅ |
| 04 | 04_NAVIGATOR_DIFF.md | ⬜ (covered in 01 + 12) |
| 05 | 05_SCREEN_WINDOW_DIFF.md | ⬜ (covered in 03) |
| 06 | 06_WEBGL_DIFF.md | ⬜ next (FIX-D) |
| 07 | 07_AUDIO_DIFF.md | ⬜ (FIX-C addressed; full doc pending) |
| 08 | 08_CANVAS2D_DIFF.md | ⏸️ research (canvas noise decision) |
| 09 | 09_PERFORMANCE_TIMING_DIFF.md | ⬜ |
| 10 | 10_FONTS_DIFF.md | ⬜ |
| 11 | 11_MEDIA_DEVICES_DIFF.md | ⬜ |
| 12 | 12_CLIENT_HINTS_DIFF.md | ⬜ (FIX-A + FIX-F addressed; full doc pending) |
| 13 | 13_CROSS_CORRELATION.md | ⬜ (initial findings in 03) |
| 14 | 14_AWS_WAF_CORRELATION.md | ⬜ |
| 15 | 15_FIX_PRIORITY_RANKED.md | ✅ |
| 16 | 16_DECISION_LOG.md | ✅ (live; FIX-A/FIX-C/FIX-F entries + validation outcome) |

## Shipped this audit cycle

- **FIX-A** (commit `960b55f`): Sec-CH-UA-Arch/Bitness/Wow64 now read from profile fields instead of being derived from `platform` (which had the MacIntel ↔ arm/x86 ambiguity bug). +3 net tests.
- **FIX-C** (commit `93c8ed4`): AudioContext.sampleRate now profile-pinned (`audio_sample_rate` field, 48000 on Apple Silicon presets); baseLatency / outputLatency derive deterministically from `audio_seed` bits. Stable across page loads in the same SharedSession.
- **FIX-F** (commit `8d8c067`): Sec-CH-Device-Memory now quantizes RAM values to the W3 spec set `{0.25, 0.5, 1, 2, 4, 8}` rather than emitting any clamped float. +2 net tests including the helper unit.
- **Validation:** single-run 3-site sweep post-fixes (`amazon-com`, `imdb`, `amazon-de`): **amazon-de flipped from blocked → 855KB L3-RENDERED** ✅; amazon-com + imdb still at AWS WAF stub bodies (2011 / 1995 bytes). Single-trial; could be WAF state noise per `docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md`.
