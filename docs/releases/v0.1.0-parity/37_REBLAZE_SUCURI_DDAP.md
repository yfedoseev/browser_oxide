# 37 — Reblaze + Sucuri + DataDome Account Protect

**Status:** reference (deeper-than-cookbook treatment for three vendors not in the 126-corpus + DataDome's distinct ATO product line)
**Audience:** anyone scoping a new customer whose target falls outside the corpus that 06/07/08/25/26 already cover; cross-vendor-pattern readers preparing for chapter 42.
**Companion docs:** `18_ANTI_BOT_VENDOR_COOKBOOK.md` (cookbook §2.7 covers Reblaze briefly, §2.8 covers Sucuri briefly — this chapter extends both), `07_DATADOME_PRIMITIVES.md` (web-side DataDome — this chapter covers DataDome's *Account Protect* product specifically), `27_VENDOR_COMPETITIVE_MATRIX.md`, `36_ATO_SPECIALISTS.md` (the companion chapter covering Castle / Sift / Forter / Akamai AP), `26_AKAMAI_BMP_DEEP.md` (BMP — Akamai AP analog to DD-AP).

**One-paragraph thesis:** Three vendors with different positioning that share one engine-side property: **none are blocking responses in the 126-site corpus.** Reblaze (now Link11-owned, dedicated-VPC-per-customer "Cloud Native WAAP") is the upper-tier of this group — sophisticated behavioral detection, capability parity with DataDome / Akamai BMP, but limited public surface and zero open-source bypass corpus. Sucuri is the bottom-tier (SMB-focused, $9.99/mo entry pricing, sub-Cloudflare sophistication), and per `18 §2.8` is "not blocking; passes via the standard navigate loop." DataDome Account Protect is the ATO-specific product line distinct from DataDome web bot protection (chapter 07) — same telemetry, different decision target. Together this chapter closes a documentation gap: a customer landing on a Reblaze / Sucuri / DD-AP target should find the answer to "what is this and what does BO do about it" within 10 minutes of reading.

---

## 1. Reblaze (deeper)

### 1.1 One-line + history

Reblaze (reblaze.com) is a cloud-native WAAP (Web Application + API Protection) platform founded ~2011 in Israel, acquired by Link11 on **2024-01-30** (per [Link11 press release](https://www.link11.com/en/blog/press/link11-grows-internationally-reblaze-technologies-becomes-part-of-the-link11-group/) + [FinSMEs](https://www.finsmes.com/2024/01/link11-acquires-reblaze-technologies.html)). The current documentation is hosted under Link11's domain as "Link11 WAAP" (per [waap.docs.link11.com](https://waap.docs.link11.com/v2.16/introduction-to-reblaze) + [gb.docs.reblaze.com](https://gb.docs.reblaze.com/introduction-to-reblaze)). The product is sold both standalone and as the WAF+bot tier of Link11's broader DDoS-protection stack.

The product set, per Link11's own materials:
- Next-gen WAF
- Bot management
- API security
- Account-takeover prevention
- Biometric + behavioral threat detection
- DDoS (layer 3/4/7) — gained via the Link11 merger

A key architectural differentiator versus Cloudflare / Akamai / Imperva: **dedicated VPC per customer.** Per Link11's product brief and [WAFPlanet review](https://wafplanet.com/waf/reblaze/), Reblaze deploys a complete autoscaling stack of "Reblazers" (security logic units) per customer inside AWS, GCP, or Azure VPCs — not shared multi-tenant. The customer pays a meaningful premium for this isolation; the security argument is "no cross-tenant data leakage, your traffic never co-located with another customer's."

### 1.2 Architecture

```
                    [public internet]
                          │
                          ▼
              ┌─────────────────────────────┐
              │  Anycast DNS / DDoS edge    │  (Link11's network)
              └─────────────────────────────┘
                          │
                          ▼
              ┌─────────────────────────────┐
              │  Customer-dedicated VPC     │
              │  ┌───────────────────────┐  │
              │  │ Reblazer autoscale    │  │
              │  │ group (security logic)│  │
              │  │  - WAF rules          │  │
              │  │  - bot detection      │  │
              │  │  - rate-limiter       │  │
              │  │  - JS challenge serve │  │
              │  └───────────────────────┘  │
              │             │               │
              │             ▼               │
              │       Origin app            │
              └─────────────────────────────┘
```

The "Reblazers" are the decision unit. Per Link11's docs, requests flow:
1. Anycast edge (DDoS scrub).
2. Routed to the customer's dedicated VPC.
3. Reblazer autoscale group inspects the request against the per-customer policy.
4. Decision: bypass / deny / allow / challenge / rate-limit.

### 1.3 The "Mc Cohen" reference

The doc-36 brief mentioned a "Mc Cohen module" as the bot-decision engine. **I cannot confirm this name in Reblaze / Link11 public documentation as of 2026-05-24.** Searches against [reblaze.com](https://www.reblaze.com/), [docs.reblaze.com](https://docs.reblaze.com/), [waap.docs.link11.com](https://waap.docs.link11.com/), [link11.com](https://www.link11.com/), Tracxn, Crunchbase, and the merger press releases turn up zero hits for "Mc Cohen" or "McCohen" as a Reblaze product/module name. The closest internal name I find is generic — "Reblazers" (the autoscale logic units, per the VPC arch). It's possible "Mc Cohen" is an internal codename used in older product collateral or sales material not indexed publicly; it is **not** a customer-facing module identifier in the 2026 product line.

Treat this as: **research item — if a customer escalation surfaces an "Mc Cohen" reference, capture the source and update this section.** Honest uncertainty preserved.

### 1.4 Detection signals

Per [reblaze.com / wiki / WAF](https://www.reblaze.com/wiki/waf/) + [gb.docs.reblaze.com](https://gb.docs.reblaze.com/introduction-to-reblaze) + [roundproxies / Reblaze bypass](https://roundproxies.com/blog/bypass-reblaze/):

| Signal class | What it covers |
|---|---|
| **JS-injected telemetry** | Browser environment fingerprinting (canvas, WebGL, navigator, screen, timezone, plugins, audio context). The JS is dropped on every protected page; collected data feeds the bot-decision logic. |
| **Behavioral / biometric** | Mouse movements, click patterns, scroll behavior, typing speed, session timing. Reblaze markets this as "Passive Challenges with Biometric Detection" — the third tier of their protection escalation (basic ACL → JS challenge → behavioral). |
| **IP / reputation** | Datacenter IP detection, VPN/proxy classification, Tor exit nodes, ASN reputation, geo. |
| **TLS / HTTP fingerprint** | Implied by the bypass blogs' emphasis on "TLS Fingerprint Spoofing" as a bypass method — Reblaze inspects ClientHello fingerprints. Not explicitly enumerated by Link11 docs. |
| **Custom rules per customer** | Per the dedicated-VPC architecture, each customer can deploy custom WAF rules + per-endpoint policies. This is the highest source of cross-tenant variability. |
| **`navigator.webdriver` + headless markers** | Per the bypass blog: "checks for automation markers like `navigator.webdriver` and headless browser signatures." |

The per-customer custom-rule capability makes Reblaze more variable than Cloudflare or DataDome in practice — what works against `customer-A.com` may not work against `customer-B.com` because the WAF rules differ. This is structurally similar to AWS WAF (chapter 06) where the per-tenant tuning is decisive.

### 1.5 Detection markers — the engine seam

Per `18_ANTI_BOT_VENDOR_COOKBOOK.md §1.1 + §1.3 + §1.4`:

| Marker | Type | Notes |
|---|---|---|
| `x-armor-shield-zone` | response header | Per cookbook §1.1; documents the Imperva/Thales-era ASZ marker (cf. cookbook §2.7 which groups Reblaze with Imperva under the legacy Thales acquisition note — both products did briefly co-exist under Thales until the Link11 deal) |
| `x-rbzid` | response header | Per [roundproxies / Reblaze bypass](https://roundproxies.com/blog/bypass-reblaze/): the request-id header set on every Reblaze-mitigated response |
| `Set-Cookie: rbzid=...` | response cookie | The session-clearance cookie issued after a successful challenge. Subsequent requests must carry `rbzid`. |
| `Set-Cookie: rbzsessionid=...` | response cookie | Additional session-tracking cookie; observed alongside `rbzid` |
| `mc_session` | response cookie | Per cookbook §1.4 (cookie table): legacy Reblaze cookie still set on some deployments — appears in older documentation alongside the Imperva grouping |
| `sucuri-style block page` body | body | Reblaze's block page brand and copy; not yet cataloged as a body marker in our `classify.rs` |
| `mc.cohen.io` or similar JS endpoint | body / network | The doc-36 brief mentioned `mc.cohen.io` as a bot-fingerprint endpoint. **I cannot verify this domain in any Reblaze documentation.** No DNS, no public corpus mention. If the brief author had a private capture this should be cross-referenced; otherwise treat as unverified. |

The verified-current markers (`x-rbzid`, `rbzid` cookie, `rbzsessionid` cookie) are sufficient for the vendor-detect logger seam at `crates/browser/src/page.rs:1054-1069`. The cookbook already lists `x-armor-shield-zone` and `mc_session` (per §1.1 and §1.4); the additions to make in §6 below are explicit `rbzid` recognition.

### 1.6 Challenge mechanism

Per [WAFPlanet / Reblaze review](https://wafplanet.com/waf/reblaze/) + the Link11 docs:

1. Edge receives request; runs ACL + WAF rules.
2. If rules trigger, escalate to JS challenge: serve a small interstitial that runs the Reblaze JS bootstrap.
3. JS collects browser fingerprint + behavioral signals, POSTs back to Reblaze.
4. On pass: edge sets `rbzid` + `rbzsessionid` cookies; redirects back to the original request.
5. Subsequent requests carry the cookies → fast-path through edge.
6. If JS challenge fails or is bypassed: serve CAPTCHA (Reblaze's own integrated CAPTCHA, not Google reCAPTCHA / hCaptcha by default).
7. If CAPTCHA fails or is suspicious: rate-limit or block (403).
8. **Honeytrap** (Reblaze's term, per Link11 docs): a deceptive endpoint or hidden link that, if hit by a non-human, instantly flags the visitor. Conceptually similar to Radware's honeypot (which is documented in `18 §1.3` only by extension; Reblaze's variant is named differently).

### 1.7 Mitigation patterns

The customer chooses, per Reblaze policy:
- **Allow** — request proceeds.
- **Block** (403) — request denied at edge.
- **CAPTCHA** — Reblaze's integrated CAPTCHA (some tenants also configure hCaptcha as the second-stage fallback).
- **Honeytrap** — silently mark the visitor as bot, downstream rate-limit or block subsequent requests.
- **Rate limit** — slow the request rate at the IP / session level.

### 1.8 BO coverage — speculative

| Scenario | Outcome | Confidence |
|---|---|---|
| Reblaze-protected site in BO's 126-corpus | **N/A** — no Reblaze-confirmed site in the corpus (per `27 §1`) | high |
| Hypothetical Reblaze-protected target with default ACL + basic JS challenge | **LIKELY PASS** — BO's faithful Chrome 148 JS surface + byte-perfect TLS clears Cloudflare-class JS challenges (per `25_CLOUDFLARE_DEEP.md`); Reblaze's mechanism is similar | medium (cannot measure without a live target) |
| Hypothetical Reblaze-protected target with full biometric (mouse/keystroke) detection enabled | **LIKELY FAIL** — BO's humanize.js (per `crates/browser/src/js/humanize.js`) is sparse mouse synthesis only; no keystroke cadence, no scroll-pattern shaping | medium |
| Reblaze with custom per-customer WAF rule that fingerprints Chrome-148 minus-some-feature | **VARIABLE** — depends entirely on the rule. Same risk as AWS WAF (chapter 06): per-tenant tuning is decisive. | low (cannot generalize) |

The honest answer: **BO probably clears low-tier Reblaze deployments and probably fails high-tier (full biometric).** No empirical data — first customer with a Reblaze target is the experiment.

### 1.9 Public solver landscape

- [roundproxies / Reblaze bypass 2026](https://roundproxies.com/blog/bypass-reblaze/) — generic bypass guide enumerating 7 methods (residential proxies, header crafting, Puppeteer Stealth, Nodriver, behavioral emulation, TLS fingerprint spoofing). No Reblaze-specific exploit code.
- [WAFPlanet / Reblaze review](https://wafplanet.com/waf/reblaze/) — neutral product-review framing.
- [Gartner Peer Insights / Link11 Reblaze](https://www.gartner.com/reviews/market/api-protection/vendor/link11/product/reblaze) — analyst reviews; commercial framing.
- GitHub: searches for "reblaze bypass" or "rbzid bypass" return ~zero notable open-source projects.

**Landscape: minimal open-source surface; commercial bypass services exist (ZenRows, Scrapfly) but their efficacy against high-tier Reblaze is not measurable from outside.**

### 1.10 Sources

- [reblaze.com main](https://www.reblaze.com/)
- [reblaze.com / go / WAF](https://www.reblaze.com/go/waf/)
- [reblaze.com / wiki / WAF](https://www.reblaze.com/wiki/waf/)
- [reblaze.com / wiki / WAF category](https://www.reblaze.com/wiki/category/waf/)
- [docs.reblaze.com main](https://docs.reblaze.com/)
- [gb.docs.reblaze.com / intro](https://gb.docs.reblaze.com/introduction-to-reblaze)
- [waap.docs.link11.com v2.16 intro](https://waap.docs.link11.com/v2.16/introduction-to-reblaze)
- [Link11 press release — Reblaze acquisition](https://www.link11.com/en/blog/press/link11-grows-internationally-reblaze-technologies-becomes-part-of-the-link11-group/)
- [Reblaze press release — joining Link11](https://www.reblaze.com/press-release/reblaze-becomes-part-of-link11/)
- [FinSMEs — Link11 acquires Reblaze 2024-01](https://www.finsmes.com/2024/01/link11-acquires-reblaze-technologies.html)
- [Bright Pixel Capital — Reblaze acquisition](https://brpx.com/reblaze-technologies-becomes-part-of-the-link11-group/)
- [Mainsights — Link11 acquires Reblaze](https://www.mainsights.io/ma-news/link11-backed-by-pride-capital-acquires-reblaze-to-bolster-global-cybersecurity-presence)
- [WAFPlanet — Reblaze review](https://wafplanet.com/waf/reblaze/)
- [Gartner Peer Insights — Link11 Reblaze](https://www.gartner.com/reviews/market/api-protection/vendor/link11/product/reblaze)
- [PeerSpot — Reblaze reviews](https://www.peerspot.com/products/reblaze-part-of-link11-reviews)
- [Tracxn — Reblaze company profile](https://tracxn.com/d/companies/reblaze/__hvwQqOXiMhZcA7VP3qXKzgik04LA4upWNB5GxdbwjFA)
- [Software Advice — Reblaze](https://www.softwareadvice.com/bot-detection-and-mitigation/reblaze-profile/)
- [LinkedIn — Reblaze WAF product](https://www.linkedin.com/products/reblaze-the-web-application-firewall/)
- [roundproxies — bypass Reblaze 2026](https://roundproxies.com/blog/bypass-reblaze/)

---

## 2. Sucuri (deeper)

### 2.1 One-line + history

Sucuri (sucuri.net) is a WordPress-focused web-security service founded 2010, acquired by GoDaddy in 2017 (still operated as Sucuri brand). Originally a malware-cleanup + WordPress security plugin company; expanded into CDN + WAF + DDoS over the 2014-2018 period. Customer profile is **SMB-heavy** — pricing starts at $9.99/mo (Basic Firewall) and tops out at $549/year (Business Platform) per [sucuri.net / website-firewall](https://sucuri.net/website-firewall/). Compare with Cloudflare (free → $200+/mo Pro tier → enterprise), DataDome (enterprise-only), Akamai (enterprise-only) — Sucuri is the only vendor in this report priced for individual bloggers.

The architectural lineage: **Cloudflare-like reverse-proxy CDN + WAF**, but with a smaller global footprint and lower-sophistication detection. Sucuri does *not* run a JS-execution-required interstitial by default (unlike Cloudflare Managed Challenge or DataDome) — most blocks are signature-based or IP-reputation-based at the edge.

### 2.2 Architecture

```
                    [public internet]
                          │
                          ▼
              ┌─────────────────────────────┐
              │  Anycast CDN edge           │
              │  (Sucuri/Cloudproxy)        │
              │  - signature WAF            │
              │  - IP reputation            │
              │  - rate-limit               │
              │  - virtual patching         │
              │  - (optional) JS shield     │
              └─────────────────────────────┘
                          │
                          ▼
                    Origin server
```

The "JS shield" (`sucuri_cloudproxy_js`) is the only client-side challenge mechanism and per [ZenRows / Sucuri bypass](https://www.zenrows.com/blog/sucuri-bypass) is *lightweight*: it's a base64-decoded cookie-set + form-submit + reload, no heavy JS-execution dependency, no behavioral fingerprinting. The original 2014-era js2py-based bypass repos still work against many deployments today.

### 2.3 Detection signals

Per [sucuri.net / website-firewall](https://sucuri.net/website-firewall/) + [docs.sucuri.net / block HTTP cookies](https://docs.sucuri.net/website-firewall/whitelist-and-blacklist/block-http-cookies/):

- **Signature-based WAF** — patterns matching SQLi, XSS, RFI/LFI, etc. Same playbook as ModSecurity rulesets.
- **Heuristic rules** — generic anti-spam, generic anti-scraping heuristics.
- **IP reputation** — datacenter, VPN, Tor, abuse-score IPs.
- **Rate limiting** — per-IP, per-session.
- **Virtual patching** — fixed-pattern rules for known CMS vulnerabilities (WordPress + Joomla + Drupal).
- **Optional 2FA / CAPTCHA on protected pages** — customer can require interactive verification on specific paths (admin login, contact form).

**Notably absent vs higher-tier vendors:**
- No browser-environment fingerprinting (canvas, WebGL, audio context).
- No behavioral / biometric analysis.
- No ML scoring.

### 2.4 Detection markers — the engine seam

Per `18 §1.1 + §1.3 + §1.4` + the cookbook §2.8 section:

| Marker | Type | Notes |
|---|---|---|
| `x-sucuri-id: <…>` | response header | Per cookbook §1.1; the unambiguous Sucuri identifier |
| `x-sucuri-cache: HIT \| MISS \| BYPASS` | response header | Cache-state indicator; appears on Sucuri-cached responses |
| `server: Sucuri/Cloudproxy` | response header | The Server header on Sucuri-fronted traffic |
| `sucuri_cloudproxy_js` | body marker | Per cookbook §1.3; appears in the JS shield interstitial |
| `Sucuri WebSite Firewall` | body marker | Brand text on the block page |
| `sucuri_cloudproxy_uuid_*` | cookie | Per cookbook §1.4; per-protected-site UUID set after clearing the JS shield |

The engine already detects most of these (per cookbook §4 logger code at lines 857-867 of the cookbook source, mirrored at `crates/browser/src/page.rs:1054-1069`). Coverage is good; no additions needed.

### 2.5 Challenge mechanism

Per [ZenRows / Sucuri bypass](https://www.zenrows.com/blog/sucuri-bypass) + [GitHub xcscxr/sucuri-cloudproxy-cookie](https://github.com/xcscxr/sucuri-cloudproxy-cookie):

1. Origin proxied by Sucuri returns a small (1-3 KB) interstitial when the request is suspicious.
2. Interstitial contains a `<script>` block that base64-decodes a value and sets a `sucuri_cloudproxy_uuid_*` cookie.
3. After cookie set, a `<meta http-equiv="refresh">` or `window.location.reload()` reloads the page.
4. The reload request now carries the cookie → Sucuri edge lets it through.
5. **Most aggressive variant:** hCaptcha or reCAPTCHA gate. Customer-configurable; not default.

The mechanism is *embarrassingly simple* by 2026 standards — comparable to Cloudflare's 2015-era `cf_clearance` cookie issuance with a 5-second wait. Any browser engine that runs JS and respects cookies clears it.

### 2.6 Mitigation patterns

- **Block** (403 with Sucuri block-page body).
- **JS shield** (the cookie-set interstitial).
- **CAPTCHA** (optional, customer-configurable).
- **Rate limit** (per-IP).

### 2.7 BO coverage assessment

| Scenario | Outcome | Confidence |
|---|---|---|
| Public-page scrape on a Sucuri-protected site (default config) | **PASS** — BO's standard navigate loop handles the cookie-set + reload pattern faithfully | high (per `18 §2.8`: "Not blocking; passes via the standard navigate loop.") |
| Sucuri-protected site with hCaptcha / reCAPTCHA gate enabled | **FAIL** — same constraint as `18 §2.9`: BO does not solve interactive CAPTCHAs in public engine | high |
| Sucuri block based on datacenter IP reputation | **FAIL** — IP-reputation blocks are out of scope for an engine-side fix; mitigated only by proxy choice | high |
| Sucuri site with custom rule blocking BO's Chrome 148 stealth profile (unlikely — Sucuri does not browser-fingerprint by default) | **theoretical / never observed** | low |

**Sucuri is the easiest of the vendors in this report.** It is the *only* vendor in this chapter that BO clears with zero special-casing. The cookbook's framing "Not blocking; passes via the standard navigate loop" is accurate.

### 2.8 Public solver landscape

The Sucuri JS shield is the only major anti-bot mechanism where mature, working open-source bypasses exist:

- [xcscxr / sucuri-cloudproxy-cookie](https://github.com/xcscxr/sucuri-cloudproxy-cookie) — script to obtain the cookie programmatically.
- [pat6969 / Sucuri-Cloudproxy-Bypass](https://github.com/pat6969/Sucuri-Cloudproxy-Bypass) — 5-minute Python script using js2py.
- [github topic / sucuri-bypass](https://github.com/topics/sucuri-bypass) — multiple repos.
- [ZenRows / Sucuri bypass](https://www.zenrows.com/blog/sucuri-bypass) — commercial reference write-up.
- [NoahCardoza / CloudProxy](https://github.com/NoahCardoza/CloudProxy) — broader anti-cloud-WAF proxy, includes Sucuri patterns.

**Landscape: mature, low-friction.** Anyone implementing a Sucuri bypass starts here.

### 2.9 Sources

- [sucuri.net main](https://sucuri.net/)
- [sucuri.net / website-firewall](https://sucuri.net/website-firewall/)
- [docs.sucuri.net / website-firewall / block HTTP cookies](https://docs.sucuri.net/website-firewall/whitelist-and-blacklist/block-http-cookies/)
- [GitHub — xcscxr/sucuri-cloudproxy-cookie](https://github.com/xcscxr/sucuri-cloudproxy-cookie)
- [GitHub — pat6969/Sucuri-Cloudproxy-Bypass](https://github.com/pat6969/Sucuri-Cloudproxy-Bypass)
- [GitHub topic — sucuri-bypass](https://github.com/topics/sucuri-bypass)
- [ZenRows — Sucuri bypass 2026](https://www.zenrows.com/blog/sucuri-bypass)
- [GitHub — NoahCardoza/CloudProxy](https://github.com/NoahCardoza/CloudProxy)
- [Scrapinghub splash issue #547](https://github.com/scrapinghub/splash/issues/547)

---

## 3. DataDome Account Protect

### 3.1 One-line + history

DataDome Account Protect (DD-AP) is DataDome's ATO-specific product line, launched 2023, distinct from DataDome's web bot protection (chapter 07). Per [datadome.co / products / account-takeover-protection](https://datadome.co/products/account-takeover-protection/) + [docs.datadome.co / account-protect](https://docs.datadome.co/docs/account-protect): DD-AP "stops account fraud from the first attempt" by collecting signals to create a digital footprint of user behavior and quickly block malicious activity. Recently (2025-2026) refreshed with two new AI models per [DataDome changelog](https://datadome.co/changelog/how-new-ai-models-strengthen-account-security/) — "designed to adapt in real time and safeguard against both automated and hybrid threats."

It is also available on AWS Marketplace as a standalone product (per [AWS Marketplace listing](https://aws.amazon.com/marketplace/pp/prodview-vyug7zaqjyxv6)).

### 3.2 DataDome web vs DataDome Account Protect

The two products share telemetry but differ in decision target:

| Dimension | DataDome web (chapter 07) | DataDome Account Protect |
|---|---|---|
| **Protects what?** | every HTTP request to public pages | login / signup / password-reset / account-action endpoints |
| **Decision target** | "is this client a bot?" | "is this credential / account-action being used by its legitimate owner?" |
| **JS Tag** | DataDome JS tag on every page (`dd-script.js`) | **Same JS tag** (per [docs.datadome.co / account-protect](https://docs.datadome.co/docs/account-protect): account-protect "is combined with any server-side integration from DataDome" — the JS Tag is shared) |
| **Server integration** | server-side module evaluates the request against the DD edge decision | DD-AP-specific endpoint, evaluating identity + behavior + history |
| **Telemetry** | device fingerprint + behavioral signals + IP + headers | **superset**: all DD-web signals + login-page interaction patterns + per-account historical rate (login attempts, request rate, session activity) + dynamic thresholds via the 2025 AI models |
| **Challenge mechanism on block** | `dd-script` interstitial with `geo.captcha-delivery.com` slider | safeguards triggered: password reset, MFA, sometimes hCaptcha (per the DataDome→hCaptcha migration docs) — customer-controlled enforcement |
| **In-corpus presence** | yes (etsy, tripadvisor, yelp, leboncoin, wsj — per `27 §1`) | no auth-flow targets in corpus |

The key architectural insight: **the same JS tag feeds both products.** A site with DD-web protection that *also* uses DD-AP doesn't load a second JS payload — the existing `dd-script` collects everything, and the AP-specific evaluation runs server-side on the auth endpoints.

### 3.3 Detection signals

Per [DataDome changelog — new AI models](https://datadome.co/changelog/how-new-ai-models-strengthen-account-security/) + [DataDome credential stuffing knowledge center](https://datadome.co/hub/credential-stuffing/) + the chapter 07 baseline:

| Signal class | What it covers |
|---|---|
| **All DD-web signals** | per chapter 07: TLS fingerprint, navigator, canvas, WebGL, audio context, headers, IP reputation, request rate, mouse/touch on the page |
| **Login-page interaction patterns** | dwell time on username field, time to first keystroke, paste detection, form-fill order, total time spent on the login form before submit |
| **Server-side per-account rate** | login attempts per credential, login attempts per IP, login attempts per device, distribution across geographies — the credential-stuffing fingerprint |
| **Dynamic thresholds (new in 2025)** | the AI models set upper-bound thresholds on login attempts, request rate, session activity per account; thresholds adjust automatically as patterns evolve (per the changelog post) |
| **Hybrid threat detection** | per the changelog: "designed to safeguard against both automated and hybrid threats" — hybrid = bots + low-skilled humans cooperating (e.g. CAPTCHA-farm-assisted credential stuffing) |
| **Behavioral biometrics** | keystroke cadence, mouse dynamics on the login form. (Note: search-result evidence is *suggestive* but not explicit in DD's public docs that DD-AP specifically does keystroke-cadence analysis at the depth Forter's USPTO patents claim. Treat as: "DD-AP collects login-form interaction signals; full per-keystroke biometric is plausible but unconfirmed in their public material.") |
| **Email intelligence** | disposable-email-domain detection, email-reuse-across-accounts |

### 3.4 Mitigation patterns

Per [datadome.co / products / account-protect](https://datadome.co/products/account-protect/):

- **Allow** — proceed.
- **Step-up MFA** — DD-AP signals high risk; customer triggers MFA challenge.
- **Force password reset** — DD-AP signals likely-credential-stuffing; customer locks the account pending reset.
- **CAPTCHA** — show DataDome's own captcha (the `geo.captcha-delivery.com` slider, same as web), OR migrate from hCaptcha to DD-CAPTCHA per the [DataDome migration guide](https://datadome.co/bot-management-protection/how-to-switch-from-your-traditional-captcha-to-datadome-captcha/).
- **Block** (403 or 401) — for the most-risky scoring; usually combined with MFA-required follow-up.

A nuance per the [DataDome G2 reviews](https://www.g2.com/products/datadome/reviews) and the marketing: DD-AP is *more tolerant of false positives* than DD-web because the user-friction cost of an MFA challenge is acceptable; whereas a DD-web false-positive blocks the page entirely. This shows up in the field as DD-AP triggering on more borderline cases (any login from a new ASN, any login at unusual time, etc).

### 3.5 BO coverage assessment

| Scenario | Outcome | Confidence |
|---|---|---|
| Public-page scrape on a site that uses **both** DD-web and DD-AP (e.g. likely etsy, tripadvisor at the auth-protected paths) | **DD-web is the dominant blocker** — chapter 07 applies. DD-AP only activates on the auth endpoints we don't touch. | high |
| Auth-flow scrape on a DD-AP-protected endpoint | **OUT OF SCOPE** — auth flows are not a BO target. | by design |
| BO triggering DD-AP false-positive via "no keystroke cadence" on a customer's auth flow | **N/A in normal scope.** Mentioned only because: if a customer pushes BO into an auth flow (via per-customer extension), the `crates/browser/src/js/humanize.js` gap on keystroke cadence would surface here. | low — speculative |
| BO future humanize.js v2 (keystroke / dwell synthesis) defeating DD-AP login-page interaction signals | **theoretical** — would require a chapter-42-scope cross-vendor behavioral effort | not in v0.1.0 scope |

The honest answer: **DD-AP does not change the public-page scrape surface.** Chapter 07's DataDome primitives (CSP relaxation, cross-origin iframe materialization, solved-cookie retry) are the entire story for DD-protected pages BO targets. DD-AP is a separate concern that surfaces only if a customer tries to scrape auth-protected pages, at which point we escalate to private `vendor_solvers` per `SCOPE.md`.

### 3.6 Public solver landscape

- [docs.datadome.co / account-protect](https://docs.datadome.co/docs/account-protect) — official integration docs; describes the JS Tag + server-side integration model. Not a bypass guide.
- [DataDome JS Tag integration docs](https://docs.datadome.co/docs/javascript-tag) + [DataDome client-side JS Tag optimizations](https://datadome.co/engineering/client-side-javascript-tag-optimizations/) — DataDome's own engineering posts on tag architecture.
- [glizzykingdreko / Breaking down DataDome captcha WAF (Medium)](https://medium.com/@glizzykingdreko/breaking-down-datadome-captcha-waf-d7b68cef3e21) — the same author covering the DD-web captcha mechanism; relevant since DD-AP shares the captcha component.
- [Sekoia / DataDome protection integration](https://docs.sekoia.io/integration/categories/network_security/datadome_protection/) — SIEM integration; useful for understanding DD-AP's log output.
- [ZenRows / DataDome bypass 2026](https://www.zenrows.com/blog/datadome-bypass) — covers DD-web; DD-AP is mentioned as the "ATO-specific" product line with no separate bypass guide.

**Landscape: public material is product-marketing-heavy, technical-low.** DD's encoder for the JS tag is private and rotates frequently; there is no public account-takeover-specific bypass corpus.

### 3.7 Sources

- [datadome.co / products / account-protect](https://datadome.co/products/account-protect/)
- [datadome.co / products / account-takeover-protection (alt path)](https://datadome.co/products/account-takeover-protection/)
- [docs.datadome.co / account-protect](https://docs.datadome.co/docs/account-protect)
- [docs.datadome.co / javascript-tag](https://docs.datadome.co/docs/javascript-tag)
- [docs.datadome.co / how-to-configure-the-javascript-tag](https://docs.datadome.co/docs/how-to-configure-the-javascript-tag)
- [DataDome changelog — new AI models for account security](https://datadome.co/changelog/how-new-ai-models-strengthen-account-security/)
- [DataDome hub / credential stuffing](https://datadome.co/hub/credential-stuffing/)
- [DataDome engineering — client-side JS Tag optimizations](https://datadome.co/engineering/client-side-javascript-tag-optimizations/)
- [DataDome — DataDome CAPTCHA product](https://datadome.co/products/datadome-captcha/)
- [DataDome — switch from traditional CAPTCHA to DataDome CAPTCHA](https://datadome.co/bot-management-protection/how-to-switch-from-your-traditional-captcha-to-datadome-captcha/)
- [AWS Marketplace — DataDome Account Protect](https://aws.amazon.com/marketplace/pp/prodview-vyug7zaqjyxv6)
- [Sekoia — DataDome protection integration](https://docs.sekoia.io/integration/categories/network_security/datadome_protection/)
- [G2 — DataDome reviews](https://www.g2.com/products/datadome/reviews)
- [GetApp — DataDome pricing/features](https://www.getapp.com/security-software/a/datadome-anti-bot-protection/)
- [netadmintools — DataDome 2026 review](https://www.netadmintools.com/datadome-review/)
- [glizzykingdreko — Breaking down DataDome CAPTCHA WAF](https://medium.com/@glizzykingdreko/breaking-down-datadome-captcha-waf-d7b68cef3e21)
- [ZenRows — DataDome bypass complete guide 2026](https://www.zenrows.com/blog/datadome-bypass)

---

## 4. Cross-vendor synthesis for these three

### 4.1 Positioning matrix

| Vendor | Tier | Pricing | Mechanism style | Public bypass surface | BO outlook |
|---|---|---|---|---|---|
| **Reblaze** | Enterprise WAAP, dedicated-VPC | enterprise (custom quote) | Cloudflare-style with stronger custom-rules + dedicated-VPC isolation + behavioral biometrics tier | minimal (zero open-source corpus) | speculative — no corpus data; likely PASS on basic JS challenge, likely FAIL on biometric tier |
| **Sucuri** | SMB WAF + CDN | $9.99-$549 (cheap) | Cloudflare-style cheaper, signature-WAF + cookie-set JS shield, no fingerprinting | mature (5+ working open-source bypasses) | PASS — easiest vendor in this report; standard navigate loop clears the JS shield |
| **DataDome Account Protect** | DD-Web extension to auth endpoints | enterprise (DD pricing) | DD-Web with auth-specific server-side scoring tuning; same JS tag, different decision target | low (DD encoder is private, no AP-specific bypass corpus) | N/A for public pages (DD-Web is the blocker); OUT OF SCOPE for auth flows |

### 4.2 Mechanism family tree

```
                            Edge-WAF + JS-challenge family
                                       │
              ┌────────────────────────┼─────────────────────────┐
              │                        │                          │
        Sucuri (low-tier)          Cloudflare              Akamai BMP / Reblaze (high-tier)
              │                        │                          │
       cookie-set JS shield        cf_clearance               sensor_data v2/v3
       no fingerprint              + Turnstile                + sec-cpt PoW
       BO: PASS                    BO: mostly PASS            BO: FAIL on biometric

                            ATO-specific products
                                       │
              ┌────────────────────────┼─────────────────────────┐
              │                        │                          │
        DataDome AP                Akamai AP                  Castle / Sift / Forter
        (chapter 37)               (chapter 36 §5)            (chapter 36 §2-4)
              │                        │                          │
       shares DD-Web tag          shares BMP _abck           dedicated SDK, advisory
       server-side decision       server-side decision       server-side decision
       BO: N/A for auth           BO: N/A for auth           BO: PASS for public pages
```

The vertical bar between rows is the architectural seam:
- **Top row (edge-WAF + JS-challenge):** vendors that BLOCK at the edge with a client-side challenge. This is where BO competes engine-vs-engine.
- **Bottom row (ATO-specific):** vendors that SCORE on a sidecar API call, advisory output to the customer's auth service. This is where BO is out of scope by design.

### 4.3 What ties them together

**All three vendors in this chapter (Reblaze + Sucuri + DD-AP) sit at the periphery of BO's competitive surface:**
- Reblaze: no corpus presence; speculation-only assessment.
- Sucuri: passes trivially; standard navigate loop handles it.
- DD-AP: shares the DD-Web JS tag (already in scope via chapter 07); the AP-specific decision is auth-only and out of scope.

This means BO's competitive position vs the 126-corpus does not change based on this chapter's content. The chapter exists to:
1. Close the documentation gap (chapter 18 §2.7 + §2.8 are intentionally brief; this chapter is the deeper resource for customer escalations).
2. Make the cross-vendor mechanism family tree explicit so chapter 42 (cross-vendor pattern synthesis, future) has a foundation.
3. Honest documentation of vendor convergence (DD has both products; Akamai has both products; the unified anti-vendor pitch is becoming standard per chapter 36 §6.3).

---

## 5. Implications for BO

### 5.1 What changes today (v0.1.0)

**Nothing.** No code changes; no measurement targets; no corpus changes. This chapter is reference-only.

### 5.2 What changes for customer onboarding

A customer with a Reblaze-protected target: refer to §1.8 above. Expected: PASS on basic-tier deployments; FAIL on full-biometric-tier. If FAIL, escalate to `vendor_solvers` consideration.

A customer with a Sucuri-protected target: per §2.7, expected PASS. No further work needed.

A customer with a DataDome-protected target (web or AP): the DD-Web path is chapter 07. DD-AP itself is auth-only, out of scope for the public engine.

### 5.3 What changes for the engine seam

Add to the vendor-detect logger at `crates/browser/src/page.rs:1054-1069` (sketch — not a code change, just the seam location):

```rust
// (sketch — do NOT modify code; this is the documented extension point)
if let Some(v) = resp.headers.get("x-rbzid") {
    eprintln!("[vendor-detect] reblaze x-rbzid {} on {}", v, resp.url);
}
// rbzid / rbzsessionid cookies already covered by the existing cookie-write logger
// at crates/browser/src/page.rs (per chapter 18 §4 logger code)

// DataDome AP: shares the existing DD detection (x-datadome header, dd-script body
// marker). No new markers; DD-AP is signaled by the auth endpoint context, not by
// new headers on the public-page response.
```

Sucuri markers (`x-sucuri-id`, `x-sucuri-cache`, `server: Sucuri/Cloudproxy`, `sucuri_cloudproxy_uuid_*`) are already detected per the cookbook §4 logger.

### 5.4 What changes for the classifier

No new rows in `crates/browser/src/classify.rs` from this chapter. Reblaze and Sucuri are already covered in spirit (Reblaze via `x-armor-shield-zone` per cookbook §1.1; Sucuri via `x-sucuri-id` per cookbook §1.1 + `sucuri_cloudproxy_js` body marker per cookbook §1.3 — both can be added if/when measurement need surfaces). DD-AP needs no new rows because it shares the DD-Web detection.

### 5.5 What changes for the v0.1.0 acceptance gate

Nothing. Per `27 §1` matrix, none of these three vendors appear in the 126-corpus, so the routed-pass count (BO 108 / Camoufox 113) is invariant to this chapter.

---

## 6. Acceptance + files

### 6.1 What "done" looks like

This is a **reference encyclopedia** in the same shape as chapter 36. Acceptance:

- [x] Per-vendor sections for all three target vendors (Reblaze, Sucuri, DD-AP) covering: history, architecture, detection signals, detection markers, challenge mechanism, mitigation patterns, BO coverage assessment, public solver landscape, sources.
- [x] Cross-vendor synthesis with positioning matrix (§4.1) and mechanism family tree (§4.2).
- [x] Honest uncertainty markers — especially the "Mc Cohen" reference (§1.3) which I cannot verify in current Reblaze / Link11 docs and explicitly call out as research-item-not-confirmed, and the DD-AP keystroke-cadence claim (§3.3) which is plausible but not in their public technical material.
- [x] Cross-links to cookbook 18 (§2.7 + §2.8), chapter 07 (DD-Web), chapter 26 (Akamai BMP — the Akamai AP analog), chapter 27 (competitive matrix), chapter 36 (ATO-specialists companion).
- [x] Every URL cited inline with markdown links.
- [x] Explicit statement of zero code/measurement impact (§5).

### 6.2 What this chapter explicitly does NOT include

- No code changes — `CLAUDE.md` / `SCOPE.md` policy applies.
- No corpus-pass-rate impact (none of these vendors are in the 126-corpus per `27 §1`).
- No DD-AP / Reblaze bypass — auth-flow vendors are explicitly out of scope.
- No verification of the "Mc Cohen module" or "mc.cohen.io" references in the doc-37 source brief — these are unverified and flagged as research-items.

### 6.3 Files cross-referenced

| File | Why referenced |
|---|---|
| `CLAUDE.md` | Out-of-scope policy: per-vendor solving → `vendor_solvers` companion crate |
| `SCOPE.md` | Auth-flow scope statement (out for the public engine) |
| `docs/releases/v0.1.0-parity/07_DATADOME_PRIMITIVES.md` | DD-Web — the public-page analog of DD-AP. Shares the same JS tag and primitives. |
| `docs/releases/v0.1.0-parity/18_ANTI_BOT_VENDOR_COOKBOOK.md` | Cookbook §2.7 (Imperva/Reblaze) and §2.8 (Sucuri) — this chapter extends both. Cookbook §1.1 / §1.3 / §1.4 marker tables are the authoritative source for vendor identification. |
| `docs/releases/v0.1.0-parity/25_CLOUDFLARE_DEEP.md` | Comparative reference — Reblaze is described as "Cloudflare-style with stronger custom-rules"; CF deep-dive is the architectural baseline. |
| `docs/releases/v0.1.0-parity/26_AKAMAI_BMP_DEEP.md` | Akamai AP shares BMP's `_abck` telemetry; DD-AP shares DD-Web's `dd-script` telemetry. Parallel architecture. |
| `docs/releases/v0.1.0-parity/27_VENDOR_COMPETITIVE_MATRIX.md` | §1 row "Imperva / Reblaze / Sucuri | 0 in corpus" — explicit acknowledgment that none are in measurement scope. |
| `docs/releases/v0.1.0-parity/36_ATO_SPECIALISTS.md` | Companion chapter — Castle / Sift / Forter / Akamai AP. Together with this chapter, covers the seven major ATO-specific vendors. |
| `crates/browser/src/page.rs:1054-1069` | Vendor-detect logger — the seam location for adding `x-rbzid` recognition (sketch in §5.3). |
| `crates/browser/src/classify.rs` | Classifier — no changes from this chapter; existing Sucuri / Reblaze coverage per cookbook §1.1 suffices. |

### 6.4 Open questions deferred

- **"Mc Cohen" module** (§1.3): is this an actual Reblaze internal codename or a misremembering / outdated reference? Resolve only if a customer escalation surfaces it with a primary source.
- **`mc.cohen.io` endpoint** (§1.5): unverified domain claim from the doc-37 brief. No DNS, no documentation, no third-party corpus mention. Resolve only with a live capture from a Reblaze-protected target.
- **DD-AP keystroke-cadence depth** (§3.3): does DD-AP collect per-keystroke biometrics at Forter-patent depth, or is it limited to coarse dwell + paste detection? Public documentation is suggestive but not explicit; resolve only with live measurement on an opt-in customer test deployment.
- **Reblaze per-customer rule variability** (§1.4): how much variance is there in practice between two Reblaze customers' WAF rule sets? Operationally important if BO ever sees Reblaze in a corpus — analogous to AWS WAF (chapter 06) where per-tenant tuning was decisive.
- **Sucuri after GoDaddy** (§2.1): GoDaddy acquired Sucuri in 2017; has product direction shifted in 2024-2026? (No major changes evident from public materials, but worth a recheck if a customer reports unexpected blocking behavior.)

These belong in chapter 42 (cross-vendor synthesis) or in a future per-vendor revision once measurement data exists.

---

## 7. Appendix — quick-lookup card

Mirror of chapter 36 §10 / cookbook §1 for the three vendors covered here.

### 7.1 Header markers

| Header | Vendor | Where it appears |
|---|---|---|
| `x-armor-shield-zone` | Reblaze (legacy Thales-era) | response from Reblaze edge |
| `x-rbzid` | Reblaze | response from Reblaze edge — request-id |
| `x-sucuri-id` | Sucuri | response from Sucuri/Cloudproxy edge |
| `x-sucuri-cache` | Sucuri | response cache-state indicator (HIT/MISS/BYPASS) |
| `server: Sucuri/Cloudproxy` | Sucuri | response Server header |
| `x-datadome` | DataDome (web + AP share this) | response from DataDome edge — see chapter 07 for status values |
| (no AP-specific header) | DataDome Account Protect | shares DD-web markers; no AP-distinct response header |

### 7.2 Cookie markers

| Cookie | Vendor | Notes |
|---|---|---|
| `rbzid` | Reblaze | session-clearance after JS challenge pass |
| `rbzsessionid` | Reblaze | additional session tracker |
| `mc_session` | Reblaze (legacy / Thales-era deployments) | per cookbook §1.4 |
| `sucuri_cloudproxy_uuid_*` | Sucuri | per-protected-site UUID, set after JS shield clears |
| `datadome` | DataDome (web + AP) | clearance cookie; same as chapter 07 |

### 7.3 Body markers

| Marker | Vendor | Notes |
|---|---|---|
| `sucuri_cloudproxy_js` | Sucuri | JS shield interstitial |
| `Sucuri WebSite Firewall` | Sucuri | block-page brand text |
| (Reblaze block page brand text) | Reblaze | not currently in `crates/browser/src/classify.rs` markers; would need a captured sample to add |
| `dd-script`, `dd_engagement`, `captcha-delivery.com`, `ddcaptchaencoded` | DataDome (web + AP share) | per cookbook §1.3 |

### 7.4 JS / network endpoint markers

| URL pattern | Vendor | Notes |
|---|---|---|
| (Reblaze JS bootstrap is per-customer; no stable global CDN URL) | Reblaze | The JS lives at customer paths in the dedicated-VPC architecture; no canonical CDN domain |
| (no stable Sucuri-side JS — the JS shield is inline in the interstitial body) | Sucuri | The cookie-set script is base64-decoded inline; no external script load |
| `js.datadome.co/js/datadome.js` or `js.datadome.co/...` | DataDome (web + AP share) | per chapter 07 |
| (DD-AP server-side endpoint — per-customer) | DataDome Account Protect | Auth-endpoint integration is customer-specific |

---

## 8. Appendix — comparative ranking

Slot the three vendors into the same ranking framework as `36 §11` for an integrated picture:

| Vendor | Detection sophistication (0-10) | Behavioral-signal depth (0-10) | Cross-tenant intelligence (0-10) | Public-page blocking? | BO impact today |
|---|--:|--:|--:|---|---|
| **Reblaze (default config)** | 6 | 3 | 5 | yes — JS challenge | N/A — not in corpus; speculative PASS |
| **Reblaze (biometric tier enabled)** | 8-9 | 8 | 5 | yes — biometric challenge | N/A — not in corpus; speculative FAIL |
| **Sucuri** | 4 | 1 | 3 | yes — JS shield, no fingerprint | PASS via standard loop |
| **DataDome Account Protect** | 9 (inherits DD-web) | 8 (login-page interaction + 2025 AI thresholds) | 8 (DD network) | no — auth-endpoint only | N/A on public pages |

The picture: **Reblaze is the most variable** (low to high depending on per-customer config); **Sucuri is the easiest** in this report (BO passes trivially); **DD-AP is invisible to public-page scraping** (shares the DD-web JS tag; AP-specific decisions happen on auth endpoints that BO doesn't touch). For chapter 27 (competitive matrix), all three remain at "0 in corpus" — they are reference-only entries.

---

## 9. Appendix — customer-pitch one-liner per vendor

For the sales-doc author who needs a single sentence per vendor (mirrors `27 §9` for scraping vendors covered in the corpus + `36 §12` for ATO specialists):

- **Reblaze:** "Not in our 126-corpus today; expected PASS on default-config deployments based on Reblaze's mechanism family being Cloudflare-style; if you have a Reblaze target with biometric detection enabled, escalation to vendor_solvers may be needed."
- **Sucuri:** "BO clears Sucuri trivially via the standard navigate loop — the JS shield is a cookie-set pattern with mature open-source bypasses; no special profile needed."
- **DataDome Account Protect:** "If your DataDome target also runs DD-AP, the public-page scrape outcome is identical to chapter 07 — DD-AP only affects login endpoints, which BO doesn't target."

---

## 10. Appendix — relationship to the public engine's `aecdf19` strip

Mirrors `36 §13`. The vendor-strip commit policy applies to these vendors as well:

- **Reblaze:** any future `ReblazeSolver` (forging `rbzid`, satisfying JS-challenge + behavioral) → `vendor_solvers`. None exists today.
- **Sucuri:** does not warrant a private solver — BO clears it via the standard loop. The js2py-based open-source bypasses (per §2.8) are the reference if one were ever needed.
- **DataDome Account Protect:** shares the existing `crates/akamai/`-era DataDome envelope code (pre-strip; per chapter 07 + chapter 26 §1). Any AP-specific extension would attach to the existing `DataDomeSolver` codepath in `vendor_solvers`, parameterized for the auth-endpoint context.

Public-engine seams unchanged:

| Public seam | Location | Coverage for vendors in this chapter |
|---|---|---|
| `ChallengeSolver` trait | `crates/browser/src/challenge.rs:55-161` | Same as scraping vendors — private solvers would implement it. |
| `ChallengeKind` enum | `crates/browser/src/challenge.rs` | No new variants needed: Reblaze fits under generic `JsChallenge`; Sucuri likewise; DD-AP not applicable (auth-endpoint-only). |
| Vendor-detect logger | `crates/browser/src/page.rs:1054-1069` | Already covers `x-sucuri-id`; `x-armor-shield-zone` per cookbook §1.1. Add `x-rbzid` per §5.3 if customer escalation surfaces. |
| Classifier markers | `crates/browser/src/classify.rs` | No changes from this chapter (per §5.4). |

Engineering invariant: **none of the work documented in this chapter requires public-engine changes.** All vendor-specific encoding lives (or would live) in `vendor_solvers`.

---

## 11. Appendix — empirical posture today

Concrete statement of what BO does and does not measure against these vendors today, to prevent the documentation from drifting toward implied measurement:

| Vendor | In 126-corpus? | Measured BO routed pass-rate? | Measured Camoufox routed pass-rate? | Last verified by |
|---|---|---|---|---|
| Reblaze | no | n/a | n/a | Vendor not in `crates/browser/src/classify.rs` markers — would need live capture |
| Sucuri | no | n/a | n/a | Vendor markers present in `classify.rs` per cookbook §1.1 but no sites with this vendor are in `crates/browser/tests/fixtures/corpus_v2.json` |
| DataDome Account Protect (as distinct from DD-web) | no (DD-web sites are in corpus; AP-only auth targets are not) | n/a for AP specifically | n/a for AP specifically | Same — DD-web is measured; AP-as-distinct-product is not, by design |

If a future customer engagement surfaces a Reblaze target, the action is:
1. Capture the response (`fetches.json`, `cookie_writes.json` per `04_TOOLING_SPEC.md`).
2. Update §1.5 detection markers with verified-from-the-wild values.
3. Add the site to a private test fixture (NOT the public corpus, since vendor presence is customer-confidential).
4. Run a single-site sweep across the 4 BO profiles to establish a baseline.
5. If FAIL: chapter-7-style primitive analysis; if PASS: log the result, no further action.

This procedure prevents the chapter from accreting unverified claims and keeps the boundary between speculative documentation (this chapter) and measured outcomes (chapter 27) clean.

---

## 12. Appendix — relationship to chapter 42 (cross-vendor pattern synthesis)

The three vendors in this chapter contribute different inputs to the future chapter 42:

**Reblaze contributes:**
- A *dedicated-VPC architecture* data point — distinct from Cloudflare's multi-tenant model. Chapter 42's "deployment architecture" axis will distinguish multi-tenant (CF, AWS WAF, DataDome) from per-tenant (Reblaze) from edge-of-customer (Castle's optional Cloudflare-module). The architectural axis matters because per-tenant deployments imply per-customer rule variability (similar to AWS WAF).
- A *behavioral-biometric-tier-optional* model — most vendors are all-or-nothing on biometrics; Reblaze sells it as a tier upgrade. This is the same shape as Sucuri's optional-CAPTCHA tier and Sift's per-Workflow rule configuration.

**Sucuri contributes:**
- The *SMB / low-tier baseline.* Every vendor catalog needs a low-end anchor for "what does the bottom of the market look like?" Sucuri is that anchor; the cookbook §2.8 framing ("simpler than Akamai/Imperva — usually a single JS shield, no WASM, no behavioural component") is the canonical statement.
- A demonstration that *mature open-source bypass corpus exists when the vendor mechanism is simple enough.* Sucuri's bypass GitHub corpus (5+ working repos) contrasts sharply with Castle / Sift / Forter / DD-AP / Akamai AP / Reblaze (all ~zero). The variable that predicts bypass-corpus depth is mechanism simplicity, not vendor maturity.

**DataDome Account Protect contributes:**
- The *shared-telemetry-layer architecture.* DD-web and DD-AP share the `dd-script` JS tag. Akamai BMP and Akamai AP share the `_abck` infrastructure. This convergence pattern is chapter 42's headline observation about vendor consolidation.
- A *2025-2026 AI-model adaptation data point.* DD-AP's 2025 changelog notes "dynamic thresholds" that adjust as patterns evolve. If this generalizes across vendors, chapter 42 will need a "model-drift mitigation" section — a private solver's accuracy decay over time becomes a maintenance-burden axis on the build-vs-license analysis.

These cross-references are placeholders; chapter 42 has not been written. The point of flagging them here is: when chapter 42 is written, the inputs from this chapter are already organized and don't need to be re-discovered.
