# 03 — Research landscape: what other projects do

This file summarizes publicly-available stealth browser research as of
2026-04. It exists so new contributors don't re-do the homework.

## The three projects the user explicitly asked us to study

### 1. `botswin/BotBrowser` (TypeScript wrapper around a patched Chromium)

- **URL**: https://github.com/botswin/BotBrowser
- **DeepWiki**: https://deepwiki.com/botswin/BotBrowser
- **Architecture**: NOT a from-scratch browser. It is a prebuilt modified
  Chrome distribution. The public GitHub repo only contains four
  illustrative patch diffs (`patches/removeHeadless.diff`, `timezone.diff`,
  `webglAttrs.diff`, `video_capture_device_descriptor.cc.diff`). The real
  engine patches are closed-source.
- **Modified dirs (per DeepWiki)**: `v8/src/`, `third_party/blink/renderer/
  {core,modules}/`, `services/network/p2p/`, `content/browser/
  client_hints/`.
- **Per-engine logic**: **None in the runtime**. Claims "unified fingerprint
  defense": one patch set for all anti-bot engines. Changelog mentions
  reactive vendor-specific fixes ("fixed a Kasada regression") but those
  are engineering work, not runtime branches.
- **Key architectural trick**: Profile pushed into Chromium `BrowserContext`
  via custom CDP command `BotBrowser.setBrowserContextFlags`. Called at
  browser-level before any page is created. The renderer reads flags at
  startup, so all Workers (Dedicated/Shared/Service) and navigations
  inherit the same bundle — no per-navigation re-injection needed.
- **Canvas**: deterministic noise injected in the Skia pipeline at Blink
  `modules/canvas/`.
- **WebGL/WebGPU**: deterministic noise at the same layer.
- **Audio**: deterministic noise at Blink `modules/webaudio/` — covers
  PeriodicWave wavetable output because everything flows through the noise
  layer.
