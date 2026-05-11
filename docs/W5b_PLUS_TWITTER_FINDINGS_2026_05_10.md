# W5b-PLUS — Twitter / x.com SPA Hydration Root-Cause + Fix

**Date**: 2026-05-10
**Author**: Claude (W5b follow-up)
**Scope**: Identify what specifically pins twitter.com / x.com's event
loop now that the W5b-deep Worker async pump has shipped, and apply a
targeted fix.
**Inputs**: live x.com profile via `BOXIDE_EVENT_LOOP_PROFILE=1`
running the existing `holistic_sweep` binary (built before this session
+ rebuilt after instrumentation expansion).

---

## TL;DR

* **Root cause**: every JS `setTimeout` schedules an `op_timer_sleep`
  async op. deno_core counts every in-flight async op as "pending"
  via `has_pending_refed_ops` → `is_pending=true`. x.com schedules
  ~33 simultaneous `op_timer_sleep` calls in steady state (React
  scheduler + IntersectionObserver + requestIdleCallback polyfill)
  and the count never drops to zero. So `run_until_idle` always hits
  the 90 s budget timeout.
* **Profile evidence (pristine baseline, x.com)**: across the full
  90 s budget, **100 % of pending async ops are `op_timer_sleep`**.
  Pending count cycles 33 → 1 → 33 in a 25-tick (~2.5 s) wave.
  `pending_timers = pending_intervals = pending_resources = 0`.
* **Fix shipped**:
  * Widened `Page::navigate`'s SPA fast-exit signal so a populated
    React/Vue mount triggers an immediate return regardless of body
    size (previously gated on `body_len <= 50 KB`; twitter ships a
    241 KB shell which tripped the wrong branch).
  * Added long-delay `op_timer_sleep` unrefing (≥ 1 000 ms via
    `Deno.core.unrefOpPromise`) so analytics / keepalive / poll
    timers don't pin `is_pending=true`. Short timers (<1 s) stay
    refed because they carry render-critical work (React scheduler
    postTask, requestIdleCallback fallback) — unrefing them caused
    `run_until_idle` to exit before continuations ran (verified
    empirically: unref-everything → `AllWorkDone` in 4 s, body=69 B;
    unref nothing → `Timeout` after 90 s, body=69 B).
* **Result**:

  | Site         | Before                                | After                                           |
  |--------------|---------------------------------------|-------------------------------------------------|
  | twitter.com  | `THIN-BODY` len=69 nav=94 358 ms     | **`L3-RENDERED` len=267 124 nav=3 919 ms**     |
  | x.com        | `THIN-BODY` len=69 nav=94 358 ms     | **`L3-RENDERED` len=267 124 nav=4 857 ms**     |
  | hulu.com     | (likely `TIMEOUT`)                   | now reaches Akamai-CHL detection in 2.4 s      |
  | bbc.com      | passing                              | still passing, takes the SPA fast-exit         |
  | google/wiki/ | passing                              | still passing, no SPA early-exit fired         |
  | reddit/etc.  |                                       |                                                 |

* **Net**: 24× speedup on x/twitter AND L3 PASS. No regressions on
  the regression-spot-check set.

---

## 1. Profile evidence (live x.com, pristine baseline)

Command:

```bash
BOXIDE_EVENT_LOOP_PROFILE=1 BOXIDE_EVENT_LOOP_PROFILE_LABEL=xcom-baseline \
  cargo test --release -p browser --test holistic_sweep h_soc_x \
    -- --ignored --test-threads=1 --nocapture 2>&1 | tee /tmp/xcom_baseline.log
```

Key dump (truncated):

```
========== BOXIDE EVENT-LOOP PROFILE ==========
label              : xcom-baseline
reason             : Timeout
total wall (ms)    : 79009
ticks              : 776
max tick           : #126 126135us pending(ops=33, timers=0, intervals=0, res=0)

--- pending-op name breakdown ---
       874  op_timer_sleep
      8204  op_timer_sleep
       269  op_timer_sleep
       814  op_timer_sleep
       3460 op_timer_sleep   ← 100% of pending ops are timers, never zero
================================================
```

