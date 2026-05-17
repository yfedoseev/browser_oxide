# PerimeterX / HUMAN — Deep Engine Analysis (browser_oxide)

**Created:** 2026-05-16 · **Baseline git HEAD:** `fd98bfa` · **Engine owner:** PerimeterX / HUMAN Security agent
**Contract:** follows `docs/research/engines/00_INVENTORY_AND_METHOD.md` template (§0–12) verbatim.
**Fact discipline:** *mechanism fact* = cited external source; *our-code fact* = `file:line` I read; *hypothesis* = explicitly labelled `[HYP]`. Negative results are first-class.

---

## 0. Executive summary & pass-guarantee thesis

**How PerimeterX/HUMAN decides bot vs human in 2026.** PX is a *server-scored, continuous-telemetry* engine. A first-party (or third-party) JS sensor (`init.js` / `client.perimeterx.net/<AppId>/main.min.js`) loads, fingerprints the JS/canvas/WebGL/audio surface, harvests pointer/behavioral telemetry into a `PXNNNN`-keyed payload, base64s it and POSTs to a collector (`/<AppId>/xhr/api/v1/collector/<AppId>` first-party, or `collector-<AppId>.perimeterx.net` / `prx.wayfair.com/px/xhr`). The server computes a risk score, returns pipe-delimited `bake|_px3|…|<jwt>` instructions, and the edge Enforcer gates content on the `_px3` cookie (AES-CBC-256 + HMAC, key = `tenant_secret + appId` via PBKDF2, **~60 s TTL**). If the cumulative score crosses the tenant block threshold the server returns a `force` instruction → the **Press-and-Hold** (PaH) HUMAN Challenge renders inside `<div id="px-captcha">`. PaH is *not* a button-hold puzzle: it is a pretext for a signed behavioral-telemetry payload (mouse trajectory, `event.isTrusted`, pointer pressure, WASM-derived `PX12590`/`PX12610`) that must independently verify server-side. Server-side TLS (JA3/JA4) + IP reputation + per-customer ML models cross-check every payload, so a single fingerprint mismatch — or a datacenter IP — can flip the verdict regardless of a perfect JS surface.

**The single highest-leverage gap blocking us.** There is **none that matters for the current 29-site corpus** — see the verdict below. Structurally, the highest-leverage gap *if a real PX site enters the corpus* is **G-PX1: zero PX sensor/solver code exists** (greenfield; `crates/stealth/src/` has `qrator.rs`/`ngenix.rs`/`aliyun.rs`/`cloudflare.rs`/`kasada.rs` but **no `perimeterx.rs`** — verified `ls`, 2026-05-16). The realistic posture is *emulate-below-threshold via the unified in-engine architecture* (run the vendor sensor in our V8 + Chrome-faithful surface, let it self-POST and bake its own `_px3`), exactly as DataDome i.js / Akamai sec-cpt are intended to self-solve — **not** a hand-written `_px3` generator.

**Honest verdict — which guarded sites we pass / block / FP today.**

| Site | 29-set? | Reality (verified) | Verdict |
|---|---|---|---|
| **wayfair** | **yes (the only PX site)** | **TRUE PASS.** `/tmp/audit_failing_sites/wayfair.json` (committed `3739da9` classifier, `Page::navigate(chrome_130_macos, 1)`, 2026-05-16): `verdict:"pass"`, `body_len:1265119` (1.27 MB real render), `final_url` unchanged, sets PX cookies `_pxvid`/`pxcts`/`__pxvid`/`_pxttld`/`_px2` (sensor ran and was *not* challenged). It was a **classifier false positive** under the *old* matcher; the typed classifier correctly calls it `pass`. | **PASS — no PX work needed** |
| zillow / trulia / bloomberg | no (prior-research targets, NOT in the 29-set) | Block with PaH on iOS per `docs/research_2026_05_14/05_PERIMETERX.md` §0 live probes (2026-05-14). Out of current corpus scope. | out-of-corpus |

**Bottom line (negative result, first-class):** *PerimeterX requires zero engineering for the current 29-site target set.* wayfair — the sole PX site — renders 1.27 MB of real content with PX cookies set and **no** challenge served; it was always a measurement artifact of the pre-`3739da9` body-substring classifier, never a real block. The greenfield solver design below (§9, §11) is documented for completeness/future corpus growth, **not** as an actionable gap for the 29-set.

---

## 1. Vendor surface & 2026 deployment

