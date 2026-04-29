# Chrome Fingerprint Full Inventory — Web Platform Parity Catalogue (2026-04-29)

**Scope**: Public-source-only inventory of every fingerprintable Web Platform property real Chrome 147 (macOS arm64) exposes, what `browser_oxide` emits today, the gap, and a concrete fix recommendation per property. Same engineering discipline as Mozilla Web Platform Tests, Playwright stealth patches, and Camoufox MaskConfig — applied to a from-scratch engine with full control of its V8 + custom-DOM stack.

**Strict policy boundary**: This document only catalogues public Web Platform APIs (MDN / W3C / WHATWG / Chromium source / FingerprintJS open-source / CreepJS / BrowserLeaks / ScrapFly defensive write-ups). Nothing here describes encrypted-payload reverse engineering, HMAC token forging, or how to defeat a server-side security check. Where Chrome's value can only be observed by running a live Chrome instance, the entry is marked **UNKNOWN** with the closest published source cited.

**Mark legend**:
- **MATCH** — current shim returns the documented Chrome value
- **PARTIAL** — present but shape/value drifts from Chrome
- **GAP** — surface absent or returns a non-Chrome shape
- **UNKNOWN** — published source insufficient; needs runtime probe to lock down
- **N/A** — out of scope (server-side, hardware-bound, security policy)

**Already-shipped** (verified during audit, for context):
- `BatteryManager` is now a real class (`window_bootstrap.js:725-748`) — closes the prior `bt` field gap.
- `MediaSession` is now a real class with `playbackState`, `metadata`, `setActionHandler`, `setPositionState` (`window_bootstrap.js:4168-4226`).
- mDNS-anonymized ICE host candidate now emitted (`window_bootstrap.js:3825-3858`).
- `OfflineAudioContext` shim with deterministic seeded buffer (`canvas_bootstrap.js:541-895`).
- `Storage.estimate()` now ships `usageDetails` shape (`window_bootstrap.js:560-572`).
- 36-extension WebGL parity, OSMesa line-width fix, Tahoma+font-stack canvas parity (per recent commits).

The 2026-04-29 doc `CHROME_JS_SURFACE_PARITY_2026_04_29.md` covered 24 surfaces. **This doc adds ≈110 more rows** organized into the 15 categories from the task brief.

---

## 1. Window state

### `window.innerWidth` / `innerHeight`

