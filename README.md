# browser_oxide

The fastest stealth headless browser. Built from scratch in Rust.

## Numbers

| | browser_oxide | Chrome 146 | Puppeteer+Stealth | Camoufox | Lightpanda |
|---|:---:|:---:|:---:|:---:|:---:|
| **Stealth score** | **18/18** | 16/18 | 14/18 | 13/18 | 8/18 |
| **Memory (RSS)** | **34 MB** | 663 MB | 74 MB | ~300 MB | 72 MB |
| **Startup** | **56ms** | — | 3028ms | 1471ms | — |
| **JS eval speed** | **0.05ms** | 0.35ms | — | — | 0.07ms |
| **Page load** | 251ms/pg | 392ms/pg | — | — | 432ms/pg |
| **TLS fingerprint** | Chrome-identical | Chrome | Chrome (real) | Firefox | Zig |
| **Anti-bot (71 sites)** | **71/71** | — | ~30/71 | ~65/71 | ~10/71 |
| **CDP leak** | **None** | Leaks | Leaks | None | — |

All numbers from real measurements, not estimates. Benchmark scripts in `benchmarks/`. Full methodology in [BROWSER_COMPARISON.md](docs/BROWSER_COMPARISON.md).

## Why

Every stealth tool today fights the same losing battle: **wrapping Chrome and hiding the puppet strings**.

- **Puppeteer/Playwright + stealth plugins** patch ~12 JS properties after Chrome launches. Anti-bot systems detect this by inspecting `Function.prototype.toString()` and the prototype chain. Fails Cloudflare, DataDome, Kasada.

- **Patched Chromium forks** (CloakBrowser, BotBrowser) modify Chrome's C++ source. Better, but still inherit all of Chromium's CDP detection vectors — the `Runtime.enable` prototype-chain Proxy leak is deterministic and unpatched as of 2026.

- **Patched Firefox** (Camoufox) avoids CDP by using Juggler protocol. Strong stealth, but Firefox has 3% browser market share — its TLS fingerprint is inherently suspicious to anti-bot systems that weight by rarity.

- **Commercial anti-detect browsers** (Multilogin, Kameleo) charge $45-100+/month and are closed-source desktop apps. Can't embed in a container farm.

**browser_oxide doesn't have puppet strings.** There's no Chrome underneath. No CDP client. No WebDriver. No patched fork. It's a browser engine built from scratch — the stealth properties are native, not injected. Anti-bot systems can't detect what doesn't exist.

The result: **19x less memory than Chrome, 54x faster startup than Puppeteer, and the only tool that passes all 18 stealth checks and all 71 anti-bot test sites.**

## What It Is

A complete headless browser engine for web scraping and AI agents:

- **V8 JavaScript** — full ES2024+, WASM, JIT compilation (same V8 as Chrome, via deno_core)
- **Real DOM** — arena-allocated, Shadow DOM, iframes with separate V8 contexts
- **Real CSS** — our own parser (not Servo's MPL crates), cascade, computed styles, media queries
- **Real Canvas** — 2D rendering via tiny-skia, WebGL stubs, AudioContext fingerprints
- **Real Layout** — flexbox/grid via taffy, `getBoundingClientRect()` with font metrics
- **Stealth HTTP** — own TLS stack (boring2/BoringSSL), Chrome-identical JA4 fingerprint, HTTP/1.1 + HTTP/2 + HTTP/3 (QUIC)
- **CDP compatible** — drop-in replacement for Puppeteer/Playwright via WebSocket
- **EventSource (SSE)** — crawl LLM agent APIs that stream responses
- **Human-like input** — Bezier curve mouse movements, variable typing speed

## Architecture

```
HTML → DOM (+ Shadow DOM) → CSS → Layout → JS (V8 + WASM) ← Stealth profiles
  ↑          ↑                       ↓
  │       iframes              Canvas 2D (tiny-skia)
  │                                  ↓
  └────── HTTP/1+2+3 (stealth TLS) ──┘
```

15 crates, all MIT/Apache-2.0. No MPL, no AGPL, no copyleft.

## Quick Start

```rust
use browser::Page;
use stealth::chrome_130_linux;

// Load a page with stealth
let profile = chrome_130_linux();
let page = Page::navigate_stealth("https://example.com", profile).await?;
println!("{}", page.title()); // "Example Domain"

// Evaluate JavaScript
let result = page.evaluate("document.querySelectorAll('a').length")?;

// Human-like interaction
page.human_click("button.submit")?;
page.human_type("input[name=email]", "user@example.com")?;
```

```rust
// CDP server — connect with Puppeteer/Playwright
use protocol::CdpServer;

let server = CdpServer::start_navigable(9222)?;
// Connect: ws://127.0.0.1:9222
// Navigate via Page.navigate, evaluate via Runtime.evaluate
```

## Anti-Bot Coverage

Tested against 71 real protected sites. All pass.

| Protection | Sites Tested | Result |
|---|---|:---:|
| **Cloudflare** | nowsecure.nl, chatgpt.com, discord.com, medium.com, coinbase.com, bet365.com | **6/6** |
| **Akamai** | adidas.com, costco.com, delta.com, homedepot.com, nike.com, united.com | **6/6** |
| **PerimeterX** | walmart.com, stockx.com, nordstrom.com, instacart.com, craigslist.org | **5/5** |
| **Kasada** | ticketmaster.com, ticketmaster.co.uk, seatgeek.com | **3/3** |
| **Shape/F5** | southwest.com, iherb.com, gap.com | **3/3** |
| **DataDome** | various sites via challenge solver | **pass** |
| **Fingerprint checks** | sannysoft, creepjs, browserleaks, pixelscan | **4/4** |

## Stealth Features

18 detection vectors covered (Chrome headless only covers 16):

| Feature | How |
|---|---|
| TLS fingerprint (JA3/JA4) | Own BoringSSL stack, Chrome 130 cipher suites/curves/extensions |
| HTTP/2 fingerprint | Chrome SETTINGS frame order, pseudo-header order, priority |
| `navigator.webdriver` | `undefined` (Chrome leaks `true`) |
| `window.chrome` | Full chrome object with runtime, loadTimes, csi |
| Plugins, mimeTypes | Spoofed with correct counts |
| WebRTC | Leak-proof RTCPeerConnection (no real IP exposure) |
| Canvas/WebGL | Seed-based deterministic fingerprints |
| AudioContext | Seed-based deterministic fingerprint |
| Font enumeration | OS-specific font lists (Windows/Mac/Linux) |
| Speech synthesis | OS-specific voice lists |
| Permissions API | Chrome-consistent responses per permission type |
| Battery API | Realistic BatteryManager |
| Media codecs | Chrome-correct `isTypeSupported()` / `canPlayType()` |
| Client Hints | Full sec-ch-ua headers matching JS APIs |
| CDP detection | **None** — not a Chromium fork, no `Runtime.enable` leak |
| Human-like input | Bezier mouse curves, variable typing speed |

## Performance

**Memory: 19x less than Chrome**

```
Browser              Idle       After 10 pages    Growth
browser_oxide        34 MB      34 MB             +0 MB
Chrome 146          663 MB     666 MB             +3 MB
Lightpanda           72 MB      74 MB             +2 MB
```

At 1,000 instances on AWS: browser_oxide uses 34 GB vs Chrome's 660 GB. **~$40K/month savings.**

**Speed: 7x faster JS, 1.6x faster page loading**

```
                     JS eval      Throughput (11 pages)
browser_oxide        0.05ms/call  251ms/page
Chrome 146           0.35ms/call  392ms/page
Lightpanda           0.07ms/call  432ms/page
```

## Crates

| Crate | Description |
|---|---|
| `css_parser` | CSS Syntax Level 3 tokenizer + parser (with nesting) |
| `css_selectors` | Selectors Level 4 parser + matcher |
| `css_values` | CSS property value parsing + computed values |
| `css_cascade` | Cascade, @layer, @media, @container, inheritance |
| `dom` | Mutable DOM + Shadow DOM + iframe contexts |
| `html_parser` | html5ever integration → DOM |
| `js_runtime` | V8 (deno_core) + DOM bindings + WASM + all Web APIs |
| `canvas` | Canvas 2D (tiny-skia) + WebGL stubs + AudioContext |
| `layout` | Box model via taffy (getBoundingClientRect) |
| `net` | HTTP/1+2+3 + stealth TLS (boring2/BoringSSL) + WebSocket + SSE |
| `event_loop` | Timers, microtasks, Promises, rAF |
| `workers` | Web Workers + Service Workers (separate V8 isolates) |
| `stealth` | Fingerprint profiles (100+ properties), navigator spoofing |
| `protocol` | CDP server (Puppeteer/Playwright drop-in) |
| `browser` | Top-level Browser/Page API |

## Build & Test

```bash
cargo test --workspace -- --test-threads=1   # 801 tests, V8 requires single-threaded
cargo clippy --workspace -- -D warnings      # Lint
cargo fmt --all -- --check                   # Format

# Browser comparison (needs Chrome + Lightpanda running)
cargo test --release -p browser --test browser_comparison -- --ignored --test-threads=1 --nocapture

# Anti-bot sites (needs internet)
cargo test --release -p browser --test anti_bot_sites -- --ignored --test-threads=1 --nocapture

# Competitor benchmarks
cd benchmarks && node bench_puppeteer.js     # Puppeteer+Stealth
python bench_all.py                          # Patchright, Camoufox
```

## Documentation

| Doc | Description |
|---|---|
| [Browser Comparison](docs/BROWSER_COMPARISON.md) | Head-to-head benchmarks vs Chrome, Lightpanda, Puppeteer, Camoufox, and 6 others |
| [Architecture](docs/ARCHITECTURE.md) | Workspace, dependency graph, external deps |
| [Networking](docs/NETWORKING.md) | HTTP/1+2+3 + stealth TLS + WebSocket + SSE |
| [Stealth](docs/STEALTH.md) | 100+ profile properties, navigator, window.chrome |
| [CDP Protocol](docs/PROTOCOL.md) | Puppeteer/Playwright compatibility |
| [CSS Parser](docs/CSS_PARSER.md) | Tokenizer + parser + nesting |
| [DOM](docs/DOM.md) | Arena DOM + Shadow DOM + iframes + Web APIs |
| [JS Runtime](docs/JS_RUNTIME.md) | V8 + deno_core + WASM + full API surface |
| [Canvas](docs/CANVAS.md) | Canvas 2D (tiny-skia) + WebGL stubs + AudioContext |

## License

MIT OR Apache-2.0
