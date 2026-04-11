# Antibot Landscape — Research Archive (2026-04-10)

This is a **verbatim archive** of external research synthesized on
2026-04-10 about the state of the major commercial and regional
antibot systems. It is meant as a reference, not a plan — for the
actual roadmap see `docs/NEXT_STEPS.md`, and for per-engine detection
signals used by our test harness see the engine detection table in
`NEXT_STEPS.md §4.3`.

All URLs and claims are attributed. Where the source could not be
confirmed, that is called out in §5 Caveats.

---

## 1. Engine-by-engine breakdown

### Kasada (KPSDK v3 / KpsAPI)

Owner: **Human Security** acquired Kasada in Jan 2025, so it is now a
HUMAN product line alongside PerimeterX. Still marketed as "Kasada"
and the SDK namespace is still KPSDK.

**Top signals (2026):**

1. **Proof-of-work on `/ips.js`** — browser posts to a
   `/149e9513-…/fp` endpoint returning `x-kpsdk-ct` (token, reusable)
   and `x-kpsdk-cd` (single-use per request). Missing/incorrect
   tokens = hard block.
   ([ZenRows Kasada bypass 2026](https://www.zenrows.com/blog/kasada-bypass),
   [lktop/kpsdk GitHub](https://github.com/lktop/kpsdk))
2. **VM-in-VM obfuscated JS** (`p.js`) — randomized opcodes per
   deploy, interpreter-evaluated; detects
   `Function.prototype.toString` tampering, `navigator.webdriver`,
   missing `chrome.*` objects, timing of `performance.now()`.
   ([Humphryyy/Kasada-Deobfuscated](https://github.com/Humphryyy/Kasada-Deobfuscated))
3. **TLS JA3/JA4** matched against claimed UA.
4. **IP reputation** — datacenter IPs hard-fail on first request
   before `ips.js` even runs.

**Response patterns:** HTTP **429** with empty or minimal body
containing `<title>` referencing retry, and `x-kpsdk-*` headers on
success path. Original block often returns 429 plus a Set-Cookie
with no value worth parsing.

**Cookies:** Kasada is header-driven, not cookie-driven. Main
identifiers are headers `x-kpsdk-ct`, `x-kpsdk-cd`, `x-kpsdk-v`,
`x-kpsdk-r`, `x-kpsdk-h`.

**Bypasses that still work (late 2025):**

- **kpsdk-solver** — Playwright-based token miner, works against
  Twitch/Kick/Nike checkout
  ([0x6a69616e/kpsdk-solver](https://github.com/0x6a69616e/kpsdk-solver))
- **Open-source POW + VM emulation**, writeup Aug 2025
  ([The Web Scraping Club #76](https://substack.thewebscraping.club/p/bypassing-kasada-2025-open-source))
- **Patchright** to bypass CDP leaks
  ([Scrapfly guide 2026](https://scrapfly.io/blog/posts/how-to-bypass-kasada-anti-scraping-waf))

**What's harder for a from-scratch V8 browser:** Kasada's `p.js`
probes dozens of **exact Chrome quirks** (`window.chrome.loadTimes`,
`chrome.csi()`, `chrome.runtime.connect` existence shape,
`navigator.userAgentData.getHighEntropyValues` return shape,
`Notification.permission` transition states). Every one of these
you have to implement by hand. This is MORE work than Camoufox
because Firefox patches start with a real Gecko engine and only
need to *lie about* Chrome-specific features in UA mode — you have
to *build the truth*.

**Public test URL:** `https://www.canadagoose.com/`,
`https://www.kick.com/`, `https://www.hyatt.com/` all run Kasada
and can be probed unauthenticated.

**Confirmed 2026 users:** Kick, Twitch (auth path), Canada Goose,
Hyatt, Ticketmaster (partial — see §2), Nike SNKRS draws (Kasada on
the draw endpoint, Akamai on the storefront).

---

### DataDome (hybrid JS + AI scoring + Device Check v2)

Owner: DataDome SAS (France), still independent.

**Top signals (2026):**

1. **Server-side ML trust score <2ms** — combines JA3/JA4, HTTP/2
   frame order, Client Hints consistency, IP ASN reputation.
   ([DataDome 2025 Global Bot Security Report](https://datadome.co/resources/bot-security-report/))
2. **`js/tags.js` fingerprinting** — canvas, WebGL, AudioContext,
   font list, `navigator.plugins`,
   `Intl.DateTimeFormat().resolvedOptions().timeZone` vs IP geo.
3. **LLM-crawler detection** added mid-2025 — looks for agentic
   headless patterns.
   ([Kameleo 2025 guide](https://kameleo.io/blog/guide-to-bypassing-datadome))
4. **Intent-based scoring** added 2025 — rate/path combinations get
   scored.
5. **Device Check v2** (the "puzzle slider" formerly called
   GeeTest-like) for borderline scores.

**Response patterns:** **403** with body containing
`dd={"rt":"c","cid":…,"hsh":…}` and
`<script src="https://ct.captcha-delivery.com/c.js">`. Header
`x-datadome: …` may be present.

**Cookies:** `datadome=` (primary — alphanumeric, ~200 chars),
Set-Cookie sent on both challenge and success. Session-bound to
TLS+IP+UA; rotating the IP invalidates.

**Bypasses:** Scrapfly claims 96–100%
([bypass page](https://scrapfly.io/bypass/datadome)), full reverse
in [glizzykingdreko Medium](https://medium.com/@glizzykingdreko/breaking-down-datadome-captcha-waf-d7b68cef3e21),
[d-suter/datadome-bp](https://github.com/d-suter/datadome-bp)
cookie generator. Residential/mobile proxy + curl_cffi Chrome 131
impersonation + real `tags.js` execution is the 2026 baseline.

**What's harder for custom V8:** DataDome's tag heavily fingerprints
`AudioContext.createAnalyser().getFloatFrequencyData()` returning
Chrome-exact float values, and `OffscreenCanvas.convertToBlob()` —
you need a full WebAudio and Canvas2D pipeline byte-equal to
Chrome. This is the hardest part of a from-scratch browser.
Camoufox sidesteps it by using Gecko+GeckoView audio which has its
own known fingerprint.

**Test URLs:** `https://antoinevastel.com/bots/datadome`,
`https://www.hermes.com/`, `https://www.rush.com/`.

**Confirmed users (late 2025/2026):** Glassdoor (confirmed),
Crunchbase (confirmed), Leboncoin, Hermes, Rush Street, Reddit
(for bot checks on signup), Allegro.pl, Vinted, Zillow (partial).

---

### Akamai Bot Manager Premier (BMP v3 sensor_data)

Owner: Akamai Technologies. 2025 pitch: "agentic era bot
management" ([blog](https://www.akamai.com/blog/security/bot-management-agentic-era)).

**Top signals:**

1. **`sensor_data` POST** to `/_bm/_data` — AES+custom-encoded
   behavioral telemetry (key+mouse events, accelerometer on
   mobile, JS env). Sent in `X-acf-sensor-data` header or body.
   ([glizzykingdreko Akamai v3 deep dive](https://medium.com/@glizzykingdreko/akamai-v3-sensor-data-deep-dive-into-encryption-decryption-and-bypass-tools-da0adad2a784),
   [xvertile/akamai-bmp-generator](https://github.com/xvertile/akamai-bmp-generator))
2. **TLS JA3/JA4** + HTTP/2 SETTINGS frame order — Akamai was an
   early adopter of JA4 fingerprinting in production.
3. **`bm_sz` / `_abck` cookie lifecycle** — a virgin `_abck` gets
   "validated" to a `~-1~-1~-1~` suffix only after a clean
   `sensor_data` post.
4. **`sec-cpt` challenge** — low-entropy proof-of-work for
   medium-score sessions.

**Response patterns:** HTTP **403** (classic) or **429** with
header `server: AkamaiGHost`, body
`<TITLE>Access Denied</TITLE>`, Set-Cookie for `_abck` containing
`~0~` (invalid suffix).

**Cookies:** `_abck` (main session integrity), `bm_sz` (edge
session), `ak_bmsc` (behavioral session), `bm_mi` (mobile
interstitial), `bm_sv` (sensor version), `_abck` suffix `~-1~-1~-1~`
= valid, `~0~-1~` or `~-1~0~` = flagged.

**Bypasses (2026):**
[xvertile/akamai-bmp-generator](https://github.com/xvertile/akamai-bmp-generator)
(full BMP v3 reverse),
[xiaoweigege/akamai2.0-sensor_data](https://github.com/xiaoweigege/akamai2.0-sensor_data),
Scrapfly 97–100% ([bypass page](https://scrapfly.io/bypass/akamai)).

**What's harder for custom V8:** Akamai's sensor JS reads
`performance.getEntriesByType("navigation")[0]` fields
(DOMContentLoaded, loadEventEnd, transferSize) and cross-checks
against expected Chrome values. It also reads
`WebGLRenderingContext.getParameter(UNMASKED_RENDERER_WEBGL)` —
you MUST have a real GPU string. Mouse/key entropy is the easier
part; the environmental fingerprint is the hard part.

**Test URLs:** `https://www.nike.com/w/mens-shoes-nik1zy7ok`
(confirmed Akamai 2026), `https://www.disneystore.com/`.

**Confirmed users:** Nike (Akamai on storefront — confirmed),
Adidas (confirmed — see customer case study
[akamai.com/resources/customer-story/adidas](https://www.akamai.com/resources/customer-story/adidas)),
Foot Locker, Sony, United Airlines, LinkedIn (Akamai on auth edge
+ in-house Voyager fraud scoring).

---

### Cloudflare Bot Management Enterprise + Turnstile

Owner: Cloudflare.

**Top signals:**

1. **JA4 fingerprint** exposed as `cf.bot_management.ja4` rule
   variable, compared against per-customer traffic baseline.
   ([Cloudflare JA4 signals blog](https://blog.cloudflare.com/ja4-signals/))
2. **HTTP/2 fingerprint** — SETTINGS, WINDOW_UPDATE, HEADERS
   pseudo-header order (akamai_h2), identical to Chrome 131
   exactly.
3. **ML bot score** 1–99 (`cf.bot_management.score`) —
   per-customer model from Jan 2025 onward
   ([per-customer defenses](https://blog.cloudflare.com/per-customer-bot-defenses/)).
4. **Turnstile invisible** — runs VM challenge, posts to
   `/cdn-cgi/challenge-platform/` and issues `cf_clearance`.
5. **Inter-request signals** — path transition Markov chains per
   JA4.

**Response patterns:** HTTP **403** with `cf-mitigated: challenge`,
or **503** "Just a moment…" interstitial. `server: cloudflare`,
`cf-ray: …`.

**Cookies:** `cf_clearance` (4096 bytes max, binds to IP+UA+TLS —
break any one and it's revoked
([docs](https://developers.cloudflare.com/cloudflare-challenges/concepts/clearance/))),
`__cf_bm` (managed challenge session, ~30min), `cf_chl_*` (challenge
state).

**Bypasses:** `curl_cffi` with Chrome 131 impersonation passes Bot
Fight Mode and low-signal enterprise; Turnstile interactive needs a
real browser. **nodriver** currently beats Patchright on Turnstile
(Castle.io June 2025,
[blog](https://blog.castle.io/from-puppeteer-stealth-to-nodriver-how-anti-detect-frameworks-evolved-to-evade-bot-detection/)).
Google patched a "100% precise" Turnstile detection in Aug 2025
([Web Scraper blog](https://webscraper.io/blog/google-patches-100-precise-cloudflare-turnstile-bot-check)).

**What's harder for custom V8:** Turnstile VM probes for **exact V8
bytecode timing** via tight `for` loops and compares against known
Chrome distributions. Your V8 is already V8, so this is a small
win. BUT Turnstile also reads
`navigator.userAgentData.getHighEntropyValues(['architecture','bitness','platformVersion','uaFullVersion','wow64','fullVersionList'])`
and they must exactly match Chrome's format. Doable.

**Test URLs:** `https://nowsecure.nl/`, `https://www.sefon.pro/`,
`https://bot-fight-test.com/` (community-run).

**Confirmed users:** Too many to list; notably Discord edge,
X/Twitter (partial), Udemy, tens of thousands of mid-tier SaaS.

---

### PerimeterX / HUMAN Bot Defender

Owner: HUMAN Security.

**Top signals:** `px.js` runs a VM that collects 150+ signals,
server validates via `_px3`. "Press & Hold" (Human Challenge) for
high-risk sessions.
([Scrapfly PX guide 2025](https://scrapfly.io/blog/posts/how-to-bypass-perimeterx-human-anti-scraping))

**Cookies:** `_px3` (clearance), `_pxvid` (visitor id), `_pxhd`
(encrypted header), `_pxff_*` (feature flags).

**Response:** **403** with body
`{"appId":"PX...","jsClientSrc":"/...","blockScript":"/...","uuid":"..."}`
and HTML `<div id="px-captcha">`.

**Bypasses:**
[Pr0t0ns/PerimeterX-Reverse](https://github.com/Pr0t0ns/PerimeterX-Reverse),
[TakionAPI docs](https://docs.takionapi.tech/px-mobile),
[The Lab #56](https://substack.thewebscraping.club/p/the-lab-56-bypassing-perimeterx-3).

**Harder for custom V8:** Press-and-Hold requires realistic pointer
pressure curves over ~2–3 sec; this is behavioral, not
environmental, so a from-scratch browser doesn't have a structural
disadvantage or advantage.

**Confirmed users:** Walmart (PX + Akamai stacked — confirmed by
[help.refractbot.com/modules/walmart](https://help.refractbot.com/modules/walmart)),
StubHub, Zillow, OpenSea, Crunchyroll, Chegg.

---

### Shape Security / F5 Distributed Cloud Bot Defense

Owner: F5 (since 2020).

**Top signals:** custom JS VM with per-session randomized opcodes,
"superpack" encoded sensor blobs, high-entropy Chrome-object
probing. The 2025/2026 rev reportedly killed most 2024-era bypasses
([Roundproxies F5 bypass 2026](https://roundproxies.com/blog/f5-bypass/),
[ZenRows](https://www.zenrows.com/blog/bypass-f5)).

**Cookies/headers:** `TS01<hex>=…` Set-Cookie, `TSxxxxxxxx` family,
sometimes `ShapeDelegate*`.

**Response:** **200** with obfuscated JS inlined (no block page —
Shape often serves soft deception to make bots waste resources) or
**403** plain.

**Confirmed users:** most major US airlines (Delta, United
secondary), banks (BofA, Wells), Intuit TurboTax login.

**Harder for custom V8:** Among the hardest. Shape's VM
reverse-engineers the browser's OWN VM timing (V8 GC pauses,
inline-cache warmup curves). **Structural advantage for
browser_oxide**: if you control your V8 embedding, you can *spoof
Chrome-identical GC timings* by inserting deliberate pauses —
something Chrome itself can't fake. But the rest (WebGL, Canvas,
Audio) is a massive build-out.

---

### Imperva Advanced Bot Protection (ex-Distil, ex-Incapsula)

Owner: Thales (acquired Imperva 2023).

**Top signals:** `reese84` POST — browser runs obfuscated challenge
VM and submits JSON payload with canvas/webgl/audio/entropy.
`__utmvc` legacy cookie for older integrations.

**Cookies:** `incap_ses_<id>_<site>`, `visid_incap_<site>`,
`reese84` (JSON-carrying token).

**Response:** **403** with `X-Iinfo: …` header (almost unique to
Imperva), body `<html><head><META NAME="robots"…` or reese84
challenge page.

**Bypasses:**
[BottingRocks/Incapsula](https://github.com/BottingRocks/Incapsula)
reese84+`__utmvc` generator,
[Scrapfly guide](https://scrapfly.io/blog/posts/how-to-bypass-imperva-incapsula-anti-scraping).
curl_cffi + reese84 solver handles most cases.

**Confirmed users:** many banks, Caixa, MercadoLibre (partial),
European government sites.

---

### Radware Bot Manager (ex-ShieldSquare)

Owner: Radware.

**Top signals:** JS SDK posts telemetry to `/rbm/…`. `rbzid` cookie
is the clearance token; they market it as "tamper-proof Secure
Identity".

**Cookies:** `rbzid`, `rbzsessionid`.

**Response:** **200 or 403** with Radware-branded block page;
header sometimes `X-SS-*` legacy.

**Confirmed users:** Indian e-commerce (Flipkart legacy), airlines
(IndiGo), SBI.

---

### Variti (Russian)

Owner: Variti (RU). Lower tier vs Qrator but aggressive ML. Uses
JS challenge + behavioral ML with operator correction
([Habr Q&A](https://qna.habr.com/q/994505),
[variti.io/technology](https://variti.io/technology/)).

**Cookies:** `variti-visit-id`, header `X-Variti-*` on some
deployments.

**Confirmed users:** mid-tier Russian retail, smaller banks. Not as
common as Qrator for top-tier.

---

### Yandex SmartCaptcha v3+ & Antirobot

Owner: Yandex. Antirobot is the **internal scoring/blocking
layer** (all Yandex properties), SmartCaptcha is the user-facing
challenge widget.

**Top signals:** Antirobot scores session via a JS beacon (`/jsrs`,
`/checkcaptcha`), SmartCaptcha runs non-image puzzle
(slider/rotate). Heavy TLS+IP reputation — Russian residential IPs
get through much easier.

**Cookies:** `yandexuid`, `yp`, `ys`, `spravka` (whitelist token —
Yandex gives one out after captcha pass; **extremely** valuable if
you can cache it).

**Response:** **200** serving `/showcaptcha?` HTML, or **302** to
showcaptcha.

**Bypasses:**
[yoori/yandex-captcha-puzzle-solver](https://github.com/yoori/yandex-captcha-puzzle-solver),
2Captcha/CapMonster for Smart; `spravka` cookie farming via mobile
Yandex app.

**Confirmed users:** ya.ru, market.yandex.ru, kinopoisk.ru, all
Yandex properties.

---

### NGENIX (Russian CDN/WAF)

Russian Cloudflare-alike, also runs origin DDoS + antibot. Grew
2023+ as RU sites migrated off Cloudflare
([ngenix.net blocking-of-cloudflare post](https://ngenix.net/news/blocking-of-cloudflare-in-russia-migrate-to-the-russian-analogue/)).
JS challenge is lighter than Qrator. Detection identifier:
`Server: NGENIX` header or `ngenix_jscc_*` cookies.

**Confirmed users:** many RU media (RBC, Kommersant partial),
government portals, various e-commerce.

---

### Qrator / Curator

Owner: Qrator Labs (branded Curator in RU market since 2024
rename). Main competitor to Variti/NGENIX in Russia.

**Top signals:** aggressive first-request 403 without JS challenge
if IP/fingerprint is bad
([Habr discussion](https://qna.habr.com/q/1409268)). JS
fingerprinting tied to `qrator_jsid` cookie. DOM inspection,
scoring vs. valid users
([Qrator antibot guide PDF](https://qrator.net/storage/01HMGWZ8BB0B8MKEPBAHSB075T.pdf)).

**Cookies:** `qrator_jsid`, `qrator_ssid`, `qrator_jsr`, header
`Server: QRATOR` (deterministic tell).

**Confirmed users:** dns-shop.ru (confirmed), Tinkoff (partial),
mvideo.ru, many banks.

---

### Volcano Engine / ByteDance Bot Defender (Douyin/TikTok)

Owner: ByteDance. Internal only but also sold via Volcano Engine
cloud. THE hardest CN stack.

**Top signals:** `X-Bogus`/`_signature` (legacy, dying), **`a_bogus`**
(main 2025), **`msToken`**, **`ttwid`**, **`X-Argus`**, **`X-Ladon`**,
**`X-Gorgon`** (mobile). All generated by an obfuscated VM
(`webmssdk.js`) that takes the URL+body and emits a signature.
Heavy environmental fingerprinting via `window` enumeration.

**GitHub reverses:**
[ohpder/douyin](https://github.com/ohpder/douyin),
[NearHuiwen/TiktokDouyinCrawler](https://github.com/NearHuiwen/TiktokDouyinCrawler),
[5ime/Tiktok_Signature](https://github.com/5ime/Tiktok_Signature),
[jackluson/a_bogus_douyin](https://github.com/jackluson/a_bogus_douyin).

**Confirmed users:** douyin.com, toutiao.com, xigua, lark.

**Harder for custom V8:** The `webmssdk.js` VM explicitly
enumerates `Object.getOwnPropertyNames(window)` and hashes it. The
Chrome 131 window namespace has ~900 keys in an exact order. You
must implement every one or be flagged. This is a **massive**
structural tax on from-scratch V8.

---

### Alibaba Cloud Anti-Bot Service (Aliyun)

Owner: Alibaba. Used across Taobao/Tmall/Alipay/Cainiao, 1688, and
sold as a service.

**Top signals:** **`acw_tc`** cookie (anti-crawler-token, signed
server-side), **slider captcha** `nc_1_*`, "big_brother" cookies
for app, behavioral biometrics via `umid.js`, extensive local port
scanning from JS
([HN discussion](https://news.ycombinator.com/item?id=20542350)).

**Cookies:** `acw_tc` (near-deterministic tell across all
Aliyun-protected sites), `cna`, `_m_h5_tk`, `isg`, `t`.

**Response:** **200** serving slider HTML or **405/punish** with
body `{"code":"punish"}`.

**Confirmed users:** taobao.com, tmall.com (both confirmed —
Aliyun stack). **JD.com does NOT use Aliyun** — JD is a direct
Alibaba competitor and runs their own in-house stack (`_JdTdudfp`,
`__jdv`, `__jda`, `__jdb` cookies). **Correction to earlier
assumptions.**

---

### Tencent Cloud WAF Bot + TCaptcha

Owner: Tencent. Lower-pressure than Aliyun/ByteDance but still
serious.

**Top signals:** TCaptcha (slide-to-unlock), behavioral + past-
activity scoring
([2Captcha tencent blog](https://2captcha.com/blog/how-to-solve-tencent-captcha)).
WAF uses `TDID`-style fingerprint cookies on some tenants.

**Cookies:** `pgv_pvid`, `pgv_info`, `ptcz`, `uin`, various
`TSW_*` headers.

**Confirmed users:** weixin.qq.com (partial — WeChat web login does
run TCaptcha), qq.com, Tencent games login, pubg.com global, some
cross-border routes.

---

### Geetest v4

Not a WAF — a captcha vendor. Used on thousands of CN sites as the
challenge layer when the upstream WAF
(Aliyun/Tencent/homegrown) decides to challenge.

**Top signal:** encrypted `w` parameter containing AES-encoded
full interaction trace
([2Captcha v4 support](https://2captcha.com/blog/geetest-v4-support)).
Reversing the `w` param generator requires re-implementing the
obfuscated VM and is a maintenance nightmare
([roundproxies](https://roundproxies.com/blog/bypass-geetest/)).

**Test URL:** [2Captcha v4 demo](https://2captcha.com/demo/geetest-v4).

---

### WBAAS (Wildberries in-house)

Confirmed Wildberries runs their own stack. Public info is thin —
uses a custom JS challenge + ML scoring on `www.wildberries.ru`
but the **`card.wb.ru` JSON API is essentially unprotected**
beyond rate limits. **Strategic answer for WB is to skip the
browser entirely**. See `docs/WILDBERRIES.md` for deep-dive.

---

### Ozon (in-house, not Qrator)

Ozon runs their own `ozon/feature/detect` fingerprint beacon and
ML scoring. Harder than WB because their API endpoints also get
JS-sign checks. Uses `__Secure-user-id`, `abt_data`,
`ADDRESSBOOKBAR_WEB_CLARIFICATION`.

### Avito (in-house)

Avito uses own stack; very aggressive ML on search endpoints.
Datacenter IPs fail immediately. No known public cookie-level tell
beyond `buyer_location_id`, `sessid`.

---

## 2. Corrected fortified-sites matrix

Key corrections from earlier assumptions are called out in bold.

| Site | Earlier guess | **Confirmed / corrected (late 2025)** | Tell from a single GET |
|---|---|---|---|
| nike.com | Akamai | **Akamai** (storefront), **Kasada** (SNKRS draws) | `_abck`, `ak_bmsc`, `server: AkamaiGHost` |
| adidas.com | Akamai | **Akamai confirmed** (customer story) | `_abck`, `bm_sz` |
| linkedin.com | in-house + Akamai | **Akamai edge + in-house Voyager fraud scoring** | `_abck`, `bcookie`, `li_at` |
| instagram.com / meta | in-house | **Meta in-house** (rate + ML); no 3rd-party WAF | `csrftoken`, `mid`, `ig_did` |
| airbnb.com | geo-redirect only | **Kasada** (late 2024 migration) + in-house | `_kuid_`, `x-kpsdk-*` on XHR |
| glassdoor.com | DataDome | **DataDome confirmed** | `datadome=` Set-Cookie, 403 body `dd={…}` |
| crunchbase.com | DataDome | **DataDome confirmed** | same |
| ticketmaster.com | Kasada | **Akamai + Queue-it + in-house Safetix**; Kasada is partial/regional | `_abck`, `TMPS` |
| walmart.com | PX/HUMAN | **PerimeterX + Akamai stacked** (confirmed refractbot docs) | `_px3`, `_abck`, `ak_bmsc` |
| amazon.com | in-house | **Amazon in-house** (CAPTCHA form at `/errors/validateCaptcha`) | `session-id`, `ubid-main` |
| avito.ru | in-house ML | **In-house ML confirmed** | `sessid`, aggressive 429 on datacenter |
| wildberries.ru | WBAAS | **confirmed** | `__wbauid`, `BasketUID`, 498, `server: wbaas` |
| ozon.ru | in-house | **Ozon in-house confirmed** | `__Secure-user-id`, `abt_data` |
| dns-shop.ru | QRATOR | **QRATOR/Curator confirmed** | `qrator_jsid`, `Server: QRATOR` |
| ya.ru | Antirobot+SmartCaptcha | **confirmed** | `yandexuid`, `spravka` |
| taobao.com | Aliyun | **Aliyun confirmed** | `acw_tc`, `_m_h5_tk` |
| tmall.com | Aliyun | **Aliyun confirmed** | `acw_tc`, `cna` |
| jd.com | Aliyun | **CORRECTION: JD in-house**, not Aliyun | `_JdTdudfp`, `__jdv` |
| douyin.com | Volcano Engine | **ByteDance in-house / Volcano** | `ttwid`, `msToken`, `_signature`, `a_bogus` |
| weixin.qq.com | Tencent | **Tencent WAF + TCaptcha confirmed** | `pgv_pvid`, `wxuin` |

---

## 3. Difficulty matrix for a from-scratch Rust/V8 browser

| Engine | Stars | Rationale |
|---|---|---|
| Cloudflare Bot Fight Mode / low signal | ★☆☆☆☆ | curl_cffi-tier; TLS + H2 order is enough |
| Variti | ★★☆☆☆ | JS challenge is solvable if JS executes; RU IP helps |
| NGENIX | ★★☆☆☆ | Similar to Variti; light JS |
| Imperva reese84 | ★★★☆☆ | Need to execute obfuscated VM correctly |
| Yandex SmartCaptcha + Antirobot | ★★★☆☆ | Need puzzle solver OR cached `spravka` |
| Qrator/Curator | ★★★☆☆ | First-request 403 is brutal without a good IP |
| Radware Bot Manager | ★★★☆☆ | Middling; rbzid is solvable |
| Tencent WAF + TCaptcha | ★★★☆☆ | Slider solvable; IP geo matters |
| DataDome | ★★★★☆ | Canvas/Audio fingerprint byte-exact is the hard part |
| Akamai BMP v3 | ★★★★☆ | Sensor data format is reversed but is a moving target |
| PerimeterX / HUMAN | ★★★★☆ | Press-and-Hold + VM challenge |
| Cloudflare Enterprise + Turnstile | ★★★★☆ | Per-customer ML; JA4 aggregate baselines |
| Aliyun Anti-Bot | ★★★★☆ | Slider + umid + `acw_tc` + CN IP all needed |
| Kasada (post-HUMAN) | ★★★★★ | POW + VM + Chrome-quirk probes; hostile to new engines |
| Shape / F5 XC | ★★★★★ | Custom VM, randomized per session, anti-debugging |
| ByteDance Douyin `a_bogus` | ★★★★★ | `window` enumeration + VM + device binding |
| WBAAS browser path | ★★★★☆ | **Strategic answer: skip it, hit `card.wb.ru` JSON** |

---

## 4. Sites ordered easiest → hardest

1. **Cloudflare Bot Fight Mode sites** — Discord web, Udemy, most
   SaaS. Your stack likely passes now.
2. **ya.ru search** — if you farm `spravka` cookies.
3. **dns-shop.ru** — Qrator but well-understood.
4. **avito.ru** — in-house; needs residential RU IP primarily.
5. **glassdoor.com, crunchbase.com** — DataDome, solvable with good
   TLS + canvas.
6. **ozon.ru** — in-house but manageable.
7. **adidas.com, nike.com (storefront)** — Akamai BMP.
8. **linkedin.com** — Akamai + behavioral; auth-gated.
9. **airbnb.com** — Kasada; hard.
10. **taobao.com / tmall.com** — Aliyun; CN IP mandatory.
11. **walmart.com** — PX+Akamai stack.
12. **ticketmaster.com** — Akamai + Queue-it + in-house.
13. **wildberries.ru browser flow** — **hit `card.wb.ru` JSON API instead.**
14. **jd.com** — JD in-house, CN IP, aggressive.
15. **nike.com SNKRS draw** — Kasada; near-impossible without
    mobile proxy + aged account.
16. **douyin.com** — ByteDance stack; CN mobile device simulation
    needed.
17. **US airlines, banks (Shape/F5)** — VM reverse + aged session.
18. **Ticketmaster queue entry during drops** — behavioral ML +
    device verification + queue randomization.

**JSON-API shortcuts:** `card.wb.ru` (Wildberries),
`api.ozon.ru/composer-api.bx` (Ozon partial),
`gateway.chotot.com` (Chotot), `m.jd.com` mobile APIs (sometimes
laxer than desktop), Crunchbase `data.crunchbase.com` (requires
auth).

---

## 5. Where from-scratch V8 has a STRUCTURAL ADVANTAGE

This is the moat. A non-Chromium-wearing-a-stealth-hat engine is
invisible to entire classes of detection:

### Strong advantage

- **Any engine that probes for `cdc_*` /
  `$cdc_asdjflasutopfhvcZLmcfl_`** — browser_oxide simply does not
  have WebDriver's signature at all. Patchright has to patch; we
  just never emit it.
- **`chrome.runtime` / CDP leak detectors** — no CDP client means
  no `__puppeteer_evaluation_script__`, no CDP target events, no
  `Runtime.evaluate` stack signature. Shape and Kasada both probe
  for this.
- **`navigator.webdriver` trap handlers** — Camoufox gets this
  right; Chromium-based stealth gets caught when
  `Object.getOwnPropertyDescriptor(Navigator.prototype, 'webdriver')`
  is inspected. We can natively omit the property.
- **V8 inline-cache / GC timing** — Shape/Kasada compare
  `performance.now()` deltas in tight loops against known Chrome
  distributions. We control the embedding and can tune GC/IC
  warmup to match Chrome 131 exactly — Chromium itself can't do
  this because Chrome's GC is whatever Chrome's GC is.
- **`Function.prototype.toString` leaks** — most stealth tools
  patch `toString` and get caught. We never inject JS, so nothing
  to patch.

### Engines / sites where this advantage lands MOST

| Target | Why it matters |
|---|---|
| **Shape / F5** | Heavy CDP + inline-VM timing probing; we're invisible to both |
| **Kasada ips.js** | Probes `chrome.loadTimes`, `chrome.csi`, CDP hooks — we emit only what we want |
| **Cloudflare Turnstile** | Runtime hook detection based on CDP; we have none |
| **DataDome** | `toString` tampering detection is a big score input |
| **Akamai BMP** | `window.navigator.webdriver` descriptor check is a hard gate |
| **Douyin `a_bogus`** | `Object.getOwnPropertyNames(window)` hash — if we implement the exact Chrome window namespace, we pass once; Chromium-stealth can't hide extension pollution |
| **PerimeterX** | VM detection of Puppeteer/Playwright shims — we have none |

### Strong DISADVANTAGE (MORE work than Camoufox)

- **Canvas/WebGL byte-exact output** — DataDome, Akamai, Aliyun
  all compare Chrome's Skia + ANGLE rendering byte-for-byte. We
  must integrate a Chrome-compatible Canvas2D + WebGL stack.
  Camoufox inherits Gecko's, which is already a known-accepted
  fingerprint.
- **Web Audio fingerprint** — same story; we need Chrome-identical
  FFT output.
- **`window` namespace completeness** — ~900+ properties in exact
  enumeration order; any missing = fail on Douyin, Kasada.
- **Client Hints** (`sec-ch-ua`, `sec-ch-ua-full-version-list`) —
  already doable with boring2 + custom headers, shipped 2026-04-10.
- **HTTP/2 SETTINGS frame order** — solved by boring2 + Chrome 131
  emulation in our `h2_client.rs`; keep it locked.

---

## 6. Strategic recommendation

Lean into the targets where **CDP invisibility + V8 timing
control** outweigh **render stack fidelity**:

1. **Kasada-protected sites** (Kick, Hyatt, Canada Goose, Airbnb
   XHR) — CDP-zero stack is a real moat.
2. **Shape/F5 sites** (airlines, banks) — high-value, hostile to
   Chromium-stealth.
3. **Cloudflare Turnstile invisible** — widespread, advantage is
   real.
4. **PerimeterX sites** (Walmart grocery, Zillow, StubHub) —
   behavioral layer still hard, but VM-detection layer we beat for
   free.

Defer until render-stack parity lands:

1. **DataDome-protected canvas-heavy sites** (Hermes, Rush)
2. **Aliyun** (CN IP + umid is a separate project)
3. **ByteDance Douyin** (window hash + device binding is its own
   program)

For **Wildberries**, just hit `card.wb.ru` JSON. For **Ozon**, use
the mobile `api.ozon.ru` endpoints. For **ya.ru**, invest in a
`spravka` farming pipeline — it's the single most reusable RU
bypass asset.

---

## Caveats

1. Not every cookie name was verified with a live curl; names in
   §1 come from reverse-engineering writeups and vendor docs —
   widely corroborated but a single vendor release could rename a
   field.
2. **Ticketmaster's exact stack is a moving target.** Confidence:
   Akamai is the primary storefront gate; Kasada's role there is
   regional and may have been deprecated. Verify with a fresh
   `curl -I` before building against it.
3. **Kasada post-HUMAN acquisition (Jan 2025)** may see tighter
   cross-product integration with PerimeterX in 2026 — watch for
   shared cookie namespaces.
4. **`spravka` farming** on Yandex has legal/ToS implications
   inside Russia — note for the roadmap, not a recommendation.
5. For Chinese sites, **IP geolocation dominates everything** —
   even a perfect browser from a non-CN IP fails. Factor a CN
   residential layer into cost before scoping Douyin/Taobao.
6. The **structural advantage section is the one to invest in** —
   Cloudflare Turnstile and Kasada/Shape CDP-probing detection
   classes are real and our architecture genuinely sidesteps them.
   The render-stack gap (canvas/webgl/audio byte-exact) is the
   part that will decide whether DataDome and Akamai ever fall.

---

## Sources

- [ZenRows: How to Bypass Kasada in 2026](https://www.zenrows.com/blog/kasada-bypass)
- [Scrapfly: Bypass Kasada 2026](https://scrapfly.io/blog/posts/how-to-bypass-kasada-anti-scraping-waf)
- [The Web Scraping Club #76: Bypassing Kasada 2025](https://substack.thewebscraping.club/p/bypassing-kasada-2025-open-source)
- [0x6a69616e/kpsdk-solver](https://github.com/0x6a69616e/kpsdk-solver)
- [lktop/kpsdk (x-kpsdk-ct/cd analysis)](https://github.com/lktop/kpsdk)
- [Humphryyy/Kasada-Deobfuscated](https://github.com/Humphryyy/Kasada-Deobfuscated)
- [Scrapfly: Bypass DataDome 2026](https://scrapfly.io/blog/posts/how-to-bypass-datadome-anti-scraping)
- [Kameleo: Bypassing DataDome 2025](https://kameleo.io/blog/guide-to-bypassing-datadome)
- [DataDome 2025 Global Bot Security Report](https://datadome.co/resources/bot-security-report/)
- [glizzykingdreko: Breaking Down DataDome](https://medium.com/@glizzykingdreko/breaking-down-datadome-captcha-waf-d7b68cef3e21)
- [d-suter/datadome-bp](https://github.com/d-suter/datadome-bp)
- [glizzykingdreko: Akamai v3 sensor_data deep dive](https://medium.com/@glizzykingdreko/akamai-v3-sensor-data-deep-dive-into-encryption-decryption-and-bypass-tools-da0adad2a784)
- [xvertile/akamai-bmp-generator](https://github.com/xvertile/akamai-bmp-generator)
- [xiaoweigege/akamai2.0-sensor_data](https://github.com/xiaoweigege/akamai2.0-sensor_data)
- [Akamai: Bot Management for the Agentic Era](https://www.akamai.com/blog/security/bot-management-agentic-era)
- [Akamai Adidas customer story](https://www.akamai.com/resources/customer-story/adidas)
- [Cloudflare: clearance cookie docs](https://developers.cloudflare.com/cloudflare-challenges/concepts/clearance/)
- [Cloudflare: JA4 fingerprint signals](https://blog.cloudflare.com/ja4-signals/)
- [Cloudflare: Per-customer bot defenses 2025](https://blog.cloudflare.com/per-customer-bot-defenses/)
- [Castle.io: Puppeteer-stealth to Nodriver evolution](https://blog.castle.io/from-puppeteer-stealth-to-nodriver-how-anti-detect-frameworks-evolved-to-evade-bot-detection/)
- [Web Scraper: Google's 100% Turnstile patch](https://webscraper.io/blog/google-patches-100-precise-cloudflare-turnstile-bot-check)
- [Scrapfly: Bypass PerimeterX 2025](https://scrapfly.io/blog/posts/how-to-bypass-perimeterx-human-anti-scraping)
- [Pr0t0ns/PerimeterX-Reverse](https://github.com/Pr0t0ns/PerimeterX-Reverse)
- [The Lab #56: Bypassing PerimeterX 3](https://substack.thewebscraping.club/p/the-lab-56-bypassing-perimeterx-3)
- [Refract: Walmart PX+Akamai stack](https://help.refractbot.com/modules/walmart)
- [Roundproxies: Bypass F5 2026](https://roundproxies.com/blog/f5-bypass/)
- [ZenRows: Bypass F5 2026](https://www.zenrows.com/blog/bypass-f5)
- [Scrapfly: Bypass Imperva 2026](https://scrapfly.io/blog/posts/how-to-bypass-imperva-incapsula-anti-scraping)
- [BottingRocks/Incapsula (reese84 + __utmvc)](https://github.com/BottingRocks/Incapsula)
- [Habr: Как работают антиботы](https://habr.com/ru/articles/908658/)
- [Habr Q&A: обход Variti](https://qna.habr.com/q/994505)
- [Habr Q&A: обход Qrator](https://qna.habr.com/q/1409268)
- [Qrator AntiBot guide PDF](https://qrator.net/storage/01HMGWZ8BB0B8MKEPBAHSB075T.pdf)
- [NGENIX: Cloudflare migration](https://ngenix.net/news/blocking-of-cloudflare-in-russia-migrate-to-the-russian-analogue/)
- [yoori/yandex-captcha-puzzle-solver](https://github.com/yoori/yandex-captcha-puzzle-solver)
- [Alibaba Cloud Anti-Bot user guide PDF](https://static-aliyun-doc.oss-cn-hangzhou.aliyuncs.com/download/pdf/85363/User_Guide_reseller_en-US.pdf)
- [HN: slider CAPTCHA + port scanning](https://news.ycombinator.com/item?id=20542350)
- [ohpder/douyin (a_bogus etc)](https://github.com/ohpder/douyin)
- [5ime/Tiktok_Signature](https://github.com/5ime/Tiktok_Signature)
- [jackluson/a_bogus_douyin](https://github.com/jackluson/a_bogus_douyin)
- [NearHuiwen/TiktokDouyinCrawler](https://github.com/NearHuiwen/TiktokDouyinCrawler)
- [Tencent Cloud Bot Protection docs](https://www.tencentcloud.com/document/product/627/35641)
- [2Captcha: Tencent captcha](https://2captcha.com/blog/how-to-solve-tencent-captcha)
- [2Captcha: Geetest v4 support](https://2captcha.com/blog/geetest-v4-support)
- [Roundproxies: Bypass Geetest 2026](https://roundproxies.com/blog/bypass-geetest/)
- [daijro/camoufox](https://github.com/daijro/camoufox)
- [Proxies.sx: Camoufox vs Nodriver 2026](https://www.proxies.sx/blog/ai-browser-automation-camoufox-nodriver-2026)
- [scrapfly/Antibot-Detector](https://github.com/scrapfly/Antibot-Detector)
