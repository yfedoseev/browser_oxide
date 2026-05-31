# Warm/PagePool path under-counts challenge sites (measurement bug + fix)

**Date:** 2026-05-30
**Branch:** fix/v0.1.0-fix4-canvas-parity

## Discovery

While diagnosing the "thin-render cluster" (reddit/duolingo/adidas), the
`thin_probe` example (cold `Page::navigate`) and the pooled sweep
(`BROWSER_OXIDE_SWEEP_POOL=1`, `PagePool::navigate` → `navigate_warm`) disagreed
sharply for reddit:

| path | reddit len | ms  |
|------|-----------|-----|
| cold `Page::navigate` | **676 140 (PASS)** | 1616 |
| warm `PagePool` (gate) | 8 367 (FAIL) | 137 |

The warm fetch returned `html_len=8424`, `scripts_total=1`, `external=0` — i.e.
reddit served a **different page** to the warm path. Dumping it:

```
<title>Reddit - Please wait for verification</title>
<script nonce=...>document.addEventListener("DOMContentLoaded",...e.elements
.namedItem("solution").value=n,e.requestSubmit()...)</script>
```

reddit served an **inline-script form-submit challenge** ("please wait for
verification"). The cold path's iteration loop follows the `requestSubmit()`
pending-navigation and re-fetches the real 676 KB app. The **warm path
deliberately skips that loop** (documented: *"warm reuse is for benign content
extraction… the pending-nav iteration loop is NOT run… Caller should fall back
to `Page::navigate` for challenge-protected origins"*), so it returns the
unsolved 8 KB shell.

## Why it matters

`sweep_metrics` (and `benchmarks/run_bo_isolated.py`) run **pooled** when
`BROWSER_OXIDE_SWEEP_POOL=1`, and took the warm result verbatim — they never did
the documented cold fallback. So **every challenge / JS-interstitial site was
silently under-counted in the pooled gate**, depressing BO's measured pass-rate
vs competitors. The honest engine capability is the cold path.

## Fix

Two changes:

1. **`PagePool::navigate` cold-fallback guard** (`pool.rs`). After
   `navigate_warm`, if the result is sub-threshold (`engine_classify().len <
   15_000`) OR `is_anti_bot_challenge()`, release the warm page and return the
   authoritative cold `Page::navigate(url, profile, 3)`. Benign pages (the warm
   fast-path's purpose) clear the threshold and keep the warm result with zero
   extra cost; only thin/challenge results — already failing — pay the cold
   re-nav.

2. **Warm-path ES-module routing** (`page.rs::navigate_warm`). The warm script
   loop ran *every* script via classic `execute_script`, so `<script
   type="module">` threw `Cannot use import statement outside a module` and the
   bundle was dropped — the warm analog of the cold-path #40 bug. Now mirrors the
   cold path: `is_module` scripts go through `eval_module_code` (bounded 10 s).

## Result

| site | warm before | warm after |
|------|-------------|-----------|
| **reddit** | 8 367 FAIL | **654 574 / 659 287 PASS** (cold fallback) |
| benign SPAs (medium/github/stackoverflow/google/instagram/linkedin/ebay) | PASS fast | PASS fast (no fallback, 1–5 s) |
| bestbuy / etsy (real challenges) | thin | thin (fallback ran, correctly still hard) |
| spotify | flaky thin | thin (genuinely thin cold too) |

`thin_probe` added as a reusable single-page render diagnostic
(`cargo run --release -p browser --example thin_probe -- <url> [profile]`):
dumps tag/len, mount child-counts, `__scriptErrors`, readyState, body tail.

## Caveat for the final gate (#46)

The cold fallback makes the pooled gate slower for challenge-heavy corpora (each
thin warm result triggers a 30–115 s cold re-nav). That is the correct trade —
correctness over throughput — but the full 126 pooled gate will run notably
longer than the old (wrong) fast number. Budget for it.

## Implication for the SOTA comparison

Any prior BO gate number measured **pooled** under-counted challenge sites and
must be re-read as a floor, not the true engine capability. The canonical
SOTA-vs-v150 comparison must use the cold path (or this fixed pool path). reddit
alone is +1; re-run the full gate to find the rest.
