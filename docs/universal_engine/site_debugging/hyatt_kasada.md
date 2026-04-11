# hyatt.com — Kasada

**Status**: BLOCKED. Same engine and pattern as canadagoose.

**Engine**: Kasada KPSDK v3.

**Baseline response**: HTTP 429, body 686 bytes containing the Kasada
interstitial scaffold and ips.js script tag.

## What's the same as canadagoose

Same Kasada engine, same `/tl` POST flow, same `x-kpsdk-cr: true`
acceptance, same retry-still-blocked outcome. See
`canadagoose_kasada.md` for the full debugging writeup. Everything in
that file applies here.

## What's different

- Different `ips.js` URL pattern (Kasada generates per-customer paths).
- Slightly smaller interstitial body (686 vs 701 bytes).
- The Kasada token format / session-binding mechanics may differ
  per-customer; verify by capturing both sites' `/tl` responses if you
  have a clean IP.

## Hypothesis: same root cause as canadagoose

The blocker is almost certainly the same as canadagoose: the retry
isn't going through ips.js's patched `window.fetch` because we don't
have a real `location.reload()` mechanism. Fix one, fix both.

## What to try next

Same as canadagoose. The refactor in `04_refactor_plan.md` will likely
fix both Kasada sites simultaneously if the root cause hypothesis is
correct.

## Reproducibility

```bash
cargo test -p browser --test blocker_rigorous_probe tier05_blockers_all \
  -- --ignored --test-threads=1 --nocapture 2>&1 | grep -A 5 hyatt
```
