# Cloudflare Bypass — Research Report (2026-05-10)

> Research-only doc. Triggered by the 2026-05-10 holistic sweep
> (`docs/HOLISTIC_TEST_2026_05_10/SUMMARY.md`) which logged exactly one
> `Cloudflare-CHL` failure: `udemy.com` returned a 476 KB Cloudflare
> challenge body. browser_oxide currently ships zero Cloudflare-specific
> reverse-engineering — no `crates/stealth/src/cloudflare.rs`, no analog
> to the existing `kasada.rs` PoW solver. This document maps the
> protection surface, the public RE landscape, and a concrete remediation
> plan.
>
> Author: research agent. Date: 2026-05-10.

---

## 0. TL;DR

1. udemy.com's challenge is **not** the legacy IUAM JS-math challenge.
   The live response on 2026-05-10 carries `cf-mitigated: challenge`,
   `cType: 'managed'` in the inline `_cf_chl_opt` blob, and a CSP that
   permits `https://challenges.cloudflare.com`. This is a **Managed
   Challenge** with an embedded Turnstile-class signal collector.
2. The Cloudflare Managed Challenge superset has effectively replaced
   IUAM in 2024–2025; "Just a moment…" is now a Managed Challenge
   wrapper, not the historic 5-second JS PoW. The page still says "Just
   a moment…" but the orchestrator under it is the modern challenge
   platform (`/cdn-cgi/challenge-platform/h/{b,g}/orchestrate/...`).
3. The first defensive layer that browser_oxide must fix is **TLS/H2
   fingerprint parity** (we already have rquest+BoringSSL + the recent
   Chrome 147 work — that's the best lever). The second layer is
   **executing the orchestrator JS to completion** (sandboxed VM +
   browser-grade signal answers); the third is **Turnstile token
   issuance** (no public open-source solver — this tier requires either
   a real headed Chromium, an antidetect Firefox build like Camoufox,
   or a paid captcha service).
4. For udemy specifically, multiple public scraping write-ups rate it
   "2/5 difficulty" — meaning a fully fingerprint-correct browser
   (correct TLS, no `navigator.webdriver`, no `Runtime.enable` leak,
   real Chrome UA-CH set) typically gets through without solving the
   PoW or the Turnstile widget at all, because the threat score never
   tips into the "must serve challenge" bucket. The fact that
   browser_oxide gets the full challenge page means we're already
   tripping the threat-score escalation.
5. **Recommended scope (V1):** ship `crates/stealth/src/cloudflare.rs`
   that (a) detects the challenge state, (b) records `_cf_chl_opt`
   contents, (c) wires CSP/CORS exceptions for
   `challenges.cloudflare.com`, and (d) executes the orchestrator JS
   inside our V8 with our real DOM/TLS — so the challenge grades us as
   a real browser. **Do not** attempt a homemade IUAM PoW solver in
   V1: the algorithm is dynamic per-Ray-ID and the obfuscation rotates.

---

## 1. Cloudflare protection-product taxonomy (2025–2026)

Cloudflare's challenge surface area in 2026 is a layered stack on top
of the **Bot Management** scoring engine. The key insight: every visible
"challenge" is just a UI/PoW shell that gets selected by an upstream
scoring decision. The score is computed from network-layer signals
(JA3/JA4, H2 SETTINGS, IP reputation) plus prior visit history; the
chosen challenge type is then served by the **challenge platform**
(`/cdn-cgi/challenge-platform/h/{b,g}/...`).

### 1.1 Bot Fight Mode (BFM) — Free plan

- **What it does:** A binary on/off setting that issues challenges to
  requests Cloudflare's edge labels "definitely automated" (bot score
  ≤ ~30 in the underlying engine, though Free plan customers can't see
  the score).
- **Tunability:** None. Cannot be customized via WAF custom rules.
- **Failure mode for scrapers:** Pretty aggressive against any
  obviously-non-browser TLS fingerprint (Python `requests`, Go
  `net/http`, etc.).
- Source: https://developers.cloudflare.com/bots/get-started/bot-fight-mode/

### 1.2 Super Bot Fight Mode (SBFM) — Pro/Business

- Adds configurable per-category actions (search-engine bots, scrapers,
  ML-flagged sophisticated bots).
- Pro tier challenges only "definitely automated"; Business tier also
  challenges "likely automated" (the ML-scored bucket).
- Source: https://blog.cloudflare.com/super-bot-fight-mode/

### 1.3 Bot Management (BM) — Enterprise

- Per-request bot score 1–99 (1 = certainly bot, 99 = certainly human).
- Customer can write WAF custom rules keyed off `cf.bot_management.score`,
  `cf.bot_management.ja3_hash`, `cf.bot_management.ja4`, etc.
- This is the tier where JA3/JA4 and the JA4 Signals array
  (`browser_ratio_1h`, `uas_rank_1h`, `paths_rank_1h`, etc.) are exposed
  to the customer.
- Source: https://developers.cloudflare.com/bots/additional-configurations/ja3-ja4-fingerprint/

### 1.4 The four challenge UIs

The customer (or BM auto-pick) chooses one of four actions when the
score crosses the threshold:

| Action | What the visitor sees | Underlying mechanism |
| --- | --- | --- |
| **JS Challenge** | "Checking your browser…" briefly. | Static client-side JS executes a small computation and POSTs back. Lower friction than IUAM. |
| **Managed Challenge** (recommended by CF) | Either invisible orchestrator only, or a Turnstile widget click, depending on score. | The `/cdn-cgi/challenge-platform/h/{b,g}/orchestrate/managed/v1` endpoint adaptively picks. This is what udemy serves. |
| **Interactive Challenge** | Always shows a click box. | Turnstile widget in visible Managed mode, with mandatory user gesture. |
| **(Legacy) IUAM** | The historic "Just a moment…" 5-second hold. | `cf-chl-bypass` page with the SHA-256 PoW. **In 2025 this name is mostly aliased to Managed Challenge** — Cloudflare has been retiring the standalone IUAM UI. The HTML `<title>Just a moment…</title>` is now reused by Managed Challenge pages. |

### 1.5 When does CF serve which?

- Score > ~70 (configurable): pass straight through, no challenge.
- Score 30–70: Managed Challenge (often resolves invisibly via Privacy
  Pass token if the visitor has one; otherwise serves a Turnstile
  widget).
