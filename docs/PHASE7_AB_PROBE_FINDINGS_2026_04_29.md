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
