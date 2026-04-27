# browser_oxide — Next Steps Roadmap

**Status as of 2026-04-10** (cross-references refreshed 2026-04-26). This
document is the master "what's next" for browser_oxide beyond the current
WB investigation. It captures the ambitious goal ("pass every major antibot
system in 2026") plus an honest scoping, the work already shipped, the test
infrastructure gaps that invalidated our prior "71/71 sites pass" claim, and
a tiered execution plan. See also:

- **`docs/SOTA_ROADMAP_2026.md`** — *new (2026-04-26)*. Sequenced 3-phase
  implementation plan for the 2026 SOTA gaps (WebGL execution via wgpu+
  Lavapipe, audio realtime surface, WebAuthn/FedCM/SAB shims, Sigma-Lognormal
  behavioral entropy, H2 golden-frame tests). Run in parallel with the
  site-by-site queue below — the SOTA roadmap closes the *fingerprint
  surface* gaps that block multiple sites at once; this doc sequences the
  *site-specific* investigations.
- `docs/GAPS.md` — P0–P33 fingerprint gaps catalogue. Updated 2026-04-26
  with corrected §26/§27 status and new §33 (deferred QUIC fingerprint).
- `docs/CAPABILITY_GAPS_2026.md` — earlier capability audit. Partially
  superseded by SOTA_ROADMAP_2026.md (audio kernel done; OSMesa-only
  WebGL replaced by cross-platform wgpu+Lavapipe). Tier 1 items T1.1
  (skia-safe Canvas 2D), T1.2 (cosmic-text fonts), T1.5 (Worker plumbing)
  still applicable as written.
- `docs/ANTIBOT_RESEARCH_2026.md` — comprehensive 2026 antibot landscape
  research archive (engine-by-engine, site-by-site, difficulty matrix,
  structural advantages). Primary input for this document's §4.3, §4.4,
  §4.5.
- `docs/WILDBERRIES.md` — WBAAS reverse-engineering, ongoing.
- `docs/STEALTH.md` — the stealth profile architecture.

---

## 1. The mission

Build the most advanced stealth headless browser in Rust — a real
engine, not a Chromium wrapper — and verify that it can pass every
major antibot system in use today. Regional coverage matters: US,
EU, Russia, China. Both "pure-TLS" filters and full JS-fingerprint +
behavioral-ML systems.

The moat is that we are **not Chromium-based**:

- No `cdc_*` variables
- No CDP client, no `/json/version` endpoint
- No WebDriver, no ChromeDriver patches
- No `__puppeteer_evaluation_script__` artifacts
- `Function.prototype.toString` returns native-code strings for our
  stealth patches because they're implemented as V8 ObjectTemplate
  accessors, not JS monkey-patches.

**This matters for antibot systems that specifically probe for
Chromium automation tells.** Kasada, DataDome, PerimeterX, Akamai —
all of them have Chromium-specific detection pipelines that were
built against the assumption that stealth frameworks wear Chrome as
a costume. We don't wear the costume; we wrote a different browser
that happens to impersonate Chrome at the network and JS API layers.

---

## 2. Reality check on scope

What the user asked for ("go one by one, confirm we can pass them all")
is a multi-week roadmap, not a single session. Rough estimate:

| Engine class | Example sites | Sites in scope | Effort per site |
|---|---|---|---|
| "Pure-TLS" filters | nowsecure.nl, Cloudflare basic | 8-10 | Probably already passing — needs verification only |
| Cookie + header engines | Akamai Bot Manager basic, PerimeterX | 10-15 | 1-2 days each; some already pass |
| JS-challenge + fingerprint engines | Kasada, DataDome, WBAAS, QRATOR, NGENIX | 15-20 | 3-7 days each |
| Behavioral ML engines | DataDome "trust profile", Variti, LinkedIn/Instagram in-house | 5-8 | Weeks — requires humanization layer |
| Identity-gated systems | WeChat, Douyin, Taobao slider captcha | 3-5 | **Cannot be fully automated** — real-name verification required |

"Confirm we pass" is tractable for ~40 sites. For the last 5 (WeChat
web, Douyin logged-in, Instagram behavioral scoring, Taobao slider
biometric acceleration, and the aged-account-only flows), the honest
answer is **there is no pure-automation bypass in 2026**. Those are
"hit the mobile API with a real-account session" or "don't bother."

Writing this down because it affects how we talk about the mission:
"the most advanced stealth browser" does not mean "passes literally
everything". It means "passes everything a from-scratch browser
engine _could_ pass, better than any other from-scratch browser
engine, with structural advantages that Chromium-based frameworks
don't have."

---

## 3. What the existing test suite claims vs reality

`crates/browser/tests/anti_bot_sites.rs` currently has probes for
50+ sites across every major vendor. It calls itself "71/71 passing"
in `docs/GAPS.md`. **That claim is wrong.**

The existing `probe()` helper classifies a site as "passed" if:

```rust
passed = resp.status == 200 || resp.status == 301 || resp.status == 302
```

Under that definition, every site we found failing on 2026-04-10 was
counted as a pass:

- **ya.ru** — 302 to `ya.ru/?nr=1&redirect_ts=...` with empty brotli
  body. Counted as "pass" because it's a 302. In reality our decoder
  errored out on the empty body (now fixed).
- **ozon.ru** — 307 to `ozon.ru/?__rr=1`. Counted as "pass". In
  reality the `__rr` redirect chain is stateful and terminates in a
  403 with a 99 KB challenge HTML.
- **wildberries.ru** — 498 challenge page. Counted as a "known block"
  under "IP reputation (datacenter IP)". In reality, fully solvable
  from this IP in a real Chrome — the mislabeling was an assumption
  made without investigation.
- **dns-shop.ru** — 401 QRATOR challenge. Same bucket — "IP
  reputation". In reality the site uses QRATOR's `__qrator` challenge
  JS, and the JSON microdata endpoint isn't even protected.

Core problems with the current probe:

1. **No redirect following.** Every antibot in 2026 uses 307/302 with
   a marker parameter as a cheap first filter (Ozon's `__rr`,
   Yandex's `nr`, Cloudflare's `cf_clearance` redirect). A single GET
   that stops at the first response misses the challenge entirely.
2. **No content validation.** A 200 response can still be a challenge
   page, a placeholder, a geo-redirect, a blank index, or a
   degraded-content "shadow ban" page (Avito explicitly does this).
   We need per-site validators that check for real content markers.
3. **Detection pattern list is incomplete.** `detect_protection`
   checks for `cf-ray`, `x-datadome`, `_abck`, `x-px`, `kasada`,
   `x-kpsdk-ct`. Missing: QRATOR (`server: QRATOR`), Variti,
   NGENIX (`ngenix_*` cookies), WBAAS (`server: wbaas`,
   `status-no-id: PG-*`, `x-wbaas-token` header), Yandex Antirobot
   (`spravka` cookie), Aliyun (`aliyun-acs-*`), Volcano Engine
   (`msToken`), Geetest (`gt=...` query params), Imperva
   (`visid_incap_*`), Radware (`rdwr_*`).

**The real pass count is unknown and we cannot quote numbers until
we rebuild the harness.**

---

## 4. What this roadmap builds

Four deliverables:

### 4.1 `docs/ANTIBOT_MATRIX.md` — the living scoreboard

One table. Every row is a site. Columns:

- **Engine** — the antibot vendor/product as confirmed by a fresh
  probe (not what we assume)
- **L1** — TLS handshake succeeds
- **L2** — After following redirects, we get a final response (not
  connection dropped / TLS EOF / 5xx)
- **L3** — Final body is **not** a challenge/captcha/interstitial
  page, validated against site-specific content markers
- **L4** — (reserved) JS runtime executes the page bootstrap without
  errors, `document.title` and `document.body.textContent` look
  right for a real user
- **L5** — (reserved) Can follow one internal link (e.g. a product
  card or search result) to a detail page and still get real content

Cells are `PASS`, `FAIL: <reason>`, or `NOT_TESTED`. No aggregate
"X/Y passing" statistics until every cell in the row is green.
Date-stamped so we can track regressions.

### 4.2 `crates/browser/tests/antibot_matrix.rs` — rigorous probes

New test harness with:

```rust
pub struct ProbeConfig {
    pub url: &'static str,
    pub profile_fn: fn() -> StealthProfile,
    pub expected_engine: Option<&'static str>,
    pub content_markers: &'static [&'static str],  // all must appear in final body
    pub negative_markers: &'static [&'static str], // none may appear
    pub follow_redirects: u8,
}

pub async fn rigorous_probe(cfg: &ProbeConfig) -> ProbeResult { ... }
```

Per-site validators. Example for `nike.com`:

```rust
ProbeConfig {
    url: "https://www.nike.com",
    profile_fn: stealth::chrome_130_windows,
    expected_engine: Some("akamai"),
    content_markers: &["Nike", "<html", "swoosh"],  // real homepage markers
    negative_markers: &["Pardon Our Interruption", "sensor_data", "Access Denied"],
    follow_redirects: 10,
}
```

The harness:

1. Runs `HttpClient::get_follow(url, cfg.follow_redirects)`.
2. Calls a shared `detect_engine()` helper with the expanded pattern
   set (QRATOR, WBAAS, Variti, NGENIX, Yandex Antirobot, Aliyun,
   Volcano, Tencent, Geetest, Imperva, Radware).
3. Classifies into L1 / L2 / L3.
4. Outputs structured JSON to stdout so we can diff runs over time.

A single test function iterates all `ProbeConfig`s and emits one
result per site. We can commit the JSON baseline to git and each
subsequent run becomes a regression test.

### 4.3 Extended engine detection

`crates/browser/tests/common/engine_detect.rs` (new) — centralizes the
signal matching so the same logic is used by `antibot_matrix.rs`,
`debug_blocked.rs`, and `challenge_solver.rs`. Patterns (updated
2026-04-10 with research from `docs/ANTIBOT_RESEARCH_2026.md`):

| Engine | Primary signal | Confirmatory signal | Response pattern |
|---|---|---|---|
| **Cloudflare CF** | `cf-ray` header | `__cf_bm` cookie, `server: cloudflare` | 503 "Just a moment" interstitial, `cf_chl_*` cookies |
| **Cloudflare Turnstile** | `cf-mitigated: challenge` header | `/cdn-cgi/challenge-platform/` in body | 403 with challenge HTML, `cf_clearance` cookie after pass |
| **DataDome** | `x-datadome` or `x-dd-b` response header | `datadome=` Set-Cookie (~200 chars) | 403 body contains `dd={"rt":"c","cid":...,"hsh":...}` and `<script src="https://ct.captcha-delivery.com/c.js">` |
| **Akamai Bot Manager** | `_abck` cookie, suffix `~0~` = flagged, `~-1~-1~-1~` = valid | `bm_sz`, `ak_bmsc`, `bm_mi`, `bm_sv`, `server: AkamaiGHost` | 403 body `<TITLE>Access Denied</TITLE>`, POST to `/_bm/_data` with `X-acf-sensor-data` header |
| **PerimeterX / HUMAN** | `_px3` cookie | `_pxvid`, `_pxhd`, `_pxff_*` | 403 body JSON `{"appId":"PX...","blockScript":"/..."}` + `<div id="px-captcha">` |
| **Kasada** | `x-kpsdk-ct` response header | `x-kpsdk-cd`, `x-kpsdk-v`, `x-kpsdk-r`, `x-kpsdk-h` — **all header-driven, not cookie-driven** | 429 with minimal body, `/ips.js` + `/149e9513-…/fp` endpoints in body |
| **Shape / F5** | `TS01<hex>=` Set-Cookie | `TSxxxxxxxx` family, sometimes `ShapeDelegate*` | 200 with inlined obfuscated JS (deception) OR 403 plain |
| **Imperva (Incapsula)** | `X-Iinfo` response header (near-unique) | `visid_incap_<site>`, `incap_ses_<id>_<site>`, `reese84` | 403 + `reese84` challenge POST, or `<META NAME="robots"` redirect body |
| **Radware Bot Manager** | `rbzid` Set-Cookie | `rbzsessionid`, legacy `X-SS-*` | 200 or 403, Radware-branded block page |
| **Cloudflare Bot Fight Mode** | `cf-ray` header, no `cf-mitigated` | passes with `__cf_bm` issued | — |
| **QRATOR / Curator** | `server: QRATOR` header (deterministic) | `qrator_jsid`, `qrator_ssid`, `qrator_jsr` cookies | 401 or 403, `/__qrator/qauth*` script src in body |
| **Variti** (Russian) | `variti-visit-id` cookie | `X-Variti-*` headers | — |
| **NGENIX** (Russian) | `server: NGENIX` header | `ngenix_jscc_*`, `ngenix_jscv_*` cookies | JS challenge lighter than Qrator |
| **WBAAS** (Wildberries) | `server: wbaas` header | `status-no-id: PG-*`, `x-wbaas-token: get`, `x_wbaas_token` / `__wbauid` / `BasketUID` cookies | 498 status, `data-req-uuid` on `<html>` element |
| **Yandex Antirobot** | `spravka` Set-Cookie (success path) | `yandexuid`, `yp`, `ys` cookies | 302 to `/showcaptcha?retpath=`, or 200 serving showcaptcha HTML |
| **Aliyun Anti-Bot** | `acw_tc` cookie (near-deterministic) | `cna`, `_m_h5_tk`, `isg`, `t`, `umid.js` script | 200 slider HTML or 405 body `{"code":"punish"}` |
| **Volcano Engine / ByteDance** | `ttwid` cookie | `msToken` cookie/query param, `a_bogus` request header, `X-Argus`/`X-Ladon`/`X-Gorgon` on mobile | 403 or soft-200 with degraded data |
| **Tencent WAF + TCaptcha** | `pgv_pvid`, `pgv_info`, `ptcz` cookies | TCaptcha slide-to-unlock HTML | — |
| **Geetest v4** | `<script src="...geetest..."` or `/captcha/...` | encrypted `w` query parameter | — (captcha vendor, upstream WAF decides when to invoke) |
| **JD.com in-house** (not Aliyun) | `_JdTdudfp` cookie | `__jdv`, `__jda`, `__jdb`, `__jdc` cookies | — |
| **Ozon in-house** | `abt_data` cookie | `__Secure-user-id`, `ADDRESSBOOKBAR_WEB_CLARIFICATION` | 307 chain on `/?__rr=<n>` |
| **Avito in-house** | `sessid` cookie | — | Aggressive 429 on datacenter IPs |
| **Amazon in-house** | `session-id`, `ubid-main` cookies | `/errors/validateCaptcha` form | 200 with captcha form |
| **Meta/Instagram in-house** | `csrftoken`, `mid`, `ig_did` cookies | — | 200 with rate-limit or ML score |
| **LinkedIn** | `bcookie`, `li_at` + Akamai `_abck` | Akamai edge + in-house Voyager fraud | stacked |

**Detection strategy**: match **multiple** signals before assigning
an engine — single-cookie matches produce false positives
(e.g., `_px3` occasionally appears on non-PX sites).

### 4.4 Ordered work queue

The site list, grouped by realistic effort:

### 4.5 Antidetect-equivalent fingerprint verification (the second matrix)

Separate from the antibot-site matrix. Instead of "does site X block
us?" this asks "does fingerprint tester X consider us a real,
unique, consistent device?" The target is to match or exceed the
trust scores that commercial antidetect browsers (Dolphin{anty},
AdsPower, Multilogin, Kameleo, GoLogin) get on the same tests.

#### What antidetect browsers actually expose (vs. the marketing)

The popular description — "real BIOS IDs, Disk IDs, GPU signatures"
— is misleading. Web pages **cannot read** BIOS UUID, disk serial,
or MAC address; those are OS-level identifiers with no JavaScript
API. What antidetect browsers actually provide:

1. **Consistent JS-layer fingerprints per profile** — Canvas pixel
   hashes, WebGL vendor/renderer strings + matching extension list
   + shader-precision triples, AudioContext output hash, font
   enumeration, screen metrics, timezone, language, plugins,
   `navigator.mediaDevices.enumerateDevices()` output.
2. **Stability** — the same profile produces the same fingerprint
   across runs.
3. **Diversity** — their catalogs contain hundreds of profiles that
   each look like a different real device (Intel UHD 620 on a Dell
   laptop, NVIDIA RTX 3060 on a gaming desktop, Apple M2 Air, etc.).
4. **Real IP pairing** — residential/mobile proxies tied to each
   profile's claimed locale.
5. **Persistent storage** — cookies, localStorage, IndexedDB, cache
   accumulated across sessions to build "trust age".
6. **WebRTC local-IP masking** — `new RTCPeerConnection()` normally
   leaks your LAN IP via STUN `host` candidates. Antidetect browsers
   either block the API or return a spoofed candidate in the proxy's
   subnet. **Blocking can itself be detected** (a real browser
   always exposes RTCPeerConnection as working); returning a fake
   candidate is stealthier.

#### What we already have

- `StealthProfile.canvas_seed` — deterministic canvas rendering
  variance per profile.
- `StealthProfile.audio_seed` — deterministic audio output per
  profile.
- `StealthProfile.webgl_vendor` + `webgl_renderer` — GPU identity
  strings.
- `StealthProfile.media_devices` — `enumerateDevices()` stubs.
- `window_bootstrap.js:1067` — `RTCPeerConnection` is shadowed with
  a no-op class. **This is item 6's "block" variant and is itself
  detectable by advanced probes.** Worth replacing with a fake-
  candidate implementation that behaves like a real RTCPeerConnection
  and emits one or two plausible local-IP candidates matching our
  proxy's subnet.
- `Function.prototype.toString` masking for stealth patches is
  natively native — a structural advantage we keep.
- `chrome_compat` tests cover 216 API contracts.
- GAPS.md §P0 items 3, 6, 7 open: audio fingerprint parameters,
  canvas fonts per OS, WebGL extension lists per GPU.

#### What's missing (the antidetect gap)

1. **GPU catalog diversity.** We have a handful of preset GPUs.
   Commercial antidetect browsers ship with catalogs of 100-500
   real GPU + driver + extension-list + shader-precision-format
   combinations. Need a `stealth::gpu::catalog` module with at
   least 30-50 real desktop GPUs across Intel, NVIDIA, AMD, Apple,
   plus matching:
   - `UNMASKED_VENDOR_WEBGL` (e.g. `"Google Inc. (NVIDIA)"`)
   - `UNMASKED_RENDERER_WEBGL` (e.g. `"ANGLE (NVIDIA, NVIDIA GeForce RTX 3060 Direct3D11 vs_5_0 ps_5_0, D3D11)"`)
   - Extension list (real Chrome dumps: WEBGL_debug_renderer_info,
     WEBGL_compressed_texture_s3tc, EXT_color_buffer_half_float, etc.)
   - `getParameter` limits: `MAX_TEXTURE_SIZE`, `MAX_RENDERBUFFER_SIZE`,
     `MAX_VIEWPORT_DIMS`, `MAX_VERTEX_UNIFORM_VECTORS`, etc.
   - `getShaderPrecisionFormat` for all 12 combinations (vertex/
     fragment × low/medium/high × float/int)
2. **Font list per OS + version.** `StealthProfile` currently has
   no `fonts` field. Antidetect browsers ship with canonical font
   lists: Windows 10 vs Windows 11 (different defaults), macOS
   Ventura vs Sonoma, Ubuntu default stack. CreepJS enumerates
   fonts via measurement-width difference across 1000+ candidate
   families; an empty or obviously-faked list is an instant tell.
3. **Battery API.** Desktop should report `{charging: true, level:
   1.0, chargingTime: 0, dischargingTime: Infinity}`. Laptops report
   a plausible level. We currently don't implement `navigator
   .getBattery()`.
4. **Hardware concurrency + memory pair sanity.** Realistic
   combinations: 4c/8gb (entry), 6c/16gb (mid), 8c/16gb (mainstream),
   12c/32gb (enthusiast). Avoid `cpu_cores=16, device_memory=2` and
   similar impossible pairs.
5. **Screen profile diversity.** We need common laptop
   resolutions: 1366x768, 1536x864, 1440x900, 1920x1080, 2560x1440,
   3440x1440 (ultrawide), Retina variants (2880x1800 @2x,
   3024x1964 @2x MBP, etc.). With matching `devicePixelRatio`,
   `colorDepth=24`, `pixelDepth=24`.
6. **`RTCPeerConnection` with fake local-IP candidate.** Replace
   the current no-op shim with an implementation that mimics a real
   STUN handshake flow and emits an ICE candidate with a plausible
   local IP (e.g. `192.168.1.<hash>`), matching the proxy's
   perceived subnet. Needs wiring through `StealthProfile
   .local_ip_seed` so it's stable per profile.
7. **`navigator.connection.effectiveType` distribution.** Already
   in the profile, but ensure presets don't all claim `4g` —
   `wifi` is also valid for desktop.
8. **Font-face rendering determinism.** If/when we render canvas
   with real fonts (canvas crate), the pixel output must match the
   declared OS's hinting + subpixel mode (Windows = ClearType,
   macOS = grayscale, Linux = varies). This is GAPS.md §P0 item 6.

#### Fingerprint test sites for the verification matrix

Public, no-account-required fingerprint probes. These are the
"second matrix" — they test our browser against a reference
scorer, not against a site's antibot gate. We should hit each one
with every `StealthProfile` preset and record:

- Whether the page loaded to completion (L1-L3 from the antibot
  matrix apply here too)
- The specific fingerprint hashes or scores reported
- Whether the scorer flagged us as a bot / headless / automation

| Site | What it tests | Signal we want |
|---|---|---|
| `https://abrahamjuliot.github.io/creepjs` | Everything CreepJS knows (prototype integrity, API coverage, fingerprint diffs in iframes and workers, lies detection) | Trust score ≥ 70%, no critical lies flagged, "Bot" verdict absent |
| `https://bot.sannysoft.com` | Classic headless-Chrome tells (webdriver, plugins, languages, platform, permissions, notifications, WebGL vendor, canvas) | All rows green |
| `https://bot.incolumitas.com` | Newer version of sannysoft with stricter checks | Score ≥ 0.9 |
| `https://www.browserleaks.com/canvas` | Canvas fingerprint hash | Hash is stable across runs with same profile, different across different profiles |
| `https://www.browserleaks.com/webgl` | WebGL vendor/renderer/extension list/params | All values consistent with claimed GPU; extension list not obviously empty |
| `https://www.browserleaks.com/audio` | AudioContext output hash | Stable per-profile, diverse across profiles |
| `https://www.browserleaks.com/fonts` | Font enumeration + measurement | Font list matches claimed OS |
| `https://www.browserleaks.com/webrtc` | RTCPeerConnection local-IP leak | No LAN IP exposed; STUN candidates look plausible |
| `https://www.browserleaks.com/features` | Full overview (navigator, screen, timezone, etc.) | All fields consistent |
| `https://pixelscan.net` | Multi-signal score + "consistency" verdict | "Consistent" verdict, no automation flags |
| `https://iphey.com` | IP + browser fingerprint cross-check | High trust score |
| `https://amiunique.org/fingerprint` | Fingerprint uniqueness across their dataset | We want a plausible score, not an outlier |
| `https://fingerprint.com/products/bot-detection` | FingerprintJS Pro bot demo | "Human" verdict |
| `https://coveryourtracks.eff.org/` | EFF fingerprint uniqueness and tracker protection | Plausible |
| `https://arh.antoinevastel.com/bots/areyouheadless` | Quick headless check | "Not headless" |
| `https://arh.antoinevastel.com/bots` | Broader Vastel bot tests | All green |
| `https://bot-detector.rebrowser.net` | Rebrowser's advanced checker (targets modern stealth frameworks) | Score ≥ their Dolphin{anty} baseline |
| `https://f.vision` | Newer fingerprint inspector | Plausible |
| `https://deviceandbrowserinfo.com` | Simple device info display | Matches claimed profile |

Commercial antidetect-browser checkers with public demo pages
(where accessible without an account — verify before adding):

- Dolphin{anty} fingerprint demo (if exposed publicly)
- Multilogin browser fingerprint test page
- GoLogin fingerprint checker
- Kameleo fingerprint test
- AdsPower profile validator

#### Trust-score targets

Per-site thresholds we want to hit or beat. Final numbers depend on
what our first baseline run shows, but the rough bar:

| Site | Minimum to ship | Stretch goal (antidetect parity) |
|---|---|---|
| CreepJS | Trust ≥ 50%, "Bot" absent | Trust ≥ 75%, 0 critical lies |
| bot.sannysoft.com | All 18 rows green | — (hard max) |
| bot.incolumitas.com | Score ≥ 0.8 | Score ≥ 0.95 |
| pixelscan.net | "Consistent" | "Consistent" + low-risk |
| rebrowser bot-detector | No "automation detected" | Score matches real Chrome |

Note: an earlier version of GAPS.md claimed **18/18 stealth checks,
higher than Chrome headless (16/18), Puppeteer+Stealth (14/18),
Camoufox (13/18), Lightpanda (8/18)**. That baseline was measured
against an internal harness, not against CreepJS trust score. **We
don't know where we actually stand against CreepJS yet.** GAPS.md
was rewritten 2026-04-26 to drop the unverified scoreboard until a
reproducible IP-disclosed benchmark backs it up. First baseline run
will tell us.

#### Diversity test (fingerprint uniqueness across profiles)

Second category of test: given N `StealthProfile::random_*`
profiles, verify that each produces a **different** fingerprint on
the test sites. Antidetect browsers sell on "1000 unique profiles,
no two match". Our diversity story is currently limited to
canvas/audio seeds; the new catalog work adds GPU, screen, font,
and memory dimensions.

Acceptance: 100 random profiles → 100 distinct canvas hashes, 100
distinct WebGL parameter tuples, 100 distinct audio fingerprints,
no duplicate full fingerprint vectors.

#### Integration with §4.1 and §4.2

The matrix helper (`rigorous_probe()` in §4.2) is extended with a
second mode: `fingerprint_probe(url, profile, expected)`. Same
harness, different validators:

- Instead of "body contains expected site-content markers", it's
  "page reported trust score ≥ X" or "page reported canvas hash
  matches our expected deterministic hash for this profile".
- Results go into a second table in `docs/ANTIBOT_MATRIX.md`
  titled "Fingerprint verification matrix".

---

#### Tier 0 — Free wins (verification only)

Expected to already pass. Just need to be rerun through the new
rigorous harness to confirm.

Real sites (antibot gates we should walk through):

- `nowsecure.nl` (Cloudflare basic)
- `https://discord.com`
- `https://medium.com`
- `https://chatgpt.com`
- `https://www.coinbase.com`
- `https://www.bet365.com`
- Cloudflare-lite tier in general

Fingerprint reference scorers (the §4.5 second matrix — treat as a
separate test set with different validators):

- `https://abrahamjuliot.github.io/creepjs` — trust score goal
- `https://bot.sannysoft.com` — all rows green
- `https://bot.incolumitas.com` — newer, stricter
- `https://www.browserleaks.com/canvas` — hash stability + diversity
- `https://www.browserleaks.com/webgl` — GPU consistency
- `https://www.browserleaks.com/audio` — audio hash
- `https://www.browserleaks.com/fonts` — font enumeration
- `https://www.browserleaks.com/webrtc` — local-IP leak check
- `https://www.browserleaks.com/features` — overview
- `https://pixelscan.net` — "Consistent" verdict
- `https://iphey.com` — IP + fingerprint cross-check
- `https://amiunique.org/fingerprint` — uniqueness across dataset
- `https://fingerprint.com/products/bot-detection` — FingerprintJS Pro demo
- `https://arh.antoinevastel.com/bots/areyouheadless` — quick headless check
- `https://arh.antoinevastel.com/bots` — broader Vastel bot tests
- `https://bot-detector.rebrowser.net` — rebrowser advanced checker
- `https://f.vision` — newer fingerprint inspector

Diagnostic mirrors (not targets, but used to verify our network shape):

- `https://tls.peet.ws/api/all` — TLS/H2 fingerprint reflected as JSON
- `https://browserleaks.com/http2` — Akamai H2 fingerprint reflected

Success criterion for Tier 0:

- Antibot gates: L3 PASS, content markers present, no JS errors.
- Fingerprint scorers: trust score meets the §4.5 targets, no
  "automation detected" or "headless" verdicts, fingerprint is
  stable across runs with the same profile and distinct across
  different profiles.

#### Tier 0.5 — Structural-advantage targets (our moat)

Per `docs/ANTIBOT_RESEARCH_2026.md §5`, these are the engines /
sites where a **from-scratch, non-Chromium, no-CDP, natively-native
`Function.prototype.toString`** browser has a REAL structural
advantage over Camoufox / Patchright / nodriver / undetected-
chromedriver. The hypothesis is that these should work with LESS
effort than the tier assignment suggests, because they're designed
to detect Chromium-wearing-a-disguise and we aren't wearing one.

**High-value moat targets**:

- **Cloudflare Turnstile invisible** — probes for Puppeteer CDP
  hooks and `cf.bot_management.score` signals. Test targets:
  `https://nowsecure.nl/` (already in Tier 0), any Turnstile-
  protected mid-tier SaaS.
- **Kasada** — `ips.js` + `p.js` VM probes for `chrome.loadTimes`,
  `chrome.csi`, CDP hooks, `Function.prototype.toString` tampering.
  We emit only what we want to emit, and we implement the Chrome
  shape natively. Test targets: `https://www.canadagoose.com/`,
  `https://www.kick.com/`, `https://www.hyatt.com/`. **Very high
  confidence of structural advantage, low confidence of render-
  stack sufficiency** — the canvas/audio part might still block us.
- **Shape / F5** — among the hardest for Chromium-stealth. Probes
  V8 GC timing and CDP existence. We control the embedding, so we
  can tune GC/IC warmup. Test targets: `https://www.delta.com/`
  (Delta uses Shape secondarily), `https://turbotax.intuit.com/`.
- **PerimeterX / HUMAN Bot Defender** — `px.js` VM detects
  Puppeteer/Playwright shims. We have none. Test targets:
  `https://www.walmart.com/grocery`, `https://www.zillow.com/`,
  `https://www.stubhub.com/`.
- **Akamai Bot Manager** — reads `Object.getOwnPropertyDescriptor(
  Navigator.prototype, 'webdriver')` as a hard gate. Camoufox gets
  this right; Chromium-stealth doesn't. We natively omit the
  property. Test target: `https://www.adidas.com/` (confirmed
  Akamai customer).
- **DataDome** — `Function.prototype.toString` tampering detection
  is a score input. We have no tampering. Test targets:
  `https://www.glassdoor.com/`, `https://www.crunchbase.com/`.

**Success criterion for Tier 0.5**: at least ONE site per engine
lands at L3 PASS. That validates the structural-advantage thesis
and tells us which disadvantage (canvas, audio, etc.) is the next
bottleneck.

**Explicit risk**: the structural advantages matter for the
Chromium-detection layer of each engine. Each engine ALSO has a
canvas/audio/webgl byte-exact comparison layer (stronger for
DataDome and Akamai, weaker for Kasada) that we currently CANNOT
match without implementing a Chrome-compatible Skia + ANGLE + Web
Audio pipeline. Tier 0.5 may land at "challenge JS accepts us, then
fingerprint hash fails" — same class as the current WB blocker.

#### Tier 1 — One-session fixes (apply already-shipped fixes, verify)

Sites that should benefit from fixes shipped 2026-04-06 → 2026-04-10:
the brotli empty-body fallback, high-entropy Client Hints,
`userAgentData.getHighEntropyValues()`, reload-shape headers on
retry, inline script execution, `document.location` alias,
`window.top/parent/frames`, stale H2 connection recovery.

- **ya.ru / yandex.ru** — was "brotli error", now should work
  through the initial 302. Followed by a captcha-free search path
  via `yandex.ru/search/?text=...` as long as Antirobot's trust
  score doesn't flag us. **Strategic shortcut**: Yandex.XML
  partner API (`yandex.ru/search/xml?...&user=...&key=...`) is
  captcha-free and paid.
- **airbnb.com** — corrected: **Kasada** (late 2024 migration),
  not just a geo-redirect. Belongs in Tier 0.5 too.
- **linkedin.com**, **amazon.com**, **google.com** — public
  homepages, believed to work; verify after the Client Hints ship.
  LinkedIn is Akamai + in-house Voyager so auth-gated pages may
  still fail.
- **stockx**, **nordstrom**, **walmart.com/grocery** — PerimeterX.
  Move to Tier 0.5 as structural-advantage targets.
- **discord.com**, **medium.com**, **chatgpt.com** — Cloudflare
  edge, should pass with current TLS + Client Hints.

Success criterion for Tier 1: L3 PASS within a single debug session
per site, no multi-day reverse engineering.

#### Tier 2 — Multi-session deep investigations

Each of these is its own mini-Wildberries. Budget days, not hours.
Per-site or per-engine doc under `docs/`. Confirmed engine
assignments from `docs/ANTIBOT_RESEARCH_2026.md`.

- **wildberries.ru** (WBAAS, in-house) — in progress. Blocked on
  fingerprint hash investigation: `challenge_fingerprint_v1.0.23
  .js`. See `docs/WILDBERRIES.md §5`. **Strategic shortcut**: hit
  `card.wb.ru` / `search.wb.ru` / `catalog.wb.ru` JSON APIs
  directly — the research confirms these are essentially
  unprotected beyond rate limits. Build first, then return to the
  browser path for completeness.
- **ozon.ru** (in-house, NOT Qrator) — 307 chain on
  `/?__rr=<n>` → 403 with 99 KB challenge HTML. Reverse-engineer
  the 99 KB page's JS (`ozon/feature/detect` fingerprint beacon).
  Create `docs/OZON.md`. Cookies to watch: `abt_data`,
  `__Secure-user-id`, `ADDRESSBOOKBAR_WEB_CLARIFICATION`.
  **Strategic shortcut**: `api.ozon.ru/composer-api.bx` or mobile
  `x-o3-app-*` path.
- **dns-shop.ru** (QRATOR / Curator, confirmed) — 401 with
  `/__qrator/qauth_utm_v2d_v9118.js` challenge. Create
  `docs/QRATOR.md`. **Strategic shortcut confirmed by research**:
  `https://www.dns-shop.ru/product/microdata/<uuid>/` is a plain
  JSON endpoint, no challenge, no CSRF.
- **DataDome sites** — glassdoor, crunchbase, leboncoin, hermes,
  rush street, reddit (signup only), vinted. Single
  `docs/DATADOME.md` covers the whole class. Detection: `datadome=`
  cookie, `dd={...}` JSON body, `x-datadome` header. Key blocker:
  `AudioContext.getFloatFrequencyData()` and `OffscreenCanvas
  .convertToBlob()` byte-exact — this is the render-stack fidelity
  gap.
- **Akamai Bot Manager Premier sites** — nike storefront, adidas,
  foot locker, sony, united airlines, linkedin edge. Require
  `sensor_data` POST to `/_bm/_data` with AES+custom encoded
  behavioral telemetry, and valid `_abck` cookie lifecycle
  (`~-1~-1~-1~` suffix). `docs/AKAMAI.md`. Reverse references:
  `xvertile/akamai-bmp-generator`, `glizzykingdreko` Akamai v3
  writeup.
- **Kasada sites (post-HUMAN acquisition)** — canadagoose, kick,
  hyatt, airbnb XHR, nike SNKRS draws, twitch auth, ticketmaster
  (partial/regional). `/ips.js` POW + `p.js` VM. `docs/KASADA.md`.
  Reverse references: `0x6a69616e/kpsdk-solver`, `lktop/kpsdk`.
- **PerimeterX / HUMAN Bot Defender sites** — walmart grocery,
  stubhub, zillow, opensea, crunchyroll, chegg. Press-and-Hold
  challenge + 150-signal VM. `docs/PERIMETERX.md`. Reverse
  references: `Pr0t0ns/PerimeterX-Reverse`.
- **Shape / F5 sites** — delta airlines, BofA, Wells Fargo,
  Intuit TurboTax login. Custom per-session VM with randomized
  opcodes. `docs/SHAPE.md`. This is the highest structural-
  advantage target per the research.
- **Chinese sites** — taobao, tmall (Aliyun, confirmed),
  **jd.com (in-house, NOT Aliyun — CORRECTION)**,
  douyin (Volcano Engine / ByteDance). Each gets its own doc:
  `docs/ALIYUN.md`, `docs/JD_INHOUSE.md`,
  `docs/BYTEDANCE.md`. **All require CN residential IPs** — no
  pure-browser work lands without the network layer.
- **Yandex Antirobot full pass** — beyond loading the home page,
  actually performing a search without triggering the
  `/showcaptcha` handoff. Requires behavior + `spravka` cookie
  warming. Research calls this "the single most reusable RU
  bypass asset". `docs/YANDEX.md`.

**Corrections applied from 2026-04-10 research**:

- Airbnb moved from "geo-redirect only" → **Kasada** (late 2024
  migration).
- Ticketmaster moved from "Kasada primary" → **Akamai + Queue-it
  + in-house Safetix**, with Kasada only partial/regional.
- Walmart moved from "PerimeterX only" → **PerimeterX + Akamai
  stacked**.
- JD.com moved from "Aliyun" → **JD in-house** (direct competitor
  of Alibaba, runs their own stack with `_JdTdudfp` cookies).
- LinkedIn clarified: **Akamai edge + in-house Voyager fraud
  scoring** (not just "in-house").
- Nike clarified: **Akamai on storefront + Kasada on SNKRS draws**
  (two different engines, two different difficulty tiers).

#### Tier 3 — Hopeless without identity/account

Documenting so we don't waste time:

- **WeChat web (weixin.qq.com)** — real-name verification gate; no
  technical bypass.
- **Douyin logged-in content** — account warm-up + behavioral ML;
  sustained session investment.
- **Instagram / Facebook beyond public home** — ML-scored account
  trust, same shape as Douyin.
- **Taobao slider captcha** — acceleration/jitter biometric match;
  the research says even Camoufox and nodriver don't solve this
  without paying 2Captcha/CapMonster.

The honest answer for Tier 3 is "integrate a paid solver
(2Captcha/CapMonster) or use a real account with a warm session."
That's a product decision, not a browser engineering problem.

---

## 5. Prerequisite: strategic pivots that give us free wins

Before spending weeks on the HTML challenge path for each site,
there are scraping shortcuts that the research consistently
recommends:

### Wildberries data (JSON API)

- `https://search.wb.ru/exactmatch/ru/common/v18/search?...&query=...`
- `https://card.wb.ru/cards/v2/detail?nm=...`
- `https://catalog.wb.ru/catalog/<shard>/v2/catalog?...`

These subdomains have a much looser gate than the HTML homepage
and are what every working WB scraper on GitHub uses. Implementing
a `net::HttpClient` wrapper for these is a ~200-line job and gives
us WB data today while the challenge path is still in
investigation. **Worth building even though the "most advanced
stealth browser" goal is about the HTML path.** Data is data.

### Ozon data (JSON API)

- `https://api.ozon.ru/composer-api.bx/page/json/v2?url=<percent-encoded path>`
- `https://www.ozon.ru/api/entrypoint-api.bx/page/json/v2?url=...`

Same pattern. The two hosts have independent antibot state —
sometimes one works when the other doesn't. Community consensus
says the **mobile-app endpoint family** is softest:

```
User-Agent: ozonapp_android/15.x
x-o3-app-name: ozonapp_android
x-o3-app-version: 15.x.x
x-o3-device-type: android
```

### dns-shop.ru data (JSON microdata)

- `https://www.dns-shop.ru/product/microdata/<uuid>/` — plain
  unauthenticated JSON, no challenge, no CSRF. Per the research,
  this is the canonical target used by every working parser.

### Yandex data (Yandex.XML partner API)

- `https://yandex.ru/search/xml?...&user=...&key=...` — paid,
  captcha-free. Not automation; it's the "legal and easy" path.

**Recommendation**: create a `crates/scraper_api/` or add a
`docs/DATA_APIS.md` that documents these for use by consumers of
browser_oxide who don't actually need the HTML/JS rendering — they
just need the data. This is a different product from "pass every
antibot system" but users of scraper_oxide (the sister project) will
probably use both.

---

## 6. What was shipped this week (2026-04-06 → 2026-04-10)

All WB-related and has direct spillover to other sites:

- `op_fetch` takes headers — JS `fetch()` forwards `init.headers`.
- `HttpClient::post_with_headers` / `get_with_headers`.
- `HttpClient::post` stores Set-Cookie (was dropped on the floor).
- Multi-value Set-Cookie via `Response.set_cookies: Vec<String>`
  (HashMap was collapsing them).
- `document.cookie` unified with `net::cookies` via `op_cookie_get`
  / `op_cookie_set` + `__syncCookiesFromNet` helper.
- Inline script execution in `Node.prototype.appendChild`.
- `document.location` aliased to `window.location`.
- `window.top`, `window.parent`, `window.frames`, `window.opener`.
- `location.ancestorOrigins` (empty list).
- `location.reload` as a no-op that doesn't throw.
- JS errors in challenge path are non-fatal to navigation.
- `ConnectionPool::evict` + stale-H2 auto-retry on GOAWAY.
- `navigate_with_challenges` reload-shape headers on retry
  (Referer + `sec-fetch-site: same-origin`).
- **High-entropy Client Hints** in `chrome_headers` — 6 new header
  fields: `sec-ch-ua-arch`, `sec-ch-ua-bitness`, `sec-ch-ua-full
  -version-list`, `sec-ch-ua-model`, `sec-ch-ua-platform-version`,
  `sec-ch-ua-wow64`.
- **`navigator.userAgentData.getHighEntropyValues()`** returning a
  real Promise with spec-compliant subset semantics, values driven
  by the active `StealthProfile`, matching the new HTTP headers.
- **Brotli decoder empty-body tolerance** — `ya.ru` was returning
  `Content-Encoding: br` on an empty 302 body; we now treat empty
  and looks-like-text bodies as passthrough.
- `stealth_ext` exposes `os_version` and `browser_name` fields to
  JS bootstraps for the userAgentData wiring.
- Tests added: 5 unit tests for `chrome_headers` Client Hints,
  7 `chrome_compat` tests for `userAgentData.getHighEntropyValues`.

Regression state: **333/333 unit tests, 216/216 chrome_compat.**
Zero known regressions from this week's work.

Still failing: **wildberries.ru** (WBAAS page gate rejects our
retry GET with a fingerprint-hash-related error — see
`docs/WILDBERRIES.md §5 Plan execution results`).

---

## 7. Decision options for the next session

Before we commit to any path, pick one scope:

### Option A — Build the matrix infrastructure first

Write `docs/ANTIBOT_MATRIX.md` skeleton, add the new
`rigorous_probe()` helper, add the extended engine-detection module,
add the JSON output runner. Run it once to get an honest baseline
across all 50+ existing sites. **Estimate: 2-3 hours of focused
work.** No site-specific fixes — just truth-in-numbers.

Pros: every claim we make after this is grounded. No more "71/71"
lies. Regression detection becomes trivial.
Cons: zero new sites passing as a result. Pure infrastructure.

### Option B — Start with one full regional lane end-to-end

Pick Russia (wildberries, ozon, dns-shop, avito, ya.ru, vk, cian,
lamoda, tinkoff). Classify each honestly at L1-L3 with real content
validators. Apply fixes where they're one-session scope (ya.ru
brotli is already done; others may need small fixes).
**Estimate: a full focused day for Russia.**

