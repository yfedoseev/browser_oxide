# 02 — Input Event APIs: completeness, correctness, and dispatch

**Scope of this audit.** The full input-event surface that behavioral
anti-bot scripts probe and that a behavioral engine must be able to
*emit*: `UIEvent` base + `MouseEvent`, `PointerEvent`, `KeyboardEvent`,
`TouchEvent`, `WheelEvent`, `InputEvent`, `FocusEvent`,
`CompositionEvent`, `DragEvent`, plus the dispatch machinery
(`addEventListener`/`removeEventListener`, `on*` handlers, bubbling,
capture, `composedPath`, `target`/`currentTarget`, `defaultPrevented`,
`isTrusted`).

**One-line verdict.** The classes exist and listeners *fire*, so a
synthesized `keydown`/`mousemove` sequence does reach page listeners
(humanize.js proves it). But three structural facts cap behavioral
authenticity hard: (1) **the entire event system lives in JS**
(`event_bootstrap.js`) — `crates/event_loop` and `crates/dom` contain
**zero** event-dispatch code, so the "we own the event loop ⇒ we can mint
native trusted events" advantage is **completely unrealized**; (2)
**`isTrusted` is a forgeable own data property**, not the unforgeable
prototype getter real Chrome 148 ships — and humanize.js's
`Object.defineProperty(ev,'isTrusted',{value:true})` is itself an
anomaly that throws on real Chrome; (3) **every event property is an own
data property on the instance**, never a getter on the prototype — a
fingerprint divergence detectable on *any* input event. These are the
load-bearing fixes; field-completeness gaps (getModifierState,
coalesced events, derived `which`) sit below them.

Cross-refs: `external/BEHAVIORAL_biometrics.md` (mouse/keystroke quality,
the `_akRecMouse`/`_akRecKey` taps), `external/VENDOR_akamai.md`,
`external/VENDOR_datadome.md`, `40_TIMING_BEHAVIORAL.md` (§3.2 keystroke
wiring), `../v0.1.0-frontier-workflows/03_BESTBUY_AKAMAI.md`.

---

## 0. Code map (where the event system actually is)

| Concern | Location | Note |
|---|---|---|
| Event classes + constructors | `crates/js_runtime/src/js/event_bootstrap.js:9-512` | All JS, all own-data-prop |
| `addEventListener`/`dispatchEvent`/capture/bubble | `event_bootstrap.js:298-462` | All JS; listeners in a JS `Map` keyed by nodeId |
| `isTrusted` | `event_bootstrap.js:19` (`this.isTrusted = ...`) | own data prop, forgeable |
| Trusted-event escape hatch | `event_bootstrap.js:7` `Symbol.for('__bo_trusted__')` | used by *no* Rust code (grep: only a test) |
| Touch / TouchEvent (second copy!) | `crates/js_runtime/src/js/window_bootstrap.js:6234-6275` | **duplicate, conflicting** with event_bootstrap |
| Behavioral emitter | `crates/browser/src/js/humanize.js:40-458` | dispatches move/scroll/key; **no clicks** |
| Mouse/keystroke generators (Rust) | `crates/js_runtime/src/extensions/input_ext.rs`, `stealth_ext.rs:185` (`op_behavior_mouse_trajectory`) | exist; consumed only by humanize.js |
| Native event dispatch (Rust) | **none** — `crates/event_loop/src/lib.rs` (581 lines) is only the timer/microtask pump; `crates/dom` has no `dispatch`/`EventListener` | the structural gap |

A consequence that shapes every fix below: **listeners are stored in a
JS-side `Map`** (`event_bootstrap.js:269` `_nodeListeners: nodeId →
Map<type,[...]>`). Rust cannot enumerate or invoke them. Any "native"
trusted event therefore still has to re-enter JS and call the JS
`dispatchEvent`. The win from a Rust op is not "bypass JS dispatch" — it
is "produce an event object whose `isTrusted` getter is genuinely
non-overridable and whose shape is byte-identical to a UA-minted event."

---

## 1. What a real browser (Chrome 148) does

### 1.1 isTrusted is *unforgeable*
Per WebIDL `[LegacyUnforgeable]` (Chromium since the original
`Event.isTrusted` intent-to-ship), `isTrusted` is an **accessor property
on `Event.prototype`** with a getter and **no setter**, marked
non-configurable on the instance. Therefore on real Chrome:

