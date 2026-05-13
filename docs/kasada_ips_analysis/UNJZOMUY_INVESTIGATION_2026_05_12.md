# Kasada Sentinel Property Tag Loss — Investigation (2026-05-12)

Companion to `VM_TRACE_FINDINGS_2026_05_12.md`. The trace identified the
divergence; this doc lists the three most-likely root-cause sites for the
follow-up fix work. **No fix shipped yet — this is a candidate list for
the next session to validate via sniff tests, then patch.**

## Three candidate divergence points

### 1. MediaDevices object-literal method extraction — `window_bootstrap.js:357-375`

```js
_navMediaDevices.enumerateDevices = ({
    enumerateDevices() { ... }
}).enumerateDevices;
```

Object-literal extraction creates a function once. The function reference
is stored in `_navMediaDevices`, accessed via the getter at line 929.
Each `navigator.mediaDevices` returns the same stable object — so identity
*should* be preserved. Probably safe; included as #1 because the `smc`
probe specifically tests MediaSource-related code.

### 2. iframe `getOwnPropertyDescriptor` Proxy handler — `dom_bootstrap.js:2259-2267`

```js
getOwnPropertyDescriptor(target, prop) {
    if (prop in target) return Object.getOwnPropertyDescriptor(target, prop);
    if (typeof prop === "string" && prop in remoteRealm) {
        return { value: remoteRealm[prop], writable: true, enumerable: true, configurable: true };
    }
    return undefined;
}
```

Returns a fresh descriptor object each call, but `.value` points to the
same `remoteRealm[prop]` function. So `descriptor.value === descriptor2.value`
should hold and tags persist. Need to verify with sniff test 2 below.

### 3. `_defProtoMethod` wrapper recreation — `window_bootstrap.js:81-95`

```js
const wrapped = ({ [name](...args) { return fn.apply(this, args); } })[name];
```

Creates a new function object PER CALL to `_defProtoMethod`. If a method is
installed twice under the same name (accidental double-call), the new wrapper
is a different object than the old. The first one might be tagged, the second
returned untagged on re-access.

## Recommended sniff tests

Add these to `chrome_compat.rs` and check identity stability:

```js
// Test 1: enumerateDevices identity stability
const a = navigator.mediaDevices.enumerateDevices;
const b = navigator.mediaDevices.enumerateDevices;
console.assert(a === b, "enumerateDevices identity divergence");
a.unjzomuybtbyyhwwkdpkxomylnab = 'tag';
console.assert(b.unjzomuybtbyyhwwkdpkxomylnab === 'tag', "tag lost on re-access");

// Test 2: iframe descriptor value identity
const iframe = document.createElement('iframe');
document.body.appendChild(iframe);
const w = iframe.contentWindow;
const d1 = Object.getOwnPropertyDescriptor(w, 'Function');
const d2 = Object.getOwnPropertyDescriptor(w, 'Function');
console.assert(d1.value === d2.value, "Function descriptor value changed");
d1.value.sentinel_test = 'tag';
console.assert(d2.value.sentinel_test === 'tag', "iframe tag lost");

// Test 3: _defProtoMethod re-installation identity
const proto = navigator.constructor.prototype;
const a2 = Object.getOwnPropertyDescriptor(proto, 'sendBeacon')?.value;
const b2 = Object.getOwnPropertyDescriptor(proto, 'sendBeacon')?.value;
console.assert(a2 === b2, "Navigator method identity divergence on re-access");
```

If any test fails, that's the divergence point. Each test takes <50 lines
to add to `chrome_compat.rs`.

## Most-likely root cause (agent's verdict)

Handler 32 throw pattern `var l = e(n); if (l[h] && ...)` consistently
indicates `l === undefined` from the value-fetcher. Either:
- We're not creating the frame object with `unjzomuybtbyyhwwkdpkxomylnab`
- OR the frame was tagged but that tag is on a **different function object**
  than the one Kasada expects to find on re-read

The most actionable hypothesis: **Kasada tags a function via
`Object.getOwnPropertyDescriptor(...).value`** and on re-access we return
a functionally equivalent but distinct function object. Most reliable if a
getter recreates the value on each call, or a Proxy `get` handler returns
a fresh wrapper.

## Next steps

1. Add sniff tests 1-3 to `chrome_compat.rs`. Each test failure pinpoints a
   divergence site.
2. Patch the failing site to return stable references.
3. Re-run `kasada_vm_dispatcher_trace` and confirm `unjzomuybtbyyhwwkdpkxomylnab`
   throws drop from 5 to 0.
4. Re-run `kasada_error_blob_capture` and verify `bot1225/csc/kl/dpv/smc`
   probes flip to `e:0`.

Estimated effort: 0.5 day for tests + investigation, 0.5–2 days for the fix
depending on which site is the culprit.
