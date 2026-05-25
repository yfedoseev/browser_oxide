# 16 — Stealth fingerprint horizontal audit

**Owner scope:** structural / stealth work (cross-cutting).
**Status:** prerequisite for any per-vendor frontier work (Kasada `sfc`/`sdt`,
DataDome closure-content probes, AWS WAF `getParameter` audits).
**Why this chapter exists:** a contributor fixing one Kasada error field
shouldn't have to rediscover the whole masking architecture. This is the
complete inventory of JS API surfaces BO exposes, which are masked, which
aren't, and which anti-bot vendor is known to probe each.

Read this with:
- `08_KASADA_FRONTIER.md` (the `sfc`/`sdt` Function.toString leak is the canonical
  motivator — see Lever 3 there)
- `02_GAP_ANALYSIS.md` (which sites need which masking)
- `17_WEB_API_PARITY_MATRIX.md` (companion doc: implemented vs missing APIs)
- `13_FILE_LOCATIONS_INDEX.md` (every file:line called out below is in the index)

---

## 1. The masking primitive — how `_maskAsNative` works

The single source of truth is `crates/js_runtime/src/js/stealth_bootstrap.js`
(140 lines). It runs FIRST in the snapshot at `crates/js_runtime/src/snapshot.rs:70`
and FIRST in every worker isolate at `crates/js_runtime/src/runtime.rs:341-343`:

> stealth_bootstrap must run first: installs Function.prototype.toString
> patch and the _nativeTag/_maskFunction/_maskAsNative helpers that
> worker_bootstrap uses.

### 1.1 The three primitives

| Helper | File:line | Purpose |
|---|---|---|
| `_nativeTag` | `stealth_bootstrap.js:13` | `Symbol.for('__browser_oxide_native__')` — global-registry symbol so cross-realm masking works |
| `_maskFunction(fn, name)` | `stealth_bootstrap.js:51-80` | Tag a single function so `String(fn)` returns `"function name() { [native code] }"` |
| `_maskAsNative(obj, ...names)` | `stealth_bootstrap.js:82-104` | Walk the prototype chain to find each named method/getter/setter and mask it |

All three are exposed on `globalThis` (`stealth_bootstrap.js:107-109`) so every
other bootstrap can call them.

### 1.2 The Function.prototype.toString patch

`stealth_bootstrap.js:14-48`:

```js
const _origFnToStr = Function.prototype.toString;
let _inPatchedToStr = false;
const _patchedFnToStr = ({ toString() {
    if (_inPatchedToStr) return _origFnToStr.call(this);
    _inPatchedToStr = true;
    try {
        if (this !== null && this !== undefined) {
            try {
                const tag = this[_nativeTag];
                if (tag) return `function ${tag}() { [native code] }`;
            } catch (_) {}
        }
        return _origFnToStr.call(this);
    } finally {
        _inPatchedToStr = false;
    }
} }).toString;
```

Notes that are easy to get wrong:
- **Method shorthand** (`{ toString() {} }.toString`) is intentional. A plain
  `function toString() {}` is constructable; the method-shorthand form is not,
  matching native `Function.prototype.toString`. Kasada's `fsc` probe records
  the `class X extends Function.prototype.toString {}` outcome (real Chrome:
  `TypeError`; our prior plain-function form silently constructed).
- **Re-entrant guard** (`_inPatchedToStr`) prevents infinite recursion when
  `this[_nativeTag]` access triggers a Proxy `get` trap that itself calls
  `Function.prototype.toString`. This was a real bug in the iframe-realm work.
- **The patched toString is itself tagged** (`stealth_bootstrap.js:41`) so
  recursive `Function.prototype.toString.call(Function.prototype.toString)`
  resolves to `"function toString() { [native code] }"`.

### 1.3 How `_maskFunction` tags a function

`stealth_bootstrap.js:51-80`:

