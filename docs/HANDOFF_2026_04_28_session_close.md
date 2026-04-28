# Handoff — browser_oxide stealth/anti-bot work
**Date**: 2026-04-28 (session-close — companion to `HANDOFF_2026_04_28.md`)
**Previous handoff**: `docs/HANDOFF_2026_04_28.md` (mid-session, before the live re-run)
**This session**: 11 commits, ~250 KB of insertions, 6 plan items closed, 2 second-layer bugs surfaced

This document is the end-of-session report. The mid-session handoff captured the plan and the post-Item-5 state; this one captures **what actually shipped, what passes/fails on live sites, and the new bugs that the day's work exposed**. Everything in this file is committed and pushed to `origin/main`.

---

## TL;DR

- **6 of 6 plan items shipped** (D, 1, 2, 3, 4, 5). Each independently verifiable.
- **2 live anti-bot sites flipped from FAIL to PASS** (ozon.ru, pixelscan.net), both went from `BLOCK 0 B` → `L3-RENDERED 97/103 KB`. Net distribution: **24 PASS / 5 CHL / 9 FAIL → 26 PASS / 5 CHL / 7 FAIL** for the page-render half.
- **1 site improved meaningfully** (zillow body 12 KB → 449 KB, 37× growth, much further into PerimeterX flow).
- **9 sites unchanged**; **0 regressions**.
- **2 sites still failing — but with NEW failure modes** that prove the V8 #60 fix worked. Both need follow-up investigation. Details below.

## What shipped (commit by commit)

| Commit | Item | Summary |
|---|---|---|
| `a9573f0` | D | chrome.app shape validation: 11 self-capture assertions + 7 parity tests against Chromium-source surface |
| `82526d0` | 1 | Synthetic V8 recursion regression suite (7 patterns) + 64 MB worker thread stacks |
| `40237ce` | 1 | `.cargo/config.toml` `RUST_MIN_STACK=67108864` — V8 #60 closed at 1 line of config |
| `a9274ee` | 2 | `strokeText` via `ttf-parser::OutlineBuilder` — real glyph contours, not aliased to `fillText` |
| `f492cbe` | 3 | `flate2/zlib-rs` + pinned `Paeth/Adaptive` filter strategy — deterministic PNG output |
| `45cdab7` | 4 | WebGL stub-output stability gate + OSMesa setup docs |
| `3286670` | 5 | WebAudio compressor gap diagnosed (kernel threshold-response bug, not oscillator scale) |
| `5a28edc` | — | Live chl_sites matrix re-run results |
| `1e8107c` | — | V8 heap raised to 4 GB + `_onNodeInserted` depth guard |

## Live-site matrix delta (page-render, from `chl_sites.rs`)

Compared against the 2026-04-28 baseline at `research_2026_test_results.md`:

| Site | Before | After | Status |
|---|---|---|---|
| ozon.ru | BLOCK 0 B | **L3-RENDERED 97 KB** | ✅ flipped |
| pixelscan.net | BLOCK 0 B | **L3-RENDERED 103 KB** | ✅ flipped |
| zillow | PerimeterX-CHL 12 KB | captcha-CHL **449 KB** | 🔼 37× progress |
| sannysoft | V8 SIGTRAP (#60) | **Rust stack overflow at 64 MB** | 🟡 V8 #60 fixed; new bug exposed |
| creepjs | V8 SIGTRAP (#60) | **V8 heap OOM at 1.8 GB** then **CPU hang at 4 GB** | 🟡 V8 #60 fixed; two new bugs exposed |
| 10 others | unchanged | unchanged | — |

Full delta in `research_2026_test_results_2026_04_29.md`.

---

## Two unresolved second-layer bugs

These were **previously misdiagnosed as the V8 #60 stack-overflow bug**. With Item 1 shipped, they revealed themselves as separate, deeper issues.

### A. creepjs — V8 heap OOM at 1.8 GB → CPU hang at 4 GB

**What we knew**: creepjs SIGTRAPs.
**What we know now**: SIGTRAP was V8's `__builtin_trap()` after exhausting the default ~1.5 GB old-space. Stack trace pointed to `Builtins_ArrayPrototypePush` deep inside `Builtins_InterpreterEntryTrampoline` — meaning JIT'd creepjs JS was filling an array unboundedly while hitting the heap ceiling.

**Mitigation shipped (`1e8107c`)**: V8 `CreateParams::heap_limits(256 MB initial, 4 GB max)` matches Chrome desktop renderer budget. Removes the OOM crash for legitimately-large fingerprint payloads.

**New behavior with the fix**: creepjs no longer crashes — but **spins at 100% CPU for 40+ minutes** without finishing or crashing. Killed two stuck processes during this session.

**What this means**: the OOM was masking an O(N²+) loop in creepjs's lie-detection / property-enumeration phase. Now bounded by time instead of memory. Likely cause: one of our shims returns a value that makes creepjs's property walk explode. Suspects (in order of likelihood):
1. **Mirror-realm constructors**: `_buildRemoteRealm` in `dom_bootstrap.js:1852` creates fresh constructors with prototypes that have parent-realm ancestors. If creepjs walks `getPrototypeOf` chains and hits an unexpected node, it might iterate forever in some other dimension.
2. **Cyclic property descriptor**: an own-property with a getter that returns `this` (or a value that itself enumerates infinitely).
3. **Array-like with infinite length**: an `arguments`-style object whose `length` getter returns `Infinity` or grows.

**Diagnosis path** (~1 day): instrument the V8 isolate to log allocation hot-spots, OR run creepjs in real Chrome with `--max-old-space-size=4096` and snapshot the heap every 30 s with `chrome://inspect` to see what data structure is growing. Comparing to our engine's heap dump pinpoints the divergent shim.

### B. sannysoft — Rust-side stack overflow at 64 MB

**What we knew**: sannysoft SIGABRTs at default thread stack.
**What we know now**: Even with `RUST_MIN_STACK=67108864` (64 MB), sannysoft still SIGABRTs. The Rust stack-overflow handler reports "thread has overflowed its stack" and aborts before any backtrace can print.

**Bisection performed this session** (debug code reverted):
| Setup | Result |
|---|---|
| Full HTML, scripts stripped | ✅ parses + builds runtime fine |
| Full HTML, only inline scripts kept (Yandex Metrika) | ✅ |
| Minimal HTML + 3 CDN libs (lodash/jquery/ua-parser) | ✅ |
| Minimal HTML + 3 CDN + script_30 (runBotDetection) | ✅ |
| Full body + 3 CDN + script_30 only | ✅ |
| Full HTML, doc.write scripts removed but everything else kept | ❌ stack overflow at script_30 |
| Full HTML | ❌ stack overflow at script_30 |

**The trigger**: full DOM size + the prior 11 inline scripts (mediaDevices probe, getBattery, fpCollect.generateFingerprint, utf8_encode/crc32, …) + script_30's `for (const documentKey in window['document']) { ... }` loop **combined**. Each of those ingredients alone is fine. The combination tickles a Rust-side recursion in op dispatch.

**Why no backtrace**: macOS's stack-overflow signal handler runs on the same overflowing stack, aborts before the Rust panic handler (and `RUST_BACKTRACE=full`) can install.

**Diagnosis path** (~1-2 days): add a `signal-hook`-based SIGSEGV handler that runs on a dedicated signal stack and uses `backtrace::trace` to dump the recursion. Or instrument the most-likely-recursing ops (`op_dom_get_attribute`, `op_dom_query_selector`, layout recompute) with depth counters that bail before stack exhaustion. Once the offending op is named, the fix is usually localized — check the recursion termination condition.

**Why this is NOT V8 #60**: V8 #60 was about V8's own stack guard not catching recursion before C-stack exhaustion. With 64 MB, V8 has enough headroom to throw RangeError on every JS recursion pattern (proven by `crates/browser/tests/v8_recursion.rs::*`). This bug is on the Rust side — one of our DOM/layout/whatever ops calls itself transitively without a base case.

---

## Files of interest for the next session

| File | Why |
|---|---|
| `crates/js_runtime/src/runtime.rs:100-110` | V8 heap_limits config (4 GB max). Try lowering if the slow loop suggests OOM is preferable. |
| `crates/js_runtime/src/js/dom_bootstrap.js:64-90` | `_onNodeInserted` depth guard (cap 64). Bumping this won't help sannysoft (different code path) but might help future doc.write cycles. |
| `crates/js_runtime/src/js/dom_bootstrap.js:1852` | Mirror-realm builder — likely candidate for creepjs investigation. |
| `crates/js_runtime/src/extensions/dom_ext.rs` | Suspect ops for sannysoft: anything that iterates DOM children. Add depth guard at hot ops. |
| `research_2026_test_results_2026_04_29.md` | Full live-site delta. |

## Recommended next-session order

1. **sannysoft signal-handler infrastructure** (~2 hours): add `signal-hook` dep, register SIGSEGV handler with `signal_hook::iterator::Signals::forever`, capture `backtrace::Backtrace::new()` to a known file, panic from main thread instead of OS abort. Once a backtrace is captured, the fix is typically a 5-line change.
2. **creepjs heap-snapshot diff** (~1 day): compile with `deno_core` heap-profiler enabled, run creepjs for 30 s, dump heap, compare to a real Chrome capture. Find which shim is producing the explosive structure.
3. Re-run full chl_sites matrix to update the production scoreboard.

## Out of scope (deliberately deferred)

- WebGL OSMesa default-enable (Item 4 documented this — needs CI infra changes, not engine work).
- WebAudio kernel rewrite for threshold-response bug (Item 5 diagnosed — needs ~1 wk of bisecting Blink's `DynamicsCompressorKernel.cpp` against ours).
- libpng-sys swap (Item 3 reframed: zlib-rs + pinned filter strategy gets us byte-determinism without the C dep, but full Chrome SHA parity needs the text-rasterization gap closed first).
- V8Thread proxy refactor (originally scoped at 2 days for Item 1 — not needed once `RUST_MIN_STACK` was found to work).

## Memory pointers

`/Users/yfedoseev/.claude/projects/-Users-yfedoseev-Projects-browser-oxide/memory/` will need updates for:
- creepjs heap OOM and the slow-loop second-layer bug
- sannysoft Rust-side stack overflow at 64 MB
- The fact that V8 #60 IS closed; the previous-handoff conflation of sannysoft+creepjs with V8 #60 was incorrect

These updates can be done incrementally as the bugs are investigated.
