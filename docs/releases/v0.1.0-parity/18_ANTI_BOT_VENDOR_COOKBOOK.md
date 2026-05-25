# 18 — Anti-bot vendor cookbook

**Status:** reference encyclopedia
**Audience:** any contributor onboarding a new site (not in the 126
corpus) and needing to recognize the vendor, understand the challenge
mechanism, and know what is publicly known about defeating it.
**Companion docs:** `02_GAP_ANALYSIS.md` (per-site root-cause within
the corpus), `06_AWS_WAF_SOLVER.md` (AWS-WAF deep dive),
`07_DATADOME_PRIMITIVES.md` (DataDome primitives), `08_KASADA_FRONTIER.md`
(Kasada research arc), `04_TOOLING_SPEC.md` (capture tooling),
`13_FILE_LOCATIONS_INDEX.md` (file:line lookup),
`14_TESTING_VALIDATION.md` (drift catch),
`crates/browser/src/classify.rs:81-156` (the canonical marker tables).

---

## Why this doc exists

A contributor encounters a site that returns a 1.5 KB body and a
`xyz-CHL` classifier verdict from the holistic sweep. They need to
answer three questions in this order:

1. **What vendor is blocking?** (Headers + body markers + cookie names.)
2. **What does that vendor's challenge actually do?** (To know whether
   the gap is engine-fixable per `02_GAP_ANALYSIS.md` categories, or
   solver-required per `06_AWS_WAF_SOLVER.md §3 / 07_DATADOME_PRIMITIVES.md`.)
3. **What is the public state of the art for that vendor?** (To
   triage: drop-the-site, file-as-research, or fix-it-this-sprint.)

This chapter is structured to answer all three questions per vendor in
~50-100 lines of reading. The shape is intentionally encyclopedic —
not a plan. The deep-dive chapters (06, 07, 08) are the plans; this is
the lookup index for the rest of the field.

---

## 1. Vendor identification flowchart

A decision tree. Run it top-down against a captured HTTP response.

### 1.1 Response-header markers (highest precision — never false-positive)

| Header | Vendor | Notes |
|---|---|---|
| `x-amzn-waf-action: challenge` | AWS WAF (Challenge action) | §2.1 — engine logs at `crates/browser/src/page.rs:1061` |
| `x-amzn-waf-action: captcha` | AWS WAF (Captcha action) | §2.10 — interactive variant |
| `x-datadome: <status>` | DataDome | §2.2 — engine logs at `page.rs:1064`. `protected` = blocked, `failed` = failed challenge, `solved` = passed. |
| `server: cloudflare` + `cf-mitigated: challenge` | Cloudflare Managed/Bot Fight | §2.5 — `cf-mitigated` is set whenever CF mitigated the request |
| `cf-ray: <id>` + status 403/503 | Cloudflare (any product) | §2.5 — `cf-ray` is on every Cloudflare-proxied response, including passes; pair with status to narrow |
| `x-wbaas-token: <…>` | wbaas (Walmart bot-as-a-service) | §2.12 — engine logs at `page.rs:1067` |
| `x-akamai-transformed: <flag>` | Akamai Bot Manager (BMP) | §2.3 — typically on resource responses, not the doc itself |
| `x-perimeterx-id` / `x-px-uuid` | PerimeterX (HUMAN Security) | §2.6 — id token cookie family is `_px3` / `_pxhd` / `_pxvid` |
| `x-sucuri-id: <…>` | Sucuri | §2.8 |
| `x-iinfo: <encoded>` | Imperva (Incapsula) | §2.7 — also `x-cdn: Imperva` |
| `x-cdn: Imperva` | Imperva (Incapsula) | §2.7 |
| `x-imperva-id` (some tenants) | Imperva (Incapsula) | §2.7 |
| `x-kpsdk-st` / `x-kpsdk-ct` on resp | Kasada | §2.4 — usually on `/ips.js` resource resp + the doc when token issued |
| `x-armor-shield-zone` | Reblaze (now Imperva/Thales) | §2.7 |

### 1.2 Status-code shape

| Status | Body size | Likely meaning |
|---|---|---|
| `200` | > 50 KB | Real page (or large SPA shell — needs hydration check) |
| `200` | 500-15000 B | Challenge stub OR SPA shell. Disambiguate by body markers (§1.3). |
| `200` | < 500 B | Edge deny / rate-limit interstitial / pixel-only response. |
| `403` | > 15 KB | Usually the vendor's *interactive* CAPTCHA page (DD `rt:'c'`, CF Turnstile widget, hCaptcha gate). |
| `403` | 1-3 KB | The canonical small challenge stub. Vendor-marker check is decisive. |
| `429` | any | Rate-limit OR Kasada/PerimeterX/CF burst guard. Cookie name check resolves it. |
| `498` | any | Used by Kasada and some CF tenants as a custom "auth required" signal. The navigate loop already treats 403/429/498 identically (`page.rs:1045`). |
| `503` | < 5 KB | Classic Cloudflare JS Challenge ("Just a moment..." page). |
| `503` | > 50 KB | Likely real upstream outage, NOT a challenge — verify by re-pulling. |

### 1.3 Response body markers (case-sensitive substrings)

| Marker | Vendor | Strength | Engine location |
|---|---|---|---|
| `/ips.js` | Kasada | unambiguous (only on Kasada stubs) | classify.rs:106 (SMALL_BODY) |
| `_kpsdk` | Kasada | unambiguous | classify.rs:105 (SMALL_BODY) |
| `kpsdk` (lowercase substr) | Kasada | unambiguous | page.rs:2287 (v8_html_is_real guard) |
| `_abck` (substring) | Akamai BMP | strong | classify.rs:104, page.rs:2288 |
| `bm_sz` | Akamai BMP (BM Edge / "BM-Sensor") | strong | page.rs:2289 |
| `akam/13` | Akamai BMP bootstrap | weak — fires on benign Akamai pages | classify.rs:103 (needs co-signal — `AKAMAI_CHALLENGE_COSIGNAL` at classify.rs:127) |
| `pardon our interruption` | Akamai (Edge deny page) | strong | classify.rs:96, also co-signal at classify.rs:133 |
| `/_sec/cp_challenge` | Akamai sec-cpt | unambiguous | classify.rs:84 (UNAMBIGUOUS) |
| `sensor_data` | Akamai BMP sensor POST body | strong (co-signal) | classify.rs:128 |
| `bm-verify` | Akamai BMP verify endpoint | strong (co-signal) | classify.rs:129 |
| `captcha-delivery.com` | DataDome | strong — phrase-gated to small bodies | classify.rs:94 |
| `dd-script` | DataDome | strong | page.rs:2291 |
| `dd_engagement` | DataDome | strong | page.rs:2292 |
| `ddcaptchaencoded` | DataDome (captcha widget) | unambiguous | classify.rs:85 (UNAMBIGUOUS) |
| `/cdn-cgi/challenge-platform/` | Cloudflare orchestrator | strong (also appears on passed CF pages — pair with `_cf_chl_opt`) | page.rs:2293, classify.rs:249 |
| `_cf_chl_opt` | Cloudflare JS Challenge / Managed | unambiguous | classify.rs:83 (UNAMBIGUOUS) |
| `cf-browser-verification` | Cloudflare classic JSC ("Checking your browser") | unambiguous | classify.rs:82 |
| `just a moment` | Cloudflare interstitial title | strong — phrase-gated | classify.rs:92 |
| `cf-turnstile` | Cloudflare Turnstile widget | unambiguous | classify.rs:148 (INTERACTIVE_CAPTCHA_COSIGNAL) |
| `challenges.cloudflare.com/turnstile` | Cloudflare Turnstile widget | unambiguous | classify.rs:149 |
| `_pxhd` | PerimeterX (HUMAN) | strong | classify.rs:107 |
| `px-captcha` | PerimeterX interstitial | strong | classify.rs:114 |
| `press & hold` | PerimeterX Press-and-Hold widget | strong | classify.rs:95 |
| `AwsWafIntegration` | AWS WAF challenge stub | unambiguous | (not in classifier — only detection via header) |
| `gokuProps` | AWS WAF challenge stub | unambiguous | (not in classifier — only via header detection) |
| `awswaf.com` (host) | AWS WAF subresource | strong | (only via fetch log) |
| `hcaptcha.com` | hCaptcha widget | unambiguous | classify.rs:147 |
| `api2/bframe` / `api2/anchor` | reCAPTCHA v2 iframe | unambiguous | classify.rs:145-146 |
| `grecaptcha` | Google reCAPTCHA (any v) | medium (also appears on legit forms) | (not in classifier — bare token) |
| `arkoselabs.com` / `funcaptcha` | Arkose Labs FunCaptcha | unambiguous | (not in classifier — extend) |
| `_Incapsula_Resource` | Imperva Incapsula | unambiguous | (not in classifier — extend) |
| `visid_incap` / `incap_ses` | Imperva Incapsula cookies | unambiguous | (not in classifier — extend) |
| `reese84` | Imperva v2 / Thales | unambiguous | (not in classifier — extend) |
| `sucuri_cloudproxy_js` | Sucuri JS shield | unambiguous | (not in classifier — extend) |
| `mc_session` | Reblaze | unambiguous | (not in classifier — extend) |
| `forter` (host or fingerprint) | Forter | unambiguous | (not in classifier — extend) |

### 1.4 Cookie markers (set in response `Set-Cookie`, then carried forward)

