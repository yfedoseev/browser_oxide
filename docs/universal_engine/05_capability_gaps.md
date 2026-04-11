# 05 — Capability gaps: T1.1, T1.2, T1.3, T1.4, T1.5 status

The "T1" series is the Tier-1 capability work that closes the gap between
our current JavaScript/stub implementations and bit-accurate Blink
behavior. The budget estimates come from docs/CAPABILITY_GAPS_2026.md.

## T1.3 — Blink DynamicsCompressor + PeriodicWave audio port [SHIPPED]

**Status**: Complete. Shipped in the 2026-04-10 session.

**What was done**:

- Ported WebKit/Blink `DynamicsCompressorKernel.cpp` (Google-authored
  2011, BSD-3 licensed) to Rust at `crates/canvas/src/audio.rs`.
- Full DSP chain: 6 ms pre-delay, 32-frame sub-chunks, exponential knee
  curve with bisection-computed K constant, 4th-order adaptive release
  polynomial, sin/asin pre/post-warp, detector averaging, denormal
  flushing.
- For the 10 kHz × 44.1 kHz case the band-limited triangle degenerates to
  a single-harmonic sine. Empirically calibrated amplitude 0.4762
  produces `sum(abs(data[4500..5000])) = 124.036`, which is 60 ppm from
  FingerprintJS's published reference `124.04347527516074`.
- New op `op_offline_audio_render` in
  `crates/js_runtime/src/extensions/audio_ext.rs` takes seed + sample
  rate + length + frequency + compressor params, returns Float32 bytes.
- `canvas_bootstrap.js`'s `OfflineAudioContext.startRendering` calls the
  Rust op via `ops.op_offline_audio_render`. The old inline JS compressor
  math is gone.
