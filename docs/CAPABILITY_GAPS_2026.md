# browser_oxide — Capability Gap Audit, April 2026

**Status (updated 2026-04-26):** Partially superseded by [`SOTA_ROADMAP_2026.md`](SOTA_ROADMAP_2026.md). Specifically:

- **§T1.3 (Audio Blink port) — DONE.** `crates/canvas/src/audio.rs:1–818` is bit-accurate to Blink at ~3.6 ppm. The remaining audio work (realtime AudioContext, Analyser FFT, Biquad response, per-`audio_seed` jitter) is tracked in SOTA_ROADMAP_2026.md Phase 2.2 and [`GAPS.md`](GAPS.md) §27.
- **§T1.4 (Finish WebGL via OSMesa) — SUPERSEDED.** OSMesa is Linux-only and the project supports macOS Darwin. Replacement plan: `wgpu` 29.x + Lavapipe (cross-platform Vulkan SwAdapter). See SOTA_ROADMAP_2026.md Phase 2.1 and [`GAPS.md`](GAPS.md) §26. The existing `webgl_render.rs` glow code is a reusable scaffold; the JS-shim wiring (`canvas_bootstrap.js:339–397` calls no ops) and cross-platform native context are the actual blockers.
- **§T1.1 (skia-safe Canvas 2D), §T1.2 (cosmic-text font stack), §T1.5 (Worker plumbing), Tier 2/3 items — STILL APPLICABLE** as written. Schedule them around the Phase 1–3 work in SOTA_ROADMAP_2026.md.

The original document continues unchanged below for reference.

---

**Purpose.** Rank the native-library integrations needed to close the remaining
fingerprint gaps between browser_oxide and Chrome 130, for the legitimate
defensive-research goal of passing every major antibot engine in 2026. This
document assumes **Path A** (wire up real native Rust libraries); fixture replay
is off the table.

**Constraint.** Every recommended dependency must be MIT/Apache-2.0 (or
equivalent permissive, e.g. BSD, Zlib, Unicode-3.0). **No MPL.** Rust bindings
to permissively-licensed C/C++ libraries are acceptable if they build
cross-platform without a display server or GPU driver.

**Current scoreboard.** 48/50 L3 PASS across 8 antibot engines. The four
hard failures (canadagoose/Kasada, hyatt/Kasada, adidas/Akamai BMP v3,
homedepot/Akamai BMP v3) all trace to the same root: the server-side validator
hashes fingerprint metadata that browser_oxide either does not compute or
computes incorrectly.

---

## 0. Confirmed bugs found during this audit

Two concrete mistakes surfaced while grounding the document in real source.
Both are tiny fixes and should land before any large integration work.

1. ~~**`crates/canvas/src/audio.rs:159`** — our `AudioParams::default` uses
   `length: 44100` (1 second), but CreepJS and the common "audio-fp" probe
   (FingerprintJS, Castle, pixelscan) use `OfflineAudioContext(1, 5000, 44100)`
   and sample indices 4500–4600 for their hash
   (ref: `creepjs/src/audio/index.ts`). Running our compressor on 44100 samples
   instead of 5000 produces wrong buffer contents at the indices that actually
   get hashed. Change the default to `length: 5000`.~~ **FIXED 2026-04-26 audit:**
   `crates/canvas/src/audio.rs:74` sets `length: 5000`; test
   `correct_length_5000` (line 704) guards it.

2. **`crates/canvas/src/canvas2d.rs:99`** — `Gradient::Radial::to_shader` drops
   `x0,y0,r0` on the floor and passes `(x1,y1)` twice to `RadialGradient::new`.
   tiny-skia's radial gradient doesn't support a two-circle (inner radius
   offset) form, so this is a latent bug: Chrome-correct radial gradients will
   never match. CreepJS's `paintCanvas` uses radial gradients with non-zero
   inner radius — this matters.

Both are pre-existing, unrelated to the Path A library integrations.

---

## 1. Capability inventory — the matrix

### 1.1 Canvas 2D

| Capability | What Chrome 130 does | browser_oxide today | Gap | FP-sensitive? | Engine | Rust library | Effort |
|---|---|---|---|---|---|---|---|
| `fillRect`, `strokeRect`, `clearRect` | Skia `SkCanvas::drawRect` with `SkPaint`; AA via analytic coverage; premultiplied sRGB | tiny_skia::Pixmap::fill_rect, AA via coverage | **NUMERICAL** — tiny_skia rasterizer differs from Skia's by 1–3 ULPs on edges | YES | Akamai, CreepJS, DataDome, Kasada | `skia-safe 0.78` (MIT) | Medium (20h) |
| Path rasterization (`beginPath`/`moveTo`/`lineTo`/`bezierCurveTo`/`arc`/`fill`/`stroke`) | Skia `SkPath` + `SkPathBuilder`, Bresenham-style scanline with cubic flattening tolerance 0.25 px | tiny_skia::PathBuilder, our own arc approx | NUMERICAL — different cubic flattening tolerance, different arc→bezier split count | YES | CreepJS (paintCanvas bezier/arc round), Akamai | skia-safe | included |
| `fillText` / `strokeText` | Skia `SkTextBlob` → HarfBuzz shaping → platform font rasterizer (CoreText / DirectWrite / FreeType+FontConfig) → AA glyph coverage | `ab_glyph` outlines DejaVu Sans with our own scanline; no shaping, no hinting, no kerning tables beyond GPOS basic | **STRUCTURAL** — no shaping, wrong font, wrong hinting, wrong AA | YES (huge) | All | **cosmic-text 0.12** (MIT/Apache) + **swash 0.1** (MIT/Apache) + **fontdb 0.22** (MIT) + **rustybuzz 0.18** (MIT) | Large (60h) |
| `measureText` (TextMetrics) | Returns width, `actualBoundingBoxLeft/Right/Ascent/Descent`, `fontBoundingBoxAscent/Descent`, `emHeightAscent/Descent`, `hangingBaseline`, `alphabeticBaseline`, `ideographicBaseline` — all from the shaped text run | Width only; all bbox fields = 0 | **STRUCTURAL** — missing fields entirely | YES (CreepJS probes every emoji) | CreepJS, Akamai | cosmic-text / swash | included |
| `drawImage` with HTMLImageElement, HTMLVideoElement, HTMLCanvasElement, ImageBitmap | Skia `drawImageRect` with `SkSamplingOptions` (Mitchell-Netravali by default for `imageSmoothingQuality=medium`) | `put_image_data` raw copy, no scaling, no filter | STRUCTURAL — no resampling filter, no transform matrix | YES | CreepJS, Akamai | skia-safe (via `drawImageRect`) + `image 0.25` for decode (already have) | Small (8h, after skia-safe) |
| `createLinearGradient`, `createRadialGradient` (with inner radius) | Skia two-point conical gradient | Linear OK; radial two-circle form broken (see §0) | NUMERICAL + bug | YES | CreepJS | skia-safe | Small |
| `createPattern` + repeat modes | Skia `SkShader::MakeImage` | Not implemented | STRUCTURAL | YES | — | skia-safe | Small |
| CSS filter (`ctx.filter = 'blur(5px)'`) | Skia `SkImageFilters::Blur` (Gaussian, 3-pass box approximation) | Not implemented | STRUCTURAL | Medium | CreepJS | skia-safe | Small |
| `globalCompositeOperation` (all 26 modes) | Skia `SkBlendMode` enum | Only `source-over` and `source` (tiny_skia only ships a subset) | STRUCTURAL for half the modes | Medium | CreepJS | skia-safe | Medium |
| `shadowBlur`/`shadowColor`/`shadowOffset` | Skia `SkDrawLooper` / `SkImageFilters::DropShadow` | Not implemented | STRUCTURAL | YES (CreepJS `paintCanvas` sets shadowBlur every round) | CreepJS | skia-safe | Small |
| `getImageData` / `putImageData` | Reads SkBitmap pixels, un-premultiplies in 8-bit with Skia's LUT (slightly different from naive `r*255/a`) | Naive `r*255/a` unpremul | NUMERICAL (1-bit off in dark pixels) | YES (CreepJS `getPixelMods` reads back and diffs 64 pixels) | CreepJS | skia-safe (use `SkBitmap::readPixels` with `kUnpremul_SkAlphaType`) | Small |
| `toDataURL("image/png")` | Skia → Skia's PNG encoder with default zlib compression level and specific chunk ordering (tIME, pHYs present) | `png 0.17` crate, different chunk ordering, no pHYs | **NUMERICAL** — bytes differ even for identical pixels | YES (Akamai sensor hashes the exact data URL string) | Akamai, DataDome | Use `skia-safe::EncodedImageFormat::PNG` for byte-identical output | Small (4h) |
| `toDataURL("image/jpeg", q)` | Skia libjpeg-turbo, quality 0.92 default, 4:2:0 chroma subsampling, specific quantization tables | Not implemented | STRUCTURAL | Medium | Akamai | skia-safe | Small |
| `toDataURL("image/webp")` | Skia libwebp | Not implemented | STRUCTURAL | Low | — | skia-safe | Small |
| `textBaseline`, `textAlign`, `direction` | Applied during text layout | Ignored | STRUCTURAL | YES | CreepJS, Akamai | cosmic-text | Small (after cosmic-text lands) |
| `imageSmoothingEnabled`, `imageSmoothingQuality` | Switches SkSamplingOptions | Ignored | STRUCTURAL | Low | — | skia-safe | Small |

