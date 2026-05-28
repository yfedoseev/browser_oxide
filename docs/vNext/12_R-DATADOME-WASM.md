# 12 — R-DATADOME-WASM: full WASM daily-key solver (vendor_solvers scope)

**Status:** ⏸️ deferred to `vendor_solvers`. Public-engine primitives shipped this session.
**Sites in scope:** etsy, yelp (and any other DataDome-WASM-protected sites).
**Effort:** 1-2 weeks for the WASM solver.
**Scope:** `vendor_solvers` per CLAUDE.md.

## TL;DR

FIX-DD (commit `78a1241`) shipped the 3 public-engine primitives that
let the DataDome bundle's OWN self-solve flow run: CSP relaxation,
iframe materialization, and cookie-watch break-on-solve. With those,
the engine "doesn't interfere". The remaining gap — the actual
**daily-key WASM computation** that produces the `datadome=` token —
is per-vendor solver code that belongs in `vendor_solvers` per
CLAUDE.md. This doc tracks the boundary + what's needed there.

## Why this matters

- etsy + yelp are DataDome-WASM-protected and represent 1-2 of the
  remaining failing sites in the corpus.
- v150 + Patchright also fail these (the WASM solver is hard); not a
  clear lift over competitors but closes a known gap.
- The public-engine primitives are now in place — the WASM solver
  work is unblocked architecturally.

## Current state

### Shipped this session (public engine):

`crates/browser/src/page.rs` (commit `78a1241`):
- `is_datadome_challenge(html)` — detects DataDome interstitial by
  `captcha-delivery.com` substring + 50 KB size gate.
- `is_datadome_solved(cookies, body)` — recognizes `datadome=` cookie
  + clean body transition.
- Wired at 3 navigate-loop gate points (CSP relax, iframe
  materialization activation, cookie-watch break-on-solve) — fires
  on ANY DataDome interstitial, not just navs where a registered
  DataDomeSolver claims the response.
- 4 unit tests verifying detection + size-gating + the
  cookie-AND-body solve semantics.

### NOT shipped (vendor_solvers scope):

- Actual WASM solver — the daily-key computation that takes
  `gokuProps.key + iv + context` (analogous structure) and emits the
  token AWS WAF / DataDome's `/verify` endpoint accepts.
- DataDome's iframe sub-realm gets the page-level engine primitives
  but no solver logic to actually compute the token.

## Architecture: how the boundary works

For DataDome-protected sites in BO:
1. Public engine fetches `etsy.com` → gets DataDome interstitial
   (response contains `captcha-delivery.com` script + `dd-script.js`).
2. `is_datadome_challenge` returns `true` (FIX-DD ✓).
3. Engine relaxes CSP (FIX-DD ✓).
4. Engine runs the dd-script in V8.
5. dd-script tries to load WASM from `captcha-delivery.com`.
6. Engine materializes the cross-origin iframe (FIX-DD ✓).
7. The iframe needs to execute WASM that computes the daily key →
   POST to `/verify` → response sets `datadome=` cookie.
8. **THIS STEP** is where the WASM solver in `vendor_solvers` does
   its work. Without it, the bundle bails partway.
9. If solver succeeds → cookie set → engine cookie-watch breaks (FIX-DD ✓)
   → engine cookie-diff retry re-fetches → site renders.
10. If solver doesn't run → bundle stalls → 90 s poll deadline →
    engine gives up → response is the interstitial body.

So the public engine handles steps 1-6 + 9; the WASM solver in
vendor_solvers handles steps 7-8.

## Next steps

### For the public engine: minor follow-ups

- The `is_datadome_challenge` detection might want to allow LARGER
  body sizes for DataDome's newer interstitials (some are 5-10 KB).
  Tune the 50 KB threshold if empirical data shows it misses some.
- The cookie-watch break should also handle DataDome's MULTI-COOKIE
  pattern (`datadome` + `_pxhd` + `_px3` — px* are PerimeterX which
  DataDome bundles use).

### For `vendor_solvers` (private repo): the solver itself

Out of scope for this repo. Reference materials:
- `docs/releases/v0.1.0-parity/07_DATADOME_PRIMITIVES.md` — DataDome
  primitive reference
- `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/state_2026_05_16_phase5_datadome.md` —
  prior-session DataDome ground truth

Implementation work would touch:
- `vendor_solvers/src/datadome/` — solver entry point
- WASM-loading + execution machinery (or interop with dd-script's
  embedded WASM if accessible)
- Per-day key derivation that matches DataDome's rotation schedule

## Validation

For each follow-up:
- `target/release/examples/sweep_metrics chrome_148_macos <(echo '[{"cat":"misc","name":"etsy","url":"https://www.etsy.com/"}]') /tmp/etsy_validation.json`
- Expected: L3-RENDERED > 15 KB.

For the WASM solver work in vendor_solvers: parallel sweep against
etsy + yelp + a known-clean DataDome site to confirm no regression.

## Dependencies

For the public engine follow-ups: none.
For the WASM solver: knowledge of the daily-key derivation; access to
captured `dd-script.js` + its embedded WASM.

## Sources / references

- `crates/browser/src/page.rs::is_datadome_challenge` +
  `is_datadome_solved` (commit `78a1241`)
- `docs/releases/v0.1.0-parity/07_DATADOME_PRIMITIVES.md` —
  DataDome primitives spec
- `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/state_2026_05_16_phase5_datadome.md`
  — prior research
- audit `16_DECISION_LOG.md` §R-DATADOME-DAILY-KEY (FIX-DD shipped) —
  what's done