- `Object.getOwnPropertyDescriptor(new MouseEvent('x'), 'isTrusted')`
  → **`undefined`** (it lives on the prototype, not the instance).
- `Object.getOwnPropertyDescriptor(Event.prototype, 'isTrusted')`
  → `{ get: ƒ, set: undefined, enumerable: true, configurable: true }`.
- `Object.defineProperty(ev, 'isTrusted', {value:true})` on an instance
  → **throws `TypeError`** (cannot redefine an unforgeable inherited
  accessor as an own data property in the way humanize.js attempts; at
  minimum it produces an own data prop that *shadows* the getter, which
  is itself the tell).

Programmatic `new MouseEvent(...)+dispatchEvent` → `isTrusted=false`.
Only UA-generated input (real device, or CDP `Input.dispatch*` injected
into the input pipeline) → `isTrusted=true`. (MDN:
*Event.isTrusted*; Chromium intent-to-ship "Make Event.isTrusted
Unforgeable".)

### 1.2 Every event property is a prototype getter
On real Chrome `clientX`, `screenX`, `button`, `key`, `code`, `deltaY`,
`pointerId`, `pressure`, etc. are **accessor properties on the relevant
`*.prototype`** (e.g. `MouseEvent.prototype.clientX`), not own data
properties on the instance. `'clientX' in MouseEvent.prototype` is
`true`; `Object.getOwnPropertyNames(ev)` is `[]`.

### 1.3 Correct field semantics
- **`MouseEvent`**: `clientX/Y`, `screenX/Y`, `pageX/Y`, `offsetX/Y`,
  `x`/`y` (aliases of clientX/Y), `movementX/Y`, `button`, `buttons`,
  `relatedTarget`, `getModifierState(key)` returns the *actual* modifier
  state, `which` derived (`button+1`).
- **`PointerEvent`**: all of MouseEvent + `pointerId`, `width`, `height`,
  `pressure` (0.5 for a mouse button-down, 0 otherwise), `tiltX/Y`,
  `twist`, `tangentialPressure`, `altitudeAngle`/`azimuthAngle`,
  `pointerType`, `isPrimary`, **`getCoalescedEvents()`** and
  **`getPredictedEvents()`** (return arrays of PointerEvent).
- **`KeyboardEvent`**: `key`, `code`, `keyCode`, `charCode`, `which`,
  `location`, `repeat`, `isComposing`, `getModifierState(key)`. `keyCode`
  and `which` are **derived from the physical key** ("a" → keyCode 65),
  not zero.
- **`WheelEvent`**: `deltaX/Y/Z`, `deltaMode`, the `DOM_DELTA_*`
  constants, inherits all MouseEvent coords.
- **`InputEvent`**: `data`, `inputType`, `isComposing`,
  `getTargetRanges()`, `dataTransfer`.
- **`CompositionEvent`**: `data` — IME path; absent in BO entirely.
- **`UIEvent`**: `view`, `detail`, **`sourceCapabilities`**
  (`InputDeviceCapabilities{firesTouchEvents}`) and `which`.

### 1.4 Dispatch
Capture (root→target) → at-target → bubble (target→root); `composedPath`
crosses shadow roots and is computed at dispatch time; `target` retargets
across shadow boundaries; `eventPhase`, `defaultPrevented`,
`stopPropagation`/`stopImmediatePropagation`, `passive` listeners cannot
`preventDefault`. For physical input Chrome emits *paired* streams
(`pointerdown`+`mousedown`, `pointermove`+`mousemove`) and a full click
sequence is `pointerdown → mousedown → pointerup → mouseup → click`
(+ `focus`/`focusin` on focusable targets).

---

## 2. What BO does today (file:line)

### 2.1 isTrusted — forgeable own data property
`event_bootstrap.js:19`:
```js
this.isTrusted = !!(options && options[_TRUSTED] === true);
```
This is an **own enumerable data property** on every event instance. Two
detections fall straight out:
- `Object.getOwnPropertyDescriptor(new MouseEvent('x'),'isTrusted')`
  returns a **data descriptor** (`{value:false, writable:true,
  enumerable:true, configurable:true}`) where real Chrome returns
  **`undefined`**. (`behavioral_polish.rs:101-104` actually *asserts*
  `new Event('click').isTrusted === false` — correct value, wrong
  shape.)
- `Object.getOwnPropertyNames(ev)` is non-empty on BO, `[]` on Chrome.

humanize.js then re-defines it per instance
(`humanize.js:105, 429, 441`):
```js
Object.defineProperty(event, 'isTrusted', { value: true, configurable: true });
```
On real Chrome this would **throw** (unforgeable inherited accessor); the
fact that it succeeds — and leaves a `configurable:true` own data prop —
is a second, independent tell. Net: BO's trusted events are
*self-incriminating*.

### 2.2 No native trusted-event path
`Symbol.for('__bo_trusted__')` (`event_bootstrap.js:7`) is the intended
clean hatch — set `isTrusted=true` via constructor option so the page
can't forge it (it doesn't know the Symbol). But **no Rust code uses
it** (grep across `crates -g '*.rs'` → only `behavioral_polish.rs:114`).
humanize.js doesn't use it either; it uses the `defineProperty` hack
instead. So today *every* synthetic input event in the live path is
either `isTrusted=false` (page-side JS) or a forged-data-prop (humanize).
`crates/event_loop` and `crates/dom` have **no dispatch entry point** at
all — the from-scratch advantage is on the table, untouched.

