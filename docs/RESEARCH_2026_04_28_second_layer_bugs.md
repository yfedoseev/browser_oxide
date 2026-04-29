# Research — sannysoft + creepjs second-layer bug root causes

**Date**: 2026-04-28 (post `HANDOFF_2026_04_28_session_close.md`)
**Purpose**: Concrete root-cause analysis and fix plans for the two bugs unmasked when V8 #60 was closed.
**Method**: Repo code mapping + creepjs source review (deepwiki + GitHub raw fetch) + sannysoft live scrape + Rust signal-handler ecosystem survey.

## Status: implemented and verified 2026-04-28

Phase 1 + Phase 2 fixes shipped. Live test results:
- **sannysoft**: was Rust 64 MB stack overflow → now `L3-RENDERED (len=26272)`. The cycle assertion in `append_child` correctly catches buggy `appendChild(parent_429..467, document)` calls from sannysoft's pre-scripts and skips them.
- **creepjs**: was 40+ minute CPU spin → now `L3-RENDERED (len=57502)` in 20.6 seconds. Mirror-realm topological build + storage `has` trap + plugin-length memoization + 1 GB heap initial together resolved the spin.
- **All 15 chl_sites tests pass** (full regression sweep, 9.6 min total) — 0 regressions on the previously-passing set.
- **All 27 dom unit tests + 24 layout unit tests pass** including 4 new cycle-handling tests.
- The cycle assertion is also surfacing a real shim bug: JS attempts `appendChild(document, document)` and `appendChild(parent, document)` on multiple sites (now diagnosed instead of crashing). Tracked separately for follow-up.

---

## TL;DR

| Bug | Root cause (highest confidence) | Recommended fix | Effort |
|---|---|---|---|
| sannysoft 64 MB Rust overflow | Unbounded direct recursion in 5 arena.rs walkers + 2 layout walkers, exposed by a DOM cycle introduced by combination of pre-scripts | Convert recursion → iterative work-stack + visited-set tripwire + arena cycle assertion on `append_child`/`insert_before` | ~1 day |
| creepjs 100% CPU spin at 4 GB heap | Mirror-realm prototype escape on `dom_bootstrap.js:1836` causes creepjs to walk parent-realm prototype chain, multiplying lie-detection work 5-10× | Build fresh prototype chain inside the fresh realm + add `has` trap to storage Proxy + cache plugin/mime lengths | ~3 hours |

**Skip the signal-hook SIGSEGV approach** — known broken on macOS (see Sources). Iterative+tripwire gives equivalent diagnostics on every platform.

---

## Bug A — sannysoft Rust stack overflow at 64 MB

### Verified facts

- The actual `runBotDetection` function on bot.sannysoft.com (extracted live):
  ```js
  for (const documentKey in window['document']) {
    if (documentKey.match(/\$[a-z]dc_/) && window['document'][documentKey]['cache_']) {
      return true;
    }
  }
  if (window['document']['documentElement']['getAttribute']('selenium')) return true;
  if (window['document']['documentElement']['getAttribute']('webdriver')) return true;
  if (window['document']['documentElement']['getAttribute']('driver')) return true;
  ```
  No recursion in user-visible JS. The trigger is in our shim/op layer when responding to property reads during the `for…in`.

### Five unbounded recursive walkers (no depth guard) in `crates/dom/src/arena.rs`

| Function | Line | Reached via |
|---|---|---|
| `collect_text` | 239 | `op_dom_get_text_content` ← `node.textContent` |
| `find_element` | 434 | `op_dom_get_element_by_id` ← `getElementById` |
| `collect_elements` | 450 | `op_dom_get_elements_by_tag_name`/`class_name` |
| `serialize_node` | 326 | `op_dom_get_inner_html`/`outer_html` ← `innerHTML`/`outerHTML` |
| `merge_subtree` | 391 | innerHTML setter / parser merges |

Plus `crates/layout/src/engine.rs:127` `build_node` and `:194` `build_children` — same pattern.

### Why DOM depth alone shouldn't overflow

Rust frame ~200 B. 64 MB ÷ 200 B ≈ 320 K depth. Real DOM ≤100 deep. **Pure tree depth cannot be the trigger.**

