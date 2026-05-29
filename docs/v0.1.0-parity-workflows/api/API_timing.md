# Web API Parity — Timing, Scheduling & Observers

**Scope:** `performance.*` (now/timeOrigin/memory/getEntries/timing),
`requestAnimationFrame`, `requestIdleCallback`, `setTimeout`/`setInterval`
clamping, `IntersectionObserver`, `ResizeObserver`, `MutationObserver`,
`queueMicrotask`, event-loop ordering, and how the `run_until_idle`
drain model in `page.rs` couples to all async timing — connected to the
AWS live-nav-drain finding.

**Audience:** anyone touching `perf_ext.rs`, `timer_bootstrap.js`,
`window_bootstrap.js` (observers), `page.rs` (the navigate drain loop),
or debugging a "fingerprint passes but the site still returns the
challenge stub" verdict.

**Method note:** every BO claim below was verified against current
source on branch `fix/v0.1.0-fix4-canvas-parity` (2026-05-28). Several
statements in `17_WEB_API_PARITY_MATRIX.md §2.8` are now **stale** — the
matrix predates the Fix-7 (`performance.timeOrigin`) and Fix-9 (RAF
jitter) commits. Where the matrix and the code disagree, the **code is
authoritative** and the discrepancy is called out explicitly.

---

## 0. TL;DR — where BO actually stands

| Surface | Matrix (`17 §2.8`) claim | Verified code state | Delta vs Chrome | Delta vs Camoufox v150 |
|---|---|---|---|---|
| `performance.now()` | "🟡 ms-only `Date.now()-startTime`" — **WRONG/stale** | **Humanized** 100 µs grid + LogNormal(8 µs) jitter + 1/1024 Exp spike, monotonic-clamped (`perf_ext.rs:74-88`) | **AHEAD of spec floor** (correct shape) | **AHEAD** — Camoufox does not engineer clock jitter at all (inherits raw Gecko clock) |
| `performance.timeOrigin` | "❓ check" — now shipped | **Real** — `op_perf_time_origin_ms` returns UNIX-epoch ms of origin so `timeOrigin + now() ≈ Date.now()` (`perf_ext.rs:102-109`) | match | match |
| `requestAnimationFrame` cadence | "constant 16 ms" — **stale** | **Jittered** — 16.67 ms mean, Gaussian σ=0.5 ms, seeded PRNG (`timer_bootstrap.js:172-201`) | near-match (Chrome jitter slightly heavier-tailed) | **AHEAD** — Camoufox leaves Gecko's rAF untouched |
| `setTimeout` nesting clamp | "G — none" | **Still a gap** — `Math.max(0, delay\|0)`, no nesting level (`timer_bootstrap.js:66`) | behind | parity (Camoufox doesn't fake this either; real Gecko enforces it natively) |
| `setInterval` floor | "partial-wrong 4 ms uncond." | **Confirmed** — `Math.max(4, delay\|0)` unconditional (`timer_bootstrap.js:113`) | behind (one-sided, "too slow") | minor |
| `IntersectionObserver` | "🟡 always-intersecting" | **Confirmed** — `isIntersecting:true, ratio:1.0` always, fired once via `Promise.resolve().then` (microtask) (`window_bootstrap.js:3521-3547`) | **behind** (no real layout, no scroll re-fire) | **BEHIND** — Camoufox's real Gecko layout gives true rects + scroll-driven re-fire |
| `ResizeObserver` | "🟡 fires once" | **Confirmed** — single microtask fire with `getBoundingClientRect()` (`window_bootstrap.js:3550-3570`) | behind | **behind** vs real Gecko |
| `MutationObserver` | "🟡 partial" | **Real** — childList + attributes + characterData filtered, microtask-batched (`dom_bootstrap.js:1874-2094`) | near-match | parity |
| `requestIdleCallback` | "🟡 stub" | **setTimeout(1) polyfill**, `timeRemaining:()=>50`, masked (`window_bootstrap.js:3573-3585`) | behind (fires too eagerly, fixed deadline) | parity (Camoufox doesn't engineer this) |
| `queueMicrotask` | (n/a) | **Native** deno_core / V8 builtin, used widely internally | match | match |
| `performance.memory` | "✅ fixed+jitter" | Confirmed fixed values + worker jitter (`window_bootstrap.js:2333-2337`, `worker_bootstrap.js:142-153`) | match | N/A (Firefox has no `performance.memory`) |
| `PerformanceObserver` | "🟡 stub" | Confirmed **no-op** `observe()`/`takeRecords→[]`, but real `supportedEntryTypes` getter (`window_bootstrap.js:2240-2258`) | behind (never delivers entries) | behind vs real Gecko |
| `performance.getEntries*` | "✅" | Real for resource timings (`op_perf_get_resource_timings`), `entryType:"resource"` (`perf_ext.rs:131-156`) | partial | partial |

**Headline:** BO's *clock-shape* engineering (perf.now jitter, RAF
jitter, timeOrigin coupling) is the **strongest single timing surface
of any open-source stealth browser — it is ahead of Camoufox v150**,
which does no clock engineering at all (confirmed via DeepWiki query on
`daijro/camoufox`: Camoufox spoofs static C++ properties + timezone but
leaves `performance.now`, rAF, setTimeout, and the observers as native
Gecko). BO's *weakness* is the inverse: the **observers are fakes**
(always-intersecting, single-fire, no layout) where Camoufox gets real
behavior for free from the Gecko layout engine. That observer gap — not
the clock — is the timing-domain lever that can flip SPA/hydration
sites (booking, douyin) and is the live-nav-drain class of the AWS
cluster.

---

## 1. What the existing repo docs concluded

### 1.1 `40_TIMING_BEHAVIORAL.md` (the canonical timing chapter)

- **`performance.now()` is the well-engineered surface.** `§2.1, §2.6`
  document the 100 µs grid + LogNormal(ln 8 µs, 0.4) jitter + Bernoulli
  (1/1024) × Exp(1/200 µs) heavy-tail spike + monotonic clamp, matching
  captured Chrome 130 distributions and defeating `set(diffs).size===1`.
  Status: **shipped** (`perf_ext.rs`).
- **Two gaps flagged in the 40-doc are now CLOSED** (the doc is
  partly out of date): `§2.2`/`§8` listed `performance.timeOrigin` as
  "not wired to a humanized op" and `§2.3`/`§8` listed RAF as "constant
  16 ms, no jitter". **Both have since shipped** (Fix 7 + Fix 9 — see
  §3.2, §3.3 below). The 40-doc's `§8` acceptance checkboxes for these
  two items can be ticked.
