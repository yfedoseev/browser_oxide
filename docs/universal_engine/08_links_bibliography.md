# 08 — Links and bibliography

Every URL referenced in this documentation. Organized by topic so you can
hunt down specific references quickly.

## The three repos the user asked us to study

- **BotBrowser (GitHub)** — https://github.com/botswin/BotBrowser
- **BotBrowser (DeepWiki)** — https://deepwiki.com/botswin/BotBrowser
- **RiskByPass demo** — https://github.com/RiskByPass/riskbypass_demo
- **Hyper-Solutions hyper-sdk-js** — https://github.com/Hyper-Solutions/hyper-sdk-js
- **Hyper-Solutions SDK (DeepWiki)** — https://deepwiki.com/Hyper-Solutions/hyper-sdk-js

## Open-source stealth browsers / clients

- **puppeteer-extra-plugin-stealth** — https://www.npmjs.com/package/puppeteer-extra-plugin-stealth
- **puppeteer-extra (GitHub)** — https://github.com/berstend/puppeteer-extra
- **undetected-chromedriver** — https://github.com/ultrafunkamsterdam/undetected-chromedriver
- **nodriver** — https://github.com/ultrafunkamsterdam/nodriver
- **Camoufox** — https://camoufox.com
- **Camoufox GitHub** — https://github.com/daijro/camoufox
- **Camoufox Stealth Overview** — https://camoufox.com/stealth/
- **Camoufox DeepWiki** — https://deepwiki.com/daijro/camoufox
- **curl_cffi** — https://github.com/lexiforest/curl_cffi
- **curl-impersonate** — https://github.com/lwthiker/curl-impersonate
- **botasaurus** — https://github.com/omkarcloud/botasaurus
- **rod (Go)** — https://github.com/go-rod/rod
- **playwright-stealth** — https://github.com/AtuboDad/playwright_stealth
- **chromiumoxide (Rust)** — https://github.com/mattsse/chromiumoxide
- **anti-detect comparison** — https://github.com/pim97/anti-detect-browser-tools-tech-comparison

## Akamai BMP reverse engineering and tooling

- **xvertile/akamai-bmp-generator** (BMP 2.1.2–3.3.4 reverse impl) —
  https://github.com/xvertile/akamai-bmp-generator
- **xiaoweigege/akamai2.0-sensor_data** (58-element array) —
  https://github.com/xiaoweigege/akamai2.0-sensor_data
- **klenne/akamai-sensor-data-tools** (parser with Coherence check,
  fpcf.fpValstr) — https://github.com/klenne/akamai-sensor-data-tools
- **OXDBXKXO/akamai-toolkit** (1.70 section parser) —
  https://github.com/OXDBXKXO/akamai-toolkit
- **Edioff/akamai-analysis** (BMP v2 detection pipeline) —
  https://github.com/Edioff/akamai-analysis
- **SteakEnthusiast/Akamai-2.0-Sensor-Data-Decryption-Tool** —
  https://github.com/SteakEnthusiast/Akamai-2.0-Sensor-Data-Decryption-Tool
- **glizzykingdreko/akamai-sensordata-decryptor** —
  https://github.com/glizzykingdreko/akamai-sensordata-decryptor
- **Annotated Akamai 1.7** (cnblogs/yangfei123) —
  https://www.cnblogs.com/yangfei123/p/16320453.html

## Akamai blog articles

- **Akamai v3 Sensor Data: Deep Dive** (glizzykingdreko, Medium) —
  https://medium.com/@glizzykingdreko/akamai-v3-sensor-data-deep-dive-into-encryption-decryption-and-bypass-tools-da0adad2a784
- **Hyper Solutions — Akamai Web Getting Started** (3-sensor max, `~0~`
  stop signal) — https://docs.hypersolutions.co/akamai-web/getting-started
- **hyper-sdk-go akamai package** (`IsCookieValid`) —
  https://pkg.go.dev/github.com/Hyper-Solutions/hyper-sdk-go/akamai
- **Scrapfly: Bypass Akamai Bot Manager** —
  https://scrapfly.io/blog/posts/how-to-bypass-akamai-anti-scraping
- **ZenRows: Bypass Akamai 2025** —
  https://www.zenrows.com/blog/bypass-akamai
