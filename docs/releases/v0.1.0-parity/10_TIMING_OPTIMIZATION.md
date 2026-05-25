# 10 — Timing optimization

**Status:** planning
**Scope:** per-page wall-clock + corpus throughput for v0.1.0
**Companion:** `09_MEMORY_OPTIMIZATION.md` — shares the warm-reuse and
`Page::drop` surfaces; read together if touching either path.

## 1. Headline truth

| Engine path | Median | p95 | p99 | Wall (126 sites) | Throughput |
|---|--:|--:|--:|--:|--:|
| **BO cold** (any profile) | 15.1 s | 92.9 s | 115.3 s | 46-54 min | 2.3-2.7 / min |
| BO pool (chrome, 97 partial) | **2.6 s** | 9.7 s | 61.4 s | 6.9 min (proj 9.0) | **14.0 / min** (proj) |
| Patchright | 3.5 s | 8.5 s | 13.3 s | 9.3 min | 13.6 / min |
| Playwright | 3.5 s | 8.9 s | 22.7 s | 10.0 min | 12.6 / min |
| Playwright + Stealth | 4.4 s | 23.6 s | 45.8 s | 16.7 min | 7.5 / min |
| Camoufox | 5.6 s | 9.5 s | 42.5 s | 15.0 min | 8.4 / min |

Source: `~/projects/browser_oxide_internal/benchmarks/baselines/2026-05-24/*.json` `summary.ms_median`,
`ms_p95`, `ms_p99`, `wall_total_ms`.

**Two truths**:

1. **Cold path is bottlenecked by per-URL V8 isolate creation.**
   Every cold navigation builds a fresh `deno_core::JsRuntime`
   (V8 isolate + bootstrap script execution + extension init). That
   spin-up is ~200-300 ms by itself; with the full
   `build_page_with_scripts_init_and_storage` pipeline it's the
   floor of every cold navigation.
2. **Pool path closes the gap to Patchright but has a known
   panic.** Warm-reuse via `PagePool` + `Page::navigate_warm` lets
   one V8 isolate handle many URLs sequentially with a
   `reset_warm_state` scrub between. 5.8× speedup on the sites it
   *does* complete. But the pool panics on `wellsfargo.com` (site 98)
   in a DOM-walk-cycle assertion — see §4.

