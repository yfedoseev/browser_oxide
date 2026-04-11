# Fingerprint polish — small per-value improvements

**Priority**: P2
**Effort**: 9-13 hours total
**Dependencies**: none — each item is independent

These are small, isolated improvements to individual navigator /
performance / global values that a sensor VM might hash. Each is
quick to implement but has uncertain individual impact. **Batch them
into one PR** and measure the cumulative effect on the blocker
probes once all are landed.

---

## performance.memory realistic fluctuating values

**Effort**: 1 hour

**Current state**:
```js
performance.memory = {
    jsHeapSizeLimit: 2172649472,
    totalJSHeapSize: 10000000,  // suspiciously round
    usedJSHeapSize: 8000000,    // suspiciously round
};
```

**Why it's wrong**: real Chrome returns integer values that fluctuate
every call (V8 reports actual heap state). Values like `10000000` and
`8000000` never occur naturally.

**Fix**: in `window_bootstrap.js`, make `performance.memory` a getter
that returns a fresh object each call with jittered values:

```js
Object.defineProperty(performance, 'memory', {
    configurable: true,
    enumerable: true,
    get() {
        // Chrome's jsHeapSizeLimit is ~2 GB, usually
        // 2172649472 bytes exactly on Win/Linux x86_64.
        const jsHeapSizeLimit = 2172649472;
        // totalJSHeapSize is the V8-allocated heap size,
        // typically 8-15 MB after a minimal page load.
        // Use a deterministic PRNG seeded by a monotonic
        // counter so it looks "real" but is reproducible
        // within a session.
        const base = 10485760; // 10 MB
        const jitter = ((Date.now() * 0x9e3779b9) >>> 0) % 5000000;
        const totalJSHeapSize = base + jitter;
        // usedJSHeapSize is always <= totalJSHeapSize,
        // usually 80-95% of it after JS has run for a bit.
        const usedJSHeapSize = Math.floor(totalJSHeapSize * 0.85);
        return {
            jsHeapSizeLimit,
            totalJSHeapSize,
            usedJSHeapSize,
        };
    },
});
```

**Test**: verify `performance.memory !== performance.memory` (each
call returns a fresh object) and that the values aren't suspiciously
round.

---

## navigator.userAgentData.brands randomized ordering

**Effort**: 30 minutes

**Current state**: we emit a fixed order `[Chromium 130, Google Chrome
130, Not?A_Brand 99]`.

**Why it's wrong**: real Chrome **randomizes the brand order** on each
call to prevent fingerprinting from comparing orders. This is a
documented Chrome anti-fingerprint measure.

**Fix**: in `window_bootstrap.js`, change the `userAgentData.brands`
getter to return a freshly-shuffled array each call:

```js
const _userAgentBrands = [
    { brand: 'Chromium', version: '130' },
    { brand: 'Google Chrome', version: '130' },
    { brand: 'Not?A_Brand', version: '99' },
];

// Fisher-Yates shuffle using a Math.random seed.
function _shuffled(arr) {
    const copy = [...arr];
    for (let i = copy.length - 1; i > 0; i--) {
        const j = Math.floor(Math.random() * (i + 1));
        [copy[i], copy[j]] = [copy[j], copy[i]];
    }
    return copy;
}

_navUserAgentData = {
    get brands() { return _shuffled(_userAgentBrands); },
    mobile: false,
    platform: 'macOS',
    getHighEntropyValues(hints) { ... },
};
```

**Verify Chrome's actual Not A Brand string**: Chrome 130 uses
`"Not/A)Brand"` (with forward-slash and close-paren) — or
`"Not-A.Brand"` — verify against a real Chrome:

```js
// In a browser console:
JSON.stringify(navigator.userAgentData.brands)
```

Our current literal `"Not?A_Brand"` is wrong because the `?` and `_`
don't appear in Chrome 130 either way. Fix this to match the exact
literal Chrome uses.

---

## navigator.connection values match Chrome defaults

**Effort**: 1 hour

**Current state** (verified by probe):
```js
navigator.connection = {
    effectiveType: '4g',
    rtt: 50,
    downlink: 10,
    saveData: false,
    type: 'wifi',
};
```

**Why it's partly wrong**: 
- `type: 'wifi'` is not a Chrome default. Chrome's
  `navigator.connection.type` is typically undefined except on
  mobile. Desktop Chrome doesn't set `type`.
- `rtt` and `downlink` should be rounded to Chrome's privacy-protected
  granularity: `rtt` rounded to nearest 25ms, `downlink` rounded to
  nearest 25 kbps.

**Fix**: remove `type` and round the other values:

```js
const _navConnection = Object.create(NetworkInformation.prototype);
Object.defineProperty(_navConnection, 'effectiveType', {
    get: () => _p("connection_effective_type", "4g"),
    enumerable: true,
});
// Chrome rounds rtt to nearest 25ms.
Object.defineProperty(_navConnection, 'rtt', {
    get: () => Math.round(_pInt("connection_rtt", 50) / 25) * 25,
    enumerable: true,
});
// Chrome rounds downlink to nearest 0.025 Mbps.
Object.defineProperty(_navConnection, 'downlink', {
    get: () => Math.round(_pFloat("connection_downlink", 10) * 40) / 40,
    enumerable: true,
});
Object.defineProperty(_navConnection, 'saveData', {
    get: () => false,
    enumerable: true,
});
// DO NOT add 'type' — desktop Chrome has it as undefined.
Object.defineProperty(_navConnection, 'downlinkMax', {
    get: () => Infinity,
    enumerable: true,
});
// Add the event target methods real Chrome has:
_navConnection.addEventListener = function() {};
_navConnection.removeEventListener = function() {};
_navConnection.dispatchEvent = function() { return true; };
_navConnection.onchange = null;
```

---

## navigator.permissions.query() returns realistic states

**Effort**: 2 hours

**Current state** (`window_bootstrap.js`):
```js
Permissions.prototype.query = function query(desc) {
    return Promise.resolve(new PermissionStatus(desc && desc.name));
};
```

`PermissionStatus.state` always returns `'prompt'`.

**Why it's wrong**: real Chrome returns different states for different
permissions based on the default permission policy:
- `notifications`, `push`, `geolocation`, `camera`, `microphone`,
  `midi`, `clipboard-read` → `prompt`
- `background-sync`, `persistent-storage`, `clipboard-write` → `granted`
- `screen-wake-lock`, `payment-handler`, `storage-access` → `prompt`
- `ambient-light-sensor`, `accelerometer`, `gyroscope`, `magnetometer`
  → `denied` (blocked by default for permissions policy)

A sensor VM that queries multiple permissions and compares the state
distribution will catch our "all prompt" response.

**Fix**: add a permission-name-keyed lookup table:

```js
const PERMISSION_DEFAULTS = {
    'notifications': 'prompt',
    'push': 'prompt',
    'geolocation': 'prompt',
    'camera': 'prompt',
    'microphone': 'prompt',
    'midi': 'prompt',
    'clipboard-read': 'prompt',
    'screen-wake-lock': 'prompt',
    'payment-handler': 'prompt',
    'background-sync': 'granted',
    'persistent-storage': 'granted',
    'clipboard-write': 'granted',
    'accelerometer': 'denied',
    'ambient-light-sensor': 'denied',
    'gyroscope': 'denied',
    'magnetometer': 'denied',
    'accessibility-events': 'denied',
};

Permissions.prototype.query = function query(desc) {
    const name = desc && desc.name;
    const state = PERMISSION_DEFAULTS[name] || 'prompt';
    const status = new PermissionStatus(name);
    Object.defineProperty(status, 'state', {
        get() { return state; },
        configurable: true,
    });
    return Promise.resolve(status);
};
```

**Verify against Chrome**: open DevTools on a real Chrome and run
`await navigator.permissions.query({name: 'notifications'})`, etc. Our
table should match.

---

## navigator.getBattery() shape (or removal)

**Effort**: 1 hour

**Current state**: `getBattery()` returns a Promise with a battery
object.

**Why it's tricky**: Chrome **removed the Battery Status API in 2024**
on non-secure contexts and made it fingerprint-resistant on secure
contexts. Modern Chrome 130 only exposes it on HTTPS and returns
degraded values.

**Fix options**:

1. **Remove `getBattery` entirely**: set `navigator.getBattery =
   undefined`. This matches Chrome's behavior on non-secure contexts.
2. **Return a charging-forever battery**: `{ charging: true,
   chargingTime: 0, dischargingTime: Infinity, level: 1.0 }`. Matches
   Chrome's privacy-protected default.

**Recommendation**: option 2 (keep the shape for compatibility, return
privacy-protected defaults). Real Chrome still has the API on HTTPS,
we should match.

---

## chrome global object (load_times, csi, runtime)

**Effort**: 1-2 hours

**Current state**: we have a partial `window.chrome` object somewhere
in window_bootstrap.js. Check if it has:
- `chrome.loadTimes()` → deprecated but still present
- `chrome.csi()` → deprecated but still present
- `chrome.runtime` → object with properties
- `chrome.app` → object

These are legacy Chrome-specific globals that many fingerprinters use
as "is this really Chrome" signals.

**Fix**: verify or add each one matching Chrome's output:

```js
globalThis.chrome = {
    app: {
        isInstalled: false,
        InstallState: { DISABLED: 'disabled', INSTALLED: 'installed', NOT_INSTALLED: 'not_installed' },
        RunningState: { CANNOT_RUN: 'cannot_run', READY_TO_RUN: 'ready_to_run', RUNNING: 'running' },
    },
    runtime: {
        OnInstalledReason: { /* enum values */ },
        OnRestartRequiredReason: { /* enum values */ },
        PlatformArch: { /* enum values */ },
        PlatformNaclArch: { /* enum values */ },
        PlatformOs: { /* enum values */ },
        RequestUpdateCheckStatus: { /* enum values */ },
        connect: function() {},
        sendMessage: function() {},
        // Chrome sets id only if an extension is present.
        id: undefined,
    },
    loadTimes: function() {
        // Deprecated but still returns an object in Chrome.
        return {
            requestTime: Date.now() / 1000 - 1,
            startLoadTime: Date.now() / 1000 - 0.5,
            commitLoadTime: Date.now() / 1000 - 0.4,
            finishDocumentLoadTime: Date.now() / 1000 - 0.2,
            finishLoadTime: Date.now() / 1000 - 0.1,
            firstPaintTime: Date.now() / 1000 - 0.3,
            firstPaintAfterLoadTime: 0,
            navigationType: 'Other',
            wasFetchedViaSpdy: true,
            wasNpnNegotiated: true,
            npnNegotiatedProtocol: 'h2',
            wasAlternateProtocolAvailable: false,
            connectionInfo: 'h2',
        };
    },
    csi: function() {
        return {
            startE: Date.now(),
            onloadT: Date.now(),
            pageT: Math.random() * 1000,
            tran: 15,
        };
    },
};

// Native-looking toString for each function.
chrome.loadTimes.toString = () => 'function loadTimes() { [native code] }';
chrome.csi.toString = () => 'function csi() { [native code] }';
```

**Important**: Chrome's `loadTimes.toString()` returns `[native code]`.
If we return the JS source, that's a giveaway. Override `toString`
explicitly.

---

## localStorage quota matching Chrome

**Effort**: 1 hour

**Current state**: our `localStorage` stub has no quota enforcement.
You can write arbitrary data.

**Why it might matter**: some fingerprinters test the quota by writing
a lot of data and checking when it throws. Chrome throws
`QuotaExceededError` at ~5 MB per origin.

**Fix**: track the total bytes written to localStorage and throw
`DOMException('QuotaExceededError')` when exceeding ~5 MB.

```js
const LOCAL_STORAGE_QUOTA = 5242880; // 5 MB
let _localStorageUsed = 0;
const _localStorage = {};

globalThis.localStorage = {
    setItem(key, value) {
        const newSize = String(key).length + String(value).length;
        const oldSize = _localStorage[key]
            ? String(key).length + String(_localStorage[key]).length
            : 0;
        if (_localStorageUsed - oldSize + newSize > LOCAL_STORAGE_QUOTA) {
            throw new DOMException(
                "Failed to execute 'setItem' on 'Storage': Setting the " +
                "value of '" + key + "' exceeded the quota.",
                'QuotaExceededError'
            );
        }
        _localStorage[key] = String(value);
        _localStorageUsed += newSize - oldSize;
    },
    getItem(key) { return _localStorage[key] ?? null; },
    removeItem(key) { /* ... */ },
    clear() { /* ... */ },
    key(i) { /* ... */ },
    get length() { return Object.keys(_localStorage).length; },
};
```

---

## Intl collator / number-format / plural-rules locale data

**Effort**: 2-4 hours

**Current state**: V8 ships with ICU, so `Intl.Collator`,
`Intl.NumberFormat`, `Intl.DateTimeFormat`, `Intl.PluralRules`,
`Intl.ListFormat`, `Intl.RelativeTimeFormat`, and
`Intl.DisplayNames` all work.

**What might be wrong**: `Intl.DateTimeFormat().resolvedOptions()`
returns the **system timezone**, not our stealth profile's timezone.
Task #38 (completed) added timezone consistency, verify it's still
working.

**Fix**: verify the Intl objects honor our `StealthProfile::timezone`.
If not, override `Intl.DateTimeFormat`'s `resolvedOptions` to return
the profile's timezone.

```js
const origResolved = Intl.DateTimeFormat.prototype.resolvedOptions;
Intl.DateTimeFormat.prototype.resolvedOptions = function() {
    const opts = origResolved.call(this);
    opts.timeZone = Deno.core.ops.op_get_profile_value('timezone') || opts.timeZone;
    return opts;
};
```

Also verify `Date.prototype.toLocaleString` and `Date.prototype.
toTimeString` return strings consistent with the profile timezone.

---

## Total effort and priority

| Fix | Effort | Probability it matters |
|---|---|---|
| performance.memory jitter | 1h | High (obvious tell) |
| userAgentData.brands shuffle | 30m | Medium-high |
| navigator.connection values | 1h | Medium |
| permissions.query states | 2h | Medium-high |
| getBattery shape | 1h | Low |
| chrome global (loadTimes/csi) | 1-2h | High (many sensors check) |
| localStorage quota | 1h | Low |
| Intl timezone consistency | 2-4h | Medium |

**Recommendation**: batch the high-probability ones
(`performance.memory`, `userAgentData.brands`, `chrome` global,
`permissions.query`) into one PR. Measure the effect on the blocker
probe. If POST body sizes on adidas/homedepot change, dig into the
lower-probability ones too. If nothing moves, stop and go back to
Tier 1 capability work.

**Each of these is a 30-minute to 2-hour fix**, but they add up to
meaningful fingerprint accuracy. The cumulative effect on trust
scores (CreepJS, FingerprintJS) should be visible even if no
individual fix unblocks a specific site.