### The actual trigger (high confidence)

A **DOM cycle** introduced by the combination of prior scripts. The bisection in the handoff confirms only the full combination crashes — each ingredient alone is safe. Suspects for the cycle:

1. `fpCollect.generateFingerprint` walking the tree with side-effects.
2. The `_onNodeInserted` JS-side depth guard (cap 64) tripping mid-mutation, leaving the arena in an inconsistent state.
3. A `cloneNode` + `appendChild` pattern that puts a node into its own subtree.
4. Document.write recursion that the existing JS guard didn't fully prevent at the arena level.

When script_30 (or earlier code) accesses any property whose getter calls a recursive walker, the walker enters the cycle and never terminates → 64 MB stack exhausts.

### Why macOS signal-handler approach is wrong

`backtrace()` from a SIGSEGV-on-stack-overflow **does not work on macOS** (any sigaltstack approach), confirmed by:
- `rust-lang/backtrace-rs#356` (open, "OS-macos / help wanted")
- `rust-lang/rust#69533`, `#71397`, `#104388`
- `backtrace-on-stack-overflow` crate explicitly disclaims macOS
- LLVM `D28265` recommends **disabling** `sigaltstack` on Apple

The handoff's "~2 h signal-hook plan" will not produce a backtrace on Darwin/arm64.

### Recommended fix (better than handoff plan)

**Part 1 — Convert recursion to iterative (~1 day, mechanical).**

Each recursive walker becomes an explicit `Vec<NodeId>` work-stack:

```rust
fn collect_text(&self, root: NodeId, result: &mut String) {
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        let Some(node) = self.get(id) else { continue };
        match &node.data {
            NodeData::Text(t) => result.push_str(t),
            _ => {
                // collect children, push reversed so we pop in document order
                let mut kids = Vec::new();
                let mut child = node.first_child;
                while let Some(c) = child {
                    kids.push(c);
                    child = self.get(c).and_then(|n| n.next_sibling);
                }
                stack.extend(kids.into_iter().rev());
            }
        }
    }
}
```

Apply to all 5 arena.rs walkers + `build_node`/`build_children`. Stack now grows on heap, not C-stack. Survives any DOM size and any cycle (cycles still loop forever, but bounded by Part 2).

**Part 2 — Tripwire + visited-set (~30 min).**

```rust
let mut visited = HashSet::with_capacity(1024);
let mut steps = 0usize;
while let Some(id) = stack.pop() {
    if !visited.insert(id) { continue; }
    steps += 1;
    if steps > 100_000 {
        panic!("DOM walk cycle in collect_text starting from {root:?} (visited {} unique nodes)", visited.len());
    }
    // ...
}
```

Panic produces a normal Rust backtrace from the *caller*'s stack. No signal-handler. Works on every OS.

**Part 3 — Cycle assertion at the mutation site (~30 min).**

Add to `append_child` and `insert_before` in arena: walk ancestors of `parent`, reject if `child` appears. Prevents the entire bug class from being introduced.

**Part 4 — (Optional) Drop trait-object recursion.**

`find_element` / `collect_elements` take `&dyn Fn(&Node) -> bool`. Trait-object call frames are slightly larger. Inlining via generics removes that, marginal but cheap.

### Validation

After fix:
1. Re-run `cargo test --workspace -- --test-threads=1` — should pass.
2. Run sannysoft via `chl_sites.rs`. If it still fails, the panic from Part 2 will name the offending walker + root NodeId.
3. Add a unit test: build an arena, manually wire a cycle (`append_child(a, b); set_parent(a, b)`), call `collect_text(a)` — should panic with a clear message, not segfault.

---

## Bug B — creepjs 100% CPU spin at 4 GB heap

### Verified facts

**creepjs `searchLies`** (from `src/lies/index.ts` via deepwiki + raw fetch):

```js
[...new Set([
  ...Object.getOwnPropertyNames(interfaceObject),
  ...Object.keys(interfaceObject),
])].sort().forEach((name) => {
  res = queryLies({...})
})
```

