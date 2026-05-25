# 29 — F5 Distributed Cloud Bot Defense (formerly Shape Security)

**Status:** reference — customer-onboarding handbook
**Cluster:** *none in the 126-corpus.* This chapter is forward-looking and exists for two reasons: (1) customers who onboard with browser_oxide will bring sites protected by F5 / Shape (banking, airlines, large retail), and we need a documented recognition + capability story; (2) cross-vendor pattern synthesis (see `42_CROSS_VENDOR_PATTERNS.md` if present, otherwise the patterns recur in `25_CLOUDFLARE_DEEP.md §0`, `26_AKAMAI_BMP_DEEP.md §2`, `08_KASADA_FRONTIER.md §0`) — Shape pioneered the JS-VM-with-rotating-opcodes pattern that Kasada, Akamai BMP-v3, and DataDome-WASM have all since adopted in some form.
**Companion docs:** `18_ANTI_BOT_VENDOR_COOKBOOK.md` (where this vendor needs a new §2 entry once we add a marker row — currently absent), `27_VENDOR_COMPETITIVE_MATRIX.md §1.1` (where this becomes a vendor cluster once we onboard a Shape-protected site), `08_KASADA_FRONTIER.md` (Kasada's `/ips.js` VM is the closest public-corpus analogue), `23_TLS_HTTP_FINGERPRINT_REFERENCE.md` (because Shape's JA3/JA4 dependence makes our `boring2` byte-perfect-Chrome TLS the load-bearing primitive).

---

## TL;DR

