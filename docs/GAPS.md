# browser_oxide — Stealth Gap Analysis

**Updated:** 2026-04-09
**Tests:** 801 pass, 0 fail | **Stealth:** 18/18 checks | **Anti-bot:** 71/71 sites | **Memory:** 34 MB

---

## Current State

browser_oxide passes all 71 anti-bot test sites and scores 18/18 on stealth checks — higher than Chrome headless (16/18), Puppeteer+Stealth (14/18), Camoufox (13/18), and Lightpanda (8/18).

However, advanced fingerprinting tools like **CreepJS** and **FingerprintJS Pro** probe deeper than our 18-check suite. This document catalogs the remaining gaps, ordered by detection risk.

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

## Implementation Priority

| Priority | Items | Expected Impact |
|----------|-------|-----------------|
| **P0 (Critical)** | 1-7 | Would pass CreepJS, FingerprintJS Pro |
| **P1 (High)** | 8-13 | Hardens against DataDome/Akamai behavioral analysis |
| **P2 (Medium)** | 14-20 | Future-proofs against 2026 detection trends |
| **P3 (Low)** | 21-25 | API completeness |

Estimated effort: P0 = 2-3 days, P1 = 1-2 days, P2 = 1 day, P3 = half day.
