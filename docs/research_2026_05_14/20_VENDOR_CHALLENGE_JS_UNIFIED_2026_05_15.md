# Unified finding: vendor challenge-JS must execute in-engine (2026-05-15)

## homedepot sec-cpt — actual captured shape (live, this session)

`GET https://www.homedepot.com/` → **HTTP 200**, 2621-byte body
(`/tmp/homedepot_seccpt.txt`), NOT a 428 JSON:

```html
<!DOCTYPE html><html lang="en"><body>
<script type="text/javascript"
  src="/Wjv3muMJul/a-27ijBVRX/bQEmGXt1/PwknTm9wYQE/QB/daSgMCCTMu?v=76fd93f0-3dbd-0575-e924-fbeac4ad17cb&t=741026095"></script>
<div id="sec-if-cpt-container" role="main" style="display:none"> … Akamai
   "Powered and protected by" branding … </div>
<script>(function(){
   var chlgeId='';                       // extracted from script src `t=`
   …proxies window.XMLHttpRequest.prototype.send…   // intercepts verify XHR
})()</script>
```

Headers: `server-timing: ak_p; desc="…"` (Akamai Bot Manager),
`x-proto: secure`, `cache-control: no-store`. **No `sec_cpt`
Set-Cookie on this response**, **no inline challenge JSON**.

The nonce / difficulty / timestamp / verify_url are **inside the
obfuscated challenge script** at the `?v=<uuid>&t=<token>` path. It
self-computes the PoW and submits the answer via the proxied XHR. This
is a **self-solving challenge bundle**, not a declarative challenge.

## Consequence: `sec_cpt::solve_crypto` is the FALLBACK, not the path

`crates/akamai/src/sec_cpt.rs` is complete + verified vs hyper-sdk-go,
but it expects a parsed `SecCptChallenge{nonce,difficulty,…}`. Those
params do NOT exist in any JSON we can parse — they're buried in the
rotating obfuscated `/Wjv3…?v=&t=` bundle. Reimplementing = a fragile
deobfuscation chase of a rotating target (same anti-pattern the W3.8
analysis rejected). The Rust solver remains valuable **insurance** /
unit-tested reference, but is not the primary homedepot path.

## The unified architecture (W3.8 + W4.2 + Cloudflare are ONE capability)

Every modern WAF challenge (Akamai sec-cpt, DataDome interstitial
i.js, Cloudflare Managed Challenge, Kasada ips.js) is now a
**self-contained JS (often + WASM) bundle that, when executed in a
real browser, solves itself and round-trips a validation cookie**.
We have a real V8 + native WASM + Chrome-faithful surface. So the
single highest-leverage engine capability is:

> **Execute the vendor challenge bundle to completion, let its
> XHR/fetch verify round-trip succeed (shared cookie jar), then
> re-issue the original request once the validation cookie is set.**

This one capability covers homedepot (union 120→121), etsy/tripadvisor
robustness (W3.8), and is the same shape Cloudflare needs. It replaces
three separate per-vendor reimplementation efforts (sec_cpt port,
DataDome crypto port, CF Turnstile) with one nav-pipeline feature.

## Why homedepot currently fails (the concrete gap)

The nav pipeline fetches the 2621-byte challenge page → it has a
`<script src="/Wjv3…?v=&t=">`. For self-solve to work the pipeline
must:

1. **Execute that external challenge script** — fetch `/Wjv3…?v=&t=`
   from the homedepot origin and run it (our script_runner does
   prefetch+exec external scripts — verify it runs THIS one; the path
   is same-origin so no CSP issue).
