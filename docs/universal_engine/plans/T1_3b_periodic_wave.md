# T1.3b — Full Blink PeriodicWave wavetable port

**Priority**: P1 (completes T1.3)
**Effort**: 10-15 hours
**Dependencies**: T1.3a shipped (it is — the DynamicsCompressor is done)

## Goal

Replace the "calibrated sine at amplitude 0.4762 for the 10 kHz case"
shortcut in `crates/canvas/src/audio.rs` with a real Blink-compatible
`PeriodicWave` wavetable that works for arbitrary oscillator
frequencies and wave types (sine, triangle, square, sawtooth).

## Why

The current T1.3a implementation hardcodes the calibration for one
specific frequency × sample-rate pair (10 kHz × 44.1 kHz) because
that's what CreepJS/FingerprintJS uses. For any other frequency the
audio output is wrong, so sites that probe different frequencies
catch us.

Additionally, "works for one frequency" is an architectural smell.
If a new fingerprinter picks 8 kHz or 440 Hz, we need to recalibrate.
A proper PeriodicWave port handles every frequency correctly.

The 60 ppm delta we observed on the 10 kHz case may also be because
our shortcut uses direct `sin()` instead of Blink's wavetable
interpolation. A proper port could close the remaining gap.

## Blink reference

**File**: `third_party/blink/renderer/platform/audio/periodic_wave.cc`
(Chromium) or `Source/WebCore/platform/audio/PeriodicWave.cpp`
(WebKit — may not exist; PeriodicWave lives under WebCore/webaudio in
some WebKit branches).

**Key concepts**:

1. **Fourier coefficients**: each wave type has a fixed set of
   `real[]` and `imag[]` coefficient arrays. For standard types:
   - Sine: real[1] = 0, imag[1] = 1 (all others zero)
   - Triangle: real[n] = 0, imag[n] = `(-1)^((n-1)/2) * (8/π²) / n²`
     for odd n, zero for even
   - Square: real[n] = 0, imag[n] = `(4/π) / n` for odd n
   - Sawtooth: real[n] = 0, imag[n] = `(2/π) / n` for all n >= 1

2. **Band-limited wavetables**: instead of generating samples on the
   fly from Fourier sums (expensive and aliased), Blink precomputes a
   set of wavetables at different pitch ranges. Each wavetable has
   the harmonics above its range's Nyquist zeroed out to prevent
   aliasing.

3. **Multiple wavetables per wave type**: Blink stores multiple
   wavetables per wave type, each band-limited to a different
   "pitch range" (the fundamental frequency range that table can
   play without aliasing). When the oscillator's frequency changes,
   Blink picks the appropriate table.

4. **Wavetable construction**: each wavetable is `kWaveTableSize = 2048`
   samples long. Blink constructs it via inverse FFT of the
   coefficient arrays. Specifically,
   `CreateBandLimitedTables(real, imag, num_coefficients)` zeros
   coefficients above Nyquist for that range, then calls an inverse
   FFT and normalizes.

5. **Oscillator sampling**: per output sample, advance a phase
   accumulator by `freq * wave_table_size / sample_rate`. Look up
   two adjacent table entries and linearly interpolate.

## Current shortcut

**File**: `crates/canvas/src/audio.rs`, `from_params` function:

```rust
// Current hack:
let amp: f32 = 0.4762;
let omega: f32 = 2.0 * std::f32::consts::PI * params.frequency as f32
    / sample_rate as f32;
// ... generates pure sin(omega * t) with fixed amplitude ...
```

## Step-by-step implementation

### Step 1 — Fetch Blink's PeriodicWave.cc (30 min)

```bash
# Try both mirrors — Chromium may 404 on main.
curl -sL "https://raw.githubusercontent.com/chromium/chromium/main/third_party/blink/renderer/platform/audio/periodic_wave.cc" -o /tmp/blink_pw.cc
curl -sL "https://raw.githubusercontent.com/chromium/chromium/main/third_party/blink/renderer/platform/audio/periodic_wave.h" -o /tmp/blink_pw.h

# If 404, use the googlesource API:
curl -sL "https://chromium.googlesource.com/chromium/src/+/main/third_party/blink/renderer/platform/audio/periodic_wave.cc?format=TEXT" | base64 -d > /tmp/blink_pw.cc
```

Read the source. Focus on:
- `PeriodicWave::CreateBandLimitedTables()`
- `PeriodicWave::WaveDataForFundamentalFrequency()` — picks which
  table to use for a given frequency
- `kNumberOfRanges` and `kCentsPerRange` constants
- `kMaxNumberOfPartials` (max harmonic count)

### Step 2 — Port the Fourier coefficients (1-2h)

**File**: `crates/canvas/src/audio/periodic_wave.rs` (new)

