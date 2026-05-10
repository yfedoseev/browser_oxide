# Canada Goose engine-side diagnosis — 2026-05-10

## Summary

`HANDOFF_2026_05_09.md` claimed Canada Goose was "VERIFIED engine parity, blocked
by datacenter IP reputation only." This is **wrong**. The engine still has
multiple addressable leaks that prevent the page from loading. The claim was
made without re-running the test against the post-05-09 code; the supporting
`cg_run*.log` files all predate the handoff by 4–5 days.

Egress IP for this investigation: `66.183.9.212` / `AS852 TELUS Communications`
(Vancouver residential). Same IP user's headed Chrome successfully opens
canadagoose.com from. **IP reputation is not the gate.**

## Issues found, in priority order

### P0 — H2 ALPN downgrade to HTTP/1.1
- **Symptom**: every connection to `www.canadagoose.com` logs
  `[net] H2 connection failed: HTTP error: ALPN negotiated http/1.1, not h2`
- **Why it matters**: Chrome 147 always negotiates h2 to a Kasada/DOSarrest
  edge. Falling back to HTTP/1.1 is an instant bot signal — and on Kasada-
  fronted CDNs the server actively *downgrades* suspicious clients to h1 as
  a soft-block tell. Our ALPN list is correct (`h2` first), so the server is
  picking h1 based on something else in the ClientHello.
- **Where to look**: `crates/net/src/tls.rs` — `chrome_connector()`. The TLS
  fingerprint constants (CIPHER_LIST, SIGALGS_LIST, CURVES, extension
  permutation) need to be diffed against a real Chrome 147 ClientHello
  capture. HANDOFF_2026_05_08 acknowledged this as open and HANDOFF_2026_05_09
  did not document a fix.
- **Status**: Not fixed in this session. Needs a Wireshark/tcpdump capture
  of real Chrome → canadagoose.com to diff against ours.

### P1 — `op_canvas_create` / `op_webgl_create_context` panic (FIXED)
- **Symptom**: `panic at gotham_state.rs:74: required type
  deno_core::ops::OpState is not present in GothamState container` —
  aborted every test that exercised canvas (so basically every Kasada VM
  run, since the Kasada VM does heavy canvas drawing).
- **Cause**: commit `6d21fc9` changed both ops to take `&mut OpState`
  annotated with `#[state]` so they could fetch `canvas_seed` from DomState.
  In deno_core 0.311 that signature generates a self-borrow that panics.
- **Fix (commit 3e36ecd)**: revert to `#[state] state: &mut CanvasState`
  pattern, pass `canvas_seed` explicitly as a `#[bigint] u64` arg. JS
  reads it from `op_get_profile_value("canvas_seed")` (newly exposed key)
  and caches.

### P2 — `reporting.cdndex.io/error` POST stream — IDENTIFIED
- **Symptom**: during canadagoose.com navigation our engine generates
  multiple POSTs to `reporting.cdndex.io/error`. These are Kasada's
  client-side error reporters — `ips.js` catches runtime exceptions
  thrown inside its VM and POSTs them to that endpoint.
- **Why it matters**: real Chrome under Kasada doesn't generate these.
  Every error report is a tell that the bytecode VM blew up on something
  our engine doesn't implement correctly.
- **Captured (2026-05-10)**: ran `kasada_error_blob_capture` test which
  intercepts TextEncoder + XHR + fetch for `*.cdndex.io/error`. 9 blobs
  captured at `docs/kasada_ips_analysis/scratch/kasada_error_*.b64`.
  Blob #0 (1283 bytes raw text BEFORE encryption) is the smoking gun:
  contains deeply nested CSS `calc()` expressions with `sin()`, `cos()`,
  `tan()`, `sqrt()`, `pi`, `e` constants — e.g.
  `calc( 1px * ( ( 2.71828 * 0.987654321 - ... ) + 0.5555555 / sin( sin( 10000.1 * tan( 50000 ) / tan( 20000 ) + 1.0 / pi * 5.0 - 0.1111 ) / 100.0 + tan( 30000 + 40000 * 50000 + 0.0001 ) / 9999.9 * pi ) - ... ) )`.
