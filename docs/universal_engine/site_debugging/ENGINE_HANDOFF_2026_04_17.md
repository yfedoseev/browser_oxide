# Blocker Debugging Handoff (Rigorous Suite)
**Date: 2026-04-17**

This is a self-contained handoff. A new dev opening only this file should be able to clone the repo, get a green test suite, reproduce the Canada Goose / Hyatt blocks, understand why they fail, and start executing the §6 roadmap. If anything below is unclear or stale, fix it in the doc itself before doing other work — docs rot fastest when people refuse to touch them.

## 0. Day-One Runbook

### 0.1 Repo + toolchain

```bash
git clone https://github.com/nicepkg/browser_oxide
cd browser_oxide
# Primary branch of active work: universal-engine-stabilization
git checkout universal-engine-stabilization
```

Toolchain requirements:

- **Rust**: stable, edition 2021 (check `rust-toolchain.toml` if present; otherwise latest stable works)
- **deno_core 0.311** is locked in `Cargo.toml` — do NOT bump without understanding V8 prebuilt-binary compatibility (CLAUDE.md calls this out)
- **V8 prebuilts**: ~130 MB download happens on first `cargo build`; expect a slow first build
- **BoringSSL**: `rquest`/`boring2` brings its own; no system BoringSSL needed
- **Node 20+**: required for the Playwright / Patchright probes (`docs/universal_engine/site_debugging/*.js`)
- **Python 3.11+**: required for the nodriver probe (`docs/universal_engine/site_debugging/nodriver_probe.py`)

License constraint: MIT / Apache-2.0 only. **Do not add MPL-licensed deps** (rules out Servo's `selectors`, `cssparser`, `style` crates — that's why we wrote our own CSS stack).

### 0.2 First green test suite

```bash
# Full build + unit tests. V8 isolates are per-thread, so --test-threads=1 is
# mandatory; otherwise tests deadlock or V8 asserts.
cargo test --workspace -- --test-threads=1

# Lint + format must stay clean
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check
```

Expected: `test result: ok` across all crates. If anything fails on a clean checkout, that's a regression — fix before proceeding. Approximate count: ~1000 passing tests at the last known-good commit (`c79bf24` + subsequent commits on `universal-engine-stabilization`).

### 0.3 Reproduce the Canada Goose / Hyatt block

This is the central motivating observation of the §3 diagnosis. Do it first so you believe the numbers.

```bash
# 1. Egress IP check — confirms we're NOT on a datacenter
curl -s https://ipinfo.io/json | python3 -m json.tool
# Expected: a residential-ISP ASN (on Yury's machine: AS852 TELUS, Vancouver).
# If you see a cloud-provider ASN, everything in §3 is wrong for YOUR setup;
# STOP and talk to Yury before re-running the diagnosis.

# 2. Set up the probes (one-time, takes ~2 min)
cd docs/universal_engine/site_debugging
cp probes_package.json package.json
npm install                                   # Installs playwright + patchright
python3 -m venv venv && . venv/bin/activate
pip install nodriver

# 3. Run the three headless probes
node playwright_probe.js  https://www.canadagoose.com/   # expect: 429 + ~740b
node patchright_probe.js  https://www.canadagoose.com/   # expect: 429 + ~740b
python3 nodriver_probe.py https://www.canadagoose.com/   # expect: 429 + ~720b

# 4. Run our engine against the same URL
cd ../../..
cargo test --release -p browser --test tier0_kasada \
  kasada_poc_canadagoose_diagnostic \
  -- --ignored --test-threads=1 --nocapture
# expect: 429 + ~732b — same bucket as (3)

# 5. THE CONTROL: open https://www.canadagoose.com/ in your real desktop
#    Chrome (headed, with your profile) on the SAME machine and SAME network.
#    You will see the full product homepage. That's the datapoint that rules
#    out "IP reputation" and points at headless-fingerprint parity.
```

If step 5 doesn't pass in your real Chrome, you ARE on a flagged IP — in which case skip to "residential proxy" as a stopgap and file an issue; the §6 roadmap assumes a clean residential egress.

### 0.4 Invoke the engine against a single URL manually

No CLI binary yet. The canonical entry point is the `browser::Page` API (`crates/browser/src/lib.rs:12`). Copy this into a scratch test:

```rust
// crates/browser/tests/_scratch.rs  (add, don't commit)
use browser::Page;
use stealth::presets;

#[tokio::test]
#[ignore]
async fn scratch_nav() {
    let profile = presets::chrome_130_windows();
    let mut page = Page::with_profile(profile).await.unwrap();
    let _ = page.navigate("https://example.com/").await.unwrap();
    println!("final_url: {}", page.url());
    println!("body_len:  {}", page.html().len());
    println!("body[..500]: {}", &page.html()[..page.html().len().min(500)]);
}
```

Run: `cargo test --release -p browser --test _scratch -- --ignored --test-threads=1 --nocapture`

Relevant public API: `Page::with_profile` (`crates/browser/src/page.rs:403`), `Page::navigate` (`:477`), `Page::navigate_stealth` (`:449`), `Page::navigate_humanized` (`:489`), `Page::navigate_with_challenges` (`:1028`), `Page::evaluate_async` (`:286`).

### 0.5 Rigorous suite (the 6/8 WIN scoreboard)

```bash
cargo test --release -p browser --test tier0_kasada \
  -- --ignored --test-threads=1 --nocapture 2>&1 | tee /tmp/tier0_run.log
```

Passes today: HomeDepot, Wildberries Solver, Yandex, plus 3 others. Fails: Canada Goose, Hyatt (the subject of §3). Any drop below 6/8 is a regression.

### 0.6 Browser-comparison benchmarks (optional)

```bash
cargo test --release -p browser --test browser_comparison \
  -- --ignored --test-threads=1 --nocapture
```

Produces the numbers in `README.md`'s top table (memory, startup, JS-eval speed, page-load). Run before/after a phase to catch performance regressions.

---

## 1. Current Progress Snapshot
*   **Verification Score**: **8/8 PASS** (Primary targets like Adidas, Southwest, Ticketmaster, DNS-Shop, Wildberries).
*   **Rigorous Suite Score**: **6/8 WIN** (Unblocked HomeDepot, Wildberries Solver, and **Yandex** this session).
*   **Remaining Fails**: Canada Goose (Kasada), Hyatt (Kasada).

---

## 2. Breakthrough: Yandex (SmartCaptcha / SSO Redirect)
**Status**: **WIN** (488,914b real homepage loaded).

### Technical Debt Fixed
1.  **h1 POST fallback**: Fixed bug where query strings were stripped during HTTP/1.1 POST fallback. This unblocked the Yandex SSO `/install?uuid=...` endpoint.
2.  **Bootstrap Cleanup**: Fixed `cleanup_bootstrap.js` wiping `__pendingNavigation` too early.
3.  **Form Reflection**: Added `HTMLFormElement` and `HTMLInputElement` property reflection, allowing JS-based form submissions to work correctly.
4.  **Navigation Loop**: Updated `Page::navigate` to correctly handle auto-submitted forms and SSO redirects.

---

## 3. Issue: Kasada (Canada Goose / Hyatt)
**Symptom**: Stuck on 732-byte challenge page. Solver runs end-to-end but server never upgrades us to real content.

### Diagnostic Findings
`ips.js` loads at full 519KB and executes. Fetch log shows the full success trace:
*   `POST /149e9513-…/tl` returns `200` with `x-kpsdk-cr: true` — server accepts challenge
*   Fresh `x-kpsdk-ct` returned, Set-Cookie with `akm_bmfp_b2=…` lands in jar
*   `document.cookie` at exit contains the new token (cookie sync works)
*   Post-settle retry + in-V8 refetch both fire carrying the fresh cookie
*   Server still returns the 732-byte challenge on every retry

TLS / H2 / headers match Chrome 146 capture (verified via tls.peet.ws). Script-fetch headers include referer + sec-fetch-*. `ips.js` contains **zero** `location`/`reload`/`href` references in 519KB — it never triggers navigation itself.

### Ruled Out This Session (exhaustive diagnostics, 3 experiments)

**ips.js does NOT patch `window.fetch` globally.** `window.fetch.toString()` still returns OUR wrapper unchanged after ips.js runs. ips.js passes `x-kpsdk-ct` explicitly as `init.headers` only on its own requests to `/tl` and `/fp`.

**KPSDK state is closure-private.** After solve, `window.KPSDK` only exposes `{now, start, scriptStart}`. The solved token lives in ips.js closures, unreachable from outside.

**sec-fetch-site = same-origin + no sec-fetch-user + Referer doesn't help.** Added `chrome_headers_reload()` + `get_follow_exact_headers()` and wired reload-semantic headers into the retry path. Server still returns 429.

**Cookies, timing, and TLS state are ALL independent of the block.** The raw-cookies diagnostic proves it:

| Step | Request | Result |
|---|---|---|
| 1 | Initial GET with clean jar | `429, 681b` |
| 2 | Immediate re-GET (same H2 pool, cookies from jar) | `429, 681b` |
| 3 | GET with reload headers + Referer | `429, 681b` |
| 4 | GET after 3s wait | `429, 681b` |
| 5 | **FRESH HttpClient, no cookies, new TLS** | `429, 681b` |

Step 5 is decisive: a brand-new client with no prior state gets the identical block. So:
- Not TLS session-ticket pinning (fresh TLS → same block)
- Not cookie freshness (no cookies → same block)
- Not timing (wait → same block)
- Not connection reuse (new connection → same block)

**Subpath test confirms it's not URL-specific.** Root, `/us/en`, product pages, category pages — all return the Kasada challenge page. Every URL on canadagoose.com returns 680-770 bytes with the challenge markers.

### Remaining Candidate: Headless-Browser Fingerprint + Zero Behavior

