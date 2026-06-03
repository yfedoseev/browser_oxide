# Contributing to browser_oxide

Thanks for your interest. This document is the quick orientation for
contributors. The longer architectural references live in `docs/`.

## Code of Conduct

This project adheres to the [Contributor Covenant Code of Conduct](CODE_OF_CONDUCT.md).
By participating you accept its terms. Report violations privately via a
GitHub security advisory or by emailing the maintainer.

## Scope

Before opening a feature request or non-trivial PR, please read
[SCOPE.md](SCOPE.md). It defines what this project is for (archival,
accessibility, AI agents, security research, CTF, defensive testing) and
what it explicitly is not for. PRs whose primary purpose falls outside
scope will be politely declined.

## Getting set up

```bash
git clone https://github.com/yfedoseev/browser_oxide
cd browser_oxide
cargo build --workspace
cargo test --workspace -- --test-threads=1
```

### Prerequisites

- **Rust 1.83+** — pinned by `deno_core 0.311` / V8 prebuilts.
- **C compiler** — `gcc` or `clang` for native deps (BoringSSL via
  `boring2`).
- **~15 GB free disk** — V8 prebuilts are ~130 MB at fetch time and the
  workspace build artifacts grow quickly.
- **Internet** — first build pulls the V8 prebuilt binaries from
  `deno_core`'s release storage. Subsequent builds are offline.

The V8 isolate is **per-thread**; tests must run with `--test-threads=1`.
The CI lane enforces this.

## Workflow

1. Open or claim an issue. For non-trivial work, agree on the approach
   in the issue before writing code.
2. Branch off `main`. Naming: `fix/<topic>`, `feat/<topic>`,
   `docs/<topic>`, `chore/<topic>`.
3. Make focused commits. Conventional Commits format
   (`feat:`, `fix:`, `docs:`, `chore:`, `refactor:`, `perf:`, `test:`,
   `build:`, `ci:`) — the PR title is what matters most.
4. Run the gates before pushing:
   ```bash
   cargo fmt --all -- --check
   cargo clippy --workspace -- -D warnings
   cargo test --workspace -- --test-threads=1
   cargo doc --no-deps --workspace
   ```
5. Open a PR using the template. Link the issue. Describe how you tested.

## Code style

- `cargo fmt` is the formatter; no manual reformatting.
- `clippy -D warnings` is the lint gate; new warnings block merge.
- Public APIs need rustdoc — at minimum a one-line summary; a worked
  example for anything non-obvious.
- `unsafe` blocks need a `// SAFETY: …` comment explaining the invariant.
- Don't write comments that re-narrate the code. Write them when the
  *why* is non-obvious — a constraint, an invariant, a workaround.
- Don't add features beyond what the issue or task requires.

## Project structure

Workspace of 15 crates. Single responsibility per crate:

| Crate | Purpose |
|---|---|
| `css_parser` / `css_selectors` / `css_values` / `css_cascade` | The CSS engine. Our own, not Servo's MPL crates. |
| `dom` / `html_parser` | Arena DOM + `html5ever` integration. |
| `js_runtime` | V8 via `deno_core`, DOM bindings, WASM, Web APIs. |
| `canvas` / `layout` | Canvas 2D rendering and box-model layout. |
| `net` | HTTP/1+2+3 + stealth TLS via `boring2`. |
| `event_loop` / `workers` | Timers, microtasks, dedicated/shared/service workers. |
| `stealth` | Fingerprint profiles + navigator spoofing. |
| `protocol` | CDP server (Puppeteer/Playwright drop-in). |
| `browser` | Top-level `Browser` / `Page` API. |

When adding a new file, put it under the crate that owns the
responsibility. If it doesn't fit, that's a design discussion — open an
issue.

## Tests

- **Unit tests** live next to the code in `#[cfg(test)] mod tests`.
- **Integration tests** live in `crates/<crate>/tests/`.
- **Live (network) tests** are gated with `#[ignore]` and not run in
  CI by default.
- **Snapshot data** (HTML, captured headers) belongs under
  `crates/<crate>/tests/fixtures/`.

## Documentation

- Engineering docs are in `docs/<topic>.md`. One per topic. Date stamps
  in filenames are out of style for public docs — they belong in
  private session notes, not engineering reference.
- Per-crate rustdoc on the lib.rs preamble explains what the crate is
  for and how it interacts with neighbours.

## Releasing (maintainers)

Releases are driven from `main`. Tag with `vX.Y.Z`, `cargo publish` per
crate in dependency order. Don't release with a red CI.

## License

By contributing you agree your work is dual-licensed MIT OR Apache-2.0,
matching the project license, unless you state otherwise in your PR.