For each property, `queryLies` creates **3 fresh Proxies** and runs **~8 prototype-cycle tests** (`setPrototypeOf` to self, Proxy-of-self, etc.) expecting `RangeError "too much recursion"` or `TypeError "Cyclic __proto__ value"`.

The original V8 OOM was in `Builtins_ArrayPrototypePush` — confirms an unbounded array push. With heap raised to 4 GB, the loop just runs longer at 100% CPU.

### Root cause #1 (HIGH confidence) — Mirror-realm prototype escape

`crates/js_runtime/src/js/dom_bootstrap.js:1836`:

```js
const freshProto = Object.create(parentProto && Object.getPrototypeOf(parentProto) || Object.prototype);
```

For `parentProto = HTMLElement.prototype`, `Object.getPrototypeOf(parentProto)` returns the **parent realm's** `Element.prototype`. So `iframe.HTMLElement.prototype.__proto__ === parent.Element.prototype` — a cross-realm escape.

When creepjs walks the chain via repeated `Object.getPrototypeOf` (it does this in lie detection to compare cross-realm chains), it hops out of the fresh realm into the parent realm and then walks the **full** native chain there. This duplicates thousands of WebIDL property checks. With ~40 mirrored constructors × ~50 properties × ~24 lie-detection ops × cross-realm escape ≈ creepjs does 5-10× more work than on real Chrome.

**Fix** — build the fresh chain in topological order so each level's `__proto__` is the *fresh* grandparent:

```js
// Build fresh constructors in topological order: Object → EventTarget → Node → Element → HTMLElement → ...
const TOPO = [
  ["Object", null],
  ["Function", "Object"],
  ["EventTarget", "Object"],
  ["Node", "EventTarget"],
  ["Element", "Node"],
  ["HTMLElement", "Element"],
  ["HTMLDivElement", "HTMLElement"],
  // ... etc, derive parent from real chain
];

function _mkMirroredConstructor(parentCtor, name, freshGrandparentProto) {
    const fresh = function() { throw new TypeError("Failed to construct '" + name + "'"); };
    // ... existing toString shape ...
    const freshProto = Object.create(freshGrandparentProto || Object.prototype);
    // ... copy property descriptors as before ...
    return fresh;
}

function _buildRemoteRealm() {
    const realm = {};
    for (const [name, parentName] of TOPO) {
        const parentCtor = globalThis[name];
        if (typeof parentCtor === "function") {
            const freshGrandparent = parentName ? realm[parentName]?.prototype : Object.prototype;
            realm[name] = _mkMirroredConstructor(parentCtor, name, freshGrandparent);
        }
    }
    // ... rest unchanged ...
    return realm;
}
```

### Root cause #2 (MEDIUM-HIGH confidence) — Missing `has` trap on storage Proxy

`crates/js_runtime/src/js/window_bootstrap.js:2393`:

```js
return new Proxy({}, {
    get, set, deleteProperty, ownKeys, getOwnPropertyDescriptor,
    // no `has`!
});
```

V8 Proxy invariant: `has(P)` and `ownKeys()` must agree about extant keys. With no `has` trap, V8 falls back to `target = {}` which says "no keys", but `ownKeys()` returns the real list. **Every `prop in storage` check forces V8 to reconcile across all ownKeys**. creepjs runs many `'name' in apiFunction` and similar checks; storage isn't `apiFunction` directly but the same pattern hits anywhere we have proxies with this defect.

**Fix**:
```js
has(target, key) {
    if (['getItem','setItem','removeItem','clear','key','length'].includes(key)) return true;
    return ops.op_dom_storage_get(type, String(key)) !== null;
},
```

Audit other Proxies in `dom_bootstrap.js` for the same defect:
- `style` proxy at `dom_bootstrap.js:494` — has `get/set` only, no `has/ownKeys/getOwnPropertyDescriptor`
- `dataset` proxy at `dom_bootstrap.js:781` — same
- `attributes` proxy at `dom_bootstrap.js:768` — `length` hardcoded to 0 with no other traps

### Root cause #3 (MEDIUM confidence) — Plugin/PluginArray getter-valued numeric indices

