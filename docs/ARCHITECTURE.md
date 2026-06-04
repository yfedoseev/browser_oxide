# BrowserOxide Architecture

A from-scratch headless browser engine in Rust. Stealth by design, MIT/Apache-2.0 licensed.

Designed to render through 2026-class anti-bot stacks (Cloudflare, Akamai,
AWS WAF, DataDome, HUMAN/PerimeterX) from a from-scratch TLS + fingerprint +
V8 engine — no per-vendor bypass code in the tree. For *measured* per-protection
results (routed 118/126, and the Kasada/DataDome residual that does **not** pass),
see [`BENCHMARK.md`](BENCHMARK.md). Kasada is the standing open gap — no
open-source tool publicly passes it from scratch.

## Why

Every existing approach to stealth web scraping is fundamentally flawed:

- **Chrome + CDP** (Puppeteer, Playwright, chromiumoxide): Controlling someone else's browser means fighting an endless war against detection vectors — `Runtime.enable` leaks, `navigator.webdriver`, `cdc_*` variables, CDP WebSocket fingerprints. You're patching a system designed to be detectable.
- **Servo**: 14 years in, still not production-ready. MPL-2.0 license on critical CSS components.
- **Lightpanda** (Zig): Proves the concept works (11x faster than Chrome) but is AGPL-3.0 — incompatible with MIT. Cannot pass canvas/WebGL challenges.

BrowserOxide is the missing piece: a **Rust-native headless browser** where stealth isn't bolted on — it's the default, because you control every API surface from TLS handshake through WASM execution to canvas rendering.

## Design Principles

1. **Zero detection surface by default** — No automation artifacts exist unless explicitly added
2. **Minimal rendering** — No full GPU pipeline, but real Canvas 2D rendering (via tiny-skia/skia) and WebGL parameter stubs for fingerprint challenges
3. **100% MIT/Apache-2.0** — Every component, including CSS parser and selectors, is permissively licensed
4. **V8-powered** — Full ES2024+, WebAssembly, and JIT performance via rusty_v8 (MIT). Required for Cloudflare Turnstile WASM challenges and heavy SPA bundles
5. **Composable crates** — Each component is a standalone crate usable outside BrowserOxide
6. **Anti-bot by design** — Built against 2026-class detection: JA4 TLS, HTTP/2 frames, WASM proof-of-work, canvas rendering verification, behavioral ML (measured results in [`BENCHMARK.md`](BENCHMARK.md))

## Workspace Structure

```
browser_oxide/
├── Cargo.toml                    # Workspace root
├── crates/
│   ├── css_parser/               # CSS Syntax Level 3 tokenizer + parser (+ nesting)
│   ├── css_selectors/            # Selectors Level 4 parser + matcher
│   ├── css_values/               # CSS property value parsing + computed values
│   ├── css_cascade/              # Cascade, specificity, inheritance, @layer, @media
│   ├── dom/                      # Mutable DOM tree + Web API interfaces + Shadow DOM
│   ├── html_parser/              # html5ever integration + TreeSink → DOM
│   ├── js_runtime/               # V8 engine (rusty_v8) + DOM bindings + WASM
│   ├── canvas/                   # Canvas 2D API (tiny-skia backend) + WebGL stubs
│   ├── layout/                   # Box model via taffy (getBoundingClientRect)
│   ├── net/                      # HTTP/1.1 + HTTP/2 + HTTP/3 + stealth TLS + cookies
│   ├── event_loop/               # Timers, microtasks, Promises, rAF
│   ├── workers/                  # Web Workers + Service Workers (separate V8 isolates)
│   ├── stealth/                  # Fingerprint profiles + navigator spoofing
│   ├── protocol/                 # CDP server (Puppeteer/Playwright compat)
│   └── browser/                  # Top-level Browser/Page/Context + iframes
├── tests/                        # Integration tests
├── benches/                      # Benchmarks vs Chrome headless, Lightpanda
└── docs/                         # This documentation
```

## Crate Dependency Graph

