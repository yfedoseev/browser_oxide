# workers — Web Workers + Service Workers

New crate. Provides Web Worker and Service Worker support. Each worker runs in its own V8 Isolate.

## Why Workers Matter

- Anti-bot systems run fingerprinting/challenge code inside Web Workers
- Kasada and some DataDome challenges execute WASM in workers
- Some SPAs offload heavy computation to workers
- `navigator.serviceWorker` must exist for fingerprint checks (anti-bot probes for API presence)

## Web Workers

### What We Implement

```javascript
// Page creates a worker
const worker = new Worker('/worker.js');
worker.postMessage({ type: 'compute', data: payload });
worker.onmessage = (e) => console.log(e.data);
worker.terminate();

// Inside worker.js
self.onmessage = (e) => {
    const result = heavyComputation(e.data);
    self.postMessage(result);
};
```

### Implementation

Each Web Worker gets:
- **Own V8 Isolate** — Isolated from the page's Isolate (this is how Deno does it)
- **Own event loop** — Timers, microtasks, async ops independent of page
- **Limited global scope** — `DedicatedWorkerGlobalScope`: no `document`, no `window`, but has `self`, `postMessage`, `importScripts`, `fetch`, `setTimeout`, `crypto`, etc.
- **WASM support** — Full WebAssembly access (required for anti-bot challenges in workers)

### Communication: Structured Clone + postMessage

`postMessage` serializes data via the Structured Clone Algorithm:

**Must support:**
- Primitives (string, number, boolean, null, undefined, BigInt)
- Date, RegExp
- ArrayBuffer, TypedArrays (Uint8Array, Float64Array, etc.)
- Map, Set
- Plain objects, arrays
- Cyclic references

**Must reject (DataCloneError):**
- Functions
- DOM nodes
- Symbols
- WeakMap, WeakSet

**Transferable objects:**
- ArrayBuffer — zero-copy transfer (sender loses access)
- MessagePort — transfer communication channels
- OffscreenCanvas — transfer rendering context

### Architecture

```
workers/
├── src/
│   ├── lib.rs
���   ├── web_worker.rs       # DedicatedWorker — spawn V8 Isolate, message channel
│   ├── service_worker.rs   # ServiceWorker registration + fetch interception
│   ├── global_scope.rs     # Worker global scope (self, postMessage, importScripts)
│   ├── structured_clone.rs # Structured Clone Algorithm implementation
│   ├── message_port.rs     # MessagePort + MessageChannel
│   └── error.rs
├── tests/
│   ├── worker_lifecycle.rs
│   ├── structured_clone.rs
│   └── wasm_in_worker.rs   # WASM execution inside worker
└── Cargo.toml
```

## Service Workers

### Minimum Viable Implementation

Full Service Worker support is complex (Cache API, lifecycle, update algorithm). For SOTA anti-bot evasion, we need:

**Must have (fingerprint checks):**
- `navigator.serviceWorker` API object exists
- `navigator.serviceWorker.register()` accepts the call
- `navigator.serviceWorker.ready` resolves (as Promise)
- `navigator.serviceWorker.controller` returns null (no active SW)

**Nice to have (if sites require it):**
- Lifecycle: `install` → `activate` → `fetch` event handling
- `event.respondWith(response)` — intercept fetch
- Cache API basics: `caches.open()`, `cache.match()`, `cache.put()`
- `clients.claim()`, `self.skipWaiting()`

**Implementation strategy**: Expose the full API surface (methods exist, don't throw). Start with the lifecycle basics. Add Cache API if real sites require it.

### Service Worker as V8 Isolate

Like Web Workers, each Service Worker runs in its own V8 Isolate with:
- `ServiceWorkerGlobalScope` (self, fetch event, install event, activate event)
- Access to Cache API, fetch, timers
- No DOM access
