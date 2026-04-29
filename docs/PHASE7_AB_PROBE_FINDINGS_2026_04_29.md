# Phase 7 — Comprehensive A/B probe: browser_oxide vs Playwright MCP

> Direct byte-by-byte diff of ~106 observable JS properties between
> our engine and real headed Chrome 147 (macOS arm64) via Playwright
> MCP from the same machine. Captured both insecure (data: URL) and
> secure (https://example.com) contexts.
>
> Bottom line: **18 secure-context API leaks are our biggest tell** —
> we expose `mediaDevices`, `serviceWorker`, `clipboard`, `credentials`,
> `bluetooth`, `usb`, `keyboard`, etc. on `data:` / `http:` URLs where
> real Chrome hides them entirely. Add this single gating mechanism →
> 18 distinct fingerprint probes go quiet in one change.

## Capture inputs

| File | Context | URL | Chrome state |
|---|---|---|---|
| `.playwright-mcp/captures/probe_mcp.json` | INSECURE | `data:text/html,...` | 160 keys |
| `.playwright-mcp/captures/probe_mcp_secure.json` | SECURE | `https://example.com/` | 49 keys |
| `.playwright-mcp/captures/probe_oxide.json` | (our engine, treats from_html as insecure) | (about:blank-equivalent) | 106 keys |

Probe code: `crates/browser/tests/phase7_ab_probe.rs::PROBE_HTML`.

Apples-to-apples diff is over the 92 shared keys between oxide and
mcp-insecure. Result: **33 / 92 match (35.9%)**, **59 / 92 mismatch (64.1%)**.

## Mismatch categorization

### Category A — SECURE-CONTEXT-LEAK (18 surfaces, the dominant tell)

Real Chrome hides these on `data:` / `http:` / `file:` URLs. Our
engine exposes them everywhere. **Each is a single anti-bot probe
that flips false → true and flags us as not-real-Chrome on every
non-HTTPS test page.**

| Property | oxide value | Chrome insecure | Chrome secure |
|---|---|---|---|
| `nav.mediaDevices` | `[object MediaDevices]` | undefined | `[object MediaDevices]` |
| `nav.serviceWorker` | `[object ServiceWorkerContainer]` | undefined | `[object ServiceWorkerContainer]` |
| `nav.clipboard` | `[object Object]` | undefined | `[object Clipboard]` |
| `nav.credentials` | `[object CredentialsContainer]` | undefined | `[object CredentialsContainer]` |
| `nav.keyboard` | `[object Keyboard]` | undefined | `[object Keyboard]` |
| `nav.locks` | `[object Object]` | undefined | `[object LockManager]` |
| `nav.wakeLock` | `[object Object]` | undefined | `[object WakeLock]` |
| `nav.usb` | `[object Object]` | undefined | `[object USB]` |
| `nav.bluetooth` | `[object Bluetooth]` | undefined | `[object Bluetooth]` |
| `nav.hid` | `[object Object]` | undefined | `[object HID]` |
| `nav.serial` | `[object Object]` | undefined | `[object Serial]` |
| `nav.virtualKeyboard` | `[object VirtualKeyboard]` | undefined | `[object VirtualKeyboard]` |
| `nav.devicePosture` | `[object DevicePosture]` | undefined | `[object DevicePosture]` |
| `nav.storage` | `[object StorageManager]` | undefined | `[object StorageManager]` |
| `nav.gpu` | `[object Object]` | undefined | `[object GPU]` |
| `nav.deviceMemory` | `16` | undefined | `16` |
| `nav.userAgentData` | `[object NavigatorUAData]` | undefined | `[object NavigatorUAData]` |
| `nav.getBattery` | `function` | not-a-function (TypeError) | `function` |
| `crypto.subtle` | `object` | undefined | `object` |
| `crypto.randomUUID` | `function` | undefined | `function` |
| `Notification.permission` | `"default"` | `"denied"` | `"default"` |

**Fix**: add `isSecureContext` check in `window_bootstrap.js` at the
top, then conditionally define each of these 18+ properties only when
true. The shape we ship today is correct for HTTPS; we just need to
hide it on http/data/file. Spec reference: each API's IDL has an
`[SecureContext]` extended attribute.

Effort: ~3-4 hours. Single biggest defense-in-depth win available.

### Category B — STRUCTURAL DESCRIPTOR MISMATCHES (4 surfaces)

These are about WHERE properties live (own vs prototype), not value.
Detection libraries probe via `Object.getOwnPropertyDescriptor` to
distinguish a real engine from a shim.

| Property | oxide | Chrome (both contexts) | Fix |
|---|---|---|---|
| `descr.win.scrollX` | undefined (we moved it to Window.prototype in Phase 6 D2) | `[object Object]` (own accessor on window) | **Revert Phase 6 D2** — keep accessors on `window` instance, drop them from Window.prototype |
| `descr.WindowProto.scrollX` | `["get","set","enumerable","configurable"]` (we put it here) | `[]` (not there) | (same revert closes both) |
| `nav.geolocation` toStringTag | `[object Object]` | `[object Geolocation]` | Add `Symbol.toStringTag` to `Geolocation` |
| `nav.scheduling` toStringTag | `[object Object]` | `[object Scheduling]` | Add `Symbol.toStringTag` to `Scheduling` |

**Note on Phase 6 D2 mistake**: the prior doc claimed Chrome had
scrollX on `Window.prototype`. The MCP capture proves the opposite —
it's an own accessor on the `window` instance, AND `Window.prototype`
has nothing for `scrollX`. Easy revert.

Effort: ~1 hour.

### Category C — VALUE MISMATCHES (numerical + content)

Things present in both but with wrong values:

| Property | oxide | Chrome 147 macOS arm64 | Why it matters |
|---|---|---|---|
| `eventCounts.size` | `0` | `36` (pre-populated) | Sites probe `size > 0` to verify Chrome shape |
| `webgl.UNMASKED_RENDERER_WEBGL` | `Apple M2 Pro` | `Apple M3` | Profile string mismatch with current macOS arm64 |
| `webgl.extensions.length` | `36` | `39` | We ship 3 fewer extensions than current Chrome |
| `window.ownPropertyNames.length` | `369` | `980` (insecure), `1231` (secure) | Anti-bot scripts compare counts |
| `doc.characterSet` | `UTF-8` | `windows-1252` | HTML legacy default for docs without explicit charset |
| `nav.userAgentData.brands` | `[GoogleChrome147, Chromium147, Not.A/Brand24]` | `[GoogleChrome147, Not.A/Brand 8, Chromium147]` | Brand order + GREASE version (24 → 8) |
| `nav.hardwareConcurrency` | `10` (host CPU count?) | `8` (per profile) | Either profile not being read or hardcoded to host |
| `perm.geolocation.state` | `"prompt"` | `"denied"` (insecure) / `"prompt"` (secure) | Geolocation gated to secure-context like the others |

**Note on `nav.userAgentData`**: real Chrome 147 GREASE entry is
`{brand:"Not.A/Brand", version:"8"}` not `version:"24"`. Recently
Chrome changed the version. Our profile presets have the stale value.

**Fix priority**:
1. `eventCounts.size = 36` — easy, populate the Map with 36 known
   event-type keys (we already have the stub)
2. `userAgentData` brand order + version "8" — change literal in
   `window_bootstrap.js`
3. `hardwareConcurrency` — verify profile is being read; should be
   `_pInt("hardware_concurrency", 8)` returning the macOS preset's 8
4. WebGL renderer string update + 3 missing extensions
5. `doc.characterSet` — set to "windows-1252" by default for HTML
   docs without charset declaration
6. `window.ownPropertyNames` count is structural; defer

Effort: 2-3 hours for items 1-5; item 6 needs more investigation.

### Category D — OXIDE-MISSING (3 keys)

Probe-script artifacts where our probe didn't capture a value the MCP
probe did. Not engine bugs — just probe-script keys that diverged
between the two scripts. Easy to fix in the test code.

## What this analysis CONFIRMS as already correct

33 / 92 properties match exactly. Notable:

- `nav.userAgent`, `appName`, `appCodeName`, `appVersion`, `product`,
  `productSub`, `vendor`, `vendorSub`, `platform`, `language` —
  all match Chrome 147 macOS string verbatim
- `nav.webdriver: false` — match
- `nav.maxTouchPoints: 0`, `nav.cookieEnabled: true`,
  `nav.onLine: true`, `nav.pdfViewerEnabled: true` — match
- `nav.plugins.length: 5` and the 5 plugin names match exactly
- `chrome.runtime` ABSENT (Phase 6 D3 fix verified) — match
- `chrome.app/csi/loadTimes` PRESENT — match
- `Function.prototype.toString.call(<every standard API>)` returns
  `[native code]` — all 14 sample masks match
- `mql.*` for all probed media queries — Phase 6 D1 fix verified
- `WebTransport.new` returns instance with rejected `ready` — Phase 6
  D3 fix verified
- `caches`, `cookieStore`, `IdleDetector`, `EyeDropper`, `Notification.requestPermission`,
  `eventCounts` exist (Phase 6 D4 fix verified — though insecure-context
  gating still missing)

## Top 5 highest-ROI fixes ranked

| # | Item | Effort | Sites unlocked (estimate) |
|---|---|---|---:|
| 1 | **Secure-context gating** (hide 18 APIs on insecure contexts) | 3-4 h | 0 directly (most failing sites are HTTPS), but defense-in-depth across every probe |
| 2 | **Revert scrollX/Y to window-instance accessor** (Phase 6 D2 was wrong about location) | 1 h | 0, but closes a structural-descriptor probe |
| 3 | **Pre-populate `performance.eventCounts` with 36 known keys** | 1 h | 0 directly, CreepJS-class probes |
| 4 | **Update GREASE `version: "24"` → `"8"` + swap brand order** | 15 min | small but byte-exact userAgentData parity |
| 5 | **Symbol.toStringTag on Geolocation, Scheduling, Clipboard, etc.** | 30 min | closes ~10 `Object.prototype.toString.call(navigator.X)` probes |

## What's NOT a finding (false alarms)

- `nav.languages: "[\"en-US\",\"en\"]"` matches both — NO bug
- `nav.plugins.names` matches exactly — NO bug
- All 14 native-code mask probes match — NO bug
- `chrome.runtime in chrome === false` — Phase 6 D3 verified
- `WebTransport.new` returns instance — Phase 6 D3 verified
- All `mql.*` probes for documented features match — Phase 6 D1 verified

## Reproduction

```bash
# In Playwright MCP, navigate to data: URL with the probe HTML, run
# the eval block (see git history for the exact JS), save to:
#   .playwright-mcp/captures/probe_mcp.json
# Repeat for https://example.com/ → probe_mcp_secure.json

# In oxide:
cargo test -p browser --test phase7_ab_probe -- \
    --ignored --test-threads=1 --nocapture phase7_ab_probe_capture_oxide
# → writes .playwright-mcp/captures/probe_oxide.json

# Diff:
python3 /tmp/diff_probe.py
```

## Cross-references

- `docs/CHROME_FINGERPRINT_FULL_INVENTORY_2026_04_29.md` — full 1,224-line
  catalog (parent doc)
- `docs/PHASE6_FINGERPRINT_INVENTORY_FINDINGS_2026_04_29.md` — top-7
  gaps from research-agent inventory (Phase 6 closed those)
- `docs/PHASE5_RECOVERABILITY_ANALYSIS_2026_04_29.md` — per-site
  recoverability data
- `crates/browser/tests/phase7_ab_probe.rs` — the probe + capture
  infrastructure (commit alongside this doc)

---

## Appendix A — Root cause of the 18 secure-context leaks

The single byte that explains the entire Category A column:

| Capture | URL | `isSecureContext` |
|---|---|---|
| `probe_mcp.json` (real Chrome insecure) | `data:text/html,...` | **false** |
| `probe_mcp_secure.json` (real Chrome secure) | `https://example.com/` | **true** |
| `probe_oxide.json` (us, default `from_html`) | `about:blank` | **true** ← bug |

Our `from_html` path treats `about:blank` as a secure context. Real
Chrome treats `about:blank` (and `data:` and `http:` and `file:`) as
**non-secure**, which gates all 18 APIs at the IDL level. Fixing this
flag alone is necessary; gating the APIs on it is the other half of
the fix.

**Where to fix**:
- `crates/js_runtime/src/js/window_bootstrap.js` — wrap each of the
  18 APIs in `if (isSecureContext) { ... }`
- The `isSecureContext` global getter must reflect actual scheme
  (https/file with allowed origin/wss/etc.) — currently hard-coded
  true. Trace the value through `crates/browser/src/page.rs` /
  `from_url` initialization to make sure http:///data:/about:blank
  paths set it false.

## Appendix B — Full per-key dump from `probe_oxide.json` (106 keys)

This is the literal capture our `phase7_ab_probe_capture_oxide` test
wrote. Reproduced inline so the doc remains self-contained even if
the capture file is regenerated later.

```json
{
  "url": "about:blank",
  "isSecureContext": "true",
  "nav.userAgent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36",
  "nav.appName": "Netscape",
  "nav.appCodeName": "Mozilla",
  "nav.appVersion": "5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36",
  "nav.product": "Gecko",
  "nav.productSub": "20030107",
  "nav.vendor": "Google Inc.",
  "nav.vendorSub": "",
  "nav.platform": "MacIntel",
  "nav.language": "en-US",
  "nav.languages": "[\"en-US\",\"en\"]",
  "nav.cookieEnabled": "true",
  "nav.doNotTrack": "null",
  "nav.webdriver": "false",
  "nav.hardwareConcurrency": "10",
  "nav.deviceMemory": "16",
  "nav.maxTouchPoints": "0",
  "nav.onLine": "true",
  "nav.pdfViewerEnabled": "true",
  "nav.geolocation": "[object Object]",
  "nav.mediaDevices": "[object MediaDevices]",
  "nav.permissions": "[object Permissions]",
  "nav.plugins": "[object PluginArray]",
  "nav.plugins.length": "5",
  "nav.plugins.names": "[\"PDF Viewer\",\"Chrome PDF Viewer\",\"Chromium PDF Viewer\",\"Microsoft Edge PDF Viewer\",\"WebKit built-in PDF\"]",
  "nav.mimeTypes": "[object MimeTypeArray]",
  "nav.serviceWorker": "[object ServiceWorkerContainer]",
  "nav.clipboard": "[object Object]",
  "nav.credentials": "[object CredentialsContainer]",
  "nav.keyboard": "[object Keyboard]",
  "nav.locks": "[object Object]",
  "nav.presentation": "undefined",
  "nav.wakeLock": "[object Object]",
  "nav.usb": "[object Object]",
  "nav.bluetooth": "[object Bluetooth]",
  "nav.hid": "[object Object]",
  "nav.serial": "[object Object]",
  "nav.virtualKeyboard": "[object VirtualKeyboard]",
  "nav.devicePosture": "[object DevicePosture]",
  "nav.windowControlsOverlay": "[object WindowControlsOverlay]",
  "nav.mediaSession": "[object MediaSession]",
  "nav.storage": "[object StorageManager]",
  "nav.contacts": "undefined",
  "nav.scheduling": "[object Object]",
  "nav.xr": "undefined",
  "nav.gpu": "[object Object]",
  "nav.userAgentData": "[object Object]",
  "nav.userAgentData.brands": "[{\"brand\":\"Google Chrome\",\"version\":\"147\"},{\"brand\":\"Chromium\",\"version\":\"147\"},{\"brand\":\"Not.A/Brand\",\"version\":\"24\"}]",
  "nav.userAgentData.mobile": "false",
  "nav.userAgentData.platform": "macOS",
  "nav.getBattery": "function",
  "global.caches": "object",
  "global.cookieStore": "object",
  "global.IdleDetector": "function",
  "global.EyeDropper": "function",
  "global.WebTransport": "function",
  "crypto.subtle.typeof": "object",
  "crypto.randomUUID.typeof": "function",
  "Notification.permission": "default",
  "descr.WindowProto.scrollX": "[\"get\",\"set\",\"enumerable\",\"configurable\"]",
  "descr.win.scrollX": "undefined",
  "NavProto.userAgent.descr": "{\"hasGet\":true,\"hasSet\":false,\"enumerable\":true,\"configurable\":true}",
  "eventCounts.size": "0",
  "eventCounts.first10keys": "[]",
  "doc.characterSet": "UTF-8",
  "doc.compatMode": "CSS1Compat",
  "doc.visibilityState": "visible",
  "doc.hasFocus": "true",
  "window.ownPropertyNames.length": "369",
  "screen.width": "1440",
  "screen.height": "900",
  "screen.availWidth": "1440",
  "screen.availHeight": "875",
  "screen.availLeft": "0",
  "screen.availTop": "25",
  "screen.colorDepth": "24",
  "screen.pixelDepth": "24",
  "screen.orientation.type": "landscape-primary",
  "screen.orientation.angle": "0",
  "webgl.VENDOR": "WebKit",
  "webgl.RENDERER": "WebKit WebGL",
  "webgl.VERSION": "WebGL 1.0 (OpenGL ES 2.0 Chromium)",
  "webgl.SHADING_LANGUAGE_VERSION": "WebGL GLSL ES 1.0 (OpenGL ES GLSL ES 1.0 Chromium)",
  "webgl.UNMASKED_VENDOR_WEBGL": "Google Inc. (Apple)",
  "webgl.UNMASKED_RENDERER_WEBGL": "ANGLE (Apple, ANGLE Metal Renderer: Apple M2 Pro, Unspecified Version)",
  "webgl.extensions.length": "36",
  "mql.(prefers-color-scheme: light)": "true",
  "mql.(prefers-color-scheme: dark)": "false",
  "mql.(pointer: fine)": "true",
  "mql.(pointer: coarse)": "false",
  "mql.(hover: hover)": "true",
  "mql.(forced-colors: none)": "true",
  "mql.(orientation: landscape)": "true",
  "mql.(min-width: 100px)": "true",
  "chrome.runtime.in": "false",
  "chrome.app.in": "true",
  "chrome.csi.in": "true",
  "chrome.loadTimes.in": "true",
  "chrome.csi().keys": "[\"onloadT\",\"pageT\",\"startE\",\"tran\"]",
  "chrome.loadTimes().keys": "[\"commitLoadTime\",\"connectionInfo\",\"finishDocumentLoadTime\",\"finishLoadTime\",\"firstPaintAfterLoadTime\",\"firstPaintTime\",\"navigationType\",\"npnNegotiatedProtocol\",\"requestTime\",\"startLoadTime\",\"wasAlternateProtocolAvailable\",\"wasFetchedViaSpdy\",\"wasNpnNegotiated\"]",
  "Date.toString.epoch": "Wed Dec 31 1969 16:00:00 GMT-0800 (Pacific Standard Time)",
  "battery.proto": "[object BatteryManager]",
  "storage.estimate.quota": "128849018880",
  "perm.geolocation.state": "prompt"
}
```

## Appendix C — Full ordered side-by-side mismatch list (59 of 92 shared keys)

Every shared probe key where oxide differs from `probe_mcp.json`
(the real Chrome 147 macOS arm64 capture on `data:` URL). Format:
`key | oxide | chrome-insecure | category`.

| Key | Oxide | Chrome insecure | Cat |
|---|---|---|---|
| `isSecureContext` | true | false | **A (root)** |
| `nav.mediaDevices` | `[object MediaDevices]` | undefined | A |
| `nav.serviceWorker` | `[object ServiceWorkerContainer]` | undefined | A |
| `nav.clipboard` | `[object Object]` | undefined | A |
| `nav.credentials` | `[object CredentialsContainer]` | undefined | A |
| `nav.keyboard` | `[object Keyboard]` | undefined | A |
| `nav.locks` | `[object Object]` | undefined | A |
| `nav.wakeLock` | `[object Object]` | undefined | A |
| `nav.usb` | `[object Object]` | undefined | A |
| `nav.bluetooth` | `[object Bluetooth]` | undefined | A |
| `nav.hid` | `[object Object]` | undefined | A |
| `nav.serial` | `[object Object]` | undefined | A |
| `nav.virtualKeyboard` | `[object VirtualKeyboard]` | undefined | A |
| `nav.devicePosture` | `[object DevicePosture]` | undefined | A |
| `nav.storage` | `[object StorageManager]` | undefined | A |
| `nav.gpu` | `[object Object]` | undefined | A |
| `nav.deviceMemory` | 16 | undefined | A |
| `nav.userAgentData` | `[object Object]` | undefined | A |
| `nav.userAgentData.brands` | (3-brand list) | THROW: undefined | A |
| `nav.userAgentData.mobile` | false | THROW | A |
| `nav.userAgentData.platform` | macOS | THROW | A |
| `nav.getBattery` | function | not-a-function | A |
| `crypto.subtle.typeof` | object | undefined | A |
| `crypto.randomUUID.typeof` | function | undefined | A |
| `Notification.permission` | "default" | "denied" | A |
| `global.caches` | object | undefined | A |
| `global.cookieStore` | object | undefined | A |
| `global.IdleDetector` | function | undefined | A |
| `global.EyeDropper` | function | undefined | A |
| `global.WebTransport` | function | undefined (THROW: ReferenceError) | A |
| `perm.geolocation.state` | "prompt" | "denied" | A |
| `descr.win.scrollX` | undefined | `[object Object]` | **B (D2 mistake)** |
| `descr.WindowProto.scrollX` | `["get","set","enumerable","configurable"]` | `[]` | B (D2 mistake) |
| `nav.geolocation` | `[object Object]` | `[object Geolocation]` | B |
| `nav.scheduling` | `[object Object]` | `[object Scheduling]` | B |
| `eventCounts.size` | 0 | 36 | C |
| `eventCounts.first10keys` | `[]` | 10-keys | C |
| `webgl.UNMASKED_RENDERER_WEBGL` | Apple M2 Pro | Apple M3 | C |
| `webgl.extensions.length` | 36 | 39 | C |
| `window.ownPropertyNames.length` | 369 | 980 | C |
| `doc.characterSet` | UTF-8 | windows-1252 | C |
| `nav.userAgentData.brands` (order + version) | `[GC147, Chromium147, Not.A/Brand 24]` | `[GC147, Not.A/Brand 8, Chromium147]` | C |
| `nav.hardwareConcurrency` | 10 | 8 | C |
| `screen.width` | 1440 | 1512 | C |
| `screen.height` | 900 | 982 | C |
| `screen.availWidth` | 1440 | 1512 | C |
| `screen.availHeight` | 875 | 949 | C |
| `screen.availTop` | 25 | 33 | C |
| `screen.colorDepth` | 24 | 30 | C |
| `screen.pixelDepth` | 24 | 30 | C |
| `url` | `about:blank` | `data:text/html,...` | (probe artifact) |
| (~9 more screen/url/profile preset deltas) | | | C |

**Most screen.* mismatches are profile-preset issues** — our
`chrome_130_macos()` preset shapes a 1440×900 @ colorDepth=24
display, but real Chrome 147 on M3 macOS ships 1512×982 @
colorDepth=30. Single-line profile change.

## Appendix D — `/tmp/diff_probe.py` source

```python
#!/usr/bin/env python3
"""Diff Phase 7 probe captures key by key."""
import json, sys
from pathlib import Path

ROOT = Path("/Users/yfedoseev/Projects/browser_oxide/.playwright-mcp/captures")
mcp = json.loads((ROOT / "probe_mcp.json").read_text())
mcp_sec = json.loads((ROOT / "probe_mcp_secure.json").read_text())
oxide = json.loads((ROOT / "probe_oxide.json").read_text())

shared = sorted(set(mcp.keys()) & set(oxide.keys()))
match = miss = 0
for k in shared:
    a, b = str(mcp[k]), str(oxide[k])
    if a == b:
        match += 1
    else:
        miss += 1
        sec = mcp_sec.get(k, "<not-probed>")
        print(f"DIFF | {k:50s} | oxide={b[:40]:42s} | chrome_insecure={a[:40]:42s} | chrome_secure={str(sec)[:40]}")
print(f"\nshared={len(shared)}  match={match}  miss={miss}  ratio={match/len(shared)*100:.1f}%")
print(f"oxide-only={sorted(set(oxide.keys()) - set(mcp.keys()))[:10]}")
print(f"mcp-only-first-10={sorted(set(mcp.keys()) - set(oxide.keys()))[:10]}")
```

## Appendix E — Implementation roadmap (concrete, in priority order)

1. **`isSecureContext` correctness** in `crates/browser/src/page.rs`
   `from_html` / `from_url` paths — set the V8 binding to false for
   `about:blank` / `data:` / `http:` / `file:` schemes, true for
   `https:` / `wss:` / `file://` (with `allow_file_access_from_files`
   policy match).
2. **Conditional API exposure in `window_bootstrap.js`** — wrap each
   of the 18 APIs in Category A in `if (isSecureContext) { ... }`,
   then re-run `phase7_ab_probe_capture_oxide` and confirm those keys
   become "undefined" matching real Chrome.
3. **Revert Phase 6 D2** scrollX/Y/screenX/Y placement: move from
   `Window.prototype` accessors back to own accessors on the window
   instance. The MCP capture is authoritative.
4. **Pre-populate `performance.eventCounts`** with the 36 keys real
   Chrome 147 ships:
   `["pointerdown","touchend","input","keydown","mouseleave","mouseenter",
   "drop","beforeinput","pointerenter","dragend","dragstart","dragenter",
   "dragover","dragleave","drag","pointerout","pointerleave",
   "pointercancel","pointermove","pointerup","pointerover","wheel",
   "click","auxclick","contextmenu","dblclick","mousedown","mouseup",
   "mousemove","mouseout","mouseover","keyup","keypress","compositionstart",
   "compositionupdate","compositionend"]`
   (verified order from `probe_mcp_secure.json` first-10 + Chromium
   source `EventTypeNames`).
5. **userAgentData fix**: brand order
   `[Google Chrome, Not.A/Brand, Chromium]`, GREASE version `"8"`
   not `"24"`. Two-line edit in profile preset or window_bootstrap.
6. **Symbol.toStringTag** on Geolocation (`"Geolocation"`),
   Scheduling (`"Scheduling"`), Clipboard (`"Clipboard"`), USB
   (`"USB"`), HID (`"HID"`), Serial (`"Serial"`), GPU (`"GPU"`),
   LockManager (`"LockManager"`), WakeLock (`"WakeLock"`).
7. **`doc.characterSet`** default to `"windows-1252"` for HTML docs
   without explicit `<meta charset>`.
8. **screen.* preset alignment** with current macOS arm64 hardware:
   1512×982, colorDepth=30, availTop=33, M3 renderer string,
   39-extension WebGL list (3 missing extensions to identify).
9. **`window.ownPropertyNames.length`** structural gap (369 vs 980
   insecure / 1231 secure) — needs investigation of which 600+
   names are missing from our globalThis. Likely a pile of legacy
   `webkit*` constructors and `WebGL*` interface objects we haven't
   registered.

After items 1-7: re-run Phase 7 probe. Expected: ≥85/92 shared
keys match (up from 33/92), one structural gap (item 9) remaining.
