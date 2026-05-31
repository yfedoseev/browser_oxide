# CSR diagnosis — wildberries.ru (WBAAS antibot data-fetch/reload gate)

Date: 2026-05-30
Branch: fix/v0.1.0-fix4-canvas-parity
Engine path read: `crates/browser/src/page.rs`, `crates/browser/src/classify.rs`,
`crates/js_runtime/src/js/dom_bootstrap.js`, `crates/js_runtime/src/module_loader.rs`

## TL;DR

This is **NOT** a CSR / render-scheduling / Vue/React-mount blocker, and it is
**NOT** the adidas-style MessageChannel async-delivery bug. There is no Vue/React
app in the 1.4 KB document at all — the document BO (and camoufox) first receives
is a **WBAAS anti-bot challenge shell** served with HTTP **498**. The real 1.57 MB
Wildberries SPA is only served on a *second* request once a valid `x_wbaas_token`
cookie is present.

**Root cause:** the render blocker is a **vendor anti-bot active-fingerprint
data-fetch gate** (Wildberries' own "WBAAS"). To get the real app you must:
`module loads → POST /api/v1/create-token → server replies 498 "challenge is
required" with a fingerprint sub-challenge → SDK fetches+runs
challenge_fingerprint_v1.0.23.js, computes a device fingerprint, re-POSTs
create-token with the solution → server returns {secureToken} → set cookie
x_wbaas_token → document.location.reload() → reload serves the 1.57 MB app.`

BO loads and executes the entry module (it is `<script type="module">`, run via
`eval_module_code`, P2 ES-module path), but it cannot complete this loop, AND the
engine's generic challenge machinery never arms for WBAAS (no marker, no
`started_as_wbaas_challenge` flag, no host budget). So BO returns the ~1.6–2 KB
challenge shell.

This blocker is **vendor-solver territory** per CLAUDE.md (active per-vendor
fingerprint challenge). The *public-engine* part that is in-scope and currently
broken is purely the **plumbing** that would let the WBAAS SDK's own self-solve
run to completion (challenge detection → arm the 90 s pending-nav reload poll →
adequate budget) — exactly the same defensive primitives the engine already
ships for DataDome / sec-cpt / Cloudflare / AWS-WAF, but which were never wired
for WBAAS.

## Evidence

### 1. The document is an antibot shell, not the app
Raw GET of `https://www.wildberries.ru/` returns 1.4 KB:
```
<title>Почти готово...</title>   ("Almost ready...")
<script type="module" crossorigin src="/__wbaas/challenges/antibot/__static/v1/index-DQJ0L4Mq.js">
<link ... href="/__wbaas/challenges/antibot/__static/v1/index-BuoI5IWB.css">
<div id="wait_msg"></div><div id="c_cont"></div>
<b id="s-key" data-site-key="7400bd5df8b843b28254659f10915f31"></b>
```
Response headers (captured by BO):
```
server: wbaas
status-no-id: PG-06-XC
x-wbaas-token: get          <-- server tells the client "go get a token"
content-type: text/html     HTTP 498
```
`__wbaas/challenges/antibot`, `server: wbaas`, `x-wbaas-token`, `data-site-key`,
status 498 = Wildberries' in-house "WBAAS" anti-bot. There is no Vue/React/Vite
*app* bundle here — only a 70 KB antibot SDK module.

### 2. The entry module is the antibot SDK (70 KB), not an app
`/__wbaas/challenges/antibot/__static/v1/index-DQJ0L4Mq.js` (70138 bytes) contains
`ANTI_SDK_WB_START_TIME`, a retry-fetch wrapper, a `challengeSolver`, and a token
service class `le` with `cookieName = "x_wbaas_token"`, header `X-Wbaas-Token`,
endpoints `/api/v1/create-token` and `/api/v1/create-one-time-token`, request
headers `X-Wb-Antibot-Key: <siteKey>` + `X-Wb-Antibot-SDK-Version`. On success it
does:
```js
document.cookie = ("x_wbaas_token", token, {secure:true, SameSite:"None", "max-age":"1209600"}),
document.location.reload()
```
No Worker / WASM / PoW in this module; it is a pure-JS fetch + fingerprint +
cookie + reload flow.

