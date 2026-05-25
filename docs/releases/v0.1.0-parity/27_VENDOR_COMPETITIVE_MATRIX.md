# 27 — Per-vendor competitive matrix

**Status:** reference
**Audience:** customer-pitch authors, product / sales doc maintainers, vendor-strategy decision-makers, and any contributor scoping a new vendor in `02_GAP_ANALYSIS.md` / `08_KASADA_FRONTIER.md`.
**Companion docs:** `12_COMPETITIVE_LANDSCAPE.md` (engine-by-engine; this chapter is vendor-by-vendor), `06_AWS_WAF_SOLVER.md`, `07_DATADOME_PRIMITIVES.md`, `08_KASADA_FRONTIER.md`, `18_ANTI_BOT_VENDOR_COOKBOOK.md`, `26_AKAMAI_BMP_DEEP.md`, `11_PER_PROFILE_STRATEGY.md`, `01_CURRENT_STATE.md`.

**One-paragraph thesis:** Chapter 12 ranks engines (Camoufox 113 > BO routed 108 > Playwright family 88-87). Chapter 27 ranks **vendors** — Cloudflare is easier than AWS WAF, AWS WAF is easier than Kasada — and shows which engine wins which vendor and why. The pattern is structural: real-browser-engine engines (Camoufox = real Firefox, Playwright = real Chromium) trivially defeat fingerprint-only vendors (Akamai BMP on adidas-class, AWS WAF on amazon-class) but lose to CDP-detecting vendors (Cloudflare on quora/openai class, DataDome on etsy/yelp). BO's own-engine + per-profile-routing + byte-perfect-Chrome-TLS combination wins where (a) the vendor checks TLS class first (PerimeterX on zillow, our own win), and (b) the vendor's CHL is conventionally handled by simple JS-execution which we satisfy faithfully. Where BO loses today is: per-vendor-specific encoder requirements that have been moved to private `vendor_solvers` per `aecdf19` (AWS WAF `getToken`, DataDome WASM iframe, Akamai sensor_data, Kasada `/tl` PoW). The customer pitch is: **"BO for low-memory + own-process + 4-profile routing; Camoufox for max-pass-today; Playwright family for CDP API + amazon-class WAF + you don't mind 5+ GB RSS."**

---

## 1. The matrix

Source: `/tmp/full_sweep_2026_05_24/{bo_*_cold,comp_*}.json`. Strict Pass = `tag == "L3-RENDERED" AND len ≥ 15000` (per `03_BENCHMARK_METHODOLOGY.md`). All numbers are sites-passing-strict over sites-in-vendor-cluster.

| Vendor | Cluster size | BO chrome | BO pixel | BO iphone | BO firefox | **BO routed** | **Camoufox** | Patchright | Playwright | PW+stealth | Notes |
|---|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|---|
| **AWS WAF (Challenge)** | 5 (amazon-de, amazon-in, amazon-com-au, imdb, amazon-jp) | 0 | 0 | 0 | 0 | **0** | 1 | 4 | 4 | 3 | Amazon WAF-tenant cluster; Camoufox catches amazon-jp on a good roll; PW family wins via real Chrome trust. Imdb specifically: BO 1995 stub × 4, Patchright `BLOCKED 117`, PW `BLOCKED 117`, Camoufox `L3 1080960`. |
| **AWS WAF (variable / lenient)** | 3 (amazon-com, amazon-ca, amazon-co-uk, amazon-fr) | 1 | 1 | 1 | 2 | **3** | 4 | 4 | 4 | 4 | These rolls flip non-deterministically; per-profile routing recovers most. amazon-com = BO firefox unique, amazon-ca = BO pixel unique, amazon-co-uk = BO chrome/iphone/firefox win (pixel loses), amazon-fr = BO iphone/firefox win. |
| **AWS WAF total** | 8 | 1 | 1 | 2 | 3 | **4** | **5** | 7 | 7 | 6 | The largest single-category gap. Per `02_GAP_ANALYSIS.md §5-8` and `06_AWS_WAF_SOLVER.md`. |
| **DataDome (silent rt:'i')** | 2 (etsy, tripadvisor) | 0 | 1 | 1 | 0 | **1** | 2 | 0 | 0 | 0 | tripadvisor flips on pixel + iphone for BO. etsy fails on all BO profiles. Camoufox flips both. Per `07_DATADOME_PRIMITIVES.md`. |
| **DataDome (interactive rt:'c')** | 1 (yelp) | 0 | 0 | 1 | 0 | **1** | 0 | 0 | 0 | 0 | Yelp serves DataDome interactive captcha to most engines; **iphone-class uniquely gets the silent variant**. Even Camoufox fails (`DataDome-CHL 1487`). |
| **DataDome (firefox-targeted)** | 1 (wsj) | 1 | 1 | 1 | 0 | **1** | 1 | 0 | 0 | 0 | DataDome treats Firefox harshly on wsj — both BO firefox AND PW family fail. |
| **DataDome total** | 4 | 1 | 2 | 3 | 0 | **3** | **3** | 0 | 0 | 0 | (also leboncoin — BO + Camoufox all pass; PW family `DataDome-CHL 1468`) |
| **Akamai BMP / sec-cpt** | 3 (homedepot, bestbuy, adidas) | 0 | 0 | 0 | 1 | **1** | 0 | 1 | 1 | 1 | adidas is BO firefox unique (1.3 MB body, see `26 §4.1`). homedepot: BO + Camoufox all fail; PW family wins (real-Chrome trust). bestbuy: SPA splash; loose-L3 pass everywhere, strict-L3 fail everywhere (`02 §hard-residual`). |
| **Kasada** | 3 (canadagoose, hyatt, realtor) | 0 | 0 | 0 | 0 | **0** | 0 | 1 (hyatt) | 1 (hyatt) | 1 (hyatt) | Open-source SOTA frontier. Even Camoufox fails all 3 strict. Patchright/PW flip hyatt to `L3 13228` (loose-L3 just below the gate); fail canadagoose/realtor. Per `08_KASADA_FRONTIER.md`. |
| **Cloudflare (Managed Challenge, iphone-class)** | 6 (udemy, weather, economist, ft, ecosia, quora, openai) | 7 | 7 | 1 | 7 | **7** | 7 | 0 | 0 | 0 | iphone profile *uniquely loses* 6 of 7 to CF Managed Challenge (the iOS Safari JA4 is a small-share class CF undertrusts). PW family loses all. Camoufox wins all 7 because Firefox class is CF-trusted. |
| **Cloudflare (general)** | many (cross-corpus) | high pass | high pass | mixed | high pass | high | high | low | low | low | Most CF-fronted sites in corpus pass for BO; CF only blocks PW family due to CDP-detect. |
| **PerimeterX / HUMAN** | 1 (zillow — chl-known); 1 (wayfair — PaH from PW only) | 1 | 1 | 1 | 0 | **1** | 0 | 0 | 0 | 0 | **BO advantage**: zillow passes 3/4 BO profiles; Camoufox + PW family ALL fail (`PerimeterX-PaH ≤ 14 KB`). |
| **Akamai Edge (BM Edge, walmart-class)** | 1 (walmart) | 1 | 1 | 1 | 1 | **1** | 1 | 1 | 1 | 1 | Parity. `bm_sz`-only / passive scoring; all engines clear. |
| **wbaas (Walmart)** | covered above | 1 | 1 | 1 | 1 | **1** | 1 | 1 | 1 | 1 | Walmart's in-house product is `bm_sz`-paired; parity. |
| **Imperva / Reblaze / Sucuri** | 0 in corpus | — | — | — | — | — | — | — | — | — | Not in our 126-corpus; documented in `18 §2.7-2.8` and §5 below. |
| **areyouheadless (diagnostic)** | 1 | 0 | 0 | 0 | 0 | **0** | 0 | 0 | 0 | 0 | Intentional probe — never going to pass cleanly on any engine. Documented in `02 §hard-residual`. |
| **wildberries (custom RU shell)** | 1 | 0 | 0 | 1 (loose) | 0 | **0** | 0 | 0 | 0 | 0 | SPA shell; cross-engine fail at strict-pass. iphone has loose `L3 7900`; Camoufox `L3 8924` (also below 15 KB gate). |

