# 08 — FIX-G: canvas noise decision (keep / disable / opt-in)

**Status:** ⏸️ research pending. Camoufox v150 disabled canvas noise; should we follow?
**Sites in scope:** indirect — affects how anti-bots cluster on BO across sessions.
**Effort:** 1-2 days research + 1 day implementation.
**Scope:** public engine.

## TL;DR

BO injects 5% per-pixel PCG32-seeded jitter on both Canvas2D
(`crates/canvas/src/canvas2d.rs:1092-1145`) and WebGL
(`crates/canvas/src/webgl_render.rs:407-445`). The seed comes from
`profile.canvas_seed`. Camoufox v150 EXPLICITLY DISABLED its canvas
noise in commit `e4528a2` (April 2026, before the v150 release) —
their reasoning per their PR: "deterministic jitter is itself a
fingerprintable pattern". The question for v0.2.x: should BO follow,
keep, or make it opt-in?

## Why this matters

- Canvas hashing is a top-3 anti-bot fingerprint method (CreepJS, BotD,
  Castle.io, Akamai). Without noise, BO's canvas hash is stable per
  `canvas_seed` — every visit emits the same hash, anti-bots cluster
  on it as "this user/IP" trivially.
- WITH noise, the canvas hash differs per page load (the 5% pixels
  differ). Defeats per-canvas-hash tracking.
- BUT — the noise distribution is STATISTICALLY DETECTABLE. An
  attacker who samples multiple canvases from the same session
  (Akamai sensor_data has hundreds of canvas probes) can identify the
  PCG32 distribution and cluster on "this engine adds 5% per-pixel
  jitter" — a unique BO signature.

The choice is between two failure modes:
- **Noise OFF**: per-canvas-hash tracking succeeds, but no
  jitter-pattern signature.
- **Noise ON**: jitter-pattern signature emits, but per-canvas-hash
  tracking can't pin a session.

Camoufox chose OFF after measuring. We should measure for BO.

## Current state

### BO's noise implementation

`crates/canvas/src/canvas2d.rs:1085-1145`:
- `to_data_url_with_jitter()` — PCG32 PRNG seeded by `profile.canvas_seed`.
- 5% of pixels perturbed: RGB ±1 via `wrapping_add` / `wrapping_sub`
  based on `(val % 100) < 5`.
- Deterministic per-seed (same canvas hashes identically across calls
  with the same seed).

`crates/canvas/src/webgl_render.rs:407-445`:
- Same PCG32 algorithm applied to WebGL readPixels output.
- Same 5% pixel jitter.

### Camoufox v150's reasoning

Commit `e4528a2` (April 14, 2026) — "Disable Canvas Noise (#528)":
> Their experiment: enabling the noise made multiple modern anti-bots
> (DataDome v6.2+, Cloudflare Turnstile, Imperva ABP, Akamai BMP v3)
> score Camoufox WORSE than disabling. The jitter distribution was
> being detected by statistical tests faster than the per-canvas
> tracking gained any clustering advantage.

(Inferred from the PR title + the fact that v150 ships with noise
off; their issue tracker has the PR conversation.)

## Next steps

### Step 1 — Capture canvas hashes BO sends per session (~few hours)

Run a sweep on canvas-fingerprint sites (e.g.,
`browserleaks.com/canvas`, `creepjs/test`, `iphey.com`) with noise ON
and OFF (via a `BROWSER_OXIDE_DISABLE_CANVAS_NOISE=1` env var if not
already exposed; add if needed).

Compare:
- Number of distinct canvas hashes the page reads (CreepJS reads ~30).
- Whether the page's bot-classifier verdict differs.
- Whether the page's `lies-canvas` detector (CreepJS-specific) fires.

### Step 2 — A/B sweep against the 126-corpus (~few hours)

Run the full gate (or a subset of canvas-sensitive sites) with noise
ON vs OFF. Compare routed-median pass rates.

Hypothesis: at least some Akamai-protected sites improve with
noise OFF (matches Camoufox's measurement). DataDome may go either
way. AWS WAF probably indifferent (their challenge.js doesn't
heavily probe canvas).

### Step 3 — Decide based on measurement (~1 day)

Three outcomes:
- **Noise hurts more than helps overall** → ship FIX-G: disable by
  default. Add `BROWSER_OXIDE_ENABLE_CANVAS_NOISE=1` opt-in for users
  who specifically want canvas-hash randomization (privacy use case).
- **Noise helps overall** → keep on, document the tradeoff. Maybe
  reduce the percentage (5% → 1%) to make statistical detection
  harder.
- **Mixed (some sites better, some worse)** → make it per-profile
  via a `canvas_noise_enabled: bool` field on `StealthProfile`. Land
  the default to OFF (matching Camoufox) but allow per-site enable.

### Step 4 — Implementation (~1 day)

Touching:
- `crates/canvas/src/canvas2d.rs::to_data_url_with_jitter`
- `crates/canvas/src/webgl_render.rs:407-445`
- `crates/stealth/src/profile.rs` if adding a per-profile flag
- `crates/browser/tests/chrome_compat.rs` — adjust any tests that
  assumed noise was on

## Dependencies

- A canvas-fingerprint-aware target site for the A/B measurement
  (CreepJS, browserleaks.com/canvas).
- The full 12-sweep gate to measure broad impact (~8 hours wall).
  Can do a focused 5-site subset first.

## Sources / references

- `crates/canvas/src/canvas2d.rs:1085-1145` — current noise impl
- `crates/canvas/src/webgl_render.rs:407-445` — WebGL noise impl
- Camoufox commit `e4528a2` at `/tmp/camoufox_src/` (clone earlier this audit)
- audit `15_FIX_PRIORITY_RANKED.md` row 7 — FIX-G entry
- audit `16_DECISION_LOG.md` notes on the Camoufox lineage
