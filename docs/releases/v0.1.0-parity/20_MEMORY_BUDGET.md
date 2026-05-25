# 20 — Memory budget per subsystem

**Status:** planning
**Scope:** translates the v0.1.0 "≤ 350 MB cold / ≤ 800 MB pool" target
into per-component caps with concrete reduction tactics.
**Companion:** `09_MEMORY_OPTIMIZATION.md` (high-level RSS triage),
`10_TIMING_OPTIMIZATION.md` (pool-path + drain interactions),
`21_V8_SNAPSHOT_PARALLEL_COLD.md` (the snapshot-warming reduction tactic
referenced from §3.1 below), `14_TESTING_VALIDATION.md` (the
multi-run validation regime that gates these numbers).

## 1. Why this chapter exists

`09_MEMORY_OPTIMIZATION.md` answers "is the perceived 10× memory gap
real?" (mostly no — Camoufox measurement bug) and "what are the two
biggest wins?" (worker reap C, DOM-arena retain). What it does NOT do
is **decompose the 388-472 MB cold peak into named subsystems** and
hand each one a budget. Without that decomposition, contributors don't
know which file to open when the next 50-MB regression lands.

This chapter is the spreadsheet view: every named allocator that
contributes ≥ 1 MB to steady-state RSS, the file:line where it lives,
the current measured/estimated bytes, the v0.1.0 target, and the
reduction tactic. It is the budget contributors should reference before
any change that adds a `Vec`, a `HashMap`, or a per-page V8 closure.

## 2. Where the 388-472 MB goes — full decomposition

Source for "current MB": `summary.rss_peak_mb` and per-result `rss_mb`
trajectories in `~/projects/browser_oxide_internal/benchmarks/baselines/2026-05-24/bo_*_cold.json`, combined
with line-level reads of each subsystem. Where direct measurement
isn't available the entry is marked *(est.)*.

| # | Subsystem | File:line | Current MB | Target MB | Reduction tactic |
|---|---|---|--:|--:|---|
| 1 | V8 isolate baseline (per cold runtime) | `crates/js_runtime/src/runtime.rs:109-111` | 50-70 *(est.)* | 30-50 | Snapshot warming (see `21_V8_SNAPSHOT_PARALLEL_COLD.md §A`) + drop `HEAP_INITIAL` to 256 MB |
| 2 | Bootstrap JS heap (closures + maps from the 15 boot scripts) | `crates/js_runtime/src/runtime.rs:193-219` | ~10 *(est.)* | ~5 | Bake into V8 startup snapshot (see `21 §A.3`) |
| 3 | Per-page `Dom` arena (steady) | `crates/dom/src/arena.rs:5-8` | 5-50 | 5-50 (drops with `Page`) | None at steady state — see §3.5 for capacity-retain mitigation |
| 4 | Per-page `LayoutEngine` state | `crates/layout/src/engine.rs:21-33` | 2-10 *(est.)* | 2-10 | None — drops with `DomState` |
| 5 | Stylesheet cache (parsed `CachedRule` Vec) | `crates/js_runtime/src/state.rs:16-18,78-105` | 5-30 *(est.)* | 5-30 | Bounded LRU keyed by stylesheet bytes hash (§3.4) |
| 6 | Worker threads — leaked, now reaped | `crates/js_runtime/src/extensions/worker_ext.rs:407-423` | 0-200 (was leaking) | 0 | ✅ Fix C applied (see `09 §4`) |
| 7 | `__fetchLog` (per-page instrumentation) | `crates/browser/src/page.rs:3076-3110,3211,3247` | 1-5 *(est.)* | 0-1 | Cap entries at 200; drop request/response bodies past 100 (§3.6) |
| 8 | `__scriptErrors` (per-page error capture) | `crates/browser/src/page.rs:3112-3120` | < 1 | < 1 | Already informally capped at first ~50 entries by site behaviour |
| 9 | `__akamai_events` (mouse/key/touch/scroll buffers) | `crates/browser/src/page.rs:1186-1198` | < 1 | < 1 | None — already cleared in `reset_warm_state` |
| 10 | `__cookieWrites` | `crates/browser/src/page.rs:3097-3099` | < 1 | < 1 | Hard cap of 100 already in source — keep |
| 11 | `SharedSession` (cookies + accept_ch, process-wide) | `crates/net/` (cookie jar + accept_ch set) | 1-5 *(est.)* | 1-5 | None — process-global, does not grow per page |
| 12 | Per-page DOM peak transient (during parse + script eval) | `crates/dom/src/arena.rs:5-8` (live use) | 30-100 | 30-100 | Drops with `Page`; no leak (§3.5 covers the warm-pool ratchet) |
| 13 | Layout box cache (per-page) | `crates/layout/src/engine.rs:21-33` | 5-30 *(est.)* | 5-30 | Drop with `DomState` (§3.4) |
| 14 | `resource_timings` (performance API buffer) | `crates/js_runtime/src/state.rs:33,74` | < 1 (typical) — unbounded by design | < 1 | Cap at 500 entries to match the WHATWG default resource-timing buffer (§3.7) |
| 15 | `BlobRegistry` (`URL.createObjectURL` table) | `crates/js_runtime/src/extensions/worker_ext.rs:24-43` | 0-50 *(est.)* (unbounded) | 0-10 | LRU + per-blob TTL; revoke after N seconds of inactivity (§3.8) |
| 16 | `boring2` + `h2` connection pool | `crates/net/` | 5-20 *(est.)* | 5-20 | Already bounded by `hyper`/`h2` defaults |
| 17 | V8 isolates of orphaned iframes (child realms) | `crates/js_runtime/src/native_fns.rs` (IframeRealmStore) | 0-30 *(est.)* | 0-30 | Children drop with parent `Page::drop` already (`page.rs:230-232`) |

