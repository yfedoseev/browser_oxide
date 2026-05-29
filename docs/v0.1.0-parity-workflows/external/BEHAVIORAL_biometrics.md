# BEHAVIORAL biometrics — mouse / keystroke / scroll / touch / pointer / focus

**Scope.** Everything a vendor scores about *how* an input event arrived
rather than *what* it looked like: mouse trajectory kinematics, keystroke
dwell/flight, scroll cadence, touch parameters, pointer event pairing,
focus/blur sequencing. This is the *motion* axis; the *static-identity*
axis (TLS, canvas, audio, UA-CH) is covered by `16_STEALTH_FINGERPRINT_AUDIT.md`.

**Verdict up front.** BO's behavioral layer is in **better shape than the
two existing repo docs (`40_TIMING_BEHAVIORAL.md`, `42_HOLISTIC_VISION.md`)
claim** — two of the three "engineering-exists-wiring-does-not" gaps those
docs flag (keystroke generator, two-level seeded RNG) have **since been
wired** (humanize.js Fix 5 + Fix 6). The doc that the task brief and
`MEMORY.md` point at is **stale on this point**. The genuinely-open gaps
are now narrower and ranked in §7:

1. keystroke wiring is **reactive-only** (fires on page-driven `focusin`,
   never proactively focuses a field — so it produces zero events on the
   typical corpus page that doesn't auto-focus a search box);
2. **touch synthesis is entirely absent** (mobile profiles set
   `maxTouchPoints: 5` but emit zero `TouchEvent`s);
3. the **live-cycle mouse model** is a downgrade from the Rust generator
   (linear anchor-to-anchor, white-noise tremor, no smoothstep tail);
4. total **mousemove event count is ~45, not the "hundreds" real users
   produce** (Castle: 378 human vs 4 bot);
5. **scroll shape** matches neither real wheel nor trackpad distribution.

Crucially, **none of these block any of the 19 currently-failing corpus
sites** — the AWS-WAF cluster, booking, douyin, x-com, etc. are gated on
async-drain / SPA-hydration / token issues (see HANDOFF_2026_05_28b §4),
not behavioral. Behavioral is the **v0.2.0 / post-2026 frontier** (`42` §7),
and the highest-leverage work is *correctness hardening* of what already
ships, plus touch for any future mobile-app-protected site.

---

## 1. What the existing repo docs already concluded (and where they are now stale)

### 1.1 `40_TIMING_BEHAVIORAL.md` — the canonical behavioral chapter

This 1174-line doc is the authoritative prior work. Its conclusions, and
their **current** truth status verified against source this session:

| `40` claim | Where | Still true? |
|---|---|---|
| Mouse Σ-Λ multi-stroke generator shipped in `behavior.rs:142-287` (Plamondon, Fitts total time, pink tremor, smoothstep terminal decel) | §3.1 | **TRUE.** Verified `crates/stealth/src/behavior.rs` (873 lines). Exposed as `op_behavior_mouse_trajectory` (`stealth_ext.rs:185`) + `op_human_mouse_path` (`input_ext.rs:67`). |
| `performance.now()` humanized: LogNormal(ln 8µs, 0.4) on 100µs grid + Bernoulli(1/1024) heavy-tail spike, monotonic clamp | §2.1, §2.6 | **TRUE.** `perf_ext.rs`. Defeats `set(diffs).size===1` probe. |
| **Keystroke generator (`behavior.rs:421-464`) exists but is NOT wired into humanize.js; `_akRecKey` defined but never called** | §3.2, §4.2 | **STALE — NOW WIRED.** See §3.2 below. humanize.js:110-154 calls `op_human_keystroke_schedule` on `focusin`; `_akRecKey` *is* called (humanize.js:139, 148). |
| **Two-level seed (`behavior.rs:109-115`) not consumed by humanize.js — uses `Math.random()`** | §5, §4.2 | **STALE — NOW WIRED.** `op_behavior_random` (`input_ext.rs:54`) backs a `Symbol.for('__browser_oxide_behavior_rand__')` slot; humanize.js:49 reads it into `_rand` and uses `_rand()` everywhere instead of `Math.random()`. |
| Touch events: zero, no `_akRecTouch`, mobile profiles tell on `touchEventCount===0` | §3.4 | **TRUE — still absent.** Verified no `new Touch`/`TouchEvent(` in humanize.js or any bootstrap except illegal-ctor stubs in window_bootstrap.js. No touch generator in behavior.rs. |
| Scroll shape matches neither wheel (modal deltaY=100) nor trackpad (30-60 momentum events) | §3.3 | **TRUE.** humanize.js:330 still emits 2 uniform-random steps. `wheel_burst` (`behavior.rs:482`) exists but **no op exposes it** — verified. |
| RAF is constant 16 ms (no jitter); setTimeout has no nesting-level clamp; hidden-tab throttle absent | §2.3, §2.4 | **TRUE** (timer_bootstrap.js). Kasada-cluster-only relevance. |
| BeCAPTCHA-Mouse: single Bezier → ~99% bot accuracy on one trajectory; Σ-Λ-with-tremor → ~93% | §1, §3.1 | **TRUE** and corroborated/extended by 2024 DMTG paper — see §2.3. |
| No 126-corpus site is currently gated on keystroke-presence or touch-presence; corpus selects for "loads a page" not "submits a form" | §3.2, §6 | **TRUE.** Confirmed against `FAILED_SITES_ANALYSIS.md` (only bestbuy filed as a *maybe*-behavioral probe, R-BESTBUY-AKAMAI). |

### 1.2 `42_HOLISTIC_VISION.md` — cross-vendor leverage view

`42` §Pattern-6 ("engineering-exists-wiring-does-not") and §3 list the
keystroke generator and two-level seed as unwired and estimate "~5 days to
wire ALL of these, lift 5-8 behavioral vendors." **Two of the three line
items it lists are now done** (`behavior.rs:421-464` keystroke and
`:109-115` seed). `42` §7 ("Behavioral biometrics is the post-2026
frontier", 8/12 vendors deploy behavioral scoring) and its strategic call
— *"BO's humanize.js is the long-term battleground… worth investing even
though the wiring gaps are short-term"* — remain the correct framing.

### 1.3 Vendor deep-dives that consume these primitives

- `26_AKAMAI_BMP_DEEP.md` §3 — sensor_data v2/v3 reads
  `__akamai_events.{mouse,key,scroll,touch}`; the in-V8 buffer humanize.js
  populates (humanize.js:69-100) is the drain surface a private
  `vendor_solvers` Akamai solver consumes.
- `07_DATADOME_PRIMITIVES.md` — `tags.js` scores a 31-feature mouse-path
  vector at POST; defeated by the **synchronous pre-population** IIFE
  (humanize.js:357-452), which drops ~14 historical Σ-Λ points into the
  buffer *before* the IIFE returns so the empty-coord-list heuristic
  never fires.
- `08_KASADA_FRONTIER.md` §3 — V3 envelope embeds mouse summary stats +
  2 timing moments + RAF cadence; the only vendor demonstrably scoring
  RAF stddev.
- `06_AWS_WAF_SOLVER.md` §3 / `VENDOR_awswaf.md` — token includes some
  signals, but per HANDOFF_2026_05_28b §4 the AWS blocker is **async
  drain, not behavioral** (see §5).

---

## 2. New external findings (2024-2026)

### 2.1 Camoufox v150's behavioral model is *weaker* than BO's — and also passive

Queried `daijro/camoufox` via deepwiki. Findings:

- Camoufox humanizes **mouse only**, via a C++ `HumanizeMouseTrajectory`
  class using a **Bezier-curve** model (`BezierCalculator::generateCurve`,
  `easeOutQuad` easing), a C++ port of **riflosnake's HumanCursor**.
  Exposed to the driver as `ChromeUtils::CamouGetMouseTrajectory`; timing
  bounded by `humanize:maxTime` / `humanize:minTime` in `MaskConfig`.
- Keystroke, scroll, touch are **NOT humanized in-engine** — Camoufox
  dispatches them verbatim through the Playwright Juggler protocol
  (`PageHandler.js` → `dispatchKeyEvent` / `dispatchTouchEvent` /
  `dispatchTapEvent`). No dwell/flight model, no scroll-momentum model.
- **Critically: Camoufox's humanization is on-demand** — it only fires when
  the *automation driver* issues a `mouse.move(x,y)`. On a bare
  `page.goto()` with no scripted actions, Camoufox emits **zero**
  synthetic behavioral events, exactly like a passive scraper.

**Strategic implication for BO.** BO's Σ-Λ Plamondon model
(`behavior.rs`) is **more sophisticated** than Camoufox's Bezier port (see
§2.3 — Bezier is caught at ~96-98%, Σ-Λ at ~87-93%). And BO *autonomously*
emits a mouse/scroll/keystroke pattern on every `navigate` (humanize.js:455
`runCycle()` + `setInterval`), whereas Camoufox emits behavioral signal
**only under driver control**. So on the same-IP passive head-to-head
(`run_delta_headtohead.py`), **behavioral signal is a column where BO
already meets-or-beats v150** — neither emits driver-less keystrokes/touch,
and BO's autonomous mouse is the better model. This is consistent with the
trustworthy baseline showing the BO-vs-v150 gap living in
hydration/token/async sites, not behavioral ones.

### 2.2 What 2025-2026 vendors actually score

From GeeTest, Bureau, Castle, Scrapfly, and the academic set:

- **Mouse efficiency / straightness** (`Euclidean(start,end) / pathLength`).
  Bureau + ScrapingAnt: humans peak at **0.3-0.4** (lots of detours);
  ~59% of *bot* movements exceed 0.94 efficiency vs only ~29% of human
  movements. A perfectly-straight lerp between anchors is the #1 tell.
  *BO relevance:* the live cycle (humanize.js:309-326) interpolates
  **linearly** between 3 anchors — high straightness within each segment.
  The Rust generator (`behavior.rs`) produces curved multi-stroke paths;
  the live cycle does not use it. Gap #3.
- **mousemove event count.** Castle's "Bot or Not" teardown: a human
  interaction emits events **"in the hundreds"** (their example: 378);
  the flagged bot emitted **4**. Instant coordinate jumps + sparse event
  streams are flagged directly. *BO relevance:* a full BO navigate emits
  ~30 live mousemove (2 strokes × 15 samples) + ~14 buffer points + 1
  synchronous pair ≈ **45 events**, then +30 every 4 s. Better than 4, but
  below "hundreds" until the page lives long enough for several cycles.
  Gap #4.
- **Acceleration / jerk profile.** Castle: bots show **near-zero
  acceleration** (constant speed) except at direction changes; "human
  acceleration is almost never close to zero." DMTG (below) adds that
  humans show **push-vs-pull acceleration asymmetry**. *BO relevance:* the
  Rust generator's Σ-Λ velocity is correctly asymmetric and the smoothstep
  tail gives bounded endpoint jerk; the **live cycle** has an impulse
  spike at each anchor terminus (last sample is `_lerp(a,b,1.0)`). Gap #3.
- **Behavioral biometrics now drive ML, not rules.** Springer 2026
  ("Behavioral Biometrics: A Comparison of Keystroke Dynamics and Mouse
  Trajectories for Bot Detection") and the CNN-on-mouse-as-image line
  (ACM 10.1007/978-3-030-31456-9_43) confirm the shift to LSTM/CNN scoring
  the *distribution*, not single thresholds — matching `40` §1's
  "distribution gap is open-ended" thesis.

### 2.3 The synthetic-trajectory arms race — DMTG (2024) gives concrete caught-rates

**DMTG: Human-Like Mouse Trajectory Generation via Entropy-Controlled
Diffusion Networks** (arXiv 2410.18233) is the current SOTA generator and
publishes discriminator catch-rates that let us *rank generator quality*:

| Generator | Discriminator catch-rate | BO equivalent |
|---|---|---|
| Linear / lerp | ~98% | live-cycle anchor segments (Gap #3) |
| Bezier | ~96-98% | **Camoufox v150's model** |
| GAN | ~99% (overfits one region) | — |
| Ghost-cursor | high (clusters at one end of human distribution) | — |
| Σ-Λ / sigma-lognormal + tremor | ~87-93% | **BO's `behavior.rs` generator** |
| DMTG (diffusion) | ~87-90%, distribution *encloses* human data | frontier — out of scope public crates |

Takeaways for BO:
- BO's Σ-Λ generator is **at the top of the publicly-reproducible tier**,
  tied with diffusion on catch-rate and clearly beating Bezier/linear. The
  right move is to **route the live cycle through the Rust generator** so
  BO realizes that quality everywhere (today only the pre-pop does).
- DMTG's headline insight — generators that *cluster at one end of the
  human distribution* (Ghost-cursor) are detectable even when individual
  traces look human — is the same **under-/over-dispersion** problem `40`
  §5 raises. BO's per-page seeded RNG (now wired) addresses
  page-determinism but **still draws a fresh seed per page**
  (`input_ext.rs:39` `rand::random::<u64>()`), so across pages BO looks
  like N different users (over-dispersion). The fix is to thread a
  *session* seed (§7, FIX-B).
- **Acceleration asymmetry (push vs pull)** is a feature no current public
  generator models, including BO. Low ROI for now (no corpus site scores
  it) but worth noting for the v1.0 frontier.

### 2.4 AWS WAF behavioral reality-check

Sources (AWS docs `waf-js-challenge-api.html`, Capsolver, roundproxies
2026) confirm AWS WAF *does* fold mouse/keyboard/navigation signals into
its targeted-protection token, and recommend non-headless for the CAPTCHA
tier. **But** the BO failure mode is upstream of any behavioral scoring:
per HANDOFF_2026_05_28b §4, challenge.js *proceeds* with the BO fingerprint
and calls `forceRefreshToken` in the offline oracle — the live path simply
never lets the PoW Web Worker spawn (50 ms inter-script drain vs 5 s in the
oracle, `page.rs` ~3535). So **for the AWS cluster, behavioral synthesis is
not the lever** — async drain is. Do not spend behavioral effort there.

---

## 3. BO code-level analysis (verified this session)

### 3.1 Mouse — two layers, both real, live cycle is the weaker one

- **Rust generator** `crates/stealth/src/behavior.rs:142-287`
  (`mouse_trajectory` / `_with_rng`): Σ-Λ multi-stroke, Fitts total time
  `T = 230 + 166·log2(D/W+1)`, per-stroke σ~N(0.25,0.05), μ~N(-1.6,0.2),
  inter-stroke onset~LogNormal(ln 90ms,0.3), pink tremor (low-passed
  α=0.3, ~8 Hz × 1.5 px), **smoothstep terminal decel** (15-sample
  C¹-continuous, zero velocity at arrival → bounded endpoint jerk, the
  BeCAPTCHA discriminator). High quality. Exposed via
  `op_behavior_mouse_trajectory` (`stealth_ext.rs:185-205`) and
  `op_human_mouse_path` (`input_ext.rs:67-102`).
- **JS pre-population** `crates/browser/src/js/humanize.js:357-452`
  (`_seedHistoricalCoords`): calls `op_behavior_mouse_trajectory`, projects
  the trajectory onto a [-1800,-100] ms historical window, pushes ~14
  points into `__akamai_events.mouse` **synchronously before the IIFE
  returns**, then dispatches one synchronous `mousemove`+`pointermove`
  pair. This is the load-bearing DataDome/PerimeterX defeat. **Uses the
  good Rust generator.** ✓
- **JS live cycle** `humanize.js:292-336` (`runCycle`): picks 3 random
  anchors, **linearly interpolates** 15 samples per stroke
  (`_lerp(a,b,tau)`, line 316) with Σ-Λ-distributed *sample times*
  (`_sigmaLognormalTimes`, line 313) and Gaussian white-noise jitter
  (`_gauss()*0.8`, lines 317-318). Fires `_fireMove` (mousemove +
  pointermove paired on window+document+body, isTrusted=true,
  screenY=clientY+90, recorded via `_akRecMouse`). **Does NOT use the Rust
  generator** → straight segments (high efficiency, Gap #3/#4), white not
  pink tremor, impulse endpoint jerk. This is the single biggest mouse
  quality gap and it is **public-crate-fixable**.
- `_rand` (humanize.js:49) reads
  `Symbol.for('__browser_oxide_behavior_rand__')` → `op_behavior_random`
  (`input_ext.rs:54`), per-page seeded ChaCha (StdRng), seed from
  `BROWSER_OXIDE_BEHAVIOR_SEED` env or `rand::random` per page
  (`input_ext.rs:35-41`). Two-level *page* determinism wired; *session*
  continuity not.

### 3.2 Keystroke — NOW WIRED (Fix 5), but reactive-only

`40` §3.2 says "NOT WIRED / `_akRecKey` never called." **Current source
contradicts this:**
- `op_human_keystroke_schedule` (`input_ext.rs:155-181`): wraps
  `stealth::behavior::keystroke_timings`, returns `[{key, code, down_ms,
  up_ms}]` cumulative-ms slots; `char_to_code` maps to W3C
  `KeyboardEvent.code`.
- Installed as `Symbol.for('__browser_oxide_keystroke_schedule__')` by
  `stealth_bootstrap.js:141-143`.
- humanize.js:116-154 reads it into `_ksFn` and registers a **capture-phase
  `focusin` listener**: on first focus of an `INPUT`/`TEXTAREA` (single-shot
  per element via a Symbol tag), it schedules `keydown`/`keyup` pairs at the
  generated offsets, dispatches `KeyboardEvent`s with isTrusted=true, and
  **calls `_akRecKey` (lines 139, 148)** — so the Akamai key buffer is
  populated.
- The Rust side `keystroke_timings` (`behavior.rs:428`): LogNormal dwell
  (ln 95ms, 0.30) + bigram-modulated flight (LogNormal(median, 0.55) ×
  `bigram_ratio`, `behavior.rs:381-419`: 0.7× alt-hand, 1.4× same-finger,
  2.0× same-key). Provenance in `BIGRAM_PROVENANCE.md`. Unit tests
  `behavior.rs:709-784`.

**The remaining gap is reactive-only triggering.** The listener only fires
if *the page* dispatches `focusin` on an input. Nothing in the navigate
path proactively focuses a field. Confirmed: the only programmatic focus is
`dom_bootstrap.js:998` (`HTMLElement.focus()` → dispatches `Event("focus")`,
not `focusin`, and only when the *page* calls `.focus()`). So on a typical
corpus page (no autofocus search box), **keystroke events are still zero**.
This is fine for v0.1.0 (no corpus site gates on keystroke presence) but is
the thing to know before claiming "keystrokes are done."

Also note the token is hardcoded `'hi'` (humanize.js:130) — a 2-char fixed
string. Adequate to make the buffer non-empty; not a realistic typing
sample if a vendor scores n-gram content.

### 3.3 Scroll — manual, wrong distribution, generator unwired

humanize.js:274-288 (`_fireScrollStep`): `WheelEvent('wheel', {deltaY,
deltaMode:0})` + real `scrollBy` + `scroll` events + `_akRecScroll`.
humanize.js:330 plan: **2 steps**, deltaY uniform in [80,120] and [60,90].
Matches neither real wheel (modal deltaY=100, discrete notches) nor
trackpad (30-60 small-delta momentum events). The Rust `wheel_burst`
generator (`behavior.rs:482-540`, models trackpad two-finger swipe with
exponential decay + mouse-wheel notches per `BehaviorProfile.scroll_style`)
**has no op and is never called** — verified `grep wheel_burst` finds only
the definition. Gap #5, public-crate-fixable.

### 3.4 Touch — entirely absent

- No `new Touch` / `TouchEvent(` anywhere except illegal-ctor stubs in
  `window_bootstrap.js` (interface identity only).
- No touch generator in `behavior.rs` (only a comment at :498 about a
  two-finger swipe inside `wheel_burst`).
- `__akamai_events.touch` array is declared (humanize.js:71) but no
  `_akRecTouch` function exists; the `touch` counter never increments.
- Mobile presets set the identity correctly: `max_touch_points: 5`
  (`presets.rs:875` iPhone; desktop presets = 0).

**Consequence:** BO's iPhone/Pixel profiles present `maxTouchPoints=5`,
`ontouchstart` etc., but emit **zero TouchEvents** — a clean
`maxTouchPoints>0 && touchCount===0 && UA~Mobile` tell for Akamai-mobile /
PerimeterX-mobile / F5-mobile / Castle. No *current* corpus site exercises
this (the corpus is desktop-profile-dominant), so it is a v0.2.0 item, but
it is the **only behavioral signal where BO is strictly worse than nothing**
(a mobile UA with zero touch is more suspicious than a desktop UA with
synthetic mouse).

### 3.5 Pointer / focus / blur sequencing — partial

- Pointer events: `pointermove` is correctly **paired** with every
  `mousemove` (humanize.js:249-269, 433-445) with `pointerType:'mouse'`,
  `pointerId:1`, `isPrimary:true`, `pressure:0` — matches real Chrome's
  per-physical-event emission. ✓ But **no `pointerdown`/`pointerup`/`click`
  sequence** is ever synthesized (grep confirms zero mousedown/mouseup/
  click/pointerdown in humanize.js). A vendor scoring "moved but never
  clicked anything in 30 s" sees pure hover. Minor.
- Focus/blur: `runCycle` dispatches `window` `focus` + document
  `visibilitychange` each cycle (humanize.js:294-295), but
  `document.visibilityState` is permanently `"visible"` (no real
  hidden-tab state machine, `40` §2.3) and there is no `blur`/`focus`
  alternation modeling tab-switching. Low priority.

---

## 4. Which corpus vendors require behavioral signal

Synthesizing `40` §3.5, `42` §Behavioral, and the vendor deep-dives, mapped
to the **19 currently-failing** corpus sites:

| Vendor (corpus sites) | Behavioral scored | Is it the BO blocker? |
|---|---|---|
| **AWS WAF** (amazon-ca/com/com-au/fr/in/jp, imdb, booking?) | mouse/key/nav folded into token | **No** — blocker is async PoW-worker drain (HANDOFF §4). Behavioral effort wasted here. |
| **Akamai BMP** (homedepot, bestbuy) | mouse + key (login) + scroll + touch; reads `__akamai_events` | **Maybe** for bestbuy (R-BESTBUY-AKAMAI filed as *needs interaction probe*). homedepot residual is sec-cpt self-solve, not behavioral. |
| **Kasada** (canadagoose, hyatt, realtor) | mouse summary + RAF cadence + 300+ signals | **Partially** — Kasada is a holistic ML tail (`08`); behavioral is one of many. No single behavioral lever flips it. |
| **DataDome** (etsy) | `tags.js` 31-feature mouse vector | **Already defeated** by pre-pop buffer; residual is WASM daily-key (R-DATADOME-WASM), not behavioral. |
| **SPA / custom** (douyin, x-com, duolingo, wildberries) | sig/hydration, not behavioral | **No.** |

**Net:** the only failing sites where fresh behavioral work *could* matter
are **bestbuy** (probe pending) and possibly the **Kasada trio** (as one
ingredient in a holistic improvement). Everything else is gated elsewhere.
This is why behavioral ranks below async-drain and SPA work for v0.1.0, and
why the honest expected-impact in §7 is low for v0.1.0 / strategic for
v0.2.0.

---

## 5. Realistic human-motion model (reference for the live-cycle rewrite)

The model BO should converge the live cycle onto — most of it already
exists in `behavior.rs`; the live cycle just doesn't call it.

**Mouse (per stroke, Σ-Λ — already in `behavior.rs`):**
- Decompose path into 2-7 strokes (Meyer 1988 / Plamondon 1995): primary
  stroke ~85% of distance, 1-3 corrective sub-movements ~15%.
- Per stroke, position follows the Σ-Λ velocity integral (asymmetric: fast
  rise, slow decay), NOT linear interpolation. Target straightness
  (efficiency) **~0.3-0.5**, not ~1.0 (Bureau/ScrapingAnt §2.2).
- Pink-spectrum tremor ~8 Hz, 1-2 px (low-passed white noise, NOT raw
  Gaussian).
- Smoothstep terminal deceleration → bounded endpoint jerk (no teleport to
  target).
- Total time per Fitts: `T = 230 + 166·log2(D/W+1)` ms.
- **Event volume:** aim for hundreds of `mousemove` over the session, not
  ~45 (Castle §2.2). 125 Hz (8 ms) sampling over multi-second motion gets
  there naturally if the live cycle uses the Rust generator's sample rate.

**Keystroke (already in `behavior.rs::keystroke_timings`):**
- Dwell (keydown→keyup) ~LogNormal(ln 95 ms, 0.30) — tight per-user.
- Flight (prev keyup→keydown) ~LogNormal(ln 130 ms, 0.55) at 50 WPM,
  bigram-modulated (alt-hand 0.7×, same-finger 1.4×, same-key 2.0×).
- Realistic extras (not yet modeled): 1-2% typo+backspace bursts; shift
  held through next dwell; practiced-sequence speed-ups.

**Scroll (in `behavior.rs::wheel_burst`, just needs an op):**
- Mouse-wheel: discrete events, modal deltaY=100, LogNormal inter-event Δt,
  no momentum.
- Trackpad: 30-60 small-delta (1-5 px) events at ~60 Hz with velocity
  ramp-up + exponential decay envelope.

**Touch (to build — NOT in `behavior.rs`):**
- `Touch` with `radiusX/radiusY` (10-25 px), `force` (0-1), `rotationAngle`.
- Swipe velocity follows Σ-Λ (reuse the mouse generator's curve sampler).
- Tap press-release timing; multi-touch finger correlation for pinch.

**Session variance (the `40` §5 / DMTG §2.3 point):**
- Per-session seed parameterizes `BehaviorProfile` (σ_dwell, σ_mouse,
  fitts_b, typing_wpm) — stable across all pages in a visitor session.
- Per-call salt folds with session seed for each path/string.
- `behavior.rs:109-115` `rng_for(salt)` already implements the two-level
  scheme; humanize.js currently consumes only a *page* seed, not a
  *session* one.

---

## 6. Acceptance / verification hooks that already exist

- `crates/stealth/src/behavior.rs:709-784` — keystroke unit tests (first
  has no flight, dwell range, WPM scaling, bigram th<dd, determinism/seed).
- `behavior.rs` mouse tests (Fitts total time, 8 ms sample rate, velocity
  diversity not uniform, determinism per seed).
- `crates/browser/tests/chrome_compat.rs:5857` `keystroke_schedule_slot_installed_and_monotonic`;
  `:5908` `behavior_rand_slot_installed_and_in_unit_range` — confirm both
  new wirings are live and tested.
- `perf_ext.rs:146-203` — clock distribution tests.

Missing acceptance: no test feeds the **live-cycle** mouse stream to a
classifier or checks its straightness/event-count, and no test asserts the
keystroke listener actually fires end-to-end on a focused input in a real
navigate. Add these alongside the FIX-A live-cycle rewrite.

---

## 7. Ranked fix list (ROI order)

ROI = (corpus-site impact + strategic v0.2.0 value) / effort. Recall
**none of these flip a v0.1.0 corpus site** — behavioral is not the current
blocker for the 19 failures (§4) — so v0.1.0 impact is honestly "0 confirmed
flips; correctness hardening + v0.2.0 prerequisite."

| # | Fix | Effort | Confidence | Public crate? | Expected impact |
|---|---|---|---|---|---|
| **A** | **Route the live cycle through `op_behavior_mouse_trajectory`** instead of linear `_lerp` anchors. Replace humanize.js:309-326 with calls to the Rust Σ-Λ generator (curved multi-stroke, pink tremor, smoothstep tail). Raises straightness 1.0→~0.4, fixes endpoint jerk, and (with 8 ms sampling over multi-second motion) pushes mousemove count toward "hundreds". | 1-2 days | high | **public** (`crates/browser` + reuse existing `stealth_ext` op) | 0 confirmed corpus flips; closes the biggest *quality* gap; best-in-public-tier mouse (§2.3). Possible marginal help on bestbuy/Kasada holistic score. |
| **B** | **Thread a session seed** (not per-page) into `BehaviorRngState`. Add a `BROWSER_OXIDE_SESSION_SEED` / profile-derived seed so a multi-page visitor shows consistent σ/μ (fixes over-dispersion, DMTG §2.3, `40` §5). | 0.5-1 day | high | **public** (`input_ext.rs` seed source) | 0 in single-page v0.1.0 corpus; **prerequisite** for any multi-page session corpus (v0.2.0). |
| **C** | **Proactive keystroke trigger.** Have `runCycle` (or a one-shot) focus the page's primary visible text input and let the already-wired keystroke schedule fire, so keystroke events exist even when the page doesn't autofocus. Guard against typing into the page's real listeners on benign forms (current single-shot + short token already mitigates). | 1 day | medium | **public** (humanize.js) | 0 confirmed flips (no corpus site gates on key-presence); de-risks bestbuy login-flow probe + future form corpus. |
| **D** | **Touch synthesis for mobile profiles.** Add a `touch_swipe`/`tap` generator in `behavior.rs` (reuse Σ-Λ sampler), an op, and humanize.js dispatch of `TouchEvent` + `_akRecTouch`, gated on `maxTouchPoints>0`. Removes the `mobile-UA && zero-touch` tell. | 4-5 days | medium | **public** (`stealth`, `input_ext`, humanize.js) | 0 in current desktop-dominant corpus; the only signal where BO is *strictly worse than nothing* on mobile profiles; needed for any mobile-app-protected target. |
| **E** | **Wire `wheel_burst` for realistic scroll.** Add an op for `behavior.rs::wheel_burst`, drive `_fireScrollStep` from it so deltaY/cadence match real wheel-or-trackpad per `BehaviorProfile.scroll_style`. | 1 day | medium | **public** | 0 confirmed flips; closes the scroll-distribution gap (`40` §3.3). |
| **F** | **Add `pointerdown`/`pointerup`/`click` on a benign target + minimal blur/focus alternation.** Makes "hovered but never clicked / never blurred in 30 s" less anomalous. | 0.5 day | low | **public** | 0 confirmed flips; marginal holistic-score hardening. |
| **G** | **RAF jitter (±1-3 ms) + setTimeout nesting-level clamp.** Timing, not behavioral-input, but bundled here for completeness; Kasada-cluster-only relevance. Already tracked in `08_KASADA_FRONTIER.md` §4 — do not double-count. | 0.5 day | low | **public** | 0-1 (Kasada trio, as one ingredient). |
| **—** | Acceleration push/pull asymmetry; diffusion-grade (DMTG) generator; BeCAPTCHA-<50%. | weeks | n/a | frontier (likely `vendor_solvers` if site-specific) | out of scope for v0.1.0/v0.2.0; v1.0 research domain (`40` §7). |

**Per-vendor-bypass note (CLAUDE.md).** None of A-G are per-vendor bypass
code — they are generic input-humanization improvements to the public
engine, so they belong in the **public crates** (`crates/stealth`,
`crates/js_runtime`, `crates/browser`), NOT `vendor_solvers`. Only
site-specific challenge-solving (e.g. a Kasada `/tl` envelope assembler or
the AWS PoW-worker driver) belongs in `vendor_solvers`.

**Recommended sequence:** A → B → C (one focused sprint, ~3-4 days, all the
mouse/keystroke quality wins) before touching D/E. Do **not** prioritize any
of these over the AWS async-drain (HANDOFF §5.1 task #5) or the SPA
hydration work — those are where the actual failing sites live.

---

## 8. Sources

**Repo prior work:** `docs/releases/v0.1.0-parity/40_TIMING_BEHAVIORAL.md`
(§1-9), `42_HOLISTIC_VISION.md` (§Pattern-6/7, §3, §7), `26_AKAMAI_BMP_DEEP.md`
§3, `07_DATADOME_PRIMITIVES.md`, `08_KASADA_FRONTIER.md` §3-4,
`06_AWS_WAF_SOLVER.md` §3, `FAILED_SITES_ANALYSIS.md` (R-BESTBUY-AKAMAI),
`docs/HANDOFF_2026_05_28b.md` §4-5.1.

**BO source (verified this session):**
`crates/stealth/src/behavior.rs` (mouse_trajectory :142-287, rng_for
:109-115, bigram_ratio :381-419, keystroke_timings :421-464, wheel_burst
:482-540, tests :709-784); `crates/js_runtime/src/extensions/input_ext.rs`
(op_behavior_random :54, op_human_mouse_path :67, op_human_keystroke_schedule
:155, ext :190); `crates/js_runtime/src/extensions/stealth_ext.rs`
(op_behavior_mouse_trajectory :185); `crates/browser/src/js/humanize.js`
(_rand :49, keystroke focusin :116-154, _fireMove :pointer-pairing
:249-269, _fireScrollStep :274-288, runCycle :292-336, pre-pop :357-452);
`crates/js_runtime/src/js/stealth_bootstrap.js:114-143`;
`crates/stealth/src/presets.rs:875` (maxTouchPoints).

**External (2024-2026):**
- DMTG diffusion trajectory generator + discriminator catch-rates —
  https://arxiv.org/html/2410.18233v1
- BeCAPTCHA-Mouse — https://arxiv.org/pdf/2005.00890 ;
  https://github.com/BiDAlab/BeCAPTCHA-Mouse
- Castle "Bot or Not" (378 vs 4 mousemove; zero-acceleration tell) —
  https://blog.castle.io/bot-or-not-can-you-spot-the-automated-mouse-movements/
- Bureau (mouse straightness/efficiency, 0.94 bot vs 0.3-0.4 human) —
  https://bureau.id/resources/blog/mouse-movement-behavioral-patterns-can-reliably-tell-bots-from-humans
- ScrapingAnt (efficiency = Euclidean/pathLength) —
  https://scrapingant.com/blog/detect-bot-by-cursor
- GeeTest behavioral biometrics overview —
  https://www.geetest.com/en/article/behavioral-biometrics-bot-detection
- Springer 2026 keystroke-vs-mouse comparison —
  https://link.springer.com/chapter/10.1007/978-3-032-16038-6_17
- CNN mouse-as-image bot detection —
  https://dl.acm.org/doi/10.1007/978-3-030-31456-9_43
- AWS WAF intelligent-threat JS API —
  https://docs.aws.amazon.com/waf/latest/developerguide/waf-js-challenge-api.html
- AWS WAF bypass landscape 2026 — https://roundproxies.com/blog/bypass-aws-waf/
- Camoufox behavioral model (deepwiki query, daijro/camoufox:
  HumanizeMouseTrajectory C++ / BezierCalculator / riflosnake HumanCursor;
  Juggler PageHandler.js dispatchKeyEvent/dispatchTouchEvent)
- CMU keystroke benchmark — https://www.cs.cmu.edu/~keystroke/ ;
  Buffalo CUBS — https://www.buffalo.edu/cubs/research/datasets.html
