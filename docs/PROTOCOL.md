# protocol — CDP Server (Puppeteer/Playwright Compatibility)

BrowserOxide exposes a Chrome DevTools Protocol (CDP) server so it can be driven by existing tools: Puppeteer, Playwright, scraper_oxide's browser layer.

## Why Implement CDP

- **Drop-in replacement**: Existing automation code that uses Puppeteer/Playwright works without changes
- **scraper_oxide integration**: scraper_oxide already speaks CDP — BrowserOxide just needs to respond to it
- **Ecosystem compatibility**: Testing tools, debugging tools, and CI/CD pipelines speak CDP
- **Lightpanda validates this approach**: They implemented CDP and achieved Puppeteer/Playwright compatibility

## Scope: CDP Subset

Full CDP has 40+ domains and thousands of commands. We implement the subset that scraping/automation tools actually use:

### Priority 1 — Core Navigation + Content

| Domain | Commands | Events |
|---|---|---|
| **Target** | `createTarget`, `closeTarget`, `getTargets`, `attachToTarget` | `targetCreated`, `targetDestroyed`, `targetInfoChanged` |
| **Page** | `navigate`, `reload`, `getFrameTree`, `setLifecycleEventsEnabled`, `addScriptToEvaluateOnNewDocument` | `frameNavigated`, `loadEventFired`, `domContentEventFired`, `lifecycleEvent` |
| **Runtime** | `evaluate`, `callFunctionOn`, `getProperties`, `releaseObject` | `executionContextCreated`, `executionContextDestroyed`, `consoleAPICalled` |
| **DOM** | `getDocument`, `querySelector`, `querySelectorAll`, `getOuterHTML`, `setOuterHTML`, `getAttributes` | `documentUpdated` |

### Priority 2 — Network + Interception

| Domain | Commands | Events |
|---|---|---|
| **Network** | `enable`, `disable`, `setCookies`, `getCookies`, `setExtraHTTPHeaders`, `setUserAgentOverride` | `requestWillBeSent`, `responseReceived`, `loadingFinished`, `loadingFailed` |
| **Fetch** | `enable`, `disable`, `fulfillRequest`, `continueRequest`, `failRequest` | `requestPaused` |

### Priority 3 — Input + Emulation

| Domain | Commands |
|---|---|
| **Input** | `dispatchMouseEvent`, `dispatchKeyEvent`, `dispatchTouchEvent` |
| **Emulation** | `setDeviceMetricsOverride`, `setUserAgentOverride`, `setGeolocationOverride`, `setTimezoneOverride`, `setLocaleOverride` |

### Priority 4 — Screenshots + PDF (Future)

| Domain | Commands |
|---|---|
| **Page** | `captureScreenshot`, `printToPDF` |

These would require a minimal rendering pipeline — deferred.

## Architecture

```
protocol/
├── src/
│   ├── lib.rs              # CdpServer — accepts WebSocket connections
│   ├── server.rs           # WebSocket server (tokio-tungstenite)
│   ├── session.rs          # Per-connection session state
│   ├── router.rs           # Method dispatch (domain.method → handler)
│   ├── types.rs            # CDP JSON types (auto-generated from protocol.json)
│   ├── domains/
│   │   ├── target.rs       # Target domain handlers
│   │   ├── page.rs         # Page domain handlers
│   │   ├── runtime.rs      # Runtime domain handlers
│   │   ├── dom.rs          # DOM domain handlers
│   │   ├── network.rs      # Network domain handlers
│   │   ├── fetch.rs        # Fetch domain handlers
│   │   ├── input.rs        # Input domain handlers
│   │   └── emulation.rs    # Emulation domain handlers
│   └── codegen/            # Generate Rust types from Chrome's protocol.json
│       ├── protocol.json   # Official CDP protocol definition
│       └── generate.rs     # Build script: JSON → Rust structs
├── tests/
│   ├── puppeteer_compat.rs # Run Puppeteer test suite against our server
│   └── playwright_compat.rs
└── Cargo.toml
```

## CDP Wire Format

CDP uses JSON-RPC over WebSocket:

```json
// Request (client → BrowserOxide)
{"id": 1, "method": "Page.navigate", "params": {"url": "https://example.com"}}

// Response (BrowserOxide → client)
{"id": 1, "result": {"frameId": "main", "loaderId": "loader1"}}

// Event (BrowserOxide → client, no id)
{"method": "Page.loadEventFired", "params": {"timestamp": 1234567890.123}}
```

## Key Design Consideration: No Runtime.enable Leak

We use V8 (same as Chrome), but our `Runtime.enable` implementation avoids the detection leak:

1. **We control the CDP server** — We decide what `Runtime.enable` actually does. Chrome's leak happens because `Runtime.enable` triggers V8's console argument serialization (which invokes Proxy traps, error stack getters). Our CDP server can enable runtime event reporting WITHOUT calling V8's `Runtime.enable` internally.
2. **Isolated execution contexts** — All automation code runs in V8 isolated contexts invisible to page scripts (same technique as Patchright/rebrowser-patches).
3. **Console interception at the Rust layer** — We intercept `console.*` calls via V8 ops before they reach the CDP event stream. No V8 preview serialization is triggered.
4. **No unconditional serialization** — Only serialize console arguments when a CDP client explicitly requests it AND the arguments are from the page context (not from automation).

This means `Runtime.enable` is safe by default in BrowserOxide, even though we use V8.
