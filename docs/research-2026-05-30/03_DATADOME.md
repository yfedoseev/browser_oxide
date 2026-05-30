# 03 — DataDome cluster: etsy-CHL (+ reuters/wsj/tripadvisor on firefox)

**Date:** 2026-05-30
**Cluster:** DataDome
**Ground truth:** camoufox v150 PASSES; browser_oxide FAILS.
- `etsy` — v150 = 248 KB, BO = `DataDome-CHL` **1.4 KB** (interstitial never cleared)
- `reuters` (ch,ip), `wsj` (ch,px,ip), `tripadvisor` (px,ip) — all FAIL **firefox** profile specifically; these are DataDome/edge tenants that weight TLS class.

**TL;DR root cause = TWO distinct, both-real failure modes, neither is the cookie-jar:**
1. **etsy (all profiles):** the DataDome interstitial's daily-rotated **WASM challenge never produces a valid `datadome=` token** because the actual solver lives in `vendor_solvers` (out of public scope) and `Page::navigate` registers an **empty solver set**. The public engine has all the *plumbing* (CSP relax, iframe materialization, 90 s poll, cookie-diff retry, `is_datadome_solved` break) but **no token producer** → the body never transitions off `captcha-delivery.com` → `is_datadome_solved` is never true → 1.4 KB interstitial is what `content()` serializes.
2. **firefox-only losers (reuters/wsj/tripadvisor):** BO's firefox profile emits a **Chrome 147 ClientHello + Chrome H2 fingerprint under a Firefox UA + Firefox headers** — an internally contradictory identity. DataDome's per-tenant ML weights `Chromium-TLS = bot`, `Firefox-TLS = human`; a JA4↔UA cross-check sees `Firefox/135` UA + Chrome-class JA4 and buckets us high-risk. **Firefox-NSS TLS is the only documented DataDome bypass** and BO has no Firefox wire arm.

**The cookie-jar child-iframe item raised in the task is NOT the bug** — verified below. The jar is correctly shared into the iframe fetch path and into the iframe's `op_fetch`/`document.cookie` writes (thread-local FETCH_CLIENT). The gap is the missing token producer, not jar isolation.

---

## 1. The DataDome live-nav path in BO (what actually runs)

### 1a. Detection + arming
`is_datadome_challenge(html)` (`crates/browser/src/page.rs:208`):
```rust
fn is_datadome_challenge(html: &str) -> bool {
    html.len() < 50_000 && html.contains("captcha-delivery.com")
}
```
This fires `started_as_dd_challenge` and arms three public primitives even with **zero registered solvers** (the empty-solver design, per `CLAUDE.md`):
- **CSP relax** (`page.rs:1830-1836`, `:1881-1887`): `is_datadome_challenge(&html)` relaxes the response CSP so the interstitial can load `geo.captcha-delivery.com` scripts that the origin's own `frame-src`/`script-src` would otherwise block.
- **iframe materialization** (`page.rs:2270-2272`): `rematerialize_iframes` runs each poll tick.
- **cookie-diff retry / poll break** (`page.rs:2285-2310`): breaks the 90 s poll on `is_datadome_solved`.

### 1b. The 90 s challenge poll
`page.rs:2244-2311`: when `started_as_dd_challenge`, BO loops up to **90 s**, every 200 ms:
1. `run_until_idle(200ms)` — drains the interstitial's JS event loop.
2. `rematerialize_iframes(&current_url, &client, &profile)` — fetches+executes any script-injected `geo.captcha-delivery.com` iframe.
3. Reads `__pendingNavigation`; if set, breaks to re-issue.
4. Reads `client.cookies_for_url(p)` + `page.content()` and breaks iff `is_datadome_solved(now, body)`:
```rust
fn is_datadome_solved(cookies: &str, body: &str) -> bool {
    cookies.contains("datadome=") && !is_datadome_challenge(body)   // page.rs:221-223
}
```