**Correction (2026-04-17, second pass):** the earlier "datacenter IP reputation" conclusion was wrong. `curl ipinfo.io` from the machine where every probe ran shows egress via `AS852 TELUS Communications` — a **residential ISP in Vancouver**, the same IP the user's headed Chrome uses to successfully load canadagoose.com. The gate is not the IP.

What actually differs between the passing Chrome and the failing probes, at the same residential IP:

1. **Headless vs. headed Chromium.** Every probe used `headless: true`. Chrome's new headless mode still leaks: empty `navigator.plugins`, SwiftShader/ANGLE software WebGL renderer instead of real GPU, `Notification.permission === 'denied'`, `document.hidden`-adjacent state, cut-down `window.chrome`, CDP-attached timing signatures.
2. **Stale UA.** Probes sent `Chrome/130.0.0.0`. Real Chrome on 2026-04-17 is 133+. UA/client-hint version mismatch is a known Kasada tell.
3. **Platform lie.** Probe UA claims `Windows NT 10.0`; `navigator.platform`/`userAgentData` from headless Linux leaks `"Linux x86_64"` / `"Linux"`. Internally-inconsistent identity.
4. **Zero behavioral signals.** Kasada's Bot Manager scores mouse/pointer/scroll/key events accumulated before the `/ftp:` probe. Headed Chrome had events (the user moved a mouse, typed in the omnibox). All probes called `goto()` with no input events ever dispatched.
5. **Fresh profile.** Headed Chrome carries years of 3p partitioned cookies (google.com, youtube.com), Private State Tokens issued from prior browsing, possibly still-valid `akm_bmfp_b2` from previous visits, populated localStorage/IndexedDB. Probes start empty.
6. **High-entropy Client Hints gap.** Real Chrome answers `Accept-CH` with `sec-ch-ua-platform-version`, `-arch`, `-bitness`, `-full-version-list`, `-model`. Headless answers partially or inconsistently.
7. **No HTTP/3.** Real Chrome negotiates h3 via Alt-Svc on repeat visits; probes stay h2.
8. **Software canvas + software audio.** SwiftShader canvas/WebGL and headless audio pipeline produce known-automation pixel/PCM hashes.

For browser_oxide the same items apply: we're a pure-Rust engine with no rendering; fingerprint items 1, 3, 4, 5, 6, 8 all hit us regardless of the excellent TLS/H2 work we've done. That's why we fail identically to Patchright and nodriver from the same residential IP.

**The engine is client-side complete for the ips.js protocol (three headers, no hidden state — deep-instrumentation harness confirmed).** What's missing is the BROWSER-IDENTITY surface, not the Kasada-protocol surface.

### Fix Path — no infrastructure needed, engineering-only

Canada Goose / Hyatt passable from *this same residential IP* with:
1. **Headless-browser fingerprint parity in the engine** (this is the §6 roadmap below) — browser_oxide becomes a pure-Rust "headless-that-looks-headed" crawler.
2. Commercial session service (Hyper-Solutions, RiskByPass) — still an option if (1) is deferred.
3. Headed Chromium via CDP for just these sites — acceptable fallback during (1)'s build-out.

### What the Engine Gained From This Investigation (universal primitives)

1. **MAJOR: fetch-API header style on op_fetch** (`chrome_headers_fetch` +
   `HttpClient::fetch_get/fetch_post_bytes`). Previously every JS `fetch()` call
   was sent with NAVIGATION headers (`upgrade-insecure-requests`, `accept: text/html`,
   `sec-fetch-dest: document`, `sec-fetch-mode: navigate`, `sec-fetch-user: ?1`,
   `priority: u=0`). Now proper fetch API style: `accept: */*`,
   `sec-fetch-dest: empty`, `sec-fetch-mode: cors`, auto-computed `sec-fetch-site`,
   `origin` + `referer` auto-injected, `priority: u=1, i`. This was a huge
   latent bot tell affecting every JS fetch on every site.

2. **Real `navigator.sendBeacon`** — was a no-op stub returning `true`. Now
   fires a real `fetch(..., {keepalive: true})`. Some challenge engines send
   solve-completion payloads via sendBeacon.

3. **x-kpsdk-* harvesting primitive** — navigate_loop_internal extracts every
   `x-kpsdk-*` header seen in req/resp across `__fetchLog` and injects them on
   the same-origin retry GET. Per Hyper-Solutions Go SDK, Kasada retries need
   8 of these as REQUEST headers; we successfully harvest 6.

4. `chrome_headers_reload()` + `get_follow_exact_headers()` — reload-semantic
   header set for any future same-origin retry.

5. In-V8 refetch primitive — valuable for fetch-patching engines
   (PerimeterX / DataDome variants).

### Kasada Remaining Gap — Reverse-engineered conclusion

Wrote a deep instrumentation harness
(`kasada_ips_deep_instrumentation` in `tests/tier0_kasada.rs`) that monkey-patches:
- `String.fromCharCode` (catches tokens built from char codes)
- `Object.defineProperty` (catches properties set to token-like strings)
- `Function` constructor via Proxy (catches dynamically-built function bodies)
- `EventTarget.addEventListener` / `dispatchEvent`
- `navigator.sendBeacon`
- `XMLHttpRequest.open/setRequestHeader/send`
- `Headers.set/append`
- `window.fetch`

Ran ips.js against this harness for 60 seconds. Final findings:

*   ips.js only ever sends **three** x-kpsdk-* headers on outgoing requests:
    `ct`, `dt`, `im`. That's all. We already harvest these.
*   Zero `navigator.sendBeacon` calls.
*   Zero `kpsdk-ready`/`kpsdk-*` listener registrations.
*   Zero dynamically-created Function bodies contain token-like strings.
*   `window.KPSDK` never grows beyond `{now, start, scriptStart}` after
    execution.

**The "missing" x-kpsdk-v/-dv/-h/-fc tokens are a red herring** from
Hyper-Solutions' public docs — they reflect a different Kasada deployment
generation. Our engine does everything ips.js actually does in 2026's
canadagoose.com build.

### The Actual Blocker — Server-side /ftp: canary + IP Reputation (CONFIRMED)

ips.js uses a probe pattern: after each solve it fetches `/ftp:` and checks
the response. On a solved session the server eventually returns something
that ips.js interprets as "upgrade complete" → document replacement happens.
Our `/ftp:` requests ALWAYS return `429 + challenge-stub` regardless of
what we send.

**Proof via real Playwright + real HEADLESS Chromium** (see
`/tmp/kasada-probe/probe.js`):

| Site | Headless Chromium from same residential IP | Our engine |
|---|---|---|
| canadagoose.com | 429 + 740b challenge (identical body) | 429 + 732b |
| hyatt.com | 403 + Akamai E6020 "unexpected browser" | 403 equivalent |
| ya.ru | 200 + 452k real homepage | 200 + 488k real homepage |

Every probe runs with `headless: true` (see
`playwright_probe.js:6`, `patchright_probe.js:6`, `nodriver_probe.py:8`).
All four tools land in the same fingerprint bucket: stale UA (130), OS
lie (UA says Windows, navigator leaks Linux), empty plugins, SwiftShader
WebGL, zero pre-nav input events, fresh profile. That bucket is what
Kasada blocks — not the IP.

**Extended cross-tool probe (also saved in this dir):**

| Tool | Implementation | Mode | Result on canadagoose.com |
|---|---|---|---|
| Our browser_oxide engine | From-scratch Rust + V8 | pure-Rust headless | 429 + 732b |
| `playwright_probe.js` — vanilla Playwright | Real Chromium | `headless: true` | 429 + 740b |
| `patchright_probe.js` — Patchright stealth Playwright | Real Chromium + JS stealth patches | `headless: true` | 429 + 740b |
| `nodriver_probe.py` — nodriver (UC successor) | CDP-direct, no Selenium | `headless: True` | 429 + 721b |
| User's real Chrome (manual UI) | Full headed Chrome with profile | `headless: false` | **200 + full homepage** |

The last row is the critical datapoint: same residential IP
(`66.183.9.212`, `AS852 TELUS`), same physical machine, same Linux
kernel — the only thing that changes between 429 and 200 is headed vs.
headless with a populated profile and organic input events.

Hyatt specifically returned an Akamai E6020 code, not Kasada — different
vendor, same fingerprint gate. Both are scoring headless-automation
identity, not IP.

**What the engine still has to close** (reachable from engine code,
client-side):
- Plugin/navigator surface parity with headed Chrome 133+
- WebGL renderer = real GPU string via `UNMASKED_RENDERER_WEBGL`
- Canvas/audio pixel-and-PCM parity with a real Chrome+GPU pair
- `window.chrome` full surface (app, csi, loadTimes)
- Synthetic pre-nav + post-load input events (mouse, pointer, scroll)
- Persistent per-origin cookies / localStorage / IndexedDB across runs
- High-entropy Client Hints responses (`sec-ch-ua-*` full set)
- HTTP/3 (QUIC) via rquest, once enabled

See §6 below for the full roadmap and ordered engineering plan.

### Fix Path for Kasada Specifically

The real gap is headless-browser fingerprint, not the ips.js protocol. Two
viable paths; both are pure client-side engineering on browser_oxide:

1. **Headed-Chrome parity in browser_oxide (primary plan, §6 below)** —
   we own the entire JS surface (V8 with our own bindings, no real Chromium
   underneath) and every network-layer field. Nothing forces us to look
   headless. Estimated 4–5 weeks to Kasada/Akamai-capable state.
2. **Commercial service** (Hyper-Solutions, RiskByPass) — optional stopgap
   during (1)'s build-out.

The universal engine is ~90% of the way to Kasada on the PROTOCOL side.
The remaining 10% is BROWSER-IDENTITY: making the engine look like headed
Chrome 133 with a populated profile and organic behavior. That's the §6
roadmap.

---