```rust
pub const WAVE_TABLE_SIZE: usize = 2048;
pub const NUM_RANGES: usize = 12;  // per Blink: 12 ranges, 1 per octave
pub const CENTS_PER_RANGE: usize = 1200;
pub const MAX_NUMBER_OF_PARTIALS: usize = WAVE_TABLE_SIZE / 2;

pub enum StandardWaveType {
    Sine,
    Triangle,
    Square,
    Sawtooth,
}

impl StandardWaveType {
    /// Returns the Fourier coefficients (real, imag) for this wave type.
    /// Arrays are length `MAX_NUMBER_OF_PARTIALS + 1` (index 0 is DC).
    pub fn fourier_coefficients(&self) -> (Vec<f32>, Vec<f32>) {
        let mut real = vec![0.0f32; MAX_NUMBER_OF_PARTIALS + 1];
        let mut imag = vec![0.0f32; MAX_NUMBER_OF_PARTIALS + 1];
        // DC is 0 for all standard waves.
        match self {
            StandardWaveType::Sine => {
                imag[1] = 1.0;
            }
            StandardWaveType::Square => {
                // b_n = (4/π) / n for odd n
                for n in (1..=MAX_NUMBER_OF_PARTIALS).step_by(2) {
                    imag[n] = (4.0 / std::f32::consts::PI) / n as f32;
                }
            }
            StandardWaveType::Sawtooth => {
                // Blink implementation has a specific sign convention.
                // Verify against the source.
                for n in 1..=MAX_NUMBER_OF_PARTIALS {
                    imag[n] = 2.0 / (n as f32 * std::f32::consts::PI);
                    if n % 2 == 0 {
                        imag[n] = -imag[n];
                    }
                }
            }
            StandardWaveType::Triangle => {
                // b_n = (8/π²) * ((-1)^((n-1)/2) / n²) for odd n
                let factor = 8.0 / (std::f32::consts::PI * std::f32::consts::PI);
                for n in (1..=MAX_NUMBER_OF_PARTIALS).step_by(2) {
                    let sign = if ((n - 1) / 2) % 2 == 0 { 1.0 } else { -1.0 };
                    imag[n] = factor * sign / (n * n) as f32;
                }
            }
        }
        (real, imag)
    }
}
```

### Step 3 — Inverse FFT for wavetable construction (2-3h)

**File**: `crates/canvas/src/audio/periodic_wave.rs`

Blink uses its own FFT implementation (
`third_party/blink/renderer/platform/audio/fft_frame.cc`), but for
our purposes a small hand-rolled inverse FFT (or the `rustfft` crate,
MIT) is fine.

**Option A: rustfft crate** (MIT, ~50 KB):

```toml
# crates/canvas/Cargo.toml
[dependencies]
rustfft = "6"
```

```rust
use rustfft::{num_complex::Complex, FftPlanner};

pub fn build_wavetable(real: &[f32], imag: &[f32]) -> Vec<f32> {
    let n = WAVE_TABLE_SIZE;
    assert!(real.len() >= n / 2 + 1);
    assert!(imag.len() >= n / 2 + 1);

    // Build the FFT input as a complex array of length N.
    // For a real-valued output, we fill the first half and mirror.
    let mut buf: Vec<Complex<f32>> = vec![Complex::new(0.0, 0.0); n];
    for k in 0..=n/2 {
        buf[k] = Complex::new(real[k], -imag[k]);  // Blink convention
        if k > 0 && k < n/2 {
            buf[n - k] = Complex::new(real[k], imag[k]);
        }
    }

    let mut planner = FftPlanner::<f32>::new();
    let ifft = planner.plan_fft_inverse(n);
    ifft.process(&mut buf);

    // Normalize. Blink normalizes to peak = 1.0.
    let samples: Vec<f32> = buf.iter().map(|c| c.re).collect();
    let peak = samples.iter().copied().fold(0.0_f32, |a, b| a.max(b.abs()));
    if peak > 0.0 {
        samples.iter().map(|s| s / peak).collect()
    } else {
        samples
    }
}
```

### Step 4 — Band-limiting for multiple ranges (2-3h)

Blink stores multiple wavetables, one per pitch range, each with
aliased harmonics zeroed.

```rust
pub struct PeriodicWave {
    wave_type: StandardWaveType,
    tables: Vec<Vec<f32>>,           // NUM_RANGES entries
    sample_rate: f32,
    nyquist: f32,
}

impl PeriodicWave {
    pub fn new(wave_type: StandardWaveType, sample_rate: f32) -> Self {
        let nyquist = sample_rate / 2.0;
        let (real, imag) = wave_type.fourier_coefficients();

        let mut tables = Vec::with_capacity(NUM_RANGES);
        for range in 0..NUM_RANGES {
            // Top frequency for this range, in Hz.
            // Range 0 covers the lowest octave; higher ranges cover higher.
            // Blink uses logarithmic ranges; verify exact formula in source.
            let top_hz = nyquist / 2.0_f32.powf(range as f32);

            // Zero any harmonic that would alias at top_hz.
            let mut real_clamped = real.clone();
            let mut imag_clamped = imag.clone();
            for n in 1..=MAX_NUMBER_OF_PARTIALS {
                // The n-th harmonic is at n * fundamental. If the table
                // plays at fundamental = top_hz, the harmonic is at
                // n * top_hz. If that's above nyquist, zero the harmonic.
                if (n as f32 * top_hz) > nyquist {
                    real_clamped[n] = 0.0;
                    imag_clamped[n] = 0.0;
                }
            }

            tables.push(build_wavetable(&real_clamped, &imag_clamped));
        }

        Self { wave_type, tables, sample_rate, nyquist }
    }

    /// Look up a sample at a given phase (in [0, 1)) for a given
    /// fundamental frequency.
    pub fn sample(&self, phase: f32, fundamental_hz: f32) -> f32 {
        // Pick the appropriate wavetable based on fundamental frequency.
        let table_index = self.table_index_for_frequency(fundamental_hz);
        let table = &self.tables[table_index];

        // Linear interpolation between adjacent samples.
        let position = phase * WAVE_TABLE_SIZE as f32;
        let i0 = position.floor() as usize % WAVE_TABLE_SIZE;
        let i1 = (i0 + 1) % WAVE_TABLE_SIZE;
        let frac = position - position.floor();
        table[i0] * (1.0 - frac) + table[i1] * frac
    }

    fn table_index_for_frequency(&self, freq: f32) -> usize {
        // Blink: log2(nyquist / freq) gives the range.
        // Clamp to [0, NUM_RANGES - 1].
        let ratio = (self.nyquist / freq).log2();
        let index = ratio.floor() as isize;
        index.max(0).min(NUM_RANGES as isize - 1) as usize
    }
}
```

