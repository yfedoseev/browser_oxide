# 05 — bestbuy / Akamai BMP, reframed as a BEHAVIORAL gap

**Site:** bestbuy.com — Akamai Bot Manager Premier (BMP) `bmak` sensor on a
React SPA.
**Mission framing (ground truth, per the user's directive):** a human opens
bestbuy in a real Chrome from this IP. Therefore — *holding the IP constant* —
bestbuy is not purely IP-bound. This doc leads with the **behavioral path**:
what Akamai's `bmak` collects into `sensor_data`, what BO emits today
(file:line), and the exact engine change to feed `bmak` **trusted human
entropy** so its behavioral sub-score reads human instead of zero/synthetic.
We flag honestly where a Favorable `_abck` may still be gated by ASN trust —
but we do not retreat to "IP-bound" before exhausting the behavioral lever.

**Reading order — extends, does not replace:**
- `docs/v0.1.0-frontier-workflows/03_BESTBUY_AKAMAI.md` (the IP/ASN-leaning
  frontier verdict — this doc is its behavioral counterweight)
- `docs/releases/v0.1.0-parity/26_AKAMAI_BMP_DEEP.md` §2 (BMP mechanism +
  `_abck` state machine + field taxonomy)
- `docs/v0.1.0-parity-workflows/external/VENDOR_akamai.md` §2-3 (sec-cpt
  provider taxonomy, `__akamai_events` unfed surface)
- `docs/v0.1.0-parity-workflows/external/BEHAVIORAL_biometrics.md` §3, §7
  (the verified humanize.js code audit + ranked behavioral fixes)
- `docs/releases/v0.1.0-parity/40_TIMING_BEHAVIORAL.md` §3 (the Σ-Λ mouse +
  keystroke generators)

---

## 0. TL;DR

1. **`bmak` self-collects behavioral entropy from real DOM event listeners.**
   The `akam/13` bootstrap installs `addEventListener` taps for mouse
   (`mact`), key (`kact`), touch (`tact`), device-orientation (`doact`) and
   device-motion (`dmact`), accumulates coordinate+timestamp arrays, and folds
   them into the `sensor_data` POST that the server scores 0 (human) – 100
   (bot) before deciding the `_abck` verdict. A session with **zero**
   behavioral events POSTs an empty `mact`/`kact` — the strongest single
   bot tell short of the IP.

2. **BO already dispatches a sophisticated synthetic mouse/scroll stream**
   (humanize.js Σ-Λ pre-pop + 4 s live cycle, `isTrusted` forced true). So
   `bmak`'s `mact` would NOT be empty under BO — this is a real and
   under-appreciated asset. **But three concrete behavioral gaps remain**, and
   each is a `bmak`-readable tell: (a) the live-cycle mouse path is
   **linear lerp** (straightness ≈ 1.0, the #1 mouse tell), (b) **zero touch
   events** on the mobile profiles (`maxTouchPoints=5 && tact===""`), and
   (c) **zero device-motion/orientation events** on mobile (`doact`/`dmact`
   empty while `DeviceMotionEvent` exists).

3. **The `isTrusted` mechanism in BO is half-wired and is the highest-value,
   lowest-effort behavioral fix.** humanize.js sets `isTrusted` via
   `Object.defineProperty(ev,'isTrusted',{value:true})` AFTER construction
   (`humanize.js:105,429,441`) — a per-instance own-property shadow. The
   engine ALSO has a clean native path: `event_bootstrap.js:19` reads
   `Symbol.for('__bo_trusted__')` from the constructor options to set
   `isTrusted` as the event is built. humanize.js does **not** use it.
   The defineProperty shadow is detectable
   (`Object.getOwnPropertyDescriptor(ev,'isTrusted')` returns a data
   descriptor on the instance, whereas a genuine `isTrusted` is an accessor
   on `Event.prototype`). Routing humanize through the `__bo_trusted__` symbol
   makes the trust signal **structurally indistinguishable** from a real
   browser event — exactly the from-scratch-engine advantage the mission
   calls out.

4. **Honest IP caveat (does not block the behavioral work):** real-Chromium
   Patchright + Camoufox v150 + BO all land on the same ~7-8 KB shell from
   this datacenter IP (`26 §4.3`). That pattern is consistent with an ASN
   trust floor on the `_abck` score. The behavioral fixes here are necessary
   (they remove the behavioral sub-score tells) but may be insufficient alone
   if the ASN sub-score dominates. The correct experiment to disambiguate is
   §4 Capture B (no-CDP Chrome, datacenter vs residential) — but **the
   behavioral fixes are profile-neutral public-engine wins regardless** and
   are the prerequisite the private `vendor_solvers` sensor encoder consumes.

---

## 1. What a real browser does — `bmak` behavioral collection, reconstructed

### 1.1 The collection lifecycle

The `akam/13/<hash>` bootstrap (the ~80-512 KB obfuscated `bmak` bundle)
installs DOM event listeners on page load and accumulates per-type behavioral
buffers, then encrypts them into the `sensor_data` POST body. Per the public
deobfuscation corpus (xiaoweigege, Edioff, glizzy, the CSDN/programmer.ink
walkthroughs, scrapebadger) the behavioral fields are:

| `bmak` field | Internal name | What the listener records | Real-session shape |
|---|---|---|---|
| `mact` | `mouse_action` | `mousemove`/`mousedown`/`mouseup`/`click`: `evt_type, x, y, t_delta` tuples, semicolon/comma-packed | **hundreds** of move tuples + a few clicks; curved, jittered, variable-velocity |
| `kact` | `key_action` | `keydown`/`keyup`/`keypress`: `evt_type, keyCode_bucketed, t_delta` (NOT the literal key — bucketed for privacy) | dwell+flight cadence if any field was typed; empty if no typing |
| `tact` | `touch_action` | `touchstart`/`touchmove`/`touchend`: `x, y, t_delta`, `Touch.radiusX/force` | non-empty on mobile UA; **empty = bot tell on mobile** |
| `doact` | device-orientation | `deviceorientation`: `alpha, beta, gamma, t` | non-empty on a real handheld; tiny continuous drift |
| `dmact` | device-motion | `devicemotion`: `acceleration{x,y,z}`, `rotationRate`, `t` | continuous micro-jitter on a real phone (the `accel` counter) |
| counters | — | per-type event counts (`mact` count, `kact` count, …) | the first-POST cadence is `"16,0,0,0,0,0"` (load events), the second after activity `"5,18,0,0,1,323"` (5 key, 18 mouse, 1 scroll, 323 accel) — `26 §2.3` |

Each behavioral field is emitted in the `sensor_data` cleartext under a marker
triplet, e.g. `-1,2,-94,-110,<mact>` for mouse and `-1,2,-94,-117,<kact>` for
key (the `-94` family of field markers; the exact triplets rotate per bundle).
The whole cleartext (behavioral + the ~100 static fingerprint fields: canvas,
WebGL, audio, screen, UA-CH, tz, `hardwareConcurrency`, plugins) is TEA-CBC
(v2) or PRNG-shuffled-JSON (v3) encrypted and POSTed. Server scores it and
sets `_abck` (`26 §2.2`): `~-1~-1~-1~` Favorable, `~0~-1~-1~` Untrusted,
`~3~` Rejected.

### 1.2 What the behavioral sub-score actually penalizes

From the 2024-2026 vendor literature (`BEHAVIORAL_biometrics.md §2.2`, Castle,
Bureau, ScrapingAnt, DMTG):
- **Empty `mact`/`kact`** = the cleanest tell. A real homepage visit produces
  continuous `mousemove` even before any click (Castle: 378 human vs 4 bot).
- **Mouse straightness / efficiency** (`Euclidean(start,end)/pathLength`):
  bots peak ≥ 0.94, humans 0.3-0.5. A perfectly-straight lerp between points
  is flagged directly.
- **Near-zero acceleration** between direction changes (constant velocity) —
  "human acceleration is almost never close to zero" (Castle).
- **Coordinate teleports** (instant jumps, no intermediate samples).
- **Mobile UA with `tact===""` / `dmact===""`** — a phone that never touched
  the screen and reports zero accelerometer jitter is a headless-mobile tell.
- **`isTrusted===false`** on dispatched events — synthetic
  `new MouseEvent(...).dispatchEvent()` carries `isTrusted=false`; the
  collector can branch on it.

---

## 2. What BO does today (file:line)

### 2.1 The behavioral stream IS emitted — and it is good

BO is **not** an empty-buffer scraper. `Page::navigate` injects
`crates/browser/src/js/humanize.js` (`page.rs:1081-1082`,
`:1096-1097`) on every navigate. humanize.js:

- **Synchronous mouse pre-population** (`humanize.js:357-452`,
  `_seedHistoricalCoords`): calls `op_behavior_mouse_trajectory`
  (`stealth_ext.rs:185` → `stealth::behavior::mouse_trajectory`, the Σ-Λ
  Plamondon generator), projects ~14 points onto a `[-1800,-100] ms`
  historical window, pushes them into `__akamai_events.mouse` **before the
  IIFE returns**, and dispatches one synchronous `mousemove`+`pointermove`
  pair on `window`+`document`+`body`. So any listener `bmak` attaches sees a
  non-empty curved history from the first instant.
- **A 4 s live cycle** (`humanize.js:292-336,455-457`, `runCycle` +
  `setInterval(runCycle,4000)`): 2-stroke mouse motion, a scroll burst, a
  `window focus` + `visibilitychange`.
- **`__akamai_events` buffer** (`humanize.js:69-100`):
  `{mouse[],key[],touch[],scroll[],counters{key,mouse,touch,scroll,accel}}`
  — the exact drain surface the (stripped, now-private) Akamai sensor encoder
  reads (`26 §1`, `VENDOR_akamai.md §3.3`).
- **Keystroke schedule wired** (`humanize.js:116-154`): on `focusin` of an
  `INPUT`/`TEXTAREA`, calls `op_human_keystroke_schedule`
  (`input_ext.rs:157`) and dispatches `keydown`/`keyup` at bigram-modulated
  LogNormal offsets, recording into `_akRecKey`.

### 2.2 Gap A — `isTrusted` is set via a detectable per-instance shadow

humanize.js marks events trusted with:

```js
// humanize.js:104-108
function _dispatch(target, event) {
    try { Object.defineProperty(event, 'isTrusted', { value: true, configurable: true }); }
    catch (e) {}
    target.dispatchEvent(event);
}
```

and identically at `humanize.js:429` and `:441`. This writes `isTrusted` as an
**own data property on the event instance**. A real browser exposes
`isTrusted` as an **accessor on `Event.prototype`** (the instance has no own
`isTrusted`). So:

```js
// bmak-side detector (trivial):
const d = Object.getOwnPropertyDescriptor(ev, 'isTrusted');
// real event:  d === undefined  (it's inherited from the prototype accessor)
// BO event:    d === {value:true, writable:false, enumerable:false, configurable:true}
```

Meanwhile the engine **already has the correct native path**:
`event_bootstrap.js:7,19` —

```js
const _TRUSTED = Symbol.for('__bo_trusted__');
// in Event constructor:
this.isTrusted = !!(options && options[_TRUSTED] === true);
```

— sets `isTrusted` as a normal instance field at construction time, the same
shape a page-constructed event has (since BO's `Event` defines `isTrusted` as
an own field on every instance, the symbol path is consistent with the rest of
the engine's event objects and leaves no defineProperty descriptor anomaly).
humanize.js does not use this symbol; the only consumer today is a test
(`behavioral_polish.rs:114`).

**This is the single cleanest behavioral fix: route every humanize dispatch
through `new MouseEvent(type, {..., [Symbol.for('__bo_trusted__')]: true})`
and delete the `_dispatch` defineProperty shadow.** It removes the
descriptor-mismatch tell entirely. (Note: the BO `Event` model puts
`isTrusted` on the instance for *all* events including page-constructed ones,
so the descriptor is uniform — the symbol path is still preferable because it
removes the post-construction mutation and the `configurable:true` flag that a
real read-only `isTrusted` never has.)

### 2.3 Gap B — live-cycle mouse is linear (straightness ≈ 1.0)

`runCycle` interpolates **linearly** between 3 random anchors:

```js
// humanize.js:309-324
const [x, y] = _lerp(a, b, tau);   // straight segment, efficiency ≈ 1.0
```

The Σ-Λ sample *times* are correct (`_sigmaLognormalTimes`, line 313) but the
*path* is straight with white-noise jitter (`_gauss()*0.8`). The pre-pop uses
the good Rust generator; the live cycle does not. So after the first ~2 s the
ongoing `mact` stream BO feeds `bmak` is high-straightness — the #1 mouse tell
(`BEHAVIORAL_biometrics.md §2.2`, Bureau 0.94 bot vs 0.3-0.5 human). The Rust
generator (`op_behavior_mouse_trajectory`, already an op) produces curved
multi-stroke paths with pink tremor + smoothstep terminal decel; the live
cycle just needs to call it.

### 2.4 Gap C — zero touch / device-motion on mobile profiles

- **No `TouchEvent` is ever dispatched.** `__akamai_events.touch` is declared
  (`humanize.js:71`) but no `_akRecTouch` exists and the `touch` counter never
  increments (`BEHAVIORAL_biometrics.md §3.4`). The mobile presets set
  `max_touch_points: 5` (`presets.rs:875`). Result: `bmak`'s `tact` is empty
  while `navigator.maxTouchPoints===5` and `ontouchstart` exists — a clean
  `maxTouchPoints>0 && tact==="" && UA~Mobile` tell. There is **no touch
  generator in `behavior.rs`** (only a comment at `:498`).
- **No `deviceorientation`/`devicemotion`** is ever dispatched, so `doact` and
  `dmact` are empty and `counters.accel` stays 0 — but a real phone reports the
  `"...,1,323"` accel cadence (`26 §2.3`). On the iPhone/Pixel profiles this is
  a second mobile tell.

**This is the behavioral signal where BO is *strictly worse than nothing* on
its own mobile profiles** — a mobile UA with zero touch/motion is more
suspicious than a desktop UA with synthetic mouse.

### 2.5 Gap D — keystroke is reactive-only (zero key events on a homepage)

The keystroke schedule fires only on a page-driven `focusin`
(`humanize.js:119`). bestbuy's homepage does not autofocus a search box, so
`kact` is empty. That matches a real user who also hasn't typed yet, so it is a
*minor* tell on a homepage — but for any flow that scores "focused a field then
typed unrealistically" (login), proactive triggering (`BEHAVIORAL_biometrics.md
§7 FIX-C`) de-risks it.

### 2.6 The drain: does `bmak` get to run + POST at all?

bestbuy gets the **25 s plain-BMP budget tier** (`page.rs:1977-1985`,
`bestbuy.com` → 25_000 ms) on the cold navigate, and the per-iteration drain is
adaptive (floored 8 s — `page.rs:684`). So on the cold path `bmak` has ample
contiguous time to install listeners, let humanize feed them, and POST. The
**50 ms warm-rebuild drain** (`page.rs:1705`) only bites if the SPA issues a
soft reload after the first POST (`frontier 03 §3.1`). So execution time is
*probably* not the behavioral blocker on the cold navigate — but Capture A
(§4) is what confirms whether the POST actually fires.

---

## 3. The gap + the exact engine change

### 3.1 The engine path the mission asks for

> "generate the `bmak` behavioral payload by running the real `bmak` JS in our
> V8 while the behavioral engine feeds it trusted mouse/key/scroll events."

This already happens structurally — `bmak` runs in BO's V8 and humanize.js
feeds it events. The work is to make those events **(a) trusted at the native
level, (b) human-shaped on the live path, (c) present on mobile (touch +
motion)**. Concretely:

**FIX-1 (trusted native events) — `humanize.js` + `event_bootstrap.js`.**
Replace the `_dispatch` defineProperty shadow with the constructor symbol.
For every event humanize builds, pass `[Symbol.for('__bo_trusted__')]: true`
in the options bag and drop the `Object.defineProperty(...,'isTrusted',...)`
lines (`humanize.js:105,429,441`). Verify `MouseEvent`/`PointerEvent`/
`KeyboardEvent`/`WheelEvent`/`TouchEvent` constructors all forward the symbol
to `Event` (they extend `UIEvent`→`Event`, `event_bootstrap.js:56` — confirm
the options bag is passed through `super(type, options)` for each subclass; if
a subclass strips unknown keys, thread the symbol explicitly).
- Effort: ~0.5 day. Confidence: high. Public crate.
- Removes the descriptor-mismatch + `configurable:true` tells for **every**
  vendor that branches on `isTrusted` (Akamai, PerimeterX/HUMAN, DataDome).

**FIX-2 (curved live-cycle mouse) — `humanize.js:309-326`.**
Replace the `_lerp` anchor loop with calls to `op_behavior_mouse_trajectory`
(already exposed, used by the pre-pop at `:374-377`). Walk anchor→anchor
through the Σ-Λ generator instead of straight segments; keep the existing
`_fireMove` dispatch (now trusted via FIX-1). Drops straightness 1.0 → ~0.4,
restores asymmetric acceleration + bounded endpoint jerk, and at the
generator's 8 ms sample rate pushes the ongoing `mact` count toward "hundreds".
- Effort: 1-2 days. Confidence: high. Public crate.

**FIX-3 (touch + device-motion synthesis on mobile) —
`stealth/src/behavior.rs` + `input_ext.rs` + `humanize.js`.**
Add a `touch_swipe`/`tap` generator (reuse the Σ-Λ sampler) and a
device-motion micro-jitter generator in `behavior.rs`; expose ops; in
humanize.js, gate on `navigator.maxTouchPoints>0` and dispatch real
`TouchEvent`s (with `Touch.radiusX/radiusY/force`) into `__akamai_events.touch`
via a new `_akRecTouch`, plus periodic `deviceorientation`/`devicemotion`
events incrementing `counters.accel`. All trusted via FIX-1.
- Effort: 4-5 days. Confidence: medium. Public crate.
- Removes the `mobile-UA && tact==="" && dmact===""` tell — the one place BO
  is currently worse-than-nothing on iPhone/Pixel profiles.

**FIX-4 (proactive keystroke) — `humanize.js`.**
One-shot: after the first `runCycle`, query for the primary visible text input
(bestbuy's search box) and `focus()` it so the already-wired keystroke schedule
fires, producing a realistic `kact`. Guard with the existing single-shot
Symbol tag and short token to avoid polluting real form listeners.
- Effort: ~1 day. Confidence: medium. Public crate.

**FIX-5 (the consumer) — `vendor_solvers` Akamai sensor encoder.**
Even a perfect `__akamai_events` buffer does nothing on bestbuy until something
**reads it and POSTs `sensor_data`**. `Page::default_solvers()` returns
`Arc<[]>` (`page.rs:850`), so nothing drains the buffer publicly. The encoder
(stripped by `aecdf19`: `crates/akamai/src/{payload,v3_payload,crypto,
tea_cbc}.rs`) must live in the private `vendor_solvers` crate and be registered
via `Page::with_solvers(...)`. FIX-1..4 are its **prerequisites** — they
determine whether the `mact`/`kact`/`tact` it encodes read human or bot.
- Effort: weeks (re-port). Confidence on flip: low-medium (gated by §3.2).
  **Private crate — must NOT go in public per CLAUDE.md / `aecdf19`.**

### 3.2 The honest IP caveat

The behavioral fixes raise the BMP **behavioral sub-score** toward human. They
do **not** change the **IP/ASN sub-score**. The evidence that the ASN sub-score
may dominate on bestbuy: real-Chromium Patchright (real trusted events, real
event loop, CDP stripped) AND Camoufox v150 AND BO all land on the identical
~7-8 KB shell from this datacenter IP (`26 §4.3`, `frontier 03 §1.2`). If the
ASN floor caps the score below Favorable regardless of behavior, FIX-1..5 are
necessary-but-insufficient and a residential/mobile egress IP is the remaining
lever (out of engine scope). **The disambiguating experiment is §4 Capture B.**
The behavioral fixes are still worth landing first because (a) they are
profile-neutral public-engine quality wins that help every behaviorally-scored
vendor, (b) they are the prerequisite the encoder consumes, and (c) the user's
directive is explicit: do not assert IP-bound for a site a real browser opens
from this IP without first closing the behavioral + API gap.

---

## 4. Validation captures (do NOT run live navs on the contended IP)

Fold into the AWS §5.1 capture window or a spare IP.

- **Capture A — does `bmak` POST `sensor_data` under a long drain, and what
  does `mact`/`kact` contain?** Fork `crates/browser/examples/awswaf_probe.rs`
  → `akamai_probe.rs`. Load a captured bestbuy shell + `akam/13` body,
  pre-inject an `addEventListener` Proxy, `run_until_idle(30 s)`, dump: did it
  POST? what `_abck` came back (`~0~-1~-1~` vs `~3~` vs no POST)? read
  `__akamai_events` to confirm humanize fed it. **If it POSTs under 30 s but
  not under the 50 ms warm drain → drain branch (public fix). If `mact` is
  populated but `_abck` stays Untrusted → behavioral-or-IP score branch.**
- **Capture B — no-CDP real Chrome (the IP disambiguator).** `nocdp.sh` real
  Chrome → bestbuy zero-interaction, **datacenter IP vs residential IP**. If
  datacenter hydrates → engine/behavioral-addressable (diff its `sensor_data`
  vs BO's). If only residential hydrates → ASN floor confirmed; behavioral
  fixes alone won't flip it.
- **Capture C — `isTrusted` descriptor probe.** In the offline oracle, after
  humanize runs, assert `Object.getOwnPropertyDescriptor(ev,'isTrusted')`
  before FIX-1 (data descriptor — anomalous) and after FIX-1 (matches a
  page-constructed event). Add as a `chrome_compat.rs` regression test.

---

## 5. Which behavioral/API-gated sites these fixes unblock

| Fix | bestbuy | Other sites it helps |
|---|---|---|
| FIX-1 trusted native events | removes `isTrusted` descriptor tell from `mact`/`kact` | **all** behaviorally-scored vendors: PerimeterX/HUMAN (canadagoose/hyatt/realtor Kasada is separate), DataDome (etsy), any future PerimeterX target |
| FIX-2 curved live mouse | raises ongoing `mact` straightness→human | Kasada holistic ML tail (one ingredient), DataDome `tags.js` 31-feature vector, any future Castle/Bureau target |
| FIX-3 touch+motion | removes mobile `tact===""`/`dmact===""` tell | any mobile-profile Akamai/PerimeterX/F5/Castle target; prerequisite for mobile-app-protected sites |
| FIX-4 proactive keystroke | populates `kact` for the search/login flow | de-risks any form-gated corpus site |
| FIX-5 sensor encoder (private) | the actual POST that turns the buffer into an `_abck` attempt | homedepot `behavioral`/`adaptive` days, nike/adidas/walmart/samsclub BMP class |

**Honest expected v0.1.0 corpus flips from FIX-1..4 alone: 0 confirmed** —
consistent with `BEHAVIORAL_biometrics.md §4,§7` (no current corpus site is
*gated* on behavioral presence; behavioral is the v0.2.0 frontier). Their value
is (a) closing the behavioral sub-score tells so that **when FIX-5 lands** the
encoded `sensor_data` reads human, and (b) hardening every behaviorally-scored
vendor's holistic score. bestbuy specifically flips only if Capture B shows the
ASN floor is not the binding constraint.

---

## 6. Ranked fixes (effort + confidence)

| # | Fix | Effort | Confidence (closes the tell) | Confidence (flips bestbuy) | Public? |
|---|---|---|---|---|---|
| **1** | **Trusted native events via `__bo_trusted__` symbol** (drop the defineProperty shadow) | 0.5 day | **high** | low (one of N sub-scores) | public |
| **2** | **Curved live-cycle mouse** (route `runCycle` through `op_behavior_mouse_trajectory`) | 1-2 days | **high** | low | public |
| **3** | **Touch + device-motion synthesis on mobile** (`behavior.rs` gen + op + humanize dispatch) | 4-5 days | medium | low (and only on mobile profiles) | public |
| **4** | **Proactive keystroke trigger** (focus the primary input post-cycle) | 1 day | medium | low | public |
| **5** | **`vendor_solvers` Akamai sensor encoder** (re-port the stripped `payload/v3_payload/crypto/tea_cbc`) — the consumer that actually POSTs | weeks | high (it POSTs) | low-medium (gated by ASN, §3.2) | **private** |
| **—** | Residential/mobile egress IP for the `_abck` ASN sub-score | n/a | n/a | medium-high (if Capture B confirms ASN floor) | **out of engine** |

**Recommended sequence:** FIX-1 → FIX-2 → FIX-4 (one ~3-4 day sprint, all the
desktop mouse/key/trust quality + the descriptor-tell removal), then run
Capture A + Capture B. If Capture B shows datacenter no-CDP Chrome hydrates →
the behavioral path is the lever, prioritize FIX-3 + FIX-5. If only residential
hydrates → land FIX-1/2/4 anyway (cross-vendor wins), park bestbuy as
ASN-gated + FIX-5-dependent, and stop. **Do NOT** re-add the sensor encoder to
a public crate (forbidden, `aecdf19`/CLAUDE.md), and **do NOT** assert
"IP-bound" before FIX-1/2 + Capture B — the user's directive is to exhaust the
behavioral + trusted-event path first.

---

## 7. Sources

**Internal:**
- `docs/v0.1.0-frontier-workflows/03_BESTBUY_AKAMAI.md` (IP/ASN verdict,
  Patchright-debunk, cross-engine shell table, drain analysis)
- `docs/releases/v0.1.0-parity/26_AKAMAI_BMP_DEEP.md` §2.2-2.3 (`_abck` state
  machine, first/second POST counter cadence), §4.3 (bestbuy cross-engine)
- `docs/v0.1.0-parity-workflows/external/VENDOR_akamai.md` §2-3
  (sec-cpt provider taxonomy, `__akamai_events` unfed surface)
- `docs/v0.1.0-parity-workflows/external/BEHAVIORAL_biometrics.md` §2-4,§7
  (humanize.js code audit, ranked behavioral fixes, vendor scoring literature)
- `docs/releases/v0.1.0-parity/40_TIMING_BEHAVIORAL.md` §3 (Σ-Λ + keystroke)

**BO source (verified this session):**
- `crates/browser/src/js/humanize.js`: `__akamai_events` buffer `:69-100`;
  `_dispatch` defineProperty shadow `:104-108`; keystroke `focusin` `:116-154`;
  `_fireMove` pointer pairing `:235-271`; `_fireScrollStep` `:274-288`;
  linear-lerp live cycle `:292-336`; Σ-Λ pre-pop `:357-452`; trusted-shadow
  again `:429,:441`; `runCycle` install `:455-457`
- `crates/js_runtime/src/js/event_bootstrap.js:7,19` (`__bo_trusted__` symbol →
  `isTrusted` at construction — the native trusted path humanize doesn't use)
- `crates/js_runtime/src/extensions/input_ext.rs`: `op_behavior_random:54`,
  `op_human_mouse_path:67`, `op_human_keystroke_schedule:157`, ext `:190`
- `crates/js_runtime/src/extensions/stealth_ext.rs:185`
  (`op_behavior_mouse_trajectory`)
- `crates/stealth/src/behavior.rs` (Σ-Λ mouse, keystroke, `wheel_burst`; no
  touch/motion generator)
- `crates/browser/src/page.rs`: humanize inject `:1081-1097`; `__akamai_events`
  reset `:1385-1395`; warm 50 ms drain `:1705`; 500 ms final drain `:1757`;
  cold drain floor 8 s `:684`; bestbuy 25 s budget tier `:1977-1985`;
  `default_solvers` empty `:850`
- `crates/browser/tests/behavioral_polish.rs:110-114` (only `__bo_trusted__`
  consumer today)

**External (2024-2026):**
- bmak behavioral fields (`mact`/`kact`/`tact`/`doact`/`dmact`, the `-94`
  marker family) — xiaoweigege/akamai2.0-sensor_data
  (https://github.com/xiaoweigege/akamai2.0-sensor_data);
  CSDN bmak walkthrough
  (https://blog.csdn.net/weixin_42114689/article/details/120630123);
  programmer.ink "analyze the data structure in log and sensor data"
  (https://programmer.ink/think/chapter-3-analyze-the-data-structure-in-log-and-sensor-data.html);
  scrapebadger Akamai bypass (https://scrapebadger.com/akamai-bypass);
  XVI.cool "Starting Akamai Part I" (https://xvi.cool/blog-posts/starting-akamai-p1)
- sensor_data behavioral + canvas/motion weighting, `_abck` returned on valid
  POST — medium.com/@240942649 "Decoding Akamai 2.0"
  (https://medium.com/@240942649/decoding-akamai-2-0-418e7c7fa0a0);
  Scrapfly Akamai bypass (https://scrapfly.io/bypass/akamai)
- mouse straightness/efficiency, event-count, zero-acceleration tells —
  Castle "Bot or Not"
  (https://blog.castle.io/bot-or-not-can-you-spot-the-automated-mouse-movements/);
  Bureau
  (https://bureau.id/resources/blog/mouse-movement-behavioral-patterns-can-reliably-tell-bots-from-humans);
  ScrapingAnt (https://scrapingant.com/blog/detect-bot-by-cursor)
- anti-bot detection surface incl. behavioral 35+ signals — microlinkhq/is-antibot
  (https://github.com/microlinkhq/is-antibot);
  Scrapfly "How to Bypass Akamai 2026"
  (https://scrapfly.io/blog/posts/how-to-bypass-akamai-anti-scraping)
- DMTG diffusion trajectory generator + discriminator catch-rates
  (https://arxiv.org/html/2410.18233v1)
