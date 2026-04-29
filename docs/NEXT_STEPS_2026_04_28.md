# Next Steps — 2026-04-28 post-holistic-sweep

**Source**: `docs/HOLISTIC_TEST_2026_04_28.md` (126 sites, 43% PASS, 0 engine errors)
**Status of engine**: Phase 1+2 (iterative DOM walkers, cycle assertion, mirror-realm topological build, storage `has` trap, complete style/dataset/attributes Proxy traps, memoized plugin lengths, 1 GB heap, `_getNodeId` -1 fix) all shipped and validated. **Engine fingerprint surface and resilience are no longer the bottleneck** for the remaining 55% of failing sites — the gaps are vendor-specific anti-bot protocols and external infrastructure (IPs).

---

## Priority order (highest ROI first)

| # | Task | Effort | Sites unlocked | Type |
|---|---|---|---:|---|
| 1 | Tighten classification heuristic — eliminate false positives | 1-2 h | ~3-5 | Reporting accuracy |
| 2 | Akamai BMP `_abck` + sensor_data POST | 4-8 h | ~9 | Protocol |
| 3 | DataDome captcha solver | 1-2 d | ~4 | Protocol |
| 4 | Russian residential proxy ($50-500/mo) | $$ | ~5+ | Infra |
| 5 | Diagnose social-cluster fingerprint gap | 4-8 h | ~7+ | Engine surface |
| 6 | PerimeterX press-and-hold (zillow + wayfair) | 6-12 h | ~2 | Protocol |
| 7 | Cloudflare Turnstile diagnosis (udemy + others) | 4-6 h | ~1 | Protocol |
| 8 | Streaming platform fingerprint diagnosis | 1-2 d | ~6 | Engine surface |
| 9 | Diagnose unexplained BLOCKs (cloudflare/coursera/bofa) | 2-4 h | ~3 | Investigation |

