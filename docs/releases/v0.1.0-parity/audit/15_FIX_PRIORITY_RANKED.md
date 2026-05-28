# 15 — Fix priority ranked (yield × effort)

**Last updated:** 2026-05-27 after FIX-A + FIX-C + FIX-F landed.

Order is **what to do next** for the v0.2.0 routed-median 107 → ≥115 push. Yield = number of the 11 target sites this fix is hypothesized to flip. Effort = wall-clock work estimate.

## Stack rank

| # | Tag | Issue | Yield (sites) | Effort | Status | File |
|--:|-----|-------|--------------|--------|--------|------|
| 1 | **FIX-A** | Sec-CH-UA-Arch/Bitness/Wow64 read profile, not platform | 0-7 (AWS WAF cluster) | 30 min | ✅ commit `960b55f` | `crates/net/src/headers.rs` |
| 2 | **FIX-B** | Single-site sweep (amazon-com / imdb / amazon-de) post-FIX-A+C | (validation) | 30 min | ✅ amazon-de FLIPPED 855KB; amazon-com + imdb still 2011/1995-byte stubs. Single-run; could be noise. | `target/release/examples/sweep_metrics` |
| 3 | **FIX-C** | AudioContext.sampleRate / baseLatency / outputLatency seed from `audio_seed`, not `Math.random()` | 0-7 (telemetry consistency) | 30 min | ✅ commit `93c8ed4` | `canvas_bootstrap.js:751-762`, profile.rs, presets.rs |
| 4 | **FIX-D** | apple_m3_macos GpuProfile aligned to WebGL 2 captured fixture: 36-ext list (was 40-mix), MAX_VIEWPORT_DIMS [16384,16384] (was [32767,32767]), ALIASED_POINT_SIZE_RANGE [1,511] (was [1,8190]) | 0-7 (cross-API correlation) | 2h shipped | ✅ commit `a8cc691` | `crates/stealth/src/gpu.rs` |
| **★** | **FIX-J** | **FileReader.readAsDataURL was a no-op stub returning empty data URL → AWS WAF challenge.js bailed "malformed". Now real base64-encoding of blob bytes.** | **7 (AWS WAF cluster)** | 4h shipped | ✅ commit pending — amazon-ca flipped to 227KB on validation | `shared_apis_bootstrap.js`, `window_bootstrap.js` + 4 chrome_compat tests + `awswaf_probe` oracle |
| 4a | **FIX-D2** | canvas_bootstrap.js: `getContext("webgl")` and `getContext("webgl2")` return same `WebGLRenderingContext` — should differentiate; current state means WebGL 1 sites see WebGL 2 values | follow-up | 1 day | ⬜ deferred | `canvas_bootstrap.js:925-964` |
| 4b | **FIX-D3** | nvidia_rtx_3060_windows + apple_m2_pro_macos + intel_uhd_630_linux: also using shared `common_params_desktop()` — needs per-GPU validation against real captures | 0-3 (Linux/Windows AWS WAF) | 1 day per preset | ⬜ deferred | `crates/stealth/src/gpu.rs` |
| 5 | **FIX-E** | `chrome_148_macos_sampled()` — per-Page profile sampling (4 screens × 5 cores × 4 RAM × seeds). Opt-in via `BROWSER_OXIDE_SAMPLE_PROFILE=1` env. | 0-3 (IP-clustering defence) | 4h shipped | ✅ commit pending | `crates/stealth/src/presets.rs`, `crates/browser/examples/sweep_metrics.rs` |
| 5a | **FIX-K** | humanize.js engagement audit — already engaged via `Page::navigate` init_scripts running BEFORE HTML scripts (`runtime.rs:286`). FIX-J flips confirm sufficient behavioural signal for some WAF regions. | (no action) | investigated | ✅ closed | `humanize.js`, audit/16 §FIX-K |
| 6 | **FIX-F** | Sec-CH-Device-Memory quantization: spec says `{0.25, 0.5, 1, 2, 4, 8}` only; quantize correctly | 0-2 (DataDome) | 1 hour | ✅ commit `8d8c067` | `crates/net/src/headers.rs:317-329` |
| 7 | **FIX-G** | Decide canvas-noise policy: keep 5% PCG32 jitter, disable, or make opt-in | 0-3 (cross-vendor) | 1 day | ⏸️ research | `crates/canvas/src/canvas2d.rs`, `webgl_render.rs` |
| 8 | **OPEN-1** | Sec-CH-UA brand-order randomization: HTTP fixed vs JS shuffled — verify real Chrome | (validation) | 4 hours | 🔵 in progress | capture real Chrome |
| 9 | **OPEN-2** | WebGL extension list validation against real Chrome 148 macOS capture | (validation) | 2 hours | 🔵 | capture real Chrome |
| 10 | **FIX-H** | screen.orientation per-profile (currently hard-coded) | 0 (no current target needs it) | 2 hours | ⬜ low priority | `window_bootstrap.js` |

## What this means for the v0.2.0 budget

**Smallest set to hit 115:**

If FIX-A flips 3-4 AWS WAF sites + FIX-C flips 1-2 more + FIX-D + FIX-F flip 1-2: that's 5-8 sites recovered. Combined with the existing 107 routed median: **est. 112-115.**

**Bigger leverage:**

FIX-E (profile sampler, 1 week) is a structural change that helps with the IP-clustering ceiling. Without it, even with perfect-fingerprint, hitting the same AWS WAF endpoint from the same datacenter IP with identical fingerprints is a reliability hazard. **Recommended for v0.2.x point release, not v0.2.0.**

## Decision rule

After each FIX commits:
1. Run single-site sweep against THE site we expect to flip
2. If it flipped, move on
3. If it didn't, dig into the response shape with `RUST_LOG=net=trace`
4. Update `16_DECISION_LOG.md` with the actual outcome

After every 3 FIXes:
1. Run the full 3-run × 4-profile gate
2. Confirm routed median is climbing
3. If not, halt and re-examine the assumptions in `03_HARDWARE_SPOOFING_DIFF.md`

## What this list does NOT include

- Per-vendor solvers (WASM PoW for AWS WAF/DataDome/Kasada) — `vendor_solvers` scope.
- Kasada frontier (canadagoose/hyatt/realtor) — deferred per `R-KASADA-FRONTIER`.
- DataDome WASM-iframe daily-key — `R-DATADOME-DAILY-KEY`, mixed scope.
- Behavioral signals — out of scope for this audit, see `humanize.js` / R-BESTBUY-AKAMAI for the visit-behaviour cluster.
