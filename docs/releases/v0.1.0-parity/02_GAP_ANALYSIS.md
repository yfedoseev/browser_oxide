# 02 — Gap analysis: every Camoufox-only-pass site

The **recoverable surface** is exactly 10 sites: Camoufox passes strict (`L3-RENDERED` ≥ 15 KB) and no BO profile does. Recover ≥ 6 of these and BO routed beats Camoufox 113.

## The 10 sites at a glance

| # | Site | BO routed best | Camoufox | Cluster | Difficulty | Chapter |
|---|---|---|---|---|---|---|
| 1 | reddit | L3 8326 | L3 1.1 MB | SPA-verify-challenge | EASY | 05 |
| 2 | duolingo | L3 13566 | L3 696 KB | Recaptcha-invisible | EASY | 05 |
| 3 | booking | L3 8473 | L3 37915 | SPA hydration | MED | 05 |
| 4 | douyin | L3 6327 | L3 1.0 MB | SPA hydration | MED | 05 |
| 5 | amazon-de | L3 2011 | L3 763 KB | AWS-WAF | HARD | 06 |
| 6 | amazon-in | L3 2011 | L3 997 KB | AWS-WAF | HARD | 06 |
| 7 | amazon-com-au | L3 2011 | L3 899 KB | AWS-WAF | HARD | 06 |
| 8 | imdb | L3 1995 | L3 1.0 MB | AWS-WAF | HARD | 06 |
| 9 | etsy | DataDome-CHL 1424 | L3 253 KB | DataDome | MED | 07 |
| 10 | x-com | THIN-BODY 69 | L3 379 KB | TLS / rate-limit | HARD | 05 / research |

## Per-site evidence

Each entry below contains: **observed flow → most likely root cause → first concrete debug step**. Every claim references a captured artifact you can reproduce.

---

### 1. reddit — `https://www.reddit.com/`

**BO observed**:
- HTTP fetch returns 8424 bytes (`/tmp/reddit_curl.html`)
- BO classifies `L3-RENDERED` with body 8326 bytes
- Total time: 316-448 ms across runs
- Only **iter 0** runs (no iter 1 in the log)
- No `[vendor-detect]` markers

**What the 8326 bytes IS**:
- Title: `Reddit - Please wait for verification`
- One `<script>` element with this body (deobfuscated):
  ```js
  document.addEventListener("DOMContentLoaded", async function() {
    const e = document.forms[0];
    e.onsubmit = function(t) {
      new URLSearchParams(document.location.search).forEach((v, k) =>
        t.target.appendChild(Object.assign(document.createElement("input"), {
          name: k, type: "hidden", value: v
        }))
      );
      return true;
    };
    const n = await (async e => e + e)("80bfd25d73acfab1");
    e.elements.namedItem("solution").value = n;
    e.requestSubmit();
  }, { once: true });
  ```
- The "challenge" is a trivial proof-of-execution: prove your JS can fire DOMContentLoaded, do an `async/await`, set a form input, and call `requestSubmit()`. Solved → POSTs a form to Reddit → server sets cookie → next GET returns real Reddit.

**Root cause hypothesis** (highest probability first):
1. `requestSubmit()` calls `this.submit()` (see `crates/js_runtime/src/js/dom_bootstrap.js:1108`); `submit()` sets `__pendingNavigation` with `method:'POST'` (line 1098-1107). Likely something in this chain isn't firing or the outer loop reads `pending_info` BEFORE the chain completes.
2. `document.forms[0]` returns wrong element — verify by adding `console.log(document.forms.length, document.forms[0])` to a test.
3. `e.elements.namedItem("solution")` returns `null` (HTMLFormControlsCollection.namedItem may not be implemented).
4. `e.requestSubmit()` throws (not registered on prototype despite `dom_bootstrap.js:1108`).

