# 17 — Web API parity matrix

**Owner scope:** structural / API surface (cross-cutting).
**Status:** prerequisite for any "site is missing API X" investigation.
**Why this chapter exists:** prevent the silent "missing API" surprises like
the `HTMLFormElement.elements` (reddit) and `MessageChannel`/`MessagePort`
(duolingo) discoveries from chapter 05. The matrix is the **inverse** of doc 16:
doc 16 asks "is what we expose masked correctly?"; this doc asks "do we expose
the right thing in the first place?".

Read this with:
- `16_STEALTH_FINGERPRINT_AUDIT.md` (companion — fingerprint masking of what we DO expose)
- `05_SPA_HYDRATION_CLUSTER.md` § 1.2 (H2 — `HTMLFormElement.elements` reddit blocker)
- `05_SPA_HYDRATION_CLUSTER.md` § 2.3 (H1 — `MessageChannel` duolingo blocker)
- `02_GAP_ANALYSIS.md` (per-site evidence of which API a SPA waits for)
- `13_FILE_LOCATIONS_INDEX.md` (every file:line below is in the index)

---

## 1. Methodology

### 1.1 Spec areas surveyed

The W3C/WHATWG surface is huge (~3000 named IDL interfaces). For v0.1.0 we cover
the ~120 interfaces that matter for SPA hydration + anti-bot fingerprinting.
Areas (alphabetical):

1. **Clipboard** (Clipboard, ClipboardItem, ClipboardEvent)
2. **CSSOM** (CSSStyleSheet, CSSStyleRule, CSSStyleDeclaration, MediaList, getComputedStyle)
3. **Crypto** (Crypto, SubtleCrypto)
4. **DOM** (Element, Node, Document, DocumentFragment, ShadowRoot, MutationObserver, IntersectionObserver, ResizeObserver, PerformanceObserver, Range, Selection, TreeWalker, NodeIterator, XPathEvaluator, XPathResult)
5. **EventSource** (SSE)
6. **Events** (Event, MouseEvent, KeyboardEvent, PointerEvent, TouchEvent, WheelEvent, InputEvent, FocusEvent, MessageEvent, ErrorEvent, ProgressEvent, AnimationEvent, TransitionEvent, ClipboardEvent, PopStateEvent, HashChangeEvent, StorageEvent, PageTransitionEvent, BeforeUnloadEvent, DragEvent, SubmitEvent, SecurityPolicyViolationEvent)
7. **Fetch** (fetch, Request, Response, Headers, AbortController, AbortSignal, FormData, URL, URLSearchParams, URLPattern)
8. **File** (Blob, File, FileReader, FileList, FileSystemHandle)
9. **Geolocation** (Geolocation, GeolocationCoordinates, GeolocationPosition)
10. **History** (History, Location, PopStateEvent)
11. **HTML** (HTMLFormElement + ~25 HTML*Element subclasses, HTMLCanvasElement, HTMLImageElement, HTMLVideoElement, HTMLMediaElement, HTMLScriptElement, HTMLLinkElement, HTMLAnchorElement, HTMLInputElement, HTMLSelectElement, HTMLTextAreaElement, HTMLFormControlsCollection, HTMLAllCollection, HTMLCollection)
12. **IndexedDB** (IDBFactory, IDBDatabase, IDBTransaction, IDBObjectStore, IDBRequest, IDBOpenDBRequest, IDBKeyRange, IDBCursor)
13. **Media** (HTMLMediaElement.canPlayType, MediaSource, MediaRecorder, MediaCapabilities, MediaDevices, MediaSession, MediaMetadata, MediaStream)
14. **Navigation** (Navigation API: NavigateEvent, NavigationHistoryEntry — Chrome 102+, anti-bot probe surface)
15. **Notification** (Notification, requestPermission)
16. **Performance** (performance.now, mark, measure, getEntries, getEntriesByType, PerformanceObserver, PerformanceNavigationTiming, PerformanceResourceTiming, performance.memory)
17. **Permissions** (Permissions, PermissionStatus)
18. **PaymentRequest** (PaymentRequest, PaymentResponse, PaymentMethodChangeEvent)
19. **PushAPI / Background** (PushManager, ServiceWorkerRegistration.pushManager) — stubbed
20. **Scheduling** (requestAnimationFrame, requestIdleCallback, scheduler.postTask, queueMicrotask, structuredClone)
21. **Service Workers** (ServiceWorker, ServiceWorkerContainer, ServiceWorkerRegistration)
22. **Storage** (localStorage, sessionStorage, Storage, StorageManager, CookieStore, CacheStorage)
23. **Streams** (ReadableStream, WritableStream, TransformStream, CompressionStream, DecompressionStream)
24. **Text** (TextEncoder, TextDecoder, TextEncoderStream, TextDecoderStream)
25. **Timers** (setTimeout, setInterval, clearTimeout, clearInterval, requestAnimationFrame)
26. **Touch / Pointer / Drag** (Touch, TouchEvent, TouchList, PointerEvent, DragEvent)
27. **TrustedTypes** (trustedTypes, TrustedHTML, TrustedScript, TrustedScriptURL)
28. **UserActivation / Activation** (UserActivation, navigator.userActivation)
29. **WebAssembly** (WebAssembly.{compile,instantiate,Module,Instance,Memory,Table,instantiateStreaming,compileStreaming}) — V8 native
30. **WebAuthn / FedCM** (PublicKeyCredential, CredentialsContainer, IdentityProvider)
31. **WebGL / WebGPU** (WebGLRenderingContext, WebGL2RenderingContext, GPU, GPUAdapter, GPUDevice)
32. **WebRTC** (RTCPeerConnection, RTCSessionDescription, RTCIceCandidate, RTCDataChannel)
33. **WebSocket / Transport** (WebSocket, WebTransport)
34. **Workers** (Worker, SharedWorker, MessageChannel, MessagePort, BroadcastChannel, importScripts, OffscreenCanvas)
35. **Other navigator surfaces** (navigator.usb, hid, serial, bluetooth, geolocation, locks, mediaDevices, clipboard, storage, serviceWorker, credentials, connection (NetworkInformation), userAgentData, keyboard, plugins, mimeTypes, virtualKeyboard)
36. **Misc** (Notification, EyeDropper, IdleDetector, PressureObserver, VirtualKeyboard, DevicePosture, WindowControlsOverlay, ViewTransition, DocumentPictureInPicture)

### 1.2 How to verify any single interface

```bash
cd /home/yfedoseev/projects/browser_oxide
# 1. Does anything install it?
grep -rn "globalThis\.HTMLFormElement\|globalThis\.MessageChannel" \
    crates/js_runtime/src/js/ | head -5

# 2. Where is the prototype defined?
grep -n "class HTMLFormElement\|class MessageChannel" \
    crates/js_runtime/src/js/*.js

# 3. Which methods does it expose?
grep -nA3 "class HTMLFormElement" \
    crates/js_runtime/src/js/dom_bootstrap.js | head -30

# 4. Run a 1-liner inside BO to confirm:
cat > /tmp/probe.json <<'JSON'
[{"cat":"probe","name":"probe","url":"data:text/html,<script>
  console.log('elements:', typeof HTMLFormElement.prototype.elements);
  console.log('namedItem:', typeof HTMLFormControlsCollection);
</script>"}]
JSON
RUST_LOG=js_runtime=info target/release/examples/sweep_metrics \
    chrome_148_macos /tmp/probe.json /tmp/probe_out.json
```

### 1.3 Classification

Every entry below is one of:

- **✅ IMPLEMENTED** — real Rust op or correct JS semantics. References to it
  work as Chrome does (within the limits documented in the per-row notes).
- **🟡 STUBBED** — defined as a class/function, returns plausible values, but
  has no real backing behavior. Most things "work" as long as you don't observe
  side effects. ~80% of API surface in BO sits here — adequate for SPA
  hydration, **inadequate** for sites whose flow depends on real semantics
  (recaptcha, paywalls, payments).
- **❌ MISSING** — referencing throws `ReferenceError` (the symbol is absent) or
  `TypeError: X is not a constructor` (the symbol is something else). These
  are the silent SPA-killers.
- **❓ UNVERIFIED** — entry is documented as defined but the author didn't
  reproduce. Resolve before relying on it.

---

## 2. The parity table

### 2.1 HTML

