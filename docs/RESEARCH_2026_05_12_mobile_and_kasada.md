# Research Synthesis — Mobile Profile, Payment Surface & Kasada VM (2026-05-12)

Synthesized from 4 parallel deep-research agents:
- iOS Safari / Android Chrome fingerprint deep-dive
- PaymentRequest / Apple Pay / Google Pay surface
- Kasada `ips.js` VM & XXTEA reverse-engineering public work
- SOTA OSS stealth toolchain audit (camoufox, patchright, nodriver, fingerprint-suite, …)

**Companion to** `docs/HANDOFF_2026_05_12.md`. Read after the handoff for context.

---

## TL;DR

1. **We are further along than the public OSS stealth ecosystem realizes.** We already ship `BatteryManager`, `NetworkInformation`, `MediaDevices.enumerateDevices`, `userAgentData` with high-entropy hints, `ApplePaySession` (macOS Chrome path, secure-context gated), `HTMLMediaElement.canPlayType` codec map, and the post-snapshot insecure-context cleanup of 30+ `[SecureContext]` interfaces. None of {camoufox, patchright, fingerprint-suite, BotBrowser, puppeteer-extra-stealth} ship the full set we have.

2. **Mobile is the right pivot for the remaining ~10 anti-bot blockers, but not as a quick fix.** A mobile profile only pays if **TLS hello, UA, UA-CH, screen, sensor presence, and 16 declined-API absences flip together**. A desktop ClientHello + mobile UA is the #1 instant flag in every Kasada/DataDome writeup. Estimated cost: 5–10 days for a credible iOS Safari profile + rquest TLS impersonation switch.

3. **The Canada Goose / Kasada blocker is profile-independent.** The CSS `calc()` `sin/cos/tan` math gap (per `kasada_real_blocker_css_calc_math.md`) and the `ao` iterator-spread probe both fire on desktop AND mobile profiles. Mobile is a force multiplier on top of fixing those, not a replacement.

4. **Our planned Kasada tools 1+2 (XXTEA key capture + dispatcher instrumentation) are NOT reinventing the wheel.** The umasii public disassembler (56★) handles only the 2021–2023 VM (58 opcodes); ours is 50, deliberately consolidated to break public tooling. No public work decrypts the XXTEA const table or traces the live dispatcher. Shipping both tools puts us 2–3 publishable blog posts ahead of the public state of art.

5. **High-confidence guess for the XXTEA key source**: the rotating noise fields `_xlq, _qnb, _ygj, _lwx, _jfk` already visible in our own decrypted blob's `__:` index. The blob field is literally a comma-separated index of these noise fields — textbook key-permutation transport.

6. **High-confidence guesses for the `ao` probe receiver** (in priority order):
   1. `document.fonts` (FontFaceSet) — has `[Symbol.iterator]` in real Chrome; commonly missed in custom engines
   2. `navigator.plugins` / `navigator.mimeTypes` (`PluginArray` / `MimeTypeArray`)
   3. `HTMLCollection` / `NodeList` from a specific query (`document.styleSheets`, `form.elements`, `document.scripts`)
   4. `URLSearchParams` iterator
   5. `CryptoKey.usages` array

7. **PaymentRequest constructor is the single highest-ROI greenfield patch.** Zero of 12 audited stealth projects ship it. ~1.5 dev-days for a minimum surface. It's already in our interfaces list (`interfaces_bootstrap.js:53`) but not wired with `canMakePayment` semantics.

---

## Current state — corrected

The OSS audit assumed gaps we don't actually have. Verified against `crates/js_runtime/src/js/`:

| Surface | Project audit assumed | Reality |
|---|---|---|
| `navigator.getBattery` / `BatteryManager` | "likely missing" | **Implemented** (`window_bootstrap.js:996–1052`), real class extends EventTarget, secure-context gated, masked toString |
| `navigator.connection` (`NetworkInformation`) | "status?" | **Implemented** (`window_bootstrap.js:110–112`), with rounded RTT/downlink |
| `MediaDevices.enumerateDevices` | "likely missing" | **Implemented** (`window_bootstrap.js:357–375`), with the two spec behaviors |
| `userAgentData` + `getHighEntropyValues` | "status?" | **Implemented** (`window_bootstrap.js:1535–1678`), secure-context gated, GREASE order matched |
| `HTMLMediaElement.canPlayType` codec map | "likely missing" | **Implemented** (`window_bootstrap.js:4946–4952`) |
| `ApplePaySession` | "0/12 ship this" | **Implemented** (`cleanup_bootstrap.js:97–139`) for macOS Chrome only, secure-context gated, post-snapshot |
| `[SecureContext]` cleanup on insecure pages | "n/a" | **Implemented** (`cleanup_bootstrap.js:27–51`), 30+ interfaces deleted |
| Generic Sensor API absent on insecure | "status?" | **Implemented** (same block) |
| `Function.prototype.toString` proxy chain | "just shipped" | **Shipped 6ef419b** — but verify cross-realm + stack-trace edge cases (see Tier 1) |

