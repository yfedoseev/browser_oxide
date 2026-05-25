# 09 — Memory optimization

**Status:** planning
**Scope:** RSS / heap / per-page leak surface for v0.1.0
**Companion:** `10_TIMING_OPTIMIZATION.md` (some root causes overlap — the
pool path that drives timing wins is what surfaced the DOM-arena retain
that drives the pool RSS regression).

## 1. Headline truth

The 10× memory perception ("BO 419 MB vs Camoufox 48 MB") is **largely
a measurement bug** in the competitor harness — not an engine
regression. Honest comparison:

| Engine | Reported RSS | Real RSS (full process tree) | Notes |
|---|--:|--:|---|
| Camoufox (reported) | 48 MB | — | wrong — see §3 |
| Camoufox (corrected) | — | **~200-400 MB** | est. — requires re-run with the fixed harness |
| BO chrome_148_macos cold | 419 MB | 419 MB | single process, RSS via `/proc/self/statm` |
| BO firefox_135_macos cold | 472 MB | 472 MB | |
| BO iphone_15_pro_safari_18 cold | 445 MB | 445 MB | |
| BO pixel_9_pro_chrome_148 cold | 388 MB | 388 MB | |
| BO chrome POOL (97 sites partial) | **1365 MB** | 1365 MB | DOM arena retains across pages — real issue |
| Playwright (chromium tree) | 5618 MB | 5618 MB | |
| Patchright (chromium tree) | 5681 MB | 5681 MB | |
| Playwright + Stealth | 5011 MB | 5011 MB | |

Source: `/tmp/full_sweep_2026_05_24/*.json` `summary.rss_peak_mb`.

Two engine bugs are *real* and addressed in this chapter:

1. **Worker leak** (cold path) — orphan `new Worker(...)` instances keep
   their OS thread + child `JsRuntime` alive forever. Drives the
   monotonic +2.4 MB/site climb in the cold-sweep RSS curve. **Fix
   applied 2026-05-24** (uncommitted) — see §4.
2. **DOM-arena retain in warm reuse** — the warm pool keeps the
   largest-ever page's arena Vec capacity for the rest of the pool's
   life. That's why the pool path (1365 MB on 97 sites) is *worse* than
   cold (419 MB on 126 sites). **Spec only** — see §6.

After both, BO target is ≤ 350 MB cold / ≤ 800 MB pool — within 1.5×
of an honestly measured Camoufox.

## 2. Per-profile RSS analysis (cold sweep)

All numbers from `/tmp/full_sweep_2026_05_24/bo_*_cold.json` `summary.rss_peak_mb`.

| Profile | RSS peak | Bootstrap chars | Notes |
|---|--:|--:|---|
| `pixel_9_pro_chrome_148` | **388 MB** | smallest mobile-Android (empty plugin/mime arrays) | |
| `chrome_148_macos` | 419 MB | desktop Chrome — 5-plugin / mime set | |
| `iphone_15_pro_safari_18` | 445 MB | iOS deltas: hardwareConcurrency cap, no UA-CH, no Bluetooth/USB/Serial/HID/Sensor/Battery/MIDI/IdleDetector/WebGPU | |
| `firefox_135_macos` | **472 MB** | largest — Gecko-flavoured bootstrap diffs | |

The 84 MB pixel→firefox spread maps to per-profile bootstrap script
weight (more shimmed APIs = more closures held on the V8 heap).
Profile definitions live in `crates/stealth/src/presets.rs`:

- `chrome_148_macos` — `crates/stealth/src/presets.rs:120`
- `firefox_135_macos` — `crates/stealth/src/presets.rs:413`
- `pixel_9_pro_chrome_148` — `crates/stealth/src/presets.rs:690`
- `iphone_15_pro_safari_18` — `crates/stealth/src/presets.rs:795`

