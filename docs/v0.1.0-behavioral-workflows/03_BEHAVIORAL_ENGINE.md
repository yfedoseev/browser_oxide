# 03 — The Behavioral Simulation Engine (design + wiring plan)

**Status:** engineering design. This is the *constructive* companion to the
*analysis* doc `docs/v0.1.0-parity-workflows/external/BEHAVIORAL_biometrics.md`
(read that first for the verified code audit + 2024-2026 external research +
ranked ROI). That doc answers "what is the gap and is it worth it"; **this**
doc answers "what is the full engine and exactly how do we wire it."

**Audience:** anyone implementing FIX-A..F. Every claim is `file:line`.

**Scope guard (CLAUDE.md).** Everything below lives in the **public** crates
(`crates/stealth`, `crates/js_runtime`, `crates/browser`). None of it is
per-vendor bypass code; it is generic input-humanization. Site-specific
challenge solving (Kasada `/tl`, AWS PoW-worker driver) stays in the private
`vendor_solvers` crate.

---

## 0. TL;DR — the engine in one diagram

```
                          PER-SESSION SEED  (BROWSER_OXIDE_SESSION_SEED
                          or profile-derived; FIX-B)
                                   │
        ┌──────────────────────────┴───────────────────────────┐
        │  crates/stealth/src/behavior.rs  (the math — SHIPPED)  │
        │  mouse_trajectory  Σ-Λ multi-stroke  :142              │
        │  keystroke_timings LogNormal+bigram  :428             │
        │  wheel_burst       momentum/notches  :482             │
        │  [touch_swipe / tap]   ← TO BUILD (FIX-D)              │
        └──────────────────────────┬───────────────────────────┘
                                   │  exposed as #[op2] ops
        ┌──────────────────────────┴───────────────────────────┐
        │  crates/js_runtime/src/extensions/  (the bridge)      │
        │  op_behavior_mouse_trajectory  stealth_ext.rs:185  ✓  │
        │  op_human_keystroke_schedule   input_ext.rs:155    ✓  │
        │  op_behavior_random            input_ext.rs:54     ✓  │
        │  [op_behavior_wheel_burst]     ← TO ADD (FIX-E)       │
        │  [op_behavior_touch_swipe]     ← TO ADD (FIX-D)       │
        └──────────────────────────┬───────────────────────────┘
                                   │  Symbol.for(...) slots, installed
                                   │  in stealth_bootstrap.js:119-152
        ┌──────────────────────────┴───────────────────────────┐
        │  crates/browser/src/js/humanize.js  (the SCHEDULER)   │
        │  _seedHistoricalCoords  :357  (uses Rust gen)  ✓      │
        │  runCycle live mouse    :292  (linear _lerp)   ✗ FIX-A │
        │  focusin keystroke      :116  (reactive only)  ~ FIX-C │
        │  _fireScrollStep        :274  (uniform 2-step) ✗ FIX-E │
        │  touch                  —      (absent)        ✗ FIX-D │
        │  trusted-event marking  :105  (defineProperty) ✗ FIX-T │
        └───────────────────────────────────────────────────────┘
```

**The single most important structural fact:** the *math is already shipped*
and *best-in-public-tier* (Σ-Λ beats Bezier/linear/ghost-cursor, ties
diffusion on the DMTG catch-rate table — see biometrics §2.3). The engine's
job is **wiring + scheduling + a trusted-event primitive**, not new
algorithms. Four of the five live-path consumers (`runCycle` mouse, scroll,
touch, trusted-marking) bypass or lack the shipped math. Closing that is the
whole game.

---

## 1. What a real browser does

A real Chrome 148 with a human at the keyboard produces, from the instant a
tab gains focus and continuously thereafter:

1. **Continuous low-level mouse motion** — hundreds of `mousemove` events
   per interaction (Castle teardown: 378 human vs 4 bot), each paired with a
   `pointermove`, emitted at the OS pointer report rate (~125 Hz USB, ~60-125
   Hz trackpad). Trajectories are **curved** (path efficiency
   `Euclidean/pathLength ≈ 0.3-0.5`, not ~1.0), have **asymmetric
   acceleration** (fast push, slow pull), **micro-tremor** (~8 Hz pink
   spectrum), and **decelerate smoothly to a stop** (bounded endpoint jerk).
