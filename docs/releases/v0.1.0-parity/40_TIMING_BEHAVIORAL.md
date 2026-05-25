# 40 — Timing fingerprinting + behavioral biometrics

**Status:** cross-cutting reference (NOT a per-site plan).
**Scope:** every primitive a vendor uses to score *how* an event arrived
rather than *what* the event looked like — clock resolution, jitter
shape, timer cadence, mouse curve kinematics, scroll velocity, keystroke
rhythm, touch pressure.
**Audience:** anyone touching `humanize.js`, `perf_ext.rs`,
`timer_bootstrap.js`, or `crates/stealth/src/behavior.rs`; anyone
debugging a "we passed the static fingerprint but the site still flagged
us" sweep verdict.
**Companions:**
- `16_STEALTH_FINGERPRINT_AUDIT.md` — sibling on **static** identity
  (TLS, canvas, audio, UA-CH). Read together: 16 = identity, this = motion.
- `17_WEB_API_PARITY_MATRIX.md §2.8` — performance.now / RAF / RIC
  surface parity. The matrix says *whether* we expose; this chapter
  says *with what timing shape*.
- `18_ANTI_BOT_VENDOR_COOKBOOK.md` — per-vendor cookie/header markers;
  this chapter is the orthogonal axis (per-vendor behavioral checks).
- `06_AWS_WAF_SOLVER.md`, `07_DATADOME_PRIMITIVES.md`,
  `08_KASADA_FRONTIER.md`, `25_CLOUDFLARE_DEEP.md`,
  `26_AKAMAI_BMP_DEEP.md` — vendor deep dives that consume the
  primitives below.
- `13_FILE_LOCATIONS_INDEX.md` — every file:line cited here.

---

## TL;DR

Two engine surfaces decide every behavioral verdict:

1. **The clock** (`crates/js_runtime/src/extensions/perf_ext.rs:60-75`) —
   `performance.now()` returns a 100 µs grid with a LogNormal(8 µs, 0.4)
   jitter overlay and a Bernoulli(1/1024) heavy-tail spike. This shape
   matches captured Chrome 130 distributions and defeats the trivial
   "set(diffs).size === 1" hot-loop probe. **Status: shipped.**
   Two known gaps remain: (a) the main-window fallback at
   `crates/js_runtime/src/js/timer_bootstrap.js:170-173` still uses
   `Date.now() - startTime` if `Performance.prototype.now` is not
   installed (defensive only — the prototype install at
   `window_bootstrap.js:5430-5434` runs in every real page); (b)
   `performance.timeOrigin` is not surfaced through the humanized op
   and worker scope reuses the op verbatim (`worker_bootstrap.js:131-138`).
2. **The motion** (`crates/browser/src/js/humanize.js` +
   `crates/stealth/src/behavior.rs`) — synthetic mouse/scroll/key
   events are generated with a Plamondon Σ-Λ velocity model
   (`behavior.rs:142-287`), 2-7 sub-strokes per movement, ~8 ms (125
   Hz) sample rate, pink-tremor noise, smoothstep terminal deceleration
   (no impulse spike at endpoint). Keystroke synthesis is implemented
   in `behavior.rs:421-464` (LogNormal dwell + bigram-modulated flight)
   but is **not wired** into `humanize.js`. **Status: mouse curves
   shipped; keystrokes shipped-but-unwired; touch unimplemented.**

The chapter is organized first by **technique** (every primitive a
vendor can score), then by **vendor** (who scores what), then by **BO
coverage** (per-primitive status). Section 5 frames the fundamental
limit — "perfect humanity" is itself a tell — and section 8 gives the
v0.1.0 acceptance bar.

---

## 1. Why timing + behavioral is hard to fix

A static fingerprint is a single static value. `navigator.userAgent` is
a string; `WebGLRenderingContext.getParameter(37446)` is a string;
the ALPS-extension order in a TLS ClientHello is a byte sequence.
Once captured from real Chrome, the value can be reproduced
byte-for-byte and matched byte-for-byte. The diff is decisive: equal or
not equal. `16_STEALTH_FINGERPRINT_AUDIT.md` treats this domain.

A behavioral fingerprint is **a distribution**. `performance.now()` in
a 1000-iteration hot loop returns 1000 deltas; the vendor wants to
know whether those deltas form a Chrome-shaped distribution (clumped
near a 100 µs grid step with lognormal jitter, occasional 250-1500 µs
spike from kernel scheduling) or a bot-shaped distribution (perfect
multiples of `Math.round(now*10)/10`; or zero variance, returning the
same number 50 times in a row; or a uniform-random scatter from a
naive jitter implementation). Two distinct dimensions:

- **Identity** (the values exist + are correctly typed). Cheap to
  fake: install a function on a prototype.
- **Distribution shape** (the values, over many calls, have the
  statistics real Chrome would produce). Hard to fake: the engine has
  to model the actual physical process — TSC + kernel scheduler +
  V8's own quantization-with-jitter — well enough that the **moments
  of the distribution** match.

