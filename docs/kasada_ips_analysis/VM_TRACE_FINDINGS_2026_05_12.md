# Kasada VM Dispatcher Trace — Findings (2026-05-12)

First successful dynamic capture of the VM dispatcher state at the point
of engine divergence. Companion to `docs/CANADA_GOOSE_DIAGNOSIS_2026_05_10_PART2.md`
(static analysis from May 10) and `docs/kasada_ips_analysis/scratch/CRACK_PROGRESS_2026_05_10.md`.

## Tool

- **`crates/browser/tests/chrome_compat.rs::kasada_vm_dispatcher_trace`** — `#[ignore]` network test that hooks `globalThis.Function` BEFORE ips.js loads. Wraps every dynamically-created VM handler and logs each invocation (args, return, exceptions) into a 4000-entry ring buffer. Writes `./kasada_vm_trace.json` on completion.
- **`docs/kasada_ips_analysis/scratch/analyze_vm_trace.py`** — companion analyzer that cross-references the trace with `decrypted_blob_*.json`, partitioning throws into `'ball'` (normal Kasada control flow) vs **engine-divergence TypeErrors**.

Run:
```bash
cargo test --release -p browser --test chrome_compat -- \
    kasada_vm_dispatcher_trace --ignored --test-threads=1 --nocapture
cp crates/browser/kasada_vm_trace.json ./kasada_vm_trace.json
python3 docs/kasada_ips_analysis/scratch/analyze_vm_trace.py
```

## Captured run on canadagoose.com (2026-05-12)