### 2.3 getModifierState is a stub
`event_bootstrap.js:85` (MouseEvent) and `:104` (KeyboardEvent):
```js
getModifierState(key) { return false; }
```
Always `false`. A capital-letter `keydown` must report
`getModifierState('Shift') === true`; an anti-bot scoring a typing burst
sees Shift never held → impossible keystrokes. Real Chrome returns the
true state. (PointerEvent/WheelEvent inherit the MouseEvent stub.)

### 2.4 All event props are own data properties
Every constructor (`event_bootstrap.js:64-266`) assigns `this.clientX =
...`, `this.key = ...`, etc. So **none** of the input-event props live on
the prototype. `'clientX' in MouseEvent.prototype` → `false` (Chrome:
`true`); `Object.getOwnPropertyNames(new MouseEvent('x'))` enumerates
~16 keys (Chrome: `[]`). This is a single, uniform divergence across the
*entire* input-event family — cheap to fingerprint with one probe loop.

### 2.5 KeyboardEvent keyCode/charCode/which not derived
`event_bootstrap.js:94-95`: `keyCode = options.keyCode || 0`,
`charCode = options.charCode || 0`, `which = options.which ||
options.keyCode || 0`. If a caller passes only `{key:'a', code:'KeyA'}`
(which humanize.js does — `humanize.js:135, 144`), `keyCode === 0`. Real
Chrome derives `keyCode 65` from the key. humanize's `keydown`/`keyup`
events therefore carry `keyCode:0, which:0, charCode:0` — a dead
giveaway for any sensor that reads keyCode.

### 2.6 MouseEvent missing `x`/`y` aliases; UIEvent missing sourceCapabilities/which
`MouseEvent` (`:64-86`) has no `x`/`y` getters (Chrome aliases them to
clientX/Y). `UIEvent` (`:56-62`) has `view`+`detail` but **no
`sourceCapabilities`** and **no `which`**. `sourceCapabilities`
(`InputDeviceCapabilities`) is present on real trusted UI events and is
read by some fingerprinters to distinguish touch- vs mouse-originated
events.

### 2.7 PointerEvent missing coalesced/predicted + getModifierState
`PointerEvent` (`:123-137`) has the static fields but **no
`getCoalescedEvents()` / `getPredictedEvents()`** methods (Chrome 148
ships both; high-frequency pointer paths return multiple coalesced
samples — their *absence* on a `pointermove` is itself anomalous), and it
inherits the broken `getModifierState`. `pressure` defaults to 0 even on
a primary button-down (Chrome uses 0.5).

### 2.8 InputEvent / CompositionEvent / DragEvent thin
- `InputEvent` (`:107-114`): has `data`, `inputType`, `isComposing`;
  **missing `getTargetRanges()` and `dataTransfer`**.
- `CompositionEvent`: **absent entirely** — not defined in
  event_bootstrap.js, only a *name* in `interfaces_bootstrap.js:_rest`
  → stubbed as an `Illegal constructor`. `new CompositionEvent(...)`
  throws. IME/`compositionstart`/`compositionend` cannot be simulated.
- `DragEvent` (`:261-266`): only `dataTransfer`; otherwise OK.

