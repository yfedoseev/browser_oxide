# 11 — Per-profile strategy & routing

**Audience:** anyone changing engine behaviour that may bias one profile over another, anyone wiring a new public-facing sweep / scrape job, and anyone proposing a new stealth profile.

**One-paragraph thesis:** BO ships 4 stealth profiles. Each one wins different sites; together they routed-pass 108/126 strict, vs the best single profile (pixel_9_pro_chrome_148) at 102. The 6-site routing premium is real and load-bearing — every shipped consumer should run a small profile rotation, not a single hard-coded preset. This doc lists every profile, its measured wins/losses, the cross-profile routing rules a caller should implement, and how to extend the set with new profiles in v0.1.x.

---

## 1. Profile catalog

The four shipped profiles. Each card is filled from the source-of-truth Rust preset constructor; YAML callers get the same values via `StealthProfile::load_from_file()`. The fields below are NOT exhaustive — see `crates/stealth/src/profile.rs:33-180` for every field.

### 1.1 chrome_148_macos — Apple Silicon desktop Chrome

Source: `crates/stealth/src/presets.rs:120-196` (Rust) · `crates/stealth/profiles/chrome_148_macos.yaml:1-86` (YAML — documented as the schema reference).

| Field | Value |
|---|---|
| `user_agent` | `Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36` |
| `browser_name` / `browser_version` | `Chrome` / `148.0.7778.168` |
| `os_name` / `os_version` | `macOS` / `15.2` |
| `platform` (navigator.platform) | `MacIntel` |
| `vendor` | `Google Inc.` |
| `product_sub` | `20030107` (Chrome's frozen value) |
| `sec-ch-ua` brand list | derived in `crates/net/src/headers.rs:415` from `browser_version` (major=148) |
| `sec-ch-ua-platform` | `"macOS"` |
| `sec-ch-ua-mobile` | `?0` |
| `sec-ch-ua-arch` / `-bitness` | `"arm"` / `"64"` |
| `sec-ch-ua-platform-version` | `"15.2.0"` |
| `sec-ch-ua-full-version-list` | full `148.0.7778.168` (per `headers.rs:401-413`) |
| `navigator.userAgentData.mobile` | `false` |
| `device_pixel_ratio` | `2.0` |
| `screen` | 1512×982, avail 1512×949, top 33, color-depth 30 |
| `inner` / `outer` | 1512×871 / 1512×982 |
| `hardware_concurrency` | 8 |
| `device_memory` | 8 |
| `max_touch_points` | 0 |
| `webgl_vendor` | `Google Inc. (Apple)` |
| `webgl_renderer` | `ANGLE (Apple, ANGLE Metal Renderer: Apple M3, Unspecified Version)` |
| `gpu_profile` | `apple_m3_macos` (see `crates/stealth/src/gpu.rs`) |
| `color_gamut` | `p3` (wide gamut — display-P3) |
| `pointer_type` / `hover` | `fine` / `hover` |
| `has_platform_authenticator` | `true` (Touch ID) |
| `tls_impersonate` codename | `chrome_147` — see §7 for why the label is 147 while the UA is 148 |
| TLS curves | `X25519_MLKEM768, X25519, SECP256R1, SECP384R1` (`tls.rs:91-96`) |
| TLS extension permutation | Fisher-Yates shuffled per handshake, 16 extensions (`tls.rs:203-228`) |
| `language` / `languages` | `en-US` / `["en-US", "en"]` |
| `timezone` | `America/Los_Angeles` |

### 1.2 pixel_9_pro_chrome_148 — Pixel-class Android Chrome

Source: `crates/stealth/src/presets.rs:672-772`.

| Field | Value |
|---|---|
| `user_agent` | `Mozilla/5.0 (Linux; Android 15; Pixel 9 Pro Build/AP4A.250105.002) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Mobile Safari/537.36` |
| `browser_name` / `browser_version` | `Chrome` / `148.0.7778.168` |
| `os_name` / `os_version` | `Android` / `15` |
| `platform` | `Linux armv81` |
| `vendor` | `Google Inc.` |
| `sec-ch-ua-mobile` | `?1` (mobile) |
| `sec-ch-ua-platform` | `"Android"` |
| `sec-ch-ua-model` | `"Pixel 9 Pro"` (display name — NOT the codename `tokay`) |
| `sec-ch-ua-form-factors` | `"Mobile"` |
| `sec-ch-ua-arch` | `""` (empty on Android per UA-CH reduction) |
| `device_pixel_ratio` | `2.625` (fractional — Pixel 9 Pro hardware) |
| `screen` | 412×870 (CSS px) |
| `inner` / `outer` | 412×870 / 412×870 (no chrome bars on mobile JS surface) |
| `hardware_concurrency` | 8 |
| `device_memory` | 8 |
| `max_touch_points` | 5 |
| `webgl_vendor` | `Google Inc. (Google)` |
| `webgl_renderer` | `ANGLE (Google, Mali-G715 MP7, OpenGL ES 3.2)` (Tensor G4 GPU) |
| `pdf_viewer_enabled` / `plugins_count` | `false` / `0` ← **single biggest mobile-vs-desktop tell on Chromium** |
| `pointer_type` / `hover` | `coarse` / `none` |
| `has_platform_authenticator` | `false` (Passkeys exist but fresh-profile reports false) |
| `tls_impersonate` codename | `chrome_147_android` |
| TLS curves | `X25519_MLKEM768, X25519, SECP256R1, SECP384R1` (Android == desktop per `tls.rs:104`; verify against a fresh Pixel capture if a vendor flags this) |
| `language` / `languages` | `en-US` / `["en-US", "en"]` |

### 1.3 iphone_15_pro_safari_18 — iPhone-class Mobile Safari

Source: `crates/stealth/src/presets.rs:795-875`.

| Field | Value |
|---|---|
| `user_agent` | `Mozilla/5.0 (iPhone; CPU iPhone OS 18_0_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0.1 Mobile/15E148 Safari/604.1` |
| `browser_name` / `browser_version` | `Safari` / `18.0.1` |
| `os_name` / `os_version` | `iOS` / `18.0.1` |
| `platform` | `iPhone` |
| `vendor` | `Apple Computer, Inc.` |
| `navigator.userAgentData` | `undefined` (Safari has NO UA-CH at all — no `sec-ch-ua*` headers emitted) |
| `device_pixel_ratio` | `3.0` (integer — iPhone 15 Pro) |
| `screen` | 393×852 |
| `hardware_concurrency` | **2** (Safari intentionally caps to limit entropy) |
| `device_memory` | undefined (WebKit doesn't expose it; preset value is sentinel 0) |
| `max_touch_points` | 5 |
| `webgl_vendor` | `Apple Inc.` |
| `webgl_renderer` | `Apple GPU` (Apple strips GPU model from WebGL — literal constant) |
| `pdf_viewer_enabled` / `plugins_count` | `false` / `0` |
| `pointer_type` / `hover` | `coarse` / `none` |
| `color_gamut` | `p3` |
| `tls_impersonate` codename | `safari_18_ios` — distinct cipher list, sigalgs, curve order |
| TLS cipher list | 20 ciphers including legacy 3DES (per Apple's `safari_18.0_iOS.yaml` reference), `tls.rs:111-132` |
| TLS sigalgs | 10 entries incl. duplicated `rsa_pss_rsae_sha384` Apple bug, `tls.rs:137-148` |
| TLS curves | `X25519, SECP256R1, SECP384R1, SECP521R1` — NO post-quantum, P-521 added vs desktop, `tls.rs:152-157` |
| TLS extension permutation | FIXED order (no Fisher-Yates), 13 extensions per `safari_18.0_iOS.yaml`, `tls.rs:169-183` |
| TLS min version | TLS 1.0 (Safari advertises 1.0/1.1/1.2/1.3 in `supported_versions` — see `tls.rs:280-289`) |
| Missing JS APIs (deliberate) | `navigator.connection`, `navigator.bluetooth`, USB/Serial/HID/Sensor/Battery/MIDI/IdleDetector, WebGPU, `PaymentRequest.prototype.hasEnrolledInstrument` |
| Present iOS-only APIs | `window.orientation === 0`, `DeviceMotionEvent.requestPermission` static, `'ontouchstart' in window` |

### 1.4 firefox_135_macos — Apple Silicon desktop Firefox

Source: `crates/stealth/src/presets.rs:413-495`.

| Field | Value |
|---|---|
| `user_agent` | `Mozilla/5.0 (Macintosh; Intel Mac OS X 14.5; rv:135.0) Gecko/20100101 Firefox/135.0` |
| `browser_name` / `browser_version` | `Firefox` / `135.0` |
| `os_name` / `os_version` | `macOS` / `14.5` |
| `platform` | `MacIntel` |
| `vendor` | `""` (empty — Firefox does NOT advertise `Google Inc.`) |
| `product_sub` | `20100101` (Firefox uses a Gecko build date; Chrome uses `20030107`) |
| `sec-ch-ua*` headers | **NONE EMITTED** — Firefox doesn't speak UA-CH |
| `device_pixel_ratio` | `2.0` |
| `screen` | 1440×900, avail 1440×875, top 25 |
| `hardware_concurrency` | 10 |
| `device_memory` | 16 |
| `webgl_vendor` | `Mozilla` (Firefox 113+ masks WebGL — both vendor AND renderer) |
| `webgl_renderer` | `Mozilla` |
| `color_gamut` | `p3` |
| `tls_impersonate` codename | `firefox_135` (string only — see §7 caveat) |
| `language` / `languages` | `en-US` / `["en-US", "en"]` |
| `timezone` | `America/Los_Angeles` |

### 1.5 Internal consistency rules (validated)

Every preset is checked by `StealthProfile::validate()` and a set of test gates in `presets.rs:877-1016`:

- `chrome_148_*_validates` — schema/coherence
- `http3_disabled_by_default_on_all_presets` (line 887) — gap #33: vanilla `quinn-proto 0.11` emits randomized transport_parameters; advertising `h3` is a *worse* fingerprint than not speaking it
- `firefox_webgl_is_masked` (line 954) — `vendor` and `renderer` must both be `"Mozilla"`
- `webdriver_not_in_profile` (line 992) — no preset accidentally includes the string `webdriver`
- `ua_contains_version` (line 1000) — UA must report the reduced `148.0.0.0` form; full version lives only in `browser_version` (Chrome's UA-reduction policy since v110)

---

## 2. Per-profile pass-rate summary

Source data: `~/projects/browser_oxide_internal/benchmarks/baselines/2026-05-24/bo_*_cold.json`. Strict Pass = `tag == "L3-RENDERED" AND len ≥ 15000`. n=126.

### 2.1 Totals

| Profile | Pass | ThinShell | CHL | ThinBody | Error | Loose L3 |
|---|--:|--:|--:|--:|--:|--:|
| **routed best-of-4** | **108** | 12 | 5 | 3 | 0 | 120 |
| pixel_9_pro_chrome_148 | **102** | 16 | 6 | 4 | 0 | 118 |
| firefox_135_macos | 101 | 14 | 9 | 4 | 0 | 115 |
| chrome_148_macos | 99 | 17 | 7 | 5 | 0 | 116 |
| iphone_15_pro_safari_18 | 98 | 15 | 11 | 4 | 0 | 113 |

Pixel is the single best profile by 1-4 sites. The routed delta (108 - 102 = +6) is the load-bearing reason this doc exists.

### 2.2 Per-category breakdown (every category, every profile)

| Category | n | chrome | pixel | iphone | firefox | Notes |
|---|--:|--:|--:|--:|--:|---|
| stores | 17 | 13 | 14 | 14 | 14 | etsy / bestbuy / homedepot leak across profiles |
| misc | 12 | 9 | 9 | 8 | 9 | yelp passes only on iphone; weather/udemy fail on iphone |
| news | 10 | 10 | 10 | 8 | 9 | iphone loses to CF on economist/ft; firefox loses wsj |
| social | 10 | 8 | 8 | 7 | 8 | reddit + x-com universal misses |
| antibot | 10 | 9 | 9 | 9 | 9 | areyouheadless universal miss (probe by design) |
| tech | 9 | 9 | 8 | 8 | 9 | pixel loses apple; iphone loses openai (CF) |
| travel | 8 | 5 | 7 | 7 | 5 | skyscanner/tripadvisor flip to mobile; booking universal miss |
| amazon | 8 | 1 | 1 | 2 | 3 | the largest single-category gap vs Camoufox (6) |
| streaming | 8 | 7 | 8 | 8 | 7 | spotify flips to mobile |
| search | 8 | 8 | 8 | 7 | 8 | iphone loses ecosia (CF) |
| ru | 6 | 5 | 5 | 5 | 5 | wildberries universal miss |
| gov-bank | 6 | 6 | 6 | 6 | 6 | parity |
| reference | 5 | 5 | 5 | 5 | 5 | parity |
| chl-known | 5 | 1 | 1 | 1 | 2 | firefox uniquely passes adidas |
| realestate | 4 | 3 | 3 | 3 | 2 | firefox loses zillow (PerimeterX) |
| **TOTAL** | **126** | **99** | **102** | **98** | **101** | routed = 108 |

### 2.3 Where each profile wins uniquely

Computed from raw sweep JSON — sites that pass with **exactly one** BO profile (no other BO profile passes it):

| Profile | Unique-pass sites | Category | Verdict on other profiles |
|---|---|---|---|
| pixel | `amazon-ca` | amazon | chrome/iphone/firefox all return AWS WAF stub 2011b |
| iphone | `yelp` | misc | chrome/pixel/firefox all return DataDome-CHL 1424b |
| firefox | `adidas` | chl-known | chrome/pixel/iphone all return Akamai-CHL 2494b |
| firefox | `amazon-com` | amazon | chrome/pixel/iphone all return AWS WAF stub 2011-2014b |
| chrome | *(none unique)* | — | every chrome win is matched by ≥1 other profile |

That `chrome has no unique wins` row is significant: chrome_148_macos is **strictly Pareto-dominated** by the routing set. Running just chrome leaves 9 recoverable sites on the table (§2.4). Running just pixel leaves 6.

### 2.4 Per-profile losses recoverable via routing

Each row is a site where the named profile fails but at least one other profile succeeds.

#### chrome_148_macos misses (9 recoverable):
| Site | Category | Recovered by | Our verdict |
|---|---|---|---|
| amazon-ca | amazon | pixel | L3 2011b (AWS WAF stub) |
| amazon-com | amazon | firefox | L3 2011b |
| amazon-fr | amazon | iphone, firefox | L3 2011b |
| adidas | chl-known | firefox | L3 2494b (Akamai CHL) |
| yelp | misc | iphone | DataDome-CHL 1424b |
| wayfair | stores | pixel, iphone, firefox | **THIN-BODY 0b** — connection drop |
| spotify | streaming | pixel, iphone | L3 9590b (SPA shell) |
| skyscanner | travel | pixel, iphone | THIN-BODY 723b |
| tripadvisor | travel | pixel, iphone | DataDome-CHL 1430b |

#### pixel_9_pro_chrome_148 misses (6 recoverable):
| Site | Category | Recovered by | Our verdict |
|---|---|---|---|
| amazon-co-uk | amazon | chrome, iphone, firefox | L3 2011b |
| amazon-com | amazon | firefox | L3 2014b |
| amazon-fr | amazon | iphone, firefox | L3 2011b |
| adidas | chl-known | firefox | L3 2494b |
| yelp | misc | iphone | DataDome-CHL 1424b |
| apple | tech | chrome, iphone, firefox | **THIN-BODY 0b** |

#### iphone_15_pro_safari_18 misses (10 recoverable — **iphone has the worst routing tax**):
| Site | Category | Recovered by | Our verdict |
|---|---|---|---|
| amazon-ca | amazon | pixel | L3 2011b |
| amazon-com | amazon | firefox | L3 2014b |
| adidas | chl-known | firefox | L3 2494b |
| **udemy** | misc | chrome, pixel, firefox | **Cloudflare-CHL 5929b** |
| weather | misc | chrome, pixel, firefox | THIN-BODY 0b |
| **economist** | news | chrome, pixel, firefox | **Cloudflare-CHL 5869b** |
| **ft** | news | chrome, pixel, firefox | **Cloudflare-CHL 271064b** |
| **ecosia** | search | chrome, pixel, firefox | **Cloudflare-CHL 5444b** |
| **quora** | social | chrome, pixel, firefox | **Cloudflare-CHL 5843b** |
| **openai** | tech | chrome, pixel, firefox | **Cloudflare-CHL 10807b** |

**Key finding for iphone**: 6 of its 10 recoverable losses are Cloudflare interstitials. Cloudflare's "managed challenge" is harsher on iOS Safari class than on Chromium-class fingerprints. Treat iphone as a "specialist" profile (yelp wins, mobile-targeted SPA wins) rather than a generalist.

#### firefox_135_macos misses (7 recoverable):
| Site | Category | Recovered by | Our verdict |
|---|---|---|---|
| amazon-ca | amazon | pixel | L3 2011b |
| yelp | misc | iphone | DataDome-CHL 1424b |
| **wsj** | news | chrome, pixel, iphone | **DataDome-CHL 1427b** |
| **zillow** | realestate | chrome, pixel, iphone | **PerimeterX-PaH 14062b** |
| spotify | streaming | pixel, iphone | L3 9584b |
| skyscanner | travel | pixel, iphone | THIN-BODY 723b |
| tripadvisor | travel | pixel, iphone | DataDome-CHL 1430b |

Firefox loses to DataDome (wsj) and PerimeterX (zillow) that the other three profiles defeat — likely because BO's Firefox profile only has UA-level differentiation (no real Gecko/NSS TLS), see §7.

---

## 3. Routing analysis

### 3.1 Site distribution by profile-coverage

Bucket every site by **how many BO profiles pass it strictly**.

| Bucket | n sites | What it means | Action |
|---|--:|---|---|
| 4/4 (universal pass) | 88 | Every profile gets through | Run cheapest profile (pixel — lowest RSS at 388 MB) |
| 3/4 | 12 | One profile blocked | Either ignore that profile or two-shot |
| 2/4 | 4 | Two profiles work | Pick one of the two for that domain class |
| 1/4 (routing-required) | 4 | One profile unique-wins | Must route or you lose the site |
| 0/4 (residual) | 18 | No BO profile passes | Out of scope here — see chapters 05/06/07/08 |
| **total** | **126** | | |

### 3.2 The 4 routing-required sites (1/4 bucket)

Without routing, you lose these by definition.

| Site | Winner | URL | Recommended routing key |
|---|---|---|---|
| adidas | firefox | adidas.com/us | Akamai class → try firefox |
| amazon-ca | pixel | amazon.ca | amazon.* → try pixel first |
| amazon-com | firefox | amazon.com | amazon.* → try firefox second |
| yelp | iphone | yelp.com | DataDome-known list → try iphone |

### 3.3 The 12 three-profile sites (3/4 bucket — which profile to AVOID)

For these sites the routing key is *negative* — avoid the one losing profile.

| Site | Loser to avoid | Reason |
|---|---|---|
| amazon-co-uk | pixel | AWS WAF rolls hard on Pixel UA-CH set |
| apple | pixel | apple.com SPA bails on `sec-ch-ua-model: "Pixel 9 Pro"` |
| economist, ft, ecosia, openai, quora, udemy, weather | iphone | Cloudflare managed challenge — iOS Safari class is filtered |
| wsj | firefox | DataDome rejects Firefox UA without NSS-class TLS |
| zillow | firefox | PerimeterX rejects same |
| wayfair | chrome | Akamai variant drops chrome_148_macos (THIN-BODY) |
| zillow | firefox | (listed above) |

### 3.4 The 18 residual sites (0/4 bucket — no BO profile passes)

Documented for completeness. **Recoverable surface** = the 10 of these that Camoufox passes (see chapter 02). **Hard residual** = the other 8 (frontier in chapter 08).

| Site | Category | Camoufox passes? | Chapter |
|---|---|---|---|
| reddit | social | yes | 05 |
| duolingo | misc | yes | 05 |
| booking | travel | yes | 05 |
| douyin | chl-known | yes | 05 |
| imdb | misc | yes | 06 |
| amazon-de | amazon | yes | 06 |
| amazon-in | amazon | yes | 06 |
| amazon-com-au | amazon | yes | 06 |
| etsy | stores | yes | 07 |
| x-com | social | yes | 05 |
| amazon-jp | amazon | no | 08 |
| bestbuy | stores | no | 08 |
| homedepot | stores | no | 08 |
| canadagoose | chl-known | no | 08 |
| hyatt | chl-known | no | 08 |
| realtor | realestate | no | 08 |
| wildberries | ru | no | 08 |
| areyouheadless | antibot | no | 08 (intentional probe) |

---

## 4. Recommended routing rules (decision tree)

This section is the *prescription*. A caller wiring a public-facing scraper should implement this as a routing function and run it before the first navigate.

### 4.1 Default profile selection by domain

```
fn pick_first_profile(url: &Url) -> Profile {
    let host = url.host_str().unwrap_or("");

    // Rule 1: Amazon family — never use chrome_148_macos first
    // (it loses 3/8 amazon sites; pixel wins amazon-ca uniquely, firefox wins amazon-com)
    if host.starts_with("amazon.") || host.contains(".amazon.") {
        return Profile::Firefox135;  // wins 3/8, best amazon profile
    }

    // Rule 2: Known DataDome list (wsj, etsy, yelp, tripadvisor, ...)
    // iphone uniquely wins yelp; pixel/iphone win tripadvisor.
    if KNOWN_DATADOME.contains(host) {
        return Profile::IPhone15Pro;
    }

    // Rule 3: Known PerimeterX (zillow, ...)
    // Firefox loses; chrome/pixel/iphone tie.
    if KNOWN_PERIMETERX.contains(host) {
        return Profile::Pixel9Pro;  // cheapest RSS, passes
    }

    // Rule 4: Cloudflare-managed-challenge list — avoid iphone
    if KNOWN_CLOUDFLARE.contains(host) {
        return Profile::Chrome148Macos;  // safest CF-class
    }

    // Rule 5: Mobile-app-class SPAs (skyscanner, spotify, tripadvisor, ...)
    // Mobile UA gets a non-WAF serve; desktop sees full anti-bot stack.
    if KNOWN_MOBILE_FRIENDLY.contains(host) {
        return Profile::Pixel9Pro;
    }

    // Rule 6: Kasada-protected (canadagoose, hyatt, realtor)
    // Per memory/state_2026_05_16_*, Camoufox is the open SOTA on Kasada.
    // BO routed loses all three; if you must try, mobile profiles sometimes
    // get a softer challenge (pixel > iphone > chrome > firefox observed).
    if KNOWN_KASADA.contains(host) {
        return Profile::Pixel9Pro;  // fallback chain to firefox, chrome
    }

    // Default: pixel (single-best profile at 102 Pass; lowest RSS at 388 MB)
    Profile::Pixel9Pro
}
```

### 4.2 Fallback chain (when first profile fails)

If `pick_first_profile` returns a verdict ≠ Pass, try the next:

| Bucket | First | Second | Third | Fourth |
|---|---|---|---|---|
| amazon.* | firefox | iphone | pixel | chrome |
| DataDome | iphone | pixel | firefox | chrome |
| PerimeterX | pixel | chrome | iphone | (skip firefox) |
| Cloudflare | chrome | pixel | firefox | (skip iphone) |
| mobile-friendly SPA | pixel | iphone | firefox | chrome |
| Kasada | pixel | firefox | chrome | iphone |
| default | pixel | firefox | chrome | iphone |

### 4.3 Parallel best-of-N strategy

For latency-critical callers that can afford the wasted work, the **fastest-first-success** pattern dominates:

- Start chrome + pixel in parallel (covers 105 sites at first response — `chrome_pass ∪ pixel_pass` per the data)
- If both fail at strict-Pass classifier, fire iphone + firefox in parallel
- Cancel slower runs on first success

Expected per-page latency improvement: median + p95 unchanged (the fast profile is the bottleneck either way); p99 drops because the long-tail-failing profile is cut early.

### 4.4 Caller-owned known-blocker lists

These four small sets are the load-bearing inputs to the decision tree above. Ship them in `crates/browser/src/router.rs` (new file, v0.1.0 deliverable; see Acceptance §8):

| Set | Members (sites that exhibit the pattern in our 126 sweep + general industry knowledge) |
|---|---|
| `KNOWN_DATADOME` | etsy.com, tripadvisor.com, wsj.com, yelp.com, glassdoor.com, leboncoin.fr |
| `KNOWN_PERIMETERX` | zillow.com, crunchbase.com, ticketmaster.com, expedia.com |
| `KNOWN_CLOUDFLARE` | ecosia.com, economist.com, ft.com, openai.com, quora.com, udemy.com, weather.com |
| `KNOWN_KASADA` | canadagoose.com, hyatt.com, realtor.com, twitch.tv (suspected) |
| `KNOWN_MOBILE_FRIENDLY` | skyscanner.com, spotify.com, wayfair.com |
| `KNOWN_AWS_WAF` | amazon.* (all locales), imdb.com, audible.com |

---

## 5. Per-profile weak spots (root-cause summary)

### 5.1 chrome_148_macos — 99 Pass — "the obvious-default profile"

**Weak categories**: amazon (1/8), travel (5/8), stores (13/17). **No unique wins** (§2.3).

**Why it loses what it loses**:
- amazon family: chrome desktop UA-CH set is the most-fingerprinted bot signal in the world; AWS WAF rolls hard on this class.
- skyscanner / tripadvisor: desktop SPAs assume full hydration budget; our cold-isolate path occasionally drops the SPA bootstrap (see chapter 10 timing).
- wayfair: Akamai variant rejects the chrome 148 macOS canvas/audio seed pair (`0xabcdef1234567890 / 0x0987654321fedcba`) — observed THIN-BODY 0b (full connection drop).

**Action**: Keep as the *reference* desktop profile; don't ship as the default for new callers. Use as the "Cloudflare-safe fallback" in §4.

### 5.2 pixel_9_pro_chrome_148 — 102 Pass — "the best single profile"

**Weak categories**: amazon (1/8 — but uniquely wins amazon-ca), tech (apple THIN-BODY).

**Why it loses what it loses**:
- apple.com: their SPA does a server-side branch on `sec-ch-ua-model: "Pixel 9 Pro"` and returns an empty body (likely a redirect to the iOS App Store deep link that we don't follow).
- amazon-co-uk / amazon-fr: pixel's UA-CH set (`sec-ch-ua-mobile: ?1` + `sec-ch-ua-platform: "Android"`) is on a separate WAF risk class that scores worse than desktop chrome here.

**Action**: This is the **recommended default profile** for v0.1.0. Lowest RSS (388 MB), highest single-profile Pass (102), wide category coverage.

### 5.3 iphone_15_pro_safari_18 — 98 Pass — "the specialist"

**Weak categories**: misc (8/12), news (8/10), search (7/8), tech (8/9). **Pattern**: Cloudflare managed challenge dislikes iOS Safari class. 6 of iphone's 10 recoverable losses are Cloudflare interstitials (§2.4).

**Why this happens**:
- Cloudflare's risk model weights `cf-bm` cookie + JA4 + UA combination. iOS Safari produces a JA4 that's "Safari on mobile" — a much smaller real-traffic share than Chrome desktop, so the model has fewer confident-real samples to anchor on, and falls back to challenge.
- BO's iOS profile has distinct TLS (`safari_18_ios` codename, distinct ciphers/sigalgs/curves per `tls.rs:111-183`) — so the JA4 is plausibly real. The problem is the *combination* (JA4 + UA + no UA-CH + traffic source) doesn't match a CF-trusted-class.

**Action**: Specialist routing only. Use for: yelp (uniquely passes), spotify/skyscanner/tripadvisor mobile-friendly SPAs, any caller targeting iOS-specific UX. Do **NOT** use as default — Cloudflare losses bleed into routine pass-rate.

### 5.4 firefox_135_macos — 101 Pass — "the contrarian"

**Weak categories**: travel (5/8), realestate (2/4 — loses zillow), news (9/10 — loses wsj). Uniquely passes adidas and amazon-com.

**Why this happens**:
- Some anti-bot vendors (DataDome on wsj, PerimeterX on zillow) treat Firefox more harshly. The TLS gap (see §7) is the likely culprit: BO's "firefox_135" profile sends Chrome-class TLS bytes under a Firefox UA, and a sufficiently-tuned vendor flags the mismatch.
- adidas + amazon-com unique wins: Akamai/AWS WAF actually treats Firefox more leniently because Firefox is ~3% of bot traffic — vendors invest in Chrome detection and undertrain on Firefox. Our preset bridges the gap when the vendor uses UA-only signals.

**Action**: Always include in the routing rotation. Drops out when real Gecko TLS (Phase B.3 — NSS cipher/extension ordering) ships; until then, it's a 2-site routing win.

### 5.5 Memory profile (RSS peak, MB, 126-site cold sweep)

| Profile | RSS peak | Reason |
|---|--:|---|
| pixel | **388** | smallest viewport (412×870), empty plugins array, simplest JS surface |
| chrome | 419 | desktop viewport, 5-plugin desktop set, full GPU profile |
| iphone | 445 | iOS-specific JS shims (orientation, DeviceMotionEvent, missing-API stubs all add code paths) |
| firefox | **472** | most-different JS surface (vendor="", productSub="20100101", masked WebGL); some Firefox-shaped behaviour paths in our bootstrap allocate more |

The 84 MB spread is within the workers/V8 background-task baseline noise; none of these profiles is a memory anomaly. See chapter 09 for the absolute-RSS investigation.

---

## 6. Adding new profiles (future)

Three additional profiles are already defined in `presets.rs` but **not in the current routing pool**:

| Preset | Source | Status | Estimated routing premium |
|---|---|---|---|
| `chrome_148_windows()` | `presets.rs:39-108` | unit-tested, not in sweep | +1-2 sites (different UA-CH `sec-ch-ua-platform: "Windows"` |
| `chrome_148_linux()` | `presets.rs:199-269` | unit-tested, not in sweep | +0-1 sites (Linux desktop is rare; some sites filter, some allow) |
| `firefox_135_windows()` | `presets.rs:498-570` | unit-tested, not in sweep | +0-1 sites (firefox + Windows class) |
| `firefox_135_linux()` | `presets.rs:573-642` | unit-tested, not in sweep | +0 sites (firefox + Linux is the worst-tier real-user class) |
| `chrome_148_de()` / `_jp()` / `_ru()` / `_cn()` | `presets.rs:271-385` | localized, not in sweep | +1-3 sites per geo when site geo-routes (e.g. yandex prefers ru) |

### 6.1 Profile candidates worth adding to v0.1.x

| Candidate | Why | Effort |
|---|---|---|
| `chrome_148_windows` (in rotation) | Adds a 5th routing leg; mostly redundant with macOS but flips 1-2 sites that filter macOS hardware reporters | trivial — already coded, just plumb into sweep_metrics |
| `safari_18_macos` (new — desktop Safari) | DESKTOP Safari is missing from the lineup; some sites (apple-news, icloud-web, parts of finance media) prefer Safari-class fingerprints; would likely add 2-4 unique wins | medium — needs a new TLS branch (`safari_18_macos` ≠ `safari_18_ios`) since macOS Safari uses different ciphers/extensions from iOS Safari |
| `chrome_148_linux` (in rotation) | Sites that explicitly allow Linux Chrome (developer-oriented news; some content sites without WAF) | trivial |
| `firefox_135_android` (new) | Firefox-mobile is a niche but uniquely-real fingerprint — would dodge vendors trained on Chrome-mobile risk | medium — needs Mobile Firefox UA, no UA-CH, mobile DPR, Mali GPU |

### 6.2 Estimating premium before adding

The pattern from chrome → pixel → iphone → firefox shows diminishing returns:

- chrome alone: 99
- + pixel: 99 → ~105 (+6, mostly mobile-targeted wins)
- + iphone: 105 → ~107 (+2, yelp + amazon-fr)
- + firefox: 107 → 108 (+1, adidas + amazon-com offset by 0 unique-loss recovery)

Each new profile is worth +0-2 sites unless it opens a fundamentally new fingerprint class (e.g. desktop Safari, mobile Firefox). Routine "add another Chrome flavor" gives +0-1.

### 6.3 Mechanical checklist for adding a profile

1. Write the preset in `crates/stealth/src/presets.rs` (~75 lines, follow `chrome_148_windows()` as template)
2. Add a YAML mirror under `crates/stealth/profiles/<name>.yaml` (`chrome_148_macos.yaml` is the schema reference)
3. Wire the name into `crates/browser/examples/sweep_metrics.rs:88` (the match-on-profile-name)
4. Add validation test in `presets.rs::tests::<name>_validates`
5. Add to `http3_disabled_by_default_on_all_presets` (line 887) profile list
6. Run the full 126 sweep with the new profile (`benchmarks/run_full_sweep.sh`)
7. Compute the cross-set routing delta with the python in §3 — if it's < +1 in 3 runs, don't ship it (noise-floor; see `docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md`)
8. If TLS is new (e.g. real Gecko, real Safari macOS), open a `crates/net/src/tls.rs` PR and add the byte-level reference capture before adding the preset

---

## 7. Profile maintenance

### 7.1 The TLS-label-vs-UA-version split (intentional)

This bites every reader who first looks at the code. `crates/net/src/tls.rs:22-57` documents it in full:

- `TLS_CHROME_MAJOR = 147` — the major whose verified-real ClientHello/H2 fingerprint these constants reproduce, byte-exact
- `UA_CHROME_MAJOR = 148` — the major every desktop Chrome preset's UA advertises

**The split is wire-coherent, not a skew.** Chrome's TLS ClientHello is version-stable across majors; the last change was the MLKEM768 post-quantum rollout at Chrome 131. There was no TLS-stack change between 147 and 148 (~1 month apart, May 2026), so the bytes real Chrome 148 puts on the wire are identical to this 147 capture. JA4 cannot encode the Chrome version. Test `tls_fingerprint_vectors_no_silent_drift` (`tls.rs:506+`) machine-checks this.

### 7.2 When Chrome ships 149

The expected delta (assuming no TLS-stack change, which is the modal case):

| File | Change |
|---|---|
| `presets.rs:120` (`chrome_148_macos`) | bump `user_agent` Chrome/`149.0.0.0`; bump `browser_version` to the new full triple |
| same for `chrome_148_windows`, `chrome_148_linux`, `chrome_148_ru/cn/de/jp`, `pixel_9_pro_chrome_148`, `chrome_148_macos.yaml` | mechanical UA bump |
| `crates/net/src/tls.rs:57` | bump `UA_CHROME_MAJOR = 149` |
| `crates/net/src/tls.rs:535-552` (test) | will fail until `UA_CHROME_MAJOR` is bumped — this is the silent-drift gate |
| `crates/net/src/tls.rs:52` (`TLS_CHROME_MAJOR`) | **leave at 147** unless you have a fresh BoringSSL capture proving the bytes changed |
| `crates/net/src/headers.rs:401-413` (sec-ch-ua-full-version-list) | reads from `browser_version` — no code change needed |

If Chrome 149 does roll a TLS-stack change (rare), follow the existing 131 MLKEM playbook:
1. Capture a real Chrome 149 ClientHello in PCAP, run it through Wireshark + JA3/JA4 tools
2. Update `CIPHER_LIST`, `SIGALGS_LIST`, `CURVES_DESKTOP`, and the extension permutation in `tls.rs:60-220`
3. Bump `TLS_CHROME_MAJOR = 149`
4. Re-run the byte-level fingerprint tests in `tls.rs:506+` to confirm

### 7.3 When iOS Safari ships 19

Similar but with more surface: iOS Safari changes its TLS list more frequently than Chrome. Refresh source: `lexiforest/curl-impersonate/tests/signatures/safari_19.X_iOS.yaml` (the canonical reference per `tls.rs:107` comment). Update the 7 iOS-specific constants in `tls.rs:107-183` and the preset in `presets.rs:795`.

### 7.4 When Firefox ships 136

UA-only at minimum: bump `presets.rs:413` (`firefox_135_macos` → `firefox_136_macos`) — there is no TLS-class swap to maintain (BO's Firefox profiles still send Chrome-class TLS; the `firefox_135` `tls_impersonate` string is informational, see comment in `presets.rs:457-463`). Real Gecko TLS is tracked as a future item (Phase B.3 — reconfigure boring2's cipher list and extension order to match NSS).

### 7.5 Schema reference (every YAML/Rust field)

The full field list and `validate()` rules live in `crates/stealth/src/profile.rs:33-180`. Key serde-default fields (so old serialized profiles don't break):
- `cpu_architecture` defaults to `"x86"`
- `cpu_bitness` defaults to `"64"`
- `platform_version` defaults to `""`
- `ua_model`, `ua_wow64`, `device_class`, `has_platform_authenticator`, `conditional_mediation`, `allow_http3` all have safe serde defaults

When adding a new field: ALWAYS add a `#[serde(default = ...)]` so existing on-disk YAML keeps loading. Validate in `profile.rs::validate()` if it has a consistency rule (e.g. mobile profile must have `max_touch_points > 0`).

### 7.6 BoringSSL TLS impersonate codename mapping (current)

| `tls_impersonate` string | Where it branches in code | Verified-real reference |
|---|---|---|
| `chrome_147` | `tls.rs:241-296` desktop path | `lexiforest/curl-impersonate chrome_147.*_macos` |
| `chrome_147_android` | `tls.rs:243` (Android curves) | `lexiforest chrome_131.0.6778.81_android` (Android lags PQ rollout — verify against fresh Pixel capture if regressions appear, per `tls.rs:99-103`) |
| `safari_18_ios` | `tls.rs:241,247-289` iOS path | `lexiforest/curl-impersonate safari_18.0_iOS.yaml` |
| `firefox_135` | currently same as `chrome_147` desktop (real Gecko TLS deferred — `presets.rs:457-463` documents the gap) | none yet |

---

## 8. Acceptance for v0.1.0

- [ ] **Per-profile pass parity or better**: each of the 4 profiles maintains its 2026-05-24 baseline strict-Pass count (chrome ≥99, pixel ≥102, iphone ≥98, firefox ≥101) on a 3-run median
- [ ] **Routed pass**: best-of-4 routed strict-Pass ≥ 115 (current: 108; need +7)
- [ ] **Routing decision tree implemented**: `crates/browser/src/router.rs` ships with the `KNOWN_DATADOME`/`KNOWN_PERIMETERX`/`KNOWN_CLOUDFLARE`/`KNOWN_KASADA`/`KNOWN_MOBILE_FRIENDLY`/`KNOWN_AWS_WAF` sets from §4, plus `pick_first_profile()` and `fallback_chain()`
- [ ] **Public sweep harness uses the router**: `benchmarks/run_full_sweep.sh` invokes a `--router` flag against `sweep_metrics`; the routed result is reported as the headline number
- [ ] **No regressions on universal-pass set**: all 88 sites passing all 4 profiles in 2026-05-24 still pass in v0.1.0 (zero false negatives from changes)
- [ ] **TLS coherence test stays green**: `tls_fingerprint_vectors_no_silent_drift` (`tls.rs:506+`) passes after any preset bump
- [ ] **Memory parity per profile**: each profile's peak RSS ≤ 2026-05-24 baseline + 10% (chrome ≤ 460, pixel ≤ 427, iphone ≤ 489, firefox ≤ 519)
- [ ] **Profile documentation up to date**: this chapter regenerated from the latest sweep data and committed alongside any routing changes

---

## 9. Files referenced

### Profile definitions
- `crates/stealth/src/presets.rs:39-108` — `chrome_148_windows()`
- `crates/stealth/src/presets.rs:120-196` — `chrome_148_macos()`
- `crates/stealth/src/presets.rs:199-269` — `chrome_148_linux()`
- `crates/stealth/src/presets.rs:271-316` — `chrome_148_ru()`
- `crates/stealth/src/presets.rs:319-363` — `chrome_148_cn()`
- `crates/stealth/src/presets.rs:366-374` — `chrome_148_de()`
- `crates/stealth/src/presets.rs:377-385` — `chrome_148_jp()`
- `crates/stealth/src/presets.rs:413-495` — `firefox_135_macos()`
- `crates/stealth/src/presets.rs:498-570` — `firefox_135_windows()`
- `crates/stealth/src/presets.rs:573-642` — `firefox_135_linux()`
- `crates/stealth/src/presets.rs:672-772` — `pixel_9_pro_chrome_148()`
- `crates/stealth/src/presets.rs:795-875` — `iphone_15_pro_safari_18()`
- `crates/stealth/src/presets.rs:877-1016` — preset validation tests
- `crates/stealth/profiles/chrome_148_macos.yaml:1-86` — YAML schema reference

### Profile schema
- `crates/stealth/src/profile.rs:33-180` — `StealthProfile` struct (every field)
- `crates/stealth/src/profile.rs:7-15` — `DeviceClass` enum (drives TLS branch)
- `crates/stealth/src/gpu.rs` — `GpuProfile` (apple_m3_macos / nvidia_rtx_3060_windows / intel_uhd_630_linux / etc.)

### TLS + header generation (per-profile wire bytes)
- `crates/net/src/tls.rs:22-57` — `TLS_CHROME_MAJOR=147` / `UA_CHROME_MAJOR=148` constants + rationale
- `crates/net/src/tls.rs:60-76` — Chrome cipher list
- `crates/net/src/tls.rs:79-88` — Chrome sigalgs
- `crates/net/src/tls.rs:91-104` — Chrome curves desktop / Android
- `crates/net/src/tls.rs:107-148` — iOS Safari cipher/sigalgs (incl. the duplicated `rsa_pss_rsae_sha384` Apple bug)
- `crates/net/src/tls.rs:152-157` — iOS Safari curves (no PQ, P-521 added)
- `crates/net/src/tls.rs:169-183` — iOS Safari fixed extension permutation (no Fisher-Yates)
- `crates/net/src/tls.rs:203-228` — Chrome extension permutation + Fisher-Yates shuffler
- `crates/net/src/tls.rs:233-308` — `chrome_connector()` per-device_class branch
- `crates/net/src/tls.rs:506+` — `tls_fingerprint_vectors_no_silent_drift` (machine-checked silent-drift gate)
- `crates/net/src/headers.rs:143` — Sec-CH-UA header generation entry
- `crates/net/src/headers.rs:401-413` — `build_sec_ch_ua_full_version_list`
- `crates/net/src/headers.rs:415+` — `build_sec_ch_ua`

### Sweep + measurement
- `crates/browser/examples/sweep_metrics.rs:1-90` — sweep harness, profile selector
- `crates/browser/tests/holistic_sweep.rs:1-700` — 126-site corpus definition
- `crates/browser/src/classify.rs` — verdict classifier (Pass / ThinShell / CHL / ThinBody / Error)
- `benchmarks/run_full_sweep.sh` — driver script for 4-profile + 4-competitor sweep
- `~/projects/browser_oxide_internal/benchmarks/baselines/2026-05-24/bo_chrome_148_macos_cold.json` — chrome raw
- `~/projects/browser_oxide_internal/benchmarks/baselines/2026-05-24/bo_pixel_9_pro_chrome_148_cold.json` — pixel raw
- `~/projects/browser_oxide_internal/benchmarks/baselines/2026-05-24/bo_iphone_15_pro_safari_18_cold.json` — iphone raw
- `~/projects/browser_oxide_internal/benchmarks/baselines/2026-05-24/bo_firefox_135_macos_cold.json` — firefox raw

### Adjacent doc cross-references
- `docs/releases/v0.1.0-parity/02_GAP_ANALYSIS.md` — per-site recovery plan (the 10 Camoufox-only-pass sites)
- `docs/releases/v0.1.0-parity/03_BENCHMARK_METHODOLOGY.md` — corpus + classifier rules
- `docs/releases/v0.1.0-parity/12_COMPETITIVE_LANDSCAPE.md` — what Camoufox/Playwright/Patchright actually do
- `docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md` — ±5-site WAF variance — sets the floor for "is this routing rule real?"
- `docs/BENCHMARK_2026_05_24.md` — narrative sweep report
- `memory/state_2026_05_16_phase5_datadome.md` — Phase 5 DataDome history (informs routing rule §4.1 rule 2)
