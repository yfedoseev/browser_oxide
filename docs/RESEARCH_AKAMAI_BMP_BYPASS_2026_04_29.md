# Research — Akamai Bot Manager bypass landscape (2026-04-29)

> Public information, code, and reverse-engineering writeups for getting
> through Akamai Bot Manager Premier (web) and BMP (mobile). Compiled to seed
> an eventual `crates/akamai/` solver crate for browser_oxide. Built by
> web-searching GitHub, Substack, Medium, Scrapfly/ZenRows blogs, and
> pricing pages — no paid services subscribed.
>
> **Context:** browser_oxide already matches Chrome 147 byte-for-byte at
> TLS+H2+JS fingerprint (99% probe parity) and detects `_abck` cookies.
> What we DON'T do: ship the `sensor_data` POST to `/_bm/_data` with
> behavioural telemetry + browser fingerprint that Chrome sends to
> upgrade `_abck` from unfavorable (`~0~-1~`) to favorable
> (`~-1~-1~-1~`). This is the gap to close for bestbuy.com,
> homedepot.com, and walmart.com.

---

## 1 — Akamai's two product lines (don't mix them up)

| Product | Endpoint | Surface | Status in our pipeline |
|---|---|---|---|
| **Akamai BM web** | `/_sec/cp_challenge/verify` or similar; sets `_abck`, `bm_sz` | browser-side `sensor_data` POST | what we need for bestbuy/homedepot/walmart |
| **Akamai BMP (mobile)** | sets `x-acf-sensor-data` header | iOS/Android SDK | not relevant — separate codebase |

Several open-source projects target one or the other; **make sure you
pick the right one before reading code**. `xvertile/akamai-bmp-generator`
(336 stars) is the most popular but it's **mobile BMP only**. The web
implementations have lower star counts but are what we need.

---

## 2 — Open-source projects (web sensor_data)

### 2.1 xiaoweigege/akamai2.0-sensor_data (★★★★★ — highest priority)

**URL**: <https://github.com/xiaoweigege/akamai2.0-sensor_data>
**Stars**: 114 · **Forks**: 25 · **Language**: JavaScript
**Targets**: Akamai web v2 AND v3
**License**: not specified (treat as "research-only")

> "Akamai has updated to version 3, and my API now supports version 3 of Akamai."

Generates the four web tokens we need:
- `sensor_data` POST body
- `_abck` cookie (returned on submission)
- `akamai-bm-telemetry` header (base64-derived from sensor_data)
- `sbsd` and `bm_s` (newer 2024+ tokens)

> "The sensor_data parameter is formed by concatenating and encrypting a 58-element array."

> "the canvas fingerprint and motion trajectory were the most vital components."

Files: `akamai2.0.js`, `akamai3.0.js`. **This is the most useful single
repo on the list — port the algorithm verbatim into Rust.** Active enough
to track v3 (which is what bestbuy/homedepot use).

### 2.2 Edioff/akamai-analysis (★★★★ — best documentation)

**URL**: <https://github.com/Edioff/akamai-analysis>
**License**: MIT
**Languages**: JavaScript + Python

> "This repository contains a detailed technical case study of Akamai Bot Manager v2's detection mechanisms. The project reverse-engineered 512KB of obfuscated JavaScript to document the complete bot detection pipeline."

Documents:
- Signal collection (100+ browser/device/behavior signals across 7 categories)
- Sensor data generation and encoding methodology
- Detection pipeline flow (page load → server validation)
- String obfuscation patterns (runtime decryption of 500+ strings)
- Cookie lifecycle for `_abck`

Notes timing traps: *"The script measures execution time of certain operations to detect if code is being debugged or running in a non-standard environment."*
And canvas: *"Renders specific text and gradients to a canvas element and hashes the result."*

**Caveat**: v2 only — v3 has different field set + crypto. But the
**signal taxonomy** transfers directly.

### 2.3 glizzykingdreko's writeup + helper module (★★★★ — best v3 algorithm documentation)

