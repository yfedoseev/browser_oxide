# Gate performance — the 10h→3h trick, and can browser_oxide do the same?

> How the competitor sweep went from ~10h to ~3h, the per-engine cost model,
> and the (different) lever that speeds up the browser_oxide gate.

## 1. The cost model

Per-engine sweep wall-time over N=126 sites:

```
wall ≈ N × (per_site_launch_overhead + per_site_nav_time)
```

The trick is which term dominates:

| Engine | Per-site **launch** overhead | Per-site **nav** | Relaunch-per-site verdict |
|---|---|---|---|
| Chromium (playwright/patchright) | **~50 s** (cold chromium + driver handshake) | ~8-43 s | relaunch DOMINATES → catastrophic |
| Camoufox (Firefox) | ~5 s | ~10-40 s | moderate |
| **browser_oxide** | **~1-2 s** (V8 snapshot restore) | ~10-90 s | launch is NEGLIGIBLE |

## 2. The 10h→3h win (Chromium): reuse one browser

Per-site **relaunch** of Chromium was ~55 s/site (mostly the ~50 s launch),
so `55 s × 126 × 5 engines ≈ 10 h`.

Chromium's driver is **stable across a long sweep**, so we reuse **one
browser for all 126 sites** (the canonical `bench_corpus_v2`): launch once,
`new_page` per site. That deletes the 50 s × 126 launch tax → **~8-43 s/site →
~15-30 min/engine**. Three Chromium engines: ~10 h → **~1-1.5 h**, plus camoufox.

**Camoufox can't** reuse one browser — its playwright-firefox driver crashes in
a sustained loop (`Connection closed while reading from the driver` after
~3-30 pages), so camoufox **must** relaunch per site (`run_competitor_isolated.py`).
That's tolerable only because Firefox launch (~5 s) is far cheaper than
Chromium's, and a retry recovers the ~20% of launches that still flake.

**So the rule:** reuse the browser when the driver is stable; relaunch per
site only when it isn't.

## 3. Why browser_oxide is already near-optimal — and why the Chromium trick barely helps it

BO's per-site runner (`run_bo_isolated.py`) relaunches the **process** per
site. Measured (full gate, 2026-05-29): **~60-65 min/profile** (chrome 3604 s,
pixel 3897 s, iphone 3674 s, firefox 3905 s). The per-site **nav** times sum to
~53 min; process-launch overhead is only ~4-7 min total (V8 snapshot restore is
~1-2 s). **BO is nav-bound, not launch-bound** — the opposite of Chromium.

⇒ The Chromium "reuse the browser" trick would save BO only ~4-7 min/profile —
**and we can't even use it**: a single BO process running all 126 sites **runs
away** (1.7 GB RSS, 100% CPU, stuck at site ~104 after 7 h) because the now-
heavy passing pages (Amazon/booking load 800 KB-1.2 MB + workers/timers/
isolates) accumulate per-nav resources that aren't fully reaped on `Page` drop.
Per-site process isolation is therefore both **necessary** (sidesteps the leak)
and **cheap** (fast V8 restore) — a happy accident.

## 3b. Why BO "init" is 1.5 s even though we're not a real browser — and the fix

Measured per-site init (`BROWSER_OXIDE_BUILD_PROFILE=1`, example.com):

```
[bp]    0ms  parse_html + find_scripts + find_stylesheets
[bp] 1543ms  BrowserJsRuntime::with_options (V8 isolate + bootstrap)   <-- ALL of it
[bp]    0ms  everything else (location, cookies, scripts, drains)
```

**1543 ms is not a browser launch — it's building the V8 snapshot at RUNTIME.**
`with_options` (`runtime.rs:51`) sets `startup_snapshot = Some(snapshot::get_snapshot())`,
and `get_snapshot()` (`snapshot.rs`) **executes all 12 JS bootstraps**
(console/stealth/interfaces/shared_apis/instances/fetch/timer/dom/event/canvas/
window/streams) into a `JsRuntimeForSnapshot` and serializes it — caching the
bytes in a process-local `static OnceLock<RUNTIME_SNAPSHOT>`.

The gate runs a **fresh process per site**, so that `OnceLock` is empty every
time → **the snapshot is rebuilt from scratch on every one of the 126 sites
(1.5 s each)**. We pay a real-browser-sized startup for a reason a real browser
doesn't have: regenerating the JS environment per process.

**Fix — build the snapshot at COMPILE time (standard deno_core pattern):**
- Add a `build.rs` that calls the existing `snapshot.rs` creation path, writes
  the blob to `$OUT_DIR/RUNTIME_SNAPSHOT.bin`.
