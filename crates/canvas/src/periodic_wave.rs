//! Blink-compatible `PeriodicWave` wavetable.
//!
//! Ports the design of Chromium's
//! `third_party/blink/renderer/platform/audio/periodic_wave.cc`:
//!
//! 1. Each standard wave type (sine / triangle / square / sawtooth) has a
//!    fixed Fourier series expressed as `(real[n], imag[n])` coefficients
//!    indexed by harmonic.
//! 2. For each of `NUM_RANGES` band-limited "ranges" (powers-of-two
//!    fundamental frequency buckets), harmonics that would alias above
//!    Nyquist are zeroed and the remaining coefficients are inverse-FFT'd
//!    into a length-`WAVE_TABLE_SIZE` time-domain table.
//! 3. Each table is peak-normalised to 1.0 so the oscillator sees unit
//!    amplitude before downstream gain.
//! 4. At render time `sample(phase, fundamental_hz)` picks the correct
//!    band-limited table for the current pitch and linearly interpolates
//!    between two adjacent table entries.
//!
//! This replaces the prior "calibrated sine at amp 0.4762" shortcut in
//! `audio.rs` — that shortcut only worked for the 10 kHz × 44.1 kHz
//! FingerprintJS case. With this port, arbitrary `(wave_type, frequency,
//! sample_rate)` combinations render a plausible Blink-shaped signal,
//! which is the capability the Tier-1 stealth work needs.
//!
//! Licence notes: Blink's implementation is BSD-3. This is an
//! independent reimplementation (not a direct copy of Blink code) so the
//! MIT/Apache-2.0 project policy is unaffected. `rustfft` is MIT.

use rustfft::{num_complex::Complex, FftPlanner};

/// Length of each per-range wavetable. Blink uses 4096 by default; we
/// match the "small-buffer" variant (2048) which is what non-WebAudio
/// rendering uses and is sufficient for interpolation-quality needs at
/// the sample rates we care about.
pub const WAVE_TABLE_SIZE: usize = 2048;

/// Maximum harmonic index we populate. Blink limits this to `size / 2`
/// to avoid aliasing at the top of the FFT buffer.
pub const MAX_NUMBER_OF_PARTIALS: usize = WAVE_TABLE_SIZE / 2;

/// Number of band-limited tables. Blink uses 12 (one per octave between
/// 20 Hz and roughly 20 kHz). We use the same count so the pitch bucket
/// boundaries match Blink's per-octave granularity.
pub const NUM_RANGES: usize = 12;

/// Standard Web Audio oscillator wave types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StandardWaveType {
    Sine,
    Triangle,
    Square,
    Sawtooth,
}

impl StandardWaveType {
    /// Fourier coefficients for this wave type, as `(real, imag)` vectors
    /// of length `MAX_NUMBER_OF_PARTIALS + 1`. Index 0 is DC; index `n`
    /// is the n-th harmonic.
    ///
    /// Conventions (match Blink's
    /// `PeriodicWave::CreateStandardWaveform`):
    /// - DC = 0 for all standard waves.
    /// - Sine: unit amplitude at harmonic 1 only.
    /// - Triangle: `b_n = (8/π²) · (-1)^((n-1)/2) / n²` for odd n.
    /// - Square: `b_n = (4/π) / n` for odd n.
    /// - Sawtooth: `b_n = (2/π) / n` for all n ≥ 1, with alternating
    ///   sign starting negative on even harmonics (matches Blink's
    ///   `1/n - 2/n` convention — equivalent to a standard sawtooth).
    #[allow(clippy::needless_range_loop)] // n indexes parallel real/imag harmonic arrays
    pub fn fourier_coefficients(self) -> (Vec<f32>, Vec<f32>) {
        let len = MAX_NUMBER_OF_PARTIALS + 1;
        let real = vec![0.0f32; len];
        let mut imag = vec![0.0f32; len];
        match self {
            StandardWaveType::Sine => {
                imag[1] = 1.0;
            }
            StandardWaveType::Square => {
                let pi = std::f32::consts::PI;
                for n in (1..=MAX_NUMBER_OF_PARTIALS).step_by(2) {
                    imag[n] = (4.0 / pi) / n as f32;
                }
            }
            StandardWaveType::Sawtooth => {
                let pi = std::f32::consts::PI;
                for n in 1..=MAX_NUMBER_OF_PARTIALS {
                    let sign = if n % 2 == 0 { -1.0 } else { 1.0 };
                    imag[n] = sign * (2.0 / pi) / n as f32;
                }
            }
            StandardWaveType::Triangle => {
                let pi = std::f32::consts::PI;
                let factor = 8.0 / (pi * pi);
                for n in (1..=MAX_NUMBER_OF_PARTIALS).step_by(2) {
                    let sign = if ((n - 1) / 2) % 2 == 0 { 1.0 } else { -1.0 };
                    imag[n] = factor * sign / (n * n) as f32;
                }
            }
        }
        (real, imag)
    }
}

