# Research-required follow-ups — 2026-04-28

After completing the DEEP_NEXT_STEPS roadmap through Phase F (humanization default-on) and Phase G.3 (AWS WAF vendor-detect logging), these items remain. Each requires dedicated reverse-engineering or vendor-fork work that exceeds the "implement from existing surface" scope of the prior phases. They are documented here so the next session can pick them up with concrete entry points.

---

## B.3 ext — Firefox TLS-class swap (NSS-coherent JA4)

**Goal**: When `BOXIDE_PROFILE=firefox_135_*`, emit Firefox's NSS-class TLS ClientHello (JA4 `t13d1715h2_5b57614c22b0_3d5424432f57`) instead of Chrome's BoringSSL JA4.

**Blocker**: `crates/net/` uses **boring2/BoringSSL** with hand-tuned cipher list, extension order, and ALPN values to match Chrome 130/147. Reconfiguring boring2 to emit Firefox's NSS-coherent ClientHello requires:
- Different cipher list (NSS uses different default order than BoringSSL)
- Different extension order (Firefox sends `key_share` earlier in the list)
- Different supported_groups order
- Different ALPN order (`h2,http/1.1` vs `h2,http/1.1,h3`)
- Different signature_algorithms list

**Effort**: 1-2 days for someone familiar with TLS internals + access to Firefox's `nss/lib/ssl/` source for reference.

**Validation**: hit `https://tls.peet.ws/api/all` from a `firefox_135_macos` profile and confirm JA4 = `t13d1715h2_5b57614c22b0_3d5424432f57`.

**Expected unlock**: leboncoin (DataDome), wsj (DataDome), some reuters/Akamai sites where TLS fingerprint is the dominant signal. Probably 4-8 sites flip.

**Entry points**:
- `crates/net/src/tls.rs` — current cipher / extension config
- `crates/net/src/lib.rs:118` — `HttpClient::new` profile-driven TLS setup

---

## G.1 — Akamai BMP `_abck` + sensor_data POST

**Goal**: Solve Akamai BMP v3 challenge by computing the sensor_data payload, POSTing to `/_bm/_data`, and following with a validated `_abck` cookie on subsequent requests.

**Blocker**: The sensor_data format is intentionally obfuscated. The reference samples in `docs/akamai_sensor_analysis/` are the deobfuscated bootstrap script (~10K lines of minified JS) but no payload-format documentation. To implement this:
1. Run the deobfuscated bootstrap in instrumented V8 (browser_oxide already does this — site renders, sensor JS runs)
2. Capture the actual sensor_data string our engine produces via `BOXIDE_DUMP_POST_DIR` env var (already exists in `crates/net/src/lib.rs:941`)
3. Compare to a real Chrome capture from the same machine
4. Identify which fingerprint surface differs (canvas, WebGL, audio, mouse-events, plugin enum, etc.)
5. Patch our shim to emit the same value

**Expected unlock**: Up to **9 retail sites** (walmart, target, homedepot, costco, bestbuy, wayfair, h-m, uniqlo, zara) + adidas, weather, expedia. The largest single-vendor opportunity.

**Effort**: 2-5 days, depending on how many shim deltas the diff surfaces.

**Entry points**:
- `docs/akamai_sensor_analysis/samsclub_akam13_bootstrap.deob.js` — deobfuscated reference
- `crates/stealth/src/kasada.rs` — pattern for a new `crates/stealth/src/akamai.rs`
- `crates/browser/tests/adidas_*.rs` — 4 existing test files for sensor capture
- Phase G.3 logging emits `[vendor-detect] akamai-bmp _abck set on …` so detected sites are easy to filter

---

## G.2 — DataDome solver (`dd_g`/`dd_s` token POST)

**Goal**: Solve DataDome's interstitial JS challenge — compute the `dd_g`/`dd_s` token, POST to `https://geo.captcha-delivery.com/captcha/check`, set the validated `datadome` cookie, retry navigation.

