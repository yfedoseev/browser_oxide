# Session 2026-05-10 — Full Summary

The complete record of one day's work on browser_oxide. Read this if
you're picking up the project cold; it cross-references the detailed
docs without duplicating their content.

## TL;DR

- **Pass rate**: 84% → 88.1% (106/126 → 111/126), confirmed via 3
  holistic sweeps. A 4th sweep is running at this writing.
- **32 commits** (a1c0735 → 9d5f96d, plus this doc).
- **TLS layer = byte-perfect Chrome 147** (verified via `tls.peet.ws`).
- **Kasada `/tl` POST report wrapper cracked** (single repeating XOR
  with key `omgtopkek`; 5-line Python decryptor).
- **5 deep research docs** + **4 handoff docs** + **5 sweep docs** +
  **diagnostic test infrastructure** for Kasada/DataDome/Cloudflare.
- **Realistic ceiling: 122-124/126 (96.8-98.4%)** with 4 remaining
  workstreams documented and scoped.

## Headline narrative (chronological)

### Morning: discover + clean
1. Loaded session with prior memory claiming "SOTA achieved" and
   "Canada Goose blocked by datacenter IP only" per HANDOFF_2026_05_09.
2. Cleaned 31 untracked WIP files; preserved Kasada research scratch;
   hardened `.gitignore`.
3. Empirically disproved the "IP only" claim: same residential IP
   user's Chrome opens canadagoose.com fine. Engine has real leaks.

### Afternoon: 84% → 87.3%
4. Fixed `op_canvas_create` panic (deno_core 0.311 OpState borrow
   pattern broken by 6d21fc9). This unblocked *all* canvas-touching
   tests and was a session blocker.
5. Captured the Kasada `reporting.cdndex.io/error` POST body via the
   existing `kasada_error_blob_capture` test; cracked the wrapper
   (XOR omgtopkek). Identified **CSS calc precision probe** as the
   load-bearing root cause.
6. Implemented full **CSS Values 4 math functions** (calc/min/max/
   clamp/sin/cos/tan/sqrt/pow/log/hypot/abs/sign/mod/rem/round + pi/e/
   infinity constants) in `crates/css_values/src/calc.rs`. Wired into
   `op_dom_get_computed_style` so getComputedStyle returns resolved
   pixel values. Verified to drop 1 of 9 captured Kasada error blobs.
7. Three background research agents spawned in parallel (TLS, DataDome,
   Cloudflare). All three reported with ~2,600 lines of detailed
   research total.
8. **TLS verified byte-perfect Chrome 147** via tls.peet.ws JA4 capture.
   The canadagoose H2→HTTP/1.1 ALPN downgrade is NOT TLS — proven by
   favicon.ico returning 200+11.7KB STILL with h1.1. Per-origin Akamai
   config / IP rate-limit.
9. Comprehensive **Function.prototype.toString mask sweep** at end of
   `dom_bootstrap.js` (~50 Web API class prototypes + 25 top-level
   globals). Killed the visible `op_dom_attach_shadow` source leak.
10. Holistic sweep #1: 110/126 (87.3%). +4 vs morning.

### Evening: 87.3% → 88.1%
11. **W17a Akamai homedepot**: captured tenant_seed = 3,420,213 + the
    obfuscated POST path via Playwright MCP. Applied to akamai crate.
12. **W7a Cloudflare UA-CH**: implemented Critical-CH spec compliance
    (one-shot retry per W3 spec) + 2 missing high-entropy hints
    (sec-ch-ua-full-version singular + sec-ch-device-memory).
13. **W4a Kasada probe stubs**: 4 missing globals stubbed
    (PressureObserver, MediaSourceHandle, DocumentPictureInPicture,
    UserActivation). The aggregate `bot1225` walker probe cleared in
    the next error capture.
14. **W5b A3** SPA early-exit on populated `#react-root` — hulu went
    47% faster; twitter/x still THIN (their stall isn't classic
    Workers).
15. **W5b-deep Worker setInterval rewrite**: replaced the
    perpetually-pending `setInterval(5)` polling with a
    `tokio::sync::Notify`-backed async op pump. Structurally cleaner;
    helps any Worker-using SPA.
16. **W6a-B Worker realm `navigator.userAgentData`**: closes the
    cross-realm DataDome scoring contradiction.