iPhone is higher than pixel despite both being mobile because iPhone
profile installs `DeviceMotionEvent.requestPermission`, `window.orientation`,
the "16 declined APIs" deletions (each is a `delete` on a prototype
chain that V8 retains as an "absent" map transition), and bigger
`plugins`/`mimeTypes` shadows — see the doc-comment at
`crates/stealth/src/presets.rs:780-794`.

The **84 MB profile spread is not a bug** — it's the cost of fidelity.
It's a candidate for §5 (V8 heap-initial tradeoff), not a fix here.

### RSS-growth curve (cold, chrome_148_macos, 126 sites)

Source: per-result `rss_mb` field in `bo_chrome_148_macos_cold.json`.

| Slice | Mean RSS (MB) |
|---|--:|
| First 20 sites | ≈ 117 |
| Q1 (sites 1-31) | 136 |
| Q2 (sites 32-63) | 213 |
| Q3 (sites 64-94) | 307 |
| Q4 (sites 95-126) | 354 |
| Linear regression | **+2.40 MB / site** |
| Baseline (site 1) → Peak (site ~125) | 63 → 419 |

First 20 RSS readings (MB):
```
62.8, 66.3, 69.4, 69.7, 80.1, 92.8, 95.6, 100.0, 100.7, 92.6,
97.9, 108.6, 108.7, 128.9, 162.5, 166.6, 169.2, 148.2, 159.4, 159.8
```

Last 20:
```
320.7 (×4), 321.8, 344.0 (×4), 343.8, 343.9 (×3),
364.2, 365.9, 366.6, 398.8 (×3), 416.6, 418.1, 418.9
```

The monotonic, near-linear shape is the smoking gun: every page leaks
roughly the same fixed amount, RSS never reclaims between pages even
though every `Page` value is dropped at the end of its iteration in
`crates/browser/examples/sweep_metrics.rs:140-167`. That's the
worker-leak fingerprint — see §4.

## 3. Camoufox measurement bug — post-mortem

**Symptom**: `comp_camoufox.json` reports `rss_peak_mb: 48.3` for a
126-site sweep through a Firefox e10s tree. Firefox alone idles at
~120 MB; under 126 navigations through anti-bot challenges the real
tree is 200-400 MB. The 48 MB number is the **Camoufox launcher PID**
in isolation, not the e10s content / Privileged Content / RDD / GPU /
Socket / Utility tree it spawned.

### The original code

`benchmarks/bench_corpus_v2.py:256-267` (pre-fix; reconstructed from
the now-updated source):

```python
# (pre-fix, removed 2026-05-24)
for proc in psutil.process_iter(['pid', 'name', 'cmdline']):
    if proc.info['name'] and (
        'fox' in proc.info['name'].lower()
        or 'amoufox' in proc.info['name'].lower()
    ):
        root_pid = proc.info['pid']
        break
```

Then `tree_rss_mb(root_pid)` (`benchmarks/bench_corpus_v2.py:101`)
walked descendants of *that* PID. But the picked PID was the
`camoufox` python launcher, not `firefox-bin`. The e10s `Web Content`
processes are children of `firefox-bin`, which is itself spawned by
the Camoufox driver in a sibling process tree. They are entirely missed.

### Why this happens — Firefox e10s topology

```
camoufox-python-driver         <-- the matched PID
└── (no child Web Content here)

firefox-bin                    <-- sibling; runs the actual content
├── Web Content (tab 1)
├── Web Content (tab 2)
├── ...
├── Privileged Content
├── RDD
├── GPU
├── Socket
└── Utility
```

The launcher / orchestrator is in one branch; the renderer tree is in
another. Camoufox's own `BrowserType.launch()` returns a `Browser`
whose `.process.pid` *is* the `firefox-bin` PID — exactly what we
need. The driver-PID heuristic was lazy and wrong.

### The fix (already applied 2026-05-24, uncommitted)

`benchmarks/bench_corpus_v2.py:265-268`:

```python
try:
    root_pid = browser.process.pid if hasattr(browser, 'process') and browser.process else os.getpid()
except Exception:
    root_pid = os.getpid()
```

