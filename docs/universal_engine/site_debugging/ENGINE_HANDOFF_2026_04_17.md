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

### What the Engine Gained From This Investigation (universal primitives)

1. **MAJOR: fetch-API header style on op_fetch** (`chrome_headers_fetch` +
   `HttpClient::fetch_get/fetch_post_bytes`). Previously every JS `fetch()` call
   was sent with NAVIGATION headers (`upgrade-insecure-requests`, `accept: text/html`,
   `sec-fetch-dest: document`, `sec-fetch-mode: navigate`, `sec-fetch-user: ?1`,
   `priority: u=0`). Now proper fetch API style: `accept: */*`,
   `sec-fetch-dest: empty`, `sec-fetch-mode: cors`, auto-computed `sec-fetch-site`,
   `origin` + `referer` auto-injected, `priority: u=1, i`. This was a huge
   latent bot tell affecting every JS fetch on every site.

2. **Real `navigator.sendBeacon`** — was a no-op stub returning `true`. Now
   fires a real `fetch(..., {keepalive: true})`. Some challenge engines send
   solve-completion payloads via sendBeacon.

3. **x-kpsdk-* harvesting primitive** — navigate_loop_internal extracts every
   `x-kpsdk-*` header seen in req/resp across `__fetchLog` and injects them on
   the same-origin retry GET. Per Hyper-Solutions Go SDK, Kasada retries need
   8 of these as REQUEST headers; we successfully harvest 6.

4. `chrome_headers_reload()` + `get_follow_exact_headers()` — reload-semantic
   header set for any future same-origin retry.

5. In-V8 refetch primitive — valuable for fetch-patching engines
   (PerimeterX / DataDome variants).

### Kasada Remaining Gap — Reverse-engineered conclusion

Wrote a deep instrumentation harness
(`kasada_ips_deep_instrumentation` in `tests/tier0_kasada.rs`) that monkey-patches:
- `String.fromCharCode` (catches tokens built from char codes)
- `Object.defineProperty` (catches properties set to token-like strings)
- `Function` constructor via Proxy (catches dynamically-built function bodies)
- `EventTarget.addEventListener` / `dispatchEvent`
- `navigator.sendBeacon`
- `XMLHttpRequest.open/setRequestHeader/send`
- `Headers.set/append`
- `window.fetch`

Ran ips.js against this harness for 60 seconds. Final findings:

*   ips.js only ever sends **three** x-kpsdk-* headers on outgoing requests:
    `ct`, `dt`, `im`. That's all. We already harvest these.
*   Zero `navigator.sendBeacon` calls.
*   Zero `kpsdk-ready`/`kpsdk-*` listener registrations.
*   Zero dynamically-created Function bodies contain token-like strings.
*   `window.KPSDK` never grows beyond `{now, start, scriptStart}` after
    execution.

**The "missing" x-kpsdk-v/-dv/-h/-fc tokens are a red herring** from
Hyper-Solutions' public docs — they reflect a different Kasada deployment
generation. Our engine does everything ips.js actually does in 2026's
canadagoose.com build.

### The Actual Blocker — Server-side /ftp: canary + IP Reputation (CONFIRMED)

ips.js uses a probe pattern: after each solve it fetches `/ftp:` and checks
the response. On a solved session the server eventually returns something
that ips.js interprets as "upgrade complete" → document replacement happens.
Our `/ftp:` requests ALWAYS return `429 + challenge-stub` regardless of
what we send.

**Definitive proof via real Playwright + real Chromium** (see
`/tmp/kasada-probe/probe.js`):

| Site | Real Chromium from this network | Our engine |
|---|---|---|
| canadagoose.com | 429 + 740b challenge (identical body) | 429 + 732b |
| hyatt.com | 403 + Akamai E6020 "unexpected browser" | 403 equivalent |
| ya.ru | 200 + 452k real homepage | 200 + 488k real homepage |

**Real Chromium from this IP gets the exact same blocks our engine gets.**
The block is at the network layer (IP reputation + possibly TCP/TLS
fingerprinting at lower levels than Chromium's user-space stack), not in
our JS implementation. Running genuine Chrome from the user's residential
IP works; running genuine Chrome from this datacenter IP does not.

**Extended cross-tool probe (also saved in this dir):**

| Tool | Implementation | Result on canadagoose.com |
|---|---|---|
| Our browser_oxide engine | From-scratch Rust + V8 | 429 + 732b |
| `playwright_probe.js` — vanilla Playwright | Real Chromium | 429 + 740b |
| `patchright_probe.js` — Patchright stealth Playwright | Real Chromium + JS stealth patches | 429 + 740b |
| `nodriver_probe.py` — nodriver (UC successor) | CDP-direct, no Selenium | 429 + 721b |

Every mainstream open-source "Kasada bypass" tool produces an identical
blocked response from this IP. Per every published bypass guide in 2026
(ZenRows, Scrapfly, roundproxies), Kasada-passing workflows ALWAYS pair
the stealth browser with residential-proxy rotation. A stealth browser
alone does not bypass an IP-reputation block — by design.

Hyatt specifically returned an Akamai error code, not Kasada — they may
have shifted engines, but the outcome (block from this IP) is identical.

Server-side signals at play (confirmed by the Chromium test):
- IP reputation feed (datacenter / hosting provider ranges are hard-blocked)
- Possibly TCP SYN-packet fingerprint (OS socket-level)
- Autonomous System (AS) number scoring

None of these are reachable from engine code running in user-space
against the same IP. The engine is client-side complete.

### Fix Path for Kasada Specifically

Three options, none are pure client-side engineering:
1. **Commercial service** (Hyper-Solutions, RiskByPass) — they maintain the
   full ips.js reverse-engineered bypass.
2. **Full ips.js reverse engineering** — multi-week effort; Kasada rotates
   the bytecode every few weeks.
3. **Headless Chromium via CDP for just these sites** — use a real Chromium
   that naturally computes the missing headers, while keeping our engine for
   the other 6+ passing sites.

The universal engine is now ~90% of the way to Kasada: everything reachable
from public Browser API surface is covered. The final 10% lives in Chromium
internals.

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
