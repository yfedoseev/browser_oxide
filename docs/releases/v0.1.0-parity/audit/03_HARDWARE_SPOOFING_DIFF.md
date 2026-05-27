# 03 — Hardware spoofing: BO vs Camoufox v150 vs real Chrome

**Sites in scope:** amazon-ca, amazon-com, amazon-com-au, amazon-fr, amazon-in, amazon-jp, imdb, booking, douyin (9 of 11 Stratum-A).
**Vendor probes reading this:** AWS WAF `challenge.js` (primary), DataDome, CreepJS, BotD.
**Status:** 🔵 in progress — value-level diff complete; cross-API consistency in flight (Explore agent dispatched 2026-05-27).

## TL;DR — the 5 plausible deltas

1. **BO ships ONE preset; v150 ships 67 macOS variants.** AWS WAF IP-clustering may flag repeated identical fingerprints. → File `R-PROFILE-SAMPLER` as follow-up.
2. **BO's WebGL renderer is hyper-specific** (`ANGLE (Apple, ANGLE Metal Renderer: Apple M3, Unspecified Version)`); v150 is anonymized (`Apple M1, or similar`). Real Chrome IS hyper-specific, so this is *correct for Chrome* — but if AWS WAF tracks per-renderer-string accept-rates, the M3 string may itself be on a stricter list. → Investigate in `06_WEBGL_DIFF.md`.
3. **v150 stays as Firefox** in identity (UA: `Firefox/150.0`, no Sec-CH-UA, no `navigator.userAgentData`). BO claims Chrome 148. v150 thus has fewer surfaces to leak cross-API inconsistency. BO's claimed-Chrome identity means *every* Chrome-specific surface must be consistent. → `12_CLIENT_HINTS_DIFF.md`.
4. **v146 fix #443** — Camoufox specifically found that v135 patched only WorkerNavigator and not main-window Navigator for platform/hardwareConcurrency/timezone. **Does BO have the same bug, in reverse?** → Cross-context consistency check dispatched (see status note above).
5. **v150 disabled Canvas Noise** (commit `e4528a2`, 2026-04-14) — anti-bots cluster on noise distributions. **Does BO inject canvas noise that's clusterable?** → `08_CANVAS2D_DIFF.md`.

## Per-field comparison

Source: BO `crates/stealth/profiles/chrome_148_macos.yaml` HEAD. Camoufox: `pythonlib/camoufox/fingerprint-presets-v150.json::presets.macos[0..66]`.

### Navigator

