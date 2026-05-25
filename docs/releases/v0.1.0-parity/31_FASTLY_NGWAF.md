# 31 — Fastly Next-Gen WAF (formerly Signal Sciences)

**Status:** reference (customer-onboarding playbook for a vendor NOT in the 126-corpus)
**Audience:** customer-onboarding engineers, sales engineers writing pitch decks against Fastly-protected targets, and the maintainer extending the vendor cookbook (chapter 18) to cover edge-compute WAFs as they show up in support tickets.
**Companion docs:** `18_ANTI_BOT_VENDOR_COOKBOOK.md` (the encyclopedic vendor index — this chapter is the deep dive for the Fastly row that doesn't exist there yet), `23_TLS_HTTP_FINGERPRINT_REFERENCE.md` (the load-bearing capability for the Fastly story — our byte-perfect Chrome TLS via `boring2`), `27_VENDOR_COMPETITIVE_MATRIX.md` (cross-vendor engine comparison), `25_CLOUDFLARE_DEEP.md` (the structural sibling — both Fastly NGWAF and Cloudflare are CDN-integrated edge-compute WAFs).

**Why this chapter exists.** Fastly Next-Gen WAF (NGWAF), formerly Signal Sciences before [Fastly's $775 M acquisition in October 2020][fastly-press], is the third tier of CDN-integrated WAF after Cloudflare and AWS WAF. **It does not appear in our 126-site corpus** — none of the holistic-sweep targets sits behind Fastly's edge with the NGWAF product active in a blocking posture. But Fastly NGWAF is **increasingly common in 2025-2026** in engineering-led shops (DoorDash, Spotify, GitHub, Reddit, T-Mobile per Fastly customer pages), and `~/projects/browser_oxide_internal` ticket history shows enterprise customers asking "does your scraper engine work on Fastly-protected sites?" before signing. This chapter answers that question with the precision of the deep-dive cluster (chapters 06/07/08/25/26), but the honest framing of a **non-corpus vendor**: every BO-coverage claim here is **inference from architecture**, not measured sweep data. Customers must run a pilot on their specific target before relying on the predictions.

**One-paragraph thesis.** Fastly NGWAF inherits Signal Sciences' "SmartParse" detection model (per-request contextual analysis, language-agnostic agents) and is fundamentally a **network-layer + custom-rule WAF**, not a heavy-JS challenger like Kasada or DataDome. JS-based client challenges exist (the newer `_fs_ch_st_*` / `_fs_ch_cp_*` cookie family, post-2023 Bot Management add-on) but the vendor's gravitational center is **edge-rule evaluation by JA3/JA4 + ASN + custom VCL signals before any HTML is served**. This is **the configuration BO is structurally strongest against**: our byte-perfect Chrome TLS ClientHello via `boring2 4.15` (chapter 23) reproduces a real-Chrome JA4 the Fastly edge cannot distinguish from a real desktop Chrome connection. **Predicted default outcome: BO passes Fastly NGWAF without per-vendor work on the vast majority of customer-onboarding pilots.** The two failure modes to spec for: (a) a customer-specific VCL rule that targets a signal BO doesn't yet faithfully reproduce (Worker headers, specific Accept-CH advertising, etc.), and (b) the rare Bot Management JS challenge tier when the customer pays for the Premier add-on. Per-tenant investigation, not per-vendor work.

---

## 1. Product overview and history

### 1.1 Signal Sciences as the pre-acquisition product

Signal Sciences was founded in 2014 by Andrew Peterson, Zane Lackey, and Nick Galbreath (per [Signal Sciences history at AppSec Santa][appsec-history]); the company shipped a **next-generation WAF + RASP product** that pioneered three things the legacy ModSecurity-class WAFs lacked:

1. **SmartParse**: a per-request contextual analysis engine that "analyzed requests in application context to detect zero-day attacks with significantly reduced false positives" ([Fastly press release][fastly-press]). The bet was that bolted-on regex rule packs (mod_security, OWASP CRS) generated too many false positives to deploy in front of real applications; SmartParse evaluated request semantics (the *interpretation* a downstream parser would produce, not just the literal bytes).
2. **Lightweight agents**: language-agnostic agents that ran inside the customer's application stack (Apache module, Nginx module, IIS module, a Go SDK, a Python SDK, etc.) and forwarded request features to a central cloud engine for scoring. The split-architecture was deliberately developer-friendly — DevOps teams could deploy without re-architecting their web tier.
3. **Developer-first APIs**: full configuration + telemetry exposed through a REST API ([Fastly NGWAF API docs][fastly-api]), so security configuration could live in Terraform and CI alongside application code.

By 2020 Signal Sciences was **protecting more than 40,000 applications and over a trillion production requests per month** ([Fastly press release][fastly-press]) — the largest-by-volume independent WAF before the acquisition. Customer concentration was heavily engineering-led (the company sold to platform/SRE teams who needed a WAF that fit into their existing CI rather than to a separate security org).

### 1.2 The Fastly acquisition

Fastly announced the acquisition on 2020-08-27 and closed it on 2020-10-01 for **approximately $775 million in cash and stock** (per [Fastly press release][fastly-press] and [Fastly investor announcement][fastly-investor]). The strategic logic — quoted from the press release — was:

> "The transaction combines Signal Sciences' powerful web application and API security solutions with Fastly's edge cloud platform and existing security offerings to form a unified suite of modern security tools, designed for the way developers work."

The product was rebranded to **Fastly Next-Gen WAF** and pivoted from a "deploy our agents in your app" model to a "deploy us at the Fastly edge" model. The pre-2020 agent-based deployment (now called **On-Prem WAF**) remains supported for customers not using Fastly's CDN, but the new strategic deployment is **Edge WAF** — NGWAF running inside Fastly's Varnish-derived edge nodes ([Fastly architecture docs][fastly-architecture]).

### 1.3 The 2025-2026 product line

Per [Fastly's official product page][fastly-product] and [docs][fastly-products-ngwaf], the current line:

| SKU | Deployment | Description |
|---|---|---|
| **Fastly Next-Gen WAF** | Three modes: Edge, On-Prem (formerly Core), Cloud | The flagship — SmartParse + REST API + customer-rules-via-console |
| **Bot Management (add-on)** | Edge WAF only | JS-based client challenges (`_fs_ch_st_*` / `_fs_ch_cp_*` cookies), JA4 fingerprint inspection (post-Feb 2025) |
| **ContentGuard** | Edge | Pre-cache bot detection at the network perimeter |
| **Secure@Edge** (legacy) | Edge | Original post-acquisition unified product name; superseded by the Bot Management add-on |
| **Compute** (orthogonal — programmable rules) | Edge | Fastly's serverless platform; customers write custom security logic in Rust/JS/Go that runs at every PoP |

Plan tiers (per [client-challenges docs][fastly-client-challenges]): Essential / Professional / Premier. **Client challenges require Professional or Premier**; the Essential tier is detect-only.

---

## 2. Architecture

### 2.1 Edge WAF (the modal deployment for 2025-2026 customers)

Per [Fastly deployment docs][fastly-deployment]:

> "The Edge WAF deployment method hosts the Next-Gen WAF on Fastly's Edge Cloud platform via the global network of POPs and integrates with Fastly's caching layer, Varnish. Since security processing happens at the edge, the Next-Gen WAF can inspect all traffic before it enters your origin infrastructure and block attacks close to where they originated."

The three architectural components ([Fastly architecture docs][fastly-architecture]):

| Component | Role |
|---|---|
| **NGWAF module** | The hook embedded in each Fastly edge node that intercepts requests and forwards features to the agent |
| **NGWAF agent** | Sidecar process at the edge that runs SmartParse + custom-rule evaluation against the per-request features; emits a verdict (allow/block/challenge) |
| **Cloud engine** | Central scoring + telemetry aggregation; receives features from agents across all PoPs, returns updated rule sets + threat intelligence |

The critical structural difference from the heavy-JS vendors (Kasada / Akamai BMP / DataDome): **NGWAF makes its verdict at the network layer based on connection metadata + request features**. There is no requirement that a JS challenge run on the client for the verdict to land — the verdict lands at `vcl_recv` time, before any HTML is served. JS challenges are an *escalation tier* the WAF can deploy when the network-layer score is ambiguous; they are not the primary detection mechanism.

This is the **opposite design philosophy** to DataDome (chapter 07) and Kasada (chapter 08), where the JS challenge IS the detection mechanism and network-layer signals are advisory.

### 2.2 On-Prem (formerly Core) WAF

Per [Fastly deployment docs][fastly-deployment]:

> "The On-Prem WAF (formerly known as Core WAF) deployment method hosts the Next-Gen WAF directly on your local environment, which means you are responsible for managing the deployment. The On-Prem WAF deployment method consists of two components, the module and the agent."

This is the original 2014-2020 Signal Sciences deployment. Same agent + module architecture, but the customer hosts both. **Network-layer signals available to On-Prem are weaker** (no Fastly-edge JA3/JA4 fingerprinting unless the customer terminates TLS at a JA4-aware proxy and forwards the fingerprint as an HTTP header). For BO-coverage purposes: a customer running On-Prem NGWAF has **fewer levers to detect us by TLS class** than Edge WAF, but the same SmartParse + custom rules apply at the application layer.

### 2.3 Cloud WAF

Per [Fastly deployment docs][fastly-deployment]:

> "Cloud WAF is hosted on Fastly's cloud-hosted infrastructure, and to use it, you must upload a TLS certificate, add an origin server using the Next-Gen WAF control panel, and update your DNS records to point to the appropriate servers."

A managed-Fastly-edge-but-not-using-Fastly-CDN-for-content option. Architecturally identical to Edge WAF from the request-path perspective.

### 2.4 Compute (programmable VCL/serverless)

Fastly's edge serverless product ([Fastly Compute][fastly-compute-tutorial]). Customers write **arbitrary security logic in Rust, JavaScript, or Go** that runs at every edge node before / alongside the NGWAF module. This is the **per-customer wildcard** in the Fastly story — a Compute-using customer can write a JA4-block-list, a path-rate-limiter, a behavioural-cookie-checker, anything. From the scraper-engine perspective:

- **Standard NGWAF rules are knowable** (we can predict what they trigger on from the docs).
- **Compute custom rules are opaque** (every customer's logic is different; we can only diagnose by per-tenant capture).

When a Fastly-protected customer onboards and BO fails, the **first hypothesis must be a Compute custom rule** (§7 below), not a SmartParse rule (which is documented and predictable).

### 2.5 No JS injection by default

This is the headline architectural property to internalize. Quoted from [Fastly Bot Management docs][fastly-bot-mgmt]:

> "Advanced client-side detection requires modify[ing] the HTML code of your website to include a JavaScript snippet to identify headless browser activity."

I.e. JS-side detection is **opt-in for the customer**, not built-in to NGWAF. The default Edge WAF deployment inspects the *network-layer + HTTP-layer* request and either passes or denies it — there is no `<script src="signalsciences.com/...">` reflexively injected into every Fastly-protected page (the way DataDome injects `dd-script.js` from `captcha-delivery.com`). **The absence of a reflexive client-side fingerprint is a structural advantage for BO**: most failure modes of BO against heavy-JS vendors (Kasada CSS calc, AWS WAF SDK fingerprint) don't apply to a vanilla Fastly NGWAF deployment.

---

## 3. Detection signals

### 3.1 JA3 fingerprinting (legacy)

Fastly has supported per-request JA3 fingerprint exposure to VCL via `tls.client.ja3_md5` for years ([Fastly VCL var docs][fastly-ja3]). NGWAF custom rules can act on JA3 signatures — per [Fastly's unified CDN+WAF blog][fastly-blog-unified]:

> "Fastly's CDN can utilize JA3 and various headers to enrich data and improve visibility in the NGWAF."

> "A beta product classifies automated bot requests based on JA3 signature, making it easier for users to block requests based on a signal type."

The customer-side configuration is a one-line VCL snippet that pushes `tls.client.ja3_md5` into a request header the NGWAF agent then evaluates. Block rules typed against the JA3 list block by TLS fingerprint.

**BO coverage of JA3 blocks.** Per chapter 23 §2.1: "we don't compute or publish JA3 — we only target the JA4 family. The JA3 of our handshakes is whatever falls out of the JA4-correct configuration." Concretely, BO's byte-perfect Chrome 147 ClientHello produces the **same JA3 as real Chrome 147** (because JA3 = `MD5(version, ciphers, extensions, groups, ec_formats)`, and our cipher list / extension list / group list / EC formats are byte-identical to the verified-real Chrome reference per `crates/net/src/tls.rs:60-76` and the `tls_fingerprint_vectors_no_silent_drift` test at `crates/net/src/tls.rs:476-553`). Customer JA3-block lists generally target **known headless-bot JA3 hashes** (the rustls JA3, the curl JA3, the Python `requests` JA3); the real-Chrome JA3 is *not* on those lists by design. Predicted outcome: **JA3-based Fastly rules don't block BO**.

### 3.2 JA4 fingerprinting (current, post-Feb 2025)

Per [Fastly's JA4 announcement][fastly-ja4-announcement]:

> "The feature was announced in February 2025 as a new enhancement to the Bot Management product. JA4 fingerprinting provides 'more detailed client identification during TLS-encrypted communications,' expanding beyond existing fingerprinting methods to analyze client behavior in secure connections."

Available as the `tls.client.ja4` VCL variable ([JA4 VCL var docs][fastly-ja4-vcl]):

> "STRING variable available in read-only mode, accessible in recv, hash, deliver, and log phases. This variable computes 'the JA4 fingerprint from the TLS Client Hello packet' as documented in the official JA4 technical specifications."

Edge deployment requirement: **version 2.10.0 or later**. Customers on older Edge agents need a VCL service re-map.

**BO coverage of JA4 blocks.** Per chapter 23 §2.3-2.4: BO's expected JA4 prefix is `t13d1516h2_*_*` (TLS 1.3, SNI present-domain, 15 ciphers, 16 extensions, h2 ALPN). The two 12-hex hashes (cipher hash and extension+sigalg hash) are stable as long as the cipher / sigalg / extension *sets* don't change — Fisher-Yates extension shuffle (which BO does per `tls.rs:222-228`) doesn't change the JA4 hash because JA4 sorts codepoints before hashing. We do not have a **published verified-real JA4 baseline** in tree (chapter 23 §2.4 is explicit: "We do NOT have a current published JA4 in the repo"). **First customer-pilot capture must verify**: run BO against a Fastly-protected target that exposes `tls.client.ja4` via response header (configurable in the customer's VCL), capture the value, diff against a real Chrome 148 sample. If they match, Fastly JA4-based rules are predicted to pass us.

The two ways BO's JA4 could fail to match real Chrome:

1. **Extension count drift.** If a Chrome major adds or removes one TLS extension, our 16-extension `CHROME_EXTENSION_PERMUTATION` (`tls.rs:516-520`) goes stale and the JA4 metadata segment becomes `t13d1517h2_*` vs real Chrome's `t13d1518h2_*` (or vice versa). The fix is in chapter 23 §refresh-cadence: re-capture a real-Chrome ClientHello quarterly.
2. **Cipher / sigalg set drift.** Similar: Chrome adding a cipher to the list changes the hash. Same refresh cadence applies.

The risk is **low and known**, with a tracked mitigation. For customer onboarding: **a Fastly-protected target newer-than-Feb-2025 that the customer has explicitly enabled Bot Management on is the only Fastly-NGWAF scenario where TLS fingerprint is the load-bearing signal we need to verify per-pilot.**

### 3.3 HTTP/2 fingerprint inspection

Less explicitly documented by Fastly than JA3/JA4, but Fastly NGWAF inspects HTTP/2 frame characteristics (SETTINGS frame values, WINDOW_UPDATE behavior, pseudo-header order) as part of the "client fingerprinting" rule type ([Fastly Bot Management docs][fastly-bot-mgmt]). The custom-rules engine can match on header-value entropy, header-order patterns, and request-shape anomalies.

**BO coverage of H2 fingerprint inspection.** Per chapter 23 §3-4: BO's `crates/net/src/h2_client.rs` ships a Chrome-compatible HTTP/2 client built on `h2-bot-protect 0.5` — SETTINGS frame matches Chrome (ENABLE_PUSH=0, MAX_CONCURRENT_STREAMS=1000, INITIAL_WINDOW_SIZE=6291456, MAX_FRAME_SIZE=16384, MAX_HEADER_LIST_SIZE=262144, HEADER_TABLE_SIZE=65536), pseudo-header order is `:method` `:authority` `:scheme` `:path` (Chrome order, vs Firefox's `:method` `:path` `:authority` `:scheme`), priority frames disabled, WINDOW_UPDATE matches Chrome's curve. Predicted outcome: **H2-fingerprint-based Fastly rules don't block BO**.

The verification gap: chapter 23 explicitly notes we don't have an in-tree H2-fingerprint verification test against `tls.peet.ws` or similar. **First customer pilot should capture H2 features and verify they match real Chrome's** as a baseline.

### 3.4 Header entropy and header-order rules

Per [Fastly NGWAF rules docs][fastly-rules]: customers can write rules that match on header values, header presence/absence, and header-order patterns. Common rules (informed by the [purpleax/sigsci-demo][sigsci-demo] examples):

- Block when `User-Agent` matches a known-bad-bot regex.
- Block when `Accept`, `Accept-Language`, `Accept-Encoding` are missing or have suspicious values.
- Block when `Sec-CH-UA*` client hints are missing on a UA claiming Chrome 100+ (browsers from that era always send them).
- Block when header order differs from "real-Chrome canonical order" (this is a stretch rule; most customers don't write it).

**BO coverage of header-based rules.** The `crates/net/src/headers.rs` Chrome-class header set is faithful by construction (chapter 23 §6 catalogues the per-profile header lists). The known weak spots:

- **Accept-CH advertising on the response.** BO does process `Accept-CH` headers (added per commit `f62584d` — process-wide SharedSession that picks up `accept_ch` from responses), so when a Fastly origin advertises `Accept-CH: Sec-CH-UA-Arch, Sec-CH-UA-Bitness, Sec-CH-UA-Model, ...`, our next request includes those headers. Verify per-pilot.
- **Sec-Fetch-* headers.** Faithful per chapter 23 §6.3.
- **Cookie order on multi-cookie sites.** SharedSession bleed mid-sweep (`f62584d` commit messages) was previously a problem; verify with single-target captures.

Predicted outcome: **header-based Fastly rules generally don't block BO**, but per-tenant per-pilot verification is warranted because the customer's specific rule set may target a signal we don't yet faithfully emit.

### 3.5 Request rate and ASN

NGWAF supports rate-limit rules + ASN-based blocking ([Fastly NGWAF rules docs][fastly-rules]):

> "Custom rules can act on ASN-based Rules: Target specific network providers distributing attacks. Proxy Headers: Block bulletproof hosting, anonymous proxies, and TOR traffic."

**BO coverage of ASN/IP rules.** Out of engine scope — the IP we present is whatever the user routes us through. The standing project policy per `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/proxy_not_the_problem.md`: **diagnose engine gaps, not IP reputation**, but recognize that some Fastly-protected customers will block by ASN. For customer-onboarding: a target where BO fails on every IP class but Camoufox passes from the same IP class would indicate an engine gap; **BO failing only from datacenter ASNs while passing from residential IPs indicates an IP-reputation issue, not an engine issue**.

### 3.6 SmartParse (the application-layer detection)

The original Signal Sciences innovation. Per [Fastly architecture docs][fastly-architecture]:

> "The Next-Gen WAF uses SmartParse, a highly accurate detection method, to evaluate the context of each request and how it would execute, to determine if there are malicious or anomalous payloads in requests."

This is **payload-attack detection** — SQLi, XSS, command injection, log4j-style RCE strings. **It is not bot detection.** Scrapers issuing benign GET requests with no malicious payloads do not trigger SmartParse; the engine evaluates the request *body* and *parameters* for evidence of an attack against the downstream application interpreter.

**BO coverage of SmartParse.** N/A — we don't generate attack payloads. If a customer's site triggers SmartParse on our scraper traffic, the cause is almost certainly a customer-side parameter-name overlap (our scraper sending `?q=something` to a search endpoint that happens to have a flawed SmartParse rule); per-tenant fix, not per-vendor.

### 3.7 Custom rules (the per-tenant wildcard)

Per [Fastly NGWAF rules docs][fastly-rules]:

> "The NGWAF console supports signature-based blocking rules that can act on: JA3 Signatures... ASN-based Rules... Proxy Headers..."

Every Fastly customer writes their own rule set. **This is the single largest unknown for any BO-vs-Fastly customer pilot.** A customer can:

- Block our specific JA3/JA4 if they've added it to a denylist (unlikely — they'd be denylisting real Chrome 147).
- Block our specific ASN (likely if we're on a datacenter IP and they have an "ASN deny" rule for datacenter ASNs).
- Block requests with `Sec-CH-UA: ""` (empty), if their rule asserts the header must have a chrome-formatted value (we ship the header faithfully).
- Block requests where `accept-language` doesn't match the geo-IP region (standard sophisticated rule; our profile presets ship `en-US,en;q=0.9` and that's what gets sent).
- Add a rate-limit per IP that we trip when running a high-concurrency scrape.

The customer-onboarding playbook (§7) catalogues the diagnostic order.

---

## 4. Mitigation patterns

### 4.1 Block (hard deny at the edge)

Per [Fastly rules docs][fastly-rules-conditions], supported custom response codes are 301, 302, and 400-599. The default block returns 406 Not Acceptable or a customer-configured 403. **Body is customer-configurable** — by default a small Fastly-branded HTML page; many customers replace it with a tenant-branded page.

Detection from the scraper side:
- Status code in `{403, 404, 406, 429}` (depending on rule)
- Response body 1-5 KB, plain HTML
- Headers include the standard Fastly identifiers (§6 below)

### 4.2 Soft block (rate limit / tarpit)

Per [Fastly blog on unified CDN+WAF][fastly-blog-unified]:

> "Penalty Box: Temporary IP-based blocking with configurable duration. Tarpit Responses: Delays responses to slow attackers. Variable Status Codes: Returns randomized 4xx responses to confuse reconnaissance."

Tarpit returns the request but slowly (10-30 s response time). The intent is to consume scraper-pool capacity without giving the scraper a clean "this IP is banned" signal.

Detection from the scraper side:
- Long TTFB (10+ seconds) that doesn't match the customer's normal response time
- Response body may still be valid HTML (the customer doesn't always serve a block page; the tarpit is the punishment)
- Difficult to distinguish from a slow origin without baseline measurements

**BO coverage.** Our default per-iteration timeout is 30 s (per `crates/browser/src/page.rs:1973`); a tarpit at 10-15 s consumes a navigation iter but doesn't fail the navigate. Tarpit on every subsequent request from the IP would eventually exhaust the 90 s navigation budget — same UX as a hard block, slower. No engine-side fix; **rotate IP** or back off the scrape rate.

### 4.3 Rate limit (429 with Retry-After)

Per [Fastly rate-limit docs][fastly-rules]: standard 429 with `Retry-After: <seconds>` header. The navigate loop at `crates/browser/src/page.rs:1045` already treats `403/429/498` identically for the initial-challenge logging. The `Retry-After` header is **not honored by the BO navigate loop today** (verify with grep); standard behavior is to retry on next iter without backing off.

**Recommendation for Fastly-rate-limited customers**: throttle scrape concurrency at the application layer (the caller scheduling work onto `Page` instances) rather than expecting the engine to honor `Retry-After`. This is consistent with how chapter 22 §production-deployment positions BO — the engine is the per-request faithful client; rate-limit-respect lives in the orchestration layer.

### 4.4 Challenge (the JS PoW tier)

The newer Bot Management add-on (post-2023, expanded Feb 2025 with JA4). Per [Fastly client-challenges docs][fastly-client-challenges]:

> "Fastly offers three client challenge variants:
> - **Dynamic Challenge**: Fastly 'automatically choose[s] the most appropriate client challenge based on the situation', potentially including Private Access Tokens, non-interactive challenges, or interactive ones when suspicious activity is detected.
> - **Non-Interactive Challenge**: Uses 'JavaScript proof-of-work' requiring clients to 'solve what is essentially a JavaScript math problem' to demonstrate browser capability.
> - **Interactive Challenge (CAPTCHA)**: Presents 'a random alphanumeric string' that users must enter to prove human access."

Deployment modes:

> "Interstitial pages: Standalone challenge pages that interrupt user flow.
> Embedded challenges: Integrated directly into application pages (recommended for React and single-page applications).
> Only dynamic and non-interactive challenges support interstitial delivery; interactive challenges only embed within pages."

When triggered:

> "The system tags requests with the `CHALLENGED` signal. Upon solving, clients receive a token stored as a browser cookie with a '1 hour expiration' default. Solved challenges are marked with the `CHALLENGE-TOKEN-VALID` signal."

The cookies (per [Fastly client-challenges docs][fastly-client-challenges]):

| Cookie | Purpose |
|---|---|
| `_fs_ch_st_<RANDOM_STRING>` | Challenge **start** cookie — set when challenge issued; mitigates trivial replay |
| `_fs_ch_cp_<RANDOM_STRING>` | Challenge **complete** cookie — set after JS PoW succeeds; signals access permitted |

> "You should not manipulate the Set-Cookie response header or the Cookie request header in VCL, as these headers are essential for identifying initiated and solved challenges via the `_fs_ch_st_<RANDOM STRING>` and `_fs_ch_cp_<RANDOM_STRING>` cookies."

**BO coverage of JS challenges.** Architecturally similar to Cloudflare's JS Challenge (chapter 25 §2.1) — the JS PoW is a math problem evaluated by the engine. The token-cookie pattern mirrors `cf_clearance` (chapter 25 §1.4). Per the generic primitives in chapter 07 (`07_DATADOME_PRIMITIVES.md`):

- **Primitive 1 (CSP relax)**: needed if the challenge JS is loaded from a Fastly-CDN host that the origin's CSP doesn't list. Not specified by Fastly docs whether the challenge JS is inline or external-CDN; assume external — `challenges.fastly.com` or similar (verify per-pilot).
- **Primitive 2 (rematerialize iframes)**: probably not needed for the non-interactive PoW (no iframe used); needed for interactive CAPTCHA variant.
- **Primitive 3 (cookies-carry-clearance + delta retry)**: needed. The `cookies_carry_anti_bot_clearance` predicate in chapter 07 §Primitive 3 should be extended to recognize `_fs_ch_cp_` as a clearance-bearing cookie name.

The engine seam already in place: the navigate loop's challenge-document detection (`crates/browser/src/page.rs:1638-1663`) is body-marker-based; **no Fastly-specific marker is currently in the classifier**. §6 below specifies the additions.

### 4.5 Honeypot routes

Some Fastly customers configure honeypot URLs (paths that no legitimate user would request — `/robots.txt`-disallowed paths, hidden links, etc.) that, when accessed, automatically add the requesting IP to the deny list for N minutes. This is a **customer configuration**, not an NGWAF feature. From the scraper side: only manifests as "we got banned after one request to /some-path-we-discovered-by-grep". The mitigation is robots.txt respect at the scheduler layer, not the engine layer.

---

## 5. BO coverage — strong on this one

This is the chapter's editorial point. **BO is structurally well-positioned against Fastly NGWAF** because:

### 5.1 The TLS argument

Per chapter 23 §1: BO uses `boring2 4.15` (Cloudflare's BoringSSL fork) to emit a **byte-perfect Chrome 147 ClientHello** verified against a real Chrome capture. Per chapter 23 §3 (HTTP/2 fingerprint): we ship Chrome-class SETTINGS frame, pseudo-header order, WINDOW_UPDATE behavior. Per chapter 23 §2.4: our JA4 metadata segment is `t13d1516h2_*` which matches real Chrome.

Fastly NGWAF's strongest signal class — TLS/HTTP fingerprint at the edge — is the **single signal class BO invests the most in** (chapter 23 is one of the longest chapters in the v0.1.0-parity plan, and the underlying `crates/net/` code is the project's most-developed network stack). **Predicted outcome: Fastly's network-layer signals do not distinguish BO from real Chrome on a vanilla NGWAF deployment**.

### 5.2 The "WE WIN" framing (with caveats)

If a customer brings a Fastly-protected site to a BO onboarding pilot, the expected default outcome is **BO passes without per-vendor work**. The caveats:

- **JA4 verification gap.** Per chapter 23 §2.4: we do not yet have a published verified-real JA4 baseline in tree. The customer-pilot's first action must be a JA4 capture comparison against a real Chrome 148 sample. If they don't match, fix at chapter 23 (cipher list, extension list, or sigalg list drift).
- **Compute custom rules.** Per §3.7: a customer using Fastly Compute can write arbitrary deny logic that no public docs predict. Per-tenant capture + diagnose; not a general BO issue.
- **Bot Management Premier add-on with JS PoW.** Per §4.4: the `_fs_ch_st_*` / `_fs_ch_cp_*` cookie-bearing challenge tier IS a heavy-JS challenge that needs the chapter 07 primitives generically applied. The challenge URL pattern is not yet documented in BO's classifier (§6 below).

### 5.3 Comparison to other vendor clusters

Cross-referencing chapter 27 §1 vendor-cluster totals (with the caveat that **Fastly is NOT in the 126-corpus** so this is inference, not measurement):

| Vendor cluster | BO routed expected | Camoufox expected | PW family expected | Why |
|---|---|---|---|---|
| **Fastly NGWAF (vanilla)** | **PASS** | PASS | PASS | Network-layer signals; all real-browser-class engines clear |
| **Fastly NGWAF + Bot Management Premier (JS PoW)** | predicted PASS (after chapter 07 primitives applied + Fastly-specific markers) | PASS | PASS | JS PoW is solvable headlessly; similar to Cloudflare JS Challenge |
| **Fastly NGWAF + customer Compute deny rule** | **DEPENDS** | DEPENDS | DEPENDS | Per-tenant — can target any signal class; not a vendor-general statement |

Compare to chapter 27 §1:
- Cloudflare Managed Challenge (iphone-class): BO routed wins 7/7, Camoufox 7/7, PW family 0/7 — **Fastly should be at least this trusting** because Fastly's bot detection is "less mature [than Cloudflare], with Signal Sciences WAF being excellent but bot detection not as sophisticated as Cloudflare or Akamai, requiring more custom rules" per [the 2025 TrustRadius comparison][trustradius-vs-cf].
- AWS WAF (Amazon WAF-tenant): BO routed 0/5, Camoufox 1/5, PW family 4/5 — Fastly is **NOT** in this cluster; the Amazon-tenant WAF rule set is aggressive, Fastly's default rule set is not.

The structural classification: **Fastly NGWAF belongs to the "TLS-fingerprint-first, JS-challenge-secondary" vendor family**. The closest analogue in the 126-corpus is **PerimeterX on zillow** (per chapter 27 §2.1 — the "marquee BO advantage" where our Chrome-class TLS beats PerimeterX's behavioural challenge). **Predicted outcome: Fastly customers in this profile see BO outperform Camoufox and Patchright/Playwright by a similar mechanism — our TLS class trust + clean Chrome JS surface, without the CDP-detect penalty that costs PW family.**

### 5.4 What "well-positioned" doesn't mean

It does **not** mean "BO passes every Fastly-protected site automatically". A Fastly customer who:

- Maintains a custom JA4 denylist that includes our specific JA4 (because they captured it and added it; unlikely without ours leaking publicly).
- Uses Compute to write a behavioral rule that targets our specific request cadence.
- Requires Private Access Token attestation (Apple/Google native crypto attestation — not a JS PoW; we cannot satisfy this from a headless browser).

…will block us. The customer-onboarding playbook (§7) is the per-pilot diagnostic.

---

## 6. Detection markers (page-load level)

To extend `crates/browser/src/classify.rs:81-156` and `crates/browser/src/page.rs:1049-1057` (vendor-detect header logger). These are the recognition markers a contributor onboarding a Fastly-protected site should look for.

### 6.1 Response headers (highest precision)

Per [Fastly header reference][fastly-headers]:

| Header | Source | Meaning |
|---|---|---|
| `x-served-by: cache-<airport>-<podid>-<n>, cache-<airport>-<podid>-<n>` | Every Fastly-served response | Identifies the edge cache nodes. Format: `cache-<airport-code>-<region-id>-<sequence>` (e.g. `cache-FYI-FYI222-1` for Fresno PoP) |
| `via: 1.1 varnish` (sometimes multiple) | Every Fastly response | Standard HTTP `Via` header — Varnish is Fastly's underlying cache daemon |
| `x-fastly-request-id: <40-hex>` | Every Fastly response | Unique per-request identifier for log correlation (per [http.dev guide][http-dev-xfrid]) |
| `x-cache: HIT` / `MISS` | Cache hit/miss indicator | Common across all Fastly responses |
| `x-timer: S<unix>.<frac>,VS<n>,VE<n>` | Fastly cache timing | Internal Varnish timing fields |
| `x-cache-hits: <n>` | Hits count | Fastly cache hit counter |
| `fastly-debug-digest: <hex>` | When enabled by customer | Debug info |

**No NGWAF-specific response header is documented as universally present** on every NGWAF response. The closest is the `_fs_ch_st_*` / `_fs_ch_cp_*` Set-Cookie when a challenge is in flight. **Fastly identification is via the cache headers + the Via header, not a single canonical "NGWAF active" header**.

### 6.2 Response body markers

By default NGWAF block pages are **customer-configurable** — there is no canonical "Fastly-branded block page" the way there is a "Cloudflare-branded error 1020 page". Some customers leave the default Fastly templates in place; the default block body typically contains:

- `Sorry, you are not authorized to access this page.` or similar generic deny
- `Powered by Fastly` footer (rare; customer-removed in most production deployments)
- No `<script>` tags in the default block path

For challenges (Bot Management add-on):

- The non-interactive JS PoW page contains a `<script>` tag loading the challenge runtime — exact host **not publicly documented**. Probable hosts: `challenges.fastly.com`, `f.signalsciences.com`, or a per-tenant subdomain. **First customer-pilot capture identifies the exact host**.
- The interactive CAPTCHA variant embeds a widget; format not yet publicly catalogued in security-research literature.

### 6.3 Cookie markers

Per §4.4:

| Cookie name pattern | When set | Lifetime |
|---|---|---|
| `_fs_ch_st_<random>` | Challenge issued | Brief (during challenge) |
| `_fs_ch_cp_<random>` | Challenge solved | 1 hour default |

The `<random>` suffix per-tenant prevents straightforward predicate matching by exact name. The cookbook's `cookies_carry_anti_bot_clearance` predicate (chapter 07 §Primitive 3) should be extended with a **prefix match** on `_fs_ch_cp_` rather than an exact name match.

### 6.4 Proposed engine additions

Following the pattern of chapter 18 §4.1-4.3, add to the engine:

**Header logger** (`crates/browser/src/page.rs:1049-1057`):

```rust
// Fastly identification (any Fastly response — not specifically a challenge)
if resp.headers.iter().any(|(k, _)| {
    let lk = k.to_ascii_lowercase();
    lk == "x-fastly-request-id" || lk == "x-served-by"
}) {
    eprintln!("[vendor-detect] fastly-edge on {}", resp.url);
}
```

Note: do NOT eagerly log on every Fastly-edge response — too noisy. Gate on either a challenge-shaped status (`403/429/498/503`) or pair with a body-marker check (next).

**Body markers** in `crates/browser/src/classify.rs:81-156`:

```rust
// Fastly NGWAF Bot Management JS challenge (post-2023 add-on)
// Conservative: marker is the cookie name in body OR a known challenge JS host
"_fs_ch_st_"           // INTERSTITIAL_COSIGNAL when paired with small body
"_fs_ch_cp_"           // CLEARANCE_COSIGNAL when in Set-Cookie
"challenges.fastly.com" // probable challenge JS host (unverified; pilot capture)
```

**v8_html_is_real guard** in `crates/browser/src/page.rs:2273-2293`:

```rust
&& !v8_html.contains("_fs_ch_st_")  // Fastly challenge in progress
```

**Clearance-cookie predicate** in `cookies_carry_anti_bot_clearance` (chapter 07 §Primitive 3):

```rust
// Prefix match (the cookie name has a per-tenant random suffix)
new_set_cookie_names.iter().any(|n| n.starts_with("_fs_ch_cp_"))
```

All four additions are **detection-only or generic** — no Fastly-specific solver logic, no vendor-encoder code. The G6 strip policy per `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/state_2026_05_16_phase5_datadome.md` is respected: vendor-specific encoders go in private `vendor_solvers`, but markers + clearance-cookie predicates are public-engine fair game (they parallel chapter 25's Cloudflare additions and chapter 26's Akamai cookie state-machine recognition).

### 6.5 Acceptance gate for the markers

After the additions land:

- The 126-site sweep MUST not regress (no false-positive Fastly classification on non-Fastly sites — `_fs_ch_st_` is a unique-enough string that collision risk is negligible).
- For any future customer-onboarding pilot against a Fastly-protected target, the `[vendor-detect] fastly-edge` log line appears in BO's per-iter output, confirming the engine identifies the vendor.
- If a Fastly Bot Management challenge fires, the `_fs_ch_cp_*` clearance is honored via chapter 07 §Primitive 3 generic plumbing.

---

## 7. Public solver landscape

Fastly NGWAF and Signal Sciences have **very limited public bypass research**. Compare to:

- AWS WAF: 7+ public GitHub bypass projects catalogued in chapter 18 §2.1.
- DataDome: 7+ public projects in chapter 18 §2.2.
- Cloudflare: dozens of projects + commercial services (chapter 25 §6).
- Kasada: 5+ public projects (chapter 18 §2.4).
- **Fastly NGWAF: effectively zero**.

Per the GitHub search results in `select:WebSearch` for "site:github.com signal sciences NGWAF bypass research":

- [`vercel-labs/vercel-bulk-waf-rules`](https://github.com/vercel-labs/vercel-bulk-waf-rules) — *Management tool*, not a bypass; exports rules from NGWAF to Vercel WAF for migration purposes.
- [`FastlySE/NGWAF-Cookbooks`](https://github.com/FastlySE/NGWAF-Cookbooks) — *Fastly-official* Terraform examples for configuring NGWAF (the opposite of bypass).
- [`signalsciences/*`](https://github.com/signalsciences) — Signal Sciences' own org, 21 repos of SDKs + integrations.
- [`ArroyoLabs/signalsciences-nginx`](https://github.com/ArroyoLabs/signalsciences-nginx) — community Nginx integration.
- [`purpleax/sigsci-demo`](https://github.com/purpleax/sigsci-demo) — Terraform demo for Fastly Cloud WAF.

**No third-party bypass projects on GitHub.** Two reasons:

1. **Fastly's strength is per-customer rule customization, not a uniform vendor-wide protocol.** There is no "Fastly challenge.js" to deobfuscate the way AWS WAF or DataDome ship a uniform SDK. Each customer's deny logic is different.
2. **The TLS-fingerprint-first detection model is generic** — bypassing Fastly TLS detection is equivalent to bypassing TLS detection in general (i.e., emit a real-Chrome ClientHello), which is what every modern stealth-client project does anyway. There's no Fastly-specific exploit because the generic Chrome-TLS-impersonation work covers it.

The implication for BO: **there is no "lift this open-source solver, adapt it" path** the way chapter 06 has for AWS WAF. The strategy is generic-Chrome-class fidelity (chapter 23) + the chapter 07 primitives for the JS PoW tier + per-tenant capture for Compute custom rules.

### 7.1 The closest analogues

For BO maintainers researching Fastly customer pilots, the closest research is the **CDN-integrated WAF research** generally — the Cloudflare bypass guides (chapter 25 §6) apply to Fastly's analogous JS-challenge product, the JA4-spoofing research applies to Fastly's JA4-based rules.

Useful references:

- [JA4 specification (FoxIO-LLC)](https://github.com/FoxIO-LLC/ja4) — the reference for understanding what `tls.client.ja4` returns.
- [JA4 fingerprinting now supported in Bot Management — Fastly announcement][fastly-ja4-announcement] — what Fastly's JA4 detection actually does.
- The [`tls.peet.ws`](https://tls.peet.ws/api/all) endpoint — for capturing BO's actual JA4 and comparing against real Chrome.
- The [Scrapfly bypass index](https://scrapfly.io/bypass) — does not list Fastly NGWAF as a separate entry (consistent with the "no Fastly-specific bypass" observation).

---

## 8. Customer onboarding playbook

When a prospective customer brings a Fastly-protected target. Run these steps in order; each produces artifacts the next needs.

### 8.1 Step 1 — Identify Fastly

```bash
URL='https://customer-site.example/'
curl -sS -D headers.txt "$URL" -o body.html
grep -iE '^(via|x-served-by|x-fastly|x-timer|x-cache):' headers.txt
```

If any of those headers appear, the site is behind Fastly. Specifically:

- `x-served-by: cache-*` → Fastly edge
- `via: 1.1 varnish` → Fastly (Varnish is Fastly's cache daemon)
- `x-fastly-request-id:` → unambiguous Fastly

NGWAF activity is **not** indicated by these headers (Fastly customers may use Fastly CDN without NGWAF). To confirm NGWAF:

- Look for `_fs_ch_st_` or `_fs_ch_cp_` cookies in the Set-Cookie headers.
- Try a known-bad request (e.g. `curl ... 'https://customer-site.example/?q=<script>'` — a SmartParse-tripping XSS payload). If the request is blocked with a different status than the benign request, NGWAF is active.
- Some customers expose `signal-sciences.com` references in their privacy policy / cookie consent dialog.

### 8.2 Step 2 — Default expectation: pass

If the customer-site is Fastly-edged + NGWAF-active + no Bot Management Premier add-on (the modal 2025-2026 configuration for an engineering-led shop), **predicted outcome is BO passes without per-vendor work**. Run a 5-URL pilot with BO chrome/pixel/iphone/firefox profiles via:

```bash
target/release/examples/sweep_metrics chrome_148_macos \
    /tmp/fastly_pilot_corpus.json /tmp/bo_out.json --capture customer-site
```

Where the corpus file lists 5 URLs from the customer's site. If all 4 profiles pass strict L3 (per chapter 03 methodology: `tag == "L3-RENDERED" AND len ≥ 15000`), the customer is **good to onboard with default routing**.

### 8.3 Step 3 — If blocked: diagnose the rule class

If BO fails the pilot, the diagnostic order:

1. **Compare against real Chrome from the same IP.** Run `chromium --headless=new --dump-dom <url>` from the same machine. If real Chrome also fails, the issue is **IP reputation, not engine fingerprint** — out of engine scope (per `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/proxy_not_the_problem.md`, with the inverse: when real Chrome ALSO fails, it really is an IP issue).
2. **Compare against Camoufox from the same IP.** If Camoufox passes and BO fails, it's an engine fingerprint gap. Run the chapter 04 capture tooling (`--capture` flag) and diff `fetches.json` between BO and Camoufox.
3. **Capture the JA4 BO emits.** Run BO against `https://tls.peet.ws/api/all` (or the customer's own JA4-exposing endpoint if they enable it via VCL `set resp.http.X-JA4 = tls.client.ja4;`). Compare against a real Chrome 148 capture from the same client. If they differ, fix at chapter 23 (cipher list, extension permutation, or sigalg list drift).
4. **Capture the response body of the block page.** If it contains `_fs_ch_st_` or `_fs_ch_cp_`, the JS challenge tier is firing. Apply chapter 07 §Primitives 1+2+3 plus the §6.4 Fastly-specific marker additions.
5. **Capture headers BO sends.** Diff against real Chrome's headers from the same target. Specifically check: `Sec-CH-UA*`, `Accept-CH` advertisement on the customer's response (some customers require all advertised hints).
6. **Suspect Compute custom rule.** If steps 1-5 don't isolate the cause, the customer is likely running Compute-based custom logic. Ask the customer (or, if they're a paying lead, their security team) for a description of the deny rule.

### 8.4 Step 4 — Travel + airline + e-commerce caveats

Industries where Fastly + custom rules combine into hard scrape:

- **E-commerce checkout flows.** Often have a Compute-based behavioral rule that requires a session cookie set via a JS callback before the checkout endpoint is accessible. Scrapers that only fetch product pages typically pass; scrapers attempting checkout typically fail.
- **News + paywall sites.** Often combine NGWAF + a paywall plugin (Piano, Tinypass). NGWAF passes us; paywall blocks the article body. Out of scope — paywalls are content-licensing, not bot-detection.
- **Engineering-led SaaS products** (GitHub, Spotify, DoorDash etc.). Generally permissive — these are companies who deploy NGWAF for the WAF/RASP value, not aggressive bot blocking.

### 8.5 Step 5 — High-risk scrape rate considerations

If the customer-target is a Fastly Bot Management Premier tenant with custom rate-limit rules:

- Default BO concurrency at the orchestration layer should be 1-3 requests/sec per IP for Fastly-protected targets (vs. 5-10/sec for non-WAF targets).
- Honor `Retry-After` at the scheduler layer, not the engine layer.
- Distribute across IPs if scraping at scale (the `crates/pool` design supports this, with concurrency limits per-IP).

### 8.6 Acceptance for the playbook

After running the playbook on a customer-pilot:

- Either the customer is onboardable with default settings (modal case).
- Or the diagnostic isolated a specific cause: TLS drift (fix at chapter 23), JS challenge needed (apply chapter 07 primitives), Compute custom rule (per-tenant work), IP reputation (out of scope).
- Or — rare — neither BO nor Camoufox passes; treat as the open frontier per chapter 02 §hard-residual.

---

## 9. Forward-looking

### 9.1 Fastly's 2025-2026 roadmap

Per [Fastly's NGWAF announcements page][fastly-announcements] and [the JA4 changelog entry][fastly-ja4-announcement], the publicly-visible roadmap:

- **JA4 fingerprinting in Bot Management** — landed Feb 2025. Establishes Fastly as JA4-aware (catching up with AWS WAF which added JA4 in March 2025 per [AWS announcement](https://aws.amazon.com/about-aws/whats-new/2025/03/aws-waf-ja4-fingerprinting-aggregation-ja3-ja4-fingerprints-rate-based-rules/)).
- **Compute + Bot Management convergence** — expected. Customers writing custom bot rules in Compute that consume the Bot Management signals (challenge state, JA4, etc.) directly rather than via the NGWAF console.
- **Private Access Tokens (PAT)** — already supported as one of the dynamic-challenge variants per [client-challenges docs][fastly-client-challenges]. Apple/Google native attestation; **not satisfiable from a headless browser without a real iOS/Android device that has the platform's attestation key**. Out of engine scope.
- **AI-driven anomaly detection** — broad industry trend, not Fastly-specific. Expect more behavioral / per-customer-baseline scoring; less deterministic detection.

### 9.2 The convergence with the broader CDN+WAF market

Per the [TrustRadius vs. Cloudflare comparison][trustradius-vs-cf]:

> "Fastly has 100+ PoPs vs Cloudflare's 310+, which means slightly higher latency in less-served regions like Africa, South America, and rural Asia."

> "Fastly's bot management is less mature, with Signal Sciences WAF being excellent but bot detection not as sophisticated as Cloudflare or Akamai, requiring more custom rules."

Fastly's positioning is **engineering-led customer base + per-customer rule sophistication** rather than **CF's "we have ML against the entire internet's traffic" approach**. The implication for BO:

- Fastly customers who write strong custom rules will be hard to scrape (Compute is unbounded).
- Fastly customers who use defaults will be easy to scrape (TLS-fingerprint-first detection at the edge defaults trusts real-Chrome-class TLS).
- The bimodal pattern means **per-tenant pilots are mandatory**; we cannot predict the outcome from "this site is on Fastly".

### 9.3 What would change BO's posture against Fastly

If Fastly:

- **Ships a default JS challenge on every NGWAF-active response** (high-friction; unlikely, as it would break their "developer-first, minimal-disruption" positioning).
- **Adds reflexive WASM-based attestation** like DataDome's `dd-script.js` model (architecturally inconsistent with Fastly's edge-compute focus; very unlikely).
- **Convinces every customer to enable Bot Management Premier + Compute custom-rule sophistication** (slow customer rollout; unlikely to be modal before late 2026).

Then the BO-vs-Fastly story shifts toward needing the same per-vendor depth we have for Kasada (chapter 08) and DataDome (chapter 07). Until then, **chapter 23 (TLS fidelity) + chapter 07 (generic primitives) + this chapter's §6 detection markers covers the realistic 2025-2026 customer-pilot space**.

---

## 10. Acceptance + files

### 10.1 Acceptance gate for this chapter (when the §6.4 additions land)

- `crates/browser/src/page.rs:1049-1057` (vendor-detect header logger) extended with the `fastly-edge` log line per §6.4.
- `crates/browser/src/classify.rs:81-156` extended with the `_fs_ch_st_` / `_fs_ch_cp_` markers per §6.4.
- `crates/browser/src/page.rs:2273-2293` (v8_html_is_real guard) extended with the `_fs_ch_st_` non-acceptance per §6.4.
- The `cookies_carry_anti_bot_clearance` predicate from chapter 07 §Primitive 3 extended with the `_fs_ch_cp_` prefix match per §6.4.
- 126-site sweep does NOT regress (no false-positive Fastly classifications; the marker strings are unique enough that collision risk is negligible).
- Manual customer-pilot validation: at least one BO-vs-real-Chrome capture on a Fastly-protected target verifies the JA4 match and the `[vendor-detect] fastly-edge` log line.

### 10.2 Files this chapter touches when its plan executes

| File | Section | Change |
|---|---|---|
| `crates/browser/src/page.rs:1049-1057` | §6.4 | Add `fastly-edge` header-logger line (4 lines) |
| `crates/browser/src/classify.rs:81-156` | §6.4 | Add `_fs_ch_st_` and `_fs_ch_cp_` markers as `INTERSTITIAL_COSIGNAL` and `CLEARANCE_COSIGNAL` (2 lines) |
| `crates/browser/src/page.rs:2273-2293` | §6.4 | Add `_fs_ch_st_` to `v8_html_is_real` guard (1 line) |
| `crates/browser/src/page.rs` (the `cookies_carry_anti_bot_clearance` predicate; chapter 07 §Primitive 3 owns the location) | §6.4 | Extend the clearance-cookie name list with a `starts_with("_fs_ch_cp_")` check (1 line within the existing predicate) |

Total: **8 lines of code, no new vendor encoder, no new private-crate dependency**. The chapter is mechanically tiny — the value is in the customer-onboarding playbook (§8) and the architectural understanding (§2-5) that informs sales / pitch decks.

### 10.3 Files this chapter does NOT touch

- **No new private `vendor_solvers` module needed** for vanilla Fastly NGWAF coverage. The Bot Management JS PoW tier is generically handled by chapter 07 primitives + §6.4 markers above.
- **No `crates/net/` changes needed** — chapter 23's TLS work is the prerequisite, not part of this chapter's plan.
- **No new chapter-31-specific test** — the corpus-wide regression gate via `crates/browser/tests/holistic_sweep.rs` catches false-positive Fastly classifications on non-Fastly sites.

### 10.4 Cross-references

This chapter is the customer-onboarding deep-dive equivalent of:

- Chapter 06 (AWS WAF deep dive) — but Fastly is the "we win" vendor rather than the "we lose without a solver" vendor.
- Chapter 07 (DataDome primitives) — the JS PoW tier reuses the same three engine primitives generically.
- Chapter 25 (Cloudflare deep dive) — Fastly is structurally the same vendor class (CDN-integrated WAF); the iphone-class gap from chapter 25 §3 may or may not appear against Fastly's JA4 rules (per-pilot verification needed).
- Chapter 27 (vendor competitive matrix) — Fastly is the **non-corpus vendor cluster** that, when measured per-pilot, is predicted to land in the BO-wins quadrant.
- Chapter 18 (anti-bot vendor cookbook) — the canonical entry for any future cookbook update should reference this chapter.

Companion to this chapter on the customer-onboarding theme: **chapter 32 (Radware Bot Manager / ShieldSquare)** — the other non-corpus vendor that comes up in onboarding tickets, structurally the opposite of Fastly (heavy-JS behavioral instead of light TLS-edge).

---

## 11. Honest uncertainty footnotes

This is the discipline correction. **Fastly NGWAF is not in the 126-corpus**; every BO-coverage claim in §5 is **inference from architecture + chapter 23 TLS fidelity claims**, not measured sweep data. The honest statements:

1. **JA4 verification gap.** We do not have a published verified-real JA4 in tree (chapter 23 §2.4). Until the first customer-pilot capture verifies the match, **§3.2 prediction of "JA4-based Fastly rules don't block BO" is unproven**.
2. **HTTP/2 fingerprint verification gap.** We do not have an in-tree H2-fingerprint verification test (chapter 23 §3 verification gap). Same caveat as §3.3.
3. **Bot Management JS challenge analysis** is based on the public Fastly docs (cookies, expected behavior); **no in-the-wild capture exists in tree**. The §6.2 statement about probable challenge host (`challenges.fastly.com` or similar) is **inference** — first customer-pilot capture identifies the actual host.
4. **No measured engine-vs-Fastly outcome exists** in `~/projects/browser_oxide_internal/benchmarks/` or `docs/research_*` directories. Every "predicted PASS" in §5.3 is inference.
5. **Customer-pilot history**: ticket history in `~/projects/browser_oxide_internal/docs/` references Fastly-protected customer prospects but no completed pilot results are catalogued. The first completed pilot will inform a future revision of §5 with measured numbers.

Per CLAUDE.md `MEASUREMENT TRAP` discipline: **`*-CHL` is not "blocked" raw; size-gate ≥30 KB = rendered FP**. When the first Fastly-vs-BO measurement lands, apply the same gate. Until then, this chapter is **engineering reasoning, honestly labelled as such, suitable for customer pitch decks with the "predicted, pending pilot" hedge**.

---

[fastly-press]: https://www.fastly.com/press/press-releases/fastly-fastly-completes-acquisition-signal-sciences-acquisition-of-signal-sciences
[fastly-investor]: https://investors.fastly.com/news/news-details/2020/Fastly-Completes-Acquisition-of-Signal-Sciences/default.aspx
[appsec-history]: https://appsecsanta.com/signal-sciences
[fastly-product]: https://www.fastly.com/products/web-application-api-protection
[fastly-products-ngwaf]: https://docs.fastly.com/products/fastly-next-gen-waf
[fastly-api]: https://www.fastly.com/documentation/signalsciences/api/
[fastly-deployment]: https://www.fastly.com/documentation/guides/next-gen-waf/setup-and-configuration/about-deploying-the-next-gen-waf/
[fastly-architecture]: https://www.fastly.com/documentation/guides/next-gen-waf/getting-started/about-the-architecture/
[fastly-bot-mgmt]: https://docs.fastly.com/products/bot-management
[fastly-ja3]: https://www.fastly.com/documentation/reference/vcl/variables/client-connection/tls-client-ja3-md5/
[fastly-ja4-vcl]: https://www.fastly.com/documentation/reference/vcl/variables/client-connection/tls-client-ja4/
[fastly-ja4-announcement]: https://www.fastly.com/documentation/reference/changes/2025/02/ja4-fingerprinting-now-supported-in-bot-management/
[fastly-blog-unified]: https://www.fastly.com/blog/stronger-security-with-a-unified-cdn-and-waf
[fastly-client-challenges]: https://www.fastly.com/documentation/guides/next-gen-waf/using-ngwaf/client-challenges/about-client-challenges/
[fastly-rules]: https://www.fastly.com/documentation/guides/next-gen-waf/using-ngwaf/rules/about-rules/
[fastly-rules-conditions]: https://www.fastly.com/documentation/guides/next-gen-waf/using-ngwaf/rules/defining-rule-conditions
[fastly-compute-tutorial]: https://www.fastly.com/documentation/solutions/tutorials/next-gen-waf-compute/
[fastly-headers]: https://developer.fastly.com/reference/http/http-headers/
[fastly-announcements]: https://www.fastly.com/documentation/guides/next-gen-waf/whats-new/announcements/
[http-dev-xfrid]: https://http.dev/x-fastly-request-id
[trustradius-vs-cf]: https://www.trustradius.com/compare-products/cloudflare-vs-fastly-next-gen-waf
[sigsci-demo]: https://github.com/purpleax/sigsci-demo
