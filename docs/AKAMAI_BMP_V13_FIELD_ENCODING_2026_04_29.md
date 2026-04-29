# Akamai BMP v13 Pixel Sensor — Field Encoding (2026-04-29)

## Executive summary

The bootstrap sensor JS for `/akam/13/pixel_<hash>` is **fully self-contained, ~600 lines, and we already have a deobfuscated copy locally**:

- `/Users/yfedoseev/Projects/browser_oxide/docs/akamai_sensor_analysis/samsclub_akam13_bootstrap.deob.js`

This is the canonical source of truth — the actual JS that produces the captured POST body. Everything below cites line numbers in that file.

**Most important finding.** The fields the user flagged as "wrong on us" — `ap`, `fonts`, `fh` — are **NOT bot signals on this build**. They are initialized to `null` in the bootstrap (line 444–451) and either:

- never assigned (`fonts`, `fh`), or
- assigned async via `Image.onload` whose callback typically does not fire before the POST is sent (`ap`).

Our captured real-Chrome reference body confirms: `ap=null, fonts=null, fh=null` from a genuine Chrome-on-macOS load. **There is no parity gap on these fields.** Effort spent shipping `ApplePaySession` shim and font-enumeration shim was wasted on this version of the pixel sensor; those probes belong to other Akamai modules (sensor_data v3 or BMP mobile), not v13/pixel.

The cv hash `1e45567918c75c40a4d45cc3baa14e263b777784` (Walmart) differs from the SamsClub bootstrap we have (`b61e5c3c2c505ef7b386f96c66cbd468`). Future builds may turn these into real probes. For now: leave nulls, focus parity work elsewhere.

---

## Per-field probe table

### `bp` — plugin hashes (NOT "battery profile")

**Probe** (line 381–388, function `R`):

```js
function R(e) {
  var t = [], a = navigator.plugins;
  if (a) for (var r = 0; r < a.length; r++)
    for (var o = 0; o < a[r].length; o++)
      t.push(f([a[r].name, a[r].description, a[r].filename,
              a[r][o].description, a[r][o].type, a[r][o].suffixes
             ].toString()));
  i(e, t.toString());
}
```

`f` is a 32-bit hash (line 40–47):

```js
function f(e) {
  var t = 0;
  for (var n = 0; n < e.length; n++) { t = (t << 5) - t + e.charCodeAt(n); t &= t; }
  return t;
}
```

This is the classic **java-string-hashCode** (`hash * 31 + char`) folded to signed int32. Each plugin × each MIME type → one int. Chrome ships the 5 well-known stub plugins (PDF Viewer, Chrome PDF Viewer, Chromium PDF Viewer, Microsoft Edge PDF Viewer, WebKit built-in PDF) each with 2 MIMEs → 10 ints. Matches captured body exactly.

**Action**: ensure `navigator.plugins` returns Chrome's exact 5-plugin × 2-mime list with identical name/description/filename/type/suffixes strings. If we already do, `bp` will match deterministically (no randomization). The 10 specific captured ints are reproducible from the canonical Chrome plugin list.

---

### `sr` — screen + viewport

**Probe** (line 356–379, function `B`):

```js
{
  inner:      [innerWidth, innerHeight],
  outer:      [outerWidth, outerHeight],
  screen:     [screenX, screenY],
  pageOffset: [pageXOffset, pageYOffset],
  avail:      [screen.availWidth, screen.availHeight],
  size:       [screen.width, screen.height],
  client:     [document.body.clientWidth, document.body.clientHeight],  // line 371
  colorDepth: screen.colorDepth,
  pixelDepth: screen.pixelDepth
}
```

