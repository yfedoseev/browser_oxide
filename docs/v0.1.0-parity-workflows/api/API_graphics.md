# API_graphics — Web API parity deep dive: graphics fingerprint surface

**Scope:** WebGL1 + WebGL2 (`getParameter` UNMASKED_*/MAX_*, `getSupportedExtensions`,
`getShaderPrecisionFormat`, `readPixels`), Canvas2D (`toDataURL`/`getImageData`
hash + noise policy), WebGPU (`navigator.gpu` adapter/limits), OffscreenCanvas.
**Date:** 2026-05-28. **Branch:** `fix/v0.1.0-fix4-canvas-parity`.
**Method:** read existing repo docs → external verification (Chrome WebGPU
changelogs, CreepJS, Camoufox via deepwiki) → BO source audit at file:line →
ranked fixes.

---

## 0. TL;DR — what this audit found

The WebGL **parameter** surface (the most-probed graphics signal, 11/12
vendors per doc 38 §4.2) is in good shape: profile-driven values matched to a
captured Chrome 147 M3 fixture, the FIX-D2 WebGL1/WebGL2 split landed this
session (commit `07491f9`) and is **coherent** for version strings, extension
lists, and class identity. Three real defects remain on the *graphics*
surface, in ROI order:

1. **WebGPU `navigator.gpu` is a strong, internally-inconsistent Chrome-148
   tell** (`window_bootstrap.js:6038-6056`). `requestAdapter()` resolves to a
   hand-rolled adapter that (a) carries a non-spec `name` property, (b) returns
   an **empty** `features` Set and `limits` `{}`, and (c) has a `requestDevice()`
   that **rejects**. Real Chrome 148 either returns a fully-populated adapter
   (with `info`, 25-40 `limits`, a `features` set, and a working `requestDevice`)
   or — the common headless/no-GPU case — `requestAdapter()` resolves to
   **`null`**. BO's "populated-but-broken" middle state is exactly what CreepJS
   "lie detection" keys on. **Public engine. ~1 day.**

2. **WebGL2RenderingContext has zero WebGL-2-only methods**
   (`canvas_bootstrap.js:661` — `class WebGL2RenderingContext extends
   WebGLRenderingContext {}`, empty body). A context returned by
   `getContext("webgl2")` reports `VERSION = "WebGL 2.0 ..."` yet
   `typeof gl.createVertexArray === "undefined"`, ditto `texImage3D`,
   `drawArraysInstanced`, `getBufferSubData`, `createQuery`, `fenceSync`, … (≈60
   WebGL-2-only entry points). A WebGL-2 context missing the WebGL-2 method set
   is a deterministic cross-API tell, the WebGL-2 analog of the conflation bug
   FIX-D2 just fixed for extensions. **Public engine. 1-2 days.**

3. **`getShaderPrecisionFormat` ignores the FIX-D2 surface split**
   (`canvas_bootstrap.js:582` calls `_g()` directly, not `_surfaceFor(this)`).
   Low impact today because BO's WebGL1 and WebGL2 shader precision are
   identical, but it is an un-finished corner of FIX-D2 and a future drift
   hazard. **Public engine. <0.5 day.**

The **canvas noise policy** (FIX-G, doc 08) remains the most important
*measured-impact* open question and is unchanged by this session: BO ships 5%
PCG32 per-pixel jitter ON by default while Camoufox v150 **disabled** its noise.
This audit reaffirms the doc-08 recommendation to A/B and most likely follow
Camoufox to OFF-by-default. **Public engine. 1-2 days incl. measurement.**

The `webgl-render` pixel-readback path stays correctly OFF by default — see §5.

---

## 1. What the existing repo docs already concluded

### 1.1 `38_VISUAL_AUDIO_FINGERPRINTING.md` (the category matrix)

- **Leverage thesis** (§1, §5): WebGL **parameters** are the single
  most-probed graphics surface (11/12 vendors; only Sucuri opts out), canvas is
  10/12, WebGL **pixel readback** is 6/12. One param/extension fix moves every
  WebGL-probing vendor at once. The doc's Tier-1 (3×1-day) recommendation was:
  (1) mask `WebGLRenderingContext.prototype`, (2) per-profile WebGL param
  goldens snapshot, (3) canvas `toDataURL` golden parity test.
- **BO WebGL coverage** (§4.3): two-layer architecture — a JS bootstrap that is
  always present (parameter/extension stubs) plus an opt-in `webgl-render`
  feature (OSMesa software GL) that is **OFF by default**. Production ships the
  stub path; `createShader/compileShader/linkProgram` are no-ops returning
  success (`canvas_bootstrap.js:589-650`).