### 1c. Why this never completes for etsy
`is_datadome_solved` requires the body to **stop being** a `captcha-delivery.com` interstitial. That transition only happens when DataDome's edge accepts a **valid PoW/WASM-signed `datadome=` token** and serves the real page. Producing that token requires running the **daily-rotated WASM** challenge (`tags.js` v5.6.3 + the `i.js`/`captcha-delivery.com` bundle) to completion and POSTing the signed payload to `https://api-js.datadome.co/js/`. That signing is the `DataDomeSolver`/`DdEncryptor` work which:
- is **byte-verified but has ZERO callers** (`datadome_crypto.rs:371`, per mined context), and
- lives in the **private `vendor_solvers`** crate, which `Page::navigate` does **not** register (`CLAUDE.md`: "`Page::navigate` registers an empty solver set").

So FP-D3 is exactly right: a `datadome=` cookie *is* present on every DD nav (incl. the failing 403), but the body stays on the interstitial → `is_datadome_solved` stays `false` → the poll burns its budget → `content()` serializes the 1.4 KB interstitial. **This is a missing-token-producer gap, deliberately out of public scope — not a fingerprint or jar bug for etsy.**

---

## 2. The cookie-jar / child-iframe item — checked, NOT the bug

The task asked to check "the DataDome child-iframe cookie-jar item (net/lib.rs HttpClient::new fresh-jar vs shared)". I traced every jar handoff on the DD path:

| Fetch site | Client used | Jar | Verdict |
|---|---|---|---|
| Top-level interstitial GET | `HttpClient::shared(&profile)` (`page.rs:1158`, `:1318`, `:1463`) | **shared** process jar (`lib.rs:146-177`, `:363-368`) | ✅ shared |
| Iframe HTML + its scripts/CSS (`geo.captcha-delivery.com`) | passed-in `client` = the same shared client (`page.rs:2271` → `rematerialize_iframes` → `iframe::ChildIframe::from_url(.., client, ..)` `iframe.rs:73-104`, `:152`, `:201`) | **shared** | ✅ shared |
| Iframe JS `fetch()` / XHR (the i.js verification POST) | thread-local `FETCH_CLIENT` (`fetch_ext.rs:284`) — the child runtime runs on the **same thread** (rematerialize is `async` on the main runtime), so it inherits the main page's installed FETCH_CLIENT | **shared** (FETCH_CLIENT was set to the shared client via `set_fetch_client`, `fetch_ext.rs:201`) | ✅ shared |
| Iframe `document.cookie = "datadome=…"` write | `op_cookie_set_sync` → FETCH_CLIENT jar **and** `set_shared_cookie_sync` into the process jar (`lib.rs:191-204`, FIX-COOKIE-SYNC) | **shared** | ✅ shared |
| `op_net_fetch_sync` (sync sub-resource loader) | builds a **fresh client** but via `new_with_shared_state(main.cookies(), …)` (`fetch_ext.rs:516-532`) — shares the jar, fresh pool only to dodge the H2 deadlock | **shared** | ✅ shared |

The cookie jar is per-domain RFC-6265-scoped (`cookies.rs:51-91`); it correctly stores a `Domain=.etsy.com` cookie set from a `api-js.datadome.co` response and serves it back to etsy.com. **There is no fresh-jar isolation leak on the DataDome path.** The two `HttpClient::new(...fresh-jar...)` fallbacks in `fetch_ext.rs:530` and `op_fetch` (`:289-291`) only fire when **no** main client is installed (never during a real nav — the page always installs one). The jar item is a dead end; do not spend effort there.

---

## 3. The firefox-only losers (reuters/wsj/tripadvisor) — the real second gap

These FAIL the **firefox** profile while passing chrome/pixel. The mechanism is purely wire:

- `chrome_connector` (`crates/net/src/tls.rs:239`) branches **only on `profile.device_class`** (Desktop / MobileAndroid / MobileIOS). There is **no Firefox arm** — Desktop = Chrome 147 BoringSSL ClientHello (MLKEM768+X25519, GREASE, Brotli cert-compress, ALPS, ECH-GREASE, ext-shuffle).
- The firefox presets set `device_class: DeviceClass::Desktop` + `tls_impersonate: "firefox_135"` (`presets.rs:473-474`, `:549`, `:622`). The preset's own comment (`presets.rs:466-472`) confirms `tls_impersonate` is **"currently informational only… The actual TLS bytes are emitted by `crates/net` via boring2/BoringSSL with a Chrome-tuned ClientHello."**
- H2 SETTINGS likewise branch only on `device_class` (`h2_client.rs`, per mined context) → Chrome H2 fingerprint.
- Only `headers.rs` emits a real Firefox header set.