- **V8**: real V8 with floating-point normalization patches in `v8/src/`.
- **TLS**: BoringSSL patches for JA3/JARM/ALPN ordering (docs say "under
  evaluation" — not fully independent TLS stack).
- **Test suite**: `tests/tests/antibots/*.spec.ts`. **Akamai targets**:
  `playstation.com, stubhub.com, aircanada.com` (all easier than
  adidas/homedepot BMP v3). **Kasada targets**: `wizzair.com` only. **No
  adidas, no homedepot, no canadagoose, no hyatt**.
- **Claims in README**: passes Cloudflare, Akamai, Kasada, Shape, DataDome,
  PerimeterX, Imperva.

**Takeaway**: Confirms the "generic runtime, aggressive capability work at
the C++ layer" approach. Does not demonstrate it can pass the hardest
tier-1 BMP v3 / Kasada sites.

### 2. `RiskByPass/riskbypass_demo` (Python)

- **URL**: https://github.com/RiskByPass/riskbypass_demo
- **Architecture**: Demo client for a **paid remote solver API**. Not a
  framework, not a browser. Each Python file is a ~100-line `requests`
  script that POSTs to `https://riskbypass.com/task/submit` and replays
  the response.
- **Per-engine / per-site**: **Maximally so**. File layout:
  ```
  kasada/kick.py
  kasada/hyatt.py
  kasada/nike.py
  kasada/sephora.py
  kasada/twitch.py
  kasada/aircanada.py
  kasada/arcteryx.py
  akamai/kohls.py
  akamai/fedex.py
  akamai/coachoutlet.py
  ```
  Each file hardcodes `task_type: "kasada" | "akamai" | "datadome"` plus
  site-specific params (`akamai_js_url`, `page_fp`, `kasada_js_domain`).
- **Browser**: None. Uses `curl_cffi` / `requests_go` for TLS
  impersonation, fetches the solved token from the remote service, then
  replays it into the customer's HTTP session.

**Takeaway**: Proves that tier-1 Kasada (hyatt, canadagoose-equivalent
Arc'teryx) is solved commercially as a **per-site product**, not
generically. The per-site nature of the solution is what customers pay for.
Nobody ships this as open-source.

### 3. `Hyper-Solutions/hyper-sdk-js` (TypeScript)

- **URL**: https://github.com/Hyper-Solutions/hyper-sdk-js
- **DeepWiki**: https://deepwiki.com/Hyper-Solutions/hyper-sdk-js
- **Architecture**: Thin client for a **paid remote solver**. Explicitly
  per-engine module layout:
  ```
  akamai/{sensor.ts, pixel.ts, sbsd.ts, sec_cpt.ts, dynamic.ts, script_path.ts}
  datadome/{interstitial.ts, slider.ts, tags.ts}
  incapsula/{reese.ts, utmvc.ts, dynamic.ts}
  kasada/{payload.ts, pow.ts, botid.ts, script_path.ts}
  ```
- Each module POSTs input params to `{engine}.hypersolutions.co/…`,
  receives sensor_data/payload/cookies, returns it.
- **Local crypto solving**: only `sec_cpt` has any, and it's minimal.
- **No V8, no browser.**
- **One architectural nugget worth noting**: they keep distinct endpoints
  per *sub-challenge* (sbsd vs sensor vs pixel vs sec_cpt for Akamai).
  That confirms even commercial vendors treat each Akamai variant as a
  separately-trained model. Akamai BMP v3 is not one uniform system — it's
  a family of challenges and the sub-challenge you see depends on the
  specific site's configuration.

**Takeaway**: Commercial bypass is per-engine AND per-sub-challenge.
Multiple models per engine. Not reachable via generic browser capability
work alone — which is why BotBrowser's test suite avoids the hardest
targets.

## Broader landscape: 10 more projects

| Project | Architecture | Per-engine? | Passes tier-1? |
|---|---|---|---|
| **puppeteer-extra-plugin-stealth** (npm) | JS `evaluateOnNewDocument` patches on real Chrome | Generic (fingerprint only) | No — ZenRows and Scrapfly 2026 reviews both say insufficient vs Akamai/Kasada |
| **undetected-chromedriver** (Python) | Real Chrome + CDP, strips webdriver flags at launch | Generic | No for BMP v3 |
| **nodriver** (Python, async CDP) | Real Chrome + CDP, zero in-page init scripts | Generic — author's explicit pitch is "no framework code in the page" | No for BMP v3 |
| **Camoufox** (Firefox fork) | Firefox Juggler C++ patches, fingerprint spoofing below the JS layer | Generic — patches at C++ level so JS can't detect | Better than most; still not adidas-level |
| **curl_cffi** (libcurl + BoringSSL fork) | Pure HTTP | No engine logic — just per-browser TLS impersonation presets (`chrome120`, `chrome131`, `safari_ios_17`) | N/A — HTTP only, no JS execution |
| **botasaurus** (Selenium wrapper) | Patched Selenium + requests | Generic stealth, per-site decorators for retry/proxy | No for BMP v3 |
| **rod** (Go, CDP) | CDP client | Generic | No for BMP v3 |
| **playwright-stealth** (Python) | Port of puppeteer-extra evasions | Generic, same limits as puppeteer-stealth | No for BMP v3 |
| **chromiumoxide** (Rust, CDP) | CDP client | Generic | No for BMP v3 |
| **servo** (Rust, Mozilla browser engine) | Full alternative browser engine | Not targeted at stealth | N/A — not a stealth project |

## The universal pattern

Every "real browser" stealth project is **generic**. Every "HTTP bypass"
project is **per-engine**. Nobody ships per-engine logic inside a real
browser runtime. The per-engine logic always lives either:

- In a commercial remote solver (Hyper-Solutions, RiskByPass, Kasada
  Solver, Shape bypass SaaS), OR
- In the end-user's scraper scripts on top of a generic browser, as per-
  site decorators / hooks / workarounds.

This is the explicit architectural principle browser_oxide adopts. See
`01_architecture_principle.md`.

## The `addScriptToEvaluateOnNewDocument` pattern

The single most important primitive found in the research: every CDP-based
stealth project uses `Page.addScriptToEvaluateOnNewDocument` to install
init scripts that Chrome re-runs automatically before every new Document's
first `<script>`. This is how `location.reload()`, `location.href = ...`,
meta-refresh, history navigation, and cross-origin redirects all inherit
the same fingerprint for free. See:

- Puppeteer docs: https://pptr.dev/api/puppeteer.page.evaluateonnewdocument
- Chrome DevTools Protocol: https://chromedevtools.github.io/devtools-protocol/tot/Page/#method-addScriptToEvaluateOnNewDocument
- Chromium internals: `content/renderer/render_frame_impl.cc` — the
  `DidCommitProvisionalLoad` path calls `ScriptController` to re-evaluate
  init scripts for every new Document. That's the C++ the CDP command
  proxies to.

**For browser_oxide**, the equivalent is: drop and rebuild the
`js_runtime::BrowserJsRuntime` on every navigation commit, with bootstrap
scripts re-running in order before any parsed-HTML `<script>` executes.
Our current architecture mostly does this already; the missing piece is
that `location.reload()` / `location.href = ...` / `<meta refresh>`
don't trigger the rebuild. See task #74 in `04_refactor_plan.md`.

## The hard truth about tier-1 sites

No open-source browser in our research passes:

- `adidas.com` (Akamai BMP v3, stringent mode)
- `homedepot.com` (Akamai BMP v3)
- `canadagoose.com` (Kasada)
- `hyatt.com` (Kasada)

**Evidence**:
- BotBrowser's own test suite specifically avoids them.
- Kasada bypass repos (RiskByPass, xvertile/akamai-bmp-generator,
  SasanFarrokh/akamai-decoder) treat them as paid per-site products.
- Multiple 2026 "how to bypass Akamai" blog posts (Scrapfly, ZenRows,
  ScraperAPI) conclude with "use a managed service."

This does not mean it's impossible. It means:

1. The economic incentive to open-source a solution is small — there's a
   SaaS market above $500/month.
2. The actual work to pass them is mostly **fingerprint bit-accuracy** plus
   **behavioral telemetry accuracy**, not architectural cleverness.
3. Anyone who solves one of these releases it privately or commercializes
   it.

The browser_oxide strategy for these sites is to **keep closing the
capability gaps** (T1.1 real canvas via skia-safe, T1.2 real fonts, T1.4
real WebGL via OSMesa) until the fingerprint is Chrome-bit-accurate. See
`05_capability_gaps.md`.

## Useful URLs for deep research

All collected in `08_links_bibliography.md`.