2. **Let the challenge script's PoW run to completion** within the
   nav budget. sec-cpt at difficulty≈15000 is sub-second on our CPU
   (the Rust port's module note); the JS version is slower but should
   finish in a few seconds — the budget must not cut it off. Akamai
   challenge hosts already get a 25 s budget bump (page.rs host-aware
   default) — verify sec-cpt pages hit that path.
3. **Let the proxied-XHR verify POST succeed** — the inline script
   wraps `XMLHttpRequest.send`; the challenge script XHRs the answer
   to its verify endpoint. Our fetch/XHR shares the HttpClient cookie
   jar, so the resulting validated `sec_cpt`/`_abck` cookie lands.
4. **Re-issue** `GET /` — Akamai now serves real content. The
   existing `navigate` iteration loop re-fetches on pending-nav;
   sec-cpt typically does a `location.reload()` on success, which the
   `__pendingNavigation` poll already catches → iteration 2 gets the
   real page → `Akamai-CHL` → `L3-RENDERED`, **union 120 → 121**.

So W4.2 is NOT "wire solve_crypto". It is: **verify the nav pipeline
fully executes + completes + re-issues the Akamai sec-cpt self-solve
cycle** — and fix whichever of steps 1-4 is currently truncating
(most likely: the external challenge script isn't being executed, or
the nav budget cuts off the PoW, or the post-solve reload isn't
re-fetched). Diagnosing requires a live homedepot run with nav
tracing (BOXIDE_DEBUG_NAV) to see which step stops.

## Pre-fix nav-trace result (2026-05-15, live homedepot, BOXIDE_DEBUG_NAV)

```
iter=0 url=homedepot html_len=2621
[akamai] learn_abck: _abck=92C6EE7B…~-1~<blob>~-1~-1~-1~…   ← _abck IS set
iter=0 FETCH GET homedepot
iter=1 url=homedepot html_len=2621                          ← SAME page
iter=1 V8DeadlineWatcher 35814ms remaining                  ← budget fine
…iter=1 _abck re-issued (fresh blob), still 2621-byte page…
```

Confirms **root-cause A** (detection gap): the sec-cpt response DOES
set `_abck` (in the Set-Cookie/header) but `_abck` is NOT in the 2621
-byte BODY — so the pre-fix `is_anti_bot_challenge()` body-substring
check (`_abck && sensor_data`) never matched. The
`sec-if-cpt-container` marker fix (committed this session) addresses
this. Budget is NOT the issue (35 s remained at iter=1) — retire the
step-2 hypothesis.

Surfaces **root-cause B** (open): even across iter 0→1 re-fetches the
page stays the 2621-byte challenge — the `<script src="/Wjv3…?v=&t=">`
self-solving bundle is **not completing its solve+verify+reload
cycle** (no verify XHR, no content flip in the trace). Either it isn't
executed, or it executes but its PoW/verify/reload doesn't finish.
Detection (fix A) is necessary but likely not sufficient; B needs the
post-fix run + challenge-script-execution instrumentation.

## PRECISE root cause (full trace, net-trace decoded) — 2026-05-15

The complete trace's `net-trace` shows our engine **DOES execute the
sec-cpt challenge bundle** and POSTs to the sec-cpt endpoint
`https://www.homedepot.com/Wjv3muMJul/…/BqRSxpZwgB`:

```
xhr  POST /Wjv3…/BqRSxpZwgB  body={"sensor_data":"3;0;1;0;8888888;…;74,0,0,3,10,0;…"} →201
fetch POST same                                                                      →201
xhr  POST /Wjv3…/BqRSxpZwgB  body={"sensor_data":"3;0;1;0;4599878;…;90,95,0,1,4,2279;…"}→201
[JS LOG] [XHR] open/send POST /Wjv3…/BqRSxpZwgB
```

So the engine capability (execute the vendor bundle + round-trip)
**works** — the bundle runs and POSTs. The bug is **payload-type
conflation**:

- `handle_akamai_flow` sees the challenge response's `_abck=…~-1~…`
  (slot 1 = -1 ⇒ our W1.3 parser = `NeedsSensor`) and runs the
  generic **Akamai BMP `sensor_data`** path, POSTing
  `{"sensor_data":"3;0;1;0;<cookieHash>;…"}` to the `/Wjv3…` endpoint.
- But `/Wjv3…` is the **sec-cpt PoW verify endpoint**. It expects a
  PoW answer submission (`{token, answers}` — `SecCptAnswerSubmission`,
  the shape `sec_cpt::solve_crypto` produces), NOT BMP sensor_data.
- Akamai accepts the POST (`201`) but never clears `_abck` (it's the
  wrong challenge response for a sec-cpt gate) → infinite
  NeedsSensor loop → `Akamai-CHL`.

(Field-5 detail: first POST uses `8888888` = the no-`bm_sz` default
from `akamai/lib.rs::parse_bm_sz`; a later POST uses `4599878` once
`bm_sz` parsed. Both 201, both ignored — confirming it's not a
cookieHash-staleness issue but a **wrong challenge-type** issue.)

### The corrected fix path

Detection (the `sec-if-cpt-container` marker, committed) is necessary.
The SOLVE must route by challenge type:

- **sec-cpt gate** (page has `sec-if-cpt-container`, the `/Wjv3…?v=&t=`
  bundle): the bundle should run its OWN PoW and submit the answer —
  OR we extract the sec-cpt params and use `sec_cpt::solve_crypto` →
  POST `SecCptAnswerSubmission{token,answers}` to the verify endpoint.
  Do **not** run `handle_akamai_flow`'s BMP sensor_data POST for these.
- **plain BMP** (`_abck` + `sensor_data` markers, no sec-cpt
  container): existing `handle_akamai_flow` path (works — bestbuy
  flips GREEN).

The gate condition in `handle_akamai_flow` must additionally check
"is this a sec-cpt challenge?" and, if so, route to the sec-cpt PoW
submission instead of the BMP sensor POST. The sec-cpt bundle already
executes in our engine (proven by the trace) — the issue is our Rust
driver *also* fires the wrong BMP POST and never lets/makes the
sec-cpt answer submission happen with the correct payload shape.

This is the genuinely hard remaining engine work for homedepot:
challenge-type-aware routing + sec-cpt answer submission. Multi-step;
the root cause is now exact. `sec_cpt::solve_crypto` (byte-verified)
supplies the answer; the open work is (a) extracting the sec-cpt
challenge params (token/nonce/difficulty/verify_url) from the executed
`/Wjv3…` bundle's context or a parseable sub-resource, and (b) gating
`handle_akamai_flow` so BMP sensor_data is not POSTed to a sec-cpt
endpoint.

## Next experiment

Run a live homedepot navigation with `BOXIDE_DEBUG_NAV=1` + script-
exec logging; observe: (a) is `/Wjv3…?v=&t=` fetched+executed?
(b) does the PoW JS run to completion or get budget-cut? (c) does the
verify XHR fire and get a 200 + cookie? (d) is the post-solve reload
re-fetched? The failing step is the W4.2 fix. This supersedes the
"parse 428 → solve_crypto" wiring plan in doc 19 (that plan assumed a
declarative challenge that does not exist for homedepot).
