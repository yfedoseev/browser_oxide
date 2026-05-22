# browser_oxide — conventions for AI coding assistants

This file documents conventions for AI coding assistants (Claude Code,
Cursor, etc.) working in this repo. For human contributors, see
`CONTRIBUTING.md` — it covers the same ground in more detail.

## Build & test

```bash
cargo build --workspace
cargo test --workspace -- --test-threads=1   # V8 isolates are per-thread
cargo clippy --workspace                     # advisory while backlog clears
cargo fmt --all -- --check
cargo doc --no-deps --workspace
```

## Architecture

- **15 crates** in `[workspace.members]` — see `Cargo.toml`. Each crate
  has a single responsibility (see `docs/ARCHITECTURE.md` for the full
  inventory).
- **Per-vendor challenge solving is out of scope here.** The engine
  exposes a `browser::ChallengeSolver` trait + `Page::navigate_with_solvers`
  hook; the concrete Akamai/Kasada/DataDome/Cloudflare implementations
  live in the private `vendor_solvers` companion crate. `Page::navigate`
  registers an empty solver set. Measured 2026-05-21: solvers add 0 net
  passes on the 126-corpus — the from-scratch engine carries the SOTA
  rate. Do NOT reintroduce vendor bypass code into public crates.
- **License:** MIT OR Apache-2.0; no GPL/LGPL/AGPL. One MPL-2.0
  transitive (`cooked-waker` via `deno_core` → `v8`) and one optional
  MPL-2.0 (`adblock`, behind the `blocker` feature in `net`, off by
  default), both tracked as per-crate exceptions in `deny.toml`.
  Mechanically enforced by `deny.toml` + the `deny` CI job.
- **V8** via `deno_core 0.311` (prebuilt binaries, ~130 MB on first
  fetch).
- **HTTP/TLS:** own stack in `crates/net/` using `boring2` (Cloudflare
  BoringSSL fork) for Chrome-identical TLS ClientHello + HTTP/2
  fingerprint.

## Key conventions

- **Tests are single-threaded.** V8 isolates are per-thread; running
  multi-threaded crashes the test process. CI enforces `--test-threads=1`.
- **Network tests are `#[ignore]`.** They require internet and live
  target sites. Run with `--ignored` locally only.
- **CSS is ours.** `crates/css_parser`, `css_selectors`, `css_values`,
  `css_cascade` are written from scratch — we do not pull Servo's MPL
  crates.
- **DOM is arena-allocated.** `NodeId` is `Copy` + `u32`; nodes live in
  a `Vec` inside the `Dom`. No `Rc<RefCell<…>>` patterns.
- **JS ops** use the `deno_core` `#[op2]` macro. State-bearing ops take
  `#[state] state: &T` for `OpState` access.
- **No new `unsafe` without a `// SAFETY:` comment.** The existing
  `unsafe` blocks (`crates/html_parser`, `crates/net/src/tls.rs`,
  `crates/browser/src/page.rs`, `crates/canvas/src/webgl_render.rs`)
  all have one — match the pattern.
- **Stealth profiles** are loaded from YAML/JSON via
  `StealthProfile::load_from_file(path)` or built from
  `stealth::presets::chrome_148_*` / `firefox_135_*` / mobile presets.
  See `crates/stealth/profiles/chrome_148_macos.yaml` for the schema.
- **Scope:** `SCOPE.md` defines what's in/out of scope for this
  project. Changes whose primary purpose is out of scope (e.g.
  site-specific exploit code) will be declined.

## Where to read more

- `CONTRIBUTING.md` — fuller contributor guide
- `SCOPE.md` — intended use, what this project is not for
- `SECURITY.md` — vulnerability reporting
- `docs/ARCHITECTURE.md` — workspace layout, dependency graph
- `docs/<CRATE>.md` — per-crate engineering reference
