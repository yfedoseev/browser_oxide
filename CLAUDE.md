# browser_oxide — Claude Code Conventions

## Build & Test
```bash
cargo test --workspace -- --test-threads=1   # V8 requires single-threaded tests
cargo clippy --workspace -- -D warnings      # Lint check
cargo fmt --all -- --check                   # Format check

# Browser comparison benchmarks (use --release for fair timing)
cargo test --release -p browser --test browser_comparison -- --ignored --test-threads=1 --nocapture
```

## Architecture
- 15 crates in workspace (see Cargo.toml members)
- MIT/Apache-2.0 licensed — NO MPL dependencies
- V8 via deno_core 0.311 (prebuilt binaries, ~130MB download)
- rquest for HTTP with BoringSSL TLS impersonation

## Key Conventions
- Tests use `--test-threads=1` because V8 isolates are per-thread
- Network tests are `#[ignore]` (require internet)
- CSS parser/selectors/values are our own (not Servo's MPL crates)
- DOM uses arena allocation with NodeId (Copy, u32 handle)
- JS ops use deno_core `#[op2]` macro with `#[state]` for OpState access