**Vendor.** PerimeterX, Inc. (founded 2014) → acquired by **HUMAN Security** (2022), consolidated into the HUMAN Defense Platform. DNS/SDK namespaces still carry `perimeterx.net`, `px-cloud.net`, `px-cdn.net`, `pxchk.net`, `px-client.net` for back-compat (mechanism fact: [The Lab #56](https://substack.thewebscraping.club/p/the-lab-56-bypassing-perimeterx-3), accessed 2026-05-16; [ZenRows 2026](https://www.zenrows.com/blog/perimeterx-bypass), accessed 2026-05-16).

**Product tiers (shared sensor + cookie state machine, different decision logic).**

| Product | Purpose | Relevance |
|---|---|---|
| **Bot Defender** | Block automation on web/mobile origin. PaH is its challenge. | The only tier relevant to scraping. |
| Account Defender | Reuse sensor to score login/signup/ATO. | Login flows only. |
| Code Defender | Client-side supply-chain detection. | Often co-deployed (wayfair). |
| HUMAN Challenge | The standalone PaH widget. | Invoked by Bot Defender on block. |
| Sightline | Marketplace cyberfraud. | N/A. |

**Script names / paths / versions in the wild.**
- Bootstrap (inline in HTML head): `window._pxAppId='PX<10char>'`, `window._pxHostUrl`, `window._pxJsClientSrc`, `window._pxFirstPartyEnabled`. The first-party path component is `appId.slice(2)` (e.g. `PXHYx10rg3` → `/HYx10rg3/xhr`).
- Sensor: first-party `/<8char>/init.js`, or third-party `//client.perimeterx.net/<AppId>/main.min.js` (~270 KB minified, ~9 000 lines deobfuscated; per-refresh function/variable renaming + VM control-flow obfuscation — mechanism fact: [Pr0t0ns/PerimeterX-Reverse](https://github.com/Pr0t0ns/PerimeterX-Reverse), accessed 2026-05-16; [glizzykingdreko WASM repo](https://github.com/glizzykingdreko/PerimeterX-Captcha-WASM), accessed 2026-05-16).
- Captcha: `/<8char>/captcha/captcha.js`, fallback `https://captcha.px-cloud.net/PX<AppId>/captcha.js`.
- Sensor version is carried as `tag=` in the collector query (e.g. `v8.9.6`, `v8.x` WASM-era since ~2024). The `PX` key namespace grows monotonically per version.

**Which target site uses which tier.**

| Site | AppId | Deployment | First-party mount | Sensor source |
|---|---|---|---|---|
| **wayfair** (29-set) | `PX3Vk96I6i` | Bot Defender + Code Defender, **Cloudflare edge in front**, hybrid | `https://prx.wayfair.com/px/xhr` | third-party `//client.perimeterx.net/PX3Vk96I6i/main.min.js` |
| zillow (NOT in 29-set) | `PXHYx10rg3` | first-party | `/HYx10rg3/xhr` | first-party `/HYx10rg3/init.js` |
| trulia (NOT in 29-set) | `PXYO6YjwLb` | first-party (Zillow Group tenant, byte-identical block template) | `/YO6YjwLb/xhr` | first-party |
| bloomberg (NOT in 29-set) | `PX8FCGYgk4` | first-party, embedded HUMAN Challenge | `/8FCGYgk4/xhr` | first-party |

Source for AppIds: `docs/research_2026_05_14/05_PERIMETERX.md` §0, live `curl` probes 2026-05-14. **Corpus note:** only **wayfair** is in the 29-site target set (master plan `docs/research_2026_05_16/00_MASTER_PLAN.md:111`; `01_SITE_SET_STATUS.md:79`). zillow/trulia/bloomberg are *prior-research* targets and out of current scope.

---

## 2. Detection pipeline — stage by stage

| Stage | Signal collected | How scored | Kill vs soft-score |
|---|---|---|---|
| **Edge / TLS** | JA3/JA4 ClientHello; for wayfair the Cloudflare worker also enforces. | Library TLS patterns "recognized instantly" (mechanism fact: [Scrapfly 2026](https://scrapfly.io/blog/posts/how-to-bypass-perimeterx-human-anti-scraping), accessed 2026-05-16). | Mismatch with UA-claimed browser → strong soft-score; gross mismatch can kill. |
| **IP reputation** | Source IP ASN class. | Residential/mobile = positive; **datacenter = significant negative** (mechanism fact: Scrapfly 2026). | Datacenter alone rarely a hard kill but caps achievable score; per-customer ML can hard-block. |
| **HTTP/2** | Header order, pseudo-header order, PRIORITY frames, Priority header. | Medium priority for PX (vs Critical for Akamai). | Soft-score; iOS Safari shape (no `sec-ch-ua`, no `sec-fetch-user`, RFC 7540 PRIORITY frames) must be coherent. |
| **Loader / bootstrap** | Presence + integrity of `window._px*` vars; sensor fetch success. | If sensor never POSTs (blocked third-party script, JS disabled) the `<noscript>` pixel fires → scored as bot. | Soft-score; missing sensor POST is a strong negative. |
| **Fingerprint JS (sensor)** | Canvas (emoji block U+1F600–1F64F), WebGL UNMASKED vendor/renderer + `WEBKIT_EXT_*` anisotropic, OfflineAudioContext+DynamicsCompressor hash, ~600-font oracle (`mmmmmmmmmmlli`), full navigator/screen/window surface, automation-marker scan, iframe-realm consistency, `event.isTrusted` audit. | Each mismatch contributes to a cumulative server-side risk score; UA↔surface coherence cross-checks (e.g. iOS UA must lack `chrome`/`userActivation`/`deviceMemory`/`connection`). | Cumulative soft-score; single hard tell (e.g. `navigator.webdriver===true`, forged event) can kill. |
| **Behavioral** | `PX58` event buffer ([x,y,t,type] ring), scroll/keydown counts, last-50 pointer tuples, visibility/focus transitions, hover dwell, pointer pressure/tilt, micro-tremor (3–8 Hz). | Bezier vs straight-line, acceleration entropy, hold duration, empty buffer = bot. **PX is the most aggressive engine on behavioral biometrics.** | Cumulative; an empty `PX58` is itself a tell. |
| **Server ML** | All of the above + per-customer model trained on the tenant's historical traffic. | "Custom ML models for each website" — a bypass that works on one PX site may fail on another (mechanism fact: Scrapfly 2026). | Final verdict; sets `_px3` score → Enforcer compares to `px_blocking_score` (default 100, 0–100 configurable — mechanism fact: [HUMAN Lambda config](https://docs.humansecurity.com/applications/lambda-configuration-options), accessed 2026-05-16). |

Key 2026 update: the per-customer ML layer is the dominant change vs the 2024 picture and is the same structural problem as Kasada's `b:1` server-ML tail (cross-engine parallel — see `docs/research/engines/kasada.md`).

---

## 3. Challenge / script anatomy (bytes & structure)

**Sensor lifecycle (clean GET /).** (mechanism fact: `docs/research_2026_05_14/05_PERIMETERX.md` §2.3 corroborated by [Pr0t0ns-Reverse](https://github.com/Pr0t0ns/PerimeterX-Reverse), [treywey px3](https://github.com/treywey/treyweys-antibot-thread/blob/master/px3), accessed 2026-05-16):

1. Origin GET, no PX cookies → Enforcer falls through → bootstrap injected.
2. Bootstrap sets `window._pxAppId/_pxHostUrl/_pxJsClientSrc/_pxFirstPartyEnabled`.
3. Sensor fetch (`init.js`/`main.min.js`): VM-obfuscated, per-refresh-renamed, base91-style decoder. License banner `// @license Copyright (C) 2012-2026 HUMAN Security, Inc`.
4. First collector POST — base64(JSON) array, type `PX182` (handshake), data keyed `PX58`(events) / `PX96`(URL) / `PX183`(ts ms) / `PX371`(has-navigator). Query carries `appId,tag,uuid,ft,pxhd,pc,rsc,seq,en,sid,vid,cts`.
5. Server response — pipe-delimited instruction array: `"do":["bake|_px3|<params>|<jwt>","bake|_pxhd|…|<jwt>","vid|<vid>","force|<challenge_token>"]`.
6. Continuous heartbeats every ~5–15 s and before nav/fetch/click; `seq`++ each time; payload grows to include `PX256`/`PX257`/`PX259` (dynamic-function challenge).
7. If score never drops → `force` → captcha page.

**Interstitial / PaH page (verbatim from prior Zillow probe).**
```html
<meta name="description" content="px-captcha">
<title>Access to this page has been denied</title>
<script>
  window._pxUuid='02e1cd63-…'; window._pxAppId='PXHYx10rg3';
  window._pxHostUrl='/HYx10rg3/xhr'; window._pxJsClientSrc='/HYx10rg3/init.js';
  var pxCaptchaSrc='/HYx10rg3/captcha/captcha.js?a=c&u=<uuid>&v=&m=0&b=<b64url>&h=<b64method>';
</script>
```
`a=c`(captcha) `u`(uuid) `v`(vid, empty first time) `m=0`(desktop)/`m=1`(mobile) `b`(b64 blocked URL) `h`(b64 method, `R0VU`=GET). Fallback static text: *"Before we continue… Press & Hold to confirm you are a human"*, *"Reference ID <uuid>"*.

**PaH widget DOM.** Mounts in `<div id="px-captcha">`. **Closed Shadow DOM** (`element.shadowRoot===null` from outside). Every inner-button event checked for `event.isTrusted===true` — synthetic `MouseEvent`/`el.click()`/CDP-dispatched events carry `isTrusted:false` and **instantly fail** (mechanism fact: [roundproxies 2026](https://roundproxies.com/blog/bypass-perimeterx/), accessed 2026-05-16; corroborated [Scrapfly bypass](https://scrapfly.io/bypass/perimeterx)).

**WASM blob (captcha.js v8.x+, since ~2024).** Encrypted WASM module, two exports invoked as `Ce.a()` → `PX12590` (hardware-rooted entropy) and `Ce.b(input)` → `PX12610`. WASM loaded via a global instance; `helpers.js` bridges JS↔WASM. Both must be evaluated under the memory layout the server expects (mechanism fact: [glizzykingdreko/PerimeterX-Captcha-WASM](https://github.com/glizzykingdreko/PerimeterX-Captcha-WASM), accessed 2026-05-16; analogous to Akamai BMP `bm-sz`). deno_core has V8 WASM so a faithfully-rendered captcha.js *should* execute it natively — `[HYP]` unverified, no PX site in corpus to test.

**Exact success signal.** A successful sensor sequence yields `Set-Cookie: _px3=<hmac>:<b64(salt:iters:aes_cbc)>` with an internal score below threshold (a `_px3` whose decrypted `s.a`/`s.b` are under `px_blocking_score`). A successful PaH solve yields `bake|_pxhd|…|<jwt>` (longer-lived "passed captcha" marker). There is **no postMessage / no separate `pxAuth` token** in the production web flow — the cookie *is* the signal (mechanism fact: prior research §3.5, [gbhackers](https://gbhackers.com/target-perimeterx-captcha/) accessed 2026-05-16).

---

## 4. Fingerprint / sensor payload — field by field

PX obfuscates field names into a `PXNNNN` namespace (~220 entries in Pr0t0ns `key_map.json`). Load-bearing fields and expected real-Chrome values:

| Field | Meaning | Expected (Chrome desktop) | How mismatch scores |
|---|---|---|---|
| `PX182`/`PX212`/`PX315` | payload type (handshake / data / mobile) | `PX182` then `PX212` | wrong sequence ⇒ malformed ⇒ block |
| `PX58` | event buffer `[x,y,t,type]` | ≥30–50 realistic tuples | empty buffer = bot tell |
| `PX96` / `PX183` / `PX371` | URL / local ms ts / has-navigator | real URL, monotonic ts, `true` | implausible ts or `false` ⇒ score |
| `PX256`/`PX257`/`PX259` | challenge config / response / 6 operator indices | computed from behavior hash | mismatch ⇒ block |
| `PX12590`/`PX12610` | WASM `a()`/`b(input)` outputs | deterministic under expected memory | wrong ⇒ block |
| navigator.webdriver | — | `false` (own, non-deletable) | `true`/`undefined`/deletable ⇒ kill/score |
| navigator.vendor | — | `"Google Inc."` (Chrome) / `""` (Safari) | mismatch w/ UA ⇒ score |
| navigator.plugins.length / mimeTypes.length | — | Chrome ≈5 / Safari 0 | UA↔value incoherence ⇒ score |
| navigator.languages | — | matches `Accept-Language` | mismatch ⇒ score |
| navigator.hardwareConcurrency / deviceMemory | — | plausible; **iOS Safari has NO deviceMemory** | exposing deviceMemory on iOS UA ⇒ kill |
| navigator.maxTouchPoints | — | desktop Chrome 0/1; iOS 5 | iOS UA w/ 0 ⇒ kill |
| navigator.platform | — | `MacIntel`/`Win32`; iOS `iPhone`/`iPad` | mismatch ⇒ kill |
| navigator.userActivation | — | present (Chromium); **absent iOS Safari** | defining on iOS UA ⇒ kill |
| navigator.connection (NetInfo) | — | present Chrome; **absent Safari** | exposing on Safari UA ⇒ kill |
| window.chrome (+ runtime/app/csi/loadTimes) | — | present Chrome (`runtime` absent off-extension); **absent Safari** | wrong presence ⇒ kill |
| Canvas hash | emoji block render | stable per GPU profile | inconsistent w/ WebGL ⇒ score |
| WebGL UNMASKED + `WEBKIT_EXT_texture_filter_anisotropic` | — | Chrome unprefixed; Safari `WEBKIT_EXT_*` | wrong prefix vs UA ⇒ kill |
| OfflineAudioContext+DynamicsCompressor hash | — | stable per profile | mismatch ⇒ score |
| Fonts (`mmmmmmmmmmlli` oracle, ~600 fonts) | — | non-empty plausible set | empty = "no fonts" = bot |
| iframe-realm Navigator ctor | — | matches parent | mismatch ⇒ kill |
| event.isTrusted (on PaH) | — | `true` | one `false` ⇒ instant challenge fail |
| PointerEvent.pressure/tilt/width/height | — | varies (touch) / 0.5 const (mouse) | constant on "touch" UA ⇒ score |

Our `crates/browser/tests/perimeterx_surface_parity.rs` already asserts most of the *passive surface* invariants (webdriver=false, vendor=Google, plugins≥5, native `Function.toString`, iframe-realm ctor match, BatteryManager/visualViewport/userAgentData GREASE, RTC mDNS `.local`). It does **not** exercise the `PXNNNN` sensor payload, `PX58`, the WASM, or PaH (the file header explicitly scopes the encrypted payload out — `perimeterx_surface_parity.rs:15-18`).

---

## 5. Crypto / encoding

- **`_px3` token.** `<hmac-32hex>:<base64(salt:iterations:aes_cbc_data)>`. AES-CBC-256, key derived from `tenant_secret + appId` via PBKDF2-HMAC-SHA256 (1..10000 iters). Decrypts to `{t (unix ts), s:{a:action-score,b:bot-score 0-100}, u (session uuid = _pxuuid), v (visitor id = _pxvid), h (optional headers hash)}`. Score is computed **server-side**; the cookie only transports it back so the edge can short-circuit. TTL **~60 s** (mechanism fact: prior research §2.2 corroborated by [perimeterx-python-wsgi px_cookie.py](https://github.com/PerimeterX/perimeterx-python-wsgi/blob/master/perimeterx/px_cookie.py), accessed 2026-05-16).
- **`_px2`** legacy v2: `2:<base64-json>` (mobile-ish path). Observed set on **wayfair** in our audit (`/tmp/audit_failing_sites/wayfair.json` `cookie_names`).
- **`X-PX-Authorization`** header tokens: `1:<token>:<expiry>:<sig>` (v1) / `3:<enc>:<sig1>:<expiry>:<sig2>` (v3) — mobile/API path (mechanism fact: TakionAPI px-mobile docs, accessed 2026-05-16; lower confidence — secondary source).
- **Sensor payload encoding.** base64(JSON array). The encryption/encoding leg is the part public RE projects mark "COMPLETED REVERSE" for v8.9.6; **the `pc` (proof-of-work-ish PC value), `sid`, `vid`, `cts` fields remain "to be reversed"** (mechanism fact: [Pr0t0ns/PerimeterX-Reverse](https://github.com/Pr0t0ns/PerimeterX-Reverse), last release 2024-04-27, accessed 2026-05-16).
- **Daily/seed rotation & anti-replay.** Per-page-load function/variable renaming + VM control-flow obfuscation rotate the *script*, not a daily key. Anti-replay = the ~60 s `_px3` TTL + server cross-check of token IP/TLS hash: a `_px3` replayed from a different IP or TLS JA3 triggers re-challenge (mechanism fact: prior research §3.1; consistent with [Scrapfly 2026](https://scrapfly.io/blog/posts/how-to-bypass-perimeterx-human-anti-scraping)).
- **Server-side TLS cross-check.** JA3/JA4 of the connection is bound into the risk model and compared to UA; this is why a perfect JS surface over a mismatched TLS stack still scores (mechanism fact: Scrapfly 2026).

---

## 6. Cookie & header lifecycle / state machine

| Cookie | Role | Lifetime | When set | Carried to |
|---|---|---|---|---|
| `_pxvid` | Visitor ID (UUIDv4), cross-session | ~1 yr (some 3 yr) | first successful sensor POST | every PX request |
| `_pxhd` | Session/"loaded sensor" token; after PaH = "passed captcha" | ~1 yr (`Max-Age=31536000`); validity often 24 h | even on block pages (bloomberg) | every request incl. challenge submit |
| `_px3` | Risk-scored access token (gates content) | **~60 s** | after sensor POST when score below block | every request in active session; refreshed by heartbeat |
| `_px2` | Legacy v2 token | ~60 s | mobile / some flows (observed on wayfair) | mobile/API origins |
| `_pxff_*` | Forensic feature flags (server-driven probe-family toggles: `_pxff_fp/rf/tm/cc/idp_c`) | match `_px3` | when sensor receives a config requiring forensics | next sensor POST (acknowledge flags) |
| `_pxde` | Data-enrichment (server→client metadata, analytics) | — | by Enforcer | sensor |
| `pxcts`,`_pxttld`,`__pxvid` | client-timestamp / TLD / vid mirror (observed on wayfair) | varies | sensor bootstrap | PX requests |

**"Solved" on the wire** = a request that carries a `_px3` whose decrypted `s.a/s.b` are below the tenant `px_blocking_score`, OR (post-PaH) a `_pxhd` with a valid JWT signature; the Enforcer then serves origin content (HTTP 200, full body) instead of the 403/405 + `px-captcha` interstitial. Valid→invalid transitions: `_px3` expiring (~60 s) without heartbeat refresh; `_px3` replayed from a different IP/TLS hash; score escalation from a `force` instruction.

**wayfair observed state (our audit, 2026-05-16):** sets `_pxvid`, `pxcts`, `__pxvid`, `_pxttld`, `_px2` — the sensor *ran and was accepted* (no `force`, no `px-captcha` interstitial, 1.27 MB origin body). This is the wire signature of a **pass**.

---

## 7. How OSS / commercial tools defeat it

| Tool / repo | Technique | 2026 reproducibility | Patched/dead | Citation (accessed 2026-05-16) |
|---|---|---|---|---|
| Pr0t0ns/PerimeterX-Reverse | v8.9.6 payload notes; encoding "COMPLETED", `pc`/`sid`/`vid`/`cts` unsolved | Partial; payload schema only, not a working solver | Stale (last release 2024-04-27); v8.9.6 likely server-evolved | https://github.com/Pr0t0ns/PerimeterX-Reverse |
| Pr0t0ns/PerimeterX-Solver (v6.7.9) / perimeterx-solution (v8.9.6) | Python `_px3` solver + fingerprint + `pc_functions` + `key_map.json` | Low — targets old sensor versions; per-customer ML defeats static solvers | Effectively dead vs 2026 sites | https://github.com/Pr0t0ns/PerimeterX-Solver |
| glizzykingdreko/PerimeterX-Captcha-WASM | Node emulation of WASM `a()`/`b()` → `PX12590`/`PX12610` | Demonstrates WASM is *emulatable*; not a full bypass | Version-specific (v8.7.x era) | https://github.com/glizzykingdreko/PerimeterX-Captcha-WASM |
| glizzykingdreko/PerimeterX-Deobfuscator | Babel AST deobfuscator | Useful for analysis, not bypass | Maintained-ish 2024 | (prior research §8.1) |
| Camoufox | Firefox stealth, binary-patched fingerprint; explicitly tests wayfair/zillow/trulia | "High" per multiple guides; behavioral still a gap | Active 2026 | [roundproxies 2026](https://roundproxies.com/blog/bypass-perimeterx/) |
| Patchright / Nodriver / SeleniumBase UC | CDP-only stealth, >100 leak patches | Medium; passes low-risk PX, fails high-risk + behavioral | Active | Scrapfly/ZenRows 2026 |
| Scrapfly / ZenRows / Bright Data | Behavior emulation + **residential proxies** + TLS impersonation; "stay below challenge threshold" | Claimed ~95% (Scrapfly); the dominant *commercial* model | Active commercial | https://scrapfly.io/bypass/perimeterx ; https://www.zenrows.com/blog/perimeterx-bypass |
| 2Captcha / CapSolver / NoCaptcha.io / SilentSolve | Human or AI PaH solvers ($2–200/1000) | Reproducible but paid + slow; CapSolver PaH "coming soon" | Service-dependent | [gbhackers](https://gbhackers.com/target-perimeterx-captcha/) ; [shrotam.com](https://shrotam.com/en/perimeterx) |
| treywey/px3 ; voidstar0 gist ; incizzle/perimeterx-tools | Annotated `_px3`/captcha analysis, payload encoder/decoder | Reference-grade, not a bypass | 2023–2024, historical | (prior research §8.1) |
| **The user-named "PauloHenrique-Maluf/perimeterx-bypass" & "h0nde/px-solver"** | — | **NOT publicly indexed in 2026-05** — renamed/private/deleted; treat as unavailable | Dead/unverifiable | (prior research §8.1, re-confirmed via 2026-05-16 search: no current index) |

**State of the art (2026), skeptically.** Two camps: (1) *actually solve PaH* — rare, expensive, needs live WASM + residential IP + full heartbeat + behavioral biometrics (SilentSolve/CapSolver closest, $50–200/1000); (2) *emulate below the challenge threshold so PaH never fires* — the dominant reproducible approach (Scrapfly/ZenRows/Bright Data). Every public static solver targets *one* sensor version and is defeated by per-customer ML + script rotation. **No free OSS project reproducibly solves a 2026 production PX site end-to-end.** This matches our cross-engine finding (Kasada `b:1`, DataDome daily-key): static client-side solvers do not survive server-ML rotation.

---

## 8. What browser_oxide does today (file:line evidence)

**Exact path from `navigate()` to any PX signal:** *there is none beyond detection.*

| Component | Status | Evidence |
|---|---|---|
| PX detection (body substring) | **WIRED & exercised** | `crates/browser/src/page.rs:181` `body.contains("px-captcha")` inside `body_has_challenge_marker()` (`page.rs:167`), reached via `is_anti_bot_challenge()` (`page.rs:263`) and `challenge_verdict()` (`page.rs:274`). Called at `page.rs:1639,1730,1836,1860,2004,2193` in the navigate loop (on a CHL marker the loop *iterates/reloads* — no solve). |
| `_pxhd` substring (stub-gated) | WIRED & exercised | `page.rs:192` `stub_sized && body.contains("human security")` — note: `_pxhd`/`_px3` are NOT in `page.rs`'s matcher; they appear only in the *test* classifiers (holistic_sweep/audit). |
| PX surface parity test | wired-but-test-only | `crates/browser/tests/perimeterx_surface_parity.rs` — asserts Chrome JS-surface invariants; **does not** build/POST a sensor, no `_px3`, no PaH (header `:15-18` scopes payload out). |
| CSP allowlist | WIRED (incomplete) | `crates/net/src/csp.rs:686` `connect-src 'self' *.akamaihd.net *.perimeterx.net` — allows `*.perimeterx.net` but **NOT** `px-cloud.net` / `px-cdn.net` / `prx.wayfair.com` (the first-party collector). Not load-bearing today (no solver) but a latent gap for any future PX solver. |
| Behavioral noise comments | inert | `crates/stealth/src/behavior.rs:4`, `crates/js_runtime/src/extensions/input_ext.rs:3`, `stealth_ext.rs:161` mention PX in doc-comments only — no PX-specific logic. |
| PX sensor builder / `_px3` generator / PaH solver | **MISSING (greenfield)** | No `crates/*/src/*perimeter*` or `*px*` file (verified `ls`, 2026-05-16). `crates/stealth/src/` has qrator/ngenix/aliyun/cloudflare/kasada — **no `perimeterx.rs`**. |

**Dead-code test result:** N/A — there is no PX solver code to be dead; the gap is *absence*, not dead code. (Contrast Akamai `sec_cpt::solve_crypto` / DataDome `DdEncryptor` which exist-but-unwired.)

---

## 9. GAP ANALYSIS — what we are missing (ranked, concrete)

> **Framing (critical):** for the **current 29-site corpus the actionable gap count is ZERO** — wayfair is a true pass (§0, §10). The gaps below are *contingent* — they only become actionable if a genuinely-blocking PX site enters the corpus. Ranked by blast radius × tractability.

| # | Gap | Evidence | Blast radius | Difficulty | Risk | Concrete fix |
|---|---|---|---|---|---|---|
| **G-PX0** | **No evidence any PX site needs work.** The only operational gap is *the absence of a decisive blocking-capture*: we cannot prove a PX site is blocked without a hard-403/405 + `px-captcha` body from a non-CDP real-browser reference (the `nocdp.sh` discipline from MEMORY). | wayfair audit = pass; zillow/trulia/bloomberg out-of-corpus | 0 sites today | — | — | Before any PX engineering: capture a clean nocdp hard-block of a *corpus* PX site. None exists ⇒ do nothing. |
| **G-PX1** | No PX sensor/solver at all (greenfield). | §8; no `perimeterx.rs` | 0 today; 1 (wayfair) *if* it ever flips; N future | **L** | High | Unified in-engine architecture (see §11) — run vendor `main.min.js` in our V8 + Chrome-faithful surface, let it self-POST to `prx.wayfair.com/px/xhr`, consume `Set-Cookie: _px3/_pxhd`, re-issue. Same shape as the intended DataDome i.js / Akamai sec-cpt self-solve. |
| **G-PX2** | CSP `connect-src` omits `px-cloud.net`/`px-cdn.net`/`prx.<site>` (first-party collector). | `csp.rs:686` | blocks a future solver's collector POST | S | Low | Add `*.px-cloud.net *.px-cdn.net` (and per-site first-party collector hosts) to the PX allowlist when a solver lands. |
| **G-PX3** | Behavioral `PX58` would be empty — even a perfect surface scores as bot on PX (most behavior-aggressive engine). | prior research §4.3; `behavior.rs` has no PX path | future PX sites | M | Med | Feed `stealth::behavior` (sigma-lognormal/Plamondon, already used for Akamai `__akamai_events`) into a PX sensor's pointer buffer; reuse, do not rebuild. |
| **G-PX4** | iOS-profile surface coherence unverified for PX's iOS consistency check (`chrome`/`userActivation`/`deviceMemory`/`connection` must be *absent*; `maxTouchPoints===5`). | prior research §6.3; `window_bootstrap.js` gating not re-audited this pass | only if an iOS-targeted PX site enters corpus | M | Med | Audit iOS-profile gating in `crates/js_runtime/src/js/window_bootstrap.js`; verify the surface-parity test covers iOS profile (currently only `chrome_130_macos`). |
| **G-PX5** | WASM `PX12590/PX12610` correctness under deno_core V8 memory model unverified `[HYP]`. | §3; no PX site to test | future v8.x+ PX | M | Med | If a solver lands, run captcha.js WASM in deno_core, diff outputs vs glizzykingdreko reference. |

---

## 10. FALSE-POSITIVE ANALYSIS of our code

### 10a. Detection FPs

| # | file:line | False claim it can produce | Why false | Catching test |
|---|---|---|---|---|
| **FP-PX-1 (RESOLVED, residual narrow)** | `crates/browser/src/page.rs:181` `body.contains("px-captcha")` | "wayfair is a PerimeterX challenge / blocked" | wayfair renders **1.27 MB** of real content (`/tmp/audit_failing_sites/wayfair.json`, 2026-05-16) and **does not** contain the literal `px-captcha`. It *does* contain `_pxCaptcha` inside a cookie-consent JSON manifest (`{"_px3":"NECESSARY","_pxCaptcha":"NECESSARY","_pxvid":"NECESSARY"…}`). `body.contains("px-captcha")` is a **substring** check: the `-` vs `_` means `px-captcha`≠`_pxCaptcha` and the literal `px-captcha` does **not** match here, so the *current* `page.rs` matcher does **not** FP on wayfair. The historical FP came from the **older** matcher/divergent classifiers (below), now corrected by the typed `ChallengeVerdict` (`page.rs:120-293`, commit `3739da9`). **Residual risk:** a benign page that legitimately contains the literal substring `px-captcha` (e.g. a CSS class, a doc page about PerimeterX, an analytics key) at *any* body size would still be classified `EdgeBlock`/`SensorFail` because `px-captcha` is in the **unconditional strong-marker** set, NOT gated by `stub_sized` (`page.rs:171-181`). Low probability but not zero. | Add a unit test: a 1 MB rendered page containing the literal `"px-captcha"` only inside an inline JSON/CSS string must classify `Pass`, not `EdgeBlock`. Currently no such test exists. |
| **FP-PX-2** | `crates/browser/tests/holistic_sweep.rs:875` `("px-captcha","PerimeterX-CHL")` is **unambiguous** (fires at *any* body size) AND `:907` `("_pxhd","PerimeterX-CHL")` is small-body-gated | "wayfair / a PX-using site is PerimeterX-CHL" | This classifier is **divergent** from `page.rs` (master plan §reconciliation). `px-captcha` at any size will mis-flag a large rendered page that has the literal substring. The `medium_body_with_pxhd_substring_is_not_chl` test (`:1039`) only guards `_pxhd`, NOT `px-captcha`. wayfair's body has `_pxCaptcha` (no hyphen) so it escapes here too — but a site embedding the literal `px-captcha` in content would be a false CHL. | Extend `holistic_sweep.rs`'s test to assert a >1 MB body with literal `px-captcha` in an inline string is NOT `PerimeterX-CHL`; gate `px-captcha` on body size like the phrase markers. |
| **FP-PX-3** | `audit_failing_sites.rs:192,289` collect `px-captcha`/`_px3`/`_pxhd` as `html_markers` | "wayfair has PX challenge markers" (misread of the audit JSON) | The audit **records markers as context, not verdict** — `verdict` comes from `page.challenge_verdict()` (`:146,261`), which correctly returned `pass` for wayfair. But a human reading `wayfair.json`'s `"html_markers":["_pxhd","_px3","datadome","captcha"]` could wrongly conclude "blocked". The `marker_contexts` field disambiguates (all from a consent JSON) but is easy to skip. This is a **reporting/interpretation FP**, not a code FP. | None needed (verdict logic is correct); document that `html_markers` ≠ verdict. The `datadome`/`captcha` markers on wayfair are *also* benign (a `<link preload href=js.datadome.co/tags.js>` + the substring `_pxCaptcha`). |
| **FP-PX-4 (thin-shell, other direction)** | `challenge_verdict()` `page.rs:286` `len<5*1024 ⇒ RenderIncomplete else Pass` | "a 5–50 KB PX page with no marker is a Pass" | If PX ever served a *markerless* deny (or a thin SPA shell that the sensor would have populated) between 5 KB and 50 KB, it's labelled `Pass`. Not observed for wayfair (1.27 MB, unambiguous real render) but a structural under-match for any future PX site that blocks without the `px-captcha` literal. | A test: a known PX `force`/deny body that lacks `px-captcha` must not classify `Pass`. Requires a real capture (none in corpus). |

### 10b. Solver / logic FPs

| # | file:line | False claim | Why false | Catching test |
|---|---|---|---|---|
| **FP-PX-5 (structural)** | entire engine — no PX solver exists | "Detecting PX = handling PX" / any pipeline that branches on `is_anti_bot_challenge()` for a PX page implies remediation | We have **only a detector**. On a real PX block the navigate loop *re-iterates/reloads* (`page.rs:1639,1730,2193`) with no solve path — a reload cannot earn `_px3`. So **any** "PX-blocked" verdict is *structurally unactionable*: detection without a solver is an FP of the *implied capability*, exactly the master-plan "exists ≠ exercised" class, in its strongest form (does-not-exist). | A regression assertion that fails loudly if a corpus site is verdict=`EdgeBlock`+vendor=PerimeterX (forces an explicit "no solver" acknowledgement rather than a silent reload loop). |
| **FP-PX-6** | `crates/browser/tests/perimeterx_surface_parity.rs` (whole file) | "PerimeterX surface parity passes ⇒ we pass PerimeterX" | The test asserts a Chrome JS surface (webdriver/vendor/iframe-realm/Battery/RTC). PX's *actual* gate is the **encrypted sensor payload + behavioral telemetry + server ML + TLS/IP** — none exercised here (header `:15-18` admits this). A green parity test says nothing about passing a real PX site. It also only runs the **`chrome_130_macos`** profile, so the iOS-specific PX consistency check (§4, G-PX4) is entirely unverified. | None can fully catch this offline (the live path differs by design — note per §4 inventory rule, the network-free gate *structurally cannot* verify a PX pass). Document the limit explicitly (done here). |
| **FP-PX-7** | `csp.rs:686` allowlists `*.perimeterx.net` | "our CSP is PX-ready" | The collector for first-party/hybrid PX is `px-cloud.net`/`px-cdn.net`/`prx.<site>`, NOT `*.perimeterx.net`. A future solver's collector POST would be CSP-blocked. Latent (no solver today) but the allowlist *implies* readiness it doesn't have. | When a solver lands: a test that the PX collector host is in the effective `connect-src`. |

---

## 11. The concrete pass-guarantee plan

### 11a. wayfair (the only 29-set PX site) — **already passing; plan = protect the pass**

1. **No engineering required.** wayfair renders 1.27 MB with PX cookies set and no challenge (§0, §6, `/tmp/audit_failing_sites/wayfair.json`). The "block" was a pre-`3739da9` classifier artifact.
2. **Regression guard (S, do this):** add a unit test that a 1 MB+ body containing the literal substring `px-captcha` *only inside an inline JSON/CSS string* classifies `Pass` (closes FP-PX-1 residual + FP-PX-2). This is the single cheap, decisive action for the corpus.
3. **Content-depth sanity (S):** wayfair is a **big-body pass** (1.27 MB) per master-plan's own "big-body passes are real renders" criterion — no thin-shell concern. Spot-check the body has product/nav DOM (not a 1 MB decoy) on the next audit run; not expected to fail.
4. **Do NOT build a PX solver for the corpus.** It would be dead code (FP-PX-5 class) with zero site flips — explicitly out of scope per "negative results are first-class".

### 11b. Greenfield PX solver (only if a blocking PX site enters the corpus) — design, not a directive

Ordered, with the verification regime:

1. **Decisive capture first (G-PX0).** Reproduce a hard 403/405 + `px-captcha` body for a *corpus* PX site from a non-CDP real-browser reference (`nocdp.sh`). If it passes from a clean IP, the engine gap is moot (IP/operational). *Gate everything on this.*
2. **PX detection module** — promote the detector to a real `crates/stealth/src/perimeterx.rs`: regex `window\._pxAppId\s*=\s*['"]PX([A-Za-z0-9]{8,12})['"]`, extract AppId, derive first-party path `appId[2..]`, expose `is_perimeterx() -> Option<AppId>`.
3. **Unified in-engine self-solve** (the only viable model — matches DataDome/Akamai): load the vendor `init.js`/`main.min.js` in our V8 on the Chrome-faithful surface; let it build + POST its own sensor to the (first-party) collector; consume `Set-Cookie: _px3`/`_pxhd`; re-issue the navigation. **Do not hand-write `_px3`** (static solvers are dead vs per-customer ML — §7).
4. **CSP + cookie plumbing** — add `*.px-cloud.net *.px-cdn.net` + per-site first-party collector to `csp.rs:686`; ensure `_pxvid/_pxhd/_px3/_px2/_pxff_*` round-trip and survive a 403 (bloomberg-style state-carrying `_pxhd`).
5. **Behavioral feed** — wire `stealth::behavior` (existing sigma-lognormal) into the sensor's pointer buffer so `PX58` is non-empty and human-shaped (reuse, don't rebuild — G-PX3).
6. **WASM verify** — run captcha.js WASM in deno_core; diff `PX12590/PX12610` vs glizzykingdreko reference (G-PX5).
7. **PaH** — out of scope for an engine (needs human-like hold or paid solver); the realistic target is *stay below the challenge threshold so `force` never fires* (industry consensus, §7).

**Verification regime / structural limit (per §4 inventory rule):** the network-free §4 regression gate **cannot** verify a PX pass — PX's verdict is server-side ML over an encrypted payload + live TLS/IP, none of which the offline gate exercises (FP-PX-6). Verification *requires* a live, non-CDP navigation that returns origin content (HTTP 200, multi-MB body, no `px-captcha`, `_px3` baked) — exactly the wayfair audit signature we already have for the only corpus site. The offline gate's role is limited to the classifier-FP regression tests in 11a.2.

---

## 12. Sources & experiments

### External sources (URL + claim + accessed date — all 2026-05-16 unless noted)

**Primary / RE:**
- [Pr0t0ns/PerimeterX-Reverse](https://github.com/Pr0t0ns/PerimeterX-Reverse) — v8.9.6 payload schema; encoding "COMPLETED", `pc/sid/vid/cts` unsolved; last release 2024-04-27 (stale). 
- [Pr0t0ns/PerimeterX-Solver](https://github.com/Pr0t0ns/PerimeterX-Solver) / perimeterx-solution — Python solvers, old sensor versions.
- [glizzykingdreko/PerimeterX-Captcha-WASM](https://github.com/glizzykingdreko/PerimeterX-Captcha-WASM) — Node emulation of WASM `Ce.a()→PX12590`, `Ce.b()→PX12610`; ~9000-line VM-obfuscated source, per-refresh renaming.
- [perimeterx-python-wsgi px_cookie.py](https://github.com/PerimeterX/perimeterx-python-wsgi/blob/master/perimeterx/px_cookie.py) — official enforcer cookie validator (`_px3` AES/HMAC structure).
- [treywey/treyweys-antibot-thread/px3](https://github.com/treywey/treyweys-antibot-thread/blob/master/px3) — annotated PX3 cookie/payload.

**Bypass guides / mechanism (2026):**
- [Scrapfly: How to Bypass PerimeterX (2026)](https://scrapfly.io/blog/posts/how-to-bypass-perimeterx-human-anti-scraping) — TLS JA3, datacenter-IP penalty, **per-customer ML models** (2026 update), emulate-below-threshold.
- [Scrapfly bypass/perimeterx](https://scrapfly.io/bypass/perimeterx) — ~95% claim; behavior + residential + TLS.
- [ZenRows: Bypass PerimeterX 2026](https://www.zenrows.com/blog/perimeterx-bypass) — cookie triple, ML adapts, updated Jan 2026.
- [roundproxies: Press & Hold 2026](https://roundproxies.com/blog/bypass-perimeterx/) — closed Shadow DOM, `isTrusted` instant-fail; orig May 2025, updated Mar 2026.
- [The Lab #56: Bypassing PerimeterX 3](https://substack.thewebscraping.club/p/the-lab-56-bypassing-perimeterx-3) — `_px3` marker, PX domains, plain Playwright fails (PaH).
- [HUMAN Lambda config](https://docs.humansecurity.com/applications/lambda-configuration-options) — `px_blocking_score` default 100, 0–100 configurable.
- [gbhackers: Threat Actors Target PerimeterX CAPTCHA](https://gbhackers.com/target-perimeterx-captcha/) — PaH solver economics ($50–200/1000), `_pxhd` = passed-captcha proof.
- [shrotam.com / NoCaptcha.io PerimeterX](https://shrotam.com/en/perimeterx) — commercial PaH solver.

**Prior internal research (inputs, not gospel — verified against code):**
- `docs/research_2026_05_14/05_PERIMETERX.md` (67 KB) — deep mechanism (read in full); its iOS/zillow/trulia/bloomberg per-site work is **out-of-corpus** (not in the 29-set); its §0 AppIds/cookie state machine remain accurate mechanism facts.
- `docs/research_2026_05_16/00_MASTER_PLAN.md:111,135,382-409` — wayfair = sole PX site, Phase 0.2 typed re-baseline = `pass` (18/29 were classifier FPs).
- `docs/research_2026_05_16/01_SITE_SET_STATUS.md:79`, `05_NON_KASADA_VENDORS_STATUS_AND_FIX.md:269-356` — "no solver at all", greenfield, L.

### Local experiments / commands run (auditable)

- `ls crates/stealth/src/ ; ls crates/*/src/*perimeter* *px*` → **no PX solver source file** (only qrator/ngenix/aliyun/cloudflare/kasada). 2026-05-16.
- `grep -rni 'perimeterx|_px3|_pxhd|px-captcha' --include=*.rs` (non-test) → only `page.rs:181/192`, `csp.rs:686` (`*.perimeterx.net`), doc-comment mentions in `behavior.rs`/`input_ext.rs`/`stealth_ext.rs`. No solver. 2026-05-16.
- Read `crates/browser/src/page.rs:115-293` (typed `ChallengeVerdict`, `body_has_challenge_marker`, classifier) and navigate-loop call sites `:1639,1730,1836,1860,2004,2193` (reload-only, no PX solve). 2026-05-16.
- Read `crates/browser/tests/perimeterx_surface_parity.rs` (full) — surface-parity only, `chrome_130_macos` profile only, payload explicitly out of scope (`:15-18`). 2026-05-16.
- Read `crates/browser/tests/holistic_sweep.rs:850-930` + `audit_failing_sites.rs:160-400` — divergent classifiers; `px-captcha` unambiguous (any size) in holistic, marker-collection only in audit. 2026-05-16.
- **Decisive:** read `/tmp/audit_failing_sites/wayfair.json` (artifact of the committed `3739da9` typed re-baseline, `Page::navigate(chrome_130_macos,1)`, 2026-05-16 13:23): `verdict:"pass"`, `body_len:1265119`, `final_url` unchanged, PX cookies (`_pxvid/pxcts/__pxvid/_pxttld/_px2`) set, `html_markers` (`_pxhd/_px3/datadome/captcha`) all from a cookie-consent JSON `{"_px3":"NECESSARY","_pxCaptcha":"NECESSARY"…}` per `marker_contexts` — **a true render, not a challenge**. No heavy `cargo` run was needed (the decisive artifact already existed); no network test executed (per constraints).

---

*End of PerimeterX engine analysis. Single deliverable; no other files modified.*