Fix B (the build-phase drain restoration from 200 ms → 8 s, applied
2026-05-24 but uncommitted) is **already in the working tree**. It
restores async-challenge chains (AWS WAF, recaptcha, reddit verify)
that the over-eager 200 ms cap was killing. Cost on benign pages is
near-zero because `humanize.js` now routes through `__bgSetTimeout`
(unref'd) — see §5.

## 2. Per-profile cold-path timing breakdown

Source: `bo_*_cold.json` summary fields.

| Profile | Median | p95 | p99 | Wall (s) | RSS peak (MB) |
|---|--:|--:|--:|--:|--:|
| `chrome_148_macos` | 15115 | 92938 | 115298 | 2900 | 418.9 |
| `firefox_135_macos` | 15108 | 93424 | 107732 | 3027 | 471.9 |
| `iphone_15_pro_safari_18` | 15413 | 93365 | 116006 | 3241 | 444.7 |
| `pixel_9_pro_chrome_148` | 15108 | 93542 | 115286 | 2769 | 388.0 |

The medians cluster tightly at ~15.1 s — that's the **nav budget
floor** for the default host (`_ => 15_000` at
`crates/browser/src/page.rs:1741`). Most benign sites exit before that
via the fast-exit branch (`page.rs:1858-1891`, body > 50 KB + no CHL
marker + `readyState=="complete"`). Sites that don't exit fast burn
the full 15 s + the 25 s extension budget (`page.rs:1749-1754` —
`BROWSER_OXIDE_NAV_BUDGET_EXTEND_MS = 25_000` default), hence the
p95/p99 in the 90 s range.

The mobile-profile p99 is **+10-20 ms higher** than desktop on
chrome_148_macos. Two contributors:
- iPhone profile has a heavier "16 declined APIs" bootstrap (see
  `crates/stealth/src/presets.rs:780-794`).
- Pixel profile uses the Android-mobile UA + an empty plugin/mime
  array (`crates/stealth/src/presets.rs:741-743`), with TLS impersonation
  `chrome_147_android` (`crates/stealth/src/presets.rs:733`). The TLS
  client setup is slightly different and accept_ch handshakes can
  re-fetch.

None of these per-profile spreads are bugs — they're the cost of
fidelity. None are addressable for v0.1.0.

## 3. The pool path (existing)

### Architecture

`crates/browser/src/pool.rs:1-87`. One V8 isolate hosted on a
warm `Page` lives in `PagePool.idle_pages`. Callers do:

```rust
let pool = browser::PagePool::new(4);
let page = pool.navigate(url, profile.clone()).await?;
let html = page.content();
pool.release(page);   // back into the queue for the next caller
```

`PagePool::navigate` (`pool.rs:78-86`) is:

```rust
let mut page = self.acquire(Some(profile)).await?;
page.navigate_warm(url).await?;
Ok(page)
```

`Page::navigate_warm` (`crates/browser/src/page.rs:1223-1530`) is the
hot path. The full sequence:

1. HTTP fetch via the shared client (page.rs:1257-1259).
2. Parallel-fetch external CSS + scripts.
3. `reset_warm_state` (page.rs:1421) — scrub V8 state, see below.
4. `replace_dom` (page.rs:1425) — swap `DomState` + reset `TimerState`.
5. `location.href = …` + `reset_nav_pending` (page.rs:1447-1451).
6. `__syncCookiesFromNet` drain — 50 ms cap (page.rs:1455-1461).
7. Re-run inline + prefetched external scripts in document order with
   50 ms drain per script (page.rs:1466-1486).
8. Re-install `humanize.js` (page.rs:1492-1494).
9. DOMContentLoaded + load events via `setTimeout(0)` (page.rs:1499-1505).
10. Meta-refresh scanner (page.rs:1511-…).
11. Final drain (500 ms cap — see `docs/PERFORMANCE_2026_05_24.md
   §Fix 4`).

### What `reset_warm_state` clears

`crates/browser/src/page.rs:1145-1202` (full doc + impl). The
docstring is canonical; summary:

- **In-flight timers** via `__cancelAllTimers()` (a generation-counter
  bump in `crates/js_runtime/src/js/timer_bootstrap.js:15-18`). Mass
  cancellation in O(1).
- `_browser_oxide.__pendingNavigation` — spurious value from
  `location.href = …` setter.
- `_browser_oxide.__fetchLog` — DevTools-style network log (reset to
  empty array).
- `window.__cookieWrites`, `window.__scriptErrors` — instrumentation
  buffers.
- `globalThis.__akamai_events` (mouse/key/touch/scroll buffers + counters).
- `globalThis.__jsCookies` — cookie cache snapshot (HTTP client jar
  is the source of truth, re-synced via `__syncCookiesFromNet`).

### What stays across warm navigations (the win)

- V8 isolate itself (~150 ms saved).
- Bootstrap scripts (`window_bootstrap.js`, `dom_bootstrap.js`,
  `timer_bootstrap.js`, …) — entire shimmed JS surface (~50-100 ms saved).
- Page-instrumentation wrappers on `globalThis.fetch`,
  `document.cookie`, `XMLHttpRequest` — installed once at first build
  (page.rs:3076-3110).

### Performance gain (measured)

From `bo_chrome_148_macos_pool.json` vs `bo_chrome_148_macos_cold.json`:

- Median 2576 ms / 15115 ms = **5.8× speedup**.
- Throughput 14.0 / 2.6 pages/min projected = **5.4× throughput**.

Per `docs/PERFORMANCE_2026_05_24.md` (single-page benchmark):

| Site | Cold | **Pool** |
|---|--:|--:|
| example.com | 244 ms | **141 ms** |
| news.ycombinator.com | 444 ms | **333 ms** |
| wikipedia.org | 849 ms | **724 ms** |

## 4. Pool path bug — wellsfargo DOM cycle panic

### Symptom

From `~/projects/browser_oxide_internal/benchmarks/baselines/2026-05-24/bo_chrome_148_macos_pool.log:762`:

```
thread 'main' (22889) panicked at crates/dom/src/arena.rs:678:17:
DOM walk cycle in collect_elements from NodeId(0) — visited 100001 unique nodes
...
thread caused non-unwinding panic. aborting.
```

Site 98 of 126 (`wellsfargo.com`). The sweep aborts entirely; 29 more
sites never run. Pool RSS at abort: 1365 MB (also a memory issue —
see `09_MEMORY_OPTIMIZATION.md §6`).

### What `arena.rs:678` is

`crates/dom/src/arena.rs:670-683`:

```rust
let mut visited: HashSet<NodeId> = HashSet::with_capacity(64);
let mut steps: usize = 0;
while let Some(id) = stack.pop() {
    if !visited.insert(id) {
        continue;
    }
    steps += 1;
    if steps > WALK_LIMIT {
        panic!(
            "DOM walk cycle in collect_elements from {:?} — visited {} unique nodes",
            root,
            visited.len()
        );
    }
    ...
}
```

`WALK_LIMIT = 100_000` (`crates/dom/src/arena.rs:14`). The panic
guards against runaway tree walks. By construction, `append_child`
(line 153) and `insert_before` (line 192) reject cycles at mutation
time, so reaching 100k *unique* visited nodes means either:

1. The DOM legitimately has > 100k nodes (wellsfargo.com loaded with
   sync-fetched JS — see the log: 30+ chain-limit triggers in the
   minutes leading up to the panic indicate a script-built DOM).
2. The arena state is corrupted such that two different "logical"
   subtrees share node ids (warm-reuse state bleed).

### Root cause hypothesis

`Page::navigate_warm` calls `replace_dom` (page.rs:1425) which does
(`crates/js_runtime/src/lib.rs:169-179`):

```rust
let mut dom_state = DomState::new(dom);
dom_state.stylesheets = stylesheets;
dom_state.update_cached_rules();
state.put(dom_state);
```

This replaces `DomState` *as a whole* — the previous one is dropped.
But the previous page's `Dom` may have been shared via op-state
back-references or via JS-side `NodeId` handles. If a previous page's
script captured a JS object handle on `_browser_oxide.__previousNode`
or similar, and the new page's arena recycled the same slot (via
`free_list` at `arena.rs:7,28,261`), the next `set_inner_html` (which
walks via `collect_elements`) could splice the recycled node into a
cycle.

