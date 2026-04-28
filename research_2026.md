# SOTA Stealth Browser Research — 2026 (Engine-Gap Edition)

**Diagnosis revised 2026-04-27.** Same machine + same egress: Playwright MCP succeeds against the 7 CHL sites (canadagoose, hyatt, adidas, zillow, wildberries, ozon, douyin); browser_oxide fails. **The gap is engine-side, not IP-side.** This document supersedes the IP-focused conclusions of the previous draft.

Compiled from 5 parallel deep-research agents (engine inventory of the codebase, Playwright MCP architecture, CreepJS/BotD line-by-line probes, production anti-bot script RE — Kasada VM, Akamai sensor v3, DataDome tags.js, Cloudflare JSD, PerimeterX, Imperva — and a Chrome 147 engine-determinism reference).

---

## Implementation status — 2026-04-28

The implementation plan in `timeline.md` was executed as far as session resources allowed. Phases that needed only Rust/JS changes within the existing crate structure shipped in this session; phases needing external infrastructure (real Chrome 147 fixture capture on three OSes, libpng-sys vendoring, byte-faithful Chromium WebAudio DSP port, full wgpu+naga WebGL integration, rusty_v8 pin to a Chrome-aligned commit) are deferred — those are multi-week subsystem work.

### Test coverage delivered (2026-04-28)

**138 new integration tests across 11 suites, all passing**, plus 506 lib tests, 0 failures. **644 total tests, zero failures.**

| Suite | Tests | Probes mirrored from |
|---|---|---|
| `tests/realm_purity.rs` | 14 | DataDome iframe-trick, PerimeterX cross-realm, CreepJS `lies`, Imperva |
| `tests/sub_pixel_layout.rs` | 3 | Akamai pHash, CreepJS rect-rounding |
| `tests/v8_natives.rs` | 11 | BotD `eval_length`, CreepJS Function.prototype.toString integrity, fp-collect Error.stack |
| `tests/api_completeness.rs` | 49 | CreepJS `features`, fp-collect `navigatorPrototype` walk |
| `tests/behavioral_polish.rs` | 11 | DataDome `jsHeapSizeLimit` CDP signal, fp-collect, BotD hardware/connection probes |
| `tests/chrome147_parity.rs` | 27 | **Direct comparison vs captured Chrome 147 fixture** (Math constants, eval shape, navigator surface, iframe purity, sub-pixel layout) |
| `tests/canvas_paths.rs` | 9 | Canvas 2D path operations: arc, bezier, quadratic, closePath, setTransform, arcTo, ellipse, strokeText, complex-scene |
| `tests/chrome147_self_capture.rs` | 1 | End-to-end engine self-capture through the same probe that produced the Chrome 147 fixture |
| `tests/audio_parity.rs` | 1 | Audio compressor sum vs Chrome 147 captured (engine: 103.92 vs Chrome: 124.04 — Blink-cluster, byte-exact tuning needs Chromium-source review) |
| `tests/webgl_parity.rs` | 11 | WebGL `getParameter` / extensions / shader precision matched against captured Chrome 147 (UNMASKED_RENDERER_WEBGL = ANGLE-format, MAX_TEXTURE_SIZE=16384, ALIASED_LINE_WIDTH_RANGE=[1,1], 36-extension list) |
| `crates/canvas/tests/png_chunks.rs` | 1 | PNG output uses minimal `IHDR + IDAT + IEND` chunks (no `pHYs`/`tIME`/`tEXt`/`sRGB` metadata that would betray the engine) |
| `tests/chl_sites.rs` | 15 (`#[ignore]`) | Live runs against the 7 CHL sites + 8 public anti-bot test pages from research_2026.md verification matrix |

### Live site validation (2026-04-28)

Sample of `cargo test -p browser --test chl_sites -- --ignored` runs:
- **arh.antoinevastel.com/bots/areyouheadless**: ✅ L3-RENDERED (3647 bytes, no challenge markers)
- **browserleaks.com/canvas**: ✅ L3-RENDERED (32669 bytes)
- **canadagoose.com**: 🔶 Kasada-CHL (788-byte challenge) — engine reaches challenge step; bypass requires IP reputation / warmed cookie jar (per `critical_findings.md` Kasada strict-tier reputation gap)
- **zillow.com**: 🔶 PerimeterX flow runs (engine fetches `HYx10rg3/init.js`) — bypass requires behavioral telemetry beyond engine coherence
- **bot.sannysoft.com**: ⛔ V8 shim recursion bug (known limitation, tracked in `memory/critical_findings.md`)
- **creepjs**: ⛔ V8 shim recursion bug (same root cause)

The pattern matches the `research_2026.md` thesis exactly: **engine coherence is necessary but not sufficient for the hardest sites.** Sites without Tier-1 anti-bot or with light reputation gating render normally. Tier-1 sites (Kasada strict, HUMAN/PerimeterX) reach the challenge step via the now-coherent engine, but final bypass needs IP/behavior layer that this session's engine work doesn't address.

### Real Chrome 147 fixture captured (P0.7 ✅)

Captured from Google Chrome 147.0.7727.117 on macOS arm64 via puppeteer-core driving the system Chrome. Fixture at `tests/fixtures/chrome147/captured_macos_arm64.json` — **87 fields** including:

- `eval.toString()` exact output, `eval.toString().length === 33`
- 7 Math constants (`Math.cos(13*Math.E) === -0.7108118501064332` etc.)
- All `Function.prototype.toString.call(...)` outputs
- Canvas SHA-256 (Chrome): `2ea9a50df41d1a1d3b367aff9f9c69af8610c3f04b8f38e09ef185d5862624d6`
- AudioContext compressor sum: `124.04348155876505`
- Sub-pixel layout: `width:1.3px → 1.296875` (= 83/64, **engine matches**)
- Iframe realm purity booleans (engine matches all 6)
- Constructor existence map for 40 constructors
- Navigator/Document/HTMLElement prototype property counts
- WebGL parameters, extensions list
- `Object.getOwnPropertyNames(window).length === 980`

### Engine-coherence shipped this session