**Gaps still open**:
- `PaymentRequest.prototype.canMakePayment / show / hasEnrolledInstrument` — class is in interfaces list, no behavior wired
- `navigator.getInstalledRelatedApps` returning `Promise<[]>`
- `IdleDetector` global with proper permission gating
- `Storage Access API` (`document.requestStorageAccess`)
- `navigator.scheduling.isInputPending` (Chrome-only)
- `navigator.userActivation` (Chrome-only)
- AudioContext seeded LCG noise in Web Audio readback
- Mobile profile of any kind (iOS or Android)
- rquest mobile TLS profile parity audit vs curl-impersonate v1.x

---

## Strategic question: mobile pivot vs deeper desktop hardening

Honest tradeoff. Two reasonable paths:

**Path A — finish desktop Chrome 147, ship mobile later.** Pros: incremental, every patch lands against a known-good baseline (114/126), no risk of TLS regression. Cons: the remaining 10 blockers are concentrated on Kasada (1) + DataDome (2) + Cloudflare (1), and these vendors all give mobile a positive trust-score baseline (mobile carrier IPs, lower fingerprint diversity) that desktop can't match.

**Path B — ship mobile profile in parallel.** Pros: opens a separate code path that may pass sites desktop can't (especially DataDome and Cloudflare per Kameleo/Scrapfly writeups). Cons: requires rquest TLS profile work AND ~14 distinct fingerprint surface flips together; partial implementation is worse than no implementation (it's an inconsistency flag).

**Recommendation: parallel tracks**, not exclusive. Path A continues with PaymentRequest + getInstalledRelatedApps + IdleDetector + Function.prototype.toString edge cases (~4 dev-days, all greenfield, all additive). Path B starts with the rquest TLS audit (1 day) — if rquest already has `safari260_ios` / `chrome131_android` profiles at parity with curl-impersonate, the mobile profile is realistic this month. If not, mobile is gated on rquest upstream work and we postpone.

---

## Prioritized roadmap

Tiers ordered by ROI / blocker. Each item lists effort, confidence, and which sources support it.

### Tier 1 — Quick wins (1–3 days each, all greenfield, no UA flips)

#### 1.1 — `PaymentRequest` constructor + `canMakePayment` shim (1.5 days, high confidence)

Drop into `dom_bootstrap.js` or a new `payment_bootstrap.js`. Minimum surface from research:

```js
class PaymentRequest extends EventTarget {
  constructor(methodData, details, options = {}) {
    super();
    if (!Array.isArray(methodData) || methodData.length === 0)
      throw new TypeError("At least one payment method is required");
    if (!details || !details.total)
      throw new TypeError("'total' is required");
    Object.defineProperty(this, 'id', { value: details.id || crypto.randomUUID() ?? '0', enumerable: true });
    Object.defineProperty(this, 'shippingAddress', { value: null, enumerable: true });
    Object.defineProperty(this, 'shippingOption',  { value: null, enumerable: true });
    Object.defineProperty(this, 'shippingType',    { value: null, enumerable: true });
    this.#methods = methodData;
  }
  #methods;
  show(detailsPromise) { return Promise.reject(new DOMException("User dismissed", "AbortError")); }
  abort()              { return Promise.resolve(undefined); }
  canMakePayment()     {
    const ok = this.#methods.some(m => m.supportedMethods === 'https://google.com/pay'
                                    || m.supportedMethods === 'basic-card');
    return Promise.resolve(ok);
  }
  hasEnrolledInstrument() { return Promise.resolve(false); }   // Chrome-only; Safari must NOT have this
  static securePaymentConfirmationAvailability() {
    return Promise.resolve('unavailable-no-user-verifying-platform-authenticator');
  }
}
class PaymentResponse extends EventTarget {}
class PaymentMethodChangeEvent extends Event {
  constructor(type, init = {}) { super(type, init); this.methodName = init.methodName || ''; this.methodDetails = init.methodDetails || null; }
}
class PaymentRequestUpdateEvent extends Event {
  constructor(type, init = {}) { super(type, init); }
  updateWith(p) {}
}
```

