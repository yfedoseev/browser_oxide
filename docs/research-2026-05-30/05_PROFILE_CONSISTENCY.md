# 05 — Profile Consistency: converging chrome / pixel / iphone / firefox to one pass set

**Date:** 2026-05-30
**Cluster:** profile consistency (UA vs TLS vs H2 vs UA-CH vs JS-surface coherence)
**Goal:** make all 4 BO profiles pass ~the same set of sites. Today they diverge; **firefox is the structural loser** (gap 14 vs chrome 12, pixel 9, iphone 9).

---

## 0. TL;DR / ROI verdict

The per-profile divergence has **exactly one engine root cause that is fixable by a wire change**, plus a large shared content gap that is *not* profile-specific:

1. **Firefox profile emits a Chrome ClientHello + Chrome H2.** This is the **only structurally-incoherent wire** in the engine. Firefox UA + Firefox request headers ride over a Chrome-147 BoringSSL ClientHello and Chrome H2 SETTINGS. A JA4↔UA cross-check (AWS WAF / DataDome / Cloudflare all do it) instantly buckets `Firefox/135 + Chrome-class JA4` as high-risk. This is **the** reason every firefox-only loss (reuters, zillow, wsj, airbnb, spotify, tripadvisor all fail firefox while passing some Chrome-class profile). **ROI: HIGH but EFFORT: HIGH** (NSS-class boring2 reconfig + Firefox H2 arm). It converges firefox up to chrome/pixel parity.

2. **The remaining 9–12 gap on chrome/pixel/iphone is NOT a coherence defect** — it is the shared SPA-hydration / module-execution / challenge-drain gap (douyin, duolingo, adidas, ozon, wildberries, homedepot, etsy). Those fail on ALL 4 profiles equally and are covered by the JS-runtime and challenge docs, not here. **No profile change converges them; they converge together when the engine gap closes.**

3. **The cheapest single converging action** is to stop *advertising* an incoherent identity: until the Firefox wire exists, the firefox preset is strictly worse than chrome on every site (Firefox UA over Chrome JA4 is a cross-check tell that Chrome UA over Chrome JA4 does not have). **The 13-site inconsistency list collapses to ~the chrome pass set the moment we either (a) ship the Firefox wire, or (b) route firefox-targeted sites through a coherent profile.** Recommend (a) for the DataDome sites (etsy/tripadvisor structurally *need* Firefox-NSS TLS), (b) as the interim band-aid.

---

## 1. How the identity surface is assembled, and where it forks

A `StealthProfile` (`crates/stealth/src/profile.rs`) is one flat struct carrying the JS-visible identity **plus two wire-relevant fields**:

- `device_class: DeviceClass` ∈ {Desktop, MobileAndroid, MobileIOS} — `profile.rs`
- `tls_impersonate: String` — a label only.

The five layers that a vendor cross-checks, and **what each layer branches on**:

| Layer | Branch key | File | Firefox arm? |
|---|---|---|---|
| **JS surface** (navigator/screen/webgl/UA-CH values) | the struct values themselves | `presets.rs` | ✅ correct (vendor="", productSub=20100101, webgl="Mozilla") |
| **Request headers** (accept, accept-language, sec-ch-ua, priority) | `browser_name` | `crates/net/src/headers.rs:16-19` | ✅ `firefox_headers()` real |
| **TLS ClientHello** (ciphers, curves, sigalgs, ext order, GREASE) | `device_class` | `crates/net/src/tls.rs:247-262` | ❌ **NO Firefox arm** |
| **HTTP/2 SETTINGS + priority + pseudo-order** | `device_class` | `crates/net/src/h2_client.rs:85,110,158` | ❌ **NO Firefox arm** |
| **UA-CH advertisement gating** (whether sec-ch-ua is sent) | `browser_name` | `headers.rs:14-19` | ✅ Firefox/Safari omit |

