# 08 — The "Pass As A Real Browser" Roadmap (synthesis)

**Status:** synthesis of docs 01-07 in this directory. This is the master
plan; the per-area docs are the evidence. Every claim below is traceable to a
`file:line` in one of those docs.

**Scope guard (CLAUDE.md).** Everything actionable here lives in the **public**
crates (`crates/stealth`, `crates/js_runtime`, `crates/browser`). It is generic
input-humanization + API-surface correctness — NOT per-vendor bypass. The
site-specific challenge payloads (Kasada `/tl`, AWS PoW-worker driver, yelp
slider CV solve, daily-rotated encoders) stay in the private `vendor_solvers`
crate and are explicitly flagged where they appear.

---

## 1. Thesis

**For any site a real Chrome opens from a given IP, browser_oxide must open it
too — holding the IP constant.** When a human can load bestbuy / yelp / a
Kasada-protected store from an IP and browser_oxide cannot from that *same* IP,
the gap is not the IP. It is one (or more) of:

1. **API-surface incompleteness or incoherence** — an interface a real Chrome
   148 exposes that BO is missing, stubbing, or implementing in a way that
   produces a state no real Chrome ever produces (a "coherence tell": WebGPU
   present-but-every-call-fails next to a real WebGL renderer; sensors throwing
   on a mobile UA). [01]
2. **Behavioral emptiness or synthetic behavior** — a real user generates
   hundreds of curved, trusted `mousemove`s, biometric keystrokes, scroll
   momentum, touch (mobile), and focus transitions *before and during* a
   challenge; BO either emits little, or emits it linearly/forgeably. [03, 05]
3. **`isTrusted` inauthenticity** — the single most load-bearing tell. BO's
   `isTrusted` is a forgeable own data property, re-stamped per event by
   `humanize.js` via `Object.defineProperty`, which (a) leaves a descriptor
   shape no real event has and (b) *would throw* against the spec-correct
   non-configurable accessor. This fires **before any behavior is sampled** and
   invalidates every behavioral signal downstream. [02, 04]

**The honest exception: genuine geo/IP cases.** Some failures *are* IP-bound and
no engine change fixes them — e.g. **ozon** returns a thin ~156B pre-JS
geo-refusal from a foreign/DC IP (it "won't load without a VPN"); it should be
marked `diagnostic:true` and dropped from the production denominator, re-opened
only with a RU residential IP. [07] **wildberries** is IP-difficulty + a real
engine half. These are flagged separately and never used to excuse an
engine/behavioral gap on a site a real browser *does* pass from the same IP.

**The structural never-IP-bind rule:** never assert "IP-bound" for
bestbuy/yelp/wildberries without a **captured hard block from a no-CDP real
Chrome at the exact benchmark IP** (the 2026-05-15 CDP-confound trap — Playwright
/ Camoufox / Patchright are CDP/Juggler-poisoned references and silently get
detected by Kasada/DataDome, so an A/B against them is invalid). [07] The
same-IP no-CDP oracle (§7 below) is the only legitimate disambiguator.

---

## 2. The structural advantage: BO can mint native trusted events

This is the moat, and today it is **completely unused** (crates/event_loop +
crates/dom leave it on the table). [04]

A CDP-driven tool (Playwright, Puppeteer, Selenium, Patchright) dispatches
synthetic input through the DevTools protocol, which a sophisticated detector can
fingerprint, and it cannot cleanly mint an in-realm event whose `isTrusted` is an
**unforgeable prototype accessor** the page can never reproduce. **browser_oxide
owns the event loop, the DOM arena, and the V8 realm in-process** — it can:

- define `isTrusted` once as a **native-masked getter on `Event.prototype`**,
  backed by a **module-private `WeakSet`/`WeakMap`** that no page script can
  reach (no `Symbol.for`, no own property), so
  `getOwnPropertyDescriptor(ev, 'isTrusted')` returns `undefined` (inherited)
  exactly like Chrome, and `ev.isTrusted = false` is a silent no-op exactly like
  Chrome; and
- mint trusted events from **outside page JS** via a `~30-line`
  `op_dispatch_trusted_event` that constructs the event, calls the closed-over
  `_markTrusted(ev)`, and dispatches via the captured
  `EventTarget.prototype.dispatchEvent` — a path page JS literally cannot
  invoke. [04 R1+R2]

