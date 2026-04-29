# Deep Next-Steps — browser_oxide vs Camoufox findings + SOTA roadmap

**Generated**: 2026-04-28
**Inputs**:
- `docs/HOLISTIC_TEST_2026_04_28.md` — browser_oxide 126-site sweep
- `docs/HOLISTIC_TEST_CAMOUFOX_2026_04_28.md` — Camoufox 126-site sweep
- `docs/COMPARISON_OXIDE_VS_CAMOUFOX_2026_04_28.md` — head-to-head
- Network capture for 6 disagreement sites: `/tmp/cam_capture/*.json` + `/tmp/cam_capture/summary.txt`
- Engine source: `crates/browser/src/page.rs`, `crates/net/src/headers.rs`, `crates/stealth/src/presets.rs`

**North star**: SOTA on **stealth bypass** AND **speed**. Camoufox is the speed reference (~3.5 s avg/site, 13× faster than us). browser_oxide already wins stealth on creepjs/sannysoft/pixelscan/duckduckgo/wellsfargo/khanacademy/duolingo/ebay/amazon-com/amazon-jp (9 sites where Camoufox is detected). The remaining 6 sites Camoufox passes that we don't are concrete, fixable.

---

## Part 1 — Per-site analysis: 6 sites Camoufox passes that browser_oxide misses

For each, the captured Camoufox primary request/response headers are at `/tmp/cam_capture/<site>.json`. browser_oxide log evidence is in `/tmp/holistic_full.log`.

### 1.1 amazon-com-au — browser_oxide TIMEOUT 90 s, Camoufox L3-RENDERED 2008 B in 795 ms

**Camoufox observations**:
- Primary response: HTTP 202 from CloudFront with `x-amzn-waf-action: challenge`
- Body: 2010 bytes (a JS-driven AWS WAF challenge page)
- 2 total requests: `www.amazon.com.au` (the page) + `1c5c1ecf7303.b053132e.ca-central-1.token.awswaf.com` (token issuance)
- Cookies set: 0 (token is JS-only, posted later)
- Total time: 795 ms — the WAF JS solves locally and the test classifier sees the 2 KB challenge page (which does NOT contain "captcha" string)

**browser_oxide gap**: Page::navigate hits the AWS WAF challenge page, runs the WAF JS for 90 s, then times out. The 2 KB challenge body should be classified L3-RENDERED (under our heuristic — small body, no markers) but our `nav_iter=3 × ~30 s` budget burns the whole 90 s.

**Why Camoufox "wins" here**: It's a classification artifact + speed. Both engines reach the same WAF challenge page; Camoufox just doesn't wait around. Once the per-test budget completes, Camoufox returns the 2 KB challenge HTML and our heuristic marks it L3-RENDERED.

