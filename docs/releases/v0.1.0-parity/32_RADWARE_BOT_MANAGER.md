# 32 — Radware Bot Manager (formerly ShieldSquare)

**Status:** reference (customer-onboarding playbook for a vendor NOT in the 126-corpus)
**Audience:** customer-onboarding engineers, sales engineers preparing pitch decks for airline / travel / e-commerce verticals (Radware's customer concentration), and the maintainer extending the vendor cookbook (chapter 18) to cover behavioral-ML bot vendors as they appear in support tickets.
**Companion docs:** `18_ANTI_BOT_VENDOR_COOKBOOK.md` (encyclopedic vendor index — Radware is missing from §2 and should be added by this chapter's plan), `07_DATADOME_PRIMITIVES.md` (the closest structural analogue — behavioral + fingerprint + cookie), `13_FILE_LOCATIONS_INDEX.md` (the `humanize.js` file index entry that's load-bearing for this chapter's BO-coverage section), `27_VENDOR_COMPETITIVE_MATRIX.md` (cross-vendor matrix), `31_FASTLY_NGWAF.md` (the other non-corpus vendor — Fastly is structurally the opposite of Radware; light TLS-edge vs heavy-JS behavioral).

**Why this chapter exists.** Radware Bot Manager — acquired from ShieldSquare in March 2019 ([Radware Wikipedia][radware-wiki]; [Capterra][capterra]) — is the **fourth-tier bot management vendor** by 126-corpus presence (zero sites) but has **concentrated dominance in airline / travel / e-commerce verticals** where it actively shows up in `~/projects/browser_oxide_internal` customer-onboarding tickets. Unlike Fastly NGWAF (chapter 31) where BO is structurally well-positioned, Radware is the **opposite case**: a heavy-JS behavioral-ML vendor where BO's coverage is **partial-and-uncertain**, with the load-bearing dependency being `crates/browser/src/js/humanize.js` (per `13_FILE_LOCATIONS_INDEX.md`) — a ~30-setTimeout mouse-event-synthesizer that we know empirically is sometimes insufficient against IDBA-class behavioral scoring. **This chapter is the honest customer-pitch position**: predicted partial coverage, recommend slower scrape rate + variable timing, expect per-tenant pilot-revision.

**One-paragraph thesis.** Radware Bot Manager's defining technology — Intent-based Deep Behavior Analysis (IDBA) per [Radware Cyberpedia][radware-idba] (auth-walled but cached references throughout Radware product literature) — operates on "a higher level of abstraction of 'intent', rather than commonly-used shallow interaction-based behavior analysis techniques" ([Radware Cyberpedia, quoted via search][radware-idba-quote]). Concretely IDBA observes mouse movements, keystroke dynamics, scroll cadence, touch events, and form-fill patterns, and **scores against a per-customer behavioral baseline**. ShieldSquare's pre-Radware integration guide documented the JS Tag as collecting **"over 250 parameters including events like page scroll, touch, button clicks, mouse movements, keystrokes, sensor data, and URL traversal"** ([Radware Medium][radware-medium-js]). For BO, this is the **single hardest signal class** to fake: our `crates/browser/src/js/humanize.js` synthesizes ~30 mouse-events on a fixed cadence per page-load, which provides *something* for Radware to observe but is **demonstrably insufficient** to match a real human's distribution of curve-radius + velocity + acceleration + scroll-pause cadence + pre-form-fill hesitation timing. **Predicted default outcome on a Radware Bot Manager Premier customer pilot: 30-60% pass rate at default settings**, with the recoverable lever being IP class + scrape-rate management at the orchestration layer (chapter 22 §production-deployment), not engine-side fingerprint work.

---

## 1. Product overview and history

### 1.1 ShieldSquare as the pre-acquisition product

ShieldSquare was a Bangalore-based bot management startup founded ~2015. The product positioned itself as a **non-intrusive API-based bot management solution** — meaning the customer integrated a JS tag (`<script>` snippet) and a server-side validation library (the SDKs at [`github.com/ShieldSquare`][shieldsquare-gh]), and ShieldSquare's cloud API returned per-request bot/human verdicts. This contrasts with:

- **Inline WAF model** (Cloudflare, Fastly NGWAF): the WAF sits in the request path and makes the verdict.
- **Reverse-proxy model** (DataDome's hosted option, Akamai BMP): vendor-owned infrastructure between client and origin.

ShieldSquare's API-first model meant the customer's origin **always sees the request** and asks the vendor "is this a bot?" — letting the customer make the final allow/block decision in their own application code. The trade-off: latency (one synchronous API call per request) for granular customer control.

The product's distinguishing technology — even pre-acquisition — was **Intent-based Deep Behavior Analysis (IDBA)**: an ML model trained on per-customer behavioral baselines that scored not just "did this request have a real mouse?" but "did this mouse pattern match the intent profile of a real shopper on this specific e-commerce site?". The whitepapers ([Radware Bot Manager whitepaper][radware-whitepaper]) emphasize the **per-vertical training** — airline-traffic IDBA model is different from retail-traffic IDBA model.

### 1.2 The Radware acquisition

Per [Radware Wikipedia][radware-wiki]:

> "In January 2019, Radware acquired ShieldSquare, described as 'a market-leading bot management provider' to expand cloud security capabilities. The article does not specify the purchase price."

The G2 / Capterra and AlternativeTo records consistently date this as **March 2019** (per [AlternativeTo on ShieldSquare][alternativeto-shieldsquare] and [Radware Cyberpedia / vendor pages][radware-cyberpedia-bm]), which is the **closing date** vs the announcement (consistent with Radware's January announcement → March close cadence at the time). The product was renamed to **Radware Bot Manager** and the company's bot-management offering integrated with their pre-existing application security suite (DefensePro, AppWall, Alteon — per the Wikipedia summary).

### 1.3 Customer concentration: airlines, travel, e-commerce

Anecdotal evidence from Radware's case-study + customer-testimonial pages (sampled in 2024-2026 from product-marketing content) shows **disproportionate concentration in**:

- **Airlines and travel** (fare-scraping is the single most economically-valuable scraping use case; airlines reasonably-paranoid about competitive intelligence + price-arbitrage scraping).
- **E-commerce retail** (inventory + price scraping; especially apparel + electronics during sales seasons).
- **Banking + financial services** (Radware's pre-acquisition strength; cross-sells into Bot Manager).
- **Media + ticketing** (sneaker drops, concert tickets — ShieldSquare's pre-acquisition heavy users).

This concentration matters for BO customer-onboarding: a prospective customer whose target list is concentrated in these verticals **should be screened in advance for Radware tenancy** (§6 below). The default scrape strategy needs adjustment for Radware-protected targets (§8).

### 1.4 The 2025-2026 product line

Per [Radware Bot Manager product page][radware-bot-manager-cached] (auth-walled live, cached via [Radware Cyberpedia indexing][radware-cyberpedia-bm]) and the G2 / Capterra reviews:

| SKU | Deployment | Description |
|---|---|---|
| **Radware Bot Manager** | API-based + JS Tag | The flagship — IDBA + fingerprinting + JS challenge |
| **Cloud Bot Manager** | SaaS hosted by Radware | Same engine, Radware-managed |
| **Mobile SDKs** | iOS + Android native | Server-side validated mobile attestation ([`radwarebotmanager/radware-botm-iOS`][radware-ios-sdk], [`radwarebotmanager/radware-botm-android`][radware-android-sdk]) |
| **API Bot Manager** | API-endpoint-only | Protects backend APIs without JS tag dependency |
| **Crypto Challenge** | Optional escalation | JS-side PoW (per [Radware Bot Manager 23.1.1.0 release notes][radware-2311]: "Force JS check for Crypto Challenge Mitigation that verifies if JavaScript execution capability is available at the client browser before a Crypto Challenge is thrown") |

Distinguishing from competitor model:

- **PerimeterX/HUMAN**: similar behavioral model, but PerimeterX is inline (request-path) and ships the 60-second `_px3` clearance cookie design (chapter 18 §2.6). Radware is API-based (more granular customer control) and uses longer-lived session cookies.
- **DataDome**: similar JS-side fingerprint + WASM challenge, but DataDome ships a uniform `dd-script.js` from `captcha-delivery.com` (chapter 18 §2.2). Radware's JS Tag is loaded per-customer from customer-controlled or `radwarebotmanager.com` hosts.
- **Akamai BMP**: similar deep-fingerprint + sensor_data POST model. Akamai is CDN-integrated (chapter 18 §2.3); Radware is application-integrated.

---

## 2. Architecture

### 2.1 The JS Tag + API flow

Per [Radware Medium blog on JS injection][radware-medium-js]:

> "ShieldSquare's JS tag integration collects over 250 parameters including events like page scroll, touch, button clicks, mouse movements, keystrokes, sensor data, and URL traversal."

And per the integration documentation (referenced in [Radware Bot Manager support pages][radware-impl-guide], login-walled but cached):

> "The Radware Bot Manager JS Tag is pasted/added to the header section of every web page where you want Radware Bot Manager Bot detection."

Concrete flow (reconstructed from cached references; per-tenant variation expected):

1. **First request** (browser → customer origin → Radware-protected page). Origin returns the page with `<script src="https://www.shieldsquare.com/ss/<customer-id>/tag.js">` (or per-tenant equivalent host).
2. **JS Tag bootstrap** (~50-200 KB minified; daily-rotating obfuscation per industry convention for behavioral vendors). Tag sets event listeners on `mousemove`, `mousedown`, `mouseup`, `keydown`, `keyup`, `scroll`, `touchstart`, `touchmove`, `touchend`, `click`, `submit`, `focus`, `blur`, and `beforeunload`. Tag also fingerprints: navigator, screen, WebGL, canvas, audio, fonts, plugins, timezone, hardwareConcurrency, performance.memory.
3. **Event accumulation**. Tag accumulates events into a per-page-load buffer for ~200-500 ms (configurable; depends on customer tier).
4. **Initial verdict POST** (sometimes piggybacked on the first XHR the page makes, sometimes a dedicated POST to `radwarebotmanager.com/<endpoint>` or the customer's origin which forwards to the API). Body is encrypted (per-customer-key XOR or AES; specifics are not publicly documented since Radware's strength is the per-tenant ML model, not the encryption envelope).
5. **Verdict cookie** set. The cookie carries a per-session token; subsequent requests carry it and the customer's server-side SDK validates it via Radware's REST API.
6. **Continuous behavioral observation**. The JS Tag continues to observe events through the session and re-POSTs updated behavior buffers periodically. **A session that starts as "human" can be re-classified as "bot" mid-session** if the behavior deviates (e.g. zero mouse movements over 60 s on a page that normally has them — indicating headless automation).
7. **Mitigation when bot**. Depending on customer-configured rule: block (403), serve a fake page (honeypot), inject a Crypto Challenge (JS PoW), or just log without blocking (monitor mode).

### 2.2 Mobile SDK (separate detection path)

Per [`radwarebotmanager/radware-botm-iOS`][radware-ios-sdk] and [`radwarebotmanager/radware-botm-android`][radware-android-sdk]:

> "Radware Bot Manager iOS Mobile SDK needs to be integrated on mobile applications to detect and mitigate bot activities."

The mobile SDK uses **platform attestation primitives**:

- iOS: DeviceCheck + (post-iOS 14) App Attest. The SDK collects a server-attested signature that an Apple platform key signs over the app+device identity.
- Android: SafetyNet (deprecated 2024) → Play Integrity API. Similar server-attested device signature.

**Implication for BO**: web scrapers do not interact with the mobile SDK at all. A Radware-protected app that publishes a web flow alongside a mobile app uses the web JS Tag for the web traffic; the mobile SDK only fires from inside the native app. BO never encounters mobile-SDK paths.

But: some Radware customers (notably airlines) **force authentication or critical operations through the mobile app** specifically because the mobile attestation is stronger than web behavioral scoring. For these customers, BO can fetch the marketing pages but cannot complete checkout flows — out of engine scope.

### 2.3 ML model: IDBA architecture

Per [Radware Cyberpedia (search-cached references)][radware-idba-quote] and the Radware Bot Manager whitepaper:

> "Intent-based deep Behavior Analysis (IDBA) reads beyond mouse movements and keystrokes to understand the intent of visitors, detect bad bots, and avert potential attacks."

> "The solution provides the industry's most accurate detection by leveraging intent-based deep behavioral analysis (IDBA) to filter highly sophisticated human-like bots at minimum false positives."

> "Bot Manager provides precise bot management across all channels by combining behavioral modeling for granular intent analysis, collective bot intelligence and fingerprinting of browsers, devices and machines."

The IDBA model is **not publicly described in algorithmic detail** (vendor IP), but inferable from the marketing language:

- **Two-tier**: a shallow per-request scoring (latency-sensitive, applied to every request) + a deeper periodic re-scoring (asynchronous, looks at session-level patterns).
- **Per-vertical training**: airline-traffic model differs from retail-model differs from banking-model. Customers in a vertical share the training signal via collective bot intelligence.
- **Per-customer baselining**: within a vertical, each customer's normal traffic shape is the baseline. Anomalies are scored relative to baseline.
- **Confidence scoring**: per-request verdict includes a confidence score; the customer's rule can act differently per confidence tier (block at >0.9, challenge at >0.7, log at >0.5, pass otherwise).

### 2.4 Per-customer model rotation

Per industry pattern (no public Radware-specific source, but consistent with behavioral vendors generally):

> ML models for behavioral vendors rotate monthly. The IDBA model deployed against customer X in January is materially different from the model deployed in March — feature weights shift as new bot patterns surface and new human-baseline data accumulates.

**Implication for BO**: any solver work targeting Radware specifically goes stale within a month. Even if BO shipped a "Radware solver" today that passed a target customer-pilot, that solver would degrade by ~30 days. This is the standing argument against per-vendor Radware encoder work in `vendor_solvers` (chapter 18 §6 / chapter 06 §3 alternatives) — the maintenance burden is hostile.

---

## 3. Detection signals — IDBA breakdown

The 250 parameters claim ([Radware Medium][radware-medium-js]) is the headline. Concretely, the signal classes BO needs to consider:

### 3.1 Mouse movement entropy

The single largest behavioral signal class. Per industry pattern (Akamai BMP + DataDome + PerimeterX all observe this; Radware is similar):

- **Position trace**: `mousemove` events sampled at ~10-30 Hz, producing per-frame `(t, x, y)` tuples.
- **Velocity profile**: derivative of position. Real humans show non-uniform velocity (deceleration when approaching click targets).
- **Acceleration profile**: second derivative. Real humans show smooth acceleration curves; bots typically show step-function changes.
- **Curvature**: `mousemove` paths between two points are not straight lines for humans (small lateral wobble) but are linear for naive bots.
- **Idle distribution**: gap between bursts of mouse activity. Real humans show power-law-distributed idle times; bots show uniform or no idle times.

**BO coverage of mouse signals**. Per `13_FILE_LOCATIONS_INDEX.md` and `crates/browser/src/js/humanize.js`:

- BO's `humanize.js` runs as a content-script-equivalent on page-load.
- It registers ~30 `setTimeout`s that fire over the first 5-10 seconds of page life.
- Each timeout dispatches a synthesized `MouseEvent` at a precomputed `(t, x, y)` coordinate on the page.
- The coordinate sequence is **a fixed-shape Bezier curve** between two predetermined endpoints — no per-page-load randomization beyond the timeout intervals.

This is **demonstrably weaker than what IDBA expects**:

1. ~30 events over 5-10 s is **much sparser than a real user** (real users emit ~200-500 events/sec when actively moving the mouse; thousands per minute of dwell).
2. The fixed Bezier shape is **uniform**, not random — IDBA's per-customer baseline shows non-Bezier curvature for real users.
3. There is **no acceleration/deceleration profile** in the synthesized events — they're emitted at the precomputed coords at the precomputed times, producing a piecewise-linear velocity profile.
4. **No click events are coupled to the mouse position** — the synthesized mouse moves through coordinates that don't correspond to any actual clickable element.

The honest assessment: `humanize.js` **provides something** for Radware to observe (so the page isn't classified as "no mouse events at all"), but **does not match real-human distribution** of any of the above metrics. IDBA at high-confidence will catch this.

### 3.2 Keystroke dynamics

Per [Radware Bot Manager whitepaper][radware-whitepaper] (search-cached): keydown/keyup pairs, intra-key timing, dwell duration, flight duration between keys.

**BO coverage of keystroke signals.** `humanize.js` does NOT synthesize keyboard events (verify by grep). On a page that requires form fill (login, search), BO emits no keystrokes — Radware will see "form submitted with no preceding keystrokes" which is a strong bot signal.

**Mitigation**: at the orchestration layer, only navigate to Radware-protected pages that don't require form interaction (read-only product pages, search results, etc.). Avoid checkout / login flows on Radware-protected targets.

### 3.3 Scroll velocity and cadence

Per the Radware Medium blog list: "page scroll" is one of the 250 parameters.

- **Scroll event rate**: real users scroll in bursts (page-down keypress, mouse wheel, touch swipe); bots typically don't scroll at all or scroll uniformly with `window.scrollTo()`.
- **Scroll velocity profile**: similar to mouse-velocity profile.
- **Scroll dwell**: pause-at-content-section pattern (real users read; bots don't).

**BO coverage of scroll signals.** `humanize.js` does NOT synthesize scroll events (verify by grep). Radware sees "loaded page with full content, no scrolling, navigated away" — a moderate-confidence bot signal.

**Mitigation**: a future `humanize.js` extension could fire `wheel` events on a randomized cadence. Not currently planned (chapter 19 profile-expansion-plan does not include this).

### 3.4 Touch events (mobile signals)

For BO's `iphone_15_pro_safari_18` profile, Radware-protected mobile pages observe:

- `touchstart` / `touchmove` / `touchend` events
- Multi-touch presence (pinch-zoom, two-finger scroll)
- Touch coordinate dispersion (real fingers have larger touch areas than synthetic clicks)
- Orientation events (`deviceorientation`, `devicemotion`)

**BO coverage of touch signals.** `humanize.js` does NOT synthesize touch events (verify by grep). The `iphone_15_pro_safari_18` profile's `navigator.maxTouchPoints` correctly reports 5 (per `crates/stealth/profiles/iphone_15_pro_safari_18.yaml`) but no actual touch events are emitted. Radware-protected mobile flow sees "iOS device with no touch events" — high-confidence bot signal.

**Mitigation**: same as scroll — orchestration-layer avoidance of Radware-protected mobile flows from the iphone profile. Use chrome/firefox profiles instead (where the absence of touch events is consistent with desktop).

### 3.5 Form-fill patterns

For pages with `<input>` fields and `<form>` submission:

- **Pre-fill hesitation**: time between focus on the first field and the first keystroke. Real users show 500ms-3s hesitation; bots show <50ms or instant.
- **Inter-field navigation**: tab key vs. mouse click to move to next field. Real users mix both; bots typically use one consistently.
- **Submission timing**: time from last keystroke to form submit. Real users show 200ms-2s pause; bots show instant or fixed-delay.

**BO coverage of form-fill signals.** `humanize.js` doesn't fill forms (it only generates synthesized mouse moves on the page). Programmatic form-fill from the orchestration layer (via `Page::type_text` and `Page::click` per `crates/browser/src/page.rs` API) does emit keyboard events with correctly-spaced timings (the `type_text` impl uses `tokio::time::sleep` between key events), BUT:

- The pre-fill hesitation is whatever the caller schedules (often instant).
- The inter-field navigation pattern is whatever the caller scripts.
- The submission timing is whatever the caller chooses.

Recommended pattern for Radware-protected form flows (orchestration layer):

```rust
// Pre-fill hesitation
tokio::time::sleep(Duration::from_millis(rand_range(500, 1500))).await;
page.type_text("input[name=email]", "u@e", Duration::from_millis(120)).await?;  // 120ms/key
// Pause between fields
tokio::time::sleep(Duration::from_millis(rand_range(200, 800))).await;
page.type_text("input[name=password]", "p", Duration::from_millis(150)).await?;
// Submission pause
tokio::time::sleep(Duration::from_millis(rand_range(300, 1200))).await;
page.click("button[type=submit]").await?;
```

This is **orchestration-layer work, not engine work** — the engine provides the timing primitives, the caller composes them.

### 3.6 Standard fingerprint baseline

Per [Radware Bot Manager blog][radware-medium-js]: "fingerprints" is listed alongside the behavioral signals. The standard fingerprint surface:

- Canvas hash (text + emoji rendering)
- WebGL renderer + extensions list
- AudioContext rendered samples (DynamicsCompressor + Oscillator)
- Font list (via measuring DOM-element widths for each candidate font)
- Plugin list (deprecated; modern browsers expose empty list)
- Screen dimensions + color depth + pixel ratio
- Navigator properties (language, platform, hardwareConcurrency, deviceMemory, etc.)
- Timezone + locale
- WebRTC IP leak

**BO coverage of fingerprint signals.** Per chapters 09/16/17 — equivalent coverage to other vendors. Canvas rendered correctly via `crates/canvas/`, WebGL via `crates/canvas/src/webgl_render.rs`, audio via `crates/audio_runtime/`, fonts via `crates/font_manager/`. Navigator surface via `crates/stealth/src/presets.rs`. The same fingerprint engine that satisfies Akamai BMP's collection (chapter 26) satisfies Radware's collection — **fingerprint is not the load-bearing failure mode for Radware**. Behavior is.

### 3.7 Headless browser detection

Per [Radware Bot Manager 23.1.1.0 release notes][radware-2311]: "Force JS check for Crypto Challenge Mitigation that verifies if JavaScript execution capability is available at the client browser before a Crypto Challenge is thrown. Bots without JavaScript capability will be blocked at source."

The mechanism: a Crypto Challenge (JS PoW) only fires when the engine has confirmed JS execution capability. If the engine sees no JS execution (curl + Python requests), it blocks at source without bothering to compute a challenge.

**BO coverage of headless detection.** BO runs full V8 + executes page JS, so the "no JS execution" hard-block doesn't apply. The Crypto Challenge tier itself (JS PoW) is solvable similar to Cloudflare's JS Challenge (chapter 25 §2.1) — generic primitive coverage via chapter 07 applies.

### 3.8 Per-session evolution

The IDBA two-tier scoring (§2.3) means **a session can transition from "human" to "bot" mid-session**. For scrapers, this manifests as:

- First few requests pass.
- After 5-10 requests with no mouse/keyboard activity, session re-scored as bot.
- Subsequent requests blocked.

**BO coverage of session evolution.** Out of engine scope — the engine can keep emitting periodic mouse events through the session (extension of `humanize.js`), but the structural fix is at the orchestration layer: **rotate sessions** (new cookie jar, new `Page` instance) more frequently for Radware-protected scrapes. The `crates/pool` design supports per-session lifetime limits; chapter 22 §production-deployment is the place to spec this.

---

## 4. Mitigation patterns

### 4.1 Block (hard 403 with Radware branding)

Per release notes + customer reviews on G2 / Capterra: the default block page is a Radware-branded HTML page (some customers replace it; the default text references "automated traffic" without naming the vendor).

Detection from the scraper side:
- Status 403
- Response body 1-5 KB
- May contain `radwarebotmanager.com` or `shieldsquare.com` in body links or `Set-Cookie` paths

### 4.2 Challenge (CAPTCHA or behavioral re-test)

Per [Radware Bot Manager 23.1.1.0][radware-2311]:

> "Force JS check for Crypto Challenge Mitigation that verifies if JavaScript execution capability is available at the client browser before a Crypto Challenge is thrown."

The Crypto Challenge is a JS PoW; on solve it sets a clearance cookie and the next request passes. Some customers also configure reCAPTCHA or hCaptcha as the interactive escalation.

Detection from the scraper side:
- Initial 200 with a small body containing the challenge JS
- The challenge JS expects to evaluate, POST a token to Radware, receive a Set-Cookie, and reload
- Generic primitive coverage via chapter 07 §Primitives 1/2/3 applies if the markers are in classify.rs (§6 below)

### 4.3 Honeypot (serve fake data)

**Distinctly Radware approach** — the IDBA verdict can be "serve a decoy page instead of blocking". Per [Radware Bot Manager content-scraping protection][radware-anti-scraping]:

> "[Radware Bot Manager is] a real-time anti-scraping solution that protects online sites from content theft using a combination of dynamic browser challenge mechanism, behavior pattern analysis, device and network profiling of request source to identify, detect, assess and prevent web scraping activity on websites."

The "prevent" mechanism explicitly includes **decoy serving**: instead of returning the real product page with the real price, return a page with fake data (wrong price, fake out-of-stock status, scrambled product descriptions). The scraper ingests garbage and doesn't know it's been compromised.

Detection from the scraper side:
- 200 OK with normal-looking page
- Content semantically wrong (prices off, descriptions don't match other sources, OOS status inconsistent with crawler observation)
- No way to detect without cross-source validation

**Implication for BO customers**: any Radware-protected scrape needs **cross-source validation** at the data-pipeline layer. Compare scraped prices against price-feed APIs, against archived snapshots from prior days, against multiple BO sessions running independently. If a single Radware-suspect source consistently shows price drift versus baseline, treat as honeypot-confirmed and rotate scraping infrastructure.

### 4.4 Soft block (intentional slow response)

Similar to Fastly NGWAF tarpit (chapter 31 §4.2). Radware-flagged sessions may receive 10-30 s response times. The intent is the same: consume scraper capacity without giving a clean "you're blocked" signal.

**Detection + mitigation**: same as Fastly tarpit — back off the scrape rate; rotate IP.

### 4.5 Log-only (monitor mode)

Many Radware customers run in monitor-only mode for weeks after first deployment to baseline traffic patterns before turning on blocking. During this window, **scrapers pass freely** even though they're flagged. The customer's analytics dashboard shows "bot traffic detected" but no requests are blocked.

This is **not detectable from the scraper side** — successful scrape, no anomaly. The implication for BO is positive: a customer-pilot today may show "BO passes Radware" because the customer is in monitor-only mode; the same scraping may fail in 4 weeks when they flip to enforce mode. **Customer-pilot results have a 4-week validity window for Radware-protected targets**.

---

## 5. BO coverage — partial

The honest framing. Unlike Fastly NGWAF (chapter 31 §5) where the prediction is strong-pass, the Radware prediction is **30-60% pass at default settings**, with significant per-customer variation.

### 5.1 The structural argument

- **Fingerprint surface**: BO matches real Chrome at the canvas / WebGL / audio / navigator level (chapters 09/16/17). **Not the failure mode for Radware**.
- **TLS surface**: BO ships byte-perfect Chrome TLS (chapter 23). **Not the failure mode for Radware** — Radware is application-layer, not network-layer.
- **Behavioral surface**: BO's `humanize.js` synthesizes ~30 mouse events per page; no keyboard, no scroll, no touch. **This IS the failure mode.**

The structural diagnosis: **Radware's IDBA scores against behavioral patterns we don't faithfully emit**.

### 5.2 What `humanize.js` actually provides

Per `crates/browser/src/js/humanize.js` (note: contributors editing this file should re-verify these counts via grep):

- Sets up ~30 `setTimeout` callbacks at various intervals.
- Each callback dispatches a `MouseEvent` (`mousemove`) at a precomputed `(x, y)` coordinate.
- Coordinates trace a fixed-shape Bezier curve over the page.
- Total active time: ~5-10 seconds from page load.

This is **enough to satisfy vendors that check "did any mouse event fire?"** (e.g. Akamai BMP's mouse-event-count counter). It is **not enough to satisfy vendors that compute per-curve statistics on the mouse trajectory** (Radware IDBA, PerimeterX behavioral, DataDome's behavioral signals).

**Recommended `humanize.js` extension** for Radware-protected coverage (not currently planned, but documented here for future iteration):

- **Scroll event synthesis**: dispatch `wheel` events on a randomized cadence to satisfy the "did the user scroll?" check.
- **Touch event synthesis** (iphone profile only): dispatch synthetic `touchstart`/`touchmove`/`touchend` events on `mousedown`/`mousemove`/`mouseup`.
- **Variable event rate**: instead of 30 events on fixed timeouts, emit 100-1000 events with per-burst randomization.
- **Acceleration profile**: compute velocity + acceleration through the Bezier and add per-frame jitter to match real-human distribution.

The chapter 19 profile-expansion plan does not currently include this; it would be a separate work item.

### 5.3 The 30-60% prediction

The interval `30-60%` is **engineering inference**, not measurement. The reasoning:

- Customers in **monitor-only mode** (§4.5): BO passes at ~100% (Radware isn't blocking).
- Customers in **low-confidence-threshold mode** (block only at very high IDBA confidence): BO passes at ~70-80% (our partial behavioral coverage is enough for the low-confidence cases; high-confidence sessions get caught).
- Customers in **moderate-confidence-threshold mode**: BO passes at ~30-50% (our weak behavioral coverage starts being load-bearing).
- Customers in **high-confidence-threshold mode + Crypto Challenge cascade**: BO passes at ~10-20% (the JS PoW tier is solvable but the per-session behavioral re-scoring catches us).

The 30-60% interval covers the modal customer (moderate threshold, default settings). **Per-pilot measurement is mandatory.**

### 5.4 Comparison to other vendor clusters

Per chapter 27 §1 (with the same caveat that Radware is not in the 126-corpus):

| Vendor cluster | BO expected | Closest 126-corpus analogue | Notes |
|---|---|---|---|
| **Radware (monitor-only)** | PASS | parity tier (chapter 27 §1 wbaas) | Customer not actively blocking |
| **Radware (low-confidence threshold)** | mostly PASS | similar to PerimeterX zillow win pattern (chapter 27 §2.1) | Behavioral verdict is lenient |
| **Radware (moderate threshold)** | PARTIAL | similar to DataDome silent rt:'i' (chapter 27 §1 etsy / tripadvisor) | Per-iteration variability |
| **Radware (high threshold + Crypto Challenge)** | mostly FAIL | similar to Akamai sec-cpt homedepot (chapter 27 §4) | Frontier; would need humanize.js extension + per-tenant capture |

### 5.5 What BO does NOT have for Radware

- **No measured pass rate** on any Radware-protected target (no corpus presence; no internal benchmarks against Radware customers — `~/projects/browser_oxide_internal/benchmarks/baselines/` does not contain a Radware row).
- **No Radware-specific marker** in `classify.rs:81-156` today (§6.4 below specs the additions).
- **No Radware-specific solver** in `vendor_solvers` (per the §2.4 ML-model-rotation argument, none planned).
- **No `humanize.js` extension** for stronger behavioral mimicry (not in chapter 19; would be a future work item if customer demand surfaces).

---

## 6. Detection markers (page-load level)

For extending `crates/browser/src/classify.rs:81-156` and `crates/browser/src/page.rs:1049-1057`.

### 6.1 Response headers

No publicly documented unique Radware Bot Manager response header in standard configurations. Some tenants add custom headers like `X-Bot-Manager-Status` or similar, but this is per-tenant.

### 6.2 Response body markers

Most reliable identification is via the JS Tag URL pattern in the page body:

| Marker | Source | Precision |
|---|---|---|
| `radwarebotmanager.com` | JS Tag host (most common) | High — unique to Radware |
| `radwarecloud.com` | Cloud Bot Manager hosted variant | High |
| `shieldsquare.com` | Legacy ShieldSquare host (some sites still on it) | High |
| `shieldsquare.atlassian.net` | Integration portal — appears in some embedded docs/cookie consents | Medium |
| `<script src=".../ss/...">` | Common ShieldSquare path convention | Medium |
| `<script src=".../botmanager/...">` | Common Radware path convention | Medium |
| `ss-set-cookie` (response header from CloudFront integration per [Radware AWS docs][radware-aws-cf]) | Specific to CloudFront-fronted deployments | High when present |

### 6.3 Cookie markers

Per [Radware Bot Manager support docs][radware-cookie] (referenced in cached search results; specific names not publicly catalogued in detail):

> "A new secure and tamper-proof cookie has been added by the Radware Bot Manager, with any attempt to spoof or reverse-engineer this cookie tracked to block bots."

The cookie name is **per-tenant configurable**, but common conventions:

- `_ss_<random>` (legacy ShieldSquare convention)
- `_rw_<random>` (Radware convention)
- `bm_<random>` (some configurations; collides with Akamai's `bm_sz` naming — caution on classification)
- `<customer-name>_bot_token` (customer-explicit naming)
- The challenge-cleared cookie name is documented in the customer's Bot Manager console; not publicly enumerable

**The precise cookie names are not documented as a single canonical list** because Radware's per-tenant customizability allows customer-renaming. The detection-engine implication: cookies are **weak identification** for Radware; rely on JS Tag URL in the body instead.

### 6.4 Proposed engine additions

Following the pattern of chapter 18 §4.1-4.3 and chapter 31 §6.4:

**Body markers** in `crates/browser/src/classify.rs:81-156`:

```rust
// Radware Bot Manager / ShieldSquare identification (per the §6.2 markers)
"radwarebotmanager.com"  // UNAMBIGUOUS when in body
"radwarecloud.com"        // UNAMBIGUOUS when in body
"shieldsquare.com"        // UNAMBIGUOUS when in body
"shieldsquare.atlassian.net"  // weak; appears in cookie-consent dialogs
```

Suggested classification tier: `SMALL_BODY_VENDOR_COSIGNAL` (so they fire as Radware-CHL only when body is small / status is challenge-shaped, mirroring the Akamai cosignal pattern at `classify.rs:127-134`).

**Header logger** (`crates/browser/src/page.rs:1049-1057`):

There are no widely-deployed Radware-canonical response headers to log. **Skip**; rely on the body markers.

**v8_html_is_real guard** in `crates/browser/src/page.rs:2273-2293`:

```rust
&& !v8_html.contains("radwarebotmanager.com")  // Radware page still in challenge state
&& !v8_html.contains("radwarecloud.com")
&& !v8_html.contains("shieldsquare.com")
```

**Clearance-cookie predicate**: Skip the engine-side predicate (per §6.3, cookie names are per-tenant). Rely on **body marker absence** as the "we cleared" signal — when the page body no longer contains the Radware JS Tag host, we've passed.

### 6.5 Acceptance gate for the markers

After the additions land:

- 126-site sweep MUST not regress (Radware host names are unique enough that false-positive collision risk is negligible).
- For any future Radware customer-pilot, the `Radware-CHL` classification appears in BO's per-iter output when the JS Tag is in the body of a small response.
- Honeypot-served pages (§4.3) are NOT detectable by these markers (200 OK, normal-looking body, no Radware JS Tag) — out-of-band cross-source validation remains the only detection path for honeypot serves.

---

## 7. Public solver landscape

Few public solvers exist. The reasoning is similar to Fastly NGWAF (chapter 31 §7) but with different specifics:

- **No public Radware-bypass GitHub repo** in the search results (`select:WebSearch` for "site:github.com radware bot manager OR shieldsquare bypass solver" returned only Radware's own SDKs, mobile integrations, and connectors — no third-party bypass).
- **The official ShieldSquare GitHub org** has 21 repos but they're all SDKs and integration code ([`github.com/ShieldSquare`][shieldsquare-gh]).
- **No commercial-solver-as-service** for Radware is documented in industry guides (compare CapSolver / 2Captcha / Anti-Captcha which all list AWS WAF, Cloudflare, hCaptcha, DataDome but **do not list Radware Bot Manager**).

Two reasons:

1. **IDBA's per-customer behavioral model rotates monthly** (§2.4). A solver targeting a specific Radware tenant degrades within ~30 days. The maintenance burden makes commercial solver-as-service unprofitable.
2. **Customer concentration in airlines + travel** means the high-value scrape targets are heavily-monitored — solvers that work get burned fast as Radware's collective bot intelligence (§2.3 IDBA description) shares signals across customers.

### 7.1 The closest analogues

For BO maintainers researching Radware customer pilots, the closest research areas:

- **PerimeterX/HUMAN research** — structurally similar (behavioral + fingerprint + short-lived clearance cookie). The `MiddleSchoolStudent/PerimeterX-solver` deobfuscator (chapter 18 §2.6) demonstrates the *approach* but not Radware-applicable code.
- **DataDome research** — chapter 07's primitives apply generically to any behavioral vendor's JS-challenge tier.
- **Akamai BMP research** — chapter 26's cookie state-machine work is the structural reference for any behavioral vendor's clearance tracking.
- **General behavioral-mimicry literature** — papers on humanizing mouse trajectories via Bezier with jitter, on keyboard timing distributions, etc. Useful for designing a `humanize.js` extension but not Radware-specific.

### 7.2 Why not commercial solver-as-service

Radware Bot Manager has **no commercial solver service** listed on the major CAPTCHA / anti-bot solver providers. The implication for customer-onboarding: when a customer asks "can I subscribe to a solver?", the answer is **no for Radware**; the path is BO-with-orchestration-layer-rate-management (§8).

---

## 8. Customer onboarding playbook

When a prospective customer brings a Radware-protected target. Order matters; each step's output feeds the next.

### 8.1 Step 1 — Identify Radware

```bash
URL='https://customer-site.example/'
curl -sS "$URL" -o body.html
grep -E 'radwarebotmanager\.com|radwarecloud\.com|shieldsquare\.com' body.html
```

If any match: Radware-protected. If none: probably not Radware. The negative case has false-negative risk (some customers proxy the JS Tag through their own domain), so the secondary check:

```bash
# Look for known cookie name patterns
curl -sS -D - "$URL" -o /dev/null | grep -iE '^set-cookie:.*(\b_ss_|\b_rw_|botmanager|shieldsquare)'

# Inspect script src hostnames
grep -oE 'src="[^"]+"' body.html | sort -u | grep -iE 'bot|shield|radware'
```

Two-out-of-three confirmation across (body marker, cookie pattern, script src) is sufficient identification.

### 8.2 Step 2 — Identify vertical

The high-Radware verticals (§1.3):
- Airlines / travel
- E-commerce retail (especially apparel + electronics)
- Banking / financial services
- Media / ticketing

A customer-target in any of these verticals raises the prior probability of Radware to ~40% (vs ~5% baseline). A non-vertical customer with Radware is unusual; verify the identification (§8.1) before scoping further.

### 8.3 Step 3 — Determine the deployment mode

Three modes, each with different BO impact:

| Mode | Indicator | BO impact |
|---|---|---|
| **Monitor only** | No 403s on any BO request; no decoy-data indicators | BO passes ~100% |
| **Block at high confidence** | Sporadic 403s on some BO requests; majority pass | BO passes ~70% |
| **Block + Crypto Challenge** | Small interstitial responses on some requests | BO passes ~30%; need chapter 07 primitives |
| **Block + decoy** | 200 OK with content that diverges from cross-source baseline | Detectable only by cross-source validation |

Determine via a 5-URL pilot:

```bash
target/release/examples/sweep_metrics chrome_148_macos \
    /tmp/radware_pilot_corpus.json /tmp/bo_out.json --capture customer-site
```

Run with N=10 iterations per URL (more than the Fastly default of 5 — Radware's session-evolution scoring means per-iteration variance is meaningful).

### 8.4 Step 4 — Set expectations honestly

The customer-pitch position:

> "Your target is Radware-protected. Our engine predicts 30-60% pass rate at default settings — we will measure this in a pilot. The recoverable lever is orchestration-layer scrape-rate management: we recommend 1-2 requests/second per IP, frequent session rotation (every 5-10 requests), and avoidance of form-fill flows (login / checkout) on Radware-protected pages. If your scrape needs include form-fill flows, those will need significant per-tenant work and we cannot guarantee success."

Compare to the Fastly customer-pitch position (chapter 31 §5.2): "we predict you pass without per-vendor work". The Radware pitch is **honestly worse** — and that's the point of having this chapter, so the customer-pitch authors don't over-promise.

### 8.5 Step 5 — Per-tenant rate management

For Radware-protected scrapes (orchestration layer):

- **Concurrency**: 1-2 requests/sec per IP per session.
- **Session rotation**: new `Page` instance every 5-10 navigations (forces new cookie jar, new behavioral baseline).
- **IP rotation**: cycle through 10+ IPs in residential pool; avoid datacenter ASNs.
- **Per-page dwell**: at least 2-5 seconds per page (longer than Akamai's 0.5-1 s minimum). Real users dwell on Radware-protected sites.
- **Avoid form flows**: read-only product / search / browse pages only on Radware-protected targets.

The `crates/pool` design supports per-pool concurrency limits; chapter 22 §production-deployment is the location for this guidance.

### 8.6 Step 6 — Cross-source validation for honeypot detection

If the customer's scrape is for **price comparison** or **inventory monitoring**, build cross-source validation into the pipeline:

- Sample 5-10% of scraped data against a second source (price-feed API, partner data exchange, archived snapshots).
- If a Radware-protected source shows consistent >5% drift from baseline, treat as honeypot-suspect.
- Rotate scraping infrastructure (new IP pool, new session rotation pattern) and re-validate.

This is **data-pipeline work, not engine work** — BO cannot detect honeypot serves by itself.

### 8.7 Step 7 — When the pilot fails

If BO + recommended rate management still produces <30% pass rate:

1. Verify the customer isn't in **enforce-mode with high confidence** (escalate to customer's security team to confirm the IDBA threshold setting).
2. Run **Camoufox from the same IP** as a baseline. If Camoufox also fails, the issue is **IDBA-frontier behavioral** — neither engine satisfies the model, scrape is impractical without API access (negotiate with customer).
3. If Camoufox passes and BO fails by >20%, file a chapter 19 (profile-expansion) or future-`humanize.js` work item; expect 4-8 weeks of engine work for material improvement.
4. If BOTH fail and the customer cannot relax their IDBA threshold, recommend **API access** as the alternative (some customers offer scrape-partner APIs at higher tiers).

### 8.8 Acceptance for the playbook

After running on a customer-pilot:

- Either the customer is onboardable with recommended rate management (~50% of cases in vertical-concentrated customer base).
- Or the customer is in monitor-mode and BO passes freely (~20%; revisit in 4 weeks).
- Or the customer is high-IDBA-threshold and neither BO nor Camoufox passes; out-of-band negotiation needed (~30%).

---

## 9. Forward-looking

### 9.1 Mobile attestation push

Per §2.2: Radware ships iOS + Android SDKs that use platform attestation (Apple App Attest, Google Play Integrity). The 2024-2026 trend across all bot-management vendors:

- More Radware customers moving sensitive operations (checkout, login, account-management) to mobile-only flows specifically because mobile attestation is stronger than web behavioral scoring.
- Web flows degrade to read-only browsing (browse products, search, view content).

**Implication for BO**: web-scraping coverage for Radware-protected customers will get *better* for read-only flows (less aggressive web blocking when sensitive flows are behind mobile) but *worse* for any operation requiring authentication or transaction (impossible without a real iOS/Android device).

This is a **product-positioning input, not an engine work item**: when scoping a Radware customer's needs, ask "do you need to scrape behind login?" — if yes, set expectations that scraping is impractical regardless of engine; if no, focus on read-only optimization.

### 9.2 API endpoint protection growing share

Radware's **API Bot Manager** product (per [Radware Cyberpedia][radware-cyberpedia-bm]) is gaining share. The product specifically targets **backend API endpoints** (REST / GraphQL) that don't render HTML pages — so the JS Tag doesn't apply, and detection is purely server-side fingerprinting (TLS fingerprint, header order, request rate, timing).

**Implication for BO**: API-only Radware deployments are **structurally similar to Fastly NGWAF** (chapter 31) — network + HTTP layer signals only. **BO should perform well against API Bot Manager** because the load-bearing signal class is TLS (chapter 23), which BO matches. The behavioral signal class doesn't apply (no JS execution in an API call).

For customer-onboarding: a target with both web pages AND API endpoints on Radware is bimodal: BO does poorly on the web pages (behavioral) and well on the API endpoints (network). Recommend API-first scraping when possible.

### 9.3 IDBA model evolution

Per industry pattern: behavioral ML vendors are **adding more signal classes** monthly. Future signals to expect:

- **Battery API** (deprecated in browsers but Radware may use timing-based proxies)
- **Performance.memory inference** (real users have different memory pressure patterns)
- **Network timing baseline** (real users on residential ISPs have characteristic latency distributions)
- **WebRTC stats** (real users have STUN/TURN-discovered IPs that don't match the request IP)

**Implication for BO**: chapter 23 (TLS) + chapter 17 (Web API parity) coverage stays load-bearing; behavioral coverage needs ongoing investment. The `humanize.js` extension referenced in §5.2 is the engine-side work item; ongoing IDBA model rotation means this is a moving target, not a one-shot fix.

### 9.4 What would change BO's posture against Radware

If BO:

- **Ships a meaningfully extended `humanize.js`** with scroll + touch + variable-cadence mouse + acceleration profiles. Estimated effort: 2-4 weeks. Predicted impact: +10-20% pass rate against moderate-confidence Radware deployments.
- **Adds form-fill humanization primitives** to the orchestration API (per §3.5). Estimated effort: 1-2 weeks. Predicted impact: enables login / checkout flow scraping for some Radware customers.
- **Ships per-session cookie-jar rotation in `crates/pool`** with Radware-aware session lifetime. Estimated effort: 1-2 weeks. Predicted impact: +5-10% pass rate via mitigation of IDBA session-evolution scoring.

None of these are in chapter 19 (profile-expansion) or the v0.1.0-parity work items. **They become priority work when customer demand surfaces** — i.e., when 3+ paying customer prospects have Radware-protected targets and bounce the deal on coverage gaps.

---

## 10. Acceptance + files

### 10.1 Acceptance gate for this chapter (when the §6.4 additions land)

- `crates/browser/src/classify.rs:81-156` extended with the `radwarebotmanager.com` / `radwarecloud.com` / `shieldsquare.com` body markers per §6.4, tiered as `SMALL_BODY_VENDOR_COSIGNAL`.
- `crates/browser/src/page.rs:2273-2293` (v8_html_is_real guard) extended with the same three host names per §6.4.
- No clearance-cookie predicate extension (per §6.3, cookie names are per-tenant — body-marker absence is the clearance signal).
- 126-site sweep does NOT regress (Radware host names are unique enough that false-positive collision risk is negligible).
- Manual customer-pilot validation: at least one BO-vs-Radware capture on a customer-target verifies the `Radware-CHL` classification when the JS Tag is in the body of a small response.

### 10.2 Files this chapter touches when its plan executes

| File | Section | Change |
|---|---|---|
| `crates/browser/src/classify.rs:81-156` | §6.4 | Add 3-4 Radware-vendor markers as `SMALL_BODY_VENDOR_COSIGNAL` (3-4 lines) |
| `crates/browser/src/page.rs:2273-2293` | §6.4 | Add 3 Radware host names to `v8_html_is_real` guard (3 lines) |

Total: **6-7 lines of code**, no new vendor encoder, no new private-crate dependency, no `humanize.js` work (deferred per §9.4).

### 10.3 Files this chapter does NOT touch (intentional deferrals)

- **No new private `vendor_solvers` module** for Radware. Per §2.4 / §7, the monthly IDBA model rotation makes per-vendor solver work unprofitable.
- **No `humanize.js` extension** in this chapter's plan. Per §5.2 / §9.4, this is a future work item gated on customer demand; the engineering effort is 2-4 weeks for material impact.
- **No new orchestration-layer rate-management code**. Per §8.5, this is documentation guidance for chapter 22 (production deployment); the `crates/pool` per-pool concurrency limits already exist as a primitive.
- **No new test in `crates/browser/tests/`**. The corpus-wide regression gate catches false-positive Radware classifications.

### 10.4 Cross-references

This chapter is the customer-onboarding deep-dive companion to:

- Chapter 07 (DataDome primitives) — the closest structural analogue; the JS Challenge tier of Radware reuses the same three generic engine primitives.
- Chapter 18 (anti-bot vendor cookbook) — should be extended with a `§2.15 Radware Bot Manager / ShieldSquare` entry that summarizes this chapter in the cookbook's 50-line format. Companion chapter 31 (Fastly NGWAF) should get the same treatment.
- Chapter 19 (profile-expansion plan) — does not currently include `humanize.js` extension; if customer demand for Radware coverage materializes, this is the chapter to expand.
- Chapter 22 (production deployment) — should be extended with the §8.5 rate-management guidance for Radware-protected targets.
- Chapter 27 (vendor competitive matrix) — Radware is the **non-corpus vendor cluster that lands in the BO-loses-or-partial quadrant** when pilot-measured, in contrast to chapter 31 (Fastly) which lands in the BO-wins quadrant.

Companion to this chapter on customer-onboarding theme: **chapter 31 (Fastly NGWAF)** — the architecturally opposite vendor (network-layer-first vs behavioral-ML-first); reading both together gives the full non-corpus customer-onboarding picture.

---

## 11. Honest uncertainty footnotes

The discipline correction, mirroring chapter 31 §11. **Radware Bot Manager is not in the 126-corpus**; every BO-coverage claim in §5 is **engineering inference**, not measured data.

1. **The 30-60% pass-rate prediction (§5.3)** is engineering inference from the architecture + the known `humanize.js` weakness. No measured BO-vs-Radware data exists in `~/projects/browser_oxide_internal/benchmarks/` or `docs/research_*` directories.
2. **The cookie name conventions (§6.3)** are pattern-inferred from sparse public sources. Specific cookie names are per-tenant and not publicly enumerable; the body-marker-only identification strategy (§6.4) reflects this uncertainty.
3. **The IDBA model rotation cadence (§2.4 monthly)** is industry-pattern inference, not a Radware-documented number. The vendor's own communications don't disclose rotation cadence.
4. **The behavioral-signal counts (~250 parameters)** come from a pre-acquisition ShieldSquare marketing source [Radware Medium][radware-medium-js]; the current Radware Bot Manager number may differ.
5. **The `humanize.js` 30-setTimeout count** in §5.2 needs verification by `grep` against the current file at `crates/browser/src/js/humanize.js`. The counts in this chapter were estimated from informal recall; a contributor reading this chapter should verify the actual implementation before relying on the numbers for a customer pitch.
6. **No completed Radware customer-pilot results** are catalogued in tree. The first completed pilot will inform a future revision of §5 with measured pass rates by deployment mode.
7. **The honeypot detection claim (§4.3)** is based on Radware's product marketing; we have no captured evidence of a Radware honeypot serve in tree. The cross-source validation guidance (§8.6) is sound defensive practice regardless.
8. **Vertical concentration claim (§1.3)** is anecdotal from Radware's case studies, not a rigorous market-share number.

Per CLAUDE.md `MEASUREMENT TRAP` discipline: when the first Radware-vs-BO measurement lands, apply the same `L3-RENDERED + size-gate ≥ 15 KB` rule from chapter 03 methodology. Until then, this chapter is **engineering reasoning, honestly labelled as such, suitable for customer pitch decks with the "predicted, pending pilot" hedge**.

The product-positioning lesson: **chapters 31 (Fastly) and 32 (Radware) bracket the non-corpus customer-onboarding space**. Fastly is the easy case; Radware is the hard case; most other non-corpus vendors fall between them on the network-vs-behavioral spectrum. Use this chapter as the template for any future "customer brings vendor X" research note.

---

[radware-wiki]: https://en.wikipedia.org/wiki/Radware
[capterra]: https://www.capterra.com/p/149999/Bot-Prevention-Solution/
[radware-idba]: https://www.radware.com/cyberpedia/bot-management/intent-based-behavioral-analysis/
[radware-idba-quote]: https://www.radware.com/cyberpedia/bot-management/intent-based-behavioral-analysis/
[radware-medium-js]: https://radwarebotmanager.medium.com/how-javascript-injection-helps-in-building-a-comprehensive-bot-detection-solution-for-web-c0d48ea9b927
[radware-bot-manager-cached]: https://www.radware.com/products/bot-manager/
[radware-cyberpedia-bm]: https://www.radware.com/cyberpedia/bot-management/bot-management/
[radware-impl-guide]: https://support.radware.com/app/answers/answer_view/a_id/1029716/~/bot-manager---implementation-guide
[radware-2311]: https://support.radware.com/app/answers/answer_view/a_id/1037013/~/radware-bot-manager-23.1.1.0-
[radware-whitepaper]: https://www.radwarebotmanager.com/web/wp-content/uploads/Radware_Bot_Manager_Ultimate_Guide_Bot_Management.pdf
[radware-anti-scraping]: https://www.radwarebotmanager.com/content-scraping-protection/
[radware-cookie]: https://support.radware.com/app/answers/answer_view/a_id/17136/~/cookie-name-per-content-rule-script-sample
[radware-aws-cf]: https://shieldsquare.atlassian.net/wiki/spaces/SIP/pages/577634308/AWS+CloudFront+Integration
[radware-ios-sdk]: https://github.com/radwarebotmanager/radware-botm-iOS
[radware-android-sdk]: https://github.com/radwarebotmanager/radware-botm-android
[shieldsquare-gh]: https://github.com/ShieldSquare
[alternativeto-shieldsquare]: https://alternativeto.net/software/shieldsquare/about/
