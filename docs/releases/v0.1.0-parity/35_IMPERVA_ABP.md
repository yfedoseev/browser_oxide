# 35 — Imperva Advanced Bot Protection (ABP)

**Status:** reference / customer-onboarding playbook. Imperva is NOT in the 126-corpus today; this chapter exists so a customer onboarding an Imperva-protected site can identify the product, predict BO's behaviour, and triage if BO is blocked.
**Audience:** customer-pitch authors, contributors scoping a new Imperva-protected site for the corpus, anyone reading a sweep result with `_Incapsula_Resource` / `visid_incap_*` / `reese84` markers.
**Companion docs:** `18_ANTI_BOT_VENDOR_COOKBOOK.md §2.7` (the existing one-screen Imperva entry — this chapter is the deep dive), `27_VENDOR_COMPETITIVE_MATRIX.md` (Imperva is the empty row at `27 §1`), `26_AKAMAI_BMP_DEEP.md` (comparable enterprise-tier deep dive), `25_CLOUDFLARE_DEEP.md` (comparable WAF deep dive), `23_TLS_HTTP_FINGERPRINT_REFERENCE.md` (TLS-class context for the Imperva detection signals), `34_PER_VENDOR_TEST_HARNESS.md` (where the future `imperva_reese84_capture` harness slots in).

**One-paragraph thesis:** Imperva Advanced Bot Protection (ABP) is the *premium* bot-management product layered on top of Imperva's WAF and DDoS portfolio. Cookbook 18 §2.7 covers the *WAF-tier* Incapsula behaviour (the `_Incapsula_Resource` JS shield, `visid_incap_*` + `incap_ses_*` cookies); this chapter covers ABP — the post-Distil-acquisition product sold separately to Fortune 500 enterprises, distinguished by the `reese84` cookie, the ~ 200-fingerprint-signal Reese84 JS sensor, and a server-side ML scoring tier with threat-intelligence integration ([Imperva ABP product page](https://www.imperva.com/products/advanced-bot-protection/)). BO's likely posture: standard Reese84-tier bypasses are JS-execution-only and a faithful headless browser passes them; ABP's behavioural-ML scoring is the harder tier where BO's lack of behaviour wiring will likely hurt on customer-tenant sites with `medium`+ sensitivity. The customer-onboarding pitch is **"BO passes most Imperva sites that any open-source headless engine passes; expect block on the most-aggressive ABP tenants until the behaviour wiring lands."**

---

## 1. Product overview

### 1.1 Product lineage

