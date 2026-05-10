# W6a — DataDome Probe-Gap Matrix (2026-05-10)

> Continuation of `docs/RESEARCH_DATADOME_BYPASS_2026_05_10.md` (the
> landscape doc). This pass moves from "what DataDome is" to "what
> exactly its `tags.js` script reads, and where browser_oxide drifts
> from real Chrome 147 macOS arm64 on each probe." The output is the
> bridge between the public RE record and the engine work that closes
> the four remaining `DataDome-CHL` failures: **yelp.com**,
> **leboncoin.fr**, **etsy.com**, **wsj.com** (this last flipped
> mid-session, but is on the same code path).
>
> Methodology: live `curl` capture of `js.datadome.co/tags.js`
> (no browser, no automation, fresh Chrome-147 UA) → AST-style
> string-table extraction → cross-reference with our existing surface
> docs and source files. **No source modified, no tests run.**
>
> Key finding up front: the engine-side fingerprint pieces are mostly
> in place. The decisive failure mode is **(a)** lack of any captured
> mouse-move / pointer events at the moment of POST and **(b)** a
> small set of *ratio* and *render-stack* tells (canvas pixel
> coverage, OS/UA-vs-platform consistency in the Worker probe, our
> heavy `_maskAsNative` weight) that bias the per-tenant ML model
> toward "headless." Detail in §4 and §6.

---

## Table of contents

1. Live `tags.js` capture — size, structure, obfuscation pattern
2. Deobfuscation — string table, base64 dictionary, control flow
3. Probe surface enumeration — every read I could identify
4. Probe-gap matrix (the main table)
5. Picasso / canvas deep-dive — what they actually draw
6. Worker-context probe — the OffscreenCanvas WebGL block
7. Top-3 highest-impact gaps — with file:line evidence and fix plan
8. Appendix A — full decoded string table (231 entries)
9. Appendix B — file/line index for cross-references
10. Appendix C — sources

---

## 1 — Live capture summary

### 1.1 Capture command

```
curl -s -A "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36" \
https://js.datadome.co/tags.js > /tmp/datadome_tags.js
```

### 1.2 Artifact

| Field | Value |
|-------|-------|
| Bytes | 110 666 |
| Header comment | `/** DataDome is a cyberfraud solution to detect bot activity https://datadome.co (version 5.6.3) */` |
| Tag version | **5.6.3** (per header) — newest major; `RESEARCH_DATADOME_BYPASS_2026_05_10.md` cites 4.x/5.x as the live range |
| Bundler signature | `var DataDomeJsTag=(()=>{ ... })();` — Webpack 5-style IIFE module bundle |
| Source-map | absent (sourceMappingURL not present) |
| Polymorphism | None observed across two consecutive curls (~hours apart) — confirms `tags.js` is *not* per-request rotated, only the signal-key dictionary inside is rotated daily |

### 1.3 Outer wrapper structure

```
var DataDomeJsTag = (() => {
    function t(t) { ... }                  // ESM-default helper
    var Wt, Gt, n, r, l, v, T = {};        // module-scope refs

    function At(t) { return ... typeof ... }  // typeof helper

    function k() {                          // main constants/closures
        var e;
        return r || (
            r = 1,
            n = function() {
                this.dataDomeCookieName = "datadome",
                this.IECustomEvent = null,
                this.emptyCookieDefaultValue = ".keep",
                this.dataDomeStatusHeader = "x-dd-b",
                ...
                this.eventNames = { ready: "dd_ready", posting: "dd_post", ... },
                this.ChallengeType = { BLOCK: "block", HARD_BLOCK: "hard_block",
                                       DEVICE_CHECK: "device_check",
                                       DEVICE_CHECK_INVISIBLE_MODE: "device_check_invisible_mode" },
                ...
            }
        );
    }
    // 30+ more module-scope helpers t(), at(), At(), n(), N(), ...
    // each one is a small obfuscated routine
    ...
})();
```

The function-name pattern is **single-letter or two-letter
identifiers** (a, t, n, r, At, Wt, Gt, k, Bt, ct, Ct, dt, etc.).
Some routines (the `n` constructor above) are **partly readable**
because configuration constants like `dataDomeCookieName` and
`x-dd-b` are unobfuscated string literals — DataDome only obfuscates
the *probe* surface, not the *control* surface. This matches
glizzykingdreko's writeup: "configuration is plain, fingerprint
collection is hex/b64-encoded with a string table."

### 1.4 Obfuscation depth (vs Kasada and Akamai)

| Vendor | Mechanism | Static-analysis difficulty |
|--------|-----------|---------------------------|
| Kasada `ips.js` | Custom register VM, ~60 opcodes, bytecode rotates hourly | High — requires VM disassembly per fetch |
| Akamai `_acch.js` | Heavy obfuscation, opaque control flow, lookup tables | Medium-high |
| **DataDome `tags.js`** | **Single string table + base64-encoded names + minor control-flow flattening** | **Low-medium — static AST walk recovers ~all identifiers** |

This matches the research doc's verdict (`§3.3 Where DataDome differs
from Kasada`): less algorithmic depth, more breadth-of-signal. From
our engineering standpoint, this means the *fingerprint quality*
fight is entirely solvable by reading the string table and lining
up our shims — there is no VM to emulate.

---

## 2 — Deobfuscation pass

### 2.1 The string table

The script bundles all probe identifiers as **base64 literals** in a
single 231-element JS array. The table variable is named `it` and
sits at byte offset 36 311 of the captured script. First few entries:

```
it = ["Zmxvb3I","cGFyc2VJbnQ","bWVzc2FnZQ","c2xpY2U","T2JqZWN0",
      "bWF0Y2g","bQ","Cg","YXQg","cHVzaA", ...];
```

Decoded base64 of the first 10:

| Index | Base64 | Decoded |
|-------|--------|---------|
| 0 | `Zmxvb3I` | `floor` |
| 1 | `cGFyc2VJbnQ` | `parseInt` |
| 2 | `bWVzc2FnZQ` | `message` |
| 3 | `c2xpY2U` | `slice` |
| 4 | `T2JqZWN0` | `Object` |
| 5 | `bWF0Y2g` | `match` |
| 6 | `bQ` | `m` |
| 7 | `Cg` | `\n` |
| 8 | `YXQg` | `at ` (note the trailing space — for stack-trace parsing) |
| 9 | `cHVzaA` | `push` |

The complete decoded table (231 entries) is in **Appendix A**. It
includes every probe target identifier plus a number of *daily-rotated
six-character signal keys* (lowercase strings of the form `bcl`,
`cssH`, `phe`, `geb`, `dp0`, `bfr`, `hdn`, `mmt`, `orf`, `dvm`,
`sirv`, `npmtm`, `vco`, `vcmk`, `opr`, `ucdv`, `plggt`, `muev`, `prso`,
`idn`, `svde`, `vpbq`, `csssp`, `mq2`, `nisd`, `ihdn`, `xt1`, `eva`,
`ecpc`, `acmpu`, `acaa`, `acaats`, `acmp4ts`, `acmp3ts`, `acwmts`,
`ocpt`, `ac_NA`, `pltod`, `bid`, `ccsB`, `ccsH`, `nt_*`, `pw`, `isf2`,
`sgc`, `sel`). These map to the `jsData` field names listed in
`RESEARCH_DATADOME_BYPASS_2026_05_10.md §3.1` (`rs_h`, `rs_w`, `rs_cd`,
`phe`, `nm`, `wdr`, `ua`, `lg`, `lgs`, `plg`, `tzp`, `glrd`, `glvd`,
`str_ss`, `str_ls`, `str_idb`, `str_odb`, `hc`, `dm`, `pmf`, `bav`)
plus a much larger set we hadn't enumerated.

### 2.2 String resolver

Just before the `it = [...]` declaration the script seeds a 32×256
matrix `G` (line `for (var A=0; A<8; A++) ...`). This is a **lookup
table for the index→string resolver** — two functions wrap `it[i]`
with index permutations driven by `G`. The two callsites I see are:

```js
function O(i) { return atob(it[K[i]]); }   // (paraphrased; K is closure)
function Y(i) { return atob(it[i]); }
```

Both decode the base64 lazily on read. Static analysis can resolve
every `O(N)` and `Y(N)` callsite by walking `G` and the table. **The
control-flow obfuscation is shallow** — there's no VM dispatch loop,
no opaque predicate, no string concealment beyond base64. A Babel
plugin (glizzykingdreko's `Datadome-Deobfuscator`) collapses it
fully.

### 2.3 High-level structure

After the string table and resolver, the script does (recovered by
reading the un-obfuscated config blocks plus follow-on logic):

1. **`new n()` constructor** — declares cookie name (`datadome`),
   header names (`x-dd-b`, `x-sf-cc-x-dd-b`), event names
   (`dd_ready`, `dd_post`, `dd_post_done`, `dd_blocked`,
   `dd_response_displayed`, `dd_response_error`, `dd_response_passed`,
   `dd_response_unload`, `dd_captcha_displayed`, `dd_captcha_error`,
   `dd_captcha_passed`), challenge-type enum (`block`, `hard_block`,
   `device_check`, `device_check_invisible_mode`), and helpers
   `getCookie` / `setCookie` / `findCookiesByName` /
   `findDataDomeCookies` / `replaceCookieDomain`.
2. **Probe collection** — enumerates the §3 surface, building a kv map
   keyed by today's six-char names.
3. **Worker spawn** — see §6; off-thread WebGL renderer-string read,
   timezone read, navigator-replay read.
4. **Behavior collection** — installs `addEventListener` hooks for
   `mousemove`, `pointermove`, `click`, `scroll`, `touchstart`,
   `touchend`, `touchmove`, `keydown`, `keyup` (the 9-element array
   we recovered).
5. **Encrypt** — dual-XOR-PRNG over the kv buffer (per
   `RESEARCH_DATADOME_BYPASS_2026_05_10.md §4.3`).
6. **Submit** — POST to `https://api-js.datadome.co/js/?dd=<payload>`
   form-encoded; honour `Set-Cookie: datadome=...` from response.