**`sr.client` = `[document.body.clientWidth, document.body.clientHeight]`** — NOT documentElement, NOT scrollWidth, NOT innerWidth/Height. The captured value `[1914, 28638]` is real-Chrome-on-Walmart: body.clientWidth=1914 (wider than 1440 viewport because Walmart's body has overflow content / wide layout), body.clientHeight=28638 (Walmart homepage is ~28k px tall).

**Action**: in browser_oxide we MUST run a real layout pass and expose `document.body.clientWidth/clientHeight` from the layout tree, NOT from CSSOM defaults. Our prior fix for `documentElement.clientWidth/Height` does not cover `body.clientWidth/Height`. The field is layout-engine-correctness-bound, not a JS shim. Confirmed our value `[1914, 28638]` matches real Chrome — so this may already be working, just verify post-load body geometry.

---

### `dp` — DOM property presence map

**Probe** (line 115–122, function `m`):

```js
function m(e) {
  var t = p(window, ['XDomainRequest','createPopup','removeEventListener','globalStorage',
                     'openDatabase','indexedDB','attachEvent','ActiveXObject',
                     'dispatchEvent','addBehavior','addEventListener','detachEvent',
                     'fireEvent','MutationObserver','HTMLMenuItemElement','Int8Array',
                     'postMessage','querySelector']);
  p(document, ['getElementsByClassName','querySelector','images','compatMode','documentMode'], t);
  t.all = +(void 0 !== document.all);
  if (window.performance) p(window.performance, ['now'], t);
  p(document.documentElement, ['contextMenu'], t);
  i(e, JSON.stringify(t));
}
```

`p` (line 54–62) calls `h` (line 49–52):

```js
function h(t, n) {
  if (void 0 === t[n]) return 0;
  var a = t[n], r = typeof a;
  return (!a || isArray(a) || (r !== 'object' && r !== 'function')) ? a : 1;
}
```

So encoding rules:
- `undefined` → `0`
- falsy primitive → the primitive (typically `false` or `0`)
- function/object → `1`
- array → the array itself
- otherwise → the value

**Probed property list (24 keys)**:
- on `window`: XDomainRequest, createPopup, removeEventListener, globalStorage, openDatabase, indexedDB, attachEvent, ActiveXObject, dispatchEvent, addBehavior, addEventListener, detachEvent, fireEvent, MutationObserver, HTMLMenuItemElement, Int8Array, postMessage, querySelector
- on `document`: getElementsByClassName, querySelector, images, compatMode, documentMode
- on `document`: `all` → coerced to `+(void 0 !== document.all)` (so `1` if defined, `0` otherwise)
- on `window.performance`: `now`
- on `document.documentElement`: `contextMenu`

**Action**: ensure browser_oxide exposes (or omits) each of these to match Chrome's layout. Each is a discrete bit of presence info; the JSON values must match Chrome exactly. Our captured body `dp` matches real Chrome already — verify in unit test.

---

### `lt` — load timing

**Probe** (line 390–398, function `F`):

```js
function F(e) {
  var n = new Date,
      a = -n.getTimezoneOffset() / 60;       // hours offset, signed
  if (a > 0) a = '+' + a; else a += '';
  t = n.valueOf() + a;                        // epoch_ms concatenated with offset
  i(e, t);
}
```

Captured `lt = 1777448688978-7` → epoch_ms `1777448688978` + tz offset `-7` (PDT, UTC−7). String concat (no separator). Matches.

---

### `ps` — storage availability

**Probe** (line 314–326, functions `N` + `k`):

```js
function k(e) {
  try {
    var t = window[e], n = '__akfp_storage_test__';
    t.setItem(n, n); t.removeItem(n);
    return true;
  } catch (e) { return false; }
}
function N(e) { i(e, k('localStorage') + ',' + k('sessionStorage')); }
```

`ps = "true,true"` matches a working browser with both stores accessible. Two booleans comma-joined.

---

### `cv` — canvas hash (HASH of the canvas data URL — NOT a build identifier!)

**Misleading naming.** The captured field `cv = 1e45567918c75c40a4d45cc3baa14e263b777784` is NOT the bootstrap version hash. It is the **canvas fingerprint** — see line 328–338, function `D`:

```js
function D(e) {
  var n = document.createElement('canvas'),
      a = n.getContext('2d');
  a.fillStyle = 'rgba(255,153,153, 0.5)'; a.font = '18pt Tahoma';
  a.textBaseline = 'top';
  a.fillText('Soft Ruddy Foothold 2', 2, 2);
  a.fillStyle = '#0000FF'; a.fillRect(100, 25, 30, 10);
  a.fillStyle = '#E0E0E0'; a.fillRect(100, 25, 20, 30);
  a.fillStyle = '#FF3333'; a.fillRect(100, 25, 10, 15);
  a.fillText('!H71JCaj)]# 1@#', 4, 8);
  var r = n.toDataURL();
  document.createElement('img').src = r;
  t = c.hash(r);   // SHA-1, hex, lowercase
  i(e, t);
}
```

`c.hash` (line 500–520) is **vanilla SHA-1** producing 40-char lowercase hex. The probe text "Soft Ruddy Foothold 2" + "!H71JCaj)]# 1@#" with 18pt Tahoma + 4 colored rects is the canonical Akamai canvas test string (also seen in older sensor_data versions).

**Action**: this is THE canvas fingerprint. Stable across the same Chrome version on the same OS/GPU. Our current canvas implementation must produce a pixel-perfect rendering of Tahoma-fallback + the rects to match. If we use a different fontstack or different rasterization, our SHA-1 differs. This is a **major** parity field — track separately.

---

### `fp` — flash-plugin via legacy DOM userData behavior

**Probe** (line 209–218, function `S`):

```js
var t = document.createElement('div');
t.style.behavior = 'url(#default#userData)';   // IE-only
document.body.appendChild(t);
t.setAttribute('fsfp', 'true1'); t.save('oXMLStore');
t.removeAttribute('fsfp'); t.load('oXMLStore');
n = ('true1' === t.getAttribute('fsfp'));
```

`fp = false` in modern Chrome (no userData behavior). Matches.

---

### `sp` — Silverlight presence

`sp = false` for non-IE — function `E` line 220–260, returns false on Chrome. Matches.

---

### `br` — browser family detection

**Probe** (line 262–276, function `C`):

```js
var t = (window.opera || ua.indexOf(' OPR/') >= 0) ? 'Opera' : 0,
    n = (typeof InstallTrigger !== 'undefined') ? 'Firefox' : 0,
    a = (Object.prototype.toString.call(window.HTMLElement).indexOf('Constructor') > 0
         || (window.safari && window.safari.pushNotification && ...)
         || window.ApplePaySession);
a = a ? 'Safari' : 0;
var r = a && ua.match('CriOS') ? 'Chrome IOS' : 0,
    o = window.chrome && !t ? 'Chrome' : 0,
    c = (window.ActiveXObject && 'ActiveXObject' in window) || document.documentMode ? 'IE' : 0,
    f = !c && window.StyleMedia ? 'Edge' : 0;
return t || n || f || c || o || r || a || '';
```

**Critical**: `window.ApplePaySession` makes `a = 'Safari'` truthy. The OR-chain order is `Opera || Firefox || Edge || IE || Chrome || ChromeIOS || Safari`. Chrome wins because `window.chrome` is checked BEFORE Safari. So our `ApplePaySession` shim is harmless here (`o = 'Chrome'` wins first). **But on a non-Chrome-with-chrome-object profile, our ApplePaySession shim would flip `br` to Safari — potential bug.** Verify our profile keeps `window.chrome` populated.

---

### `ieps` — IE-only DOM userData persistent storage

`ieps = false` in Chrome. function `S` again; same as `fp`-pattern. Matches.

---

### `av` — Acrobat plugin via legacy ActiveX

`av = false` on non-IE. Matches.

---

### `z` — anti-debug / instrumentation flags

**Probe** (line 124–192, function `w`):

```js
var n = { $: 1, _: 1, B: 1, c: 1, d: 1, e: 1, s: 1, w: 1 };  // first-letter filter
var a = { /* 32 SHA-1 hashes mapped to indices 0..31 */ };
var r = [], o = 0;

for (var c = 0; c < [window, document].length; c++) {
  var f = [window, document][c];
  for (var d in f) if (n[d[0]]) {            // only props starting with $/_/B/c/d/e/s/w
    var l = a[t(d)];                          // SHA-1 of the property name
    if (l !== undefined) { r.push(l); o |= 1; }
  }
}

// Selenium markers on documentElement
var s = ['selenium','driver','webdriver'];
for (...) if (documentElement.getAttribute(s[c])) o |= 2;

// Sequentum scraper marker
if (window.external && external.toString().indexOf('Sequentum') > -1) o |= 4;

i('z', { a: o ^ v, b: +!!(window.XPathResult || document.XPathResult), c: +!(!window.chrome || chrome.runtime) });
i('zh', r + '');
```

Where `v = window['bazadebezolkohpepadr']`. If unset, `v = undefined`, and `o ^ undefined` coerces to `o` (since `undefined → 0` for XOR).

Captured: `z = {"a":1043671558,"b":0,"c":1}`.
- `c: 1` ⟺ `!(window.chrome && !chrome.runtime)` is `false` → `+!false = 1` ⟹ Chrome with no `runtime` (i.e. plain web Chrome, not extension context). Matches.
- `b: 0` ⟺ neither `window.XPathResult` nor `document.XPathResult` truthy. Wait: in real Chrome, `document.XPathResult` IS defined. The probe is `+!!(...)` so `!!truthy = 1`. Captured `b=0` is suspicious — but the body shows raw real-Chrome capture is `b:0`. **This means `XPathResult` is on `XPathResult` global as a constructor but `!!XPathResult` falsy?** Investigate. Possibly Walmart strips it via CSP, or the probe is `+!(!a && !b)` and our parsing is off.

Actually rereading: `b: +!(!w['XPathResult'] && !u['XPathResult'])`. So `b = +!(both undefined) = +!(false) = 1` if either present. Captured `b=0` would mean `XPathResult is undefined on both`. This is unusual for Chrome — possibly Walmart removes it, or our captured body was from an earlier version. Treat as low-priority.

- `a: 1043671558` = `o ^ v` where `v = window.bazadebezolkohpepadr` (server-injected nonce per page). Since `v` differs per page-load, our value of `a` is page-injection-bound, not browser-bound. **As long as we read the nonce from the page and XOR correctly, this matches.**

**Action**: ensure browser_oxide does NOT spuriously expose any of:
- `window` or `document` properties starting with `$`, `_`, `B`, `c`, `d`, `e`, `s`, `w` whose SHA-1 is in the 32-hash bot-detection list (line 137–167 has the full hash table — these are SHA-1s of names like `$cdc_*`, `_phantom`, `webdriver`, `_selenium`, `cdc_*`, etc.)
- `documentElement.getAttribute('selenium' | 'driver' | 'webdriver')`
- `external.toString()` containing 'Sequentum'

If we pass on all three, `o` becomes 0 (or only the original `v`-baseline), and `z.a = v ^ v = 0` ... wait, looking again, `o |= 1` is set whenever ANY enumerable matched. So `o=1` for any match. Real Chrome captures `z.a = 1043671558` which is `v ^ small_int`. Need real-page `bazadebezolkohpepadr` to verify. **Bottom line: avoid leaking any of the 32 known bot markers; comply with the rest is automatic.**

---

### `nav` — navigator JSON

**Probe** (line 64–77, function `O`):

Reads these navigator properties verbatim with the `h()` coercion above:
```
userAgent, appName, appCodeName, appVersion, appMinorVersion, product, productSub,
vendor, vendorSub, buildID, platform, oscpu, hardwareConcurrency, language, languages,
systemLanguage, userLanguage, doNotTrack, msDoNotTrack, cookieEnabled, geolocation,
vibrate, maxTouchPoints, webdriver
```

Plus `plugins` = array of plugin names (line 70–71).

**Action**: every one of those 24 properties must match Chrome-on-macOS. `productSub: '20030107'` (constant for Chrome), `vendor: 'Google Inc.'`, `webdriver: false`, `language: 'en-US'`, `languages: ['en-US','en']`, `hardwareConcurrency: 8` (varies by host CPU), etc.

---

### `nap` — Permissions API enumeration (the 20-digit string)

**Probe** (line 400–435, function `M`):

```js
var n = ['geolocation','notifications','push','midi','camera','microphone','speaker',
         'device-info','background-sync','bluetooth','persistent-storage',
         'ambient-light-sensor','accelerometer','gyroscope','magnetometer',
         'clipboard','accessibility-events','clipboard-read','clipboard-write',
         'payment-handler'];
if (!navigator.permissions) return i(e, 6);
// for each name, navigator.permissions.query({name}).then(result => ...):
//   'prompt'  → digit 1
//   'granted' → digit 2
//   'denied'  → digit 0
//   default    → digit 5
// .catch:
//   if message includes 'is not a valid enum value of type PermissionName' → digit 4
//   else → digit 3
// If exception during scheduling: i(e, 7)
i(e, t.join(''));
```

20 names, one digit each, joined → 20-char string.

Captured: `nap = 11111144242222244122` (20 digits). Decoding by index:

| idx | name | digit | meaning |
|-----|------|-------|---------|
| 0 | geolocation | 1 | prompt |
| 1 | notifications | 1 | prompt |
| 2 | push | 1 | prompt |
| 3 | midi | 1 | prompt |
| 4 | camera | 1 | prompt |
| 5 | microphone | 1 | prompt |
| 6 | speaker | 4 | enum-rejected (Chrome doesn't accept 'speaker' as PermissionName) |
| 7 | device-info | 4 | enum-rejected |
| 8 | background-sync | 2 | granted |
| 9 | bluetooth | 4 | enum-rejected |
| 10 | persistent-storage | 2 | granted |
| 11 | ambient-light-sensor | 2 | granted |
| 12 | accelerometer | 2 | granted |
| 13 | gyroscope | 2 | granted |
| 14 | magnetometer | 2 | granted |
| 15 | clipboard | 4 | enum-rejected |
| 16 | accessibility-events | 4 | enum-rejected |
| 17 | clipboard-read | 1 | prompt |
| 18 | clipboard-write | 2 | granted |
| 19 | payment-handler | 2 | granted |

**Action**: browser_oxide's `navigator.permissions.query({name})` shim must reproduce exactly this state-table for these 20 names on macOS Chrome. Currently undefined behavior in our shim → likely returns `6` (no permissions API) or wrong digits. **High-value field.** Adding a permissions stub keyed off this exact dict is a 30-line patch.

---

### `crc` — `window.chrome` JSON dump

**Probe** (line 437–442, function `T`):

```js
function T(e) {
  var t = { 'window.chrome': window.chrome || '-not-existent' };
  i(e, JSON.stringify(t));
}
```

JSON.stringify drops functions and `undefined`, keeps nested objects. Whatever `window.chrome` enumerates → JSON-flattened.

**Action**: our `window.chrome` shim must produce a JSON-stringifiable object structurally identical to Chrome's. Captured shows nested `app.{InstallState, RunningState}`, `webstore`, `runtime`, `csi`, `loadTimes`, etc. — the standard Chrome-on-Chromium shape. Verify our shim doesn't drop methods (good — they're already dropped by JSON.stringify) and DOES include the constants.

---

### `bt` — Battery API

**Probe** (line 79–97, function `j`):

```js
function j(e) {
  if (!navigator.getBattery) return i(e, 0);
  navigator.getBattery().then(function(t) {
    var n = {};
    for (var a in t) {
      var r = t[a];
      n[a] = (r === Infinity) ? 'Infinity' : r;
    }
    i(e, JSON.stringify(n));
  });
}
```

`for...in` enumerates **own AND inherited** props of BatteryManager. Captured shows `charging, chargingTime, dischargingTime, level, onchargingchange, onchargingtimechange, ondischargingtimechange, onlevelchange`. All event handler props are `null`.

**Action**: our `navigator.getBattery()` must resolve to a BatteryManager-like object whose `for..in` enumerates exactly those 8 keys. `dischargingTime: Infinity` becomes the literal STRING `"Infinity"` in JSON output (note the captured body proves this).

---

### `ap` — async PNG-pixel canvas readback

**Probe** (line 99–113, function `A`):

```js
var t = new Image, n = canvas.getContext('2d');
t.onload = function() {
  n.drawImage(t, 0, 0);
  i(e, 0 === n.getImageData(0, 0, 1, 1).data[3]);
};
t.src = 'data:image/png;base64,iVBORw0KGgoA...';   // a 1×1 transparent PNG
```

The body's `ap` is whatever was in `X.ap` AT POST TIME. Since the load is async and `compute()` doesn't await, `ap=null` is the typical value (initial null left in place). On a slow Image decode on real Chrome → `null`. On a fast load → `true` (since the image is transparent → alpha=0 → `0===0` → true).

**Captured body has `ap=null`.** Our body has `ap=null`. **No parity gap.**

If you want to flip this to `true` to match a "Hi-perf load" capture: implement Image+canvas drawing; the data: URL above decodes to a 1×1 PNG with alpha=0; `getImageData[3]===0` is true. But it's not required for current parity.

---

### `fonts` and `fh` — null on this build

Initialized at line 446–447: `fonts: null, fh: null`. **Never written by the bootstrap.** A separate sensor module (not loaded for the captured page) would compute these. Our null matches.

If a future build computes them, it will likely use a candidate font list with canvas `measureText` width-difference vs serif/sans-serif/monospace. The i7solar pixel.go reference confirms `fh = SHA1(fonts string)`. We have not located the candidate font list for v13 in the wild.

---

### `t` — bootstrap-script identifier hash

Line 456: `i('t', t(v))` where `t(...)` calls `c.hash(...)` (SHA-1) on `v = window.bazadebezolkohpepadr`. So `t = SHA1(server_nonce)`. This is a server-server check — our value is locked to the server-injected nonce, no parity work needed.

---

### `u` — hardcoded build version hash

Line 602: `g = 'b61e5c3c2c505ef7b386f96c66cbd468'` — assigned to `u`. This IS the bootstrap-build hash (32-hex MD5-style). **Our walmart capture has a different `u`** because Walmart ships a different bootstrap build. The `u` is hardcoded into the bootstrap JS — we can't compute it; we'd echo whatever the page-served bootstrap contains.

---

### `cv` — top-level "build cv hash"

Captured `cv = 1e45567918c75c40a4d45cc3baa14e263b777784` (40-char SHA-1) IS the canvas fingerprint (function `D`, see above). **Naming is misleading.** In SamsClub's bootstrap it's the canvas-content SHA-1. Walmart's value is therefore reproducible if our canvas rasterization matches Chrome.

---

### `timing` — per-probe execution durations

`timing.profile.{bp, sr, dp, lt, ps, cv, fp, sp, br, ieps, av, z1..z4, jsv, nav, nap, crc}` records milliseconds taken by each probe (via the `a()` wrapper at line 21–24). The captured `cv: 10` ms is canvas hash time; `nap: 1` ms is permissions-API time. These are weak signals (real Chrome varies wildly), but extreme outliers (e.g. `cv: 0` or `nap: 200`) might raise scores.

---

## Priority-1 deliverables answered

| Field | Status | Action |
|-------|--------|--------|
| `ap` | **Not a probe — async Image-onload writeback. Real Chrome also sends null.** | None. Drop ApplePaySession shim work for this layer (it lives in `br` and could over-trigger Safari-detection). |
| `fonts` | **Not computed in this bootstrap version.** | None. Drop font-enumeration shim work for v13/pixel. |
| `fh` | **Not computed.** Would be `SHA1(fonts)` per i7solar reference. | None. |
| `sr.client` | `[document.body.clientWidth, document.body.clientHeight]` | Verify body geometry post-layout matches real Chrome. |

## Priority-2 deliverables answered

| Field | Encoding | Action |
|-------|----------|--------|
| `bp` | java-hashCode of `[name,desc,filename,mimeDesc,mimeType,mimeSuffixes].toString()` per plugin × mime | Already correct if our plugins shim is correct. |
| `nap` | 20-digit string from `permissions.query({name})` over a fixed 20-name list | **Implement permissions stub** with the state table above. |
| `dp` | 24-property presence map on window/document/performance/documentElement | Verify our presence matches Chrome's; especially `document.all` quirk. |
| `crc` | `JSON.stringify({ "window.chrome": window.chrome })` | Verify our chrome shim is JSON-clean. |
| `z` | XOR-flag bag: window/document enumerable props matching SHA-1 bot list, selenium attrs, Sequentum string | Audit our `window`/`document` enumerables for any name starting with `$_BcdesW` whose SHA-1 is on line 137-167. |
| `bt` | for-in enumeration of BatteryManager → JSON | Implement getBattery shim with 8 specific keys. |

---

## Key sources

1. **`docs/akamai_sensor_analysis/samsclub_akam13_bootstrap.deob.js`** — the canonical 609-line deobfuscated bootstrap. Most-trusted source.
2. **`docs/akamai_sensor_analysis/southwest_akam13_bootstrap.raw.js`** — second sample, byte-similar size, confirms the bootstrap is consistent across sites.
3. **i7solar/Akamai pixel.go** — Go-side encoder for an older variant (1.7x). Confirms field names, hash type (SHA-1 for `fh`, plugin hashes for `bp`, etc.), but its `sr.client = [innerHeight-17, innerWidth]` is WRONG for v13 (newer bootstrap uses `body.clientWidth/Height`). https://github.com/i7solar/Akamai/blob/main/pixel.go
4. **Hyper-Solutions hyper-sdk-py / hyper-sdk-go** — commercial-grade pixel generator, open-source but no field-level docs. https://github.com/Hyper-Solutions/hyper-sdk-py
5. **xvertile/akamai-bmp-generator** — mobile BMP (not web pixel). Wrong layer for our work.

Trust ranking: **(1) > (2) >> (3) > (4) > (5)**. Always defer to the deobfuscated bootstrap.
