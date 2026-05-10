# Canada Goose engine-leak inventory — 2026-05-10 part 2

After landing the CSS calc() math fix (commits 347ab0d + 82bafb6), we
re-ran the Kasada error capture and decrypted the report (the wrapper
crack from commit-pending agent task #4 — see
`docs/kasada_ips_analysis/scratch/CRACK_PROGRESS_2026_05_10.md` —
revealed the wire protocol is `base64(json({"data": base64(xor(plain,
"omgtopkek"))}))`).

Result: **error blob count dropped from 9 to 8** — the 1283-byte CSS
calc precision probe is now gone. But 16 other error-bearing fields
remain in the decrypted report. Listed in priority order; each reveals
a specific addressable engine gap.

## The real engine leaks (16 fields)

### Highest impact — undefined-receiver group (4 fields, 1 root cause)
Fields `bot1225`, `csc`, `kl`, `dpv` all carry the same exception:
```
TypeError: Cannot read properties of undefined (reading 'unjzomuybtbyyhwwkdpkxomylnab')
```
The 28-char identifier `unjzomuybtbyyhwwkdpkxomylnab` looks like an
obfuscated property name from the Kasada VM string table — it's a
probe of *some Web API surface* that our engine returns `undefined`
for, where Chrome returns an object. `bot1225` is reportedly the
single biggest contributor to the trust score. Need to:
1. Search the deobfuscated `kasada_function_bodies.js` and the
   ips.js string table for `unjzomuybtbyyhwwkdpkxomylnab` to identify
   what's being probed.
2. Add a stub for that API.

### Function.prototype.toString source leaks (3 fields)
- `sfc.funcs`: dumps `.toString()` of `queueMicrotask`, `fetch`,
  `HTMLDocument`, `HTMLElement`, etc. Returns our literal JS source
  (`"function queueMicrotask(cb) {\n if (typeof cb != \"function\")...`)
  instead of `[native code]`.
- `sdt.w`: dumps `attachShadow.toString()` returns
  `"attachShadow(init = {}) {\n const mode = init.mode || \"open\";\n
   const shadowId = ops.op_dom_attach_shadow(_getNodeId(this), mode);\n..."`
  — exposes our deno_core op name verbatim.

Fix: every JS-defined function on a Web API prototype needs
`_maskAsNative` (or the equivalent) so `.toString()` returns
`function NAME() { [native code] }`. The helper already exists in
`window_bootstrap.js`; it just isn't applied consistently. Audit
needed across `crates/js_runtime/src/js/*.js`.

### MediaSource probe (`smc`)
```
{"mp4":{"o":"TypeError: Cannot read properties of undefined (reading 'unjzomuybtbyyhwwkdpkxomylnab')"}}
```
Kasada calls `MediaSource.isTypeSupported('video/mp4')` etc. and our
`MediaSource` is undefined. Need a stub class with `isTypeSupported`
that returns Chrome's per-codec verdict for h264/aac/etc.

### structuredClone constructor (`nppm`)
```
TypeError: Failed to construct 'structuredClone': Illegal constructor
```
Kasada tries `new structuredClone()`. Chrome rejects with that exact
text — but does our `structuredClone` actually do this? Need to verify
the message matches Chrome's word-for-word, including the `'` quotes
around the name.

### Class-extension probes (`fsc`, `npc`)
```
fsc: TypeError: Class extends value function toString() { [native code] } is not a constructor or null
npc: TypeError: Class extends value #<C> is not a constructor or null
```
Both probes do `class X extends Y` where Y isn't a constructor. The
error STRING needs to match Chrome's exactly — `#<C>` is V8's anonymous
class representation, may differ in our V8 version.

### Iteration probe (`ao`)
```
TypeError: Invalid attempt to spread non-iterable instance.\nIn order to be iterable, non-array objects must have a [Symbol.iterator]()...
```
Our error message text differs subtly from Chrome's. Probably fine;
verify against captured Chrome reference.

### `String.prototype.toString` probe (`esce`)
```
TypeError: String.prototype.toString requires that 'this' be a String
```
This actually IS Chrome's exact error. Likely passing. Just listed for
completeness.

### Internal helper leak (`esd`)
```
TypeError: this._loadGpuProfile is not a function
```
Our internal `_loadGpuProfile` private method name is leaked through
an error stack. Need to either rename to `[[boxide_internal]]` style or
catch+rewrap with a generic message.

### Function.prototype.toString (`wse`, `bfe`)
```
TypeError: Function.prototype.toString requires that 'this' be a Function
```
Kasada does `Function.prototype.toString.call(someValue)` with a
non-function. The thrown text matches Chrome — but the very fact that
the probe runs and we throw is fingerprintable. Need to check what
Chrome returns vs ours (likely identical; double-check).

### `cbf` — undefined .toString
```
TypeError: Cannot read properties of undefined (reading 'toString')
```
Probably a `[][0].toString()` style probe where a slot we should
populate is empty.

## Recommended next-action sequence
1. **Find what `unjzomuybtbyyhwwkdpkxomylnab` is.** Single biggest
   leverage — fixes `bot1225`/`csc`/`kl`/`dpv`/`smc` simultaneously.
   Grep ips.js + string-table decoder output.
2. **Audit all JS-defined Web API functions for `_maskAsNative`.** The
   `attachShadow` and `queueMicrotask` leaks are the most visible; a
   sweeping audit across `crates/js_runtime/src/js/*_bootstrap.js`
   will catch all instances.
3. **Stub `MediaSource` + `MediaSource.isTypeSupported`.** Even
   non-functional, just needs to be present.
4. **Verify error message text parity** for `Function.prototype.toString`,
   `class extends`, spread, and structuredClone exceptions against
   captured Chrome reference.
5. **Hide `_loadGpuProfile`** (and other internal names) from leaking
   into error stacks.

Each of these is independently shippable.

## Validation tooling now available

- `crates/browser/tests/chrome_compat.rs::kasada_error_blob_capture` —
  intercepts all `cdndex.io/error` POSTs.
- `docs/kasada_ips_analysis/scratch/decrypt_report.py` — decrypts any
  captured blob using the cracked `omgtopkek` XOR key.
- `docs/kasada_ips_analysis/scratch/decrypted_blob_*.json` — the
  current state's decrypted error reports.

Re-run that test after each fix and re-decrypt to see error count drop.
The current baseline is 8 blobs / 16 error fields.
