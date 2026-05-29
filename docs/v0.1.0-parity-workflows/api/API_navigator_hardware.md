# API parity deep dive — navigator + screen + hardware fingerprint surface

**Owner scope:** structural / stealth (cross-cutting); feeds AWS WAF cluster
(amazon-*, imdb), DataDome, CreepJS/BotD, Kasada holistic ML.
**Status:** audit complete; concrete fixes ranked at the end.
**Date:** 2026-05-28. Branch `fix/v0.1.0-fix4-canvas-parity`.

This doc audits every `navigator.*`, `screen.*`, and `window.*` hardware/geometry
surface the task brief named, classifies each as implemented / stubbed / missing,
checks the **per-profile value correctness** and — the load-bearing part —
**cross-API coherence**. It compares BO's hand-curated-preset model against
browserforge's Bayesian-network joint sampling and Camoufox's C++-level managers.

---

## 1. What the existing repo docs already concluded

Read these first; this doc extends, does not repeat, them.

- **`16_STEALTH_FINGERPRINT_AUDIT.md` §2.2** — the Navigator surface is the row
  `Navigator.prototype (userAgent, platform, …) ~25 methods ✅ via
  _maskAsNative(_NavProto,…) window_bootstrap.js:1029`. The masking story for the
  navigator scalar getters is *done*: every getter is `[native code]`-masked at
  `window_bootstrap.js:1029-1035`. The doc's headline gaps are all on *other*
  surfaces (Event ctors, fetch trio, XHR, WebGL methods) — none of which are the
  navigator/screen scalar values this doc covers. **Conclusion carried forward:
  navigator scalar masking is not a gap.**
- **`17_WEB_API_PARITY_MATRIX.md` §2.24 + §2.25** — `NetworkInformation`
  (`navigator.connection`) ✅, `screen` ✅, `devicePixelRatio` ✅,
  `innerWidth/outerWidth/screenX/Y` "✅ from profile (verify)". These rows are
  presence-level; they do not check value plausibility or coherence — that is
  this doc's job.
- **`audit/03_HARDWARE_SPOOFING_DIFF.md`** — the closest prior work. It did a
  per-field BO-vs-v150-vs-realChrome table for the chrome_148_macos profile and
  surfaced 5 deltas. Its key conclusions I build on:
  - **BO ships one preset; v150 ships 67 macOS variants** → clustering risk
    (`R-PROFILE-SAMPLER`). *Partly resolved since that doc: a chip-coherent
    sampler now exists (`presets.rs:750`, §5 below).*
  - **v150 IS Firefox** so has fewer Chrome-specific surfaces to make
    inconsistent; **BO claims Chrome 148 so every Chrome surface must cohere.**
    This is the strategic reason cross-API coherence matters more for BO.
  - It left 7 cross-API correlation rows as ❓ (Sec-CH-UA HTTP ↔ userAgentData JS,
    main-window navigator ↔ worker navigator, screen×DPR physical realism,
    outerHeight−innerHeight chrome-bar realism). **This doc resolves those rows
    against source** (§4).
  - It flagged `device_memory` "verify value: 8 GB plausible" — this doc finds a
    concrete bug in the *sampled* profile (§3.3 / FIX-NAV-1).