Totals (chrome_148_macos cold sweep, 126 sites at peak):
- Sum of "current MB" mid-range: ≈ 130 MB true component baseline.
- Observed peak: 419 MB.
- Gap: ≈ 289 MB → attributable to worker leak (now reaped → -100 MB
  expected), V8 fragmentation under `HEAP_INITIAL = 1 GB` (~80-150 MB
  retained working set), and small accumulators (§3.6 / §3.7 / §3.8).

The decomposition is necessarily fuzzy — V8's malloc + JIT scratch are
not directly attributable to source structs. The point is the *order
of magnitude* per row, which is enough to direct optimisation effort.

## 3. The "ratchet" pattern — why RSS grows non-monotonically

Per `09_MEMORY_OPTIMIZATION.md §2`, the cold-sweep RSS curve grows
roughly **+2.40 MB per site** with a 6:1 step-up:step-down imbalance
(74 step-ups vs 12 step-downs). That's not normal allocator churn;
it's a **ratchet** — components that grow once per site and never
shrink between pages.

Components that ratchet:

| Component | Ratchet mechanism | Mitigation status |
|---|---|---|
| V8 isolate `malloc` arenas | glibc `malloc_trim` doesn't run automatically; V8 retains large old-space pages within its `HEAP_MAX` reservation | Mitigated by `low_memory_notification` (`09 §5` proposal) |
| Worker leak | Each `new Worker()` spawned a 64 MB-stack OS thread; never reaped | ✅ Fixed by C (`drain_owned_workers` at `worker_ext.rs:407-423`) |
| `BlobRegistry` | `URL.createObjectURL` adds entries; sites do not reliably `revokeObjectURL` | Open — §3.8 |
| `SharedSession` `accept_ch` set | Grows once per new origin; tiny but monotonic | Acceptable — the set saturates at corpus diversity (~100 origins) |
| `resource_timings` (within a single isolate's lifetime) | Per-page accumulator; unbounded `Vec` | Open — §3.7 |
| V8 JIT code cache | Compiled functions stay cached across navigations on the same isolate (warm pool) | Acceptable on cold path (isolate drops); large contributor to pool ratchet |

The cold path ratchet is dominated by #1 + #2 (V8 malloc arenas +
worker leak). The pool path ratchet adds #3 + #6 + V8 JIT cache + the
DOM-arena Vec capacity retain (`09 §6`).

## 4. Per-subsystem reduction tactics — detailed specs

Each of these is a concrete change with a file:line target. The order
mirrors the budget table in §2.

### 4.1 V8 `HEAP_INITIAL` 1 GB → 128 / 256 MB

**File**: `crates/js_runtime/src/runtime.rs:109-111`:

```rust
const HEAP_INITIAL: usize = 1024 * 1024 * 1024; // 1 GB initial
const HEAP_MAX: usize = 4 * 1024 * 1024 * 1024; // 4 GB max
let create_params = deno_core::v8::CreateParams::default().heap_limits(HEAP_INITIAL, HEAP_MAX);
```