| Cookie name | Vendor | Lifetime | Notes |
|---|---|---|---|
| `aws-waf-token` | AWS WAF | ~5 min default ("immunity time") | Sent BOTH via Set-Cookie and `x-aws-waf-token` request header on cross-domain |
| `datadome` | DataDome | session, rotating | Set on EVERY response from a DD origin, including failed ones — see `07_DATADOME_PRIMITIVES.md` Primitive 3 |
| `_abck` | Akamai BMP | persistent (year+) | "Score-bearing" — `~0~-1~-1~` infix means uncleared; clear value indicates a valid sensor solve |
| `bm_sz` | Akamai BM Edge | session | Set on first response; used to bind the sensor |
| `bm_mi` / `bm_sv` | Akamai BMP variants | session | Some tenants only |
| `sec_cpt` | Akamai sec-cpt | per-challenge | Set after the PoW bundle self-solves |
| `ak_bmsc` | Akamai 1.7 (legacy) | session | Pre-BMP variant; some sites still on this |
| `cf_clearance` | Cloudflare | ~30 min default | Required to access CF-protected origin after a challenge clear |
| `__cf_bm` | Cloudflare Bot Management | session | Behavioural session token (not a clearance) |
| `_px3` | PerimeterX | ~60 s | Main clearance — short-lived, requires constant refresh |
| `_pxhd` | PerimeterX | persistent (year+) | Device-level identity |
| `_pxvid` | PerimeterX | persistent | Visitor-id (long-lived) |
| `_pxff_*` | PerimeterX | session | Per-feature flags |
| `kpsdk` (`_kpsdk_ct` etc.) | Kasada | ~30 min for `_ct`, single-use for `_cd` | `x-kpsdk-ct` is the long-lived; `x-kpsdk-cd` is one-shot |
| `visid_incap_*` / `incap_ses_*` | Imperva Incapsula | session | Both required; mutating either invalidates |
| `reese84` | Imperva v2 / Thales | session | Set after `/_Incapsula_Resource` POST validates |
| `sucuri_cloudproxy_uuid_*` | Sucuri | session | Per-protected-site UUID |
| `mc_session` | Reblaze | session | |

The engine's generic clearance-cookie predicate is spec'd at
`07_DATADOME_PRIMITIVES.md §Primitive 3` (`cookies_carry_anti_bot_clearance`)
and covers the seven highest-volume vendors. Extending it to cover
the table above is a one-line change per new vendor.

### 1.5 The fast decision tree

```
                       ┌─ x-amzn-waf-action → AWS WAF (§2.1 or §2.10)
                       │
                       ├─ x-datadome → DataDome (§2.2)
                       │
                       ├─ cf-mitigated → Cloudflare (§2.5)
                       │      OR (cf-ray + status ∈ {403,503})
                       │
       headers?────────┼─ x-wbaas-token → wbaas (§2.12)
                       │
                       ├─ x-akamai-transformed → Akamai (§2.3)
                       │      (rare on doc; usually on resources)
                       │
                       ├─ x-perimeterx-id → PerimeterX (§2.6)
                       │
                       ├─ x-iinfo / x-cdn:Imperva → Imperva (§2.7)
                       │
                       └─ x-sucuri-id → Sucuri (§2.8)
                                │
                                ▼ no header signal — fall through to body
                       ┌─ AwsWafIntegration or gokuProps → AWS WAF
                       ├─ /ips.js or _kpsdk → Kasada (§2.4)
                       ├─ _abck or bm_sz or pardon our interruption → Akamai (§2.3)
                       ├─ captcha-delivery.com or dd-script → DataDome
       body?──────────┼─ /cdn-cgi/challenge-platform/ or _cf_chl_opt → Cloudflare
                       ├─ _pxhd or px-captcha or press & hold → PerimeterX
                       ├─ _Incapsula_Resource or visid_incap or reese84 → Imperva
                       ├─ hcaptcha.com → hCaptcha (§2.11)
                       ├─ arkoselabs.com or funcaptcha → Arkose (§2.10b)
                       └─ grecaptcha + interactive widget → Google reCAPTCHA (§2.9)
                                │
                                ▼ still nothing
                       Treat as "unknown vendor" — log capture + add a marker
                       (extend classify.rs:81-156 + page.rs:1054-1069). See §4.
```

---

## 2. Per-vendor chapters

Each chapter follows the same skeleton: description → markers →
challenge mechanism → public solver state (open-source projects, with
URLs) → BO coverage (per sweep data) → failure mode → telemetry to
capture.

---

### 2.1 AWS WAF — Bot Control + Challenge action

**One-line description:** AWS-managed WAF service with a JS proof-of-work
challenge that issues an `aws-waf-token` cookie via the `AwsWafIntegration`
SDK shipped as `challenge.js` from a per-tenant `*.token.awswaf.com` host.

**Detection markers**
- Header: `x-amzn-waf-action: challenge` (silent PoW), `x-amzn-waf-action: captcha` (interactive)
- Body: `AwsWafIntegration`, `gokuProps = { key, iv, context }`, `<script src="https://*.token.awswaf.com/.../challenge.js">`
- Cookie set on solve: `aws-waf-token=<JWT-like>`
- Stub size: 2011 B (Amazon variants), 1995 B (IMDb) — extremely consistent