17. **W6a-A pre-population of mouse-event buffer**: synthesizes 12
    historical sigma-lognormal coords + 1 synchronous mousemove
    dispatch BEFORE any antibot script can read the buffer. Closes
    DataDome's empty-coord-list scoring.
18. **CSP bypass extended** to hyatt/realtor/footlocker/ticketmaster/
    udemy (was just walmart+canadagoose). Caught hyatt going from
    Kasada-CHL → THIN-BODY (body=0) because CSP refused to load the
    Kasada ips.js script. Verified fix: hyatt body=0 → 756.
19. **humanize.js dispatches mousemove on `window`** in addition to
    document+body. Per W6a research, DataDome listens at window scope.
20. Holistic sweep #2: 111/126 (88.1%). +5 vs morning.
21. Holistic sweep #3 in progress at session-write time, with the last
    three CSP/humanize/UA-CH/W6a-A fixes folded in.

## What's empirically proven (cite next session)

1. **TLS = byte-perfect Chrome 147** — `t13d1516h2_8daaf6152771_d8a2da3f94cd`
2. **Kasada `/tl` wrapper = XOR with `omgtopkek`** (deployment-wide
   constant; `docs/kasada_ips_analysis/scratch/decrypt_report.py`)
3. **Date.getTimezoneOffset is patched** (`window_bootstrap.js:2113`)
   — W6a research's "not patched" was a false positive
4. **canadagoose H2→HTTP/1.1 is per-origin Akamai config**, NOT TLS
   (favicon.ico verifies)
5. **Kasada uses property-access not eval for unjzomuy probes**
   (verified 0 of 161 evals contain the identifier)
6. **Worker setInterval(5) was a perpetually-pending blocker** (now
   replaced with Notify async pump)
7. **DataDome rejects even with full CH on first visit** — multi-signal
   scoring (mouse-path + UA-CH + Date in concert)
8. **Engine outperforms headless Chrome on stealth** (15/18 vs 14/18
   probes) and uses **19× less memory** (40MB vs 750MB per page)

## Document inventory (everything created this session)

### Research (5)
- `RESEARCH_TLS_FINGERPRINT_FIX_2026_05_10.md` (470 lines)
- `RESEARCH_DATADOME_BYPASS_2026_05_10.md` (1316 lines)
- `RESEARCH_CLOUDFLARE_BYPASS_2026_05_10.md` (850 lines)
- `W4a_KASADA_PROBE_IDENTIFICATION_2026_05_10.md` (669 lines)
- `W5b_SPA_HYDRATION_PROFILE_2026_05_10.md` (~360 lines)
- `W6a_DATADOME_PROBE_GAP_MATRIX_2026_05_10.md` (980 lines)

### Handoff (4 in chain)
- `HANDOFF_2026_05_10.md` (round 1 — TLS + Kasada wrapper + CSS calc)
- `HANDOFF_2026_05_10_PART2.md` (round 2 — W17a/W7a/W4a/W5b)
- `HANDOFF_2026_05_10_PART3.md` (verification — empirical sweep results)
- `HANDOFF_2026_05_10_PART4.md` (round 3 finale — hyatt CSP + humanize)

### Sweep (5)
- `HOLISTIC_TEST_2026_05_10/SUMMARY.md` (morning baseline 106/126)
- `HOLISTIC_TEST_2026_05_10/FAILURE_ROOT_CAUSES.md` (per-site failure
  classification)
- `HOLISTIC_TEST_2026_05_10/SESSION_DELTA.md` (mid-session focused
  retest)
- `HOLISTIC_TEST_2026_05_10/SESSION_FINAL.md` (end-of-PART2 sweep
  110/126)
- `HOLISTIC_TEST_2026_05_10/SESSION_FINAL_R3.md` (end-of-PART3 sweep
  111/126)

### Comparison + Operational (3)
- `COMPARISON_2026_05_10.md` (vs Chrome 147 headless — oxide wins on
  memory + stealth)
- `RESIDENTIAL_PROXY_SETUP.md` (proxy infra docs)
- `kasada_ips_analysis/scratch/CRACK_PROGRESS_2026_05_10.md` + the
  `decrypt_report.py` decryptor