Matches the `run_playwright` (line 199-202) and `run_patchright` (line
223-226) branches. The doc-comment above the fix (lines 252-264)
explains the failure mode for future readers.

### How to validate

```bash
cd /home/yfedoseev/projects/browser_oxide
# Re-run Camoufox only (no need to touch BO runs)
python3 benchmarks/bench_corpus_v2.py --engine camoufox --corpus tests/corpus_v2.json --out /tmp/comp_camoufox_FIXED.json
# Expect:
#  rss_peak_mb jumps from ~48 to ~200-400
#  pass count unchanged (113)
#  median_ms unchanged (~5600)
python3 -c "import json; d=json.load(open('/tmp/comp_camoufox_FIXED.json')); print(d['summary']['rss_peak_mb'])"
```

If the corrected number is < 150 MB on a 126-site sweep, something
else is wrong (Firefox e10s alone is heavier than that on Linux with
the GPU/Socket/Utility processes running) — investigate before
publishing.

## 4. Worker leak fix — applied, document it

**Cluster**: every `new Worker(url|blob)` from page JS spawned an OS
thread with a 64 MB stack guard + a child `JsRuntime` (V8 isolate).
The OS thread polled `terminate.load(...)` to know when to exit. Page
JS that never explicitly called `worker.terminate()` left the thread
spinning until process exit. **13 sites in the 126-corpus** are the
dominant offenders (the +15 MB step-up cluster in the RSS curve):

```
cnn, bloomberg, youtube, discord, udemy, asos, x.com,
reddit, douyin, amazon-*, etsy, hulu, ...
```

(exact set is workload-dependent — recaptcha enterprise loads a
`webworker.js`; Akamai BMP v3 loads worker from a blob: URL; many
analytics SDKs spawn one too).

### Before — `Page::drop`

`crates/browser/src/page.rs:216-233` was:

```rust
impl Drop for Page {
    fn drop(&mut self) {
        while self.children.pop().is_some() {}
    }
}
```

Workers were not in `self.children` (those are iframe child isolates).
They lived in a static registry in `worker_ext.rs` keyed by id, with
no back-reference to which Page spawned them. Drop never reaped them.

### After — three coordinated changes

#### 4.1 `crates/js_runtime/src/extensions/worker_ext.rs` — `WorkerOwnership` state

New type at lines 425-434:

```rust
/// Per-`JsRuntime` set of worker IDs spawned by this isolate. Populated
/// by `op_worker_spawn`; drained by `drain_owned_workers` at `Page::drop`.
/// `RefCell` because deno_core's `#[op2]` macro requires all `#[state]`
/// parameters on a single op be either all `&` or all `&mut` — we need
/// `&DomState` + `&StealthState` (immutable) so the only way to mutate
/// this third state from inside the op is via interior mutability.
#[derive(Default)]
pub struct WorkerOwnership {
    pub spawned_ids: RefCell<Vec<u32>>,
}
```

`op_worker_spawn` (line 213-249) takes it as a third state:

```rust
#[op2(fast)]
#[smi]
pub fn op_worker_spawn(
    #[state] state: &DomState,
    #[state] stealth: &StealthState,
    #[state] owned: &WorkerOwnership,   // ← new
    #[string] script: String,
    #[string] _name: String,
    is_module: bool,
) -> i32 {
    ...
    owned.spawned_ids.borrow_mut().push(worker_id);   // ← new (line 249)
    ...
}
```

#### 4.2 `drain_owned_workers` reaper

`crates/js_runtime/src/extensions/worker_ext.rs:407-423`:

```rust
/// Reaper for `Page::drop` — terminates every worker spawned by a
/// page's V8 isolate. ... cnn / bloomberg / youtube / discord / udemy
/// — the 13 sites driving the +15 MB step-ups in the cold-sweep RSS
/// curve.
pub fn drain_owned_workers(state: &mut deno_core::OpState) {
    let ids: Vec<u32> = state
        .try_borrow::<WorkerOwnership>()
        .map(|o| std::mem::take(&mut *o.spawned_ids.borrow_mut()))
        .unwrap_or_default();
    for id in ids {
        terminate_worker_inner(id);
    }
}
```

Uses the existing `terminate_worker_inner` (line 396-405) which sets
the worker's terminate flag, removes the registry slot, and wakes any
pending `op_worker_await_message` so the worker thread exits its
tokio `block_on` cleanly. Idempotent for explicitly-terminated workers
(the id is already gone from `spawned_ids`).

#### 4.3 `runtime.rs` — register `WorkerOwnership` in `OpState`

`crates/js_runtime/src/runtime.rs:156-162`:

```rust
// Per-Page worker-ownership tracker — every `new Worker(...)` push
// its id here so `Page::drop` can reap orphans (see
// `extensions::worker_ext::drain_owned_workers`).
runtime
    .op_state()
    .borrow_mut()
    .put(crate::extensions::worker_ext::WorkerOwnership::default());
