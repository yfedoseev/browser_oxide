# Session Handoff — 2026-05-14 end-of-day

## What landed this session

6 commits addressing the Kasada audit
(`12_KASADA_FINGERPRINT_ERROR_AUDIT_2026_05_14.md`) Groups A/D/E/F/G,
all derived from the captured `decrypted_blob_{1,2}_pretty.json` plus
the May-14 VM-dispatch trace (`kasada_vm_trace.json`).

| Commit  | Audit Group | What |
|---------|-------------|------|
| 7e9628c | G | MediaSource.isTypeSupported codec aliases (m4a/aac/acc/mpeg/ogg/wav/flac/mp3) — second `_supportedTypes` Set was incomplete |
| 5912bc3 | A + bonus | navigator.mediaCapabilities (was MISSING) + HTMLVideoElement.requestVideoFrameCallback/cancelVideoFrameCallback (Chrome 83+) |
| 8624c22 | bonus | HTMLCanvasElement.transferControlToOffscreen (Chrome 69+) |
| ab9f1d9 | F + D | per-Plugin-instance namedItem/item source leak fixed via _maskAsNative; lowercase identifiers (structuredClone, queueMicrotask, getComputedStyle, customElements, ...) skipped in interfaces_bootstrap.js _rest stub loop so `new structuredClone()` throws `is not a constructor` instead of `Illegal constructor` |
| 0ff7044 | E | iframe.contentWindow pre-populated with `screen` (availWidth/Height, width/height, availLeft/Top, colorDepth/pixelDepth, orientation, isExtended) and viewport (innerWidth/Height, outerWidth/Height, devicePixelRatio, scrollX/Y, pageXOffset/Y) so Kasada spd probe gets numbers, not "n/a" |

## What's pending in the audit

| Audit Group | Probe | Effort | Probability |
|-------------|-------|--------|-------------|
| B | Function.prototype.toString accepts non-Function `this` | LARGE (V8 internals) | LOW |
| C | `Class extends value #<C> is not a constructor` repr | MEDIUM (V8 trace) | UNKNOWN |

Both remaining items require V8 source-code work, not JS-level patches.
LOW probability per audit rankings.

## Measurement state

- In-flight 4-profile sweep (pre-this-session-fixes for chrome/pixel/iphone,
  partial-fixes for firefox depending on build timing): chrome 114 L3/126,
  pixel 115 L3/126, iphone 111 L3/126, firefox in-flight.
- Queued `next_sweep.sh` (PID 20978) waits for ALL DONE marker then runs
  fresh 4-profile sweep with all 6 commits — first cumulative measurement
  opportunity. Expected ~22:55 PDT 2026-05-14.

## Next session priorities (ordered by impact/effort)

1. **Read queued sweep results.** Compare with 2026-05-13 baseline
   (120/126 union, ±8 variance noise floor). Expected delta from audit
   fixes: +2 to +5 unique sites if Kasada's blob-shape probe matrix
   actually depends on these surfaces.
2. **Capture fresh VM trace post-fixes** via
   `kasada_capture_vm_trace.rs` and diff against May-14 baseline. The
   "first_throw_at" index should be higher (or different) than 2141.
3. **W4.5 candidate-A Kasada IPS.js VM emulation** — scaffolding shipped
   in `crates/akamai/src/tea_cbc.rs`. Need to:
   - Decode the 307 handler bodies from the captured `kasada_vm_trace.json`
   - Identify the handlers that produce the `body` field of the sensor POST
   - Reimplement in Rust + sign via captured key derivation
4. **Live Akamai fileHash via oxc_ast** — addresses homedepot's 40-min
   rotation. Reads bmak.js, walks Babel AST to find the literal Number
   that gets passed to substitute/shuffle. ~1 week effort.
5. **Behavioral fidelity** — `stealth/behavior.rs` produces mouse paths
   but BeCAPTCHA/Kasada classify by 2nd derivative (jerk) profile. If
   current paths use uniform spacing, classifier will flag.

## Universal-block set (going into next session)

- **canadagoose** (Kasada): sentinel still escapes despite all surface fixes.
  Crack requires VM emulation OR capturing a leak that pinpoints the EXACT
  remaining undefined.
- **hyatt** (Kasada): same root cause as canadagoose.
- **realtor** (Kasada): same.
- **douyin** (regional): China-only; needs CN locale + IP.
- **wildberries** (regional + TLS errors): RU; locale + IP + TLS.
- **homedepot** (Akamai): bmak.js fileHash rotates every ~40 minutes,
  faster than our static registry can keep up.

## Architecture notes (for picking up next session)

- All JS bootstrap files (`crates/js_runtime/src/js/*.js`) are embedded
  via `include_str!` in `snapshot.rs` — touching any of them invalidates
  the V8 snapshot and triggers a full rebuild (~5 min).
- Each `cargo test --release -p browser --test holistic_sweep
  holistic_sweep_parallel -- --ignored --test-threads=1` takes ~25 min
  per profile (4 profiles × 126 sites).
- Per-profile variance is ±2 sites, so ±8 union; single-run deltas ≤+1
  are noise, ≥+2 are signal.
- Akamai POST is gated to N=1 per page (was N=8, settled on N=1 per
  variance characterization).
- `BOXIDE_AKAMAI_FILE_HASHES` env var overrides per-host fileHash
  registry for ad-hoc captures.

## Quick reference: relevant files

- `crates/js_runtime/src/js/window_bootstrap.js` — Navigator surface,
  WebIDL classes, mediaCapabilities (this session), requestVideoFrameCallback
- `crates/js_runtime/src/js/canvas_bootstrap.js` — Canvas, OffscreenCanvas,
  transferControlToOffscreen (this session)
- `crates/js_runtime/src/js/dom_bootstrap.js` — DOM + iframe Proxy
  (Group E screen mirror in this session)
- `crates/js_runtime/src/js/interfaces_bootstrap.js` — WebIDL stub
  loop (structuredClone fix in this session)
- `crates/akamai/src/v3_payload.rs` — 30-key v3 cleartext schema
- `crates/akamai/src/crypto.rs` — shuffle_tokens_v3 + substitute_chars_v3
- `crates/akamai/src/lib.rs` — per-host fileHash registry,
  parse_bm_sz, build_v3_for_host
- `crates/akamai/src/tea_cbc.rs` — W4.5 Kasada candidate-A scaffolding
- `crates/browser/tests/capture_bmak_js.rs` — live bmak.js capture
- `docs/research_2026_05_14/12_KASADA_FINGERPRINT_ERROR_AUDIT_2026_05_14.md`
  — the audit driving this session