**Net identity for a BO firefox nav:** Firefox/135 UA + Firefox headers + **Chrome-class JA4** (`t13d1516h2…` vs Firefox's `t13d1715h2_5b57614c22b0_3d5424432f57`). DataDome (and AWS WAF, Cloudflare) run a JA4↔UA coherence check; the contradiction buckets the request high-risk. Camoufox v150 is a **real Firefox**, so its JA4 is authentic Firefox-NSS — that is *the single reason* it passes these where BO's firefox profile fails. This is also the documented-only DataDome bypass for etsy/tripadvisor (`GAP_DEEP_ANALYSIS_2026_04_28`): "Firefox-NSS TLS is the only DataDome/tripadvisor bypass."

A faithful Firefox wire needs (none present today): boring2 NSS-class reconfig — `record_size_limit (28)`, `delegated_credentials (34)` sigalgs ext, FFDHE2048/3072 in supported_groups, real ECH (not GREASE), **fixed** (non-shuffled) extension order, **and** Firefox H2 SETTINGS (keep SETTINGS 0x3, RFC7540 priority tree, different INITIAL_WINDOW). Tracked internally as "Phase B.3"; never landed.

---

## 4. Incorrectly / incompletely implemented (engine findings)

1. **`tls_impersonate` is a dead string** (`presets.rs:474`, `tls.rs:239`): the field is read **nowhere** in the connector — only `device_class` is. A Firefox profile silently emits Chrome bytes. This is a correctness bug: the profile claims a Firefox wire it cannot deliver. At minimum, the connector should branch on `tls_impersonate`/`browser_name`, or `firefox_135_*` should be feature-gated off until B.3 lands so it doesn't ship a lie.
2. **DataDome solver is byte-verified dead code** (`DdEncryptor`, `datadome_crypto.rs:371`, zero callers): the engine has the crypto but no wiring on the public path. This is by-design per `CLAUDE.md` (token solving = `vendor_solvers`), so for the **public** engine etsy is structurally out of reach without either (a) the private solver, or (b) a way to drive the bundle's own WASM self-solve to a valid token — which it currently does **not** achieve in live nav (the 90 s drain produces a `datadome=` cookie but no body transition).
3. **The 50 KB `is_datadome_challenge` size gate** (`page.rs:209`) means a *partially*-rendered real etsy page that still references `captcha-delivery.com` in a `<script>` would be misclassified as still-challenged. Low risk today (interstitials are 1-5 KB) but brittle.

---

## 5. Fixes, ROI-ranked

### FIX-DD-FF1 — Firefox-NSS TLS arm (HIGH leverage, HIGH effort) — flips reuters/wsj/tripadvisor-firefox + is the only etsy path that doesn't need the private solver
Add a real Firefox branch to `chrome_connector` (`tls.rs:239`) gated on `tls_impersonate.starts_with("firefox")` / `browser_name == "Firefox"`: NSS cipher order, `record_size_limit`, `delegated_credentials`, FFDHE groups, real ECH, fixed extension order; plus a Firefox H2 SETTINGS arm in `h2_client.rs`. This is the documented-only DataDome bypass and fixes every firefox-profile loser at once (the cluster's 3 firefox sites + the wider 14-site firefox cohort). Effort: substantial (boring2 reconfig + a JA4 byte-equivalence test vs a captured real-Firefox ClientHello). **Confidence: high it's necessary; medium it's sufficient alone for etsy (DD also weights behavioral/ML tail).**

### FIX-DD-FF2 — stop shipping a contradictory firefox identity (LOW effort, immediate honesty) — pre-req / de-risk for FF1
Until FF1 lands, make the firefox presets coherent: either (a) feature-gate `firefox_135_*` out of `all_presets()` so a Chrome-wire profile never claims a Firefox UA, or (b) branch the connector on `browser_name` and **error** if a Firefox profile is used without the B.3 wire. Removes the worst-class signal (UA↔JA4 mismatch) from the corpus today. `presets.rs:473`, `tls.rs:239`. **Confidence: high** (correctness), **low** that it alone flips a site (it removes a negative signal, doesn't add a positive one).

### FIX-DD-3 — drive the bundle WASM self-solve to a real token in live nav (MEDIUM effort, public-scope) — the only public-engine etsy path
The 90 s poll already rematerializes the `captcha-delivery.com` iframe and shares the jar; what's missing is that the WASM bundle's async PoW produces **zero forward progress** in the offline-style drain (same class as the AWS-WAF live-nav drain blocker, M-1, noted in the latest handoff). Investigate whether `run_until_idle(200ms)` is starving the blob-worker / `requestIdleCallback` chain the DD bundle uses (cf. the SPA event-loop drain gaps: Worker `setInterval(5)` poll, `requestIdleCallback`→`setTimeout(fn,1)`). If the WASM can be driven to POST a valid token, `is_datadome_solved` flips and etsy renders **without** any vendor solver. `page.rs:2244-2311`, `timer_bootstrap.js`, `window_bootstrap.js`. **Confidence: medium** — depends on whether DD's verification is pure-client PoW (drivable) vs server-ML-gated (not drivable from-scratch).

### FIX-DD-4 — register the byte-verified DataDomeSolver on a feature flag (LOW effort, scope-bounded) — out of public scope by `CLAUDE.md` but the fastest etsy flip if a private build is allowed
`DdEncryptor` is byte-verified (`datadome_crypto.rs:371`) and `is_datadome_solved`/CSP-relax/iframe-materialize plumbing is all wired to consume its output. Wiring it behind `--features vendor_solvers` would flip etsy immediately. Explicitly **declined for the public engine** per `CLAUDE.md`; listed only to record that the gap is "wiring + scope policy," not "missing crypto." **Confidence: high it flips etsy; N/A for public scope.**

### FIX-DD-5 — tighten `is_datadome_challenge` to require interstitial structure (LOW effort) — robustness, no flip
Gate on `rt:'i'` / the `dd` config object shape in addition to the `captcha-delivery.com` substring + 50 KB cap (`page.rs:208`) so a real page mentioning the CDN can't be misclassified as challenged. **Confidence: high** (correctness), **no expected site flip.**

---

## 6. Evidence index (file:line)
- `crates/browser/src/page.rs:208-223` — `is_datadome_challenge` / `is_datadome_solved`
- `crates/browser/src/page.rs:1158,1318,1463` — nav uses `HttpClient::shared` (shared jar)
- `crates/browser/src/page.rs:1830-1836,1881-1887` — CSP relax on `is_datadome_challenge`
- `crates/browser/src/page.rs:2244-2311` — 90 s DD poll + rematerialize + cookie-diff break
- `crates/browser/src/page.rs:778-833` — `rematerialize_iframes` (passes shared `client`)
- `crates/browser/src/iframe.rs:73-104,152,201,232` — `ChildIframe::from_url` uses passed `client`
- `crates/net/src/lib.rs:121-204` — SharedSession, `set_shared_cookie_sync` (FIX-COOKIE-SYNC)
- `crates/net/src/lib.rs:279-337,353-414` — `HttpClient::new` (fresh jar) vs `shared` / `new_with_shared_state`
- `crates/net/src/cookies.rs:51-110` — RFC-6265 domain-scoped jar (handles `Domain=.etsy.com`)
- `crates/js_runtime/src/extensions/fetch_ext.rs:201,284,516-532` — FETCH_CLIENT thread-local; sync-fetch shares jar via `new_with_shared_state`
- `crates/net/src/tls.rs:239-248` — `chrome_connector` branches on `device_class` only; **no Firefox arm**
- `crates/stealth/src/presets.rs:466-474` — firefox preset: `tls_impersonate:"firefox_135"` informational-only; `device_class: Desktop`
- Mined: `datadome_crypto.rs:371` (`DdEncryptor` byte-verified, zero callers); `GAP_DEEP_ANALYSIS_2026_04_28` (Firefox-NSS = only DD bypass).
