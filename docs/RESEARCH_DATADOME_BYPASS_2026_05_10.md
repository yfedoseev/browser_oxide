# Research — DataDome bypass landscape (2026-05-10)

> Public information, code, and reverse-engineering writeups for getting
> through DataDome bot defense. Compiled to seed an eventual
> `crates/datadome/` solver crate (or `crates/stealth/src/datadome.rs`
> module) for browser_oxide. Built by web-searching GitHub, Substack,
> Medium, vendor docs, and independent research blogs. No paid services
> subscribed.
>
> **Trigger context:** the 2026-05-10 holistic sweep
> (`docs/HOLISTIC_TEST_2026_05_10/SUMMARY.md`) shows 4 sites stuck on
> `DataDome-CHL` outcome:
> - **leboncoin.fr** (FR classifieds; case study from DataDome themselves
>   confirms 9.5M malicious requests/day blocked, 30 protected endpoints)
> - **yelp.com** (US local-search)
> - **wsj.com** (paywall + DataDome)
> - **etsy.com** (DataDome customer story published; e-commerce search
>   protection)
>
> The leboncoin run log captured the diagnostic CSP message we expected:
> `[csp] Refused to frame 'https://geo.captcha-delivery.com/captcha/'`.
> That confirms (a) we reach the page, (b) the DataDome JS tag executes,
> (c) it scores us as bot, (d) it tries to inject the captcha iframe
> from `geo.captcha-delivery.com`. We have ZERO DataDome RE work in the
> repo — this doc is the foundation.
>
> Companion docs:
> - `docs/RESEARCH_KASADA_BYPASS_2026_04_29.md` — Kasada equivalent.
> - `docs/RESEARCH_AKAMAI_BMP_BYPASS_2026_04_29.md` — Akamai equivalent.
> - `docs/kasada_ips_analysis/` — full ips.js + decoded VM opcodes.
> - `crates/stealth/src/kasada.rs` — Kasada PoW solver implementation.

---

## Table of contents

1. Vendor & deployment surface
2. Bootstrap script (loader, hosts, JS tag URL pattern)
3. Probe surface (what the JS measures)
4. Submission protocol (sensor POST, encryption)
5. Token / cookie flow (`datadome` cookie lifecycle)
6. Captcha / interstitial gate (`geo.captcha-delivery.com`)
7. Public RE work to study (GitHub, Medium, vendors)
8. Per-site notes — leboncoin, yelp, wsj, etsy
9. Recommended fix architecture for browser_oxide
10. What can be done without a captcha-solving service
11. Action plan with time estimates
12. Open questions / what we still need to capture

---

## 1 — Vendor & deployment surface

**Vendor**: DataDome SAS — Paris/NYC, founded 2015, raised >$80M
through Series C (2024). Primary product is a multi-tenant bot-management
WAF / "Agent Trust" platform.

**Marketing claims to anchor scale** (from
`https://datadome.co/`, accessed 2026-05-10):
- "5 trillion signals processed daily."
- "<2 ms scoring latency."
- "85,000 customer-specific ML models" (per-customer / per-endpoint).
- "0.01% visible-CAPTCHA rate" — i.e. 99.99% of visible captchas hit
  bots, not humans (from `datadome.co/products/datadome-captcha/`).
- Customer base: ~1,200 enterprises per coverage articles
  (`zenrows.com/blog/datadome-bypass`); Wappalyzer indexes ~48,900
  domains using DataDome.

**Confirmed deployments relevant to us** (vendor-published or
high-confidence press):
- **Etsy** — DataDome customer story
  (`datadome.co/customers-stories/etsy-stops-unwanted-traffic-reduces-computing-costs-with-datadome-google/`).
- **Leboncoin** — DataDome customer story, "9.5M blocks/day, 30 endpoints,
  0.0018% FP rate" — a real production deployment that they brag about
  (`datadome.co/customers-stories/how-leboncoin-blocks-millions-of-malicious-requests-every-day/`).
- **SoundCloud, SeatGeek, Pokemon Center, UEFA ticketing, Arsenal FC,
  PayPal, Foot Locker, Reddit (some endpoints), Allegro.pl** — listed
  by various articles; not all current as of 2026.
- **Yelp / WSJ** — strongly suspected but I could not find a
  vendor-published case study; classification depends on the JS tag URL
  pattern which we should sniff in our run logs (search for
  `js.datadome.co` or any first-party path serving the tag).

**Tier system [hypothesis]**: DataDome does **not** publish a tier
catalog the way Akamai BMP / Kasada do. However, the customer
configuration knobs imply at least three protection levels:
1. **Monitor-only** (silent allow, ML score logged) — not visible to
   bots; score harmless.
2. **CAPTCHA challenge** — slider GeeTest-style puzzle on suspicious
   score (`rt:'c'` in the `dd` object).
3. **Interstitial / Device Check** — non-interactive proof page
   (`rt:'i'`); often combined with a WASM CPU challenge (see §6).
4. **Hard 403** — top-tier; no challenge served, score must clear or
   client is dropped.

The four sites we fail on serve `rt:'c'` or `rt:'i'` based on the CSP
log evidence — they're at level 2/3 (i.e. challenge with retry, not
hard-block). That's good news because we get a chance to retry with a
solved cookie.

Difficulty proxy (anecdotal, from bypass-vendor pages
`scrapfly.io/bypass/datadome`, `kameleo.io/blog/guide-to-bypassing-datadome`):
- **Mid difficulty** — most retail (Etsy, Foot Locker, Pokemon Center)
  passes with stealth + residential IP.
- **High difficulty** — Leboncoin / SeatGeek / UEFA tickets — per-customer
  ML models trained for years on their specific bot landscape; even
  perfect-fingerprint scrapers get challenged on suspicious nav pattern.
- **Hardest** — DataDome's own demo page intentionally over-tuned.

---

## 2 — Bootstrap script

### 2.1 Standard third-party JS tag

Default integration loads the tag from DataDome's CDN:

```html
<script>
  window.ddjskey  = 'ABCDEF1234567890ABCDEF1234567890';   // tenant key
  window.ddoptions = { /* per-tenant config */ };
</script>
<script src="https://js.datadome.co/tags.js" async></script>
```

(Source: `docs.datadome.co/docs/javascript-tag` and
`docs.datadome.co/docs/how-to-configure-the-javascript-tag`.)

The `ddjskey` is the **tenant identifier**, NOT a session token. It's
static and visible in the page source — good signal for engine-side
detection that DataDome is in play.

**File pinning**: The serving `tags.js` is **not strongly polymorphic**.
glizzykingdreko's writeup
(`medium.com/@glizzykingdreko/breaking-down-datadome-captcha-waf-d7b68cef3e21`)
calls out daily rotation of *signal keys* (six-char random strings every
~24h), but the script body itself is mostly static across requests for
a given tenant. The hackmd analysis
(`hackmd.io/@s2Q4nEh2Shu1POK_sxsPnA/B1vsX2v8o`) likewise notes "in this
case the script remains static across requests" though "such files are
sometimes made to vary from request to request". So expect:
- Script body: stable for hours-to-days.
- Signal-key dictionary embedded in the script: rotates ~daily.
- This is **far weaker** than Kasada (which rotates the entire VM
  bytecode every ~hour per-tenant). DataDome is closer to Akamai BMP
  in update cadence.

### 2.2 First-party (proxied) deployment

Customers can serve the tag through their own domain to dodge
ad-blockers. Pattern:

```html
<script src="https://www.example.com/some-path/tags.js" async></script>
```

The proxy fetches the upstream `js.datadome.co/tags.js` (or a
slightly customized variant) and serves it under the customer's
domain. **Engine-side detection has to grep for behaviors, not just
the host name.** Indicators:
- `window.ddjskey` set inline.
- `window.ddoptions` object.
- POST to `*api-js.datadome.co*` OR a first-party
  `/<path>/api-js/...` proxied endpoint.