```

#### 4.4 `Page::drop` calls the reaper

`crates/browser/src/page.rs:216-233`:

```rust
impl Drop for Page {
    fn drop(&mut self) {
        // Reap any Workers this page's V8 isolate spawned but never
        // explicitly `worker.terminate()`'d from JS. ...
        {
            let op_state = self.event_loop.runtime_mut().op_state();
            let mut state = op_state.borrow_mut();
            js_runtime::extensions::worker_ext::drain_owned_workers(&mut state);
        }
        // Drop children (newer isolates) before parent (older isolate)
        // V8 requires reverse drop order
        while self.children.pop().is_some() {}
    }
}
```

### Why the `op2` macro forced `RefCell`

Mixing `&` + `&mut` `#[state]` parameters on the same op is unsupported
by `#[op2]` in `deno_core 0.311`. `op_worker_spawn` already takes
`&DomState` (line 215) and `&StealthState` (line 216) — both shared.
The only way to mutate `WorkerOwnership` from inside the same op is
through interior mutability, hence `RefCell<Vec<u32>>`. This isn't
a thread-safety relaxation: the V8 isolate is single-threaded per the
project convention in `CLAUDE.md`; the `RefCell` only mediates between
the op handler borrow and the `Page::drop` borrow which can't happen
during op execution.

### Validation status

- **Build**: `cargo build --release --workspace` — clean.
- **Spot-check**: 33-site abbreviated sweep ran with no functional
  regression (no pass-count delta, no new errors).
- **Full validation pending**: 126-site sweep with the patch to
  measure RSS delta directly. Hypothesis: peak RSS drops from 419 MB
  → ≤ 320 MB on chrome_148_macos cold; slope reduces from
  +2.40 MB/site to < +1.0 MB/site.

### How to validate

```bash
cargo build --release -p browser --example sweep_metrics
target/release/examples/sweep_metrics chrome_148_macos \
    /tmp/holistic_corpus_v2.json /tmp/bo_chrome_cold_REAP.json \
    2>&1 | tee /tmp/bo_chrome_cold_REAP.log

python3 - <<'PY'
import json
d = json.load(open("/tmp/bo_chrome_cold_REAP.json"))
res = d["results"]
rss = [r["rss_mb"] for r in res]
n = len(rss)
print(f"baseline RSS: {rss[0]:.1f} MB")
print(f"peak    RSS: {max(rss):.1f} MB")
print(f"q1 mean: {sum(rss[:n//4])/(n//4):.1f}, q4 mean: {sum(rss[3*n//4:])/(n-3*n//4):.1f}")
# linear slope
mx = sum(range(n))/n; my = sum(rss)/n
slope = sum((i-mx)*(y-my) for i,y in enumerate(rss)) / sum((i-mx)**2 for i in range(n))
print(f"slope: {slope:+.3f} MB/site (baseline was +2.40)")
print(f"pass count: {d['summary']['pass']} (expect within ±2 of baseline)")
PY
```

Acceptance: peak ≤ 320 MB **and** pass count within ±2 of baseline 99.

