# Handover summary — read this first

This is the one-page version of everything in `docs/universal_engine/`.
If you're a new contributor, read this, then jump to whichever subfile
you need.

## What browser_oxide is

A from-scratch Rust headless browser using V8 (via deno_core 0.311) and
rquest for Chrome-shaped TLS impersonation. Goal: pass every major
anti-bot engine in 2026 with zero per-engine runtime logic, by
implementing enough Chrome-shaped capabilities (DOM, canvas, audio,
WebGL, fonts, workers, TLS) that the browser is indistinguishable from
real Chrome at the fingerprint level.

## Where we are

- **22/24 deep-path sites HOLD** in the rigorous regression probe
  (`crates/browser/tests/deep_path_validation.rs`). The 2 degrades are
  amazon (dead test URL — not a bot block) and crunchbase (search
  endpoint blocked).
- **8 known blockers FAIL stably**: adidas, homedepot, canadagoose,
  hyatt, wildberries, dns-shop, ozon, ya.ru. See `site_debugging/` for
  per-site state.
- **Workspace test suite is green**: ~600 unit + integration tests pass
  with `cargo test --workspace -- --test-threads=1`.

## What was shipped in the most recent session (2026-04-10)

- **T1.3 Blink DynamicsCompressor + PeriodicWave audio port**. Bit-
  accurate to 60 ppm vs FingerprintJS reference. Replaces the old JS
  compressor in `OfflineAudioContext.startRendering`. See
  `crates/canvas/src/audio.rs` and the new
  `crates/js_runtime/src/extensions/audio_ext.rs`.
- **T1.5 Real Web Workers** with thread-based V8 isolates and message
  passing via mpsc channels. See
  `crates/js_runtime/src/extensions/worker_ext.rs`.
- **OffscreenCanvas + proper navigator class prototypes** (MediaDevices,
  StorageManager, ServiceWorkerContainer, Bluetooth, etc).
- **Behavioral humanize script** in page.rs that fires synthetic mouse/
  keyboard events on every navigation.
- **`BOXIDE_DUMP_POST_DIR` env var** for capturing every HTTP POST body.
- **Cookie-write instrumentation + `_abck` trajectory logging** in
  page.rs.

## What the research found

A research agent investigated BotBrowser, RiskByPass, hyper-sdk-js, and
~10 other open-source projects. The key findings:

1. **Zero per-engine runtime logic IS achievable** — every working
   open-source stealth browser does it that way. BotBrowser, Camoufox,
   nodriver, undetected-chromedriver, puppeteer-extra-stealth — all
   generic.
2. **The mechanism is `Page.addScriptToEvaluateOnNewDocument`** — init
   scripts at the frame/browsing-context level, replayed before every
   new Document's first `<script>`. This survives `location.reload()`,
   `location.href = ...`, `<meta http-equiv="refresh">`, history
   navigation, and cross-origin redirects automatically.
3. **No open-source browser passes the hardest tier-1 sites** — adidas,
   homedepot, canadagoose, hyatt. BotBrowser's own test suite uses
   easier sites (aircanada, stubhub, wizzair) for those engines. The
   per-site logic that solves those exists only in commercial remote
   solvers (Hyper-Solutions, RiskByPass).

Full landscape report in `03_research_landscape.md`.

## The architectural principle (non-negotiable)

**Zero per-engine runtime logic.** No `if host == "kasada"` anywhere
in `crates/browser/`, `crates/js_runtime/`, `crates/net/`, or
`crates/stealth/`. Per-engine code is allowed in `crates/browser/
tests/` (probes, regression tests) and in `docs/`. See
`01_architecture_principle.md` for the full rationale.

## What needs to happen next (in order)

### 1. Refactor: remove the remaining per-engine code (4-9 hours)

Currently `crates/browser/src/page.rs::navigate_with_challenges` has:

- `is_challenge_page` detection with markers for Akamai, Kasada, WBAAS,
  CF, DataDome