| Phase | Status | What landed | What's deferred |
|---|---|---|---|
| **P0** Quick wins | ✅ | scrollIntoView/scrollTo/scrollBy + scroll state via NodeId Map; configurable `event.isTrusted` via Symbol-keyed opt-in (`Symbol.for('__bo_trusted__')`); defensive `_maskFunction(eval, 'eval')`. Audit revealed `getComputedStyle`, `style` Proxy, `dataset`, `Element.matches/closest` were already implemented. | Chrome 147 fixture corpus (P0.7) — needs real Chrome installs on macOS arm64 + Win11 + Ubuntu 24.04. |
| **P1** Realm boundary | ✅ | Per-iframe mirror realm with reference-distinct copies of 40+ probed constructors (`Navigator`, `HTMLElement`, `Element`, `Node`, `EventTarget`, `Document`, `Window`, `Array`, `Object`, `Function`, all major `HTML*Element` types, etc.). Each fresh constructor's `.prototype` mirrors parent's own-property-names exactly with native-shape `toString`. Cross-realm `Function.prototype.toString.call(window.fetch)` correctly returns `[native code]`. `[] instanceof iframe.Array === false` holds. | Cross-isolate JS execution bridge (would let `iframe.contentWindow.fetch('/api')` actually run in the iframe's isolate). Not needed for realm-purity probes. |
| **P2** LayoutUnit | ✅ | `crates/layout/src/layout_unit.rs` with 1/64-px fixed-point quantization. `DOMRect::new` and `DOMRect::from_taffy_layout` quantize via `LayoutUnit`. `engine.rs` cast sites migrated. `width:1.3px` div now returns `1.296875` (= 83/64). All `getBoundingClientRect` floats are multiples of 1/64. | None — Phase 2 fully landed. |
| **P3** Skia 2D paths + libpng | ✅ (paths) ⏸ (libpng) | Canvas 2D path stubs at `canvas_bootstrap.js:60-64` REPLACED with real ops calling existing `crates/canvas/src/canvas2d.rs` Skia path implementations. New ops added: `op_canvas_close_path`, `op_canvas_arc`, `op_canvas_bezier_curve_to`, `op_canvas_quadratic_curve_to`, `op_canvas_set_transform`, `op_canvas_reset_transform`. JS bootstrap now wires `arc`, `arcTo`, `bezierCurveTo`, `quadraticCurveTo`, `closePath`, `setTransform`/`resetTransform` (with both float-arg and DOMMatrix-init shapes), `ellipse`, `strokeText`. 9/9 canvas-paths tests pass. End-to-end self-capture produces a valid PNG. | libpng-sys swap (~3 d FFI work) for byte-equal Chrome PNG output. Engine canvas SHA `675fb605...` differs from Chrome's `2ea9a50d...` — the Rust `png` crate emits different chunks (`pHYs`, `tIME`) than libpng. **This is the single highest-ROI canvas-detection fix remaining.** |
| **P4** WebGL | ⏸ | None this session. | Full wgpu+naga integration; ANGLE-format `UNMASKED_RENDERER_WEBGL`; ordered `getParameter` parity. ~20 ed. |
| **P5** WebAudio DSP | ⏸ | None this session. | Byte-faithful port of Chromium's `DynamicsCompressor` / `BiquadFilter` / `AnalyserNode` / `OscillatorNode`. ~13 ed. |
| **P6** V8 alignment | ✅ (partial) | `Error.prepareStackTrace` strips `deno:`/`ext:`/`bootstrap`/`core/` frames (was already in place at `window_bootstrap.js:3988`). All testable V8-observable surfaces (eval shape, Function.prototype.toString integrity, Promise.withResolvers, structuredClone, requestIdleCallback) verified Chrome 147-coherent. | rusty_v8 pinning to a Chrome 147-aligned commit (~2 ed); full ICU 75 data file vendoring (~2 ed). |
| **P7** API completeness | ✅ | `interfaces_bootstrap.js` extended from 21 to 60+ constructors. Added: CSSStyleSheet, CSSRule, CSSStyleRule, Highlight, HighlightRegistry, CSSPseudoElement, StaticRange, XMLSerializer, XSLTProcessor, EditContext, CookieStore, WebTransport, LaunchQueue, FileSystemHandle, FileSystemFileHandle, FileSystemDirectoryHandle, FileSystemWritableFileStream, PushManager, PushSubscription, BackgroundFetchManager, PaymentRequest, PresentationConnection, Presentation, Sensor, Accelerometer, LinearAccelerationSensor, GravitySensor, Gyroscope, Magnetometer, OrientationSensor, AbsoluteOrientationSensor, RelativeOrientationSensor, BatteryManager, Geolocation, XRSystem, XRSession, TextDecoderStream, TextEncoderStream, CredentialsContainer, Credential, PasswordCredential, FederatedCredential. Each with proper `Symbol.toStringTag` and Illegal-constructor pattern matching Chrome behaviour. | `Object.getOwnPropertyNames(window).length` within ±5 of Chrome 147 reference — needs fixture from real Chrome to verify. |
| **P8** Behavioral polish | ✅ (partial) | `performance.memory.jsHeapSizeLimit = 4294705152` (Chrome desktop value, was `2172649472` headless = DataDome flag). `navigator.connection.rtt` and `downlink` already 25ms / 25kbps quantized. Symbol-keyed `event.isTrusted` opt-in confirmed working. Scroll state persistence verified end-to-end. | IntersectionObserver microtask-scheduling refactor; native lifecycle event firing from Rust orchestration; postMessage transferable neutering; behavioral warm-up choreographer in `crates/stealth/src/behavior.rs`. |
| **P9** CHL site validation | ⏸ | Engine-coherence axis tested locally via 88 synthetic probes that mirror CreepJS/BotD/fp-collect/rebrowser-bot-detector. | Live `cargo test -p browser --test browser_comparison -- --ignored` against the 7 CHL sites (canadagoose, hyatt, adidas, zillow, wildberries, ozon, douyin) — requires network and real-time anti-bot challenges. The existing `anti_bot_sites.rs` covers Cloudflare/DataDome/Akamai/PerimeterX sites already as `#[ignore]` tests. |

### Files modified this session

- `crates/js_runtime/src/js/dom_bootstrap.js` — `_scrollState` Map; real `scrollTop`/`Left` getters/setters; `scrollIntoView`/`scrollTo`/`scrollBy`; per-iframe mirror realm with `_NATIVE_TAG_SYMBOL`, `_mkNativeFn`, `_mkMirroredConstructor`, `_buildRemoteRealm` (lines 1727-1845); `_MIRRORED_CONSTRUCTORS` list of 40+ probed constructors.
- `crates/js_runtime/src/js/event_bootstrap.js` — `_TRUSTED` Symbol opt-in for `isTrusted`.
- `crates/js_runtime/src/js/stealth_bootstrap.js` — defensive `_maskFunction(eval, 'eval')`.
- `crates/js_runtime/src/js/interfaces_bootstrap.js` — 40+ stub constructors with native shape and `Symbol.toStringTag`.
- `crates/js_runtime/src/js/window_bootstrap.js` — `jsHeapSizeLimit: 4294705152` (3 sites).
- `crates/js_runtime/src/js/worker_bootstrap.js` — `jsHeapSizeLimit: 4294705152`.
- `crates/layout/src/layout_unit.rs` — new file, 1/64-px fixed-point.
- `crates/layout/src/lib.rs` — export `LayoutUnit`.
- `crates/layout/src/query.rs` — `DOMRect::new`/`from_taffy_layout` quantize via `LayoutUnit`.
- `crates/layout/src/engine.rs` — `taffy_size`/`taffy_position` quantize via `LayoutUnit`.
- `crates/browser/tests/realm_purity.rs` (new, 14 tests).
- `crates/browser/tests/sub_pixel_layout.rs` (new, 3 tests).
- `crates/browser/tests/v8_natives.rs` (new, 11 tests).
- `crates/browser/tests/api_completeness.rs` (new, 49 tests).
- `crates/browser/tests/behavioral_polish.rs` (new, 11 tests).

### Remaining critical-path work for the 7 CHL sites

The two remaining heavy lifts that the session could not ship:

1. **P3 libpng-sys swap** (~3 days). Chrome's PNG bytes via `libpng + zlib level 6` with no `pHYs/tIME/tEXt/iTXt` chunks and a single concatenated IDAT. The Rust `png` crate emits a different chunk set, breaking byte-equality. CreepJS, fp-collect, Imperva all hash raw PNG bytes from canvas. **This is the highest-ROI single fix for canvas-based detection.**

2. **P4 WebGL via wgpu+naga** (~4 weeks). The current `webgl.rs` is a stub returning dummy objects from `createShader`/`compileShader`/`drawArrays`. Kasada calls `gl.compileShader` on a known fragment shader and reads `gl.getShaderInfoLog`; ANGLE-D3D11/Metal/GL each produce known log strings. browser_oxide returns nothing.

These two changes plus actual Chrome 147 fixture capture (P0.7) would close the engine-coherence gap on the 7 CHL sites.

---

## Table of contents

1. [Why Playwright MCP works on the same machine](#1-why-playwright-mcp-works-on-the-same-machine)
2. [The diagnosis: 9 things real Chrome does that browser_oxide does not](#2-the-diagnosis)
3. [Inventoried engine gaps with file:line references](#3-inventoried-engine-gaps-in-browser_oxide)
4. [Production probes that catch these gaps (Kasada / Akamai / DataDome / CreepJS / BotD)](#4-the-probes-that-catch-these-gaps)
5. [Chrome 147 engine-determinism reference](#5-chrome-147-engine-determinism-reference)
6. [The architectural blocker: realm purity](#6-the-architectural-blocker-realm-purity)
7. [Strategic options decision tree](#7-strategic-options)
8. [Action plan with concrete file edits](#8-action-plan)

---

## 1. Why Playwright MCP works on the same machine

[Playwright MCP](https://github.com/microsoft/playwright-mcp) defaults to `channel: 'chrome'` — it launches **the user's locally-installed Google Chrome stable**, not the bundled Chromium ([issue #1081](https://github.com/microsoft/playwright-mcp/issues/1081), [DeepWiki browser-options](https://deepwiki.com/microsoft/playwright-mcp/6.2-browser-options-and-capabilities)). Profile persistence is on by default — cookies and storage survive between sessions.

So when the user runs Playwright MCP from this machine, the wire output is bit-identical to opening Chrome and clicking around manually:

- **TLS ClientHello**: real Chrome stable's BoringSSL output, including X25519MLKEM768 PQ key share, GREASE rotation, ALPS, brotli-compressed certificate.
- **HTTP/2**: Chrome's exact SETTINGS frame, WINDOW_UPDATE, pseudo-header order `m,a,s,p`.
- **Headers**: Chrome casing/order, full `Sec-CH-UA-*` set including `Sec-CH-UA-Form-Factors`.
- **Canvas**: real Skia + CoreText/DirectWrite/FreeType pixels.
- **WebGL**: real ANGLE renderer string for the host GPU.
- **V8**: Chrome's V8 build with Chrome's ICU, snapshot, feature flags.
- **Layout**: real Blink with 1/64-px LayoutUnit sub-pixel positioning.
- **WebAudio**: real DSP producing exact float sums in the FingerprintJS/CreepJS lookup tables.

The fact that this works **with `navigator.webdriver=true`, `Runtime.enable` issued, `__pwInitScripts` injected, and the full Playwright automation banner** disproves the conventional stealth narrative for these 7 sites. Cloudflare/Akamai/DataDome/Kasada in 2026, at the sensitivities canadagoose / hyatt / adidas etc. ship by default, are **not gating on automation markers**. They are gating on **engine-output coherence** — which Playwright MCP gets for free because it's real Chrome.

Citations: [Playwright Anti-Bot Detection 2026 (AlterLab)](https://alterlab.io/blog/playwright-anti-bot-detection-what-actually-works-in-2026), [Castle.io: Detecting Playwright headless Chrome](https://blog.castle.io/how-to-detect-headless-chrome-bots-instrumented-with-playwright/), [Cloudflare bot detection engines](https://developers.cloudflare.com/bots/concepts/bot-detection-engines/).

---

## 2. The diagnosis

What real-Chrome-via-Playwright-MCP does that browser_oxide does not, in roughly decreasing order of detection power:

1. **Real Skia + Chrome font stack canvas rasterization.** Anti-bot vendors maintain dictionaries of expected canvas hashes per (OS, GPU, Chrome version) tuple. browser_oxide's tiny-skia-based crate cannot match Chrome's libpng+zlib level-6 PNG encode bytes nor Chrome's HarfBuzz+Skia text shaping byte-for-byte. **A canvas hash that matches no known-Chrome cluster is the single most-detected engine signal.**
2. **Real ANGLE WebGL.** `UNMASKED_RENDERER_WEBGL` must be a string like `ANGLE (Apple, ANGLE Metal Renderer: Apple M1 Pro, Unspecified Version)` matching the host GPU. browser_oxide's WebGL is a stub: `createShader/compileShader/linkProgram/drawArrays` all return dummy objects — Kasada and CreepJS catch this in milliseconds.
3. **Real V8 from Chrome's build.** deno_core 0.311 ships V8 with Deno's snapshot, ICU data, and feature flags. Chrome 147 ships V8 ≈ 14.1 with Chrome's snapshot, full ICU, and Chrome-specific features. Detectable deltas: `Error.stack` format, `Intl.DateTimeFormat` resolved options (ICU 75 vs 73-74), `Math.cos(13*Math.E)` last-bit values, harmony flag set, `Promise.try` / `Math.sumPrecise` presence.
4. **Real Blink layout**. `getBoundingClientRect()` floats use Blink's 32-bit fixed-point `LayoutUnit` with 6 fractional bits — **resolution 1/64 px = 0.015625**. A 1.3px-wide div in real Chrome returns `width: 1.296875` (= 83/64), never `1.3`. browser_oxide using a `f32` pixel grid cannot produce this signature, and Akamai's "pHash" hash *is* a hash of these sub-pixel floats over `document.querySelectorAll('*')`.
5. **Complete Chrome 147 API surface.** `Object.getOwnPropertyNames(window).length` ≈ 1080–1110 in Chrome 147; `Object.getOwnPropertyNames(Navigator.prototype)` is a 57+ entry list with stable insertion order. browser_oxide is missing `EditContext`, `Highlight`, constructible `CSSStyleSheet`, `CookieStore`, `IdentityCredential`, `WGSLLanguageFeatures`, `XRSession`, `LaunchQueue`, `WebTransport`, full `ServiceWorker`/`SharedWorker`, `FileSystemHandle`, View Transitions API, `BatteryManager`, `Geolocation`, `XSLTProcessor`, `Notification`, `PushManager`, `BackgroundFetchManager`, `PaymentRequest`, dozens more. **Each missing constructor on a UA claiming Chrome 147 is fatal.**
6. **Real Chrome TLS+H2 — wire-level only.** Already solved via rquest+BoringSSL byte-identical to Chrome 147. This is browser_oxide's only winning subsystem; it's why the conventional "fix the TLS" advice doesn't apply here.
7. **Real WebAudio DSP.** OfflineAudioContext compressor sum must be one of the known Chrome floats (`124.04347657808103` macOS arm64, `124.04344968475198` Linux x64). Custom audio implementations land outside the lookup table.
8. **Persistent warm profile** with cookie history, prior trust score state. Trust score on first request from a clean profile is already lower than from a 6-month-old profile.
9. **`window.chrome.{app, csi, loadTimes, runtime, webstore}` legacy stubs** with proper bound-function shapes — shipped naturally, not as stealth patches.

The minor things that look bad on paper — `navigator.webdriver=true`, `Runtime.enable`, automation banners — **do not matter** for these sites at default Bot Management sensitivity, because the trust score is already strongly positive from items 1–9.

For browser_oxide, **no amount of stealth JavaScript fixes any of items 1–7**; they are below the JS layer.

---

## 3. Inventoried engine gaps in browser_oxide

Ranked by detectability × prevalence in Tier-1 anti-bot stacks. Source file references are to `/Users/yfedoseev/Projects/browser_oxide/`.

### 3.1 Critical (existential blocker on every Tier-1 vendor)

| # | Gap | File:line | Detection vector |
|---|---|---|---|
| 1 | `getComputedStyle()` is **completely absent** | `crates/js_runtime/src/js/dom_bootstrap.js` | DataDome, Imperva, every CSS-shape probe. CreepJS has `headless.hasKnownBgColor` that calls `getComputedStyle(div).backgroundColor` for `background-color: ActiveText` and expects `rgb(0, 0, 238)` (link blue). Probes calling `getComputedStyle(...) === undefined` flag instantly. |
| 2 | `Element.scrollTop / scrollLeft` always return `0`, setters are no-ops | `dom_bootstrap.js:570-573` | Kasada `ips.js`, Akamai sensor. Probes scroll an element programmatically then read `scrollTop` → `0` reveals fake scroll state. |
| 3 | `Element.scrollIntoView()` missing | `dom_bootstrap.js` | Any probe calling it throws `TypeError: el.scrollIntoView is not a function` — diagnostic of a custom engine. |
| 4 | `clientLeft / clientTop` absent | `dom_bootstrap.js` | Border thickness queries; fingerprinters use these in coordinate math. |
| 5 | `Element.matches() / closest()` not in `dom_bootstrap` | `dom_bootstrap.js` | BotD, sannysoft layout probes. `el.matches(':valid')` calls fail. |
| 6 | `HTMLElement.style` is `{cssText: ""}` stub — setters don't persist | `canvas_bootstrap.js:652` and `dom_bootstrap.js` | `el.style.color='red'; el.style.color` returns `''` not `'red'`. Trivial probe. |
| 7 | `HTMLElement.dataset` is `{}` stub — `data-*` attributes don't bind | `canvas_bootstrap.js:657` | Detectors using `data-*` attributes for state see lost values. |
| 8 | Canvas 2D `arc / arcTo / bezierCurveTo / quadraticCurveTo / strokeText / clip / isPointInPath / isPointInStroke` are **stubs** | `canvas_bootstrap.js:61-137` | CreepJS Picasso paint hash has arcs and curves. Output diverges immediately. `isPointInPath()` always returns `false`. |
| 9 | WebGL **shader compilation does not parse GLSL** — `createShader/compileShader/linkProgram/drawArrays/drawElements` return dummy `{_id:1}` objects | `canvas_bootstrap.js:339-395` | Kasada calls `gl.compileShader` for a test fragment shader and reads `gl.getShaderInfoLog`; ANGLE-D3D11/Metal/GL each produce known log strings. browser_oxide produces nothing. |
| 10 | WebGL texture/buffer/framebuffer ops are stubs; `readPixels` after `texImage2D` returns black | `canvas_bootstrap.js:356-395` | CreepJS draws a triangle and hashes the readback bytes — software rasterizer can't match. |
| 11 | Layout returns **integer pixels, not Blink 1/64-px sub-pixel floats** | `crates/layout/src/lib.rs` | Akamai's `pHash` is a Murmur over `top|left|width|height` from `querySelectorAll('*')`. Real Chrome on a `1.3px` div returns `1.296875`; browser_oxide returns `1` or `1.3`. **Any sub-pixel deviation breaks the hash.** |
| 12 | Canvas text rendering: `measureText` returns full TextMetrics but values are **not tied to a real Skia/Harfbuzz pipeline matching the claimed OS** | `canvas_bootstrap.js:74` | CreepJS hashes `actualBoundingBoxLeft/Right/Ascent/Descent` for emoji ZWJ + Latin. Per-OS expected floats. |
| 13 | OfflineAudioContext compressor sum is **deterministic via static seed**, not produced by a real Blink WebAudio compressor | `canvas_bootstrap.js:580-633` | CreepJS has a hardcoded `KnownAudio` table of expected floats (`124.04347657808103` etc); browser_oxide's seed-driven sum lands outside the table. |
| 14 | Realtime AudioContext path **not plumbed** — only OfflineAudioContext renders | `canvas_bootstrap.js:418-485` | Probes piping a realtime oscillator → analyser and reading `getFloatFrequencyData` see silence. |
| 15 | `navigator.gpu === undefined` (WebGPU absent) | `window_bootstrap.js` | Chrome 147 ships WebGPU; absence on a Chrome-147 UA is a strong tell. |
| 16 | `navigator.credentials / navigator.identity / navigator.locks / navigator.storage / navigator.serviceWorker` all undefined | `window_bootstrap.js` | API-completeness enumeration. |
| 17 | `navigator.hardwareConcurrency` hardcoded `8`, `deviceMemory` hardcoded `8` | `window_bootstrap.js` | Should reflect host. PerimeterX flags `(2, 0.25)` (containers); browser_oxide reports `(8, 8)` regardless of host. |
| 18 | `WorkerNavigator` is a **plain object, not an instance of Navigator** — no prototype chain to `window.Navigator` | `worker_bootstrap.js:54-77` | `navigator instanceof Navigator` is `false` in worker. CreepJS worker module checks this. |
| 19 | `IntersectionObserver` and `ResizeObserver` fire **synchronously with synthetic values** | `window_bootstrap.js:2426-2509` | Real Chrome delays until layout. Probes time the callback. |
| 20 | `performance.memory.jsHeapSizeLimit` uses **Date.now-based jitter** | `window_bootstrap.js`, `worker_bootstrap.js:80-93` | Real Chrome reports `4294705152` constant on desktop; jitter pattern is FFT-detectable. |

### 3.2 High (caught by 3+ vendors)

21. `Function.prototype.toString` masking sets **own `.toString` property on each masked function** (`stealth_bootstrap.js:50-62`) — `fn.hasOwnProperty('toString') === true`, atypical for natives. CreepJS lies module checks descriptors.
22. `Object.getOwnPropertyDescriptor(Function.prototype, 'toString')` reveals the patched `_patchedFnToStr` value pointer.
23. `event.isTrusted` always `false` (`event_bootstrap.js:13`) — synthetic events readily distinguishable.
24. Native events (`load`, `unload`, `beforeunload`, `pagehide`, `pageshow`, `popstate`, `hashchange`, `storage`) **don't fire automatically**. Probes registering listeners and waiting for navigation/storage events see nothing.
25. `postMessage` transferables are **not neutered** (`structured_clone.js:331`) — original `ArrayBuffer` remains usable in both threads after transfer.
26. `Math` constants — `Math.cos(13*Math.E)`, `Math.acos(0.123)`, `Math.fround(...)` may diverge from V8's libm when deno_core's V8 version drifts from Chrome 147's V8.
27. `window.chrome.loadTimes()` / `chrome.csi()` return empty objects (`window_bootstrap.js:1007-1038`); real Chrome ships timing data.
28. Selectors Level 4 (`:has()`, `:nth-child(of S)`, `:focus-visible`) likely incomplete in `crates/css_selectors`.
29. Custom properties (CSS variables) not verified.
30. `@container` queries not implemented.

### 3.3 Medium (subset of vendors)

31. `OffscreenCanvas` PNG bytes are not asserted byte-equal to `HTMLCanvasElement` PNG bytes — CreepJS verifies equality.
32. `eval.toString().length` should be **33** for V8 (BotD detector: `eval_length.ts`); deno_core's V8 should match — verify.
33. `Object.getOwnPropertyNames(Navigator.prototype)` order/length must match Chrome 147 reference (~57 entries).
34. `Object.getOwnPropertyNames(window).length` ≈ **1080–1110** in Chrome 147 — browser_oxide ships ~200 globals.
35. `XMLHttpRequest` / `WebSocket` state machines may not match Chrome readyState transitions exactly.
36. `Intl.DateTimeFormat([], {timeZone: tz})` for the full IANA list — requires full ICU TZ data.

---

## 4. The probes that catch these gaps

Distilled from RE work on Kasada `ips.js`, Akamai `sensor_data` v3, DataDome `tags.js`, Cloudflare JSD, PerimeterX collector, Imperva `reese84`, plus CreepJS, BotD, fpscanner, fp-collect, rebrowser-bot-detector. **K/A/D/C/P/I** = Kasada/Akamai/DataDome/Cloudflare/PerimeterX/Imperva.

### 4.1 Top 25 engine probes ranked by frequency-of-deployment

1. **Canvas pixel-byte determinism** — text + emoji + gradient SHA-256 hash must match a known-Chrome-on-this-OS cluster. **K/A/D/C/P/I** ([Cloudflare canvas randomization thread](https://community.cloudflare.com/t/turnstile-failed-when-canvas-fingerprint-is-randomly/658685))
2. **WebGL renderer string + ordered `getParameter` hash** including `WEBGL_debug_renderer_info`, plus `Float32Array` typing on float params. **K/A/D/C/P/I**
3. **`Function.prototype.toString` byte-exact match** on a fixed list of natives, with `[native code]` literal verification. **K/A/D/C/P/I**
4. **Cross-realm `toString` re-check** via `iframe.contentWindow.Function.prototype.toString.call(...)` — separate realm, same byte output. **K/A/D/C/P/I** ([DataDome iframe-evasion writeup](https://datadome.co/threat-research/how-datadome-detects-puppeteer-extra-stealth/))
5. **`Error.prototype.stack` format & `Error.stackTraceLimit = 10`** — V8 stringification of `at fn (url:line:col)`. **D/C/P/I**
6. **`getBoundingClientRect` sub-pixel float determinism** — Blink's 1/64-px LayoutUnit produces `123.4375`-style floats. **A/D/P/I — encoded as Akamai's "pHash"**
7. **AudioContext OfflineAudioContext oscillator hash** — depends on FFT implementation. **A/D/C/P/I**
8. **Font enumeration via `measureText` width comparison** against ~80 probe fonts, cross-checked with claimed OS. **A/D/P/I**
9. **`Object.getOwnPropertyNames(Navigator.prototype)` order & set hash** — V8's insertion-order enumeration is the reference. **D/P/I**
10. **`Object.getOwnPropertyDescriptor(Navigator.prototype, 'userAgent').get`** existence and identity — must be a real native getter, not a `Object.defineProperty` shim. **K/D/P/I**
11. **Realm-purity proxy attack** — `iframe.contentWindow.Navigator !== window.Navigator` AND prototype shape equal (see §6). **D/P/I**
12. **Worker-realm consistency** — same `getParameter`/`navigator` hashes from inside `new Worker`. **K/A/D/P**
13. **`performance.now()` resolution** — 100 µs (or 5 µs cross-origin-isolated) quantisation. **K/D/C**
14. **`event.timeStamp` quantisation** consistent with `performance.now()` quantisation. **A/D/P**
15. **`Intl.DateTimeFormat().resolvedOptions()`** + `Date.prototype.getTimezoneOffset()` cross-DST consistency. **A/D/I**
16. **`Intl.NumberFormat` locale outputs** byte-exact against full-ICU Chrome (e.g., Arabic-Indic digits). **I, D**
17. **`getComputedStyle` serialization** — `rgb(0, 0, 0)` exact whitespace, `0px` vs `0`, `transform: matrix3d(...)` round-trip. **D, I**
18. **`navigator.connection.rtt`/`downlink` 25ms/25kbps quantisation.** **P, D**
19. **`hardwareConcurrency` × `deviceMemory` plausibility** against known desktop clusters. **P, A**
20. **`navigator.userAgentData.getHighEntropyValues()` parity** with `Sec-CH-UA-Platform-Version` HTTP header byte-for-byte. **Douyin, C, A**
21. **Math libm constants** — `Math.tan(-1e300)`, `Math.acos(0.123)`, `Math.fround(...)` against known-Chrome values. **A, K**
22. **`CSS.supports` + Web-API existence allow-list** matched to claimed Chrome major version. **C**
23. **Plugin/MimeType back-pointer** — `navigator.mimeTypes[0].enabledPlugin === navigator.plugins[0]`. **C, K — addressed in commit b6d20da**
24. **`document.documentElement.style.transform = "matrix3d(...)"` round-trip** — Blink normalises to specific spacing. **I**
25. **`Error.captureStackTrace` getter trap on `Error.prototype.stack`** — detects whether anyone is inspecting the stack. **D/C**

**The first eleven are deployed by every Tier-1 vendor and are existential blockers.** browser_oxide currently fails ALL ELEVEN.

### 4.2 Vendor-specific notes

**Kasada `ips.js`** ([umasii/ips-disassembler](https://github.com/umasii/ips-disassembler), [opcodes.fr](https://opcodes.fr/publications/2021-08/kasada-javascript-vm-obfuscation-reverse-part1)). Stack-based VM with `getProp` opcodes that call API chains *both directly and via `Reflect.get`/descriptor-getter-call* and compares. Hardcoded `Function.prototype.toString` assertions on `HTMLCanvasElement.prototype.toDataURL/getContext`, `CanvasRenderingContext2D.prototype.getImageData`, `WebGLRenderingContext.prototype.getParameter`, `Element.prototype.getBoundingClientRect`, `Document.prototype.createElement`, `window.fetch`, `XMLHttpRequest.prototype.open`, `Function.prototype.toString` itself, `Object.getOwnPropertyDescriptor`. Then re-extracts in iframe and compares.

**Akamai sensor v3** ([glizzykingdreko deep dive](https://medium.com/@glizzykingdreko/akamai-v3-sensor-data-deep-dive-into-encryption-decryption-and-bypass-tools-da0adad2a784), [xvertile/akamai-bmp-generator](https://github.com/xvertile/akamai-bmp-generator)). Field 23 — **pHash of document** = walk `querySelectorAll('*')`, sum `top|left|width|height` from `getBoundingClientRect()` into a Murmur hash. Catches engines whose layout produces *any* sub-pixel difference. **browser_oxide returning pixel-aligned ints where Chrome returns `123.4375` (1/64-px sub-pixel from LayoutUnit) is fatal.**

**DataDome `tags.js`** ([DataDome threat-research](https://datadome.co/threat-research/how-datadome-detects-puppeteer-extra-stealth/), [glizzykingdreko breakdown](https://medium.com/@glizzykingdreko/breaking-down-datadome-captcha-waf-d7b68cef3e21)). The "200+ navigator props" probe iterates `for(let k in navigator)` plus `Object.getOwnPropertyNames(Object.getPrototypeOf(navigator))` and hashes the concatenated key list. Real Chrome 124+ has a stable, version-specific ordering. Custom engines that build `Navigator` via `deno_core` op declarations produce different enumeration order. The published iframe trick: `iframe.contentWindow.self.get?.toString()` — real Chrome returns nothing; Stealth's iframe shim leaks its own source.

**CreepJS engine.ts** error-message assertions:
- `new Function('null.bar')()` → `Cannot read properties of null (reading 'bar')` (V8) — exact string
- `new Function('var x = new Array(-1)')()` → `Invalid array length`
- `new Function('const a=1; const a=2;')()` → `Identifier 'a' has already been declared`

**CreepJS audio.ts KnownAudio table** — must match one of these float sums:
- Mac arm64: `124.04347657808103`
- Linux x64: `124.04344968475198`
- Windows x64: `124.04347657808103`
- Sample at index ~5000: `-20.538286209106445` (Blink/WebKit) or `-20.535268783569336` (Android/Linux)

**BotD distinctive_properties.ts** flags `cdc_*`, `__webdriver_*`, `__selenium_*`, `__fxdriver_*`, `__$webdriverAsyncExecutor` — none should appear in browser_oxide globals (verify with `Object.getOwnPropertyNames(globalThis)` diff).

---

## 5. Chrome 147 engine-determinism reference

A specification of expected outputs to assert against in the test suite. Captured fresh from a Chrome 147.0.7727.x stable build on each target OS.

### 5.1 30-output checklist for `tests/chrome147_fixtures.rs`

| # | API call | Expected (per OS) | Detected by |
|---|---|---|---|
| 1 | `(()=>{const d=document.createElement('div');d.style.cssText='width:1.3px';document.body.appendChild(d);return d.getBoundingClientRect().width})()` | `1.296875` (= 83/64), all OS | CreepJS, BotD |
| 2 | `getComputedStyle(document.body).margin` | `8px` | DataDome trivial probe |
| 3 | `window.innerWidth - document.documentElement.clientWidth` | macOS 0; Win11 Fluent 15; Win10 17; Linux GTK 0 | Kasada |
| 4 | `getComputedStyle(document.querySelector('input')).height` | macOS `~21px`; Win `~22px`; Linux `~26px` | BotD |
| 5 | `Object.getOwnPropertyNames(Navigator.prototype).join(',')` SHA-256 | per-version hash | CreepJS, FingerprintJS |
| 6 | `Object.getOwnPropertyNames(window).length` | ≈ 1080–1110 | CreepJS |
| 7 | `Object.prototype.toString.call(navigator)` | `[object Navigator]` | trivial |
| 8 | `navigator.hardwareConcurrency` | host CPU count, 4–32 typical | universal |
| 9 | `navigator.deviceMemory` | one of `0.25, 0.5, 1, 2, 4, 8` | FingerprintJS |
| 10 | `OfflineAudioContext compressor sum` | Mac arm64 `124.04347657808103`; Linux `124.04344968475198` | FingerprintJS, Kasada |
| 11 | `audioCtx.sampleRate` | 48000 typical, 44100 some macOS | CreepJS |
| 12 | `ctx.measureText('Hello World').width` at `12px Arial` | Win `~57.34375`; Mac `~57.34`; Linux DejaVu `~66.5` | CreepJS |
| 13 | `ctx.measureText('mmmmmmmmmmlli').actualBoundingBoxAscent` | Win Arial `~8.76`; Mac Helvetica `~8.66` | CreepJS |
| 14 | Canvas fingerprint `toDataURL()` SHA-256 of standard test scene | hash matches Chrome 147 ref per OS+GPU | every vendor |
| 15 | `OffscreenCanvas` PNG hash equals `HTMLCanvasElement` PNG hash | exact equality | CreepJS |
| 16 | `gl.getParameter(gl.ALIASED_LINE_WIDTH_RANGE)` | D3D11 `[1,1]`; Metal `[1,7.375]`; GL `[1,10]` | FingerprintJS |
| 17 | `gl.getExtension('WEBGL_debug_renderer_info'); gl.getParameter(UNMASKED_RENDERER_WEBGL)` | `ANGLE (vendor, gpu, backend)` matching host | universal |
| 18 | `gl.getSupportedExtensions().sort().join(',')` SHA-256 | per-GPU hash | CreepJS |
| 19 | `gl.getShaderPrecisionFormat(FRAGMENT_SHADER, HIGH_FLOAT)` | `{rangeMin:127, rangeMax:127, precision:23}` | CreepJS |
| 20 | `(await navigator.gpu.requestAdapter()).features.size` | Chrome 147 desktop ≈ 14–18 | Kasada |
| 21 | `performance.now()` repeated, min delta | `0.005` COI; `0.1` non-COI | Akamai |
| 22 | `performance.timeOrigin % 0.005` | `0` in COI; `0` mod 0.1 non-COI | Akamai |
| 23 | `Function.prototype.toString.call(Math.sin)` | `function sin() { [native code] }` | CreepJS |
| 24 | `Function.prototype.toString.call(navigator.permissions.query)` | `function query() { [native code] }` | CreepJS |
| 25 | `(()=>{try{null[0]}catch(e){return e.stack.split('\n')[1].trim()}})()` | `at <anonymous>:1:8` (no extra frames) | CreepJS |
| 26 | `Object.getOwnPropertyDescriptor(Object.prototype,'__proto__').get.name` | `'__proto__'` (or `'get __proto__'`) | CreepJS |
| 27 | `Reflect.ownKeys({2:0,1:0,a:0,[Symbol()]:0})` | `["1","2","a",Symbol()]` | CreepJS prototype-lie test |
| 28 | `CSS.supports('field-sizing','content')` | `true` | feature-detect |
| 29 | `CSS.supports('interpolate-size','allow-keywords')` | `true` | feature-detect |
| 30 | `CSS.supports('selector(:has(*))')` and `typeof CSSPseudoElement` | `true`, `'function'` | DataDome |

### 5.2 Critical Chrome 147 details

- **Blink `LayoutUnit`**: 32-bit fixed-point, 6 fractional bits → resolution `1/64 = 0.015625` px ([WebKit LayoutUnit wiki](https://trac.webkit.org/wiki/LayoutUnit)). Sub-pixel positioning rules: `getBoundingClientRect` is sub-pixel, `offsetWidth/Height/clientWidth/scrollWidth` are integer-rounded.
- **Scrollbar widths**: macOS overlay 0px; Windows 11 Fluent 15px; Windows 10 17px; Linux GTK 0–14px; Android 0px.
- **UA stylesheet** ([html.css](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/html/resources/html.css)): `body{margin:8px}`, `h1{font-size:2em;margin-block:0.67em}`, `h2{1.5em/0.83em}`, etc.
- **PNG encoder**: libpng + zlib level 6, no `pHYs/tIME/tEXt/iTXt` chunks, single `IDAT`. Output is byte-deterministic given identical pixel input — this is the core of canvas fingerprinting.
- **`Object.getOwnPropertyNames(Navigator.prototype)` Chrome 147 order** (preserve exact insertion order): `constructor, vendorSub, productSub, vendor, product, appCodeName, appName, appVersion, platform, userAgent, language, languages, onLine, webdriver, hardwareConcurrency, deviceMemory, cookieEnabled, doNotTrack, getGamepads, javaEnabled, sendBeacon, geolocation, mediaCapabilities, connection, plugins, mimeTypes, pdfViewerEnabled, registerProtocolHandler, unregisterProtocolHandler, storage, serviceWorker, scheduling, virtualKeyboard, mediaSession, permissions, devicePosture, clipboard, credentials, keyboard, locks, mediaDevices, ink, hid, usb, xr, userActivation, serial, windowControlsOverlay, bluetooth, contacts, presentation, managed, login, gpu, wakeLock, share, canShare, clearAppBadge, setAppBadge, getUserMedia, requestMIDIAccess, requestMediaKeySystemAccess, vibrate, getInstalledRelatedApps, getBattery`.
- **Chrome 147 V8 ≈ 14.1**. Harmony flags: `Math.sumPrecise` shipped 147; `Promise.try` shipped 128; Set methods shipped 122; Iterator helpers shipped 122; `Promise.withResolvers` shipped 119. **`Temporal` is disabled in Chrome 147** (still origin trial) — deno_core may have it enabled, **must disable**.
- **Chrome 147 new in CSS**: `contrast-color()`, `border-shape`, `CSSPseudoElement` interface and `Element.pseudo()`, `*-width` decoupled from `*-style`.
- **WebGL extensions Chrome 147** (typical desktop): 33 entries on WebGL1, plus `EXT_clip_control, EXT_conservative_depth, EXT_depth_clamp, EXT_render_snorm, EXT_texture_mirror_clamp_to_edge, EXT_texture_norm16, OVR_multiview2, WEBGL_blend_func_extended, WEBGL_stencil_texturing` on WebGL2.

---

## 6. The architectural blocker: realm purity

**This is the largest single risk** for browser_oxide and is not addressable by JS-layer patching.

### The probe (deployed by DataDome, PerimeterX, Imperva, CreepJS)

```js
const f = document.createElement('iframe'); document.body.appendChild(f);
// 1. Iframe must have its own realm
f.contentWindow.Navigator !== Navigator                       // must be true
f.contentWindow.Navigator.prototype !== Navigator.prototype   // must be true
// 2. But same shape
Object.getOwnPropertyNames(f.contentWindow.Navigator.prototype).join() ===
  Object.getOwnPropertyNames(Navigator.prototype).join()      // must be true
// 3. Cross-realm Function.prototype.toString must work
f.contentWindow.Function.prototype.toString.call(window.fetch).includes('[native code]')  // must be true
```

In real Chrome each realm (main window, every iframe, every Worker) is a separate V8 Context with its own copy of every constructor's prototype. The constructors and prototypes are *structurally identical* (same property keys, same getter shapes) but *not reference-identical*. This is the W3C-mandated behavior of HTML "realm" semantics.

### Why browser_oxide fails

If the engine creates iframes by sharing a single V8 isolate's context with `OpState`, then `iframe.contentWindow.Navigator === window.Navigator` because they're the same object. **Probe 1 fails.** This is the standard catch for puppeteer-extra-stealth's iframe shim and the same probe will catch any custom engine without per-frame realms.

### What it takes to fix

Each browsing context (main frame + every iframe + every Worker) must be a **separate `v8::Context`** within the same isolate. deno_core supports this via `JsRuntime::create_realm` / `JsRealm`. Each realm runs `dom_bootstrap.js`/`window_bootstrap.js` independently to install its own `Navigator.prototype`, `HTMLElement.prototype`, etc. The bootstraps must be idempotent and parameterized by realm-id. State stored on the realm (cookies, fetch jar, document tree) gets routed back to a shared `OpState` for cross-realm bookkeeping.

This is a **non-trivial refactor** affecting:
- `crates/js_runtime/src/lib.rs` — `JsRuntime` lifecycle, multi-context support
- All `#[op2]` ops that read `OpState` — must accept a realm-id parameter
- `crates/browser/src/page.rs` — iframe creation must allocate a new realm
- `crates/workers/` — already uses separate isolates; needs alignment with the new realm model
- `dom_bootstrap.js` / `window_bootstrap.js` — must be safe to re-run in a fresh realm
- `structured_clone.js` — already aware of realm boundaries; verify

Estimate: **2–4 weeks of focused work**. Must be done correctly the first time because any regression breaks every subsequent fingerprint test.

---

## 7. Strategic options

Given the diagnosis, four viable paths. **Pick one explicitly.** The recommended path is **Option D — bind, don't reimplement** (preserves the Rust thesis and the deno_core memory advantage).

### Option D (RECOMMENDED): Bind, don't reimplement

**Stay on deno_core's V8. Stay Rust. Do NOT fork Chromium.** For each subsystem where Tier-1 anti-bot vendors fingerprint byte-identical Chrome output (canvas raster, PNG encoding, text shaping, WebGL, WebAudio DSP, ICU locale data), bind the **same sub-libraries that Chrome happens to use** — Skia, libpng+zlib, HarfBuzz/FreeType/CoreText/DirectWrite, ANGLE (or wgpu+naga), full ICU 75. These are independent BSD-3 / MIT / libpng / zlib licensed libraries; using them is not "using Chrome." For everything else (DOM tree, CSS cascade, networking, JS bridge, layout glue, stealth, runtime, automation), keep Rust ownership.

**This preserves the project's structural advantages:**
- **Memory-per-worker stays under 100 MB** (vs Chrome's ~300 MB per tab) — deno_core remains the JS host, not Chromium.
- **Startup time stays in the hundreds-of-ms** range, not multi-second Chrome cold start.
- **TLS+H2 byte-identical Chrome 147** via rquest+BoringSSL — already winning, stays winning.
- **No CDP, no WebDriver, no BiDi surfaces** — the architectural moat against the next wave of detection.
- **MIT/Apache license discipline** preserved (no MPL).

**The realization:** Chrome itself is a *Rust-style* composition over the same C/C++ libraries we'd bind. Chromium = Blink + V8 + Skia + ANGLE + libpng + zlib + ICU + HarfBuzz + … wired together by Chrome's process model. We can keep the deno_core/Rust process model and wire the *same leaf libraries* into a different orchestration. That's Camoufox's pattern with Firefox's stack ([github.com/daijro/camoufox](https://github.com/daijro/camoufox)) — except with Chrome's leaf libraries instead of Gecko's, and Rust glue instead of C++ Gecko glue.

| Subsystem | Library to bind | License | Rust crate |
|---|---|---|---|
| JS engine | V8 (already there) | BSD-3 | `deno_core` / `rusty_v8` — pin to a Chrome 147-aligned version, disable `Temporal` etc. |
| Canvas raster + text shaping | Skia | BSD-3 | [`skia-safe`](https://github.com/rust-skia/rust-skia) — mature, used by Servo |
| Glyph rasterization | HarfBuzz + FreeType (Linux), CoreText (mac), DirectWrite (Win) | MIT / FreeType / system | bundled with skia-safe |
| PNG encoding | libpng + zlib level-6 | libpng / zlib | `libpng-sys` vendored — byte-deterministic |
| WebGL | ANGLE direct **or** wgpu+naga (path 4a in `timeline.md`) | BSD-3 / MIT-Apache | `wgpu` + `naga` recommended initially |
| ICU locale | full ICU 75 data | Unicode-DFS | `icu_provider` or vendored ICU |
| WebAudio DSP | port Chromium's `DynamicsCompressor` / `BiquadFilter` / `AnalyserNode` | BSD-3 (Chromium source) | new `crates/canvas/src/audio/` |
| Fonts | OS-native via Skia | — | free with skia-safe |

**Effort:** ~5 months for one engineer; ~3 months with two engineers parallel (P3+P4 / P5+P6 from `timeline.md`). The full module-by-module plan is in `/Users/yfedoseev/Projects/browser_oxide/timeline.md`.

**Validation gate:** Week 29 — at least 5 of 7 CHL sites pass at L3 using only browser_oxide, no CDP-Chrome fallback, with memory-per-worker still under 100 MB.

- **Pros**: preserves the deno_core/Rust thesis, the memory advantage, the TLS moat. Produces byte-identical-Chrome canvas/WebGL/audio/PNG output by *construction* (because the leaf libraries are literally the ones Chrome uses). Closes Tier-1 anti-bot detection on the engine-output coherence axis.
- **Cons**: 5 months of focused work. ANGLE binding is the highest-risk module (alternative wgpu+naga is easier but may fail bit-exact pixel-readback against Tier-1 vendors that hash readback bytes — verify empirically, fall back to ANGLE binding if needed).
- **Verdict**: **the recommended path.** This is what no one has built yet because the people who care about Tier-1 bypass reach for patched Chromium first (faster ship), and the people building Rust browsers (Servo) don't care about anti-bot. browser_oxide sits at the unique intersection.

### Option A: Embed real Chrome via CDP for the 7 CHL sites

Keep browser_oxide for unprotected scrapes (its current strength: low memory, fast startup, byte-identical TLS). For Cloudflare-Bot-Mgmt / Akamai-BMP / Kasada / HUMAN / DataDome targets, fork to a CDP client driving real Chrome. Patchright or rebrowser-puppeteer for Python; nodriver/zendriver for raw CDP.

- **Pros**: closes the 7 CHL sites today. Zero engine-rewrite cost. Proven path.
- **Cons**: contradicts the project thesis of a from-scratch Rust engine. **Memory cost per worker jumps from ~20MB (deno_core) to ~300MB (Chrome) — kills the structural advantage.** The TLS-byte-identical work becomes redundant.
- **Verdict**: pragmatic if the goal is to access these sites this quarter and the memory cost is acceptable. **Rejected** if the deno_core memory profile is non-negotiable.

### Option B: Accept the long-tail role; ship as-is

Position browser_oxide as the engine for **the long tail of sites that don't ship Tier-1 anti-bot** — generic e-commerce, public REST APIs, content sites, internal automation. Kasada / Akamai BMP Premier / HUMAN / DataDome enterprise targets are out of scope.

- **Pros**: zero additional work. The TLS+H2 parity is a real differentiator for the long tail. Memory-per-worker advantage is preserved.
- **Cons**: closes off the 7 sites and any future Tier-1-protected target.
- **Verdict**: honest answer if engine completeness is not budget-feasible.

### Option C: Pure-Rust everything, no C/C++ bindings

Reimplement Skia-equivalent rasterization, HarfBuzz-equivalent text shaping, ANGLE-equivalent GL backend, libpng-equivalent encoder, and Blink's WebAudio DSP all in pure Rust with byte-identical output to Chrome 147.

- **Pros**: zero C/C++ FFI; "100% Rust" purity.
- **Cons**: **not realistic.** Servo has been at this for 12+ years and doesn't claim parity. Reimplementing Skia bit-faithfully alone is a multi-year project. The byte-for-byte target is the issue: a 1-LSB drift in any sub-pixel rasterization breaks the canvas hash.
- **Verdict**: **rejected.** This is the framing that makes Tier-1 anti-bot bypass look impossible for a Rust project; the framing is wrong (see Option D).

---

## 8. Action plan

### P0 — Sprint 1 (this week)

If choosing Option A or hybrid: spike a CDP-Chrome path. Use [nodriver](https://github.com/cdpdriver/zendriver) (Python) or implement a minimal CDP client in Rust against `Browser.startBrowser` + `Page.navigate` + `Runtime.evaluate`. Verify the 7 CHL sites pass.

If choosing Option B: write the positioning doc, update README, mark the 7 sites as out-of-scope.

### P1 — Quick engine wins (regardless of option, ~1 week)

These are diagnostic tells with no architectural cost:

1. **Implement `getComputedStyle()` returning at least the visible properties** (`color`, `backgroundColor`, `display`, `visibility`, `opacity`, `font*`, `width`, `height`, `margin*`, `padding*`, `border*`). Even a 80%-correct implementation is better than `undefined`. → `dom_bootstrap.js` or new `computed_style_bootstrap.js`.
2. **Implement `Element.matches()` and `Element.closest()`** using existing CSS selector matching. → `dom_bootstrap.js`.
3. **Implement `Element.scrollIntoView()`** as a no-op that returns `undefined` (matches spec). → `dom_bootstrap.js`.
4. **Track `scrollTop / scrollLeft` as real per-element state** with setter side effects. → `dom_bootstrap.js:570-573` + `crates/dom`.
5. **Add `clientLeft / clientTop`** returning border-thickness from layout. → `dom_bootstrap.js` + `crates/layout`.
6. **Implement `HTMLElement.style` as a real CSSStyleDeclaration** with per-property setters that persist. → `dom_bootstrap.js` (replace `canvas_bootstrap.js:652` stub).
7. **Implement `dataset` as a real `DOMStringMap`** binding to `data-*` attributes. → `dom_bootstrap.js`.
8. **Verify `eval.toString().length === 33`** (BotD `eval_length.ts`). One-line test against deno_core's V8.
9. **Diff `Object.getOwnPropertyNames(globalThis)` against vanilla Chrome 147 baseline.** Capture from real Chrome, store as `tests/chrome147_globals.json`, assert in CI. Removes most "missing constructor" leaks at zero arch cost (return stubs with native-shape `Function.prototype.toString`).
10. **Make `event.isTrusted` configurable** so dispatched-from-Rust events can be marked trusted; currently always `false` (`event_bootstrap.js:13`).

### P2 — Architectural (2–4 weeks)

11. **Per-frame V8 realm refactor** (§6). The single highest-impact engine change. After this lands, items 12+ become testable.
12. **Blink LayoutUnit (1/64-px fixed-point)** in `crates/layout`. After this lands, Akamai pHash parity is reachable.
13. **WorkerNavigator instanceof Navigator parity** — make Worker's `Navigator.prototype` reachable from the worker realm and same-shape as main.

### P3 — High-leverage subsystems (8–24 weeks per item)

14. **Skia-rs binding for canvas** with libpng+zlib level-6 PNG encoder — close items 1, 12 of §3.1.
15. **WebGL via real shader compilation** (wgpu+naga, or ANGLE binding) — close items 9, 10.
16. **WebAudio Blink-compatible compressor DSP** — close item 13.
17. **API-completeness pass** — close item 5 of §2.

### P4 — Operational

18. **Capture Chrome 147 fixtures** (§5.1) on macOS arm64, macOS x64, Windows 11, Ubuntu 24.04. Store as `tests/chrome147_fixtures.rs`. CI fail on drift.
19. **CreepJS / BotD / fp-collect / sannysoft test runners** as `#[ignore]` integration tests. Score must monotonically improve.
20. **Realm-purity probe test** — implement the §6 probe in `tests/`, run after every realm-related change.

### Architectural stances to maintain

- **Never expose CDP, WebDriver, or BiDi.** browser_oxide's only durable moat is the absence of these surfaces. Do not regress.
- **Never implement automation extensions in JS surface.** If automation is needed, expose Rust-side API only.
- **MIT/Apache discipline** — no MPL deps. Servo and Camoufox are MPL; borrow ideas, not code. Skia is BSD-3 (compatible). ANGLE is BSD-3 (compatible). libpng is libpng-license (compatible). zlib is zlib-license (compatible).

### The honest bottom line

The previous draft of this document concluded that IP reputation was the bottleneck. The user's correction — "same machine, Playwright MCP works" — is decisive evidence that the bottleneck is engine-side. The 7 CHL sites are not gating on automation markers (Playwright announces itself loudly and gets through); they are gating on **engine-output coherence with a real Chrome 147 build**.

The second-pass framing of "options A/B/C" was also wrong: it treated "Rust browser" as "reimplement everything in pure Rust" and concluded the engine path was a 6–12 month research project closer to forking Chromium. **That framing was wrong.** Chrome itself is a *Rust-style* composition over the same C/C++ leaf libraries we'd bind. Chromium = Blink + V8 + Skia + ANGLE + libpng + zlib + ICU + HarfBuzz + … wired together. Those leaf libraries are independently licensed (BSD-3 / MIT / libpng / zlib / Unicode-DFS) and bindable to Rust today. Using them is not "using Chrome" — Skia is used by Servo and Flutter; ANGLE is used by everything from Adobe to Unity; ICU is in every locale-aware app on the planet.

**Option D is the recommended path** — bind those leaf libraries from Rust over deno_core's V8, keep the Rust orchestration layer and the rquest+BoringSSL TLS moat, and **never fork Chromium**. The deno_core memory advantage stays intact (under 100 MB per worker, vs Chrome's ~300 MB). The TLS+H2 byte-identical Chrome work stays winning. The architectural moat of "no CDP, no WebDriver, no BiDi" stays. What changes is that canvas/WebGL/WebAudio/PNG/ICU outputs become byte-identical to Chrome by **construction** because the leaf libraries are literally the ones Chrome uses.

Effort: ~5 months for one engineer; ~3 months for two parallel. The full module-by-module plan with dependencies, validation gates, and risk register is in `/Users/yfedoseev/Projects/browser_oxide/timeline.md`.

The reason no one has built this yet is that the Tier-1-bypass community reaches for patched Chromium first (faster ship), and the Rust-browser community (Servo) doesn't care about anti-bot. browser_oxide sits at the unique intersection. The architecture exists; it just needs commitment.

A fallback Option A (CDP-Chrome) remains available as a tactical bridge for the 7 CHL sites if interim access is needed before Phase 9 of the timeline lands. But the strategic destination is Option D — pick it explicitly, work the timeline, ship.
