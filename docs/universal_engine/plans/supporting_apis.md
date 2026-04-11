# Supporting Web APIs

Capabilities needed for a complete Chrome impression but not directly
tied to a currently-known-blocker. Mostly these are "things fingerprint
sites might check exist and behave correctly."

---

## IndexedDB

**Priority**: P1
**Effort**: 30-50 hours
**Why**: FingerprintJS, CreepJS, pixelscan all write a small entry to
IndexedDB to verify it's functional. Our current stub (in
`window_bootstrap.js`) returns fake IDBRequest objects that fire
onsuccess but don't actually persist. A closer look at one of these
fingerprinters would detect our stub.

**Approach**:

1. Use SQLite via `rusqlite` (MIT) as the backing store. Each origin
   gets its own SQLite database file under a configurable directory
   (default: in-memory).
2. Implement `IDBFactory.open(name, version)`, `IDBDatabase.
   createObjectStore`, `IDBDatabase.transaction`,
   `IDBObjectStore.put/get/delete/add/clear`,
   `IDBCursor.continue/advance`, `IDBKeyRange.bound/only`.
3. The hard part is the request/transaction lifecycle: every method
   returns an `IDBRequest` whose `onsuccess` fires asynchronously
   after the transaction commits. Our event loop needs to queue the
   onsuccess callback on a microtask after the op returns.
4. Structured clone of stored values (covered by the separate
   "Structured clone" section below).

**Files to add**:
- `crates/js_runtime/src/extensions/idb_ext.rs` — Rust ops backed by
  rusqlite
- `crates/js_runtime/src/js/idb_bootstrap.js` — JS surface

**License check**: rusqlite is MIT, SQLite itself is public domain.
Clean.

**Test**: write a simple FingerprintJS-style "open, put, get, compare"
test. Run against CreepJS's IndexedDB probe as a real-world check.

---

## Streams (ReadableStream / WritableStream / TransformStream)

**Priority**: P1
**Effort**: 20-30 hours
**Why**: Modern fetch responses use ReadableStream
(`response.body` is a ReadableStream). Some fingerprinters check
`typeof ReadableStream === 'function'` and verify it works. Our
current stub is minimal.

**Approach**: Pure JS implementation following the WHATWG Streams
spec. No ops needed — streams are entirely in-process.

1. Start from the reference implementation at
   https://github.com/whatwg/streams/tree/main/reference-implementation
   (MIT licensed). It's ~3000 lines of JS implementing the spec.
2. Copy the classes into `crates/js_runtime/src/js/streams_bootstrap.js`.
3. Wire `Response.prototype.body` in `fetch_bootstrap.js` to return a
   ReadableStream that yields the response body in chunks.
4. Wire `Request.prototype.body` similarly for POST bodies.

**Test**: 
```js
const response = await fetch(url);
const reader = response.body.getReader();
let received = 0;
while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    received += value.length;
}
console.log(`received ${received} bytes`);
```

---

## Structured clone algorithm

**Priority**: P1 (for Workers)
**Effort**: 8-15 hours
**Why**: `Worker.postMessage(data)` and `MessageChannel` both require
structured clone to serialize. Our current Worker implementation uses
`JSON.stringify` which fails on: cyclic references, TypedArrays, Map,
Set, Date, RegExp, Blob, ArrayBuffer. Real Chrome's structured clone
handles all of these.

**Approach**: Implement the WHATWG structured clone algorithm in JS.

