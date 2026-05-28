# 11 — Beyond FIX-J: per-region IP-clustering + behavioural for the AWS WAF cluster

**Status:** 🔵 partially shipped (FIX-J unblocks the bailout chain). Final-mile work for the remaining 5-7 stub sites.
**Sites in scope:** amazon-com, imdb, amazon-fr, amazon-in, amazon-de, amazon-jp (and any other AWS WAF sites the next gate identifies).
**Effort:** 1-2 weeks.
**Scope:** public engine + behavioural signals.

## TL;DR

FIX-J (commit `ef4f561`) fixed the `FileReader.readAsDataURL` no-op
stub that AWS WAF challenge.js needs to base64-encode its encrypted
fingerprint payload. After FIX-J, sites flip in single-trial sweeps
(amazon-ca → 227 KB, amazon-com-au → 917 KB / 988 KB) but DIFFERENT
sites flip on different trials — never all at once. Pattern is
consistent with "bailout chain unblocked, downstream IP-clustering /
per-region WAF rate-limit checks still apply". To get from "1/8 per
trial" to "consistent 8/8", we need to address the downstream layer
that varies.

## Why this matters

- AWS WAF is the highest-leverage vendor cluster — 7 of the 11 v150-
  vs-BO target sites are AWS-WAF-protected (per HANDOFF §1.1).
- The current per-trial 1/8 pattern means BO can ALREADY pass these
  sites — just not consistently on the same trial. Closing this gap
  IS the v0.2.0 ship-blocker for routed-median ≥ 115.
- The same fix likely improves booking.com (also AWS WAF self-hosted
  per audit/16 §R-SPA-BOOKING-FETCH-CHAIN reclassification).

## Current state (what we know)

Validation evidence:
- **Pre-FIX-J**: 0/8 AWS WAF sites flipped across 3 separate trials.
- **Post-FIX-J trial 1**: amazon-ca flipped (227 KB).
- **Post-FIX-J trial 2**: amazon-com-au flipped (917 KB).
- **Post-FIX-J trial 5 (sampler off, control)**: amazon-com-au flipped
  again (988 KB).
- Different sites per trial. Pattern: ONE site at a time gets past
  AWS WAF.

Plausible hypotheses (ranked by likelihood):

1. **Per-region IP-clustering** — AWS WAF deploys per-AWS-region
   (us-west-2, eu-west-1, ap-northeast-1, etc.). Each region has its
   own classifier state. BO's datacenter IP gets flagged by ONE
   region's classifier per trial; other regions haven't seen us yet.
   Different amazon TLDs (amazon-fr → eu-west-1, amazon-jp → ap-...)
   route to different regions.

2. **Behavioural-signal threshold** — challenge.js may require N
   mouse/scroll events within M ms; humanize.js fires some but maybe
   not enough variety. Per-region differences in the threshold would
   give the observed pattern.

3. **Cross-fingerprint clustering within a session** — even with FIX-J
   working, AWS WAF may track "this IP has issued tokens before;
   here's another challenge to verify". The cookie state from
   previous successful flips poisons subsequent attempts.

## Next steps

### Track 1 — Diagnose with the existing oracle (~2-3 days)

Extend `awswaf_probe.rs` to:
- Inject the probe instrumentation in LIVE flow (not just captured
  HTML replay) — modify `Page::navigate_with_init` to add the probe
  script before the navigation runs.
- Capture the FULL access trace for a live amazon-fr (or any
  consistently-failing site).
- Identify what challenge.js does AFTER our captured trace showed
  it got to the `forceRefreshToken-rejected` step in offline replay.
  In live operation with FIX-J working, what happens past that point?

### Track 2 — Behavioural enrichment (~3-5 days)

If Track 1 reveals AWS WAF wants more behavioural events:
- Extend `humanize.js`'s historical-mouse-trace pre-population to
  cover MORE events (current: ~14 historical points; try 50+).
- Add per-element event dispatch (real users move cursor toward
  CTA buttons; humanize fires generic coordinates).
- Add scroll patterns that vary with viewport (current: 2 fixed
  scroll steps; real users scroll based on content height).

### Track 3 — Per-IP cookie hygiene (~1-2 days)

If Track 1 reveals cookie state from prior visits is poisoning:
- Add an `aws-waf-token` cookie scrubber that clears any
  `aws-waf-token=*` cookie before a fresh nav to an AWS WAF site
  (similar to FIX-J + R-SHAREDSESSION work shape).
- Wire it into the navigate loop with the existing
  `is_datadome_challenge`-style helper pattern.

### Track 4 — Per-region BO header tuning (~3-5 days)

If Track 1 reveals per-region differences in expected headers:
- Different `sec-ch-ua-platform-version` granularity per region
  (some regions accept `15.2.0`, others want `15.2`).
- Different `accept-language` weighting per region (amazon-fr
  expects `fr-FR,fr;q=0.9,en-US;q=0.8,en;q=0.7`, BO sends
  `en-US,en;q=0.9` — that's almost certainly wrong for amazon-fr).

**Quick win in Track 4**: profile-aware `accept-language` per amazon
TLD. amazon-fr should send French; amazon-de German; amazon-jp
Japanese. Either change the profile per-site (hairy) or have the
HTTP layer inject the right `accept-language` based on the URL's
TLD (cleaner).

## Validation

For each track:
- Run a focused 8-site AWS WAF sweep 3 times.
- Compare "sites flipped per trial" before/after — expect lift from
  "1/8 per trial" toward "N/8 per trial".
- Run the full 4-profile 3-run gate (~8 hours) to confirm no
  regression elsewhere.

## Dependencies

- The awswaf_probe oracle (already shipped, commit `ef4f561`).
- A captured live-flow trace (needs the LIVE-flow probe instrumentation
  hook, Track 1).

## Sources / references

- `crates/js_runtime/src/js/shared_apis_bootstrap.js:545-625` —
  FileReader (FIX-J)
- `crates/browser/examples/awswaf_probe.rs` — the offline-replay oracle
- `crates/browser/src/js/humanize.js` — the behavioural primitive
- `crates/net/src/headers.rs:330` — accept-language construction
- audit `16_DECISION_LOG.md` §FIX-J / §FIX-J round 2 — the per-trial flip pattern
- audit `15_FIX_PRIORITY_RANKED.md` row 4★ — FIX-J entry
