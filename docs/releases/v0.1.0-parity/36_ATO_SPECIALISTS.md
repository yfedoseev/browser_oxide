# 36 — Account-takeover specialists (Castle / Sift / Forter / Akamai Account Protector)

**Status:** reference (customer-onboarding + cross-vendor pattern synthesis)
**Audience:** anyone scoping a new customer whose target site uses an ATO-focused vendor in addition to (or instead of) the scraping-focused vendors covered in chapters 06/07/08/25/26.
**Companion docs:** `18_ANTI_BOT_VENDOR_COOKBOOK.md` (vendor-id flowchart + the existing 12 vendor sections), `27_VENDOR_COMPETITIVE_MATRIX.md` (per-vendor engine wins), `26_AKAMAI_BMP_DEEP.md` (BMP-specific; Account Protector layers on top), `07_DATADOME_PRIMITIVES.md` (web-side DataDome), `37_REBLAZE_SUCURI_DDAP.md` (DataDome Account Protect deep-dive), `01_CURRENT_STATE.md` (scope statement), `SCOPE.md` (the in/out-of-scope line for the public engine).

**One-paragraph thesis:** Account-takeover (ATO) protection is a vendor category *adjacent to* — not identical with — scraping protection. The four vendors covered here (Castle, Sift, Forter, Akamai Account Protector) sit at LOGIN / SIGNUP / PASSWORD-RESET endpoints, not at the public-pages we care about for scraping. **For browser_oxide's primary use-case (public-page rendering for downstream extraction), direct exposure to these vendors is low** — none of them appear in the 126-site corpus (per `27 §1`), and we intentionally do not solve auth flows. But three forces make this chapter load-bearing: (1) **shift-left**: ATO vendors increasingly fire on PUBLIC pages too (signal-collection for later auth events — Castle and Forter both document this), (2) **dual-stacks**: a site with Akamai BMP at the edge often also has Akamai Account Protector at the login endpoint and ATO-vendor JS may load on every page, (3) **customer onboarding**: a customer pitching "scrape behind login" will land here; honest scoping demands we know the floor and ceiling. This chapter is the catalog so the question "we hit a site with Castle, are we cooked?" has a faster-than-2-hours answer.

---

## 1. Why ATO specialists matter

### 1.1 The category, in one paragraph each

