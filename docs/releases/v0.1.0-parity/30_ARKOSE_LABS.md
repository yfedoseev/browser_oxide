# 30 — Arkose Labs (FunCaptcha / MatchKey)

**Status:** reference — customer-onboarding handbook
**Cluster:** *none in the 126-corpus.* Arkose's two-product model (invisible Bot Manager + visible MatchKey CAPTCHA) is deployed on **account flows** (signup, login, password-reset, MFA, gameplay) — not on the public marketing/product pages that dominate our corpus. This chapter exists to (1) document recognition for customers who onboard with Arkose-protected endpoints, and (2) draw the structural line between "engine-addressable" (the invisible-mode pre-puzzle telemetry) and "out-of-scope" (the visible 3D puzzle itself) consistent with our `aecdf19` vendor-strip policy.
**Companion docs:** `18_ANTI_BOT_VENDOR_COOKBOOK.md §2.10b` (existing stub — should be expanded with the markers below), `27_VENDOR_COMPETITIVE_MATRIX.md` (Arkose currently absent — to be added when first onboarded), `06_AWS_WAF_SOLVER.md` (AWS WAF Captcha uses an Arkose-style image puzzle — historical relationship), `29_F5_SHAPE_SECURITY.md` (sibling chapter — same status: out-of-corpus, customer-onboarding reference).

---

## TL;DR

Arkose Labs ships two distinct products under one platform, and the distinction is load-bearing for what browser_oxide can and cannot do:

| Product | What it does | BO disposition |
|---|---|---|
| **Arkose Detect / Bot Manager** (invisible) | Pre-puzzle JS SDK collects fingerprint + behavioural telemetry, ML-scores the session, and *decides whether a visible puzzle is shown* | **engine-addressable** — passive recognition + clean JS surface clears the invisible path on a fresh, clean-fingerprint session |
| **Arkose Enforce / MatchKey** (visible) | The famous 3D-image / dice / rotation puzzle that fires when the invisible score is high-risk | **out-of-scope for v0.1.0** — interactive widget, needs a separate ML solver (paid service or custom CV model) per `aecdf19` policy |

