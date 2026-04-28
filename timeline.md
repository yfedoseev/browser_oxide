# browser_oxide — Engine-Coherence Timeline

**Goal:** close the engine gap to Chrome 147 such that Tier-1 anti-bot vendors (Cloudflare BM, Akamai BMP, DataDome, Kasada, HUMAN, Imperva) score browser_oxide as coherent-Chrome on items 1–11 of `research_2026.md` §4.1.

**Architecture:** Option D — **Rust orchestration layer over the same C/C++ libraries Chrome uses.** No pure-Rust reimplementations of subsystems whose output is fingerprinted byte-for-byte (Skia, ANGLE, libpng, ICU, WebAudio DSP). Pure-Rust everywhere else (DOM tree, CSS cascade, networking, JS bridge, stealth, runtime, layout glue).

**Premise validated by research:** the diagnosis is engine-output coherence with a real Chrome 147 build, not IP reputation. Playwright MCP from the same machine succeeds with `navigator.webdriver=true`, `Runtime.enable`, automation banners — because it launches stock Google Chrome stable. Coherence dominates noise.

**Estimated total effort:** ~5 months for one engineer; ~3 months with two engineers in parallel on independent modules.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                Rust orchestration (browser_oxide owns)          │
│  ┌────────────────────┐  ┌────────────────────┐                 │
│  │ Page lifecycle     │  │ Stealth profiles   │                 │
│  │ Event loop         │  │ Behavior synth     │                 │
│  │ Navigation         │  │ Cookie persistence │                 │
│  └────────────────────┘  └────────────────────┘                 │
│                                                                 │
│  ┌────────────────────────────────────────────┐                 │
│  │ DOM tree (arena, NodeId u32) — pure Rust   │                 │
│  │ CSS cascade / selectors — pure Rust        │                 │
│  │ HTML parser — pure Rust (html5ever)        │                 │
│  │ Layout glue (LayoutUnit fixed-point)       │                 │
│  └────────────────────────────────────────────┘                 │
│                                                                 │
│  ┌─────────────────┐  ┌─────────────────┐  ┌────────────────┐   │
│  │ Net (rquest+    │  │ JS bridge       │  │ Workers        │   │
│  │  BoringSSL)     │  │ (deno_core ops) │  │ (per realm)    │   │
│  └─────────────────┘  └─────────────────┘  └────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│            Bound C/C++ libraries (same as Chrome)               │
│  V8 14.x (rusty_v8 pinned to Chrome 147 build)                  │
│  Skia (skia-safe) — text shaping, canvas raster                 │
│  HarfBuzz + FreeType / CoreText / DirectWrite (via Skia)        │
│  libpng + zlib level-6 (libpng-sys vendored)                    │
│  ANGLE (direct binding) OR wgpu+naga (alternative)              │
│  ICU 75 full data                                               │
│  WebAudio DSP (port Chromium's audio module, or bind)           │
└─────────────────────────────────────────────────────────────────┘
```

The TLS+H2 byte-identical Chrome 147 work via rquest+BoringSSL — already winning — stays exactly where it is. That's the project's existing moat.

---

## Phase 0 — Quick wins (Week 1)

No architectural cost. These are diagnostic tells that gate every fingerprint test before they reach the deeper subsystems.

| # | Item | File | Effort |
|---|---|---|---|
| 0.1 | `getComputedStyle()` returning visible properties (`color`, `backgroundColor`, `display`, `visibility`, `opacity`, `font*`, `width`, `height`, `margin*`, `padding*`, `border*`) | `crates/js_runtime/src/js/dom_bootstrap.js` (new section) + `crates/css_cascade` | 2 d |
| 0.2 | `Element.matches()` and `Element.closest()` using existing CSS selector matching | `dom_bootstrap.js` | 0.5 d |
| 0.3 | `Element.scrollIntoView()` no-op (matches spec when no scroll container) | `dom_bootstrap.js` | 0.25 d |
| 0.4 | Real `scrollTop / scrollLeft` per-element state with setter persistence | `dom_bootstrap.js:570-573` + `crates/dom` | 1 d |
| 0.5 | `clientLeft / clientTop` returning border thickness from layout | `dom_bootstrap.js` + `crates/layout` | 0.5 d |
| 0.6 | `HTMLElement.style` as real `CSSStyleDeclaration` with per-property setters that persist | `dom_bootstrap.js` (replace `canvas_bootstrap.js:652` stub) | 1 d |
| 0.7 | `HTMLElement.dataset` as real `DOMStringMap` binding to `data-*` | `dom_bootstrap.js` | 0.5 d |
| 0.8 | Verify `eval.toString().length === 33` against deno_core V8 — fix if off | `tests/` | 0.25 d |
| 0.9 | Capture `Object.getOwnPropertyNames(globalThis)` from real Chrome 147 (mac, win, linux); diff against browser_oxide; stub the missing constructors with native-shape `Function.prototype.toString` | `tests/chrome147_globals.json` + `interfaces_bootstrap.js` | 1 d |
| 0.10 | Make `event.isTrusted` configurable; mark Rust-dispatched events as trusted | `event_bootstrap.js:13` + `crates/dom` | 0.25 d |

**Gate:** browser_oxide passes BotD's full suite of "easy" detectors. CreepJS `lies` score drops noticeably.

---

## Phase 1 — Per-frame V8 realms (Weeks 2–4)

**The architectural blocker.** Without this, every cross-realm probe (DataDome iframe trick, PerimeterX `iframe.contentWindow.Navigator !== Navigator`, Imperva, CreepJS) flags. With it, every subsequent gap-fix becomes testable.

### Work items

| # | Item | File | Effort |
|---|---|---|---|
| 1.1 | `JsRuntime` lifecycle refactor to support N concurrent `v8::Context`s within one isolate, indexed by `RealmId(u32)` | `crates/js_runtime/src/lib.rs` | 5 d |
| 1.2 | Every `#[op2]` op currently reading `OpState` accepts a `realm_id: RealmId` parameter; route per-realm state via `OpState::borrow::<RealmStore>()` | All op modules in `crates/js_runtime`, `crates/net`, `crates/workers` | 4 d |
| 1.3 | iframe creation in `crates/browser/src/page.rs` allocates a new `JsRealm`, runs bootstraps in it | `crates/browser/src/page.rs` | 3 d |
| 1.4 | Bootstrap files become idempotent and parameterized — re-runnable in a fresh realm without polluting `globalThis` of other realms | `dom_bootstrap.js`, `window_bootstrap.js`, `canvas_bootstrap.js`, `event_bootstrap.js`, `streams_bootstrap.js`, `interfaces_bootstrap.js` | 4 d |
| 1.5 | `structured_clone.js` realm-aware: serializing across realms strips identity, deserializing rehydrates with the destination realm's prototypes | `structured_clone.js` | 2 d |
| 1.6 | Cross-realm `instanceof` works correctly: `iframe.contentWindow.Array.isArray([])` is `true`, `[] instanceof iframe.Array` is `false` | verify via test | 1 d |
| 1.7 | Realm-purity probe test in `tests/` — runs after every realm-touching change | new `tests/realm_purity.rs` | 1 d |

### Validation

```js
// All of these must hold after Phase 1
const f = document.createElement('iframe'); document.body.appendChild(f);
f.contentWindow.Navigator !== Navigator
f.contentWindow.Navigator.prototype !== Navigator.prototype
Object.getOwnPropertyNames(f.contentWindow.Navigator.prototype).join() ===
  Object.getOwnPropertyNames(Navigator.prototype).join()
f.contentWindow.Function.prototype.toString.call(window.fetch).includes('[native code]')
```

**Gate:** DataDome iframe-trick test passes. CreepJS `lies` score drops further.

---

## Phase 2 — LayoutUnit (Weeks 5–6)

Blink's 32-bit fixed-point with 6 fractional bits — resolution 1/64 px. **Akamai's "pHash" hashes `getBoundingClientRect` floats over `querySelectorAll('*')`. Without this, every Akamai-protected site fails.**

| # | Item | File | Effort |
|---|---|---|---|
| 2.1 | New `LayoutUnit(i32)` type with 6-bit shift conversion, all arithmetic ops, conversion to/from `f64` for JS exposure | `crates/layout/src/layout_unit.rs` (new) | 2 d |
| 2.2 | Replace `f32`/`f64` pixel coordinates throughout `crates/layout` with `LayoutUnit`; conversions only at public API boundary | `crates/layout/src/*.rs` | 5 d |
| 2.3 | `getBoundingClientRect`, `getClientRects`, `offsetLeft/Top/Width/Height`, `clientLeft/Top/Width/Height`, `scrollLeft/Top/Width/Height` all return JS-floats produced via `LayoutUnit::to_f64()` | `crates/js_runtime/src/ops/layout.rs` | 1 d |
| 2.4 | Sub-pixel round-trip test: `width:1.3px` div returns `1.296875` (= 83/64) | `tests/sub_pixel_layout.rs` | 1 d |
| 2.5 | Akamai pHash parity test — render a fixture page with mixed sub-pixel borders/margins, hash all rects, compare against captured Chrome 147 hash | `tests/akamai_phash.rs` | 1 d |

**Gate:** sub-pixel test passes; Akamai pHash matches Chrome 147 fixture for at least one reference page.

---

## Phase 3 — Skia binding for canvas (Weeks 7–10)

The single most-detected engine signal. Custom canvas implementations cannot match Chrome's PNG output byte-for-byte; binding to Skia + libpng+zlib level-6 makes byte-identity automatic.

| # | Item | File | Effort |
|---|---|---|---|
| 3.1 | Add [`skia-safe`](https://github.com/rust-skia/rust-skia) dependency; vendor build for cross-platform reproducibility | `Cargo.toml`, CI | 2 d |
| 3.2 | Replace `crates/canvas` raster backend with Skia. Maintain the existing public API to `canvas_bootstrap.js` | `crates/canvas/src/lib.rs` | 5 d |
| 3.3 | Implement Canvas 2D path ops via Skia: `arc`, `arcTo`, `bezierCurveTo`, `quadraticCurveTo`, `strokeText`, `clip`, `isPointInPath`, `isPointInStroke` (replace stubs in `canvas_bootstrap.js:61-137`) | `canvas_bootstrap.js` + `crates/canvas` | 4 d |
| 3.4 | `measureText` returning full TextMetrics from Skia's `SkParagraph` shaper | `crates/canvas` | 2 d |
| 3.5 | `toDataURL('image/png')` via libpng+zlib level-6 vendored — no `pHYs/tIME/tEXt/iTXt`, single IDAT, byte-deterministic | `crates/canvas` | 3 d |
| 3.6 | `getImageData` with un-premultiply at extraction (matches Chrome behavior) | `crates/canvas` | 1 d |
| 3.7 | `OffscreenCanvas` shares the same Skia surface configuration as `HTMLCanvasElement` so PNG output is byte-identical (CreepJS verifies this) | `canvas_bootstrap.js` + `crates/canvas` | 1 d |
| 3.8 | Canvas fixture test — render the CreepJS Picasso paint scene, hash output, compare against captured Chrome 147 hash per OS/GPU | `tests/canvas_chrome147.rs` | 2 d |

**Gate:** CreepJS canvas hash matches Chrome 147 fixture. fp-collect `tpCanvas` test passes (`[255,255,255,255]` round-trip).

---

## Phase 4 — WebGL via wgpu+naga or ANGLE (Weeks 11–16)

**Decision point at week 11.** Two paths:

### Path 4a — wgpu + naga (Rust-native, easier, ~5 weeks)

Use [`wgpu`](https://github.com/gfx-rs/wgpu) for hardware GL access and [`naga`](https://github.com/gfx-rs/naga) for shader translation. Renderer string is faked to ANGLE-format using the actual host GPU info.

| # | Item | File | Effort |
|---|---|---|---|
| 4a.1 | `wgpu` integration; map WebGL1/WebGL2 commands to `wgpu` operations | `crates/js_runtime/src/ops/webgl.rs` | 8 d |
| 4a.2 | `naga`-based GLSL → SPIR-V shader compilation; real `compileShader`, `getShaderInfoLog` | `crates/js_runtime/src/ops/webgl.rs` | 5 d |
| 4a.3 | Fake `UNMASKED_RENDERER_WEBGL` to ANGLE-format string using host GPU info from `wgpu::AdapterInfo` (e.g., `ANGLE (Apple, ANGLE Metal Renderer: Apple M1 Pro, Unspecified Version)`) | `canvas_bootstrap.js` | 2 d |
| 4a.4 | All ordered `getParameter` values match Chrome 147 desktop (`MAX_TEXTURE_SIZE`, `MAX_VIEWPORT_DIMS`, `ALIASED_LINE_WIDTH_RANGE`, `getShaderPrecisionFormat`) | profile-driven, fixture-asserted | 2 d |
| 4a.5 | Worker `OffscreenCanvas` WebGL renderer matches main-thread (CreepJS hasBadWebGL) | `worker_bootstrap.js` + `canvas_bootstrap.js` | 2 d |
| 4a.6 | Triangle pixel-readback fixture test — render the CreepJS test triangle, hash readback bytes, compare against Chrome 147 reference | `tests/webgl_chrome147.rs` | 2 d |
| 4a.7 | `getSupportedExtensions().sort().join(',')` SHA-256 matches Chrome 147 per-GPU reference | profile-driven | 1 d |

**Risk:** triangle pixel-readback may not hash-match real ANGLE because rasterizer rounding differs. Mitigation: probabilistic check (Hamming distance < N) instead of exact-match for the readback hash, since vendors don't always exact-match either.

### Path 4b — ANGLE direct binding (~10 weeks)

Bind ANGLE's C++ API; pass GLSL through ANGLE's translator; render via ANGLE backend (D3D11/Metal/GL/Vulkan as Chrome does). Heavier but produces byte-identical output.

| # | Item | Effort |
|---|---|---|
| 4b.1 | Vendor ANGLE source; build static lib for each target platform | 3 wk |
| 4b.2 | Rust FFI bindings via `bindgen` | 1 wk |
| 4b.3 | All of 4a but routed through ANGLE | 4 wk |
| 4b.4 | Test parity at byte level | 2 wk |

**Recommendation:** ship Path 4a first. If specific Tier-1 vendors gate on bit-exact ANGLE output (verify empirically), upgrade to Path 4b later.

**Gate:** `gl.getParameter(UNMASKED_RENDERER_WEBGL)` matches Chrome 147 host. CreepJS WebGL probe passes. Worker WebGL parity holds.

---

## Phase 5 — WebAudio DSP parity (Weeks 17–19)

OfflineAudioContext compressor sum must land in CreepJS's `KnownAudio` table (`124.04347657808103` macOS arm64, `124.04344968475198` Linux x64, etc).

| # | Item | File | Effort |
|---|---|---|---|
| 5.1 | Port Chromium's `DynamicsCompressor` algorithm from `third_party/blink/renderer/modules/webaudio/dynamics_compressor.cc` to Rust, byte-faithful | `crates/canvas/src/audio/compressor.rs` (new) | 5 d |
| 5.2 | Port Chromium's `BiquadFilter` and `AnalyserNode` (FFT) similarly | `crates/canvas/src/audio/biquad.rs`, `analyser.rs` | 4 d |
| 5.3 | Port `OscillatorNode` waveform tables (sine/square/sawtooth/triangle) — Chrome uses pre-computed PeriodicWaves | `crates/canvas/src/audio/oscillator.rs` | 2 d |
| 5.4 | Replace seed-driven static rendering in `canvas_bootstrap.js:580-633` with real DSP path | `canvas_bootstrap.js` | 1 d |
| 5.5 | Realtime AudioContext path plumbed (currently only Offline works) | `canvas_bootstrap.js:418-485` + `crates/canvas` | 3 d |
| 5.6 | Audio fixture test — run the CreepJS-canonical graph, assert sum matches one of `KnownAudio` floats per OS | `tests/audio_chrome147.rs` | 1 d |

**Gate:** CreepJS audio probe lands in `KnownAudio` table. Silent oscillator returns all-zero samples (CreepJS `hasFakeAudio`).

---

## Phase 6 — V8 alignment with Chrome 147 (Weeks 20–22)

deno_core 0.311 ships V8 with Deno's snapshot, ICU 73-74, and Deno's feature flags. Chrome 147 ships V8 ≈ 14.1 with full ICU 75 and Chrome's flags. Detectable deltas: `Error.stack` format, `Intl` resolved options, `Math.cos(13*Math.E)` last-bit, harmony flag set.

| # | Item | File | Effort |
|---|---|---|---|
| 6.1 | Pin `rusty_v8` to a V8 build matching Chrome 147's V8 (14.x); document the bump cadence | `crates/js_runtime/Cargo.toml` | 2 d |
| 6.2 | Disable harmony features Chrome 147 doesn't ship (`Temporal` is the big one — origin trial only). Whitelist Chrome 147's flag set | `crates/js_runtime/src/lib.rs` (V8 init) | 1 d |
| 6.3 | Vendor full ICU 75 data file; register with V8 at startup | `crates/js_runtime` | 2 d |
| 6.4 | `Error.prepareStackTrace` set to strip `deno:`, `rust:`, `<bo_internal>` paths from frames; emit Chrome-format `at fn (url:line:col)` | `window_bootstrap.js` (early init) | 1 d |
| 6.5 | Math constant fixture test — `Math.cos(13*Math.E)`, `Math.acos(0.123)`, etc. against Chrome 147 reference values from CreepJS `math.ts` | `tests/math_chrome147.rs` | 1 d |
| 6.6 | Intl fixture test — `Intl.DateTimeFormat / NumberFormat / DisplayNames / RelativeTimeFormat` per-locale outputs match Chrome 147 | `tests/intl_chrome147.rs` | 2 d |
| 6.7 | `eval.toString().length === 33` and `Function.prototype.toString.call(Math.sin) === 'function sin() { [native code] }'` — verify across all natives | `tests/v8_natives.rs` | 1 d |

**Gate:** CreepJS engine, math, intl, errors modules pass against Chrome 147 fixtures.

---

## Phase 7 — API completeness pass (Weeks 23–26)

Chrome 147 has ~1080–1110 properties on `window` and ~57 on `Navigator.prototype`. browser_oxide currently has a fraction. Fill in the rest with native-shape stubs (most don't need real implementations — just to exist with correct prototype/`toString`).

| # | Item | File | Effort |
|---|---|---|---|
| 7.1 | Capture `Object.getOwnPropertyNames(window)` and full prototype chains from real Chrome 147 on macOS/Win/Linux | `tests/fixtures/chrome147_*.json` | 1 d |
| 7.2 | Stub missing constructors as `class X { constructor(){throw new TypeError('Illegal constructor')} }` with `Symbol.toStringTag = 'X'`. Targets: `EditContext`, `Highlight`, `HighlightRegistry`, constructible `CSSStyleSheet`, `CookieStore`, `IdentityCredential`, `CredentialsContainer`, `WGSLLanguageFeatures`, `XRSession`, `LaunchQueue`, `WebTransport`, `EventSource`, `BroadcastChannel`, `MessageChannel`, `MessagePort`, `BatteryManager`, `Geolocation`, `XSLTProcessor`, `DOMParser`, `XMLSerializer`, `Range`, `StaticRange`, `AbortController`, `AbortSignal`, `FileSystemHandle`, `FileSystemFileHandle`, `FileSystemDirectoryHandle`, `Notification`, `PushManager`, `BackgroundFetchManager`, `PaymentRequest`, `PresentationConnection`, `Accelerometer`, `Gyroscope`, `LinearAccelerationSensor`, `OrientationSensor`, `RelativeOrientationSensor`, `AbsoluteOrientationSensor` | `interfaces_bootstrap.js` (new big section) | 8 d |
| 7.3 | `navigator.gpu` returning a real WebGPU adapter via `wgpu` (already installed in Phase 4a) | `window_bootstrap.js` | 3 d |
| 7.4 | `navigator.credentials`, `navigator.locks`, `navigator.storage`, `navigator.serviceWorker` as functional-enough stubs (matching shape, methods reject promise where Chrome does) | `window_bootstrap.js` | 4 d |
| 7.5 | `navigator.hardwareConcurrency` returns host count via `op_host_concurrency`; `deviceMemory` returns one of `0.25, 0.5, 1, 2, 4, 8` based on host RAM | `window_bootstrap.js` | 1 d |
| 7.6 | `WorkerNavigator instanceof Navigator` parity — Worker realm has its own `Navigator.prototype` with full getter set | `worker_bootstrap.js` | 2 d |
| 7.7 | `Object.getOwnPropertyNames(Navigator.prototype)` insertion order matches Chrome 147 reference (~57 entries in exact order) | `dom_bootstrap.js` ordering audit | 1 d |
| 7.8 | `chrome.loadTimes() / chrome.csi()` return populated objects with realistic timing data | `window_bootstrap.js:1007-1038` | 1 d |
| 7.9 | API-completeness fixture — `Object.getOwnPropertyNames(window).length` within ±5 of Chrome 147 reference per OS | `tests/api_completeness.rs` | 1 d |

**Gate:** CreepJS `features` module recognizes the engine as Chrome 147. fp-collect `navigatorPrototype` walk matches Chrome reference.

---

## Phase 8 — Behavioral and IO polish (Weeks 27–28)

Last-mile items not covered elsewhere.

| # | Item | File | Effort |
|---|---|---|---|
| 8.1 | `IntersectionObserver` fires on actual layout intersection, not synchronously — match Chrome's microtask scheduling | `window_bootstrap.js:2426-2509` | 2 d |
| 8.2 | `ResizeObserver` reports real geometry from layout, not synthetic | `window_bootstrap.js` | 1 d |
| 8.3 | `performance.memory.jsHeapSizeLimit === 4294705152` (constant Chrome desktop value), no jitter | `window_bootstrap.js`, `worker_bootstrap.js:80-93` | 0.25 d |
| 8.4 | Native event firing — `load`, `unload`, `beforeunload`, `pagehide`, `pageshow`, `popstate`, `hashchange`, `storage` fire automatically on the right lifecycle moments | `crates/browser/src/page.rs` + `event_bootstrap.js` | 3 d |
| 8.5 | `postMessage` transferables actually neuter the source `ArrayBuffer` | `structured_clone.js:331` | 1 d |
| 8.6 | `navigator.connection.rtt` rounded to 25 ms, `downlink` to 25 kbps (Chrome's quantization) | `window_bootstrap.js` | 0.5 d |
| 8.7 | Behavioral warm-up choreographer — 2–4 s idle + Sigma-Lognormal mouse drift + one scroll + one focus event before first script-driven action | `crates/stealth` (extend existing `behavior.rs`) | 2 d |
| 8.8 | Capture and replay full Chrome 147 fixture suite (§5.1 in research_2026.md) on macOS arm64, macOS x64, Windows 11, Ubuntu 24.04 | `tests/fixtures/`, CI matrix | 3 d |

**Gate:** all 30 fixture rows from `research_2026.md` §5.1 pass on the relevant OS profile.

---

## Phase 9 — Validation against the 7 CHL sites (Week 29)

Real-world sign-off.

| # | Item | Effort |
|---|---|---|
| 9.1 | Run the 30-site verification matrix; confirm previous L3 sites still pass | 1 d |
| 9.2 | Run the 7 CHL sites: canadagoose, hyatt, adidas, zillow, wildberries, ozon, douyin | 1 d |
| 9.3 | For any remaining failures, compare side-by-side with Playwright MCP capture (request headers, response cookies, JS challenge payloads) and identify the residual delta | 2 d |
| 9.4 | Targeted fix or write up "site-specific quirk" against the engineering backlog | 1 d |

**Success criterion:** at least 5 of 7 CHL sites move to L3 PASS. Any remaining failures have a documented root cause that is not a generic engine gap.

---

## Dependency graph

```
P0 (independent quick wins)
  │
  ▼
P1 (per-frame realms) ──┬──► P3 (Skia canvas)
                        ├──► P4 (WebGL)
                        ├──► P5 (WebAudio)
                        ├──► P6 (V8 alignment)
                        └──► P7 (API completeness)
P2 (LayoutUnit) ────────┘                ▲
                                         │
                                         P8 (behavioral polish)
                                         │
                                         P9 (CHL validation)
```

P0, P1, P2 are sequential. P3–P6 can run in **parallel** if you have multiple engineers — they're independent subsystem bindings. P7, P8 depend on P1 and any of P3–P6 they overlap with. P9 is the final integration gate.

**One engineer, sequential:** ~29 weeks.
**Two engineers, P3+P4 parallel + P5+P6 parallel:** ~16 weeks.
**Three engineers, P3+P4+P5 parallel then P6+P7 parallel:** ~13 weeks.

---

## Risk register

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| skia-safe build is fragile cross-platform | Medium | Medium | Vendor pre-built binaries per platform; CI builds verify |
| wgpu+naga pixel output doesn't hash-match real ANGLE | High | Medium | Fall back to Path 4b (direct ANGLE binding) for sites that probe pixel-byte equality |
| Per-frame realm refactor introduces regressions in cookie/storage/Worker plumbing | High | High | Comprehensive realm-purity test suite before any other Phase 1 work lands; bisect on regression |
| Chrome 147 ships a new constructor we miss; site adopts it; we stay vulnerable | Continuous | Low–Medium | Quarterly fixture refresh against latest Chrome stable; CI alerts on Chrome version skew |
| deno_core's V8 version drifts ahead of Chrome 147 (bringing in `Temporal` etc.) | Medium | Medium | Pin rusty_v8 to a Chrome 147-aligned commit; explicit feature-flag whitelist |
| WebAudio compressor port has subtle FP-rounding bugs that put sum outside KnownAudio table | Medium | High | Port from Chromium source byte-faithfully (LICENSE: BSD-3); compare bit-by-bit against Chrome reference recordings |
| Akamai sensor v3 rotates and adds a probe we haven't covered | Continuous | Medium | Subscribe to anti-bot research feeds (glizzykingdreko, scrapfly, datadome blog, kasada blog); refresh probe inventory quarterly |
| Skia API drift between minor versions breaks builds | Low | Low | Pin skia-safe version; bump deliberately |
| One subsystem takes 2× the estimate | Medium | Medium | Phase gates allow re-planning without blocking other phases (since P3–P6 are parallelizable) |

---

## License posture

All bound libraries are permissive-compatible with browser_oxide's MIT/Apache stance:

| Library | License | Compatible? |
|---|---|---|
| V8 (rusty_v8) | BSD-3 | ✓ |
| Skia (skia-safe) | BSD-3 | ✓ |
| HarfBuzz | MIT | ✓ |
| FreeType | FreeType / GPLv2 dual | ✓ (FreeType license) |
| libpng | libpng | ✓ |
| zlib | zlib | ✓ |
| ANGLE | BSD-3 | ✓ |
| wgpu / naga | MIT/Apache | ✓ |
| ICU | Unicode-DFS-2016 | ✓ |
| Chromium DynamicsCompressor source (port) | BSD-3 | ✓ |

**No MPL.** Servo's layout, Servo's Skia bindings (servo-fork), Camoufox (Firefox-fork) — all MPL — remain off-limits for code reuse but are valid for design reference.

---

## Operating principles (carry forward)

- **Never expose CDP, WebDriver, or BiDi.** browser_oxide's only durable moat against the next wave of detection is the architectural absence of these surfaces. Do not regress.
- **Never implement automation extensions in JS surface.** If automation is needed, expose Rust-side API only.
- **TLS+H2 byte-identical Chrome parity stays the ground truth at the network layer.** rquest+BoringSSL pinned to Chrome 147, BoringSSL revision tracked against Chromium DEPS, JA4 drift CI test.
- **Fixture-driven validation.** Every claim of Chrome parity is backed by a captured Chrome 147 fixture in `tests/fixtures/` and a CI assertion. No hand-waving.
- **One realm per browsing context, always.** Once Phase 1 lands, do not regress to shared globals across iframes/workers.

---

## What success looks like

At Week 29, browser_oxide:

- Passes the 30-output Chrome 147 fixture suite on macOS arm64, macOS x64, Windows 11, Ubuntu 24.04.
- Lands inside CreepJS's `KnownAudio` table, matches CreepJS canvas hash for the host OS+GPU, passes BotD's full detector list, passes fp-collect navigator-prototype walk, passes rebrowser-bot-detector's realm-purity probes.
- Verifies at least 5 of the 7 CHL sites — canadagoose, hyatt, adidas, zillow, wildberries, ozon, douyin — at L3 PASS using only browser_oxide (no Playwright fallback).
- Maintains the existing 17 L3 PASS sites and CI/CD green.
- Memory-per-worker stays under 100 MB (vs. Chrome's ~300 MB) — the project's structural advantage over CDP-Chrome stacks is preserved.

That's a Rust stealth browser that beats Chromium-fork stacks on the metrics that matter (memory, startup time, license cleanliness, TLS parity) while matching them on the metric that's been the blocker (engine-output coherence). That's the project at the destination.
