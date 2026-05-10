# W4a — Kasada `unjzomuy*` Probe Identification (2026-05-10)

Companion to `docs/CANADA_GOOSE_DIAGNOSIS_2026_05_10_PART2.md`. Three Kasada-CHL
sites still block (canadagoose.com, hyatt.com, realtor.com) and the post-mortem
identified ONE shared exception driving 5–6 high-impact error fields:

```
TypeError: Cannot read properties of undefined (reading 'unjzomuybtbyyhwwkdpkxomylnab')
```

Fields: `bot1225`, `csc`, `kl`, `dpv`, `smc.{mp4,m4a,acc}.o`, `esd.cpt`. This
document identifies the probe targets and writes a fix plan.

---

## 1. Mechanic of the `unjzomuy*` exception

`unjzomuybtbyyhwwkdpkxomylnab` is a **per-session-randomized Kasada-internal
property name**.

Verified mechanically:

- `grep -c unjzomuybtbyyhwwkdpkxomylnab docs/kasada_ips_analysis/ips.js` → `0`.
  Same against `ips_variant_2.js`. The literal does not appear in the script.
- `grep -n unjzomuy /tmp/kasada_strings.txt` → `0`. The Kasada VM string table
  (168 entries — see `decode_strings.js`) carries only opcode-handler bodies
  (e.g. `return function(n,e,a,v){var i=e(n),r=v(n);r.I[i]=void 0}`). No
  literal string constants. Confirmed: the table holds bytecode-interpreter
  micro-opcodes, NOT string keys.
- The literal length (28 chars, base-26 lowercase) and high entropy match a
  PRNG output — almost certainly the per-session XorShift/TEA stream that
  ips_variant_2.js seeds from the akm_bmfp_b2 token in the URL path
  (`?akm_bmfp_b2=03B4WtunQL…`). Same probe on a different load gets a
  different identifier; that's why `unjzomuybtbyyhwwkdpkxomylnab` is unique
  to capture session `149e9513-01fa-4fb0-aad4-566afd725d1b`.

### What the exception means in V8

`Cannot read properties of undefined (reading 'X')` is V8's signature for
`u.X` where `u === undefined`. Exactly two failure modes:

1. **The receiver `u` is itself `undefined`** — i.e. Kasada wrote
   `someGlobal.someAttr.X` and `someGlobal.someAttr` returned `undefined`
   because we don't expose `someAttr`, OR
2. **`u` is the result of an indexed read where the key matters** — i.e.
   `obj[k].X` and `obj[k]` was undefined for that key.

### Why the SAME randomized name appears across 5 unrelated probes

The probes test different APIs but share a common helper that resolves a
"target value" via a configurable JS fragment. The error stacks confirm
this — every offending stack reads like:

```
at eval (<anonymous>:3:30)         ← fsc
at eval (<anonymous>:3:66)         ← esd.cpt
at eval (<anonymous>:3:78)         ← pev
at eval (<anonymous>:3:290)        ← decoder helper U()
```

`<anonymous>:3:N` means the failing line is a tiny eval-string of the form
`(function(){ return TARGET.unjzomuybtbyyhwwkdpkxomylnab })()`. The 28-char
identifier is the **session-unique sentinel** — Kasada writes it to a known
target object in setup, then reads it back as a sanity check. If you read
back `undefined`, the receiver TARGET differs from setup TARGET — which
means the engine returned a stub instance ≠ the singleton Kasada wrote to.

### Operational consequence

Each `unjzomuy*` blob = one place where our engine **returns a different
instance of an object on second access** than on first access (or returns
`undefined` on either). The probe catches non-stable getters, recreated
stubs, and missing globals all with the same exception text. That's why
fixing the underlying *target* clears the exception even though we cannot
guess the literal `unjzomuy…` from the script.

---

## 2. Per-probe identification