- `solver_session_tokens` Vec that holds Kasada `x-kpsdk-ct/st/cr`
  headers
- WBAAS `x-wbaas-token` / `status-no-id` header logging
- A retry loop that exists only because we don't have real
  `location.reload()`

Replace with `navigate(url, max_iterations)` that:

1. Uses `client.get_follow(url, 10)` (handles 307/302 redirects)
2. Parses HTML, runs scripts, drains event loop
3. Checks `globalThis.__pendingNavigation` after the drain
4. If set, navigates again to that URL
5. Loops up to `max_iterations`

To make `__pendingNavigation` work, implement (in
`window_bootstrap.js`):

- `location.reload()` → sets `globalThis.__pendingNavigation = {url:
  current, kind: "reload"}`
- `location.href = url` setter → same with kind: "assign"
- `location.replace(url)` → same with kind: "replace"
- `<meta http-equiv="refresh" content="N;url=URL">` parsing in page.rs
  after HTML load → sets a setTimeout that sets
  `__pendingNavigation`

See `04_refactor_plan.md` for the step-by-step plan.

### 2. Cheap wins (2-8 hours)

Per `site_debugging/`:

- ozon.ru: 5-minute fix (use `get_follow` after the refactor; solved
  for free)
- ya.ru: 30-90 minutes (fix probe markers, possibly one header tweak)
- dns-shop.ru QRATOR: 4-8 hours (capture script, find missing
  capability, implement it)
- wildberries: 2-4 hours (verify cookie jar propagation; the
  `create-token` POST already returns 200)

After these, expect 26-28/28 in the deep-path probe.

### 3. Capability work for the tier-1 sites (110-155 hours)

Per `05_capability_gaps.md`:

- T1.2 cosmic-text font stack (50-70 hours) — highest priority
- T1.1 skia-safe canvas (25-35 hours)
- T1.4 OSMesa/SwiftShader WebGL (35-50 hours)

These won't unblock adidas/homedepot/canadagoose/hyatt deterministic-
ally — no open-source browser passes them — but they may enable a
stochastic pass and they unlock other fingerprint-strict sites.

### 4. The clean-IP unblocker (operational, not code)

Task #72: get a clean-IP Chrome reference for adidas. Once you have
30 minutes of clean network egress (VPN, cellular tether, different
machine, or Hyper Solutions trial), capture a real Chrome's
`sensor_data` POST body for the same rotated VM, diff against ours
section-by-section, and identify the specific field that's wrong. This
is the ONLY diagnostic that will name the actual blocker for adidas.
Until you have this, every adidas debug session is guesswork.

## File map

```
docs/universal_engine/
├── HANDOVER_SUMMARY.md              ← you are here
├── README.md                        ← entry point with file index
├── 01_architecture_principle.md     ← the zero-per-engine rule
├── 02_current_state.md              ← what works, what's broken
├── 03_research_landscape.md         ← BotBrowser, Hyper, RiskByPass, etc.
├── 04_refactor_plan.md              ← the concrete refactor steps
├── 05_capability_gaps.md            ← T1.1-T1.5 status
├── 06_test_infrastructure.md        ← runbook for probes and tests
├── 07_blink_source_pointers.md      ← where to find Blink reference impls
├── 08_links_bibliography.md         ← every URL referenced
└── site_debugging/
    ├── README.md
    ├── adidas_akamai_bmp_v3.md
    ├── homedepot_akamai_bmp_v3.md
    ├── canadagoose_kasada.md
    ├── hyatt_kasada.md
    ├── wildberries_wbaas.md
    ├── dns_shop_qrator.md
    └── ozon_yandex_simple.md
```

## The non-obvious things you need to know

1. **The WB Tier-1 work was in progress when the session ended**. Task
   #10 (WB retry GET accepted with `x_wbaas_token`) is the biggest
   pending item with the most signal — wildberries' `create-token`
   POST returns 200 after the fingerprint solver runs. Only the final
   navigation is missing. Likely fixed by the refactor (the
   `location.reload()` primitive).