**Article**: <https://medium.com/@glizzykingdreko/akamai-v3-sensor-data-deep-dive-into-encryption-decryption-and-bypass-tools-da0adad2a784>
**Code**: <https://github.com/glizzykingdreko/akamai-v3-sensor-data-helper> (npm: `akamai-v3-sensor-data-helper`, MIT, JavaScript)
**Web app**: <https://akamai-v3-tools.vercel.app/>

The article documents the **v3 encryption algorithm** end-to-end:

> Two-step transformation:
> **(1) Element Shuffling** — "The JSON payload data is first converted into a colon-delimited string. Elements within this string are shuffled using a pseudo-random number generator (PRNG), initialized with a unique file hash extracted from Akamai's JavaScript."
>
> **(2) Character Substitution** — "After shuffling, each character in the string is substituted with another character. This substitution uses another PRNG seeded with a cookie-derived hash, typically from the bm_sz cookie."

Key facts from the article:
- **Not XOR / AES / base64** — pure PRNG-based permutation + substitution.
- **Cookie hash** defaults to `8888888` on the first request (before any `bm_sz` is set), then derives from the `bm_sz` cookie returned after the first sensor_data POST.
- **JS-file hash** must be extracted dynamically from Akamai's JS via Babel AST (different per deployment, rotates).
- Full round-trip is **encrypt(payload, cookie_hash, file_content)** ↔ **decrypt(sensor_data, file_content)**.

The helper module (`akamai-v3-sensor-data-helper`) is **the encryption layer only** — it doesn't know the field set / order of `payload`. That's what xiaoweigege/akamai2.0-sensor_data and Edioff/akamai-analysis fill in.

**Together, these three sources cover ~80% of the v3 implementation.**

### 2.4 klenne/akamai-sensor-data-tools (★★★ — parsing/analysis only)

**URL**: <https://github.com/klenne/akamai-sensor-data-tools>
**Web app**: <https://sensor-data-tools.netlify.app/>
**Language**: JavaScript + TypeScript
**Targets**: v2

> "This Application can parse akamai sensor data payloads, and deobfuscate most of the script"

Useful for **parsing captured sensor_data strings** — i.e., diff-debugging
when our generator produces something that doesn't match a captured
real-Chrome reference. NOT a generator.

### 2.5 DalphanDev/akamai-sensor (★★★)

**URL**: <https://github.com/DalphanDev/akamai-sensor>
**Status**: TBD — needs separate fetch.

> "Simple script to help reverse engineer akamai's sensordata payload."

### 2.6 cirleamihai/akamai-1.7-cookie-generator (★★ — legacy)

**URL**: <https://github.com/cirleamihai/akamai-1.7-cookie-generator>
**Targets**: Akamai v1.7

> "Requests Based Akamai v1.7 Generator. Can be used with Custom Clients that share the same Interface as Requests module."

Legacy version — useful only if a target site is still on v1.7.

### 2.7 i7solar/Akamai (★★ — legacy, Go)

**URL**: <https://github.com/i7solar/Akamai>
**Targets**: v1.7X
**Language**: Go

> "Akamai 1.75 Cookie Generator for _abck and ak_bmsc"

**Useful pattern**: this is the closest precedent for a **systems-language**
implementation of an Akamai cookie generator. If we go Path B below
(port to Rust), the Go code structure is the natural blueprint.

### 2.8 FRIS-Solutions-Vault/akamai-sdk-go (★★)

**URL**: <https://pkg.go.dev/github.com/FRIS-Solutions-Vault/akamai-sdk-go>
**Status**: TBD — Go package on pkg.go.dev. Probably another commercial-SDK shape.

---

## 3 — Open-source projects (mobile BMP — for reference only)

### 3.1 xvertile/akamai-bmp-generator (★★★★ for mobile, NOT for our use case)

**URL**: <https://github.com/xvertile/akamai-bmp-generator>
**Stars**: 336 · **Forks**: 118 · **Language**: Go (99.3%)
**License**: not specified
**Targets**: BMP (mobile) versions 3.3.4, 3.3.1, 3.3.0, 3.2.3, 3.1.0, 2.2.3, 2.2.2, 2.1.2

