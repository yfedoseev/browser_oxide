# Kasada `unjzomuy` Sentinel — Still Open After W1.1 (2026-05-15)

## Status

The Kasada universal block (canadagoose / hyatt / realtor) is **NOT
cracked by W1.1 `_buildRemoteRealm` memoization**. Memoization is
confirmed live (`dom_bootstrap.js:2205-2208` — module-level
`_cachedRemoteRealm`), and the post-W1 4-profile sweep ran with it in
place, yet the captured VM trace still throws at the same opcode index.

## The captured throw (kasada_vm_trace.json, re-read 2026-05-15)

```
handler_count: 307
first_throw_at:  2141   (UNCHANGED from the 2026-05-14 baseline)
msg: "Cannot read properties of undefined (reading
      'unjzomuybtbyyhwwkdpkxomylnab')"
stack:
  at eval (<anonymous>:3:82)
  at h (<init_script_0>:51:34)        <- TEST instrumentation wrapper,
                                          NOT production (see §9.3 of
                                          09_KASADA_DEEP). In prod with
                                          the W2.7 scrub this is
                                          <anonymous>.
  at U (/149e9513-.../ips.js…:5:116)
  at eval (<anonymous>:3:254)
  at eval (<anonymous>:3:52)
```

`first_throw_at` is still **2141** — none of the W1/W2/audit JS-surface
fixes this session shifted the index. That means the sentinel loss is
upstream of every surface we have been patching.

## VM opcode bodies that matter

From `throwing_handler_bodies`:

| Handler | Body | Role |
|---|---|---|
| 2  | `n.V[0]=e(n)` | store to register |
| 8  | `for(var i=e(n),r=e(n),o=v(n);o;o=o.Q)if(i in o.h){o.h[i]=r;return}throw 'ball'` | **scope-chain WRITE** |
| 10 | `for(var i=e(n),r=v(n);r;r=r.Q)if(i in r.h){a(n,r.h[i]);return}throw 'ball'` | **scope-chain READ** |
| 14 | `a(n,e(n)[e(n)])` | member access |

`pre_throw_window` shows repeated `h=…  a=6  a0="o{V,e}"  r="undefined"`
just before the throw — the VM is walking a scope chain (`r=r.Q`) for
the sentinel key and every frame's `.h` map returns `undefined` for it.

## Interpretation

Kasada plants the property `unjzomuybtbyyhwwkdpkxomylnab` on **some
host object** during one VM opcode, then later evals
`<thatObject>.unjzomuybtbyyhwwkdpkxomylnab` to verify the environment is
the genuine one it tagged. Our engine returns `undefined` for
`<thatObject>` (or returns a *different* object instance than the one
tagged), so the sentinel is gone → `TypeError`.

W1.1 ruled out the *iframe remote-realm rebuild* as the lost-identity
source (memoized, still throws). The remaining candidates, in priority
order:

1. **A cross-realm constructor / prototype that we rebuild per access**
   other than the iframe realm — e.g. a `Function`/`Object` mirror,
   a getter that returns a fresh object each call, or a Proxy whose
   `get` trap synthesizes a new value.
2. **A global the VM tags that we expose as a fresh object per read**
   (the `pre_throw_window` `a0="o{V,e}"` object — `V` is the VM
   register file, `e` is the opcode-decode closure; the tag target is
   reached *through* one of these).
3. **`eval` indirection identity** — the throw is inside
   `eval(<anonymous>:3:…)`. If our indirect `eval` runs in a different
   scope/realm than direct `eval`, the sentinel set in one is invisible
   to the other.

## Why this is not a one-session fix

Per `09_KASADA_DEEP_2026_05_14.md` and the prior session handoff, the
realistic resolutions are:

- **VM emulation** (W4.5): decode the 307 handler bodies, reimplement
  the dispatcher in Rust, sign via captured TEA-CBC key. Multi-day.
- **Pinpoint-leak capture**: instrument the exact opcode at idx ~2134
  (the WRITE that plants the sentinel) and at 2141 (the READ that
  loses it), dump the *identity* of the object on each side, diff.
  This is the cheapest next experiment (hours, not days) and is the
  recommended next step — it converts "some object loses identity"
  into "object X at site Y loses identity", which is then a targeted
  JS fix.

## REFINED FINDING (2026-05-15, live canadagoose trace re-run)

The eval-source interceptor added this session captured **0** entries:
`sentinel_evals: []`. The sentinel name is **not an eval literal** — the
Kasada VM reconstructs `unjzomuybtbyyhwwkdpkxomylnab` from its XOR'd
bytecode string table, so source-grepping the eval text can never see
it. That approach is a dead end; do not pursue it.