2. **`isTrusted === true`** on every one of those events, because the
   browser's input pipeline — not page JS — created them. Page-constructed
   `new MouseEvent(...)` is `isTrusted === false`, and the property is a
   **non-configurable accessor on `Event.prototype`** (cannot be
   `Object.defineProperty`'d away on an instance).
3. **Keystrokes with biometric timing** — per-key dwell (keydown→keyup,
   ~LogNormal(ln 95 ms, 0.30)) and inter-key flight (~LogNormal(ln 130 ms,
   0.55) at 50 WPM), bigram-modulated (alt-hand fast, same-finger slow),
   each a real `keydown`/`keypress`/`input`/`keyup` chain with
   `isTrusted=true`. Locale-correct: a RU profile types Cyrillic with
   `KeyboardEvent.code` still physical-US (`KeyA`) but `.key` Cyrillic.
4. **Scroll cadence** — wheel: discrete events, modal `deltaY=100`,
   LogNormal inter-event gaps; trackpad: 30-60 small-delta (1-5 px) events at
   ~60 Hz with a velocity ramp + exponential momentum decay.
5. **Touch** (mobile) — `touchstart`/`touchmove`/`touchend` with `Touch`
   objects carrying `radiusX/radiusY`, `force`, `rotationAngle`; swipe
   velocity follows the same Σ-Λ curve as mouse.
6. **Focus / visibility lifecycle** — `focus`/`blur` on tab switch,
   `visibilitychange` with a real `document.visibilityState` machine, idle
   gaps (humans pause for seconds, then burst).
7. **Click chains** — `pointerdown → mousedown → pointerup → mouseup →
   click` with realistic press dwell (~80-120 ms), not bare hover.

Anti-bot behavioral modules — **Akamai BMP** `bmak` (sensor_data v2/v3 reads
`__akamai_events.{mouse,key,scroll,touch}`), **DataDome** `tags.js` (31-feature
mouse vector at POST), **Kasada** (mouse summary + RAF cadence + 300 signals),
**PerimeterX/HUMAN** (Σ-Λ + isTrusted), **Castle/Bureau** (efficiency,
event-count, acceleration) — score the **distribution shape** of all of the
above with LSTM/CNN models, not single thresholds (Springer 2026; biometrics
§2.2-2.3).

---

## 2. What BO does today (verified `file:line`)

| Channel | Where | Quality | Verdict |
|---|---|---|---|
| Mouse — Rust generator | `crates/stealth/src/behavior.rs:142-287` | Σ-Λ multi-stroke, Fitts time, pink tremor, smoothstep tail | **excellent, shipped** |
| Mouse — pre-population (historical buffer) | `humanize.js:357-452` `_seedHistoricalCoords` | calls `op_behavior_mouse_trajectory`, drops ~14 Σ-Λ points into `__akamai_events.mouse` synchronously + 1 sync move/pointer pair | **good — uses Rust gen** |
| Mouse — **live cycle** | `humanize.js:292-336` `runCycle` | **linear `_lerp(a,b,tau)` (line 316)** between 3 random anchors, white-noise tremor (`_gauss()*0.8`), impulse endpoint at each anchor | **weak — bypasses Rust gen (FIX-A)** |
| Keystroke — generator | `behavior.rs:428-464` + `op_human_keystroke_schedule` (`input_ext.rs:155`) | LogNormal dwell + bigram flight | **shipped** |
| Keystroke — wiring | `humanize.js:116-154` (capture-phase `focusin`) | dispatches keydown/keyup at schedule offsets, calls `_akRecKey` | **wired but reactive-only + hardcoded `'hi'` (FIX-C)** |
| Scroll | `humanize.js:274-336` `_fireScrollStep` | 2 uniform-random steps, `WheelEvent`+`scrollBy`; `wheel_burst` (`behavior.rs:482`) **has no op, never called** | **weak (FIX-E)** |
| Touch | — | **absent.** `__akamai_events.touch` array declared (`humanize.js:71`) but no `_akRecTouch`, no generator, mobile profiles set `max_touch_points:5` (`presets.rs`) and emit **zero** TouchEvents | **missing (FIX-D)** |
| Pointer pairing | `humanize.js:249-269, 433-445` | `pointermove` paired with every `mousemove`, correct attrs | **good**; no down/up/click (FIX-F) |
| Focus / visibility | `humanize.js:294-295` | fires `focus`+`visibilitychange` per cycle; `visibilityState` permanently `"visible"`, no blur alternation | **partial (FIX-F)** |
| Seed | `op_behavior_random` (`input_ext.rs:54`), slot `stealth_bootstrap.js:119-131` | per-**page** ChaCha, fresh `rand::random` seed each page (`input_ext.rs:39`) | **page-deterministic, NOT session-stable (FIX-B)** |
| **Trusted marking** | `humanize.js:105` `_dispatch` | `Object.defineProperty(event,'isTrusted',{value:true,configurable:true})` **after construction** | **fragile + detectable (FIX-T) — see §3.0** |
| Scheduler | `humanize.js:457` `setInterval(runCycle, 4000)` + `__bgSetTimeout` for per-event timers | one cycle then every 4 s; timers unref'd so they don't pin idle (`page.rs:3727`) | adequate; see §3.7 cadence |

**Re-confirmed-stale claims:** MEMORY.md and `40_TIMING_BEHAVIORAL.md` say
"keystroke generator exists but is NEVER wired" and "two-level seed not
consumed." **Both are now wired** (Fix 5 / Fix 6; `humanize.js:116`, `:49`;
`stealth_bootstrap.js:119-152`). Do not re-do that work. The *remaining*
keystroke gap is reactive-only triggering (FIX-C), and the remaining seed gap
is page-vs-session scope (FIX-B).

