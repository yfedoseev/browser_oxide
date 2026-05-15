# Kasada canadagoose 429 = EDGE token handshake, NOT JS fingerprint (2026-05-15)

## The decisive evidence

`kasada_error_blob_capture` (live canadagoose) captured the Kasada
error-reporting POST to `https://reporting.cdndex.io/error`. Body is
`{"data": base64(XOR_omgtopkek(payload))}`. Decoded (key `omgtopkek`,
100% printable — confirmed correct):

```json
{"type":"ab","action":"allow","og":"null","sid":"…",
 "bot1225":{"e":1,"r":"TypeError: Cannot read properties of undefined
   (reading 'unjzomuybtbyyhwwkdpkxomylnab')","t":1,"b":1},"time":3}
```

**`"action":"allow"`.** Kasada's own client-side anti-bot verdict for
this session is **allow**. The full sensor payload (blob 0) shows
smc/dpv/csc/kl/ao throwing the `unjzomuy` sentinel TypeError and
`spd` = all `"n/a"` — but these are the **`kasada_error_blob_capture`
test's own `globalThis.Function` wrapper artifact** (the §9.3 caveat),
NOT production:

- The clean production probe (`kasada_sentinel_identity_clean`, NO
  Function wrapper) proved the sentinel mechanism works correctly in
  production (80 tagged closures, all misses legit natives).
- Kasada itself reports `action:allow` even WITH those wrapper-induced
  errors present — it tolerates them.

## Conclusion — the investigation pivots

The canadagoose **429 is an edge-level decision**, not a JS-fingerprint
block:

1. JS verdict = `allow` (Kasada's own report).
2. Production sentinel mechanism = healthy (clean probe).
3. The 429 arrives at the **reload / network layer** with
   `[net] H2 connection failed … ALPN negotiated http/1.1, not h2`
   and `x-kpsdk-ct` learned but the reload still 429.

⇒ The block is the **Kasada edge token handshake**: the
`x-kpsdk-ct` → compute `x-kpsdk-cd` → resubmit cycle in
`crates/net/src/kasada_session.rs`. The edge keeps returning 429
because our computed `x-kpsdk-cd` (the PoW/duration/`rst`/`st`/`d`
fields) is being rejected — independent of how perfect the JS
fingerprint is. All the prior-session JS-surface audit work
(W1.1 memoization, the audit-group fixes, the audio FP parity) was
necessary hardening but was never the canadagoose blocker.

## Why this is good news

- It RULES OUT the entire JS-fingerprint surface as the canadagoose
  blocker (the clean probe + `action:allow` are independent
  confirmations).
- It localizes the problem to **one file** —
  `crates/net/src/kasada_session.rs` — the `compute_cd_header` token
  math (st / rst / duration / d / workTime). This session already
  touched it (the `rst` page-relative fix).
- It is a bounded crypto/protocol problem (the x-kpsdk-cd field
  derivation), the kind that is unit-testable against a captured
  known-good handshake — far more tractable than open-ended
  fingerprint parity.

## Next experiment (for the loop / next session)

Capture, on a live canadagoose run, the EXACT `x-kpsdk-cd` we send
and the edge's response. Compare each field (st, rst, duration, d, v,
workTime) against the Kasada ips.js reference derivation
(`docs/research_2026_05_14/01_KASADA.md` §2.x has the deobfuscated
formulae). The reload-still-429 means one field is wrong. The
`rst` semantics were corrected this session (page-relative); the
likely remaining suspects are `duration` (must match PoW difficulty),
`d` (clock-drift = workTime − server_st), or the PoW solution itself.
Decisive, file-local, unit-testable — not multi-day fingerprinting.

## Status correction

Prior docs framed canadagoose as "fingerprint-parity, days." The
error-blob `action:allow` supersedes that: it is an **edge
token-handshake** problem in `kasada_session.rs`, even more localized.
The audio FP work (committed, 123.97 ≈ Chrome 124.04) remains valuable
hardening with 7-site leverage (DataDome boring_challenge still needs
it) but is NOT on the canadagoose critical path. The canadagoose
critical path is the x-kpsdk-cd token.
