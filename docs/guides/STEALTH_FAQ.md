# Guide: stealth FAQ — what's native, what's not

BrowserOxide's bet is that a fingerprint is most convincing when it's
*native* — a property of a real engine — rather than *injected* over someone
else's browser. This guide is the honest boundary of what that buys you.

### Why "native" beats patched Chrome / patched Firefox

- **Chrome + CDP** (Puppeteer/Playwright/stealth plugins): you're hiding the
  puppet strings of a browser designed to be controllable — `Runtime.enable`
  leaks, `navigator.webdriver`, `cdc_*` vars, CDP WebSocket fingerprints. There's
  no CDP here at all.
- **Patched Chromium/Firefox forks**: inherit the base engine's detection vectors
  (or ride a low-market-share engine, itself a signal).
- BrowserOxide controls every surface from the TLS handshake through WASM to
  canvas, so the properties are coherent by construction. Whether that's the
  *right* bet is empirical — see [../BENCHMARK.md](../BENCHMARK.md).

### What's real

- **TLS**: own BoringSSL stack; Chrome- and Firefox-accurate ClientHello, JA3/JA4,
  HTTP/2 SETTINGS/frame fingerprint.
- **JS**: full V8 (ES2024+, WASM, JIT) — runs real challenge WASM and heavy SPAs.
- **DOM/CSS**: from-scratch parser, Selectors L4, cascade, layout (box model).
- **Canvas 2D**: real rasterization via `tiny-skia` (deterministic per-profile
  seed) — not a stub.
- **Fingerprint surface**: 100+ coherent `navigator`/`window`/screen/GPU props.

### What's *not* implemented (be honest in your integration)

- **No real WebGL/GPU raster.** WebGL is parameter/extension *stubs* for
  fingerprint queries; there's no GPU pipeline. Sites that gate on a real WebGL
  *render* (vs. parameter reads) are a known limit.
- **No HTTP/3 by default.** Wired but off — vanilla `quinn-proto` emits
  randomized transport params, a *worse* tell than not speaking h3.
- **No audio playback.** `AudioContext` exists for fingerprinting, not sound.
- **No vendor bypass code.** Hard challenges (Kasada; DataDome interactive
  Device-Check) are **not** auto-cleared — see [CHALLENGES.md](CHALLENGES.md) and
  the `ChallengeSolver` hook. Kasada is the standing open gap (no OSS tool passes
  it from scratch).
- **Service Workers** are in-memory only (no persistence across navigations).

### Coherence > any single trick (a worked example)

Making one layer "stronger" in isolation can *hurt*. When the Firefox profile was
emitting a Firefox UA over a *Chrome* TLS handshake, `adidas.com`'s Akamai tenant
waved it through (it had no confident bot anchor for that odd combo). Shipping a
*coherent* real Firefox TLS handshake made Firefox-on-adidas start failing — the
combo no longer looked anomalous. That's the correct global tradeoff (coherent
everywhere), and routing to the Chrome/iPhone profile still wins the site. Lesson:
optimize the *whole* identity, not one axis.

### Practical tips

- **Space same-IP, same-vendor requests.** Bursts trip token-clustering →
  false failures. The benchmark spaces them with a vendor cooldown.
- **Use cold `Page::navigate` for protected sites**; the warm `PagePool` skips the
  challenge-follow loop (great for benign high-throughput pages, wrong for walls).
- **Route across profiles** — see [PROFILES.md](PROFILES.md).
- **Trust `challenge_verdict()`, not HTTP status** — challenge pages return 200.