The real signal is in the **throwing handler bodies** (handlers 18 & 26,
the closure-creation opcodes):

```js
h18: function(n,e,a,v,i,r){
  var o=e(n), l=e(n), _=e(n), h=r[4];          // h = the sentinel string
  if (l[h] && l[h].I === l) {                   // <-- THROWS: l is undefined
    n.V = [ l[h].E,                             // entrypoint
            { w:o, S:l, V:n.V, h:[], Q:l[h].Q,  // new scope frame
              k: n.K?.get(l[h].k) },
            void 0,
            function(){return arguments}.apply(void 0,_) ];
    ...
h26: function(n,e,a,v,i,r){
  e(n)[e(n)]=e(n);                              // member store
  var o=e(n), l=e(n), _=e(n), h=r[4];
  if (l[h] && l[h].I === l) { ...same shape... }
```

`r[4]` is the sentinel string. `l = e(n)` is the **callee function**
the VM is about to invoke. `l[sentinel]` is Kasada's per-function
*closure-identity record*: `{ I: l (self-ref), E: entrypoint,
Q: scope-chain, k: key }`. The opcode reads it to build the call
frame. The throw `Cannot read properties of undefined (reading
'unjzomuy…')` means **`l` itself is `undefined`** — an upstream opcode
that should have produced the VM function reference produced
`undefined` in our engine.

### Sharper hypothesis (actionable for next session)

The VM's *first* function must be tagged by Kasada's bootstrap:
`bootstrapFn[sentinel] = { I: bootstrapFn, E:…, Q:…, k:… }`. If
`bootstrapFn` is a **global we return as a fresh wrapper per access**
(a masked native fn, a getter-returned bound fn, a Proxy-trapped fn),
then Kasada tags instance A, the VM later re-fetches the global and
gets instance B, `B[sentinel]` is `undefined`, and the whole closure
chain collapses → `l` is `undefined` at the first call opcode.

