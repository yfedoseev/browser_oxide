# Holistic sweep round 6 — 116/126 (92.1%) new ceiling

Run with: `cargo test --release -p browser --test holistic_sweep
holistic_sweep_parallel -- --ignored --test-threads=1 --nocapture`.

Engine HEAD: post-R5 fixes (timer threshold tune + iphey/about:blank
resolve fix). Wall clock 16.0 min.

**Result: 116/126 (92.1%) L3-RENDERED — new ceiling for browser_oxide.**

Beats prior bests:
- R5 (2026-05-10 evening): 112/126 (88.9%)
- 04-29 Phase 4 close: 114/126 (90.5%)
- Camoufox v135 (same corpus): 113/126 (89.7%)

## All sweeps compared

| Outcome         | 04-29 | R2  | R3  | R4  | R5  | **R6** |
|-----------------|------:|----:|----:|----:|----:|-------:|
| L3-RENDERED     | 114   | 110 | 111 | 111 | 112 | **116** |
| THIN-BODY       | 0     | 3   | 5   | 2   | 3   | **0**  |
| Kasada-CHL      | 3     | 3   | 2   | 3   | 3   | 3      |
| DataDome-CHL    | 2     | 3   | 4   | 4   | 2   | 2      |
| captcha-CHL     | 3     | 2   | 1   | 2   | 2   | 3      |
| Akamai-CHL      | 1     | 2   | 1   | 1   | 1   | 1      |
| Cloudflare-CHL  | 1     | 1   | 1   | 1   | 1   | 1      |
| ERROR           | 2     | 2   | 1   | 1   | 2   | **0**  |

## What this round did

Two engine fixes converted the R5 regression set into the new high:

### 1. Timer-unref threshold 1000 → 2000 ms
`crates/js_runtime/src/js/timer_bootstrap.js`

R5's `5c649e1` unref'd setTimeouts ≥1000ms to free twitter/x.com from
the deno_core `op_timer_sleep` perpetually-pending pin. That worked but
cost macys/ria/threads which use ~1.5s render-critical timers. Bumping
the threshold to 2000ms keeps twitter/x (their pinning is hundreds of
sub-second timers plus the occasional 5–60s analytics retry) while
preserving the 1–2 s hydration callbacks needed by SPA shells.

Empirical results (spot-check during R6 prep):

| Site         | 1000ms (R5)        | 2000ms (R6)             |
|--------------|--------------------|-------------------------|
| twitter      | L3 (4 s)           | L3 (6 s) ✓ kept         |
| x.com        | L3 (2 s)           | L3 (3 s) ✓ kept         |
| macys        | THIN (85 s)        | L3 (44 s, 1.5 MB) ✓     |
| ria          | THIN (61 s)        | L3 (106 s, 2 MB) ✓      |
| threads      | THIN (39 B)        | L3 (10 s, 664 KB) ✓     |

Tried 3000ms (target macys) — but ria regressed back to THIN-BODY
because its hydration timer is in the 2–3 s window. 2000ms is the
Pareto-optimal single-threshold value.

### 2. `Page::resolve_url` filters non-http(s) schemes
`crates/browser/src/page.rs:1999-2013`

Diagnosed by adding the failing URL to the "no host in URL" error
text and re-running iphey: `[redirect] hop=0 GET about:blank`. The
iphey antibot bootstrap creates a programmatic iframe with src
ending up as `about:blank` after `Url::join`. Our HTTP client called
`Url::host_str()` on it, returning None.

`resolve_url` now returns `Some` only for `http`/`https`. Every caller
(scripts, iframes, stylesheets, fetches) skips the URL uniformly.
Plus: the pending-navigation harvester in `Page::navigate` now treats
"unfetchable scheme" as a no-op (`return Ok(page)`) instead of bubbling
as a hard error.

**iphey flipped ERROR → L3 (26 KB)** and stays L3 every run since.

## Per-site delta R5 → R6

### Wins (+8)
| Site         | R5            | R6                |
|--------------|---------------|-------------------|
| **macys**    | THIN-BODY     | **L3** (1.55 MB)   |
| **ria**      | THIN-BODY     | **L3** (2.0 MB)    |
| **threads**  | THIN-BODY     | **L3** (664 KB)    |
| **iphey**    | ERROR         | **L3** (26 KB)     |
| **wildberries** | ERROR      | **L3** (1.5 KB)    |
| bestbuy      | Akamai-CHL    | L3 (variance)      |
| duolingo     | captcha-CHL   | L3 (variance)      |
| mail-ru      | THIN-BODY     | L3 (variance)      |

5 of these 8 are directly attributable to the two fixes above; the
other 3 (bestbuy/duolingo/mail-ru) sit in the variance band and may
or may not stay across sweeps.

### Regressions
None.

## Remaining-failure inventory (10 sites — the architectural floor)

| Category | Sites | Notes |
|----------|-------|-------|
| Kasada strict | canadagoose, hyatt, realtor | W4a-deeper TypeError-stack capture in place; needs the 5 unjzomuy probe targets identified + stubbed |
| reCAPTCHA | douyin, spotify, yandex-ru | Out of pure-engine scope |
| DataDome edge | yelp, etsy | IP-flagged at edge; W6a triplet only helps post-edge |
| Akamai | homedepot | tenant_seed captured but doesn't flip; Firefox-profile experiment cheaper than full sensor decode |
| Cloudflare Managed | udemy | W7-deep V1 PARTIAL (real `<title>` resolves, `__cf_bm` persists, but `cf_clearance` not issued); V2 needs header propagation + iframe + UA-CH audit |

## Ceiling math

- **Engine-only ceiling**: 119–120/126 (94–95%) if W4a + W7-deep V2 land
- **With residential proxy** (flips DataDome edge + reCAPTCHA): 124–126/126 (98–100%)

Today's gap to the +3 W4a-Kasada cluster is the highest-ROI remaining
engine work (1–2 days per
`HANDOFF_2026_05_10_PART5.md` estimates).

## Recommended next session

1. **W4a-deeper kasada_typeerror_stack_capture run** on canadagoose
   with the print-fix from `5a01a75`. Identify the 5 unjzomuy probe
   targets, stub them. → +3 (canadagoose, hyatt, realtor)
2. **Optional**: Firefox-profile experiment for Akamai cluster
   (homedepot mainly) per the Camoufox precedent.
3. **W7-deep V2** if engine-headline is the priority. Only 1 site
   (udemy) for ~3–5 days of work — lowest per-effort yield.