1. Sets `fn.name` to the desired Chrome-visible name (configurable own property,
   matching V8's native shape).
2. Sets `fn[_nativeTag]` to the same name. The patched
   `Function.prototype.toString` reads this tag and synthesizes the
   `[native code]` string.
3. **Does NOT install an own `toString` on `fn`.** Previously we did — it was
   a self-inflicted FP because real Chrome native functions inherit
   `toString` from `Function.prototype`. `Object.getOwnPropertyNames(fn)`
   should return `['length','name'(,'prototype')]`, never include `toString`.
   See the comment block at `stealth_bootstrap.js:62-75` for the full bug.

### 1.4 How `_maskAsNative` resolves prototype-chain methods

`stealth_bootstrap.js:82-104`:

```js
const _maskAsNative = (obj, ...names) => {
    for (const name of names) {
        try {
            let target = obj;
            let desc = Object.getOwnPropertyDescriptor(target, name);
            while (!desc && target && target !== Object.prototype) {
                target = Object.getPrototypeOf(target);
                if (target) desc = Object.getOwnPropertyDescriptor(target, name);
            }
            if (desc) {
                if (desc.get) _maskFunction(desc.get, `get ${name}`);
                if (desc.set) _maskFunction(desc.set, `set ${name}`);
                if (typeof desc.value === 'function') _maskFunction(desc.value, name);
            } else {
                const val = obj[name];
                if (typeof val === 'function') _maskFunction(val, name);
            }
        } catch (e) {}
    }
};
```

- Walks the prototype chain to find where the property actually lives, then
  masks the underlying getter/setter/value on its real descriptor object.
- Methods are masked as `name`. Getters as `get name`. Setters as `set name`.
  Same convention Chrome uses for `Function.prototype.toString` of accessors.
- Silently no-ops if the property doesn't exist or the descriptor is unreadable
  (a frozen exotic). The caller can't tell — write the call defensively and
  verify with an actual `String(SomeAPI.prototype.someMethod)` test (see § 5).

### 1.5 Why Kasada `sfc`/`sdt` matters here

From `08_KASADA_FRONTIER.md` § Phase 3 finding 2:

> Our `Function.prototype.toString` returns BO's literal JS source for
> `attachShadow`, `queueMicrotask`, `fetch`, `HTMLDocument`, `HTMLElement`, etc.
> — including the deno_core op name `op_dom_attach_shadow`. Need a sweeping
> `_maskAsNative` audit across `crates/js_runtime/src/js/*_bootstrap.js`.

There's already a partial sweep at `dom_bootstrap.js:2995-3094`
(a hand-curated list of class names that gets walked). Every entry in the
table in § 2 below that's marked ❌ unmasked or 🟡 partial is a probe surface
Kasada (or a similar holistic-fingerprint vendor) can read and score against.

---

## 2. Masking audit — every JS API surface BO exposes

### 2.1 How to read the table

- **Masked?** column key:
  - ✅ verified masked (own descriptor confirmed via direct `_maskFunction` /
    `_maskAsNative` call OR caught by the `dom_bootstrap.js:2995` sweep)
  - 🟡 partial — some methods masked, others not
  - ❌ unmasked — methods on this prototype return literal JS source
  - N/A — interface is a class declaration only (`Illegal constructor` stub) so
    has no methods to leak

- **Method count** is a rough size hint (the prototype's own method count, not
  including inherited members), so reviewers can prioritize the big surfaces.

- **File:line** is where the interface is defined OR where it's masked.
  Always read the actual file before believing the entry — code drifts.

- **Vendor probes** lists known-from-our-research probes only; absence ≠ safe.

### 2.2 The audit table

| Interface | File:line (def) | Method count | Masked? | Vendor probes |
|---|---|--:|---|---|
| **Window / global** | | | | |
| `Window` (named ctor) | `window_bootstrap.js:46-67` | 0 | ✅ | Akamai, Kasada `bof` |
| top-level `fetch` | `fetch_bootstrap.js:175` | 1 | ✅ (`dom_bootstrap.js:3079-3094`) | Kasada `sfc` |
| top-level `setTimeout/setInterval/clearTimeout/clearInterval` | `timer_bootstrap.js:62-142` | 4 | ✅ (`timer_bootstrap.js:180-186`) | PerimeterX, Akamai |
| top-level `requestAnimationFrame/cancelAnimationFrame` | `timer_bootstrap.js:147-164` | 2 | ✅ (`timer_bootstrap.js:185-186`) | Kasada `rafj` |
| top-level `requestIdleCallback/cancelIdleCallback` | `window_bootstrap.js:3379-3391` | 2 | ✅ | (none observed) |
| top-level `queueMicrotask` | V8 native | 1 | ✅ (`dom_bootstrap.js:3080`) | Kasada `sfc` |
| top-level `structuredClone` | V8 native + `structured_clone.js:177+` | 1 | ✅ (`dom_bootstrap.js:3083`) | Kasada `nppm` |
| top-level `reportError` | `window_bootstrap.js:5970-5976` | 1 | ✅ | (none) |
| top-level `getComputedStyle` | `window_bootstrap.js:3403` | 1 | ✅ (`window_bootstrap.js:3455`) | CreepJS |
| top-level `matchMedia` | `window_bootstrap.js:4007` | 1 | ✅ (`window_bootstrap.js:4013`) | (none) |
| top-level `scroll/scrollTo/scrollBy` | `window_bootstrap.js:1545-1571` | 3 | ✅ (`window_bootstrap.js:1556,1569`) | (none) |
| top-level `alert/confirm/prompt/open/close/postMessage/stop/print` | `window_bootstrap.js:4018-4062` | 8 | ✅ (4047-4062) | (none) |
| top-level `addEventListener/removeEventListener/dispatchEvent` | `event_bootstrap.js:459-461` | 3 | 🟡 (`dom_bootstrap.js:3086-3087`) | Castle |
| top-level `atob/btoa` | `window_bootstrap.js:3182-3228` | 2 | ✅ (`window_bootstrap.js:3202,3228`) | (none) |
| **Navigator surfaces** | | | | |
| `Navigator.prototype` (`userAgent`, `platform`, …) | `window_bootstrap.js:1029-1031` | ~25 | ✅ via `_maskAsNative(_NavProto,…)` (`window_bootstrap.js:1029`) | AWS WAF, Akamai |
| `navigator.permissions` | `window_bootstrap.js:522-541` | 1 (`query`) | ✅ (`window_bootstrap.js:5351`) | AWS WAF, DataDome |
| `navigator.mediaDevices` | `window_bootstrap.js:443-466` | 7 | ✅ per-method `_maskFunction` (`window_bootstrap.js:444-466`) | Castle, Kasada |
| `navigator.clipboard` | `window_bootstrap.js:893-898` | 2 | ✅ (`window_bootstrap.js:895,898`) | (none) |
| `navigator.storage` (`StorageManager`) | `window_bootstrap.js:812-839` | 3 | ✅ (`window_bootstrap.js:831,834,837`) | (none) |
| `navigator.serviceWorker` | `window_bootstrap.js:842-889` | 5+EventTarget | ✅ (`window_bootstrap.js:872-887`) | DataDome |
| `navigator.usb` | `window_bootstrap.js:697-707` | 4 | ✅ (`window_bootstrap.js:704-706`) | (none) |
| `navigator.serial` | `window_bootstrap.js:710-717` | 2 | ✅ (`window_bootstrap.js:715-717`) | (none) |
| `navigator.hid` | `window_bootstrap.js:720-728` | 2 | ✅ (`window_bootstrap.js:726-728`) | (none) |
| `navigator.bluetooth` (`Bluetooth.prototype`) | `window_bootstrap.js:681-693` | 2 | ✅ (`window_bootstrap.js:688,691`) | (none) |
| `navigator.geolocation` (`Geolocation.prototype`) | `window_bootstrap.js:903-926` | 3 | ✅ (`window_bootstrap.js:911,919,922`) | (none) |
| `navigator.locks` | `window_bootstrap.js:730-740` | 2 | ✅ (`window_bootstrap.js:737,739`) | (none) |
| `navigator.connection` (`NetworkInformation`) | `window_bootstrap.js:171-174` | 0 (props only) | N/A — accessor-only | Akamai sensor |
| `navigator.credentials` (`CredentialsContainer.prototype`) | `window_bootstrap.js:637-678` | 4 | ✅ (`window_bootstrap.js:656,670,673,676`) | (none) |
| `navigator.keyboard` (`Keyboard.prototype`) | `window_bootstrap.js:799-809` | 3 | ✅ (`window_bootstrap.js:5332`) | Castle |
| `navigator.userAgentData` (`NavigatorUAData`) | `window_bootstrap.js:1820-1845` | 2 + getters | ✅ (`window_bootstrap.js:1827,1838`) | DataDome ClientHints |
| `navigator.javaEnabled/sendBeacon/getBattery` | `window_bootstrap.js:5331` | 3 | ✅ | CreepJS, Akamai |
| `PluginArray.prototype` / `Plugin.prototype` / `MimeTypeArray.prototype` | `window_bootstrap.js:5333-5335` | item/namedItem/refresh | ✅ (`window_bootstrap.js:5333-5335`) | CreepJS |
| **WebAuthn / FedCM** | | | | |
| `PublicKeyCredential` (static methods) | `window_bootstrap.js:574-611` | 3 | ✅ (`window_bootstrap.js:584,591,610`) | (none) |
| `IdentityProvider` | `window_bootstrap.js:620-627` | 1 | ✅ (`window_bootstrap.js:626`) | (none) |
| `AuthenticatorResponse / Attestation / Assertion` | `window_bootstrap.js:561-572` | 0 | N/A | (none) |
| **DOM core** | | | | |
| `EventTarget.prototype` (`addEventListener` etc.) | `event_bootstrap.js:276-461` | 3 | 🟡 — masked at top-level only; prototype methods unmasked | Kasada `sdt`, Castle |
| `Node.prototype` (~40 methods/getters) | `dom_bootstrap.js:426-575` | ~40 | ✅ via sweep at `dom_bootstrap.js:3033` | Kasada `sdt` |
| `Element.prototype` (~80 methods/getters) | `dom_bootstrap.js:642-1024` | ~80 | ✅ via sweep at `dom_bootstrap.js:3033` | Kasada `sdt` |
| `Element.prototype.attachShadow` | `dom_bootstrap.js:1003-1022` | 1 | ✅ via sweep | **Kasada `sdt.c`** (the bug that triggered the `_wrap` correction in 2026-05-10) |
| `Document.prototype` (~50 methods) | `dom_bootstrap.js:1305-1566` | ~50 | ✅ via sweep at `dom_bootstrap.js:3034` | Kasada `bot1225` |
| `Document.prototype.write/writeln` | `dom_bootstrap.js:1433-1447` | 2 | ✅ (`window_bootstrap.js:5345`) | (none) |
| `Document.prototype.createElement/createTextNode/createComment` | `dom_bootstrap.js:1367-1407` | 8 | ✅ (`window_bootstrap.js:5364`) | Castle |
| `HTMLAllCollection.prototype` (`item`, `namedItem`) | `dom_bootstrap.js:1282-1303` | 2 | ✅ (`window_bootstrap.js:397`) | Castle |
| `HTMLFormElement.prototype.submit/requestSubmit` | `dom_bootstrap.js:1068-1110` | 2 | ✅ via sweep at `dom_bootstrap.js:3061` | reddit verify-page |
| `HTMLCanvasElement.prototype.toDataURL/getContext` | `canvas_bootstrap.js:887-981` + `dom_bootstrap.js:1171` | ~5 | ✅ via sweep at `dom_bootstrap.js:3056` | CreepJS, Kasada |
| `HTMLMediaElement.canPlayType` | `window_bootstrap.js:5176-5189` | 1 | ✅ (`window_bootstrap.js:5189`) | Castle (codec list) |
| `HTMLVideoElement.prototype` (decoded-frame APIs) | `window_bootstrap.js:5835` | ~6 | ✅ (`window_bootstrap.js:5835`) | (none) |
| `NodeList` / `DOMTokenList` / `HTMLCollection` / `NamedNodeMap` | `dom_bootstrap.js:300-422` + sweep | varies | ✅ via sweep at `dom_bootstrap.js:3036` | (none) |
| `Range / Selection` | `dom_bootstrap.js:1610-1683` | ~30 | 🟡 — defined as classes; `Range` prototype methods not in sweep list | (none) |
| `MutationObserver` | `dom_bootstrap.js:1827` | ~3 | ❌ not in sweep | Akamai sensor |
| `IntersectionObserver` | `window_bootstrap.js:3327-3353` | 4 | ❌ not in sweep | booking SPA gate |
| `ResizeObserver` | `window_bootstrap.js:3356-3376` | 3 | ❌ not in sweep | (none) |
| `DOMParser / XMLSerializer` | `dom_bootstrap.js:1796-1813` | 1 each | ❌ not in sweep | (none) |
| `TreeWalker / NodeIterator` | `dom_bootstrap.js:1415-1420` | inline stubs | ❌ stub object (no class prototype) | (none) |
| **Style / CSSOM** | | | | |
| `CSSStyleSheet / CSSStyleRule` | `dom_bootstrap.js:1567-1608` | ~10 | ✅ via sweep at `dom_bootstrap.js:3037` (`CSSStyleDeclaration` only) | (none) |
| `CSSStyleDeclaration` style proxy | `dom_bootstrap.js:583-635` | property-proxy | ✅ via sweep | Castle |
| **Events** | | | | |
| `Event / CustomEvent / MouseEvent / KeyboardEvent / PointerEvent / TouchEvent / WheelEvent / InputEvent / FocusEvent / MessageEvent / ErrorEvent / ProgressEvent / AnimationEvent / TransitionEvent / ClipboardEvent / PopStateEvent / HashChangeEvent / StorageEvent / PageTransitionEvent / BeforeUnloadEvent / DragEvent / SecurityPolicyViolationEvent` | `event_bootstrap.js:9-471` | 0 (data classes) | ❌ NOT in sweep — these emit JS class source on `String(Event)` | Kasada `sdt`, Akamai sensor |
| **Fetch / Networking** | | | | |
| `Headers.prototype` | `fetch_bootstrap.js:4-25` | ~9 | ❌ NOT in sweep | DataDome |
| `Request.prototype` | `fetch_bootstrap.js:107-140` | ~5 | ❌ NOT in sweep | duolingo Request.signal |
| `Response.prototype` | `fetch_bootstrap.js:27-105` | ~12 | ❌ NOT in sweep | (none) |
| `FormData.prototype` | `shared_apis_bootstrap.js:354-369` | ~9 | ✅ (`shared_apis_bootstrap.js:369`) | (none) |
| `URL.prototype / URLSearchParams.prototype` | `shared_apis_bootstrap.js:268-303` / `window_bootstrap.js:4126-4163` | ~10 each | ✅ (`shared_apis_bootstrap.js:303`) — but `_maskAsNative(URL)` not the proto | (none — but high-value) |
| `AbortController / AbortSignal` | `window_bootstrap.js:4066-4111` / `shared_apis_bootstrap.js:376-402` | ~5 | ✅ (`shared_apis_bootstrap.js:402`) | duolingo |
| `XMLHttpRequest.prototype` | `window_bootstrap.js:3465-3683` | ~13 | ❌ NOT in sweep | Akamai, Kasada |
| `WebSocket.prototype` | `window_bootstrap.js:3686-3757` | ~5 | ❌ NOT in sweep | (none) |
| `EventSource.prototype` | `window_bootstrap.js:2274-2290` | 1 | ❌ NOT in sweep | (none) |
| **Workers / messaging** | | | | |
| `Worker.prototype` (`postMessage / terminate / addEventListener`) | `window_bootstrap.js:1879-2044` | ~6 | ❌ NOT in sweep — exposes our `_drainOnce` JS source | duolingo recaptcha |
| `SharedWorker` | `window_bootstrap.js:2045-2063` | 0 | ✅ via sweep (`dom_bootstrap.js:3040`) | (none) |
| `ServiceWorker.prototype` | `window_bootstrap.js:2065-2077` | 0 | ❌ NOT in sweep | DataDome |
| `MessagePort.prototype` (`postMessage / start / close`) | `window_bootstrap.js:2256-2263` | 3 | ✅ via sweep at `dom_bootstrap.js:3050` | **duolingo H1** (see 05 § 2.3) |
| `MessageChannel` | `window_bootstrap.js:2265-2271` | 0 | ✅ via sweep at `dom_bootstrap.js:3050` | duolingo |
| `BroadcastChannel.prototype` | `window_bootstrap.js:2248-2253` | 2 | ✅ via sweep at `dom_bootstrap.js:3050` | (none) |
| **Crypto** | | | | |
| `Crypto.prototype` (`getRandomValues / randomUUID / subtle`) | `window_bootstrap.js:2951-3020` | 2 + accessor | 🟡 — methods are masked-shorthand (`_defProtoMethod`) but `Crypto`/`SubtleCrypto` constructors NOT in sweep | DataDome, Kasada (digest probe) |
| `SubtleCrypto.prototype` (`digest / sign / verify / ...`) | `window_bootstrap.js:2955-2988` | 12 | 🟡 — same as above | DataDome |
| **Text I/O** | | | | |
| `TextEncoder.prototype` | `window_bootstrap.js:3042-3105` | 2 (`encode`, `encodeInto`) | 🟡 — defined inline as `class` so prototype methods leak source | Kasada `nppm`-adjacent |
| `TextDecoder.prototype` | `window_bootstrap.js:3121-3165` | 1 (`decode`) | 🟡 — same | Kasada |
| **Storage** | | | | |
| `Storage.prototype` (localStorage/sessionStorage facade) | `window_bootstrap.js:3262-3302` | 5 | ❌ NOT in sweep | (none) |
| `IDBFactory.prototype + IDBDatabase + IDBTransaction + IDBObjectStore + IDBRequest + IDBOpenDBRequest + IDBKeyRange + IDBCursor` | `shared_apis_bootstrap.js:440-543` / `window_bootstrap.js:4855-4925` | many | ✅ (`shared_apis_bootstrap.js:542`) | (none) |
| **Performance** | | | | |
| `Performance.prototype` (`now/mark/measure/getEntries/getEntriesByType/getEntriesByName`) | `window_bootstrap.js:2933` etc. | ~10 | ✅ (`timer_bootstrap.js:187-189` for `now`; rest unverified) | Akamai sensor |
| `PerformanceObserver` | `window_bootstrap.js:2183-2202` | 3 | ❌ NOT in sweep | Akamai sensor |
| `PerformanceEntry / Mark / Measure / Resource` | `window_bootstrap.js:2203-2209` | 0 (data) | ❌ NOT in sweep | Akamai sensor |
| **WebGL / Canvas2D / Audio** | | | | |
| `CanvasRenderingContext2D.prototype` | `canvas_bootstrap.js:118-263` | ~40 | ✅ via sweep at `canvas_bootstrap.js` (verify) | CreepJS, BotD |
| `WebGLRenderingContext.prototype` (`getParameter` etc.) | `canvas_bootstrap.js:266-585` | ~80 | 🟡 — `_walkProto` of WebGL not in sweep; methods leak source | AWS WAF, CreepJS, BotD |
| `OffscreenCanvas.prototype` | `window_bootstrap.js:4367-4384` / `shared_apis_bootstrap.js:408-418` | 3 | ✅ (`shared_apis_bootstrap.js:417`) | duolingo (image-rec) |
| `AudioContext.prototype` (+ all `AudioNode` subclasses) | `canvas_bootstrap.js:585-1014` | many | ✅ via sweep at `dom_bootstrap.js:3046-3048` | CreepJS, Akamai T1.3 |
| `MediaSource.prototype` | `window_bootstrap.js:5492-5509` | 1 (`isTypeSupported`) | ✅ (`window_bootstrap.js:5348`) | Castle |
| `MediaRecorder.prototype` | `window_bootstrap.js:5512-5529` | 4 | ❌ NOT in sweep | (none) |
| `MediaCapabilities.prototype` | `window_bootstrap.js:5735-5769` | 2 | ✅ (`window_bootstrap.js:5779`) | DataDome |
| **DOM geometry helpers** | | | | |
| `DOMRect / DOMRectReadOnly` | `dom_bootstrap.js:117-133` | 0 (props) | 🟡 — class names masked (`dom_bootstrap.js:136-139`); no prototype methods | (none) |
| `DOMPoint / DOMPointReadOnly` | `dom_bootstrap.js:103-115` | 1 | ✅ (`shared_apis_bootstrap.js:576`) | (none) |
| `DOMMatrix / DOMMatrixReadOnly / WebKitCSSMatrix` | `shared_apis_bootstrap.js:578-592` | ~10 | ✅ (`shared_apis_bootstrap.js:592`) | (none) |
| **Streams** | | | | |
| `ReadableStream / ReadableStreamDefaultReader / Default Controller` | `streams_bootstrap.js:31-394` | ~10 each | ❌ NOT in sweep | (none — but high-leverage) |
| `WritableStream / WritableStreamDefaultWriter` | `streams_bootstrap.js:396-528` | ~5 each | ❌ NOT in sweep | (none) |
| `TransformStream` | `streams_bootstrap.js:531-579` | 0 | ❌ NOT in sweep | (none) |
| `CompressionStream / DecompressionStream` | `window_bootstrap.js:2293-2308` | 0 | ❌ NOT in sweep | (none) |
| `Blob.prototype / File.prototype` | `shared_apis_bootstrap.js:315-348` | ~5 | ✅ (`shared_apis_bootstrap.js:341,347`) | (none) |
| `FileReader.prototype` | `shared_apis_bootstrap.js:548-559` / `window_bootstrap.js:2095-2118` | ~6 | ✅ (`shared_apis_bootstrap.js:558`) | (none) |
| **Crypto / WebRTC** | | | | |
| `RTCPeerConnection.prototype` (`createOffer`, `createAnswer`, …) | `window_bootstrap.js:4936-5017` | ~20 | ✅ (`window_bootstrap.js:5337-5341`) | CreepJS |
| `RTCSessionDescription / RTCIceCandidate / RTCDataChannel` | `window_bootstrap.js:4931-5019` | 0 | ❌ NOT in sweep | (none) |
| **Misc surfaces** | | | | |
| `trustedTypes` (`createPolicy`, `isHTML`, …) | `window_bootstrap.js:5914-5938` | 6 | ✅ (`window_bootstrap.js:5938`) | (none) |
| `scheduler` (`postTask`, `yield`) | `window_bootstrap.js:5948-5962` | 2 | ✅ (`window_bootstrap.js:5962`) | (none) |
| `speechSynthesis` (`getVoices`, `speak`, …) | `window_bootstrap.js:2312-2330` | 5 | ✅ (`window_bootstrap.js:5350`) | CreepJS |
| `Touch / TouchEvent / TouchList` | `window_bootstrap.js:5985-6031` | 0 | ✅ (`window_bootstrap.js:6009,6026,6031`) | (none) |
| `Notification.prototype` + `Notification.requestPermission` | `window_bootstrap.js:6196-6252` | 1 + static | ✅ (`window_bootstrap.js:6250-6251`) | Cloudflare |
| `IdleDetector.requestPermission` | `window_bootstrap.js:6279-6298` | 1 static | ✅ (`window_bootstrap.js:6296`) | (none) |
| `EyeDropper` | `window_bootstrap.js:6305-6314` | 1 | ❌ NOT in sweep | (none) |
| `VirtualKeyboard / DevicePosture / WindowControlsOverlay` | `window_bootstrap.js:6320-6390` | varies | ❌ NOT in sweep | (none) |
| `PaymentRequest.prototype` (`show / abort / canMakePayment / hasEnrolledInstrument`) | `window_bootstrap.js:6496-6557` | 4 | ✅ (`window_bootstrap.js:6551-6555`) | Stripe / WAF "real browser" probes |
| `PressureObserver` | `window_bootstrap.js:5239-5257` | 2 | ❌ NOT in sweep | (none) |
| `CookieStore.prototype` | `window_bootstrap.js:6107-6143` | ~5 | ❌ NOT in sweep | (none) |
| `CacheStorage.prototype` | `window_bootstrap.js:6066-6086` | ~5 | ❌ NOT in sweep | (none) |
| `DocumentPictureInPicture.prototype` | `window_bootstrap.js:5275-5290` | 1 | ❌ NOT in sweep | (none) |
| `UserActivation` (`hasBeenActive`, `isActive`) | `window_bootstrap.js:5304-5315` | 0 (props) | ❌ NOT in sweep | (none) |
| `VisualViewport.prototype` | `window_bootstrap.js:5570-5588` | ~8 | ❌ NOT in sweep | (none) |
| `External.prototype` (`AddSearchProvider / IsSearchProviderInstalled`) | `interfaces_bootstrap.js:158-164` | 2 | ✅ (`interfaces_bootstrap.js:162`) | Akamai |
| **History / Location** | | | | |
| `Location.prototype` (`assign/replace/reload/toString`) | `window_bootstrap.js:1317-1413` | 4 | ✅ (`window_bootstrap.js:1398`) | Castle |
| `History.prototype` | `window_bootstrap.js:3793` | ~5 (`pushState/replaceState/back/forward/go`) | ❌ NOT in sweep | (none) |
| **Console / Errors** | | | | |
| `console` (`log/warn/error/info/debug/trace/group/...`) | `console_bootstrap.js:1-100` | 19 | ✅ (`stealth_bootstrap.js:128-138`) | **Kasada `ofc` (single biggest ML weight)** |
| `Error / TypeError / RangeError / etc.` | V8 native | many | N/A — V8 builtins, intrinsically `[native code]` | Kasada `wse`, `bfe` |

### 2.3 Headline gaps from this table (rank-ordered)

1. **Event classes** — every `Event` subclass in `event_bootstrap.js` is unmasked.
   Real Chrome: `String(Event)` = `"function Event() { [native code] }"`. Ours
   emits the literal `class Event { ... }` source. Probably the second-biggest
   Function.toString leak in BO after the cleanup_bootstrap noise that's already
   handled. **Fix: add `'Event','CustomEvent','UIEvent','MouseEvent', …` to the
   sweep list at `dom_bootstrap.js:3032-3069`.**
2. **Fetch trio** (`Headers / Request / Response`) — unmasked prototype methods.
   DataDome reads these. **Fix: extend `_toMask` in `dom_bootstrap.js:3032`.**
3. **XMLHttpRequest.prototype** — `open/send/setRequestHeader/...` unmasked.
   Akamai sensor_data reads our `op_net_fetch_sync` source. **Fix: sweep entry.**
4. **WebGLRenderingContext.prototype** — `getParameter` etc. unmasked.
   AWS WAF and CreepJS probe these directly. **Fix: sweep entry + per-method
   `_maskFunction` in `canvas_bootstrap.js`.**
5. **MutationObserver / IntersectionObserver / ResizeObserver / PerformanceObserver**
   — unmasked. Akamai sensor probes IntersectionObserver. **Fix: sweep entry.**
6. **Worker.prototype** — `postMessage` unmasked, leaks the wire-serialize
   function source. **Fix: sweep entry + per-method `_maskFunction`.**
7. **Streams** (`ReadableStream / WritableStream / TransformStream`) — completely
   unmasked. Future surface for any hydration-fingerprint probe. **Fix: sweep
   entry.**
8. **History.prototype** — unmasked. Castle probes `pushState`. **Fix: sweep.**
9. **Storage.prototype** — unmasked. **Fix: sweep.**
10. **CookieStore.prototype / CacheStorage.prototype** — unmasked. **Fix: sweep.**

---

## 3. Vendor-probe matrix

For each vendor we have evidence of, what JS API signals it reads and where BO
is weak. Source: `02_GAP_ANALYSIS.md`, `08_KASADA_FRONTIER.md` (and the memory
notes referenced there), `06_AWS_WAF_SOLVER.md`, `07_DATADOME_PRIMITIVES.md`.

### 3.1 AWS WAF (`challenge.js`)

| Probe | What it reads | BO status |
|---|---|---|
| `navigator.webdriver` getter source | `_maskFunction(get webdriver, …)` | ✅ `window_bootstrap.js:992` |
| WebGL parameters | `gl.getParameter(VENDOR/RENDERER/MAX_*)` | ✅ values from profile, but `getParameter.toString()` LEAKS source — see § 2.3 #4 |
| `navigator.permissions.query` consistency | denied/granted/prompt for camera/mic | ✅ masked, profile-shaped |
| `performance.now()` granularity | sub-millisecond resolution check | 🟡 implementation uses `Date.now() - startTime` (`timer_bootstrap.js:170-173`) — millisecond granularity, **not Chrome-native sub-ms**; flips a heuristic |
| `Worker(URL)` real spawn vs polyfill | does the worker actually run code | ✅ `worker_ext.rs:214` — real OS thread + child JsRuntime |
| WebAssembly probe-of-execution | `WebAssembly.instantiate` + roundtrip | ✅ V8-native |
| `crypto.subtle.digest` SHA-256 | Used for token PoW | ✅ `window_bootstrap.js:2969-2979` (real Rust op) |

**BO's principal AWS WAF weakness:** WebGL `getParameter` leaking source +
`performance.now()` low granularity. See `06_AWS_WAF_SOLVER.md` for the full
amazon-de/in/com-au/imdb situation.

### 3.2 Kasada (`ips.js`)

| Field | Probe | BO status |
|---|---|---|
| `sfc` | `Function.prototype.toString.call(fetch)` etc. — must include `[native code]` | 🟡 partial — `dom_bootstrap.js:3079` sweep covers some, MANY gaps remain (see § 2.3) |
| `sdt` | Same as `sfc` but for DOM API methods | 🟡 same |
| `bot1225` | `Cannot read properties of undefined (reading 'unjzomuybtbyyhwwkdpkxomylnab')` — a specific 28-char Web API our globalThis is missing | ❌ unknown which API; the open Lever 4 in `08_KASADA_FRONTIER.md` |
| `csc`, `kl`, `dpv`, `smc` | Same undefined-property root as bot1225 | ❌ same |
| `ofc` | `console.<method>.toString()` for all ~19 methods | ✅ `stealth_bootstrap.js:128-138` |
| `nppm` | `new structuredClone()` throws TypeError shape | 🟡 — `structured_clone.js:208` throws correct text but the `TypeError: X is not a constructor` shape for lowercase-IDL globals needs full audit (`interfaces_bootstrap.js:100-110`) |
| `fsc` | `class X extends Function.prototype.toString {}` must throw | ✅ via method-shorthand toString (`stealth_bootstrap.js:25-39`) |
| `npc` | `class X extends Y` shape for non-constructible Y | ✅ (the previous code-FP-fix arc) |
| `esd` | Error.stack must not leak our private helper names | 🟡 — recent fix scrubbed `_loadGpuProfile` but full sweep TBD |
| `wse`, `bfe` | `Function.prototype.toString` thrown-text format | ✅ |
| `ao` | "spread non-iterable" error text | ✅ V8 native |
| `cbf` | `Cannot read properties of undefined (reading 'toString')` — empty slot | ❌ unknown which slot |
| **CSS `calc()` math fns** | Inject `calc(sin(...)*cos(...))`, compare f64 result | ❌ MISSING in `crates/css_values/src/types/length.rs:43-57` per Kasada Lever 2 |

**BO's principal Kasada weakness:** all of Lever 1-4 in `08_KASADA_FRONTIER.md`.
The masking sweep proposed in § 5 below addresses Levers 3 directly (the `sfc`
and `sdt` fields) and is a prerequisite for any K2-DIFF reduction.

### 3.3 DataDome (`tags.js` / `js.captcha-delivery.com`)

| Probe | What it reads | BO status |
|---|---|---|
| `document.cookie` pattern | `datadome=...` cookie persistence | ✅ `crates/net/src/lib.rs:140-174` (`SharedSession`) |
| `navigator.userAgent` vs ClientHints | Cross-check `Sec-CH-UA` matches `navigator.userAgent` | ✅ `crates/stealth/profiles/*.yaml` + worker `navigator.userAgentData` (`worker_bootstrap.js:55-128`) |
| iframe `contentWindow` shape | Hidden iframe with `0x0` size — must have Window, document, frames | ✅ `dom_bootstrap.js:2347-2931` (iframe realm + `_buildRemoteRealm`) |
| `Worker(URL).onmessage` IPC | Spawns daily-key worker; reads response cookie | 🟡 Worker works; daily-key flow is `07_DATADOME_PRIMITIVES.md` open work |
| `WebAssembly` execution proof | Loads a daily-rotated WASM, expects specific output | ❌ NOT exercised end-to-end — etsy/tripadvisor/yelp fail here |
| `navigator.permissions.query({name:'notifications'})` | `state === 'denied'` on insecure → `prompt` on secure | ✅ `window_bootstrap.js:469-503` |
| `Request.signal` on `Request.prototype` | Capability gate | ✅ `fetch_bootstrap.js:120-138` |
| `IntersectionObserverEntry` interface existence | `'IntersectionObserverEntry' in globalThis` | ✅ `window_bootstrap.js:3315-3324` |

**BO's principal DataDome weakness:** WebAssembly daily-key flow + iframe
sub-realm not delivering the right cookie. See `07_DATADOME_PRIMITIVES.md`.

### 3.4 Cloudflare (`/cdn-cgi/challenge-platform/`)

| Probe | What it reads | BO status |
|---|---|---|
| `window.chrome` object | `window.chrome.runtime.PlatformOs` etc. | ✅ `window_bootstrap.js:1625-1697` |
| `navigator.plugins` count | Must be ≥1 with PDF viewer plugins | ✅ profile-supplied |
| WebGL extensions | `getSupportedExtensions()` shape | ✅ `canvas_bootstrap.js` |
| `Notification.permission` default | Must be `'default'` on https, `'denied'` on insecure | ✅ `window_bootstrap.js:6196+` |
| Sub-second JS challenge solving | Iterating loops, `Math.floor` of timing | ✅ V8-native |

(Cloudflare is largely solved; see chapter 02 — Cloudflare-protected sites pass
in the 126-corpus.)

### 3.5 Akamai BMP v3 (`/_bm/_data` sensor)

| Probe | What it reads | BO status |
|---|---|---|
| `Object.prototype.toString.call(navigator)` | Must be `"[object Navigator]"` | ✅ `window_bootstrap.js:1242` Symbol.toStringTag |
| `performance.getEntriesByType('resource')` | Resource list shape | ✅ (`window_bootstrap.js:2599-2885`) |
| Mouse trajectory, key timings | Real interaction trace | N/A — out of public-engine scope; see `humanize.js` |
| `External.AddSearchProvider` | Must be a callable function with right shape | ✅ `interfaces_bootstrap.js:158-164` |
| Date.prototype.getTimezoneOffset | Profile-consistent | ✅ `window_bootstrap.js:2401-2429` |
| `XMLHttpRequest.prototype.open.toString()` | Must include `[native code]` | ❌ unmasked — see § 2.3 #3 |

**BO's principal Akamai weakness:** behavior absence is the dominant scorer
once the static surface is clean; the XHR.toString leak is a contributing
secondary fix.

### 3.6 Castle.io

| Probe | What it reads | BO status |
|---|---|---|
| `Error.stack` literal format | Must match V8's `at FName (file:line:col)` exactly | ✅ snapshot script name is `<anonymous>` (`snapshot.rs:97`) |
| `document.write` after DCL | Must succeed for legacy compatibility | ✅ `dom_bootstrap.js:1433-1444` |
| `Document.prototype.createElement('script').src` setter | Must trigger fetch (or no-op deterministically) | ✅ `dom_bootstrap.js:1375-1389` |

---

## 4. Per-bootstrap-file content map

Read this in tandem with `13_FILE_LOCATIONS_INDEX.md § JS bootstrap scripts`.
Concatenation order (`snapshot.rs:67-91`):

```
console_bootstrap → stealth → interfaces → instances → fetch → timer →
dom → event → canvas → window → streams → structured_clone
```

Cleanup runs LATER (per-page, not in snapshot) at `page.rs` build_page time.

| File | Lines | Purpose |
|---|--:|---|
| `console_bootstrap.js` | 100 | Defines `console.{log,warn,error,...}` 19 methods routed through `op_console_log`. Must be FIRST so masking can refer to it later. |
| `stealth_bootstrap.js` | 140 | Function.prototype.toString patch + `_maskAsNative`/`_maskFunction`/`_nativeTag` helpers + initial console-method masking. Documented in § 1. |
| `interfaces_bootstrap.js` | 202 | Stub-defines ~600 Web IDL interface names so `typeof X === 'function'` is true even without a real impl. Skip-list at L66-92 reserves names that get real impls later. Also defines 120 `on*` event-handler property slots + 22 misc Window properties (`external`, `personalbar`, etc.). |
| `instances_bootstrap.js` | 18 | Tiny — exposes `_browser_oxide` private global slot used by other bootstraps. |
| `fetch_bootstrap.js` | 395 | `Headers`, `Request`, `Response`, `globalThis.fetch` (real, backed by `op_fetch`). Blob-URL handling. CSP probing. |
| `timer_bootstrap.js` | 191 | `setTimeout/setInterval/clearTimeout/clearInterval` + `__bgSetTimeout` (humanize unref'd) + `__cancelAllTimers` (warm reset). `requestAnimationFrame` at 16 ms cadence. `performance.now` fallback. Masks every entry. |
| `dom_bootstrap.js` | 3112 | The largest file. `DOMPoint/Rect/Matrix`, `NodeList`, `DOMTokenList`, `EventTarget`, `Node`, `Element`, `HTMLElement` and ~25 HTML*Element subclasses, `HTMLFormElement.submit/requestSubmit` (the reddit gate), `HTMLAllCollection`, `Document` (`getElementById`, `querySelector`, `createElement`, `attachShadow`, `createRange`, …), `Range`, `Selection`, `MutationObserver`, iframe realm + remote realm builder, attribute interception (`setAttribute`, `removeAttribute`, `remove`). Ends with the masking sweep at L2995-3094 (the `_toMask` list). |
| `event_bootstrap.js` | 516 | All `Event` subclasses (`MouseEvent`, `KeyboardEvent`, `PointerEvent`, `TouchEvent`, `WheelEvent`, `InputEvent`, …, `SecurityPolicyViolationEvent`). The listener dispatch core (`_addEventListener`, `_removeEventListener`, `_dispatchEvent`, `_fireListeners`). All `on*` handler routing. |
| `canvas_bootstrap.js` | 1301 | `ImageData`, `CanvasRenderingContext2D` (40+ methods), `WebGLRenderingContext` (80+ methods + constants), all `AudioNode` types (`Oscillator/Gain/DynamicsCompressor/BiquadFilter/Analyser/AudioDestination`), `BaseAudioContext`/`AudioContext`/`OfflineAudioContext`, real `OffscreenCanvas`, `HTMLCanvasElement` class. |
| `window_bootstrap.js` | 6704 | The other big one. The `Window` named constructor, `Navigator` + 30 props, `Permissions`/`PermissionStatus`, WebAuthn (`PublicKeyCredential`, `IdentityProvider`), `CredentialsContainer`, `Bluetooth/USB/HID/Serial`, `KeyboardLayoutMap`/`Keyboard`, `StorageManager`, `ServiceWorkerContainer`, `BatteryManager`, navigator-shape masks, the iframe realm + sandbox + remote-realm machinery, `Crypto`/`SubtleCrypto`, `TextEncoder`/`TextDecoder`, `localStorage`/`sessionStorage`, `IntersectionObserver`/`ResizeObserver`, `requestIdleCallback`, `getComputedStyle`, `XMLHttpRequest`, `WebSocket`, `History`, `URL`/`URLSearchParams`, `FormData`, `AbortController`, `customElements`, `Blob/File/OffscreenCanvas`, IndexedDB, `RTCPeerConnection`, `FontFace`, `PressureObserver`, `MediaSource`/`MediaRecorder`, `VisualViewport`, `MediaCapabilities`, `trustedTypes`, `scheduler`, `Touch/TouchEvent/TouchList`, `CacheStorage`, `CookieStore`, `Notification`, `IdleDetector`, `EyeDropper`, `VirtualKeyboard`, `DevicePosture`, `WindowControlsOverlay`, `ViewTransition`, `PaymentRequest`, `clientInformation`, `name/status/defaultStatus`. |
| `streams_bootstrap.js` | 594 | `ReadableStream` + Default Reader + Default Controller, `WritableStream` + Default Writer + Default Controller, `TransformStream`. Real backpressure semantics so duolingo / booking SPA hydration that consumes a stream actually advances. |
| `structured_clone.js` | 370 | Polyfill (or guard around V8 native) for `structuredClone`. Handles ArrayBuffer/TypedArray/Map/Set/Date/RegExp/Error round-trip. |
| `worker_bootstrap.js` | 291 | Worker-isolate bootstrap. Sets `self`, `postMessage`, `onmessage` dispatch, `close`, `importScripts`. Worker-side `navigator` (`WorkerNavigatorUAData`), Intl sync to profile timezone. Drains parent→worker via `op_worker_self_recv`. |
| `cleanup_bootstrap.js` | 582 | Runs LAST per-page (not in snapshot). Strips deno_core globals (`Deno`, `__bootstrap`, `Symbol.dispose` from non-Chrome locations), hides the `__browser_oxide` slot, gates SecureContext-only APIs on insecure pages, asserts shape invariants. |
| `sse_bootstrap.js` | 122 | Server-Sent Events (`EventSource` real impl, replacing the stub at `window_bootstrap.js:2274`). Installed on demand. |
| `input_bootstrap.js` | 18 | Tiny — exposes input-related ops for `humanize.js`. |

---

## 5. Concrete `_maskAsNative` sweep plan

### 5.1 The problem one more time

From `08_KASADA_FRONTIER.md` Phase 3 finding 2: Kasada records the `sfc` and
`sdt` error fields whenever
`Function.prototype.toString.call(SomeWebAPI)` returns anything that doesn't
match `/^function \w+\(\) \{ \[native code\] \}$/`. The current dom_bootstrap
sweep (L2995-3094) is a hand-curated list, demonstrably incomplete by the table
in § 2.

### 5.2 The audit script (proposed)

Live as a test in `crates/browser/tests/chrome_compat.rs` (a new top-level fn):

```rust
#[tokio::test(flavor = "current_thread")]
#[ignore] // discovery-only; flips to required once the baseline is locked
async fn native_code_mask_audit() {
    let profile = stealth::presets::chrome_148_macos();
    let client = net::HttpClient::new().unwrap();
    let page = Page::build_page_with_scripts(
        "<!DOCTYPE html><html><body></body></html>",
        "https://example.com/",
        &profile,
        &client,
    ).await.unwrap();

    // Enumerate every constructor + prototype method in globalThis
    // and assert that String(fn) matches the [native code] shape.
    // Dump the failures (NOT a panic) so the audit produces a sortable
    // list rather than aborting on the first miss.
    let script = r#"
        (function() {
            const failures = [];
            const expected = /^function \w+(\(.*\))? \{ \[native code\] \}$/;
            const seen = new WeakSet();

            function check(name, fn) {
                if (typeof fn !== 'function') return;
                if (seen.has(fn)) return;
                seen.add(fn);
                const s = String(fn);
                if (!expected.test(s)) {
                    failures.push({ name, sample: s.slice(0, 120) });
                }
            }

            function walk(ctorName, ctor) {
                check(ctorName, ctor);
                if (!ctor || !ctor.prototype) return;
                for (const key of Object.getOwnPropertyNames(ctor.prototype)) {
                    if (key === 'constructor') continue;
                    const desc = Object.getOwnPropertyDescriptor(ctor.prototype, key);
                    if (!desc) continue;
                    if (desc.value) check(`${ctorName}.prototype.${key}`, desc.value);
                    if (desc.get) check(`get ${ctorName}.prototype.${key}`, desc.get);
                    if (desc.set) check(`set ${ctorName}.prototype.${key}`, desc.set);
                }
            }

            for (const k of Object.getOwnPropertyNames(globalThis)) {
                try {
                    const v = globalThis[k];
                    if (typeof v === 'function') walk(k, v);
                } catch (_) {}
            }
            // Sort + return JSON so the Rust side can compare against a golden.
            failures.sort((a, b) => a.name.localeCompare(b.name));
            return JSON.stringify(failures, null, 2);
        })();
    "#;

    let out: String = page.event_loop().execute_script(script).unwrap_or_default();
    std::fs::write("/tmp/bo_mask_audit.json", &out).unwrap();
    // First run: just write the file. Once stabilized, snapshot-test it.
    eprintln!("[audit] wrote {} bytes", out.len());
}
```

Run with:

```bash
cd /home/yfedoseev/projects/browser_oxide
cargo test --release -p browser --test chrome_compat \
    native_code_mask_audit -- --ignored --test-threads=1 --nocapture
cat /tmp/bo_mask_audit.json | jq '. | length'
cat /tmp/bo_mask_audit.json | jq -r '.[].name' | sort -u
```

### 5.3 The fix loop

For each entry in `bo_mask_audit.json`:

1. Find the file:line where the function is defined (use the table in § 2 or
   `grep -n 'globalThis.X = ' crates/js_runtime/src/js/`).
2. Decide masking strategy:
   - **Single function (top-level global):** add `_maskFunction(globalThis.X, 'X');`
     near the definition, OR add `'X'` to the `_topLevelFns` array at
     `dom_bootstrap.js:3079-3088`.
   - **Constructor + prototype methods:** add the class name to the `_toMask`
     array at `dom_bootstrap.js:3032-3069`. The `_walkProto` helper will mask
     every prototype member.
   - **Getter/setter:** use `_maskAsNative(SomeProto, 'propName')` (which
     handles accessor descriptors correctly).
3. Re-run the audit. The entry should disappear from `bo_mask_audit.json`.
4. Commit one logical group (e.g., "Mask all Event subclass constructors";
   "Mask Headers/Request/Response prototypes"; "Mask Worker.prototype.postMessage").

### 5.4 Prioritization

Address the entries in this order (matches the vendor-probe matrix in § 3):

1. **Event class constructors** (~25 entries) → cleans `sdt`
2. **Fetch trio prototypes** (3 entries × ~10 methods each) → cleans DataDome
3. **XMLHttpRequest.prototype** (~13 entries) → cleans Akamai
4. **WebGLRenderingContext.prototype** (~80 entries) → cleans AWS WAF
5. **Worker.prototype + ServiceWorker.prototype** → cleans duolingo path
6. **Observer constructors** (Mutation/Intersection/Resize/Performance/Pressure) → cleans Akamai sensor
7. **Streams constructors** (`ReadableStream` etc.) → future-proof
8. **History.prototype / Storage.prototype** → cleans Castle
9. **All remaining ❌ entries from § 2** → cleans residual ML weight

### 5.5 Idempotency

`_maskFunction` is idempotent (`stealth_bootstrap.js:57-65`): re-tagging an
already-tagged function is a no-op. The dom_bootstrap sweep can therefore be
called BEFORE adding direct per-function `_maskFunction` calls, in either order,
without conflict.

---

## 6. Cross-realm caveats (iframe / Worker)

The `_nativeTag` is `Symbol.for(...)` (global registry — `stealth_bootstrap.js:13`)
so the tag survives realm boundaries. But the patched
`Function.prototype.toString` is installed PER realm:

- **Main realm:** installed at snapshot time.
- **Worker realm:** installed at worker bootstrap time (`runtime.rs:341-343` runs
  `stealth_bootstrap` first in workers too — verify still holds).
- **Iframe contentWindow:** the iframe realm builder at `dom_bootstrap.js:2287`
  must install the patched toString on the iframe's `Function.prototype`. As of
  HEAD this is wired (the mirrored-constructor + remote-realm code).

If you add a NEW realm path (e.g., a Worklet, AudioWorklet, ServiceWorker), the
patched toString must be re-installed there. The cross-realm `Symbol.for` tag
ensures parent-realm functions still report `[native code]` when their
toString is called from the child realm — only the toString patch itself is
realm-local.

---

## 7. Acceptance criteria for v0.1.0

- [ ] **Audit script `native_code_mask_audit` exists** in `crates/browser/tests/chrome_compat.rs`
- [ ] **`/tmp/bo_mask_audit.json` is committed as a golden** at `crates/browser/tests/fixtures/mask_audit_golden.json` (or wherever the fixtures live), updated on every intentional change
- [ ] **The audit golden is empty** (or contains only documented exceptions) on a release-build run with HEAD
- [ ] **Top-20 most-probed methods masked** (Event ctors, Headers/Request/Response, XHR, WebGL, Worker.postMessage, MutationObserver/IntersectionObserver, History.pushState, Storage.getItem). Each commit message references the audit-script delta.
- [ ] **Kasada `sfc`/`sdt` error-field count drops** in the blob-capture test at `crates/browser/tests/chrome_compat.rs::kasada_error_blob_capture` — current count (per `08_KASADA_FRONTIER.md` Phase 3) is **5+ blobs with `sfc`/`sdt`**; target: **0 blobs** in the captured set.
- [ ] **AWS WAF `/report` POST volume decreases** (= less detected as bot). Measure: `grep -c 'awswaf.com/.../report' /tmp/full_sweep_2026_05_24/bo_chrome_148_macos_cold.log` BEFORE vs AFTER. Expect at least amazon-de or imdb to flip from `2011 bytes` to `>15 KB` after sweep + relevant per-vendor fixes.
- [ ] **No regression** on the 437+ existing chrome_compat tests (`cargo test --workspace -- --test-threads=1`).

---

## 8. Files referenced

| File:line | Purpose |
|---|---|
| `crates/js_runtime/src/js/stealth_bootstrap.js:13` | `_nativeTag = Symbol.for('__browser_oxide_native__')` |
| `crates/js_runtime/src/js/stealth_bootstrap.js:14-48` | `Function.prototype.toString` patch (method-shorthand toString) |
| `crates/js_runtime/src/js/stealth_bootstrap.js:51-80` | `_maskFunction(fn, name)` helper |
| `crates/js_runtime/src/js/stealth_bootstrap.js:82-104` | `_maskAsNative(obj, ...names)` helper |
| `crates/js_runtime/src/js/stealth_bootstrap.js:107-109` | Helpers exposed on globalThis |
| `crates/js_runtime/src/js/stealth_bootstrap.js:128-138` | Console-method mask (Kasada `ofc`) |
| `crates/js_runtime/src/snapshot.rs:67-91` | Bootstrap concatenation order |
| `crates/js_runtime/src/snapshot.rs:97-99` | `<anonymous>` script-name pseudonym (Castle.io) |
| `crates/js_runtime/src/runtime.rs:341-343` | Worker isolate runs `stealth_bootstrap` first |
| `crates/js_runtime/src/js/dom_bootstrap.js:2995-3094` | The current curated mask-sweep `_toMask` list |
| `crates/js_runtime/src/js/dom_bootstrap.js:3079-3094` | Top-level-fn mask list (queueMicrotask, fetch, setTimeout, etc.) |
| `crates/js_runtime/src/js/dom_bootstrap.js:3032-3069` | Prototype-walk class-name list |
| `crates/js_runtime/src/js/console_bootstrap.js` | console methods backing the `ofc` probe |
| `crates/js_runtime/src/js/interfaces_bootstrap.js:42-53` | `_stub("Name")` illegal-constructor pattern |
| `crates/js_runtime/src/js/interfaces_bootstrap.js:66-92` | Skip-list (interfaces with real impls later) |
| `crates/js_runtime/src/js/interfaces_bootstrap.js:96-110` | Lowercase-IDL globals NOT stubbed (per nppm fix) |
| `crates/js_runtime/src/js/interfaces_bootstrap.js:158-164` | `External.prototype` (Akamai) |
| `crates/js_runtime/src/js/timer_bootstrap.js:180-189` | timer + performance.now masks |
| `crates/js_runtime/src/js/event_bootstrap.js:9-471` | All Event subclasses (currently ❌ unmasked — § 2.3 #1) |
| `crates/js_runtime/src/js/fetch_bootstrap.js:4-25` | Headers (❌ unmasked) |
| `crates/js_runtime/src/js/fetch_bootstrap.js:27-105` | Response (❌ unmasked) |
| `crates/js_runtime/src/js/fetch_bootstrap.js:107-140` | Request (❌ unmasked) |
| `crates/js_runtime/src/js/canvas_bootstrap.js:266-585` | WebGLRenderingContext (🟡 partial) |
| `crates/js_runtime/src/js/window_bootstrap.js:992` | `get webdriver` mask |
| `crates/js_runtime/src/js/window_bootstrap.js:1029` | navigator props mass mask |
| `crates/js_runtime/src/js/window_bootstrap.js:1879-2044` | Worker class (❌ postMessage unmasked) |
| `crates/js_runtime/src/js/window_bootstrap.js:2256-2271` | MessagePort / MessageChannel (✅ in sweep) |
| `crates/js_runtime/src/js/window_bootstrap.js:2933-3020` | Crypto / SubtleCrypto (🟡) |
| `crates/js_runtime/src/js/window_bootstrap.js:3315-3376` | IntersectionObserver / ResizeObserver (❌) |
| `crates/js_runtime/src/js/window_bootstrap.js:3465-3683` | XMLHttpRequest (❌) |
| `crates/js_runtime/src/js/window_bootstrap.js:3686-3757` | WebSocket (❌) |
| `crates/js_runtime/src/js/window_bootstrap.js:5331-5364` | Per-instance _maskAsNative cluster |
| `crates/js_runtime/src/js/window_bootstrap.js:5779` | MediaCapabilities mask |
| `crates/js_runtime/src/js/window_bootstrap.js:6196-6252` | Notification + requestPermission |
| `crates/browser/tests/chrome_compat.rs` | Where to add `native_code_mask_audit` |
| `crates/browser/tests/chrome_compat.rs::kasada_error_blob_capture` | The end-to-end Kasada `/tl` blob diagnostic |
| `08_KASADA_FRONTIER.md` § Phase 3 finding 2, Lever 3 | The motivation for this whole audit |
| `02_GAP_ANALYSIS.md` § 1-10 | Per-site evidence of which vendor probes which surface |
| `13_FILE_LOCATIONS_INDEX.md` § JS bootstrap scripts | Quick lookup |

---

## 9. Sequencing within v0.1.0

This audit can run in parallel with chapters 05/06/07 (each per-vendor work
stream independently consumes the masking improvements). Recommended order:

1. **Build the audit script** (§ 5.2). 1 day. Output: the current failure list.
2. **Lock the golden** at the current state so future regressions break CI.
3. **Sweep Event constructors + Fetch trio + XHR** (§ 5.4 top 3). 1 day.
4. **Re-run Kasada `kasada_error_blob_capture`** to confirm `sfc`/`sdt` field count drops.
5. **Sweep WebGL + Worker + Observers** (§ 5.4 #4-6). 1 day.
6. **Sweep the long tail** (§ 5.4 #7-9). 1 day.
7. **A/B sweep against the 126-corpus** to confirm no regression and capture
   the per-site impact (expect amazon-de or imdb to flip).

Total budget: **3-5 days of focused work**, runnable by one contributor while
chapters 05/06/07 progress in parallel.

