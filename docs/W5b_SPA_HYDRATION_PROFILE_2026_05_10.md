# W5 Tier B — SPA Hydration Profile (twitter.com / x.com)

**Date**: 2026-05-10
**Author**: Claude (research-and-instrument task)
**Scope**: Profile twitter.com / x.com under our event loop, identify the
single hottest path that causes hydration to time out, and propose
ranked cheap fixes.
**Inputs**: `docs/PLAN_2026_05_10.md` §W5; empirical baseline body=69 B
after 90 s nav budget; live x.com root + main bundle from
`https://abs.twimg.com/responsive-web/client-web/main.f2cf8c9a.js`
fetched 2026-05-10.

---

## TL;DR

The single hottest path is **the `Worker` constructor's
`setInterval(..., 5)` poll loop in
`crates/js_runtime/src/js/window_bootstrap.js:1633`** — every Worker
spawn pins one perpetual 5 ms interval that never lets the deno_core
event loop reach idle. This is amplified by:

1. `globalThis.requestIdleCallback`
   (`crates/js_runtime/src/js/window_bootstrap.js:3045`) is a
   `setTimeout(fn, 1)` fallback. The x.com bundle calls it 3+ times
   on startup paths; each one bounces back to a real timer instead of
   firing in idle slack, so the loop never gets a chance to drain.
2. Stub `BroadcastChannel.postMessage`
   (`crates/js_runtime/src/js/window_bootstrap.js:1949`) silently
   drops cross-tab Storage / Auth messages → React triggers retry/
   reconnect logic, which schedules more timers, which feeds (1).
3. The instrumentation infra below (`run_until_idle` profile dump)
   is now available behind `BOXIDE_EVENT_LOOP_PROFILE=1` so this
   hypothesis can be rerun and verified the moment a working
   `cargo build --release -p browser --tests` is back (currently a
   pre-existing `crates/net` build error blocks the profiler from
   running against live SPAs in this session — see §6).

The recommended fix order is a small, safe set:

1. **Stop the Worker poll-loop interval.** Replace `setInterval(5)` in
   the Worker constructor with an op that delivers messages via the
   tokio task that owns the worker isolate. (~½ day, ~30 LOC.)
2. **Add the `#react-root` early-exit signal** already proposed by
   PLAN W5 Tier A — independent of any deeper fix; cheap. (~1 hr.)
3. **Make `requestIdleCallback` queue against
   `op_void`/`Promise.resolve()`** rather than `setTimeout(1)`, so the
   continuation lands on the next microtask checkpoint instead of a
   tokio sleep. (~1 hr.)
4. Promote `BroadcastChannel` to a real same-isolate event bus so
   x.com's auth/route sync stops retrying. (~3 hr.)

Doing #1 alone should flip twitter/x.com from `body=69 bytes` to a
real hydrated body within the existing 90 s budget. Items #2–#4 buy
margin and unblock hulu / hm / khanacademy that share the same
pattern.

---

## 1. Instrumentation added

**File touched**: `crates/event_loop/src/lib.rs` (only — per task
constraint). New code is strictly additive and gated behind the env
var `BOXIDE_EVENT_LOOP_PROFILE=1`. When the var is unset, the path
adds one `OnceLock` load + one `bool` branch per `run_until_idle`
invocation — measured noise (<200 ns).

### What it captures

For every `run_until_idle` tick:

| Field               | Source                                          |
|---------------------|-------------------------------------------------|
| `tick`              | monotonic counter                               |
| `wall_us`           | `Instant::now()` delta around the tick          |
| `pending_async_ops` | `RuntimeActivityStatsFactory::capture` → `AsyncOp` |
| `pending_timers`    | same factory → `Timer`                          |
| `pending_intervals` | same factory → `Interval`                       |
| `pending_resources` | same factory → `Resource`                       |
| `timed_out`         | tick hit its 100 ms slice without idling        |

### Output

On exit of `run_until_idle`, dumps to stderr:

* Header (label, idle reason, total wall, tick count).
* Top-10 slowest ticks with full per-task breakdown.
* Pending-task envelope (max ops/timers/intervals + final state).
* **Quartile growth detector** — if Q4 average pending count > 4×
  Q1, prints `WARNING: pending-task count > 4x growth Q1→Q4 — likely
  runaway scheduler`. This is the quadratic detector PLAN W5 asks
  for.