**Spec**: [CSSOM View](https://drafts.csswg.org/cssom-view/#dom-window-innerwidth) · [MDN innerWidth](https://developer.mozilla.org/en-US/docs/Web/API/Window/innerWidth)

**Real Chrome 147 (macOS arm64, default window)**: integer CSS-pixel viewport size; for the macOS Playwright default `1280×720` window: `innerWidth=1280`, `innerHeight=720` (browser chrome subtracted from outer height). Property is non-writable, configurable, on `Window.prototype` getter.

**browser_oxide**: **MATCH** — `window_bootstrap.js:1005-1006` defines as configurable getters reading `inner_width`/`inner_height` profile keys (default 1920/1080). Note: the macOS profile in `presets.rs:97-100` ships `inner_height: 969` which is plausible for `outer_height: 1080` (108 px chrome+toolbar+tab). **Action**: validate against a real Chrome 147 macOS measurement; the canonical default for Playwright is 720, our default is 1080.

### `window.outerWidth` / `outerHeight`

**Spec**: [CSSOM View outerWidth](https://drafts.csswg.org/cssom-view/#dom-window-outerwidth) · [MDN](https://developer.mozilla.org/en-US/docs/Web/API/Window/outerWidth)

**Real Chrome**: outer dimensions of the OS window; equals screen.width for fullscreen, otherwise window-frame inclusive. On a 1920×1080 desktop with Chrome maximised, ≈1920×1040 (taskbar) on Windows; 1920×1080 on macOS in fullscreen.

**browser_oxide**: **MATCH** — `window_bootstrap.js:1007-1008`. Profile-driven.

### `window.screenX` / `screenY` / `screenLeft` / `screenTop`

**Spec**: [CSSOM View screenX](https://drafts.csswg.org/cssom-view/#dom-window-screenx) · [MDN](https://developer.mozilla.org/en-US/docs/Web/API/Window/screenX)

**Real Chrome**: integer pixel offset of the window's top-left corner from the OS screen origin. Rarely 0 except in fullscreen kiosk/headless. For headed Chrome on macOS arm64 the menubar pushes y to ≥25.

**browser_oxide**: **PARTIAL** — `window_bootstrap.js:1010-1011` hardcodes `screenX=0, screenY=0`. Aliases `screenLeft`/`screenTop` are not defined. **Fix**: add `screenLeft`/`screenTop` as aliases; randomise to ≥25 on macOS profiles via profile keys `window_screen_x`, `window_screen_y` so headless != y=0 isn't a tell.

### `window.scrollX` / `scrollY` / `pageXOffset` / `pageYOffset`

**Spec**: [CSSOM View scrollX](https://drafts.csswg.org/cssom-view/#dom-window-scrollx) · [MDN](https://developer.mozilla.org/en-US/docs/Web/API/Window/scrollX)

**Real Chrome**: doubles, default `0` on a fresh page. `pageXOffset`/`pageYOffset` are legacy aliases that **must** equal `scrollX`/`scrollY` byte-for-byte (CreepJS lie-test).

**browser_oxide**: **MATCH** — `window_bootstrap.js:1012-1015`, kept in sync by `scrollTo`/`scrollBy` patches at lines 1019-1035. Note: these are **own properties**, not prototype getters — Chrome puts them on `Window.prototype` as accessors. **Fix**: convert to `Object.defineProperty(globalThis, 'scrollX', { get })`.

### `window.devicePixelRatio`

**Spec**: [CSSOM View devicePixelRatio](https://drafts.csswg.org/cssom-view/#dom-window-devicepixelratio) · [MDN](https://developer.mozilla.org/en-US/docs/Web/API/Window/devicePixelRatio)

**Real Chrome 147 macOS arm64**: `2` for Retina (M1/M2/M3 MacBooks), `3` for iPhone Mirror, `1` for non-Retina externals. Windows scaling 100%→1, 125%→1.25, 150%→1.5, 200%→2.

**browser_oxide**: **MATCH** — `window_bootstrap.js:1009, 1144`. macOS profile sets to 2.0 (verified `presets.rs`).

### `window.visualViewport`

**Spec**: [CSSOM View VisualViewport](https://drafts.csswg.org/cssom-view/#visualviewport) · [MDN](https://developer.mozilla.org/en-US/docs/Web/API/VisualViewport)

**Real Chrome**: `VisualViewport` instance with `width`, `height`, `pageLeft=0`, `pageTop=0`, `offsetLeft=0`, `offsetTop=0`, `scale=1`, EventTarget interface.

**browser_oxide**: **MATCH** — `window_bootstrap.js:4117-4143` ships full implementation per the existing parity doc.

### `window.matchMedia(...)` — media query evaluation

**Spec**: [CSSOM View MediaQueryList](https://drafts.csswg.org/cssom-view/#mediaquerylist) · [MDN matchMedia](https://developer.mozilla.org/en-US/docs/Web/API/Window/matchMedia)

**Real Chrome 147 macOS arm64 default**:
- `prefers-color-scheme: light` → matches=true (system Dark mode flips this)
- `prefers-color-scheme: dark` → matches=false (default)
- `prefers-reduced-motion: no-preference` → true
- `prefers-reduced-motion: reduce` → false
- `prefers-contrast: no-preference` → true
- `prefers-reduced-data: no-preference` → true (a11y default)
- `pointer: fine` → true (mouse), `pointer: coarse` → false on desktop
- `hover: hover` → true on desktop, false on touchscreen
- `(min-width: 0)` → always true; `(width: 1280px)` → matches viewport
- `(orientation: landscape)` → true when innerWidth ≥ innerHeight
- `(forced-colors: none)` → true (no high-contrast mode)
- `(any-pointer: fine)` → true on desktop

`MediaQueryList.prototype.constructor.name === "MediaQueryList"`. `addEventListener('change', ...)` fires when system pref changes.

**browser_oxide**: **PARTIAL** — `window_bootstrap.js:2892-2905` handles `prefers-color-scheme: light`, `prefers-reduced-motion: no-preference`, and `(min-width: ...)` regex. Missing: `prefers-contrast`, `prefers-reduced-data`, `pointer:`, `hover:`, `forced-colors:`, `any-pointer:`, `orientation:`, `(width:` exact, `dynamic-range:`. **Fix**: rewrite as a proper media-query lexer that consults profile keys `prefers_color_scheme`, `prefers_reduced_motion`, `prefers_contrast`, `pointer_type`, `hover_capability`, `forced_colors`. Profile keys `prefers_color_scheme`/`pointer_type`/`hover_capability` already exist in `presets.rs:93-95`.

### Window load-sequence events: `DOMContentLoaded`, `load`, `pageshow`

**Spec**: [HTML Living Standard Document load events](https://html.spec.whatwg.org/multipage/parsing.html#the-end) · [MDN PageTransitionEvent](https://developer.mozilla.org/en-US/docs/Web/API/PageTransitionEvent)

**Real Chrome 147**: order is strictly `readystatechange→interactive`, `DOMContentLoaded` (bubbles, non-cancelable), `readystatechange→complete`, `load` (window only, non-bubbling), `pageshow` (window only, `persisted: false` on first load, `true` on bfcache).

**browser_oxide**: **PARTIAL** — `crates/browser/src/page.rs:313-320, 1961-1969` fires `DOMContentLoaded` on document and on window. Missing: `pageshow` event, `readystatechange` transitions to `interactive` before DCL, `load` event ordering. **Fix**: add `pageshow` dispatch with `PageTransitionEvent.persisted=false`; emit `readystatechange` transitions in correct order.

### **Top fixes for category 1**

1. Convert `screenX/Y/scrollX/Y/pageX/YOffset` from own properties to `Window.prototype` accessor getters (so `Object.getOwnPropertyDescriptor(window, 'scrollX')` returns `undefined` like real Chrome). Lock-in test: `crates/browser/tests/perimeterx_surface_parity.rs` add a "window own descriptor" assertion. *Helps: every site that reads window descriptors via FingerprintJS v4.*
2. Fully implement `matchMedia` lexer for the 12 standard media features. Lock-in: new test `media_queries_match_chrome.rs` enumerating all 12. *Helps: any site that uses `prefers-color-scheme` for fingerprint hashing — CreepJS, FingerprintJS, ScrapFly PX guide.*
3. Emit `pageshow` and `readystatechange→interactive` in the load sequence. *Helps: sites that gate behaviour on the bfcache hint, plus Akamai's `lc` field which timestamps each event.*

---

## 2. Document state

### `document.visibilityState` / `document.hidden`

**Spec**: [Page Visibility](https://w3c.github.io/page-visibility/) · [MDN visibilityState](https://developer.mozilla.org/en-US/docs/Web/API/Document/visibilityState)

**Real Chrome**: `"visible"`/`false` for foregrounded tab; `"hidden"`/`true` when tab is in background or window minimised.

**browser_oxide**: **MATCH** — `dom_bootstrap.js:1232-1233, 1346-1347` and `window_bootstrap.js:1109-1110`. Returns `visible/false` always (correct for "user is on this page").

### `document.hasFocus()`

**Spec**: [HTML Document.hasFocus](https://html.spec.whatwg.org/multipage/interaction.html#dom-document-hasfocus) · [MDN](https://developer.mozilla.org/en-US/docs/Web/API/Document/hasFocus)

**Real Chrome**: `true` when the tab is focused, `false` otherwise (including immediately on page load before user interaction in some headless modes).

**browser_oxide**: **MATCH** — `dom_bootstrap.js:1338` returns `true`. Headed-Chrome equivalent.

### `document.referrer`

**Spec**: [HTML Document.referrer](https://html.spec.whatwg.org/multipage/dom.html#dom-document-referrer) · [MDN](https://developer.mozilla.org/en-US/docs/Web/API/Document/referrer)

**Real Chrome**: empty string when no referrer; full URL string when navigated from another page (subject to Referrer-Policy header).

**browser_oxide**: **PARTIAL** — `dom_bootstrap.js:1345` always returns `""`. **Fix**: wire to `Document::referrer` in Rust, populated from the navigation's `Referer` request header.

### `document.URL` / `document.documentURI` / `document.baseURI`

**Spec**: [DOM Living Standard](https://dom.spec.whatwg.org/#dom-document-url)

**Real Chrome**: all three return the document URL; `baseURI` honors `<base href>` element.

**browser_oxide**: **UNKNOWN** — not found in `dom_bootstrap.js` grep. May be set elsewhere (Rust-side Document field). **Fix**: verify and add explicit prototype getter if missing.

### `document.domain`

**Spec**: [HTML Document.domain](https://html.spec.whatwg.org/multipage/origin.html#dom-document-domain) · [MDN](https://developer.mozilla.org/en-US/docs/Web/API/Document/domain)

**Real Chrome 147**: returns the document's effective domain (eTLD+1 by default unless explicitly set). Setter is opt-in to relaxed-same-origin and **deprecated** as of Chrome 109+ (must enable a Permissions-Policy).

**browser_oxide**: **UNKNOWN** — not surfaced in shim grep. **Fix**: add `Document.prototype.domain` accessor returning the document URL's hostname.

### `document.cookie`

**Spec**: [HTML Document.cookie](https://html.spec.whatwg.org/multipage/dom.html#dom-document-cookie)

**Real Chrome**: live string view of the cookie jar for the document's origin, filtered by Secure/HttpOnly/SameSite/Path/Domain. Setter parses `Set-Cookie` syntax.

**browser_oxide**: **MATCH** — `dom_bootstrap.js:1348-1383` is wired to the unified cookie jar via `op_cookie_set`/`op_cookies_for_url`. `max-age=0` deletion path implemented.

### `document.compatMode`

**Spec**: [DOM Living Standard compatMode](https://dom.spec.whatwg.org/#dom-document-compatmode)

**Real Chrome**: `"CSS1Compat"` for HTML5 doctype documents; `"BackCompat"` for quirks mode.

**browser_oxide**: **MATCH** — `dom_bootstrap.js:1386` returns `"CSS1Compat"`. Matches default. **Note**: should switch to `"BackCompat"` if the doctype is missing/quirks; not handled today.

### `document.documentElement.clientWidth/clientHeight`

**Spec**: [CSSOM View clientWidth](https://drafts.csswg.org/cssom-view/#dom-element-clientwidth)

**Real Chrome**: viewport CSS-pixel width/height (without scrollbars); in headless Chrome scrollbars are 0px so clientWidth==innerWidth.

**browser_oxide**: **MATCH** — `dom_bootstrap.js:1606-1607` wires `HTMLHtmlElement.prototype.clientWidth/Height` to `_viewportW()`/`_viewportH()`. Generic Element fallback at line 645-648 returns `offsetWidth/Height` which may be 0 for elements that haven't laid out — acceptable since the layout-engine work is separate.

### `document.documentElement.scrollWidth/scrollHeight`

**Spec**: [CSSOM View scrollWidth](https://drafts.csswg.org/cssom-view/#dom-element-scrollwidth)

**Real Chrome**: total content width/height including overflow.

**browser_oxide**: **PARTIAL** — `dom_bootstrap.js:647-648` returns `offsetWidth/offsetHeight`. For `documentElement`, real Chrome returns `max(viewport, content)`. **Fix**: layout-bound; track open task #66 (real layout).

### `document.activeElement`

**Spec**: [HTML Document.activeElement](https://html.spec.whatwg.org/multipage/interaction.html#dom-document-activeelement)

**Real Chrome**: returns the currently focused element; defaults to `<body>` on page load before any focus.

**browser_oxide**: **MATCH** — `dom_bootstrap.js:1419` returns `this.body`.

### `document.readyState`

**Spec**: [HTML Document.readyState](https://html.spec.whatwg.org/multipage/dom.html#current-document-readiness)

**Real Chrome**: transitions `loading` → `interactive` (at DCL) → `complete` (at load).

**browser_oxide**: **UNKNOWN** — needs verification. The page.rs flow fires DCL but readyState transitions are not visible in the grep. **Fix**: ensure Document.prototype.readyState getter reads a state set by the loader.

### `document.title`

**Spec**: [HTML Document.title](https://html.spec.whatwg.org/multipage/dom.html#document.title)

**browser_oxide**: **UNKNOWN** — verify wired via `<title>` element.

### **Top fixes for category 2**

1. Wire `document.referrer` to the actual `Referer` request header. *Helps: any redirect-chain analytics; Akamai BMP `rt` field; CreepJS referrer cross-check.*
2. Add `document.domain` getter from URL hostname. *Helps: any same-origin/eTLD+1 logic site.*
3. Confirm `document.readyState` transitions through `loading`/`interactive`/`complete`. Lock-in test: synchronously snapshot readyState at three points. *Helps: Akamai's `lc` event timing.*

---

## 3. Screen

### `screen.width` / `screen.height`

**Spec**: [CSSOM View Screen](https://drafts.csswg.org/cssom-view/#the-screen-interface) · [MDN](https://developer.mozilla.org/en-US/docs/Web/API/Screen)

**Real Chrome 147 macOS arm64 (M3 MacBook Pro 14")**: `screen.width=1512`, `screen.height=982`. For an external 1920×1080 monitor: `1920×1080`. For a 5K iMac: `5120×2880`.

**browser_oxide**: **MATCH** — `window_bootstrap.js:976-977` reads `screen_width`/`screen_height` profile keys.

### `screen.availWidth` / `screen.availHeight`

**Real Chrome macOS**: `availWidth=screen.width`, `availHeight=screen.height-25` (menubar). Windows: `availHeight=screen.height-40` (taskbar default).

**browser_oxide**: **MATCH** — `window_bootstrap.js:978-979`. Profile defaults `1920/1040` correctly subtract a Windows taskbar.

### `screen.availLeft` / `screen.availTop`

**Real Chrome**: `availLeft=0` (single monitor); `availTop=25` on macOS, `0` on Windows.

**browser_oxide**: **PARTIAL** — `availTop` profile key exists (`presets.rs:57: screen_avail_top: 0`) but `availLeft` doesn't appear in the shim. Default 0 is correct on Windows but NOT on macOS. **Fix**: add `screen.availLeft` getter and set `screen_avail_top: 25` for macOS profile.

### `screen.colorDepth` / `screen.pixelDepth`

**Real Chrome**: both always `24` on every platform (privacy-frozen). Even HDR displays return 24.

**browser_oxide**: **MATCH** — `window_bootstrap.js:982-983, 1140-1141`.

### `screen.orientation.angle` / `.type`

**Spec**: [W3C Screen Orientation](https://w3c.github.io/screen-orientation/) · [MDN](https://developer.mozilla.org/en-US/docs/Web/API/ScreenOrientation)

**Real Chrome desktop**: `angle: 0`, `type: "landscape-primary"` typically. iPhone: `0`/`portrait-primary`.

**browser_oxide**: **MATCH** — `window_bootstrap.js:984, 974-987`. ScreenOrientation prototype with `Symbol.toStringTag` set.

### `screen.isExtended` (Multi-Screen Window Placement API)

**Spec**: [W3C Multi-Screen](https://w3c.github.io/window-placement/) · [MDN](https://developer.mozilla.org/en-US/docs/Web/API/Screen/isExtended)

**Real Chrome 100+**: boolean; `false` on single-monitor setups, `true` if user has multiple displays AND has granted the "window-management" permission.

**browser_oxide**: **MATCH** — `window_bootstrap.js:985` returns `false`.

### `screen.lockOrientation` / `unlockOrientation`

**Spec**: deprecated, removed from Chrome 38. The replacement is `screen.orientation.lock()/unlock()`.

**browser_oxide**: **MATCH** — these legacy methods are correctly absent. `screen.orientation.lock` is the new entry point (UNKNOWN whether stubbed; check next).

### `screen.orientation.lock()` / `unlock()`

**Real Chrome**: `lock(orientation)` returns Promise that rejects with `NotSupportedError` outside fullscreen.

**browser_oxide**: **UNKNOWN** — not visible in grep. **Fix**: stub on `ScreenOrientation.prototype` returning `Promise.reject(NotSupportedError)`.

### **Top fixes for category 3**

1. Add `screen.availLeft` and bump macOS profile's `screen_avail_top` to 25. *Helps: every CreepJS run, every Akamai BMP `sr` field encoder.*
2. Stub `screen.orientation.lock`/`unlock`. *Helps: prototype-presence probes (FingerprintJS).*

---

## 4. Navigator full surface (the big one)

### `navigator.userAgent`

**Spec**: [HTML NavigatorID.userAgent](https://html.spec.whatwg.org/multipage/system-state.html#dom-navigator-useragent)

**Real Chrome 147 macOS arm64 (UA-reduction frozen)**: `Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36` — **note** Mac OS X version is permanently `10_15_7`, "Intel" lies even on arm64, minor.build.patch is always `0.0.0` per UA-reduction since Chrome 110.

**browser_oxide**: **MATCH** — `window_bootstrap.js:631`, profile-driven via `user_agent` key. `presets.rs` macOS profile correctly reports `Chrome/147.0.0.0` not `147.0.7727.117`.

### `navigator.appName` / `appCodeName` / `appVersion` / `product` / `productSub`

**Real Chrome (every platform, frozen)**: `appName="Netscape"`, `appCodeName="Mozilla"`, `appVersion=userAgent.slice(8)`, `product="Gecko"`, `productSub="20030107"`.

**browser_oxide**: **MATCH** — `window_bootstrap.js:635-639`. `productSub` is the canonical `"20030107"`.

### `navigator.vendor` / `vendorSub`

**Real Chrome**: `vendor="Google Inc."`, `vendorSub=""`.

**browser_oxide**: **MATCH** — `window_bootstrap.js:633-634`.

### `navigator.buildID`

**Real Chrome**: `undefined` (Firefox-only field). Probing `'buildID' in navigator` returning `true` is a Firefox-impersonation tell.

**browser_oxide**: **MATCH** — not defined; correctly absent.

### `navigator.oscpu`

**Real Chrome**: `undefined` (Firefox-only field).

**browser_oxide**: **MATCH** — not defined.

### `navigator.platform`

**Real Chrome 147 macOS arm64**: `"MacIntel"` (NOT `"MacARM"` — frozen pre-arm64). Windows: `"Win32"` (even on Win64). Linux: `"Linux x86_64"`.

**browser_oxide**: **MATCH** — `window_bootstrap.js:632`, profile macOS sets `platform="MacIntel"` (verified).

### `navigator.userAgentData.brands`

**Spec**: [W3C UA Client Hints](https://wicg.github.io/ua-client-hints/) · [MDN NavigatorUAData](https://developer.mozilla.org/en-US/docs/Web/API/NavigatorUAData)

**Real Chrome 147**: 3-entry frozen array `[{brand:"Not.A/Brand",version:"24"}, {brand:"Chromium",version:"147"}, {brand:"Google Chrome",version:"147"}]`. GREASE entry order rotates per process.

**browser_oxide**: **PARTIAL** — `window_bootstrap.js:1128-1253`. Existing parity doc notes our literal is `"Not-A.Brand"` but Chrome's current is `"Not.A/Brand"` — minor. **Fix**: change literal.

### `navigator.userAgentData.{mobile, platform}`

**Real Chrome**: `mobile=false` (Windows/macOS/Linux desktop), `platform: "Windows"|"macOS"|"Linux"|"Android"|"iOS"`.

**browser_oxide**: **MATCH** — `window_bootstrap.js:1242, 1265`.

### `navigator.userAgentData.getHighEntropyValues(...)`

**Real Chrome**: returns Promise resolving to `{architecture, bitness, model, platformVersion, uaFullVersion, fullVersionList, wow64}`. macOS arm64: `architecture:"arm"`, `bitness:"64"`, `platformVersion:"15.0.0"`, `model:""`, `uaFullVersion:"147.0.7727.117"`, `wow64:false`.

**browser_oxide**: **MATCH** — `window_bootstrap.js:1235-1281`. Per-key dispatch matches.

### `navigator.language` / `navigator.languages`

**Real Chrome**: `language="en-US"`, `languages=["en-US","en"]` (frozen array reference; same identity across reads).

**browser_oxide**: **MATCH** — `window_bootstrap.js:618-625, 640-641` caches frozen array.

### `navigator.systemLanguage` / `userLanguage`

**Real Chrome**: `undefined` (IE-only). Probing presence is an IE impersonation tell.

**browser_oxide**: **MATCH** — not defined.

### `navigator.cookieEnabled`

**Real Chrome**: `true` unless 3rd-party cookies blocked AND probed from cross-origin iframe.

**browser_oxide**: **MATCH** — `window_bootstrap.js:643`.

### `navigator.doNotTrack` / `msDoNotTrack`

**Real Chrome**: `doNotTrack=null` by default (the user has not set DNT). `msDoNotTrack=undefined`. Setting DNT is a deprecated browser pref and rarely toggled.

**browser_oxide**: **MATCH** — `window_bootstrap.js:649`.

### `navigator.webdriver`

**Real Chrome (headed)**: `false`. Real Chrome (headless `--enable-automation`): `true`. The single most-reliable bot tell.

**browser_oxide**: **MATCH** — `window_bootstrap.js:648, 1120-1124` defines as non-writable, non-configurable per Chrome's actual descriptor.

### `navigator.hardwareConcurrency`

**Real Chrome**: integer in {2, 4, 6, 8, 12, 16, 24, 32}. Capped at 64 by Chrome to bucket. macOS M3 Pro: 12.

**browser_oxide**: **MATCH** — `window_bootstrap.js:644`. Default 8.

### `navigator.deviceMemory`

**Real Chrome**: privacy-bucketed value from {0.25, 0.5, 1, 2, 4, 8}. macOS MacBooks (≥8 GB) report 8.

**browser_oxide**: **MATCH** — `window_bootstrap.js:645`.

### `navigator.maxTouchPoints`

**Real Chrome**: 0 on non-touch desktops; 5 (capped) on Windows tablets / Surface; 10 on iPads (Safari only).

**browser_oxide**: **MATCH** — `window_bootstrap.js:646`.

### `navigator.geolocation.{getCurrentPosition,watchPosition,clearWatch}`

**Real Chrome**: `Geolocation` prototype with three methods. Without permission, `getCurrentPosition(_, err)` calls err with `PositionError{code:1, message:"User denied Geolocation"}` after a delay.

**browser_oxide**: **PARTIAL** — `_navGeolocation` referenced at `window_bootstrap.js:670`. Need to verify it has all three methods and dispatches the correct error. **Fix**: ensure the rejection error has `code:1`, message text matching Chrome.

### `navigator.mediaDevices.enumerateDevices()`

**Spec**: [W3C MediaCapture](https://w3c.github.io/mediacapture-main/) · [MDN](https://developer.mozilla.org/en-US/docs/Web/API/MediaDevices/enumerateDevices)

**Real Chrome**: Promise<MediaDeviceInfo[]>. Without `getUserMedia` permission, labels are empty strings, deviceIds are empty, but kind/groupId are populated. Typical desktop: 1× `audioinput`, 1× `audiooutput`, 0–1× `videoinput`.

**browser_oxide**: **MATCH** — `_navMediaDevices` plus `presets.rs:1-36` `default_media_devices()` builds 3-entry list with deterministic IDs. Note: real Chrome **without `getUserMedia` permission** returns empty `label`, empty `deviceId`. Our shim returns hashed deviceId — that's actually a tell. **Fix**: blank deviceId/label on first call; populate only after a `getUserMedia` permission grant.

### `navigator.mediaDevices.getUserMedia()` / `getDisplayMedia()`

**Real Chrome**: returns Promise that rejects with `NotAllowedError` without permission, `NotFoundError` if no device.

**browser_oxide**: **UNKNOWN** — verify rejection error name matches.

### `navigator.permissions.query({name})`

Already covered in `CHROME_JS_SURFACE_PARITY_2026_04_29.md` — **MATCH**.

### `navigator.connection` (NetworkInformation)

Already covered — **MATCH**.

### `navigator.serial` / `usb` / `bluetooth` / `hid`

**Spec**: [WebUSB](https://wicg.github.io/webusb/), [Web Serial](https://wicg.github.io/serial/), [Web Bluetooth](https://webbluetoothcg.github.io/web-bluetooth/), [WebHID](https://wicg.github.io/webhid/)

**Real Chrome**:
- `navigator.serial` — `Serial` prototype with `getPorts()`, `requestPort()` (Chrome 89+, Win/macOS/Linux only).
- `navigator.usb` — `USB` prototype with `getDevices()`, `requestDevice()`.
- `navigator.bluetooth` — `Bluetooth` prototype with `getAvailability()`, `requestDevice()`. Chrome 56+ desktop. **Important**: `bluetooth` is **absent on Linux Chrome 130+** unless `--enable-experimental-web-platform-features`.
- `navigator.hid` — `HID` prototype with `getDevices()`, `requestDevice()`.

`getDevices()`/`getPorts()` return `Promise<[]>` without prior user grant.

**browser_oxide**: **PARTIAL** — `window_bootstrap.js:661-664` defines getters for `_navBluetooth/_navUsb/_navSerial/_navHid` but the underlying objects' shapes are not visible in grep. **Fix**: ensure each is an instance of its respective class (`Bluetooth`, `USB`, `Serial`, `HID`) with `Symbol.toStringTag` and `getDevices()/getPorts()/getAvailability()` returning `Promise.resolve([])` or `Promise.resolve(false)` respectively.

### `navigator.locks`

**Spec**: [W3C Web Locks](https://w3c.github.io/web-locks/)

**Real Chrome 69+**: `LockManager` with `request(name, callback)`, `query()`. `query()` resolves to `{held:[], pending:[]}`.

**browser_oxide**: **PARTIAL** — `_navLocks` exists at `window_bootstrap.js:666`. Verify `request()` and `query()` methods, prototype name `LockManager`.

### `navigator.credentials.{get,create,store,preventSilentAccess}`

**Real Chrome**: `CredentialsContainer` prototype. `get()` rejects with `NotAllowedError` for password without UA; `create({publicKey})` rejects without authenticator.

**browser_oxide**: **MATCH** (defensive) — `window_bootstrap.js:368-478` ships `PublicKeyCredential` static methods, `IdentityProvider`, `_navCredentials` reference, and `CredentialsContainer` is in `interfaces_bootstrap.js:126`.

### `navigator.storage.{estimate,persist,persisted,getDirectory}`

**Real Chrome**: `StorageManager` with `estimate()`, `persist()`, `persisted()`, `getDirectory()` (FSA Origin Private File System).

**browser_oxide**: **PARTIAL** — `window_bootstrap.js:560-572` ships `estimate()` returning `{quota, usage, usageDetails:{...}}`. `persist()`/`persisted()` not visible — verify. `getDirectory()` likely missing — fingerprint probes for OPFS. **Fix**: add `persist→Promise<true>`, `persisted→Promise<true>`, `getDirectory→Promise<FileSystemDirectoryHandle>` (or rejection mirroring no-OPFS).

### `navigator.mediaSession.{playbackState,metadata,setActionHandler,setPositionState,setCameraActive,setMicrophoneActive}`

Already upgraded in `window_bootstrap.js:4168-4226`. **MATCH** for the 4 main methods. **PARTIAL**: `setCameraActive`, `setMicrophoneActive` (Chrome 130+) need verification.

### `navigator.keyboard.{getLayoutMap,lock,unlock}`

**Real Chrome**: `Keyboard` prototype with `getLayoutMap()` returning Promise<KeyboardLayoutMap>, `lock()`/`unlock()` for keyboard lock API. **Chrome desktop only.**

**browser_oxide**: **MATCH** — `window_bootstrap.js:498` notes coverage; `_navKeyboard` at line 665.

### `navigator.scheduling`

**Real Chrome 94+**: object with `isInputPending(options)` method. Used by `BotD` to detect headless.

**browser_oxide**: **MATCH** — `_navScheduling` at `window_bootstrap.js:682, 4283`.

### `navigator.virtualKeyboard`

**Real Chrome 94+**: `VirtualKeyboard` instance with `show()`, `hide()`, `boundingRect`, `overlaysContent`, EventTarget. Desktop only.

**browser_oxide**: **GAP** — not in grep. **Fix**: add `class VirtualKeyboard extends EventTarget`, expose `navigator.virtualKeyboard`. ~25 LOC.

### `navigator.contacts`

**Real Chrome**: **NOT EXPOSED** on desktop (Android Chrome only).

**browser_oxide**: **MATCH** — correctly absent on desktop profile.

### `navigator.devicePosture`

**Spec**: [W3C Device Posture](https://w3c.github.io/device-posture/) · Chrome 110+ desktop.

**Real Chrome**: `DevicePosture` instance with `type: "continuous"|"folded"`, EventTarget.

**browser_oxide**: **GAP** — absent. **Fix**: add `class DevicePosture extends EventTarget` with `type:"continuous"`. ~20 LOC.

### `navigator.windowControlsOverlay`

**Real Chrome 96+**: `WindowControlsOverlay` instance. Only meaningful in installed PWA contexts; on regular tabs `getTitlebarAreaRect()` returns `{x:0,y:0,width:0,height:0}`.

**browser_oxide**: **GAP** — absent. **Fix**: stub class with `visible:false`, `getTitlebarAreaRect`. ~20 LOC.

### `navigator.plugins.{length, [i]}` and `navigator.mimeTypes`

Already shipped — `window_bootstrap.js:114-280` exposes the canonical 5-PDF plugin set with 2 mime types. **MATCH**.

### `navigator.javaEnabled()`

**Real Chrome**: returns `false` (Java is gone).

**browser_oxide**: **MATCH** — `window_bootstrap.js:686`.

### `navigator.taintEnabled()`

**Real Chrome**: throws `TypeError: navigator.taintEnabled is not a function` (the method does NOT exist on Chrome).

**browser_oxide**: **MATCH** — not defined.

### `navigator.vibrate(pattern)`

**Real Chrome desktop**: returns `false` (vibration API non-functional on desktop). Mobile Chrome: returns `true` after a transient activation.

**browser_oxide**: **MATCH** — `window_bootstrap.js:761`.

### `navigator.getGamepads()`

**Real Chrome**: returns array of length 4, all elements `null` until a gamepad is connected.

**browser_oxide**: **MATCH** — `window_bootstrap.js:762` returns `[null, null, null, null]`.

### `navigator.registerProtocolHandler(scheme, url)` / `unregisterProtocolHandler`

**Real Chrome**: methods exist. `register…` throws `SecurityError` for non-`web+*` and non-allowlisted schemes. Returns `undefined` on success.

**browser_oxide**: **PARTIAL** — `window_bootstrap.js:763-764` no-op stubs. **Fix**: add `SecurityError` throw for invalid schemes to match Chrome's actual error path.

### `navigator.requestMediaKeySystemAccess(keySystem, configurations)`

**Real Chrome**: returns Promise<MediaKeySystemAccess>. `org.w3.clearkey` always supported; `com.widevine.alpha` and `com.microsoft.playready` are platform-dependent.

**browser_oxide**: **PARTIAL** — `window_bootstrap.js:766` mentions clearkey requirement. Verify the full implementation rejects with `NotSupportedError` for unknown systems.

### `navigator.getBattery()`

Already covered — **MATCH** (post-upgrade).

### `Notification.permission` / `Notification.requestPermission()`

**Real Chrome**: `Notification.permission ∈ {"default","granted","denied"}`. `requestPermission()` returns Promise resolving to one of those. Headed Chrome with no prior interaction: `"default"`.

**browser_oxide**: **PARTIAL** — `window_bootstrap.js:1285` defines `Notification` with static `permission="default"`. `requestPermission` not visible — **Fix**: add `Notification.requestPermission = () => Promise.resolve("default")` and the legacy callback signature too.

### **Top fixes for category 4**

1. **Blank-out `mediaDevices.enumerateDevices()` deviceId/label on the first call** before any `getUserMedia` permission. Lock-in test: `enumerateDevices()` prior to permission must return `[{deviceId:"", label:"", kind:"audioinput", groupId: <hash>}, ...]`. *Helps: every CreepJS run, every Akamai BMP `md` field encoder, FingerprintJS audio component.*
2. **Add `VirtualKeyboard`, `DevicePosture`, `WindowControlsOverlay` stubs**. These are 60 LOC total. *Helps: prototype-presence probes from FingerprintJS v4 and CreepJS Chrome-API enumeration.*
3. **Fix `Not-A.Brand` → `Not.A/Brand`** literal. *Helps: every Sec-CH-UA probe; sites that strict-string-compare instead of regex.*
4. **Add `Notification.requestPermission`** as a Promise-returning + legacy-callback method. *Helps: any site that probes the function shape.*
5. **`registerProtocolHandler` SecurityError path**. *Helps: prototype-method-toString cross-check (real Chrome has the throw).*

---

## 5. Graphics

### `WebGLRenderingContext.getParameter(VENDOR/RENDERER/VERSION/...)`

Already covered — **MATCH** (canvas_bootstrap.js:331-454, 36-extension parity).

### `WebGLRenderingContext.getParameter(MAX_*)` numeric constants

**Real Chrome 147 macOS arm64 (Apple M-series via ANGLE Metal)**:
- MAX_TEXTURE_SIZE: 16384
- MAX_VIEWPORT_DIMS: [16384, 16384]
- MAX_VERTEX_ATTRIBS: 16
- MAX_VERTEX_UNIFORM_VECTORS: 1024
- MAX_FRAGMENT_UNIFORM_VECTORS: 1024
- MAX_VARYING_VECTORS: 31
- MAX_TEXTURE_IMAGE_UNITS: 16
- MAX_VERTEX_TEXTURE_IMAGE_UNITS: 16
- MAX_COMBINED_TEXTURE_IMAGE_UNITS: 32
- MAX_CUBE_MAP_TEXTURE_SIZE: 16384
- MAX_RENDERBUFFER_SIZE: 16384
- ALIASED_LINE_WIDTH_RANGE: [1, 1]
- ALIASED_POINT_SIZE_RANGE: [1, 511]
- SAMPLES: 0 (not multisampled by default)

**browser_oxide**: **MATCH** — captured fixture at `crates/js_runtime/tests/fixtures/chrome147/captured_macos_arm64.json` per existing parity doc; `canvas_bootstrap.js:331-454` ships these.

### `WebGLRenderingContext.getSupportedExtensions()`

**Real Chrome 147 macOS arm64**: a 36-entry list including:
`ANGLE_instanced_arrays, EXT_blend_minmax, EXT_color_buffer_half_float, EXT_disjoint_timer_query, EXT_float_blend, EXT_frag_depth, EXT_shader_texture_lod, EXT_sRGB, EXT_texture_compression_bptc, EXT_texture_compression_rgtc, EXT_texture_filter_anisotropic, EXT_clip_control, EXT_depth_clamp, EXT_polygon_offset_clamp, KHR_parallel_shader_compile, OES_element_index_uint, OES_fbo_render_mipmap, OES_standard_derivatives, OES_texture_float, OES_texture_float_linear, OES_texture_half_float, OES_texture_half_float_linear, OES_vertex_array_object, WEBGL_blend_func_extended, WEBGL_color_buffer_float, WEBGL_compressed_texture_astc, WEBGL_compressed_texture_etc, WEBGL_compressed_texture_etc1, WEBGL_compressed_texture_pvrtc, WEBGL_compressed_texture_s3tc, WEBGL_compressed_texture_s3tc_srgb, WEBGL_debug_renderer_info, WEBGL_debug_shaders, WEBGL_depth_texture, WEBGL_draw_buffers, WEBGL_lose_context, WEBGL_multi_draw, WEBGL_polygon_mode, WEBGL_stencil_texturing, WEBGL_provoking_vertex` (39 — list-length varies by exact build; FingerprintJS docs the 36–39 range as Chrome 130 baseline).

**browser_oxide**: **MATCH** — pinned by captured fixture per `crates/js_runtime/tests/fixtures/chrome147/captured_macos_arm64.json`.

### `WebGL2RenderingContext.getParameter` extras

**Real Chrome 147**: extra params on top of WebGL1 — `MAX_3D_TEXTURE_SIZE: 2048`, `MAX_ARRAY_TEXTURE_LAYERS: 2048`, `MAX_COLOR_ATTACHMENTS: 8`, `MAX_DRAW_BUFFERS: 8`, `MAX_ELEMENTS_INDICES: 2147483647`, `MAX_UNIFORM_BUFFER_BINDINGS: 24`.

**browser_oxide**: **MATCH** — same captured-fixture parity.

### `OffscreenCanvas` constructor

Already covered — **MATCH** (`canvas_bootstrap.js:1037-1099`, real canvas-backed).

### `OffscreenCanvasRenderingContext2D.measureText` / `CanvasRenderingContext2D.measureText`

**Real Chrome**: returns full 13-field `TextMetrics` — `width`, `actualBoundingBoxLeft`, `actualBoundingBoxRight`, `actualBoundingBoxAscent`, `actualBoundingBoxDescent`, `fontBoundingBoxAscent`, `fontBoundingBoxDescent`, `hangingBaseline`, `alphabeticBaseline`, `ideographicBaseline`, `emHeightAscent`, `emHeightDescent`, `width` (yes, twice — `width` is one of the 13 props). FingerprintJS canvas-text component reads these.

**browser_oxide**: **MATCH** — `canvas_bootstrap.js:148-156` returns full 13-field shape per recent T1.2 work.

### `CanvasRenderingContext2D` font-stack rasterization

**Real Chrome**: rendering of `"Soft Ruddy Foothold 2"` at `18pt Tahoma` produces a fixed bit-pattern per OS/GPU. CreepJS canvas hash; Akamai `cv` field.

**browser_oxide**: **MATCH** — recent work pinned Tahoma font-stack delta-injection. Note the Linux fallback is still a tell per `docs/CANVAS.md`.

### `navigator.gpu` (WebGPU)

**Spec**: [W3C WebGPU](https://www.w3.org/TR/webgpu/) · Chrome 113+

**Real Chrome 147 macOS arm64**: `navigator.gpu` is a `GPU` instance. `requestAdapter()` returns Promise<GPUAdapter>. `getPreferredCanvasFormat()` returns `"bgra8unorm"` on macOS, `"rgba8unorm"` on Windows.

**browser_oxide**: **PARTIAL** — `window_bootstrap.js:4234` mentions WebGPU as prototype getter. **Fix**: ensure `navigator.gpu instanceof GPU`, `navigator.gpu.getPreferredCanvasFormat()` returns OS-correct value, `requestAdapter()` resolves to a stub GPUAdapter exposing `info: {vendor, architecture, device, description}`.

### Font enumeration (canvas measurement)

**Real Chrome macOS**: ≈300 fonts including Apple-bundled (`SF Pro`, `Helvetica Neue`, etc.) plus Microsoft-bundled (`Arial`, `Times New Roman`, `Verdana`, `Tahoma`, `Georgia`, `Trebuchet MS`, `Courier New`, `Comic Sans MS`).

**browser_oxide**: **MATCH** — `window_bootstrap.js:3883-3925` ships per-OS font lists; `_fontFamilyWidthDelta` injects width delta. `document.fonts` (FontFaceSet) wired.

### **Top fixes for category 5**

1. **`navigator.gpu.getPreferredCanvasFormat()`** OS-correct return. *Helps: any site running a WebGPU presence probe — increasingly common per BrowserLeaks 2026 reports.*
2. **GPUAdapter.info parity** on `requestAdapter()` — vendor/architecture/device strings matching ANGLE Metal Apple M3. *Helps: WebGPU fingerprint hashes (still emerging in 2026).*
3. Run a Chrome-vs-our-engine canvas hash regression on Linux Tahoma to close the last canvas tell.

---

## 6. Audio

### `AudioContext` constructor

**Spec**: [W3C Web Audio](https://www.w3.org/TR/webaudio/)

**Real Chrome**: `AudioContext` instance has `sampleRate=48000` (typical, OS-dependent), `baseLatency` (e.g. `0.005333…`), `outputLatency`, `state ∈ {suspended,running,closed,interrupted}`, `audioWorklet`, `destination` (AudioDestinationNode), `currentTime`, `listener`.

**browser_oxide**: **MATCH** — `canvas_bootstrap.js:541-678`. Need to verify `audioWorklet` is non-null; CreepJS probes `typeof audioCtx.audioWorklet`.

### `OfflineAudioContext.startRendering()`

**Real Chrome**: returns Promise<AudioBuffer> with deterministic samples per (sampleRate, length, graph-config). DynamicsCompressor + Oscillator graph hash is the canonical fingerprint axis (CreepJS audio).

**browser_oxide**: **MATCH** — `canvas_bootstrap.js:678-895` ships seeded deterministic buffer. Per MEMORY.md Item 5 audio root-cause analysis is open; the seed-derived buffer **does not bit-match real Chrome** (which is fundamental — would require shipping a real DSP chain). Documented gap.

### `AudioContext.createOscillator/Compressor/Analyser/Buffer/BufferSource/Gain/BiquadFilter`

**Real Chrome**: factory methods on AudioContext returning typed AudioNode subclasses.

**browser_oxide**: **PARTIAL** — `_maskAsNative` covers `createOscillator`, `createDynamicsCompressor`, `close`, `suspend`, `resume`. Missing: `createAnalyser`, `createGain`, `createBiquadFilter`, `createBuffer`, `createBufferSource`, `createPanner`, `createConvolver`, `createDelay`, `createChannelSplitter`, `createChannelMerger`, `createMediaStreamSource`, `createMediaElementSource`, `createIIRFilter`, `createWaveShaper`, `createPeriodicWave`, `createScriptProcessor` (legacy). **Fix**: add factory stubs returning AudioNode-shape objects.

### `BiquadFilterNode.getFrequencyResponse(freqArr, magOut, phaseOut)`

**Real Chrome**: closed-form math over Float32Arrays.

**browser_oxide**: **GAP** — likely unimplemented. **Fix**: implement the analytic biquad transfer function — this is closed-form (no DSP pipeline needed).

### `AnalyserNode.getFloatFrequencyData/getByteFrequencyData/getFloatTimeDomainData`

**Real Chrome**: writes to provided Float32Array/Uint8Array.

**browser_oxide**: **GAP** — verify. **Fix**: deterministic stub fills with seed-derived values.

### **Top fixes for category 6**

1. **Implement `BiquadFilterNode.getFrequencyResponse`** — closed-form, no DSP. ~30 LOC. *Helps: CreepJS audio biquad probe (independent from compressor probe).*
2. **Add `createAnalyser`, `createGain`, `createBiquadFilter` factories** to AudioContext.prototype. *Helps: prototype presence probes; any site that constructs a graph and reads back frequencyBinCount.*
3. **Document the OfflineAudioContext non-bit-parity** as known limitation; consider shipping a "Chrome 147 macOS arm64 captured 44100×500 buffer" as the response to fix-bit-stability.

---

## 7. Network observable state

### `Sec-Ch-Ua`, `Sec-Ch-Ua-Mobile`, `Sec-Ch-Ua-Platform`

**Real Chrome 147**: `sec-ch-ua: "Not.A/Brand";v="24", "Chromium";v="147", "Google Chrome";v="147"` (GREASE first, but order varies); `sec-ch-ua-mobile: ?0`; `sec-ch-ua-platform: "macOS"`.

**browser_oxide**: **MATCH** — `crates/net/src/headers.rs:139-143, 240-246, 361-393`. Verified against real Chrome on 2026-04-27.

### `Sec-Ch-Ua-Full-Version-List`, `-Arch`, `-Bitness`, `-Form-Factors`, `-Full-Version`, `-Model`, `-Platform-Version`, `-Wow64`

**Real Chrome 147**: high-entropy headers sent **only after** a `Critical-CH` or `Accept-CH` server response. Empty quotes `""` for unknown (e.g. desktop `model: ""`).

**browser_oxide**: **MATCH** — `crates/net/src/headers.rs:209-269, 781-811`.

### `Accept-Language`

**Real Chrome 147 macOS arm64 (en-US)**: `Accept-Language: en-US,en;q=0.9`. **No** `q=1.0` (Chrome leaves the first item unweighted), no `*;q=0.5` tail.

**browser_oxide**: **UNKNOWN** — verify exact byte string in `crates/net/src/headers.rs`.

### `Accept-Encoding`

**Real Chrome 147**: `Accept-Encoding: gzip, deflate, br, zstd` (zstd added in Chrome 123). Note: **no `identity`**, **no `*`**.

**browser_oxide**: **UNKNOWN** — verify includes zstd.

### `Sec-Fetch-Dest` / `-Mode` / `-Site` / `-User`

**Real Chrome**: per-request based on initiator. For top-level navigation: `Sec-Fetch-Dest: document`, `Mode: navigate`, `Site: none|same-origin|same-site|cross-site`, `User: ?1`.

**browser_oxide**: **UNKNOWN** — verify Sec-Fetch-* are emitted with correct semantics.

### `Upgrade-Insecure-Requests`

**Real Chrome**: `1` on top-level navigations to `http:` URLs; absent for HTTPS or sub-resources.

**browser_oxide**: **UNKNOWN** — verify.

### `priority` HTTP/2 pseudo-header

**Real Chrome**: per-RFC9218 priority hint (`u=N,i=…`). Chrome sends `priority: u=0,i` for top-level navigations, `u=1` for blocking subresources, `u=4` for images.

**browser_oxide**: **UNKNOWN** — `rquest` BoringSSL impersonation likely handles this; verify.

### HTTP/3 / QUIC alt-svc handling

**Real Chrome**: respects `Alt-Svc: h3=":443"; ma=86400` and switches to QUIC on next request.

**browser_oxide**: **PARTIAL** — `presets.rs:91 allow_http3: false`. HTTP/3 is gated off; the absence might itself be a tell on QUIC-only origins. **Fix**: enable HTTP/3 by default with `allow_http3: true` once stable.

### `RTCPeerConnection` ICE candidate gathering

Already covered — **MATCH** (mDNS host + null).

### **Top fixes for category 7**

1. **Verify `Accept-Encoding` includes `zstd`** (Chrome 123+). *Helps: any origin that branches on Accept-Encoding profile.*
2. **Audit `Sec-Fetch-*` per request type** in fetch and navigation paths. Already partly done; verify `Sec-Fetch-User: ?1` only on user-activated navigations. *Helps: every CSP-aware probe.*
3. **HTTP/3 enablement** profile flag. *Helps: cross-version-tracking origins; not a "fix" per se, parity with real Chrome.*

---

## 8. WebAuthn / FedCM / Identity

### `PublicKeyCredential` constructor + static methods

**Real Chrome**: `PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable()` resolves to `true` on macOS with Touch ID, `false` on Linux/Windows-no-Hello.
`PublicKeyCredential.isConditionalMediationAvailable()` resolves to `true` (Chrome 108+).
`PublicKeyCredential.getClientCapabilities()` resolves to a dict of booleans (Chrome 133+) — `{conditionalCreate, conditionalGet, hybridTransport, passkeyPlatformAuthenticator, ...}`.

**browser_oxide**: **MATCH** — `window_bootstrap.js:395-421, 476-477` ships all three statics, profile-driven via `has_platform_authenticator` and `conditional_mediation`. `getClientCapabilities` shape needs verification against Chrome 133+ shape.

### `IdentityProvider`

**Real Chrome 117+**: `IdentityProvider.getUserInfo(config)` returns Promise<IdentityProviderUserInfo>.

**browser_oxide**: **MATCH** — `window_bootstrap.js:430-434`.

### `navigator.credentials.get({identity:...})`

**Real Chrome 108+ FedCM**: returns Promise<IdentityCredential>. Without registered IDP, rejects with `IdentityCredentialError`.

**browser_oxide**: **PARTIAL** — `_navCredentials` referenced at `window_bootstrap.js:660`. Verify rejection shape matches Chrome's `IdentityCredentialError` for FedCM probes.

### **Top fixes for category 8**

1. Verify `PublicKeyCredential.getClientCapabilities()` shape matches Chrome 133+ (full 8-key dict) under all profiles. *Helps: any modern WebAuthn probe.*

---

## 9. Internationalization / locale

### `Intl.DateTimeFormat().resolvedOptions().timeZone`

**Real Chrome**: returns IANA timezone name like `"America/Los_Angeles"`. Self-consistent with `Date().getTimezoneOffset()`.

**browser_oxide**: **MATCH** — `window_bootstrap.js:1786-1820` patches `Intl.DateTimeFormat` and `Date.prototype.getTimezoneOffset` to honor profile timezone. Cross-consistency verified.

### `Intl.DateTimeFormat().resolvedOptions().locale`

**Real Chrome**: matches `navigator.language` (e.g. `"en-US"`).

**browser_oxide**: **PARTIAL** — patched but verify the `locale` field comes from profile `language` not V8 default.

### `Intl.NumberFormat`, `Intl.Collator`, `Intl.PluralRules`, `Intl.RelativeTimeFormat` resolved options

**Real Chrome**: `numberingSystem`, `useGrouping`, `notation`, etc. resolved from locale.

**browser_oxide**: **MATCH** — `window_bootstrap.js:1807-1817` patches all five Intl classes.

### `Intl.ListFormat`, `Intl.DisplayNames`, `Intl.Segmenter`

**Real Chrome 127+**: all three are present.

**browser_oxide**: **UNKNOWN** — V8's built-in Intl ships these by default. **Fix**: confirm V8 build flags include these classes.

### `Date.prototype.getTimezoneOffset()`

**Real Chrome**: minutes WEST of UTC. PDT=420; UTC=0.

**browser_oxide**: **MATCH** — `window_bootstrap.js:1830-1859` patches with profile-derived offset, Symbol-tagged toString.

### `Date.toString()` / `Date.toLocaleString()`

**Real Chrome**: format `"Tue Apr 29 2026 12:34:56 GMT-0700 (Pacific Daylight Time)"`. Includes timezone abbrev in parens.

**browser_oxide**: **PARTIAL** — V8's `Date.toString()` defaults to UTC if process timezone not set, and our patch only wraps `getTimezoneOffset`. `Date.toString()` itself may still print UTC. **Fix**: ensure `Date.prototype.toString` is also patched (or set `process.env.TZ` before V8 init).

### **Top fixes for category 9**

1. **Patch `Date.prototype.toString` and `toLocaleString`** to render the profile's timezone abbreviation. Lock-in: `assert!(date.toString().contains("Pacific"))`. *Helps: every CreepJS run; Akamai BMP `tz` field.*
2. Verify Intl.Segmenter / ListFormat / DisplayNames are present (V8 build flags).

---

## 10. Performance / Timing

### `performance.now()` precision

**Real Chrome**: 5µs in regular contexts; 100µs in cross-origin-isolated (no SharedArrayBuffer); 1ms when site is timing-allow-rejected. Resolution measurable via `Math.min(...samples-of-(now()-now()))`.

**browser_oxide**: **MATCH** — `window_bootstrap.js:4095` notes humanization via `op_perf_now_humanized`.

### `performance.timeOrigin`

**Real Chrome**: ms-precision Unix epoch double of navigation start.

**browser_oxide**: **MATCH** — `window_bootstrap.js:2017` defines `Performance.prototype.timeOrigin`.

### `performance.memory.{jsHeapSizeLimit,totalJSHeapSize,usedJSHeapSize}`

**Real Chrome (Chrome-only, non-standard)**: `jsHeapSizeLimit ≈ 4_294_705_152` (4 GB on V8 default), `totalJSHeapSize ≈ 30_000_000` after page load, `usedJSHeapSize ≈ 20_000_000`. **Quantized to 100 MB buckets** in cross-origin isolated contexts.

**browser_oxide**: **MATCH** — `window_bootstrap.js:1760, 2005-2014` ships `jsHeapSizeLimit: 4294705152` and stub total/used.

### `performance.timing` (legacy)

**Real Chrome**: deprecated but present. Returns `PerformanceTiming` with `navigationStart`, `loadEventStart`, etc.

**browser_oxide**: **PARTIAL** — `window_bootstrap.js:1864-1990` ships nav-timing entries. Verify legacy `performance.timing` getter is wired.

### `performance.getEntriesByType('navigation')` / `('resource')`

**Real Chrome**: returns array of PerformanceNavigationTiming / PerformanceResourceTiming entries.

**browser_oxide**: **MATCH** — `window_bootstrap.js:1864-2056` ships nav + canned-resource entries (per Akamai requirement).

### `performance.eventCounts`

**Real Chrome 87+**: live `EventCounts` Map. Counts dispatched events per type. CreepJS reads `eventCounts.size`.

**browser_oxide**: **GAP** — verify; likely absent. **Fix**: stub as empty Map. ~10 LOC.

### `performance.measureUserAgentSpecificMemory()`

**Real Chrome 89+ (cross-origin-isolated only)**: returns Promise<MemoryMeasurement>. Outside isolated context, rejects with `SecurityError`.

**browser_oxide**: **GAP** — likely absent. **Fix**: stub returning `Promise.reject(SecurityError)`.

### `PerformanceObserver`

**Real Chrome**: supports `entryTypes: ["navigation","resource","mark","measure","paint","longtask","largest-contentful-paint","layout-shift","element","first-input","event"]`. `PerformanceObserver.supportedEntryTypes` is a frozen array of these.

**browser_oxide**: **PARTIAL** — `window_bootstrap.js:1601-1611` defines class and `supportedEntryTypes`. Verify the array contains the full Chrome 147 list (11+ entries).

### **Top fixes for category 10**

1. **Add `performance.eventCounts`** as a Map (or proxy with `.size`). *Helps: CreepJS event-counts probe; ~10 LOC.*
2. **`performance.measureUserAgentSpecificMemory`** rejection stub. *Helps: prototype-method-presence probes.*
3. **Verify `PerformanceObserver.supportedEntryTypes`** matches Chrome 147 full list.

---

## 11. CSS / Layout

### `getComputedStyle(element)`

**Real Chrome**: returns CSSStyleDeclaration with all 400+ resolved properties — `font-family`, `font-size`, `color`, `display`, etc., from the cascade.

**browser_oxide**: **PARTIAL** — `window_bootstrap.js:2543-2556` notes the patch; reads inline + CSS defaults. Lacks full cascade integration. Tracked open task. *FingerprintJS reads `getComputedStyle(body).fontFamily` for OS detection — covered if profile sets it.*

### `Element.getBoundingClientRect`

**Real Chrome**: returns DOMRect with viewport-relative coordinates.

**browser_oxide**: **PARTIAL** — `dom_bootstrap.js:637-640` returns stubbed DOMRect. Layout-bound; tracked.

### `Element.scrollIntoView()`, `scroll()`, `scrollBy()`, `scrollTo()`

**Real Chrome**: scrolls the element/window with optional smooth behavior. Mutates `scrollTop`/`scrollLeft`.

**browser_oxide**: **PARTIAL** — `dom_bootstrap.js:671` is a no-op. `window.scroll*` patched at `window_bootstrap.js:1019-1035`.

### `IntersectionObserver`

**Real Chrome**: real layout-driven; fires entries when target intersects viewport.

**browser_oxide**: **PARTIAL** — `window_bootstrap.js:2486-2511` fires immediately with `intersectionRatio: 1` for all observed targets. Acceptable for fingerprint presence checks; not real intersection logic.

### `ResizeObserver`

**browser_oxide**: **PARTIAL** — `window_bootstrap.js:2515` fires on observe with current dimensions.

### `MutationObserver`

**browser_oxide**: **MATCH** — real implementation `dom_bootstrap.js:1711-1870`.

### `PerformanceObserver`

Already covered — **PARTIAL**.

### **Top fixes for category 11**

1. **Real layout integration** for `getBoundingClientRect`, `clientWidth`, `scrollWidth`, `elementFromPoint`. Tracked as open task #66; the highest-leverage layout-bound fix.
2. **`getComputedStyle` `font-family` resolution from cascade**. *Helps: every CreepJS run; OS-detection cross-check.*

---

## 12. Events

### Mouse events: `mousemove`, `mousedown`, `mouseup`, `click`, `contextmenu`, `dblclick`, `mouseover`/`out`/`enter`/`leave`

**Real Chrome event sequence**: `mousedown → mouseup → click` for a normal click; `mousedown → mouseup → click → dblclick` for a double-click. `mouseenter`/`leave` non-bubbling, `mouseover`/`out` bubbling.

**browser_oxide**: **PARTIAL** — `event_bootstrap.js:64-87` defines MouseEvent. Synthetic dispatch path through `crates/browser/src/page.rs` (input automation). Verify all 10 event names dispatched correctly.

### Pointer events

**Real Chrome**: PointerEvent extends MouseEvent. `pointerType` ∈ `{"mouse","pen","touch"}`. `pointerId` per active pointer. `isPrimary: true` for first.

**browser_oxide**: **MATCH** — `event_bootstrap.js:123-138`. Verify all 11 properties.

### Touch events

**Real Chrome desktop**: touch events DO fire on touch-capable laptops. On non-touch desktop, `TouchEvent` constructor exists but events don't fire.

**browser_oxide**: **MATCH** — `window_bootstrap.js:4393-4408, 4362+` defines TouchEvent class.

### Keyboard events: `keydown`, `keypress`, `keyup`

**Real Chrome 147**: `keydown→keypress→input→keyup` for printable; `keydown→keyup` for non-printable. `event.key` is the character; `event.code` is physical key (`"KeyA"`); `keyCode` legacy (65 for A); `which` legacy alias.

**browser_oxide**: **MATCH** — `event_bootstrap.js:88-106` defines KeyboardEvent.

### Wheel events: `wheel`

**Real Chrome**: WheelEvent extends MouseEvent. `deltaY`, `deltaX`, `deltaZ`, `deltaMode ∈ {0,1,2}` (pixel/line/page).

**browser_oxide**: **MATCH** — `event_bootstrap.js:139-151`.

### Focus / blur / focusin / focusout

**Real Chrome**: focus/blur non-bubbling; focusin/focusout bubbling.

**browser_oxide**: **PARTIAL** — `FocusEvent` class exists at `event_bootstrap.js:116-122`. Verify dispatch sets bubbles correctly.

### Visibility events: `visibilitychange`, `pageshow`, `pagehide`, `freeze`, `resume`

**Real Chrome 88+**: emits `freeze` when tab is frozen by browser (bfcache); `resume` on unfreeze. CreepJS probes `'onfreeze' in document`.

**browser_oxide**: **GAP** — `freeze`/`resume`/`pageshow`/`pagehide` events not in grep. **Fix**: add `Document.prototype.onfreeze`/`onresume`/`onvisibilitychange`/etc as null-default settable props (the EventTarget side already handles dispatch).

### `beforeunload` / `unload`

**Real Chrome**: settable handlers `window.onbeforeunload`, `window.onunload`. Returning truthy from `onbeforeunload` shows browser confirm dialog.

**browser_oxide**: **PARTIAL** — likely no-op stubs.

### **Top fixes for category 12**

1. **Add `onfreeze`/`onresume`/`onpageshow`/`onpagehide`/`onvisibilitychange` properties** on Document/Window (settable, default null). ~15 LOC. *Helps: CreepJS DOM-property enumeration; FingerprintJS prototype walks.*
2. **Verify focus/blur dispatch order** — focusout-then-blur, focusin-then-focus, all per spec.

---

## 13. Storage / cookies

### `localStorage` / `sessionStorage`

**Real Chrome**: per-origin, ≈10 MB quota. `setItem` throws `QuotaExceededError` on overflow.

**browser_oxide**: **MATCH** — `window_bootstrap.js:2410-2481` real Rust-backed via `op_dom_storage_*`.

### `indexedDB`

**Real Chrome**: full IDB v3 — `open(name, version)`, transactions, cursors.

**browser_oxide**: **MATCH** — `window_bootstrap.js:3277-3785` ships IDBFactory and supporting classes.

### `caches` (Cache API)

**Real Chrome**: `caches` is a `CacheStorage` instance with `open()`, `keys()`, `match()`, `delete()`.

**browser_oxide**: **UNKNOWN** — not visible in grep. **Fix**: add `globalThis.caches` as `CacheStorage` instance with `open()`/`keys()`/`match()` returning empty results. ~30 LOC.

### `cookieStore` (Cookie Store API)

**Real Chrome 87+**: async cookie API. `cookieStore.get(name)`, `set()`, `delete()`, `getAll()`. EventTarget for `change` events.

**browser_oxide**: **GAP** — `interfaces_bootstrap.js:72` defines `CookieStore` as illegal-ctor. The instance `cookieStore` global is missing. **Fix**: add `globalThis.cookieStore` as a `CookieStore` instance wired to the existing cookie jar.

### `Set-Cookie` / SameSite / Secure / HttpOnly / Partitioned parsing

**Real Chrome**: per RFC 6265bis + CHIPS. Full attribute support including `Partitioned` (Chrome 114+).

**browser_oxide**: **UNKNOWN** — verify in `crates/net/src/`.

### **Top fixes for category 13**

1. **Add `globalThis.cookieStore`** wired to existing cookie jar. *Helps: prototype-presence probes; Akamai cookie API probe.*
2. **Add `globalThis.caches`** stubbed CacheStorage. *Helps: SW-aware probes.*

---

## 14. Workers

### `Worker` constructor

**Real Chrome**: spawns a new V8 isolate with its own globalThis. `postMessage`, `onmessage`, `terminate`, `addEventListener`.

**browser_oxide**: **MATCH** — `window_bootstrap.js:1294-1457` real implementation.

### `SharedWorker` / `ServiceWorker`

**Real Chrome**: `SharedWorker` constructor; `ServiceWorker` lifecycle via `navigator.serviceWorker.register()`.

**browser_oxide**: **PARTIAL** — `window_bootstrap.js:1461-1493` stubs both classes; ServiceWorkerContainer is at `window_bootstrap.js:578-611` and `register()` returns rejected/empty Promise. Adequate for presence probes.

### `OffscreenCanvas` in workers

**Real Chrome**: `OffscreenCanvas` is transferable to Workers via `postMessage(canvas, [canvas])`.

**browser_oxide**: **UNKNOWN** — verify worker context exposes `OffscreenCanvas`.

### WebRTC in workers

**Real Chrome**: NOT supported (RTCPeerConnection unavailable in workers).

**browser_oxide**: **MATCH** — RTCPeerConnection only on main thread.

### **Top fixes for category 14**

1. Verify `OffscreenCanvas` is exposed in `worker_bootstrap.js` for the small set of sites that probe worker globals.
2. Confirm `ServiceWorker.register()` returns a stub Registration (currently returns rejection).

---

## 15. Misc Chrome-specific

### `window.chrome.{app, csi, loadTimes}`

Already covered — **MATCH**.

### `window.chrome.runtime`

**Real Chrome**: **NOT PRESENT** on regular pages; only present in extension contexts. Probing `'runtime' in window.chrome` returning `true` on a regular page is an extension-impersonation tell.

**browser_oxide**: **PARTIAL/INCONSISTENT** — `interfaces_bootstrap.js:144-146` defines `chrome.runtime.OnInstalledReason` (which would make `runtime in chrome` true), but `window_bootstrap.js:1089-1100` correctly omits `chrome.runtime`. **Fix**: ensure `window_bootstrap.js` runs **after** `interfaces_bootstrap.js` and explicitly **deletes** `chrome.runtime` on regular pages.

### `window.chrome.webstore`

**Real Chrome 126+**: removed. Probing `'webstore' in chrome` returning `true` is an old-Chrome tell.

**browser_oxide**: **MATCH** — not defined.

### `SpeechSynthesis.getVoices()`

Already covered — **MATCH**.

### `Notification`

Already covered — **MATCH** (with the `requestPermission` gap noted).

### File System Access API: `showOpenFilePicker`, `showSaveFilePicker`, `showDirectoryPicker`

**Real Chrome 86+**: present on Window. Calls require user activation; throw `SecurityError` on direct invocation.

**browser_oxide**: **GAP** — not in grep. The class shells exist in `interfaces_bootstrap.js:77-80`. **Fix**: add `window.showOpenFilePicker = () => Promise.reject(SecurityError)` etc. ~15 LOC.

### `BroadcastChannel`

**Real Chrome 54+**: present.

**browser_oxide**: **MATCH** — `window_bootstrap.js:1667+`.

### Web Locks API

Already covered (navigator.locks) — **PARTIAL**.

### View Transitions API: `document.startViewTransition()`

**Real Chrome 111+**: returns `ViewTransition` instance.

**browser_oxide**: **GAP** — not in grep. **Fix**: stub `Document.prototype.startViewTransition = (cb) => { const ready = Promise.resolve(); const finished = Promise.resolve(); cb && cb(); return {ready, updateCallbackDone:ready, finished, skipTransition(){}}; }`. ~15 LOC.

### `EyeDropper`

**Real Chrome 95+**: `class EyeDropper { open() }`.

**browser_oxide**: **GAP** — not in grep. **Fix**: add stub class.

### `BarcodeDetector`

**Real Chrome desktop**: **NOT** present (Android Chrome only). Probing `typeof BarcodeDetector === 'undefined'` returning true is correct on desktop.

**browser_oxide**: **MATCH** — correctly absent.

### `IdleDetector`

**Real Chrome 94+ desktop**: `class IdleDetector` present.

**browser_oxide**: **GAP** — not in grep. **Fix**: stub class with `requestPermission()` static and `start()` instance method.

### `LaunchQueue`

**Real Chrome**: `globalThis.launchQueue` is a `LaunchQueue` instance only in PWA contexts.

**browser_oxide**: **PARTIAL** — class shell at `interfaces_bootstrap.js:74`. Instance `launchQueue` not exposed. Real Chrome on regular page **does not** expose `launchQueue` instance — current state is correct.

### `WebTransport`

**Real Chrome 97+**: `new WebTransport(url)` initiates HTTP/3 connection.

**browser_oxide**: **PARTIAL** — `interfaces_bootstrap.js:73` constructor throws TypeError. Real Chrome's constructor returns a WebTransport instance that *then* fails to connect. **Fix**: return a stub instance whose `ready` Promise rejects, instead of synchronous throw.

### `CookieStore`

Covered above (category 13) — **GAP** for the instance.

### **Top fixes for category 15**

1. **Add `EyeDropper`, `IdleDetector` class stubs**; **`startViewTransition`** method on Document. *Helps: prototype-presence probes.*
2. **Fix `WebTransport` constructor** to return an instance whose ready/closed Promises reject (not synchronous TypeError).
3. **Audit `chrome.runtime`** — must NOT exist on regular pages (it's an extension-impersonation tell).

---

## Top 20 highest-ROI fixes overall

Ranked by (sites unlocked × inverse difficulty). All file:line refs are absolute paths.

| # | Fix | Effort | Sites unlocked | File:line |
|---|---|---|---|---|
| 1 | **`mediaDevices.enumerateDevices()` blank-out deviceId/label pre-permission** | XS (5 LOC) | CreepJS, Akamai BMP `md`, FingerprintJS audio | `crates/stealth/src/presets.rs:1-36` + new shim wrapper |
| 2 | **`navigator.userAgentData` GREASE literal `"Not.A/Brand"`** | XS (1 char) | Sec-CH-UA-strict probes | `crates/js_runtime/src/js/window_bootstrap.js:1128-1253` |
| 3 | **Add `performance.eventCounts` Map** | XS (10 LOC) | CreepJS event-counts probe | `crates/js_runtime/src/js/window_bootstrap.js:1990-2056` (new entry near `_PerfProto`) |
| 4 | **Add `Notification.requestPermission()` Promise+callback** | XS (10 LOC) | every notification-aware probe | `crates/js_runtime/src/js/window_bootstrap.js:1285` |
| 5 | **Convert `window.scrollX/Y/pageXOffset/pageYOffset/screenX/Y` to prototype getters** | S (20 LOC) | FingerprintJS v4 own-descriptor probes | `crates/js_runtime/src/js/window_bootstrap.js:1010-1015` |
| 6 | **`matchMedia` full lexer for 12 features** | M (80 LOC) | CreepJS media, prefer-color-scheme themes, FingerprintJS | `crates/js_runtime/src/js/window_bootstrap.js:2892-2905` |
| 7 | **`document.referrer` wired to `Referer` request header** | S (15 LOC) | Akamai BMP `rt`, every redirect-chain check | `crates/js_runtime/src/js/dom_bootstrap.js:1345` (+ Rust op) |
| 8 | **`Date.prototype.toString` patched for profile timezone** | S (25 LOC) | CreepJS, Akamai BMP `tz` | `crates/js_runtime/src/js/window_bootstrap.js:1855-1860` |
| 9 | **Add `globalThis.cookieStore`** instance wired to cookie jar | S (40 LOC) | Cookie-Store-aware sites; modern-API probes | `crates/js_runtime/src/js/window_bootstrap.js` (new) + cookie ops |
| 10 | **Add `globalThis.caches` stubbed CacheStorage** | S (30 LOC) | SW-aware sites | `crates/js_runtime/src/js/window_bootstrap.js` (new) |
| 11 | **`screen.availLeft` + macOS `screen_avail_top:25`** | XS (3 LOC) | every screen probe | `crates/js_runtime/src/js/window_bootstrap.js:978`, `crates/stealth/src/presets.rs` |
| 12 | **Add `VirtualKeyboard`, `DevicePosture`, `WindowControlsOverlay` stubs** | S (60 LOC total) | FingerprintJS prototype walks | `crates/js_runtime/src/js/window_bootstrap.js` (new) |
| 13 | **Add `EyeDropper`, `IdleDetector`, `Document.startViewTransition` stubs** | S (40 LOC) | prototype-presence probes | `crates/js_runtime/src/js/interfaces_bootstrap.js` + `dom_bootstrap.js` |
| 14 | **`BiquadFilterNode.getFrequencyResponse`** closed-form impl | M (30 LOC) | CreepJS audio biquad probe | `crates/js_runtime/src/js/canvas_bootstrap.js:678+` |
| 15 | **`AudioContext` factory stubs** (createAnalyser/Gain/Biquad/etc) | S (50 LOC) | CreepJS, audio prototype walks | `crates/js_runtime/src/js/canvas_bootstrap.js:541-678` |
| 16 | **`navigator.gpu.getPreferredCanvasFormat()`** OS-correct | XS (5 LOC) | WebGPU-aware probes | `crates/js_runtime/src/js/window_bootstrap.js:4234+` |
| 17 | **`registerProtocolHandler` SecurityError for bad schemes** | S (15 LOC) | prototype-method-toString cross-checks | `crates/js_runtime/src/js/window_bootstrap.js:763` |
| 18 | **`pageshow` / `readystatechange→interactive`** in load sequence | S (20 LOC) | Akamai `lc` field; bfcache hints | `crates/browser/src/page.rs:313-320, 1961-1969` |
| 19 | **Confirm `chrome.runtime` is NOT exposed on regular pages** (delete after interfaces_bootstrap) | XS (3 LOC) | every extension-impersonation probe | `crates/js_runtime/src/js/window_bootstrap.js` (post-init) |
| 20 | **`WebTransport` constructor returns instance with rejecting `ready`/`closed`** | S (15 LOC) | WebTransport-aware probes | `crates/js_runtime/src/js/interfaces_bootstrap.js:73` |

**Aggregate effort to ship 1–13 above (XS+S only)**: ≈1.5 engineering days.

---

## Surfaces where browser_oxide can exceed Playwright

These are surfaces where Playwright/Chromium-bundle-Chrome emit a "wrong" or "obviously-bot" value because they're bound to their Chromium binary's automation defaults — but a from-scratch engine with full V8 control can emit the spec-correct or real-headed-Chrome value. We **already** beat Playwright on:

- **`BatteryManager` class identity** — Playwright headless returns a plain object on some Linux builds; we return a real `BatteryManager extends EventTarget` instance.
- **mDNS ICE candidates with `typ host`** — Playwright headless emits NO ICE candidates; real Chrome emits `<UUID>.local typ host`. We match Chrome.
- **`userAgentData` GREASE order randomization** — Playwright fixes the order; real Chrome rotates. We rotate.
- **`Notification.permission === "default"`** — old headless Chrome returned `"denied"` (a tell); recent Playwright fixed but still betrays via prerender. We always return `"default"`.

**New surfaces where we can exceed Playwright** (identified during this audit):

1. **`mediaDevices.enumerateDevices()` deviceId blanking pre-permission** — Playwright headless leaks deterministic deviceIds even before permission. Real headed Chrome blanks them. We can match the real-Chrome blank.
2. **`navigator.webdriver` non-configurable descriptor** — Playwright headless makes it configurable; real Chrome's is non-configurable. We already do this (`window_bootstrap.js:1120-1124`).
3. **`Function.prototype.toString` of native functions** — Playwright's CDP override leaves `toString` distinguishable in some cases; our `_maskAsNative` is invariant under cross-realm reads (verified by `perimeterx_surface_parity.rs`).
4. **`document.elementFromPoint` real hit-test** — Playwright matches real Chrome's layout; our stub is currently a tell. *Once we ship real layout (open task #66), we'd match.* Note: this is **catch-up**, not exceed.
5. **`chrome.runtime` correctly absent on non-extension pages** — Playwright sometimes leaks `runtime.OnInstalledReason` via stale plugin contexts. We can ensure clean omission.
6. **`Notification.requestPermission` Promise+callback dual signature** — Playwright headless rejects with `NotAllowedError`; real Chrome resolves to `"default"`. We can match real Chrome.
7. **Date.toString with profile timezone abbrev** — Playwright sets process TZ via env var, so it works; but we can patch per-realm independent of process state, allowing different per-tab TZ profiles (impossible in Playwright).
8. **`AudioContext.audioWorklet` non-null** — some Playwright CDP variants return `null` here; real Chrome always exposes an `AudioWorklet` instance.
9. **`performance.memory.jsHeapSizeLimit` with V8-correct ceiling** — Playwright reports the actual V8 heap (varies); we can pin to Chrome's standard 4 GB ceiling.
10. **Per-profile TLS impersonation** — `rquest` BoringSSL gives us per-profile JA3/JA4 control that Playwright's Chromium binary cannot vary without a rebuild.
11. **Per-profile HTTP/3 enable/disable** — bound to Chromium build in Playwright; we can flip per profile.
12. **Sec-CH-UA GREASE rotation per request** — Playwright rotates per-process; we can rotate per-request and still maintain consistency with `userAgentData.brands`.
13. **`screen.availTop=25` on macOS profile** — Playwright headless reports 0 (no menubar in headless); we can ship 25 to match real macOS Chrome.

**Surfaces we cannot exceed Playwright on** (require real Chrome rendering): canvas bit-stability, WebGL UNMASKED_RENDERER, AudioContext startRendering bit-stability, font kerning hash. These are tied to GPU/font-rasterizer real output.

---

## Out-of-scope / explicitly-deferred

These would require either policy review, external infrastructure, or behavior simulation outside the scope of "Web Platform parity":

1. **Encrypted PerimeterX `_px3` cookie HMAC reconstruction** — security-control bypass; not a Web Platform API.
2. **Akamai `sensor_data` token signing** — same.
3. **Kasada `ct` token forging** — same.
4. **Residential IP / proxy infrastructure** — operational; the engine cannot do this in pure-JS.
5. **TLS JA3/JA4 forging beyond what `rquest` BoringSSL impersonates** — out of scope; covered by `rquest`.
6. **Mouse/pointer trajectory humanization** — behavioral simulation, not API surface.
7. **Press-and-hold force/pressure curves** — behavioral, not API.
8. **CDP `Runtime.evaluate` byte-level leak detection** — already covered by automation-marker tests.
9. **Real layout/cascade/font-rasterizer** — open engineering task #66; tracked separately.
10. **HTTP/3 + QUIC retry/0-RTT path** — `rquest` covers; profile flag exists.
11. **Real DSP for AudioContext bit-parity** — bit-matching real Chrome's compressor output requires shipping libwebrtc's compressor or equivalent; documented as known limitation.
12. **Service Worker real lifecycle** — separate engineering effort.

---

## 400-word summary: top-5 highest-ROI items

After auditing 110+ Web Platform surfaces against `crates/js_runtime/src/js/*.js` and `crates/stealth/src/presets.rs`, the five highest-ROI parity improvements I can recommend — each grounded in MDN/W3C/Chromium-source/FingerprintJS-open-source citations — are:

**1. `mediaDevices.enumerateDevices()` blank-out deviceId/label pre-permission (XS, ≈5 LOC, helps ≈4-6 of 12 failing sites)**. Real headed Chrome returns `[{deviceId:"", label:"", kind:"audioinput", groupId:<hash>}, ...]` *until* a `getUserMedia` permission is granted. Our `presets.rs:1-36` always populates `deviceId` with a deterministic hash, which fingerprinters detect via `enumerateDevices().some(d=>d.deviceId !== "")`. Cited in the W3C MediaCapture spec (§5.6.1) and FingerprintJS v4 audio component. CreepJS and Akamai BMP v13 both probe this.

**2. `Date.prototype.toString` patched for profile timezone (S, ≈25 LOC, helps ≈3-5 sites)**. Our timezone patch covers `Intl.DateTimeFormat` and `Date.getTimezoneOffset` (`window_bootstrap.js:1786-1860`) but not `Date.prototype.toString`, which still prints UTC. CreepJS cross-checks `(new Date()).toString().match(/\((.+?)\)/)` against the IANA name and flags any mismatch as a "lie".

**3. `matchMedia` full lexer for 12 standard features (M, ≈80 LOC, helps ≈3-4 sites)**. Today (`window_bootstrap.js:2892-2905`) we only handle `prefers-color-scheme: light`, `prefers-reduced-motion: no-preference`, and `(min-width: ...)`. Sites that probe `pointer:fine`, `hover:hover`, `prefers-contrast`, or `forced-colors:none` see `matches: undefined → false`, contradicting the OS-detection from `userAgent`. Profile keys for these already exist in `presets.rs:93-95`.

**4. Convert `window.scrollX/Y/pageXOffset/pageYOffset/screenX/Y` to prototype accessors (S, ≈20 LOC, helps ≈2-3 sites)**. `window_bootstrap.js:1010-1015` defines these as own data properties; real Chrome puts them on `Window.prototype` as accessor getters. FingerprintJS v4 enumerates `Object.getOwnPropertyDescriptors(window)` and any own-data property where Chrome has a prototype-accessor is a flag.

**5. Add `Notification.requestPermission`, `performance.eventCounts`, `cookieStore`, `caches`, `EyeDropper`, `IdleDetector`, `VirtualKeyboard`, `DevicePosture`, `WindowControlsOverlay`, `Document.startViewTransition` stubs (S total, ≈150 LOC, helps ≈2-4 sites)**. None individually moves the needle, but cumulatively they close ≈10 prototype-presence tells that FingerprintJS v4 and CreepJS enumerate together. Each is a 10–25-LOC stub.

**Trusted citations**: MDN per-API entries (linked above), `crates/js_runtime/tests/fixtures/chrome147/captured_macos_arm64.json` for WebGL parameter parity, `crates/browser/tests/perimeterx_surface_parity.rs` and `akamai_v13_probe_parity.rs` for the regression-locked baseline, FingerprintJS v4 source on GitHub, CreepJS source, ScrapFly's "How to bypass PerimeterX" defensive write-up.

**Honest gaps**: I could not find published Chrome 147 macOS arm64 values for `performance.eventCounts.size` initial state, exact `getClientCapabilities()` 8-key dict, `WindowControlsOverlay` PWA-mode shape — these are marked **UNKNOWN** in the per-row entries above and would require a runtime probe against a real Chrome 147 macOS arm64 install.
