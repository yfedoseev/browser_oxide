# 01 — browser_oxide hardware fingerprint surface baseline

**Surveyed:** 2026-05-27 via two Explore agents + direct source reads.
**Purpose:** snapshot of every hardware-class fingerprint surface BO currently exposes, where the value flows from, and whether it's masked. Used as the "before" state for the v150 diff in `02_*` / `03_*`.

## Coverage snapshot

- **29/34** hardware surfaces fully wired through the stealth-profile system. All critical identifiers (CPU cores, device memory, screen dimensions, WebGL strings, audio configuration) are profile-driven.
- **2** surfaces unimplemented:
  - `navigator.oscpu` — Firefox-only API; Chromium doesn't expose it. ✅ correct for Chrome.
  - `screen.orientation` — hard-coded `landscape-primary` / angle `0` (realistic desktop default; not per-profile).
- **3** surfaces with design risk:
  - **`AudioContext.sampleRate`** — randomized per-session via `Math.random() < 0.80 ? 44100 : 48000` at `canvas_bootstrap.js:751`. Cross-page-load inconsistency within a SharedSession. → FIX-C.
  - `screen.availLeft` — hard-coded `0` (matches real Chrome).
  - `AnalyserNode` defaults — standard WebAudio defaults (`fftSize:2048`, `smoothingTimeConstant:0.8`). Not per-profile.

## Critical definition sites (file:line ↔ profile field)

| Surface | Definition site | Profile YAML field |
|---|---|---|
| `navigator.hardwareConcurrency` | `crates/js_runtime/src/js/window_bootstrap.js:974` | `cpu_cores` (via stealth_ext mapping at `stealth_ext.rs:64`) |
| `navigator.deviceMemory` | `window_bootstrap.js:979` | `device_memory` |
| `navigator.platform` | `window_bootstrap.js:962` | `platform` |
| `navigator.userAgent` | `window_bootstrap.js:961` | `user_agent` |
| `navigator.language` | `window_bootstrap.js:970` | `language` |
| `navigator.languages` | `window_bootstrap.js:971,955` | `languages` (frozen array) |
| `navigator.maxTouchPoints` | `window_bootstrap.js:981` | `max_touch_points` |
| `navigator.vendor` | (verify; defaulted `"Google Inc."`) | `vendor` |
| `navigator.productSub` | (verify) | `product_sub` |
| `navigator.appVersion` | (verify) | `app_version` |
| `navigator.webdriver` | `window_bootstrap.js:992` | hard-coded `false` (gated) |
| `navigator.userAgentData.brands` | `window_bootstrap.js:1790` (`_makeLowBrands`) | derived from `browser_version` major |
| `navigator.userAgentData.mobile` | `window_bootstrap.js:1791` | hard-coded `false` (verify per `device_class`) |
| `navigator.userAgentData.platform` | `window_bootstrap.js:1792` | `os_name` |
| `navigator.userAgentData.getHighEntropyValues` arch | `window_bootstrap.js:1794-1825` | `cpu_architecture` ✅ (FIX-A made HTTP match) |
| `navigator.userAgentData.getHighEntropyValues` bitness | same | `cpu_bitness` |
| `navigator.userAgentData.getHighEntropyValues` platformVersion | same | `platform_version` |
| `navigator.userAgentData.getHighEntropyValues` model | same | `ua_model` |
| `screen.width` | `window_bootstrap.js:1437` | `screen_width` |
| `screen.height` | `window_bootstrap.js:1438` | `screen_height` |
| `screen.availWidth` | `window_bootstrap.js:1439` | `screen_avail_width` |
| `screen.availHeight` | `window_bootstrap.js:1440` | `screen_avail_height` |
| `screen.availTop` | `window_bootstrap.js:1442` | `screen_avail_top` |
| `screen.availLeft` | `window_bootstrap.js:1441` | hard-coded `0` (Chrome default) |
| `screen.colorDepth` | `window_bootstrap.js:1443` | `screen_color_depth` |
| `screen.pixelDepth` | `window_bootstrap.js:1444` | (usually == colorDepth — verify) |
| `screen.orientation.type` | (hard-coded `landscape-primary`) | not per-profile |
| `screen.orientation.angle` | (hard-coded `0`) | not per-profile |
| `window.innerWidth` | `window_bootstrap.js:1481` | `inner_width` |
| `window.innerHeight` | `window_bootstrap.js:1482` | `inner_height` |
| `window.outerWidth` | `window_bootstrap.js:1483` | `outer_width` |
| `window.outerHeight` | `window_bootstrap.js:1484` | `outer_height` |
| `window.devicePixelRatio` | `window_bootstrap.js:1485` | `device_pixel_ratio` |
| WebGL `VENDOR` (0x1F00) | `canvas_bootstrap.js:446` | `gpu.vendor` (from GpuProfile) |
| WebGL `RENDERER` (0x1F01) | `canvas_bootstrap.js:447` | `gpu.renderer` |
| WebGL `UNMASKED_VENDOR_WEBGL` (0x9245) | `canvas_bootstrap.js:450` | `gpu.unmaskedVendor` (`webgl_vendor`) |
| WebGL `UNMASKED_RENDERER_WEBGL` (0x9246) | `canvas_bootstrap.js:451` | `gpu.unmaskedRenderer` (`webgl_renderer`) |
| WebGL `MAX_TEXTURE_SIZE`, `MAX_VIEWPORT_DIMS`, etc. | `canvas_bootstrap.js:419-441` | `gpu.params` (44 entries) |
| WebGL extension list | `canvas_bootstrap.js` (verify) | `gpu.extensions` (~30 entries) |
| `AudioContext.sampleRate` | `canvas_bootstrap.js:751,767` | ⚠️ `Math.random()` — not per-profile (see FIX-C) |
| `AudioContext.baseLatency` | `canvas_bootstrap.js:752-757,768` | ⚠️ same |
| `AudioContext.outputLatency` | `canvas_bootstrap.js:758-762,769` | ⚠️ same |
| `AudioContext.destination.maxChannelCount` | `canvas_bootstrap.js:741` | hard-coded `2` |
| `OfflineAudioContext` DynamicsCompressor signature | `canvas_bootstrap.js:827-882` + `ops.op_offline_audio_render` | seeded by `audio_seed` |
| `MediaDevices.enumerateDevices()` | `window_bootstrap.js:443-466` | `media_devices` array |
| `navigator.connection.effectiveType` | `window_bootstrap.js:171-174` | `connection_effective_type` |
| `navigator.connection.downlink` | same | `connection_downlink` |
| `navigator.connection.rtt` | same | `connection_rtt` |
| `performance.now()` | `op_perf_now_humanized()` (Rust) | granularity TBD |

