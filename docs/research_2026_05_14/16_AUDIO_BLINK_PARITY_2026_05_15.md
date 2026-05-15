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

## Interim mitigation shipped this session

Retuned the exponent slope `0.125 → 0.070` so the **most-probed metric**
(the canonical −50 dB `sum[4500..5000]`) moves from the obvious-outlier
~140 toward Chrome's ~124, reducing the single strongest audio tell
while the principled per-sample bisection remains future work. This is
a detectability reduction, NOT byte-parity — the buffer *shape* is
still curve-fit, not Blink-exact. Honest status: improves the Kasada
audio signal, does not by itself guarantee the canadagoose 429→200
flip (re-sweep is the verifier).

## Sources
- Chromium Blink `DynamicsCompressorKernel.cpp`
  (chromium.googlesource.com/chromium/blink, Source/platform/audio/)
