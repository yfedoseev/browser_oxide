# 02 — Camoufox v150 architecture, what changed

**Source-of-truth:** `/tmp/camoufox_src/` (git clone `daijro/camoufox@v150.0.2-beta.25`, 1.5 GB).
**License:** MPL-2.0. Read for inspiration; do NOT copy verbatim into browser_oxide (MIT/Apache).
**Surveyed:** 2026-05-27.

## TL;DR

- Camoufox v150 is **Firefox 149-152** with surgical C++ patches in `patches/` (32 .patch files, 4389 lines for the 9 hardware-relevant ones).
- v150 does NOT pretend to be Chrome. Every macOS preset's UA is `Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:150.0) Gecko/20100101 Firefox/150.0`. The bypass is **hardware-coherence**, not UA-spoofing.
- Spoofing is **per-`userContextId`** — a Firefox `BrowsingContext`. Values live in a `RoverfoxStorageManager` keyed by `mUserContextId`. Same model as browser_oxide's per-`Page` `StealthProfile`.
- Configuration flows: `CAMOU_CONFIG[_N]` env-var JSON → `MaskConfig::GetJson()` (the `nlohmann::json` parser at `additions/camoucfg/MaskConfig.hpp`) → typed getters (`GetString`, `GetUint32`, …) → WebIDL bindings via `IsXFunctionEnabledForWebIDL` gates → spoofed value returned to JS.
- Fingerprints supplied by **BrowserForge**: 67 macOS + N linux + N windows presets in `pythonlib/camoufox/fingerprint-presets-v150.json` (16,512 lines, version 1, min_firefox_version 149, generated 2026-05-11 from the `roverfox_fingerprints` data source).

## Architecture map

```
camoufox/
├── additions/                — code added on top of Firefox source
│   ├── camoucfg/             — config schema reader (CAMOU_CONFIG env-var → typed C++ getters)
│   │   ├── MaskConfig.hpp    — the entire schema-reading machinery, 313 lines
│   │   ├── MouseTrajectories.hpp
│   │   ├── json.hpp          — nlohmann::json embedded
│   │   └── moz.build
│   ├── juggler/              — Playwright/CDP protocol bridge (NOT fingerprint-related)
│   └── browser/branding/     — icons + branding (NOT fingerprint-related)
│
├── patches/                  — unified diffs against vanilla Firefox source (32 files)
│   ├── navigator-spoofing.patch       573 lines — installs NavigatorManager
│   ├── screen-spoofing.patch          420 lines — installs ScreenDimensionManager
│   ├── webgl-spoofing.patch           687 lines — installs WebGLParamsManager + getParameter hook
│   ├── audio-context-spoofing.patch    74 lines — AudioContext.sampleRate/maxChannelCount/outputLatency
│   ├── audio-fingerprint-manager.patch 440 lines — AudioFingerprintManager (OscillatorNode randomization)
│   ├── fingerprint-injection.patch    355 lines — bootstrap hooks per-process
│   ├── media-device-spoofing.patch    109 lines — enumerateDevices output shape
│   ├── font-list-spoofing.patch       305 lines — installed-font enumeration
│   ├── anti-font-fingerprinting.patch 1426 lines — measureText / FontFace randomization
│   ├── voice-spoofing.patch                     — speechSynthesis.getVoices
│   ├── speech-voices-spoofing.patch             — fakeCompletion mode
│   ├── geolocation-spoofing.patch
│   ├── timezone-spoofing.patch
│   ├── locale-spoofing.patch
│   ├── webrtc-ip-spoofing.patch
│   ├── network-patches.patch
│   ├── shadow-root-bypass.patch
│   ├── system-ui-font-spoofing.patch
│   ├── playwright/, ghostery/, librewolf/ — vendor sub-directories with extra patches
│   └── (15 more non-fingerprint patches: dark theme, addon pinning, etc.)
│
├── pythonlib/camoufox/       — Python interface (Playwright wrapper + fingerprint injector)
│   ├── fingerprint-presets-v150.json  — THE preset data: 67+N+N fingerprints per OS, 16512 lines
│   ├── fingerprint-presets.json       — legacy preset file (v135 era)
│   ├── fingerprints.py                — fingerprint-loader + env-var encoder
│   ├── webgl/
│   │   ├── webgl_data.db              — SQLite DB of WebGL parameter values by GPU
│   │   ├── sample.py                  — preset sampler
│   │   └── __init__.py
│   ├── fonts.json                     — font preset library
│   ├── voices.json                    — voice preset library
│   └── (locales, geolocation, IP, browserforge integration)
│
└── scripts/                  — build orchestration (mozfetch, patch, package)
```

