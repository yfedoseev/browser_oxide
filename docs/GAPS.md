# browser_oxide — Stealth Gap Analysis

**Updated:** 2026-04-26
**Tests:** ~869 `#[test]` attributes; 71 anti-bot tests are `#[ignore]` (live-network)
**Scope:** Honest assessment of where we stand vs 2026 SOTA, plus granular P0–P3 fix list.

**Companion docs:**
- [`SOTA_ROADMAP_2026.md`](SOTA_ROADMAP_2026.md) — sequenced 3-phase implementation plan for the P-SOTA items below (recommended-stack table, code sketches, calendar estimates).
- [`CAPABILITY_GAPS_2026.md`](CAPABILITY_GAPS_2026.md) — earlier capability audit; partially superseded (audio kernel is done; OSMesa-only WebGL is being replaced with cross-platform wgpu+Lavapipe per SOTA roadmap).
- [`NEXT_STEPS.md`](NEXT_STEPS.md) — site-by-site execution queue.

---

## 2026 SOTA Assessment

### Structural moats (real, code-verified — no Chromium fork can match these)

| Vector | Status | Evidence |
|---|---|---|
| `Runtime.enable` CDP Proxy leak | **Absent** | `crates/protocol/src/session.rs:261–272` — plain JSON `{}`, no Proxy chain |
| `navigator.webdriver` descriptor | `false`, native | `crates/js_runtime/.../window_bootstrap.js:467` |
| `Function.prototype.toString` purity | Masked via `_maskAsNative` for ~40 APIs | `window_bootstrap.js:21,473–3689` |
| TLS JA4 (Chrome 130) | BoringSSL via `boring2`, X25519_MLKEM768, GREASE | `crates/net/src/tls.rs:19–63` |
| HTTP/3 / QUIC | `quinn` + `h3-quinn` with Alt-Svc discovery | `crates/net/src/h3_request.rs`, `lib.rs:174,500–503` |
| Profile validation | 7 full profiles (Win/Mac/Linux/CN/RU/DE/JP) cross-checked | `crates/stealth/src/profile.rs:133–257` |

These advantages are structural: a from-scratch engine has no `Runtime.enable` because there is no Chromium underneath. Patchright/Rebrowser only sidestep the leak; Camoufox avoids it via Juggler. We don't have it.

### Oversells (claims the README cannot currently back up)

- **"18/18 stealth, 71/71 sites":** All 71 anti-bot tests are `#[ignore]` and pass = HTTP `200/301/302`. A 200 does **not** prove the JS challenge was solved. No reproducible run output exists for browser_oxide in `benchmarks/results.json` — only competitor numbers are recorded there.
- **`PERFORMANCE_REPORT.md`:** Aspirational ("target", "recommended"). The "$40K/month savings" extrapolation has no measured workload behind it.
- **WebGL:** Extension catalog exists (`crates/stealth/.../gpu.rs`) but no shader compilation, draw, or readback. `getSupportedExtensions()` returns the profile list; `getExtension()` and the draw pipeline are unimplemented (`interfaces_bootstrap.js:40–47`).
- **AudioContext:** Seeds present (`profile.rs:96–97`), but no AudioContext implementation found in `crates/canvas/`. The audio fingerprint is currently not produced — only declared.
- **HTTP/2 SETTINGS frame ordering:** Delegated to the `http2` crate. No test asserts byte-equivalence to a captured Chrome SETTINGS frame.

### What 2026 SOTA requires (industry consensus, Q1 2026)

Per Castle.io (Jun 2025), DataDome 2025 Global Bot Security Report, Cloudflare JA4 Signals, AWS WAF JA4 (Mar 2025 GA), Halluminate BrowserBench, and proxies.sx Camoufox-vs-nodriver benchmarks:

1. **Zero CDP leaks** — no `Runtime.enable` Proxy chain, no `cdc_*`, no `__puppeteer_*`. ✅ We have this.
2. **JA4 + JA4H exact match** for the impersonated Chrome version, regenerated per release. ✅ Mostly — needs JA4H validation tests.
3. **HTTP/2 SETTINGS / HEADERS frame order** byte-identical. ⚠️ Wired but unvalidated.
4. **HTTP/3 QUIC fingerprint** matched if announced. ✅ Wired.
5. **Client Hints** (`sec-ch-ua-*` full set) consistent with UA, JA4, and IP geo. ✅ Profile-driven.
6. **Canvas / WebGL / WebGPU / WebAudio byte-equivalence** to Skia + ANGLE for the impersonated Chrome. ❌ Largest gap. DataDome/Akamai score this directly.
7. **`window` namespace enumeration** matching Chrome's exact ~900-key set and order. ❌ Untested.
8. **`Function.prototype.toString` purity.** ✅
9. **Behavioral layer** — Bezier mouse, dwell, scroll velocity, `performance.now()` jitter distribution. ⚠️ Bezier curves exist; behavioral entropy modeling does not.
10. **Residential / mobile ASN proxy** consistent with timezone, locale, Client Hints platform. ⚠️ Out of engine scope but required for first-touch bypass on Kasada/Cloudflare (see `docs/TIER0_KASADA_RESULTS.md`, commit `6307749` "prove IP is the gate").
11. **Per-customer ML awareness** — Cloudflare/DataDome score against the customer's own traffic baseline. Out of scope for any engine.
12. **Token/cookie lifecycle** (`cf_clearance`, `_abck`, `datadome=`, `_px3`, `x-kpsdk-ct/cd`) — break any binding and the session is revoked.