**First debug step**:
```bash
cat > /tmp/just_reddit.json <<'JSON'
[{"cat":"social","name":"reddit","url":"https://www.reddit.com/"}]
JSON
BROWSER_OXIDE_DEBUG_NAV=1 RUST_LOG=js_runtime=trace,browser=debug \
  target/release/examples/sweep_metrics chrome_148_macos /tmp/just_reddit.json /tmp/out.json \
  2>&1 | grep -E "(pending_nav|reddit|requestSubmit|forms\[|solution|JS LOG|JS ERROR)"
```

After build_page returns, the loop calls `PENDING_NAV_JS` (`page.rs:1944`). Log this value. If empty → submit() didn't fire. If non-empty but iter 1 doesn't follow → the URL handler is wrong.

**Cross-check** (run in isolation, no prior corpus state):
- Capture `body.html`, `script_errors`, `fetches.json` per the spec in `04_TOOLING_SPEC.md`.

**Expected fix**: 1-line if it's a `namedItem`/`requestSubmit` typo; medium if the loop's `PENDING_NAV_JS` reader has a race.

---

### 2. duolingo — `https://www.duolingo.com/`

**BO observed**:
- BO body: 13327-13566 bytes (across profiles)
- Time: 15171 ms (full nav budget)
- Loads `https://www.recaptcha.net/recaptcha/enterprise.js?render=6LcLOdsjAAAAAFfwGusLLnnn492SOGhsCh-uEAvI`
- Loads `https://www.gstatic.com/recaptcha/releases/Br0hYqpfWeFzYCAXLD4UuCIV/recaptcha__en.js`
- Loads `https://www.recaptcha.net/recaptcha/enterprise/webworker.js` ← **WORKER**
- Final body: SPA shell + reCAPTCHA invisible badge, no hydrated content

**What's missing**: The duolingo SPA hydrates only after the recaptcha invisible challenge fires `grecaptcha.execute()` and resolves with a token. Our Worker (`webworker.js`) almost certainly fails — recaptcha enterprise.js depends on cross-thread `Worker` messaging that BO supports but with caveats (see `crates/js_runtime/src/extensions/worker_ext.rs`).

**Root cause hypothesis**:
1. `Worker(webworker.js)` is spawned but the recaptcha verifier inside it detects we're not a real browser (no real `MessageChannel`, missing some `Worker` API).
2. `grecaptcha.execute()` never invokes its `.then(token)` handler → SPA never receives the token → never fetches `/api/...` → 13 KB.

**1.7 KB to gate** — duolingo is the closest miss. A single forced render of the lessons-list mount could push it over.

**First debug step**:
```bash
# Run duolingo in isolation with worker-extension trace logging
RUST_LOG=js_runtime::extensions::worker_ext=trace,info \
  target/release/examples/sweep_metrics chrome_148_macos /tmp/just_duolingo.json /tmp/out.json \
  2>&1 | tee /tmp/duo.log
grep -E "(worker|Worker|grecaptcha|recaptcha)" /tmp/duo.log
```

Then capture both Camoufox + BO with the tooling from `04_TOOLING_SPEC.md` and diff `fetches.json`. Expect: Camoufox fetches `/api/v2/users/me` or similar after recaptcha token resolves; BO doesn't.

**Expected fix**: depends on diagnosis. May be MessageChannel impl, may be `navigator.userAgent` inside Worker context, may be a missing `Worker.prototype.postMessage` field.

---

### 3. booking — `https://www.booking.com/`

**BO observed**:
- Body 8473 bytes (chrome/pixel/firefox), 3891 bytes (iphone — different)
- Time: 15084 ms (full budget)
- No iter 1
- Final body: SPA shell

**Root cause hypothesis**: SPA shell only. booking.com is a heavy server-side React app; the first response is an SPA bootstrap that needs to fetch `/api/...` to hydrate. Something in that fetch chain fails.

**First debug step**: Capture both BO and Camoufox `fetches.json`. Diff to find the missing fetch.

