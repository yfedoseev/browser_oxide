# 05 — R-SPA-DOUYIN-SIG: `__ac_signature` reverse-engineering

**Status:** ⏸️ deferred to v0.3.0+. Open-ended signature reverse-engineering.
**Sites in scope:** douyin (1). Camoufox passes (Firefox base); Patchright fails. Firefox-only solve = unusual asymmetry.
**Effort:** 1-2 weeks open-ended.
**Scope:** public engine if tractable, but tail end of value.

## TL;DR

douyin.com returns HTTP 200 with a 72 KB normal SPA shell (NOT a
vendor antibot challenge). Bot detection happens inside the SPA's own
`__ac_signature` computation — obfuscated JS that reads
`crypto.getRandomValues` + AudioContext + UA + screen and emits a
signature the server-side verifier checks. BO's V8 produces a value
douyin's verifier rejects. Camoufox (Firefox) passes; Patchright
(Chromium) fails — confirming this is Firefox-vs-Chromium asymmetric.

## Why this matters

Low priority for v0.2.0 / v0.3.0:

- Single site (douyin = TikTok China-facing app).
- Firefox-only solve. If BO mimics Chrome (which it does by design),
  douyin probably won't ever flip without us implementing Firefox-
  signature emulation — which contradicts our Chrome positioning.
- v150's success on douyin comes from Firefox naturally producing the
  right value distribution; not a transferable lesson for BO.
- The 1-2 week budget is better spent on items affecting multiple sites
  ([07_FIX-D2-D3-WebGL.md](07_FIX-D2-D3-WebGL.md), [11_R-AWSWAF-FIX-J-deep.md](11_R-AWSWAF-FIX-J-deep.md)).

The reasons to keep this on the radar: TikTok's antibot stack appears
on many properties (TikTok web, douyin, capcut), so understanding the
signature mechanism has slow-burn value. But not now.

## Current state

Captured this session (audit/16 §R-SPA-DOUYIN-SIG):

- `curl https://www.douyin.com/` returns HTTP 200, **72,914 bytes** —
  a regular SPA shell, no `awswaf` / `datadome` / `kasada` / `akamai` /
  `captcha-delivery` markers.
- BO's measured behaviour (prior baseline): receives exactly **6327
  bytes** across all 4 BO profiles (deterministic detection — the
  server-side verifier responds based on the signature check, and
  ALL of BO's profiles compute the same wrong signature).
- v135 + v150 pass at 1MB.
- Patchright fails at 8601b. Firefox-only solve.

Known tokens (per HANDOFF §1.6):
- `__ac_signature` — the load-bearing signature
- `ttwid` — TikTok web ID
- `mssdk_` — Microsoft SDK polyfill stub? (probably a coincidence;
  TikTok-internal naming)
- `msToken` — token

The signature is typically a function of: UA, screen dimensions,
timezone, AudioContext fingerprint, mouse/keyboard event sequence,
performance.now() drift.

## Next steps

If/when pursued:

### Step 1 — Capture the 6327-byte BO response (~1 hour)

```bash
target/release/examples/sweep_metrics chrome_148_macos \
  <(echo '[{"cat":"misc","name":"douyin","url":"https://www.douyin.com/"}]') \
  /tmp/douyin_bo.json > /tmp/douyin_bo.log 2>&1
# Body in /tmp/douyin_bo.json results
```

### Step 2 — Locate `__ac_signature` source (~few hours to days)

douyin's SPA loads JS modules. The signature-computing code is in
one of them (obfuscated). Use the `awswaf_probe`-style oracle to
instrument navigator + crypto + AudioContext + performance.now
accesses; identify which module first reads these and the order.

Expected output: a call graph rooted at the signature-emitting
function. Then static-analyze that function to identify inputs +
hash algorithm.

### Step 3 — Implement signature-matching fixture (~1-2 weeks)

Per-profile-deterministic-yet-real-looking value of the inputs that
make BO's signature accepted. The exact mechanism depends on what
Step 2 finds.

Risk: the signature may incorporate behavioural-signal inputs
(mouse/key sequence) that humanize.js doesn't currently mimic well
enough for douyin's verifier. Adding douyin-specific behavioural
signals starts to feel like vendor solving.

### Step 4 — Validate

```bash
target/release/examples/sweep_metrics chrome_148_macos \
  <(echo '[{"cat":"misc","name":"douyin","url":"https://www.douyin.com/"}]') \
  /tmp/douyin_validation.json
```

Expected: L3-RENDERED at ~1 MB.

## Dependencies

- AST-walking tooling for obfuscated JS analysis.
- Sustained engineer time (1-2 weeks).
- Tolerance for "douyin might rotate the signature daily" — fix
  may not be stable.

## Sources / references

- `crates/js_runtime/src/extensions/crypto_ext.rs` (if exists)
- `crates/js_runtime/src/extensions/audio_ext.rs`
- `docs/releases/v0.1.0-parity/02_GAP_ANALYSIS.md` §douyin
- `docs/releases/v0.1.0-parity/audit/16_DECISION_LOG.md` §R-SPA-DOUYIN-SIG — this session's classification