- iframe whose `src` starts with `https://*.captcha-delivery.com/`.
- HTML inline `var dd = { rt: 'c'|'i', cid: '...', hsh: '...',
  host: 'geo.captcha-delivery.com' }` on a 403 / interstitial page.

### 2.3 Mobile SDK variant

Mobile apps (Android/iOS) hit a different POST endpoint:
`https://api-sdk.datadome.co/sdk/`. Body is form-encoded with fields
like `cid`, `ddk`, `request`, `ua`, `ddv` (SDK version), `ddvc` (app
version), `d_ifv` (device install ID).
(Source: cerealautomation Medium article on Too Good To Go, see §7.)

This is **not** what our headless browser will hit (we present as web,
not native), but the cookie format is the same — cross-channel intel
is useful.

### 2.4 What our log evidence already tells us

From the leboncoin run we saw `[csp] Refused to frame
'https://geo.captcha-delivery.com/captcha/'`. That single line proves:
- The `tags.js` ran inside our V8.
- It rejected our score and tried to render the `geo.captcha-delivery.com`
  iframe.
- Our default CSP (or our CSP enforcement layer documented in
  `docs/CSP_ENFORCEMENT_DESIGN_2026_04_29.md`) refused the frame.

Action item before any solver work: **capture the actual `tags.js`
that `js.datadome.co` returned during that run.** We need it on disk
to feed into the deobfuscator (see §7).

---

## 3 — Probe surface (what the JS sensor measures)

### 3.1 Confirmed signal categories

Compiled from
- `dataresearchtools.com/bypass-datadome-anti-bot-web-scraping/`
  (lists exact field names),
- `medium.com/@mayankchandel2567/exploring-methods-of-evading-datadomes-bot-protection-...`,
- `kameleo.io/blog/guide-to-bypassing-datadome`,
- `datadome.co/threat-research/the-art-of-bot-detection-picasso-...`,
- `datadome.co/threat-research/how-new-headless-chrome-the-cdp-signal-...`,
- `datadome.co/anti-detect-tools/audio-fingerprint/`.

Known signal keys in the `jsData` payload (these are *human-readable*
internal names — the wire-format keys are six-char rotating
obfuscated):

| Key       | Source                              | What it captures                |
|-----------|-------------------------------------|---------------------------------|
| `rs_h`    | `window.screen.height`              | Raw screen height               |
| `rs_w`    | `window.screen.width`               | Raw screen width                |
| `rs_cd`   | `window.screen.colorDepth`          | Color depth (24 typical)        |
| `ars_h`   | `window.screen.availHeight`         | Available screen height         |
| `ars_w`   | `window.screen.availWidth`          | Available screen width          |
| `phe`     | `window._phantom`                   | PhantomJS sentinel              |
| `nm`      | `window.__nightmare`                | Nightmare sentinel              |
| `wdr`     | `navigator.webdriver`               | WebDriver flag                  |
| `ua`      | `navigator.userAgent`               | UA string                       |
| `lg`      | `navigator.language`                | Primary language                |
| `lgs`     | `navigator.languages.length`        | Languages array length          |
| `plg`     | `navigator.plugins.length`          | Plugin count                    |
| `tzp`     | `Date().getTimezoneOffset()`        | Timezone offset                 |
| `glrd`    | WebGL `UNMASKED_RENDERER_WEBGL`     | GPU renderer string             |
| `glvd`    | WebGL `UNMASKED_VENDOR_WEBGL`       | GPU vendor string               |
| `str_ss`  | `'sessionStorage' in window`        | Storage support flag            |
| `str_ls`  | `'localStorage' in window`          | Storage support flag            |
| `str_idb` | `'indexedDB' in window`             | Storage support flag            |
| `str_odb` | `'openDatabase' in window`          | WebSQL legacy flag              |
| `hc`      | `navigator.hardwareConcurrency`     | CPU core count                  |
| `dm`      | `navigator.deviceMemory`            | Device memory (GB)              |
| `pmf`     | `navigator.permissions`             | Permissions API surface         |
| `bav`     | `navigator.brave`                   | Brave shim                      |

Plus the **events** array: timestamped mouse moves, scrolls, keyboard
events, focus/blur. These are appended over the lifetime of the page
and flushed on the first POST.

### 3.2 Higher-order checks

Beyond per-property reads, the script does cross-checking:

1. **Picasso canvas challenge** — server sends a sequence of canvas
   draw instructions (lines, beziers, fonts) seeded with a random
   value; the JS replays them and hashes the resulting pixel buffer.
   The hash differs per OS/GPU/browser combination because of subtle
   rasterization differences between Skia versions and GPU drivers. A
   spoofed UA string ("I'm Chrome on Mac") that produces a Linux
   raster signature is caught immediately.
   - Source: `datadome.co/threat-research/the-art-of-bot-detection-picasso-for-device-class-fingerprinting/`
     and the original Bursztein paper
     `elie.net/static/files/picasso-lightweight-device-class-fingerprinting-for-web-clients/`.
   - **This is what bites us.** Our `crates/canvas` does Skia, but our
     `gpu` impl returns dummy/placeholder reads for many WebGL probes
     (see `docs/PHASE6_FINGERPRINT_INVENTORY_FINDINGS_2026_04_29.md`).

2. **Audio fingerprint** — `OfflineAudioContext` synthesis hash. Same
   class-of-device discrimination idea applied to audio rendering.
   Source: `datadome.co/anti-detect-tools/audio-fingerprint/`.

3. **Function.toString / native code probes** — calls
   `Function.prototype.toString` on known natives (e.g.
   `HTMLCanvasElement.prototype.toDataURL`,
   `OfflineAudioContext.prototype.startRendering`,
   `Notification.requestPermission`) and checks the result equals
   `function name() { [native code] }`. Any monkey-patched override
   is exposed.
   - Our `Function.toString` mask sweep is documented as recently
     completed (commit history mentions "Function.toString mask sweep")
     but DataDome may probe natives we don't intercept yet.

4. **CDP / Runtime.enable detection** — the bombshell from Antoine Vastel
   (DataDome) on 2024-06-13:
   `datadome.co/threat-research/how-new-headless-chrome-the-cdp-signal-are-impacting-bot-detection/`
   and follow-up rebrowser writeup
   `rebrowser.net/blog/how-to-fix-runtime-enable-cdp-detection-of-puppeteer-playwright-and-other-automation-libraries`.
   - Mechanism: trigger `console.debug({ get foo(){…} })` inside a
     `try`/observer; if the JIT-serialized object pops up via
     `Runtime.consoleAPICalled`, the browser has CDP attached.
   - **Our advantage**: browser_oxide does NOT speak CDP. We embed V8
     directly, no devtools protocol attached. So this probe naturally
     returns "not detected" for us. This is one of the structural wins
     from §1.7 of `docs/RESEARCH_DEEP_DIVE_SOTA_2026.md`.

5. **Headless artefact probes** (from
   `datadome.co/headless-browsers/headless-chrome/`):
   - `navigator.webdriver === true`
   - `navigator.languages.length === 0`
   - `navigator.plugins.length === 0`
   - User-agent contains `HeadlessChrome` substring
   - `chrome.runtime` undefined when UA claims Chrome
   - Permissions API contradicts cookie-enabled state
   - Specific codecs: `HTMLMediaElement.canPlayType('video/mp4; codecs="avc1.42E01E"')`
     returns wrong value for headless