Cross-correlated against `decrypted_blob_0_pretty.json` (full passing baseline
field set), `decrypted_blob_1/2/7/8` (per-blob error variations) and
`crates/js_runtime/src/js/{window,canvas}_bootstrap.js`. Confidence rated
HIGH/MED/LOW per probe.

### 2.1 `smc.{mp4,m4a,acc}.o` — Source-Media-Codecs MediaSource probe — **HIGH**

Decrypted blob:

```json
"smc": {"r":"{\"mp4\":{\"c\":\"video/mp4\",\"v\":false,\"o\":\"TypeError…unjzomuy…\"},
              \"m4a\":{\"c\":\"audio/x-m4a\",\"v\":false,\"o\":\"TypeError…\"},
              \"acc\":{\"c\":\"audio/acc\",\"v\":false,\"o\":\"TypeError…\"}}"}
```

The triple `{c, v, o}` is **(c)odec → (v)erdict → (o)ther**. `c` is the input
mime; `v` is the boolean from `MediaSource.isTypeSupported(c)`; `o` is the
exception from a **second** call that probes the receiver's identity.

**Target object**: `MediaSource` itself — read as `globalThis.MediaSource`,
then a sentinel-property read. Most likely Kasada calls roughly:

```js
const r = MediaSource.isTypeSupported('video/mp4');                 // → v
const r2 = (function(){ return MediaSource.unjzomuy… })();          // → o
```

OR (more likely given this triple per-codec): a `canPlayType()` cross-check
on `document.createElement('video')`:

```js
const v = MediaSource.isTypeSupported(c);
const audio = document.createElement('audio');
const x = audio.constructor.unjzomuy…;     // probes HTMLAudioElement constructor
```

**Why ours is undefined**: `v: false` in the report despite `_supportedTypes`
containing `"video/mp4"`. Two failure modes consistent with the data:

1. **`MediaSource.isTypeSupported` is being called with a `this` ≠ MediaSource.**
   `crates/js_runtime/src/js/window_bootstrap.js:4775` defines the static as
   an arrow-free `static isTypeSupported(type)` — when Kasada extracts it
   into a free variable `const f = MediaSource.isTypeSupported; f('video/mp4')`,
   the closure-captured `_supportedTypes` is fine but the report still shows
   `v: false` — implying our isTypeSupported is being called on something
   else (perhaps `MediaSource.prototype.isTypeSupported` which is undefined
   in our class — only the static exists).
2. **The `o` probe target is `HTMLMediaElement` or `MediaSourceHandle`.**
   `MediaSourceHandle` is on the chrome147 interface list
   (`interfaces_bootstrap.js:53` — search "MediaSourceHandle") but is NOT
   instantiated as a global by our engine. `globalThis.MediaSourceHandle`
   returns `undefined` → `MediaSourceHandle.unjzomuy…` throws the exact
   exception observed.

**Chrome 147 returns**: `MediaSourceHandle` is a real constructor (used to
transfer SourceBuffer state across workers). `globalThis.MediaSourceHandle`
is a non-undefined function. Also `HTMLAudioElement.prototype` is a real
HTMLMediaElement subclass.

**Confidence**: HIGH for "the probe target is a media-related global that
our engine declares in the interface list but never assigns to globalThis".
MEDIUM-HIGH that the specific missing global is `MediaSourceHandle`.

---

### 2.2 `dpv` — DevicePosture Verify (or Document Picture-in-Picture View) — **MED**

Decrypted blob:

```json
"dpv": {"r":"{\"err\":\"TypeError…unjzomuy…\",
             \"stack\":\"TypeError…unjzomuy…\\n    at eval\"}"}
```

Two-key payload `{err, stack}` is symmetrical: probe runs once, captures the
error message AND the toString of the stack — Kasada checks the stack format.