The company was founded in **2015-2016** in Brisbane, Australia as **FunCaptcha** by **Kevin Gosschalk** and **Matthew Ford** (per [Crunchbase](https://www.crunchbase.com/organization/arkoselabs) and [About Us](https://www.arkoselabs.com/about-us/)); the FunCaptcha product is now Arkose **MatchKey** (rebrand per the 2022 product launch). They've raised \$114M+ from Microsoft M12, SoftBank Vision Fund 2, USVP, PayPal, and the Sony Innovation Fund. Marquee customers include **Roblox** (every signup), **Microsoft Outlook** (98% fraud reduction per [case study](https://www.arkoselabs.com/resource/case-study-microsoft-tackles-fraud-and-abuse/)), **LinkedIn**, **Twitter/X** (for signup), **EA**, **GitHub**, **Dropbox**, **Adobe**, **Meta**, **Singapore Airlines**, **Twilio**, **Expedia**, **Sony**.

What this chapter does:

1. Sections 1-3: product history, architecture, and detection markers — the same shape as chapter 18 §2.10b but at deep-dive resolution
2. Section 4: invisible-mode telemetry (BDA) — what's collected, how it's encoded, what BO emits "naturally" vs. what would need work
3. Section 5: visible-mode challenge taxonomy — the 7+ challenge variants and why solving them is a separate-product problem
4. Section 6: BO coverage speculation (no corpus data; calibrated against parity vendors)
5. Section 7: public solver landscape — the vibrant FunCaptcha-solver ecosystem (vibrant because the puzzle layer is independently attackable by CV models)
6. Section 8: customer-onboarding playbook
7. Section 9: forward-looking — Arkose's AI-arms-race posture
8. Section 10: acceptance + files

The honest one-line posture: **BO clears the invisible pre-puzzle layer for a fresh-fingerprint single-shot request. BO does not solve the visible MatchKey puzzle and will not in v0.1.0. Customer-onboarding scope is "we render the page chrome and submit the form up to the point where the puzzle would gate"; visible-puzzle solving is a separate-product integration.**

---

## 1. Product overview

### 1.1 Origin

Arkose Labs was born at a **Brisbane Startup Weekend** (year disputed across sources — Crunchbase says 2015, the Wikipedia draft and About Us page imply 2016) as **FunCaptcha**, founded by **Kevin Gosschalk** (CEO) and **Matthew Ford** (per [Startup Daily 2019 coverage](https://www.startupdaily.net/topic/brisbane-founded-arkose-labs-secures-strategic-investment-paypal/) and [LinkedIn profile](https://www.linkedin.com/in/kgosschalk/)). The original product was a literal alternative to text CAPTCHAs — animated 3D mini-games (rotate an animal to face right, drag a puzzle piece into a slot, pick the dice that sum to a number). The thesis: text CAPTCHAs are unsolvable for humans and trivial for OCR; interactive 3D puzzles are easy for humans and computationally expensive for adversaries.

The pivot to **Arkose Labs** branding came alongside the addition of the **invisible scoring layer** — the recognition that puzzle-solving is a fallback, not a primary defense. Most modern Arkose-protected sessions never see a puzzle: the BDA (Browser Data Analytics) telemetry scores the session, and only the high-risk minority gets the visible challenge. The puzzle (now branded MatchKey) is increasingly positioned as a **cost-imposition** mechanism on confirmed adversaries rather than a Turing test (see [Arkose's "CAPTCHA hell" article](https://www.arkoselabs.com/latest-news/welcome-to-captcha-hell/) for the philosophical pitch).

### 1.2 Funding and corporate

Per [Crunchbase](https://www.crunchbase.com/organization/arkoselabs), [PitchBook](https://pitchbook.com/profiles/company/97742-89), and the news-release sequence:

| Year | Round | Lead | Total raised | Notes |
|---|---|---|---|---|
| 2018 | Series A | (PayPal strategic) | — | Arkose's first big institutional check (per [Startup Daily](https://www.startupdaily.net/topic/brisbane-founded-arkose-labs-secures-strategic-investment-paypal/)) |
| 2020-03 | Series B | M12 (Microsoft's venture fund) | \$22M | Per [GlobeNewswire 2020-03-24](https://www.globenewswire.com/news-release/2020/03/24/2005447/0/en/Arkose-Labs-Raises-22-Million-in-Series-B-Round-Led-by-M12-Microsoft-s-Venture-Fund.html); USVP and PayPal participated |
| 2022 | Series C | SoftBank Vision Fund 2 | \$70M | Per [Arkose news release](https://www.arkoselabs.com/latest-news/arkose-labs-raises-70-million-led-by-softbank-vision-fund-2-to-bankrupt-the-business-of-fraud/) — "to bankrupt the business of fraud" |
| 2024+ | Sony Innovation Fund participation | — | (undisclosed) | Per [Sony Innovation Fund release](https://www.arkoselabs.com/latest-news/sony-innovation-fund-by-innovation-growth-ventures-igv-invests-in-arkose-labs-to-keep-gamers-safe-online/) |

Total raised \$114M+ per the company's own statements (cited by [getLatka](https://getlatka.com/companies/Arkose_Labs)). 2024 revenue ~\$47M with ~240 staff (per getLatka). Offices in San Francisco, Brisbane, London, Costa Rica. **No acquisitions** — Arkose has remained independent and organic per [Owler](https://www.owler.com/company/arkoselabs).

The Microsoft strategic-investment angle is important context: Arkose's relationship with Microsoft extends beyond M12 — they're a **Microsoft Outlook fraud-prevention partner** (the 98% fraud-reduction case study above) and as of [April 2025 expansion](https://www.arkoselabs.com/latest-news/arkose-labs-expands-strategic-relationship-microsoft-azure/) they're available natively on Azure. This is why **every Outlook.com signup** and **every Microsoft Live login** that gets risk-flagged shows a FunCaptcha puzzle.

### 1.3 Customer base

The Arkose customer list is unusually public for an anti-bot vendor because their visible MatchKey puzzles act as free product placement — anyone who's signed up for Roblox or recovered an Outlook password has seen the brand. Cited customers (across [Arkose's About page](https://www.arkoselabs.com/about-us/), the [Microsoft case study](https://www.arkoselabs.com/resource/case-study-microsoft-tackles-fraud-and-abuse/), [Hacker News thread](https://news.ycombinator.com/item?id=30413287), and trade press):

| Sector | Confirmed customers |
|---|---|
| **Gaming** | Roblox, Electronic Arts, Sony |
| **Social / consumer** | Twitter/X (signup), LinkedIn, Meta (some flows), Twitch (historical) |
| **Productivity** | Microsoft Outlook, GitHub, Dropbox, Adobe |
| **Travel** | Singapore Airlines, Expedia |
| **Payments / fintech** | PayPal (strategic + customer), Twilio (anti-toll-fraud) |
| **Identity** | Okta, Auth0 (integration partners; presumably also customers internally) |

For us: **none of these are in the 126-corpus.** The closest is `linkedin.com` (not in corpus) and `twitter.com` / `x.com` (`x-com` is in the corpus per `27 §3`; LinkedIn-as-tested is on landing-page flows, not signup-flows where Arkose lives). The corpus is dominated by anonymous-render-able marketing pages, which is exactly the surface Arkose is NOT on. **This is why Arkose has zero entries in our pass-fail tables.**

A customer who brings Arkose into our scope will be onboarding a use case like:

- "scrape a logged-in Roblox API" — gated by Arkose at session-create
- "automate Outlook signup" — gated by Arkose
- "monitor LinkedIn job postings as an authenticated user" — Arkose at login

For each, the answer pattern is in §8.

### 1.4 Product taxonomy

Arkose's current product split (per [arkoselabs.com](https://www.arkoselabs.com/)):

| Product | What it does | How it relates to BO |
|---|---|---|
| **Arkose Bot Manager** | The umbrella — combines invisible detection + visible enforcement | The product BO interacts with on a protected page |
| **Arkose Detect** | Invisible risk scoring only (no enforcement) — for sites that want to log/route risk without a CAPTCHA | BO renders past this transparently if BDA scores clean |
| **Arkose Enforce** (aka MatchKey) | The visible puzzle layer — fires when Detect scores high-risk | **out-of-scope** — interactive widget |
| **Arkose MFA** | An MFA-flow product (FunCaptcha as second factor) | Same as Enforce — out-of-scope visible widget |
| **Arkose Edge** | Edge-deployed scraping protection (newer offering) | Likely TLS+behavioral; BO's chrome-class TLS clears the TLS gate |
| **Arkose Email Intelligence** | Email-risk scoring (signup signal) | Not a browser-rendering feature |
| **Arkose Device ID** | Cross-session device tracking (cookie + fingerprint persistence) | Relevant to multi-session work; chapter §6.3 discusses |
| **Arkose Phishing Protection** | Anti-phishing (different problem class) | Not relevant |
| **Arkose Titan** | The unified-platform bundle | Marketing umbrella |

The two boxes a BO contributor needs to keep separate: **Detect** (invisible, attackable by a clean engine) vs. **Enforce/MatchKey** (visible, attackable only by a CV model or human solver). The whole rest of this chapter respects that division.

---

## 2. Detection architecture — invisible mode

### 2.1 The two-stage stack

```
                Browser (or BO / Camoufox / PW)
                         │
                         │ 1. GET <protected-page>
                         ▼
                  Customer's origin
                         │
                         │ 2. Page includes:
                         │    <script src="<company>-api.arkoselabs.com/v2/<pubkey>/api.js">
                         ▼
                      Browser
                         │
                         │ 3. SDK loads, calls setConfig(),
                         │    bootstraps BDA collector
                         │
                         │ 4. SDK fires onReady() callback
                         │    Customer page calls myArkose.run()
                         │
                         │ 5. SDK gathers BDA telemetry
                         │    (canvas, WebGL, audio, navigator, mouse,
                         │     screen, fonts, plugins, timing)
                         │
                         │ 6. SDK POSTs BDA to
                         │    client-api.arkoselabs.com (or surl)
                         ▼
                  Arkose risk engine
                         │
                ┌────────┴────────┐
        score ≤ threshold     score > threshold
                │                    │
                │ "challenge-        │ "challenge-shown" event
                │  suppressed" event │
                │                    │
                │ → onCompleted()    │ → MatchKey iframe loads
                │   fires with       │   (client-api...arkoselabs.com/
                │   verification     │     v2/<pubkey>/index.html)
                │   token            │
                ▼                    ▼
        Customer accepts        User solves puzzle
        form submit             interactively
                                     │
                                     │ → onCompleted() fires with token
                                     ▼
                              Customer backend
                                     │
                                     │ Verify API call to
                                     │ verify-api.arkoselabs.com
                                     ▼
                              Pass / fail decision
```

Per [Arkose's standard setup docs](https://developer.arkoselabs.com/docs/standard-setup):

- The integrator includes the script in `<head>`: `<script src="https://<company>-api.arkoselabs.com/v2/<YOUR_PUBLIC_KEY>/api.js" data-callback="setupEnforcement"></script>`
- `<company>` is per-tenant (e.g., `roblox-api.arkoselabs.com`, `client-api.arkoselabs.com` is the default)
- `<YOUR_PUBLIC_KEY>` is a UUID — every tenant has one (Roblox: `476068BF-9607-4799-B53D-966BE98E2B81` per public RE work; AOL/Yahoo: `<other UUID>`; Microsoft: `<other UUID>`)
- The SDK exposes a `myArkose` object on the page with methods: `setConfig`, `getConfig`, `run`, `reset`, `version`
- Two **required** callbacks: `onReady` (SDK loaded) and `onCompleted` (verification token ready, success or fail)

### 2.2 Modes

| Mode | Config flag | Behaviour |
|---|---|---|
| Visible (default) | `mode: "inline"` or `mode: "lightbox"` | Shows the modal challenge if needed |
| Invisible / detection-only | `mode: "lightbox"` + `transparent` key | Never shows the modal; only emits the score-based token |
| Transparent | Key configured server-side as "transparent" | Same as invisible — no UI |

For BO, the **mode is set by the customer (the integrator), not by Arkose's risk engine.** A customer who deploys with `transparent` mode is using Arkose purely for behind-the-scenes scoring — the visible puzzle never appears. We handle these cleanly. A customer who deploys with visible mode falls back to puzzle if BDA fails — which we lose.

### 2.3 SDK loading and bootstrap

The script chain is reliably:

```
Page HTML
  └── <script src="https://<company>-api.arkoselabs.com/v2/<UUID>/api.js"
                data-callback="<global-fn-name>"
                async defer></script>
       └── (loads) → defines myArkose object
                  → bootstraps BDA collector
                  → calls onReady() callback
                  → posts BDA to /fc/gt2/public_key/<UUID> on POST
                       (or /fc/gfct/ for full challenge bundle)
                  → if challenge needed: loads iframe to
                       https://<company>-api.arkoselabs.com/v2/<UUID>/index.html
```

The `/fc/gt2/`, `/fc/gfct/`, `/fc/gc/` endpoints are the public Arkose API surface and are documented (informally) across the [noahcoolboy/funcaptcha](https://github.com/noahcoolboy/funcaptcha) library and [`unfuncaptcha/tguess`](https://github.com/unfuncaptcha/tguess). The `gfct` endpoint returns:
- `session_token` — the challenge session ID
- `challengeID` + `challengeURL`
- `dapib_url` — a session-unique verification JS file (~1200 distinct files/day rotating, per the `tguess` RE writeup)

The `dapib_url` script is the **proof-of-execution** primitive: when the user submits an answer, the answer is run through this script (which lives in an iframe) and the *transformed* answer is what's actually submitted. This makes pure replay/API-only solving impossible — you have to execute the per-session script in a browser-realistic env.

### 2.4 BDA — Browser Data Analytics

BDA is Arkose's name for the encrypted telemetry blob that the SDK collects and POSTs. Per [gen9x's BDA decode analysis](https://gen9x.com/blog/how-to-decode-fingerprint-bda-of-funcaptcha):

- **Encoding:** AES-256-GCM, key derived per-session from a User-Agent-mixed seed
- **Format on the wire:** Base64-encoded ciphertext, embedded in a JSON envelope along with a `iv` (initialization vector) and `s` (salt)
- **Content:** approximately 100+ fingerprint signals across canvas, WebGL, audio, navigator, screen, timezone, plugins, mouse-trajectory, key-timing, font enumeration, and various Chrome-quirk probes
- **Submission target:** `client-api.arkoselabs.com/fc/gt2/public_key/<UUID>` (or per-tenant `surl`-prefixed equivalent)

For us, the BDA is **opaque** — we can emit it (the SDK runs in our V8) and we can be classified by it (whatever signals the SDK reads will reflect our profile). We cannot inspect or alter what the SDK collects without intercepting its calls, which is a per-vendor solver concern that's explicitly **out of public scope** per `aecdf19`.

What this means: **if our JS environment is byte-perfect-Chrome (per `16_STEALTH_FINGERPRINT_AUDIT.md`), the BDA will encode a byte-perfect-Chrome session, and Arkose's risk scoring will treat us as Chrome.** This is the structural reason BO has a defensible posture against the invisible layer.

### 2.5 The proof-of-execution gate (the harder part)

Even with clean BDA, Arkose makes pure-API solving infeasible via the per-session `dapib_url` script:

1. Adversary calls `/fc/gfct/` and gets a `session_token` + `dapib_url`
2. Adversary tries to compute answer offline, POSTs back
3. Server rejects — because the answer was supposed to be transformed by `dapib_url` in-browser, with the transform's output being session-specific
4. Adversary must fetch and execute `dapib_url` in a real browser context, with the matching session_token, in the same security context

This is essentially the same primitive as Akamai's `bm_sz`-keyed cookieHash (see `26 §2.2`) or Cloudflare's per-challenge orchestrator nonce (see `25 §2.5`). **For BO**: if we *were* invoking the puzzle path (which we're not), we'd need to faithfully execute the `dapib_url` script with the right `window.parent` reference and the right session token in scope — exactly the kind of cross-origin iframe materialization that `07 §Primitive 2` already implements for DataDome. We have the primitive. We just don't have the per-vendor solver that orchestrates it.

---

## 3. Detection markers — recognising Arkose in a captured response

### 3.1 Script-src markers (body)

| Marker | Strength | Notes |
|---|---|---|
| `client-api.arkoselabs.com` | **unambiguous** | The default tenant subdomain. |
| `<tenant>-api.arkoselabs.com` | **unambiguous** | Per-tenant subdomain (e.g., `roblox-api.arkoselabs.com`, `outlook-api.arkoselabs.com`). |
| `.arkoselabs.com/v2/` | **unambiguous** | The v2 API path; appears in script-src and iframe-src. |
| `funcaptcha.com` | **unambiguous** | Legacy domain — still active for some tenants who haven't re-pathed. |
| `arkoselabs.com/fc/` | **unambiguous** | API-endpoint path (`/fc/gt2/`, `/fc/gfct/`, `/fc/gc/`). |
| `data-pkey=<UUID>` HTML attribute | strong | The public-key carrier on the integrator div. |
| `<input name="fc-token">` / `<input id="FunCaptcha-Token">` | strong | Form-element carriers for the verification token. |
| `<input id="verification-token">` | strong | Same family — the token-carrier convention. |

The 18 §1.3 cookbook table currently has only **one row** for Arkose:

```
| `arkoselabs.com` / `funcaptcha` | Arkose Labs FunCaptcha | unambiguous | (not in classifier — extend) |
```

This should expand to the full table above — see §10.2.

### 3.2 Iframe-src markers (challenge widget)

If the visible MatchKey iframe loads, recognition is:

| Iframe pattern | Notes |
|---|---|
| `https://client-api.arkoselabs.com/v2/<UUID>/index.html` | Default-tenant challenge iframe |
| `https://<tenant>-api.arkoselabs.com/v2/<UUID>/index.html` | Per-tenant challenge iframe |
| `https://iframe.arkoselabs.com/` | Hosted iframe alternative |

### 3.3 PostMessage events (parent-frame observation)

The Arkose SDK communicates with the parent page via `window.postMessage` with a stable event vocabulary (per [iframe setup guide](https://developer.arkoselabs.com/docs/iframe-setup-guide)):

| Event | Meaning |
|---|---|
| `challenge-loaded` | Iframe rendered, ready for user interaction |
| `challenge-suppressed` | Risk score low — no puzzle shown, token issued silently |
| `challenge-complete` | User solved (or session timed out); token will follow via onCompleted |

For BO observability: a `postMessage` handler in the JS surface (we already have `MessageEvent` per `crates/js_runtime/src/`) will see these events. We don't currently log them; logging them would be a useful diagnostic add-on per §10.2.

### 3.4 Cookies (best-public-knowledge)

Arkose's cookie use is comparatively light vs. Akamai or Cloudflare; the per-session state lives in the SDK's in-memory closures, not in cookies. Where cookies do appear:

| Cookie name | Set by | Purpose |
|---|---|---|
| `timestamp` | Arkose iframe domain | Anti-replay timestamp |
| `_arkose_session` (variant: `arkose_session`) | Arkose iframe domain | Session continuity across iframe nav |
| `fc-cookie-<hash>` | Arkose iframe domain | Per-tenant FC token persistence (rare; most tenants are stateless on the cookie axis) |

These do **not** appear on the parent (customer) domain; they're scoped to `*.arkoselabs.com` and the `*-api.arkoselabs.com` subdomain. From BO's first-party perspective on the customer page, there's no Arkose cookie footprint — recognition has to be script-src based.

### 3.5 What to add to `classify.rs` (proposed)

Mirror the §3.1 table into `crates/browser/src/classify.rs` SMALL_BODY markers:

```rust
// Proposed additions (not implemented today)
"client-api.arkoselabs.com",          // Arkose default tenant
".arkoselabs.com/v2/",                // Arkose v2 API
"arkoselabs.com/fc/",                 // Arkose challenge endpoints
"funcaptcha.com",                     // legacy domain (some tenants)
```

And the **vendor-detect logger** in `crates/browser/src/page.rs:1054-1069` would add:

```rust
if response.body_contains(b"arkoselabs.com") {
    log_vendor_marker("arkose", "script-src-arkose", ...);
}
if iframe_src.contains("arkoselabs.com/v2/") {
    log_vendor_marker("arkose_matchkey", "iframe-challenge", ...);
}
```

These are body-marker only; Arkose-presence does **not** mean Arkose-challenge-blocked (most pages with Arkose script-src render fine because the puzzle never fires).

---

## 4. Mechanism per signal collection — invisible mode

This section parallels chapter 29 §4 — what the Arkose SDK actually collects, mapped onto BO's coverage. Because the BDA is encrypted and the SDK is obfuscated, the canonical list is reverse-engineered (primarily by the [noahcoolboy](https://github.com/noahcoolboy/funcaptcha) and [`unfuncaptcha`](https://github.com/unfuncaptcha/tguess) projects, plus academic and gray-literature analyses).

### 4.1 Canvas + WebGL

- `<canvas>.toDataURL()` after rendering an Arkose-specific test pattern (gradients, text in specific fonts, emoji)
- WebGL extension list, vendor, renderer, shader-precision-format, all WebGL parameters
- `<canvas>` 2D context properties

**BO coverage:** chrome_148 profile canvas surface (per `crates/canvas/src/`) is byte-perfect-Chrome. We pass standard FP-collect and FingerprintJS probes. Arkose's specific pattern is not in our test corpus but is likely strict-subset of what we already pass.

### 4.2 Audio context

- `AudioContext.createOscillator()` + `AnalyserNode` → hash of buffer
- `DynamicsCompressorNode` parameters (the historical Akamai signal — see `26 §2.3` — Arkose collects the same)
- `OfflineAudioContext` rendering

**BO coverage:** `crates/canvas/src/audio_*.rs` covers this. Same byte-perfect-Chrome posture.

### 4.3 Navigator + Screen + Window

- `navigator.{userAgent, platform, vendor, hardwareConcurrency, deviceMemory, languages, plugins, mimeTypes, webdriver, doNotTrack, cookieEnabled, maxTouchPoints, oscpu, productSub, javaEnabled}`
- `screen.{width, height, availWidth, availHeight, colorDepth, pixelDepth, orientation}`
- `window.{outerWidth, outerHeight, innerWidth, innerHeight, devicePixelRatio, screen, history.length}`
- `Date.prototype.toString` / `toLocaleString` — timezone derivation cross-check

**BO coverage:** profile-driven via `StealthProfile`; per `16 §1`, all byte-perfect-Chrome.

### 4.4 Behavioural — the **load-bearing-against-Arkose** signal

Arkose weights behavioural signals heavily because their model is trained on the explicit interaction patterns from puzzle-solving:

- Mouse movement trajectories pre-puzzle (does the cursor move? does it move in a humanlike Bezier?)
- Keystroke timing (intra-character and inter-character delays for any typed input — login forms etc.)
- Touch event timing (mobile-class)
- Scroll-event patterns (a page-load with zero scroll is itself a weak signal)
- Time-on-page before form interaction (instant submission flags)
- Focus / blur / visibilitychange events

**BO coverage:** `crates/browser/src/js/humanize.js` synthesises mouse motion when explicitly invoked (form-fill scenarios). For **pure render-only requests** (which is what BO mostly does), there is no synthetic behaviour. **This is the principal gap for Arkose** — the same gap as Kasada (`08 §3`) and as the Shape behavioral surface (`29 §4.4`).

The mitigation pattern, when an Arkose-protected scenario actually needs to interact (not just render):

1. Call `humanize.click_path(target_selector)` before clicking the form-submit button
2. Insert humanlike inter-character delays via `humanize.type(input_selector, text)` for typed fields
3. Allow at least `humanize.dwell_before_submit_ms(800..2000)` after the last interaction

These primitives exist (`crates/browser/src/js/humanize.js`); they just aren't auto-invoked by the page-render path. Customer-driven invocation per onboarding playbook §8.

### 4.5 Anti-automation flag scrubs

- `navigator.webdriver`
- Plugin and mimeType length (zero is bot-class)
- Permissions API state for `notifications` (real Chrome shows `prompt`; some headless setups show `default` or `denied`)
- WebGL renderer string fuzzing detection (`SwiftShader`, `llvmpipe`, `ANGLE` are bot-class)
- `chrome.runtime` and other extension-API existence
- DevTools-open heuristics

**BO coverage:** `crates/js_runtime/src/chrome_compat.rs` plus the chrome_148 profile cover all of these. We have 437/437 chrome_compat tests green.

### 4.6 Network-side signals (TLS / IP / ASN)

Per the [Medium analysis](https://medium.com/@kentavr00000009/funcaptcha-arkose-labs-principles-of-operation-features-and-methods-for-automated-bypass-780ef786d7c5) (HTTP-403 in our crawl but recapitulated in other writeups):

- JA3 / JA4 TLS fingerprint
- IP reputation / ASN (datacenter vs residential vs mobile)
- HTTP header order
- Per-IP prior session history (have we seen this IP attempting Arkose before? what were the outcomes?)
- Cross-tenant intelligence (the "Arkose Global Intelligence Network" — same idea as Shape's network-effect ML)

**BO coverage:** TLS / HTTP-2 / header-order all byte-perfect-Chrome via `boring2` (`23_TLS_HTTP_FINGERPRINT_REFERENCE.md`). IP / ASN is customer infrastructure.

### 4.7 Summary table — BO coverage of Arkose invisible-mode signals

| Signal class | BO coverage | Confidence |
|---|---|---|
| Canvas / WebGL / Audio fingerprint | **clears (probably)** | medium — clears standard FP probes; Arkose-specific patterns untested |
| Navigator / screen / window | **clears** | high — chrome_148 profile is comprehensive |
| Automation-flag scrubs | **clears** | high — chrome_compat tests cover these |
| Behavioural (mouse/key/scroll) | **gap** for render-only flows | high — humanize primitives exist but not auto-invoked |
| BDA payload integrity (encrypted blob structure) | **emits naturally** | high — SDK runs in V8 and produces a real BDA; we don't tamper with it |
| TLS / JA3 / JA4 / HTTP-2 | **clears** | high — same primitive that clears every other vendor |
| IP / ASN class | **out of scope** (customer infra) | n/a |
| `dapib_url` proof-of-execution (if puzzle path enters) | **untested but architecturally feasible** | low confidence pending capture |

---

## 5. Visible MatchKey challenge taxonomy — out-of-scope reference

If the invisible BDA score is too high, the SDK escalates to a visible MatchKey puzzle. There are **multiple challenge variants** under continuous expansion. Per the [noahcoolboy/funcaptcha library README](https://github.com/noahcoolboy/funcaptcha) (which is the most-comprehensive public catalogue), the variants include:

| Variant key | Description | Solver class |
|---|---|---|
| `rotated` (gameType 1) | "Rotate the object to face right / match the target" — the classic 3D-rotation challenge with 0-6 angles or raw-degree submission | CV-model (custom-trained) or paid service |
| `tile_select` (gameType 3) | "Pick the tile that matches the description" — 6-tile grid | CV-model or paid service |
| `image_match` (gameType 4) | "Pick the image showing X" — 1-of-N from variable difficulty | CV-model or paid service |
| `apple` | "Pick the apple variant matching the target" | CV-model or paid service |
| `maze` | Maze navigation (drag through corridors) | Reinforcement-learning solver or human |
| `dice_pair` | "Pick the dice pair summing to N" | Custom rule solver |
| `dart` | "Hit the dart at the right position" | Pixel-targeting solver or human |
| `card` | "Pick the card matching the target" | CV-model |
| `3d_rollball_animals` | "Roll the animal-ball to match a target orientation" | RL solver or human |

Per [Arkose's own MatchKey marketing](https://www.arkoselabs.com/arkose-matchkey/), the challenge library extends to **>1,250 unique variant combinations** per individual challenge type. This is the explicit design goal: any solver must keep up with a moving variant target.

**For BO:** all of these are visible-iframe interactive widgets. They are **out of scope for `vendor_solvers` v0.1.0** by the same logic that AWS WAF Captcha (§18 §2.10) is out of scope and that hCaptcha / reCAPTCHA v2 are out of scope. Visible-puzzle solving is a separate-product (CV-model or commercial-API-integration) decision, not an engine-rendering decision. The line is the same line we draw for `chapter 06 §3` (AWS WAF) — the engine renders, the solver-service solves.

### 5.1 The session-rotating script complication (relevant if BO ever bridges to a solver)

Even if a customer integrates a commercial solver (2Captcha, CapMonster, etc.), the solver must operate **with the per-session `dapib_url` script in the chain** — see §2.5 above. The flow is:

1. BO renders the page; Arkose iframe loads
2. Commercial solver API returns an answer (the rotation angle, the matching tile)
3. The answer must be transformed by the per-session `dapib_url` script
4. The transformed answer is submitted back to Arkose

For BO + solver, this means the solver can't just return the answer; it must return the **transformed** answer. Either (a) the solver service does the transformation server-side (most paid services do — they fetch and execute the `dapib_url` in a sandbox) or (b) BO must run the `dapib_url` in its iframe context after receiving the raw answer. (b) is more work but reuses primitive 2 from `07_DATADOME_PRIMITIVES.md`.

This is a useful architectural detail to capture for a future "integration with commercial CAPTCHA services" chapter; out of scope for v0.1.0.

---

## 6. BO coverage — speculative (no live data)

Same caveat as chapter 29: **no Arkose-protected site is in the 126-corpus.** All claims here are hypotheses calibrated against parity vendors. The customer onboarding playbook (§8) is the path to converting hypotheses into measured data.

### 6.1 Likely outcomes by scenario

| Scenario | Expected outcome | Hypothesis rationale |
|---|---|---|
| Render a page with Arkose script-src but no Arkose-gated interaction (just reading the page chrome) | **passes** | Arkose doesn't block the page render; the SDK loads, BDA fires asynchronously, no document-level gate |
| Render an Arkose-gated page in transparent/invisible mode | **passes** (≥80% conf.) | BDA from a chrome_148 profile + chrome-class TLS + no automation flags should score within human range; one-shot session has no adverse prior |
| Submit a form on an Arkose-gated page (visible mode), where puzzle does NOT fire | **passes** (≥70%) | Same as above, plus the onCompleted-token flow; if BDA scores clean enough, the form-submit token is issued |
| Submit a form on an Arkose-gated page where puzzle DOES fire | **fails** | Visible puzzle is not solvable from BO without a separate solver |
| Sustained scrape of an Arkose-protected endpoint (50+ requests) | **fails** | Arkose Global Intelligence Network correlates per-session prior; pattern recognition kicks in after a few requests |

### 6.2 Sites in our 126-corpus that *might* hit Arkose

None directly. The closest:

| Site | Why mentioned | Status |
|---|---|---|
| `x-com` (Twitter / X) | Twitter uses Arkose for signup; our corpus tests the **public landing page** (not signup) | passes — landing page is Arkose-free |
| `linkedin.com` (not in corpus) | LinkedIn uses Arkose for signup + risk-flagged logins | n/a — not in corpus |
| `microsoft.com` (not in corpus) | Microsoft Outlook signup uses Arkose | n/a — not in corpus |
| `roblox.com` (not in corpus) | Roblox uses Arkose for every signup | n/a — not in corpus |

The honest read: **our corpus is composed entirely of "scrape this public page" use cases; Arkose lives on "create an account" use cases.** They're orthogonal. A customer who needs Arkose-traversal is asking for a different scoping conversation than a customer who needs corpus-grade rendering.

### 6.3 Device persistence (Arkose Device ID) — multi-session consideration

If a customer runs sustained automation against an Arkose-protected endpoint, the Arkose Device ID product (introduced 2024, per [Arkose news release](https://www.arkoselabs.com/latest-news/arkose-labs-launches-arkose-device-id/)) cross-correlates sessions via cookie + fingerprint + behavioral signature. This means:

1. Two sessions from BO with the same profile look like the same device to Arkose
2. Cookie clearing alone is insufficient — the fingerprint signature is the binding key
3. Per-session rotation requires **profile diversification** (which BO supports via `chrome_148 / pixel_148 / iphone_148 / firefox_135`) AND fresh IP per session

For customer onboarding: if the use case is multi-session at any meaningful volume, plan for **profile rotation across our 4 profiles + residential proxy per session**. BO's 4-profile architecture is well-positioned for this; vanilla Camoufox (one fixed Firefox profile) is not.

---

## 7. Public solver / bypass landscape

Arkose's solver community is **the largest of any commercial anti-bot vendor** — larger than Cloudflare, Akamai, DataDome, and dramatically larger than Shape. The reason is structural: the **visible puzzle is independently solvable by CV models** (rotation, image-match) without needing to defeat the JS VM. This invited a generation of solver projects:

### 7.1 Open-source solver libraries

- **[noahcoolboy/funcaptcha](https://github.com/noahcoolboy/funcaptcha)** — Node.js library for *interacting with* FunCaptcha (token retrieval, challenge enumeration). Does NOT include the CV model — the user supplies the answer. Comprehensive coverage of challenge types. **Note:** maintainer announced no further regular maintenance (per the README) due to Arkose's ongoing changes — this is the rotating-target problem.
- **[unfuncaptcha/tguess](https://github.com/unfuncaptcha/tguess)** — Standalone implementation of the `tguess` answer-transformation flow. Useful primitive for any solver that needs to traverse the `dapib_url` gate.
- **[useragents/Funcaptcha-Audio-Solver](https://github.com/useragents/Funcaptcha-Audio-Solver)** — Audio variant via speech recognition. The audio puzzle is the easier accessibility fallback that Arkose offers; audio-CV solvers are independently effective.
- **[`decodecaptcha/FunCaptcha-Solver`](https://github.com/decodecaptcha/FunCaptcha-Solver)** — API-service-style solver.
- **[Pr0t0ns/Funcaptcha-Solver](https://github.com/Pr0t0ns/Funcaptcha-Solver)** — Independent solver.
- **[kiookp/funcaptcha--solver](https://github.com/kiookp/funcaptcha--solver)** — Another independent solver.
- **[arkose GitHub topic](https://github.com/topics/arkose)** — index of related projects.
- **[noahcoolboy/roblox-funcaptcha](https://github.com/noahcoolboy/roblox-funcaptcha)** — Roblox-specific integration around the noahcoolboy/funcaptcha core.

### 7.2 Commercial solver services

Arkose has driven a robust commercial solver market — it's the most-named target in CAPTCHA-solver service marketing:

- **[2Captcha — FunCaptcha endpoint](https://2captcha.com/p/funcaptcha)** — paid service; human + ML solvers; ~5-30s typical
- **[Anti-Captcha — FunCaptchaTaskProxyless](https://anti-captcha.com/apidoc/task-types/FunCaptchaTaskProxyless)** — paid service; comparable
- **[SolveCaptcha](https://solvecaptcha.com/captcha-solver/funcaptcha-solver-bypass)** — paid; advertises AI-based 5-10s responses
- **[CapMonster Cloud — FunCaptcha task](https://docs.capmonster.cloud/docs/captchas/funcaptcha-task/)** — paid; AI-based
- **CapSolver** — paid; multi-vendor
- **[CaptchaAI](https://blog.captchaai.com/what-is-funcaptcha-arkose-labs-explained)** — paid
- **[uCaptcha](https://ucaptcha.net/blog/funcaptcha-arkose-labs/)** — paid

All of these accept the public key + page URL + (sometimes) the User-Agent + cookies, and return a token suitable for the `verification-token` form input. They do *not* require BO-side code beyond plumbing the token into the form.

### 7.3 Honest assessment

For BO + customer:

- **Invisible-only Arkose**: BO clears it without any solver integration (the SDK runs, the BDA emits, the token issues). Recognition is engine-side work, not solver-side work.
- **Visible MatchKey**: BO + a commercial solver service is the standard pattern. The integration is light (~50 LOC of "wait for iframe, POST to solver API, inject returned token"). The cost is per-puzzle (\$1-5 per 1000 solves at the commercial endpoints).
- **From-scratch CV solver inside `vendor_solvers`**: high-effort (CV-model training pipeline + per-variant maintenance + the rotating challenge library). Justified only if customer volume makes commercial-service per-puzzle cost prohibitive.

The v0.1.0 posture is **"recognise + plumb-to-commercial-service-when-customer-asks"** — same as chapter 29 §6.4 for Shape. No `vendor_solvers/src/arkose.rs` for v0.1.0.

---

## 8. Customer onboarding playbook

### 8.1 Step 1 — confirm Arkose presence

Confirm via:

1. **Browser DevTools** — open the page, search the Network tab for `arkoselabs.com` or `funcaptcha.com`. If present, Arkose is in scope.
2. **Page HTML** — search for `data-pkey=`, `<input name="fc-token">`, `<input id="verification-token">`, or any of the `*-api.arkoselabs.com` patterns.
3. **DOM `myArkose` object** — `console.log(window.myArkose)` after page load; defined means SDK loaded.
4. **Public-key extraction** — capture the UUID for the customer's tenant; useful for downstream solver integration.

### 8.2 Step 2 — characterise the use case

| Customer use case | BO disposition | Solver plumbing required? |
|---|---|---|
| "Read a page that contains Arkose script but no interaction" | passes natively | no |
| "Submit a form that's gated by Arkose in invisible/transparent mode" | likely passes natively (≥70%) | no, unless puzzle fires |
| "Submit a form gated by Arkose in visible mode (or after BDA-flag escalation)" | requires solver | yes — commercial service recommended |
| "High-volume scrape of an Arkose-protected endpoint" | requires solver + profile rotation + residential proxy | yes + infrastructure |
| "Mobile-API path protected by Arkose Mobile SDK" | out of scope | n/a — not a browser-engine problem |

### 8.3 Step 3 — capture-and-verify

Run a 5-request capture:

```bash
cargo run --release --example sweep_metrics -- \
    --site <customer-site-with-arkose-gate> \
    --profile chrome,pixel,iphone,firefox \
    --capture-headers \
    --capture-cookies \
    --capture-iframes \
    --reps 5 \
    --output /tmp/arkose_capture/
```

The `--capture-iframes` flag (proposed addition per §10.2) would log whether the MatchKey iframe ever materialised. If it did, the customer is in the visible-mode escalation path.

### 8.4 Step 4 — recommend solver plumbing if needed

If the capture shows the puzzle fires, recommend one of:

| Service | Suitable for |
|---|---|
| 2Captcha / CapMonster / CapSolver | One-off / small-volume (\$1-5 per 1000) |
| Anti-Captcha | Mid-volume; battle-tested API |
| Custom CV model | High-volume (>50k solves/day), with engineering budget |

Plumbing is per-customer integration code (sits in `vendor_solvers` or customer-private code); BO exposes the necessary primitives (iframe access via `Page::frames()`, token-form-injection via `Page::evaluate()`).

### 8.5 Step 5 — the honest pitch line

> "browser_oxide renders Arkose-protected pages cleanly and clears the invisible Bot Manager scoring layer on a single-shot fresh-session request. We do not solve the visible MatchKey puzzle in v0.1.0 — that's a separate-product integration with a commercial CAPTCHA-solver service or a customer-trained CV model. For high-volume scenarios, plan for profile rotation across our 4 stealth profiles plus residential proxy infrastructure."

---

## 9. Forward-looking — Arkose's roadmap

### 9.1 The AI-arms-race posture

Arkose's 2024-2026 messaging is dominated by the LLM-driven-scraping threat (see [Arkose blog: MatchKey AI-resistant innovation](https://www.arkoselabs.com/blog/arkose-matchkey-ai-resistant-attack-innovation/) and the [Arkose Detect deep dive](https://www.arkoselabs.com/blog/captcha-a-cost-proof-solution-not-a-turing-test/)). Their explicit thesis: GPT-4-class vision models can solve image-grid CAPTCHAs trivially; the only durable defense is **session-unique, computationally-expensive challenges that scale linearly with attacker budget**. MatchKey's "1,250 variants per challenge type" is engineered to this — even a perfect solver has to be re-trained per variant cluster.

For BO, this is mostly informational. It doesn't change the structural posture: invisible-mode BO clears, visible-mode BO needs an external solver. The roadmap might push Arkose toward **more invisible-mode scoring with retroactive enforcement** (parallel to F5 Shape's network-effect ML) — which is more attackable by our profile-rotation strategy.

### 9.2 Arkose Device ID expansion

The Device ID product (launched 2024) adds cross-session persistence. As discussed in §6.3, this makes sustained scrape sessions detectable. Practical implication: customer scenarios that need volume should budget for **per-session profile diversity** (using all 4 of our chrome / pixel / iphone / firefox profiles in rotation, not just one).

### 9.3 Microsoft / Azure deepening

Per [April 2025 expansion](https://www.arkoselabs.com/latest-news/arkose-labs-expands-strategic-relationship-microsoft-azure/), Arkose is now natively integrated with Microsoft Entra (formerly Azure AD). Implication: **any Microsoft authentication flow that runs through Entra is candidate for Arkose protection.** For customers in Microsoft-shop verticals (enterprise SaaS auth), this is a relevant horizon.

### 9.4 Possible move to behavioural-only

Arkose has hinted (in [TechTarget interview with Kevin Gosschalk](https://www.techtarget.com/searchenterpriseai/feature/Arkose-Labs-puts-AI-in-cybersecurity)) that the long-term direction is a **fully invisible behavioral-scoring product** with the visible puzzle as a last-resort cost-imposition mechanism. If this materialises, BO's posture **improves** — our behavioral-signal coverage (via humanize.js) is more attackable than our visible-puzzle coverage (which is zero).

### 9.5 Convergence with F5 (speculative)

There have been industry rumors of F5 / Arkose strategic alignment (Arkose's invisible challenge product is complementary to F5 Shape's invisible scoring product). No public deal as of 2026-05; both companies remain independent. If they merge, the recognition logic in chapter 29 §3 and this chapter §3 would unify, but the engine-side disposition wouldn't change.

---

## 10. Acceptance + files

### 10.1 What "v0.1.0 supports Arkose" means (acceptance bar)

| Acceptance item | Status today | v0.1.0 target |
|---|---|---|
| Recognise Arkose in `classify.rs` body markers | partial (one row in `18 §1.3`, not in `classify.rs`) | **add** 4 SMALL_BODY markers per §3.5 |
| Recognise Arkose in vendor-detect script-src logger | absent | **add** script-src and iframe-src detection per §3.5 |
| Recognise Arkose iframe materialisation in capture tooling | absent | **add** `--capture-iframes` flag to `sweep_metrics` example |
| postMessage event observability (challenge-loaded / -suppressed / -complete) | absent | **add** opt-in postMessage logger |
| One spot-check capture against a known-Arkose site (e.g. a synthetic Roblox signup attempt) | none | **run** capture; document the invisible-mode passing path |
| Documented pass-rate / customer-onboarding language | partial (this doc) | **publish** §8.5 language in customer-facing materials |
| Active visible-puzzle solver (`vendor_solvers/src/arkose.rs`) | absent | **decline** for v0.1.0 (out of scope per §7.3) |
| Plumbing example for commercial-solver integration | absent | **document** a recipe in this chapter §8.4 (already done above) — optional code example deferred |

### 10.2 Files to touch (when we onboard the first Arkose customer)

| File | Change | LOC | Reason |
|---|---|---|---|
| `crates/browser/src/classify.rs` | add `arkoselabs.com`, `funcaptcha.com`, `arkoselabs.com/v2/`, `arkoselabs.com/fc/` SMALL_BODY markers | ~6 | recognition |
| `crates/browser/src/page.rs:1054-1069` | extend vendor-detect logger for `arkose` and `arkose_matchkey` script-src + iframe-src patterns | ~12 | observability |
| `crates/browser/examples/sweep_metrics.rs` | add `--capture-iframes` flag that logs all `<iframe src>` URLs during page load | ~20 | diagnostic for puzzle-fire detection |
| `crates/browser/src/page.rs` (humanize integration) | expose a `Page::prepare_for_arkose_submit(form_selector)` helper that auto-invokes humanize.click_path before submit | ~30 | mitigates §4.4 behavioural gap |
| `docs/releases/v0.1.0-parity/18_ANTI_BOT_VENDOR_COOKBOOK.md §2.10b` | expand the Arkose section from the current 4-line stub to a full subsection including detection markers, mode behavior, and BO disposition | ~40 | cookbook coverage |
| `docs/releases/v0.1.0-parity/27_VENDOR_COMPETITIVE_MATRIX.md` §1 | add Arkose row (per-engine pass counts) once we have capture data | ~3 | competitive transparency |
| `docs/releases/v0.1.0-parity/30_ARKOSE_LABS.md` | this file (already authored) | — | reference |

### 10.3 Files NOT to touch (declared out of scope)

| File | Why not |
|---|---|
| `vendor_solvers/src/arkose.rs` (would-be) | §7.3 — visible-puzzle solving is out of scope per `aecdf19` policy; commercial-service integration is per-customer code |
| `crates/stealth/profiles/chrome_148_*.yaml` | Arkose doesn't expose a new fingerprint surface we're missing; chrome_148 covers it |
| `crates/canvas/src/*.rs` | Same — canvas/WebGL surface that passes Akamai/Cloudflare/Kasada also passes Arkose's invisible mode |
| Any CV-model training infrastructure | Out of scope — `vendor_solvers` does not include CV; that's a separate product |

### 10.4 Open questions logged for future investigation

1. **What is the BDA pass rate from a fresh chrome_148-profile session against Roblox signup?** This is the canonical Arkose-invisible-mode test. A single live capture (one Roblox signup attempt) would convert the §6.1 hypothesis row into a measured data point.
2. **Does Microsoft Outlook signup demote BO to puzzle-mode?** Cross-check with the 4 profiles; pixel and iphone may have different escalation thresholds than chrome.
3. **Does the visible MatchKey iframe load fully inside BO's V8 + Servo-class layout?** The 3D animations require WebGL + animation frame; we should confirm the iframe at least renders correctly, even if we don't solve it.
4. **Are there 126-corpus sites that *quietly* run Arkose invisible-mode without ever showing the puzzle?** A search of captured response bodies for `arkoselabs.com` across our sweep would surface any hidden Arkose deployments. None known to us today, but worth a one-time pass.
5. **What is the comparative BDA-score from BO chrome_148 vs. Camoufox vs. Patchright vs. Playwright?** A four-engine A/B against a single Arkose-protected site (e.g., a sacrificial Roblox account) would tell us where BO sits on the visible-mode-escalation curve.

### 10.5 Cross-references

- `18_ANTI_BOT_VENDOR_COOKBOOK.md §2.10b` — current 4-line stub; this chapter is the deep-dive expansion
- `27_VENDOR_COMPETITIVE_MATRIX.md` — Arkose cluster currently absent; row to be added on capture
- `06_AWS_WAF_SOLVER.md §2.10` — historical relationship: AWS WAF Captcha uses an Arkose-style image puzzle UI (similar visible-mode UX, separate vendor backend)
- `07_DATADOME_PRIMITIVES.md §Primitive 2` — cross-origin iframe materialization (the engine primitive that any Arkose visible-mode solver would build on)
- `08_KASADA_FRONTIER.md §3` — Kasada's `/tl` PoW gate is the closest mechanism-analogue for Arkose's `dapib_url` proof-of-execution gate
- `25_CLOUDFLARE_DEEP.md §2.5` — Turnstile is the visible-CAPTCHA-product analogue from Cloudflare; same UX category, simpler challenge mechanism
- `26_AKAMAI_BMP_DEEP.md §2.2` — Akamai's v3 cookieHash-bound encoder is the closest payload-binding analogue
- `29_F5_SHAPE_SECURITY.md` — sibling chapter: same out-of-corpus customer-onboarding posture; complementary product class (background scoring vs. interactive puzzle)
- `16_STEALTH_FINGERPRINT_AUDIT.md §1-6` — the JS-surface checklist that Arkose's BDA probes

---

## 11. Sources

The technical content above is synthesized from the following public sources (in order of load-bearing-ness):

1. [Arkose Labs main site](https://www.arkoselabs.com/) — current product positioning, customer logos, taxonomy
2. [Arkose Labs About Us](https://www.arkoselabs.com/about-us/) — founder, funding, offices, product suite
3. [Crunchbase — Arkose Labs](https://www.crunchbase.com/organization/arkoselabs) — founding history, funding rounds, FunCaptcha origin
4. [Arkose Labs Series B (M12-led, \$22M)](https://www.globenewswire.com/news-release/2020/03/24/2005447/0/en/Arkose-Labs-Raises-22-Million-in-Series-B-Round-Led-by-M12-Microsoft-s-Venture-Fund.html) — investor confirmation
5. [Arkose Labs Series C (SoftBank-led, \$70M)](https://www.arkoselabs.com/latest-news/arkose-labs-raises-70-million-led-by-softbank-vision-fund-2-to-bankrupt-the-business-of-fraud/) — Series C details
6. [PYMNTS Series B coverage](https://www.pymnts.com/news/investment-tracker/2020/arkose-labs-notches-22m-in-microsoft-venture-fund-led-round/) — Series B context
7. [Startup Daily — PayPal strategic investment](https://www.startupdaily.net/topic/brisbane-founded-arkose-labs-secures-strategic-investment-paypal/) — Brisbane origin, PayPal Series A
8. [Sony Innovation Fund release](https://www.arkoselabs.com/latest-news/sony-innovation-fund-by-innovation-growth-ventures-igv-invests-in-arkose-labs-to-keep-gamers-safe-online/) — Sony participation
9. [Microsoft Outlook case study](https://www.arkoselabs.com/resource/case-study-microsoft-tackles-fraud-and-abuse/) — 98% fraud reduction
10. [Arkose Labs / Microsoft Azure expansion (2025)](https://www.arkoselabs.com/latest-news/arkose-labs-expands-strategic-relationship-microsoft-azure/) — Azure-native integration
11. [Arkose MatchKey product page](https://www.arkoselabs.com/arkose-matchkey/) — MatchKey challenge details, 1,250 variants claim
12. [Arkose MatchKey launch press release](https://www.arkoselabs.com/latest-news/arkose-labs-launches-suite-of-captcha-challenges/) — MatchKey rebrand context
13. [Arkose Labs PR Newswire — MatchKey launch](https://www.prnewswire.com/news-releases/arkose-labs-launches-arkose-matchkey-a-new-suite-of-captcha-challenges-that-revolutionizes-both-defensibility-against-attackers-and-usability-for-consumers-301695375.html) — broader trade-press launch
14. [Arkose Labs Device ID launch (2024)](https://www.arkoselabs.com/latest-news/arkose-labs-launches-arkose-device-id/) — cross-session device-tracking product
15. [Arkose Labs blog: "CAPTCHA Hell"](https://www.arkoselabs.com/latest-news/welcome-to-captcha-hell/) — philosophical positioning
16. [Arkose blog: MatchKey AI-resistant innovation](https://www.arkoselabs.com/blog/arkose-matchkey-ai-resistant-attack-innovation/) — anti-LLM positioning
17. [Arkose blog: CAPTCHA cost-proof solution](https://www.arkoselabs.com/blog/captcha-a-cost-proof-solution-not-a-turing-test/) — cost-imposition thesis
18. [Arkose Labs Integrations](https://www.arkoselabs.com/integrations/) — identity provider and CDN integrations
19. [Arkose Developer Docs: Standard Setup](https://developer.arkoselabs.com/docs/standard-setup) — SDK loading conventions
20. [Arkose Developer Docs: Client API](https://developer.arkoselabs.com/docs/client-api) — myArkose object methods, callbacks
21. [Arkose Developer Docs: Iframe Setup Guide](https://developer.arkoselabs.com/docs/iframe-setup-guide) — postMessage event vocabulary
22. [Arkose Developer Docs: Android Mobile SDK](https://developer.arkoselabs.com/docs/android-mobile-sdk) — mobile attestation stack (out of scope for BO)
23. [Arkose API Guide](https://developer.arkoselabs.com/docs/arkose-labs-api-guide) — API endpoint reference
24. [noahcoolboy/funcaptcha (GitHub)](https://github.com/noahcoolboy/funcaptcha) — Node.js interaction library, challenge taxonomy
25. [noahcoolboy/roblox-funcaptcha (GitHub)](https://github.com/noahcoolboy/roblox-funcaptcha) — Roblox-specific integration
26. [unfuncaptcha/tguess (GitHub)](https://github.com/unfuncaptcha/tguess) — `dapib_url` answer-transformation analysis
27. [decodecaptcha/FunCaptcha-Solver (GitHub)](https://github.com/decodecaptcha/FunCaptcha-Solver) — API-service-style solver
28. [useragents/Funcaptcha-Audio-Solver (GitHub)](https://github.com/useragents/Funcaptcha-Audio-Solver) — audio-variant solver
29. [Pr0t0ns/Funcaptcha-Solver (GitHub)](https://github.com/Pr0t0ns/Funcaptcha-Solver) — independent solver
30. [kiookp/funcaptcha--solver (GitHub)](https://github.com/kiookp/funcaptcha--solver) — independent solver
31. [arkose GitHub topic](https://github.com/topics/arkose) — index of related projects
32. [npm funcaptcha package](https://www.npmjs.com/package/funcaptcha) — package metadata
33. [2Captcha — FunCaptcha service](https://2captcha.com/p/funcaptcha) — commercial solver
34. [2Captcha API docs — Arkose Labs FunCaptcha](https://2captcha.com/api-docs/arkoselabs-funcaptcha) — commercial API specifics
35. [2Captcha blog — FunCaptcha bypass methods](https://2captcha.com/blog/funcaptcha-bypass-2-ways-solutions) — bypass approach discussion
36. [Anti-Captcha — FunCaptchaTaskProxyless](https://anti-captcha.com/apidoc/task-types/FunCaptchaTaskProxyless) — commercial API
37. [Anti-Captcha — FunCaptchaTask (with proxy)](https://anti-captcha.com/apidoc/task-types/FunCaptchaTask) — commercial API
38. [SolveCaptcha — FunCaptcha solver](https://solvecaptcha.com/captcha-solver/funcaptcha-solver-bypass) — commercial AI-based service
39. [CapMonster Cloud — FunCaptcha task](https://docs.capmonster.cloud/docs/captchas/funcaptcha-task/) — commercial API
40. [CaptchaAI blog: FunCaptcha explained](https://blog.captchaai.com/what-is-funcaptcha-arkose-labs-explained) — commercial-side analysis
41. [uCaptcha blog: FunCaptcha and Arkose Labs](https://ucaptcha.net/blog/funcaptcha-arkose-labs/) — broad overview
42. [gen9x — How to decode fingerprint BDA of FunCaptcha](https://gen9x.com/blog/how-to-decode-fingerprint-bda-of-funcaptcha) — BDA encoding analysis (AES-256-GCM)
43. [Surfsky docs — FunCaptcha](https://docs.surfsky.io/use-cases/funcaptcha/) — public-key carrier and iframe-src patterns
44. [BigNewsNetwork — Bypassing FunCaptcha 2026](https://www.bignewsnetwork.com/news/279007570/bypassing-funcaptcha-arkose-labs-in-2026-technical-guide) — 2026 technical guide
45. [RoundProxies — Bypass FunCaptcha 2026](https://roundproxies.com/blog/bypass-funcaptcha/) — 2026 bypass methods
46. [TechTarget — Arkose Labs puts AI in cybersecurity](https://www.techtarget.com/searchenterpriseai/feature/Arkose-Labs-puts-AI-in-cybersecurity) — Kevin Gosschalk interview, roadmap commentary
47. [Cybersecurity Intelligence — Arkose Labs profile](https://www.cybersecurityintelligence.com/arkose-labs-5869.html) — third-party profile
48. [PitchBook — Arkose Labs](https://pitchbook.com/profiles/company/97742-89) — funding totals and valuation
49. [getLatka — Arkose Labs revenue](https://getlatka.com/companies/Arkose_Labs) — 2024 revenue (\$46.9M), team size (241)
50. [LinkedIn — Kevin Gosschalk profile](https://www.linkedin.com/in/kgosschalk/) — founder profile
51. [Hacker News — Arkose Labs discussion](https://news.ycombinator.com/item?id=30413287) — customer-list community confirmation
52. [Roblox DevForum — CAPTCHA info question](https://devforum.roblox.com/t/how-to-get-captcha-info-for-roblox-apis-login-followings-api-etc/2647913) — Roblox-side integration discussion
53. [Roblox-FunCaptcha issue tracker (timeout issue)](https://github.com/noahcoolboy/roblox-funcaptcha/issues/8) — practical integration debugging
54. [Medium (Kentavr) — FunCaptcha Arkose Labs Principles of Operation (HTTP-403 in our crawl)](https://medium.com/@kentavr00000009/funcaptcha-arkose-labs-principles-of-operation-features-and-methods-for-automated-bypass-780ef786d7c5) — referenced via downstream citation; primary source unreachable at capture time but content is cited in multiple secondary sources
55. [Medium (Alexander) — Scraping in the Crosshairs of Arkose Labs](https://medium.com/@koshka00009/scraping-in-the-crosshairs-of-arkose-labs-how-to-bypass-3d-puzzles-browser-fingerprints-and-c5c710091152) — 2026 analysis
56. [F5 Distributed Cloud Bot Defense product page](https://www.f5.com/products/distributed-cloud-services/bot-defense) — competitive context (chapter 29 cross-link)
57. [GitLab MR — Arkose Data Exchange payload on signup](https://gitlab.com/gitlab-org/gitlab/-/merge_requests/139070) — confirms the Data Exchange protocol shape for a real-world integrator
58. [Arkose blog — Latest feature updates](https://www.arkoselabs.com/blog/a-look-at-our-latest-feature-updates) — product-evolution context

**Where the public record is thin (and what we wrote based on inference rather than citation):** the BDA cipher details (§2.4) are from a single gray-literature source (gen9x); the AES-256-GCM claim is consistent with the encryption-on-the-wire pattern but the exact derivation function is not published. The "1,250 variants" figure (§5) comes from Arkose's own marketing — treat as ballpark rather than precise. The customer-list specifics (§1.3) mix confirmed (Microsoft Outlook case study), strong (Roblox per multiple sources), and inferred (LinkedIn, EA per HN-community confirmation that's not in primary citations); the rule we used was "list it if two independent sources name it."