### 3. The create-token call returns an active fingerprint sub-challenge (498)
Direct sequential POST (IP-safe) reproduces the gate:
```
POST /__wbaas/challenges/antibot/api/v1/create-token
  X-Wb-Antibot-Key: 7400bd5df8b843b28254659f10915f31
  X-Wb-Antibot-SDK-Version: 1.0.0
->  HTTP 498
{"code":498,"message":"challenge is required",
 "challenge":{"scriptPath":"/statics/challenge_fingerprint_v1.0.23.js",
              "payload":"<base64 signed token>"}}
```
The base64 `payload` decodes to a signed (JWT-style `body.sig`) token:
```json
{"version":1,"timestamp":1780186308412,"id":"d3913f92-…","ip":"2001:569:…",
 "ua":"Mozilla/5.0 (Macintosh; …) Chrome/148.0.0.0 …","siteKey":"7400bd5df8…",
 "deviceToken":"","userScore":{"hash":""},"action":"reusable","tokenType":"2",
 "fpid":"d3913f92-…","fid":"5f4e7a88-…","pbs":0,"mtds":0,"clls":"dbdc1ab3"}
```
The empty `deviceToken` / `userScore.hash` are what
`challenge_fingerprint_v1.0.23.js` must fill in (device fingerprint + score hash)
before re-POSTing create-token. The fingerprint script path is itself gated behind
the antibot (requesting it returns the same 498 challenge HTML), so the whole
origin is closed until a valid `x_wbaas_token` exists.

### 4. BO loads + runs the module but is killed mid-flight, and never reloads
Live BO run (`sweep_stable chrome_148_macos`, single sequential run):
```
sweep: [1/1] t wildberries L3-RENDERED len=1647 ms=15688
[navigate] Initial challenge response headers (498): server: wbaas; x-wbaas-token: get; status-no-id: PG-06-XC
[vendor-detect] wbaas on https://www.wildberries.ru/
[csp] installed 1 policies from headers=0 meta=1 enforce=true
[navigate] iter=0 url=https://www.wildberries.ru/ html_len=1447
[navigate] iter=0 installing V8DeadlineWatcher with 6567ms remaining
[V8DeadlineWatcher] deadline 6567ms expired — firing terminate_execution
```
There is **no iter=1** — the module never reached `location.reload()`, so the
navigate loop never re-fetched with a token. BO returns the 1647 B challenge shell
(the task's "2033 B shell"). The module IS executing (it is dispatched through the
P2 ES-module path at `page.rs:3690` `eval_module_code`, 10 s bound) — it gets
killed by the per-iteration V8 deadline / runs out of the 15 s default budget
while it is mid create-token retry loop against a server that keeps answering 498.

(Note: the earlier "connection closed before headers / TLS handshake failed
unexpected EOF" was *self-inflicted IP rate-limiting* from rapid repeated probes —
even plain `curl` then returned 498. A single spaced run connects fine. TLS/JA is
NOT the blocker.)

## Why the engine's generic challenge handling does not save it

The engine already has the exact primitives that would let a self-solving antibot
SDK finish: a 90 s **pending-navigation poll** that waits for `location.reload()`
after a challenge (`page.rs:2265-2273`), per-host budget tiers
(`page.rs:1970-2026`), and persistent `started_as_<vendor>_challenge` origin flags
so the poll stays armed even after the SDK mutates the DOM (DataDome, sec-cpt,
Cloudflare, AWS-WAF — `page.rs:1903-1939`). **WBAAS is wired into none of them:**

1. **No challenge marker.** `is_anti_bot_challenge()` →
   `classify::engine_classify()` (`page.rs:185-189`, `classify.rs`) has no WBAAS
   marker (`__wbaas`, `server: wbaas`, `x-wbaas-token`, `data-site-key`,
   `status PG-…`, status 498). The 1447 B shell is classified as a normal tiny
   page, so `is_anti_bot_challenge()` returns **false**.
2. **No `started_as_wbaas_challenge` flag.** Only DD / sec-cpt / CF / AWS-WAF set a
   persistent origin flag (`page.rs:1903-1939`). WBAAS vendor-detect is
   **observability only** — a bare `eprintln!("[vendor-detect] wbaas …")` at
   `page.rs:1218-1220` with no behavioral effect.
3. **Net effect:** the `if pending_info.is_empty() && (is_anti_bot_challenge() ||
   started_as_*_challenge)` guard at `page.rs:2265-2270` is **false**, so the 90 s
   reload poll is never entered. Even if the SDK *did* call reload, the loop would
   need that poll armed to re-fetch with the token.