6. **Timing tells**: `performance.now()` resolution and jitter
   distribution. Headless browsers often have suspiciously clean
   timing sequences. (DataDome doesn't publish exact thresholds, but
   it's mentioned in third-party guides.)

7. **WebGL probe set**: not just renderer/vendor strings — also
   `MAX_TEXTURE_SIZE`, `MAX_RENDERBUFFER_SIZE`, `getSupportedExtensions()`
   list ordering, shader compilation success/failure for specific
   GLSL programs.

### 3.3 Where DataDome differs from Kasada

| Surface                                    | Kasada                                                  | DataDome                                                  |
|--------------------------------------------|---------------------------------------------------------|-----------------------------------------------------------|
| VM-based JS execution                      | Yes, custom register VM, ~60 opcodes                    | No VM — straight obfuscated JS                            |
| Bytecode rotation                          | ~hourly, per-tenant `ips.js`                            | ~daily, signal-key rotation only                          |
| PoW challenge                              | Yes (`x-kpsdk-ct` token mint requires CPU work)         | Yes for interstitial (`boring_challenge` Rust→WASM)       |
| Picasso canvas class fingerprint           | No                                                      | Yes (signature feature)                                   |
| CDP `Runtime.enable` detection             | Yes                                                     | Yes (publicized 2024-06)                                  |
| Behavioral mouse path scoring              | Light (just collects events)                            | Heavy — 31 derived signals from coord lists               |
| Tenant ML model                            | Mostly per-product                                      | Per-customer + per-endpoint (85,000+ models)              |
| Cookie name                                | `KP_UIDz`, `x-kpsdk-ct`, `x-kpsdk-cd`                   | `datadome` (single cookie, year-long)                     |
| Sensor encryption                          | Custom report-body wrapper, base64                      | Dual-XOR + custom base64, PRNG seeded from `cid`          |
| Submission endpoint                        | `/tl` on tenant domain                                  | `https://api-js.datadome.co/js/` (or first-party)         |
| Captcha provider                           | Native Kasada (cliffhanger)                             | DataDome's own (slider) + GeeTest (slider) + interstitial |

**Big-picture**: DataDome is *less algorithmically heavy* than Kasada
(no VM bytecode), but *more breadth-of-signal heavy* (Picasso, audio,
behavioral coord scoring, per-tenant ML). For us, this is a different
fight: less "execute the sensor correctly" (V8 does that fine), more
"present a correct rendering stack and a non-trivial behavioral
trace".

### 3.4 Check against Kasada-related work

We learned in `docs/CANADA_GOOSE_DIAGNOSIS_2026_05_10_PART2.md` that
five fields share the
`unjzomuybtbyyhwwkdpkxomylnab`-undefined-receiver root cause.
DataDome will probably have an *analogous but different* set of
"undefined-receiver" paths — its obfuscation builds prototype-chain
calls dynamically. The deobfuscator (§7) will surface them.

---

## 4 — Submission protocol (sensor POST, encryption)

### 4.1 The endpoint

Confirmed across all sources:
- **Standard**: `POST https://api-js.datadome.co/js/`
- **First-party deployments**: customer-prefixed path proxying upstream
  to the same origin.
- **Mobile SDK** (out of scope for headless browser): `POST
  https://api-sdk.datadome.co/sdk/`.

(Sources: `dataresearchtools.com/bypass-datadome-anti-bot-web-scraping/`,
the hackmd analysis, glizzykingdreko Medium.)

### 4.2 The payload

Form-encoded body containing two main pieces:

1. **`jsData`** — JSON-like object, post-encryption rendered as a custom
   base64 string. Pre-encryption it's a kv map of the §3.1 signals.
2. **`events`** — JSON list of `{type, ts, x?, y?, key?}` rows captured
   during page lifetime.

Plus housekeeping fields:
- `ddk` — tenant js-key.
- `cid` — current `datadome` cookie value (or freshly generated 64-hex
  if none).
- `ddv` — JS tag version (e.g. `4.6.0`).
- `request` — full URL of the page.
- `ua` — UA string.
- `referer` — referer string.

### 4.3 Encryption (the painful part)

From glizzykingdreko's `Datadome-encryption` repo
(`github.com/glizzykingdreko/datadome-encryption`) and the
"Breaking Down Datadome Captcha WAF" Medium post
(`medium.com/@glizzykingdreko/breaking-down-datadome-captcha-waf-d7b68cef3e21`),
the algorithm (which they call `ddCaptchaEncodedPayload`) has four
stages:

**Stage 1 — PRNG seeding**

Two PRNGs are initialized:
- PRNG-A: seeded with `(websiteHash, cid, salt)` — used per-pair XOR
  passes during build.
- PRNG-B: seeded with `(cid, salt)` — used for the global second pass.

Salt comes from a constant XOR'd with the website hash:
- For Captcha: hash XOR `-1748112727`, salt comes from a dynamic HSV.
- For Interstitial: hash XOR `-883841716`, salt fixed at `9E9FC74889F6`.

The hash is a **djb2 variant** of input strings. The mixing function
is non-linear bit math.

**Stage 2 — buffer construction**

For each kv pair:
1. Add a start marker — XOR'd `'{'` for first pair, XOR'd `','` for
   subsequent.
2. Stringify the key, XOR each byte against PRNG-A.
3. Add separator — XOR'd `':'`.
4. Stringify the value, XOR each byte against PRNG-A.

End the buffer with XOR'd `'}'`.

**Stage 3 — second XOR pass**

Walk the entire buffer applying PRNG-B byte by byte. This second pass
is what the glizzykingdreko writeup calls "Dual-XOR Reversal".

**Stage 4 — custom base64 encoding**

A non-standard alphabet (close to URL-safe base64 but reordered) is
applied, with an additional XOR using a decrementing salt counter on
each emitted character.

### 4.4 Per-day signal-key rotation

The kv keys themselves are **not** the readable names from §3.1 — those
are the original *internal* names. On the wire they're rotated daily
to random six-character strings. So the encryption alone doesn't get
you a stable schema; you also need to scrape today's key dictionary
out of the tags.js for that tenant.

The dictionary is built in tags.js with literal-string assignments —
the deobfuscator has to walk the AST and find them. glizzykingdreko's
tools handle this.

### 4.5 Differences from the Captcha submission

When you've solved a captcha (slider), the submission goes to
`https://geo.captcha-delivery.com/captcha/check` (per the campo Medium
post) with a *different* encrypted payload — the `ddCaptchaEncodedPayload`
form. This contains:
- Browser fingerprint (same signals).
- Movement signals — 31 features derived from `_initialCoordsList`
  (page-load → button-click) and `_coordsList` (slider drag).
- WASM challenge result (if interstitial).
- Hash-challenge computations.

The check endpoint, on success, returns
```json
{ "status": 0, "cookie": "datadome=...; Max-Age=31536000; ..." }
```
which the page sets in `document.cookie`.

---

## 5 — Token / cookie flow

### 5.1 The single source of truth

DataDome operates on **one cookie**: `datadome`.
- Source: `docs.datadome.co/docs/cookie-session-storage`.
- Lifetime: `Max-Age=31536000` (1 year).
- Size: ~128 bytes value (encrypted blob).
- Attributes: `Domain=.<site>; Path=/; SameSite=Lax; Secure;
  HttpOnly` typically (HttpOnly varies by tenant).

Compared to Kasada (which juggles `KP_UIDz` + `x-kpsdk-ct` +
`x-kpsdk-cd` headers + a session token across requests), DataDome is
*much simpler*: one cookie does everything.

### 5.2 Auxiliary local state

- **`dd_testcookie`** (1 byte, transient) — sanity check that cookies
  work. JS sets and immediately deletes it during page load.
- **`ddSession`** (localStorage key) — a *duplicate* of the `datadome`
  cookie, written only when the tenant has `sessionByHeader: true`
  set in `ddoptions`. Used as a fallback when cookies are blocked.
- **`ddOriginalReferrer`** (sessionStorage) — stores the referer at
  the moment a captcha is triggered, so post-solve redirects can
  return the user to their original page.

### 5.3 Lifecycle

```
1. First visit: no datadome cookie.
   → tags.js generates a random cid (64-hex), POSTs jsData+events
     to api-js.datadome.co/js/.
   → Server scores, returns Set-Cookie: datadome=<encrypted blob>
     in the response. Status: 0 (allowed).

2. Score above threshold:
   → Server returns 403 with HTML body containing inline
     `<script>var dd = { rt:'c', cid, hsh, t:'fe', host:
     'geo.captcha-delivery.com', cookie: '<base64>' }</script>`
   → Tenant page loader injects iframe to
     https://geo.captcha-delivery.com/captcha/?initialCid=<cid>&...

3. User solves captcha:
   → /captcha/check returns { status:0, cookie:'datadome=NEW_VALUE;...' }
   → Iframe postMessages to parent, which writes the new cookie.
   → Parent reloads original URL with new cookie.

4. Subsequent navigation:
   → Browser sends datadome cookie automatically.
   → Server validates blob server-side (decrypts, checks signed
     timestamp + IP binding + score).
   → If still valid: pass through.
   → If TTL expired or IP changed: re-issue (back to step 1) or
     re-challenge (back to step 2).

5. Cookie refresh:
   → On every navigation, server may issue a new datadome value via
     Set-Cookie. The blob includes a rolling timestamp to defeat
     replay across long time windows. [hypothesis based on observed
     ~1y Max-Age but per-request rotation]
```

### 5.4 IP binding

Per the campo Medium analysis: "datadome cookies are unique for every
ip and session". A cookie minted on residential IP A will be rejected
when presented from datacenter IP B. **This is critical for our proxy
strategy** — we must mint and reuse cookies on the same egress IP.

### 5.5 No Privacy Pass / blind tokens

Unlike Cloudflare (which experimented with Privacy Pass blind tokens)
or Kasada (which has a stateful `x-kpsdk-cd` device-trust token),
DataDome does not expose any cryptographic public-key attestation
scheme. The cookie is a plain server-side encrypted blob. **This means
we can't defeat it offline** — every fresh session must hit DataDome's
servers at least once.

---

## 6 — Captcha / interstitial gate

### 6.1 The `dd` object signature

When DataDome blocks, the response body (HTML, status 403) contains:

```html
<script>
var dd = {
  'rt'    : 'c',          // challenge type: 'c'=captcha, 'i'=interstitial
  'cid'   : 'AHrlqAAAAAMAseGpalIFikoAyV2i2w==',  // client id (b64)
  'hsh'   : '14D062F60A4BDE8CE8647DFC720349',     // server-issued hash
  't'     : 'fe',         // 'fe'=normal, 'bv'=IP banned outright
  's'     : <int>,        // numeric param (purpose unclear)
  'b'     : <int>,        // numeric param (purpose unclear)
  'host'  : 'geo.captcha-delivery.com',
  'cookie': '<base64-of-current-datadome-value>'
};
</script>
```

(Source: glizzykingdreko Medium, takionapi docs, capsolver docs.)

If `t === 'bv'`, the IP is hard-banned — rotate proxy first, no point
solving. If `t === 'fe'`, proceed.

### 6.2 The challenge iframe

```
https://geo.captcha-delivery.com/captcha/?initialCid=<cid>&hash=<hsh>
    &cid=<cid>&t=fe&referer=<urlencoded>&s=<s>&e=<e>
```

Or for interstitial:

```
https://geo.captcha-delivery.com/interstitial/?initialCid=<cid>&hash=<hsh>...
```

The iframe loads its own JS bundle from
`https://interstitial.captcha-delivery.com/i.js` (or `/c.js` for
captcha).

### 6.3 Captcha types served

Per the Medium / vendor analyses:

1. **DataDome native slider** — "Swipe right to solve the puzzle".
   This is the default and most common. Visible time ~2.2s for a
   real human (per DataDome's own marketing).
2. **GeeTest-style image slider** — for some tenants, an image with
   a missing piece you slide a slider to align. Same drag-curve
   scoring.
3. **Interstitial / Device Check** — non-interactive. May render a
   blank page or "Verify your device" message while running:
   - Picasso canvas batch.
   - WASM `boring_challenge` — a Rust-compiled WASM blob that runs a
     state-machine loop with bit-twiddling. Seeds 10M-20M iterations,
     uses CPU core count as a hint. Forces measurable CPU time as
     proof of legitimate hardware.
   - Hash chain probe.

DataDome does NOT use FunCaptcha (Arkose) or hCaptcha — those are
separate vendors. (Some tenants overlay hCaptcha on top of DataDome,
but that's a tenant choice, not DataDome itself.)

### 6.4 Solver flow on success

```
POST/GET to https://geo.captcha-delivery.com/captcha/check
  ?cid=<cid>&icid=<initialCid>&ddk=<tenant_key>...
  &payload=<ddCaptchaEncodedPayload>

Response:
  { "status": 0, "cookie": "datadome=NEW_VALUE; Max-Age=31536000;
    Domain=.<site>; Path=/; SameSite=Lax" }
```

The iframe then `window.parent.postMessage`s the new cookie value to
the parent frame, which:
1. Sets `document.cookie = <returned cookie>`.
2. Reloads `window.location` (so the next navigation carries the new
   cookie).

### 6.5 Critical movement-score features

For the slider, 31 derived features over `_initialCoordsList` (mouse
moves between page load and slider grab) and `_coordsList` (slider
drag path). Per glizzykingdreko: "curvature, length, straightness".
A perfectly straight drag from start to end fails. A teleport (no
intermediate coordinates) fails harder.

This is why ghost-cursor tooling is the standard advice — it generates
human-like Bezier paths with realistic acceleration.

---

## 7 — Public RE work to study

Ranked roughly by usefulness for our purposes.

### 7.1 glizzykingdreko ★★★★★

The single most valuable contributor to public DataDome RE.

- **Datadome-Deobfuscator**:
  `https://github.com/glizzykingdreko/Datadome-Deobfuscator` — Babel
  AST-based deobfuscator for `tags.js`. Decodes hex-encoded strings,
  extracts and inlines string-concealing function calls, removes dead
  code, optionally renames identifiers.
- **Datadome-Interstitial-Deobfuscator**:
  `https://github.com/glizzykingdreko/Datadome-Interstitial-Deobfuscator`
  — sister tool for the interstitial challenge JS (`i.js`).
- **datadome-encryption**:
  `https://github.com/glizzykingdreko/datadome-encryption` — clean-room
  reimplementation of the encryption (djb2 hash + non-linear mixer +
  PRNG + dual XOR + custom base64). Documents both Captcha and
  Interstitial parameter constants. **This is what we'd port to Rust
  for our solver crate.**
- **"Breaking Down Datadome Captcha WAF"**:
  `https://medium.com/@glizzykingdreko/breaking-down-datadome-captcha-waf-d7b68cef3e21`
  — long-form writeup covering loop-switch obfuscation, the lookup
  table generator, dynamic key rotation, WASM `boring_challenge`,
  slider scoring, and the dual-XOR encryption stages.

### 7.2 rebrowser ★★★★

Production-grade patches for puppeteer/playwright that specifically
target DataDome+Cloudflare detection.

- **rebrowser-patches**:
  `https://github.com/rebrowser/rebrowser-patches` — disables
  `Runtime.Enable` on every frame; offers three modes (addBinding,
  alwaysIsolated, enableDisable). Also rewrites sourceURL from
  `pptr:...` to generic `app.js`, renames the utility world to `util`.
- **Blog: "How to fix Runtime.Enable CDP detection"**:
  `https://rebrowser.net/blog/how-to-fix-runtime-enable-cdp-detection-of-puppeteer-playwright-and-other-automation-libraries`
- **Blog: "How to scrape seatgeek.com protected by DataDome in 2024"**:
  `https://rebrowser.net/blog/how-to-scrape-seatgeek-com-protected-by-datadome-in-2024`
  — concrete walkthrough on a known-hard target.

**Note for us**: we don't speak CDP at all (no devtools protocol
attached to our V8). This whole class of leak is inherently absent from
browser_oxide. Win.

### 7.3 Hyper-Solutions SDKs ★★★

Commercial-grade Go/Python/Node SDKs that handle DataDome (along with
Akamai, Kasada, Incapsula) without launching a browser.

- **hyper-sdk-go**: `https://github.com/Hyper-Solutions/hyper-sdk-go`
  — exposes `GenerateDataDomeInterstitial()`, `GenerateDataDomeSlider()`,
  `ParseInterstitialDeviceCheckLink()`,
  `ParseSliderDeviceCheckLink()`. Submits to
  `https://geo.captcha-delivery.com/interstitial/` and slider check
  URL.
- **hyper-sdk-py**: same surface, Python.

Code is closed-source-ish (the SDK calls back to their server for the
actual sensor generation). Useful for *reference behavior* — what
endpoints to hit, what cookie to expect — but not for clean-room
reimplementation.

### 7.4 Smaller projects ★★

- **d-suter/datadome-bp**:
  `https://github.com/d-suter/datadome-bp` — a Node service exposing
  `POST /cookie-generator` that returns a fresh `datadome` cookie for a
  target domain. README admits "may not be working anymore as of ~6
  months back." Worth reading the source for the request shape.
- **66niko99/PyDatadome**:
  `https://github.com/66niko99/PyDatadome` — Python lib that takes a
  reCAPTCHA token + UA and constructs the `/check` URL to mint a
  datadome cookie. Limited to reCAPTCHA-based DD tenants (rare
  these days).
- **romainp12/datadome-gen**:
  `https://github.com/romainp12/datadome-gen` — labeled "Datadome
  4.6.0 cookie generator". Likely outdated (current JS tag version
  is 5.x as of 2026) but the request structure is still useful.
- **ellisfan/bypass-datadome**:
  `https://github.com/ellisfan/bypass-datadome` — Go+Python tag.js
  bypass attempt, low maintenance.
- **chris-period/datadome-interstitial**:
  `https://github.com/chris-period/datadome-interstitial` — interstitial-
  specific solver attempt.
- **recaptchaUser/datadome-Interstitial-solver**:
  `https://github.com/recaptchaUser/datadome-Interstitial-solver` —
  thin wrapper around Capsolver. Useful for the URL parameter docs.
- **Fweak/aa49c300a6a09b35e4af3612c8b2a0cb (gist)**:
  `https://gist.github.com/Fweak/aa49c300a6a09b35e4af3612c8b2a0cb` —
  Allegro.pl interstitial walkthrough. Concrete real-world example.

### 7.5 Vendor / scraping-service writeups ★★

These are advertising masquerading as research, but they leak
operational detail:
- **Scrapfly** — `https://scrapfly.io/blog/posts/how-to-bypass-datadome-anti-scraping`
  (covers TLS / behavior / IP categories).
- **ZenRows** — `https://www.zenrows.com/blog/datadome-bypass`
  (gives the dd-object structure and a slider example).
- **Kameleo** — `https://kameleo.io/blog/guide-to-bypassing-datadome`
  (lists the headless artefacts to suppress; emphasises ghost-cursor).
- **Scrapfly bypass page** — `https://scrapfly.io/bypass/datadome`
  (claims 96% success, lists customer site sample).
- **Webparsers** — `https://webparsers.com/bypass-datadome/`.
- **TakionAPI docs** — `https://docs.takionapi.tech/datadome` (paid
  service docs but free reading; describes interstitial vs captcha,
  endpoints).
- **CapSolver / 2Captcha / CapMonster docs** — useful for the URL
  parameter structure (`initialCid`, `hash`, `cid`, `t`, `referer`,
  `s`, `e`).

### 7.6 Vendor self-publications (DataDome blog) ★★★

DataDome's own threat-research posts are often surprisingly explicit
about what they detect:
- **Picasso**: `https://datadome.co/threat-research/the-art-of-bot-detection-picasso-for-device-class-fingerprinting/`
- **CDP / Runtime.enable** (Antoine Vastel, 2024-06):
  `https://datadome.co/threat-research/how-new-headless-chrome-the-cdp-signal-are-impacting-bot-detection/`
- **Detecting Selenium**:
  `https://datadome.co/threat-research/detecting-selenium-chrome/`
- **Detecting puppeteer-extra-stealth**:
  `https://datadome.co/bot-management-protection/detecting-headless-chrome-puppeteer-extra-plugin-stealth/`
- **Headless Chrome detection page**:
  `https://datadome.co/headless-browsers/headless-chrome/`
- **Audio fingerprinting**:
  `https://datadome.co/anti-detect-tools/audio-fingerprint/`
- **TLS fingerprinting**:
  `https://datadome.co/engineering/how-tls-fingerprinting-reinforces-datadomes-protection/`
- **JS tag optimization (perf claims hint at architecture)**:
  `https://datadome.co/engineering/client-side-javascript-tag-optimizations/`

### 7.7 Academic background

- **Picasso original paper (Bursztein et al., Google)**:
  `https://elie.net/static/files/picasso-lightweight-device-class-fingerprinting-for-web-clients/picasso-lightweight-device-class-fingerprinting-for-web-clients-paper.pdf`
  — DataDome's Picasso impl is essentially a productionized version
  of this 2016 paper.

---

## 8 — Per-site notes

### 8.1 leboncoin.fr (FR classifieds)

- **Stack**: Pure DataDome, **not Akamai**. The 2026-01 article from
  AIMGroup
  (`https://aimgroup.com/2026/01/07/leboncoin-works-with-datadome-on-scraping-protection/`)
  and DataDome's own customer story
  (`https://datadome.co/customers-stories/how-leboncoin-blocks-millions-of-malicious-requests-every-day/`)
  confirm Leboncoin chose DataDome **specifically** for scraping
  protection (90% of blocked attempts are scraping).
- **Tier**: HIGHEST. 7+ year customer, 30 protected endpoints, 9.5M
  daily blocks (peak 30M). Per-customer ML model deeply trained on
  Leboncoin's scraper population.
- **What we'd hit**: search and listing pages are protected; the
  homepage may be lighter. Our holistic test hits the root URL — if
  even root is gated, it's the strongest deployment we face.
- **CAPTCHA type**: slider (default DataDome).

### 8.2 yelp.com (US local search)

- **Stack**: not vendor-confirmed in any DataDome case study I could
  find. Attribution comes from third-party scraping articles citing
  the captcha-delivery iframe pattern. **[hypothesis — should be
  validated by inspecting our run log for the actual JS tag URL.]**
- **Tier**: LIKELY MID. Yelp's anti-scraping has historically been a
  mix of in-house heuristics and third-party WAF. If the run log
  shows `js.datadome.co`, confirmed.
- **CAPTCHA type**: slider.

### 8.3 wsj.com (paywall + DataDome)

- **Stack**: Dow Jones (WSJ parent) is a DataDome customer per
  multiple coverage articles. Layered with their own paywall logic
  (which is older and Akamai-fronted). The DD layer protects against
  scrapers harvesting paywalled article text in bulk.
- **Tier**: HIGH (publisher-grade, anti-AI-training-scraper tuning
  publicly emphasized in DataDome's 2025 LLM-crawler-detection
  rollout —
  `https://datadome.co/press/datadomes-2025-global-bot-security-report-exposes-the-ai-traffic-crisis/`).
- **What we'd hit**: even pre-paywall homepage / article landing
  pages serve the JS tag.
- **CAPTCHA type**: slider; possibly interstitial when scoring is
  most suspicious.

### 8.4 etsy.com (e-commerce)

- **Stack**: Vendor-confirmed DataDome customer
  (`https://datadome.co/customers-stories/etsy-stops-unwanted-traffic-reduces-computing-costs-with-datadome-google/`).
  Etsy moved off in-house bot tooling onto DataDome to "reduce
  computing costs."
- **Tier**: MID. The pitch was cost reduction (cheaper bot scrubbing),
  not extreme protection. A clean fingerprint should pass. Search and
  listing endpoints are likely the harder ones.
- **CAPTCHA type**: slider.

### 8.5 Tier ordering by expected difficulty

Easiest → hardest (estimated):
1. **etsy** — DD chosen for cost, baseline tier.
2. **yelp** — moderate, search-driven tenant.
3. **wsj** — high, anti-AI-scraper tuning.
4. **leboncoin** — highest, dedicated 7-year per-tenant ML model.

Implication for our test plan: when we ship a DataDome solver, expect
etsy/yelp to clear first. If wsj passes, leboncoin probably will too.
If leboncoin doesn't pass after solving the cookie, the gap is in
behavioral/IP signals not addressed by the JS layer.

---

## 9 — Recommended fix architecture for browser_oxide

### 9.1 Mirror the Kasada module layout

Existing pattern (per `crates/stealth/src/`):
- `kasada.rs` — the PoW solver, called by `Page::navigate` when a
  Kasada `ips.js` is detected.

Proposed:
- `crates/stealth/src/datadome.rs` — analogous module.

If the implementation grows beyond ~2k LOC (likely, given the
encryption + fingerprint generation + captcha-iframe handling), promote
to its own crate `crates/datadome/` mirroring the `crates/akamai/`
structure.

### 9.2 Where to plug in

`Page::navigate` already has a sequence we can model from:

```
Page::navigate(url) →
  1. Pre-flight TLS / H2 (already done; matches Chrome 147)
  2. HTTP fetch (rquest)
  3. Parse HTML, build DOM
  4. Detect bootstrap scripts (Kasada `ips.js`, Akamai `_bm/_acch.js`)
     [+ NEW: DataDome `tags.js` / `ddjskey` inline]
  5. Run V8 with stealth profile
  6. Watch for sensor POST / token mint
  7. Handle response
```

The DataDome hook attaches at step 4 (detection) and step 6 (sensor
intercept).

### 9.3 Detection logic

Match any of:
- `<script src=".../js.datadome.co/tags.js">` (third-party)
- Inline `window.ddjskey = ...` (universal sentinel — works for
  first-party deployments too)
- 403 response with body containing `var dd = {` and
  `'host':'geo.captcha-delivery.com'`
- iframe with src starting `https://geo.captcha-delivery.com/`

If detected, set a flag on the Page state that downstream layers
(particularly behavior simulation and request retry) can consult.

### 9.4 Module surface

```rust
// crates/stealth/src/datadome.rs (sketch — DO NOT IMPLEMENT NOW)

pub struct DatadomeChallenge {
    pub kind: ChallengeKind,        // None / Captcha / Interstitial / HardBlock
    pub cid: Option<String>,        // current client id
    pub hsh: Option<String>,        // server-issued hash
    pub tenant_key: Option<String>, // ddjskey
    pub host: String,               // captcha-delivery host
}

pub enum ChallengeKind { None, Captcha, Interstitial, HardBlock }

pub fn detect_from_html(html: &str) -> Option<DatadomeChallenge> { ... }
pub fn detect_from_response(resp: &Response) -> Option<DatadomeChallenge> { ... }

pub struct DatadomeSensor {
    // Computed signals — equivalent of the §3.1 table.
    rs_h: u32, rs_w: u32, rs_cd: u32,
    ars_h: u32, ars_w: u32,
    plg: u32, ua: String, lg: String, ...
    events: Vec<EventRecord>,
}

impl DatadomeSensor {
    pub fn from_page(page: &Page) -> Self { ... }
    pub fn encrypt(&self, cid: &str, salt: u32) -> String { ... }
    // ^ Stage 1-4 of §4.3.
}

pub async fn submit(
    sensor: &DatadomeSensor,
    cid: &str,
    tenant_key: &str,
    referer: &Url,
    ua: &str,
    client: &HttpClient,
) -> Result<Cookie, DatadomeError> { ... }
```

### 9.5 Minimum viable scope (Tier-0)

Goal: **silent token issuance only — no captcha solving.**

Requires:
1. Detect the JS tag is present.
2. Build a sensor that scores below threshold (i.e. our fingerprint is
   already good enough that the JS tag would naturally POST and get a
   `status:0` response without challenge).
3. Either:
   - **(Option A — preferred)** Let the actual `tags.js` execute in our
     V8, and ensure all probe surfaces return native-looking values
     (Picasso canvas, audio, navigator props, no Runtime.enable). This
     is mostly an *engine-fingerprint quality* problem, not a *crypto*
     problem.
   - **(Option B — fallback)** Bypass the JS tag and POST a hand-built
     sensor directly. Requires us to port the encryption (§4.3) and
     keep up with daily key rotation. Higher maintenance burden.

For the holistic-sweep target (4 sites passing silently), **Option A
is correct.** We don't need a captcha solver if our fingerprint scores
clean.

### 9.6 Where Option A is likely to fail today

Based on docs in this repo:
- **`docs/PHASE6_FINGERPRINT_INVENTORY_FINDINGS_2026_04_29.md`** — WebGL
  parity gaps (probably the biggest single Picasso-related risk).
- **`docs/CHROME_JS_SURFACE_PARITY_2026_04_29.md`** — JS surface
  inventory; note any natives whose `toString` we don't mask.
- **`docs/CANADA_GOOSE_DIAGNOSIS_2026_05_10_PART2.md`** — the
  `unjzomuybtbyyhwwkdpkxomylnab`-undefined-receiver class. DataDome
  may have an analogous probe set.
- Behavior surface — DataDome's behavioral coord scoring expects
  `_initialCoordsList` populated. If our `crates/stealth/src/behavior.rs`
  doesn't synthesize at least a few mouse moves before any
  click-equivalent event, the empty list itself is a fail signal.

---

## 10 — What can be done WITHOUT a captcha-solving service

### 10.1 The line

DataDome's CAPTCHA is **only triggered when scoring exceeds a
threshold.** Below threshold, the JS tag silently POSTs and gets a
clean cookie. Two pieces of evidence:
1. DataDome's marketing — "<0.01% of human requests see a captcha"
   (`https://datadome.co/products/datadome-captcha/`). Captcha is the
   exception, not the norm.
2. The cookie-flow analysis (§5) shows step 1 (clean POST → cookie)
   and step 2 (challenge) are alternates, not a sequence. A clean
   score skips the challenge entirely.

So the question reduces to: **can we score clean enough to skip the
captcha?**

### 10.2 Evidence that fingerprint-only is sufficient (for some
deployments)

