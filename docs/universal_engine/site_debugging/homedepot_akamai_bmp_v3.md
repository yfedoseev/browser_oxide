# homedepot.com — Akamai Bot Manager v3

**Status**: BLOCKED. Same engine and pattern as adidas, slightly less
strict.

**Engine**: Akamai BMP v3.

**Baseline response**: HTTP 200, body 2621 bytes, contains a
`<div id="sec-if-cpt-container">` similar to adidas.

## Notable observation: one spurious PASS

In the first session run of `blocker_rigorous_probe.rs`, homedepot
returned `solver=PASS (973378b)` — about 1 MB of real page content with
product listings, navigation menus, etc. This was not reproducible in
two subsequent runs. Both later runs returned `solver=INTR (2694b)`.

**Interpretation**: Akamai's verdict for homedepot is **stochastic near
the borderline**. Whatever combination of (TLS fingerprint, headers,
sensor POST content, behavioral events) we're providing is sometimes
just barely enough to pass, but mostly not. This is different from
adidas where the verdict is deterministically reject across all runs.

This is actionable: it tells us we're CLOSE on homedepot. If you can
identify which specific session-state difference between the lucky run
and the failing runs caused the verdict flip, you've found the
load-bearing fingerprint or behavioral signal.

## What's the same as adidas

Same Akamai BMP v3 engine. Same `sec-if-cpt-container` interstitial
pattern. Same `_abck` trust slot mechanic. Same `/_bm/get_params?type=
get-akid` flow. Same general sensor VM structure (though the per-site
configuration may differ).

For a deep dive on the engine, see `adidas_akamai_bmp_v3.md`. Most of
that file applies here too.

## What's different from adidas

- **Looser configuration.** The 2621-byte interstitial is slightly
  larger than adidas's 2351 bytes — homedepot's interstitial may
  include more behavioral collection. The verdict is also less strict
  (one stochastic pass observed).
- **Different sensor VM URL pattern.** homedepot uses URLs like
  `/_bm/sensor/...` rather than adidas's `/9qizx734Mu_fe/...`. Not
  meaningful but worth noting if you're searching logs.

## What we've tried (mostly inherited from adidas work)

Everything we tried for adidas applies. T1.3 audio, T1.5 workers,
OffscreenCanvas, navigator class prototypes, humanize — all ran
against homedepot too via the same code paths and produced similar
INTR results.

## What to try next

Same as adidas + one extra:

1. **Capture both adidas AND homedepot sensor POSTs in the same
   session via Playwright** with the clean IP. Diff against ours.
   The "homedepot is stochastically passable" observation suggests
   our gap is smaller for homedepot, so the diff might be more
   tractable.

2. **Specifically investigate the 1-MB lucky run.** The probe
   captured the 973378-byte body. If you can find that capture in
   `/tmp/oxide-sensor-human/` or recreate the conditions (same
   profile, same time of day, same IP state), you can examine what
   was actually returned and reason backward to what session state
   produced it.

## Reproducibility

```bash
cargo test -p browser --test blocker_rigorous_probe tier05_blockers_all \
  -- --ignored --test-threads=1 --nocapture 2>&1 | grep homedepot
```

Run 5+ times. Expect 4 INTR + 1 occasional PASS (or 5 INTR — the
stochastic pass is rare).