### 1.2 WebGL 1 / WebGL 2

| Capability | Chrome 130 | browser_oxide | Gap | FP-sensitive? | Engine | Library | Effort |
|---|---|---|---|---|---|---|---|
| Context creation (`canvas.getContext('webgl')`, `webgl2`, `experimental-webgl`) | ANGLE (D3D11 on Windows, Metal on macOS, Vulkan/GL on Linux) | JS stub returns opaque object; NO backing context | STRUCTURAL | YES | All | OSMesa already wired behind `webgl-render` feature; recommend upgrading backend to **glow 0.14** + **surfman** (permissive subset) or stay on OSMesa | Medium (40h to plumb through) |
| `getParameter` (all ~160 pnames) | Returns ANGLE's translation of the underlying driver's values | Static `WebGLParams` struct with ~20 fields per profile; most pnames return `undefined` | STRUCTURAL for 140 of 160 | YES | Akamai, CreepJS | Populate from a real OSMesa/LLVMpipe context or from a captured fixture table per profile | Large (30h to fill) |
| `UNMASKED_VENDOR_WEBGL`, `UNMASKED_RENDERER_WEBGL` | Real driver strings (`"Google Inc. (NVIDIA)"` / `"ANGLE (NVIDIA, NVIDIA GeForce RTX 3080 ...)"`) | Matches profile | COSMETIC OK | YES | All | — | — |
| `getSupportedExtensions` | ~32 extensions, order matters (Chrome sorts alphabetically with EXT_ before OES_) | Returns hardcoded list, order correct | COSMETIC OK | YES | CreepJS | — | — |
| Shader compilation (`shaderSource`, `compileShader`, `getShaderInfoLog`) | ANGLE's translator → D3D HLSL / Metal MSL / GL, produces specific info log text | Nothing (empty string) | STRUCTURAL | YES (CreepJS reads `getShaderPrecisionFormat` AND info logs) | CreepJS, Akamai | OSMesa/Mesa's GLSL compiler (feature already wired) | Medium |
| `getShaderPrecisionFormat(FRAGMENT_SHADER, HIGH_FLOAT)` | Returns `{rangeMin:127, rangeMax:127, precision:23}` on Chrome desktop | Matches profile | OK | YES | CreepJS | — | — |
| Draw + `readPixels` | Real rasterization; returns exact byte pattern | Returns empty/transparent | **STRUCTURAL** — the big one | YES | CreepJS (draws a triangle and hashes the readback), Akamai | OSMesa + glow (already wired behind feature), build OSMesa with LLVMpipe so rasterization matches across hosts | Medium (40h to finish plumbing + determinism testing) |
| Float precision (shader round-trip) | IEEE-754 single-precision on ANGLE | Depends on LLVMpipe build | NUMERICAL | Medium | CreepJS | — | — |
| `EXT_disjoint_timer_query_webgl2` | Restricted in Chrome (non-null only with cross-origin isolation); returns specific null pattern | Not in extension list | COSMETIC | YES | CreepJS | Add to list per profile | Trivial |
| WebGL2 `getInternalformatParameter(RGBA8, SAMPLES)` | Returns `[4, 2, 1]` (NVIDIA), `[8, 4, 2, 1]` (Intel), etc. | Not implemented | STRUCTURAL | Medium | CreepJS | Populate from OSMesa | Small (after OSMesa) |

### 1.3 OffscreenCanvas

| Capability | Chrome | browser_oxide | Gap | Library |
|---|---|---|---|---|
| `new OffscreenCanvas(w,h)` | Full alt canvas, usable from worker | Stub class | STRUCTURAL | Same skia-safe path once the 2D context is backed by Skia |
| `transferToImageBitmap` | Produces ImageBitmap | Missing | STRUCTURAL | skia-safe |
| `convertToBlob` | PNG/JPEG/WebP encode | Missing | STRUCTURAL | skia-safe |

### 1.4 Audio — AudioContext / OfflineAudioContext

