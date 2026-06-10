# event_loop — Timers, Microtasks, Promises, rAF

Implements the [HTML Living Standard event loop](https://html.spec.whatwg.org/multipage/webappapis.html#event-loops).

**Note**: With deno_core as the V8 ops layer, we get a production-grade event loop for free. deno_core integrates V8 microtasks with tokio, handles async ops, and manages Promise resolution. This crate wraps deno_core's event loop with browser-specific scheduling (rAF, idle callbacks, timer clamping).

## Why This Matters

JavaScript is single-threaded and event-driven. Without a correct event loop, nothing works:
- `setTimeout(() => {}, 0)` never fires
- `fetch().then(...)` callbacks never execute
- `Promise.resolve().then(...)` never runs
- `MutationObserver` callbacks never fire
- SPAs that use `requestAnimationFrame` hang
- WASM proof-of-work challenges never complete

## Event Loop Model

```
┌─────────────────────────────────────────┐
│              Event Loop                 │
│                                         │
│  1. Run oldest task from task queue     │
│  2. Run ALL microtasks                  │
│  3. If rendering opportunity:           │
│     a. Run rAF callbacks               │
│     b. Run IntersectionObserver        │
│     c. Run ResizeObserver              │
│  4. Go to 1                             │
│                                         │
│  Task Queue:                            │
│  ┌────┬────┬────┬────┐                  │
│  │ T1 │ T2 │ T3 │ ...│                  │
│  └────┴────┴────┴────┘                  │
│                                         │
│  Microtask Queue:                       │
│  ┌────┬────┬────┐                       │
│  │ M1 │ M2 │ M3 │                       │
│  └────┴────┴────┘                       │
└─────────────────────────────────────────┘
```

### Task Sources

| Source | Example |
|---|---|
| Timers | `setTimeout`, `setInterval` callbacks |
| Networking | `fetch` response callbacks, XHR `onload` |
| DOM events | `click`, `input`, `load`, `DOMContentLoaded` |
| History | `popstate` events |
| Messaging | `postMessage` handlers |

### Microtask Sources

| Source | Example |
|---|---|
| Promises | `.then()`, `.catch()`, `.finally()` callbacks |
| `queueMicrotask()` | Explicit microtask scheduling |
| MutationObserver | Mutation record delivery |

## Implementation

The event loop wraps deno_core's JsRuntime event loop with browser-specific scheduling:

```rust
pub struct BrowserEventLoop {
    /// deno_core handles: V8 microtasks, async ops, Promise resolution
    js_runtime: JsRuntime,    // deno_core::JsRuntime
    
    /// Browser-specific scheduling on top of deno_core
    timers: TimerRegistry,
    raf_callbacks: Vec<RafCallback>,
    idle_callbacks: Vec<IdleCallback>,
    running: bool,
}

pub struct TimerRegistry {
    next_id: u32,
    timers: BTreeMap<Instant, Vec<TimerEntry>>,
}

struct TimerEntry {
    id: u32,
    callback: v8::Global<v8::Function>,
    interval: Option<Duration>,  // None = setTimeout, Some = setInterval
    nesting_level: u8,           // For 4ms clamping at depth > 5
}
```

### Running the Loop

For scraping, we don't run forever. We run until:
1. The page is "stable" (no pending network requests, no pending timers < threshold)
2. A timeout is reached
3. A specific condition is met (e.g., an element appears)

```rust
impl BrowserEventLoop {
    pub async fn run_until_idle(&mut self, timeout: Duration) -> Result<()> {
        let deadline = Instant::now() + timeout;

        loop {
            // 1. Run deno_core event loop tick (handles V8 microtasks + async ops)
            self.js_runtime.run_event_loop(PollEventLoopOptions {
                wait_for_inspector: false,
                pump_v8_message_loop: true,
            }).await?;

            // 2. Fire ready timers
            self.fire_ready_timers()?;

            // 3. Fire rAF (simulated 60fps ticks)
            self.fire_raf_callbacks()?;

            // 4. Fire idle callbacks (if time remaining)
            self.fire_idle_callbacks(deadline)?;

            // 5. Check termination
            if self.is_idle() || Instant::now() >= deadline {
                break;
            }

            // 6. Sleep until next timer or network event
            let next_wake = self.next_timer_deadline();
            tokio::time::sleep_until(next_wake.min(deadline)).await;
        }

        Ok(())
    }
}
```

## Timer Semantics

Per the spec:
- `setTimeout(fn, 0)` clamps to 0ms (but runs after current task + microtasks)
- Nested `setTimeout` (depth > 5) clamps to minimum 4ms
- `setInterval` minimum is 4ms (to prevent CPU hogging, though for scraping we may relax this)
- Timer IDs are monotonically increasing integers
- `clearTimeout` / `clearInterval` cancel by ID

## Integration with deno_core + tokio

deno_core provides the async bridge between V8 and tokio:

```
tokio runtime
  └── deno_core JsRuntime
        ├── V8 Isolate (JS execution + WASM)
        ├── Op futures (Rust async ops callable from JS)
        ├── V8 microtask checkpoint (after each op resolves)
        └── Module loading (ES modules fetched async)
  └── BrowserEventLoop (wraps JsRuntime)
        ├── Timer scheduling (setTimeout/setInterval)
        ├── rAF simulation (16.67ms ticks)
        ├── requestIdleCallback
        └── Idle detection (network quiescence + timer drain)
```

Each iframe/worker has its own `BrowserEventLoop` + `JsRuntime`. They run as independent tokio tasks.

## Architecture

```
event_loop/
├── src/
│   ├── lib.rs          # EventLoop struct
│   ├── task.rs         # Task, Microtask, TaskSource
│   ├── timers.rs       # TimerRegistry, setTimeout/setInterval/clearTimeout
│   ├── raf.rs          # requestAnimationFrame (simulated)
│   └── idle.rs         # Idle detection (when is the page "done"?)
├── tests/
│   ├── timer_order.rs  # Verify correct execution order
│   ├── microtask.rs    # Promise/microtask interleaving
│   └── idle.rs         # Idle detection heuristics
└── Cargo.toml
```