- **`audit/02_CAMOUFOX_V150_OVERVIEW.md`** + DeepWiki confirm Camoufox spoofs at
  the C++ level via `NavigatorManager` / `ScreenDimensionManager` /
  `RoverfoxStorageManager`, keyed per `userContextId`, so worker and main-window
  navigators read the *same* store and CSS media queries
  (`nsMediaFeatures.cpp`) are hooked to agree with `screen.*`. Camoufox does
  **not** spoof `navigator.userAgentData`/`getHighEntropyValues` at all (it's
  Firefox — the interface doesn't exist), which is exactly why v150 has fewer
  coherence axes than BO.

---

## 2. Per-property audit table

Legend: **Impl** = implemented / stubbed / missing. **Profile-driven** = value
comes from `StealthProfile` via `op_get_profile_value` (stealth_ext.rs:53) vs a
JS-side constant. File:line is the definition site in
`crates/js_runtime/src/js/window_bootstrap.js` unless noted.

### 2.1 navigator.*

| Property | Impl | Value source | file:line | Notes / coherence |
|---|---|---|---|---|
| `userAgent` | impl | profile `user_agent` | 961 | reduced `Chrome/148.0.0.0` form (presets.rs:42). ✅ |
| `userAgentData` (NavigatorUAData) | impl | profile, lazy | 1786-1844 | SecureContext-gated; low-entropy brands/mobile/platform + `getHighEntropyValues`. See §2.4. |
| `userAgentData.getHighEntropyValues` | impl | profile | 1794-1827 | Rejects non-array with TypeError (Chrome-accurate); masked native (1827). Returns architecture/bitness/platformVersion/model/wow64 from profile. ✅ |
| `platform` | impl | profile `platform` | 962 | Validated == `MacIntel`/`Win32`/`Linux*` by `os_name` (profile.rs:253-264). ✅ |
| `hardwareConcurrency` | impl | profile `cpu_cores` | 974 | maps to `p.cpu_cores` (stealth_ext.rs:64). ✅ value, ⚠️ see §3 single-value clustering. |
| `deviceMemory` | impl | profile `device_memory` | 979 | SecureContext-gated; skipped on iOS (correct — no NavigatorDeviceMemory on Safari). **BUG in sampler: not clamped to Chrome's {…,8} cap — §3.3.** |
| `languages` | impl | profile `languages`, frozen+cached | 952-958, 971 | Memoized frozen array (identity-stable, Chrome-accurate). ✅ |
| `language` | impl | profile `language` | 970 | Validated ∈ `languages` (profile.rs:307). ✅ |
| `plugins` (PluginArray) | impl | profile `plugins_count` | 1007, 273-360 | Live count + indexed + named access + iterator + masked. ✅ strong. |
| `mimeTypes` (MimeTypeArray) | impl | profile `mime_types_count` | 1008, 313-410 | Same shape as plugins. ✅ |
| `webdriver` | impl | constant `false` | 991-995 | getter masked `get webdriver`; returns `false` (modern-Chrome correct, not `undefined`). ✅ |
| `vendor` | impl | constant `"Google Inc."` | 963 | ⚠️ **hard-coded constant, NOT profile-read** — see §3.5. Wrong for the Firefox presets. |
| `vendorSub` | impl | constant `""` | 964 | ✅ Chrome. |
| `oscpu` | **missing** (correct) | — | — | Chrome has no `navigator.oscpu` (Firefox-only). Correctly absent. ✅ |
| `connection` (NetworkInformation) | impl | profile | 174-180, 1005 | `effectiveType/rtt/downlink/saveData/downlinkMax`. rtt rounded to 25 ms bucket, downlink to 0.025 (Chrome privacy quantization — correct, 176-177). Skipped on iOS. ✅ |
| `maxTouchPoints` | impl | profile `max_touch_points` | 981 | Validated against pointer_type (profile.rs:267). ✅ |
| `pdfViewerEnabled` | impl | constant `true` | 982 | ⚠️ ignores profile `pdf_viewer_enabled` (which the *worker* navigator honors, worker_bootstrap.js:156). Minor inconsistency — §3.6. |

### 2.2 screen.*

| Property | Impl | Value source | file:line | Notes |
|---|---|---|---|---|
| `width` / `height` | impl | profile | 1437-1438 | prototype-backed (own-descriptor probe → undefined, Chrome-accurate). ✅ |
| `availWidth` / `availHeight` | impl | profile | 1439-1440 | ✅ |
| `availLeft` | impl | constant `0` | 1441 | ✅ (real macOS = 0). |
| `availTop` | impl | profile `screen_avail_top` | 1442 | macOS menubar 33 px (presets.rs:145). ✅ |
| `colorDepth` / `pixelDepth` | impl | profile `screen_color_depth` | 1443-1444 | Both read same key → always equal (Chrome-accurate). 30 on HDR MBP. ✅ |
| `orientation` | impl | **constant object** | 1433, 1445 | `{type:"landscape-primary", angle:0}` hard-coded; `ScreenOrientation.prototype` exists. ⚠️ static — see §3.4. |
| `isExtended` | impl | constant `false` | 1446 | ✅ |

### 2.3 window.* geometry

| Property | Impl | Value source | file:line | Notes |
|---|---|---|---|---|
| `innerWidth` / `innerHeight` | impl | profile | 1481-1482 | lazy getter. ✅ |
| `outerWidth` / `outerHeight` | impl | profile | 1483-1484 | Validated `outer ≥ inner ≤ screen` (profile.rs:291-296). ✅ |
| `devicePixelRatio` | impl | profile `device_pixel_ratio` | 1485 | masked `get devicePixelRatio`; correctly **Window-only, NOT on navigator** (1668-1681 documents the removal). ✅ |
| `screenX` / `screenY` | impl | **constant `0`** | 1524-1533 | ⚠️ always 0. Real maximized macOS Chrome: screenX 0, screenY ~25-38 (menubar). See §3.4. |
| `screenLeft` / `screenTop` | impl | constant `0` | 1534-1539 | same as screenX/Y. |
| `scrollX/Y`, `pageXOffset/YOffset` | impl | module state | 1501-1522 | own accessor on window (Chrome-accurate placement). ✅ |

### 2.4 userAgentData detail

`window_bootstrap.js:1786-1844`. Low-entropy `brands` use a per-construction
crypto-shuffled GREASE array (1743-1778) with the *current* Chrome GREASE entry
`{brand:"Not.A/Brand", version:"8"}` (1767) — this matches real Chrome 147/148.
`getHighEntropyValues` (1794) pulls `architecture`/`bitness`/`platformVersion`/
`model`/`wow64` from the profile, so the JS UA-CH surface is built from the same
profile fields that drive the HTTP `Sec-CH-UA-*` headers — see §4.1.

---

## 3. Coherence findings (the load-bearing part)

BO's design guarantees coherence *by construction* for everything that reads
`op_get_profile_value`: the main-window navigator, the worker navigator, and the
HTTP client-hints builder all read the **single `StealthProfile` struct**, and
`StealthProfile::validate()` (profile.rs:218-376) is a 30-rule cross-field
consistency checker run in `debug_assert!` by the sampler (presets.rs:823).
This is architecturally stronger than browserforge's post-hoc sampling and on par
with Camoufox's shared-store model. The gaps below are where a value either
bypasses the profile (a JS-side constant) or where the profile/validator itself
permits an incoherent value.

### 3.1 RESOLVED: main-window navigator ↔ worker navigator (audit/03.C, Camoufox #443)

Camoufox bug #443 (v146) was: v135 patched only `WorkerNavigator`, leaving the
*main-window* `Navigator.platform/hardwareConcurrency/timezone` un-spoofed → a
worker-vs-main mismatch any fingerprinter can read by spawning a Worker and
comparing `self.navigator.platform` to the parent's. The audit asked "does BO
have this in reverse?" **Answer: no.** Both read the same Rust profile:
- main: `hardwareConcurrency` ← `_pInt("hardware_concurrency")` (window_bootstrap.js:974)
- worker: `hardwareConcurrency` ← `_pInt("hardware_concurrency")` (worker_bootstrap.js:147)

Same for `platform`, `deviceMemory`, `languages`, `userAgentData`. The values are
literally the same `op_get_profile_value` op. **BO is coherent here; close
audit/03.C as not-a-bug.** One nit: the *fallback defaults* differ when no profile
is installed (worker default UA = `Chrome/147`, worker_bootstrap.js:140; main
default = `Chrome/130`, window_bootstrap.js:961). Defaults only fire with no
profile (never in production), so this is cosmetic — but worth aligning to avoid
a future foot-gun. (FIX-NAV-5.)

### 3.2 RESOLVED: screen × devicePixelRatio physical realism (audit/03 ❓)

1512×982 @ DPR 2.0 → 3024×1964 backing = real 13.6" MBP / 14" MBP base panel.
The sampler (presets.rs:760-808) only emits real Apple panel geometries per chip.
`outerHeight−innerHeight = 982−871 = 111 px` is exactly the Chrome-148-macOS
toolbar+tab+bookmarks bar (presets.rs:805-806 documents the 111). ✅ Coherent.

### 3.3 BUG (real): `deviceMemory` in the sampler exceeds Chrome's spec cap — FIX-NAV-1

`navigator.deviceMemory` per the W3C Device Memory spec is **clamped client-side
to the set {0.25, 0.5, 1, 2, 4, 8}** — real Chrome **never reports more than 8**,
regardless of physical RAM (this is a privacy upper-bound, not the real GB count).
The Pixel preset comment knows this (presets.rs:848 "Chrome rounds to spec set
{0.25,0.5,1,2,4,8}"), and the static macOS preset is fine (device_memory=8,
presets.rs:149).

But the **sampler** sets `p.device_memory` from `ram_pool` containing
`16/18/24/36/48` (presets.rs:765, 776, 787, 797). Then `deviceMemory` getter
(window_bootstrap.js:979) returns that raw value via `_pInt("device_memory")`.
**Result: a sampled M3 Max profile reports `navigator.deviceMemory === 48`, which
real Chrome can never produce.** This is a deterministic, single-read bot tell on
every AWS-WAF amazon-* site whenever the sampler is used. The `validate()` bound
only rejects `>64` (profile.rs:302), so it passes the debug_assert.

The physical RAM should still drive the *Sec-CH-UA / hardware-coherence* story
(M3 Max has 36-48 GB), but the **JS `navigator.deviceMemory` getter must clamp to
`min(8, round-down-to-spec)`**. Cleanest fix: clamp inside the getter
(window_bootstrap.js:979) and the worker (worker_bootstrap.js:148), or split the
profile into `device_memory_physical` (drives nothing JS-exposed) and a
`device_memory_js` capped at 8. Lowest-risk: clamp at the getter.

### 3.4 GAP: `screen.orientation` and `screenX/screenY` are static constants — FIX-NAV-3

- `_screenOrientation` is a frozen `{type:"landscape-primary", angle:0}`
  (window_bootstrap.js:1433). For desktop this is correct. But for the **mobile
  presets** (pixel_9_pro, iphone_15_pro) a portrait device should report
  `portrait-primary` / angle 0, and the orientation type must agree with
  `screen.width < screen.height`. Currently a Pixel profile (412×870, portrait)
  still reports `landscape-primary` → an internal screen-vs-orientation
  contradiction CreepJS reads directly. **Make orientation profile-derived:**
  `width >= height ? "landscape-primary" : "portrait-primary"`.
- `window.screenX/screenY` hard-coded to 0 (1524-1533). Real maximized Chrome on
  macOS: `screenX===0`, `screenY≈25-38` (below the menubar). `screenY===0` while
  `screen.availTop===33` is a mild contradiction (the window can't be above the
  available area). Low impact but cheap: set `screenY` from a `screen_avail_top`-
  consistent constant.

### 3.5 BUG: `navigator.vendor` / `vendorSub` are hard-coded constants — FIX-NAV-2

`window_bootstrap.js:963-964` hard-code `vendor = "Google Inc."`, `vendorSub = ""`.
The profile struct *has* a `vendor` field (stealth_ext.rs:58 exposes it; the
worker navigator at worker_bootstrap.js:154 reads `_p("vendor")`). So:
- For the **Firefox presets** (`firefox_135_*`, presets.rs:422+), real Firefox
  reports `navigator.vendor === ""`. BO's main-window navigator will report
  `"Google Inc."` while claiming a Firefox UA → a one-line, 100%-reliable
  Firefox-impersonation breaker. (The worker navigator would correctly read `""`
  from the profile — so it's *also* a main-vs-worker mismatch, the inverse of #443
  for the Firefox presets.)
- Even for Chrome this is fragile: it should read the profile so it can never
  drift. **Fix: `_defNav('vendor', () => _p("vendor", "Google Inc."))` and
  `vendorSub` likewise.** This mirrors the worker. One-liner, removes a whole
  class of future incoherence.

### 3.6 NIT: `pdfViewerEnabled` ignores the profile — FIX-NAV-4

Main navigator hard-codes `true` (window_bootstrap.js:982); worker reads
`_p("pdf_viewer_enabled")` (worker_bootstrap.js:156). If a profile ever sets
`pdf_viewer_enabled=false` (e.g. an enterprise-locked-down persona), main and
worker disagree. Make main read the profile for symmetry. Tiny.

### 3.7 OBSERVATION: single hardwareConcurrency value (clustering) — addressed

audit/03 flagged "BO ships one preset; v150 covers 8 hwConcurrency values." The
chip-coherent sampler (presets.rs:750) now covers cores ∈ {8,11,12,14,16} and
RAM ∈ {8,16,18,24,36,48}, each constrained to a real Apple chip + matching
GpuProfile + panel. This is the *right* design (browserforge's joint sampling and
Camoufox's per-context store both keep cores↔GPU↔RAM correlated). The remaining
gap is only that the sampler exists for macOS but not the Windows/Linux/mobile
presets, and that deviceMemory bug (§3.3) currently makes the sampler emit
detectable values.

---

## 4. External comparison — how the SOTA generates coherent values

### 4.1 browserforge (apify fingerprint-suite): Bayesian joint sampling

`browserforge/fingerprints/generator.py` does **not** hand-author presets. It
loads a `BayesianNetwork` trained on real apify fingerprint datapoints and calls
`generate_consistent_sample_when_possible({'userAgent': (ua,)})`
(generator.py:204-209). Every field — `screen` (15 sub-fields incl.
`availTop`, `devicePixelRatio`, `innerHeight`, `screenX`),
`deviceMemory`, `hardwareConcurrency`, `maxTouchPoints`, `videoCard`,
`platform` — is drawn from the **conditional joint distribution** given the UA,
so correlations (DPR↔screen, cores↔GPU↔RAM, platform↔UA) are preserved by
construction rather than by 30 hand-written rules. Key contrasts with BO:
- browserforge's `NavigatorFingerprint` (generator.py:46-66) includes `oscpu`
  (Firefox) and `deviceMemory: Optional[int]` — it *omits* the field for browsers
  that don't expose it, exactly as BO's iOS gating does (window_bootstrap.js:978).
- It does **not** itself produce `userAgentData`/`getHighEntropyValues` JS shape
  — that is BO's value-add and also BO's larger coherence burden (see §4.3).

**Takeaway for BO:** the static-preset + 30-rule-validator model is robust and
arguably *more* auditable than a black-box network, but it scales by hand. BO's
sampler (presets.rs:750) is the right middle path; widen it to other OSes and fix
the deviceMemory clamp and it matches browserforge's coherence guarantee for the
fields anti-bots actually cross-check.

### 4.2 Camoufox v150: C++ managers + shared store (DeepWiki-confirmed)

`NavigatorManager` / `ScreenDimensionManager` back `hardwareConcurrency`,
`platform`, `screen.*`, with `WorkerNavigator::GetPlatform()` /
`HardwareConcurrency()` hooked to read the **same** per-`userContextId` store —
so main-window and worker can never disagree, and CSS media queries
(`nsMediaFeatures.cpp`) are hooked to match `screen.width/height`. `devicePixelRatio`
is a global-only `CAMOU_CONFIG` key. Camoufox does **not** spoof `userAgentData`
(Firefox lacks it). **BO already achieves the same worker/main coherence** via the
shared `op_get_profile_value` op (§3.1) — the one place BO breaks the model is the
JS-side *constants* (`vendor`, `pdfViewerEnabled`, `orientation`) that bypass the
shared store (§3.4-3.6). Fixing those makes BO's coherence model equivalent to
Camoufox's, plus BO additionally hooks CSS via `matchMedia` from the same profile
(window_bootstrap.js:4007-4093, `device-width`/`orientation`/`resolution` cases).

### 4.3 The strategic asymmetry (from audit/03 §TL;DR-3)

Because v150 is Firefox, it has *no* `userAgentData`, no `Sec-CH-UA`, and a small
navigator surface — fewer axes to make inconsistent. BO claims Chrome 148, so it
must keep **three** representations of the same identity in sync: the UA string,
the `Sec-CH-UA-*` HTTP headers (`crates/net/`), and `navigator.userAgentData` JS.
All three read the same profile in BO, so they cohere — but it means BO carries a
coherence burden v150 simply doesn't have. This is acceptable (Chrome impersonation
is the higher-value target), but it raises the cost of every JS-side constant that
bypasses the profile. **Every one of §3.5/3.6/3.4 is a place where BO took on the
Chrome coherence burden and then leaked it via a hard-coded value.**

External corroboration that these mismatches are scored: bot-detection vendors
explicitly cross-check UA ↔ UA-CH ↔ `navigator.userAgentData` and flag a
"Safari/Firefox-like UA string paired with Chromium Sec-CH-UA" as an implausible
combination (wilico.co.jp consistency-check writeup; MDN UA-reduction). BO's
hard-coded `vendor="Google Inc."` under a Firefox UA (§3.5) is precisely this
class of tell.

---

## 5. Ranked fix list (ROI order)

All fixes are **public-engine** (JS bootstraps + Rust presets/profile) — none
touch per-vendor bypass code, so none belong in `vendor_solvers`.

| ID | Fix | Effort | Confidence | Site impact | Engine |
|---|---|---|---|---|---|
| FIX-NAV-1 | Clamp `navigator.deviceMemory` getter to Chrome's spec cap (max 8) in main (window_bootstrap.js:979) + worker (worker_bootstrap.js:148); keep physical RAM only for non-JS coherence. Tighten `validate()` to reject JS deviceMemory>8. | 0.5 day | high | AWS-WAF cluster (amazon-ca/com/au/fr/jp + imdb) — removes a deterministic tell on every sampled profile; plausibly flips 1-2 if it was the marginal signal | public |
| FIX-NAV-2 | Make `navigator.vendor`/`vendorSub` profile-read (window_bootstrap.js:963-964 → `_p("vendor")`). Fixes Firefox-preset breakage + main/worker mismatch. | 1 hr | high | Firefox-impersonation sites; removes a 100%-reliable tell whenever a firefox_135_* preset is used | public |
| FIX-NAV-3 | Derive `screen.orientation.type/angle` from `screen.width vs height` (window_bootstrap.js:1433); fix mobile presets reporting landscape. Also nudge `window.screenY` to a menubar-consistent value. | 0.5 day | high | mobile presets (pixel/iphone) + CreepJS orientation cross-check | public |
| FIX-NAV-4 | Make `navigator.pdfViewerEnabled` profile-read (window_bootstrap.js:982) for main/worker symmetry. | 15 min | high | low (defensive; no current site) | public |
| FIX-NAV-5 | Align worker-vs-main *fallback defaults* (worker UA `Chrome/147`→`148`, window UA `Chrome/130`→`148`) so a no-profile run is coherent. | 15 min | high | none (cosmetic; production always has a profile) | public |
| FIX-NAV-6 | Extend the chip-coherent sampler (presets.rs:750) to Windows (Intel/AMD core↔RAM↔GPU pools) and Linux, mirroring the macOS chip model, to widen anti-clustering coverage. Depends on FIX-NAV-1. | 1 week | medium | AWS-WAF clustering across amazon-com/com-au (Windows persona variety) | public |

### Sequencing
1. FIX-NAV-1 first (highest impact, blocks the sampler from emitting tells; cheap).
2. FIX-NAV-2 + FIX-NAV-4 + FIX-NAV-5 in one "navigator reads profile, not
   constants" commit (an hour total).
3. FIX-NAV-3 (orientation) — needed before the mobile presets are trustworthy.
4. FIX-NAV-6 last (the big one; only worthwhile once 1-3 land and an A/B confirms
   the deviceMemory clamp alone didn't already flip the AWS cluster).

### What is NOT a gap (don't spend time here)
- Navigator scalar getter masking — done (window_bootstrap.js:1029-1035).
- `oscpu` absence — correct for Chrome.
- main/worker navigator value coherence — guaranteed by shared profile op (§3.1).
- `devicePixelRatio` placement (Window-only, not navigator) — already correct.
- `connection.rtt/downlink` quantization — already Chrome-accurate (25 ms / 0.025
  buckets, window_bootstrap.js:176-177).
- screen×DPR physical realism + chrome-bar math — already coherent (§3.2).

---

## 6. Open questions

- Does the AWS-WAF `challenge.js` actually read `navigator.deviceMemory`, or only
  the WebGL/timing surface? FIX-NAV-1 is cheap regardless, but the *site-flip*
  expectation needs a live capture (the offline oracle already proceeds — per
  HANDOFF_2026_05_28b §4 the AWS blocker is the live-nav async-drain, not a value
  tell, so FIX-NAV-1 may reduce score without flipping until the drain is fixed).
- Are the Windows/Linux static presets' `device_memory=8` + `cpu_cores=8` a
  *too-uniform* pair that AWS clusters on? (Needs the same A/B as FIX-NAV-6.)
- Should `screen.orientation` be a live `EventTarget` (Chrome fires `change`)? No
  corpus site is known to need it; deferred.

## 7. Files referenced
- `crates/js_runtime/src/js/window_bootstrap.js:94-121` (profile read helpers),
  `:961-1035` (navigator), `:1432-1485` (screen+window), `:1786-1844` (userAgentData),
  `:174-180` (connection), `:273-410` (plugins/mimeTypes)
- `crates/js_runtime/src/js/worker_bootstrap.js:86-159` (worker navigator)
- `crates/js_runtime/src/extensions/stealth_ext.rs:53-146` (`op_get_profile_value`)
- `crates/stealth/src/profile.rs:218-376` (`validate()` cross-field rules)
- `crates/stealth/src/presets.rs:39-203` (static presets), `:750-830` (chip sampler), `:850+` (mobile)
- `docs/releases/v0.1.0-parity/16_STEALTH_FINGERPRINT_AUDIT.md` §2.2
- `docs/releases/v0.1.0-parity/17_WEB_API_PARITY_MATRIX.md` §2.24-2.25
- `docs/releases/v0.1.0-parity/audit/03_HARDWARE_SPOOFING_DIFF.md`
- browserforge `fingerprints/generator.py` (Bayesian joint sampling)

Sources (external):
- [NavigatorUAData.getHighEntropyValues() — MDN](https://developer.mozilla.org/en-US/docs/Web/API/NavigatorUAData/getHighEntropyValues)
- [Bot Detection Triggered by Fingerprint Mismatches (UA vs UA-CH) — wilico.co.jp](https://wilico.co.jp/en/blog/browser-fingerprint-inconsistency-detection-consistency-check)
- [User-Agent reduction — MDN](https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/User-agent_reduction)
- DeepWiki: daijro/camoufox — Navigator & Screen Properties (NavigatorManager / ScreenDimensionManager / WorkerNavigator hooks)