W1.1 fixed the *iframe realm* case. The generalization: audit **every
global function reachable from Kasada's bootstrap that our engine
hands out non-identity-stable**. Prime suspects, in order:
1. `_maskFunction`-wrapped globals — does the wrapper re-create per
   read, or is it a stable own value? (It defines `toString`/name on
   the *original* fn, so identity should be stable — verify the fn
   isn't itself behind a getter.)
2. Any `Object.defineProperty(…, { get(){ return <fresh fn> } })` on
   a global Kasada touches (`eval`, `Function`, `setTimeout`,
   `Object`, `Array`, `Promise`, `Reflect`, DOM ctors).
3. Indirect-vs-direct `eval` returning different function identities.

### Concrete next experiment

Add a **global sentinel-property trap** to the trace init script
(before ips.js):

```js
let _SENT='unjzomuybtbyyhwwkdpkxomylnab', _tags=[];
Object.defineProperty(Object.prototype,_SENT,{
  configurable:true,
  set(v){ _tags.push({op:'set',
           recv:Object.prototype.toString.call(this),
           ctor:this&&this.constructor&&this.constructor.name});
          Object.defineProperty(this,_SENT,
            {value:v,writable:true,configurable:true}); },
  get(){ return undefined; }
});
globalThis.__sentinelTags=_tags;
```

Dump `__sentinelTags`. Every `set` names the object Kasada tags.
Cross-reference with the throw: the tagged object whose identity we
fail to return on re-fetch is the bug. This is a *property* trap (the
sentinel string IS known at runtime) and unlike the eval interceptor
it WILL fire, because Kasada assigns `obj[sentinel]=…` via normal
member-store, which goes through `Object.prototype`'s setter when the
object has no own `sentinel`.

## Two-root-cause model (important for scoping the fix)

`l = e(n)` decodes a VM operand (register/constant-pool index). `l`
being `undefined` has exactly two possible origins:

1. **Lost object identity** — Kasada tagged function F (instance A);
   the VM re-fetched F and our engine returned instance B; `B[sentinel]`
   is absent. Detectable by the sentinel-property trap (a `get` miss
   with `everTaggedId:-1` on a function). **If the trap shows this, the
   fix is targeted** (make that one global identity-stable, like W1.1
   did for the iframe realm).

2. **Cascading value divergence** — an *earlier* fingerprint probe
   (audio hash, WebGL readback, timing jitter, canvas, perf) returned a
   value our engine computes differently from Chrome; Kasada's VM
   branches on it, the register file diverges, and `l` ends up
   `undefined` with no single lost object. The trap shows **no** miss
   in this case. Known divergent inputs that could feed this: audio FP
   `140.05` vs Chrome `~124.04` (multi-day DynamicsCompressor parity),
   `performance.now` jitter, WebGL precision. **If the trap shows no
   miss, this is the regime** — and the only resolutions are full VM
   emulation (W4.5) or byte-perfect probe parity. Both multi-day.

The trap experiment is decisive precisely because it *discriminates
between these two regimes*, which determines whether canadagoose is a
targeted fix or a multi-day program. That is the single most valuable
diagnostic remaining and it is now scaffolded + running.

## DECISIVE RESULT — clean production probe (2026-05-15)

`kasada_sentinel_identity_clean` (Object.prototype sentinel trap, **NO
Function wrapper** — §9.3-safe, production-representative) against live
canadagoose:

```
tags: 80          (Kasada tagged 80 of its own VM closures)
miss: 80
missTaggedElsewhere: 0   (ZERO missed objects were ever tagged)
tagSample src:  "function(){var p=t();p.V[3]=arguments;for(var f=0;
                 f<arguments.length;f++)p.V[f+4…"   ← Kasada VM trampolines
missSample src: slice/concat/apply/floor/createElement/appendChild
                "[native code]"  +  "function anonymous(\n){return…"
```

**Every sentinel MISS is a legitimate native built-in** (`slice`,
`concat`, `apply`, `floor`, `createElement`, `appendChild`) plus
`new Function()` results. That is **correct, expected behavior**:
Kasada's call opcodes (18/26) do `if (l[sentinel] && l[sentinel].I===l)`
to decide *"is `l` one of my trampolined VM closures (tagged) or a real
native function (untagged) I should call directly?"*. Reading
`slice[sentinel] === undefined` and branching to "native call" is
exactly what the VM is supposed to do — and it happens identically in
real Chrome.

`missTaggedElsewhere: 0` + 80 healthy tags means **our engine does NOT
lose tagged-closure identity in production.** The earlier 60/60
`everTaggedId:-1` result was 100% the `kasada_vm_dispatcher_trace`
Function-wrapper artifact (the §9.3 caveat, now empirically confirmed).

### Regime determination: it is Regime 2 (cascading value divergence)

Per the two-root-cause model above, the clean probe shows **no
lost-identity miss** → **Regime 2**. The Kasada VM *executes correctly*
in our engine (80 closures created, tagged, dispatched). canadagoose
still returns **429** (captured: `reload response headers (429)` +
`ips.js` served) because the **fingerprint + behavioral signals the VM
collects score as bot** — not because of any JS crash, missing API, or
identity loss.

**This eliminates the "multi-day VM emulation" framing for
canadagoose/hyatt/realtor.** They do not need W4.5 VM reimplementation.
They need fingerprint-divergence closure. The single known concrete
divergence is the **AudioContext hash (`140.05` ours vs `~124.04`
Chrome)** — a DynamicsCompressor byte-parity gap. Other candidate
inputs: behavioral (sigma-lognormal is wired but Kasada scores 2nd-
derivative jerk distribution), `performance.now` jitter, WebGL
precision readback, and TLS/H2 (already byte-perfect per prior work).

### Revised next-session priority (much higher ROI than VM emulation)

1. **Audio FP byte-parity** — get the OfflineAudioContext hash to
   Chrome 147's exact value. Our `crates/canvas/src/audio.rs` already
   ports Blink's DynamicsCompressorKernel; the 140 vs 124 delta is a
   coefficient/rounding gap, not a missing feature. Closing this is
   the highest-probability single lever for all 3 Kasada sites and is
   *days, not weeks* — far cheaper than VM emulation.
2. Behavioral jerk-profile audit (Kasada scores the 2nd derivative of
   the sigma-lognormal path; verify our sampling preserves it).
3. Re-sweep canadagoose after each — the 429→200 flip is the verifier.

This is the most important finding of the 2026-05-15 session: the
Kasada universal block is a *fingerprint-parity* problem (tractable,
days) — NOT a *VM-emulation* problem (intractable, weeks). The prior
handoffs' "multi-day VM-emulation" framing is **retired**.

## What IS shipped (so this doc isn't misread as "nothing works")

W1.1–W1.10, W2.6/2.7/2.8, Akamai `_abck` parser + dynamic tenant +
v3 envelope + CounterTuple, PerimeterX iOS surface gating, DataDome
iframe self-loops + sigma-lognormal behavior, the interfaces_bootstrap
stub-preemption fix, and the V8 stack-name scrub are all in `main`.
chrome_compat is 415/415 green. The Kasada VM sentinel is the single
identified universal blocker that remains and it is explicitly a
multi-day VM-emulation problem, not a missed quick win.
