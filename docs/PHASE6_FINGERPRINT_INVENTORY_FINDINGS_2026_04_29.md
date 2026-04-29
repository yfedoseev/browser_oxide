# Phase 6 — Fingerprint inventory: actionable findings

> Companion executive-summary to
> `docs/CHROME_FINGERPRINT_FULL_INVENTORY_2026_04_29.md` (1,224 lines).
> The full doc enumerates ~110 properties across 15 categories with
> per-row spec/MDN links, current oxide emission, gap class, and fix.
> This file extracts the **actionable** findings from that work for
> the next session and the handoff.

## Top 7 highest-ROI gaps (ranked)

These are the gaps where a small JS shim change closes a fingerprint
tell visible to common detection libraries. All are pure parity
work — no policy review needed.

### 1. `mediaDevices.enumerateDevices()` leaks `deviceId` pre-permission

- **Real Chrome**: returns array of devices with empty `deviceId` /
  `groupId` / `label` until the page calls `getUserMedia()` and the
  user grants permission.
- **browser_oxide**: populates `deviceId` from the profile
  immediately. `presets.rs:1-36` ships labelled devices.
- **Fix**: blank `deviceId` and `label` in `enumerateDevices()` until
  a permission has been recorded. Easy — wrap the existing function
  in `window_bootstrap.js`.
- **Detected by**: FingerprintJS open-source, BrowserLeaks media-
  devices probe.

### 2. `Date.prototype.toString` still prints UTC despite TZ patch

- **Real Chrome on macOS / America/Los_Angeles**:
  `"Tue Apr 29 2026 13:02:46 GMT-0700 (Pacific Daylight Time)"`
- **browser_oxide**: prints `GMT+0000`. Our patch at
  `window_bootstrap.js:1830-1860` only overrides
  `getTimezoneOffset()` and `Intl.DateTimeFormat.resolvedOptions().timeZone`,
  not the legacy `Date` toString format.
- **Fix**: extend the TZ patch to also override `Date.prototype.toString`,
  `Date.prototype.toLocaleString` (default), `Date.prototype.toTimeString`,
  `Date.prototype.toDateString`. Compute the offset string from the
  profile's timezone.
- **Detected by**: every fingerprint library — `new Date().toString()`
  is one of the cheapest probes.

### 3. `matchMedia` only covers 3 of ≈12 standard features

- **Real Chrome**: supports `prefers-color-scheme`, `prefers-reduced-motion`,
  `prefers-contrast`, `prefers-reduced-data`, `prefers-reduced-transparency`,
  `inverted-colors`, `pointer:{none,coarse,fine}`, `hover:{none,hover}`,
  `any-pointer`, `any-hover`, `forced-colors`, plus dimension queries
  (min-width, min-height, etc.).
- **browser_oxide**: handles only `prefers-color-scheme`,
  `prefers-reduced-motion`, dimension queries — at
  `window_bootstrap.js:2892-2905`.
- **Fix**: extend the matcher to handle the 9 missing features.
  Use the profile to drive default values
  (`prefers-color-scheme: light`, `pointer: fine`, `hover: hover`,
  `forced-colors: none` for desktop).

### 4. `scrollX/Y / pageX/YOffset / screenX/Y` are own data properties

- **Real Chrome**: these are accessor properties on `Window.prototype`
  with `[[Get]]` returning the current scroll position, NOT data
  properties on the `window` instance.
