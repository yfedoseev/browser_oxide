# 21 — V8 snapshot warming + parallel cold sweep

**Status:** planning — both items are detailed specs, neither is
implemented as of 2026-05-24.
**Scope:** detailed implementation for two R&D items mentioned in
`10_TIMING_OPTIMIZATION.md §6.1` and `§6.4`. Each is a one-line
reference there; this chapter is the spec.
**Companion:** `09_MEMORY_OPTIMIZATION.md` (the snapshot-warming memory
saving lands in §2 / §4.2 of `20_MEMORY_BUDGET.md`),
`10_TIMING_OPTIMIZATION.md` (cold-path latency baseline this attacks),
`14_TESTING_VALIDATION.md` (sweep regimen the parallel harness must
preserve).

## 1. Why this chapter exists

`10 §6.1` says "snapshot warming is in scope, look at `runtime.rs:191`"
and `10 §6.4` says "we could spawn N tokio runtimes on N OS threads".
Neither is a buildable spec. A contributor reading the 10 chapter and
trying to land the work has to re-derive:

- Where exactly the snapshot is built today.
- Which bootstrap scripts are in it vs. not.
- How `BrowserRuntimeOptions.startup_snapshot` flows through
  `BrowserJsRuntime::with_options`.
- What changes when `is.None()` vs `is.Some(...)`.
- Whether parallel sweeps trip on the project's hard "V8 isolates are
  per-thread" rule.

This chapter answers all of that with file:line references, gives each
optimisation a numbered implementation plan, and provides a measured
acceptance gate. Both optimisations are independent; either can land
without the other.

# PART A — V8 startup snapshot warming

## A.1 Background — what's already in place

The infrastructure for snapshots is **already wired**. The work is
purely operational (make sure all bootstrap scripts are baked into
the snapshot at build time, and make sure runtime-construction skips
re-execution).

### A.1.1 Snapshot is built once, lazy-static

`crates/js_runtime/src/snapshot.rs:19-23`:

```rust
static RUNTIME_SNAPSHOT: OnceLock<Box<[u8]>> = OnceLock::new();

pub fn get_snapshot() -> &'static [u8] {
    RUNTIME_SNAPSHOT.get_or_init(|| {
        tracing::info!("Creating cold V8 snapshot");
        // ...
    })
}
```

First call constructs a `JsRuntimeForSnapshot`, executes the bootstrap
JS, and serialises the heap to `Box<[u8]>`. Subsequent calls hand back
the cached slice. The cost is paid once per process.

### A.1.2 The snapshot already contains most bootstraps

`crates/js_runtime/src/snapshot.rs:67-99`:

```rust
const BOOTSTRAP_JS: &str = concat!(
    include_str!("js/console_bootstrap.js"),
    "\n",
    include_str!("js/stealth_bootstrap.js"),
    "\n",
    include_str!("js/interfaces_bootstrap.js"),
    "\n",
    include_str!("js/instances_bootstrap.js"),
    "\n",
    include_str!("js/fetch_bootstrap.js"),
    "\n",
    include_str!("js/timer_bootstrap.js"),
    "\n",
    include_str!("js/dom_bootstrap.js"),
    "\n",
    include_str!("js/event_bootstrap.js"),
    "\n",
    include_str!("js/canvas_bootstrap.js"),
    "\n",
    include_str!("js/window_bootstrap.js"),
    "\n",
    include_str!("js/streams_bootstrap.js"),
    "\n",
    include_str!("js/structured_clone.js"),
);

runtime
    .execute_script("<anonymous>", BOOTSTRAP_JS)
    .expect("snapshot bootstrap failed");
```

### A.1.3 Runtime auto-loads the snapshot

`crates/js_runtime/src/lib.rs:50-57`:

```rust
pub fn with_options(dom: Dom, mut options: BrowserRuntimeOptions) -> Self {
    if options.startup_snapshot.is_none() {
        options.startup_snapshot = Some(snapshot::get_snapshot());
    }
    let (inner, nav_signal) = create_runtime_with_signals(dom, options);
    Self { inner, nav_signal }
}
```

And `crates/js_runtime/src/runtime.rs:191-224`:

```rust
// Execute bootstrap JS only if NOT starting from snapshot
if options.startup_snapshot.is_none() {
    const BOOTSTRAP_JS: &str = concat!(
        include_str!("js/console_bootstrap.js"),
        // ...
        include_str!("js/structured_clone.js"),
    );
    runtime
        .execute_script("<anonymous>", BOOTSTRAP_JS)
        .expect("bootstrap failed");
}
```

### A.1.4 Diff between snapshot and runtime bootstrap lists

Compare `snapshot.rs:67-91` against `runtime.rs:193-219`:

| Bootstrap script | In snapshot? | In runtime fallback? | Notes |
|---|:-:|:-:|---|
| `console_bootstrap.js` | yes | yes | aligned |
| `stealth_bootstrap.js` | yes | yes | aligned |
| `interfaces_bootstrap.js` | yes | yes | aligned |
| `shared_apis_bootstrap.js` | **NO** | yes | gap |
| `instances_bootstrap.js` | yes | yes | aligned |
| `fetch_bootstrap.js` | yes | yes | aligned |
| `timer_bootstrap.js` | yes | yes | aligned |
| `dom_bootstrap.js` | yes | yes | aligned |
| `event_bootstrap.js` | yes | yes | aligned |
| `canvas_bootstrap.js` | yes | yes | aligned |
| `window_bootstrap.js` | yes | yes | aligned |
| `streams_bootstrap.js` | yes | yes | aligned |
| `structured_clone.js` | yes | yes | aligned |
| `cleanup_bootstrap.js` | NO | (runs unconditionally at `runtime.rs:230-232`) | by design — cleanup must always run on the per-page runtime |

The headline gap: **`shared_apis_bootstrap.js` (595 lines) is in the
runtime bootstrap list but missing from the snapshot list.** Fixing
that one omission alone closes most of A's work. Every cold isolate
re-executes ~595 lines that should be baked.

### A.1.5 Why `cleanup_bootstrap.js` is the exception

`crates/js_runtime/src/runtime.rs:229-232`:

```rust
// Always run cleanup to hide internals, even when restoring from snapshot.
runtime
    .execute_script("<anonymous>", include_str!("js/cleanup_bootstrap.js"))
    .expect("cleanup failed");
```

`cleanup_bootstrap.js` scrubs identifiers (`browser_oxide` references
in error stacks, helper functions, etc.) that the snapshot would
otherwise re-expose. It must run on **every** runtime — both
post-snapshot and (already covered) post-bootstrap. Do not move it
into the snapshot.

There are several other per-runtime-only sections in `runtime.rs` that
must NOT be baked into the snapshot:

- The captured `Function.prototype.toString` reference at
  `runtime.rs:170-189` — this captures the *genuine* V8 builtin
  *before* `stealth_bootstrap.js` replaces it. If we bake it into the
  snapshot it works correctly, but the iframe-realm install at
  `runtime.rs:181-189` and the native_tag_sym capture at
  `runtime.rs:241-267` are post-bootstrap-only — moving them is out
  of scope for A.
- `init_scripts` (`runtime.rs:290-294`) — caller-provided. Per-page
  by design.
- Stealth profile values — set into `OpState` at runtime, read lazily
  by op handlers. The snapshot doesn't bake any profile (it builds
  with `StealthState::new_with_flags(None, false, true)` at
  `snapshot.rs:58`).

## A.2 The optimisation — what changes

Two concrete changes:

1. **Add `shared_apis_bootstrap.js` to the snapshot list** —
   `snapshot.rs:67-91`.
2. **Remove the matching entry from the runtime fallback list** —
   `runtime.rs:193-219` — but only via the existing
   `if options.startup_snapshot.is_none()` guard, which is already
   correct. The runtime path *only* runs the fallback when no
   snapshot is provided (e.g. in tests that construct a runtime
   directly without going through `with_options`). So the runtime
   list stays intact for that case; the snapshot list grows.

Expected savings:

- **~595 lines of JS no longer re-executed per cold isolate** —
  measured cold-start improvement: -80 to -100 ms (per the 10 §6.1
  "100 ms" estimate, plus the bigger window_bootstrap.js + dom_bootstrap.js
  closures that are already in the snapshot).
