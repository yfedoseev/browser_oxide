# 07 — Blink source pointers: where to find reference implementations

When you're porting a Web API to Rust, the source of truth is Chromium's
Blink implementation (BSD-3 licensed, directly portable). This file lists
the specific paths you'll need.

## How to fetch Blink source

The canonical Chromium repo is `chromium.googlesource.com/chromium/src`,
but for scripted fetching **use the WebKit mirror on GitHub** — the
Google-authored files are the same, and raw.githubusercontent.com is
more reliable:

```bash
curl -sL -o /tmp/file.cpp \
  "https://raw.githubusercontent.com/WebKit/WebKit/main/Source/WebCore/<path>"
```

For files that only exist in Chromium (not WebKit), fetch from Chromium's
GitHub mirror `github.com/chromium/chromium`. **Note**: that mirror is
sometimes stale or has 404s on `main`. Try specific release branches
(e.g., `main`, `132.0.6834.84`) or use the Googlesource web UI and copy-
paste.

## Audio — DynamicsCompressorKernel + PeriodicWave

**Used by T1.3 (shipped)**:

- `WebKit/Source/WebCore/platform/audio/DynamicsCompressorKernel.h`
- `WebKit/Source/WebCore/platform/audio/DynamicsCompressorKernel.cpp` —
  Google-authored 2011, 475 lines, BSD-3. Our port is
  `crates/canvas/src/audio.rs`. Rev-verified in the session.
- `WebKit/Source/WebCore/platform/audio/DynamicsCompressor.{h,cpp}` —
  wraps the kernel with parameter management. Contains the default
  `ParamThreshold = -24`, `ParamKnee = 30`, etc.
- `WebKit/Source/WebCore/platform/audio/AudioUtilities.{h,cpp}` —
  `linearToDecibels`, `decibelsToLinear`,
  `discreteTimeConstantForSampleRate` helpers.

**Still needed (not yet ported)**:

- `chromium/third_party/blink/renderer/platform/audio/periodic_wave.cc`
  — the full band-limited wavetable generator. Ports triangle/sine/
  square/sawtooth to Fourier coefficient arrays, inverse-FFTs them into
  time-domain wavetables, applies anti-aliasing by zeroing harmonics
  above Nyquist per-wavetable-range.
- `chromium/third_party/blink/renderer/modules/webaudio/oscillator_node.
  cc` — uses the PeriodicWave to sample per frame. Includes phase
  increment computation, detune handling, and cubic interpolation
  between adjacent wavetable samples.

**Why it matters**: our current shortcut uses a calibrated sine at
amplitude 0.4762 for the 10 kHz case. For any other frequency, the
calibration is wrong. Bit-accurate results for arbitrary frequencies
require the full wavetable port.

## Canvas 2D

**For T1.1 (pending)**:

- Skia itself is at `skia.googlesource.com/skia` (BSD-3). Don't try to
  port pieces of it — use the `skia-safe` crate which binds the whole
  thing.