## 5. V8 `HEAP_INITIAL = 1 GB` tradeoff

`crates/js_runtime/src/runtime.rs:98-111`:

```rust
// HEAP_INITIAL was 256 MB but caused early-growth GC pauses on
// fingerprint-heavy sites (creepjs allocates well past 256 MB during
// its lie-detection pass; V8 spent time compacting old space before
// growing the heap). 1 GB initial skips those early compactions.
const HEAP_INITIAL: usize = 1024 * 1024 * 1024; // 1 GB initial
const HEAP_MAX: usize = 4 * 1024 * 1024 * 1024; // 4 GB max
let create_params = deno_core::v8::CreateParams::default().heap_limits(HEAP_INITIAL, HEAP_MAX);
```

**Tradeoff**: V8 doesn't *commit* the full 1 GB up front (it's
reserved virtual + a smaller resident set). But across a 126-site
sweep, allocator pressure pushes resident growth toward the reserve.
On Linux RSS only counts faulted-in pages, so this isn't a hard 1 GB
penalty — but it does mean V8 *prefers* growing within the reservation
rather than asking the OS for more, and reclaims back to the OS are
rare in the absence of a `low_memory_notification` hint.

**Why we can't just drop it**:
- `creepjs` is in the 126-corpus (`crates/browser/tests/holistic_sweep.rs`)
  and currently passes.
- Pre-bump (256 MB initial) creepjs spent time in
  `Builtins_ArrayPrototypePush` compactions during its
  fingerprint-collection pass and OOM'd around 1.8 GB on macOS arm64
  (see the source comment).
- Reverting to 256 MB without a compensating mechanism risks a creepjs
  regression — and that's one of our keystone "the engine is
  fingerprint-honest" pass sites.

### Proposal — 256 MB initial + `low_memory_notification` after `Page::content()`

Mechanism:

1. Drop `HEAP_INITIAL` to 256 MB. Keep `HEAP_MAX = 4 GB` (so creepjs
   can still grow into it without OOM).
2. After `Page::content()` (or in `Page::drop`), call
   `runtime_mut().isolate_handle().low_memory_notification()`. V8
   treats this as a hard hint to compact + return unused heap pages
   to the OS.

Pseudo-patch in `crates/browser/src/page.rs` (in `Drop` or a new
`Page::shrink_after_extract` helper):

```rust
// hypothetical — not yet implemented
let isolate = self.event_loop.runtime_mut().isolate_handle();
isolate.low_memory_notification();   // V8: compact + release pages
```

### Risks

- **creepjs regression** — if compaction time grows or fingerprint
  output changes after the hint. Mitigation: A/B test before commit.
- **General pass-rate regression** — same risk for any heap-heavy
  challenge VM (Kasada ips.js, Akamai sensor_data, DataDome i.js).
  All are in the 126-corpus and will surface immediately.

### Acceptance gate (if pursued)

- 3-run 126-site sweep before/after.
- ΔPass ≥ -2 sites (within ±5 noise floor — see
  `docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md`).
- Δcreepjs body-len within ±5% (single-site regression check).
- ΔRSS peak ≥ -50 MB cold (otherwise the change is not worth the risk).

**If A/B test fails any gate, do not commit.** Document in
`15_OPEN_QUESTIONS.md` and move on — fix C (worker reap) is the
larger memory win anyway.

## 6. Pool path DOM-arena retain (spec)

**Symptom**: `comp_chrome_148_macos_pool.json` reports
`rss_peak_mb: 1365` on a 97-site partial sweep (panic on site 98,
wellsfargo — see `10_TIMING_OPTIMIZATION.md §4`). That's 3.3× the
cold sweep's peak on 30 more sites. Pool is *supposed* to be lighter,
not heavier.

### Root cause

`crates/js_runtime/src/lib.rs:169-179`:

```rust
pub fn replace_dom(&mut self, dom: Dom, stylesheets: Vec<String>) {
    let state = self.inner.op_state();
    let mut state = state.borrow_mut();
    // Replace DomState — ops will pick up the new DOM on next call
    let mut dom_state = DomState::new(dom);
    dom_state.stylesheets = stylesheets;
    dom_state.update_cached_rules();
    state.put(dom_state);
    // Reset timer state (clear pending timers from old page)
    state.put(extensions::timer_ext::TimerState::new());
}
```

This is called from `Page::reload_html` (page.rs:387),
`Page::navigate_warm` (page.rs:1425). It builds a fresh `DomState`
and `put`s it into `OpState`, which drops the previous `DomState`.

That drop *should* free the old arena. But:

1. The `Dom`'s backing storage is `nodes: Vec<Option<Node>>` +
   `free_list: Vec<usize>` (`crates/dom/src/arena.rs:5-8`). The
   old `DomState` is dropped, the Vec deallocates — fine.
2. **However**, the layout engine (`dom_state.layout_engine`) and the
   stylesheets clone are *new* allocations every page. Compounded with
   V8 heap reservation behaviour (§5) and the worker-leak issue (§4
   — still present on the warm path because `Page::drop` reaps only at
   the very end of the pool lifetime, not between navigations), the
   "freed" capacity stays in the process via V8 fragmentation and
   small leaks.
3. Worse: every page's bootstrap-instrumented event listeners
   (`__cookieWrites`, `__fetchLog`, etc.) sit on `globalThis` and are
   only cleared (`length = 0`) by `reset_warm_state` (page.rs:1172-1202).
   `length = 0` reclaims the Vec slots but the V8 array's backing
   buffer keeps its allocated capacity. A page that pushed 100k
   `__fetchLog` entries leaves a 100k-capacity Array forever on the
   warm isolate.

### Spec — fix surface

1. **Drop layout cache aggressively in `replace_dom`**:
   - Free `dom_state.layout_engine`'s caches before replacing.
   - Apply `Vec::shrink_to_fit()` to the new `DomState` `stylesheets`
     after install.
2. **Reset `__cookieWrites`, `__fetchLog` *containers*, not contents**:
   - In `reset_warm_state` (page.rs:1172-1202), reassign:
     ```js
     g._browser_oxide.__fetchLog = [];   // new Array — old backing GC'd
     ```
     instead of `g._browser_oxide.__fetchLog.length = 0;`.
   - Same for `window.__cookieWrites`, `window.__scriptErrors`.
3. **Call `runtime_mut().isolate_handle().low_memory_notification()`
   at the end of every `Page::navigate_warm`** (after the page has
   been returned to the caller — i.e. in `PagePool::release`):
   `crates/browser/src/pool.rs:51-56`.
4. **Optional**: every N pages, recycle the warm isolate (drop the
   Page, build a fresh one). This is the V8 equivalent of "reboot the
   renderer every 100 tabs". 50-100 is a reasonable N for our cost
   curve (~150 ms isolate spin-up vs unbounded retain).

### Acceptance gate

- 3-run 126-site pool sweep peak RSS ≤ 800 MB (from 1365 / 97).
- No site flips from PASS to FAIL.
- Pool path completes all 126 sites without OOM (assuming
  `10_TIMING_OPTIMIZATION.md §4` also fixes the wellsfargo cycle
  panic).

## 7. Target numbers for v0.1.0

| Metric | Current | Target | Stretch |
|---|--:|--:|--:|
| Cold path peak RSS (any single profile) | 388-472 MB | **≤ 350 MB** | ≤ 280 MB |
| Pool path peak RSS (126 sites) | 1365 MB (97 partial) | **≤ 800 MB** | ≤ 500 MB |
| Cold path RSS slope | +2.40 MB/site | **≤ +0.5 MB/site** | ≤ +0.1 MB/site |
| Honest comparison to Camoufox tree | 419 / 48 = 8.7× (bogus) | **≤ 1.5× corrected** | ≤ 1.2× corrected |
| Comparison to Playwright tree (5618 MB) | 13× lighter | keep | keep |