### 1.1 Vendor cluster totals (the headline)

Roll the vendor-cluster wins into a single pass-count table (this is what the customer-pitch lines in §9 quote):

| Vendor cluster | Sites in cluster | BO routed wins | Camoufox wins | Patchright wins | Playwright wins |
|---|--:|--:|--:|--:|--:|
| AWS WAF | 8 | 4 | 5 | 7 | 7 |
| DataDome | 4 | 3 | 3 | 0 | 0 |
| Akamai BMP | 3 | 1 | 0 | 1 | 1 |
| Kasada | 3 | 0 | 0 | 1 | 1 |
| Cloudflare Managed (iphone-targeted) | 7 | 7 | 7 | 0 | 0 |
| PerimeterX (zillow) | 1 | 1 | 0 | 0 | 0 |
| Akamai Edge / wbaas / parity | 1 | 1 | 1 | 1 | 1 |
| **Total (these vendor clusters)** | **27** | **17** | **16** | **10** | **10** |

The vendor matrix re-summarises the engine matrix (`12_COMPETITIVE_LANDSCAPE.md §3`): BO and Camoufox are tied at the WAF-tier (within noise floor); Patchright/Playwright lose Cloudflare + DataDome + PerimeterX dramatically, win AWS WAF + Akamai cleanly via real-Chrome trust.

---

## 2. Where BO structurally wins vs Camoufox (the verified-real wins)

These are the sites where BO routed passes strict and Camoufox does not, per `12 §3.2` cross-checked against `/tmp/full_sweep_2026_05_24/comp_camoufox.json`:

| Site | Vendor | BO profile that wins | Camoufox result | Why BO wins |
|---|---|---|---|---|
| adidas | Akamai BMP | firefox (1.3 MB body) | `L3 2384` (interstitial-sized) | Per `26 §4.1`: `vendor=""` + masked WebGL + Firefox UA + Chrome-class TLS appears to dodge Akamai's bot-pattern correlation. Hypothesis under live-capture confirmation. |
| amazon-ca | AWS WAF | pixel | `L3 5524` (small benign response) | Pixel mobile UA gets a non-WAF Amazon serve. Camoufox's Firefox UA gets the (smaller) mobile-style page but doesn't trigger full hydration. |
| amazon-com | AWS WAF | firefox | `L3 5540` | Firefox UA gets a smaller benign response (5540 B); BO firefox hydrates to 949 KB. |
| yelp | DataDome | iphone | `DataDome-CHL 1487` (interactive captcha) | iOS Safari class triggers a different DataDome serving rule that issues the silent variant. |
| zillow | PerimeterX | chrome / pixel / iphone (firefox loses) | `PerimeterX-PaH 10866` (Press-and-Hold widget) | **The marquee BO advantage.** Our byte-perfect-Chrome TLS + headers + JS surface beats PerimeterX; Camoufox's Firefox-class TLS triggers PaH escalation. |

Additional non-strict but practical wins (loose-L3 / category coverage):

- **leboncoin** (DataDome): BO and Camoufox both pass; PW family fails. (BO not "uniquely" wins vs Camoufox here — joint win.)
- **5 sites total** where BO routed beats Camoufox per `12 §3.2`. Camoufox routed beats BO by 10 sites per `12 §3.1`. Net: Camoufox leads 113 → 108.

### 2.1 The deeper pattern

BO wins where Camoufox loses because:

1. **PerimeterX zillow** — PerimeterX's scoring weights TLS class. Real Firefox = high-prior bot signal (the `_px3` 60-s lifetime is designed around this — Firefox-bot-traffic is over-represented). Chrome-class TLS + Chrome UA + clean Chrome JS surface = trusted.
2. **Akamai adidas (firefox profile)** — see hypothesis breakdown in `26 §4.1`. The firefox profile's combination of features is internally consistent but rare enough that Akamai's risk model doesn't have a confident-bot anchor.
3. **AWS WAF (amazon-ca pixel, amazon-com firefox)** — per-profile risk-class diversity. Camoufox runs ONE profile (Firefox-on-host-OS). BO runs ANY of 4 (chrome, pixel, iphone, firefox). Coverage compounds.
4. **DataDome yelp (iphone)** — iOS Safari class triggers DataDome's lower-tier-protection rules at yelp's tenant. Camoufox is locked to Firefox UA + Firefox JA4 → always gets the interactive captcha.

The structural lesson: **multi-profile + own-engine-TLS = a routing premium that single-engine vendors can't match without changing UA, which they don't.**

---

## 3. Where BO structurally loses to Camoufox (the recoverable surface)

The 10 sites where Camoufox passes strict and BO routed does not. Cross-reference `02_GAP_ANALYSIS.md` and `12 §3.1`:

| Site | Vendor | Cluster | Why Camoufox wins | BO restoration plan |
|---|---|---|---|---|
| amazon-de | AWS WAF | Amazon WAF-tenant | Real Firefox JIT timing + real WebAssembly; AWS WAF challenge.js's getToken() runs to completion | Chapter 06 |
| amazon-in | AWS WAF | same | same | Chapter 06 |
| amazon-com-au | AWS WAF | same | same | Chapter 06 |
| imdb | AWS WAF | same (Amazon-tenant) | same | Chapter 06 |
| etsy | DataDome | DataDome silent rt:'i' | Camoufox's iframe handling + cross-origin CSP allowance lets `dd-script.js` materialize the challenge iframe | Chapter 07 §Primitives 1+2+3 |
| reddit | proof-of-execution challenge | small form-submit JS gate | Real Firefox's `requestSubmit()` actually fires the form submit; BO's likely doesn't reach the loop's `PENDING_NAV_JS` reader in time | Chapter 05 (EASY) |
| duolingo | reCAPTCHA Enterprise via Worker | Worker(webworker.js) | Real Firefox Worker satisfies recaptcha's `grecaptcha.execute()` worker-side scoring | Chapter 05 (close miss — 13.5 KB to gate) |
| booking | SPA hydration | server-side React | Camoufox completes the fetch chain that BO drops | Chapter 05 |
| douyin | TikTok-CN custom anti-bot | `__ac_signature` + `ttwid` | Camoufox's Firefox env satisfies the signature check | Chapter 05 |
| x-com | TLS / rate-limit | mid-sweep cookie bleed | Camoufox's session isolation isn't subject to BO's SharedSession bleed (`f62584d`) | Chapter 05 |

After chapters 05 + 06 + 07 + 26 ship, the routed delta is +6 to +10 sites — putting BO routed at 114-118, past the Camoufox 113 reference.

---

## 4. Where everyone loses (the open-source frontier)

These 8 sites fail across BO + Camoufox + Playwright family. They're the open-source SOTA frontier per `02 §hard-residual`. Cross-checked against `/tmp/full_sweep_2026_05_24/comp_camoufox.json`:

| Site | Vendor | BO routed | Camoufox | Best engine | Chapter |
|---|---|---|---|---|---|
| homedepot | Akamai sec-cpt | `Akamai-CHL 2754` | `Akamai-CHL 2638` | Patchright `L3 1245838` | 26 (post-strip restoration needed) |
| realtor | Kasada | `Kasada-CHL 1764` | `Kasada-CHL 1772` | (none — all CHL) | 08 |
| canadagoose | Kasada | `Kasada-CHL 732` | `Kasada-CHL 740` | (none — all CHL) | 08 |
| hyatt | Kasada | `Kasada-CHL 737` | `Kasada-CHL 745` | Patchright `L3 13228` (loose-L3 just below 15K gate) | 08 |
| wildberries | custom RU shell | `L3 7900 iphone` | `L3 8924` | (Camoufox — but still below 15 KB gate) | 02 (hard residual) |
| areyouheadless | diagnostic probe | `L3 3653` | `L3 3668` | (intentional fail by design) | 02 (hard residual) |
| amazon-jp | AWS WAF | `L3 2011` ×4 | `L3 5635` | Patchright `L3 913300` | 06 (depends on getToken solver) |
| bestbuy | Akamai SPA shell | `L3 7887` | `L3 7465` | (none — splash, not a CHL) | 26 (out of v0.1.0 scope) |

Two distinct patterns:

1. **Kasada (canadagoose, hyatt, realtor)** — Camoufox is the SOTA but it ALSO fails. Not engine-fixable in v0.1.0. Per `08 §6 Lever 1`, the K2-DIFF in-VM plaintext-sensor dump is the unblocking lever.
2. **AWS WAF amazon-jp** — Playwright family wins via real-Chrome trust; BO and Camoufox both lose. Implies amazon-jp's WAF tenant is more aggressive than amazon-com/co-uk; even a real-Firefox env can fail when the WAF risk model is high-tier.

---

## 5. Per-vendor "what does Camoufox do differently"

Per vendor cluster, the structural advantage Camoufox has over BO at HEAD (and why):

### 5.1 AWS WAF — real Firefox JIT timing + real WebAssembly

The `challenge.js` SDK (loaded from `*.token.awswaf.com/.../challenge.js`) fingerprints 50+ navigator/screen/WebGL/AudioContext properties (per `06_AWS_WAF_SOLVER.md` analysis + `02_GAP_ANALYSIS.md §5-8`), then runs a HashcashScrypt / SHA-256 / NetworkBandwidth-variant PoW in WebAssembly.

What Camoufox does that BO doesn't:
- Real Firefox's V8-equivalent (SpiderMonkey) has well-known timing characteristics that AWS WAF's risk model trusts.
- Real Firefox's WASM has the exact compile + execute timing curve that the SDK measures.
- Real Firefox's `navigator.connection.rtt/downlink` is real (BO ships preset values).
- Camoufox's C++ patches make spoofed properties indistinguishable from native (BO has the same property — own-engine Rust impl — but the *implementation* of getters like `webgl.getParameter` differs from real Firefox).

What BO needs (per `06_AWS_WAF_SOLVER.md`): either patch the engine to be JIT-time-indistinguishable (impractical) or ship a Rust solver in `vendor_solvers` that POSTs a forged `aws-waf-token`. Chapter 06 evaluates both.

### 5.2 DataDome — spoofing context layer handles `i.js` correctly

DataDome's challenge document is a 1424-byte interstitial that loads `dd-script.js` from `captcha-delivery.com`, which materializes a cross-origin iframe to `geo.captcha-delivery.com/captcha/?...`. The iframe runs WASM that computes Picasso canvas + audio fingerprints and POSTs a check.

What Camoufox does that BO doesn't (per `07_DATADOME_PRIMITIVES.md`):
- Real Firefox's iframe loading doesn't enforce the origin's strict CSP on the challenge document (browser-internal interstitial-handling special case).
- Real Firefox creates the cross-origin iframe + executes its WASM, which BO's post-`aecdf19` engine doesn't (because the trigger flag `started_as_dd_challenge` is solver-only).

What BO needs: chapter 07 §Primitives 1/2/3. After those land, BO matches Camoufox on etsy + tripadvisor (yelp is the interactive-captcha variant — even Camoufox fails).

### 5.3 Akamai BMP — real Firefox passes sensor_data trivially

Real Firefox's full JS env + masked WebGL + correct OfflineAudioContext samples + native canvas paint sequence produces a `sensor_data` payload that Akamai's risk engine accepts. BO's preset matches the surface but the **rendered audio buffer** (the largest hash input per `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/tier1_priority_for_akamai.md`) has a ~60 ppm drift from Blink's reference at the sum level, which may correspond to per-sample differences at the f32-bit-accuracy level.

What Camoufox does that BO doesn't:
- Actually IS Firefox at the audio kernel layer. The DynamicsCompressor + Oscillator output is the same bytes Akamai expects from a real Firefox.