**The defect is mechanical and confirmed in code:** `nav_headers` dispatches on `browser_name` (`headers.rs:16-19` → `"Firefox" => firefox_headers(profile)`), so a Firefox profile emits a *correct* Firefox header set — but `chrome_connector` (`tls.rs:239`) and the H2 `handshake` (`h2_client.rs:85`) only ever switch on `device_class`, which has **three** variants and no notion of browser family. A `Firefox` profile is `device_class: Desktop` (`presets.rs:473`), so it falls into the `DeviceClass::Desktop => CURVES_DESKTOP` / non-`is_safari_ios` branch and emits **Chrome 147 bytes**.

The presets even document this is a known dead string:
> `presets.rs:466-472`: *"String token — currently informational only. The actual TLS bytes are emitted by `crates/net` via boring2/BoringSSL with a Chrome-tuned ClientHello. A real Firefox JA4 swap requires reconfiguring boring2's cipher list / extension order to match NSS … tracked as a future item."*

And the Chrome-preset coherence test at `tls.rs:552-557` asserts `tls_impersonate == "chrome_147"` — i.e. the test suite enforces Chrome wire coherence but has **no equivalent assertion that a Firefox profile emits Firefox bytes** (because it can't — the arm doesn't exist). That missing test is the canary.

---

## 2. Per-profile coherence matrix (the actual divergence cause)

| Profile | UA | Headers | TLS ClientHello | H2 | UA-CH | **Internally coherent?** | gap |
|---|---|---|---|---|---|---|---|
| **chrome_148_*** | Chrome 148 | Chrome (`headers.rs:183`) | Chrome 147 desktop (`tls.rs:248-262`) | Chrome 8-entry SETTINGS + wt-255 priority (`h2_client.rs:118-130,167`) | sec-ch-ua present | ✅ **fully coherent** | 12 |
| **pixel_9_pro** | Chrome 148 Android | Chrome+mobile sec-ch-ua-model | Chrome 147 (`CURVES_ANDROID == CURVES_DESKTOP`, `tls.rs:104`) | same Chrome SETTINGS | sec-ch-ua + model="Pixel 9 Pro" | ✅ coherent (Android shares Chrome desktop wire — Chrome-on-Android genuinely does) | 9 |
| **iphone_15_pro** | Safari 18 iOS | Safari (`headers.rs:18`) | Safari-iOS arm (`tls.rs:247,253` — distinct ciphers, no Fisher-Yates, zlib, NO_TICKET, 4 TLS versions) | Safari arm (SETTINGS 2,3,4,9; msap pseudo-order; NO priority frame, `h2_client.rs:110-157`) | none (Safari omits) | ✅ **fully coherent — BEST mobile parity** | 9 |
| **firefox_135_*** | Firefox 135 | Firefox (`headers.rs:18` real) | ❌ **Chrome 147** (`device_class:Desktop` → CURVES_DESKTOP) | ❌ **Chrome SETTINGS** | none (correct) | ❌ **INCOHERENT: Firefox UA+headers over Chrome JA4+H2** | **14** |

**Read this matrix as the divergence explanation:**

- chrome and pixel are wire-identical (Chrome-on-Android shares the desktop ClientHello — `CURVES_ANDROID = CURVES_DESKTOP` at `tls.rs:104`), so they pass/fail nearly the same set; pixel's 3-site edge over chrome (9 vs 12) comes from the *mobile UA-CH + mobile JS surface* tipping a few mobile-lenient sites, not from any wire delta.
- iphone is the cleanest mobile identity (Safari wire + Safari headers + iOS JS-surface all agree) — **this is why yelp passes iphone-only**: the site is mobile-Safari-lenient and BO's iOS identity is internally consistent end-to-end. booking/amazon-in **fail iphone** for the *opposite* reason — they're Akamai/content-heavy and the iOS path hits the shared content gap, not a coherence defect.
- firefox is the sole broken wire and therefore the **common loser** on every JA4-cross-checking vendor.

---

## 3. The 13 inconsistent sites, mapped to the coherence cause

