# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project aims to
follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] — 2026-06-13

> First open-source release of BrowserOxide — a from-scratch stealth headless
> browser engine in Rust: own HTTP/1+2+3 + BoringSSL TLS stack, V8 via
> deno_core, from-scratch CSS/DOM/layout/canvas, configurable browser-identity
> profiles, and a CDP-compatible debugging surface. Dual-licensed MIT OR Apache-2.0.

### Added
- From-scratch browser engine: HTML parser, arena-allocated DOM + Shadow DOM +
  iframes, CSS parser/selectors/values/cascade, layout, and Canvas 2D / WebGL
  rendering — no Chromium, no fork.
- Stealth networking stack: HTTP/1, HTTP/2, and HTTP/3 with Chrome-identical
  TLS ClientHello + HTTP/2 fingerprint via boring2 (Cloudflare BoringSSL fork).
- Native (not injected) browser fingerprint via configurable stealth profiles
  (Chrome 148 / Firefox 135 / Safari 18 desktop + mobile presets), loadable
  from YAML/JSON.
- JavaScript runtime on V8 (deno_core 0.403) with Web-platform APIs, workers,
  and an event loop.
- `ChallengeSolver` trait + `Page::navigate_with_solvers` hook for embedders;
  no per-vendor bypass code ships in the public crate (see `SCOPE.md`).
- Python bindings (PyO3), published to PyPI as `browser-oxide`.
- MCP server (`browser_oxide_mcp`) for AI assistants.
- CDP-compatible debugging/automation surface (Puppeteer/Playwright drop-in).

### Performance
- Single-process architecture: ~60–135 MB peak RSS per page vs a headless-Chrome
  process tree's 1–2 GB — roughly 15× lighter (see [`docs/MEMORY.md`](docs/MEMORY.md)).
- Warm `PagePool` amortizes V8 isolate + snapshot setup across navigations.

### Notes
- Anti-bot corpus: routed 118/126 commercially-protected sites to a real render
  in a same-machine, same-IP cleanroom run, with zero per-vendor bypass code
  (see [`docs/BENCHMARK.md`](docs/BENCHMARK.md)).
- **Python wheels ship for macOS (Apple Silicon + Intel) and Windows.** The Linux
  wheel is deferred to 0.1.1: the prebuilt V8 uses a local-exec TLS model that
  can't link into a `-shared` CPython extension, and a from-source rebuild isn't
  possible from the crates.io `v8` tarball. The Linux package will land via a
  sidecar (engine binary + thin Python client). The Rust crate and the MCP server
  are unaffected and support Linux, macOS, and Windows.

[Unreleased]: https://github.com/yfedoseev/browser_oxide/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/yfedoseev/browser_oxide/releases/tag/v0.1.0
