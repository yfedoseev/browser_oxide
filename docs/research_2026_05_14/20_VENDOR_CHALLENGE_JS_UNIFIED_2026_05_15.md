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

## Next experiment

Run a live homedepot navigation with `BOXIDE_DEBUG_NAV=1` + script-
exec logging; observe: (a) is `/Wjv3…?v=&t=` fetched+executed?
(b) does the PoW JS run to completion or get budget-cut? (c) does the
verify XHR fire and get a 200 + cookie? (d) is the post-solve reload
re-fetched? The failing step is the W4.2 fix. This supersedes the
"parse 428 → solve_crypto" wiring plan in doc 19 (that plan assumed a
declarative challenge that does not exist for homedepot).
