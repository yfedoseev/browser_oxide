# 07 — Cross-cutting preset coherence audit (the unifying root cause of clusters 01/02)

**Status:** root-cause + fix-plan research, 2026-05-29. No live navigations (competitor
benchmark holds the single IP). Reasoned from the captured per-profile tag/len matrix
(`00_DATA_per_profile_matrix.md`), the four BO presets (`crates/stealth/src/presets.rs`),
the wire stack (`crates/net/src/tls.rs`, `h2_client.rs`, `headers.rs`), the JS surface
(`crates/js_runtime/src/extensions/stealth_ext.rs`, `js/*_bootstrap.js`), the existing
network audit (`docs/v0.1.0-parity-workflows/external/NETWORK_fingerprint.md`), the TLS/H2
implementation reference (`docs/releases/v0.1.0-parity/23_TLS_HTTP_FINGERPRINT_REFERENCE.md`,
`39_NETWORK_LAYER_FINGERPRINTING.md`), the Camoufox overview
(`docs/releases/v0.1.0-parity/audit/02_CAMOUFOX_V150_OVERVIEW.md`), and 2026 external sources.

---

## 0. Headline

The 20 consistency-gap sites are **not 20 independent site bugs**. They collapse onto a
small number of **per-profile coherence defects** — places where a profile's UA tells one
story and its TLS/H2/WebGL fingerprint tells another. The mechanism is the single thing
every 2026 anti-bot vendor weights heaviest: **cross-layer mismatch** ("UA says X, the
wire/GPU says Y → bot"; confirmed by Cloudflare, DataDome, AWS, Akamai sources in §4).

Three defects explain the bulk of the gap, in descending blast radius:

| Defect | Profiles hit | Cluster(s) it explains | Severity |
|---|---|---|---|
| **D1 — Firefox UA over Chrome TLS + Chrome H2** (no Firefox wire class) | firefox | reuters, wsj, tripadvisor (DataDome), zillow (PX) | HIGH |
| **D2 — Wrong GPU profile on mobile** (`apple_m3_macos()` GPU on BOTH pixel and iphone) | pixel, iphone | iphone Cloudflare cluster (raises bot score → challenged); pixel WebGL incoherence | HIGH (iphone), MED (pixel) |
| **D3 — Safari iOS extension-count / PADDING residual + no-CH coherence** | iphone | economist, ecosia, ft, openai, quora, udemy (all Cloudflare-CHL) | MED–HIGH |

The remaining gaps are **not** coherence tells and must NOT be "fixed" as if they were:
- **Pixel THIN-BODY 0 / ERROR** (airbnb, yandex-ru, prime-video) and **chrome/firefox/pixel
  TIMEOUT** (uber) are **nav-reliability bugs**, not fingerprint mismatches (§5).
- **amazon-ca chrome 5310** is the known AWS-WAF same-IP token-clustering artifact already
  understood this session (§6).
- **spotify desktop 9881 vs mobile 147KB** is partly **legitimate content** (mobile gets a
  lighter shell) AND partly a DataDome desktop tell (§6).

Fixing D1+D2+D3 is the highest-leverage path to bring firefox 106→~110 and iphone 108→~114,
i.e. toward per-profile v150 parity. D2 is the single best ROI (one struct field per
profile, two new GPU profiles, fixes a HIGH iphone defect and a MED pixel defect at once).

---

## 1. Method: what each profile actually puts on the wire / in JS

The decisive fact: **`crates/net/src/tls.rs::chrome_connector` and
`h2_client.rs::handshake` branch ONLY on `profile.device_class`** (`Desktop` /
`MobileAndroid` / `MobileIOS`), never on `browser_name` and never on the
`tls_impersonate` string. So per-profile wire bytes are:

| Profile | device_class | UA / browser_name | TLS ClientHello (tls.rs branch) | H2 SETTINGS+pseudo (h2_client.rs branch) | UA-CH headers | WebGL gpu_profile |
|---|---|---|---|---|---|---|
| chrome_148_macos | Desktop | Chrome 148 | **Chrome 147** (correct) | **Chrome** `m,a,s,p` (correct) | sec-ch-ua (correct) | apple_m3_macos (correct) |
| pixel_9_pro_chrome_148 | MobileAndroid | Chrome 148 Mobile | Chrome 147 (curves==desktop, see L7) | **Chrome** `m,a,s,p` (correct for Chrome-Android) | sec-ch-ua mobile (correct) | **apple_m3_macos (WRONG — D2)** |
| iphone_15_pro_safari_18 | MobileIOS | Safari 18 | **Safari iOS** (impl'd; residual D3) | **Safari** `m,s,a,p` (correct) | none (correct) | **apple_m3_macos (WRONG — D2)** |
| firefox_135_macos | **Desktop** | **Firefox 135** | **Chrome 147 (WRONG — D1)** | **Chrome** `m,a,s,p` **(WRONG — D1)** | none (correct, headers.rs) | apple_m2_pro_macos (plausible, but masked to "Mozilla") |

Evidence for the branch-on-device_class-only fact:
- `tls.rs:241` `let is_safari_ios = profile.device_class == DeviceClass::MobileIOS;`
- `tls.rs:242-246` curves `match profile.device_class { … }` — Desktop and MobileAndroid
  both → Chrome; firefox is `DeviceClass::Desktop` (`presets.rs:473`) → Chrome path.
- `h2_client.rs:85` `let is_safari_ios = profile.device_class == DeviceClass::MobileIOS;` —
  every non-iOS profile, firefox included, takes the Chrome `m,a,s,p` SETTINGS branch.
- `tls.rs:547` the silent-drift test only asserts desktop Chrome presets carry
  `tls_impersonate == "chrome_147"`; nothing reads `"firefox_135"`. The string is
  aspirational (`presets.rs:464-474`, `presets.rs:550`).
- WebGL: `stealth_ext.rs:90-119` reads `unmasked_vendor`, `unmasked_renderer`,
  `webgl_params`, `shader_precision`, `extensions` from `p.gpu_profile`; both pixel
  (`presets.rs:931`) and iphone (`presets.rs:1035`) set
  `gpu_profile: crate::gpu::apple_m3_macos()` with a literal `// TODO` admitting the bug.

---

## 2. Defect D1 — Firefox UA over Chrome TLS + Chrome H2 (the firefox cluster)

### 2.1 Data signature
firefox-only gap-fails: **reuters DataDome-CHL 1456**, **wsj DataDome-CHL 1461**,
**tripadvisor DataDome-CHL 1464**, **zillow PerimeterX-PaH 14558**, plus macys/spotify
(THIN/nav — different cause, §5/§6). The DataDome trio + the PX site are exactly the
vendor classes that gate on TLS-JA3/JA4-vs-UA (§4). All three desktop/mobile siblings
(chrome/pixel/iphone) PASS reuters/wsj/tripadvisor with full bodies (1.1MB+). The ONLY
thing that changes for firefox is the **UA + headers say Firefox while the JA4 + akamai-H2
say Chrome 147** — the textbook mismatch.

### 2.2 Code root cause
The firefox preset (`presets.rs:421-505`) sets `browser_name: "Firefox"`, a Gecko UA, and
`tls_impersonate: "firefox_135"` — but `device_class: DeviceClass::Desktop`
(`presets.rs:473`). Therefore:
- TLS → `CIPHER_LIST` (15 Chrome ciphers), `CURVES_DESKTOP` (X25519_MLKEM768 lead),
  `SIGALGS_LIST` (8 Chrome sigalgs), 16-extension **Fisher-Yates shuffle**, **Brotli** cert
  compression, **ALPS** payload, **ECH GREASE**. → JA4 `t13d1516h2_…` = **Chrome 147**.
- H2 → `1:65536;2:0;4:6291456;6:262144|15663105|0|m,a,s,p` = **Chrome akamai_fingerprint**.
- `headers.rs::firefox_headers` (`headers.rs:594-706`) correctly emits Firefox headers
  (no `sec-ch-ua*`, no `priority`, shorter `accept`, `q=0.5` accept-language) — so the
  HTTP layer is Firefox but the TLS+H2 layer is Chrome. **Internally contradictory.**

A real Firefox 135 emits a completely different JA4: different cipher set/count, **no**
MLKEM768-lead desktop ordering, NSS extension order with **no Fisher-Yates**, **no ALPS**,
**no ECH-grease**, **no Brotli** cert compression, plus Firefox H2
(`HEADER_TABLE_SIZE=65536, ENABLE_PUSH=0, INITIAL_WINDOW_SIZE=131072, MAX_FRAME_SIZE=16384`,
pseudo-order `m,p,a,s`). None of that is what BO sends.

### 2.3 Why it presents as DataDome/PX and not Cloudflare
DataDome and PerimeterX analyze the **TLS ClientHello before any HTTP byte** and keep a
JA3/JA4→client-type database (§4). "Firefox UA + BoringSSL/Chrome JA4" is a high-confidence
automation signal for them. Cloudflare-fronted firefox-routed sites in the corpus mostly
also have a Chrome/pixel sibling that passes, so the firefox UA isn't being routed into a
CF JA4 trap as often — but the same defect would bite there too (it is masked, not absent;
NETWORK_fingerprint.md §2.3, §3.1).

### 2.4 Cited prior work
`NETWORK_fingerprint.md` already identifies this as **L1**, "the cleanest single
network-layer leak in BO today" (§2.3, §0 headline), and notes Camoufox runs the **real
Firefox NSS stack** so its Firefox JA4 is genuine — "Camoufox-Firefox wins and BO-Firefox
loses" on any JA4-vs-UA vendor (§3.3). `23_TLS_HTTP_FINGERPRINT_REFERENCE.md` §1.4 table
row 4 and §5.3 confirm `firefox_135` is "currently same as chrome_147 desktop … real
Gecko/NSS TLS is deferred." This audit elevates L1 from "latent leak" to "**proven cause
of 4 firefox-only gap sites**" using the per-profile matrix.

---

## 3. Defect D2 — Wrong GPU profile on BOTH mobile presets (pixel + iphone)

### 3.1 The bug
`presets.rs:931` (pixel) and `presets.rs:1035` (iphone) both ship
`gpu_profile: crate::gpu::apple_m3_macos()` with an explicit `// TODO`. The JS surface reads
the **whole WebGL fingerprint** from `gpu_profile`, not from the single `webgl_renderer`
string field (`stealth_ext.rs:89-119`):
- `webgl_unmasked_vendor` ← `gpu_profile.unmasked_vendor`
- `webgl_unmasked_renderer` ← `gpu_profile.unmasked_renderer`
- `webgl_params`, `shader_precision`, `extensions`, GL version, shading-language version ←
  all from `gpu_profile`

So the masked `webgl_renderer` string is per-device-correct, but everything a fingerprinter
reads via `WEBGL_debug_renderer_info` + `getParameter` + `getShaderPrecisionFormat` +
`getSupportedExtensions` describes an **Apple M3 Mac**.

### 3.2 What a fingerprinter sees
- **iphone:** `navigator.platform="iPhone"`, UA=Safari/iOS, masked renderer="Apple GPU"
  (correct), **but** unmasked debug renderer + GL params + shader precision + extension list
  = desktop **Apple M3 (macOS Metal/ANGLE)**. Real iOS Safari on an A17 Pro: does not expose
  `WEBGL_debug_renderer_info` the same way, reports a mobile GPU param envelope
  (different `MAX_TEXTURE_SIZE`, `MAX_VIEWPORT_DIMS`, `MAX_RENDERBUFFER_SIZE`, precision
  ranges, and a **WebGL-ES extension set** distinct from desktop). An iOS UA returning a
  macOS-desktop GL param vector is an internal contradiction.
- **pixel:** masked `webgl_renderer="ANGLE (Google, Mali-G715 MP7, OpenGL ES 3.2)"`
  (correct string) **but** unmasked vendor/renderer + GL params + extensions = **Apple M3**.
  Mali claimed in the string, Apple Metal numbers returned. Internally contradictory on an
  Android phone.

### 3.3 Why this maps to the iphone Cloudflare cluster
The iphone gap is **6 Cloudflare-CHL** sites (economist/ecosia/ft/openai/quora/udemy) where
chrome/pixel/firefox all PASS. Cloudflare's Managed Challenge is **bot-score-gated**: a
visitor below the threshold passes uncontested; above it gets the JS challenge page
(§4 — Heuristics + JavaScript-Detections + ML engines). The iphone profile carries TWO
score-raising coherence tells the desktop profiles don't: **D2 (macOS GPU under an iOS UA)**
and **D3 (Safari TLS residual)**. Together they push iphone over the Managed-Challenge
threshold on exactly the CF-fronted sites where the desktop profiles sit under it. The
small response bodies (5891/5444/5843/5929/10807) are the **challenge interstitial**, and
the challenge then fails to solve on the Safari profile (a JS-detection engine probing
WebGL would find the same M3-on-iPhone contradiction during the solve). ft (271064) is a
partially-rendered challenge page. This is a **per-profile bot-score delta**, not a
site-wide block — the proof is that three other profiles pass the identical sites.

### 3.4 Why it also hurts pixel
pixel's WebGL incoherence is a standing tell on any WebGL-cross-checking vendor; it doesn't
explain pixel's THIN-BODY/ERROR gaps (those are nav-reliability, §5) but it is a latent
liability and a near-free fix once the GPU profiles exist.

### 3.5 Cited prior work
The `// TODO: add android_mali_g715 GPU profile` / `// TODO: ios_apple_a17_pro GPU profile`
comments (`presets.rs:931,1035`) are the in-tree admission. The sampler doc
(`presets.rs:728-749`, `macos_sampler_keeps_per_chip_cross_api_consistency` test
`presets.rs:1247-1284`) already establishes the project's own invariant: **(cpu_cores,
gpu_renderer, device_memory) must describe ONE real device, and varying GPU independently
of cores regressed an 8-site AWS WAF sweep 1/8→0/8**. By that exact principle, an
Apple-M3 GPU under a Pixel/iPhone UA is the same class of regression, unmeasured because
no mobile cross-API test exists yet.

---

## 4. Defect D3 — Safari iOS TLS residual + no-Client-Hints coherence (iphone Cloudflare)

Safari iOS TLS **is** implemented (distinct 20-cipher list incl. 3DES tail, duplicated
`rsa_pss_rsae_sha384` Apple bug, no-PQ curves + P-521, FIXED 13-extension order, Zlib cert
compression, NO_TICKET, skip ALPS, skip ECH-grease — `tls.rs:111-183,241-326`). But two
residuals remain that raise the iphone bot score on Cloudflare:

1. **Extension-count / PADDING residual.** `tls.rs:163-183` documents that real Safari emits
   a PADDING extension whose positional ordering requires raw extension injection that is
   **deferred**, and BoringSSL auto-emits PADDING only past ~512 bytes. The implemented
   permutation has **13 extensions** (JA4 `…13…` count); if real Safari's offered-extension
   count differs (PADDING present/absent or an extension BoringSSL can't position), the JA4
   `ext_count` digit and the `ext_sigalg_hash` shift off real Safari. A JA4 that reads
   "almost-Safari-but-not-quite" is worse than Chrome's exact match — it's an
   unrecognized-but-Safari-claiming client, which CF's heuristics engine scores as
   suspicious.
2. **No UA-CH is correct but rare.** Safari sends no `sec-ch-ua*` and BO omits them
   (correct — `safari_headers_impl`, `presets.rs:994` notes). That's right, but it means the
   iphone profile has **less corroborating signal** than Chrome; any single remaining tell
   (D2's GPU, D3's JA4 residual) is not offset, so the score crosses CF's threshold more
   easily than for a Chrome profile that has consistent UA-CH + JA4 + H2 all reinforcing.

Net: D2 + D3 stack so the iphone profile is the one CF decides to challenge. Removing D2
(the bigger, cheaper tell) likely drops the score back under threshold for most of the six;
closing D3's extension-count residual is the second lever.

---

## 5. NON-coherence gaps — do NOT treat as fingerprint tells

### 5.1 Pixel nav-reliability (airbnb THIN-BODY 0, yandex-ru THIN-BODY 0, prime-video ERROR)
These are **empty bodies / hard errors**, not challenge pages. chrome/iphone/firefox all
render airbnb (528–589KB), yandex-ru (2.7–3.2MB), prime-video (508–691KB) fine. A
fingerprint block would produce a challenge interstitial (sized 1–15KB) like the firefox
DataDome rows, not 0 bytes / ERROR. The signature points at a **request-path bug specific
to the MobileAndroid code path** — candidates: (a) a fractional-DPR / mobile-viewport layout
or resource-fetch crash, (b) a mobile redirect (m.* / app-interstitial) the nav loop
doesn't follow, (c) an H2 stream error on these specific large mobile responses. yandex-ru
THIN-BODY 0 on pixel while the desktop profiles pull multi-MB strongly suggests a
mobile-redirect or mobile-CDN host the nav path drops. **This is a reliability fix, not a
preset/TLS fix.** Needs a captured trace (deferred — IP locked).

### 5.2 uber TIMEOUT on chrome/pixel/firefox, PASS on iphone (700KB)
The inverse pattern: only the **iOS** profile completes; the other three time out. This is
almost certainly **content/route**, not fingerprint — uber serves a heavy desktop SPA whose
nav-settle never fires within the timeout (an engine settle/quiescence bug on uber's
desktop bundle), while the iOS UA gets a lighter mobile bundle that settles in time. Treat
as a nav-settle/timeout-budget issue, not a coherence tell.

### 5.3 macys firefox THIN-BODY 0, spotify desktop thin
macys firefox = nav error (THIN-BODY 0) like §5.1 — desktop siblings render 1.2–1.5MB.
spotify chrome 9881 / firefox 9875 (desktop) vs pixel/iphone 147KB (mobile): the desktop
"thin" is L3-RENDERED but under the 15KB bar — a **DataDome desktop soft-gate AND/OR a
legitimately thin desktop shell**; mobile gets the fuller open.spotify shell. Partly
content, partly the D1 firefox tell on the firefox row. Distinguish before acting.

---

## 6. Known artifacts (already understood this session)
- **amazon-ca chrome 5310 / pass on other 3:** AWS-WAF same-IP token-clustering — issuing
  the *same* macOS profile from the *same* datacenter IP trips WAF IP-clustering; the
  `chrome_148_macos_sampled*` fix (`presets.rs:721-830`, FIX-E/E2) and request spacing
  address it. Not a coherence tell; the other 3 profiles diversify enough to pass.
- **homedepot chrome 994KB / Akamai-CHL on pixel/iphone/firefox:** Akamai sec-cpt
  self-solve currently lands only on the Chrome-desktop path; the mobile + firefox profiles
  don't complete the sec-cpt bundle. Tracked separately (Akamai sec-cpt parity), partially
  related to D1/D2 (the mobile/firefox fingerprint deltas) but primarily a challenge-solve
  coverage gap.

---

## 7. External research (2026), with sources

- **Cross-layer (UA-vs-TLS) mismatch is the #1 high-confidence bot signal**, deployed at
  Cloudflare, DataDome, AWS, Akamai. DataDome explicitly: "anti-bot systems check if
  User-Agent matches JA3. Mismatch = instant detection"; "a Firefox user agent paired with
  non-Firefox TLS characteristics … create[s] a detectable mismatch"; Chrome=BoringSSL,
  Firefox=NSS produce distinct fingerprints. This is precisely BO's D1 (Firefox UA over
  BoringSSL JA4). [DataDome detection / ProxyHat](https://proxyhat.com/blog/datadome-detection-residential-proxies),
  [TLS fingerprinting explained / ProxyHat](https://proxyhat.com/blog/tls-fingerprinting-explained),
  [Scrapfly JA3/JA4](https://scrapfly.io/web-scraping-tools/ja3-fingerprint).
- **JA4+ is universal at Cloudflare/AWS/VirusTotal in 2026**, and is engineered to survive
  Chrome's extension shuffle (sorts + strips GREASE) — so BO's Fisher-Yates and GREASE are
  correctly JA4-invisible, but they do nothing to hide the Firefox-UA-over-Chrome-JA4
  mismatch (the JA4 still reads Chrome 147).
  [proxies.sx TLS/JA4 guide](https://www.proxies.sx/use-cases/privacy/tls-fingerprint),
  [Cloudflare JA4 signals](https://blog.cloudflare.com/ja4-signals/),
  [Auth0 JA4 signals](https://auth0.com/blog/strengthening-bot-detection-ja4-signals/).
- **Cloudflare Managed Challenge is bot-score-gated** across a Heuristics engine
  (known-fingerprint patterns), a JavaScript-Detections engine (browser analysis incl.
  WebGL), and an ML engine; only traffic below threshold passes uncontested. This is why a
  single per-profile tell (D2/D3 on iphone) flips a site from pass to challenge while
  sibling profiles below threshold pass.
  [Cloudflare challenges concepts](https://developers.cloudflare.com/cloudflare-challenges/),
  [Capsolver: what is a CF challenge](https://www.capsolver.com/blog/All/what-is-cloudflare-challenge).
- **Cloudflare advises against JA3 rules for mobile** because OS/browser updates shift
  mobile fingerprints — i.e. CF's mobile path leans more on JS/behavioral + ML, which makes
  the iphone WebGL (D2) contradiction directly load-bearing.
  [Cloudflare JA3/JA4 fingerprint docs](https://developers.cloudflare.com/bots/additional-configurations/ja3-ja4-fingerprint/).
- **Camoufox uses the native Firefox NSS network stack** (spoofs only headers + WebRTC IPs),
  so its Firefox JA4 + akamai-H2 are genuine Firefox and coherent with its UA — a strict
  competitive advantage over BO's D1 until BO builds a real Firefox wire class.
  (`audit/02_CAMOUFOX_V150_OVERVIEW.md`; DeepWiki `daijro/camoufox` per NETWORK_fingerprint §3.3.)
- **Reference impls for a Firefox-correct wire stack exist** (lower the D1 effort to
  transcription + boring2 expressivity check): `lexiforest/curl-impersonate` `firefox_*`
  YAML, `0x676e67/wreq-util` `src/emulate/profile/firefox/`, `sardanioss/httpcloak` (Go,
  Chrome+Firefox+Safari TLS/H2/H3). [curl-impersonate](https://github.com/lexiforest/curl-impersonate),
  [wreq-util](https://github.com/0x676e67/wreq-util),
  [httpcloak](https://github.com/sardanioss/httpcloak).
- **Per-browser TLS reality:** Chrome/Safari = BoringSSL-class, Firefox = NSS; each emits a
  distinct cipher/extension/sigalg shape — confirming that one BoringSSL config cannot
  faithfully serve a Firefox UA. [roundproxies BoringSSL](https://roundproxies.com/blog/boringssl/).

---

## 8. Ranked fix list (per-profile site gain · effort · confidence · public-engine)

> All wire-layer + preset fixes live in public `crates/net` / `crates/stealth` per
> `CLAUDE.md` ("HTTP/TLS: own stack in `crates/net/`"). None is per-vendor solver code, so
> all are **public-engine**. Vendor *challenge solving* stays in `vendor_solvers`.

### FIX-P1 — Add real `android_mali_g715` + `ios_apple_a17_pro` GPU profiles and wire them (closes D2)
**What:** Author two `GpuProfile`s in `crates/stealth/src/gpu.rs` (unmasked vendor/renderer,
GL params, shader precision, extension list, GL + shading-language versions) for a Pixel 9
Pro Mali-G715 (WebGL-ES, ANGLE/Vulkan-on-Android param envelope) and an iPhone 15 Pro A17
Pro (Apple GPU, iOS-Safari WebGL-ES param envelope, masked `Apple GPU`). Set them at
`presets.rs:931` and `:1035` replacing `apple_m3_macos()`. Add a mobile analogue of
`macos_sampler_keeps_per_chip_cross_api_consistency` asserting (platform, UA, gpu_renderer,
GL params) describe ONE real device.
**Per-profile gain:** iphone — likely drops bot score under CF threshold for several of
{economist, ecosia, ft, openai, quora, udemy} (est. **+3 to +5 iphone**); pixel — removes a
standing WebGL tell (latent, may help homedepot-Akamai mobile; est. **+0 to +1 pixel**
immediate, larger as a coherence floor). **Effort:** 1–2 days (param vectors transcribable
from public WebGL-report dumps; no wire work). **Confidence:** medium-high for iphone
(matches the bot-score-gating mechanism + CF mobile-leans-on-JS evidence; needs a sweep to
confirm exact count). **Public engine:** yes. **Best ROI — do first.**

### FIX-P2 — Close the Safari iOS extension-count / PADDING residual (sharpens D3)
**What:** Verify BO's iphone JA4 `ext_count` + `ext_sigalg_hash` against a real iOS Safari
18 capture (`tls.peet.ws/api/all` from a real device / BrowserStack — out-of-band, not the
locked IP). If the offered-extension count or PADDING presence differs, add raw PADDING
extension injection (deferred at `tls.rs:163-183,341-351`) so the count matches exactly.
**Per-profile gain:** iphone — the second lever on the CF cluster after P1 (est. **+1 to
+3 iphone**, overlapping P1's set; together P1+P2 target the full 6). **Effort:** 2–4 days
(raw extension injection in boring2 is the hard part; capture is easy). **Confidence:**
medium (residual is documented but its JA4 impact is unmeasured without a real-Safari
baseline). **Public engine:** yes.

### FIX-P3 — Build the real Firefox 135 TLS + HTTP/2 class (closes D1)
**What:** Add a Firefox branch keyed on `browser_name == "Firefox"` (or a new
`DeviceClass`/predicate) in BOTH `tls.rs::chrome_connector` (Firefox cipher list, NSS
extension set/order with **no Fisher-Yates**, **no ALPS**, **no ECH-grease**, different
sigalgs/curves, **no Brotli** cert compression) and `h2_client.rs::handshake`
(`HEADER_TABLE_SIZE=65536, ENABLE_PUSH=0, INITIAL_WINDOW_SIZE=131072, MAX_FRAME_SIZE=16384`,
pseudo-order `m,p,a,s`, no priority hint). Transcribe constants from `curl-impersonate`
`firefox_*` + `wreq-util` firefox profile; add a `firefox_h2_settings` byte test + a Firefox
JA4 baseline. **Risk:** confirm boring2 can express Firefox's extension order/set; if Firefox
offers an extension boring2 can't position, that is the hard part.
**Per-profile gain:** firefox — directly targets **reuters, wsj, tripadvisor (DataDome),
zillow (PX)** = **+3 to +4 firefox**, plus converts current "accidental, fragile" firefox
wins (adidas-class) into robust ones. **Effort:** 1–2 weeks (mostly reference transcription
+ boring2 expressivity validation + capture). **Confidence:** medium (impact mechanism is
proven by D1's data signature + §7 DataDome quote; exact count needs the sweep). **Public
engine:** yes. **Highest blast radius for firefox; biggest effort.**

### FIX-P4 — Interim Firefox-routing guard (cheap D1 risk reduction until P3 lands)
**What:** Until P3 ships, gate Firefox-UA routing AWAY from DataDome/PerimeterX/Akamai/CF
JA4-cross-checking sites; only route firefox where UA+headers alone flip the decision.
Routing-table flag, no wire changes.
**Per-profile gain:** prevents firefox from being *routed into* a JA4 trap; protects current
firefox wins (does not by itself flip the 4 gap sites — P3 does that). **Effort:** 0.5–1 day.
**Confidence:** medium-high. **Public engine:** yes. (Mirrors NETWORK_fingerprint FIX-N2.)

### FIX-P5 — Capture per-profile JA4 + akamai_fingerprint baselines into tree (verification)
**What:** Out-of-band (not the locked IP) hit `tls.peet.ws/api/all` per profile; store
`ja4.txt` + `akamai_h2.txt` under `crates/net/tests/captures/<profile>/`; upgrade
`test_tls_fingerprint_peet` from `t13d`-prefix to exact-match. This is what makes P2/P3
measurable (you cannot confirm a Firefox/Safari JA4 fix without a baseline) and arms the
silent-drift guard.
**Per-profile gain:** 0 direct; unblocks P2/P3 verification. **Effort:** 0.5–1 day.
**Confidence:** high. **Public engine:** yes. (Mirrors NETWORK_fingerprint FIX-N1.)

### FIX-P6 — Mobile/desktop nav-reliability bugs (separate track — NOT a coherence fix)
**What:** Capture traces (out-of-band) for pixel airbnb/yandex-ru/prime-video (THIN-BODY 0 /
ERROR), firefox macys (THIN-BODY 0), and the uber TIMEOUT-on-3-profiles case; fix the
MobileAndroid request/redirect/H2-stream path and the desktop nav-settle/timeout budget.
**Per-profile gain:** pixel **+2 to +3** (airbnb, yandex-ru, prime-video), firefox **+1**
(macys), chrome/pixel/firefox **+1** (uber) — but these are reliability fixes, distinct from
the fingerprint work; do not conflate. **Effort:** 2–5 days (trace-dependent). **Confidence:**
medium (cause inferred from 0-byte/ERROR signature, not yet traced). **Public engine:** yes.

### Summary of expected per-profile movement
| Profile | now | after P1 | after P1+P2 | after P3 | after P6 | target |
|---|--:|--:|--:|--:|--:|--:|
| iphone | 108 | ~111–113 | ~113–114 | — | — | ~114 |
| firefox | 106 | — | — | ~109–110 | ~110–111 | ~110+ |
| pixel | 108 | ~108–109 | ~108–109 | — | ~110–112 | ~111+ |
| chrome | 110 | — | — | — | ~111 (uber) | ~111+ |

Ordering by ROI: **P1 (D2, cheap+broad iphone) → P5 (verification, unblocks the rest) →
P2 (D3) → P3 (D1, big firefox win) → P4 (interim guard if P3 slips) → P6 (reliability,
parallel track).**

---

## 9. Files referenced
- `crates/stealth/src/presets.rs` — `chrome_148_macos` (121-202), `firefox_135_macos`
  (421-505, `device_class:Desktop`=473, aspirational `tls_impersonate`=474),
  `pixel_9_pro_chrome_148` (850-933, GPU bug=931), `iphone_15_pro_safari_18` (956-1037,
  GPU bug=1035), macOS sampler cross-API invariant (721-830, test 1247-1284).
- `crates/net/src/tls.rs` — device_class-only branch (241-246), Chrome constants (60-220),
  Safari iOS constants + deferred PADDING (111-183, 341-351), silent-drift test (476-553).
- `crates/net/src/h2_client.rs` — device_class-only branch (85-130), Chrome vs Safari
  SETTINGS/pseudo-order.
- `crates/net/src/headers.rs` — `firefox_headers*` (594-706), `safari_headers*` (721-803).
- `crates/js_runtime/src/extensions/stealth_ext.rs` — WebGL surface from `gpu_profile`
  (89-119).
- `docs/v0.1.0-match-workflows/00_DATA_per_profile_matrix.md` — the per-profile tag/len matrix.
- `docs/v0.1.0-parity-workflows/external/NETWORK_fingerprint.md` — L1 Firefox leak, FIX-N1/N2/N3.
- `docs/releases/v0.1.0-parity/23_TLS_HTTP_FINGERPRINT_REFERENCE.md` — TLS/H2 impl reference,
  §1.4 codename table, §5.3 Firefox-deferred.
- `docs/releases/v0.1.0-parity/39_NETWORK_LAYER_FINGERPRINTING.md` — cross-vendor network view.
- `docs/releases/v0.1.0-parity/audit/02_CAMOUFOX_V150_OVERVIEW.md` — Camoufox native-NSS context.
</content>
</invoke>
