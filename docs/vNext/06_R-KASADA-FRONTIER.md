# 06 — R-KASADA-FRONTIER: canadagoose / hyatt / realtor — `vendor_solvers` scope

**Status:** ⏸️ deferred indefinitely. Out of public-engine scope per CLAUDE.md.
**Sites in scope:** canadagoose, hyatt, realtor (3).
**Effort:** months of research per the existing memory lineage.
**Scope:** `vendor_solvers` private companion crate.

## TL;DR

Three Kasada-protected sites that NO Chromium-class engine passes
today (BO + Camoufox v150 + Patchright all fail). The prior research
session (May 2026) CLOSED the realm/sentinel/identity hunt as
not-the-bug — all 4 global paths in BO's Kasada-relevant V8 are
byte-identical to Chrome invariants. The residual is "holistic ML
tail, no single lever". Per CLAUDE.md, per-vendor solving lives in
the private `vendor_solvers` companion crate; the public engine has no
work to do on these sites.

## Why this matters

The honest case for keeping this in vNext at all:

- It's the **visible long-term ceiling** on BO's measured pass-rate.
  As long as these 3 sites fail, BO's routed-median has a hard upper
  bound below 124/126.
- v150 also fails — confirms it's not a fingerprint-surface fix.
  Closing them requires either:
  1. Full Kasada `/tl` interactive token computation in
     `vendor_solvers` (open-ended research — months).
  2. Cross-engine fingerprint corpus + classifier training to
     predict which corpus-of-fingerprints values Kasada's ML accepts
     (research-project scale).
- For v0.2.0 / v0.3.0 timeframe: **no public-engine work**. Tracker
  doc only.

## Current state (from prior research)

Per `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/`:

- `state_2026_05_16_phase0_rebaseline.md` §Phase 2 Outcome A:
  "Kasada realm/sentinel/identity line CLOSED as not-the-bug" — all 4
  global paths identical = Chrome invariant. `everTaggedId: -1` in all
  measurements (sentinel chain matches Chrome).
- `state_2026_05_17_unblock_execution.md` — K2-DIFF scoped: capture
  our `/tl` POST body + field-diff vs real Chrome. That work is
  in-flight in the prior research; captures at `/tmp/k2_diff/` (if
  the box survived).
- `state_2026_05_15_session_synthesis.md` — Kasada ceiling synthesis.

The residual after Phase 2: **holistic ML classifier over many low-
signal features**. No single fingerprint surface lever moves these
three sites. v150's hardware-spoofing additions (which moved AWS WAF)
didn't move Kasada either — confirms the ML-classifier nature.

Patchright on hyatt gets 13228 bytes (loose L3, sub-15 KB) — partial
progress, suggests hyatt's Kasada threshold is the lowest of the
three. canadagoose + realtor are full Kasada-CHL across every engine.

## Next steps

### For the public engine: NONE.

Public-engine fingerprint surface work (the focus of R-FP-AUDIT-2026Q3
and this `docs/vNext/`) does not affect these sites per the closed
hunt. Don't reintroduce vendor bypass code into public crates per
CLAUDE.md.

### For `vendor_solvers` (private repo):

The right next step (if/when prioritized) is to pick up the K2-DIFF
work. That's documented in the memory above. **Do not duplicate that
work in the public repo.**

## Out-of-scope changes that WOULD help (anti-recommendations)

Listing these so they don't get attempted:

- ❌ Reverting the Phase-2 changes that closed the realm hunt
  (already validated as not-the-bug).
- ❌ Adding more aggressive humanize.js patterns specifically for
  Kasada (the classifier doesn't gate on single behavioural events).
- ❌ Per-site engine forks (anti-pattern; breaks SCOPE.md).
- ❌ Importing `vendor_solvers` code into the public engine.

## Sources / references

- `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/state_2026_05_16_phase0_rebaseline.md`
- `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/state_2026_05_17_unblock_execution.md`
- `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/state_2026_05_15_session_synthesis.md`
- `docs/releases/v0.1.0-parity/08_KASADA_FRONTIER.md`
- `CLAUDE.md` "Per-vendor challenge solving is out of scope here"
- `SCOPE.md` "PRs that reintroduce site-specific bypass code into this repo will be declined"
- audit `16_DECISION_LOG.md` §R-KASADA-FRONTIER — this session's classification