- **Handler count**: 303 (151 outer factories + 152 inner handlers — the recursive wrap caught both layers of `new Function('return function(n,e,a,v,i,r){...}')()`)
- **Trace size**: 4000 (ring buffer cap reached — Kasada VM cycles much more than this)
- **Total throws captured**: 10
  - **5 normal `'ball'`** (Kasada's scope-chain miss — control flow, not bug)
  - **5 engine-divergence TypeErrors** — these are fixable engine gaps

## Engine-divergence throws

Captured stack pattern is identical for all 5:
```
TypeError: Cannot read properties of undefined (reading 'unjzomuybtbyyhwwkdpkxomylnab')
    at eval (<anonymous>:3:66)            ← inside an eval'd handler body
    at h (<init_script_0>:51:34)          ← our wrapper (this test)
    at U (.../ips.js:5:116)               ← Kasada's dispatcher U
    at eval (<anonymous>:3:254)           ← outer eval'd caller
    at eval (<anonymous>:3:52)            ← outermost
```

**Throwing handler IDs**: 28, 32, 70, 127, 155, 243

### Handler bodies (verified from trace's `throwing_handler_bodies` map)

| Handler | Body | Opcode (per `opcode_table.md`) |
|---|---|---|
| 28 | `a(n, e(n)[e(n)])` | GET (1 prop) |
| 32 | `var o=e(n),l=e(n),_=e(n),h=r[4]; if(l[h] && l[h].I===l) { ... }` | **FUNCTION CALL FRAME SETUP (NEW)** |
| 70 | `e(n)[e(n)]=e(n); var o=e(n),l=e(n),_=e(n),h=r[4]; if(l[h] && l[h].I===l) { ... }` | PUT then FRAME SETUP |
| 127 | (variant) | (function-call related) |
| 155 | `var v=e(n), i=e(n); a(n, v(i))` | CALL FUNCTION (1 arg) |
| 243 | `a(n, new Array(e(n))); e(n)[e(n)]=e(n); ...; if(l[h] && l[h].I===l) {...}` | NEW ARRAY + PUT + FRAME SETUP |

The throw consistently happens at column 66 — that's the position of `l[h]` inside the FRAME SETUP variants (handlers 32, 70, 243), or at column 82 / column 45 for the simpler GET / CALL variants (28, 155, 175).

### Pre-throw window pattern

The 60 ops before the first throw show repeated dispatcher cycles, all with `arg0=o{V,e}` (the VM state object containing `V` chain + `e` value-fetcher), all returning `undefined`. The handler-id sequence right before the first throw:
```
... h=14 (PUT) → h=32 (FRAME SETUP, THROWS) ...
... h=24 (GET WINDOW PROP) → h=28 (GET, THROWS) ...
```

This means the GET WINDOW PROP / GET handlers walk the scope chain `r=v(n); for(...; r=r.Q)` looking for a binding. When they find it, they fetch `r.h[i]` — but our engine has `r.h[i] === undefined` where Chrome has a real frame object with `.unjzomuybtbyyhwwkdpkxomylnab` set.

## Cross-reference with blob taxonomy

Per `CANADA_GOOSE_DIAGNOSIS_2026_05_10_PART2.md` §"Highest impact":
> Fields `bot1225`, `csc`, `kl`, `dpv` all carry the same exception:
> `TypeError: Cannot read properties of undefined (reading 'unjzomuybtbyyhwwkdpkxomylnab')`
> ... `bot1225` is reportedly the single biggest contributor to the trust score.

So the trace dynamically confirms what static analysis showed: **5 distinct probes (`bot1225, csc, kl, dpv, smc`) all fire because of the same root cause** — Kasada's VM expects a tagged function-frame object in our scope chain that we don't produce.

## What this is NOT

Not a Web API stub gap. The `unjzomuybtbyyhwwkdpkxomylnab` property is a **Kasada-internal sentinel** (a 28-char random identifier used to tag VM-tracked frames). It's never read from a real Web API — it's set on certain values BY Kasada earlier in execution and then re-read.

Specifically, looking at handler 32's body shape, this is the **FUNCTION CALL FRAME SETUP** opcode. It expects `l = e(n)` to return a previously-tagged closure or callable. The tag (`l[h] && l[h].I === l`) is Kasada's "is this already-set-up function frame mine" check.

## What needs to be fixed (not in this turn — multi-day dev)

When Kasada captures a function reference (e.g. via a `getOwnPropertyDescriptor` on a Web API method, or via `Function.prototype.bind` on a built-in), they then assign a sentinel property to the resulting object: `result.unjzomuybtbyyhwwkdpkxomylnab = something`. They later look up the function via the scope chain and check the tag.

If our engine's scope chain emulation drops the tag — e.g. because we use Proxy or a fresh object on each access instead of returning a stable reference — the tag isn't there on lookup, and the divergence fires.

**Investigation entry points**:
1. Audit which Web API methods get re-bound or re-wrapped between accesses (could lose attached properties)
2. Check whether our `Object.getOwnPropertyDescriptor` returns a fresh descriptor each call, dropping anything Kasada hung off the previous call's `.value`
3. Verify whether `_maskFunction`'d wrappers create new function objects each call (they shouldn't — they should be stable references)

## Other notable throws (not blockers, listed for completeness)

- `"Failed to construct 'structuredClone': Illegal constructor"` (handler 70 returning structuredClone construction probe) — matches blob `nppm`, message text already correct per Chrome
- `"Class extends value #<C> is not a constructor or null"` (handler 32 returning `Class extends` probe) — matches blob `npc/fsc`
- `"String.prototype.toString requires that 'this' be a String"` (handler 32) — normal probe behavior, message matches Chrome
- `"Function.prototype.toString requires that 'this' be a Function"` (handlers 70, 175) — same, normal probe behavior

## Tooling delivered this turn

| Artifact | Purpose |
|---|---|
| `chrome_compat.rs::kasada_vm_dispatcher_trace` | Live VM dispatcher trace, network test |
| `analyze_vm_trace.py` | Cross-reference trace + blobs, classify throws |
| `match_opcodes.py` (prior) | Map captured handler bodies → umasii opcode names |
| `opcode_table.md` (prior) | 62/164 of our 50+ opcode bodies labeled |
| **This document** | Findings from the first dynamic capture |

The trace is **reproducible per-session** — re-run it after any fix to confirm the divergence count drops from 5.