- **Known gaps it logged** (§4.3.3): `webgl-render` off ⇒ identical pixel hash
  for every BO instance (very high for the 6 pixel-readback vendors); OSMesa ≠
  ANGLE-Metal even when on; **`WebGLRenderingContext.prototype` methods
  unmasked** (since FIXED — see §3.4); profile/renderer cross-signal risk if a
  macOS profile runs on Linux; `WebGL2RenderingContext` was a literal alias of
  `WebGLRenderingContext` (since PARTIALLY fixed by FIX-D2 — §3.2); WebGPU
  "fully MISSING" (now present-but-flawed — §3.5).
- **Canvas coverage** (§2.4): Skia-backed, CPU raster, "Chrome's 2D-canvas text
  IS Skia → parity by construction" (`canvas2d.rs:976-984`). Logged gaps: emoji
  rasterisation (renders Linux Noto under a macOS profile — high consistency
  gap), GPU-vs-CPU AA signature, missing `toDataURL` golden parity test.
- **WebGL gap rank-order** (§4.4): (1) mask the full prototype [DONE], (2)
  per-profile param goldens, (3) decide the `webgl-render` story → pick "keep
  off for v0.1.0" because no measured corpus site fails on pixel readback.

### 1.2 `07_FIX-D2-D3-WebGL.md` (the ticket)

- **FIX-D2**: `getContext` returned the SAME `WebGLRenderingContext` instance
  for both `"webgl"` and `"webgl2"` → a WebGL-1 request saw WebGL-2 strings +
  WebGL-2-only extensions. Modeled the fix on Camoufox's `MaskConfig.hpp`
  dichotomy (`webGl:parameters` vs `webGl2:parameters`,
  `webGl:supportedExtensions` vs `webGl2:supportedExtensions`).
- **FIX-D3**: `nvidia_rtx_3060_windows` / `apple_m2_pro_macos` /
  `intel_uhd_630_linux` still use the unmodified `common_params_desktop()`
  baseline with the wrong `MAX_VIEWPORT_DIMS=[32767,32767]` /
  `ALIASED_POINT_SIZE_RANGE=[1,8190]` that FIX-D corrected only for
  apple_m3_macos. **Still open** (verified §3.3).
- AWS-WAF's `challenge.js` reads `MAX_VIEWPORT_DIMS`, `ALIASED_POINT_SIZE_RANGE`,
  `MAX_TEXTURE_SIZE`, `VENDOR`/`RENDERER`, `UNMASKED_*`, `getSupportedExtensions`,
  `getContextAttributes`, `getShaderPrecisionFormat`.

### 1.3 `08_FIX-G-canvas-noise.md` (the noise policy decision)

- BO injects 5% PCG32-seeded jitter on Canvas2D (`canvas2d.rs:1092-1145`) and
  WebGL readPixels (`webgl_render.rs:407-445`), seeded by `profile.canvas_seed`.
- **Camoufox v150 DISABLED its canvas noise** (commit `e4528a2`, Apr 2026):
  enabling it made DataDome v6.2+/Cloudflare Turnstile/Imperva ABP/Akamai BMP v3
  score Camoufox *worse* — the jitter distribution was statistically detected
  faster than per-canvas tracking gained any clustering advantage.
- Decision: A/B-measure, then most likely disable-by-default with an opt-in env
  var. **Status: still research-pending** (not touched this session).

### 1.4 `HANDOFF_2026_05_28b.md` + audit/17 (the latest state)

- FIX-D2 (WebGL1/2 split) shipped (`07491f9`) and worker secure-context shipped
  (`5216336`, restores `crypto.subtle` in workers). **Neither flips a site.**
- The AWS-WAF cluster (7 sites: amazon-{ca,com,com-au,fr,in,jp}, imdb) is **NOT
  a fingerprint gap** — challenge.js *proceeds* with BO's fingerprint and calls
  `forceRefreshToken` in the offline oracle. The live blocker is the 50 ms
  inter-script `run_until_idle` drain in `build_page_with_scripts_init_and_storage`
  (`page.rs` ~3535) vs the oracle's 5 s drain: the PoW Web Worker never spawns.
  **This re-frames graphics fixes as hygiene/parity, not the AWS unlock.**

---

## 2. New external findings (verified this session)

### 2.1 WebGPU `GPUAdapter` shape changed — `requestAdapterInfo()` was REMOVED in Chrome 131

Per the Chrome for Developers changelogs and the Chromium blink-dev intent:

- **Chrome 127 (Jul 2024)**: `GPUAdapter.info` synchronous attribute shipped.
- **Chrome 130 (Oct 2024)**: non-standard `requestAdapterInfo()` deprecated.
- **Chrome 131 (Nov 2024)**: `requestAdapterInfo()` **removed**.

So a browser claiming Chrome 148 must expose `adapter.info` (a `GPUAdapterInfo`
with `vendor`, `architecture`, `device`, `description`) and must **not** rely on
`requestAdapterInfo()`. There is **no** standard `adapter.name` property — it
never existed on the spec adapter. BO's adapter exposes `name` and omits `info`.