`crates/js_runtime/src/js/window_bootstrap.js:202-234` defines `plugins[i]` as **getters** that re-fetch on every access. creepjs does `Array.from(navigator.plugins)` and `[...navigator.plugins]` — each spread iterates and calls every getter. If `_pluginsLen()` is inconsistent across calls (e.g., reads from a mutable array creepjs's `queryLies` indirectly mutates), the spread re-iterates.

**Fix** — cache `_pluginsLen()` once at construction and define numeric props as data properties, not accessors. (Plugins should be effectively immutable post-construction anyway.)

### Root cause #4 (LOW confidence, easy to test) — V8 heap initial too aggressive

`crates/js_runtime/src/runtime.rs:106`:

```rust
const HEAP_INITIAL: usize = 256 * 1024 * 1024;
```

256 MB initial may cause GC thrashing — V8 spends time in old-space compaction before growing past 256 MB. **Try**: drop to 64 MB (V8 default) or raise to 1 GB. Bench against creepjs cold-start time.

### Diagnosis path if fixes don't fully resolve

Add a JS instrumentation tag that runs *before* creepjs:

```js
const _origGOPN = Object.getOwnPropertyNames;
Object.getOwnPropertyNames = function(o) {
    const r = _origGOPN(o);
    if (r.length > 100) console.log("[probe] large GOPN", o, r.length);
    return r;
};
```

Surfaces any shim returning suspiciously large arrays in ~30 s. Cheaper than the heap-snapshot diff (~2 h vs 1 day).

---

## Recommended next-session order

| # | Task | Effort | Notes |
|---|---|---|---|
| 1 | Convert arena.rs + layout/engine.rs recursion → iterative + tripwire | 1 day | Closes sannysoft + protects against future DOM cycles |
| 2 | Add cycle assertion to `append_child`/`insert_before` | 30 min | Prevents the bug class at the root |
| 3 | Fix mirror-realm prototype chain (`dom_bootstrap.js:1836`) | 1 hour | Highest-confidence creepjs fix |
| 4 | Add `has` trap to storage proxy (+ audit style/dataset/attributes) | 30 min | Cheap, almost certainly helps creepjs |
| 5 | Cache plugin/mime lengths + convert numeric indices to data props | 1 hour | Eliminates a known O(N²) pattern under spread |
| 6 | Tune `HEAP_INITIAL` (try 1 GB) | 15 min + bench | Cheap, may flip creepjs from "slow" to "fast" |
| 7 | If creepjs still spins, add JS GOPN instrumentation hook | 2 hours | Surfaces the remaining bad shim concretely |

**Skip entirely**: signal-hook SIGSEGV handler (handoff's plan). Doesn't work on macOS, the iterative+tripwire approach gives strictly better diagnostics on every platform.

---

## Sources

- [rust-lang/backtrace-rs#356 — backtrace on stack overflow doesn't work on macOS](https://github.com/rust-lang/backtrace-rs/issues/356)
- [rust-lang/rust#69533 — Rust's signal stack should be guarded against overflow](https://github.com/rust-lang/rust/issues/69533)
- [rust-lang/rust#71397 — RUST_BACKTRACE on macOS can trigger crashes](https://github.com/rust-lang/rust/issues/71397)
- [rust-lang/rust#104388 — capturing backtrace slow/segfaults on Apple Silicon](https://github.com/rust-lang/rust/issues/104388)
- [backtrace-on-stack-overflow crate (disclaims macOS)](https://docs.rs/backtrace-on-stack-overflow/latest/backtrace_on_stack_overflow/)
- [LLVM D28265 — disable sigaltstack on Apple platforms](https://reviews.llvm.org/D28265)
- [creepjs DeepWiki — Lie Detection System](https://deepwiki.com/abrahamjuliot/creepjs)
- [creepjs source (lies/index.ts)](https://github.com/abrahamjuliot/creepjs)
- [bot.sannysoft.com (runBotDetection source, scraped live)](https://bot.sannysoft.com/)
- [Debugging A Stack Overflow In Rust — Geo's Notepad](https://geo-ant.github.io/blog/2022/debug-stackoverflow-in-rust/)