Pros: one region fully mapped, demonstrable product improvement.
Cons: infrastructure is still ad-hoc; next region starts from
scratch with the same test-harness weakness.

### Option C — Cherry-pick the free wins first

Re-test `nowsecure`, `bot.sannysoft`, `creepjs`, `bot.incolumitas`,
`browserleaks`, `pixelscan`, `ya.ru` (with brotli fix),
`airbnb.com` (with redirect-follow). All Tier 0 + Tier 1 sites
where recent fixes should have moved the needle. **Estimate:
30-45 minutes.**

Pros: immediate validation that the week's work didn't regress
anything, and actually improved things. Visible win to show the
user.
Cons: doesn't build the infrastructure; doesn't close any hard
cases; leaves the "71/71" problem in place.

### Option D — Wait for the async research agent, then decide

An async research agent is currently running to validate/falsify
the article claims about 2026 antibot systems. Its output will tell
us which engines listed in the article are current, which cookies /
statuses / detection signals are the real ones to match against,
and where `from-scratch-non-Chromium` has a structural advantage.
**Estimate: ~15 min wait.**

Pros: decisions are data-driven from the agent output. Won't waste
effort chasing a claim that turns out to be stale.
Cons: idle time (though we can do other work while waiting).

**Recommended order: D → C → A → B, one lane at a time.**