- Real residential Chrome users *never* see a DataDome captcha on
  Etsy, Yelp, WSJ, Leboncoin — these are mass-market consumer sites
  where any visible challenge would tank conversion. DataDome tunes
  thresholds accordingly.
- Camoufox / Kameleo / Brave / undetected-chromedriver routinely pass
  Etsy/Yelp on residential IPs without a solver
  (`https://kameleo.io/blog/guide-to-bypassing-datadome`,
  `https://scrapfly.io/blog/posts/how-to-bypass-datadome-anti-scraping`).
  These are *just headless browsers with good fingerprint masking* —
  no proprietary captcha solving.
- Per Scrapfly, "Combining stealth tooling with residential proxies and
  warm-up navigation … improves success rates" → no solver
  involvement.

### 10.3 Evidence that fingerprint-only is NOT sufficient (for top-tier
deployments)

- Leboncoin specifically: per-customer ML model trained on years of
  scraper traffic. Even with Camoufox + residential IP, single-page
  hits get challenged. The fix per Kameleo: long warm-up nav (homepage
  → category → listing → detail), and `ghost-cursor` for mouse paths.
  Even then "no method is guaranteed."
- DataDome 2025 update introduced **intent-based scoring** — analyzes
  the *purpose* of the visit, not just the device. A lone GET to a
  search results URL with no prior browsing is itself a signal.
  (`https://datadome.co/press/datadomes-2025-global-bot-security-report-exposes-the-ai-traffic-crisis/`)