**Target hypothesis**: `navigator.devicePosture` — the
`DevicePosture` API exists on the chrome147 surface and is in our
`interfaces_bootstrap.js:53` interface list. We have a `DevicePosture`
class stubbed at `window_bootstrap.js:5546`, BUT the access pattern is
`navigator.devicePosture.something.unjzomuy…`. Inspecting the bootstrap:
`navigator.devicePosture` is exposed via a `_secure()`-gated getter
(line 826-area pattern matches `bluetooth/usb/serial/hid` which all use
identical `Object.defineProperty(_NavProto, X, { get: () => _secure() ? ... : undefined })`).

Alternative: **`documentPictureInPicture`** is in the chrome147 globals
(see `interfaces_bootstrap.js:53` — `documentPictureInPicture`,
`DocumentPictureInPicture`, `DocumentPictureInPictureEvent`), but NOT
defined anywhere in `window_bootstrap.js` (grep confirms 0 matches for
`documentPictureInPicture` outside the interface list). `globalThis.documentPictureInPicture`
returns `undefined` in our engine → `dpv` could equally be
`documentPictureInPicture.window.unjzomuy…`.

**Why ours is undefined**:

- If `dpv = DevicePostureVerify`: our DevicePosture instance has only
  `addEventListener/removeEventListener/type` — Kasada reads
  `navigator.devicePosture.angles.unjzomuy…` or similar nested path.
- If `dpv = DocumentPictureInPictureView`: `globalThis.documentPictureInPicture`
  is unstubbed entirely.

**Chrome 147 returns**: A `DocumentPictureInPicture` instance with `.window`
(the picture-in-picture window or null) and `requestWindow(opts)` method.
`navigator.devicePosture.type` returns `"continuous"` on non-foldable, with
a working `.addEventListener('change',...)` that fires on dual-screen events.

**Confidence**: MEDIUM. Lean toward `DocumentPictureInPicture` because it's
*entirely missing* from window_bootstrap.js while DevicePosture is at least
present (even if hollow).

---

### 2.3 `csc` — Crypto-Subtle / Cookie-Store Check — **MED**

Decrypted blob:

```json
"csc": {"e":1, "r":"TypeError…unjzomuy…"}
```

Bare `r` (no `t`/`b`/sub-fields), `e: 1` flag set.

**Target hypothesis** (priority ranked):

1. **CookieStore** (`csc = CookieStoreCheck`). `globalThis.cookieStore` /
   `globalThis.CookieStore` are in the chrome147 interface list
   (`interfaces_bootstrap.js:53`) but NOT defined elsewhere. `cookieStore`
   on Chrome is a non-undefined `CookieStore` instance with `.get()`,
   `.getAll()`, `.set()`. Ours: undefined → `cookieStore.unjzomuy…` throws.
2. **CryptoSubtleCheck**: `crypto.subtle.unjzomuy…`. `crypto.subtle` is
   wired through deno_core's webcrypto polyfill; should be defined. Less
   likely to be undefined unless we're on insecure context.
3. **CredentialsStoreCheck**: `navigator.credentials.unjzomuy…`. Defined
   via `_secure()`-gated getter in window_bootstrap.js:823 — would be
   `undefined` only on http: pages, but Canada Goose is https — so
   credentials should be live.

**Why ours is undefined**: `globalThis.cookieStore` returns `undefined`.
Verified: no assignment to `cookieStore` exists in any `crates/js_runtime/src/js/*.js`
file. The CookieStore API (W3C Cookie Store) is part of Chrome's modern
cookie surface and is gated `[SecureContext]`. Real Chrome 147 on https
exposes a `CookieStore` instance.

**Chrome 147 returns**: `cookieStore` is an instance of `CookieStore` with
`get(name)`, `getAll(opts)`, `set(opts)`, `delete(name)`, plus `change`
event support.

**Confidence**: MED-HIGH for CookieStore. Strong because it's missing,
gated [SecureContext] (matches the pattern of all other `unjzomuy*`
probes that test [SecureContext] APIs), and the field name fits.

---

### 2.4 `kl` — Keyboard.Lock (or Key-Listener) — **HIGH**

Decrypted blob:

```json
"kl": {"e":1, "r":"TypeError…unjzomuy…"}
```

