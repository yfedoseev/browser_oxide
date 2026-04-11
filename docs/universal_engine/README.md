# Universal Engine — Handover Documentation

This directory is a handover package for new developers continuing the work on
**browser_oxide**, a from-scratch Rust headless browser (V8 via deno_core 0.311)
targeting "the most advanced stealth browser in Rust that passes every major
anti-bot engine in 2026."

Read the files in order. Each answers one question a new contributor will ask.

| # | File | Question it answers |
|---|---|---|
| 01 | `01_architecture_principle.md` | What is the core architectural principle? Why "zero per-engine runtime logic"? |
| 02 | `02_current_state.md` | **HISTORICAL (2026-04-10 baseline).** Kept for diff value; superseded by `09`. |
| 03 | `03_research_landscape.md` | What did other open-source stealth browsers do? What approaches exist? |
| 04 | `04_refactor_plan.md` | What's the concrete refactor to reach the zero-per-engine goal? |
| 05 | `05_capability_gaps.md` | What capability work is still needed (T1.1-T1.5)? |
| 06 | `06_test_infrastructure.md` | How do I run the probes, debug tools, and regression tests? |
| 07 | `07_blink_source_pointers.md` | Where do I find Blink's implementation of X for comparison? |
| 08 | `08_links_bibliography.md` | Every URL, repo, paper, and reference I used. |
| **09** | **`09_session_2026_04_11_state.md`** | **CURRENT STATE.** What shipped through Sprint 0-3, what the probe revealed, what's still blocked. Start here. |
| — | `site_debugging/` | Per-site deep-dives for the 8 blocker sites. |
| — | `TODO.md` | Master sprint status + next steps. |

## Quick-start checklist for new contributors

1. Read `01_architecture_principle.md` first. The "zero per-engine logic" rule
   is load-bearing for every decision. Do not violate it without discussion.
2. Read `09_session_2026_04_11_state.md` — **this is the current state**,
   not `02`. What shipped through Sprint 0-3, the probe results, what's
   still blocked, and the concrete next step (Chrome wire capture).
3. Check `TODO.md` for the sprint status table and the "What's actually
   blocking us now" section.
4. Run `cargo test --workspace -- --test-threads=1` to confirm the
   workspace is green on your machine (expected: 1005 passing).
5. Run the blocker probe and the TLS fingerprint probe:
   ```
   cargo test -p browser --test blocker_rigorous_probe tier05_blockers_all \
     -- --ignored --test-threads=1 --nocapture
   cargo test -p browser --test tls_fingerprint_probe probe_tls_peet_ws \
     -- --ignored --test-threads=1 --nocapture
   ```
   The second one is the diagnostic that exposes our wire fingerprint
   via tls.peet.ws — use its output to diff against real Chrome.
6. Pick a pending task from `TODO.md`. Task 80 (diagnose missing TLS
   extension) is the highest-signal item if you can run Chrome from
   the test machine and capture its ClientHello.

## The elevator pitch

browser_oxide is a ground-up reimplementation of a headless Chrome-compatible
browser in Rust. The theory: real Chrome passes every public anti-bot engine by
just navigating and running scripts — no "is this Kasada?" branches exist in
Chrome's code. If we implement enough Chrome-shaped capabilities (DOM, canvas,
audio, WebGL, fonts, workers, correct TLS), we inherit that generic bypass for
free. The advantage over modifying Chrome itself is that every API we ship is
a known-good value we control, so fingerprint mismatches from C++ can't leak
through.

As of the last session, browser_oxide passes 22/24 deep-path tests (including
JD, Taobao, Reddit, LinkedIn, Coinbase, ChatGPT, Delta, Walmart, Nike, etc.)
but fails the hardest tier-1 Akamai BMP v3 sites (adidas, homedepot) and the
hardest Kasada sites (canadagoose, hyatt). No open-source browser passes those
either — see `03_research_landscape.md`.

## Critical reading: the non-negotiables

- **Zero per-engine runtime logic.** The only per-engine code allowed is in
  tests and debug probes under `crates/browser/tests/`. No `if host.contains(
  "kasada")` anywhere in the actual browser crates.
- **BSD-3 / MIT / Apache-2.0 only.** No MPL, no LGPL, no GPL. Symphonia is
  specifically banned (pulls in MPL). Everything else in `Cargo.toml` is
  verified clean as of 2026-04.
- **Tests use `--test-threads=1`** because V8 isolates are per-thread. Never
  remove this.
- **Network tests are `#[ignore]`.** Running them requires internet and hits
  live sites; they're not part of CI.

## When to ask for help

If you're stuck for more than 2 hours on one of the tier-1 blockers (adidas,
canadagoose, homedepot, hyatt) without measurable progress, STOP and read
`09_site_debugging/` for what's already been tried. These sites are at the
open-source frontier — no project is publicly known to pass them without
either a commercial remote solver (Hyper-Solutions, RiskByPass) or private
proprietary code. Don't grind on a problem that a $500/month SaaS solves
because there's a reason nobody has open-sourced it.