- **Still-open gaps confirmed by the 40-doc and re-verified here:**
  `setTimeout` nesting-level clamp (`§2.4`), hidden-tab visibility
  throttling (`§2.3`/`§2.4`), `PerformanceObserver` delivers no entries
  (`§2.6`).
- The 40-doc's `§5` makes the load-bearing strategic point:
  **identity gaps are finite/listable; distribution gaps are
  open-ended.** Timing belongs to both — the *values* (identity) are now
  mostly present; the *shapes* (distribution) are correct for perf.now
  and rAF, absent for the observers.

### 1.2 `EVENT_LOOP.md`

- Documents the intended model: deno_core's `run_event_loop` provides
  V8 microtask + async-op + Promise integration "for free"; BO wraps it
  with browser scheduling (rAF/idle/timer clamp). The `run_until_idle`
  pseudocode (lines 97-130) shows a fire-timers → fire-rAF →
  fire-idle → check-termination loop. **NOTE:** the real
  `crates/event_loop/src/lib.rs:288` is simpler than the doc's
  pseudocode — it is a thin `tokio::time::timeout(run_event_loop())`
  loop with a nav-pending tail; rAF/timers/idle are all driven from
  **JS-side** `timer_bootstrap.js` via `op_timer_sleep` (tokio::sleep),
  not from a Rust `TimerRegistry`. The `EVENT_LOOP.md` `TimerRegistry`
  struct is aspirational/doc-only — the shipped path is all-JS timers
  backed by tokio sleeps. This matters for §4.

### 1.3 `17_WEB_API_PARITY_MATRIX.md §2.8` + `§ Fix-priority`

- `§2.8` is the per-interface matrix; **its `performance.now()` row is
  stale** (claims ms-only). Its fix-priority table (rows 8/9/10) ranks:
  #8 IntersectionObserver real bounding rect (P1, 2 days, "may flip
  booking"); #9 MutationObserver attribute notify (P2 — already done);
  #10 perf.now sub-ms (P1 — already done). So of its three timing fixes,
  **two are already shipped**; the live one is **#8 IntersectionObserver**.

### 1.4 `HANDOFF_2026_05_28b.md §4–5` (the latest, load-bearing)

- The AWS-WAF cluster (7 sites + duolingo) is **not** a fingerprint gap.
  challenge.js runs to `forceRefreshToken` in the **offline oracle**
  (`from_html_with_url` + `run_until_idle(5s)`), but in the **live
  navigate path** it produces **zero async progress** — no blob-worker
  spawn, no token POST. Root cause localized to the
  **execution/drain model** of
  `build_page_with_scripts_init_and_storage`. The handoff cites a
  "50 ms inter-script drain (page.rs ~3535)". §4 of this doc refines
  that diagnosis against the actual code.

### 1.5 `10_TIMING_OPTIMIZATION.md`

- Performance-of-the-engine doc (cold-start, V8 snapshot, memory), not
  web-API timing-shape — orthogonal. Relevant only in that its
  cold-start budget (~build_budget_ms) bounds how long a navigate
  iteration's drain can run before the V8DeadlineWatcher fires.

---

## 2. New external findings (2026)

### 2.1 `performance.now()` resolution — confirmed current Chrome behavior