/// Inverse-FFT a pair of Fourier coefficient arrays into a time-domain
/// wavetable of length `WAVE_TABLE_SIZE`. The output is peak-normalised
/// so that `max(|sample|) == 1.0` (or all zeros for a silent wave).
fn build_wavetable(real: &[f32], imag: &[f32]) -> Vec<f32> {
    let n = WAVE_TABLE_SIZE;
    assert!(real.len() > n / 2, "real coefficient array too short");
    assert!(imag.len() > n / 2, "imag coefficient array too short");

    // Build the length-N complex FFT input. For a real-valued time-domain
    // output we need the spectrum to be conjugate-symmetric: `X[N-k] =
    // conj(X[k])`. We map the `(real, imag)` Fourier coefficients to the
    // FFT input with Blink's sign convention (imag negated on the positive
    // side so that a positive `imag[n]` contributes `+sin(n·ω·t)`).
    let mut buf: Vec<Complex<f32>> = vec![Complex::new(0.0, 0.0); n];
    for k in 0..=(n / 2) {
        buf[k] = Complex::new(real[k], -imag[k]);
        if k > 0 && k < n / 2 {
            buf[n - k] = Complex::new(real[k], imag[k]);
        }
    }

    let mut planner = FftPlanner::<f32>::new();
    let ifft = planner.plan_fft_inverse(n);
    ifft.process(&mut buf);

    // rustfft's inverse is unnormalised (no 1/N). That's fine because we
    // immediately peak-normalise to 1.0 per Blink.
    let mut samples: Vec<f32> = buf.iter().map(|c| c.re).collect();
    let peak = samples.iter().copied().fold(0.0_f32, |a, b| a.max(b.abs()));
    if peak > 0.0 {
        let inv = 1.0 / peak;
        for s in &mut samples {
            *s *= inv;
        }
    }
    samples
}

/// A band-limited wavetable oscillator, one set of tables per wave type.
pub struct PeriodicWave {
    tables: Vec<Vec<f32>>,
    sample_rate: f32,
    /// `nyquist / 2^range` cap for each entry of `tables`. A table is
    /// safe to play for fundamental frequencies up to this cap without
    /// aliasing the harmonics it contains.
    range_top_hz: Vec<f32>,
}

impl PeriodicWave {
    /// Construct a `PeriodicWave` for a standard wave type at the given
    /// sample rate. Builds `NUM_RANGES` band-limited wavetables eagerly.
    pub fn new(wave_type: StandardWaveType, sample_rate: f32) -> Self {
        let nyquist = sample_rate * 0.5;
        let (real, imag) = wave_type.fourier_coefficients();

        let mut tables = Vec::with_capacity(NUM_RANGES);
        let mut range_top_hz = Vec::with_capacity(NUM_RANGES);

        for range in 0..NUM_RANGES {
            // Per-octave cap. Range 0 covers the widest pitch bucket
            // (close to nyquist); subsequent ranges cover half that, etc.
            // A table for fundamentals up to `top_hz` must not contain
            // harmonics above `nyquist / top_hz * top_hz = nyquist`.
            let top_hz = nyquist / 2.0_f32.powi(range as i32);
            range_top_hz.push(top_hz);

            let mut real_clamped = real.clone();
            let mut imag_clamped = imag.clone();
            // Zero any harmonic whose frequency at the top of this range
            // would exceed Nyquist.
            for n in 1..=MAX_NUMBER_OF_PARTIALS {
                if (n as f32) * top_hz > nyquist {
                    real_clamped[n] = 0.0;
                    imag_clamped[n] = 0.0;
                }
            }

            tables.push(build_wavetable(&real_clamped, &imag_clamped));
        }

        Self {
            tables,
            sample_rate,
            range_top_hz,
        }
    }

    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// Pick the index of the band-limited table to use for a given
    /// fundamental frequency.
    ///
    /// Each table is band-limited against its own `top_hz` upper cap:
    /// range 0 keeps only harmonics that survive at the highest
    /// fundamental (i.e. very few), and higher-index tables keep more
    /// harmonics because they're designed for lower fundamentals.
    ///
    /// For a given `fundamental_hz` we want the *most* harmonics that
    /// are still alias-free — i.e. the highest-index table whose
    /// `top_hz` is still ≥ `fundamental_hz`. That's the narrowest pitch
    /// bucket that still contains our fundamental.
    fn table_index_for_frequency(&self, fundamental_hz: f32) -> usize {
        let mut best = 0usize;
        for (i, &top_hz) in self.range_top_hz.iter().enumerate() {
            if top_hz >= fundamental_hz {
                best = i;
            }
        }
        best
    }

