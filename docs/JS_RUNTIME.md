# js_runtime — V8 Engine + DOM Bindings + WebAssembly

Executes page JavaScript and WebAssembly with full access to browser_oxide's DOM and Web APIs.

## Engine: V8 via rusty_v8 + deno_core

| Property | Value |
|---|---|
| Engine | V8 (same engine as Chrome, Deno, Node.js) |
| Rust bindings | `rusty_v8` (MIT) — stable since v129.0.0 |
| Ops layer | `deno_core` (MIT) — Rust functions as JS ops, event loop, module loading |
| ES conformance | 100% (ES2024+, all proposals Chrome supports) |
| WebAssembly | Full support (free with V8) — WASM MVP, SIMD, threads, GC |
| Build | Prebuilt V8 static libraries (seconds). Source build: ~30min |
| Binary size | ~30MB (V8 static lib) |

### Why V8, Not QuickJS

QuickJS (previous plan) fails for SOTA 2026:

| Requirement | QuickJS | V8 |
|---|---|---|
| Cloudflare Turnstile WASM challenges | **No WASM support** | Full WASM |
| Kasada proof-of-work (WASM) | **Fails** | Passes |
| Heavy SPA bundles (React 19, Next.js 15) | 70x slower, pages timeout | JIT-compiled, native speed |
| ES2024+ features | ~ES2023 only | Complete |
| Performance.now() timing accuracy | Different from Chrome | Same as Chrome |

### Why deno_core, Not Raw rusty_v8

`deno_core` adds critical infrastructure on top of raw V8 bindings:

- **Op system**: Define Rust functions callable from JS with `#[op2]` macro. Handles serialization, async, error propagation.
- **Event loop**: Integrated tokio event loop that drives V8 microtasks, timers, and async ops.
- **Module loader**: ES module resolution and loading via a `ModuleLoader` trait.
- **Snapshot support**: Serialize initialized V8 heap → restore instantly for fast page creation.
- **Extension system**: Bundle ops + JS glue into reusable extensions.

`deno_core` does NOT include Deno's runtime (no built-in fetch, no file system, no Node compat). It's a blank V8 + ops infrastructure.

## Architecture

```
js_runtime/
├── src/
│   ├── lib.rs              # JsRuntime struct — owns V8 Isolate via deno_core
│   ├── runtime.rs          # Runtime creation, snapshot, extension loading
│   ├── extensions/         # deno_core extensions (groups of ops)
│   │   ├── dom_ext.rs      # DOM ops (querySelector, createElement, etc.)
│   │   ├── window_ext.rs   # Window global (setTimeout, location, navigator)
│   │   ├── fetch_ext.rs    # fetch() → net crate
│   │   ├── console_ext.rs  # console.log/warn/error → Rust tracing
│   │   ├── event_ext.rs    # addEventListener, dispatchEvent
│   │   ├── storage_ext.rs  # localStorage, sessionStorage
│   │   ├── canvas_ext.rs   # Canvas 2D API → canvas crate
│   │   ├── webgl_ext.rs    # WebGL parameter stubs
│   │   ├── crypto_ext.rs   # crypto.getRandomValues, crypto.subtle
│   │   ├── url_ext.rs      # URL, URLSearchParams
│   │   ├── timer_ext.rs    # setTimeout, setInterval → event_loop
│   │   ├── perf_ext.rs     # performance.now(), performance.timing
│   │   ├── intl_ext.rs     # Intl.* locale verification (V8 built-in)
│   │   ├── speech_ext.rs   # speechSynthesis.getVoices() stub
│   │   ├── media_ext.rs    # MediaDevices, MediaSession stubs
│   │   └── websocket_ext.rs # WebSocket client → tokio-tungstenite
│   ├── bindings/
│   │   ├── mod.rs
│   │   ├── document.rs     # Document interface → DOM ops
│   │   ├── element.rs      # Element/HTMLElement → DOM ops
│   │   ├── node.rs         # Node interface → DOM ops
│   │   ├── navigator.rs    # navigator.* (stealth-controlled)
│   │   ├── screen.rs       # screen.* (stealth-controlled)
│   │   ├── location.rs     # window.location
│   │   ├── history.rs      # history.pushState, replaceState
│   │   └── mutation_observer.rs
│   ├── module_loader.rs    # ES module resolution → net crate fetch
│   └── snapshot.rs         # V8 heap snapshot for fast startup
├── tests/
│   ├── dom_manipulation.rs
│   ├── wasm_execution.rs   # WASM module instantiation
│   ├── fetch_tests.rs
│   └── timer_tests.rs
└── Cargo.toml
```

## V8 Isolate Lifecycle

```
1. Create V8 Isolate (or restore from snapshot)
2. Create Context with global object template
3. Populate globals (window, document, navigator, ...)
4. Execute page scripts
5. Run event loop (timers, fetch callbacks, WASM)
6. Extract content
7. Destroy Isolate (all JS objects freed)
```

**Snapshots** — Pre-initialize DOM bindings + Web API stubs, serialize V8 heap. On page creation, restore from snapshot (~5ms) instead of re-running setup JS (~50ms). This is how Deno achieves fast startup.

**Concurrent pages** — Each page gets its own V8 Isolate. Isolates are independent and can run on different tokio tasks. No shared mutable state between pages.

## Global Object Surface

The JS global must match what anti-bot scripts expect from Chrome. The full surface includes 50+ navigator properties, window properties, and API constructors. See [STEALTH.md](STEALTH.md) for the exhaustive list.

Critical globals that must exist and behave correctly:

```javascript
// Identity
window.chrome            // Must exist with correct shape
navigator.userAgent      // Profile UA
navigator.webdriver      // undefined (NOT false)
navigator.plugins        // Realistic plugin list
navigator.languages      // Profile languages

// APIs anti-bot probes for existence
window.speechSynthesis   // Must have getVoices()
window.Notification      // Constructor + permission
window.RTCPeerConnection // WebRTC presence
navigator.mediaDevices   // enumerateDevices()
navigator.bluetooth      // Presence (Chrome)
navigator.usb            // Presence (Chrome)
navigator.credentials    // Credential Management
navigator.permissions    // query() with realistic states

// Timing
performance.now()        // Chrome resolution (100μs normal, 5μs COOP)
performance.memory       // Chrome-specific heap info

// Integrity checks
document.hasFocus()      // Must return true
Function.prototype.toString.call(fn) // Must return "[native code]" for native fns
```

## WebAssembly

V8 provides WASM for free. Critical for:

- **Cloudflare Turnstile**: Runs WASM proof-of-work (~50-200ms computation)
- **Kasada**: Polymorphic WASM challenges with environment integrity checks
- **DataDome**: WASM-based browser verification (2025+)

Implementation: WASM modules loaded via `WebAssembly.compile()` / `WebAssembly.instantiate()` run natively in V8. No additional work needed beyond exposing the standard WebAssembly global (which V8 provides by default).

Key consideration: WASM timing must be realistic. Running in a debugger or instrumented V8 adds overhead that Kasada detects. browser_oxide runs V8 without debugging hooks by default.

## Module Loading

ES modules (`import ... from "..."`) resolved via custom `ModuleLoader`:

1. Relative URLs resolved against page's base URL
2. Absolute URLs fetched via net crate
3. Module source cached (same-URL dedup)
4. `import.meta.url` returns resolved module URL
5. Dynamic `import()` supported (returns Promise)
6. WASM modules importable via `import ... from "mod.wasm"` (stage 3 proposal)
