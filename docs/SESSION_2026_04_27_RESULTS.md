# Session 2026-04-27 — CI/CD Fix: Worker Wire Serialization + Storage Persistence

A focused debugging session that fixed all failing tests so CI/CD passes clean.
Three distinct root-cause bugs traced and closed. Zero test regressions.

**Commit**: `3748ebd` — "fix: Worker postMessage binary types + storage persistence across navigations"

---

## 1. Headline

| Metric | Before | After |
|---|---|---|
| `cargo test --workspace` | failing (3+ test failures) | **all pass (0 failures)** |
| Worker binary-type round-trip tests | 3 FAIL | 3 PASS |
| Storage persistence tests | 2 FAIL | 2 PASS |
| Worker echo/addEventListener tests | FAIL | PASS (fixed in prior segment) |
| execute_script arity errors | compile errors | fixed |

All test suites run clean. CI/CD unblocked.

---

## 2. Bug 1 — Worker.postMessage drops ArrayBuffer/TypedArray/Map/Set/Date

### Symptom

```
thread 'worker_post_message_arraybuffer_round_trip' panicked:
  left: Some("false|-1|-1|-1")
 right: Some("true|4|16|64")

thread 'worker_post_message_typed_array_round_trip' panicked:
  left: Some("false|NOT_U8:[object Object]")
 right: Some("true|1,2,3,250,255")

thread 'worker_post_message_map_set_date_survive' panicked:
  left: None
 right: Some("true,true,true,true,true,true,true,true")
```

Worker received `{}` or a plain tagged object instead of real `ArrayBuffer` / `Map` / `Uint8Array`.

### Root cause — `Object.freeze` on `__boxide`

At the end of `dom_bootstrap.js` (line 1791), the bootstrap installs a FINAL `__boxide` object:

```javascript
// BEFORE (broken):
Object.defineProperty(globalThis, '__boxide', {
    value: Object.freeze({ _getNodeId }),
    enumerable: false,
    configurable: true,
    writable: false,
});
```

`Object.freeze` makes the object immutable — no new properties can be added. Then `structured_clone.js` runs AFTER `dom_bootstrap.js` and tries:

```javascript
if (!globalThis.__boxide) globalThis.__boxide = {};
globalThis.__boxide.serializeForWire = _serializeForWire;   // silently fails (frozen!)
globalThis.__boxide.deserializeFromWire = _deserializeFromWire; // silently fails (frozen!)
```

In non-strict mode, assignments to frozen objects fail silently. So `serializeForWire` and `deserializeFromWire` were never registered. The `_boxide` captured at line 3 of `window_bootstrap.js` was the frozen object without wire functions. Every `Worker.postMessage(complexValue)` fell through to `|| message`, JSON-stringifying raw ArrayBuffers as `{}`.

### Fix

**`crates/js_runtime/src/js/dom_bootstrap.js`** — remove `Object.freeze`:

```javascript
// AFTER (fixed):
Object.defineProperty(globalThis, '__boxide', {
    value: { _getNodeId },
    enumerable: false,
    configurable: true,
    writable: false,
});
```

Now `structured_clone.js` can add `serializeForWire`/`deserializeFromWire` to the mutable object. The `_boxide` closure captured in `window_bootstrap.js` is a reference to the same object, so it picks up both wire functions after `structured_clone.js` runs.

### Secondary fix — use closure-captured `_boxide` in poll timer and worker postMessage

`cleanup_bootstrap.js` deletes `globalThis.__boxide` before page scripts run. Two call sites were still using `globalThis.__boxide` by name instead of the captured closure variable, so they'd silently get `undefined` after cleanup:

**`crates/js_runtime/src/js/window_bootstrap.js`** — Worker poll timer:

```javascript
// BEFORE:
const deserializer = globalThis.__boxide && globalThis.__boxide.deserializeFromWire;
// AFTER:
const deserializer = _boxide && _boxide.deserializeFromWire;
```

**`crates/js_runtime/src/js/worker_bootstrap.js`** — worker-side `postMessage`:

```javascript
// BEFORE:
wire = (globalThis.__boxide && globalThis.__boxide.serializeForWire && ...) || message;
// AFTER:
wire = (_boxide && _boxide.serializeForWire && ...) || message;
```

The worker's `_boxide` is captured at line 9 of `worker_bootstrap.js`, AFTER `structured_clone.js` runs (which is step 5 in the worker bootstrap sequence), so it has both wire functions at capture time.

### Why the other worker tests were passing

The echo/addEventListener/blob/module tests all sent strings or plain objects — no TypedArrays, ArrayBuffers, Maps, or Dates. Plain values serialize correctly through `JSON.stringify` so those tests never hit the missing wire serializer.

---

## 3. Bug 2 — Worker `onmessage` never fired (EventTarget missing)

### Symptom (from prior session segment)

Worker echo tests (`worker_echo_round_trip`, `worker_addeventlistener_roundtrip`) returned empty results. The worker's `drainOnce` poll loop called `self.dispatchEvent(event)` but `dispatchEvent` didn't exist in the worker scope — silently thrown exception swallowed.

### Root cause

`worker_bootstrap.js` had no EventTarget implementation. The `drainOnce` function tried to call `self.dispatchEvent(event)` and `self.addEventListener('message', fn)` but those weren't defined.

### Fix

**`crates/js_runtime/src/js/worker_bootstrap.js`** — added full EventTarget:

```javascript
const _wListeners = {};
self.addEventListener = function addEventListener(type, fn) { ... };
self.removeEventListener = function removeEventListener(type, fn) { ... };
self.dispatchEvent = function dispatchEvent(event) {
    // fire registered listeners, then self.onmessage
    ...
    return true;
};
```

---

## 4. Bug 3 — `structured_clone.js` wire registration skipped by early-return guard

### Symptom

`structured_clone.js` originally had this structure:

```javascript
((globalThis) => {
    // ... polyfill code with structuredClone ...
    if (typeof globalThis.structuredClone === "function") {
        return;  // EXITS BEFORE registering wire functions
    }
    // wire functions registered here (dead code path)
})(globalThis);
```

When deno_core provides a native `structuredClone` (or any runtime where it was pre-defined), the IIFE returned early and `serializeForWire`/`deserializeFromWire` were never registered on `__boxide`.

### Fix

**`crates/js_runtime/src/js/structured_clone.js`** — moved wire registration BEFORE the early-return guard:

```javascript
((globalThis) => {
    // Wire functions ALWAYS registered first
    const TAG = "__boxsc";
    function _serializeForWire(value, seen) { ... }
    function _deserializeFromWire(value) { ... }

    if (!globalThis.__boxide) globalThis.__boxide = {};
    globalThis.__boxide.serializeForWire = _serializeForWire;
    globalThis.__boxide.deserializeFromWire = _deserializeFromWire;

    // structuredClone polyfill — only install if not natively present
    if (typeof globalThis.structuredClone === "function") {
        return;  // Wire functions already registered above, safe to exit
    }
    // polyfill ...
})(globalThis);
```

---

## 5. Bug 4 — localStorage/sessionStorage lost on navigation

### Symptom

`test_local_storage_persistence_across_navigation` and `test_session_storage_persistence_across_navigation` failed — storage values written on page 1 were not visible on page 2.

### Root cause

`navigate_loop_internal` in `page.rs` created a new `Page` for each redirect without carrying over the storage state from the previous page. `drop(page)` discarded the storage.

### Fix

**`crates/browser/src/page.rs`** — thread `current_storage` through the iteration loop:

```rust
let mut current_storage: Option<HashMap<String, HashMap<String, String>>> = None;
// ...
// Before each drop(page):
current_storage = Some(page.event_loop().get_storage());
drop(page);
// ...
// When building the next page:
Self::build_page_with_scripts_init_and_storage(..., current_storage.take())
```

Applied at BOTH `drop(page)` sites in the loop: the cookie-delta retry path and the pending-navigation follow path.

---

## 6. Bug 5 — `execute_script` arity mismatch

### Symptom

Compile errors: `execute_script` called with 1 argument but signature requires 2.

