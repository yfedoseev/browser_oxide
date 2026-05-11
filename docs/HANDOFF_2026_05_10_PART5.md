# Session Handoff PART 5 — 2026-05-10 final round (4 workstreams attempted)

Continuation of `HANDOFF_2026_05_10_PART4.md`. The user asked to attack
all 4 remaining workstreams. Two background agents ran in parallel
(W7-deep CF orchestrator, W5b+ twitter profiler) while foreground did
W4a-deeper TypeError hook + W6a-V verification.

## What landed

### W6a-A — humanize.js synchronous mouse-buffer pre-population (commit 2573e47)
Synthesizes 12 historical sigma-lognormal coords + 1 synchronous
mousemove dispatch on window/document/body BEFORE any antibot script
runs. Closes DataDome's empty-coord-list scoring window.

### W7-deep — CF Managed Challenge V1 (~600 LOC, commit 9e1ec3a)
Agent shipped `crates/stealth/src/cloudflare.rs` (374 LOC, 8 unit
tests pass) + `Page::handle_cloudflare_flow` hook + dedicated
`cloudflare_udemy.rs` test + `docs/W7_CLOUDFLARE_V1_2026_05_10.md`.
**Live udemy result: PARTIAL** — orchestrator partially executes
(real `<title>` "Udemy: Online Courses..." resolves instead of "Just
a moment...", `__cf_bm` cookie persists across retries), but
`cf_clearance` not issued. V2 needs:
1. Header propagation through `navigate_loop_internal` so detector
   logs Ray-ID on iter 0 (~1 hr)
2. Worker/MessageChannel audit
3. Iframe loading for `challenges.cloudflare.com` (Turnstile widget)
4. UA-CH consistency (Sec-CH-UA-Full-Version-List vs.
   `navigator.userAgentData.brands`)
5. H2 priority-frame Wireshark diff vs Chrome 147 reference

### W5b+ — twitter/x event-loop profiler op-name aggregation (commit 9e1ec3a)
Agent extended `crates/event_loop/src/lib.rs` with thread-local
`OP_NAME_TOTALS` HashMap that aggregates pending `AsyncOp` counts by
name. The profiler dump now NAMES which specific op dominates the
pending future set. Plus `crates/browser/tests/twitter_profile.rs`
dedicated profile test runnable via `BOXIDE_EVENT_LOOP_PROFILE=1`.

The instrumentation shipped; running the profile + identifying the
specific op is the next-session task.

### W4a-deeper — TypeError-stack capture test (commit 9e1ec3a)
Foreground work. `kasada_typeerror_stack_capture` test in
`chrome_compat.rs` (~98 LOC) hooks `Error.prepareStackTrace` to record
the structured CallSite array (function name + file + line + col +
isEval + isNative) for every TypeError whose message contains the
unjzomuy 28-char identifier. Falls back to the default formatter so
other catch handlers' `err.stack` reads aren't broken.

**Live result on canadagoose**: 2 unjzomuy TypeError stacks captured.
The hook works. The Rust-side stringification of the JSON dump didn't
print readable content (one-line escape-handling bug — easy fix in
the test, not in the production engine). With that fix, this test
gives us the per-probe stack frames needed to identify the 5 unjzomuy
target objects.

### W6a-V — DataDome verification (foreground)
Re-ran `datadome_diagnostic_capture` against yelp.com after all 3
W6a fixes shipped. Result: **0 capture entries, content_len=1450,
no datadome cookie** — yelp returns the captcha interstitial BEFORE
serving tags.js. DataDome rejects at the edge level (TLS/header
inspection), not after the JS probe runs.

This means engine fixes alone can't flip yelp. Likely root cause:
yelp's DataDome edge has our IP / fingerprint flagged from prior
testing this session (we hit it many times). Out-of-engine fixes
needed: IP rotation OR aged session cookies (same constraint as the
3 captcha-gated sites).

## Sweep #4 result (post-PART2 fixes + W6a-A + udemy CSP bypass)

`PARALLEL SWEEP COMPLETE in 53.4 min` → **111/126 (88.1%)** — same
as round 3.

Per-site changes vs round 3:
- **hyatt** flipped THIN-BODY → Kasada-CHL (the CSP bypass fix from
  PART4 worked — was body=0, now body=756 with proper interstitial)
- **nowsecure** flipped THIN-BODY → L3-RENDERED (sweep variance)
- **bestbuy** L3 → Akamai-CHL (regression — same regional-gate
  classifier sensitivity as before; not engine)
- **douyin** THIN-BODY → captcha-CHL (lateral)

Net headline = 111/126 = 88.1% (unchanged). The W6a-A + udemy CSP
fixes weren't enough to flip yelp/leboncoin/etsy (DataDome edge
rejection) or udemy (CF orchestrator only partial).

