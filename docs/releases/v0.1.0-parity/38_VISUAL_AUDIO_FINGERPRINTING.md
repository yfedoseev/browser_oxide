# 38 — Visual & audio fingerprinting (canvas / WebGL / audio)

**Owner scope:** cross-cutting category chapter — organized by DETECTION
TECHNIQUE, not by vendor.
**Status:** reference chapter feeding the synthesis chapter (42) for
cross-vendor leverage analysis.
**Why this chapter exists:** chapters 06-08 / 25-27 are per-vendor deep
dives, each touching canvas/WebGL/audio in passing. If you ask "should I
spend a week on a canvas fix or a WebGL fix?" no per-vendor chapter can
answer — only a category-cut can, because every vendor that probes
canvas benefits from the same canvas fix. This chapter is the matrix.

Read this with:
- `18_ANTI_BOT_VENDOR_COOKBOOK.md` — lists vendors that use each technique
  (the per-vendor row in our matrices)
- `16_STEALTH_FINGERPRINT_AUDIT.md` — BO's JS-API masking status
  (the *names* of the canvas/audio/WebGL classes — `CanvasRenderingContext2D.prototype`
  etc. — are partially masked; this chapter scores the *pixel/sample
  output* underneath, a different axis)
- `17_WEB_API_PARITY_MATRIX.md` — Web API coverage (canvas/WebGL/audio entries)
- `06_AWS_WAF_SOLVER.md` — AWS WAF reads WebGL `getParameter` directly + canvas
- `07_DATADOME_PRIMITIVES.md` — DataDome runs Picasso canvas inside an iframe
- `08_KASADA_FRONTIER.md` — Kasada `ips.js` probes canvas/WebGL/audio inside its VM
- `25_CLOUDFLARE_DEEP.md`, `26_AKAMAI_BMP_DEEP.md` — vendor-specific surfaces
- `27_VENDOR_COMPETITIVE_MATRIX.md` — head-to-head pass rates

---

## 1. Why this is a CATEGORY chapter, not a vendor chapter

### 1.1 The leverage thesis

Fingerprinting is the only major bot-detection axis where **one engine
fix moves multiple vendors at once**. Compare:

- A *per-vendor* fix — e.g., the Kasada `bot1225` empty-slot fix in
  `08_KASADA_FRONTIER.md` Lever 4 — flips Kasada and nothing else.
- A *per-bootstrap* fix — e.g., the `_maskAsNative` sweep in
  `16_STEALTH_FINGERPRINT_AUDIT.md §5` — touches every vendor that reads
  `Function.prototype.toString`. Likewise canvas / WebGL / audio: every
  vendor that reads the pixel/sample output benefits in lockstep.

If we can establish "8 of N vendors probe canvas" or "9 of N probe
WebGL", the prioritisation argument writes itself: the **same engineer
day** spent on WebGL parity moves 9 vendors instead of 1.

### 1.2 The vendor census (corpus + cookbook overlap)

The 12 anti-bot products covered in `18_ANTI_BOT_VENDOR_COOKBOOK.md`:

1. AWS WAF (`§2.1` cookbook, `06_AWS_WAF_SOLVER.md`)
2. DataDome (`§2.2`, `07_DATADOME_PRIMITIVES.md`)
3. Akamai BMP (`§2.3`, `26_AKAMAI_BMP_DEEP.md`)
4. Kasada (`§2.4`, `08_KASADA_FRONTIER.md`)
5. Cloudflare Bot Management (`§2.5`, `25_CLOUDFLARE_DEEP.md`)
6. PerimeterX / HUMAN (`§2.6`)
7. Imperva / Incapsula / Thales (`§2.7`)
8. Sucuri (`§2.8`)
9. Google reCAPTCHA (`§2.9`)
10. Arkose Labs / FunCaptcha (`§2.10b`)
11. hCaptcha (`§2.11`)
12. F5 / Shape Security (per acquired-product research, [Round Proxies F5 bypass](https://roundproxies.com/blog/f5-bypass/))

Plus secondary mentions: wbaas (`§2.12`), Friendly Captcha (`§2.13`),
Forter (`§2.14`), Castle (`§2.6` of chap 16). Forter and Castle are
present in the cookbook but as *risk scorers* rather than blockers — we
include them in the matrices because they read the same signals.

### 1.3 Headline takeaway (proven below in §2.3, §3.2, §4.2)

| Technique | Vendors that read it | Fixed-effort win |
|---|---|---|
| Canvas 2D (text + emoji + composite) | 10 of 12 | high |
| WebGL parameters (RENDERER/VENDOR/extensions) | 11 of 12 | very high |
| WebGL pixel readback (`readPixels`) | 6 of 12 | medium |
| Audio (DynamicsCompressor output) | 8 of 12 | medium-high |
| Font enumeration | 9 of 12 | low (already largely solved) |

**Pre-conclusion (firmed in §5):** WebGL parameter consistency is the
single highest-leverage category. Canvas text+emoji rendering is second.
Audio compressor accuracy is third. Each is "1-2 weeks of focused work"
in our current state.

---

## 2. Canvas fingerprinting — full deep dive

### 2.1 Mechanism (origin → present)

#### 2.1.1 Origin paper

Mowery & Shacham, "Pixel Perfect: Fingerprinting Canvas in HTML5",
2012, [Hovav Shacham's CSE site PDF](https://hovav.net/ucsd/dist/canvas.pdf).
The seminal paper observes that rendering identical Canvas 2D draw
calls produces pixel-different outputs across browsers, OSes, and GPUs
— enough entropy to act as a tracker.

#### 2.1.2 First in-the-wild study

Acar, Eubank, Englehardt, Juarez, Narayanan, Diaz, "The Web Never
Forgets: Persistent Tracking Mechanisms in the Wild", CCS 2014,
[ACM DL](https://dl.acm.org/doi/10.1145/2660267.2660347). Crawled top-100K
sites; found 5% deployed canvas fingerprinting, predominantly via
AddThis. This is the paper that legitimised "canvas FP" as a tracking
category in policy discussions.

#### 2.1.3 The minimal canvas FP probe

A modern fingerprinter walks this exact sequence (per [thumbmarkjs.com](https://www.thumbmarkjs.com/content/browser-fingerprinting-techniques/) /
[browserleaks.com](https://browserleaks.com/canvas) / [FingerprintJS canvas.ts](https://github.com/fingerprintjs/fingerprintjs/blob/master/src/sources/canvas.ts)):

```js
const canvas = document.createElement('canvas');
canvas.width = 200; canvas.height = 50;
const ctx = canvas.getContext('2d');
ctx.textBaseline = 'top';
ctx.font = "14px 'Arial'";
ctx.fillStyle = '#f60';                       // (a) opaque rect — easy
ctx.fillRect(125, 1, 62, 20);
ctx.fillStyle = '#069';                       // (b) text — font + AA
ctx.fillText("Cwm fjordbank gly 😃", 2, 15);
ctx.fillStyle = 'rgba(102, 204, 0, 0.7)';     // (c) translucent overlap
ctx.fillText("Cwm fjordbank gly 😃", 4, 17);
const fingerprint = canvas.toDataURL();       // (d) extract pixels
// hash = md5(fingerprint)  OR  CRC32 from PNG IDAT chunk
```

Variations:
- **FingerprintJS** uses the test string "Cwm fjordbank gly" + the emoji
  `String.fromCharCode(55357, 56835)` (😃), fonts `11pt 'Times New
  Roman'` then `18pt Arial`. Two canvases — one **text-only**, one
  **geometry-only** (three circles with `globalCompositeOperation =
  'multiply'`, plus concentric `'evenodd'` rule fills) — so unstable
  text rendering doesn't poison the more-stable geometry hash
  ([FingerprintJS canvas.ts](https://github.com/fingerprintjs/fingerprintjs/blob/master/src/sources/canvas.ts)).
- **CreepJS** ([abrahamjuliot/creepjs](https://github.com/abrahamjuliot/creepjs))
  breaks canvas into FIVE separate buckets: Image, Blob, Paint, Text,
  Emoji — and reports lies per-bucket (see §6).
- **DataDome "Picasso"** ([DataDome threat-research blog](https://datadome.co/threat-research/the-art-of-bot-detection-picasso-for-device-class-fingerprinting/))
  is a randomised challenge: a per-session seed selects a different
  draw sequence (curves + text + composite ops in randomised order +
  colours) so the **same browser** produces a **different output** per
  challenge; the *equivalence class* of outputs is what's classified.

#### 2.1.4 Why outputs vary

Five entropy sources, in approximate order of contribution:

1. **Font rendering** — font-fallback chain (which face draws "😃"?
   Apple Color Emoji vs Segoe UI Emoji vs Noto Color Emoji), hinting
   on/off, subpixel positioning, LCD vs grayscale anti-aliasing,
   gamma. macOS uses CoreText, Windows uses DirectWrite, Linux uses
   FreeType + (typically) Fontconfig — three subpixel-shape families.
2. **Emoji rendering** — `😃` is U+1F603, which renders as:
   - Apple Color Emoji on iOS/macOS (a glossy 3D face)
   - Segoe UI Emoji on Windows 10+ (flat, recolour-able)
   - Noto Color Emoji on Android / most Linux distros
   - SwiftShader fallback or `.notdef` glyph on non-emoji-capable systems
   Each is a wildly different raster — bigger entropy than any other
   single canvas signal.
3. **GPU acceleration vs CPU fallback** — Chrome's Skia compositor uses
   the GPU for blends and shadows when available; falls back to CPU
   raster when not. The two paths produce visibly different anti-alias
   coverage values on translucent overlaps.
4. **Anti-aliasing algorithm** — Skia's CPU AA uses 8-bit alpha
   coverage; GPU AA on macOS/Metal uses 4× MSAA per default; Direct3D
   path uses ANGLE's coverage-mask supersampling. Different sub-pixel
   coverage → different RGBA at edges → different hash.
5. **Composite operation precision** — Canvas 2D specifies blends in
   the spec but the per-pixel rounding behaviour is
   implementation-defined; multiply / overlay / soft-light differ at
   the ±1 LSB level between Skia versions.

#### 2.1.5 What anti-bot vendors specifically score

- **Equivalence-class membership** — is this canvas output one of the
  ~50 known device-class outputs we've seen before? (DataDome Picasso
  approach.)
- **Consistency with declared profile** — does the canvas hash for a
  `User-Agent: ... Macintosh ...` match the canvas hashes other Mac
  users have produced? (PerimeterX cross-signal consistency.)
- **Tamper detection** — has `HTMLCanvasElement.prototype.toDataURL`
  been overridden? Is `getImageData` non-native? Is the noise pattern
  characteristic of CanvasBlocker/Brave-farble vs a real noisy GPU?
  ([DataDome canvas tampering](https://datadome.co/anti-detect-tools/canvas-tampering-detection/),
  [PerimeterX consistency note](https://www.scrapeless.com/en/blog/webgl-fingerprint)).

### 2.2 What each draw operation reveals

| Draw op | Entropy axis | Detection difficulty |
|---|---|---|
| `fillText` with emoji | OS family + emoji font version | trivially distinguishes Mac/Win/Linux/Android |
| `fillText` with rare characters (`fjordbank`, `glyph`) | font fallback chain + hinting | medium — distinguishes within an OS |
| `fillRect` with opaque colour | almost none (deterministic) | useful only as a "did we render at all?" canary |
| Translucent `fillText` overlap | AA + blend precision | distinguishes Skia versions and GPU vs CPU |
| Bezier curves (`bezierCurveTo`) | path rasteriser + AA | strong — Skia / Direct2D / CoreGraphics differ visibly |
| `shadowBlur` + `shadowColor` | Gaussian blur sigma scaling | high — `sigma = radius/2` is Chrome's; others differ |
| `globalCompositeOperation = 'multiply'` (and similar) | per-pixel blend math | high — distinguishes Skia 0.62 vs 0.71 vs … |
| `createPattern` with image | image sampler (bilinear/nearest) | low (deterministic) |
| `createConicGradient` (sweep) | gradient interpolation precision | medium — newer browsers only |

### 2.3 Per-vendor canvas-check matrix

Sources cited per row. `✓` = vendor reads this signal. `?` = no public
evidence but probable. `✗` = vendor doesn't read this category
(network-layer-only or interactive-only).

| Vendor | text+font | emoji | curves/shapes | composite/shadow | pattern | tamper-detect | Source |
|---|:---:|:---:|:---:|:---:|:---:|:---:|---|
| AWS WAF | ✓ | ✓ | ✓ | ? | ? | ✓ | [AWS docs](https://docs.aws.amazon.com/waf/latest/developerguide/waf-js-challenge-api.html), [Round Proxies](https://roundproxies.com/blog/bypass-aws-waf/), `06_AWS_WAF_SOLVER.md` |
| DataDome | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | [DataDome Picasso](https://datadome.co/threat-research/the-art-of-bot-detection-picasso-for-device-class-fingerprinting/), [tampering](https://datadome.co/anti-detect-tools/canvas-tampering-detection/) |
| Akamai BMP v3 | ✓ | ✓ | ✓ | ✓ | ? | ✓ | [mobileproxy.space](https://mobileproxy.space/en/pages/akamai-bot-manager-premier-in-2026-architecture-signals-ml-and-operational-tactics.html), `26_AKAMAI_BMP_DEEP.md` |
| Cloudflare Bot Mgmt | ✓ | ✓ | ✓ | ? | ? | ✓ | [Medium analysis](https://medium.com/@ayushaggarwal42003/advanced-evasion-techniques-and-architecture-analysis-of-cloudflare-bot-management-systems-in-2026-1b4ba7cc3b22), `25_CLOUDFLARE_DEEP.md` |
| Kasada | ✓ | ✓ | ✓ | ? | ? | ✓ | `08_KASADA_FRONTIER.md`, plus general anti-bot literature |
| PerimeterX | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | [Scrapeless WebGL guide](https://www.scrapeless.com/en/blog/webgl-fingerprint), [Scrapfly PX bypass](https://scrapfly.io/bypass/perimeterx) |
| Imperva (reese84) | ✓ | ✓ | ✓ | ✓ | ? | ✓ | [scrapebadger](https://scrapebadger.com/imperva-bypass), [2captcha](https://2captcha.com/h/imperva-bypass) (180+ signals) |
| Sucuri | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | Network layer only; no fingerprint script |
| reCAPTCHA v3 / Enterprise | ✓ | ✓ | ? | ? | ? | ✓ | Google opaque; CreepJS shows the surface is read |
| Arkose Labs / FunCaptcha | ✓ | ? | ? | ? | ? | ✓ | [Habr FunCaptcha](https://habr.com/en/articles/908464/), [Round Proxies](https://roundproxies.com/blog/bypass-funcaptcha/) |
| hCaptcha (Enterprise) | ✓ | ? | ? | ? | ? | ✓ | Similar to reCAPTCHA Enterprise architecturally |
| F5 / Shape Security | ✓ | ✓ | ✓ | ✓ | ? | ✓ | [Round Proxies F5](https://roundproxies.com/blog/f5-bypass/), Shape multi-signal docs |

**Tally:** 10 of 12 vendors read canvas text+emoji; 7 read curves; 5
read composite-op output; **at least 9 do tamper-detection on the
canvas API surface itself** (override of `toDataURL` / `getImageData` /
`getContext`).

### 2.4 BO canvas coverage (audit)

#### 2.4.1 Rust backend — `crates/canvas`

Skia-backed via `skia-safe`, 4398 lines across the crate
(`crates/canvas/src/{canvas2d,audio,webgl,webgl_render,path,periodic_wave,osmesa_ffi,text/*}.rs`).
Two backends live side by side:

- **Default (always built):** CPU Skia raster via `skia_safe::surfaces::wrap_pixels`
  → premultiplied RGBA8 directly into a `Vec<u8>` (`canvas2d.rs:1186-1204`).
  Same Skia version Chrome uses, so geometry/composite/shadow output is
  byte-close-to-Chrome by construction. The seminal correctness comment
  block is at `canvas2d.rs:976-984`: "Chrome's 2D-canvas text IS Skia,
  so using the same Skia glyph rasterizer + the same settings is
  canvas-fingerprint parity by construction".
- **`webgl-render` feature (optional):** real shader execution via
  OSMesa (software OpenGL) + `glow`. `crates/canvas/src/webgl_render.rs:42-148`.
  Off by default; the JS-side `WebGLRenderingContext` falls back to the
  parameter-only path (`webgl_ext.rs:33-39` → `WebGLState::available =
  cfg!(feature = "webgl-render")`).

#### 2.4.2 What we get right

- **Text rasterisation:** Skia's own glyph rasteriser via
  `FontMgr::new().new_from_data(...)` → `Font::set_edging(AntiAlias)`
  → `set_subpixel(true)` → `set_hinting(FontHinting::None)`. These
  three settings exactly match Chrome's 2D-canvas defaults
  (`canvas2d.rs:1012-1016`).
- **Composite ops:** all 27 CSS `globalCompositeOperation` keywords
  mapped to `skia_safe::BlendMode` 1:1 (`canvas2d.rs:347-376`). No
  "fall back to source-over for unknown" surprises that would shift
  hash.
- **Shadow:** `image_filters::drop_shadow` with `sigma = blur/2` —
  Chrome's exact conversion (`canvas2d.rs:1259-1274`).
- **Gradients:** linear, radial (Skia `two_point_conical`), conic
  (Skia `sweep_gradient`) — all three Canvas 2D primitives mapped to
  the corresponding Skia shader (`canvas2d.rs:118-196`).
- **Patterns:** all 4 repetition modes (`repeat`, `repeat-x`,
  `repeat-y`, `no-repeat`) via `TileMode` pairs
  (`canvas2d.rs:246-272`).
- **CSS filter chain:** 8 operations (blur/grayscale/sepia/invert/
  brightness/contrast/saturate/opacity) via `image_filters::color_filter`
  with feColorMatrix matrices that match the W3C filter-effects spec
  byte-for-byte (`canvas2d.rs:517-602`).
- **PNG byte-determinism:** `flate2/zlib-rs` (C-zlib-compatible
  DEFLATE) + pinned filter strategy (`Paeth` + adaptive) → same input
  pixels produce same output bytes across runs
  (`canvas2d.rs:1147-1175`). Verified at `crates/canvas/tests/png_byte_parity.rs`
  and `crates/canvas/tests/png_chunks.rs`.
- **Optional per-profile jitter:** `to_data_url_with_jitter`
  (`canvas2d.rs:1093-1144`) seeds a PCG32 PRNG with the profile's
  `canvas_seed` (`crates/stealth/profiles/chrome_148_macos.yaml:68`)
  and perturbs ±1 LSB on 5% of pixels — so two profiles produce
  different fingerprints without the noise looking like CanvasBlocker.

#### 2.4.3 What's a known gap

| Gap | Severity | Vendor that catches it | Where in code |
|---|---|---|---|
| Emoji rasterisation — we render `😃` through whatever face the OS happens to have (system Noto Color Emoji on most CI Linux), NOT Apple Color Emoji. So our Mac profile says it's macOS but produces a Linux-shape emoji raster. | **High (consistency)** | DataDome Picasso, CreepJS Emoji bucket, FingerprintJS emoji-end | `canvas2d.rs:985-1035` (no special emoji-font selection); upstream gap in `crates/canvas/src/text/font_database.rs` |
| GPU/CPU acceleration signature — Chrome's 2D canvas is GPU-accelerated by default on supported devices; we always render on CPU via Skia raster. Translucent overlap AA coverage differs ±2-3 LSB. | Medium | Any vendor with multi-sample equivalence-class match | `canvas2d.rs:1187-1204` (`surfaces::wrap_pixels` — CPU only) |
| `measureText().actualBoundingBox*` — implemented (`canvas2d.rs:971-973` → `text::measure_text_metrics`) but the per-OS metric drift hasn't been compared against captured-Chrome golden | Medium | FingerprintJS measureText fallback, CreepJS Text bucket | `crates/canvas/src/text/` (multiple files) |
| Color-space precision — Skia internally uses sRGB; on the macOS-profile we should ideally use Display P3 to match Chrome's `colorSpace: "display-p3"` context option. Not implemented. | Low (rare option) | None measured in 126-corpus | n/a |
| `drawImage` from `HTMLImageElement` with CORS-tainted source — we don't currently track taint propagation; a real tainted canvas throws on `toDataURL`. Not currently a measured FP. | Low | Castle (the Castle blog flags this specifically) | `canvas_bootstrap.js:936-937` |

#### 2.4.4 Test coverage

- `crates/canvas/tests/png_byte_parity.rs` — byte-exact reproducibility
- `crates/canvas/tests/png_chunks.rs` — IHDR/IDAT/IEND only, no extras
- `crates/canvas/tests/canvas_paths.rs` — path rasterisation primitives
- `crates/canvas/tests/font_metrics.rs` — measureText vs golden
- `crates/canvas/tests/audio_reference.rs` and `audio_parity.rs` — audio
  side
- `crates/browser/tests/chrome_compat.rs:1139-1230` — end-to-end via
  full V8 + JS: `cls_html_canvas_element`, `canvas_2d_context`,
  `canvas_webgl_context`, `canvas_to_data_url`,
  `canvas_drawing_produces_nonblank_data_url` (asserts >1 KB after
  draw, NOT byte-equality)

**Gap in test coverage:** no test compares our `canvas.toDataURL()`
hash on the FingerprintJS / browserleaks reference draw sequence
against a captured Chrome 148 macOS hash. The PNG-byte-parity tests
prove `BO → BO` determinism, NOT `BO ↔ Chrome` parity. The
diagnostic suite (`crates/browser/tests/diagnostic_browserleaks.rs`,
`diagnostic_creepjs.rs`) exists but is `#[ignore]` and not gated.

### 2.5 The canvas gap rank-order

1. **Emoji rasterisation per-profile** — biggest *quality* gap. Vendor
   impact: DataDome (Picasso), Akamai, Cloudflare, F5/Shape — at minimum.
   Effort: 2-3 days to wire profile-supplied emoji-font path into the
   text-shaping stack, ship Apple-Color-Emoji-equivalent and
   Segoe-UI-Emoji-equivalent and Noto-Color-Emoji raster assets behind
   a feature flag, and select by profile OS.
2. **`canvas.toDataURL()` golden parity test** — biggest *process*
   gap. We have all the building blocks; we don't have the regression
   guard. Effort: 1 day to capture Chrome 148 macOS goldens on the
   FingerprintJS + browserleaks + thumbmarkjs probes, write the
   `#[ignore]` audit-test → snapshot transition.
3. **GPU-vs-CPU AA signature** — most expensive to fix (would need
   either GPU canvas or a deterministic CPU AA path that mimics
   Chrome's MSAA-quantised coverage). Effort: 1-2 weeks. Defer until
   we have evidence a single corpus site fails *because of* this
   specifically (none currently identified).

---

## 3. Audio fingerprinting — full deep dive

### 3.1 Mechanism

#### 3.1.1 Origin

[Englehardt & Narayanan, "Online Tracking: A 1-million-site
Measurement and Analysis", CCS 2016](https://www.cs.princeton.edu/~arvindn/publications/OpenWPM_1_million_site_tracking_measurement.pdf)
first identified `AudioContext` as a fingerprinting vector. The
canonical probe code is in OpenWPM's [audiofingerprint.openwpm.com](https://audiofingerprint.openwpm.com/) test page.

#### 3.1.2 The minimal probe

Per [Fingerprint.com audio FP blog](https://fingerprint.com/blog/audio-fingerprinting/)
and the [thumbmarkjs sample](https://www.thumbmarkjs.com/content/browser-fingerprinting-techniques/):

```js
const ctx = new OfflineAudioContext(1, 5000, 44100);
const osc = ctx.createOscillator();
const comp = ctx.createDynamicsCompressor();
osc.type = 'triangle'; osc.frequency.value = 10000;
// Compressor defaults often overridden:
comp.threshold.value = -50;  // dB
comp.knee.value      = 40;   // dB
comp.ratio.value     = 12;   // :1
comp.attack.value    = 0;
comp.release.value   = 0.2;
osc.connect(comp); comp.connect(ctx.destination);
osc.start(0);
const buf = await ctx.startRendering();
const data = buf.getChannelData(0);
// Hash strategies:
// (a) sum(abs(data[4500..5000]))            ← the canonical "sum hash"
// (b) sha256(Float32Array(data).buffer)     ← stricter, less stable
```

Why this works:
- `OfflineAudioContext` renders synchronously and faster than
  real-time, **no audio hardware involved**. So this isn't about
  speakers / microphones; it's about the **DSP code path**.
- The **DynamicsCompressorNode** is a non-trivial DSP block (per-sample
  envelope detector + lookahead pre-delay + adaptive release polynomial
  + asin/sin pre/post-warp). The per-sample math is float32 — any
  per-CPU floating-point quirk, any SIMD intrinsic, any compiler choice
  shifts the output by ULP-level amounts that compound over 5000
  samples.
- Samples `[4500..5000]` are taken because the compressor's envelope
  has fully settled by then, so transient state is gone and we measure
  the steady-state gain curve.

#### 3.1.3 Why outputs vary

Per [Fingerprint.com](https://fingerprint.com/blog/audio-fingerprinting/) +
[WebAudio issue #1500](https://github.com/WebAudio/web-audio-api/issues/1500):

1. **Floating-point math** — IEEE 754 says "the result of basic ops is
   correctly rounded", but DSP operations compound transcendental
   approximations (sin/asin/exp/log). Different CPU microcodes
   (Skylake vs Zen3 vs Apple M-series) take different rounding paths
   in transcendentals at the LSB.
2. **SIMD intrinsics** — Blink's audio code uses AVX2 on x86_64
   Windows/Linux, NEON on ARM macOS / Android. The order of
   accumulation in vectorised loops differs by lane count → different
   rounding.
3. **FFT implementation per OS** — "Chrome uses a separate fast
   Fourier transform implementation on macOS" (per the article above).
   Affects the AnalyserNode path more than the compressor path.
4. **Browser engine** — Blink vs Gecko vs WebKit have independent
   compressor implementations. The numerical outputs differ in the
   third decimal at the `sum(abs(...))` level.
5. **Android version** — same hardware running Android 9 vs 10
   produces distinct hashes due to platform audio-stack changes.

#### 3.1.4 What anti-bot vendors specifically score

- **Equivalence-class membership** — does `sum(abs(data[4500..5000]))`
  fall in a known cluster (e.g., Chrome-on-Mac-Silicon = 124.0434...,
  Chrome-on-Win-x64 = 124.0411..., Firefox-on-anything = 35.7382...)?
- **Stability proof** — repeat the probe; does it produce the same
  output? Brave's "farble" perturbs per-session, so the second probe
  differs from the first → instant detection.
- **Cross-signal consistency** — does the audio hash's cluster
  membership match what `navigator.userAgent`, `WebGL.UNMASKED_RENDERER`,
  and canvas hash claim about the device?

### 3.2 Per-vendor audio-check matrix

| Vendor | uses audio FP | DynamicsCompressor | OscillatorNode | AnalyserNode | BiquadFilter | Source |
|---|:---:|:---:|:---:|:---:|:---:|---|
| AWS WAF | ✓ | ✓ | ✓ | ? | ? | [Round Proxies AWS WAF](https://roundproxies.com/blog/bypass-aws-waf/), `06_AWS_WAF_SOLVER.md` |
| DataDome | ✓ | ✓ | ✓ | ? | ? | [Fingerprint.com vendor mention](https://fingerprint.com/blog/audio-fingerprinting/) |
| Akamai BMP | ✓ | ✓ | ✓ | ✓ | ✓ | `26_AKAMAI_BMP_DEEP.md`, Akamai T1.3 history (memory `tier1_priority_for_akamai.md`) — DynamicsCompressor port was their headline ask in 2026-04 |
| Cloudflare | ✓ | ✓ | ✓ | ? | ? | [Medium 2026 analysis](https://medium.com/@ayushaggarwal42003/advanced-evasion-techniques-and-architecture-analysis-of-cloudflare-bot-management-systems-in-2026-1b4ba7cc3b22) |
| Kasada | ✓ | ✓ | ✓ | ? | ? | Generic fingerprint literature; not confirmed in K2-DIFF yet |
| PerimeterX | ✓ | ✓ | ✓ | ? | ? | [Scrapeless](https://www.scrapeless.com/en/blog/webgl-fingerprint) |
| Imperva | ✓ | ✓ | ✓ | ? | ? | [scrapebadger Imperva](https://scrapebadger.com/imperva-bypass) (180+ signals incl. AudioContext) |
| Sucuri | ✗ | ✗ | ✗ | ✗ | ✗ | Network only |
| reCAPTCHA | ✓ | ? | ? | ? | ? | Closed-source; the surface is read |
| Arkose | ✓ | ? | ? | ? | ? | [Habr FunCaptcha](https://habr.com/en/articles/908464/) — "AudioContext rendering results" listed |
| hCaptcha | ? | ? | ? | ? | ? | Not separately catalogued |
| F5 / Shape | ✓ | ✓ | ✓ | ? | ? | [Round Proxies F5](https://roundproxies.com/blog/f5-bypass/) |

**Tally:** 8 of 12 confirmed read audio (likely 10 of 12 — hCaptcha
and reCAPTCHA are opaque), all 8 lean on `DynamicsCompressorNode`
specifically. AnalyserNode and BiquadFilter are second-tier (mostly
CreepJS-only; not seen in vendor-side probes captured to date).

### 3.3 BO audio coverage (audit)

#### 3.3.1 Rust backend — `crates/canvas/src/audio.rs` (837 lines)

The compressor is a **direct Rust port of WebKit's DynamicsCompressorKernel.cpp**
(Google-authored 2011, BSD-3) — `audio.rs:237-689`. Per the header
comment:

> Port is bit-accurate to the C++ for the single-channel mono case. We
> use f32 throughout (matching Blink's float32 DSP) so the numerical
> behavior of the loop matches. Pre-delay buffer with default 6 ms
> latency. Adaptive release curve via the 4th-order polynomial
> coefficients. asin/sin pre-warp / post-warp around compressorGain.

The full pipeline (`audio.rs:130-234`):
- Generate band-limited triangle samples via
  `crate::periodic_wave::PeriodicWave` (`crates/canvas/src/periodic_wave.rs:341` lines)
  → Fourier synthesis of odd harmonics with above-Nyquist culling.
- Empirical `BLINK_OSCILLATOR_SCALE = 0.47624` calibrated against
  FingerprintJS reference sum (124.04347 ± 3.6 ppm) at default
  threshold (`audio.rs:177`).
- Threshold-aware makeup-gain exponent: `0.6 + 0.0739 * ((-db_threshold - 24) / 26)`
  for thresholds ≤ -24 dB (`audio.rs:500-504`) — empirically tuned to
  match Chrome 147 across both the FingerprintJS-default (−24 dB) and
  CreepJS / Kasada (−50 dB) probe families.
- Process loop runs `N_DIVISION_FRAMES = 32` at a time, matching Blink
  exactly. NaN/Inf gremlin-fixers at the same points Blink has them.
- Per-seed jitter (`crates/js_runtime/src/extensions/audio_ext.rs:52-77`):
  - threshold ± 5 mdB
  - release ± 0.1 ms
  - both derived from `sin(seed * k)` so seed=0 → zero jitter
    (preserves the calibrated reference baseline).

Other audio nodes:
- **`OscillatorNode`** — pure Rust wavetable
  (`crates/canvas/src/periodic_wave.rs`, 4 standard wave types with
  Fourier synthesis + band-limit).
- **`AnalyserNode.getFloatFrequencyData`** — Blackman window + rustfft
  + magnitude + dB clamp (`audio_ext.rs:90-130`). Matches the Web
  Audio §1.36.2 spec coefficients exactly (a0=0.42, a1=0.5, a2=0.08).
- **`BiquadFilterNode.getFrequencyResponse`** — closed-form magnitude
  + phase computation (`audio_ext.rs` further down, P2.2b op).

#### 3.3.2 What we get right

- The default-threshold (−24 dB) FingerprintJS case matches Chrome 147
  to **3.6 ppm** of the sum — measured, calibrated, locked at
  `audio.rs:155-177` with a documented "do not change without
  re-running audio_reference.rs golden".
- All 4 wave types produce distinct, plausibly-shaped audio (see
  `audio.rs:736-836` tests: `wavetable_works_at_440_hz`,
  `wavetable_sine_1khz_differs_from_default`, `wave_type_affects_output`).
- Deterministic across runs (`deterministic_output` test).
- Per-profile differentiation works without breaking the calibrated
  baseline (the `sin(seed * k)` formulation).

#### 3.3.3 Known gaps

| Gap | Severity | Vendor that catches it | Where in code |
|---|---|---|---|
| **CreepJS/Kasada `-50 dB` threshold** — the calibrated `BLINK_OSCILLATOR_SCALE` was set to match the **−24 dB FingerprintJS-default** case. At −50 dB our sum is 103.92 vs Chrome's 124.04 — an 16% deviation. A single global scale can't fit both thresholds. Bug is in the compressor's response curve, not the oscillator. | **High** for CreepJS / Kasada audit probes; **none** for vendors using the default | Kasada (per probe family), CreepJS audio bucket | `audio.rs:163-176` (the open issue comment). Closing it requires bisecting Blink's static compression curve / makeup-gain math (~1 wk per author estimate). |
| Per-OS audio-stack signature — real Chrome uses a different FFT on macOS (per [Fingerprint.com](https://fingerprint.com/blog/audio-fingerprinting/)); our single-implementation can't reproduce the OS-correlated sum cluster | Medium | Cross-signal consistency: if profile says macOS but audio sum matches Linux cluster | Implementation-wide |
| Multi-channel rendering (stereo, 5.1) — port is single-channel mono only ("bit-accurate to the C++ for the single-channel mono case", `audio.rs:18`) | Low (FP probes are mono) | None measured | `audio.rs:316-358` (Kernel hardcoded to one pre-delay buffer per channel but only mono-tested) |
| `AudioWorklet` / `AudioWorkletNode` — `interfaces_bootstrap.js` stub only (per `17_WEB_API_PARITY_MATRIX.md §2.17`) | Low (rare for FP) | None measured | n/a |
| Real-time `AudioContext` (`new AudioContext()` not `OfflineAudioContext`) — present but doesn't process audio meaningfully; suspended state forever | Low | None measured (FP uses Offline) | `canvas_bootstrap.js:585+` |

#### 3.3.4 Test coverage

- `crates/canvas/tests/audio_reference.rs` — calibration golden
  (~124.04 ± 3 ppm at default params)
- `crates/canvas/tests/audio_parity.rs` — Chrome vs BO parity
- In-crate `audio.rs::tests` — 7 tests covering determinism, range,
  wavetable, wave-type cross-product, seed variation
- **Gap:** no end-to-end V8-driven `OfflineAudioContext.startRendering()`
  test in `chrome_compat.rs` (the audio ops are exercised via Rust,
  not via JS surface).

### 3.4 The audio gap rank-order

1. **-50 dB threshold parity** — biggest measurable miss. Vendor
   impact: Kasada, CreepJS audio. Effort: 1 week (bisect the static
   curve / makeup-gain math at thresholds ≤ -24 dB; the polynomial
   coefficients in `audio.rs:524-529` are the most likely suspect).
2. **`OfflineAudioContext` end-to-end test** — add to `chrome_compat.rs`
   so the audio surface is exercised from JS, not just Rust. Effort: 1
   day.
3. **Per-OS FFT/SIMD signature** — costly fix (would need OS-conditional
   compilation of the inner loop). Defer until a corpus site fails
   provably on audio cross-signal consistency.

---

## 4. WebGL fingerprinting — full deep dive

### 4.1 Mechanism

#### 4.1.1 The probe surface

Per [browserleaks.com/webgl](https://browserleaks.com/webgl), [thumbmarkjs](https://www.thumbmarkjs.com/content/browser-fingerprinting-techniques/), and [Scrapeless WebGL guide 2025](https://www.scrapeless.com/en/blog/webgl-fingerprint):

```js
const canvas = document.createElement('canvas');
const gl = canvas.getContext('webgl');          // or 'webgl2'
const ext = gl.getExtension('WEBGL_debug_renderer_info');
const data = {
  vendor:           gl.getParameter(gl.VENDOR),                     // "WebKit", "Mozilla", ...
  renderer:         gl.getParameter(gl.RENDERER),                   // "WebKit WebGL", ...
  unmaskedVendor:   gl.getParameter(ext.UNMASKED_VENDOR_WEBGL),     // "Google Inc. (Apple)"
  unmaskedRenderer: gl.getParameter(ext.UNMASKED_RENDERER_WEBGL),   // "ANGLE (Apple, ANGLE Metal Renderer: Apple M3, ...)"
  version:          gl.getParameter(gl.VERSION),                    // "WebGL 2.0 (OpenGL ES 3.0 Chromium)"
  shadingLanguage:  gl.getParameter(gl.SHADING_LANGUAGE_VERSION),
  maxTextureSize:   gl.getParameter(gl.MAX_TEXTURE_SIZE),
  maxRenderbuffer:  gl.getParameter(gl.MAX_RENDERBUFFER_SIZE),
  maxVertexAttribs: gl.getParameter(gl.MAX_VERTEX_ATTRIBS),
  // ... ~20 more max-* parameters ...
  highPrec:         gl.getShaderPrecisionFormat(gl.FRAGMENT_SHADER, gl.HIGH_FLOAT),
  extensions:       gl.getSupportedExtensions(),   // ~36 strings, varies by GPU/driver
};
```

Two distinct levels of WebGL FP:

1. **Parameter-level (cheap, ubiquitous):** the data above. ~50
   scalar/string values per context. Read by every vendor in our
   table.
2. **Pixel-level (expensive, ML-grade):** compile a shader, render a
   complex scene, `readPixels` → hash. Reveals driver-level rounding,
   precision, and per-GPU rasteriser quirks. Read by ~6 vendors per
   §4.2.

#### 4.1.2 Why outputs vary

- **GPU + driver → renderer string** — `ANGLE (Apple, ANGLE Metal
  Renderer: Apple M3, ...)` is the only string you'll see on
  Mac-Silicon Chrome 148. `ANGLE (NVIDIA, NVIDIA GeForce RTX 3080
  Direct3D11 vs_5_0 ps_5_0, D3D11)` is the typical Windows
  desktop-Chrome. Linux variants run gamut: `Mesa/X.org`,
  `AMD Radeon`, `Intel Mesa`. The renderer string is the highest-value
  single signal in fingerprinting — it identifies a hardware class
  with surgical precision.
- **Extensions list** — supported extensions vary by GPU + driver
  version. Recent NVIDIA on Windows reports 35-36 extensions; older
  Intel iGPU on Linux Mesa reports 28. Mismatches with the renderer
  string are an obvious tamper signal.
- **Shader precision** — `getShaderPrecisionFormat(FRAGMENT, HIGH_FLOAT)`
  returns `{rangeMin: 127, rangeMax: 127, precision: 23}` for IEEE
  single-precision; mobile GPUs sometimes report `{15, 15, 10}` for
  `mediump`.
- **Max-* parameters** — `MAX_TEXTURE_SIZE` is 16384 on every modern
  desktop GPU; older Intel iGPUs report 8192; mobile commonly reports
  4096. Useful as a "mobile vs desktop" gate.
- **Pixel rendering** — shader precision rounding + per-driver
  rasteriser anti-aliasing → readPixels hash differs across GPUs even
  for identical shader source.

#### 4.1.3 What anti-bot vendors specifically score

- **Renderer string vs declared OS / UA** — does
  `ANGLE (... Apple M3 ...)` match a macOS UA? If your UA says
  Windows but renderer says Apple, instant fail.
- **`SwiftShader` / `llvmpipe` / `Mesa software` substrings** — these
  are software-rendering substrings that betray headless Chrome
  without GPU access. **High-signal tell.** Per [AWS WAF analysis](https://www.scrapeless.com/en/blog/webgl-fingerprint):
  "headless browsers like Puppeteer and Playwright often have default
  WebGL fingerprints that are easily identifiable as non-human, for
  example, they might report a generic GPU like 'Google SwiftShader'".
- **Extension list + renderer consistency** — extensions known to ship
  only on D3D11 paths shouldn't appear under a Metal renderer.
- **Pixel readback hash equivalence class** — same as canvas: cluster
  the readPixels hash, check membership.

### 4.2 Per-vendor WebGL-check matrix

| Vendor | params (VENDOR/RENDERER/UNMASKED_*) | extensions | precision | max-* | pixel readback | tamper-detect | Source |
|---|:---:|:---:|:---:|:---:|:---:|:---:|---|
| AWS WAF | ✓ | ✓ | ✓ | ✓ | ? | ✓ | [Scrapeless](https://www.scrapeless.com/en/blog/webgl-fingerprint), `06_AWS_WAF_SOLVER.md` (Amazon-tenants probe WebGL specifically) |
| DataDome | ✓ | ✓ | ? | ✓ | ✓ | ✓ | DataDome bot-detection-techniques learning centre, `07_DATADOME_PRIMITIVES.md` |
| Akamai BMP | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | [mobileproxy.space Akamai 2026](https://mobileproxy.space/en/pages/akamai-bot-manager-premier-in-2026-architecture-signals-ml-and-operational-tactics.html), `26_AKAMAI_BMP_DEEP.md` |
| Cloudflare | ✓ | ✓ | ? | ✓ | ✓ | ✓ | [Medium 2026 analysis](https://medium.com/@ayushaggarwal42003/advanced-evasion-techniques-and-architecture-analysis-of-cloudflare-bot-management-systems-in-2026-1b4ba7cc3b22) |
| Kasada | ✓ | ✓ | ✓ | ✓ | ? | ✓ | `08_KASADA_FRONTIER.md`; the Kasada VM specifically probes WebGL params |
| PerimeterX | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | [Scrapeless](https://www.scrapeless.com/en/blog/webgl-fingerprint) — "WebGL fingerprints are often much higher and operate at a lower level than Canvas and is much harder to spoof" |
| Imperva (reese84) | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | [scrapebadger Imperva](https://scrapebadger.com/imperva-bypass) — WebGL is one of the 180+ signals |
| Sucuri | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | Network only |
| reCAPTCHA Enterprise | ✓ | ✓ | ? | ? | ? | ✓ | Opaque; CreepJS shows the surface is read |
| Arkose Labs | ✓ | ✓ | ? | ? | ✓ | ✓ | [Habr FunCaptcha](https://habr.com/en/articles/908464/) — "WebGL render times" listed |
| hCaptcha | ? | ? | ? | ? | ? | ? | Not separately catalogued |
| F5 / Shape | ✓ | ✓ | ✓ | ✓ | ? | ✓ | [Round Proxies F5](https://roundproxies.com/blog/f5-bypass/) + Shape multi-signal stack |

**Tally:** 11 of 12 read WebGL parameters (Sucuri is the only opt-out).
9 of 12 do tamper-detection. 6 of 12 do real pixel readback.
WebGL is the **single most-probed fingerprint surface** across the
vendor population.

### 4.3 BO WebGL coverage (audit)

#### 4.3.1 The two-layer architecture

- **JS layer (always present):** `crates/js_runtime/src/js/canvas_bootstrap.js:266-585`
  defines `WebGLRenderingContext` + `WebGL2RenderingContext` (alias)
  with full constant tables (`canvas_bootstrap.js:267-285` lists ~150
  constants) and method stubs. `getParameter` (`canvas_bootstrap.js:443-457`)
  routes string values from a static `_g()` accessor that reads from
  the stealth profile via `op_get_profile_value`.
- **Rust profile catalog:** `crates/canvas/src/webgl.rs` (267 lines)
  defines `WebGLParams` with three baked profiles: `nvidia_rtx_3080`,
  `intel_iris_xe`, `apple_m2` (`webgl.rs:86-186`). These are the
  defaults shipped with the engine — real profiles override via YAML
  (`crates/stealth/profiles/chrome_148_macos.yaml:40-41`):
  `webgl_vendor: "Google Inc. (Apple)"`,
  `webgl_renderer: "ANGLE (Apple, ANGLE Metal Renderer: Apple M3, Unspecified Version)"`.
- **Rust render layer (`webgl-render` feature, OPT-IN):**
  `crates/canvas/src/webgl_render.rs` (627 lines) uses **OSMesa
  software OpenGL** via `crate::osmesa_ffi` (33-line FFI shim) + `glow`
  for shader compilation + draw + readPixels. The ops are wired in
  `crates/js_runtime/src/extensions/webgl_ext.rs` (332 lines) but only
  active when the `webgl-render` feature is on. By default,
  `op_webgl_available` returns `false` (`webgl_ext.rs:43-45`) and the
  JS side falls back to the parameter-only path.

#### 4.3.2 What we get right

- **Parameter values** — `VENDOR/RENDERER/VERSION/SHADING_LANGUAGE_VERSION/UNMASKED_*`
  + ~16 max-* parameters + extensions list — all profile-driven, all
  YAML-overridable. The defaults (`canvas_bootstrap.js:371-403`)
  match captured Chrome 147 macOS arm64 from
  `tests/fixtures/chrome147/captured_macos_arm64.json`.
- **`getSupportedExtensions()`** — returns 36 extensions, exact list
  matching Chrome 147 macOS arm64 (`canvas_bootstrap.js:464-481`).
  Stealth profiles can override.
- **`getShaderPrecisionFormat`** — profile-driven per-shader-type +
  per-precision-type lookup (`canvas_bootstrap.js:510-516`); IEEE
  single-precision defaults `{127, 127, 23}` if profile silent.
- **`getContextAttributes()`** — returns the exact 10 Chrome defaults
  (`canvas_bootstrap.js:495-508`).
- **Class shape consistency** — `Symbol.toStringTag` set to
  `"WebGLRenderingContext"` (`canvas_bootstrap.js:993`),
  `WebGL2RenderingContext` aliased (`canvas_bootstrap.js:1010`),
  constants copied to instance (`canvas_bootstrap.js:326-328`).
- **`getParameter` callable from arbitrary `this`** — the `static _g()`
  cache (`canvas_bootstrap.js:369-442`) was introduced specifically to
  fix the Kasada `esd.wgl` field that caught
  `TypeError: this._g is not a function` when
  `getParameter.call(somethingElse)` was invoked (per comment block
  `canvas_bootstrap.js:362-368`).
- **Real WebGL pipeline (opt-in)** — when `webgl-render` is enabled,
  full shader compilation + uniform binding + draw + readPixels works
  through OSMesa. Used by:
  - `crates/browser/tests/webgl_render.rs`
  - `crates/browser/tests/webgl_parity.rs`
  - `crates/browser/tests/webgl_byte_parity.rs`

#### 4.3.3 Known gaps

| Gap | Severity | Vendor that catches it | Where in code |
|---|---|---|---|
| **`webgl-render` is OFF by default** — production builds use the JS-stub path: `createShader/compileShader/linkProgram` are no-ops returning success (`canvas_bootstrap.js:519-528`). Any vendor that compiles a shader and reads back pixels gets the same hash for every BO instance (no GPU entropy). | **Very high** for pixel-readback vendors | DataDome, Akamai BMP, Cloudflare, PerimeterX, Imperva, Arkose (6 vendors) | Build config; `crates/canvas/Cargo.toml:12` |
| **OSMesa software-rendered ≠ ANGLE-Metal/D3D11** — even with `webgl-render` on, the readPixels output comes from a software OpenGL rasteriser (llvmpipe-class). Vendors comparing against the known equivalence class for "ANGLE Metal on Apple M3" will see a software-renderer cluster and either: (a) blacklist the cluster, or (b) flag the cross-signal mismatch (renderer says Apple M3, pixels say software). | High | All 6 pixel-readback vendors | `webgl_render.rs:1-3` ("software OpenGL rendering") |
| **`WebGLRenderingContext.prototype` methods unmasked** (per `16_STEALTH_FINGERPRINT_AUDIT.md §2.3 #4`) — `String(WebGLRenderingContext.prototype.getParameter)` leaks our JS source. AWS WAF + CreepJS read this; Kasada's `sfc`/`sdt` fields record it. **Partial mask exists at `canvas_bootstrap.js:1290-1295`** for a hand-picked subset (`clear`, `clearColor`, `drawArrays`, `drawElements`, `enable`, `disable`, `getParameter`) but it's not the full ~80-method prototype. | High | AWS WAF, Kasada, CreepJS | `canvas_bootstrap.js:1290-1295`; the gap is documented in `16_STEALTH_FINGERPRINT_AUDIT.md §5.4 #4` (priority P1 in the sweep plan) |
| Profile/renderer cross-signal — the `chrome_148_macos.yaml` profile says macOS but if anyone runs that profile on a Linux box, the underlying OSMesa pixel readback (if enabled) reflects the Linux GPU/driver. The pixel output cluster and the renderer string disagree. | Medium | Pixel-readback vendors with cross-signal scoring | Profile/build separation, no single fix |
| `WebGL2RenderingContext` is a literal alias of `WebGLRenderingContext` (`canvas_bootstrap.js:1010`) — real Chrome has them as distinct classes with a subclass relationship. `WebGL2RenderingContext.prototype.constructor === WebGLRenderingContext` is wrong. | Low (rare probe) | None measured | `canvas_bootstrap.js:1010` |
| `getError()` always returns 0 (NO_ERROR) in JS-stub path (`canvas_bootstrap.js` further down) — real GL probes intentionally provoke errors (e.g., bind a buffer of the wrong type) to check the error code. | Low | None measured in 126-corpus | n/a |
| WebGPU (`navigator.gpu`) — fully MISSING (`17_WEB_API_PARITY_MATRIX.md §2.10`). On bleeding-edge sites (Chrome 113+), absence is a small negative signal. | Low (still rare) | Future | n/a |

#### 4.3.4 Test coverage

- `crates/browser/tests/webgl_parity.rs` — parameter parity vs Chrome
- `crates/browser/tests/webgl_byte_parity.rs` — readPixels parity (only
  meaningful with `webgl-render` on)
- `crates/browser/tests/webgl_render.rs` — end-to-end render smoke
- `crates/browser/tests/chrome_compat.rs:1167-1310` — getContext, vendor/
  renderer/extensions per profile, MAX_TEXTURE_SIZE

**Gap in test coverage:** none of these tests run in default
`cargo test --workspace` because `webgl-render` is gated. The
production fingerprint we ship to users is the stub fingerprint.

### 4.4 The WebGL gap rank-order

1. **Mask the full `WebGLRenderingContext.prototype`** — drop-in fix.
   Vendor impact: AWS WAF + Kasada + every vendor that does
   tamper-detect (9 of 12). Effort: 1 day. Already on the priority
   list at `16_STEALTH_FINGERPRINT_AUDIT.md §5.4 #4`.
2. **Capture per-profile "real-Chrome" WebGL parameter goldens** —
   our profile catalog defaults are already good (matched to captured
   Chrome 147 macOS arm64) but per-profile drift goes uncaught. Effort:
   1 day to extract goldens from the captures we have, snapshot-test
   each profile's full param dump.
3. **Decide on the `webgl-render` story** — either:
   - (a) Enable `webgl-render` by default and accept the OSMesa cost +
     mismatched renderer-vs-pixel signature, OR
   - (b) Keep it off, document that pixel-readback vendors are
     out-of-scope for v0.1.0, OR
   - (c) Implement a "fake-but-stable" readPixels — generate pixels
     from a profile-seeded PRNG that produces values in the right
     equivalence class for the declared renderer.
   Pick (b) for v0.1.0 (no measured corpus site fails on this); design
   (c) for v0.2.

---

## 5. Cross-category leverage analysis

### 5.1 Vendor × technique grand matrix

Distilled from §2.3, §3.2, §4.2. Each cell is "✓" if the vendor probes
that category, "✗" if it does not (network-only), "?" if not catalogued
publicly.

| Vendor | Canvas | Audio | WebGL params | WebGL pixels |
|---|:---:|:---:|:---:|:---:|
| AWS WAF        | ✓ | ✓ | ✓ | ? |
| DataDome       | ✓ | ✓ | ✓ | ✓ |
| Akamai BMP     | ✓ | ✓ | ✓ | ✓ |
| Cloudflare     | ✓ | ✓ | ✓ | ✓ |
| Kasada         | ✓ | ✓ | ✓ | ? |
| PerimeterX     | ✓ | ✓ | ✓ | ✓ |
| Imperva        | ✓ | ✓ | ✓ | ✓ |
| Sucuri         | ✗ | ✗ | ✗ | ✗ |
| reCAPTCHA      | ✓ | ? | ✓ | ? |
| Arkose         | ✓ | ✓ | ✓ | ✓ |
| hCaptcha       | ✓ | ? | ? | ? |
| F5/Shape       | ✓ | ✓ | ✓ | ? |
| **Vendor count probing this category** | **10** | **8** | **11** | **6** |

### 5.2 Fix → vendor-impact ranking

For each category, "fix" means "BO's signature becomes
indistinguishable from Chrome's equivalence-class for the declared
profile". Effort is gut-feel based on §2.5 / §3.4 / §4.4:

| Fix | Effort | Vendors helped | Leverage = vendors / day |
|---|---|---|---|
| **Mask `WebGLRenderingContext.prototype`** (already on §5.4 plan) | 1 day | 9 (every tamper-detect vendor) | 9.0 |
| WebGL param/extension per-profile goldens + snapshot test | 1 day | 11 (every WebGL-probing vendor at the param level) | 11.0 |
| Canvas `toDataURL` golden parity test (capture + lock) | 1 day | 10 (canary against future drift) | 10.0 |
| Per-profile emoji-font rasterisation | 2-3 days | 10 (every canvas-probing vendor with emoji in their probe) | 3.3-5.0 |
| Audio compressor `-50 dB` makeup-gain bisect | 5-7 days | 2-3 (Kasada + CreepJS audio + maybe Akamai T1.3 follow-on) | 0.4 |
| Real shader-execution path enabled by default | 5 days (config) + 10 days (per-OS readPixels cluster fakery) | 6 | 0.4 |

### 5.3 Recommendation for v0.1.0

Tier-1 (must ship): the three 1-day fixes — WebGL prototype masking,
WebGL param goldens, canvas toDataURL golden. Combined: 3 days,
**moves every fingerprint-probing vendor in the matrix**.

Tier-2 (should ship): per-profile emoji-font rasterisation. 2-3 days,
big quality win on DataDome / Akamai / Cloudflare consistency scoring.

Tier-3 (post v0.1.0): audio -50 dB parity and `webgl-render` default.
Each is 1-2 weeks. Defer until a specific corpus site fails *because
of* these, which has not been measured (see `15_OPEN_QUESTIONS.md` for
the open audit list).

---

## 6. Detection of detection — how we know which vendors read what

### 6.1 Public solver repos (the gold-vein)

When a third party publishes a working solver, the solver code is
forced to reproduce the **exact** signal payload the vendor expects.
That's the cleanest read-out of "what does vendor X probe?".

- **AWS WAF**: [`xKiian/awswaf`](https://github.com/xKiian/awswaf),
  [`neiii/aws-waf-solver`](https://github.com/neiii/aws-waf-solver),
  [`Switch3301/Aws-Waf-Solver`](https://github.com/Switch3301/Aws-Waf-Solver),
  [`jonathanyly/awswaf-solver-api`](https://github.com/jonathanyly/awswaf-solver-api).
  Per `06_AWS_WAF_SOLVER.md`, the fingerprint payload includes WebGL
  + canvas + audio + 50 navigator/screen properties.
- **Kasada**: [`0x6a69616e/kpsdk-solver`](https://github.com/0x6a69616e/kpsdk-solver),
  [`lktop/kpsdk`](https://github.com/lktop/kpsdk),
  [`nixbro/Kasada-Solver`](https://github.com/nixbro/Kasada-Solver),
  [`Hyper-Solutions/hyper-sdk-js`](https://github.com/Hyper-Solutions/hyper-sdk-js)
  — last one is the commercial multi-vendor reference. Cross-reference
  with `08_KASADA_FRONTIER.md`'s field decode for the in-VM signal list.
- **DataDome**: [`glizzykingdreko/Datadome-Captcha-Deobfuscator`](https://github.com/glizzykingdreko/Datadome-Captcha-Deobfuscator)
  (deobfuscator) + the [Picasso threat-research blog](https://datadome.co/threat-research/the-art-of-bot-detection-picasso-for-device-class-fingerprinting/)
  (vendor's own write-up).
- **Akamai BMP**: [`xiaoweigege/akamai2.0-sensor_data`](https://github.com/xiaoweigege/akamai2.0-sensor_data),
  [`i7solar/Akamai`](https://github.com/i7solar/Akamai),
  [`cirleamihai/akamai-1.7-cookie-generator`](https://github.com/cirleamihai/akamai-1.7-cookie-generator),
  [`Edioff/akamai-analysis`](https://github.com/Edioff/akamai-analysis).
- **Imperva**: [`BottingRocks/Incapsula`](https://github.com/BottingRocks/Incapsula)
  (reese84 + `__utmvc` payload generator).
- **Arkose**: solver list at [`Pr0t0ns/Funcaptcha-Solver`](https://github.com/Pr0t0ns/Funcaptcha-Solver),
  [`decodecaptcha/FunCaptcha-Solver`](https://github.com/decodecaptcha/FunCaptcha-Solver),
  the [`arkose` GitHub topic](https://github.com/topics/arkose).

### 6.2 Open-source classifier libraries (CreepJS, FingerprintJS)

- **CreepJS** ([abrahamjuliot/creepjs](https://github.com/abrahamjuliot/creepjs))
  is the most aggressive open-source canvas+audio+WebGL classifier.
  Its "lies" report ranks every fingerprint surface and tells you
  exactly which ones your headless browser fakes badly. We have
  `crates/browser/tests/diagnostic_creepjs.rs` for self-testing; it's
  `#[ignore]` today (per §2.4.4 audit).
- **FingerprintJS** ([fingerprintjs/fingerprintjs](https://github.com/fingerprintjs/fingerprintjs))
  is the most-deployed open-source FP library. Their `src/sources/canvas.ts`
  is the canonical "what a FP library actually draws". We mirror that
  draw sequence in `crates/browser/tests/diagnostic_browserleaks.rs`.

### 6.3 Captured challenge bundles (when we can deobfuscate)

The K2-DIFF Kasada lever (`08_KASADA_FRONTIER.md §Lever 1`) is the
template: capture the in-VM plaintext sensor (intercept
`XMLHttpRequest.send` / `fetch` on the `/tl` URL, dump body PRE-XOR),
field-diff against a real-Chrome capture, identify which fields
mismatch. AWS WAF's `challenge.js` deobfuscation follows the same
pattern (per `06_AWS_WAF_SOLVER.md`).

### 6.4 Behavioural A/B

When source is impenetrable: A/B test a single signal. Modify just our
canvas hash (e.g., comment out the per-profile jitter at
`canvas2d.rs:1093`), re-run the corpus, see which sites flip. If
DataDome's etsy.com flips to pass and Akamai's adidas.com doesn't, we
know etsy uses canvas-equivalence-class membership and adidas doesn't.
The diagnostic suite at `crates/browser/tests/diagnostic_*.rs` is
designed for exactly this — each diagnostic is a single-signal isolator.

### 6.5 Vendor's own marketing pages

DataDome publishes ([Picasso threat-research](https://datadome.co/threat-research/the-art-of-bot-detection-picasso-for-device-class-fingerprinting/),
[canvas tampering detection](https://datadome.co/anti-detect-tools/canvas-tampering-detection/),
[browser fingerprinting techniques](https://datadome.co/learning-center/browser-fingerprinting-techniques/)).
Cloudflare publishes ([Bot Mgmt blog ML](https://blog.cloudflare.com/cloudflare-bot-management-machine-learning-and-more/)).
These are marketing but often technically accurate.

---

## 7. Acceptance criteria for v0.1.0

- [ ] **Per-technique vendor matrix populated** — done (§2.3, §3.2,
  §4.2 above).
- [ ] **BO coverage audited per technique** — done (§2.4, §3.3, §4.3
  above).
- [ ] **Top-3 cross-vendor leverage fixes identified** — done (§5.2,
  §5.3 above): (1) `WebGLRenderingContext.prototype` mask, (2) per-profile
  WebGL param goldens snapshot test, (3) canvas toDataURL golden
  snapshot test against FingerprintJS+browserleaks+thumbmarkjs probes.
- [ ] **Snapshot tests committed** — Tier-1 fixes from §5.3 land as
  three new tests in `crates/browser/tests/chrome_compat.rs` (or
  alongside in a new `fingerprint_goldens.rs`). The goldens live in
  `crates/browser/tests/fixtures/`.
- [ ] **CreepJS / browserleaks / thumbmarkjs diagnostic tests
  un-ignore** for at least the canvas / WebGL params / audio sum
  components, gating on the per-profile golden.
- [ ] **No regression** in the 437+ chrome_compat tests
  (`cargo test --workspace -- --test-threads=1`).
- [ ] **No regression** in the 126-corpus sweep — measure pre/post
  with `benchmarks/run_full_sweep.sh`; expect ≥ 110 routed pass as the
  current `01_CURRENT_STATE.md` baseline.

---

## 8. Sequencing within v0.1.0

This chapter can run **fully in parallel** with the per-vendor work in
chapters 06/07/08/25/26 because the Tier-1 fixes are mechanical (mask
sweep + golden capture). Recommended order:

1. Day 1: WebGL prototype mask (`canvas_bootstrap.js:1290-1295`
   expansion) + commit + run `chrome_compat`.
2. Day 2: WebGL params per-profile golden — capture, snapshot-test,
   commit.
3. Day 3: Canvas toDataURL golden — capture (FingerprintJS draw
   sequence on chrome_148_macos profile), snapshot-test, commit.
4. Day 4-6 (Tier-2 optional): per-profile emoji-font asset (Apple
   Color Emoji + Segoe UI Emoji + Noto Color Emoji) + profile-driven
   font selection at the text-shaping stack.
5. Day 7+: re-measure 126-corpus sweep; expect ≥ 0 site loss (these
   are quality-preserving mechanical fixes), look for any AWS WAF
   or DataDome pass-rate uplift as a bonus signal.

Total budget: **3-7 days of focused work** (Tier-1: 3 days; Tier-2
optional: +3-4 days). Audio `-50 dB` parity and `webgl-render` default
explicitly deferred to v0.2 per §5.3.

---

## 9. Files referenced

### 9.1 BO source

| File:line | Purpose |
|---|---|
| `crates/canvas/src/canvas2d.rs:976-984` | Comment block: "Chrome's 2D-canvas text IS Skia" — the parity-by-construction argument |
| `crates/canvas/src/canvas2d.rs:1012-1016` | Skia font settings (AntiAlias / subpixel-on / hinting-off) matching Chrome 2D canvas |
| `crates/canvas/src/canvas2d.rs:118-196` | Linear / radial / conic gradient → Skia shader mapping |
| `crates/canvas/src/canvas2d.rs:246-272` | 4-mode pattern repetition (`TileMode` pairs) |
| `crates/canvas/src/canvas2d.rs:347-376` | 27 `globalCompositeOperation` keywords → `BlendMode` |
| `crates/canvas/src/canvas2d.rs:382-427` | `parse_filter_chain` for CSS `ctx.filter` |
| `crates/canvas/src/canvas2d.rs:517-602` | feColorMatrix filter primitives (grayscale/sepia/etc.) |
| `crates/canvas/src/canvas2d.rs:1093-1144` | `to_data_url_with_jitter` — per-profile canvas FP noise |
| `crates/canvas/src/canvas2d.rs:1147-1175` | PNG byte-determinism (zlib-rs DEFLATE + Paeth filter) |
| `crates/canvas/src/canvas2d.rs:1187-1204` | `with_canvas` — CPU Skia surface via `surfaces::wrap_pixels` |
| `crates/canvas/src/canvas2d.rs:1218-1287` | `build_paint` — composite + shadow + filter pipeline |
| `crates/canvas/src/audio.rs:1-31` | Audio module header — Blink DynamicsCompressor port intent |
| `crates/canvas/src/audio.rs:130-234` | `from_params` — full audio FP pipeline |
| `crates/canvas/src/audio.rs:155-177` | The `BLINK_OSCILLATOR_SCALE = 0.47624` calibration |
| `crates/canvas/src/audio.rs:237-689` | `DynamicsCompressorKernel` Rust port |
| `crates/canvas/src/audio.rs:500-504` | Threshold-aware makeup-gain exponent (the open `-50 dB` gap origin) |
| `crates/canvas/src/audio.rs:524-529` | Adaptive release polynomial coefficients (likely-suspect for `-50 dB` fix) |
| `crates/canvas/src/audio.rs:736-836` | Audio tests (determinism, range, wavetable, wave-type, seed variation) |
| `crates/canvas/src/webgl.rs:1-186` | `WebGLParams` profile catalog + 3 baked profiles |
| `crates/canvas/src/webgl_render.rs:1-148` | `WebGLContext` (OSMesa + glow) initialisation |
| `crates/canvas/src/webgl_render.rs:155-330` | Shader / program / buffer / draw pipeline |
| `crates/canvas/src/osmesa_ffi.rs` | OSMesa C FFI (33 lines) |
| `crates/canvas/src/periodic_wave.rs` | Band-limited triangle/sine/square/sawtooth wavetable |
| `crates/canvas/Cargo.toml:12` | `webgl-render = ["glow"]` feature gate |
| `crates/js_runtime/src/js/canvas_bootstrap.js:118-263` | `CanvasRenderingContext2D` class (~40 methods) |
| `crates/js_runtime/src/js/canvas_bootstrap.js:266-585` | `WebGLRenderingContext` class (~80 methods + ~150 constants) |
| `crates/js_runtime/src/js/canvas_bootstrap.js:362-368` | The Kasada `esd.wgl` regression note (static `_g()` accessor) |
| `crates/js_runtime/src/js/canvas_bootstrap.js:369-442` | `static _g()` — profile-driven GPU parameter loader |
| `crates/js_runtime/src/js/canvas_bootstrap.js:443-457` | `getParameter` (string + numeric + array dispatch) |
| `crates/js_runtime/src/js/canvas_bootstrap.js:458-484` | `getSupportedExtensions` (36-extension default) |
| `crates/js_runtime/src/js/canvas_bootstrap.js:485-516` | `getExtension`, `getContextAttributes`, `getShaderPrecisionFormat` |
| `crates/js_runtime/src/js/canvas_bootstrap.js:519-528` | Shader/program/uniform STUBS (the JS-only path) |
| `crates/js_runtime/src/js/canvas_bootstrap.js:583-1014` | Web Audio classes (AudioContext + all AudioNode subclasses) |
| `crates/js_runtime/src/js/canvas_bootstrap.js:637` | `DynamicsCompressorNode` JS class |
| `crates/js_runtime/src/js/canvas_bootstrap.js:775-776` | `createOscillator` / `createDynamicsCompressor` on `BaseAudioContext` |
| `crates/js_runtime/src/js/canvas_bootstrap.js:928-983` | `HTMLCanvasElement` class + getContext + toDataURL wiring |
| `crates/js_runtime/src/js/canvas_bootstrap.js:993-1010` | `Symbol.toStringTag` for Canvas / WebGL / WebGL2 classes |
| `crates/js_runtime/src/js/canvas_bootstrap.js:1260-1295` | Per-prototype `_maskAsNative` calls (the PARTIAL WebGL mask) |
| `crates/js_runtime/src/extensions/audio_ext.rs:1-88` | `op_offline_audio_render` op (the Rust audio pipeline entry point) |
| `crates/js_runtime/src/extensions/audio_ext.rs:52-77` | Per-seed compressor jitter (threshold ± 5 mdB, release ± 0.1 ms) |
| `crates/js_runtime/src/extensions/audio_ext.rs:90-130` | `op_audio_analyser_freq_data` (Blackman + FFT + magnitude + dB clamp) |
| `crates/js_runtime/src/extensions/webgl_ext.rs:1-332` | WebGL ops (gated on `webgl-render` feature) |
| `crates/js_runtime/src/extensions/webgl_ext.rs:43-45` | `op_webgl_available` — false unless `webgl-render` is on |
| `crates/stealth/profiles/chrome_148_macos.yaml:40-41` | `webgl_vendor` + `webgl_renderer` per-profile override |
| `crates/stealth/profiles/chrome_148_macos.yaml:68-69` | `canvas_seed` + `audio_seed` per-profile seed |

### 9.2 BO tests

| File | Purpose |
|---|---|
| `crates/canvas/tests/png_byte_parity.rs` | PNG byte determinism |
| `crates/canvas/tests/png_chunks.rs` | PNG chunk minimality (IHDR/IDAT/IEND) |
| `crates/canvas/tests/canvas_paths.rs` | Path 2D primitives |
| `crates/canvas/tests/font_metrics.rs` | `measureText` vs golden |
| `crates/canvas/tests/audio_reference.rs` | Audio calibration golden (124.04 ± 3 ppm) |
| `crates/canvas/tests/audio_parity.rs` | BO vs Chrome audio parity |
| `crates/browser/tests/chrome_compat.rs:1139-1230` | End-to-end canvas via V8: cls / 2D context / WebGL context / toDataURL / drawing-produces-non-blank |
| `crates/browser/tests/chrome_compat.rs:1232-1357` | WebGL extensions / unmasked vendor & renderer per profile / MAX_TEXTURE_SIZE |
| `crates/browser/tests/chrome_compat.rs:2159-2177` | Symbol.toStringTag for canvas / 2D context / WebGL context |
| `crates/browser/tests/webgl_parity.rs` | WebGL parameter parity vs Chrome |
| `crates/browser/tests/webgl_byte_parity.rs` | readPixels parity (needs `webgl-render`) |
| `crates/browser/tests/webgl_render.rs` | End-to-end render smoke (needs `webgl-render`) |
| `crates/browser/tests/diagnostic_browserleaks.rs` | (`#[ignore]`) browserleaks probe self-test |
| `crates/browser/tests/diagnostic_creepjs.rs` | (`#[ignore]`) CreepJS probe self-test |
| `crates/browser/tests/fingerprint_scorers.rs` | Aggregate FP scoring |
| `crates/browser/tests/fingerprint_suite.rs` | Aggregate FP suite |

### 9.3 Sibling chapters

| Chapter | Why |
|---|---|
| `06_AWS_WAF_SOLVER.md` | AWS WAF — uses canvas + WebGL + audio inside `challenge.js` |
| `07_DATADOME_PRIMITIVES.md` | DataDome — runs Picasso canvas + audio + WebGL inside the challenge iframe |
| `08_KASADA_FRONTIER.md` | Kasada — `ips.js` VM probes canvas + WebGL + audio (and the masking surface) |
| `11_PER_PROFILE_STRATEGY.md` | The per-profile angle of WebGL / canvas / audio seed choice |
| `16_STEALTH_FINGERPRINT_AUDIT.md` | Companion: masking the JS-API surface (the *names* — orthogonal axis to this chapter's *pixel/sample* surface) |
| `17_WEB_API_PARITY_MATRIX.md` §2.10, §2.17 | WebGL / Audio interface inventory |
| `18_ANTI_BOT_VENDOR_COOKBOOK.md` | The per-vendor encyclopedia (used as source for §2.3 / §3.2 / §4.2 matrices) |
| `25_CLOUDFLARE_DEEP.md` | Cloudflare specifics for our canvas/audio/WebGL surface |
| `26_AKAMAI_BMP_DEEP.md` | Akamai sensor specifics (the T1.3 DynamicsCompressor port history) |
| `27_VENDOR_COMPETITIVE_MATRIX.md` | Head-to-head pass-rate data feeding the leverage calculation |
| `42` (synthesis chapter, future) | Will consume §5 of this chapter for the cross-vendor priority synthesis |

### 9.4 External URLs cited

#### Academic / origin papers
- [Mowery & Shacham 2012, "Pixel Perfect"](https://hovav.net/ucsd/dist/canvas.pdf)
- [Acar et al. 2014, "The Web Never Forgets"](https://dl.acm.org/doi/10.1145/2660267.2660347)
- [Englehardt & Narayanan 2016, OpenWPM 1M-site study](https://www.cs.princeton.edu/~arvindn/publications/OpenWPM_1_million_site_tracking_measurement.pdf)
- [audiofingerprint.openwpm.com](https://audiofingerprint.openwpm.com/)

#### Open-source FP libraries
- [FingerprintJS canvas.ts](https://github.com/fingerprintjs/fingerprintjs/blob/master/src/sources/canvas.ts)
- [abrahamjuliot/creepjs (source + demo)](https://github.com/abrahamjuliot/creepjs)
- [CreepJS live demo](https://abrahamjuliot.github.io/creepjs/)
- [thumbmarkjs browser-fingerprinting-techniques](https://www.thumbmarkjs.com/content/browser-fingerprinting-techniques/)
- [WebAudio API issue #1500 (DynamicsCompressor / Oscillator FP)](https://github.com/WebAudio/web-audio-api/issues/1500)

#### Vendor self-publications
- [DataDome: Picasso for device class fingerprinting](https://datadome.co/threat-research/the-art-of-bot-detection-picasso-for-device-class-fingerprinting/)
- [DataDome: Canvas tampering detection](https://datadome.co/anti-detect-tools/canvas-tampering-detection/)
- [DataDome: Browser fingerprinting techniques](https://datadome.co/learning-center/browser-fingerprinting-techniques/)
- [Cloudflare Bot Management ML blog](https://blog.cloudflare.com/cloudflare-bot-management-machine-learning-and-more/)
- [Cloudflare Bot Management docs](https://developers.cloudflare.com/bots/get-started/bot-management/)
- [AWS WAF JS challenge API](https://docs.aws.amazon.com/waf/latest/developerguide/waf-js-challenge-api.html)
- [Fingerprint.com audio fingerprinting blog](https://fingerprint.com/blog/audio-fingerprinting/)

#### Third-party FP test pages
- [browserleaks.com/canvas](https://browserleaks.com/canvas)
- [browserleaks.com/webgl](https://browserleaks.com/webgl)
- [Scrapfly canvas FP test](https://scrapfly.io/web-scraping-tools/canvas-fingerprint)
- [Scrapfly audio FP test](https://scrapfly.io/web-scraping-tools/audio-fingerprint)
- [webbrowsertools.com WebGL FP](https://webbrowsertools.com/webgl-fingerprint/)
- [antidetect.net audio FP](https://antidetect.net/audio-fingerprint.html)
- [Coronium fingerprint guide 2026](https://www.coronium.io/blog/browser-fingerprint-detection-guide)

#### Vendor-bypass guides (third-party)
- [Scrapfly bypass index](https://scrapfly.io/bypass)
- [Scrapfly DataDome bypass](https://scrapfly.io/bypass/datadome)
- [Scrapfly Akamai bypass](https://scrapfly.io/bypass/akamai)
- [Scrapfly Cloudflare bypass](https://scrapfly.io/bypass/cloudflare)
- [Scrapfly PerimeterX bypass](https://scrapfly.io/bypass/perimeterx)
- [Scrapfly Imperva bypass](https://scrapfly.io/bypass/incapsula)
- [Scrapfly AWS WAF bypass](https://scrapfly.io/bypass/aws-waf)
- [Round Proxies — F5/Shape bypass](https://roundproxies.com/blog/f5-bypass/)
- [Round Proxies — AWS WAF bypass](https://roundproxies.com/blog/bypass-aws-waf/)
- [Round Proxies — DataDome bypass](https://roundproxies.com/blog/bypass-datadome/)
- [Round Proxies — Imperva bypass](https://roundproxies.com/blog/bypass-imperva-incapsula/)
- [Round Proxies — FunCaptcha bypass](https://roundproxies.com/blog/bypass-funcaptcha/)
- [Round Proxies — WebGL FP](https://roundproxies.com/blog/webgl-fingerprinting/)
- [scrapebadger — Akamai bypass](https://scrapebadger.com/akamai-bypass)
- [scrapebadger — Imperva bypass](https://scrapebadger.com/imperva-bypass)
- [2captcha — Imperva](https://2captcha.com/h/imperva-bypass)
- [Scrapeless — WebGL fingerprint 2025 guide](https://www.scrapeless.com/en/blog/webgl-fingerprint)
- [proxies.sx — Fingerprinting 2026](https://www.proxies.sx/use-cases/privacy/fingerprinting)
- [mobileproxy.space — Akamai Bot Manager 2026](https://mobileproxy.space/en/pages/akamai-bot-manager-premier-in-2026-architecture-signals-ml-and-operational-tactics.html)
- [Medium — Cloudflare 2026 evasion analysis](https://medium.com/@ayushaggarwal42003/advanced-evasion-techniques-and-architecture-analysis-of-cloudflare-bot-management-systems-in-2026-1b4ba7cc3b22)
- [Habr — FunCaptcha analysis](https://habr.com/en/articles/908464/)

#### Open-source solvers / SDKs
- [xKiian/awswaf](https://github.com/xKiian/awswaf)
- [neiii/aws-waf-solver](https://github.com/neiii/aws-waf-solver)
- [Switch3301/Aws-Waf-Solver](https://github.com/Switch3301/Aws-Waf-Solver)
- [jonathanyly/awswaf-solver-api](https://github.com/jonathanyly/awswaf-solver-api)
- [0x6a69616e/kpsdk-solver](https://github.com/0x6a69616e/kpsdk-solver)
- [lktop/kpsdk](https://github.com/lktop/kpsdk)
- [nixbro/Kasada-Solver](https://github.com/nixbro/Kasada-Solver)
- [Hyper-Solutions/hyper-sdk-js](https://github.com/Hyper-Solutions/hyper-sdk-js)
- [glizzykingdreko/Datadome-Captcha-Deobfuscator](https://github.com/glizzykingdreko/Datadome-Captcha-Deobfuscator)
- [xiaoweigege/akamai2.0-sensor_data](https://github.com/xiaoweigege/akamai2.0-sensor_data)
- [Edioff/akamai-analysis](https://github.com/Edioff/akamai-analysis)
- [BottingRocks/Incapsula (reese84)](https://github.com/BottingRocks/Incapsula)
- [niespodd/browser-fingerprinting (analysis index)](https://github.com/niespodd/browser-fingerprinting)
- [Pr0t0ns/Funcaptcha-Solver](https://github.com/Pr0t0ns/Funcaptcha-Solver)
- [decodecaptcha/FunCaptcha-Solver](https://github.com/decodecaptcha/FunCaptcha-Solver)
- [arkose GitHub topic](https://github.com/topics/arkose)