Hygiene checklist (mirror `_defProtoMethod` pattern from `window_bootstrap.js:81–92`):
- `PaymentRequest.length === 2` (methodData, details — options has default)
- All methods produce `function name() { [native code] }` via toString
- `PaymentRequest.prototype.canMakePayment.length === 0`
- `PaymentRequest` removed from the `delete` list in `cleanup_bootstrap.js:39` (keep deleting on insecure contexts only — same as Chrome)

**Sources**: W3C Payment Request spec, MDN, GoogleChromeLabs/payment-request-shim. Zero stealth projects ship this — first to land it.

#### 1.2 — `Function.prototype.toString` cross-realm + stack-trace audit (0.5 days, high confidence)

The 05-12 commit `6ef419b` re-enabled the toString patch. fingerprint-suite's `redirectToString` reveals 4 detector edge cases:
1. `HTMLMediaElement.prototype.canPlayType.toString + ""` — must apply trap
2. `Function.prototype.toString.call(yourProxy)` — must return `function ${name}() { [native code] }`
3. Cross-realm: `iframe.contentWindow.Function.prototype.toString.apply(yourFn)` — must not throw, must not unmask
4. Stack-trace sanitization: `try { yourProxy.toString.call(undefined) } catch (e) { e.stack }` — Kasada's `p.js` reads `.stack` and grep-detects `'at Object.apply'`, `'at Object.get'`, `'at Reflect.apply'`, `'at newHandler.<computed>'`

Add 4 new tests to `chrome_compat.rs` (one per case) and patch any failures. **DataDome's own threat-research blog explicitly tests this in 2026.**

#### 1.3 — Missing Chrome-only navigator surfaces (1 day, medium confidence)

Add to `window_bootstrap.js`, all secure-context gated where Chrome gates them:
- `navigator.getInstalledRelatedApps()` → `Promise.resolve([])`
- `navigator.userActivation` → `{ hasBeenActive: true, isActive: false }` (object with the two getters)
- `navigator.scheduling.isInputPending` → `function() { return false; }`
- `IdleDetector` class with `static requestPermission()` returning `Promise<"denied">` (matches a fresh Chrome with no permission grant)

Per the audit: detection fires on **inconsistency**, not absence. Chrome 147 UA without `getInstalledRelatedApps` is more anomalous than its presence.

#### 1.4 — `MediaDevices.enumerateDevices` plausible non-empty list (1 day, high confidence)

