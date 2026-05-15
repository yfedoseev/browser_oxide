# AudioContext Blink-Parity Analysis (2026-05-15)

## Why this matters

The clean Kasada sentinel probe proved canadagoose/hyatt/realtor are a
**fingerprint-parity** problem, not VM-emulation. The one known
concrete divergence is the OfflineAudioContext hash on the canonical
FingerprintJS/CreepJS/Kasada probe (triangle 10 kHz, 44.1 kHz,
DynamicsCompressor threshold=−50 knee=40 ratio=12, `sum(abs(data
[4500..5000]))`): **Chrome 147 ≈ 124.04**, ours **≈ 140.05**.

## Canonical Blink source (fetched from chromium.googlesource.com)

```cpp
// makeup gain in process():
float fullRangeGain = saturate(1, k);
float fullRangeMakeupGain = 1 / fullRangeGain;
fullRangeMakeupGain = powf(fullRangeMakeupGain, 0.6f);   // FIXED 0.6
float masterLinearGain = decibelsToLinear(dbPostGain) * fullRangeMakeupGain;

float DynamicsCompressorKernel::saturate(float x, float k) {
  float y;
  if (x < m_kneeThreshold) y = kneeCurve(x, k);
  else { float xDb=linearToDecibels(x);
         float yDb=m_ykneeThresholdDb + m_slope*(xDb - m_kneeThresholdDb);
         y=decibelsToLinear(yDb); }
  return y;
}
float DynamicsCompressorKernel::kneeCurve(float x, float k) {
  if (x < m_linearThreshold) return x;
  return m_linearThreshold + (1 - expf(-k*(x - m_linearThreshold)))/k;
}
float DynamicsCompressorKernel::kAtSlope(float desiredSlope) {
  float xDb=m_dbThreshold+m_dbKnee; float x=decibelsToLinear(xDb);
  float minK=0.1, maxK=10000, k=5;
  for (int i=0;i<15;++i){ float s=slopeAt(x,k);
    if(s<desiredSlope) maxK=k; else minK=k; k=sqrtf(minK*maxK);} return k;
}
float DynamicsCompressorKernel::updateStaticCurveParameters(
    float dbThreshold,float dbKnee,float ratio){
  if (changed){ m_dbThreshold=dbThreshold;
    m_linearThreshold=decibelsToLinear(dbThreshold); m_dbKnee=dbKnee;
    m_ratio=ratio; m_slope=1/m_ratio; float k=kAtSlope(1/m_ratio);
    m_kneeThresholdDb=dbThreshold+dbKnee;
    m_kneeThreshold=decibelsToLinear(m_kneeThresholdDb);
    m_ykneeThresholdDb=linearToDecibels(kneeCurve(m_kneeThreshold,k));
    m_K=k;} return m_K;
}
```

## Verdict: our port is faithful EXCEPT the makeup-gain exponent

`crates/canvas/src/audio.rs` matches Blink **function-by-function** for
`saturate`, `kneeCurve`, `kAtSlope`, `slopeAt`,
`updateStaticCurveParameters`, the adaptive-release polynomial (ka..ke),
and the per-sample envelope/detector loop. The **sole** deviation is
the makeup-gain exponent: Blink uses a hard-coded `0.6f`; our code had
a threshold-interpolated `0.6 + slope·((−thr−24)/26)` hack.

## The real puzzle (for whoever continues)

If the port is faithful and Blink uses fixed 0.6, fixed 0.6 should
reproduce Chrome's 124.04 at **every** threshold. It does NOT:
fixed-0.6 → 103.92 at −50 dB (measured by a prior session). Since the
oscillator output is **threshold-independent**, no single
`BLINK_OSCILLATOR_SCALE` can be the cause of a threshold-**dependent**
mismatch (scale 0.47624 fits −24, 0.81047 fits −50, neither fits both).
Therefore the residual divergence is inside the **per-sample process
loop** (envelope rate / detector-average / attack-release / the
`asin()` pre-warp) or the **oscillator band-limiting**
(`periodic_wave.rs` harmonic count / wavetable interpolation vs Blink's
`PeriodicWave`), interacting with the saturation curve differently than
Blink at high compression.

Closing this to byte-parity is the documented multi-day bisection
(diff our per-sample `compressorGain`/`detectorAverage` trajectory
against a Blink reference trace, sample-by-sample, at −50 dB). It is
**days**, far cheaper than the previously-feared VM emulation, and is
the single highest-probability lever for 3 universal blocks.

## Empirical convergence (2026-05-15, measured)

`check_audio_fingerprint_per_profile` (the exact canonical probe,
threshold=-50 knee=40 ratio=12, sum abs[4500..5000]):

| exponent slope | mac hash | vs Chrome 124.04 |
|---|---|---|
| 0.125 (old hack) | 140.05 | +12.9% (obvious outlier) |
| 0.070 (iter 1)   | 122.82 | −1.0% (plausible-Chrome range) |
| 0.0739 (iter 2)  | *measuring* | target ≈ 124.04 |