F5 acquired Shape Security in January 2020 for **\$1 B** (see [GeekWire 2019-12-19](https://www.geekwire.com/2019/f5-networks-will-acquire-shape-security-1b-bolster-online-fraud-protection-services/), [F5 press release](https://www.f5.com/company/news/press-releases/f5-to-acquire-shape-security)). The product is now branded **F5 Distributed Cloud Bot Defense** (still called "Shape" internally by F5's docs and on customer deployments where the legacy script paths haven't been re-pathed). It is the highest-tier commercial anti-bot stack, deployed at most of the top-tier US banks, large airlines, and major retailers (see §1.3).

The technical heart is a **custom JavaScript Virtual Machine** with a rotating instruction set (~230 handlers, of which 80+ are randomized roughly every 30 minutes per [svebaa's RE analysis](https://svebaa.github.io/personal/blog/shape-security/)), proprietary "superpack" compression of the telemetry payload, and transmission via custom HTTP headers with per-tenant prefixes (the `X-DQ7Hy5L1-{a..z}` pattern that recurs in public reverse-engineering work — see [`ranftldieterHub/Shape-Security-protection-reverse-engineering`](https://github.com/ranftldieterHub/Shape-Security-protection-reverse-engineering)). On the server side, the Shape AI Cloud scores every request with a network-effect ML model trained across F5's full customer base (per [F5 product page](https://www.f5.com/products/distributed-cloud-services/bot-defense)).

What this means for browser_oxide:

1. **Recognition** — Shape is identifiable by a handful of script paths (`/ssx/ssx.mod.js`, the per-tenant launcher), the custom `X-D*-{a..z}` request header family, and a small set of cookies (covered in §3). We have **zero** detection markers for this vendor in `crates/browser/src/classify.rs` today.
2. **Engine-side capability** — Because Shape's first-pass detection is heavily TLS / HTTP-2 / JA3 / JA4 based (per [RoundProxies 2026 bypass guide](https://roundproxies.com/blog/f5-bypass/)), our existing `boring2` byte-perfect Chrome ClientHello + HTTP/2 SETTINGS profile (per `23_TLS_HTTP_FINGERPRINT_REFERENCE.md`) already clears Shape's TLS-class gate. The harder gate is the in-VM telemetry that the rotating opcodes produce.
3. **Solver** — out of scope for `vendor_solvers` v0.1, full stop. A Shape solver is at least a quarter of dedicated work by a senior reverse-engineer (Kasada was ~6 months and the surface is smaller). It does not justify itself in the v0.1.0 roadmap.
4. **Customer-onboarding posture** — if a customer brings a Shape-protected site, BO clears the **passive** (no-JS) layer (TLS + headers + initial fingerprint surface). BO does **not** clear the **active** layer (rotating-VM telemetry). For a customer whose use case is page-render-only of Shape-protected content, we have a viable story; for one whose use case is high-volume POST-flows behind Shape (login, checkout, scrape-after-login), the honest answer is no.

This chapter documents that posture defensibly.

---

## 1. Product overview

### 1.1 Origin

Shape Security was founded in **2011** in California by Derek Smith, Sumit Agarwal, and Justin Call (per [StartupIntros profile](https://startupintros.com/orgs/shape-security)). Their thesis: large-enterprise web traffic is dominated by automated abuse (credential stuffing, gift-card cracking, inventory hoarding), and the cost of a single account takeover dwarfs the cost of a sophisticated client-side defense. They built Shape Enterprise Defense around an **AI-powered + JavaScript-VM-obfuscated** dual stack — the VM made reverse-engineering expensive, and the ML cloud caught what the VM missed.

By 2019 Shape was protecting "two billion logins a month" (per the F5 acquisition announcement) for marquee customers — primarily **financial services** and **airlines** and large **retail**. F5 announced acquisition on 2019-12-19 for **\$1 billion** cash (per [GeekWire](https://www.geekwire.com/2019/f5-networks-will-acquire-shape-security-1b-bolster-online-fraud-protection-services/) and the [F5 employee letter](https://www.f5.com/company/blog/announcing-shape-security-acquisition)), the deal closing in January 2020. F5 retained Shape CEO Derek Smith and the Santa Clara office. The product was sequentially rebranded:

| Period | Brand | Notes |
|---|---|---|
| 2011-2019 | **Shape Enterprise Defense** | Standalone Shape Security product |
| 2020-2022 | **Shape Integrated Bot Defense** | Post-acquisition transitional name |
| 2022-present | **F5 Distributed Cloud Bot Defense** | Current branding; integrated into F5's XC (Distributed Cloud) SaaS platform |

Per F5's customer-facing site, the product is also still purchasable as **F5 BIG-IP Advanced WAF + Bot Defense** for on-premises deployments, and customer legacy script paths (`assets.targetimg1.com/ssx/...` per [svebaa's RE analysis](https://svebaa.github.io/personal/blog/shape-security/)) preserve the original Shape domain conventions.

### 1.2 Deployment modes

F5 supports **three** deployment topologies (per [F5 product page](https://www.f5.com/products/distributed-cloud-services/bot-defense) and [F5 Hybrid Security Architectures KB](https://community.f5.com/kb/technicalarticles/f5-hybrid-security-architectures-part-4---f5-xc-bot-and-ddos-defense-and-big-ip-/312342)):

1. **F5 Distributed Cloud (managed SaaS)** — customer's traffic is routed through F5's global PoPs; Shape's JavaScript and telemetry path is fully managed by F5. The **majority** of new deployments (per Gartner and trade-press coverage). The script-serving and verdict-issuing happens entirely off the customer's infrastructure.
2. **BIG-IP on-prem (via iApp / native module)** — Customer runs F5 BIG-IP appliances (the Traffic Management Module, TMM, stack). Shape script-injection happens at the BIG-IP layer, telemetry flows back out to F5 Cloud over a backhaul connection. Common at large banks with regulatory in-DC constraints (per [F5's BIG-IP integration KB](https://community.f5.com/kb/technicalarticles/how-to-easily-protect-your-big-ip-applications-using-f5s-distributed-cloud-bot-d/295578)).
3. **CDN-fronted (connector-based)** — F5 ships pre-built connectors for Amazon CloudFront, Salesforce Commerce Cloud, Adobe Commerce. Shape script + verdict path runs at the CDN edge.

What matters for browser_oxide identification: **the script paths and header conventions are stable across deployment modes** (the launcher + VM file naming, the per-tenant header prefix). Mode (1) tends to use F5-owned subdomains (`*.cloud.f5.com`, `*.ves-io.com`); modes (2) and (3) typically use a customer-owned subdomain (e.g. `static.<customer>.com/ssx/ssx.mod.js`) — which is one reason Shape is easier to miss than Cloudflare (CF's `cf-ray` header is on every response).

### 1.3 Customer base (best-public-knowledge)

The F5 product page explicitly cites "world's largest banks, retailers, and airlines" and shows customer case studies for **PUMA North America** and **Q2** (community bank platform — see [F5 customer page](https://www.f5.com/customers)). Industry analyst writeups and Shape's pre-acquisition disclosures put the customer list more specifically:

| Sector | Known / commonly cited customers | Source confidence |
|---|---|---|
| **Top-tier US banks** | Bank of America, JPMorgan Chase, Wells Fargo, Capital One, US Bank | Strong (multiple analyst sources; F5 cites "9 of the top 10 US banks"); specific customer-site confirmation requires per-site capture |
| **Airlines** | Southwest, JetBlue (Shape was cited in JetBlue's anti-scraping response); rumored across most majors | Moderate (Southwest is named in F5 case-study material; rest is inferred) |
| **Retail** | PUMA NA, Marriott (post-2018 hack), Starbucks rewards | Strong (PUMA is on the F5 site; Marriott is in pre-acquisition press) |
| **Other** | Q2 (community-banking platform — 97% malicious traffic blocked per F5) | Strong (named case study) |

**Cross-reference with our corpus:** the 126-corpus includes `wellsfargo` (passes BO on all 4 profiles per `01_CURRENT_STATE.md`). If Wells Fargo deploys Shape, *and BO passes*, the inference is one of: (a) Wells Fargo's public marketing site is on a non-Shape path (likely — Shape is typically on auth flows, not marketing pages); (b) Shape's passive layer accepts our byte-perfect Chrome TLS without escalating to the rotating-VM tier (plausible per RoundProxies' finding that "TLS alone blocks 80%+ of early attempts"); or (c) the script is present but the engine renders past it because the page doesn't gate on a Shape verdict. **We have not run a per-page capture on wellsfargo to disambiguate.** This is logged as an open question (§9.4).

The honest takeaway: a customer-onboarding capture from a Shape-protected site is the only way to confirm BO's behaviour against Shape. The capture tooling lives in `crates/browser/examples/sweep_metrics.rs` and the methodology is in `04_TOOLING_SPEC.md §3`.

### 1.4 Why F5 calls it different from competitors

Per F5's own [Gartner Peer Insights page](https://www.gartner.com/reviews/market/online-fraud-detection/vendor/f5/product/f5-distributed-cloud-bot-defense) and product-page positioning, the load-bearing differentiator vs. Akamai BMP / Imperva / DataDome / Cloudflare is:

> "Even when an attacker successfully solves our challenge, our ML cloud scores them as automated based on hundreds of other signals across the network."

This is the **intent vs. automation** framing — Shape claims that even a successfully-bypassed client (one that emits valid telemetry) still gets blocked if the **request pattern** matches a known attack campaign. This is also what justifies the premium price (F5 quotes are public-knowledge in the \$200k-\$1M / year range for enterprise tier). For us, the practical implication is that **a single successful page-render is a strictly weaker test than a sustained scrape session** — Shape's network-effect ML model needs request volume to score. A one-shot capture from a fresh IP might pass; the second-hundredth probably won't.

---

## 2. Detection architecture

Shape's architecture is a textbook two-tier "challenge + cloud" pattern, where the challenge layer is heavily obfuscated and the cloud layer is the actual decision engine. The pattern was novel in 2011 and has been imitated by every major vendor since.

### 2.1 The two-tier stack

```
                 Browser (or BO / Camoufox / PW)
                          │
                          │ 1. GET <protected-page>
                          ▼
       F5 edge (XC PoP, BIG-IP TMM, or CDN connector)
                          │
                          │ 2. Inject launcher script
                          │    (<script src="…/ssx/ssx.mod.js?async">)
                          ▼
                       Browser
                          │
                          │ 3. Launcher fetches VM file
                          │    (<script src="…/ssx/ssx.mod.js?seed=<id>">)
                          │
                          │ 4. VM bootstraps, hooks DOM events,
                          │    starts collecting telemetry signals
                          │
                          │ 5. Form submit / XHR / fetch
                          ▼
       Browser POSTs with X-D*-{a..z} headers (superpack-encoded
                                                        telemetry)
                          │
                          ▼
       F5 edge — first-pass synchronous scoring (TLS, JA3/JA4,
                 HTTP/2 SETTINGS, header order, IP reputation,
                 per-session cookies, ASN class, IP age)
                          │
                          │ pass → forward to origin
                          │ fail → 4xx / silent serve / shadow-ban
                          ▼
                   Telemetry → Shape AI Cloud
                          │
                          │ network-effect ML scoring
                          │ retroactive cookie / IP poisoning
                          ▼
              Future requests pre-blocked at edge
```

The crucial property: **the second-tier ML is asynchronous w.r.t. the first request.** A bypass that succeeds on request 1 may be transparently blocked on requests 2..N by retroactive cookie/IP poisoning. This is the "intent detection" claim above.

### 2.2 Mobile attestation (separate stack)

F5 ships a **Shape Mobile SDK** for iOS and Android that's separate from the web stack — it runs in the customer app, collects device-attestation signals (Apple DeviceCheck, Android Play Integrity), TLS pinning, jailbreak/root detection, and posts its own per-request token. For us this is **not relevant** — we render web content, not mobile-app content. A customer who needs to defeat the Shape Mobile SDK is asking for an entirely different product class (mobile-app instrumentation, not a browser engine).

### 2.3 The Shape JavaScript Virtual Machine — what is publicly known

Per [svebaa's RE analysis](https://svebaa.github.io/personal/blog/shape-security/) and the [`g2asell2019/shape-security-decompiler-toolkit`](https://github.com/g2asell2019/shape-security-decompiler-toolkit) repo, the VM has the following structure:

- **Three-component architecture:** entry block (initializes VM state) → dispatcher (fetches bytecodes, decodes, dispatches to handler) → handlers (execute instruction semantics)
- **~230 instruction handlers** total — split between ~90 atomic instructions (bitwise, stack, register) and ~140 superoperators (composite instructions combining multiple atomics)
- **Rotating instruction set:** approximately 80+ randomly-selected handlers are modified between versions; new VM versions are pushed roughly every **30 minutes**
- **Stack-based CISC** design (per the decompiler toolkit README)
- **Two-file load:** launcher (`ssx.mod.js?async`) + VM file (`ssx.mod.js?seed=<id>`)
- **Bytecode delivery:** the bytecode is embedded in the VM-file source as a string; the launcher fires an event listener that bootstraps execution with encoded parameters

What this design defeats:

- **Static analysis** (the bytecode is opaque without the VM)
- **Naive dynamic re-execution** (the rotating instruction set means a snapshot of decoded behaviour expires within ~30 min)
- **Header replay** (each session has a unique seed → different header values for the same telemetry)
- **DOM hook detection** (Function.prototype.toString is overridden inside the VM; classic stealth checks see the real values)

What this design does **not** defeat (and what we exploit):

- **TLS fingerprint** (Shape sees it before any JS runs — and our `boring2` ClientHello is byte-identical to Chrome's)
- **HTTP/2 SETTINGS / WINDOW_UPDATE / header-frame ordering** (per `23_TLS_HTTP_FINGERPRINT_REFERENCE.md`, also pre-JS)
- **Initial cookie set + request shape** (the doc fetch happens before the VM runs)
- **One-shot reads of non-interactive content** (if the page is fully rendered before the VM's telemetry post fires, BO can read the content even if the eventual telemetry verdict would have been "fail")

For deeper context on the same pattern in other vendors, see `08_KASADA_FRONTIER.md §2` (Kasada `/ips.js` is the closest analogue — same general design, smaller scope) and `26_AKAMAI_BMP_DEEP.md §2.2` (Akamai's `tAD` 58-element TEA-CBC envelope is the historical-encoder analogue).

### 2.3.1 The superpack encoding

Per the [`ranftldieterHub/Shape-Security-protection-reverse-engineering`](https://github.com/ranftldieterHub/Shape-Security-protection-reverse-engineering) repo and [RoundProxies' 2026 bypass analysis](https://roundproxies.com/blog/f5-bypass/), Shape's telemetry payload undergoes a three-stage transform before transmission:

1. **Collection** — VM collects ~35 fingerprint signals (canvas, WebGL, audio, navigator, etc.) plus behavioural counters (mouse moves, key events, scroll, focus changes) into an in-memory structure
2. **Superpack compression** — proprietary compression scheme that exploits common substrings across signal values (similar in spirit to brotli's static dictionary, but with a Shape-specific dictionary baked into the VM)
3. **Encryption** — output is XOR-encrypted with two session-specific seeds (`encryption_seed1`, `encryption_seed2`) derived from the launcher's bundle initialization; a `bundle_seed` (hidden in the bytecode) varies per VM version; a `custom_alphabet` (per-session base64 alphabet permutation) is applied during final encoding

The result is split across 26 custom HTTP headers (`X-<prefix>-{a..z}`), where each header carries one alphabet-letter's worth of the encoded blob. Per the RoundProxies analysis:

> "Custom HTTP headers transmit this data (e.g., `X-DQ7Hy5L1-a`, prefix varies by implementation)."

The implication for any would-be solver: the VM, the bundle_seed, both encryption seeds, the custom alphabet, AND the superpack dictionary must all be extracted from the running VM — and all rotate. This is why the public bypass cadence is so brittle (§6.4).

### 2.3.2 The 30-minute rotation cadence — what it actually rotates

Per svebaa's analysis, the VM publishes a new version "approximately every 30 minutes." What rotates:

| Element | Rotation cadence | Stability boundary |
|---|--:|---|
| Instruction-set handler implementations (~80 of 230) | ~30 min | a lifter built for version N breaks on version N+1 |
| `bundle_seed` (per-VM-version constant) | ~30 min | tied to version; harvested from the bytecode at load |
| `encryption_seed1` / `encryption_seed2` | per-session | reset on every new launcher load |
| `custom_alphabet` (base64 permutation) | per-session | reset on every new launcher load |
| Superpack dictionary | quarterly+ | major-version change; not in the 30-min cycle |
| Telemetry signal list | quarterly+ | new probes added with major releases |
| Header prefix `X-<8-alphanum>-{a..z}` | per-tenant (rare changes) | stable across rotations for a given tenant |

What this means for engineering economics: **a Shape solver maintained at any acceptable success rate requires a continuous-deployment pipeline that re-lifts the VM every 30 minutes.** The public bypass projects (g2asell2019, ranftldieterHub) demonstrate the lifter capability but stop short of operating it as a service — which is why their stars-per-month curve flatlines.

### 2.4 Server-side scoring

The Shape AI Cloud receives telemetry from every protected request. Per F5's marketing, the cloud:

- Scores each session against patterns learned across "thousands of the world's most highly trafficked apps" (cross-tenant ML — your bot signature, learned at Wells Fargo, blocks you at Marriott)
- Generates "sophisticated rulesets" specific to each tenant under domain-expert curation
- Retains the right to issue retroactive verdicts (cookie-poison your session N minutes after the first request, so request N+1 is rejected at the edge)
- Adapts to "attacker retooling" — when a known scraper updates its toolchain, Shape claims to re-classify it within 24-48 hours (per [RoundProxies 2026 analysis](https://roundproxies.com/blog/f5-bypass/))

The "intent vs. automation" claim is what this scoring is doing — a clean technical bypass doesn't help if your IP class + request pattern + telemetry profile matches a campaign that's been previously sanctioned. The implication for us is **per-customer scope of work** — even if BO works on a Shape-protected page once, the customer needs to budget for session-rotation infrastructure (residential proxy pool, fresh-cookie warmup) to stay clean.

---

## 3. Detection markers — recognising Shape in a captured response

This section is the most directly actionable for browser_oxide: what to look for in a captured HTTP response to know "this site is on F5 / Shape." It mirrors the marker tables in `18 §1.1-1.3`.

### 3.1 Response-header markers

Shape is **not** generally header-identifiable in the same way Cloudflare is (no equivalent of `cf-ray`). However:

| Header | Strength | Notes |
|---|---|---|
| `server: BigIP` (sometimes `BIGipServer*` cookie) | weak — confirms F5 BIG-IP, not Shape specifically | Many F5 BIG-IP deployments don't use Shape; many Shape deployments don't expose BIG-IP server header (CDN-fronted). |
| `set-cookie: TS<32-hex-chars>` | strong (Shape-class session cookie family) | F5/Shape uses `TS01<...>` and `TS01<8-hex>=...` for session-state cookies. The `TS01` prefix is the most reliable single signature in our experience. |
| `set-cookie: BIGipServer<pool-name>=...` | strong (F5 BIG-IP origin) | Doesn't *confirm* Shape but raises the prior. |
| `x-akamai-*` *absent* + `cf-ray` *absent* + Shape-class cookies present | strong (rule-out logic) | When a customer-owned domain serves no other CDN/WAF markers but issues `TS01` cookies and runs `ssx.mod.js`, it's Shape. |

### 3.2 Body / DOM markers

| Marker | Strength | Notes |
|---|---|---|
| `/ssx/ssx.mod.js` (script src substring) | **unambiguous** | The classic Shape launcher path. Variants: `?async`, `?seed=<id>`. Customer-tenant subdomains may vary the prefix path. |
| `assets.targetimg1.com/ssx/` | strong | Per [svebaa's analysis](https://svebaa.github.io/personal/blog/shape-security/), Shape's reference deployment domain. |
| `<script ... data-bm-target=...>` | moderate | One pre-acquisition Shape integration convention. |
| `pixel.zycdn.net` / `pixelcdn.zycdn.net` | moderate | F5/Shape's pixel-tracking CDN (post-acquisition). |
| Inline `<script>` with a single ~64-char base64 blob + `eval()` | weak (matches many bootstrap obfuscators) | Shape's launcher inlines a small kickoff blob. Co-signal required. |

### 3.3 Outgoing request markers (telemetry POST recognition)

After the VM bootstraps, the browser begins emitting telemetry. Per the public reverse-engineering work cited above, the telemetry rides as **custom HTTP request headers** with a per-tenant prefix:

| Pattern | Notes |
|---|---|
| Request headers `X-DQ7Hy5L1-a` ... `X-DQ7Hy5L1-z` | The reference prefix from [`ranftldieterHub/Shape-Security-protection-reverse-engineering`](https://github.com/ranftldieterHub/Shape-Security-protection-reverse-engineering). Per-tenant; the `DQ7Hy5L1` portion varies, but the `X-D<8-alphanum>-{a..z}` shape is invariant. |
| `X-<prefix>-a` through `X-<prefix>-z` (26 headers total) | Each contains a different superpack-encoded telemetry chunk. |
| POST body field: superpack-encoded blob, often via a custom MIME or `text/plain` | Some tenants deliver the telemetry via the POST body instead of headers. |

For BO's recognition pipeline, the **single load-bearing marker is `/ssx/ssx.mod.js` in the script chain**. Everything else is a co-signal.

### 3.4 What to add to `classify.rs` (when we onboard a Shape-protected site)

When the first Shape-protected customer site lands in our corpus, the patch is roughly (this is **not implemented today** — it's a forward-looking design):

```rust
// crates/browser/src/classify.rs — proposed §SMALL_BODY additions
"/ssx/ssx.mod.js",            // F5 Shape (launcher) — unambiguous
"assets.targetimg1.com/ssx",  // F5 Shape (reference deployment)
```

And the **vendor-detect logger** in `crates/browser/src/page.rs:1054-1069` would add:

```rust
// Header-based detection (in send_request / response inspection)
if response.cookies().any(|c| c.name().starts_with("TS01")) {
    log_vendor_marker("f5_shape", "TS01-cookie", ...);
}
if response.headers().get_all("set-cookie")
    .iter().any(|v| v.to_str().unwrap_or("").contains("BIGipServer")) {
    log_vendor_marker("f5_bigip", "BIGipServer-cookie", ...);
}
```

The classifier verdict body-marker is **less load-bearing** for Shape than for DataDome / Cloudflare because Shape doesn't typically serve a "challenge page" — it scores in the background and either passes the real content or silently shadow-bans. The recognition is for *posture* (do we expect Shape behaviour here?) more than for *verdict* (did the request get challenged?).

---

## 4. Mechanism per signal collection

What the Shape VM actually collects, per public reverse-engineering work + F5's own positioning:

### 4.1 Canvas + WebGL fingerprinting

- `<canvas>.toDataURL()` after rendering a vendor-specific pattern (Shape uses a different test pattern than the public-domain `fp-collect` / `FingerprintJS` patterns — confirmed in pre-2020 RE work).
- WebGL `UNMASKED_VENDOR_WEBGL` (`WEBGL_debug_renderer_info` extension) and `UNMASKED_RENDERER_WEBGL`.
- WebGL parameter dumps: `MAX_TEXTURE_SIZE`, `MAX_VIEWPORT_DIMS`, `MAX_VERTEX_UNIFORM_VECTORS`, etc. (the `~35 data elements in randomized order` from the `ranftldieterHub` repo synthesis).

**BO coverage:** canvas surface lives in `crates/canvas/src/`; WebGL surface in `crates/canvas/src/webgl_render.rs`. Per `16_STEALTH_FINGERPRINT_AUDIT.md §3-5`, our canvas and WebGL profiles are byte-perfect-Chrome (chrome_148 profile) — they pass FingerprintJS-grade checks. **Status against Shape: unknown** (no live capture), but the same primitive that passes Akamai/DataDome/Kasada canvas-tracking will almost certainly pass Shape canvas-tracking. The unknown is whether Shape's test pattern catches a subtler quirk that the standard FP-collect patterns miss.

### 4.2 Audio context fingerprinting

- `AudioContext.createOscillator().getChannelData()` → SHA hash of the oscillator output (the FingerprintJS audio-FP technique).
- `DynamicsCompressorNode` output sampling (the Akamai-historical signal — see `26 §2.3`).

**BO coverage:** `crates/canvas/src/audio_*.rs` and the audio worklet hooks. Per `16 §6`, we pass standard audio-FP probes. Same caveat as canvas: untested against Shape's specific patterns.

### 4.3 Navigator and screen surface

- `navigator.{userAgent, platform, vendor, hardwareConcurrency, deviceMemory, languages, plugins, mimeTypes, webdriver}`
- `screen.{width, height, availWidth, availHeight, colorDepth, pixelDepth}`
- `window.{outerWidth, outerHeight, innerWidth, innerHeight, devicePixelRatio}`

**BO coverage:** all of these are profile-driven via `StealthProfile` (see `crates/stealth/profiles/chrome_148_*.yaml`). Per `16 §1`, they are byte-perfect-Chrome.

### 4.4 Behavioral signals (mouse, key, touch, scroll)

- `mousemove` / `click` / `keydown` / `keyup` / `touchstart` / `scroll` event taps from VM bootstrap until telemetry POST.
- Per RE work: timestamp deltas, x/y trajectories, velocity, acceleration, dwell times.
- The Shape VM also collects `focus` / `blur` / `visibilitychange` / `pagehide` events.

**BO coverage:** `crates/browser/src/js/humanize.js` synthesises mouse motion (Bezier paths between targets) and the timing-jitter primitives. **For a one-shot page render this surface emits very little behaviour** — which Shape will flag as "no mouse activity = automation prior." BO's humanizer is designed for explicit user-action scenarios (form-fill, click), not for pure-render scenarios. **This is a gap** that mirrors the Kasada "/tl" telemetry gap from `08_KASADA_FRONTIER.md` — same root cause (no synthetic user-presence for non-interactive flows), different vendor.

### 4.5 Automation-flag scrubs

- `navigator.webdriver` (false-positive on Playwright/Puppeteer; we set to `undefined`)
- CDP banner detection (`Runtime.enable` / `Debugger.enable` traces — see RoundProxies analysis)
- DevTools-open heuristics (window-dimension differences, `console.log` hooks)
- Function.prototype.toString tampering checks (our chrome-compat shim handles this — see `crates/js_runtime/src/chrome_compat.rs`)

**BO coverage:** we are CDP-free by design (no CDP server is attached to the V8 isolate in production paths — only `crates/browser/examples/*` opens DevTools). `webdriver` flag is `undefined`. Function.prototype.toString returns native-bracketed source (verified in 437/437 chrome_compat tests). **Status against Shape: should pass these probes** based on parity with Kasada/Akamai equivalents.

### 4.5.1 The full ~35-signal list (per public RE work)

The [`ranftldieterHub` repo](https://github.com/ranftldieterHub/Shape-Security-protection-reverse-engineering) and [svebaa's writeup](https://svebaa.github.io/personal/blog/shape-security/) together let us assemble a probable signal inventory. Each row maps to a BO surface we already cover (or a gap we'd need to close):

| # | Signal | Surface | BO surface | Status |
|--:|---|---|---|---|
| 1 | `navigator.userAgent` | navigator | StealthProfile UA string | clears |
| 2 | `navigator.platform` | navigator | StealthProfile platform | clears |
| 3 | `navigator.vendor` | navigator | StealthProfile vendor | clears |
| 4 | `navigator.hardwareConcurrency` | navigator | StealthProfile (4/8/16) | clears |
| 5 | `navigator.deviceMemory` | navigator | StealthProfile (4/8) | clears |
| 6 | `navigator.languages` | navigator | StealthProfile languages | clears |
| 7 | `navigator.plugins.length` + names | navigator | chrome_compat Plugin list | clears |
| 8 | `navigator.mimeTypes.length` + names | navigator | chrome_compat MimeType list | clears |
| 9 | `navigator.webdriver` | navigator | undefined (chrome_compat) | clears |
| 10 | `navigator.doNotTrack` | navigator | StealthProfile DNT | clears |
| 11 | `navigator.cookieEnabled` | navigator | always true | clears |
| 12 | `navigator.maxTouchPoints` | navigator | StealthProfile (0/5/10) | clears |
| 13 | `navigator.productSub` | navigator | "20030107" (Chrome constant) | clears |
| 14 | `screen.{width, height, availWidth, availHeight}` | screen | StealthProfile screen dims | clears |
| 15 | `screen.{colorDepth, pixelDepth, orientation.type}` | screen | StealthProfile | clears |
| 16 | `window.devicePixelRatio` | window | StealthProfile DPR | clears |
| 17 | `window.{innerWidth, innerHeight, outerWidth, outerHeight}` | window | StealthProfile viewport | clears |
| 18 | `Intl.DateTimeFormat().resolvedOptions().timeZone` | Intl | StealthProfile TZ | clears |
| 19 | `Date.prototype.getTimezoneOffset()` | Date | derived from TZ | clears |
| 20 | Canvas FP (custom test pattern → toDataURL hash) | canvas | `crates/canvas/src/` chrome-148 path | clears (probably) |
| 21 | WebGL `UNMASKED_VENDOR_WEBGL` | webgl | StealthProfile WebGL vendor | clears |
| 22 | WebGL `UNMASKED_RENDERER_WEBGL` | webgl | StealthProfile WebGL renderer | clears |
| 23 | WebGL parameter dump (~50 params) | webgl | `crates/canvas/src/webgl_render.rs` | clears (probably) |
| 24 | WebGL extension list | webgl | StealthProfile extension list | clears |
| 25 | AudioContext FP (oscillator hash) | webaudio | `crates/canvas/src/audio_*.rs` | clears |
| 26 | `DynamicsCompressorNode` FP | webaudio | chrome-148 audio path | clears |
| 27 | Font enumeration (CSS test against known fonts) | DOM | font list per profile | clears |
| 28 | CSS feature support (`@supports`, properties) | DOM | matches Chrome 148 surface | clears |
| 29 | Mouse-move event counter (since VM bootstrap) | events | none (no synthetic activity) | **gap** |
| 30 | Mouse-move x/y trajectory samples | events | none | **gap** |
| 31 | Key-event counter + timing | events | none | **gap** for non-form flows |
| 32 | Scroll-event counter + delta | events | none | **gap** |
| 33 | Focus / blur / visibilitychange events | events | not synthesized | **gap** |
| 34 | `chrome.runtime` and other extension-API probes | JS env | chrome_compat shim returns realistic values | clears |
| 35 | `Function.prototype.toString` integrity (anti-hook check) | JS env | chrome_compat returns native source | clears |

**Headline:** ~28 of ~35 signals clear without any active work; 5-7 of them are behavioural events that emit zero under our default page-render path. Whether the 5-7 missing signals are individually load-bearing or whether they aggregate to a threshold is the unanswered question (see §9.4 — captured A/B against a known Shape-protected site is the only way to know).

### 4.6 Network / IP / TLS class signals (server-side, not in JS)

- JA3 / JA4 TLS fingerprint
- HTTP/2 SETTINGS frame contents + WINDOW_UPDATE patterns
- HTTP header order
- ASN class (datacenter vs residential)
- IP age + first-seen
- ASN history (has this ASN been associated with known scraper campaigns?)

**BO coverage:** TLS fingerprint via `crates/net/src/tls.rs` + `boring2` (per `23_TLS_HTTP_FINGERPRINT_REFERENCE.md`) is byte-identical to Chrome 148. HTTP/2 SETTINGS, header order, frame ordering all match Chrome. **What we don't control:** ASN/IP class — that's customer infrastructure. The honest onboarding statement is "BO clears the TLS/HTTP-2/header-class gate; the IP-class gate is your problem (residential proxy if needed)."

### 4.7 Summary table — BO coverage of Shape signals

| Signal class | BO coverage | Confidence |
|---|---|---|
| TLS / JA3 / JA4 / HTTP-2 | **clears** | high — same primitive that clears all other vendors |
| Canvas / WebGL / WebAudio | **clears (probably)** | medium — clears standard FP probes; Shape's specific patterns untested |
| Navigator / screen | **clears** | high — profile-driven, byte-perfect-Chrome |
| Automation flags / CDP scrubs | **clears** | high — CDP-free, webdriver `undefined`, native `toString` |
| Behavioral (mouse/key/scroll) | **gap** for pure-render flows | high — no synthetic behaviour without explicit humanize hooks |
| ASN / IP / cookie-volume class | **out of scope** (customer infrastructure) | n/a — same as every other vendor |
| Rotating-VM payload integrity | **gap** | high — no Shape-specific encoder in `vendor_solvers`, no plan to add one in v0.1.0 |

### 4.8 The tier-1 vs tier-2 separability — why the TLS-class gate matters disproportionately

A useful structural observation: Shape's two-tier stack (TLS-class first-pass + VM-encoded telemetry second-pass) **gates the second pass behind the first.** A request that fails the TLS check is rejected (or shadow-banned) before the VM telemetry is even consulted; the server returns either a Shape challenge document or a deceptive "real" body that doesn't gate on a telemetry verdict.

RoundProxies' "TLS alone blocks 80%+ of early attempts" claim (per [their 2026 analysis](https://roundproxies.com/blog/f5-bypass/)) is consistent with the public reverse-engineering work and consistent with what we see in our own corpus on Cloudflare and Akamai: the load-bearing pre-VM check is TLS fingerprint + HTTP/2 SETTINGS + IP-class.

For BO specifically:

| Layer | What Shape checks | BO primitive | BO clears? |
|---|---|---|---|
| TLS ClientHello cipher list + extensions order | JA3 hash | `crates/net/src/tls.rs` boring2 ClientHello (Chrome 148 byte-for-byte) | yes — verified in `23 §2` |
| TLS extension content (GREASE, ALPN, signature-algorithms) | JA4 hash | same | yes — verified in `23 §2` |
| HTTP/2 SETTINGS frame | SETTINGS values + ordering | `crates/net/src/h2.rs` (Chrome SETTINGS profile) | yes — verified in `23 §3` |
| HTTP/2 first WINDOW_UPDATE / PRIORITY frames | Frame ordering signature | same | yes |
| HTTP header order in the request line | Pseudo-header + regular-header order | `crates/net/src/h2.rs` header encoding | yes — Chrome-class order |
| ASN class | Datacenter vs residential vs mobile | customer-provided IP infrastructure | **out of scope** |
| IP age / first-seen | Time since IP first observed | customer-provided IP infrastructure | **out of scope** |

Critically: even if BO's VM-tier behaviour is imperfect (gap §4.4 — no synthetic mouse activity), the TLS-tier pass is so heavily-weighted in Shape's scoring that we expect a meaningful pass rate against passive Shape deployments **from a clean IP** without any solver work. The asymmetry is exactly the asymmetry that lets us pass Cloudflare Managed on chrome/pixel/firefox (per `27 §1`): Shape's structural bias toward TLS class first means a real-Chrome-class TLS engine is over-credited relative to its actual JS surface fidelity.

This is good news with two caveats:

1. **From a known-bot ASN**, the tier-1 gate is configured to flag-on-IP, so TLS-class doesn't carry; the second-tier ML then decides. This is where we'd lose.
2. **Sustained sessions** trigger the retroactive ML re-scoring (§2.4). The TLS-pass is a one-time credit, not a session-spanning credit. So the "BO passes Shape passive" claim should always be qualified as "for a one-shot fresh-session request."

---

## 5. BO coverage — what we'd see today on a Shape-protected site (speculative)

Because no Shape-protected site is in the 126-corpus today, this section is the **honest hypothesis** that a customer should be told upfront. It's calibrated against parity vendors (Kasada, Akamai BMP) where we DO have data.

### 5.1 Likely outcomes by request class

| Request class | Expected outcome | Hypothesis rationale |
|---|---|---|
| One-shot GET of a marketing / content page (no Shape-gated content) | **passes (≥80% confidence)** | TLS-class first-pass succeeds; rotating-VM telemetry irrelevant because the page doesn't gate on a verdict. Same pattern as our 102-site one-shot passes on the broader corpus. |
| One-shot GET of a Shape-gated page (login wall, auth-only content) | **mixed** | Page loads, content renders (depending on whether the Shape JS blocks the doc render — typically it doesn't), but a downstream form submit will fail. For a customer who needs to read the public page chrome but not submit, BO works; for a customer who needs to log in, BO does not. |
| POST / form submit behind Shape | **fails (≥80% confidence)** | The X-D*-{a..z} headers are not generated; Shape's server side recognises their absence and rejects. |
| Sustained scrape session (50+ requests on the same cookie / IP) | **fails (≥90% confidence)** | Shape's network-effect ML scores cross-request patterns; even a clean-on-request-1 client gets retroactively poisoned. |

### 5.2 Sites in our 126-corpus that are *plausibly* Shape-protected

We have not confirmed any of these by capture; they are listed as candidates for a future spot-check:

| Site | Why suspected | Current BO result | Implication if Shape-confirmed |
|---|---|---|---|
| `wellsfargo.com` | Top-tier US bank — Shape's traditional customer base | passes 4/4 profiles per `01 §2` | Either not on Shape, or Shape's first-pass clears BO's TLS+headers without VM escalation |
| `southwest.com` (not in corpus) | Public Shape case study | — | Onboarding candidate |
| `marriott.com` (not in corpus) | Post-2018-hack Shape deployment | — | Onboarding candidate |
| `usbank.com` / `bankofamerica.com` (not in corpus) | Top-10 US bank | — | Onboarding candidate |

If `wellsfargo` is on Shape and BO passes, the reading is "BO clears Shape's passive layer." That's a strong (if anecdotal) data point and worth a one-time targeted capture (the `crates/browser/examples/sweep_metrics.rs --site wellsfargo.com --capture-headers` command on a sweep run logs every X-* header to disk; per `04_TOOLING_SPEC.md §3.2`).

### 5.3 What a "we support Shape" claim would actually require

To claim Shape support honestly in marketing material, we'd need:

1. **Recognition** — `/ssx/ssx.mod.js` body marker in `classify.rs`, `TS01*` cookie sniffing in vendor-detect logger (~50 LOC). **Cheap.**
2. **Live A/B capture** against 3-5 Shape-protected sites with both BO and a control (Camoufox + Patchright). Sites: pick from §5.2 + customer-supplied. **Per-site: ~30 min.**
3. **Per-profile measurement** across chrome / pixel / iphone / firefox stealth profiles. **Standard sweep tooling.**
4. **Documented pass rate** with the honest disclaimer about retroactive ML scoring. **Documentation work.**
5. **Optional: a Shape solver in `vendor_solvers`** — would require ~3 months of senior RE work for the VM + ~1 month for the superpack codec + ongoing tracking of the 30-min rotation. **Not justified by v0.1.0.**

The (1)+(2)+(3)+(4) package is **2-3 days of work** and gives us a defensible "we recognise Shape and the passive layer clears" story, which is enough for onboarding-conversations. (5) is a separate product decision driven by customer revenue.

---

## 6. Public solver / bypass landscape

Shape has the **smallest** open-source bypass community of any major commercial anti-bot vendor — this is the inverse of Cloudflare (which has dozens of public bypass projects, see `25 §6`) and inverse of Kasada (which has [`b3rsec/kasada-bypass`](https://github.com/b3rsec/kasada-bypass) and friends, see `08 §4`). The reason: Shape is sold to enterprises that pursue legal remedies against scrapers, and the technical bar (VM with rotating opcodes) is genuinely high.

What public material exists:

### 6.1 Reverse-engineering writeups

- **[svebaa: "Dissecting Shape Security's Virtual Machine"](https://svebaa.github.io/personal/blog/shape-security/)** — the canonical public technical writeup. Covers entry block / dispatcher / handlers + the rotating-instruction-set design. Does **not** publish a working solver. Reads as research-grade documentation of *what Shape is*, not a recipe for bypass.
- **[ranftldieterHub/Shape-Security-protection-reverse-engineering](https://github.com/ranftldieterHub/Shape-Security-protection-reverse-engineering)** — a Node.js emulator that goes further: decrypts obfuscated strings, emulates a browser env, generates the `X-DQ7Hy5L1-a` ... `X-DQ7Hy5L1-z` request headers, uses worker threads for CPU-intensive payload generation. **Working solver for a specific Shape version**. Stability across the 30-min rotation: unverified and almost certainly poor.
- **[g2asell2019/shape-security-decompiler-toolkit](https://github.com/g2asell2019/shape-security-decompiler-toolkit)** — a dynamic deobfuscator: injects a tracer into the VM, traces all executed opcodes, lifts the trace back to JavaScript. Methodological — produces analysable JS from the VM, not a bypass per se. Author notes "self-defending code and tamper checks" limit the lifter.

### 6.2 Commercial bypass services

- **None public.** Unlike FunCaptcha (chapter 30) or AWS-WAF or Akamai BMP, there's no `capsolver.com`-class commercial endpoint for Shape (or if there is, it's not publicly indexed by the major SEO terms). The economics are the wrong way: enterprise legal pressure on commercial bypass providers + technical difficulty of staying on top of the rotation.

### 6.3 Industry analyses (vendor-neutral)

- **[RoundProxies "How to bypass Shape (F5) antibot in 2026"](https://roundproxies.com/blog/f5-bypass/)** — practical bypass recipes (curl_cffi + TLS impersonation, Nodriver for CDP-free Chrome, undetected_chromedriver). Honest about the 30-min rotation eating bypass durability. Claims 60-70% success on "Shape-protected endpoints" using the combo.
- **[ZenRows "How to Bypass F5 Antibot in 2026"](https://www.zenrows.com/blog/bypass-f5)** — vendor-pitch for the ZenRows commercial proxy product; useful for the markers list at the top, less useful for technical depth.

### 6.4 Honest assessment

A from-scratch Shape solver inside `vendor_solvers` would be:

- **3-6 months of senior reverse-engineering work** to build the VM emulator + opcode tracer + superpack codec
- **Ongoing maintenance** ~10-20 hours/month to track the 30-min rotation (most of which is automated once the lifter is solid, but breakage on opcode changes still requires manual patching)
- **Unclear ROI** absent a customer with confirmed Shape-protected revenue
- **Legal risk surface** — F5 actively pursues anti-bot bypass projects; the public bypass ecosystem's size is downstream of this

The honest v0.1.0 posture: **document recognition, document the passive-layer pass story, decline to ship a Shape solver until customer revenue justifies it.** This mirrors our posture on FunCaptcha (chapter 30): vendor-class is in scope for recognition and customer onboarding, vendor-class encoders are out of scope for the public engine and out of scope for the v0.1.0 `vendor_solvers`.

---

## 7. Customer onboarding playbook

This section is meant to be lifted into a customer-facing playbook (with the speculative qualifiers removed and the marketing language tightened).

### 7.1 Step 1 — confirm vendor

The customer says "site X uses some bot defense." Confirm Shape with three checks:

1. **DNS / network** — `dig <site>` or `curl -v https://<site>` and look for any of: `ssx.mod.js` in the response body, `TS01*` cookies in `set-cookie`, the `BIGipServer*` cookie family. If two of three, it's Shape.
2. **Browser DevTools** — load the page in Chrome, look for a script-src to `/ssx/ssx.mod.js` (any path; the launcher path is the giveaway). Confirmed if present.
3. **F5 customer-mention** — F5's own [marketing site](https://www.f5.com/customers) and trade-press coverage will sometimes name customers. Not reliable as the sole signal but useful as confirmation.

### 7.2 Step 2 — characterise the customer's use case

The customer's use case maps to a likely outcome per §5.1. Three buckets:

| Use case | Likely BO outcome | Customer guidance |
|---|---|---|
| Public-page render (price scraping, content monitoring, news aggregation) | Likely passes the first-pass; sustained-session ML is the risk | "BO renders the page. For volume > 100 req/day, plan for residential proxy rotation and session-cookie hygiene." |
| Auth-flow (login submit, password-reset, account creation) | Likely fails the telemetry post | "Shape protects the auth flow specifically. BO can render the login page but the submit will be blocked. You'd need a Shape solver (not in v0.1.0) or session-handoff after manual login." |
| API behind login + Shape | Likely fails | "If the API itself emits the X-D* headers, you'd need the solver. If the API accepts a session cookie issued after a Shape-cleared login (done out-of-band), BO can carry that cookie." |

### 7.3 Step 3 — capture-and-verify

Even with the above heuristics, run a 5-request capture on the customer's actual target page:

```bash
cargo run --release --example sweep_metrics -- \
    --site <customer-site> \
    --profile chrome,pixel,iphone,firefox \
    --capture-headers \
    --capture-cookies \
    --reps 5 \
    --output /tmp/shape_capture/
```

Output: per-request body size, classifier verdict, set-cookie observations, response headers (including any X-* prefixes — we don't decode them, but their presence is itself a signal). Per-profile pass-rate over 5 reps tells the customer whether routing across our 4 stealth profiles materially helps.

### 7.4 Step 4 — the honest pitch line

> "browser_oxide clears the passive layer of F5 Distributed Cloud Bot Defense (TLS class, HTTP/2 fingerprint, header order, JS environment surface). One-shot page renders typically succeed. We do not currently ship an active-layer solver for Shape's rotating-VM telemetry — auth-flow submits and high-volume sustained sessions are out of scope until a customer-funded solver lands in our private `vendor_solvers` companion."

This is the language we should use until §7.3 captures from real customer sites give us actual pass-rate numbers to quote.

---

## 8. Forward-looking — F5's roadmap and convergence

F5's strategic direction (per their [2026 Distributed Cloud product announcements](https://www.f5.com/products/distributed-cloud-services) and analyst commentary):

### 8.1 Convergence with WAAS/SaaS

F5 is consolidating its product portfolio under the "Distributed Cloud" SaaS banner — Shape (bot defense) joins:
- F5 Distributed Cloud WAAP (web app + API protection)
- F5 Distributed Cloud DDoS Defense
- F5 Distributed Cloud App Connect
- F5 Distributed Cloud CDN

For us, this means a single Shape-protected customer might increasingly *also* be on F5 WAAP — the same `BIGipServer*` cookie family, with the WAAP layer adding its own challenge response (potentially closer to the Cloudflare-style explicit interstitial than the current Shape-style background scoring). Our recognition rules should keep `BIGipServer*` as a soft signal and `/ssx/ssx.mod.js` as the hard Shape-specific signal.

### 8.2 AI-resilient roadmap

F5's 2025-2026 messaging emphasizes "AI-driven attack" defense — the implicit acknowledgement that LLM-driven scraping (browser-use, claude-computer-use, etc.) is now a first-class adversary class. The product roadmap suggests:

- More aggressive ML cross-tenant signal-sharing
- Behavioral signals reweighted (an LLM-driven agent has different mouse patterns than a Selenium script)
- More server-side decisioning (less weight on JS-VM telemetry; more weight on request-pattern + IP-class)

For us, this is **good news** — server-side decisioning is more attackable by a clean-TLS + clean-IP engine like ours than by a CDP-detectable one like vanilla Playwright. BO's structural wins on Cloudflare and PerimeterX (per `27 §2`) suggest the same logic applies forward to Shape.

### 8.3 Mobile-app side

The Shape Mobile SDK is **out of scope** for the browser engine. If a customer needs Shape-protected mobile API access, that's a mobile-instrumentation problem (Frida, app re-signing, attestation-bypass) and not a browser-rendering problem. We should be unambiguous about this when scoped.

### 8.4 Convergence with Arkose Labs (chapter 30)?

Speculative — there have been intermittent industry rumours of F5 / Arkose strategic alignment (Arkose's invisible-challenge product is complementary to Shape's invisible-scoring product). No public deal as of 2026-05; F5 and Arkose remain independent. If they merge, the recognition logic in §3 would extend to cover Arkose's `client-api.arkoselabs.com` iframe family too.

---

### 8.5 Cross-vendor pattern synthesis — what Shape reveals about anti-bot architecture

Shape isn't just an isolated vendor; it's the historical archetype that other vendors have variously copied, simplified, or extended. Useful to lay out the family tree because it informs what BO primitives are doing double duty.

### 8.5.1 The "JS-VM with rotating opcodes" lineage

| Vendor | First shipped VM | Approximate VM cadence | Telemetry transport |
|---|---|---|---|
| **Shape Security** | 2013-ish (per [Shape engineering blog](https://blog.shapesecurity.com/category/shape-engineering/)) | 30-minute rotation | 26 custom request headers (`X-<prefix>-{a..z}`) |
| **Kasada** | 2018 (per Kasada launch announcements) | per-deploy (less frequent — maybe weekly) | `x-kpsdk-ct` / `x-kpsdk-cd` request headers + `/tl` POST |
| **Akamai BMP v3** (post-2024) | 2024 | per-tenant (irregular) | POST body, PRNG-shuffled JSON keyed by `bm_sz`-derived cookieHash |
| **DataDome WASM** | 2023+ | per-day (daily key rotation) | iframe-served WASM evaluates a daily key + posts back |
| **AWS WAF Challenge** | 2022 | per-month (string-array seed rotation) | POST to `/challenge` with `getToken()` |

Shape's design choices set the template that subsequent vendors picked from:

- **Custom VM with proprietary bytecode** — defeats static analysis (Shape's contribution; mirrored by Kasada, partly Akamai v3, less so DataDome and AWS WAF)
- **Per-session encryption keys derived from a VM-internal seed** — defeats header replay (Shape; mirrored by Kasada `/tl`, Akamai v3 cookieHash, DataDome daily key)
- **Behavioural telemetry weighted heavily** — defeats clean-fingerprint-but-no-mouse (Shape; mirrored by every modern vendor)
- **Server-side network-effect ML** — defeats per-request cleanliness (Shape; mirrored by Kasada cross-tenant correlation, Akamai bot-pattern ML)

What Shape pioneered that **not** every successor adopted:

- **Per-tenant header-prefix obfuscation** (`X-DQ7Hy5L1-{a..z}`) — Kasada uses fixed header names (`x-kpsdk-*`); Akamai uses POST body; DataDome uses iframe-internal cookies. Shape's per-tenant prefix is the most operationally hostile to a generic solver: a Shape solver needs per-tenant configuration.
- **Aggressive opcode rotation (~30 min)** — Kasada rotates much less frequently; Akamai's v3 envelope is closer to per-quarter. Shape's cadence makes solver maintenance the dominant cost.

### 8.5.2 The BO primitives reused across the family

Per `aecdf19`, BO's public engine keeps a small set of **vendor-neutral primitives** that any private solver consumes. The primitives that apply directly to a hypothetical Shape solver (from `07 §Primitives 1-3`, `25 §4.4`, and `26 §3.A-C`):

| Primitive | Origin chapter | Applicability to Shape |
|---|---|---|
| Challenge-doc CSP relaxation | `07 §Primitive 1` | useful — Shape challenge documents may carry restrictive CSP; we'd want to relax it so the VM-loaded iframe can mount |
| Cross-origin challenge-iframe materialization | `07 §Primitive 2` | applicable — if Shape escalates to an interactive challenge (rare), the iframe primitive lets us mount it |
| Solved-cookie retry | `07 §Primitive 3` | applicable — Shape sets `TS01<...>` cookies on pass; the retry loop should learn those just as it learns `_abck` (Akamai) and `cf_clearance` (Cloudflare) cookies |
| `_abck`-state recognition pattern | `26 §3.A` | extensible — same state-machine primitive applies to `TS01<...>` cookie state (uncleared vs verified) |
| Persistent `started_as_*_challenge` flag | `26 §3.B` | extensible — add `started_as_shape_challenge` body-marker flag (detection on `/ssx/ssx.mod.js` presence + `TS01<8-hex>` cookie absence) |
| Vendor-marker logger | `26 §3.C` | extensible — already drafted per §3.4 above |
| Turnstile token relay | `25 §4.4` | not applicable — Shape doesn't have an interactive token-relay step; the relay primitive is Cloudflare-Turnstile-specific |

The point: when (and if) we onboard a Shape-protected customer, we are NOT building from scratch. The five DataDome / Cloudflare / Akamai primitives that landed across `07 / 25 / 26` already handle the engine-side seam — what remains for a Shape solver in `vendor_solvers` is purely the VM lift + payload encoder, which is hard but bounded.

### 8.5.3 Where Shape sits in our competitive pitch (per chapter 27)

Cross-link to `27 §1.1`: Shape would land as a new row in the vendor-cluster totals table. Honest expected numbers (informed by §5.1):

| Vendor cluster | Sites | BO routed wins | Camoufox wins | Patchright wins | Playwright wins |
|---|--:|--:|--:|--:|--:|
| **F5 / Shape (passive, fresh IP, page render)** | — (none in corpus today) | ~0-2 of ~3 onboarded | likely 0-1 | likely 0 | likely 0 |
| **F5 / Shape (active, form submit, sustained session)** | — | 0 | 0 | 0 | 0 |

The first row is the pitchable case: BO + Camoufox both clear the passive layer (real-Chrome-class TLS for BO, real-Firefox-class TLS for Camoufox); Patchright/Playwright don't because CDP fingerprints leak. The second row is universally negative — no public-engine solves Shape's active path.

This is the same shape as the AWS WAF Challenge row in `27 §1.1`: BO and Camoufox split the passive cluster, Patchright/Playwright dominate via real-Chrome trust, no public engine solves the encoder.

---

## 9. Acceptance + files

### 9.1 What "v0.1.0 supports Shape" means (acceptance bar)

| Acceptance item | Status today | v0.1.0 target |
|---|---|---|
| Recognise Shape in `classify.rs` body markers | absent | **add** `/ssx/ssx.mod.js` (1 line) |
| Recognise Shape in vendor-detect cookie logger | absent | **add** `TS01*` cookie family (5 lines) |
| Recognise Shape in vendor-detect header logger | absent | **add** `BIGipServer*` cookie + `server: BigIP` header (5 lines) |
| One spot-check capture against a Shape-protected site | none | **run** `sweep_metrics` against `wellsfargo` + 1-2 customer-supplied sites |
| Documented pass-rate disclaimer in customer-facing copy | absent | **publish** §7.4 language |
| Active solver (`vendor_solvers/src/shape.rs`) | absent | **decline** for v0.1.0 (out of scope per §6.4) |

The acceptance bar is **recognition + capture + honest customer-facing language** — not a working solver. The decision to defer the solver is a product call: the engineering cost (§6.4) is high and the corpus contains zero Shape-confirmed sites today.

### 9.2 Files to touch (when we onboard the first Shape customer)

| File | Change | LOC | Reason |
|---|---|---|---|
| `crates/browser/src/classify.rs` | add `/ssx/ssx.mod.js` and `ssx.mod.js?seed=` SMALL_BODY markers | ~2 | recognition |
| `crates/browser/src/page.rs:1054-1069` | extend vendor-detect logger for `TS01*` cookie + `BIGipServer*` cookie + `server: BigIP` | ~10 | observability |
| `crates/browser/src/page.rs:2287-2293` | extend `v8_html_is_real` guard list to include `ssx.mod.js` | ~1 | prevent V8 from accepting a Shape stub as "real content" |
| `docs/releases/v0.1.0-parity/18_ANTI_BOT_VENDOR_COOKBOOK.md` | add §2.13 "F5 Distributed Cloud Bot Defense (Shape)" entry mirroring the §2.10b Arkose entry | ~30 | cookbook coverage |
| `docs/releases/v0.1.0-parity/27_VENDOR_COMPETITIVE_MATRIX.md` §1 | add row for F5/Shape with the per-engine pass counts (once we have capture data) | ~3 | competitive transparency |
| `docs/releases/v0.1.0-parity/29_F5_SHAPE_SECURITY.md` | this file (already authored) | — | reference |

### 9.3 Files NOT to touch (declared out of scope)

| File | Why not |
|---|---|
| `vendor_solvers/src/shape.rs` (would-be) | §6.4 — solver work is post-v0.1.0 + customer-funded |
| `crates/stealth/profiles/chrome_148_*.yaml` | Shape doesn't expose a new fingerprint surface we're missing; chrome_148 already covers it |
| `crates/canvas/src/*.rs` | Same — canvas / WebGL surface that passes Akamai also passes Shape |

### 9.4 Open questions logged for future investigation

1. **Does `wellsfargo` actually use Shape?** A targeted capture is ~5 minutes; doing it would convert the "speculative" of §5.2 into a concrete data point.
2. **Does Shape's body-marker presence (`/ssx/ssx.mod.js`) correlate with a measurable change in BO's pass rate?** A "before / after" sweep on a Shape-suspected vs. Shape-confirmed-absent control set would test this.
3. **How does Shape's challenge serve compare to Cloudflare Managed?** Both are silent-score-then-block; CF Managed serves an explicit interstitial on fail, Shape (per the public docs) typically shadow-bans or serves benign-but-empty content. Need a captured fail to confirm.
4. **What is the actual cardinality of Shape-protected sites in the global top-1k?** Best public estimate is ~3-5% per [Built With](https://trends.builtwith.com); if true, that's ~30-50 sites in a top-1k corpus and we should consider adding 3-5 to the 126-corpus for v0.2.

### 9.5 Cross-references

- `18_ANTI_BOT_VENDOR_COOKBOOK.md` — needs the new §2.13 entry per §9.2 above
- `27_VENDOR_COMPETITIVE_MATRIX.md` — Shape cluster is currently absent; will become a row once we have capture data
- `25_CLOUDFLARE_DEEP.md §0` and `26_AKAMAI_BMP_DEEP.md §2` — analogous two-tier-vendor patterns
- `08_KASADA_FRONTIER.md` — closest public-corpus analogue (Kasada's `/ips.js` VM is the same idea, 5 years younger and narrower in scope)
- `23_TLS_HTTP_FINGERPRINT_REFERENCE.md` — the load-bearing primitive that Shape sees before any JS runs
- `16_STEALTH_FINGERPRINT_AUDIT.md` — the JS-surface checklist that Shape's VM probes
- `30_ARKOSE_LABS.md` — sibling chapter for Arkose / FunCaptcha (similar status: out-of-corpus, customer-onboarding reference)

---

## 10. Sources

The technical content above is synthesized from these public sources (in order of load-bearing-ness):

1. [F5 Distributed Cloud Bot Defense product page](https://www.f5.com/products/distributed-cloud-services/bot-defense) — current product positioning, deployment modes
2. [F5 / Shape Security acquisition announcement](https://www.f5.com/company/blog/announcing-shape-security-acquisition) — \$1B, January 2020 close, retained CEO + 370 employees
3. [GeekWire 2019-12-19 coverage of the acquisition](https://www.geekwire.com/2019/f5-networks-will-acquire-shape-security-1b-bolster-online-fraud-protection-services/) — financial and historical context
4. [F5 press release](https://www.f5.com/company/news/press-releases/f5-to-acquire-shape-security) — official deal terms
5. [F5 SEC 8-K filings](https://www.sec.gov/Archives/edgar/data/0001048695/000114036120001429/nc10007101x3_ex99-1.htm) — formal acquisition disclosure
6. [F5 Hybrid Security Architectures Knowledge Base](https://community.f5.com/kb/technicalarticles/f5-hybrid-security-architectures-part-4---f5-xc-bot-and-ddos-defense-and-big-ip-/312342) — deployment topology specifics
7. [F5 BIG-IP iApp integration guide](https://community.f5.com/kb/technicalarticles/how-to-easily-protect-your-big-ip-applications-using-f5s-distributed-cloud-bot-d/295578) — on-prem deployment details
8. [F5 Distributed Cloud Bot Defense technical docs](https://docs.cloud.f5.com/docs-v2/bot-defense/concepts/about-bot-defense) — official capability documentation
9. [F5 Distributed Cloud Bot Defense solution overview PDF](https://www.f5.com/pdf/solution-profiles/f5-distributed-cloud-bot-defense-solution-overview.pdf) — official solution-brief copy
10. [F5 Distributed Cloud Bot Defense on AWS Marketplace](https://aws.amazon.com/marketplace/pp/prodview-x5mf4isftlzcc) — AWS-tenant deployment context
11. [F5 / Shape Security DevCentral KB article](https://community.f5.com/kb/technicalarticles/what-is-shape-security/284359) — F5's own conceptual overview
12. [F5 BIG-IP + Shape Enterprise Defense integration guide](https://my.f5.com/manage/s/article/K37712232) — legacy Shape-product deployment
13. [Gartner Peer Insights — F5 Distributed Cloud Bot Defense](https://www.gartner.com/reviews/market/online-fraud-detection/vendor/f5/product/f5-distributed-cloud-bot-defense) — third-party positioning and reviews
14. [TrustRadius reviews — F5 Distributed Cloud Bot Defense (formerly Shape Security)](https://www.trustradius.com/products/shape-security/reviews) — third-party user feedback
15. [G2 reviews — F5 Distributed Cloud Bot Defense](https://www.g2.com/products/f5-f5-distributed-cloud-bot-defense/reviews) — third-party user feedback
16. [StartupIntros — Shape Security history](https://startupintros.com/orgs/shape-security) — founding date / founders
17. [Sumble — F5 technology usage](https://sumble.com/tech/f5) — customer-deployment inference
18. [svebaa — "Dissecting Shape Security's Virtual Machine"](https://svebaa.github.io/personal/blog/shape-security/) — canonical public RE writeup
19. [g2asell2019/shape-security-decompiler-toolkit (GitHub)](https://github.com/g2asell2019/shape-security-decompiler-toolkit) — dynamic deobfuscation toolkit
20. [ranftldieterHub/Shape-Security-protection-reverse-engineering (GitHub)](https://github.com/ranftldieterHub/Shape-Security-protection-reverse-engineering) — Node.js emulator + telemetry generator
21. [RoundProxies "How to bypass Shape (F5) antibot in 2026"](https://roundproxies.com/blog/f5-bypass/) — 2026 practical bypass landscape
22. [ZenRows "How to Bypass F5 Antibot in 2026"](https://www.zenrows.com/blog/bypass-f5) — alternative practical bypass coverage
23. [F5, Inc. on Wikipedia](https://en.wikipedia.org/wiki/F5,_Inc.) — parent-company overview
24. [Nomios Group — F5 Shape partner page](https://www.nomios.com/partners/f5-networks/f5-shape/) — partner/reseller perspective on product positioning
25. [WorldTech IT — F5 Distributed Cloud Bot Defense overview](https://wtit.com/f5-distributed-cloud-bot-defense/) — reseller deployment overview

**Where the public record is thin (and what we wrote based on inference rather than citation):** customer list specifics (§1.3) are partially inferred from analyst commentary + F5's own anonymized case studies. The "9 of the top 10 US banks" claim is repeated across multiple sources but not in a single primary citation; treat as marketing-grade rather than verifiable. The 30-min rotation cadence (§2.3) is from a single source (svebaa) and may have changed since publication.

---

## Appendix A — quick-reference one-pager

For contributors who need the short version on a Shape-protected customer ticket:

1. **Confirm vendor**: look for `/ssx/ssx.mod.js` in the page body OR a `TS01<8-hex>=...` cookie OR a `BIGipServer*` cookie. Two-of-three is decisive.
2. **Classify use case**: render-only / form-submit / sustained-scrape. Each has a different disposition (per §5.1).
3. **Run a one-shot sweep_metrics capture**: 5 reps across 4 profiles; record body sizes + cookies + per-response headers (including `X-*` prefixes).
4. **Read the verdicts**: passing one-shot renders → say "BO clears the passive layer." Failing form-submits → say "active layer is out of v0.1.0 scope; solver required."
5. **Disclose ML-retroactivity**: any "we work against Shape" statement MUST be qualified as "for fresh-session single-shot requests; sustained sessions trigger Shape's network-effect ML re-scoring."
6. **Don't promise**: a Shape solver in v0.1.0 (deferred per §6.4). Don't claim 100% pass rate (we've never measured a Shape site). Don't promise mobile-SDK bypass (different product, out of scope).

The honest customer-facing language is in §7.4. The marker patch (§3.5) is the v0.1.0 acceptance bar. Everything else is forward-looking research.