This is why **Layer 1 below is the foundation**: realize this primitive first,
and every behavioral channel (mouse, key, scroll, touch, click) becomes
genuinely indistinguishable at the descriptor level — a property no CDP tool has.

---

## 3. The layered plan

The layers are **strictly ordered by dependency**: nothing in Layer 2 is worth
shipping until Layer 1 lands, because a perfect Σ-Λ trajectory carrying a
forgeable `isTrusted` own-prop still fails a descriptor probe checked
*independent of trajectory quality*. [03 §3.0, 04]

### Layer 1 — Input-event APIs + native trusted-event minting (THE FOUNDATION)

The unforgeable `isTrusted` primitive + correct event object shape. Without this,
no behavioral work is creditable.

- **L1.1 `isTrusted` as a native-masked prototype accessor.** Replace the
  per-instance own-data assignment (`event_bootstrap.js:19`) with a single
  `get`-only accessor on `Event.prototype` backed by a **module-private
  WeakSet** (not an own prop, not a global Symbol), masked native via
  `_maskFunction`, plus a closed-over `_markTrusted(ev)`. Closes all four
  descriptor tells at once: `getOwnPropertyDescriptor` undefined,
  `hasOwnProperty('isTrusted')` false, assignment is a no-op, getter `toString`
  native. [04 R1; 03 §3.0]
- **L1.2 Delete the forgeable mint path.** Drop the globally-registered
  `Symbol.for('__bo_trusted__')` constructor grant (`event_bootstrap.js:7`) so a
  page can no longer forge trusted events; trust is granted ONLY by the
  closed-over `_markTrusted`. Flip the `behavioral_polish.rs:114` assertion that
  currently asserts forgeability-as-a-feature to expect `false`. [04 R2]
- **L1.3 The native dispatch op.** Add `op_dispatch_trusted_event(target_node,
  kind, init)` in `input_ext.rs` calling a closed-over JS bridge
  (`__bo_internal_dispatch`) that constructs the event, `_markTrusted`s it, and
  dispatches via the captured `EventTarget.prototype.dispatchEvent`; delete the
  bridge from `globalThis` in `cleanup_bootstrap.js` after capture. Node-id
  resolution `_getNodeId` already exists (`event_bootstrap.js:272`). This is the
  unused moat. [04 R2]
- **L1.4 Rewrite humanize dispatch to use it.** Replace `humanize.js` `_dispatch`
  (`:104-108`) and the two inline `Object.defineProperty(event,'isTrusted',…)`
  blocks (`:429`, `:441`) with the `_markTrusted`/`op_dispatch_trusted_event`
  path; **delete all three `defineProperty` sites**. [03 §3.0, 04 R2, 05, 06]
- **L1.5 Event props as prototype getters.** Convert every input-event property
  (`clientX`, `key`, `code`, `deltaY`, `pointerId`, `pressure`, …) from an own
  data property to an **accessor getter on the relevant `*.prototype`**, backed
  by a hidden Symbol-keyed per-instance slot, code-generated from a field table,
  across MouseEvent/PointerEvent/KeyboardEvent/WheelEvent/UIEvent/InputEvent/
  FocusEvent/TouchEvent/DragEvent. Today `'clientX' in MouseEvent.prototype` is
  `false` (Chrome: `true`) and `getOwnPropertyNames(ev)` enumerates ~16 keys
  (Chrome: `[]`) — a single-probe fingerprint across the whole family. [02]
- **L1.6 `getModifierState` + keyCode/which derivation.** Back
  `getModifierState` with the event's own ctrl/shift/alt/meta flags (+caps/num);
  add a US-layout `key→keyCode` table in the `KeyboardEvent` constructor so
  `keyCode`/`charCode`/`which` derive from `key`/`code` instead of defaulting to
  0; update `op_human_keystroke_schedule` (`input_ext.rs:155`) to emit
  `keyCode`, and have humanize set `shiftKey:true` on capitals. Today every
  humanize keystroke is `keyCode:0, which:0` and Shift is never held — impossible
  keystrokes. [02]
- **L1.7 Remove the duplicate conflicting `TouchEvent`** and make
  `Touch`/`TouchEvent` constructible with spec fields (also needed by L2.4). [02]

### Layer 2 — The behavioral engine (wire the shipped math from nav start)

