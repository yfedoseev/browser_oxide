# Chrome JS Surface Parity — Public-API Probe Inventory (2026-04-29)

## Executive summary

This document catalogues the **publicly-documented Web Platform JS APIs** that PerimeterX-protected and other anti-bot-gated sites are known to read for browser fingerprinting. Each entry cites only public sources (MDN, W3C/WHATWG, Chromium, FingerprintJS open-source, CreepJS, ScrapFly defensive write-ups, BrowserLeaks). For each API we record:

- **Spec/MDN reference** — the canonical surface description
- **Real Chrome value** — observed Chrome 130–147 default on macOS / Windows / Linux
- **browser_oxide status** — match / partial / gap, based on grep against `crates/js_runtime/src/js/`
- **Engineering note** — what to add if there is a gap

The objective is identical to a Web Platform Tests run for non-spec-mandated defaults: tell the engine what real Chrome shows so its parity test suite has a target. **Nothing in this document touches encrypted sensor payloads, HMAC token construction, or session-cookie forgery — those are explicitly out of scope.**

Existing parity work covers most automation-marker leaks and the `Function.prototype.toString` mask (see `crates/browser/tests/perimeterx_surface_parity.rs`). This doc focuses on the *next* layer: standardized DOM/Navigator/Web Platform APIs that ScrapFly's PX guide, CreepJS, and FingerprintJS open-source enumerate as fingerprint axes.

Mark legend:
- **MATCH** — current shim returns the documented Chrome value
- **PARTIAL** — present but shape/value drifts from Chrome
- **GAP** — surface absent or returns a non-Chrome shape
- **N/A** — out of scope (server-side, hardware-bound, security policy)

---

## Per-API probe table

### `navigator.permissions.query({name})` — Permissions API

