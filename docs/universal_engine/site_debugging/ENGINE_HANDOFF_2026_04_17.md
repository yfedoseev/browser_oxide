# Blocker Debugging Handoff (Rigorous Suite)
**Date: 2026-04-17**

## 1. Current Progress Snapshot
*   **Verification Score**: **8/8 PASS** (Primary targets like Adidas, Southwest, Ticketmaster, DNS-Shop, Wildberries).
*   **Rigorous Suite Score**: **6/8 WIN** (Unblocked HomeDepot, Wildberries Solver, and **Yandex** this session).
*   **Remaining Fails**: Canada Goose (Kasada), Hyatt (Kasada).

---

## 2. Breakthrough: Yandex (SmartCaptcha / SSO Redirect)
**Status**: **WIN** (488,914b real homepage loaded).

### Technical Debt Fixed
1.  **h1 POST fallback**: Fixed bug where query strings were stripped during HTTP/1.1 POST fallback. This unblocked the Yandex SSO `/install?uuid=...` endpoint.
2.  **Bootstrap Cleanup**: Fixed `cleanup_bootstrap.js` wiping `__pendingNavigation` too early.
3.  **Form Reflection**: Added `HTMLFormElement` and `HTMLInputElement` property reflection, allowing JS-based form submissions to work correctly.
4.  **Navigation Loop**: Updated `Page::navigate` to correctly handle auto-submitted forms and SSO redirects.

---

## 3. Issue: Kasada (Canada Goose / Hyatt)
**Symptom**: Stuck on 732-byte challenge page. Solver runs end-to-end but server never upgrades us to real content.

### Diagnostic Findings
`ips.js` loads at full 519KB and executes. Fetch log shows the full success trace:
*   `POST /149e9513-…/tl` returns `200` with `x-kpsdk-cr: true` — server accepts challenge
*   Fresh `x-kpsdk-ct` returned, Set-Cookie with `akm_bmfp_b2=…` lands in jar
*   `document.cookie` at exit contains the new token (cookie sync works)
*   Post-settle retry + in-V8 refetch both fire carrying the fresh cookie
*   Server still returns the 732-byte challenge on every retry

TLS / H2 / headers match Chrome 146 capture (verified via tls.peet.ws). Script-fetch headers include referer + sec-fetch-*. `ips.js` contains **zero** `location`/`reload`/`href` references in 519KB — it never triggers navigation itself.

### Ruled Out This Session
*   **ips.js does NOT patch `window.fetch` globally.** `window.fetch.toString()` still returns OUR wrapper unchanged after ips.js runs. ips.js passes `x-kpsdk-ct` explicitly as `init.headers` only on its own requests to `/tl` and `/fp`. So the "Option A" in-V8 refetch (leveraging a patched fetch) doesn't help on Kasada's top-level URLs.
*   **KPSDK state is closure-private.** After solve, `window.KPSDK` only exposes `{now, start, scriptStart}`. The solved token lives in ips.js closures, unreachable from outside. No public `getClientToken()` method.
*   **Cookies alone don't upgrade the session.** Jar carries the fresh `akm_bmfp_b2` on every retry; server keeps issuing new challenges.

### Remaining Candidate Factors (need external evidence)
*   TLS session-ticket resumption — Kasada likely pins sessions to TLS state, not cookies. Verify with a Wireshark capture: does real Chrome reuse the session ticket from the solve POST on its retry GET? Does our connection pool reuse it?
*   H2 connection-id stickiness on the Kasada edge
*   Timing window between solve and retry

### Fix Path
Capture real Chrome 130 HAR + TLS keylog against canadagoose.com. Compare the TLS handshake on the post-solve GET against our Rust client. If session resumption is the tell, enable ticket reuse in `crates/net/src/tls.rs`. If it's something else, the HAR will show it. **Do not speculate further without this evidence.**

---

## 4. General Engine Work

### A. Worker OpState Robustness
*   **Status**: Fixed the crash where workers lacked `OpState`.
*   **Next Step**: Verify that workers spawned *by other workers* (nested workers) correctly inherit the `StealthProfile`.

### B. TLS Wire-Level Fingerprinting
*   **Status**: Using BoringSSL with Chrome 146-matched cipher order, H2 pseudo-header order, and SETTINGS frame. Verified via tls.peet.ws.
*   **Next Step**: JA4-level diff against a live Chrome 130 capture for Kasada-specific sites.

---

## 5. Relevant Files for Next Developer
*   `crates/browser/src/page.rs`: Main navigation loop logic.
*   `crates/browser/src/script_runner.rs`: Script extraction and decoding.
*   `crates/js_runtime/src/js/dom_bootstrap.js`: DOM API overrides and instrumentation.
*   `crates/net/src/lib.rs`: Stealth HTTP client and header defaults.