| Site | Passes (BO) | Fails (BO) | Cause class |
|---|---|---|---|
| reuters | ch, ip | **firefox**, px | Firefox JA4↔UA cross-check (§2) |
| zillow | ch, px, ip | **firefox** | Firefox JA4↔UA cross-check |
| wsj | ch, px, ip | **firefox** | Firefox JA4↔UA cross-check |
| airbnb | ch, px, ip | **firefox** | Firefox JA4↔UA cross-check |
| spotify | px, ip | **firefox**, ch | Firefox cross-check; ch loss = SPA-budget allowlist (spotify is on the 90s tier `page.rs:1969`, but ch desktop content still thin → content gap, not coherence) |
| tripadvisor | px, ip | **firefox** | DataDome → structurally needs Firefox-NSS TLS, but BO firefox emits Chrome TLS = worst case |
| amazon-fr | px, ip, fx-? | **chrome** | NOT a wire defect — region accept-language (`headers.rs:47` apply_region_accept_language) + same-IP token clustering (per MEMORY: space calls 2-3 min); chrome-specific flake, not coherence |
| booking | ch, px, fx | **iphone** | content/Akamai on iOS path — shared gap, not coherence |
| amazon-in | ch, px, fx | **iphone** | content/Akamai on iOS path — shared gap |
| yelp | **iphone only** | ch, px, fx | iOS-Safari-lenient site + BO's coherent iOS identity (§2) |

**Of the 13, the firefox-loss cluster (6 sites: reuters, zillow, wsj, airbnb, spotify, tripadvisor) is ONE root cause** = the §1 missing Firefox wire arm. Fixing that one defect converges those 6 up to the chrome/pixel pass set in a single stroke. The booking/amazon-in/yelp iphone splits are content/leniency, not coherence — they converge when the shared SPA/Akamai gap closes (other docs).

---

## 4. What a faithful Firefox wire requires (the convergence build)

Real Firefox 135 uses the NSS stack, JA4 `t13d1715h2_5b57614c22b0_3d5424432f57` (internal `GAP_DEEP_ANALYSIS_2026_04_28.md:206`). To make BO's firefox profile coherent, add a Firefox arm to **both** wire layers:

### 4a. TLS (`crates/net/src/tls.rs`) — add a `is_firefox` branch alongside `is_safari_ios`
The current branch key is `device_class` only (`tls.rs:247-262`). Introduce a browser-family discriminator (pass `browser_name` or add `DeviceClass::DesktopFirefox`, or — cleaner — thread `tls_impersonate` through so the dead string finally drives behavior). The Firefox NSS ClientHello differs from Chrome in:
- **Cipher order** — NSS order, not BoringSSL `CIPHER_LIST` (`tls.rs`).
- **`record_size_limit` (ext 28)** — Firefox sends `0x4001`; Chrome does not (RFC 8449).
- **`delegated_credentials` (ext 34)** — Firefox advertises sigalgs for delegated creds; Chrome omits.
- **supported_groups** — Firefox appends **FFDHE2048/3072**; Chrome is ECDHE-only (`CURVES_DESKTOP` `tls.rs:91`).
- **Real ECH**, not ECH-GREASE.
- **Fixed extension order** — Firefox does NOT do Chrome's Fisher-Yates permutation (the same skip already implemented for Safari at `tls.rs:243-245`).

