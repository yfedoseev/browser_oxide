# Wildberries (WBAAS) — Reverse-Engineering Notes

A captured play-by-play of making browser_oxide pass Wildberries'
anti-bot system (WBAAS — Wildberries Anti-Abuse Service). These notes
exist because WBAAS touches almost every subsystem in a from-scratch
browser and surfaced a lot of bugs that aren't visible on simpler sites.

**Status (2026-04-10):** browser_oxide's JS runtime now runs the WBAAS
module end-to-end, executes the solver's `fetch()` POSTs successfully,
receives `x_wbaas_token` via `Set-Cookie`, and stores it in the cookie
jar. The final gap is the reload request being re-challenged by WB
despite sending a valid token. Hypotheses and a re-test plan are below.

---

## Plan execution results (2026-04-10)

A 5-step plan was executed to close the WB retry-GET gap. All steps
shipped; none individually solved the gap, but each narrowed it
substantially. New evidence was collected that points the next
iteration at browser fingerprinting (not network or headers).

### What shipped

| Step | Change | Status |
|---|---|---|
| 0 | Grep WB solver for custom headers (diagnostic) | ✓ Done — ruled out solver-header replay as a fix path |
| 1 | High-entropy Client Hints in `chrome_headers` | ✓ Shipped (`crates/net/src/headers.rs`); 5 new unit tests |
| 2 | `navigator.userAgentData.getHighEntropyValues()` | ✓ Shipped (`window_bootstrap.js`); 7 new chrome_compat tests |
| 3 | Cookie-write ordering audit | ✓ Done — ordering is correct (async ops drain in `run_until_idle`) |
| 4 | HPACK / H2 SETTINGS audit | ✓ Done — already Chrome-130-matched in `h2_client.rs:40-65` |
| 5 | Reload-shape headers on retry (Referer + `sec-fetch-site: same-origin`) | ✓ Shipped (`page.rs::navigate_with_challenges`) |

Unit tests: 333/333. chrome_compat: 216/216. Zero regressions.

### What we learned (new evidence)

The WBAAS response headers on the challenge page expose an internal
status code that varies as we change our request shape:

| Our request shape | `status-no-id` | `x-wbaas-token` | Result |
|---|---|---|---|
| Chrome_headers, no cookie (cold first request) | `PG-03-DL` | `get` | 498 |
| Chrome_headers + token cookie | `PG-03-XC` | `get` | 498 |
| Chrome_headers + token + reload shape (Referer + `sec-fetch-site: same-origin`) | `PG-03-NB` | `get` | 498 |

The fact that the code changes proves we're reaching different code
paths on WBAAS's server — our changes are being observed. But none of
them yet resolves to "validated, serve the real page".

**Critical insight about the two-validator architecture**: the solver's
POSTs to `/__wbaas/challenges/antibot/api/v1/create-token` all return
200 even when sending the same `x_wbaas_token` the page gate rejects.
So **WBAAS has two token validators** — an API gate that accepts any
well-formed token, and a page gate that has stricter rules. We are
passing the first and failing the second.

**New lead for next session**: during the solver run, we observed
these two additional fetches we hadn't seen before:
- `/__wbaas/challenges/antibot/statics/challenge_solver_v1.0.4.js`
- `/__wbaas/challenges/antibot/statics/challenge_fingerprint_v1.0.23.js`

The second one is a **browser-fingerprinting script** that runs as
part of the challenge flow. It almost certainly collects Canvas,
WebGL, Audio, font, and other fingerprints, computes a hash, and
includes it in the `create-token` POST body. If browser_oxide's
fingerprints don't match what a real Chrome would compute, WBAAS
may be issuing a "valid but marked-suspicious" token that the API
gate accepts but the page gate rejects. **This aligns with GAPS.md
P0 items 3, 6, 7** (audio fingerprint params, canvas fonts, WebGL
extensions per-GPU) — the same gaps we deferred earlier.

### Ruled in / out

**Ruled OUT as the blocker** (for WB specifically — still valuable
for other sites):
- Missing headers on retry: solver sends no custom headers on reload.
- Cookie jar losing the token: verified the full token is in the
  outgoing `Cookie` header.
- Cookie write ordering race: the async drain works correctly.
- Client Hints: WB's 498 response has **no `Accept-CH`**.
- H2 HPACK / SETTINGS drift: already Chrome-130-matched.
- JS-side errors: empty throughout.