## Per-context spoofing model

Every fingerprint surface is gated by `Is<Feature>FunctionEnabledForWebIDL(JSContext*, JSObject*)` which:

1. Reads `nsGlobalWindowInner` from the JS object.
2. Pulls `BrowsingContext->OriginAttributesRef().mUserContextId`.
3. Looks up `<feature>_d_<id>` in `RoverfoxStorageManager`.
4. Returns `!disabled` — i.e. spoofing ENABLED unless the storage key explicitly disables it.

Per-context keys observed (from grep):

| Manager | C++ keys | Disabled-key prefix |
|---|---|---|
| `NavigatorManager` | platform, oscpu, hwc, ua | `nav_plat`, `nav_oscpu`, `nav_hwc`, `nav_ua` |
| `WebGLParamsManager` | vendor, renderer | `webgl_vd`, `webgl_rd` |
| `ScreenDimensionManager` | (single gate) | (one key) |
| `AudioFingerprintManager` | (single gate) | (one key) |
| `FontListManager` | (single gate) | (one key) |
| `FontSpacingSeedManager` | (single gate) | (one key) |
| `SpeechVoicesManager` | (single gate) | (one key) |
| `TimezoneManager` | (single gate) | (one key) |
| `WebRTCIPManager` | ipv4, ipv6 | (two keys) |

## Complete configuration schema (every key v150 reads)

Extracted from `grep -rhoE 'MaskConfig::Get\w+\("[^"]+"\)' patches/`:

### Identity (6 keys)
```
navigator.userAgent          string
navigator.appVersion         string
navigator.platform           string
navigator.oscpu              string   ← Firefox-only
navigator.language           string
navigator.hardwareConcurrency uint64
navigator.globalPrivacyControl bool
```

### Screen (8 keys)
```
screen.width                 int32
screen.height                int32
screen.availTop              int32
screen.availHeight           int32   (queried in pythonlib but not in patches grep — verify)
screen.availLeft             int32   (queried in pythonlib but not in patches grep — verify)
screen.availWidth            int32   (queried in pythonlib but not in patches grep — verify)
screen.colorDepth            uint32
screen.pixelDepth            uint32
screen.pageXOffset           double
screen.pageYOffset           double
```

### Window (12 keys)
```
window.innerWidth            double
window.innerHeight           double
window.outerWidth            int32
window.outerHeight           int32
window.devicePixelRatio      double
window.screenX               int32
window.screenY               int32
window.scrollMaxX            int32
window.scrollMaxY            int32
window.scrollMinX            int32
window.scrollMinY            int32
window.history.length        uint32
```

### WebGL (6 simple + 3 nested)
```
webGl:vendor                       string
webGl:renderer                     string
webGl:contextAttributes            object  (per-attribute, e.g. .premultipliedAlpha)
webGl:parameters                   object  (keyed by GL enum pname → value)
webGl:shaderPrecisionFormats       object  (keyed by "<shaderType>,<precisionType>" → {rangeMin,rangeMax,precision})
webGl:supportedExtensions          string list
webGl:parameters:blockIfNotDefined bool     ← strict mode
webGl:shaderPrecisionFormats:blockIfNotDefined bool
+ webGl2:* equivalents (5 more keys)
```