The same split applies to mouse motion. A bot that emits one
`mousemove` event with `clientX/clientY` at the click target passes
"events exist" but fails "velocity profile, micro-tremor amplitude,
sub-stroke decomposition". Per BeCAPTCHA-Mouse benchmarks, a
random-forest classifier separates straight-line / single-Bezier
trajectories from real human motion with **~99% accuracy on a single
trajectory** ([arXiv 2005.00890](https://arxiv.org/pdf/2005.00890)). A
Σ-Λ-with-tremor synthesizer drops that to ~93% — still detectable, but
no longer at the "one trajectory and you're done" cliff.

The implication for BO:

- **Identity gaps are listable, finite, and fixable.** `17_WEB_API_PARITY_MATRIX.md`
  enumerates ~120 interfaces; a missing one is a single PR.
- **Distribution gaps are open-ended.** A vendor can always add one
  more feature to its random forest. The defender's job is to keep the
  distribution close enough to Chrome's that the marginal feature is
  uninformative — and to track which moments matter most per vendor.

Two consequences shape the rest of this chapter:

1. **Static-only fixes have an asymptote.** Every Class-A primitive
   you ship (canvas noise, TLS impersonation, prototype masking) gets
   you closer to "passes identity probes" but cannot, by itself, pass
   "passes distribution probes". The 110-routed pass number from
   `01_CURRENT_STATE.md` includes sites where the distribution probe
   never fired (because no challenge gated on it, or because the page
   had no scroll-trigger). For sites that DO gate (homedepot's
   Akamai sec-cpt; the Kasada cluster), no amount of static work fixes
   them. See `26_AKAMAI_BMP_DEEP.md` §4 + `08_KASADA_FRONTIER.md` §2.
2. **"Perfect" is its own signal.** A bot that synthesizes ten
   identical Σ-Λ trajectories across ten sessions is detectable by
   the *absence of session-to-session variance*. The defender has to
   inject variance at the right places (per-session RNG seed, per-call
   site salt) — covered in §5.

---

## 2. Timing fingerprinting deep dive

### 2.1 `performance.now()` granularity

#### Spec baseline

W3C High Resolution Time Level 3 [§4 Time Resolution](https://www.w3.org/TR/hr-time/#sec-time-resolution):

> Let *time resolution* be 100 microseconds, or a higher
> implementation-defined value.
>
> If `crossOriginIsolatedCapability` is true, set time resolution to
> be **5 microseconds**, or a higher implementation-defined value.

The Spectre mitigation history is in
[Chrome 91 announcement](https://developer.chrome.com/blog/cross-origin-isolated-hr-timers)
— prior to Chrome 91, desktop Chrome with site-isolation returned
5 µs, Android returned 100 µs. The spec change unified both platforms
at 100 µs default, with 5 µs available only under COOP+COEP-enabled
cross-origin isolation. The Spectre context is documented in
[V8's "A year with Spectre"](https://v8.dev/blog/spectre): high-res
timers accelerate the timing side-channel by collapsing the
side-channel decoding loop from ~milliseconds to ~microseconds, so
restricting them shipped as a defense-in-depth measure alongside the
process-isolation work.

W3C [HR-Time §7.1](https://www.w3.org/TR/hr-time/#dom-performance-now)
also mandates **monotonicity**:

> The difference between any two chronologically recorded time values
> returned from the `now()` method MUST never be negative if the two
> time values have the same time origin.

And [§9.1](https://www.w3.org/TR/hr-time/#privacy-security) explicitly
lists the three implementation defenses:

> Mitigation techniques include:
> - Resolution reduction.
> - Added jitter.
> - Abuse detection and/or API call throttling.

Chrome's choice is **#1 + #2**: 100 µs grid, plus a small jitter to
defeat trivial recovery by averaging.

#### Real Chrome 130 measured shape

From [vmonaco / "Device Fingerprinting with Peripheral Timestamps"](https://vmonaco.com/papers/Device%20Fingerprinting%20with%20Peripheral%20Timestamps.pdf)
and our own ab_harness `/tl` captures (2026-05-10, the Kasada V3
envelope research arc):

```
performance.now() hot-loop, 10_000 samples, idle desktop
  grid                : 0.1 ms (100 µs) — quantized step
  jitter distribution : LogNormal(μ = ln 8 µs, σ ≈ 0.4) clamped [0, 35] µs
  heavy-tail spikes   : Bernoulli(p ≈ 1/1024) × Exp(λ = 1/200 µs) clamped ≤ 1500 µs
  monotonic clamp     : never decreases between two calls
```

The shape is **not** Gaussian-around-the-grid; the right tail from
kernel scheduling latency dominates. A pure software clock that returns
`floor(raw_us / 100) * 100` produces `set(diffs).size === 1` for tight
loops on a fast CPU (every adjacent call hits the same 100 µs bucket
because the raw clock advanced by less than 100 µs). That is the
**single most common bot tell in any vendor's timing probe**, and it is
the one BO explicitly defeats — see §2.6.

#### Vendor probes

Three documented categories:

- **Grid probe.** Run `n = 1000` calls in a hot loop; collect
  `diffs = pairs(t[i+1] - t[i])`; assert `new Set(diffs).size > 1`.
  Trivial; defeated by any non-zero jitter.
- **Distribution-shape probe.** Same loop, then compute moments
  (mean, variance, skew, kurtosis) of the diffs. Compare against a
  Chrome reference distribution. A uniform-random jitter
  (`Math.random() * 35`) trivially fails on skew (real distribution
  is right-skewed lognormal-with-tail; uniform is flat).
- **Heavy-tail probe.** Long hot loop (10_000+ samples) looking for
  the ~1 in 1024 large excursion. A clamped-Gaussian jitter never
  produces one and looks "too clean". Kasada's `/tl` v2 envelope
  embeds two such moments in the ~30-field cleartext JSON (per
  `08_KASADA_FRONTIER.md` §3).

Resolution recovery via averaging is also a published academic attack
([Schwarz et al., "JavaScript Zero"](https://www.ndss-symposium.org/wp-content/uploads/2018/02/ndss2018_07A-2_Schwarz_paper.pdf)):
N independent calls quantized to 100 µs can still resolve sub-µs
events if you can repeat the experiment N times. Browsers add jitter
(per the spec) specifically to break this — but the jitter itself
becomes a fingerprintable distribution. Both Chrome (uniform across
the grid step) and Firefox (different shape) are distinguishable from
each other this way.

### 2.2 `Date.now()`

Spec ([ECMAScript §21.4.3.1](https://tc39.es/ecma262/#sec-date.now)):

> Returns the number of milliseconds elapsed since the epoch, as a
> Number value, with millisecond precision.

Always 1 ms granularity by spec; cannot be reduced by Spectre
mitigations (it doesn't carry sub-ms precision in the first place).
**This makes `Date.now()` the canonical fallback when sites can't get
sub-ms data.** Kasada's `/tl` envelope mixes both: `performance.now()`
for sub-ms event timings, `Date.now()` for the wall-clock event
attached to the POST. Real Chrome has a small (~1-3 ms) skew between
`Date.now()` and a same-call `performance.now() + performance.timeOrigin`
because `Date.now()` may have been adjusted by NTP between the time
origin capture and the call. A bot returning `Date.now() === origin +
performance.now()` exactly is detectable.

BO's `Date.now()` is the V8 builtin (no override). `performance.now()`
in BO is `op_perf_now_humanized()` (Rust-side `Instant::elapsed`-based
clock), and `performance.timeOrigin` is **not** wired to a humanized
op — see §2.6. Means: an attacker who recomputes
`origin + performance.now()` and compares against `Date.now()` will see
two clocks coming from different sources, with no guarantee of any
particular skew distribution. Action item for §8.

### 2.3 `requestAnimationFrame` cadence

Spec ([HTML Living Standard "rendering and animation"](https://html.spec.whatwg.org/multipage/webappapis.html#dom-animationframeprovider-requestanimationframe)):
the user agent samples the callback list at a rate matching the
display refresh, typically 60 Hz (every ~16.67 ms). When the document
is in a hidden tab, [Chrome](https://developer.chrome.com/blog/timer-throttling-in-chrome-88)
pauses rAF entirely; setTimeout/setInterval throttle to **1 Hz**
instead.

What real Chrome looks like:

```
visible tab:
  consecutive rAF callback ts: 16.66, 33.33, 50.00, 66.66 …  (Δ ≈ 16.67 ms)
  occasional missed-frame: 16.66, 33.33, 66.66 (one 33 ms gap from GC)

hidden tab (Page Visibility "hidden"):
  rAF: PAUSED — no callbacks fire at all
  setTimeout(0): clamped to 1000 ms minimum
  setInterval(1000): undisturbed
```

Vendor probes:

- **Cadence probe.** Schedule 60 rAF callbacks; collect timestamps;
  assert mean(Δ) ∈ [16.0, 17.5] ms and stddev(Δ) < 5 ms. A "fire
  every microtask" implementation produces Δ ≈ 0 and fails instantly.
  Kasada's main-window probe scores this (per `08_KASADA_FRONTIER.md`
  §3).
- **Hidden-tab probe.** Set `document.visibilityState`, schedule
  setTimeout(0) and rAF, assert rAF doesn't fire and setTimeout
  fires after >900 ms. A bot that ignores visibility-state in its
  timer scheduler fails this.

BO's rAF (`timer_bootstrap.js:147-160`):

```js
globalThis.requestAnimationFrame = function requestAnimationFrame(callback) {
    const id = ++_rafId;
    _rafCallbacks.set(id, callback);
    // Fire at ~16ms (60fps) via real timer, not microtask.
    // Anti-bot systems (Kasada) measure rAF timing and flag instant firing.
    setTimeout(() => {
        const cb = _rafCallbacks.get(id);
        if (cb) {
            _rafCallbacks.delete(id);
            cb(performance.now());
        }
    }, 16);
    return id;
};
```

The 16 ms constant defeats the cadence probe (mean is correct), but it
is **constant**, not jittered. A vendor that scores stddev(Δ) over 60
callbacks would see ~0 ms variance, well outside the 1-3 ms real
Chrome shows. This is an **open gap** — see §8.

Hidden-tab throttling: **not implemented**. BO never marks a document
"hidden"; the visibility state machine in `window_bootstrap.js` always
returns "visible". A vendor that explicitly drives `visibilitychange`
+ checks rAF/setTimeout cadence would catch this. In practice, no site
in the 126 corpus has been observed to use this probe (vendors prefer
the cheaper static probes), but it's a hole.

### 2.4 `setTimeout` / `setInterval` clamping

The HTML spec
([HTML Living Standard "timers and user prompts" §5](https://html.spec.whatwg.org/multipage/timers-and-user-prompts.html#timer-initialisation-steps)):

> Step 5: **If nesting level is greater than 5, and timeout is less
> than 4, then set timeout to 4.**
>
> Step 10-11: Increment nesting level by one. Set task's timer nesting
> level to nesting level.

In English: the first 5 levels of nested `setTimeout(fn, 0)` get
~0 ms; from the 6th level onward, the implementation clamps to 4 ms.
This means a tight `setTimeout(fn, 0)` loop slows down measurably after
~5 iterations. The phenomenon is universally implemented across
Chrome / Firefox / Safari and is a well-known JS performance gotcha.

A bot that ignores nesting-level clamping fires the 6th setTimeout(0)
in ~0 ms instead of 4 ms — measurable from JS by chaining and timing.
Kasada's `/ips.js` has at least one nesting-aware probe (visible in
the captured 2026-05-10 sensor decode).

Background tab clamping is a separate rule
([Chrome 88 timer throttling](https://developer.chrome.com/blog/timer-throttling-in-chrome-88)):
setTimeout/setInterval in a `visibilityState === "hidden"` document
clamp to **1000 ms** minimum, and chained timers get bucketed into
1-second wakeup batches to save battery. Same probe shape as the
nesting-level one but for the visibility axis.

BO's `setTimeout` (`timer_bootstrap.js:62-79`):

```js
globalThis.setTimeout = function setTimeout(callback, delay = 0, ...args) {
    if (typeof callback !== "function") {
        callback = new Function(String(callback));
    }
    const ms = Math.max(0, delay | 0);     // no nesting-level clamp
    const id = ops.op_set_timeout(ms);
    const p = ops.op_timer_sleep(ms);
    _maybeUnref(p, ms);
    ...
};
```

`setInterval` (`timer_bootstrap.js:109-133`) does enforce a **4 ms
floor unconditionally** (`Math.max(4, delay | 0)`), which is *too*
tight (real Chrome only clamps to 4 ms after nesting > 5; the first
call to setInterval(0, …) should fire at ~0 ms). The wrong direction
(too slow not too fast) — still detectable, but flagged-as-bot only if
a vendor times the first-call latency of `setInterval(0, fn)` against
a reference.

`setTimeout` has **no nesting-level clamp at all**: BO will fire the
100th nested setTimeout(0) in ~0 ms, while real Chrome fires it in
~4 ms. This is a real probe surface — see §8 acceptance bar.

A separate path, `__bgSetTimeout` (`timer_bootstrap.js:91-107`), is
the engine-internal helper that `humanize.js` uses for its synthetic
input events (the recent `_sched = __bgSetTimeout || setTimeout`
selector at `humanize.js:52`). It exists purely so the event-loop
"idle" detector can return to the caller without waiting for
humanize's pending 1.8 s of synthetic-mouse timers to finish. Semantics
match `setTimeout` except the underlying promise is unref'd
(`Deno.core.unrefOpPromise`) so it doesn't gate `run_until_idle`. The
JS-observable behavior is **identical** — `humanize.js`'s timers still
fire on schedule for any page whose event loop stays alive
(anti-bot pages do, benign pages don't need them to). Importantly,
the `__bgSetTimeout` global is hidden by `cleanup_bootstrap.js`
before page-script execution, so it is not a probe surface for
vendors.

### 2.5 Per-vendor timing checks

The capability axis. Filled from the deep-dive chapters
(`06`, `07`, `08`, `25`, `26`) and the open-source references cited.

| Vendor | `performance.now` shape | RAF cadence | setTimeout nesting | Background throttle | `Date.now` skew | Source |
|---|---|---|---|---|---|---|
| **AWS WAF** (Targeted) | ✓ (token includes timestamp; SDK observes hot-loop diffs) | ? | ? | ? | ? | `06_AWS_WAF_SOLVER.md §3`; [AWS WAF JS API](https://docs.aws.amazon.com/waf/latest/developerguide/waf-js-challenge-api.html) |
| **Kasada** | ✓ (V3 envelope embeds 2 timing moments; `/ips.js` hot-loop probe captured 2026-05-10) | ✓ (60-callback cadence + stddev scored) | ✓ (nesting-level chained probe captured) | ? | ✓ (origin+now vs Date.now compared) | `08_KASADA_FRONTIER.md §3` |
| **DataDome** | ✓ (`tags.js` per W6a research scored a `set(diffs).size === 1` check; defeated by the perf_ext jitter) | ? | ? | ? | ✓ (event timestamps validated against Date.now) | `07_DATADOME_PRIMITIVES.md`; [Kameleo DataDome 2025 guide](https://kameleo.io/blog/guide-to-bypassing-datadome) |
| **Akamai BMP** | ✓ (sensor_data v2 field set includes hot-loop diff samples; v3 schema same) | ✓ (BMP SDK collects RAF timestamps per [Akamai BMP SDK docs](https://developer.akamai.com/tools/sdk/bot-manager)) | ? | ✓ (visibilitychange + sleep cycle observed in BMP scripts) | ✓ | `26_AKAMAI_BMP_DEEP.md §3` |
| **Cloudflare** (Managed) | ? (challenge JS heavily obfuscated; assumed yes) | ? | ? | ? | ? | `25_CLOUDFLARE_DEEP.md` |
| **Cloudflare Turnstile** | ✓ (per [CaptchaAI Turnstile breakdown](https://blog.captchaai.com/how-cloudflare-turnstile-works) the widget tracks cursor velocity + click timing) | ✓ | ? | ? | ? | `25_CLOUDFLARE_DEEP.md §Turnstile` |
| **PerimeterX / HUMAN** | ✓ (sensor JS has clock probe per industry write-ups) | ? | ? | ? | ✓ | [Scrapfly PerimeterX 2026](https://scrapfly.io/blog/posts/how-to-bypass-perimeterx-human-anti-scraping) |
| **Imperva (Incapsula)** | ✓ (180+ properties incl. timing per `18_ANTI_BOT_VENDOR_COOKBOOK.md §2.7`) | ? | ? | ? | ? | `18 §2.7` |
| **F5 / Shape** | ✓ (signals attached to every HTTP request per [F5 Shape DevCentral](https://community.f5.com/kb/technicalarticles/what-is-shape-security/284359)) | ? | ? | ? | ? | `18 §2.13` |
| **Radware IDBA** | ✓ (timing one of the 250+ params per [Radware IDBA WP](https://www.radwarebotmanager.com/web/wp-content/uploads/IDBA_WP.pdf)) | ? | ? | ? | ? | `18 §2.14` |

`?` = no public evidence either way; vendor's challenge JS is
obfuscated and we have not captured the relevant probe. Absence of a
✓ is **not** evidence the check is absent; it's "we cannot point at
the line that does it". When in doubt, assume yes — these checks are
cheap to add server-side, so defenders ship them.

### 2.6 BO timing coverage

Per-primitive status. **`P`** = present; **`H`** = humanized
(distribution-shape-correct, not just identity); **`G`** = gap.

| Primitive | Status | File:line | Notes |
|---|---|---|---|
| `performance.now()` on `Performance.prototype` | **H** | `window_bootstrap.js:5430-5434` → `op_perf_now_humanized` (`perf_ext.rs:84-87`) | LogNormal(ln 8 µs, 0.4) jitter on 100 µs grid + Bernoulli(1/1024) spike. Distinct-value test asserts >10 distinct deltas in 500-iter hot loop (`perf_ext.rs:146-162`). |
| `performance.now()` in workers | **H** | `worker_bootstrap.js:131-138` | Reuses the same op; same shape. |
| `performance.now()` fallback (legacy install) | **G** | `timer_bootstrap.js:170-173` | `Date.now() - startTime` ms granularity. Defensive only — never reached if `Performance.prototype.now` was installed (it is, in every real page). Verify by audit. |
| `performance.timeOrigin` | **G** | unknown — likely stub or absent | A Kasada-style `origin + now() === Date.now()` probe would catch the mismatch. Action item §8. |
| `performance.mark / measure` | **P** | `window_bootstrap.js:2902+` | Identity present; per-entry timestamp comes from the same humanized op (verify). |
| `PerformanceObserver` | **G** (stub) | `window_bootstrap.js:2183-2202` | Class exists, methods no-op. Akamai sensor reads from this; a check that calls `observe({entryTypes:['paint']})` and waits for any callback never gets one. `17_WEB_API_PARITY_MATRIX.md §2.8` flags this. |
| `requestAnimationFrame` | **P** (cadence) / **G** (jitter, hidden-tab) | `timer_bootstrap.js:147-160` | Fires at 16 ms via setTimeout — constant, not jittered. Hidden-tab pause not implemented. |
| `cancelAnimationFrame` | **P** | `timer_bootstrap.js:162-164` | Standard. |
| `requestIdleCallback` | ? | (search bootstrap) | Likely missing; sites that depend on it polyfill via setTimeout. |
| `setTimeout` nesting-level clamp | **G** | `timer_bootstrap.js:62-79` | No nesting-level tracking; 6th nested setTimeout(0) fires at ~0 ms instead of 4 ms. |
| `setInterval` 4-ms floor | partial-wrong | `timer_bootstrap.js:113` | Clamps to 4 ms unconditionally. Real Chrome only clamps after nesting > 5. Direction is "too slow", less detectable than "too fast". |
| `clearTimeout` / `clearInterval` | **P** | `timer_bootstrap.js:135-142` | Standard. |
| `Date.now()` | **P** | V8 builtin (no override) | 1 ms precision. Decoupled from `performance.now()` — see §2.2 skew gap. |
| `setTimeout` background-throttle (1 s in hidden tab) | **G** | not implemented | Document always "visible"; visibility-state never flips. |

The headline pattern: **identity coverage is solid; distribution
coverage is solid only for `performance.now`**. RAF cadence is constant
(no jitter), nesting clamps are missing or one-sided, hidden-tab
throttling is absent, and `performance.timeOrigin` is the canonical
"escape hatch" a vendor would reach for if they couldn't recover from
`performance.now()` jitter directly.

### 2.7 The humanize tax

`humanize.js` (covered in detail in §3.1) emits its mouse/scroll
timers through `__bgSetTimeout`, which routes to
`Deno.core.unrefOpPromise` and lets the event loop return to the
caller without waiting. There is **no observable timing difference**
for the page — the timers still fire on schedule when the loop is
otherwise busy (anti-bot pages always are). The cost is only paid in
the **idle benign page** case, where we'd otherwise wait 2-3 s for the
synthetic input timers to drain on a page that doesn't care about them.

The naming is load-bearing: `__bgSetTimeout` is the engine-internal
selector, hidden from page scripts by `cleanup_bootstrap.js` before
script execution. A vendor cannot probe its existence (no
`globalThis.__bgSetTimeout` after cleanup). The engine-side selector
in `humanize.js:52` is `globalThis.__bgSetTimeout || globalThis.setTimeout`
which runs before cleanup, so the selector resolves correctly. If
cleanup were ever reordered before humanize execution, `humanize.js`
would silently fall back to plain `setTimeout` and the synthetic input
events would gate `run_until_idle` — same correctness, worse latency.
The risk register flags this in `24_RISK_REGISTER.md` (pool-warm-reuse
path).

---

## 3. Behavioral biometrics deep dive

### 3.1 Mouse trajectory analysis

#### What real users produce

Real cursor motion is well-modeled by Plamondon's **Σ-Λ (sigma-lognormal)
kinematic theory**:

> v(t) = (1 / (σ √(2π))) · (1/(t−t₀)) · exp(−(ln(t−t₀) − μ)² / (2σ²))

(per the `humanize.js` doc comment lines 12-22). The velocity profile
is asymmetric (fast acceleration, slow decay), and a long ballistic
path decomposes into 2-7 sub-strokes per Meyer 1988 + Plamondon 1995:
~85% of the distance in a primary stroke, ~15% in 1-3 corrective
sub-movements (each itself Σ-Λ). On top of the gross motion sits a
~8 Hz, ~1-2 px **physiological tremor**, modeled adequately by a
smoothed (pink-spectrum) white-noise process.

The vendor's classifier (BeCAPTCHA-Mouse, per [arXiv 2005.00890](https://arxiv.org/pdf/2005.00890)
§5.3) integrates the **2nd derivative (jerk) of position over time**
plus the velocity-profile asymmetry to score "humanness". The
discriminative features it extracts:

- Number of velocity peaks (real: 2-7; bot single-Bezier: 1)
- Σ across strokes of (peak-velocity-time / stroke-duration) (real:
  clustered ~0.35; bot: uniform)
- Endpoint jerk integral (real: bounded, smooth deceleration; bot:
  delta function — last sample teleports to target)
- Per-session entropy of (σ, μ) across strokes (real: tight; bot: 0
  or undefined)

A single bot trajectory that's a uniform-time Bezier hits ~99%
classification accuracy as "bot" on a single trajectory. Σ-Λ-with-tremor
drops that to ~93%. To get below 50% (i.e. classifier no better than
random), you also need **per-session and per-target variance** in the
σ/μ parameters and the sub-stroke count.

#### What BO produces

BO has **two layers** of mouse synthesis:

1. **`crates/stealth/src/behavior.rs:142-287`** — the canonical
   high-quality generator. Σ-Λ multi-stroke with Fitts's-law-derived
   total time (`T = 230 + 166 · log2(D/W + 1)` ms), per-stroke
   amplitudes (`0.85·D` primary + correctives), per-stroke σ ~
   Normal(0.25, 0.05), per-stroke μ ~ Normal(−1.6, 0.2), inter-stroke
   onset ~ LogNormal(ln 90 ms, 0.3), pink-tremor at ~8 Hz × 1.5 px,
   smoothstep tail (15-sample C¹-continuous deceleration with zero
   added velocity at splice and zero velocity at arrival — **bounded
   jerk at the endpoint**, the BeCAPTCHA discriminator). Exposed as
   `op_behavior_mouse_trajectory` and called from
   `humanize.js:319-325`.
2. **`crates/browser/src/js/humanize.js`** — the in-V8 dispatcher.
   It calls `op_behavior_mouse_trajectory` for the **historical
   pre-population** of `__akamai_events.mouse` (the W6a "user moved
   cursor 1800 ms before nav" buffer, `humanize.js:303-398`), then
   uses an **in-JS Σ-Λ sampler** (`_sigmaLognormalTimes` at
   `humanize.js:121-136`) for the **live mouse-motion cycle** that
   runs every 4 s (`humanize.js:238-282`). The in-JS sampler is a
   simpler version: it just produces sample times along a linear path
   between anchor points, with the times following the Σ-Λ density
   (denser near the modal time). It does NOT do the full multi-stroke
   amplitude split or the smoothstep terminal decel from `behavior.rs`.

Both layers emit:
- `mousemove` (legacy) on window + document + body
  (`humanize.js:185-194`)
- `pointermove` (modern, Chrome 55+) on window + document + body
  (`humanize.js:198-215`) — paired emission per
  `humanize.js:181-217` doc comment, matching real Chrome's
  per-physical-event behavior
- Per-event recording into `globalThis.__akamai_events.mouse`
  (`humanize.js:75-80`) — the per-page buffer that any private
  vendor solver consumes

The **synchronous pre-population** is the key piece (`humanize.js:303-398`).
DataDome's `tags.js` scores its 31-feature mouse-path vector at POST
time. If `__akamai_events.mouse` is empty at the moment `tags.js`
serializes — which happens when our setTimeouts haven't fired yet —
the empty-coord-list heuristic flags us. So the pre-population uses
the Rust Σ-Λ generator to drop ~12-14 historical points spanning a
[−1800, −100] ms window into the buffer **synchronously, before
returning from the IIFE**. Then it also dispatches one synchronous
mousemove+pointermove pair so live event listeners (DataDome,
PerimeterX) see at least one event before any async-timer callback
can fire.

#### Coverage table

| Feature | Real Chrome | `humanize.js` (live cycle) | `behavior.rs` (Rust gen) | Gap |
|---|---|---|---|---|
| Multi-stroke (2-7 sub-movements) | yes | partial (anchor-to-anchor only, no per-anchor sub-strokes) | yes | live cycle uses linear interpolation between anchors |
| Σ-Λ velocity within each stroke | yes | yes (via `_sigmaLognormalTimes`) | yes | match |
| Fitts's-law total time | yes | no — fixed `800 + Math.random() * 300` ms | yes (`T = 230 + 166 · log2(D/W+1)`) | live cycle ignores target width |
| Pink-spectrum tremor | yes (~8 Hz, 1-2 px) | yes (`_gauss() * 0.8` per sample — Gaussian, not low-passed) | yes (low-passed at α=0.3) | live cycle is white noise, not pink |
| Endpoint smoothstep decel | yes (bounded jerk) | no — last sample is `_lerp(a, b, 1.0)` | yes | live cycle has an impulse spike at each anchor terminus |
| Per-session variance | yes (mood/fatigue) | yes (`Math.random()` for anchors, durations, jitter sigma per cycle) | yes (per-session `BehaviorProfile.seed`) | match in shape, but BO uses `Math.random()` not a seeded PRNG → not reproducible for testing |
| 8 ms (125 Hz) sample rate | typical USB mouse | no — sample-time scaled to stroke duration / 15 | yes (8 ms fixed) | live cycle has variable inter-sample time; this is actually **less detectable** than fixed-grid, because real USB rate jitters too |
| `isTrusted: true` on events | yes | yes (`Object.defineProperty` at `humanize.js:97`) | n/a | match |
| `screenX/Y` offset from `clientX/Y` | yes (window chrome) | yes (`+ 90` per `humanize.js:188, 203`) | n/a | match (90 px = nominal Chrome title bar height) |

The live-cycle gaps are not critical for the current set of vendors
BO is targeting (DataDome's W6a buffer is satisfied by the
pre-population; the live cycle is "keep-alive" noise during long page
loads), but they're documented here so a future contributor working
on a Kasada or PerimeterX deep dive doesn't have to rediscover them.

**The real risk** is the live-cycle `Math.random()` calls. BO has no
session-level RNG state for the live cycle — every cycle samples
independently. A vendor that integrates the mouse-event stream across
a multi-page session and looks for per-session correlation (do the
σ/μ across pages cluster like one user, or are they uncorrelated like
50 different users?) would see the BO pattern as "50 different users".
This is the **per-session variance vs per-call randomness** problem
covered in §5.

### 3.2 Keystroke dynamics

#### What real users produce

Two per-keystroke metrics (CMU + Buffalo benchmarks per the
[Buffalo CUBS dataset docs](https://www.buffalo.edu/cubs/research/datasets.html)
and [CMU keystroke benchmark](https://www.cs.cmu.edu/~keystroke/)):

- **Dwell time** (keydown → keyup). LogNormal(μ ≈ ln 95 ms, σ ≈ 0.30).
  Tight per-user cluster — this is **why keystroke dynamics works as
  authentication**: a user's σ across sessions is small.
- **Flight time** (previous keyup → current keydown). LogNormal(μ ≈
  ln 130 ms, σ ≈ 0.55) at a baseline of 50 WPM. Heavy right tail.
  Bigram-modulated: alt-hand digraphs (`th`, `he`, `or`) at ~70% of
  median; same-finger digraphs (`ed`, `er`, `un`) at ~140%; same-key
  (`ll`, `oo`) at ~200%.

Real typing also includes:
- 1-2% typo rate followed by a backspace+correct burst at faster cadence
- Shift-key handling: the shift is typically held during the next
  letter's dwell, not released between
- Rare same-finger super-fast bursts (~50 ms) on practiced sequences
  (the user's own name, "the", "and")

#### What BO produces

`crates/stealth/src/behavior.rs:421-464` — `keystroke_timings`. Generates
a `Vec<KeystrokeTiming>` with `dwell_ms` + `flight_ms` per character:

```rust
let ms_per_char = 60_000.0 / (profile.typing_wpm_mean * 5.0);
let flight_median = (ms_per_char - 95.0).max(40.0);
let flight_dist = LogNormal::new(flight_median.ln(), 0.55).unwrap();
let dwell_dist = LogNormal::new(95.0_f32.ln(), 0.30).unwrap();
// ... per character:
let dwell = dwell_dist.sample(rng).clamp(40.0, 400.0);
let flight = (flight_dist.sample(rng) * bigram_ratio(prev, ch)).clamp(20.0, 1000.0);
```

Bigram ratios (`behavior.rs:381-419`): a curated table of the top-20
English bigrams from Norvig's analysis, with 0.7× (alt-hand), 1.0×
(neutral), 1.4× (same-finger), 2.0× (same-key) multipliers.
Provenance documented in `crates/stealth/src/BIGRAM_PROVENANCE.md`
(aggregates from CMU + Buffalo published means).

**Wiring status: NOT WIRED.** `keystroke_timings` is exposed as a
Rust function with unit tests
(`behavior.rs:583-685` for the trajectory tests; keystroke-specific
tests at the end of the file) but **no op exposes it to JS**, and
`humanize.js` does **not** synthesize keystrokes. The only keystroke
events BO ever dispatches are zero (verified by grep: `humanize.js`
has no `keydown`/`keyup` event creation, and `_akRecKey` at
`humanize.js:81-86` is defined but never called).

#### Consequence

Sites that gate on keystroke biometrics — auth pages, search
autocomplete that scores typing rhythm, in-form-fill detection
(F5/Shape, Akamai BMP on login flows, Imperva on form posts) — see
**zero keystroke events from BO** when no real input is sent. The
absence is itself a signal: a page that loads, has `<input>` focused,
and never receives a single `keydown` for 30+ seconds is suspicious.

What that means in practice:
- BO can navigate to a login page; it cannot submit a login.
- BO can render a search page; it cannot type into the search box
  and have the suggestions trigger.
- BO does pass any check that scores keystroke timing distribution,
  because there's no data to score (no false positive); but a check
  that fires on `keydownCount === 0 && hasInput()` flags us.

Per `02_GAP_ANALYSIS.md`, no site in the 126 corpus is currently
gated on keystroke-event presence (the corpus selects for "loads a
page", not "submits a form"), so wiring `keystroke_timings` is **not
on the v0.1.0 critical path**. It is a top-3 candidate for v0.2.0
when the corpus expands to form-submission flows.

### 3.3 Scroll patterns

#### What real users produce

Two distinct distributions:

- **Trackpad** (Apple Magic Trackpad, MacBook touchpad). Continuous
  inertial scroll with exponential momentum decay. WheelEvent
  `deltaY` values are small (1-5 px per event) at ~60 Hz, with a
  velocity ramp-up + decay envelope.
- **Mouse wheel** (notched scroll wheel). Discrete events at LogNormal
  intervals, `deltaY` clustered at 100 (Chrome's standard "1 line"
  per notch in `deltaMode=0`), sometimes 33 or 53 (per-OS variants).
  No momentum.

Vendors fingerprint:
- Modal `deltaY` value (100 → mouse wheel; sub-10 → trackpad)
- Inter-event Δt distribution
- Cumulative scroll distance vs total time (real: bursty; bot:
  uniform)

#### What BO produces

`crates/stealth/src/behavior.rs` has `WheelTick` + a scroll-burst
generator (`behavior.rs:470+`, full body not shown above but it
synthesizes either Trackpad or Wheel patterns per
`BehaviorProfile.scroll_style`). Like keystrokes, it is exposed as a
Rust function but **not wired into `humanize.js`**.

`humanize.js:220-234` (`_fireScrollStep`) synthesizes scrolls
manually:

```js
function _fireScrollStep(deltaY) {
    try {
        const wheel = new WheelEvent('wheel', {
            bubbles: true, cancelable: true, view: window,
            deltaY, deltaMode: 0,
        });
        _dispatch(document, wheel);
        // Drive a real scroll on the documentElement so subsequent
        // pageYOffset reads reflect the motion.
        window.scrollBy({ top: deltaY, behavior: 'instant' });
        _dispatch(document, new Event('scroll', { bubbles: true }));
        _dispatch(window, new Event('scroll', { bubbles: false }));
        _akRecScroll(deltaY);
    } catch (e) {}
}
```

The per-cycle scroll plan (`humanize.js:274-281`):

```js
const scStartT = mouseT + 100;
const steps = [80 + Math.random() * 40, 60 + Math.random() * 30];
let curScT = scStartT;
for (const step of steps) {
    _sched(() => _fireScrollStep(step), curScT);
    curScT += 100 + Math.random() * 100;
}
```

**Two scroll events** per cycle, deltaY in [80, 120] and [60, 90].
This pattern does **not match either of the real distributions**:
- Not "mouse wheel": deltaY is not clustered at 100 (it's uniformly
  random in two bands).
- Not "trackpad": only 2 events per cycle, not the 30-60 events of a
  real momentum-scroll.

Status: identity present (events are dispatched, scroll position
advances, listeners fire), distribution gap (wrong shape for either
input device). A vendor that scores deltaY modal value would not
recognize the pattern.

### 3.4 Touch events (mobile)

#### What real users produce

Touch events carry **multiple per-touch parameters** that bots
typically miss:

- `touches[].radiusX / radiusY` — finger contact area, ~10-25 px on
  most phones
- `touches[].force` — pressure (0-1 scaled), varies per touch
- `touches[].rotationAngle` — major-axis angle of the ellipse
- Multi-touch: `touches[].length > 1` during a pinch

Touch-specific behavioral biometrics (per [BeCAPTCHA, arXiv 2005.13655](https://arxiv.org/pdf/2005.13655)):
- Swipe curve velocity profile (Σ-Λ applies here too)
- Press-release timing per tap
- Inter-touch finger correlation in multi-touch
- Accelerometer + gyroscope readings during the touch (real phones
  always show non-zero device motion during touch)

Mobile-focused vendors (Akamai BMP iPhone/Android SDKs, PerimeterX
mobile, F5 Shape mobile SDK) score these heavily because **the
desktop bot-detection arsenal doesn't transfer to mobile**: TLS
fingerprint can be faked on a server, but accelerometer-correlated
touch can't.

#### What BO produces

Search for `TouchEvent` in `humanize.js`: **zero matches**. Search
for `Touch(` constructor calls: zero. The `__akamai_events.touch`
buffer is declared (`humanize.js:63`) and `_akRecTouch` would record
into it, but there is no `_akRecTouch` function (the counters object
has a `touch` field but no helper that increments it).

`Touch` and `TouchEvent` interfaces exist in `interfaces_bootstrap.js`
(illegal-ctor stubs only per `17_WEB_API_PARITY_MATRIX.md`), but
neither is instantiated or dispatched.

**Status: not implemented.** A vendor that gates on
`touchEventCount > 0 && userAgent.includes('Mobile')` will see BO's
iPhone and Pixel profiles as "mobile UA but no touches" — a strong
bot signal. This is **unique to the mobile profiles**; desktop
profiles are not expected to fire touch events.

The mobile profiles in `crates/stealth/src/presets.rs` set the right
`navigator.maxTouchPoints` value (5 for iPhone, 5 for Pixel — verify
in `presets.rs`) so the identity check passes; the behavioral gap
opens when a vendor goes one step further and checks for actual touch
event presence.

### 3.5 Per-vendor behavioral checks

| Vendor | Mouse curve | Mouse pre-pop buffer | Keystroke | Scroll modal-deltaY | Touch (mobile) | Source |
|---|---|---|---|---|---|---|
| **Akamai BMP** | ✓ scored | ✓ `_abck` sensor reads `__akamai_events.mouse` | ✓ on login pages | ✓ | ✓ (BMP SDK) | `26_AKAMAI_BMP_DEEP.md §3`; [Akamai BMP SDK](https://developer.akamai.com/tools/sdk/bot-manager) |
| **Kasada** | ✓ (300+ signals per session, behavioral one) | ✓ (V3 envelope embeds mouse summary stats) | ✓ | ✓ | ✓ | `08_KASADA_FRONTIER.md §3` |
| **Radware IDBA** | ✓ (intent encoder consumes mouse/keystroke/URL) | ✓ | ✓ | ✓ | ✓ | [Radware IDBA WP](https://www.radwarebotmanager.com/web/wp-content/uploads/IDBA_WP.pdf) |
| **F5 / Shape** | ✓ (dwell + flight per [F5 DevCentral](https://community.f5.com/kb/technicalarticles/what-is-shape-security/284359)) | partial | ✓ | ? | ✓ | `18 §2.13` |
| **PerimeterX / HUMAN** | ✓ (Press-and-Hold pressure curve) | ✓ (sensor JS coord buffer) | ? | ? | ✓ (mobile SDK) | [Scrapfly PerimeterX 2026](https://scrapfly.io/blog/posts/how-to-bypass-perimeterx-human-anti-scraping); `18 §2.6` |
| **DataDome** | ✓ (`tags.js` 31-feature mouse-path vector at POST time) | ✓ (`_initialCoordsList`) | partial | partial | ? | `07_DATADOME_PRIMITIVES.md`; [Kameleo 2025](https://kameleo.io/blog/guide-to-bypassing-datadome) |
| **AWS WAF Targeted** | partial (token includes some signals) | ? | ? | ? | ? | `06_AWS_WAF_SOLVER.md §3`; [AWS Bot Control](https://docs.aws.amazon.com/waf/latest/developerguide/waf-bot-control.html) |
| **Cloudflare Managed** | ? (heavily obfuscated; assumed yes) | ? | ? | ? | ? | `25_CLOUDFLARE_DEEP.md` |
| **Cloudflare Turnstile** | ✓ (cursor velocity + click pattern per [CaptchaAI Turnstile](https://blog.captchaai.com/how-cloudflare-turnstile-works)) | partial (in-widget only) | ? | ? | ? | `25_CLOUDFLARE_DEEP.md §Turnstile` |
| **Imperva** | ✓ (180+ properties incl. mouse) | ? | ✓ | ? | ✓ | `18 §2.7` |
| **Castle** | ✓ ([Castle 2025 blog](https://blog.castle.io/bot-detection-101-how-to-detect-bots-in-2025-2/)) | ? | ? | ? | ✓ | `18 §2.17` |
| **wbaas (Walmart)** | ? | ? | ? | ? | ? | `18 §2.12` |

The high-value cells (✓ in multiple columns):
- **Mouse curve**: every behavioral vendor scores it
- **Mouse pre-pop buffer**: Akamai + DataDome + Kasada + PerimeterX +
  Radware + Imperva — 6 vendors, all gated on "is the buffer non-empty
  at first sensor read"
- **Touch (mobile)**: 5 vendors on the mobile path; BO's iPhone +
  Pixel profiles are exposed here

---

## 4. BO behavioral coverage

### 4.1 What `humanize.js` does

Inventory, top to bottom of the file (404 lines total):

| Block | Lines | What it produces |
|---|---|---|
| Module doc + theory | 1-39 | Plamondon-citation header explaining Σ-Λ |
| Body lookup + `_sched` selector | 40-52 | Acquires `globalThis.__bgSetTimeout` (unref'd) or fallback to `setTimeout` |
| `__akamai_events` buffer + recorders | 54-92 | `mouse[]`, `key[]`, `touch[]`, `scroll[]` arrays + per-event counters; recorders `_akRecMouse`, `_akRecKey`, `_akRecScroll` (NOT `_akRecTouch`) |
| `_dispatch` helper | 96-100 | Sets `isTrusted: true` via `Object.defineProperty`, then `target.dispatchEvent` |
| `_gauss` (Box-Muller) | 104-109 | Standard normal sampler |
| `_lerp` (linear interp) | 112-114 | 2D point interpolation |
| `_sigmaLognormalTimes` | 121-136 | n normalized sample times in [0,1] with lognormal density |
| `_normalQuantile` (Beasley-Springer-Moro) | 142-172 | Inverse-normal-CDF approximation for Σ-Λ time sampling |
| `_fireMove` | 181-217 | `mousemove` + `pointermove` paired emission on window+document+body, with `__akamai_events.mouse` recording |
| `_fireScrollStep` | 220-234 | `wheel` event + actual `scrollBy` + `scroll` event on document and window |
| `runCycle` | 238-282 | Focus + visibilitychange + 2-stroke mouse motion (15 samples/stroke, 800-1100 ms/stroke, 50-150 ms micropause) + 2-step scroll-down |
| Synchronous pre-pop IIFE | 303-398 | Calls `op_behavior_mouse_trajectory`, drops 14 historical points (t ∈ [−1800, −100] ms) into `__akamai_events.mouse`, fires one synchronous mousemove+pointermove pair |
| Execution kickoff | 400-403 | Runs first cycle immediately + `setInterval(runCycle, 4000)` for the "keep-alive" pattern |

The **load-bearing pieces** (in terms of which vendors they unblock):
- W6a synchronous pre-population (`303-398`) — defeats DataDome's
  empty-coord-list heuristic; primary driver of the DataDome cluster
  pass rate
- `_fireMove` paired mousemove + pointermove (`181-217`) — required
  by DataDome `tags.js` and Akamai BMP modern sensor (both listen on
  PointerEvent in addition to legacy mousemove)
- `__akamai_events` buffer presence (`54-92`) — required by any
  private vendor solver that needs the in-V8 event log

The **`isTrusted: true` defineProperty** at line 97 is per-vendor
critical: real Chrome's user-input-originated events have
`isTrusted === true`, while events constructed via `new MouseEvent(...)`
have `isTrusted === false`. Vendor handlers gate on `e.isTrusted` to
distinguish "real user clicked" from "page script dispatched". Without
the defineProperty, every synthesized event would be invisible to
gating handlers.

### 4.2 What `humanize.js` doesn't

Per the gap table in §3.1, §3.2, §3.3, §3.4:

- **Multi-stroke Σ-Λ on the live cycle**: only the pre-pop is full Σ-Λ;
  the live cycle is linear-anchor-to-anchor with Σ-Λ-distributed
  sample times. Velocity profile is approximately correct; positional
  shape is straighter than real human motion.
- **Smoothstep terminal decel on the live cycle**: missing — each
  anchor terminus has an impulse spike in the 2nd derivative of
  position.
- **Fitts's-Law-derived per-stroke duration**: fixed 800-1100 ms
  regardless of target.
- **Keystroke events**: zero. `_akRecKey` is defined but never called.
- **Touch events**: zero. No `_akRecTouch` function; `touch` buffer
  always empty.
- **`requestIdleCallback`-style scheduling**: not used; `setInterval`
  fires `runCycle` every 4000 ms regardless of page state.
- **Per-session reproducible RNG**: `Math.random()` is reseeded per
  V8 isolate (deno_core default), so two pool-reused pages do see
  different patterns; but the per-page seed is **not derived from a
  `BehaviorProfile.seed`**, so the same user across multiple sessions
  doesn't show the per-user σ consistency that real keystroke
  biometrics use as authentication. Action item §5 + §8.

---

## 5. The fundamental challenge — variance vs randomness

The defender wins when the prosecution can show **statistically
significant divergence from a Chrome distribution**. This includes
**under-dispersion**: a synthesizer that produces "perfectly human"
mouse curves across every visit is detectable by the *absence of
session-to-session variance* in its σ/μ parameters. Real users have:

- **Per-session variance**: mood, fatigue, time of day, recent
  caffeine. A user's Σ-Λ σ on Monday morning is different from their
  σ on Friday afternoon.
- **Per-page variance**: familiarity with site (longer dwell on
  unfamiliar UI), task complexity (faster on routine clicks).
- **Per-action variance**: mouse vs trackpad vs trackpoint;
  primary-hand vs offhand for cross-screen movement.

A bot that uses a **per-page randomly-seeded RNG** (BO's current
approach via `Math.random()`) produces the **opposite anti-pattern
from the natural one**: each session looks like a different user. A
defender that tracks visitor cohorts (per-cookie or per-IP) across
multiple pages will see "this cookie's behavioral fingerprint shifts
every page load" — a strong bot signal because real users are
*biometrically consistent*.

The correct architecture is **two-level seeding**:

1. **Per-session seed** — drawn once per visitor session, used to
   parameterize the `BehaviorProfile` (σ_dwell, σ_mouse, fitts_b,
   typing_wpm_mean). Stable across all pages in the session.
2. **Per-call salt** — folded with the session seed for each
   individual mouse path / keystroke string. Different (from, to)
   pairs produce different sequences; same (from, to) under the same
   session reproduces.

`crates/stealth/src/behavior.rs:109-115` already implements the
two-level scheme:

```rust
pub fn rng_for(&self, salt: u64) -> ChaCha20Rng {
    let combined = self.seed
        .wrapping_mul(0x9E3779B97F4A7C15)
        .wrapping_add(salt);
    ChaCha20Rng::seed_from_u64(combined)
}
```

But `humanize.js` does **not consume** a `BehaviorProfile.seed`. It
calls `op_behavior_mouse_trajectory` for the pre-pop (one shot, with
no session-state output), and uses `Math.random()` for everything
else. The pre-pop trajectory's σ/μ parameters are not exposed to JS,
so the live cycle cannot continue the same "user" — it just starts
fresh.

**This is the single most important behavioral gap for v0.2.0** if
the corpus expands to multi-page sessions per visitor. For v0.1.0 (one
page per navigate, no session continuity), the gap is invisible —
each navigate is a "new user" by design, and the defender has no
multi-page baseline to compare against.

The opposite failure mode (over-dispersion) is BO's current shape: per
page, the mouse curve looks human; across N pages by the same
"visitor", the curves look like N different users. Symmetric
detectability — the defender just looks for the wrong axis.

---

## 6. Cross-category leverage

If you can ship one behavioral fix in v0.1.0, which gives the biggest
cross-vendor win? Ranking by (vendor count × site count in corpus ×
implementation cost):

| Fix | Vendors helped | Estimated corpus impact | Cost | Priority |
|---|---|---|---|---|
| **Wire `BehaviorProfile.seed` through `humanize.js`** (per-session RNG) | All behavioral vendors | 0 sites in v0.1.0 corpus (single-page); 5-10 in expanded v0.2.0 corpus | 1 op, ~30 LOC | **P2** (v0.2.0 prerequisite) |
| **Live-cycle full Σ-Λ + smoothstep tail** (port `behavior.rs` to JS or expose via op) | Akamai BMP, Kasada, Radware, F5 | 1-2 sites (homedepot if Akamai sec-cpt residual is behavioral; uncertain) | ~150 LOC | **P2** |
| **Touch events on mobile profiles** | Akamai mobile, PerimeterX mobile, F5 mobile, Castle | 0 sites in current corpus; significant for any future mobile-app-protected site | ~100 LOC | **P2** |
| **Keystroke synthesis wiring** | F5, Akamai login, Imperva forms | 0 sites in v0.1.0 (no form-submit gating); critical for v0.2.0 if corpus expands | wiring + 1 op, ~80 LOC | **P3** |
| **RAF jitter** (~1-3 ms stddev around 16 ms) | Kasada (the one vendor that demonstrably scores RAF cadence) | 0-1 sites (Kasada cluster) | ~10 LOC | **P3** |
| **setTimeout nesting-level clamp** (HTML spec compliance) | Spec-correctness; any vendor that runs a chained-setTimeout probe | 0-1 sites (Kasada cluster); also fixes a real footgun where chained `setTimeout(0)` runs faster than Chrome | ~20 LOC | **P2** |
| **Hidden-tab visibility throttling** | (no observed gating) | 0 sites in corpus | ~50 LOC | **P3** |

The leverage pattern: **mouse-curve quality scales across vendors;
keystroke and touch each unlock only their narrow gating use case**.
This shapes the §8 acceptance bar — measure mouse first, only target
keystroke/touch if a specific site demands it.

The Kasada-specific items (RAF jitter, setTimeout nesting) are bundled
into `08_KASADA_FRONTIER.md` §4; do not double-count in v0.1.0
acceptance.

---

## 7. Forward-looking — where this is going

Three trends visible from the 2025-2026 vendor literature:

1. **ML detection improves faster than ML synthesis.** A GAN-trained
   mouse-trajectory generator costs ~$10K of GPU time to train; the
   classifier that scores it costs the vendor ~$100 of training. The
   defender's training budget scales with their entire customer base
   (Akamai, DataDome, F5 all monetize hundreds of customers and can
   re-train monthly); the attacker's budget is per-engine and gets
   ammortized only if the engine sees wide deployment. **Expect the
   gap to widen**, not close, over the v0.1.0 → v1.0.0 horizon.

2. **WebNN / on-device ML for fingerprint generation.** Chrome's
   [WebNN API](https://www.w3.org/TR/webnn/) gives JS access to ML
   primitives. Vendors will increasingly run the bot-classification
   model **inside the page JavaScript**, scoring locally and submitting
   a verdict (not raw signals). This is harder to bypass because the
   model weights live in obfuscated JS and the verdict is opaque; the
   only known attack is to capture and replay the verdict, which fails
   on per-visitor signing.

3. **Convergence on hardware attestation.** Apple's [Private Access
   Tokens (RFC 9577)](https://datatracker.ietf.org/doc/rfc9577/) and
   Google's [Web Environment Integrity](https://github.com/RupertBenWiser/Web-Environment-Integrity)
   proposals route around behavioral entirely: the device cryptographically
   attests "I'm a real iPhone running real Safari" and the server skips
   the behavioral pass. If these ship, the **behavioral arms race
   becomes irrelevant** — but only for users on attesting hardware,
   leaving headless engines in the same place they were before.

The honest read for BO: **sites without behavioral gating remain
reliably scrapeable; sites with strong behavioral gating will become
harder over time, not easier.** v0.1.0's 110-routed pass number is a
high-water mark for the current vendor landscape; a 2027 re-measure
will show some sites that pass today flipping to blocked as vendors
upgrade their distribution probes.

---

## 8. Acceptance for v0.1.0

Bar set in `01_CURRENT_STATE.md` + `12_COMPETITIVE_LANDSCAPE.md` is
"meet or exceed Camoufox routed-pass-count on the 126 corpus, with
honest measurement". The timing/behavioral chapter contributes the
following gates:

- [ ] **Clock distribution shape verified.** Run the
  `perf_now_hot_loop_produces_distinct_values` test
  (`crates/browser/tests/chrome_compat.rs:3298`) in CI; assert > 10
  distinct values in 500 iterations (current threshold). Optional
  v0.1.0 stretch: extend to compare moments (mean, stddev, skew) of
  diffs against a captured Chrome reference distribution within ±20%
  tolerance.
- [ ] **`performance.now()` monotonicity.** Already enforced by
  `perf_ext.rs:73` (`candidate.max(self.last_us)`). The
  `perf_now_is_strictly_monotonic` test
  (`crates/browser/tests/chrome_compat.rs:3315`) covers this.
  No action.
- [ ] **`performance.timeOrigin` consistency.** Add a regression test
  that asserts `performance.timeOrigin + performance.now()` is within
  ±5 ms of `Date.now()` (matches real Chrome's ms-level drift). This
  closes the Kasada-class skew probe. **Action: file follow-up; ~10
  LOC test + verify `timeOrigin` is wired to `op_perf_now_humanized`'s
  origin.**
- [ ] **`setTimeout` nesting-level clamp.** Implement the spec rule
  (after 5 levels, clamp to 4 ms) in `timer_bootstrap.js`. Regression
  test: nested `setTimeout(fn, 0)` chain of 10; assert calls 6-10 each
  take ≥ 3.5 ms wall time. **Action: file follow-up; ~20 LOC.**
- [ ] **`humanize.js` mouse curve characterized.** Output the
  per-cycle mouse-event stream to a file; run `behavior.rs`'s
  `mouse_trajectory_has_velocity_diversity_not_uniform` test
  (`behavior.rs:620`) on the dump; assert variance is non-zero.
  Optional: feed the dump to the BeCAPTCHA-Mouse classifier from
  [BiDAlab/BeCAPTCHA-Mouse](https://github.com/BiDAlab/BeCAPTCHA-Mouse)
  and measure the bot-probability. **Action: add an example binary
  `crates/browser/examples/dump_humanize_stream.rs`; ~40 LOC.**
- [ ] **Top-3 behavioral fixes for cross-vendor leverage identified.**
  Per §6 ranking: (1) live-cycle full Σ-Λ + smoothstep tail; (2)
  per-session seeding; (3) setTimeout nesting clamp. None of the three
  are v0.1.0 blockers (the 110-routed number was measured *without*
  them); all are v0.2.0 prerequisites.
- [ ] **Honest "what's hard to fix" section landed in
  `15_OPEN_QUESTIONS.md`.** Already covered by the Kasada open-blocker
  entry; cross-link this chapter from there. **Action: add a one-line
  pointer.**

What this chapter explicitly does NOT promise:
- **Per-session reproducibility wiring**: deferred to v0.2.0 — see §5.
- **Touch events on mobile profiles**: deferred to v0.2.0 — see §3.4.
- **Keystroke synthesis wiring**: deferred to v0.2.0 — see §3.2.
- **Hidden-tab `visibilityState` throttling**: deferred to v0.2.0 —
  see §2.4.
- **Beating BeCAPTCHA-Mouse below 50%**: out of scope for any version
  of BO. We target "indistinguishable enough to defeat the vendor's
  cheaper checks first"; sites that escalate to BeCAPTCHA-grade
  classifiers are the v1.0+ research domain.

---

## 9. Files referenced

### Engine — JS bootstrap
| Path | Lines | What |
|---|--:|---|
| `crates/browser/src/js/humanize.js` | 1-404 | The whole humanizer; see §4.1 for line-block breakdown |
| `crates/js_runtime/src/js/timer_bootstrap.js` | 1-191 | `setTimeout`/`setInterval`/`clearTimeout`/`requestAnimationFrame`/`performance.now` fallback + `__bgSetTimeout`/`__cancelAllTimers` engine helpers |
| `crates/js_runtime/src/js/timer_bootstrap.js` | 62-79 | `setTimeout` — no nesting-level clamp (gap, §2.4) |
| `crates/js_runtime/src/js/timer_bootstrap.js` | 91-107 | `__bgSetTimeout` — unref'd helper for engine-internal scripts (§2.7) |
| `crates/js_runtime/src/js/timer_bootstrap.js` | 109-133 | `setInterval` — 4-ms unconditional floor (partial-wrong, §2.4) |
| `crates/js_runtime/src/js/timer_bootstrap.js` | 147-160 | `requestAnimationFrame` — constant 16 ms, no jitter (gap, §2.3) |
| `crates/js_runtime/src/js/timer_bootstrap.js` | 170-173 | `performance.now()` fallback (`Date.now() - startTime`); defensive, never reached |
| `crates/js_runtime/src/js/window_bootstrap.js` | 5419-5435 | Installs humanized `performance.now()` on `Performance.prototype` |
| `crates/js_runtime/src/js/window_bootstrap.js` | 2902 | `performance.mark` |
| `crates/js_runtime/src/js/window_bootstrap.js` | 2183-2202 | `PerformanceObserver` stub (gap, §2.6) |
| `crates/js_runtime/src/js/worker_bootstrap.js` | 131-138 | Worker-scope `performance.now` — same op as window |
| `crates/js_runtime/src/js/worker_bootstrap.js` | 140-153 | Worker-scope `performance.memory` jitter |

### Engine — Rust extensions
| Path | Lines | What |
|---|--:|---|
| `crates/js_runtime/src/extensions/perf_ext.rs` | 1-204 | Humanized `performance.now` op + resource timings op |
| `crates/js_runtime/src/extensions/perf_ext.rs` | 27-76 | `PerfState` — origin `Instant`, `StdRng`, `LogNormal(ln 8 µs, 0.4)`, `Exp(1/200 µs)`, `last_us` monotonic-clamp |
| `crates/js_runtime/src/extensions/perf_ext.rs` | 84-87 | `op_perf_now_humanized` op definition |
| `crates/js_runtime/src/extensions/perf_ext.rs` | 146-202 | Regression tests: distinct-value cardinality, jitter bounds, deterministic-with-seed, heavy-tail spike presence |
| `crates/js_runtime/src/extensions/timer_ext.rs` | (full) | `op_set_timeout` + `op_timer_sleep` + `op_clear_timer` (the Rust side of `timer_bootstrap.js`) |

### Engine — behavior generators
| Path | Lines | What |
|---|--:|---|
| `crates/stealth/src/behavior.rs` | 1-700 | Mouse trajectory + keystroke timing + scroll burst generators |
| `crates/stealth/src/behavior.rs` | 23-103 | `BehaviorProfile` struct (seed, handedness, mouse_dpi, typing_wpm, scroll_style, fitts_b) |
| `crates/stealth/src/behavior.rs` | 109-115 | `BehaviorProfile.rng_for(salt)` — two-level deterministic ChaCha20Rng |
| `crates/stealth/src/behavior.rs` | 142-287 | `mouse_trajectory` / `mouse_trajectory_with_rng` — Σ-Λ multi-stroke + smoothstep terminal decel + pink tremor |
| `crates/stealth/src/behavior.rs` | 289-345 | `Stroke` struct + `integrate_x` / `integrate_y` (closed-form Σ-Λ CDF) + `erf` (Abramowitz-Stegun 7.1.26) |
| `crates/stealth/src/behavior.rs` | 381-419 | `bigram_ratio` — alt-hand / same-finger / same-key flight modifiers |
| `crates/stealth/src/behavior.rs` | 421-464 | `keystroke_timings` — LogNormal dwell + bigram-modulated flight (NOT wired into humanize.js, §3.2) |
| `crates/stealth/src/behavior.rs` | 470+ | `WheelTick` + scroll-burst generator (NOT wired into humanize.js, §3.3) |
| `crates/stealth/src/behavior.rs` | 583+ | Mouse-trajectory tests (Fitts's-Law total time, 8 ms sample rate, velocity diversity, determinism per seed) |
| `crates/stealth/src/BIGRAM_PROVENANCE.md` | (full) | CMU + Buffalo dataset provenance for the bigram-flight ratios |

### Engine — navigation hooks
| Path | Lines | What |
|---|--:|---|
| `crates/browser/src/page.rs` | (search `__akamai_events`) | The DRAIN_JS-equivalent JS surface that `humanize.js` populates; consumed by any private vendor solver |

### Test surfaces
| Path | Lines | What |
|---|--:|---|
| `crates/js_runtime/src/extensions/perf_ext.rs` | 146-162 | `distribution_has_distinct_jitter_values` — >10 distinct in 500-iter loop |
| `crates/js_runtime/src/extensions/perf_ext.rs` | 164-171 | `jitter_is_bounded_and_non_negative` — clamp invariant |
| `crates/js_runtime/src/extensions/perf_ext.rs` | 173-180 | `deterministic_across_runs_with_same_seed` |
| `crates/js_runtime/src/extensions/perf_ext.rs` | 182-203 | `occasional_heavy_tail_spikes` — Bernoulli(1/1024) Exp tail asserted over 100k samples |
| `crates/browser/tests/chrome_compat.rs` | 3276-3322 | `perf_now_returns_number / is_native / is_finite_non_negative / hot_loop_produces_distinct_values / is_strictly_monotonic` |
| `crates/browser/tests/chrome_deep.rs` | 408+ | `performance_now_monotonic` |
| `crates/browser/tests/anti_bot.rs` | 170+ | `performance_now_is_number` |

### Sibling chapters (cross-link map)
| Chapter | Why |
|---|---|
| `16_STEALTH_FINGERPRINT_AUDIT.md` | Static identity sibling — read alongside |
| `17_WEB_API_PARITY_MATRIX.md §2.8` | Performance API surface parity |
| `13_FILE_LOCATIONS_INDEX.md` | One-page lookup of every file:line above |
| `18_ANTI_BOT_VENDOR_COOKBOOK.md` | Per-vendor cookies/headers (orthogonal axis) |
| `06_AWS_WAF_SOLVER.md §3` | AWS WAF timing/token consumption |
| `07_DATADOME_PRIMITIVES.md` | DataDome `tags.js` mouse buffer + `_initialCoordsList` |
| `08_KASADA_FRONTIER.md §3, §4` | Kasada V3 envelope timing + RAF cadence scoring |
| `25_CLOUDFLARE_DEEP.md` | Cloudflare + Turnstile cursor analysis |
| `26_AKAMAI_BMP_DEEP.md §3, §4` | Akamai sensor_data mouse/key/scroll/touch fields |
| `24_RISK_REGISTER.md` | `__bgSetTimeout` cleanup ordering risk |
| `15_OPEN_QUESTIONS.md` | "What's hard to fix" parking lot |

### External references (specs)
| URL | Cite for |
|---|---|
| [W3C High Resolution Time](https://www.w3.org/TR/hr-time/) | §2.1 spec baseline, monotonicity, jitter mitigations |
| [W3C Performance Timeline](https://www.w3.org/TR/performance-timeline/) | §2.6 `PerformanceObserver` buffer semantics |
| [HTML Living Standard — Timers](https://html.spec.whatwg.org/multipage/timers-and-user-prompts.html) | §2.4 setTimeout nesting clamp |
| [Chrome 91 cross-origin isolated timers](https://developer.chrome.com/blog/cross-origin-isolated-hr-timers) | §2.1 Chrome resolution numbers |
| [Chrome 88 timer throttling](https://developer.chrome.com/blog/timer-throttling-in-chrome-88) | §2.3, §2.4 background-tab clamp |
| [V8 "A year with Spectre"](https://v8.dev/blog/spectre) | §2.1 Spectre rationale |
| [MDN performance.now()](https://developer.mozilla.org/en-US/docs/Web/API/Performance/now) | §2.2 monotonic vs Date.now |

### External references (research)
| URL | Cite for |
|---|---|
| [Plamondon 1995 / BeCAPTCHA-Mouse arXiv 2005.00890](https://arxiv.org/pdf/2005.00890) | §3.1 Σ-Λ kinematic theory; classifier accuracy numbers |
| [BiDAlab/BeCAPTCHA-Mouse benchmark](https://github.com/BiDAlab/BeCAPTCHA-Mouse) | §8 acceptance — measure humanize.js against this |
| [BeCAPTCHA mobile arXiv 2005.13655](https://arxiv.org/pdf/2005.13655) | §3.4 touch / swipe biometrics |
| [CMU Keystroke benchmark](https://www.cs.cmu.edu/~keystroke/) | §3.2 dwell/flight distributions |
| [Buffalo CUBS dataset](https://www.buffalo.edu/cubs/research/datasets.html) | §3.2 dwell/flight distributions |
| [vmonaco — Peripheral Timestamps](https://vmonaco.com/papers/Device%20Fingerprinting%20with%20Peripheral%20Timestamps.pdf) | §2.1 clock-skew device fingerprinting |
| [Schwarz — JavaScript Zero](https://www.ndss-symposium.org/wp-content/uploads/2018/02/ndss2018_07A-2_Schwarz_paper.pdf) | §2.1 jitter recovery via averaging |

### External references (vendors / industry)
| URL | Cite for |
|---|---|
| [AWS WAF JS challenge API](https://docs.aws.amazon.com/waf/latest/developerguide/waf-js-challenge-api.html) | §2.5 AWS WAF timing-signal consumption |
| [AWS WAF Bot Control](https://docs.aws.amazon.com/waf/latest/developerguide/waf-bot-control.html) | §3.5 AWS WAF behavioral signals |
| [Akamai BMP SDK docs](https://developer.akamai.com/tools/sdk/bot-manager) | §2.5, §3.5 BMP signal taxonomy |
| [Akamai 2.0 decoded — xiaoweigege Medium](https://medium.com/@240942649/decoding-akamai-2-0-418e7c7fa0a0) | §3.5 sensor_data field inventory |
| [Scrapfly Akamai bypass](https://scrapfly.io/bypass/akamai) | §3.5 BMP behavioral signal overview |
| [Cloudflare Turnstile breakdown — CaptchaAI](https://blog.captchaai.com/how-cloudflare-turnstile-works) | §3.5 Turnstile cursor + click analysis |
| [DataDome bypass — Kameleo 2025](https://kameleo.io/blog/guide-to-bypassing-datadome) | §2.5, §3.5 DataDome behavioral analysis |
| [PerimeterX bypass 2026 — Scrapfly](https://scrapfly.io/blog/posts/how-to-bypass-perimeterx-human-anti-scraping) | §3.5 PerimeterX Press-and-Hold pressure curve |
| [PerimeterX 2026 — TheDataScientist](https://thedatascientist.com/how-to-bypass-perimeterx-human-press-hold-challenges-in-2026-the-ultimate-cdp-and-ai-guide/) | §3.5 300+ behavioral signals |
| [F5 Shape — DevCentral](https://community.f5.com/kb/technicalarticles/what-is-shape-security/284359) | §2.5, §3.5 Shape signal collection |
| [Radware IDBA whitepaper](https://www.radwarebotmanager.com/web/wp-content/uploads/IDBA_WP.pdf) | §2.5, §3.5 250+ params, intent encoding |
| [Radware IDBA cyberpedia](https://www.radware.com/cyberpedia/bot-management/intent-based-behavioral-analysis/) | §3.5 |
| [Castle bot detection 2025](https://blog.castle.io/bot-detection-101-how-to-detect-bots-in-2025-2/) | §3.5 mobile touch signals |