**Fix (concrete)**:
1. **Detect AWS WAF challenge response and short-circuit the navigate loop**. Add a marker check for `x-amzn-waf-action: challenge` response header in `crates/browser/src/page.rs::navigate_with_init` (line ~1414 area). If detected and the WAF JS completes (cookie set), return immediately rather than burning the full budget.
2. **Implement AWS WAF token POST flow** — same shape as Akamai sensor. Token endpoint format: `https://<id>.token.awswaf.com/<id>/<id>/inputs?client=browser`. Body: encoded fingerprint payload. This is documented externally.
3. **Quick win**: cap navigate iteration budget at 30 s for non-CHL responses (don't waste 90 s on a 2 KB stub).

**Expected unlock**: `amazon-com-au` + likely additional AWS-WAF protected sites that we haven't tested yet.

---

### 1.2 chl-known/adidas — browser_oxide THIN-BODY (75 s), Camoufox L3-RENDERED 2372 B in 1.9 s

**Camoufox observations**:
- Primary response: HTTP 200, body 2372 bytes (Akamai-protected adidas landing — bot-script first)
- `Set-Cookie` includes: `_abck`, `bm_ss`, `bm_so`, `bm_sz`, `bm_lso`, `bm_s`, `bm_sc`, `ak_bmsc`, `geo_*`, `AKA_A2`, `akacd_Phased_*` (16 cookies)
- 4 requests total to `www.adidas.com`
- Akamai BMP serves Camoufox a **first-touch sensor page** (~2 KB, mostly script tags).

**browser_oxide gap**: Returns THIN-BODY (~< 1 KB on first iter). Our captured holistic log shows `len=0` from oxide — meaning we got an empty body. Likely Akamai immediately serves us a **403** instead of the sensor page because:
- Our Chrome 147 sec-ch-ua headers may not match the JA4 Akamai expects
- We don't send the `accept-encoding: zstd` Camoufox does (we DO — verified in `headers.rs:241`)
- Most likely: Akamai detects browser_oxide via the **HTTP/2 frame ordering** or **PSK extension** in TLS, OR via a TCP-level SYN cookie pattern, things our reqwest+rquest stack may not match perfectly. Camoufox uses real Firefox's network stack.

**Fix (concrete)**:
1. **First-touch capture & diff**: run both engines through `mitmproxy` against adidas.com — diff every byte of the request including TLS ClientHello. Source of truth.
2. **Try Firefox profile**: add `firefox_135_macos` preset (see Part 2 §A) — Firefox UA may bypass adidas's bot scoring entirely (Camoufox proves this).
3. **Akamai sensor_data POST** (already roadmapped as `NEXT_STEPS_2026_04_28.md` item #2) — would unlock the rest of Akamai post-bypass.

**Expected unlock**: adidas + the 9 other Akamai-protected sites in our matrix (walmart, target, homedepot, costco, bestbuy, wayfair, expedia, weather, h-m, uniqlo, zara).

---

### 1.3 chl-known/leboncoin — browser_oxide DataDome-CHL (53 s), Camoufox L3-RENDERED 473 KB in 3.9 s

**Camoufox observations**:
- Primary response: HTTP 200, **`x-datadome: protected`** + sets a `datadome` cookie immediately on first response
- Body: 473 KB — full leboncoin homepage (!)
- Response includes `accept-ch: Sec-CH-UA, Sec-CH-UA-Mobile, Sec-CH-UA-Platform, Sec-CH-UA-Arch, Sec-CH-UA-Full-Version-List, Sec-CH-UA-Model, Sec-CH-Device-Memory` — DataDome ASKS for client hints
- Request domains: `static.captcha-delivery.com` (3 requests — DataDome's sensor JS), `ct.captcha-delivery.com`, `geo.captcha-delivery.com`
- Camoufox's Firefox UA causes DataDome to **issue the protection cookie immediately without challenge**, because:
  - Firefox doesn't send sec-ch-ua — DataDome can't apply Chrome-fingerprint scoring
  - Or DataDome's risk model treats Firefox/Mac as lower-risk by default

**browser_oxide gap**: We send Chrome-class headers + sec-ch-ua. DataDome's first-touch decision puts us in a "verify" bucket, serves the captcha challenge.

**Fix (concrete)**:
1. **Firefox profile** (highest ROI): if the UA + sec-ch-ua absence is the trigger, we can flip leboncoin without implementing the DataDome protocol at all.
2. **DataDome challenge solver** (already roadmapped). Captures: `dd_g`, `dd_s`, the JS challenge to `https://geo.captcha-delivery.com/captcha/check`. Code: a new `crates/stealth/src/datadome.rs` mirroring `kasada.rs`.
3. **Test header experiments**:
   - Try sending sec-ch-ua values matching mobile Chrome (some sites blanket-pass mobile)
   - Try omitting sec-ch-ua entirely (mimics Firefox/Safari) — see if DataDome relaxes
   - Try `accept-ch` response → second request includes high-entropy hints

**Expected unlock**: leboncoin + 3 other DataDome sites (etsy, wsj, yelp).

---

### 1.4 ru/wildberries — browser_oxide captcha-CHL (53 s), Camoufox L3-RENDERED 1619 B in 2.3 s

**Camoufox observations**:
- Primary response: **HTTP 498** (custom WBAAS status) + `x-wbaas-token: get` header + `server: wbaas`
- Body: 1619 bytes — **this is the WBAAS challenge page** (the SDK loader that asks the browser to solve a fingerprint challenge)
- Camoufox's body does NOT contain the substring `captcha`, so our classifier flags it L3-RENDERED — **classification false positive**
- 8 total requests, all to `www.wildberries.ru`

**browser_oxide gap**: Receives the same WBAAS challenge page. Body presumably contains `captcha` substring (perhaps because our V8 evaluates more of the WBAAS bootstrap JS that mentions "captcha" in error paths). Our classifier flags it CHL — actually correct: neither engine renders the real homepage.

**Fix (concrete)**:
1. **Tighten classifier** (already roadmapped as `NEXT_STEPS_2026_04_28.md` item #1). Skip generic `captcha` substring match for bodies that contain a known WBAAS marker (`x-wbaas-token` response header) — neither engine actually rendered WBAAS, so both should classify as `WBAAS-CHL`, not split-classify.
2. **WBAAS solver** (already tracked as `memory/open_tasks.md#68`): per `memory/critical_findings.md`, the token creation works (score=1000) but the subsequent reload still gets a challenge — IP-based. **Russian residential proxy is the only real fix** here.

**Expected unlock**: wildberries + ozon + yandex.ru + vk + mail.ru (5 RU sites) once proxy is in place.

---

### 1.5 social/twitter — browser_oxide THIN-BODY 69 B (55 s), Camoufox L3-RENDERED 282 KB in 8.3 s

**Camoufox observations**:
- Primary response: **HTTP 301 → Location: https://x.com/**
- 73 total requests across multiple domains
- Cookies set: `guest_id_marketing`, `guest_id_ads`, `personalization_id`, `guest_id`, `__cf_bm` — Cloudflare bot-management cookies
- Camoufox follows the 301, then renders the full x.com page

**browser_oxide gap**: We get the 301, but the FINAL body is 69 bytes. Two possible failures:
1. Our `get_follow` (max=10 redirects, `crates/net/src/lib.rs:660`) doesn't carry over Cookies/__cf_bm during the redirect → x.com returns a 69-byte stub
2. We follow the redirect but x.com returns the 69-byte body (a meta-refresh fallback) and our `Page::navigate` doesn't process meta-refresh

**Action**:
1. **Add tracing to `get_follow`**: log every hop's Location header, response status, and final body length. Confirm whether we're at twitter.com (got 301 but didn't follow) or at x.com (followed but got tiny body).
2. **Cookie carry-through audit**: when get_follow chains 301→302→200, verify all `Set-Cookie` from intermediate responses land in the cookie jar BEFORE the next request is sent. Likely candidate for the bug.
3. **Meta-refresh handling**: scan returned HTML for `<meta http-equiv="refresh" ...>` and follow it (Chrome and Firefox both do; we may not).
4. **Test with x.com directly**: skip twitter.com → does x.com pass? If yes, the bug is purely the redirect. If no, x.com challenges browser_oxide independently.

**Expected unlock**: twitter.com + any other site that uses old→new domain redirects.

---

### 1.6 streaming/disneyplus — browser_oxide captcha-CHL (50 s), Camoufox L3-RENDERED 178 KB in 772 ms

**Camoufox observations**:
- 1 single request, gets the full 178 KB body in 772 ms — Disney serves Firefox/Mac the page directly
- No challenge marker, no captcha
- Body length 178 KB suggests a real render

**browser_oxide gap**: We get a captcha-CHL response. Disney's anti-bot probably scores Chrome+macOS (our profile) higher than Firefox+Windows (Camoufox), or Disney just serves Chrome users a different bot-check JS.

**Fix (concrete)**:
1. **Firefox profile** (highest ROI for stealth class) — Disney clearly treats Firefox differently
2. Same as adidas: capture both engines through mitmproxy, diff TLS ClientHello + headers byte-by-byte
3. Possible quick test: try a different OS profile (Linux Chrome, Windows Chrome) — our current is `chrome_130_macos`; macOS may trigger Disney's bot scoring more

**Expected unlock**: disneyplus + likely several streaming category sites that share the same scoring stack.

---

## Part 2 — Cross-cutting observations

### A. Firefox profile is the highest single ROI stealth lever

**Evidence from the 6-site capture**:
- 4 of 6 disagreements (adidas, leboncoin, disneyplus, possibly wildberries-class) are likely "Firefox passes, Chrome doesn't" patterns
- Camoufox's actual UA is `Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:135.0) Gecko/20100101 Firefox/135.0` — vanilla Firefox 135 on Windows
- Camoufox sends NO sec-ch-ua headers (Firefox doesn't)
- Anti-bot vendors invest disproportionately in Chrome detection because Chrome is ~70% of bot traffic

**Implementation**:
1. Add `firefox_135_macos`, `firefox_135_windows`, `firefox_135_linux` presets to `crates/stealth/src/presets.rs` mirroring the existing `chrome_*` ones
2. Update `crates/net/src/headers.rs` to support a `firefox_headers()` builder that omits sec-ch-ua* and uses Firefox's accept format (`text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8`) and Firefox's accept-language (`en-US,en;q=0.5` — note q=0.5 not q=0.9)
3. Use Firefox-class TLS impersonation via rquest (or move TLS to actual Firefox cipher list)
4. JA4 should match Firefox's: `t13d1715h2_5b57614c22b0_3d5424432f57` (verifiable on tls.peet.ws)
5. Bonus: add a `randomize_browser` mode that picks Chrome or Firefox per-page (bot detectors look for cohort consistency; mixing is suspicious unless paired with cookie isolation per-page)

**Effort**: 1 day. **Sites unlocked (estimated)**: 6-15 (adidas, leboncoin, disneyplus, plus side benefits across categories).

### B. Header-order presence/absence matters more than values

Looking at Camoufox's request headers, the big differences from our Chrome profile:
- **No** `sec-ch-ua*` (Firefox)
- **No** `priority` header (Firefox)
- Has `connection: keep-alive` and explicit `host:` (HTTP/1-style — both engines use HTTP/2 but Camoufox surfaces them in headers list because Playwright captures them)
- Different `accept`: `text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8` (Firefox) vs `text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7` (Chrome)
- Different `accept-language` `q=0.5` vs `q=0.9` quality value

These per-vendor differences are why "Firefox profile" isn't just User-Agent — we'd need a full Firefox-class header builder.

### C. Some "wins" are classification artifacts, not real renders

**wildberries** is the clearest example: Camoufox's 1619-byte body is the WBAAS challenge page (status 498), but it doesn't contain `captcha` → classifies L3-RENDERED. **Both engines fail the actual site**; only the heuristic disagrees. Action: tighten classifier to use response headers (presence of `x-wbaas-token`, `x-datadome`, `x-amzn-waf-action`, `_abck` cookie set, etc.) instead of body string match.

This single classifier improvement would:
- Reclassify wildberries from CAM-only-PASS → both-CHL (fixes a false win for Camoufox)
- Likely reflect github/news/big-CMS sites better (the false positives identified in the original `NEXT_STEPS_2026_04_28.md`)
- Make all comparison numbers more honest

---

## Part 3 — Speed roadmap (close the 13× gap)

Camoufox completes 126 sites in 7.3 min; browser_oxide takes 96 min (13×). Per-site avg: 3.5 s vs 46 s. Drilling into where the time goes:

### Where browser_oxide spends its 46-second-per-site budget today

From the holistic log inspection:
- **Iteration 0 budget**: 49 s deadline (`V8DeadlineWatcher`)
- **Iteration extension**: +25 s (when body > 20 KB and no CHL marker)
- **Iteration 1**: 73 s deadline
- **Iteration 2**: 73 s deadline
- **Default `max_iterations`**: 3

For sites that complete fast (e.g., google.com renders in 1-2 s of JS), we still wait the full iter-0 budget. For sites that render the page but trigger a soft challenge marker, we then run all 3 iterations.

### Speed Item #1 — Tighten the navigate budget (effort: 4 h, payoff: 5-8× speedup on fast sites)

**Current** (`crates/browser/src/page.rs::navigate_with_init`):
- iter 0 budget: 49 s
- iter 1+ budget: 73 s
- 3 iterations always attempted unless explicit success

**Proposed**: adaptive budget that exits early
- Attempt 1: 15 s budget. If body > 50 KB AND DOMContentLoaded fired AND no CHL marker → return immediately
- Attempt 2: only if attempt-1 returned a CHL marker. 30 s budget.
- Attempt 3: only on retry-with-challenge-cookie loop. 30 s budget.
- Hard ceiling: 75 s (was implicitly 195 s)

**Expected**: ~15 s per fast site (was 50 s); ~75 s worst case (was 195 s). For 126-site sweep: ~30 min (was 96 min).

### Speed Item #2 — Multi-page parallel execution (effort: 1-2 days, payoff: 4-8× when batching)

V8 isolates are thread-local but each thread can host its own. The sweep is currently serial because `Page::navigate` is `async` but each test creates its own JsRuntime in the calling thread. **Refactor** to support concurrent navigation:

**Approach 1 — Thread pool with one isolate per thread**:
- Create a `PageWorker` thread-pool of N workers (e.g., 4 — matches typical core count)
- Each worker has its own `JsRuntime` instance, reusable across navigations
- Submit URL → return a future → completes when navigate finishes
- This is the same model Chrome uses (renderer-process-per-tab, capped at N renderers)

**Approach 2 — Actor model with ChannelMap**:
- Each navigation owns a worker via a oneshot channel
- Worker pool grows/shrinks as needed
- Simpler than #1 but more allocation per navigate

**Code touch points**:
- New `crates/browser/src/parallel.rs` — `ParallelPager { workers: Vec<JoinHandle<()>>, queue: Sender<Job> }`
- Move `Page::navigate` body into a worker-thread-friendly form (it's mostly async already — just need to pin the JsRuntime to a thread)

**Expected**: 4-worker pool sweeps the same 126 sites in **~8 min** (was 30 min after Speed Item #1, was 96 min before). Camoufox-class speed.

### Speed Item #3 — Built-in resource blocker (effort: 2-3 days, payoff: 30% speedup on news/store sites)

Camoufox bundles uBlock Origin which short-circuits ~30% of network requests. Our log showed pages like nytimes/cnn making hundreds of analytics/ad requests we then sit and parse JS for.

**Approach**:
- Bundle a curated EasyList + EasyPrivacy filter list (compiled into binary as static data — public domain)
- Hook into `crates/net/src/fetch.rs` (or wherever op_fetch lives in `js_runtime`); reject requests matching filter rules with HTTP 200 empty body
- Optional CLI flag `--no-blocker` for pure rendering tests
- Reuse adblock-rust crate (Brave's open source filter engine) — no need to write the filter parser

**Expected**: 20-40% per-site speedup on news/store/social sites; minor impact on antibot/tech sites.

### Speed Item #4 — HTTP/3 enabled by default (effort: 4-8 h, payoff: 1-2 s per site)

Per `crates/stealth/src/presets.rs:38`, HTTP/3 is currently disabled. Enabling for sites with `alt-svc: h3` advertisement (most CDNs, Cloudflare-fronted sites) saves the H2 handshake RTT.

**Approach**:
- Set `http3_enabled = true` on Chrome 130/Firefox 135 presets
- Verify rquest's H3 path is functional (it is, per its docs)
- Watch for sites that fingerprint H3 separately — Cloudflare's bot management does check H3 behavior

**Expected**: 0.5-1.5 s per site on H3-capable origins (probably 60% of the corpus).

### Speed Item #5 — JS module compile cache (effort: 1-2 days, payoff: 0.5-2 s per site)

If two pages load the same script (e.g., gtm.js, jquery.min.js), V8 today compiles it twice. With deno_core's snapshot facility, we can pre-compile common scripts and reuse the compiled bytecode.

**Approach**:
- Use `v8::Isolate::create_code_cache` after first compile of any script > 10 KB
- Cache by script URL (or content hash if URL is non-canonical)
- Bust on version change

**Expected**: 0.5-2 s on script-heavy sites (news, e-commerce). Smaller benefit on first run.

### Speed Item #6 — Skip page settling waits more aggressively (effort: 2 h, payoff: 1-3 s per site)

Many sites fire a flurry of `setTimeout(0)`, `requestAnimationFrame`, `IntersectionObserver` callbacks for ~2 s after DCL. Currently `Page::navigate` waits for all of these. We can **return as soon as `document.readyState === 'complete'` AND no in-flight fetch AND no pending timer** rather than the current generous wait.

**Approach**:
- Add a `wait_for_settled()` that polls these conditions every 50 ms with a 5 s cap
- Return body at first quiescence

**Expected**: 1-3 s per site, especially on news/social.

### Speed Item #7 — Removed redundant V8 flag (already done — `HEAP_INITIAL=1 GB`)

Already shipped in Phase 2 of the recent fix session. No further action.

### Roadmap summary for speed

| Order | Item | Effort | Expected speedup |
|---|---|---|---|
| 1 | Tighten navigate budget (adaptive iters, fast-exit on success) | 4 h | 5-8× |
| 2 | Parallel pager (4-worker pool) | 1-2 d | 4-8× |
| 3 | Resource blocker (adblock-rust + EasyList) | 2-3 d | 1.3× |
| 4 | HTTP/3 default-on for capable origins | 4-8 h | 1.1-1.2× |
| 5 | JS module compile cache | 1-2 d | 1.1-1.3× |
| 6 | Page-settled fast-return | 2 h | 1.1-1.3× |

**Combined target**: 126-site sweep in **5-8 minutes** (currently 96 min) — Camoufox-equivalent or faster.

---

## Part 4 — Stealth roadmap (compose with speed work)

The 6 sites Camoufox passes break down into 3 root causes:
1. **Browser fingerprint = Firefox** — adidas, leboncoin, disneyplus, possibly amazon-com-au
2. **Engine bug** — twitter (redirect/cookie carry)
3. **Classification artifact** — wildberries

For Items #1: Firefox profile (Part 2 §A). Items #2: redirect tracing + meta-refresh. Item #3: classifier improvement.

Beyond those 6, the broader stealth roadmap (covers the 66 both-fail sites):

### Stealth Item #A — Mouse + scroll humanization wiring (effort: 1-2 d, payoff: 5-15 sites)

**Already have**: `crates/stealth/src/sigma_lognormal.rs` (Bezier-curve mouse paths with sigma-lognormal velocity profile — matches human kinetic models from the literature).

**Missing**: integration into `Page::navigate`. Currently humanization is opt-in via `Page::navigate_humanized` but not wired into the default flow. Specifically:
- Random mousemove on page load (1-3 movements within first 3 s)
- One scroll event (down 200-400 px) within first 5 s
- Click on a non-link element (e.g., body) within first 8 s
- Use sigma_lognormal for inter-event timing

**Wire into**: a new `Page::navigate_with_behavioral_polish(url, profile, max_iter)` that combines `navigate_humanized` + the timing model. Make it the default for `Page::navigate` (current `navigate_humanized` becomes opt-in via `navigate_pure`).

**Expected**: 5-15 sites flip — especially behavioral-fingerprint-heavy (PerimeterX, DataDome, Akamai BMP). Mouse/scroll signals are critical.

### Stealth Item #B — Per-tab cookie+IP isolation (effort: 1 day per scope, payoff: prevents cross-contamination)

When running 126 sites in sequence, our cookie jar persists. Some sites (Amazon, social) flag cross-site cookies as bot-suspicious. **Action**: add `Page::navigate_isolated(url, profile)` that uses a fresh empty cookie jar per call.

This is also a precondition for Speed Item #2 (parallel pager) — each worker MUST have its own jar, otherwise concurrent requests cross-contaminate.

### Stealth Item #C — Audio fingerprint variance (effort: 4-6 h, payoff: 2-5 sites)

Per `docs/HANDOFF_2026_04_28_session_close.md` and the current memory: WebAudio compressor kernel has a threshold-response bug vs Chrome. creepjs detects this. Our recent work passes creepjs anyway via the mirror-realm fix (which throws creepjs's check off track), but a true WebAudio fix would close other audio-fingerprint sites (some banking, some social).

### Stealth Item #D — WebGL extension list audit (effort: 2-4 h, payoff: 2-3 sites)

Some sites (notably Disney+, Netflix per our Streaming-category data) check `gl.getSupportedExtensions()` and look for Chrome-specific extensions. Our WebGL stub may differ. Capture from real Chrome, diff against our stub.

### Stealth Item #E — Permissions API state matching (effort: 2 h, payoff: 1-2 sites)

`navigator.permissions.query({name: 'notifications'})` should return `{state: 'default'}` for fresh Chrome (not `'denied'`). Our `_PERMISSION_STATE_MAP` (window_bootstrap.js:283 area) already handles this but worth verifying matches headed Chrome 147 exactly.

### Stealth Item #F — Network-level fingerprint (effort: 1 week, payoff: 5+ sites)

Some sites (Cloudflare-fronted, WAF-protected) check **TCP-level** SYN cookie pattern, **TLS ClientHello** record sizes, **HTTP/2 SETTINGS frame ordering**. browser_oxide already matches Chrome 147 at JA4 level (per `memory/critical_findings.md`) but the deeper L4 fingerprint may diverge. Audit via `wireshark`/`tcpdump` against a real Chrome packet capture.

---

## Part 5 — Combined plan (priority-ordered)

This consolidates Parts 3 + 4 into an execution order that maximizes total ROI per day:

### Day 1 — quick wins on speed and stealth (8 h)
- 4 h: Speed #1 (tighten navigate budget — fast-exit, fewer iterations)
- 2 h: Speed #6 (page-settled fast-return)
- 2 h: Tighten classifier (Part 2 §C — already in `NEXT_STEPS_2026_04_28.md` item #1)

**Expected**: sweep time drops from 96 min → ~30 min. Classification noise drops, comparison numbers become honest.

### Day 2-3 — Firefox profile (16 h)
- 1 d: `firefox_135_*` presets in `stealth::presets`
- 0.5 d: Firefox header builder in `net::headers`
- 0.5 d: TLS impersonation (verify Firefox JA4 via tls.peet.ws)

**Expected**: 6-15 additional sites flip to PASS (adidas, leboncoin, disneyplus, several streaming/social).

### Day 4 — Twitter redirect + meta-refresh fix (4-8 h)
- Trace `get_follow` cookies during 301 chain
- Add meta-refresh handler
- Validate twitter, then run a focused 5-site test (sites with known redirects)

### Day 5-6 — Parallel pager (Speed #2) (16 h)
- New `crates/browser/src/parallel.rs` with worker-pool
- Each worker = own JsRuntime, own cookie jar
- Update `holistic_sweep.rs` to use it

**Expected**: sweep time → ~8 min (Camoufox-class).

### Day 7-8 — Resource blocker (Speed #3) (16 h)
- adblock-rust integration
- EasyList + EasyPrivacy bundled
- Hook into op_fetch

**Expected**: another 1.3× speedup, especially on news/stores.

### Day 9 — Mouse + scroll humanization wiring (Stealth #A) (8 h)

**Expected**: 5-15 sites flip — esp. PerimeterX, DataDome, Akamai BMP.

### Day 10 — Akamai BMP `_abck` + sensor_data POST (8 h)

**Already roadmapped** as `NEXT_STEPS_2026_04_28.md` item #2. Unlocks 9 retail + adidas if Firefox profile didn't already.

### Day 11+ — Vendor-specific protocols
- DataDome solver (1-2 d)
- PerimeterX press-and-hold (6-12 h)
- AWS WAF token POST (4-8 h)

---

## Part 6 — Concrete file changes summary

| Change | File(s) | Effort |
|---|---|---|
| Adaptive navigate budget | `crates/browser/src/page.rs::navigate_with_init` (line 592 area) | 4 h |
| Page-settled fast-return | same file, after html-build phase | 2 h |
| Firefox profile presets | `crates/stealth/src/presets.rs` (mirror chrome_*) | 8 h |
| Firefox header builder | `crates/net/src/headers.rs` (new `firefox_headers()`) | 4 h |
| TLS Firefox impersonation | `crates/net/src/lib.rs` (rquest config) | 4 h |
| Classifier tightening | `crates/browser/tests/holistic_sweep.rs` + `chl_sites.rs` (extract shared `classify()`) | 2 h |
| `get_follow` cookie tracing | `crates/net/src/lib.rs::get_follow` (line 660) | 1 h |
| Meta-refresh follower | `crates/browser/src/page.rs` (after final HTML returned) | 2 h |
| Parallel pager | `crates/browser/src/parallel.rs` (new) + `crates/browser/src/lib.rs` reexport | 16 h |
| Resource blocker | `crates/net/src/blocker.rs` (new, wraps adblock-rust) + `crates/js_runtime/src/extensions/fetch_ext.rs` hook | 16 h |
| HTTP/3 default-on | `crates/stealth/src/presets.rs:38` set `http3_enabled = true`; verify rquest H3 wiring | 4 h |
| Mouse/scroll wiring into `Page::navigate` | `crates/browser/src/page.rs` + `crates/stealth/src/sigma_lognormal.rs` | 8 h |
| Per-page cookie isolation | `crates/browser/src/page.rs` + cookie jar plumbing | 4 h |
| Compile cache | `crates/js_runtime/src/runtime.rs` (V8 code cache) | 16 h |
| AWS WAF detector + early-return | `crates/browser/src/page.rs::navigate_with_init` | 2 h (detector) + 8 h (full solver) |
| Akamai BMP sensor | `crates/stealth/src/akamai.rs` (new) | 8 h |
| DataDome solver | `crates/stealth/src/datadome.rs` (new) | 16 h |
| PerimeterX press-and-hold | `crates/stealth/src/perimeterx.rs` (new) | 8-16 h |

---

## Part 7 — Validation plan

After each day's work, validate via:

```bash
# 1. Unit + integration suite
cargo test --workspace -- --test-threads=1

# 2. Re-run holistic sweep — compare against baseline
cargo test --release -p browser --test holistic_sweep \
    -- --ignored --test-threads=1 --nocapture > /tmp/run.log 2>&1
diff <(grep "^holistic-end:" /tmp/holistic_full.log | awk '{print $4, $5}' | sort) \
     <(grep "^holistic-end:" /tmp/run.log | awk '{print $4, $5}' | sort)

# 3. Compare to Camoufox
source /tmp/camoufox-test/bin/activate
python /tmp/camoufox_sweep.py 2>&1 > /tmp/cam_run.log
join -t'|' -1 1 -2 1 \
    <(awk -F'|' '{print $1"_"$2"|"$3}' /tmp/holistic_results.psv | sort) \
    <(awk -F'|' '{print $1"_"$2"|"$3}' /tmp/camoufox_results.psv | sort) \
    > /tmp/comparison.psv
```

**Pass criteria**:
- All workspace tests pass (no engine regressions)
- Holistic PASS count never decreases vs current baseline (54)
- Sweep time decreases monotonically
- Comparison-vs-Camoufox: oxide-only PASS count ≥ camoufox-only PASS count

---

## Part 8 — Out of scope

- **Captcha solving** as general capability (e.g. integration with 2Captcha/Anti-Captcha) — separate product
- **DRM/Widevine** real implementation — requires CDM license
- **IP rotation infrastructure** — operational, not engine. Russian residential proxy is the highest-priority IP work tracked elsewhere
- **Headed-browser fingerprint matching** — we are headless by definition; if a site requires a real GPU/display, we'd need a different approach

---

## Inputs and references

- Camoufox per-site network captures: `/tmp/cam_capture/*.json`, summary at `/tmp/cam_capture/summary.txt`
- browser_oxide log: `/tmp/holistic_full.log`
- Comparison PSV: `/tmp/comparison.psv`
- Header builder source: `crates/net/src/headers.rs`
- Profile source: `crates/stealth/src/presets.rs`
- Navigate source: `crates/browser/src/page.rs::navigate_with_init` (line 592), `navigate_with_challenges` (line 1319)
- Existing roadmap: `docs/NEXT_STEPS_2026_04_28.md`
- Comparison report: `docs/COMPARISON_OXIDE_VS_CAMOUFOX_2026_04_28.md`