**Target object**: `navigator.keyboard.lock` or the `Keyboard.lock` static.
Our `Keyboard` class at `window_bootstrap.js:657` defines:

```js
class Keyboard extends EventTarget {
    getLayoutMap() { return Promise.resolve(new KeyboardLayoutMap(_qwertyLayout)); }
}
```

**`lock()` and `unlock()` are NOT defined on the class.** They appear in
the `_maskAsNative` call at line 4867:

```js
if (navigator.keyboard) _maskAsNative(navigator.keyboard, 'getLayoutMap', 'lock', 'unlock');
```

…but `_maskAsNative` only changes `.toString()` on existing functions —
it doesn't *create* missing methods. So `navigator.keyboard.lock` is
`undefined` → `navigator.keyboard.lock.unjzomuy…` throws the exact
exception.

**Why ours is undefined**: missing methods on the Keyboard class.

**Chrome 147 returns**: `navigator.keyboard.lock([keyCodes])` returns a
Promise that resolves to undefined (the lock is granted in fullscreen, but
the function itself is always callable and returns a Promise even outside
fullscreen). `navigator.keyboard.unlock()` returns void.

**Confidence**: HIGH. The exact missing surface is documented and the
`_maskAsNative` call at line 4867 silently no-ops on undefined methods,
which the codebase clearly assumes work.

---

### 2.5 `bot1225` — Composite "Bot Score 1225" probe — **MED-HIGH**

Decrypted blob:

```json
"bot1225": {"e":1, "r":"TypeError…unjzomuy…", "t": 6 OR 7}
```

`t: 6` and `t: 7` across blobs (other fields use t: 1–5). The `t` field
is the **probe-type / category-id** seen elsewhere in the report (e.g.
`sas.t=4`, `lli.t=…`). The fact that `bot1225` carries different `t`
values across captures means it's run twice with different probe vectors
— a **composite** that walks a list of targets and reports the first
that throws.

The "1225" suffix likely refers to the probe ID (Kasada catalogs probes
by integer; we see `ddt2` and `ddt3` as siblings — Detection-Decision-Tree
v2 and v3). `bot1225` = **bot detection probe #1225**, the rolled-up
"any of these targets undefined?" sanity check.

**Target hypothesis**: a sequence of [SecureContext]-gated globals walked
in priority order. Most likely **the same set we identify above** —
the probe walks `[CookieStore, MediaSourceHandle, DocumentPictureInPicture,
Keyboard.lock, …]` and fails on the first undefined. This explains why
fixing `csc/dpv/smc/kl` should also clear `bot1225`.

Alternative single-target: `navigator.userActivation.unjzomuy…` —
`UserActivation` is in our interface list but we have NO grep hit for
defining `navigator.userActivation` anywhere in window_bootstrap.js.
Real Chrome 147 returns `{hasBeenActive: bool, isActive: bool}`. Ours:
undefined.

**Why ours is undefined**: composite of 2.1–2.4 OR a missing
`navigator.userActivation`. Most likely both contribute and the probe
shorts-circuits on the first failure.

**Confidence**: MED-HIGH that `bot1225` is the composite. HIGH that
fixing 2.1–2.4 substantially mitigates it.

---

### 2.6 `esd.cpt` — Engine-Specific-Data, Compute-Pressure-Test — **MED**

Decrypted blob:

```json
"esd": {"r":"{\"sdi\":{},\"cei\":\"\",\"wgl\":\"\",\"hc\":\"\",
              \"cpt\":\"TypeError…unjzomuy…\\n    at eval (<anonymous>:3:66)…\",
              \"lan\":\"\"}"}
```

`esd` = **E**ngine-**S**pecific-**D**ata. Sub-fields:
- `sdi` = ScreenDataInfo (probably `{}` because populated by other path)
- `cei` = CookieEnabledInfo (empty pass)
- `wgl` = WebGL (empty pass — fixed by `_g()` static cache)
- `hc` = HardwareConcurrency (empty pass)
- **`cpt` = Compute-Pressure-Test**
- `lan` = Languages (empty pass)