- **Diagnosis**: Kasada uses a **CSS calc() math-function precision
  fingerprint probe**. Per CSS Values 4 (https://www.w3.org/TR/css-values-4/#math-function)
  Chrome supports `sin/cos/tan/asin/acos/atan/atan2/sqrt/pow/exp/log/hypot/abs/sign/mod/rem/round` plus
  `pi/e/infinity/NaN` constants inside `calc()`. Our `CalcExpr` enum at
  `crates/css_values/src/types/length.rs:43-57` only supports
  Add/Sub/Mul/Div/Negate/Min/Max/Clamp — none of the math functions.
  When Kasada injects these expressions and reads the computed value back
  via `getComputedStyle`, we either return `auto` or an arithmetic
  default; the comparison fails, the VM throws, the error gets POSTed.
- **Status**: Root cause identified. Fix is to extend `CalcExpr` +
  parser + evaluator with all CSS Values 4 math functions, with
  bit-identical f64 semantics to Chrome (libm-equivalent). Tracked as a
  separate task — see task #9.

### P3 — `/mfc` returns 200 but no `x-kpsdk-fc`
- **Symptom**: with the `cookies for /mfc` log line, /mfc now returns 200
  (was 304). But the response still lacks `x-kpsdk-fc` — meaning Kasada
  decided not to issue a forwarded-challenge token.
- **Why it matters**: without `x-kpsdk-fc`, the Hyper-Solutions Flow 2
  cannot complete on stricter tenants.
- **Where to look**: `crates/net/src/lib.rs:443-490`
  (`fetch_kasada_mfc_if_needed`). The request hardcodes
  `x-kpsdk-dt: 11qox8sw33mzd5rx62nvw43pjz99vza39w0a3lycjlwbby5126x2thw75s`
  on every fetch — this is a session token Kasada normally derives from
  page state. Hardcoding it is a clear tell and probably the reason fc
  isn't issued.
- **Status**: Not fixed. The right fix is to source `x-kpsdk-dt` from the
  page's V8 state (set by ips.js during init) rather than hardcoding it,
  and possibly let the page's own `fetch()` call hit /mfc instead of
  racing it from Rust.

### P4 — Spurious `/akam/13/sensor_data` POST
- **Symptom**: `Page::navigate` calls `handle_akamai_flow` unconditionally
  for every page. For canadagoose.com — which is Kasada-protected, NOT
  Akamai-protected — the cookie jar contains `_abck` (set by Kasada's
  edge for caching), so the fallback at `crates/browser/src/page.rs:213-216`
  triggers a POST to `/akam/13/sensor_data` with bestbuy-format v2
  sensor_data. The response is the Kasada interstitial HTML with status
  429 — Kasada's edge sees a request without valid `x-kpsdk-ct` and
  returns the challenge page.
- **Why it matters**: low-priority because the real engine bug is the H2
  + Kasada chain. But the spurious POST burns budget and pollutes logs.
- **Where to look**: `crates/browser/src/page.rs:204-218`. The `_abck`
  fallback is too liberal. Gate on a stronger Akamai signal (e.g. `_abck`
  with the `~0~-1~` suffix specifically — that's the "untrusted, prove
  you're real" marker that means Akamai actually wants sensor_data).
- **Status**: Not fixed.

### P5 — Default V8 deadline is 15s; Kasada VM needs 30+s
- **Symptom**: `[V8DeadlineWatcher] deadline 8000ms expired — firing
  terminate_execution` repeatedly mid-ips.js execution.
- **Cause**: `Page::navigate` defaults `BOXIDE_NAV_BUDGET_MS=15_000` at
  `crates/browser/src/page.rs:1019`. The deadline floor inside the loop
  is `max(remaining, 5s)` — so iterations past the budget get only 5s.
  The comment at `page.rs:1112-1116` literally says "Kasada KPSDK takes
  30+ seconds" but the budget undershoots that.
- **Why it matters**: the ips.js VM running inside our V8 is heavy
  (~530KB script, ~1000s of opcodes). Without enough time, the PoW
  finishes but the post-PoW phase that issues the cookie / fetches /mfc
  / posts /tl never runs to completion.
- **Where to look**: `crates/browser/src/page.rs:1015-1026`. Either bump
  the default to 45s for Kasada-known hosts, or autodetect "we are still
  on a challenge interstitial after first iter" and extend.
- **Status**: Workaround verified — running with
  `BOXIDE_NAV_BUDGET_MS=60000 BOXIDE_NAV_BUDGET_EXTEND_MS=60000` lets the
  PoW + ct token learning complete. Fix would be to wire host-aware
  defaulting, not require the env var.

## Validation observations
- After P1 fix, the Kasada PoW *does* complete:
  `[kasada] LEARNED x-kpsdk-ct for www.canadagoose.com (len=174)` and
  the engine *does* inject it on subsequent requests
  (`[kasada] INJECTING x-kpsdk-ct on GET www.canadagoose.com (len=174)`).
- But the reload still gets 429 — so even with valid ct token, Kasada
  rejects. This points back to either P0 (H2 downgrade is the actual
  decisive signal) or P2 (the error reports are flagging us during the
  initial /tl POST so the issued ct token isn't trusted).

## Recommended fix sequence
1. Capture a `reporting.cdndex.io/error` POST body and identify the JS
   error our engine throws inside the Kasada VM (P2). The error name
   should pinpoint a missing/wrong API.
2. Fix that API gap.
3. Wireshark-capture real Chrome → canadagoose.com TLS ClientHello and
   diff against ours; fix whatever extension/cipher/sigalg drives the
   ALPN downgrade (P0).
4. Source `x-kpsdk-dt` from page state, not the hardcoded literal (P3).
5. Tighten the Akamai sensor_data gate to the `~0~-1~` suffix (P4).
6. Make the V8 deadline host-aware so Kasada hosts get 45s default (P5).

Once 1–3 are done, the page should load. 4–6 are polish that prevents
collateral damage on similar sites.