- **~20-30 MB per cold isolate** in V8 heap (the parsed AST +
  generated closures + class prototypes no longer have to be allocated
  during runtime construction; they ride in on the snapshot bytes
  which V8 mmap's).

The combined savings × 126 sites × 4 profiles = -50 to -60 s per
4-profile sweep. Negligible against the ~200 minute serial wall, but
substantial when combined with PART B (parallel cold).

## A.3 Implementation plan

### Step 1 — verify the gap

```bash
diff <(grep -E "include_str!.*bootstrap" \
        /home/yfedoseev/projects/browser_oxide/crates/js_runtime/src/snapshot.rs) \
     <(grep -E "include_str!.*bootstrap" \
        /home/yfedoseev/projects/browser_oxide/crates/js_runtime/src/runtime.rs)
```

Expected output: `shared_apis_bootstrap.js` appears only in
`runtime.rs`.

### Step 2 — add `shared_apis_bootstrap.js` to the snapshot

**File**: `crates/js_runtime/src/snapshot.rs:67-91`.

Insert between `interfaces_bootstrap.js` and `instances_bootstrap.js`
(matching the order in `runtime.rs:193-219`):

```rust
const BOOTSTRAP_JS: &str = concat!(
    include_str!("js/console_bootstrap.js"),
    "\n",
    include_str!("js/stealth_bootstrap.js"),
    "\n",
    include_str!("js/interfaces_bootstrap.js"),
    "\n",
    include_str!("js/shared_apis_bootstrap.js"),  // <-- NEW
    "\n",
    include_str!("js/instances_bootstrap.js"),
    "\n",
    // ... rest unchanged
);
```

### Step 3 — verify `shared_apis_bootstrap.js` is snapshot-safe

Read `crates/js_runtime/src/js/shared_apis_bootstrap.js` and confirm
NONE of the following hold:
- It reads `globalThis._browser_oxide.__stealthProfile` or any other
  per-page state.
- It calls any op that requires `DomState` to be populated with a
  per-page DOM (snapshot `DomState` is `Dom::new()` = empty document).
- It conditionally executes based on `secure_context` /
  `cross_origin_isolated` (snapshot is built with
  `is_secure_context = true`, see `snapshot.rs:58`).

If any of these hold, that section must be moved to
`cleanup_bootstrap.js` (which always runs per-page) OR gated behind a
lazy getter that reads the profile at access time. The bake-time vs
runtime-time boundary matters — anything bake-time-frozen is
permanently the snapshot-build values for the rest of the process's
isolates.

### Step 4 — rebuild and verify the snapshot

```bash
cargo build --release -p js_runtime
cargo build --release -p browser
```

`snapshot.rs:67-101` runs the first time `get_snapshot()` is called
(lazy), not at compile time. So the rebuild itself is fast; the
snapshot blob is rebuilt on first runtime construction.

### Step 5 — runtime fallback is already correct, no change needed

`runtime.rs:191-224` is gated on
`if options.startup_snapshot.is_none()`. When `with_options` is
called (`lib.rs:50-57`) the snapshot is always populated, so this
branch is dead in the normal path. It only runs for direct
`JsRuntime::new(RuntimeOptions { startup_snapshot: None, .. })`
callers — e.g. some integration tests. Those tests would slow down by
~80 ms per construction, which is acceptable.

### Step 6 — verify per-runtime extras still run

`runtime.rs:229-232` (cleanup_bootstrap.js — always runs),
`runtime.rs:241-267` (native_tag_sym capture — always runs),
`runtime.rs:276-279` (`install_native_fp_tostring` — always runs),
and `runtime.rs:290-294` (init_scripts) all remain outside the
`is_none()` branch and are unaffected by A.

### Step 7 — profile-specific values

The stealth profile is set in `OpState` AFTER snapshot load (per
`runtime.rs:92-96` for `stealth_state`; `runtime.rs:146-152` for
`OpState.put`). Bootstrap scripts that need profile values call ops
(`op_get_profile_value`, etc.) which read from `StealthState` at
call-time. The snapshot bakes the *code* of the bootstrap scripts;
each per-page runtime gets its own profile-specific values when those
ops are invoked. Verify by reading
`crates/js_runtime/src/extensions/stealth_ext.rs` ops — they all
take `#[state] state: &StealthState` (read at call time), never embed
profile data in static state.

## A.4 Risks

| Risk | Mitigation |
|---|---|
| Snapshot binary grows from ~3 MB to ~5-10 MB | Acceptable — the binary is already 60+ MB; another 5 MB is rounding. Distributed via `include_bytes!`/cached `OnceLock` blob, no extra disk cost at install. |
| `shared_apis_bootstrap.js` has bake-time-dependent code (e.g. reads `Date.now()` at top-level and stores it) | Step 3 audit catches this. Most "shared APIs" code defines classes and methods — bake-safe by construction. |
| Per-realm state (iframe child contexts) doesn't get the snapshot | Already handled — `op_create_child_realm` (`crates/js_runtime/src/native_fns.rs`) creates a NEW v8 Context inside the same isolate; the snapshot's globals are not inherited. Iframe realms run their own bootstrap install path; A does not change that. |
| Tests that rely on `is_none()` codepath silently regress | Run `cargo test --workspace -- --test-threads=1` before commit. Any test that constructs a `JsRuntime` directly will exercise the fallback path and confirm the runtime bootstrap list still works. |
| Profile-value caching by bootstrap code | Per Step 3, audit each script for capture-at-load patterns. The current `stealth_bootstrap.js` (140 lines, `crates/js_runtime/src/js/stealth_bootstrap.js`) installs `_nativeTag` / `_maskFunction` helpers — bake-safe. Verify `shared_apis_bootstrap.js` similarly. |
| Snapshot version skew during development | Rare; `OnceLock` is per-process so a stale snapshot never survives a fresh `cargo build`. CI runs `cargo build` before tests, so this is non-issue. |

## A.5 Validation

### Cold-start time measurement

```bash
# Baseline (current code)
target/release/examples/sweep_metrics chrome_148_macos \
    tests/holistic_corpus_v2.json /tmp/baseline_cold.json

# Extract per-site "ms" for the first 10 sites (which are dominated by
# isolate construction, not page complexity)
python3 -c "
import json
d = json.load(open('/tmp/baseline_cold.json'))
ms = [r['ms'] for r in d['results'][:10]]
print('first10 mean cold ms:', sum(ms)/len(ms))
print('first10 raw:', ms)
"
```

After the change, repeat. Acceptance: ≥ 80 ms reduction on the
first-10-site mean (the first 10 sites are most dominated by isolate
construction; later sites' ms is more page-complexity than isolate
spin-up).

### Memory measurement

```bash
# RSS at site 1
python3 -c "
import json
b = json.load(open('/tmp/baseline_cold.json'))
a = json.load(open('/tmp/after_cold.json'))
print(f'baseline site 1 RSS: {b[\"results\"][0][\"rss_mb\"]:.1f} MB')
print(f'after    site 1 RSS: {a[\"results\"][0][\"rss_mb\"]:.1f} MB')
print(f'delta: {a[\"results\"][0][\"rss_mb\"] - b[\"results\"][0][\"rss_mb\"]:+.1f} MB')
"
```

Acceptance: ≥ 20 MB reduction at site 1 (the cold-start memory is
mostly isolate baseline; later sites' RSS is dominated by per-page
allocations and any leaks).

### Functional regression

```bash
cargo test --workspace -- --test-threads=1
```

Acceptance: full test suite still passes. No skipped tests.

### Pass-rate regression

```bash
# Full 126-site sweep
target/release/examples/sweep_metrics chrome_148_macos \
    tests/holistic_corpus_v2.json /tmp/after_cold_full.json

# Compare to baseline
python3 -c "
import json
b = json.load(open('/tmp/baseline_cold_full.json'))
a = json.load(open('/tmp/after_cold_full.json'))
print(f'baseline pass: {b[\"summary\"][\"pass\"]}, after pass: {a[\"summary\"][\"pass\"]}')
"
```

Acceptance: Δpass within ±5 sites (within
`docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md` noise floor).

# PART B — Parallel cold sweep across N profiles

## B.1 Background

The cold sweep is the source of the 4-profile pass-rate table in
`01_CURRENT_STATE.md` and `09 §2`. It runs sequentially today:

```
chrome_148_macos cold    — 46 min wall
firefox_135_macos cold   — 50 min wall
iphone_15_pro_safari_18  — 54 min wall
pixel_9_pro_chrome_148   — 46 min wall
                          ─────────
                           ~196 min total
```

For a release-candidate validation that runs 3 times for variance
(per `14_TESTING_VALIDATION.md`), that's 3 × 196 = ~10 hours just
for the cold sweeps. A 4-way parallel harness with one thread per
profile finishes in ~54 min (the slowest profile), 11× faster end-to-end.

## B.2 Why V8-isolates-per-thread is NOT a blocker

`CLAUDE.md` says:

> Tests are single-threaded. V8 isolates are per-thread; running
> multi-threaded crashes the test process. CI enforces
> `--test-threads=1`.

The constraint is "**one V8 isolate cannot be touched from two
threads.**" That's a Cargo-test-runner concern: `cargo test` spawns
tasks across a shared thread pool, and any `JsRuntime` constructed in
the test harness gets moved between threads — which V8 forbids.

Spawning N OS threads, each owning its OWN V8 isolate that NEVER
crosses thread boundaries, is fully supported. That's literally how
Chrome itself runs N renderer processes/threads. The sweep harness is
not `cargo test`; it's a `cargo run --example` binary, where the
author controls thread placement.

The pattern is:

- 1 OS thread per profile
- Inside the thread, build a `tokio::runtime::Builder::new_current_thread()`
  + `tokio::task::LocalSet` (matches the existing
  `sweep_metrics.rs:85` `#[tokio::main(flavor = "current_thread")]`)
- That thread's isolate is constructed, used, and dropped without
  ever being moved.

Each thread is identical to the current single-thread sweep — N
copies running side-by-side.

## B.3 The optimisation — what changes

Two delivery paths, in priority order:

1. **Primary (v0.1.0)**: a shell-script wrapper that spawns N copies
   of the existing `sweep_metrics` binary as separate processes,
   staggered to avoid WAF rate-limit triggers. Zero engine code
   change; per-process RSS attribution comes for free.
2. **Stretch (post-v0.1.0)**: an in-process
   `crates/browser/examples/sweep_metrics_parallel.rs` that spawns
   N OS threads, each with its own V8 isolate. Lower memory per
   profile (no 4× process baseline), at the cost of having to handle
   shared `/proc/self/statm` accounting.

The primary path covers the headline benefit (4-profile sweep wall
~196 min → ~55 min) without touching the engine. The stretch path is
for customers who want in-process pool parallelism.

## B.4 Implementation plan — shell harness (primary)

**File**: `benchmarks/run_parallel_sweep.sh` (new):

```bash
#!/bin/bash
# Parallel 4-profile cold sweep — runs each profile in its own process
# for clean per-profile RSS attribution. Staggers profile starts by 5 s
# to avoid WAF rate-limit triggers on simultaneous TLS handshakes from
# the same IP.
set -e
CORPUS=${1:-tests/holistic_corpus_v2.json}
OUT_DIR=${2:-/tmp/parallel_sweep_$(date +%Y_%m_%d)}
mkdir -p "$OUT_DIR"

profiles=(chrome_148_macos firefox_135_macos iphone_15_pro_safari_18 pixel_9_pro_chrome_148)
pids=()
i=0
for p in "${profiles[@]}"; do
    sleep $((i * 5))  # stagger TLS handshakes
    target/release/examples/sweep_metrics "$p" "$CORPUS" "$OUT_DIR/$p.json" \
        > "$OUT_DIR/$p.log" 2>&1 &
    pids+=($!)
    i=$((i + 1))
done
for pid in "${pids[@]}"; do wait "$pid"; done
echo "Parallel sweep complete: $OUT_DIR"
```

Mirrors the structure of the existing `benchmarks/run_full_sweep.sh`
(serial loop) but backgrounds each invocation. The 5 s stagger is the
key WAF-friendliness lever (per B.5).

## B.5 Implementation plan — in-process harness (stretch)

**File**: `crates/browser/examples/sweep_metrics_parallel.rs` (new).

Structure:

- Parse comma-separated profile list from argv.
- For each profile, spawn an OS thread via
  `std::thread::Builder::new().name(...).stack_size(8 * 1024 * 1024).spawn(move || ...)`.
- Inside each thread: build a fresh
  `tokio::runtime::Builder::new_current_thread().enable_all().build()`
  + `tokio::task::LocalSet`, then `block_on` the existing
  per-profile sweep body (a lift from
  `crates/browser/examples/sweep_metrics.rs:108-269`).
- Stagger profile starts by `(i * 5) seconds` `std::thread::sleep`
  before each thread enters its sweep.
- Main thread `join()`s all worker threads, then writes
  `<out_dir>/_summary.json` with `wall_total_ms = max(per-profile)`
  and per-profile pass counts.

**Per-thread tokio invariant**: each worker thread MUST build its own
runtime — `#[tokio::main]` would put one runtime on the main thread and
all worker spawns would land in that runtime's thread pool, defeating
the per-thread-isolate invariant. The reference pattern is
`op_worker_spawn` at
`crates/js_runtime/src/extensions/worker_ext.rs:268-277`, which has
been running per-thread current-thread runtimes inside worker isolates
in production. Each is fully independent.

**Keep the canonical serial harness intact** —
`crates/browser/examples/sweep_metrics.rs` stays as the single-thread
reference. Do not refactor its body into a shared library function;
the duplication is intentional so a regression in the parallel harness
cannot break the canonical baseline.

**RSS-measurement caveat**: `self_rss_mb()` at
`sweep_metrics.rs:73-83` reads `/proc/self/statm`, which is
process-wide. In the in-process harness every per-site `rss_mb` is
actually the sum across all 4 thread-isolates. Annotate the
per-profile JSON `summary._note` field with this caveat; rely on the
shell-script harness for the per-profile-RSS headline number.

## B.6 Risks

| Risk | Mitigation |
|---|---|
| **WAF detection on parallel TLS** — 4 simultaneous TLS handshakes from one IP to `amazon.com` may trip rate limits (per `02_GAP_ANALYSIS.md §10` x-com hypothesis) | 5 s stagger between profile starts. Different profiles fingerprint as different browsers (Chrome / Firefox / Safari / mobile-Chrome) — same IP, different TLS ClientHello + UA, which WAFs SHOULD treat as separate sessions. Validate per §B.7. |
| **Per-process / per-thread memory cost** — 4× the 50-70 MB baseline + 4× the per-page DOM arena. Peak: 4× single-profile peak ≈ 1.5-2 GB | Size the host machine. The current sweep box has 16+ GB; 2 GB for the parallel sweep is fine. Document host requirement in the harness preamble. |
| **Multiple per-thread tokio runtimes in one process** (in-process harness only) | `op_worker_spawn` already runs per-thread current-thread runtimes inside worker isolates in production — no regressions. Each runtime is fully independent. |
| **`SharedSession` cookies cross-pollination** (in-process harness only) — process-wide `SharedSession` (per `09 §2` row #11) shared across threads | By design — existing single-threaded sweeps also share. Anti-bot sites set cookies per IP+UA so cross-profile cookies are not a behavioural issue. Per-thread isolation is out of scope. |
| **Stdout interleaving** (in-process harness only) — 4 threads scrambling per-site log lines | Each thread writes to its own log file; stdout summary only at thread join. |

## B.7 Validation

```bash
# Functional + wall-time
time benchmarks/run_parallel_sweep.sh tests/holistic_corpus_v2.json /tmp/parallel_v1
ls /tmp/parallel_v1/*.json

# Pass-rate parity (compare against single-profile serial baseline)
python3 -c "
import json
for p in ['chrome_148_macos','firefox_135_macos','iphone_15_pro_safari_18','pixel_9_pro_chrome_148']:
    s = json.load(open(f'~/projects/browser_oxide_internal/benchmarks/baselines/2026-05-24/bo_{p}_cold.json'))
    par = json.load(open(f'/tmp/parallel_v1/{p}.json'))
    print(f'{p}: serial pass={s[\"summary\"][\"pass\"]}  parallel pass={par[\"summary\"][\"pass\"]}  delta={par[\"summary\"][\"pass\"]-s[\"summary\"][\"pass\"]:+d}')
"
```

**Acceptance** (all must hold):
- Parallel wall time ≤ serial wall time ÷ 3 (4 profiles, conservative
  3× speedup expected after stagger overhead).
- |Δpass| ≤ 2 per profile vs the serial baseline (within
  `docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md` noise floor).
- Per-site regression count (sites PASS-in-serial / FAIL-in-parallel)
  ≤ 3 per profile. If > 5, investigate the specific sites — most
  likely WAF rate-limit hits; tune the stagger interval or shard
  the corpus by host.

# PART C — Combined effect

Independent benefits multiply, sort of:

| Optimisation | Per-cold time | Per-cold RSS | 4-profile wall |
|---|--:|--:|--:|
| Today | — | 388-472 MB | ~196 min serial |
| A (snapshot warming) alone | -80 to -100 ms | -20 to -30 MB | -16 to -20 s per profile × 4 = ~ -75 s |
| B (parallel sweep) alone | unchanged per cold | unchanged | ~55 min (max(per-profile) instead of sum) |
| A + B together | -80 to -100 ms per cold | -20 to -30 MB per cold | ~55 min × (1 - 80ms/15000ms) ≈ 54.7 min |

The dominant timing win is B (76% wall-time reduction). A is the
dominant memory + correctness win (every cold isolate saves ~25 MB).
They're independent — either can land first.

For v0.1.0 the highest-leverage ordering is:

1. Ship A — small, contained change in `snapshot.rs:67-91`. Validates
   the auto-snapshot infrastructure end-to-end.
2. Ship B (shell script form) — a `benchmarks/run_parallel_sweep.sh`
   that calls the existing `sweep_metrics` binary in 4 backgrounded
   processes. No code change to the engine.
3. (Optional, post-v0.1.0) Ship in-process
   `sweep_metrics_parallel.rs` for the case where the customer needs
   `PagePool` parallelism inside one process.

## 1. Acceptance for v0.1.0

- [ ] **V8 snapshot warming implemented** (PART A) — diff between
      `snapshot.rs:67-91` and `runtime.rs:193-219` shows zero gaps
      (`shared_apis_bootstrap.js` added to snapshot).
- [ ] **A measured** — per the §A.5 validation: cold-start ms ≥ 80 ms
      lower on the first-10-site mean; site-1 RSS ≥ 20 MB lower.
- [ ] **A passes regression** — `cargo test --workspace --
      --test-threads=1` clean; ±5 pass delta on 126-site sweep.
- [ ] **Parallel sweep harness implemented** (PART B) — at minimum
      `benchmarks/run_parallel_sweep.sh` lands and runs 4 profiles
      concurrently. The Rust `sweep_metrics_parallel.rs` is
      stretch.
- [ ] **B measured** — 4-profile sweep wall ≤ 60 min (from ~200 min
      serial), per §B.7.
- [ ] **B passes rate-limit check** — no profile regresses > 5 pass
      sites in parallel vs. serial (per §B.7 per-site check).
- [ ] **A + B combined sweep** — one 3-run aggregated parallel sweep
      with snapshot warming committed; numbers published in
      `01_CURRENT_STATE.md` with the parallel-wall headline.
- [ ] **Documentation** — the shell-script harness preamble explains
      the staggered start + per-process RSS attribution; the
      `sweep_metrics_parallel.rs` (if shipped) docstring explains
      the per-thread isolate invariant and links back to
      `CLAUDE.md` for the original "V8-per-thread" rule.

## 2. Files referenced

All paths absolute under `/home/yfedoseev/projects/browser_oxide`.

**Snapshot warming (PART A) target sites**:

- `crates/js_runtime/src/snapshot.rs:19-23` —
  **`RUNTIME_SNAPSHOT: OnceLock<Box<[u8]>>` + `get_snapshot`** (A's
  primary change site)
- `crates/js_runtime/src/snapshot.rs:26-44` — `JsRuntimeForSnapshot`
  extension list (must match `runtime.rs:114-131`)
- `crates/js_runtime/src/snapshot.rs:46-64` — `OpState` population
  for snapshot build
- `crates/js_runtime/src/snapshot.rs:67-91` —
  **`BOOTSTRAP_JS` const** (A's insertion point for
  `shared_apis_bootstrap.js`)
- `crates/js_runtime/src/snapshot.rs:97-101` — actual snapshot bake
  via `runtime.execute_script`
- `crates/js_runtime/src/lib.rs:50-57` — **`BrowserJsRuntime::with_options`**
  (where snapshot is auto-loaded; A relies on this being unchanged)
- `crates/js_runtime/src/runtime.rs:30-53` — `BrowserRuntimeOptions`
  (includes `startup_snapshot: Option<&'static [u8]>`)
- `crates/js_runtime/src/runtime.rs:113-139` — `JsRuntime::new`
  (`startup_snapshot` wired at line 132)
- `crates/js_runtime/src/runtime.rs:191-224` — **bootstrap-skip
  branch** gated on `if options.startup_snapshot.is_none()` (A's
  invariant)
- `crates/js_runtime/src/runtime.rs:229-232` — `cleanup_bootstrap.js`
  always runs (A §A.1.5 — do NOT move into snapshot)
- `crates/js_runtime/src/runtime.rs:241-267` — `native_tag_sym`
  capture (always runs)
- `crates/js_runtime/src/runtime.rs:276-279` —
  `install_native_fp_tostring` (always runs)
- `crates/js_runtime/src/runtime.rs:290-294` — `init_scripts` loop
  (per-page, always runs)
- `crates/js_runtime/src/js/shared_apis_bootstrap.js` (595 lines) —
  missing-from-snapshot file A adds (audit per §A.3 Step 3)
- `crates/js_runtime/src/js/cleanup_bootstrap.js` (582 lines) —
  per-page, not in snapshot, by design
- `crates/js_runtime/src/extensions/stealth_ext.rs` — confirm ops
  read `StealthState` lazily (A §A.3 Step 7 audit)

**Parallel sweep (PART B) target sites**:

- `crates/browser/examples/sweep_metrics.rs:1-100` — canonical
  single-thread harness preamble; do NOT refactor away
- `crates/browser/examples/sweep_metrics.rs:73-83` — `self_rss_mb`
  (B in-process caveat — process-wide RSS)
- `crates/browser/examples/sweep_metrics.rs:85` —
  `#[tokio::main(flavor = "current_thread")]` (why parallel cannot
  reuse it)
- `crates/browser/examples/sweep_metrics.rs:101-269` — per-profile
  sweep body (copy target for in-process harness)
- `crates/browser/src/page.rs:200-214` — `Page` struct
- `crates/browser/src/pool.rs:1-87` — `PagePool` (relevant for an
  in-process parallel-pool extension)
- `crates/js_runtime/src/extensions/worker_ext.rs:268-277` —
  **existing per-thread tokio runtime build** (reference pattern
  for B in-process harness)
- `crates/event_loop/src/lib.rs:1-60` — event-loop profiling
  preamble (per-thread profile dumps)
- `crates/stealth/src/presets.rs:120 / 413 / 690 / 795` — four
  stealth profiles

**Benchmark / data**:

- `benchmarks/run_full_sweep.sh` — existing serial 4-profile sweep
  (B harness mirrors its structure)
- `benchmarks/run_parallel_sweep.sh` — **new**, per §B.4
- `benchmarks/bench_corpus_v2.py:101-106` — `tree_rss_mb` (why
  per-process RSS attribution matters)
- `~/projects/browser_oxide_internal/benchmarks/baselines/2026-05-24/bo_*_cold.json` — serial baselines for
  the §B.7 pass-rate parity check

**Project rules / conventions**:

- `CLAUDE.md` — "V8 isolates are per-thread" — the rule B navigates
  around (N isolates in N threads, each isolate never moves)
- `docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md` — ±5 noise floor for the
  §A.5 / §B.7 pass-rate gates

**Sibling chapters**:

- `09_MEMORY_OPTIMIZATION.md` — high-level memory triage; A lands in
  its budget table
- `10_TIMING_OPTIMIZATION.md` — §6.1 / §6.4 reference this chapter
- `14_TESTING_VALIDATION.md` — multi-run aggregation regime for the
  §A.5 / §B.7 gates
- `20_MEMORY_BUDGET.md` — owns the §4.2 entry pointing here; A's
  per-cold RSS saving lands in rows #1 + #2 of §2 there
