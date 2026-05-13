# Mobile TLS Profile Audit — 2026-05-12

**Tier 2.1** from `RESEARCH_2026_05_12_mobile_and_kasada.md`. Pre-condition for any mobile-profile work in `browser_oxide`.

## Headline corrections vs the synthesis doc

The synthesis doc and the OSS audit both assumed `browser_oxide` depends on **rquest**. **It does not.** Verified:

- `crates/net/Cargo.toml` does NOT depend on `rquest`. The line is **commented out** (`# rquest = { version = "5.1", ... }`).
- Our actual stack is **direct boring2 4.15** (BoringSSL via `boring2`/`boring-sys2`/`tokio-boring2`) + a forked `http2` crate.
- All TLS configuration lives in `crates/net/src/tls.rs` (~283 lines).
- All HTTP/2 configuration lives in `crates/net/src/h2_client.rs` (~252 lines).
- The constructor `chrome_connector()` takes **no profile parameter** — every value is `const` or hardcoded.

This means **Tier 2.1 is not "audit a third-party library"** — it's "what would we have to add to our own TLS stack for mobile profiles." That refactor is a multi-day project, not the 1-day audit the synthesis doc assumed. This document scopes that work.

## Current state — Chrome 147 desktop, byte-perfect

`crates/net/src/tls.rs` lines 19–95:

| Knob | Value (hardcoded) | Source ref |
|---|---|---|
| Cipher list | 15 suites starting `TLS_AES_128_GCM_SHA256...` ending `TLS_RSA_WITH_AES_256_CBC_SHA` | `CIPHER_LIST` |
| Sigalgs | 8 entries, no SHA-1 | `SIGALGS_LIST` |
| Curves | `X25519MLKEM768, X25519, SECP256R1, SECP384R1` | `CURVES` |
| Extension permutation | 16 extensions, Fisher-Yates shuffled per ClientHello | `CHROME_EXTENSION_PERMUTATION` |
| Cert compression | Brotli only (algorithm id 2) | `add_cert_compression_alg(Brotli)` |
| ALPS payload | HTTP/2 SETTINGS (4 settings: HEADER_TABLE_SIZE=65536, ENABLE_PUSH=0, INITIAL_WINDOW_SIZE=6291456, MAX_HEADER_LIST_SIZE=262144) + empty ACCEPT_CH frame | `configure_connection`, lines 224–250 |
| HTTP/2 SETTINGS on connection | Identical to ALPS payload — same 4 settings, same order | `h2_client.rs` lines 38–41 |
| Min/Max TLS | 1.2 / 1.3 | `set_min_proto_version`, `set_max_proto_version` |
| ALPN | `h2, http/1.1` | `ALPN_PROTOS` |
| ECH grease | enabled per connection | `set_enable_ech_grease(true)` |

This is verified byte-perfect against `docs/CHROME_147_TLS_REFERENCE_2026_04_29.json`.

## Per-knob delta table — Chrome 147 desktop vs iOS Safari 18+ vs Chrome Android

| Knob | Chrome 147 desktop (current) | iOS Safari 18.0 | iOS Safari 18.4 | Chrome 147 Android (Pixel) |
|---|---|---|---|---|
| TLS lib | BoringSSL | Apple coreTLS | Apple coreTLS | BoringSSL |
| Min TLS | 1.2 | **1.0** (record_version=0x0301) | 1.0 | 1.2 |
| Cipher count | 15 | **20** (incl. 3 × 3DES + extra RSA) | 20 (same) | **15 — identical to desktop order** |
| Sigalgs | 8 | **10** (incl. SHA-1 + a duplicated `rsa_pss_rsae_sha384` — Apple bug we must reproduce verbatim) | 10 (same) | identical to desktop |
| Curves | `X25519MLKEM768, X25519, P-256, P-384` | `X25519, P-256, P-384, P-521` (no MLKEM, no Kyber, **adds P-521**) | same | `X25519Kyber768Draft00, X25519, P-256, P-384` (or no PQ on older builds) |
| ECH (encrypted_client_hello) | yes | **NO** | NO | yes |
| ALPS / `application_settings` | yes (codepoint 17613) | **NO** | NO | yes (codepoint 17513 in older Android signatures; 17613 on M147) |
| `cert_compression` | brotli (id 2) | **zlib (id 1)** | zlib | brotli |
| `session_ticket` extension | yes | **NO** | NO | yes |
| `padding` extension at tail | no | **yes** (RFC 7685) | yes | no |
| Extension order | Fisher-Yates shuffle | **fixed** (same every handshake) | fixed | Fisher-Yates (same as desktop) |
| handshake_version / record_version | 0x0303 / 0x0303 | **0x0303 / 0x0301** (TLS 1.0 record!) | 0x0303 / 0x0301 | 0x0303 / 0x0301 |
| HTTP/2 SETTINGS (count, ids) | 4 — `1, 2, 4, 6` | **5** — `2, 3, 4, 8, 9` (incl. ENABLE_CONNECT_PROTOCOL=1) | **4** — `2, 3, 4, 9` (drops 8 — version-distinguishing!) | identical to desktop (`1, 2, 4, 6`) |
| HTTP/2 INITIAL_WINDOW_UPDATE | 15663105 | 10420225 | 10420225 | 15663105 |
| HTTP/2 pseudo-header order | `:method :authority :scheme :path` ("masp") | `:method :scheme :authority :path` ("msap") | "msap" | "masp" — same as desktop |
| Extra empty SETTINGS frame post-WINDOW_UPDATE | no | **yes** (macOS Safari only — iOS does NOT) | n/a | no |