```js
// crates/js_runtime/src/js/structured_clone.js
function structuredClone(value, options) {
    const seen = new Map();
    return clone(value, seen, options?.transfer || []);
}

function clone(value, seen, transferables) {
    // Primitives
    if (value === null || typeof value !== 'object' && typeof value !== 'function') {
        if (typeof value === 'function' || typeof value === 'symbol') {
            throw new DOMException('...', 'DataCloneError');
        }
        return value;
    }

    // Already seen (cycle or shared)
    if (seen.has(value)) return seen.get(value);

    // Specific types in order of specificity
    if (value instanceof Date) {
        const c = new Date(value.getTime());
        seen.set(value, c);
        return c;
    }
    if (value instanceof RegExp) {
        const c = new RegExp(value.source, value.flags);
        seen.set(value, c);
        return c;
    }
    if (ArrayBuffer.isView(value)) {
        // TypedArray / DataView
        const c = new value.constructor(value);
        seen.set(value, c);
        return c;
    }
    if (value instanceof ArrayBuffer) {
        const c = value.slice(0);
        seen.set(value, c);
        return c;
    }
    if (value instanceof Map) {
        const c = new Map();
        seen.set(value, c);
        for (const [k, v] of value) {
            c.set(clone(k, seen, transferables), clone(v, seen, transferables));
        }
        return c;
    }
    if (value instanceof Set) {
        const c = new Set();
        seen.set(value, c);
        for (const v of value) {
            c.add(clone(v, seen, transferables));
        }
        return c;
    }
    if (Array.isArray(value)) {
        const c = new Array(value.length);
        seen.set(value, c);
        for (let i = 0; i < value.length; i++) {
            c[i] = clone(value[i], seen, transferables);
        }
        return c;
    }
    // Plain object
    const c = {};
    seen.set(value, c);
    for (const key of Object.keys(value)) {
        c[key] = clone(value[key], seen, transferables);
    }
    return c;
}

globalThis.structuredClone = structuredClone;
```

**Integration**: change `Worker.postMessage` in `window_bootstrap.js`
to call `structuredClone(message)` before JSON-encoding for the
Rust side transport. (The actual wire format stays JSON; structured
clone handles the pre-serialization transformations.)

Or better: implement a binary format (like V8's serialization
protocol) so TypedArrays survive round-trips.

---

## Worker importScripts()

**Priority**: P2
**Effort**: 2-4 hours
**Why**: Some fingerprinters use `importScripts()` inside a Worker to
load additional code. Our current stub is a no-op.

**Approach**:

1. In `crates/js_runtime/src/js/worker_bootstrap.js`, implement
   `self.importScripts(...urls)`:
   ```js
   self.importScripts = function(...urls) {
       for (const url of urls) {
           // Fetch synchronously via an op.
           const source = Deno.core.ops.op_worker_sync_fetch(url);
           if (source) {
               // Execute in worker global scope.
               (0, eval)(source);
           } else {
               throw new Error(`importScripts failed to load ${url}`);
           }
       }
   };
   ```
2. Add `op_worker_sync_fetch(url)` to `crates/js_runtime/src/extensions/
   worker_ext.rs`. It does a blocking HTTP GET on the worker thread
   (which has its own tokio runtime — use `block_on`).

**Test**: spawn a worker, have it importScripts a blob:URL or
data:URL, verify the imported code runs.

---

## Module workers (type: 'module')

**Priority**: P2
**Effort**: 6-10 hours
**Why**: Newer sites use `new Worker(url, { type: 'module' })` for
ES6 import support. Our implementation only handles classic workers.

**Approach**:

1. Parse the `type` option in the `Worker` constructor.
2. For module type, use deno_core's module loader to resolve and
   execute the worker script as an ES module instead of as a classic
   script.
3. Implement `import` in workers — deno_core handles this via its
   module resolver; we just need to wire it into the worker
   runtime's `JsRuntime::load_main_es_module` path.

---

## Worker transferables

**Priority**: P2
**Effort**: 6-10 hours
**Why**: Transferables let you move an ArrayBuffer to a worker without
copying. `worker.postMessage(buf, [buf])` detaches `buf` from the
main thread. Used by high-performance web apps and some
fingerprinters.

**Approach**:

1. Extend the message passing format to include a "transferables"
   section alongside the cloned data.
2. On the sender side, neuter (detach) the transferred ArrayBuffer
   after the op returns.
3. On the receiver side, reconstruct the ArrayBuffer from the bytes.
4. Only supports `ArrayBuffer` and `MessagePort` initially; add
   `OffscreenCanvas`, `ImageBitmap`, etc. later if needed.

---

## OffscreenCanvas in Workers

**Priority**: P2
**Effort**: 10-20 hours
**Why**: OffscreenCanvas is the recommended API for heavy canvas
work in workers (avoids blocking the main thread). A few
fingerprinters test this.

**Approach**:

1. Forward canvas ops from the worker thread to the main thread via
   the worker message channel.
2. Main thread runs the actual canvas ops against a shared
   `Canvas2D` instance owned by the main thread.
3. Results (like `getImageData` output) flow back through the same
   channel.

