# Web API parity deep dive — Web Audio fingerprint

**Scope:** `AudioContext.sampleRate` / `baseLatency` / `outputLatency`,
`OfflineAudioContext` render hash (Oscillator → DynamicsCompressor →
destination), `AnalyserNode.getFloatFrequencyData`, `BiquadFilterNode`,
the per-profile `audio_seed`, and cross-signal coherence with the spoofed
hardware.

**Status:** audit + ranked fixes. Verified against BO source at the
file:line level and against external research (May 2026).

**Read with:**
- `docs/releases/v0.1.0-parity/38_VISUAL_AUDIO_FINGERPRINTING.md §3` — the
  prior category deep-dive on audio (this doc extends and corrects it).
- `docs/releases/v0.1.0-parity/audit/15_FIX_PRIORITY_RANKED.md` row 3
  (FIX-C, commit `93c8ed4`) and row 4a/4b.
- `docs/releases/v0.1.0-parity/17_WEB_API_PARITY_MATRIX.md` — Web Audio
  surface entries.
- Memory `tier1_priority_for_akamai.md` — the historical DynamicsCompressor
  port (Akamai T1.3, Apr-2026).

---

## 1. What the existing repo docs already concluded

### 1.1 doc 38 §3 (Visual & audio fingerprinting)

- **Vendor census:** 8 of 12 catalogued anti-bot vendors confirmed to read
  audio FP, **all 8 leaning on `DynamicsCompressorNode`** (AWS WAF,
  DataDome, Akamai BMP, Cloudflare, Kasada, PerimeterX, Imperva, F5/Shape).
  AnalyserNode/BiquadFilter are second-tier (mostly CreepJS-only). The
  ROI thesis: audio is the *third* highest-leverage fingerprint category
  after WebGL params and canvas text/emoji (doc 38 §1.3).
- **BO backend is a bit-accurate port of WebKit's
  `DynamicsCompressorKernel.cpp`** (Google-authored 2011, BSD-3),
  `crates/canvas/src/audio.rs`. Single-channel mono, f32 throughout to
  match Blink's float32 DSP.
- **What we get right (per doc 38 §3.3.2):** the default-threshold
  (−24 dB) FingerprintJS case matches Chrome 147 to **3.6 ppm** of the
  `sum(abs(data[4500..5000]))` reference (124.04347). Deterministic across
  runs; per-profile differentiation without breaking the calibrated
  baseline.
- **The headline open gap (doc 38 §3.3.3, marked "High"):** the
  CreepJS/Kasada **−50 dB threshold** probe family. The single global
  `BLINK_OSCILLATOR_SCALE = 0.47624` was calibrated to the −24 dB case; at
  −50 dB BO produces 103.92 vs Chrome's ~124.04 (≈16% deviation). A single
  scale cannot fit both thresholds → the bug is in the compressor's
  response curve / makeup-gain math, not the oscillator. doc 38 §3.4 ranks
  this #1, estimated ~1 week.
- **Second gap:** no end-to-end V8/JS-driven
  `OfflineAudioContext.startRendering()` test (the ops are exercised from
  Rust, not the JS surface). doc 38 §3.4 #2, ~1 day.
- **Third gap (Medium):** per-OS audio-stack signature — Chrome uses a
  different FFT on macOS and different SIMD on ARM vs x86; BO's single
  implementation can't reproduce the OS-correlated sum cluster. doc 38
  §3.4 #3 defers this as expensive.

### 1.2 audit/15 row 3 — FIX-C (shipped, commit `93c8ed4`)

FIX-C addressed a **different** audio signal from the offline-render hash:
the **realtime `AudioContext`** telemetry triplet. Before FIX-C,
`canvas_bootstrap.js` set `sampleRate` via `Math.random() < 0.80 ? 44100 :
48000` **per IIFE = per page load**, and `baseLatency`/`outputLatency`
likewise. Within one BO `SharedSession`, sequential page loads reported
*different* sample rates — AWS WAF + DataDome cross-check page-N's value
against page-(N−1) and reject on drift. FIX-C:
- added `StealthProfile.audio_sample_rate: u32` (validated ∈ {44100,
  48000, 96000, 192000}), default 44100, Apple-Silicon presets → 48000;
