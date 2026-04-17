# Blocker Debugging Handoff — Session Update (2026-04-17 PM)

This supersedes the AM handoff. Rigorous suite diagnostics uncovered three latent
engine bugs affecting any site with forms or cookie-based challenges. Ten
universal primitives landed. Yandex went from stuck to full page load.

---

## 1. Current Progress Snapshot

| Site | Before session | After session |
|---|---|---|
| Adidas (Akamai BMP) | WIN | WIN |
| HomeDepot (Akamai BMP) | WIN | WIN |
| Wildberries (WBAAS) | WIN | WIN |
| DNS-Shop (QRATOR) | WIN | WIN |
| Ozon (DDoS-Guard) | WIN | WIN |
| **Yandex (ya.ru)** | **Intr 2273b** | **488,914b real homepage** — title "Яндекс — быстрый поиск в интернете" |
| Canada Goose (Kasada) | Intr 732b | Intr 732b (external followup — Task #7) |
| Hyatt (Kasada) | Intr 686b | Intr 737b (same Kasada issue) |

---

## 2. Three Latent Bugs Found and Fixed

### A. h1 POST fallback stripped URL query strings
**File:** `crates/net/src/lib.rs:559`

Before:
```rust
let path = parsed.path().to_string();
h1_client::send_post(&mut tls_stream, host, &path, &hdrs, body).await?
```

After:
```rust
let path = match parsed.query() {
    Some(q) => format!("{}?{}", parsed.path(), q),
    None => parsed.path().to_string(),
};
```

**Impact:** Any HTTP/1.1 POST with a query string lost it. GET had this right
(line 320-325) but POST didn't. Yandex `POST /install?uuid=...` hit h1 fallback
and got `400 "query must have required property 'uuid'"` — server never saw
the uuid. Post-fix: `200 OK` with seven Set-Cookie headers.

### B. `cleanup_bootstrap.js` wiped `__pendingNavigation` before driver check
**File:** `crates/js_runtime/src/js/cleanup_bootstrap.js`

Synchronous inline scripts (`form.submit()`, `location.href = ...`) set
`__pendingNavigation` during the same tick cleanup runs. Deleting it there
lost the signal before `run_until_idle` and the Rust driver check. Kasada's
flow happened to work because its signal comes from async fetch callbacks
that fire after cleanup, but any synchronous navigation signal was silently
discarded. Removed from the cleanup list; the variable is non-enumerable per
`window_bootstrap.js:394` so it still doesn't leak via `Object.keys`.

### C. `navigate_loop_internal` explicitly cleared `__pendingNavigation` after scripts ran
**File:** `crates/browser/src/page.rs`

The `// Clear any synchronous pending navigation` line ran AFTER
`build_page_with_scripts_and_init` had already executed all inline scripts. If
one of those scripts called `form.submit()` or `location.reload()`, the signal
was nulled out before the driver could honor it. Removed — `__pendingNavigation`
starts undefined on each fresh V8 isolate anyway.

Together, B and C explain why the Yandex SSO page rendered a form, our
instrumentation accepted the submit, yet `__pendingNavigation` was always null
by the time the driver looked.

---

## 3. Universal Primitives Added

### HTMLFormElement / HTMLInputElement IDL property reflection
**File:** `crates/js_runtime/src/js/dom_bootstrap.js`

`_reflectStr` and `_reflectBool` helpers now wire the spec-mandated
property↔attribute reflection for:

- `HTMLInputElement`: `name`, `value`, `type`, `placeholder`, `checked`,
  `disabled`, `readOnly`, `required`
- `HTMLFormElement`: `action`, `method`, `enctype`, `target`, `name`,
  `noValidate`

Before this, scripts that did `form.action = 'https://…'` or
`input.value = 'x'` just stuck a JS own-property on the element, and our
`submit()` serializer (which read via `getAttribute`) saw empty values.
Unblocks every programmatically-constructed form.

### Form-POST header enrichment
**File:** `crates/browser/src/page.rs` `navigate_loop_internal`

Form POSTs now carry `Content-Type: application/x-www-form-urlencoded`,
`Origin`, and `Referer`. Without `Content-Type`, the server couldn't parse
our body — yet another reason the Yandex install POST was failing.

### Bounded poll-loop for deferred navigation signals
**File:** `crates/browser/src/page.rs`

Replaces the previous hard-coded 2-second wait on anti-bot pages with a
bounded poll that checks `__pendingNavigation` every 200ms for up to 10s,
exiting early on first hit. Works for PoW flows, auto-submitted forms, and
meta-refresh.

### Post-settle cookie-delta retry
**File:** `crates/browser/src/page.rs`

After `run_until_idle` drains and no `__pendingNavigation` is set, if the page
still looks like a challenge AND the cookie jar gained new cookies during the
iteration, issue one more GET of the same URL. This is the "user pressed F5
after the challenge solved" primitive and covers engines whose solver deposits
a session cookie but relies on user-initiated reload (as Kasada's ips.js
does — 519KB of solver code with zero `location` references).

### Debug-gated iteration logging
`BOXIDE_DEBUG_NAV=1` prints iter / url / html_len and per-fetch method+URL
for every iteration. Cheap, zero-overhead when unset.

### Dead-code cleanup
`navigate_with_init` and `navigate_loop_internal` lost stale
`current_method` / `current_body` locals from an abandoned POST-retry refactor.

---

## 4. Remaining Blocker: Kasada (Canada Goose, Hyatt)

Fully diagnosed but needs external evidence to fix.

**What works:**
- TLS / H2 / headers match Chrome 146 capture
- Solver runs end-to-end: `POST /…/tl` returns `200` with `x-kpsdk-cr: true`
  (challenge accepted) and fresh `x-kpsdk-ct`
- Cookie sync works — `document.cookie` at iteration exit contains
  `akm_bmfp_b2=…` matching the x-kpsdk-ct from the solver response
- Our post-settle retry fires on every iteration carrying the fresh cookie

**What fails:**
- Server still returns the 732-byte challenge on the retry GET, even with the
  just-issued session cookie. Each iteration gets a new cookie; the server
  never upgrades us to the real page.

**Candidate remaining factors (need real-Chrome side-by-side to confirm):**
- TLS session-ticket pinning across the post-settle retry
- H2 connection-id binding
- `sec-fetch-user`/`sec-fetch-site` nuance between JS-initiated reload vs
  user-initiated F5 (we currently send "none" + user=?1 — matches a fresh nav,
  not a reload)

**Concrete next step:** Take a real Chrome 130 HAR capture against
canadagoose.com, diff it byte-for-byte against our request stream on the
post-solve GET. Don't speculate further without evidence.

---

## 5. Relevant Files for Next Developer

- `crates/browser/src/page.rs` — navigate loop + post-settle retry
- `crates/js_runtime/src/js/dom_bootstrap.js` — form/input reflection,
  HTMLFormElement.submit
- `crates/js_runtime/src/js/window_bootstrap.js` — location.* instrumentation
- `crates/js_runtime/src/js/cleanup_bootstrap.js` — internals scrub list
- `crates/net/src/lib.rs` — HttpClient POST/GET paths
- `crates/browser/tests/tier0_kasada.rs` — diagnostic harness
  (`kasada_canadagoose_cookie_and_fetch_diagnostic`,
  `yandex_sso_form_submit_diagnostic`, `yandex_sso_install_post_dump`)