**Ruled IN as likely blocker** (next iteration):
- Browser fingerprint mismatch — audio/canvas/WebGL values included
  in `challenge_fingerprint_v1.0.23.js` don't match real Chrome.
- WB's stricter "page gate" validator uses the fingerprint hash
  embedded in the token's JSON payload (the part we haven't decoded).

### Next investigation (for a future session)

1. **Fetch `challenge_fingerprint_v1.0.23.js`** and grep for canvas /
   WebGL / audio / font APIs to see exactly what it collects.
2. **Dump the `create-token` POST body** to see the encoded
   fingerprint. Cross-reference the fields against our stealth
   profile and GAPS.md P0 items.
3. **Fix GAPS.md P0 item 3** (audio fingerprint params) first —
   smallest change, highest-leverage if the token includes an audio
   hash.
4. **Run a Chrome capture** against WB from the same machine and
   diff the ENTIRE request (headers + body) against ours, not just
   the obvious differences.
5. **Alternative strategic path**: hit the public JSON APIs
   (`search.wb.ru`, `card.wb.ru`) directly via `net::HttpClient` —
   they have looser TLS/H2 gating and may not require the full
   browser challenge at all.

---

## ⚠ Unsolved

**The reload GET is re-challenged by WBAAS even though we send a valid
`x_wbaas_token`.** Everything up to and including the solver POST being
accepted works. What doesn't work is WB treating the next `GET /` as
"authenticated" when we present the cookie we just received. Chrome on
the same machine and the same IP works, so the gate is request-shape,
not network-level.

| What | Status |
|---|---|
| TLS handshake to `www.wildberries.ru` | ✓ works |
| Initial `GET /` → 498 challenge | ✓ works |
| Fetch + parse + execute the module solver | ✓ works |
| Solver's `POST /__wbaas/.../api/v1` → 200 | ✓ works |
| `Set-Cookie: x_wbaas_token` stored in jar | ✓ works |
| Retry `GET /` sends the token | ✓ verified in logs |
| Retry `GET /` returns real homepage | ✗ **returns 498 again** |

See **§5 "The remaining gap"** below for the full hypothesis list,
what we tried that didn't help, and the recommended investigation
order for the next session.

**Do not hammer WB while debugging this.** Failed attempts trigger a
TLS-layer rate limiter ("TLS handshake unexpected EOF" when trying to
connect) on the order of several minutes per cycle. Single clean runs,
not loops.

---

## 1. What WBAAS is