## Effort estimates by profile

### Chrome 147 Android — **~0.5 dev-days**

90% reuses desktop scaffolding. Three actual deltas:
1. Swap `SslCurve::X25519_MLKEM768` for the older Kyber768Draft00 OR omit PQ entirely. Need fresh M147 Pixel capture to verify whether Chrome rolled MLKEM to Android by 147 (lexiforest's most recent android wrapper still lacked it as of mid-2025).
2. `Sec-CH-UA-Mobile: ?1`, `Sec-CH-UA-Platform: "Android"`, `Sec-CH-UA-Model: "Pixel 9 Pro"`, mobile UA (the headers piece is in `crates/net/src/headers.rs` and the JS-surface piece is in `crates/js_runtime/src/js/window_bootstrap.js:1535+`).
3. `Sec-CH-UA-Form-Factors: "Mobile"`.

HTTP/2 layer untouched. ALPS untouched. Cipher/sigalgs/extension list untouched.

**Risk: low.** Refactor `chrome_connector()` to take `&StealthProfile` and branch on `profile.device_class` for the curves choice; everything else stays.

### iOS Safari 18 — **~2–3 dev-days + 1 hard sub-task**

Essentially a new TLS profile from scratch. Build order:

1. **`safari_ios_connector()`** parallel to `chrome_connector()`. Distinct cipher list (20 entries, see [lexiforest signature](https://github.com/lexiforest/curl-impersonate/blob/main/tests/signatures/safari_18.0_iOS.yaml)), distinct sigalgs (with duplicate), distinct curves (no PQ + P-521 added), fixed extension order (no `set_permute_extensions` call).
2. **Cert compression: zlib (id 1)** instead of brotli. boring2 4.15 supports `CertCompressionAlgorithm::Zlib` — verify; if not exposed, use the raw `SSL_CTX_add_cert_compression_alg` FFI. The `flate2` crate handles compression on our side.
3. **Disable ALPS, session_ticket, ECH:** drop the `SSL_add_application_settings` call, set `SslOptions::NO_TICKET`, drop `set_enable_ech_grease`.
4. **Padding extension at tail:** BoringSSL emits the RFC 7685 padding extension automatically when ClientHello length crosses ~512 bytes. May come for free if our ClientHello hits that threshold. If positional ordering matters (it should — Safari emits it last), we need raw extension injection via `SSL_CTX_add_client_custom_ext`.
5. **HTTP/2 SETTINGS rewrite:** in `h2_client.rs`, parameterize the SETTINGS list and order. iOS 18.4 = `2=0, 3=100, 4=2097152, 9=1`. Pseudo-header order swap to "msap".
6. **WINDOW_UPDATE delta:** initial increment 10420225 instead of 15663105.

### The hard sub-task: TLS 1.0 record version (0x0301)

BoringSSL emits `0x0303` (TLS 1.2) in the outer record header for the ClientHello — there's **no knob to set 0x0301**. Real Safari sends 0x0301 (TLS 1.0). curl-impersonate solves this by **patching BoringSSL** at `ssl_write_client_hello` (look for `TLS1_VERSION` in the upstream patch).

**Three options:**
1. **Vendor a patched boring2.** Fork the boring2 crate, apply the 1-line change to the underlying BoringSSL build, publish as a path dependency. ~half a day, but maintenance burden.
2. **Skip it and accept the JA4_r divergence.** Most production fingerprinters (Akamai's `_abck`, Kasada's KP_UIDz, Cloudflare's `cf_bm`) hash extensions/ciphers/curves but **NOT the record version**. JA3/JA4 (text form) doesn't include record version. JA4_r ("raw") and a few custom bot rules do. **Low-risk to skip for MVP.**
3. **Use SSL_set_version_callback** if exposed. boring2 doesn't expose it — would need custom binding work.

**Recommendation: skip for MVP**, document as a known divergence, revisit only if a specific target fingerprints record version.

## Implementation roadmap

### Phase 1 — refactor TLS layer to be profile-driven (1 day, no behavior change)

1. Add `device_class: enum { Desktop, MobileIOS, MobileAndroid }` to `StealthProfile` (already proposed in the synthesis doc as Tier 2.2).
2. Refactor `chrome_connector()` → `tls_connector(profile: &StealthProfile)`.
3. Move `CIPHER_LIST`, `SIGALGS_LIST`, `CURVES`, `CHROME_EXTENSION_PERMUTATION`, ALPS payload from module-level `const` to per-profile lookup.
4. **Verify bit-perfect equivalence** for all desktop tests after the refactor. **Zero behavior change** is the gate.

### Phase 2 — Chrome 147 Android profile (0.5 days)

1. Add `pixel_9_pro_chrome_147()` preset in `crates/stealth/src/presets.rs` with `device_class: MobileAndroid`.
2. Add Android-specific curve list (omit MLKEM; verify against fresh capture).
3. Add `Sec-CH-UA-*` mobile-flavor headers in `crates/net/src/headers.rs`.
4. Add chrome_compat tests verifying mobile JS surface (covered in synthesis doc Tier 2.5).

### Phase 3 — iOS Safari 18 profile (2–3 days)

1. Add `safari_ios_connector` branch in `tls_connector()`.
2. Add `iphone_15_pro_safari_18()` preset.
3. Add zlib cert compression (verify boring2 support; fallback to raw FFI).
4. Add iOS-specific HTTP/2 SETTINGS in `h2_client.rs`.
5. Add chrome_compat tests verifying iOS JS surface.

### Phase 4 — validation against lexiforest signatures (0.5 days)

Run our generated ClientHello through the canonical signature comparators:
- `tests/signatures/safari_18.0_iOS.yaml`
- `tests/signatures/chrome_131.0.6778.81_android.yaml`

Diff JA3 / JA4 / Akamai HTTP/2 hashes. Any divergence is a bug.

**Total: 4–5 dev-days for both mobile profiles, gated on Phase 1 refactor.**

## boring2 / BoringSSL gotchas catalog

| Gotcha | Solution |
|---|---|
| Cert compression: zlib (Safari) | `boring2::ssl::CertCompressionAlgorithm::Zlib` if exposed, else raw FFI |
| TLS 1.0 record version (Safari) | **Skip for MVP**, vendor-patched BoringSSL if needed |
| Padding extension positional order (Safari) | `SSL_CTX_add_client_custom_ext` for guaranteed tail position |
| No ALPS (Safari) | Don't call `SSL_add_application_settings` (per-profile branch) |
| No session_ticket (Safari) | `SslOptions::NO_TICKET` |
| Duplicated `rsa_pss_rsae_sha384` in Safari sigalgs | `set_sigalgs_list` accepts duplicates — verify boring2 doesn't dedupe |
| Extra empty SETTINGS frame post-WINDOW_UPDATE (macOS only) | h2 crate doesn't expose; would need fork. **iOS doesn't need this** — skip |
| HTTP/2 SETTINGS order matters | already handled in our forked `http2` crate (`PseudoOrder/SettingsOrder`) — verify mobile order is reachable through the same API |

## Authoritative references

- [lexiforest/curl-impersonate `safari_18.0_iOS.yaml` signature](https://github.com/lexiforest/curl-impersonate/blob/main/tests/signatures/safari_18.0_iOS.yaml) — canonical ClientHello + JA3/JA4/Akamai hashes
- [lexiforest/curl-impersonate `safari_18.4_iOS.yaml`](https://github.com/lexiforest/curl-impersonate/blob/main/tests/signatures/safari_18.4_iOS.yaml) — shows HTTP/2 delta from 18.0
- [lexiforest/curl-impersonate `chrome_131_android.yaml`](https://github.com/lexiforest/curl-impersonate/blob/main/tests/signatures/chrome_131.0.6778.81_android.yaml)
- [lexiforest curl_safari180_ios wrapper script](https://github.com/lexiforest/curl-impersonate/blob/main/bin/curl_safari180_ios)
- [lexiforest curl_chrome131_android wrapper](https://github.com/lexiforest/curl-impersonate/blob/main/bin/curl_chrome131_android) — comment "*The only difference from desktop is the absence of MLKEM*"
- [wreq-util safari/tls.rs](https://github.com/0x676e67/wreq-util/blob/main/src/emulate/profile/safari/tls.rs) — Rust source for ZlibCompressor + Safari extension order
- [wreq-util safari/http2.rs](https://github.com/0x676e67/wreq-util/blob/main/src/emulate/profile/safari/http2.rs) — six numbered HTTP/2 profile variants
- [Apple PQC support page](https://support.apple.com/en-us/122756) — confirms iOS 18 has NO MLKEM; iOS 26 is first to ship it

## Decision

**Recommendation: proceed with Phase 1 refactor + Phase 2 Android profile** (~1.5 days total). Defer iOS Safari to a separate session — it's a much larger project with the BoringSSL record-version question as a known but non-blocking divergence.

This unblocks the synthesis doc's Tier 2 mobile profile foundation work for Android. iOS can follow once the Phase 1 refactor proves the per-profile TLS branching works without regressions on desktop.