What BO needs: chapter 26 §3.A (cookie state machine) for engine-side correctness; vendor_solvers sensor_data encoder (chapter 26 §6 Steps 5+6) for sites where the in-V8 bundle self-solve doesn't suffice. Note: adidas works on BO firefox via a different mechanism (per `26 §4.1` — likely a UA+TLS-class scoring bypass, not a bit-accurate audio fingerprint match).

### 5.4 Kasada — open frontier (Camoufox is SOTA but not deterministic)

Per `08_KASADA_FRONTIER.md`: Camoufox is the closest-to-passing on canadagoose/hyatt/realtor but it ALSO fails all three. The hypothesis is **passive engine-vs-real-Chrome surface divergence** — the `/tl` POST sensor body has discriminating fields that flag headless-class engines.

What Camoufox does that BO doesn't:
- Slightly closer to passing because Firefox-class TLS is rare (Kasada has fewer training samples). Insufficient to flip.

What BO needs: chapter 08 levers — the K2-DIFF in-VM plaintext-sensor dump, the 16 error-bearing fields (`bot1225` `csc` `kl` `dpv` `smc` `sfc` `sdt` `nppm` `fsc` `npc` `esd` `wse` `bfe` `ao` `cbf`), CSS calc math gap (sin/cos/tan in `calc()`). Out of v0.1.0 scope.

### 5.5 Cloudflare — real Firefox JS env trivially passes Managed Challenge

CF's Managed Challenge orchestrator (the `/cdn-cgi/challenge-platform/h/b/jsd/...` script) fingerprints + scores; CF's risk model weights `cf-bm` cookie + JA4 + UA combination. For Firefox-on-macOS class, the JA4 + UA is well-represented in CF's trusted-real-user dataset.

What Camoufox does that BO doesn't:
- Camoufox produces a Firefox JA4 (NSS TLS) that CF's model trusts.
- BO's `firefox_135_macos` profile currently ships Chrome-class TLS (per `11 §7.6`), so the JA4 + UA combination is *internally inconsistent*. Most CF tenants ignore this; the 6-7 iphone losses on iphone-class are the exception.

What BO needs: real Gecko TLS (Phase B.3, post-v0.1.0). Until then: route around iphone-class for CF-protected sites (`11 §4.1 rule 4`).

### 5.6 PerimeterX — Camoufox LOSES because Firefox TLS is the wrong class

Inverted from §5.5. PerimeterX (HUMAN) on zillow's tenant treats Firefox harshly — possibly because Firefox-bot traffic is over-represented in their training data. Real Firefox = high-prior bot.

What Camoufox does that BO doesn't:
- Camoufox is Firefox; can't dodge this.
- BO has chrome/pixel/iphone profiles that don't have Firefox-class TLS → uniquely wins zillow.

What BO needs: ALREADY HAS. zillow is one of the 5 BO-routed-wins listed in `12 §3.2`.

---

## 6. Per-vendor "what does Patchright do differently"

Patchright (Playwright + CDP-leak patches per `12_COMPETITIVE_LANDSCAPE.md §1.3`) patches:

- `navigator.webdriver === undefined` (vs Playwright's `true`)
- `--enable-automation` removed; `--disable-blink-features=AutomationControlled` added
- `Runtime.enable` not called (uses isolated ExecutionContexts)
- Closed Shadow Roots handled

Per-vendor effect from `/tmp/full_sweep_2026_05_24/comp_patchright.json` vs `comp_playwright.json`:

| Vendor | Patchright pass | Playwright pass | Δ |
|---|--:|--:|--:|
| AWS WAF | 7/8 | 7/8 | 0 |
| DataDome | 0/4 | 0/4 | 0 |
| Akamai BMP | 1/3 (homedepot) | 1/3 (homedepot) | 0 |
| Kasada | 1/3 (hyatt loose) | 1/3 (hyatt loose) | 0 |
| Cloudflare Managed (7) | 0/7 | 0/7 | 0 |
| PerimeterX zillow | 0/1 | 0/1 | 0 |
| imdb (AWS WAF) | BLOCKED 117 | BLOCKED 117 | 0 |

**Per-vendor: Patchright = Playwright within noise floor on every cluster.**

The interpretation per `12 §1.3`:

> Patchright defeats the easiest tier of CDP-driver detection (navigator.webdriver, Runtime.enable timing, --enable-automation flag). The heavy vendors (Cloudflare, AWS WAF, DataDome, Kasada) check **far more than that**. Our measured number contradicts Patchright's claim of passing those vendors.

Concretely: CDP hiding alone isn't enough. The underlying chromium fingerprint (font lists, WebGL bytes, WASM JIT timing) is what vendors fingerprint. Patchright cannot hide what real Chromium reports.

### 6.1 Why playwright-stealth is one site BEHIND Playwright

`comp_playwright_stealth.json` shows `pass=87` vs Playwright `pass=88`. The single missing site is amazon-ca: `playwright_stealth L3-RENDERED 5524` (the small mobile-style response) vs `playwright L3-RENDERED 1184173` (full hydration). The stealth's `addInitScript` override on `navigator.userAgent` / `navigator.languages` evidently shifts amazon-ca's WAF risk-class onto the failing branch. Per `12 §1.4`:

> "Not perfect." JS-injected overrides leave a `Function.prototype.toString` and property-descriptor signature that sophisticated vendors detect.

The amazon-ca regression is consistent with that warning — AWS WAF's `challenge.js` does inspect `navigator.userAgent.toString` via the `Function.prototype.toString` chain.

---

## 7. Routing wins from vendor analysis

Per `11_PER_PROFILE_STRATEGY.md §4.1`, the routing decision tree pre-selects a profile by vendor class. Updated per vendor matrix above:

### 7.1 AWS WAF (amazon family)

| Site | Best BO profile | Why |
|---|---|---|
| amazon-com | firefox | Firefox UA → AWS WAF lower-risk class for `.com` |
| amazon-ca | pixel | Pixel mobile UA → non-WAF mobile-style serve |
| amazon-co-uk | chrome/iphone/firefox (avoid pixel) | Pixel UA-CH set scores worse on this tenant |
| amazon-fr | iphone/firefox (avoid chrome/pixel) | Same tenant pattern |
| amazon-de, amazon-in, amazon-com-au, imdb, amazon-jp | (none — needs chapter 06) | Consistent across all 4 BO profiles |

**Recommended routing:** `pick_first_profile = firefox` for `amazon.*` per `11 §4.1 rule 1`. Fallback `iphone → pixel → chrome`.

### 7.2 Kasada

Per `08_KASADA_FRONTIER.md`: all 3 sites fail every profile. The `pixel slightly better than chrome` ordering in `02 §recovery` table is noise-level. **Recommended routing:** `pick_first_profile = pixel` for known-Kasada list; fallback firefox → chrome → iphone. No real per-profile lever until chapter 08 work lands.

### 7.3 Cloudflare Managed Challenge

Per `11 §4.1 rule 4` + `26 §4.1 hypothesis a` connection (TLS-class trust):

| Cluster | First | Avoid |
|---|---|---|
| ecosia, economist, ft, openai, quora, udemy, weather (the iphone-uniquely-fails 7) | chrome or firefox | **iphone** (6 losses concentrated here) |

**Recommended routing:** `pick_first_profile = chrome` for `KNOWN_CLOUDFLARE`. Fallback pixel → firefox. **Skip iphone.**

### 7.4 DataDome

Per `26 §4.1` and `07_DATADOME_PRIMITIVES.md` measurement:

| Site | Best BO profile | Camoufox parity |
|---|---|---|
| etsy | (none on BO until chapter 07) | Camoufox passes |
| tripadvisor | pixel or iphone | Camoufox passes; BO wins post-chapter-07 |
| wsj | chrome/pixel/iphone (avoid firefox) | Camoufox passes; BO firefox uniquely fails |
| yelp | iphone (gets silent variant) | Camoufox fails (interactive variant) |
| leboncoin | any | All BO pass; PW family fails |

**Recommended routing:** `pick_first_profile = iphone` for `KNOWN_DATADOME` per `11 §4.1 rule 2`. Special-case wsj to chrome.

### 7.5 Akamai BMP

Per `26 §4`:

| Site | Best BO profile |
|---|---|
| adidas | **firefox** (the only profile that flips the 2494 → 1.3 MB threshold today) |
| homedepot | iphone (per pre-strip `b623d5d`; requires chapter 26 §3 + chapter 07 §P1) |
| bestbuy | (any — splash; below strict-pass everywhere) |
| walmart | any (passive scoring; parity) |

**Recommended routing:** `pick_first_profile = firefox` for `KNOWN_AKAMAI` (adidas-class); special-case homedepot to iphone.

### 7.6 PerimeterX

Per `11 §4.1 rule 3`:

| Site | First | Avoid |
|---|---|---|
| zillow | chrome/pixel/iphone | **firefox** (escalates to Press-and-Hold) |

**Recommended routing:** `pick_first_profile = pixel` (cheapest RSS, passes) for `KNOWN_PERIMETERX`.

---

## 8. Cross-engine attack/defense table

Per-vendor: which engines does the vendor trust vs filter? Rows are vendor classes; columns are engines.

Legend: ✓ trusted, ✗ filtered, ~ variable/mixed, blank = not tested in cluster.

| Vendor / class | Real Chrome | Real Firefox | BO chrome | BO pixel | BO iphone | BO firefox | PW Chromium | Patchright |
|---|---|---|---|---|---|---|---|---|
| AWS WAF (Amazon-de cluster) | ✓ | ✓ | ✗ | ✗ | ✗ | ✗ | ✓ | ✓ |
| AWS WAF (Amazon-com / variable) | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| DataDome silent (etsy/tripadvisor) | ✓ | ✓ | ✗ | ~ | ~ | ✗ | ✗ | ✗ |
| DataDome (wsj) | ✓ | ✓ | ✓ | ✓ | ✓ | ✗ | ✗ | ✗ |
| DataDome interactive (yelp) | ~ | ✗ | ✗ | ✗ | ✓ | ✗ | ✗ | ✗ |
| Akamai sec-cpt (homedepot) | ✓ | ✓ | ✗ | ✗ | ✗ | ✗ | ✓ | ✓ |
| Akamai BMP (adidas) | ✓ | ✓ | ✗ | ✗ | ✗ | ✓ | ✓ | ✓ |
| Kasada (canadagoose/hyatt/realtor) | ✓ | ~ | ✗ | ✗ | ✗ | ✗ | ~ | ~ |
| Cloudflare Managed (general) | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✗ | ✗ |
| Cloudflare Managed (iphone-targeted 7) | ✓ | ✓ | ✓ | ✓ | **✗** | ✓ | ✗ | ✗ |
| PerimeterX zillow | ✓ | ✗ | ✓ | ✓ | ✓ | ✗ | ✗ | ✗ |
| PerimeterX wayfair (Camoufox passes; PW fails) | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✗ | ✗ |
| Akamai Edge (walmart) | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Imperva (not in corpus) | ✓ | ✓ | ? | ? | ? | ? | ✗? | ✗? |

Key cells worth dwelling on (the architecture insights):

- **BO firefox is *uniquely* trusted by Akamai adidas** (the lone ✓ in that row). Cause analysis in `26 §4.1`.
- **BO iphone is *uniquely* filtered by Cloudflare on 6/7 sites** (the lone ✗ in that row). Cause analysis in `11 §5.3` — iOS Safari JA4 is a small-share class CF undertrusts.
- **PW Chromium and Patchright are universally filtered by CF/DataDome/PerimeterX** despite being real-Chrome under the hood — the CDP detection signal is dispositive for these vendors.
- **PW Chromium and Patchright are universally trusted by AWS WAF + Akamai BMP** despite being CDP-driven — real-Chrome trust beats CDP signal for these vendors.

The split is sharp and consistent: vendors that prioritize CDP-detection over fingerprint-class (Cloudflare, DataDome, PerimeterX) are weak on PW family + strong on BO family. Vendors that prioritize fingerprint-class over CDP-detection (AWS WAF, Akamai) are strong on PW family + weak on BO until vendor_solvers re-add.

---

## 9. Customer pitch lines (the "why BO" matrix)

By customer use case + dominant vendor mix:

### 9.1 Amazon scraping (8-product family, AWS WAF heavy)

**Today:** Use Camoufox (5/8) or Playwright family (7/8). BO routed gets 4/8 — adequate for low-scale, insufficient if Amazon is the primary target.

**Post-chapter-06:** BO routed projected 7-8/8 once the `getToken()` solver lands in `vendor_solvers`. At that point BO becomes competitive at amazon-scale because of its 14 pages/min throughput vs Camoufox's 8.4 (per `12 §4.2` cost-per-page table).

**Cost angle:** at 1M pages/month scale, BO routed at $0.74/1M pages vs Playwright $13.23/1M pages (per `12 §4.2`).

### 9.2 News / social / SaaS (Cloudflare heavy)

**Today:** Use BO. Cloudflare blocks PW family at 7/7 of the Managed Challenge cluster (economist, ft, ecosia, openai, quora, udemy, weather). Camoufox + BO both pass; BO is ~12× cheaper RSS.

**Concrete:** for a CF-fronted news aggregator scraping 100K pages/day, BO chrome (419 MB RSS, 14 pages/min) is the obvious pick.

**Routing tip:** skip the iphone profile for CF-protected sites (per §7.3).

### 9.3 E-commerce (Akamai / DataDome heavy)

**Today:** Mixed. Akamai BMP sites with passive scoring (walmart, leboncoin) pass everywhere. The hard ones (homedepot, adidas, bestbuy) need:
- Camoufox for adidas-class (it doesn't pass adidas in our sweep, but works for several other Akamai sites)
- Playwright family for homedepot (only engines that pass it strict)
- BO firefox for adidas (the only engine that passes it strict)

**Post-chapter-07 + chapter-26:** BO routed adds etsy + tripadvisor + (likely) yelp on iphone. Post-vendor_solvers re-add: BO matches Camoufox on the full DataDome cluster.

**Routing tip:** firefox first for Akamai (`adidas` lever); iphone first for DataDome (`yelp` lever) per §7.4 + §7.5.

### 9.4 Fintech (likely Imperva + Akamai BMP, not measured)

**Today:** unmeasured cluster. Imperva + Akamai BMP is documented in `18 §2.3, 2.7` but not in our 126-corpus.

**Recommendation:** capture a representative fintech site per `18 §3 onboarding playbook`, run BO + Camoufox + Patchright sweep, decide based on Imperva-tier results. If primarily Imperva (visid_incap_* / reese84 cookies): BO is likely competitive because Imperva weights TLS-class similarly to PerimeterX (where BO wins).

### 9.5 Real estate (Kasada heavy + zillow PerimeterX)

**Today:** Mixed.

- **zillow (PerimeterX)** — BO routed wins; Camoufox + PW family all fail. **Pick BO.**
- **realtor, canadagoose, hyatt (Kasada)** — Everyone fails. Hyatt has a slim Patchright/PW loose-L3 pass (`L3 13228` just below 15 KB strict gate). **Pick Patchright if you only need loose-L3 hyatt; otherwise the bar is the chapter 08 frontier.**

For a real-estate aggregator hitting zillow + realtor + others: BO routed is the best mix today (zillow wins, realtor parity-with-everyone).

### 9.6 General-purpose / mixed corpus

**Today:**
- Highest pass rate single engine: Camoufox (113/126)
- Highest pass rate with routing: BO routed (108/126); chapter 05 + 06 + 07 + 26 path to 113+
- Lowest memory per worker: BO pixel (388 MB) at 102/126 single-profile
- Fastest throughput: Patchright (13.6 pages/min) but only 88 strict Pass
- Lowest cold-start latency: BO (in-process, no subprocess)
- Largest API surface: Playwright (Page Object Model, codegen, trace viewer)

**Recommendation by customer shape** (mirror of `12 §4.1`):

- **High-volume / cost-sensitive (Lambda / Cloud Run):** BO (12× lower RSS than PW; close-second to Camoufox at the wall; routes flexibly)
- **Max pass rate today:** Camoufox
- **Existing Playwright codebase, want quick win:** Patchright (drop-in)
- **Internal testing only:** Playwright (full API)
- **Per-vendor optimization (4 different profile rotations for 4 different domain classes):** BO (only engine that ships per-profile routing as a feature)

---

## 10. Per-vendor maturity / staleness rating (2026-05-24)

A snapshot of where each vendor sits on the public-research-vs-vendor-rotation curve. Useful for triaging which vendor work is leverageable today vs which has gone stale.

| Vendor | Public-solver maturity | Vendor rotation cadence | Last public protocol change | Right action for BO |
|---|---|---|---|---|
| AWS WAF | High — 6+ open repos (xKiian, neiii, jonathanyly, Switch3301, aferapi) all maintained 2026 | ~monthly (challenge.js rebuilds) | ~2026-Q1 (gokuProps shape stabilised; HashcashScrypt / NetworkBandwidth fork) | Chapter 06 vendor solver: viable now; rebuild quarterly |
| DataDome | High — glizzy + Hyper-Solutions cover both interstitial + interactive | ~daily (WASM blob keys rotate) | structurally stable since 2024 (rt:'i'/rt:'c' split) | Chapter 07 §P1+P2+P3 primitives are stable across rotations; vendor encoder needs daily re-key |
| Akamai BMP | Medium — v2 well-covered (xiaoweigege, glizzy) but v3 obfuscation rotates often | ~weekly (per-tenant) | 2024 (v3 JSON elementSwapping) | Chapter 26 vendor encoder restoration uses the v3 reference (pre-strip `c53fa56` byte-perfect parity); private side absorbs the weekly drift |
| Akamai sec-cpt | High — algorithm is stable + bundle self-solves in-V8 | structurally stable | 2023 (PoW reduction formula) | Public engine §3.B `started_as_seccpt_challenge` already does the heavy lifting |
| Kasada | Low — public research is sparse; open-source SOTA frontier | ~quarterly (full bytecode + opcode table) | 2025-Q4 (XOR wrapper key change observed) | Chapter 08 levers; out of v0.1.0 |
| Cloudflare Turnstile / Managed | Medium — multiple bypasses (vvanglro, x404xx) but CF rolls fast | ~weekly | 2025-Q4 (Managed Challenge risk-model overhaul) | Engine wins most via real-Chrome TLS; per-profile routing per §7.3 handles the rest |
| PerimeterX / HUMAN | Medium — MiddleSchoolStudent solver maintained, but PX rotates weekly | ~weekly | 2025 (sensor obfuscation rebuild) | BO wins zillow today; chapter not needed for v0.1.0 |
| Imperva / Reblaze / Thales | Medium — 2captcha + scrapebadger guides; reese84 algorithm understood | ~monthly | structurally stable since 2024 | Not in 126-corpus; add to corpus via `18 §3` onboarding if customer demand |
| Sucuri | Low — rarely studied | rarely changed | stable since 2022 | Engine handles; no action |
| hCaptcha / reCAPTCHA / FunCaptcha (interactive) | High (commercial) | continuous | continuous | Out of scope (needs CAPTCHA-solving-service integration) |
| Friendly Captcha | High (PoW is open) | stable | stable | Trivially solvable; out of scope |
| Forter (fraud) | N/A — passive | rare | stable | Doesn't block navigation |
| wbaas (Walmart) | Low — no public solver | unknown | unknown | Engine handles passive scoring; deeper protection unknown |

**The actionable read:** the post-v0.1.0 vendor work prioritized by ROI:

1. **Chapter 06 (AWS WAF)** — highest ROI: cluster of 5 sites; mature public research; vendor encoder fits in vendor_solvers.
2. **Chapter 07 (DataDome primitives)** — highest engine-side value: 3 primitives flip 2-3 sites without vendor encoders.
3. **Chapter 26 (Akamai)** — moderate: 1 site (homedepot) hinges on chapter 07 §P1 landing first.
4. **Chapter 08 (Kasada)** — research phase; not v0.1.0 scope.

The customer-pitch differentiation that holds even at v0.2.0+:

- **CF + DataDome cluster**: BO + Camoufox lead; PW family is structurally weak.
- **AWS WAF + Akamai cluster**: PW family leads; BO + Camoufox catch up via vendor_solvers re-add.
- **PerimeterX zillow**: BO leads (uncontested in our sweep).
- **Kasada**: nobody leads.

---

## 11. Vendor coverage drift signal

Per `18 §5.3` (the 3-light drift cadence) — extend it with per-vendor cluster monitoring. The nightly sweep buckets are below; alert thresholds in `14_TESTING_VALIDATION.md §L5`.

| Bucket | Sites | Trailing-7-night-median pass | Drift signal threshold (Yellow) | Vendor likely cause |
|---|---|--:|--:|---|
| AWS WAF (amazon variable) | 4 | 3 | drop to ≤ 1 in 2/3 consecutive nights | AWS WAF challenge.js rotation |
| AWS WAF (amazon hard) | 5 | 0 | flips to ≥ 1 (improvement!) | engine improvement |
| DataDome (silent + wsj) | 4 | 3 | drop to ≤ 1 | DataDome interstitial daily key rotation |
| Akamai sec-cpt | 1 | 0 (HEAD) | flips to ≥ 1 (improvement after chapter 26) | chapter 26 §3 lands |
| Cloudflare iphone-class | 7 | 7 (with routing) | drop to ≤ 5 | CF Managed Challenge risk-model change |
| PerimeterX zillow | 1 | 1 | drop to 0 | PX rotation OR our TLS regression |
| Kasada | 3 | 0 | flips to ≥ 1 (improvement) | huge engine improvement OR Kasada deprecates |

**The two-direction signal**: drift down = vendor rotated against us; drift up = vendor relaxed OR our engine improved. The 3-night-consecutive rule per `18 §5.3` filters single-night noise.

When per-vendor drift down hits Yellow, follow the per-vendor capture cycle from `18 §5.2`:
- AWS WAF Yellow → re-capture amazon-de challenge.js + diff
- DataDome Yellow → re-pull dd-script.js (daily key only — wrapper stable)
- Akamai Yellow → re-capture sensor_data POST body
- CF Yellow → re-pull the orchestrator at `/cdn-cgi/challenge-platform/h/b/jsd/...`
- PX Yellow → re-run MiddleSchoolStudent's deobfuscator on the sensor JS
- Kasada Yellow → re-run K2-DIFF per `08_KASADA_FRONTIER.md §Lever 1`

When per-vendor drift UP hits (a cluster gained sites), it's almost always:
- An engine fix landed (cross-reference with git log of last 7 nights)
- The vendor relaxed risk model (rare; usually correlates with their public blog)
- Routing changed (cross-reference with `crates/browser/src/router.rs` history)

---

## 12. Files referenced

### Sweep data (the entire measurement basis)

- `/tmp/full_sweep_2026_05_24/bo_chrome_148_macos_cold.json` — `summary.pass=99`, raw per-site results
- `/tmp/full_sweep_2026_05_24/bo_pixel_9_pro_chrome_148_cold.json` — `summary.pass=102` (best single)
- `/tmp/full_sweep_2026_05_24/bo_iphone_15_pro_safari_18_cold.json` — `summary.pass=98`
- `/tmp/full_sweep_2026_05_24/bo_firefox_135_macos_cold.json` — `summary.pass=101`; adidas unique L3 1.3 MB
- `/tmp/full_sweep_2026_05_24/comp_camoufox.json` — `summary.pass=113` (open-source SOTA)
- `/tmp/full_sweep_2026_05_24/comp_patchright.json` — `summary.pass=88`, throughput 13.55 pages/min
- `/tmp/full_sweep_2026_05_24/comp_playwright.json` — `summary.pass=88`, RSS 5618 MB
- `/tmp/full_sweep_2026_05_24/comp_playwright_stealth.json` — `summary.pass=87`, RSS 5011 MB
- `/tmp/full_sweep_2026_05_24/run.log` — sweep harness log
- `/tmp/full_sweep_2026_05_24/{bo,comp}_*.log` — per-engine per-profile capture logs

### Sweep harness / corpus

- `crates/browser/examples/sweep_metrics.rs` — BO sweep harness
- `crates/browser/tests/holistic_sweep.rs:1-700` — 126-site corpus definition
- `benchmarks/run_full_sweep.sh` — 4-profile + 4-competitor driver
- `benchmarks/bench_corpus_v2.py` — Python competitor wrapper
- `benchmarks/build_report.py` — report builder

### Engine code (vendor matrix justifications)

- `crates/browser/src/classify.rs:81-156` — UNAMBIGUOUS, PHRASE, SMALL_BODY, AKAMAI_CHALLENGE_COSIGNAL, INTERACTIVE_CAPTCHA_COSIGNAL marker tables
- `crates/browser/src/classify.rs:170-228` — `verdict_for` / `engine_classify` (the canonical classifier)
- `crates/browser/src/classify.rs:247-251` — `is_cf_challenge_doc` (persistent CF origin-flag)
- `crates/browser/src/page.rs:1045-1069` — initial challenge response header logger (`x-amzn-waf-action`, `x-datadome`, `x-wbaas-token`)
- `crates/browser/src/page.rs:1186-1196` — `__akamai_events` JS collector (kept post-strip)
- `crates/browser/src/page.rs:1638-1663` — `started_as_dd_challenge` / `started_as_seccpt_challenge` / `started_as_cf_challenge` (the three persistent origin flags)
- `crates/browser/src/page.rs:2283-2293` — `v8_html_is_real` (vendor body markers preventing the V8 refetch from accepting a re-served challenge as real)
- `crates/browser/src/challenge.rs:55-161` — `ChallengeSolver` trait + `ChallengeKind` + `SolveOutcome` (the seam private vendor_solvers binds to)
- `crates/net/src/tls.rs:22-57` — `TLS_CHROME_MAJOR` / `UA_CHROME_MAJOR` constants (the TLS-class-trust analysis basis)
- `crates/net/src/tls.rs:60-220` — Chrome cipher list, sigalgs, curves, extension permutation
- `crates/net/src/tls.rs:107-183` — iOS Safari TLS constants (the iphone-unique CF-filtering hypothesis basis)
- `crates/stealth/src/presets.rs:413-495` — `firefox_135_macos()` preset (the adidas firefox-only-win basis)
- `crates/stealth/src/presets.rs:120-196` — `chrome_148_macos()` preset

### Sibling docs in this release plan

- `docs/releases/v0.1.0-parity/01_CURRENT_STATE.md` — headline numbers
- `docs/releases/v0.1.0-parity/02_GAP_ANALYSIS.md` — the 10 Camoufox-only sites + 8 hard-residual sites
- `docs/releases/v0.1.0-parity/03_BENCHMARK_METHODOLOGY.md` — strict-Pass classifier definition
- `docs/releases/v0.1.0-parity/04_TOOLING_SPEC.md` — `--capture` mode for per-vendor diagnostics
- `docs/releases/v0.1.0-parity/05_SPA_HYDRATION_CLUSTER.md` — reddit / duolingo / booking / douyin
- `docs/releases/v0.1.0-parity/06_AWS_WAF_SOLVER.md` — chapter 06 for AWS WAF deep dive
- `docs/releases/v0.1.0-parity/07_DATADOME_PRIMITIVES.md` — chapter 07 for DataDome restoration plan
- `docs/releases/v0.1.0-parity/08_KASADA_FRONTIER.md` — chapter 08 for Kasada research arc
- `docs/releases/v0.1.0-parity/09_MEMORY_OPTIMIZATION.md` — Camoufox memory measurement post-mortem
- `docs/releases/v0.1.0-parity/10_TIMING_OPTIMIZATION.md` — throughput analysis
- `docs/releases/v0.1.0-parity/11_PER_PROFILE_STRATEGY.md` — per-profile internals + routing decision tree (the §7 routing tables in this doc cross-link)
- `docs/releases/v0.1.0-parity/12_COMPETITIVE_LANDSCAPE.md` — the engine-by-engine comparison this chapter pairs with
- `docs/releases/v0.1.0-parity/13_FILE_LOCATIONS_INDEX.md` — file:line lookup
- `docs/releases/v0.1.0-parity/14_TESTING_VALIDATION.md` — drift detection
- `docs/releases/v0.1.0-parity/15_OPEN_QUESTIONS.md` — research backlog
- `docs/releases/v0.1.0-parity/16_STEALTH_FINGERPRINT_AUDIT.md` — per-API audit
- `docs/releases/v0.1.0-parity/17_WEB_API_PARITY_MATRIX.md` — Web API coverage
- `docs/releases/v0.1.0-parity/18_ANTI_BOT_VENDOR_COOKBOOK.md` — the reference encyclopedia (this chapter's vendor totals derive from §1-§2 there)
- `docs/releases/v0.1.0-parity/19_PROFILE_EXPANSION_PLAN.md` — additional profile candidates
- `docs/releases/v0.1.0-parity/20_MEMORY_BUDGET.md` — RSS budgeting
- `docs/releases/v0.1.0-parity/21_V8_SNAPSHOT_PARALLEL_COLD.md` — snapshot/parallel
- `docs/releases/v0.1.0-parity/22_PRODUCTION_DEPLOYMENT.md` — deployment shapes
- `docs/releases/v0.1.0-parity/23_TLS_HTTP_FINGERPRINT_REFERENCE.md` — TLS deep dive
- `docs/releases/v0.1.0-parity/24_RISK_REGISTER.md` — known unknowns
- `docs/releases/v0.1.0-parity/26_AKAMAI_BMP_DEEP.md` — Akamai BMP restoration plan (sibling deep dive)

### Adjacent BO docs

- `docs/BENCHMARK_2026_05_24.md` — narrative sweep report
- `docs/PERFORMANCE_2026_05_24.md` — per-page perf
- `docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md` — ±5-site WAF variance characterization

### External references (URLs cited)

- Camoufox: https://github.com/daijro/camoufox
- Patchright: https://github.com/Kaliiiiiiiiii-Vinyzu/patchright
- playwright-stealth: https://github.com/AtuboDad/playwright_stealth
- Playwright: https://github.com/microsoft/playwright
- AWS WAF Challenge API spec: https://docs.aws.amazon.com/waf/latest/developerguide/waf-js-challenge-api.html
- glizzykingdreko Akamai v3 walkthrough: https://medium.com/@glizzykingdreko/akamai-v3-sensor-data-deep-dive-into-encryption-decryption-and-bypass-tools-da0adad2a784
- glizzykingdreko DataDome walkthrough: https://medium.com/@glizzykingdreko/breaking-down-datadome-captcha-waf-d7b68cef3e21
- Hyper-Solutions docs: https://docs.hypersolutions.co
- Scrapfly vendor guides: https://scrapfly.io/bypass/akamai · https://scrapfly.io/bypass/perimeterx · https://scrapfly.io/blog/posts/how-to-bypass-imperva-incapsula-anti-scraping · https://scrapfly.io/blog/posts/how-to-bypass-cloudflare-turnstile
- ZenRows vendor guides: https://www.zenrows.com/blog/bypass-akamai · https://www.zenrows.com/blog/perimeterx-bypass · https://www.zenrows.com/blog/bypass-cloudflare · https://www.zenrows.com/blog/incapsula-bypass
- CapSolver guides: https://www.capsolver.com/blog/aws-waf/top-aws-solver-ranking · https://www.capsolver.com/blog/Cloudflare/solve-cloudflare-in-2026
- RoundProxies guides: https://roundproxies.com/blog/bypass-aws-waf/ · https://roundproxies.com/blog/bypass-perimeterx/ · https://roundproxies.com/blog/bypass-imperva-incapsula/
- lexiforest/curl-impersonate: https://github.com/lexiforest/curl-impersonate
- creepjs: https://github.com/abrahamjuliot/creepjs
- fingerprintjs: https://github.com/fingerprintjs/fingerprintjs

### Memory (auto-context)

- `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/MEMORY.md`
- `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/state_2026_05_16_phase5_datadome.md` (Akamai BMP/sec-cpt; DataDome WASM-iframe research)
- `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/tier1_priority_for_akamai.md` (adidas sensor VM instrumentation)
- `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/kasada_akamai_real_blocker_2026_04_17.md` (the fingerprint-not-IP correction)
- `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/kasada_wrapper_cracked_and_remaining_leaks.md` (the `/tl` XOR wrapper)
- `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/kasada_real_blocker_css_calc_math.md` (CSS Values 4 math probe)
- `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/measurement_holistic_chl_fp_trap.md` (size-gate ≥30 KB rule)
- `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/proxy_not_the_problem.md` (IP-class vindication)

### Workspace constraints

- `CLAUDE.md` — vendor scope rules; the boundary §9.3 / §9.4 plans respect
- `SCOPE.md` — what's in scope for this project
- `aecdf19` — the G6 strip commit that put vendor solvers in private `vendor_solvers` per the design `26 §1` and §3 of this doc both depend on