### Plans (2)
- `PLAN_2026_05_10.md` (original workstream-ranked plan)
- `PLAN_2026_05_10_UPDATE.md` (post-TLS-verification addendum)

### Diagnostic + per-domain (3)
- `CANADA_GOOSE_DIAGNOSIS_2026_05_10.md` (P0-P5 root causes)
- `CANADA_GOOSE_DIAGNOSIS_2026_05_10_PART2.md` (post-CSS-calc inventory
  of 13 remaining error fields)

## Memory entries written (auto-persist across sessions)

- `state_2026_05_10.md` — current ground truth post-session
- `kasada_real_blocker_css_calc_math.md` — CSS calc gap was load-bearing
- `kasada_wrapper_cracked_and_remaining_leaks.md` — wrapper algorithm +
  field inventory
- `session_delta_2026_05_10.md` — workstream completion summary

Stale memory removed: `session_2026_04_10_tier0_0.5.md` (was claiming
48/50 L3 with metrics 22 days obsolete).

## What still needs work (4 workstreams to ceiling)

| Workstream | Effort | Sites unblocked |
|---|---|---:|
| **W6a #C** finishing (mouse-path synth done; verify the W6a-A+B+Date triplet flips DataDome sites) | 1-2 days verification | 3 (yelp/leboncoin/etsy) |
| **W4a deeper** — V8 TypeError-construction hook to identify the 5 unjzomuy probe targets via receiver capture (eval-source approach proven not to work) | 2-3 days | 3 (canadagoose/hyatt/realtor) |
| **W7-deep** — execute Cloudflare orchestrator JS to completion in our V8 | 6-9 days | 1 (udemy) |
| **W5 Tier B+** — identify what specifically pins twitter/x's loop now that Workers are fixed; likely ServiceWorker promise chains | 1 week | 2 (twitter/x.com) |

**Realistic ceiling: 122-124/126 (96.8-98.4%)** with all four shipped.

The 3 captcha-gated sites (douyin/spotify/sometimes iphey) are the
inherent floor — even real headed Chrome from a non-residential IP
gets challenged on these.

## Next-session recommended order

1. **Wait for round-4 sweep to land** (started at session-write time;
   covers hyatt CSP / humanize-window / udemy CSP / W6a-A buffer
   pre-pop / W6a-B Worker UA-CH). If it lands at 112+/126 = 88.9%+
   the W6a fixes are working. If yelp/etsy/leboncoin flip → DataDome
   triplet was sufficient. Take 30 min.
2. **W6a verification**: if DataDome sites are still failing, capture
   the new error report via the kasada_error_blob_capture pattern
   (or equivalent for DataDome) and decrypt to see what specifically
   still tells.
3. **W4a deeper** — start with hyatt or canadagoose. Add a hook in
   our V8 init that intercepts `TypeError` construction and logs the
   receiver reference. Identify the 5 unjzomuy probe target objects.
   Add stubs.
4. **W7-deep** Cloudflare orchestrator JS — only after W4 + W6 are
   done since they have higher per-effort site-flip ratio.
5. **W5 Tier B+** — last because twitter/x is the lowest immediate
   business value among the remaining workstreams.

## Engineering reminders for any future session

1. **Capture-driven debugging is the high-ROI tool.** The
   `kasada_error_blob_capture` + `decrypt_report.py` round-trip surfaced
   more engine bugs in one day than the prior month of guesswork.
   Replicate this pattern for any new vendor.

2. **Don't trust prior handoffs without re-verifying.** This session
   started by disproving the 05-09 handoff's "IP only" claim. The
   05-09 handoff also missed multiple CSS engine gaps and Function.
   toString leaks because no fresh sweep was run before claiming SOTA.

3. **Real Chrome reference captures** are essential for any vendor
   workstream. TLS work this session validated against live tls.peet.ws.
   DataDome workstream needs the same — capture from real Chrome on
   the same residential IP, diff against ours.

4. **The 19× memory advantage matters** — never sacrifice it for
   marginal pass-rate gains. Per `COMPARISON_2026_05_10.md`, oxide
   stays under 50 MB while Chrome holds 700+ MB per page. Measure
   memory after each engine fix.

5. **Add `holistic_sweep_parallel` to nightly CI** so any commit that
   drops the pass count below baseline is caught immediately. Currently
   `#[ignore]`'d, only run manually.