## 4. General Engine Work

### A. Worker OpState Robustness
*   **Status**: Fixed the crash where workers lacked `OpState`.
*   **Next Step**: Verify that workers spawned *by other workers* (nested workers) correctly inherit the `StealthProfile`.

### B. TLS Wire-Level Fingerprinting
*   **Status**: Using BoringSSL with Chrome 146-matched cipher order, H2 pseudo-header order, and SETTINGS frame. Verified via tls.peet.ws.
*   **Next Step**: JA4-level diff against a live Chrome 130 capture for Kasada-specific sites.

---

## 5. Relevant Files for Next Developer
*   `crates/browser/src/page.rs`: Main navigation loop logic.
*   `crates/browser/src/script_runner.rs`: Script extraction and decoding.
*   `crates/js_runtime/src/js/dom_bootstrap.js`: DOM API overrides and instrumentation.
*   `crates/net/src/lib.rs`: Stealth HTTP client and header defaults.

---

## 6. Roadmap: Headed-Chrome Parity Without UI Rendering

browser_oxide is a pure-Rust engine — V8 with our own bindings, our own DOM,
our own canvas rasterizer, our own HTTP stack. We do NOT embed Chromium. That
means, unlike Playwright/Patchright/nodriver which are stuck with whatever
Chrome's `--headless=new` exposes, **we can report any internally-consistent
browser identity we want**. There is no `headless: false` flag to flip — there
is just the JS surface and network surface we emit. The goal is to make that
surface indistinguishable from headed Chrome 133 with an established profile.

Ordered by impact on Kasada / Akamai Bot Manager scoring. Items 1–9 are
fingerprint parity; 10–14 are behavioral/session; 15–18 are network-layer;
19–23 are advanced surface.

### Phase 1 — Fingerprint parity (highest impact, ~1–2 weeks)

**1. `navigator.plugins` / `navigator.mimeTypes`**

Emit a real Chrome 133 `PluginArray` (5 entries: PDF Viewer, Chrome PDF Viewer,
Chromium PDF Viewer, Microsoft Edge PDF Viewer, WebKit built-in PDF). Each
`Plugin` has stable `name`/`description`/`filename`, iterable `MimeType` entries,
`item()`/`namedItem()` methods. `navigator.mimeTypes` mirrors them. Identity
must be stable across accesses (same object reference).
Touch points: `crates/js_runtime/src/js/dom_bootstrap.js` (or new
`plugins_bootstrap.js`), `crates/stealth/src/profile.rs` (add `plugins: Vec<PluginSpec>`).

**2. `window.chrome` full surface**

Today: partial. Needed:
- `chrome.app` — `{InstallState, RunningState, getDetails(), getIsInstalled(), isInstalled}`
- `chrome.csi()` — returns `{onloadT, pageT, startE, tran}` with numeric values tied to actual load timing
- `chrome.loadTimes()` — extended legacy timing: `requestTime, startLoadTime, commitLoadTime, finishDocumentLoadTime, finishLoadTime, firstPaintTime, firstPaintAfterLoadTime, navigationType, wasFetchedViaSpdy, wasNpnNegotiated, npnNegotiatedProtocol, wasAlternateProtocolAvailable, connectionInfo`
- `chrome.webstore` — legacy, exists as object with methods

Touch points: `crates/js_runtime/src/js/window_bootstrap.js`, a new Rust op
`op_get_chrome_timings(page_id)` that reads from `Page`'s navigation timing
record.

**3. WebGL parameter fidelity**

We already have `GpuProfile` with `unmasked_vendor`, `unmasked_renderer`,
extensions, params, shader precision. Audit:
- `WEBGL_debug_renderer_info` extension MUST be in `getSupportedExtensions()`
- `getParameter(UNMASKED_RENDERER_WEBGL)` MUST equal `gpu_profile.unmasked_renderer` exactly
- `getParameter(UNMASKED_VENDOR_WEBGL)` MUST equal `gpu_profile.unmasked_vendor`
- All 30+ `MAX_*` integer limits match a real Chrome 133 + that GPU
- `getShaderPrecisionFormat(VERTEX_SHADER, HIGH_FLOAT)` returns `{rangeMin, rangeMax, precision}` matching the GPU

Verify with `tests/fingerprint_scorers.rs` — diff our WebGL output against
creepjs / fingerprintjs-pro snapshots.

**4. Canvas rasterization consistency**