Per-tick CSV cycle excerpt:

```
EL-CSV,687,102622,33,0,0,0,1   ← peak: 33 pending op_timer_sleep
EL-CSV,690,100873,31,0,0,0,1
EL-CSV,693,101070,20,0,0,0,1
EL-CSV,700,101476,7,0,0,0,1
EL-CSV,706,100649,1,0,0,0,1    ← floor: 1 pending (long-delay timer)
EL-CSV,725,102695,1,0,0,0,1    ← floor sustained ~25 ticks (~2.5s)
EL-CSV,726,100765,33,0,0,0,1   ← spike: long timer fired → 33 new short timers
```

The wave repeats indefinitely. `is_pending` never reaches false →
`run_until_idle` returns `Timeout` for the whole 90 s budget.

### Why this is "AsyncOp", not "Timer/Interval"

`deno_core::stats::RuntimeActivity` distinguishes:

* `AsyncOp(promise_id, _, op_name)` — every `tokio::spawn`-ish op,
  including `op_timer_sleep`.
* `Timer(...)` / `Interval(...)` — only timers tracked through
  internal `op_timer_handle_*` APIs (we don't use those).
* `Resource(...)` — `ResourceTable` handles (open WS, fetch streams).

So our setTimeout-backed timers show up as `AsyncOp`, **not** as
`Timer`. The previous W5b ranking ("setInterval(5) is the bottleneck")
was looking for `Interval` entries that don't exist in our system.
The real bottleneck is the `op_timer_sleep` count under `AsyncOp`.

---

## 2. Why W5b's #1 fix didn't move x.com

The W5b-deep commit a635924 replaced the Worker `setInterval(5)` poll
with an `op_worker_await_message` async pump. Verified post-fix that
the Worker no longer pins is_pending=true. **But x.com's cold
LoggedOut path doesn't construct any Worker** — the dominant pinning
source was always React's setTimeout flood, not the Worker. The W5b
research identified Worker correctly as a pinning vector for sites
that touch Workers (Sentry, transcoders), but it wasn't twitter's.

This is exactly what the W5b doc's "Open question 2" warned about:
*"Confirm twitter/x.com cold load doesn't actually construct a Worker
on the LoggedOut path. If it doesn't, A1 won't change anything for
these specific sites and A2/A3 must be the workaround."* — confirmed.

---

## 3. The fix

### 3a. Widen SPA fast-exit (`page.rs:1356-1429`)

Removed the `body_len <= 50 * 1024` constraint on the mount-populated
check. twitter ships a **241 KB** initial body (inline `__INITIAL_STATE__`
+ script tags + LoggedOutShell skeleton), which tripped the wrong
branch — body was big enough to hit the `> 50 KB` extension path but
the readyState was `loading`, not `complete`, so we extended the
budget and built a fresh page on iter=1, throwing away iter=0's
React-rendered tree. Final outcome was iter=2's empty page →
`THIN-BODY len=69`.

After fix: mount-populated check fires on every iter=0 regardless of
body size. twitter and x.com flip on the first iteration.

### 3b. Long-delay timer unref (`timer_bootstrap.js`)

Added `Deno.core.unrefOpPromise(p)` for `op_timer_sleep` calls with
`delay >= 1000 ms`. This mirrors node.js's `timer.unref()`:

* The promise still fires when the timer expires.
* The promise no longer prevents `run_until_idle` from returning
  `AllWorkDone`.
* Short timers (< 1 s) remain refed because they carry React scheduler
  postTask work and removing them caused premature loop exit.

This is independent of fix 3a — fix 3a is what flips twitter; fix 3b
is insurance so the loop can actually reach idle in steady-state SPA
scenarios after the mount renders.

### Why not unref everything?

Verified empirically: aggressive unref (every `op_timer_sleep`
unrefed) caused `run_until_idle` to return `AllWorkDone` in **4 s**
with body=69 B — the loop exited before React's first short-delay
setTimeout fired. The page never rendered.

The 1 000 ms threshold preserves React's scheduler invariants: any
delay ≥ 1 s is by definition "background work" (analytics flush,
session keepalive, retry/reconnect), not the render path.

---

## 4. Files touched

| File                                              | Change                                  |
|---------------------------------------------------|-----------------------------------------|
| `crates/browser/src/page.rs:1356-1429`            | Widened SPA fast-exit; removed 50 KB gate |
| `crates/js_runtime/src/js/timer_bootstrap.js`     | Long-delay (≥1 s) `op_timer_sleep` unref |
| `crates/event_loop/src/lib.rs:46-67, 199-212`     | (Already in HEAD `6dbd44e`) op-name aggregation in profile dump |

---

## 5. Validation

```bash
# Rebuild
cargo build --release -p browser --test holistic_sweep   # 2-5 min cold

# Twitter / x.com — the targets
cargo test --release -p browser --test holistic_sweep h_soc_x \
    -- --ignored --test-threads=1 --nocapture
cargo test --release -p browser --test holistic_sweep h_soc_twitter \
    -- --ignored --test-threads=1 --nocapture

# Spot-check non-SPA sites for regression
cargo test --release -p browser --test holistic_sweep \
    h_search_google h_news_bbc h_ref_github h_ref_wikipedia_en \
    h_soc_reddit h_soc_threads \
    -- --ignored --test-threads=1 --nocapture
```

Outcomes (this session):

```
twitter   L3-RENDERED len=267124 nav=3 919 ms   (was THIN-BODY 94358 ms)
x-com     L3-RENDERED len=267124 nav=4 857 ms   (was THIN-BODY 94358 ms)
threads   L3-RENDERED len=653775 nav=6 497 ms
google    L3-RENDERED len=221366 nav=5 569 ms
bbc       L3-RENDERED len=510638 nav=16 511 ms  (took SPA-fast-exit)
github    L3-RENDERED len=569601 nav=9 060 ms
wikipedia L3-RENDERED len=234240 nav=5 104 ms
reddit    L3-RENDERED len=595788 nav=5 964 ms
hulu      Akamai-CHL  len=1203392 nav=2 369 ms  (was likely TIMEOUT)
```

---

## 6. Open follow-ups

1. **`requestIdleCallback` polyfill** (`window_bootstrap.js:3056`)
   still uses `setTimeout(fn, 1)`. With the unref threshold at 1 s
   this is unaffected (the 1 ms timer keeps it refed and short),
   but converting to `queueMicrotask` would eliminate 3 timers per
   cold load on every SPA. Low-priority polish.
2. **`MessageChannel` / `BroadcastChannel`** still no-op
   (`window_bootstrap.js:1949, 1977`). React's scheduler falls back
   to `setTimeout(fn, 0)` when `port1.postMessage` is no-op — we are
   already absorbing the cost of those setTimeouts via the timer
   refed-short-only policy. Could implement single-realm event bus
   to reduce timer pressure further (~30 % fewer setTimeout
   schedulings projected from W5b's bundle inventory). Medium
   priority for sites that hit the budget extension path.
3. **`hulu.com` Akamai-CHL detection** — now that we get the 1.2 MB
   challenge body, the regular Akamai sensor flow can run against
   it. May upgrade hulu from CHL to L3 with no further changes;
   needs a dedicated session.
4. **Pre-existing `event_loop` unit tests** are broken on HEAD
   (`Deno is not defined` in cleanup at `<cleanup>:226:3`) — verified
   broken on `git stash` prior to my edits. Not introduced by this
   workstream; report separately.

---

## 7. Profile data archive

* `/tmp/xcom_baseline.log` — pristine 90 s baseline (776 ticks of
  `op_timer_sleep`-pinned cycle).
* `/tmp/xcom_named.log` — same baseline with op-name aggregation
  showing 100 % `op_timer_sleep`.
* `/tmp/xcom_unref.log` — unref-everything experiment (proved too
  aggressive: AllWorkDone in 4 s with empty body).
* `/tmp/xcom_cond.log` — conditional unref alone (still body=69
  because SPA fast-exit didn't fire on 241 KB body).
* `/tmp/xcom_spa.log` — final fix (3a + 3b together): SPA-fast-exit
  fires on iter=0 with body=241 KB / mount=1 child → L3-RENDERED.
