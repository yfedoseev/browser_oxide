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

## 6. Action items

1. Add `--parallel N` + vendor-aware scheduler to `run_bo_isolated.py`
   (≥150 s same-vendor spacing) → ~4 h → ~1 h for the 4-profile BO gate.
2. (Independent) root-cause the per-nav resource leak (worker/timer/isolate
   reaping on `Page` drop) → enables a shared-process path + lowers prod RSS.
3. Keep Chromium on shared-browser `bench_corpus_v2`; keep camoufox on per-site
   relaunch + retry.

— 2026-05-29