Plus the previously-tracked tasks in `memory/open_tasks.md`: WBAAS (#68 — needs RU proxy), Akamai (#64 — same as task #2 here), HUMAN/PerimeterX (#65 — same as #6 here), Douyin signature wiring (#66), leboncoin retest (#70).

---

## 1. Tighten classification heuristic (1-2 h, ~3-5 sites flipped)

**Problem**: The current `chl_sites.rs` / `holistic_sweep.rs` classifier matches the literal substring `"captcha"` anywhere in the body. Several false positives in the sweep:

- **github** — body 581 KB, real GitHub homepage; the string `"captcha"` likely appears in their privacy/security text
- **bloomberg / cnn / washingtonpost / reuters / economist** — multi-MB bodies (4-7 MB) classified as `BLOCKED` because the body contains `"blocked"` (e.g., "ad-blocker detected"), `"403"`, or `"access denied"` mentions in CMS / footer text. These were likely real renders.

**Fix**: replace word-substring match with a more selective heuristic:
1. Check for vendor-specific markers FIRST (`_kpsdk`, `_abck`, `_pxhd`, `captcha-delivery`, etc.) — these are reliable.
2. Only fall back to generic `captcha` / `blocked` substrings if body length is small (<10 KB) or the marker is in a `<title>` / `<h1>` / specific element.
3. Treat body length >100 KB as a strong signal of real render even if a fuzzy marker is present, requiring a stronger marker to override.

**Files**: `crates/browser/tests/chl_sites.rs:33-50`, `crates/browser/tests/holistic_sweep.rs:33-50` (both share the marker list — extract to a shared helper module under `crates/browser/tests/common/` or move into the browser crate as a public utility).

**Validation**: re-run the holistic sweep; expected to flip ~5 sites from CHL/BLOCKED to L3-RENDERED.

---

## 2. Akamai BMP `_abck` + sensor_data POST (4-8 h, ~9 sites unlocked)

**Already tracked** as `memory/open_tasks.md#64`. Confirmed by the holistic sweep: Akamai-CHL hits **9 sites** — walmart, target, homedepot, costco, bestbuy, wayfair, expedia, weather, h-m, uniqlo, zara, bloomberg, yelp-adjacent (and adidas from the chl-known set).

**What's needed**:
- `_abck` cookie parsing — token format documented in `docs/akamai_sensor_analysis/`
- sensor_data JSON POST to `/_bm/_data` — protocol well-documented externally
- Retry loop on `_abck` invalidation
- Integration into `Page::navigate_with_challenges` chain (already exists for Kasada)

**Where to add**: new `crates/stealth/src/akamai.rs` module mirroring the structure of `crates/stealth/src/kasada.rs`. The Page-side wiring goes through `page.rs::navigate_with_challenges` (line 1319 area).

**Reference material**: `docs/akamai_sensor_analysis/` (existing in repo), plus `crates/browser/tests/adidas_*.rs` test files (4 files) which already capture sensor flow.

---

## 3. DataDome captcha solver (1-2 days, ~4 sites)

**Hits**: leboncoin, wsj, etsy, yelp.

DataDome serves a captcha challenge page with its own protocol:
- `captcha-delivery.com/captcha/?initialCid=…&hash=…` redirect
- POST to `/captcha/check` with cookies + token
- On success, sets `datadome` cookie and redirects back

**Approach**:
1. Detect DataDome redirect (already classified as `DataDome-CHL`)
2. Implement the GET → solve → POST → cookie loop in a new `crates/stealth/src/datadome.rs`
3. The captcha itself often requires real human interaction (slider, click) — for headless, look for cases where DataDome's "interstitial" path (no captcha, just JS challenge) is sufficient. The interstitial uses `dd_g` / `dd_s` JS-VM tokens that we may be able to compute.

**Risk**: full captcha solver may require human-in-the-loop or third-party CAPTCHA service. Start with the interstitial-only path which works on a majority of DD-protected sites.

**Where**: `crates/stealth/src/datadome.rs` (new). Wire into `page.rs::navigate_with_challenges`.

---

## 4. Russian residential proxy (cost: $50-500/mo, unlocks ~5 sites + spillover)

**Already tracked** as `memory/open_tasks.md#68`.

The holistic sweep confirms: every Russian site CHL'd or BLOCKED — `wildberries`, `yandex-ru`, `vk`, `mail-ru`, `ria`. Per existing memory, WBAAS token creation already works (`create-token` scores 1000/1000); the failure is IP-level rejection on the subsequent page load. A datacenter IP from outside Russia will get challenged regardless of fingerprint quality.

**Action**: subscribe to a Russian residential proxy service (Bright Data, Smartproxy, IPRoyal, etc. all have RU residential pools). Estimated cost $50-500/mo depending on traffic.

**Validation**: re-run the holistic sweep with the proxy injected via `BOXIDE_PROXY` (or whatever env var the existing fetch_ext supports). Expected: 4-5 RU sites flip + possibly some strict-tier sites (Kasada/PerimeterX) as a side effect of having clean IP reputation.

---

## 5. Diagnose social-cluster fingerprint gap (4-8 h, ~7 sites)

**Hits the entire social category**: facebook, instagram, linkedin, pinterest, reddit, threads, tumblr, quora — all `captcha-CHL`.

Facebook, Instagram, and Threads share Meta's anti-bot stack. Reddit, Pinterest, Quora, and Tumblr each have their own. But the consistency of detection (8/10 sites in social category) suggests a single fingerprint surface gap that all of these check.

**Diagnosis approach**:
1. Manually load Facebook in real Chrome with DevTools open. Look at the initial JS that fingerprints the browser — find the function names and flags they check.
2. Compare to what our `dom_bootstrap.js` / `window_bootstrap.js` shims expose. Look for properties that real Chrome exposes but we don't (or vice versa).
3. Quick wins to check first:
   - WebGL extension list (we may be missing some Chrome-only extensions)
   - `Intl.DateTimeFormat().resolvedOptions().timeZone` matches `Date.toString()` timezone
   - `navigator.connection` shape (we have `_navConnection` but it may have wrong jitter)
   - `Notification.permission` value (default 'default' vs Chrome's 'denied' for some configs)
   - `chrome.runtime` shape — Meta tracks this closely
   - Battery API removal — modern Chrome no longer exposes Battery API, we may be exposing it

**Validation**: re-run social category only after each candidate fix. Even unlocking 2-3 of the 8 social sites would be strong signal.

---

## 6. PerimeterX press-and-hold (6-12 h, 2 sites)

**Already tracked** as `memory/open_tasks.md#65`. Hits zillow + wayfair.

PerimeterX serves a "press and hold" interstitial that requires sustained mousedown. The behavioral primitive exists at `crates/stealth/src/sigma_lognormal.rs` (humanized motion). Missing: PerimeterX wire protocol — how the press-duration is reported back, what cookies are issued.

**Approach**: capture the network trace from a real Chrome browser solving zillow's press-and-hold; reverse-engineer the POST format. No public reference — proprietary.

**Risk**: medium — requires manual capture and analysis. The behavioral simulation is already done.

---

## 7. Cloudflare Turnstile diagnosis (4-6 h, 1 site immediately + others)

Hits **udemy** (only Cloudflare-CHL in the sweep). Cloudflare Turnstile is the modern replacement for Cloudflare's old "Just a moment" interstitial — it's a non-interactive captcha that runs JS challenges.

**Approach**:
1. Capture Turnstile challenge JS from udemy with Playwright MCP.
2. Diff against our shim outputs to find the differing fingerprint surface.
3. Likely candidates: `crypto.subtle` algorithm support list, WASM SIMD support, font enumeration timing, font canvas fingerprint.

This is lower priority because only 1 site hit Turnstile in the sweep, but Cloudflare is rapidly migrating customers from JS challenges to Turnstile, so investing here pays off as more sites adopt it.

---

## 8. Streaming platform fingerprint diagnosis (1-2 days, ~6 sites)

**Hits**: netflix, disneyplus, hulu, spotify, twitch, youtube, prime-video — all captcha-CHL except Vimeo (PASS).

These DRM-adjacent platforms all check:
- **EME / MediaKeys API** support (Widevine probe). Our shim may report wrong supported keysystems.
- **MediaCapabilities** API (`navigator.mediaCapabilities.decodingInfo`)
- **Audio context fingerprint** — already partially addressed (Item 5 of last session diagnosed the kernel threshold-response gap; not yet fixed)
- **Picture-in-Picture support**

**Approach**: same as #5 — capture real Chrome's JS execution on netflix.com homepage, identify the EME/MediaCapabilities probes, ensure our shims return Chrome-coherent values.

Vimeo passing while everyone else fails suggests Vimeo doesn't check Widevine specifically. Other 7 do.

---

## 9. Diagnose unexplained BLOCKs (2-4 h, 3 sites)

Some BLOCKs are not obviously vendor-attributable:
- **cloudflare.com** itself — `BLOCKED 1 MB body` — strange; Cloudflare's own homepage shouldn't 403 us. Possibly a Cloudflare internal challenge.
- **coursera** — `BLOCKED 800 KB body` — similar suspicious-real-render pattern
- **bofa** — `BLOCKED 345 KB body` — likely a bot-detection header

**Approach**: capture full HTTP response headers + first 1 KB of body for each. Look for the actual block trigger (CSP redirect? meta refresh? specific header?). Could be classification false positives that the heuristic fix in #1 resolves automatically.

---

## Cleanup / housekeeping

- `crates/js_runtime/src/extensions/dom_ext.rs` has accumulated warnings; the new `op_dom_get_attribute_names` op is clean but the file overall could use `cargo clippy --fix`.
- The `holistic_sweep.rs` test file is heavy (~150 lines of boilerplate). Consider extracting the `site!` macro and `fetch_one` helper into a shared `tests/common.rs` module reused by `chl_sites.rs` too.
- Pre-existing clippy warnings (`crates/stealth/src/kasada.rs`, `crates/css_*`, etc.) on main are unrelated to this session but could be one cleanup PR.
- Remove the `eprintln!` in `crates/dom/src/arena.rs::append_child` cycle assertion if no cycle eprintlns appear in 30 days of testing — it's a fail-safe diagnostic, not production logging.

---

## Out of scope (deliberate)

These are explicitly NOT next steps because they have known structural barriers, not just engineering work:

- **Kasada strict tier** (canadagoose, hyatt, realtor) — server-side reputation gating per `memory/critical_findings.md`. Even byte-identical TLS + cookies don't get past 1-AA. Needs warmed-IP service or KaaS solver, which is operational not engine work.
- **adidas full Akamai** — partially covered by #2 above. The chl-known status will remain until #2 is implemented.
- **Captcha solving** as a general capability — out of scope for this engine; integration with 3rd-party services (2Captcha etc.) is a separate product.
- **DRM / Widevine** real implementation — would require a Widevine CDM library license. Engine can shim the JS APIs (#8) but cannot actually decrypt DRM content.

---

## Suggested order for the next session

If picking up tomorrow with one full day:
1. **Morning**: Item #1 (1-2 h, classification cleanup + re-run sweep to get accurate baseline)
2. **Mid-day**: Item #2 (4-8 h, Akamai BMP — biggest single-PR ROI)
3. **End of day**: Validate Akamai by re-running just the affected sites (~10 min); update `memory/open_tasks.md` to mark #64 closed.

If picking up tomorrow with half a day:
1. Item #1 only — gives much cleaner reporting and surfaces any false-positive sites that were actually passing.

If a 2-day block is available:
1. Day 1: Items #1 + #2.
2. Day 2: Item #5 (social diagnosis) — the highest engine-surface payoff. Requires Playwright MCP capture + JS shim diff.