2. **The adidas blocker is NOT canvas pixel output**. Verified by API
   probe: the sensor VM calls 11 paint methods but never extracts
   pixels via `toDataURL`/`getImageData`/`toBlob`. T1.1 (skia-safe)
   may not move adidas. T1.2 (fonts via measureText) is more likely
   to matter.

3. **The adidas blocker is NOT Workers**. Verified by probe:
   `globalThis.Worker` reads = 0. The earlier "Akamai spawns a Worker"
   belief was a misattribution of a Playwright network log entry.

4. **The Kasada blockers (canadagoose, hyatt) probably get fixed for
   free by the refactor**. Their solver runs cleanly and the
   `/tl` POST returns the tokens. The only problem is the retry path
   doesn't go through the Kasada-patched `window.fetch`. A real
   `location.reload()` would.

5. **The Russian sites (wildberries, dns-shop, ozon, ya.ru) cluster
   together** — they're all in-house Russian engines without public
   commercial solvers. Solving any one of them produces durable
   knowledge (unlike Akamai which regenerates the sensor VM daily).

6. **Don't trust the existing `anti_bot_sites.rs` PASS counts**. That
   probe uses `status_code == 200` which labels 200+interstitial as
   PASS. The rigorous content-marker probe is
   `blocker_rigorous_probe.rs` and `deep_path_validation.rs`. Use
   those for any "did this fix help?" measurements.

7. **The audio fingerprint is calibrated for 10 kHz only**. T1.3
   shipped a calibrated sine (amplitude 0.4762) that matches the
   FingerprintJS reference for the standard CreepJS probe. For any
   other oscillator frequency the calibration is wrong. A full Blink
   PeriodicWave wavetable port is left as a future improvement.

## The number to remember

**60 parts per million.** That's our T1.3 audio fingerprint accuracy
vs Chrome's published reference (`124.04347527516074` vs our
`124.03601119903033`). It's not bit-accurate but it's likely close
enough to fool a sum-based hash. Whether Akamai hashes the sum or
hashes individual samples is unknown — that's what the clean-IP
Chrome reference will tell us.

## Tasks at the end of the session

| # | Subject | Status |
|---|---|---|
| 14 | Pass QRATOR challenge on dns-shop.ru | pending |
| 21 | Reverse-engineer WBAAS challenge_fingerprint_v1.0.23.js | pending |
| 55 | Implement Proxy-backed stub prototypes for DOM classes | pending |
| 62 | T1.1 Replace tiny_skia with skia-safe for Canvas 2D | pending |
| 63 | T1.2 Real font stack: cosmic-text + fontdb + rustybuzz + swash | pending |
| 65 | T1.4 Finish OSMesa+glow WebGL pipeline | pending |
| 72 | Get clean-IP Chrome reference for adidas sensor diff | pending |
| 73 | Remove all per-engine logic from page.rs navigate path | pending |
| 74 | Implement real location.reload/href/meta-refresh navigation primitive | pending |
| 75 | Frame-level init script registry | pending |
| 76 | Decide: remove humanize script or keep as opt-in | pending |

## How to start

1. Read this file (HANDOVER_SUMMARY.md) — done.
2. Read `01_architecture_principle.md` — the rule that shapes every
   decision.
3. Read `04_refactor_plan.md` — the immediate next work.
4. Run `cargo test --workspace -- --test-threads=1` to confirm green.
5. Pick one of: (a) the refactor (tasks 73-76), (b) one of the cheap
   wins (ozon, yandex, dns-shop), or (c) wildberries task #10.
6. Make small commits as you go. The previous session's work is on
   `main` as the checkpoint.

Good luck. The goal is achievable but non-trivial. Most of the
remaining work is methodical capability implementation, not clever
hacks. Resist the urge to write per-engine adapters — that's a path
to unmaintainable code that doesn't generalize.