## Profile→JS plumbing

Two paths:

**Path 1 — Direct profile reads.**

`window_bootstrap.js` and `worker_bootstrap.js` call `_p(key, fallback)` / `_pInt(key, fallback)` / `_pFloat(key, fallback)` helpers that wrap `ops.op_get_profile_value(key)`. The op is implemented in `crates/js_runtime/src/extensions/stealth_ext.rs:64-150` which switches on the key name and reads the field from `StealthProfile`. Field name in JS ≠ Rust field name in many cases — `stealth_ext.rs:64` maps `"hardware_concurrency" → p.cpu_cores`, for example. See the full map there.

**Path 2 — GpuProfile object for WebGL.**

`canvas_bootstrap.js:444` calls `WebGLRenderingContext._g()` which lazy-loads from `StealthProfile.gpu_profile` (a separate struct holding 44 GL parameters + extension list + per-precision-format triples). The object is cached on the WebGLRenderingContext class. All WebGL-related reads come from this single object.

## Worker realm parity ✅

`worker_bootstrap.js` at lines 54-129 replicates the navigator surface — main and worker realms agree on `platform`, `hardwareConcurrency`, `deviceMemory`, `userAgent`, `language`, `languages`, `maxTouchPoints`. The v146 inverse-bug check (Camoufox's #443 — main and worker disagreed in v135) PASSES on browser_oxide HEAD as of `5e06a56`.

## Masking coverage

Inherits from `crates/js_runtime/src/js/stealth_bootstrap.js` (the `_maskFunction` / `_maskAsNative` machinery) and the curated sweep at `dom_bootstrap.js:2995-3094`. **For the full table see `../16_STEALTH_FINGERPRINT_AUDIT.md §2`.**

Hardware-class surfaces specifically:

- ✅ All Navigator getter functions masked via `_maskAsNative(_NavProto, …)` at `window_bootstrap.js:1029-1035` (covers `userAgent`, `platform`, `hardwareConcurrency`, `deviceMemory`, `maxTouchPoints`, `vendor`, `productSub`, etc.)
- ✅ Screen prototype methods covered via the dom_bootstrap sweep at `:3036`
- 🟡 WebGL `getParameter` itself is implemented in JS — Function.prototype.toString returns native-code-style (`_maskFunction`'d) but the method body IS a JS function. Anti-bots typically don't read the bytecode, only the toString. **Coverage is appropriate.**
- ✅ AudioContext / all AudioNode subclasses masked via sweep at `dom_bootstrap.js:3046-3048`

## Cross-API consistency (FIX-A baseline)

The cross-API agent (2026-05-27) verified:
- ✅ Main-window navigator.* == worker navigator.* (for the v146 #443 inverse)
- ✅ WebGL vendor/renderer strings match `gpu` object exactly (no whitespace truncation)
- ✅ Screen/window arithmetic consistent: `screen_width × device_pixel_ratio = 3024×1964` matches a real 13.6" MBP M3
- ✅ `outer_height - inner_height = 111px` matches Chrome on macOS w/ bookmarks bar
- ❌ **`Sec-CH-UA-Arch` was `"x86"` in HTTP, `"arm"` in JS** — fixed in commit `960b55f` (FIX-A) on 2026-05-27.

## Known issues + follow-ups (this audit cycle)

| Tag | Issue | File | Status |
|---|---|---|---|
| FIX-A | Sec-CH-UA-Arch/Bitness/Wow64 hardcoded vs profile fields | `crates/net/src/headers.rs:253-300` | ✅ commit `960b55f` |
| FIX-C | AudioContext.sampleRate randomized per-session, not per-profile | `canvas_bootstrap.js:751-762` | ⬜ next |
| FIX-G | Canvas noise (5% PCG32 per-pixel) potentially detectable | `crates/canvas/src/canvas2d.rs:1092-1145`, `webgl_render.rs:407-445` | ⏸️ research pending |
| FIX-H | screen.orientation hard-coded, not per-profile | `window_bootstrap.js` | ⬜ low priority (mobile presets need it; desktop OK) |
| OPEN-1 | navigator.userAgentData.brands order randomization — does it need to match HTTP? | `window_bootstrap.js:1764-1768` | 🔵 capture real Chrome behaviour |
| OPEN-2 | WebGL extension list values for chrome_148_macos — verified against real Chrome capture? | `gpu_profile.extensions` | 🔵 capture |
| OPEN-3 | Sec-CH-Device-Memory header quantization vs `profile.device_memory` value | `headers.rs:300-302` | ⬜ Chrome spec says `{0.25, 0.5, 1, 2, 4, 8}` only |
| OPEN-4 | `connection.effectiveType` / `rtt` / `downlink` consistency with real network state | `window_bootstrap.js:171-174` | ⏸️ |