| Capability | Chrome (Blink `AudioNode`s, inherits from WebAudio spec, backed by Chromium's platform audio + libsamplerate + Blink's own DSP) | browser_oxide | Gap | FP? | Engine | Library | Effort |
|---|---|---|---|---|---|---|---|
| `OfflineAudioContext(1, 5000, 44100).startRendering()` | Runs full node graph | Returns a handcrafted buffer; no real graph execution | **STRUCTURAL** + our length is 44100 not 5000 (bug §0) | YES | CreepJS, FingerprintJS, Akamai, DataDome | Build a real graph runner with **fundsp 0.18** (MIT/Apache) or roll our own with **dasp 0.11** (MIT/Apache) | Large (60h to match Blink sample-for-sample) |
| `OscillatorNode` with `type='triangle'`, 10 kHz | Blink's `oscillator_node.cc` uses lookup tables built from a band-limited series (BLIT-like), not naive triangle; specific coefficient table | We generate naive triangle `4f/T` | **NUMERICAL** — Blink's bandlimited triangle has Gibbs ripple, ours doesn't | YES | CreepJS, DataDome | Port Blink's `PeriodicWave` table (it's open-source, BSD in Chromium) | Medium (20h — one-off port) |
| `DynamicsCompressorNode` defaults: threshold=-24, knee=30, ratio=12, attack=0.003, release=0.25 | Blink `DynamicsCompressor.cpp` — uses per-division lookahead, specific envelope, sidechain HPF | We implement a simplified envelope with instant attack; no lookahead, no sidechain HPF | **NUMERICAL** (big) | YES | CreepJS, DataDome, Akamai | Port Blink's `DynamicsCompressorKernel` | Medium (30h — ~500 LOC port of C++ to Rust) |
| `BiquadFilterNode` | Blink's biquad.cc — specific TDF2 implementation | Not implemented | STRUCTURAL | YES | CreepJS | **biquad 0.4** crate (MIT/Apache) — but needs TDF2 to match Blink | Medium (12h) |
| `GainNode` | Trivial multiplier | Not implemented as graph node (only as direct multiply) | STRUCTURAL (shape) | Low | — | — | Small |
| `AnalyserNode` with `getFloatFrequencyData` | Blink-specific FFT (kiss_fft or ffmpeg fft depending on build), Blackman window | Not implemented | **STRUCTURAL** | YES (CreepJS calls it) | CreepJS | **rustfft 6** (MIT/Apache) + Blackman window helper | Medium (15h) |
| `ChannelMergerNode`, `ChannelSplitterNode`, `ConvolverNode`, `DelayNode`, `WaveShaperNode` | Blink | Not implemented | STRUCTURAL (shape only for most scrapers) | Low | — | fundsp has most of these | Medium |
| `ScriptProcessorNode` (deprecated but checked) | Blink | Not present as a class | COSMETIC | YES (CreepJS probes existence) | CreepJS | JS-only shim | Trivial |
| `AudioWorklet` + `addModule` + `AudioWorkletNode` | Runs user code in real-time audio thread | Empty stub | STRUCTURAL | Low (most sensors avoid it) | — | Post-MVP | Large |
| `AudioBuffer.getChannelData` returning `Float32Array` view | Must be a real typed array tied to the buffer (identity across calls) | Need to check — likely returning a new array each call | COSMETIC | YES (CreepJS calls twice and compares identity) | CreepJS | JS shim fix | Trivial |

### 1.5 Fonts

| Capability | Chrome (Windows 11) | browser_oxide | Gap | Engine | Library | Effort |
|---|---|---|---|---|---|---|
| Font enumeration via `document.fonts` | `FontFaceSet` over system fonts + `@font-face` loaded from CSS | `FontFaceSet` empty | STRUCTURAL | CreepJS, Akamai (font list probe) | **fontdb 0.22** (MIT) + embed a font bundle | Medium (20h) |
| `document.fonts.check("16px Arial")` | Returns true for installed fonts, false otherwise | Always returns true | STRUCTURAL | CreepJS, Akamai, DataDome | fontdb with a profile-specific font list | Medium |
| Real glyph shaping (complex scripts, ligatures, kerning, GSUB/GPOS) | HarfBuzz in DirectWrite/CoreText wrapper | ab_glyph raw glyph IDs, no shaping | **STRUCTURAL** | CreepJS (emoji probe), Akamai | **rustybuzz 0.18** (MIT — HarfBuzz port) + **swash 0.1** (MIT/Apache) or **cosmic-text 0.12** as a higher-level wrapper | Large (40h) |
| Font fallback chain ("sans-serif" → Arial on Windows → Helvetica on macOS → DejaVu on Linux) | CSS font matching with platform-specific fallback | Always DejaVu Sans | STRUCTURAL | All | cosmic-text `FontSystem` with profile-specific fallback list | Medium (12h) |
| `FontFace` constructor + `load()` | Real TTF/OTF/WOFF/WOFF2 decode + registration | Stub returns immediately | STRUCTURAL | CreepJS | ttf-parser + **woff2-rs** if we need WOFF2 (check license) | Medium |
| Kerning tables (GPOS type 2) | Applied by HarfBuzz | ab_glyph applies basic kern only | NUMERICAL | CreepJS (measureText varies by kerning) | rustybuzz | — |
| Font metrics (`fontBoundingBox*`, `hangingBaseline`) | From OS/2, hhea, BASE tables | Zero | STRUCTURAL | CreepJS | ttf-parser | Small |

### 1.6 WebCrypto

| Capability | Chrome | browser_oxide | Gap | Library |
|---|---|---|---|---|
| `digest(SHA-1/256/384/512)` | BoringSSL | sha1, sha2 crates ✅ | — | — |
| `sign`/`verify` HMAC | BoringSSL | hmac crate — **check if wired in crypto_ext.rs** | Likely structural | hmac crate (MIT/Apache) |
| `sign`/`verify` RSASSA-PKCS1, RSA-PSS | BoringSSL | Missing | STRUCTURAL | **rsa 0.9** (MIT/Apache) |
| `sign`/`verify` ECDSA (P-256, P-384) | BoringSSL | Missing | STRUCTURAL | **p256**, **p384** (MIT/Apache) |
| `generateKey(RSA)` | BoringSSL | Missing | STRUCTURAL | rsa crate |
| `deriveBits` / `deriveKey` (HKDF, PBKDF2) | BoringSSL | Missing | STRUCTURAL | **hkdf**, **pbkdf2** crates |
| `encrypt`/`decrypt` (AES-GCM, AES-CBC, AES-CTR) | BoringSSL | Missing | STRUCTURAL (some sensors use it) | **aes-gcm**, **aes** crates |
| `exportKey(jwk)` / `importKey(jwk)` | Blink serialization | Missing | STRUCTURAL | — |

### 1.7 Streams, Compression, Structured Clone

| Capability | Chrome | browser_oxide | Gap | Library |
|---|---|---|---|---|
| `ReadableStream` (real) | Blink | Stub class | STRUCTURAL for async consumers | Pure-JS port of whatwg streams or Rust side via async channels |
| `TransformStream`, `WritableStream` | Blink | Stub | STRUCTURAL | same |
| `CompressionStream('gzip')`, `('deflate')`, `('deflate-raw')` | Chromium's zlib | Stub | STRUCTURAL (fetch API uses it) | **flate2** (MIT/Apache) |
| `CompressionStream('br')` | Chromium's brotli | Stub | STRUCTURAL | **brotli** (MIT/Apache) |
| `structuredClone` | Blink | Need to confirm; likely `JSON.parse(JSON.stringify(x))` equivalent | NUMERICAL (loses Maps, Dates, typed arrays) | v8 ValueSerializer op via `deno_core` |
| `postMessage` / MessageChannel | Blink | Basic postMessage on iframe proxy | PARTIAL | — |

### 1.8 Workers

| Capability | Chrome | browser_oxide | Gap | Library |
|---|---|---|---|---|
| `new Worker(url)` runs in a real thread | Blink spawns a worker thread, own V8 isolate | `crates/workers/src/lib.rs` has a `WebWorker` with its own BrowserJsRuntime but **it is never spawned from JS** — JS `Worker` class is a JS-only stub | STRUCTURAL | Plumb through an op that creates a WebWorker, plus a mpsc bridge; deno_core supports this pattern |
| `SharedWorker` | Blink | Stub | STRUCTURAL | Same bridge with a registry |
| `ServiceWorker` | Separate worker, intercepts fetch | Stub | STRUCTURAL for real SW semantics | Large — defer |
| `navigator.serviceWorker.register` returns a promise and updates internal state | Yes | Returns empty object | COSMETIC for sites that don't call back | JS shim improvement |

### 1.9 Storage

| Capability | Chrome | browser_oxide | Gap | Library |
|---|---|---|---|---|
| `localStorage` / `sessionStorage` | Persistent key-value | Need to verify — search for localStorage | Likely in-memory only | — |
| `indexedDB` (IDBFactory/IDBDatabase/IDBTransaction/IDBObjectStore/IDBCursor) | LevelDB/SQLite backend | Empty stub class | STRUCTURAL | **rusqlite 0.32** (MIT) or **idb 0.6** (MIT/Apache) or just implement in-memory with BTreeMaps |
| `Cache` API / `CacheStorage` | Blink, disk-backed | Missing | STRUCTURAL | In-memory BTreeMap shim |
| `File System Access API` | OS FS access | Missing | STRUCTURAL | Tokio fs — but usually this is just presence-checked |
| `Origin Private File System` | Blink-managed dir | Missing | STRUCTURAL | Same |

### 1.10 Observers, Events, Animations

| Capability | Chrome | browser_oxide | Gap | Effort |
|---|---|---|---|---|
| `MutationObserver` | Blink, synchronous microtask | Real (in `dom_bootstrap.js`) | OK | — |
| `IntersectionObserver` | Real geometry checks | Stub always reports "in viewport" | NUMERICAL | Small (tied to layout) |
| `ResizeObserver` | Real layout observer | Stub reports layout dims once | NUMERICAL | Small |
| `PerformanceObserver`, `PerformanceEventTiming` | Blink timeline | Stub | STRUCTURAL (rarely FP-sensitive) | Small |
| `Web Animations API` (`element.animate`) | Blink | Missing | STRUCTURAL | Medium |
| `requestAnimationFrame` | Real 16.67ms tick tied to vsync | Need to verify — probably instant | NUMERICAL (FP-sensitive for RAF-timing sensors) | Small — deterministic clock tied to event loop |
| `queueMicrotask` | V8 | deno_core provides it | OK | — |

### 1.11 Media / Capture / RTC

| Capability | Chrome | browser_oxide | Gap | FP? | Library |
|---|---|---|---|---|---|
| `HTMLMediaElement` (`<video>`, `<audio>`) playback | FFmpeg-via-libav + platform decoders | Stub element, no decode | STRUCTURAL | Low for sensors; medium for media-site scrapers | Can stay stub for antibot; defer |
| `MediaSource Extensions` | Blink | Stub | STRUCTURAL | Medium (DataDome probes `MediaSource.isTypeSupported`) | JS shim with a correct codec allow-list |
| `MediaDevices.enumerateDevices()` | Real enumeration of mics/cameras/speakers with stable deviceIds | Returns `[]` | COSMETIC → NUMERICAL | YES (CreepJS reads deviceId patterns) | JS shim with profile-specific device list |
| `MediaDevices.getUserMedia()` | Returns a MediaStream or rejects with `NotAllowedError` | Need to verify — probably rejects | COSMETIC | YES | JS shim |
| `MediaRecorder` | Chromium's audio/video encoder | Stub | STRUCTURAL | Low | Defer |
| `RTCPeerConnection` | libwebrtc | Stub class; `createDataChannel`, `createOffer` empty | STRUCTURAL | YES (CreepJS hashes SDP offer, DataDome checks ICE candidates) | **webrtc-rs** (MIT/Apache) — large footprint; or ship a fake SDP with the exact string Chrome produces on a dummy offer |
| `RTCDataChannel` | libwebrtc | Stub | STRUCTURAL | Low for L3 | Defer |
| `WebCodecs` (`VideoDecoder`, `AudioDecoder`) | Blink | Missing | STRUCTURAL | Low | Defer |
| Web Speech API (`SpeechRecognition`, `SpeechSynthesis`) | Blink + platform engines; `speechSynthesis.getVoices()` returns ~15 voices on Windows | `SpeechSynthesis` empty class | STRUCTURAL → NUMERICAL (CreepJS hashes voice list) | CreepJS | JS shim with profile-specific voice table (Windows / macOS / Linux) |

### 1.12 Device / Sensor / Permission APIs

| Capability | Chrome | browser_oxide | Gap | FP? | Effort |
|---|---|---|---|---|---|
| `navigator.permissions.query({name:'notifications'})` | Returns `{state:'prompt'}` for most, `'granted'` for some | JS shim returns `'prompt'` always | NUMERICAL | YES (some sensors probe multiple names) | Small |
| `Notification.permission` | `'default'`/`'granted'`/`'denied'` | `'default'` | OK | YES | — |
| `Notification.requestPermission()` | Async prompt | Missing method? | COSMETIC | Low | Small |
| `navigator.geolocation.getCurrentPosition` | Prompts; returns coords or error | Empty object | STRUCTURAL (presence-check OK; rejects with error is better) | Medium | Small |
| `navigator.getBattery()` (deprecated, still probed) | Returns `{charging, level, chargingTime, dischargingTime}` with plausible values | Check JS shim | Likely OK | YES (CreepJS probes) | Small |
| `Gamepad` (`navigator.getGamepads()`) | Returns `[null,null,null,null]` in desktop headless | `[null,null,null,null]` ✅ | OK | YES | — |
| `navigator.usb`, `navigator.bluetooth`, `navigator.hid`, `navigator.serial` | Constructors exist, all methods reject | Stubs as empty objects | COSMETIC → STRUCTURAL | YES (CreepJS checks `typeof navigator.usb.requestDevice`) | Small (add methods that reject) |
| `PaymentRequest` | Constructor exists; `canMakePayment` returns false in headless | Need to verify presence | COSMETIC | YES | Small |
| `Clipboard API` (`navigator.clipboard.read`/`write`) | Real, gated on permission | Returns empty string / resolves | COSMETIC | YES (CreepJS checks `typeof`) | Small |

### 1.13 CSS & Layout (the fingerprint-visible subset)

| Capability | Chrome | browser_oxide | Gap | Effort |
|---|---|---|---|---|
| `getComputedStyle` | Full cascade + inherited + resolved values | css_cascade crate — check what it returns | PARTIAL | Medium |
| `CSS.supports('display', 'grid')` | Returns true/false per property db | — need to verify | COSMETIC | Small |
| Font matching via CSS (`font-family: Arial, sans-serif`) | Walks font-cache, returns resolved family | Always DejaVu | STRUCTURAL | Medium (tied to cosmic-text + fontdb integration) |
| Houdini `CSS.paintWorklet` | Blink | Missing | STRUCTURAL (rarely probed) | Large — defer |
| `CSS Typed OM` | Blink | Missing | STRUCTURAL (presence-checked) | Small |
| Element.getBoundingClientRect | Real layout | Need to verify — likely returns layout-engine values | OK if layout crate works | — |

---

## 2. Ranked build order

"Fingerprint impact per engineering hour," ordered highest to lowest. The top
five are the smallest set that should, if executed well, close the canadagoose
/ hyatt / adidas / homedepot gap. Each unblocks a specific engine.

### Tier 1 — "ship these first, they move 4/4 blocked sites"

#### T1.1 — Replace tiny_skia with skia-safe for Canvas 2D + encoding

- **Closes:** all canvas numerical drift; byte-identical `toDataURL`; all
  compositing modes; filters; shadows; image resampling; `getImageData`
  unpremul LUT; radial gradients (fixes §0 bug 2)
- **Crate:** `skia-safe = "0.78"` — MIT. Wraps Google Skia (BSD-3-Clause).
- **Size impact:** ~40 MB statically linked; precompiled via `skia-bindings`
  feature `use-system-libraries` is not portable enough; keep
  `embed-freetype` off on Linux. Compile time: ~8 minutes cold, cached after.
- **Drop-in?** No — needs glue code replacing `tiny_skia::Pixmap` calls in
  `crates/canvas/src/canvas2d.rs`. ~500 LOC changes, plus op bridge updates in
  `crates/js_runtime/src/extensions/canvas_ext.rs` to pass through new params
  (`filter`, `shadowBlur`, composite modes).
- **Integration steps:**
  1. Add `skia-safe = { version = "0.78", default-features = false, features = ["textlayout"] }` to `crates/canvas/Cargo.toml`
  2. Introduce a `SkiaBackend` behind the existing `Canvas2D` type; feature-gate the tiny_skia path for dev builds
  3. Replace `Pixmap`, `Paint`, `Transform`, `Path`, `Shader` with `Surface`, `Paint`, `Matrix`, `Path`, `Shader` from skia-safe
  4. Wire `shadowBlur`, `globalCompositeOperation`, `filter` through the canvas ext op signatures
  5. Implement `toDataURL` via `EncodedImageFormat::PNG` so byte-level output matches Chrome
  6. Add a regression test that renders the samsclub Akamai canvas sequence (see §3) and diffs against a captured Chrome 130 reference
- **Unblocks:** Akamai BMP v3 on adidas.com and homedepot.com (the sensor stores the canvas hash in `sensor_data`; matching Chrome bytes here is a necessary condition for A0/A1 → A2 trust upgrade). Also fixes all CreepJS canvas-section scores.
- **Effort:** 25–35 hours. Highest ROI item in the document.

#### T1.2 — Real font stack: cosmic-text + fontdb + rustybuzz + swash + Chrome font bundle

- **Closes:** `fillText`/`measureText` glyph shapes, TextMetrics fields, font
  fallback, `document.fonts.check`, emoji shaping, kerning
- **Crates:**
  - `cosmic-text = "0.12"` (MIT/Apache) — high-level text layout + shaping
  - `fontdb = "0.22"` (MIT) — font DB with system + embedded sources
  - `rustybuzz = "0.18"` (MIT) — HarfBuzz port for shaping (used under the hood by cosmic-text)
  - `swash = "0.1"` (MIT/Apache) — glyph rendering with hinting
  - `ttf-parser = "0.24"` (MIT/Apache) — metrics access
- **Font bundle:** Ship a ~30 MB zip with the fonts Chrome has on Windows 11 English locale: Arial, Arial Black, Calibri (MS proprietary — **skip**, use Carlito as metric-compat substitute), Cambria (**skip**, use Caladea), Comic Sans MS (skip, use Comic Neue), Consolas (skip, use Consola Mono), Courier New, Georgia, Impact, Segoe UI, Segoe UI Emoji, Tahoma, Times New Roman, Trebuchet MS, Verdana. Legally we can ship only fonts with permissive licenses (OFL, Apache). The pragmatic approach: ship DejaVu + Noto + Liberation (metric-compatible with Arial/Times/Courier) + Carlito/Caladea (metric-compatible with Calibri/Cambria) + Noto Color Emoji. Metric compatibility is what matters for `measureText`, not the visual glyphs — though Akamai does hash glyph rasters. For canvas pixel matching specifically, you need the **real** Chrome fonts. Decision: embed Liberation + Noto as baseline; document that for pixel-perfect Akamai matching, the operator must point at a system with genuine Microsoft Core Fonts (users typically already have them on real Windows/Mac deployment boxes).
- **Integration steps:**
  1. Remove `ab_glyph` from `crates/canvas`. Add cosmic-text.
  2. Create `FontSystem` singleton at runtime startup loading `fontdb` with embedded fonts + OS font directories (guarded by profile)
  3. Rewrite `crates/canvas/src/text.rs` to use `cosmic-text::Buffer::set_text` + `SwashCache` for rasterization
  4. Implement `measureText` returning real TextMetrics with all 14 fields, computed from shaped run + OS/2 table via ttf-parser
  5. Populate `document.fonts` from fontdb (`Database::faces()`)
  6. Wire `FontFace` constructor through to `fontdb::Database::load_font_data`
  7. Add `fillText` regression test against the samsclub fingerprint sequence
- **Size:** cosmic-text+swash+fontdb+rustybuzz ~3 MB code; font bundle ~30–60 MB; total under budget
- **Unblocks:** Akamai (canvas text hash), CreepJS emoji probe, sets up the Kasada fingerprint because Kasada's ips.js implicitly reads `measureText` during canvas probes
- **Effort:** 50–70 hours

#### T1.3 — Real OfflineAudioContext node graph (dasp-based minimal path)

- **Closes:** audio fingerprint; CreepJS audio, DataDome audio, Akamai audio
- **Crates:**
  - `dasp = "0.11"` (MIT/Apache) — sample DSP primitives
  - `rustfft = "6"` (MIT/Apache) — for AnalyserNode
  - `biquad = "0.4"` (MIT/Apache) — for BiquadFilter baseline
- **Strategy:** We don't need a general-purpose audio engine — we need
  bit-accurate reproductions of the three nodes sensors use: OscillatorNode,
  DynamicsCompressorNode, AnalyserNode. Everything else can be a passthrough
  gain. The pragmatic approach is to port Chromium's
  `third_party/blink/renderer/modules/webaudio/dynamics_compressor.cc` and the
  `PeriodicWave` initialization code directly to Rust (both are BSD-3-Clause
  in Chromium, compatible with our MIT/Apache). This is a one-off port of
  ~600 LOC of C++.
- **Integration steps:**
  1. Fix §0 bug 1 (`length: 5000`) — blocker for any progress
  2. Port Blink's `PeriodicWave::GenerateBasicWaveform` for `type='triangle'`
  3. Port Blink's `DynamicsCompressorKernel::process` sample-for-sample
  4. Build a small "MiniAudioGraph" struct in `crates/canvas/src/audio.rs` that connects OscillatorNode → DynamicsCompressorNode → destination in a fixed pipeline triggered by `startRendering`
  5. Wire `AnalyserNode::getFloatFrequencyData` via rustfft with a Blackman window
  6. Regression test against the CreepJS fingerprint bytes (indices 4500–4600 must match Chrome 130)
- **Size:** <500 KB added
- **Unblocks:** CreepJS audio, DataDome audio (partial), any sensor that hashes
  OfflineAudioContext output — **including the Akamai fingerprint metadata**
  that gates the A0/A1 → A2 upgrade on adidas/homedepot. Because Akamai
  compares the client's audio vector against its Chrome DB, even a 1-byte diff
  rejects the upgrade. Getting this right is 50% of the adidas fix.
- **Effort:** 40–55 hours

#### T1.4 — Finish WebGL via OSMesa + glow (ship the existing feature)

- **Closes:** WebGL `readPixels`, shader info logs, `getParameter` gaps
- **Crates:** already in tree — `glow = "0.14"` (MIT/Apache/Zlib) plus OSMesa
  via FFI (`crates/canvas/src/osmesa_ffi.rs`)
- **Problem:** OSMesa is gated behind the `webgl-render` feature and does not
  ship in the default build. The code exists but isn't exercised.
- **Integration steps:**
  1. Add OSMesa build pipeline to CI: `libosmesa6-dev` on Linux, build from source on macOS and Windows (OSMesa is part of Mesa, Apache-compatible)
  2. Make `webgl-render` part of the default feature set
  3. Finish `WebGLContext::read_pixels` plumbing — currently `op_webgl_read_pixels` exists but the JS bootstrap doesn't call it
  4. Replace the JS WebGL stub in `window_bootstrap.js` with a path that routes `readPixels`, `drawArrays`, `drawElements`, `getParameter` through the ops
  5. Populate `WebGLParams::get_parameter_int` for all ~160 pnames via a captured fixture table per profile (one-time capture from a real Chrome 130 on each target OS)
  6. Add a regression test that compiles a trivial vertex+fragment shader, draws a red triangle, and reads back 4 pixels
- **Gotcha:** OSMesa's LLVMpipe path is deterministic across x86_64 Linux but
  can diverge slightly on macOS/Windows if the build uses different LLVM
  versions. Pin LLVM. Document that macOS/Windows support requires bundling
  the built .dylib/.dll.
- **Unblocks:** CreepJS WebGL section, the WebGL probe in Kasada ips.js (we
  know it's there from public RE even though it's string-obfuscated), any
  sensor that hashes a draw + readPixels round-trip
- **Effort:** 35–50 hours

#### T1.5 — Worker thread plumbing + real postMessage

- **Closes:** structural gap where Kasada's ips.js may run parts of its code
  in workers (need to verify from the bundle), sensors that check
  `typeof Worker` and then actually instantiate one
- **Crate:** none new; use existing `workers` crate and deno_core's worker
  host pattern
- **Integration steps:**
  1. Add an op `op_worker_spawn(script: String) -> worker_id`
  2. Replace the JS `Worker` stub in `window_bootstrap.js` with a class that calls `op_worker_spawn` on construction and routes `postMessage`/`onmessage` through an op pair
  3. Run the WebWorker on a dedicated tokio task (not a thread, since V8 is per-thread but deno_core can host a worker as a separate isolate on a dedicated thread)
  4. Wire postMessage via mpsc channels already present in `WebWorker`
  5. Add a structured-clone serializer via `deno_core::v8::ValueSerializer`
- **Size:** minor
- **Unblocks:** Kasada (we suspect ips.js offloads ~15% of its work to
  workers; confirmed by the bundle containing `new Worker` fragments but we
  should verify), CreepJS worker-scope probe, any scraper target that uses
  a worker-based captcha
- **Effort:** 25–35 hours

### Tier 2 — "important but not blocking any specific site"

- **T2.1 — Real IndexedDB** (`rusqlite` or in-memory BTreeMap-backed) — 20h. Needed for any site that tries to read a previous session's fingerprint out of idb. Not a gating factor for the 4 blocked sites.
- **T2.2 — CompressionStream (gzip, deflate, deflate-raw, br)** with `flate2` + `brotli` — 8h. Trivial, unblocks some fetch-heavy scrapers.
- **T2.3 — Full WebCrypto sign/verify/generateKey** with rsa, p256, p384, aes-gcm, pbkdf2, hkdf — 30h. Needed if any of our blocked sensors call `crypto.subtle.sign` — need to re-check the bundles.
- **T2.4 — Web Speech API voice list** (profile-specific) — 4h. CreepJS probes `speechSynthesis.getVoices()`. JS shim only.
- **T2.5 — MediaDevices.enumerateDevices** returning 3-4 plausible devices with stable UUIDs per profile — 4h. JS shim.
- **T2.6 — RTCPeerConnection: return a realistic SDP offer** from a captured Chrome 130 fixture (not a full libwebrtc integration) — 12h. Unblocks any sensor that probes `createOffer().then(o => hash(o.sdp))`.
- **T2.7 — Structured clone via v8 ValueSerializer** — 6h. Matters for `postMessage` across iframe/worker boundaries.
- **T2.8 — Performance timing determinism** (`performance.now()` with jittered but monotonically-increasing clock tied to our event loop) — 6h. CreepJS checks timer resolution.

### Tier 3 — "defer until a concrete site breaks on it"

- **T3.1 — Full WebGL2 parameter coverage** (beyond the 20 pnames in the matrix) — 20h
- **T3.2 — Houdini paintWorklet** — 40h
- **T3.3 — Web Animations API** — 30h
- **T3.4 — WebCodecs** — 60h
- **T3.5 — Full WebRTC with libwebrtc-rs** — 200h+
- **T3.6 — AudioWorklet real execution** — 80h
- **T3.7 — Service Worker with real fetch interception** — 80h
- **T3.8 — File System Access API** — 30h
- **T3.9 — Full MediaSource / HLS / DASH** — 150h

---

## 3. Canvas, WebGL, Audio, Font — deep drill-down

### 3.1 Canvas — exact fingerprint patterns

#### Akamai BMP v3 (samsclub sample, line 333)

Verified from `docs/akamai_sensor_analysis/samsclub_akam13_bootstrap.deob.js`:

```js
a.fillStyle = 'rgba(255,153,153, 0.5)';
a.font = '18pt Tahoma';
a.textBaseline = 'top';
a.fillText('Soft Ruddy Foothold 2', 2, 2);
a.fillStyle = '#0000FF';
a.fillRect(100, 25, 30, 10);
a.fillStyle = '#E0E0E0';
a.fillRect(100, 25, 20, 30);
a.fillStyle = '#FF3333';
a.fillRect(100, 25, 10, 15);
a.fillText('!H71JCaj)]# 1@#', 4, 8);
var r = n.toDataURL();
```

**What Chrome 130 produces:** we don't have a cached reference here.
Acquisition task (see §5): run the above in real Chrome 130 on macOS (M2) and
Windows 11, capture `toDataURL()` output, embed as fixtures under
`crates/canvas/tests/fixtures/akamai_canvas_*.txt`. These become the
regression targets for the skia-safe integration.

**Key dependencies for a pixel match:**
- Real Tahoma (not DejaVu) — Tahoma is proprietary; Liberation Sans is NOT metric-compatible with Tahoma. This is a genuine problem. Options: (a) ship a bundled Tahoma under license from operator's existing Microsoft install, (b) use fontconfig to fall back to a Tahoma-equivalent and accept some drift, (c) ship Dejavu and spoof the UA to Linux (where Tahoma is less expected). Option (c) is easiest but cuts off a whole class of sites.
- Real `fillStyle = 'rgba(255,153,153, 0.5)'` alpha compositing — Skia's 8-bit alpha blend path, not ours
- `textBaseline = 'top'` — we currently ignore textBaseline

#### CreepJS canvas (verified from creepjs/src/canvas/index.ts)

- **Paint canvas:** 50×50 canvas, 10 rounds. Each round:
  - `fillStyle = <radial gradient from seeded palette>` — 50-color palette, seed-driven
  - `shadowBlur = <round-indexed>`, `shadowColor = <palette color>`
  - One of: `arc(cx, cy, r, 0, 2*PI)`, `bezierCurveTo(...)`, `quadraticCurveTo(...)`
  - `ctx.fill()`
- **Text rendering:**
  - `fillText('A', 7, 37)` at 50px
  - `fillText('👾', 0, 37)` at 35px (emoji → tests color emoji font)
- **measureText emoji sweep:** iterates all Unicode emoji, calls `measureText`, records `actualBoundingBoxAscent`, `actualBoundingBoxDescent`, `width`, etc.
- **getPixelMods:** draws 64 random pixels to 8x8, `getImageData`, compares, hashes the diff. **This detects any un-premul rounding error.** Our naive `r*255/a` diverges from Skia's LUT-based unpremul in exactly this probe.

**Chrome 130 reference:** CreepJS maintains a known-good hash list per browser
version. The reference values for Chrome 130 on Blink engine are published
inside the CreepJS test runner — see `creepjs/src/canvas/patterns.ts` (or
equivalent; file name changes across versions). Pull those during integration.

### 3.2 WebGL — expected readPixels for a simple triangle

The canonical CreepJS/BrowserLeaks test:

```js
const gl = canvas.getContext('webgl');
const vs = `attribute vec2 p; void main(){gl_Position=vec4(p,0,1);}`;
const fs = `precision mediump float; void main(){gl_FragColor=vec4(1,0,0,1);}`;
// compile, link, use
gl.bindBuffer(gl.ARRAY_BUFFER, gl.createBuffer());
gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1,-1, 1,-1, 0,1]), gl.STATIC_DRAW);
// vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0); enableVertexAttribArray(0);
gl.viewport(0, 0, 256, 256);
gl.drawArrays(gl.TRIANGLES, 0, 3);
const px = new Uint8Array(4);
gl.readPixels(128, 128, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, px);
// Expected on Chrome 130 + ANGLE + any desktop GPU: [255, 0, 0, 255]
```

Center-pixel `[255, 0, 0, 255]` is stable across vendors because the triangle
covers the center fully with `precision mediump float` red. The interesting
probe is the **edge pixel at (1,127)** where AA coverage varies by rasterizer:
ANGLE/D3D11 gives ~0x80 coverage, ANGLE/Metal gives ~0x7f, LLVMpipe gives
~0x7f. We need OSMesa+LLVMpipe to reproduce within ±1.

Real CreepJS hashes a 256×256 draw of more complex geometry. Acquisition task:
capture a 4096-byte readPixels output from Chrome 130 on macOS M2 running
against a reference shader, embed as fixture.

### 3.3 Audio — DynamicsCompressor output on a 1 kHz sine

Blink's compressor is not trivially equivalent to the standard spec formula.
It uses a 4-division lookahead, RMS detection over a configurable window, and
a fixed-gain makeup stage. Our current implementation in
`crates/canvas/src/audio.rs:83-111` uses:

- Instantaneous input dB → knee curve (scalar)
- Single-pole envelope follower
- Attack/release via `1 - exp(-1/(t*sr))` time constants

Blink uses:

- **Pre-delay buffer** of ~6 ms
- **Detector input**: averaged peak over 3 ms window, then converted to dB
- **Knee curve**: 3rd-order polynomial, not quadratic
- **Envelope**: sidechained exponential with branched attack/release, two
  timers (fast + slow)
- **Makeup gain**: `0.6` linear, not 1.0

This means our output at indices 4500–4600 of a 5000-sample render will be
materially different from Chrome's, even with the triangle wave input correct.
Porting Blink's `DynamicsCompressorKernel::process` is the only path to bit
accuracy. The source is in Chromium at
`third_party/blink/renderer/platform/audio/DynamicsCompressorKernel.cpp`, BSD-3,
~500 LOC, directly portable to Rust with one-to-one array/float operations.

Reference for a 10 kHz triangle × compressor default → the sum of
`data[4500..5000]` on Chrome 130 is publicly documented by FingerprintJS as
`124.04347527516074` (±1e-6). This is the target number for the audio
regression test.

### 3.4 Fonts — what ships with Chrome

**Windows 11 (English, US, default install)** — Chrome uses DirectWrite on top
of the system font cache. The always-present baseline includes:

- Arial, Arial Black, Arial Narrow, Arial Unicode MS
- Bahnschrift
- Calibri, Calibri Light
- Cambria, Cambria Math
- Candara
- Comic Sans MS
- Consolas
- Constantia
- Corbel
- Courier New
- Ebrima
- Franklin Gothic Medium
- Gabriola
- Gadugi
- Georgia
- HoloLens MDL2 Assets
- Impact
- Ink Free
- Javanese Text
- Leelawadee UI
- Lucida Console, Lucida Sans Unicode
- Malgun Gothic
- Marlett
- Microsoft Himalaya, Microsoft JhengHei, Microsoft New Tai Lue, Microsoft PhagsPa, Microsoft Sans Serif, Microsoft Tai Le, Microsoft YaHei, Microsoft Yi Baiti
- MingLiU-ExtB, PMingLiU-ExtB, MingLiU_HKSCS-ExtB
- Mongolian Baiti
- MS Gothic, MS PGothic, MS UI Gothic, MS Mincho, MS PMincho
- MV Boli
- Myanmar Text
- Nirmala UI
- Palatino Linotype
- Segoe Fluent Icons
- Segoe MDL2 Assets
- Segoe Print, Segoe Script
- Segoe UI, Segoe UI Black, Segoe UI Emoji, Segoe UI Historic, Segoe UI Symbol, Segoe UI Variable
- SimSun, NSimSun, SimSun-ExtB
- Sitka (series)
- Sylfaen
- Symbol
- Tahoma
- Times New Roman
- Trebuchet MS
- Verdana
- Webdings
- Wingdings, Wingdings 2, Wingdings 3
- Yu Gothic, Yu Gothic UI, Yu Mincho

**macOS 14 (Sonoma) — Apple Silicon M2**:

- .AppleSystemUIFont (San Francisco)
- Al Bayan, Al Nile, Al Tarikh
- American Typewriter
- Andale Mono
- Apple Chancery, Apple Symbols
- Apple Braille, Apple Color Emoji
- Arial, Arial Black, Arial Narrow, Arial Rounded MT Bold, Arial Unicode MS
- Avenir, Avenir Next, Avenir Next Condensed
- Baskerville
- Big Caslon
- Bodoni 72 (series)
- Bradley Hand
- Brush Script MT
- Chalkboard, Chalkboard SE, Chalkduster
- Charter
- Cochin
- Comic Sans MS
- Copperplate
- Courier, Courier New
- Didot
- DIN Alternate, DIN Condensed
- Futura
- Geneva
- Georgia
- Gill Sans
- Helvetica, Helvetica Neue
- Herculanum
- Hiragino (several)
- Hoefler Text
- Impact
- Kefa
- Lucida Grande
- Marker Felt
- Menlo
- Microsoft Sans Serif
- Monaco
- Noteworthy
- Optima
- Palatino
- Papyrus
- PingFang SC/TC/HK
- Rockwell
- San Francisco (display, text, mono, pro, rounded variants)
- Savoye LET
- SignPainter
- Skia
- Snell Roundhand
- Tahoma
- Times, Times New Roman
- Trattatello
- Trebuchet MS
- Verdana
- Zapf Dingbats, Zapfino

**Ubuntu 22.04 (default GNOME desktop install, English locale)** — sparse:

- DejaVu Sans, DejaVu Sans Mono, DejaVu Serif
- Liberation Mono, Liberation Sans, Liberation Serif
- Noto Color Emoji
- Noto Mono
- Noto Sans (CJK variants)
- Noto Serif (CJK variants)
- Ubuntu, Ubuntu Condensed, Ubuntu Mono
- (a few CJK-specific ones)

**Implication:** the Linux font list is the smallest baseline and the easiest
to reproduce without licensing problems (all DejaVu/Liberation/Noto/Ubuntu
fonts are OFL/GPL-with-font-exception, compatible with redistribution). We
should initially target Linux fingerprint profiles and accept that
Windows/macOS targeting is harder because it requires legally bundling
Microsoft/Apple proprietary fonts, or running on boxes that already have them.

The 4 blocked sites: adidas.com and homedepot.com both expect a desktop-class
device. If we spoof Linux for these, we need to make sure the rest of the
profile (Navigator, Screen, WebGL UNMASKED_RENDERER) says Linux too, which our
Linux profile already does.

---

## 4. License audit

All recommended libraries (verified April 2026 via crates.io versions API):

| Crate | Version | License | Notes |
|---|---|---|---|
| `skia-safe` | 0.78 | **MIT** | Wraps Skia (BSD-3-Clause) via `skia-bindings` (MIT). Fine. |
| `skia-bindings` | 0.78 | MIT | — |
| `cosmic-text` | 0.12 | MIT OR Apache-2.0 | — |
| `fontdb` | 0.22 | **MIT** | Single-license but permissive |
| `rustybuzz` | 0.18 | **MIT** | HarfBuzz port, permissive |
| `swash` | 0.1 | MIT OR Apache-2.0 | — |
| `ttf-parser` | 0.24 | MIT OR Apache-2.0 | — |
| `ab_glyph` | 0.2 | **Apache-2.0** | Currently used; OK but we're replacing it |
| `fontdue` | latest | MIT OR Apache-2.0 OR Zlib | Alternative to ab_glyph; not recommended (no shaping) |
| `font-kit` | latest | MIT OR Apache-2.0 | Alternative to fontdb; more featureful but heavier |
| `parley` | latest | Apache-2.0 OR MIT | Google-maintained text layout; considered, but cosmic-text is more mature |
| `dasp` | 0.11 | MIT OR Apache-2.0 | — |
| `fundsp` | 0.18 | MIT OR Apache-2.0 | Alternative graph engine — heavier than we need |
| `rustfft` | 6 | MIT OR Apache-2.0 | — |
| `biquad` | 0.4 | MIT OR Apache-2.0 | — |
| `rubato` | latest | **MIT** | Resampling — for real audio, we don't need |
| `hound` | latest | Apache-2.0 | WAV codec — only if we add `decodeAudioData`; low priority |
| `cpal` | latest | Apache-2.0 | Real audio I/O — not needed for headless; skip |
| `rodio` | latest | MIT OR Apache-2.0 | — |
| `kira` | latest | MIT OR Apache-2.0 | — |
| **`symphonia`** | latest | **MPL-2.0** | ❌ **BANNED** — cannot use for audio file decoding |
| `glow` | 0.14 | MIT OR Apache-2.0 OR Zlib | Already in tree |
| `glutin` | latest | Apache-2.0 | Not needed in headless |
| `winit` | latest | Apache-2.0 | Not needed |
| `wgpu` | latest | MIT OR Apache-2.0 | For future WebGPU; big dependency |
| `naga` | latest | MIT OR Apache-2.0 | Shader translation; bundled with wgpu |
| `image` | 0.25 | MIT OR Apache-2.0 | Already in tree |
| `jpeg-decoder` | latest | MIT OR Apache-2.0 | — |
| `png` | 0.17 | MIT OR Apache-2.0 | Already in tree |
| `rusqlite` | 0.32 | **MIT** | For IndexedDB backing |
| `idb` | 0.6 | MIT OR Apache-2.0 | Higher-level IDB if we prefer |
| `flate2` | latest | MIT OR Apache-2.0 | — |
| `brotli` | latest | MIT OR Apache-2.0 | — |
| `rsa` | 0.9 | MIT OR Apache-2.0 | — |
| `p256`, `p384` | latest | Apache-2.0 OR MIT | — |
| `aes-gcm`, `aes` | latest | Apache-2.0 OR MIT | — |
| `pbkdf2`, `hkdf`, `hmac` | latest | Apache-2.0 OR MIT | — |
| **`symphonia`** | — | **MPL-2.0** | ❌ banned, reiterated |
| **`surfman`** | latest | `MIT OR Apache-2.0 OR MPL-2.0` | ⚠️ Triple-licensed. Taking the MIT or Apache option is legal, but the triple-license suggests upstream has absorbed MPL contributions. Safe path: avoid, stay on OSMesa direct FFI. |
| **`hifitime`** | latest | **MPL-2.0** | ❌ banned (not needed, just flagging) |
| **`raqote`** | latest | **BSD-3-Clause** | OK but we prefer Skia for Chrome pixel parity |
| `kurbo` | latest | Apache-2.0 OR MIT | For path math if we want to avoid Skia for pure-Rust builds |
| `vello`/`peniko` | latest | Apache-2.0 OR MIT | Linebender's GPU-accelerated 2D. Not a good match for headless. |
| `resvg`/`usvg` | latest | Apache-2.0 OR MIT | For SVG; not needed for fingerprint gap closure |
| `freetype-rs` | latest | **MIT** | Wraps FreeType (FreeType license or GPLv2 — FreeType license is OK for redistribution). If cosmic-text ever depends on it transitively, verify. |
| `rust-fontconfig` | latest | **MIT** | Pure-Rust fontconfig reimplementation; Apache-compatible |

**Binary size projection** (release, stripped, x86_64 Linux, adding Tier 1 only):
- Current: ~180 MB (V8 is 130 of that)
- + skia-safe: +45 MB → ~225 MB
- + cosmic-text stack: +3 MB → ~228 MB
- + font bundle (Liberation + Noto baseline): +50 MB → ~278 MB
- + OSMesa (linked as shared .so; not bundled statically): +2 MB → ~280 MB
- + audio Rust DSP: +1 MB → ~281 MB

Well under the 500 MB cap.

---

## 5. Open questions — things I could not determine without running Chrome

1. **Byte-exact `toDataURL` output for the samsclub Akamai canvas pattern on Chrome 130.** Needs a capture run. This is the single most important fixture.
2. **CreepJS's current Chrome 130 reference hashes** for canvas, audio, WebGL, font. CreepJS embeds these in its source tree but they drift each release — need to pull the current commit.
3. **Exact Kasada ips.js fingerprint probes** — the 529 KB bundle we have is fully obfuscated with encoded strings. The public reverse engineering writeups (Castle.io, KoSSSHi) say Kasada probes Canvas, WebGL, Audio, Fonts, Navigator, Screen, Timers and that ips.js computes an HMAC-SHA256 over the serialized probe results using a key derived from the initial handshake. To know which specific canvas/WebGL patterns Kasada uses, we'd need to run ips.js under an instrumented V8 and dump the method calls. The `functions.json` file is empty (2 bytes) — whoever ran the disassembler either didn't emit function data or ips.js is control-flow flattened beyond what the tool handles.
4. **Whether Kasada runs any of ips.js in a Worker context.** We suspect yes based on size and the presence of a postMessage pattern, but have not verified. If yes, T1.5 (Worker plumbing) is a hard blocker for canadagoose/hyatt.
5. **Exact Blink `DynamicsCompressorKernel` parameter defaults in Chrome 130** versus older versions. Chromium's compressor kernel has seen ~3 commits between Chrome 100 and Chrome 130; we need the Chrome 130 snapshot specifically, not a spec-level port. Verify against the Chromium release branch tag.
6. **Akamai sensor's canvas hash input format.** We see the samsclub sensor call `toDataURL()` and store `r` locally, but we don't know whether it then takes the raw string, base64-decodes it, or just hashes the raw string. Needs a live trace of `sensor_data` payload post-canvas. Likely in `docs/akamai_sensor_analysis/` if we instrument southwest/samsclub further.
7. **Whether the adidas/homedepot failure is actually gated on canvas+audio+webgl+fonts, or on something else.** Our current theory is fingerprint mismatch, but it could be timing-based (we click too fast, mouse move too linear, etc.). Before investing 150 hours in native library work, consider a 1-day experiment where we manually upload a fixture-captured Chrome 130 sensor_data payload and see if _abck upgrades to A2. If yes, it's FP. If no, it's timing.
8. **Whether our existing `WebGL` ops (already present behind the feature) actually work with OSMesa on Linux x86_64.** They compile but we don't know if there's a test exercising the full pipeline end-to-end.
9. **Chrome 130 default value of `navigator.userActivation` and `Notification.permission`** on macOS vs Windows. CreepJS hashes these. Small gap, small fix.
10. **Whether `document.fonts.ready` resolves synchronously or asynchronously in Chrome 130.** Affects the JS shim correctness.

Several of these (1, 2, 5, 9) are one-day capture jobs once we set up a
reference Chrome 130 environment with tracing. Several others (3, 4, 6, 7)
require operator-level instrumentation and should be the first thing done
before committing 2-4 weeks to native library work — especially #7, because
it determines whether any of this document's recommendations actually matter
for adidas/homedepot.

---

## 6. Recommended start order for the next 2-4 weeks

1. **Day 1 — validate the premise.** Do the fixture-upload experiment for
   adidas (open question #7). If Akamai accepts a captured Chrome 130
   sensor_data payload on our IP, fingerprint work is the right path. If not,
   stop and pivot to timing/behavior work instead.

2. **Day 2 — capture fixtures.** Run Chrome 130 on a reference machine (we
   have Node 22 + npm, so `puppeteer-core` + a downloaded Chrome 130 build
   will work). Capture: the samsclub canvas toDataURL; a CreepJS audio render
   at indices 4500–4600; the 256×256 triangle WebGL readPixels; `measureText`
   on ~100 emoji; the full CreepJS JSON output. Store under
   `crates/*/tests/fixtures/`.

3. **Days 3–4 — fix the two §0 bugs.** Set `length: 5000` in AudioParams;
   fix radial gradient two-circle form (or document that tiny_skia can't do
   it and treat as blocker for T1.1).

4. **Week 1 — T1.1 skia-safe integration.** Biggest single ROI item.

5. **Week 2 — T1.2 font stack.** Depends on T1.1 for the `drawText` code path
   but can start in parallel on the font database side.

6. **Week 3 — T1.3 audio node port + T1.4 finish OSMesa WebGL.** These are
   independent and can run in parallel with one engineer each.

7. **Week 4 — T1.5 worker plumbing + Tier 2 cleanups (CompressionStream,
   permissions, voices, enumerateDevices).**

Re-test the 4 blocked sites after each Tier 1 item lands. Expected behavior:
Akamai A2 upgrade after T1.1+T1.2+T1.3 are all in (canvas bytes + text bytes +
audio bytes are what gate the upgrade). Kasada token acceptance after
T1.1+T1.2+T1.4+T1.5 (canvas + fonts + webgl + workers).

If the 4 sites still fail after all five Tier 1 items land, the remaining gap
is almost certainly timing/behavior, not fingerprint — and that's a different
research document.