Wildberries' edge antibot. Public name: **WBAAS** (Wildberries
Anti-Abuse Service). In-house system, no public reverse-engineering
writeups. The clearest external reference is a Habr Q&A
(https://qna.habr.com/q/1402866) that confirms the same 498-status
challenge flow on `card.wb.ru` and pins the primary discrimination vector
as **TLS-stack fingerprinting** rather than headers or JS — `urllib +
pyOpenSSL` passes, plain `requests` fails with identical headers.

**Implication:** much of WBAAS's decision happens before any JS runs.
browser_oxide's `net::HttpClient` (boring2 + custom chrome_headers) is
already good enough to avoid the TLS-fingerprint gate on WB's subdomains
and to receive the JS challenge on the HTML path. What we had to fix was
the JS runtime side.

## 2. The WBAAS flow

The initial `GET https://www.wildberries.ru/` returns **HTTP 498** with a
~1.4 KB HTML shell:

```html
<!DOCTYPE html>
<html data-theme="light"
      data-req-uuid="da1d96092c6c4d09038f6170b041dd3d"
      data-req-ip="2001:569:728c:f600:216:3eff:feef:8cf3">
<head>
  <meta http-equiv="refresh" content="60">
  <script>window.LOAD_START=Date.now()</script>
  <title>Почти готово...</title>
  <script type="module" crossorigin
          src="/__wbaas/challenges/antibot/__static/v1/index-DQJ0L4Mq.js"></script>
  <link rel="stylesheet" crossorigin href="/__wbaas/...">
</head>
<body>...</body>
</html>
```

Things to note:
- The shell embeds `data-req-uuid` and `data-req-ip` on `<html>` — the
  challenge state ID the server has for this client.
- The solver is a `type="module"` script, served from
  `/__wbaas/challenges/antibot/__static/v1/index-<hash>.js`
  (~70 KB Vite bundle).
- The `<meta http-equiv="refresh" content="60">` means the page will
  auto-reload after 60 s if the solver doesn't redirect earlier.
- Title is always `"Почти готово..."` ("Almost ready...") on the
  challenge shell. **Our tests key on this title as the failure signal.**

### 2.1 The solver module

- Despite `type="module"`, the bundle has **no `import`/`export`/
  `import.meta`/dynamic `import()`**. It's a pure IIFE — runs fine under
  our classic-script `execute_script` path. That's a happy accident we
  lean on (we don't have a real `JsRuntime::load_main_es_module` wired
  up yet).
- The bundle starts with Vite's module-preload polyfill (`document
  .createElement("link").relList` etc.), then helper functions, then the
  main `document.addEventListener("DOMContentLoaded", ...)` handler that
  drives the solver.
- The solver expects to register a function on `window` via a `Symbol`
  key derived from the script filename. It polls `window[symbol]` until
  the dynamically-loaded inner script registers itself, then calls it.

### 2.2 The network conversation the solver makes

What we observed over the wire once the solver actually ran:

```
POST /__wbaas/challenges/antibot/api/v1        → 200 OK   (first puzzle)
GET  /__wbaas/challenges/antibot/static        → 200 OK   (fetch some asset)
POST /__wbaas/challenges/antibot/api/v1        → 498      (wrong / retry)
GET  /__wbaas/challenges/antibot/static        → 200 OK
POST /__wbaas/challenges/antibot/api/v1        → 200 OK   (accepted — Set-Cookie: x_wbaas_token=...)
```

Three POSTs, two GETs. The 498 in the middle is WB asking for another
attempt, not an error — the solver catches it and tries again. The final
200 includes `Set-Cookie: x_wbaas_token=<token>`.

### 2.3 The token format

The cookie value decodes to a pipe-delimited record embedded in base64:

```
x_wbaas_token=
  1                                        <- version
  .1000                                    <- ?
  .af36c00a51044471a925e9f0c07b3603        <- nonce / signature prefix
  .MTAw...(base64)...                      <- payload
```

The base64 payload decodes to:

```
100|<client-IPv6>|<full-User-Agent>|<unix-timestamp>|reusable|2|<base64 JSON>
```

So the token is **signed against `{attempt_count, client IP, exact UA,
issue timestamp}`** — plus a nested JSON blob we don't decode. Which
means: the retry request that uses this token **must** present the exact
same IP and UA (verbatim, including case and spacing). browser_oxide's
profile-driven UA is stable across the solver POST and the retry GET
(both go through `chrome_headers(&profile)`), so that invariant holds.

`reusable|2` — hypothesis — is `type=reusable` with a usage counter `2`.
Not confirmed.

## 3. Bugs we found and fixed in browser_oxide

These are all committed in this session. They are listed roughly in the
order we uncovered them — each was blocking the next.

### 3.1 `op_fetch` dropped caller-supplied headers

`crates/js_runtime/src/extensions/fetch_ext.rs`

`op_fetch(url, method, body)` had no `headers` parameter at all.
`fetch_bootstrap.js` read `init.method` and `init.body` but silently
discarded `init.headers`. Every JS `fetch()` with custom headers lost
`Content-Type`, `X-Requested-With`, `Sec-Fetch-*`, etc.

Fix: extend the op to `op_fetch(url, method, headers: HashMap, body)`,
forward headers through `net::HttpClient::get_with_headers` /
`post_with_headers`, and auto-default `Content-Type: text/plain;charset
=UTF-8` for POSTs with a body (mirrors Chrome's fetch default).

### 3.2 `HttpClient::post` never stored `Set-Cookie`

`crates/net/src/lib.rs`

`get()` copied Set-Cookie into the jar. `post()` did not. The WBAAS
solver's POST response is exactly where `x_wbaas_token` is issued, so
the token was being dropped on the floor.

Fix: extract a `store_set_cookies` helper and call it from both `get`
and `post`. Also take case-insensitive "set-cookie" matching instead of
lowercase-only.

### 3.3 Multi-value `Set-Cookie` collapsed in the response

`crates/net/src/lib.rs`

`Response.headers` was a `HashMap<String, String>`. A response with
three `Set-Cookie:` headers (normal for cookie-heavy sites) collapsed to
one — only the last one survived. We'd see exactly one cookie stored
when the server actually set three.

Fix: add `Response.set_cookies: Vec<String>`. During `build_response`,
split `set-cookie` entries out of the header map into the new vector.
Same treatment for the HTTP/1.1 path and `h3_request.rs`.
`store_set_cookies` now reads from `Vec<String>`, not the HashMap.

### 3.4 `document.cookie` was a disconnected JS-only store

`crates/js_runtime/src/js/dom_bootstrap.js`

`document.cookie` read/wrote a `globalThis.__jsCookies` object that had
zero connection to `net::cookies`. So when a `Set-Cookie` arrived via
the network response it was stored in the jar but `document.cookie` saw
nothing — and when JS wrote `document.cookie = "x=y"` nothing ever
reached the jar.

Fix:
- Add two new async ops `op_cookie_get(url)` and `op_cookie_set(url, raw)`
  in `fetch_ext.rs` that proxy to `HttpClient::cookies_for_url` /
  `set_cookie_str` on the shared `FETCH_CLIENT`.
- `document.cookie` setter writes to the local `__jsCookies` mirror AND
  fire-and-forgets `op_cookie_set` so the change propagates.
- `fetch_bootstrap.js` exposes `__syncCookiesFromNet(url)` that pulls the
  origin's jar snapshot back into `__jsCookies`. Call it after every
  `fetch()` response (sync'd before we return the Response), so any JS
  that reads `document.cookie` right after a fetch sees what the server
  set.

### 3.5 Inline scripts appended via `createElement` never ran

`crates/js_runtime/src/js/dom_bootstrap.js`

`Node.prototype.appendChild` only handled `<script src="...">` — dynamic
script loading via `document.head.appendChild(script)`. Inline scripts
(`script.textContent = "..."; document.head.appendChild(script)`) were
silently ignored. Real Chrome executes those synchronously on insertion.

Fix: in the appendChild branch, when the child is a `<script>` with no
`src` and non-empty `textContent`, synchronously `(0, eval)(code)` it
before returning.

Not the WB-blocker (WB uses `.src`) but a correctness fix that unblocks
other antibot solvers (Cloudflare / DataDome / Akamai all use inline
injected scripts at some stage).

### 3.6 `document.location` was undefined, crashing `location.reload()`

`crates/js_runtime/src/js/dom_bootstrap.js`

Real browsers: `document.location === window.location`. Ours had
`globalThis.location` (a Proxy with `.reload`) but no `document.location`
property. The WBAAS solver's final step is `document.location.reload()`
to refresh the page with the validated `x_wbaas_token`. We threw:

```
TypeError: Cannot read properties of undefined (reading 'reload')
```

— which propagated out of `run_until_idle` and killed navigation after
the solver had already succeeded.

Fix: add `Document.prototype.location` getter/setter that aliases
`globalThis.location`.

### 3.7 `window.top`, `window.parent`, `window.frames` missing

`crates/js_runtime/src/js/window_bootstrap.js`

Only `globalThis.self` was defined. Any solver code that touches
`window.parent.location`, `window.top.location`, or `window.frames[...]`
crashed. Real browsers: when a page is not framed, `top === parent ===
window`.

Fix: `globalThis.top = globalThis.parent = globalThis.frames = globalThis;
globalThis.opener = null;`. Also added `location.ancestorOrigins`
returning an empty list (CreepJS-class probe).

### 3.8 JS errors in the challenge path aborted navigation

`crates/browser/src/page.rs`

`build_page_with_scripts` used `run_until_idle(Duration::from_secs(30))
.await?` — propagating any uncaught JS exception. But a solver that
throws 1 ms before `location.reload()` shouldn't kill the whole
navigation, because by then the cookie jar already has the token.

Fix: `if let Err(e) = run_until_idle(...) { eprintln!(...); }` —
non-fatal, continue.

### 3.9 Stale H2 connection after the solver

`crates/net/src/lib.rs` and `pool.rs`

When the solver POSTs completed, WB sometimes sent GOAWAY(NO_ERROR)
on the H2 session. Our next retry GET got:
`HTTP/2 not ready: connection error received: not a result of an error`.

Fix:
- Add `ConnectionPool::evict(host, port)` — explicit eviction.
- In `get_with_headers` / `post_with_headers`, wrap the h2 `send_*` call
  in a 2-iteration loop. On the first attempt, if the error looks like
  a stale-connection signal (`is_stale_conn_error`), evict from the
  pool and retry once with a fresh handshake.
- Expose `HttpClient::evict_connection(host, port)` for page.rs.

`is_stale_conn_error` matches:
- `"not a result of an error"` (h2 GOAWAY/NO_ERROR wording)
- `"broken pipe"`
- `"connection closed"`
- `"ResetStream"` / `"stream was reset"`
- `"HTTP/2 not ready"`

### 3.10 Polling loop was reading `document.cookie` on a disconnected store

`crates/browser/src/page.rs`

The old `navigate_with_challenges` polled `page.evaluate("document.cookie")`
waiting for cookies to appear. Combined with bug 3.4, that poll could
never see anything the server actually set. Also `evaluate_async("void
0", 2s)` returns immediately once the event loop has no pending work, so
the "poll for 2 s" was effectively "poll for 0 ms" — the whole wait
collapsed to ~100 ms.

Fix: delete the polling loop entirely. `build_page_with_scripts` already
runs the event loop for 30 s, during which the WBAAS solver completes
all its POSTs. After that, we directly check the `net::cookies` jar via
`client.cookies_for_url(url)` to see whether the cookie string changed
from the pre-GET snapshot.

## 4. The solver diagnostic sequence

Once all the fixes above were in place, the test logged exactly this on
a clean run:

```
Challenge detected (attempt 1/2, status 498, body 1447b), solving...
[fetch] POST https://www.wildberries.ru/__wbaas/challenges/antibot/api/v1 → 200
[fetch] GET  https://www.wildberries.ru/__wbaas/challenges/antibot/static → 200
[fetch] POST https://www.wildberries.ru/__wbaas/challenges/antibot/api/v1 → 498
[fetch] GET  https://www.wildberries.ru/__wbaas/challenges/antibot/static → 200
[fetch] POST https://www.wildberries.ru/__wbaas/challenges/antibot/api/v1 → 200
  Challenge solved — cookies updated (0 -> 436 chars)
  post cookies: cookietest=; x_wbaas_token=1.1000.af36c00a...
```

Every step of that sequence represents a bug we fixed.

## 5. The remaining gap — retry GET is re-challenged

After the solver succeeds and `x_wbaas_token` is in the jar, our retry
GET of `/` **sends the cookie** (verified — we log the pre-cookies on the
next attempt and see the full token) but the server responds with a
**brand-new 498 challenge** (new UUID, new challenge state).

We verified the user can load `https://www.wildberries.ru` in a real
Chrome on the same machine and the retry works. So our TLS fingerprint
is not blocked IP-wide; something about our reload request shape
differs from Chrome's.

Things we tried that did NOT help:
- Forcing a fresh TLS connection on retry via `client.evict_connection(...)`
- Adding `Referer: https://www.wildberries.ru/` on the retry
- Overriding `sec-fetch-site: same-origin` (broke things — triggered
  `connection closed before headers`, so probably disturbed the
  chrome_headers order that WB JA3-validates)
- 500 ms delay between solver POST and retry GET

Hypotheses still worth testing (next session, from a cold cache):

1. **HTTP/2 SETTINGS / Akamai fingerprint drift.** Our
   `chrome_connector` might produce a slightly-wrong ClientHello or
   H2 SETTINGS order. Verify by fetching `https://tls.peet.ws/api/all`
   and `https://browserleaks.com/http2` and diffing against a real
   Chrome 131 run. If our JA4 or Akamai H2 fingerprint doesn't match
   Chrome's exactly, WB's post-solve validator could be keying on it.
   Reference: `0x676e67/wreq` (Rust Chrome impersonation successor to
   rquest) has `Emulation::Chrome131..137` — worth diffing what their
   SETTINGS frame looks like vs ours.

2. **Missing high-entropy Client Hints. [STRONG EVIDENCE]** Our
   `chrome_headers` sends `sec-ch-ua`, `sec-ch-ua-mobile`,
   `sec-ch-ua-platform`. Chrome also sends `sec-ch-ua-arch`,
   `sec-ch-ua-bitness`, `sec-ch-ua-model`, `sec-ch-ua-full-version-list`,
   `sec-ch-ua-platform-version`, `sec-ch-ua-wow64` when the server has
   previously responded with `Accept-CH`.

   **Confirmed in a sibling probe (ya.ru, 2026-04-10):** Yandex's first
   response includes exactly this header:
   ```
   accept-ch: Sec-CH-UA-Platform-Version, Sec-CH-UA-Mobile, Sec-CH-UA-Model,
              Sec-CH-UA, Sec-CH-UA-Full-Version-List, Sec-CH-UA-WoW64
   ```
   So at least one major Russian site actively uses Client Hints as a
   bot signal. Very plausible WB does the same on its 498 response.
   Worth checking whether WB's 498 contains an `Accept-CH` header and
   echoing the requested hints on the retry. This is now the
   **highest-priority** hypothesis.

3. **HTTP header case / order** diverging from Chrome on the reload
   path. `chrome_headers()` emits lowercase, which is correct for H2
   (HPACK normalizes case). But the **order** matters — HPACK preserves
   insertion order and some WAFs fingerprint on it. Worth double-
   checking ours against a real Chrome H2 capture on `tls.peet.ws`.

4. **Token binding to the original H2 stream**. The solver POSTs happen
   on our pooled H2 session. When we reload on the same pooled session,
   WB sends GOAWAY. When we reconnect fresh, WB re-challenges. Maybe
   WB's edge wants the reload on the **same** connection without the
   pool eviction — but survives the GOAWAY somehow. Worth tracing
   whether the GOAWAY happens before or after our reload request is
   sent.

5. **The solver leaves additional UI / DOM / fetch work unfinished**.
   Real Chrome runs the solver to completion including DOM updates
   and possibly a second fetch we're not observing because our event
   loop returns as soon as microtasks drain. Increase the event-loop
   timeout from 30 s to 60 s and log fetch activity after the visible
   success POST, in case there's a trailing request that only fires
   after some `setTimeout`.

6. **The `reusable|2` counter**. Hypothesis: `2` is remaining uses.
   Our test burns 3 POSTs per solve (the 498 → 200 loop inside the
   solver). If the server decrements on every POST/validation, the
   counter hits 0 before the reload even happens. The fact that our
   polling loop previously re-ran the solver for a second attempt and
   got a NEW token with `|reusable|2|` doesn't disprove this — WB
   might just issue a fresh counter each time.

### 5.1 Recommended investigation order

Do these **cold** (no WB traffic in the previous ~5 minutes — otherwise
the edge rate-limiter gives you "TLS handshake unexpected EOF" before
you can see anything useful). Run each as a single clean test, don't
loop.

1. **Zero-code diagnostic first (5 minutes).** In
   `crates/browser/src/page.rs::navigate_with_challenges`, after the
   initial `client.get(&current_url).await?` that returns the 498, log
   `resp.headers` (the response headers from the challenge page) and
   specifically look for:
   - `accept-ch` — which Client Hints does WB ask for?
   - `critical-ch` — are any required before the reload?
   - `set-cookie` — is there a challenge-state cookie beyond
     `cookietest`?
   - `server` / `x-*` — anything identifying Akamai / Cloudflare / an
     in-house reverse proxy?

   This single log line probably narrows hypotheses 1-3 to one.

2. **Then hypothesis 2 (Client Hints)** if `accept-ch` was present.
   Add the requested hints to `crates/net/src/headers.rs::chrome_headers`
   gated on whether they were requested. Retest.

3. **Then hypothesis 1 (TLS / H2 fingerprint)**. Run browser_oxide
   against `https://tls.peet.ws/api/all` and `https://browserleaks.com
   /http2`, save the JSON, then run a real Chrome 131 against the same
   endpoints, diff. Any field that differs is a candidate. Particularly
   look at:
   - `ja3`, `ja3_hash`, `ja4`, `ja4_r`
   - `akamai_fingerprint` (H2 SETTINGS + WINDOW_UPDATE + PRIORITY +
     pseudo-header order)
   - Extension order in ClientHello (Chrome randomizes but keeps a
     deterministic pre-randomization seed)

4. **Then hypothesis 5 (trailing solver work)**. Bump
   `run_until_idle(Duration::from_secs(30))` to 60 s in
   `build_page_with_scripts`, and add a one-line log inside the JS
   `fetch` wrapper for every URL the module script hits after its
   "successful" POST. If there's a trailing request we're cutting off,
   that's it.

5. **Hypotheses 4 and 6 are last-resort** — they require deeper
   instrumentation (h2 connection lifecycle tracing, decoding the
   `reusable|2` counter semantics against observed behavior) and
   shouldn't be attempted until 1-3 are ruled out.

### 5.1a WB solver header audit (2026-04-10)

Grep of `benchmarks/wb_challenge.js` for every header the solver sets
during its POST chain. Findings:

- **Solver POST headers** (set in `httpService` constructor at line 2846):
  - `X-Wb-Antibot-Key: <siteKey>` — siteKey is read from
    `document.querySelector("#s-key").dataset.siteKey` (line 3210).
  - `X-Wb-Antibot-SDK-Version: js-front-<platform>/<version>` —
    derived from `E.getPlatform().type` and a compile-time constant.
  - `Content-Type: application/json;charset=UTF-8` for analytics
    reports (line 3124).

  **All three pass through correctly via our `init.headers`-forwarding
  path** in fetch_bootstrap.js → op_fetch → `post_with_headers`. Our
  logs show the solver POSTs hit 200. No missing-header issue for the
  solver flow itself.

- **`X-Wbaas-Token` header** (`const ce = "X-Wbaas-Token"`, line 2656):
  only used on the background `frontend-analytics` POST (line 2725)
  AFTER the token is issued, as an alternate auth path alongside the
  cookie. Not used on the reload. Worth noting because it proves WBAAS
  accepts the token via either cookie OR header — a useful fallback if
  cookies ever stop working for us.

- **Token flow** (contradicts an assumption in §5 hypothesis 4):
  - `createToken()` POST returns the token in the **JSON response
    body**, not via `Set-Cookie`.
  - Solver manually writes `document.cookie = "x_wbaas_token=<val>;
    secure; SameSite=None; max-age=1209600"` (line 3254).
  - Then calls `document.location.reload()` (line 3260).

  So our `store_set_cookies` on the POST response does nothing for the
  token — the token reaches our `net::cookies` jar **only via our
  `document.cookie` setter** (`op_cookie_set` fire-and-forget, drained
  by `run_until_idle`). The "cookies updated (0 -> 436)" we see in the
  logs is from that JS path, not from the POST response. This path
  works correctly.

- **Reload request shape**: the solver calls `document.location.reload()`
  with **zero custom headers**. It's a plain browser reload. Which means
  the WB re-challenge we're hitting is NOT caused by a missing
  `X-*` / custom header on the retry — it must be a Chrome fingerprint
  drift issue (Client Hints / H2 SETTINGS / HPACK / TLS), validating
  hypothesis 1, 2, and 3 in §5 and ruling out any NGENIX-style
  `x-promo-msg` parallel.

**Action items this rules in / out:**
- ✅ Keep Step 1 (Client Hints) as the highest-ROI fix.
- ✅ Keep Step 4 (H2 HPACK + SETTINGS verification).
- ❌ Rule out "solver sets header X that we don't replay on retry" —
  the solver sets no headers on reload.
- ❌ Rule out "token write ordering race" — `store_set_cookies` on POST
  was never the mechanism; the async `op_cookie_set` triggered by JS
  `document.cookie =` is the real one, and it drains correctly in
  `run_until_idle`.

### 5.2 Known good + known bad signals (so we can trust the diagnosis)

When you resume this, use these as tripwires:

- **Known good**: seeing `[fetch] POST .../api/v1 → 200` **followed by**
  `Challenge solved — cookies updated (0 -> 436 chars)` proves 100% of
  the solver path works. If you don't see both of those, something in
  §3 regressed — don't blame WB, blame our fixes.
- **Known bad**: seeing `Challenge detected (attempt 2/2, ...)` means
  the retry GET was re-challenged. That's the open bug.
- **Very bad**: seeing `TLS handshake failed unexpected EOF` on the
  initial GET means you're rate-limited at the TLS layer and nothing
  you do to the code will help until you wait 5-10 minutes.
- **Confusing**: seeing `connection closed before headers` is usually
  caused by sending headers WB doesn't expect — that's the symptom we
  triggered by force-overriding `sec-fetch-site: same-origin`. If you
  see it, the last change you made to `chrome_headers` or the retry
  header merge is the suspect.

## 6. What we learned about WBAAS

- **It's solvable.** The exact same IP and UA that Chrome uses pass with
  the real Chrome browser, and our V8 runtime can execute the full
  solver module without any faked/mocked calls. The path from "run
  JavaScript" to "have the token in the cookie jar" is fully
  functional in browser_oxide as of this session.

- **The solver is resilient but not clever.** It's a Vite IIFE bundle
  that does maybe 3 round-trips, reads no exotic APIs, doesn't probe
  for navigator.webdriver, doesn't check canvas fingerprints. The
  heavy lifting happens server-side on the submitted POST payload and
  on TLS / H2 fingerprinting. If you get the solver's POST accepted,
  you've done 95% of the work — the rest is making the reload look
  identical to what Chrome sends.

- **The token is not opaque.** You can decode the base64 payload and
  see the exact contract WBAAS thinks it has with the client: version
  | IP | UA | timestamp | type | counter | signed JSON. That's
  valuable for debugging — any mismatch between your retry request
  and those fields is a guaranteed failure.

- **The challenge page is cheap to trigger.** Fresh UUID every time,
  ~1.4 KB shell, ~70 KB solver. Means you can iterate quickly on the
  solver path. But *do* be gentle — hammering the origin with failed
  solves triggers "TLS handshake unexpected EOF" pretty fast, which is
  a rate limiter at the TLS layer, not an IP ban. Wait a few minutes
  between debugging cycles.

## 7. What this session proved about browser_oxide

Everything in §3 was a latent bug that any full site with a dynamic
script + cookie handshake would have hit eventually. They're not
WB-specific. The WB challenge just exercised the whole surface in one
flow. After this session:

- `fetch(url, {method, headers, body})` correctly forwards headers, auto-
  defaults Content-Type, and sync-back of Set-Cookie into
  `document.cookie`.
- `document.cookie` is a real view into `net::cookies`, not a separate
  store.
- Multi-value `Set-Cookie` responses no longer silently collapse.
- Inline and external dynamic scripts both execute on `appendChild`.
- `document.location`, `window.top`, `window.parent`, `window.frames`,
  `window.opener`, `location.ancestorOrigins` — all present.
- `document.location.reload()` is a no-op that doesn't throw.
- JS errors in solvers don't abort navigation.
- HTTP/2 stale-connection errors auto-recover via a pool-evict + retry
  on the first attempt.

328/328 workspace unit tests pass. 209/209 `chrome_compat` tests pass.

## 8. Files touched in this session

| File | Change |
|---|---|
| `crates/net/src/lib.rs` | `Response.set_cookies` vec, `get_with_headers`, `post_with_headers`, `store_set_cookies` helper, `cookies_for_url`, `set_cookie_str`, `evict_connection`, stale-H2 auto-retry, `has_header` + `merge_headers` helpers, `is_stale_conn_error` |
| `crates/net/src/pool.rs` | `ConnectionPool::evict` |
| `crates/net/src/h3_request.rs` | Split `set-cookie` out of headers map |
| `crates/js_runtime/src/extensions/fetch_ext.rs` | `op_fetch` takes `headers` arg, new ops `op_cookie_get` / `op_cookie_set` |
| `crates/js_runtime/src/js/fetch_bootstrap.js` | Forwards `init.headers`, default `Content-Type`, `__syncCookiesFromNet` helper, syncs after every fetch response |
| `crates/js_runtime/src/js/dom_bootstrap.js` | `document.cookie` talks to ops + local mirror, `document.location` getter, inline script exec in `appendChild` |
| `crates/js_runtime/src/js/window_bootstrap.js` | `top`, `parent`, `frames`, `opener`, `location.ancestorOrigins` |
| `crates/browser/src/page.rs` | `navigate_with_challenges` simplified (no more bad polling loop), non-fatal `run_until_idle` |

## 9. How to reproduce / re-test

```bash
# From a cold network state (haven't hit wildberries.ru in 10 min):
cargo test -p browser --test e2e_browser challenge_wildberries \
    -- --ignored --test-threads=1 --nocapture
```

Expected log today:
```
Challenge detected (attempt 1/2, status 498, body 1447b), solving...
[fetch] POST .../api/v1 → 200
[fetch] GET  .../static → 200
[fetch] POST .../api/v1 → 498
[fetch] GET  .../static → 200
[fetch] POST .../api/v1 → 200
  Challenge solved — cookies updated (0 -> 436 chars)
Challenge detected (attempt 2/2, status 498, body 1447b), solving...
...
Challenge not solved, still on challenge page: Почти готово...
```

Test target when this is fully fixed:
```
Challenge detected (attempt 1/2, status 498, body 1447b), solving...
[fetch] POST .../api/v1 → 200
...
  Challenge solved — cookies updated (0 -> 436 chars)
[wildberries] title: Интернет-магазин Wildberries: широкий ассортимент...
[wildberries] body: >100000 bytes
```

(The real WB homepage is ~800 KB of rendered HTML.)