| Field | BO (current) | v150 distribution | Notes |
|-------|--------------|-------------------|-------|
| `userAgent` | `Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36` | `Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:{149,150,151,152}.0) Gecko/20100101 Firefox/{...}.0` | Different browsers — BO mimics Chrome. v150 IS Firefox. |
| `platform` | `MacIntel` | `MacIntel` | ✅ MATCH |
| `vendor` | `Google Inc.` | (Firefox returns `""`) | BO correct for Chrome. Don't change. |
| `productSub` | `20030107` | `20100101` (Firefox) | BO correct for Chrome. |
| `oscpu` | (not exposed — Chrome doesn't have it) | (not exposed in macos presets — Firefox on macOS returns `Intel Mac OS X 10.15` if `oscpu` is on) | BO correct for Chrome. |
| `hardwareConcurrency` | `8` | `{2, 4, 6, 8, 10, 12, 16, 18}` (8 is 1 of 8 values; not most common) | ⚠️ Single value. v150 diversity covers more clusters. Consider profile sampler. |
| `deviceMemory` | `8` | NOT SPOOFED (Firefox doesn't expose `navigator.deviceMemory` — that's a Chromium-only API) | BO must keep — Chrome 148 returns it. Verify value: 8 GB is plausible (8/16/32 most common). |
| `maxTouchPoints` | `0` | `0` (uniform) | ✅ MATCH |
| `language` | `en-US` | varies per preset | ✅ COVERAGE |
| `cookieEnabled` | (verify) | `true` | (verify BO returns `true`) |
| `webdriver` | `false` (gated) | `false` | ✅ |
| `userAgentData.brands` | (verify per profile) | NOT EXIST (Firefox lacks `navigator.userAgentData`) | Chrome-specific. BO must match `Sec-CH-UA` HTTP header exactly. See 12_*. |

### Screen

| Field | BO | v150 distribution | Real Chrome 148 / 13.6" MBP M3 |
|-------|----|-------------------|-------------------------------|
| `width` | `1512` | 30 distinct (`736`-`5120`) | `1512` (matches 13.6" MBP) ✅ |
| `height` | `982` | 31 distinct (`414`-`2560`) | `982` (matches 13.6" MBP) ✅ |
| `availWidth` | `1512` | typically == width | `1512` ✅ |
| `availHeight` | `949` | width minus menubar (25-100px) | `949` (982 - 33 menubar = 949) ✅ |
| `availTop` | `33` | varies | `33` (menubar height) ✅ |
| `availLeft` | (0, hard-coded; see 01_BO_BASELINE.md) | (verify) | `0` ✅ |
| `colorDepth` | `30` | `{24, 30}` | `30` (HDR-capable MBP M3) ✅ |
| `pixelDepth` | (verify; usually = colorDepth) | (verify) | `30` |
| `orientation.type` | `landscape-primary` (hard-coded — see 01) | varies | `landscape-primary` ✅ |
| `orientation.angle` | `0` (hard-coded) | varies | `0` ✅ |

### Window

| Field | BO | v150 distribution | Real Chrome 148 / 13.6" MBP M3 |
|-------|----|-------------------|-------------------------------|
| `devicePixelRatio` | `2.0` | `{1, 2, 2.4, 2.609}` | `2.0` ✅ |
| `innerWidth` | `1512` | varies | depends on browser chrome (full viewport) ⚠️ usually < width |
| `innerHeight` | `871` | varies | `871` = screen.height - 111 (chrome bar) for full-screen Chrome ✅ |
| `outerWidth` | `1512` | varies | `1512` ✅ |
| `outerHeight` | `982` | varies | `982` (no minimize/restore math, just screen.height) ✅ |
| `screenX` | (verify; usually 0 for maximized) | varies | `0` if maximized |
| `screenY` | (verify; usually 25-33 for menubar offset) | varies | `25` typically |

### WebGL

| Field | BO | v150 distribution | Real Chrome 148 / Apple M3 |
|-------|----|-------------------|---------------------------|
| `VENDOR` (unmasked off) | `WebKit` (V8 default — verify in canvas_bootstrap.js) | typically `Mozilla` for Firefox | `WebKit` for Chrome ✅ |
| `RENDERER` (unmasked off) | `WebKit WebGL` (verify) | typically `Mozilla` for Firefox | `WebKit WebGL` for Chrome ✅ |
| `UNMASKED_VENDOR_WEBGL` | `Google Inc. (Apple)` | `Apple` (66%), `Intel Inc.` (13%), `ATI Technologies Inc.` (13%) | `Google Inc. (Apple)` ✅ for ANGLE-on-Mac |
| `UNMASKED_RENDERER_WEBGL` | `ANGLE (Apple, ANGLE Metal Renderer: Apple M3, Unspecified Version)` | `Apple M1, or similar` (66%), 5 other patterns | Real Chrome on M3: `ANGLE (Apple, ANGLE Metal Renderer: Apple M3, Unspecified Version)` ✅ |
| `getParameter(MAX_TEXTURE_SIZE)` | from `GpuProfile` struct (canvas_bootstrap.js:419-421) | from `webGl:parameters[3379]` config | typically `16384` on Apple Silicon |
| `getParameter(MAX_VIEWPORT_DIMS)` | from GpuProfile | from config | `[16384, 16384]` typical |
| Supported extensions | from GpuProfile (~30 extensions on M3) | per-preset list | Chrome on M3: a specific 33-extension list |
| `getContextAttributes()` | (verify shape) | per-config `webGl:contextAttributes.premultipliedAlpha` etc. | Chrome default: `{alpha:true, antialias:true, depth:true, ...}` |
| `getShaderPrecisionFormat()` | (verify return shape) | per-config table | Chrome reference values |

### AudioContext

| Field | BO | v150 distribution | Real Chrome 148 |
|-------|----|-------------------|------------------|
| `sampleRate` | **RANDOMIZED per-session** (80% 44100, 20% 48000) — `canvas_bootstrap.js:767` | per-profile `AudioContext:sampleRate` | `44100` or `48000` depending on output device — typically deterministic per device |
| `maxChannelCount` | (verify) | per-profile `AudioContext:maxChannelCount` | `2` (default macOS speakers) |
| `outputLatency` | (verify; Firefox-specific?) | per-profile `AudioContext:outputLatency` | Chrome: in seconds, e.g. `0.041` |
| Akamai T1.3 DynamicsCompressor signature | seeded via `audio_seed` | seeded via `audio:seed` | a specific f32 sequence |

**⚠️ ANOMALY:** BO randomizes sampleRate per-session, not per-profile. Camoufox v150 makes it a per-profile config key. If AWS WAF challenge.js fingerprints AudioContext.sampleRate and clusters on it, BO's instability is a tell. **Fix: pin to profile.**

### Cross-API correlations to check

(Marked ❓ — Explore agent dispatched 2026-05-27 to enumerate. See `13_CROSS_CORRELATION.md` when ready.)

- ❓ Does `Sec-CH-UA-Platform` HTTP header match `navigator.userAgentData.platform` JS value?
- ❓ Does `Sec-CH-UA-Mobile` HTTP header match `navigator.userAgentData.mobile`?
- ❓ Does the UA string brand+version match `navigator.userAgentData.brands`?
- ❓ Does main-window `navigator.platform` == worker `self.navigator.platform`?
- ❓ Does main-window `navigator.hardwareConcurrency` == worker `self.navigator.hardwareConcurrency`?
- ❓ Does `screen.width × devicePixelRatio` give a physically-real macOS resolution? (1512 × 2 = 3024 → MBP 14" or 13.6" — valid)
- ❓ Does `outerHeight - innerHeight` give a realistic chrome-bar size? (982 - 871 = 111 px → matches Chrome on macOS with bookmarks bar visible)

## Implications + work to ship

| § | Item | Files | Effort | Site impact |
|---|------|-------|--------|--------------|
| 03.A | **Pin AudioContext.sampleRate to profile, not random** (the ANOMALY above) | `crates/js_runtime/src/js/canvas_bootstrap.js:767` | 30 min | Maybe AWS WAF |
| 03.B | **Cross-API consistency audit** (Sec-CH-UA HTTP ↔ userAgentData JS ↔ UA string) | `crates/net/`, `crates/js_runtime/src/js/window_bootstrap.js`, `crates/stealth/src/lib.rs` | 1-2 days | High — AWS WAF / DataDome |
| 03.C | **Main-window/worker context consistency** (the v146 #443 inverse check) | `window_bootstrap.js` vs `worker_bootstrap.js` | 1 day | Medium |
| 03.D | **Verify GpuProfile values for chrome_148_macos** match real-Chrome captures (MAX_TEXTURE_SIZE, MAX_VIEWPORT_DIMS, supported extensions list) | `crates/canvas/`, `crates/js_runtime/src/js/canvas_bootstrap.js:419-441` | 1 day | High — AWS WAF |
| 03.E | **Profile sampler** — pick 1 of N pre-validated profiles per Page | `crates/stealth/src/lib.rs`, `crates/browser/src/page.rs` | 1 week | Medium — anti-clustering |
| 03.F | **Audit canvas noise** — confirm we don't inject detectable noise | `crates/canvas/`, `crates/js_runtime/src/js/canvas_bootstrap.js` | 1 day | Medium |

These 6 items feed the `15_FIX_PRIORITY_RANKED.md` priority list.

## Status

🔵 in progress. Awaiting cross-API Explore agent results to populate the ❓ rows.