- **Kameleo glossary — _abck cookie** —
  https://kameleo.io/glossary/akamai-abck-cookie

## Kasada reverse engineering

- **Kasada ips.js flow (Glizzy Kingdreko)** —
  https://medium.com/@glizzykingdreko (various posts on Kasada 5.2+)
- **kasada-bypass (Bablosoft, closed)** — commercial only, no public repo

## DataDome

- **DataDome JS trace (various writeups on blackhat forums, non-linked)** —
  consult DataDome's public docs at https://docs.datadome.co/

## Cloudflare Turnstile / Bot Management

- **CloudflareBypasser** — https://github.com/sarperavci/CloudflareBypasser
- **FlareSolverr** — https://github.com/FlareSolverr/FlareSolverr

## Chromium / Blink / WebKit source references

- **Chromium googlesource** —
  https://chromium.googlesource.com/chromium/src/
- **Chromium GitHub mirror** — https://github.com/chromium/chromium
- **WebKit main mirror** — https://github.com/WebKit/WebKit
- **Chromium source search** — https://source.chromium.org/chromium
- **cs.chromium.org (old UI)** — https://cs.chromium.org
- **grep.app (cross-repo search)** — https://grep.app
- **Chrome DevTools Protocol** —
  https://chromedevtools.github.io/devtools-protocol/
- **Page.addScriptToEvaluateOnNewDocument** —
  https://chromedevtools.github.io/devtools-protocol/tot/Page/#method-addScriptToEvaluateOnNewDocument
- **Puppeteer evaluateOnNewDocument docs** —
  https://pptr.dev/api/puppeteer.page.evaluateonnewdocument

## Specific Chromium files referenced in `07_blink_source_pointers.md`

- **DynamicsCompressorKernel.cpp (WebKit)** —
  https://github.com/WebKit/WebKit/blob/main/Source/WebCore/platform/audio/DynamicsCompressorKernel.cpp
- **DynamicsCompressorKernel.h (WebKit)** —
  https://github.com/WebKit/WebKit/blob/main/Source/WebCore/platform/audio/DynamicsCompressorKernel.h
- **DynamicsCompressor.cpp (WebKit)** —
  https://github.com/WebKit/WebKit/blob/main/Source/WebCore/platform/audio/DynamicsCompressor.cpp
- **AudioUtilities.{h,cpp} (WebKit)** —
  https://github.com/WebKit/WebKit/blob/main/Source/WebCore/platform/audio/AudioUtilities.h
  (and .cpp)
- **Blink PeriodicWave (Chromium)** —
  https://chromium.googlesource.com/chromium/src/+/main/third_party/blink/renderer/platform/audio/periodic_wave.cc
- **Blink OscillatorNode (Chromium)** —
  https://chromium.googlesource.com/chromium/src/+/main/third_party/blink/renderer/modules/webaudio/oscillator_node.cc

## Rust crates we use or considered

- **deno_core** — https://crates.io/crates/deno_core (V8 integration)
- **rquest** — https://crates.io/crates/rquest (Chrome TLS fingerprinting)
- **boring2** — https://crates.io/crates/boring2 (BoringSSL bindings)
- **tiny_skia** — https://crates.io/crates/tiny_skia (MIT, currently used)
- **skia-safe** — https://crates.io/crates/skia-safe (for T1.1)
- **cosmic-text** — https://crates.io/crates/cosmic-text (for T1.2)
- **fontdb** — https://crates.io/crates/fontdb (for T1.2)
- **rustybuzz** — https://crates.io/crates/rustybuzz (for T1.2)
- **swash** — https://crates.io/crates/swash (for T1.2)
- **glow** — https://crates.io/crates/glow (for T1.4)
- **web-audio-api** — https://crates.io/crates/web-audio-api — **BLOCKED:
  pulls in symphonia 0.5 which is MPL-2.0. Do not use.**

## Fingerprinting test / reference sites

- **FingerprintJS** — https://fingerprint.com (commercial); their
  published audio reference sum `124.04347527516074` is our calibration
  target.
- **CreepJS** — https://abrahamjuliot.github.io/creepjs/ —
  comprehensive fingerprint probe; we've validated against it in prior
  sessions.
- **bot.incolumitas** — https://bot.incolumitas.com — bot-detection
  score (scored in `docs/fingerprint_scorers.md`).