### Fix

Added `, None` as second argument at 4 call sites in `crates/workers/src/lib.rs` and `crates/js_runtime/tests/worker.rs` and `crates/js_runtime/tests/audio_fingerprint.rs`.

---

## 7. Bootstrap execution order (important reference)

### Main runtime bootstrap order

```
1. console_bootstrap.js
2. stealth_bootstrap.js
3. interfaces_bootstrap.js
4. instances_bootstrap.js
5. fetch_bootstrap.js
6. timer_bootstrap.js
7. dom_bootstrap.js         ← creates __boxide = { _getNodeId } (now mutable)
8. event_bootstrap.js
9. canvas_bootstrap.js
10. window_bootstrap.js     ← captures: const _boxide = globalThis.__boxide  ← still mutable here
11. streams_bootstrap.js
12. structured_clone.js     ← adds serializeForWire/deserializeFromWire to __boxide ← works now
13. cleanup_bootstrap.js    ← deletes globalThis.__boxide (but captured _boxide survives)
```

### Worker runtime bootstrap order

```
1. stealth_bootstrap.js
2. console_bootstrap.js
3. timer_bootstrap.js
4. fetch_bootstrap.js
5. structured_clone.js      ← creates __boxide = {} (no dom_bootstrap), adds wire functions
6. worker_bootstrap.js      ← captures: const _boxide = globalThis.__boxide (has wire functions)
7. canvas_bootstrap.js
8. cleanup_bootstrap.js     ← deletes globalThis.__boxide (but captured _boxide survives)
```

Key invariant: `_boxide` in both runtimes is a **closure-captured object reference** that survives `cleanup_bootstrap.js`'s deletion of `globalThis.__boxide`. All access to wire functions must go through this captured reference, not through `globalThis.__boxide`.

---

## 8. Files changed

```
crates/js_runtime/src/js/dom_bootstrap.js
  ! Remove Object.freeze from final __boxide installation

crates/js_runtime/src/js/structured_clone.js
  ! Restructured: wire functions registered before structuredClone early-return guard

crates/js_runtime/src/js/window_bootstrap.js
  ! Worker poll timer: globalThis.__boxide → _boxide (closure-captured)
  + Worker class EventTarget unchanged (already used _boxide for serialization)

crates/js_runtime/src/js/worker_bootstrap.js
  + EventTarget (addEventListener, removeEventListener, dispatchEvent)
  ! worker-side postMessage: globalThis.__boxide → _boxide (closure-captured)

crates/browser/src/page.rs
  + current_storage threading through navigate_loop_internal
  + build_page_with_scripts_init_and_storage called with current_storage.take()

crates/workers/src/lib.rs
  ! execute_script calls: add None second arg

crates/js_runtime/tests/worker.rs
  ! execute_script calls: add None second arg

crates/js_runtime/tests/audio_fingerprint.rs
  ! execute_script calls: add None second arg
```

---

## 9. Don't repeat these mistakes

- **`Object.freeze` on internal bridge objects**: if other bootstrap files need to ADD properties to `__boxide` after `dom_bootstrap.js` sets it, the object must be mutable. Never freeze an object that sibling bootstrap scripts depend on extending.
- **`globalThis.__boxide` access after cleanup**: cleanup_bootstrap.js deletes `__boxide`. Any runtime code (poll timers, event handlers, postMessage) must capture the reference in a closure variable during bootstrap, not read `globalThis.__boxide` at call time.
- **Non-strict mode freezing pitfall**: assigning to a frozen object's property is a silent no-op in non-strict mode. This class of bug produces no error, no warning, and the function reference is simply `undefined`. Always verify wire functions are actually registered with a quick `console.log(typeof __boxide.serializeForWire)` when debugging Worker postMessage failures.
- **Worker bootstrap doesn't run dom_bootstrap.js**: the worker's `__boxide` starts as `undefined`. `structured_clone.js` creates it from scratch via `if (!globalThis.__boxide) globalThis.__boxide = {}`. This means worker's `__boxide` is always mutable — the freeze bug only affected the main runtime.