> "Generate sensor data for Akamai's Bot Management Protocol (BMP) to bypass bot detection."
> "2K unique device fingerprints"
> "Proof of Work (PoW) support"

Run pattern: `cd /server && go run main.go`. Example uses
`com.ihg.apps.android` — **Android-only**. No web equivalent in this
repo.

**Why we still care**: the Go implementation is well-structured. If we
ever target Akamai's mobile flow, this is the blueprint. Skip otherwise.

### 3.2 reverse-god/akamai-sensordata (mobile BMP, x-acf-sensor-data)

**URL**: <https://github.com/reverse-god/akamai-sensordata>

> "Generates Akamai x-acf-sensor-data header for all mobile application, accross all websites."

Mobile-only.

### 3.3 dawud-outsystems/AkamaiBPMCordovaPlugin

**URL**: <https://github.com/dawud-outsystems/AkamaiBPMCordovaPlugin>

Cordova/Ionic native plugin wrapping Akamai's actual BMP SDK. Useful if
we ever embed Cordova; not applicable to a pure browser engine.

---

## 4 — Public protocol facts (web sensor_data)

### 4.1 Request flow

1. Initial GET to a protected page → Akamai sets `_abck=...~-1~-1~-1~` (NEW favorable) or `~0~-1~` (unfavorable).
2. Page loads; Akamai's challenge JS runs, builds a sensor_data payload from 100+ signals, POSTs to `/_sec/cp_challenge/verify` (or similar; path varies per tenant) with body `{"sensor_data":"<encoded>"}`.
3. Response refreshes `_abck` to favorable form (`~-1~-1~-1~`), sets / refreshes `bm_sz`, and may set `ak_bmsc`.
4. Subsequent requests carry favorable `_abck` and Akamai accepts them.
5. After ~1 hour or N requests Akamai may demand another sensor_data POST to refresh.

If sensor_data is not accepted, the `_abck` suffix flips to one of:
- `~-1~0~-1~` — server demands a sec-cpt PoW challenge.
- `~0~-1~-1~` — sensor_data invalid.
- `~-1~-1~0~-1~` — pixel challenge required.

### 4.2 sensor_data payload (v3)

Per glizzykingdreko + xiaoweigege, the v3 payload is a colon-delimited
concatenation of ~58 fields, then PRNG-shuffled, then PRNG-substituted.
Field categories (per Edioff/akamai-analysis taxonomy):

1. **Browser identity** — UA, navigator props, plugins, mimeTypes, languages.
2. **Hardware** — screen.{width,height,colorDepth,availWidth,availHeight,availTop}, deviceMemory, hardwareConcurrency.
3. **Canvas fingerprint** — render specific text + gradient, hash. *"The most vital component."*
4. **WebGL** — UNMASKED_VENDOR_WEBGL, UNMASKED_RENDERER_WEBGL, extensions, parameters.
5. **AudioContext** — fingerprint hash from offline buffer.
6. **Font enumeration** — measureText widths against a known string in many fonts.
7. **Behavioural** — mouse trajectory + keystrokes + touch events captured during the page session.
8. **Anti-debug** — `Function.prototype.toString.call(...)` checks, `eval`/`Function` access detection, performance-counter timing of "fingerprintable operations" vs a known baseline.

Behavioural data is the second-most-vital. Akamai weights canvas + motion-trajectory most heavily; many bypasses fail because they ship a static fingerprint with no mouse trajectory.

### 4.3 _abck cookie suffix decoding

Format: `<token>~<n1>~<n2>~<n3>~<n4>~<n5>` (number of fields varies).

| Suffix | Meaning |
|---|---|
| `~-1~-1~-1~` | favorable / passed |
| `~0~-1~-1~` | sensor_data invalid |
| `~-1~0~-1~` | sec-cpt PoW required |
| `~-1~-1~0~` or `~-1~-1~0~-1~` | pixel challenge required |
| `~0~0~0~` | hard block |

(Exact suffix semantics vary by version; treat as documentation, not contract.)

### 4.4 Cookie lifecycle

