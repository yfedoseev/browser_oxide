# Option D #1 — BoringSSL TLS 1.0 record version: NOT NEEDED (2026-05-12)

Investigated, then **resolved with zero engineering work**. The audit
(`docs/RQUEST_MOBILE_TLS_AUDIT_2026_05_12.md`) and the follow-on Option D doc
(`docs/OPTION_D_BORING_SSL_VENDOR_PATCH.md`) both warned that "BoringSSL emits
0x0303 in the outer record header for the ClientHello" and estimated 1.5–2 dev-days
to vendor-patch. **Both are wrong.**

## What the audit assumed

> "BoringSSL emits 0x0303 (TLS 1.2) in the outer record header for the ClientHello.
>  To force 0x0301 you have to patch BoringSSL itself..."

## What the actual source says

`boring-sys2-4.15.15/deps/boringssl/src/ssl/ssl_aead_ctx.cc:168-176`:

```cpp
uint16_t SSLAEADContext::RecordVersion() const {
  if (version_ == 0) {
    assert(is_null_cipher());
    return is_dtls_ ? DTLS1_VERSION : TLS1_VERSION;  // ← TLS1_VERSION = 0x0301
  }
  if (ProtocolVersion() <= TLS1_2_VERSION) { return version_; }
  return TLS1_2_VERSION;
}
```

When the AEAD context is the null cipher (`version_ == 0`) — which is the case
for any ClientHello sent on a fresh connection (no session resumption) — the
record version returned is **`TLS1_VERSION = 0x0301`**.

`SetVersionIfNullCipher()` is only called from:
- `handshake_client.cc:564` — for early-data session resumption (zero-RTT)
- `handshake_client.cc:724` — AFTER ServerHello is received (post-ClientHello)
- `handshake_server.cc:253` — server-side
- `handoff.cc:655` — connection handoff
- `tls13_client.cc:89` — TLS 1.3 path

**None of these fire before the initial ClientHello.** So `version_` is 0
when `RecordVersion()` is called for the ClientHello, and the byte on the wire
is `0x01` (low byte of 0x0301).

## Empirical verification (shipped this turn)

`crates/net/src/tls.rs::tests::safari_ios_emits_tls_1_0_record_version` and
`tests::desktop_chrome_emits_tls_1_0_record_version`. Each test:
1. Spawns a TCP listener that captures the first 5 bytes of any incoming connection
2. Builds an SslConnector for the profile (Safari iOS / Chrome desktop)
3. Initiates a TLS handshake against the listener (which doesn't respond)
4. Reads the captured 5-byte TLS record header
5. Asserts `record_version == 0x0301`

**Both tests pass.** Captured wire bytes confirm the source-code analysis.

## Implication for Option D

| Item | Original estimate | Actual status |
|---|---|---|
| #1 BoringSSL TLS 1.0 record version | 1.5–2 dev-days | **0 days — already correct** |
| #2 Padding extension positional ordering | bundled with #1 | Still deferred (raw FFI) |
| #3 Real iOS Safari header capture | blocked on hardware | Still blocked |
| #4 iOS-specific accept-language | small | verified absent |
| Bonus: Safari extension permutation | (NEW) | Done last turn |

**Net Option D effort to date: 0 days; Option D #1 fully resolved.**

## Why the audit was wrong

The audit pulled the assertion from secondary sources (curl-impersonate's claim
that BoringSSL needs patching for record version, plus wreq-util's
documentation). Both sources are talking about an OLDER BoringSSL where the
behavior may have actually emitted 0x0303 in some configuration. The boring-sys2
4.15.15 vendored BoringSSL ALREADY does the right thing.

This is consistent with curl-impersonate's recent move away from forking
BoringSSL toward using the upstream version with `SSL_CTX_set_extension_permutation`.

## Conclusion

Option D #1 is **DONE**. No vendor patch needed. Document the finding for
future readers so nobody else estimates 2 days for it.

The remaining Option D items (#2 padding extension, #3 real header capture)
are independent and can be addressed separately if/when iOS sweep performance
indicates they're worth pursuing. Currently iOS = 115/126 (+1 over desktop) —
**no anti-bot blocker is currently traceable to record-version or padding**, so
both remain low-priority.