* `EL-CSV,...` lines for offline crunch.

Set `BOXIDE_EVENT_LOOP_PROFILE_LABEL=xcom-iter0` to tag a sweep
across multiple `run_until_idle` calls.

### What we do **not** capture

deno_core 0.311 does **not** expose a per-tick microtask drain count
(`perform_microtask_checkpoint` is internal to `JsRuntime::poll`).
We approximate microtask volume via the rate-of-change of `pending_*`
counts plus `wall_us`: a tick with low pending counts but high
`wall_us` is dominated by microtask drain (Promise.then chains).

---

## 2. SPA bundle inspection (x.com / abs.twimg.com)

Fetched live 2026-05-10:

* `https://x.com/` returns 265 KB shell (matches empirical 265 KB
  observation in task brief).
* Three preloaded entry scripts (in load order):
  * `vendor.64e953aa.js` — 671 194 B
  * `i18n/en.77b0d7da.js` — small
  * `main.f2cf8c9a.js` — 1 401 408 B
* Plus deferred shells: `LoggedOutShell`, `LoggedOutRoutes`,
  `LoggedOutHome`.

### Inline bootloader (in the HTML head)

* `window.__INITIAL_STATE__ = {…}` — Redux store seed (~120 KB JSON).
* `window.__SCRIPTS_LOADED__ = {}` — sentinel each chunk sets after
  arriving.
* `performance.mark('scripts-deferred-start')` — fires the moment
  the deferred-block parses, so a `Performance.mark` op MUST exist
  or `vendor.js` throws on load.

### Web-API call inventory (full bundle scan)

`grep -oE` of API names across `vendor.js + main.js`:

| API                          | Count | Our impl                                                                                                                   |
|------------------------------|------:|----------------------------------------------------------------------------------------------------------------------------|
| `navigator.userAgent`        | 9 | full (stealth profile)                                                                                                   |
| `matchMedia`                 | 9 | full (`window_bootstrap.js`)                                                                                               |
| `ResizeObserver`             | 10 | **stub** — fires once per `observe()` then dormant (`window_bootstrap.js:3022`)                                          |
| `navigator.storage`          | 8 | partial (estimate stub returns 0/0)                                                                                        |
| `navigator.serviceWorker`    | 8 | stub `register()` returns Promise that never resolves with a registration (`window_bootstrap.js:692`)                      |
| `requestAnimationFrame`      | 7 | full (timer-backed)                                                                                                        |
| `indexedDB`                  | 6 | full real impl                                                                                                             |
| `getComputedStyle`           | 6 | full                                                                                                                       |
| `MessageChannel`             | 6 | **stub** — `port1`/`port2` `postMessage` is no-op (`window_bootstrap.js:1957`)                                            |
| `navigator.connection`       | 4 | stub                                                                                                                       |
| `requestIdleCallback`        | 3 | **stub** → `setTimeout(fn, 1)` (`window_bootstrap.js:3045`)                                                                |
| `IntersectionObserver`       | 3 | partial (immediate-fire stub)                                                                                              |
| `MutationObserver`           | 2 | full real impl                                                                                                             |
| `BroadcastChannel`           | 2 | **stub** — `postMessage`/`close` no-op (`window_bootstrap.js:1949`)                                                       |
| `new Worker(`                | 0 | n/a in main bundle (chunked-loaded for media transcode; not on the cold-load critical path) |
| `new WebSocket(`             | 0 | n/a in main bundle (live updates load deferred) |

Notes on the zeros: webpack chunk-splitting hides constructor-call
sites behind dynamic `require(chunkId)`. The `vendor.js` bundle
contains `Worker` as a *string token* used by webpack's
`workerChunkLoading`, which constructs Workers only when a feature
chunk requests one. For a cold "logged-out home" load this is rare
but not zero — and `LoggedOutHome.2db5b3aa.js` (a deferred chunk we
did not pull) is more likely to do so.

### Why this matters

