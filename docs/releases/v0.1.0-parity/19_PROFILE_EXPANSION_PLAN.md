# 19 — Profile expansion plan

**Audience:** anyone proposing to add new stealth profiles to the v0.1.x rotation, anyone reviewing the cost of expanding the 4-profile pool, and anyone updating the sweep_metrics harness or per-profile validation.

**One-paragraph thesis:** The current 4 shipped profiles route to 108 strict Pass / 120 loose L3 (per `11_PER_PROFILE_STRATEGY.md` §2.1). The cheapest +3 to +7 routing wins available without any engine change is adding 2-3 more stealth profiles to the rotation — `safari_18_macos` (new TLS branch), `chrome_148_windows` (already coded), and optionally `chrome_148_linux` (already coded). This doc inventories the candidate profiles, the per-candidate cost, the expected routing premium, the maintenance burden, and the YAML schema you'll edit. Pair it with `23_TLS_HTTP_FINGERPRINT_REFERENCE.md` if you're adding a profile that needs a new TLS branch (Safari macOS or Edge).

---

## 1. Current 4 profiles — weak spots recap

Read `11_PER_PROFILE_STRATEGY.md` §5 for the full analysis. The compressed picture, ordered by current strict Pass on the 126-site 2026-05-24 sweep:

| Profile | Pass | Unique wins | Worst categories | Reference |
|---|--:|---|---|---|
| pixel_9_pro_chrome_148 | 102 | `amazon-ca` | tech (apple THIN-BODY 0b) | `11_PER_PROFILE_STRATEGY.md:50-79` |
| firefox_135_macos | 101 | `adidas`, `amazon-com` | travel (5/8), realestate (2/4 — loses zillow) | `11_PER_PROFILE_STRATEGY.md:112-134` |
| chrome_148_macos | 99 | **none** (Pareto-dominated) | amazon (1/8), wayfair THIN-BODY | `11_PER_PROFILE_STRATEGY.md:13-49` |
| iphone_15_pro_safari_18 | 98 | `yelp` | misc / news / search / tech — Cloudflare-class penalty on iOS Safari | `11_PER_PROFILE_STRATEGY.md:81-110` |

**Key insight from doc 11 §2.3:** `chrome_148_macos` has **zero unique-pass sites** — every win is also won by ≥1 other profile. Adding any new desktop Chrome variant is therefore competing for the same set of wins, modulo the platform-specific UA differentiation (which is what UA-CH `sec-ch-ua-platform` exists for).

**Cloudflare bias on iPhone (doc 11 §5.3):** 6 of iPhone's 10 recoverable losses are Cloudflare interstitials (`udemy`, `economist`, `ft`, `ecosia`, `quora`, `openai`). Cloudflare's managed challenge is harsher on the iOS Safari class because its risk model has fewer confident-real samples to anchor on. Adding `safari_18_macos` is the leading candidate for differentiating against this bias — a *desktop* Safari class is rare-but-real, larger CF baseline.

**Firefox load-bearing wins (doc 11 §5.4):** 2 unique sites (`adidas`, `amazon-com`). Worth keeping in rotation indefinitely; the cost of adding `firefox_135_windows` to the pool is whether the additional differentiation (Windows UA + Win32 platform) wins anything beyond what `firefox_135_macos` already covers.

---

## 2. New profile candidates — ranked by expected routing gain

The ranking below assumes the 2026-05-24 baseline (`~/projects/browser_oxide_internal/benchmarks/baselines/2026-05-24/`) and the noise floor of ±5 sites (`docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md`). Every projection below is an *upper bound* — actual gains are bounded by what the 18 residual sites tolerate (most are 0/4 across our shipped 4 profiles, so most new profiles will pile up wins on a small subset).

### 2.1 Candidate A — `safari_18_macos` (desktop Safari, NEW TLS branch)

**Why**: Desktop Safari is currently MISSING from the BO lineup. Some content sites whitelist Safari-class fingerprints explicitly (Apple-Books, iCloud-Web, some finance media); some WAFs (Cloudflare in particular, per doc 11 §5.3) penalize the mobile-Safari class but trust desktop-Safari more because desktop Safari traffic is ~17-20% of real Mac users and unrepresented in most bot toolchains.

