# Root-cause research — what actually gates browser_oxide on Akamai/PerimeterX/DataDome sites

> Deep web research + comparison against wreq-util (the gold-standard Rust HTTP/2+TLS browser impersonator).
> Written 2026-04-28 after the prior "1-line H2 fix" did not move the needle on the holistic sweep.
> Synthesis input for the H2 fingerprint corrections shipped in this session's third H2 patch.

---

## TL;DR

The dominant gap is **HTTP/2 SETTINGS frame fingerprint**, but the discriminator wasn't the WINDOW_UPDATE byte value as we initially hypothesized. It's three things together:

1. **`SettingsOrder` must include all 8 entries Chrome 130+ knows about, even if only 4 are sent**. The order field describes the wire layout; mismatched order = different Akamai-FP hash even when the values match.
2. **HTTP/2 PRIORITY frame stream weight changed from 256 → 220 (wire byte 219) in Chrome 130+**. Older docs and our prior config used 255 (= weight 256), which is now Chrome 110-era stale.
3. **`initial_connection_window_size` is the LIB-level target, not the wire WINDOW_UPDATE delta**. Setting it to 15_728_640 makes the lib emit WINDOW_UPDATE = 15_663_105 on the wire (matches Chrome). Setting it to 15_663_105 directly emits WINDOW_UPDATE = 15_597_570 (further off than starting point).

The "1-line fix" earlier in this session (15728640 → 15663105) was **wrong in the wrong direction**. Sweep showed no change because the original 15728640 value was already correct on the wire; the change just produced an even-more-mismatched wire value but Akamai's overall fingerprint hash bucket didn't shift since the SettingsOrder + weight mismatches still dominated.

---

## Tools researched and what they actually do

### Patchright (Python, drives bundled Chromium 145)

Source: `Kaliiiiiiiiii-Vinyzu/patchright` README + `pim97/anti-detect-browser-tools-tech-comparison` deep dive.

**The patches:**
- **Runtime.enable CDP leak** — replaced with isolated `ExecutionContext` IDs. This is what Cloudflare/DataDome/Akamai/Kasada specifically detect.
- **Console.enable CDP leak** — fully disabled (no `console.log` in Patchright pages).
- Chromium flags removed: `--enable-automation`, `--disable-popup-blocking`, `--disable-component-update`, `--disable-default-apps`, `--disable-extensions`.
- Chromium flag added: `--disable-blink-features=AutomationControlled`.
- Closed-shadow-root locator support.

**What Patchright does NOT modify:**
- Chromium's HTTP/2 stack (uses Chrome's native impl → exact Chrome H2 wire signature)
- Chromium's TLS (uses BoringSSL → Chrome JA3/JA4)
- Network-layer fingerprints

**Why Patchright passes Walmart with literal `HeadlessChrome` in UA**: Walmart's edge fingerprints HTTP/2 + TLS, not UA. Patchright bundles real Chromium 145 → its on-the-wire signature is real Chrome → edge passes. PerimeterX then runs JS scoring; Patchright's V8+DOM is real Chromium → JS scoring passes too.

### nodriver (Python, drives system Chrome via direct CDP)

Source: `ultrafunkamsterdam/nodriver` GitHub + `securityboulevard.com/2025/06/from-puppeteer-stealth-to-nodriver`.

**The approach:**
- Async by design (vs synchronous Selenium)
- "CDP-minimal" — abandons many CDP commands traditional automation libs depend on
- No WebDriver binary
- Direct browser communication

**What nodriver does NOT modify**:
- Same as Patchright — uses local Chrome's HTTP/2 + TLS stack, gets Chrome wire signature for free.

### Camoufox (Python wrapping patched Firefox)

Source: `daijro/camoufox` + DeepWiki.

