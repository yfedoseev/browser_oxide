# 03 — R-BESTBUY-AKAMAI: Stratum B; Akamai BMP sensor_data POST diff needed

**Status:** ⏸️ deferred; Stratum B classified.
**Sites in scope:** bestbuy (1). Patchright passes; BO + Camoufox v150 fail.
**Effort:** 1-2 days trace capture + 2-5 days fix (depends on what's identified).
**Scope:** public engine (probably + behavioural signals).

## TL;DR

bestbuy.com edge-tier hard-blocks naked-curl-class requests entirely
(this session: HTTP/2 stream RST on first connection; HTTP/1.1 30s
timeout). BO's chrome_147 TLS impersonation gets PAST the edge → BO
sees a 7 KB SPA shell, but the React app never hydrates. Patchright
(real Chromium) passes at 1246k. The differential between Patchright
and BO is most likely Akamai BMP sensor_data POST shape — what BO
writes there that Patchright doesn't.

## Why this matters

- Patchright passing proves the site is **Chromium-engine-reachable**.
  This is Stratum B (single site Patchright passes that neither BO nor
  v150 pass), so flipping it gives BO an edge over v150.
- Akamai BMP is one of the highest-prevalence anti-bot vendors;
  diagnosing bestbuy improves our understanding of Akamai's classifier
  beyond the homedepot sec-cpt analog ([01_R-AKAMAI-SECCPT-FLAKE.md](01_R-AKAMAI-SECCPT-FLAKE.md)).
- This session's `awswaf_probe` oracle pattern (which found FIX-J)
  should generalize to Akamai sensor_data — the same instrumentation
  shape applies.

## Current state

Captured this session (audit/16 §R-BESTBUY-AKAMAI):

- `curl https://www.bestbuy.com/` with Chrome 148 UA + HTTP/2:
  **stream 1 reset by server (error 0x2 INTERNAL_ERROR)**.
- `curl --http1.1`: **30 s timeout, 0 bytes**. Edge actively refuses
  naked-curl on this datacenter IP.
- BO's actual measured behaviour (prior baseline): receives a **7 KB
  SPA shell** body. So BO's TLS gets past the edge — but the SPA
  doesn't hydrate. The shell is a React app entry point that needs
  client-side JS to render real content.
- Patchright result (HANDOFF §1.8): **PASS 1246 k**.

The differential between Patchright (passing) and BO (7 KB shell):
- (a) TLS / HTTP/2 SETTINGS-frame fingerprint subtlety — BO's
  `chrome_147` impersonation may have a delta from real Chrome 148.
- (b) Behavioural-signal gap inside the SPA bootstrap — BO's
  `humanize.js` fires standard mouse/key events; Akamai BMP
  behavioural-analytics may want something more (mouse trajectories
  toward specific element classes, scroll on specific viewport
  fractions, etc.).
- (c) A specific JS API the SPA bootstrap reads where BO's value
  differs from real Chrome on macOS (post-FIX-D the M3 GpuProfile is
  fixture-aligned, but there may be other surfaces).

## Next steps

### Step 1 — Capture BO's traffic on bestbuy (~1 hour)

```bash
mkdir -p /tmp/bestbuy_probe
RUST_LOG=net=trace target/release/examples/sweep_metrics chrome_148_macos \
  <(echo '[{"cat":"stores","name":"bestbuy","url":"https://www.bestbuy.com/"}]') \
  /tmp/bestbuy_probe/sweep.json > /tmp/bestbuy_probe/trace.log 2>&1
```

Extract:
- The 7 KB SPA shell body — what scripts does it load?
- The Akamai sensor_data POST (URL pattern `/_bm/_data` typically).
  What's the payload structure / size?
- All cookies the response sets / requests carry.

### Step 2 — Diff against Patchright capture (~1 day)

Use the existing competitor harness (`benchmarks/bench_corpus_v2.py
patchright`) with packet-level inspection (mitmproxy or similar) to
capture Patchright's request flow on bestbuy. Compare:
- TLS ClientHello (JA3/JA4) — if these differ, that's the edge-tier
  delta.
- HTTP/2 SETTINGS frame (HEADER_TABLE_SIZE / ENABLE_PUSH /
  MAX_CONCURRENT_STREAMS / INITIAL_WINDOW_SIZE / MAX_FRAME_SIZE /
  MAX_HEADER_LIST_SIZE) — Chrome has specific values.
- The sensor_data POST payload — field-by-field diff. Akamai BMP
  sensor_data has documented field IDs; the field 65 (mouse
  trajectory) is the most common gap.

### Step 3 — Identify which differential blocks BO

Most likely first-cut:
- If TLS ClientHello differs → BO's `chrome_147` impersonation needs a
  `chrome_148` update (or finer-grained TLS-library work).
- If sensor_data payload differs → the gap is in `crates/akamai/src/`
  or `crates/browser/src/js/humanize.js`'s behavioural-event capture
  (the `__akamai_events` buffer).
- If both differ → fix TLS first (edge), then sensor_data (behavioural).

### Step 4 — Land the fix

Probable shapes (all public-engine):
- Update `chrome_147` TLS impersonation to `chrome_148` (touch
  `crates/net/src/tls.rs`). Cheap if `boring2` already supports it.
- Extend humanize.js to fire more event types / on specific elements
  Akamai BMP scores against.
- Add a public-engine sensor_data POST formatter (Akamai-specific but
  public-engine because it's the bundle's OWN format, not a vendor
  solve).

### Step 5 — Validate

```bash
target/release/examples/sweep_metrics chrome_148_macos \
  <(echo '[{"cat":"stores","name":"bestbuy","url":"https://www.bestbuy.com/"}]') \
  /tmp/bestbuy_validation.json
```

Expected: L3-RENDERED with body > 15 KB.

## Dependencies

- mitmproxy or equivalent for Patchright TLS / HTTP/2 capture (the
  competitor isn't BO-instrumentable directly).
- Documentation of Akamai BMP sensor_data field IDs (in BO's
  `docs/releases/v0.1.0-parity/26_AKAMAI_BMP_DEEP.md`).

## Sources / references

- `crates/browser/src/js/humanize.js` — the `__akamai_events` event buffer
- `crates/akamai/src/` — Akamai sensor_data field builders (if exist)
- `crates/net/src/tls.rs` — TLS impersonation
- `docs/releases/v0.1.0-parity/26_AKAMAI_BMP_DEEP.md` — Akamai field reference
- `docs/releases/v0.1.0-parity/audit/16_DECISION_LOG.md` §R-BESTBUY-AKAMAI — this session's classification
