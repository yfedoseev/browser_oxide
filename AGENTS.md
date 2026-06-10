# BrowserOxide — conventions for AI coding assistants

This file documents conventions for AI coding assistants (Claude Code,
Cursor, etc.) working in this repo. For human contributors, see
`CONTRIBUTING.md` — it covers the same ground in more detail.

## Build & test

```bash
cargo build --workspace
cargo test --workspace -- --test-threads=1            # V8 isolates are per-thread
cargo clippy --all-targets --workspace -- -D warnings # GATING: warnings = errors
cargo fmt --all -- --check
cargo doc --no-deps --workspace
```

**Lint policy is strict.** CI builds, clippy, and rustdoc all run with
`-D warnings`, so any warning (`dead_code`, `unused`, clippy lints) fails the
build — there is no warning backlog. Suppressing a lint requires a *reasoned*
allow: `clippy::allow_attributes_without_reason` is denied workspace-wide, so a
bare `#[allow(dead_code)]` is itself an error. Every allow must justify itself:

```rust
#[allow(dead_code, reason = "RAII: keeps the worker channel open until the receive path is wired")]
```

Prefer removing the dead/offending code over suppressing it; reach for a
reasoned allow only when the lint is a genuine false positive.

## Architecture

- **One engine crate, modular inside.** The whole engine lives in the
  single `browser_oxide` crate (`crates/browser_oxide`), organized into
  modules — `dom`, `html_parser`, `js_runtime`, `event_loop`, `net`,
  `stealth`, `canvas`, `workers`, `css_parser`/`css_selectors`/
  `css_values`/`css_cascade`, `layout`, `protocol`, `host` — each with a
  single responsibility (see `docs/ARCHITECTURE.md`). The workspace
  publishes exactly two crates: `browser_oxide` and the `browser_oxide_mcp`
  server. `browser_oxide_py` is a standalone (PyPI) workspace.
- **Per-vendor challenge solving is out of scope here.** The engine
  exposes a `browser_oxide::ChallengeSolver` trait + `Page::navigate_with_solvers`
  hook; concrete per-vendor solver implementations are out of scope for
  this repository (see `SCOPE.md`). `Page::navigate` registers an empty
  solver set. Do NOT add vendor-specific bypass code to the public crate.
- **License:** MIT OR Apache-2.0; no GPL/LGPL/AGPL. One MPL-2.0
  transitive (`cooked-waker` via `deno_core` → `v8`) and one optional
  MPL-2.0 (`adblock`, behind the `blocker` feature, off by default), both
  tracked as exceptions in `deny.toml`. Mechanically enforced by
  `deny.toml` + the `deny` CI job.
- **V8** via `deno_core 0.403` (prebuilt binaries, ~130 MB on first
  fetch).
- **HTTP/TLS:** own stack in the `net` module
  (`crates/browser_oxide/src/net/`) using `boring2` (Cloudflare BoringSSL
  fork) for Chrome-identical TLS ClientHello + HTTP/2 fingerprint.

## Key conventions

- **Tests are single-threaded.** V8 isolates are per-thread; running
  multi-threaded crashes the test process. CI enforces `--test-threads=1`.
- **Network tests are `#[ignore]`.** They require internet and live
  target sites. Run with `--ignored` locally only.
- **CSS is ours.** The `css_parser`, `css_selectors`, `css_values`,
  `css_cascade` modules are written from scratch — we do not pull Servo's
  MPL crates.
- **DOM is arena-allocated.** `NodeId` is `Copy` + `u32`; nodes live in
  a `Vec` inside the `Dom`. No `Rc<RefCell<…>>` patterns.
- **JS ops** use the `deno_core` `#[op2]` macro. State-bearing ops take
  `#[state] state: &T` for `OpState` access.
- **No new `unsafe` without a `// SAFETY:` comment.** The existing
  `unsafe` blocks (under `crates/browser_oxide/src/` in the `html_parser`,
  `net/tls.rs`, `page.rs`, `js_runtime`, and `canvas/webgl_render.rs`
  modules) all have one — match the pattern.
- **Stealth profiles** are loaded from YAML/JSON via
  `StealthProfile::load_from_file(path)` or built from
  `stealth::presets::chrome_148_*` / `firefox_135_*` / mobile presets.
  See `crates/browser_oxide/profiles/chrome_148_macos.yaml` for the schema.
- **Scope:** `SCOPE.md` defines what's in/out of scope for this
  project. Changes whose primary purpose is out of scope (e.g.
  site-specific exploit code) will be declined.

## Where to read more

- `CONTRIBUTING.md` — fuller contributor guide
- `SCOPE.md` — intended use, what this project is not for
- `SECURITY.md` — vulnerability reporting
- `docs/ARCHITECTURE.md` — workspace layout, dependency graph
- `docs/<CRATE>.md` — per-crate engineering reference