- Score < 30: Interactive Challenge, or block depending on rule.
- The exact thresholds are customer-tuned via WAF rules; the above is
  default behavior cited by multiple bypass guides
  (https://www.zenrows.com/blog/bypass-cloudflare,
  https://scrapfly.io/blog/posts/how-to-bypass-cloudflare-anti-scraping).

### 1.6 Challenge-state detection (engine-side)

Cloudflare publishes the canonical detection signal:
**`cf-mitigated: challenge`** is set on every response that intercepted
the request and served a challenge instead of origin content. The header
has only one valid value, "challenge" (no separate "block" / "captcha"
values).
Source: https://developers.cloudflare.com/cloudflare-challenges/challenge-types/challenge-pages/detect-response/

Body-level corroborating signals (in case `cf-mitigated` were ever
stripped by a downstream proxy):
- `<title>Just a moment…</title>` (universal across CF challenge UIs).
- Inline `<script>` containing `window._cf_chl_opt = { … cType: '…' }`.
- CSP includes `script-src … https://challenges.cloudflare.com`.
- `cf-ray: <id>-<colo>` header (always present on CF, but combined
  with the above it's diagnostic).
- `server: cloudflare`.
- The challenge platform script tag has `src` matching
  `/cdn-cgi/challenge-platform/h/[bg]/orchestrate/(chl_page|jsch|managed|captcha|flow)/v[12]`.

The `cType` value is the most useful internal triage signal:
- `cType: 'managed'` → Managed Challenge (udemy's case 2026-05-10).
- `cType: 'non-interactive'` → Turnstile non-interactive widget.
- `cType: 'interactive'` → Turnstile interactive widget.
- `cType: 'jsch'` → legacy JS Challenge.

---

## 2. The challenge math: IUAM PoW + the orchestrator flow

### 2.1 The historical IUAM JS challenge (still informative)

The clearest public RE write-up is by Noah Riveiro
(https://blog.noah.ovh/cloudflare-js-challenge-1/). The IUAM challenge
was a 3-phase script. browser_oxide does not need to reimplement this
verbatim because (per §1.4) the standalone IUAM action is being phased
out in favor of Managed Challenge — but the *PoW algorithm* in Phase 3
is the same kernel still in use today, only re-shelled.

**Phase 0 (configuration).** The HTML serves a single inline script:
```js
window._cf_chl_opt = {
    cFPWv: 'g',           // platform variant ('b' or 'g')
    cH:    '<base64-blob>',
    cITimeS: '<unix-ts>', // initial timestamp the server thinks it is
    cN:    '<csp-nonce>',
    cRay:  '<cf-ray-id>',
    cType: 'managed' | 'jsch' | 'non-interactive' | …,
    cZone: '<host>',
    fa:    '<full-action-token>',
    md:    '<encoded-metadata>',
    mdrd:  '<metadata-redirect>',
    cUPMDTk:'<update-metadata-token>',
};
// Then:
var a = document.createElement('script');
a.nonce = '<csp-nonce>';
a.src   = '/cdn-cgi/challenge-platform/h/g/orchestrate/chl_page/v1?ray=<rayId>';
document.head.appendChild(a);
```
The orchestrator fetches the second-stage payload using the Ray ID as
a dynamic decryption key. Per multiple sources, **the encryption keys
rotate every ~30 minutes**, which is why static deobfuscation is
worthless — the deobfuscator has to run live.

**Phase 1 (validation + signal collection).**
- Cookies enabled? IE rejected? URL matches?
- Listens for keyboard/mouse/touch until ~25 events fire.
- Compresses the collected context with a custom LZW.
- POST to `/cdn-cgi/challenge-platform/flow/ov1/` with body
  `v_<rayId>=<lzw-compressed-context>` and header `cf-challenge`.

**Phase 2 (browser fingerprinting).**
- "JSFuck match challenge" — a tiny eval-style probe.
- Image dimensions of seeded image elements.
- HTTP status codes of probed sub-resources.
- `navigator.plugins`.
- Hardcoded fingerprint accumulator into `_cf_chl_ctx[chC]`.

**Phase 3 (proof-of-work).** Server delivers a JS payload that runs:
```text
workers   = ceil(navigator.hardwareConcurrency * 0.75 || 6)
seed      = e=<unix-ms>&d=<difficulty>&n=<nonce-0..9999><iv>
solve     = find counter such that SHA-256(seed||counter) prefix matches
            <difficulty> hex chars
on solve  → POST modified ctx to clearance endpoint → cf_clearance issued
```
This is the kernel that is morally identical to Kasada's `ovida(...)` we
already solve in `crates/stealth/src/kasada.rs`. SHA-256 + difficulty
prefix matching, just with different framing.
Source: https://blog.noah.ovh/cloudflare-js-challenge-1/

### 2.2 Modern Managed-Challenge orchestrator (2024–2026)

Public RE notes (https://github.com/scaredos/cfresearch and
https://github.com/VeNoMouS/cloudscraper/issues/298) describe the
modern flow as:

1. `GET /cdn-cgi/challenge-platform/h/{b,g}/orchestrate/managed/v1?ray=…`
   — returns the orchestrator JS bundle (heavily obfuscated, AST-mangled,
   string-array-encoded).
2. The bundle invokes signal-collection submodules at endpoints like
   `/cdn-cgi/challenge-platform/h/g/jsd/r/<dynamic-id>/<rayId>` (note:
   the `jsd` path segment is new — it's the JavaScript-detection
   subsystem).
3. Final clearance POST to a per-Ray-ID endpoint with payload fields
   `md`, `sh`, `aw` — these are the encrypted bundles of collected
   browser metadata, signed-hash, and answer-words. Body is
   `text/plain;charset=UTF-8` carrying base64 / opaque blob.
4. If signals + answers grade the visitor as human, response sets
   `cf_clearance` and either redirects back to origin or 302s to the
   originally-requested URL with the cookie attached.

### 2.3 cf_clearance cookie binding

The `cf_clearance` cookie is **not** transferable. From Cloudflare's own
docs (https://developers.cloudflare.com/cloudflare-challenges/concepts/clearance/):

- "Securely tied to the specific visitor and device it was issued to."
- Bound to (at minimum): IP address, User-Agent, TLS fingerprint, and
  the broader request fingerprint at issuance time.
- Lifetime is the zone's "Challenge Passage" setting (default 30 min,
  often configured 30–60 min, can be up to 1 year on Enterprise).
- The Turnstile widget has an opt-in **pre-clearance** mode where the
  widget itself issues a `cf_clearance` cookie alongside its token —
  this is the integration path most modern customers use, because it
  avoids the visible "Just a moment" interstitial entirely.

Practical implications for browser_oxide:
- We **cannot** fetch a `cf_clearance` from one process/session and
  reuse it in another with different IP or TLS fingerprint. Per
  https://www.zenrows.com/blog/cf-clearance the cookie "lets you
  access protected content for 30-60 minutes, but you must maintain
  the same User-Agent and IP address that generated it."
- Therefore the cookie must be acquired **inside our engine's own
  session** (same TCP origin, same TLS, same UA, same H2 SETTINGS) —
  the cookie is just storage; the *binding* is the network identity
  it was issued to.

---

## 3. Turnstile architecture

### 3.1 Modes

Turnstile is a separately-marketed widget that also under-pins Managed
Challenge. Three modes
(https://developers.cloudflare.com/turnstile/concepts/widget/):

- **Managed (default, recommended):** invisible by default; widget
  decides per-visitor whether to show an interaction prompt based on
  collected signals.
- **Non-Interactive:** always visibly renders, but completes without
  user input.
- **Invisible:** hidden completely; runs verification in the background.

### 3.2 Endpoints + tokens

- Widget script:
  `https://challenges.cloudflare.com/turnstile/v0/api.js`.
- The widget POSTs collected signals to `https://challenges.cloudflare.com/turnstile/v0/...`
  endpoints (the exact paths are obfuscated and rotate).
- On success, widget produces a **clearance token** (string), which is
  injected into the form as `<input name="cf-turnstile-response">` (or
  exposed via `turnstile.getResponse(widgetId)` in JS).
- Server validates by POSTing the token to the Siteverify API at
  `https://challenges.cloudflare.com/turnstile/v0/siteverify` with the
  site's secret key. Siteverify is one-shot: each token is consumable
  exactly once and expires within ~5 minutes.

### 3.3 Privacy Pass / blind-RSA token integration

This is the most interesting part for an antibot perspective:

- Cloudflare deployed a **Turnstile attester** for Privacy Pass
  (https://github.com/cloudflare/privacypass-attester). When a visitor
  successfully solves Turnstile, the attester signs a blind-RSA token
  (RFC 9474) that the visitor's browser stores.
- On future visits to *any* Cloudflare-protected site, the browser can
  unblindly redeem these tokens to skip the challenge — the verifier
  knows "this token came from someone who recently solved Turnstile"
  but cannot link the token back to *which* solve.
- Cloudflare's own Privacy Pass browser extension
  (https://github.com/cloudflare/pp-browser-extension) implements the
  client side; Safari ships a native Privacy Pass implementation that
  Apple operates as the issuer.
- This means a **legitimate Chrome on Linux** has *no* Privacy Pass
  tokens by default and so will solve at least one Turnstile per
  Challenge Passage window. An automated browser without Privacy Pass
  support is treated identically to a "no-token user" — so we don't
  need to fake Privacy Pass; we need to be a credible "Chrome on Linux
  visiting fresh".
- The fact that Privacy Pass uses RSA blind signatures means we
  *cannot* mint our own tokens; the issuer key is held by Cloudflare.

Sources:
- https://blog.cloudflare.com/privacy-pass-standard/
- https://blog.cloudflare.com/privacy-pass-the-math/
- https://deepwiki.com/cloudflare/pp-browser-extension/1.1-privacy-pass-protocol

### 3.4 Signals Turnstile collects (per Scrapfly's analysis)

Three buckets (https://scrapfly.io/blog/posts/how-to-bypass-cloudflare-turnstile):

1. **Browser environment:** Canvas fingerprinting, WebGL renderer
   string + parameters, presence/absence of specific Web APIs (e.g.
   `SharedArrayBuffer`, `Notification`, `Bluetooth`), UA-CH high-entropy
   values (`Sec-CH-UA-Full-Version-List`, `Sec-CH-UA-Bitness`, etc. —
   note that udemy's `accept-ch` and `critical-ch` headers explicitly
   request these).
2. **Behavioral patterns:** Mouse movement entropy, keystroke dynamics,
   timing of page interactions, scroll velocity.
3. **Network signals:** IP ASN and reputation, JA3/JA4 TLS fingerprint
   match against known browser corpus, HTTP/2 SETTINGS+priority frame
   fingerprint match.

---

## 4. Detection signals — which are hardest to fake

Ranked by *engine effort to fake correctly* (10 = hardest):

| Signal | Effort | Notes |
| --- | --- | --- |
| **JA3/JA4 TLS fingerprint** | 4 | rquest+BoringSSL solves this. browser_oxide already has Chrome 147 parity here. Easy if you control the TLS stack. |
| **HTTP/2 SETTINGS frame + priority + pseudo-header order** | 6 | Akamai-style passive H2 fingerprint. Format `[SETTINGS]\|WINDOW_UPDATE\|PRIORITY\|Pseudo-Header-Order\|HEADERS_FRAME\|WINDOW_UPDATE*`. Example Chrome value: `1:65536;4:131072;5:16384\|12517377\|3:0:0:201\|m,p,a,s`. Requires custom h2 client config — rquest has knobs for this. |
| **ALPN order** | 2 | `h2,http/1.1`. Trivial. |
| **Header order + casing** | 3 | rquest preserves order; we just need to emit the Chrome canonical sequence. |
| **`sec-ch-ua-*` UA-CH values matching critical-ch demands** | 3 | udemy's `critical-ch` lists ~14 hint headers. We must answer all of them with values consistent with our advertised User-Agent. |
| **`navigator.webdriver === false`** | 1 | Trivial JS shim. |
| **Absence of `Runtime.enable` CDP leak** | 5 | The classic CDP detection (console.debug serializing custom getters) was patched in V8 in May 2025 (https://blog.castle.io/why-a-classic-cdp-bot-detection-signal-suddenly-stopped-working-and-nobody-noticed/). browser_oxide uses deno_core/V8 directly without CDP, so this signal is N/A for us — *advantage*. |
| **Iframe-bridge timing** | 7 | The Turnstile widget creates an iframe to `challenges.cloudflare.com`; the round-trip latency between inner-iframe scripts and outer-page handlers is measured. Has to feel like real cross-origin postMessage on a real network. |
| **Canvas fingerprint stable + matches GPU vendor** | 7 | Per kasada/akamai work in our repo, canvas fp is high-entropy. browser_oxide has GPU/canvas work in `crates/stealth/src/gpu.rs`; need to verify it produces values that don't show up in CF's known-bad-list. |
| **WebGL renderer string consistent with hardware claim** | 6 | `UNMASKED_VENDOR_WEBGL` / `UNMASKED_RENDERER_WEBGL` must match a real Chrome+driver combo. |
| **Audio fingerprint (AnalyserNode quantization)** | 7 | Adidas (akamai) probe shows audio is in active use; CF reportedly samples it too [hypothesis]. |
| **Behavioral entropy (mouse, scroll, click timing)** | 9 | Hardest to do well. Real humans have noisy, cubic-spline-ish trajectories with pauses; bots have linear or perfectly-Bezier trajectories. Our existing `crates/stealth/src/behavior.rs` is a good starting point. |
| **Privacy Pass token presence** | n/a | We cannot mint these. Absence is normal for a fresh Linux Chrome session. |
| **IP reputation (ASN, datacenter / residential, prior abuse)** | 10 | Out of scope for engine work — needs proxy infrastructure. |

The ranking matters because it tells us **the order to fix things**.
For udemy specifically, *we must already be passing* TLS+H2+UA-CH
(browser_oxide claims Chrome 147 parity), so the failure is most likely
in a layer above: either (a) the orchestrator script cannot run to
completion in our DOM (CSP violation, missing API, broken postMessage),
or (b) some collected signal (canvas, WebGL, audio) shows a
non-Chrome-on-Linux-laptop value that tips the threat score.

---

## 5. Public RE projects to study (2024–2026)

### 5.1 VeNoMouS/cloudscraper
- URL: https://github.com/VeNoMouS/cloudscraper
- Approach: Python module, Requests-based session wrapper. Detects
  challenge HTML, executes the JS challenge using a JS interpreter
  (default `js2py`, optional Node/V8/ChakraCore), submits the answer to
  get `cf_clearance`. v3 (2025) added "JavaScript VM-based challenge"
  support and a `captcha` provider hook (2captcha, anticaptcha,
  capsolver, etc.) for Turnstile.
- What it doesn't solve: TLS fingerprint mismatch (it uses `requests`,
  whose JA3 is unmistakably Python). Modern Managed Challenge
  obfuscation defeats its JS interpreter for many sites. Maintenance
  has been intermittent — the substack writeup
  (https://substack.thewebscraping.club/p/how-to-bypass-cloudflare-turnstile)
  flatly says "the repository has not been updated for two years" as
  of 2025.

### 5.2 FlareSolverr
- URL: https://github.com/FlareSolverr/FlareSolverr
- Approach: Spawns a real undetected-chromedriver instance, navigates
  to the protected URL, lets the real browser solve the challenge,
  scrapes the resulting cookies and returns them via an HTTP API. Used
  as a sidecar by Prowlarr / Sonarr / Radarr.
- What it doesn't solve: Turnstile interactive challenges — when CF
  escalates to a captcha, FlareSolverr can't solve it (issue #1286,
  many issues in the 1500–1600 range). Average reported success rate
  ~92.5% across tested CF sites.

### 5.3 ultrafunkamsterdam/undetected-chromedriver
- URL: https://github.com/ultrafunkamsterdam/undetected-chromedriver
- Approach: Patches Selenium ChromeDriver binary to remove `cdc_*`
  variables, patches Chrome flags (e.g. `--enable-automation` removed),
  patches the JS surface (`navigator.webdriver = false`,
  `window.chrome` populated, plugin list faked).
- What it doesn't solve: Doesn't fix CDP signals from `Runtime.enable`
  (Patchright fixes that — see below). Has lost effectiveness against
  CF Turnstile rolled out Feb 2025.

### 5.4 Kaliiiiiiiiii-Vinyzu/patchright
- URL: https://github.com/Kaliiiiiiiiii-Vinyzu/patchright
- Approach: Source-level patch of Playwright that avoids
  `Runtime.enable` entirely (uses isolated `ExecutionContexts` instead),
  closes the CDP leak. Written for both Python and Node.
- What it doesn't solve: TLS / H2 fingerprint mismatches that occur
  before any JS runs.

### 5.5 daijro/camoufox
- URL: https://github.com/daijro/camoufox (referenced via the proxies.sx
  and roundproxies analyses)
- Approach: Custom **Firefox** build with fingerprint spoofing
  implemented at the C++ level — `navigator.hardwareConcurrency`,
  WebGL renderer string, AudioContext, screen geometry, WebRTC are
  all spoofed before JS can ever inspect them. No JS shims, so
  Object.getOwnPropertyDescriptor cannot detect the patch.
- What it doesn't solve: Firefox JA3 differs from Chrome JA3, so its
  use is constrained to "be Firefox" rather than "be any browser".

### 5.6 ultrafunkamsterdam/nodriver
- URL: https://github.com/ultrafunkamsterdam/nodriver
- Approach: Async Python client that talks raw CDP without selenium /
  webdriver. Avoids `Runtime.enable` and other classic detection signals.
- What it doesn't solve: TLS/H2 inherits from the bundled Chrome
  binary — no impersonation flexibility.

### 5.7 lexiforest/curl-impersonate (active fork of lwthiker/curl-impersonate)
- URL: https://github.com/lexiforest/curl-impersonate
- Approach: A curl build linked against BoringSSL (for Chrome) or NSS
  (for Firefox), with TLS extensions, cipher order, ALPN list,
  HTTP/2 SETTINGS, header order patched to match real browsers exactly.
  As of Aug 2025 includes X25519Kyber768/X25519MLKEM PQ curves added in
  Chrome 124/130.
- What it doesn't solve: It's only a network client — no JS execution,
  no DOM. Fine for endpoints that don't run a JS challenge; fails
  against any real CF Managed Challenge.
- Python binding: `curl_cffi` (https://pypi.org/project/curl-cffi/).

### 5.8 scaredos/cfresearch
- URL: https://github.com/scaredos/cfresearch (archived but useful)
- Approach: Pure documentation of the CF orchestrator endpoint paths,
  payload structure, and request sequencing. Does not ship a solver.
- Best resource for understanding what to detect and what fields exist.

### 5.9 cloudflare-themis / similar
- No first-class Themis-equivalent for CF exists publicly. Closest
  spiritual analogs are `Humphryyy/Kasada-Deobfuscated` (Kasada-only)
  and the cfresearch repo above.

---

## 6. udemy.com — concrete fingerprinting

Live `curl -sI` against `https://www.udemy.com/` on 2026-05-10:

```text
HTTP/2 403
date: Sun, 10 May 2026 16:51:05 GMT
content-type: text/html; charset=UTF-8
content-length: 5510
accept-ch: Sec-CH-UA-Bitness, Sec-CH-UA-Arch, Sec-CH-UA-Full-Version,
           Sec-CH-UA-Mobile, Sec-CH-UA-Model, Sec-CH-UA-Platform-Version,
           Sec-CH-UA-Full-Version-List, Sec-CH-UA-Platform, Sec-CH-UA,
           UA-Bitness, UA-Arch, UA-Full-Version, UA-Mobile, UA-Model,
           UA-Platform-Version, UA-Platform, UA
cf-mitigated: challenge
content-security-policy: default-src 'none';
   script-src 'nonce-DoMfY25WKXmykWcQSPEtFn' 'unsafe-eval'
              https://challenges.cloudflare.com;
   …
   frame-src 'self' https://challenges.cloudflare.com blob:;
   child-src 'self' https://challenges.cloudflare.com blob:;
   worker-src blob:;
server: cloudflare
critical-ch: <same list as accept-ch>
set-cookie: __cf_bm=...; HttpOnly; Secure; Path=/; Domain=udemy.com; …
cf-ray: 9f9a7259de914d45-YVR
```

Body excerpt (the inline orchestrator config):
```js
window._cf_chl_opt = {
    cFPWv: 'g',
    cH:    'EwhK3.EvWCoWNMFp08JGZPYK4Kgq.KINXsAzHRc2JtI-1778431871-1.2.1.1-…',
    cITimeS: '1778431871',
    cN:    'VMXi8tUab5dnlKwLSWc0p3',  // CSP nonce
    cRay:  '9f9a727a2808cb95',
    cTplB: '0', cTplC: 0, cTplO: 0, cTplV: 5,
    cType: 'managed',                 // <— Managed Challenge
    cUPMDTk:'/?__cf_chl_tk=…',
    cvId:  '3',
    cZone: 'www.udemy.com',
    fa:    '/?__cf_chl_f_tk=…',
    md:    '1wf1ARxfTUw…',
    mdrd:  'PGcLxGtOfF6NY…',
};
var a = document.createElement('script');
a.nonce = 'VMXi8tUab5dnlKwLSWc0p3';
a.src   = '/cdn-cgi/challenge-platform/h/g/orchestrate/chl_page/v1?ray=9f9a727a2808cb95';
```

Diagnosis:
- `cf-mitigated: challenge` + `cType: 'managed'` + presence of
  `challenges.cloudflare.com` in CSP unambiguously: this is a
  **Managed Challenge** that *will* embed a Turnstile widget if the
  signal-collection phase grades the visitor poorly.
- The `__cf_bm` cookie is set immediately. This is the Cloudflare
  Bot Management cookie used to track this session through the
  challenge flow. (Different from `cf_clearance`, which is only
  issued *after* successful challenge completion.)
- `cFPWv: 'g'` means platform variant "g" (the newer of "b"/"g"
  variants). Path: `/h/g/orchestrate/chl_page/v1`.
- `cvId: '3'` likely means challenge-version 3 (the JS VM tier).
- The CSP requires every script we load to either be inline with
  the matching nonce, or hosted at `challenges.cloudflare.com`. If
  browser_oxide's CSP enforcer is stricter than Chrome's (e.g.
  blocks the `blob:` worker-src), we will fail to even start the
  challenge — that alone could be the root cause.
- `accept-ch` + `critical-ch` listing 14 UA-CH headers means: if
  our response to the **second** request (after the client-hint
  negotiation) doesn't carry every one of those headers with
  internally consistent values, threat score goes up.

Public difficulty assessments:
- Scraperly (https://scraperly.com/scrape/udemy) rates udemy "2/5
  difficulty" and notes "datacenter proxies work but JavaScript
  execution is essential for full content. curl_cffi works, but
  the API approach is more reliable."
- This is consistent with the hypothesis that *udemy serves
  Managed Challenge to anything that doesn't look like a
  fully-fingerprint-correct browser*, but does not aggressively
  escalate to interactive Turnstile for residential IPs running
  Chromium-grade clients.

---

## 7. Recommended fix architecture for browser_oxide

### 7.1 Where the code lives

Mirror the kasada layout exactly:

- `crates/stealth/src/cloudflare.rs` — pure-Rust module, parsing +
  detection helpers + (eventually) PoW solver. No deno_core dependency.
- `crates/stealth/src/lib.rs` — add `pub mod cloudflare;` next to
  `pub mod kasada;`.
- `crates/browser/src/page.rs` — add `handle_cloudflare_flow(...)`
  next to the existing kasada hook around the post-navigate retry
  loop (line ~2200).
- `crates/net/src/lib.rs` (or wherever HttpClient lives) — analogous
  to `learn_kasada_prefix` / `kasada_sessions().store_*`, add a
  `cloudflare_sessions()` cache keyed by host that retains
  `__cf_bm`, `cf_clearance`, the last `cf-ray`, and any harvested
  Turnstile tokens.

### 7.2 Detection helper (V0, half-day work)

```rust
// crates/stealth/src/cloudflare.rs
pub enum CfChallengeKind { Jsch, Managed, Interactive, NonInteractive, Unknown }
pub struct CfChallengeContext {
    pub kind: CfChallengeKind,
    pub ray: String,
    pub zone: String,
    pub csp_nonce: String,
    pub orchestrator_url: String,
    pub fa_url: String,
    pub mdrd: String,
}
pub fn detect_challenge(headers: &HeaderMap, body: &str)
    -> Option<CfChallengeContext>;
```

Logic: check for `cf-mitigated: challenge` in headers. If absent, scan
body for `window._cf_chl_opt = {...}` and extract `cType`, `cRay`,
`cN`, `cZone`, the orchestrator script `src=`, `fa`, `mdrd`. Use a
small handwritten parser, not a JSON parser — the `_cf_chl_opt`
literal contains JS expressions, not strict JSON.

### 7.3 Engine hooks in `Page::navigate`

After the response body lands, call the detector. If a CF challenge
is detected and our retry budget allows:

1. **Negotiate UA-CH.** Re-issue the request with all `accept-ch` /
   `critical-ch` headers populated with values consistent with our
   advertised User-Agent. (browser_oxide already has presets in
   `crates/stealth/src/presets.rs` — wire them in.)
2. **Allow CSP exception for `https://challenges.cloudflare.com`
   and `blob:` workers.** Without this, the orchestrator can't load
   its iframe or worker, and the challenge can't progress. Per the
   2026-05-10 holistic run.log we have a bunch of `[csp] Refused to
   load…` lines for cloudflareinsights.com — verify our CSP enforcer
   isn't over-blocking the `challenges.cloudflare.com` origin.
3. **Run the orchestrator JS to completion in our V8 + DOM.** The
   key insight: don't try to deobfuscate the orchestrator. Run it.
   Our V8 is real V8; our DOM has a real `document.createElement`,
   real `postMessage`, real `XMLHttpRequest`. The orchestrator will
   do its thing and either set `cf_clearance` via a 302 redirect or
   POST a payload that triggers the cookie. We then retry navigate
   with the cookie attached.
4. **Wait policy.** The kasada flow currently has a 45 s budget for
   high-tier sites. Cloudflare Managed Challenge typically
   completes in 2–6 s if everything is in order. Add a 10–15 s
   budget for the CF case.
5. **Behavioral signals during the challenge wait.** During the
   interstitial, fire scroll / mousemove / focus events from
   `crates/stealth/src/behavior.rs` to satisfy the Phase-1
   "25 events" gate. Even if we're past Phase 1 in the modern
   orchestrator, some signal collectors still measure event rate.

### 7.4 Minimum-viable scope for V1

Targets, in order of priority:

1. **No-challenge path:** make sure 90% of CF sites pass without ever
   serving a challenge. This is *purely* a fingerprint quality
   problem — TLS, H2 SETTINGS, header order, UA-CH consistency,
   navigator.webdriver, canvas/WebGL/audio. Any improvement here
   reduces challenge frequency across the entire CF userbase.
2. **JS Challenge (`cType: 'jsch'`) and Managed Challenge that
   doesn't escalate to a widget** (`cType: 'managed'` resolving
   invisibly): support by running the orchestrator to completion as
   above. Target: pass udemy.com.
3. **Turnstile non-interactive widget** (`cType: 'non-interactive'`):
   same approach — run the widget JS in a real iframe to
   `challenges.cloudflare.com`, let it post back. Should work if
   step 2 works, since same machinery.
4. **(Out of scope for V1):** Turnstile interactive widget, JS-VM
   v3 challenge. These require either a Camoufox-grade fingerprint
   spoof (deeper than browser_oxide currently provides) or paid
   captcha service integration. Mark as "fall back to upstream
   captcha solver via configurable plug-in".

### 7.5 Is this a TLS problem or a JS-VM problem?

**Both, in this order:**

- For ~80% of CF sites, *purely* a TLS+H2+UA-CH fingerprint problem.
  If our network stack looks Chrome-shaped, we never see a challenge.
  The fact that browser_oxide is reportedly "Chrome 147 parity" and
  *still* hits the challenge on udemy means either (a) parity has
  some gap (probably H2 priority frames or the UA-CH consistency
  matrix), or (b) our IP looks bad (datacenter ASN), or (c) udemy is
  one of the CF customers running an aggressively-tuned WAF.
- For the other ~20%, plus any time we *do* hit a challenge, it
  becomes a JS-VM problem — the orchestrator must execute and grade
  us as human. Here, the right answer is **don't fight the script**:
  let it run, make sure our DOM/V8 look real, satisfy its
  expectations.
- A Kasada-style hand-coded SHA-256 PoW solver is the **wrong**
  V1 answer for CF. The CF PoW kernel is similar (SHA-256 prefix
  match), but the surrounding ceremony (signal collection, AST-
  rotated obfuscation, per-Ray-ID encryption, dynamic answer
  format) means a hand-coded solver would be obsolete within the
  next 30-minute key rotation window.

---

## 8. Solvability per tier (2026)

| Tier | Headless-only feasible? | Evidence |
| --- | --- | --- |
| **Basic JS Challenge (`cType: 'jsch'`)** | Yes, with execution. | cloudscraper handles these in pure Python with js2py. browser_oxide with real V8 + DOM should pass trivially, given correct fingerprint. |
| **Managed Challenge invisible (`cType: 'managed'`, no widget)** | Yes, if fingerprint is good. | This is what Patchright + good UA-CH usually clears. Source: https://www.zenrows.com/blog/patchright. |
| **Managed Challenge that escalates to Turnstile non-interactive** | Yes, but tighter. | Camoufox at ~0% headless detection clears it. Stock Playwright/Puppeteer gets caught. browser_oxide's advantage: we're not on CDP at all, so the classic CDP leak is N/A. |
| **Turnstile interactive (visible click box)** | Generally no — needs either captcha service or human. | Scrapfly: "tokens require a full JavaScript environment and cannot be created through HTTP-only clients." 2Captcha/CapSolver charge per solve. Cloud browsers (Scrapfly, Browserless, Bright Data) bundle this. |
| **JS-VM v3 challenge (rare top-tier)** | No, in 2026. | "Cloudflare v3 challenges run in a JavaScript Virtual Machine, use advanced detection algorithms… generate dynamic code that is harder to reverse-engineer" — cloudscraper25 docs. Even VeNoMouS's enhanced fork falls back to captcha services here. |

So: V1 (tiers 1–2) is solvable with engineering work; V2 (tier 3) is
solvable with engineering work + GPU/canvas/audio polish; V3 (tiers
4–5) is not realistically solvable without an external solver.

---

## 9. Action plan (ordered, with time estimates)

| # | Task | Estimate | Notes |
| --- | --- | --- | --- |
| 1 | **Audit current TLS+H2 fingerprint vs Chrome 147 baseline against udemy specifically.** Capture the H2 SETTINGS+priority frames we send to udemy and diff against a real Chrome 147 capture. | 0.5 day | Use Wireshark on a `cargo test --release` run; compare to `docs/CHROME_147_TLS_REFERENCE_2026_04_29.json`. If we're already aligned, move on. |
| 2 | **Audit our CSP enforcer for false-positive blocks of `challenges.cloudflare.com`, `blob:`, and `worker-src blob:`.** | 0.5 day | run.log already shows CSP violations on cloudflareinsights.com. Confirm those are intentional and that the more permissive challenges.cloudflare.com origin will be allowed when CSP demands it. |
| 3 | **Implement UA-CH negotiation.** Respond to `accept-ch` and `critical-ch` headers by re-issuing the second request with all 14 hint headers populated consistently with our advertised UA. | 1 day | This is the single highest-leverage fix. Many CF customers (incl. udemy) score visitors largely on UA-CH consistency. |
| 4 | **Write `crates/stealth/src/cloudflare.rs` with detection-only API (`detect_challenge`).** Parse `cf-mitigated` header, body `_cf_chl_opt` blob, expose `CfChallengeContext`. Unit tests against the captured udemy body. | 1 day | No solver yet. Just observability so we can tell which CF sites we hit, what kind, and accumulate data. |
| 5 | **Wire `handle_cloudflare_flow` into `Page::navigate`.** On detection: log the kind+ray, do not retry yet. Add to the existing post-navigate retry budget. | 0.5 day | At this stage we're still failing, but we have telemetry. |
| 6 | **Run the orchestrator JS to completion.** In `handle_cloudflare_flow`, if we detect a challenge and we have a clean V8+DOM session: do nothing — let our normal page-load machinery execute the inline `<script>` and the orchestrator script. Then poll for `cf_clearance` cookie or for `cf-mitigated` to disappear on a probe request. If clearance arrives within 10 s, retry the original navigation. | 1 day | The trick: ensure our event loop actually drives the orchestrator's `setTimeout` / `XMLHttpRequest` / `fetch` calls. The kasada hook already does similar — copy that scaffold. |
| 7 | **Behavioral noise during challenge wait.** Inject mousemove/scroll/keyup events at the rate `behavior.rs` already produces, scoped to the challenge document, for 2–6 seconds while the orchestrator runs. | 0.5 day | Avoids the Phase-1 "25 events" gate. |
| 8 | **Validate against udemy.com.** End-to-end test: navigate, expect non-challenge response, expect a meaningful `<title>` and a >50 KB body that contains "Udemy". | 0.25 day | If it works, mark Cloudflare-CHL row in the holistic table closed. |
| 9 | **Extend test corpus.** Add 4–6 other CF sites at varying difficulty: a Cloudflare-Free site, a SBFM site, a BM-Enterprise site. Track pass rate. | 1 day | Good candidates: any of the cloudflarestatuspage hosts, hackernews has CF Free, indeed.com is SBFM-strong, openai.com has BM-Enterprise. |
| 10 | **(Stretch) JA4 signals diversity.** Verify our request mix doesn't produce skewed `paths_rank_1h` / `uas_rank_1h` ratios when many requests run from one process. | 0.5 day | Mitigate by interleaving real navigation with sub-resource loads, and varying the request pattern. |
| 11 | **(Stretch / V2) Turnstile non-interactive widget.** Add iframe-bridge timing realism for the `challenges.cloudflare.com` iframe; confirm widget completes and emits `cf-turnstile-response` token. | 2 days | Should "just work" if step 6 is solid; budget for debugging weird postMessage / iframe-load ordering. |
| 12 | **(V3, defer)** Pluggable captcha-service hook for Turnstile interactive — CapSolver / 2Captcha API client. | 1 day to ship, but only when we hit a real customer need. | Mark as out-of-scope for V1. |

**Total V1 (steps 1–9): ~6.25 engineer-days.**
**Total V2 (steps 1–11): ~9.25 engineer-days.**

---

## 10. Glossary + canonical URLs

- `__cf_bm` — Cloudflare Bot Management session cookie. Set on every
  request reaching CF. Tracks this session through the challenge flow.
  Not the same as `cf_clearance`.
- `cf_clearance` — Issued only after a successful challenge solve.
  Bound to IP+UA+TLS fingerprint. Lifetime per zone (default 30 min).
- `cf-ray` — Per-request unique identifier; suffix is the CF colo
  (`-YVR` = Vancouver). Used as input to dynamic JS decryption keys.
- `cf-mitigated: challenge` — The single canonical detection signal
  that a challenge page was served instead of origin content.
- IUAM — "I'm Under Attack Mode". Legacy 5-second JS-PoW interstitial.
  Largely subsumed by Managed Challenge in 2024–2026. The HTML
  `<title>Just a moment…</title>` is reused.
- Managed Challenge — Cloudflare's adaptive challenge that picks
  between invisible verification, Turnstile non-interactive, and
  Turnstile interactive based on bot score.
- Turnstile — Cloudflare's CAPTCHA replacement. Three modes: managed
  (default), non-interactive, invisible. Issues a one-shot token that
  the customer's server validates via the Siteverify API.
- Privacy Pass — IETF protocol (RFC 9474, blind RSA signatures) where
  a successful Turnstile solve mints a token redeemable across CF
  sites. Cannot be minted client-side.
- JA3 / JA4 — TLS ClientHello fingerprints. JA3 is the older
  MD5-of-fields scheme; JA4 (FoxIO, Sept 2023) sorts extensions and
  yields stable per-browser fingerprints. CF exposes both to BM
  Enterprise customers.

### Primary sources

Cloudflare official docs:
- Challenges overview: https://developers.cloudflare.com/cloudflare-challenges/
- Detect challenge response: https://developers.cloudflare.com/cloudflare-challenges/challenge-types/challenge-pages/detect-response/
- Clearance cookie: https://developers.cloudflare.com/cloudflare-challenges/concepts/clearance/
- Turnstile: https://developers.cloudflare.com/cloudflare-challenges/challenge-types/turnstile/
- Turnstile widgets: https://developers.cloudflare.com/turnstile/concepts/widget/
- Bot Management overview: https://developers.cloudflare.com/bots/get-started/bot-management/
- Bot Fight Mode: https://developers.cloudflare.com/bots/get-started/bot-fight-mode/
- JA3/JA4: https://developers.cloudflare.com/bots/additional-configurations/ja3-ja4-fingerprint/
- JA4 Signals: https://developers.cloudflare.com/bots/additional-configurations/ja3-ja4-fingerprint/signals-intelligence/
- Cloudflare cookies reference: https://developers.cloudflare.com/fundamentals/reference/policies-compliances/cloudflare-cookies/
- `cf.bot_management.ja4` field: https://developers.cloudflare.com/ruleset-engine/rules-language/fields/reference/cf.bot_management.ja4/

Cloudflare blog posts:
- Super Bot Fight Mode: https://blog.cloudflare.com/super-bot-fight-mode/
- Per-customer bot defenses: https://blog.cloudflare.com/per-customer-bot-defenses/
- JA4 Signals: https://blog.cloudflare.com/ja4-signals/
- Privacy Pass standard: https://blog.cloudflare.com/privacy-pass-standard/
- Privacy Pass math: https://blog.cloudflare.com/privacy-pass-the-math/
- Turnstile launch: https://blog.cloudflare.com/turnstile-private-captcha-alternative/
- Integrating Turnstile with WAF: https://blog.cloudflare.com/integrating-turnstile-with-the-cloudflare-waf-to-challenge-fetch-requests/

Reverse-engineering write-ups:
- Noah Riveiro — IUAM challenge phases: https://blog.noah.ovh/cloudflare-js-challenge-1/
- 0xdevalias gist of CF/Akamai notes: https://gist.github.com/0xdevalias/b34feb567bd50b37161293694066dd53
- scaredos/cfresearch (orchestrate endpoints): https://github.com/scaredos/cfresearch
- Akamai H2 fingerprint whitepaper: https://blackhat.com/docs/eu-17/materials/eu-17-Shuster-Passive-Fingerprinting-Of-HTTP2-Clients-wp.pdf
- HTTP/2 fingerprint demonstrator: https://privacycheck.sec.lrz.de/passive/fp_h2/fp_http2.html
- Castle.io — V8 patch killing the CDP signal: https://blog.castle.io/why-a-classic-cdp-bot-detection-signal-suddenly-stopped-working-and-nobody-noticed/
- Datadome — new headless Chrome + CDP signal: https://datadome.co/threat-research/how-new-headless-chrome-the-cdp-signal-are-impacting-bot-detection/

Open-source RE projects:
- VeNoMouS/cloudscraper: https://github.com/VeNoMouS/cloudscraper
  - Modern challenge format issue: https://github.com/VeNoMouS/cloudscraper/issues/298
  - Changelog: https://github.com/VeNoMouS/cloudscraper/blob/master/CHANGELOG.md
- FlareSolverr: https://github.com/FlareSolverr/FlareSolverr
- ultrafunkamsterdam/undetected-chromedriver: https://github.com/ultrafunkamsterdam/undetected-chromedriver
- ultrafunkamsterdam/nodriver (referenced in proxies.sx article)
- Kaliiiiiiiiii-Vinyzu/patchright: https://github.com/Kaliiiiiiiiii-Vinyzu/patchright
- daijro/camoufox (referenced in proxies.sx + roundproxies articles)
- lexiforest/curl-impersonate: https://github.com/lexiforest/curl-impersonate
- lwthiker/curl-impersonate (parent project): https://github.com/lwthiker/curl-impersonate
- curl_cffi (Python binding): https://pypi.org/project/curl-cffi/
- cloudflare/privacypass-attester: https://github.com/cloudflare/privacypass-attester
- cloudflare/pp-browser-extension: https://github.com/cloudflare/pp-browser-extension
- Privacy Pass protocol explainer: https://deepwiki.com/cloudflare/pp-browser-extension/1.1-privacy-pass-protocol
- sardanioss/httpcloak (Go HTTP client with full impersonation): https://github.com/sardanioss/httpcloak
- Xetera/nginx-http2-fingerprint: https://github.com/Xetera/nginx-http2-fingerprint

Bypass guides + analysis:
- ZenRows — Bypass CF: https://www.zenrows.com/blog/bypass-cloudflare
- ZenRows — CF JS challenge: https://www.zenrows.com/blog/cloudflare-js-challenge-bypass
- ZenRows — Patchright: https://www.zenrows.com/blog/patchright
- ZenRows — cf_clearance scraping: https://www.zenrows.com/blog/cf-clearance
- ZenRows — curl-impersonate: https://www.zenrows.com/blog/curl-impersonate
- Scrapfly — Bypass CF (overview): https://scrapfly.io/blog/posts/how-to-bypass-cloudflare-anti-scraping
- Scrapfly — Turnstile: https://scrapfly.io/blog/posts/how-to-bypass-cloudflare-turnstile
- Scrapfly — H2/H3 fingerprint guide: https://scrapfly.io/blog/posts/http2-http3-fingerprinting-guide
- Scrapfly — curl-impersonate guide: https://scrapfly.io/blog/posts/curl-impersonate-scrape-chrome-firefox-tls-http2-fingerprint
- Substack TheWebScrapingClub — How to bypass CF in 2025: https://substack.thewebscraping.club/p/how-to-bypass-cloudflare-turnstile
- Capsolver — Bypass CF challenge 2026: https://www.capsolver.com/blog/Cloudflare/bypass-cloudflare-challenge-2025
- Capsolver — TLS fingerprinting: https://www.capsolver.com/blog/Cloudflare/cloudflare-tls
- Capsolver — Turnstile vs Challenge: https://www.capsolver.com/blog/Cloudflare/how-to-identify-turnstile-challenge
- Browserless — TLS fingerprint bypass: https://www.browserless.io/blog/tls-fingerprinting-explanation-detection-and-bypassing-it-in-playwright-and-puppeteer
- Kameleo — Bypass Runtime.enable: https://kameleo.io/blog/bypass-runtime-enable-with-kameleos-undetectable-browser
- Kameleo — Bypass Turnstile with Scrapy: https://kameleo.io/blog/how-to-bypass-cloudflare-turnstile-with-scrapy
- Proxies.sx — Camoufox / Nodriver / Stealth MCP: https://www.proxies.sx/blog/ai-browser-automation-camoufox-nodriver-2026
- Roundproxies — Patchright alternatives: https://roundproxies.com/blog/best-patchright-alternatives/
- Roundproxies — Patchright guide: https://roundproxies.com/blog/patchright/
- Roundproxies — cloudscraper guide: https://roundproxies.com/blog/cloudscraper/
- Roundproxies — cf_clearance: https://roundproxies.com/blog/cf-clearance/
- Scraperly — Udemy scraping guide: https://scraperly.com/scrape/udemy
- BrowserLeaks — H2 fingerprint: https://browserleaks.com/http2

---

## 11. Open questions / hypotheses to verify

- **[hypothesis]** browser_oxide's H2 priority frame emission on the
  initial GET to udemy may not match Chrome's `3:0:0:201|m,p,a,s`
  exactly. Worth a Wireshark check against
  `docs/CHROME_147_TLS_REFERENCE_2026_04_29.json`.
- **[hypothesis]** The `Sec-CH-UA-Full-Version-List` header — when we
  do answer it — must list the same brand+version triples that our
  inline `navigator.userAgentData.brands` claims. Inconsistency is
  a high-value detection signal for CF (and Akamai). Worth a sweep.
- **[hypothesis]** The `[csp]` violations in run.log on
  `cloudflareinsights.com` may not be a problem (it's the analytics
  beacon, not the challenge orchestrator), but the *same enforcement
  path* might block `challenges.cloudflare.com` if CF ever serves a
  challenge that needs a worker. Worth a code review of
  `crates/dom/src/csp/...` (or wherever).
- **[hypothesis]** The Phase-3 SHA-256 PoW kernel from the original
  IUAM challenge is not always present in modern Managed Challenge
  flows — the orchestrator now relies more heavily on signal grading
  and less on PoW work. If true, hand-coding a PoW solver buys very
  little. Confirm by tracing what udemy's orchestrator actually
  POSTs after running.
- **Worth revisiting:** the canvas/WebGL/audio fingerprint values
  emitted by browser_oxide's `crates/stealth/src/gpu.rs` — do they
  appear in CF's known-fingerprint corpus as a real Chrome on a real
  Linux laptop? Or do they look like a programmatically-generated
  hash? The Akamai sensor analysis already captured for adidas
  (`docs/akamai_sensor_analysis/`) is a useful reference: the CF
  collector probes a similar surface.
- **Engine-vs-vendor question:** at what point do we accept that
  Turnstile interactive is "off the table" for purely-headless
  browser_oxide and ship a configurable plug-in for an external
  captcha service? Suggest: V1 ships without it; V2 ships the
  plug-in interface; we document loudly that "if the site demands a
  visible Turnstile click, browser_oxide cannot solve it and you
  must bring your own solver."

---

*End of report. Estimated read time: 25 min. Suggested next step:
implement steps 1–4 in §9, deliver telemetry-only V0 in 2 days, then
gate the rest of V1 on the udemy-specific TLS+UA-CH audit results.*