- `_abck` — primary trust cookie, ~1h TTL, refreshes on every successful sensor_data POST.
- `bm_sz` — sandbox/session-id cookie, used as PRNG seed for v3 character substitution.
- `ak_bmsc` — long-term Akamai session cookie (older v1/v2 mechanism, still set on some tenants).
- `sbsd`, `bm_s` — newer (2024+) tokens, mentioned in xiaoweigege; protocol partially documented.

---

## 5 — Commercial bypass services (the ceiling we'd undercut)

### 5.1 Hyper-Solutions / hypersolutions.co

**Repos**: <https://github.com/Hyper-Solutions/hyper-sdk-py>, <https://github.com/Hyper-Solutions/hyper-sdk-js>, <https://github.com/Hyper-Solutions/hyper-sdk-go>
**Pricing**: Pay-as-you-go + subscription tiers (rates not public).
**Architecture**: SDKs (MIT) are clients to a paid backend. Akamai
support is feature-complete: *"sensor data, sec-cpt challenges, pixel
challenges, cookie validation"*.

If we go Path C below, this is the cleanest API to wire up.

### 5.2 Scrapfly — Bypass Akamai Bot Manager (97% Success)

**URL**: <https://scrapfly.io/bypass/akamai>
Scraping API. ~97% claimed success rate. Same paid-API shape.

### 5.3 ZenRows — How to Bypass Akamai

**URL**: <https://www.zenrows.com/blog/bypass-akamai>
Marketing + 3 methods overview (TLS impersonation + scraping API + headless browser farms).

### 5.4 Bright Data, Oxylabs, Capmonster, ScraperAPI

All advertise Akamai bypass. None publish algorithm-level details.

---

## 6 — Adjacent prior art

### 6.1 scrapy-impersonate + Pierluigi Vinciguerra's writeup (★★★ — useful baseline)

**URL**: <https://substack.thewebscraping.club/p/bypassing-akamai-for-free>
**Date**: 2025-03-23 by Pierluigi Vinciguerra.

Argues TLS-impersonation alone (`scrapy-impersonate` with
`'impersonate': 'chrome110'`) **passes Akamai 90% of the time** without
any sensor_data work. Demonstrated on Gucci.

> "If the sensor_data is generated more realistically, the pass rate is very high, bypassing Akamai's system in less than a second. If the confidence is not very high, it will slow down, and it might take up to 10 seconds to pass."

**Implication for us**: our TLS+H2 fingerprint is byte-exact Chrome 147,
which means we should ALREADY be in the 90% no-sensor-data bracket for
weakly-protected Akamai sites. The 2 sites we fail (bestbuy, homedepot)
are in the harder 10% that demand sensor_data. The other ~13 Akamai
sites in the holistic sweep currently PASS for us.

### 6.2 Camoufox / Patchright

Patchright's per-engine patches include Akamai-specific interventions
(Function.toString unmask, prototype-pin restoration, etc.). Read
their patch list at <https://github.com/Kaliiiiiiiiii-Vinyzu/patchright>
for individual probe interventions we may have missed.

### 6.3 xiaoweigege Medium writeup

**URL**: <https://medium.com/@240942649/decoding-akamai-2-0-418e7c7fa0a0>
Companion article to the v2 GitHub repo above. Same author. v2 focus.

### 6.4 Akamai's own marketing pages

- <https://www.akamai.com/products/bot-manager>

---

## 7 — Realistic implementation paths for browser_oxide

Ranked by ROI:

### 7.1 Path A: ride glizzykingdreko + xiaoweigege port (5–10 days)

