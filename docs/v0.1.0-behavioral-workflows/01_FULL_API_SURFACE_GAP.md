# Full Web API surface gap — what a real Chrome 148 exposes that BO is missing, stubbing, or rendering incoherent

**Owner scope:** structural / API completeness (cross-cutting); feeds Akamai bmak
(mobile sensor_data), Kasada holistic ML + `bot1225`, DataDome, CreepJS/BotD,
PerimeterX `'X' in window` probes.
**Status:** new (2026-05-29). Branch `fix/v0.1.0-fix4-canvas-parity`.
**Method:** code-level audit (file:line) of `interfaces_bootstrap.js` (the
~1100-name list), `window_bootstrap.js`, `cleanup_bootstrap.js`,
`worker_bootstrap.js`, plus a live in-process enumeration probe (built, run,
deleted) and external research.

## 0. Read first (do not duplicate)

This doc is scoped to **API presence/shape/coherence**, the axis-1 + axis-2-shape
half of the parity problem. It deliberately does NOT re-cover:

- `docs/v0.1.0-parity-workflows/api/API_navigator_hardware.md` — the navigator/
  screen **scalar value** coherence audit (FIX-NAV-1..6: deviceMemory clamp,
  vendor/vendorSub profile-read, screen.orientation derive, etc.). **Those fixes
  stand; this doc references them, does not repeat them.** The one overlap I
  re-confirm from runtime is `screen.orientation.type === "landscape-primary"`
  regardless of profile (FIX-NAV-3 still open).
- `docs/v0.1.0-parity-workflows/external/DETECT_vectors.md` — the CreepJS `lies`
  engine (L1 toString, L3 worker-nav shape, L8 stack-trace). Those are *masking*
  artifacts, not missing surfaces. The D1–D8 list there is the masking backlog;
  this doc is the **surface-completeness** backlog and is largely orthogonal.
- `docs/releases/v0.1.0-parity/17_WEB_API_PARITY_MATRIX.md` — the presence matrix.

**Headline finding:** BO's Web API surface is **far more complete than the brief
assumed.** The ~1100-name interface list (`interfaces_bootstrap.js:58`) is
installed, the high-value navigator sub-objects (`mediaDevices`, `permissions`,
`credentials`, `bluetooth`, `usb`, `serial`, `hid`, `locks`, `storage`,
`scheduling`, `clipboard`, `geolocation`, `wakeLock`, `gpu`, `mediaCapabilities`,
`keyboard`, `virtualKeyboard`, `windowControlsOverlay`, `getInstalledRelatedApps`)
all exist and are mostly SecureContext-gated correctly. There is **no large block
of missing globals.** The residual gaps are narrow and specific: (1) the **Generic
Sensor API is non-functional** (Illegal-constructor stubs only — the bmak-mobile
gap), (2) **WebGPU is present-but-broken** (a coherence tell), (3) **RTCPeerConnection
is a shallow class** (no real EventTarget wiring, no `getConfiguration`, empty
`getStats`), (4) **`ondevicemotion`/`ondeviceorientation` are absent from the window
handler set**, and (5) one **named-but-undecoded Kasada `bot1225` API stub.**

---

## 1. Method + what "present" actually means here

A naive `typeof X` enumeration in the default unit harness reports `undefined` for
`navigator.gpu`, `navigator.usb`, `navigator.hid`, `navigator.serial`,
`navigator.bluetooth`, `navigator.storage`, `navigator.locks`,
`navigator.getBattery`, `navigator.userAgentData`, and the whole Generic Sensor
family. **This is correct, not a bug.** The default `BrowserJsRuntime::new(dom)`
runs in an **insecure context** (no profile, `about:blank`-equiv), and
`cleanup_bootstrap.js:8-84` strips every `[SecureContext]`-only API to match real
Chrome on `http:`/`data:`/`about:blank`. On a real navigation to an `https://`
site with a profile, `op_is_secure_context()` returns true and those surfaces are
present. So the audit below distinguishes:

- **Insecure-context absence** = correct (don't touch).
- **Secure-context presence but wrong shape/behavior** = the real gap.

The live probe (in-process, `BrowserJsRuntime`, insecure context) confirmed the
non-gated truths used below: `Accelerometer` → `ReferenceError`,
`'ondevicemotion' in window` → `false`, `RTCPeerConnection.prototype.addEventListener
=== EventTarget.prototype.addEventListener` → `false` (it's a no-op override),
`'getConfiguration' in RTCPeerConnection.prototype` → `false`, `getStats()` →
`[object Promise]` (resolves to empty `Map`), `screen.orientation.type` →
`landscape-primary`, `speechSynthesis.getVoices().length` → `3`,
`performance.memory` → present, `mediaCapabilities` → present, `scheduling` →
present.

---

## 2. Per-surface audit (the surfaces the brief named, ranked by anti-bot relevance)

### 2.1 Generic Sensor API — Accelerometer / Gyroscope / Magnetometer / *OrientationSensor — **GAP (mobile-Android)**

**What a real browser does.** On **Android Chrome** (secure context), the Generic
Sensor API is fully functional: `const a = new Accelerometer({frequency:60}); a.start();
a.onreading = () => {a.x, a.y, a.z}` produces a live stream of float readings;
`Gyroscope`, `Magnetometer`, `LinearAccelerationSensor`, `GravitySensor`,
`AbsoluteOrientationSensor`, `RelativeOrientationSensor` likewise. On **desktop
Chrome** the constructors exist but `start()` emits an `error` event
(`NotReadableError`/no hardware) — i.e. present-and-constructable, but no readings.
On **iOS Safari** none of these interfaces exist (only the legacy `DeviceMotionEvent`/
`DeviceOrientationEvent` with `requestPermission`).

Akamai's BMP/bmak **reads these on mobile**: confirmed externally — the BMP SDK
collector references `gyroscope, magnetometer, accelerometer,
accelerationIncludingGravity, ambient-light-sensor, rotationRate, deviceorientation,
DeviceOrientationEvent, DeviceMotionEvent` and signs a `sensor_data` payload from
them; a missing/empty sensor stream on a mobile UA is a direct bot tell
(Scrapfly/akamai-bmp-generator writeups). DataDome and Kasada also probe
`'Accelerometer' in window` for OS coherence.

**What BO does today.** The sensor classes exist **only as Illegal-constructor
stubs**. They are defined in the `_rest` list at `interfaces_bootstrap.js:58`
(`"Accelerometer","AbsoluteOrientationSensor","Gyroscope","Magnetometer",
"LinearAccelerationSensor","GravitySensor","OrientationSensor",
"RelativeOrientationSensor","Sensor"`) and installed via `_stub(name)`
(`interfaces_bootstrap.js:42-53`), whose body throws
`TypeError: Illegal constructor`. There is **no functional implementation anywhere**
— grep for `onreading`/`class Accelerometer`/`Sensor extends` in
`crates/js_runtime/src/js/*.js` and `extensions/*.rs` returns nothing. They are
then **deleted** in two paths:
- insecure context: `cleanup_bootstrap.js:44-48` (correct — Generic Sensor is
  SecureContext).
- `device_class === "MobileIOS"`: `cleanup_bootstrap.js:168-170` (correct — Safari
  lacks them).

The **Android (`MobileAndroid`) device_class has no special handling**
(`cleanup_bootstrap.js:152` only branches on `MobileIOS`), so on a secure Pixel
profile the sensors are present **but throw on construction** — i.e. an Android
Chrome that cannot `new Accelerometer()`. Worse, even if construction were allowed,
there is no `reading` event source, so bmak's `sensor_data` accelerometer buffer
would be empty/static = bot-like.

**Exact engine change.** Add a functional Generic Sensor implementation gated to
secure + `MobileAndroid` (and a non-reading desktop variant):
1. New `crates/js_runtime/src/js/sensor_bootstrap.js` (or a block in
   `window_bootstrap.js` near the other secure-context installs) defining a real
   `Sensor extends EventTarget` base with `start()`/`stop()`/`activated`/
   `hasReading`/`timestamp` and subclasses `Accelerometer`/`Gyroscope`/
   `Magnetometer`/`LinearAccelerationSensor`/`GravitySensor`/`*OrientationSensor`
   with the right reading axes (`x/y/z` or `quaternion`).
2. Drive readings from a Rust op (`op_sensor_reading` in a new `sensor_ext.rs`, or
   reuse the behavioral engine) so the stream carries **profile-seeded, slowly
   varying, human-plausible** values (a phone at rest has tiny gravity-vector
   jitter, not zeros) — this is the same entropy bmak's encoder reads. Gate on
   `device_class === "MobileAndroid"`.
3. On **Desktop** secure context, keep the constructors but make `start()` async-
   emit an `error` event (`NotReadableError`) and never fire `reading` — matching
   real desktop Chrome. This removes the Illegal-constructor tell while staying
   truthful (desktop has no motion hardware).
4. Wire `ondevicemotion`/`ondeviceorientation` (see §2.4) and make
   `DeviceMotionEvent`/`DeviceOrientationEvent` *dispatch* on mobile with the same
   seeded readings so the legacy path bmak also reads is non-empty.

**Sites it unblocks.** Akamai mobile/behavioral days on a Pixel persona
(bestbuy-mobile, homedepot behavioral/adaptive — though the encoder itself is
`vendor_solvers`, see VENDOR_akamai §Fix-6; this fix produces the *input entropy*
the encoder reads, so it is the public-engine prerequisite). DataDome/Kasada OS-
coherence probes for any Android profile.

### 2.2 WebGPU (`navigator.gpu`) — **GAP: present but incoherent (a tell, not a missing API)**

**What a real browser does.** WebGPU is **stable on desktop Chrome since 113** and
on Android Chrome 121+ (caniuse/Chrome dev blog). On a real Chrome 148 desktop with
a GPU, `navigator.gpu` is a `GPU` instance; `await navigator.gpu.requestAdapter()`
returns a `GPUAdapter` with a populated `features` (a `GPUSupportedFeatures` set
with real entries like `"texture-compression-bc"`, `"timestamp-query"`) and a real
`limits` (`GPUSupportedLimits` with `maxTextureDimension2D: 16384`, etc.), and
crucially **`adapter.requestDevice()` resolves to a working `GPUDevice`**.
`navigator.gpu.wgslLanguageFeatures` is a populated set.

**What BO does today.** `window_bootstrap.js:6096-6112`:
- `requestAdapter()` resolves to an **object literal** (not a `GPUAdapter` instance)
  with `features: new Set()` (empty), `limits: {}` (empty), and
  `requestDevice() { return Promise.reject(new DOMException("Not supported",
  "NotSupportedError")); }`.
- No `wgslLanguageFeatures` (live probe: `gpu.wgslLanguageFeatures` → missing).
- `getPreferredCanvasFormat()` returns `"bgra8unorm"` (plausible).

This is the classic **"feature present but every call fails"** coherence tell. A
detector that does `if (navigator.gpu) navigator.gpu.requestAdapter().then(a =>
a.requestDevice())` sees: GPU exists (so this is a modern Chromium), WebGL reports a
real NVIDIA/Apple GPU (`gpu.rs`), but WebGPU `requestDevice` *always* rejects with
`NotSupportedError` — a combination a real Chrome-148-with-a-real-GPU never
produces. Empty `features`/`limits` and the missing `wgslLanguageFeatures` compound
it. CreepJS and several commercial vendors hash adapter features/limits.

**Exact engine change.** Two options, in increasing fidelity:
- **Low effort (coherence patch):** make `requestAdapter()` return a proper
  `GPUAdapter`-shaped instance (`Object.create(GPUAdapter.prototype)`), populate
  `features` from a profile-keyed feature list (matching the GPU vendor in
  `gpu.rs`), `limits` from the standard Chrome default limit table, add
  `wgslLanguageFeatures`, and make `requestDevice()` **resolve** to a `GPUDevice`
  stub that throws only on actual draw calls (or returns a lost-device promise
  rarely). The goal: `requestDevice()` must not deterministically reject. Keep it
  in `window_bootstrap.js:6094`.
- **Higher effort:** wire a real (or software-rasterizer-backed) WebGPU via a Rust
  op so `getParameter`-equivalent reads are byte-stable per profile. Likely
  overkill unless a corpus site renders WebGPU.
- Either way, set `features`/`limits` to **agree with the WebGL renderer string**
  (an Apple M-series limits profile vs an NVIDIA RTX limits profile differ).

**Sites it unblocks.** CreepJS trust score; AWS-WAF / Kasada holistic ML tail
(canadagoose/hyatt/realtor) where the WebGPU-broken-but-WebGL-real mismatch feeds
the ML score. Not expected to single-flip a site, but it removes a deterministic
axis-2 contradiction.

### 2.3 RTCPeerConnection — **GAP: shallow class, missing EventTarget wiring + methods**

**What a real browser does.** `RTCPeerConnection` is a real `EventTarget`;
`addEventListener('icecandidate', fn)` works (the standard listener path, not just
the `onicecandidate` property); it has `getConfiguration()`, `setConfiguration()`,
`getStats()` resolving to a populated `RTCStatsReport`, `restartIce()`,
`getIdentityAssertion`, `peerIdentity`, `canTrickleIceCandidates`, `sctp`, etc.
WebRTC IP-leak detectors (browserleaks) and CreepJS read candidate count + types.

**What BO does today.** `window_bootstrap.js:5186-5267`. It `extends EventTarget`
(good for the prototype-chain probe — live probe confirmed
`Object.getPrototypeOf(RTCPeerConnection.prototype) === EventTarget.prototype`), and
the **mDNS host candidate emission** (`:5234-5246`) is a thoughtful parity touch
(real Chrome emits one mDNS `.local` host then null — BO does too, avoiding the
"only null candidate" tell). But:
- **`addEventListener`/`removeEventListener` are overridden as no-ops**
  (`:5263-5264`) — live probe confirmed they are NOT the inherited
  `EventTarget.prototype.addEventListener`. So a script that does
  `pc.addEventListener('icecandidate', fn)` (the modern idiom, more common than the
  `onicecandidate` property) **never receives the candidate** — only the
  `onicecandidate=` property path fires (`:5241`). This is a behavioral mismatch: a
  real `RTCPeerConnection` delivers to both.
- **No `getConfiguration()`** (live probe: `'getConfiguration' in proto` → false).
- `getStats()` resolves to an **empty `Map`** (`:5254`), not a populated
  `RTCStatsReport` — a detector reading `report.size === 0` after a connection sees
  a tell.
- Missing `setConfiguration`, `restartIce`, `canTrickleIceCandidates`, `sctp`,
  `getIdentityAssertion`, `peerIdentity`, `currentLocalDescription`/
  `pendingLocalDescription`.

**Exact engine change.** In `window_bootstrap.js:5186`:
1. **Delete the no-op `addEventListener`/`removeEventListener` overrides** so the
   inherited `EventTarget` ones are used, and dispatch a real
   `RTCPeerConnectionIceEvent` via `this.dispatchEvent(...)` in
   `setLocalDescription` (instead of, or in addition to, calling
   `this.onicecandidate` directly). This makes both the property and listener paths
   fire, matching Chrome. `RTCPeerConnectionIceEvent` is already in the interface
   list — give it a real ctor.
2. Add `getConfiguration()` returning the stored config (with Chrome defaults:
   `iceServers:[]`, `iceTransportPolicy:'all'`, `bundlePolicy:'balanced'`,
   `rtcpMuxPolicy:'require'`), `setConfiguration()`, `restartIce()`,
   `canTrickleIceCandidates` getter, `sctp:null`, the `*Local/RemoteDescription`
   accessors.
3. Make `getStats()` resolve to a small but **non-empty** `RTCStatsReport` (a `Map`
   subclass with a `transport`/`candidate-pair`/`local-candidate` entry shaped per
   spec) once a description is set.

**Sites it unblocks.** browserleaks WebRTC page parity; CreepJS WebRTC section;
any site using `addEventListener('icecandidate')` for a real connectivity probe
(the no-op listener is a silent hang → timeout tell). Low single-flip probability
but a clean correctness win.

### 2.4 `ondevicemotion` / `ondeviceorientation` window handlers — **GAP (missing from the 120-handler set)**

**What a real browser does.** Chrome exposes `window.ondevicemotion`,
`window.ondeviceorientation`, `window.ondeviceorientationabsolute` as `null`
event-handler IDL attributes on `Window`. `'ondevicemotion' in window` → `true` on
all platforms (desktop and mobile), even though desktop never fires them.

**What BO does today.** The 120-handler list in `interfaces_bootstrap.js:117-148`
**omits `ondevicemotion`, `ondeviceorientation`, `ondeviceorientationabsolute`**
(grep confirms; live probe: `'ondevicemotion' in window` → `false`). It has
`ongamepadconnected`, `onbeforexrselect`, etc. but not the device-motion trio.

**Exact engine change.** Add `"ondevicemotion", "ondeviceorientation",
"ondeviceorientationabsolute"` to the handler array at
`interfaces_bootstrap.js:117`. One line. On mobile-Android, the §2.1 sensor work
should actually *dispatch* the corresponding events through these handlers so the
legacy `DeviceMotionEvent` path bmak reads is live; on desktop they stay `null` and
never fire (correct).

**Sites it unblocks.** `'ondevicemotion' in window` is a cheap presence probe in
bmak and several BotD-class checks; its absence is a direct "not a real Chrome
Window" signal. Trivial fix, broad relevance.

### 2.5 Kasada `bot1225` — **GAP: one named-but-undecoded missing/wrong-signature API**

**What a real browser does.** Unknown — the specific API is hidden behind Kasada's
`unjzomuyb…` string-table obfuscation. Per
`docs/v0.1.0-parity-workflows/external/VENDOR_kasada.md:61,182-184,265-273`, `bot1225`
is a 28-char API stub that throws an **undefined-receiver** error in BO, indicating
a **missing or wrong-signature Web API** that the Kasada VM probes. It is explicitly
flagged as **public-engine fixable once named** (stub it in the right
`*_bootstrap.js`).

**What BO does today.** The API is either absent or has the wrong call signature;
the VM's call throws and that throw is a Kasada trust signal.

**Exact engine change.** Run the K2-DIFF / in-VM plaintext-sensor decode
(VENDOR_kasada §K-3) to **name** the API, then stub it with the correct signature
in the matching bootstrap. Cannot be specified further without the decode (out of
scope for this static audit) — flagged here as the single remaining *named* surface
gap so it isn't lost.

**Sites it unblocks.** Kasada sites (canadagoose/hyatt/realtor) — per VENDOR_kasada
this is "the biggest trust driver" among the open Kasada items.

---

## 3. Surfaces audited and found ADEQUATE (do not spend effort here)

These were named in the brief; the audit confirms they are present and reasonably
coherent. Listed so future work doesn't re-litigate them.

| Surface | Status | file:line | Note |
|---|---|---|---|
| `navigator.userAgentData` + `getHighEntropyValues` | ✅ impl, profile-driven | `window_bootstrap.js:1786-1844` | SecureContext-gated; brands GREASE-shuffled; see API_navigator_hardware §2.4 |
| `navigator.mediaCapabilities.decodingInfo/encodingInfo` | ✅ impl | `window_bootstrap.js:5928-6060` | real codec table; live probe = `object` |
| `navigator.permissions.query` | ✅ impl + masked | `window_bootstrap.js:469-503, 542, 5601` | Notification-permission-bug coherence handled (DETECT §3) |
| `navigator.plugins` / `mimeTypes` | ✅ impl, profile count | `window_bootstrap.js:273-410` | live count, indexed+named+iterator, masked |
| `navigator.connection` (NetworkInformation) | ✅ impl | `window_bootstrap.js:174-180` | rtt/downlink Chrome-quantized; iOS-gated |
| `navigator.deviceMemory` | ✅ impl (⚠️ sampler clamp bug) | `window_bootstrap.js:979` | **see API_navigator_hardware FIX-NAV-1** (clamp to ≤8); not re-opened here |
| `navigator.hardwareConcurrency` | ✅ impl, profile | `window_bootstrap.js:974` | |
| `navigator.gpu` presence/gating | ✅ present (⚠️ broken calls) | `window_bootstrap.js:6094` | **see §2.2** for the coherence gap |
| `navigator.bluetooth/usb/hid/serial` | ✅ impl, SecureContext-gated | `window_bootstrap.js:694-746, 1028-1031` | correct toStringTags; getUSB/HID/Serial getDevices present |
| `navigator.credentials` (CredentialsContainer) | ✅ impl | `window_bootstrap.js:679, 552-554` | WebAuthn/FedCM branches handled |
| `navigator.locks` (LockManager) | ✅ impl, SecureContext | `window_bootstrap.js:733, 1033` | |
| `navigator.storage` (StorageManager) | ✅ impl, `estimate` masked | `window_bootstrap.js:840, 5604` | |
| `navigator.scheduling.isInputPending` | ✅ impl | `window_bootstrap.js:930-937, 1049-1052` | Chrome-only, correctly absent on iOS |
| `navigator.keyboard` (getLayoutMap/lock) | ✅ impl + masked | `window_bootstrap.js:810, 5582` | |
| `navigator.clipboard` | ✅ impl + masked | `window_bootstrap.js:891, 5603` | |
| `navigator.geolocation` | ✅ impl | `window_bootstrap.js:903-926` | |
| `navigator.wakeLock` | 🟡 impl (empty object w/ toStringTag) | `window_bootstrap.js:927` | `request()` missing — minor; add `request()→Promise<WakeLockSentinel>` |
| `navigator.getBattery` (BatteryManager) | ✅ impl, real class | `window_bootstrap.js:1096-1175` | seeded level/charging, EventTarget, SecureContext |
| `navigator.mediaDevices.enumerateDevices` | ✅ impl + masked | `window_bootstrap.js:416-466, 5602` | |
| `navigator.virtualKeyboard` / `windowControlsOverlay` | ✅ impl | `window_bootstrap.js:6567-6643` | |
| `navigator.getInstalledRelatedApps` | ✅ impl | `window_bootstrap.js:6858-6874` | |
| `navigator.sendBeacon` | ✅ real impl (keepalive fetch) | `window_bootstrap.js:1058-1089` | |
| `window.visualViewport` (VisualViewport) | ✅ impl | live probe = `object` | |
| `window.matchMedia` / `getComputedStyle` | ✅ impl | `window_bootstrap.js:4007+` | media features profile-driven |
| `screen.*` geometry | ✅ impl | `window_bootstrap.js:1432-1485` | **orientation static = FIX-NAV-3** |
| `speechSynthesis.getVoices()` | ✅ impl, OS-shaped | `window_bootstrap.js:2471, 5323` | live probe = 3 voices |
| `document.fonts` (FontFaceSet) | ✅ impl | `window_bootstrap.js:5290-5327` | iterator yields FontFace; ⚠️ `queryLocalFonts` = SecureContext, absent on insecure (correct) |
| `performance.memory` + PerformanceObserver | ✅ impl | live probe = present | |
| `PressureObserver` / `PressureRecord` | ✅ impl | `window_bootstrap.js:5472-5507` | Kasada `esd.cpt` probe |
| `trustedTypes` / Trusted Types | ✅ impl + masked | `window_bootstrap.js:6156-6188` | |
| `scheduler.postTask/yield` | ✅ impl + masked | `window_bootstrap.js:6195-6213` | |
| `reportError` | ✅ impl | `window_bootstrap.js:6219-6227` | |
| Touch/TouchEvent ctors (desktop too) | ✅ impl | `window_bootstrap.js:6234+` | |
| WebGL/WebGL2 | ✅ impl (FIX-D2 split landed) | `canvas_bootstrap.js` | DETECT §4.1 |
| Web Audio (OfflineAudioContext etc.) | ✅ impl + seeded | `audio_ext.rs` | DETECT §4.2 |
| `IntersectionObserver`/`ReportingObserver`/`OffscreenCanvas`/`Notification` | ✅ impl | live probe = function | |

**SecureContext-gated APIs that correctly report `undefined` on insecure pages**
(NOT gaps): `gpu`, `usb`, `hid`, `serial`, `bluetooth`, `storage`, `locks`,
`credentials`, `getBattery`, `userAgentData`, `mediaDevices`, `clipboard`,
`wakeLock`, `serviceWorker`, `queryLocalFonts`, the Generic Sensor family, `caches`,
`cookieStore`, `IdleDetector`, `EyeDropper`, `WebTransport`. Verified the stripping
logic at `cleanup_bootstrap.js:8-84` matches Chrome's insecure-page namespace.

---

## 4. Which behavioral/API-gated sites each gap unblocks

| Gap | Primary detector | Sites |
|---|---|---|
| §2.1 Sensors non-functional | Akamai bmak/BMP `sensor_data` (mobile), DataDome/Kasada OS-coherence | Akamai mobile/behavioral days on Pixel persona (bestbuy, homedepot behavioral/adaptive — input entropy only; encoder is `vendor_solvers`) |
| §2.2 WebGPU broken-but-present | CreepJS, AWS-WAF/Kasada holistic ML | canadagoose/hyatt/realtor (ML tail), amazon-* (AWS-WAF) — score-hardening, not single-flip |
| §2.3 RTCPeerConnection shallow | browserleaks WebRTC, CreepJS | browserleaks/CreepJS diagnostics; any site using `addEventListener('icecandidate')` |
| §2.4 `ondevicemotion` absent | bmak/BotD presence probe | broad — cheap Window-shape tell on every Akamai/BotD page |
| §2.5 Kasada `bot1225` | Kasada VM | canadagoose/hyatt/realtor (biggest open Kasada trust driver) |

**Honest framing (per HANDOFF_2026_05_28b):** the currently-failing hard set is
dominated by axis-2 *execution* (AWS live-nav async drain) and *behavioral* gaps,
not missing-API tells. None of §2.1–2.5 is expected to single-handedly flip
imdb/booking/amazon-in. Their value is (a) **mobile-Android Akamai parity** (§2.1 is
the real one — it produces the sensor entropy the bmak encoder needs), (b)
shrinking the **Kasada/holistic ML tail** (§2.2, §2.5), and (c) **clean
diagnostics** (areyouheadless/CreepJS) via §2.3/§2.4.

---

## 5. Ranked fix list (ROI order)

All public-engine (JS bootstraps + optional Rust ops); none touch `vendor_solvers`.

| ID | Fix | Effort | Confidence | Site impact | Engine |
|---|---|---|---|---|---|
| FIX-API-1 | Add `ondevicemotion`/`ondeviceorientation`/`ondeviceorientationabsolute` to the handler list (`interfaces_bootstrap.js:117`). | 15 min | high | broad presence-probe win; removes a Window-shape tell on every bmak/BotD page | public |
| FIX-API-2 | Functional Generic Sensor API on secure `MobileAndroid` (constructable `Sensor` subclasses + seeded `reading` stream via new `op_sensor_reading`); desktop secure = constructable but `start()`→`error` (NotReadableError); dispatch legacy `DeviceMotionEvent`/`DeviceOrientationEvent` on mobile. Add `MobileAndroid` branch to `cleanup_bootstrap.js:152`. | 3-5 days | high | the **public-engine prerequisite** for Akamai mobile/behavioral parity (bmak `sensor_data` input entropy) on a Pixel persona | public |
| FIX-API-3 | WebGPU coherence: `requestAdapter()`→real `GPUAdapter` instance with profile-keyed `features`/`limits` agreeing with the WebGL renderer; `requestDevice()` must **resolve** to a `GPUDevice` stub (not deterministically reject); add `wgslLanguageFeatures`. (`window_bootstrap.js:6094`). | 1-2 days | medium-high | removes a deterministic axis-2 contradiction; CreepJS/AWS-WAF/Kasada ML score-harden | public |
| FIX-API-4 | RTCPeerConnection: remove no-op `addEventListener`/`removeEventListener` overrides (use inherited EventTarget + `dispatchEvent` a real `RTCPeerConnectionIceEvent`); add `getConfiguration`/`setConfiguration`/`restartIce`/`canTrickleIceCandidates`/`sctp`; non-empty `getStats()` `RTCStatsReport`. (`window_bootstrap.js:5186`). | 1 day | high | browserleaks/CreepJS parity; fixes silent listener hang | public |
| FIX-API-5 | Kasada `bot1225`: run K2-DIFF to **name** the 28-char API, then stub with correct signature in the matching bootstrap. (Blocked on decode; tracked in VENDOR_kasada §K-3.) | 2-3 days (mostly decode) | medium | canadagoose/hyatt/realtor — biggest open Kasada trust driver | public |
| FIX-API-6 | `navigator.wakeLock.request()` → `Promise<WakeLockSentinel>` (currently an empty object). (`window_bootstrap.js:927`). | 30 min | high | low (defensive) | public |

### Sequencing
1. FIX-API-1 + FIX-API-6 (one trivial commit, minutes).
2. FIX-API-4 (RTCPeerConnection correctness — self-contained, 1 day).
3. FIX-API-3 (WebGPU coherence — 1-2 days, needs the limits/features tables).
4. FIX-API-2 (the big one; only worthwhile alongside a mobile-Android persona +
   the bmak input path — coordinate with VENDOR_akamai §Fix-6 / the
   `__akamai_events` buffer work in humanize.js).
5. FIX-API-5 last (gated on the Kasada decode).

### What is explicitly NOT a gap (verified, do not re-open)
- The ~1100 interface list is installed (`interfaces_bootstrap.js:58`).
- All SecureContext-gated navigator sub-objects exist and gate correctly
  (`window_bootstrap.js:1025-1052`; `cleanup_bootstrap.js:8-84`).
- Sensors being `undefined` on **insecure** pages and on **iOS** = correct Chrome
  behavior.
- navigator scalar masking, deviceMemory clamp, vendor/orientation = covered by
  `API_navigator_hardware.md` (FIX-NAV-*), not this doc.
- The masking-artifact axis (toString/stack/worker-shape) = `DETECT_vectors.md`
  D1–D8, orthogonal to surface completeness.

---

## 6. Files referenced
- `crates/js_runtime/src/js/interfaces_bootstrap.js:42-58` (stub factory + ~1100 list),
  `:117-148` (window event handlers — `ondevicemotion` missing)
- `crates/js_runtime/src/js/window_bootstrap.js:694-746` (usb/hid/serial/locks),
  `:1025-1052` (navigator getter installs), `:1096-1175` (BatteryManager),
  `:5186-5269` (RTCPeerConnection), `:5472-5507` (PressureObserver),
  `:5928-6060` (mediaCapabilities), `:6094-6112` (WebGPU)
- `crates/js_runtime/src/js/cleanup_bootstrap.js:8-84` (insecure-context strip),
  `:152-208` (MobileIOS strip — no MobileAndroid branch)
- `crates/js_runtime/src/js/worker_bootstrap.js:86-160` (worker navigator)
- `crates/js_runtime/src/lib.rs:32-93` (`BrowserJsRuntime::new`/`with_profile`/`execute_script`)

### External sources
- [Akamai BMP / bmak sensor_data reads accelerometer/gyroscope/magnetometer/deviceorientation — Scrapfly Akamai bypass](https://scrapfly.io/bypass/akamai)
- [xvertile/akamai-bmp-generator (sensor_data fields) — DeepWiki](https://deepwiki.com/xvertile/akamai-bmp-generator)
- [Websites requesting motion sensors on desktop — grantwinney.com](https://grantwinney.com/websites-requesting-access-to-motion-sensors/)
- [WebGPU stable since Chrome 113 desktop / 121 Android — caniuse](https://caniuse.com/webgpu)
- [What's New in WebGPU (Chrome 147-148) — Chrome for Developers](https://developer.chrome.com/blog/new-in-webgpu-147-148)
- [WebGPU troubleshooting (navigator.gpu / requestAdapter / requestDevice) — Chrome for Developers](https://developer.chrome.com/docs/web-platform/webgpu/troubleshooting-tips)

### Internal docs cited
- `docs/v0.1.0-parity-workflows/api/API_navigator_hardware.md` (FIX-NAV-1..6)
- `docs/v0.1.0-parity-workflows/external/DETECT_vectors.md` (D1–D8 masking backlog)
- `docs/v0.1.0-parity-workflows/external/VENDOR_akamai.md` (§Fix-6, `__akamai_events`)
- `docs/v0.1.0-parity-workflows/external/VENDOR_kasada.md` (§K-3, `bot1225`)
- `docs/HANDOFF_2026_05_28b.md` (axis-2 execution vs lie framing)
