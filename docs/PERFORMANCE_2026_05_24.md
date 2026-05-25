# Performance investigation — 2026-05-24

Branch `main` (post-fix). Companion to `BENCHMARK_2026_05_24.md` (corpus
pass-rate + per-engine metrics). This document covers the **root-cause
investigation** that closed a 6× per-navigation gap to Playwright on
benign pages.

## TL;DR

| Engine path | example.com per nav (median, 5 runs) | Notes |
|---|---:|---|
| browser_oxide (pre-fix) | 6585 ms | `Page::navigate` |
| browser_oxide (fix 1) | 2308 ms | `__pendingNavigation` scrub |
| browser_oxide (fix 2) | 2308 ms | build-phase drain caps |
| browser_oxide (fix 3) | 264 ms | `humanize.js` background timers |
| **browser_oxide (warm pool)** | **141 ms** | `PagePool::navigate` (new API) |
| playwright (chromium-headless) | 181 ms | for reference |
| puppeteer | 595 ms | for reference |

End-to-end **47× speedup** on a 528-byte static page through four scoped
fixes. No behavioural change for anti-bot pages (all
chrome_compat / anti_bot / navigation_primitives tests still pass).

## Methodology

- Bench harness: `crates/browser/examples/nav_timed.rs` (Rust, in-process)
  for browser_oxide; `/tmp/bo-bench/nav_timed.js` for Playwright /
  Puppeteer (single-line JSON per run, same field shape).
- Sites: example.com (528 B static), news.ycombinator.com (~35 KB
  light JS), wikipedia.org/Main_Page (~230 KB).
- 5 runs per site / per engine, median reported.
- Single-IP, single box (load < 2), warm Chrome binary cache.

## Bug 1 — Spurious `__pendingNavigation` re-fetch loop

`crates/event_loop/src/lib.rs::reset_nav_pending`

### Root cause

`build_page_with_scripts_init_and_storage` seeds the URL state with
`event_loop.execute_script("location.href = '<url>';")`. The JS-side
`location.href` setter in `window_bootstrap.js` writes
`_browser_oxide.__pendingNavigation = { kind: "assign", url: … }` as a
side-effect — that is its documented behaviour for `location.href = …`
*coming from page scripts*. The build phase isn't a page script; it's
URL-state seed. The setter doesn't know that.

`event_loop.reset_nav_pending()` cleared the Rust-side
`NavSignal::AtomicBool` but never scrubbed the JS-side
`__pendingNavigation` value. The navigate loop's `PENDING_NAV_JS`
therefore reads back a non-empty pending-nav after every iteration, and
the loop does its full inner iteration **3 times** for every URL — each
iteration spins up a fresh V8 isolate and bootstrap.

### Fix

Make `reset_nav_pending` scrub both sides, and take `&mut self` so the
JS-side scrub can run through the same code path:

```rust
pub fn reset_nav_pending(&mut self) {
    self.runtime.reset_nav_pending();
    let _ = self.runtime.execute_script(
        "globalThis._browser_oxide && (globalThis._browser_oxide.__pendingNavigation = null);",
        None,
    );
}
```

All 4 callers in `crates/browser/src/page.rs` already had `&mut`.

### Impact

example.com: **6585 ms → 2308 ms (2.85×).**

## Bug 2 — Build-phase drains waiting out their timeouts

`crates/browser/src/page.rs::build_page_with_scripts_init_and_storage`

### Root cause

Two `run_until_idle(…)` calls in the build path:

- `__syncCookiesFromNet` drain: `Duration::from_secs(1)`
- post-meta-refresh build drain: `Duration::from_secs(8)`

Each `run_until_idle` only returns `AllWorkDone` when the event loop has
no pending tasks. With `humanize.js` (a default-on init script)
scheduling 30 pending `setTimeout` callbacks spread over ~1.8 s and a
recurring `setInterval(runCycle, 4000)`, the event loop is **never
idle** — the drains hit their full timeouts on every navigation,
adding ~2 s of pure wait per page regardless of how trivial the page
itself is.

Verified with `BROWSER_OXIDE_EVENT_LOOP_PROFILE=1`:
```
=== EVENT-LOOP PROFILE ===
reason: Timeout
total wall (ms): 101
pending-op name breakdown: 33 op_timer_sleep
```