The cold "≤ 350 MB" target is achievable with §4 (worker reap) alone
based on the +15 MB step-up arithmetic — 13 leak sites × 15 MB = 195 MB
of avoidable retain. The "≤ 800 MB" pool target requires §6 work in
addition.

## 8. Acceptance checklist (gates v0.1.0)

- [ ] **Camoufox measurement fix committed** — `benchmarks/bench_corpus_v2.py:265-268`
      (the fix is already in the working tree; needs to land as a
      commit per `00_README.md §"Memory-mode notes"`).
- [ ] **Camoufox honest tree RSS re-measured** — single sweep on the
      same hardware to publish the real "≥ 200 MB" number.
- [ ] **Worker reap committed** — three-file change in `worker_ext.rs`
      + `runtime.rs` + `page.rs` (also already in the working tree).
- [ ] **Worker reap validated on full 126-site cold sweep** — RSS
      delta published; ≥ 80 MB peak drop expected per §4 hypothesis.
- [ ] **Worker reap regression-tested** — `cargo test --workspace --
      --test-threads=1` clean (no functional regression on
      anti_bot / chrome_compat / navigation_primitives).
- [ ] **HEAP_INITIAL tradeoff investigated** — either committed with
      A/B evidence per §5, or explicitly deferred in `15_OPEN_QUESTIONS.md`
      with reason.
- [ ] **Pool DOM-arena retain investigated** — at minimum the cheap
      changes from §6 (1, 2) committed; full §6.3-6.4 may defer.
- [ ] **3-run RSS aggregation** in `14_TESTING_VALIDATION.md` so a
      future bench tells us "regression" vs "noise floor".

## 9. What is explicitly NOT in scope

- Switching off `deno_core` to a smaller V8 binding. Out of scope for
  v0.1.0; documented in `15_OPEN_QUESTIONS.md`.