**Scraping-protection vendors** (Cloudflare, AWS WAF, DataDome web, Akamai BMP, Kasada, PerimeterX) sit at the **edge** and gate every HTTP request. They optimize for: "is this client a bot at all?" The decision is fast (sub-100 ms), made before any auth context exists, and the false-positive cost is *medium* (a real user retries; rage-quit rate matters but isn't catastrophic).

**ATO-protection vendors** (Castle, Sift, Forter, Akamai Account Protector) sit at the **application** layer and gate specifically the credential-bearing endpoints. They optimize for: "is *this credential* being used by its legitimate owner?" The decision is slower (sub-500 ms is acceptable, sometimes deferred to async scoring), made with full account context (login history, prior devices, geo, value-at-risk), and the false-positive cost is *high* (locking out the legitimate account owner is worse than letting a small fraction of ATO through and catching it on transaction).

The two categories *historically* lived in separate vendor lanes. In 2024-2026 they began to converge — most enterprise scraping-protection vendors added an ATO product (DataDome → DataDome Account Protect, Akamai BMP → Akamai Account Protector, HUMAN/PerimeterX → HUMAN Account Defender), and several ATO vendors expanded to public-page coverage (Castle's "shift-left" device fingerprinting, Forter's "Trusted Identities" pre-auth signals).

### 1.2 Where they sit in the request lifecycle

```
                                 [user types URL]
                                       │
                                       ▼
                       ┌───────────────────────────────┐
                       │  EDGE / CDN                   │
                       │  Cloudflare / Akamai / AWS    │
                       │  + scraping-vendor (DD, KSD…) │  ← chapters 06/07/08/25/26
                       └───────────────────────────────┘
                                       │
                          (public page or static asset)
                                       ▼
                       ┌───────────────────────────────┐
                       │  APPLICATION                  │
                       │  loads page HTML + JS         │
                       │  (may include ATO-vendor JS)  │  ← Sift / Castle / Forter SDK
                       │                               │     loads invisibly here too
                       └───────────────────────────────┘
                                       │
                       (user clicks "Sign in" / "Sign up")
                                       ▼
                       ┌───────────────────────────────┐
                       │  AUTH ENDPOINT                │
                       │  /login, /signup, /reset      │
                       │  ATO-vendor sidecar API call  │  ← Castle /v1/risk, Sift events,
                       │  decision: allow/MFA/block    │     Forter token, Akamai-user-risk header
                       └───────────────────────────────┘
                                       │
                                       ▼
                                 [session cookie]
```

The scraping vendor sees every request. The ATO vendor *may* drop JS on every page (for signal-collection) but only fires its decision on the auth POST.

### 1.3 Why browser_oxide cares

Three concrete reasons:

1. **Cross-vendor pattern recognition.** All four ATO vendors collect device-fingerprint signals that overlap heavily with scraping vendors: canvas hash, WebGL renderer, AudioContext fingerprint, navigator.* enumeration, font enumeration, timezone, hardware concurrency. Our `crates/stealth/profiles/*.yaml` already covers the scraping-vendor set; the ATO-vendor set is a superset with two additions: **mouse/keystroke micro-timing** (Sift, Forter) and **session-replay-quality behavioral capture** (Castle's `bot_behavior` signal). Documenting the gap lets chapter 42 (cross-vendor synthesis, future) frame the eventual unified profile.
2. **Customer onboarding.** A customer who says "I need to scrape product pages on `nordstrom.com`" gets the standard chapter-07/26 treatment. A customer who says "I need to scrape *behind* `nordstrom.com`'s login" hits Forter and (per §3.3 below) the answer is: **escalate to vendor_solvers + understand this is a different product line.** Knowing the vendor identification markers (§2-5) prevents misdiagnosis.
3. **Shift-left risk.** Castle's docs explicitly support deploying its SDK as an "edge module" on Cloudflare via no-code integration (per [castle.io/device-api](https://castle.io/device-api/)). When that happens, Castle JS fires on PUBLIC pages and our `chrome_compat.rs` profile becomes load-bearing for non-auth requests too. We have to know about it.

### 1.4 What this chapter does NOT do

- It does NOT plan a Castle / Sift / Forter / Akamai-AP **solver**. All four are auth-endpoint vendors and per `SCOPE.md` + the `aecdf19` precedent, per-vendor solving lives in the private `vendor_solvers` companion crate.
- It does NOT propose adding ATO-vendor markers to `crates/browser/src/classify.rs`. The classifier targets *blocking* responses (status + body markers); ATO vendors typically score silently and return 200 even on high-risk requests (with the decision delivered to the backend via headers or sidecar API). The right detection seam is the existing vendor-detect logger at `crates/browser/src/page.rs:1054-1069`, extended in §6.
- It does NOT estimate corpus pass-rate impact. None of these vendors are in the 126-site corpus (per `27 §1` and `02 §hard-residual`). They're future-customer concerns.

---

## 2. Castle

### 2.1 One-line + history

Castle (castle.io) is a device-intelligence + risk-API platform for fraud/abuse prevention founded 2014, San Francisco-based, Series-A funded. Originally a behavioral-biometric add-on for SaaS login flows; in 2023-2024 expanded into "device intelligence" as a standalone product line aimed at edge deployment (per the [castle.io/device-api](https://castle.io/device-api/) landing). Customer logos on the front page include **Atlassian, Rockstar Games, Rakuten, Framer, Lovable, Canva, Sinch, GoTo, Farfetch, DailyPay** (per [castle.io](https://castle.io/)). Positioning: "enterprise security without the friction" — they emphasize edge deployment and Cloudflare no-code integration as a differentiator vs Sift (Sift is JS-snippet-only at the app layer).

### 2.2 Detection signals — full inventory

Per the official signals reference at [docs.castle.io/docs/signals-reference](https://docs.castle.io/docs/signals-reference), Castle exposes **43 distinct signals** across six categories:

| Category | Signals |
|---|---|
| **Automated activity** | `bot_behavior`, `credential_stuffing`, `generated_email`, `high_activity_account`, `high_activity_device`, `high_activity_ip` |
| **Anomalous behavior** | `impossible_travel`, `multiple_accounts_per_device` |
| **Device-data errors** | `missing_device_data`, `invalid_device_data`, `replayed_device_data` |
| **Device intelligence** | `spoofed_device`, `headless_browser`, `http_client_library`, `web_crawler`, `carrier_ip_country_mismatch`, `missing_headers`, `timezone_area_mismatch`, `rooted_device`, `emulated_device` |
| **Email intelligence** | `disposable_email_domain`, `fraudulent_email_domain`, `unreachable_email`, `invalid_email`, `low_quality_email`, `multiple_aliases_per_email` |
| **IP intelligence** | `abuse_ip`, `datacenter_ip`, `proxy_ip`, `tor_ip` |
| **Novel user attributes** | `new_country`, `new_device`, `new_device_type`, `new_isp`, `new_language`, `new_os` |

Per [docs.castle.io/docs/device-fingerprinting](https://docs.castle.io/docs/device-fingerprinting), the Browser SDK collects: **User Agent, OS, CPU, screen resolution, canvas fingerprinting, WebGL info, mouse and keyboard interactions, timezone, installed plugins, headless-browser checks, Tor-browser detection, browser type/version, incognito-mode detection, IP address**.

Mobile SDKs (iOS / Android) collect: unique device identifiers, memory/storage metrics, location, device orientation, carrier info, emulation detection, jailbreak/root, sensor data, battery, system languages.

Castle claims "up to 99.5% accuracy" with "0.0001% collision rate" on the device-fingerprint hash (per [castle.io](https://castle.io/)).

### 2.3 JS SDK pattern

Per [castle.io/device-api](https://castle.io/device-api/) + [roundproxies / Castle bypass](https://roundproxies.com/blog/castle-bypass/):

- **CDN script URL:** `https://cdn.castle.io/...` (versioned — observed in the wild as `castle-web` 2.1.8 per [castle.io](https://castle.io/) front-page reference; exact path obfuscated and rotates per release).
- **Publishable key format:** `pk_...` (per Castle bypass write-up).
- **Request token header (server side):** `x-castle-request-token` — generated fresh per request by the JS SDK and sent on every protected endpoint call (per the docs, "designed to be generated fresh before each server-side request to Castle's APIs").
- **First-party storage:** Castle's fingerprinting uses both cookies and localStorage on the *site owner's* domain. The cookie name has been observed as `__cuid` (session UID) per third-party bypass docs; not confirmed in Castle's public reference.
- **Castle API endpoint (server-to-server):** `https://api.castle.io/v1/risk` (login/signup decisions), `https://api.castle.io/v1/devices` (device intelligence).
- **Obfuscation:** Castle's bypass blog notes "proprietary obfuscation and randomization techniques" — the script changes structure between releases. This is consistent with our experience for Kasada (`crates/akamai/`-era) — assume a versioned, obfuscated bootstrap, not stable code.

### 2.4 Risk scoring + decision flow

1. **Browser SDK loads** on every protected page (login or arbitrary, depending on customer config).
2. **SDK collects signals** (§2.2) and computes a device fingerprint + maintains a session token.
3. **On a protected action** (login submit, signup submit, sensitive API call), the JS SDK generates a fresh `request_token` and the *server-side* code calls Castle's `/v1/risk` API with the user identity + token + context.
4. **Castle returns a decision**: `allow | challenge | deny` + a risk-score (0.0-1.0) + the list of triggered signals.
5. **Customer's app** decides what to do: allow, MFA, password reset, block, queue for review.

The key architectural point: **Castle is server-decided, JS-collected.** The JS never blocks; it just emits signals. The "block" happens at the customer's auth endpoint, not in-browser. This is why a non-auth scrape on a Castle-protected site typically just sees Castle JS load and runs to completion — no challenge page.

### 2.5 Mitigation patterns

The customer chooses, per Castle's documentation:
- **Allow** (low risk): proceed.
- **Challenge** (medium risk): MFA, email-verification, captcha (customer's choice of vendor — Castle doesn't ship one).
- **Deny** (high risk): block + lockout.

Castle is *advisory*, not gating. This is opposite to Cloudflare's challenge model (CF returns a 403/503 stub itself). For BO this means: **Castle alone does not produce a blocking response on public pages.** Castle's effect on scraping is indirect: collected signals feed the customer's own decision logic.

### 2.6 BO coverage assessment

| Scenario | Outcome | Confidence |
|---|---|---|
| Public-page scrape, Castle JS loads, no auth attempted | **PASS** — Castle does not block; SDK runs to completion, BO gets full page | high (consistent with Castle's documented architecture) |
| Auth-flow scrape on Castle-protected endpoint | **OUT OF SCOPE** — auth flows are not a BO target | by design |
| Public-page scrape, Castle deployed as Cloudflare edge module, set to "deny on `bot_behavior`" or `headless_browser` | **AT RISK** — Castle can issue a CF Worker block; signal-class will tag any headless engine | medium (no public evidence of any customer running this aggressive a config on public pages) |
| BO triggering `replayed_device_data` because we reuse stealth profile across sessions | **AT RISK** — Castle has explicit anti-replay; the `Castle.io` SDK rotates request tokens per request | medium (would need a live capture to confirm) |

The honest answer: **Castle is not in the 126-corpus, has not been observed blocking BO in any live measurement, and architecturally is unlikely to block public-page scrapes.** The "AT RISK" entries are theoretical and would surface only if a customer specifically reports a Castle-edge deployment.

### 2.7 Public solver landscape

- [roundproxies / Castle bypass](https://roundproxies.com/blog/castle-bypass/) — describes 6 generic bypass methods (residential proxies, real-browser automation, header crafting); no working Castle-specific encoder code is published openly.
- [captchasolv / Castle Solver API](https://captchasolv.com/captcha/castle) — commercial solver-as-a-service, opaque.
- [blog.castle.io / fingerprints beyond device IDs](https://blog.castle.io/fingerprints-beyond-device-ids-engineered-representations-for-fraud-detection/) — Castle's own technical post on their hash construction; useful for understanding *what* the fingerprint encodes (input space) but does not enable a solver.
- GitHub searches for "castle.io bypass" return zero notable repos as of 2026-05-24.

**Landscape: smallest of the four ATO vendors covered here.** Castle is below the radar of the open-source bypass community; commercial solvers exist (capsolver-style) but their efficacy is not measurable from outside.

### 2.8 Sources

- [castle.io main](https://castle.io/)
- [castle.io / device API](https://castle.io/device-api/)
- [castle.io / device intelligence product](https://castle.io/product/devices/)
- [docs.castle.io overview](https://docs.castle.io/)
- [docs.castle.io / signals reference](https://docs.castle.io/docs/signals-reference)
- [docs.castle.io / device fingerprinting](https://docs.castle.io/docs/device-fingerprinting)
- [blog.castle.io / fingerprints beyond device IDs](https://blog.castle.io/fingerprints-beyond-device-ids-engineered-representations-for-fraud-detection/)
- [roundproxies / Castle bypass](https://roundproxies.com/blog/castle-bypass/)
- [captchasolv / Castle Solver API](https://captchasolv.com/captcha/castle)

---

## 3. Sift

### 3.1 One-line + history

Sift (sift.com, formerly Sift Science) is the largest, oldest fraud-prevention platform in this group — founded 2011, San Francisco, valued at $1B+ in 2021's $50M Series F (per [TechCrunch, 2021-04-22](https://techcrunch.com/2021/04/22/fraud-prevention-platform-sift-raises-50m-at-over-1b-valuation-eyes-acquisitions/)). They sell a unified "Digital Trust & Safety Platform" spanning fraud, ATO, content moderation, payment risk. Customer base is enterprise-heavy: per [Harvard / d3.harvard.edu](https://d3.harvard.edu/platform-digit/submission/fighting-fraud-using-machines/), Airbnb is a documented customer; the wider customer claim ("Uber, Instacart, Robinhood") appears in second-hand summaries but is not directly confirmed by Sift's public marketing (their G2 / Capterra profiles list anonymized customer-size segments). **Network-effect framing:** Sift's pitch is "1 trillion data signals across 34k+ sites" (per [sift.com platform](https://sift.com/platform/)) — when one customer's traffic identifies a new fraud pattern, the model improvement is delivered to all customers.

### 3.2 Detection signals

Per [sift.com platform](https://sift.com/platform/) + [G2 Sift features](https://www.g2.com/products/sift/features):

- **Device fingerprint** (canvas, WebGL, navigator, screen, audio).
- **Behavioral signals**: mouse, keystroke, scroll, click-pattern, form-interaction timing.
- **Network**: IP, ASN, proxy/VPN/Tor detection.
- **Transactional history** (server-side): per-account login attempts, geo deltas, value-at-risk, prior fraud flags.
- **Identity correlation**: email + phone + name fuzzy-match against known fraud signatures.
- **Network-effect intelligence**: cross-customer pattern sharing (the "1 trillion signals" pool).

**Distinct from Castle**: Sift weighs *transactional* signals heavily. A login attempt in isolation is rated against the account's full history (last 90 days of devices, last 30 days of geo, payment-method delta, etc). Castle is more device-first; Sift is more account-first.

### 3.3 JS SDK pattern

Per [developers.sift.com/docs](https://developers.sift.com/docs):

| Component | Details |
|---|---|
| **Primary script URL** | `https://cdn.sift.com/s.js` |
| **SRI-pinned versioned script** | `https://cdn.sift.com/js/s-117.js` (and successive integer versions) |
| **First-party cookie** | `__ssid` — visitor ID, ~4-year lifetime |
| **Beacon key** | per-customer key configured in Console (under API Keys) |
| **REST API base** | `https://api.sift.com/v205/events` |
| **Auth method (server)** | HTTP Basic via `Authorization: Basic <b64(api_key:)>` OR `$api_key` JSON body field |
| **Decisions endpoint** | `POST /v3/accounts/{accountId}/decisions` |
| **Score endpoint** | `GET /v203/score/{userId}` |
| **Mobile SDKs** | `com.siftscience:sift-android` (Android), CocoaPods/Carthage/SwiftPM (iOS), `sift-react-native` (RN) |
| **Rate limits** | Events 500 req/sec (3/sec per user), Decisions 40 req/sec, Score 9 req/sec |

The `__ssid` cookie is the marker — its presence on any HTTP response identifies a Sift-instrumented site. The script load (`cdn.sift.com/s.js`) is the second-best marker; the cookie is set first-party so it shows up on the customer's domain, not on `cdn.sift.com`.

### 3.4 Risk scoring + decision flow

1. **`s.js` loads** on every page (configurable; default is every page where the snippet is included).
2. **JS posts events** to `https://api.sift.com/v205/events` — login attempts, page views, transactions, etc.
3. **Server side** (customer's backend) **calls Sift's Decisions or Score API** to get a risk classification.
4. **Sift returns** a score (0-100) + decision (`allow | watch | block`) + list of triggered abuse types (`payment_abuse`, `account_takeover`, `account_abuse`, `content_abuse`, `legacy`).
5. **Customer enforces** per their workflow.

Like Castle, Sift is advisory. The JS does not block. The decision is server-side.

### 3.5 Mitigation patterns

Sift exposes Workflows (visual rules engine) and Decisions (categorical labels). A workflow rule can:
- Apply a decision (`block`, `watch`, `allow`).
- Send a webhook (notify the customer's app).
- Set an account label (long-term).

The actual blocking is in the customer's code. Sift gives the customer the *decision*, not the *enforcement*.

### 3.6 BO coverage assessment

| Scenario | Outcome | Confidence |
|---|---|---|
| Public-page scrape, Sift `s.js` loads | **PASS** — Sift does not block in-browser; load the script, page hydrates | high |
| Public-page scrape, Sift fires `bot_behavior` because of identical stealth profile across 1000 sessions in 60 s | **AT RISK** — Sift's account-level rate intelligence will tag the IP / device-hash | medium |
| Auth-flow scrape | **OUT OF SCOPE** — Sift fires hardest on login / signup / payment | by design |
| Sift `__ssid` cookie reuse across mutually-distinct sessions | **AT RISK** — Sift treats __ssid as the visitor ID; reuse across distinct sessions inflates the device-velocity counter | medium (would need live measurement to confirm) |

The honest answer: **Sift is the most behaviorally-sophisticated of the four; on public-page scrapes it does not block, but its fingerprint will be visible to the customer's analytics and the customer's IT team may later use that fingerprint to throttle the IP at the WAF layer.** Indirect exposure, not direct blocking.

### 3.7 Public solver landscape

- No notable open-source "Sift bypass" projects on GitHub as of 2026-05-24.
- [about-fraud / Sift](https://www.about-fraud.com/providers/sift/) — industry-analyst overview, no technical exploit content.
- Commercial solvers (capsolver, 2captcha) do not list Sift; consistent with Sift's "no in-browser challenge" model (nothing to solve client-side).

**Landscape: zero meaningful open-source bypass surface.** This is because Sift's *blocking* is in the customer's code; you can't bypass Sift, you bypass the customer's enforcement of Sift's score.

### 3.8 Sources

- [sift.com platform overview](https://sift.com/platform/)
- [developers.sift.com / docs](https://developers.sift.com/docs)
- [G2 / Sift features](https://www.g2.com/products/sift/features)
- [about-fraud / Sift profile](https://www.about-fraud.com/providers/sift/)
- [Crunchbase / Sift](https://www.crunchbase.com/organization/sift-science)
- [TechCrunch — Sift Series F](https://techcrunch.com/2021/04/22/fraud-prevention-platform-sift-raises-50m-at-over-1b-valuation-eyes-acquisitions/)
- [Harvard d3 / Sift case study](https://d3.harvard.edu/platform-digit/submission/fighting-fraud-using-machines/)
- [GetApp / Sift](https://www.getapp.com/website-ecommerce-software/a/sift-science/)
- [Sift blog / key components of a fraud prevention platform](https://sift.com/blog/key-components-of-platform-solutions)

---

## 4. Forter

### 4.1 One-line + history

Forter (forter.com) is an e-commerce fraud-prevention platform founded 2013 in Israel, US HQ in New York. Specialty: **real-time decisioning** at checkout, with the unique architectural angle of a **cross-merchant identity network** ("Identity Graph"). Per [Forter blog / Nordstrom case study](https://www.forter.com/blog/how-nordstrom-strikes-balance/) + [Forter Trusted Identities brief](https://www.forter.com/wp-content/uploads/2021/09/Forter-Trusted-Identities-Solution-Brief-091321.pdf): the Identity Graph has surpassed 1 billion online identities; the pitch is "a fraudster recognizable to one retailer is known to all." Customer logos confirmed on forter.com main page: **Adidas, eBay, Grubhub, Instacart, Nordstrom, Wayfair, ASOS, Priceline, Puma**. (Sephora and Ralph Lauren appear in some third-party customer-list aggregations, but as of 2026-05 are not on the current forter.com landing.)

The Forter product line expanded from pure transaction-fraud to ATO ("Account Protection"), abuse prevention ("Abuse Prevention"), payment optimization, dispute management, and most recently agentic orchestration (per [forter.com](https://www.forter.com/)).

### 4.2 Detection signals

Per the [USPTO patents for Forter's behavioral biometric cookies](https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/9779423) + [Forter Trusted Identities brief](https://www.forter.com/wp-content/uploads/2021/09/Forter-Trusted-Identities-Solution-Brief-091321.pdf):

- **Device fingerprint**: canvas, WebGL, audio, navigator, screen, font enumeration, plugins. Plus mobile-specific signals on native SDKs.
- **Behavioral biometrics**: mouse trajectory, click cadence, scroll behavior, keystroke dynamics (per the patents — explicitly novel claims around "behavioral biometric cookies").
- **Network**: IP reputation, proxy/VPN detection, IP velocity.
- **Cross-merchant identity graph** (the marquee feature): given a {email, phone, billing-address, device-hash, payment-token} tuple, Forter knows whether this identity has previously transacted across any of its merchant network — and whether those transactions were fraudulent.
- **Session signals**: duration, pages visited, interaction counts.

### 4.3 JS SDK pattern

Per [roundproxies / Forter bypass](https://roundproxies.com/blog/bypass-forter/) + [EFF privacybadger issue #926](https://github.com/EFForg/privacybadger/issues/926) (which flagged Forter's CNAME-cloaking tracker):

| Component | Details |
|---|---|
| **CDN script URL** | `https://cdn.forter.com/...` (versioned; sometimes loaded via merchant-domain CNAME to evade tracker blockers) |
| **Primary cookie** | `forterToken` (also seen as `ForterTokenCookie` in patent docs) — encrypted blob containing session duration, pages visited, interaction counts, fingerprint hashes, behavioral metrics |
| **Server-side header** | typically a custom merchant header carrying the Forter token at the auth POST; Forter API integration is via REST |
| **Detection mechanism** | the JS continuously updates `forterToken` as the session progresses |

The **CNAME-cloaking** detail in the EFF issue is important: Forter is sometimes loaded from `tracking.{merchant}.com` (a CNAME pointing at Forter's infrastructure), specifically to bypass third-party-tracker blocklists. This makes Forter harder to identify by domain alone — the cookie name `forterToken` is more reliable.

### 4.4 Risk scoring + decision flow

1. **`cdn.forter.com` JS loads** on every page of protected sites.
2. **JS collects behavioral data** continuously, encrypts into `forterToken`.
3. **On a sensitive event** (checkout, login, account creation), customer's backend POSTs the user identity + Forter token + event context to Forter's REST API.
4. **Forter returns a decision** (`approve | decline | flag`) backed by the Identity Graph + behavioral signals + IP / device intelligence.
5. **Merchant** acts on the decision; Forter offers a chargeback-guarantee tier (they refund the merchant if a Forter-approved transaction is later disputed).

The chargeback guarantee is Forter's commercial wedge — competitors like Sift do not offer it.

### 4.5 Mitigation patterns

- **Approve** → transaction goes through.
- **Decline** → blocked at the merchant's checkout / login page (merchant chooses the message; Forter does not block in-browser).
- **Flag** → review queue.

Again: **Forter is advisory, not in-browser-blocking.** Public-page scrapes don't see a Forter challenge.

### 4.6 BO coverage assessment

| Scenario | Outcome | Confidence |
|---|---|---|
| Public-page scrape on a Forter-protected merchant | **PASS** — Forter JS loads, `forterToken` is set, scrape completes | high |
| BO triggering Forter's "headless browser" detection via a stealth-profile leak | **POSSIBLE LATER COST** — flag does not affect the current scrape, but the customer's IT team may later flag the IP at WAF | medium |
| Checkout-flow scrape | **OUT OF SCOPE** | by design |
| Forter on a site that *also* has Akamai BMP (e.g. adidas — both vendors confirmed) | **DOMINANT vendor is BMP** for our purposes; per chapter 26 the BMP fail mode dwarfs anything Forter would do | high — adidas is in our 126-corpus and the failure mode is `Akamai-CHL 2494`, not Forter |

The adidas data point is concrete and worth flagging: per `02_GAP_ANALYSIS.md` + the routed-sweep, adidas is an Akamai BMP cluster (3/4 BO profiles fail at the BMP interstitial; firefox uniquely passes via 1.3 MB body). Forter's presence on adidas does not change the BO failure mode — the BMP fail happens at the edge before Forter JS gets a chance to load.

### 4.7 Public solver landscape

- [roundproxies / Forter bypass 2026](https://roundproxies.com/blog/bypass-forter/) — generic bypass guide; no working `forterToken` encoder published.
- USPTO patents (cited above) — useful for understanding Forter's *claims* (what the cookie encodes), not enough to reproduce.
- GitHub: searches for "forter bypass" return ~3-5 inactive / abandoned scrapers; nothing actively maintained.
- Commercial solvers: anti-captcha / capsolver do not list Forter — same reason as Sift, no in-browser CAPTCHA to solve.

**Landscape: a handful of stale repos, USPTO patents are the highest-quality public technical artifact.** Forter's actual encoder is private and changes frequently per the bypass write-ups.

### 4.8 Sources

- [forter.com main](https://www.forter.com/)
- [forter.com / Nordstrom case study](https://www.forter.com/blog/how-nordstrom-strikes-balance/)
- [forter.com / Trusted Identities solution brief PDF](https://www.forter.com/wp-content/uploads/2021/09/Forter-Trusted-Identities-Solution-Brief-091321.pdf)
- [Forter Announces Trusted Identities — BusinessWire](https://www.businesswire.com/news/home/20211207005283/en/Forter-Announces-Trusted-Identities-to-Simplify-Authentication-for-eCommerce-Interactions)
- [EFF PrivacyBadger issue #926 (Forter CNAME cloaking)](https://github.com/EFForg/privacybadger/issues/926)
- [USPTO 9779423 — behavioral biometric cookies](https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/9779423)
- [USPTO 9531710 — behavioral authentication](https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/9531710)
- [USPTO 10298614 — behavioral biometric cookies cont.](https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/10298614)
- [roundproxies / bypass-forter](https://roundproxies.com/blog/bypass-forter/)
- [CIO Bulletin / Forter profile](https://ciobulletin.com/magazine/profile/forter-providing-a-holistic-fraud-prevention-solution-for-e-commerce-merchants)
- [devopsconsulting / top device-fingerprint tools](https://www.devopsconsulting.in/blog/top-10-device-fingerprinting-tools-features-pros-cons-comparison/)
- [Third Point Ventures / Forter](https://www.thirdpointventures.com/companies/forter/)

---

## 5. Akamai Account Protector

### 5.1 One-line + history

Akamai Account Protector (Akamai AP) is the ATO-specific product layered on top of Akamai's existing Bot Manager Premier (chapter 26) and the broader Akamai edge platform. Launched 2021-2022; major capability refresh October 2024 adding lifecycle protection (account creation → password reset → post-login activity), flexible risk management, and "advanced API operations" (per [Akamai press release, 2024-10-29](https://www.akamai.com/newsroom/press-release/akamai-account-protector-adds-new-capabilities-to-power-the-fight-against-fraud-and-abuse) + [SiliconANGLE](https://siliconangle.com/2024/10/29/new-akamai-account-protector-features-target-advanced-fraud-detection-across-user-accounts/)). Customers: existing Akamai BMP customers cross-sold + new fintech.

The key architectural framing per [techdocs.akamai.com / account-protector](https://techdocs.akamai.com/cloud-security/docs/account-protector): **"Bot Manager Premier tells you whether a request comes from a human; Account Protector tells you whether a request comes from a genuine user or someone using stolen credentials."** They layer:

1. BMP filters bot traffic (chapter 26 — `_abck`, `bm_sz`, sensor_data, sec-cpt).
2. AP scores authentic-human-but-possibly-impersonator traffic on the auth endpoints.

When you buy AP, you also get BMP capabilities (per techdocs).

### 5.2 Detection signals

Per [techdocs.akamai.com / account-protector](https://techdocs.akamai.com/cloud-security/docs/account-protector):

| Signal class | What it covers |
|---|---|
| **Individual user profiles** | Prior devices, prior locations, prior network/ASN, activity time-of-day patterns. UUID-keyed, independent of the username string (so a username change does not reset the profile). |
| **Population profiles** | When no user-history is available (first login, new account), AP falls back to baselines derived from Akamai's global delivery platform. |
| **Additional risk indicators** | Source reputation, behavioral anomalies, bot detections (from BMP). |

Plus, from the [Akamai AP product brief PDF](https://techcity.cloud/wp-content/uploads/2022/05/akamai-account-protector-product-brief.pdf): user-behavior telemetry, browser fingerprinting, automated-browser detection, HTTP-anomaly detection.

### 5.3 JS SDK / integration pattern

Akamai AP is **architecturally a sidecar to BMP** rather than a separate JS SDK. The data-collection layer is the *same* `_abck` / `bm_sz` / sensor_data infrastructure documented in chapter 26 — AP just adds a scoring path that runs at the login/signup endpoints.

The output signal is delivered to the customer via a single header: `akamai-user-risk` (per [Auth0 docs](https://auth0.com/docs/secure/attack-protection/configure-akamai-supplemental-signals) + [PingOne integration docs](https://docs.pingidentity.com/pingoneaic/latest/release-notes/rapid-channel/akamai-acc-protect-node.html) + [NextReason Threat Guard docs](https://docs.nextreason.com/docs/threat-guard-akamai-edge)).

| Component | Details |
|---|---|
| **JS data collection** | reuses Akamai BMP's `_abck` cookie infrastructure (chapter 26 `crates/akamai/src/payload.rs`, `sensor_data` POSTs, the `__akamai_events` collector preserved in `crates/browser/src/page.rs:1186-1196`) |
| **Cookie family** | `_abck`, `bm_sz`, `bm_mi`, `bm_so`, `ak_bmsc` — the existing BMP set |
| **Decision header** | `akamai-user-risk` (forwarded by Akamai edge to origin on auth requests, only when AP creates a risk score for that request) |
| **Header content** | risk category — `new device | high risk | medium risk | low risk | impossible travel`, per the NextReason integration |
| **Endpoint coverage** | configured per-tenant — customer specifies which paths (login, signup, password-reset, payment-update) AP evaluates |

This is the deepest convergence of an ATO product and a scraping-protection product in the industry: **the same telemetry layer feeds both decisions.** Implication for BO: solving Akamai BMP (chapter 26 / vendor_solvers) also addresses the data-collection side of AP; the *decision* side (scoring on the auth endpoint) is purely server-side and out of reach regardless.

### 5.4 Risk scoring + decision flow

1. **Page loads** with BMP infrastructure: `_abck` issued, sensor_data POSTed (per chapter 26).
2. **User attempts auth** — sends credentials to a configured AP-protected endpoint.
3. **Akamai edge evaluates the request** against the user profile (history) + population profile (baseline) + BMP signals (bot probability) + behavioral anomalies.
4. **AP computes a risk score** + risk category.
5. **Akamai edge adds the `akamai-user-risk` header** to the origin-bound request.
6. **Origin (customer's auth service)** reads the header and decides: allow / step-up MFA / block.

Like the other three: **Akamai AP is advisory.** It does not return a 403 itself. The customer's auth backend enforces.

### 5.5 Mitigation patterns

The Akamai console exposes a "User Risk Response Strategy" (per [techdocs / apr-ds-user-risk-response-strategy](https://techdocs.akamai.com/terraform/docs/apr-ds-user-risk-response-strategy)) where the customer maps risk categories to actions. The customer's *origin* enforces — Akamai AP just provides the score.

### 5.6 BO coverage assessment

| Scenario | Outcome | Confidence |
|---|---|---|
| Public-page scrape on an AP-protected site | **Identical to chapter 26 (BMP)** — AP only activates on configured auth paths | high |
| Auth-flow scrape on an AP-protected endpoint | **OUT OF SCOPE** — auth flows not a BO target | by design |
| Real-customer profile: a tenant with both BMP at the edge AND AP on `/login` | **BO sees only BMP** on public pages; AP is invisible to scraping | high |

The honest implication: **Akamai AP does not change the BO surface vs Akamai BMP.** Chapter 26's hard-residual (homedepot — BMP failure on the doc-20 sec-cpt path) is unchanged by AP's existence. Solving BMP solves the public-page surface; AP only matters if a customer asks to scrape *behind* a login.

### 5.7 Public solver landscape

- Akamai AP shares the same underlying envelope as BMP — sensor_data v2/v3, `_abck` state, sec-cpt — so the same private-solver investment (chapter 26 + `vendor_solvers`) covers both for data-collection. **The decision is server-side and unreachable.**
- [glizzykingdreko / Akamai v3 sensor_data](https://medium.com/@glizzykingdreko/akamai-v3-sensor-data-deep-dive-into-encryption-decryption-and-bypass-tools-da0adad2a784) — covers BMP-layer envelopes; this is the closest public artifact for AP-relevant work.
- No public "Akamai-user-risk header forgery" — that header is set by Akamai edge in-flight after evaluation; not user-controllable.

### 5.8 Sources

- [akamai.com / account protector product](https://www.akamai.com/products/account-protector)
- [techdocs.akamai.com / account-protector](https://techdocs.akamai.com/cloud-security/docs/account-protector)
- [techdocs.akamai.com / about bots](https://techdocs.akamai.com/cloud-security/docs/about-bots)
- [techdocs.akamai.com / integrate security solutions](https://techdocs.akamai.com/cloud-security/docs/integrate-security-solutions)
- [techdocs.akamai.com / handle adversarial bots](https://techdocs.akamai.com/cloud-security/docs/handle-adversarial-bots)
- [techdocs.akamai.com / user-risk-response-strategy](https://techdocs.akamai.com/terraform/docs/apr-ds-user-risk-response-strategy)
- [akamai.com / product brief PDF](https://www.akamai.com/resources/product-brief/account-protector)
- [techcity.cloud / AP product brief PDF mirror](https://techcity.cloud/wp-content/uploads/2022/05/akamai-account-protector-product-brief.pdf)
- [Akamai press release — AP capabilities 2024-10-29](https://www.akamai.com/newsroom/press-release/akamai-account-protector-adds-new-capabilities-to-power-the-fight-against-fraud-and-abuse)
- [SiliconANGLE — new AP features Oct 2024](https://siliconangle.com/2024/10/29/new-akamai-account-protector-features-target-advanced-fraud-detection-across-user-accounts/)
- [Auth0 / configure Akamai supplemental signals](https://auth0.com/docs/secure/attack-protection/configure-akamai-supplemental-signals)
- [PingOne — Akamai Account Protector node](https://docs.pingidentity.com/pingoneaic/latest/release-notes/rapid-channel/akamai-acc-protect-node.html)
- [NextReason — Threat Guard + Akamai Edge AP](https://docs.nextreason.com/docs/threat-guard-akamai-edge)
- [Gartner Peer Insights / Akamai AP reviews](https://www.gartner.com/reviews/market/online-fraud-detection/vendor/akamai/product/account-protector)

---

## 6. Cross-vendor patterns

### 6.1 What all four share

| Dimension | Castle | Sift | Forter | Akamai AP |
|---|---|---|---|---|
| **Device fingerprint** | yes (43 signals) | yes (canvas/WebGL/audio + behav) | yes (canvas/WebGL/audio + behav) | yes (via BMP infrastructure) |
| **Behavioral signals** | yes (mouse, keyboard) | **yes (heavy emphasis)** | **yes (USPTO-patented)** | yes (via BMP) |
| **IP/network intelligence** | yes (proxy/Tor/datacenter) | yes | yes | yes (Akamai global pool) |
| **Server-side scoring** | yes — `/v1/risk` | yes — `/events` + `/decisions` | yes — REST | yes — edge → `akamai-user-risk` header |
| **In-browser blocking** | **no** — advisory only | **no** — advisory only | **no** — advisory only | **no** — advisory only |
| **Real-time decisioning** | sub-100 ms claim | sub-second | sub-second (99% < 1s) | sub-second (edge-local) |
| **Cross-customer threat intel** | yes (Castle's network) | yes ("1T signals across 34k sites") | **yes (Identity Graph — the marquee feature)** | yes (Akamai global edge) |
| **First-party cookie?** | yes (`__cuid` third-party reports) | yes (`__ssid`) | yes (`forterToken`) | yes (`_abck`, etc — BMP infrastructure) |
| **Mobile SDK?** | yes (iOS + Android) | yes (iOS + Android + RN) | yes (deeply) | partial (BMP mobile SDK) |

The shared architectural pattern: **server-side decision, client-side telemetry collection, advisory output to the customer's auth service.** None of the four block in-browser on public pages. All four block (indirectly, via the customer's enforcement code) at auth endpoints.

### 6.2 How they differ

- **Castle**: **device-first, edge-deployable.** Castle's recent product direction (Cloudflare module, edge SDK) makes it the most likely of the four to fire on PUBLIC pages — they explicitly market this as a differentiator. The 43-signal catalog is the most detailed published model.
- **Sift**: **account-history-first.** Sift's strength is the *transaction history* model — given an account, what does the last 30/60/90 days of its activity look like, and is *this* request consistent with that. Bot-protection signals are secondary.
- **Forter**: **cross-merchant identity graph.** Forter's marquee is the Identity Graph (1B+ identities). A scrapeable user identity that triggers a fraud signal on `merchantA` will trigger that signal again on `merchantB` — even though the merchants don't share data with each other directly. From a scraping POV, this means: an IP / device combination flagged at one Forter customer might propagate.
- **Akamai AP**: **shares telemetry with BMP**, decisions deferred to origin. The unique aspect is the layered architecture — AP reuses BMP's `_abck` infrastructure but adds an auth-endpoint scoring path.

### 6.3 Convergence with anti-scraping vendors

The 2024-2026 trend: anti-scraping vendors adding ATO products (DataDome → DD Account Protect, chapter 37; Akamai BMP → Akamai AP, this chapter; HUMAN/PerimeterX → HUMAN Account Defender) and ATO vendors expanding to public-page coverage (Castle's edge deployment, Forter's "Trusted Identities" pre-auth signals).

**Implication for BO product strategy** (covered in `27 §9` customer-pitch lines): the customer's mental model is collapsing — "anti-bot" and "ATO" are becoming one vendor selection. Our marketing should mirror: a single sentence for "BO for public-page scrape; auth-flow ATO requires per-vendor solving in `vendor_solvers`" rather than treating them as separate questions.

---

## 7. Implications for BO scraping

### 7.1 The decision flow for a new customer / target

Reproduce this in customer onboarding:

```
Customer says: "We need to scrape <target site>."
              │
              ▼
1. Identify vendor stack with `crates/browser/src/page.rs:1054-1069` + chapter 18 markers.
              │
              ├─ Only scraping-protection (CF / DD-web / AWS WAF / BMP / Kasada / PX)
              │  → Standard chapters 06/07/08/25/26 path. Proceed.
              │
              ├─ Only ATO vendor (Castle / Sift / Forter / Akamai AP)
              │  → Public-page scrape: PASS expected (advisory-only model).
              │  → Auth-flow scrape: OUT OF SCOPE. Escalate.
              │
              └─ BOTH (very common — e.g. nordstrom.com)
                 │
                 ▼
                 Dominant blocker = the scraping-protection layer.
                 Solve via the relevant chapter; ATO vendor is invisible to BO.
```

### 7.2 The four specific patterns

#### 7.2.1 ATO-only public-page scrape

Castle, Sift, Forter all run JS, set first-party cookies, and *do not block*. BO's chrome_148 stealth profile (per `16_STEALTH_FINGERPRINT_AUDIT.md`) covers the device-fingerprint surface they collect; behavioral signals get a neutral baseline (no humanlike mouse — see `crates/browser/src/js/humanize.js`). Expected: scrape completes.

Risk to mitigate: behavioral signals that flag `bot_behavior` (Castle) / unusual session patterns (Sift). These don't block the current scrape but feed into the customer's analytics; if the customer's IT team later sees a wave of "high-risk no-mouse sessions" they may block at the WAF tier. This is an indirect, lagged risk — call it out in customer onboarding so the customer knows.

#### 7.2.2 ATO-only auth-flow scrape

BO does not support authenticated browsing as a first-class product (per `SCOPE.md`). Customer would need:
- Pre-existing session cookies from the customer's app (out of band).
- A solver chain that knows how to generate the vendor's request-token (`x-castle-request-token` for Castle, `forterToken` for Forter, `__ssid` + per-event signature for Sift, full Akamai BMP sensor_data + the AP scoring layer for Akamai AP).

This is a `vendor_solvers` companion-crate problem at minimum, and arguably a deeper "ATO-bypass" product line that BO does not aspire to.

#### 7.2.3 BOTH (the common case)

Per `27_VENDOR_COMPETITIVE_MATRIX.md`: every major retailer in the corpus that uses an ATO vendor *also* uses a scraping-protection vendor. The scraping-protection vendor is the dominant blocker for public-page scrapes. The ATO vendor's effect is invisible to BO unless the customer pushes into auth flows.

Concrete example: **adidas.com** uses Akamai BMP at the edge (per `26 §4.1`, BO firefox-only passes via 1.3 MB body, other profiles fail at `Akamai-CHL 2494`) AND Forter for transaction fraud (per Forter's customer page). For a product-page scrape, Akamai BMP is the entire problem. Forter's JS loads invisibly and has no effect on the scrape outcome.

#### 7.2.4 Forter Identity Graph propagation risk

Unique to Forter: a fingerprint that fraud-flags at one Forter customer may propagate to all Forter customers. This is theoretical for BO (we haven't observed it in measurement), but a customer who reports "we got blocked everywhere after one bad run" should be tested with a fresh stealth profile + fresh IP before assuming a deeper engine bug.

### 7.3 What to add to the engine

**Detection-only**, no solver work — extend the existing vendor-detect logger at `crates/browser/src/page.rs:1054-1069` with these ATO-vendor markers (rough sketch — actual implementation is in chapter 18 §4 / `crates/browser/src/classify.rs` headers section):

```rust
// (sketch — do NOT modify code; this is the seam to extend if a customer
//  scenario surfaces an ATO-vendor-related diagnostic need)
if let Some(v) = resp.headers.get("x-castle-request-token") {
    eprintln!("[vendor-detect] castle.io {} on {}", v, resp.url);
}
if let Some(v) = resp.headers.get("akamai-user-risk") {
    eprintln!("[vendor-detect] akamai-account-protector {} on {}", v, resp.url);
}
// Body markers:
//   `cdn.castle.io`           → Castle JS load
//   `cdn.sift.com/s.js`       → Sift JS load
//   `cdn.forter.com`          → Forter JS load (also CNAME-cloaked variants)
// Cookie markers (set by JS, observed on first response or later):
//   `__cuid` (Castle, unconfirmed),
//   `__ssid` (Sift, confirmed),
//   `forterToken` / `ForterTokenCookie` (Forter, confirmed via patents)
```

The reason these are detection-only and not classifier rows: ATO vendors do not produce a *blocking* response on public pages, so the classifier (which targets blocking) is not the right home. The vendor-detect logger captures "we saw vendor X" which is sufficient for diagnostics during a customer escalation.

---

## 8. Forward-looking

### 8.1 Shift-left to public pages

Castle is the leading edge of the trend — explicit edge-module deployment (Cloudflare Workers, custom edge stacks). If this generalizes to Sift / Forter / Akamai AP across 2026-2027, ATO vendors become an *active* blocker on public pages, not just an advisory layer.

What it would look like for BO: today's `chrome_compat.rs` profile may be inadequate if a Castle edge deployment specifically tags `headless_browser`. The right defense is to extend chapter 16's stealth audit to cover the Castle 43-signal catalog explicitly (today we cover most of it implicitly via Chrome-compat; the gaps are the behavioral signals — `bot_behavior`, `high_activity_device`).

### 8.2 Vendor consolidation / convergence

DataDome already has both products (DD-Web in chapter 07, DD-Account Protect in chapter 37). Akamai already has both (BMP in chapter 26, AP in this chapter). The unified vendor pitch is becoming standard.

Implication: a customer who currently sees "DataDome blocks 5/10 of my targets" will next year see "DataDome blocks 5/10 AND its ATO tier blocks the 2 sites I also wanted to auth-flow." This nudges the customer toward a single anti-vendor wedge — which is good for the `vendor_solvers` business case (one private solver covers both layers if it covers the underlying envelope).

### 8.3 Behavioral-biometric arms race

Castle's `bot_behavior` signal + Forter's USPTO-patented behavioral biometric cookies + Sift's "machine learning across 1T signals" all converge on the same gap: **lack of human-like input patterns from a headless browser.** Today's `crates/browser/src/js/humanize.js` (≈sparse mouse / scroll synthesis) is the start of an answer. A future humanize.js v2 (chapter ~40, speculative) covering keystroke cadence + dwell times + sigmoidal mouse trajectories is the chapter-42 cross-vendor unifier candidate.

This is explicitly aspirational, not v0.1.0 scope. Calling it out for the roadmap.

---

## 9. Acceptance + files

### 9.1 What "done" looks like for this chapter

This is a **reference encyclopedia**, not a planning document. Acceptance:

- [x] Per-vendor sections for all four target vendors (Castle, Sift, Forter, Akamai AP) covering: history, detection signals, JS SDK pattern, risk scoring, mitigation patterns, BO coverage assessment, public solver landscape, sources.
- [x] Cross-vendor pattern table (§6.1) showing the four vendors' shared and distinguishing characteristics.
- [x] Customer-onboarding decision flow (§7.1) so the first question on a new target is "ATO-only, scraping-only, or both?" rather than ad-hoc.
- [x] Forward-looking section flagging the shift-left, convergence, and behavioral-biometric trends.
- [x] Every URL cited inline with markdown links.
- [x] Honest uncertainty markers (`medium confidence`, `not directly observed`, `third-party reports`) where the public information is incomplete or where BO has no live measurement.

### 9.2 What this chapter explicitly does NOT include

- No code changes — `CLAUDE.md` / `SCOPE.md` makes per-vendor solving a `vendor_solvers` concern.
- No corpus-pass-rate impact estimate — these vendors are not in the 126-site corpus, so a delta cannot be measured today.
- No claim that BO solves any of these vendors at the auth-flow layer — by design.

### 9.3 Files cross-referenced

| File | Why referenced |
|---|---|
| `CLAUDE.md` | Out-of-scope policy: per-vendor solving → `vendor_solvers` companion crate |
| `SCOPE.md` | Auth-flow scope statement (out for the public engine) |
| `docs/releases/v0.1.0-parity/01_CURRENT_STATE.md` | Today's vendor coverage baseline |
| `docs/releases/v0.1.0-parity/16_STEALTH_FINGERPRINT_AUDIT.md` | Stealth profile coverage for device-fingerprint signals overlap with ATO vendors |
| `docs/releases/v0.1.0-parity/18_ANTI_BOT_VENDOR_COOKBOOK.md` | Vendor identification flowchart + the existing 12 vendor sections; this chapter is the ATO-vendor extension |
| `docs/releases/v0.1.0-parity/26_AKAMAI_BMP_DEEP.md` | Akamai AP shares the BMP telemetry layer; chapter 26 is the prerequisite read for §5 |
| `docs/releases/v0.1.0-parity/27_VENDOR_COMPETITIVE_MATRIX.md` | Per-vendor engine wins/losses; ATO vendors are explicitly outside the corpus |
| `docs/releases/v0.1.0-parity/37_REBLAZE_SUCURI_DDAP.md` | The companion chapter; covers DataDome Account Protect specifically (the DD-side analog of Akamai AP) |
| `crates/browser/src/page.rs:1054-1069` | Vendor-detect logger — the seam to extend with ATO markers (§7.3 sketch) |
| `crates/browser/src/classify.rs` | Classifier — explicitly NOT the right home for ATO markers (ATO vendors don't produce blocking responses on public pages) |
| `crates/browser/src/js/humanize.js` | Behavioral-biometric humanization seam — relevant for the §8.3 forward-looking note |
| `crates/stealth/profiles/chrome_148_*.yaml` | Stealth profile — covers ~80% of ATO-vendor device-fingerprint signals implicitly |

### 9.4 Open questions deferred to chapter 42 (cross-vendor synthesis)

- Does Castle's `bot_behavior` signal use a model that overlaps with Sift's behavioral model? (Both vendors describe similar signal collection; the model differences would matter for a "unified humanize" effort.)
- Forter's Identity Graph: what's the actual propagation latency between a fraud-flag on merchant A and the signal appearing on merchant B? (Operationally relevant; not in any public documentation.)
- Akamai AP's `akamai-user-risk` header: stable schema or rotating? (Per the Auth0 and PingOne integration docs the schema is `new device | high risk | medium risk | low risk | impossible travel` as of late 2024; not version-pinned.)
- All four vendors' mobile SDKs: do they share the same encoder family between web and mobile, or are they distinct codepaths? (Architecturally relevant — same encoder means one solver suffices.)

These belong in a future "cross-vendor pattern synthesis" chapter (provisionally chapter 42) once the v0.1.0 release stabilizes and customer escalations create real-data inputs.

---

## 10. Appendix — vendor-by-vendor quick lookup card

For the cookbook-style "I just saw this header / cookie / script, what is it?" lookup. Mirrors `18_ANTI_BOT_VENDOR_COOKBOOK.md §1` shape but for ATO vendors specifically.

### 10.1 Header markers

| Header | Vendor | Where it comes from |
|---|---|---|
| `x-castle-request-token` | Castle | Set by client JS, expected on server-bound POST to protected endpoints. **Not** set by Castle's CDN; set by the Castle Browser SDK in the calling page. |
| `akamai-user-risk` | Akamai Account Protector | Added by Akamai edge on auth-endpoint requests bound for origin. Schema: `category=<new device\|high risk\|medium risk\|low risk\|impossible travel>` plus optional score. |
| `Authorization: Basic <b64(api_key:)>` to `api.sift.com` | Sift | Server-to-server only; not visible in browser traffic. |
| No widely-published Forter-specific HTTP header | Forter | The `forterToken` cookie carries the data; server-to-server API is REST + custom auth. |

### 10.2 Cookie markers

| Cookie | Vendor | Set by | Lifetime |
|---|---|---|---|
| `__cuid` | Castle (third-party reports — not confirmed in Castle docs) | client JS, first-party on customer domain | session-scope |
| `__ssid` | Sift | `cdn.sift.com/s.js`, first-party on customer domain | ~4 years |
| `forterToken` / `ForterTokenCookie` | Forter | `cdn.forter.com` JS (or CNAME-cloaked equivalent), first-party | session-scope, refreshed continuously |
| `_abck`, `bm_sz`, `bm_mi`, `bm_so`, `ak_bmsc` | Akamai BMP (shared by Akamai AP) | Akamai edge | varies; see chapter 26 |

### 10.3 Script URL markers

| URL pattern | Vendor | Notes |
|---|---|---|
| `cdn.castle.io/...` (versioned) | Castle | Bootstrap loader; further requests go to `api.castle.io/v1/...` |
| `cdn.sift.com/s.js` or `cdn.sift.com/js/s-<N>.js` | Sift | SRI-versioned variant pins to a specific release |
| `cdn.forter.com/...` or `tracking.<merchant-domain>.com/...` (CNAME) | Forter | CNAME cloaking common to evade tracker blocklists per EFF PrivacyBadger issue |
| (BMP infrastructure) | Akamai AP | Shares chapter 26 markers; no AP-distinct script URL |

### 10.4 API endpoints (server-side, not browser-visible)

| Endpoint | Vendor | Method | Purpose |
|---|---|---|---|
| `https://api.castle.io/v1/risk` | Castle | POST | Per-event risk evaluation (login, signup) |
| `https://api.castle.io/v1/devices` | Castle | GET / POST | Device intelligence lookup |
| `https://api.sift.com/v205/events` | Sift | POST | Event ingestion (login attempt, transaction, page view) |
| `https://api.sift.com/v3/accounts/{accountId}/decisions` | Sift | POST | Decision query |
| `https://api.sift.com/v203/score/{userId}` | Sift | GET | Score query |
| Forter REST API | Forter | POST | Transaction / login authorization (URL varies per integration) |
| (Akamai AP) | Akamai | (edge → origin header injection) | Decision delivered via `akamai-user-risk` header, not a separate API call |

---

## 11. Appendix — comparative ranking against scraping-protection vendors

For the customer-onboarding reader who wants a single picture of "how sophisticated is ATO-vendor X compared to scraping-vendor Y." Subjective; calibrated against `27_VENDOR_COMPETITIVE_MATRIX.md` ratings.

| Vendor | Detection sophistication (0-10) | Behavioral-signal depth (0-10) | Cross-tenant intelligence (0-10) | Public-page blocking? | BO impact today |
|---|--:|--:|--:|---|---|
| **Kasada** (scraping) | 10 | 8 | 7 | yes — full block | 0/3 BO routed (per `27 §1`) |
| **Akamai BMP** (scraping) | 9 | 7 | 8 | yes — full block | 1/3 BO routed (per `27 §1`) |
| **Akamai Account Protector** (ATO) | 9 (inherits BMP) | 8 | 9 (BMP + AP global pool) | no — auth-endpoint only | N/A on public pages |
| **DataDome web** (scraping, chapter 07) | 9 | 7 | 7 | yes — full block | 3/4 BO routed |
| **Forter** (ATO) | 9 | 9 (USPTO-patented behav cookies) | 10 (Identity Graph 1B+ identities) | no — advisory | N/A on public pages |
| **Sift** (ATO) | 8 | 9 (model trained on 1T signals across 34k sites) | 9 (network effect) | no — advisory | N/A on public pages |
| **Castle** (ATO) | 8 | 7 | 6 (smaller network) | no — advisory, but edge-deployable (shift-left risk) | PASS on public pages; risk only if customer enables Castle-at-edge |
| **AWS WAF** (scraping, chapter 06) | 8 | 5 | 7 (AWS global pool) | yes — full block | 4/8 BO routed |
| **Cloudflare Managed** (scraping, chapter 25) | 8 | 5 | 9 (CF global pool) | yes — full block | 7/7 BO routed except iphone-profile gap |
| **PerimeterX/HUMAN** (scraping) | 8 | 7 | 8 | yes — full block | 1/1 BO routed (zillow) |
| **Reblaze** (scraping, chapter 37) | 7-9 (variable per customer config) | 6-9 (biometric tier optional) | 5 (per-customer VPC, less network effect) | yes — full block | N/A — not in corpus |
| **DataDome Account Protect** (ATO, chapter 37) | 9 (inherits DD-web) | 8 (login-page interaction + AI thresholds) | 8 (DD network) | no — auth-endpoint only | N/A on public pages |
| **Sucuri** (scraping, chapter 37) | 4 | 1 (none) | 3 (SMB pool) | yes — JS shield | PASS via standard loop |

The picture for ATO vendors: **all four cluster at 8-9 on detection sophistication, 7-9 on behavioral depth, 6-10 on cross-tenant intelligence — but none block public pages today.** This is the "advisory" model of §6.1. The cross-tenant intelligence column is the most differentiated: Forter's 1B+ identity graph and Akamai AP's BMP-leveraged global pool are the most valuable assets (and the hardest to evade), but neither asset matters for our public-page scraping use-case.

---

## 12. Appendix — a customer-pitch one-liner for each vendor

For the sales-doc author who needs a single sentence per ATO vendor in the customer-pitch slide (mirrors `27 §9` for scraping vendors):

- **Castle:** "BO clears Castle on public pages today; for auth-flow scraping behind Castle, expect this to require a private custom solver (escalation path)."
- **Sift:** "BO clears Sift on public pages today; Sift's account-history model is server-side and out of reach — focus on Sift's customer's enforcement rules, not Sift itself, for any blocking behavior."
- **Forter:** "BO clears Forter on public pages today; Forter's Identity Graph means fingerprints propagate across merchants, so use a fresh stealth profile per customer if scraping at scale across multiple Forter-protected merchants."
- **Akamai Account Protector:** "AP layers on Akamai BMP — if you can scrape Akamai BMP-protected pages today, you don't see AP on those scrapes. AP only matters at login endpoints, which BO doesn't target."

---

## 13. Appendix — relationship to the public engine's `aecdf19` strip

The `aecdf19` vendor-strip commit (2026-05-21, referenced extensively in chapter 26 §1) removed every per-vendor solver from the public tree. The policy framing: per-vendor encoder code lives in the private `vendor_solvers` companion crate; the public engine ships the *seams* (the `ChallengeSolver` trait, the body-marker classifier, the vendor-detect logger) but never the vendor-specific encoders.

This policy applies fully to the four ATO vendors covered here:

- **Castle:** any future `CastleSolver` (forging `x-castle-request-token`, mimicking the 43-signal output) → `vendor_solvers`.
- **Sift:** any future `SiftSolver` (mimicking `s.js` event-stream + decision-API forgery) → `vendor_solvers`.
- **Forter:** any future `ForterSolver` (forging `forterToken` encrypted blob, satisfying USPTO-patented behavioral cookie schema) → `vendor_solvers`.
- **Akamai AP:** shares the existing `crates/akamai/` private codepath — no AP-specific solver needed beyond extending the existing BMP `sensor_data` encoder with auth-endpoint context (a `vendor_solvers` problem, not a public-engine problem).

The seams in the public engine that the ATO-vendor private solvers would attach to:

| Public seam | Location | What it enables for ATO solvers |
|---|---|---|
| `ChallengeSolver` trait | `crates/browser/src/challenge.rs:55-161` | Private ATO solvers implement this trait; `Page::navigate_with_solvers(...)` plugs them in. |
| `ChallengeKind` enum | `crates/browser/src/challenge.rs` | Would need new variants for ATO triggers (`AccountProtectorRiskScore`, `CastleRiskCheck`, etc) — additive, no breaking change. |
| Vendor-detect logger | `crates/browser/src/page.rs:1054-1069` | Already the extension point per §7.3; ATO markers added at the existing locations. |
| `__akamai_events` JS collector | `crates/browser/src/page.rs:1186-1196` | DEAD in the public engine post-strip, but the JS surface still emits counters — Akamai AP private solver could consume directly without re-instrumenting. |

The key engineering invariant: **adding ATO solvers to `vendor_solvers` does not require any change to the public engine.** This is the same property that holds for the existing scraping-vendor solvers per the `aecdf19` design and is the reason the public engine's coverage is invariant to the existence of any private solver.