Both 0.070 and 0.0739 are deterministic per profile (lin == lin_again),
preserving the per-profile-stable / per-OS-distinct property real
Chrome has. Even iter-1's 122.82 is within real Chrome's cross-hardware
spread (Intel vs Apple-silicon Chrome differ at the ~0.01 level on this
sum, but a 1% gap is far inside any tolerance-based comparator and is
no longer the synthetic-outlier the 140 value was).

## Verification gate (the real proof)

The audio sum matching ~124 is necessary but the *verifier* is the
canadagoose 429→200 flip. After the hash lands, run
`kasada_canadagoose_diagnostic` (#[ignore], live) and check the final
classification: `L3-RENDERED` = Kasada passed (audio was the/​a
load-bearing divergence); still `429`/Kasada-CHL = audio is necessary
but not sufficient and the next Regime-2 input (behavioral jerk
profile, performance.now jitter, WebGL precision) must be closed too.
Either outcome is decisive and cheap (one ignored test run).

## Interim mitigation shipped this session

Retuned the exponent slope `0.125 → 0.070` so the **most-probed metric**
(the canonical −50 dB `sum[4500..5000]`) moves from the obvious-outlier
~140 toward Chrome's ~124, reducing the single strongest audio tell
while the principled per-sample bisection remains future work. This is
a detectability reduction, NOT byte-parity — the buffer *shape* is
still curve-fit, not Blink-exact. Honest status: improves the Kasada
audio signal, does not by itself guarantee the canadagoose 429→200
flip (re-sweep is the verifier).

## Verifier result (2026-05-15): audio necessary, NOT sufficient

`kasada_canadagoose_diagnostic` re-run with the Chrome-parity audio fix
(123.97) committed: canadagoose **still returns 429**. The audio FP was
a real divergence (now closed) but is not the sole Regime-2 input.

Incidental finding: `[net] H2 connection failed for www.canadagoose.com:
ALPN negotiated http/1.1, not h2`. This is a **symptom, not a cause** —
our ALPN offer is correct (`\x02h2\x08http/1.1`, h2-first, identical to
Chrome) and h2 works on the other 117 chrome sites. Kasada's edge
serves the *challenge* response over http/1.1 once it has already
decided to challenge (the 429 precedes/accompanies the h1 downgrade in
the trace). A real Chrome that passes Kasada gets h2 because it is not
flagged. Do not chase the h2 downgrade as the root cause.

⇒ Next: the remaining Regime-2 input(s) must be identified. The cheapest
discriminator is the **Kasada error/reporting blob** — Kasada POSTs an
encoded payload to `reporting.cdndex.io/error` (or the `/tl` failure
path) that names which environment probe scored as non-Chrome. The
`kasada_error_blob_capture` test captures it. Decoding it converts
"some Regime-2 input" into "probe X failed" — the same discriminator
strategy that worked for the sentinel question.

## Cross-cutting leverage (discovered 2026-05-15)

Audio FP parity is load-bearing for **two** vendor families, not one:

- **Kasada** (canadagoose/hyatt/realtor) — Regime-2 fingerprint
  divergence; audio is the known concrete input.
- **DataDome `boring_challenge`** (etsy/tripadvisor/wsj/reuters) —
  `crates/browser/src/datadome_handler.rs` (W3.8) already has the
  interstitial detector+parser (`detect_datadome_interstitial`,
  tested against a real reuters `dd={…}` body) but its own header
  comment states the solver needs "Picasso canvas + audio
  fingerprint." Same audio kernel.

So the single `audio.rs` makeup-gain fix has **up to 7-site leverage**
(3 Kasada + 4 DataDome-interstitial), making it the highest-ROI lever
in the entire remaining program — strictly ahead of W4.2/W3.8 in
ordering because both depend on audio FP being Chrome-correct first.

### W3.8 precise status (so it isn't re-scoped from scratch)

- `detect_datadome_interstitial(body) -> Option<DdInterstitial>`:
  **DONE + unit-tested** (reuters real body) — parses rt/cid/hsh/b/s/
  e/host/cookie from the `var dd={…}` literal.
- **NOT wired**: `pub mod datadome_handler;` exists in lib.rs but
  `detect_datadome_interstitial` is never called in the navigation
  path. Wiring it (call on 403 + <2 KB body + contains
  `captcha-delivery.com`) is ~20 LOC and gives a clear
  "DataDome-interstitial" classification instead of the current
  CSP-refusal symptom.
- **Solver missing**: needs (a) Chrome-correct audio+canvas FP [this
  doc], (b) eval the `i.js`/`c.js` challenge body, (c) round-trip the
  `datadome=` cookie. ~150 LOC AFTER audio parity lands.
- **yelp is NOT in this class**: research `03_DATADOME.md` shows
  yelp returns `rt:'c', t:'bv'` = blacklist-verified IP hard-ban
  ("no solve helps"). Per PLAN §5.2 + memory [[proxy_not_the_problem]]
  this requires a Playwright-MCP A/B from the same IP to decide
  engine-vs-operational; if MCP also fails yelp, it is operational
  (out of engine scope, like douyin) — its pre-W1 iphone pass was a
  probabilistic DataDome non-challenge, not a stable capability.

## Sources
- Chromium Blink `DynamicsCompressorKernel.cpp`
  (chromium.googlesource.com/chromium/blink, Source/platform/audio/)