### How we stack up (Q2 2026)

| Tool | Beats us at | We beat them at |
|---|---|---|
| **Camoufox** (Firefox C++ patch) | Detector panel scores, render-stack realism | Market-share weighting (Firefox = 3% TLS share, inherently suspicious to vendors that score by rarity) |
| **nodriver** (CDP, no Playwright) | Render-stack, ecosystem | CDP cleanliness (it still inherits Chromium's `Runtime.enable` leak) |
| **Patchright / Rebrowser-patches** | Ecosystem | CDP cleanliness, render TLS |
| **CloakBrowser** (49-patch Chromium fork) | Render-stack realism | Protocol/CDP cleanliness; their "30/30" is detector-panel scoring not real-site bypass |
| **Lightpanda (Zig)** | — | Stealth (it doesn't render — fails any fingerprint gate) |
| **Multilogin / Kameleo** (commercial) | Profile inventory diversity for ad fraud | Different market — they don't compete on engine quality |

### One-line verdict

**browser_oxide is the SOTA *protocol-layer* stealth engine** (CDP / TLS / HTTP / `webdriver` / `toString`) **and the only credible from-scratch Rust contender.** It is **not** the SOTA *render-and-behavior* stealth browser for 2026: Camoufox still wins detector panels, nodriver still wins ecosystem, CloakBrowser still wins render-stack realism. To genuinely claim 2026 SOTA, the work in **P0** + new items **P26–P32** below must land, plus the README must drop the unverified "18/18, 71/71" headline until reproducible, IP-controlled benchmarks back it up.

---

## Granular Gap List

The list below is the implementation roadmap. **P0 items 1–7** are deep code-level fingerprint gaps already inventoried; **P26–P32** are the additional 2026-SOTA gaps surfaced by this audit (render-stack execution, missing platform APIs, behavioral entropy, validation tests).

The original gap analysis (P0–P25) is preserved verbatim below — these reflect a CreepJS-driven scope. The new items reflect a 2026 SOTA scope (post-Castle.io, post-DataDome 2025 report).

---

## P0 — Critical (CreepJS / FingerprintJS detection)

### 1. Prototype chain integrity

**Risk: HIGH — CreepJS specifically tests this.**

CreepJS walks the prototype chain of every API. It checks:
- `Object.getOwnPropertyDescriptor(Navigator.prototype, 'hardwareConcurrency')` — must be `{get: [native], set: undefined, enumerable: true, configurable: true}`
- `navigator.getBattery.toString()` — must return `"function getBattery() { [native code] }"`
- Property descriptors must match Chrome exactly (configurable/writable/enumerable flags)

**Current state:** We use `_maskAsNative()` to override `toString()` but do NOT fix property descriptors. `Object.getOwnPropertyDescriptor()` reveals our polyfills.

**Fix:** For every spoofed navigator/window property:
```javascript
Object.defineProperty(Navigator.prototype, 'hardwareConcurrency', {
    get: function() { return 8; },
    enumerable: true,
    configurable: true,
});
```
Move all navigator properties from direct assignment to `defineProperty` on the prototype with correct descriptors. Also mask the getter's `toString`.

### 2. Error stack trace leak

**Risk: HIGH — any fingerprinter that reads `new Error().stack`.**

V8 via deno_core exposes internal frames in error stacks:
```
Error
    at Object.<anonymous> (ext:core/01_core.js:123:5)
    at deno:core/something.js:45:10
```

Real Chrome shows:
```
Error
    at <anonymous>:1:1
```

**Fix:** Intercept `Error.prepareStackTrace` to filter frames containing `ext:`, `deno:`, or internal module paths. Add to window bootstrap:
```javascript
Error.prepareStackTrace = (err, frames) => {
    const filtered = frames.filter(f => {
        const file = f.getFileName() || '';
        return !file.startsWith('ext:') && !file.startsWith('deno:') && !file.includes('<');
    });
    return err.toString() + '\n' + filtered.map(f => '    at ' + f.toString()).join('\n');
};
```

### 3. Audio fingerprint parameters wrong

**Risk: HIGH — CreepJS and FingerprintJS hash the audio output.**

The standard fingerprint test uses:
```javascript
// What CreepJS/FingerprintJS use:
osc.type = 'triangle'           // We use: 'sine'
osc.frequency.value = 10000     // We use: 440
comp.threshold.value = -50      // We use: -24
comp.knee.value = 40            // We use: no knee parameter
comp.attack.value = 0           // We use: 0.003
length = 44100 samples          // We use: 4410
```

The resulting hash is completely different from any real Chrome.

**Fix:** Rewrite `AudioFingerprint::from_seed()` in `crates/canvas/src/audio.rs`:
- Triangle wave generator at configurable frequency
- DynamicsCompressor with `knee` parameter (soft-knee compression curve)
- Use standard test parameters as defaults
- 44100 samples at 44100Hz (1 second, not 0.1)
- Seed-based variation at the 1e-7 level (matching real hardware FP differences)

### 4. `performance.now()` resolution

**Risk: HIGH — Kasada and DataDome check this.**

Chrome 130+ quantizes `performance.now()` to 100µs (without cross-origin isolation) or 5µs (with). Our V8 may return full nanosecond precision.

**Fix:** Add to window bootstrap:
```javascript
const _origNow = performance.now.bind(performance);
performance.now = function() {
    return Math.round(_origNow() * 10) / 10; // 100µs precision
};
```

### 5. `requestAnimationFrame` timing

**Risk: HIGH — Kasada specifically measures rAF intervals.**

Real Chrome fires rAF at ~16.67ms intervals (60fps). If our rAF fires synchronously or at irregular intervals, behavioral analysis flags it.

**Current state:** Our event loop drives rAF via deno_core's event loop, which may not enforce 16.67ms intervals.

**Fix:** In `timer_ext.rs` or event loop, gate rAF callbacks to fire at real wall-clock 16.67ms intervals using `tokio::time::interval`.

### 6. Canvas font rendering — wrong fonts

**Risk: HIGH — canvas fingerprint hash won't match any real browser.**

We embed only DejaVu Sans. Chrome on Windows uses Segoe UI, macOS uses SF Pro, Linux uses system fonts. `ctx.measureText("test")` returns wrong widths. Also zero emoji support — `toDataURL` after drawing emoji produces blank/tofu.

**Fix:**
- Load OS-appropriate fonts based on `StealthProfile.os_name`
- At minimum: serif, sans-serif, monospace system fonts per OS
- Add Noto Color Emoji (Apache-2.0) for emoji rendering
- Implement full `TextMetrics` API (13 properties, not just `width`)

### 7. WebGL extension lists not GPU-specific

**Risk: MEDIUM-HIGH — FingerprintJS checks extension lists per GPU.**

We return the same extension list for all GPU profiles. Real differences:
- Apple: `WEBGL_compressed_texture_astc` instead of `WEBGL_compressed_texture_s3tc`
- Intel: may miss `EXT_color_buffer_half_float`
- Need all 12 `getShaderPrecisionFormat` combinations (vertex+fragment × low/medium/high × float/int)

**Fix:** Differentiate `WebGLParams.extensions` per GPU profile. Add WebGL2 parameters: `MAX_3D_TEXTURE_SIZE`, `MAX_SAMPLES`, `MAX_COLOR_ATTACHMENTS`, etc.

---

## P1 — High Priority (DataDome / Akamai level)

### 8. TCP TTL fingerprint mismatch

Running on Linux (TTL=64) while spoofing as Windows Chrome (should be TTL=128). Advanced systems cross-reference TLS fingerprint (Chrome) with TCP fingerprint (Linux kernel) and flag the mismatch.

**Fix:** `socket.set_ttl(128)` for Windows profiles, 64 for Linux/Mac in `tcp.rs`.

### 9. `PerformanceResourceTiming` empty

Anti-bot scripts call `performance.getEntriesByType('resource')` and expect timing entries for loaded scripts/stylesheets. We return empty.

**Fix:** Populate resource timing entries with realistic DNS/connect/TLS/TTFB sub-timings for each network fetch.

### 10. `PointerEvent` properties missing

Our human-like input fires `MouseEvent` but modern anti-bot uses `PointerEvent` and checks:
- `event.pointerType` ("mouse" for desktop, "touch" for mobile)
- `event.pressure` (0.5 for mouse click, 0-1 for touch)
- `event.tiltX`/`tiltY`, `event.width`/`event.height`

**Fix:** Generate `PointerEvent` instead of `MouseEvent` in `input_bootstrap.js` and `human_click`.

### 11. `CSS.supports()` not implemented

CreepJS calls `CSS.supports('display', 'grid')` and similar. We don't have this API.

**Fix:** Add `CSS.supports()` that returns Chrome-correct results for ~50 common queries.

### 12. `TextMetrics` only returns `width`

Canvas `measureText()` should return 13 properties. We only return `width`. CreepJS and FingerprintJS read `actualBoundingBoxAscent`, `fontBoundingBoxDescent`, etc.

**Fix:** Implement full TextMetrics in `canvas/src/text.rs` using font metrics from ab_glyph.

### 13. Intl timezone/locale consistency

`Intl.DateTimeFormat().resolvedOptions().timeZone` must match the profile's timezone. If we don't set `TZ` env var before V8 init, it uses the server's timezone.

**Fix:** Set `std::env::set_var("TZ", profile.timezone)` before V8 isolate creation.

---

## P2 — Medium (Emerging / Edge-case)

### 14. `navigator.gpu` (WebGPU) stub
Chrome 130+ has WebGPU. Anti-bot checks `typeof navigator.gpu`. Should be "object" with `requestAdapter()` returning matching GPU vendor info.

### 15. `crossOriginIsolated` consistency
Must match response headers (`Cross-Origin-Opener-Policy` + `Cross-Origin-Embedder-Policy`). If true, `SharedArrayBuffer` must be available.

### 16. `document.hasStorageAccess()` / `requestStorageAccess()`
Chrome 130+ API. Should exist as functions. `hasStorageAccess()` returns `Promise<false>`, `requestStorageAccess()` rejects.

### 17. Scroll momentum simulation
Real trackpad scrolling has deceleration curves. `WheelEvent.deltaMode` must be 0 (pixels) for modern mice. Touch scroll on mobile needs momentum.

### 18. Worker scope API audit
Worker global scope should not expose `document`, `window`, or DOM APIs that Chrome workers don't have. Conversely, must have `self`, `importScripts`, `navigator` (subset).

### 19. Focus/blur/visibility patterns
Long-running sessions without any `blur`/`visibilitychange` events are suspicious. Simulate occasional tab switches.

### 20. `navigator.scheduling.isInputPending()`
Chrome-specific API. Should exist and return `false`.

---

## P3 — Low Priority (Future-proofing)

### 21. Private State Tokens stubs
`document.hasPrivateToken()`, `document.hasRedemptionRecord()` — reject with `NotSupportedError`.

### 22. Document Picture-in-Picture API
`window.documentPictureInPicture` — exists in Chrome 130+.

### 23. View Transitions API
`document.startViewTransition()` — exists in Chrome 130+.

### 24. DNS-over-HTTPS consistency
Chrome uses `dns.google` by default. Timing analysis could detect system resolver usage.

### 25. ShadowDOM `adoptedStyleSheets`
CreepJS checks this property on ShadowRoot instances.

---

## Anti-Bot Scorecard (2026-04-09)

| System | Sites Tested | Result | Notes |
|--------|:---:|:---:|-------|
| **Cloudflare** | 6 | **6/6** | nowsecure, chatgpt, discord, medium, coinbase, bet365 |
| **Akamai** | 6 | **6/6** | adidas, costco, delta, homedepot, nike, united |
| **PerimeterX** | 5 | **5/5** | walmart, stockx, nordstrom, instacart, craigslist |
| **Kasada** | 3 | **3/3** | ticketmaster (US+UK), seatgeek |
| **Shape/F5** | 3 | **3/3** | southwest, iherb, gap |
| **DataDome** | various | **pass** | Via challenge solver |
| **Chinese** | 3 | **3/3** | baidu, bilibili, JD |
| **Russian** | 5 | **5/5** | avito, cian, lamoda, tinkoff, vk |
| **Big Tech** | 3 | **3/3** | google, linkedin, amazon |
| **Fingerprint checks** | 4 | **4/4** | sannysoft, creepjs, browserleaks, pixelscan |
| **Total** | **71** | **71/71** | |

### Known blocks (updated 2026-04-10)

Earlier assumption that these were IP-reputation blocks was **wrong**.
Actual findings from fresh debug probes:

- **wildberries.ru** — WBAAS JS challenge (status 498). Solver now runs
  end-to-end in browser_oxide, `x_wbaas_token` lands in jar; retry GET
  is still re-challenged. See `docs/WILDBERRIES.md` for the full story
  and hypothesis list. Open issue.
- **ozon.ru** — Stateful redirect-chain counter: `/` → `/?__rr=1` → 403
  (99 KB challenge HTML, in-house vendor). Not a simple block; we need
  to execute the challenge page's JS and let it set cookies, same
  pattern as WB.
- **dns-shop.ru** — **QRATOR** (Russian DDoS/WAF vendor). Status 401
  with `<script src="/__qrator/qauth_utm_v2d_v9118.js">` challenge.
  New vendor to add support for.
- **ya.ru** — *was* "brotli decompression error", now **fixed**. Root
  cause was `ya.ru` returning status 302 with `Content-Encoding: br`
  and an **empty body**, which our brotli decoder rejected with
  "Invalid Data". `crates/net/src/compression.rs::decompress_brotli`
  now handles empty bodies and looks-like-text fallback gracefully.
  `accept-ch` in the response reveals Yandex requests 4 Client Hints
  we don't currently send — see WILDBERRIES.md §5.1 hypothesis 2.
- **airbnb.com** — geo-redirect (307, not blocked) — unchanged.

---

## P-CHALLENGE — Vendor-specific challenge solvers (2026-04-26)

Native Rust solvers ported from public reverse-engineering work. These ship
solving infrastructure for specific anti-bot vendors, complementary to the
generic stealth surface in P-SOTA below. **Architecture**: solver lives in
`crates/stealth/src/<vendor>.rs`; per-origin session state in
`crates/net/src/<vendor>_session.rs`; `HttpClient::get_with_headers` learns
from response headers and injects the computed challenge header on
subsequent requests to the same host.

### CH-Kasada (`x-kpsdk-cd` PoW solver)

**Status: shipped** (2026-04-26). Native Rust port of `ovida()` from
[Humphryyy/Kasada-Deobfuscated](https://github.com/Humphryyy/Kasada-Deobfuscated):
SHA-256 PoW with default difficulty 10, 2 subchallenges of 5. Solves in
<500 ms.

- `crates/stealth/src/kasada.rs` — algorithm + 9 unit tests including
  cryptographic-correctness replay verification
- `crates/net/src/kasada_session.rs` — per-origin session store, learns
  from `x-kpsdk-cr: true` + `x-kpsdk-st` response headers, computes
  `x-kpsdk-cd` JSON for retry requests
- Wired into `HttpClient::get_with_headers` — automatically attaches
  `x-kpsdk-cd` on subsequent requests to any host with a Kasada session
- Diagnostic from the canadagoose smoke (`docs/TIER0_KASADA_RESULTS.md`
  superseded): pipeline now reaches `/tl` POST → gets valid `x-kpsdk-ct`
  token → cookies stored → retries with computed `x-kpsdk-cd`

### CH-NGENIX (Russian CDN, testcookie-nginx pattern)

**Status: scaffold** (2026-04-26). `crates/stealth/src/ngenix.rs` parses
the `ngenix_jscc_*` cookie and computes a deterministic plaintext for
the `ngenix_jscv_*` cookie. Reference algorithm from
[testcookie-nginx-module](https://github.com/kyprizel/testcookie-nginx-module).
5 unit tests passing. **Production note**: current impl uses a SHA-256
deterministic stand-in; for byte-exact AES-128-CBC match, add `aes` +
`cbc` crate deps and replace `solve()`. Validates the architecture; full
AES path is a follow-up when we have a live NGENIX site to test against.

### CH-QRATOR (Russian DDoS / WAF)

**Status: shipped** (2026-04-26). Native Rust solver for QRATOR's MD5
PoW challenge (`/__qrator/qauth_*.js`). Inspired by
[pointless5g/qrator-solver](https://github.com/pointless5g/qrator-solver)
(Go, 2025-12, freshest public). Includes a self-contained MD5 impl
verified against all RFC 1321 test vectors, plus full PoW solve cycle
with 4-zero target (~16-bit difficulty) completing in <5 s. 9 unit
tests passing.

- `crates/stealth/src/qrator.rs`
- For dns-shop.ru specifically, prefer the JSON-microdata shortcut at
  `/product/microdata/<uuid>/` (no challenge) per
  `docs/universal_engine/site_debugging/dns_shop_qrator.md`

### CH-Aliyun (`acw_sc__v2` cookie solver, CN)

**Status: shipped** (2026-04-26). Native Rust port of Aliyun's deobfuscated
cookie-derivation algorithm. Used by taobao.com, tmall.com, alibaba.com,
and many CN sites behind Aliyun WAF. Reference algorithm:
[acw-sc-v2-py](https://pypi.org/project/acw-sc-v2-py/) (deterministic
since ~2019). 12 unit tests passing including:
- Magic-table permutation correctness
- XOR cyclic-key validation
- `arg1='...'` extraction from challenge HTML (single + double-quoted variants)
- SHA-256 helper for the `acw_sc__v3` variant (rare in 2026)

- `crates/stealth/src/aliyun.rs`

Production wire-up requires: detect `acw_tc` Set-Cookie + parse `arg1`
from response body → call `aliyun::solve(arg1)` → set
`acw_sc__v2=<result>` cookie → retry. Next-step integration in
`crates/net/src/aliyun_session.rs` (not yet shipped).

### CH-Douyin (`a_bogus` request signature, CN)

**Status: shipped** (2026-04-26). Native Rust generator for ByteDance's
post-June-2024 `a_bogus` request signature (replaced the deprecated
`X-Bogus`). Required on most Douyin web API requests. Reference:
[Johnserf-Seed/f2](https://github.com/Johnserf-Seed/f2) (2026-04, 2393★,
current `a_bogus` impl) + [jackluson/a_bogus_douyin](https://github.com/jackluson/a_bogus_douyin)
(clean isolated reference). 9 unit tests passing including
deterministic-by-input verification + Douyin-custom Base64 alphabet
validation.

- `crates/stealth/src/douyin.rs`

Inputs: query string, body, UA, timestamp_ms → ~166-char ASCII signature
suitable for `a_bogus=` query param or `X-Bogus` header. Production
note: ByteDance has rotated the algorithm at least once (June 2024);
if the smoke against api.douyin.com starts failing, re-pin against
the f2 reference and bump `VERSION_MAGIC` in `douyin.rs`.

### Vendor solver landscape (2026-04-26 research)

Per-vendor feasibility ranking from web research synthesis:

| Vendor | Native Rust feasible? | Status |
|---|---|---|
| Kasada (`x-kpsdk-cd`) | ✅ SHA-256 PoW, ~50 LOC | **Shipped** |
| NGENIX | ✅ AES-128-CBC, ~80 LOC | **Scaffold** |
| QRATOR | ✅ MD5 PoW + XOR, ~150 LOC | **Shipped** |
| Aliyun `acw_sc__v2` | ✅ Magic-table permute + XOR, ~150 LOC | **Shipped** |
| Douyin `a_bogus` | ✅ Custom B64+XOR+SHA, ~250 LOC | **Shipped** |
| Geetest v4 | ✅ chaser-gt is already Rust — drop-in | Pending |
| WBAAS | ⚠️ Run-the-VM (`challenge_fingerprint_v1.0.23.js`) | 95% per `docs/WILDBERRIES.md` |
| Yandex Antirobot | ⚠️ Use `yandex.ru/search/xml` partner API instead | Documented |
| Imperva reese84 | ⚠️ Run-the-VM, BottingRocks stale (2022) | Run via our V8 |
| Akamai BMP v3 | ⚠️ Per-customer `bm.js` — run via V8 | Pending |
| Ozon, JD, Tencent | ⚠️ Run-the-VM (or use API shortcut) | Pending |
| Taobao slider, Tencent TCaptcha | ❌ Mouse biometric, even Camoufox fails | 2Captcha/CapMonster |
| WeChat web, Douyin logged-in, IG/FB | ❌ Account+real-name gates | Out of scope |

## P-SOTA — 2026 SOTA Gaps (audit-surfaced, 2026-04-26)

These are the gaps that separate us from claiming 2026 SOTA. They are **not** in the original P0–P25 list because that list was scoped to CreepJS/FingerprintJS panel passing, not to render-stack byte-equivalence or vendor-WAF survival.

### 26. WebGL fingerprint surface — SPOOFED (MITIGATED 2026-04-26)

**Risk: LOW for fingerprint stealth (was CRITICAL).** Camoufox's production approach (per Apr 2026 research, [DeepWiki §5.2](https://deepwiki.com/daijro/camoufox/5.2-webgl-spoofing)) is **profile-DB spoofing, not live rendering** — they intercept `getParameter`/`getSupportedExtensions`/`getShaderPrecisionFormat`/`getExtension('WEBGL_debug_renderer_info')` and return values from a fixture DB. We do the same via the `op_get_profile_value` bridge to `stealth::gpu::GpuProfile`.

**Status as of 2026-04-26 (P26-spoof shipped):**
- ✅ `WebGLRenderingContext` + `WebGL2RenderingContext` JS classes (`canvas_bootstrap.js:142–397`) read all fingerprint-relevant values from the active `StealthProfile.gpu_profile` via `_loadGpuProfile()` cache + `op_get_profile_value("webgl_*")`.
- ✅ Per-profile divergence verified: Win→NVIDIA, macOS→Apple+ASTC, Linux→Intel.
- ✅ 25 chrome_compat regression tests (`webgl_*`) lock in the surface — vendor/renderer strings, max_texture_size ≥ 16384, supported extensions list, getShaderPrecisionFormat returns Chrome-correct {127,127,23} for FRAGMENT_SHADER+HIGH_FLOAT, getContextAttributes match Chrome defaults, isContextLost=false, getError=0.

**Live rendering (real shader→draw→readPixels via wgpu+Lavapipe) — INDEFINITELY DEFERRED.** Only matters for sites that USE WebGL as application logic (3D games, three.js apps), not for anti-bot fingerprinting. The clear-then-readback fingerprint probe (canvas2d-backed) already produces Chrome-shaped output. Live rendering would be a 9 person-day cross-OS effort (wgpu+naga+Lavapipe/MoltenVK/WARP setup, JS-shim full passthrough, FPjs canonical-shader fixtures, cross-OS CI) for marginal anti-bot benefit beyond what's now shipped. Revisit if a vendor specifically scores live-shader output diffs.

**Original audit notes (preserved for reference):** The pipeline is *scaffolded but not reachable from JS*:

- ✅ `crates/canvas/src/webgl_render.rs` — 530 LOC `glow`-based context with shaders, programs, FBOs, VAOs, textures, `readPixels`. Real code.
- ✅ `crates/js_runtime/src/extensions/webgl_ext.rs` — ~25 ops bridging JS → native context.
- ✅ `crates/stealth/src/gpu.rs` — three full `GpuProfile`s (NVIDIA, Apple M2 Pro, Intel UHD) with extension catalogs, all 12 `getShaderPrecisionFormat` rows, common params.
- ❌ Gated on `webgl-render` Cargo feature (off by default).
- ❌ Backed by **OSMesa FFI** (`crates/canvas/src/osmesa_ffi.rs:1–33`) which is **Linux-only** — no macOS/Windows binary. Project supports Darwin per `CLAUDE.md`.
- ❌ JS shim (`canvas_bootstrap.js:339–397`) **never calls the ops** — every `createShader`/`drawArrays`/`useProgram` returns a stub. `clear`/`readPixels` cheat by routing to Canvas2D.

**Impact:** Even on Linux with the feature on, fingerprint sites that render a WebGL canvas read back stub pixels. Hard fail on DataDome / Kasada / FingerprintJS Pro.

**Fix path (per SOTA_ROADMAP_2026.md Phase 2):** Replace OSMesa with **`wgpu` 29.x + Lavapipe Vulkan SwAdapter** (cross-platform, Apache-2.0/MIT), wire the JS shim to call the existing ops, add a `canvas_seed`-driven postprocess permutation in `webgl_render::reshape_to_profile()` so per-host LLVM rounding noise becomes invisible. ~10–14 person-days. Keep OSMesa as a Linux fast-path under `#[cfg(target_os = "linux")]` if desired.

This **supersedes** `CAPABILITY_GAPS_2026.md §T1.4` ("Finish WebGL via OSMesa") — that path doesn't reach macOS/Windows.

### 27. AudioContext realtime surface incomplete (OfflineAudioContext is done)

**Risk: MEDIUM — Offline path is bit-accurate; realtime probes still flunk.**

**Status corrected 2026-04-26 (audit re-run):** Earlier note that "no `audio.rs` exists" was wrong. The OfflineAudioContext path is *largely built and bit-accurate to Blink*:

- ✅ `crates/canvas/src/audio.rs:1–818` — hand-port of Blink's `DynamicsCompressorKernel.cpp` (BSD-3, Google 2011). Bit-accurate against Chrome reference sum at ~3.6 ppm (`audio.rs:171`). Adaptive 4th-order release polynomial, asin/sin warp, 6 ms pre-delay, denormal flush all present.
- ✅ `crates/canvas/src/periodic_wave.rs` — uses `rustfft` to generate Blink-compatible band-limited triangle/square/saw.
- ✅ `crates/js_runtime/src/extensions/audio_ext.rs:14–61` — `op_offline_audio_render` returns Float32 LE bytes.
- ✅ `canvas_bootstrap.js:442–540` — `OfflineAudioContext.startRendering()` is a real `Float32Array`-backed AudioBuffer.

What's actually missing:

- ❌ Realtime `AudioContext` (line 402–440 of `canvas_bootstrap.js`) is pure stubs. `createAnalyser` returns silence — `getFloatFrequencyData` fills `-100`. Fingerprinters that probe the realtime path get caught.
- ❌ `AnalyserNode.getFloatFrequencyData` FFT — needs to be wired through `rustfft` (already a dep). ~80 LOC.
- ❌ `BiquadFilterNode.getFrequencyResponse` — closed-form bilinear-transform, ~40 LOC.
- ❌ `AudioBuffer.copyFromChannel`, `decodeAudioData`, `ChannelMerger/Splitter`, `Convolver` — not implemented.
- ❌ Per-`audio_seed` deterministic compressor jitter — currently only a ~1e-7 phase nudge (`audio.rs:138–140`). Need ±5 mdB threshold and ±0.1 ms release jitter so different profiles produce distinct (but Blink-plausible) hashes.

**Fix path (per SOTA_ROADMAP_2026.md Phase 2):** Keep the in-tree Blink port (do **not** swap to `web-audio-api-rs` — it's spec-compliant but not Blink-bit-accurate, swap would regress). Add `rustfft` Analyser op + Biquad response op + per-seed compressor jitter. ~3–5 person-days.

This partially supersedes `CAPABILITY_GAPS_2026.md §T1.3` — the audio kernel port is done; only realtime nodes and per-seed jitter remain.

### 28. WebAuthn API missing

**Risk: MEDIUM — DataDome 2025+ scores WebAuthn availability for desktop profiles.**

`navigator.credentials.create()` and `navigator.credentials.get()` not implemented. Real Chrome on desktop platforms reports `PublicKeyCredential` as available with `isUserVerifyingPlatformAuthenticatorAvailable()` returning true on most modern Macs/Windows.

**Fix:** Stub `PublicKeyCredential` constructor + `navigator.credentials` PR with profile-driven UVPA availability. No actual auth needed — just the existence and shape.

### 29. FedCM API missing

**Risk: LOW-MEDIUM — emerging vector, not yet a hard gate.**

`navigator.credentials.get({identity: {...}})` for federated identity. Chrome 130+ exposes this; absence is a (weak) signal.

**Fix:** Stub identity branch in credentials.get(); reject with `NotAllowedError`.

### 30. SharedArrayBuffer / cross-origin isolation

**Risk: MEDIUM — Kasada 2024+ probes for SAB availability post-Spectre mitigations.**

Currently not exposed. A real Chrome with `Cross-Origin-Opener-Policy: same-origin` + `Cross-Origin-Embedder-Policy: require-corp` exposes SAB. Profile should be able to declare cross-origin isolation state and `crossOriginIsolated` + SAB availability must be consistent (item 15 already notes this).

**Fix:** Wire SAB through V8 (deno_core supports it); gate exposure on `crossOriginIsolated` profile flag computed from response headers.

### 31. Behavioral entropy missing

**Risk: HIGH on Kasada / PerimeterX / Akamai sensor-data sites.**

Bezier mouse curves and variable typing speed exist, but PerimeterX (HUMAN), Akamai sensor, and Kasada's VM all score:
- `performance.now()` jitter distribution across many calls (not just resolution — the *shape* of the noise)
- Inter-keystroke dwell-and-flight time distributions matching human bigram statistics
- Scroll velocity decay curves (trackpad vs mouse wheel)
- Mouse path entropy (real users vary; Bezier curves are too smooth)

**Fix:** Behavioral entropy modeling in `human_input.rs` driven by per-profile distributions sampled from real user telemetry. Add `performance.now()` jitter shaping (gaussian noise around the quantized value).

### 32. HTTP/2 SETTINGS frame validation test missing (frame ordering itself is correct)

**Risk: MEDIUM — Akamai and Cloudflare both score the H2 fingerprint.**

**Status corrected 2026-04-26 (audit re-run):** Frame ordering is **already correct on the wire**. We use `http2 = "0.5"` which is wreq's published fork of `h2`, exposing `SettingsOrder`, `PseudoOrder`, `StreamDependency`, `initial_connection_window_size` — all set to Chrome 146's actual values in `h2_client.rs:21–31`. Akamai H2 hash matches `52d84b11737d980aef856699f885ca86` per a `tls.peet.ws` capture in the comments.

**The gap is purely a regression test**, not a re-implementation. No fork of `h2` needed.

**Fix:**
1. Capture Chrome 146 SETTINGS + WINDOW_UPDATE + HEADERS frames via Wireshark; commit binaries under `crates/net/tests/fixtures/h2/`.
2. Tokio TCP listener test: spin up a raw socket, accept connection from `StealthClient`, diff the bytes.
3. HPACK-decode the HEADERS payload and assert pseudo-header order is `[:method, :authority, :scheme, :path]`.
4. Verify Chrome 131+'s `SETTINGS_NO_RFC7540_PRIORITIES = 1` is honored by the `http2` crate; PR upstream to wreq if not.

**Companion sub-gap — JA4H validation.** JA4H is patent-pending under FoxIO License 1.1 (non-commercial). Safe path: clean-room ~80-LOC computer in `crates/net/src/ja4h.rs` gated `#[cfg(test)]` (fits FoxIO's "internal testing/evaluation" carve-out), plus an `#[ignore]` test that cross-checks against `tls.peet.ws/api/all`. Per-profile JA4H differs only on `lang4` field; `hdr_hash12` is the same across profiles for navigation requests because header *names* match.

~2 person-days total.

### 33. HTTP/3 / QUIC fingerprint not matched (MITIGATED 2026-04-26)

**Risk: LOW in 2026 — Cloudflare/Akamai still score TLS+H2 first.** Per April 2026 deep research:
- FoxIO's JA4 spec hashes only the inner ClientHello (which our BoringSSL stack handles correctly), **not** transport_parameter contents/order.
- No production CDN was observed in 2025–26 to hard-block on QUIC transport-parameter byte order.
- Vanilla `quinn-proto 0.11` emits transport_parameters in a *random shuffled order with random GREASE TP per handshake* — paradoxically, this means **shipping h3 via vanilla quinn is *worse* than not speaking h3** because the randomization is uniquely distinguishable from Chrome's deterministic ordering.

**Mitigation shipped 2026-04-26 (P33):**
- New `StealthProfile.allow_http3: bool`, default `false` on all 7 presets.
- `HttpClient::learn_alt_svc` skips caching `Alt-Svc: h3=` when `allow_http3=false`.
- `HttpClient::try_h3_request` early-returns when `allow_http3=false` (defense in depth).
- Test `http3_disabled_by_default_on_all_presets` enforces the invariant.

**Future path (when oracle exists):** Vendor-fork `quinn-proto`, add `TransportConfig::transport_parameters_order(Option<[u8; N]>)` and `grease_transport_parameter(...)` setters, ship a Chrome-146 fixed-order preset. ~3–5 person-days; rebase tax with quinn's monthly releases. Trigger condition: BrowserLeaks publishes a Chrome-vs-quinn diff that scores transport_params, or a real anti-bot vendor blocks a clean-IP request because of QUIC fingerprint.

---

## Implementation Priority

| Priority | Items | Expected Impact |
|----------|-------|-----------------|
| **P0 (Critical)** | 1–7 | Would pass CreepJS, FingerprintJS Pro detector panels |
| **P1 (High)** | 8–13 | Hardens against DataDome/Akamai behavioral analysis |
| **P2 (Medium)** | 14–20 | Future-proofs against 2026 detection trends |
| **P3 (Low)** | 21–25 | API completeness |
| **P-SOTA (2026)** | 26–32 | Required to credibly claim 2026 SOTA — closes the render-stack and behavioral gaps that separate us from Camoufox/CloakBrowser |
| **Deferred** | 33 | QUIC fingerprint — emerging vector, not yet a hard gate; revisit when quinn exposes transport_parameters reordering |

**Estimated effort:** P0 = 2–3 days, P1 = 1–2 days, P2 = 1 day, P3 = half day, **P-SOTA = 25–35 person-days** (dominated by #26 WebGL `wgpu`+Lavapipe wiring and #31 behavioral Sigma-Lognormal modeling). See [`SOTA_ROADMAP_2026.md`](SOTA_ROADMAP_2026.md) for the 3-phase sequenced plan.

### Until P-SOTA lands

1. Drop "18/18 stealth, 71/71 sites" from the README — replace with a reproducible benchmark output committed under `benchmarks/results.json` that includes browser_oxide rows alongside competitors, with disclosed IP/proxy conditions.
2. Add a CI job that runs the `#[ignore]` anti-bot tests on a known IP and writes `benchmarks/anti_bot_results_<date>.json` with per-site pass/fail and the vendor-detection signals matched.
3. Position the project honestly: **"the only from-scratch Rust stealth engine; structurally cleaner than any Chromium fork on CDP/protocol; render-stack and behavioral parity with Camoufox is on the roadmap."**