- **D first** — let the research agent finish, absorb findings into
  the engine detection pattern list and the tier assignments in §4.
- **C second** — validate the week's work with a 30-minute Tier 0
  run. Gives a morale win and confirms no regressions shipped.
- **A third** — build the matrix infrastructure so every site after
  this has honest tracking.
- **B last** — once infrastructure is in place, each region becomes
  a repeatable pipeline: run matrix, identify failures, iterate,
  commit new baseline.

---

## 8. Dependencies / risks / watch-outs

### Rate limits are real and per-vendor-specific

Pounding WBAAS triggers their edge rate limiter within minutes.
DataDome remembers IP reputation across days. Kasada assigns trust
scores. Any site we start debugging, we "use up" for at least 30-60
minutes between attempts. This forces the work to be deliberate
and single-cycle — no retry loops, no "let me just try one more
thing."

**Mitigation**: every test function should print a `# Next safe
retry after:` hint based on vendor. The runner refuses to re-run a
recently-failed site within its cool-down window.

### Fingerprint gaps will be the common blocker

GAPS.md §P0 items 1–7 (prototype integrity, error stack, audio
fingerprint params, `performance.now()` precision, rAF timing,
canvas fonts, WebGL extensions) **plus the new §P-SOTA items 26–32**
(WebGL execution, audio realtime, WebAuthn/FedCM/SAB, behavioral
entropy, H2 golden test) are likely the same gaps that kill us on
Kasada, DataDome, and WBAAS's page gate. Fixing them once probably
unblocks a whole tier of sites at once. That's the case for building
matrix infrastructure BEFORE chasing individual sites — we want to
measure the impact of each gap fix across the whole portfolio. See
`docs/SOTA_ROADMAP_2026.md` for the sequenced 3-phase plan.

