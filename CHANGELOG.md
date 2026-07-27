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

### Dependencies
Closes [#32](https://github.com/yfedoseev/browser_oxide/issues/32) and
supersedes the open Dependabot PRs
([#22](https://github.com/yfedoseev/browser_oxide/pull/22)–[#31](https://github.com/yfedoseev/browser_oxide/pull/31)),
whose commits are cherry-picked here with authorship preserved.

- `deno_core` 0.403 → **0.404**. 0.408 was tried and reverted: it builds and
  passes the full suite in release, but **aborts (SIGABRT) during V8 isolate
  construction in debug builds on Linux** — `basic_js_execution`, which only
  builds a runtime and evaluates `1 + 2`, dies before printing a result. The
  bump needs a debug repro before it can land.
- `taffy` 0.8 → **0.12** (adds safe-alignment keywords).
- `skia-safe` 0.97 → **0.99**, `tokio-tungstenite` 0.27 → **0.30**,
  `webpki-root-certs` 0.26 → **1.0**, `brotli` 7 → **8**, `base64` 0.22 →
  **0.23**, `glow` 0.17 → **0.18** (behind the `webgl-render` feature).
- **`png` deliberately held at 0.17.** 0.18 merges `FilterType` +
  `AdaptiveFilterType` into one `Filter` enum, and while `Compression::Balanced`
  does map back to the same flate2 level, `Filter::Adaptive` is *not*
  equivalent to the `Paeth` + adaptive pair the canvas encoder uses. Measured
  on the standard FingerprintJS canvas sequence, 0.18 emits a 9,646-byte data
  URL where 0.17 emits 17,502 — i.e. a different canvas fingerprint for every
  page. Added `examples/canvas_fp_probe.rs` so this is checkable in one command
  before any future bump.

On [#32](https://github.com/yfedoseev/browser_oxide/issues/32): the reported
`deno_error` conflict is an artifact of how `cargo-outdated` probes. It
synthesizes a manifest requiring the latest of *everything simultaneously*,
which pairs `deno_core` 0.409 (whose own manifest pins `deno_error` **=0.7.1**)
against `deno_error` 0.7.3 — a combination that cannot resolve upstream and
does not exist in this workspace. It will keep recurring in the monthly
`outdated` workflow until `deno_core` catches up with `deno_error`.
- `sha1` and `sha2` 0.10 → **0.11**. These must move together: `sha2` 0.11
  pulls `digest` 0.11, which makes the in-scope `Digest` trait incompatible
  with a `sha1` still on `digest` 0.10.
- `adblock` 0.12 → **0.13** (optional `blocker` feature). Required an API port
  — `Engine::from_filter_set` → `new_with_filter_set`, `Request::new` gained a
  fourth argument, `BlockerResult.matched` → `should_block()`. Ported by
  [@Ran-Mewo](https://github.com/Ran-Mewo) in the SilvR-AI fork; adopted here
  with thanks. The `deny.toml` MPL-2.0 exception is name-based and still
  applies.
- `chrono` 0.4.44 → 0.4.45, `http2` 0.5.17 → 0.5.19, plus `cargo update` across
  the tree for all remaining semver-compatible upgrades.

### Security
Two advisories in the dependency tree are resolved by the `cargo update` above.
Neither is reachable through a public `browser_oxide` API, but both are worth
noting for anyone auditing the tree:

- **`quinn-proto` 0.11.14 → 0.11.16** — [RUSTSEC-2026-0185], remote memory
  exhaustion via unbounded out-of-order stream reassembly. This one sits in the
  HTTP/3 path, so it is reachable from a hostile server on an h3 connection.
- **`crossbeam-epoch` 0.9.18 → 0.9.20** — [RUSTSEC-2026-0204], invalid pointer
  dereference in the `fmt::Pointer` impl for `Atomic`/`Shared`.
- `anyhow` 1.0.102 → 1.0.104 also clears [RUSTSEC-2026-0190] (unsoundness in
  `Error::downcast_mut()`).

`deny.toml`: added documented ignores for [RUSTSEC-2026-0206] (`rustybuzz`) and
[RUSTSEC-2026-0192] (`ttf-parser`) — both *unmaintained* notices rather than
vulnerabilities, on the direct text-shaping stack, with no maintained pure-Rust
replacement. Dropped the now-stale `adler` ignore, which the `deno_core` bump
resolved.

[RUSTSEC-2026-0185]: https://rustsec.org/advisories/RUSTSEC-2026-0185
[RUSTSEC-2026-0204]: https://rustsec.org/advisories/RUSTSEC-2026-0204
[RUSTSEC-2026-0190]: https://rustsec.org/advisories/RUSTSEC-2026-0190
[RUSTSEC-2026-0206]: https://rustsec.org/advisories/RUSTSEC-2026-0206
[RUSTSEC-2026-0192]: https://rustsec.org/advisories/RUSTSEC-2026-0192
- CI actions: `actions/checkout` 4 → 6, `actions/upload-artifact` 4 → 7,
  `codecov/codecov-action` 4 → 7, `taiki-e/install-action` 2.49.40 → 2.81.11,
  `github/codeql-action` 4.36.0 → 4.36.2 (all SHA-pinned).

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