- Blink's wrapper: `chromium/third_party/blink/renderer/modules/canvas/
  canvas2d/canvas_rendering_context_2d.cc`. Mostly just forwards to
  Skia; useful to understand which Skia flags Blink sets (e.g.,
  `kAntialias`, `kLowPriorityFence`, etc.).
- `chromium/third_party/blink/renderer/platform/graphics/` — contains
  Blink's color space handling, gradient construction, path rasterization
  config. Worth reading if your `skia-safe` output diverges from Chrome.

## Fonts and text

**For T1.2 (pending)**:

- `chromium/third_party/blink/renderer/platform/fonts/` — Blink's font
  matching, fallback chains, and shaping interface. Key file:
  `font_fallback_list.cc`.
- HarfBuzz (the shaping engine Blink uses): `github.com/harfbuzz/harfbuzz`
  — written in C++. `rustybuzz` is a pure-Rust port of a snapshot (no
  active HarfBuzz upstream sync; we'd freeze at whatever rustybuzz
  version is current).
- Chromium's font list per OS: `chromium/third_party/test_fonts/
  fonts.conf` and `chromium/ui/gfx/platform_font_skia.cc` (Linux), plus
  `platform_font_win.cc` and `platform_font_mac.h`.

## WebGL

**For T1.4 (pending)**:

- `chromium/third_party/blink/renderer/modules/webgl/
  webgl_rendering_context.cc` — the JS-exposed WebGL 1.0 context.
- `chromium/third_party/blink/renderer/modules/webgl/
  webgl2_rendering_context.cc` — WebGL 2.0 additions.
- `chromium/gpu/command_buffer/service/` — Chrome's GLES2 command
  decoder that sits between Blink and the actual driver. Extremely
  complex; don't try to port this.
- Alternative: use SwiftShader (`swiftshader.googlesource.com/
  SwiftShader`, Apache-2.0) as a software GL backend and drive it via
  the `glow` Rust crate.

## Web Workers

**Already shipped (T1.5)**:

- `chromium/third_party/blink/renderer/core/workers/` — contains
  `worker_global_scope.cc`, `dedicated_worker.cc`, `worker_thread.cc`.
  Our implementation in `crates/js_runtime/src/extensions/worker_ext.rs`
  uses the same structural pattern: one OS thread per worker, message
  channels for communication, init scripts run on the new isolate
  before any worker-script execution.

## Navigator and Window

- `chromium/third_party/blink/renderer/core/frame/navigator.cc`
- `chromium/third_party/blink/renderer/core/frame/local_dom_window.cc`
- `chromium/third_party/blink/renderer/core/frame/window_or_worker_
  global_scope.cc` — the shared surface between Window and Worker.

Useful for verifying the exact shape of `navigator.userAgentData`,
`navigator.connection`, `navigator.permissions`, etc. Our
`window_bootstrap.js` currently approximates these; the Blink source is
authoritative.

## Fetch and XHR

- `chromium/third_party/blink/renderer/modules/fetch/` — Blink's fetch
  implementation. Useful for understanding Request/Response/Headers
  classes and how they enforce CORS.
- `chromium/third_party/blink/renderer/core/xmlhttprequest/
  xml_http_request.cc` — XHR. We implement most of this in
  `window_bootstrap.js`; cross-check against the canonical source if
  anything looks off.

## Cookies

- `chromium/net/cookies/cookie_monster.cc` — Chromium's cookie jar.
  Useful if our `net::cookies::CookieJar` disagrees with real Chrome
  about cookie scoping, SameSite handling, or secure-only attribute
  enforcement.
- RFC 6265 (HTTP State Management) for the spec.

## Location and navigation

**For the refactor (task #74)**:

- `chromium/third_party/blink/renderer/core/frame/location.cc` — the
  Location interface. `assign()`, `replace()`, `reload()`, and the
  `href` setter. Source of truth for how the browser kicks off a new
  navigation.
- `chromium/third_party/blink/renderer/core/loader/frame_loader.cc` —
  the FrameLoader that handles navigation requests, including meta-
  refresh parsing.
- `chromium/third_party/blink/renderer/core/loader/document_loader.cc`
  — commits a navigation and sets up the new Document.
- `chromium/third_party/blink/renderer/core/page/page.cc` — Page-level
  state, init script evaluation per new document (`ScriptController`
  integration).

## Stealth-relevant Blink files

These are the files you should read when you want to understand what
fingerprint surface Blink exposes:

- `chromium/third_party/blink/renderer/modules/canvas/canvas2d/
  text_metrics.cc` — the 13 TextMetrics fields.
- `chromium/third_party/blink/renderer/core/html/canvas/
  canvas_rendering_context.h` — contextAttributes, alpha premultipl.
- `chromium/third_party/blink/renderer/platform/fonts/shaping/
  shape_result.cc` — glyph shaping output structure.
- `chromium/third_party/blink/renderer/modules/webgl/
  webgl_rendering_context_base.cc` — getParameter, getExtension, and
  the anti-aliasing / stencil / depth-buffer default values that
  real WebGL context reports.
- `chromium/third_party/blink/renderer/modules/device_orientation/
  device_orientation_event.cc` — the alpha/beta/gamma properties that
  mobile fingerprinters check.
- `chromium/third_party/blink/renderer/modules/battery/battery_manager.
  cc` — `navigator.getBattery()` return shape. (Chrome removed this
  API in 2024 for non-secure contexts — verify current state.)

## Chromium source search tips

- **cs.chromium.org** (or source.chromium.org) has a web UI for Chromium
  source that's easier to browse than grep. Use it to find a
  function/class and then jump to the file.
- **grep.app** (grep.app) lets you search across Chromium and many other
  repos simultaneously with regex support.
- **DeepWiki** (deepwiki.com/chromium/chromium) has AI-summarized
  docs for specific files. Not always accurate but good for orientation.

## Don't port the whole thing

Blink is ~9 million lines of C++. Do not try to port it in full. The
strategy is:

1. Find the one file that implements the specific thing you're fixing.
2. Read it and understand the algorithm.
3. Port just that file (or the ~100 relevant lines) to Rust.
4. Verify against a fingerprint reference sum or known Chrome output.
5. Move on.

Our T1.3 audio port is 490 lines of Rust (plus 150 lines of
surrounding test code) replacing two Blink C++ files. That's the right
scale per T1.x item. If you find yourself porting thousands of lines,
stop — either the scope is wrong or you're reimplementing something
you don't need.

## A warning about `%` and file paths

Chromium's googlesource uses `/src/+/main/...` URLs that contain `+`.
Some curl invocations mangle that. If a fetch gives you HTML instead of
raw source, try:

1. The WebKit mirror: `raw.githubusercontent.com/WebKit/WebKit/main/
   Source/WebCore/...` — reliably returns raw files.
2. The Chromium GitHub mirror: `raw.githubusercontent.com/chromium/
   chromium/main/...` — sometimes 404s on main; try specific branches.
3. The Googlesource REST API: `https://chromium.googlesource.com/
   chromium/src/+/main/path/to/file?format=TEXT` — returns base64-
   encoded content. Needs `base64 -d` to decode. Rate-limited; expect
   503 errors if you hammer it.
