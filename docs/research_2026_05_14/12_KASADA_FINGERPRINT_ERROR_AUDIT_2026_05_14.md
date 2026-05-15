# Kasada Fingerprint Error Audit — 2026-05-14

Direct analysis of `docs/kasada_ips_analysis/scratch/decrypted_blob_{1,2}_pretty.json`
— Kasada's reporting POST body to `reporting.cdndex.io/error`, cracked
via 9-byte XOR(`omgtopkek`) per `CRACK_PROGRESS_2026_05_10.md`.

The blob contains 118 fields representing Kasada's view of our engine.
The KEY observation: **some fields contain TypeError stack traces** —
those are probes that crashed on our engine where they'd succeed on
real Chrome. Each crash is a discrete fingerprint gap.

## Confirmed engine gaps (from blob_1 & blob_2)

### Group A — `unjzomuybtbyyhwwkdpkxomylnab` sentinel lookup on undefined

The sentinel-attach probes that the W1.1 realm cache was supposed to
fix. Confirmed via the audit test that our 3 hypothesized loss sites
are NOT the problem; these crashes show DIFFERENT lookup sites.

| Probe | Error | Likely source |
|-------|-------|---------------|
| `smc` (Source Media Capability) | `TypeError: Cannot read properties of undefined (reading 'unjzomuybtbyyhwwkdpkxomylnab')` for mp4 / m4a / acc | Kasada calls `MediaSource.makeIsTypeSupported(type)` or similar that returns undefined in our engine |
| `csc` | Same sentinel error | Probably a parallel media-codec probe |
| `dpv` | Same with stack trace `at eval` | Display-pixel-vector probe; likely on `screen.X` or `devicePixelRatio` |

These are not random — the error message format is unique to V8's
"Cannot read properties of undefined" template. The probe pattern is:
1. Get some media/display object reference (which our engine returns undefined for)
2. Attempt to read the sentinel as a property
3. Crash with TypeError

**Fix direction**: identify the specific media/display API call that
returns undefined, return a meaningful object instead.

### Group B — `Function.prototype.toString requires that 'this' be a Function`

| Probe | Field | Notes |
|-------|-------|-------|
| `wse` (Web Security?) | TypeError on toString with non-Function this | Real Chrome's toString might be more permissive in some cross-realm context |
| `bfe` (Browser Frame Environment) | Same error in `win`/`winf` fields; empty in `ifr`/`ifrf` | Window-side toString crashes; iframe-side returns empty |
| `esce` (Eval Source Capture / Eval Cross Edge?) | Same error in `ce` and `pg` fields | Eval-related toString probe |

Our `Function.prototype.toString` throws on non-Function `this`. Real
Chrome's behavior: it depends on whether the toString is the actual
`Function.prototype.toString` or a property re-installed on the
prototype.