### 10.4 Practical line for browser_oxide

Without a captcha solver we can probably clear:
- ✅ etsy (DD-mid)
- ✅ yelp (DD-mid, IF the run log confirms DD)
- ⚠ wsj (DD-high; depends whether anti-LLM-crawler scoring is on)
- ❓ leboncoin (DD-highest; per-tenant ML may bite even with perfect
  fingerprint, but rendering the homepage should be doable — the
  category/search pages are the harder targets)

The structural advantages of browser_oxide (no CDP, real V8 JIT, real
HTML/CSS render stack) buy us a free anti-detection win on the
*automated-browser* axis. The Picasso/audio class fingerprint is the
**main remaining risk**, and it's an engine-quality problem (Skia
parity, GPU vendor strings, codec table parity) rather than a
crypto-protocol problem.

### 10.5 If we hit the captcha anyway

Two options:
1. **Defer to a paid solver** (Capsolver, 2Captcha, CapMonster) — out
   of scope for the open-source repo but trivial to integrate as an
   optional crate behind a feature flag.
2. **Build our own slider solver** — the slider is image-based; we'd
   need OpenCV-style template matching to find the puzzle slot, plus
   our own ghost-cursor curve generator. Doable but several weeks of
   work; not worth it unless we hit the captcha consistently.