### Behavioral ML is a separate subsystem

DataDome trust scoring, LinkedIn / Instagram in-house scoring,
Yandex Antirobot, Variti timing analysis — all of these need a
**humanization layer** (mouse jitter, typing cadence Gaussian,
scroll momentum physics, realistic per-page dwell times). We have
none of this today. It's a multi-week project in its own right and
blocked on the Tier 2 investigations completing first (no point
humanizing a browser that can't even load the page).

### The dataset-IP problem

Everything we test is from one datacenter IP. The research
consistently says "residential/mobile proxies are mandatory for
Russian/Chinese e-commerce." Even if our browser is perfect, a
datacenter IP can get us auto-blocked before the browser fingerprint
is even evaluated. This is an ops concern, not an engineering one,
but it needs to be in the plan: we'll need a way to **verify our
browser is perfect independently of the IP**, probably by running a
local mock antibot server against ourselves or by tunneling through
a known-clean residential IP for specific verification runs.

### StealthProfile schema growth (from §4.5)

The antidetect-equivalent verification work in §4.5 requires
several new `StealthProfile` fields:

- `fonts: Vec<String>` — per-OS font list
- `battery: BatteryState` — charging, level, chargingTime, dischargingTime
- `gpu_extensions: Vec<String>` — WebGL extension list matching `webgl_renderer`
- `gpu_params: HashMap<GLenum, GLValue>` — per-GPU `getParameter` values
- `shader_precision: [ShaderPrecisionFormat; 12]` — the 12 v/f × l/m/h × float/int entries
- `local_ip_seed: u64` — for RTCPeerConnection fake-candidate generation
- `screen_dpr: f64` — already present as `device_pixel_ratio`, verify consistency with screen_width × dpr
- Consider making `webgl_vendor` / `webgl_renderer` derived from a new `gpu_id: GpuProfile` enum
  variant so we can't have them drift out of sync

**Breaking-change risk**: every `StealthProfile` constructor and
every preset in `crates/stealth/src/presets.rs` has to be updated.
Tests relying on profile construction will fail until the new
fields are populated. Budget a half-day for the schema migration
alone, before the actual antidetect features start.

**Mitigation**: add all new fields with `#[serde(default)]` and a
sensible default, so existing presets compile. Then enrich preset-
by-preset as we expand the GPU catalog.

### WebRTC: stop blocking, start faking

Our current `globalThis.RTCPeerConnection` in `window_bootstrap.js`
is a no-op class that never fires ICE candidates. This is itself
detectable — a real browser always returns candidates. §4.5 flags
this as item 6. The fix is to make the shim emit a plausible
`host` candidate with a local IP derived from a profile seed,
mimicking the STUN flow just long enough for the ICE gathering
phase to complete. Don't actually make any UDP connections; just
return the candidate events.

### The mobile profile class

Everything is desktop. Mobile Chrome has different:
- `sec-ch-ua-mobile: ?1`
- `sec-ch-ua-model: "Pixel 7"` (non-empty)
- `navigator.platform: "Linux armv8l"`
- `navigator.maxTouchPoints: 5`
- `ontouchstart` defined on window
- `screen.width`, `screen.height` matching phone dimensions
- `devicePixelRatio: 2.625` or similar
- No `navigator.hardwareConcurrency > 8`
- different `Accept` header order
- different `sec-fetch-site` behavior in some cases

Mobile is worth building because several of the "easy" APIs
(Ozon mobile, Wildberries mobile) use ` ozonapp_android` / similar
User-Agents and have drastically looser antibot gates. **Mobile
profile class is a Tier 1 prerequisite for the "hit the mobile API"
scraping shortcuts.** Track separately from the browser challenge
work.

---

## 9. How to extend this document

When we actually start on any of the tiers:

1. Create a per-engine doc (`docs/DATADOME.md`, `docs/AKAMAI.md`,
   `docs/KASADA.md`, etc.) the first time we touch that engine.
   Same structure as `docs/WILDBERRIES.md`: status, what the engine
   is, the flow we observed, the bugs we found and fixed, what's
   unsolved.
2. Update `docs/ANTIBOT_MATRIX.md` on every material change so the
   baseline JSON tells the truth.
3. If a fix generalizes (e.g., Client Hints fixed WB + Yandex +
   Ozon simultaneously), note it in `docs/STEALTH.md` as a
   profile-consistency rule.
4. Kill claims in `docs/GAPS.md` that this week's work invalidated
   (done for the "known blocks" list).

This document (`NEXT_STEPS.md`) is the index. When a tier ships,
move its entry from §4's queue to a "Shipped" subsection. When the
roadmap is done, archive this doc.
