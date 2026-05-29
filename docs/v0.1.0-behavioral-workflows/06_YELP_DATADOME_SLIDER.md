# 06 — yelp DataDome slider (`rt:'c'`) reframed: behavioral path, drag geometry, honest verdict

**Mission frame (the user's directive).** A human opens yelp in a real Chrome from
this IP and gets in. Therefore yelp is not purely IP-bound; find the engine +
behavioral gap. This doc reframes the long-standing "yelp = human gate, out of
scope" verdict (`02_DATADOME_DEEP.md` §5, `VENDOR_datadome.md` §"Concrete targets")
against the actual DataDome slider mechanism and BO's behavioral/event code, and
asks the three questions the brief poses:

- **(a)** Does DataDome *auto-clear* the slider when behavioral signal is strong
  (so a real user never drags), or does a real user actually drag it?
- **(b)** If a drag is needed, the geometry of a human-like grab-and-drag to the
  gap with trusted events — generatable by BO's behavioral engine?
- **(c)** The DataDome behavioral/device-check payload, and the honest engine path
  to pass yelp: confident drag vs. solving the visual puzzle gap.

**One-paragraph answer up front.** The decisive finding is **(a)**: per DataDome's
own product copy, **"less than 0.01% of users ever encounter their slider"**
([DataDome Device Check](https://datadome.co/products/datadome-device-check/),
[CAPTCHA Alternative](https://datadome.co/products/captcha-alternative/)). The
real-Chrome user from this IP almost certainly **never sees the slider** — the
**silent Device Check (`rt:'i'`)** clears them first, exactly the etsy/tripadvisor
path. yelp serving BO the *interactive* slider (`rt:'c'`) is a **symptom of BO
scoring worse on the upstream ML gate**, not a fixed property of yelp. So the
single highest-value engine work for yelp is the **same `rt:'i'` self-solve cluster
as etsy** (no-CDP cleanliness raising trust score + the child-iframe cookie-jar fix
`02_DATADOME_DEEP.md` §3.3) — *get the silent challenge instead of the slider*.
**If** BO is still served `rt:'c'`, the slider genuinely requires (1) a **visual
gap-offset solve** of the puzzle image (`.jpg` background + `.frag.png` piece;
[Hyper Solutions slider doc](https://docs.hypersolutions.co/datadome/slider-captcha))
and (2) a trusted human-like drag to that x-offset. (1) is a CV/template-match
solve that is **per-vendor bypass = `vendor_solvers`-only**; (2) is a real public
engine gap (BO synthesizes **zero** `mousedown/mousemove/mouseup` drag and routes
trusted events through a brittle late `defineProperty` path, not the engine's
`__bo_trusted__` mechanism). **A confident drag alone does not pass it** — the
landing x must match the gap, and the gap is image-dependent. Camoufox v150 also
fails yelp and has **no slider code** (deepwiki, this session). Net ranking: the
yelp win is the `rt:'i'` cluster (public, shared with etsy); the slider drag
primitives are worth building for the broader behavioral engine (public), but the
gap-image solve is `vendor_solvers` and even then low-confidence + daily-rotated.

---

## 1. What a real browser does

### 1.1 The real user almost never sees the slider — Device Check clears them

DataDome scores every request with an ML model fed by TLS + HTTP/2 fingerprint,
browser fingerprint, behavioral signals, and IP reputation, then routes to one of
([ZenRows](https://www.zenrows.com/blog/datadome-bypass),
[Scrapfly](https://scrapfly.io/bypass/datadome), `02_DATADOME_DEEP.md` §1.3):

| Outcome | What the user experiences |
|---|---|
| **Allow** | Page loads. No challenge. |
| **Silent Device Check** (`rt:'i'` interstitial) | JS + WASM PoW runs invisibly, posts a payload, gets `datadome=`, page reloads. **No human action — "Real users are not interrupted."** ([DataDome Device Check](https://datadome.co/products/datadome-device-check/)) |
| **Slider** (`rt:'c'` captcha) | A visible slider: drag the handle right to seat a puzzle piece into the gap in a background image. |

DataDome's own product copy is the load-bearing fact for the brief's question (a):

> *"Device Check provides invisible verification for most traffic… because it's
> invisible to users, Device Check reduces the number of visible challenges
> (CAPTCHAs) users face. Real users are not interrupted."*
> — [DataDome Device Check](https://datadome.co/products/datadome-device-check/)

> *"…our lightweight slider… **less than 0.01% of users ever encounter** their
> slider."* — [DataDome CAPTCHA Alternative](https://datadome.co/products/captcha-alternative/)

**Conclusion for (a):** the real-Chrome user from this IP passes yelp by being
silently cleared (`rt:'i'` or Allow), **not by dragging a slider**. The slider is
reserved for borderline/elevated-threat scoring. So "a human opens yelp" is
evidence that **the silent path is reachable from this IP** — which makes yelp's
*engine-addressable* lever the same `rt:'i'` self-solve cluster as etsy, not a
slider solver. BO being shown `rt:'c'` means BO's trust score landed in the
slider band, i.e. BO scored *worse than the real browser*.

### 1.2 When the slider IS shown, what the human does

If scoring lands in the slider band, the human:
1. Sees a background image with a notch/gap and a puzzle piece, plus a slider track.
2. Moves the cursor (already with a rich pre-slider movement history) to the handle.
3. **Presses** (mousedown/pointerdown/touchstart) on the handle.
4. **Drags** right with a natural velocity profile (accelerate, overshoot/correct,
   decelerate) until the piece visually seats in the gap.
5. **Releases** (mouseup/pointerup/touchend).

DataDome captures, throughout: the `_initialCoordsList` (movements from page load to
the slide-button click) and `_coordsList` (the drag), and computes **~31 signals**
(curvature, length, straightness, x/y standard deviation, average x/y speed,
timestamps) plus device traits, and validates that the **landing x matches the gap
x-offset** ([the cursor moves "from a starting x value to an x value determined by
the location of the puzzle piece"](https://github.com/joekav/SlideCaptcha),
[glizzykingdreko](https://medium.com/@glizzykingdreko/breaking-down-datadome-captcha-waf-d7b68cef3e21)).
**Both** the path biometrics **and** the geometric landing must be right — a
confident human-shaped drag that lands at the wrong x fails.

### 1.3 Trusted events

The drag the human produces is **browser-generated → `isTrusted === true`**. Page
JS that calls `new MouseEvent(...)+dispatchEvent()` produces `isTrusted === false`;
DataDome (and every behavioral vendor) checks this. A from-scratch engine that owns
the event loop can mint genuinely engine-blessed trusted events — this is BO's
structural advantage (brief area C), and BO already has the mechanism (§2.3).

---

## 2. What BO does today (file:line)

### 2.1 BO never synthesizes a drag — only hover

`crates/browser/src/js/humanize.js` is the entire behavioral layer (458 lines). Its
output is **hover + scroll + reactive keystroke** only:

- `runCycle()` (humanize.js:292-336) fires `mousemove`+`pointermove` pairs along
  linearly-interpolated anchor segments (`_lerp`, humanize.js:316), plus 2 scroll
  steps.
- `_seedHistoricalCoords()` (humanize.js:357-452) synchronously pre-populates the
  movement buffer + dispatches one synchronous move pair (the DataDome
  `_initialCoordsList` defeat).
- **There is NO `mousedown` / `mouseup` / `click` / `pointerdown` / `pointerup` /
  drag anywhere in humanize.js.** Verified: `grep mousedown|mouseup|click|pointerdown|drag humanize.js`
  returns only the file's header comment (humanize.js:3). `BEHAVIORAL_biometrics.md`
  §3.5 records the same: *"no `pointerdown`/`pointerup`/`click` sequence is ever
  synthesized… a vendor scoring 'moved but never clicked anything in 30 s' sees
  pure hover."* So BO **cannot grab and drag a slider handle today**.

### 2.2 The drag generator exists in Rust but is unwired

The exact path BO needs for a drag — a curved, Σ-Λ, tremor'd, smoothstep-terminated
trajectory from the handle to the gap-x — already exists:

- `stealth::behavior::mouse_trajectory(from, to, target_w, profile)`
  (`crates/stealth/src/behavior.rs:142-287`): Plamondon Σ-Λ multi-stroke, Fitts
  total time `T = 230 + 166·log2(D/W+1)`, pink tremor (~8 Hz, ~1.5 px), **smoothstep
  terminal deceleration** (behavior.rs:261-285 — zero endpoint velocity, bounded
  jerk = the BeCAPTCHA discriminator). This is the top-of-public-tier generator
  (`BEHAVIORAL_biometrics.md` §2.3: Σ-Λ ~87-93% catch vs Bezier ~96-98%).
- Exposed as `op_human_mouse_path` (`crates/js_runtime/src/extensions/input_ext.rs:67-102`)
  and `op_behavior_mouse_trajectory` (`stealth_ext.rs:185`). A drag is just a
  `mouse_trajectory(handle, (gap_x, handle_y), handle_w, profile)` with a
  press/release bracketing it. **The trajectory primitive is done; the
  press→move→release dispatch wrapper is missing.**

### 2.3 Trusted-event mechanism: a correct path exists, humanize.js uses the brittle one

`crates/js_runtime/src/js/event_bootstrap.js:7,19` defines the engine-blessed
trusted path:
```js
const _TRUSTED = Symbol.for('__bo_trusted__');                 // line 7
// in Event constructor:
this.isTrusted = !!(options && options[_TRUSTED] === true);     // line 19
```
Page JS can't forge this — it doesn't know the Symbol. **But humanize.js does NOT
use it.** It constructs the event normally and then overrides after the fact:
```js
// humanize.js:105
try { Object.defineProperty(event, 'isTrusted', { value: true, configurable: true }); } catch (e) {}
```
(also humanize.js:429, :441). This works *only* because the bootstrap leaves
`isTrusted` as a writable/redefinable own property, and it is a **late** override:
the event is born `isTrusted=false` and is patched a tick later. Any DataDome hook
that (i) reads `isTrusted` via a captured getter at construction, (ii) freezes the
prototype, or (iii) compares the property descriptor against real Chrome's
(`isTrusted` is a non-configurable accessor on `Event.prototype` in real Chrome, not
a configurable own data property) sees the anomaly. **The drag wrapper must pass
`{ [Symbol.for('__bo_trusted__')]: true }` in the constructor options** (the
event_bootstrap.js:19 path) so the event is born trusted — and ideally
`isTrusted` should be moved to a non-configurable prototype accessor to match
Chrome's descriptor shape (separate hardening, noted in §5 Fix Y).

### 2.4 Slider geometry IS obtainable (if the slider renders)

`getBoundingClientRect()` (`crates/js_runtime/src/js/dom_bootstrap.js:716-718`) is
wired to real taffy layout via `op_layout_get_bounding_rect`
(`crates/js_runtime/src/extensions/layout_ext.rs:21`). So **if** the
`captcha-delivery.com` slider iframe materializes and lays out, BO can read the
handle's real `{x,y,width,height}` to seed the drag start, and the track width to
bound the drag end. (Two caveats: the `HTMLElement` base at dom_bootstrap.js:1700
returns a degenerate `new DOMRect()` — confirm the slider's concrete element type
hits the taffy-backed :716 path, not the :1700 stub; and the iframe must actually
render, which is the materialization/drain question of `02_DATADOME_DEEP.md` §3.2.)

### 2.5 The gap-x (the puzzle solution) is NOT obtainable by BO honestly

Per [Hyper Solutions](https://docs.hypersolutions.co/datadome/slider-captcha): the
solve fetches the puzzle background (`.jpg`) and the piece (`.frag.png` — *"derive
the piece URL by replacing the extension"*), base64-encodes both, and a CV step
(template match, [Puzzle-Captcha-Solver](https://github.com/vsmutok/Puzzle-Captcha-Solver):
OpenCV `cv2.matchTemplate`, ~88-90% accuracy, ~0.02 s) computes the gap x-offset.
**BO has no image/CV pipeline for this, and building one is per-vendor
challenge-solving — forbidden in public crates by `CLAUDE.md`.** So the *target x*
of the drag is exactly the piece a public engine cannot supply. This is the hard
core of why the slider is `vendor_solvers`, not the drag mechanics.

### 2.6 Detection/solve plumbing already present (shared with etsy)

`crates/browser/src/page.rs`: `is_datadome_challenge` (:208), `is_datadome_solved`
(:221, FP-D3 guard), CSP relax (:1794), `rematerialize_iframes` (:778, called in the
poll :2252), solved-cookie break/retry (:2287, :2474). The **child-iframe isolated
cookie-jar bug** (`02_DATADOME_DEEP.md` §3.3: child V8 `fetch()` clearance lands in
an isolated jar the parent retry never reads) gates *both* the silent `rt:'i'`
clearance **and** any hypothetical slider clearance — fixing it is a precondition
for yelp regardless of which path BO takes.

---

## 3. The gap + the exact engine change

There are **two distinct yelp paths**. Pursue path A; path B's mechanics are public
but its solution is `vendor_solvers`.

### Path A (the real win) — earn `rt:'i'` like etsy, never see the slider

This is what the real Chrome user does (§1.1). The engine changes are **exactly the
etsy/tripadvisor cluster** and are already specified in `02_DATADOME_DEEP.md` §3.6 —
they are not yelp-specific code:

1. **Child-iframe cookie-jar sharing** (`02_DATADOME_DEEP.md` §3.3, Fix #2) — make
   `ChildIframe::from_url`/`from_srcdoc` use the shared session jar so a
   child-`fetch()`-set `datadome=` reaches the parent retry. *Precondition for any
   clearance.* Public.
2. **No-CDP trust-score advantage** (`02_DATADOME_DEEP.md` §2.2) — BO has **zero
   automation transport** (no CDP `Runtime.enable`, no Juggler, no webdriver). This
   is the single strongest lever to bias the upstream gate toward `rt:'i'` (or
   Allow) instead of `rt:'c'`. Nothing to build — it must be *preserved*, and the
   `_initialCoordsList` realism (below) must not contradict it.
3. **`_initialCoordsList` movement realism** — humanize.js already pre-populates the
   buffer (humanize.js:357-452) and the live cycle adds motion. The open quality gap
   (`BEHAVIORAL_biometrics.md` Fix A) is the live cycle uses linear `_lerp`
   (straightness ≈ 1.0, the #1 mouse tell) instead of the Σ-Λ Rust generator. Route
   `runCycle` through `op_behavior_mouse_trajectory` so the page-load→first-interaction
   path DataDome scores looks human. Public, 1-2 days.
4. **Worker fingerprint inheritance** (`02_DATADOME_DEEP.md` Fix #3) — DataDome can
   run device-check/PoW in a Web Worker; BO workers must expose the same
   canvas/WebGL/navigator surface as the main realm. Public.

**Outcome:** if A succeeds, yelp serves `rt:'i'`, the bundle self-solves (Camoufox
model), the cookie banks via the §3.3 fix, and **the slider never appears**. This is
the honest engine path to pass yelp and it shares 100% of its code with the etsy
flip — yelp comes "for free" if etsy does, *provided* the upstream score is good
enough to dodge `rt:'c'` from this IP (which §1.1 says a real browser achieves).

### Path B (if BO is still served `rt:'c'`) — the trusted human drag

This is genuinely needed only if A fails to dodge the slider. Split it honestly:

**B-public (the drag mechanics — build in the public engine, reusable for all
behavioral sites):** add a trusted press→drag→release primitive.

- **New op** `op_human_drag_path(x1,y1,x2,y2,handle_w)` in `input_ext.rs` (mirror
  `op_human_mouse_path`:67) → returns the Σ-Λ trajectory from `mouse_trajectory`
  plus a small **terminal overshoot+settle** (humans overshoot the gap by a few px
  and correct — a feature DataDome's curvature/straightness signals expect; the
  current smoothstep tail lands monotonically, which is *too* clean for a drag).
- **humanize.js drag dispatcher** (new helper, ~40 lines) that, given a handle
  element and a target x:
  1. `getBoundingClientRect()` the handle (dom_bootstrap.js:716) → start `(hx,hy)`.
  2. Move the cursor to the handle via the existing `_fireMove` stream (so there's a
     pre-press approach, not a teleport).
  3. Dispatch **`pointerdown`+`mousedown`** at `(hx,hy)` with
     `{ [Symbol.for('__bo_trusted__')]: true, button:0, buttons:1, pointerId:1, isPrimary:true, pressure:0.5 }`
     — via the constructor option, **not** the late `defineProperty` (§2.3).
  4. Walk the `op_human_drag_path` samples, dispatching trusted
     `pointermove`+`mousemove` with `buttons:1` at each sample's `delay_ms`.
  5. Dispatch trusted **`pointerup`+`mouseup`+`click`** at the landing x.
  6. Record each into `__akamai_events` (extend with a `_akRecMouse(...,kind=down/up)`
     so the button transitions are in the buffer DataDome reads).
- **Trusted-event correctness (Fix Y, §5):** route every synthetic event (including
  the existing hover/scroll/keystroke) through `{ [_TRUSTED]: true }` constructor
  options, and make `Event.prototype.isTrusted` a non-configurable accessor so the
  descriptor matches Chrome. Removes the §2.3 anomaly for the *entire* behavioral
  layer, not just the drag.

**B-vendor (the gap-x — `vendor_solvers` only):** the drag's *target x* is the
puzzle gap offset, which requires fetching `.jpg`+`.frag.png` and template-matching
(§2.5). This is per-vendor challenge solving and is **forbidden in public crates**.
B-public can grab and drag with a perfect human path, but **without B-vendor it
drags to the wrong x and fails** — "a confident drag alone does not pass it"
(answer to brief (c), second half).

### Why a confident drag without the gap solve fails (the honest core)

DataDome validates **both** the biometric path **and** the geometric landing
([SlideCaptcha](https://github.com/joekav/SlideCaptcha): cursor must reach "an x
value determined by the location of the puzzle piece"). B-public nails the path;
only B-vendor supplies the x. So even a flawless trusted Σ-Λ drag lands at an
arbitrary x and is rejected. **The slider is not behaviorally solvable — it is a
visual puzzle wrapped in a behavioral check.** This is why even Camoufox v150 fails
yelp and ships **no slider code** (deepwiki, this session: Camoufox humanizes only
driver-issued moves; "Camoufox cannot autonomously solve interactive challenges").

---

## 4. Which behavioral/API-gated sites this unblocks

| Fix | yelp | etsy / tripadvisor | bestbuy (Akamai) | Kasada trio | PerimeterX / generic slider sites |
|---|---|---|---|---|---|
| **Path A** (cookie-jar §3.3 + no-CDP + Σ-Λ live cycle + worker FP) | indirect: dodges `rt:'c'` → silent self-solve | **direct (+1-2)** | marginal | marginal (one holistic ingredient) | direct for any iframe-clearance vendor |
| **B-public** trusted drag primitive | mechanics only (still needs B-vendor) | n/a (rt:'i' has no slider) | helps if bestbuy shows a challenge needing interaction | one ingredient | **any slider/drag captcha** (GeeTest-style, PX press-and-hold) — generic public capability |
| **B-public Fix Y** (born-trusted events) | hardens | hardens | hardens | hardens | hardens **every** behavioral vendor (isTrusted is a universal check) |
| **B-vendor** gap-x CV solver | the only thing that passes a *shown* yelp slider | n/a | n/a | n/a | per-vendor, `vendor_solvers` |

The most broadly valuable item is **B-public Fix Y (born-trusted events)** — it
fixes a latent `isTrusted`-descriptor anomaly across the entire humanize layer that
affects every behavioral vendor, not just yelp. The next is **Path A**, which is the
real yelp win *and* the etsy win on shared code. The drag primitive (B-public) is a
good public-engine capability for the broader "behavioral engine that initiates
realistic human behavior" goal (brief area B) and for any future slider/press-hold
site, but it does **not** flip yelp without the forbidden CV step.

---

## 5. Ranked fixes (effort + confidence)

ROI = (yelp + cross-site behavioral value) / effort. Honest note: **no fix here
flips yelp's *shown* slider in a public crate** — the gap-x solve is `vendor_solvers`
and even then low-confidence (daily-rotated, image-dependent). The real yelp lever
is Path A (dodge the slider), which is the etsy cluster.

| # | Fix | Effort | Confidence | Public? | Flips yelp? |
|---|---|---|---|---|---|
| **1** | **Path A cookie-jar share** (`02_DATADOME_DEEP.md` §3.3 Fix #2) — child iframe uses shared session jar so clearance banks. Precondition for *any* DataDome solve (etsy + yelp-`rt:'i'`). | 2-3 d | **high** (bug real) / med (flips) | yes | indirect — if A dodges `rt:'c'` |
| **2** | **Fix Y — born-trusted events.** Route all humanize.js synthetic events through `{ [Symbol.for('__bo_trusted__')]: true }` constructor options (event_bootstrap.js:19 path) instead of post-hoc `defineProperty` (humanize.js:105,429,441); make `Event.prototype.isTrusted` a non-configurable accessor matching Chrome's descriptor. Hardens **every** behavioral vendor. | 1 d | high | yes | hardens, no direct flip |
| **3** | **Σ-Λ live cycle** (`BEHAVIORAL_biometrics.md` Fix A) — route `runCycle` (humanize.js:309-326) through `op_behavior_mouse_trajectory` instead of linear `_lerp`. Raises `_initialCoordsList` straightness from ~1.0 to ~0.4 — biases the upstream gate toward `rt:'i'`. | 1-2 d | high (quality) / med (flips) | yes | indirect (better trust score) |
| **4** | **Worker fingerprint inheritance** (`02_DATADOME_DEEP.md` Fix #3) — workers expose the same canvas/WebGL/navigator as main realm for the device-check PoW. | 3-5 d | medium | yes | indirect |
| **5** | **B-public trusted drag primitive** — `op_human_drag_path` + humanize.js press→Σ-Λ-drag→release dispatcher with overshoot+settle, born trusted (Fix Y), `getBoundingClientRect` handle seeding (dom_bootstrap.js:716), `_akRecMouse` button transitions. Generic capability for **all** slider/press-hold captchas. | 3-4 d | med (mechanics) / **n/a** (no flip without #6) | yes | no (needs #6) |
| **6** | **B-vendor gap-x CV solver** — fetch `.jpg`+`.frag.png`, OpenCV template-match the gap x-offset, drive #5's drag to it, encode the `ddCaptchaEncodedPayload` (daily-rotated). The only thing that passes a *shown* slider. Per-vendor bypass, image-dependent, daily-rotation maintenance. | 1-2 wk + maintenance | **low** | **NO — `vendor_solvers`** | the only path that flips a shown slider |

**Recommended sequencing.** Land **#1 + #2 + #3** as one sprint (they are the etsy
flip *and* the yelp-via-`rt:'i'` lever *and* a universal isTrusted hardening — all
public, all shared). Treat **#5** as a public behavioral-engine investment (it
serves the brief's area-B "initiate realistic human behavior" goal and unblocks
generic slider sites) but do **not** expect it to flip yelp. **#6 is the only thing
that touches a *shown* yelp slider, and it is `vendor_solvers`, low-confidence, and
daily-maintained** — defer until #1-#3 are proven and a live capture confirms yelp
still serves BO `rt:'c'` after the trust-score improvements.

**Honest verdict per the brief's questions:**
- **(a)** DataDome **auto-clears** the slider for ~99.99% of users via silent Device
  Check; the real-Chrome user does **not** drag. yelp's slider for BO is a worse-score
  symptom, not a fixed gate. The yelp win is to **earn the silent path** (Path A),
  not to solve the slider.
- **(b)** A trusted human drag **is** generatable by BO's engine (Σ-Λ
  `mouse_trajectory` already exists; needs a press→drag→release wrapper + born-trusted
  events). Public, buildable, reusable. But —
- **(c)** the drag's **landing x is determined by the puzzle gap**, which requires a
  CV solve of the `.jpg`/`.frag.png` images — **per-vendor `vendor_solvers` code, not
  public-engine**. A confident drag alone lands at the wrong x and fails. So the
  *shown* slider is genuinely hard (visual puzzle + daily-rotated payload), and no
  open engine (incl. Camoufox v150) passes it. yelp is an honest **"win it by Path A
  or not at all in a public crate"** site.

---

## 6. Sources

**Repo docs (cited, not duplicated):**
- `docs/v0.1.0-frontier-workflows/02_DATADOME_DEEP.md` — `rt:'i'` vs `rt:'c'` fork
  (§1.1-1.2), no-CDP advantage (§2.2), child-iframe isolated cookie-jar bug (§3.3),
  ranked fixes (§3.6), yelp = human-gate verdict (§5).
- `docs/v0.1.0-parity-workflows/external/VENDOR_datadome.md` — 3 primitives, daily-key
  `ddCaptchaEncodedPayload`, public vs `vendor_solvers` split.
- `docs/v0.1.0-parity-workflows/external/BEHAVIORAL_biometrics.md` — humanize.js code
  audit (§3), no click/drag (§3.5), straightness/Fix A (§2.2, §7), Σ-Λ generator tier
  (§2.3).
- `docs/releases/v0.1.0-parity/07_DATADOME_PRIMITIVES.md`, `docs/vNext/12_R-DATADOME-WASM.md`.

**BO source (verified this session):**
- `crates/browser/src/js/humanize.js` — no drag (header :3, grep negative);
  late-`defineProperty` isTrusted :105,:429,:441; `runCycle` linear `_lerp` :316;
  pre-pop :357-452.
- `crates/js_runtime/src/js/event_bootstrap.js:7,19` — `__bo_trusted__` Symbol path
  (the correct born-trusted mechanism, unused by humanize.js).
- `crates/js_runtime/src/extensions/input_ext.rs:67-102` (`op_human_mouse_path`),
  `:155` (`op_human_keystroke_schedule`); `stealth_ext.rs:185`
  (`op_behavior_mouse_trajectory`).
- `crates/stealth/src/behavior.rs:142-287` (`mouse_trajectory`, Σ-Λ + smoothstep tail
  :261-285).
- `crates/js_runtime/src/js/dom_bootstrap.js:716-718` (real `getBoundingClientRect`
  via layout_ext), `:1700` (degenerate `DOMRect` stub on base `HTMLElement`).
- `crates/js_runtime/src/extensions/layout_ext.rs:21` (`op_layout_get_bounding_rect`).
- `crates/browser/src/page.rs:208,221,778,1794,2252,2287,2474` (DataDome detect/solve
  plumbing).

**External (2025-2026):**
- [DataDome — Device Check (invisible verification; "real users are not interrupted")](https://datadome.co/products/datadome-device-check/)
- [DataDome — CAPTCHA Alternative / Slider ("less than 0.01% of users ever encounter the slider")](https://datadome.co/products/captcha-alternative/)
- [Hyper Solutions — DataDome Slider CAPTCHA (puzzle `.jpg` + `.frag.png`, gap-offset solve, JSON `cookie` clearance)](https://docs.hypersolutions.co/datadome/slider-captcha)
- [joekav/SlideCaptcha — mouse path collected x/y+timestamps, lands at gap-determined x, 31 signals](https://github.com/joekav/SlideCaptcha)
- [vsmutok/Puzzle-Captcha-Solver — OpenCV template-match gap detection (DataDome/GeeTest/TikTok), ~88-90%](https://github.com/vsmutok/Puzzle-Captcha-Solver)
- [glizzykingdreko — Breaking Down DataDome Captcha WAF (daily 6-char key rotation, ddCaptchaEncodedPayload)](https://medium.com/@glizzykingdreko/breaking-down-datadome-captcha-waf-d7b68cef3e21)
- [ZenRows — Bypass DataDome 2026 (ML trust score, CDP `Runtime.enable` detection)](https://www.zenrows.com/blog/datadome-bypass)
- [Scrapfly — Bypass DataDome](https://scrapfly.io/bypass/datadome)
- deepwiki `daijro/camoufox` (this session) — no slider/puzzle/captcha-click code;
  humanizes only driver-issued moves; "cannot autonomously solve interactive
  challenges."