Imperva acquired Distil Networks in July 2019 ([Wikipedia: Imperva — Key Acquisitions](https://en.wikipedia.org/wiki/Imperva)); the Distil bot-management stack became the seed of what is now marketed as Imperva Advanced Bot Protection. Distil's pre-acquisition product was branded "Distil ABM" (Advanced Bot Management) and had been the leading commercial bot-management product for the years leading up to acquisition.

Notable corporate-history beats relevant for understanding the product positioning:

- **Founded 2002** as WebCohort (Shlomo Kramer, Amichai Shulman, Mickey Boodaei). First product 2003: SecureSphere Web Application Database Protection. Rebranded Imperva 2004. ([Wikipedia: Imperva](https://en.wikipedia.org/wiki/Imperva))
- **2011** IPO on NYSE (ticker IMPV).
- **2014** acquired Incapsula — the cloud WAF + DDoS service that today carries the `visid_incap_*` / `incap_ses_*` cookies. Incapsula is the *WAF* tier; ABP is the bot-management tier layered on top.
- **2019** Thoma Bravo took Imperva private ($2.1 B).
- **July 2019** Distil Networks acquisition. Distil's product becomes "Advanced Bot Protection." This is the moment ABP and the Incapsula-WAF-bot-features diverged into two SKUs.
- **December 2023** acquired by Thales Group ($3.6 B). Imperva is now a Thales business unit — see the `docs-cybersec.thalesgroup.com` URL pattern that now serves the historical `docs.imperva.com` documentation (the cookbook 18 §2.7 reference URLs redirect through Thales).
- **~ 1400 employees** as of 2023 acquisition.

### 1.2 Distinguishing ABP from base WAF

Imperva's own product page is explicit about the positioning ([Imperva ABP product page](https://www.imperva.com/products/advanced-bot-protection/)):

> "It's great that you're already using WAF and DDoS protection. But while WAFs and DDoS protection are essential, they often focus on general traffic filtering and volumetric attacks. Advanced bots are more sophisticated and can mimic human behavior, making them difficult to detect with traditional tools."

In practice, the SKU split means:

| Aspect | Incapsula WAF (base) | ABP (premium) |
|---|---|---|
| **Detection** | TLS / JA3 / HTTP signature / OWASP rules / per-tenant rate limits | All of the WAF tier PLUS a deep ML behavioural model + threat-intelligence feed + dedicated JS sensor |
| **Cookies issued** | `visid_incap_<site_id>`, `incap_ses_<n>_<site_id>` | All of the above PLUS `reese84` |
| **JS sensor** | `_Incapsula_Resource?…` script — collects ~ 100 properties | Reese84 sensor — collects 200+ properties, heavily obfuscated client encoder |
| **Block page** | "Request unsuccessful. Incapsula incident ID: …" | Tenant-customised; commonly a captcha or "press to continue" gate |
| **Customer tier** | Mid-market WordPress / e-commerce | Fortune 500: airlines, financial services, large retail |
| **Pricing** | Per-bandwidth | Per-bandwidth + per-detection-tier (negotiated enterprise contract) |

The Imperva product page does not name competitors directly, but the product capability claims position it explicitly against:
- Akamai Bot Manager Premier
- DataDome enterprise
- Cloudflare Bot Management Enterprise
- HUMAN (PerimeterX) Bot Defender Enterprise
- F5 Distributed Cloud Bot Defense (formerly Shape Security)

(Sourced from Imperva's marketed differentiators: "over 700 dimensions" of detection, "real-time policy adaptation", "threat-intelligence feeds" — these are the analyst-report axes Gartner / Forrester evaluate the cohort on; named competitor list from industry-standard MQ/Wave participation rather than from Imperva's site itself.)

### 1.3 Customer segments

From the Imperva ABP product page case-study language (paraphrased):

- **Airlines / travel** — flight-search scraping is the canonical use case; an airline operations manager testimonial appears on the product page.
- **Retail / e-commerce** — Black Friday / inventory scraping; case studies reference holiday-traffic surges.
- **Financial services** — credential stuffing + scraping defense (implicit in the use-case patter; explicit in the threat-research blog).
- **API + mobile app protection** — distinct from the JS-sensor product; ABP also ships a server-side SDK for native-app traffic.

These segments correlate with sites we *would* encounter via customer engagements but that aren't in the 126-corpus today (which is biased toward consumer-facing publishers/retail/social, not enterprise B2B / financial).

---

## 2. Detection architecture

ABP is a layered detection stack. The vendor markets four conceptual layers; the publicly-documented technical layers map roughly:

### 2.1 Layer A — passive request signals

Before any JS runs, ABP scores the request on:

- **TLS fingerprint** — JA3 + JA4 hash. Per [scrapfly Imperva bypass](https://scrapfly.io/blog/posts/how-to-bypass-imperva-incapsula-anti-scraping): "TLS (or SSL) fingerprinting is a modern technique of identifying a client based on the way the client and server negotiate." Imperva specifically checks JA3/JA4.
- **HTTP/2 + HTTP/3 fingerprinting** — header ordering, pseudo-header order, frame settings. Imperva strongly prefers H/2 + H/3 clients; an HTTP/1.1 request to an ABP-protected tenant is itself a small bot signal.
- **IP reputation** — datacenter vs. residential classification via metadata databases. ABP integrates with Imperva's threat-intelligence feed.
- **Client Hints validation** — ABP checks that the `sec-ch-ua` set is internally consistent with the UA string.
- **Header ordering / values** — common detection check across all vendors of this tier.

### 2.2 Layer B — JS sensor (Reese84)

If layer A doesn't issue a confident block, the page response includes the Reese84 sensor script. The script:

1. Loads from a per-tenant path under the protected origin (NOT a third-party CDN — Imperva proxies the script under the tenant's domain to dodge ad-blockers and CSP issues). Common patterns: `/Incapsula_Resource?CWUDNSAI=…&SWKMTFSR=…`, `/<random>/reese.js`, `/_imp/reese.js`.
2. Heavily obfuscated — uses AST transformations that change quarterly. The deobfuscator at [`Thoosje/Reese84-Deobfuscator`](https://github.com/Thoosje/Reese84-Deobfuscator) is "not finished" as of the latest commit, reflecting the difficulty.
3. Collects 200+ signals according to [2captcha's Imperva article](https://2captcha.com/h/imperva-bypass): "obfuscated JavaScript to harvest over 200 device attributes, ranging from Canvas and WebGL rendering to audio contexts."
4. Encrypts the envelope client-side, POSTs to `/_Incapsula_Resource?SWJIYLWA=…` (the validation endpoint).
5. Server returns the `reese84` cookie on a successful score, or a block-page / captcha challenge on rejection.

### 2.3 Layer C — server-side ML behavioural

Once `reese84` issues, subsequent requests are scored on:

- **Behavioural baseline per-site** — ABP trains a per-tenant ML model on the site's organic traffic; deviations score down. Per Imperva's own product copy: "rigorous testing and validation against historical data and hundreds of browsers."
- **Mouse / touch / keystroke streams** — the Reese84 script continues collecting on the page after the initial POST; this stream feeds the ML model.
- **Request cadence / page transition timing** — the same "ML models have become more sophisticated at detecting unnatural browsing patterns" axis the scrapfly article calls out.
- **Honeypot links / fields** — embedded in pages, invisible to humans, ABP-watched. Click → instant block. This is a tenant-config choice, not always-on.

### 2.4 Layer D — threat intelligence + good-bot allowlist

- **Threat intelligence feed** — IP / ASN reputation, known-bad signatures, freshly-rotated proxy lists. ABP enriches its scoring with the Imperva-managed feed.
- **Good-bot allowlist + verification** — Googlebot / Bingbot / etc. are identified via the standard `googlebot-verification` reverse-DNS check, allowed through without sensor.
- **Custom rules per tenant** — tenants set per-path sensitivity (a `/login` path can have a stricter policy than `/products`).

---

## 3. Detection signals (from public research)

Catalogued from the deobfuscation and bypass references cited above:

### 3.1 Browser fingerprint signals

The Reese84 sensor collects (subset — full list is per-bundle-version):

- **Canvas hash** — Picasso-style canvas draw + hash.
- **WebGL** — `UNMASKED_VENDOR_WEBGL` / `UNMASKED_RENDERER_WEBGL` + a renderer probe.
- **AudioContext** — frequency-domain hash of a known waveform.
- **Font enumeration** — flash-fingerprint-style font-existence probing via offsetWidth measurement.
- **Navigator surface** — `userAgent`, `platform`, `language`, `languages`, `hardwareConcurrency`, `deviceMemory`, `maxTouchPoints`, `vendor`, `cookieEnabled`, `webdriver`, `plugins.length`, `mimeTypes.length`.
- **Screen** — `width`, `height`, `availWidth`, `availHeight`, `colorDepth`, `pixelDepth`, `devicePixelRatio`.
- **Window** — `innerWidth`, `innerHeight`, `outerWidth`, `outerHeight`, `chrome` object, `opener`.
- **Document** — `document.referrer`, `document.URL`, presence of `document.documentMode` (IE detect — should be undefined on modern browsers).
- **Timing** — `performance.now()` consistency, `performance.timing` (deprecated but ABP still probes it on tenants that haven't rotated bundles).
- **Function.toString leak detection** — Imperva, like Kasada and Akamai BMP, checks `Function.prototype.toString` returns of known native APIs to detect bootstrap-source leaks.
- **Behavioural primitives at script load** — initial mouse position, time-to-first-touch, scroll velocity.

### 3.2 Behavioural signals (post-Reese84 issue)

- Mouse movement trajectory + acceleration profile.
- Click timing (down→up duration, dwell between consecutive clicks).
- Keystroke cadence (when on form-input pages).
- Page-transition timing.
- Scroll velocity + scroll-end inertia.
- Focus / blur events.

### 3.3 Network signals

- TLS JA3/JA4 (per §2.1).
- HTTP/2 SETTINGS frame ordering.
- TLS extension order (especially for fingerprint-faithful clients).
- IP reputation lookup at every request.

### 3.4 Honeypot signals

ABP supports tenant-configurable honeypots:

- Invisible `<a href="…">` links that should never be clicked.
- Form fields with `style="display: none"` that should never be filled.
- Pages at intentionally-misleading paths (`/admin-do-not-touch/`) — fetching them triggers a block.

A faithful headless browser following only visible / rendered links is robust to most of these; engine + crawler design choices matter.

---

## 4. Detection markers (the cookbook view)

The full marker table for Imperva is in `18 §2.7`. For onboarding convenience, the ABP-specific markers (vs. the base-WAF markers also listed there):

### 4.1 Response headers

| Header | Meaning |
|---|---|
| `x-cdn: Imperva` | All Imperva products (WAF or ABP). |
| `x-iinfo: <encoded>` | Incapsula incident ID — present on blocks AND passes. The encoded string carries the per-incident metadata Imperva support uses for trace lookup. |
| `set-cookie: visid_incap_<site_id>=…` | Base WAF visitor ID. ABP also issues this. |
| `set-cookie: incap_ses_<n>_<site_id>=…` | Base WAF session token. ABP also issues this. |
| `set-cookie: reese84=…` | **ABP-specific** — issued by the Reese84 validation. Strong indicator that ABP (not just base WAF) is in play. |
| `set-cookie: ___utmvc=…` | Some Imperva tenants — Distil-legacy fingerprint cookie, ABP-adjacent. Hyper-SDK includes this alongside reese84 ([`Hyper-Solutions/hyper-sdk-js`](https://github.com/Hyper-Solutions/hyper-sdk-js): `generateReese84Sensor()` + `generateUtmvcCookie()`). |
| `set-cookie: nlbi_<site_id>=…` | Network load-balancer cookie — Imperva infrastructure marker. Pairs with the above. |

### 4.2 Response body markers

| Marker | Strength | Notes |
|---|---|---|
| `_Incapsula_Resource` | unambiguous | The Reese84 / Incapsula JS URL prefix on the protected origin. |
| `?CWUDNSAI=…&SWKMTFSR=…` | unambiguous | The challenge-URL parameter pair — both required, key names rotate quarterly but the pair shape is stable. |
| `?SWJIYLWA=…` | unambiguous | The Reese84 validation POST URL parameter — distinct from the GET challenge URL. |
| `Powered By Incapsula` | strong | Block-page literal on legacy tenants. |
| `Request unsuccessful. Incapsula incident ID:` | unambiguous | The canonical block-page H1 / body text. |
| `<iframe src="//content.incapsula.com/jsTest.html"…>` | unambiguous | A pre-Reese84 challenge variant; less common now. |

### 4.3 The classifier-extension delta

Per `18 §2.7`, none of the Imperva markers are in the engine's `classify.rs:81-156` tables today. The minimal one-line extensions to land:

```rust
// crates/browser/src/classify.rs — extend SMALL_BODY_VENDOR_MARKERS
const IMPERVA_MARKERS: &[(&str, &str)] = &[
    ("_Incapsula_Resource", "Imperva-CHL"),
    ("?CWUDNSAI=",           "Imperva-CHL"),     // challenge URL fingerprint
    ("Incapsula incident ID", "Imperva-Block"),
];

// And cookie-clearance predicate at page.rs:1054-1069
const IMPERVA_CLEARANCE_COOKIES: &[&str] = &["reese84", "visid_incap_", "incap_ses_"];
```

These are the prerequisite for the corpus including Imperva-protected sites (per `19_PROFILE_EXPANSION_PLAN.md` — onboarding sites currently skipped because their CHL state is uncategorised).

---

## 5. Mitigation patterns

What ABP actually *does* to a request flagged by any of layers A-D:

### 5.1 Block (hard)

- Response `403 Forbidden` with the "Request unsuccessful. Incapsula incident ID: …" body.
- No retry path; site-config decision.
- Common on `/login`, checkout, and API endpoints with `high`-sensitivity policy.

### 5.2 Captcha gate

- Response is the captcha page — historically a custom Imperva captcha; more recently, ABP tenants integrate hCaptcha (preferred) or reCAPTCHA.
- User must solve; on solve, ABP issues `reese84` + standard cookies.
- Captcha gate is the most common ABP mitigation for mid-suspicion traffic.

### 5.3 Reese84 cookie acquisition (the "solve" path)

- Most ABP-flagged traffic ends up here: ABP serves the Reese84 challenge inline, expecting the legitimate browser to execute the JS, POST the envelope, get the cookie, and continue.
- This is the path BO can potentially clear with a faithful JS surface — see §8 below.

### 5.4 Rate limit + soft block

- ABP can throttle without blocking — `HTTP 429` with retry-after, repeat-rate limits per-IP, etc.
- This is a tenant-config choice; not common on Fortune 500 deployments (they tend to favour hard-block over throttle).

### 5.5 Silent allow with downgraded service

- ABP can mark a request as "suspicious but not blocked" — tenants then degrade service (don't show real-time inventory, force cached prices, etc.).
- This is the hardest mitigation to detect from the client side; manifests as "scraping works but data is stale."

---

## 6. The Reese84 mechanism — the main solve path

The flow, end-to-end, as documented across the public research:

### 6.1 Initial fetch

1. Client makes a GET for `https://<protected>.com/<path>`.
2. ABP scores the request on layer A (TLS / IP / headers).
3. If score is in the "issue challenge" band:
   - Response status `200`.
   - Response body contains the inline Reese84 sensor script tag — common shapes:
     - `<script src="/_Incapsula_Resource?CWUDNSAI=<tenant_id>&SWKMTFSR=<challenge_token>">`
     - `<script src="/<random_path>/reese.js?_=…">`
   - Response includes the initial cookies `visid_incap_<site_id>` and `incap_ses_<n>_<site_id>` already.

### 6.2 Script execution

4. Browser fetches the Reese84 script (~ 80-150 KB obfuscated JS).
5. Script collects the ~ 200-signal fingerprint envelope (per §3).
6. Script computes a HMAC-like integrity field over the envelope + a challenge nonce parsed from script-load context.
7. Script encrypts the envelope client-side — the encoder is per-bundle-version, AST-obfuscated.

### 6.3 POST to validation endpoint

8. Script POSTs the encrypted envelope to `/_Incapsula_Resource?SWJIYLWA=<validation_token>`.
9. ABP server-side decrypts, scores via layer C (the ML model).
10. If accepted:
    - Response status `200` with body containing the cookie value to set, OR
    - Direct `Set-Cookie: reese84=<base64-blob>` response header.
11. If rejected:
    - Captcha challenge served (response is hCaptcha or reCAPTCHA widget).
    - Or hard `403`.

### 6.4 Subsequent requests

12. Client carries `reese84` + `visid_incap_*` + `incap_ses_*` + `___utmvc` (if applicable) on every subsequent request.
13. ABP server-side validates `reese84` on every request — short-lived (session-bound; minutes to ~ hour depending on tenant).
14. ABP keeps the Reese84 script *on every page*, continuously feeding the behavioural stream (layer C).

### 6.5 Cookie scope + invalidation

- `reese84` cookie is session-scoped (`Domain=<protected>.com; HttpOnly; Secure`).
- Mutating any of the cookie set on the client side invalidates immediately (server-side cross-checks the three).
- Cross-domain: Imperva tenants behind multi-domain configs require per-domain Reese84 acquisition.
- Per [`docs.imperva.com/bundle/advanced-bot-protection/page/75134.htm`](https://docs.imperva.com/bundle/advanced-bot-protection/page/75134.htm) (now serving as `docs-cybersec.thalesgroup.com/...`): the cookie scope documentation is the authoritative reference, but the page does not detail the JS challenge mechanism publicly.

---

## 7. Public solver landscape

A survey of what's publicly available (per the GitHub + commercial-solver search):

### 7.1 Open-source solvers / generators

| Project | What it does | Status |
|---|---|---|
| [`BottingRocks/Incapsula`](https://github.com/BottingRocks/Incapsula) | "Incapsula Payload Generator for Reese84 and __utmvc" — JavaScript-based, generates both cookies | Active 2026, ~ 120 stars |
| [`Hyper-Solutions/hyper-sdk-js`](https://github.com/Hyper-Solutions/hyper-sdk-js) | Commercial SDK: `generateReese84Sensor()`, `generateUtmvcCookie()`, `parseDynamicReeseScript()`. Helper `getSessionIds()`, `isSessionCookie()`. Returns sensor payload + the `swhanedl` companion value | Active, commercial |
| [`Thoosje/Reese84-Deobfuscator`](https://github.com/Thoosje/Reese84-Deobfuscator) | AST deobfuscator for the Reese84 script | Marked "Not finished" — reflects script-obfuscation difficulty |
| [`Thoosje/Reese-Browser-Gen`](https://github.com/Thoosje/Reese-Browser-Gen) | Sandbox gen using undetected-selenium for reese84 | Active |
| [`manudeobs/reese84-rs`](https://github.com/manudeobs/reese84-rs) | Rust generator | Active 2026 (rare — most ecosystem is JS / Python) |
| [`oioie/Incaps_lancet`](https://github.com/oioie/Incaps_lancet) | "An anti-confusion tools of reese84-gen.js" | Active 2025 |
| [`sssynk/reese84_checker`](https://github.com/sssynk/reese84_checker) | Validation-only script — checks if a reese84 token is valid | Useful for harness assertions |
| [`Imbuedhush/Incapsula-Bypass`](https://github.com/Imbuedhush/Incapsula-Bypass) | NodeJS server that bypasses Incapsula WAF (base, not ABP) | Active 2026 |
| [`niespodd/browser-fingerprinting`](https://github.com/niespodd/browser-fingerprinting) | Reference — analysis of bot-protection systems; lists Imperva ABP (former Distil Networks) but minimal Reese84 detail | Active 2026, ~ many stars (reference work) |
| [`RiskByPass/riskbypass_demo`](https://github.com/RiskByPass/riskbypass_demo) | Multi-vendor (Shape / Kasada / PerimeterX / Akamai / DataDome / Imperva) bypass demo | Active 2026 |

### 7.2 Commercial / API solvers

- [2captcha Imperva](https://2captcha.com/h/imperva-bypass) — generic captcha service plus Reese84 hints; not a turn-key Reese84 API.
- [Hyper-Solutions](https://hyper-solutions.com/) — the canonical commercial enterprise solver; exposes `generateReese84Sensor()` via REST API; supports Akamai + Incapsula + Kasada + DataDome.
- [Takion API](https://github.com/Takion-API-Services/TakionAPI-Incapsula-Bypass) — Incapsula + reCAPTCHA combined-bypass service.
- ScrapFly / ZenRows / Bright Data — managed scraping services that abstract Imperva away (they handle the Reese84 acquisition internally).

### 7.3 What this tells us

- **The Reese84 client-encoder is hard but not unbroken.** Multiple independent re-implementations exist; the obfuscation is rotating but tractable.
- **TLS impersonation is often sufficient on its own.** Per [scrapfly](https://scrapfly.io/blog/posts/how-to-bypass-imperva-incapsula-anti-scraping): "JA3/JA4 resistant" libraries are step 1; many tenants pass on TLS alone without needing the Reese84 dance.
- **High false-positive rate for Imperva.** Enterprise customer ops teams frequently complain about Imperva blocking *their own* internal scrapers, monitoring tools, and partner integrations. This is the operational gap that creates Imperva-bypass demand even from legitimate users.
- **The high-trust path is "real browser + clean IP + Reese84 acquired."** Headless engines pass when the JS sensor is satisfied; behavioural-tier bypass is the harder tier and requires the full layer-C dance.

---

## 8. BO coverage — speculative (Imperva sites not in 126-corpus)

Without an Imperva site in the corpus today, this section is informed speculation grounded in the cross-vendor evidence we have.

### 8.1 Where BO is likely to pass

- **Standard fingerprint coverage** — the same coverage that handles Akamai BMP passively (per `26 §`), Kasada's passive collection (per `08 §Phase 2-4`), and DataDome's signal collection. The Reese84 envelope overlaps ~ 80% with these. The fixes already landed for Kasada (`_maskAsNative` sweep per `08 §Lever 3`, CSS calc math per `08 §Lever 2`, the 16 bot1225-class error-field stubs per `08 §Phase 3`) generalise to any vendor that probes the same Web API surface.
- **TLS impersonation via boring2** — Chrome-identical TLS ClientHello + JA3/JA4 (`23 §`). Per scrapfly's bypass guide, TLS-class is often the dominant signal on Imperva. BO's TLS is byte-perfect to Chrome 148; this is the strongest single BO-vs-Imperva edge.
- **Reese84 acquisition via JS execution** — the Reese84 sensor runs as standard JS in V8. The script is a 200-signal collector + encoder; BO's V8 environment is faithful enough to satisfy all 200 signals on the *passive* axes.

### 8.2 Where BO is likely to fail

- **High-sensitivity ABP tenants with behavioural ML** — layer C scoring penalises absence-of-behaviour. BO does not currently wire mouse / scroll / keystroke generation into the navigate path (per the standing "behavior-wiring deferred" memory note from `state_2026_05_17_unblock_execution.md`). On `medium`+ sensitivity ABP tenants this is likely the gating signal.
- **Captcha-gate variants** — if ABP's score band serves hCaptcha, BO does not solve interactive captchas (parity with all other open-source engines per `18 §2.11`).
- **Honeypot-aggressive tenants** — if the tenant has many invisible-link honeypots and BO's crawler logic doesn't perfectly distinguish visible vs. invisible links, it can self-bait into a block.

### 8.3 BO vs. competitors on Imperva (speculative)

By analogy with `27 §1` rankings on adjacent vendors:

| Engine | Expected Imperva posture | Reasoning |
|---|---|---|
| **BO routed** | Passes most ABP-protected sites that any open-source engine passes | TLS class advantage, faithful Reese84 execution |
| **Camoufox** | Comparable — Firefox-class TLS may trigger different Imperva scoring band; net likely parity | Real Firefox engine, behavioural model adapted; no behavioural emission |
| **Patchright / Playwright family** | Mixed — real Chrome trust helps; CDP detection on aggressive ABP tenants likely hurts | Same dynamic as `27 §1` Cloudflare row |
| **Plain curl + reese84 generator** | Passes well via solver SDK | Decouples TLS + Reese84 from browser execution; the dominant production pattern |

---

## 9. Customer onboarding playbook

When a customer says "I want to scrape `<imperva-protected-site>.com`":

### 9.1 Identification

Run the §1.5 decision tree of `18`:

1. `curl -sS -D headers.txt -A "Mozilla/5.0 …" "https://<site>/" -o body.html`
2. Check headers for `x-cdn: Imperva`, `x-iinfo`, `Set-Cookie: visid_incap_*` / `incap_ses_*` / `reese84` / `___utmvc`.
3. Check body for `_Incapsula_Resource`, `?CWUDNSAI=`, `Incapsula incident ID`.

If `reese84` cookie is set → ABP confirmed (not just base WAF).
If only `visid_incap_*` + `incap_ses_*` → base WAF Incapsula; ABP MAY still be configured but at lower sensitivity.

### 9.2 Test BO against the site

1. Add the site to a one-off corpus JSON.
2. Run `target/release/examples/sweep_metrics chrome_148_macos /tmp/imperva_test.json /tmp/out.json` across all 4 profiles.
3. Examine results:
   - **L3-RENDERED + body ≥ 50 KB** on any profile → BO clears Imperva for this site on at least that profile. Recommend customer use the routed multi-profile pattern (`11_PER_PROFILE_STRATEGY.md`).
   - **`Imperva-CHL` classification** (once classifier extended per §4.3) → BO is sitting on the challenge. Triage per §9.3.
   - **403 + "Incapsula incident ID"** → Hard block. Layer A signal triggered before JS. Investigate TLS / IP first.

### 9.3 Triage if blocked

If BO is blocked, the diagnostic loop:

1. **Capture with `sweep_metrics --capture <site>`** (per `04 §`). Examine the 7 artifacts.
2. **Check `fetches.json`** — did the Reese84 script load? Did the POST to `/_Incapsula_Resource?SWJIYLWA=` happen? Did the response set `reese84`?
3. **Compare against Camoufox** — run the same capture via `capture_camoufox.py <site>`. Diff with `capture_diff.py BO_DIR CAMOUFOX_DIR`.
4. **If POST happened but no cookie:** layer C scoring rejected. Behavioural-tier gap. Mitigation: customer accepts limited concurrency / needs to wire behaviour generation (post-v0.1.0).
5. **If POST never happened:** the Reese84 script either threw, didn't execute, or detected a sentinel. Examine `script_errors.json`. Likely candidates: a `Function.toString` leak (per `08 §Phase 3` Lever 3) or a missing Web API stub.
6. **If POST happened, cookie issued, but subsequent requests still 403:** cookie scope / domain mismatch. Verify `Domain` attribute. Check for cross-origin requests requiring per-domain Reese84.

### 9.4 Expected outcomes

- **For ~ 70-80% of Imperva-protected sites** (rough estimate based on cross-vendor parity): BO should pass on at least one of the 4 profiles. Customer use case is viable.
- **For the remaining 20-30%** (high-sensitivity tenants with behavioural-ML scoring): BO likely fails until behaviour wiring lands (post-v0.1.0). Customer recommendation: either accept reduced concurrency (let BO's natural request timing approximate human cadence) or use a commercial Reese84 SDK as a sidecar.

---

## 10. Onboarding Imperva to the 126-corpus

For Imperva to graduate from "speculative coverage" to "measured coverage," the corpus needs Imperva-protected entries. The minimal additions:

1. **Identify 3-5 Imperva-protected sites** suitable for the corpus. Candidates from public industry references:
   - Various airline booking sites (ABP's marketed customer segment).
   - Major retail sites known to use Imperva (industry public sources).
   - Financial-services portals with public read endpoints.
   - Sites listed in the BottingRocks / Hyper-Solutions test fixtures.
2. **Extend `classify.rs`** per §4.3 to recognise Imperva-CHL / Imperva-Block.
3. **Add per-vendor capture harness** — `imperva_reese84_capture` per `34_PER_VENDOR_TEST_HARNESS.md §3`. Spec:
   - **Target site:** one of the above (pinned at harness commit).
   - **POST endpoint pattern:** `/_Incapsula_Resource\?SWJIYLWA=` regex.
   - **Capture method:** JS-level (the Reese84 encoder runs client-side; pre-encryption interception is the easier path than reverse-engineering the AST-rotating encoder).
   - **Decryption needed?** No (intercepting pre-encryption).
   - **Expected fields:** ≥ 80 signal entries in the envelope.
   - **Assertion criteria:** Set-Cookie reese84 observed within ≤ 5 s; envelope decodes; signal-count assertion.
   - **Output path:** `~/projects/browser_oxide_internal/captures/imperva/<YYYY-MM-DD>_<target>/`.
4. **Run baseline measurement** — add Imperva-cluster row to `27 §1` matrix; fill in BO vs. Camoufox vs. Patchright vs. Playwright numbers across the chosen sites × 4 profiles.

This is post-v0.1.0 work; the v0.1.0 deliverable is the deep-dive doc (this one) + the classifier-extension preparation, not new corpus rows.

---

## 11. Forward-looking — Imperva roadmap signals

Public signals about ABP's near-term direction (as of 2026-05-24):

### 11.1 Post-Thales integration

- Thales acquired Imperva in December 2023. The integration is ongoing — public-facing docs URLs now redirect from `docs.imperva.com` to `docs-cybersec.thalesgroup.com`. Product branding remains "Imperva" but the corporate parent is Thales.
- Likely consequences: Thales' threat-intelligence feed (separate from Imperva's pre-acquisition feed) may be merged into ABP's scoring → more aggressive IP-reputation tier.
- Thales' broader security portfolio (CipherTrust, IoT security) may converge with ABP for cross-product device attestation. Not yet visible in product surfaces.

### 11.2 ML model evolution

- Imperva markets "over 700 dimensions" of detection signal. The cadence of new-dimension addition is opaque from outside but observable indirectly via Reese84 bundle size growth (which has trended upward year-over-year per archived bundle samples).
- The behavioural-ML tier (layer C) is the active investment area. Expect more behavioural-axis weight in the next 12-18 months.

### 11.3 Captcha integration shifts

- ABP-tenant captcha defaults have shifted from custom Imperva captchas → hCaptcha (the preferred 2025-2026 integration).
- This trend matches the broader industry — hCaptcha has displaced reCAPTCHA in enterprise B2B WAF integrations.
- For BO, this means captcha-gated paths remain inaccessible regardless of which vendor sits in front (consistent with the `18 §2.11` posture on hCaptcha — needs solver service or human).

### 11.4 The convergence with Thales

- Thales' broader WAF + DDoS portfolio is "layered protection" per `Wikipedia: Imperva`. The Imperva ABP product page positions itself as the dedicated bot tier above this WAF.
- Convergence signals to watch: shared cookie namespaces (`reese84` extending to other Thales products), unified scoring API, cross-product session continuity.

---

## 12. Acceptance + files

### 12.1 Acceptance for v0.1.0

- [ ] This chapter (`35_IMPERVA_ABP.md`) exists and cross-references `18_ANTI_BOT_VENDOR_COOKBOOK.md §2.7`, `27_VENDOR_COMPETITIVE_MATRIX.md` (Imperva row), `34_PER_VENDOR_TEST_HARNESS.md` (future Imperva harness).
- [ ] `18_ANTI_BOT_VENDOR_COOKBOOK.md §2.7` updated to link to this chapter.
- [ ] `27_VENDOR_COMPETITIVE_MATRIX.md` Imperva row marked "see `35_IMPERVA_ABP.md` for the deep dive."
- [ ] `classify.rs` `IMPERVA_MARKERS` + cookie-clearance predicate stubbed (compiles, not yet wired to a target site) — at least the constant arrays exist and are referenced by a unit test.
- [ ] Customer-pitch one-liner in `00_README.md` mentions Imperva alongside the other named vendors.

### 12.2 Acceptance post-v0.1.0

- [ ] Imperva-protected site selected and added to the 126-corpus → 127+-corpus.
- [ ] `imperva_reese84_capture` per-vendor harness implemented per `34_PER_VENDOR_TEST_HARNESS.md`.
- [ ] `27_VENDOR_COMPETITIVE_MATRIX.md §1` Imperva row populated with measured BO vs. competitor numbers.
- [ ] If behaviour wiring lands (post-v0.1.0 follow-on), re-measure Imperva pass rate as the canonical proof that behaviour-tier ABP scoring is the gating signal.

### 12.3 Files referenced

- `crates/browser/src/classify.rs:81-156` — gains `IMPERVA_MARKERS` (per §4.3 above).
- `crates/browser/src/page.rs:1054-1069` — gains `IMPERVA_CLEARANCE_COOKIES` entry in the cookies-carry-clearance predicate.
- `crates/browser/tests/per_vendor_imperva.rs` — NEW (post-v0.1.0), implements `imperva_reese84_capture` per `34 §3`.
- `crates/browser/tests/holistic_sweep.rs` — gains Imperva site entries (post-v0.1.0).
- `docs/releases/v0.1.0-parity/18_ANTI_BOT_VENDOR_COOKBOOK.md §2.7` — primary cross-reference (the one-screen vendor-cookbook entry).
- `docs/releases/v0.1.0-parity/27_VENDOR_COMPETITIVE_MATRIX.md §1` — Imperva row (currently empty placeholder).
- `docs/releases/v0.1.0-parity/34_PER_VENDOR_TEST_HARNESS.md` — sibling chapter; this chapter's `imperva_reese84_capture` spec lives there.
- `~/projects/browser_oxide_internal/captures/imperva/` — capture output root (post-v0.1.0).
- `~/projects/browser_oxide_internal/captures/imperva/DIFF.md` — rotation log (post-v0.1.0).

### 12.4 External references

- [Imperva ABP product page](https://www.imperva.com/products/advanced-bot-protection/) — primary product positioning, customer segments, "over 700 dimensions" marketing claim.
- [Wikipedia: Imperva](https://en.wikipedia.org/wiki/Imperva) — corporate history, acquisitions (Incapsula 2014, Distil 2019, Thales 2023).
- [scrapfly Imperva bypass guide](https://scrapfly.io/blog/posts/how-to-bypass-imperva-incapsula-anti-scraping) — TLS / JA3-JA4 signal detail, headless-browser recommendations.
- [2captcha Imperva content](https://2captcha.com/h/imperva-bypass) — "infamous reese84 cookie," "200+ device attributes" reference.
- [docs-cybersec.thalesgroup.com — ABP cookie scope](https://docs-cybersec.thalesgroup.com/bundle/advanced-bot-protection/page/75134.htm) — official cookie-scope documentation (cookie-scope only; JS challenge mechanism not publicly documented).
- [`BottingRocks/Incapsula`](https://github.com/BottingRocks/Incapsula) — JavaScript Reese84 + ___utmvc payload generator.
- [`Hyper-Solutions/hyper-sdk-js`](https://github.com/Hyper-Solutions/hyper-sdk-js) — commercial SDK; documents `generateReese84Sensor()`, `generateUtmvcCookie()`, `parseDynamicReeseScript()` API surface.
- [`Thoosje/Reese84-Deobfuscator`](https://github.com/Thoosje/Reese84-Deobfuscator) — AST deobfuscator (status: "Not finished" — reflects obfuscation difficulty).
- [`Thoosje/Reese-Browser-Gen`](https://github.com/Thoosje/Reese-Browser-Gen) — undetected-selenium-based Reese84 gen.
- [`manudeobs/reese84-rs`](https://github.com/manudeobs/reese84-rs) — Rust Reese84 generator (rare ecosystem entry).
- [`oioie/Incaps_lancet`](https://github.com/oioie/Incaps_lancet) — Reese84-gen.js anti-confusion tooling.
- [`sssynk/reese84_checker`](https://github.com/sssynk/reese84_checker) — Reese84-token validator.
- [`Imbuedhush/Incapsula-Bypass`](https://github.com/Imbuedhush/Incapsula-Bypass) — NodeJS WAF (not ABP) bypass.
- [`Takion-API-Services/TakionAPI-Incapsula-Bypass`](https://github.com/Takion-API-Services/TakionAPI-Incapsula-Bypass) — commercial Incapsula + reCAPTCHA bypass.
- [`niespodd/browser-fingerprinting`](https://github.com/niespodd/browser-fingerprinting) — reference analysis ("Advanced Bot Protection by Imperva (former Distil Networks)").
- [`RiskByPass/riskbypass_demo`](https://github.com/RiskByPass/riskbypass_demo) — multi-vendor bypass demo including Imperva.
- [thewebscrapingclub Bypassing PerimeterX 3](https://substack.thewebscraping.club/p/the-lab-56-bypassing-perimeterx-3) — adjacent vendor walkthrough (PerimeterX), cited for the headless-engine + behavioural-tier pattern that applies to ABP.

---

## 13. Comparative posture against adjacent vendors

To set customer expectations correctly, here is how ABP compares to the other premium-tier products documented in the cookbook (`18 §2`):

### 13.1 ABP vs. Akamai BMP

| Axis | Imperva ABP | Akamai BMP |
|---|---|---|
| **Lineage** | Distil Networks (2019) | In-house Akamai (originally 1.7, then v2, now v3) |
| **JS sensor** | Reese84 — single concatenated bundle, AST-rotating | sensor_data v2 (TEA-CBC) / v3 (PRNG-JSON, cookie-keyed) |
| **Cookie** | reese84 (session) + visid_incap_* (persistent) | `_abck` (persistent, score-bearing infix) + `bm_sz` (session) |
| **ML scoring tier** | Yes — explicit "behavioural ML" marketing | Yes — risk model adapted from Akamai's broader CDN-scale traffic |
| **Threat-intel feed** | Imperva-managed + Thales (post-2023) | Akamai-managed (CDN-scale, the largest in the industry) |
| **Customer overlap** | Airlines, finance, retail | Same — significant overlap in Fortune 500 |
| **BO posture** | Expected pass on most non-behavioural tenants | Pass on adidas (firefox-uniquely); fail on homedepot (sec-cpt) / bestbuy (Akamai SPA shell) |

Both vendors target the same enterprise tier and have similar detection-layer composition; ABP is slightly more aggressive on TLS / JA3-JA4 scoring, BMP is more aggressive on persistent-identity tracking via the long-lived `_abck`.

### 13.2 ABP vs. DataDome

| Axis | Imperva ABP | DataDome |
|---|---|---|
| **Lineage** | Distil (2019) acquisition into Imperva | French independent vendor |
| **Architecture** | Inline JS sensor + server ML | Cross-origin iframe with WASM-derived daily-rotating key |
| **JS dependency** | Yes — Reese84 script runs on every page | Yes — dd-script + iframe |
| **Cookie** | reese84 (session) | datadome (rotating, set on every response) |
| **Captcha** | hCaptcha integration (preferred) | Custom DataDome captcha (Picasso canvas + GeeTest variant) |
| **BO posture** | Expected pass on Reese84 tier (faithful JS), block on behavioural tier | Recoverable via the 3 primitives in `07 §`; etsy + tripadvisor on the recovery path |

DataDome's daily-rotating WASM is the harder reverse-engineering target; ABP's AST-rotating JS encoder is harder to *deobfuscate* but easier to *execute* (because we just need JS-faithful execution, not key recovery).

### 13.3 ABP vs. PerimeterX (HUMAN Security)

| Axis | Imperva ABP | PerimeterX |
|---|---|---|
| **Lineage** | Distil (2019) | Acquired by HUMAN Security |
| **Cookie lifetime** | Session-bound reese84 (minutes to hour) | `_px3` (60 s) — designed to require constant refresh |
| **Behavioural emphasis** | High but not central | Central — the 60-s cookie design forces continuous interaction |
| **BO posture** | Expected pass on Reese84 tier | BO passes zillow (`27 §2` — the marquee BO advantage); behavioural tier still untested in corpus |

PerimeterX's design choice of 60-s `_px3` is the most aggressive behavioural-pressure model in the cohort; ABP's session-bound reese84 is moderate in comparison. BO's zillow win on PerimeterX suggests our TLS-class advantage propagates to ABP-equivalent scenarios.

### 13.4 ABP vs. Cloudflare Bot Management

| Axis | Imperva ABP | Cloudflare Bot Management |
|---|---|---|
| **Distribution** | Per-tenant, customer-installed | CDN-default + per-tenant tuning |
| **Detection signal weight** | Behavioural ML > TLS > network | TLS + behavioural roughly even, plus the unique CF-Bot-Score model |
| **Captcha** | hCaptcha | Cloudflare Turnstile (also hCaptcha-aligned) |
| **BO posture** | Speculative pass | Mixed — `27 §1` shows BO routed wins 7/7 of iphone-targeted CF Managed Challenge sites Camoufox also wins |

The structural difference: CF Bot Management is *the* bot model trained on CDN-scale traffic; ABP is trained on per-tenant traffic. CF detects bots at the network level *before* any JS challenge; ABP relies on the JS sensor to be its primary signal. BO's strength is faithful JS surface — better-suited to ABP than to CF Bot Management's network-tier model.

---

## 14. Open research questions

Filed against `15_OPEN_QUESTIONS.md` as adjacent entries (this chapter expands the per-question rationale):

1. **What is the relative weight of the behavioural-ML tier vs. the Reese84-sensor tier in ABP scoring?** Without an ABP site in the corpus, we don't know if BO's missing-behaviour gap is the gating signal or merely a contributing factor. Resolvable by adding 3-5 ABP sites to the corpus + measuring per-profile.
2. **Does the post-Thales threat-intelligence-feed merger change ABP's IP-reputation aggressiveness?** Public signals suggest yes but unverified. Resolvable by re-measuring 6-12 months after a merger announcement.
3. **What is the v0.1.0 ↔ post-behaviour-wiring delta on ABP pass rate?** This is the canonical test of the "behaviour wiring is the load-bearing fix" hypothesis from the standing memory note. If ABP pass rate jumps materially when behaviour lands, the hypothesis is confirmed.
4. **Are honeypot links / fields actually a major blocker in ABP today?** Public references say "yes," but our crawler is link-following with no DOM-aware visibility filter — we'd need to instrument to know.
5. **Does the `reese84` cookie carry usefully across BO profiles within a single session?** If the cookie is bound to one Reese84 envelope hash, profile-switching invalidates immediately. Worth measuring via the capture harness once it ships.

---

## 15. Cross-references

- `18_ANTI_BOT_VENDOR_COOKBOOK.md §2.7` — the one-screen Imperva entry (this chapter is the deep dive).
- `27_VENDOR_COMPETITIVE_MATRIX.md §1` — Imperva row (currently empty; populate post-v0.1.0).
- `34_PER_VENDOR_TEST_HARNESS.md` — `imperva_reese84_capture` harness spec extension.
- `26_AKAMAI_BMP_DEEP.md` — comparable enterprise-tier deep dive (sister product Akamai BMP).
- `25_CLOUDFLARE_DEEP.md` — comparable enterprise-tier deep dive (sister product Cloudflare Bot Management).
- `23_TLS_HTTP_FINGERPRINT_REFERENCE.md` — TLS-class context (Imperva's layer-A heavy-weights this dimension).
- `19_PROFILE_EXPANSION_PLAN.md` — corpus growth path; Imperva entries are one expansion target.
- `08_KASADA_FRONTIER.md §Lever 3` — the `_maskAsNative` audit that benefits the Reese84 surface as a side-effect.
- `04_TOOLING_SPEC.md` — `--capture` mode used in §9.3 triage.
- `11_PER_PROFILE_STRATEGY.md` — multi-profile routing recommended in §9.2 customer guidance.
- `15_OPEN_QUESTIONS.md` — adds the open question "When does the behavioural-ML tier of ABP become the gating signal vs. the Reese84-sensor tier?"
