# Douyin thin-render diagnosis (www.douyin.com)

**Date:** 2026-05-30
**BO result:** `L3-RENDERED len=6327` (thin shell), ms=3687, 0 JS errors
**Verdict:** NOT a CSR/framework gap and NOT an engine stub bug. The blocker is
**ByteDance's `__ac` (acrawler) anti-bot interstitial + a server-side slide-captcha
gate** (`验证码中间页` / `TTGCaptcha`, `subtype:"slide"`, `region:"cn"`). This is a
**data-fetch-gate / challenge** class blocker, not a render-scheduling one.

---

## What actually loads (the 72 KB is NOT the app)

`www.douyin.com/` does **not** serve the SPA on first hit. The 72,914-byte HTML is
the **acrawler anti-bot interstitial**, with an empty `<body>` and exactly two scripts:

- **Script 0 (71,725 B):** ByteDance's VMP bytecode interpreter
  `window._$jsvmprt = function(b,e,f){…}` which defines `window.byted_acrawler`.
  Its env-capture array at the tail pulls only standard builtins
  (`Array, Error, JSON, Promise, WebSocket, eval, setTimeout, encodeURIComponent,
  encodeURI, Request, Headers, decodeURIComponent, RegExp`) — **all of which BO
  supports** (no exotic dependency).
- **Script 1 (1,090 B):** the bootstrap. Verbatim tail:
  ```js
  window.byted_acrawler.init({aid:99999999,dfp:0});
  var __ac_nonce=_f2("__ac_nonce"),
      __ac_signature=window.byted_acrawler.sign("",__ac_nonce);
  _f3("__ac_signature",__ac_signature), _f3("__ac_referer",document.referrer||"__ac_blank",!0);
  ...
  window.location.reload();
  ```
  i.e. read `__ac_nonce` (set by `Set-Cookie` on the first response), compute
  `__ac_signature = byted_acrawler.sign("", nonce)`, write it as a cookie, then
  **reload**.

The server response carries `Set-Cookie: __ac_nonce=…` and a `tt_stable`/`x-tt-*`
risk header set — classic ByteDance Whale anti-bot.

## What the reload returns (the 6313 B shell BO ends on)

On the reload, the server inspects `__ac_signature` + client risk signals and routes
to one of two pages. From this datacenter IP it routes to the **slide-captcha middle
page** — a 6,313-byte document `<title>验证码中间页</title>` ("Captcha Middle Page")
that loads `https://lf-cdn-tos.bytescm.com/obj/static/sec_sdk_build/3.5.2/captcha/index.js`
and runs:
```js
window.TTGCaptcha.init(options)
window.TTGCaptcha.render({ verify_data })
// verify_data = {"type":"verify","subtype":"slide","region":"cn",
//                "server_sdk_env":{"region":"CN","server_type":"whale"}, ...}
// captchaOptions.successCb: () => setTimeout(() => location.reload(), 2000)
```
The real app only appears after the **interactive slide puzzle** is solved
(`successCb` → reload). BO's final 6,327 B output IS this captcha middle page.

## BO did everything right (proof it is not an engine bug)

Live `BROWSER_OXIDE_DEBUG_NAV=1` trace (`/tmp/douyin.log`):
```
iter=0 url=…/ html_len=72914               # acrawler interstitial fetched
budget extended +25000ms (body=72KB …)
iter=0 FETCH GET https://www.douyin.com/   # reload re-fetch (location.reload handled)
iter=1 url=…/ html_len=6313                # reload returned the captcha middle page
[net] H2 POST https://verify.snssdk.com/captcha/reportFrontend  ×2   # TTGCaptcha SDK booted
```
- The acrawler VMP ran (no JS errors), `sign()` produced a value, cookie set.
- `location.reload()` is correctly wired: `window_bootstrap.js:1402` sets
  `__pendingNavigation{kind:"reload"}` → `_signalNav()` → the navigate loop
  re-fetches the same origin **carrying the cookie jar** (`lib.rs:237` "carry
  storage across same-origin reloads"), so `__ac_signature` WAS re-sent.
- On the captcha page BO loaded + executed `captcha/index.js` and its SDK POSTed
  `reportFrontend` twice — i.e. BO correctly drove the captcha SDK too.

There is no missing API, no failed CN fetch, no framework scheduling stall. The
pipeline completed; the **server chose to serve the captcha**.

## curl confirmation (server-side, engine-independent)

Reproduced the exact server behavior with curl from the same box:
| Request | Cookies | Bytes | Page |
|---|---|---|---|
| 1st GET | none | 72,914 | acrawler interstitial (`byted_acrawler`) |
| reload | `__ac_nonce` only | 6,313 | `验证码中间页` slide captcha |
| reload | `__ac_nonce` + **fake** `__ac_signature` | 6,313 | `验证码中间页` slide captcha |
| fresh GET (`?dummy=N`) | none | 72,914 | acrawler interstitial |

The reload always lands on the slide captcha from this IP — the gate is the
**server's risk decision on the reload**, reachable with plain curl. (Web research
corroborates: `__ac_signature` shortens once cookies are present, and the
`验证码中间页` is ByteDance's standard scraper block.)

## Root cause

`region:"cn" / server_type:"whale"` + datacenter/headless IP risk → ByteDance routes
the post-`__ac` reload to an **interactive slide captcha** (`TTGCaptcha`). No headless
engine renders the Douyin SPA without (a) a `byted_acrawler.sign()` the server
*accepts* AND (b) solving the slide puzzle, OR a clean (residential/CN) IP that the
risk engine lets skip the captcha. This is per-vendor challenge-solving =
**out of scope** per CLAUDE.md (no `vendor_solvers` in public crates).

## Why the MessageChannel/adidas fix did not flip it

That fix addressed React-18 concurrent-scheduler render scheduling. Douyin never
reaches React: it is stopped one layer earlier, at the acrawler→captcha gate, before
any app bundle is served. Different blocker class entirely.

## Is it flippable? (maybe — but not a public-engine render fix)

The ONLY engine-addressable lever that could move this is making the server *accept*
`__ac_signature` so the reload serves the app instead of the captcha — i.e. raising
the fidelity of the entropy that `byted_acrawler.sign()` digests (canvas/WebGL/UA/
navigator surface) so the risk score drops below the captcha threshold. Even then,
the `region:"cn"` routing and datacenter-IP reputation likely keep the slide captcha
in place (matches the repo's standing "datacenter IP gets the interactive captcha"
pattern, e.g. yelp/DataDome). Verifying whether camoufox v150's "full render" is real
or itself a same-IP flake requires a same-IP head-to-head (`run_delta_headtohead.py`);
v150 passing would point at signature/fingerprint fidelity, not a CN/IP wall.

**Recommendation:** classify douyin as a **vendor-challenge (ByteDance acrawler +
slide captcha) site, out of public-engine scope** — same bucket as Kasada/DataDome
interactive gates. Do not spend render-path budget here. If pursued, the work is
fingerprint-fidelity for `byted_acrawler.sign()` (entropy parity), not CSR rendering,
and must be confirmed with a same-IP v150 delta first to rule out an IP/geo wall.

## Evidence files
- `/tmp/douyin.log` — live BO nav trace
- `/tmp/douyin.out` — BO result (L3-RENDERED, 6327 B)
- `/tmp/douyin_raw.html` (72,914 B acrawler interstitial), `/tmp/r2.html` (6,313 B captcha middle page)
- Engine refs: `crates/js_runtime/src/js/window_bootstrap.js:1402` (reload),
  `crates/js_runtime/src/lib.rs:237` (reload carries cookie jar)