- `get_snapshot()` becomes `include_bytes!(concat!(env!("OUT_DIR"), "/RUNTIME_SNAPSHOT.bin"))`
  — every process *restores* the embedded blob (**~50-100 ms**) instead of
  *building* it (1.5 s). ~15× faster cold init.

**Impact:**
- **Production cold-start:** every first page-load drops from ~1.5 s → ~0.1 s
  (the bigger win — this is latency on *every* fresh BO instance, not just the gate).
- **Gate:** per-site init 1.5 s → ~0.1 s ⇒ ~3 min/profile saved (modest — BO is
  nav-bound), but it compounds with pool reuse and parallelism below.

Caveat: the snapshot bakes the bootstraps as of build time; per-nav dynamic
state (profile values, `__keepLongTimersRefed`, etc.) is still applied live
post-restore (already the design — `cleanup_bootstrap.js` runs even when
restoring), so a compile-time snapshot is safe.

## 4. The lever that DOES speed up the BO gate: parallelism across vendors

BO's bottleneck is nav-time × 126, run **sequentially**. The win is to run
sites **concurrently in separate processes**, because:

- BO has **no shared-browser stability constraint** (each site is already its
  own process), and the box has **8 cores** (only ~1 used today).
- Parallel requests to **different origins** are fine — real browsers open many
  tabs. The only hazard is **same-vendor same-IP clustering** (the AWS-WAF
  token issue), which is a *scheduling* constraint, not a reason to stay serial.

**Proposed `run_bo_isolated.py --parallel N` (vendor-aware scheduler):**
- Run up to **N≈6** site-processes concurrently (8 cores − headroom).
- Constraint: **never two same-vendor sites in flight, and ≥150 s between two
  same-vendor starts** (reuse `corpus_vendor_map` + the spaced-run policy). AWS
  ×9, DataDome, Akamai, Kasada each serialize within their vendor; everything
  else parallelizes freely.
- Expected: ~53 min nav / ~6 effective parallelism on untagged sites →
  **~12-18 min/profile** → 4 profiles **~1 h** (from ~4 h). The vendor-spaced
  sites cap the floor but they're a minority.

This is the BO analog of the Chromium win: Chromium avoids paying the launch
tax 126× by reusing the browser; BO avoids paying the **nav tax serially** by
running cheap processes in parallel (it can't reuse one process due to the leak).

## 5. Bonus: fixing the per-nav resource leak (real engine bug)

The single-process runaway is a genuine bug worth fixing independent of the
gate: production scrapers reuse one engine for thousands of navs. Likely
culprits (drop-time reaping):
- **owned Web Workers** not always terminated/reaped (`worker_ext` registry) —
  64 MB stack each;
- **timers/intervals** left refed (esp. challenge navs with `__keepLongTimersRefed`);
- **V8 isolate / DOM arena** retained across the per-iteration runtime.

Fixing these would (a) let a single BO process survive 126 sites (enabling a
shared-process fast path), and (b) cut steady-state RSS for long-lived
deployments. Until then, **per-site isolation + parallelism (§4)** is the
gate-speed answer.

## 6. Action items (ranked)

1. **Compile-time V8 snapshot** (`build.rs` → `include_bytes!`) — cuts cold init
   ~1.5 s → ~0.1 s. **Biggest production win** (every fresh BO instance pays this
   today), plus ~3 min/profile in the gate. High ROI, self-contained.
2. **`run_bo_isolated.py --parallel N`** + vendor-aware scheduler (≥150 s
   same-vendor spacing) → BO 4-profile gate ~4 h → ~1 h. Uses idle cores; AWS/
   DataDome/Akamai/Kasada serialize within-vendor, everything else parallelizes.
3. **Root-cause the per-nav resource leak** (worker/timer/isolate reaping on
   `Page` drop) → enables a shared-process fast path (pay init once, reuse across
   navs — the true "reuse" the user asked for) + lowers steady-state RSS for
   long-lived production deployments.
4. Keep Chromium on shared-browser `bench_corpus_v2`; keep camoufox on per-site
   relaunch + retry.

**The "reuse" the user asked for, precisely:** (1) makes per-process init cheap
(snapshot restore); (3) lets one process serve many navs without the runaway
(pool/`navigate_warm` already implements the reuse path — it's just blocked by
the leak today). Together they give BO the equivalent of Chromium's shared-
browser speedup, and BO *should* end up faster than any real browser because its
"launch" is a memcpy-grade snapshot restore, not a process+renderer spin-up.

— 2026-05-29