### Step 5 — Use PeriodicWave in from_params (1h)

**File**: `crates/canvas/src/audio.rs`

Replace the hardcoded `amp * sin(omega * t)` with:

```rust
pub fn from_params(seed: u64, params: AudioParams) -> Self {
    // ... existing setup ...

    let wave = PeriodicWave::new(
        match params.wave_type {
            WaveType::Sine => StandardWaveType::Sine,
            WaveType::Triangle => StandardWaveType::Triangle,
            WaveType::Square => StandardWaveType::Square,
            WaveType::Sawtooth => StandardWaveType::Sawtooth,
        },
        params.sample_rate as f32,
    );

    let mut input = vec![0.0f32; padded_len];
    let mut phase = seed_phase;
    let phase_increment = params.frequency as f32 / params.sample_rate as f32;
    for (i, slot) in input.iter_mut().take(length).enumerate() {
        *slot = wave.sample(phase, params.frequency as f32) * seed_gain;
        phase += phase_increment;
        if phase >= 1.0 {
            phase -= 1.0;
        }
    }

    // ... rest unchanged: run through DynamicsCompressorKernel ...
}
```

### Step 6 — Verify the 10 kHz case still matches (1h)

Run the existing reference test:

```bash
cargo test -p canvas --test audio_reference reports_current_sum -- --nocapture
```

The sum should be very close to `124.04347527516074`. If it's now
further from the reference than our calibrated-sine shortcut (0.007),
investigate why — probably the wavetable normalization differs from
Blink's or the phase initialization is off.

If the wavetable version is WORSE than the shortcut (unlikely but
possible), either (a) fix the port, or (b) keep the calibrated sine
ONLY for the 10 kHz case and use the wavetable for all other
frequencies. Hybrid is ugly but pragmatic.

### Step 7 — Add tests for other frequencies (30 min)

```rust
#[test]
fn periodic_wave_triangle_at_440hz() {
    let wave = PeriodicWave::new(StandardWaveType::Triangle, 44100.0);
    // Sample one period at 440 Hz.
    let n = (44100.0 / 440.0) as usize;
    let mut samples = Vec::with_capacity(n);
    let mut phase = 0.0_f32;
    let inc = 440.0_f32 / 44100.0;
    for _ in 0..n {
        samples.push(wave.sample(phase, 440.0));
        phase += inc;
        if phase >= 1.0 { phase -= 1.0; }
    }
    // Peak should be ~1.0 (normalized).
    let peak = samples.iter().fold(0.0_f32, |a, b| a.max(b.abs()));
    assert!((peak - 1.0).abs() < 0.01);
    // Triangle wave mean-of-absolute should be ~0.5.
    let mean: f32 = samples.iter().map(|s| s.abs()).sum::<f32>() / n as f32;
    assert!((mean - 0.5).abs() < 0.05);
}
```

## Acceptance criteria

1. The 10 kHz × 44.1 kHz reference test still passes within 60 ppm
   of `124.04347527516074` (or ideally better).
2. New tests for 440 Hz and 1000 Hz pass with expected shapes.
3. All workspace tests green.
4. The calibrated-sine shortcut is removed from `from_params`.

## Why this matters for tier-1 sites

Only matters IF (a) the adidas sensor VM hashes individual samples at
f32 precision (uncertain; needs clean-IP Chrome reference to verify)
AND (b) the remaining 60 ppm delta comes from the wavetable
approximation rather than from the DynamicsCompressor port. If both
are true, a proper port closes the last gap. If either is false, this
is capability completeness work that doesn't flip any specific site
but is still correct to do.

## Related

- `crates/canvas/src/audio.rs` — current implementation
- `crates/canvas/tests/audio_reference.rs` — calibration reference
- `docs/universal_engine/site_debugging/adidas_akamai_bmp_v3.md` —
  includes "audio bit-accuracy" as the #1 remaining hypothesis for
  why adidas fails
