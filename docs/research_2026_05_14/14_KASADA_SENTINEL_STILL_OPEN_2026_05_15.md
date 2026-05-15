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

## Recommended next experiment (for the next session)

Extend `kasada_vm_dispatcher_trace` to, at handler-8 (write) and
handler-10 (read) invocations whose key === the sentinel string,
record `Object.prototype.toString.call(target)`, a WeakRef identity
tag, and the realm marker. Then a single canadagoose run yields the
exact object whose identity we fail to preserve. That is the load-
bearing unknown; everything else in the W-plan is already shipped.

## What IS shipped (so this doc isn't misread as "nothing works")

W1.1–W1.10, W2.6/2.7/2.8, Akamai `_abck` parser + dynamic tenant +
v3 envelope + CounterTuple, PerimeterX iOS surface gating, DataDome
iframe self-loops + sigma-lognormal behavior, the interfaces_bootstrap
stub-preemption fix, and the V8 stack-name scrub are all in `main`.
chrome_compat is 415/415 green. The Kasada VM sentinel is the single
identified universal blocker that remains and it is explicitly a
multi-day VM-emulation problem, not a missed quick win.