**Differentiation from existing profiles**:
- TLS: needs a NEW BoringSSL branch — `safari_18_macos` is DISTINCT from `safari_18_ios` (different ciphers, different curve list, different extension set, no zlib cert compression on macOS, has session_ticket). See `23_TLS_HTTP_FINGERPRINT_REFERENCE.md` §3.
- UA: `Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0.1 Safari/605.1.15` — no `Mobile/15E148` token.
- Header set: no `sec-ch-ua*` (Safari doesn't speak UA-CH at all — same as iPhone, same as Firefox).
- HTTP/2 SETTINGS: Safari desktop uses the same 4-setting set as iOS Safari (2, 3, 4, 9 per `crates/net/src/h2_client.rs:106-118`); pseudo-header order is also Safari's `msap`.
- JS surface: `navigator.platform = "MacIntel"`, `vendor = "Apple Computer, Inc."`, `webkit-only API surface` (no `chrome.runtime`, no Web Authn brand list, undefined `deviceMemory`).
- Hardware: macOS 15 desktop dimensions (1512×982 like `chrome_148_macos`), `max_touch_points: 0` (vs iPhone's 5), DPR 2.0 (vs iPhone's 3.0).

**Expected wins**: +2 to +4 sites. Specifically targets:
- `udemy` / `economist` / `ft` / `ecosia` / `openai` / `quora` Cloudflare cluster — desktop Safari class differentiation against the iOS Safari penalty
- some Apple-property pages (apple.com SPA failing on Pixel UA-CH; `safari_18_macos` may be the "native" passable profile for that one site)
- some niche Mac-targeting media sites that pixel/firefox/iphone don't match well

**Implementation cost**: **medium**. Requires a new TLS branch (the device_class machinery already exists; needs a new `DeviceClass::DesktopSafari` variant or — better — an additional discriminator since `DeviceClass::Desktop` is currently equated with Chrome-class TLS at `crates/net/src/tls.rs:241-256`). Concretely:
1. Add the preset in `crates/stealth/src/presets.rs` (new function, ~80 lines, mirroring `chrome_148_macos` for hardware and `iphone_15_pro_safari_18` for browser-shape fields)
2. Decide: extend `DeviceClass` enum with `DesktopSafari` OR add a `browser_name == "Safari" && os_name == "macOS"` predicate in `tls.rs::chrome_connector()` (~20 lines)
3. Add a `safari_18_macos` cipher list constant in `tls.rs` (likely identical to or very close to `CIPHER_LIST_SAFARI_IOS` minus 3DES — `lexiforest/curl-impersonate/tests/signatures/safari_18.0_macos.yaml` is the canonical reference)
4. Verify `h2_client.rs` branches correctly for `DeviceClass::DesktopSafari` / Safari-on-macOS — currently it gates on `is_safari_ios` only (`crates/net/src/h2_client.rs:85`); needs `is_safari || is_safari_ios` split
5. Add `safari_18_macos` to `crates/browser/examples/sweep_metrics.rs:92-99` match arm
6. Add YAML mirror at `crates/stealth/profiles/safari_18_macos.yaml`
7. Add `safari_18_macos_validates` unit test in `presets.rs::tests`

### 2.2 Candidate B — `chrome_148_windows` (already coded, NOT in rotation)

**Why**: The function exists at `crates/stealth/src/presets.rs:39-108` and is unit-tested (`presets.rs:882-885`). It's NOT plumbed into `sweep_metrics.rs` and therefore not in any of our sweep numbers. Adding it to the rotation costs ~5 minutes of plumbing.

**Differentiation from existing profiles**:
- UA: `Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36`
- UA-CH: `sec-ch-ua-platform: "Windows"`, `sec-ch-ua-platform-version: "15.0.0"`, `sec-ch-ua-arch: "x86"`, `sec-ch-ua-bitness: "64"`, `sec-ch-ua-model: ""` (empty for desktop)
- Hardware: 1920×1080 viewport (vs macOS 1512×982), DPR 1.0 (vs macOS 2.0), 24-bit color depth (vs macOS 30-bit)
- WebGL: NVIDIA GeForce RTX 3080 via D3D11 (vs Apple M3 Metal Renderer on macOS) — `webgl_renderer: "ANGLE (NVIDIA, NVIDIA GeForce RTX 3080 Direct3D11 vs_5_0 ps_5_0, D3D11)"` per `presets.rs:65`
- TLS: identical to `chrome_148_macos` (Chrome's TLS is platform-agnostic — see `23_TLS_HTTP_FINGERPRINT_REFERENCE.md` §1)
- `has_platform_authenticator: true` (Windows Hello)
- Color gamut: `srgb` (vs macOS `p3`)

**Expected wins**: +1 to +3 sites. Targets:
- Enterprise IT-vendor pages that filter "non-Windows desktop" (rare but real)
- Some media sites that A/B-test on `sec-ch-ua-platform` and serve a less-restricted variant for Windows
- WAFs that pre-block Linux (per the `chrome_148_linux` discussion in §2.3) but allow Windows
- Possibly `apple.com` (which currently fails on Pixel — Windows class avoids the `sec-ch-ua-model: "Pixel 9 Pro"` SPA branch)

**Implementation cost**: **trivial** (~10 lines).
1. Add `"chrome_148_windows" => stealth::presets::chrome_148_windows()` to `crates/browser/examples/sweep_metrics.rs:92-99`
2. Add to `benchmarks/run_full_sweep.sh` profile loop
3. YAML mirror at `crates/stealth/profiles/chrome_148_windows.yaml` (optional — Rust preset is the source of truth; YAML is for users who want to load a profile without compiling)

### 2.3 Candidate C — `chrome_148_linux` (already coded, NOT in rotation)

**Why**: Exists at `crates/stealth/src/presets.rs:199-269`, unit-tested at `presets.rs:920-924`. Linux desktop Chrome is a real-but-tiny share of traffic (~1-2% of desktop), so anti-bot vendors classify it differently — some allow it for developer-tools usage, some pre-block (a 1-2% share isn't worth dispositive blocking but is worth scoring against).

**Differentiation from existing profiles**:
- UA: `Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36`
- UA-CH: `sec-ch-ua-platform: "Linux"`, `sec-ch-ua-platform-version: ""` (Chrome on Linux reports EMPTY platform_version per real Chrome — enforced by `crates/stealth/src/profile.rs:323-328`)
- WebGL: Intel UHD 630 via Mesa, `webgl_renderer: "ANGLE (Intel, Mesa Intel(R) UHD Graphics 630 (CFL GT2), OpenGL 4.6)"` per `presets.rs:225`
- TLS: identical to other desktop Chrome variants
- `has_platform_authenticator: false` on most Linux configs (no Touch ID / no Windows Hello equivalent) — preset currently sets `false` per `presets.rs` (verify)

**Expected wins**: +0 to +1 sites. Most sites that allow Linux desktop already pass via macOS desktop; the diff is small. Possible niche wins on developer-focused content sites (Hacker News-class, technical docs, some GitHub-adjacent pages — none of which currently fail on our corpus though).

**Implementation cost**: **trivial** (~5 lines — same plumbing as Candidate B).

### 2.4 Candidate D — `edge_148_windows` (NEW preset, no Rust code yet)

**Why**: Microsoft Edge has its own brand presence in the `sec-ch-ua` triple (`"Microsoft Edge";v="148", "Not.A/Brand";v="8", "Chromium";v="148"`). Microsoft-property sites (xbox.com, microsoft.com, MSDN/learn.microsoft.com, Office Online, OneDrive) sometimes whitelist Edge specifically; some enterprise WAFs (Akamai, Imperva on B2B portals) maintain Edge-friendly allowlists.

**Differentiation from existing profiles**:
- UA: `Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36 Edg/148.0.2792.79` — note the `Edg/` token at the end (NOT `Edge/`; that's IE-class)
- UA-CH brand list: `"Microsoft Edge";v="148", "Not.A/Brand";v="8", "Chromium";v="148"` — needs a new `build_sec_ch_ua_edge()` in `crates/net/src/headers.rs` paralleling the Chrome builder at `headers.rs:415-430`
- UA-CH `sec-ch-ua-full-version-list` adds `Edg` brand too
- TLS: identical to Chrome (Edge uses Chromium's BoringSSL upstream — no separate impersonation needed)
- HTTP/2: identical to Chrome
- JS surface: `navigator.vendor: "Google Inc."` (same as Chrome — Edge keeps Chromium's value), plus Edge-specific window globals (`window.chrome.app.isInstalled` exists; `microsoftEdgeShare` and others)

**Expected wins**: +0 to +2 sites. Narrow target — most of our 126-site corpus doesn't favor Edge specifically. Worth deferring to v0.1.x if a known customer demand exists; not v0.1.0 critical path.

**Implementation cost**: **medium-low** (~120 lines).
1. New preset constructor in `presets.rs` (~80 lines)
2. Edge-specific brand builder in `headers.rs` (~20 lines + adjust `nav_headers()` dispatch)
3. YAML mirror
4. Sweep harness plumbing
5. Unit-test validation

### 2.5 Candidate E — `firefox_135_windows` (already coded, NOT in rotation)

**Why**: Exists at `crates/stealth/src/presets.rs:498-570`, unit-tested at `presets.rs:939-945`. Adds a Firefox-on-Windows risk class. Some anti-bot vendors (DataDome notably, per doc 11 §5.4) treat Firefox more leniently because Firefox is ~3% of bot traffic — adding the Windows variant gives a second axis on which to differentiate within "Firefox-class".

**Expected wins**: +0 to +1 sites. The `firefox_135_macos` profile already takes the Firefox-class wins; `firefox_135_windows` differs only in UA platform string. Most sites that pass `firefox_135_macos` will also pass `firefox_135_windows` and vice versa — no expected unique wins on the current corpus.

**Implementation cost**: **trivial** (~5 lines).

### 2.6 Candidate F — `chrome_149_macos` (Chrome 149 maintenance — not a new profile)

**Why**: When Chrome 149 ships stable, every existing preset's UA + `browser_version` + `sec-ch-ua-full-version-list` must be bumped. This is documented in detail at `11_PER_PROFILE_STRATEGY.md` §7.2 and `23_TLS_HTTP_FINGERPRINT_REFERENCE.md` §5. It's NOT a new profile per se but it's *the* recurring maintenance task that every consumer doing serious deployment will hit quarterly.

**Implementation cost**: **trivial** (UA bump only — the TLS-stack hasn't rev'd since Chrome 131 MLKEM768, so `TLS_CHROME_MAJOR` stays at 147 unless Chrome 149 ships a deliberate TLS-stack change). Follow the playbook at `23_TLS_HTTP_FINGERPRINT_REFERENCE.md` §5.

---

## 3. Per-candidate implementation cost summary

| Candidate | Expected wins | Effort | Blocker | Net v0.1.0 priority |
|---|--:|---|---|---|
| A. `safari_18_macos` | +2 to +4 | medium (new TLS branch) | none | **HIGH** — biggest single addition |
| B. `chrome_148_windows` | +1 to +3 | trivial | none | **HIGH** — already coded |
| C. `chrome_148_linux` | +0 to +1 | trivial | none | medium |
| D. `edge_148_windows` | +0 to +2 | medium-low | none | low — defer to v0.1.x |
| E. `firefox_135_windows` | +0 to +1 | trivial | none | low (redundancy with macOS Firefox) |
| F. Chrome 149 maintenance | 0 (parity) | trivial | wait for Chrome 149 stable | **REQUIRED** when 149 ships |

**Recommendation for v0.1.0**: A + B. Defer C/D/E to v0.1.x. F when the upstream signal exists.

---

## 4. Expected combined routing gain (best-of-N)

Method: for each candidate set, compute the best-of-N routed strict Pass by union over per-site Pass sets. The numbers below assume the upper-bound estimates from §2 land — actual measured deltas will be in the same ballpark but subject to the ±5-site noise floor (`docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md`).

| Set | Profiles | Projected routed strict Pass | Delta vs current (108) |
|---|---|--:|--:|
| current | chrome / pixel / iphone / firefox | 108 | 0 |
| +A | + safari_18_macos | 110-112 | +2 to +4 |
| +A+B | + safari_18_macos + chrome_148_windows | 111-114 | +3 to +6 |
| +A+B+C | + safari_18_macos + chrome_148_windows + chrome_148_linux | 112-115 | +4 to +7 |
| +A+B+C+D | + safari_18_macos + chrome_148_windows + chrome_148_linux + edge_148_windows | 113-117 | +5 to +9 |

**Reading these projections honestly:**
- Each new profile is worth +0-2 sites unless it opens a fundamentally new fingerprint class (e.g., desktop Safari per A). Routine "add another Chrome flavor" gives +0-1 (per doc 11 §6.2).
- The v0.1.0 acceptance bar in `11_PER_PROFILE_STRATEGY.md` §8 is "routed best-of-4 ≥ 115" — adding A+B+C closes that gap.
- Hitting 115 routed without engine changes requires the projections to land at the high end of the range. If they land low end, we need to combine profile expansion with chapter 05 / 06 engine work.

---

## 5. Maintenance cost per added profile

Each profile in rotation is a recurring obligation. The cadence below assumes Chrome / Safari / Firefox follow their current 4-6 week stable release cycles.

### 5.1 Recurring tasks per shipped profile

| Task | Frequency | Owner | Where |
|---|---|---|---|
| UA bump on major rev | Quarterly (per browser) | profile maintainer | `presets.rs::<profile>` `user_agent` + `browser_version` |
| `sec-ch-ua-full-version-list` bump | Same as UA | profile maintainer | derived from `browser_version` — auto if you bumped the version field |
| TLS impersonate verification | Quarterly + on any boring2 update | TLS lead | `crates/net/src/tls.rs` constants + `tls_fingerprint_vectors_no_silent_drift` test |
| ClientHints brand list update | Major version bumps (`Not.A/Brand` rotates per Chrome major) | profile maintainer | `crates/net/src/headers.rs:401-430` |
| Profile-specific YAML mirror | Whenever Rust preset changes | profile maintainer | `crates/stealth/profiles/<name>.yaml` |
| Sweep regression (per 14 §L4) | Quarterly + on every profile change | benchmark owner | `benchmarks/run_full_sweep.sh` |
| Routing decision tree audit (doc 11 §4.1) | When per-profile wins/losses shift by >2 sites | router maintainer | `crates/browser/src/router.rs` (v0.1.0 deliverable) |

### 5.2 Cross-profile invariants enforced by unit tests

Each new profile must keep these test gates green (existing in `crates/stealth/src/presets.rs:877-1016` and `crates/net/src/tls.rs:476-553`):

- `<profile>_validates` — schema/coherence via `StealthProfile::validate()`
- `http3_disabled_by_default_on_all_presets` (`presets.rs:887+`) — `allow_http3` MUST stay `false` until we have a Chrome-fixed-order quinn fork; randomized transport_parameters are a worse fingerprint than not speaking h3 at all
- `firefox_webgl_is_masked` (`presets.rs:954+`) — Firefox profiles MUST have `webgl_vendor == "Mozilla"` AND `webgl_renderer == "Mozilla"` (Firefox 113+ masks both)
- `webdriver_not_in_profile` (`presets.rs:992+`) — no preset accidentally includes the string `webdriver`
- `ua_contains_version` (`presets.rs:1000+`) — UA must report the reduced `<major>.0.0.0` form (Chrome's UA-reduction policy since v110); full version lives only in `browser_version`
- `tls_fingerprint_vectors_no_silent_drift` (`tls.rs:476+`) — cipher list / sigalg list / curves order / extension count must match the verified-real Chrome reference

### 5.3 CI / nightly sweep drift detection

Per `14_TESTING_VALIDATION.md` §"CI integration", a nightly sweep is the catch-all for drift in any profile (whether TLS, UA, or behavioural):
- Job: `benchmarks/run_full_sweep.sh` runs all 4 (eventually 5-6) profiles cold against the 126-site corpus
- Alert threshold: per-profile Pass count drops by >5 sites vs the 3-run trailing median
- The same job catches anti-bot vendor evolution (per `12_COMPETITIVE_LANDSCAPE.md` §5.2), profile drift, and TLS silent drift (the in-process test `tls_fingerprint_vectors_no_silent_drift` is faster but doesn't catch network-level changes)

---

## 6. Profile YAML schema reference

The canonical schema is documented in `crates/stealth/src/profile.rs:33-186` (`StealthProfile` struct) with the documented example YAML at `crates/stealth/profiles/chrome_148_macos.yaml`. Every top-level field follows.

### 6.1 Identity (always required)

| Field | Type | Example | Notes |
|---|---|---|---|
| `user_agent` | string | `"Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36"` | MUST contain the reduced `<major>.0.0.0` form for Chrome; `<major>.0` for Firefox |
| `browser_name` | string | `"Chrome"` / `"Firefox"` / `"Safari"` | drives `nav_headers()` dispatch in `headers.rs:16-23` |
| `browser_version` | string | `"148.0.7778.168"` | FULL version for `sec-ch-ua-full-version-list` |
| `os_name` | string | `"macOS"` / `"Windows"` / `"Linux"` / `"Android"` / `"iOS"` | drives platform validation + `sec-ch-ua-platform` |
| `os_version` | string | `"15.2"` | display only |
| `platform` | string | `"MacIntel"` / `"Win32"` / `"Linux x86_64"` / `"iPhone"` / `"Linux armv81"` | navigator.platform — MUST match `os_name` per `validate()` |
| `vendor` | string | `"Google Inc."` / `""` (Firefox) / `"Apple Computer, Inc."` | navigator.vendor |
| `vendor_sub` | string | `""` typically | |
| `product_sub` | string | `"20030107"` (Chrome frozen) / `"20100101"` (Firefox Gecko date) | navigator.productSub — MUST be Chrome's `20030107` or Firefox's `20100101` |
| `app_version` | string | UA minus the `Mozilla/` prefix | redundant with `user_agent`; kept for navigator.appVersion |

### 6.2 Hardware

| Field | Type | Example | Notes |
|---|---|---|---|
| `screen_width` | u32 | `1512` | CSS px |
| `screen_height` | u32 | `982` | CSS px |
| `screen_avail_width` | u32 | `1512` | excludes OS chrome (e.g., macOS dock) |
| `screen_avail_height` | u32 | `949` | excludes macOS menu bar (~33 px) |
| `screen_avail_top` | u32 | `33` | macOS menu bar offset; `0` on Win/Linux |
| `screen_color_depth` | u32 | `30` (macOS HDR) / `24` (Windows/Linux) | |
| `device_pixel_ratio` | f64 | `2.0` (macOS Retina) / `1.0` (Win/Linux) / `2.625` (Pixel 9 Pro fractional) / `3.0` (iPhone) | |
| `cpu_cores` | u8 | `8` (Chrome) / `2` (Safari intentional cap per `presets.rs:817`) | maps to navigator.hardwareConcurrency |
| `device_memory` | u8 | `8` / `0` (iOS — Safari doesn't expose; JS bootstrap returns undefined) | Chrome rounds to spec set `{0.25, 0.5, 1, 2, 4, 8}` |
| `max_touch_points` | u8 | `0` (desktop) / `5` (mobile) | drives `validate()` mobile-vs-desktop pointer check |

### 6.3 GPU (`webgl_vendor` / `webgl_renderer`)

| Field | Type | Example |
|---|---|---|
| `webgl_vendor` | string | `"Google Inc. (Apple)"` / `"Google Inc. (NVIDIA)"` / `"Google Inc. (Intel)"` / `"Mozilla"` (Firefox masks) / `"Apple Inc."` (iOS) |
| `webgl_renderer` | string | `"ANGLE (Apple, ANGLE Metal Renderer: Apple M3, Unspecified Version)"` / `"ANGLE (NVIDIA, NVIDIA GeForce RTX 3060 ..., D3D11)"` / `"Mozilla"` (Firefox) / `"Apple GPU"` (iOS — literal, no model) |
| `gpu_profile` (serde-default) | GpuProfile | `apple_m3_macos()` / `nvidia_rtx_3060_windows()` / `intel_uhd_630_linux()` — see `crates/stealth/src/gpu.rs:53,111,169,223` |

`validate()` enforces vendor-renderer consistency: NVIDIA renderer requires NVIDIA vendor, Intel requires Intel, Apple requires Apple (and only on macOS).

### 6.4 Locale

| Field | Type | Example |
|---|---|---|
| `language` | string | `"en-US"` |
| `languages` | Vec<string> | `["en-US", "en"]` — MUST contain `language` per `validate()` |
| `timezone` | string | `"America/Los_Angeles"` |

### 6.5 Client Hints (high-entropy, sent only after Accept-CH)

| Field | Type | Example | Notes |
|---|---|---|---|
| `cpu_architecture` (serde-default `"x86"`) | string | `"arm"` (macOS Apple Silicon / Android) / `"x86"` (Win/Linux) | MUST be exactly `"x86"` or `"arm"` |
| `cpu_bitness` (serde-default `"64"`) | string | `"64"` typical / `"32"` rare | MUST be exactly `"64"` or `"32"` |
| `platform_version` (serde-default `""`) | string | `"15.2.0"` macOS / `"15.0.0"` Win / `""` Linux (Chrome on Linux reports empty per validate) | |
| `ua_model` (serde-default `""`) | string | `"Pixel 9 Pro"` (mobile only — empty for desktop per validate) | |
| `ua_wow64` (serde-default `false`) | bool | `false` typical | `true` only on 32-bit Chrome on 64-bit Windows |

### 6.6 Network

| Field | Type | Example |
|---|---|---|
| `device_class` (serde-default `Desktop`) | enum | `Desktop` / `MobileAndroid` / `MobileIOS` |
| `tls_impersonate` | string | `"chrome_147"` / `"chrome_147_android"` / `"safari_18_ios"` / `"firefox_135"` — see `23_TLS_HTTP_FINGERPRINT_REFERENCE.md` §1 |
| `connection_effective_type` | string | `"4g"` |
| `connection_rtt` | u32 | `50` (ms) |
| `connection_downlink` | f64 | `10.0` (Mbps) |
| `proxy` (serde-default `None`) | Option<string> | usually `null` |
| `allow_http3` (serde-default `false`) | bool | KEEP `false` until we vendor-fork quinn — see `11_PER_PROFILE_STRATEGY.md` §1.5 |

### 6.7 Plugins

| Field | Type | Chrome | Firefox | iOS Safari |
|---|---|---|---|---|
| `pdf_viewer_enabled` | bool | `true` | `true` | `false` |
| `plugins_count` | u32 | `5` (PDF + 4 internal) | `5` | `0` (empty array on mobile) |
| `mime_types_count` | u32 | `2` | `2` | `0` |

### 6.8 Fingerprint seeds (deterministic per-profile)

| Field | Type | Notes |
|---|---|---|
| `canvas_seed` | u64 | Drives canvas-render PRNG; pick a per-profile constant so repeated loads of the same site get the same canvas hash |
| `audio_seed` | u64 | Same role for AudioContext fingerprint |

YAML 1.2 doesn't parse `0x` literals — write the decimal form (e.g., `12379813812177893520` for `0xabcdef1234567890`). The YAML file at `crates/stealth/profiles/chrome_148_macos.yaml:65-69` documents this gotcha.

### 6.9 WebAuthn / FedCM probe shape

| Field | Type | Default |
|---|---|---|
| `has_platform_authenticator` (serde-default `false`) | bool | `true` on Mac/Windows desktop (Touch ID / Windows Hello); `false` on Linux desktop / Android-fresh / iOS-fresh |
| `conditional_mediation` (serde-default `true`) | bool | `true` typical |

### 6.10 Media features

| Field | Type | Example |
|---|---|---|
| `prefers_color_scheme` | string | `"light"` / `"dark"` |
| `pointer_type` | string | `"fine"` (desktop mouse) / `"coarse"` (touch) |
| `hover_capability` | string | `"hover"` (desktop) / `"none"` (touch) |
| `color_gamut` (serde-default `"srgb"`) | string | `"p3"` (macOS / iPhone) / `"srgb"` (Win/Linux/Android) |

### 6.11 Window dimensions

| Field | Type | Notes |
|---|---|---|
| `inner_width` / `inner_height` | u32 | viewport (CSS px, excludes browser chrome) |
| `outer_width` / `outer_height` | u32 | including browser chrome (title bar, address bar — desktop) |

For mobile, `inner == outer` typically (no chrome bars on the JS surface).

### 6.12 Media devices

`media_devices` (serde-default `Vec::new()`): list of `{device_id, kind, label, group_id}` for `navigator.mediaDevices.enumerateDevices()` to return. Per-OS defaults via `default_media_devices()` in `presets.rs`.

### 6.13 CSP enforcement

`enforce_csp` (serde-default `true`): real Chrome enforces CSP on sub-resource fetches and `<script src>` loads. Setting `false` is legacy behaviour (issue every fetch the page requests) which is a cross-vendor bot tell. Override at runtime with `BROWSER_OXIDE_CSP_BYPASS=1`.

---

## 7. Acceptance for v0.1.0 (profile expansion)

- [ ] Add `safari_18_macos` profile (Candidate A) — new preset + new TLS branch + sweep harness plumbing
- [ ] Add `chrome_148_windows` profile to the rotation (Candidate B) — already coded; just plumb into sweep + benchmark scripts
- [ ] `sweep_metrics.rs:88-99` supports both new profile names; rejects unknown profile with the same `panic!` shape
- [ ] 3-run baseline (per `14_TESTING_VALIDATION.md` §L5) includes 6 profiles; report includes per-profile pass count + routed best-of-6
- [ ] Each new profile has a `<name>_validates` unit test (matching the pattern at `presets.rs:882`)
- [ ] Each new profile is included in `http3_disabled_by_default_on_all_presets` loop (`presets.rs:887+`)
- [ ] `tls_fingerprint_vectors_no_silent_drift` still passes with the new TLS branch added (`tls.rs:506+`)
- [ ] Documented expected routing-win sites per profile: this doc + `11_PER_PROFILE_STRATEGY.md` §6.1 updated
- [ ] Routing decision tree (doc 11 §4.1) extended with rules for the 2 new profiles (Safari macOS routing key, Windows desktop routing key)
- [ ] Routed best-of-6 strict Pass ≥ 112 (3-run median) — bar set to mid of §4 projection; if measured lower, document the gap and re-baseline

---

## 8. Cross-references

- `11_PER_PROFILE_STRATEGY.md` — current 4-profile internals + routing rules (READ FIRST)
- `12_COMPETITIVE_LANDSCAPE.md` — competitive context (Camoufox has 1 profile; we ship a rotation)
- `13_FILE_LOCATIONS_INDEX.md` — file index across the engine
- `14_TESTING_VALIDATION.md` — L4/L5 sweep methodology used for measuring new profiles
- `15_OPEN_QUESTIONS.md` §Q3 — SharedSession bleed (x-com THIN-BODY) is a confound that may affect new-profile measurement
- `23_TLS_HTTP_FINGERPRINT_REFERENCE.md` — TLS reference; READ if adding a profile that needs a new TLS branch (Safari macOS, Edge)
- `docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md` — ±5-site WAF variance — sets the floor for "is this routing win real?"

---

## 9. Files referenced

### Existing 4-profile Rust presets
- `crates/stealth/src/presets.rs:39-108` — `chrome_148_windows()` (already coded, NOT in rotation — Candidate B)
- `crates/stealth/src/presets.rs:120-196` — `chrome_148_macos()` (shipped)
- `crates/stealth/src/presets.rs:199-269` — `chrome_148_linux()` (already coded, NOT in rotation — Candidate C)
- `crates/stealth/src/presets.rs:272-316` — `chrome_148_ru()` (localized variant, not in rotation)
- `crates/stealth/src/presets.rs:319-363` — `chrome_148_cn()` (localized)
- `crates/stealth/src/presets.rs:366-374` — `chrome_148_de()` (locale-only override of `chrome_148_windows`)
- `crates/stealth/src/presets.rs:377-385` — `chrome_148_jp()` (locale-only override)
- `crates/stealth/src/presets.rs:413-495` — `firefox_135_macos()` (shipped)
- `crates/stealth/src/presets.rs:498-570` — `firefox_135_windows()` (already coded, NOT in rotation — Candidate E)
- `crates/stealth/src/presets.rs:573-642` — `firefox_135_linux()` (already coded, NOT in rotation)
- `crates/stealth/src/presets.rs:690-792` — `pixel_9_pro_chrome_148()` (shipped)
- `crates/stealth/src/presets.rs:795-875` — `iphone_15_pro_safari_18()` (shipped)
- `crates/stealth/src/presets.rs:877-1016` — preset validation tests

### Profile schema
- `crates/stealth/src/profile.rs:8-15` — `DeviceClass` enum (Desktop / MobileAndroid / MobileIOS; needs extension if adding `safari_18_macos`)
- `crates/stealth/src/profile.rs:33-186` — `StealthProfile` struct (every field; serde defaults documented inline)
- `crates/stealth/src/profile.rs:208-351` — `StealthProfile::validate()` consistency rules
- `crates/stealth/src/gpu.rs:53` — `nvidia_rtx_3060_windows()`
- `crates/stealth/src/gpu.rs:111` — `apple_m3_macos()`
- `crates/stealth/src/gpu.rs:223` — `intel_uhd_630_linux()`
- `crates/stealth/profiles/chrome_148_macos.yaml` — documented schema reference (86 lines)

### TLS + HTTP/2 (per-profile wire bytes)
- `crates/net/src/tls.rs:52-57` — `TLS_CHROME_MAJOR = 147` / `UA_CHROME_MAJOR = 148`
- `crates/net/src/tls.rs:60-76` — Chrome cipher list
- `crates/net/src/tls.rs:107-148` — iOS Safari cipher/sigalgs
- `crates/net/src/tls.rs:233-369` — `chrome_connector()` per-`device_class` branch (HERE is where a `safari_18_macos` branch goes)
- `crates/net/src/h2_client.rs:85-130` — HTTP/2 per-profile branch (HERE for Safari-on-desktop)
- `crates/net/src/headers.rs:16-23` — `nav_headers()` browser_name dispatch
- `crates/net/src/headers.rs:415-430` — `build_sec_ch_ua()` (HERE for Edge if Candidate D ships)

### Sweep + measurement
- `crates/browser/examples/sweep_metrics.rs:88-99` — profile string → preset match (THIS is the plumbing change for Candidates B/C/E)
- `crates/browser/tests/holistic_sweep.rs:1-700` — 126-site corpus definition
- `crates/browser/src/classify.rs` — verdict classifier
- `benchmarks/run_full_sweep.sh` — multi-profile sweep driver

### Sweep data (2026-05-24 baseline)
- `~/projects/browser_oxide_internal/benchmarks/baselines/2026-05-24/bo_chrome_148_macos_cold.json`
- `~/projects/browser_oxide_internal/benchmarks/baselines/2026-05-24/bo_pixel_9_pro_chrome_148_cold.json`
- `~/projects/browser_oxide_internal/benchmarks/baselines/2026-05-24/bo_iphone_15_pro_safari_18_cold.json`
- `~/projects/browser_oxide_internal/benchmarks/baselines/2026-05-24/bo_firefox_135_macos_cold.json`

### Workspace docs
- `CLAUDE.md` — conventions (per-thread V8, license rules, scope rules)
- `CONTRIBUTING.md` — contributor guide
- `docs/STEALTH.md` — referenced from `profile.rs:32` as the full field reference
- `docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md` — variance floor