`op_dom_set_inner_html` (`crates/js_runtime/src/extensions/dom_ext.rs:452-479`):

```rust
let id = NodeId::from_raw(node_id as u32);
let fragment_dom = html_parser::parse_html(&format!("<body>{}</body>", html));
...
let old_children: Vec<NodeId> = state.dom.children(id);
for child in old_children {
    state.dom.remove(child);
}
if let Some(body_id) = body {
    for child_id in fragment_dom.children(body_id) {
        let new_child = state.dom.merge_subtree(&fragment_dom, child_id);
        state.dom.append_child(id, new_child);
    }
}
state.layout_engine.mark_dirty();
```

If `node_id` was captured by JS from a previous page and survives the
warm reset (e.g. a Promise/closure not GC'd by V8 yet), this could
`append_child` into a stale parent that now points back at an
ancestor — exactly the cycle pattern `collect_elements` would trip on.

### Fix plan

Two layered fixes:

#### 4.1 Defensive — make `replace_dom` *actually* invalidate stale node handles

`crates/js_runtime/src/lib.rs:167-179` should additionally bump a
generation counter (analogous to `_timerGen` in `timer_bootstrap.js`),
exposed to JS as `_browser_oxide.__domGen`. JS-side node handles
captured by user code (`document.querySelector(...).remember = …`)
should be invalidated on a generation mismatch by the op layer.

Pseudo-patch:

```rust
// in DomState
pub struct DomState {
    pub gen: u32,       // bumps on every replace_dom
    pub dom: Dom,
    ...
}

// in replace_dom
let mut dom_state = DomState::new(dom);
dom_state.gen = prev_gen + 1;
state.put(dom_state);
```

Every `op_dom_*` op checks the generation tag on the incoming NodeId.
Mismatch → return `-1` (treated as missing by JS).

#### 4.2 Investigate — capture the actual graph at the cycle point

Before committing the defensive fix, root-cause it for real:

```bash
# Repro
cat > /tmp/just_wellsfargo.json <<'JSON'
[{"cat":"gov-bank","name":"wellsfargo","url":"https://www.wellsfargo.com/"}]
JSON

# Run as the 98th call in a corpus, with previous 97 sites varied
# enough to surface the bleed. Probably easiest is to use the same
# corpus order that hit the panic in the original sweep:
head -98 tests/holistic_corpus_v2.json > /tmp/repro_corpus.json
BROWSER_OXIDE_SWEEP_POOL=1 RUST_LOG=js_runtime=trace,dom=trace,browser=debug \
    target/release/examples/sweep_metrics chrome_148_macos \
        /tmp/repro_corpus.json /tmp/out.json \
        2>&1 | tee /tmp/wellsfargo_repro.log

# Grep for the last ~200 lines before the panic
tail -200 /tmp/wellsfargo_repro.log | grep -E "replace_dom|set_inner_html|append_child|cycle"
```

The first concrete answer should be: does the cycle appear on the
*first* iteration after warm-reuse, or only after wellsfargo's own
inline scripts have run? That tells us "state bleed" vs "wellsfargo's
own DOM-building script triggers it independent of pool path" — and
the cycle would then also reproduce on cold, ruling pool out.

If cold reproduces the cycle → it's a wellsfargo-specific arena bug
even with a fresh isolate, not warm-reuse related. File pointers:

- `crates/dom/src/arena.rs:140-160` — `append_child` cycle assertion
- `crates/dom/src/arena.rs:184-200` — `insert_before` cycle assertion
- `crates/dom/src/arena.rs:540-575` — `merge_subtree` (called from
  `set_inner_html`)
- `crates/dom/src/arena.rs:655-699` — `collect_elements` (panic site)

### Acceptance gate

- Repro the panic on cold path. If reproducible → fix the arena bug
  upstream, the pool issue evaporates. If NOT reproducible → 4.1
  defensive fix is needed.
- 3-run full 126-site pool sweep completes without panic.
- 3-run RSS aggregation in `14_TESTING_VALIDATION.md` shows pool
  median ms within ±10% across runs.

## 5. Fix B post-mortem — build-phase drain restored 200 ms → 8 s

**Origin**: A 2026-05-24 perf-pass set `build_page` final drain to
200 ms based on `docs/PERFORMANCE_2026_05_24.md §Bug 2` (which
correctly identified that humanize timers were pinning the drain).
That was right for benign pages but **wrong for async-challenge
pages** — AWS WAF challenge.js, reddit's `requestSubmit()` POST,
recaptcha's invisible-token resolve all need 2-8 s of post-build
async work to set `__pendingNavigation` and trigger iter 1.

`crates/browser/src/page.rs:3385-3402`:

```rust
// 8 s cap. This drain flushes microtasks, zero-delay setTimeouts
// (DOMContentLoaded / load handlers, meta-refresh scanner), and
// any short async chains kicked off by inline scripts.
//
// Important: `humanize.js` now schedules its synthetic mouse /
// scroll timers via `globalThis.__bgSetTimeout` (timer_bootstrap.js)
// which is `.unref()`'d — so the humanize setTimeouts no longer
// pin this drain to its full ceiling. Benign pages exit idle in
// milliseconds; anti-bot challenge pages (AWS WAF / reddit verify /
// recaptcha invisible / DataDome) get the full 8 s they need for
// their async POST+reload chain to complete, which sets
// `__pendingNavigation` and triggers iter 1 with the proper token
// cookie. Cutting this below ~5 s causes those chains to never
// complete and the outer loop returns the challenge stub as the
// "rendered" page.
if let Err(e) = event_loop.run_until_idle(Duration::from_secs(8)).await {
    tracing::warn!(error = %e, "Event loop error during run");
}
```

**Why it's free on benign pages**: `humanize.js` (`crates/browser/src/js/humanize.js:52`):

```js
const _sched = globalThis.__bgSetTimeout || globalThis.setTimeout;
```

…and every synthetic mouse/scroll/key timer at lines 268, 279 uses
`_sched`. `__bgSetTimeout` (`crates/js_runtime/src/js/timer_bootstrap.js:91-107`)
*always* calls `Deno.core.unrefOpPromise` on the sleep promise, so
those 30+ in-flight tokio sleeps don't count as "pending refed ops"
and `run_until_idle` reaches `AllWorkDone` as soon as the page's own
work settles. The 8 s drain becomes a 50-200 ms drain on benign pages
and a real 2-8 s window for challenge chains.

### Validation evidence

From the working-tree spot-check (2026-05-24):

| Site | Pre-fix-B body | Post-fix-B body | Cause |
|---|--:|--:|---|
| adidas.com | 2.5 KB | 1.3 MB | Akamai BMP needed > 200 ms to set its cookie |
| amazon-jp | 2 KB | 822 KB | AWS WAF challenge.js POST+reload needed > 200 ms |
| x.com | 69 B | OK | SPA hydration window |

Per-profile full-sweep aggregate not yet re-run with fix B applied;
this is captured in §8 acceptance.

### Files (fix B in tree)

- `crates/browser/src/page.rs:3385-3402` — the restored 8 s drain
- `crates/browser/src/page.rs:3389-3399` — explanatory comment
- `crates/browser/src/js/humanize.js:40-52` — `_sched` resolution
- `crates/browser/src/js/humanize.js:268,279` — `_sched(…)` callsites
- `crates/js_runtime/src/js/timer_bootstrap.js:81-107` — `__bgSetTimeout` definition

## 6. Future timing improvements

### 6.1 V8 snapshot warming

`crates/js_runtime/src/runtime.rs:39` already exposes
`pub startup_snapshot: Option<&'static [u8]>` and the runtime
constructor uses it (`crates/js_runtime/src/runtime.rs:132,
191-192`):

```rust
startup_snapshot: options.startup_snapshot,
...
// Execute bootstrap JS only if NOT starting from snapshot
if options.startup_snapshot.is_none() {
    ...
}
```

But the build system doesn't produce a snapshot — every isolate
re-runs all bootstrap scripts cold. Pre-baking
`window_bootstrap.js` + `dom_bootstrap.js` + `timer_bootstrap.js` +
stealth init into a `[u8]` snapshot at `cargo build` time and
embedding it via `include_bytes!` should knock 100 ms off cold-path
isolate creation.

**Risk**: snapshots are profile-specific. The bootstrap scripts read
profile-config from a `[state]` set at runtime, but if any bootstrap
JS *evaluates* profile-dependent expressions at startup (timezone,
language, etc.), the snapshot would freeze the wrong values. Audit
required.

**File pointers**:

- `crates/js_runtime/src/runtime.rs:113-139` — RuntimeOptions wiring
- `crates/js_runtime/src/runtime.rs:191-227` — bootstrap-skip branch
- Build-time snapshot generation — needs new file
  (`crates/js_runtime/build.rs`).

### 6.2 Adaptive nav budget — already exists

`crates/browser/src/page.rs:1858-1909`:

```rust
// 1. FAST-EXIT — body > 50 KB AND no CHL marker AND readyState
//    "complete" → the site rendered cleanly, return it now.
//    Skips iter 1 and iter 2 entirely. Closes the dominant
//    fast-site stall in the holistic sweep (where every fast
//    page used to wait the full 50 s budget for nothing).
//
// 2. EXTEND — body > 50 KB but readyState still "loading"
//    (e.g. footlocker, walmart pre-paint). Give one extension.
```

And the SPA-fast-exit branch at `page.rs:1943` — mount has children
→ return early. These already exist; mention them so contributors
don't reinvent. The cold-median 15 s is *after* fast-exit takes most
benign sites out of the 90 s tail.

### 6.3 Per-host budget knobs — already exists

`crates/browser/src/page.rs:1697-1742` — host-specific budget table:

```rust
Some(h)
    if h.ends_with("canadagoose.com")
        || h.ends_with("hyatt.com")
        || ... => 45_000,
Some(h)
    if h.ends_with("twitter.com")
        || h.ends_with("x.com")
        || ... => 90_000,
Some(h) if h.ends_with("homedepot.com") => 45_000,
Some(h)
    if h.ends_with("bestbuy.com")
        || h.ends_with("nike.com")
        || ... => 25_000,
_ => 15_000,
```

`page.rs:1743-1754` — both base and `_EXTEND_MS` env-overridable:

```rust
let mut nav_budget = Duration::from_millis(
    std::env::var("BROWSER_OXIDE_NAV_BUDGET_MS")
        .ok().and_then(|s| s.parse().ok())
        .unwrap_or(host_budget_default_ms),
);
let nav_budget_extend = Duration::from_millis(
    std::env::var("BROWSER_OXIDE_NAV_BUDGET_EXTEND_MS")
        .ok().and_then(|s| s.parse().ok())
        .unwrap_or(25_000),
);
```

Document for contributors; not a code change for v0.1.0.

### 6.4 Parallel cold sweep across N threads

Per `CLAUDE.md`: "V8 isolates are per-thread; running multi-threaded
crashes the test process. CI enforces `--test-threads=1`." That's per
*test process*, not per *binary*. The cold sweep harness
(`crates/browser/examples/sweep_metrics.rs`) runs one thread today.
We could spawn N tokio runtimes on N OS threads, each with its own
V8 isolate, splitting the corpus across them.

Sub-linear speedup expected (network is shared, target-site rate
limits hit faster with parallelism), but a 4-way parallel sweep
should cut wall from 46 min → ~15 min — competitive with Camoufox
without needing to fix the cold path's per-isolate cost.

**Status**: spec only. Not in v0.1.0 critical path.

### 6.5 Investigate persisting page-instrumentation across pool resets

The cookie-write / fetch-log / error-tracking wrappers at
`page.rs:3076-3110` are installed by `build_page_with_scripts...`,
not by `navigate_warm`. They already persist across warm
navigations (that's documented at `page.rs:1166-1171`). No change
needed — but worth noting so contributors don't accidentally
re-install them in `navigate_warm` (would be a regression).

## 7. Target numbers for v0.1.0

| Metric | Current | v0.1.0 target | Stretch |
|---|--:|--:|--:|
| Cold path median | 15.1 s | **keep (15-17 s)** | — |
| Cold path p99 | 115 s | ≤ 120 s | ≤ 90 s |
| Cold path wall (126 sites) | 46-54 min | ≤ 55 min | — |
| Pool path median | 2.6 s | **≤ 3 s (= Patchright)** | ≤ 2 s |
| Pool path p95 | 9.7 s | ≤ 9 s (= Patchright) | ≤ 7 s |
| Pool path completes 126 sites | 97 (panic on 98) | **126 (no panic)** | ✓ |
| Pool throughput | 14.0 / min (proj) | ≥ 13.5 / min (= Patchright 13.6) | ≥ 20 / min |

The cold-path median is **not improvable in v0.1.0** without
attacking the per-URL isolate creation cost — and that's the
snapshot-warming work in §6.1, which is risk-bound. The pool path
is the real customer story for high-throughput scraping; closing
the wellsfargo panic + the §6.5 polish gets it to parity with
Patchright on benign sites *with strictly higher pass rate on
anti-bot sites*.

## 8. Acceptance checklist (gates v0.1.0)

- [ ] **wellsfargo panic root-caused.** Bash command in §4.2 produces
      a clear "cold-repros vs warm-only" verdict; finding documented
      in `15_OPEN_QUESTIONS.md`.
- [ ] **wellsfargo panic fixed** — either upstream arena fix or §4.1
      defensive generation-bump, committed with regression test.
- [ ] **Full 126-site pool sweep completes without panic** —
      `target/release/examples/sweep_metrics chrome_148_macos
      tests/holistic_corpus_v2.json /tmp/pool_sweep.json` with
      `BROWSER_OXIDE_SWEEP_POOL=1` set.
- [ ] **Multi-run aggregation** — 3 pool sweeps; median ms variance
      ≤ 10% (anchored in `14_TESTING_VALIDATION.md`).
- [ ] **Fix B (drain restore) committed** as a separate logical
      commit per `00_README.md §"Memory-mode notes"`. Already in
      working tree.
- [ ] **Fix B validated on 126-site cold sweep** — pass count
      delta within ±5 noise floor (per `docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md`)
      and the +pass sites match the spot-check (adidas / amazon-jp /
      reddit / WAF cluster).
- [ ] **V8 snapshot warming investigated** — either committed with
      A/B evidence per §6.1, or explicitly deferred in
      `15_OPEN_QUESTIONS.md` with reason.
- [ ] **Cold p99 documented as inherent** — 115 s is the
      nav_budget × max_iterations envelope; nothing to "fix" without
      cutting pass rate.

## 9. What is explicitly NOT in scope

- Reimplementing the cookie-diff / pending-nav iteration loop on the
  warm path. Documented at `Page::navigate_warm` doc-comment
  (`page.rs:1216-1222`) — "challenge VMs dominate runtime so the
  ~150 ms saved by reusing the isolate is rounding error". Callers
  who hit a challenge on warm path are expected to release and fall
  back to cold (pattern in `docs/PERFORMANCE_2026_05_24.md
  §Fix 4 — Caller flow`).
- Replacing `deno_core` with a tighter V8 binding. Out of scope; see
  `15_OPEN_QUESTIONS.md`.
- Speculative parallel pool navigations on a single isolate (V8 is
  single-threaded per the `CLAUDE.md` convention).

## 10. Files referenced

Engine source (all paths absolute under
`/home/yfedoseev/projects/browser_oxide`):

- `crates/browser/src/page.rs:200-214` — `Page` struct
- `crates/browser/src/page.rs:216-233` — `Drop for Page` (worker
  reap — companion to `09_MEMORY_OPTIMIZATION.md §4`)
- `crates/browser/src/page.rs:380-416` — `Page::reload_html`
- `crates/browser/src/page.rs:1145-1202` — `Page::reset_warm_state`
  (canonical doc of what clears between pages)
- `crates/browser/src/page.rs:1223-1530` — `Page::navigate_warm`
- `crates/browser/src/page.rs:1257-1259` — warm path: shared HTTP
  client acquisition
- `crates/browser/src/page.rs:1421-1431` — `reset_warm_state` +
  `replace_dom` callsites in warm path
- `crates/browser/src/page.rs:1439-1442` — V8 deadline watcher for
  warm-path build
- `crates/browser/src/page.rs:1455-1461` — `__syncCookiesFromNet`
  50 ms drain (warm path)
- `crates/browser/src/page.rs:1466-1486` — warm-path script execute
  loop with per-script 50 ms drain
- `crates/browser/src/page.rs:1697-1742` — per-host budget table
- `crates/browser/src/page.rs:1743-1754` — `BROWSER_OXIDE_NAV_BUDGET_MS` +
  `_EXTEND_MS` env knobs
- `crates/browser/src/page.rs:1758-1770` — outer iteration loop bail
- `crates/browser/src/page.rs:1820-1849` — `V8DeadlineWatcher` install
  + drain timeout calc
- `crates/browser/src/page.rs:1858-1909` — fast-exit / extend branch
- `crates/browser/src/page.rs:1943` — SPA-fast-exit branch (mount has
  children)
- `crates/browser/src/page.rs:3040-3072` — `__syncCookiesFromNet`
  50 ms drain (cold path)
- `crates/browser/src/page.rs:3076-3110` — cookie-write instrumentation
- `crates/browser/src/page.rs:3380-3402` — **fix B: build_page final
  8 s drain (restored 2026-05-24)**
- `crates/browser/src/pool.rs:1-87` — `PagePool` API
- `crates/browser/src/pool.rs:42` — `reload_html` on acquire (warm
  scrub)
- `crates/browser/src/pool.rs:78-86` — `PagePool::navigate`
- `crates/browser/src/js/humanize.js:40-52` — `_sched` resolution
- `crates/browser/src/js/humanize.js:268,279` — `_sched(...)` schedule
  callsites for synthetic input
- `crates/browser/examples/sweep_metrics.rs:1-100` — sweep harness
  preamble
- `crates/browser/examples/sweep_metrics.rs:101-135` — pool vs cold
  branch + first-page-ready timing
- `crates/browser/examples/sweep_metrics.rs:138-197` — per-site
  navigate loop + RSS accumulation
- `crates/dom/src/arena.rs:5-19` — `Dom` + `WALK_LIMIT = 100_000` +
  `ANCESTOR_LIMIT`
- `crates/dom/src/arena.rs:140-200` — `append_child` / `insert_before`
  cycle assertions
- `crates/dom/src/arena.rs:256-263` — `Dom::remove` (slot recycle into
  `free_list`)
- `crates/dom/src/arena.rs:540-575` — `merge_subtree` (called by
  `set_inner_html`)
- `crates/dom/src/arena.rs:655-699` — **`collect_elements` (panic site
  at line 678)**
- `crates/event_loop/src/lib.rs:1-60` — event-loop profiling preamble
  (`BROWSER_OXIDE_EVENT_LOOP_PROFILE=1`)
- `crates/js_runtime/src/lib.rs:167-179` — `replace_dom`
- `crates/js_runtime/src/runtime.rs:36-50` — `BrowserRuntimeOptions`
  with `startup_snapshot`
- `crates/js_runtime/src/runtime.rs:98-111` — `HEAP_INITIAL`
- `crates/js_runtime/src/runtime.rs:113-139` — `JsRuntime::new`
- `crates/js_runtime/src/runtime.rs:191-227` — snapshot-skip bootstrap
  branch (§6.1 candidate)
- `crates/js_runtime/src/extensions/dom_ext.rs:452-479` —
  `op_dom_set_inner_html` (panic-trigger callsite)
- `crates/js_runtime/src/extensions/worker_ext.rs:179-202` —
  `notify_parent` Notify mechanism (W5b-deep fix referenced in
  bootstrap comments)
- `crates/js_runtime/src/extensions/worker_ext.rs:436-485` —
  `op_worker_await_message` (async, no polling — replaces W5b
  `setInterval(5)` pump that pinned the loop)
- `crates/js_runtime/src/js/timer_bootstrap.js:1-60` —
  `__cancelAllTimers` + `_maybeUnref` + `UNREF_THRESHOLD_MS = 2000`
- `crates/js_runtime/src/js/timer_bootstrap.js:62-79` —
  `setTimeout`
- `crates/js_runtime/src/js/timer_bootstrap.js:81-107` —
  **`__bgSetTimeout` (always-unref'd)** — the load-bearing helper
  for fix B
- `crates/js_runtime/src/js/timer_bootstrap.js:109-133` —
  `setInterval`
- `crates/stealth/src/presets.rs:120` — `chrome_148_macos`
- `crates/stealth/src/presets.rs:413` — `firefox_135_macos`
- `crates/stealth/src/presets.rs:690-772` — `pixel_9_pro_chrome_148`
  (with bootstrap-weight notes relevant to §2)
- `crates/stealth/src/presets.rs:780-815` — `iphone_15_pro_safari_18`
  ("16 declined APIs" rationale)

Benchmark / data:

- `benchmarks/bench_corpus_v2.py:109-179` — `visit` (per-site
  measurement)
- `benchmarks/bench_corpus_v2.py:287-330` — `aggregate` (median /
  p95 / p99 / throughput computation, same formula as sweep_metrics
  for cross-engine apples-to-apples)
- `crates/browser/examples/nav_timed.rs` — per-engine single-page
  timing harness (companion to sweep_metrics; used in
  `docs/PERFORMANCE_2026_05_24.md`)

Sweep JSONs (timing data source):

- `~/projects/browser_oxide_internal/benchmarks/baselines/2026-05-24/bo_chrome_148_macos_cold.json` —
  median 15115 ms, p95 92938, p99 115298, wall 2900 s
- `~/projects/browser_oxide_internal/benchmarks/baselines/2026-05-24/bo_firefox_135_macos_cold.json` —
  median 15108, p95 93424, p99 107732, wall 3027 s
- `~/projects/browser_oxide_internal/benchmarks/baselines/2026-05-24/bo_iphone_15_pro_safari_18_cold.json` —
  median 15413, p95 93365, p99 116006, wall 3241 s
- `~/projects/browser_oxide_internal/benchmarks/baselines/2026-05-24/bo_pixel_9_pro_chrome_148_cold.json` —
  median 15108, p95 93542, p99 115286, wall 2769 s
- `~/projects/browser_oxide_internal/benchmarks/baselines/2026-05-24/bo_chrome_148_macos_pool.json` —
  median 2576, p95 9731, p99 61381, wall 415 s (97 sites, panic on
  98); `summary._note` documents the panic + JSON reconstruction
  from log
- `~/projects/browser_oxide_internal/benchmarks/baselines/2026-05-24/bo_chrome_148_macos_pool.log:762-783` —
  panic capture (wellsfargo collect_elements cycle)
- `~/projects/browser_oxide_internal/benchmarks/baselines/2026-05-24/comp_camoufox.json` — median 5603, p95
  9511, p99 42543, wall 899 s
- `~/projects/browser_oxide_internal/benchmarks/baselines/2026-05-24/comp_patchright.json` — median 3470,
  p95 8484, p99 13270, wall 558 s
- `~/projects/browser_oxide_internal/benchmarks/baselines/2026-05-24/comp_playwright.json` — Playwright
  baseline

Sibling chapters:

- `00_README.md` — release plan overview
- `01_CURRENT_STATE.md` — headline numbers (timing table is the data
  source for §1 here)
- `09_MEMORY_OPTIMIZATION.md` — companion; same warm-reuse + drop
  surfaces; §6 of that doc covers the DOM-arena retain that
  compounds the pool RSS / pool panic relationship
- `docs/PERFORMANCE_2026_05_24.md` — the 4-bug post-mortem (cold
  6585 ms → pool 141 ms) — read for the historical context of how
  the pool path arose and why the drain caps were tuned the way
  they were
- `docs/BENCHMARK_2026_05_24.md` — narrative for the full sweep
- `docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md` — ±5 noise floor (used
  for §8 acceptance gate)
