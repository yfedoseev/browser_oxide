# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project aims to
follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.2]

### Fixed
- **`PagePool` / warm reuse leaked V8 heap without bound**
  ([#33](https://github.com/yfedoseev/browser_oxide/issues/33)). Reusing a
  `Page` across navigations grew V8's live (non-collectable) heap by ~10 MB
  per page, eventually OOMing long batches. Every one of the engine's reapers
  was wired only to `Page::drop`, which a pool by definition never reaches,
  and the bootstrap JS keeps several registries scoped to the `JsRuntime`
  rather than to the document. Now reaped on reuse:
  - all registered event listeners (`__cancelAllListeners()` in
    `event_bootstrap.js`) — `window`-bound listeners were keyed against the
    one object that outlives every navigation, so their closures pinned the
    previous page's entire object graph, and `_nodeListeners` was a strong
    `Map` that was never pruned at all;
  - the DOM node-wrapper cache, scroll state, `MutationObserver` registry,
    and iframe/frame registries (`__resetDomRegistries()`);
  - custom-element definitions (`__resetCustomElements()`);
  - globals the page hung off `window` (`__resetPageGlobals()`), diffed
    against a baseline the engine marks before any page script runs.
- **Warm reuse misfired the previous page's handlers on the new document.**
  `_nodeListeners` and the node-wrapper cache are keyed by `nodeId`, and node
  IDs restart at zero when `replace_dom` swaps the document — so the old
  page's listener for node 42 fired on the new page's node 42, and the new
  page's node could be handed the old page's wrapper (with its expandos).
  Fixed by the same reset.
- **Custom elements could not be re-defined across a warm navigation.**
  `customElements.define()` for a name the *previous* page had registered was
  a silent no-op, so the new page's class never upgraded.
- `Page::navigate_warm` left `__keepLongTimersRefed` set after a challenge
  page, pinning long timers on every subsequent navigation of that `Page`.
- **The CDP protocol server leaked the same way.** `Page.navigate` swaps the
  document with `reload_html` on a `Page` the session keeps alive for its
  whole lifetime, so it accumulated the previous document's state for as long
  as a client stayed connected. It now resets between documents.
- Page-assigned `on*` handlers (`window.onscroll = …`, `document.onclick = …`)
  survived reuse. These already exist as own properties at bootstrap, so a
  key-set diff cannot see the assignment; handler *values* are now snapshotted
  at baseline and restored, which clears page assignments while preserving the
  engine's own `window.onerror` instrumentation.

### Added
- `Page::reset_for_reuse()` — public, bundles every cross-navigation reaper
  (timers, listeners, DOM registries, custom elements, page globals, orphan
  Workers, child iframe isolates). Consumers that hand-roll page reuse — e.g.
  calling `Page::reload_html` on a `Page` they keep alive — should call this
  between documents; `PagePool`, `Page::navigate_warm` and the CDP server
  already do.
- `Page::v8_heap_used_bytes()` and `Page::collect_garbage()` (also on
  `BrowserJsRuntime`) — lets pool operators verify heap health directly.
  Sample after each navigation; a healthy pool stays flat.

### Removed
- Dead `_listeners` registry in `event_bootstrap.js` (declared, never read).

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
