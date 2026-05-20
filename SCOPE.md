# Scope and intended use

`browser_oxide` is a research-grade browser engine written from scratch
in Rust. This document spells out what it's *for*, what it *isn't for*,
and the values that drive triage decisions on this project.

It is short on purpose. If your situation isn't covered, open a
discussion and we'll talk.

## What this project is for

The engine is intended to support, but is not limited to:

- **Archival** — preserving the live web (link rot, dead sites, ML
  training corpus snapshots, regulatory compliance archives) without
  the resource footprint of headful Chrome.
- **Accessibility** — re-rendering uncooperative sites for users on
  screen readers, low-bandwidth links, or assistive tech that real
  browsers handle badly.
- **AI agents** — programmatic browsing for retrieval, summarisation,
  research assistants, and workflow automation, where running a full
  Chrome per agent is wasteful.
- **Security research** — academic and industrial study of web platform
  behaviour, fingerprint surfaces, anti-automation systems, and the
  interactions between them. CTF challenges fall under this.
- **Defensive testing** — your team auditing your own site's bot
  defences against a known-from-scratch engine, in an authorised
  pentest engagement.

## What this project is not for

- Circumventing access controls on sites you are not authorised to
  access.
- Bulk credential stuffing, account takeover, or other attacks against
  authentication systems.
- Bypassing paywalls or rate limits to extract content commercially
  against the operator's wishes.
- Mass scraping in violation of a site's `robots.txt`, terms of
  service, or applicable law.
- Building products whose primary value proposition is "defeat anti-
  bot vendor X." The engine ships APIs; what you do with them is
  your responsibility and we will not accept PRs whose purpose is to
  ship site-specific exploit code.

If you are unsure whether your use is in scope, the test we apply is:
*would the operator of the target site reasonably consent if you
asked them?* If yes, you're fine. If no, this is the wrong tool.

## Values that drive triage

- **Engine work first.** Fixes that make the engine match documented
  browser behaviour (Chrome, Firefox, Safari) ship. Fixes whose only
  motivation is a single anti-bot vendor heuristic do not.
- **Honesty over marketing.** Measured numbers, named residuals, and
  explicit caveats live in the README. Aspirational marketing does
  not.
- **Permissive license, no copyleft transitive deps.** Dual MIT /
  Apache-2.0; we do not pull in MPL or AGPL crates. Drop-in for
  proprietary downstream is intentional.
- **Reproducible measurement.** Every claim in `README.md` traces to
  a test in `crates/browser/tests/` or a benchmark in `benches/`. If
  it doesn't, it doesn't go in the README.

## Reporting misuse

If you observe a deployment of `browser_oxide` that's clearly outside
this scope (e.g., a service marketed as a turn-key "bypass Cloudflare"
product, or a public scraper aggressively abusing a site), please tell
the maintainer privately. We can't police every downstream, but we
will publicly disassociate from clear violations.

## Legal

This document is not a license. The license is in `LICENSE-MIT` and
`LICENSE-APACHE`. Nothing here grants permission to do anything that
the law of your jurisdiction prohibits.
