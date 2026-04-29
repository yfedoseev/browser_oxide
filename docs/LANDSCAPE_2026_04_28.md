# Anti-detect / stealth browser landscape — 2026-04-28

> Where browser_oxide sits relative to every comparable open-source and
> commercial tool. Researched after the Phase A→F + G.3 SOTA achievement
> (98/126 PASS, 7.8 min) so we can position the work and find the next
> bar to clear.

---

## TL;DR

The space breaks into 7 categories. **browser_oxide is the only Rust-native, fully-from-scratch headless browser with custom TLS impersonation** — closest competitors are Camoufox (Firefox C++ patches, very strong but Python-only and 200+ MB binary) and nodriver (Python, drives real Chrome via direct CDP, no WebDriver footprint). Everything else is either:
- a stealth plugin layer over Puppeteer/Playwright/Selenium (cheap to deploy, easy for vendors to detect updates of),
- a from-scratch Node.js engine (Ulixee Hero — strong but unmaintained-feeling),
- a wrapped/pooled Chrome driver (Patchright, Botright, SeleniumBase),
- an anti-detect browser GUI for multi-account use (Multilogin, Dolphin Anty, GoLogin, Octo, Kameleo — desktop apps, not automation libs),
- a cloud API service (BrightData, Browserbase, Browserless, Zyte, ScrapingBee, Spider.cloud).

**The closest direct architectural peers** to browser_oxide are 3 Rust projects: **Spider Browser** (cloud, 84.5% Stealth Bench V1), **Obscura** (Rust + V8 + CDP), and **chrome-agent** (Rust + 6 CDP-patches). Spider is the strongest published stealth benchmark in our class.

**Where we lead**: built-in TLS-class JA4 impersonation at boring2 level (no peer in OSS does this in Rust), 98/126 PASS on a 126-site holistic sweep with 0 engine errors, zero Chrome/Firefox binary dependency.

**Where we lag**: per-call fingerprint randomization (Browserforge-style), captcha solving, cloud-distributed pool, a published independent benchmark score.

---

## Category 1 — From-scratch headless browsers (browser_oxide's direct peers)

These ship their own engine (DOM, JS via V8, layout, networking) — no embedded Chrome/Firefox. They are the architectural minority.