---

### 4. douyin — `https://www.douyin.com/` (TikTok-CN)

**BO observed**:
- Body 6327 bytes uniformly across profiles
- Time: 3230-3948 ms
- No iter 1

**Root cause hypothesis**: similar to booking — SPA shell that requires fingerprint check. douyin.com uses a custom anti-bot called "ttwid" + `__ac_signature` cookie. The 6327-byte body may include their JS that tries to compute the signature.

**First debug step**: Capture body, search for `__ac_signature`, `ttwid`, `mssdk_*` in script content. Compare to Camoufox fetch chain — Camoufox likely solves the signature and gets the FYP feed.

---

### 5-8. AWS WAF cluster — amazon-de / amazon-in / amazon-com-au / imdb

**BO observed** (all 4 sites, all 4 profiles):
- Body exactly 2011 bytes (amazon variants) or 1995 bytes (imdb)
- Time: 15-90 s (full budget exhausted or iterations triple)
- `[vendor-detect] aws-waf` marker fires (page.rs:1050)
- Engine POSTs to `https://*.token.awswaf.com/.../report` (telemetry endpoint)
- Engine **does not** POST to `/token` (the actual challenge-solve endpoint)

**The 2011-byte body** (verbatim, captured via curl from amazon-de):
```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title></title>
  <script>
    window.awsWafCookieDomainList = [];
    window.gokuProps = {
      "key": "AQIDAHj...",
      "iv": "A6wUCgAhZgAB...",
      "context": "glDLQwA7fqdXeYzd6QoT..."
    };
  </script>
  <script src="https://1c5c1ecf7303.d474e66d.us-west-2.token.awswaf.com/.../challenge.js"></script>
</head>
<body>
  <div id="challenge-container"></div>
  <script>
    AwsWafIntegration.saveReferrer();
    AwsWafIntegration.checkForceRefresh().then((forceRefresh) => {
      if (forceRefresh) {
        AwsWafIntegration.forceRefreshToken().then(() => window.location.reload(true));
      } else {
        AwsWafIntegration.getToken().then(() => window.location.reload(true));
      }
    });
  </script>
</body>
</html>
```

**Mechanism**: `AwsWafIntegration.getToken()` (in challenge.js) fingerprints the browser, computes a token via WebAssembly proof-of-work, POSTs to `awswaf.com/.../verify`, and the response sets an `aws-waf-token` cookie. Then `location.reload(true)` re-fetches the page WITH the cookie → AWS WAF lets it through.

**Why BO fails**: challenge.js detects our engine via one or more fingerprint signals (likely: Worker behaviour, WebAssembly subtle differences, fonts list, hardware concurrency, or a stealth-related telemetry mismatch) and **silently does not call `getToken()`** — only the `/report` telemetry fires.

**Non-determinism**: same site, same code, different result run-to-run:
- amazon-co-uk: chrome got 696 KB, pixel got 2011, iphone got 1 MB, firefox got 694 KB
- amazon-ca: chrome 2011, pixel 1 MB, iphone 2011, firefox 2011

This means part of the gap is AWS-side risk-rolling (the token-issuance gate is probabilistic, not deterministic on fingerprint). Routing already saves 4 of 8 amazon variants. The other 4 (de/in/com-au/jp) are consistently 2011 across all profiles — fingerprint detection is firm.

**Chapter 06** has the detailed solver plan: capture challenge.js, deobfuscate, identify the fingerprint check, decide on solver (engine-side stealth patch vs Rust-side token request vs full solver in `vendor_solvers`).

---

### 9. etsy — `https://www.etsy.com/`

**BO observed**:
- All 4 profiles: `DataDome-CHL` 1424 bytes
- `[vendor-detect] datadome` fires (`x-datadome` response header)
- Pre-strip (before `aecdf19`): etsy passed via the WASM-iframe-daily-key path (see `memory/state_2026_05_16_phase5_datadome.md` for the prior solution).