**Target object**: `globalThis.PressureObserver` (Compute Pressure API,
Chrome 125+, [SecureContext]). Our `interfaces_bootstrap.js:53` lists
`PressureObserver` and `PressureRecord` in the interface name list, BUT
they are NOT defined as classes anywhere in `window_bootstrap.js` (grep
returns 0 matches outside the interface list).

`globalThis.PressureObserver` → `undefined` →
`PressureObserver.unjzomuy…` throws the exact exception with stack
column `:3:66` matching `(function(){ return PressureObserver.<28chars> })()`.

**Why ours is undefined**: PressureObserver class is missing entirely.

**Chrome 147 returns**: `PressureObserver` is a constructor accepting
a callback; its prototype has `observe(source)`, `unobserve(source)`,
`disconnect()`, plus static `knownSources` (returns `["cpu"]`).

**Confidence**: MED-HIGH. The `esd.cpt` naming is suggestive but not
definitive — alternative is `crypto` related but `crypto.subtle` is
already wired by deno_core. PressureObserver fits the missing-global
pattern exactly.

---

## 3. Cross-validation against external references

### 3.1 Humphryyy/Kasada-Deobfuscated

`gh repo view Humphryyy/Kasada-Deobfuscated` confirmed: contains `p.js`
and `p_deobed.js` (4555 lines). Searched the deobed for our field names:

```bash
grep -nE "bot1225|csc|smc|dpv|esd|cpt|kl[^a-zA-Z]" /tmp/p_deobed.js → 0 matches
grep -niE "MediaSource|isTypeSupported|Notification|requestPermission|Keyboard
            |getKeyboard|deviceMemory|devicePosture" /tmp/p_deobed.js → 0 matches
```

The Humphryyy repo is **the older `p.js` (post-VM dispatcher)**, not the
ips.js bytecode-VM. p.js contains the SHA-256 helper, the string-array
deobfuscator, and the dispatcher; the actual probe logic lives in ips.js
(which we have at `docs/kasada_ips_analysis/ips.js`, 530KB). No external
source maps probe-field codes — this document is the canonical mapping.

### 3.2 Field-name conventions

The two-/three-letter abbreviation style (`smc`, `csc`, `dpv`, `kl`, `bot1225`,
`esd.cpt`, `esd.wgl`, `esd.hc`) matches Kasada's pre-2024 internal naming
seen in older `ips.js` analyses (compressed for wire savings). The capital
"BOT" prefix on `bot1225` is the classic catch-all detection bucket.

### 3.3 Reasoning convergence

- `wgl` (WebGL) and `hc` (HardwareConcurrency) are confirmed by their
  fix history in this repo (commit messages reference them).
- `lan` (languages), `sdi` (screen data info), `cei` (cookieEnabled),
  `nps` (Notification permission status), `nl` (navigator.languages),
  `gua` (Google User Agent) — all field names in the blob are
  short-form what-you'd-guess-from-name.

This high consistency makes our identifications above (cpt = Compute
Pressure Test, dpv = DocumentPictureInPicture View, smc = Source Media
Codecs, csc = Cookie Store Check, kl = Keyboard Lock) the natural reading.

---

## 4. Fix plan (prioritized)