### Audio (4 keys)
```
AudioContext:sampleRate      uint32
AudioContext:maxChannelCount uint32
AudioContext:outputLatency   double
audio:seed                   uint32   ← for deterministic AnalyserNode/OscillatorNode randomization
```

### Media devices (4 keys)
```
mediaDevices:enabled         bool
mediaDevices:micros          uint32   ← # of microphones
mediaDevices:speakers        uint32   ← # of speakers
mediaDevices:webcams         uint32   ← # of webcams
```

### Voices + fonts (5 keys)
```
voices                       array of {lang, name, voiceUri, isDefault, isLocalService}
voices:blockIfNotDefined     bool
voices:fakeCompletion        bool
voices:fakeCompletion:charsPerSecond double
fonts                        string list
fonts:spacing_seed           uint32
```

### Battery (4 keys)
```
battery:charging       bool
battery:chargingTime   double
battery:dischargingTime double
battery:level          double
```

### Geolocation (3 keys)
```
geolocation:accuracy   double
geolocation:latitude   double
geolocation:longitude  double
```

### Locale (4 keys)
```
locale:all      object
locale:language string
locale:region   string
locale:script   string
```

### Headers + misc (5 keys)
```
headers.User-Agent       string
headers.Accept-Encoding  string
headers.Accept-Language  string
timezone                 string
debug                    bool
disableTheming           bool
```

**Total: ~70 distinct configuration keys.** This is the v150 fingerprint surface. Every key has a corresponding `MParamGL` / `MaskConfig::Get*` call inside one of the 32 patches.

## v150 macOS preset distribution (the actual values)

From `fingerprint-presets-v150.json::presets.macos` (67 presets):

### Identity
- UA: only 4 distinct values — Firefox 149, 150, 151, 152 (all `Macintosh; Intel Mac OS X 10.15`)
- platform: `MacIntel` (uniformly)
- maxTouchPoints: `0` (uniformly)
- hardwareConcurrency: `{2, 4, 6, 8, 10, 12, 16, 18}` distinct values

### Screen
- width: 30 distinct values from 736 to 5120 — heavy on 1280, 1440, 1920, 2560
- height: 31 distinct values from 414 to 2560
- colorDepth: `{24, 30}` (24 = legacy SDR, 30 = HDR-capable display)
- devicePixelRatio: `{1, 2, 2.4, 2.609}` (1.0 = non-Retina; 2.0 = Retina; 2.4/2.609 = iPad/iPhone-class)
- availWidth: typically == width
- availHeight: width minus menubar (varies 25-100px less than height)

### WebGL (this is the KEY surface)
- unmaskedVendor distribution: `Apple` (44/67 = 66%) | `Intel Inc.` (9) | `ATI Technologies Inc.` (9) | `Google Inc. (Apple)` (NONE seen)
- unmaskedRenderer distribution:
  - `Apple M1, or similar` — 44/67 (66%)
  - `Intel(R) HD Graphics 400, or similar` — 9
  - `Radeon R9 200 Series, or similar` — 6
  - `Radeon HD 3200 Graphics, or similar` — 3
  - `Intel(R) HD Graphics, or similar` — 3
  - `Intel 945GM, or similar` — 2

**Note:** every renderer string ends in `, or similar` — BrowserForge anonymization. This is a recognizable string pattern that anti-bot vendors CAN cluster on, but apparently the sheer popularity (66% of macOS users → Apple M1 anyway) and the value-format match for valid hardware makes it sufficient.

Notably **none** of v150's macOS presets use Chrome's `ANGLE` prefix. Chrome on macOS reports e.g. `ANGLE (Apple, ANGLE Metal Renderer: Apple M3, Unspecified Version)` — that's a **Chrome-specific format**. v150's `Apple M1, or similar` is Firefox-style. Therefore browser_oxide (claiming Chrome 148) MUST keep the ANGLE format; copying v150's bare-vendor format would be a Chrome-vs-Firefox identity leak.

## What v150 added in the hardware-spoofing lineage (v146 → v150)