### 10.6 Recommendation

**Tier-0 only**: ship a detection + fingerprint-quality fix, no
solver. Re-test holistic sweep. Sites that still fail get logged
separately for a future Tier-1 (slider solver) effort. Captcha
solving is a separate research track that should not block Tier-0.

---

## 11 — Action plan with time estimates

Ordered as a single dev (you) would tackle it.

### Step 1 — Capture the artifact (T+0, ~2 hours)

Re-run the holistic_sweep against leboncoin.fr (cheapest of the four,
fastest reproduction). Record:
- Full HTTP transcript (URLs, request/response headers, status codes).
- The actual `tags.js` body served (save to
  `docs/datadome_sensor_analysis/<date>_leboncoin_tags.js`).
- The 403 response HTML containing the inline `dd = {…}` object.
- Any `geo.captcha-delivery.com` URLs the page tried to load.
- Our V8 console output during the sensor run.

Repeat for etsy / yelp / wsj — each tenant's `tags.js` may differ
slightly (key dictionary).

**Deliverable**: `docs/datadome_sensor_analysis/{site}_capture/` per
target.

### Step 2 — Deobfuscate the tags.js (~1 day)

Run `glizzykingdreko/Datadome-Deobfuscator` on the captured scripts.
Walk the output by hand. Identify:
- The signal-key dictionary (today's six-char names → human meaning).
- The exact set of probes (which navigator props, canvas operations,
  WebGL probes, CDP detection paths).
- Any tenant-specific behavior (some keys differ by `ddjskey`).

**Deliverable**:
`docs/datadome_sensor_analysis/{date}_tagsjs_decoded.md` — analogous
to `docs/akamai_sensor_reference_2026_04_29.txt`.

### Step 3 — Identify our engine probe failures (~1-2 days)

Cross-reference the deobfuscated probe list against:
- `docs/CHROME_JS_SURFACE_PARITY_2026_04_29.md`
- `docs/PHASE6_FINGERPRINT_INVENTORY_FINDINGS_2026_04_29.md`
- `docs/CANADA_GOOSE_DIAGNOSIS_2026_05_10_PART2.md`

For each probe DataDome runs, check what our engine returns and
whether it matches Chrome 147. Build a table:

| DD probe                         | Chrome 147 returns        | Our engine returns        | Gap   |
|----------------------------------|---------------------------|---------------------------|-------|
| Picasso canvas hash (seed=…)     | <hash A>                  | <hash B or null>          | YES   |
| navigator.languages.length       | 3                         | 3                         | NO    |
| Function.prototype.toString.call(Notification.requestPermission) | "function ..." | …  |       |
| ...                              |                           |                           |       |

**Deliverable**:
`docs/datadome_sensor_analysis/probe_gap_matrix.md`.

### Step 4 — Fix engine gaps (~3-7 days, scope-dependent)

This is the bulk of the work and the *real* fix. Touch points:
- `crates/canvas` for Picasso parity (likely most expensive — Skia
  rasterization differences).
- `crates/dom` / V8 bindings for any missing JS surface.
- `crates/stealth/src/behavior.rs` for synthetic mouse coord
  generation (ghost-cursor analog).
- Any `Function.toString` masks we missed.

Each gap is independently small; the count is what makes this
multi-day. **No new crate needed yet** — this is a fingerprint
quality push, not a protocol push.

### Step 5 — Build detection layer (~0.5 day)

Add `crates/stealth/src/datadome.rs` with:
- `detect_from_html`
- `detect_from_response`
- A `DatadomeChallenge` struct on `Page` state.

Wire into `Page::navigate` step 4. Used by:
- The retry layer (so we don't naively retry into a 403 loop).
- The behavior simulator (to know when to be extra-careful with
  coord lists).

**Deliverable**: ~200 LOC new module, ~20 LOC of `Page::navigate`
glue, integration test that detects the leboncoin 403 without
attempting to bypass.

### Step 6 — Behavioral coord enrichment (~1 day)

Ensure that by the time the DataDome JS would POST, the page has
seen at least:
- 5-10 `mousemove` events on a non-trivial Bezier path.
- 1-2 `scroll` events.
- Realistic timestamps (gaps of 10-50ms, not all zero).

This is purely engine-side; no crypto involved. The behavior crate
already exists — extend it for the DataDome event-array shape.

### Step 7 — Re-run holistic sweep (~0.5 day)

Confirm:
- etsy passes.
- yelp passes.
- wsj passes (or not — log evidence).
- leboncoin passes (or not — log evidence).

If 3-4 pass, ship Tier-0. If only 1-2 pass, regroup.

### Step 8 — (Conditional) Tier-1 sensor solver (~2-3 weeks)

ONLY if step 7 fails on > 2 sites. Then we'd:
- Port glizzykingdreko's encryption to Rust
  (`crates/datadome/src/encryption.rs`).
- Implement the daily-key-dictionary scraper that runs on
  `tags.js` captures.
- Build the HTTP client for `api-js.datadome.co/js/`.
- Hand-build a sensor that posts a clean fingerprint and consumes
  the `Set-Cookie: datadome=…` response.

This **bypasses** running the JS tag at all — we POST as if we were
the tag. Saves the V8 round-trip but introduces a reverse-engineering
maintenance burden (daily for key rotation, weekly for tag updates).

### Step 9 — (Conditional) Tier-2 captcha solver (~weeks)

ONLY if step 8 still leaves us captcha'd. Slider solver: OpenCV
template matching + ghost-cursor curves. Or hard pivot to an external
solver service (out of scope for the OSS repo).

### Total time estimate (Tier-0 only)

**~7-12 working days end to end.** Most of it is Step 4 (engine gaps),
which is incremental work that compounds with our existing Akamai +
Kasada fingerprint hardening. Steps 1-3 are one-shot research; Step 5
is small; Steps 6-7 are routine.

---

## 12 — Open questions / what we still need to capture

Things this doc can't answer without live capture:

1. **Which exact `tags.js` version is leboncoin / etsy / yelp / wsj
   serving?** Tag version 4.6.x vs 5.x has different signal sets.
2. **Is wsj/yelp confirmed-DataDome via `js.datadome.co`?** Need to
   grep the run log.
3. **What does our V8 actually return for `navigator.webdriver`,
   `chrome.runtime`, the Picasso canvas seed, the codec probe?** —
   these are the answers Step 3 produces.
4. **Are any of the four target sites running first-party (proxied)
   tag deployments?** If so, the URL pattern won't be
   `js.datadome.co` and our detection has to fallback to `ddjskey`
   sentinel.
5. **Does the leboncoin 403 carry `t='fe'` or `t='bv'`?** If `'bv'`,
   our IP is the problem (datacenter-flagged), not the fingerprint —
   different fix path entirely.
6. **What's the `ddv` version we should claim?** Latest stable JS tag
   version per the changelog
   (`https://docs.datadome.co/changelog/javascript-tag-201`).

These answers come from the Step 1 capture pass. After that, this
doc becomes a workplan; before that, it's a hypothesis space.

---

## Appendix A — Quick reference URLs

- `https://js.datadome.co/tags.js` — third-party JS tag
- `https://api-js.datadome.co/js/` — sensor POST endpoint (web)
- `https://api-sdk.datadome.co/sdk/` — sensor POST endpoint (mobile)
- `https://geo.captcha-delivery.com/captcha/` — captcha iframe (web)
- `https://geo.captcha-delivery.com/interstitial/` — interstitial iframe
- `https://geo.captcha-delivery.com/captcha/check` — slider check
- `https://interstitial.captcha-delivery.com/i.js` — interstitial JS
- `https://ct.captcha-delivery.com/...` — captcha-related CDN
- Cookie name: `datadome`
- Inline tenant key: `window.ddjskey`
- Inline 403 marker: `var dd = { rt: ..., cid: ..., host: 'geo.captcha-delivery.com' }`
- CSP allowlist DataDome itself documents:
  - `script-src js.datadome.co ct.captcha-delivery.com`
  - `connect-src api-js.datadome.co`
  - `frame-src *.captcha-delivery.com`
  - `worker-src blob:`

## Appendix B — Comparison cheatsheet (Kasada vs Akamai vs DataDome)

| Dimension                | Kasada                    | Akamai BMP            | DataDome                       |
|--------------------------|---------------------------|-----------------------|--------------------------------|
| Bootstrap                | `[uuid]/ips.js`           | `_bm/_acch.js` etc.   | `js.datadome.co/tags.js`       |
| Submission URL           | `/tl` on tenant           | tenant-specific path  | `api-js.datadome.co/js/`       |
| Cookie name              | `KP_UIDz` / `x-kpsdk-ct`  | `_abck`, `bm_sz`      | `datadome`                     |
| Cookie validity          | seconds-minutes           | minutes-hours         | 1 year (rolling)               |
| JS protection            | Custom VM, ~60 opcodes    | Heavy obfuscation     | Heavy obfuscation, no VM       |
| Daily key rotation       | Bytecode (~hourly)        | Per-tenant            | Signal keys (~daily)           |
| PoW                      | Yes (CPU-bound)           | No (challenge token)  | Yes for interstitial (WASM)    |
| Picasso canvas           | No                        | No                    | Yes (signature feature)        |
| Behavior scoring         | Light                     | Heavy                 | Heavy (31 features)            |
| CDP detection            | Yes                       | Yes                   | Yes (publicized 2024-06)       |
| Per-tenant ML            | Some                      | Some                  | Heavy (85k+ models)            |
| Captcha provider         | Native cliffhanger        | Akamai own + reCAPTCHA | DD slider + GeeTest + interstitial |
| Open-source RE quality   | Excellent (nullpt.rs)     | Good (Akamai-bypass)  | Very good (glizzykingdreko)    |
| Browser_oxide status     | Solver implemented        | Sensor analysis done  | **Nothing yet** — this doc     |

## Appendix C — Sources cited

- `https://datadome.co/`
- `https://datadome.co/customers/`
- `https://datadome.co/customers-stories/`
- `https://datadome.co/customers-stories/etsy-stops-unwanted-traffic-reduces-computing-costs-with-datadome-google/`
- `https://datadome.co/customers-stories/how-leboncoin-blocks-millions-of-malicious-requests-every-day/`
- `https://datadome.co/products/datadome-captcha/`
- `https://datadome.co/press/datadomes-2025-global-bot-security-report-exposes-the-ai-traffic-crisis/`
- `https://datadome.co/threat-research/the-art-of-bot-detection-picasso-for-device-class-fingerprinting/`
- `https://datadome.co/threat-research/how-new-headless-chrome-the-cdp-signal-are-impacting-bot-detection/`
- `https://datadome.co/threat-research/detecting-selenium-chrome/`
- `https://datadome.co/bot-management-protection/detecting-headless-chrome-puppeteer-extra-plugin-stealth/`
- `https://datadome.co/headless-browsers/headless-chrome/`
- `https://datadome.co/anti-detect-tools/audio-fingerprint/`
- `https://datadome.co/engineering/how-tls-fingerprinting-reinforces-datadomes-protection/`
- `https://datadome.co/engineering/client-side-javascript-tag-optimizations/`
- `https://docs.datadome.co/docs/javascript-tag`
- `https://docs.datadome.co/docs/how-to-configure-the-javascript-tag`
- `https://docs.datadome.co/docs/cookie-session-storage`
- `https://docs.datadome.co/docs/sdk-ios-cookies`
- `https://docs.datadome.co/changelog/javascript-tag-201`
- `https://github.com/glizzykingdreko/Datadome-Deobfuscator`
- `https://github.com/glizzykingdreko/Datadome-Interstitial-Deobfuscator`
- `https://github.com/glizzykingdreko/datadome-encryption`
- `https://medium.com/@glizzykingdreko/breaking-down-datadome-captcha-waf-d7b68cef3e21`
- `https://github.com/rebrowser/rebrowser-patches`
- `https://rebrowser.net/blog/how-to-fix-runtime-enable-cdp-detection-of-puppeteer-playwright-and-other-automation-libraries`
- `https://rebrowser.net/blog/how-to-scrape-seatgeek-com-protected-by-datadome-in-2024`
- `https://github.com/Hyper-Solutions/hyper-sdk-go`
- `https://github.com/d-suter/datadome-bp`
- `https://github.com/66niko99/PyDatadome`
- `https://github.com/romainp12/datadome-gen`
- `https://github.com/ellisfan/bypass-datadome`
- `https://github.com/chris-period/datadome-interstitial`
- `https://github.com/recaptchaUser/datadome-Interstitial-solver`
- `https://gist.github.com/Fweak/aa49c300a6a09b35e4af3612c8b2a0cb`
- `https://hackmd.io/@s2Q4nEh2Shu1POK_sxsPnA/B1vsX2v8o`
- `https://medium.com/@mayankchandel2567/exploring-methods-of-evading-datadomes-bot-protection-a-comprehensive-guide-for-2023-ef5274ee1698`
- `https://medium.com/@cerealautomation/how-to-obtain-datadome-cookies-for-the-too-good-to-go-api-47bd661c191e`
- `https://medium.com/@campo1312/how-to-detect-block-and-manage-datadome-c6e94c74a4f4`
- `https://scrapfly.io/blog/posts/how-to-bypass-datadome-anti-scraping`
- `https://scrapfly.io/bypass/datadome`
- `https://www.zenrows.com/blog/datadome-bypass`
- `https://kameleo.io/blog/guide-to-bypassing-datadome`
- `https://docs.takionapi.tech/datadome`
- `https://docs.takionapi.tech/datadome/apis`
- `https://docs.capsolver.com/en/guide/captcha/datadome/`
- `https://docs.capmonster.cloud/docs/captchas/datadome/`
- `https://2captcha.com/p/datadome-captcha-solver`
- `https://www.wappalyzer.com/technologies/security/datadome/`
- `https://aimgroup.com/2026/01/07/leboncoin-works-with-datadome-on-scraping-protection/`
- `https://elie.net/static/files/picasso-lightweight-device-class-fingerprinting-for-web-clients/picasso-lightweight-device-class-fingerprinting-for-web-clients-paper.pdf`

---

*End of document. ~900 lines markdown. No source code modified.*
