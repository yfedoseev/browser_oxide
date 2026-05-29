# DETECT_vectors — Master detection-vector catalog (open detectors)

**Scope:** source-of-truth enumeration of every fingerprinting / headless-detection
vector probed by the open detectors (CreepJS, apify fingerprint-suite / BotD,
BrowserScan, sannysoft, areyouheadless / intoli, pixelscan, browserleaks), each
mapped to BO's current coverage with `file:line` and a leak verdict, ending in a
ROI-ranked fix list. This is the reference for the `areyouheadless` diagnostic and
all Web-API parity work.

**Status:** new (2026-05-28). Extends the repo's existing audits — does NOT duplicate them.
**Read first (existing repo conclusions, cited throughout):**
- `docs/releases/v0.1.0-parity/16_STEALTH_FINGERPRINT_AUDIT.md` — the masking
  architecture + the full `_maskAsNative` audit table (§2.2) and the ranked
  `Function.toString` leak list (§2.3). This catalog is the *detector-driven* view
  of that same surface.
- `docs/releases/v0.1.0-parity/17_WEB_API_PARITY_MATRIX.md` — implemented-vs-missing APIs.
- `docs/releases/v0.1.0-parity/audit/15_FIX_PRIORITY_RANKED.md` — current FIX backlog (FIX-A..K, J, G).
- `docs/HANDOFF_2026_05_28b.md` — latest state (AWS = self-solve-execution gap, not fingerprint).
- `docs/releases/v0.1.0-parity/38_VISUAL_AUDIO_FINGERPRINTING.md`, `39_NETWORK_LAYER_FINGERPRINTING.md`.

---

## 0. The two-axis model every detector uses

Every open detector scores on two orthogonal axes. Conflating them is the
single biggest source of wasted effort (see HANDOFF_2026_05_28b: AWS is axis-2,
not axis-1).

- **Axis 1 — value fingerprint** ("what does this browser report?"): canvas/WebGL/audio
  hashes, font list, voices, codecs, navigator values, screen metrics. BO largely
  controls these via the profile + canvas/webgl/audio noise.