The Σ-Λ mouse + bigram keystroke + wheel_burst math is **already shipped and
best-in-public-tier** in `crates/stealth/src/behavior.rs`; Layer 2 is **wiring +
scheduling**, not new algorithms. [03] All dispatch goes through the Layer-1
trusted path.

- **L2.1 (FIX-A) Route the live mouse cycle through the Rust generator.** Rewrite
  `runCycle`'s mouse section (`humanize.js:292-336`) to call the already-exposed
  `op_behavior_mouse_trajectory` (`stealth_ext.rs:185`) per anchor and `_sched` a
  `_fireMove` at each 8 ms `t_ms`; delete `_lerp` (`:166`/`:316`), `_gauss`,
  `_sigmaLognormalTimes`, `_normalQuantile` from the hot path. The pre-pop path
  (`_seedHistoricalCoords :357`) already uses the good generator; the live cycle
  is linear `_lerp` (efficiency ≈ 1.0 = the #1 mouse tell, ~98% catch). Raises
  straightness to ~0.4, fixes endpoint jerk, pushes mousemove count from ~30 to
  100-250/cycle toward Castle's "hundreds." [03 §3.1, 05]
- **L2.2 (FIX-B) Session seed, not per-page.** Add `BROWSER_OXIDE_SESSION_SEED`
  (or profile-derived) as Level-1 above `rand::random` in
  `BehaviorRngState::from_env_or_random` (`input_ext.rs:35-41`), threaded into
  `BehaviorProfile.seed` for all three generators via the existing two-level
  `rng_for(salt)` scheme (`behavior.rs:109-115`). Today the per-page draw makes a
  multi-page visitor look like N different users (over-dispersion tell). [03 §3.2]
- **L2.3 (FIX-C) Proactive keystroke trigger + locale token.** In `runCycle`,
  find the primary visible text input (`querySelector` + `offsetParent` + rect)
  and dispatch a synthetic trusted `focus`/`focusin` so the already-wired
  capture-phase listener (`humanize.js:116-154`) fires; keep the single-shot
  `_typedSym` guard, **do not mutate `.value`** (no `input` event → benign forms
  uncorrupted), use a locale-appropriate token (not hardcoded `'hi'` at `:130`),
  extend `char_to_code` (`input_ext.rs:137`) for non-ASCII `.key`→physical-US
  `.code`. Today the listener is reactive-only; a no-autofocus page produces zero
  key events. [03 §3.3, 05]
- **L2.4 (FIX-E) Wire `wheel_burst` for scroll cadence.** Add
  `op_behavior_wheel_burst`, install a slot, rewrite the scroll section of
  `runCycle` to call it (preserving the correct `WheelEvent`+`scrollBy` body of
  `_fireScrollStep`). The generator (`behavior.rs:482-540`) has full unit tests
  but no op and is never called. [03 §3.4]
- **L2.5 (FIX-F) Click chains + visibility/blur lifecycle.** Dispatch a full
  trusted `pointerdown → mousedown → (80-120 ms dwell) → pointerup → mouseup →
  click` over inert area (never navigate); add a `visibilityState`
  visible↔hidden machine with `visibilitychange` + window `blur`/`focus`.
  humanize today synthesizes pure hover (zero click) and `visibilityState` is
  permanently `"visible"`. [03 §3.6]
- **L2.6 Scheduling/cadence.** Drive FIX-A cycles back-to-back for the first
  ~3-5 s (humans move a lot right after load), then decay to a LogNormal idle
  (not a fixed 4 s metronome — periodicity is itself a tell); add a single
  `MutationObserver` that steers the next cycle toward an appearing
  challenge-affordance (iframe/canvas/slider/input) so motion reads as "user
  reacting to the challenge." Keep the load-bearing synchronous
  `_seedHistoricalCoords` IIFE that drops historical points before any anti-bot
  script can read the buffer. [03 §3.7]

### Layer 3 — Per-site behavioral payloads / generic capabilities

Public, generic capabilities — and the explicit private boundary.

- **L3.1 (bestbuy / Akamai bmak) — PUBLIC.** With Layers 1+2, BO emits a
  trusted, curved, hundreds-count mouse/key stream into `__akamai_events`. The
  `sensor_data` POST consumer/encoder is private (`vendor_solvers`); the public
  engine produces the *input entropy* it reads. Honest caveat: an ASN trust floor
  on `_abck` may still gate it (BO + Patchright + Camoufox all hit the same
  7-8KB shell) → behavioral fixes are **necessary-not-proven-sufficient**;
  Capture B (§7) disambiguates. [05, 07]
- **L3.2 (mobile Akamai/PerimeterX) FIX-D — touch synthesis — PUBLIC.** Add
  `touch_swipe`/`tap` to `behavior.rs` (reuse the Σ-Λ sampler), ops + slots, and
  in `humanize.js` gate on `navigator.maxTouchPoints>0` to dispatch real trusted
  `TouchEvent`s (with `Touch.radiusX/radiusY/force`) via a new `_akRecTouch` +
  periodic deviceorientation/devicemotion incrementing `counters.accel`. This is
  **the one signal where BO is strictly worse than nothing**: mobile presets set
  `maxTouchPoints=5` (`presets.rs:875`) yet emit zero touch → a clean mobile-
  headless tell. Ties to the L4.1 sensor work for `counters.accel`. [03 §3.5, 05]
- **L3.3 (yelp / DataDome) — PATH A is PUBLIC, slider is PRIVATE.** The yelp win
  is **NOT a slider solver** — DataDome silently Device-Checks (rt:'i') >99.99%
  of users; the slider (rt:'c') is a worse-trust-score symptom. Earn rt:'i' like
  etsy via the **public** shared cluster: child-iframe shared cookie-jar fix
  (clearance currently lands in an isolated child V8 jar the parent retry never
  reads), preserve the no-CDP zero-transport trust advantage, route runCycle
  through op_behavior_mouse_trajectory (L2.1), worker fingerprint inheritance for
  the device-check PoW. A generic trusted drag (`op_human_drag_path` +
  press→drag→release dispatcher) is **public and reusable** for slider/press-hold
  captchas — but the drag's **landing x = the visual puzzle gap**, which needs a
  CV template-match + daily-rotated `ddCaptchaEncodedPayload` = **vendor_solvers
  only, forbidden in public crates**. A confident drag that lands at an arbitrary
  x is rejected (DataDome validates both biometrics AND geometric landing). No
  open engine, including Camoufox v150 (which has no slider code), passes a
  *shown* slider. Defer; the public yelp win is Path A. [06]
- **L3.4 (Kasada trio: canadagoose/hyatt/realtor) — PUBLIC ingredients, private
  `/tl`.** No single behavioral lever flips these — it is a holistic ML tail. The
  public Layer-1/2 fixes are *ingredients* (isTrusted, Σ-Λ mouse, session-seed
  over-dispersion, WebGPU coherence); the `/tl` payload solve stays private.
  Score-hardening, not single-flip. [03 §4, 01]

### Layer 4 — Full API-surface completion

BO's surface is **far more complete than the brief assumed**: the ~1100-interface
list is installed; the high-value navigator sub-objects (gpu/usb/hid/serial/
bluetooth/locks/storage/credentials/scheduling/mediaCapabilities/permissions/
getBattery/userAgentData) all exist and SecureContext-gate correctly (verified
via in-process enumeration — sensors `undefined` on insecure context is *correct*
Chrome behavior, not a bug). The remaining gaps are **narrow**: [01]

- **L4.1 Generic Sensor API (the bmak-mobile gap) — HIGH.** Accelerometer/
  Gyroscope/Magnetometer/*OrientationSensor/Sensor are non-functional Illegal-
  constructor stubs (`interfaces_bootstrap.js:42-58`), no reading stream. On
  secure MobileAndroid real Chrome fires a live reading stream that bmak/BMP
  signs (`sensor_data` from accelerometer/gyroscope/rotationRate/
  deviceorientation). New `sensor_bootstrap.js`: `Sensor extends EventTarget` +
  subclasses with x/y/z or quaternion readings driven by `op_sensor_reading`
  (profile-seeded, slowly-varying, human-plausible), **gated on
  device_class===MobileAndroid**; on secure Desktop keep constructors but
  `start()`→`NotReadableError` and never fire (matches real desktop). Dispatch
  legacy DeviceMotion/DeviceOrientation on mobile. Add a MobileAndroid branch to
  `cleanup_bootstrap.js:152` (today only MobileIOS deletes them). Ties to L3.2's
  `counters.accel`. [01 §2.1]
- **L4.2 `window.ondevicemotion/ondeviceorientation/ondeviceorientationabsolute`
  — HIGH, ONE LINE.** Absent from the 120-handler list
  (`interfaces_bootstrap.js:117-148`); `'ondevicemotion' in window === false`.
  Real Chrome exposes them as `null` on Window on all platforms — a cheap
  bmak/BotD presence probe. Add the three to the handler array; on mobile-Android
  dispatch through them (tie to L4.1); on desktop they stay null. [01 §2.4]
- **L4.3 WebGPU coherence — MEDIUM.** `requestAdapter()` resolves to an object
  literal (not a `GPUAdapter` instance) with empty `features`/`limits` and
  `requestDevice()` **always rejects** `NotSupportedError`; no
  `wgslLanguageFeatures`. "GPU present but every call fails while WebGL reports a
  real NVIDIA/Apple GPU" is a deterministic axis-2 contradiction no real Chrome
  produces. Fix at `window_bootstrap.js:6094`:
  `Object.create(GPUAdapter.prototype)` with profile-keyed features/limits
  **agreeing with the WebGL renderer string** (Apple M-series vs NVIDIA RTX limit
  tables differ); add `wgslLanguageFeatures`; `requestDevice()` must **resolve**
  to a GPUDevice stub (throw only on actual draw calls). [01 §2.2]
- **L4.4 RTCPeerConnection depth — HIGH.** The no-op
  `addEventListener`/`removeEventListener` overrides (`window_bootstrap.js:5263-
  5264`) mean `pc.addEventListener('icecandidate',fn)` **never fires** (only the
  `onicecandidate=` path at `:5241` works) — a silent listener-hang tell; no
  `getConfiguration()`; `getStats()` resolves to an empty Map. Delete the no-op
  overrides (use inherited EventTarget) and dispatch a real
  `RTCPeerConnectionIceEvent` via `this.dispatchEvent` in `setLocalDescription`;
  add `getConfiguration/setConfiguration/restartIce/canTrickleIceCandidates/
  sctp` + the *Local/RemoteDescription accessors; make `getStats()` resolve to a
  non-empty `RTCStatsReport` once a description is set. (The mDNS host-candidate
  emission at `:5234-5246` is a good parity touch — keep it.) [01 §2.3]
- **L4.5 Kasada `bot1225` API — LOW / private.** One named-but-undecoded Kasada
  API; decode lives with the private `/tl` work. [01]

---

## 4. Consolidated ranked fix table (deduped across docs 01-07)

Ranked by ROI = (correctness gain + strategic value) / effort, honoring the
honest "0 confirmed v0.1.0 flips today" reality. `isTrusted` is rank 1 because it
is checked **independent of trajectory quality and before any behavior is
sampled** — a perfect path with a forged own-prop still fails.

| # | Layer | Gap (what real Chrome does vs BO) | Change | file:line | Unblocks | Effort | Conf |
|---|---|---|---|---|---|---|---|
| 1 | L1 | isTrusted is a non-configurable prototype accessor in Chrome; BO sets a per-instance own data prop (forgeable, `getOwnPropertyDescriptor`≠undefined, `=false` mutates) | Native-masked getter on `Event.prototype` backed by module-private WeakSet; closed-over `_markTrusted`; drop the `Symbol.for` grant; flip `behavioral_polish.rs:114` | `event_bootstrap.js:19`,`:7`; `behavioral_polish.rs:108-116` | PerimeterX/HUMAN, Kasada, DataDome, Akamai, bestbuy, yelp — every descriptor probe | 0.5 d | high |
| 2 | L1 | humanize re-stamps isTrusted via `defineProperty({value:true,configurable:true})` (would throw on real Chrome) | Delete all three `defineProperty` sites; route through `_markTrusted`/`op_dispatch_trusted_event` | `humanize.js:105,429,441` | all behaviorally-scored vendors | (in #1/#3) | high |
| 3 | L1 | No native dispatch op; 100% of input is JS `dispatchEvent` (CDP-class) | Add `op_dispatch_trusted_event` (~30 LOC) + closed-over `__bo_internal_dispatch` bridge; delete bridge in cleanup | `input_ext.rs`; `cleanup_bootstrap.js`; `event_bootstrap.js:272` | the unused moat; DataDome/Kasada/PerimeterX in-page scoring; vendor_solvers prerequisite | 0.5 d | high |
| 4 | L1 | Event props are own data props; `'clientX' in MouseEvent.prototype===false`, `getOwnPropertyNames(ev)` enumerates ~16 (Chrome `[]`) | Convert props to prototype accessor getters over a hidden Symbol slot, code-gen from a field table | input-event `*_bootstrap.js` classes | all behavioral sites with event introspection | 1-2 d | high |
| 5 | L2 | Live mouse cycle is linear `_lerp` (efficiency≈1.0, #1 tell ~98% catch); pre-pop already uses Rust gen | Route runCycle mouse through `op_behavior_mouse_trajectory`; delete `_lerp/_gauss/_sigmaLognormalTimes/_normalQuantile` from hot path | `humanize.js:292-336,166,316`; `stealth_ext.rs:185` | bestbuy probe, Kasada holistic ingredient, DataDome 31-feature vector | 1-2 d | high |
| 6 | L1 | `getModifierState` always false; keyCode/charCode/which default 0 | Back getModifierState w/ event flags; US-layout key→keyCode table; humanize sets shiftKey on capitals; op emits keyCode | `input_ext.rs:137,155`; KeyboardEvent ctor; `humanize.js:135,144` | booking/imdb (Akamai bmak key), DataDome key channel | 0.5-1 d | high |
| 7 | L2 | BehaviorRngState draws `rand::random` per page → multi-page visitor = N users (over-dispersion) | `BROWSER_OXIDE_SESSION_SEED`/profile-derived Level-1 seed into `BehaviorProfile.seed` for all 3 gens | `input_ext.rs:35-41`; `behavior.rs:109-115` | Kasada over-dispersion ingredient; v0.2.0 multi-page prerequisite | 0.5-1 d | high |
| 8 | L4 | `ondevicemotion/orientation/orientationabsolute` absent from Window (Chrome: null on all platforms) | Add the three to the handler array (one line); dispatch on mobile | `interfaces_bootstrap.js:117` | broad — removes Window-shape tell on every bmak/BotD page | 0.1 d | high |
| 9 | L4 | Generic Sensor API non-functional Illegal-ctor stubs; on mobile real Chrome fires reading stream | New `sensor_bootstrap.js` (`Sensor extends EventTarget` + subclasses), `op_sensor_reading`, gate on MobileAndroid; MobileAndroid branch in cleanup | `interfaces_bootstrap.js:42-58`; `cleanup_bootstrap.js:152` | Akamai mobile/behavioral days on Pixel (bestbuy/homedepot adaptive); DataDome/Kasada OS-coherence | 2-3 d | medium |
| 10 | L4 | RTCPeerConnection: no-op addEventListener (icecandidate never fires), no getConfiguration, empty getStats | Delete no-op overrides (inherit EventTarget); dispatch real RTCPeerConnectionIceEvent; add getConfiguration/setConfiguration/restartIce/canTrickleIceCandidates/sctp + getStats report | `window_bootstrap.js:5186,5263-5264,5234-5246` | browserleaks/CreepJS WebRTC parity; icecandidate connectivity probes | 1-2 d | high |
| 11 | L2 | Keystroke trigger reactive-only (no autofocus → zero keys); token hardcoded `'hi'` | Proactive focus of primary visible input via trusted focus/focusin; locale token; no `.value` mutation; `char_to_code` non-ASCII | `humanize.js:116-154,130`; `input_ext.rs:137` | bestbuy login probe; form corpus | 1 d | medium |
| 12 | L4 | WebGPU present-but-broken (empty adapter, requestDevice always rejects) next to real WebGL = coherence tell | `Object.create(GPUAdapter.prototype)` w/ profile-keyed features/limits agreeing w/ WebGL renderer; `wgslLanguageFeatures`; requestDevice resolves | `window_bootstrap.js:6094` | CreepJS trust; AWS-WAF/Kasada holistic tail (score-hardening) | 1-2 d | medium |
| 13 | L2 | Scroll cadence: 2 uniform steps; `wheel_burst` exists w/ tests but has no op, never called | Add `op_behavior_wheel_burst` + slot; rewrite runCycle scroll cadence | `humanize.js:274-336`; `behavior.rs:482-540` | scroll-distribution gap (marginal) | 1 d | medium |
| 13b | L3 | yelp/etsy/tripadvisor: clearance lands in isolated child V8 cookie jar parent retry never reads (rt:'c' not rt:'i') | Child-iframe shared cookie-jar fix + worker fingerprint inheritance; preserve no-CDP trust | `02_DATADOME_DEEP.md §3.3` | yelp (indirect), etsy + tripadvisor (direct +1-2) | (per 06) | high |
| 14 | L3 | Mobile: maxTouchPoints=5 but zero TouchEvents (worse than nothing) | `touch_swipe`/`tap` (Σ-Λ) + ops/slots; trusted TouchEvent w/ `_akRecTouch`; gate on maxTouchPoints>0; deviceorientation incrementing counters.accel | `behavior.rs`; `input_ext.rs`; `humanize.js:71`; `presets.rs:875` | any mobile Akamai/PerimeterX/F5/Castle target (v0.2.0) | 4-5 d | medium |
| 15 | L2 | No click chain (hover-only); visibilityState permanently "visible" | Trusted pointerdown→…→click over inert area; visible↔hidden machine + blur/focus | `humanize.js:294-295` | "hovered never clicked" anomaly (marginal) | 0.5 d | low |
| 16 | L3 | yelp SHOWN slider: drag landing x = visual gap (CV) + daily-rotated payload | **vendor_solvers ONLY** — fetch .jpg/.frag.png, template-match, encode `ddCaptchaEncodedPayload` | `vendor_solvers` | a shown yelp rt:'c' slider (no open engine passes) | low | low |
| — | infra | ozon: thin ~156B pre-JS geo-refusal from DC/foreign IP | Mark `diagnostic:true`, drop from denominator; re-open w/ RU residential IP | — | honest denominator (NOT engine) | — | — |

**Recommended sequence:** **#1→#2→#3→#4 (Layer 1 foundation) as one ~2-day
sprint**, then **#5→#6→#7→#8 (high-value, low-effort)**, then #9-#13 (API +
DataDome iframe), with #14 (touch) reserved for the v0.2.0 mobile corpus and #16
strictly private. Do Layer 1 *after* the AWS async-drain and SPA-hydration work
that actually blocks current corpus flips (§6).

---

## 5. The isTrusted structural advantage (restated as the deliverable)

Three independently-fatal, IP/behavior-independent hard fingerprints exist today:
(1) isTrusted is an own data property, (2) granted by a globally-forgeable
`Symbol.for`, (3) re-stamped per-event by `defineProperty`. [04] All three are
closed by Layer 1, and the close is **a capability no CDP tool can match**:

- **What a CDP tool can do:** dispatch input over the DevTools protocol, which
  marks events trusted at the browser level but is itself fingerprintable, and
  cannot redefine `Event.prototype.isTrusted` cleanly in-realm.
- **What BO can do (and must):** because it owns the realm in-process, define
  `isTrusted` as the exact Chrome descriptor (inherited accessor, no setter,
  native `toString`) backed by a page-unreachable WeakSet, and mint trusted
  events from a Rust op outside page JS. A page (or anti-bot script) probing the
  descriptor, freezing the prototype, or attempting `Symbol.for` forgery sees
  **the same result as real Chrome** — which is the entire point.

This is the highest-leverage, lowest-effort, lowest-risk item in the whole
roadmap (`~30 LOC`, high confidence) and the reason Layer 1 is the foundation.

---

## 6. The same-IP validation protocol (the only legitimate disambiguator)

The project currently has **no same-IP measurement** for bestbuy/yelp/
wildberries/ozon, so it cannot honestly attribute any failure to IP vs engine.
[07] The protocol that resolves it:

1. **No-CDP oracle at the exact benchmark IP.** Run `nocdp.sh` (real Chrome,
   zero automation surface, observed out-of-band via `xprop WM_NAME` +
   screenshot) from the *exact* benchmark IP per site, with a **pre-committed
   decision rule**:
   - **PASS** ⇒ the site is **ENGINE + BEHAVIORAL** (BO is in the same no-CDP
     detection class as the passing real browser) → fix the engine, capture +
     diff `sensor_data`/payload via offline replay (`awswaf_probe.rs`).
   - **Captured hard block** ⇒ **IP/GEO** → document the IP requirement; do not
     spend engine effort.
2. **BO side is offline replay (no egress)** so it never touches the contended
   benchmark IP — this is mandatory while the single IP is held by the competitor
   benchmark.
3. **Never use Playwright/Patchright/Camoufox as the real-browser reference** —
   they are CDP/Juggler-poisoned and Kasada/DataDome detect them, producing the
   2026-05-15 CDP-confound (the invalid A/B that falsely "confirmed" IP bans).
4. **Capture B specifically for bestbuy:** no-CDP real Chrome at the DC IP,
   zero-interaction. DC hydrates ⇒ engine/behavioral-addressable (diff
   `sensor_data`); only residential hydrates ⇒ ASN floor confirmed, behavioral
   fixes necessary-but-insufficient.

Per-site current read (all pending the oracle): **bestbuy/yelp lean
ENGINE+BEHAVIORAL** (same no-CDP class as the passing real browser);
**wildberries = IP-difficulty + a real engine half**; **ozon = genuinely
IP/GEO-bound** (thin pre-JS geo-refusal). [07]

---

## 7. Honest ceiling

With **full Layer 1 (isTrusted + event shape) + Layer 2 (wired behavioral
engine) + Layer 4 (sensor/RTC/WebGPU/device-motion API completion):**

| Target | Becomes passable? | Why / caveat |
|---|---|---|
| **bestbuy (Akamai)** | **Plausibly yes — pending Capture B.** | Layers 1+2 give the trusted curved hundreds-count stream bmak reads; the `sensor_data` encoder is private (`vendor_solvers`). Behavioral fixes are **necessary, not proven sufficient** — an ASN trust floor on `_abck` may still gate it. The no-CDP oracle resolves it. [05, 07] |
| **yelp (DataDome)** | **Plausibly yes via Path A (dodge the slider), NOT via the slider.** | The win is earning rt:'i' (silent Device Check) via the public etsy cluster (child-iframe cookie-jar + worker inheritance + Σ-Λ mouse + no-CDP trust). A *shown* slider (rt:'c') is unwinnable in the public engine (CV gap-x + daily key = vendor_solvers; no open engine incl. Camoufox v150 passes it). [06] |
| **etsy / tripadvisor (DataDome)** | **+1-2 direct** from the same child-iframe cookie-jar fix. | Shares the yelp Path A code. [06] |
| **Kasada trio (canadagoose/hyatt/realtor)** | **Score-hardened, not single-flip.** | Holistic ML tail; Layer 1/2/4 fixes are ingredients (isTrusted, Σ-Λ mouse, session-seed, WebGPU coherence). The `/tl` solve is private. No single public lever flips them. [03 §4, 01] |
| **PerimeterX/HUMAN targets** | **Materially improved** (isTrusted + Σ-Λ + press-and-hold drag capability) — single-flip unverified without a corpus target. [04, 06] |
| **booking / imdb (Akamai bmak key channel)** | **De-risked** by L1.6 (keyCode/getModifierState) + L2.3 (proactive keystroke), but their *confirmed* blocker is the **AWS live-nav async-drain**, not behavioral — fix that first. [03 §4] |
| **mobile-protected sites** | **Required-capability unblocked** by L3.2/L4.1 (touch + sensors), gated to v0.2.0 mobile corpus. [03 §3.5, 01] |
| **ozon** | **NO — genuinely IP/GEO-bound.** | Thin pre-JS geo-refusal from DC/foreign IP; needs a RU residential IP. Mark diagnostic, drop from the denominator. [07] |
| **wildberries** | **Partial — IP-difficulty + a real engine half.** | Needs the same-IP A/B to split the engine half from the IP half. [07] |

**Bottom line:** of the four headline targets, **bestbuy and yelp are the
realistic engine+behavioral wins** (pending the same-IP oracle), the **Kasada
trio is holistic score-hardening**, and **only ozon is honestly IP/geo-bound**.
No fix in this roadmap flips a *currently-confirmed* v0.1.0 corpus site by
itself — the present corpus blockers are the AWS async-drain and SPA hydration —
so Layer 1+2 is **correctness hardening + the v0.2.0 prerequisite**, executed
*after* the AWS/SPA work, with `isTrusted` (rank 1) as the single highest-leverage
change because it is the foundation everything behavioral stands on. [03 §4]