---

## 3. The gaps + the exact engine change

### 3.0 FIX-T — the `isTrusted` primitive (do this FIRST; it is free and load-bearing)

**What a real browser does.** `isTrusted` is a non-configurable accessor on
`Event.prototype`; the input pipeline sets the internal flag at event
creation. Pages cannot forge it.

**What BO does today — TWO tells:**

1. `event_bootstrap.js:19` sets `this.isTrusted = !!(options[_TRUSTED]===true)`
   as a **per-instance own data property** in the constructor. Real Chrome
   exposes it as a **getter on the prototype** — `Object.getOwnPropertyDescriptor(ev,'isTrusted')`
   returns `undefined` on a real event (it's inherited), but on BO returns a
   data descriptor. This is a structural tell independent of value.
2. `humanize.js:105` then **does not even use** the clean `_TRUSTED` symbol
   path — it calls `Object.defineProperty(event,'isTrusted',{value:true,
   configurable:true})` on an already-constructed event. That leaves a
   `configurable:true` own-data descriptor — doubly anomalous, and it would
   *throw* against a spec-correct non-configurable getter (so it's also
   fragile to a future correctness fix).

**The engine change.** The fix has two halves and they compose:

- **(a) Make `isTrusted` a real prototype accessor with a hidden backing
  slot.** In `event_bootstrap.js`, replace the per-instance assignment with:
  - a `WeakSet` (or a non-enumerable Symbol slot) `_trustedEvents`;
  - `Object.defineProperty(Event.prototype, 'isTrusted', { get() { return
    _trustedEvents.has(this); }, configurable: true, enumerable: true })`
    (mask it native via `_maskFunction` like the other accessors);
  - in the constructor, `if (options && options[_TRUSTED] === true)
    _trustedEvents.add(this);`.
  Now `getOwnPropertyDescriptor(ev,'isTrusted') === undefined` (inherited),
  matching Chrome, and the value cannot be forged by a page (it has no
  reference to `_trustedEvents` or the `_TRUSTED` symbol — the symbol is
  module-local, NOT `Symbol.for`-registered… see the caveat below).
- **(b) Make humanize.js construct trusted events via the symbol, not
  defineProperty.** Replace the `_dispatch` helper (`humanize.js:104-108`)
  so it passes `{ [Symbol.for('__bo_trusted__')]: true }` **in the
  constructor options** of every synthetic event, and delete the
  post-construction `Object.defineProperty`. Concretely, every `new
  MouseEvent / PointerEvent / WheelEvent / KeyboardEvent / Event` in
  humanize.js gets `[_TRUSTED]: true` merged into its options object (define
  `const _TRUSTED = Symbol.for('__bo_trusted__'); const _T = {[_TRUSTED]:
  true};` once at the top, spread it into each options literal).

**Caveat that must be fixed as part of (a):** today `_TRUSTED =
Symbol.for('__bo_trusted__')` (`event_bootstrap.js:7`) is **globally
registered** — any page can call `Symbol.for('__bo_trusted__')` and forge a
trusted event. For the pre-pop/humanize use that's acceptable (we *want* to
mark our own events), but it means a *page* could also mark its own events
trusted, and more importantly an anti-bot script that knows the trick can
detect us by *constructing* a "trusted" event and seeing it succeed (real
Chrome ignores the options entirely). The hardened design:
  - keep a **module-local** (un-registered) `const _TRUSTED = Symbol()` inside
    `event_bootstrap.js` that the constructor honors;
  - expose a **non-enumerable, page-invisible** marker function on the
    internals object that humanize.js / Rust dispatchers call, e.g.
    `globalThis[Symbol.for('__browser_oxide_mark_trusted__')](ev)` (installed
    in `stealth_bootstrap.js` alongside the other slots, survives the
    `internals` purge), which internally does `_trustedEvents.add(ev)`;
  - drop the `Symbol.for('__bo_trusted__')` constructor path entirely so a
    page can no longer forge.

