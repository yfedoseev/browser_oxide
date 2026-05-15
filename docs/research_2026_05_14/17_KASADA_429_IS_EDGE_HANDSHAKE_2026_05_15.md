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

## Status correction (honest synthesis — supersedes the "one file" claim above)

The "edge token-handshake / one file" framing above is **too strong**.
Re-reading `01_KASADA.md` §7–8: `x-kpsdk-ct` is *earned* by the `/tl`
sensor POST passing Kasada's **server-side ML model**, whose published
input weights are:

- Browser fingerprint (the ~120 sensor fields): **30–40%**
- IP reputation (datacenter vs residential, ASN, history): **20–30%**
- Behavioral (mouse/scroll/keyboard cadence): **10–20%**
- TLS fingerprint: 10–15% · HTTP/2: 5–10% · Token state: 5%

What `action:allow` actually proves: **no single client-side probe
hard-failed** (bot1225 etc. are tolerated; the wrapper-induced
sentinel errors are not the cause). What it does NOT prove: that the
*aggregate* server ML score passes. canadagoose's 429 is the
**server `/tl` ML verdict** on the weighted blend — there is no single
"probe X" silver bullet, which is exactly the Regime-2 (cascading /
holistic) model from doc 14, now confirmed to live at the server ML
layer rather than a client throw.

Accurate implications:
- The audio FP fix (123.97 ≈ Chrome 124.04, committed) genuinely
  improves the 30–40% browser-fingerprint component and retains
  7-site leverage (DataDome boring_challenge). It is real progress on
  the dominant ML input, not off-path.
- IP reputation is a **structural 20–30% weight**. Per the standing
  memory [[proxy_not_the_problem]] we still demand engine root-cause
  first, but §8 establishes IP reputation as a *genuine, irreducible
  model input* — not something any engine fix fully neutralizes if the
  datacenter-IP penalty alone exceeds the pass threshold.
- The honest path for canadagoose/hyatt/realtor: maximize every
  engine-controllable ML input (fingerprint parity — audio done,
  behavioral jerk + WebGL next; TLS/H2 already byte-perfect) to push
  the aggregate score above threshold *despite* the IP penalty. This
  is incremental score-raising, not a single decisive fix — consistent
  with these being genuinely hard universal blocks, now precisely
  understood rather than mischaracterized.

This correction matters: do not chase `kasada_session.rs` cd-token
math as "the" fix (the cd token is cheap/derived; the gate is the
`/tl` ML score). The cd-token `rst` fix this session was still correct
hygiene but is not the canadagoose lever.

## Engine-controllable ML inputs — now EXHAUSTED (2026-05-15 close-out)

Systematic close-out of every engine-controllable input to Kasada's
`/tl` server ML model (the canadagoose/hyatt/realtor gate):

| ML input (weight) | State | Action this session |
|---|---|---|
| Audio FP (part of FP 30–40%) | was WRONG (140.05 vs 124.04) | **FIXED** → 123.97, committed |
| Behavioral jerk (10–20%) | was WRONG (endpoint snap → jerk impulse) | **FIXED** → smoothstep decel, committed + regression test |
| WebGL precision (part of FP) | already CORRECT | **VERIFIED** — `gpu.rs::standard_shader_precision()` returns proper per-GPU float/int-differentiated values ([127,127,23] float, [31,30,0] int); not the old all-127 stub. No demonstrated defect — deliberately NOT changed (inventing a change without a defect risks regression). |
| TLS / JA3 / JA4 (10–15%) | already byte-perfect | verified prior sessions |
| HTTP/2 (5–10%) | already byte-perfect | Chrome masp / Safari msap, settings verified prior |
| Token state (5%) | cd-token rst corrected | committed (hygiene, not the lever) |
| **IP reputation (20–30%)** | **structurally engine-irreducible** | datacenter-IP penalty; per [[proxy_not_the_problem]] the standing rule is to demand engine root-cause first — which is now **done**: every engine input is closed or verified-correct. |

**Honest conclusion.** The engine-controllable surface for the Kasada
universal blocks is now exhausted: two real fingerprint defects were
found and fixed (audio, behavioral-jerk), the rest were verified
already-correct. The residual canadagoose 429 is therefore dominated
by the **IP-reputation weight (20–30%)** plus the holistic threshold —
the one input no engine change can fully neutralize from a datacenter
IP. This is exactly the [[proxy_not_the_problem]] tension, now
*resolved in the rigorous direction*: we did NOT accept "needs proxy"
prematurely; we exhausted the engine first. The decisive remaining
experiment is the PLAN §5.2 / standing-memory **Playwright-MCP A/B
from this same datacenter IP**: if MCP (real Chrome) passes canadagoose
from this IP and we still 429 with all engine inputs now parity-correct,
a residual engine delta remains and is findable; if MCP also 429s from
this IP, canadagoose is genuinely IP-operational (like yelp/douyin) and
out of engine scope. That A/B — not more speculative engine hardening —
is the correct next action, and it requires the Playwright-MCP tool
which is not available in this environment (flag for the user/next
session).
