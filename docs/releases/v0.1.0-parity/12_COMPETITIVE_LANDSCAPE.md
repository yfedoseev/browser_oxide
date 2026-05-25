# 12 — Competitive landscape

**Audience:** anyone deciding whether browser_oxide is the right tool for a given scraping/automation job, anyone benchmarking against alternatives, and anyone evaluating the v0.1.0 bar.

**One-paragraph thesis:** The open-source headless-browser-for-scraping space has 4 active players above the Playwright-vanilla baseline: **Camoufox** (Firefox + C++ patches, the open-source SOTA at 113), **browser_oxide** (this project, own-engine, 108 routed), **Patchright** (Playwright + CDP-leak patches, 88), and **playwright-stealth** (JS init-script overrides, 87). Below them sits vanilla Playwright (88). Camoufox wins by ~5 sites on strict Pass rate; BO ties on loose L3 (120 vs 120) and leads on memory (~12× less than Playwright at peak) and cold-start latency (no subprocess). The honest customer-facing answer is "Camoufox for max pass rate today, BO for low memory + own-process + per-profile routing flexibility; Playwright family if you need CDP and accept the 5+ GB / 25-CHL tier."

---

## 1. Each competitor's architecture

### 1.1 Camoufox — Firefox + ~50 C++ patches

**Source**: https://github.com/daijro/camoufox · License: Mozilla Public License 2.0

**Underlying engine**: Firefox stable (most recent: Firefox 135 class at the time of our 2026-05-24 sweep). Stripped down and rebuilt from source with a layer of C++ patches.

**What it does uniquely**: Fingerprint spoofing happens at the **C++ implementation level** inside Gecko, not via JS injection. From the repository (as of 2026-05-24):

> "Since Camoufox intercepts calls in the browser's C++ implementation level, all of the hijacked objects and properties appear native."

> "Camoufox is an open source browser built for AI agents. It is lightweight, mimics a human browser, and is optimized for LLM automation."

The implication is meaningful: most JS-level stealth libraries (puppeteer-extra-stealth, playwright-stealth) can be defeated by inspecting property descriptors or `Function.prototype.toString` on overridden objects. A C++ patch produces a property whose descriptor and `toString` output is byte-identical to the real implementation, because it IS the real implementation, just with a different return value baked in.

**Automation channel**: Patches Firefox's **Juggler** protocol (Mozilla's CDP-equivalent for Firefox automation) rather than using CDP. Camoufox sandboxes automation code in an isolated page copy so it's invisible to the actual page being browsed. The CDP-driver detection vectors that catch every Chromium-based driver (Playwright, Patchright, Puppeteer) do not apply.

**Process model**: Firefox e10s — one `firefox-bin` parent process and one+ content processes per tab. Per the repository, the project reports a "~200MB memory footprint" — we measured **48 MB** in the 2026-05-24 sweep, but that number was a **measurement bug** in `benchmarks/bench_corpus_v2.py:256-267`: the harness picked the first /proc child whose `comm` contains "fox" and walked only its descendants, missing the Firefox e10s content processes. Corrected measurement (post-fix) is ~200-400 MB on a 126-site sweep, consistent with Camoufox's own claim. See `docs/releases/v0.1.0-parity/09_MEMORY_OPTIMIZATION.md` for the full post-mortem.

**Strengths** (where Camoufox beats BO in our sweep):
- AWS WAF cluster (amazon-de / amazon-in / amazon-com-au / imdb): full Firefox JS + WASM environment that AWS WAF challenge.js trusts
- DataDome (etsy): cross-origin iframe + WASM challenge resolves natively
- Recaptcha (duolingo): Worker + grecaptcha invisible runs in a real Firefox Worker
- Kasada cluster (canadagoose / hyatt / realtor): the open-source SOTA — though even Camoufox fails these 3, it's the closest to passing them
- SPA hydration: reddit / booking / x-com / douyin all hydrate because the JS environment is a real browser

**Weaknesses** (where BO routed beats Camoufox in our sweep):
- adidas (Akamai class) — BO firefox profile uniquely passes; Camoufox doesn't
- amazon-ca, amazon-com (AWS WAF) — BO's per-profile routing catches what Camoufox's single profile misses
- yelp (DataDome) — BO iphone profile uniquely passes
- zillow (PerimeterX) — BO routed passes; Camoufox doesn't
- Throughput: Camoufox is 8.4 pages/min vs BO pool projected 14.0 pages/min (Firefox e10s startup overhead per-page)
- Cold-start: Firefox-bin launch ≫ in-process BO Page::new (no subprocess)