- derived `baseLatency`/`outputLatency` deterministically from
  `audio_seed` bits so they look like hardware variance but stay stable.

FIX-C flips **0 sites on its own** (telemetry-consistency hardening) but is
a correctness prerequisite for the AWS WAF cluster. Confirmed in the live
code (`canvas_bootstrap.js:839-891`).

---

## 2. New external findings (May 2026)

### 2.1 The offline-render hash is platform-independent in its *input*, but Chrome's *code path* is not

- `OfflineAudioContext` "generates audio as fast as possible … does not
  render to device hardware"; the `sampleRate` you pass the constructor is
  honoured verbatim and is **independent of the actual audio device**
  ([MDN OfflineAudioContext](https://developer.mozilla.org/en-US/docs/Web/API/OfflineAudioContext/OfflineAudioContext),
  [Fingerprint.com audio FP](https://fingerprint.com/blog/audio-fingerprinting/)).
- **But the rendered values still vary by platform**, because "Chrome uses
  a separate fast Fourier transform implementation on macOS … and
  different vector operation implementations on different CPU architectures
  (which are used in the DynamicsCompressor implementation)"
  ([Fingerprint.com](https://fingerprint.com/blog/audio-fingerprinting/),
  echoed by [Octo Browser](https://blog.octobrowser.net/canvas-audio-and-webgl-an-in-depth-analysis-of-fingerprinting-technologies)
  and [DataDome audio FP](https://datadome.co/anti-detect-tools/audio-fingerprint/)).

**Consequence for BO — disentangle two distinct signals:**

| Signal | API | Platform-dependent? | BO source of truth |
|---|---|---|---|
| Realtime context rate | `new AudioContext().sampleRate` | **Yes** — reflects output device (44100 typical macOS, 48000 typical Win/conferencing) | `profile.audio_sample_rate` (FIX-C) |
| Offline render hash | `OfflineAudioContext(1,N,44100)` → compressor sum | **Yes via CPU/FFT/SIMD path**, NOT via the passed rate | `audio.rs` kernel (single impl, profile-blind except `audio_seed` jitter) |

The offline hash is the one **all 8 vendors read**. BO's per-profile
`audio_sample_rate` does **not** touch it — it only changes the realtime
`AudioContext.sampleRate` property and the offline buffer's reported
`.sampleRate` field. So the high-value coherence question is: *does BO's
offline compressor sum land in the cluster the spoofed OS/CPU would
produce?* Today it lands in **one fixed cluster** regardless of profile
(see §3.4).

### 2.2 The "Apple Silicon ⇒ 48000" assumption is unverified and likely backwards

External consensus (MDN, dev.to, Chromium issue trackers) is that **44100
is the most common default and is typical on macOS**, while **48000 is
typical on Windows / conferencing** ([MDN BaseAudioContext.sampleRate](https://developer.mozilla.org/en-US/docs/Web/API/BaseAudioContext/sampleRate),
[dev.to sample-rate](https://dev.to/rijultp/why-sample-rate-matters-when-building-audio-features-in-the-browser-4982)).
No source confirms Apple-Silicon Chrome reports 48000; Apple hardware
commonly runs the built-in output at 44100. BO's preset comment at
`crates/stealth/src/profile.rs:122-129` and `presets.rs:178-181`
asserts "48000 Hz native on Apple Silicon" **without a captured reference**.

This is a **cross-signal liability**: if the real value on the target
device class is 44100 and BO advertises 48000 under a macOS UA, a vendor
with a sampleRate→OS lookup table flags the mismatch. Recommendation:
capture `new AudioContext().sampleRate` on a real Apple-Silicon Chrome 148
before trusting 48000; until then, 44100 is the safer default for the
macOS preset too. (Flagged as FIX-AUDIO-2 below.)

### 2.3 How Camoufox (current open-source SOTA) does it — a different philosophy

Per the camoufox source (deepwiki query, May 2026), Camoufox does **NOT**
re-implement the compressor or offline render. It takes two orthogonal
approaches:

1. **Static property spoofing via `MaskConfig`** at the C++ level:
   `AudioContext:sampleRate` (patches `PreferredSampleRate` in
   `dom/media/CubebUtils.cpp`, default 44100 under RFP),
   `AudioContext:outputLatency`, `AudioContext:maxChannelCount` (default 2).
2. **`AudioFingerprintManager` — per-context output perturbation**: a
   `uint32` `audio:seed` (random 1..2³²−1, never 0) drives a deterministic
   transform applied at the **read points**, not the DSP: a **0.8 %
   variance with a non-linear polynomial component** on float data, and
   ±1 integer adjustments on byte data, injected into
   `AudioBuffer.getChannelData`, `AudioBuffer.copyFromChannel`, and **all
   four** `AnalyserNode` getters (`getFloatFrequencyData`,
   `getByteFrequencyData`, `getFloatTimeDomainData`,
   `getByteTimeDomainData`). Resolves per `userContextId` so it is
   coherent within a browsing context and inherited into Web Workers.

**Architectural contrast with BO:**

| Axis | Camoufox | browser_oxide |
|---|---|---|
| Compressor math | Gecko's real DSP (untouched) | hand-port of Blink's kernel (`audio.rs`) |
| Per-profile entropy | 0.8 % polynomial farble at the read points | ±5 mdB threshold / ±0.1 ms release jitter + ~1e-6 osc gain/phase, then run through the kernel (`audio_ext.rs:62-77`, `audio.rs:137-140`) |
| Baseline cluster | Gecko cluster (Firefox audio, ~35.7 sum) | Blink cluster (Chrome audio, ~124.04 sum) — **strictly the right target for a Chrome-stealth engine** |
| AnalyserNode realtime | real Gecko render, then farbled | **not rendered — returns silence** (§3.5) |
| Cross-OS variation | none (one Gecko build) — **same gap as BO** | none (one Rust impl) |

The strategically important point: **BO targets the Chrome (Blink) audio
cluster, which is correct** — Camoufox is a Firefox fork and lands in the
Gecko cluster, which is a *tell* against a Chrome UA. So on the **default
−24 dB probe, BO's audio FP is closer to Chrome than Camoufox is.** The
two places BO can still lose to Camoufox: (a) the −50 dB probe family where
BO's port is 16 % off, and (b) realtime AnalyserNode probes that BO returns
as silence (Camoufox returns real-but-farbled data). Neither of those is
known to gate a specific corpus site yet.

Sources: [Camoufox audio (deepwiki)](https://deepwiki.com/daijro/camoufox),
[Fingerprint.com Safari-17 audio bypass](https://fingerprint.com/blog/bypassing-safari-17-audio-fingerprinting-protection/),
[WebAudio issue #1500 — DynamicsCompressor/Oscillator FP](https://github.com/WebAudio/web-audio-api/issues/1500),
[scrapfly audio FP test](https://scrapfly.io/web-scraping-tools/audio-fingerprint).

---

## 3. BO code-level analysis

### 3.1 The render pipeline (correct, well-built)

JS `OfflineAudioContext.startRendering()`
(`canvas_bootstrap.js:949-1004`) reads `audio_seed` via
`op_get_profile_value` using exact 32-bit truncation
(`BigInt.asIntN(32, …)`, line 975 — deliberately avoids the
`parseInt | 0` u64→i32 collision documented inline) and calls
`op_offline_audio_render` (`audio_ext.rs:32-88`), which builds
`AudioParams` and calls `AudioFingerprint::from_params`
(`audio.rs:130-234`). Node-graph wiring is complete and correct:

- `OscillatorNode.type` / `frequency.value` setters forward to
  `_setOscType` / `_setOscFreq` on the context
  (`canvas_bootstrap.js:679-693`).
- `DynamicsCompressorNode` threshold/knee/ratio/attack/release are
  `AudioParam`s whose setters forward to `_setComp*`
  (`canvas_bootstrap.js:719-728`), so a fingerprinter that overrides the
  defaults (the −50 dB / knee-40 case) actually changes the render inputs.

This is a genuine strength — many stealth tools stub `startRendering` to
return zeros, which is trivially detectable. BO renders real, correctly
parameterised DSP.

### 3.2 The −24 dB calibration (locked, correct)

`BLINK_OSCILLATOR_SCALE = 0.47624` (`audio.rs:177`) yields
`sum(abs(data[4500..5000])) = 124.04347` to ~3.6 ppm at default params,
backed by the golden in `crates/canvas/tests/audio_reference.rs`
(`CHROME_REFERENCE_SUM = 124.04347527516074`). The threshold-aware makeup
exponent at `audio.rs:500-505`:

```rust
let exponent = if db_threshold <= -24.0 {
    0.6 + 0.0739 * ((-db_threshold - 24.0) / 26.0)
} else { 0.6 };
```

is an **empirical patch**, not a derived correction — the inline comment
admits it was fitted by 2 iterations to interpolate toward 124.04 at
−50 dB. It is the prime suspect for the −50 dB miss.

### 3.3 The per-seed jitter (sound design)

Two independent jitter sources, both `sin()`-based so **seed 0 ⇒ zero
jitter** (preserves the calibrated baseline):
- `audio_ext.rs:64-65`: threshold ±5 mdB, release ±0.1 ms.
- `audio.rs:137-140`: oscillator phase offset ~1e-7 and gain ~1e-6.

This is more conservative and more *physically plausible* than Camoufox's
0.8 % blanket farble — BO perturbs DSP *inputs* and lets the kernel
propagate them, so the output stays on the correct response manifold. Good.

### 3.4 The −50 dB threshold miss (the #1 measurable gap, confirmed)

`audio.rs:163-176` documents it precisely: at threshold = −50, scale
0.47624 → sum 103.92 (−16 %); scale 0.81047 → 172.6 at −24 dB (+39 %). No
single global scale fits both. CreepJS and the Kasada `ips.js` audio probe
both use threshold = −50, knee = 40, ratio = 12. **This is the only audio
signal where BO measurably diverges from Chrome on a probe vendors
actually send.** Root cause is in the static-curve / makeup-gain math
(`saturate`/`k_at_slope`/`update_static_curve_parameters`,
`audio.rs:385-452`, and the makeup-gain exponent at `:500-505`), NOT the
oscillator. Public-engine fixable; ~1 week to bisect against a captured
Chrome 148 −50 dB golden.

### 3.5 Realtime AnalyserNode returns silence (functional gap)

`AnalyserNode._timeDomain` is initialised to `null`
(`canvas_bootstrap.js:769`) and is **never populated** by a connected
source (`connect()` is a no-op, `canvas_bootstrap.js:669`). So the common
realtime probe — `osc.connect(analyser); analyser.getFloatFrequencyData(a)`
— returns all `minDecibels` (−100 dB, i.e. silence) via the early-return at
`canvas_bootstrap.js:784-787`. The FFT op (`op_audio_analyser_freq_data`,
`audio_ext.rs:126-201`) is correct and spec-compliant (Blackman window
coefficients a0=0.42/a1=0.5/a2=0.08, rustfft, dB clamp), but it never
receives a non-silent time-domain buffer. A vendor that renders a tone
into an analyser and checks for a non-silent spectrum would see BO produce
flat −100 dB. **Lower priority** — the dominant vendor probe is the
*offline compressor sum*, and no corpus site is currently known to gate on
realtime analyser spectra. But it is a clean, bounded fix.

### 3.6 Missing node-shape properties (low-severity tells)

Verified absent in `canvas_bootstrap.js`:
- `AudioNode` exposes no `numberOfInputs`, `numberOfOutputs`,
  `channelCount` (Chrome 2), `channelCountMode`, `channelInterpretation`
  (the base class is bare, `canvas_bootstrap.js:667-671`).
- `DynamicsCompressorNode` has no read-only `reduction` property (real
  Chrome exposes `compressor.reduction` as a negative-dB float; CreepJS
  reads it).
- `AudioDestinationNode.maxChannelCount` is hard-coded 2
  (`canvas_bootstrap.js:823`) — fine for desktop, but not profile-driven.

These are enumerable-shape probes (a fingerprinter walking node prototypes
or reading `.reduction`). Not known to gate any corpus site; cheap to add.

### 3.7 BiquadFilter (correct, but `_sampleRate` not wired)

`op_audio_biquad_response` (`audio_ext.rs:340-389`) implements the W3C
§1.7 bilinear-transform coefficients correctly. One nit:
`getFrequencyResponse` reads `this._sampleRate || 44100`
(`canvas_bootstrap.js:746`), but `_sampleRate` is never set on the
`BiquadFilterNode`, so it always uses 44100 even under a 48000 profile.
Minor coherence gap; biquad FP is rare.

---

## 4. Ranked fix list (ROI order)

ROI = (probe prevalence among the 8 audio-reading vendors) × (measurable
divergence from Chrome) ÷ effort. None of these is *known* to flip a
specific corpus site today — the AWS WAF cluster blocker is the live-nav
drain (HANDOFF_2026_05_28b §5.1), not audio — so these are
coherence/hardening fixes that reduce the audio attack surface and remove
cross-signal tells. Public-engine for all; no vendor_solvers needed
(audio FP correctness is generic, not per-vendor bypass).

### FIX-AUDIO-1 — Close the −50 dB DynamicsCompressor parity gap
- **What:** Re-derive the makeup-gain / static-curve math so the offline
  sum matches Chrome at **both** −24 dB and −50 dB. Bisect `saturate` /
  `k_at_slope` / makeup-gain (`audio.rs:385-505`) against a freshly
  captured Chrome 148 golden at threshold=−50/knee=40/ratio=12, then
  delete the empirical `0.6 + 0.0739·…` patch (`audio.rs:500-505`) in
  favour of the correct closed form. Add a second golden to
  `audio_reference.rs`.
- **Effort:** ~1 week (the kernel comment estimates the same).
- **Expected impact:** removes BO's only measurable audio divergence on a
  live vendor probe. Touches CreepJS + Kasada audio buckets; contributes
  to Kasada-cluster coherence (canadagoose/hyatt/realtor) and any
  CreepJS-style holistic scorer. Likely 0 direct flips but removes a
  standing tell; **the single most defensible audio investment.**
- **Confidence:** medium (root cause localized; closed form is known DSP).
- **Engine:** public (`crates/canvas/src/audio.rs`).

### FIX-AUDIO-2 — Verify / correct the per-platform realtime sampleRate
- **What:** Capture `new AudioContext().sampleRate` on a real Apple-Silicon
  Chrome 148 (and a Windows Chrome 148) and reconcile against the preset
  claims (`profile.rs:122-129`, `presets.rs:178-181`). If Apple Silicon is
  actually 44100, change the macOS presets back to 44100. Add the captured
  values as asserted goldens. Also wire `BiquadFilterNode._sampleRate` from
  the profile (`canvas_bootstrap.js:746`).
- **Effort:** 2-3 hours (mostly a capture + a constant change).
- **Expected impact:** removes a potential sampleRate→OS cross-signal
  mismatch that would betray the macOS profile. Touches the AWS WAF /
  DataDome telemetry consistency surface FIX-C started. 0 direct flips, but
  the 48000 claim is currently *unsourced and likely wrong*.
- **Confidence:** high (the divergence from external consensus is clear;
  needs one capture to settle).
- **Engine:** public (`crates/stealth`).

### FIX-AUDIO-3 — Add the missing node-shape + `reduction` properties
- **What:** Add `numberOfInputs`/`numberOfOutputs`/`channelCount`(=2)/
  `channelCountMode`(="max"|"clamped-max")/`channelInterpretation`
  (="speakers") to `AudioNode` (`canvas_bootstrap.js:667`); add read-only
  `DynamicsCompressorNode.reduction` (compute from the kernel's metering
  gain, or return a plausible small negative dB after render); make
  `AudioDestinationNode.maxChannelCount` profile-driven.
- **Effort:** 3-4 hours.
- **Expected impact:** closes enumerable-shape and `.reduction` probes
  (CreepJS reads `reduction`). 0 known flips; pure surface hardening.
- **Confidence:** high (mechanical, spec-defined values).
- **Engine:** public.

### FIX-AUDIO-4 — Realtime AnalyserNode should render a real spectrum
- **What:** When a source (oscillator/buffer) connects to an
  `AnalyserNode` and a render/tick occurs, populate `_timeDomain` with the
  source's samples so `getFloatFrequencyData` returns a real spectrum via
  the already-correct `op_audio_analyser_freq_data`, instead of silence
  (`canvas_bootstrap.js:769,784-787`). Apply the same per-`audio_seed`
  perturbation philosophy as the offline path so it stays coherent.
- **Effort:** 1-2 days (needs a minimal connect-graph + a synchronous tick
  for the analyser; the DSP op already exists).
- **Expected impact:** removes the "analyser returns silence" tell vs real
  Chrome / Camoufox. Realtime analyser probes are rarer than the offline
  sum; **defer unless a corpus site is shown to read it.**
- **Confidence:** medium.
- **Engine:** public.

### FIX-AUDIO-5 — End-to-end JS-driven OfflineAudioContext golden test
- **What:** Add a `chrome_compat.rs` test that drives
  `new OfflineAudioContext(1,5000,44100)` → osc(triangle,10k) →
  compressor(default) → `startRendering()` from V8/JS and asserts the tail
  sum ≈ 124.04 (and, after FIX-AUDIO-1, the −50 dB sum). Currently the ops
  are only exercised from Rust (doc 38 §3.3.4 gap).
- **Effort:** ~1 day.
- **Expected impact:** regression guard so FIX-AUDIO-1/3 can't silently
  break the JS path. 0 flips; process quality.
- **Confidence:** high.
- **Engine:** public.

### Deferred (documented, not recommended now)
- **Per-OS FFT/SIMD signature** (doc 38 §3.4 #3): conditionally compile the
  inner loop to mimic the macOS FFT path / ARM-vs-x86 accumulation order so
  the offline sum lands in the *spoofed* OS cluster rather than one fixed
  cluster. High effort (the order-of-accumulation must match Blink's
  vectorised loop per arch), and no corpus site is known to cross-check the
  audio sum against the OS cluster. **Defer until evidence.** Note Camoufox
  has the *identical* gap (one Gecko build), so this is not where BO loses
  to v150.

---

## 5. Bottom line

BO's audio fingerprint is **architecturally ahead of Camoufox for a
Chrome-stealth target**: it renders a real Blink-cluster offline-compressor
output (124.04 sum, 3.6 ppm at −24 dB) rather than landing in the Gecko
cluster. The residual audio surface is three concrete, public-engine
fixes: (1) the −50 dB makeup-gain miss (the one real divergence on a live
vendor probe), (2) the unsourced/likely-wrong Apple-Silicon 48000
sampleRate, and (3) missing node-shape/`reduction` properties. None is
known to flip a corpus site on its own — the AWS WAF cluster is gated by
the live-nav async drain, not audio — so these are coherence/hardening
work that close standing tells and protect the cross-signal story.
