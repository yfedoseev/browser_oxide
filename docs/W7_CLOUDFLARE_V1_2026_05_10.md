# W7 — Cloudflare Bypass V1 (2026-05-10)

> Implementation companion to `docs/RESEARCH_CLOUDFLARE_BYPASS_2026_05_10.md`.
> Tracks what V1 ships and what V2 still needs.

## Scope of V1

V1 is **scaffolding + telemetry**, not a closed-loop solver. It mirrors
the V0 Kasada scaffold from before the PoW solver landed: detect, log,
let the orchestrator run, retry on cookie delta. Per the research doc
§7.4, this is the recommended first cut — the *correct* answer is to
run the orchestrator JS in our V8/DOM rather than re-implement the
ever-rotating PoW kernel in Rust.

### What ships

1. `crates/stealth/src/cloudflare.rs` (~330 LoC, 8 unit tests).
   - `CfChallengeKind` enum: `Managed | Jsch | NonInteractive | Interactive | Unknown`.
   - `CfChallengeContext` struct with `kind, ray, zone, csp_nonce, orchestrator_url, fa_url, mdrd, platform_variant, cf_mitigated_header`.
   - `detect_challenge(headers, body)` — fires on either canonical
     `cf-mitigated: challenge` header **or** body `_cf_chl_opt` /
     `/cdn-cgi/challenge-platform/` markers when `server: cloudflare`.
   - JS-blob field extractor that tolerates single/double quotes and
     keeps mid-identifier matches out (`_cf_chl_opt` only).
2. `crates/stealth/src/lib.rs` — `pub mod cloudflare`.
3. `crates/browser/src/page.rs`:
   - `Page::handle_cloudflare_flow(&client)` — runs after Akamai's
     `handle_akamai_flow`. Detects, logs `[cloudflare] cf=… ray=… zone=… variant=… orchestrator=… mitigated_hdr=…`,
     short-circuits on interactive Turnstile (V1 cannot solve), checks
     for an existing `cf_clearance`, injects 30 ticks of low-rate
     mousemove/scroll/keyup behavioural noise, then drives the event
     loop in 250 ms slices for up to 10 s polling for either
     `cf_clearance` in the cookie jar **or** an orchestrator-queued
     `__pendingNavigation`.
   - Wired into `navigate_loop_internal` directly after
     `handle_akamai_flow`. The existing cookie-delta retry path
     ([page.rs:1340–1550]) picks up `cf_clearance` and re-fetches
     automatically — no engine changes needed.
4. `crates/browser/tests/cloudflare_udemy.rs` — synthetic detector test
   (passes; no network) and `--ignored` end-to-end test against
   `https://www.udemy.com/`.

### Build status

- `cargo check -p stealth` — green.
- `cargo check -p browser` — green.
- `cargo test -p stealth --lib cloudflare` — 8 / 8 pass.
- `cargo test -p browser --test cloudflare_udemy detects_synthetic_managed_challenge` — pass.

### Live udemy result (2026-05-10 run)

Outcome: **partial — orchestrator detected, scaffolding fired across 3
iterations, but no `cf_clearance` was issued.**

Diagnostic gold from the run:

```
[cloudflare] cf=Unknown ray= zone= variant= orchestrator=/cdn-cgi/challenge-platform/scripts/jsd/main.js mitigated_hdr=false
[cloudflare] orchestrator did not produce cf_clearance within budget — V2 work needed
[udemy] title: "Udemy: Online Courses for Skills, Careers & AI"   ← real title (not "Just a moment...")
[udemy] body length: 476222
[udemy] still on challenge page: true                              ← /cdn-cgi/challenge-platform/ still in body
```

Two important signals:

- The page title is the **real udemy title**, not the challenge
  interstitial — meaning the orchestrator script ran far enough for the
  document.title to be set, but not so far that the redirect-to-origin
  fired and the challenge body got replaced.
- `__cf_bm` cookie is being set on the initial response and surviving
  iterations (visible in the request log), but `cf_clearance` is never
  added.

The detector reported `Unknown` kind on iter 1+ because by the time we
poll the DOM via `page.content()`, the orchestrator has already mutated
the inline blob away — only the `/cdn-cgi/challenge-platform/scripts/jsd/main.js`
URL is still detectable. This is cosmetic: the *initial* HTML body
parse correctly identified the Managed Challenge.

## What V2 needs

Ranked by likely impact on udemy specifically. None of these are
fingerprint-trivial; each one is a real engineering chunk.

1. **Capture the response headers at navigate-loop entry** so the
   detector receives the canonical `cf-mitigated: challenge` and
   `cf-ray` headers, not just the body. Today
   `navigate_loop_internal` only forwards `csp_headers`; pass through
   `cf-mitigated` + `cf-ray` + `server` so iter 0 logs the full
   diagnostic. (Mechanical, ~1 hr.)
2. **Audit why the orchestrator script is not reaching its clearance
   POST.** Concretely: enumerate every `fetch` / `XMLHttpRequest` the
   orchestrator issues in our V8, compare against a real Chrome
   capture against udemy, and identify the first missing call. Most
   likely culprits:
   - `Worker` / `SharedWorker` instantiation not actually running our
     event loop (`worker-src blob:` in udemy's CSP).
   - `MessageChannel` / `BroadcastChannel` semantics required by the
     iframe-bridge timing path (research §3.4 / §4 row 8).
   - `navigator.sendBeacon` being a no-op in our runtime.
3. **Iframe bridge.** udemy's CSP allows
   `frame-src 'self' https://challenges.cloudflare.com blob:`. The
   Managed Challenge often spawns a `challenges.cloudflare.com` iframe
   to host the Turnstile widget; if our iframe-loader doesn't actually
   navigate cross-origin and execute the inner script, the orchestrator
   never gets its postMessage handshake.
4. **Critical-CH retry was already shipped (commit 37e0c7c)**, but the
   audit per research §11 hypothesis 2 — verify
   `Sec-CH-UA-Full-Version-List` brand+version triples match
   `navigator.userAgentData.brands` exactly — has not been done. CF
   reportedly grades inconsistencies hard.
5. **TLS/H2 priority-frame audit** per research §11 hypothesis 1.
   Wireshark capture of our hello + initial frames against udemy,
   diffed against `docs/CHROME_147_TLS_REFERENCE_2026_04_29.json`. If
   we're sending a non-Chrome priority sequence, threat score escalates
   before any JS runs.
6. **Privacy Pass token presence is N/A for V1** (we are correctly a
   "no-token Linux Chrome"); no work needed here unless V3.
7. **Turnstile interactive solver** (`cType: 'interactive'`) — the
   `kind.v1_solvable()` gate already short-circuits this. V3 will need
   a pluggable captcha-service hook (CapSolver / 2Captcha). Not on the
   roadmap until a real customer hits it.

## Files touched (uncommitted)

| File | Change | Lines |
| --- | --- | --- |
| `crates/stealth/src/cloudflare.rs` | new module | +328 |
| `crates/stealth/src/lib.rs` | `pub mod cloudflare;` | +1 |
| `crates/browser/src/page.rs` | `handle_cloudflare_flow` + wire-in | +137 |
| `crates/browser/tests/cloudflare_udemy.rs` | new test file | +73 |
| `docs/W7_CLOUDFLARE_V1_2026_05_10.md` | this doc | new |

No other crates touched. Per CLAUDE.md: V8 single-thread requirement
preserved; no MPL deps added; the new module is pure Rust + serde +
the existing `sha2`/`hex` deps already in `stealth/Cargo.toml`.