    /// Sample the wavetable at `phase ∈ [0, 1)`, using the table
    /// appropriate for `fundamental_hz`. Linear interpolation between
    /// adjacent entries (Blink also uses linear, not cubic).
    pub fn sample(&self, phase: f32, fundamental_hz: f32) -> f32 {
        let index = self.table_index_for_frequency(fundamental_hz);
        let table = &self.tables[index];
        let position = phase * WAVE_TABLE_SIZE as f32;
        let i0 = (position.floor() as usize) % WAVE_TABLE_SIZE;
        let i1 = (i0 + 1) % WAVE_TABLE_SIZE;
        let frac = position - position.floor();
        table[i0] * (1.0 - frac) + table[i1] * frac
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pure sine: exactly one harmonic, peak-normalised to 1.0.
    #[test]
    fn sine_peak_and_shape() {
        let wave = PeriodicWave::new(StandardWaveType::Sine, 44100.0);
        let mut max_abs: f32 = 0.0;
        let mut min: f32 = 0.0;
        for i in 0..WAVE_TABLE_SIZE {
            let phase = i as f32 / WAVE_TABLE_SIZE as f32;
            let s = wave.sample(phase, 440.0);
            max_abs = max_abs.max(s.abs());
            min = min.min(s);
        }
        assert!(
            (max_abs - 1.0).abs() < 0.01,
            "sine peak should normalise to 1.0, got {max_abs}"
        );
        assert!(min < -0.9, "sine should swing negative, min = {min}");
    }

    /// Triangle shape characteristic: mean(|x|) over one period is ~0.5
    /// after peak normalisation (pure-triangle RMS ≈ 0.577, mean-abs = 0.5).
    /// Our Fourier-summed triangle has small ripple but should still be
    /// within a reasonable band.
    #[test]
    fn triangle_mean_abs() {
        let wave = PeriodicWave::new(StandardWaveType::Triangle, 44100.0);
        let n = 1024usize;
        let mut sum = 0.0f32;
        for i in 0..n {
            let phase = i as f32 / n as f32;
            sum += wave.sample(phase, 440.0).abs();
        }
        let mean = sum / n as f32;
        // Pure math triangle: 0.5. Band-limited + normalised allows a
        // wider window — use 0.4..0.6 to catch gross regressions but
        // tolerate reasonable harmonic truncation.
        assert!(
            (0.40..=0.60).contains(&mean),
            "triangle mean-abs out of range: {mean}"
        );
    }

    /// Square shape characteristic: every sample is close to ±1 after
    /// peak normalisation (band-limited edges ring, so allow up to 25%
    /// of samples in the "transition" bucket but the rest must be near
    /// the rails).
    #[test]
    fn square_bimodal() {
        let wave = PeriodicWave::new(StandardWaveType::Square, 44100.0);
        let n = 1024usize;
        let mut high = 0;
        let mut low = 0;
        for i in 0..n {
            let phase = i as f32 / n as f32;
            let s = wave.sample(phase, 440.0);
            if s > 0.5 {
                high += 1;
            } else if s < -0.5 {
                low += 1;
            }
        }
        let edge_free = high + low;
        assert!(
            edge_free as f32 / n as f32 > 0.6,
            "square should be mostly at rails, got {}/{n} near ±1",
            edge_free
        );
    }

    /// At 10 kHz × 44.1 kHz only the fundamental survives band-limiting
    /// (n=3 harmonic is at 30 kHz > 22.05 kHz Nyquist). The triangle
    /// wavetable in that regime should therefore behave like a pure
    /// sine — this is the case the old `audio.rs` shortcut calibrated
    /// against.
    #[test]
    #[allow(clippy::needless_range_loop)] // i indexes the wave table + analytic ref
    fn triangle_at_10khz_is_fundamental_only() {
        let wave = PeriodicWave::new(StandardWaveType::Triangle, 44100.0);
        // The topmost (narrowest) range table is what 10 kHz lands in:
        // top_hz drops below 10 kHz as we climb indices, so 10 kHz picks
        // the widest range, which for triangle should still retain only
        // the n=1 harmonic (nothing above it survives Nyquist).
        let index = wave.table_index_for_frequency(10_000.0);
        let table = &wave.tables[index];
        // Compare peak-normalised table against an analytic sine.
        let n = WAVE_TABLE_SIZE;
        let mut max_err = 0.0f32;
        for i in 0..n {
            let phase = i as f32 / n as f32;
            let sine_ref = (2.0 * std::f32::consts::PI * phase).sin();
            max_err = max_err.max((table[i] - sine_ref).abs());
        }
        assert!(
            max_err < 0.01,
            "triangle @ 10 kHz regime should collapse to pure sine (max_err={max_err})"
        );
    }

    #[test]
    fn deterministic() {
        let a = PeriodicWave::new(StandardWaveType::Triangle, 44100.0);
        let b = PeriodicWave::new(StandardWaveType::Triangle, 44100.0);
        for (i, (ta, tb)) in a.tables.iter().zip(b.tables.iter()).enumerate() {
            assert_eq!(ta, tb, "table {i} not deterministic");
        }
    }
}