Tricky because canvas ops are synchronous on the worker side (
`ctx.fillRect(...)` doesn't return a promise). We'd need either
(a) transfer the canvas ops as batched operations that the main
thread processes, or (b) block the worker thread on a response (
which defeats the point).

**Alternative**: give the worker thread its own `Canvas2D` instance
independent of the main thread. Fine for rendering but means the
canvas isn't shared with the main thread, so `transferControlToOffscreen`
semantics are different from real Chrome.

**Defer** unless a specific site fails on this.

---

## Fetch blob: URLs

**Priority**: P1
**Effort**: 2-4 hours
**Why**: Some sites do `fetch(blob:URL)` to load dynamically-created
resources. Our fetch currently rejects `blob:` URLs (see
`crates/js_runtime/src/js/fetch_bootstrap.js:115,130`). This is a
regression from task #8 ago — blob URLs should work now that we
have the BlobRegistry from T1.5.

**Approach**:

1. In `crates/js_runtime/src/js/fetch_bootstrap.js`, handle `blob:`
   URLs by calling `op_blob_fetch_text(url)` (or a new
   `op_blob_fetch_bytes(url)` for binary blobs).
2. Return a synthetic `Response` object with the blob's bytes as the
   body, content-type from the Blob's `type` field.
3. Test: `fetch(URL.createObjectURL(new Blob(['hello'])))` should
   return 200 with body "hello".

---

## SharedWorker real implementation

**Priority**: P3 (nice to have, not load-bearing)
**Effort**: 15-25 hours
**Why**: SharedWorker allows multiple browsing contexts to share one
worker. Rarely used by fingerprinters.

**Approach**: extend the worker registry in `worker_ext.rs` to
support named workers with multiple connections. Ports via
`MessagePort`.

**Defer** unless a site fails because of this.

---

## ServiceWorker real implementation

**Priority**: P3 (substantial work for marginal value)
**Effort**: 40-80 hours
**Why**: ServiceWorker intercepts fetches. Fingerprinters don't
usually probe it. Sites that use it do so for push notifications and
offline support, neither of which a scraper cares about.

**Approach**: out of scope for 2026 plan. Keep the stub that exists.

---

## Proxy-backed stub prototypes for DOM classes

**Task**: #55
**Priority**: P2
**Effort**: 10-15 hours
**Why**: Our current DOM class hierarchy (HTMLDivElement,
HTMLCanvasElement, etc.) has prototype methods that throw if called
on the wrong class. Real Chrome's DOM uses WebIDL-generated bindings
where methods forward through a proxy chain. Some sophisticated
fingerprinters walk the prototype chain and check that each method
is callable on the correct types.

**Approach**:

1. Identify which DOM classes have the throw-on-wrong-type bug.
2. Wrap their prototypes with Proxy traps that silently delegate to
   the right handler instead of throwing.
3. Chrome-specific: match the error messages real Chrome produces
   when a DOM method IS called on the wrong type (e.g.,
   `"Illegal invocation"` at the exact point real Chrome throws).

**Note**: this is "completeness" work. No specific site is known to
need it. Do it if you have spare time and everything above is done.

---

## Total Sprint 3 budget

| Item | Priority | Effort |
|---|---|---|
| IndexedDB | P1 | 30-50h |
| Streams | P1 | 20-30h |
| Structured clone | P1 | 8-15h |
| importScripts() | P2 | 2-4h |
| Module workers | P2 | 6-10h |
| Transferables | P2 | 6-10h |
| OffscreenCanvas in Workers | P2 | 10-20h |
| Fetch blob: URLs | P1 | 2-4h |
| SharedWorker real impl | P3 | 15-25h |
| ServiceWorker real impl | P3 | 40-80h |
| Proxy DOM prototypes | P2 | 10-15h |
| **Total** | — | **149-263h** |

P1 items alone: **60-99 hours**. Those are the ones likely to pay
off for stealth browsing. P2/P3 are optional and can be done later.

## Recommended order

1. Fetch blob: URLs (2-4h, trivial, part of T1.5 completion)
2. Structured clone (8-15h, needed for robust Worker.postMessage)
3. importScripts (2-4h, cheap win)
4. Streams (20-30h, visible surface area that fingerprinters check)
5. IndexedDB (30-50h, the big one; unblocks many fingerprint
   sites)

Defer everything else (transferables, OffscreenCanvas in workers,
SharedWorker, ServiceWorker, Proxy DOM) until a specific site
needs them.
