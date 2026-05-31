# #49 Render-loop efficiency — NEGATIVE RESULT (refed MessagePort abandoned)

**Date:** 2026-05-30
**Branch:** fix/v0.1.0-fix4-canvas-parity
**Status:** Hypothesis FALSIFIED by measurement. Change reverted. Diff archived
at `09_ABANDONED_refed_messageport_diff.patch`.

## Hypothesis (from #49)

The committed MessagePort `_deliver` delivers React-18's concurrent-scheduler
macrotasks via `__bgSetTimeout` (which *unrefs* the `op_timer_sleep`). The theory
was that an unref'd 0 ms timer lets `run_event_loop` report `AllWorkDone` while
React's next scheduler chunk is still pending, so `run_until_idle` exits after one
chunk and heavy renders need dozens of page.rs drains → slow/flaky (adidas
1.45 MB once-in-N, duolingo stuck at 13 KB). Fix: deliver on a *refed*
`setTimeout(0)` so the whole `performWorkUntilDeadline` chain drains in one
`run_event_loop` call.

Three changes were tried together:
1. `window_bootstrap.js` `_deliver`: `__bgSetTimeout` → refed `setTimeout`.
2. `page.rs` SPA-fast-exit: gated on `body_len >= 15_000` (don't bail below the
   pass threshold).
3. `page.rs` benign-return: a bounded "thin-render rescue" continued-drain for
   sub-15 KB benign pages (up to 3 no-growth stalls).

## Measurement (chrome_148_macos, pool, same IP)

Baseline = committed (unref) vs change (refed+floor+rescue):

| site     | baseline (unref) | change (refed) | verdict |
|----------|------------------|----------------|---------|
| duolingo | 13327 FAIL       | 13381 FAIL     | no change |
| reddit   | 8326 FAIL        | 8326 FAIL      | no change |
| hulu     | 1.39M PASS       | 1.39M PASS     | no change |
| netflix  | 511K PASS        | 610K PASS      | no change |
| **spotify** | **126K PASS** | **6349 FAIL**  | **REGRESSION** |
| vimeo    | 1.78M PASS       | 1.79M PASS     | no change |
| adidas   | ~2.4K (flaky)    | 2407 FAIL      | no flip |

Broad 22-site regression set with the change: 18/22, no timeouts/hangs — but
spotify is a clean regression vs baseline, and nothing flipped.

## Findings

1. **The committed unref delivery already renders the whole SPA cluster.**
   hulu/netflix/vimeo/instagram/macys/wayfair/cloudflare/github all render full
   and FAST (1–3 s) on baseline. The "slow/flaky" premise did not reproduce as a
   *loop-efficiency* problem — the loop is already efficient.

2. **Refed delivery REGRESSES spotify** (126 KB → 6 KB). spotify's full render
   depends on the unref delivery timing; forcing the refed single-drain path
   truncates it. Net effect of the change across the tested set: −1 (spotify),
   0 flips. Strictly worse.

3. **adidas / duolingo / reddit are render-COMPLETENESS failures, not
   loop-efficiency.** They mount a shell and reach `AllWorkDone` genuinely idle
   at iter 0 — the bounded rescue continued-drain produced ZERO body growth
   across its stall window. Nothing is queued behind a timer to drain; the app
   simply stops rendering. This is #48 territory (a missing API / event /
   IntersectionObserver-driven lazy load), not the event loop.

## Decision

Reverted all three changes to committed baseline. #49 closed as a falsified
hypothesis: the render loop is already efficient for SPAs; the thin-render
cluster (duolingo/reddit/adidas) is a completeness problem to attack via #48's
angle (what API/event causes React to stop after the shell), NOT loop pacing.

## Spin-off lead for the completeness work

spotify renders 126 KB but duolingo stops at 13 KB and reddit at 8 KB, all React
SPAs on the same engine. The discriminator between "renders" and "mounts-then-
stops" is the real thin-render lever. Next completeness investigation should
diff what spotify's bundle reaches that duolingo's/reddit's does not (which
event/API the stalled apps are awaiting).

## Incidental observation

homedepot rendered **987 KB (PASS)** in the 22-site regression run — consistent
with the earlier sec-cpt budget/`is_seccpt_solved` work. Re-confirm in the final
gate; #47 may already be satisfied.