33 pending `op_timer_sleep` ops, 0 timers/intervals — i.e. 33 in-flight
tokio sleeps from `humanize`'s setTimeout calls.

### Fix

Tighten the drain caps to "what the work actually needs":

| Call | Old | New | Rationale |
|---|---:|---:|---|
| `__syncCookiesFromNet` drain | 1 s | 50 ms | One `op_cookie_get` op call; microtask resolves in <1 ms |
| build-phase post-meta-refresh drain | 8 s | 200 ms | DOMContentLoaded/load `setTimeout(0)` + microtasks. Inline scripts already get 50 ms each via the per-script drain. Longer work is correctly handled by the outer nav-loop drain. |

Both retain useful safety margin without blocking on humanize timers.

### Impact

example.com per-nav: 2308 ms → … *still 2054 ms* — the wait moved from
build-phase to the outer navigate-loop drain. Wait time is on humanize,
not where it lives. **Fix 3** addresses the root.

## Bug 3 — humanize.js timers pinning the event loop

`crates/js_runtime/src/js/timer_bootstrap.js` (new `__bgSetTimeout`),
`crates/browser/src/js/humanize.js` (uses `__bgSetTimeout`).

### Root cause

`humanize.js` synthesises sigma-lognormal mouse trajectories +
scroll events via ~30 `setTimeout` calls (50 ms-1.8 s spread) plus a
recurring `setInterval(runCycle, 4000)`. The existing
`UNREF_THRESHOLD_MS = 2000` means timers shorter than 2 s are NOT
unref'd, so the event loop sees them as pending work and `run_until_idle`
won't reach `AllWorkDone` until they all fire.

Real Chrome treats synthesised input events as background work that
doesn't gate navigation idle. Our humanize timers should match that
semantic — they need to fire eventually, but they shouldn't pin the
loop open.

### Fix

Two-part:

1. Add `__bgSetTimeout(callback, delay)` to `timer_bootstrap.js`. Same
   as `setTimeout` but always calls `Deno.core.unrefOpPromise` on the
   sleep promise. Semantically equivalent to Node's
   `setTimeout(...).unref()`.

2. `humanize.js` resolves `_sched = globalThis.__bgSetTimeout ||
   globalThis.setTimeout` once at startup, and uses `_sched(…)` for
   every event scheduling.

### Behavioural invariant

For anti-bot pages (Akamai/Kasada/DataDome/Cloudflare), the challenge
VM keeps the event loop alive on its own — humanize timers still fire
fully because something else pins idle. For benign pages, the engine
can exit `run_until_idle` as soon as the page's own work settles
(usually <100 ms); humanize timers that haven't fired yet are
background no-ops on a page that already returned.

Validated by:
- `cargo test --release -p browser --test anti_bot` — 85/85 pass
- `cargo test --release -p browser --test chrome_compat` — 414/414 pass
- `cargo test --release -p browser --test navigation_primitives` — 5/5 pass

### Impact

example.com per-nav: 2308 ms → **264 ms (8.7× on top of fixes 1+2).**

## Fix 4 — `PagePool::navigate(url)`: warm-isolate reuse

`crates/browser/src/pool.rs`, new method on `crates/browser/src/page.rs`
called `Page::navigate_warm(url)`.

### Why

After fixes 1-3 the per-navigation cost is **~250 ms** — the dominant
remaining cost is V8 isolate creation + bootstrap (~150 ms) and
page-instrumentation install (~80 ms). Both are amortizable: if you
navigate multiple URLs through the same engine instance, every page
after the first can reuse the warm isolate.

Playwright/Puppeteer get this for free — Chrome launches once, every
new page reuses the renderer. browser_oxide's existing `PagePool` only
supported `reload_html` on caller-supplied HTML; there was no URL-driven
warm-navigate path.

### API

```rust
let pool = browser::PagePool::new(4);
let mut page = pool.navigate(url, profile).await?;
let html = page.content();
pool.release(page); // back to the pool for the next URL
```

### Mechanism

`Page::navigate_warm(url)`:

1. Fetches the URL via the shared HTTP client (same cookie jar as cold).
2. Parallel-fetches external CSS + scripts.
3. Calls `Page::reset_warm_state()`:
   - `__cancelAllTimers()` (new in `timer_bootstrap.js`) — bumps a
     generation counter; every in-flight `setTimeout`/`setInterval`
     callback checks gen on resolve and bails if stale. Mass-cancels
     the previous page's timers in O(1).
   - Clears `_browser_oxide.__pendingNavigation`,
     `_browser_oxide.__fetchLog`, `window.__cookieWrites`,
     `window.__scriptErrors`, `__akamai_events`, `__jsCookies`.
4. `replace_dom` swaps DOM (also resets Rust-side `TimerState`).
5. Re-runs inline + prefetched external scripts in document order.
6. Re-installs `humanize.js` on the fresh DOM.
7. Re-installs DOMContentLoaded/load events + meta-refresh scanner.
8. Drains (500 ms cap — same as cold).

**Skipped vs cold path**: V8 isolate creation (~150 ms), bootstrap
scripts (`window_bootstrap.js`, `dom_bootstrap.js`, …),
page-instrumentation wrappers on `globalThis.fetch` / `document.cookie`
/ `XMLHttpRequest` (these persist on the warm isolate).

### Scope

Warm reuse handles **benign content extraction**. It does NOT run the
cookie-diff / pending-nav iteration loop that `Page::navigate` does for
anti-bot pages. The justification:

- Challenge VMs (Kasada ips.js, Akamai sensor_data, DataDome i.js)
  dominate runtime — usually 10-30 s on our V8 — so the ~150 ms saved
  by reusing the isolate is rounding error.
- Reproducing the 600+ line cookie-diff retry loop on a warm path is a
  separate project and a regression risk.

Caller flow for mixed workloads:

```rust
// Try warm path; if challenged, release and fall back to cold.
let page = match pool.navigate(url, profile.clone()).await {
    Ok(p) if !p.is_anti_bot_challenge() => p,
    Ok(p) => { pool.release(p); Page::navigate(url, profile, 3).await? }
    Err(_) => Page::navigate(url, profile, 3).await?,
};
```

### Impact

| Site | cold (`Page::navigate`) | **pool (`PagePool::navigate`)** |
|---|---:|---:|
| example.com (528 B) | 244 ms | **141 ms** |
| news.ycombinator.com (~35 KB) | 444 ms | **333 ms** |
| wikipedia.org/Main_Page (~230 KB) | 849 ms | **724 ms** |

## Where the time goes after all four fixes

Per-navigation breakdown for example.com, warm pool, 5-run median:

| Phase | ms |
|---|---:|
| HTTP fetch (warm client) | 25 |
| `reset_warm_state` (timer cancel + scrubs) | 0 |
| `replace_dom` | 1 |
| Inline + external script execute | 0 (no scripts) |
| `humanize.js` re-install | 1 |
| DOMContentLoaded/load fire + meta-refresh scan | 0 |
| Drain (500 ms cap, exits at AllWorkDone) | 110 |
| Other | 4 |
| **Total** | **141** |

The drain dominates and is bounded by tokio's tick granularity (100 ms)
+ the time it takes for the new humanize cycle's first sigma-lognormal
sample to fire on the new DOM (~10 ms). Below ~110 ms is unreachable
without changing the drain semantics.

## Files changed (summary)

```
 crates/browser/src/js/humanize.js           |  14 +-
 crates/browser/src/page.rs                  | 451 +++++++++++++++++++++++++++-
 crates/browser/src/pool.rs                  |  30 ++
 crates/event_loop/src/lib.rs                |  14 +-
 crates/js_runtime/src/js/timer_bootstrap.js |  47 +++
 5 files changed, 549 insertions(+), 7 deletions(-)
```

Plus two new benchmark binaries:
- `crates/browser/examples/nav_timed.rs` — per-engine timing harness
  (cold + pool, with phase split)
- `crates/browser/examples/sweep_metrics.rs` — full-corpus sweep with
  customer-facing metrics

## Regression coverage

- `cargo test --release -p browser --test chrome_compat -- --test-threads=1` — 414/414 pass
- `cargo test --release -p browser --test anti_bot -- --test-threads=1` — 85/85 pass
- `cargo test --release -p browser --test navigation_primitives -- --test-threads=1` — 5/5 pass
- Full 126-corpus pass-rate sweep across 4 BO profiles + 4 competitors:
  see `BENCHMARK_2026_05_24.md`.