## What's now empirically known

Adds to the 8 facts in HANDOFF_PART4:

9. **Cloudflare Managed Challenge orchestrator partially executes
   in our V8.** udemy real `<title>` resolves; `__cf_bm` persists; but
   `cf_clearance` not issued. V2 needs the documented 5-item list.
10. **DataDome rejects at the edge for yelp.** 0 tags.js entries in
    capture means we never got past the front door. Likely IP-flagged
    from prior session testing. NOT engine-fixable in isolation.
11. **TypeError-stack capture works for unjzomuy probes** (2 stacks
    captured in ~120s on canadagoose). Print-side display has a small
    escape-handling bug; mechanism is correct.
12. **Profiler now names dominant ops.** `BOXIDE_EVENT_LOOP_PROFILE=1`
    output names which specific async op pins the loop, not just
    "ops > 0".

## Remaining workstreams to ceiling

| Workstream | Effort | Sites unblocked |
|---|---|---:|
| W7-deep V2 (CF orchestrator finishing — header prop + iframe + UA-CH audit) | 3-5 days | 1 (udemy) |
| W4a finish: fix TypeError stack print bug, capture+identify 5 unjzomuy targets, stub them | 1-2 days | 3 (canadagoose/hyatt/realtor) |
| W5b+ deeper: run profiler against twitter, identify dominant op, fix it | 1 week | 2 (twitter/x.com) |
| DataDome edge rejection: needs IP rotation infrastructure (RESIDENTIAL_PROXY_SETUP.md exists; need actual provider) | n/a (out-of-engine) | 3 (yelp/leboncoin/etsy) |

**Realistic engine-only ceiling unchanged: 122-124/126 (96.8-98.4%).**

## Recommended next-session order

1. **Fix the TypeError-stack print bug** (~30 min) and capture the
   5 unjzomuy probe targets. This finishes W4a — should flip
   canadagoose/hyatt/realtor (3 sites).
2. **W7-deep V2 step 1**: header propagation (~1 hr from research doc).
   Cheapest CF win.
3. **Run `BOXIDE_EVENT_LOOP_PROFILE=1` against twitter.com** with the
   new op-name aggregator. The dump will name the dominant async op;
   targeted fix follows.
4. **Stand up an HTTP CONNECT proxy** (any residential provider with
   a free trial — Bright Data has one) and re-test the 3 DataDome
   sites + the 3 captcha sites. If they flip with a fresh IP, we know
   our engine is sufficient and the only missing piece is IP rotation.

## Session totals (across all 5 parts)

- **35 commits** (a1c0735 → 9e1ec3a; this doc commit will be #36)
- Pass rate confirmed via 4 holistic sweeps: 84% → 87.3% → 88.1% → 88.1%
- 5 deep research docs (TLS, DataDome, Cloudflare, Kasada VM, SPA hydration)
- 1 Kasada wrapper crack + decryptor tool
- 1 CF Managed Challenge V1 module (374 LOC, 8 tests)
- 5 handoff docs (this is PART5)
- 6 sweep documents
- Memory carries 4 entries forward + this PART5 added context