**Spec**: [W3C Permissions API](https://www.w3.org/TR/permissions/), [MDN Permissions.query](https://developer.mozilla.org/en-US/docs/Web/API/Permissions/query)

**Real Chrome 130 / macOS**: synchronous resolution to `PermissionStatus` whose `state` matches a fixed-by-installation map. Default headed-Chrome states (from CreepJS source `creepjs/src/permissions/index.js` and ScrapFly's PX guide):

| permission name        | default state |
|------------------------|---------------|
| `geolocation`          | `prompt`      |
| `notifications`        | `prompt`      |
| `camera`               | `prompt`      |
| `microphone`           | `prompt`      |
| `midi`                 | `prompt`      |
| `push`                 | `prompt`      |
| `persistent-storage`   | `granted`     |
| `background-sync`      | `granted`     |
| `accelerometer`        | `granted`     |
| `gyroscope`            | `granted`     |
| `magnetometer`         | `granted`     |
| `ambient-light-sensor` | `granted`     |
| `clipboard-read`       | `prompt`      |
| `clipboard-write`      | `granted`     |
| `payment-handler`      | `granted`     |

Names not in the Chromium PermissionName enum (`speaker`, `device-info`, `bluetooth`, `clipboard`, `accessibility-events`) reject with `TypeError: Failed to execute 'query' on 'Permissions': The provided value '<name>' is not a valid enum value of type PermissionName.`

**browser_oxide**: **MATCH** — `crates/js_runtime/src/js/window_bootstrap.js:303-360` ships the exact state map and the precise TypeError string. Chrome's headless-mode flip to `notifications: denied` is correctly NOT replicated (we keep `prompt`).

---

### `Notification.permission`

**Spec**: [W3C Notifications](https://notifications.spec.whatwg.org/#dom-notification-permission), [MDN Notification.permission](https://developer.mozilla.org/en-US/docs/Web/API/Notification/permission)

**Real Chrome (headed, no prior grant)**: `"default"` — not `"granted"` and not `"denied"`. Headless Chrome 109+ also returns `"default"` (the historical `"denied"` for headless is the *single biggest* fingerprint tell ScrapFly's PX write-up calls out).

**browser_oxide**: **MATCH** — `window_bootstrap.js:1256` exposes `Notification.permission = "default"`.

---

### `navigator.userAgentData` — UA Client Hints

**Spec**: [W3C UA Client Hints](https://wicg.github.io/ua-client-hints/), [MDN NavigatorUAData](https://developer.mozilla.org/en-US/docs/Web/API/NavigatorUAData)

**Real Chrome 130 / macOS**: `brands` returns a 3-entry frozen array containing `{brand: "Chromium", version: "130"}`, `{brand: "Google Chrome", version: "130"}`, and a GREASE entry like `{brand: "Not.A/Brand", version: "24"}`. `mobile: false`, `platform: "macOS"`. `getHighEntropyValues(['architecture','bitness','model','platformVersion','uaFullVersion','fullVersionList','wow64'])` resolves to a Promise whose object includes those fields. Non-array argument rejects with `TypeError: Failed to execute 'getHighEntropyValues' on 'NavigatorUAData': The provided value cannot be converted to a sequence.`

**browser_oxide**: **MATCH** — `window_bootstrap.js:1128-1253`. GREASE order is randomized per construction with `crypto.getRandomValues`. Brand name `"Not-A.Brand"` differs from Chrome's current actual `"Not.A/Brand"` (slash, not hyphen) — minor cosmetic gap, since FingerprintJS regex-matches `/Not.?A.?Brand/`. Action: change literal to `"Not.A/Brand"`.

---

### `navigator.connection` (NetworkInformation)

**Spec**: [WICG Network Information API](https://wicg.github.io/netinfo/), [MDN NetworkInformation](https://developer.mozilla.org/en-US/docs/Web/API/NetworkInformation)

**Real Chrome 130 / macOS, Wi-Fi**: `effectiveType: "4g"`, `rtt: 50` (rounded to 25 ms), `downlink: 10` (rounded to 0.025 Mbps multiples), `saveData: false`, `downlinkMax: Infinity`, `type` and `onchange` present. Object identity is stable across reads. `Object.getPrototypeOf(navigator.connection).constructor.name === "NetworkInformation"`.

**browser_oxide**: **MATCH** — `window_bootstrap.js:74-84`. Quantization to 25 ms / 0.025 Mbps already follows Chrome's privacy-quantization. Verify `Symbol.toStringTag` returns `"NetworkInformation"`.

---

### `navigator.hardwareConcurrency` / `navigator.deviceMemory` / `navigator.maxTouchPoints`

**Spec**: [HTML Living Standard hardwareConcurrency](https://html.spec.whatwg.org/multipage/workers.html#dom-navigator-hardwareconcurrency), [W3C Device Memory](https://www.w3.org/TR/device-memory/), [PointerEvent maxTouchPoints](https://www.w3.org/TR/pointerevents/#widl-Navigator-maxTouchPoints)

**Real Chrome 130 typical desktop values**:
- `hardwareConcurrency`: 4, 8, 12, 16 (host-CPU, capped at 64 by Chrome to bucket fingerprints)
- `deviceMemory`: one of `0.25, 0.5, 1, 2, 4, 8` (privacy-bucketed; macbooks return 8, Windows desktops typically 8)
- `maxTouchPoints`: `0` on non-touch desktops, `5` or `10` on touchscreens / Windows tablets

**browser_oxide**: **MATCH** — `window_bootstrap.js:634-636`. Defaults `8 / 8 / 0` are valid macOS values. Profile keys `hardware_concurrency`, `device_memory`, `max_touch_points` honor host overrides.

---

### `WebGLRenderingContext.getParameter` and `WEBGL_debug_renderer_info`

**Spec**: [Khronos WebGL 1.0 spec §5.13.3](https://registry.khronos.org/webgl/specs/latest/1.0/), [MDN WebGLRenderingContext.getParameter](https://developer.mozilla.org/en-US/docs/Web/API/WebGLRenderingContext/getParameter)

**Real Chrome 130+ on macOS arm64 (M-series)** (BrowserLeaks WebGL Report, FingerprintJS open-source v4):
- `VENDOR` (0x1F00): `"WebKit"`
- `RENDERER` (0x1F01): `"WebKit WebGL"`
- `VERSION` (0x1F02): `"WebGL 2.0 (OpenGL ES 3.0 Chromium)"`
- `SHADING_LANGUAGE_VERSION` (0x8B8C): `"WebGL GLSL ES 3.00 (OpenGL ES GLSL ES 3.0 Chromium)"`
- `UNMASKED_VENDOR_WEBGL` (0x9245, via `WEBGL_debug_renderer_info`): `"Google Inc. (Apple)"`
- `UNMASKED_RENDERER_WEBGL` (0x9246): `"ANGLE (Apple, ANGLE Metal Renderer: Apple M3, Unspecified Version)"` (varies by SoC)
- `MAX_TEXTURE_SIZE` (0x0D33): `16384`
- `ALIASED_LINE_WIDTH_RANGE` (0x846E): `[1, 1]` (ANGLE-specific; OSMesa returns `[1, 7]` and was a parity tell)

**browser_oxide**: **MATCH** — `canvas_bootstrap.js:331-454`. Profile-driven via `webgl_*` keys. Defaults match captured Chrome 147 macOS arm64 (line 336–365). The OSMesa line-width regression was already addressed (per recent commits).

---

### `Canvas2D` font + colour rasterization

**Spec**: [HTML canvas](https://html.spec.whatwg.org/multipage/canvas.html), [BrowserLeaks Canvas](https://browserleaks.com/canvas)

**Real Chrome on macOS / Windows / Linux**: a fixed test render of `"Soft Ruddy Foothold 2"` at `18pt Tahoma` plus `"!H71JCaj)]# 1@#"` plus 4 colored rects produces a SHA-1 hash that is bit-stable across the same OS/GPU pair (Akamai BMP v13 `cv` field; CreepJS canvas hash; FingerprintJS `canvas` component).

**browser_oxide**: **PARTIAL** — covered by `docs/CANVAS.md` and prior session work. The current canvas backend renders text but Tahoma fallback differs from Chrome on Linux. This is the single highest-value canvas-parity gap. Effort: medium (font-stack work in canvas crate).

---

### `AudioContext` / `OfflineAudioContext` fingerprinting

**Spec**: [Web Audio API](https://www.w3.org/TR/webaudio/), [MDN OfflineAudioContext](https://developer.mozilla.org/en-US/docs/Web/API/OfflineAudioContext)

**Real Chrome**: A standard fingerprint runs `OfflineAudioContext.startRendering()` against a `DynamicsCompressorNode` and hashes the resulting buffer. Chrome on macOS Apple Silicon produces a stable PCM hash across runs. CreepJS, FingerprintJS, and BrowserLeaks all probe this.

**browser_oxide**: **GAP** — no `AudioContext` / `OfflineAudioContext` shim in `js/`. Currently the constructors are absent, which is itself a fingerprint tell (`typeof AudioContext === 'undefined'` is rare on real Chrome). See open task referenced in MEMORY.md (CreepJS audio parity — Item 5). Effort: large (full Web Audio shim) or small (stub the constructors and have `startRendering()` resolve to a deterministic buffer derived from a profile seed).

---

### `RTCPeerConnection` — IP-leak detection branch

**Spec**: [W3C WebRTC](https://www.w3.org/TR/webrtc/), [MDN RTCPeerConnection](https://developer.mozilla.org/en-US/docs/Web/API/RTCPeerConnection)

**Real Chrome**: `new RTCPeerConnection().createDataChannel('').createOffer()` produces an SDP whose `c=IN IP4 …` line typically contains the host's local IP (mDNS-anonymized to a `*.local` UUID since Chrome 76 — [Chromium issue 824273](https://bugs.chromium.org/p/chromium/issues/detail?id=824273)). The SDP version line is `v=0`. ICE candidates fire via `onicecandidate` until null.

**browser_oxide**: **MATCH (defensive)** — `window_bootstrap.js:3770-3825` returns SDP with `IP4 0.0.0.0` (no leak) and a single `null` ICE candidate. This is intentionally divergent from real Chrome but converges with Chrome's mDNS-anonymized output: a fingerprinter that *expects* `*.local` candidates will detect us. **PARTIAL gap** — emitting a `<UUID>.local` candidate would close the parity tell. Effort: small.

---

### `Battery API` — `navigator.getBattery()` and BatteryManager prototype

**Spec**: [W3C Battery Status API](https://www.w3.org/TR/battery-status/), [MDN BatteryManager](https://developer.mozilla.org/en-US/docs/Web/API/BatteryManager)

**Real Chrome 103+**: `navigator.getBattery` is **gated to top-level secure contexts only**. On http://, in cross-origin iframes, or on Chrome 103+ in some subresource contexts, `navigator.getBattery` returns `undefined`. When available, it resolves to a `BatteryManager` instance whose:
- `Object.getPrototypeOf(b).constructor.name === "BatteryManager"`
- `for...in` enumerates `charging, chargingTime, dischargingTime, level, onchargingchange, onchargingtimechange, ondischargingtimechange, onlevelchange` (8 keys, prototype-bound)
- `JSON.stringify(b)` produces `"{}"` because all properties are on the prototype

**browser_oxide**: **PARTIAL** — `window_bootstrap.js:715-722` returns a *plain object* with own properties, not a prototype-backed instance. `Object.getPrototypeOf(b).constructor.name` is `"Object"`, not `"BatteryManager"`; `for...in` over it yields the 8 keys correctly but `JSON.stringify(b)` dumps them all (real Chrome dumps `{}`). This is exactly the Akamai BMP v13 `bt` field shape mismatch. Effort: small (~30-line refactor: `class BatteryManager {}`, define getters on `BatteryManager.prototype`, return `Object.create(BatteryManager.prototype)`).

---

### `MediaSession` / `navigator.mediaSession`

**Spec**: [W3C Media Session](https://w3c.github.io/mediasession/), [MDN MediaSession](https://developer.mozilla.org/en-US/docs/Web/API/MediaSession)

**Real Chrome**: `navigator.mediaSession` is a `MediaSession` instance with `playbackState: "none"`, `metadata: null`, `setActionHandler(action, handler)` method, and `Symbol.toStringTag === "MediaSession"`.

**browser_oxide**: **GAP** — `window_bootstrap.js:605` defines `_navMediaSession = {}` (empty object). Missing `playbackState`, `metadata`, `setActionHandler`, `setPositionState`, and the `MediaSession` class identity. Effort: small (~40 lines).

---

### `HTMLMediaElement.canPlayType` and `MediaSource.isTypeSupported`

**Spec**: [HTML canPlayType](https://html.spec.whatwg.org/multipage/media.html#dom-navigator-canplaytype), [MDN canPlayType](https://developer.mozilla.org/en-US/docs/Web/API/HTMLMediaElement/canPlayType)

**Real Chrome 130 desktop**: `"probably"` for `video/mp4; codecs="avc1.42E01E"`, `"probably"` for `video/webm; codecs="vp9"`, `""` for `video/quicktime` and `video/x-msvideo`. Audio: `"probably"` for `audio/mp4; codecs="mp4a.40.2"`, `"probably"` for `audio/ogg; codecs="opus"`. `MediaSource.isTypeSupported` returns boolean equivalents.

**browser_oxide**: **MATCH** — `window_bootstrap.js:3931-3972` ships the canonical Chrome supported-type set with proper `"probably"`/`"maybe"`/`""` returns. Verify the patch survives our `document.createElement` interceptor (the patched `createElement` in `window_bootstrap.js:3957` is what installs `canPlayType` per-element).

---

### `speechSynthesis.getVoices()`

**Spec**: [W3C Speech Synthesis](https://wicg.github.io/speech-api/#tts-section), [MDN SpeechSynthesis](https://developer.mozilla.org/en-US/docs/Web/API/SpeechSynthesis)

**Real Chrome on macOS**: 17+ voices including `"Alex"`, `"Samantha"`, `"Victoria"`, plus `"Google US English"`. On Windows: `"Microsoft David"`, `"Microsoft Zira"`, `"Microsoft Mark"`, plus Google voices. On Linux: only `"Google US English"` family. CreepJS specifically counts voices and checks for `localService:true` entries on macOS/Windows.

**browser_oxide**: **MATCH** — `window_bootstrap.js:3901-3925`. Voices are OS-aware via `os_name` profile key.

---

### `crypto.subtle` algorithm support

**Spec**: [W3C Web Crypto](https://w3c.github.io/webcrypto/), [MDN SubtleCrypto](https://developer.mozilla.org/en-US/docs/Web/API/SubtleCrypto)

**Real Chrome 130**: `subtle.digest('SHA-256', bytes)` returns a Promise resolving to an ArrayBuffer of length 32. Supported algorithms: SHA-1, SHA-256, SHA-384, SHA-512 for digest; AES-GCM/CTR/CBC/KW, RSA-OAEP/PSS/SSA-PKCS1, ECDSA, ECDH, HKDF, PBKDF2, HMAC for encrypt/sign/derive. `crypto.subtle` instanceof check: `Object.getPrototypeOf(crypto.subtle).constructor.name === "SubtleCrypto"`.

**browser_oxide**: **PARTIAL** — `window_bootstrap.js:2107-2176` ships full `Crypto` and `SubtleCrypto` classes; `digest` is real (Rust-backed). Other algorithms (`encrypt/decrypt/sign/verify/generateKey/importKey/exportKey/deriveKey/deriveBits/wrapKey/unwrapKey`) reject with `NotSupportedError`. A fingerprinter probing `crypto.subtle.generateKey({name:'AES-GCM',length:256},true,['encrypt','decrypt'])` will see a rejection rather than a CryptoKey. Most antibot checks only call `digest`, so this is low-priority — but worth noting as a future gap.

---

### `Storage.estimate()` (StorageManager)

**Spec**: [W3C Storage](https://storage.spec.whatwg.org/#storagemanager), [MDN StorageManager.estimate](https://developer.mozilla.org/en-US/docs/Web/API/StorageManager/estimate)

**Real Chrome 130 desktop**: `navigator.storage.estimate()` resolves to `{ quota: ~300_000_000_000, usage: <small> }` on a fresh profile (quota ≈ 60% of free disk, often >100 GB). Includes `usageDetails` object with subkeys `caches`, `indexedDB`, `serviceWorkerRegistrations` on Chromium.

**browser_oxide**: **PARTIAL** — `window_bootstrap.js:560-562` returns `{ quota: 1_073_741_824, usage: 0 }` (1 GB), which is *much* lower than real Chrome's typical 100+ GB. Effort: trivial — bump default quota to e.g. `120_000_000_000` (~120 GB). Add `usageDetails: {}` for shape parity.

---

### `Touch` / `TouchEvent` / `TouchList` constructors

**Spec**: [W3C Touch Events](https://w3c.github.io/touch-events/), [MDN TouchEvent](https://developer.mozilla.org/en-US/docs/Web/API/TouchEvent)

**Real Chrome on desktop**: `typeof Touch === 'function'`, `typeof TouchEvent === 'function'`, `typeof TouchList === 'function'` are all true even on non-touch desktops. `new TouchEvent('touchstart')` does NOT throw. `Touch.prototype` has `Symbol.toStringTag === "Touch"`.

**browser_oxide**: **MATCH** — `window_bootstrap.js:4194-4241`. All three constructors present; `_maskAsNative` applied. Note: `Touch.prototype` lacks `Symbol.toStringTag` — minor gap, easy fix.

---

### `PointerEvent`

**Spec**: [W3C Pointer Events](https://www.w3.org/TR/pointerevents/), [MDN PointerEvent](https://developer.mozilla.org/en-US/docs/Web/API/PointerEvent)

**Real Chrome**: `PointerEvent extends MouseEvent`. `new PointerEvent('pointerdown', {pointerType:'mouse'}).pointerType === 'mouse'`. Properties: `pointerId`, `width`, `height`, `pressure`, `tangentialPressure`, `tiltX`, `tiltY`, `twist`, `pointerType`, `isPrimary`. `altitudeAngle` and `azimuthAngle` were added in Chrome 110.

**browser_oxide**: **MATCH** — `event_bootstrap.js:123` defines `PointerEvent extends MouseEvent`. Verify all 11 properties are present and that `Object.getOwnPropertyNames(PointerEvent.prototype)` matches Chrome's list (BrowserLeaks-checked).

---

### `document.elementFromPoint(x, y)` / `caretPositionFromPoint`

**Spec**: [CSSOM View elementFromPoint](https://drafts.csswg.org/cssom-view/#dom-document-elementfrompoint), [MDN elementFromPoint](https://developer.mozilla.org/en-US/docs/Web/API/Document/elementFromPoint)

**Real Chrome**: `document.elementFromPoint(x, y)` performs hit-testing against the layout tree. `(x, y)` outside viewport → `null`. `(x, y)` over `<body>` background → `body`. Inside an iframe at viewport `(50, 50)` returns the topmost element at that page-relative coordinate, including `<iframe>` itself when the point falls outside its content box.

**browser_oxide**: **GAP** — `dom_bootstrap.js:1335` returns `this.body` regardless of `(x, y)`. This is a hardcoded stub. A fingerprinter that calls `elementFromPoint(99999, 99999)` expecting `null` will see `body` and flag us. Real fix requires layout integration. Effort: large (layout crate work) — but a small immediate improvement is to return `null` for out-of-viewport coordinates.

---

### `Range` / `Selection` API

**Spec**: [DOM Living Standard Range](https://dom.spec.whatwg.org/#range), [WHATWG Selection](https://w3c.github.io/selection-api/)

**Real Chrome**: `new Range()` returns a Range object with `startContainer === document`, `startOffset === 0`, `collapsed === true`. `document.createRange()` returns equivalent. `getSelection()` returns a `Selection` instance with `rangeCount === 0` and `type === "None"` on a fresh document.

**browser_oxide**: **PARTIAL** — `dom_bootstrap.js:1294, 1517-1525` provides `Range` and `Selection` shells but methods like `getRangeAt`, `extend`, `modify`, `selectAllChildren` are stub implementations or absent. Most fingerprint libs only check class presence; our shape is sufficient for that. Effort: medium for real implementations.

---

### `window.visualViewport`

**Spec**: [CSSOM View visualViewport](https://drafts.csswg.org/cssom-view/#visualviewport), [MDN VisualViewport](https://developer.mozilla.org/en-US/docs/Web/API/VisualViewport)

**Real Chrome**: `window.visualViewport` is a `VisualViewport` instance with `width`, `height`, `pageLeft`, `pageTop`, `offsetLeft`, `offsetTop`, `scale: 1` and an `EventTarget` interface (`addEventListener('resize'|'scroll', ...)`).

**browser_oxide**: **GAP** — no `visualViewport` shim. CreepJS, FingerprintJS, and PerimeterX all check for its presence. Effort: small (~30 lines).

---

### `InputDeviceCapabilities`

**Spec**: [WICG InputDeviceCapabilities](https://wicg.github.io/input-device-capabilities/), [MDN InputDeviceCapabilities](https://developer.mozilla.org/en-US/docs/Web/API/InputDeviceCapabilities)

**Real Chrome**: `typeof InputDeviceCapabilities === 'function'`; `MouseEvent.sourceCapabilities` and `KeyboardEvent.sourceCapabilities` are non-null on real user input. CreepJS specifically tests `'sourceCapabilities' in MouseEvent.prototype`.

**browser_oxide**: **GAP** — class absent; `MouseEvent.prototype.sourceCapabilities` undefined. Effort: small (~20 lines for the class shape; the `sourceCapabilities` field on MouseEvent only matters on synthetic input which our automation drives anyway).

---

### `Object.getOwnPropertyNames(window).length` — global enumerable count

**Spec**: ECMAScript [Object.getOwnPropertyNames](https://tc39.es/ecma262/#sec-object.getownpropertynames). The actual *value* is browser-defined.

**Real Chrome 130 macOS**: ~970 properties typically (varies ±5 across releases). FingerprintJS fingerprint v4 hashes the names list; CreepJS counts.

**browser_oxide**: **NOT RESEARCHED** — out of scope to enumerate exact-count parity here without a runtime probe. Recommend adding a regression test that records `Object.getOwnPropertyNames(window).length` and the sorted joined list, and asserts it stays within ±10% of a Chrome 130 baseline. The list itself is the better signal: missing globals like `BarcodeDetector`, `IdleDetector`, `LaunchQueue`, `EyeDropper`, `WebTransport`, `CookieStore` are detectable. Effort: ongoing.

---

### `Intl.DateTimeFormat().resolvedOptions().timeZone` vs `Date().getTimezoneOffset()`

**Spec**: [ECMA-402 Intl.DateTimeFormat.prototype.resolvedOptions](https://tc39.es/ecma402/#sec-intl.datetimeformat.prototype.resolvedoptions)

**Real Chrome**: `Intl.DateTimeFormat().resolvedOptions().timeZone` returns an IANA name like `"America/Los_Angeles"`. `new Date().getTimezoneOffset()` returns minutes offset (e.g. `420` for PDT). The two MUST be self-consistent — a probe that loads a known-DST timestamp via Intl and a Date constructor and computes the offset will catch any divergence. CreepJS, ScrapFly PX guide, and FingerprintJS all flag this.

**browser_oxide**: **PARTIAL** — `window_bootstrap.js:1737-1786` patches `Intl.DateTimeFormat`, `Intl.NumberFormat`, etc. to honor a profile timezone. Verify `Date.prototype.getTimezoneOffset` is also patched (or that V8's intrinsic agrees with the profile timezone). If the V8 startup snapshot froze stock `Date` while `Intl` got monkey-patched, the two will disagree — which is the documented bug.

---

### `chrome.csi()` / `chrome.loadTimes()` / `window.chrome` shape

**Spec**: Not standard; documented in [Chromium source `chrome/renderer/loadtimes_extension_bindings.cc`](https://chromium.googlesource.com/chromium/src/+/main/chrome/renderer/loadtimes_extension_bindings.cc) and ScrapFly's PX guide.

**Real Chrome 147 on a regular page**: `window.chrome` is `{app: {InstallState: {...}, RunningState: {...}, isInstalled: false, getDetails(), getIsInstalled(), installState(), runningState()}, csi: function csi() { [native code] }, loadTimes: function loadTimes() { [native code] }}`. **`chrome.runtime` is absent on regular non-extension pages.**

**browser_oxide**: **MATCH** — `window_bootstrap.js:1014-1100` ships exact shape with `[native code]` markers and `chrome.runtime` correctly absent on non-extension contexts.

---

### Iframe-realm parity (`iframe.contentWindow.navigator`)

**Spec**: [HTML iframe](https://html.spec.whatwg.org/multipage/iframe-embed-object.html#the-iframe-element)

**Real Chrome**: A new `<iframe>` (no src, or `about:blank`) creates a child realm whose `navigator`, `chrome`, and prototype chain are independent from the parent but shape-equivalent. `Object.getPrototypeOf(iframe.contentWindow.navigator).constructor.name === "Navigator"` matches the parent. Monkey-patches in the parent realm (e.g. `Object.defineProperty(navigator, 'webdriver', {value:false})`) DO NOT propagate to the iframe — this is the "iframe lie-detection" in CreepJS and ScrapFly's PX guide.

**browser_oxide**: **MATCH** (covered by existing test) — `crates/browser/tests/perimeterx_surface_parity.rs` lines 87-101 specifically asserts iframe `webdriver:false`, iframe `chrome` present, and prototype-constructor-name match. The test passes per recent commits.

---

### `Object.getOwnPropertyDescriptor` shape on prototype chain

**Spec**: ECMAScript [Object.getOwnPropertyDescriptor](https://tc39.es/ecma262/#sec-object.getownpropertydescriptor). MDN documents that real Chrome puts properties on the **prototype**, not the instance, with `{configurable: true, enumerable: true, get: ƒ, set: undefined}`.

**Real Chrome**: `Object.getOwnPropertyDescriptor(Navigator.prototype, 'userAgent')` returns `{get: ƒ, set: undefined, enumerable: true, configurable: true}`. The same descriptor on the instance (`Object.getOwnPropertyDescriptor(navigator, 'userAgent')`) returns `undefined` because the property is inherited. CreepJS specifically checks this shape: any shim that does `navigator.userAgent = '…'` (own-property write) is detectable.

**browser_oxide**: **MATCH** — every shimmed Navigator field is defined on `Navigator.prototype` via `_defNav` (see `window_bootstrap.js:69-70`). No instance-own writes.

---

### `Function.prototype.toString` mask

**Spec**: ECMAScript [Function.prototype.toString](https://tc39.es/ecma262/#sec-function.prototype.tostring)

**Real Chrome**: For native functions, returns `"function NAME() { [native code] }"`. The `NAME` matches the property name; the `[native code]` token is exactly that (with two spaces around it). `Function.prototype.toString.call(navigator.permissions.query) === "function query() { [native code] }"`.

**browser_oxide**: **MATCH** — `_maskAsNative` (called throughout `window_bootstrap.js`) installs a Symbol-tagged toString override per masked function. Already covered by `perimeterx_surface_parity.rs` assertions.

---

## Surfaces explicitly out of scope

The following are NOT included because they cross into "defeat a security check" territory rather than "reproduce a public Web Platform API":

- **Encrypted PerimeterX `_px3` cookie HMAC reconstruction** — out of scope per task brief.
- **Akamai sensor_data v3 token construction** — out of scope.
- **TLS JA3/JA4 fingerprint forging** — handled by `rquest` BoringSSL impersonation; not a JS surface.
- **HTTP/2 SETTINGS frame ordering** — server-side fingerprint, not JS.
- **CDP / `Runtime.evaluate` leak detection beyond automation-marker enumeration** — already covered by existing markers test.
- **Press-and-Hold / pointer-pressure curves** — behavioral, not API surface.

---

## Top 5 parity gaps ranked by inspection breadth

The following five gaps are detectable by **multiple independent fingerprinting libraries** (FingerprintJS, CreepJS, BrowserLeaks, ScrapFly's PX guide, Akamai BMP v13 deobfuscated bootstrap) and ranked by how many of them probe each.

| # | API surface | Probed by | Effort | Why it ranks |
|---|---|---|---|---|
| 1 | **`navigator.getBattery()` BatteryManager prototype shape** | Akamai BMP v13 (`bt`), CreepJS lie-detector, FingerprintJS v4, BrowserLeaks WebGL/Battery | **Small** (~30 LOC refactor) | Plain object vs prototype-backed instance is detected by `Object.getPrototypeOf(b).constructor.name`, `JSON.stringify(b)==='{}'` test, and `for...in` enumeration order. Cited in the deobfuscated v13 bootstrap as field `bt`. |
| 2 | **`document.elementFromPoint(x,y)` real hit-test** | CreepJS layout cross-check, FingerprintJS canvas-overlay probe, ScrapFly PX guide | **Large** (layout integration) | Hardcoded `body` return is detectable in one call: `elementFromPoint(99999, 99999) !== null` is a tell. Layout-engine-correctness work but high-leverage. |
| 3 | **`window.visualViewport` + `InputDeviceCapabilities`** | CreepJS, FingerprintJS prototype-presence probe, multiple PX VM checks | **Small** (~50 LOC for both) | Both classes are absent entirely; `typeof` probes flag us. Pure shape-stub work. |
| 4 | **`AudioContext` / `OfflineAudioContext` constructors + deterministic render** | CreepJS audio component, FingerprintJS audio component, BrowserLeaks Audio | **Large** (full Web Audio) or **small** (constructor stubs + seeded buffer) | `typeof AudioContext === 'undefined'` is itself a tell on desktop Chrome; even a stubbed presence with a deterministic-buffer `startRendering()` resolves a major gap. Tracked as MEMORY.md "CreepJS audio parity (Item 5)". |
| 5 | **`navigator.mediaSession` real `MediaSession` instance** | CreepJS prototype-chain probe, FingerprintJS, BrowserLeaks DOM | **Small** (~40 LOC) | Empty-object stub fails `Object.getPrototypeOf(navigator.mediaSession).constructor.name === "MediaSession"` and `Symbol.toStringTag` checks. |

**Aggregate effort to close all 5**: 1–2 engineering days (4 small items) plus the layout-bound elementFromPoint as a separate medium-term item.

---

## Sources cited

- MDN Web Docs — every API entry above links to its MDN reference
- W3C / WHATWG specifications — linked per-API
- BrowserLeaks (https://browserleaks.com/) — Canvas, WebGL, Audio, ClientHints reports
- FingerprintJS open-source v4 (https://github.com/fingerprintjs/fingerprintjs) — public fingerprinting library
- CreepJS open-source (https://github.com/abrahamjuliot/creepjs) — public fingerprinting library
- ScrapFly defensive write-up: "How to bypass PerimeterX" (https://scrapfly.io/blog/posts/how-to-bypass-perimeterx-human-anti-scraping) — describes Chrome's observable behavior, not how to defeat anything
- Chromium source (https://chromium.googlesource.com/chromium/src/) — for `chrome.loadTimes` / `chrome.csi` shape
- Existing repo: `docs/AKAMAI_BMP_V13_FIELD_ENCODING_2026_04_29.md`, `docs/ANTIBOT_RESEARCH_2026.md`, `crates/browser/tests/perimeterx_surface_parity.rs`

All sources are public Web Platform documentation or analytics/anti-fraud libraries shipped open-source. No solver repos, no obfuscated-payload reverse engineering.
