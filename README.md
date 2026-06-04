# browser_oxide — stealth headless browser engine in Rust (anti-bot, from scratch, no Chromium)

*A from-scratch stealth browser engine in Rust — own BoringSSL TLS/JA4 fingerprint, real JS, no Chromium, no CDP. Python + MCP bindings.*

[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)]()
[![Built with Rust](https://img.shields.io/badge/built%20with-Rust-orange.svg)]()
[![Status: pre-1.0](https://img.shields.io/badge/status-research--grade%20pre--1.0-yellow.svg)]()
[![Anti-bot corpus: routed 118/126](https://img.shields.io/badge/anti--bot%20corpus-routed%20118%2F126-brightgreen.svg)]()
[![Bindings: Rust · Python · MCP](https://img.shields.io/badge/bindings-Rust%20%C2%B7%20Python%20%C2%B7%20MCP-informational.svg)]()

**browser_oxide is a stealth headless browser engine written from scratch in
Rust** for web scraping, archival, and AI agents. It implements a real
HTML/CSS/DOM/JS browser — including its own BoringSSL TLS stack and a *native*
(not injected) browser fingerprint — with **no Chromium and no Chrome DevTools
Protocol (CDP) driver** underneath. You control every surface from the TLS
handshake through WASM to canvas, so the fingerprint is native rather than
injected.

> **TL;DR.** Every other native-fingerprint stealth browser is either Python-only
> (camoufox, a Firefox fork) or drives Chrome over CDP (nodriver, Obscura), where
> the automation is itself detectable. **As of 2026 there is no Rust equivalent to
> camoufox** — browser_oxide is built to be exactly that. In a same-machine,
> same-IP cleanroom run it routed **118 of 126** commercially-protected sites
> (Cloudflare, Akamai, DataDome, PerimeterX, Kasada) to a real render with **zero
> per-vendor bypass code**. Kasada is the one honest open gap.

**Get started:** [Rust](docs/getting-started-rust.md) · [Python](docs/getting-started-python.md) (`pip install browser-oxide`) · [MCP for AI agents](#mcp-server-for-ai-agents) · [Benchmark](docs/BENCHMARK.md) · [How it compares](#how-it-compares)

> **Status: research-grade, pre-1.0.** API surfaces are not stable. License is MIT OR Apache-2.0.

## Why this exists

Most stealth tooling today wraps a real browser and hides the puppet strings.
CDP-driver stealth plugins patch a handful of JS properties at runtime and
lose to `Function.prototype.toString` checks. Patched-Chromium forks still
inherit Chromium's CDP detection vectors. Patched-Firefox forks ride a
browser engine with low market share, which is itself a fingerprint signal.

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

## How it compares

The native-fingerprint stealth-browser field, and where browser_oxide sits:

| Tool | Language | Engine | No CDP/WebDriver | Native fingerprint | Runs JS | TLS impersonation |
|---|---|---|:--:|:--:|:--:|:--:|
| **browser_oxide** | **Rust (+ Python, MCP)** | **from-scratch (own HTML/CSS/DOM/JS + BoringSSL TLS)** | ✅ | ✅ | ✅ (V8) | ✅ (JA3/JA4) |
| camoufox | Python | Firefox fork | ✅ | ✅ (patched into Firefox) | ✅ (Gecko) | partial (Firefox NSS) |
| nodriver / Obscura | Python | real Chrome (CDP) | ❌ (uses CDP) | ❌ (injected) | ✅ (Chrome) | ❌ (real Chrome TLS) |
| undetected-chromedriver | Python | real Chrome (WebDriver) | ❌ (WebDriver) | ❌ (injected) | ✅ (Chrome) | ❌ |
| curl_cffi | Python | none (HTTP/TLS only) | n/a | n/a | ❌ | ✅ (JA3/JA4) |

**The takeaway:** camoufox proved native-fingerprint stealth works, but it's a
Python-only Firefox fork. CDP-driven tools (nodriver, Obscura) inherit Chrome's
automation-detection surface. curl_cffi nails the TLS handshake but runs no
JavaScript, so it loses to any JS challenge. **As of 2026 there is no Rust
equivalent to camoufox** — browser_oxide fills that gap: native fingerprint, no
CDP, real JS, and a Rust core with Python + MCP bindings.

## What it can do (measured, not estimated)

Anti-bot coverage measured against a 126-site corpus of commercially-
protected pages (Cloudflare, Akamai, DataDome, PerimeterX, Kasada,
Shape/F5, etc.), release build. These numbers are from the
vendor-stripped open-source engine — no per-vendor bypass code in the
tree. Same machine, same IP, same hour, same classifier
(`browser::engine_classify`).

| browser_oxide profile        | **Pass** (real render, ≥15 KB) | loose `L3` tag |
|------------------------------|--:|--:|
| `chrome_148_macos`           | **114** | 118 |
| `firefox_135_macos`          | **111** | 115 |
| `pixel_9_pro_chrome_148`     | **114** | 118 |
| `iphone_15_pro_safari_18`    | **118** | 121 |
| **best-of-4 routed**         | **118** | 121 |

The headline `Pass` column is the honest gate: the engine's `L3-RENDERED`
tag **and** `≥15 KB` of actual content (`ChallengeVerdict::Pass`, the rule
the engine's own audit harness uses). The loose column counts the
`L3-RENDERED` tag alone — it over-counts by ~4, because the corpus has
10–15 SPA-bootstrap sites that ship a 2–13 KB shell to *any* HTTP client
and get tagged `L3-RENDERED` by the *absence* of a challenge marker even
though no real render happened (e.g. `duolingo` returns a 13.5 KB shell —
**not** a pass). "Routed" = the caller picks the best profile per domain,
which most real scraping pipelines do naturally. Full breakdown and the
scoring caveats (the strict gate also has a few false *negatives*) are in
[`docs/BENCHMARK.md`](docs/BENCHMARK.md).

(Profile labels reflect the actual emitted User-Agent. All presets ship a
current Chrome 148 / Firefox 135 / Safari 18 identity.)

**The hard residual** is seven sites that returned no real content on any
profile: three Kasada pages (`canadagoose.com`, `hyatt.com`,
`realtor.com` — no OSS tool publicly passes Kasada from scratch),
`etsy.com` (DataDome interactive Device-Check, human-gated, out of
scope), `duolingo.com` (CSR SPA that only reaches its shell),
`wildberries.ru` (WBAAS), and `homedepot.com` (Akamai sec-cpt — flaky;
renders on a good risk-roll). `adidas.com` is *not* in this set — it
passes via routing (chrome + iphone render the 1.5 MB storefront; firefox
+ pixel stay at the interstitial). `adidas`/`homedepot` are both flaky
Akamai, so the exact residual shifts ±1–2 sites run to run.

> **The engine carries the number, not bypass code.** A/B measurements
> with per-vendor challenge solvers enabled vs fully removed from the
> tree show no difference in routed pass rate. Every site that renders,
> renders on the from-scratch TLS + fingerprint + V8 engine alone. The
> open-source engine ships no solver implementations (see "Challenge
> solving" below).

### Things to know before believing the numbers

- **Anti-bot responses are noisy.** Single sweep runs vary by ±5 sites
  from WAF lottery alone (measured per-site 3× re-tests: amazon variants
  have ~1-in-3 pass rate per fetch). The routed `118` is the central
  tendency of one cleanroom run, not a guaranteed per-run result.
- **Kasada is the OSS-wide gap.** No open-source tool publicly passes
  Kasada from scratch.

### Per-page performance

A single Rust process (not Chrome over CDP), so resident memory stays in
the tens of MB. Per-page wall-clock, 5-run median, same box, single IP,
warm binary cache:

| Path | example.com (528 B) | hacker news (~35 KB) | wikipedia (~230 KB) |
|---|--:|--:|--:|
| cold (`Page::navigate`) | 244 ms | 444 ms | 849 ms |
| **warm pool (`PagePool::navigate`)** | **141 ms** | **333 ms** | **724 ms** |

A warm `PagePool` amortizes V8 isolate + snapshot setup across
navigations (the `PagePool::navigate(url)` API).

Full per-profile + per-site breakdown: [`docs/BENCHMARK.md`](docs/BENCHMARK.md).

## FAQ

### Is there a Rust equivalent to camoufox?
browser_oxide is built to be exactly that. camoufox is a native-fingerprint
stealth browser, but it's a Python-only Firefox fork. As of 2026 there is no other
from-scratch stealth browser engine in Rust — browser_oxide provides a native
fingerprint, no CDP, real JavaScript execution, and a Rust core with Python and
MCP bindings.

### Is browser_oxide a Chromium or Firefox fork?
No. The HTML parser, CSS engine, DOM, layout, and TLS stack are written from
scratch in Rust; JavaScript runs on V8 via `deno_core`. There is no Chrome
process and no CDP driver, so it doesn't inherit Chromium's automation-detection
vectors (`navigator.webdriver`, `cdc_*` variables, CDP WebSocket fingerprints).

### How is it different from nodriver, Obscura, or undetected-chromedriver?
Those drive a real Chrome over CDP or WebDriver. browser_oxide is its own engine
— no automation protocol underneath — so the fingerprint is native rather than
patched onto an automated Chrome that vendors can detect.

### How is it different from curl_cffi?
curl_cffi impersonates a browser's TLS handshake but doesn't run JavaScript, so it
fails any site needing JS or a real DOM. browser_oxide ships a full V8 runtime,
real DOM/CSS/layout/canvas **and** a Chrome-matched TLS fingerprint.

### Does it pass Cloudflare, Akamai, and DataDome?
In a measured 126-site cleanroom run it routed 118/126 commercially-protected
sites to a real render, including most Cloudflare, Akamai, AWS WAF, and DataDome
pages — with no per-vendor bypass code. DataDome's interactive Device-Check
(human-gated, e.g. etsy.com) is not cleared.

### Does it pass Kasada?
No. Kasada (e.g. canadagoose.com, hyatt.com, realtor.com) is the standing open
gap; no open-source tool publicly passes Kasada from scratch, and browser_oxide
ships no Kasada solver.

### Can I use it from Python?
Yes — `pip install browser-oxide`, then `Browser` / `Page` / `Profile` / `Verdict`
(the GIL is released during navigation). There's also an MCP server so AI agents
can drive the engine. See [docs/getting-started-python.md](docs/getting-started-python.md).

### Does it work with Puppeteer or Playwright?
Yes, via a CDP-compatible WebSocket server (`CdpServer::start_navigable`) as a
drop-in target — but nothing drives a real browser underneath. There is no
Selenium/WebDriver support.

### Is it production-ready?
It's research-grade and pre-1.0; APIs are not stable and the crates aren't on
crates.io yet. Anti-bot pass rates are point-in-time and vary ±5 sites per run
from WAF noise. MIT OR Apache-2.0.

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

The engine is `!Send` (V8 isolates are per-thread), so run it on a current-thread
runtime + `LocalSet`:

```rust
use browser::{ChallengeVerdict, Page};

fn main() {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    tokio::task::LocalSet::new().block_on(&rt, async {
        let profile = stealth::presets::chrome_148_macos();   // built-in identity
        let mut page = Page::navigate("https://example.com", profile, 5).await.unwrap();

        println!("{}", page.title());
        println!("{}", page.evaluate("document.querySelectorAll('a').length").unwrap());

        // challenge pages return HTTP 200 — trust the verdict, not the status:
        if page.challenge_verdict() == ChallengeVerdict::Pass {
            println!("real content rendered");
        }
    });
}
```

Runnable: `cargo run --release -p browser --example getting_started -- https://example.com`.
Full walkthrough: [docs/getting-started-rust.md](docs/getting-started-rust.md).

### Configurable browser identity

The browser identity (UA string, Chrome version, screen, locale, TLS
impersonation label, etc.) is a `StealthProfile`. Load one from disk:

```rust
use stealth::StealthProfile;

let profile = StealthProfile::load_from_file("profiles/chrome_148_macos.yaml")?;
profile.validate()?;
let mut page = Page::navigate("https://example.com", profile, 5).await?;
```

YAML and JSON are both supported; format is picked by extension. See
`crates/stealth/profiles/chrome_148_macos.yaml` for the full field
schema. The struct definition (`StealthProfile` in `crates/stealth/src/
profile.rs`) is the source of truth — every field is documented there.

### Python

```python
from browser_oxide import Browser, Profile, Verdict

with Browser(profile=Profile.chrome()) as b:
    page = b.navigate("https://example.com")
    print(page.title, len(page.html), page.verdict)
    if page.verdict == Verdict.PASS:
        print(page.evaluate("navigator.userAgent"))
```

`pip install browser-oxide` (or `maturin develop` from source). Full guide:
[docs/getting-started-python.md](docs/getting-started-python.md).

### MCP server (for AI agents)

A Model Context Protocol server (`browser-oxide-mcp`) lets an AI agent drive the
stealth engine — tools: `fetch_page`, `evaluate`, and `check_protection` (*"is
this URL behind Akamai/DataDome/Kasada, and did a real render get through?"*).

```json
{ "mcpServers": { "browser-oxide": { "command": "browser-oxide-mcp" } } }
```

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

**Using it**

| Guide | Description |
|---|---|
| [Getting started (Rust)](docs/getting-started-rust.md) | Install, navigate, read the page, verdicts, pooling |
| [Getting started (Python)](docs/getting-started-python.md) | `pip install browser-oxide`; the `Browser`/`Page`/`Profile` API |
| [Profiles](docs/guides/PROFILES.md) | Choosing & customizing browser identities; routing |
| [Challenges](docs/guides/CHALLENGES.md) | Verdict semantics + the `ChallengeSolver` extension point |
| [Stealth FAQ](docs/guides/STEALTH_FAQ.md) | What's native vs. not — the honest boundary |
| [Debugging](docs/guides/DEBUGGING.md) | Thin renders, fetch logs, JS errors, the probes |
| [CDP server](docs/guides/CDP.md) | Puppeteer/Playwright drop-in |
| [Benchmark](docs/BENCHMARK.md) | Measured anti-bot pass rates |

**Engine internals**

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