The Camoufox repo doesn't ship per-version changelogs (the v146-hardware release notes literally say "This release has not been tested yet. Please don't use unless you know what you are doing :)"). The lineage MUST be inferred from git history. Open question — see `16_DECISION_LOG.md`.

Empirically (from the gate measurement on 2026-05-27): v150 gained 8 sites over v135 — amazon-ca, amazon-com, amazon-com-au, amazon-fr, amazon-in, amazon-jp, booking, douyin. The 7 AWS WAF + booking + douyin set strongly implies the v146→v150 patches went DEEPER on hardware coherence (e.g. screen.width/height now validated against `window.outer*`/`window.inner*`/`window.devicePixelRatio` for cross-consistency, or WebGL `getParameter(MAX_*)` values now reading from per-renderer canonical tables).

The diff to compute (open task):
```bash
cd /tmp/camoufox_src
git log --all --oneline --before=2026-03-14 --after=2025-12-31 -- patches/
git log --all --oneline --before=2026-05-11 --after=2026-03-14 -- patches/
```

(Cannot run yet — the v150.0.2-beta.25 clone is shallow `--depth 1`. Need a `--unshallow` to get the lineage.)

## Implications for browser_oxide

1. **Camoufox stays as Firefox.** Browser_oxide claims to be Chrome 148. The AWS WAF challenge.js apparently accepts Firefox fingerprints, so the issue isn't UA-class. The issue is **internal consistency** — does our claimed Chrome 148 macOS fingerprint hold up under cross-API correlation?
2. **v150's WebGL surface is HEAVILY anonymized** (`, or similar` suffix on every renderer). Chrome doesn't do this; real Chrome reports `ANGLE (Apple, ANGLE Metal Renderer: Apple M3, Unspecified Version)`. We CANNOT copy v150's pattern — see `06_WEBGL_DIFF.md`.
3. **v150's preset diversity is large** (67 macOS variations). Browser_oxide ships ONE chrome_148_macos preset. AWS WAF's IP-clustering may flag repeated identical fingerprints; v150's diversity bypasses this. **Implication:** browser_oxide should support a preset SAMPLER — pick one of N pre-validated profiles per Page — but that's `R-PROFILE-SAMPLER`, scope-creep for this audit. (File as follow-up in `15_FIX_PRIORITY_RANKED.md`.)
4. **v150 ships `audio:seed`** for deterministic AnalyserNode/OscillatorNode randomization. Browser_oxide already has this (`audio_seed` in YAML, `AudioFingerprintManager` port at `crates/canvas/src/audio.rs`).
5. **v150 ships `mediaDevices:micros/speakers/webcams`** counts. Browser_oxide's `media_devices` field handles this — coverage parity.
6. **v150 patches at the WebIDL binding layer** (per-context gates in C++). Browser_oxide's `_maskAsNative` in JS bootstrap achieves the same goal but at a different layer — the WebIDL gate cannot be bypassed by JS unless the attacker breaks out of V8, while our JS masks can be enumerated by `Object.getOwnPropertyNames`. Lower-priority: only matters for sandboxed-attack vectors, not the AWS WAF browser-side probe.

## What to read next

- [`03_HARDWARE_SPOOFING_DIFF.md`](03_HARDWARE_SPOOFING_DIFF.md) — the per-field BO vs v150 vs real-Chrome comparison
- [`06_WEBGL_DIFF.md`](06_WEBGL_DIFF.md) — WebGL renderer/vendor/getParameter consistency analysis
- [`14_AWS_WAF_CORRELATION.md`](14_AWS_WAF_CORRELATION.md) — which surfaces challenge.js reads and why our specific values fail

## Status

⬜ Pending follow-up:
- Unshallow the camoufox repo to get the v146-v150 commit lineage.
- Sample 5 linux + 5 windows presets to confirm same schema.
- Inspect `pythonlib/camoufox/webgl/webgl_data.db` (SQLite) for the per-GPU parameter table.