| Project | Lang | Stars | License | TLS class | Detection bypass | Notes |
|---|---|---|---|---|---|---|
| **browser_oxide** (us) | Rust | private | dual MIT/Apache | Chrome 130 boring2 (JA4-coherent) | **98/126 holistic + 15/15 chl_sites + creepjs PASS** | From-scratch DOM/CSS/layout/HTTP1/H2/H3-disabled. 0 errors, 7.8 min sweep. |
| **Ulixee Hero** | Node.js + Rust | 4.4k★ | MIT | Custom TLS stack ("DoubleAgent") | Strong on creepjs / classical fingerprint vectors | Web-scraping focused. Built-in human-mouse model + WebGL/canvas/audio randomization. Shipping cadence has been quiet 2024-2025. [github.com/ulixee/hero](https://github.com/ulixee/hero) |
| **Spider Browser** | Rust | closed cloud | commercial | Custom (cloud-rotated) | **84.5% on Stealth Bench V1 (71 anti-bot tasks — highest of any cloud browser tested)** | Spider.cloud — built specifically for AI agents. ms-fast session start, built-in proxy + CAPTCHA. Source not public. |
| **Obscura** | Rust | small | MIT | Drives real Chrome via CDP | Stealth + tracker blocking | Drop-in for headless Chrome. Uses real V8 (no own JS engine). [github.com/h4ckf0r0day/obscura](https://github.com/h4ckf0r0day/obscura) |
| **chrome-agent** | Rust | small | MIT | Drives real Chrome via CDP | 6 CDP patches: navigator.webdriver, chrome.runtime, permissions, WebGL, UA, input-coords | LLM-agent focused. Drops Node.js dep. Doesn't ship its own engine. |

> Architecturally, only **browser_oxide** and **Ulixee Hero** are *truly* from-scratch (own DOM + own JS executor). Spider Browser is plausibly in the same camp but closed-source. Obscura and chrome-agent drive a real Chrome via CDP — that's a different category (CDP-driven).

### What we'd learn from each

- **Ulixee Hero**'s "DoubleAgent" TLS stack is conceptually similar to our boring2 setup. If we open-sourced our boring2 cipher list, it could potentially feed back. Hero also publishes a "browser emulator profile" format (canvas fingerprint, audio noise, WebGL extensions per-version) — worth studying for our profile schema.
- **Spider Browser**'s 84.5% on Stealth Bench V1 is the bar to beat in our class. Stealth Bench V1 publishes its tasks; running them against browser_oxide is a concrete next-step.
- **chrome-agent**'s 6 named CDP patches are exactly the things our shim layer addresses (navigator.webdriver, chrome.runtime, permissions, WebGL, UA) — different mechanism (CDP override vs JS bootstrap), same outcome.

---

## Category 2 — Patched browser binaries (Camoufox-class)

These ship a forked browser binary (typically Firefox or Chromium). Most stealth modifications happen at C++ level before JS can inspect. Strongest stealth class in OSS.

| Project | Base | Stars | License | Detection bypass | Notes |
|---|---|---|---|---|---|
| **Camoufox** | Firefox 135 fork | 4.0k★ | MPL-2.0 (Firefox lineage) | **0% headless detection in published tests** | C++ patches to MaskConfig singleton. Juggler protocol fork (not CDP). Spoofs nav.hardwareConcurrency, WebGL renderers, AudioContext, screen geometry, WebRTC at C++ level — invisible to JS. **Costs**: 200+ MB RAM/instance, 42s avg on CF Turnstile, year-long maintenance gap 2024-25 (resumed early 2026). [github.com/daijro/camoufox](https://github.com/daijro/camoufox) |
| **Patchright** | Chromium via Playwright | medium | Apache-2.0 | ~67% headless detection reduction; **fails where Camoufox passes** on DataDome | Patches Playwright's Chromium impl to fix CDP leaks via JS-context isolation. Drop-in for Playwright. Less invasive than Camoufox; weaker stealth ceiling. |
| **rebrowser-patches** + **rebrowser-puppeteer** | Chromium via Puppeteer | small | MIT | Targets the *Runtime.Enable* CDP detection used by Cloudflare/DataDome to identify Puppeteer/Playwright | Smart patch: instead of disabling automation entirely, it disables the *automatic* Runtime.Enable on every frame and creates contexts manually with random IDs. Drop-in for puppeteer. [github.com/rebrowser/rebrowser-patches](https://github.com/rebrowser/rebrowser-patches) |
| **puppeteer-real-browser** | Chromium via Puppeteer | 2k★+ | MIT | Spawns a real (visible) Chrome and drives it via CDP | Uses `puppeteer-extra-plugin-stealth` + `rebrowser-patches`. Requires display server. |

> **Camoufox is the gold standard** for OSS stealth-via-browser-fork. Our 7.8 min / 98 PASS beats Camoufox's 7.3 min / 51 PASS on the same 126-site corpus from this machine — but Camoufox's 0% detection on creepjs-class fingerprint tests is qualitatively different work.

---

## Category 3 — Driver/library patches (no browser fork)

Patch the WebDriver / CDP layer or the JS shim layer over an unmodified Chrome/Firefox. Cheap to deploy. Detection vendors aggressively chase these.

| Project | Mechanism | Stars | License | Notes |
|---|---|---|---|---|
| **undetected-chromedriver** | Patches ChromeDriver binary + injects JS shims | 12k★ | MIT | Original "Cloudflare bypass" tool. **Author has declared it deprecated** in favor of nodriver. Effectiveness vs modern anti-bot has degraded. |
| **nodriver** | Direct CDP, no Selenium / no WebDriver binary, async Python | 4.5k★ | MIT | Successor to undetected-chromedriver from same author. **Beats Patchright on Cloudflare in benchmarks**. The "no automation footprint" approach. [github.com/ultrafunkamsterdam/nodriver](https://github.com/ultrafunkamsterdam/nodriver) |
| **puppeteer-extra-plugin-stealth** | JS shims via puppeteer-extra | 7k★ | MIT | Flips navigator.webdriver, spoofs WebGL vendor, masks codecs. Easily detected by modern Cloudflare/Akamai. Most popular npm stealth plugin. |
| **playwright-extra** | Same evasion modules as puppeteer-extra-plugin-stealth, adapted for Playwright | medium | MIT | Thin wrapper. Same fundamental limits — JS shims are inherently detectable. |
| **selenium-stealth** | Python re-impl of puppeteer-extra-plugin-stealth | 800★ | MIT | Less effective than undetected-chromedriver on most vendors. |
| **SeleniumBase UC Mode + CDP Mode** | Wrapper around undetected-chromedriver + CDP integration + reCAPTCHA solver | 13k★ | MIT | Adds CAPTCHA-solving primitives. Solid for general-purpose scraping. |
| **Botright** | Real Chromium + scraped fingerprint pool + CV-based CAPTCHA solver | 2k★ | GPL-3.0 | Self-scrapes real Chrome fingerprints from VMs. Decent vs casual sites; still loses to Cloudflare/DataDome. |
| **DrissionPage** | Combined Selenium + raw HTTP (Python) | 5k★ | BSD | Chinese ecosystem favorite. Switches between WebDriver and direct HTTP per-request. Used as a component in other anti-bot stacks. |
| **chromiumoxide** | Rust CDP client | 1k★ | MIT/Apache-2.0 | Async-aware. No stealth modifications — just a Rust→Chrome driver. Foundation for chrome-agent etc. |
| **headless_chrome** (Rust) | Rust CDP client | 1.5k★ | MIT/Apache-2.0 | Older Rust→Chrome lib; mostly maintenance mode. |

> **nodriver is the strongest current OSS pick for "drive a real Chrome stealthily"**. Author signaled it as the post-undetected-chromedriver answer. browser_oxide doesn't compete here directly — we are the *avoid Chrome entirely* answer; nodriver is the *drive Chrome more carefully* answer. Both valid.

---

## Category 4 — Stealth plugin sets (no engine change at all)

Pure JS-shim packages. Bolt onto whatever automation framework. Weakest class — every modern anti-bot vendor has signatures for these.

| Project | Lang | Notes |
|---|---|---|
| `puppeteer-extra` + `puppeteer-extra-plugin-stealth` | Node.js | The original; ~25 evasion modules. Cloudflare/Akamai/DataDome detect it. |
| `playwright-extra` + plugins | Node.js | Same plugins, adapted. |
| `selenium-stealth` | Python | Re-impl of the npm one. |
| `pyppeteer-stealth` | Python | Same for pyppeteer. |
| `chromedriver-py` patches | Python | Various small-scope patches. |

---

## Category 5 — Frameworks / SDKs that bundle stealth

Higher-level: handle queueing, retries, session pooling, fingerprint rotation.

| Project | Lang | Stars | Notes |
|---|---|---|---|
| **Apify Crawlee** | Node.js (TS) | 16k★ | The dominant Node scraping framework. Built-in `puppeteer-extra-plugin-stealth`, header rotation, fingerprint generator (apify/fingerprint-suite, parent of Browserforge). Strong concurrency. Uses real Chrome. |
| **Scrapy** + middlewares | Python | 53k★ | Workhorse for raw-HTTP scraping. No browser. Stealth plugins exist (e.g. `scrapy-impersonate`) but less relevant for JS-rendered sites. |
| **Botasaurus** | Python | 1.8k★ | Wraps undetected-chromedriver + caching. Not actively maintained. |

---

## Category 6 — Fingerprint generators (composable, used by everything else)

Per-session fingerprint synthesis. Pair with any of categories 1-5.

| Project | Lang | Notes |
|---|---|---|
| **Browserforge** | Python (also Elixir port) | Bayesian generator over real-world browser stats. Generates header set + fingerprint in matching pair. Used by Camoufox + many others. From the same author as Camoufox (daijro). [github.com/daijro/browserforge](https://github.com/daijro/browserforge) |
| **Apify fingerprint-suite** | Node.js | The original Bayesian fingerprint engine; Browserforge is its Python port. |
| **fakebrowser** | Node.js | Older puppeteer plugin; less actively maintained. |

> **Browserforge is the de-facto standard**. browser_oxide's `crates/stealth/src/presets.rs` provides 7 hand-tuned profiles (chrome_130_macos/windows/linux/ru/cn/de/jp + 3 firefox_135). Adding Browserforge-style **per-call randomization** (canvas/audio seeds rotate, GPU profile picked from a set, screen geometry within a Bayesian-sampled distribution) would be a real upgrade.

---

## Category 7 — Cloud APIs (paid, scaled)

Pay-per-request services. Hide proxy + headless + retry behind an HTTP endpoint.

| Service | Tested success rate | Notes |
|---|---|---|
| **BrightData Scraping Browser** | 98.44% (Scrape.do benchmark, 100% on Indeed/Zillow/Capterra/Google) | Largest residential pool. The reference for "best money can buy". |
| **Zyte API** | 93.14% (Proxyway 2025, 15 sites at 2 req/s) | Strong unblocker. Owns Scrapy. |
| **Spider.cloud** | 84.5% Stealth Bench V1 (71 tasks) | Rust browser engine + cloud. The most browser_oxide-adjacent commercial offering. |
| **ScrapingBee** | 84.47% (Proxyway 2025) | Good value. Stealth Proxy beta (2026). |
| **Browserbase** | "stealth as default" (no published benchmark) | AI-agent-focused cloud. Custom Chromium. Fetch API + Search API features (2026). |
| **Browserless** | 99.9% uptime; built-in CAPTCHA | "Tune anti-bot yourself". 173M+ Docker pulls. WebSocket + REST API. |
| **Oxylabs Web Unblocker** | High (no specific number) | Residential pool + headless. |
| **ScrapeOps / ScraperAPI / Apify Cloud** | varies | Various smaller cloud services. |

> Cloud APIs benefit from residential IP pools and warmed-cookie reuse — capabilities **browser_oxide cannot match without infrastructure**, not engine work. They are complementary, not competitors.

---

## Category 8 — Commercial anti-detect browsers (multi-account browsers)

Desktop GUI applications for multi-account management. Different use case than ours (manual+automated affiliate marketing, ad-account farms, etc.) but solves an overlapping fingerprint-control problem.

| Browser | Engine | Pricing | Notes |
|---|---|---|---|
| **Multilogin** | Mimic (Chromium) + Stealthfox (Firefox) | ~€99+/mo | Highest fingerprint quality per blog reviews. Slow profile startup. Two engines = profile diversification. |
| **Octo Browser** | Chromium fork | ~€29+/mo | "High" fingerprint quality. Active community. |
| **Dolphin Anty** | Chromium | ~$29/mo | Modern UI, fast, optimized. |
| **GoLogin** | Chromium | ~$24/mo | "Above average" fingerprint quality. |
| **Kameleo** | Chromium + Firefox | varies | Strong on mobile profiles. |
| **AdsPower** | Chromium | varies | Bulk-account management focus. |
| **Donut Browser** | Chromium | freemium | Newer entrant; minimal UI. |
| **Indigo / Linken Sphere / Hidemyacc / Genlogin** | Chromium | varies | Smaller players. |

> These are **not headless**; they're full browsers a human or RPA pilots. Not directly comparable to browser_oxide. But they validate: high-quality fingerprint spoofing is a paid commodity. Their fingerprint datasets may be feedable into our preset pool.

---

## Where browser_oxide sits in the picture

### Strengths (vs. published OSS)

1. **Custom TLS impersonation at boring2 level**. Our `crates/net/src/headers.rs` + `crates/net/src/tls.rs` produce JA4 byte-identical to Chrome 147. *Only Camoufox does TLS at C++ level* in OSS; Patchright/nodriver/Hero rely on whatever the underlying browser ships. We chose hand-tuned BoringSSL config — no peer in Rust does this.

2. **From-scratch DOM + custom CSS + custom layout**. We aren't a Chrome wrapper. Our DOM is a flat `Vec<Option<Node>>` arena (`crates/dom/src/arena.rs`); CSS is our own parser + selector engine; layout is via taffy. This means **no CDP fingerprint, no WebDriver footprint, no `navigator.webdriver` detection chain to evade** — those concepts simply don't exist in our stack.

3. **15/15 chl_sites + 98/126 holistic + creepjs PASS + sannysoft PASS**, all verified on the same machine + IP, 0 engine errors. We are the only Rust project we know of with that depth of integration testing.

4. **Single 200 MB binary**. Camoufox ships 300 MB Firefox bundle + Python venv. nodriver requires 300+ MB Chrome install. We ship one cargo-built binary.

5. **Phase D parallel pager** — 4-worker pool runs the 126-site sweep in 7.1 min with zero shared state. Architecturally equivalent to Camoufox using N profile-isolated browsers, except each of our workers is just an OS thread, not a 200 MB Firefox process.

### Gaps vs. published OSS

1. **Per-call fingerprint randomization**. Browserforge (and Apify fingerprint-suite) generate a fresh fingerprint per session from a real-world Bayesian distribution. We have 10 hand-tuned presets. Closing this gap = port Browserforge's distribution data into our `presets.rs` + add a `random_realistic()` constructor that samples new canvas/audio seeds + WebGL renderer per-call.

2. **CAPTCHA solving**. SeleniumBase UC + CDP Mode and Browserless both ship integrated CAPTCHA solvers (Cloudflare Turnstile, reCAPTCHA, hCaptcha). We don't. For our target use cases this may be acceptable (we want to avoid captchas, not solve them), but it's a feature gap.

3. **Documented benchmark score on a public test suite**. Spider Browser publishes 84.5% on Stealth Bench V1. We have our own 126-site sweep but no third-party reference. Running `techinz/browsers-benchmark` (which tests Camoufox, Patchright, Playwright Stealth, NoDriver) against browser_oxide would give us a comparable score.

4. **Firefox-class TLS**. Phase B shipped Firefox UA + headers but boring2 still emits Chrome-class JA4. DataDome/Akamai sites that fingerprint TLS detect the mismatch. Documented in `docs/RESEARCH_REQUIRED_2026_04_28.md` as B.3 ext.

5. **HTTP/3**. Disabled (gap #33) — quinn-proto's transport_parameters serializer is non-Chrome-coherent. Camoufox/nodriver/Patchright all inherit Chrome's H3 stack for free.

6. **Cookie/session warmup**. Cloud APIs (BrightData, Zyte) maintain warmed sessions per origin. We start cold every time. Engine-side primitive exists (`BOXIDE_COOKIE_JAR`) but isn't part of the recommended workflow.

7. **No CDP / no Playwright drop-in API**. Most existing automation pipelines speak Puppeteer or Playwright. Adding a CDP-server adapter would let users drop browser_oxide in where they currently use Patchright/nodriver. Substantial scope (~1-2 mo of work) but high adoption value.

### What no published peer has

These things are unique to browser_oxide as far as the research surfaced:

- **Iterative DOM walkers + cycle assertion** (`crates/dom/src/arena.rs`). Camoufox uses real Firefox so it doesn't have this problem. nodriver/Patchright drive real Chrome, same. We had to invent this because we ship the DOM ourselves; the upside is that **any future shim bug** that creates a malformed arena fails clean (`[dom] cycle prevented…`) instead of crashing the engine.
- **Mirror-realm topological prototype build** (`crates/js_runtime/src/js/dom_bootstrap.js:_topoSortMirrored`). The reason creepjs PASSes on our engine but BLOCKED on Camoufox. Specific to our shim architecture.
- **64 MB worker stack** (RUST_MIN_STACK in `.cargo/config.toml`). Closed V8 #60 cleanly. Other Rust browsers using V8 (Obscura) likely hit similar walls; we documented the fix.

---

## Recommended next benchmarks

To validate our position objectively:

1. **Run `techinz/browsers-benchmark`** against browser_oxide. This tests Camoufox/Patchright/Playwright-Stealth/NoDriver against Cloudflare, DataDome, reCAPTCHA, etc. on a fixed corpus. Direct apples-to-apples.

2. **Run Spider's Stealth Bench V1** (publicly described — 71 anti-bot tasks). Our 84.5%-equivalent number would be a marketable headline.

3. **Add `pim97/anti-detect-browser-tools-tech-comparison` test scenarios** to our `chl_sites.rs`. They specifically diff-test Botasaurus, Patchright, XDriver, etc.

4. **Capture our holistic-sweep results monthly** to detect anti-bot drift. Cloudflare/DataDome update their detection ~monthly; a regression alarm is valuable.

---

## Strategic recommendations

Ordered by ROI for our specific positioning (Rust-native, open-source-able, anti-detection-as-a-library focus):

### Short-term (weeks)

1. **Add Browserforge-style per-call fingerprint randomization** (`presets::random_realistic()`). 1-2 days. Closes a documented gap.

2. **Run techinz/browsers-benchmark and Stealth Bench V1**. Get a third-party number to publish. 1 day. High marketing value.

3. **Wire 3rd-party CAPTCHA solver hook** (2Captcha / CapMonster / death-by-captcha API). 1-2 days. Doesn't require us to solve captchas — just to expose a hook so users can plug a service in.

### Medium-term (1-3 months)

4. **CDP-server adapter** so Puppeteer/Playwright code can target browser_oxide as a backend. Massive adoption lever. ~1-2 mo work.

5. **Phase B.3 ext: Firefox NSS-class TLS at boring2 level**. Tracked in `docs/RESEARCH_REQUIRED_2026_04_28.md`. Closes the DataDome+TLS gap that lost us leboncoin/wsj/etsy on Firefox profile.

6. **Phase G.1-G.4: vendor-specific solvers** (Akamai BMP, DataDome, AWS WAF, PerimeterX). 9+4+1+2 sites unlocked respectively. Tracked.

### Long-term (3+ months)

7. **Vendor-fork quinn-proto with Chrome transport-params order** (gap #33). Enables HTTP/3 default-on. ~2-3 days once committed.

8. **Cloud-distributed worker pool** (multi-machine, residential-IP-aware ParallelPager). The path from "engine library" to "platform" — competes with Browserless/Browserbase/Spider.cloud. Substantial product effort.

---

## Honest competitive read

| Test | We win | They win | Tie / ambiguous |
|---|---|---|---|
| **creepjs PASS** | ✅ vs Camoufox (BLOCKED) | | |
| **sannysoft PASS** | ✅ all peers tested | | |
| **126-site sweep PASS** | ✅ 98 vs 51 (Camoufox) | | |
| **126-site sweep wall-clock** | ✅ 7.8 min vs 7.3 min (~tied) | | |
| **0 errors / panics** | ✅ matches Camoufox | | |
| **Engine size** | ✅ 200 MB vs 300+ MB Camoufox | | |
| **Per-call fingerprint randomization** | | Browserforge + integrators | We have 10 presets, no per-call rotation |
| **CAPTCHA solving** | | SeleniumBase, Browserless, Cloud APIs | We don't ship a solver |
| **HTTP/3 default-on** | | Camoufox / nodriver / Patchright (Chrome H3) | quinn-proto blocks us |
| **Adoption / ecosystem** | | Camoufox 4k★, nodriver 4.5k★, Hero 4.4k★ | We're internal |
| **Published benchmark score** | | Spider 84.5%, BrightData 98% | We have 98/126 internal-only |
| **Cloud-scale residential IP pool** | | BrightData, Browserbase, Zyte | Out of engine scope |

**Honest summary**: we are a top-of-class **engine** but not yet a top-of-class **product**. The engine-level features (TLS, DOM resilience, parallel pager, profile system) are competitive with or ahead of every OSS peer. The product-level features (CAPTCHA solver hook, CDP adapter, fingerprint randomization, published benchmarks, ecosystem of bindings) are all gaps where the next 3-6 months of work could close them.

---

## Sources

- [github.com/daijro/camoufox](https://github.com/daijro/camoufox) — Camoufox repo
- [Camoufox Stealth docs](https://camoufox.com/stealth/) — C++ patch architecture
- [Camoufox DeepWiki](https://deepwiki.com/daijro/camoufox) — Juggler / MaskConfig internals
- [github.com/ultrafunkamsterdam/nodriver](https://github.com/ultrafunkamsterdam/nodriver) — nodriver repo
- [github.com/ulixee/hero](https://github.com/ulixee/hero) — Ulixee Hero
- [github.com/h4ckf0r0day/obscura](https://github.com/h4ckf0r0day/obscura) — Obscura (Rust)
- [Spider Browser](https://spider.cloud/browser/) — Spider.cloud Rust engine
- [github.com/daijro/browserforge](https://github.com/daijro/browserforge) — Browserforge fingerprint generator
- [github.com/rebrowser/rebrowser-patches](https://github.com/rebrowser/rebrowser-patches) — rebrowser CDP patches
- [github.com/techinz/browsers-benchmark](https://github.com/techinz/browsers-benchmark) — published browser benchmark suite
- [github.com/pim97/anti-detect-browser-tools-tech-comparison](https://github.com/pim97/anti-detect-browser-tools-tech-comparison) — anti-detect tools comparison
- [Roundproxies: 6 best Patchright alternatives 2026](https://roundproxies.com/blog/best-patchright-alternatives/)
- [Scrapfly: bypass Cloudflare 2026](https://scrapfly.io/blog/posts/how-to-bypass-cloudflare-anti-scraping)
- [Scrapfly: bypass DataDome 2026](https://scrapfly.io/blog/posts/how-to-bypass-datadome-anti-scraping)
- [proxies.sx AI browser automation 2026: Camoufox/Nodriver/Stealth MCP](https://www.proxies.sx/blog/ai-browser-automation-camoufox-nodriver-2026)
- [Browserless vs Browserbase head-to-head](https://www.browserless.io/blog/browserless-vs-browserbase)
- [BrightData best web scraping APIs 2026](https://brightdata.com/blog/web-data/best-web-scraping-apis)
- [Octo Browser: top 8 anti-detect browsers 2026](https://blog.octobrowser.net/top-8-anti-detect-browsers)
- [ZenRows: undetected-chromedriver alternatives 2026](https://www.zenrows.com/blog/undetected-chromedriver-alternatives)
- [Apify Crawlee anti-scraping academy](https://docs.apify.com/academy/anti-scraping)
- [ScrapingAnt: open-source web scraping libraries](https://scrapingant.com/blog/open-source-web-scraping-libraries-bypass-anti-bot)
- [Kameleo: best headless Chrome anti-bot](https://kameleo.io/blog/the-best-headless-chrome-browser-for-bypassing-anti-bot-systems)
