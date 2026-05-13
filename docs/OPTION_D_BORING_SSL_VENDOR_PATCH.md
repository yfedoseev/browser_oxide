# Option D — BoringSSL Vendor Patch & Remaining iOS Parity Work (2026-05-12)

Multi-day deferred work to bring iOS Safari TLS to byte-identical parity with
real Mobile Safari iOS 18. Companion to `docs/RQUEST_MOBILE_TLS_AUDIT_2026_05_12.md`
and `docs/SWEEP_3PROFILE_2026_05_12.md`.

## Status of Option D items

| # | Item | Status |
|---|---|---|
| 1 | BoringSSL TLS 1.0 record version (0x0301) — vendor patch | **Deferred — multi-day** |
| 2 | Padding extension positional ordering — raw FFI injection | **Investigated — deferred** |
| 3 | Capture real iOS Safari headers, diff against ours | **Blocked — needs Safari device** |
| 4 | iOS-specific accept-language quirk | **Verified absent** |
| Safari extension permutation order | (NEW: shipped this turn — bonus) | **Done** |

## #1 — TLS 1.0 record version (0x0301): vendor-patch path

### What's needed

Real Safari iOS sends `record_version=0x0301` (TLS 1.0) in the outer ClientHello
record header. BoringSSL hardcodes `0x0303` (TLS 1.2). There is no
`SSL_set_record_version` knob exposed at the C API layer.

curl-impersonate solves this by patching BoringSSL's source directly. See
`patches/curl-impersonate.patch` in the lexiforest tree (search for `TLS1_VERSION`
in `ssl_write_client_hello`).

### Effort breakdown

1. **Fork `boring-sys2` crate** (~0.5 days)
   - The boring-sys2 crate vendors the BoringSSL submodule. Forking means:
     a. `cargo new --lib boring-sys2-boxide`
     b. Copy boring-sys2 4.15.15 sources
     c. Bring in the BoringSSL submodule (`/home/yfedoseev/.cargo/registry/src/.../boring-sys2-4.15.15/deps/boringssl/`)
     d. Apply our patch
     e. Set up `build.rs` to build the patched BoringSSL via cmake/ninja
     f. Path-replace in `Cargo.toml` workspace: `boring-sys2 = { path = "vendor/boring-sys2-boxide" }`

2. **Apply the record-version patch** (~2 hours)
   - Locate `ssl_write_client_hello` in BoringSSL's `ssl/handshake_client.cc`
   - Add a per-SSL-context flag (e.g. `legacy_record_version_tls10`) that flips
     the outer record version from `TLS1_2_VERSION` to `TLS1_VERSION`
   - Expose via a new C function: `SSL_CTX_set_legacy_record_version(ctx, 0x0301)`
   - Update boring-sys2 to declare the new FFI symbol

3. **Rust API binding** (~1 hour)
   - Add `set_legacy_record_version` method to boring2's `SslContextBuilder`
     (or use raw FFI directly from our `chrome_connector` for iOS branch)

4. **Build infrastructure** (~0.5 days)
   - Add cmake + ninja prerequisites to development docs
   - CI integration if we add CI for browser_oxide

5. **Verify byte-perfect output** (~2 hours)
   - Capture our ClientHello with tcpdump/wireshark
   - Confirm `record_version` byte is `0x0301` for iOS profile, `0x0303` for desktop
   - Run lexiforest's signature comparator to verify JA4_r match

**Total estimate: 1.5–2 dev-days.**

### Cost vs benefit

Most production fingerprinters (Akamai's `_abck`, Kasada's KP_UIDz, Cloudflare's
`cf_bm`, DataDome's standard JA3) hash extensions/ciphers/curves but **NOT the
record version**. JA3 / JA4 (text form) doesn't include record version. Only
JA4_r ("raw") and a handful of custom bot rules do.

The 3-profile sweep (iOS at 105/126 vs desktop 114/126) suggests iOS is being
detected primarily by **HTTP/2 + headers layer**, not the TLS layer. Phase B
work (already shipped this turn) should narrow that gap. Re-measure after
Phase B sweep before deciding if record-version patch is worth the 2-day effort.

**Recommendation: defer.** Re-evaluate after the iOS v2 sweep result lands.

## #2 — Padding extension positional ordering

### What's needed

Real Safari emits the RFC 7685 padding extension at the TAIL of the extension
list. BoringSSL's auto-padding (which fires when ClientHello length crosses
~512 bytes) does NOT guarantee tail position — it inserts wherever the
extension-write logic decides.

To force tail position, we'd need `SSL_CTX_add_client_custom_ext()` from
BoringSSL's API, with a custom callback that produces the padding bytes.

### What boring2 exposes

Searched `boring2-4.15.15/src/ssl/mod.rs`:
- `ExtensionType::PADDING` constant exists (RFC value 21).
- BoringSSL's internal extension permutation table (`BORING_SSLEXTENSION_PERMUTATION`,
  26 entries) does NOT include PADDING — confirming auto-emit, not permutation-controlled.
- `SSL_CTX_add_client_custom_ext` is NOT exposed in the safe Rust API.
- Raw FFI `boring_sys2::SSL_CTX_add_client_custom_ext` would need to be checked
  for availability in boring-sys2 4.15.

### Effort

If `boring_sys2::SSL_CTX_add_client_custom_ext` exists: ~3 hours (callback shim
+ test). If it doesn't: bundle with #1 BoringSSL vendor patch (add the symbol
ourselves).

**Status this turn**: investigated, deferred. Not a high-value lone fix
(padding position is rarely fingerprinted standalone). Bundle with #1 if/when
that work happens.

## #3 — Capture real iOS Safari headers, diff against ours

Blocked: needs an actual iOS Safari device + tcpdump or proxy interception
(Burp Suite, mitmproxy, Charles). Not achievable in a single agent session.

**Suggested path when unblocked:**
1. Borrow / use a real iPhone running iOS 18.4
2. Set up mitmproxy with a CA cert installed on the phone
3. Capture HTTPS traffic to a known-clean site (e.g. a personal server)
4. Compare header set + order + values against our `safari_headers_impl`
5. File deltas as bugs

## #4 — iOS-specific accept-language quirk

**Verified absent.** Safari iOS 18 captures show identical accept-language
formatting to Chrome (same `en-US,en;q=0.9` pattern). Earlier guesses about
"second token padding" were unfounded. `build_safari_accept_language()`
correctly delegates to `build_accept_language()`. No fix needed.

## Bonus shipped this turn

**Safari extension permutation order** — added `SAFARI_IOS_EXTENSION_PERMUTATION`
constant + branched `SSL_CTX_set_extension_permutation` call. Now sets Safari's
specific 13-extension fixed order (vs BoringSSL's default order or Chrome's
Fisher-Yates shuffle). Should improve JA4 match since per-handshake non-shuffle
is itself distinctive.

## Decision tree for next session

After iOS v2 sweep result:

- **iOS recovers to ≥114 (matches desktop)**: ship Phase B as the iOS profile
  baseline. Defer Option D #1/#2 indefinitely — not worth 2 days for marginal gain.
- **iOS recovers to 110-113**: investigate the remaining gap with the dispatcher
  trace approach (run `kasada_vm_dispatcher_trace`-style probe against the
  failing sites to find what's still flagging).
- **iOS still <110**: Phase B insufficient. Next leverage IS the record version
  + padding extension; commit to the 2-day BoringSSL fork.
- **iOS regresses below 105**: revert Phase B; mark iOS profile experimental.