**Spec**:
- Drop `HEAP_INITIAL` to 256 MB (or 128 MB if creepjs A/B passes).
- Keep `HEAP_MAX = 4 GB` (so creepjs's lie-detection pass still has
  headroom — the 1 GB initial was set specifically because creepjs
  was hitting `Builtins_ArrayPrototypePush` OOM at ~1.8 GB; the
  *initial* doesn't change the *max*).
- Pair with `isolate_handle().low_memory_notification()` in `Page::drop`
  so V8 actively returns pages to the OS (currently it grows lazily
  within its reservation and rarely shrinks).

**Tradeoff per the inline comment** (`runtime.rs:104-108`): a 256 MB
initial caused early-growth GC pauses on creepjs. The proposal is to
*combine* the smaller initial with the explicit memory hint, so V8
isn't compacting on the hot path.

**Validation gate** (per `09 §5`): 3-run 126-site sweep before/after.
ΔPass ≥ -2 sites (within ±5 noise per
`docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md`); Δcreepjs body-len within
±5%; ΔRSS peak ≥ -50 MB.

**Expected saving**: 30-100 MB peak cold (V8 stops retaining ~1 GB of
reservation; resident set follows the actual working set).

### 4.2 V8 startup snapshot — bake bootstrap JS

Handoff to `21_V8_SNAPSHOT_PARALLEL_COLD.md §A`. The chapter you're
reading owns the *budget* (bootstrap JS heap, row #2 in §2). The 21
chapter owns the *implementation*.

**Expected saving** (per 21 §A.5): -20 to -30 MB per cold isolate +
-80 to -100 ms cold-start time.

**File targets**: `crates/js_runtime/src/snapshot.rs` (bootstrap is
already executed at snapshot-build time at `snapshot.rs:67-99` — the
file does the right thing today; the gap is that not every bootstrap
script is in the snapshot list).

### 4.3 Layout box cache — drop with `DomState`, no LRU needed

**File**: `crates/layout/src/engine.rs:21-33`:

```rust
pub struct LayoutEngine {
    // ...
    dom_to_taffy: HashMap<u32, taffy::NodeId>,
    // ...
}
```

**Current behaviour**: `LayoutEngine` is owned by `DomState`
(`crates/js_runtime/src/state.rs:9`). When `replace_dom` runs
(`crates/js_runtime/src/lib.rs:167-179`), the old `DomState` is
dropped and a fresh `LayoutEngine::new()` is built. So on the **cold**
path layout doesn't ratchet — it ratchets only on the **warm pool**
path via the underlying `taffy` arena reuse.

**Spec**: no change for v0.1.0 — the per-page layout cache is bounded
by DOM size and dropped with the page. Document this so contributors
do not mistakenly add an LRU and confuse the lifecycle.

**Pool-path note**: see §5 for the spec to call `shrink_to_fit` on
`dom_to_taffy` from `reset_warm_state`.

### 4.4 Stylesheet cache — bounded LRU on the warm path

**File**: `crates/js_runtime/src/state.rs:16-18`:

```rust
pub stylesheets: Vec<String>,
pub cached_rules: Vec<CachedRule>,
```

`update_cached_rules` (`state.rs:78-106`) reparses every stylesheet
on every `replace_dom`. On the cold path it drops with the page; on
the warm pool path the previous page's `cached_rules` is replaced by
the new one (`replace_dom` builds a fresh `DomState`).

**Spec for v0.1.0**: no bounded LRU is needed yet — `cached_rules` is
already bounded by the *current* page's stylesheet count and dropped
on `replace_dom`. The structure to watch is `stylesheets: Vec<String>`
which holds raw CSS text. For sites with 50 KB+ of inline CSS this
adds up. Add `stylesheets.shrink_to_fit()` after `update_cached_rules`
to prevent Vec capacity retain across warm navigations.

**File:line**: add `self.stylesheets.shrink_to_fit()` after line 81 in
`state.rs:78-106` (`update_cached_rules`).

**Expected saving**: 1-5 MB on pool path; negligible on cold.

### 4.5 Per-page DOM `shrink_to_fit` on `Page::drop` and pool reset

**File**: `crates/dom/src/arena.rs:5-8`:

```rust
pub struct Dom {
    nodes: Vec<Option<Node>>,
    free_list: Vec<usize>,
}
```

`Dom::remove` (`arena.rs:256-263`) pushes the freed slot into
`free_list` and leaves a `None` in `nodes`. After a heavy parse +
`set_inner_html` storm, `nodes.capacity()` can be 10× the live count.
On the cold path the whole `Dom` drops — capacity recovered. On the
warm pool path `replace_dom` replaces the `DomState` with a fresh one
(`lib.rs:169-179`) so the old `Dom` *also* drops — but only if no
op-state back-reference still pins it.

**Spec**:
- Add a `Dom::shrink_to_fit()` method that calls
  `self.nodes.shrink_to_fit()` + `self.free_list.shrink_to_fit()`.
- Call it from `reset_warm_state` (`crates/browser/src/page.rs:1172-1202`)
  via a new op or as part of `replace_dom`'s body
  (`crates/js_runtime/src/lib.rs:167-179`) — the new `DomState` builds
  a fresh `Dom`, but we want to also shrink the *outgoing* one's
  capacity just before drop to release pages to the allocator promptly.

**File:line**: new method at `crates/dom/src/arena.rs:~85` (after the
`allocate` family); callsite addition in
`crates/js_runtime/src/lib.rs:178` (after `state.put(dom_state)`).

**Expected saving**: 5-20 MB on the warm pool path; minimal cold.

### 4.6 `__fetchLog` cap — 200 entries, body-drop past 100

**File**: `crates/browser/src/page.rs:3076-3110` (install) +
`page.rs:3211,3247` (push sites in fetch/XHR wrappers).

**Current behaviour**: every `fetch()` and every `XMLHttpRequest.send()`
pushes a record into `globalThis._browser_oxide.__fetchLog`. There is
no cap. A page that issues 5000 telemetry pings (typical Akamai BMP)
ratchets the array to 5000 entries.

**Spec**:
- Cap entry count at 200. Drop the oldest on push past the cap.
- Past entry 100, push request/response *headers only* — drop bodies.
- Implement in JS at the wrapper site (`page.rs:3211` and `page.rs:3247`):

  ```js
  const FETCH_LOG_MAX = 200;
  const FETCH_LOG_BODY_MAX = 100;
  const log = globalThis._browser_oxide.__fetchLog;
  if (log.length >= FETCH_LOG_MAX) log.shift();
  const entry = { /* req/resp summary */ };
  if (log.length >= FETCH_LOG_BODY_MAX) {
      delete entry.body;
      delete entry.requestBody;
  }
  log.push(entry);
  ```

**Expected saving**: 1-3 MB cold per analytics-heavy page; cumulative
gain compounds on the pool path.

### 4.7 `resource_timings` cap — 500 entries (WHATWG default)

**File**: `crates/js_runtime/src/state.rs:33`:

```rust
pub resource_timings: Vec<net::TimingStats>,
```

**Current behaviour**: pushed by the fetch path; read by
`op_perf_get_resource_timings`
(`crates/js_runtime/src/extensions/perf_ext.rs:111-122`). No cap. The
WHATWG Resource Timing Level 2 buffer is **150 entries** by default
in Chrome (`performance.setResourceTimingBufferSize`); 500 is a safe
2× headroom that matches what most sites set.

**Spec**:
- Add a `RESOURCE_TIMING_MAX: usize = 500` const at the top of
  `state.rs`.
- At every push site (search: `resource_timings.push`), guard with
  `if state.resource_timings.len() < RESOURCE_TIMING_MAX`.
- Optionally also expose
  `op_perf_set_resource_timing_buffer_size(size: u32)` so sites that
  ask for less (or call `performance.clearResourceTimings()`) get
  expected behaviour.

**Expected saving**: < 1 MB on typical pages; bounds the worst case
(a long-poll page that runs for hours and accumulates 100k entries).

### 4.8 `BlobRegistry` TTL / LRU

**File**: `crates/js_runtime/src/extensions/worker_ext.rs:32-43`:

```rust
struct BlobRegistry {
    blobs: HashMap<String, BlobEntry>,
}
```

**Current behaviour**: every `URL.createObjectURL(blob)` inserts a
`Vec<u8>` of the blob's bytes. `URL.revokeObjectURL(url)` removes it.
Many sites (analytics, image-processing, video) never call revoke. The
registry is process-global (`OnceLock<Mutex<…>>`) — it does NOT drop
with `Page`. A 5 MB video blob from one cold navigation persists for
the rest of the process.

**Spec**:
- Augment `BlobEntry` with `last_access: Instant` and
  `created: Instant`.
- On `op_blob_register`, evict entries older than `BLOB_TTL` (e.g.
  60 s of zero access).
- Bound total registry size at `BLOB_REGISTRY_MAX_BYTES = 50 MB` —
  on register, if the new blob would exceed this, evict the
  least-recently-accessed until under the cap.
- Touch `last_access` on `op_blob_fetch_bytes` /
  `op_blob_fetch_text`.

**File:line**: change `BlobRegistry`'s fields at
`worker_ext.rs:32-34`; insert eviction logic in `op_blob_register`
(`worker_ext.rs:48-62`) and touch in `op_blob_fetch_bytes`
(`worker_ext.rs:93-107`).

**Expected saving**: 5-50 MB on long-lived processes (production pool
running for hours); 0-5 MB on a 126-site cold sweep.

### 4.9 V8 isolate `low_memory_notification` on `Page::drop`

Already specified in `09 §5` as a companion to the `HEAP_INITIAL`
drop. Mentioned here so the budget audit has the file-pointer:

```rust
// proposal in crates/browser/src/page.rs Drop impl
let isolate = self.event_loop.runtime_mut().isolate_handle();
isolate.low_memory_notification();
```

**File:line**: `crates/browser/src/page.rs:216-233`, immediately after
`drain_owned_workers` and before child drop.

**Expected saving**: 30-80 MB on the cold path (V8 returns pages to
the OS more aggressively).

## 5. Pool path memory plan

The pool path measured 1365 MB RSS on 97 sites (`09 §6`) — **worse
than cold (419 MB on 126 sites)**. That is upside-down: the pool is
supposed to amortise V8 setup, not retain it.

### 5.1 Root cause recap

Per `09 §6` and the source at `crates/js_runtime/src/lib.rs:167-179`:

```rust
pub fn replace_dom(&mut self, dom: Dom, stylesheets: Vec<String>) {
    let state = self.inner.op_state();
    let mut state = state.borrow_mut();
    let mut dom_state = DomState::new(dom);
    dom_state.stylesheets = stylesheets;
    dom_state.update_cached_rules();
    state.put(dom_state);
    state.put(extensions::timer_ext::TimerState::new());
}
```

The new `DomState` lands in `OpState`, dropping the old one. But:
- The V8 isolate stays alive — its JIT code cache, hidden classes,
  and bootstrap closures persist (this is the *point* of the pool).
- The `globalThis.__fetchLog` array's V8-side backing buffer keeps its
  capacity even after `__fetchLog.length = 0`
  (`reset_warm_state` at `page.rs:1180`).
- Stylesheet text + parsed `CachedRule`s for the previous page also
  drop, but `Vec` capacity for the freshly-built one may be larger
  than the current page needs.

### 5.2 Spec — `reset_warm_state` should `shrink_to_fit` aggressively

**File**: `crates/browser/src/page.rs:1172-1202`.

Add four operations to the warm-state reset:

1. **DOM arena shrink** — new op + call:
   ```js
   if (globalThis.__shrinkDomArena) globalThis.__shrinkDomArena();
   ```
   Backed by a new op in `dom_ext.rs` that calls
   `state.dom.shrink_to_fit()` (per §4.5).

2. **Stylesheet vector shrink** — happens automatically if `state.rs`
   adds the `shrink_to_fit` in `update_cached_rules` (§4.4).

3. **`__fetchLog` *reassignment* (not `.length = 0`)** — drop the V8
   array's backing buffer:
   ```js
   if (g._browser_oxide) g._browser_oxide.__fetchLog = [];
   if (Array.isArray(w.__cookieWrites)) w.__cookieWrites = [];
   if (Array.isArray(w.__scriptErrors)) w.__scriptErrors = [];
   ```
   In place of the `.length = 0` lines at `page.rs:1180,1184,1185`.

4. **Low-memory notification** — at the end of `reset_warm_state` (so
   V8 can release the freed capacity):
   ```rust
   self.event_loop.runtime_mut().isolate_handle().low_memory_notification();
   ```

### 5.3 Pool::release should also nudge V8

**File**: `crates/browser/src/pool.rs:51-56`:

```rust
pub fn release(&self, page: Page) {
    let mut pages = self.idle_pages.lock().unwrap_or_else(|e| e.into_inner());
    if pages.len() < self.max_size {
        pages.push_back(page);
    }
}
```

**Spec**: before re-queueing, run a final scrub:

```rust
pub fn release(&self, mut page: Page) {
    // Release any V8 heap pages the previous navigation grew into.
    // Safe: low_memory_notification is a hint, not a forced GC, and
    // pages.is_empty()? early-return path is unchanged for hot reuse.
    page.event_loop.runtime_mut()
        .isolate_handle().low_memory_notification();
    let mut pages = self.idle_pages.lock().unwrap_or_else(|e| e.into_inner());
    if pages.len() < self.max_size {
        pages.push_back(page);
    }
}
```

Cost: `low_memory_notification` triggers a major GC. On a 419 MB
isolate this is typically 50-200 ms — acceptable on the *release*
path (caller is done with the page), unacceptable on `navigate`
(would add directly to the 2.6 s median).

### 5.4 Pool acceptance gate

Per `09 §6.4`:
- 3-run 126-site pool sweep peak RSS ≤ 800 MB (from 1365 / 97).
- No site flips from PASS to FAIL.
- Pool path completes all 126 sites without OOM (depends on the
  wellsfargo cycle panic being fixed per `10 §4`).

## 6. Long-lived process memory plan (production)

A customer running the pool path for hours through millions of URLs
faces a different memory profile from the 126-site sweep. Even after
§5 mitigations, the V8 isolate's JIT code cache + the BlobRegistry +
SharedSession's cookie jar will trend upward over real-world time.

### 6.1 Spec — `PagePool::compact()`

Add an explicit API customers can call on a long-running pool:

```rust
impl PagePool {
    /// Drop the warm Page furthest from the front of the queue, then
    /// build a fresh one to replace it. Reclaims V8 heap that GC alone
    /// can't release because of JIT code-cache and per-isolate retain.
    pub async fn compact(&self, profile: StealthProfile) -> Result<(), AnyError> {
        let to_drop = {
            let mut pages = self.idle_pages.lock().unwrap_or_else(|e| e.into_inner());
            pages.pop_back()
        };
        drop(to_drop); // Page::drop reaps workers, runs final GC notification
        // Replace with a fresh isolate so the pool still has max_size warms
        let fresh = Page::from_html(BLANK_HTML, Some(profile)).await?;
        let mut pages = self.idle_pages.lock().unwrap_or_else(|e| e.into_inner());
        if pages.len() < self.max_size {
            pages.push_back(fresh);
        }
        Ok(())
    }
}
```

**File**: `crates/browser/src/pool.rs` — add at end of `impl PagePool`.

### 6.2 Spec — automatic recycle after N navigations

Internally track per-Page navigation count. Drop+rebuild after
N (default 500) navigations:

```rust
pub struct Page {
    // ...
    pub(crate) nav_count: u32,
}

impl PagePool {
    pub async fn navigate(&self, url: &str, profile: StealthProfile) -> Result<Page, AnyError> {
        let mut page = self.acquire(Some(profile.clone())).await?;
        if page.nav_count >= 500 {
            drop(page); // implicit recycle
            page = self.acquire(Some(profile.clone())).await?;
        }
        page.navigate_warm(url).await?;
        page.nav_count += 1;
        Ok(page)
    }
}
```

**File:line**: `crates/browser/src/page.rs:200-214` (`Page` struct add
field) + `crates/browser/src/pool.rs:78-86` (`navigate` recycle check).

### 6.3 Spec — RSS watchdog

Optional opt-in: spawn a background tokio task that reads
`self_rss_mb()` (existing helper at
`crates/browser/examples/sweep_metrics.rs:73-83` — promote it to a
library helper) every N seconds. If RSS exceeds a configured
threshold, force-drop the oldest pool isolate:

```rust
impl PagePool {
    pub fn install_rss_watchdog(self: Arc<Self>, threshold_mb: f64, profile: StealthProfile) {
        let pool = Arc::clone(&self);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;
                if browser::self_rss_mb() > threshold_mb {
                    let _ = pool.compact(profile.clone()).await;
                }
            }
        });
    }
}
```

**File:line**: new method on `crates/browser/src/pool.rs`; promote
`self_rss_mb` from
`crates/browser/examples/sweep_metrics.rs:73-83` to a public helper
in `crates/browser/src/lib.rs`.

## 7. Per-profile memory comparison — why iPhone is +57 MB vs Pixel

Source: `summary.rss_peak_mb` in
`~/projects/browser_oxide_internal/benchmarks/baselines/2026-05-24/bo_*_cold.json`.

| Profile | RSS peak (MB) | File:line | Cause |
|---|--:|---|---|
| `pixel_9_pro_chrome_148` | **388** | `crates/stealth/src/presets.rs:690` | Leanest — empty plugin / mime arrays (`presets.rs:741-743`); Android UA |
| `chrome_148_macos` | 419 | `crates/stealth/src/presets.rs:120` | Full desktop Chrome — 5-plugin set + standard mime map |
| `iphone_15_pro_safari_18` | 445 | `crates/stealth/src/presets.rs:795` | Apple "16 declined APIs" deletions retained as V8 absent-map transitions |
| `firefox_135_macos` | **472** | `crates/stealth/src/presets.rs:413` | Gecko-flavour bootstrap diffs (different `navigator.*` getters, different prototype-chain shape) |

### Why iPhone is +57 MB vs Pixel

Read `crates/stealth/src/presets.rs:780-815` for the catalogue. The
iPhone profile adds:

- `DeviceMotionEvent.requestPermission` static (iOS 13+)
- `window.orientation = 0` (legacy iOS-only)
- `'ontouchstart' in window: true`
- Apple "16 declined APIs" deletions: every `delete` on a prototype
  chain creates a V8 "absent" map transition — V8 retains both the
  pre-delete and post-delete maps as siblings for IC feedback. 16
  deletions × multiple prototypes = several hundred map transitions
  permanently in the heap.
- The literal `"Apple GPU"` WebGL renderer constant (negligible).
- AudioContext sampleRate fixup (negligible).

The map-transition cost is the dominant one. V8 represents prototype
absences as separate transitions from "this slot is undefined", so
the per-prototype delete burden is real heap. Pixel skips this whole
class of mutation.

The **84 MB pixel→firefox spread is not a bug** — it's the price of
profile fidelity. None of these are addressable for v0.1.0 without
ceasing to be a faithful Apple/Mozilla impersonator (which would
break the engine's stealth thesis).

## 8. Acceptance for v0.1.0

- [ ] **Per-subsystem budget table populated with measured values** —
      re-run the cold sweep with `BROWSER_OXIDE_EVENT_LOOP_PROFILE=1`
      and instrument each row's struct with a sizing query (the §2
      values today are mostly *(est.)* — populate them properly).
- [ ] **Pool path RSS ≤ 800 MB on 126 sites** — depends on §5
      (`reset_warm_state` shrink + `pool.release`
      `low_memory_notification`) and the wellsfargo panic fix
      (`10 §4`).
- [ ] **Cold path RSS ≤ 350 MB on all 4 profiles** — depends on
      §4.1 (`HEAP_INITIAL` drop) + worker reap (already in tree).
- [ ] **`shrink_to_fit` discipline applied** — `Dom`, `stylesheets`,
      `cached_rules`, `dom_to_taffy` all have a callsite or
      lifecycle that drops capacity within bounded time.
- [ ] **Long-lived process plan documented + implementable** —
      `PagePool::compact()`, automatic recycle after N navs, and the
      RSS watchdog are spec'd here; at least one (compact or
      recycle-after-N) lands in v0.1.0 with a customer-facing example
      in `crates/browser/examples/`.
- [ ] **`__fetchLog` capped** — entry cap + body-drop past N
      implemented per §4.6, with a regression test in
      `crates/browser/tests/` that asserts the cap holds across 1000
      simulated fetches.
- [ ] **`resource_timings` capped at 500** — per §4.7. Doc the cap
      in the perf API doc so customers know to call
      `performance.setResourceTimingBufferSize` if they need more.
- [ ] **`BlobRegistry` TTL + size cap** — per §4.8 with a unit test
      in `crates/js_runtime/tests/` asserting eviction occurs.
- [ ] **Per-profile RSS regression test** — add to
      `14_TESTING_VALIDATION.md` the 4-profile RSS aggregation so
      future PRs that bloat one profile flag immediately.

## 9. What is explicitly NOT in scope

- Switching off `deno_core` to a smaller V8 binding — deferred to
  `15_OPEN_QUESTIONS.md`.
- Per-profile V8 snapshots (one snapshot per stealth profile) — small
  steady-state benefit, large build-system complexity, deferred.
- Aggressive `low_memory_notification` on the navigate-warm hot path
  — would directly add to the 2.6 s median (per §5.3). Acceptable on
  release, not on navigate.
- Custom `malloc` (jemalloc / mimalloc) for the rust side — out of
  scope; deno_core's V8 has its own allocator and the Rust side is a
  rounding error.
- Disabling `__fetchLog` / `__cookieWrites` instrumentation entirely
  — still load-bearing for the classifier and debug tooling.

## 10. Files referenced

All paths absolute under `/home/yfedoseev/projects/browser_oxide`.

**Engine source** (target sites for §3 / §4 / §5 / §6 specs):

- `crates/browser/src/page.rs:200-214` — `Page` struct
- `crates/browser/src/page.rs:216-233` — `Drop for Page` (fix C
  worker reap + future `low_memory_notification` site)
- `crates/browser/src/page.rs:380-416` — `Page::reload_html`
- `crates/browser/src/page.rs:1145-1202` — `Page::reset_warm_state`
  (§5.2 target; `.length = 0` lines at 1180 / 1184 / 1185;
  `__akamai_events` clear at 1186-1198)
- `crates/browser/src/page.rs:3076-3110` — `__cookieWrites` install
  (push site with hard cap of 100 at 3097-3099)
- `crates/browser/src/page.rs:3112-3120` — `__scriptErrors` install
- `crates/browser/src/page.rs:3124-3211` — fetch wrapper (`__fetchLog`
  push, §4.6 cap target)
- `crates/browser/src/page.rs:3233-3247` — XHR wrapper (`__fetchLog`
  push)
- `crates/browser/src/pool.rs:1-87` — `PagePool` API
  (`release` at 51-56 = §5.3 insertion point; `navigate` at 78-86;
  `reload_html` on acquire at 42)
- `crates/browser/examples/sweep_metrics.rs:73-83` — `self_rss_mb`
  (candidate to promote for §6.3 watchdog)
- `crates/browser/examples/sweep_metrics.rs:138-197` — sweep loop
- `crates/dom/src/arena.rs:5-19` — `Dom` struct + `WALK_LIMIT` +
  `ANCESTOR_LIMIT`
- `crates/dom/src/arena.rs:57-67` — `allocate` (free_list slot
  recycle)
- `crates/dom/src/arena.rs:256-263` — `Dom::remove`
- `crates/dom/src/arena.rs:655-699` — `collect_elements` (panic site
  blocking §5 pool sweep at 1365 MB)
- `crates/js_runtime/src/lib.rs:50-57` —
  `BrowserJsRuntime::with_options` (snapshot auto-load)
- `crates/js_runtime/src/lib.rs:167-179` — `replace_dom` (§4.5 / §5.2
  change site)
- `crates/js_runtime/src/runtime.rs:36-50` — `BrowserRuntimeOptions`
- `crates/js_runtime/src/runtime.rs:98-111` — **`HEAP_INITIAL = 1 GB`**
  (§4.1 target)
- `crates/js_runtime/src/runtime.rs:113-139` — `JsRuntime::new`
- `crates/js_runtime/src/runtime.rs:156-162` —
  `WorkerOwnership::default()` (fix C)
- `crates/js_runtime/src/runtime.rs:191-227` — bootstrap-skip branch
  (§4.2 target)
- `crates/js_runtime/src/snapshot.rs:1-103` — `get_snapshot` (chapter
  21's implementation site)
- `crates/js_runtime/src/state.rs:7-34` — `DomState`
  (`stylesheets` + `cached_rules` at 16-18; **`resource_timings`
  at 33** = §4.7 cap target; `update_cached_rules` at 78-106 =
  §4.4 `shrink_to_fit` insertion)
- `crates/js_runtime/src/extensions/perf_ext.rs:111-122` —
  `op_perf_get_resource_timings`
- `crates/js_runtime/src/extensions/worker_ext.rs:24-43` —
  **`BlobRegistry`** (§4.8 target)
- `crates/js_runtime/src/extensions/worker_ext.rs:48-62` —
  `op_blob_register`
- `crates/js_runtime/src/extensions/worker_ext.rs:93-107` —
  `op_blob_fetch_bytes`
- `crates/js_runtime/src/extensions/worker_ext.rs:213-249` —
  `op_worker_spawn` (64 MB stack at 251-258)
- `crates/js_runtime/src/extensions/worker_ext.rs:385-405` —
  `terminate_worker_inner`
- `crates/js_runtime/src/extensions/worker_ext.rs:407-423` —
  **`drain_owned_workers`** (fix C reaper)
- `crates/js_runtime/src/extensions/worker_ext.rs:425-434` —
  `WorkerOwnership` definition
- `crates/layout/src/engine.rs:21-33` — `LayoutEngine` struct
- `crates/stealth/src/presets.rs:120 / 413 / 690 / 795` — four
  stealth profiles (pixel empty plugins at 741-743; iPhone "16
  declined APIs" at 780-815)

**Benchmark / data**:

- `benchmarks/bench_corpus_v2.py:50-54` — `get_rss_mb`
- `benchmarks/bench_corpus_v2.py:101-106` — `tree_rss_mb`
- `benchmarks/bench_corpus_v2.py:265-268` — `run_camoufox` PID fix
- `~/projects/browser_oxide_internal/benchmarks/baselines/2026-05-24/bo_chrome_148_macos_cold.json` (peak 418.9),
  `bo_firefox_135_macos_cold.json` (471.9),
  `bo_iphone_15_pro_safari_18_cold.json` (444.7),
  `bo_pixel_9_pro_chrome_148_cold.json` (388.0),
  `bo_chrome_148_macos_pool.json` (1365, 97 partial)

**Sibling chapters**:

- `09_MEMORY_OPTIMIZATION.md` — high-level memory triage (this 20 is
  the per-subsystem deep-dive)
- `10_TIMING_OPTIMIZATION.md` — companion; pool memory + wellsfargo
  panic block this chapter's §5 acceptance
- `14_TESTING_VALIDATION.md` — multi-run regime for §8 gates
- `21_V8_SNAPSHOT_PARALLEL_COLD.md` — owns §4.2 (snapshot warming)
  referenced from rows #1 + #2 of §2