| Interface | Status | File:line | Vendor / site that needs it |
|---|---|---|---|
| `HTMLElement` | ✅ | `dom_bootstrap.js:1032` | universal |
| `HTMLFormElement` | ✅ | `dom_bootstrap.js:1067-1110` | reddit |
| `HTMLFormElement.submit` | ✅ (sets `__pendingNavigation`) | `dom_bootstrap.js:1068-1107` | reddit |
| `HTMLFormElement.requestSubmit` | ✅ delegates to `submit()` | `dom_bootstrap.js:1108-1110` | reddit |
| **`HTMLFormElement.elements`** | **❌ MISSING — no getter on prototype** | (would go after `dom_bootstrap.js:1110`) | **reddit (see 05 § 1.3 H2)** |
| `HTMLFormElement.action / method / enctype / target / name / noValidate` | ✅ via `_reflectStr/_reflectBool` | `dom_bootstrap.js:1144-1149` | reddit |
| **`HTMLFormControlsCollection`** | **❌ MISSING** (not defined; depends on `.elements`) | (would go in `dom_bootstrap.js`) | **reddit (the `e.elements.namedItem('solution')` call)** |
| `HTMLInputElement` | ✅ | `dom_bootstrap.js:1066` | universal |
| `HTMLInputElement.value / checked / disabled / readOnly / required / type / placeholder` | ✅ via reflection | `dom_bootstrap.js:1136-1143` | universal |
| `HTMLInputElement.files` | ❌ MISSING | n/a | (file-upload SPAs — none in 126-corpus) |
| `HTMLSelectElement` | 🟡 marker only | `dom_bootstrap.js:1152` | (forms — partial) |
| `HTMLSelectElement.options / selectedIndex / value` | ❌ MISSING (would need real impl) | n/a | (forms — partial) |
| `HTMLTextAreaElement` | 🟡 marker only | `dom_bootstrap.js:1153` | (forms — partial) |
| `HTMLButtonElement` | 🟡 marker only | `dom_bootstrap.js:1151` | (forms — partial) |
| `HTMLLabelElement` | 🟡 marker only | `dom_bootstrap.js:1201` | (form labels) |
| `HTMLOptionElement` | 🟡 marker only | `dom_bootstrap.js:1202` | (select options) |
| `HTMLCanvasElement` | ✅ | `dom_bootstrap.js:1154` + `canvas_bootstrap.js:887-981` | universal (fingerprint) |
| `HTMLCanvasElement.width / height / toDataURL / getContext` | ✅ real (skia-backed) | `dom_bootstrap.js:1155-1183` + `canvas_bootstrap.js` | CreepJS, Kasada canvas fp |
| `HTMLCanvasElement.captureStream` | ❌ MISSING | n/a | (rare; some video sites) |
| `HTMLImageElement` | ✅ | `dom_bootstrap.js:1038-1065` | universal |
| `HTMLImageElement.width / height / naturalWidth / naturalHeight / complete / decode` | ✅ | `dom_bootstrap.js:1039-1065` | universal |
| `HTMLVideoElement` | 🟡 marker + decoded-frame APIs | `dom_bootstrap.js:1190` + `window_bootstrap.js:5835` | YouTube embeds (skipped) |
| `HTMLAudioElement` | 🟡 marker only | `dom_bootstrap.js:1191` | (rare) |
| `HTMLMediaElement.canPlayType` | ✅ real codec lookup | `window_bootstrap.js:5176-5189` | Castle |
| `HTMLMediaElement.play / pause / load / volume / currentTime` | ❌ MISSING (no real media stack) | n/a | (video-gate SPAs — rare) |
| `HTMLScriptElement` | 🟡 marker; `src` setter triggers fetch | `dom_bootstrap.js:1184` + `dom_bootstrap.js:1375-1389` | universal |
| `HTMLLinkElement` | 🟡 marker only | `dom_bootstrap.js:1186` | universal |
| `HTMLAnchorElement` | 🟡 marker; `href` reflects via `_reflectStr` | `dom_bootstrap.js:1037` | universal |
| `HTMLAnchorElement.click()` → navigation | ✅ inherited Element.click; sets `__pendingNavigation` via location? | (verify) | (navigation triggers) |
| `HTMLMetaElement` | 🟡 marker; `<meta http-equiv=refresh>` scanner separately at `page.rs:3338-3366` | `dom_bootstrap.js:1187` | universal |
| `HTMLTableElement` and subrows | 🟡 marker only | `dom_bootstrap.js:1188,1198-1200` | universal |
| `HTMLIFrameElement` | 🟡 marker; iframe realm wired separately | `dom_bootstrap.js:1189` + `dom_bootstrap.js:2287-2931` | DataDome, Akamai |
| `HTMLTemplateElement.content` | ❌ MISSING (would need DocumentFragment-typed inner) | n/a | (Web Components — none in corpus) |
| `HTMLSlotElement` | ❌ MISSING | n/a | (Web Components) |
| `HTMLDialogElement` (`showModal / close`) | ❌ MISSING | n/a | (rare) |
| `HTMLDetailsElement / HTMLSummaryElement` | ❌ MISSING | n/a | (rare) |
| `HTMLProgressElement / HTMLMeterElement` | ❌ MISSING | n/a | (rare) |
| `HTMLFieldSetElement` | ❌ MISSING | n/a | (form fieldsets) |
| `HTMLOptGroupElement / HTMLOptionsCollection` | ❌ MISSING | n/a | (forms) |
| `HTMLAllCollection` (`document.all`) | ✅ | `dom_bootstrap.js:1282-1303` | Castle |

### 2.2 DOM