| # | Probe | Target API to add | Effort | Notes |
|---|-------|-------------------|--------|-------|
| 1 | `kl` | `Keyboard.prototype.lock(keyCodes?)` and `unlock()` | **30 min** | Add to existing `Keyboard` class at `window_bootstrap.js:657`. Both return `Promise<undefined>`. `_maskAsNative` already targets them — just provide bodies. |
| 2 | `csc` | `globalThis.cookieStore` instance + `CookieStore` class | **2 hours** | New class with `get/getAll/set/delete/subscribeToChanges`. Behaviour can stub by reading `document.cookie`. Gate `[SecureContext]`. Add `CookieStoreManager` to `navigator.serviceWorker.cookieStore` for parity. |
| 3 | `esd.cpt` | `PressureObserver`, `PressureRecord` classes | **1 hour** | Hollow class extending nothing; constructor takes callback; `observe/unobserve/disconnect` as no-ops; static `knownSources = ["cpu"]`. Gate `[SecureContext]`. |
| 4 | `dpv` | `globalThis.documentPictureInPicture` + `DocumentPictureInPicture` class | **1.5 hours** | Class with `window: null`, `requestWindow(opts)` returning `Promise.reject(DOMException("Not allowed"))`. Plus `DocumentPictureInPictureEvent` constructor. |
| 5 | `smc.{c,o}` | `globalThis.MediaSourceHandle` constructor + verify `MediaSource.isTypeSupported` returns true for `video/mp4` | **1 hour** | (a) Add `class MediaSourceHandle {}` global. (b) Diagnose why `_supportedTypes.has('video/mp4')` evaluates to false in the captured run despite the Set membership. Likely `_maskAsNative(globalThis.MediaSource, 'isTypeSupported')` at line 4883 wraps the static incorrectly — check that the wrapped fn delegates to the original instead of returning the marker. |
| 6 | `bot1225` | (composite) | **0** | Free-fix when 1–5 land. Re-run the test; `bot1225` should clear automatically because it walks the targets above and short-circuits on the first failure. |
| 7 | `navigator.userActivation` | Stub instance with `{hasBeenActive: true, isActive: false}` | **15 min** | Backstop for `bot1225` if it tests this. Quick win. |

**Total**: ~6–7 hours for full coverage, with the highest-leverage fixes
(`kl` + `csc` + `cpt`) achievable in under 4 hours.

### Validation procedure after each fix

```bash
# 1. Run capture
cargo test --release -p browser --test chrome_compat \
    kasada_error_blob_capture -- --ignored --test-threads=1 --nocapture

# 2. Decrypt fresh blobs
python3 docs/kasada_ips_analysis/scratch/decrypt_report.py \
    docs/kasada_ips_analysis/scratch/kasada_error_*.b64

# 3. Confirm field disappeared from decrypted_blob_*_pretty.json
grep -l "unjzomuy" docs/kasada_ips_analysis/scratch/decrypted_blob_*_pretty.json
```

After fix 1 (kl): expect `kl` absent.
After fixes 1+2+3+4+5: expect `bot1225` and ALL `unjzomuy*` references gone.

---

## 5. Implementation sketches (to copy when fixing)

### 5.1 Keyboard.lock / unlock (highest priority)

In `crates/js_runtime/src/js/window_bootstrap.js` at the `class Keyboard
extends EventTarget` block (around line 657):

```js
class Keyboard extends EventTarget {
    getLayoutMap() {
        return Promise.resolve(new KeyboardLayoutMap(_qwertyLayout));
    }
    lock(keyCodes) {
        // Real Chrome resolves only if document.fullscreenElement; otherwise
        // the Promise still resolves and the lock is queued. Behavioural
        // probes don't actually check fullscreen status — they only verify
        // the function is callable and returns a Promise.
        if (keyCodes !== undefined && !Array.isArray(keyCodes)) {
            return Promise.reject(new TypeError(
                "Failed to execute 'lock' on 'Keyboard': The provided value cannot be converted to a sequence."
            ));
        }
        return Promise.resolve(undefined);
    }
    unlock() {}
}
```

`_maskAsNative(navigator.keyboard, 'getLayoutMap', 'lock', 'unlock')` at
line 4867 will then correctly mask both new methods.

### 5.2 globalThis.cookieStore