### 2.9 TouchEvent defined TWICE, conflicting
`event_bootstrap.js:152-163` defines `class TouchEvent extends UIEvent`
and exports it (`:500`). `window_bootstrap.js:6234-6275` *also* defines
`globalThis.TouchEvent = function TouchEvent(...)` whose prototype is
`Object.create(Event.prototype)` — **skipping UIEvent**. Whichever loads
last wins; the two have different prototype chains
(`UIEvent`-derived vs `Event`-derived) and different property shapes. A
probe of `TouchEvent.prototype.__proto__ === UIEvent.prototype` is
nondeterministic / wrong. `Touch` and `TouchList` real constructors live
only in window_bootstrap (`Touch` at `:6235`); `interfaces_bootstrap`
defers them via `_realImplLater`. Net: the touch family is inconsistent
and the duplicate must be removed.

### 2.10 Dispatch — mostly correct, two real gaps
`dispatchEvent` (`event_bootstrap.js:316-366`) implements
capture→target→bubble, `composedPath`, `eventPhase`, `once`,
`stopImmediatePropagation`, and fires `on*` handlers. Good. Gaps:
- **`composedPath` ignores shadow DOM** (`:29-34` and the dispatch path
  walk `parentNode` only). Components using shadow roots see a
  truncated path; `event.composedPath()` won't include shadow ancestors.
  (Lower priority — most anti-bot runs in the main tree.)
