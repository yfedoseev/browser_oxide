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
**Symptom**: Stuck in challenge loop (200 OK) or 429 Rate Limit.

### Technical Debt / Hypothesis
In `crates/browser/src/page.rs`, we pre-fetch external scripts manually.
*   **The Gap**: Our manual script fetch logic uses a custom header map that might be missing `Accept-Encoding: gzip, deflate, br` or failing to propagate cookies from the initial HTML response.
*   **Evidence**: `ips.js` (Kasada's solver) is 500KB+. If we receive a small dummy script, Kasada has detected the fetch as a bot.
*   **Fix Path**: Refactor `Page::build_page_with_scripts_and_init` to use the full `HttpClient` defaults for pre-fetching, ensuring it behaves identically to a real browser's script tag engine.

---

## 4. General Engine Issues to Fix

### A. Navigation Loop Polish
*   **Stability**: The 2-second "anti-bot wait" in `navigate_loop_internal` should be replaced with a more robust signal if possible.

### B. Worker OpState Robustness
*   **Status**: Fixed the crash where workers lacked `OpState`.
*   **Next Step**: Verify that workers spawned *by other workers* (nested workers) correctly inherit the `StealthProfile`.

### C. TLS Wire-Level Fingerprinting
*   **Status**: Using BoringSSL.
*   **Next Step**: Some sites (including potentially Kasada) use JA4 fingerprinting. Ensure `crates/net/src/tls.rs` matches real Chrome 130 cipher suites.

---

## 5. Relevant Files for Next Developer
*   `crates/browser/src/page.rs`: Main navigation loop logic.
*   `crates/browser/src/script_runner.rs`: Script extraction and decoding.
*   `crates/js_runtime/src/js/dom_bootstrap.js`: DOM API overrides and instrumentation.
*   `crates/net/src/lib.rs`: Stealth HTTP client and header defaults.