Kasada / FingerprintJS draw text + gradient + arc, then `toDataURL()`-hash
the output. Our rasterizer must produce pixel output that (a) is stable
per-profile, (b) differs slightly between profiles (so one installation
isn't flagged as "the same bot"), (c) falls in the distribution of real
Chrome+GPU hashes.

Approach: `canvas_seed` in profile (already present) + Rust rasterizer
adds deterministic sub-pixel noise keyed by seed. On top of that, build
a small "canvas replay table" for the ~10 known fingerprinting probes
(FingerprintJS, creepjs, imprintjs, bmak) — for each probe's exact draw
sequence, emit a pre-recorded pixel buffer captured from real headed
Chrome. Detection is by draw-call signature.
Touch points: `crates/js_runtime/src/js/canvas_bootstrap.js`, new Rust
module `crates/canvas/src/fingerprint_replay.rs`.

**5. AudioContext / OfflineAudioContext**

FingerprintJS pipeline: create `OfflineAudioContext`, route
`OscillatorNode` through `DynamicsCompressorNode`, render 44100 samples,
sum magnitudes, hash. Output is GPU-independent but compilation- and
platform-dependent.

Currently: unimplemented or stub. Need a small Rust audio graph (or,
faster, a replay table keyed by node-graph signature that returns a
pre-recorded buffer matching the target profile's claimed Chrome + OS).
Touch points: new `crates/audio` crate; JS bindings in a new
`audio_bootstrap.js`.

**6. Notification.permission, document.hidden, document.visibilityState**

One-line patches:
- `Notification.permission === 'default'`
- `document.hidden === false`
- `document.visibilityState === 'visible'`

Touch: `window_bootstrap.js`, `dom_bootstrap.js`.

**7. `navigator.permissions.query`**

Map name → state matching real Chrome:
- `notifications` → `{state: 'default'}` (headless returns 'denied')
- `geolocation`, `camera`, `microphone`, `midi` → `{state: 'prompt'}`
- `persistent-storage` → `{state: 'granted'}`
- `background-sync` → `{state: 'granted'}`

Touch: `window_bootstrap.js`.

**8. `navigator.userAgentData` + high-entropy Client Hints**

JS side:
- `.brands` = `[{brand:"Chromium",version:"133"}, {brand:"Google Chrome",version:"133"}, {brand:"Not-A.Brand",version:"99"}]` with GREASE ordering per Chrome algorithm
- `.mobile = false`
- `.platform = profile.os_name`
- `.getHighEntropyValues([...])` returns complete `{architecture, bitness, model, platformVersion, uaFullVersion, fullVersionList, wow64}`

HTTP side: when server sends `Accept-CH: sec-ch-ua-platform-version, sec-ch-ua-arch, ...`, record the request and respond on subsequent same-origin requests with matching headers. Currently we ship a static set — needs to be profile-driven and Accept-CH–responsive.
Touch: `crates/net/src/lib.rs` (client hints state machine), `window_bootstrap.js`.

**9. `MediaDevices.enumerateDevices`**

Return plausible device list matching `profile.media_devices`. Before
microphone/camera permission is granted, Chrome returns devices with empty
`label` fields. Honor that.

### Phase 2 — Behavioral signals (highest single-lever impact, ~2–3 weeks)

This is the biggest remaining gap for Kasada/Akamai. Even a perfect
fingerprint fails without input events.

**10. Pre-navigation and in-flight input event synthesis**

Rust-side scheduler injects synthetic events via a new `op_dispatch_event`
into the JS event loop with realistic timing:
- `mousemove` at 30–60 Hz with bezier-curve paths, small jitter, occasional pauses
- `pointermove` (pointerType:"mouse") mirroring mouse events
- `scroll` (wheel events + `window.scrollY` update) at realistic cadence if page is scrollable
- `visibilitychange` once after DOMContentLoaded
- `focus` at load, occasional `blur`/`focus` pair to simulate tab switch

Seeded per-profile via new `behavior_seed: u64` in profile — deterministic
replays, diverse patterns across profiles. Start emission immediately on
navigation commit, continue through load+2s idle.
Touch points: new `crates/behavior` crate, `crates/browser/src/page.rs`
(schedule events on nav), new `op_dispatch_event`.

**11. Post-load idle behavior**

After `load`, continue emitting mouse/scroll events for 2–5 seconds.
Kasada's score accumulates over time; the `/ftp:` probe waits for
threshold. Without post-load events, score plateaus below threshold.

**12. Form interaction emulation**

When our engine auto-submits a form (SSO / challenge form), precede
with: `focus → keydown → beforeinput → input → keyup → change → blur`
per character, with inter-keystroke delay ~80–200ms per a typing-rhythm
model. Currently we jump straight to `submit()`.
Touch: `crates/browser/src/page.rs` form-submit path.

### Phase 3 — Persistent session state (~1 week)

**13. Cookie + localStorage + IndexedDB persistence across runs**

Per-profile on-disk store keyed by origin. Cookies — already mostly
there via rquest jar — just persist the jar. localStorage and IndexedDB
currently in-memory only; persist via sled or sqlite. Each run reads
existing state first, so Kasada sees a "returning" session with possibly
still-valid `akm_bmfp_b2`.
Touch: new `crates/profile_store` crate, `crates/browser/src/page.rs`
storage hooks, `dom_bootstrap.js` storage bindings.

**14. Prior-browsing simulation (optional but cheap)**

Before hitting target site, optionally auto-visit google.com and scroll
briefly to accumulate 3p partitioned cookies + trust signals. Gated by
profile flag (slows every run by ~2s).

### Phase 4 — Network-layer parity (~1 week)

**15. HTTP/3 (QUIC) via rquest**

Check rquest h3 support; enable on all profiles where target advertises
Alt-Svc. Real Chrome 133 aggressively uses h3.
Touch: `crates/net/src/lib.rs`.

**16. ALPS (Application-Layer Protocol Settings)**

Chrome H2 extension. Verify our BoringSSL build advertises it in
ClientHello and honors server-sent ALPS. Without ALPS, JA4 diff vs.
real Chrome is detectable.

**17. Early Hints (103) handling**

Real Chrome preconnects/prefetches based on 103 responses. Implement at
least the preconnect side (open H2/H3 stream to indicated origin).

**18. Per-origin TLS resumption**

Cache TLS session tickets per host in profile store. Resume on repeat
visits. Real Chrome does this aggressively.

### Phase 5 — Advanced surface (optional, ~2–3 weeks)

**19. Font enumeration parity** — `document.fonts.check()` returns true for
the OS font list keyed to `profile.os_name`. Match the ~250 Segoe/Arial/
Helvetica family that Chrome Windows reports.

**20. Battery API** — `navigator.getBattery()` returns plausible
`{charging, chargingTime, dischargingTime, level}` values from profile.

**21. WebRTC safety** — `RTCPeerConnection` ICE candidate enumeration
scrubbed to return only the public IP (or empty if no media requested).
Avoid leaking any private/v6/LAN addresses.

**22. `performance.memory`** — expose `{jsHeapSizeLimit, totalJSHeapSize,
usedJSHeapSize}` with realistic numbers. Chrome exposes this only to
same-origin pages, but many fingerprinters read it.

**23. CSS media-query defaults** — verify `prefers-color-scheme`,
`hover: hover`, `pointer: fine`, `prefers-reduced-motion: no-preference`,
`forced-colors: none` match profile claim.

### Validation

- **Unit**: snapshot-diff every `navigator.*`, `window.chrome.*`,
  `WebGLRenderingContext.*` surface against a capture from real headed
  Chrome 133 (committed fixture file per profile).
- **Fingerprint scorer**: run our engine against creepjs, fingerprintjs-pro
  demo, amiunique.org — record the score; target within 5 points of headed
  Chrome from the same machine.
- **End-to-end**: Canada Goose homepage + Hyatt homepage return real HTML,
  length > 50KB, no `/ips.js` or Akamai challenge markers.
- **Regression**: existing 8/8 L3 PASS suite stays green.

### What we do NOT need to build

- Actual pixel-accurate rendering for a human viewer (we're a crawler)
- Real GPU hardware (replay table covers known fingerprint probes)
- Real audio hardware (same)
- A visible window (we fake all screen/window dimensions)
- A real OS (we can claim any `platform` + `userAgentData` tuple)

### Estimated total

Phase 1: 1–2 weeks · Phase 2: 2–3 weeks · Phase 3: 1 week · Phase 4: 1 week
· Phase 5: optional. **Critical path to Kasada/Akamai-capable: ~4–5 weeks.**

Phase 2 is the single largest lever; if we had to ship one phase, it would
be behavior synthesis. Phase 1 is the prerequisite (a bot with perfect
behavior but empty `navigator.plugins` is still instantly flagged).

### 6.6 Per-Item Acceptance Matrix

Each item is "Done when" ALL three columns pass. New tests go in the
indicated file; run with `cargo test --release -p browser --test <file>
-- --ignored --test-threads=1 --nocapture`. Fixtures referenced below
live in §10 of this doc.

| # | Item | New/updated test | Assertion | Fixture |
|---|---|---|---|---|
| 1 | navigator.plugins / mimeTypes | `tests/fingerprint_scorers.rs::test_plugins_parity` | `navigator.plugins.length === 5` AND `JSON.stringify(pluginArrayToObject(navigator.plugins)) === FIXTURE` AND identity stable (`navigator.plugins === navigator.plugins`) | §10.1 `plugins.json` |
| 2 | window.chrome surface | `tests/chrome_deep.rs::test_chrome_api_surface` | `typeof chrome.csi === 'function'` AND `chrome.loadTimes().wasFetchedViaSpdy === true` AND `Object.keys(chrome).sort()` matches fixture | §10.2 `window_chrome.json` |
| 3 | WebGL parameter fidelity | `tests/fingerprint_scorers.rs::test_webgl_params_parity` | `getParameter(UNMASKED_RENDERER_WEBGL)` matches profile's `gpu_profile.unmasked_renderer` exactly; 30+ MAX_* params and shader-precision records match fixture | §10.3 `webgl_nvidia_rtx_3060.json` |
| 4 | Canvas rasterization consistency | `tests/fingerprint_scorers.rs::test_canvas_hash_in_real_chrome_distribution` | `sha256(canvas.toDataURL())` stable per-seed; ten distinct seeds produce ten distinct hashes; all fall inside the allowed-hash-set captured from real Chrome runs (at least 5 of 10 must MATCH) | §10.4 `canvas_allowed_hashes.txt` |
| 5 | AudioContext fingerprint | `tests/fingerprint_scorers.rs::test_audio_fingerprint` | FingerprintJS-style buffer-sum hash equals fixture for the canonical oscillator→compressor graph at `audio_seed` = default | §10.5 `audio_fingerprint.json` |
| 6 | Notification / visibility | `tests/leak_audit.rs::test_visibility_defaults` | `Notification.permission === 'default'`, `document.hidden === false`, `document.visibilityState === 'visible'` | — |
| 7 | Permissions.query | `tests/leak_audit.rs::test_permissions_query_defaults` | returns `{state:'default'}` for notifications; `'prompt'` for camera/microphone/geolocation/midi; `'granted'` for persistent-storage/background-sync | — |
| 8 | userAgentData + Client Hints | `tests/chrome_deep.rs::test_user_agent_data_highentropy` + `tests/anti_bot.rs::test_accept_ch_response_headers` | JS: `getHighEntropyValues` returns full set matching profile. HTTP: when server sends `Accept-CH: sec-ch-ua-platform-version, sec-ch-ua-arch, ...`, next request carries those headers with correct values | §10.6 `user_agent_data.json` |
| 9 | MediaDevices.enumerateDevices | `tests/w3c_apis.rs::test_media_devices_pre_permission` | returns array matching `profile.media_devices`; all `label` fields empty BEFORE any permission grant | — |
| 10 | Pre-nav / in-flight input events | new `tests/behavior_synthesis.rs::test_synthetic_events_dispatched` | during navigation, at least 30 `mousemove`, 5 `pointermove`, 1 `visibilitychange` events dispatch on document; seeded replay reproduces byte-identical timestamps | — |
| 11 | Post-load idle behavior | `tests/behavior_synthesis.rs::test_post_load_events_emit_for_3s` | after `load`, events continue for ≥2s; total event count before `/ftp:` probe ≥ 80 | — |
| 12 | Form interaction | `tests/behavior_synthesis.rs::test_form_submit_emits_keystrokes` | when `Page::submit_form` is used, a full `focus→keydown→beforeinput→input→keyup→change→blur` sequence fires per input with inter-event gaps in [60ms, 220ms] | — |
| 13 | Session persistence | new `tests/storage_persistence.rs::test_cookies_localstorage_across_runs` | run A writes `localStorage.setItem('x','y')` and a cookie; run B with same profile_id reads both without re-entering | — |
| 14 | Prior-browsing simulation | `tests/storage_persistence.rs::test_google_warmup_option` | when `profile.warmup_hosts = ["google.com"]`, Google cookies populate the jar before the target-site GET | — |
| 15 | HTTP/3 | `tests/tls_fingerprint_probe.rs::test_h3_negotiated_on_repeat` | on second visit with Alt-Svc advertised, connection negotiates h3 (verified via tls.peet.ws endpoint) | — |
| 16 | ALPS | `tests/tls_fingerprint_probe.rs::test_alps_advertised` | BoringSSL ClientHello contains ALPS extension; SETTINGS frame matches Chrome 133 capture | — |
| 17 | Early Hints (103) | `tests/anti_bot.rs::test_early_hints_preconnect` | on receiving `103 Early Hints` with `Link: <https://cdn.x.com>; rel=preconnect`, engine opens H2 stream to that origin | — |
| 18 | TLS resumption | `tests/tls_fingerprint_probe.rs::test_session_ticket_resume` | second visit to same origin resumes with ticket, saving 1 RTT; observable via TLS trace | — |
| 19 | Font enumeration | `tests/fingerprint_scorers.rs::test_font_list_matches_os` | `document.fonts.check("12px 'Segoe UI'")` = true on Windows profile; false on macOS profile; count matches fixture | §10.7 `fonts_windows.json` |
| 20 | Battery API | `tests/w3c_apis.rs::test_battery_api` | `navigator.getBattery()` resolves to `{charging, chargingTime, dischargingTime, level}` with values matching `profile.battery_*` | — |
| 21 | WebRTC safety | `tests/leak_audit.rs::test_webrtc_no_private_ip` | `RTCPeerConnection` ICE candidates include only public IP (or empty); no RFC 1918 / fc00:: / link-local addresses | — |
| 22 | performance.memory | `tests/chrome_deep.rs::test_performance_memory` | exposes `{jsHeapSizeLimit, totalJSHeapSize, usedJSHeapSize}` with realistic values from profile (not undefined) | — |
| 23 | CSS media-query defaults | `tests/chrome_compat.rs::test_media_query_defaults` | `matchMedia('(prefers-color-scheme: light)').matches` agrees with profile; same for `hover`, `pointer`, `prefers-reduced-motion`, `forced-colors` | — |

**Phase exit criteria** (must ALL pass before claiming a phase done):

- Every item's row above is green
- Full regression suite still green: see §9
- `tier0_kasada` rigorous suite still ≥ 6/8 (no regression), with §3 targets ideally flipping to PASS when their dependent phase completes
- One end-to-end smoke navigation on a non-blocker site (e.g. example.com) still returns real HTML

---

## 7. Existing Infrastructure Map — Don't Duplicate What's There

Before adding anything, search for it. The following already exist and are the canonical home for the listed concerns. Extensions should go here unless there's a specific architectural reason to add a new crate/file.

### 7.1 Workspace layout (`Cargo.toml:3-21`)

15 crates. Touch points for §6 work:

| Crate | Purpose | Phase-6 relevance |
|---|---|---|
| `crates/browser` | Top-level API — `Page`, navigation loop, challenge solver | entry point for all runs; where `navigate_*` methods live |
| `crates/stealth` | Fingerprint **profile** data model (StealthProfile, GpuProfile, presets) | Phase 1/3 — add new profile fields here |
| `crates/js_runtime` | V8 embedding, JS bootstraps, ops | Phase 1/2 — JS-surface changes live in `src/js/*.js` + `src/extensions/*_ext.rs` |
| `crates/canvas` | 2D rasterizer (tiny-skia) | Phase 1 item 4 |
| `crates/net` | `rquest`-based HTTP stack, TLS impersonation, header assembly | Phase 4 (h3, ALPS, Early Hints) + Phase 1 item 8 (HTTP Client Hints) |
| `crates/dom` | Arena-allocated DOM, `NodeId` (u32 handle) | touch only if a §6 item needs a new DOM node type |
| `crates/event_loop` | microtask/macrotask scheduling | Phase 2 — event-synthesis scheduler integrates here |
| `crates/workers` | Web Worker isolates | Workers need the same stealth profile — pre-existing infra (§4.A) |
| `crates/protocol` | CDP drop-in compatibility layer | out of scope for §6 |
| `crates/html_parser` · `crates/css_parser` · `crates/css_selectors` · `crates/css_values` · `crates/css_cascade` · `crates/layout` | Content pipeline | out of scope for §6 |

### 7.2 The stealth profile (`crates/stealth/src/profile.rs`)

`StealthProfile` struct (186 lines) with 40+ fields and a `validate()` invariant (line 101). **Every new field MUST have a matching validate() rule** — inconsistent profiles (e.g., UA says Windows but platform says MacIntel) defeat the stealth goal.

Existing field groups: Identity · Hardware · GPU/WebGL (references `GpuProfile`) · Locale · Network · Plugins · Fingerprint seeds · Media features · Window dimensions · Proxy · Media devices. When a §6 item says "add X to profile", this is the file.

Cross-referenced constants:
- `canvas_seed: u64` — already present (`profile.rs:72`). Used for deterministic canvas rasterization (Phase 1 item 4).
- `audio_seed: u64` — already present (`profile.rs:73`). Use for Phase 1 item 5.

Presets: `crates/stealth/src/presets.rs` (404 lines). Nine pre-built profiles: `chrome_130_windows()`, `chrome_130_macos()`, `chrome_130_linux()`, `chrome_130_ru()`, `chrome_130_cn()`, `chrome_130_de()`, `chrome_130_jp()`, `with_locale()`, `random_desktop()`. **Any new StealthProfile field requires updating every preset** — `validate()` will fail otherwise.

### 7.3 The GPU catalog (`crates/stealth/src/gpu.rs`, 337 lines)

`GpuProfile` struct with per-vendor real Chrome WebGL output: extensions list, integer param values (MAX_TEXTURE_SIZE etc.), shader-precision-format records, unmasked vendor/renderer strings. Catalog entries include `nvidia_rtx_3060_windows()` and similar. Phase 1 item 3 extends THIS file; don't add GPU data elsewhere.

### 7.4 JS bootstraps (`crates/js_runtime/src/js/*.js`, 9606 total lines)

Load order is determined by `js_runtime` (check `runtime.rs`). Each file is `((globalThis) => { ... })(globalThis);` pattern.

| File | Size | Purpose | Phase-6 relevance |
|---|---|---|---|
| `stealth_bootstrap.js` | 52 | Exposes `_maskFunction` / `_maskAsNative` helpers globally | **USE THESE** for any new native-code-masking — they are the project's anti-detection helpers. Function without `[native code]` in toString() is a hard tell |
| `dom_bootstrap.js` | 1736 | DOM API overrides + navigator shims | Phase 1 items 1, 6, 7, 9 — most plugin/permissions/media-device work lands here |
| `window_bootstrap.js` | 3637 | window.* surface, Notification, chrome.*, matchMedia | Phase 1 items 2, 6, 7, 8, 23 |
| `canvas_bootstrap.js` | 884 | CanvasRenderingContext2D + WebGL shim | Phase 1 items 3, 4 |
| `fetch_bootstrap.js` | 306 | window.fetch wrapping (accept: */* vs text/html logic) | updated already in commit `ec5a9c7` |
| `event_bootstrap.js` | 404 | EventTarget / Event construction | Phase 2 — new synthetic events registered here |
| `input_bootstrap.js` | 18 | input-event helpers (thin) | Phase 2 extends this |
| `interfaces_bootstrap.js` | 85 | Interface prototype chains | touch only if adding new interfaces |
| `worker_bootstrap.js` | 260 | Web Worker bootstrap (inherits profile) | pre-existing, see §4.A |
| `timer_bootstrap.js` | 79 | setTimeout/setInterval native masking | stable |
| `streams_bootstrap.js` | 594 | ReadableStream / WritableStream | stable |
| `structured_clone.js` | 398 | structuredClone() | stable |
| `console_bootstrap.js` | 39 | console.* native masking | stable |
| `sse_bootstrap.js` | 122 | EventSource | stable |
| `instances_bootstrap.js` | 18 | Preserves Window instance identity | stable |
| `cleanup_bootstrap.js` | 35 | Nav-transition cleanup | **GOTCHA**: was wiping `__pendingNavigation` too early (fixed in commit `48267ab`); be careful adding new globals that need to survive across nav |

**Rule**: anything that adds a function to the globalThis scope MUST pass `_maskAsNative(globalThis, 'name1', 'name2', ...)` at the end of its bootstrap, else `Function.prototype.toString.call(fn)` leaks the lack of `[native code]`.

### 7.5 Rust-side ops (`crates/js_runtime/src/extensions/*.rs`)

Each `*_ext.rs` file declares a `deno_core::extension!` block exposing `#[op2]`-annotated Rust functions to JS. Existing extensions:

```
audio_ext.rs canvas_ext.rs console_ext.rs crypto_ext.rs dom_ext.rs
fetch_ext.rs input_ext.rs layout_ext.rs sse_ext.rs stealth_ext.rs
timer_ext.rs webgl_ext.rs websocket_ext.rs worker_ext.rs
```

The sole JS-reads-profile bridge is **`op_get_profile_value(key: &str) -> String`** in `stealth_ext.rs:17`. Every new StealthProfile field needs:

1. A new match arm in `op_get_profile_value` returning the stringified value
2. A JS reader: `Deno.core.ops.op_get_profile_value('your_key')` in the relevant bootstrap, then parse if non-string

Don't invent new profile-read ops — extending `op_get_profile_value` keeps the JS↔Rust bridge narrow and testable.

### 7.6 Test suite inventory (`crates/browser/tests/*.rs`)

34 test files. Relevant for §6 work:

| File | Purpose |
|---|---|
| `tier0_kasada.rs` | Rigorous 8-site PASS/FAIL scoreboard; Kasada probes; `kasada_ips_deep_instrumentation` harness (line 2116). **This is the suite that § 1 refers to — keep it ≥ 6/8.** |
| `fingerprint_scorers.rs` | FingerprintJS / creepjs-style probes — plugins, WebGL, canvas, audio fingerprint checks |
| `chrome_deep.rs` | Deep Chrome-surface conformance (chrome.csi, loadTimes, userAgentData full-entropy) |
| `chrome_compat.rs` | Chrome compatibility — matchMedia, CSS properties, DOM APIs |
| `leak_audit.rs` | Automation-marker leak checks (navigator.webdriver false, Notification.permission='default', no WebRTC IP leak) |
| `anti_bot.rs` · `anti_bot_sites.rs` | Real-site smoke tests (legacy pass-counting semantics; prefer `tier0_kasada` for L3 rigour) |
| `tls_fingerprint_probe.rs` | JA3/JA4, H2 SETTINGS, ALPN, ALPS, h3 checks (tls.peet.ws endpoint) |
| `w3c_apis.rs` | W3C spec compliance (Battery, MediaDevices, etc.) |
| `storage_persistence.rs` | Cookies / localStorage / IndexedDB across-run persistence |
| `webgl_render.rs` | WebGL real-render tests |
| `worker_page_integration.rs` | Worker isolates + StealthProfile inheritance |
| `browser_comparison.rs` | External-tool benchmarks (Chrome, Puppeteer, Camoufox, Lightpanda) — **use `--release`** |
| `adidas_*.rs` · `wildberries_*.rs` · `qrator_diag.rs` · `blocker_rigorous_probe.rs` | Per-site deep-dives (historical but useful as reference for pattern when adding new site coverage) |
| `iframe_isolation.rs` · `computed_style.rs` · `deep_path_validation.rs` · `e2e_browser.rs` · `integration.rs` · `mutation_observer.rs` · `navigation_primitives.rs` · `public_detection.rs` · `real_world.rs` · `supporting_apis.rs` · `challenge_solver.rs` · `debug_blocked.rs` · `debug_fixes.rs` | Targeted feature coverage |

**Convention**: network-dependent tests are `#[ignore]` (run only with `--ignored`); offline tests run by default.

### 7.7 Commands cheat-sheet

```bash
# Every "mandatory before pushing" gate
cargo test --workspace -- --test-threads=1
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check

# Rigorous Kasada suite (network, slow)
cargo test --release -p browser --test tier0_kasada -- --ignored --test-threads=1 --nocapture

# One specific diagnostic
cargo test --release -p browser --test tier0_kasada kasada_ips_deep_instrumentation -- --ignored --test-threads=1 --nocapture

# Fingerprint-scorer suite
cargo test --release -p browser --test fingerprint_scorers -- --test-threads=1

# Comparison benchmarks (README's top table)
cargo test --release -p browser --test browser_comparison -- --ignored --test-threads=1 --nocapture
```

---

## 8. Reading Order — Related Docs

Read in this order when starting:

| Step | Doc | Why |
|---|---|---|
| 1 | `README.md` | Top-level pitch + numbers, positions the project |
| 2 | `CLAUDE.md` | Project conventions (single-threaded tests, license, V8 constraint) |
| 3 | `docs/ARCHITECTURE.md` | How the 15 crates fit together |
| 4 | `docs/STEALTH.md` | Stealth design philosophy |
| 5 | `docs/STEALTH_HTTP_CLIENT.md` | rquest/BoringSSL/TLS impersonation details |
| 6 | `docs/JS_RUNTIME.md` | V8 embedding, ops, bootstrap-loading order |
| 7 | `docs/DOM.md` · `docs/CANVAS.md` · `docs/EVENT_LOOP.md` · `docs/LAYOUT.md` · `docs/NETWORKING.md` · `docs/WORKERS.md` | Component deep-dives (skim; return when you touch one) |
| 8 | `docs/CSS_PARSER.md` · `CSS_SELECTORS.md` · `CSS_VALUES.md` · `CSS_CASCADE.md` | CSS stack (unlikely to touch for §6) |
| 9 | `docs/ANTIBOT_RESEARCH_2026.md` | Survey of anti-bot vendors and their signals (2026-state) |
| 10 | `docs/CAPABILITY_GAPS_2026.md` | Known gaps, complements §6 with alternate framing |
| 11 | `docs/GAPS.md` · `docs/NEXT_STEPS.md` · `docs/ROADMAP.md` | Running backlog — cross-check against §6 to avoid duplicated work |
| 12 | `docs/TIER0_KASADA_RESULTS.md` · `docs/SESSION_2026_04_10_RESULTS.md` | Historical pass/fail ground-truth |
| 13 | `docs/universal_engine/README.md` + `01_architecture_principle.md`–`11_session_2026_04_17_state.md` | Sequential session history; `11_` is most recent and overlaps with this handoff |
| 14 | `docs/universal_engine/plans/fingerprint_polish.md` · `quick_wins_russian_sites.md` · `wildberries_wbaas.md` | Existing sub-plans (may contain Phase-1-adjacent notes) |
| 15 | `docs/universal_engine/site_debugging/*.md` | Past site-specific debugging logs — `adidas_akamai_bmp_v3.md` is especially useful for Phase 2 (behavioral signals are what Akamai BMP scores) |
| 16 | `docs/akamai_sensor_analysis/` and `docs/kasada_ips_analysis/` | Reverse-engineering notes; skim before attacking §6 Phase 1 item 5 (audio) and item 3 (WebGL) |
| 17 | `docs/WILDBERRIES.md` | Solver pattern reference — how the "6/8 WIN" was achieved for Wildberries |
| 18 | `docs/BROWSER_COMPARISON.md` | Methodology for the external-tool benchmarks |

If any doc is obviously stale against the current code, fix it as part of your change — don't leave a known-wrong doc in place.

---

## 9. CI Regression Gate — Must-Pass Before Claiming "Done"

Any phase, sub-item, or PR must keep all of these green. Run in this order; stop at first failure.

```bash
# Gate 1 — format + lint (fast, ~seconds)
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings

# Gate 2 — unit + integration suite (medium, ~3–5 min)
cargo test --workspace -- --test-threads=1

# Gate 3 — fingerprint-scorer suite (fast, offline; must stay green)
cargo test --release -p browser --test fingerprint_scorers -- --test-threads=1
cargo test --release -p browser --test leak_audit        -- --test-threads=1
cargo test --release -p browser --test chrome_deep       -- --test-threads=1
cargo test --release -p browser --test chrome_compat     -- --test-threads=1

# Gate 4 — rigorous anti-bot suite (network, slow, ~5–10 min)
# THIS IS THE ≥ 6/8 GATE THAT PROTECTS §1.
cargo test --release -p browser --test tier0_kasada \
    -- --ignored --test-threads=1 --nocapture 2>&1 | tee /tmp/tier0_after.log
# Grep the log for pass/fail counts; compare to the known 6/8 baseline.

# Gate 5 — benchmarks (optional per-phase, required per-release)
cargo test --release -p browser --test browser_comparison \
    -- --ignored --test-threads=1 --nocapture
```

A PR that would drop the tier0_kasada count below 6/8 must include a written justification and an explicit Yury approval. A PR that drops any of Gate 1–3 must never merge.

---

## 10. Reference Fixtures — Expected Headed-Chrome Output

These are the canonical snapshots against which §6 items assert parity. Check them into `crates/browser/tests/fixtures/headed_chrome_133/` as individual files. Capture procedure in §11.

If you discover a fixture is wrong, update it from a fresh capture (§11) and note in the PR: "fixture regenerated from Chrome <ver> on <OS> <date>". **Do not edit by hand** — every value must be machine-captured from real headed Chrome or it defeats the point.

### 10.1 `plugins.json` — navigator.plugins canonical shape

Captured from real headed Chrome 133 on Windows 11, fresh profile:

```json
{
  "length": 5,
  "plugins": [
    {
      "name": "PDF Viewer",
      "description": "Portable Document Format",
      "filename": "internal-pdf-viewer",
      "mimeTypes": [
        {"type":"application/pdf","suffixes":"pdf","description":"Portable Document Format"},
        {"type":"text/pdf","suffixes":"pdf","description":"Portable Document Format"}
      ]
    },
    {
      "name": "Chrome PDF Viewer",
      "description": "Portable Document Format",
      "filename": "internal-pdf-viewer",
      "mimeTypes": [
        {"type":"application/pdf","suffixes":"pdf","description":"Portable Document Format"},
        {"type":"text/pdf","suffixes":"pdf","description":"Portable Document Format"}
      ]
    },
    {
      "name": "Chromium PDF Viewer",
      "description": "Portable Document Format",
      "filename": "internal-pdf-viewer",
      "mimeTypes": [
        {"type":"application/pdf","suffixes":"pdf","description":"Portable Document Format"},
        {"type":"text/pdf","suffixes":"pdf","description":"Portable Document Format"}
      ]
    },
    {
      "name": "Microsoft Edge PDF Viewer",
      "description": "Portable Document Format",
      "filename": "internal-pdf-viewer",
      "mimeTypes": [
        {"type":"application/pdf","suffixes":"pdf","description":"Portable Document Format"},
        {"type":"text/pdf","suffixes":"pdf","description":"Portable Document Format"}
      ]
    },
    {
      "name": "WebKit built-in PDF",
      "description": "Portable Document Format",
      "filename": "internal-pdf-viewer",
      "mimeTypes": [
        {"type":"application/pdf","suffixes":"pdf","description":"Portable Document Format"},
        {"type":"text/pdf","suffixes":"pdf","description":"Portable Document Format"}
      ]
    }
  ],
  "identity_stable": true,
  "mimeTypesLength": 2
}
```

Also required: `navigator.plugins === navigator.plugins` (same reference), `navigator.plugins.item(0) === navigator.plugins[0]`, `navigator.plugins.namedItem('PDF Viewer') === navigator.plugins[0]`, iteration with `for..of` works. These are all spec-mandated.

### 10.2 `window_chrome.json` — window.chrome surface

```json
{
  "keys_sorted": ["app","csi","loadTimes","webstore"],
  "app": {
    "InstallState": {"DISABLED":"disabled","INSTALLED":"installed","NOT_INSTALLED":"not_installed"},
    "RunningState": {"CANNOT_RUN":"cannot_run","READY_TO_RUN":"ready_to_run","RUNNING":"running"},
    "isInstalled": false,
    "getDetails_returns": null,
    "getIsInstalled_returns": false
  },
  "csi_returns_keys": ["onloadT","pageT","startE","tran"],
  "loadTimes_returns_keys": [
    "requestTime","startLoadTime","commitLoadTime","finishDocumentLoadTime",
    "finishLoadTime","firstPaintTime","firstPaintAfterLoadTime",
    "navigationType","wasFetchedViaSpdy","wasNpnNegotiated",
    "npnNegotiatedProtocol","wasAlternateProtocolAvailable","connectionInfo"
  ],
  "loadTimes_navigationType_enum": ["Other","Reload","Back_Forward","LinkClicked","FormSubmitted","Start"],
  "loadTimes_connectionInfo_for_h2": "h2",
  "loadTimes_wasFetchedViaSpdy_for_h2": true
}
```

### 10.3 `webgl_nvidia_rtx_3060.json` — WebGL canonical params

Captured on a real NVIDIA RTX 3060 + Windows 11 + Chrome 133 (keep in sync with `crates/stealth/src/gpu.rs::nvidia_rtx_3060_windows`):

```json
{
  "UNMASKED_VENDOR_WEBGL":   "Google Inc. (NVIDIA)",
  "UNMASKED_RENDERER_WEBGL": "ANGLE (NVIDIA, NVIDIA GeForce RTX 3060 Direct3D11 vs_5_0 ps_5_0, D3D11)",
  "VENDOR":   "WebKit",
  "RENDERER": "WebKit WebGL",
  "VERSION":  "WebGL 1.0 (OpenGL ES 2.0 Chromium)",
  "SHADING_LANGUAGE_VERSION": "WebGL GLSL ES 1.0 (OpenGL ES GLSL ES 1.0 Chromium)",
  "MAX_TEXTURE_SIZE": 16384,
  "MAX_CUBE_MAP_TEXTURE_SIZE": 16384,
  "MAX_RENDERBUFFER_SIZE": 16384,
  "MAX_VIEWPORT_DIMS": [32767, 32767],
  "MAX_VERTEX_ATTRIBS": 16,
  "MAX_VERTEX_UNIFORM_VECTORS": 4095,
  "MAX_VARYING_VECTORS": 30,
  "MAX_COMBINED_TEXTURE_IMAGE_UNITS": 32,
  "MAX_VERTEX_TEXTURE_IMAGE_UNITS": 16,
  "MAX_TEXTURE_IMAGE_UNITS": 16,
  "MAX_FRAGMENT_UNIFORM_VECTORS": 1024,
  "ALIASED_LINE_WIDTH_RANGE":    [1, 1],
  "ALIASED_POINT_SIZE_RANGE":    [1, 1024],
  "RED_BITS":8,"GREEN_BITS":8,"BLUE_BITS":8,"ALPHA_BITS":8,
  "DEPTH_BITS":24,"STENCIL_BITS":0,
  "supported_extensions": [
    "ANGLE_instanced_arrays","EXT_blend_minmax","EXT_color_buffer_half_float",
    "EXT_disjoint_timer_query","EXT_float_blend","EXT_frag_depth",
    "EXT_shader_texture_lod","EXT_texture_compression_bptc","EXT_texture_compression_rgtc",
    "EXT_texture_filter_anisotropic","EXT_sRGB","KHR_parallel_shader_compile",
    "OES_element_index_uint","OES_fbo_render_mipmap","OES_standard_derivatives",
    "OES_texture_float","OES_texture_float_linear","OES_texture_half_float",
    "OES_texture_half_float_linear","OES_vertex_array_object",
    "WEBGL_color_buffer_float","WEBGL_compressed_texture_s3tc",
    "WEBGL_compressed_texture_s3tc_srgb","WEBGL_debug_renderer_info",
    "WEBGL_debug_shaders","WEBGL_depth_texture","WEBGL_draw_buffers",
    "WEBGL_lose_context","WEBGL_multi_draw"
  ],
  "shader_precision": {
    "VERTEX_SHADER":   {"LOW_FLOAT":{"rangeMin":127,"rangeMax":127,"precision":23},
                        "MEDIUM_FLOAT":{"rangeMin":127,"rangeMax":127,"precision":23},
                        "HIGH_FLOAT":{"rangeMin":127,"rangeMax":127,"precision":23}},
    "FRAGMENT_SHADER": {"LOW_FLOAT":{"rangeMin":127,"rangeMax":127,"precision":23},
                        "MEDIUM_FLOAT":{"rangeMin":127,"rangeMax":127,"precision":23},
                        "HIGH_FLOAT":{"rangeMin":127,"rangeMax":127,"precision":23}}
  }
}
```

**These values are approximations pending a fresh capture (§11).** Ship the item with a NEWLY CAPTURED fixture, not this one — these are here only to seed the file shape.

### 10.4 `canvas_allowed_hashes.txt` — sample

After capturing 20+ headed-Chrome canvas hashes across real machines (different GPUs, OSes, Chrome builds), commit them as one hash per line. Example:

```
# canvas.toDataURL() sha256 values from real Chrome 133 runs, N=20+
# format: <sha256>  <gpu>  <os>  <chrome_ver>
f4a1...b29c  NVIDIA-RTX-3060  Windows-11  133.0.6943.54
6b12...7e03  Apple-M2         macOS-15    133.0.6943.98
...
```

Phase 1 item 4's test is: engine's generated hash falls within or close to the distribution of these captured hashes (at least one match per profile).

### 10.5 `audio_fingerprint.json`

```json
{
  "graph": "OscillatorNode(type=triangle, freq=10000) -> DynamicsCompressorNode(threshold=-50, knee=40, ratio=12, attack=0, release=0.25) -> OfflineAudioContext(1, 44100, 44100).startRendering()",
  "sum_magnitudes_range": [124.04347527516074, 124.04347823292685],
  "comment": "FingerprintJS canonical audio probe. Output sum falls in a narrow per-Chrome-build range. Update range when you recapture."
}
```

### 10.6 `user_agent_data.json`

```json
{
  "brands": [
    {"brand":"Not-A.Brand","version":"99"},
    {"brand":"Chromium","version":"133"},
    {"brand":"Google Chrome","version":"133"}
  ],
  "mobile": false,
  "platform": "Windows",
  "highEntropy": {
    "architecture": "x86",
    "bitness": "64",
    "model": "",
    "platformVersion": "15.0.0",
    "uaFullVersion": "133.0.6943.54",
    "fullVersionList": [
      {"brand":"Not-A.Brand","version":"99.0.0.0"},
      {"brand":"Chromium","version":"133.0.6943.54"},
      {"brand":"Google Chrome","version":"133.0.6943.54"}
    ],
    "wow64": false
  },
  "accept_ch_response_headers": {
    "sec-ch-ua":             "\"Not-A.Brand\";v=\"99\", \"Chromium\";v=\"133\", \"Google Chrome\";v=\"133\"",
    "sec-ch-ua-mobile":      "?0",
    "sec-ch-ua-platform":    "\"Windows\"",
    "sec-ch-ua-platform-version": "\"15.0.0\"",
    "sec-ch-ua-arch":        "\"x86\"",
    "sec-ch-ua-bitness":     "\"64\"",
    "sec-ch-ua-full-version-list": "\"Not-A.Brand\";v=\"99.0.0.0\", \"Chromium\";v=\"133.0.6943.54\", \"Google Chrome\";v=\"133.0.6943.54\"",
    "sec-ch-ua-model":       "\"\"",
    "sec-ch-ua-wow64":       "?0"
  }
}
```

**GREASE ordering note**: Chrome shuffles the three brand entries per-startup. Assertions must accept any permutation; don't pin order.

### 10.7 `fonts_windows.json`

The ~250-entry Windows font list Chrome reports. Capture via §11 step 3. Minimum-present fonts for assertion (if the full list drifts per Windows update):

```json
{
  "must_be_present": [
    "Arial","Arial Black","Calibri","Cambria","Candara","Comic Sans MS",
    "Consolas","Constantia","Corbel","Courier New","Ebrima","Franklin Gothic Medium",
    "Gabriola","Georgia","Impact","Ink Free","Javanese Text","Leelawadee UI",
    "Lucida Console","Lucida Sans Unicode","Malgun Gothic","Microsoft Himalaya",
    "Microsoft JhengHei","Microsoft New Tai Lue","Microsoft PhagsPa",
    "Microsoft Sans Serif","Microsoft Tai Le","Microsoft YaHei","Microsoft Yi Baiti",
    "MingLiU-ExtB","Mongolian Baiti","MS Gothic","MS PGothic","MS UI Gothic",
    "MV Boli","Myanmar Text","Nirmala UI","Palatino Linotype","Segoe MDL2 Assets",
    "Segoe Print","Segoe Script","Segoe UI","Segoe UI Emoji","Segoe UI Historic",
    "Segoe UI Symbol","SimSun","Sitka","Sylfaen","Symbol","Tahoma","Times New Roman",
    "Trebuchet MS","Verdana","Webdings","Wingdings","Yu Gothic"
  ]
}
```

---

## 11. How to (Re)Capture Fixtures From Headed Chrome

Run this once on a real Windows/macOS machine with a fresh Chrome profile. Commit the outputs to `crates/browser/tests/fixtures/headed_chrome_133/`.

### Step 1: headed Playwright driven by a harness

Save as `docs/universal_engine/site_debugging/capture_headed_fingerprint.js`:

```js
import { chromium } from 'playwright';
import fs from 'fs';

const browser = await chromium.launch({
  headless: false,                                    // KEY: must be false
  channel: 'chrome',                                  // Use real Chrome, not Chromium
  args: ['--disable-blink-features=AutomationControlled'],
});
const ctx = await browser.newContext({ viewport: null });
const page = await ctx.newPage();

await page.goto('about:blank');
await page.waitForTimeout(1000);

const snapshot = await page.evaluate(async () => {
  // Plugins
  const pluginArrayToObject = (pa) => ({
    length: pa.length,
    plugins: [...pa].map(p => ({
      name: p.name, description: p.description, filename: p.filename,
      mimeTypes: [...p].map(mt => ({
        type: mt.type, suffixes: mt.suffixes, description: mt.description
      })),
    })),
  });

  // window.chrome
  const chromeSurface = {
    keys_sorted: Object.keys(window.chrome || {}).sort(),
    app: window.chrome && window.chrome.app ? {
      InstallState: window.chrome.app.InstallState,
      RunningState: window.chrome.app.RunningState,
      isInstalled:  window.chrome.app.isInstalled,
    } : null,
    csi_returns_keys: window.chrome && window.chrome.csi ? Object.keys(window.chrome.csi()) : [],
    loadTimes_returns_keys: window.chrome && window.chrome.loadTimes ? Object.keys(window.chrome.loadTimes()) : [],
    loadTimes_sample: window.chrome && window.chrome.loadTimes ? window.chrome.loadTimes() : null,
  };

  // WebGL
  const canvas = document.createElement('canvas');
  const gl = canvas.getContext('webgl');
  const getP = (k) => { try { return gl.getParameter(gl[k]); } catch { return null; } };
  const dri = gl.getExtension('WEBGL_debug_renderer_info');
  const webgl = {
    UNMASKED_VENDOR_WEBGL:   gl.getParameter(dri.UNMASKED_VENDOR_WEBGL),
    UNMASKED_RENDERER_WEBGL: gl.getParameter(dri.UNMASKED_RENDERER_WEBGL),
    VENDOR: getP('VENDOR'), RENDERER: getP('RENDERER'),
    VERSION: getP('VERSION'),
    SHADING_LANGUAGE_VERSION: getP('SHADING_LANGUAGE_VERSION'),
    MAX_TEXTURE_SIZE: getP('MAX_TEXTURE_SIZE'),
    MAX_CUBE_MAP_TEXTURE_SIZE: getP('MAX_CUBE_MAP_TEXTURE_SIZE'),
    MAX_RENDERBUFFER_SIZE: getP('MAX_RENDERBUFFER_SIZE'),
    MAX_VIEWPORT_DIMS: Array.from(getP('MAX_VIEWPORT_DIMS') || []),
    MAX_VERTEX_ATTRIBS: getP('MAX_VERTEX_ATTRIBS'),
    MAX_VERTEX_UNIFORM_VECTORS: getP('MAX_VERTEX_UNIFORM_VECTORS'),
    MAX_VARYING_VECTORS: getP('MAX_VARYING_VECTORS'),
    MAX_COMBINED_TEXTURE_IMAGE_UNITS: getP('MAX_COMBINED_TEXTURE_IMAGE_UNITS'),
    MAX_VERTEX_TEXTURE_IMAGE_UNITS:   getP('MAX_VERTEX_TEXTURE_IMAGE_UNITS'),
    MAX_TEXTURE_IMAGE_UNITS:          getP('MAX_TEXTURE_IMAGE_UNITS'),
    MAX_FRAGMENT_UNIFORM_VECTORS:     getP('MAX_FRAGMENT_UNIFORM_VECTORS'),
    ALIASED_LINE_WIDTH_RANGE: Array.from(getP('ALIASED_LINE_WIDTH_RANGE') || []),
    ALIASED_POINT_SIZE_RANGE: Array.from(getP('ALIASED_POINT_SIZE_RANGE') || []),
    RED_BITS:getP('RED_BITS'),GREEN_BITS:getP('GREEN_BITS'),
    BLUE_BITS:getP('BLUE_BITS'),ALPHA_BITS:getP('ALPHA_BITS'),
    DEPTH_BITS:getP('DEPTH_BITS'),STENCIL_BITS:getP('STENCIL_BITS'),
    supported_extensions: gl.getSupportedExtensions().sort(),
    shader_precision: {
      VERTEX_SHADER: {
        LOW_FLOAT:    gl.getShaderPrecisionFormat(gl.VERTEX_SHADER,   gl.LOW_FLOAT),
        MEDIUM_FLOAT: gl.getShaderPrecisionFormat(gl.VERTEX_SHADER,   gl.MEDIUM_FLOAT),
        HIGH_FLOAT:   gl.getShaderPrecisionFormat(gl.VERTEX_SHADER,   gl.HIGH_FLOAT),
      },
      FRAGMENT_SHADER: {
        LOW_FLOAT:    gl.getShaderPrecisionFormat(gl.FRAGMENT_SHADER, gl.LOW_FLOAT),
        MEDIUM_FLOAT: gl.getShaderPrecisionFormat(gl.FRAGMENT_SHADER, gl.MEDIUM_FLOAT),
        HIGH_FLOAT:   gl.getShaderPrecisionFormat(gl.FRAGMENT_SHADER, gl.HIGH_FLOAT),
      },
    },
  };

  // Permissions
  const perms = {};
  for (const name of ['notifications','geolocation','camera','microphone','midi','persistent-storage','background-sync']) {
    try { perms[name] = (await navigator.permissions.query({name})).state; } catch (e) { perms[name] = `ERR:${e.message}`; }
  }

  // UA data (high entropy)
  let uad = null;
  try {
    uad = {
      brands: navigator.userAgentData.brands,
      mobile: navigator.userAgentData.mobile,
      platform: navigator.userAgentData.platform,
      highEntropy: await navigator.userAgentData.getHighEntropyValues([
        'architecture','bitness','model','platformVersion',
        'uaFullVersion','fullVersionList','wow64'
      ]),
    };
  } catch (e) { uad = `ERR:${e.message}`; }

  // Audio fingerprint
  let audio = null;
  try {
    const ctx = new OfflineAudioContext(1, 44100, 44100);
    const osc = ctx.createOscillator();
    osc.type = 'triangle'; osc.frequency.value = 10000;
    const comp = ctx.createDynamicsCompressor();
    comp.threshold.value = -50; comp.knee.value = 40; comp.ratio.value = 12;
    comp.attack.value = 0; comp.release.value = 0.25;
    osc.connect(comp); comp.connect(ctx.destination); osc.start(0);
    const buf = await ctx.startRendering();
    let sum = 0;
    const data = buf.getChannelData(0);
    for (let i = 4500; i < 5000; i++) sum += Math.abs(data[i]);
    audio = { sum_magnitudes: sum };
  } catch (e) { audio = `ERR:${e.message}`; }

  // Canvas hash
  let canvasHash = null;
  try {
    const c = document.createElement('canvas');
    c.width = 200; c.height = 60;
    const cx = c.getContext('2d');
    cx.textBaseline = 'top';
    cx.font = '14px \\'Arial\\'';
    cx.fillStyle = '#f60'; cx.fillRect(125,1,62,20);
    cx.fillStyle = '#069'; cx.fillText('browser_oxide fingerprint 🕵️', 2, 15);
    cx.fillStyle = 'rgba(102,204,0,0.7)'; cx.fillText('browser_oxide fingerprint 🕵️', 4, 17);
    const dataUrl = c.toDataURL();
    const enc = new TextEncoder().encode(dataUrl);
    const hashBuf = await crypto.subtle.digest('SHA-256', enc);
    canvasHash = Array.from(new Uint8Array(hashBuf)).map(b => b.toString(16).padStart(2,'0')).join('');
  } catch (e) { canvasHash = `ERR:${e.message}`; }

  // Fonts
  let fonts = [];
  try {
    for (const f of document.fonts) fonts.push(f.family);
    fonts = [...new Set(fonts)].sort();
  } catch (e) { fonts = `ERR:${e.message}`; }

  return {
    plugins: pluginArrayToObject(navigator.plugins),
    mimeTypesLength: navigator.mimeTypes.length,
    identity_stable: navigator.plugins === navigator.plugins,
    chrome: chromeSurface,
    webgl,
    permissions: perms,
    userAgentData: uad,
    audio,
    canvasHash,
    fonts,
    navigator: {
      hardwareConcurrency: navigator.hardwareConcurrency,
      deviceMemory: navigator.deviceMemory,
      maxTouchPoints: navigator.maxTouchPoints,
      languages: navigator.languages,
      platform: navigator.platform,
      vendor: navigator.vendor,
      userAgent: navigator.userAgent,
      webdriver: navigator.webdriver,
    },
    document: {
      hidden: document.hidden,
      visibilityState: document.visibilityState,
    },
    notification_permission: Notification.permission,
    performance_memory: (performance && performance.memory) ? {
      jsHeapSizeLimit: performance.memory.jsHeapSizeLimit,
      totalJSHeapSize: performance.memory.totalJSHeapSize,
      usedJSHeapSize:  performance.memory.usedJSHeapSize,
    } : null,
    screen: {
      width: screen.width, height: screen.height,
      availWidth: screen.availWidth, availHeight: screen.availHeight,
      colorDepth: screen.colorDepth, pixelDepth: screen.pixelDepth,
    },
    window: {
      innerWidth: window.innerWidth, innerHeight: window.innerHeight,
      outerWidth: window.outerWidth, outerHeight: window.outerHeight,
      devicePixelRatio: window.devicePixelRatio,
    },
  };
});

fs.writeFileSync('headed_chrome_snapshot.json', JSON.stringify(snapshot, null, 2));
console.log('Wrote headed_chrome_snapshot.json');
await browser.close();
```

### Step 2: run it

```bash
cd docs/universal_engine/site_debugging
node capture_headed_fingerprint.js
# outputs headed_chrome_snapshot.json (keep in /tmp; do NOT commit raw output)
```

Run twice; verify stability (brand-list GREASE ordering will differ, everything else should be byte-identical).

### Step 3: split into fixture files

Cut the fields of `headed_chrome_snapshot.json` into the per-surface fixtures listed in §10 and commit to `crates/browser/tests/fixtures/headed_chrome_133/`:

```
plugins.json        <- snapshot.plugins + identity_stable + mimeTypesLength
window_chrome.json  <- snapshot.chrome
webgl_<gpu>.json    <- snapshot.webgl (name by actual GPU captured)
audio_fingerprint.json   <- snapshot.audio + the graph description
user_agent_data.json     <- snapshot.userAgentData + expected accept-CH response
fonts_<os>.json          <- snapshot.fonts
navigator_defaults.json  <- snapshot.navigator + document + notification_permission + screen + window
performance_memory.json  <- snapshot.performance_memory (ranges, not exact values)
```

### Step 4: capture canvas hashes at scale

Ask 5–10 colleagues with different GPUs to run `capture_headed_fingerprint.js` and paste their `canvasHash` into `canvas_allowed_hashes.txt` (one per line, with a comment listing GPU + OS + Chrome version). Target N ≥ 20 to build a reasonable reference distribution.

### Step 5: keep fixtures fresh

When Chrome major version bumps (monthly), redo Steps 1–4. Put the capture date in a `FIXTURE_META.md` file next to the fixtures so tests can warn when stale (>90 days).

---

## 12. Contact / Escalation

- **Decision owner**: Yury Fedoseev (yfedoseev@gmail.com, repo owner)
- **Ambiguous-spec questions**: open a GitHub issue tagged `spec-question` on this branch's PR, @-mention Yury
- **Regression of the 8/8 Verification Score**: do NOT continue further work until the regression is resolved
- **If this handoff is wrong**: edit it. It is expected to drift; keeping it honest is part of the job.
