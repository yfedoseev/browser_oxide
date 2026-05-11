# W4a-deeper — Kasada TypeError stack capture (canadagoose 2026-05-11)

Test: `kasada_typeerror_stack_capture` (chrome_compat.rs:5289)
Engine HEAD: 4c6d364 (R6 close, 116/126).
Capture: 2 unjzomuy stacks in 1 navigate cycle (~11.6 s).

## Captured probes

Both TypeErrors had the same shape:

```
msg: Cannot read properties of undefined (reading 'unjzomuybtbyyhwwkdpkxomylnab')

Stack #0:
  [0] fn=eval  line=3 col=66     isEval=true  (Kasada VM frame)
  [1] fn=U     line=5 col=116    isEval=false (ips.js dispatcher)
  [2] fn=eval  line=3 col=254    isEval=true
  [3] fn=eval  line=3 col=52     isEval=true
  [4] fn=U     line=5 col=116    (dispatcher again — recursion)
  [5] fn=eval  line=3 col=254
  [6] fn=eval  line=3 col=52
  [7] fn=U     line=5 col=116

Stack #1: identical pattern but col=59 / col=306 in places.
```

## What this proves

1. **The TypeError-stack hook works** — the `Error.prepareStackTrace`
   override fires on every TypeError matching the 28-char unjzomuy
   regex and records `callsite.getFunctionName/getFileName/
   getLineNumber/getColumnNumber/getEvalOrigin`.
2. **The probe is a property access on `undefined`** —
   `obj.unjzomuybtbyyhwwkdpkxomylnab` where `obj` was `undefined`.
3. **The probe runs inside a Kasada-eval'd VM** — all frames except
   `U` are `isEval=true`. `U` is the ips.js dispatcher at line=5 col=116
   that calls into VM with `eval(s)`.

## What this does NOT tell us

V8's TypeError message format gives the property name but **not the
receiver**. We see `unjzomuybtbyyhwwkdpkxomylnab` was the property,
but we don't know what `obj` was meant to be (e.g.
`navigator.unjzomuyb...` vs `window.foo.bar.unjzomuyb...`).

The previous "eval-source approach" (try to decompile the VM script)
was tried in HANDOFF_PART4 and didn't yield the receiver either —
the VM is heavily obfuscated and the receiver is built dynamically
from constant tables.

## Recommended next step — Proxy-based receiver capture

Wrap `globalThis` (and key prototypes: `Navigator`, `Window`,
`Document`, `HTMLElement`) with a `Proxy` whose `get` trap records
**every property access path containing a 28-char `[a-z]{28}` token**:

```js
const _kasadaAccessLog = [];
const _trackProxy = (target, basePath) => new Proxy(target, {
  get(obj, prop, recv) {
    const key = String(prop);
    if (/^[a-z]{28}$/.test(key)) {
      _kasadaAccessLog.push({
        path: basePath + '.' + key,
        receiver_was_undefined: obj === undefined,
        receiver_keys: obj ? Object.keys(obj).slice(0, 20) : null,
      });
    }
    return Reflect.get(obj, prop, recv);
  }
});
globalThis.__kasadaAccessLog = _kasadaAccessLog;
```

Install at runtime startup (before any antibot script). After the
canadagoose navigation, dump `__kasadaAccessLog` — every entry shows
the **path** Kasada tried to read, and the **keys** of the actual
receiver (so we can identify what global it expected).

Then for each missing path, decide:
- Stub the missing global (if it's a real Chrome API we don't ship)
- Mock the property if it's a fingerprint check (canvas, audio, etc.)
- Match the value to Chrome 147's actual return

## Estimate

- Proxy wrapper: ~50 LOC in `window_bootstrap.js`
- Identify the 5 distinct unjzomuy targets: 1 capture run
- Stub each: 5–15 LOC apiece, depending on what they are
- Total: 4–8 hours of focused work

If they're real APIs (e.g. obscure Web Platform features we missed
during the W6a sweep), we likely flip canadagoose + hyatt + realtor
because they share Kasada's deployment-wide probe set.

## Why this is +3 sites high-leverage

All three Kasada-strict sites (canadagoose, hyatt, realtor) use the
same `cdndex.io` reporting endpoint with the same wrapper key
(`omgtopkek`, already cracked) and the same `ips.js` dispatcher
shape. If the unjzomuy probe set is deployment-wide (likely — the
identifier is constant across hosts), one stub set covers all three.

