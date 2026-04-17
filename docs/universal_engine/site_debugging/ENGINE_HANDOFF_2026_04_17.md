# Blocker Debugging Handoff (Rigorous Suite)
**Date: 2026-04-17**

## 1. Current Progress Snapshot
*   **Verification Score**: **8/8 PASS** (Primary targets like Adidas, Southwest, Ticketmaster, DNS-Shop, Wildberries).
*   **Rigorous Suite Score**: **5/8 WIN** (Unblocked HomeDepot and Wildberries Solver this session).
*   **Remaining Fails**: Canada Goose (Kasada), Hyatt (Kasada), Yandex (SmartCaptcha).

---

## 2. Issue: Kasada (Canada Goose / Hyatt)
**Symptom**: Stuck in challenge loop (200 OK) or 429 Rate Limit.

### Technical Debt / Hypothesis
In `crates/browser/src/page.rs`, we pre-fetch external scripts manually.
*   **The Gap**: Our manual script fetch logic uses a custom header map that might be missing `Accept-Encoding: gzip, deflate, br` or failing to propagate cookies from the initial HTML response.
*   **Evidence**: `ips.js` (Kasada's solver) is 500KB+. If we receive a small dummy script, Kasada has detected the fetch as a bot.
*   **Fix Path**: Refactor `Page::build_page_with_scripts_and_init` to use the full `HttpClient` defaults for pre-fetching, ensuring it behaves identically to a real browser's script tag engine.

---

## 3. Issue: Yandex (SmartCaptcha / SSO Redirect)
**Symptom**: Stuck on `https://sso.passport.yandex.ru/push?...`.

### Technical Debt / Hypothesis
Yandex uses an automated SSO flow where a hidden form is submitted via JavaScript.
*   **The Gap**: Our navigation loop in `Page::navigate` accurately tracks `location.href` changes, but automated form submissions (`document.forms[0].submit()`) might not be triggering the `__pendingNavigation` flag quickly enough or the event loop is idling before the submission starts.
*   **Evidence**: The page body contains a `<form>` and a script with a `retpath`.
*   **Fix Path**: 
    1.  Instrument `HTMLFormElement.prototype.submit` in `dom_bootstrap.js` to set `globalThis.__pendingNavigation`.
    2.  Check if `Page::navigate_loop_internal` needs a longer `run_until_idle` period (currently 30s, but Yandex PoW can be slow).

---

## 4. General Engine Issues to Fix

### A. Navigation Loop Polish
*   **Warnings**: My fixes for the loop introduced `unused_assignments` warnings for `current_method` and `current_body` in `page.rs`. These should be cleaned up to maintain zero-warning compilation.
*   **Stability**: The 2-second "anti-bot wait" I added to `navigate_loop_internal` is a heuristic. It should be replaced with a more robust signal if possible.

### B. Worker OpState Robustness
*   **Status**: Fixed the crash where workers lacked `OpState`.
*   **Next Step**: Verify that workers spawned *by other workers* (nested workers) correctly inherit the `StealthProfile`. This is critical for Tier-0.5 sites that offload complex crypto-challenges to background threads.

### C. TLS Wire-Level Fingerprinting
*   **Status**: Using BoringSSL.
*   **Next Step**: Some sites (including potentially Kasada) use JA4 fingerprinting. Ensure `crates/net/src/tls.rs` is not just "using BoringSSL" but is configured with a cipher suite order that matches a real Chrome 130 on Windows/MacOS.

---

## 5. Relevant Files for Next Developer
*   `crates/browser/src/page.rs`: Main navigation loop logic.
*   `crates/browser/src/script_runner.rs`: Script extraction and decoding.
*   `crates/js_runtime/src/js/dom_bootstrap.js`: DOM API overrides and instrumentation.
*   `crates/net/src/lib.rs`: Stealth HTTP client and header defaults.