We have the method. Verify the returned list shape against camoufox's `mediaDevices:micros/webcams/speakers` config:
- ≥1 audioinput, ≥1 audiooutput, ≥1 videoinput on Mac/Windows desktop profiles
- `deviceId === ""` (no permission granted) but `groupId` stable, non-empty, consistent across calls
- `kind` field order matches Chrome emit order
- Promise resolves (NOT rejects — headless Chromium often rejects, that's a tell)

#### 1.5 — AudioContext seeded LCG noise on AudioBuffer reads (2 days, medium confidence)

Camoufox patches this with a seeded LCG + 0.8% variance + non-linear polynomial (their own comment notes Brave's 0.1–0.2% was bypassed). Apply to:
- `AnalyserNode.getFloatFrequencyData / getByteFrequencyData / getFloatTimeDomainData / getByteTimeDomainData`
- `OfflineAudioContext.startRendering()` returned AudioBuffer's `getChannelData()`

Seed the LCG from `(profile.canvas_seed ^ 0xAFAFAFAF)` so noise is deterministic per-profile but distinct per-engine.

**Tier 1 total: ~6 dev-days, all additive, no risk of regression.**

---

### Tier 2 — Mobile profile foundation (5–10 days, gated on rquest TLS audit)

#### 2.1 — rquest mobile TLS profile audit (1 day, blocking)

Verify `rquest` ships at least:
- `safari_ios` / `safari260_ios` (per curl_cffi v0.15)
- `chrome_android` / `chrome131_android` (per curl_cffi)

If gaps exist, file rquest issues OR backport curl-impersonate v1.x mobile profiles. **Without TLS parity, all of Tier 2 is wasted effort.**

Test methodology: capture our ClientHello vs a real iPhone Safari hello (use SSL Labs' test endpoint or run against `tls.peet.ws`), diff cipher order, extension order, ALPS behavior, `application_settings` extension presence/absence, X25519MLKEM768 priority.

#### 2.2 — `StealthProfile` schema extension for `device_class` (0.5 days)

Add `device_class: enum { Desktop, MobileIOS, MobileAndroid }` to the profile struct. Existing `os_name` already drives some branching; this makes mobile-vs-desktop a first-class axis. Update `op_get_profile_value` callers to consult it.

#### 2.3 — iOS Safari profile bundle (3 days, high confidence)

Single `iphone_15_pro_safari_18.json` profile that flips:

| Surface | Value |
|---|---|
| UA | `Mozilla/5.0 (iPhone; CPU iPhone OS 18_0_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0.1 Mobile/15E148 Safari/604.1` |
| `userAgentData` | **undefined** (Safari doesn't ship UA-CH) — adjust `window_bootstrap.js:1678` gate |
| Sec-CH-UA-* headers | **none sent** by rquest |
| `navigator.platform` | `"iPhone"` |
| `navigator.maxTouchPoints` | `5` |
| `navigator.deviceMemory` | **undefined** (WebKit doesn't expose it) |
| `navigator.hardwareConcurrency` | `2` |
| `navigator.vendor` | `"Apple Computer, Inc."` |
| `navigator.plugins` | empty PluginArray (length 0) |
| `navigator.pdfViewerEnabled` | `false` |
| `navigator.connection` | **undefined** (Safari doesn't expose NetworkInformation) |
| `screen.width × height` | `393 × 852` |
| `devicePixelRatio` | `3` |
| `window.orientation` | `0` (legacy iOS-only — desktop browsers do NOT have it) |
| `'ontouchstart' in window` | `true` |
| `TouchEvent` / `Touch` / `TouchList` constructors | present |
| WebGL `UNMASKED_VENDOR_WEBGL` | `"Apple Inc."` |
| WebGL `UNMASKED_RENDERER_WEBGL` | `"Apple GPU"` (literal constant, no model info) |
| AudioContext sampleRate | `48000` |
| `DeviceMotionEvent.requestPermission` | static method returning `Promise<"denied">` |
| `DeviceOrientationEvent.requestPermission` | static method returning `Promise<"denied">` |
| `ApplePaySession` | **always present** (every Safari has it) |
| `PaymentRequest.prototype.hasEnrolledInstrument` | **must NOT exist** (Safari doesn't ship it) |
| Permissions API supported names | restrict to `geolocation, notifications, camera, microphone, persistent-storage` |

#### 2.4 — Strip 16 declined APIs for iOS profile (1 day, high ROI)

When `device_class === MobileIOS`, delete from globalThis in cleanup_bootstrap (post-snapshot):

```
Bluetooth, USB, USBAlternateInterface, USBConfiguration, USBConnectionEvent,
USBDevice, USBEndpoint, USBInTransferResult, USBInterface,
USBIsochronousInTransferPacket, USBIsochronousInTransferResult,
USBIsochronousOutPacket, USBIsochronousOutTransferResult, USBOutTransferResult,
HID, HIDConnectionEvent, HIDDevice, HIDInputReportEvent,
Serial, SerialPort,
NetworkInformation,
IdleDetector,
Sensor, Accelerometer, AbsoluteOrientationSensor, GravitySensor, Gyroscope,
LinearAccelerationSensor, Magnetometer, OrientationSensor,
RelativeOrientationSensor,
GPU (WebGPU — feature-flagged on iOS 18+, simpler to absent),
```

Plus from `navigator`:
- `bluetooth, usb, serial, hid, requestMIDIAccess, getBattery` (entire BatteryManager API)
- Replace `connection` getter with one returning `undefined`

This is the **single highest-ROI mobile patch** — dozens of leaks vanish at once.

#### 2.5 — Android Chrome profile bundle (2 days, parallel to iOS)

`pixel_9_pro_chrome_147.json`:

| Surface | Value |
|---|---|
| UA | `Mozilla/5.0 (Linux; Android 15; Pixel 9 Pro Build/AP4A.250105.002) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Mobile Safari/537.36` |
| `Sec-CH-UA-Mobile` | `?1` |
| `Sec-CH-UA-Platform` | `"Android"` |
| `Sec-CH-UA-Model` | `"Pixel 9 Pro"` |
| `Sec-CH-UA-Form-Factors` | `"Mobile"` |
| `Sec-CH-UA-Platform-Version` | `"15.0.0"` |
| `userAgentData.mobile` | `true` |
| `navigator.platform` | `"Linux armv81"` |
| `navigator.maxTouchPoints` | `5` |
| `navigator.deviceMemory` | `8` |
| `navigator.hardwareConcurrency` | `8` |
| `navigator.plugins` | **empty PluginArray** (this is the single biggest mobile-vs-desktop tell on Chromium) |
| `navigator.pdfViewerEnabled` | `false` |
| `screen.width × height` | `412 × 870` |
| `devicePixelRatio` | `2.625` (fractional — **don't round to 2 or 3**) |
| WebGL renderer | `"ANGLE (Google, Mali-G715 MP7, OpenGL ES 3.2)"` |
| `'ontouchstart' in window` | `true` |
| Generic Sensor API constructors | **present** (Accelerometer/Gyroscope/etc — Android Chrome has them; iOS doesn't) |
| `DeviceMotionEvent.requestPermission` | **must NOT exist** (Android grants implicitly — that static is iOS-only) |
| `navigator.connection` | present, `effectiveType: "4g"`, `downlink: 10`, `rtt: 50` |
| `ApplePaySession` | absent |

#### 2.6 — Mobile-aware sweep (0.5 days)

Add `holistic_sweep_parallel_mobile_ios` and `holistic_sweep_parallel_mobile_android` test entry points. Run all three sweeps next session. Distinguish "blocked everywhere" sites from "blocked desktop, passes mobile" sites — that's the empirical mobile-bypass evidence.

**Tier 2 total: 7–8 dev-days. Gated on Tier 2.1 (rquest audit). If rquest has the profiles, ship within 2 weeks.**

---

### Tier 3 — Kasada-specific tooling (2–4 days, refined plan)

The handoff already plans Tool 1 (XXTEA key capture) and Tool 2 (dispatcher instrumentation). Research adds three concrete refinements:

#### 3.1 — Tool 1 refinement: try the noise fields as the XXTEA key first (0.5 days)

Before instrumenting `new Function`, try the static-decryption shortcut:
- Capture `__: "_xlq,_qnb,_ygj,_lwx,_jfk,..."` from a fresh blob
- Hash these field names (try SHA-1, SHA-256 truncated to 16 bytes; try concatenating the field VALUES and hashing; try XOR of the values)
- Try the resulting key against the const-table ciphertext from `kasada_function_bodies.js` opcode #5

If this works, Tool 1 collapses from "1–2 days dynamic instrumentation" to "1 hour Python script". The blob structure literally indexes those fields in `__:` — that is overwhelmingly likely the key derivation.

If it doesn't work, fall back to the planned `new Function` hook. Either way: 0.5 day spent here is cheap insurance.

#### 3.2 — Tool 2 refinement: port umasii's static disassembler to our 50-opcode table (1 day)

[github.com/umasii/ips-disassembler](https://github.com/umasii/ips-disassembler) ships:
- Base-62 bytecode decoder
- 58 named opcodes for the 2021–2023 VM
- String-table extraction via `bytecode[len-1] XOR len`

Our VM has 50 opcodes — Kasada deliberately consolidated them post-publication to break this tool. **Diff our opcode bodies against umasii's table by index** — opcodes 0x00–0x16 (arithmetic, control flow) are likely unchanged. Divergence point names which slots Kasada repurposed.

The dispatcher instrumentation (the planned Tool 2) then logs against the named opcode table instead of raw indices. Much higher-quality output for the same effort.

#### 3.3 — Tool 3 (NEW): sensor-blob field decoder (0.5 days)

The research agent partially decoded our blob's field taxonomy from context:

| Field | Likely meaning |
|---|---|
| `ao` | argument-spread override (iterator probe) |
| `wse` | window string eval (Function.prototype.toString probe) |
| `wgp` | webgl renderer/vendor parameters |
| `nl` | navigator.languages |
| `npf` | navigator.plugins fingerprint |
| `nppm` | navigator + structured-clone probe |
| `bfe` | browser Function.toString error in iframe |
| `fsc` | Function.toString subclass error |
| `wdt` | webdriver test |
| `ifw` | inframe window checks |
| `puam` | permission/UA-CH/multiple |
| `dpi` | devicePixelRatio setter/getter probe |
| `crs` | crypto.random stream |
| `ifc` | iframe consistency |
| `sas` | system as (UA/platform/timezone) |
| `cnf` | config (JSON.stringify+eval natives) |
| `hsp` | high-resolution timer probe |
| `tnp` | timing (perf.now) probe |

Document these in a new `docs/kasada_ips_analysis/sensor_field_taxonomy.md`. **No public source has this.** Useful as a punch-list: every field with `e:1` or `t:1` (suspicious score) is a probe we're failing.

**Tier 3 total: 2 days, drastically de-risked vs the original plan.**

---

### Tier 4 — `ao` receiver hunt (1 day, blocked on Tier 3)

Once Tool 2 is logging opcode + register snapshots, the `ao` receiver is identifiable in **one instrumented run**: the `GET WINDOW PROP` opcode immediately before the failing `CALL FUNCTION` at the `__values`/`__spread` site names the receiver.

If Tool 1 cracked the XXTEA key statically, we don't even need to wait for Tool 2 — the const table will contain the property name string, decoded once.

**Pre-flight tests** (before instrumentation, to narrow the search space): add `chrome_compat.rs` checks that `[...x]` succeeds on Chrome and our engine for each candidate:

```js
[...document.fonts]                       // FontFaceSet
[...document.styleSheets]                 // StyleSheetList
[...navigator.plugins]                    // PluginArray
[...navigator.mimeTypes]                  // MimeTypeArray
[...new URLSearchParams("a=1&b=2")]       // URLSearchParams
[...crypto.subtle.generateKey(/*...*/).then(k => k.usages)]
[...document.head.attributes]             // NamedNodeMap
[...document.head.children]               // HTMLCollection
[...new FormData(document.createElement('form')).keys()]
[...new Headers().keys()]
[...performance.getEntriesByType('navigation')]
```

Any one that throws on us but works in Chrome is a candidate. The 05-12 work landed `DOMTokenList` iterability and iterator self-iterability — `document.fonts` (FontFaceSet) is the most likely remaining miss given it's analogous (collection-like with custom iterator).

---

### Tier 5 — Watch-list (no immediate action)

- **camoufox 2026 Q2 commits** ([daijro/camoufox](https://github.com/daijro/camoufox)) — `Disable Canvas Noise` toggle (#528, 04-15), system-ui font spoofing (#599, 05-08). They're working the same surfaces we are; cherry-pick ideas.
- **rebrowser-patches** for Runtime.enable — n/a to us (no CDP exposure). Document this as a **structural advantage**: every Playwright/Puppeteer-based stealth library is fighting a leak we don't have.
- **Closed shadow-root piercing detection** — same. We don't expose a remote-control surface.
- **kpsdk-solver explicit recommendation: Firefox + Windows for Kasada bypass** — Kasada's bot 1225 detector reportedly treats Firefox more leniently. Worth considering a "Firefox 132 Windows" stealth profile alongside Chrome 147 and the mobile profiles. Lower-priority.
- **patchright (3K★) claims Kasada bypass** but the patches are CDP-layer (Runtime.enable, isolated worlds) — irrelevant for our architecture.

---

## What NOT to do

- **Don't ship a partial mobile profile.** A mobile UA with `navigator.deviceMemory: 8`, `userAgentData` present, and `'ontouchstart' in window: false` is more anomalous than a honest desktop profile. Either flip ALL ~14 surfaces in section 2.3/2.5 atomically, or don't ship mobile at all.
- **Don't expose `ApplePaySession` under a Chrome UA.** The cardinal sin. Our current `cleanup_bootstrap.js:97–139` correctly gates on `_osName === "macOS"` — preserve this.
- **Don't pile more reactive Web API patches without identifying probe targets first.** The 05-12 lesson: every patch (`DOMTokenList` iter, iterator self-iter, toString) was correct and additive, but didn't move the Kasada flag count because we were attacking shadows. Tier 3+4 → ground truth → THEN Tier 1.5 patches as needed.
- **Don't drop the `Function.prototype.toString` patch again.** Tier 1.2 is hardening, not removal.
- **Don't `npm install` anything.** This codebase is workspace-Rust + bundled JS only. fingerprint-suite reference code is to be **read for patterns**, not vendored.

---

## Empirical validation plan

Independent of which tier we ship first, every change should be measured against:

1. **Parallel sweep** (`holistic_sweep_parallel`): canonical metric. 114/126 baseline. Mobile sweep variants (Tier 2.6) for cross-profile comparison.
2. **`kasada_error_blob_capture`**: count of `e:1` flags per probe. Currently 6. Each `ao`/`bfe`/`fsc`/etc. probe that drops to `e:0` is a measurable win.
3. **CreepJS / browserleaks / pixelscan / iphey / browserscan** integration tests (one per: add `#[ignore]` smoke tests like the existing kasada captures). These public scoring sites give a numeric fingerprint quality score we can chart over time.
4. **5-run sweep variance** (Tool 3 from handoff, 2hrs): characterize ±2 noise so single-commit improvements are distinguishable.

---

## Appendix A — Per-UA presence/absence matrix

For the Tier 1.3 audit. `Y` = expose, `N` = must be absent, `~` = present but specific behavior required.

| Surface | Chrome 147 Win/Mac/Linux | Chrome 147 Android | Safari 18 macOS | Safari 18 iOS | Firefox 132 |
|---|:-:|:-:|:-:|:-:|:-:|
| `PaymentRequest` | Y | Y | Y | Y | **N** |
| `PaymentRequest.hasEnrolledInstrument` | Y | Y | **N** | **N** | n/a |
| `ApplePaySession` | **N** (except macOS Safari user-installed Chrome share) | **N** | **Y** | **Y** | **N** |
| `userAgentData` | Y | Y | **N** | **N** | **N** |
| `getInstalledRelatedApps` | Y | Y | **N** | **N** | **N** |
| `IdleDetector` | Y | Y | **N** | **N** | **N** |
| `BatteryManager` / `getBattery` | Y | Y | **N** | **N** | Y (limited) |
| `Bluetooth` | Y (desktop) | Y | **N** | **N** | **N** |
| `USB` / `Serial` / `HID` | Y (desktop) | partial | **N** | **N** | **N** |
| `NetworkInformation` (`navigator.connection`) | Y | Y | **N** | **N** | partial |
| `Generic Sensor API` (Accelerometer etc.) | Y (HTTPS+permission) | Y | **N** | **N** | **N** |
| `DeviceMotionEvent.requestPermission` static | **N** | **N** | **Y** | **Y** | **N** |
| `window.orientation` (legacy) | **N** | **N** | **N** | **Y** | **N** |
| `MediaDevices.enumerateDevices` returns ≥1/kind | Y | Y | Y | Y | Y |
| `navigator.userActivation` | Y | Y | **N** | **N** | Y |
| `navigator.scheduling.isInputPending` | Y | Y | **N** | **N** | **N** |
| `WebGPU` (`navigator.gpu`) | Y | Y | partial (18+) | partial (18+) | partial |
| `navigator.plugins` non-empty PDF set | Y | **empty** | Y (different from Chrome) | empty | empty |

**Cardinal rules** (anti-bot cross-checks):
1. TLS JA4 ↔ UA ↔ Sec-CH-UA-Platform — must agree on iOS/Android
2. UA mobile token ↔ Sec-CH-UA-Mobile: ?1 ↔ navigator.userAgentData.mobile (Chromium only)
3. `navigator.maxTouchPoints > 0` ↔ `'ontouchstart' in window` ↔ TouchEvent constructor exists
4. `screen.width/height` ↔ CSS media query results ↔ `window.innerWidth × devicePixelRatio` (within tolerance)
5. `navigator.platform` ↔ UA OS token (`"iPhone"` requires `iPhone` in UA; `"Linux armv81"` requires Android in UA)
6. Accept-Language ↔ `navigator.languages[0]`
7. `'ApplePaySession' in window` ↔ UA contains `Safari` AND NOT `Chrome|Chromium|CriOS|FxiOS|EdgiOS`

---

## Appendix B — Mobile profile constants (canonical values)

### iPhone 15 Pro / Safari 18.0.1
- UA: `Mozilla/5.0 (iPhone; CPU iPhone OS 18_0_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0.1 Mobile/15E148 Safari/604.1`
- Build token: `Mobile/15E148` (constant — do NOT randomize)
- Screen: 393 × 852
- DPR: 3
- maxTouchPoints: 5
- hardwareConcurrency: 2
- deviceMemory: undefined
- AudioContext sampleRate: 48000
- WebGL vendor/renderer: `"Apple Inc."` / `"Apple GPU"`

### iPhone 15 / Safari (non-Pro)
- Same UA family, screen 393 × 852, AV1 hardware decode = false (Pro-only)

### iPhone 15 Plus / Pro Max
- Screen 430 × 932, DPR 3, otherwise same

### iPhone 16 Pro
- Screen 402 × 874, DPR 3

### iPhone 16 Pro Max
- Screen 440 × 956, DPR 3

### Pixel 9 Pro / Chrome 147
- UA: `Mozilla/5.0 (Linux; Android 15; Pixel 9 Pro Build/AP4A.250105.002) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Mobile Safari/537.36`
- Sec-CH-UA-Model: `"Pixel 9 Pro"` (display name, not codename `tokay`)
- Screen: 412 × 870
- DPR: 2.625 (**fractional — keep it**)
- maxTouchPoints: 5
- hardwareConcurrency: 8
- deviceMemory: 8
- WebGL: `"ANGLE (Google, Mali-G715 MP7, OpenGL ES 3.2)"`
- platform: `"Linux armv81"`

### Pixel 8 / Chrome 147
- Same UA family, model `"Pixel 8"`, screen 412 × 915, DPR 2.625
- WebGL: `"ANGLE (Google, Mali-G715 MC11, OpenGL ES 3.2)"` (Tensor G3, MC11 not MP7)

---

## Appendix C — Kasada sensor field taxonomy (partial)

Document at `docs/kasada_ips_analysis/sensor_field_taxonomy.md`. Each field appears in the decrypted `/tl/error` blob. Score format: `{"e": 1, "r": "<error>", "b": 1, "t": <int>}` where `e=1` means probe failed, `b=1` means contributes to block decision, `t` is per-probe trust score.

Known fields (high confidence): `ao, wse, wgp, nl, npf, nppm, bfe, fsc, wdt, ifw, puam, dpi, crs, ifc, sas, cnf, hsp, tnp` (18 of ~123 in our blob). Goal: reach 60+ documented, then it's Kasada-research publishable.

---

## Sources (consolidated)

### Mobile fingerprint
- [WebKit Safari 26 fingerprinting protection](https://webkit.org/blog/16993/news-from-wwdc25-web-technology-coming-this-fall-in-safari-26-beta/)
- [CSS-Tricks — Apple's 16 declined web APIs](https://css-tricks.com/apple-declined-to-implement-16-web-apis-in-safari-due-to-privacy-concerns/)
- [Castle.io — WebGL renderer fingerprinting](https://blog.castle.io/the-role-of-webgl-renderer-in-browser-fingerprinting/)
- [Niels Leenheer — Safari iOS 26 user agent](https://nielsleenheer.com/articles/2025/the-user-agent-string-of-safari-on-ios-26-and-macos-26/)
- [Privacy Sandbox — UA reduction Android model](https://privacysandbox.google.com/blog/user-agent-reduction-android-model-and-version)

### Payment surface
- [W3C Payment Request API](https://www.w3.org/TR/payment-request/)
- [W3C Secure Payment Confirmation](https://w3c.github.io/secure-payment-confirmation/)
- [WebKit ApplePaySession.h source](https://github.com/WebKit/webkit/blob/main/Source/WebCore/Modules/applepay/ApplePaySession.h)
- [Apple Pay JS change log](https://developer.apple.com/documentation/applepayontheweb/apple-pay-js-change-log)
- [justeat/applepayjs-polyfill](https://github.com/justeat/applepayjs-polyfill) (BSD-licensed reference)
- [GoogleChromeLabs/payment-request-shim](https://github.com/GoogleChromeLabs/payment-request-shim)

### Kasada VM RE
- [umasii/ips-disassembler](https://github.com/umasii/ips-disassembler) (Tier S)
- [Humphryyy/Kasada-Deobfuscated](https://github.com/Humphryyy/Kasada-Deobfuscated) (deobed control sample)
- [jtwmyd/kasada-dissembler](https://github.com/jtwmyd/kasada-dissembler) (cross-check opcodes)
- [nullpt.rs — Devirtualizing Nike's VM Pt 1](https://nullpt.rs/devirtualizing-nike-vm-1) / [Pt 2](https://nullpt.rs/devirtualizing-nike-vm-2)
- [lktop/kpsdk](https://github.com/lktop/kpsdk) (2021 vintage diff)
- [Hyper-Solutions/hyper-sdk-go](https://github.com/Hyper-Solutions/hyper-sdk-go) (closed but confirms protocol)

### OSS stealth toolchain
- [daijro/camoufox](https://github.com/daijro/camoufox) (8K★, Firefox-fork, deepest patch set)
- [Kaliiiiiiiiii-Vinyzu/patchright](https://github.com/Kaliiiiiiiiii-Vinyzu/patchright) (3K★, claims Kasada)
- [rebrowser/rebrowser-patches](https://github.com/rebrowser/rebrowser-patches) (Runtime.enable canonical fix)
- [ultrafunkamsterdam/nodriver](https://github.com/ultrafunkamsterdam/nodriver) (4K★, undetected-chromedriver successor)
- [apify/fingerprint-suite](https://github.com/apify/fingerprint-suite) (only mobile-aware project)
- [daijro/browserforge](https://github.com/daijro/browserforge) (Python reimpl, better iOS than upstream)
- [botswin/BotBrowser](https://github.com/botswin/BotBrowser) (Chromium fork, per-context fingerprint)
- [lexiforest/curl_cffi](https://github.com/lexiforest/curl_cffi) (TLS profiles incl. safari260_ios)
- [bogdanfinn/tls-client](https://github.com/bogdanfinn/tls-client) (Go, broader iOS coverage)

### Anti-bot writeups
- [Scrapfly — How to bypass Kasada 2026](https://scrapfly.io/blog/posts/how-to-bypass-kasada-anti-scraping-waf)
- [ZenRows — Kasada bypass](https://www.zenrows.com/blog/kasada-bypass)
- [The Web Scraping Club — Lab #76 Kasada 2025](https://substack.thewebscraping.club/p/bypassing-kasada-2025-open-source)
- [Kameleo — DataDome bypass guide](https://kameleo.io/blog/guide-to-bypassing-datadome)
- [DataDome — End of fingerprinting 2025-12](https://datadome.co/threat-research/end-of-fingerprinting-how-browser-privacy-reshaping-bot-detection/)
- [Castle — Puppeteer to nodriver evolution](https://blog.castle.io/from-puppeteer-stealth-to-nodriver-how-anti-detect-frameworks-evolved-to-evade-bot-detection/)