**Blocker**: DataDome's challenge JS is obfuscated. Two potential failure modes:
1. **Interstitial-only path** — DataDome runs JS that sets the `datadome` cookie automatically without user interaction (works on most third-party sites). Camoufox passes leboncoin via this path. Our engine fails because either:
   - DataDome scores Chrome+US-IP-with-Chrome-JA4 as bot risk (Phase B Firefox profile didn't help — need NSS TLS too)
   - Our shim returns a property value DataDome scores as inhuman (canvas hash, audio hash, screen dimensions, etc.)
2. **Captcha path** — when interstitial fails, DataDome serves a slider captcha. This requires either human-in-the-loop or a 3rd-party captcha service (out of scope).

**Quick experiment**: enable Phase B Firefox profile + (when shipped) Firefox NSS TLS, retest leboncoin. If it flips, the gap was UA+headers+TLS combined.

**Expected unlock**: 3-4 sites (etsy, leboncoin, wsj, yelp).

**Effort**: 1-2 days for interstitial-only path; weeks for captcha solver (effectively requires a CV model).

**Entry points**:
- `crates/stealth/src/kasada.rs` — pattern
- Phase G.3 logging emits `[vendor-detect] datadome on …` for filtering

---

## G.4 — PerimeterX press-and-hold

**Goal**: Solve PerimeterX's press-and-hold interstitial. Currently affects 2 sites: zillow (PerimeterX-PaH), wayfair (PerimeterX-CHL).

**Blocker**: Need PerimeterX's wire protocol — specifically what the press-duration is reported as in their telemetry POST. No public reference; proprietary protocol.

**Approach**:
1. Use Playwright MCP to load zillow with mitmproxy intercepting POSTs
2. Capture the press-and-hold sequence — what cookies / payloads are sent
3. Reverse-engineer the timing format
4. Implement in `crates/stealth/src/perimeterx.rs`

**Existing primitive**: `crates/browser/src/js/humanize.js` already simulates click sequences (mousedown → mouseup with a 45-75 ms delay). Press-and-hold needs a *sustained* mousedown for ~3-5 s — different shape.

**Effort**: 6-12 h once the wire protocol is known. The mitmproxy capture is the dominant time sink.

**Expected unlock**: zillow + wayfair (2 sites). May help others if PerimeterX is deployed.

**Entry points**:
- `crates/browser/src/js/humanize.js` — current click sequence
- `crates/stealth/src/behavior.rs` — likely lives next to existing behavioral primitives

---

## H.1 — HTTP/3 default-on

**Goal**: Enable HTTP/3 (QUIC) on all profiles for the 1-2 s per-site speedup on H3-capable origins (Cloudflare-fronted, most CDNs).

**Blocker**: documented as **gap #33** in `crates/net/src/lib.rs:303`:
> when `profile.allow_http3 = false` (the default) we DO NOT cache the h3 alternative. Reason: vanilla `quinn-proto 0.11` emits transport_parameters in a *random* order with a *random* GREASE TP per handshake. Real Chrome uses a deterministic fixed order — so upgrading to QUIC with our current stack would emit a uniquely-distinguishable browser_oxide signature. Until we vendor-fork quinn-proto with a Chrome-fixed-order patch, advertising h3 is worse than not speaking it at all.

**Approach**: Vendor-fork `quinn-proto 0.11`, patch `transport_parameters` serialization to emit Chrome's fixed order:
1. `original_destination_connection_id` (server only)
2. `max_idle_timeout` (30000 ms)
3. `max_udp_payload_size` (1472)
4. `initial_max_data` (256 KiB × 16 = 4 MiB)
5. `initial_max_stream_data_bidi_local` (1 MiB)
6. `initial_max_stream_data_bidi_remote` (1 MiB)
7. `initial_max_stream_data_uni` (1 MiB)
8. `initial_max_streams_bidi` (100)
9. `initial_max_streams_uni` (3)
10. `ack_delay_exponent` (3)
11. `max_ack_delay` (25)
12. `disable_active_migration` (present)
13. `initial_source_connection_id` (random)
14. `active_connection_id_limit` (4)
15. `grease_quic_bit` (always present, NOT random)

Plus deterministic GREASE TP at a fixed offset, not random.

**Effort**: 2-3 days (vendor fork + patch + tests + CI).

**Risk**: Cloudflare's bot management increasingly fingerprints H3 transport params — our current "no H3 advertised" stance is **safer than mis-fingerprinted H3**. Only enable after the fork is verified against real Chrome packet capture.

**Expected unlock**: 0.5-1.5 s per H3-capable site (~60% of the corpus). At current 7.0 min sweep, could shave ~30-60 s.

---

## H.2 — V8 module compile cache

**Goal**: Cache compiled bytecode for scripts > 10 KB. If two pages load the same script (gtm.js, jquery, etc.), reuse the compiled bytecode instead of re-parsing.

**Blocker**: Need to wire `v8::Isolate::create_code_cache` into our deno_core integration. Cache key = script URL (with content hash for safety). Storage = on-disk under `BOXIDE_CACHE_DIR` or in-memory LRU.

**Effort**: 1-2 days. Mostly understanding deno_core's existing script-execution path and inserting cache lookup before compile + cache store after compile.

**Expected unlock**: 0.5-2 s per site on script-heavy pages. Marginal at our current sweep speed (already 7 min).

**Entry points**:
- `crates/js_runtime/src/runtime.rs` — JsRuntime construction
- deno_core's `JsRuntime::execute_script` is what we'd wrap

---

## Recommendation order

If returning to this work:

1. **G.1 Akamai** — biggest single-vendor unlock (9 sites). Have the deobfuscated reference. Highest ROI.
2. **B.3 ext Firefox TLS** — would unlock the Firefox profile's full potential. Boring2 reconfiguration is a defined task even without the cipher diff.
3. **G.2 DataDome interstitial** — 3-4 sites; some of these may auto-flip when B.3 ext lands.
4. **G.4 PerimeterX press-and-hold** — 2 sites; modest scope.
5. **H.1 HTTP/3 with quinn-proto fork** — speed win; requires substantial fork effort.
6. **H.2 V8 code cache** — marginal speed gain at current state.

Sites still detected after G.1+G.2+G.4 land: ~5-8 (Cloudflare-CHL, generic captcha, BLOCKED outcomes — most IP-attributable, would need residential proxy per `memory/open_tasks.md#68`).

End-state estimate: **97 → ~115 PASS** with G.1+G.2+G.4 done. The remaining ~10 are infrastructure (IP) bound.