- **pixelscan.net** — https://pixelscan.net — consistency check
- **amiunique.org** — https://amiunique.org — uniqueness scoring
- **antoinevastel.com/bots/areyouheadless** —
  https://antoinevastel.com/bots — headless detection
- **nowsecure.nl** — https://nowsecure.nl — Cloudflare-protected test
  site used in our probe suite
- **sannysoft** — https://bot.sannysoft.com — bot detection test

## Anti-bot blog posts, 2024-2026

- **Scrapfly.io/blog/posts** — search for "bypass" + engine name
- **ZenRows.com/blog** — regular updates on bypass techniques
- **ScraperAPI.com/blog** — comprehensive comparisons of tools

## Miscellaneous

- **RFC 6265** (HTTP State Management / cookies) —
  https://datatracker.ietf.org/doc/html/rfc6265
- **W3C HTML Living Standard (location interface)** —
  https://html.spec.whatwg.org/multipage/nav-history-apis.html#the-location-interface
- **W3C Web Audio API** —
  https://www.w3.org/TR/webaudio/

## Internal references (browser_oxide own files)

- `docs/ANTIBOT_RESEARCH_2026.md` — the prior research writeup
- `docs/CAPABILITY_GAPS_2026.md` — T1 capability matrix and Chrome
  reference values (audio sum, font lists, GPU profiles)
- `docs/STEALTH.md` — stealth profile design
- `docs/STEALTH_HTTP_CLIENT.md` — HTTP client fingerprinting
- `docs/WILDBERRIES.md` — prior WBAAS research
- `docs/WORKERS.md` — Worker implementation notes
- `docs/akamai_sensor_analysis/` — captured Akamai sensor VM samples
  (xiaoweigege 2.0/3.0, samsclub, southwest)
- `docs/kasada_ips_analysis/` — captured Kasada ips.js samples
- `CLAUDE.md` — project conventions

## Memory files (per-session context)

Located at `/home/yfedoseev/.claude/projects/-home-yfedoseev-projects-
browser-oxide/memory/`:

- `MEMORY.md` — index
- `project_overview.md` — project summary
- `user_role.md` — contributor profile
- `session_2026_04_10_tier0_0.5.md` — prior session results
- `tier1_priority_for_akamai.md` — tier 1 priority notes (corrected
  multiple times as investigation progressed)

## Session artifacts

Generated during the 2026-04-10 session, kept under `/tmp/`:

- `/tmp/adidas_sensor_vm.js` — captured 438 KB Akamai sensor VM
- `/tmp/adidas-cookies.txt` — Netscape cookies from live Playwright
- `/tmp/oxide-sensor-*/` — POST body captures at various fix stages
- `/tmp/chrome-sensor/final.html` — Playwright's view of adidas WAF
  hardblock (~2.7 KB, "Reference Error" page)
- `/tmp/webkit_compressor.cc` — WebKit DynamicsCompressorKernel.cpp
  fetched from GitHub
- `/tmp/webkit_compressor.h`
- `/tmp/webkit_compressor_full.cc` — DynamicsCompressor.cpp
- `/tmp/webkit_audio_utilities.{h,cpp}` — linearToDecibels etc.
- `/tmp/capture_adidas_sensor.js` — Playwright capture script (needs
  Node.js and playwright installed at `/tmp/pw-capture/node_modules`)
- `/tmp/get_cookies.js` — CDP cookie extraction script (WebSocket +
  Network.getAllCookies)

## Commercial SaaS that solve these sites

(For reference, not endorsement.)

- **Hyper Solutions** — https://hypersolutions.co (Akamai, DataDome,
  Incapsula, Kasada)
- **RiskByPass** — https://riskbypass.com (per-site Kasada, Akamai,
  DataDome)
- **ScraperAPI** — https://www.scraperapi.com
- **ScrapingBee** — https://www.scrapingbee.com
- **Scrapfly** — https://scrapfly.io
- **ZenRows** — https://www.zenrows.com
- **Brightdata** — https://brightdata.com (proxy + browser)
- **CapSolver** — https://www.capsolver.com (CAPTCHA focus)

These are mentioned so new contributors understand the commercial
landscape. If a tier-1 site is absolutely required for a business
use case, a paid SaaS is often faster than adding more capability work.