1. Read the v3 algorithm from glizzykingdreko (article + npm helper).
2. Read the field set from xiaoweigege/akamai2.0-sensor_data.
3. Read the signal taxonomy from Edioff/akamai-analysis.
4. Port to Rust as `crates/akamai/`. Expected size: ~1500 LOC.
5. Hook into our existing `humanize.js` mouse-trajectory capture to feed the **behavioural** part of the payload.
6. Wire in our `WebGL`/`canvas`/`audio` fingerprint state for the **device** part.
7. POST to `/_sec/cp_challenge/verify` (or whatever path Akamai's challenge JS targeted) and cycle `_abck` until favorable.
8. Re-test bestbuy / homedepot.

**Risk**: Akamai rotates field set + JS-file hash regularly. Need a way
to either dynamically extract the file hash via AST (Babel) or detect
Akamai's specific challenge JS version on each request. Roughly weekly
maintenance.

### 7.2 Path B: port the existing Go BMP generator structure (web variant)

1. Take xvertile/akamai-bmp-generator as code-architecture blueprint.
2. Replace mobile BMP packet shape with web sensor_data shape.
3. Same Rust port effort as Path A, ~1500 LOC.

**Trade-off**: same end-state as Path A, just sourced from a different
reference. Worth using if Path A's references go stale.

### 7.3 Path C: pay Hyper-Solutions for the sensor_data API (operational, immediate)

`hypersolutions.co` returns valid sensor_data + cookies given our
session. Engineering cost: thin Rust client (~100 LOC) + API key.

**Trade-off**: ongoing per-request cost; not "best stealth engine ever"
but the best operational option to ship today.

### 7.4 Path D: accept and move on

Akamai-CHL is 2 sites of 126. The other ~13 Akamai-protected sites in
the sweep already pass via our TLS+H2 alone (consistent with Pierluigi's
"90%" claim). Recover the last 2 later when Path A or C is justified.

---

## 8 — Open follow-ups for whoever picks this up

- **Capture a real Chrome 147 sensor_data POST** against bestbuy.com via Playwright MCP. The body of `{"sensor_data": "<x>"}` is exactly what we need to produce. Feed it through `glizzykingdreko/akamai-v3-sensor-data-helper` to **decrypt** it back to a JSON payload — that gives a known-good template our generator must match.
- **Diff our environment vs that decrypted payload**. Most fields we already have (canvas, WebGL, audio, navigator props). The deltas tell us where to add behavioural state.
- **Stand up a local copy of `akamai-v3-tools.vercel.app`** (the glizzykingdreko web app) for round-tripping during development.
- **Read Patchright's Akamai-specific patches** to learn what tightening Camoufox already shipped.

---

## 9 — Sources (verbatim URLs)

### Web sensor_data (priority for our use case)
- <https://github.com/xiaoweigege/akamai2.0-sensor_data>
- <https://github.com/Edioff/akamai-analysis>
- <https://github.com/glizzykingdreko/akamai-v3-sensor-data-helper>
- <https://medium.com/@glizzykingdreko/akamai-v3-sensor-data-deep-dive-into-encryption-decryption-and-bypass-tools-da0adad2a784>
- <https://medium.com/@240942649/decoding-akamai-2-0-418e7c7fa0a0>
- <https://github.com/klenne/akamai-sensor-data-tools>
- <https://github.com/DalphanDev/akamai-sensor>
- <https://github.com/cirleamihai/akamai-1.7-cookie-generator>
- <https://github.com/i7solar/Akamai>
- <https://akamai-v3-tools.vercel.app/>
- <https://sensor-data-tools.netlify.app/>

### Mobile BMP (for reference, not our use case)
- <https://github.com/xvertile/akamai-bmp-generator>
- <https://github.com/reverse-god/akamai-sensordata>
- <https://github.com/dawud-outsystems/AkamaiBPMCordovaPlugin>

### Commercial / scraping APIs
- <https://github.com/Hyper-Solutions/hyper-sdk-py>
- <https://github.com/Hyper-Solutions/hyper-sdk-js>
- <https://hypersolutions.co>
- <https://docs.hypersolutions.co/akamai-web/getting-started>
- <https://scrapfly.io/bypass/akamai>
- <https://www.zenrows.com/blog/bypass-akamai>

### Topical writeups
- <https://substack.thewebscraping.club/p/bypassing-akamai-for-free> (Pierluigi Vinciguerra, 2025-03-23)
- <https://www.akamai.com/products/bot-manager>
- <https://github.com/topics/akamai-sensor-generator>

### Adjacent prior art
- <https://github.com/Kaliiiiiiiiii-Vinyzu/patchright>
- <https://pkg.go.dev/github.com/FRIS-Solutions-Vault/akamai-sdk-go>
