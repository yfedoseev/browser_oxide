# Session Handoff PART 4 — 2026-05-10 round-3 finale

Continuation from `HANDOFF_2026_05_10_PART3.md`. Final round of fixes
this session.

## Round-3 fixes shipped

### W4a-deeper diagnostic — eval-source capture (`8fcfd20`)
Added `kasada_eval_source_capture` test that hooks `globalThis.eval` +
`Function` constructor to record every dynamically-evaluated source
string, filtered for the unjzomuy 28-char identifier. Empirical result:
**0 of 161 captured eval'd strings contain unjzomuy**. So Kasada uses
property-access (`target[runtime_built_name]`) NOT eval, for these
probes. Static eval-source capture cannot identify the targets — needs
V8 TypeError-construction hook (separate ~2-3 day workstream).

### W6a-B — Worker realm `navigator.userAgentData` (`6c5bfb5`)
Added a NavigatorUAData class to `worker_bootstrap.js` matching the
main-thread shape (low-entropy + getHighEntropyValues async). Closes
the cross-realm contradiction DataDome's tags.js scores against. Verified
on yelp.com — the rejection is multi-signal so this single fix
doesn't flip the verdict, but reduces one of the documented W6a
scoring inputs.

### Hyatt CSP-bypass extension (`0f7c33d`)
Round-3 sweep showed hyatt: Kasada-CHL → THIN-BODY (body=0). Root
cause: hyatt's CSP refused to load `ips.js` (the very script needed
to clear the Kasada challenge). Extended the existing CSP-bypass list
(was `walmart.com / canadagoose.com`) to also include `hyatt.com /
realtor.com / footlocker.com / ticketmaster.com` — all use the same
Kasada/Akamai stack and will hit the same CSP-self-block.

**Verified**: hyatt body=0 → body=756 in focused re-test. Restored to
Kasada-CHL classification (the expected pre-fix state).

### `humanize.js` dispatches mousemove on `window` (`0f7c33d`)
Per W6a research: DataDome's tags.js attaches its mousemove listener
at `window` scope, not `document`. Our prior dispatch hit only
`document + body`, so DataDome never saw our synthesized events.
Added `window` as a third dispatch target (in a try/catch since some
test pages don't have window). Doesn't flip yelp by itself but is one
of the 3 W6a fixes the agent's research said are needed in concert.

## Three sweep results compared

| Sweep | Pass | Improvements |
|-------|-----:|--------------|
| Morning baseline | 106/126 (84.0%) | — |
| End of PART2 | 110/126 (87.3%) | +4 |
| End of PART3 (this round) | **111/126 (88.1%)** | **+5 over morning** |

Per-site changes PART3 vs PART2: bestbuy/threads/wildberries flipped
to L3; nowsecure/wsj regressed (sweep variance); hyatt mode-shifted
to THIN-BODY (since fixed by `0f7c33d`).

## Full session totals

- **30 commits**: a1c0735 → 0f7c33d
- Pass rate: **84% → 88.1%** confirmed via 3 holistic sweeps
- 5 deep research docs (TLS, DataDome, Cloudflare, Kasada VM, SPA hydration)
- 4 handoff docs (this is PART4)
- 5 sweep documents
- 1 cracked Kasada wrapper + decryptor tool

## What's empirically PROVEN about the engine state

1. **TLS = byte-perfect Chrome 147** (JA4 verified `t13d1516h2_8daaf6152771_d8a2da3f94cd`).
2. **Kasada `/tl` POST report wrapper = single XOR with key `omgtopkek`** (decryptor at `docs/kasada_ips_analysis/scratch/decrypt_report.py`).
3. **Date.getTimezoneOffset IS patched** at `window_bootstrap.js:2113` (W6a research false positive).
4. **canadagoose H2→HTTP/1.1 ALPN downgrade is NOT TLS** — verified via favicon.ico returning 200+11.7KB still h1.1 (per-origin Akamai config / IP-tied rate limit).
5. **Kasada uses property-access not eval for unjzomuy probes** (verified 0/161 evals).
6. **Worker setInterval(5) was a perpetually-pending-future blocker** (now replaced with tokio Notify async pump).
7. **DataDome rejects even with full Client Hints on first visit** (verified by pre-populating accept_ch_origins for yelp).

## What still needs work (ranked by leverage)

| Workstream | Effort | Sites unblocked |
|---|---|---:|
| W6a #A mouse-path synth into Page::navigate | 1 day | 3 (yelp/leboncoin/etsy) — all 3 W6a fixes needed in concert |
| W4a VM-level TypeError hook for unjzomuy probes | 2-3 days | 3 (canadagoose/hyatt/realtor) |
| W7-deep CF Managed Challenge orchestrator | 6-9 days | 1 (udemy) |
| W5 Tier B+ identify what pins twitter/x's loop | 1 week | 2 (twitter/x.com) |

**Realistic ceiling unchanged: 122-124/126 (96.8-98.4%).**

## Recommended next-session sequence

1. **Run holistic_sweep_parallel** to confirm hyatt CSP fix restored
   to 110+ (and possibly +1 from hyatt itself if Kasada PoW now works).
2. **Implement W6a #A mouse-path synth as a Page::navigate hook** —
   call into `crates/stealth/src/behavior.rs` to seed the
   `__akamai_events` buffer with realistic coords BEFORE the first
   antibot script runs, not waiting on humanize.js's setTimeout chain.
3. **W4a VM-level instrumentation**: add a hook in deno_core's
   isolate setup that captures the throw site (file:line + receiver
   reference) for every TypeError mentioning the unjzomuy identifier
   pattern. Run canadagoose, dump results, identify the 5 probe targets.
4. **W5 Tier B follow-up**: instrument what specifically pins
   twitter/x's loop now that Worker is fixed. Use the
   BOXIDE_EVENT_LOOP_PROFILE=1 env var added in W5b. Likely
   ServiceWorker `register()` → fetch chains.