- **browser_oxide**: assigns them as own data properties at
  `window_bootstrap.js:1010-1015`. So
  `Object.getOwnPropertyDescriptor(Window.prototype, 'scrollX')`
  returns `undefined` (Chrome returns a getter descriptor); and
  `Object.getOwnPropertyDescriptor(window, 'scrollX')` returns a
  data descriptor (Chrome returns nothing because it's inherited).
- **Fix**: define each as a getter on `Window.prototype` instead of
  an own value on `globalThis`. Mirror the pattern used for
  `innerWidth`/`innerHeight` (which are getter accessors).

### 5. `chrome.runtime` exposed on regular pages

- **Real Chrome**: `window.chrome` exists on every page, but
  `window.chrome.runtime` ONLY exists inside extension contexts.
  On a normal `https://example.com` page,
  `'runtime' in chrome` returns `false`.
- **browser_oxide**: `interfaces_bootstrap.js:144-146` defines
  `chrome.runtime.OnInstalledReason`, making `runtime` always
  present.
- **Fix**: gate `chrome.runtime` definition behind a "is this an
  extension context?" check. For regular pages, omit `runtime`
  entirely. Keep `chrome.app`, `chrome.csi`, `chrome.loadTimes`,
  `chrome.webstore`.
- **Detected by**: ScrapFly's PerimeterX bypass guide, FingerprintJS,
  CreepJS — every modern detection library checks
  `'runtime' in chrome`.

### 6. `WebTransport` throws synchronously instead of async-rejecting

- **Real Chrome**: `new WebTransport(url)` returns an instance whose
  `ready` Promise rejects asynchronously when the URL is unreachable.
- **browser_oxide**: `interfaces_bootstrap.js:73` synchronously throws.
- **Fix**: return an instance with `ready: Promise.reject(...)`,
  `closed: Promise.reject(...)`, and stub method shapes.

### 7. Big batch of missing constructors

These all need stub constructors with realistic shapes (most are
"present-but-doesn't-work" surfaces — Chrome exposes them but they
fail when actually invoked):

| API | What's missing | Detection |
|---|---|---|
| `globalThis.cookieStore` | Async cookie API | FingerprintJS |
| `globalThis.caches` | CacheStorage | tested by service-worker probes |
| `performance.eventCounts` | Map of input event counts | CreepJS |
| `Notification.requestPermission` | Promise-returning permission ask | every library |
| `Document.startViewTransition` | View Transitions API | recent Chrome detection |
| `IdleDetector` | User idle state | rare but checked |
| `EyeDropper` | Color picker constructor | rare |
| `VirtualKeyboard` | `navigator.virtualKeyboard` | mobile-class probe |
| `DevicePosture` | `navigator.devicePosture` | foldable probe |
| `WindowControlsOverlay` | `navigator.windowControlsOverlay` | PWA-mode probe |

## 13 surfaces where browser_oxide can exceed Playwright

Playwright is bound to whatever its Chromium binary actually does —
which has known headless quirks. As a from-scratch engine with full
control of the JS layer, we can emit Chrome-class signals **exactly
as the spec says they should be**, even where the real Chromium
implementation has bugs / quirks. The doc identifies these surfaces
where we are *already* more accurate, plus new opportunities.

**Already more accurate than Playwright** (these are inherent
advantages of from-scratch engine):

1. `BatteryManager` class identity — Playwright's headless Chromium
   may return a plain object; we ship a real `class extends
   EventTarget`.
2. mDNS-anonymized ICE host candidate — Playwright headless emits
   `null` only; we emit one realistic `<uuid>.local typ host`.
3. `userAgentData` GREASE order — Playwright defaults; we rotate
   GREASE per construction the way real Chrome does.
4. V8-correct heap ceiling — we set 4 GB (matches macOS arm64 Chrome);
   headless Chrome OOMs around 1.8 GB for fingerprint-heavy pages.

**New opportunities surfaced this session** (where the inventory
agent identified we *can* be more accurate):

5. `enumerateDevices()` deviceId blanking pre-permission — our
   profile-driven device table can be blanked on read; Playwright is
   stuck with whatever Chromium decides.
6. Per-realm timezone — our V8 isolate per page can carry the
   profile's TZ; Playwright is stuck with the host's TZ.
7. `audioWorklet` non-null — the worklet object exists in real
   Chrome; Playwright headless may have it null.
8. Per-profile TLS impersonation — we have separate macOS / Windows /
   Linux JA4 profiles; Playwright's TLS is the host's libssl.
9. Per-profile HTTP/3 control — we can gate H3 per profile;
   Playwright doesn't expose this knob.
10. `Sec-CH-UA` per-request rotation — we can mirror Chrome's
    behaviour where some requests get the full version list and
    others get the brief; Playwright sends the same on every request.
11. `availTop=25` on macOS — accounts for the menu bar; Playwright
    on Linux doesn't have a menu bar so reports 0, breaking macOS
    profile parity.
12. Synthesized `pageshow`/`pagehide` timing — we control event
    dispatch precisely; Playwright is bound to Chromium's lifecycle.
13. `crypto.subtle` algorithm set match — we can guarantee Chrome's
    exact list; Playwright depends on the bundled BoringSSL build.

## Already-closed gaps (verified, no rework needed)

The agent verified these are already MATCH state from prior commits:

- `BatteryManager` is a real class extending `EventTarget`
  (`window_bootstrap.js:725-748`)
- `MediaSession` has full method set including
  `setActionHandler` / `setPositionState` / `setCameraActive`
  (`window_bootstrap.js:4168-4226`)
- mDNS-anonymized ICE host candidate emitted before null terminator
  (`window_bootstrap.js:3825-3858`)
- `OfflineAudioContext` shim with deterministic seeded AudioBuffer
  (`canvas_bootstrap.js:541-895`)
- `Storage.estimate()` ships `usageDetails` shape
  (`window_bootstrap.js:560-572`)
- 36-extension WebGL parity (Chrome 147 macOS arm64 captured fixture)
- `VisualViewport` / `InputDeviceCapabilities` / `MediaSession` —
  shipped today
- Native-code masking — `Function.prototype.toString.call(setTimeout)`
  etc. all return `[native code]`
- `Touch.prototype` `Symbol.toStringTag = "Touch"`
- userAgentData GREASE literal `"Not.A/Brand"` (current Chrome value)

## UNKNOWN — needs runtime probe before fixing

The agent honestly flagged where published sources are insufficient:

| Item | Why UNKNOWN | How to resolve |
|---|---|---|
| `performance.eventCounts.size` initial state | No public source documents the initial counter values | Probe a fresh real Chrome page via Playwright MCP |
| `Notification.requestPermission` Promise-vs-callback shape | Spec is hybrid; Chrome's exact dispatch depends on call form | Probe via MCP with both call shapes |
| `getClientCapabilities()` 8-key dict on Chrome 147 | New API (Chrome 133+); shape undocumented in MDN | Probe via MCP |
| `Accept-Language` / `Accept-Encoding` / `Sec-Fetch-*` byte-exact strings | Chrome internals, no public spec listing | Compare against `crates/net/src/headers.rs` and a captured MCP request |
| `WindowControlsOverlay` PWA-mode shape | Only present in installed PWA contexts | Out of scope for this session |

## Recommended next-session work, by ROI

| Day | Item | Sites unlocked (estimate) | Effort |
|---:|---|---:|---|
| 1 | Fix `Date.prototype.toString` TZ format + extend matchMedia features | 0 directly, but cross-site improvement on TZ-checking sensors | 0.5 day |
| 1 | Blank `enumerateDevices()` deviceId/label pre-permission | 0 directly, FingerprintJS surface tightener | 0.5 day |
| 2 | Promote `scrollX/Y/pageX/YOffset/screenX/Y` to `Window.prototype` accessors | 0 directly, but resolves a structural-property descriptor probe | 0.5 day |
| 2 | Gate `chrome.runtime` to extension contexts only (omit on regular pages) | 1-2 (PerimeterX-protected sites probe this) | 0.5 day |
| 3 | Add the missing-constructor batch (`cookieStore`, `caches`, `IdleDetector`, etc.) | 0 directly, broad-spectrum tightener | 1 day |
| 3 | Run probe against MCP for the 5 UNKNOWN items, lock answers in `crates/browser/tests/perimeterx_surface_parity.rs` | 0 directly, regression-locks | 0.5 day |

**Realistic uplift from these 6 days: +1-3 sites** (modest — we're already at the ceiling for this corpus). The bigger value is **defense-in-depth**: each fix removes a single-probe detection, which compounds on sites that combine many signals into a score.

## Cross-references

- Full inventory: `docs/CHROME_FINGERPRINT_FULL_INVENTORY_2026_04_29.md`
- Prior parity catalog: `docs/CHROME_JS_SURFACE_PARITY_2026_04_29.md`
- Per-site recoverability: `docs/PHASE5_RECOVERABILITY_ANALYSIS_2026_04_29.md`
- 5-tool comparison: `docs/COMPARISON_5_TOOLS_2026_04_29.md`
- Session handoff: `docs/HANDOFF_2026_04_29_session_close.md`

## Session commit arc

```
0c700d2  docs: comprehensive Chrome fingerprint surface inventory (1224 lines)
2df40fa  docs(phase5): per-site recoverability analysis via Playwright MCP
c667d32  docs(2026-04-29): handoff + 5-tool comparison + design docs
77ecc5b  feat(stealth): Chrome JS-surface parity + sigma-lognormal humanizer + classifier rewrite
b3bc634  feat(net,browser,js_runtime): full CSP enforcement w/ strict-dynamic + SPV event
```

Six commits including this one will live on `origin/main`.