### 2.4 Daily key rotation evidence

The 6-element array of per-fetch hashes
`["8FE0CF7F8AB30EC588599D8046ED0E", "87F03788E785FF301D90BB197E5803",
"765F4FCDDF6BEDC11EC6F933C2BBAF", "00D958EEDB6E382CCCF60351ADCBC5",
"E425597ED9CAB7918B35EB23FEDF90", "E425597ED9CAB7918B35EB23FEDF90"]` is
embedded in the script and rotates daily. These appear to be HMAC
seeds / hash-chain anchors used by the encryption pass (matches the
`hsh` server-issued field shape).

---

## 3 — Probe surface enumeration

Every probe identified in the captured `tags.js`. For each, the
column "Worker?" indicates whether the probe runs in the OffscreenCanvas
Worker (§6) or only on the main thread.

### 3.1 Navigator / window scalars

| Internal name | Source API | Worker? | Notes |
|---|---|---|---|
| `ua` | `navigator.userAgent` | yes | Read in main + worker; mismatch is a tell |
| `lg` | `navigator.language` | no | |
| `lgs` | `navigator.languages.length` | yes (worker reads `JSON.stringify(navigator.languages)`) | Both cardinality and content compared |
| `plg` | `navigator.plugins.length` | no | |
| `tzp` | `Date().getTimezoneOffset()` | no | |
| `tz` | `Intl.DateTimeFormat().resolvedOptions().timeZone` | yes | Worker's read must agree with main's |
| `wdr` | `navigator.webdriver` | no | Headless tell |
| `phe` (table[45]) | `window._phantom` | no | PhantomJS sentinel |
| `nm` | `window.__nightmare` | no | (table entry `__nightmare`) |
| `awesomium` | `window.awesomium` (from table[44]) | no | Legacy headless tell |
| `domAutomation` (table[46]) | `window.domAutomation` | no | ChromeDriver legacy |
| `domAutomationController` (table[58]) | `window.domAutomationController` | no | ChromeDriver |
| `__driver_unwrapped` (table[53]) | `window.__driver_unwrapped` | no | |
| `__webdriver_unwrapped` (table[54]) | `window.__webdriver_unwrapped` | no | |
| `__fxdriver_unwrapped` (table[55]) | `window.__fxdriver_unwrapped` | no | |
| `$cdc_asdjflasutopfhvcZLmcfl_` (table[56]) | `window[$cdc_asdjflasutopfhvcZLmcfl_]` | no | undetected-chromedriver tell |
| `__webdriverFunc` (table[57]) | `window.__webdriverFunc` | no | |
| `__webdriver_script_function` (table[59]) | `window.__webdriver_script_function` | no | Selenium |
| `__playwright_builtins__` (table[210]) | `window.__playwright_builtins__` | no | Playwright |
| `__playwright__binding__` (table[211]) | `window.__playwright__binding__` | no | Playwright |
| `__playwright__binding__controller__` (table[212]) | `window.__playwright__binding__controller__` | no | Playwright |
| `pplx-agent-0_0-overlay-stop-button` (table[213]) | `document.getElementById(...)` | no | Perplexity browser-agent extension |
| `__stagehandV3__` (table[215]) | `window.__stagehandV3__` | no | Stagehand AI agent |
| `claude-agent-animation-styles` (table[38]) | `document.getElementById(...)` | no | Claude Code browser agent |
| `data-browser-use-highlight` (table[39]) | DOM-element attribute scan | no | browser-use library |
| `data-browser-use-interaction-highlight` (table[40]) | DOM attribute scan | no | browser-use library |
| `__REACT_DEVTOOLS_GLOBAL_HOOK__` (table[128]) | `window.__REACT_DEVTOOLS_GLOBAL_HOOK__` | no | DevTools hook (informational only) |
| `chrome.runtime` (table[97]) | `window.chrome.runtime` | no | Should be **absent** on regular pages |
| `evaluate` / `eval at evaluate` (table[157,158]) | stack-trace string-match for CDP `Runtime.evaluate` | no | CDP tell — see §6 |

### 3.2 Screen / viewport

| Internal name | Source API | Notes |
|---|---|---|
| `rs_h` | `window.screen.height` | |
| `rs_w` | `window.screen.width` | |
| `rs_cd` (table[78]) | `window.screen.colorDepth` | typically 24 or 30 |
| `ars_h` (table[77]: `availHeight`) | `window.screen.availHeight` | |
| `ars_w` | `window.screen.availWidth` | |
| `cg_w` (table[79]) | (canvas-related width — cross-checks `screen.width` vs canvas area) | |
| `dpr` | `window.devicePixelRatio` (table[80]) | macOS retina = 2 |

### 3.3 Hardware / system

| Internal name | Source API | Worker? |
|---|---|---|
| `hc` (table[75]) | `navigator.hardwareConcurrency` | yes (worker reads `e.hc`) |
| `dm` | `navigator.deviceMemory` | no (deviceMemory is `[SecureContext]`-only and not exposed in worker_bootstrap on insecure) |
| `mob` | `navigator.userAgentData?.mobile` | yes (worker `e.mob`) |
| `pf` | `navigator.platform` | yes (worker `e.pf`) |
| `onL` | `navigator.onLine` | yes (worker `e.onL`) |

### 3.4 Storage support flags

| Internal name | Source API | Notes |
|---|---|---|
| `str_ss` | `'sessionStorage' in window` | |
| `str_ls` | `'localStorage' in window` | |
| `str_idb` | `'indexedDB' in window` | |
| `str_odb` | `'openDatabase' in window` | WebSQL — must be absent in modern Chrome |

### 3.5 Audio codec table (table entries 171-184)

For each MIME, the script calls `HTMLMediaElement.canPlayType(mime)`
and `MediaSource.isTypeSupported(mime)`. Combined into a 9-bit mask
field:

| Probe key | MIME |
|---|---|
| `acmpu` (table[173] `audio/mpegurl;`) | `audio/mpegurl` |
| `acaa` (`audio/aac;`) | `audio/aac` |
| `acaats` (`audio/aac;` — isTypeSupported variant) | `audio/aac` |
| `acmp4ts` (`audio/3gpp;`) | `audio/3gpp` |
| `acmp3ts` (`audio/flac;`) | `audio/flac` |
| `acwmts` (`audio/webm;`) | `audio/webm` |
| `ocpt` | `audio/ogg; codecs="opus"` (inferred) |
| `ac_NA` | sentinel for unsupported result |

Plus video codecs: `video/3gpp;`, `video/mpeg;` via the same probe
(table indices 93-95).

### 3.6 Display / viewport / matchMedia

| Internal name | Source API |
|---|---|
| `mq2` (table[152]) | `matchMedia("(color-gamut: p3)").matches` etc. |
| `display-mode` (table[155]) | `matchMedia("(display-mode: standalone)").matches` |
| `(display-mode: fullscreen)` (table[217]) | `matchMedia("(display-mode: fullscreen)").matches` |
| `color-gamut` (table[153]) | `matchMedia("(color-gamut: p3)")` and `... srgb`, `... rec2020` |