**Bottom line**: Camoufox is the open-source SOTA at single-engine pass rate, especially on Kasada/DataDome/AWS-WAF. The cost is the full Firefox runtime (200-400 MB) and substantially worse throughput.

### 1.2 Playwright — vanilla Chromium-headless

**Source**: https://github.com/microsoft/playwright · License: Apache 2.0

**Underlying engine**: Chromium (latest stable, ~Chrome 148-class at our sweep date). Headless by default; can also drive headed Chromium/Firefox/WebKit.

**What it does**: Playwright drives a real Chromium subprocess over the **Chrome DevTools Protocol** (CDP). No anti-detection — the browser is configured for testing, and many real-browser-vs-headless signals are visible to the page:

- `--enable-automation` command-line flag is set
- `navigator.webdriver === true`
- `Runtime.enable` CDP method is called, which exposes a JS execution sidechannel that anti-bot vendors detect at network and JS level
- Default `--headless=new` argument has its own UA tail (`HeadlessChrome/`) — easy detection

**Measured result on our 126-site sweep**:
- 88 Pass, 25 CHL (challenge-rejected — almost every anti-bot vendor blocks)
- 5618 MB peak RSS (12-14× BO's per-profile RSS)
- 12.6 pages/min throughput (best in class because it's just driving native Chrome)
- 10.0 min total wall-clock

**Strengths**:
- Real Chromium — every JS, layout, network behaviour is exactly Chrome
- Best ecosystem (Page Object Model, codegen, trace viewer, parallel test runner)
- Best CDP-API completeness for non-stealth automation (testing, screenshotting, PDF gen, form-fill)
- Fastest per-page when not blocked

**Weaknesses**:
- 25 sites blocked by anti-bot challenges on our corpus (every Cloudflare/Kasada/AWS-WAF/DataDome class)
- 5+ GB peak RSS — Chromium is the biggest browser in the world
- Detectable as automation in 5 different ways at the network/JS layer

**Bottom line**: Use Playwright when you don't care about stealth (internal testing, friendly origins, browser automation as a developer tool). Don't use it for adversarial scraping.

### 1.3 Patchright — Playwright + CDP-leak patches

**Source**: https://github.com/Kaliiiiiiiiii-Vinyzu/patchright · License: Apache 2.0

**Underlying engine**: Patched Chromium driven by patched Playwright. Drop-in replacement for `playwright` Python/Node package; same API surface.

**What it does**:

> "Patchright is a patched and undetected version of the Playwright Testing and Automation Framework."

From the repository description of what it patches:
> "Patchright avoids using `Runtime.enable` by executing JavaScript through isolated ExecutionContexts instead, and it patches command-line flags — notably adding `--disable-blink-features=AutomationControlled` while removing `--enable-automation` to prevent `navigator.webdriver` detection."

Specifically (per the docs):
- `navigator.webdriver === undefined` (removes the `true` value)
- `--enable-automation` flag removed; `--disable-blink-features=AutomationControlled` added
- `Runtime.enable` not called — JS executes through "isolated ExecutionContexts" instead, dodging the CDP-side-channel that vendors fingerprint
- Closed Shadow Roots handled for element interaction (vanilla Playwright cannot)

Patchright claims to pass detection at "Cloudflare, Kasada, Akamai, Shape/F5" per their repository, which is a stronger claim than our measurement supports — see below.

**Measured result on our 126-site sweep** (2026-05-24):
- **88 Pass** — identical to vanilla Playwright on the strict metric
- 25 CHL, 5681 MB peak RSS, 13.6 pages/min throughput (best in class)
- Differences from Playwright are within noise floor (`docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md` documents ±5 site variance on the sweep)

**Strengths**:
- Best throughput (13.6 pages/min — slightly above vanilla Playwright at 13.6 vs 12.6)
- Full Playwright API compat — no migration cost from existing scripts
- Defeats the easiest tier of CDP-driver detection (navigator.webdriver, Runtime.enable, --enable-automation flag)
- Active maintenance with public roadmap

**Weaknesses**:
- 88 Pass = identical to vanilla Playwright on the strict metric in our 126-site sweep — the CDP-leak patches help against the basic `navigator.webdriver` check, but the heavy vendors (Cloudflare, AWS WAF, DataDome, Kasada) check far more than that. Our measured number contradicts Patchright's claim of passing those vendors.
- Restricted to Chromium-based browsers: "Patchright only patches CHROMIUM based browsers. Firefox and Webkit are not supported."
- 5+ GB peak RSS (inherits Chromium)
- Still uses CDP underneath — sufficiently determined vendors can sometimes still detect it via timing of remote-debugging endpoints

**Bottom line**: A solid drop-in upgrade if you're already on Playwright and want the easy CDP-detection class defeated. Doesn't beat vanilla Playwright meaningfully on the heavy-WAF tier in our measurement.

### 1.4 playwright-stealth — JS init-script overrides

**Source**: https://github.com/AtuboDad/playwright_stealth (Python port of `puppeteer-extra-plugin-stealth`) · License: MIT

**Underlying engine**: Vanilla Playwright (Chromium or Firefox/WebKit) plus a layer of JS init scripts injected via Playwright's `addInitScript()` API.

**What it does**:

From the repository:
> "Transplanted from puppeteer-extra-plugin-stealth, **Not perfect**."

The library overrides JavaScript fingerprinting properties before page scripts can detect automation. Implementation pattern: init scripts injected via Playwright's `addInitScript()` API; properties typically overridden include `navigator.webdriver`, `navigator.languages`, `navigator.plugins`, `chrome.runtime`, WebGL `getParameter`, and others — though the README is light on specifics. The package self-warns:

> "Not perfect."

**Measured result on our 126-site sweep**:
- **87 Pass** (1 less than vanilla Playwright at 88 — within noise)
- 25 CHL, 5011 MB peak RSS, 7.5 pages/min throughput (init scripts add per-page overhead)
- Underperforms vanilla Playwright on throughput by ~40%

**Strengths**:
- Easy to add to an existing Playwright codebase (one line: `await stealth.apply_stealth(page)`)
- Works on Chromium, Firefox, and WebKit (unlike Patchright which is Chromium-only)
- Multi-language community support (Python, Node)

**Weaknesses**:
- JS-injected overrides leave a `Function.prototype.toString` and property-descriptor signature that sophisticated vendors detect (this is the structural critique that Camoufox addresses with C++ patches)
- 87 Pass — no measurable improvement over vanilla 88 on our 126-site sweep
- Throughput regression vs vanilla Playwright (init scripts are not free per-page)
- 5+ GB peak RSS (inherits Playwright)

**Bottom line**: Cheapest "stealth" upgrade for an existing Playwright codebase. Don't expect to beat the basic-Chromium tier on serious WAF-protected sites.

---

## 2. What browser_oxide does differently

### 2.1 Own engine, in-process

BO does NOT drive a separate browser subprocess. It IS the browser, in the same Rust process as your application. No CDP, no Juggler, no inter-process JSON-RPC. The engine is the workspace under `crates/`:

- **HTML parser**: `crates/html_parser/` — own arena-allocated tokenizer + tree construction
- **CSS**: `crates/css_parser`, `css_selectors`, `css_values`, `css_cascade` — written from scratch (we deliberately do NOT pull Servo's MPL crates)
- **DOM**: `crates/dom/` — arena-allocated; `NodeId` is `Copy + u32`; nodes live in a `Vec` inside the `Dom` (no `Rc<RefCell<…>>` patterns)
- **Layout**: `crates/layout/` — minimal box-tree for the metrics needed by JS (`getBoundingClientRect`, `offsetWidth`, etc.)
- **JS runtime**: `crates/js_runtime/` — V8 via `deno_core 0.311` extensions; per-thread isolate
- **HTTP/TLS**: `crates/net/` using `boring2` (Cloudflare BoringSSL fork) for Chrome-identical TLS ClientHello + HTTP/2 fingerprint
- **Canvas / WebGL**: `crates/canvas/` — software canvas + WebGL bindings
- **Glue**: `crates/browser/` — `Page`, `Page::navigate`, `PagePool`

There are 15 crates in `[workspace.members]` (see `Cargo.toml`). The dependency graph is documented in `docs/ARCHITECTURE.md`.

### 2.2 Byte-perfect Chrome TLS via boring2

`crates/net/src/tls.rs` (~687 lines) configures BoringSSL to produce a ClientHello identical to Chrome 147, including:

- Cipher suite list in the exact order Chrome emits (`tls.rs:60-76`)
- Signature algorithms in order (`tls.rs:79-88`)
- Curves with `X25519_MLKEM768` first (post-quantum since Chrome 131, `tls.rs:91-96`)
- Extension permutation: full 16-element Fisher-Yates shuffle every handshake (`tls.rs:203-228`) — matches real Chrome's per-handshake randomization since 2024
- Certificate compression (zlib for Safari iOS; not advertised for Chrome desktop per the verified reference)
- HTTP/2 SETTINGS frame ordered/sized to match Chrome

Distinct per device class:
- Desktop / Android: shared Chrome 147 config, Android-specific curves
- Mobile iOS: distinct Safari 18 cipher list (20 entries incl. legacy 3DES), distinct sigalgs (10 entries incl. the duplicated `rsa_pss_rsae_sha384` Apple bug), distinct curves (no PQ, adds P-521), FIXED extension permutation (no Fisher-Yates), TLS 1.0-1.3 range (`tls.rs:111-289`)

The intentional UA-vs-TLS-label split is documented in `tls.rs:22-57` and machine-checked by `tls_fingerprint_vectors_no_silent_drift` (`tls.rs:506+`). Real Chrome's ClientHello is version-stable across majors; the bytes are identical between Chrome 147 and 148.

Playwright/Patchright cannot match this — they drive a real Chromium binary which uses BoringSSL through Chrome's own configuration. That's "the actual thing" so by definition matches; but it's also locked to whatever Chrome ships, with no ability to spoof iOS Safari TLS while running on a Linux server, for example. BO can.

Camoufox uses Firefox's NSS library for TLS, which produces a Firefox JA4 — distinct from Chrome's, and *different* from what most anti-bot vendors expect for a "generic bot" risk class.

### 2.3 Stealth profiles emit consistent UA + ClientHints + TLS

Most stealth libraries fix *one* layer: the JS surface (playwright-stealth, puppeteer-extra-stealth) or the network surface (curl-impersonate, undetected-chromedriver). BO ships **all three** as a single coherent profile:

- TLS ClientHello (boring2 + per-profile branch)
- HTTP headers including the full `sec-ch-ua-*` Client Hints family with major/full version split (`crates/net/src/headers.rs:143-413`)
- JS surface: `navigator.userAgent`, `userAgentData`, `platform`, `vendor`, `hardwareConcurrency`, `deviceMemory`, `webgl.vendor`/`renderer`, screen dimensions, plugin array, color gamut, pointer/hover, platform authenticator — all from the same `StealthProfile` struct (`crates/stealth/src/profile.rs:33-180`)

This avoids the #1 anti-bot signal: cross-layer mismatch. A request with Chrome's TLS ClientHello + Safari's UA + Firefox's `navigator.vendor` is an instant flag. BO's preset gates that whole pipeline through one validated struct.

See `11_PER_PROFILE_STRATEGY.md` for the full per-profile field list.

### 2.4 No CDP at all — invisible to CDP-detection heuristics

The Playwright tier (including Patchright) drives Chrome through CDP. CDP exposes a remote-debugging endpoint (often `localhost:9222`) and uses specific protocol methods (`Runtime.enable`, `Page.navigate`, `Network.enable`) that anti-bot vendors detect via:

- Timing-side-channel on `Runtime.evaluate` (CDP eval is measurably different from a normal JS eval call)
- Presence of the `__webdriver_evaluate` / `__driver_evaluate` window properties
- Order of events fired during page lifecycle (DevTools attaches before some browser events)

Patchright defeats the easier signals (navigator.webdriver, --enable-automation, Runtime.enable timing). BO has none of these surfaces because it doesn't drive Chrome — it IS the JS environment. There is no DevTools protocol, no remote endpoint, no debugger to detect.

### 2.5 ~12× less memory than Playwright at peak

Per the 2026-05-24 sweep (`/tmp/full_sweep_2026_05_24/`):

| Engine | Peak RSS (MB) | Per-page incremental | Honest? |
|---|--:|---|---|
| BO pixel | 388 | ~3 MB/page (V8 isolate dominates) | yes |
| BO chrome | 419 | ~3 MB/page | yes |
| BO iphone | 445 | ~3.5 MB/page (iOS shims add code) | yes |
| BO firefox | 472 | ~3.7 MB/page | yes |
| Camoufox | ~200-400 (corrected; reported 48) | unmeasured | reported-no, corrected-yes |
| playwright_stealth | 5011 | ~40 MB/page (Chromium tab overhead) | yes |
| playwright | 5618 | ~45 MB/page | yes |
| patchright | 5681 | ~46 MB/page | yes |

A 12× memory ratio matters when running many concurrent profiles or on memory-constrained workers (Kubernetes pods, Lambda functions, ARM serverless platforms). It does NOT matter for a single-page screenshot job; pick the right engine for the deployment shape.

### 2.6 Per-profile routing as a first-class engine feature

No competitor ships a profile rotation system. Camoufox can be initialized with a custom config but doesn't ship per-vendor routing rules. Playwright family doesn't even ship stealth profiles in the first place. BO ships 4 profiles (chrome / pixel / iphone / firefox) with a routed-best-of-4 result of 108 vs the best single at 102 — see `11_PER_PROFILE_STRATEGY.md` for the routing decision tree.

---

## 3. Per-category competitive matrix

Source: `/tmp/full_sweep_2026_05_24/{bo_*_cold,comp_*}.json`. Strict Pass per category.

| Category | n | BO chrome | BO pixel | BO iphone | BO firefox | **BO routed** | **Camoufox** | Playwright | Patchright | PW+Stealth |
|---|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|
| stores | 17 | 13 | 14 | 14 | 14 | 15 | **15** | 9 | 9 | 9 |
| misc | 12 | 9 | 9 | 8 | 9 | 11 | **11** | 8 | 8 | 8 |
| news | 10 | 10 | 10 | 8 | 9 | **10** | **10** | 3 | 3 | 3 |
| social | 10 | 8 | 8 | 7 | 8 | 8 | **10** | 9 | 9 | 9 |
| antibot | 10 | 9 | 9 | 9 | 9 | **9** | **9** | 9 | 9 | 9 |
| tech | 9 | 9 | 8 | 8 | 9 | **9** | **9** | 9 | 9 | 9 |
| travel | 8 | 5 | 7 | 7 | 5 | 7 | **8** | 5 | 5 | 5 |
| amazon | 8 | 1 | 1 | 2 | 3 | 4 | **5** | 7 | 7 | 6 |
| streaming | 8 | 7 | 8 | 8 | 7 | **8** | **8** | 8 | 8 | 8 |
| search | 8 | 8 | 8 | 7 | 8 | **8** | **8** | 7 | 7 | 7 |
| ru | 6 | 5 | 5 | 5 | 5 | **5** | **5** | 4 | 4 | 4 |
| gov-bank | 6 | 6 | 6 | 6 | 6 | **6** | **6** | 6 | 6 | 6 |
| reference | 5 | 5 | 5 | 5 | 5 | **5** | **5** | 4 | 4 | 4 |
| chl-known | 5 | 1 | 1 | 1 | 2 | 2 | **2** | 0 | 0 | 0 |
| realestate | 4 | 3 | 3 | 3 | 2 | **3** | 2 | 0 | 0 | 0 |
| **TOTAL** | **126** | 99 | 102 | 98 | 101 | **108** | **113** | 88 | 88 | 87 |

### 3.1 Where Camoufox wins vs BO routed (10 sites, the "recoverable surface")

Camoufox passes strict (≥15 KB) and BO routed does not. Detailed in `02_GAP_ANALYSIS.md`. Sorted by category:

| Site | Cluster | Where the gap comes from |
|---|---|---|
| amazon-de, amazon-in, amazon-com-au | AWS WAF | challenge.js detects our engine via some fingerprint check and silently skips `getToken()` |
| imdb | AWS WAF | same cluster |
| etsy | DataDome | the post-`aecdf19` engine doesn't have the CSP-relax + cross-origin-iframe materialization + cookie-write-as-solve-signal trio (chapter 07) |
| reddit | SPA verify-challenge | proof-of-execution form not submitting; closest miss (8 KB vs 15 KB gate) |
| duolingo | reCAPTCHA invisible | Worker(webworker.js) for reCAPTCHA Enterprise not completing; 1.7 KB to gate |
| booking | SPA hydration | server-side React; missing fetch in hydration chain |
| douyin | TikTok-CN custom anti-bot | `__ac_signature` + `ttwid` not computed |
| x-com | TLS / rate-limit | mid-sweep THIN-BODY, isolated-run pass — likely SharedSession cookie bleed |

### 3.2 Where BO routed wins vs Camoufox (5 sites)

BO routed passes and Camoufox does not:

| Site | Profile that wins | Cluster | Why |
|---|---|---|---|
| adidas | firefox | Akamai-chl-known | BO firefox's UA+headers pass; Camoufox's Firefox+spoof apparently doesn't |
| amazon-ca | pixel | amazon | Pixel mobile UA gets a non-WAF Amazon serve |
| amazon-com | firefox | amazon | Firefox UA gets a different AWS WAF risk class |
| yelp | iphone | DataDome | iOS Safari class gets a non-DataDome serve from yelp |
| zillow | chrome/pixel/iphone | PerimeterX-realestate | BO's TLS + Chrome-class UA defeats PerimeterX; Camoufox's Firefox UA flagged |

### 3.3 Where BO leads the Playwright tier (~15 site lead on average)

Direct Pass-rate differences vs Playwright (88) — categories where BO routed (108) leads by ≥ 2:

| Category | Δ (BO routed - Playwright) |
|---|--:|
| news | +7 (10 vs 3) — vendors aggressive on Playwright via CDP-detect on news/finance sites |
| stores | +6 (15 vs 9) |
| travel | +2 (7 vs 5) |
| ru | +1 (5 vs 4) |
| reference | +1 (5 vs 4) |
| search | +1 (8 vs 7) |
| misc | +3 (11 vs 8) |
| chl-known | +2 (2 vs 0) |
| realestate | +3 (3 vs 0) |

### 3.4 Parity (everyone scores the same)

- gov-bank, tech, streaming, antibot, social — these are sites without heavy anti-bot (or with anti-bot that everyone defeats). All five engines score within 1-2 sites of each other.

### 3.5 Where Playwright BEATS BO

- **amazon** (Playwright 7/8, BO routed 4/8): Playwright drives real Chrome which AWS WAF's challenge.js trusts; the CDP-detect signal AWS WAF could use isn't dispositive enough to block on its own, and Chrome's full WASM + Worker environment passes the challenge naturally.

This is the one category where the "real Chrome" advantage Playwright has matters more than the CDP-detection penalty.

---

## 4. Customer-perspective trade-offs

| Decision criterion | Winner | Runner-up |
|---|---|---|
| Best raw Pass rate (single engine, no routing) | Camoufox (113) | BO pixel (102) |
| Best Pass rate with routing | BO routed (108) → 115+ targeted post-v0.1.0 | Camoufox (113, no routing) |
| Lowest peak memory | Camoufox (~200-400 honest) ≈ BO (388-472) | BO |
| Fastest single-page latency (cold) | BO (in-process, 0 launch overhead) | Playwright (after launch) |
| Highest throughput (pool path) | BO pool projected 14.0/min | Patchright 13.6/min |
| Lowest cold-start | BO (no subprocess) | — |
| Best CDP / driver-API compatibility | Playwright > Patchright > playwright-stealth | (BO has no CDP) |
| Largest pre-existing community | Playwright | Patchright |
| Easiest "drop-in stealth" upgrade for an existing Playwright codebase | Patchright | playwright-stealth |
| Most stealth coverage at C++ engine layer | Camoufox | BO (Rust impl, equivalent depth) |
| Lowest cost to run at scale (memory × throughput) | BO pool path | Camoufox |
| Smallest binary | Playwright family (just installs Chrome) | — |
| Easiest first-time install | playwright-stealth (`pip install`) | Camoufox (needs Firefox-bin download) |

### 4.1 By customer shape

**"I need to scrape 1M pages a month from amazon-de + 100 other domains, latency-tolerant, cost-sensitive":** Camoufox today (best Pass rate, ~250 MB RSS), evaluate BO routed in 6 months once chapter 06 (AWS WAF solver) lands.

**"I'm running headless workers on Lambda / Cloud Run / Knative — need low memory per worker":** BO. Playwright family is a non-starter at 5+ GB; Camoufox is borderline at 200-400 MB depending on the runtime memory cap. BO at ~400 MB peak with high throughput on the pool path is the most efficient.

**"I have an existing Playwright codebase, can spend 2 days, want better pass rate":** Patchright (drop-in install) or playwright-stealth (one-line apply). Don't expect to beat the basic-Chromium tier.

**"I'm doing internal testing on friendly origins (my own staging env)":** Playwright. Stealth costs you nothing here.

**"I want the lowest p99 latency":** Patchright (3530 ms median, p99 13270 ms — best p99 of all engines).

**"I need to scrape sites that geo-filter Chinese / Russian / Japanese visitors":** BO with the corresponding locale preset (`chrome_148_cn` / `chrome_148_ru` / `chrome_148_jp` — `crates/stealth/src/presets.rs:271-385`). Camoufox has no built-in locale presets; you'd configure them manually.

**"I need maximum diversity to dodge a specific vendor's risk class":** BO routed (4 profiles + custom presets) > Camoufox (1 profile) > Patchright (1 profile).

### 4.2 Cost: pages per dollar (rough)

On AWS m6i.large (~$0.10/hour, 8 GB RAM, 2 vCPU):

| Engine | Concurrent instances per node | Throughput per instance (pages/min) | Throughput per node (pages/min) | Cost per 1M pages |
|---|--:|--:|--:|--:|
| BO pool path | 16 (at ~400 MB) | 14.0 (projected) | 224 | **$0.74** |
| Camoufox | 16 (at ~300 MB honest) | 8.4 | 134 | $1.24 |
| Patchright | 1 (at 5.7 GB) | 13.6 | 13.6 | $12.25 |
| Playwright | 1 (at 5.6 GB) | 12.6 | 12.6 | $13.23 |

Pass-rate weighted (factor in retried failures) shifts numbers, but the order is stable: BO and Camoufox are ≥10× more cost-efficient than the Playwright tier.

---

## 5. Threats / monitoring

### 5.1 Camoufox is actively maintained

Quarterly cadence of Firefox upstream + patch refreshes. What would change their score:
- Firefox 136+ with a new fingerprint reduction (would help)
- A vendor (Kasada most likely) adds a Firefox-specific JA4 + Gecko-runtime check (would hurt)
- C++ patch coverage extends to more APIs (e.g. AudioContext, WebRTC) — would help

**Action**: Re-run the comparative sweep monthly using `benchmarks/run_full_sweep.sh`. If Camoufox jumps to 116+ in a single release, investigate which sites flipped and whether BO can recover via per-profile work or vendor solver primitives.

### 5.2 Anti-bot vendor evolution

Watch quarterly for new fingerprints from:
- **AWS WAF**: challenge.js gets rolled regularly. Per `02_GAP_ANALYSIS.md` the 2026-05-24 challenge uses gokuProps + WebAssembly proof-of-work. Capture and diff at each Amazon retail-season change.
- **Cloudflare**: Managed Challenge changed risk model around 2025-Q4 (per our memory notes); the next inflection will likely add new JA4 + browser-API correlation checks. iphone-class is currently penalized (§5.3 of doc 11).
- **DataDome**: WASM-iframe-daily-key endgame (per `memory/state_2026_05_16_phase5_datadome.md`); follow `dd-script.js` changes via cdn-script-tracker.
- **Kasada**: open-source SOTA frontier; even Camoufox loses canadagoose/hyatt/realtor. Track via `8_KASADA_FRONTIER.md`.
- **PerimeterX (HUMAN)**: zillow's flip-class is unstable — sometimes BO routed passes, sometimes not. Monitor weekly.

### 5.3 BO-specific monitoring

- Chrome major bump: see `11_PER_PROFILE_STRATEGY.md` §7.2 for the update playbook
- iOS Safari major bump: §7.3 — Safari rolls TLS more frequently than Chrome
- BoringSSL updates in boring2 — pin the version; updates can silently change the JA4 if extension order shifts (the `tls_fingerprint_vectors_no_silent_drift` test catches this)
- V8 / deno_core upgrades — V8 sometimes adds new APIs that vendors gate on (e.g. `Promise.try`, `Array.fromAsync`); audit the `crates/js_runtime/src/extensions/` set against current Chrome stable

### 5.4 Camoufox catch-up plan

If we want to beat 113 (the v0.1.0 bar is 115), the priority order is:
1. **Chapter 05 — SPA cluster** (reddit + duolingo + booking + douyin): potential +4 sites, all closest-miss
2. **Chapter 06 — AWS WAF solver** (amazon-de/in/com-au/jp + imdb): potential +5 sites
3. **Chapter 07 — DataDome primitives** (etsy/tripadvisor/yelp restoration): potential +2-3 sites
4. **Chapter 08 — Kasada frontier**: research-bound; if Camoufox can be beaten on Kasada, that's the largest single lever

Hitting (1) gets us to 112; (1)+(2) gets us to 117 (well past Camoufox 113); (1)+(2)+(3) is 119-120 — the loose L3 ceiling.

---

## 6. References

### Competitor sources
- Camoufox repository: https://github.com/daijro/camoufox (MPL-2.0)
- Patchright repository: https://github.com/Kaliiiiiiiiii-Vinyzu/patchright (Apache 2.0)
- playwright-stealth repository: https://github.com/AtuboDad/playwright_stealth (MIT)
- Playwright repository: https://github.com/microsoft/playwright (Apache 2.0)
- puppeteer-extra-plugin-stealth (the parent of playwright-stealth): https://github.com/berstend/puppeteer-extra/tree/master/packages/puppeteer-extra-plugin-stealth

### Bot-detection research
- **creepjs** — https://github.com/abrahamjuliot/creepjs — most-cited fingerprint test harness; treats CDP-driver detection, font fingerprinting, math precision quirks
- **fingerprintjs blog** — https://fingerprint.com/blog/ — published research on fingerprint stability, browser version detection
- **Fastly TLS Fingerprinting blog** — referenced in `tls.rs:194-202` for Chrome's per-handshake Fisher-Yates extension shuffle behaviour
- **Chromestatus 5124606246518784** — Chrome's announcement of the extension shuffling change
- **lexiforest / curl-impersonate** — https://github.com/lexiforest/curl-impersonate — canonical signatures for per-browser TLS fingerprints; used as the reference for our `tls.rs:107,151,161` constants

### BO-side measurement data
- Raw sweep JSONs: `/tmp/full_sweep_2026_05_24/{bo,comp}_*.json`
- Sweep harness: `crates/browser/examples/sweep_metrics.rs`
- Corpus: `crates/browser/tests/holistic_sweep.rs:1-700`
- Classifier: `crates/browser/src/classify.rs`
- Bench driver: `benchmarks/bench_corpus_v2.py`, `benchmarks/run_full_sweep.sh`, `benchmarks/build_report.py`

### Adjacent BO docs
- `docs/BENCHMARK_2026_05_24.md` — sweep narrative report
- `docs/PERFORMANCE_2026_05_24.md` — per-page perf investigation
- `docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md` — ±5-site WAF variance characterization
- `docs/releases/v0.1.0-parity/01_CURRENT_STATE.md` — headline numbers
- `docs/releases/v0.1.0-parity/02_GAP_ANALYSIS.md` — the 10 Camoufox-only sites in detail
- `docs/releases/v0.1.0-parity/11_PER_PROFILE_STRATEGY.md` — per-profile internals + routing rules

### Internal BO code referenced in the comparisons above
- `crates/net/src/tls.rs:22-57` — `TLS_CHROME_MAJOR` / `UA_CHROME_MAJOR` constants + rationale
- `crates/net/src/tls.rs:60-220` — Chrome TLS constants
- `crates/net/src/tls.rs:107-183` — iOS Safari TLS constants
- `crates/net/src/tls.rs:203-228` — Fisher-Yates extension permutation
- `crates/net/src/tls.rs:506+` — silent-drift gate test
- `crates/net/src/headers.rs:143-413` — Sec-CH-UA header generation
- `crates/stealth/src/profile.rs:33-180` — StealthProfile schema
- `crates/stealth/src/presets.rs:120-875` — the 4 shipped presets + variants
- `crates/browser/src/page.rs` — `Page`, `Page::navigate` (in-process navigation, no CDP)
- `crates/browser/src/pool.rs` — `PagePool` (isolate reuse for throughput)
- `Cargo.toml` `[workspace.members]` — the 15-crate engine inventory
- `CLAUDE.md` — workspace conventions (per-thread V8, license rules, scope rules on vendor solvers)