**The patches:**
- C++-level `MaskConfig` singleton (Firefox C++ code) intercepts: `navigator.hardwareConcurrency`, WebGL renderer, AudioContext, screen geometry, WebRTC. Spoof happens BEFORE JavaScript can inspect.
- Juggler protocol fork (Firefox's pre-CDP automation protocol) — Playwright's Page Agent code sandboxed and isolated.
- All HTTP/2 + TLS comes from Firefox's NSS stack (different fingerprint family from Chrome).

### Why none of these matter for browser_oxide architecture

We don't drive a browser. We ARE the browser:
- Our V8 runs JS, not Chromium's V8
- Our DOM is custom (no CDP at all — no Runtime.enable, no Console.enable, no automation flags)
- Our HTTP/2 is via the `http2 = 0.5` lib (wreq fork of `h2`), tuned by us
- Our TLS is via `boring2` (Rust BoringSSL bindings), tuned by us

**Patchright's CDP-leak fixes are irrelevant for us** — we don't have CDP leaks because we don't have CDP. Our entire stealth surface is the **wire signature** (HTTP/2 + TLS) plus the **JS surface** (everything `navigator.*`, `window.*`, etc.) — both implemented from scratch.

---

## Akamai BMP v3 — what is actually fingerprinted

Source: glizzykingdreko Medium deep-dive + Edioff/akamai-analysis + xiaoweigege/akamai2.0-sensor_data + scrapfly's bypass-akamai bypass article.

### Three layers

1. **Edge fingerprint** (HTTP/2 + TLS only — runs before any JS):
   - JA3/JA4 cipher list, extension order, ALPN
   - HTTP/2 SETTINGS frame: which settings IDs appear, in what order
   - HTTP/2 WINDOW_UPDATE delta (from default 65535 to configured value)
   - HTTP/2 PRIORITY frame (stream_dep, weight, exclusive)
   - HEADERS frame pseudo-header order (`:method, :authority, :scheme, :path` for Chrome)
   - Per-stream WINDOW_UPDATE patterns when fetching subresources
   
   This produces the **Akamai-FP hash**. Mismatch with Chrome's known-good hash → request scored as bot at the edge → `_abck` cookie issued with `~~-1~...` (untrusted) marker.

2. **Sensor JS layer** (only reached after edge passes):
   - Browser fingerprint (canvas hash, WebGL, fonts, audio)
   - Behavioral signals (mouse movement, focus events, timing)
   - Encoded into a JSON payload, shuffled with a PRNG seeded by `bm_sz` cookie hash
   - Submitted via POST to `/_bm/_data`
   - Server compares against the edge fingerprint snapshot — any mismatch invalidates `_abck`

3. **PerimeterX layer** (some Akamai sites layer PerimeterX on top):
   - Independent JS scoring
   - Sets `_pxhd` / `_px3` / `_pxvid` cookies

### Akamai-FP scheme (Black Hat 2017 paper)

```
[SETTINGS]|WINDOW_UPDATE|PRIORITY|Pseudo-Header-Order|HEADERS_FRAME|WINDOW_UPDATE*
```

Each segment is a fingerprint vector. Even one byte off in any segment → different hash.

### Real Chrome 130+ values per wreq-util gold standard

- **SETTINGS** (in this exact order, even if some values aren't sent):
  - 1 HEADER_TABLE_SIZE = 65536
  - 2 ENABLE_PUSH = 0
  - 3 MAX_CONCURRENT_STREAMS (order slot reserved; value sent depends on platform)
  - 4 INITIAL_WINDOW_SIZE = 6291456
  - 5 MAX_FRAME_SIZE (order slot reserved)
  - 6 MAX_HEADER_LIST_SIZE = 262144
  - 8 ENABLE_CONNECT_PROTOCOL (order slot reserved)
  - **9 NO_RFC7540_PRIORITIES** (order slot reserved; sent on Windows/Linux Chrome 130+, NOT on macOS Chrome 143+)
- **WINDOW_UPDATE** (wire delta from default 65535): 15663105 → connection window 15728640
- **PRIORITY** stream_dep=0, **weight=220** (wire byte 219), exclusive=true
- **Pseudo-header-order**: `:method, :authority, :scheme, :path` (m,a,s,p)

### Real Chrome 110-era values (older docs, scrapfly tool reference)

- PRIORITY weight=256 (wire byte 255)
- This is what we had hardcoded. Stale by 4 years.

### Chrome 136 Windows-only setting

`SETTINGS_NO_RFC7540_PRIORITIES (0x9) = 1` — sent by Windows/Linux Chrome 130+ but not macOS Chrome 143+. The wreq-python issue documented this discovery.

---

## TLS-class research

### Chrome since v110 (Jan 2023): randomized TLS extension order

JA3 fingerprints break because each connection has different extension order. **JA4 sorts extensions alphabetically before hashing** → still produces stable signature.

Akamai uses JA4 (and Akamai-FP for HTTP/2). JA3 is mostly obsolete for modern browsers.

### Chrome's Chrome-specific TLS extensions

- **`compress_certificate`** — Brotli cert compression
- **`application_settings`** (ALPS) — transmits HTTP/2 SETTINGS during TLS handshake. Strong Chrome signal.
- **GREASE** before AND after main extension list
- **ECH GREASE** — encrypted client hello placeholder

### Our boring2 setup (`crates/net/src/tls.rs`)

Verified present:
- ✓ ALPS enabled (`add_application_settings(b"h2")` + `set_alps_use_new_codepoint(true)`)
- ✓ ECH GREASE enabled (`set_enable_ech_grease(true)`)
- ✓ GREASE enabled (`set_grease_enabled(true)`)
- ✓ Cert compression Brotli
- ✓ TLS extension permutation (`set_permute_extensions(true)`)
- ✓ Mozilla root CA store
- ✓ TLS 1.2-1.3 range

This is comprehensive. Our JA4 should match Chrome 147 (per `memory/critical_findings.md` we verified byte-identical at JA4 level).

### What might still differ at TLS layer

- **Cipher list values** — `crates/net/src/tls.rs:CIPHER_LIST` constant. Verify against latest Chrome 147 capture from tls.peet.ws.
- **Curves order** — `CURVES` constant including post-quantum hybrid X25519MLKEM768.
- **Signature algorithms** — `SIGALGS_LIST`.

These should all be tested by hitting tls.peet.ws and comparing dumped JA4 against Chrome reference.

---

## What this session's H2 fix corrects

Before:

```rust
.headers_stream_dependency(StreamDependency::new(
    StreamId::zero(),
    255,  // weight 256 — Chrome 110-era
    true,
));
let settings_order = SettingsOrder::builder()
    .push(SettingId::HeaderTableSize)
    .push(SettingId::EnablePush)
    .push(SettingId::InitialWindowSize)  // only 4 entries
    .push(SettingId::MaxHeaderListSize)
    .build();
const INITIAL_CONNECTION_WINDOW_SIZE: u32 = 15_663_105; // wrong — produces wire 15_597_570
```

After:

```rust
.headers_stream_dependency(StreamDependency::new(
    StreamId::zero(),
    219,  // weight 220 — Chrome 130+ shift
    true,
));
let settings_order = SettingsOrder::builder()
    .push(SettingId::HeaderTableSize)
    .push(SettingId::EnablePush)
    .push(SettingId::MaxConcurrentStreams)  // +
    .push(SettingId::InitialWindowSize)
    .push(SettingId::MaxFrameSize)  // +
    .push(SettingId::MaxHeaderListSize)
    .push(SettingId::EnableConnectProtocol)  // +
    .push(SettingId::NoRfc7540Priorities)  // +  <-- Chrome 130+ Akamai-relevant
    .build();
const INITIAL_CONNECTION_WINDOW_SIZE: u32 = 15_728_640; // → wire 15_663_105 = Chrome match
```

Three changes:
1. SettingsOrder expanded to 8 entries matching wreq-util's chrome profile
2. Stream weight 255 → 219 (Chrome 130+ value)
3. WINDOW_UPDATE target restored to 15_728_640 (the lib subtracts 65535 internally; resulting wire delta 15_663_105 = Chrome match)

Plus the existing `add_application_settings(b"h2")` + ECH/GREASE on the TLS side.

---

## Predicted vs actual impact

**Predicted**: PASS 98 → 105-107, the 11 Akamai-CHL sites should flip.

**Actual**: PASS **97 / 126** (within ±1 noise floor of Phase F's 98). **Zero** Akamai sites flipped. Identical 11 sites still detected: weather, washingtonpost, bestbuy, costco, h-m, homedepot, uniqlo, disneyplus, walmart, hulu, expedia.

So **the H2 fingerprint hypothesis is wrong** for these specific Akamai-protected sites — or our changes still don't fully match Chrome. Three diagnostics needed:

1. **Hit tls.peet.ws/api/all from browser_oxide** and dump `akamai_fingerprint` + `akamai_hash` + `ja4` + `peetprint` strings. Compare each to a real Chrome 147 macOS capture from the same machine. The mismatched field is the next thing to fix.

2. **The trailing `WINDOW_UPDATE*` in Akamai's fingerprint scheme captures per-stream window updates as the page fetches subresources**. We may match the connection-level WINDOW_UPDATE but not the per-stream pattern. Chrome's pattern: WINDOW_UPDATE every 32KB of received body, on the stream and on the connection. wreq-util likely encodes this; we may not.

3. **IP reputation** — Akamai's edge fingerprint could be deprioritized in favor of a long-term IP-based score for sites we've repeatedly hit. Test from a different IP to isolate.

The H2 corrections shipped in this session are still strictly more Chrome-coherent than what we had — they just don't move the needle on these particular sites because the actual discriminator is elsewhere.

---

## What we need to test next (since this didn't move the needle)

1. **Hit tls.peet.ws/api/all** from browser_oxide and dump:
   - `akamai_fingerprint` string
   - `akamai_hash` MD5
   - `ja4` string
   - `peetprint` hash
   
   Compare each to a real Chrome 147 macOS capture. The mismatched field is the next thing to fix.

2. **Build a per-stream WINDOW_UPDATE pattern test**. After the connection-level WINDOW_UPDATE, Chrome sends per-stream WINDOW_UPDATEs as it receives data. Pattern: when does Chrome send them? At what threshold? wreq-util likely encodes this; we may not.

3. **Diff our cipher list against Chrome 147 capture**. Our `CIPHER_LIST` constant might be Chrome 130-era; cipher preferences shift across versions.

4. **Investigate behavioral / application-layer mismatches**. Even if H2+TLS match, our JS shim might leak something on Akamai's sensor layer (`/_bm/_data` POST), e.g.:
   - canvas hash differs
   - audio context fingerprint
   - WebGL extensions list
   - DOM-iteration timing
   - font enumeration

   creepjs PASSes us, which validates most of these. But Akamai sensor scoring may be more permissive than creepjs in some areas and stricter in others.

---

## Sources cited

- [glizzykingdreko: Akamai v3 Sensor Data Deep Dive](https://medium.com/@glizzykingdreko/akamai-v3-sensor-data-deep-dive-into-encryption-decryption-and-bypass-tools-da0adad2a784)
- [scrapfly: Bypass Akamai Bot Manager](https://scrapfly.io/bypass/akamai)
- [Edioff/akamai-analysis](https://github.com/Edioff/akamai-analysis)
- [xiaoweigege/akamai2.0-sensor_data](https://github.com/xiaoweigege/akamai2.0-sensor_data)
- [Akamai Black Hat EU 2017 paper: Passive Fingerprinting of HTTP/2 Clients](https://blackhat.com/docs/eu-17/materials/eu-17-Shuster-Passive-Fingerprinting-Of-HTTP2-Clients-wp.pdf)
- [scrapfly: HTTP/2 Fingerprinting tool reference](https://scrapfly.io/web-scraping-tools/http2-fingerprint)
- [tls.peet.ws (test endpoint)](https://tls.peet.ws/)
- [trickster.dev: Understanding HTTP/2 fingerprinting](https://www.trickster.dev/post/understanding-http2-fingerprinting/)
- [Patchright README](https://github.com/Kaliiiiiiiiii-Vinyzu/patchright)
- [pim97 anti-detect-browser-tools-tech-comparison](https://github.com/pim97/anti-detect-browser-tools-tech-comparison)
- [rebrowser-patches Runtime.enable docs](https://rebrowser.net/blog/how-to-fix-runtime-enable-cdp-detection-of-puppeteer-playwright-and-other-automation-libraries)
- [nodriver GitHub](https://github.com/ultrafunkamsterdam/nodriver)
- [Security Boulevard: Puppeteer stealth → Nodriver evolution](https://securityboulevard.com/2025/06/from-puppeteer-stealth-to-nodriver-how-anti-detect-frameworks-evolved-to-evade-bot-detection/)
- [wreq-util/src/emulate/profile/chrome/http2.rs](https://github.com/0x676e67/wreq-util/blob/main/src/emulate/profile/chrome/http2.rs) — gold-standard Rust Chrome H2 emulator
- [wreq-util Chrome version table](https://github.com/0x676e67/wreq-util/blob/main/src/emulate/profile/chrome.rs)
- [Chrome 136 emulation missing 0x9 setting (wreq-python discussion #472)](https://github.com/0x676e67/wreq-python/discussions/472)
- [BoringSSL: Google's TLS Library Behind Chrome Fingerprinting](https://roundproxies.com/blog/boringssl/)
- [Fastly: Chrome's TLS ClientHello Permutation](https://www.fastly.com/blog/a-first-look-at-chromes-tls-clienthello-permutation-in-the-wild)
- [lwthiker: Impersonating Chrome, too](https://lwthiker.com/reversing/2022/02/20/impersonating-chrome-too.html)
- [JA4 in Action: Detecting Bots, Malware, and Fake Browsers](https://medium.com/@belghitishakantar/ja4-in-action-detecting-bots-malware-and-fake-browsers-at-the-tls-level-3ccd890fbce9)