### 3.7 Performance timing fields (table entries 193-208)

The script reads `performance.getEntriesByType("navigation")[0]` and
maps the following fields to its own internal names:

| Internal | PerformanceNavigationTiming field |
|---|---|
| `nt_dns` | `domainLookupEnd - domainLookupStart` |
| `nt_rd` | `redirectEnd - redirectStart` |
| `nt_tls` | `secureConnectionEnd - secureConnectionStart` |
| `nt_ttf` | `responseStart - requestStart` (TTFB) |
| `nt_swt` | `workerStart > 0 ? responseStart - workerStart : 0` |
| `nt_csd` | `connectEnd - connectStart` |
| `nt_nhp` | `responseEnd - responseStart` |
| `nt_dcle` | `domContentLoadedEventStart - navigationStart` |

These are **monotonic-time deltas**, computed from the same
`PerformanceNavigationTiming` we already populate in
`crates/js_runtime/src/extensions/perf_ext.rs`. The values **must
form a non-decreasing chain** (`navigationStart ≤ requestStart ≤
responseStart ≤ responseEnd ≤ domInteractive ≤
domContentLoadedEventStart`); a flat-zero or out-of-order chain is
itself a tell.

### 3.8 Behavior events array

Captured event types (verbatim from table line `["mousemove",
"pointermove", "click", "scroll", "touchstart", "touchend",
"touchmove", "keydown", "keyup"]`):

| Event | Notes |
|---|---|
| `mousemove` | Most informative — coord deltas + timing jitter feed the 31-feature path scorer |
| `pointermove` | Same as mousemove plus `pointerType` |
| `click` | Single-event timestamp + coords |
| `scroll` | `scrollY` / `scrollX` deltas |
| `touchstart`/`end`/`move` | Touchscreen path |
| `keydown`/`keyup` | Includes `code` (per `keydown` cited in glizzykingdreko Medium) |

The `_initialCoordsList` is the array of `{x, y, t}` triples between
page load and the first real interaction. **An empty
`_initialCoordsList` is a stronger tell than a present-but-imperfect
one** (`RESEARCH_DATADOME_BYPASS_2026_05_10.md §6.5`).

### 3.9 Misc

| Internal name | Source |
|---|---|
| `vendor` (table[186]) | `navigator.vendor` |
| `bav` | `navigator.brave?.isBrave` (Brave shim probe) |
| `pmf` | `navigator.permissions.query` surface (call with `geolocation` etc., expect `PermissionStatus`) |
| `mediaDevices` (table[27]) | `navigator.mediaDevices.enumerateDevices()` for `audioinput` count |
| `mimeTypes` (table[73]) | `navigator.mimeTypes.length` and `enabledPlugin` cross-check (table[134]) |
| `hasFocus` (table[165]) | `document.hasFocus()` |
| `XMLDocument.prototype.hasStorageAccess` (table[120]) | feature-detect for `hasStorageAccess` |
| `process` (table[169]) | `window.process` (Node integration tell — Electron) |
| `opener` (table[170]) | `window.opener` |
| `spawn` (table[185]) | `window.process?.spawn` (Electron / pkg) |
| `default` (table[230]) | `Notification.permission === "default"` check |
| `bid` (table[189]) | `navigator.bluetooth?.getAvailability()` (boolean) |
| `pltod` (table[188]) | platform-OS deduction string |
| `systemLanguage` (table[190]) | `document.documentElement.lang` / IE legacy `navigator.systemLanguage` |
| `eva` / `evaluate` (157, 158) | new Error().stack string-match for `Runtime.evaluate` (CDP tell) |
| `pw` (table[209]) | "playwright" string match across globals |

### 3.10 What's notably ABSENT from the captured tags.js

I expected and did **not** find direct calls to:

- `OfflineAudioContext.prototype.startRendering` for the audio
  fingerprint hash (CreepJS-style 30k-sample triangle hash). The
  audio surface is reduced to **codec-table feature detect only**
  (the `acmpu`/`acaa`/`acaats`/... bitfield). Real audio rendering is
  not part of the main `tags.js` — it likely lives in the **interstitial
  challenge** (`interstitial.captcha-delivery.com/i.js`), which only
  loads after the score-fail. The version 5.6.3 main tag has
  *removed* the audio render hash from the silent-pass path.
- `CanvasRenderingContext2D.fillText` of a fixed test string. The
  Picasso operations also do not appear in the main tag — they are
  in the **interstitial challenge** payload. The main tag does
  feature-detect the existence of canvas APIs but does not render a
  reference image. (See §5 for what this means.)
- WASM `boring_challenge`. Not present in main tag.
- `RTCPeerConnection.createOffer()` IP-leak read. Not present.
- `navigator.getBattery()`. Not in this main tag (table contains
  `BatteryManager` only as part of the global-name enumeration of
  §3.11).

### 3.11 Global-namespace enumeration

Table entries 71-128 are a **list of globals to probe** via
`typeof globalThis[name]`:

```
DeprecationReportBody, MathMLElement, XMLHttpRequestEventTarget,
WritableStream, TextTrackCue, VisualViewport, StyleSheet,
SVGGeometryElement, RTCStatsReport, CropTarget, BatteryManager,
LaunchQueue, CSSFontPaletteValuesRule, ServiceWorkerContainer,
MozMobileMessageManager, CSS2Properties.prototype.MozOSXFontSmoothing,
ContentVisibilityAutoStateChangeEvent, PerformanceServerTiming,
VideoFrame, CSSCounterStyleRule, XMLDocument.prototype.hasStorageAccess,
CryptoKey, VideoPlaybackQuality, EventCounts, RTCError,
CSSCharsetRule, RTCPeerConnectionIceErrorEvent, MediaSourceHandle,
__REACT_DEVTOOLS_GLOBAL_HOOK__, ContactsManager, HTMLVideoElement,
XMLDocument
```

`MozMobileMessageManager` and `CSS2Properties.prototype.MozOSXFontSmoothing`
are **Firefox-only** — present-on-Chrome-UA is a tell. `BatteryManager`
is `[SecureContext]` and *should* be present on https Chrome-147
desktop. `ContactsManager` is Chrome Android only — present on a
desktop UA is a tell.

This is a `present?yes:no` 32-bit mask, not a render or computation —
each missing or extra global flips one bit. **This is the exact
shape we cover in `crates/js_runtime/src/js/interfaces_bootstrap.js`
(the 600+-element global-name list).**

---

## 4 — Probe-gap matrix

Severity legend:
- **HIGH** — engine returns a value that *cannot* match Chrome 147 on a
  real macOS arm64 user, or returns a structurally-incorrect shape
- **MEDIUM** — value plausibly matches but is fragile / order-sensitive
- **LOW** — minor detail, individually un-decisive

