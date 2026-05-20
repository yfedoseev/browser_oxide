# browser_oxide

A headless browser engine written from scratch in Rust. Real HTML/CSS/DOM
parser, V8-backed JS runtime, own CSS engine (not Servo's MPL crates), own
stealth-grade HTTP stack with BoringSSL TLS impersonation, CDP-compatible
remote-debugging surface, no Chromium underneath.

> **Status: research-grade, pre-1.0.** Works against a 126-site corpus of
> commercially-protected pages (see "What it can do" below for measured
> numbers). API surfaces are not stable. License is MIT OR Apache-2.0.

## Why this exists

Every stealth tool today wraps a real browser and hides the puppet strings.
Puppeteer/Playwright + stealth plugins patch ~12 JS properties at runtime
and lose to `Function.prototype.toString` checks. Patched-Chromium forks
still inherit Chromium's CDP detection vectors. Patched-Firefox forks
(Camoufox et al.) have ~3% browser-market-share which is itself a signal.

`browser_oxide` is a different bet: build the engine from the parser up so
the fingerprint properties are *native*, not *injected*. There is no
Chrome process, no CDP client, no WebDriver, no patched-fork inheritance.
Whether that's the *right* bet is empirical — see the numbers section.

## What it is

A complete browser engine for scraping, archival, and AI agent workloads:

- **V8 JavaScript** via `deno_core` 0.311 — full ES2024+, WASM, JIT
- **Arena-allocated DOM** with `NodeId` (Copy, u32) handles, Shadow DOM, iframes
- **Own CSS engine** — Syntax L3 tokenizer, Selectors L4 matcher, cascade,
  `@layer`/`@media`/`@container`, computed styles
- **Real Canvas** — 2D rendering via `tiny-skia`, WebGL stubs, AudioContext
- **Layout** via `taffy` with font metrics for `getBoundingClientRect()`
- **Stealth HTTP** — own TLS stack (`boring2`/BoringSSL), Chrome-matched
  ClientHello, HTTP/1.1 + HTTP/2 (HTTP/3 wired but disabled by default —
  vanilla `quinn-proto` emits randomized transport parameters which is a
  worse fingerprint than not speaking h3)
- **CDP-compatible debugging surface** — drop-in target for Puppeteer/
  Playwright via WebSocket
- **EventSource (SSE)** for streaming endpoints
- **Configurable browser identity** — load profiles from YAML or JSON at
  runtime, or use the built-in `chrome_148_*` / `firefox_135_*` /
  `pixel_9_pro_chrome_148` / `iphone_15_pro_safari_18` presets

## What it can do (measured, not estimated)

Anti-bot coverage measured against a 126-site corpus of commercially-
protected pages (Cloudflare, Akamai, DataDome, PerimeterX, Kasada,
Shape/F5, etc.) on 2026-05-17:

| Profile (per-site routing) | L3-rendered / 126 |
|---|---:|
| Chrome 148 macOS | 117 |
| Pixel 9 Pro Chrome 148 (Android) | 119 |
| iPhone 15 Pro Safari 18 | 113 |
| Firefox 135 macOS | 115 |
| **Per-domain best-of-profile (routed)** | **121** |

**The five sites that no current profile passes** are three Kasada-
protected pages (`canadagoose.com`, `hyatt.com`, `realtor.com`),
`homedepot.com` (Akamai sec-cpt — passes the 3-iter holistic metric, fails
under a strict 1-iter lens), and `iphey.com` (a thin-body fingerprint test
page, debug-build render artifact under contention).

Numbers are debug-build, single-threaded (V8 isolate constraint), from one
datacenter IP, on 2026-05-17. Release builds and clean IPs improve on
these. We do not claim "all of Kasada" — Kasada has a known residual we
have a named (not yet shipped) fix list for. See the engine docs in
`docs/` and the test harness in `crates/browser/tests/` for the
underlying measurements.

### Things to know before believing the numbers

- **Free-OSS SOTA parity, not "we beat Chrome"**: On the broad corpus we
  measure in the same tier as real-browser-driver tools (Camoufox,
  Patchright, nodriver). The point isn't that we win — it's that a
  from-scratch engine reaches that tier at all.
- **No live competitor sweep was run for this README.** Comparison
  numbers cited in older versions of this file (e.g. "Puppeteer 30/71")
  were not freshly re-measured and have been removed.
- **Kasada is the OSS-wide gap.** No open-source tool publicly passes
  Kasada from scratch. The published 2026 winners are paid real-browser
  farms (Scrapfly et al.).
- **Memory and startup numbers** depend heavily on workload and OS — we
  don't have a recent apples-to-apples benchmark we'd defend in this
  README, so they're removed. The benchmark scaffolding lives in
  `crates/browser/tests/browser_comparison.rs`.

## Architecture

```
HTML → DOM (+ Shadow DOM) → CSS → Layout → JS (V8 + WASM) ← Stealth profile
  ↑          ↑                       ↓
  │       iframes              Canvas 2D (tiny-skia)
  │                                  ↓
  └────── HTTP/1+2 (stealth TLS) ────┘
```

15 crates, MIT OR Apache-2.0. No GPL/LGPL/AGPL. The only MPL-2.0 in
the default tree is `cooked-waker`, pulled in transitively via
`deno_core` → `v8` and linked unmodified; MPL-2.0 is file-scope
copyleft so this does not infect downstream code. An optional
`blocker` Cargo feature (off by default) adds Brave's MPL-2.0
`adblock` crate. Both are explicit per-crate exceptions in
`deny.toml`. Full crate inventory in `docs/ARCHITECTURE.md`.

| Crate | Description |
|---|---|
| `css_parser` | CSS Syntax Level 3 tokenizer + parser (with nesting) |
| `css_selectors` | Selectors Level 4 parser + matcher |
| `css_values` | CSS property value parsing + computed values |
| `css_cascade` | Cascade, `@layer`, `@media`, `@container`, inheritance |
| `dom` | Arena DOM + Shadow DOM + iframe contexts |
| `html_parser` | `html5ever` integration → DOM |
| `js_runtime` | V8 (`deno_core`) + DOM bindings + WASM + Web APIs |
| `canvas` | Canvas 2D (`tiny-skia`) + WebGL stubs + AudioContext |
| `layout` | Box model via `taffy` (`getBoundingClientRect`) |
| `net` | HTTP/1+2+3 + stealth TLS (`boring2`/BoringSSL) + WebSocket + SSE |
| `event_loop` | Timers, microtasks, Promises, rAF |
| `workers` | Web Workers + Service Workers (separate V8 isolates) |
| `stealth` | Fingerprint profiles (100+ properties), navigator spoofing |
| `protocol` | CDP server (Puppeteer/Playwright drop-in) |
| `browser` | Top-level `Browser`/`Page` API |
| `akamai` | Akamai BMP sensor payload encoder |

## Quick start

```rust
use browser::Page;

// Built-in preset
let profile = stealth::presets::chrome_148_macos();
let page = Page::navigate_stealth("https://example.com", profile).await?;
println!("{}", page.title());

// Evaluate JavaScript
let result = page.evaluate("document.querySelectorAll('a').length")?;
```

### Configurable browser identity

The browser identity (UA string, Chrome version, screen, locale, TLS
impersonation label, etc.) is a `StealthProfile`. Load one from disk:

```rust
use stealth::StealthProfile;

let profile = StealthProfile::load_from_file("profiles/chrome_148_macos.yaml")?;
profile.validate()?;
let page = Page::navigate_stealth("https://example.com", profile).await?;
```

YAML and JSON are both supported; format is picked by extension. See
`crates/stealth/profiles/chrome_148_macos.yaml` for the full field
schema. The struct definition (`StealthProfile` in `crates/stealth/src/
profile.rs`) is the source of truth — every field is documented there.

### CDP server (Puppeteer/Playwright drop-in)

```rust
use protocol::CdpServer;

let server = CdpServer::start_navigable(9222)?;
// Connect with Puppeteer to ws://127.0.0.1:9222
```

## Build and test

```bash
cargo test --workspace -- --test-threads=1    # V8 isolates are per-thread
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check

# Live anti-bot sweep (needs internet, --release for fair timing)
cargo test --release -p browser --test holistic_sweep -- --ignored --test-threads=1 --nocapture
```

The browser-comparison harness (`crates/browser/tests/browser_comparison
.rs`) compares against locally-installed Chrome / Lightpanda when those
binaries are present; it is `#[ignore]` by default.

## Documentation

| Doc | Description |
|---|---|
| [Architecture](docs/ARCHITECTURE.md) | Workspace layout, dependency graph, external deps |
| [Networking](docs/NETWORKING.md) | HTTP/1+2+3 + stealth TLS + WebSocket + SSE |
| [CDP Protocol](docs/PROTOCOL.md) | Puppeteer/Playwright drop-in surface |
| [CSS Parser](docs/CSS_PARSER.md) / [Selectors](docs/CSS_SELECTORS.md) / [Values](docs/CSS_VALUES.md) / [Cascade](docs/CSS_CASCADE.md) | The CSS engine |
| [DOM](docs/DOM.md) | Arena DOM + Shadow DOM + iframes + Web APIs |
| [JS Runtime](docs/JS_RUNTIME.md) | V8 + `deno_core` + WASM + API surface |
| [Canvas](docs/CANVAS.md) | Canvas 2D + WebGL stubs + AudioContext |
| [Layout](docs/LAYOUT.md) | Box model via `taffy` |
| [Event Loop](docs/EVENT_LOOP.md) | Timers, microtasks, rAF, Promises |
| [Workers](docs/WORKERS.md) | Dedicated/Shared/Service Workers |

## Use, scope, what this is not for

This is engine-side research. The intended use is automated browsing for
archival, accessibility, AI agents, security research, and CTF-style
challenges where you have legitimate authorization to access the target
site. The repository ships an engine, not a "circumvent paywalls" recipe
list; site-specific recipes and reverse-engineering notes are kept in a
private companion repository.

If you build a product on top of this, respect the target site's terms,
robots policy, and rate limits. The maintainer is not responsible for
downstream misuse.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option. Unless explicitly stated otherwise, any contribution
intentionally submitted for inclusion shall be dual-licensed as above,
without any additional terms or conditions.