Sources:
- [Chrome for Developers — WebGPU troubleshooting](https://developer.chrome.com/docs/web-platform/webgpu/troubleshooting-tips)
- [What's New in WebGPU (Chrome 127)](https://developer.chrome.com/blog/new-in-webgpu-127)
- [What's New in WebGPU (Chrome 131)](https://developer.chrome.com/blog/new-in-webgpu-131)
- [blink-dev: Intent to Remove non-standard requestAdapterInfo()](https://groups.google.com/a/chromium.org/g/blink-dev/c/HxOgGf4NzQ4)

### 2.2 WebGPU in headless / no-GPU contexts → `requestAdapter()` returns **null**

Per the same troubleshooting doc: "`requestAdapter()` returns null when there is
no matching GPU adapter" and "WebGPU requires a GPU (either hardware or
software-emulated)". Headless Chrome on a GPU-less Linux server (BO's typical
deployment) most commonly has `navigator.gpu` **present** (it's a Window
property gated on secure context) but `requestAdapter()` resolving to `null`
unless `--enable-unsafe-webgpu` + `--enable-vulkan`/SwiftShader are set. A
returned adapter on such a box would surface a SwiftShader/`fallback`-class
adapter, again with a populated `info`/`limits`/`features`.

**Implication for BO:** the *safest* Chrome-consistent behaviour for a stealth
engine that does not actually run a WebGPU backend is `requestAdapter() → null`
(matches headless no-GPU Chrome), NOT a fake populated adapter. A populated
adapter implies a working `requestDevice`; rejecting it is the inconsistency.

Source: [Chrome for Developers — WebGPU troubleshooting](https://developer.chrome.com/docs/web-platform/webgpu/troubleshooting-tips).

### 2.3 Anti-bot is already reading WebGPU `adapter.features` / `device.limits`

CreepJS evaluates OffscreenCanvas + WebGPU as first-class vectors and runs "lie
detection" — *consistency* checks across signals, not just collection. Anti-fraud
systems call `navigator.gpu.requestAdapter()` then read `adapter.features` and
`device.limits` for GPU architecture (shader support, max buffer sizes, SIMD
parallelism). An empty `features`/`limits` while WebGL advertises a concrete
`ANGLE (Apple, ... Apple M3 ...)` renderer is a cross-signal contradiction.

Sources:
- [Scrapfly — Browser fingerprinting with CreepJS](https://scrapfly.io/blog/posts/browser-fingerprinting-with-creepjs)
- [WebBrowserTools — Detect WebGPU Fingerprint](https://webbrowsertools.com/webgpu-fingerprint/)
- [RoundProxies — WebGL fingerprinting 2026](https://roundproxies.com/blog/webgl-fingerprinting/)

### 2.4 Camoufox (the v150 SOTA we chase) does NOT spoof WebGPU at all

Per deepwiki on `daijro/camoufox`: there is **no** `navigator.gpu`/WebGPU
spoofing in Camoufox — it's a Firefox fork, so `navigator.gpu` reflects genuine
Firefox/Gecko WebGPU state (on most Firefox-stable builds WebGPU is off →
`navigator.gpu` undefined or `requestAdapter` null). Camoufox concentrates on
WebGL params (`WebGLParamsManager`, `UNMASKED_*` via `setWebGLVendor/Renderer`
self-destructing setters), `getShaderPrecisionFormat`, `getContextAttributes`,
**deterministic per-context Canvas2D noise** (`ApplyCanvasNoise`: ±1 on the
first non-zero RGB channel, zero channels skipped to preserve `clearRect`
transparency), and OffscreenCanvas (resolves `userContextId` via
`WorkerPrivate` fallback so worker-side `OffscreenCanvas` gets the same seed as
the main thread).

**Two takeaways:**
1. BO's WebGPU shim is a *self-inflicted* tell that Camoufox simply does not
   carry. BO claims Chrome (where WebGPU is broadly shipped) so it can't just
   drop `navigator.gpu`; but it should make the adapter either spec-correct or
   `null`.
2. Camoufox's canvas-noise algorithm (±1 on first non-zero channel, skip zero
   channels) is *more conservative* than BO's (±1 on 5% of all pixels incl.
   potentially zero channels). BO's `clearRect`-transparency handling and noise
   footprint should be re-checked against this (relevant to FIX-G).

Source: deepwiki `daijro/camoufox` (Per-Context Fingerprint Isolation §5.6;
`CanvasFingerprintManager::ApplyCanvasNoise`; `WebGLParamsManager`).

---

## 3. BO code-level analysis (file:line)

### 3.1 WebGL parameter surface — `getParameter` and the profile catalog (GOOD)

- `canvas_bootstrap.js:489-503` — `getParameter(pname)` dispatches string params
  (VENDOR 0x1F00, RENDERER 0x1F01, VERSION 0x1F02, SLV 0x8B8C, UNMASKED_VENDOR
  0x9245, UNMASKED_RENDERER 0x9246) from the surface object, then numeric/array
  params from the profile-loaded `params` map.
- `crates/stealth/src/gpu.rs:18-71` — `GpuProfile` + `WebGL1Surface`; well
  documented. `apple_m3_family_profile` (`gpu.rs:175-231`) is the gold template:
  WebGL-2 surface in the top-level fields, distinct WebGL-1 surface in `webgl1`.
- `gpu.rs:301-313` — `apple_m3_params()` correctly overrides `MAX_VIEWPORT_DIMS`
  → `[16384,16384]` and `ALIASED_POINT_SIZE_RANGE` → `[1,511]` from the captured
  M3 fixture, diverging from the `common_params_desktop()` defaults
  (`[32767,32767]` / `[1,8190]`) that FIX-D found wrong.
- `gpu.rs:625-693` — `apple_m3_matches_captured_chrome_147_fixture` snapshot test
  locks the 36-extension WebGL-2 list, the overridden params, and HIGH_FLOAT
  precision. This is the doc-38 §4.4 item-2 "param goldens" guard, **already
  present for macOS**.

**Verdict:** the parameter surface is coherent and well-guarded *for the macOS
profile*. The residual gap is FIX-D3 (§3.3).

### 3.2 FIX-D2 WebGL1/WebGL2 split — coherent, with two unfinished corners

The split (commit `07491f9`) is real and well-built:
- `gpu.rs:246-292` — `apple_m3_webgl1_surface()`: spec-correct delta (drops
  WebGL-2-only `EXT_color_buffer_float`/`OES_draw_buffers_indexed`/
  `EXT_disjoint_timer_query_webgl2`/`WEBGL_clip_cull_distance`/
  `WEBGL_provoking_vertex`, re-adds core-promoted `OES_texture_float`/
  `ANGLE_instanced_arrays`/`WEBGL_depth_texture`/`EXT_disjoint_timer_query`/
  `WEBGL_color_buffer_float`); guarded by `apple_m3_webgl1_surface_is_spec_correct`
  (`gpu.rs:494-538`).
- `canvas_bootstrap.js:450-488` — `_g1()` (WebGL-1 surface, downgrades version
  strings) + `_surfaceFor(ctx)` (returns `_g1()` only when `ctx._isWebGL2 ===
  false`, else `_g()`). `getParameter` (`:491`), `getSupportedExtensions`
  (`:506`), and `getExtension` (`:560`, via `this.getSupportedExtensions()`) all
  route through the surface selector.
- `canvas_bootstrap.js:661` — `WebGL2RenderingContext extends
  WebGLRenderingContext` is now a *distinct* class with its own `toStringTag`
  (`:1127`) and `constructor` (`:1136`), and `getContext("webgl2")` constructs
  it with `_isWebGL2 = true` (`:1053-1055`, `:1231-1235`). The old
  `WebGLRenderingContext === WebGL2RenderingContext` one-line tell is gone.

**Unfinished corner A — `getShaderPrecisionFormat` bypasses the split.**
`canvas_bootstrap.js:582`: `const gpu = WebGLRenderingContext._g();` — uses the
WebGL-2 surface unconditionally instead of `_surfaceFor(this)`. Low risk
*today* because `standard_shader_precision()` (`gpu.rs:469-484`) is identical
for both surfaces, and the profile schema has no `webgl1_shader_precision`. But
it is an inconsistent application of the FIX-D2 pattern and a drift hazard if a
profile ever differentiates WebGL-1 mediump precision (mobile GPUs do).

**Unfinished corner B — WebGL2RenderingContext has no WebGL-2-only methods.**
`canvas_bootstrap.js:661` is an *empty* subclass. Confirmed by grep: BO's
canvas_bootstrap defines **none** of `createVertexArray`, `bindVertexArray`,
`deleteVertexArray`, `texImage3D`, `texStorage2D/3D`, `drawArraysInstanced`,
`drawElementsInstanced`, `vertexAttribDivisor`, `getBufferSubData`,
`createQuery`/`beginQuery`/`endQuery`, `createSampler`, `createTransformFeedback`,
`fenceSync`/`clientWaitSync`, `getUniformBlockIndex`, `drawBuffers`,
`getFragDataLocation`, … (~60 WebGL-2-only entry points). A context that
reports `getParameter(VERSION) === "WebGL 2.0 (OpenGL ES 3.0 Chromium)"` yet
returns `typeof gl.createVertexArray === "undefined"` is a deterministic
cross-API contradiction — the WebGL-2 *method-set* analog of the
extension-conflation tell FIX-D2 was created to kill. Any vendor that does
`"createVertexArray" in gl` or `typeof gl.texImage3D` (CreepJS-class probes,
AWS-WAF `challenge.js` enumerations) sees the lie.

### 3.3 FIX-D3 still open — non-macOS profiles carry wrong params + no WebGL-1 surface

- `gpu.rs:76-126` `nvidia_rtx_3060_windows()` — `params: common_params_desktop()`
  (the `[32767,32767]`/`[1,8190]` values FIX-D found wrong for Apple; need
  per-hardware NVIDIA capture), `webgl1: None` (so a WebGL-1 context on the
  Windows profile falls back to the JS `_g1()` version-string downgrade but
  still serves the WebGL-2-mixed extension list via `getSupportedExtensions`'s
  default branch — partial cover only).
- `gpu.rs:318-369` `apple_m2_pro_macos()` and `gpu.rs:373-420`
  `intel_uhd_630_linux()` — same: `common_params_desktop()` + `webgl1: None` +
  top-level `version: "WebGL 1.0 ..."` (these two were never updated to a WebGL-2
  surface at all, so they're internally WebGL-1-shaped but un-split).
- Data provenance is stale: header says "Chrome **131**" / "tls.peet.ws /
  browserleaks captures Q1 2026" (`gpu.rs:11-14`), while the corpus target is
  Chrome 148. The macOS profile was re-captured for 147; Windows/Linux were not.

**Impact:** any sweep that selects a Windows or Linux profile ships
param/extension values that don't match a real Chrome-148 on that hardware. Per
the ticket, fixing this "probably flips amazon-fr / amazon-in cluster on those
profiles' sweeps" — but HANDOFF_2026_05_28b's AWS root-cause (the live-nav drain)
says the AWS bail is *not* fingerprint, so down-weight that expectation. Treat
FIX-D3 as parity hygiene + a hedge for DataDome/Cloudflare cross-signal scoring.

### 3.4 WebGL prototype masking — DONE and durable (verifies doc-38 §4.4 item-1)

`canvas_bootstrap.js:1436-1451` — `_maskAllProtoFns(proto)` iterates
`Object.getOwnPropertyNames(proto)`, skips `constructor`, masks every own
function via `_maskAsNative`. Applied to both `WebGLRenderingContext.prototype`
(`:1447`) and `WebGL2RenderingContext.prototype` (`:1450`). This is *durable*
(new methods auto-mask) and resolves the doc-38 §4.3.3 / §5.4 "unmasked
prototype" P1 gap. `String(gl.getParameter)` now reports
`"function getParameter() { [native code] }"`.

**Caveat tied to §3.2-B:** `_maskAllProtoFns(WebGL2RenderingContext.prototype)`
is effectively a **no-op** — the WebGL2 subclass has zero own methods, so there's
nothing to mask there. When the WebGL-2-only methods are added (fix #2), they
must be defined as own properties on `WebGL2RenderingContext.prototype` *before*
this mask sweep runs so they get masked too. Order matters.

### 3.5 WebGPU `navigator.gpu` — present but inconsistent (the top new finding)

`window_bootstrap.js:6038-6056`:
```js
if (!_NavProto.hasOwnProperty('gpu')) {
    const _navGpu = {
        requestAdapter() {
            return Promise.resolve({
                name: _p("webgl_renderer", "ANGLE (NVIDIA, ...RTX 3080)"),  // (a) non-spec prop
                features: new Set(),                                         // (b) empty
                limits: {},                                                  // (b) empty
                isFallbackAdapter: false,                                    // (c) claims real GPU
                requestDevice() { return Promise.reject(new DOMException("Not supported", "NotSupportedError")); }, // (c) rejects
            });
        },
        getPreferredCanvasFormat() { return "bgra8unorm"; },
    };
    Object.defineProperty(_navGpu, Symbol.toStringTag, { value: "GPU", configurable: true });
    _defNav('gpu', () => _secure() ? _navGpu : undefined);   // secure-context gate (correct)
}
```
Problems, in order of detectability:
1. **`name` is not a real GPUAdapter property** and `info` is missing (§2.1).
   Real Chrome 148: `adapter.info` is a `GPUAdapterInfo`; there is no `name`.
2. **`features` empty Set + `limits` empty `{}`** while `isFallbackAdapter:
   false` — a real non-fallback adapter has ~25-40 `limits` numeric entries and
   a non-empty `features` set (§2.3). Empty + non-fallback is contradictory.
3. **`requestAdapter()` resolves a populated adapter but `requestDevice()`
   rejects** — a populated non-fallback adapter that cannot produce a device is
   a state real Chrome never reaches. This is the cleanest CreepJS "lie".
4. The adapter is a **plain object**, not branded `GPUAdapter`
   (`Object.prototype.toString.call(adapter)` ≠ `"[object GPUAdapter]"`; no
   `GPUAdapter`/`GPUAdapterInfo` globals on `dom_bootstrap.js:3107` only lists
   `GPUAdapter` as a name string, not a real constructor).
5. `requestAdapter` / `getPreferredCanvasFormat` are not `_maskAsNative`-masked
   → `Function.prototype.toString` leaks JS source.

**The fix-direction is not "build a full WebGPU".** The Chrome-consistent move
for an engine with no WebGPU backend is to mirror **headless-no-GPU Chrome**:
`navigator.gpu` present (secure-context gated — already correct at `:6055`),
`getPreferredCanvasFormat()` returns `"bgra8unorm"` (fine), but
**`requestAdapter()` resolves to `null`** (§2.2). That removes every
inconsistency above with the least surface area. (If a populated adapter is ever
wanted, it must be fully spec-correct: `GPUAdapter`-branded, `info` present, no
`name`, populated `limits`/`features`, working `requestDevice` returning a
`GPUDevice` — a much larger build, not justified by any measured corpus site.)

### 3.6 Canvas2D — Skia parity good; noise policy is the open lever

- `canvas2d.rs:1186-1204` — CPU Skia `surfaces::wrap_pixels` → premult RGBA8.
- `canvas2d.rs:1093-1144` — `to_data_url_with_jitter`: PCG32 seeded by
  `profile.canvas_seed`, perturbs ±1 LSB on ~5% of pixels.
- Masking: `canvas_bootstrap.js:1361-1402` masks `CanvasRenderingContext2D.prototype`
  methods + `HTMLCanvasElement.prototype` `getContext`/`toDataURL`/`toBlob`/
  `transferControlToOffscreen`. Good.
- **Noise policy (FIX-G, unresolved):** BO 5%-all-pixels ON by default vs
  Camoufox v150 noise-OFF, and vs Camoufox's narrower ±1-on-first-nonzero-channel
  algorithm when it *was* on (§2.4). doc-08's A/B is still pending. This is the
  highest *measured-impact* canvas item but is a measurement task, not a code
  defect.
- Emoji-rasterisation consistency gap (doc-38 §2.4.3) unchanged: a macOS profile
  renders Linux Noto Color Emoji. Out of scope for this graphics-API audit
  (font-stack work) but noted for cross-reference.

### 3.7 OffscreenCanvas — better than doc-38 implied; one real gap

- `canvas_bootstrap.js:1308-1352` — `RealOffscreenCanvas` (main-thread) backs
  `getContext("2d")` with the same Skia ops as `<canvas>`, plus
  `transferToImageBitmap` and `convertToBlob`. `toStringTag = "OffscreenCanvas"`
  (`:1353`). `transferControlToOffscreen` masked (`:1398`).
- **But `RealOffscreenCanvas.getContext` returns `null` for anything but
  `"2d"`** (`:1316-1317` `if (type !== "2d") return null`). By contrast the
  on-DOM `<canvas>.getContext` (`:1218-1239` and `:1049-1056`) *does* support
  `"webgl"`/`"webgl2"`. So `new OffscreenCanvas(1,1).getContext("webgl")` →
  `null` while `document.createElement("canvas").getContext("webgl")` → a
  context. Real Chrome supports WebGL on OffscreenCanvas (it's the headless
  rendering path). A vendor probing OffscreenCanvas-WebGL (CreepJS does) sees the
  asymmetry. **Medium-low**; Camoufox explicitly handles OffscreenCanvas-WebGL
  in workers (§2.4), so this is a parity gap vs the SOTA.
- `convertToBlob` only honours PNG-ish output via `op_canvas_to_data_url`
  (`:1336-1351`); `image/jpeg`/`image/webp` quality options are not applied
  (low impact).

### 3.8 readPixels / pixel-readback — correctly deferred

`canvas_bootstrap.js:343` `readPixels` exists in the stub path; the real shader
pipeline is the OSMesa `webgl-render` feature (`webgl_render.rs`,
`webgl_ext.rs:43-45` `op_webgl_available → false` unless feature on). doc-38
§4.4 item-3 concluded: keep `webgl-render` OFF for v0.1.0 (no measured corpus
site fails on pixel readback; OSMesa software output would itself be a
cross-signal mismatch vs an ANGLE-Metal renderer string). This audit concurs —
do not flip the default. The stub `readPixels` returning seeded-constant data is
acceptable until a measured site proves otherwise.

---

## 4. Coherence verdict on the FIX-D2 split (the task's direct ask)

- **Version strings:** coherent. WebGL-1 ctx → `"WebGL 1.0 (OpenGL ES 2.0
  Chromium)"`, WebGL-2 → `"WebGL 2.0 (OpenGL ES 3.0 Chromium)"`. ✔
- **Extension lists:** coherent + spec-correct delta, guarded by a unit test. ✔
- **Class identity:** coherent — distinct constructors, distinct `toStringTag`,
  `getContext` wires the right one. ✔ (fixes the old alias tell)
- **`getParameter` / `getSupportedExtensions` / `getExtension`:** all route
  through `_surfaceFor(this)`. ✔
- **Prototype `Function.toString` masking:** done and durable for
  WebGLRenderingContext; a **no-op for WebGL2** today because the subclass has no
  own methods (see below). ✔ for WebGL-1, ⚠ pending for WebGL-2.
- **`getShaderPrecisionFormat`:** ✗ does not use the surface split
  (`:582` uses `_g()`). Low risk, but incoherent with the FIX-D2 pattern.
- **WebGL-2 method set:** ✗ the WebGL-2 context is missing all ~60 WebGL-2-only
  methods → the largest remaining cross-API tell on this surface.

**Bottom line:** FIX-D2 is *coherent for what it covers* (strings, extensions,
identity, the masked prototype) but is **incomplete in two ways** — the empty
WebGL-2 method set and the un-split `getShaderPrecisionFormat`. The prototype
getParameter masking is properly applied for WebGL-1.

---

## 5. Ranked fixes (ROI order)

| # | Fix | Effort | Confidence | Engine |
|---|---|---|---|---|
| G1 | WebGPU adapter consistency: `requestAdapter() → null` (headless-no-GPU Chrome), drop `name`, mask `requestAdapter`/`getPreferredCanvasFormat` | ~1 day | high | public |
| G2 | Add WebGL-2-only method set to `WebGL2RenderingContext.prototype` (so a webgl2 ctx has `createVertexArray`/`texImage3D`/… before the mask sweep) | 1-2 days | medium-high | public |
| G3 | FIX-G: A/B canvas noise ON vs OFF on CreepJS/browserleaks + corpus subset; most likely default-OFF + opt-in env var | 1-2 days (mostly measurement) | medium | public |
| G4 | FIX-D3: per-hardware NVIDIA + Intel Chrome-148 captures → params + WebGL-1 surface for windows/linux profiles | 2-4 days (needs real captures) | medium | public |
| G5 | Route `getShaderPrecisionFormat` through `_surfaceFor(this)` + add `webgl1_shader_precision` profile key | <0.5 day | high | public |
| G6 | OffscreenCanvas WebGL: let `RealOffscreenCanvas.getContext("webgl"/"webgl2")` return a context (parity w/ `<canvas>` + Camoufox) | 1 day | medium | public |

### Detail per fix

**G1 — WebGPU adapter consistency (top ROI).** `window_bootstrap.js:6038-6056`.
Smallest, highest-signal-removal change: make `requestAdapter()` resolve `null`
(matches the dominant headless-no-GPU Chrome state, §2.2) instead of a
contradictory populated-but-deviceless adapter (§2.1, §2.3). Keep the
secure-context gate (`:6055` already correct) and `getPreferredCanvasFormat →
"bgra8unorm"`. `_maskAsNative` the two functions. Expected site impact: no
single corpus site is *known* to gate on WebGPU today, but this removes a clean
CreepJS-class "lie" and a Chrome-148 inconsistency that any ML scorer
(DataDome/Cloudflare/Akamai) can weight — pure downside-removal, near-zero
regression risk. **Does not by itself flip a site; it is consistency hygiene** —
align expectations with the FIX-D2 lesson (HANDOFF_2026_05_28b: real fixes that
don't flip a site are still worth shipping for parity).

**G2 — WebGL-2 method set.** `canvas_bootstrap.js:661`. Define the ~60
WebGL-2-only methods as stubs on `WebGL2RenderingContext.prototype` (mirror the
WebGL-1 stub style at `:589-650`: VAO create/bind/delete returning `{_id}`,
`texImage3D`/`texStorage*` no-ops, `drawArraysInstanced`/`drawElementsInstanced`
no-ops, query/sampler/transform-feedback/sync object stubs, `drawBuffers`,
`getBufferSubData` writing zeros). Critically, define them **before** the
`_maskAllProtoFns(WebGL2RenderingContext.prototype)` sweep (`:1450`) so
`Function.toString` is masked. Add a `webgl_parity.rs` guard asserting
`typeof gl2.createVertexArray === "function"` and `gl2.createVertexArray !==
gl1.createVertexArray`-style checks. Removes the largest remaining cross-API
graphics tell. Confidence medium-high; the method list is well-defined by the
WebGL 2 spec.

**G3 — Canvas noise A/B (FIX-G).** Highest *measured-impact* item but blocked on
measurement, hence not #1. Add `BROWSER_OXIDE_DISABLE_CANVAS_NOISE=1`, A/B on
CreepJS + browserleaks/canvas + a canvas-sensitive corpus subset, compare
verdicts + `lies-canvas`. Camoufox's measured outcome (noise hurt) is the prior;
expect default-OFF. While at it, narrow BO's algorithm toward Camoufox's
(±1 on first non-zero channel, skip zero channels to preserve `clearRect`
transparency) if noise stays opt-in. Touches `canvas2d.rs:1093-1144`,
`webgl_render.rs:407-445`, `profile.rs` (per-profile flag).

**G4 — FIX-D3 captures.** Blocked on real Chrome-148 Windows-NVIDIA and
Linux-Intel WebGL dumps (sources: a real Chrome run, or the Camoufox
`webgl_data.db` at `crates/stealth/fixtures/camoufox_webgl/webgl_data.db` which
has per-OS NVIDIA/Intel/AMD rows). Mirror the `apple_m3_family_profile` +
`apple_m3_webgl1_surface` + snapshot-test structure for
`nvidia_rtx_3060_windows`/`intel_uhd_630_linux`. Down-weight the AWS-flip
expectation (AWS bail is the live-nav drain, not fingerprint — HANDOFF_2026_05_28b);
value is parity + DataDome/Cloudflare cross-signal hedge.

**G5 — getShaderPrecisionFormat surface split.** `canvas_bootstrap.js:582`:
`_g()` → `_surfaceFor(this)`; add an optional `webgl1_shader_precision` profile
key + `WebGL1Surface.shader_precision`. Trivial, finishes FIX-D2's pattern,
prevents future drift. Low measured impact today (precision is identical across
surfaces) but cheap and removes an inconsistency.

**G6 — OffscreenCanvas WebGL.** `canvas_bootstrap.js:1316-1317`: drop the
`type !== "2d"` early-null; construct a `WebGL[2]RenderingContext` with
`_isWebGL2` like the `<canvas>` path. Achieves parity with both BO's own
`<canvas>` and Camoufox's worker-side OffscreenCanvas-WebGL. Medium confidence;
CreepJS probes this path.

---

## 6. Open questions

- Does any of the 19 non-passing corpus sites actually gate on WebGPU presence
  or `adapter.features`? No evidence yet; G1 is consistency hygiene, not a known
  unlock. A diagnostic (`diagnostic_creepjs.rs`, currently `#[ignore]`) could
  confirm the "lie" fires.
- Is the FIX-G canvas-noise tradeoff the same for BO (Chrome-shaped, Skia CPU
  raster) as Camoufox measured for Firefox? Must A/B on BO directly (G3).
- Does the Camoufox `webgl_data.db` carry Chrome-148-era (vs 131/147) NVIDIA/Intel
  rows, or do we need a fresh real-Chrome capture for G4?
- Should BO ever ship a *fully* spec-correct populated WebGPU adapter (G1
  alternative), and is there a corpus site that would reward it over `null`?
  Currently judged not worth the build.

---

## 7. Files referenced (BO)

| File:line | What |
|---|---|
| `crates/js_runtime/src/js/window_bootstrap.js:6038-6056` | `navigator.gpu` shim (G1 site) |
| `crates/js_runtime/src/js/canvas_bootstrap.js:343` | stub `readPixels` |
| `…/canvas_bootstrap.js:450-488` | `_g1()` / `_surfaceFor` (FIX-D2 core) |
| `…/canvas_bootstrap.js:489-503` | `getParameter` dispatch |
| `…/canvas_bootstrap.js:506-540` | `getSupportedExtensions` (surface-aware) |
| `…/canvas_bootstrap.js:566-579` | `getContextAttributes` (10 Chrome defaults) |
| `…/canvas_bootstrap.js:582` | `getShaderPrecisionFormat` (G5 — uses `_g()`) |
| `…/canvas_bootstrap.js:589-650` | WebGL stub method bodies (G2 template) |
| `…/canvas_bootstrap.js:661` | empty `WebGL2RenderingContext` (G2 site) |
| `…/canvas_bootstrap.js:1049-1056`, `1218-1240` | `<canvas>.getContext` webgl/webgl2 |
| `…/canvas_bootstrap.js:1308-1352` | `RealOffscreenCanvas` (G6 site) |
| `…/canvas_bootstrap.js:1436-1451` | `_maskAllProtoFns` prototype mask sweep |
| `crates/stealth/src/gpu.rs:18-71` | `GpuProfile` + `WebGL1Surface` |
| `crates/stealth/src/gpu.rs:175-313` | apple_m3 family profile + WebGL1 surface + params |
| `crates/stealth/src/gpu.rs:76-126, 318-420` | nvidia/m2pro/intel profiles (G4 — FIX-D3 open) |
| `crates/stealth/src/gpu.rs:494-693` | gpu.rs guard tests (incl. captured-fixture snapshot) |
| `crates/canvas/src/canvas2d.rs:1093-1144` | canvas noise (FIX-G / G3) |
| `crates/canvas/src/webgl_render.rs:407-445` | WebGL readPixels noise (OSMesa, opt-in) |
| `crates/js_runtime/src/extensions/webgl_ext.rs:43-45` | `op_webgl_available → false` by default |
| `crates/stealth/fixtures/camoufox_webgl/webgl_data.db` | per-OS GPU rows for G4 captures |

## 8. Sibling docs

`38_VISUAL_AUDIO_FINGERPRINTING.md` (category matrix), `07_FIX-D2-D3-WebGL.md`,
`08_FIX-G-canvas-noise.md`, `16_STEALTH_FINGERPRINT_AUDIT.md` (prototype mask
axis), `17_WEB_API_PARITY_MATRIX.md` (§2.10 WebGL/§2.17 audio interface
inventory), `06_AWS_WAF_SOLVER.md`, `HANDOFF_2026_05_28b.md` (AWS root-cause =
live-nav drain, not fingerprint).
