# adidas.com — Akamai Bot Manager v3

**Status**: BLOCKED. Stable across multiple session runs.

**Engine**: Akamai BMP v3 (stringent configuration). Same engine as
homedepot, stubhub, aircanada, playstation.

**Baseline response**: HTTP 200, body 2351-2418 bytes, contains a
`<div id="sec-if-cpt-container">` element and a dynamically-named sensor
script URL at `/9qizx734Mu_fe/...` or `/uRSXyxnkl5ofu295cA/...`. The
script URL rotates per request. The interstitial's outer structure is
identical between our baseline and our solver run — Akamai is treating
both as equally bot-like.

## What the sensor VM does (probe results)

From `crates/browser/tests/adidas_sensor_api_probes.rs` (read this file
if you need to re-run the probe):

### Globals touched (read counts)

- `WeakRef` × 7
- `navigator.permissions` × 7 — heavy queries on the permissions
  object
- `Notification` × 5
- `navigator.maxTouchPoints` × 4
- `navigator.connection` × 4
- `ServiceWorker` × 2
- `navigator.storage` × 2
- `navigator.userAgentData` × 2
- `performance.memory` × 2
- Single reads: `SharedWorker`, `OffscreenCanvas`, `SharedArrayBuffer`,
  `crossOriginIsolated`, `navigator.mediaDevices`, `navigator.bluetooth`,
  `navigator.hardwareConcurrency`, `navigator.deviceMemory`,
  `navigator.webdriver`

### Method calls (Canvas 2D paint sequence)

11 total Ctx2D paint calls, all in a deferred Promise callback (happen
DURING the event loop drain, not in the synchronous IIFE):

1. `fillRect(125, 1, 62, 20)`
2. `fillText("SomeCanvasFingerPrint.65@345876", 2, 15)` — first pass
3. `fillText("SomeCanvasFingerPrint.65@345876", 4, 17)` — same text at
   offset (2, 2) for sub-pixel rendering fingerprint
4. `arc(40, 40, 25, 0, 6.283185307179586, true)` — full circle
5. `fill()`, `stroke()`, `closePath()` — the circle finalization
6. `beginPath(); moveTo(67, -40); lineTo(200, 250); lineTo(100, 250);
   closePath(); fill()` — triangle with one vertex off-canvas
7. `beginPath(); moveTo(88, 0); bezierCurveTo(200, 50, 400, 250, 500,
   200); stroke()` — a bezier curve

### Method calls (Audio)

Full CreepJS-style OfflineAudioContext pipeline:

- `OfflineAudioContext.createOscillator()`
- `OfflineAudioContext.createDynamicsCompressor()`
- `OfflineAudioContext.startRendering()`
- `AudioContext.createAnalyser()`
- `AudioContext.createBufferSource()`

### Method calls (DOM)

- `Document.querySelector("head")` × 2
- `Element.setAttribute("type", "file")`, `setAttribute("capture",
  "user")` — mobile camera capability probe via a temp `<input>`
- `Element.remove()`

### What the sensor VM does NOT do (verified by probe)

- **No pixel extraction**: `toDataURL`, `getImageData`, `toBlob`,
  `convertToBlob`, `transferToImageBitmap`, `createImageBitmap` — all
  zero calls.
- **No canvas side-channel reads**: `measureText`, `getLineDash`,
  `isPointInPath`, `isPointInStroke` — all zero.
- **No WebGL at all** — every method probed on
  WebGL(2)RenderingContext.prototype reports zero calls.
- **No Worker APIs** — `Worker` reads = 0. The earlier assumption that
  "Akamai spawns a blob: Worker" was a misattribution of a Playwright
  network log entry.
- **No observer APIs** (IntersectionObserver, ResizeObserver,
  MutationObserver, PerformanceObserver) — all zero.
- **No `Function.prototype.toString` on canvas/audio methods** — the VM
  only calls .toString() on its own internal obfuscated functions
  (`hQfSQHChKk` × 10, `GKWKWywJSh` × 10) and on `eval`/`indexOf`/`Array`
  built-ins (which correctly return `{ [native code] }` in our runtime).
- **No async errors raised** during the canvas/audio code path.

## POST body structure

Two POSTs per solver run:

1. **Initial ping** — 277 bytes, 9 top-level `;`-separated sections.
   Sentinel value `8888888` in position 5 (the `bm_sz` cookie is not
   yet set, so the VM uses its internal fallback constant).

2. **Full sensor** — 3762 bytes (pre-humanize) or 4480 bytes
   (post-humanize). 30-48 top-level sections (variance run-to-run).
   Section 5 contains the `akid` token (44-char base64, fetched from
   `/_bm/get_params?type=get-akid&v=<script_hash>`). Section 6 contains
   event counts `[mme_cnt, mduce_cnt, tme_cnt, tduce_cnt, ke_cnt/other,
   elapsed_ms]`.

After humanize, 5 POSTs per solver run observed: ping + 4 sensor posts
of increasing size.