| # | Probe (DD internal) | Target API | Chrome 147 macOS arm64 returns | browser_oxide currently returns | Gap | Suggested fix | Effort |
|---|---|---|---|---|---|---|---|
| 1 | `ua` | `navigator.userAgent` | macOS Chrome 147 string | `_p("user_agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) ... Chrome/130.0.0.0 ...")` — **default profile says Win10 + Chrome 130** (`window_bootstrap.js:783`) | **MEDIUM** — only HIGH if profile not set; tests that pre-set a macOS UA are fine | Verify the holistic-sweep harness loads a macOS-arm64 stealth profile before navigation; add a panic if `os_name=macOS` but UA contains `Windows NT` | trivial — config audit |
| 2 | `pf` | `navigator.platform` | `"MacIntel"` | `_p("platform", "Win32")` (`window_bootstrap.js:784`) | **MEDIUM** — same as above | Same audit; assert UA-platform-OS triplet consistency | trivial |
| 3 | `lg` / `lgs` | `navigator.language` / `languages.length` | `"en-US"` / `2` (`["en-US", "en"]`) | `_p("language", "ru-RU")` / 2 from `_pJson("languages", ["en-US","en"])` (`window_bootstrap.js:792, 777`) | **HIGH if region mismatch** — `ru-RU` on a US Chrome UA visiting yelp.com is a strong mismatch | Profile-side fix: en-US default for US/global tenants, fr-FR for leboncoin.fr | trivial |
| 4 | `wdr` | `navigator.webdriver` | `false` (own descriptor on Navigator.prototype) | `false` via `Object.defineProperty(Navigator.prototype, 'webdriver', { get: ()=>false })` (`window_bootstrap.js:802-806`) | **MATCH** | none | — |
| 5 | `phe` | `window._phantom` | `undefined` | `undefined` (we don't set it) | **MATCH** | none | — |
| 6 | `nm` | `window.__nightmare` | `undefined` | `undefined` | **MATCH** | none | — |
| 7 | `chrome.runtime` | presence | absent on regular pages | absent on regular pages (`window_bootstrap.js:1014-1100`) | **MATCH** | none | — |
| 8 | `__driver_unwrapped`, `__webdriver_unwrapped`, `__fxdriver_unwrapped`, `$cdc_asdjflasutopfhvcZLmcfl_`, `__webdriverFunc`, `domAutomationController`, `__webdriver_script_function` | window props | all `undefined` | all `undefined` (we do not introduce them; nothing in `window_bootstrap.js` mentions any of these names) | **MATCH** | none | — |
| 9 | `__playwright_builtins__`, `__playwright__binding__`, `__playwright__binding__controller__`, `__stagehandV3__`, `pplx-agent-0_0-overlay-stop-button`, `claude-agent-animation-styles`, `data-browser-use-highlight`, `data-browser-use-interaction-highlight` | window props / DOM scans | all `undefined` / no matching elements | all `undefined` | **MATCH** | none — these target other agents, not us | — |
| 10 | `evaluate` / `eva` (CDP detection) | stack-trace match for `Runtime.evaluate` in injected scripts | absent — Chrome user has no devtools attached | absent — we don't speak CDP at all (no Runtime.evaluate frames ever surface) | **MATCH (structural win)** | none | — |
| 11 | `rs_h`, `rs_w`, `rs_cd`, `ars_h`, `ars_w` | `screen.*` | typical macOS retina: `1117 / 1728 / 30 / 1117 / 1728` (or 24-bit colorDepth on non-XDR) | profile-driven via `screen_*` keys; `_pInt("color_depth", 24)` (default); width/height come from window-size profile | **MEDIUM** — `colorDepth: 24` is plausible; macOS reports `30` on XDR displays and `24` otherwise — both are valid Chrome reads | Confirm the sweep profile sets `screen_width=1728, screen_height=1117, color_depth=30` for macOS arm64 | trivial |
| 12 | `dpr` | `window.devicePixelRatio` | `2` (retina) | profile-driven (`device_pixel_ratio`) | **MEDIUM** — fragile if profile leaves at 1 | Set to 2 for macOS arm64 profiles | trivial |
| 13 | `tzp` | `Date().getTimezoneOffset()` | per-locale: `420` for PDT, `-60` for CET, etc. | V8 intrinsic — patched by `Intl.DateTimeFormat.resolvedOptions()` patch (`window_bootstrap.js:1737+`) but **does V8's native `Date.prototype.getTimezoneOffset` agree?** Per `CHROME_JS_SURFACE_PARITY_2026_04_29.md:286`, this is flagged as PARTIAL | **HIGH** — `Date` and `Intl` divergence is a 2-line cross-check probe and the doc explicitly notes risk of disagreement | Add explicit `Date.prototype.getTimezoneOffset` patch to honor profile timezone; assert `Intl.DateTimeFormat().resolvedOptions().timeZone` and `new Date().getTimezoneOffset()` agree | small |
| 14 | `tz` | `Intl.DateTimeFormat().resolvedOptions().timeZone` | IANA name e.g. `"America/Los_Angeles"` | profile-patched (`window_bootstrap.js:1737-1786`) | **MATCH** if profile set | none | — |
| 15 | `plg` | `navigator.plugins.length` | typically `5` on macOS Chrome 147 (PDF viewer plugins) | `_navPlugins.length` from `window_bootstrap.js:155-220` — defines a 5-element array | **MATCH** | verify length is 5 not 0 | — |
| 16 | `mimeTypes` len | `navigator.mimeTypes.length` | `2` on Chrome (PDF MIME types) | from same shim, length 2 | **MATCH** | — | — |
| 17 | `hc` | `navigator.hardwareConcurrency` | typically `8` or `12` | `_pInt("hardware_concurrency", 8)` (`window_bootstrap.js:796`) | **MATCH** | — | — |
| 18 | `dm` | `navigator.deviceMemory` | `8` (privacy-bucketed) | `_pInt("device_memory", 8)` (`window_bootstrap.js:799`) | **MATCH** | — | — |
| 19 | `mob` | `navigator.userAgentData.mobile` | `false` | `false` (`window_bootstrap.js:1421+`) | **MATCH** | — | — |
| 20 | `bav` | `navigator.brave?.isBrave` | `undefined` (Chrome doesn't have brave shim) | `undefined` (we don't add `navigator.brave`) | **MATCH** | — | — |
| 21 | `str_ss` / `str_ls` / `str_idb` | `'sessionStorage' in window` etc. | `true` / `true` / `true` | `true` for all three (we ship Storage and indexedDB shims) | **MATCH** | — | — |
| 22 | `str_odb` | `'openDatabase' in window` | **`false`** on Chrome 116+ (WebSQL removed) | `false` (we don't expose `openDatabase`) | **MATCH** | — | — |
| 23 | `glvd` (worker WebGL) | `getParameter(UNMASKED_VENDOR_WEBGL)` from OffscreenCanvas in **Worker** | `"Google Inc. (Apple)"` | per `canvas_bootstrap.js:380`, default `"Google Inc. (Apple)"`; profile-overridable via `webgl_unmasked_vendor` | **MATCH** if profile set | verify Worker isolate also gets this profile (it does — `runtime.rs:253` `dom_state.stealth_profile = profile.clone()`) | — |
| 24 | `glrd` (worker WebGL) | `getParameter(UNMASKED_RENDERER_WEBGL)` from OffscreenCanvas in **Worker** | `"ANGLE (Apple, ANGLE Metal Renderer: Apple M3, Unspecified Version)"` | per `canvas_bootstrap.js:381`, exactly the same default | **MATCH** | — | — |
| 25 | Worker `tz` (Intl) | `Intl.DateTimeFormat().resolvedOptions().timeZone` inside Worker | IANA name | `worker_bootstrap.js:34-52` patches `Intl.DateTimeFormat.prototype.resolvedOptions` to overlay `profile.timezone` | **MATCH** | — | — |
| 26 | Worker `e.lgs` | `JSON.stringify(navigator.languages)` inside Worker | `'["en-US","en"]'` | `worker_bootstrap.js:60` reads `_pJson("languages", ["en-US","en"])` | **MATCH** | — | — |
| 27 | Worker `e.pf` | `navigator.platform` inside Worker | `"MacIntel"` | `worker_bootstrap.js:61` `_p("platform", "Win32")` — **same Win32 default as main thread** | **MEDIUM** — only fails if the macOS profile isn't loaded into the Worker isolate. `runtime.rs:253` does propagate it, so this should be fine | confirm via diagnostic test that Worker reports `MacIntel` when main reports `MacIntel` | trivial |
| 28 | Worker `e.ua` | `navigator.userAgent` inside Worker | macOS Chrome 147 UA | `worker_bootstrap.js:57` reads same profile key | **MATCH** | confirm cross-thread consistency | — |
| 29 | Worker `e.hc` | `navigator.hardwareConcurrency` | `8` | `worker_bootstrap.js:64` `_pInt("hardware_concurrency", 8)` | **MATCH** | — | — |
| 30 | Worker `e.mob` | `navigator.userAgentData.mobile` | `false` | **GAP** — `worker_bootstrap.js` does not define `userAgentData`, only the flat navigator scalars. The DD worker probe does `navigator.userAgentData?navigator.userAgentData.mobile:"NA"` so it will return `"NA"` | **HIGH** — real Chrome Worker context exposes `navigator.userAgentData` with the same surface as main; getting `"NA"` from Worker while main has it is a **direct cross-thread mismatch tell** | Port the `navigator.userAgentData` shim from `window_bootstrap.js:1421-1547` into `worker_bootstrap.js`. Same `brands`/`mobile`/`platform` low-entropy fields. | small (~30 lines) |
| 31 | Worker `e.onL` | `navigator.onLine` | `true` | `worker_bootstrap.js:62` `true` | **MATCH** | — | — |
| 32 | `nt_*` (PerformanceNavigationTiming) | various deltas | non-zero monotonic-increasing chain | `crates/js_runtime/src/extensions/perf_ext.rs` populates a `PerformanceNavigationTiming` entry; values come from real timing on `Page::navigate` | **MEDIUM** — ensure `workerStart`, `redirectStart`, `secureConnectionEnd-Start` all have plausible non-zero values for an HTTPS navigation. The recent commit `feat(stealth): dynamic PerformanceResourceTiming and SharedArrayBuffer parity` (visible in git log, last week) addressed this | spot-check fields against a real Chrome PerfNavTiming on the same URL | small |
| 33 | `pmf` | `navigator.permissions.query` | returns `PermissionStatus` with the documented state map | `window_bootstrap.js:303-360` (per parity doc), correct state map and TypeError | **MATCH** | — | — |
| 34 | `mediaDevices.enumerateDevices()` | list of `audioinput`/`videoinput`/`audiooutput` | typically 1-2 audioinputs, 1-2 audiooutputs | shim returns empty array (per past inventory) | **MEDIUM** — empty enumeration on a desktop Chrome UA is a tell; CreepJS counts | Return at least one fake `audioinput` (`{deviceId:"", kind:"audioinput", label:"", groupId:""}`) to match Chrome's "no permission yet" surface | small |
| 35 | Audio codec table (`acmpu`/`acaa`/`acaats`/`acmp4ts`/`acmp3ts`/`acwmts`/`ocpt`) | `HTMLMediaElement.canPlayType` + `MediaSource.isTypeSupported` for `audio/mpegurl`, `audio/aac`, `audio/3gpp`, `audio/flac`, `audio/webm`, `audio/ogg;codecs="opus"` | `"probably"`/`"maybe"`/`""` per Chrome's table | `window_bootstrap.js:4753-4794` covers the canonical table; verify all 6 audio MIMEs return Chrome's exact values | **MATCH (per parity doc)** but **VERIFY** specifically `audio/3gpp` and `audio/mpegurl` which are unusual probes | quick test via `datadome_diagnostic_capture` | trivial |
| 36 | Video codec table (`video/3gpp`, `video/mpeg`) | `canPlayType` | `""` for `video/mpeg`, `"probably"` for `video/3gpp; codecs="mp4v.20.8, samr"` on macOS | same shim | **MATCH but verify** | — | — |
| 37 | matchMedia `(color-gamut: p3)` | `MediaQueryList.matches` | `true` on macOS XDR / P3 displays, `false` otherwise | `window_bootstrap.js` `matchMedia` shim — **need to verify it returns sensible answers for `color-gamut: p3` and `display-mode: standalone`/`fullscreen`** | **HIGH** — if our shim returns `false` for everything (default) and our profile says macOS-XDR, the cross-check `dpr=2 + colorDepth=30 + color-gamut: p3 → false` is a tell | Audit `matchMedia` to handle `color-gamut`, `display-mode`, `prefers-color-scheme`, `prefers-reduced-motion` queries with profile-driven answers | small |
| 38 | `display-mode: standalone` / `fullscreen` | matchMedia | `false` (regular tab) | should be `false` | **MATCH** | verify | — |
| 39 | `hasFocus` | `document.hasFocus()` | `true` on a focused tab | likely `true` (we ship `document.hasFocus`); verify | **MEDIUM** | confirm — our holistic test simulates a focused tab via stealth_bootstrap | trivial |
| 40 | `vendor` | `navigator.vendor` | `"Google Inc."` | `_defNav('vendor', () => "Google Inc.")` (`window_bootstrap.js:785`) | **MATCH** | — | — |
| 41 | `process`, `opener`, `spawn` | window props | `undefined` (regular browser tab; no Electron) | `undefined` | **MATCH** | — | — |
| 42 | Global-name typeof probe (table 71-128) | `typeof globalThis[name]` for ~30 names | per Chrome 147: `MozMobileMessageManager: undefined`, `CSS2Properties.MozOSXFontSmoothing: undefined` (these are FF-only); `BatteryManager`, `CryptoKey`, `LaunchQueue`, `VisualViewport`, `ContactsManager`, `RTCPeerConnectionIceErrorEvent` etc. all `function` | per `interfaces_bootstrap.js:53` we ship a 600+ element list of global names. Need to verify *each* of the 30 DD-probed names is present (or absent for FF-only) | **MEDIUM** — most are present; missing one shows up as one off-bit and accumulates | Add a regression test that explicitly probes the 30 DD names and asserts the bitmask matches a captured Chrome 147 bitmask | small |
| 43 | `XMLDocument.prototype.hasStorageAccess` | feature detect | `function` | `dom_bootstrap.js` likely covers `XMLDocument`; `hasStorageAccess` is a `Document` method (added Chrome 113); verify on `XMLDocument.prototype` | **GAP (likely)** — `Document` has it, `XMLDocument` extends Document so should inherit it | grep `hasStorageAccess` in source — if absent on `XMLDocument`, add it as a no-op returning `Promise.resolve(false)` | trivial |
| 44 | `_initialCoordsList` (behavior) | accumulated `mousemove` events between page-load and POST | non-empty array of `{x, y, t}` triples on a real user; the 31-feature path scorer expects curvature, length, straightness | **GAP** — our `crates/stealth/src/behavior.rs` exists but per `RESEARCH_DATADOME_BYPASS_2026_05_10.md §9.6` "If our behavior crate doesn't synthesize at least a few mouse moves before any click-equivalent event, the empty list itself is a fail signal." We currently do not auto-fire `mousemove` events on page load | **HIGH (this is the decisive gap)** | See §7.1 below | medium |
| 45 | Picasso canvas (interstitial-only) | server-driven canvas op replay → pixel hash | per OS/GPU bit-exact hash | not run from main `tags.js` — only in `interstitial.captcha-delivery.com/i.js` after score-fail | **deferred** — only relevant once score-fail triggers interstitial | see §5 | — |
| 46 | Audio fingerprint (interstitial-only) | `OfflineAudioContext.startRendering` of `DynamicsCompressorNode` | per-OS hash | not run from main `tags.js` | **deferred** | — | — |
| 47 | `Function.prototype.toString` mask weight | `_origFnToStr.toString()` + repeated calls counted | one native `toString` shim | per `stealth_bootstrap.js:33-37`, the explicit re-assignment of `Function.prototype.toString` is **commented out** — only per-function `_maskFunction` patches are applied (lines 41-86). This means `Function.prototype.toString.call(myFn)` for a *patched* function returns `myFn.toString.call(this)` if `toString` is shadowed on `myFn` (which `_maskFunction` does) | **MATCH currently** — we don't override `Function.prototype.toString` globally, only per-function. `Function.prototype.toString.toString()` returns the genuine native string from V8 | confirm in `perimeterx_surface_parity.rs` | — |

### 4.1 Aggregate

| Severity | Count |
|----------|-------|
| HIGH | **3** (rows 13, 30, 37, 44 — labelled HIGH in column) |
| MEDIUM | 10 |
| LOW / MATCH | balance |

The HIGH-severity rows are: timezone Date/Intl divergence (#13),
Worker `userAgentData` absence (#30), `matchMedia` color-gamut /
display-mode answers (#37), and **empty `_initialCoordsList` /
behavior events** (#44).

---

## 5 — Picasso / canvas deep-dive

### 5.1 What the captured tags.js does NOT do

The version-5.6.3 main tag does **not** issue Picasso draw
instructions. There is no `fillText("Soft Ruddy Foothold 2", ...)`,
no `bezierCurveTo`, no fixed test image. The only canvas-related
references in the main tag are:

- The OffscreenCanvas WebGL probe in §6 — reads renderer/vendor
  strings only (no rasterization).
- Codec MIME tests (§3.5) — no canvas.
- A `Path2D`/`SVG` reference in the global-name table — typeof check.

This means the *silent-pass* path does NOT depend on canvas
rasterization parity. The 4 sites failing now are not failing
because of Picasso.

### 5.2 What the interstitial DOES do (out of scope for silent pass)

If/when score-fail triggers the interstitial, `i.js` from
`interstitial.captcha-delivery.com` *does* do Picasso canvas. From
the research doc:
- Server sends canvas-op script.
- Client replays.
- Pixel buffer hashed.
- Hash sent in `ddCaptchaEncodedPayload`.

That's a separate code path. **Closing the silent-pass gap removes
the trigger that loads interstitial in the first place**, so the
canvas rasterization fight is deferred until the silent-pass fight
is won.

### 5.3 Our current canvas pipeline (for completeness)

| Layer | File:line | What it does |
|-------|-----------|--------------|
| Rust 2D backend | `crates/canvas/src/canvas2d.rs:1-1850` | tiny-skia raster; `fillText` via `text/` shaper; deterministic PNG encode (`to_png_bytes` lines 1098-1114, `Paeth` filter pinned, `flate2/zlib-rs` for byte-stable IDAT) |
| Jitter | `crates/canvas/src/canvas2d.rs:1047-1084` | `to_data_url_with_jitter` — PCG32-style PRNG (state `seed.wrapping_add(0x9E3779B97F4A7C15)`, 5% pixel perturbation by ±1 LSB). Triggered on every `toDataURL` call |
| Op layer | `crates/js_runtime/src/extensions/canvas_ext.rs:357-390` | `op_canvas_to_data_url` — also applies a *separate* jitter pass with `0x9e3779b9u32` seed (line 367). **Two jitter passes** stack |
| JS shim | `crates/js_runtime/src/js/canvas_bootstrap.js:902` | `toDataURL(type)` calls the op |
| WebGL params | `crates/js_runtime/src/js/canvas_bootstrap.js:357-457` | profile-driven `_g()` cache; defaults to Chrome 147 macOS arm64 strings |

### 5.4 The double-jitter concern

The `op_canvas_to_data_url` code path (lines 357-390 of
`canvas_ext.rs`) runs jitter, but then calls
`c.to_data_url_with_jitter()` (line 387) which ALSO jitters. The
inner jitter actually re-fetches `get_image_data` from the
canvas (line 1048 of `canvas2d.rs`), discarding the outer jitter
pass entirely — so only one jitter actually lands. Verify with a
test that `toDataURL` returns a stable but Chrome-distinct hash. (The
two-pass structure is dead code in the outer ext; the actual jitter
is the inner PCG32 in `canvas2d.rs:1047-1084`.) This is **not** a
DataDome blocker (since DD doesn't read canvas in the silent path),
but flagging for cleanup.

### 5.5 Picasso readiness assessment

If/when we need it (interstitial path):
- **Skia rasterization**: tiny-skia (Apache-2.0) is *not* the same
  Skia that Chrome ships. Glyph metrics, AA edge handling, gradient
  interpolation differ in the LSBs. Bit-exact parity with Chrome
  Skia for arbitrary draw scripts is **not achievable** with
  tiny-skia. To achieve Picasso-grade parity we would need to ship
  a real Chrome-compiled Skia (CIPD blob, ~70 MB) or precompute
  expected hashes per OS profile.
- **Workaround**: since DD's Picasso seeds are server-issued and
  client-replayed, we *could* intercept the seed → look up a
  pre-captured hash from a real Chrome 147 macOS arm64 capture →
  return that. Requires a hash-map of seed→hash captures (a few
  hundred entries to cover the seed space). This is the **same
  technique** the public bypass services use.

---

## 6 — Worker-context probe (the OffscreenCanvas WebGL block)

### 6.1 The captured worker code

Extracted from line `q[O(246)]&&q[Y(21)]&&q[O(247)]?(r=new Blob([...`
of the captured tag (offset ~57k of `/tmp/datadome_tags.js`):

```js
function e(e,t){
  return function(){
    var n=Array.prototype.slice.call(arguments), r=[t];
    return new Promise(function(t,r){
      try{ var a=e.apply(null,n);
           a&&"function"==typeof a.then ? a.then(t).catch(r) : t(a)
      }catch(e){ r(e) }
    }).then(function(e){r[1]=e})
      .catch(function(e){r[2]="Err: "+e})
      .then(function(){self.postMessage(r)})
  }
}

// Probe 0: navigator scalars in worker realm
e(function(){
  var e={};
  return e.ua  = navigator.userAgent,
         e.hc  = navigator.hardwareConcurrency,
         e.pf  = navigator.platform,
         e.mob = navigator.userAgentData ? navigator.userAgentData.mobile : "NA",
         e.lgs = JSON.stringify(navigator.languages),
         e.onL = navigator.onLine,
         e
}, 0)();

// Probe 1: WebGL UNMASKED_RENDERER / UNMASKED_VENDOR via OffscreenCanvas
e(function(){
  var e, t, n = new OffscreenCanvas(1,1).getContext("webgl"),
      r = /Firefox\/(\d+)/.exec(navigator.userAgent);
  if (r && r[1] > 91) {
    e = n.VENDOR; t = n.RENDERER;          // FF92+ unmasked by default
  } else {
    var a = n.getExtension("WEBGL_debug_renderer_info");
    e = a.UNMASKED_VENDOR_WEBGL; t = a.UNMASKED_RENDERER_WEBGL;
  }
  return [n.getParameter(e), n.getParameter(t)];
}, 1)();

// Probe 2: timezone in worker realm
e(function(){
  if (self.Intl && self.Intl.DateTimeFormat &&
      "function"==typeof self.Intl.DateTimeFormat.prototype.resolvedOptions)
    return self.Intl.DateTimeFormat().resolvedOptions().timeZone || "NA";
}, 2)();
```

The Blob is then materialized into a Worker URL via
`URL.createObjectURL(blob)` and a `new Worker(url)` is spawned. The
Worker posts back three results indexed `[0]`, `[1]`, `[2]`.

### 6.2 Implications

This is a **cross-realm consistency check**:
- main thread `navigator.userAgent` must equal worker
  `navigator.userAgent`
- main thread WebGL `UNMASKED_RENDERER_WEBGL` must equal worker
  WebGL `UNMASKED_RENDERER_WEBGL`
- main thread `Intl.DateTimeFormat().resolvedOptions().timeZone`
  must equal worker's
- main thread `navigator.userAgentData.mobile` must equal worker's
  (or both are "NA" — but Chrome 147 has `userAgentData`, so "NA"
  in the worker is itself a tell when the main thread reports a
  proper `false`)

### 6.3 Our coverage

| Field | Main thread | Worker thread | Match? |
|-------|-------------|---------------|--------|
| `userAgent` | `_p("user_agent", ...)` | `_p("user_agent", ...)` | YES (same profile key) |
| `hardwareConcurrency` | `_pInt("hardware_concurrency", 8)` | `_pInt("hardware_concurrency", 8)` | YES |
| `platform` | `_p("platform", "Win32")` | `_p("platform", "Win32")` | YES |
| `userAgentData.mobile` | shim returns `false` | **MISSING — `worker_bootstrap.js` does not define `navigator.userAgentData`** | **NO** — main returns `false`, worker returns `"NA"` |
| `languages` | `_pJson("languages", ...)` | `_pJson("languages", ...)` | YES |
| `onLine` | `true` | `true` | YES |
| WebGL `UNMASKED_RENDERER` | profile-driven `webgl_unmasked_renderer` | same | YES (Worker isolate gets `dom_state.stealth_profile = profile.clone()` via `runtime.rs:253`, and `canvas_bootstrap.js` is loaded into worker per `runtime.rs:322`) |
| `Intl.DateTimeFormat().resolvedOptions().timeZone` | profile-patched | profile-patched (`worker_bootstrap.js:34-52`) | YES |

So the **single Worker-realm consistency gap** is the missing
`navigator.userAgentData` shim in the worker. This is **HIGH
severity**.

---

## 7 — Top-3 highest-impact gaps

### 7.1 Gap A — Empty `_initialCoordsList` / no behavior events

**What's broken**: When `tags.js` runs and fires its first POST
to `api-js.datadome.co/js/`, our page has logged ZERO `mousemove`,
ZERO `pointermove`, ZERO `scroll` events. The 31-feature behavior
scorer in DataDome's per-tenant ML model trained on years of real
Yelp/Etsy/Leboncoin traffic strongly weights "no behavior" toward
"bot." Even a perfect device fingerprint can't compensate for a
flat-line behavior profile on a high-tier tenant.

**Where to fix**:
- `crates/stealth/src/behavior.rs` (exists already, per
  `RESEARCH_DATADOME_BYPASS_2026_05_10.md §9.6`). Audit what it
  does on `Page::navigate`. Ensure it fires:
  1. 5-10 `mousemove` events along a Bezier curve from a random
     entry point to a content-area centroid, with 30-80 ms gaps
     and Gaussian-jittered velocity.
  2. 1-2 `scroll` events at 200-500 ms after page load.
  3. A `pointermove` mirror for each `mousemove` (Chrome fires both).
  4. Real synthetic `MouseEvent` and `PointerEvent` instances
     dispatched via the actual event-loop, so our
     `crates/js_runtime/src/extensions/input_ext.rs` ops fire,
     `addEventListener` callbacks register, and `tags.js`'s
     `mousemove` listener captures them.

**How to verify**:
- Re-run `crates/browser/tests/chrome_compat.rs::datadome_diagnostic_capture`
  against a DD-protected URL and dump the captured `jsData` payload.
  Confirm the `events` array has ≥5 entries with non-trivial deltas.
- Re-run holistic sweep against etsy.com first (lowest tier) — if
  it flips from `DataDome-CHL` to `OK`, behavior was the dominant
  signal.

**Effort**: 1 day. The behavior crate exists; we're extending its
default invocation policy. No new ops.

### 7.2 Gap B — `navigator.userAgentData` missing in Worker realm

**What's broken**: When DataDome's Worker probe (§6.1) reads
`navigator.userAgentData ? navigator.userAgentData.mobile : "NA"`,
it gets `"NA"` because our `worker_bootstrap.js` does not define
the `userAgentData` shim. The main-thread `tags.js` separately
reads `navigator.userAgentData.mobile` and gets `false`. The
mismatch (main says `false`, worker says `"NA"`) is a direct
cross-realm fingerprint contradiction.

**Where to fix**:
- `crates/js_runtime/src/js/worker_bootstrap.js:54-77` — extend the
  `workerNavigator` object with a `userAgentData` field that
  mirrors the main-thread shim. Specifically, port the low-entropy
  `brands` / `mobile` / `platform` getters from
  `crates/js_runtime/src/js/window_bootstrap.js:1421-1547` (the
  full Client Hints shim), but keep it Worker-safe (no DOM
  references). The high-entropy `getHighEntropyValues` async method
  can stay since it's pure async + profile reads.

```js
// Sketch (do NOT implement now — research output only):
workerNavigator.userAgentData = {
    brands: [/* same low-entropy list as main */],
    mobile: false,
    platform: _p("platform", "Win32") === "MacIntel" ? "macOS" : ...,
    getHighEntropyValues: async (hints) => { /* same impl */ },
};
Object.defineProperty(workerNavigator.userAgentData, Symbol.toStringTag,
    { value: "NavigatorUAData", configurable: true });
```

**How to verify**:
- Add an assertion to `datadome_diagnostic_capture` that captures
  the Worker-result `[0][1]` array (the `e.mob` field) and asserts
  it equals `false` (matching the main thread).
- Spawn a Worker manually inside the test and call
  `navigator.userAgentData.mobile` — assert non-undefined.

**Effort**: small (~30 lines). No new ops.

### 7.3 Gap C — `Date.prototype.getTimezoneOffset` may not honor profile timezone

**What's broken**: Per `CHROME_JS_SURFACE_PARITY_2026_04_29.md:286`,
"If the V8 startup snapshot froze stock `Date` while `Intl` got
monkey-patched, the two will disagree." The `tzp` probe reads
`new Date().getTimezoneOffset()` (V8 intrinsic, NOT patched by
us). The `tz` probe reads `Intl.DateTimeFormat().resolvedOptions().timeZone`
(patched by us via
`crates/js_runtime/src/js/window_bootstrap.js:1737-1786`). If we
load a `Europe/Paris` profile, `Intl` returns `"Europe/Paris"` but
V8's native `Date` returns whatever the host system reports
(probably UTC or the build host's TZ). Cross-check fails — DD
flags as inconsistent.

**Where to fix**:
- `crates/js_runtime/src/js/window_bootstrap.js` — patch
  `Date.prototype.getTimezoneOffset`, `Date.prototype.toString`,
  `Date.prototype.toLocaleString`, `Date.prototype.toTimeString`,
  `Date.prototype.toDateString`, and the `Date` constructor's
  string-parsing path to all honor the profile timezone.
- Mirror the patch in `worker_bootstrap.js` (Worker realm has its
  own `Date`).
- Compute the offset by passing the profile's IANA name through a
  small TZ-database lookup. `chrono-tz` (in our existing Cargo deps
  via `time/chrono`) handles this.

**How to verify**:
- Add a unit test:
  ```js
  const tz = Intl.DateTimeFormat().resolvedOptions().timeZone;
  // expect tz === profile.timezone
  const offset = new Date().getTimezoneOffset();
  // expect offset matches tz at this moment
  ```
- Re-run holistic sweep against leboncoin.fr (whose Paris-based
  tenant is the most likely to have tight TZ scoring).

**Effort**: medium (~150 lines + one chrono-tz lookup helper). The
patch is mechanical but spans a few `Date` prototype methods.

### 7.4 Honorable mention — `matchMedia` color-gamut answers

`window.matchMedia('(color-gamut: p3)').matches` should return
`true` on macOS XDR retina (which our profile claims). If our shim
returns `false` by default, the cross-check `colorDepth=30 +
dpr=2 + color-gamut: p3=false` is inconsistent — XDR displays
imply P3 support. Audit `matchMedia` in `window_bootstrap.js` and
return profile-derived answers for the four queries DataDome
probes: `(color-gamut: p3)`, `(color-gamut: srgb)`,
`(display-mode: standalone)`, `(display-mode: fullscreen)`.

---

## 8 — Appendix A — Full decoded string table (231 entries)

```
[0] "floor"                    [1] "parseInt"               [2] "message"
[3] "slice"                    [4] "Object"                 [5] "match"
[6] "m"                        [7] "\n"                     [8] "at "
[9] "push"                    [10] "detail"                [11] "initCustomEvent"
[12] "atan2"                  [13] "isNaN"                 [14] "TypeError"
[15] "Invalid attempt to iterate non-iterable instance.\nIn order to be iterable, non-array objects must have a [Symbol.iterator]() method."
[16] "Boolean"                [17] "valueOf"               [18] "Set"
[19] "^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$"           [20] "p"
[21] "URL"                    [22] "_datadome-det-cd"      [23] "nosup"
[24] "OuterErr: "             [25] "query"                 [26] "catch"
[27] "mediaDevices"           [28] "audioinput"            [29] "deviceId"
[30] "k:"                     [31] " g:"                   [32] "bcl"
[33] "level"                  [34] "architecture"          [35] "model"
[36] ","                      [37] "k_lyts"                [38] "claude-agent-animation-styles"
[39] "data-browser-use-highlight"
[40] "data-browser-use-interaction-highlight"
[41] "_"                      [42] "hasAttribute"          [43] "addedNodes"
[44] "awesomium"              [45] "phe"                   [46] "domAutomation"
[47] "__nightmare"            [48] "geb"                   [49] "S"
[50] "sin"                    [51] "cHhzaWQ="              [52] "exp19"
[53] "__driver_unwrapped"     [54] "__webdriver_unwrapped" [55] "__fxdriver_unwrapped"
[56] "$cdc_asdjflasutopfhvcZLmcfl_"  [57] "__webdriverFunc"
[58] "domAutomationController"
[59] "__webdriver_script_function"   [60] "addEventListener"
[61] "cache_"                 [62] "clearInterval"         [63] "dp0"
[64] "createElement"          [65] "style"                 [66] "px"
[67] "setProperty"            [68] "color"                 [69] "removeChild"
[70] "cssH"                   [71] "bfr"                   [72] "hdn"
[73] "mimeTypes"              [74] "mmt"                   [75] "hardwareConcurrency"
[76] "orf"                    [77] "availHeight"           [78] "rs_cd"
[79] "cg_w"                   [80] "devicePixelRatio"      [81] "so"
[82] "dvm"                    [83] "dddd"                  [84] "sirv"
[85] "dd_testcookie=; expires=Thu, 01 Jan 1970 00:00:00 UTC; path=/; SameSite=None; Secure"
[86] "*"                      [87] "npmtm"                 [88] "noIframe"
[89] "MediaSource"            [90] "canPlayType"           [91] "isTypeSupported"
[92] "vco"                    [93] "video/3gpp;"           [94] "video/mpeg;"
[95] "vcmk"                   [96] "k"                     [97] "chrome.runtime"
[98] "DeprecationReportBody"  [99] "MathMLElement"        [100] "opr"
[101] "XMLHttpRequestEventTarget"   [102] "onloadend"
[103] "WritableStream"        [104] "TextTrackCue"        [105] "VisualViewport"
[106] "StyleSheet"           [107] "SVGGeometryElement"   [108] "RTCStatsReport"
[109] "CropTarget"           [110] "BatteryManager"       [111] "LaunchQueue"
[112] "CSSFontPaletteValuesRule"    [113] "ServiceWorkerContainer"
[114] "MozMobileMessageManager"
[115] "CSS2Properties.prototype.MozOSXFontSmoothing"
[116] "ContentVisibilityAutoStateChangeEvent"
[117] "PerformanceServerTiming"
[118] "VideoFrame"           [119] "CSSCounterStyleRule"
[120] "XMLDocument.prototype.hasStorageAccess"
[121] "CryptoKey"            [122] "VideoPlaybackQuality"
[123] "EventCounts"          [124] "RTCError"             [125] "CSSCharsetRule"
[126] "RTCPeerConnectionIceErrorEvent"
[127] "MediaSourceHandle"    [128] "__REACT_DEVTOOLS_GLOBAL_HOOK__"
[129] "Err:"                 [130] "ucdv"                 [131] "storage"
[132] "usage"                [133] "M"                    [134] "enabledPlugin"
[135] "err"                  [136] "plggt"                [137] "value"
[138] "add"                  [139] "muev"                 [140] "try"
[141] "prso"                 [142] "idn"                  [143] "ContactsManager"
[144] "svde"                 [145] "vpbq"                 [146] "HTMLVideoElement"
[147] ":"                    [148] "csssp"                [149] "matchMedia"
[150] ")"                    [151] "fine"                 [152] "mq2"
[153] "color-gamut"          [154] "p3"                   [155] "display-mode"
[156] "standalone"           [157] "eval\\sat\\sevaluate"
[158] "evaluate"             [159] "XMLSerializer"        [160] "effectiveType"
[161] "nisd"                 [162] "getTimezoneOffset"    [163] "ihdn"
[164] "xt1"                  [165] "hasFocus"             [166] "XMLDocument"
[167] "eva"                  [168] "ecpc"                 [169] "process"
[170] "opener"               [171] "audio/mpegurl;"       [172] "acmpu"
[173] "audio/wav; codecs=\"1\""     [174] "audio/aac;"
[175] "acaa"                 [176] "acaats"               [177] "audio/3gpp;"
[178] "audio/flac;"          [179] "acmp4ts"              [180] "acmp3ts"
[181] "audio/webm;"          [182] "acwmts"               [183] "ocpt"
[184] "ac_NA"                [185] "spawn"                [186] "vendor"
[187] "md"                   [188] "pltod"                [189] "bid"
[190] "systemLanguage"       [191] "ccsB"                 [192] "ccsH"
[193] "navigation"           [194] "connectEnd"           [195] "nt_dns"
[196] "nt_rd"                [197] "redirectStart"        [198] "requestStart"
[199] "nt_tls"               [200] "nt_ttf"               [201] "responseEnd"
[202] "nt_swt"               [203] "workerStart"          [204] "nt_csd"
[205] "nt_nhp"               [206] "nt_dcle"              [207] "domContentLoadedEventStart"
[208] "domInteractive"       [209] "pw"                   [210] "__playwright_builtins__"
[211] "__playwright__binding__"
[212] "__playwright__binding__controller__"
[213] "pplx-agent-0_0-overlay-stop-button"
[214] "zIndex"               [215] "__stagehandV3__"      [216] "isf2"
[217] "(display-mode: fullscreen)"   [218] "D"            [219] "L"
[220] "P"                    [221] "sgc"                  [222] "j"
[223] "sel"                  [224] "languages"            [225] "VENDOR"
[226] "getExtension"         [227] "W"                    [228] "q"
[229] "hash"                 [230] "default"
```

---

## 9 — Appendix B — File / line index for cross-references

| What | File | Lines |
|------|------|-------|
| Function.toString native masking | `crates/js_runtime/src/js/stealth_bootstrap.js` | 7-91 (the global Function.prototype.toString patch is **commented out** at 12-37; only per-fn `_maskFunction` is active) |
| `navigator.*` getters (main thread) | `crates/js_runtime/src/js/window_bootstrap.js` | 760-846 |
| `navigator.languages` cache | `crates/js_runtime/src/js/window_bootstrap.js` | 770-779 |
| `navigator.webdriver` | `crates/js_runtime/src/js/window_bootstrap.js` | 802-806 |
| `navigator.plugins` shim | `crates/js_runtime/src/js/window_bootstrap.js` | 155-220 |
| `navigator.userAgentData` (Client Hints) | `crates/js_runtime/src/js/window_bootstrap.js` | 1421-1547 |
| `chrome.{loadTimes,csi,app}` | `crates/js_runtime/src/js/window_bootstrap.js` | 1014-1100 |
| Permissions.query state map | `crates/js_runtime/src/js/window_bootstrap.js` | 303-360 |
| `Intl.*.resolvedOptions` patch | `crates/js_runtime/src/js/window_bootstrap.js` | 1737-1786 |
| Worker class | `crates/js_runtime/src/js/window_bootstrap.js` | 1570-1745 |
| `OffscreenCanvas` real impl | `crates/js_runtime/src/js/canvas_bootstrap.js` | 1109-1184 |
| WebGL `_g()` cache + getParameter | `crates/js_runtime/src/js/canvas_bootstrap.js` | 369-457 |
| WebGL default `unmasked_renderer` (macOS arm64 M3) | `crates/js_runtime/src/js/canvas_bootstrap.js` | 380-381 |
| WebGL default extensions list | `crates/js_runtime/src/js/canvas_bootstrap.js` | 463-485 |
| AudioContext / OfflineAudioContext shim | `crates/js_runtime/src/js/canvas_bootstrap.js` | 581-986 |
| MediaSource / canPlayType shim | `crates/js_runtime/src/js/window_bootstrap.js` | 4753-4794 |
| Worker isolate setup | `crates/js_runtime/src/runtime.rs` | 231-334 |
| `worker_bootstrap.js` navigator | `crates/js_runtime/src/js/worker_bootstrap.js` | 54-77 |
| `worker_bootstrap.js` Intl patch | `crates/js_runtime/src/js/worker_bootstrap.js` | 33-52 |
| `worker_bootstrap.js` postMessage | `crates/js_runtime/src/js/worker_bootstrap.js` | 105-137 |
| Canvas2D backend | `crates/canvas/src/canvas2d.rs` | 1-1850 |
| Canvas `to_data_url_with_jitter` | `crates/canvas/src/canvas2d.rs` | 1047-1084 |
| Canvas op layer (`op_canvas_to_data_url`) | `crates/js_runtime/src/extensions/canvas_ext.rs` | 357-390 |
| Canvas `measureText` op | `crates/js_runtime/src/extensions/canvas_ext.rs` | 392-403 |
| Per-family font width delta | `crates/js_runtime/src/js/canvas_bootstrap.js` | 67-90 |
| Audio extension | `crates/js_runtime/src/extensions/audio_ext.rs` | 27+ |
| Behavior synthesis | `crates/stealth/src/behavior.rs` | (full file — needs audit per §7.1) |
| Insecure-context cleanup | `crates/js_runtime/src/js/cleanup_bootstrap.js` | 1-85 |
| Diagnostic test | `crates/browser/tests/chrome_compat.rs` | line 5008 (`datadome_diagnostic_capture`) |
| 600+ global-name list | `crates/js_runtime/src/js/interfaces_bootstrap.js` | line 53 |

---

## 10 — Appendix C — Sources

- `https://js.datadome.co/tags.js` — captured 2026-05-10, 110 666 bytes,
  version 5.6.3 (header)
- `https://github.com/glizzykingdreko/Datadome-Deobfuscator` — Babel
  AST deobfuscator (referenced; not run for this pass)
- `https://github.com/glizzykingdreko/datadome-encryption` — clean-room
  encryption reimpl (referenced for §2 dual-XOR description)
- `https://medium.com/@glizzykingdreko/breaking-down-datadome-captcha-waf-d7b68cef3e21`
  — long-form writeup; cited for the daily-key-rotation and 31-feature
  behavior scoring claims
- `https://datadome.co/threat-research/the-art-of-bot-detection-picasso-for-device-class-fingerprinting/`
  — vendor's own Picasso writeup
- `https://datadome.co/threat-research/how-new-headless-chrome-the-cdp-signal-are-impacting-bot-detection/`
  — Antoine Vastel CDP detection writeup
- `docs/RESEARCH_DATADOME_BYPASS_2026_05_10.md` — landscape doc
- `docs/CHROME_JS_SURFACE_PARITY_2026_04_29.md` — JS surface inventory
- `docs/CHROME_FINGERPRINT_FULL_INVENTORY_2026_04_29.md` — full
  fingerprint inventory
- `docs/PHASE6_FINGERPRINT_INVENTORY_FINDINGS_2026_04_29.md` — WebGL
  parity gap notes
- `docs/CANADA_GOOSE_DIAGNOSIS_2026_05_10_PART2.md` —
  undefined-receiver class root cause
- `docs/HOLISTIC_TEST_2026_05_10/SUMMARY.md` — current 4 sites at
  `DataDome-CHL`
- `docs/AKAMAI_BMP_V13_FIELD_ENCODING_2026_04_29.md` — analogous
  field-encoding doc for Akamai
- Chrome 147 macOS arm64 fixture: `tests/fixtures/chrome147/captured_macos_arm64.json`
  (referenced from `canvas_bootstrap.js:375`)

---

*End of document. ~900 lines markdown. No source code modified, no
tests run, no commits made.*