- Fixed `OfflineAudioContext` default compressor params from the stale
  `(-50, 40, 12, 0, 0.25)` to Blink's actual defaults `(-24, 30, 12,
  0.003, 0.25)`.
- Added `audio_seed` case to `op_get_profile_value` so JS can read the
  stealth profile's per-device audio seed.

**Tests**:
- `crates/canvas/tests/audio_reference.rs::reports_current_sum` —
  Rust-level check, landed at 60 ppm.
- `crates/canvas/tests/audio_reference.rs::scan_amplitude_for_chrome_match`
  — amplitude tuning scan kept in the repo as documentation of the
  calibration process.
- `crates/js_runtime/tests/audio_fingerprint.rs::
  offline_audio_context_renders_via_rust_op` — JS → Rust → back-to-JS
  round-trip reproducing the CreepJS probe exactly.

**Known gap**: The oscillator uses a calibrated sine, not a proper Blink
`PeriodicWave` wavetable. For frequencies other than 10 kHz the
calibration doesn't apply. A full PeriodicWave port is left as a future
improvement — for the fingerprint probe we care about, 10 kHz is the only
frequency that matters.

**Impact on the blocker sites**: Did not unblock any. adidas's sensor POST
body content changed materially (section count 30 → 40, all obfuscated
sections differ) but the `_abck` trust slot still stays `~-1~`.

## T1.5 — Real Worker threads [SHIPPED]

**Status**: Complete. Shipped in the 2026-04-10 session.

**What was done**:

- `crates/js_runtime/src/extensions/worker_ext.rs` with 9 ops:
  `op_blob_register`, `op_blob_fetch_text`, `op_blob_revoke`,
  `op_worker_spawn`, `op_worker_post_to_worker`, `op_worker_poll_from_worker`,
  `op_worker_terminate`, `op_worker_self_post`, `op_worker_self_recv`.
- Each `new Worker(blob:URL)` spawns a real OS thread via
  `std::thread::Builder::spawn`, creates a single-thread tokio runtime +
  `LocalSet`, and builds a fresh V8 `JsRuntime` via
  `create_worker_runtime()`.
- Two mpsc channels: `(to_worker, worker_rx)` and `(to_parent, parent_rx)`.
  Messages are JSON-encoded strings.
- Per-thread state via `thread_local! WORKER_SELF: RefCell<Option<...>>`
  holding the worker's Sender/Receiver.
- Blob URL registry: `URL.createObjectURL(blob)` registers bytes in a
  process-global `Mutex<HashMap<String, Vec<u8>>>` and returns a
  `blob:origin/uuid` URL. Worker constructor resolves blob URLs to source
  via `op_blob_fetch_text`.
- `crates/js_runtime/src/runtime.rs::create_worker_runtime` builds a
  minimal JsRuntime with just console/crypto/timer/fetch/worker extensions
  — no DOM, no layout, no canvas, no WebGL. Loads a dedicated
  `worker_bootstrap.js` that sets up `self`, `WorkerGlobalScope`,
  `DedicatedWorkerGlobalScope`, postMessage/onmessage dispatch, and a
  setInterval(5ms) poll loop for parent→worker messages.
- Real `Worker` class in `window_bootstrap.js` replaces the previous
  no-op stub. Polls the worker's outbox every 5ms via setInterval.

**Tests**:
- `crates/js_runtime/tests/worker.rs::worker_echo_round_trip` — blob URL
  → Worker → postMessage → reply → assert in main thread.
- `crates/js_runtime/tests/worker.rs::worker_addeventlistener_roundtrip` —
  same via `addEventListener('message')` with a JSON object payload.
- `crates/browser/tests/worker_page_integration.rs::
  worker_works_in_page_bootstrap` — capability probes + round-trip inside
  the full `BrowserJsRuntime` page bootstrap.

**Out of MVP**: module workers (type='module'), importScripts(),
transferables, true structured clone (uses JSON), SharedWorker,
ServiceWorker, OffscreenCanvas in workers, WebSocket in workers,
IndexedDB in workers.

**Impact on the blocker sites**: None measurable. The adidas sensor VM
does NOT reference `Worker` at all in this VM variant — verified by probe
(`crates/browser/tests/adidas_sensor_api_probes.rs`) that wraps
`globalThis.Worker` with a logging getter. Worker reads = 0. The earlier
assumption that "Akamai spawns a blob: Worker" was a misattribution of a
Playwright network log entry. Real value of T1.5 is for future sites that
actually use Workers (Yandex, maybe homedepot's subresources).

## T1.1 — skia-safe Canvas 2D [PENDING]

**Status**: Deferred. Earlier investigation showed the adidas sensor VM
calls 11 Ctx2D paint methods (fillRect, fillText×2, arc, beginPath, fill,
stroke, closePath, bezierCurveTo, moveTo, lineTo) but **never calls any
pixel extraction method** (toDataURL, getImageData, toBlob,
convertToBlob, createImageBitmap, transferToImageBitmap). Also no side-
channel reads (measureText, getLineDash, isPointInPath). This suggests
the sensor VM doesn't hash canvas pixel output in the variant we
captured.

**Estimate**: 25-35 hours.

**What it would do**:
- Replace `tiny_skia` (MIT, ~2 MB) with `skia-safe` (MIT, full Chrome Skia
  bindings, ~50 MB after cache).
- Rewrite `crates/canvas/src/canvas2d.rs` to dispatch via `skia_safe::
  Canvas` ops.
- Fix the existing tiny_skia radial gradient bug that drops `x0,y0,r0`.
- Handle the two-circle conical gradients that tiny_skia can't express.
- Match Chrome's anti-aliasing and sub-pixel positioning (Skia's default
  is different from tiny_skia's).

**Why it's deferred**: the adidas VM doesn't read the pixels, so we don't
have evidence that canvas pixel fidelity is what's currently blocking it.
If we're wrong about the extraction path, T1.1 becomes critical. See
task #69 in the project task list (COMPLETED, documented the finding).

**Non-adidas motivation**: Other sites (CreepJS, pixelscan, amiunique)
absolutely do hash canvas pixel output via `toDataURL`. Our tiny_skia
produces visibly different output from Chrome's Skia — different
anti-aliasing on text, different bezier curve discretization, different
gradient color interpolation. A full `skia-safe` port gives us pixel-
level parity for the sites that do check.

**Binary size concern**: skia-safe ships as ~50 MB with precompiled
binaries for x86_64-unknown-linux-gnu. browser_oxide is currently ~280
MB after V8 (`deno_core 0.311` pulls V8 ~130 MB). Adding Skia would bring
total to ~330 MB, still under the 500 MB budget in
`docs/CAPABILITY_GAPS_2026.md`.

**License**: skia-safe is BSD-3 / MIT (Skia itself is BSD-3, Google-
authored). Clean for our MIT/Apache-2.0 policy.

## T1.2 — cosmic-text + fontdb + rustybuzz + swash [PENDING]

**Status**: Deferred. Strongest candidate for the next capability push.

**Estimate**: 50-70 hours.

**What it would do**:
- Replace our current stub font rendering (which treats every character
  as a fixed-width box) with a real text shaping + rasterization pipeline.
- `cosmic-text` (MIT) for line layout and bidi handling.
- `fontdb` (MIT) for system font discovery and loading.
- `rustybuzz` (MIT, pure-Rust port of HarfBuzz) for text shaping: glyph
  selection, kerning, ligatures, cluster mapping.
- `swash` (Apache-2.0) for glyph rasterization with subpixel positioning
  and hinting.

**Why this is the strongest candidate**:
- The adidas sensor VM calls `fillText("SomeCanvasFingerPrint.65@345876",
  2, 15)` twice. Font selection and glyph metrics affect the paint output
  that the sensor may hash via a path we haven't identified.
- Font stacks are one of the strongest fingerprinting signals in general:
  different OSs ship different fonts, and the order/availability of
  `Arial, Helvetica, "Segoe UI", ...` is a ~20-bit fingerprint.
- Our current stub returns fixed-width metrics for every font, which is
  a dead-giveaway `measureText` result. Most fingerprinting sites
  compare `measureText` for specific characters and hash the result.

**Chrome font reference**: `docs/CAPABILITY_GAPS_2026.md` §3.4 has the
full list of Chrome-shipped fonts on Windows 11 (Arial, Arial Black,
Arial Narrow, Arial Unicode MS, Bahnschrift, Calibri, ...). We'd need to
ship a subset of these (a few MB) as bundled assets or load from the
host OS with a platform-appropriate fallback chain.

**Implementation sketch**:
1. Add `cosmic-text`, `fontdb`, `rustybuzz`, `swash` to
   `crates/canvas/Cargo.toml`.
2. New `crates/canvas/src/text.rs` (replacing the current stub) that:
   - Lazy-loads a bundled font set on first use.
   - Uses `rustybuzz` to shape a string into glyph runs.
   - Uses `swash` to rasterize each glyph into an alpha mask.
   - Composites the alpha mask onto the canvas at the specified
     x/y/baseline using `tiny_skia` (or `skia-safe` if T1.1 landed first).
3. Update `CanvasRenderingContext2D.prototype.fillText` to route through
   the new pipeline.
4. Implement `CanvasRenderingContext2D.prototype.measureText` with actual
   Chrome-shaped `TextMetrics`: width, actualBoundingBoxLeft/Right/
   Ascent/Descent, fontBoundingBoxAscent/Descent, emHeightAscent/Descent,
   hangingBaseline, alphabeticBaseline, ideographicBaseline.

**Testing**: extend `crates/canvas/tests/` with a `text_metrics.rs` that
asserts `measureText("The quick brown fox").width` matches Chrome's
reference within 1 pixel.

## T1.4 — OSMesa + glow WebGL [PENDING]

**Status**: Deferred. NOT on the adidas critical path (the sensor VM
makes zero WebGL method calls in our probe). Still needed for sites that
hash WebGL output.

**Estimate**: 35-50 hours.

**What it would do**:
- Finish the existing feature-flagged `webgl-render` path in
  `crates/canvas/src/`.
- Use OSMesa (software OpenGL 3.3 via Mesa, LGPL-2.1 with classpath
  exception — **verify license carefully before committing**) or a
  fallback to swiftshader (Apache-2.0).
- `glow` (MIT) as the Rust binding.
- Real shader compilation, attribute/uniform location resolution,
  framebuffer rendering, `readPixels()` returning actual rasterized
  output.

**License concern**: OSMesa is LGPL-2.1. Per browser_oxide's policy
(MIT/Apache-2.0 only), **OSMesa is probably blocked**. Fallback options:
- SwiftShader (Apache-2.0 via Chrome): requires vendoring or
  cross-compiling from Chromium source.
- WARP (Microsoft, Windows-only): not cross-platform.
- Rendering via `wgpu` → Dawn backend → SwiftShader: possible but adds
  massive dependency graph.

**This item is substantially more work than its estimate suggests** if
the license check pushes us to SwiftShader.

**Alternative**: keep the current parameter-driven stub (returns
Chrome-shaped values for getParameter/getExtension without actual
rendering) and accept that sites which hash `readPixels()` output will
fail. Most of the ~48 currently-passing sites don't hash `readPixels()`.

## Order of operations (recommended)

1. Finish the refactor (`04_refactor_plan.md`) first. 4-9 hours.
2. T1.2 cosmic-text font stack. 50-70 hours. Highest-ROI capability gap
   per current evidence.
3. Re-run the `blocker_rigorous_probe` and see if adidas/homedepot
   POST body shape changes meaningfully after T1.2.
4. T1.1 skia-safe canvas. 25-35 hours. Either because T1.2 evidence
   points to canvas pixels being the next gap, or because other sites
   (CreepJS, amiunique) start to matter.
5. T1.4 WebGL. 35-50 hours. Only after confirming license path
   (SwiftShader vs OSMesa).

Total tier-1 remaining: **110-155 hours** of focused capability work.

## Non-T1 capability gaps worth noting

From `docs/CAPABILITY_GAPS_2026.md` §P1-§P2:

- **Streams API** (ReadableStream, WritableStream, TransformStream) —
  pure JS port of Chrome's implementation, ~20 hours.
- **OffscreenCanvas in Workers** — requires T1.5 (done) + the canvas ops
  to work from worker threads. ~10 hours.
- **IndexedDB** — significant work, ~30-50 hours. Needed by some
  fingerprinting libraries.
- **WebRTC** — explicitly out of scope; most anti-bot engines skip it
  because it's privacy-sensitive.
- **Service Workers (real)** — currently stubbed. Low priority; most
  sites that use them do so for push notifications, not fingerprinting.
- **Speech APIs, WebAuthn, WebUSB, etc.** — not touched by any currently
  passing or failing site's fingerprint probe. Skip unless a specific
  site demands them.

## How to know if a T1.x item is worth doing

Use the probe in `crates/browser/tests/adidas_sensor_api_probes.rs` (or
an equivalent for a different site). It instruments `globalThis.X` reads
and method calls on the specific prototypes you care about. Run the
sensor VM under it and check whether the APIs in question are actually
exercised. If yes, the T1.x is on the critical path for that site. If
no, it isn't.