## Server-side verdict

All POSTs return `HTTP 201 Created`. Cookies grow to 4593+ chars
including `_abck`, `bm_sz`, `bm_ss`, `bm_lso`, `ak_bmsc`, `AKA_A2`. But
`_abck` trust slot stays `~-1~` across all three positions in the
cookie. The second blob in `_abck` (starts with `AAQAAAAF`) is the
server's encrypted verdict state — it grows with each POST, confirming
the server is processing our payload, just not accepting it.

**Full example `_abck` value** (797 chars, from a recent session run):
```
7A355BBD57D931E446C6B2299480810F~-1~YAAQa5UeuL3OMGOdAQAACCdKeg9LaL2bBvo+
rgQI3CgvgQUGCsMYSKTlb6iMPx8d+6MyQxl9DNuMH2fyKw/KwlVb8r+jfL3HTt5GKVvHckMR
DgC0xggD3cctMxp+XK472DVzJ2cosRfAG/awsAkbtQuYAdg2PXR44QTHYmQRQCUee1DXKoHj
6DwBpAf/bHNYHqpHyQuK+uXqxtq1A5WIlCUMnUyp9/9gdzzAKgTgKlAtKvONGZTpE4VJQ5nL
75gN/ExuCuSxTsIRox3fVlF47Zo6grXKZwXdn+etSi2m2OXqQVxLERZfnKh4C2kAQFstrZwP
...(truncated)...~-1~-1~1775876770~AAQAAAAF%2f%2f%2f%2f%2f8PR0817xj0...
```

Format: `HASH ~ TRUST_SLOT_1 ~ ENCRYPTED_STATE ~ TRUST_SLOT_2 ~ 
TRUST_SLOT_3 ~ TIMESTAMP ~ ENCRYPTED_STATE_2 ~ ANOTHER_TIMESTAMP`

All three trust slots are `-1`. `~0~` anywhere would mean trusted.

## What we've tried (and what didn't work)