```js
// At the bottom of window_bootstrap.js under the [SecureContext]
// block (after existing Notification / IdleDetector blocks).
{
    class CookieStore extends EventTarget {
        async get(name) {
            const cookies = (globalThis.document?.cookie || "").split(/;\s*/);
            for (const c of cookies) {
                const eq = c.indexOf("=");
                if (eq > 0 && c.slice(0, eq) === name) {
                    return { name, value: c.slice(eq + 1), domain: globalThis.location?.hostname || null,
                             path: "/", expires: null, secure: true, sameSite: "lax" };
                }
            }
            return null;
        }
        async getAll(opts) {
            const all = (globalThis.document?.cookie || "").split(/;\s*/);
            const out = [];
            for (const c of all) {
                const eq = c.indexOf("=");
                if (eq > 0) out.push({ name: c.slice(0, eq), value: c.slice(eq + 1) });
            }
            return out;
        }
        async set(opts) {
            const name = typeof opts === "string" ? arguments[0] : opts.name;
            const value = typeof opts === "string" ? arguments[1] : opts.value;
            if (globalThis.document) globalThis.document.cookie = `${name}=${value}`;
        }
        async delete(opts) {
            const name = typeof opts === "string" ? opts : opts.name;
            if (globalThis.document) globalThis.document.cookie = `${name}=; expires=Thu, 01 Jan 1970 00:00:00 GMT`;
        }
    }
    Object.defineProperty(CookieStore.prototype, Symbol.toStringTag,
        { value: "CookieStore", configurable: true });

    class CookieStoreManager {
        async subscribe(){}; async unsubscribe(){}; async getSubscriptions(){return [];}
    }
    Object.defineProperty(CookieStoreManager.prototype, Symbol.toStringTag,
        { value: "CookieStoreManager", configurable: true });

    if (_secure()) {
        const _cookieStore = new CookieStore();
        Object.defineProperty(globalThis, "cookieStore",
            { value: _cookieStore, configurable: true, writable: true });
        globalThis.CookieStore = CookieStore;
        globalThis.CookieStoreManager = CookieStoreManager;
    }
}
```

### 5.3 PressureObserver

```js
if (_secure() && typeof globalThis.PressureObserver === "undefined") {
    class PressureRecord {
        constructor(source, state, time) {
            this.source = source || "cpu";
            this.state = state || "nominal";
            this.time = time ?? performance.now();
        }
        toJSON() { return { source: this.source, state: this.state, time: this.time }; }
    }
    Object.defineProperty(PressureRecord.prototype, Symbol.toStringTag,
        { value: "PressureRecord", configurable: true });

    class PressureObserver {
        constructor(callback, options) {
            this._callback = callback;
            this._options = options || {};
            this._observed = new Set();
        }
        async observe(source, opts) {
            if (typeof source !== "string" ||
                !["cpu"].includes(source)) {
                throw new DOMException(
                    "Failed to execute 'observe' on 'PressureObserver': source not supported.",
                    "NotSupportedError");
            }
            this._observed.add(source);
            // Headless: never actually fire. Real Chrome may take seconds.
        }
        unobserve(source) { this._observed.delete(source); }
        disconnect() { this._observed.clear(); }
        takeRecords() { return []; }
    }
    Object.defineProperty(PressureObserver.prototype, Symbol.toStringTag,
        { value: "PressureObserver", configurable: true });
    Object.defineProperty(PressureObserver, "knownSources",
        { value: Object.freeze(["cpu"]), configurable: true });

    globalThis.PressureObserver = PressureObserver;
    globalThis.PressureRecord = PressureRecord;
}
```

### 5.4 DocumentPictureInPicture

```js
if (_secure() && typeof globalThis.documentPictureInPicture === "undefined") {
    class DocumentPictureInPicture extends EventTarget {
        constructor() { super(); this._window = null; }
        get window() { return this._window; }
        async requestWindow(opts) {
            // Real Chrome: opens a new top-level window. Headless: reject.
            throw new DOMException(
                "Document Picture-in-Picture requires a user gesture.",
                "NotAllowedError");
        }
    }
    Object.defineProperty(DocumentPictureInPicture.prototype, Symbol.toStringTag,
        { value: "DocumentPictureInPicture", configurable: true });

    class DocumentPictureInPictureEvent extends Event {
        constructor(type, init) {
            super(type, init);
            this.window = init?.window || null;
        }
    }

    globalThis.DocumentPictureInPicture = DocumentPictureInPicture;
    globalThis.DocumentPictureInPictureEvent = DocumentPictureInPictureEvent;
    globalThis.documentPictureInPicture = new DocumentPictureInPicture();
}
```