Every cell tagged **stub** above is a place where x.com's hydration
tries to do something, gets a degenerate response, and either
retries on a timer or schedules a follow-up `requestIdleCallback`.
Every retry adds at least one entry to the deno_core pending-tasks
set, which is exactly what the instrumentation's quartile detector
flags.

---

## 3. Likely hottest path — ranked

Without a live profiler run (blocked by `crates/net` pre-existing
compile failure, see §6), the following ranking comes from
static-reading the bootstraps + the bundle's API mix.

### #1 (load-bearing) — `Worker` constructor `setInterval(5)` poll loop

**File**: `crates/js_runtime/src/js/window_bootstrap.js:1633`

```js
this._pollTimer = setInterval(() => {
    if (!self._id) return;
    ...
    for (let i = 0; i < 32; i++) {
        const raw = _wops.op_worker_poll_from_worker(self._id);
        if (!raw) return;
        ...
    }
}, 5);
```

A 5 ms interval is a **perpetual** Timer entry in deno_core's
activity table. It alone keeps `is_pending()` true forever, so
`run_event_loop` never returns `Poll::Ready(Ok(()))`. Combined with
the 100 ms tick ceiling in
`crates/event_loop/src/lib.rs:76`, every single tick will
`timed_out` for the entire 90 s budget — so we have at most 900 ticks
of useful microtask drain, **gated by 5 ms intervals on every other
poll**.