- **`passive` listeners are tracked but not enforced** (`:302, 306`):
  a passive listener that calls `preventDefault()` should be a no-op +
  console warning; BO lets it through (it'll set `defaultPrevented`).
  Minor.
- **`relatedTarget` retargeting** across shadow boundaries not handled
  (minor).

### 2.11 Behavioral emitter never clicks
humanize.js's own header (`humanize.js:5`) advertises "`click`", but the
code emits only `mousemove`+`pointermove` (`_fireMove`, `:235`),
`wheel`+`scroll` (`_fireScrollStep`, `:274`), `focus`/`visibilitychange`
(`:294-295`), and a focusin-triggered `keydown`/`keyup` burst
(`:119-153`). There is **no `mousedown`/`mouseup`/`click`/`pointerdown`/
`pointerup`** anywhere (grep confirmed). Akamai bmak and DataDome score
*click cadence* (down→up dwell, move-to-click distance); BO emits an
empty click channel. (This is a behavioral-engine gap, but it's gated by
the event-class correctness above — no point clicking with `keyCode:0`,
forged `isTrusted` events.)

---

## 3. The gap and the exact engine change

### G1 — isTrusted must be an unforgeable prototype getter, minted by Rust
**Change.** Stop storing `isTrusted` per-instance. Define it once as an
accessor on `Event.prototype`:
```js
// event_bootstrap.js, after class Event:
const _trustedSet = new WeakSet();          // events the UA minted
Object.defineProperty(Event.prototype, 'isTrusted', {
  get() { return _trustedSet.has(this); },   // no setter ⇒ unforgeable
  enumerable: true, configurable: true,      // Chrome's descriptor shape
});
```
- Remove `this.isTrusted = ...` (`event_bootstrap.js:19`).
- Page-side `new Event()` → not in the set → `false`. Correct.
- Trusted minting: provide an internal, **non-page-reachable** function
  `__bo_mintTrusted(ev)` that calls `_trustedSet.add(ev)`. Expose it only
  through the Symbol hatch or a closure the page can't grab.
- **Delete the `defineProperty(ev,'isTrusted',...)` calls in
  humanize.js** (`:105, 429, 441`) — replace `_dispatch` with a call to
  the mint helper before `dispatchEvent`.
- Result: `getOwnPropertyDescriptor(ev,'isTrusted') === undefined`
  (own), the getter lives on the prototype, no setter, and the page
  cannot forge it. **This is the single highest-value fix** — it removes
  two independent tells *and* makes humanize's events legitimately
  trusted in shape.

**Even better (the from-scratch advantage):** add a Rust op
`op_input_mint_trusted_move/click/key(...)` in a new `input_ext` op that
constructs the event *and* adds it to the trusted set in one step the
page can never observe, then calls JS dispatch. Today nothing in Rust
mints events — `crates/event_loop`/`crates/dom` have no dispatch. Wiring
even a thin Rust→JS "mint + dispatch" bridge realizes the moat we
currently leave unused. Effort is mostly the JS-side WeakSet (above);
the Rust op is a 30-line wrapper.

### G2 — move all input-event props onto the prototype
**Change.** Convert each input-event class so coordinate/key/delta props
are accessor (or at least non-enumerable own) properties matching
Chrome's descriptor shape. Pragmatic minimum: define them as getters on
`MouseEvent.prototype` / `KeyboardEvent.prototype` / etc., backed by a
hidden per-instance slot (a Symbol-keyed bag). This makes
`'clientX' in MouseEvent.prototype === true` and
`Object.getOwnPropertyNames(ev) === []`. Apply uniformly to MouseEvent,
PointerEvent, KeyboardEvent, WheelEvent, UIEvent, InputEvent, FocusEvent,
TouchEvent, DragEvent. (Can be code-generated from a field table to keep
it maintainable.)

### G3 — real getModifierState
**Change.** `event_bootstrap.js:85, 104`: back `getModifierState` with the
event's own modifier flags:
```js
getModifierState(k){
  switch(k){case 'Shift':return this.shiftKey;case 'Control':return this.ctrlKey;
  case 'Alt':return this.altKey;case 'Meta':return this.metaKey;
  case 'CapsLock':return !!this._capsLock;case 'NumLock':return !!this._numLock;
  default:return false;}
}
```
Then humanize's keystroke burst must set `shiftKey:true` on capital
letters and the matching modifier `keydown` ordering.

### G4 — derive keyCode/charCode/which from key/code
**Change.** Add a key→keyCode table (US layout) in `event_bootstrap.js`
KeyboardEvent constructor; if `keyCode` not supplied, derive from
`code`/`key`. Set `which = keyCode`. For `keypress`-style printable
input set `charCode`. Update `op_human_keystroke_schedule`
(`input_ext.rs:155`) to also emit the keyCode so humanize passes it.

### G5 — PointerEvent getCoalescedEvents/getPredictedEvents + pressure
**Change.** Add the two methods to `PointerEvent` (`:123`): return `[this]`
for a single sample (Chrome returns ≥1 for `pointermove`), or the actual
coalesced batch when humanize emits a dense path. Default `pressure` to
0.5 when `buttons !== 0`.

### G6 — add CompositionEvent; complete InputEvent
**Change.** Define `class CompositionEvent extends UIEvent {data}` in
event_bootstrap.js and export it (remove it from the
`interfaces_bootstrap` stub fallthrough). Add `getTargetRanges()`
(return `[]`) and `dataTransfer` to InputEvent.

### G7 — UIEvent.sourceCapabilities + which; MouseEvent x/y aliases
**Change.** Add `sourceCapabilities` (an `InputDeviceCapabilities`
instance — needs a tiny real class with `firesTouchEvents`) and `which`
to UIEvent; add `x`/`y` getters aliasing clientX/Y to MouseEvent.

### G8 — remove the duplicate TouchEvent
**Change.** Delete `window_bootstrap.js:6234-6275`'s `TouchEvent`
redefinition; keep the `event_bootstrap.js` `class TouchEvent extends
UIEvent` (correct chain) and keep only `Touch`/`TouchList` in
window_bootstrap. Verify `TouchEvent.prototype.__proto__ ===
UIEvent.prototype` after load.

### G9 — emit a real click sequence in the behavioral engine
**Change.** Add to humanize.js a `_fireClick(x,y,target)` that dispatches
the full Chrome sequence with realistic timing:
`pointerdown → mousedown → (focus/focusin) → pointerup → mouseup →
click`, with `buttons:1` during the press, `button:0`, a 60-110 ms
down→up dwell (draw from `op_behavior_random`), and `_akRecMouse(...,
1, 0)` taps so the Akamai click channel is populated. Trigger it on a
plausible target after a settle (e.g. the navigation's first focusable
control, or a low-frequency idle click). Gate it conservatively to avoid
firing the page's own handlers on benign forms.

### G10 — composedPath through shadow DOM (lower priority)
**Change.** Extend the path walk (`:29-34`, `:327-333`) to cross
shadow-root boundaries and implement event retargeting. Most anti-bot
runs in the light DOM; defer unless a target uses web components for its
challenge UI.

---

## 4. Which sites these unblock

These are **enablers**, not direct flips — they unblock the *behavioral*
work that the gated sites actually need. From the trustworthy
2026-05-28 delta baseline (BO 5 / v150 8), the behavioral-gated set is
Akamai (bestbuy/booking/imdb), DataDome (homedepot/amazon-in), and the
Kasada cluster.

| Fix | Mechanism unblocked | Sites |
|---|---|---|
| G1 isTrusted unforgeable | removes the two most basic synthetic-event tells; makes *all* humanize events legitimately trusted | **bestbuy (Akamai)**, **homedepot/amazon-in (DataDome)**, Kasada cluster, PerimeterX/HUMAN |
| G2 prototype-getter shape | removes a one-probe fingerprint across every input event | same set — this is table-stakes for any sensor that introspects event objects |
| G3 getModifierState + G4 keyCode | makes the keystroke channel survive field-level scoring | **booking/imdb (Akamai bmak)**, DataDome key buffer |
| G5 coalesced/pressure | pointermove realism for high-freq pointer scoring | DataDome `_initialCoordsList`, Kasada |
| G9 click sequence | populates the empty click channel bmak/DataDome score | **bestbuy**, homedepot, Kasada |
| G6/G7/G8 (composition, sourceCapabilities, TouchEvent dedupe) | API-completeness; closes existence/shape probes | broad anti-bot existence checks; mobile/touch profiles |

Honest caveat (per MEMORY): G1 alone "may not flip a site" — the
2026-05-28b handoff showed individual behavioral fixes not flipping
targets because the *aggregate* entropy/shape must be right and the live
nav drain (the AWS blocker) gates async events. These fixes are
**necessary, jointly**: a behavioral engine that clicks (G9) with
`keyCode:0` (G4 unfixed) and forged `isTrusted` (G1 unfixed) is worse
than no clicks. Land G1+G2+G3+G4 together as the "events are real-shaped"
baseline, then G9 to drive them.

---

## 5. Ranked fixes (effort + confidence)

Ranking = (detectability removed × breadth) ÷ effort. Confidence is in
the *mechanism*, not in a site flip.

| # | Fix | Effort | Confidence | Why this rank |
|---|---|---|---|---|
| 1 | **G1** isTrusted → unforgeable prototype getter + WeakSet mint; delete humanize `defineProperty` | S (½ day JS; +30 lines for the Rust mint op) | high | Two independent tells removed; foundational for every other behavioral event. Self-incriminating today. |
| 2 | **G2** all input-event props → prototype getters (own slots) | M (1-2 days; code-gen from field table) | high | Single uniform divergence across the whole family; one probe loop finds it. |
| 3 | **G4** derive keyCode/charCode/which | S (½ day; key table + op tweak) | high | humanize keys are `keyCode:0` today — trivially flagged; cheap. |
| 4 | **G3** real getModifierState | S (½ day) | high | Capital-letter typing currently reports Shift never held — impossible. |
| 5 | **G9** real click sequence in behavioral engine | M (1 day) | medium | Fills the empty click channel; depends on 1-4 being correct first. |
| 6 | **G8** remove duplicate TouchEvent | XS (delete + verify chain) | high | Nondeterministic prototype chain is a clean bug; near-zero risk. |
| 7 | **G5** PointerEvent coalesced/predicted + pressure | S | medium | Closes pointer-introspection probes; modest breadth. |
| 8 | **G7** UIEvent sourceCapabilities/which + MouseEvent x/y | S | medium | Existence/shape probes; needs a tiny InputDeviceCapabilities class. |
| 9 | **G6** CompositionEvent + InputEvent completeness | S | low-medium | `new CompositionEvent` throws today; needed for IME-aware sites, niche. |
| 10 | **G10** composedPath shadow DOM + passive enforcement | M | low | Correctness; rarely on the anti-bot path. Defer. |

**Sequencing.** Land 1→4 as one "real-shaped events" PR (they're
interdependent and cheap), regression-tested with new assertions:
`getOwnPropertyDescriptor(ev,'isTrusted')===undefined`,
`'clientX' in MouseEvent.prototype`, `getModifierState('Shift')` honors
shiftKey, `new KeyboardEvent('keydown',{key:'a'}).keyCode===65`. Then 5
(click engine) + 6 (TouchEvent dedupe). 7-10 as cleanup.

**The structural note worth repeating for the next agent.** The biggest
*unrealized* asset is that BO owns the event loop and DOM in Rust but
routes 100% of event dispatch through JS with a forgeable trusted flag.
G1's WeakSet+mint gets 90% of the moat for ½ a day. A full native
dispatch path in `crates/event_loop`/`crates/dom` is **not** required to
beat isTrusted detection and is *not* recommended now — the listener
registry lives in JS, so native dispatch would have to call back into JS
anyway. Spend the effort on G1-G4 shape correctness, not a Rust dispatch
rewrite.