## Files referenced

- `crates/browser/tests/chrome_compat.rs:5289` — the test that
  produced this output
- `/tmp/kasada_typeerror.log` — full run log (2026-05-11 00:18 PST)
- `crates/js_runtime/src/js/dom_bootstrap.js` — where the Proxy
  wrapper would go

## Update 2026-05-11 00:34 — eval/Function-source capture added

Extended the test to hook both `globalThis.eval` and `globalThis.Function`
(via Proxy on construct + apply). After the run we dump every source >
200 chars or containing `[a-z]{28}`.

### Stats from a canadagoose.com run
```
evalCalls:  2
fnCalls:  162
recorded: 164 (after filter: 9 sources >200 chars / hasObf)
```

Kasada uses `new Function(body)()` ~162 times as indirect compilation.

### Captured VM handlers — none contain `unjzomuyb` literally

All 9 are 1-line Function bodies that look like the VM's interpreter
loop. Sample shapes (each is a distinct opcode handler):

```js
// #0 — STORE indexed: e(n)[e(n)] = e(n) (most likely the probe site)
return function(n,e,a,v,i,r){
  e(n)[e(n)]=e(n);
  var o=e(n), l=e(n), _=e(n), h=r[4];
  if(l[h] && l[h].C===l) {  // function table dispatch
    n.X=[l[h].H, {Q:o,K:l,X:n.X,v:[],V:l[h].V, G:...}, ...];
  } else {
    n.X[2] = l.apply(o,_);  // invoke as method
  }
}

// #2/#3 — function-table register (creates a callable that
//          dispatches into the VM)

// #4 — variable lookup with scope walk:
return function(n,e,a,v,i,r){
  for(var i=0;i<2;i++) {
    var r=e(n), o=e(n), l=!1;
    for(var _=v(n); _; _=_.V) if(r in _.v) { _.v[r]=o; l=!0; break; }
    if(!l) throw 'ball';
    ...
  }
}

// #5 — XTEA cipher block (len=1170, kind=Function-apply):
function n(i,l){
  var p=l[0], u=l[1], c=0;
  var d=2654435769;   // XXTEA magic constant (golden ratio)
  for(var f=0;f<32;f+=1) {
    p = p + ((u<<4^u>>5)+u^c+i[c&3]) | 0;
    c = c + d | 0;
    u = u + ((p<<4^p>>5)+p^c+i[c>>11&3]) | 0;
  }
  return [p, u];
}
```

### The real probe is hidden in the const table

The `unjzomuybtbyyhwwkdpkxomylnab` identifier never appears as a string
literal in any captured eval source. It must arrive via:

1. The XXTEA-decrypted constant table (#5 is the cipher)
2. As an opcode operand inside the VM bytecode

So **the probe is**: `e(n)[e(n)]` where the inner `e(n)` returns the
decrypted string `'unjzomuyb...'` from the const table, and the outer
`e(n)` returns the receiver (which is `undefined` for us).

### Why the receiver is undefined

The receiver came from an earlier VM opcode — likely a `globalThis`
walk or a property read on some real Chrome API. Our engine missed
some setup step that initializes a Kasada-internal data structure.
The 28-char "unjzomuyb..." is **NOT a Chrome API name** — it's
Kasada's own obfuscated key for its internal state.

This rules out the "stub a missing browser API" approach. It points
at: Kasada's bootstrap sequence assumes some pre-call returned a
non-undefined value, and ours returned undefined.

### Next steps — fresh workstream needed

This requires walking the VM execution step-by-step. Approaches:

1. **Instrument the VM at the opcode-level**: replace `new Function`
   for VM handlers with wrappers that log every register read/write.
   The handlers have a stable shape so the wrapper can be parametric.
2. **Capture the XTEA key**: cipher block #5 takes its key from the
   first argument array. Log the key on first call, then we can
   decrypt the constant table off-line and reconstruct the real
   property names.
3. **Diff our VM state vs Chrome 147's** at the same point in the
   bootstrap. Run a parallel Chrome session via Playwright MCP with
   the same Kasada init, hook eval there too, and diff the captured
   state.

Estimate: **3–5 days** of focused VM reverse engineering.

This is too much for an autonomous loop iteration. Stopping W4a-deeper
here with the VM internals mapped — pick up next session with the
opcode-level instrumentation plan above.

## Update 2026-05-11 00:42 — L4 prototype-getter probe

Installed `Object.prototype.unjzomuybtbyyhwwkdpkxomylnab` as a getter
before any antibot script runs. Any read on a real (non-undefined)
receiver where the property doesn't exist as own walks the prototype
chain and hits this getter, which logs `Object.prototype.toString.call(this)`
+ constructor name + own keys + a stack snapshot.

### Result on canadagoose.com

- **100 probe-getter hits** (cap, real number likely higher)
- **All 100 receivers are `[object Function]`** — Kasada writes/reads
  this obfuscated property on its own internal Function objects (the
  VM dispatchers + handler functions captured earlier)
- **159 Function() calls** (down from 162 without the getter — the
  3-call drop is presumably my init-script overhead, not Kasada
  detecting the tampering). Full VM bootstrap completes.
- **The 2 TypeError captures still fire** — same 2 unjzomuy errors
  with identical stack shape as before

### Critical implication

The probe pattern is **NOT** "stub a missing API". Kasada writes the
unjzomuy property on its own Functions (legal in real Chrome too) and
reads it back. Real Chrome doesn't reject this. **Our engine ALSO
accepts the writes** (verified — 100 successful read-throughs in our
V8). So the engine does NOT block the *normal* probes.

The 2 *failed* probes — where the receiver was `undefined` — happen
on a DIFFERENT code path. Something upstream of the property read
returned undefined where Chrome would have returned a real Function/
Object. Possibilities:

1. A WeakMap.get() returned undefined (our WeakMap stub returns
   undefined for unseeded keys; real Chrome's may have been seeded
   by an earlier Kasada op)
2. A Map.get() on a Kasada-internal Map miss
3. A property read on a real Web API where our value is `null`
   instead of a Function (e.g., `Notification.requestPermission` is
   a function in real Chrome but null/undefined in our engine)
4. A getter on a DOM property returning undefined when Chrome would
   return a Function (e.g., `document.adoptedStyleSheets`, certain
   draft APIs)

### Stack format for the 2 failed probes (unchanged from L1 capture)
```
msg: Cannot read properties of undefined (reading 'unjzomuybtbyyhwwkdpkxomylnab')
[0] eval line=3 col=66    isEval=true   (Kasada VM frame)
[1] U    ips.js line=5 col=116
[2-7] alternating eval and U frames, deeper in the VM
```

`U` at line=5 col=116 of ips.js is the VM dispatcher (calls into the
captured handlers). Frame [0] at eval:3:66 is the opcode body —
specifically the `e(n)[e(n)]=e(n)` pattern in handler #0 we captured.

### Next-step plan refined

Don't reverse the entire VM. Instead:

1. **Hook every WeakMap/Map .get/.set** and log the keys + values.
   Compare with a Playwright Chrome trace. If Kasada uses a
   per-session WeakMap that we initialize differently, we'll see it.
2. **Hook `Notification.requestPermission`** and audit other commonly-
   probed APIs to confirm they're Functions (not null/undefined).
3. **Capture the FULL ips.js source line 5 around col=116** to see
   the *dispatcher* signature — that tells us what `U` was called
   with at the failure point. We have ips.js URL; can fetch it from
   the captured headers.

The fastest path forward is now (3) — fetch ips.js, look at line 5
around col 116, identify what variable was undefined.

## Update 2026-05-11 00:46 — full 50-opcode VM table captured

Removed the source-filter cap to capture EVERY Function call. First 50
are the VM opcode table. Decoded:

| # | Opcode (inferred) | Body |
|---|---|---|
|  0 | STORE-X0           | `n.X[0]=e(n)` |
|  1 | DECLARE-UNDEFINED  | `var i=e(n),r=v(n);r.v[i]=void 0` |
|  2 | LOAD-I3            | `a(n,i[3])` |
|  3 | STORE-SCOPE-LOOKUP | `for(var i=e(n),r=e(n),o=v(n);o;o=o.V)if(i in o.v){...}` |
|  4 | LOAD-SCOPE-LOOKUP  | `for(var i=e(n),r=v(n);r;r=r.V)if(i in r.v){a(n,r.v[...])` |
|  5 | DIV                | `a(n,e(n)/e(n))` |
|  6 | **LOAD-INDEXED**   | `a(n,e(n)[e(n)])`  ← suspect for TypeError |
|  7 | NEW-ARRAY          | `a(n,[])` |
|  8 | CALL               | `var o=e(n),l=e(n),_=e(n),h=r[4];if(l[h]&&l[h].C===l){...}` |
|  9 | LOAD-I0-INDEXED    | `a(n,i[0][e(n)])` |
| 10 | NEW-ARRAY-N        | `a(n,new Array(e(n)))` |
| 11 | STORE-INDEX-ALLOC  | `a(n,e(n)[e(n)]),a(n,new Array(e(n)))` |
| 12 | **STORE-INDEXED**  | `e(n)[e(n)]=e(n)`  ← suspect for TypeError (line 3 col 66) |
| 13 | SUB                | `a(n,e(n)-e(n))` |
| 14 | ADD-R5             | `var o=r[5];a(n,o(n)+e(n))` |
| 15 | SUB-R5             | `var o=r[5];a(n,o(n)-e(n))` |
| 16 | ADD                | `a(n,e(n)+e(n))` |
| 17 | SUB                | `a(n,e(n)-e(n))` |
| 18 | LOAD               | `a(n,e(n))` |
| 19 | STORE-INDEX-CALL   | `e(n)[e(n)]=e(n);var o=e(n),l=e(n),_=e(n),h=r[4]...` |
| 20 | LOAD-SCOPE-LOOKUP-2| (variant of #4) |
| 21–24 | DECLARE-N (n=3,4,2,5) | `for(var i=v(n),r=0;r<N;r++){var o=e(n);i.v[o]=void 0}` |
| 25 | DEFINE-FN          | `var o,l=e(n),_=e(n),h=e(n),u=v(n),t=r[2],b=r[3],s=r[4]...` |
| 26 | STORE-SELF-K       | `var i=e(n),r=v(n),o=r.K;r.v[i]=o` |
| 27 | STRICT-EQ          | `a(n,e(n)===e(n))` |
| 28 | NEW-OBJECT         | `a(n,{})` |
| 29 | TERNARY-STORE-X0   | `e(n)?e(n):n.X[0]=e(n)` |
| 30 | TYPEOF             | `a(n,typeof e(n))` |
| 31 | STRICT-NEQ-R5      | `var o=r[5];a(n,o(n)!==e(n))` |
| 32 | LOAD-I0-DOUBLE-IDX | `a(n,i[0][e(n)]),a(n,e(n)[e(n)])` |
| 33 | LOAD-SCOPE-LOOKUP-3| variant |
| 34 | STORE-IDX-PLUS-ALLOC | `a(n,new Array(e(n))),e(n)[e(n)]=e(n)` |
| 35 | LOAD-SCOPE-LOOKUP-4| variant with throw |
| 36 | STORE-LOAD-COMBINED| `e(n)[e(n)]=e(n);for(var i=e(n),r=v(n);r;r=r.V)...` |
| 37 | DOUBLE-LOAD-INDEXED| `a(n,e(n)[e(n)]),a(n,e(n)[e(n)])` |
| 38 | DEFINE-FN-2        | (variant of #25) |
| 39 | NESTED-LOOKUP      | `n:for(var i=0;i<2;i++){for(var r=e(n),o=v(n);o;o=o.V)...}` |
| 40 | CALL-3             | `var v=e(n),i=e(n),r=e(n);a(n,v(i,r))` |
| 41 | STORE-SCOPE        | `var i=e(n),r=e(n),o=v(n);o.v[i]=r` |
| 42 | DECLARE-2-WITH-VAL | `for(var i=v(n),r=0;r<2;r++){var o=e(n);i.v[o]=r===0...}` |
| 43 | FOR-IN             | `var v=e(n),i=[];for(var r in v)i.push(r);a(n,i)` |
| 44 | LOOSE-EQ           | `var v=e(n),i=e(n);a(n,v==i)` |
| 45 | TERNARY-STORE      | `e(n)?n.X[0]=e(n):e(n)` |
| 46 | NESTED-LOOKUP-3    | (variant of #39) |
| 47 | DOUBLE-STORE-IDX   | `e(n)[e(n)]=e(n),e(n)[e(n)]=e(n)` |
| 48 | CALL-4             | `var v=e(n),i=e(n),r=e(n),o=e(n);a(n,v(i,r,o))` |
| 49 | ADD                | `a(n,e(n)+e(n))` |

### Convention

- `n` = VM state (instruction pointer + stack frame)
- `e(n)` = read next operand (constant or register reference)
- `a(n, val)` = push to accumulator
- `v(n)` = read current scope chain element (linked list via `.V`)
- `i` = instruction table (per-handler immediates)
- `r` = global registers (`r[4]` is the function-table marker; `r[5]`
  is the load-prev-accumulator op; `r[2]` and `r[3]` are
  closure/scope helpers in DEFINE-FN)

### The failing opcode is #12 STORE-INDEXED

Stack frame [0] at `eval line=3 col=66 isEval=true` matches opcode
#12's body `e(n)[e(n)]=e(n)` — col 66 inside the synthetic
`new Function` script (header is 2 lines, so body line 1 col 66 maps
to `e(n)[e(n)]=e(n)` middle). The FIRST `e(n)` returned the receiver,
which was `undefined` for us. Inferred semantics:

```
STORE-INDEXED: take 3 operands from instruction stream:
  receiver = e(n)
  key      = e(n)  // = 'unjzomuybtbyyhwwkdpkxomylnab' from const table
  value    = e(n)
  receiver[key] = value;
```

For the 100+ probe-getter hits (receiver = a Function), this is
caching state on the Function. For the 2 failures, the receiver
resolved to `undefined` — likely from an UPSTREAM opcode (#3, #4,
#20, #33, #35 — scope-lookup variants) that didn't find what
Kasada expected in our globalThis scope chain.

### Scope-lookup opcodes worth instrumenting next

Opcodes #3, #4, #20, #33, #35 share the pattern:
```
for (var o = v(n); o; o = o.V) {
  if (key in o.v) {
    // hit — use o.v[key]
  }
}
```

`v(n)` returns the current scope head; `.V` walks the parent chain.
If the chain DOESN'T contain `key`, the loop exits without setting a
result — which leaves the accumulator at undefined.

### The CRITICAL static-analysis target

ips.js line 5 col=116 (and col=169, col=175) is **dispatcher `U`**.
From the stack traces: U(:5:116) calls `eval(:3:66)` — i.e. U calls
into the VM body. col=116 is the call site where U passes (n, e, a,
v, i, r) to the handler.

We could capture ips.js source by routing through `<script src>`
fetch in Rust (page.rs:2126 dumps to oxide_dump/ but evidently this
path isn't taken for canadagoose in the test) or by a one-line edit
to dump every .get_follow_with_headers response that contains
"omgtopkek" or matches the ips.js URL pattern.

### Investigation paused — multi-day work remaining

Confirmed achievements this iteration:
1. Full 50-opcode VM map captured
2. Failing opcode identified (#12 STORE-INDEXED)
3. Probe receiver in 100+ success cases identified ([object Function])
4. Scope-lookup opcodes (#3/#4/#20/#33/#35) identified as the
   upstream candidates for the missing-receiver case

Remaining work (~3 days):
1. Dump ips.js source via Rust net hook
2. Static-analyze U dispatcher at line 5 col 116
3. Hook the scope-lookup opcodes (#3/#4) to log every `key` and
   whether the lookup succeeded — find the one that returns
   undefined right before the failed STORE-INDEXED
4. Identify the missing globalThis property/value
5. Add it to window_bootstrap.js
6. Verify canadagoose flips L3

## Update 2026-05-11 00:54 — **DECRYPTED ERROR REPORTS — THE HIT LIST**

Re-ran the existing `kasada_error_blob_capture` test (which writes 8
encrypted POST bodies) + decrypted via existing
`docs/kasada_ips_analysis/scratch/decrypt_report.py` (XOR
`omgtopkek`).

**The reports tell us which specific probes flag us as a bot.**

### The bot-detection signal: `e: 1` fields

Each probe in the report has a structure `{r: result, t: type, b: browser-match, e: error-flag}`.
The **`e: 1` fields are what Kasada flags as bot-indicators**.

| Field | hits (of 8 blobs) | Error result |
|-------|------:|---|
| **ao** | 3 | `TypeError: Invalid attempt to spread non-iterable instance` |
| **cbf** | 3 | `TypeError: Cannot read properties of undefined (reading 'toString')` |
| **crs** | 3 | `"no error thrown"` (Kasada *expected* an error — we didn't throw) |
| **csc** | 3 | `TypeError: Cannot read properties of undefined (reading 'unjzomuybtbyyhwwkdpkxomylnab')` |
| **kl** | 3 | unjz error |
| **nhp** | 3 | `"unexpected"` (output pattern didn't match Chrome reference) |
| **mrs** | 1 | `TypeError: Cannot read properties of undefined (reading 'isTypeSupported')` |
| **bot1225** | 3 | unjz error (final bot verdict) |

The first decrypted blob has **117 probe fields total** — we score
**6 bot-flags out of ~117 probes**. That's the gap between "rendered"
and "blocked" on canadagoose.

### Key insights from the error messages

1. **`ao` — non-iterable spread**: Kasada does `[...x]` where `x` is
   non-iterable in our engine but iterable in Chrome. Candidates:
   `navigator.plugins`, `navigator.userAgentData.brands`,
   `MediaSource.activeSourceBuffers`, `document.fonts`,
   `HTMLCollection`, `RTCRtpReceiver.getCapabilities`. Need to make
   the probed object implement `Symbol.iterator`.

2. **`crs` — "no error thrown"**: Kasada expected our code to throw.
   It didn't. Probably attempts to call a non-callable (`new` on a
   getter, etc.) and we silently succeed where Chrome's "Illegal
   constructor" or "is not a function" should fire. Audit any
   Web-API stub that's missing constructor-call validation.

3. **`mrs` — `.isTypeSupported` undefined**: A media-related API
   accessor returns undefined. We have `MediaSource.isTypeSupported`
   and `MediaRecorder.isTypeSupported`, so it's a third path
   (possibly Worker-realm `MediaSource`, or
   `OffscreenCanvas`/`WebCodecs`'s isConfigSupported via a different
   path).

4. **`nhp` — "unexpected"**: A hash/checksum probe of navigator
   hardware concurrency or similar didn't match Chrome's reference
   set. Likely `navigator.hardwareConcurrency` value or fingerprint
   pattern mismatch.

5. **`cbf` — undefined.toString**: Some object is undefined in our
   engine where Chrome has a real value with `.toString()`. Field
   neighbors: `uh, ss, cbf, eck, npe` — could be a CodecBlob,
   CryptoBlobFormat, or CallableBitField.

6. **`csc`, `kl` — unjz on undefined**: Two APIs return undefined.
   Field clusters give hints: `snp, dpv, csc, spd, gp` (CSS / display)
   and `jdo, tn, kl, nps, uad` (UA data).

### Field-cluster decoded (positional probability)

Probe field IDs are grouped by category. The byte positions in the
JSON suggest:

- `nppm npf crs npc ecp` — **NewPrototype** family (probe class
  constructor behavior)
- `snp dpv csc spd gp` — **Screen/display/properties** family
- `spc kbk ao ddt2 cpl` — **Spread/iter** family
- `jdo tn kl nps uad` — **Navigator/UA** family
- `nps uad nhp eem eem2` — same family, fingerprint hash
- `uh ss cbf eck npe` — **Storage/Crypto** family
- `cduf stc mrs ccf bfs` — **Media/Recorder** family

### Status: roadmap delivered

This iteration **identified the exact 6 bot-flagged probes** plus
the field clusters they live in. Next session's path is now:

1. Read the cluster hints; for each `e=1` field, write a small focused
   probe in our engine that mimics what Kasada is testing
2. Compare against Chrome — fix the divergence
3. Re-run kasada_error_blob_capture; verify `e=1` count drops
4. Repeat until `bot1225` no longer fires

Estimate now down from "3 days of VM RE" to **6–8 fixes of ~30 min
each** (3–4 hours). Each `e=1` field becomes a single targeted
patch in `window_bootstrap.js` or `dom_bootstrap.js`.

### Files

- `kasada_error_*.b64` (8 raw encrypted blobs in project root)
- `docs/kasada_ips_analysis/scratch/decrypted_blob_*.json` (decoded)
- `docs/kasada_ips_analysis/scratch/decrypt_report.py` (XOR decoder)
- `kasada_function_bodies.js` (the captured 50 opcode handlers)

### File at the heart of it

ips.js at `/149e9513-01fa-4fb0-aad4-566afd725d1b/2d206a39-8ed7-437e-a3be-862e0f06eea3/ips.js`
(per-canadagoose path; rotates each session).

Line 5 col 116 of that file is the dispatcher's argument list /
call site. Static analysis of those ~200 chars should reveal
what global Kasada was reading from.