1. **Cookie portability** (task #61, completed): Captured 56 cookies
   from a live Playwright Chrome session that successfully loaded the
   real adidas page. Replayed them via our `net::HttpClient` (Chrome
   131 TLS). Both baseline and cookies-present requests returned the
   same 2379-byte interstitial. Cookies are NOT portable — the trust
   verdict is bound to live sensor execution in the same session.

2. **T1.3 Blink audio port** (task #64, completed): Replaced our JS
   compressor with a Rust port matching FingerprintJS reference sum
   124.04 within 60 ppm. POST body content changed materially (section
   count 30→40, all obfuscated sections differ between pre-T1.3 and
   post-T1.3) but trust slot stayed at `~-1~`. Audio hash propagates but
   isn't the bottleneck.

3. **T1.5 Real Workers** (task #66, completed): Implemented real Web
   Workers on dedicated OS threads with mpsc channels and blob-URL
   resolution. Verified with `worker_page_integration.rs`. Re-ran adidas
   probe under instrumentation — sensor VM never calls `new Worker()`.
   Zero impact on adidas (the earlier "Akamai spawns a Worker" belief
   was wrong).

4. **OffscreenCanvas + proper navigator class prototypes** (task #68,
   completed): Added `OffscreenCanvas` class (was undefined), gave
   `MediaDevices`/`StorageManager`/`ServiceWorkerContainer`/`Bluetooth`/
   `NetworkInformation`/`Permissions`/`PermissionStatus` proper class
   prototypes so `Object.getPrototypeOf(nav.storage).constructor.name`
   returns the right brand. POST body moved from 3762 to 3757-3765
   bytes (noise). Not the bottleneck.

5. **Humanize script** (task #70, completed): 30 mousemoves + 2 clicks
   + keydown/keyup fired on every navigation. Section 6 event counts
   went from `44,92,0,1,13,2139` (physiologically impossible, 46 Hz
   clicking) to `30,39,0,0,8,0` through `12,39,0,0,5,2075` (more
   plausible). Sensor VM started emitting 5 POSTs per solver run
   instead of 2, final sensor body grew from 3762 to 4480 bytes (+19%).
   Trust verdict still `~-1~`. Behavioral signals are not the
   bottleneck either.

6. **akid round-trip verification** (task #71, completed): Confirmed
   our solver sends `GET /_bm/get_params?type=get-akid&v=HASH` with a
   44-char base64 SHA-256 (computed by the sensor VM via
   `Function.prototype.toString` on its own obfuscated top-level
   function). Server returns 200 for every akid GET. Cannot verify
   bit-for-bit correctness vs real Chrome without a synchronized
   Playwright capture of the same rotated VM URL — blocked by
   Playwright's WAF hard-block from this IP.

## What is almost certainly still wrong (ranked by likelihood)

1. **Audio sample bit-accuracy**. Our 60 ppm sum delta comes from using
   a calibrated sine instead of a proper Blink PeriodicWave wavetable.
   Per-sample f32 values differ by ~1e-4. If Akamai hashes individual
   samples (not just the sum), this matters. Fix requires a real
   PeriodicWave port beyond our T1.3 shortcut.

2. **Canvas paint output**. Despite the sensor VM not calling pixel
   extraction methods, our probe may have missed a path. Real Skia
   (skia-safe via T1.1) produces different pixels than tiny_skia. If
   the VM hashes via an unusual method (e.g., serializing the entire
   context state), we'd be caught.

3. **Font metrics**. The VM calls `fillText` twice. If `measureText`
   returns different values than Chrome's HarfBuzz shaper + DirectWrite
   rasterizer, that's a fingerprint. T1.2 would close this gap.

4. **`performance.memory` values**. Our values
   `jsHeapSizeLimit=2172649472, totalJSHeapSize=10000000,
   usedJSHeapSize=8000000` are suspiciously round. Chrome returns
   fluctuating values like `totalJSHeapSize: 10485760,
   usedJSHeapSize: 9478420` (not exact multiples of a million).

5. **`navigator.userAgentData.brands` ordering**. We emit a fixed order
   `[Chromium 130, Google Chrome 130, Not?A_Brand 99]`. Chrome
   randomizes the order on each navigation (to prevent fingerprinting
   that order). Fixed order = fingerprint.

6. **`navigator.connection.type == 'wifi'`**. We set it to `'wifi'` but
   Chrome's default for headless is `'4g'` with no `type`. Small detail
   that may matter.

7. **akid poisoning** (low probability given the evidence). The `v=`
   hash our VM sends is computed from `Function.prototype.toString` on
   the sensor VM itself. deno_core 0.311 uses V8 ~12.x; Chrome 130 uses
   V8 12.8. Theoretically the two V8 builds should produce identical
   `.toString()` output for the same source, but if there's any
   normalization difference, the hash differs and every subsequent
   sensor POST is rejected with a poison verdict even though the server
   returns 201. Would require synchronized capture to verify.

## What to try next (in order, with cost/risk)

1. **Get a clean-IP Chrome reference** (task #72). Capture a real
   Chrome's sensor_data POST body for the same rotated VM via
   Playwright. Diff section-by-section against ours. That names the
   specific field in 30 minutes. Cost: zero in code, minutes of
   ops (VPN, tether, different machine, or Hyper Solutions trial).
   **This is the only diagnostic that will actually identify the
   blocker.** Everything else is guessing.

2. **T1.2 cosmic-text font stack**. 50-70 hours. Closes the
   `measureText`/`fillText` gap. Likely matters if (1) is inconclusive.

3. **Full Blink PeriodicWave port** (beyond T1.3 shortcut). 10-15
   hours. Closes the audio bit-accuracy gap for frequencies other than
   10 kHz (useful for other sites) and probably closes the per-sample
   adidas gap if audio is the bottleneck.

4. **T1.1 skia-safe canvas**. 25-35 hours. Only do this if (1)
   specifically points at canvas.

## What NOT to try

- **Don't** add more per-engine token forwarding. The research confirms
  no open-source browser does this at runtime.
- **Don't** spend more time tuning our amplitude/phase/compressor
  params without a reference. We got to 60 ppm by scanning; going
  tighter requires a known-good target.
- **Don't** reverse-engineer the obfuscated sensor VM further by hand.
  It's 438 KB of intentionally impenetrable code. Commercial solvers
  use VM emulation, not static analysis.
- **Don't** commit any code that branches on `host.contains("adidas")`
  or similar. Site-specific probes in tests are fine; runtime code
  must stay generic per `01_architecture_principle.md`.

## Reproducibility

To reproduce the current blocked state:

```bash
cd /home/yfedoseev/projects/browser_oxide
BOXIDE_DUMP_POST_DIR=/tmp/my-adidas \
  cargo test -p browser --test adidas_sensor_capture -- \
  --ignored --test-threads=1 --nocapture
```

Expected: 4-10 POSTs dumped to `/tmp/my-adidas/`, final solver result
is the 2351-2418 byte interstitial, `_abck` cookie contains `~-1~`.

To run the API probe against the captured sensor VM:

```bash
# First capture the current VM.
cargo test -p browser --test adidas_fetch_sensor_vm -- \
  --ignored --test-threads=1 --nocapture
# Then run the probe.
cargo test -p browser --test adidas_sensor_api_probes -- \
  --ignored --test-threads=1 --nocapture
```

## Related tasks

- #61 Validate Akamai failure is fingerprint not timing [done]
- #64 T1.3 Port Blink DynamicsCompressorKernel [done]
- #67 Compare browser_oxide sensor_data POST to live Chrome [done,
  partial]
- #68 Investigate why sensor VM skips Worker [done]
- #69 Resolve sensor VM canvas extraction path [done]
- #70 Inject realistic behavioral events [done]
- #71 Verify Akamai akid v= round-trip [done, partial]
- #72 Get clean-IP Chrome reference [pending, the unblocker]