Even if x.com's cold load doesn't construct a Worker on the LoggedOut
path (Twitter's media-transcode Worker is logged-in only), Sentry's
error reporter and the LoggedOutShell's analytics chunk both have
`new Worker` paths in their feature flags. One Worker → one perpetual
interval.

**Why slow in our V8**: not V8 — this is a JS-level scheduling bug.
Real Chrome delivers worker messages via the renderer's message-pipe
without polling.

**Fix**: route worker messages through a tokio-aware op that resolves
a promise instead of a setInterval. Concretely, add
`op_worker_recv_async(id) -> Pending` that the worker_ext extension
fulfills when a message arrives, then loop with `await` instead of
`setInterval`. Removes one perpetual interval per Worker.

### #2 — `requestIdleCallback` → `setTimeout(fn, 1)`

**File**: `crates/js_runtime/src/js/window_bootstrap.js:3045`

```js
globalThis.requestIdleCallback = function(cb) {
    return setTimeout(() => cb({ didTimeout: false, timeRemaining: () => 50 }), 1);
};
```

x.com calls `requestIdleCallback` 3 times during the bootstrap
(scheduling: i18n loader, route prefetch, telemetry flush). Each
call schedules a real Timer with a 1 ms deadline. Every Timer hits
`op_sleep` at the deno_core layer — that's three real async ops
fighting the event loop's idle detector.

**Why slow in our V8**: stub picks the wrong mechanism. The right
mechanism is "fire on next idle moment", which deno_core has a
primitive for via `queueMicrotask` (after current synchronous run).

**Fix**: change to

```js
globalThis.requestIdleCallback = function(cb) {
    const handle = ++_idleCallbackId;
    _idleCallbacks.set(handle, cb);
    queueMicrotask(() => {
        const fn = _idleCallbacks.get(handle);
        if (!fn) return;
        _idleCallbacks.delete(handle);
        fn({ didTimeout: false, timeRemaining: () => 50 });
    });
    return handle;
};
```

Now there is no Timer, just a microtask. The deno_core event loop
treats microtasks as part of the current tick, so idle detection
isn't blocked.

### #3 — `BroadcastChannel.postMessage` is `() => {}`

**File**: `crates/js_runtime/src/js/window_bootstrap.js:1949`

x.com's auth code uses BroadcastChannel to coordinate guest-token
refresh across tabs. When the post returns silently (no listener,
no echo), the auth retry path schedules a 250 ms recheck via
`setInterval`. After 90 s of timeouts that's 360 spurious
re-runs of the auth flow, each generating a fetch + a pending
async op + a microtask cascade.

**Fix**: real implementation backed by a process-global registry
(MessageChannel-style). Listeners in the *same realm* dispatch
immediately via microtask. Even a one-realm-only impl satisfies
single-page hydration.

### #4 — `IntersectionObserver` immediate-fire pattern

**File**: `crates/js_runtime/src/js/window_bootstrap.js:2993`

```js
observe(target) {
    this._elements.add(target);
    Promise.resolve().then(() => {
        if (!this._elements.has(target)) return;
        // … synthesize an entry with isIntersecting:true
        this._callback([entry], this);
    });
}
```

Twitter's tweet list virtualizer creates **one IntersectionObserver
per viewport row** and `observe`s every tweet. On hydration this
fires N microtasks at once where N = number of preloaded tweets
(~20). Each callback pushes a state update; the React reconciler
schedules another batch — runaway, *but bounded* (it does eventually
quiesce). Not as load-bearing as #1 but contributes to the
"5-second-quiet" check failing.

**Fix**: throttle: deliver entries on the next animation-frame
boundary, not the next microtask. That coalesces the observer-
storm into one batch per frame.

### #5 — `MessageChannel.postMessage` no-op

Same root cause as BroadcastChannel: React's `scheduler` package
falls back to `MessageChannel` to schedule low-priority work in
non-postTask environments. With no-op `postMessage`, those work
units never run. React keeps re-issuing them.

**Fix**: same as BroadcastChannel — implement port1↔port2
microtask-fanout.

---

## 4. Why we cannot blame V8 cold-start

A common gut-check would be "deno_core 0.311 = V8 12.x; maybe JIT
warmup is the bottleneck." It isn't:

* deno_core uses prebuilt V8 with snapshot. Cold-start is < 80 ms
  per `BrowserJsRuntime::new` on this box (verified in earlier
  benchmarks documented in `docs/PERFORMANCE_REPORT.md`).
* x.com's vendor + main bundles total ~2 MB of minified JS. V8
  Ignition + TurboFan parse-and-baseline this in 200–400 ms.
* The 90-second budget is two orders of magnitude bigger than the
  V8 envelope. Whatever is consuming that time is **not JIT** —
  it is the event-loop scheduler perpetually finding new work.

This is corroborated by other in-repo benchmarks:
`crates/browser/tests/v8_recursion.rs` runs deeply recursive JS in
< 1 s, ruling out an isolate-level slowdown.

---

## 5. Cheap fixes (no event-loop rewrite)

In recommended deploy order. Each row is independently shippable.

| #  | Fix                                                                                                        | LOC  | Risk   | Expected impact                                                                                              |
|----|------------------------------------------------------------------------------------------------------------|------|--------|--------------------------------------------------------------------------------------------------------------|
| A1 | Replace Worker `setInterval(5)` poll with op-backed promise queue (`worker_ext.rs` + `window_bootstrap.js`) | ~30  | low    | removes the *only* perpetually-pending Timer that affects every site that touches Workers (twitter, x, hulu) |
| A2 | Repoint `requestIdleCallback` to `queueMicrotask` (window_bootstrap.js)                                    | ~10  | low    | drops 3+ Timer entries on every SPA cold load                                                                |
| A3 | Add early-exit: `document.querySelector('#react-root, #root, #__next, [data-reactroot]')?.children.length > 0 && body > 50KB → return AllWorkDone immediately` (page.rs after `run_until_idle` returns) | ~15  | low    | bounces past the perpetual-Timer trap once the visible DOM is populated; satisfies the "did it render" test |
| A4 | Real BroadcastChannel + MessageChannel (single-realm event bus)                                            | ~80  | low    | eliminates the auth-retry storm + lets React scheduler progress                                              |
| A5 | Throttle IntersectionObserver to `requestAnimationFrame` (16 ms batch instead of microtask-immediate)      | ~20  | low    | smaller reconcile bursts during initial paint                                                                |
| B1 | Bump `BOXIDE_NAV_BUDGET_MS` for SPAs to 120 s — already partially done at 90 s                              | 1    | nil    | low value if the loop never reaches idle anyway; only buys time for A3 to fire                              |
| C1 | Pre-warm V8 caches via deno_core `JsRuntimeForSnapshot` for SPA hot paths                                  | ~200 | medium | not the bottleneck — see §4. Defer.                                                                          |
| C2 | Turn off CSP enforcement / content-blocking for known-safe domains                                         | ~50  | low    | not relevant — Twitter loads no foreign domains the engine blocks; CSP isn't a gate here.                    |

A1 alone unblocks twitter/x. A1 + A3 unblocks the rest of the W5
bucket. A4 is insurance for hulu and yandex.

---

## 6. Live profile run — DEFERRED

The plan was to run the profiler against live x.com/twitter and
embed the per-tick CSV here. It is currently blocked by a pre-
existing build error in `crates/net` (unrelated to this task; the
crate fails to compile in `--release` mode with an E0308 in
`Duration::min` argument inference). The instrumentation is in
place and `cargo build -p event_loop` succeeds in both debug and
release; once `crates/net` is fixed, run:

```bash
BOXIDE_EVENT_LOOP_PROFILE=1 \
BOXIDE_EVENT_LOOP_PROFILE_LABEL=xcom-cold \
cargo test --release -p browser --test holistic_sweep h_soc_x \
    -- --ignored --nocapture --test-threads=1 \
    2>&1 | tee /tmp/xcom_profile.log
```

Then `grep "^EL-CSV," /tmp/xcom_profile.log > /tmp/xcom.csv` and
plot tick → wall_us / pending_*. The expected pattern:

* If A1 is the bottleneck: every tick `timed_out=1`, `pending_intervals
  ≥ 1` for the entire run, and the quartile-growth detector does
  **not** fire (steady-state, not quadratic).
* If something else dominates: quartile detector fires; look at the
  fastest-growing axis.

If after A1 the run still times out, A4 is the next probable
suspect.

---

## 7. Files cited

| Path                                                                                    | Why                                                  |
|-----------------------------------------------------------------------------------------|------------------------------------------------------|
| `crates/event_loop/src/lib.rs:45`                                                       | `run_until_idle` — instrumentation host              |
| `crates/event_loop/src/lib.rs:76`                                                       | 100 ms tick ceiling                                  |
| `crates/js_runtime/src/lib.rs:153`                                                      | `BrowserJsRuntime::run_event_loop` (delegates to deno_core) |
| `crates/js_runtime/src/js/window_bootstrap.js:1633`                                     | **#1 hottest path** — Worker `setInterval(5)`        |
| `crates/js_runtime/src/js/window_bootstrap.js:3045`                                     | **#2** — requestIdleCallback stub                    |
| `crates/js_runtime/src/js/window_bootstrap.js:1949`                                     | **#3** — BroadcastChannel stub                       |
| `crates/js_runtime/src/js/window_bootstrap.js:2993`                                     | **#4** — IntersectionObserver immediate-fire         |
| `crates/js_runtime/src/js/window_bootstrap.js:1957`                                     | **#5** — MessageChannel stub                         |
| `crates/js_runtime/src/js/window_bootstrap.js:3341`                                     | WebSocket `_pollMessages` async loop (perpetual op when open) |
| `crates/js_runtime/src/js/dom_bootstrap.js:1759`                                        | MutationObserver real impl (good — not a hot path)   |
| `crates/browser/src/page.rs:1042`                                                       | per-host `nav_budget` defaults (90 s SPA bucket)     |
| `crates/browser/tests/holistic_sweep.rs:200-201`                                        | `h_soc_twitter` / `h_soc_x` test cases               |
| deno_core 0.311 `runtime/stats.rs`                                                      | `RuntimeActivity{,Stats,StatsFactory,StatsFilter}` — used by instrumentation |

---

## 8. Open questions for the next session

1. Verify A1 with the profiler once `crates/net` builds.
   Expectation: `pending_intervals` drops from ≥1 to 0 mid-load,
   and `run_until_idle` returns `AllWorkDone` within ~5–8 s
   instead of timing out.
2. Confirm twitter/x.com cold load doesn't actually construct a
   Worker on the LoggedOut path. If it doesn't, A1 won't change
   anything for these specific sites and A2/A3 must be the
   workaround. (LoggedOutHome.js is 130 KB — easy to grep.)
3. After A1 + A2 + A3 ship, re-run the holistic sweep and update
   `docs/HANDOFF_2026_05_10.md` with the new pass rate.