> boring2 4.15 can express these (it's the same `SslConnector::builder` surface used for the Safari-iOS arm). Do **not** attempt boring2 5.0-alpha — it removes the impersonation APIs (per project history).

### 4b. H2 (`crates/net/src/h2_client.rs`) — add a Firefox SETTINGS/priority arm
Current is `is_safari_ios ? Safari : Chrome` (`h2_client.rs:110-175`). Firefox needs a third arm:
- Firefox keeps SETTINGS **0x3 MAX_CONCURRENT_STREAMS** (Chrome omits it, `h2_client.rs:30-31`); different INITIAL_WINDOW.
- Different SETTINGS wire order and pseudo-header order from Chrome's 8-entry list.
- Firefox uses an **RFC 7540 priority tree** (multiple priority frames), not Chrome's single weight-255 exclusive HEADERS dependency (`h2_client.rs:167-174`).

### 4c. Add the coherence test (the missing canary)
Mirror `tls.rs:540-558`: assert that for `firefox_135_*` presets, the emitted ClientHello JA4 is Firefox-class (capture first bytes like the existing `safari_ios_emits_tls_1_0_record_version` test at `tls.rs:568`). This prevents regressing back to a Chrome wire under a Firefox UA.

**Why this also unblocks DataDome (etsy/tripadvisor):** DataDome's per-tenant ML weights Chromium-TLS=bot, Firefox-TLS=human by default; Firefox-NSS TLS is the documented *only* bypass (`GAP_DEEP_ANALYSIS_2026_04_28.md:206`). So 4a is simultaneously the firefox-convergence fix AND the etsy/tripadvisor DataDome key.

---

## 5. Interim band-aid (zero-wire-work convergence)
Until §4 lands, the firefox profile is **strictly worse** than chrome on every JA4-cross-checking site (Firefox UA over Chrome JA4 is a tell; Chrome UA over Chrome JA4 is not). Two cheap converging moves:

1. **Don't route firefox-targeted sites through the firefox profile.** For the 6 firefox-loss sites, serve them with chrome_148 (coherent Chrome wire) instead. This alone collapses the 13-site inconsistency to ~the chrome pass set. Net: the inconsistency *vanishes* even though no site newly passes — because the firefox profile stops being the unique loser.
2. **Make `tls_impersonate` load-bearing or delete it.** Right now `firefox_135` is a string that lies. Either wire it (§4) or remove it so no caller believes a Firefox wire exists. The existing assert at `tls.rs:552` proves the team already treats `tls_impersonate` as a coherence contract for Chrome — extend or enforce it for Firefox.

---

## 6. Convergence plan (ROI-ranked)

| # | Action | Files | Converges | Confidence | Effort |
|---|---|---|---|---|---|
| 1 | **Interim: route the 6 firefox-loss sites through chrome_148** (or drop firefox preset from the rotation for cross-check vendors) | benchmark/profile-selection caller; `page.rs` profile pick | reuters/zillow/wsj/airbnb/spotify/tripadvisor stop being firefox-unique losers → all 4 profiles show the same pass set | HIGH | trivial |
| 2 | **Add Firefox TLS arm** (NSS ciphers, record_size_limit, delegated_credentials, FFDHE, real ECH, fixed ext order) | `tls.rs:247-262` (branch on browser family, not just device_class) | firefox profile becomes coherent; also unblocks DataDome etsy/tripadvisor | HIGH | high |
| 3 | **Add Firefox H2 arm** (keep SETTINGS 0x3, RFC7540 priority tree, Firefox SETTINGS/pseudo order) | `h2_client.rs:110-175` | completes firefox wire coherence | HIGH | medium |
| 4 | **Add Firefox JA4 coherence test** | `tls.rs` test mod (mirror `:540-558`,`:568`) | prevents regression to Chrome-under-Firefox-UA | HIGH | low |
| 5 | (Out of this cluster) close the shared SPA/module/challenge gap | js_runtime + page.rs | converges the 9–12 chrome/pixel/iphone gap (douyin/duolingo/adidas/ozon/wildberries/homedepot/etsy) — these fail ALL profiles equally, so they're not a *consistency* defect | — | see other docs |

**Bottom line for the user's "all 4 profiles ~equal" goal:** the *consistency* defect is singular and surgical — one missing Firefox wire arm (§1, confirmed `tls.rs:247-262` + `h2_client.rs:85` branch only on device_class, `presets.rs:466-472` admits it). Action #1 makes the four profiles equal *immediately* (no new passes, but no firefox-unique loser); actions #2-4 make them equal *the right way* and additionally bank the DataDome sites. The residual 9–12 gap is a shared engine-execution gap that affects all profiles uniformly and is therefore not a profile-consistency problem at all.