**Mechanism**: DataDome interstitial. The `1424 bytes` is the challenge document that loads:
- `dd-script.js` from captcha-delivery.com
- A cross-origin iframe to `geo.captcha-delivery.com/captcha/?...`
- The iframe runs the actual challenge (WASM-based) and POSTs a result that yields a `datadome=` cookie.

**Why BO fails**: After `aecdf19`, the public engine doesn't:
1. Relax CSP on the challenge document (DataDome's interstitial blocks its own iframe under strict CSP — real Chrome's interstitial handling permits this)
2. Materialize the cross-origin iframe contents (BO has `rematerialize_iframes` at `page.rs:1981` but it's gated by `started_as_dd_challenge` which depends on the removed handler)
3. Recognize the `datadome=` cookie write as a solve signal (would trigger a re-fetch)

**Chapter 07** plans restoration of these 3 behaviours as **engine primitives** (no vendor name in code) so they apply to any vendor — DataDome / Cloudflare / Akamai — that follows the same pattern.

---

### 10. x-com — `https://x.com/`

**BO observed**:
- Full sweep: `THIN-BODY` 69 bytes (after 24 prior nav requests in the sweep)
- Isolated run: `L3-RENDERED` 274 KB

The difference is real and reproducible. In isolation, x.com serves the SPA shell. Mid-sweep (after 24 other sites have populated the shared cookie/Accept-CH jar), Twitter's WAF drops the connection at the TLS/HTTP boundary.

**Root cause hypothesis**: `f62584d` SharedSession bleed. The process-wide `accept_ch` set picked up an `Accept-CH` header from some earlier site that Twitter's WAF heuristic now treats as anomalous, and it drops the connection. (Or: rate-limit per-IP after enough other site fetches.)

**First debug step**: A/B test SharedSession (HttpClient::shared vs HttpClient::new) on the FULL 126-site sweep, observe whether x.com flips from THIN-BODY back to L3-RENDERED.

**Not in v0.1.0 critical path** unless A/B test confirms. Track in `15_OPEN_QUESTIONS.md`.

## The 8 hard-residual sites (NOT in scope for v0.1.0)

Even Camoufox fails these. They're the open-source-SOTA frontier. Documented in `08_KASADA_FRONTIER.md` for posterity / future work.

| Site | Block | Notes |
|---|---|---|
| amazon-jp | AWS WAF | When the roll goes hard |
| bestbuy | Akamai SPA shell | Cross-engine failure |
| homedepot | Akamai sec-cpt | Was passing on iPhone profile pre-strip |
| realtor | Kasada | Open frontier |
| canadagoose | Kasada | Open frontier |
| hyatt | Kasada | Open frontier |
| wildberries | SPA shell | Cross-engine |
| areyouheadless | antibot probe | Diagnostic — never going to pass cleanly |

## Per-profile breakdown (which profile contributes which sites to routed-108)

Use this to understand which profile to prioritize. For each site that's in **routed pass but NOT in EVERY profile**, the table shows which profile uniquely passed it.

See `11_PER_PROFILE_STRATEGY.md` for the routing-decision tree.

## Files referenced in this chapter

- `crates/browser/src/page.rs:1050` — `[vendor-detect]` AWS WAF logger
- `crates/browser/src/page.rs:1944` — `PENDING_NAV_JS` reader after build_page
- `crates/browser/src/page.rs:1981` — `rematerialize_iframes` (DataDome-gated)
- `crates/js_runtime/src/js/dom_bootstrap.js:1098-1110` — `submit()` / `requestSubmit()`
- `crates/js_runtime/src/extensions/worker_ext.rs` — Worker implementation
- `/tmp/full_sweep_2026_05_24/bo_chrome_148_macos_cold.log` — full per-site trace
- `/tmp/amazon_de_curl.html` — captured 2011-byte AWS WAF stub