**Fix direction**: investigate whether real Chrome 148's
Function.prototype.toString accepts ALL `this` values or only Function
instances. May need to special-case the implementation to:
- For Function instances: return the [native code] string
- For non-Function: return a generic string representation OR throw
  a DIFFERENT error (RangeError? matching Chrome's actual behavior)

### Group C — `Class extends value <X> is not a constructor or null`

| Probe | Field | Value |
|-------|-------|-------|
| `fsc` (Function Source Capture) | `Class extends value function toString() { [native code] } is not a constructor or null` | Kasada does `class C extends Function.prototype.toString {}` |
| `npc` (Navigator Plugins / Prototype Class) | `Class extends value #<C> is not a constructor or null` | Real Chrome shows `#<C>` for unnamed classes; ours shows different |

Our error MATCHES the expected error format for fsc — that's fine.
The npc field shows `#<C>` which is Chrome's *unnamed class* repr.
If our V8 shows the class differently, this is a divergence.

**Fix direction**: investigate V8's class-toString output for unnamed
classes. May require ensuring our patch path doesn't intercept
class-source rendering.

### Group D — `Failed to construct 'X': Illegal constructor`

| Probe | Field | Value |
|-------|-------|-------|
| `nppm` (Navigator Permission?) | `TypeError: Failed to construct 'structuredClone': Illegal constructor` | structuredClone is a FUNCTION, not a constructor — should NOT throw this error |

Real Chrome: `new structuredClone()` throws `TypeError: structuredClone is not a constructor`. Our engine throws `Failed to construct 'X': Illegal constructor` — which is the error shape for WebIDL DOM interfaces.

**Fix direction**: structuredClone must error with the standard "not a constructor" message, not the "Illegal constructor" template. Likely an interfaces_bootstrap.js stub list issue.

### Group E — Screen dimensions return `"n/a"`

```json
"spd": {
  "r": "{\"availWidth\":\"n/a\",\"availHeight\":\"n/a\",\"width\":\"n/a\",\"height\":\"n/a\",\"innerWidth\":\"n/a\",\"innerHeight\":\"n/a\",\"outerWidth\":\"n/a\",\"outerHeight\":\"n/a\"}",
  "t": 1,
  "b": 1
}
```

All 8 dimensions return "n/a". Real Chrome returns numbers.

**Fix direction**: likely `iframe.contentWindow.screen.X` access path
is broken. Our iframe Proxy might not expose `screen` at all. Verify
by writing a test that reads `iframe.contentWindow.screen.width` and
seeing what comes back.

### Group F — Plugin source leaks

```json
"npf": {
  "r": "{\"npr1\":\"() =%3E {}\",\"npr2\":\"function refresh() { [native code] }\",\"npn1\":\"function namedItem(n) {\\n        const len = _pluginsLen();\\n        for (let i = 0; i %3C len; i++) if (_allPlugins[i].name === n) return _allPlugins[i];\\n        return null;\\n    }\",\"npn2\":\"function namedItem() { [native code] }\"}"
}
```

`npr1`: `() => {}` — arrow function source (our stub for some plugin method, leaking source)
`npn1`: full multi-line source for our `namedItem` implementation — **clear engine source leak!**

The fact that `npn2` returns `function namedItem() { [native code] }` shows that ONE of our namedItem implementations is masked and one isn't. The unmasked one is leaking pristine source.

**Fix direction**: find the unmasked namedItem and apply `_maskFunction`. Highest-confidence individual fix in this list — the source leak alone is a definitive headless tell.

### Group G — `MediaSource codec verdicts say `v:false`

For mp4/m4a/acc, Kasada gets `v: false`. Real Chrome returns true for video/mp4 (codec h.264 native support). Our `MediaSource.isTypeSupported` returns false.

Our `_supportedTypes` Set in window_bootstrap.js:4979-4995 DOES include `video/mp4`. So either:
1. `MediaSource.isTypeSupported` doesn't consult `_supportedTypes`
2. Kasada is calling something else (e.g. `MediaRecorder.isTypeSupported`)

**Fix direction**: trace which API Kasada calls and verify our implementation returns the same answer real Chrome does.

## Ranked patch list (effort / impact)

| # | Patch | Effort | Probability of helping |
|---|-------|--------|------------------------|
| 1 | Mask `namedItem` source leak (Group F) | 5 LOC | HIGH — pristine source is a kill signal |
| 2 | Fix MediaSource.isTypeSupported to match `_supportedTypes` (Group G) | 10 LOC | HIGH — `v:false` for h.264 is a clear divergence |
| 3 | structuredClone error message (Group D) | 5 LOC | MEDIUM |
| 4 | Screen dimensions in iframe contentWindow (Group E) | 30 LOC | MEDIUM |
| 5 | toString accepts non-Function this where Chrome does (Group B) | 30 LOC | LOW — would require V8 internals understanding |
| 6 | Identify the canPlayType/MediaSource API returning undefined (Group A) | unknown | UNKNOWN — root of the sentinel-loss chain |

Groups 1, 2, and 3 are pure surface bugs with small fixes.
Group 4 likely involves iframe Proxy enhancement.
Groups 5 and 6 require deeper investigation.

## Diff against current code

These are inferences from the captured blob. Actual implementation
state lives in:
- `crates/js_runtime/src/js/window_bootstrap.js:4976-5025` — canPlayType
- `crates/js_runtime/src/js/window_bootstrap.js` — Plugin / namedItem
- `crates/js_runtime/src/js/interfaces_bootstrap.js:38-48` — structuredClone stub list
- `crates/js_runtime/src/js/dom_bootstrap.js::_getIframeWindow` — iframe Proxy

## Open questions for the research agents

The 3 agents currently running can confirm or refine:
1. Is there a known-good Kasada sensor-data POST flow that successfully flips a Kasada-protected session L3? (agent 1)
2. What's the exact MediaSource.isTypeSupported behavior for video/mp4 in Chrome 148? (agent 3)
3. Does Chrome's Function.prototype.toString throw on non-Function `this`, or accept it via some path? (agent 3)