| Interface | Status | File:line | Vendor / site that needs it |
|---|---|---|---|
| `EventTarget` | ✅ | `event_bootstrap.js:276-461` + `dom_bootstrap.js:424` | universal |
| `Node` (`parentNode / childNodes / appendChild / removeChild / cloneNode / textContent`) | ✅ real | `dom_bootstrap.js:426-575` | universal |
| `Element` (`querySelector / closest / getAttribute / classList / style / innerHTML / outerHTML / getBoundingClientRect / matches`) | ✅ real | `dom_bootstrap.js:642-1024` | universal |
| `Element.attachShadow` | ✅ real (`op_dom_attach_shadow`) | `dom_bootstrap.js:1003-1022` | Kasada `sdt.c` |
| `Element.shadowRoot` | ✅ | `dom_bootstrap.js:1023` | (web components) |
| `Element.scrollIntoView` | ❓ check | (verify) | (rare) |
| `ShadowRoot` | 🟡 inherits Node; mode + host + innerHTML | `dom_bootstrap.js:1010-1019` | (web components) |
| `Document` (`getElementById / querySelector / createElement / createTextNode / createComment / createRange / createTreeWalker / createNodeIterator / createEvent / write / writeln / open / close`) | ✅ | `dom_bootstrap.js:1305-1566` | universal |
| `Document.cookie` getter/setter | ✅ wired to net::SharedSession | (verify location) | DataDome |
| `Document.forms / images / scripts / styleSheets` | ✅ via `getElementsByTagName` | `dom_bootstrap.js:1326+` | reddit |
| `DocumentFragment` | ✅ class | `dom_bootstrap.js:1277` | universal |
| `Text / Comment / Attr` | ✅ (Text, Comment) / ❌ (Attr typed) | `dom_bootstrap.js:1265-1276` | universal |
| `MutationObserver` (`observe / disconnect / takeRecords`) | ✅ class + dispatch path | `dom_bootstrap.js:1814-2031` | Akamai sensor |
| `MutationRecord` | ✅ | `dom_bootstrap.js:1814-1826` | Akamai |
| `IntersectionObserver` (`observe / unobserve / disconnect / takeRecords`) | 🟡 fires "always intersecting" callback via microtask | `window_bootstrap.js:3327-3353` | booking SPA gate |
| `IntersectionObserverEntry` | ✅ | `window_bootstrap.js:3315-3324` | duolingo, booking |
| `ResizeObserver` | 🟡 fires once with current dims | `window_bootstrap.js:3356-3376` | many SPAs |
| `PerformanceObserver` | 🟡 stub class | `window_bootstrap.js:2183-2202` | Akamai sensor |
| `Range` | 🟡 minimal stub | `dom_bootstrap.js:1610-1631` | (rare) |
| `Selection` (`getSelection`) | 🟡 minimal stub | `dom_bootstrap.js:1633-1672` | (rare) |
| `TreeWalker` | 🟡 returns object literal (no class prototype) | `dom_bootstrap.js:1415-1417` | (rare; could be an FP) |
| `NodeIterator` | 🟡 same | `dom_bootstrap.js:1418-1420` | (rare) |
| `XPathEvaluator / XPathResult / XPathExpression` | ❌ MISSING | n/a | (rare) |
| `NodeList` | ✅ array-like with `forEach / item / [Symbol.iterator]` | `dom_bootstrap.js:300-347` | universal |
| `NodeFilter` (the constants object) | ❌ MISSING (used by TreeWalker; rare) | n/a | (rare) |
| `NamedNodeMap` | ❌ MISSING (we don't return one from `Element.attributes`) | n/a | (Akamai sensor reads `element.attributes`) |
| `DOMTokenList` (classList) | ✅ | `dom_bootstrap.js:348-422` | universal |
| `DOMRect / DOMRectReadOnly / DOMPoint / DOMPointReadOnly / DOMMatrix / DOMMatrixReadOnly` | ✅ | `dom_bootstrap.js:103-133` + `shared_apis_bootstrap.js:568-592` | universal |
| `DOMParser` | 🟡 stub; `parseFromString` returns a Document but doesn't parse arbitrary trees | `dom_bootstrap.js:1796-1813` | recaptcha, many SPAs |
| `XMLSerializer` (`serializeToString`) | ❌ MISSING (interface stubbed by `interfaces_bootstrap.js` but no impl) | n/a | (rare) |
| `MathML elements (MathMLElement)` | ❌ MISSING | n/a | (universities / docs) |
| `CSS namespace (CSS.supports / CSS.escape / CSS.registerProperty)` | ✅ (`CSS.escape` used at `dom_bootstrap.js:1290`) — verify .supports | (verify) | booking SPA |
| `CustomElementRegistry (customElements)` (`define / get / whenDefined / upgrade`) | ✅ | `window_bootstrap.js:4274-4317` | universal |
| `Image` (HTMLImageElement constructor) | ✅ | `dom_bootstrap.js:1788` | (rare) |
| `Audio` (HTMLAudioElement constructor) | ❌ MISSING | n/a | (audio-bot probes) |
| `Option` (HTMLOptionElement constructor) | ❌ MISSING | n/a | (forms) |

### 2.3 CSSOM

| Interface | Status | File:line | Vendor / site that needs it |
|---|---|---|---|
| `getComputedStyle` | ✅ real (`op_layout_compute_style`) | `window_bootstrap.js:3403-3460` | universal |
| `CSSStyleDeclaration` (element.style proxy) | ✅ via Proxy | `dom_bootstrap.js:583-635` | universal |
| `CSSStyleSheet` | 🟡 minimal | `dom_bootstrap.js:1567-1583` | universal |
| `CSSStyleRule` | 🟡 minimal | `dom_bootstrap.js:1584-1608` | (rare) |
| `CSSRule / CSSMediaRule / CSSImportRule / CSSKeyframesRule / CSSKeyframeRule / CSSFontFaceRule / CSSPageRule / CSSSupportsRule / CSSLayerBlockRule / CSSScopeRule / CSSStartingStyleRule / CSSContainerRule / CSSPositionTryRule` | ❌ MISSING (illegal-ctor stubs only via `interfaces_bootstrap.js`) | n/a | Castle |
| `MediaList / MediaQueryList / matchMedia` | ✅ class + impl | `window_bootstrap.js:3987-4015` | universal |
| `CSS Typed OM (CSSStyleValue, CSSUnitValue, …)` | ❌ MISSING (illegal-ctor stubs only) | n/a | (rare) |
| `StyleSheet / StyleSheetList` | 🟡 stubs | `interfaces_bootstrap.js:58` | (rare) |
| `FontFace / FontFaceSet (document.fonts)` | 🟡 class + load Promise stub | `window_bootstrap.js:5035` | Castle, font-fingerprint |

### 2.4 Fetch

| Interface | Status | File:line | Vendor / site that needs it |
|---|---|---|---|
| `fetch(url, init)` | ✅ real (`op_fetch` + boring2 TLS) | `fetch_bootstrap.js:175` | universal |
| `Request` | ✅ class | `fetch_bootstrap.js:107-140` | universal |
| `Request.signal` accessor | ✅ on prototype (per duolingo fix) | `fetch_bootstrap.js:120-138` | duolingo |
| `Response` | ✅ class with text/json/arrayBuffer/blob/body stream | `fetch_bootstrap.js:27-105` | universal |
| `Response.body` (ReadableStream) | ✅ lazy stream | `fetch_bootstrap.js:53-71` | many SPAs |
| `Response.formData()` | ❌ MISSING | n/a | (rare) |
| `Headers` | ✅ class with get/set/has/delete/forEach/entries/keys/values | `fetch_bootstrap.js:4-25` | universal |
| `AbortController / AbortSignal` | ✅ | `window_bootstrap.js:4066-4111` + `shared_apis_bootstrap.js:376-402` | duolingo, many SPAs |
| `AbortSignal.timeout(ms)` | ✅ | `shared_apis_bootstrap.js:382-389` | (rare) |
| `AbortSignal.any(signals)` | ❌ MISSING | n/a | (rare; Chrome 116+) |
| `FormData` | ✅ | `shared_apis_bootstrap.js:354-369` | universal |
| `URL / URLSearchParams` | ✅ | `shared_apis_bootstrap.js:268-303` | universal |
| `URLPattern` | ❌ MISSING (illegal-ctor stub only) | n/a | (rare; Chrome 95+) |
| `XMLHttpRequest` | ✅ real (sync + async via `op_net_fetch_sync` / `op_fetch`) | `window_bootstrap.js:3465-3683` | Akamai, all legacy sites |
| `XMLHttpRequestUpload` | 🟡 class with event handler slots | `window_bootstrap.js:3494-3506` | (file uploads — rare) |
| `EventSource` (SSE) | ✅ real impl in `sse_bootstrap.js` (replaces window_bootstrap stub) | `sse_bootstrap.js` | (chat/streams sites) |

### 2.5 Workers / Messaging

| Interface | Status | File:line | Vendor / site that needs it |
|---|---|---|---|
| `Worker(url, opts)` | ✅ real (separate OS thread + child JsRuntime) | `window_bootstrap.js:1879-2044` + `worker_ext.rs:214-358` | duolingo, AWS WAF |
| `Worker.postMessage / addEventListener / terminate` | ✅ real (wire-serialized via `op_worker_post_to_worker`) | `window_bootstrap.js:1970-2044` | universal |
| `Worker.prototype.name` getter | 🟡 stored as `_name` instance prop; no public getter — see 05 § 2.3 H3 | `window_bootstrap.js:1883` | (rare) |
| `SharedWorker` | 🟡 stub class (no real shared isolate) | `window_bootstrap.js:2045-2063` | (rare) |
| `ServiceWorker` | 🟡 stub class | `window_bootstrap.js:2065-2077` | (rare) |
| `ServiceWorkerContainer (navigator.serviceWorker)` | 🟡 stubs Promise-resolving methods | `window_bootstrap.js:842-889` | DataDome |
| `ServiceWorkerRegistration` | ❌ MISSING (returned object is plain `{}`) | n/a | DataDome |
| `WorkerGlobalScope / DedicatedWorkerGlobalScope` | 🟡 stub classes | `window_bootstrap.js:2078-2081` | (rare) |
| **`MessageChannel`** | **🟡 NO-OP stub — ports are NOT paired** | `window_bootstrap.js:2265-2271` | **duolingo, booking, many SPAs (see 05 § 2.3 H1)** |
| **`MessagePort`** | **🟡 NO-OP stub — `postMessage` is empty** | `window_bootstrap.js:2256-2263` | **duolingo (see 05 § 2.3 H1)** |
| `BroadcastChannel` | 🟡 stub class with no cross-tab dispatch | `window_bootstrap.js:2248-2253` | (rare; cross-tab sync) |
| `importScripts` (worker-side) | ✅ real (sync fetch + eval) | `worker_bootstrap.js:232-256` | recaptcha worker |
| `Worker module loading (type: 'module')` | ✅ `_options.type === 'module'` path | `window_bootstrap.js:1884-1888` | (rare) |
| `OffscreenCanvas` | 🟡 class returns null context | `window_bootstrap.js:4367-4384` + `shared_apis_bootstrap.js:408-418` | duolingo image-rec, recaptcha |
| `OffscreenCanvas.transferControlToOffscreen` (on HTMLCanvasElement) | ✅ (per `state_2026_05_14_sota_kasada_audit_fixes.md`) | `canvas_bootstrap.js` (verify) | Kasada audit |
| `MessageEvent` | ✅ class | `event_bootstrap.js:165-175` | universal |

### 2.6 Storage

| Interface | Status | File:line | Vendor / site that needs it |
|---|---|---|---|
| `localStorage / sessionStorage` (`Storage` interface) | ✅ in-memory via `op_storage_*` | `window_bootstrap.js:3262-3302` | universal |
| `Storage` class (the prototype) | 🟡 instance-only; no `Storage.prototype` exposed | (verify) | (rare) |
| `indexedDB / IDBFactory / IDBDatabase / IDBTransaction / IDBObjectStore / IDBRequest / IDBOpenDBRequest / IDBKeyRange / IDBCursor` | ✅ in-memory full impl | `shared_apis_bootstrap.js:423-543` + `window_bootstrap.js:4855-4925` | DataDome, many SPAs |
| `IDBIndex` (`createIndex / index`) | 🟡 returns plain `{}` proxy | `shared_apis_bootstrap.js:493-494` | (rare) |
| `IDBVersionChangeEvent` | ❌ MISSING (fired event is plain `{}`) | n/a | (rare) |
| `CookieStore` (`get / getAll / set / delete / subscribeToChanges`) | 🟡 class with stub methods | `window_bootstrap.js:6107-6143` | (Chrome 87+) |
| `CookieStoreManager` | ❌ MISSING | n/a | (Chrome 87+) |
| `CacheStorage` (`caches.open / match / has / delete / keys`) | 🟡 stub class | `window_bootstrap.js:6066-6086` | DataDome |
| `Cache` (`add / addAll / put / match / matchAll / delete / keys`) | ❌ MISSING (caches.open returns nothing usable) | n/a | (rare) |
| `StorageManager (navigator.storage)` (`estimate / persist / persisted`) | 🟡 returns Promise-resolved fixed values | `window_bootstrap.js:812-839` | (rare) |
| `FileSystemHandle / FileSystemDirectoryHandle / FileSystemFileHandle / FileSystemObserver / FileSystemWritableFileStream` | ❌ MISSING (illegal-ctor stubs only) | n/a | (rare) |
| `Lock / LockManager (navigator.locks)` | 🟡 stub class | `window_bootstrap.js:730-740` | (rare) |

### 2.7 Permissions

| Interface | Status | File:line | Vendor / site that needs it |
|---|---|---|---|
| `Permissions.query({name})` | ✅ returns Permission state per profile | `window_bootstrap.js:469-541` | AWS WAF, DataDome |
| `PermissionStatus` (`state / name / onchange`) | ✅ | `window_bootstrap.js:505-520` | universal |
| `PermissionDescriptor` validation (rejects unknown names with TypeError) | ✅ matches Chrome | `window_bootstrap.js:528-539` | DataDome (probes the rejection) |
| Per-API `requestPermission` (Notification, IdleDetector, …) | ✅ each Promise-resolves `denied` | `window_bootstrap.js:6196-6298` | (rare) |

### 2.8 Performance

| Interface | Status | File:line | Vendor / site that needs it |
|---|---|---|---|
| `performance.now()` | 🟡 `Date.now() - startTime` — millisecond granularity, NOT Chrome's sub-ms | `timer_bootstrap.js:170-173`; worker: `worker_bootstrap.js:134` uses `op_perf_now_humanized` | AWS WAF (timing probe) |
| `performance.timeOrigin` | ❓ check | (verify) | Castle |
| `performance.mark(name, opts)` | ✅ | `window_bootstrap.js:2902` | (rare) |
| `performance.measure(name, start, end)` | ✅ | (verify) | (rare) |
| `performance.getEntries() / getEntriesByType() / getEntriesByName()` | ✅ | `window_bootstrap.js:2885+` | Akamai sensor |
| `performance.memory` (jsHeapSizeLimit, totalJSHeapSize, usedJSHeapSize) | ✅ fixed + jitter in worker | `window_bootstrap.js:2333-2337` + `worker_bootstrap.js:142-153` | Akamai sensor, Castle |
| `performance.measureUserAgentSpecificMemory()` | ❌ MISSING | n/a | douyin (per 05 § 4.2 H3) |
| `PerformanceObserver` | 🟡 stub | `window_bootstrap.js:2183-2202` | Akamai sensor |
| `PerformanceEntry / PerformanceMark / PerformanceMeasure / PerformanceResourceTiming / PerformanceNavigationTiming / PerformancePaintTiming / PerformanceEventTiming / PerformanceLongTaskTiming / PerformanceLongAnimationFrameTiming / LargestContentfulPaint / LayoutShift / TaskAttributionTiming` | 🟡 illegal-ctor stubs only; cannot construct | `interfaces_bootstrap.js:58` | Akamai sensor reads `entryType` |
| `PerformanceTiming` (legacy `performance.timing`) | ❓ check (likely stubbed via getter) | (verify) | legacy analytics |
| `EventCounts` (`performance.eventCounts`) | ✅ class with `size / get / forEach` | `window_bootstrap.js:6145-6159` | (rare) |

### 2.9 WebSocket / WebRTC / WebTransport

| Interface | Status | File:line | Vendor / site that needs it |
|---|---|---|---|
| `WebSocket` | ✅ real (tokio-tungstenite via `op_ws_*`) | `window_bootstrap.js:3686-3757` | (chat / live sites) |
| `CloseEvent` | ✅ | `window_bootstrap.js:3761-3768` | (chat / live sites) |
| `RTCPeerConnection` | 🟡 stub class (createOffer/Answer return Promise<{}>) | `window_bootstrap.js:4936-5017` | CreepJS (probes existence + shape) |
| `RTCSessionDescription / RTCIceCandidate / RTCDataChannel` | 🟡 stub class | `window_bootstrap.js:4931-5019` | CreepJS |
| `RTCRtpReceiver / RTCRtpSender (.getCapabilities)` | 🟡 has `getCapabilities` masked | `window_bootstrap.js:5548-5560` | Kasada |
| `WebTransport / WebTransportBidirectionalStream / WebTransportDatagramDuplexStream / WebTransportError` | ❌ MISSING | n/a | (Chrome 97+; rare in corpus) |

### 2.10 WebGL / WebGPU

| Interface | Status | File:line | Vendor / site that needs it |
|---|---|---|---|
| `WebGLRenderingContext` (~80 methods + constants) | ✅ real values via Canvas2D pixel routing + profile params | `canvas_bootstrap.js:266-585` + `webgl_ext.rs` | universal (fingerprint) |
| `WebGL2RenderingContext` | ✅ alias to WebGLRenderingContext (`canvas_bootstrap.js:1010`) | (verify) | universal |
| `WebGLActiveInfo / WebGLBuffer / WebGLFramebuffer / WebGLProgram / WebGLQuery / WebGLRenderbuffer / WebGLSampler / WebGLShader / WebGLShaderPrecisionFormat / WebGLSync / WebGLTexture / WebGLTransformFeedback / WebGLUniformLocation / WebGLVertexArrayObject` | 🟡 illegal-ctor stubs only | `interfaces_bootstrap.js:58` | CreepJS |
| `WebGLContextEvent` | ❌ MISSING (no event firing) | n/a | (rare) |
| `GPU (navigator.gpu)` | ❌ MISSING (no WebGPU adapter) | n/a | (Chrome 113+; rare in corpus, growing) |
| `GPUAdapter / GPUDevice / GPUBuffer / ...` | ❌ MISSING (illegal-ctor stubs only) | n/a | (Chrome 113+) |

### 2.11 Crypto

| Interface | Status | File:line | Vendor / site that needs it |
|---|---|---|---|
| `crypto.getRandomValues(view)` | ✅ real (`op_crypto_random_fill`) | `window_bootstrap.js:2991` | universal |
| `crypto.randomUUID()` | ✅ real | `window_bootstrap.js:3003` | universal |
| `crypto.subtle.digest(alg, data)` | ✅ real (`op_crypto_digest` for SHA-1/256/384/512) | `window_bootstrap.js:2969-2979` | AWS WAF, DataDome |
| `crypto.subtle.sign / verify / encrypt / decrypt / generateKey / importKey / exportKey / deriveKey / deriveBits / wrapKey / unwrapKey` | 🟡 each rejects with `NotSupportedError` | `window_bootstrap.js:2980-2988` | douyin (`__ac_signature` per 05 § 4.2 H1) |
| `CryptoKey / CryptoKeyPair` | 🟡 illegal-ctor stub | (verify) | (rare) |

### 2.12 Streams

| Interface | Status | File:line | Vendor / site that needs it |
|---|---|---|---|
| `ReadableStream` (`getReader / pipeTo / pipeThrough / tee / cancel`) | ✅ real backpressure | `streams_bootstrap.js:171-394` | universal (fetch().body) |
| `ReadableStreamDefaultReader` (`read / cancel / releaseLock`) | ✅ | `streams_bootstrap.js:63-169` | universal |
| `ReadableStreamBYOBReader / ReadableByteStreamController / ReadableStreamBYOBRequest` | ❌ MISSING (illegal-ctor stubs only) | `interfaces_bootstrap.js:58` | (rare; binary streams) |
| `WritableStream` (`getWriter / abort / close`) | ✅ | `streams_bootstrap.js:482-528` | (rare) |
| `WritableStreamDefaultWriter / WritableStreamDefaultController` | ✅ | `streams_bootstrap.js:396-481` | (rare) |
| `TransformStream` | ✅ class | `streams_bootstrap.js:531-579` | (rare) |
| `TransformStreamDefaultController` | ❌ MISSING (stubbed only) | n/a | (rare) |
| `CompressionStream / DecompressionStream` | 🟡 stub returning empty streams | `window_bootstrap.js:2293-2308` | (rare) |
| `ByteLengthQueuingStrategy / CountQueuingStrategy` | ❌ MISSING (illegal-ctor stubs only) | n/a | (rare) |
| `TextEncoderStream / TextDecoderStream` | ❌ MISSING (illegal-ctor stubs only) | n/a | (rare) |

### 2.13 Text I/O

| Interface | Status | File:line | Vendor / site that needs it |
|---|---|---|---|
| `TextEncoder` (`encode / encodeInto / encoding`) | ✅ real with Chrome-shape getters | `window_bootstrap.js:3041-3105` | universal |
| `TextDecoder` (`decode / encoding / fatal / ignoreBOM`) | ✅ real | `window_bootstrap.js:3121-3165` | universal |

### 2.14 Geolocation / Sensors

| Interface | Status | File:line | Vendor / site that needs it |
|---|---|---|---|
| `Geolocation` (`getCurrentPosition / watchPosition / clearWatch`) | ✅ class; methods all error with code 1 "User denied" | `window_bootstrap.js:903-926` | (rare) |
| `GeolocationCoordinates / GeolocationPosition / GeolocationPositionError` | 🟡 illegal-ctor stubs | `interfaces_bootstrap.js:58` | (rare) |
| `DeviceOrientationEvent / DeviceMotionEvent` | 🟡 illegal-ctor stubs | `interfaces_bootstrap.js:58` | (rare) |
| `Accelerometer / Gyroscope / LinearAccelerationSensor / GravitySensor / Magnetometer / AbsoluteOrientationSensor / RelativeOrientationSensor` | ❌ MISSING (stubs only) | n/a | (rare; Generic Sensor API) |

### 2.15 Notifications / Background

| Interface | Status | File:line | Vendor / site that needs it |
|---|---|---|---|
| `Notification` (`requestPermission` static, constructor, prototype) | ✅ Promise-resolving (default→denied on insecure, denied on secure) | `window_bootstrap.js:6196-6252` | Cloudflare, DataDome |
| `Notification.permission` static | ✅ via getter | `window_bootstrap.js:6240` | (rare) |
| `PushManager / PushSubscription / PushSubscriptionOptions` | ❌ MISSING (stubs only) | n/a | (rare) |
| `BackgroundFetchManager / BackgroundFetchRecord / BackgroundFetchRegistration` | ❌ MISSING (stubs only) | n/a | (rare) |
| `PeriodicSyncManager / SyncManager` | ❌ MISSING (stubs only) | n/a | (rare) |

### 2.16 Clipboard / Drag-and-drop

| Interface | Status | File:line | Vendor / site that needs it |
|---|---|---|---|
| `Clipboard` (`readText / writeText / read / write`) | 🟡 class with read/writeText only; both no-op resolve | `window_bootstrap.js:893-898` | (rare) |
| `ClipboardItem` | ❌ MISSING (stub only) | n/a | (rare) |
| `ClipboardEvent` | ✅ class | `event_bootstrap.js:214-219` | (rare) |
| `DataTransfer / DataTransferItem / DataTransferItemList` | ❌ MISSING (stubs only) | n/a | (drag-drop SPAs) |
| `DragEvent` | ✅ class | `event_bootstrap.js:261-273` | (drag-drop SPAs) |

### 2.17 Media

| Interface | Status | File:line | Vendor / site that needs it |
|---|---|---|---|
| `MediaSource` (`isTypeSupported / addSourceBuffer / endOfStream`) | ✅ static `isTypeSupported`; instance stubs | `window_bootstrap.js:5492-5509` + `worker_bootstrap.js:271-279` | Castle |
| `SourceBuffer / SourceBufferList` | 🟡 stub classes | `window_bootstrap.js:5473-5491` | (rare) |
| `MediaRecorder` | ✅ static `isTypeSupported`; instance stubs (start/stop/pause/resume) | `window_bootstrap.js:5512-5529` + `worker_bootstrap.js:281-290` | (rare) |
| `MediaCapabilities` (`decodingInfo / encodingInfo`) | ✅ class returning supported=true for known codecs | `window_bootstrap.js:5735-5769` | DataDome |
| `MediaDevices` (`enumerateDevices / getUserMedia / getDisplayMedia / getSupportedConstraints`) | 🟡 each returns Promise.reject (NotFoundError) | `window_bootstrap.js:443-466` | CreepJS, Castle |
| `MediaStream / MediaStreamTrack` | ❌ MISSING (stubs only) | n/a | (rare) |
| `MediaSession / MediaMetadata` | ✅ class with setActionHandler etc. | `window_bootstrap.js:5622-5665` | (rare) |
| `MediaKeys / MediaKeySession / MediaKeyStatusMap / MediaKeySystemAccess` | ❌ MISSING (stubs only) | n/a | (DRM — rare) |
| `AudioContext (Web Audio API)` (all 40+ node types) | ✅ class with real DynamicsCompressor + Oscillator + Gain + BiquadFilter + Analyser nodes | `canvas_bootstrap.js:585-1014` | Akamai T1.3, CreepJS audio fp |
| `OfflineAudioContext` (`startRendering`) | ✅ class with real render | `canvas_bootstrap.js:805-885` | Akamai T1.3 |
| `AudioWorklet / AudioWorkletNode` | ❌ MISSING (stubs only) | n/a | (rare) |
| `AudioData / AudioDecoder / AudioEncoder / EncodedAudioChunk / EncodedVideoChunk / VideoDecoder / VideoEncoder / VideoFrame / ImageDecoder` (WebCodecs) | ❌ MISSING (stubs only) | n/a | (rare; growing) |
| `ImageBitmap / createImageBitmap` | ✅ stub class | `window_bootstrap.js:2120-2127` + `shared_apis_bootstrap.js:560-566` | duolingo image-rec |

### 2.18 WebAuthn / FedCM / Credentials

| Interface | Status | File:line | Vendor / site that needs it |
|---|---|---|---|
| `PublicKeyCredential` (+ static `isUserVerifyingPlatformAuthenticatorAvailable / isConditionalMediationAvailable / getClientCapabilities`) | ✅ class with all statics | `window_bootstrap.js:574-611` | (rare; passkey sites) |
| `AuthenticatorResponse / AuthenticatorAttestationResponse / AuthenticatorAssertionResponse` | ✅ classes | `window_bootstrap.js:561-572` | (rare) |
| `CredentialsContainer (navigator.credentials)` (`create / get / store / preventSilentAccess`) | 🟡 each resolves undefined | `window_bootstrap.js:637-678` | (rare) |
| `IdentityCredential / IdentityProvider` (FedCM) | ✅ classes; `IdentityProvider.getUserInfo` stub | `window_bootstrap.js:613-627` | (rare) |
| `FederatedCredential / PasswordCredential` | 🟡 illegal-ctor stubs | `interfaces_bootstrap.js:58` | (rare) |
| `OTPCredential / DigitalCredential` | 🟡 illegal-ctor stubs | `interfaces_bootstrap.js:58` | (rare) |

### 2.19 Trusted Types / CSP

| Interface | Status | File:line | Vendor / site that needs it |
|---|---|---|---|
| `trustedTypes (createPolicy / isHTML / isScript / isScriptURL / getAttributeType / getPropertyType)` | ✅ object with all methods | `window_bootstrap.js:5914-5938` | (CSP-enforced sites — rare) |
| `TrustedHTML / TrustedScript / TrustedScriptURL` | ✅ classes | `window_bootstrap.js:5935-5937` | (rare) |
| `TrustedTypePolicy / TrustedTypePolicyFactory` | ✅ via createPolicy return | `window_bootstrap.js:5914` | (rare) |
| `SecurityPolicyViolationEvent` | ✅ class | `event_bootstrap.js:471-489` | (rare) |
| `Sanitizer` API | ❌ MISSING (stub only) | n/a | (Chrome 105+; rare) |

### 2.20 Payments / Wallets

| Interface | Status | File:line | Vendor / site that needs it |
|---|---|---|---|
| `PaymentRequest` (`show / abort / canMakePayment / hasEnrolledInstrument`) | ✅ class with stub methods | `window_bootstrap.js:6496-6557` | (rare) |
| `PaymentResponse / PaymentMethodChangeEvent / PaymentRequestUpdateEvent` | ✅ classes | `window_bootstrap.js:6559-6605` | (rare) |
| `PaymentAddress / PaymentManager` | 🟡 illegal-ctor stubs | `interfaces_bootstrap.js:58` | (rare) |
| `ApplePaySession` (`canMakePayments / canMakePaymentsWithActiveCard / supportsVersion`) | ✅ on iphone profile only | `window_bootstrap.js:6263-6266` | iPhone profile differentiator |

### 2.21 Navigation API (modern, Chrome 102+)

| Interface | Status | File:line | Vendor / site that needs it |
|---|---|---|---|
| `Navigation (navigator.navigation)` | ❌ MISSING (stub only) | n/a | (modern SPAs — rare) |
| `NavigateEvent / NavigationDestination / NavigationHistoryEntry / NavigationCurrentEntryChangeEvent / NavigationTransition / NavigationPrecommitController / NavigationPreloadManager / NavigationActivation` | ❌ MISSING (stubs only) | n/a | (modern SPAs) |
| `History (window.history)` (`pushState / replaceState / back / forward / go / state / length`) | ✅ via Object.create(_HistoryProto) | `window_bootstrap.js:3793` | universal |
| `Location` (`assign / replace / reload / href / origin / protocol / host / hostname / port / pathname / search / hash`) | ✅ | `window_bootstrap.js:1266-1413` | universal |
| `PopStateEvent / HashChangeEvent` | ✅ classes | `event_bootstrap.js:221-235` | universal |

### 2.22 Visual Viewport / Scrolling / Animations

| Interface | Status | File:line | Vendor / site that needs it |
|---|---|---|---|
| `VisualViewport (window.visualViewport)` | ✅ class with `width / height / scale / pageLeft / pageTop` + EventTarget | `window_bootstrap.js:5570-5588` | (mobile SPAs) |
| `Animation / KeyframeEffect / AnimationEffect / AnimationTimeline / AnimationPlaybackEvent / DocumentTimeline / ViewTimeline / ScrollTimeline` | ❌ MISSING (illegal-ctor stubs) | n/a | (rare) |
| `Element.animate()` (returns an Animation) | 🟡 returns stub `{finished, cancel, play, pause}` | `dom_bootstrap.js:1001` | (rare) |
| `Element.getAnimations()` | ✅ returns `[]` | `dom_bootstrap.js:1002` | (rare) |

### 2.23 Touch / Pointer / Input

| Interface | Status | File:line | Vendor / site that needs it |
|---|---|---|---|
| `Touch / TouchEvent / TouchList` | ✅ classes (iPhone/Pixel profiles) | `window_bootstrap.js:5985-6031` | mobile SPAs |
| `PointerEvent / MouseEvent / WheelEvent / KeyboardEvent / FocusEvent / InputEvent / UIEvent` | ✅ classes | `event_bootstrap.js:64-152` | universal |
| `InputDeviceCapabilities` | ✅ stub class | `window_bootstrap.js:5603-5611` | (rare) |
| `InputDeviceInfo` | 🟡 illegal-ctor stub | `interfaces_bootstrap.js:58` | (rare) |
| `CompositionEvent` | ❌ MISSING (illegal-ctor stub only) | n/a | (CJK input) |
| `BeforeInstallPromptEvent` | ❌ MISSING (illegal-ctor stub only) | n/a | (PWA install) |

### 2.24 Misc Web Platform

| Interface | Status | File:line | Vendor / site that needs it |
|---|---|---|---|
| `IdleDetector` (`requestPermission / start / stop`) | ✅ stub class | `window_bootstrap.js:6279-6298` | (rare) |
| `EyeDropper` (`open`) | ✅ stub class | `window_bootstrap.js:6305-6314` | (rare; Chrome 95+) |
| `PressureObserver / PressureRecord` | ✅ stub classes | `window_bootstrap.js:5226-5257` | (Chrome 125+; rare) |
| `VirtualKeyboard (navigator.virtualKeyboard)` | ✅ stub class | `window_bootstrap.js:6320-6336` | (mobile rare) |
| `DevicePosture` | ✅ stub class | `window_bootstrap.js:6349-6360` | (rare; foldables) |
| `WindowControlsOverlay (navigator.windowControlsOverlay)` | ✅ stub class | `window_bootstrap.js:6375-6390` | (PWA — rare) |
| `ViewTransition (document.startViewTransition)` | ✅ stub class | `window_bootstrap.js:6403-6430` | (Chrome 111+) |
| `DocumentPictureInPicture (window.documentPictureInPicture)` | ✅ stub class | `window_bootstrap.js:5275-5290` | (rare) |
| `UserActivation (navigator.userActivation)` | ✅ stub class with `isActive / hasBeenActive` | `window_bootstrap.js:5304-5315` | Castle, Akamai sensor |
| `SpeechSynthesis (window.speechSynthesis)` + `SpeechSynthesisVoice / Utterance / Event` | ✅ object + 3 canned voices | `window_bootstrap.js:2312-2330` | CreepJS |
| `SpeechRecognition` | ❌ MISSING (stub only) | n/a | (rare) |
| `WakeLock / WakeLockSentinel (navigator.wakeLock.request)` | ❌ MISSING (stub only) | n/a | (rare) |
| `BarcodeDetector` | ❌ MISSING | n/a | (rare) |
| `FaceDetector / TextDetector` (Shape Detection API) | ❌ MISSING | n/a | (rare) |
| `Sanitizer` | ❌ MISSING (stub only) | n/a | (rare) |
| `Reporting / ReportingObserver` | 🟡 stub class | `window_bootstrap.js:2210` | (rare) |
| `External (window.external)` (`AddSearchProvider / IsSearchProviderInstalled`) | ✅ | `interfaces_bootstrap.js:158-164` | Akamai (probes existence) |
| `NetworkInformation (navigator.connection)` | ✅ class extends EventTarget; `effectiveType / downlink / rtt / saveData` from profile | `window_bootstrap.js:171-174` | Akamai sensor, CreepJS |
| `Keyboard (navigator.keyboard)` (`getLayoutMap / lock / unlock`) | ✅ stub class returning Promise<KeyboardLayoutMap> | `window_bootstrap.js:777-809` | (rare) |
| `Bluetooth (navigator.bluetooth)` (`getAvailability / requestDevice`) | ✅ class; both rejects-with-NotFoundError | `window_bootstrap.js:681-693` | CreepJS |
| `USB (navigator.usb)` (`getDevices / requestDevice`) | ✅ stub | `window_bootstrap.js:697-707` | CreepJS |
| `Serial (navigator.serial)` (`getPorts / requestPort`) | ✅ stub | `window_bootstrap.js:710-717` | CreepJS |
| `HID (navigator.hid)` (`getDevices / requestDevice`) | ✅ stub | `window_bootstrap.js:720-728` | CreepJS |
| `LockManager (navigator.locks)` (`request / query`) | ✅ stub | `window_bootstrap.js:730-740` | (rare) |
| `Presentation / PresentationRequest / PresentationConnection / ...` | 🟡 illegal-ctor stubs | `interfaces_bootstrap.js:58` | (rare; Cast) |
| `RemotePlayback` | ❌ MISSING | n/a | (rare) |
| `XRSystem (navigator.xr) / XR* family` | 🟡 illegal-ctor stubs (~60 names) | `interfaces_bootstrap.js:58` | (WebXR — rare) |

### 2.25 Window / global properties

| Property | Status | File:line | Vendor / site that needs it |
|---|---|---|---|
| `window / self` | ✅ | `dom_bootstrap.js:3098-3099` | universal |
| `globalThis.clientInformation` (= navigator) | ✅ | `window_bootstrap.js:6681` | Castle |
| `offscreenBuffering / defaultStatus / status / name` | ✅ | `window_bootstrap.js:6682-6685` | universal |
| `closed / crashReport / credentialless / external / fence / frameElement / launchQueue / locationbar / menubar / personalbar / scrollbars / statusbar / styleMedia / toolbar / viewport / webkitMediaStream / webkitURL` | ✅ | `interfaces_bootstrap.js:166-200` | universal |
| `chrome` object | ✅ on chrome_148 profile | `window_bootstrap.js:1625-1697` | Cloudflare |
| `devicePixelRatio` | ✅ | `window_bootstrap.js:1485` | universal |
| `innerWidth / innerHeight / outerWidth / outerHeight / screenX / screenY` | ✅ from profile | (verify) | universal |
| `crossOriginIsolated / isSecureContext / originAgentCluster` | ✅ getter from profile/state | (verify) | (rare) |
| `cookieStore / caches / sharedStorage / showOpenFilePicker / showSaveFilePicker / showDirectoryPicker` | 🟡 stubs/missing — see per-row entries | n/a | (rare) |

---

## 3. The "missing API" gotcha list (top 20 by impact)

These are the biggest known leverage points to flip a corpus site OR
significantly reduce vendor scoring. Pulled from § 2 entries marked ❌ or 🟡
with a known vendor probe.

| # | API | Status | Site(s) affected | Effort | Priority |
|--:|---|---|---|---|---|
| 1 | **`HTMLFormElement.elements` getter + `HTMLFormControlsCollection`** | ❌ | reddit | **1 day** | **P0 (≥ +1 site)** |
| 2 | **`MessagePort` / `MessageChannel` paired-port routing** | 🟡 NO-OP | duolingo, possibly booking, x.com | **2-3 days** | **P0 (≥ +1 site, possibly +3)** |
| 3 | **Event subclass constructors masked** (Event/MouseEvent/PointerEvent/…) | ❌ unmasked | Kasada `sdt` ML weight | 1 day | **P0 (Kasada fingerprint)** |
| 4 | `Headers.prototype` + `Request.prototype` + `Response.prototype` masked | ❌ unmasked | DataDome | 0.5 day | P1 |
| 5 | `XMLHttpRequest.prototype` masked | ❌ unmasked | Akamai | 0.5 day | P1 |
| 6 | `WebGLRenderingContext.prototype.getParameter` masked | ❌ unmasked | AWS WAF, CreepJS | 0.5 day | P1 |
| 7 | `Worker.prototype.postMessage` masked | ❌ unmasked | duolingo | 0.5 day | P1 |
| 8 | `IntersectionObserver` reads real bounding rect from layout op | 🟡 always-intersecting | booking, many SPAs | 2 days | P1 |
| 9 | `MutationObserver` observe + notify implemented for `attributes` | 🟡 partial | Akamai sensor | 2-3 days | P2 |
| 10 | `performance.now()` sub-millisecond resolution | 🟡 ms-only | AWS WAF timing | 1 day | P1 |
| 11 | `crypto.subtle.sign / verify / importKey / generateKey` real backing | 🟡 reject | douyin `__ac_signature`, payments sites | 3-5 days | P2 |
| 12 | `Notification.permission` profile-driven default | ✅ correct | (none currently broken) | n/a | done |
| 13 | `WebAssembly.instantiateStreaming` byte-exact roundtrip | ✅ V8 native | DataDome daily key | n/a | done |
| 14 | `History.pushState / replaceState` updates `window.location.href` correctly + fires popstate | 🟡 verify | universal | 1 day | P2 |
| 15 | `Audio()` constructor (HTMLAudioElement) | ❌ MISSING | (audio fingerprint probes — rare) | 0.5 day | P3 |
| 16 | `Option()` constructor (HTMLOptionElement) | ❌ MISSING | (form-heavy sites) | 0.5 day | P3 |
| 17 | `URLPattern` | ❌ MISSING | (modern SPAs — Chrome 95+) | 1 day | P3 |
| 18 | `ServiceWorkerRegistration` real shape | ❌ returned object is `{}` | DataDome (rare flow) | 1 day | P3 |
| 19 | `Animation / KeyframeEffect / Element.animate` real promise resolution | 🟡 stub | (rare animations-gate SPAs) | 2 days | P3 |
| 20 | `NamedNodeMap` (returned by `Element.attributes`) | ❌ MISSING | Akamai sensor reads attributes shape | 1 day | P2 |

### 3.1 Cross-reference

- #1 fixes the canonical reddit issue (`05_SPA_HYDRATION_CLUSTER.md § 1.2 H2`).
- #2 fixes the canonical duolingo issue (`05_SPA_HYDRATION_CLUSTER.md § 2.3 H1`).
- #3-7 are the masking sweep prerequisites — see `16_STEALTH_FINGERPRINT_AUDIT.md § 5`.
- #8 unblocks booking + several Akamai-instrumented React sites
  (`05_SPA_HYDRATION_CLUSTER.md § 3.2`).
- #10 reduces AWS WAF detection (`06_AWS_WAF_SOLVER.md`).
- #11 unblocks douyin (`05_SPA_HYDRATION_CLUSTER.md § 4.2 H1`).

---

## 4. How to add a new API — worked example: `MessageChannel` paired ports

This is the recommended workflow for adding any missing API. Pick MessageChannel
because it's the highest-leverage of the missing-implementation items (see #2 above).

### 4.1 Read the spec

- HTML Living Standard, "MessageChannel and MessagePort objects":
  https://html.spec.whatwg.org/multipage/web-messaging.html
- Key behaviors:
  - `new MessageChannel()` creates two `MessagePort` objects (`port1`, `port2`)
    that are entangled — every `port1.postMessage(msg)` schedules a `message`
    event on `port2` with the deserialized `msg`, and vice versa.
  - Messages are **queued** if the receiving port hasn't been "started" yet
    (started = `start()` was called OR `onmessage = fn` was assigned).
  - Messages are serialized via structured clone.
  - `port.close()` detaches the entanglement.

### 4.2 Locate the existing stub

`crates/js_runtime/src/js/window_bootstrap.js:2256-2272`:

```js
if (!globalThis.MessagePort) {
    globalThis.MessagePort = class MessagePort extends EventTarget {
        constructor() { super(); this.onmessage = null; this.onmessageerror = null; }
        postMessage() {}  // <-- NO-OP
        start() {}
        close() {}
    };
}
if (!globalThis.MessageChannel) {
    globalThis.MessageChannel = class MessageChannel {
        constructor() {
            this.port1 = new MessagePort();
            this.port2 = new MessagePort();
        }
    };
}
```

### 4.3 Design the real impl

Pure JS — no new ops needed. Pseudo-code:

```js
class MessagePort extends EventTarget {
    constructor() {
        super();
        this.onmessage = null;
        this.onmessageerror = null;
        this._peer = null;      // set by MessageChannel ctor
        this._started = false;  // explicit start() or onmessage= assignment
        this._queue = [];       // buffered messages pre-start
    }
    postMessage(msg, transferOrOptions) {
        if (!this._peer) return;
        // Structured-clone (reuse the global helper)
        let cloned;
        try { cloned = globalThis.structuredClone(msg); }
        catch (e) { /* fire messageerror on peer */ return; }
        const event = new MessageEvent('message', { data: cloned });
        if (this._peer._started) {
            // Schedule via microtask to match spec ordering
            Promise.resolve().then(() => this._peer.dispatchEvent(event));
        } else {
            this._peer._queue.push(event);
        }
    }
    start() {
        if (this._started) return;
        this._started = true;
        const drain = this._queue.splice(0);
        for (const ev of drain) {
            Promise.resolve().then(() => this.dispatchEvent(ev));
        }
    }
    close() {
        if (this._peer) this._peer._peer = null;
        this._peer = null;
    }
}
// Implicit start when onmessage is assigned (per HTML spec)
Object.defineProperty(MessagePort.prototype, 'onmessage', {
    get() { return this._onmessage || null; },
    set(fn) {
        this._onmessage = fn;
        if (typeof fn === 'function') this.start();
    },
    configurable: true,
});

class MessageChannel {
    constructor() {
        this.port1 = new MessagePort();
        this.port2 = new MessagePort();
        this.port1._peer = this.port2;
        this.port2._peer = this.port1;
    }
}
```

### 4.4 Wire EventTarget dispatch through `dispatchEvent`

The `dispatchEvent` already calls registered listeners. Make sure `onmessage`
is routed through dispatch by having the existing `_fireListeners` consult
`target['on' + type]` (it does — see `event_bootstrap.js:368-457`).

### 4.5 Mask the result

```js
_maskFunction(MessagePort, 'MessagePort');
_maskFunction(MessageChannel, 'MessageChannel');
_maskAsNative(MessagePort.prototype, 'postMessage', 'start', 'close');
```

(Already covered by the `dom_bootstrap.js:3050` sweep entry — verify after
landing.)

### 4.6 Add a test in `crates/browser/tests/chrome_compat.rs`

```rust
#[tokio::test(flavor = "current_thread")]
async fn message_channel_paired_ports() {
    let profile = stealth::presets::chrome_148_macos();
    let client = net::HttpClient::new().unwrap();
    let page = Page::build_page_with_scripts(
        r#"<!DOCTYPE html><html><body><div id="out"></div>
        <script>
            const ch = new MessageChannel();
            ch.port2.onmessage = (e) => {
                document.getElementById('out').textContent = 'GOT:' + e.data;
            };
            ch.port1.postMessage('hello');
        </script></body></html>"#,
        "https://example.com/",
        &profile,
        &client,
    ).await.unwrap();

    // Wait one microtask + one drain
    let _ = page.event_loop().run_until_idle(std::time::Duration::from_millis(100)).await;

    let content = page.content();
    assert!(content.contains("GOT:hello"), "MessagePort routing failed; body:\n{}", content);
}
```

Run: `cargo test --release -p browser message_channel_paired_ports -- --test-threads=1 --nocapture`.

### 4.7 Re-run the duolingo bench

```bash
cargo build --release -p browser --example sweep_metrics
target/release/examples/sweep_metrics chrome_148_macos /tmp/just_duolingo.json /tmp/d.json
jq '.results[] | select(.name=="duolingo") | {tag, len, ms}' /tmp/d.json
```

If `len > 50000` and `tag == "L3-RENDERED"`, the duolingo gate is flipped.

### 4.8 If an op IS needed (counter-example)

Not the case for MessageChannel (pure JS) but for completeness: if your new
API needs a Rust-side capability (e.g., `MediaStream` needs a real audio
source), the flow is:

1. Add the op signature in the right `crates/js_runtime/src/extensions/*_ext.rs`
   using the `#[op2]` macro. Take `#[state]` params for any cross-call state.
2. Register the op in the extension's `init_ops()` list in `mod.rs`.
3. Add the JS-side wrapper in the appropriate `*_bootstrap.js`.
4. If the op is async, mark `#[op2(async)]` and await it on the JS side.
5. The snapshot rebuilds on `cargo build` because each `include_str!` is a
   dep-tracked file.

---

## 5. Acceptance criteria for v0.1.0

- [ ] **Parity table populated** for at least the top 100 most-used Web APIs (§ 2 covers ~150).
- [ ] **`HTMLFormElement.elements` getter implemented** + `HTMLFormControlsCollection` returned with `namedItem(name)` matching by `el.name || el.id`. reddit gate flips.
- [ ] **`MessageChannel` + `MessagePort` paired-port routing implemented** with implicit-start on `onmessage = fn` and structured-clone serialization. duolingo gate flips.
- [ ] **5+ other high-impact missing APIs identified + tracked** as issues with `parity-v0.1.0` label. § 3 lists 20 candidates.
- [ ] **Per-spec-area health metric** in CI: number of `❌ MISSING` entries from § 2 must not grow.
- [ ] **The 20-row "gotcha list"** in § 3 is the active worklist; at minimum #1, #2, #3-7 land before v0.1.0.

---

## 6. Files referenced

| File:line | Purpose |
|---|---|
| `crates/js_runtime/src/snapshot.rs:67-91` | Bootstrap concatenation order |
| `crates/js_runtime/src/runtime.rs:300-345` | Worker isolate bootstrap order |
| `crates/js_runtime/src/js/interfaces_bootstrap.js:58` | The 600+ illegal-ctor stub list |
| `crates/js_runtime/src/js/interfaces_bootstrap.js:66-92` | Skip-list of real-impl-later interfaces |
| `crates/js_runtime/src/js/interfaces_bootstrap.js:158-200` | Window misc properties (external, personalbar, etc.) |
| `crates/js_runtime/src/js/dom_bootstrap.js:103-133` | DOM geometry helpers |
| `crates/js_runtime/src/js/dom_bootstrap.js:300-422` | NodeList, DOMTokenList |
| `crates/js_runtime/src/js/dom_bootstrap.js:426-575` | Node prototype |
| `crates/js_runtime/src/js/dom_bootstrap.js:583-635` | CSSStyleDeclaration proxy |
| `crates/js_runtime/src/js/dom_bootstrap.js:642-1024` | Element prototype + attachShadow |
| `crates/js_runtime/src/js/dom_bootstrap.js:1032-1205` | All HTML*Element subclasses |
| `crates/js_runtime/src/js/dom_bootstrap.js:1067-1110` | HTMLFormElement.submit/requestSubmit (reddit gate) |
| `crates/js_runtime/src/js/dom_bootstrap.js:1119-1149` | _reflectStr / _reflectBool helpers |
| `crates/js_runtime/src/js/dom_bootstrap.js:1265-1276` | Text + Comment |
| `crates/js_runtime/src/js/dom_bootstrap.js:1282-1303` | HTMLAllCollection |
| `crates/js_runtime/src/js/dom_bootstrap.js:1305-1566` | Document prototype |
| `crates/js_runtime/src/js/dom_bootstrap.js:1415-1420` | TreeWalker / NodeIterator (🟡 stubs) |
| `crates/js_runtime/src/js/dom_bootstrap.js:1567-1608` | CSSStyleSheet / CSSStyleRule |
| `crates/js_runtime/src/js/dom_bootstrap.js:1610-1672` | Range / Selection |
| `crates/js_runtime/src/js/dom_bootstrap.js:1796-1813` | DOMParser |
| `crates/js_runtime/src/js/dom_bootstrap.js:1814-2031` | MutationObserver |
| `crates/js_runtime/src/js/dom_bootstrap.js:2287-2931` | Iframe realm + remote realm |
| `crates/js_runtime/src/js/event_bootstrap.js:9-273` | All Event subclasses |
| `crates/js_runtime/src/js/event_bootstrap.js:276-461` | EventTarget dispatch core |
| `crates/js_runtime/src/js/fetch_bootstrap.js:4-140` | Headers / Response / Request |
| `crates/js_runtime/src/js/fetch_bootstrap.js:175+` | fetch() impl |
| `crates/js_runtime/src/js/timer_bootstrap.js:62-164` | setTimeout / rAF |
| `crates/js_runtime/src/js/timer_bootstrap.js:166-174` | performance.now fallback |
| `crates/js_runtime/src/js/window_bootstrap.js:46-67` | Window named ctor |
| `crates/js_runtime/src/js/window_bootstrap.js:171-174` | NetworkInformation (navigator.connection) |
| `crates/js_runtime/src/js/window_bootstrap.js:469-541` | Permissions / PermissionStatus |
| `crates/js_runtime/src/js/window_bootstrap.js:574-693` | WebAuthn / FedCM / Bluetooth |
| `crates/js_runtime/src/js/window_bootstrap.js:697-740` | USB / Serial / HID / Locks |
| `crates/js_runtime/src/js/window_bootstrap.js:777-809` | Keyboard / KeyboardLayoutMap |
| `crates/js_runtime/src/js/window_bootstrap.js:812-839` | StorageManager |
| `crates/js_runtime/src/js/window_bootstrap.js:842-889` | ServiceWorkerContainer |
| `crates/js_runtime/src/js/window_bootstrap.js:903-926` | Geolocation |
| `crates/js_runtime/src/js/window_bootstrap.js:1087-1157` | BatteryManager |
| `crates/js_runtime/src/js/window_bootstrap.js:1266-1413` | Location |
| `crates/js_runtime/src/js/window_bootstrap.js:1625-1697` | `chrome` object |
| `crates/js_runtime/src/js/window_bootstrap.js:1820-1845` | NavigatorUAData (ClientHints) |
| `crates/js_runtime/src/js/window_bootstrap.js:1879-2044` | Worker class |
| `crates/js_runtime/src/js/window_bootstrap.js:2065-2077` | ServiceWorker class |
| `crates/js_runtime/src/js/window_bootstrap.js:2078-2081` | WorkerGlobalScope / DedicatedWorkerGlobalScope |
| `crates/js_runtime/src/js/window_bootstrap.js:2095-2118` | FileReader (also `shared_apis_bootstrap.js:548-559`) |
| `crates/js_runtime/src/js/window_bootstrap.js:2120-2127` | ImageBitmap + createImageBitmap |
| `crates/js_runtime/src/js/window_bootstrap.js:2131-2170` | DOMPoint / DOMMatrix |
| `crates/js_runtime/src/js/window_bootstrap.js:2183-2210` | PerformanceObserver / ReportingObserver |
| `crates/js_runtime/src/js/window_bootstrap.js:2220-2253` | ReadableStream / Writable / Transform / BroadcastChannel |
| `crates/js_runtime/src/js/window_bootstrap.js:2256-2271` | MessagePort / MessageChannel (**duolingo**) |
| `crates/js_runtime/src/js/window_bootstrap.js:2274-2290` | EventSource stub (replaced by sse_bootstrap.js) |
| `crates/js_runtime/src/js/window_bootstrap.js:2293-2308` | CompressionStream / DecompressionStream |
| `crates/js_runtime/src/js/window_bootstrap.js:2312-2330` | SpeechSynthesis |
| `crates/js_runtime/src/js/window_bootstrap.js:2599-2885` | performance metrics + entries |
| `crates/js_runtime/src/js/window_bootstrap.js:2941-3020` | Crypto / SubtleCrypto |
| `crates/js_runtime/src/js/window_bootstrap.js:3041-3165` | TextEncoder / TextDecoder |
| `crates/js_runtime/src/js/window_bootstrap.js:3262-3302` | Storage (localStorage/sessionStorage) |
| `crates/js_runtime/src/js/window_bootstrap.js:3315-3376` | IntersectionObserver / ResizeObserver |
| `crates/js_runtime/src/js/window_bootstrap.js:3379-3391` | requestIdleCallback |
| `crates/js_runtime/src/js/window_bootstrap.js:3403-3460` | getComputedStyle |
| `crates/js_runtime/src/js/window_bootstrap.js:3465-3683` | XMLHttpRequest |
| `crates/js_runtime/src/js/window_bootstrap.js:3686-3768` | WebSocket + CloseEvent |
| `crates/js_runtime/src/js/window_bootstrap.js:3793` | History |
| `crates/js_runtime/src/js/window_bootstrap.js:3987-4015` | matchMedia / MediaQueryList |
| `crates/js_runtime/src/js/window_bootstrap.js:4066-4163` | AbortController / DOMException / URLSearchParams / URL |
| `crates/js_runtime/src/js/window_bootstrap.js:4241-4317` | FormData / customElements |
| `crates/js_runtime/src/js/window_bootstrap.js:4324-4925` | Blob / OffscreenCanvas / File / IndexedDB (window-side) |
| `crates/js_runtime/src/js/window_bootstrap.js:4931-5019` | RTCPeerConnection / Session / Ice / DataChannel |
| `crates/js_runtime/src/js/window_bootstrap.js:5035` | FontFace |
| `crates/js_runtime/src/js/window_bootstrap.js:5176-5189` | HTMLMediaElement.canPlayType |
| `crates/js_runtime/src/js/window_bootstrap.js:5226-5257` | PressureObserver / PressureRecord |
| `crates/js_runtime/src/js/window_bootstrap.js:5275-5290` | DocumentPictureInPicture |
| `crates/js_runtime/src/js/window_bootstrap.js:5304-5315` | UserActivation |
| `crates/js_runtime/src/js/window_bootstrap.js:5473-5529` | SourceBufferList / MediaSource / MediaRecorder |
| `crates/js_runtime/src/js/window_bootstrap.js:5570-5611` | VisualViewport / InputDeviceCapabilities |
| `crates/js_runtime/src/js/window_bootstrap.js:5622-5665` | MediaMetadata / MediaSession |
| `crates/js_runtime/src/js/window_bootstrap.js:5735-5779` | MediaCapabilities |
| `crates/js_runtime/src/js/window_bootstrap.js:5914-5938` | trustedTypes |
| `crates/js_runtime/src/js/window_bootstrap.js:5948-5976` | scheduler / reportError |
| `crates/js_runtime/src/js/window_bootstrap.js:5985-6031` | Touch / TouchEvent / TouchList |
| `crates/js_runtime/src/js/window_bootstrap.js:6066-6086` | CacheStorage |
| `crates/js_runtime/src/js/window_bootstrap.js:6107-6143` | CookieStore |
| `crates/js_runtime/src/js/window_bootstrap.js:6145-6159` | EventCounts |
| `crates/js_runtime/src/js/window_bootstrap.js:6196-6252` | Notification |
| `crates/js_runtime/src/js/window_bootstrap.js:6263-6266` | ApplePaySession (iphone profile) |
| `crates/js_runtime/src/js/window_bootstrap.js:6279-6298` | IdleDetector |
| `crates/js_runtime/src/js/window_bootstrap.js:6305-6314` | EyeDropper |
| `crates/js_runtime/src/js/window_bootstrap.js:6320-6390` | VirtualKeyboard / DevicePosture / WindowControlsOverlay |
| `crates/js_runtime/src/js/window_bootstrap.js:6403-6430` | ViewTransition |
| `crates/js_runtime/src/js/window_bootstrap.js:6496-6605` | PaymentRequest family |
| `crates/js_runtime/src/js/canvas_bootstrap.js:102-115` | ImageData |
| `crates/js_runtime/src/js/canvas_bootstrap.js:118-263` | CanvasRenderingContext2D |
| `crates/js_runtime/src/js/canvas_bootstrap.js:266-585` | WebGLRenderingContext |
| `crates/js_runtime/src/js/canvas_bootstrap.js:585-1014` | Web Audio (AudioNode family + AudioContext / OfflineAudioContext) |
| `crates/js_runtime/src/js/canvas_bootstrap.js:887-981` | HTMLCanvasElement |
| `crates/js_runtime/src/js/canvas_bootstrap.js:1166-1216` | RealOffscreenCanvas |
| `crates/js_runtime/src/js/shared_apis_bootstrap.js:39-49` | DOMException |
| `crates/js_runtime/src/js/shared_apis_bootstrap.js:55-90` | atob / btoa |
| `crates/js_runtime/src/js/shared_apis_bootstrap.js:95-150` | Crypto / SubtleCrypto (worker-side mirror) |
| `crates/js_runtime/src/js/shared_apis_bootstrap.js:153-224` | TextEncoder / TextDecoder (worker-side mirror) |
| `crates/js_runtime/src/js/shared_apis_bootstrap.js:240-303` | URL / URLSearchParams (worker + main) |
| `crates/js_runtime/src/js/shared_apis_bootstrap.js:315-348` | Blob / File |
| `crates/js_runtime/src/js/shared_apis_bootstrap.js:354-369` | FormData |
| `crates/js_runtime/src/js/shared_apis_bootstrap.js:376-402` | AbortController / AbortSignal |
| `crates/js_runtime/src/js/shared_apis_bootstrap.js:408-418` | OffscreenCanvas (shared) |
| `crates/js_runtime/src/js/shared_apis_bootstrap.js:423-543` | IndexedDB (shared) |
| `crates/js_runtime/src/js/shared_apis_bootstrap.js:548-592` | FileReader / ImageBitmap / DOMPoint / DOMMatrix (shared) |
| `crates/js_runtime/src/js/streams_bootstrap.js:31-587` | All stream classes |
| `crates/js_runtime/src/js/structured_clone.js` | structuredClone polyfill + guards |
| `crates/js_runtime/src/js/worker_bootstrap.js:55-128` | WorkerNavigator + WorkerNavigatorUAData |
| `crates/js_runtime/src/js/worker_bootstrap.js:132-153` | Worker performance.now + performance.memory |
| `crates/js_runtime/src/js/worker_bootstrap.js:156-189` | self.postMessage |
| `crates/js_runtime/src/js/worker_bootstrap.js:198-229` | drainOnce parent→worker message pump |
| `crates/js_runtime/src/js/worker_bootstrap.js:232-256` | importScripts |
| `crates/js_runtime/src/js/worker_bootstrap.js:263-290` | Worker-side MediaSource / MediaRecorder.isTypeSupported |
| `crates/js_runtime/src/js/sse_bootstrap.js` | EventSource real impl |
| `crates/js_runtime/src/extensions/*.rs` | Per-area Rust ops (dom_ext 1448 LOC, fetch_ext 708, canvas_ext 678, audio_ext 648, worker_ext 531, webgl_ext 331, sse_ext 224, perf_ext 204, stealth_ext 186, websocket_ext 161, layout_ext 123, input_ext 89, timer_ext 83, nav_ext 48, crypto_ext 47) |
| `05_SPA_HYDRATION_CLUSTER.md § 1.2 H2` | reddit HTMLFormElement.elements blocker |
| `05_SPA_HYDRATION_CLUSTER.md § 2.3 H1` | duolingo MessageChannel blocker |
| `16_STEALTH_FINGERPRINT_AUDIT.md § 5` | Mask-sweep for items #3-7 in § 3 above |

---

## 7. Sequencing within v0.1.0

The parity-table maintenance is structural (lives alongside the codebase). The
work driven by this doc breaks into:

### 7.1 Immediate (Phase 1, 1 week)

- **#1 `HTMLFormElement.elements` + `HTMLFormControlsCollection`** → flips reddit
- **#2 `MessageChannel` paired ports** → flips duolingo
- Verify each fix with the per-site bench in `05_SPA_HYDRATION_CLUSTER.md § 6`
  and the full 4-profile sweep

### 7.2 Mid-term (Phase 2, parallel with chapter 06/07, 1-2 weeks)

- **#8 IntersectionObserver real bounding rect** → may flip booking
- **#10 performance.now() sub-ms** → reduces AWS WAF detection
- **#20 NamedNodeMap** → reduces Akamai sensor weight

### 7.3 Long tail (post-v0.1.0)

- #11 `crypto.subtle.sign/verify` real impl → unblocks douyin + payments
- #9 `MutationObserver` attribute notify → reduces Akamai detection
- #19 `Animation` real promise resolution → niche
- Everything else in § 3 ranked P3

### 7.4 Continuous

- **Run the audit-script-equivalent** for this doc: a test that enumerates
  the table in § 2 and asserts each entry's reported status matches reality.
  Suggested home: `crates/browser/tests/chrome_compat.rs::web_api_parity_audit`.
  The same script can produce a Markdown diff against the current table on each
  failure, so the table doesn't drift.