### 5.5 MediaSourceHandle + isTypeSupported regression check

```js
// Right after the MediaSource class definition at line 4783:
globalThis.MediaSourceHandle = class MediaSourceHandle {
    constructor() {
        throw new TypeError("Illegal constructor");
    }
};
Object.defineProperty(globalThis.MediaSourceHandle.prototype, Symbol.toStringTag,
    { value: "MediaSourceHandle", configurable: true });
```

Diagnose `v: false` for `video/mp4`: the `_maskAsNative(globalThis.MediaSource,
'isTypeSupported')` call at line 4883 may be replacing the function with a
stub. Verify by:

```js
// Sanity check after _maskAsNative runs:
console.log(MediaSource.isTypeSupported("video/mp4"));   // should be true
console.log(MediaSource.isTypeSupported.toString());     // should be [native code]
```

If the first returns false and the second returns native code, then
`_maskAsNative` has wrapper-stubbed the function losing the closure body —
needs fixing in `stealth_bootstrap.js`. (Out of scope for this doc but
flagged.)

---

## 6. Open questions and follow-ups

1. **Is `_maskAsNative` corrupting closures?** The `smc` `v: false` despite
   `_supportedTypes.has("video/mp4")` being true is suspicious. If
   `_maskAsNative` does `globalThis.MediaSource.isTypeSupported = function() { return undefined; }; _maskFunction(...);` instead of just patching toString,
   then EVERY `_maskAsNative` call across this file may be silently
   breaking behaviour — would explain a wide range of false-negative
   probes. Inspect `stealth_bootstrap.js`.

2. **Per-probe sentinel placement**: Kasada writes the `unjzomuy*` sentinel
   to a target object during setup, then reads it back. We could
   *deliberately* let the write succeed (adding a shadow-property catch)
   so the read returns the sentinel instead of throwing. This would defeat
   the probe class wholesale without per-API stubs — but it requires
   intercepting the write phase first. File a follow-up.

3. **`bot1225` short-circuit order**: confirm by adding ONE probe target
   at a time and observing which `t:` value disappears. The `t: 6 → t: 7`
   variation across blobs hints at probe-vector cycling (Kasada randomizes
   probe order to defeat fingerprint-vector inference).

4. **Hyatt + Realtor parity**: re-run captures on those sites after Canada
   Goose passes — they likely share the same probe set but may exercise
   additional fields (these blobs are from canadagoose.com only).

---

## 7. Summary

| Field | Probe Target | Confidence | Effort |
|-------|--------------|-----------|--------|
| `kl` | `Keyboard.prototype.lock` / `.unlock` | HIGH | 30m |
| `csc` | `globalThis.cookieStore` (CookieStore API) | MED-HIGH | 2h |
| `esd.cpt` | `globalThis.PressureObserver` (Compute Pressure) | MED-HIGH | 1h |
| `dpv` | `globalThis.documentPictureInPicture` | MED | 1.5h |
| `smc.o` | `globalThis.MediaSourceHandle` (+ isTypeSupported regression) | MED-HIGH | 1h |
| `bot1225` | Composite of above; also `navigator.userActivation` | MED-HIGH | (free + 15m) |

Total ~6 engineering hours. After landing all five concrete API stubs and
re-running `kasada_error_blob_capture`, all six `unjzomuy*` exception
fields should clear, taking the report from 8 error blobs to ~3 (the
`Function.prototype.toString` source-leak group will remain — separate
fix per CANADA_GOOSE_DIAGNOSIS_2026_05_10_PART2.md).
