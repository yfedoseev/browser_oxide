# JA4H license note

`ja4h.rs` in this directory is a clean-room implementation of the JA4H HTTP
fingerprint algorithm published by FoxIO LLC.

## Why this is here

JA4H is the canonical 2024+ HTTP-layer fingerprint used by Cloudflare, AWS
WAF, Auth0, and others. We need to compute it locally so we can:

1. Assert per-profile JA4H stability across releases (regression test).
2. Cross-check against `tls.peet.ws/api/all` (network-gated oracle test).

## Why it is `#[cfg(test)]`-gated

JA4H is **patent-pending** under [FoxIO License 1.1](https://github.com/FoxIO-LLC/ja4/blob/main/LICENSE)
which permits **non-commercial** use only. Per the license text, "internal
testing/evaluation" is explicitly within scope.

Our usage is strictly:
- Compiled only with `cargo test` (the `#[cfg(test)]` attribute on the module
  declaration in `lib.rs` ensures the function never reaches a release binary).
- Called only by other `#[cfg(test)]` code in `crates/net/tests/ja4h_*.rs`.
- Not exposed via the `net` crate's public API.

This fits inside the FoxIO License's testing/evaluation carve-out.

## Do NOT

- Re-export `ja4h::ja4h()` from `crates/net/src/lib.rs` outside `#[cfg(test)]`.
- Call `ja4h::ja4h()` from any production code path.
- Ship JA4H computation to customers without a separate FoxIO commercial license
  and patent review by counsel.

## Algorithm source

The algorithm is implemented from the spec text in
`https://github.com/FoxIO-LLC/ja4/blob/main/python/ja4h.py` (the only canonical
description; the `JA4H.md` doc in the same repo is incomplete). No code was
copied — all Rust code in `ja4h.rs` was written from the spec description and
tested against the spec's examples.