4. **No host budget tier.** `wildberries.ru` falls to the `_ => 15_000` default
   (`page.rs:2025`). 15 s is too tight for: module fetch+exec + create-token POST +
   fingerprint-script fetch+exec + fingerprint compute + re-POST + reload + drain.
   The watcher fired at 6.5 s, mid-flight.

## Classification

`blocker_class = data-fetch-gate` (vendor anti-bot, WBAAS). Secondary:
`render-scheduling` budget too low. NOT framework-API, NOT MessageChannel/CSR,
NOT geo, NOT TLS.

## Flippable?

**maybe — but the decisive part is vendor-solver scope, not public-engine.** The
real lever is computing a valid WBAAS *device fingerprint* (`deviceToken` +
`userScore.hash` from `challenge_fingerprint_v1.0.23.js`) and re-submitting it.
That is a per-vendor active fingerprint challenge ⇒ `vendor_solvers`, out of scope
for the public engine per CLAUDE.md (same posture as Akamai/Kasada/DataDome).

What IS in public-engine scope is the **plumbing** so the WBAAS SDK's own
self-solve can run to completion under the engine's generic challenge loop —
mirroring the DataDome/sec-cpt/CF/AWS-WAF defensive primitives. Whether that alone
flips the site depends on whether `challenge_fingerprint_v1.0.23.js` runs cleanly
in BO's environment (it needs canvas/webgl/audio/navigator fingerprint surfaces;
unverified because the script is gated behind the same 498 and could not be
fetched from this IP). If the fingerprint script's checks pass in BO, the plumbing
fixes below could flip it without any vendor code; if WBAAS's fingerprint scoring
rejects BO's surfaces, a vendor_solver is required.

## Public-engine fixes (plumbing only — DO NOT add vendor bypass)

These make BO *behave like a browser that lets WBAAS solve itself*, identical in
spirit to the existing DD/sec-cpt/CF/AWS-WAF arms.

1. **Add a WBAAS challenge detector** (`classify.rs` + `page.rs`). Detect the WBAAS
   shell from the *response* signals, not body text: header `x-wbaas-token`
   present, or `server: wbaas`, or status 498 with body containing
   `/__wbaas/challenges/antibot` + `data-site-key`. Mirror `is_awswaf_challenge`
   (`page.rs:1939`) — narrow (len < 4 KB + the `__wbaas` marker) so it can't false
   positive on rendered pages.

2. **Set `started_as_wbaas_challenge` and OR it into the poll guard**
   (`page.rs:1939` neighborhood + the guard at `page.rs:2265-2270`). This arms the
   existing 90 s pending-nav reload poll so when the SDK calls
   `document.location.reload()` after setting `x_wbaas_token`, the navigate loop
   re-fetches with the cookie and gets the real app — exactly the AWS-WAF pattern.

3. **Add a host-budget tier for `wildberries.ru` / `wb.ru`** (`page.rs:1970-2026`).
   The flow is multi-stage (module + create-token + fingerprint script + re-POST +
   reload). Give it the heavy tier (45–60 s), like the Kasada/sec-cpt hosts, so the
   V8DeadlineWatcher and drain budget cover the full self-solve. Today it gets the
   15 s default and is killed at 6.5 s.

4. **Add a WBAAS solved-detector** mirroring `is_datadome_solved` /
   `is_awswaf_solved`: cookie jar contains `x_wbaas_token=` AND body is no longer
   the `__wbaas` shell ⇒ stop iterating and return.

Concrete file:line anchors for the patch:
- detector + `started_as_wbaas_challenge`: `crates/browser/src/page.rs:1939`
  (next to `is_awswaf_challenge`) and marker set in `crates/browser/src/classify.rs`
- arm the poll: `crates/browser/src/page.rs:2265-2270`
- host budget: `crates/browser/src/page.rs:2025` (replace the `_ => 15_000` arm
  with a `wildberries.ru`/`wb.ru` 45_000–60_000 tier)
- solved-detector: alongside `is_datadome_solved` `crates/browser/src/page.rs:238`

**Verification gate (after plumbing):** if BO then reaches `iter=1` with an
`x_wbaas_token` cookie and a body > 50 KB, the fingerprint script runs and passes
in BO — public-engine plumbing is sufficient. If it loops on 498
("challenge is required") forever, the fingerprint scoring rejects BO and the
remainder is `vendor_solvers` scope.