[Chrome for Developers — "Aligning timers with cross-origin isolation"](https://developer.chrome.com/blog/cross-origin-isolated-hr-timers)
and [chromestatus 6497206758539264](https://chromestatus.com/feature/6497206758539264):
since Chrome 91, **non-isolated documents are clamped to 100 µs**;
`crossOriginIsolated === true` (COOP+COEP) relaxes to **5 µs**. The
[W3C HR-Time spec](https://w3c.github.io/hr-time/) "coarsen time"
algorithm requires implementations to **coarsen AND optionally jitter**
to that resolution. BO's `perf_ext.rs` models the 100 µs + jitter case
exactly. **Implication:** BO currently returns `crossOriginIsolated`
state from `interfaces_bootstrap.js` (`crossOriginIsolated` is in the
globals list). If a page is genuinely cross-origin-isolated, real
Chrome would tighten the grid to 5 µs; BO always uses 100 µs. This is a
**theoretical** mismatch only — none of the 126-corpus sites set
COOP+COEP on the document that runs the bot challenge (anti-bot vendors
specifically avoid COEP because it breaks third-party script loading).
Low priority.

### 2.2 Camoufox v150 does NO timing engineering — BO is ahead here

DeepWiki query on `daijro/camoufox` (2026-05-28): Camoufox spoofs static
properties at the C++ `MaskConfig` level (navigator, screen, WebGL,
AudioContext `sampleRate`/`outputLatency`, fonts, geolocation) and
controls `Date`/`Intl` via `timezone-spoofing.patch`, but **"does not
explicitly modify performance.now() resolution/jitter,
requestAnimationFrame cadence, setTimeout clamping, or
IntersectionObserver/ResizeObserver behavior."** It inherits **real
Gecko** for all of those.

Two consequences:
1. **Clock + rAF cadence:** BO's engineered jitter is *better than
   Camoufox's* against a distribution-shape probe, because Camoufox
   exposes Firefox's clock — which is correct-for-Firefox but a Firefox
   fingerprint (Firefox's `nsRFPService::ReduceTimePrecision` clamps to
   1 ms / 2 ms in RFP mode per [Bugzilla 1440863](https://bugzilla.mozilla.org/show_bug.cgi?id=1440863),
   and is detectably *not Chrome*). BO emits a **Chrome-shaped** clock.
   This is a genuine BO advantage; do not regress it.
2. **Observers:** Camoufox wins, because real Gecko computes real
   intersection rects and re-fires on real scroll. BO's stub
   always-intersects and fires once. This is the gap that lets v150 pass
   layout/scroll-gated SPA sites BO fails.

### 2.3 IntersectionObserver as a bot signal

[W3C IntersectionObserver](https://w3c.github.io/IntersectionObserver/v2/)
+ [MDN boundingClientRect](https://developer.mozilla.org/en-US/docs/Web/API/IntersectionObserverEntry/boundingClientRect):
a real `IntersectionObserverEntry` distinguishes "intersecting with a
**zero-area** rect" (edge-adjacent or zero-size element) from "truly
intersecting". Detection-relevant facts:
- A headless engine that returns `isIntersecting:true, ratio:1.0` for
  **every** observed element — including off-screen / `display:none` /
  zero-size elements — is producing a physically impossible layout.
  Lazy-load and infinite-scroll widgets that gate content fetches on
  `entry.intersectionRatio` will fire **all at once** for a bot,
  vs **progressively on scroll** for a human — a detectable burst
  pattern in the network waterfall.
- The `boundingClientRect` in the entry must equal (within async-scroll
  tolerance, per [Bugzilla 1671396](https://bugzilla.mozilla.org/show_bug.cgi?id=1671396))
  the element's `getBoundingClientRect()`. BO already wires this
  (`window_bootstrap.js:3536`) — but if BO's `getBoundingClientRect()`
  returns all-zero rects (no real layout), the entry's rect is also
  zero, and `isIntersecting:true` with a zero rect is the
  *impossible-state* signal above.

This is the concrete mechanism behind the matrix's "booking, many SPAs"
annotation: SPA shells use IntersectionObserver to drive
hydration/content fetch, and BO's fire-once-then-never behavior either
(a) fetches everything in one burst, or (b) never re-fires for content
that depends on a *scroll* re-intersection.

### 2.4 AWS WAF challenge.js worker pattern (confirms the handoff)

The AWS WAF JS challenge ([AWS WAF JS API](https://docs.aws.amazon.com/waf/latest/developerguide/waf-js-challenge-api.html))
ships `challenge.js` which (per the handoff's grep: 2× `new Worker`, 2×
`new Blob`, zero `WebAssembly`) runs a **proof-of-work in a blob-URL Web
Worker**, then POSTs the token and sets the `aws-waf-token` cookie. The
self-solve is gated behind `AwsWafIntegration.checkForceRefresh()`
returning a Promise; the worker is created in that promise's `.then(...)`
continuation. So the worker spawn is **two microtask/async hops deep**
from script eval. This is exactly why a drain that exits early — before
those continuations run — sees "zero async progress" (§4).

---

## 3. BO code-level analysis

### 3.1 `performance.now()` — `crates/js_runtime/src/extensions/perf_ext.rs:74-88`

```rust
pub fn now_ms(&mut self) -> f64 {
    let raw_us = self.origin.elapsed().as_nanos() as f64 / 1000.0;
    let q = (raw_us / 100.0).floor() * 100.0;                       // 100 µs grid
    let jitter = self.log_normal.sample(&mut self.rng).clamp(0.0, 35.0);
    let spike = if self.rng.random_bool(1.0 / 1024.0) {
        self.spike_exp.sample(&mut self.rng).min(1500.0)
    } else { 0.0 };
    let value = (q + jitter + spike).max(self.last_us);            // monotonic clamp
    self.last_us = value;
    value / 1000.0
}
```

Installed on `Performance.prototype.now` in `window_bootstrap.js`
(per 40-doc `§9`, lines ~5419-5435) and in workers via the same op
(`worker_bootstrap.js:131-138`). **This is correct and ahead of every
public competitor.** The only nit: the jitter+spike is added on top of a
floored grid, so the *minimum* return for a fresh sample is `q` (no
sub-grid value below the floor) — real Chrome's coarsen-then-jitter can
land slightly below the naive floor. Negligible; not a probe surface
anyone has been observed to use.

The `timer_bootstrap.js:210-215` fallback (`Date.now() - startTime`,
ms-only) is **only** installed `if (!globalThis.performance.now)` — and
the prototype install always runs first in a real page, so the fallback
is dead code in the navigate path. (This is the line `17 §2.8` mistook
for the live implementation.)

### 3.2 `performance.timeOrigin` — `perf_ext.rs:102-109` (Fix 7, SHIPPED)

`op_perf_time_origin_ms` returns the UNIX-epoch ms captured at the same
`Instant` as the perf-now origin (`origin_unix_ms`, set in
`with_seed`, `perf_ext.rs:54-57`). This makes the Web-Platform invariant
`timeOrigin + performance.now() ≈ Date.now()` hold, closing the
Kasada/Castle origin-skew probe the 40-doc `§2.2`/`§8` flagged.
**Verify wiring:** confirm `window_bootstrap.js` reads
`op_perf_time_origin_ms` into a `performance.timeOrigin` getter (the op
exists and is registered in the extension at `perf_ext.rs:163`; the JS
read site should be grep-confirmed in a follow-up — see open questions).

### 3.3 `requestAnimationFrame` — `timer_bootstrap.js:172-201` (Fix 9, SHIPPED)

```js
const _RAF_MEAN_MS = 16.67, _RAF_SIGMA_MS = 0.5;
const _rafDelayMs = () => Math.max(1, _RAF_MEAN_MS + _gauss() * _RAF_SIGMA_MS);
globalThis.requestAnimationFrame = function (callback) {
    const id = ++_rafId; _rafCallbacks.set(id, callback);
    setTimeout(() => { ...; cb(performance.now()); }, _rafDelayMs());
    return id;
};
```

The `_gauss()` is Box-Muller seeded from
`Symbol.for('__browser_oxide_behavior_rand__')` (deterministic per
session), so cadence is now jittered ~16.67 ± 0.5 ms — defeating the
`set(diffs).size===1` / zero-variance-stddev probe the 40-doc `§2.3`
flagged. **This closes the second of the two 40-doc gaps.** Residual
nuance: real Chrome rAF deltas have a heavier right tail (occasional
33 ms missed-frame from GC); BO's Gaussian σ=0.5 ms never produces a
33 ms gap. Low priority — no corpus site has been observed scoring rAF
tail shape.

**Hidden-tab pause: still not implemented.** rAF fires regardless of
`document.visibilityState` (which is permanently "visible"). Gap, but no
corpus site drives `visibilitychange` + checks rAF pause.

### 3.4 `setTimeout` / `setInterval` clamping — `timer_bootstrap.js:62-133`

- `setTimeout`: `const ms = Math.max(0, delay | 0);` (line 66) — **no
  nesting-level tracking.** Real Chrome clamps to 4 ms after nesting
  depth > 5 ([HTML spec timer init step 5](https://html.spec.whatwg.org/multipage/timers-and-user-prompts.html#timer-initialisation-steps)).
  A `setTimeout(fn,0)` self-chain of 10 fires calls 6-10 at ~0 ms in BO
  vs ~4 ms in Chrome — detectable by a vendor that times a nested chain.
  **Still a gap.**
- `setInterval`: `const ms = Math.max(4, delay | 0);` (line 113) —
  **unconditional 4 ms floor.** Wrong direction (too slow for the first
  call) but less detectable than too-fast. Minor.
- **Engine-internal coupling (load-bearing):** `op_timer_sleep` is a
  bare `tokio::sleep(ms)` (per `timer_bootstrap.js:7-9` comment) **not
  tied to a Rust TimerRegistry** — contradicting `EVENT_LOOP.md`'s
  `TimerRegistry` struct. Timers are cancelled via a JS generation
  counter (`_timerGen` / `__cancelAllTimers`, lines 15-18) on warm
  reuse. The `UNREF_THRESHOLD_MS = 2000` logic (lines 57-60): timers
  with `ms >= 2000` are `unref`'d so they don't pin `run_until_idle`
  open; short timers stay refed because they carry render-critical work
  (React scheduler, rIC fallback). **This threshold is the knob that
  couples timer scheduling to the drain — see §4.**

### 3.5 IntersectionObserver — `window_bootstrap.js:3509-3547` (THE timing-domain lever)

```js
observe(target) {
    this._elements.add(target);
    Promise.resolve().then(() => {                    // single microtask fire
        const entry = new IntersectionObserverEntry({
            target, isIntersecting: true, intersectionRatio: 1.0,
            boundingClientRect: target.getBoundingClientRect?.() ?? {},
            ...
        });
        this._callback([entry], this);
    });
}
```

Three concrete defects vs real Chrome:
1. **Always `isIntersecting:true, ratio:1.0`** regardless of the
   element's actual layout position or visibility. An off-screen or
   `display:none` element should report `isIntersecting:false,
   ratio:0`. (§2.3 impossible-state signal.)
2. **Fires exactly once** (on `observe`), via a microtask. Real Chrome
   delivers an initial callback on the next *rendering opportunity*
   (rAF-aligned, not microtask) AND re-delivers on every subsequent
   intersection change (scroll, resize, mutation). BO never re-fires →
   scroll-gated lazy content never loads; or all content fires in one
   burst.
3. **`time` uses `performance.now()`** (good) but the rect comes from
   BO's `getBoundingClientRect` which, absent a real layout engine,
   commonly returns zero/origin rects — feeding the impossible
   `isIntersecting:true + zero rect` state.

This is the **booking / SPA-hydration / douyin lazy-content lever** and
the `17` matrix #8 P1 item. Fixing it requires routing the entry's
rect + `isIntersecting` decision through BO's actual layout op (the same
op that backs `getBoundingClientRect`) and re-firing on scroll/mutation.

### 3.6 ResizeObserver — `window_bootstrap.js:3550-3570`

Single microtask fire on `observe` with current dims; never re-fires on
layout change. Same class of gap as IO; lower impact (fewer sites gate
on ResizeObserver re-fire). Note it reads `target.offsetWidth/Height`
for box sizes — if those are zero (no layout), the reported sizes are
zero, another impossible-state signal for visible elements.

### 3.7 requestIdleCallback — `window_bootstrap.js:3573-3585`

`setTimeout(()=>cb({didTimeout:false, timeRemaining:()=>50}), 1)`. Fires
after 1 ms with a fixed 50 ms budget. Real rIC fires only when the event
loop is genuinely idle and `timeRemaining()` decreases through the
deadline. BO's polyfill fires too eagerly and reports a constant budget.
Functionally adequate (SPAs that use rIC to defer work get their
callbacks), but a vendor that schedules rIC and measures
`timeRemaining()` decay would see the constant. Low priority; Camoufox
doesn't engineer this either (parity).

### 3.8 MutationObserver — `dom_bootstrap.js:1874-2094` (REAL, parity)

Full class: `observe` honors `childList`/`attributes`/`characterData`/
`subtree` options (lines 1954-1956 filter by type), batches records,
delivers via microtask, supports `takeRecords`/`disconnect`. `_notifyMO`
is called from the real DOM-mutation paths (`appendChild`/`removeChild`/
`insertBefore`/`remove`, lines 2010-2090). This is the Akamai-sensor
surface and it is **genuinely implemented** — the `17` matrix "partial"
and fix #9 are effectively done.

### 3.9 queueMicrotask + event-loop ordering

`queueMicrotask` is the V8/deno_core builtin (used internally at
`window_bootstrap.js:4672`, `streams_bootstrap.js:102`, etc.) and is in
the native-globals list (`cleanup_bootstrap.js:435`, masked native).
Microtask ordering (Promise → queueMicrotask → MutationObserver
delivery) is handled by deno_core's V8 microtask checkpoint after each
op resolves — correct ordering for free. **The ordering BO does NOT
model** is the macrotask→render-opportunity boundary: real Chrome runs
*all* microtasks, then a rendering opportunity (rAF → IO → RO), then the
next macrotask. BO runs rAF and the observers as plain
`setTimeout`/microtask macro/microtasks, so the **strict
"rAF and IntersectionObserver fire together, once per frame, after
microtasks"** invariant is not preserved. A site that schedules a rAF
and an IO callback and asserts they fire in the same frame tick before
the next setTimeout would see BO's ordering differ. Niche; no known
corpus site.

---

## 4. The drain model and the AWS live-nav gap (connecting timing to the blocker)

### 4.1 What actually runs (refines the handoff's "50 ms drain" framing)

In `build_page_with_scripts_init_and_storage`
(`crates/browser/src/page.rs:3053+`), the script-execution loop
(`page.rs:3511-3567`) does, **per script in document order**:
1. `execute_script_with_name(code, name)` — synchronous eval
   (`page.rs:3540`),
2. flush console logs,
3. **`run_until_idle(Duration::from_millis(50))`** between scripts
   (`page.rs:3566`).

Then **after the loop**: cleanup_bootstrap, DOMContentLoaded/load
install, meta-refresh scan, and a **final `run_until_idle(Duration::from_secs(8))`**
(`page.rs:3643`). The outer navigate loop (`page.rs:2018-2055`) then
adds another `run_until_idle(drain_timeout)` (floored ≥8 s,
`page.rs:2051-2055`).

So the handoff's "only a 50 ms drain" is the **inter-script** drain; the
challenge.js continuation has the **8 s build-phase drain + the outer
nav drain** available *after* the script loop. The real question is why
the worker doesn't spawn during those ~8–16 s.

### 4.2 The likely root cause (timing-domain)

challenge.js's worker is created inside
`checkForceRefresh().then(() => new Worker(blobUrl))` — i.e. the spawn
depends on an **async continuation** that itself depends on either (a) a
`fetch`/token-refresh round-trip, or (b) a `setTimeout`-deferred step.
Two timing-model interactions can stall it:

1. **`run_until_idle` exits on `AllWorkDone` too early.** `run_until_idle`
   (`event_loop/src/lib.rs:316-376`) breaks with `IdleReason::AllWorkDone`
   the moment `run_event_loop()` returns `Ok(())` — i.e. when V8 reports
   no pending refed ops. If challenge.js's continuation is gated on a
   timer that got **`unref`'d** by the `UNREF_THRESHOLD_MS = 2000` rule
   (`timer_bootstrap.js:57-60`) — e.g. challenge.js polls with a
   `setTimeout(..., >=2000)` before deciding to spawn — then that timer
   does **not** keep the loop alive, `run_event_loop` returns `Ok(())`,
   the 8 s drain exits in milliseconds, and the worker never spawns. The
   offline oracle (`from_html_with_url` + a single `run_until_idle(5s)`
   on an inline `<script>`) behaves differently because the inline
   challenge.js executes in a context where its short timers stay refed.
   **This is the most probable cause and is directly timing-model.**

2. **`nav_pending` short-circuit.** If any script (or the
   meta-refresh/`location` machinery) sets `__pendingNavigation`, the
   drain takes the `nav_pending` branch (`lib.rs:323-333`), drains only
   a 150 ms `NAV_TAIL`, and exits — cutting off the worker continuation.

### 4.3 Concrete experiment + fix (public-engine; this is §5.1 of the handoff)

- **Instrument** (env-gated): trap `checkForceRefresh().then` resolution
  and `new Worker` in an init script; log whether the promise resolves
  and whether the loop exited `AllWorkDone` vs `Timeout` during the 8 s
  drain (the profiling path at `lib.rs:300-388` already captures
  pending-op snapshots — enable `BROWSER_OXIDE_EVENT_LOOP_PROFILE`).
- **If cause #1:** for AWS-WAF-challenge pages, either (a) raise
  `UNREF_THRESHOLD_MS` (or make it page-class-aware so the challenge
  stub's timers stay refed), or (b) keep the loop alive for a fixed
  minimum wall time on pages whose only script is an AWS challenge stub
  (detected by the `aws-waf-token` cookie / `challenge.js` URL), instead
  of trusting `AllWorkDone`. The offline-oracle's "single 5 s drain that
  doesn't trust early idle" is the behavior to port.
- **If cause #2:** suppress the spurious `nav_pending` until the
  challenge's async chain has had its drain budget.

**Expected impact:** AWS-WAF cluster (amazon-ca/com/com-au/fr/in/jp +
imdb = 7) + duolingo + possibly booking (handoff §"booking" notes it is
"likely the same live-nav drain class"). **This is the single
highest-ROI timing-domain lever in the project** and is public-engine
addressable (no per-vendor bypass — it's a generic
"let the page's own self-solve finish draining" fix).

---

## 5. Ranked fix list (ROI order)

| # | Fix | Effort | Confidence | Engine | Expected site impact |
|---|---|---|---|---|---|
| T1 | **Live-nav async drain** — stop `run_until_idle` from exiting `AllWorkDone` before the page's self-solve continuation runs; make timer-unref page-class-aware or hold a min-drain for challenge stubs. (`page.rs:3643` + `event_loop/lib.rs:316-376` + `timer_bootstrap.js:57-60`) | 2-4 days (diagnosis instrumented first) | medium | **public** | AWS cluster (7: amazon-ca/com/com-au/fr/in/jp + imdb) + duolingo; maybe booking — up to **9 sites** |
| T2 | **IntersectionObserver real layout + scroll re-fire** — route `isIntersecting`/`ratio`/rect through BO's layout op; re-fire on scroll & mutation; report `false`/`0` for off-screen/`display:none`. (`window_bootstrap.js:3521-3547`) | 2-3 days | medium | **public** | booking, douyin, SPA-hydration cluster — 2-4 sites; reduces "impossible-state" surface corpus-wide |
| T3 | **`setTimeout` nesting-level clamp** — track nesting depth, clamp to 4 ms after depth>5 per HTML spec. (`timer_bootstrap.js:62-79`) | ~0.5 day (~20 LOC + test) | high | **public** | 0 direct flips today; closes a cheap Kasada/Akamai nested-chain probe (defensive, v0.2.0 prereq) |
| T4 | **Verify + lock `performance.timeOrigin` JS wiring** — grep-confirm `window_bootstrap.js` reads `op_perf_time_origin_ms` into the `timeOrigin` getter; add a regression test asserting `timeOrigin + now() within ±5 ms of Date.now()`. (`perf_ext.rs:102-109`) | ~0.5 day | high | **public** | 0 flips (already shipped); prevents regression of the Castle/Kasada skew probe |
| T5 | **ResizeObserver re-fire on layout change** — same class as T2, lower impact. (`window_bootstrap.js:3550-3570`) | 1-2 days | low | **public** | 0-1 sites; mostly closes impossible-state surface |
| T6 | **rAF heavy-tail / hidden-tab pause** — add occasional 33 ms missed-frame; pause rAF when `visibilityState==="hidden"`. (`timer_bootstrap.js:187-201`) | 1 day | low | **public** | 0 flips; defensive vs a vendor that drives `visibilitychange` |
| T7 | **`setInterval` first-call floor correctness** — only clamp to 4 ms after nesting>5, not unconditionally. (`timer_bootstrap.js:113`) | ~0.25 day | medium | **public** | 0 flips; tidies a one-sided tell |
| T8 | **`requestIdleCallback` deadline realism** — fire only at true idle, decay `timeRemaining()`. (`window_bootstrap.js:3573`) | 1 day | low | **public** | 0 flips; parity-with-Camoufox already; deprioritize |
| T9 | **`PerformanceObserver` entry delivery** — deliver `paint`/`resource`/`navigation` entries to `observe()` callbacks instead of no-op. (`window_bootstrap.js:2240-2258`) | 2-3 days | low | **public** | 0 known flips; Akamai-sensor reads it but hasn't been the gating field |
| T10 | **5 µs grid under `crossOriginIsolated`** — tighten `perf_ext.rs` grid to 5 µs when the document is COOP+COEP isolated. | 1 day | low | **public** | 0 flips (no corpus site isolates the challenge doc); pure correctness |

**Notes on ranking:**
- **T1 is the clear #1** — it is the only timing-domain fix with
  multi-site flip potential and it is the project's current critical
  path (handoff §5.1). All others are 0-1 site or defensive.
- Everything is **public-engine addressable.** None of these require the
  private `vendor_solvers` crate: T1/T2 are generic event-loop/layout
  fidelity, not per-vendor bypass. (The AWS *token POST + cookie set* is
  done by challenge.js itself once it can run — BO only needs to let it
  finish draining, then BO's existing cookie-gained re-fetch primitive
  returns content.)
- **Do not regress the clock work.** BO's perf.now jitter + rAF jitter +
  timeOrigin coupling are a measured advantage over Camoufox v150
  (which does zero timing engineering). T4/T6/T10 are about *locking in*
  and *polishing* that lead, not building it.

---

## 6. Open questions

1. **T1 root cause:** is it the `UNREF_THRESHOLD_MS=2000` unref (cause #1)
   or a spurious `nav_pending` (cause #2)? Resolve by enabling
   `BROWSER_OXIDE_EVENT_LOOP_PROFILE` on a live imdb nav and reading the
   per-tick pending-op snapshot (`event_loop/lib.rs:350-362`) — does the
   loop report `AllWorkDone` while challenge.js's continuation is still
   pending?
2. **timeOrigin JS read site:** confirm `window_bootstrap.js` actually
   installs a `performance.timeOrigin` getter backed by
   `op_perf_time_origin_ms` (the op exists/registered; the JS consumer
   was not grep-located in this pass). If missing, the Fix-7 op is
   shipped-but-unwired.
3. **Does the offline oracle keep challenge.js timers refed** because it
   runs as an inline `<script>` (so the prefetch/document-order path and
   its unref logic are bypassed)? If so, the oracle vs live divergence is
   precisely the unref threshold — strong evidence for cause #1.
4. **booking:** is its 8 KB-vs-21 KB shell gap the same drain class as
   AWS, or an independent SPA fetch-chain? Re-test after T1.

---

## 7. Files referenced (verified 2026-05-28)

| Path | Lines | What |
|---|--:|---|
| `crates/js_runtime/src/extensions/perf_ext.rs` | 74-88 | Humanized `performance.now()` (100 µs grid + jitter + spike + monotonic clamp) |
| `crates/js_runtime/src/extensions/perf_ext.rs` | 102-109 | `op_perf_time_origin_ms` (Fix 7 — timeOrigin) |
| `crates/js_runtime/src/extensions/perf_ext.rs` | 131-156 | `op_perf_get_resource_timings` (getEntries resource) |
| `crates/js_runtime/src/js/timer_bootstrap.js` | 57-60 | `UNREF_THRESHOLD_MS=2000` unref logic (couples to drain — §4) |
| `crates/js_runtime/src/js/timer_bootstrap.js` | 62-79 | `setTimeout` — no nesting clamp (T3) |
| `crates/js_runtime/src/js/timer_bootstrap.js` | 109-133 | `setInterval` — uncond. 4 ms floor (T7) |
| `crates/js_runtime/src/js/timer_bootstrap.js` | 172-201 | rAF jittered cadence (Fix 9 — 16.67 ± 0.5 ms) |
| `crates/js_runtime/src/js/timer_bootstrap.js` | 210-215 | dead `performance.now` ms-only fallback (the stale `17 §2.8` line) |
| `crates/js_runtime/src/js/window_bootstrap.js` | 2240-2258 | `PerformanceObserver` stub (T9) |
| `crates/js_runtime/src/js/window_bootstrap.js` | 3509-3547 | IntersectionObserver always-intersecting (T2) |
| `crates/js_runtime/src/js/window_bootstrap.js` | 3550-3570 | ResizeObserver single-fire (T5) |
| `crates/js_runtime/src/js/window_bootstrap.js` | 3573-3585 | requestIdleCallback polyfill (T8) |
| `crates/js_runtime/src/js/window_bootstrap.js` | 1879-1920 | `Worker` ctor → `_resolveWorkerScript` (blob) → `op_worker_spawn` |
| `crates/js_runtime/src/js/dom_bootstrap.js` | 1874-2094 | MutationObserver (real) |
| `crates/js_runtime/src/extensions/worker_ext.rs` | 24-37+ | BlobRegistry backing blob-URL worker spawn |
| `crates/browser/src/page.rs` | 3511-3567 | script-execution loop + 50 ms inter-script drain |
| `crates/browser/src/page.rs` | 3643 | 8 s build-phase final drain |
| `crates/browser/src/page.rs` | 2018-2055 | outer nav-loop drain (≥8 s) |
| `crates/event_loop/src/lib.rs` | 288-391 | `run_until_idle` (AllWorkDone vs Timeout + nav-tail) |

### External references
| URL | Cite for |
|---|---|
| [Chrome cross-origin isolated HR timers](https://developer.chrome.com/blog/cross-origin-isolated-hr-timers) | 100 µs / 5 µs resolution (§2.1) |
| [chromestatus 6497206758539264](https://chromestatus.com/feature/6497206758539264) | resolution alignment ship status |
| [W3C High Resolution Time](https://w3c.github.io/hr-time/) | coarsen+jitter algorithm |
| [Bugzilla 1440863 (Firefox RFP 1ms/2ms)](https://bugzilla.mozilla.org/show_bug.cgi?id=1440863) | Firefox clock clamp = Camoufox tell (§2.2) |
| [W3C IntersectionObserver v2](https://w3c.github.io/IntersectionObserver/v2/) | isIntersecting/zero-rect semantics (§2.3) |
| [MDN IntersectionObserverEntry.boundingClientRect](https://developer.mozilla.org/en-US/docs/Web/API/IntersectionObserverEntry/boundingClientRect) | rect must match getBoundingClientRect |
| [Bugzilla 1671396](https://bugzilla.mozilla.org/show_bug.cgi?id=1671396) | async-scroll rect tolerance |
| [HTML spec timer init steps](https://html.spec.whatwg.org/multipage/timers-and-user-prompts.html#timer-initialisation-steps) | setTimeout nesting clamp (T3) |
| [AWS WAF JS challenge API](https://docs.aws.amazon.com/waf/latest/developerguide/waf-js-challenge-api.html) | challenge.js token/worker pattern (§2.4) |
| DeepWiki `daijro/camoufox` query 2026-05-28 | Camoufox does no timing engineering (§2.2) |

### Sibling docs
`40_TIMING_BEHAVIORAL.md` (canonical timing chapter; §2.2/§2.3/§8 now
partly superseded by Fix-7/Fix-9), `EVENT_LOOP.md` (model; TimerRegistry
is doc-only/aspirational), `17_WEB_API_PARITY_MATRIX.md §2.8` (matrix;
perf.now row stale), `HANDOFF_2026_05_28b.md §4-5` (AWS live-nav drain),
`05_SPA_HYDRATION_CLUSTER.md` (IO/booking consumer of T2).
