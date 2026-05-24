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
  `pixel_9_pro_chrome_147` / `iphone_15_pro_safari_18` presets (the
  `pixel_9_pro_chrome_147` constructor emits a current Chrome 148 UA)

## What it can do (measured, not estimated)

Anti-bot coverage measured against a 126-site corpus of commercially-
protected pages (Cloudflare, Akamai, DataDome, PerimeterX, Kasada,
Shape/F5, etc.), release build, 2026-05-23. **These numbers are from
the vendor-stripped open-source engine** — no per-vendor bypass code
in the tree. Same machine, same IP, same hour, same classifier
(`browser::engine_classify`) across browser_oxide and every
competitor.

| Engine                            | **Pass** (real render, ≥15 KB) | L3-tag (loose) |
|-----------------------------------|--:|--:|
| Chromium headless (vanilla)       | 86 | 97 |
| Playwright + Stealth              | 87 | 97 |
| Patchright (CDP-hidden)           | 86 | 97 |
| **browser_oxide chrome_148_macos**       | **102** | **116** |
| **browser_oxide pixel_9_pro_chrome_148** | **104** | **119** |
| **browser_oxide iphone_15_pro_safari_18**| **106** | **120** |
| **browser_oxide firefox_135_macos**      | **101** | **115** |
| Camoufox (Firefox-based)          | 108 | 118 |
| **browser_oxide best-of-4 routed**       | **110** | **122** |

Two numbers per row because the corpus contains 10–15 SPA-bootstrap
sites (amazon stubs, imdb, booking, …) that ship a 2–13 KB shell to
*any* non-real-browser HTTP client — they get tagged `L3-RENDERED` by
absence of a challenge marker but the body isn't a real render. The
strict `Pass` column requires `≥15 KB` of actual content
(`ChallengeVerdict::Pass`, the rule the engine's own audit harness
uses). `L3-tag` is the loose count for compatibility with prior
reports.

(Built-in preset constructors `chrome_130_*` / `pixel_9_pro_chrome_147`
are deprecated aliases that emit a current Chrome 148 UA — the profile
labels above reflect the actual emitted User-Agent, not the legacy
function name.)

**The hard residual** is exactly three Kasada-protected pages —
`canadagoose.com`, `hyatt.com`, `realtor.com`. DataDome's interactive
captcha pages (`yelp.com`, `etsy.com`) are human-gated and out of
scope; they pass on some profiles and block on others.

> **The engine carries the number, not bypass code.** Earlier A/B
> measurements with per-vendor challenge solvers enabled vs fully
> removed from the tree show no difference in routed pass rate. Every
> site that renders, renders on the from-scratch TLS + fingerprint +
> V8 engine alone. The open-source engine ships no solver
> implementations (see "Challenge solving" below).

### Things to know before believing the numbers

- **Best single-profile result trails Camoufox by 2 Pass** (iphone 106
  vs Camoufox 108). When the caller is free to pick the best profile
  per domain (the routed row above), browser_oxide takes the lead by
  +2 Pass / +4 L3-tag. Most real scraping pipelines do this naturally.
- **Clear lead over the CDP-driver tier**: Chromium headless,
  Playwright + Stealth, and Patchright all sit at 86–87 Pass with
  ~25 CHL per engine (anti-bot vendors detect their CDP-driver
  fingerprint regardless of stealth plugins). browser_oxide routed
  shows 3 routed CHL — almost 8× fewer challenges hit us.
- **Anti-bot responses are noisy.** Single sweep runs vary by ±5
  sites from WAF lottery alone (measured per-site 3× re-tests: amazon
  variants have ~1-in-3 pass rate per fetch on any engine). The
  routed `110` number is the central tendency, not a guaranteed
  per-run result. See `docs/BENCHMARK_2026_05_23.md` and
  `docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md` for the full per-site
  breakdown and reproduction commands.
- **Kasada is the OSS-wide gap.** No open-source tool publicly passes
  Kasada from scratch. The published 2026 winners are paid
  real-browser farms (Scrapfly et al.).

## Challenge solving

The engine exposes a `ChallengeSolver` trait + a
`Page::navigate_with_solvers(url, profile, n, solvers)` entry point so
embedders can plug in per-vendor challenge handling (Akamai BMP
sensor_data, Kasada PoW, DataDome interstitial round-trip, Cloudflare
orchestrator). **The open-source engine ships no solver
implementations** — `Page::navigate` registers an empty set, so a
challenged page resolves to `ChallengeVerdict::ChallengeIncomplete`
rather than being auto-cleared. This is deliberate (see `SCOPE.md`):
site-specific bypass code is out of scope here, and — per the A/B
measurement above — it isn't what produces the corpus pass rate
anyway.

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
| `browser` | Top-level `Browser`/`Page` API + `ChallengeSolver` trait |

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
| [Stealth profiles](docs/STEALTH.md) | Custom browser identity — fields, loading from YAML/JSON, consistency rules |
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