**Challenge mechanism**
1. Origin returns the 2011-B stub when its WAF rule fires.
2. `challenge.js` loads from `<tenant>.token.awswaf.com` (~50-150 KB minified + a WASM blob).
3. Browser runs `AwsWafIntegration.saveReferrer()` then `AwsWafIntegration.getToken()`.
4. The SDK fingerprints the browser (50+ navigator/screen/WebGL/AudioContext properties), encrypts the result with `gokuProps.key`/`iv`, and POSTs to `<tenant>/inputs?client=browser`.
5. Server returns a PoW puzzle (HashcashScrypt / SHA-256 / NetworkBandwidth variant). SDK computes via WASM.
6. SDK POSTs solution to `<tenant>/verify`. Server returns `{"token": "..."}` and sets `Set-Cookie: aws-waf-token=...`.
7. SDK calls `window.location.reload(true)` — second GET carries the cookie, WAF lets through.
8. Documented `getToken` ceiling: 2 s wait then throws ([AWS spec](https://docs.aws.amazon.com/waf/latest/developerguide/waf-js-challenge-api-get-token.html)).

**Public solver state**
- [`xKiian/awswaf`](https://github.com/xKiian/awswaf) — Python + Go reverse, supports token + invisible CAPTCHA.
- [`neiii/aws-waf-solver`](https://github.com/neiii/aws-waf-solver) — independent reimplementation; documents the HashcashScrypt / SHA-256 / NetworkBandwidth flavour split.
- [`jonathanyly/awswaf-solver-api`](https://github.com/jonathanyly/awswaf-solver-api) — FastAPI service returning `aws-waf-token` strings.
- [`Switch3301/Aws-Waf-Solver`](https://github.com/Switch3301/Aws-Waf-Solver), [`aferapi/aws-waf-solver`](https://github.com/aferapi/aws-waf-solver) — third-party Python solvers, license/freshness vary.

**BO coverage**
- amazon-de / amazon-in / amazon-com-au / imdb: **fail consistently** (2011 B / 1995 B stub across all profiles, per `02_GAP_ANALYSIS.md:141-194`).
- amazon-com / amazon-co-uk / amazon-ca / amazon-fr: **variable pass** — AWS-side risk-rolling. Routing recovers 4 of 8 amazon variants.
- huggingface (`~/projects/browser_oxide_internal/docs/VERIFICATION_REPORT_2026_04_26.md:342`): passes cleanly — confirms WAF baseline is not uniformly hostile; Amazon-tenant config is more aggressive.

**Failure mode**
- 200 OK + 2011-B body, `[vendor-detect] aws-waf challenge` log line.
- Telemetry `POST` to `awswaf.com/.../report` fires (we run challenge.js) but `getToken()` never reaches its `.then(token => …)` continuation. No `/verify` POST, no cookie, no reload.

**Telemetry to capture** (per `04_TOOLING_SPEC.md`)
- `fetches.json` — must show `POST .../report` from BO and `POST .../verify` + `Set-Cookie: aws-waf-token=...` from Camoufox.
- `script_errors.json` — any `Promise` left unresolved in `challenge.js`; the fingerprint bail commonly silently rejects.
- `cookie_writes.json` — confirm `aws-waf-token` is NOT being set on BO; IS being set on Camoufox.

**See:** Full deep dive in `06_AWS_WAF_SOLVER.md` (deobfuscation recipe, fingerprint candidate list, solver design alternatives A/B/C).

---

### 2.2 DataDome

**One-line description:** Paris-based bot-mitigation vendor. Deploys an
interstitial challenge document that fetches `dd-script.js` from
`captcha-delivery.com`, injects a cross-origin iframe to
`geo.captcha-delivery.com/captcha/?...`, and runs a WASM-based PoW +
fingerprint inside the iframe.

**Detection markers**
- Header: `x-datadome: protected` (blocked), `x-datadome: failed` (failed challenge), `x-datadome: solved` (passed)
- Body: `captcha-delivery.com`, `dd-script`, `dd_engagement`, `ddcaptchaencoded`, `var dd = { ... }` literal object
- Cookie set on solve: `datadome=<long-base64-blob>`
- Stub size: 1424-1487 B for `rt:'i'` (silent), larger for `rt:'c'` (interactive)

**Challenge mechanism**
1. Origin returns 1424-B interstitial. Body contains `<script>var dd = {…}; </script>` with `t:'fe'` (frontend), `rt:'i'` (interstitial silent) or `rt:'c'` (interactive captcha).
2. `dd-script.js` (~30 KB obfuscated, daily-rotating WASM blob) is loaded from `captcha-delivery.com`.
3. Script injects iframe to `geo.captcha-delivery.com/captcha/?...` carrying device hints.
4. Iframe runs WASM that computes Picasso canvas hash + audio fingerprint + DOM-timing probes.
5. Iframe POSTs the result envelope to `captcha-delivery.com/captcha/check`. Response body contains a JSON with the cookie value to set.
6. Outer script reads `document.cookie`, detects the new `datadome=` and either reloads or auto-submits a hidden form.

**Public solver state**
- [`glizzykingdreko/Datadome-Captcha-Deobfuscator`](https://github.com/glizzykingdreko/Datadome-Captcha-Deobfuscator) — JS deobfuscator for the daily-rotating challenge bundle.
- [`glizzykingdreko/Datadome-GeeTest-Captcha-Solver`](https://github.com/glizzykingdreko/Datadome-GeeTest-Captcha-Solver) — image-recognition for the interactive slider variant.
- [`recaptchaUser/datadome-Interstitial-solver`](https://github.com/recaptchaUser/datadome-Interstitial-solver) — interstitial-only solver.
- [`Hyper-Solutions/hyper-sdk-js`](https://github.com/Hyper-Solutions/hyper-sdk-js) — commercial SDK covering DataDome alongside Akamai/Incapsula/Kasada.
- [`campo1312/DataDome`](https://github.com/campo1312/DataDome) — example DataDome HTML pages for familiarisation.
- Walkthrough: [glizzykingdreko / "Breaking Down Datadome Captcha WAF"](https://medium.com/@glizzykingdreko/breaking-down-datadome-captcha-waf-d7b68cef3e21).

**BO coverage**
- etsy: `DataDome-CHL 1424` across all 4 profiles. Pre-strip Phase 5 was on its way to flipping it (per memory `state_2026_05_16_phase5_datadome.md`). Recoverable via the 3 primitives in `07_DATADOME_PRIMITIVES.md`.
- tripadvisor: `DataDome-CHL 1430` × 4 profiles. Same recovery path.
- yelp: `DataDome-CHL 1424` × 4 profiles AND Camoufox also fails with `DataDome-CHL 1487` — interactive captcha variant (`rt:'c'`); stretch goal.
- leboncoin / wsj: previously flipped pre-strip, now degraded — same recovery path.
- All other DataDome sites with sensor (no captcha): typically passable after Primitives 1+2+3.

**Failure mode**
- 403 + 1424 B body, `[vendor-detect] datadome` log line.
- Outer engine never enters the challenge-iframe poll because `started_as_dd_challenge` requires a registered solver (per `07_DATADOME_PRIMITIVES.md §Primitive 2`). With no solver, `rematerialize_iframes` (`page.rs:649`) never runs, iframe never fetches, challenge never solves.

**Telemetry to capture**
- `fetches.json` — Camoufox shows `GET captcha-delivery.com/captcha/...`, then `POST captcha-delivery.com/captcha/check`, then `Set-Cookie: datadome=...`. BO shows none of these.
- `iter_summary.json` — BO sits at iter 0 with 1424-B body until 90 s budget exhausted; Camoufox flips to ≥ 50 KB body within ~5 s.

**See:** Full restoration plan in `07_DATADOME_PRIMITIVES.md` (Primitives 1/2/3 — CSP relax, iframe rematerialize, solved-cookie retry).

---

### 2.3 Akamai Bot Manager (BMP)

**One-line description:** Akamai's bot-management product layered on top
of their CDN. Issues the `_abck` clearance cookie after a successful
`sensor_data` POST, which carries an obfuscated payload of device
telemetry encrypted with a per-script PRNG.

**Detection markers**
- Header: `x-akamai-transformed` (resources), no canonical header on the doc itself; rely on body markers
- Body: `_abck` cookie reference, `bm_sz` cookie reference, `sensor_data`, `akam/13` (script src), `pardon our interruption` (edge deny copy), `/_sec/cp_challenge` (sec-cpt subpath)
- Cookies: `_abck`, `bm_sz`, `bm_mi`, `bm_sv`, `ak_bmsc`, `sec_cpt`
- Sub-products:
  - **BMP v2** — older `sensor_data` (TEA-CBC + per-tenant integrity field)
  - **BMP v3** — JSON colon-delimited, PRNG-shuffled, uses cookie state as encryption key; harder to static-reverse
  - **sec-cpt** — adversarial PoW bundle (browser computes preimage)
  - **BM Edge** — lighter-touch edge classifier (`bm_sz` only)

**Challenge mechanism**
1. Origin returns a doc with `<script src="...akam/13/...">` (BMP bootstrap) or a small 403 with `pardon our interruption`.
2. BMP script collects 200+ properties: navigator, screen, fonts, WebGL, AudioContext, plugin list, mouse/touch event listeners, timing of bootstrap, etc.
3. Script encrypts the payload (v2: TEA-CBC; v3: PRNG-shuffled, cookie-keyed) and POSTs to a per-tenant endpoint (commonly `/<random>` or `/akam/13/verify`).
4. Response sets `_abck` cookie. Score-bearing infix: `~0~-1~-1~` means uncleared, anything clear (no `-1~-1`) is valid.
5. Subsequent requests carry `_abck` — Akamai gate either accepts or re-challenges.
6. sec-cpt variant: the bundle itself is the challenge — a JS function computes a SHA preimage and the answer sets `sec_cpt`. Self-solves in our V8 today (per `memory/state_2026_05_16_phase5_datadome.md`).

**Public solver state**
- [`xiaoweigege/akamai2.0-sensor_data`](https://github.com/xiaoweigege/akamai2.0-sensor_data) — sensor_data + telemetry + sbsd + bm_s generator (claims 100% pass at 100 concurrency).
- [`i7solar/Akamai`](https://github.com/i7solar/Akamai) — Go-based cookie generator for legacy Akamai 1.7X sites.
- [`cirleamihai/akamai-1.7-cookie-generator`](https://github.com/cirleamihai/akamai-1.7-cookie-generator) — Python `requests`-based v1.7 generator.
- [`Hyper-Solutions/hyper-sdk-js`](https://github.com/Hyper-Solutions/hyper-sdk-js) — commercial SDK.
- Walkthrough: [glizzykingdreko / "Akamai v3 Sensor Data: Deep Dive into Encryption, Decryption, and Bypass Tools"](https://medium.com/@glizzykingdreko/akamai-v3-sensor-data-deep-dive-into-encryption-decryption-and-bypass-tools-da0adad2a784).
- Reference (vendor docs): [Scrapfly bypass guide](https://scrapfly.io/bypass/akamai), [ZenRows Akamai bypass](https://www.zenrows.com/blog/bypass-akamai).

**BO coverage**
- adidas — passes on `firefox_135_macos` uniquely (BO advantage over Camoufox per `12_COMPETITIVE_LANDSCAPE.md §1.1`).
- homedepot — sec-cpt; flipped on `iphone_15_pro_safari_18` pre-strip (memory `state_2026_05_16_phase5_datadome.md`), currently failing in HEAD.
- bestbuy — Akamai SPA shell; even Camoufox fails. In `02_GAP_ANALYSIS.md` hard residual.
- Many other Akamai-fronted sites pass cleanly because the engine's V8 is faithful enough to satisfy passive collection + the sec-cpt bundle self-solves.

**Failure mode**
- 403 + small body containing `_abck` reference or `pardon our interruption`.
- Or: 200 OK with the bootstrap loaded, but `_abck` value stays unclear (`~0~-1~-1~`) and subsequent requests get 403.

**Telemetry to capture**
- `cookie_writes.json` — capture every Set-Cookie for `_abck`, `bm_sz`. Compare the `_abck` infix across iterations: if it stays `-1~-1`, the sensor was rejected; if it clears, the gate passed.
- `fetches.json` — locate the `sensor_data` POST URL; capture its body (Akamai's sensor POST body is the gold seam for diffing fingerprints).

---

### 2.4 Kasada

**One-line description:** Australian bot-mitigation vendor. Ships `ips.js`
containing a custom bytecode VM that runs an encrypted PoW + active
fingerprint, producing `x-kpsdk-ct` (30-min reusable) and `x-kpsdk-cd`
(one-shot) tokens carried as **request headers** (not just cookies).

**Detection markers**
- Headers (response): `x-kpsdk-st`, `x-kpsdk-ct` on the `/ips.js` resource
- Body: `/ips.js`, `_kpsdk`, `KPSDK` global
- Token cookies/headers used by client: `x-kpsdk-ct`, `x-kpsdk-cd`, `x-kpsdk-im`, `x-kpsdk-fc`, `x-kpsdk-h`, `x-kpsdk-v`, `x-kpsdk-r`
- Stub size: 740 B (canadagoose), 745 B (hyatt), 1764-1772 B (realtor)

**Challenge mechanism**
1. Origin returns a small interstitial with `<script src="/ips.js">`.
2. `ips.js` is a JS file containing an interpreter for Kasada's own custom bytecode (a VM with ~1000 opcodes, obfuscated). The bytecode runs:
   - Passive collection (navigator, fonts, WebGL, audio, CSS `calc()` math precision probes, `Function.prototype.toString` leak detectors)
   - PoW computation (the "ct" token cost is non-trivial — seconds of CPU)
   - Sensor POST to `/tl` with the collected envelope, wrapped in `base64(json({"data": base64(xor(plaintext, b"omgtopkek"))}))` (per `memory/kasada_wrapper_cracked_and_remaining_leaks.md`).
3. Response from `/tl` sets `x-kpsdk-ct` header. Client retains and forwards on subsequent navs.
4. The `x-kpsdk-cd` is computed per-request from `x-kpsdk-ct` + a request fingerprint.

**Public solver state**
- [`0x6a69616e/kpsdk-solver`](https://github.com/0x6a69616e/kpsdk-solver) — Playwright-based solver.
- [`unicorn-aio/kpsdk`](https://github.com/unicorn-aio/kpsdk), [`Mrclintons/kpsdk-1`](https://github.com/Mrclintons/kpsdk-1) — community ips.js reverse copies.
- [`lktop/kpsdk`](https://github.com/lktop/kpsdk) — `x-kpsdk-ct` / `x-kpsdk-cd` encryption-algorithm analysis.
- [`nixbro/Kasada-Solver`](https://github.com/nixbro/Kasada-Solver) — high-level overview.
- [`Hyper-Solutions/hyper-sdk-js`](https://github.com/Hyper-Solutions/hyper-sdk-js) — commercial SDK with documented `x-kpsdk-*` header forwarding rules.

**BO coverage**
- canadagoose / hyatt / realtor: **fail across all profiles**. Even Camoufox fails them. They're the open-source SOTA frontier (`08_KASADA_FRONTIER.md`).
- Live oracle captured: `ab_harness/tl/hyatt.tl_body.bin` (36 KB decrypted plaintext) — diffing against our `/tl` output is the K2-DIFF lever.
- Engine internals fix backlog: CSS calc math (sin/cos/tan), 16 error-bearing fields (`bot1225` `csc` `kl` `dpv` `smc` `sfc` `sdt` `nppm` `fsc` `npc` `esd` `wse` `bfe` `ao` `cbf`), `_maskAsNative` audit. See `08_KASADA_FRONTIER.md §4`.
- K1 deferral of the parallel Rust `compute_cd_header` already shipped on branch `fix/engine-fp-backlog`.

**Failure mode**
- 200 + small body or 429 + `bot1225.b:1` error report (the canonical "VM caught a fingerprint mismatch" signal).
- `/tl` POST happens but the response is `b:1`, indicating Kasada minted a token but flagged it.

**Telemetry to capture**
- `fetches.json` — `POST /tl` body + response. Decode with the XOR wrapper at `docs/kasada_ips_analysis/scratch/decrypt_report.py`.
- Custom K2-DIFF tool — in-VM plaintext sensor dump per `08_KASADA_FRONTIER.md §Lever 1` (intercepts `XMLHttpRequest.send` / `fetch` on URL `/tl`, captures body PRE-XOR).
- Compare each field of the decoded sensor against `ab_harness/tl/hyatt.tl_body.bin` and `canadagoose.pcap+.keys`.

**See:** Full research arc in `08_KASADA_FRONTIER.md`.

---

### 2.5 Cloudflare — Turnstile / Bot Fight Mode / Managed Challenge / JS Challenge

**One-line description:** Cloudflare's CDN-integrated bot mitigation,
deployed in four progressive tiers: Bot Fight Mode (free), Managed
Challenge (the new default — Turnstile-backed, mostly silent), JS
Challenge (classic "Just a moment..." interstitial), and the
Turnstile widget (visible).

**Detection markers**
- Header: `cf-ray: <id>` (always on CF), `cf-mitigated: challenge` (only when CF actively mitigates), `server: cloudflare`
- Status: 403/503 + small body strongly suggests challenge
- Body: `_cf_chl_opt = {…}` (orchestrator object — strongest signal), `/cdn-cgi/challenge-platform/` (the JSD URL — also appears on PASSED CF pages, so pair with `_cf_chl_opt`), `cf-browser-verification` (classic JSC), `just a moment`, `cf-turnstile` (widget), `challenges.cloudflare.com/turnstile`
- Cookies: `cf_clearance` (clearance after a solve, ~30 min), `__cf_bm` (behavioural session)

**Challenge mechanism — Managed Challenge (modern default)**
1. Origin (proxied through CF) sees a "suspicious" request; CF returns a small HTML body with `<script src="/cdn-cgi/challenge-platform/h/b/jsd/...">`.
2. The orchestrator script computes a fingerprint envelope, POSTs to a CF endpoint, server replies with a Turnstile widget challenge (sometimes silent, sometimes interactive).
3. If silent: client computes a PoW + passes hidden behavioural checks (timing, mouse events if available, hardware-attestation-like signals).
4. If interactive: user clicks the Turnstile checkbox; CF widget posts back.
5. CF sets `cf_clearance`; original URL re-fetched.

**Challenge mechanism — JS Challenge (classic, "Just a moment...")**
1. Origin returns 503 + "Just a moment..." page.
2. Body contains an obfuscated JS challenge (math problem evaluated after a 5 s wait — the literal `setTimeout(…, 5000)`).
3. Form auto-submits with the answer → `cf_clearance` issued.

**Public solver state**
- [`vvanglro/cf-clearance`](https://github.com/vvanglro/cf-clearance) — Python Playwright-based; emphasizes "use the SAME IP and UA when reusing the cookie".
- [`x404xx/Turnstile-Solver`](https://github.com/x404xx/Turnstile-Solver) — Rest-API Turnstile bypass.
- [Byparr](https://github.com/) — Camoufox-backed reverse proxy with the highest measured Turnstile pass rate (per [Scrape.do 2026 bypass guide](https://scrape.do/blog/bypass-cloudflare/)).
- [FlareSolverr](https://github.com/) — Selenium + undetected-chromedriver; legacy but still common in scraper stacks.
- Topic: [`cloudflare-turnstile-bypass` GitHub topic](https://github.com/topics/cloudflare-turnstile-bypass)
- Reference: [Scrapfly bypass guide](https://scrapfly.io/blog/posts/how-to-bypass-cloudflare-turnstile), [ZenRows Cloudflare bypass 2026](https://www.zenrows.com/blog/bypass-cloudflare).

**BO coverage**
- udemy — Managed Challenge orchestrator shell; classified `Cloudflare-CHL` in a large body (the `_cf_chl_opt` shell that ran but never cleared, per classify.rs:189). Listed in `02_GAP_ANALYSIS.md` selective-CSP-bypass set (`page.rs:2066-2080`).
- Many CF-fronted sites: pass because CF often skips the challenge for "low risk" requests.
- iphone profile: hit by managed-challenge on 6 sites per `11_PER_PROFILE_STRATEGY.md` — Safari sec-ch-ua absence is sometimes a signal.

**Failure mode**
- 503 + "Just a moment..." HTML (classic JSC).
- 200 OK + body containing `_cf_chl_opt` + `<script src="/cdn-cgi/challenge-platform/h/b/jsd/...">` that the engine ran but did NOT complete (Managed Challenge orchestrator shell — classified `ChallengeIncomplete` per classify.rs:189).
- 403 + interactive Turnstile widget body (Turnstile high-confidence challenge).

**Telemetry to capture**
- `fetches.json` — POSTs to `/cdn-cgi/challenge-platform/...` and the eventual `Set-Cookie: cf_clearance=...` are the success markers.
- `script_errors.json` — Managed Challenge orchestrator routinely throws when it fails fingerprinting; the throw is silent-by-design.

---

### 2.6 PerimeterX / HUMAN Security

**One-line description:** PerimeterX (acquired by HUMAN Security) ships a
JS sensor that fingerprints + behaviourally observes the user; once
satisfied, issues a `_px3` clearance cookie (~60 s lifetime, designed
to require continual interaction).

**Detection markers**
- Headers: `x-perimeterx-id`, `x-px-uuid` (tenant-dependent)
- Body: `px-captcha` (interstitial hook), `_pxhd` reference, `press & hold` (the PaH widget literal), `https://*.perimeterx.net/init.js`
- Cookies: `_px3` (~60 s clearance), `_pxhd` (device id, year+), `_pxvid` (visitor id), `_pxff_*` (feature flags), `_px2` (legacy)

**Challenge mechanism**
1. PerimeterX sensor JS (often from `/init.js` or inlined) collects browser fingerprint.
2. Sensor POSTs encrypted payload to `/xhr/api/v2/collector` (tenant-specific endpoint).
3. Server scores the request. Low score → `_px3` issued, high score → "Press & Hold" interactive widget (or block).
4. PaH widget detects sustained mouse-down on a button; if held with realistic pressure-timing curve, issues `_px3`.
5. The `_px3` cookie's 60-second TTL forces continual re-fingerprinting — this is the vendor's defining design choice.

**Public solver state**
- [`MiddleSchoolStudent/PerimeterX-solver`](https://github.com/MiddleSchoolStudent/PerimeterX-solver) — deobfuscation tool + solver; community-tested.
- Vendor-side SDKs (for legitimate site owners): [`PerimeterX/perimeterx-node-express`](https://github.com/PerimeterX/perimeterx-node-express), [`PerimeterX/perimeterx-python-wsgi`](https://github.com/PerimeterX/perimeterx-python-wsgi) — useful for understanding the cookie validation logic from the server side.
- Reference: [Scrapfly bypass](https://scrapfly.io/bypass/perimeterx), [ZenRows guide](https://www.zenrows.com/blog/perimeterx-bypass), [thewebscrapingclub Bypassing PerimeterX 3](https://substack.thewebscraping.club/p/the-lab-56-bypassing-perimeterx-3).

**BO coverage**
- **zillow — PASSES (BO advantage over Camoufox)** per `12_COMPETITIVE_LANDSCAPE.md §1.1`. Routed best-of-4.
- Other PerimeterX sites in corpus: not separately catalogued.

**Failure mode**
- 403 + small body with `px-captcha` or `press & hold` literal.
- Or: 200 + clean body but follow-on requests 403 because `_px3` expired after 60 s and was never refreshed.

**Telemetry to capture**
- `cookie_writes.json` — track `_px3` lifecycle: when set, when expired, when re-issued.
- `fetches.json` — POSTs to `/xhr/api/v2/collector` (or tenant variant).

---

### 2.7 Imperva (Incapsula) / Reblaze / Thales

**One-line description:** Long-running WAF, originally Incapsula, now
Imperva. Reblaze is a sister product (also under Thales). Detection
combines TLS fingerprint + an obfuscated JS challenge (`_Incapsula_Resource`
or `reese84`) that fingerprints 180+ browser signals.

**Detection markers**
- Headers: `x-iinfo: <encoded>` (Incapsula incident id), `x-cdn: Imperva`, `x-armor-shield-zone` (Reblaze)
- Body: `Incapsula incident ID`, `_Incapsula_Resource`, `Powered By Incapsula`, `subject=WAF Block Page` (mailto on the block page)
- Cookies: `visid_incap_*` (visitor id), `incap_ses_*` (session — BOTH required), `reese84` (v2/Thales clearance), `mc_session` (Reblaze)

**Challenge mechanism**
1. Origin behind Imperva returns the block page or a JS challenge document.
2. JS challenge (`/_Incapsula_Resource?...`) collects 180+ properties: canvas, WebGL, AudioContext, navigator, mouse events.
3. POSTs to `/_Incapsula_Resource?SWJIYLWA=...` with the encrypted payload.
4. Response sets `visid_incap_<site_id>` + `incap_ses_<n>_<site_id>` (and on v2 / Thales tenants, `reese84`).
5. Subsequent requests carry both cookies. Session-consistency-checked: mutating either invalidates immediately.
6. Reblaze (now Imperva): similar pattern, `mc_session` cookie, different endpoint.

**Public solver state**
- Walkthrough: [glizzykingdreko / Akamai v3 article](https://medium.com/@glizzykingdreko/akamai-v3-sensor-data-deep-dive-into-encryption-decryption-and-bypass-tools-da0adad2a784) covers the v3 sensor pattern that Imperva also uses.
- Reference: [Scrapfly bypass](https://scrapfly.io/blog/posts/how-to-bypass-imperva-incapsula-anti-scraping), [ZenRows](https://www.zenrows.com/blog/incapsula-bypass), [scrapebadger reese84 deep dive](https://scrapebadger.com/imperva-bypass).
- Commercial: [2captcha Imperva bypass](https://2captcha.com/h/imperva-bypass).
- Vendor docs: [Imperva ABP cookie scope](https://docs.imperva.com/bundle/advanced-bot-protection/page/75134.htm), [Thales cookie scope](https://docs-cybersec.thalesgroup.com/bundle/advanced-bot-protection/page/75136.htm).

**BO coverage**
- Not explicitly tracked in the 126-corpus. Likely covered indirectly via Akamai/CF routing.
- Add to `classify.rs` markers (§4 below).

**Failure mode**
- 403 + body with `Incapsula incident ID` or `Request unsuccessful. Incapsula incident ID: ...`.
- Or: 200 + JS challenge stub with `_Incapsula_Resource` script tag.

**Telemetry to capture**
- `cookie_writes.json` — `visid_incap_*` + `incap_ses_*` lifecycle. Either being absent on the next request after a 403 indicates the sensor was rejected.
- `fetches.json` — POSTs to `/_Incapsula_Resource?SWJIYLWA=...`.

---

### 2.8 Sucuri

**One-line description:** Lower-tier WAF, common on smaller WordPress
sites. Simpler than Akamai/Imperva — usually a single JS shield, no
WASM, no behavioural component.

**Detection markers**
- Headers: `x-sucuri-id: <…>`, `server: Sucuri/Cloudproxy`
- Body: `sucuri_cloudproxy_js`, `Sucuri WebSite Firewall`
- Cookies: `sucuri_cloudproxy_uuid_*`

**Challenge mechanism**
1. Origin returns a small interstitial with `<script>` that does a base64-decoded cookie set + form submit, OR a simple 5 s wait.
2. After cookie set → reload.
3. Most aggressive variant: hCaptcha or reCAPTCHA gate.

**Public solver state**
- No widely-cited dedicated Sucuri solver — the JS shield is simple enough that a faithful headless browser (or even basic `requests` + cookie parser) usually clears it.

**BO coverage**
- Not blocking; passes via the standard navigate loop.

**Failure mode**
- 403 + interstitial; usually clears on next iter.

**Telemetry to capture**
- `fetches.json` + `cookie_writes.json` — verify the cookie sets and the reload completes.

---

### 2.9 Google reCAPTCHA v2 / v3 / Enterprise

**One-line description:** Google's CAPTCHA service. v2 is the visible
checkbox + image-grid; v3 is a fully invisible score (0.0-1.0) that the
site decides how to act on; Enterprise is v3 with more attestation
signals (used by larger sites).

**Detection markers**
- Body: `https://www.google.com/recaptcha/api.js`, `https://www.recaptcha.net/recaptcha/enterprise.js`, `https://www.gstatic.com/recaptcha/releases/.../recaptcha__en.js`, `grecaptcha`, `g-recaptcha-response`, `g-recaptcha-badge`
- v2-specific: `api2/anchor`, `api2/bframe` (the iframe URLs the widget loads), `i'm not a robot`
- Worker: `recaptcha/enterprise/webworker.js` (Enterprise variant runs scoring inside a Worker)

**Challenge mechanism**
- v3 / Enterprise: invisible. Page calls `grecaptcha.execute(siteKey, {action: 'login'})` → returns a token. Site posts token to its backend for verification with Google. Score < threshold → site rejects.
- v2: user clicks the checkbox, sometimes shown image-grid; widget posts back; site gets a token.

**Public solver state**
- Commercial: [2captcha](https://2captcha.com/), [CapSolver](https://www.capsolver.com/), [Anti-Captcha](https://anti-captcha.com/) — all offer reCAPTCHA-as-a-service via API key.
- Open-source v2 audio solvers exist but are heavily rate-limited by Google's audio-challenge anti-abuse.
- No reliable "no-API" v3 / Enterprise solver — the score is server-side opaque.

**BO coverage**
- duolingo: Enterprise variant, uses a Worker (`webworker.js`); the SPA hydrates only after `grecaptcha.execute()` resolves. Per `02_GAP_ANALYSIS.md §2`, BO's Worker subsystem may not satisfy the Worker-side scoring; close miss (13.5 KB to gate). Recoverable per `05_SPA_HYDRATION_CLUSTER.md`.

**Failure mode**
- SPA shell stays unhydrated (no error — `grecaptcha.execute()` promise never resolves with a high-enough score).
- 403 if site uses v2 and we never solve.

**Telemetry to capture**
- `fetches.json` — captures the `enterprise.js`, `webworker.js`, and the eventual `/recaptcha/.../reload` POST.
- `script_errors.json` — Worker-side errors specifically; Worker scope is separate from main page console.

---

### 2.10 AWS WAF Captcha (interactive)

**One-line description:** Same vendor as §2.1, different rule action.
Returns a visible CAPTCHA (typically an Arkose Labs-style image
puzzle) when the WAF rule action is `captcha` rather than `challenge`.

**Detection markers**
- Header: `x-amzn-waf-action: captcha`
- Body: `AwsWafCaptcha` (vs `AwsWafIntegration`)
- Cookies: still `aws-waf-token` on solve

**Challenge mechanism**
- Same flow as §2.1 but the puzzle is interactive — needs a user.

**Public solver state**
- [`Needslq/aws-amazon-captcha-solving`](https://github.com/Needslq/aws-amazon-captcha-solving) — uses CapSolver as a backend.
- [`luminati-io/AWS-WAF-captcha-solver`](https://github.com/luminati-io/AWS-WAF-captcha-solver) — Bright Data solver.
- Reference: [CapSolver AWS-WAF rankings 2026](https://www.capsolver.com/blog/aws-waf/top-aws-solver-ranking), [RoundProxies bypass guide 2026](https://roundproxies.com/blog/bypass-aws-waf/).

**BO coverage**
- Not in our corpus paths (the Amazon-tenant ones use the silent `challenge` action). Treat as out-of-scope for headless: interactive CAPTCHA always needs a solving service or a real user.

**Failure mode**
- 403 + visible puzzle HTML. No way to clear from a headless browser without API integration.

### 2.10b Arkose Labs (FunCaptcha)

**One-line description:** Image-puzzle CAPTCHA (rotate the animal, pick
the dice with matching sum, etc.). Commonly invoked from Microsoft,
LinkedIn, Roblox, Twitch.

**Detection markers**
- Body: `arkoselabs.com`, `funcaptcha`, `client-api.arkoselabs.com`
- Iframe: `https://client-api.arkoselabs.com/v2/<public_key>/index.html`

**Public solver state**
- [`decodecaptcha/FunCaptcha-Solver`](https://github.com/decodecaptcha/FunCaptcha-Solver) — API service.
- [`useragents/Funcaptcha-Audio-Solver`](https://github.com/useragents/Funcaptcha-Audio-Solver) — audio variant via speech recognition.
- [`Pr0t0ns/Funcaptcha-Solver`](https://github.com/Pr0t0ns/Funcaptcha-Solver), [`kiookp/funcaptcha--solver`](https://github.com/kiookp/funcaptcha--solver) — independent.
- Topic: [`arkose` GitHub topic](https://github.com/topics/arkose).
- Commercial: [Anti-Captcha FunCaptchaTaskProxyless](https://anti-captcha.com/apidoc/task-types/FunCaptchaTaskProxyless).

**BO coverage**
- Not in 126-corpus.

**Failure mode**
- Interactive widget; not headless-passable without a service.

---

### 2.11 hCaptcha

**One-line description:** Cloudflare-aligned reCAPTCHA alternative.
Visible image-puzzle checkbox; widely used as the "harder than
Turnstile" tier for sites that aren't fully behind CF.

**Detection markers**
- Body: `hcaptcha.com`, `<div class="h-captcha">`, `js.hcaptcha.com/1/api.js`
- Iframe src: `https://newassets.hcaptcha.com/captcha/v1/...`

**Challenge mechanism**
- Image grid; user picks matching tiles. Newer "Enterprise" mode is invisible scoring (similar to reCAPTCHA v3).

**Public solver state**
- Commercial: [SolveCaptcha](https://github.com/solvercaptcha/solvecaptcha-python), [CapSolver hCaptcha](https://www.capsolver.com/), [2captcha](https://2captcha.com/) — all offer hCaptcha-as-a-service.
- Open-source image-recognition solvers exist but pass rates degrade as hCaptcha rotates puzzle sets.

**BO coverage**
- Not in 126-corpus directly.

**Failure mode**
- Interactive widget; not headless-passable without a service.

---

### 2.12 wbaas (Walmart bot-as-a-service)

**One-line description:** Walmart's in-house bot-mitigation, exposed
behind some of their endpoints. Less documented than the major
vendors.

**Detection markers**
- Header: `x-wbaas-token: <…>` (engine logs at `page.rs:1067`)
- Body markers: not separately catalogued.

**Challenge mechanism**
- Unknown publicly. Likely similar to PerimeterX-style scoring.

**Public solver state**
- No widely-cited public solver.

**BO coverage**
- walmart.com: hard target, in the selective-CSP-bypass list at `page.rs:2066-2080`.

**Failure mode**
- 403 / 429 + small body. Often paired with Akamai.

---

### 2.13 Friendly Captcha

**One-line description:** EU-based PoW-only CAPTCHA — no image
recognition, no behavioural. Pure proof-of-work that the user's
browser computes silently.

**Detection markers**
- Body: `friendlycaptcha.com`, `<div class="frc-captcha">`, `https://api.friendlycaptcha.com/api/v1/puzzle`

**Challenge mechanism**
- Page widget POSTs to `/api/v1/puzzle` → gets a PoW challenge → browser computes (1-30 s of CPU) → POSTs solution → token issued.
- 100% solvable headless because there's no fingerprint, no behavioural component.

**Public solver state**
- Trivially solvable: clone the PoW algorithm (documented in their open-source widget source) and POST.
- Not commonly listed in solver guides because it's not a typical blocker.

**BO coverage**
- Not in 126-corpus.

**Failure mode**
- Widget visible but engine never computes the PoW (because the widget normally requires a user click "Start" — though the silent variant exists).

---

### 2.14 Forter

**One-line description:** E-commerce fraud-detection vendor, less common
as a general bot blocker but appears in checkout flows on large
retailers.

**Detection markers**
- Body: `<script src="https://api.forter.com/.../forter.js">`, `ftr_pixel_id` cookie/global
- Cookies: `forter-token`, `ftr_*`

**Challenge mechanism**
- Passive — collects device fingerprint + behavioural for fraud scoring. Rarely blocks navigation; mostly used at checkout for risk scoring.

**Public solver state**
- No notable open-source solver — Forter is rarely a blocking gate, more a risk signal.

**BO coverage**
- Not blocking the 126-corpus.

**Failure mode**
- Checkout submit rejected, page navigation usually succeeds.

---

## 3. New-vendor onboarding playbook

A contributor encounters a NEW site (not in the 126 corpus) that returns
a 1.5 KB body and an unknown CHL classification. Walk these steps in
order; each builds artifacts the next step needs.

### Step 1 — Capture the response

```bash
URL='https://example.com/'
mkdir -p /tmp/newvendor && cd /tmp/newvendor

curl -sS -D headers.txt \
     -A 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36' \
     -H 'Accept: text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8' \
     -H 'Accept-Language: en-US,en;q=0.9' \
     "$URL" \
     -o body.html

wc -c body.html
cat headers.txt
```

### Step 2 — Run the §1.5 decision tree

Header markers first, body markers second. If any match resolves a vendor
in §2, stop here — skip to Step 4.

### Step 3 — Vendor not yet identified

If §1 yields nothing:

1. **Body length signal.** 500-3000 B = small interstitial (challenge stub
   most likely). 50-500 B = pure pixel/empty body (edge deny). > 50 KB =
   either real page (look for content) or interactive captcha gate.
2. **String archaeology.** `grep -oE '[a-z0-9-]+\.(com|net|io)' body.html`
   then look up any hosts you don't recognize on a search engine —
   most anti-bot vendors load their JS from a recognizable host.
3. **Cookie names.** `grep -oE 'Set-Cookie: [^=]+=' headers.txt` — any
   cookie name resembling `_<vendor>_*` is a tell.
4. **Script src host hosts to look up:** `arkoselabs.com`, `hcaptcha.com`,
   `gstatic.com/recaptcha`, `cloudflare.com/turnstile`, `forter.com`,
   `perimeterx.net`, `captcha-delivery.com`, `awswaf.com`,
   `incapsula.com`, `friendlycaptcha.com`.

If you still can't identify the vendor after Steps 1-3, file a new
research entry in `15_OPEN_QUESTIONS.md` with the captured body +
headers attached.

### Step 4 — Capture with full BO + Camoufox tooling (per `04_TOOLING_SPEC.md`)

```bash
# Create a single-site corpus file
cat > /tmp/new_corpus.json <<JSON
[{"cat":"unknown","name":"newsite","url":"$URL"}]
JSON

# BO capture
target/release/examples/sweep_metrics chrome_148_macos \
    /tmp/new_corpus.json /tmp/bo_out.json --capture newsite

# Camoufox capture
python /tmp/cam_capture.py newsite --har /tmp/cam.har
```

You now have:

- `/tmp/capture/bo/chrome_148_macos/newsite/{body.html, fetches.json, script_errors.json, cookie_writes.json, …}`
- Camoufox HAR + cookies + body

### Step 5 — Fingerprint deep-dive (per `06_AWS_WAF_SOLVER.md §2` methodology)

Open the BO and Camoufox `body.html` side-by-side. If they differ:

1. **Camoufox larger, BO smaller** → vendor fingerprint mismatch on BO.
   Walk the §2 candidate signals table from `06_AWS_WAF_SOLVER.md` (the
   list applies to any vendor — webdriver, UA-CH consistency, hardware
   concurrency, canvas hash, WebGL extensions, AudioContext, performance.now
   quantization, etc.).
2. **Both larger, BO smaller** → recoverable gap, walk §2 of *this*
   chapter for the vendor mechanism and locate which step is missing.
3. **Both fail** → either even-Camoufox-frontier (Kasada class), or a
   true vendor-solver-required gap.

### Step 6 — Decision: engine fix vs solver work

Use the categories from `02_GAP_ANALYSIS.md`:

| Symptom | Likely category | Action |
|---|---|---|
| Camoufox passes, BO ≤ 5 KB body | Fingerprint mismatch on a specific signal | Engine-side fix in `crates/stealth` / `crates/js_runtime` per `06_AWS_WAF_SOLVER.md §3 Alt A` |
| Camoufox passes, BO has 50 KB unhydrated SPA shell | SPA-hydration cluster (Worker, fetch, IntersectionObserver) | Engine-side fix per `05_SPA_HYDRATION_CLUSTER.md` |
| Camoufox passes via cross-origin iframe + cookie write; BO 1.4 KB stub | Engine-internal primitive missing | Apply `07_DATADOME_PRIMITIVES.md` (Primitives 1/2/3) generically |
| Both engines fail; vendor is Kasada | Open-research frontier | `08_KASADA_FRONTIER.md` lever list — out of v0.1.0 scope |
| Both engines fail; vendor is AWS-WAF + paid solvers exist | Vendor-solver required | Spec into private `vendor_solvers` crate per `06_AWS_WAF_SOLVER.md §3 Alt B/C` — do NOT add bypass code to public engine |
| Body contains visible CAPTCHA (hCaptcha, Arkose, AWS-WAF Captcha) | Interactive solver required | Out of scope for headless — needs a CAPTCHA-solving service integration |

---

## 4. Detection improvements for BO

Per `13_FILE_LOCATIONS_INDEX.md`, BO currently checks ~13 vendor markers
in code:

- `crates/browser/src/page.rs:1049-1057` — explicit header logger:
  `x-amzn-waf-action`, `x-datadome`, `x-wbaas-token` (THREE headers).
- `crates/browser/src/page.rs:2273-2293` — `v8_html_is_real` guard:
  `/ips.js`, `/149e9513-`, `kpsdk`, `_abck`, `bm_sz`,
  `captcha-delivery.com`, `dd-script`, `dd_engagement`,
  `/cdn-cgi/challenge-platform/` (NINE body substrings).
- `crates/browser/src/classify.rs:81-156` — the marker tables
  (UNAMBIGUOUS, PHRASE, SMALL_BODY, AKAMAI_CHALLENGE_COSIGNAL,
  INTERACTIVE_CAPTCHA_COSIGNAL).

The vendor list in §2 is roughly **3× larger**. Spec out the extensions:

### 4.1 Extend the header logger at `page.rs:1049-1057`

Add the following to the `if let Some(...)` / `contains_key(...)` chain:

```rust
if let Some(v) = resp.headers.get("cf-mitigated") {
    eprintln!("[vendor-detect] cloudflare-mitigated {} on {}", v, resp.url);
}
if let Some(v) = resp.headers.get("x-iinfo") {
    eprintln!("[vendor-detect] imperva-incapsula {} on {}", v, resp.url);
}
if resp.headers.get("x-cdn").map(|v| v.to_ascii_lowercase().contains("imperva")).unwrap_or(false) {
    eprintln!("[vendor-detect] imperva-cdn on {}", resp.url);
}
if let Some(v) = resp.headers.get("x-perimeterx-id") {
    eprintln!("[vendor-detect] perimeterx {} on {}", v, resp.url);
}
if let Some(v) = resp.headers.get("x-sucuri-id") {
    eprintln!("[vendor-detect] sucuri {} on {}", v, resp.url);
}
if let Some(v) = resp.headers.get("x-akamai-transformed") {
    eprintln!("[vendor-detect] akamai-edge {} on {}", v, resp.url);
}
if resp.headers.iter().any(|(k, _)| k.to_ascii_lowercase().starts_with("x-kpsdk")) {
    eprintln!("[vendor-detect] kasada (x-kpsdk-*) on {}", resp.url);
}
if let Some(v) = resp.headers.get("x-armor-shield-zone") {
    eprintln!("[vendor-detect] reblaze {} on {}", v, resp.url);
}
```

Detection-only. No flow change. Pure observation.

### 4.2 Extend `v8_html_is_real` at `page.rs:2273-2293`

The current guard misses the following vendors that — by §1.3 above —
indicate a re-served challenge document and must NOT be accepted as
"real content":

```rust
// Already covered: /ips.js, /149e9513-, kpsdk, _abck, bm_sz,
// captcha-delivery.com, dd-script, dd_engagement,
// /cdn-cgi/challenge-platform/
// Add:
    && !v8_html.contains("AwsWafIntegration")
    && !v8_html.contains("gokuProps")
    && !v8_html.contains("_Incapsula_Resource")
    && !v8_html.contains("visid_incap")
    && !v8_html.contains("reese84")
    && !v8_html.contains("_px3")
    && !v8_html.contains("_pxhd")
    && !v8_html.contains("px-captcha")
    && !v8_html.contains("press &amp; hold")
    && !v8_html.contains("sucuri_cloudproxy_js")
    && !v8_html.contains("Incapsula incident ID")
```

The matching pattern is intentional: each vendor's "challenge document is
still in effect" tell is the same string the classifier already keys on
(`classify.rs:81-156`), so the v8_html_is_real guard stays in sync with
the verdict logic.

### 4.3 Generic `is_challenge_document_response` function (per `07_DATADOME_PRIMITIVES.md §Primitive 1`)

The most leverage comes from the generic engine-side detector that
covers ALL vendors at once:

```rust
// In crates/browser/src/classify.rs (new section)
pub fn is_challenge_document_response(
    status: u16,
    headers: &[(String, String)],
    body: &str,
) -> bool {
    // Decision-tree from §1.5 of 18_ANTI_BOT_VENDOR_COOKBOOK.md
    //
    // Small body + 4xx/5xx + any vendor signaller header
    let small = body.len() < INTERSTITIAL_MAX_BYTES;
    let bad_status = matches!(status, 403 | 429 | 498 | 503);
    let has_vendor_header = headers.iter().any(|(k, _)| {
        let lk = k.to_ascii_lowercase();
        lk == "x-amzn-waf-action"
            || lk == "x-datadome"
            || lk == "cf-mitigated"
            || lk == "x-wbaas-token"
            || lk == "x-perimeterx-id"
            || lk == "x-iinfo"
            || lk == "x-sucuri-id"
            || lk == "x-armor-shield-zone"
            || lk.starts_with("x-kpsdk")
    });
    if (small && bad_status) || has_vendor_header { return true; }

    // Body-shape signals (matches the classifier marker tables)
    let lower = body.to_ascii_lowercase();
    body.contains("AwsWafIntegration")
        || body.contains("gokuProps")
        || lower.contains("captcha-delivery.com")
        || body.contains("/cdn-cgi/challenge-platform/")
        || body.contains("_cf_chl_opt")
        || body.contains("/ips.js")
        || body.contains("_kpsdk")
        || (small && (lower.contains("_abck") || lower.contains("bm_sz")))
        || body.contains("_Incapsula_Resource")
        || body.contains("px-captcha")
        || lower.contains("press &amp; hold")
        || body.contains("sucuri_cloudproxy_js")
}
```

This function is then consumed by `07_DATADOME_PRIMITIVES.md §Primitives
1/2/3`: it gates CSP relaxation, the rematerialize-iframes poll, and the
cookie-delta retry path — uniformly across all 13 vendors above.

### 4.4 Acceptance for §4

After landing 4.1+4.2+4.3:

- The 126-site sweep MUST not regress (per `14_TESTING_VALIDATION.md`
  drift gate of ±5 sites).
- `[vendor-detect]` log lines now appear for ≥ 8 vendor classes (vs 3
  today), enabling post-sweep analysis to split CHL outcomes by vendor.
- The `is_challenge_document_response` predicate enables the
  `07_DATADOME_PRIMITIVES.md` primitives without per-vendor naming in
  the public engine.

---

## 5. Vendor protocol-change tracking

Anti-bot vendors rotate probes on cadences ranging from monthly to
quarterly. The empirical observations from our internal capture history:

| Vendor | Rotation cadence | What rotates |
|---|---|---|
| AWS WAF | ~monthly | challenge.js obfuscation layer (string-array seed, identifier mangling). PoW shape stable. |
| DataDome | ~daily | The `dd-script.js` WASM blob key + the iframe URL's path segment |
| Akamai BMP | ~weekly (per-tenant) | sensor_data PRNG seed + the per-script file hash |
| Cloudflare | ~weekly | The orchestrator JS at `/cdn-cgi/challenge-platform/h/b/jsd/...` URL |
| Kasada | ~quarterly | The `ips.js` bytecode + opcode table; the underlying VM grammar is stable |
| PerimeterX | ~weekly | Sensor JS obfuscation + the collector endpoint path |
| Imperva | ~monthly | `_Incapsula_Resource` script obfuscation |

### 5.1 Recommended cadence

**Quarterly** is the right baseline for full re-captures + diffs against
the prior snapshot, because:
- Faster cadence (monthly) wastes effort on noise — the marker shape
  rarely changes within a vendor's obfuscation rotation; only the keys
  shift.
- Slower (semi-annual) misses the structural changes (e.g. AWS WAF
  adding a new PoW flavor) that DO matter.

Per `14_TESTING_VALIDATION.md §L5` — the 3-run aggregated sweep nightly
already catches *symptomatic* drift (pass rate dropping > 3 sites for >
3 consecutive nights). The quarterly capture cycle is the
*diagnostic* follow-up: when drift hits, the captures tell you which
vendor rotated and what changed.

### 5.2 Quarterly checklist

```bash
QUARTER=$(date +%Y_Q$((($(date +%-m)-1)/3+1)))
mkdir -p docs/research_$QUARTER/vendor_captures

# Capture each major vendor's challenge stub + dependent JS
for site in amazon-de etsy hyatt zillow www.example-imperva-tenant.com; do
  curl -sS -A 'Mozilla/5.0 (Macintosh; …) Chrome/148.0.0.0' \
       "https://$site/" -o "docs/research_$QUARTER/vendor_captures/$site.html"
done

# Hash + diff against prior quarter
for f in docs/research_$QUARTER/vendor_captures/*.html; do
  PRIOR=$(echo "$f" | sed "s/$QUARTER/$PREV_QUARTER/")
  if [ -f "$PRIOR" ]; then
    echo "=== $(basename $f) ==="
    diff <(sha256sum "$PRIOR" | cut -c1-12) <(sha256sum "$f" | cut -c1-12) || \
      echo "  ROTATED — re-deobfuscate"
  fi
done
```

Per-vendor follow-up tasks when rotation detected:

| Vendor | Action |
|---|---|
| AWS WAF | Re-run §1 capture recipe of `06_AWS_WAF_SOLVER.md`; re-deobfuscate; diff fingerprint-collector identifier mangling. |
| DataDome | Re-pull `dd-script.js`; the WASM blob is daily — only the keys rotate. The script wrapper is rarely structural. |
| Akamai | Re-run sensor_data capture; the per-script file hash + PRNG seed move; the field set is stable per BMP major version. |
| Cloudflare | Re-pull the orchestrator JS at `/cdn-cgi/challenge-platform/h/b/jsd/...`; Cloudflare publishes some details in their blog when major changes ship. |
| Kasada | Re-run the K2-DIFF tool (per `08_KASADA_FRONTIER.md §Lever 1`); the new opcode table needs decoding. |
| PerimeterX | Re-run the [`MiddleSchoolStudent/PerimeterX-solver`](https://github.com/MiddleSchoolStudent/PerimeterX-solver) deobfuscator (or its successor) — its update cadence tracks PX rotations. |
| Imperva | Re-pull `/_Incapsula_Resource?...`; sensor field set is stable. |

### 5.3 Drift signals

The nightly sweep is the early-warning. Three lights:

1. **Green** — pass rate within ±3 sites of trailing-7-night median, no
   per-vendor concentration (drift evenly distributed across the corpus).
2. **Yellow** — pass rate drop of 4-8 sites concentrated in one vendor's
   cluster (e.g. all 4 BO chrome amazon variants suddenly 2011 B). Likely a
   single AWS WAF rotation; capture + diff at the next slot.
3. **Red** — pass rate drop > 8 sites, OR cross-vendor (multiple vendor
   classes regressing simultaneously). Likely an engine regression, NOT
   a vendor rotation. Bisect against the last green build.

Cross-link to `14_TESTING_VALIDATION.md §L5` for the alerting wiring.

---

## 6. References

### 6.1 Vendor official documentation

- **AWS WAF** — [Using the intelligent threat JavaScript API](https://docs.aws.amazon.com/waf/latest/developerguide/waf-js-challenge-api.html), [Intelligent threat API specification](https://docs.aws.amazon.com/waf/latest/developerguide/waf-js-challenge-api-specification.html), [How to use `getToken`](https://docs.aws.amazon.com/waf/latest/developerguide/waf-js-challenge-api-get-token.html), [Protect against bots with AWS WAF Challenge and CAPTCHA actions](https://aws.amazon.com/blogs/networking-and-content-delivery/protect-against-bots-with-aws-waf-challenge-and-captcha-actions/).
- **Imperva** — [Advanced Bot Protection cookie scope](https://docs.imperva.com/bundle/advanced-bot-protection/page/75134.htm), [Thales Cookie Scope](https://docs-cybersec.thalesgroup.com/bundle/advanced-bot-protection/page/75136.htm), [How Incapsula Client Classification Challenges Bots](https://www.imperva.com/blog/archive/how-incapsula-client-classification-challenges-bots/).
- **HUMAN Security (PerimeterX)** — [Use of cookies & web storage](https://edocs.humansecurity.com/docs/cookies).
- **Cloudflare** — [Turnstile documentation](https://developers.cloudflare.com/turnstile/), [Bot Management](https://developers.cloudflare.com/bots/).
- **Kasada, DataDome, Akamai BMP, Reblaze** — no full public protocol documentation; their vendor pages describe product capabilities only.

### 6.2 Open-source solvers and reverse engineering

AWS WAF:
- [`xKiian/awswaf`](https://github.com/xKiian/awswaf)
- [`neiii/aws-waf-solver`](https://github.com/neiii/aws-waf-solver)
- [`jonathanyly/awswaf-solver-api`](https://github.com/jonathanyly/awswaf-solver-api)
- [`Switch3301/Aws-Waf-Solver`](https://github.com/Switch3301/Aws-Waf-Solver)
- [`aferapi/aws-waf-solver`](https://github.com/aferapi/aws-waf-solver)
- [`Needslq/aws-amazon-captcha-solving`](https://github.com/Needslq/aws-amazon-captcha-solving) (Captcha variant)
- [`luminati-io/AWS-WAF-captcha-solver`](https://github.com/luminati-io/AWS-WAF-captcha-solver)

DataDome:
- [`glizzykingdreko/Datadome-Captcha-Deobfuscator`](https://github.com/glizzykingdreko/Datadome-Captcha-Deobfuscator)
- [`glizzykingdreko/Datadome-GeeTest-Captcha-Solver`](https://github.com/glizzykingdreko/Datadome-GeeTest-Captcha-Solver)
- [`recaptchaUser/datadome-Interstitial-solver`](https://github.com/recaptchaUser/datadome-Interstitial-solver)
- [`recaptchaUser/vinted-datadome-solver`](https://github.com/recaptchaUser/vinted-datadome-solver)
- [`luminati-io/datadome-captcha-solver`](https://github.com/luminati-io/datadome-captcha-solver)
- [`javapuppteernodejs/solving-datadome-captcha`](https://github.com/javapuppteernodejs/solving-datadome-captcha)
- [`campo1312/DataDome`](https://github.com/campo1312/DataDome)

Akamai:
- [`xiaoweigege/akamai2.0-sensor_data`](https://github.com/xiaoweigege/akamai2.0-sensor_data)
- [`i7solar/Akamai`](https://github.com/i7solar/Akamai)
- [`cirleamihai/akamai-1.7-cookie-generator`](https://github.com/cirleamihai/akamai-1.7-cookie-generator)
- [`FRIS-Solutions-Vault/akamai-sdk-go`](https://pkg.go.dev/github.com/FRIS-Solutions-Vault/akamai-sdk-go)

Kasada:
- [`0x6a69616e/kpsdk-solver`](https://github.com/0x6a69616e/kpsdk-solver)
- [`unicorn-aio/kpsdk`](https://github.com/unicorn-aio/kpsdk)
- [`lktop/kpsdk`](https://github.com/lktop/kpsdk)
- [`Mrclintons/kpsdk-1`](https://github.com/Mrclintons/kpsdk-1)
- [`nixbro/Kasada-Solver`](https://github.com/nixbro/Kasada-Solver)

PerimeterX:
- [`MiddleSchoolStudent/PerimeterX-solver`](https://github.com/MiddleSchoolStudent/PerimeterX-solver)
- [`PerimeterX/perimeterx-node-express`](https://github.com/PerimeterX/perimeterx-node-express) (vendor SDK)
- [`PerimeterX/perimeterx-python-wsgi`](https://github.com/PerimeterX/perimeterx-python-wsgi) (vendor SDK)

Cloudflare:
- [`vvanglro/cf-clearance`](https://github.com/vvanglro/cf-clearance)
- [`x404xx/Turnstile-Solver`](https://github.com/x404xx/Turnstile-Solver)
- [`cloudflare-turnstile-bypass` GitHub topic](https://github.com/topics/cloudflare-turnstile-bypass)
- Byparr / FlareSolverr (community-maintained reverse proxies)

Arkose Labs / FunCaptcha:
- [`decodecaptcha/FunCaptcha-Solver`](https://github.com/decodecaptcha/FunCaptcha-Solver)
- [`useragents/Funcaptcha-Audio-Solver`](https://github.com/useragents/Funcaptcha-Audio-Solver)
- [`Pr0t0ns/Funcaptcha-Solver`](https://github.com/Pr0t0ns/Funcaptcha-Solver)
- [`arkose` GitHub topic](https://github.com/topics/arkose)

Multi-vendor:
- [`Hyper-Solutions/hyper-sdk-js`](https://github.com/Hyper-Solutions/hyper-sdk-js) — commercial SDK covering Akamai + Incapsula + Kasada + DataDome
- [`solvercaptcha/solvecaptcha-python`](https://github.com/solvercaptcha/solvecaptcha-python) — multi-CAPTCHA service Python client

### 6.3 Independent walkthroughs and research

- glizzykingdreko — [Akamai v3 sensor deep dive](https://medium.com/@glizzykingdreko/akamai-v3-sensor-data-deep-dive-into-encryption-decryption-and-bypass-tools-da0adad2a784)
- glizzykingdreko — [Breaking Down Datadome Captcha WAF](https://medium.com/@glizzykingdreko/breaking-down-datadome-captcha-waf-d7b68cef3e21)
- thewebscrapingclub — [The Lab #56: Bypassing PerimeterX 3](https://substack.thewebscraping.club/p/the-lab-56-bypassing-perimeterx-3)
- Scrapfly bypass guides — [Akamai](https://scrapfly.io/bypass/akamai), [PerimeterX](https://scrapfly.io/bypass/perimeterx), [Imperva](https://scrapfly.io/blog/posts/how-to-bypass-imperva-incapsula-anti-scraping), [Cloudflare Turnstile](https://scrapfly.io/blog/posts/how-to-bypass-cloudflare-turnstile)
- ZenRows guides — [Akamai](https://www.zenrows.com/blog/bypass-akamai), [PerimeterX](https://www.zenrows.com/blog/perimeterx-bypass), [Incapsula](https://www.zenrows.com/blog/incapsula-bypass), [Cloudflare](https://www.zenrows.com/blog/bypass-cloudflare)
- CapSolver guides — [AWS WAF ranking 2026](https://www.capsolver.com/blog/aws-waf/top-aws-solver-ranking), [DataDome guide](https://docs.capsolver.com/en/guide/captcha/datadome/), [Cloudflare 2026](https://www.capsolver.com/blog/Cloudflare/solve-cloudflare-in-2026)
- RoundProxies — [AWS WAF bypass 2026](https://roundproxies.com/blog/bypass-aws-waf/), [PerimeterX 2026](https://roundproxies.com/blog/bypass-perimeterx/), [Imperva 2026](https://roundproxies.com/blog/bypass-imperva-incapsula/)
- scrapebadger — [Imperva reese84](https://scrapebadger.com/imperva-bypass)

### 6.4 Bot-detection research foundations

- [creepjs](https://abrahamjuliot.github.io/creepjs/) — the canonical browser-fingerprinting probe; reading what creepjs collects gives the cross-vendor signal set
- [fingerprintjs](https://github.com/fingerprintjs/fingerprintjs) — open-source fingerprinting library; their blog posts describe many of the signals modern WAFs collect
- AmIUnique research project
- The Browser Fingerprinting series at [`brave.com/privacy-updates/`](https://brave.com/privacy-updates/)

### 6.5 Internal cross-references

Sibling docs in this release plan:
- `02_GAP_ANALYSIS.md` — per-site root-cause for the 10 recoverable + 8 hard-residual sites
- `04_TOOLING_SPEC.md` — capture-mode tooling (the `--capture <name>` flag)
- `05_SPA_HYDRATION_CLUSTER.md` — reddit / duolingo / booking / douyin
- `06_AWS_WAF_SOLVER.md` — AWS WAF deep dive (capture recipe, fingerprint table, solver alternatives)
- `07_DATADOME_PRIMITIVES.md` — engine primitives for any vendor following the canonical interstitial + iframe + cookie pattern
- `08_KASADA_FRONTIER.md` — Kasada research arc (open frontier)
- `11_PER_PROFILE_STRATEGY.md` — which BO profile catches which vendor class
- `12_COMPETITIVE_LANDSCAPE.md` — Camoufox / Playwright / Patchright per-vendor coverage
- `13_FILE_LOCATIONS_INDEX.md` — file:line lookup for everything
- `14_TESTING_VALIDATION.md` — drift detection cadence (§L5 nightly sweep)
- `15_OPEN_QUESTIONS.md` — research backlog

Engine source-of-truth:
- `crates/browser/src/page.rs:1049-1057` — vendor-detect header logger (3 vendors today; §4.1 extends to 11)
- `crates/browser/src/page.rs:2273-2293` — `v8_html_is_real` body-marker guard (9 markers today; §4.2 extends to ~20)
- `crates/browser/src/classify.rs:81-156` — canonical marker tables (UNAMBIGUOUS, PHRASE, SMALL_BODY, AKAMAI_CHALLENGE_COSIGNAL, INTERACTIVE_CAPTCHA_COSIGNAL)
- `crates/browser/src/challenge.rs:55-161` — `ChallengeSolver` trait surface (the seam private vendor_solvers binds to)
- `crates/browser/tests/holistic_sweep.rs` — the 126-site corpus
- `crates/browser/src/classify.rs:247-251` — `is_cf_challenge_doc` (the pattern `is_challenge_document_response` should follow per §4.3)

Memory (auto-context, persistent across sessions):
- `state_2026_05_16_phase5_datadome.md` — pre-strip DataDome solution arc
- `state_2026_05_16_kasada_engine_gap_sharpened.md` — Kasada engine-addressable thesis
- `kasada_wrapper_cracked_and_remaining_leaks.md` — `/tl` POST wrapper (`xor(plaintext, b"omgtopkek")`) + 16 remaining error-bearing fields
- `kasada_real_blocker_css_calc_math.md` — CSS Values 4 math function probe

Workspace constraints (`CLAUDE.md`):
- Per-vendor solver code stays in private `vendor_solvers`; public engine carries only generic primitives + detection-only markers.
- Licensing: only MIT / Apache-2.0 in the public tree. The third-party reverse-eng repos cited here are research references — DO NOT copy their code into public crates.
- Reading their protocol descriptions to understand the wire format is fine; lifting code is not.