**Effort:** 0.5 day. **Confidence:** high. **Public crate:** yes
(`event_bootstrap.js`, `stealth_bootstrap.js`, `humanize.js`). **Why first:**
every other behavioral channel dispatches through `_dispatch`; fixing the
primitive once upgrades mouse + scroll + key + touch + click simultaneously,
and the `getOwnPropertyDescriptor` tell is checked by PerimeterX/Castle
*regardless of trajectory quality* — i.e. a perfect Σ-Λ path with a
`configurable:true isTrusted own-prop` still fails.

---

### 3.1 FIX-A — route the live mouse cycle through the Rust Σ-Λ generator

**Gap.** `runCycle` (`humanize.js:292-336`) interpolates **linearly**
between 3 anchors (`_lerp`, line 316). Per biometrics §2.2: linear segments
give path-efficiency ≈ 1.0 (the #1 tell; ~98% catch on the DMTG table), white
tremor not pink, and an impulse-velocity discontinuity at each anchor
terminus. The pre-pop path already does this correctly via
`op_behavior_mouse_trajectory`; the live cycle just doesn't call it.

**Engine change.** Rewrite `runCycle`'s mouse section to:

1. Maintain a persistent cursor position `globalThis.__akamai_events._lastPos`
   (already seeded at `humanize.js:450`).
2. Pick the next anchor with the existing `_rand` viewport logic.
3. Call the **already-exposed** op:
   ```js
   const ops = Deno.core.ops;
   const raw = ops.op_behavior_mouse_trajectory(fromX, fromY, toX, toY, targetW);
   const traj = JSON.parse(raw || '[]'); // [{t_ms, x, y}] at 8 ms cadence
   ```
4. Schedule a `_fireMove(p.x, p.y, prev)` at each `p.t_ms` offset via
   `_sched` (the `__bgSetTimeout` helper), threading `prev` for `movementX/Y`.
5. Repeat for 2-3 anchors per cycle (the generator already does multi-stroke
   *within* one move; chaining anchors gives the inter-target pauses).

This deletes `_lerp` (line 166), `_sigmaLognormalTimes` (line 175),
`_normalQuantile` (line 196) and `_gauss` (line 158) from the hot path — all
of that math is now done correctly in Rust. Net: live motion gains curved
multi-stroke geometry, pink tremor, and the smoothstep terminal decel
(bounded endpoint jerk), and — because the generator samples at **8 ms over
multi-second motion** — the per-cycle `mousemove` count jumps from ~30 to
~100-250, pushing the session total toward the "hundreds" Castle expects
(biometrics §2.2 Gap #4).

**Effort:** 1-2 days. **Confidence:** high. **Public crate:** yes (reuses
existing op). **Unblocks:** 0 confirmed v0.1.0 flips, but closes the single
biggest mouse-quality gap and is a prerequisite for bestbuy/Kasada holistic
improvement.

---

### 3.2 FIX-B — thread a SESSION seed (not per-page)

**Gap.** `BehaviorRngState::from_env_or_random` (`input_ext.rs:35-41`) draws
`rand::random::<u64>()` **per page** when `BROWSER_OXIDE_BEHAVIOR_SEED` is
unset. A multi-page visitor therefore looks like N different users (different
σ_dwell, σ_mouse, fitts_b each page) — the over-dispersion tell DMTG §2.3 and
`40` §5 both flag (a real visitor's keystroke/mouse *style* is stable across
their session; that consistency is precisely why keystroke biometrics work).

**Engine change.**
1. Add `BROWSER_OXIDE_SESSION_SEED` (decimal u64) read in
   `BehaviorRngState::from_env_or_random` with priority *above*
   `rand::random` but below the explicit per-page override. Better: derive the
   session seed from the loaded `StealthProfile` (hash of profile path +
   a process-lifetime nonce) so each *profile* is a stable "person."
2. The two-level scheme already exists — `BehaviorProfile::rng_for(salt)`
   (`behavior.rs:109-115`) folds `seed * 0x9E3779B9… + salt`. Wire the
   session seed into `BehaviorProfile.seed` for *all three* generators
   (`mouse_trajectory`, `keystroke_timings`, `wheel_burst`) so a session's
   mouse, keys, and scroll all share one "person," with per-call salts
   distinguishing individual paths/strings.
3. Keep per-page *salt* variation so two page loads in the same session don't
   replay byte-identical event streams (different salts already do this).

**Effort:** 0.5-1 day. **Confidence:** high. **Public crate:** yes
(`input_ext.rs` + how `BehaviorProfile` is constructed in `page.rs`).
**Unblocks:** 0 in single-page v0.1.0 corpus; **prerequisite** for any
multi-page session corpus (v0.2.0) and for not looking over-dispersed to ML
scorers.

---

### 3.3 FIX-C — proactive keystroke trigger (+ locale-correct token)

**Gap.** The keystroke listener (`humanize.js:116-154`) is **capture-phase
`focusin`, reactive-only**: it fires only if *the page* focuses an
`INPUT`/`TEXTAREA`. Nothing in the navigate path proactively focuses a field
(the only programmatic focus is `dom_bootstrap.js:998` `HTMLElement.focus()`,
which the *page* must call). So on a typical corpus page with no autofocus
search box, **zero keystroke events** are produced. Also the typed token is
hardcoded `'hi'` (line 130) — fine for buffer non-emptiness, useless if a
vendor scores n-gram content, and not locale-correct.

**Engine change.**
1. In `runCycle` (or a one-shot ~600-1500 ms after load), if the buffer has
   no key events yet, **find the page's primary visible text input** —
   `document.querySelector('input[type=text],input[type=search],input:not([type]),textarea')`
   filtered by `offsetParent !== null` and a non-trivial bounding rect — and
   if one exists, dispatch a synthetic `focus`/`focusin` on it (trusted, via
   FIX-T) so the already-wired listener fires.
   - **Guard against polluting real forms:** keep the existing single-shot
     `_typedSym` tag; additionally **do not actually mutate `.value`** — the
     listener only dispatches `keydown`/`keyup` (no `input` event, no value
     change), so a benign page's controlled input is not corrupted. If a
     vendor needs the `input`/`beforeinput` chain, gate the value-mutating
     variant behind a config flag (off by default).
2. **Locale-correct token + key mapping.** Replace `'hi'` with a short,
   locale-appropriate token drawn from the active `StealthProfile` locale
   (e.g. a 3-5 char fragment), and extend `char_to_code` (`input_ext.rs:137`)
   so non-ASCII `.key` values still map to the physical-US `.code` a real
   keyboard reports (a RU user's `ф` is still `code:"KeyA"`). The dwell/flight
   math (`keystroke_timings`) is locale-agnostic and already correct.

**Effort:** 1 day. **Confidence:** medium. **Public crate:** yes
(`humanize.js`, small `input_ext.rs` `char_to_code` extension). **Unblocks:**
0 confirmed flips (no corpus site gates on key-presence); de-risks the
**bestbuy** login-flow probe (R-BESTBUY-AKAMAI) and any future form corpus.

---

### 3.4 FIX-E — wire `wheel_burst` for realistic scroll cadence

**Gap.** `_fireScrollStep` (`humanize.js:274-288`) is driven by a 2-step
plan with `deltaY` uniform in [80,120]/[60,90] (`humanize.js:330`) — matches
neither real wheel (modal `deltaY=100`, discrete notches, LogNormal gaps) nor
trackpad (30-60 small-delta momentum events at 60 Hz). The correct generator
`wheel_burst` (`behavior.rs:482-540`, models both styles per
`BehaviorProfile.scroll_style`) **exists with full unit tests but has no op
and is never called** (verified: `grep wheel_burst` finds only `behavior.rs`).

**Engine change.**
1. Add `op_behavior_wheel_burst(target_dy: f32) -> String` to
   `stealth_ext.rs` (or `input_ext.rs`), mirroring `op_behavior_mouse_trajectory`:
   build a `BehaviorProfile` (session-seeded per FIX-B, `scroll_style` from
   the profile — trackpad for laptop UAs, wheel for desktop), call
   `wheel_burst`, return JSON `[{t_ms, delta_y, mode}]`.
2. Register it in the `stealth_extension` op list (`stealth_ext.rs:198`) and
   install a `Symbol.for('__browser_oxide_wheel_burst__')` slot in
   `stealth_bootstrap.js` (same pattern as the mouse/keystroke slots).
3. Rewrite the scroll section of `runCycle` (`humanize.js:328-335`) to call
   the slot, then `_sched` a `_fireScrollStep(tick.delta_y)` at each
   `tick.t_ms`, preserving the existing `WheelEvent`+`scrollBy`+`scroll`+
   `_akRecScroll` body of `_fireScrollStep` (it's correct; only the *cadence*
   was wrong). Set `WheelEvent.deltaMode` from `tick.mode`.

**Effort:** 1 day. **Confidence:** medium. **Public crate:** yes.
**Unblocks:** 0 confirmed flips; closes the scroll-distribution gap (`40`
§3.3, biometrics §3.3).

---

### 3.5 FIX-D — touch synthesis for mobile profiles

**Gap.** **The only behavioral signal where BO is strictly worse than
nothing.** Mobile presets advertise `max_touch_points:5`, `ontouchstart`,
mobile UA (`presets.rs` iPhone) but emit **zero** `TouchEvent`s — a clean
`maxTouchPoints>0 && touchCount===0 && UA~Mobile` tell for Akamai-mobile /
PerimeterX-mobile / F5 / Castle. `__akamai_events.touch` is declared
(`humanize.js:71`) but `_akRecTouch` does not exist and nothing increments
the touch counter. No touch generator in `behavior.rs`.

**Engine change.**
1. **Rust:** add `touch_swipe(from, to, profile) -> Vec<TouchPoint>` and
   `tap(at, profile) -> Vec<TouchPoint>` to `behavior.rs`, **reusing the Σ-Λ
   sampler** from `mouse_trajectory` for the swipe path. Each `TouchPoint`
   carries `t_ms, x, y, radius_x (10-25 px), radius_y, force (0-1),
   rotation_angle`. Tap = press dwell ~80-120 ms (LogNormal) at one point.
2. **Op + slot:** `op_behavior_touch_swipe` / `op_behavior_tap` in
   `input_ext.rs`, registered, with `Symbol.for('__browser_oxide_touch_*__')`
   slots in `stealth_bootstrap.js`.
3. **humanize.js:** gate on `navigator.maxTouchPoints > 0`. Add `_akRecTouch`
   (mirror `_akRecMouse`). Construct **real `TouchEvent`s with `Touch`
   objects** — note `window_bootstrap.js` currently has *illegal-ctor stubs*
   for `Touch`/`TouchEvent` (interface identity only); FIX-D must make these
   **constructible** with the spec fields, or expose an internal factory the
   humanizer calls (so the page still sees the illegal-ctor behavior real
   Chrome has for `Touch` — Chrome's `Touch` *is* constructible, `TouchEvent`
   is too via `new TouchEvent`, so making them real is spec-correct, not a
   tell). Dispatch `touchstart → touchmove* → touchend` (trusted via FIX-T)
   with `touches`/`targetTouches`/`changedTouches` TouchLists populated.
4. Replace the mobile-profile mouse cycle with a touch cycle (real phones
   emit touch, *and* synthesize the compat `mousemove`/`click` Chrome fires
   after a tap — but driven by touch, not standalone mouse motion).

**Effort:** 4-5 days (largest item — new generator + constructible
`Touch`/`TouchEvent` + TouchList plumbing). **Confidence:** medium.
**Public crate:** yes (`stealth`, `input_ext`, `window_bootstrap.js`,
`humanize.js`). **Unblocks:** 0 in the current desktop-dominant corpus; the
only signal where BO is *worse than nothing* on mobile profiles; required for
any mobile-app-protected target (v0.2.0 mobile corpus).

---

### 3.6 FIX-F — click chains + focus/blur/visibility lifecycle

**Gap.** humanize.js synthesizes pure hover: no `pointerdown`/`mousedown`/
`pointerup`/`mouseup`/`click` chain (grep confirms zero in humanize.js), so a
vendor scoring "moved 30 s, never clicked" sees an anomaly. Focus handling is
shallow: `runCycle` fires `focus`+`visibilitychange` each cycle
(`humanize.js:294-295`) but `document.visibilityState` is permanently
`"visible"` (no hidden-tab machine, `40` §2.3) and there's no `blur`/`focus`
alternation modeling a tab switch.

**Engine change.**
1. Occasionally (e.g. once per session, after a settle delay) move the cursor
   (via FIX-A) onto a benign, non-navigating element — the `<body>` or a
   non-link region — and dispatch a full trusted chain `pointerdown →
   mousedown → (press dwell 80-120 ms) → pointerup → mouseup → click` at that
   point, with `_akRecMouse(... kind=down/up)`. **Do not click links/buttons**
   (avoid navigating the page); pick coordinates over inert area.
2. Add a minimal visibility state machine: a `Symbol.for`-installed
   `__browser_oxide_visibility__` op-backed getter so `visibilityState` can
   flip `visible→hidden→visible` with a matching `visibilitychange`, modeling
   a brief tab-switch (low frequency). Pair `blur` then later `focus` on
   `window`.

**Effort:** 0.5 day. **Confidence:** low. **Public crate:** yes.
**Unblocks:** 0 confirmed; marginal holistic-score hardening.

---

### 3.7 Scheduling / cadence — when to fire

This is the "engine" part beyond the per-channel generators. The current
scheduler (`humanize.js:454-457`): one `runCycle` immediately + `setInterval(
runCycle, 4000)`, with per-event timers via `__bgSetTimeout` (unref'd so they
don't pin `run_until_idle`, `page.rs:3727`). The design target:

1. **From nav start (t=0):** the synchronous `_seedHistoricalCoords` IIFE
   (`humanize.js:357`) already drops ~14 historical Σ-Λ points + 1 sync
   move/pointer pair **before any anti-bot script can read the buffer** — this
   is the load-bearing DataDome/PerimeterX defeat. Keep it. *Add* FIX-T so
   that first pair is properly trusted.
2. **Continuous low-level activity (t=0 → page settle):** drive FIX-A mouse
   cycles back-to-back (not on a 4 s gap) for the first ~3-5 s so the
   mousemove count reaches "hundreds" *before* the typical sensor_data /
   `tags.js` POST window, then **decay to the 4 s idle interval**. Humans move
   a lot right after a page loads, then settle.
3. **Reactive on challenge-element appearance:** add a `MutationObserver`
   (one, installed once) that watches for the appearance of known
   challenge-affordance shapes — an `iframe` whose src/title matches a
   challenge, a newly-inserted canvas/slider, a focused input — and, when one
   appears, **steer the next mouse cycle toward its bounding box** (Σ-Λ move
   to its center) and, for inputs, trigger FIX-C. This converts "random
   ambient motion" into "user reacting to the challenge," which is what the
   behavioral scorer expects to see *during* the challenge.
4. **Idle realism:** insert LogNormal idle gaps (1-5 s) between bursts rather
   than a fixed 4 s metronome — a perfectly periodic 4 s cycle is itself a
   weak periodicity tell.
5. **Determinism:** every random draw in the scheduler uses `_rand`
   (`op_behavior_random`, session-seeded per FIX-B), so a fixed
   `BROWSER_OXIDE_SESSION_SEED` reproduces the entire event stream for tests
   (the `chrome_compat.rs:5857/5908` slot tests already assert the slots are
   live; add a stream-reproducibility test).

**Two-level seed recap (the determinism contract):**
- **Level 1 — session seed** (FIX-B): one u64 per visitor-session/profile →
  `BehaviorProfile.seed`. Stable across all pages → consistent "person."
- **Level 2 — call salt:** `rng_for(salt)` folds the session seed with a
  per-call salt (path geometry, string, scroll target) so individual
  trajectories differ while the *style* is constant. Already implemented
  (`behavior.rs:109-115`); FIX-B just feeds it the right Level-1 value.

---

## 4. Which behavioral-/API-gated sites each fix unblocks

Honest mapping (from biometrics §4, cross-checked against the 19 failing
corpus sites). **No fix below flips a confirmed v0.1.0 corpus site** —
behavioral is not the current blocker for the failures (AWS = async PoW-worker
drain; SPA = hydration/sig; DataDome etsy = WASM daily-key; Kasada = holistic
ML tail). This is correctness hardening + v0.2.0 prerequisite, stated plainly.

| Fix | bestbuy (Akamai) | Kasada trio (canadagoose/hyatt/realtor) | etsy (DataDome) | mobile corpus (v0.2.0) | AWS cluster |
|---|---|---|---|---|---|
| **T** isTrusted primitive | maybe (PX/Castle check descriptor regardless of trajectory) | one holistic ingredient | already defeated by pre-pop | yes | **no** (async-drain) |
| **A** live Σ-Λ mouse | maybe (probe pending) | one ingredient | n/a | n/a | **no** |
| **B** session seed | — | one ingredient (over-dispersion) | — | prerequisite | **no** |
| **C** proactive keystroke | de-risks login probe | — | — | — | **no** |
| **E** scroll cadence | marginal | marginal | — | — | **no** |
| **D** touch | — | — | — | **required** | **no** |
| **F** click/visibility | marginal | marginal | — | marginal | **no** |

**Do NOT** spend behavioral effort on the AWS cluster (amazon-*, imdb,
booking) — biometrics §2.4 + HANDOFF_2026_05_28b §4 confirm the blocker is
the live-nav async drain (50 ms inter-script vs 5 s oracle), not behavioral.

---

## 5. Ranked fixes (effort + confidence)

ROI = (correctness/quality gain + strategic v0.2.0 value) / effort, given the
honest "0 confirmed v0.1.0 flips" reality. Recommended sequence: **T → A → B →
C** as one ~3-4 day sprint (the mouse/keystroke/trusted quality wins),
*after* AWS async-drain and SPA work, then D/E/F as a v0.2.0 mobile/holistic
follow-up.

| # | Fix | Effort | Confidence | Why this rank |
|---|---|---|---|---|
| **T** | **isTrusted prototype-accessor primitive + symbol-based marking in humanize** (§3.0) | 0.5 d | **high** | Free, load-bearing, upgrades ALL channels at once; the `getOwnPropertyDescriptor`/`configurable:true` tell is checked independent of trajectory quality — a perfect path with a forged own-prop still fails. Do first. |
| **A** | **Route live mouse cycle through `op_behavior_mouse_trajectory`** (§3.1) | 1-2 d | **high** | Biggest single quality gap; lerp efficiency≈1.0 is the #1 mouse tell (~98% catch); reuses shipped op; raises event count toward "hundreds." |
| **B** | **Session seed (Level-1) into `BehaviorRngState`/`BehaviorProfile`** (§3.2) | 0.5-1 d | **high** | Fixes over-dispersion (DMTG §2.3); prerequisite for multi-page session corpus; tiny change to an existing two-level scheme. |
| **C** | **Proactive keystroke trigger + locale token** (§3.3) | 1 d | medium | Turns the wired-but-dormant keystroke path live; de-risks bestbuy login probe; no value-mutation so safe on benign forms. |
| **E** | **Wire `wheel_burst` scroll op** (§3.4) | 1 d | medium | Generator + tests already exist; just needs an op + slot + cadence rewrite. |
| **F** | **Click chain + visibility/blur lifecycle** (§3.6) | 0.5 d | low | Removes "hovered, never clicked / never blurred" anomaly; marginal. |
| **D** | **Touch synthesis for mobile** (§3.5) | 4-5 d | medium | Only signal where BO is *worse than nothing* on mobile; needs new generator + constructible `Touch`/`TouchEvent`; required for v0.2.0 mobile corpus, not current desktop corpus. |
| — | accel push/pull asymmetry; diffusion (DMTG)-grade generator; BeCAPTCHA <50% | weeks | — | v1.0 frontier; out of scope for v0.1.0/v0.2.0 (`40` §7). |

---

## 6. Acceptance hooks

Existing (reuse): `behavior.rs:709-784` keystroke unit tests; `behavior.rs`
mouse tests (Fitts time, 8 ms cadence, velocity-CV>0.4, no-endpoint-jerk,
determinism); `behavior.rs:790-849` scroll-burst tests;
`chrome_compat.rs:5857` keystroke-slot + `:5908` behavior-rand-slot.

Add alongside the fixes:
- **FIX-T:** assert `Object.getOwnPropertyDescriptor(new MouseEvent('x'),
  'isTrusted') === undefined` (inherited getter), and that a page calling
  `Symbol.for('__bo_trusted__')` can NOT forge trusted (post-hardening).
- **FIX-A:** feed the *live-cycle* mouse stream (not just the generator) to a
  straightness/event-count check — efficiency in [0.25, 0.6], count in the
  hundreds over a multi-second window, no endpoint jerk outlier.
- **FIX-B:** same `BROWSER_OXIDE_SESSION_SEED` ⇒ byte-identical event stream
  across two simulated page loads; different seeds ⇒ different streams;
  *within* a session, σ_dwell/σ_mouse stable across pages.
- **FIX-C:** end-to-end: navigate a fixture page with a text input, assert
  `__akamai_events.key.length > 0` without the page autofocusing.
- **FIX-E:** wheel burst deltaY modal=100 (wheel) or 30-60 small-delta
  (trackpad) per profile; cadence LogNormal not uniform.
- **FIX-D:** mobile profile ⇒ `__akamai_events.touch.length > 0` and
  `new Touch({...})` / `new TouchEvent('touchstart', {...})` succeed.

---

## 7. Sources

- Repo: `crates/stealth/src/behavior.rs`, `crates/js_runtime/src/extensions/{input_ext,stealth_ext}.rs`,
  `crates/js_runtime/src/js/{event_bootstrap,stealth_bootstrap,timer_bootstrap}.js`,
  `crates/browser/src/js/humanize.js`, `crates/browser/src/page.rs`.
- Companion analysis: `docs/v0.1.0-parity-workflows/external/BEHAVIORAL_biometrics.md`
  (verified audit + DMTG/Castle/Bureau/Springer external set + ranked ROI).
- `docs/releases/v0.1.0-parity/40_TIMING_BEHAVIORAL.md` (canonical behavioral
  chapter; §3.2/§5 stale on keystroke+seed wiring — now done).
- Vendor deep-dives: `26_AKAMAI_BMP_DEEP.md` §3 (sensor_data reads
  `__akamai_events`), `07_DATADOME_PRIMITIVES.md` (tags.js 31-feature vector),
  `08_KASADA_FRONTIER.md` §3 (RAF cadence), `06_AWS_WAF_SOLVER.md` (async
  drain, NOT behavioral), `docs/HANDOFF_2026_05_28b.md` §4-5.
- External (via biometrics §8): Plamondon 1995 (Σ-Λ kinematics); BeCAPTCHA-Mouse;
  DMTG arXiv 2410.18233 (diffusion generator catch-rate table); Castle "Bot or
  Not" (378 vs 4 mousemove); Bureau/ScrapingAnt (path efficiency); Springer 2026
  (keystroke vs mouse ML); CMU+Buffalo keystroke benchmarks.
