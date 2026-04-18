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

### Ruled Out This Session (exhaustive diagnostics, 3 experiments)

**ips.js does NOT patch `window.fetch` globally.** `window.fetch.toString()` still returns OUR wrapper unchanged after ips.js runs. ips.js passes `x-kpsdk-ct` explicitly as `init.headers` only on its own requests to `/tl` and `/fp`.

**KPSDK state is closure-private.** After solve, `window.KPSDK` only exposes `{now, start, scriptStart}`. The solved token lives in ips.js closures, unreachable from outside.

**sec-fetch-site = same-origin + no sec-fetch-user + Referer doesn't help.** Added `chrome_headers_reload()` + `get_follow_exact_headers()` and wired reload-semantic headers into the retry path. Server still returns 429.

**Cookies, timing, and TLS state are ALL independent of the block.** The raw-cookies diagnostic proves it:

| Step | Request | Result |
|---|---|---|
| 1 | Initial GET with clean jar | `429, 681b` |
| 2 | Immediate re-GET (same H2 pool, cookies from jar) | `429, 681b` |
| 3 | GET with reload headers + Referer | `429, 681b` |
| 4 | GET after 3s wait | `429, 681b` |
| 5 | **FRESH HttpClient, no cookies, new TLS** | `429, 681b` |

Step 5 is decisive: a brand-new client with no prior state gets the identical block. So:
- Not TLS session-ticket pinning (fresh TLS → same block)
- Not cookie freshness (no cookies → same block)
- Not timing (wait → same block)
- Not connection reuse (new connection → same block)

**Subpath test confirms it's not URL-specific.** Root, `/us/en`, product pages, category pages — all return the Kasada challenge page. Every URL on canadagoose.com returns 680-770 bytes with the challenge markers.

### Remaining Candidate: IP / Fingerprint Reputation

The only layer left we haven't directly controlled is **IP reputation / machine fingerprint**. Kasada runs a global IP reputation feed and also scores behavior. A datacenter IP (our sandbox) is almost certainly hard-blocked regardless of client fingerprint.

### Actionable Fixes (all infrastructure, not engineering)

Canada Goose / Hyatt can only pass from:
1. **Residential or rotating-IP proxy** — workaround for IP rep. No code change. The engine is already correct.
2. **Commercial anti-bot bypass service** (Hyper Solutions, RiskByPass) — they maintain solved-session pools.
3. **Leave it blocked.** For most business cases these are special-case targets anyway; the engine passes 6/8 rigorous sites without per-site code.

### What the Engine Gained From This Investigation
*   `net::headers::chrome_headers_reload()` — reload-semantic header set for any future same-origin retry
*   `net::HttpClient::get_follow_exact_headers()` — bypass chrome_headers overlay when the caller needs exact control
*   In-V8 refetch primitive (still valuable for fetch-patching engines like some PerimeterX / DataDome variants)
*   Confidence that no further client-side engineering will move the needle on Kasada specifically — the block is at a layer below the client.

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