```
                       ┌──────────┐
                       │ browser  │  ← top-level API + iframe contexts
                       └────┬─────┘
                            │
         ┌──────────────────┼──────────────────┐
         │                  │                  │
    ┌────▼────┐   ┌────────▼────────┐  ┌──────▼──────┐
    │protocol ��   │   js_runtime    │  │   stealth   │
    │  (CDP)  │   │  (V8 + WASM)   │  └─────────────┘
    └─────────┘   └────────┬────────┘
                           │
              ┌────────────┼──────────────┬────────────┐
              │            │              │            │
        ┌─────▼──┐   ┌────▼───┐   ┌─��────▼─────┐ ┌───▼─────┐
        │  dom   │   │ canvas │   │ event_loop │ │ workers │
        │+shadow │   │(skia)  │   └────────────┘ └─────────┘
        └───┬────┘   └────────┘
            │
   ┌────────┼──────────┐
   │        │          │
┌──▼─────┐┌─▼────┐┌───▼──────┐
│html    ││css   ││css       │
│_parser ││_sel. ││_cascade  │
└────────┘└──────┘└───┬──────┘
                      │
               ┌──────▼──────┐
               │ css_values  ���
               └──────┬──────┘
                      │
               ┌──────▼──────┐
               │ css_parser  │
               └─────────────┘

    ┌─────────────┐   ┌────────┐
    │ net         │   │ layout │
    │ (HTTP/1+2+3)│   │(taffy) │
    └─────────────┘   └────────┘
```

## External Dependencies

| Crate | License | Purpose | Layer |
|---|---|---|---|
| `html5ever` | MIT/Apache-2.0 | HTML5 spec-compliant parser | html_parser |
| `rusty_v8` | MIT | V8 JavaScript engine bindings | js_runtime |
| `deno_core` | MIT | V8 ops layer + event loop + module loader | js_runtime |
| `rquest` | Apache-2.0 | HTTP/1.1+2 client + BoringSSL TLS impersonation | net |
| `quinn` | MIT/Apache-2.0 | QUIC transport (pure Rust) | net |
| `h3` + `h3-quinn` | MIT | HTTP/3 client | net |
| `tiny-skia` | MIT/Apache-2.0 | CPU-based 2D rendering (Canvas API backend) | canvas |
| `taffy` | MIT | Flexbox/Grid layout computation | layout |
| `fontdb` | MIT | Font database (find system fonts) | layout, canvas |
| `rustybuzz` | MIT | Text shaping (glyph advances) | layout, canvas |
| `cosmic-text` | MIT/Apache-2.0 | Text layout + rendering | canvas |
| `tokio` | MIT | Async runtime | all |
| `tokio-tungstenite` | MIT | WebSocket (CDP server + client API) | protocol, net |
| `cookie_store` | MIT/Apache-2.0 | RFC 6265 cookie jar | net |
| `serde` / `serde_json` | MIT/Apache-2.0 | Serialization | all |

**No GPL/LGPL/AGPL dependencies.** CSS parsing, selectors, values, and cascade are all implemented from scratch — we do not pull in Servo's MPL crates. The only MPL-2.0 in the default tree is `cooked-waker`, pulled in transitively via `deno_core` → `v8` and linked unmodified; MPL-2.0 is file-scope copyleft so this does not infect downstream code. An optional `blocker` Cargo feature (off by default) on the `net` crate adds Brave's MPL-2.0 `adblock` crate. Both are tracked as explicit per-crate exceptions in `deny.toml`.

## Anti-Bot Detection Coverage

Engine mechanism vs. **measured** outcome on the 126-site corpus (see
[`BENCHMARK.md`](BENCHMARK.md) for the full table and caveats). "Mostly" =
passes on at least one routed profile for most corpus sites of that class;
anti-bot responses are noisy (±1–2 sites/run from WAF lottery).

| Anti-Bot System | Detection Method | Engine mechanism | Measured |
|---|---|---|---|
| **Cloudflare Turnstile** | WASM PoW + canvas render + env checks | V8 runs WASM natively; tiny-skia renders canvas; native env APIs | mostly passes |
| **Cloudflare Managed** | JA4 TLS + HTTP/2 frames + JS fingerprint | BoringSSL TLS; correct HTTP/2 SETTINGS; clean JS surface | mostly passes |
| **Akamai** (BMP/sec-cpt) | 150+ sensor signals + timing + rendering | Full navigator/window surface; correct `performance.now()` | mostly passes; flaky tail (adidas, homedepot) |
| **AWS WAF** | token challenge + cookie handshake | challenge.js runs inline; cookie persistence | mostly passes |
| **DataDome** | canvas + behavioral ML + device graph | real canvas; consistent profiles; FF-TLS arm | partial — interactive Device-Check (etsy) is human-gated, does **not** pass |
| **HUMAN/PerimeterX** | prototype integrity + iframe isolation + behavioral | native `toString`; iframe execution; behavior sim | partial |
| **Kasada** | polymorphic WASM challenges + timing | V8 WASM at native speed; no instrumentation | **open gap** — canadagoose/hyatt/realtor do **not** pass (no OSS tool does, from scratch) |