- Per-page V8 snapshot specialization (different snapshots per
  profile). Spec'd in `10_TIMING_OPTIMIZATION.md §6`; memory side-effect
  is small (snapshot reload doesn't change steady-state RSS).
- Disabling the `__cookieWrites` / `__fetchLog` instrumentation (still
  load-bearing for debug / classifier).

## 10. Files referenced

Engine source (all paths absolute under
`/home/yfedoseev/projects/browser_oxide`):

- `crates/browser/src/page.rs:200-214` — `Page` struct definition (children + event_loop + url + solvers)
- `crates/browser/src/page.rs:216-233` — `Drop for Page` with worker reap (fix C applied)
- `crates/browser/src/page.rs:380-416` — `Page::reload_html` (calls `replace_dom`)
- `crates/browser/src/page.rs:1145-1202` — `Page::reset_warm_state` doc + impl
- `crates/browser/src/page.rs:1223-1530` — `Page::navigate_warm` (warm-reuse path)
- `crates/browser/src/page.rs:1425` — `replace_dom` callsite in warm path
- `crates/browser/src/page.rs:3389-3400` — build_page final drain (8 s)
- `crates/browser/src/pool.rs:1-87` — `PagePool` API
- `crates/browser/src/pool.rs:51-56` — `PagePool::release` (candidate for `low_memory_notification`)
- `crates/browser/src/js/humanize.js:52` — `_sched` resolution to `__bgSetTimeout`
- `crates/browser/examples/sweep_metrics.rs:73-83` — `self_rss_mb` (cold-sweep measurement)
- `crates/browser/examples/sweep_metrics.rs:138-197` — sweep loop with `pool.release()` + `Page` drop
- `crates/dom/src/arena.rs:5-19` — `Dom` struct + `WALK_LIMIT` / `ANCESTOR_LIMIT`
- `crates/dom/src/arena.rs:256-263` — `Dom::remove` (slot recycling)
- `crates/dom/src/arena.rs:655-699` — `collect_elements` (the panic site, also relevant to §10 timing doc)
- `crates/event_loop/src/lib.rs:1-60` — event loop profiling preamble (BROWSER_OXIDE_EVENT_LOOP_PROFILE)
- `crates/js_runtime/src/lib.rs:167-179` — `replace_dom` (DOM-arena retain origin)
- `crates/js_runtime/src/runtime.rs:98-111` — `HEAP_INITIAL = 1 GB` (§5)
- `crates/js_runtime/src/runtime.rs:113-139` — `JsRuntime::new` with all extensions
- `crates/js_runtime/src/runtime.rs:156-162` — `WorkerOwnership::default()` `put` into OpState (fix C applied)
- `crates/js_runtime/src/extensions/worker_ext.rs:1-11` — module docstring (thread-spawn semantics)
- `crates/js_runtime/src/extensions/worker_ext.rs:212-340` — `op_worker_spawn` with `owned` state + tracking push
- `crates/js_runtime/src/extensions/worker_ext.rs:251-258` — 64 MB stack OS thread spawn
- `crates/js_runtime/src/extensions/worker_ext.rs:385-405` — `op_worker_terminate` + `terminate_worker_inner`
- `crates/js_runtime/src/extensions/worker_ext.rs:407-423` — `drain_owned_workers` (fix C reaper)
- `crates/js_runtime/src/extensions/worker_ext.rs:425-434` — `WorkerOwnership` definition
- `crates/js_runtime/src/js/timer_bootstrap.js:40-107` — `_maybeUnref` + `__bgSetTimeout`
- `crates/stealth/src/presets.rs:120` — `chrome_148_macos`
- `crates/stealth/src/presets.rs:413` — `firefox_135_macos`
- `crates/stealth/src/presets.rs:690` — `pixel_9_pro_chrome_148`
- `crates/stealth/src/presets.rs:780-815` — `iphone_15_pro_safari_18` (with "16 declined APIs" rationale)

Benchmark / data:

- `benchmarks/bench_corpus_v2.py:50-54` — `get_rss_mb` (/proc/PID/statm reader)
- `benchmarks/bench_corpus_v2.py:57-98` — `all_descendant_pids` (proc-tree walker)
- `benchmarks/bench_corpus_v2.py:101-106` — `tree_rss_mb`
- `benchmarks/bench_corpus_v2.py:199-202` — `run_playwright` PID-pickup (canonical pattern)
- `benchmarks/bench_corpus_v2.py:223-226` — `run_patchright` PID-pickup
- `benchmarks/bench_corpus_v2.py:252-268` — `run_camoufox` PID-pickup (post-fix)

Sweep JSONs (RSS data source):

- `/tmp/full_sweep_2026_05_24/bo_chrome_148_macos_cold.json` — peak 418.9
- `/tmp/full_sweep_2026_05_24/bo_firefox_135_macos_cold.json` — peak 471.9
- `/tmp/full_sweep_2026_05_24/bo_iphone_15_pro_safari_18_cold.json` — peak 444.7
- `/tmp/full_sweep_2026_05_24/bo_pixel_9_pro_chrome_148_cold.json` — peak 388.0
- `/tmp/full_sweep_2026_05_24/bo_chrome_148_macos_pool.json` — peak 1365 (97 sites, panic on wellsfargo)
- `/tmp/full_sweep_2026_05_24/comp_camoufox.json` — reported 48 (BUG); fix applied to harness
- `/tmp/full_sweep_2026_05_24/comp_playwright.json` — 5618
- `/tmp/full_sweep_2026_05_24/comp_patchright.json` — 5681

Sibling chapters:

- `00_README.md` — release plan overview
- `01_CURRENT_STATE.md` — headline numbers (RSS table is the data source for §1 here)
- `10_TIMING_OPTIMIZATION.md` — companion chapter; same `replace_dom` + `Page::drop` paths
- `docs/PERFORMANCE_2026_05_24.md` — per-page perf investigation that motivated the pool path
- `docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md` — ±5 noise floor (used for §5 acceptance gate)
- `docs/BENCHMARK_2026_05_24.md` — narrative for the underlying sweep