- **Axis 2 — coherence / lie detection** ("do the reported values + the *mechanism*
  that reports them make sense together, and do they match a real browser's
  internal shape?"). This is where a from-scratch JS-injected engine like BO is
  structurally exposed and where Camoufox (C++-level spoof) has a categorical
  advantage. CreepJS's `lies` engine is 100% axis-2.

**The structural truth for BO** (confirmed by `16_STEALTH_FINGERPRINT_AUDIT.md` §1
and the Camoufox architecture below): BO spoofs in JS, so every spoof is a
JS-observable artifact (a non-native getter, an object literal where a class
instance is expected, a `toString` that must be patched, a stack frame pointing at
`<anonymous>` bootstrap source). Camoufox spoofs in patched Firefox C++
(`Navigator.cpp`, `ClientWebGLContext::GetParameter()`, `CanvasRenderingContext2D.cpp`)
so there is *nothing JS-observable to detect* — the values are native all the way
down. **BO cannot reach Camoufox-level axis-2 parity by adding more JS masks; it can
only minimize the residual.** Every fix below is "shrink the JS-artifact surface,"
and the long tail is irreducible without a native-spoof layer (out of current scope).

---

## 1. The detectors and how they score (external research)

| Detector | Repo / URL | Axis-1 | Axis-2 (lie) | Headless-specific |
|---|---|---|---|---|
| **CreepJS** | github.com/abrahamjuliot/creepjs | heavy | **the strongest open lie engine** (953-line `src/lies/index.ts`) | `getHeadlessFeatures` + `getBotHash` |
| **fingerprint-suite / BotD** | github.com/apify/fingerprint-suite | heavy (generator/injector) | light | BotD: webdriver, chrome obj, permissions, plugins, UA |
| **sannysoft / intoli** | bot.sannysoft.com, intoli.com | light | medium | the classic checklist (webdriver, languages, plugins, chrome, permissions, iframe, toString) |
| **areyouheadless** | intoli "are you headless" | — | — | webdriver, UA "Headless", plugins length, languages, webgl vendor SwiftShader |
| **BrowserScan** | browserscan.net | heavy | medium | "Robot" score, WebDriver, CDP, automation flags |
| **pixelscan** | pixelscan.net | medium | **consistency** (IP/timezone/locale/UA cross-check) | masking-tool detection |
| **browserleaks** | browserleaks.com | heavy (per-vector pages) | low | TLS/JA3/JA4, HTTP/2, WebRTC IP leak |

Sources: [CreepJS](https://github.com/abrahamjuliot/creepjs),
[fingerprint-suite](https://github.com/apify/fingerprint-suite),
[rebrowser-patches](https://github.com/rebrowser/rebrowser-patches),
[codeline.co CreepJS review](https://www.codeline.co/thoughts/repo-review/2024/creepjs-fingerprinting-lie-detection),
[octobrowser anonymity-checkers](https://blog.octobrowser.net/how-anonymity-checkers-pixelscan-browserleaks-whoer-and-creepjs-work),
[browserleaks](https://browserleaks.com/),
[Camoufox (deepwiki)](https://deepwiki.com/daijro/camoufox).

---

## 2. The CreepJS `lies` engine — every tampering test (axis-2, the hard part)

This is the catalog's most important section: these are the tests that detect BO's
*masking itself*, independent of whether the spoofed values are correct. CreepJS's
`createLieDetector` runs the battery below against `Function`, `Navigator`,
`WebGLRenderingContext`, `CanvasRenderingContext2D`, `AudioBuffer`, `AnalyserNode`,
`Date`, `Intl`, `Element`, `HTMLElement`, etc. — every interface BO patches.

| # | Lie test | What it does | BO exposure | File:line |
|---|---|---|---|---|
| L1 | **`toString` mismatch** | `String(fn)` must be exactly `function name() { [native code] }` and `fn.toString.toString()` must also be native | 🟡 partial — patched at `stealth_bootstrap.js:14-48`, but MANY prototypes unmasked (Event ctors, Headers/Request/Response, XHR, WebGL methods, Worker.postMessage, Observers, Streams) per `16_…AUDIT.md` §2.3 | `stealth_bootstrap.js:14-48`; gaps in `event_bootstrap.js`, `fetch_bootstrap.js`, `window_bootstrap.js:3465` (XHR) |
| L2 | **`toString` length / own-prop** | native fns expose only `length,name(,prototype)` as own props; a patched fn that adds own `toString` leaks | ✅ fixed — `_maskFunction` deliberately does NOT add own `toString` (`stealth_bootstrap.js:62-75`) | `stealth_bootstrap.js:51-80` |
| L3 | **getter descriptor mismatch** | `Object.getOwnPropertyDescriptor(Navigator.prototype,'x')` must be a native getter on the *prototype*, not a data prop on the instance | 🟡 navigator masked on `_NavProto` (`window_bootstrap.js:1029`) — good — but **worker navigator is an object literal with data props** (`worker_bootstrap.js:139-160`), a descriptor-shape lie | `window_bootstrap.js:1029`; `worker_bootstrap.js:139-160` |
| L4 | **`Reflect.setPrototypeOf` / proxy detect** | sets proto to `null`, cycles the chain, checks `TypeError` shape to unmask Proxy traps | ✅ BO does not use Proxy for the masked fns (direct `defineProperty`); style proxy is the exception (`dom_bootstrap.js:583`) | n/a |
| L5 | **`class X extends fn {}`** | extending a non-constructible native must `TypeError` | ✅ method-shorthand toString fixes the `fsc` case (`stealth_bootstrap.js:25-39`); covered by Kasada `fsc`/`npc` arc | `stealth_bootstrap.js:25-39` |
| L6 | **`new fn()` / `fn.call` / `fn.apply` interface error** | calling a method on the wrong receiver must throw the exact native `TypeError: Illegal invocation` | 🟡 depends on op impl — methods backed by deno ops may throw a different message than `Illegal invocation` | `dom_ext.rs`, `webgl_ext.rs` (verify per-op) |
| L7 | **`Object.keys` / `getOwnPropertyNames` / `Reflect.ownKeys` consistency** | the three enumerations must agree and match Chrome's exact key set/order | 🟡 — BO stub interfaces (`interfaces_bootstrap.js`) and hand-built prototypes can have key-order drift vs Chrome | `interfaces_bootstrap.js`, `window_bootstrap.js` |
| L8 | **STACK-TRACE lie test** | triggers an error *inside* a native fn; native throws a system error with NO script frame, a JS-replaced fn reveals the **script path / source URL** in `error.stack` | ❌ **HIGH RISK** — BO runs bootstrap as `<anonymous>` (good: `snapshot.rs:97`) but external page scripts run via `execute_script_with_name(code, url)` (`page.rs:419,467`); any throw inside a BO polyfill that is *not* a deno op can surface a frame. No `Error.prepareStackTrace` scrub found in `cleanup_bootstrap.js`/`stealth_bootstrap.js` (grep clean). `16_…AUDIT.md` §3.2 marks `esd` as 🟡 "full sweep TBD". | no scrub; `snapshot.rs:97` |
| L9 | **`Illegal` property-access error** | accessing a getter on the wrong object type must throw native error | 🟡 per-interface | various |
| L10 | **Math/Date/Intl algorithmic identity** | `getMaths` compares `Math.acosh/expm1/...` results to V8's exact f64 bits; `Intl` resolvedOptions must match locale | ✅ Math/Date/Intl are V8-native (not polyfilled) — strong | V8 native |

**Verdict:** BO's biggest axis-2 exposures, in order: **L8 (stack-trace) → L1 (toString
gaps) → L3 (worker-navigator object-literal shape) → L7 (key-order) → L6 (Illegal
invocation message)**. L1 is already tracked by `16_…AUDIT.md §5` (the sweep plan);
L8 and L3 are *under-tracked* and are this catalog's new contributions.

---

## 3. CreepJS headless / bot scoring (`getHeadlessFeatures` + `getBotHash`)

These are the explicit checklist items (axis-1/3, the easy wins). Each maps to a
concrete BO source location.

| Signal | CreepJS name | Chrome-real value | BO status | File:line |
|---|---|---|---|---|
| `navigator.webdriver` | explicit headless | `false`, getter on proto | ✅ masked getter | `window_bootstrap.js:992` |
| UA contains "HeadlessChrome" | explicit headless | absent | ✅ profile UA | `crates/stealth/profiles/*.yaml` |
| `window.chrome` missing | `noChrome` like-headless | present w/ `runtime`,`csi`,`loadTimes` | ✅ | `window_bootstrap.js:1625-1697` |
| permissions↔Notification bug | `hasPermissionsBug` | `query({notifications})`='prompt' while `Notification.permission`='default' on https; 'denied' coherent on insecure | ✅ | `window_bootstrap.js:469-503`, `6196+` |
| `navigator.plugins.length===0` | `noPlugins` | ≥1 (PDF viewer set) | ✅ profile-supplied | `window_bootstrap.js:5333-5335` |
| `navigator.mimeTypes` empty | `noMimeTypes` | non-empty | ✅ | `window_bootstrap.js:5335` |
| `Notification.permission==='denied'` default | `notificationIsDenied` | 'default' on https | ✅ | `window_bootstrap.js:6196+` |
| known headless bg color / `prefers-color` | `hasKnownBgColor`/`prefersLightColor` | matched media | 🟡 verify `matchMedia('(prefers-color-scheme)')` returns light, not headless default | `window_bootstrap.js:4007` |
| `navigator.userAgentData` blank | `uaDataIsBlank` | populated | ✅ | `window_bootstrap.js:1820-1845` |
| `navigator.pdfViewerEnabled===false` | `pdfIsDisabled` | true | ✅ profile | worker `worker_bootstrap.js:155`, main profile |
| screen has no taskbar (availHeight===height) | `noTaskbar` | availHeight < height by ~taskbar | 🟡 verify `screen.availHeight` per profile (FIX-H low-pri, `15_FIX_PRIORITY`) | `window_bootstrap.js` screen |
| viewport===screen resolution | `hasVvpScreenRes` | innerHeight < screen.height | 🟡 verify window vs screen metrics differ | `window_bootstrap.js` |
| WebGL renderer = SwiftShader/llvmpipe | stealth WebGL | real GPU string | ✅ profile GPU (`gpu.rs`) | `crates/stealth/src/gpu.rs` |
| iframe `contentWindow` proxying | stealth | real sub-realm | ✅ | `dom_bootstrap.js:2347-2931` |
| `chrome.runtime` tampering | stealth | coherent | ✅ | `window_bootstrap.js:1625` |

**Verdict:** the explicit-headless checklist is essentially clean in BO (this is why
`areyouheadless` should pass on the value axis). The residual is the 🟡 *screen-metric*
items (taskbar/viewport) and the axis-2 lies in §2.

---

## 4. Axis-1 value-fingerprint vectors — BO coverage

### 4.1 Canvas 2D / WebGL / OffscreenCanvas
| Vector | Detector | BO status | File:line |
|---|---|---|---|
| `toDataURL`/`getImageData` pixel hash | CreepJS `getCanvas2d`, BotD, browserleaks | ✅ deterministic per-profile **5% PCG32 jitter** | `crates/canvas/src/canvas2d.rs:1092-1120` |
| WebGL readPixels hash | CreepJS `getCanvasWebgl` | ✅ 5% jitter | `crates/canvas/src/webgl_render.rs:407-426` |
| `UNMASKED_VENDOR/RENDERER_WEBGL` | CreepJS, AWS WAF, areyouheadless | ✅ from profile | `crates/stealth/src/gpu.rs`, `canvas_bootstrap.js:266-585` |
| `getParameter`/`getSupportedExtensions` MAX_* | CreepJS, AWS WAF | ✅ values; 🟡 method `toString` leaks (L1) | `canvas_bootstrap.js`; FIX-D/D2 (`15_FIX_PRIORITY`) |
| **canvas-noise *consistency*** | CreepJS hashes BOTH `toDataURL` and a 2nd read; **noise must be deterministic across reads of the same draw** | ⚠️ **VERIFY** — if jitter re-randomizes per call, two reads differ → instant lie. Camoufox noise is "deterministic per context" (deepwiki). BO seeds from profile (`canvas2d.rs:1097` `state=seed+const`) → deterministic IF same canvas → likely OK but **must be tested against CreepJS double-read**. FIX-G (`15_FIX_PRIORITY`) flags noise policy as unresolved. | `canvas2d.rs:1092` |

### 4.2 Audio
| Vector | Detector | BO status | File:line |
|---|---|---|---|
| OfflineAudioContext render hash | CreepJS `getOfflineAudioContext`, browserleaks | ✅ per-`audio_seed` jitter (FIX-C) | `audio_ext.rs:51-79` |
| DynamicsCompressor params | CreepJS, Akamai T1.3 | ✅ seeded threshold/release jitter | `audio_ext.rs:62-79` |
| sampleRate/baseLatency/outputLatency | CreepJS | ✅ seeded (FIX-C) | `canvas_bootstrap.js:751-762` |

### 4.3 Fonts
| Vector | Detector | BO status | File:line |
|---|---|---|---|
| Font enumeration (measureText width per font) | CreepJS `getFonts`, browserleaks, fingerprint-suite | ⚠️ **VERIFY** — does BO's `measureText`/`offsetWidth` vary per font-family the way real font metrics do? If all fonts measure identically → "no fonts installed" headless signal. Camoufox uses a per-context allowed-font set + HarfBuzz spacing seed. | `canvas_bootstrap.js` measureText, layout_ext.rs |
| `document.fonts` / FontFace check API | CreepJS | 🟡 `FontFace` defined (`window_bootstrap.js`); verify `fonts.check()` returns plausible | `window_bootstrap.js` |
| platform-font-list ↔ OS coherence | CreepJS `liedPlatformVersion` via fonts | ⚠️ font list must match `navigator.platform` OS | profile |

### 4.4 Navigator / screen / locale coherence
| Vector | Detector | BO status | File:line |
|---|---|---|---|
| `hardwareConcurrency`/`deviceMemory`/`languages`/`platform`/`vendor`/`productSub` | all | ✅ profile-driven | `window_bootstrap.js:1029`, profile |
| `userAgent` ↔ `Sec-CH-UA` ↔ `userAgentData` cross-check | DataDome, pixelscan, CreepJS | ✅ (FIX-A) | `crates/net/src/headers.rs`, `window_bootstrap.js:1820` |
| timezone ↔ Intl ↔ `getTimezoneOffset` ↔ IP | pixelscan, CreepJS | ✅ Intl native + `window_bootstrap.js:2401-2429`; ⚠️ IP-geo coherence is deploy-time | `window_bootstrap.js:2401` |
| `doNotTrack` / `globalPrivacyControl` unusual-value | CreepJS | ✅ `null` / absent | `worker_bootstrap.js:154` |

### 4.5 Worker-vs-window identity (CreepJS `getBestWorkerScope` — HIGH VALUE)
CreepJS spawns dedicated/shared/service workers and compares
`hardwareConcurrency`, `language`/`languages`, `userAgentData`,
`platform`, WebGL vendor/renderer (via OffscreenCanvas), and canvas/audio
hashes between worker and window. **Any mismatch = `liedWorkerScope` = strong bot signal.**

| Sub-vector | BO status | File:line |
|---|---|---|
| worker `navigator.*` VALUES match window | ✅ both seed from same profile `_p()` | `worker_bootstrap.js:139-160` |
| worker `userAgentData.mobile/platform` matches | ✅ explicitly fixed (DataDome contradiction note) | `worker_bootstrap.js:88-137` |
| worker navigator **SHAPE** (proto/descriptors) | ❌ **object literal w/ data props**, not a `WorkerNavigator` instance with getters on a prototype — CreepJS L3 descriptor test + `Object.getPrototypeOf(navigator)` reveals `Object.prototype`, not `WorkerNavigator.prototype` | `worker_bootstrap.js:139-160` |
| worker canvas/audio/WebGL hash matches window | ⚠️ **VERIFY** — worker OffscreenCanvas + audio must use the SAME seed as window. Camoufox routes worker reads to the same per-context seed (deepwiki). If BO's worker uses a different seed/no-noise → cross-scope hash mismatch = lie. | `worker_ext.rs`, `canvas_bootstrap.js` (worker path) |
| worker `crypto.subtle` present | ✅ FIXED this session (commit `5216336`) | `worker_ext.rs`, `runtime.rs` |

### 4.6 Speech voices / media codecs
| Vector | Detector | BO status | File:line |
|---|---|---|---|
| `speechSynthesis.getVoices()` (names/lang/localService) | CreepJS `getVoices`, sannysoft | ✅ populated, OS-shaped (mac vs win lists) | `window_bootstrap.js:2471-2479`, `5323-5327` |
| voices ↔ OS coherence | CreepJS | 🟡 verify mac profile gets mac voices, not Google-only | `window_bootstrap.js:5323` |
| `canPlayType` / `MediaCapabilities.decodingInfo` codec list | Castle, DataDome, CreepJS `getMedia` | ✅ | `window_bootstrap.js:5176-5189`, `5735-5769` |

### 4.7 ClientRects / emoji / SVG / DOMRect
| Vector | Detector | BO status | File:line |
|---|---|---|---|
| `getClientRects` emoji-render fingerprint | CreepJS `getClientRects`, browserleaks | ⚠️ **VERIFY** — depends on BO layout engine producing OS-plausible glyph metrics; emoji width is a known headless tell | layout_ext.rs, `dom_bootstrap.js` DOMRect |
| `getBoundingClientRect` sub-pixel | CreepJS | ⚠️ verify sub-pixel float precision matches Chrome | layout_ext.rs |

### 4.8 Network-layer (browserleaks / pixelscan, axis-1 but below JS)
| Vector | BO status | Ref |
|---|---|---|
| TLS ClientHello / JA3 / JA4 | ✅ boring2 Chrome-identical | `23_TLS_HTTP_FINGERPRINT_REFERENCE.md`, `crates/net/src/tls.rs` |
| HTTP/2 frame fingerprint (Akamai h2) | ✅ own stack | `39_NETWORK_LAYER_FINGERPRINTING.md` |
| WebRTC local/public IP leak | ✅ `RTCPeerConnection` masked; verify no real ICE candidate leak | `window_bootstrap.js:4936-5017` |
| Client Hints headers | ✅ FIX-A/F | `crates/net/src/headers.rs` |

---

## 5. CDP / automation-leak vectors (rebrowser-patches class) — BO is structurally immune

This class is the #1 detector of Puppeteer/Playwright/Camoufox-over-CDP and is why
naive Playwright A/B tests are invalid for CDP-sniffers (see MEMORY: CDP-confound).

| Leak | Detector method | BO status |
|---|---|---|
| `Runtime.enable` side-effect | anti-bot watches for CDP execution-context events | ✅ **N/A — BO has no CDP**, it embeds V8 directly via deno_core. Structural immunity. |
| `__puppeteer_utility_world__` / isolated-world | name probe | ✅ N/A |
| `//# sourceURL=pptr:` / `app.js` | regex on eval'd script names | 🟡 BO names external scripts by their real URL (`page.rs:419`) — *correct* — and bootstrap `<anonymous>` (`snapshot.rs:97`). No `pptr:`/Playwright tells. |
| `cdc_` / `$cdc_` ChromeDriver props | property probe | ✅ N/A |
| `navigator.webdriver` via CDP | — | ✅ masked false |

**This is BO's one categorical advantage over Camoufox-via-Playwright** (Camoufox
itself runs over CDP/Juggler and inherits some of this risk; BO does not). Source:
[rebrowser-patches](https://github.com/rebrowser/rebrowser-patches).

---

## 6. How Camoufox wins axis-2 (and what BO can/can't copy)

Camoufox spoofs in patched Firefox C++ (`Navigator.cpp`, `WorkerNavigator.cpp`,
`ClientWebGLContext::GetParameter()`, `CanvasRenderingContext2D.cpp`,
`AudioBuffer.cpp`, `AnalyserNode.cpp`, `gfxPlatformFontList.cpp`,
`SpeechSynthesis.cpp`), keyed by `userContextId` via a thread-safe
`RoverfoxStorageManager` with cross-process IPC so **worker processes read the same
per-context value as the window**. Net effect:
- No JS-observable getter, no patched `toString`, no descriptor-shape mismatch
  (defeats CreepJS L1/L3/L8 entirely — there is nothing to find).
- Worker/window coherence is automatic (same C++ store), defeating `getBestWorkerScope`.
- Self-destructing `window.setWebGLVendor()` injectors leave no residue.

**Implication for BO:** the §2 lie vectors are an *asymptote* BO approaches but cannot
zero out in pure JS. The realistic public-engine goal is "minimize residual so
holistic ML score stays below the block threshold," matching the project's
holistic-tail conclusion (`42_HOLISTIC_VISION.md`, MEMORY Kasada-realm-closed). A
true fix would be a native V8-side spoof layer (e.g. op-backed property installation
that V8 reports as native) — a large structural effort, flagged here as the only
path to literal Camoufox axis-2 parity. Source:
[Camoufox deepwiki](https://deepwiki.com/daijro/camoufox).

---

## 7. Gaps this catalog adds beyond the existing repo docs

1. **L8 stack-trace lie test is under-tracked.** `16_…AUDIT.md` §3.2 lists `esd` as
   🟡 "full sweep TBD" but does not frame it as a *detector* test. CreepJS explicitly
   triggers errors inside native fns and reads `error.stack` for a script path. BO has
   no `Error.prepareStackTrace` scrub (grep of `cleanup_bootstrap.js`/`stealth_bootstrap.js`
   is clean). Any throw inside a non-op JS polyfill leaks a frame.
2. **Worker-navigator SHAPE leak (L3)** — `worker_bootstrap.js:139-160` builds a plain
   object literal with data properties and assigns it to `self.navigator`. Real Chrome
   `Object.getPrototypeOf(navigator)===WorkerNavigator.prototype` with the props as
   getters on the proto. CreepJS `getBestWorkerScope` + the descriptor lie test catches
   this even though the *values* are coherent. Not in the §2.2 audit table (which is
   window-realm-focused).
3. **Worker canvas/audio/WebGL hash coherence** — needs explicit verification that the
   worker OffscreenCanvas/audio path uses the same seed as the window. Camoufox's whole
   cross-process-storage patch exists to guarantee this; BO must prove it.
4. **Canvas-noise determinism across double-reads** — CreepJS hashes the same draw twice;
   non-deterministic jitter is an instant lie. FIX-G leaves the policy open.
5. **Screen-metric headless tells** (`noTaskbar`/`hasVvpScreenRes`) — `availHeight===height`
   and viewport===screen are CreepJS `like-headless` flags; only FIX-H (low-pri) touches this.

---

## 8. Ranked fix list (ROI order)

Ranking = (sites the open-detector axis-2/headless score affects) × (low effort)
× (confidence). Note: per HANDOFF_2026_05_28b, the *currently-failing 19* are mostly
axis-2 self-solve (AWS) or behavioral, so most of these are **score-hardening /
diagnostic-passing** wins (areyouheadless, CreepJS trust score, holistic ML tail
reduction) rather than guaranteed single-site flips. Confidence reflects that.

See StructuredOutput for the machine-readable ranked list. Narrative:

1. **D1 — `Function.toString` mask sweep (L1)** — execute the §5 plan already written in
   `16_…AUDIT.md`: add Event ctors, Headers/Request/Response, XHR, WebGL methods,
   Worker.postMessage, Observers, Streams, History, Storage to the `dom_bootstrap.js:3032`
   sweep list. 3-5 days. Cleans Kasada `sfc`/`sdt` + CreepJS L1. Public engine. Plan
   exists, just needs execution + the `native_code_mask_audit` golden test.
2. **D2 — Worker navigator real-prototype shape** — replace the `worker_bootstrap.js:139-160`
   object literal with a `WorkerNavigator` class whose props are getters on its prototype,
   matching window. 1 day. Cleans CreepJS L3 + `getBestWorkerScope` shape lie. Public.
3. **D3 — Worker↔window canvas/audio/WebGL seed parity test + fix** — add a chrome_compat
   test that reads the canvas/audio/WebGL hash in a worker and asserts it equals the
   window hash; fix the worker path to share the seed if it diverges. 1-2 days. Cleans
   `getBestWorkerScope` hash lie. Public.
4. **D4 — `Error.prepareStackTrace` / stack scrub (L8)** — install a stack-frame filter
   that drops `<anonymous>`/bootstrap frames and any BO-internal helper from `error.stack`,
   matching Chrome's `at fn (url:line:col)` shape for page code only. 1-2 days. Cleans
   CreepJS L8 + Kasada `esd` + Castle. Public.
5. **D5 — Canvas-noise double-read determinism test** — add CreepJS-style double-read test;
   confirm jitter is deterministic per (canvas, draw); resolve FIX-G policy. 0.5 day.
   Public.
6. **D6 — Screen-metric headless tells** — set `screen.availHeight < height` (taskbar) and
   ensure viewport != screen per profile. 0.5 day (extends FIX-H). Public.
7. **D7 — `Illegal invocation` message parity (L6)** — audit deno-op-backed methods so
   wrong-receiver calls throw the exact Chrome `TypeError: Illegal invocation`. 2-3 days
   (per-op). Public. Lower ROI (rarely probed in isolation).
8. **D8 — `getOwnPropertyNames`/`Reflect.ownKeys` key-order parity (L7)** — golden-test the
   key order of patched prototypes vs a real Chrome dump. 2 days. Public. Long-tail.
9. **D-NATIVE — native V8-side spoof layer** — the only path to literal Camoufox axis-2
   parity (no JS-observable spoof). Large structural effort; flagged, not scoped here.
   Public-engine in principle but very high effort; most axis-2 vectors collapse if done.

**What this list does NOT fix (correctly out of scope):** the AWS self-solve-execution
gap (§5.1 of HANDOFF — that's the live-nav drain, not a detection vector); per-vendor
PoW/WASM solvers (`vendor_solvers`); behavioral mouse/key signals (`humanize.js`).
None of D1–D8 will flip imdb/booking/amazon-in, which are axis-2 *execution* not
axis-2 *lie* — but D1–D6 are the prerequisites for a clean `areyouheadless`/CreepJS
diagnostic and for shrinking the Kasada/holistic ML tail (canadagoose/hyatt/realtor).
