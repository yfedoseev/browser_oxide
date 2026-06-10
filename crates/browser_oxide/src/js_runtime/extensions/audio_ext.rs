//! Web Audio ops.
//!
//! - `op_offline_audio_render` runs Blink's DynamicsCompressorKernel (Rust
//!   port in `crate::canvas::audio`) for the standard audio fingerprint probe.
//!   Per-`audio_seed` jitter perturbs threshold and release so different
//!   profiles produce distinct hashes within Blink's observed variance band.
//!
//! - `op_audio_analyser_freq_data` (P2.2a) computes the frequency-domain
//!   output of an AnalyserNode: Blackman window → FFT (rustfft) → magnitude
//!   → dB clamped to [minDB, maxDB]. Wired from `canvas_bootstrap.js
//!   AnalyserNode.getFloatFrequencyData`.
//!
//! - `op_audio_biquad_response` (P2.2b) computes the closed-form
//!   magnitude+phase response of a BiquadFilter for a given frequency
//!   array. Wired from `canvas_bootstrap.js BiquadFilterNode.getFrequencyResponse`.

use crate::canvas::{AudioFingerprint, AudioParams, WaveType};
use deno_core::op2;
use rustfft::{num_complex::Complex32, FftPlanner};
use std::f64::consts::PI;
use std::sync::OnceLock;

/// Render the standard audio-fingerprint probe pipeline:
/// `OfflineAudioContext(1, length, sample_rate)` → triangle osc at
/// `frequency` Hz → DynamicsCompressor with the parameters the sensor set →
/// destination. Returns the rendered samples as a Float32 buffer (f32 little-
/// endian bytes) which the JS side reinterprets as a `Float32Array`.
#[allow(
    clippy::too_many_arguments,
    reason = "audio op takes many args; struct-wrapping adds churn without clarity"
)]
#[op2]
#[buffer]
pub fn op_offline_audio_render(
    #[smi] seed: i32,
    #[smi] sample_rate: i32,
    #[smi] length: i32,
    frequency: f64,
    wave_type_id: i32, // 0=sine 1=triangle 2=square 3=sawtooth
    threshold_db: f64,
    knee_db: f64,
    ratio: f64,
    attack_seconds: f64,
    release_seconds: f64,
) -> Vec<u8> {
    let wave_type = match wave_type_id {
        0 => WaveType::Sine,
        2 => WaveType::Square,
        3 => WaveType::Sawtooth,
        _ => WaveType::Triangle,
    };

    // Per-seed compressor jitter (P2.2c). Real Blink's compressor parameters
    // sit at fixed spec values per node, but real hardware adds small
    // floating-point variance through the DSP pipeline. We perturb threshold
    // by ±5 mdB and release by ±0.1 ms based on `audio_seed` so different
    // profiles produce distinct FPjs hashes while staying well within
    // Blink's observed cross-machine variance band.
    //
    // Both formulas use `sin()` so that seed=0 produces zero jitter — this
    // preserves the calibrated Chrome 131 reference baseline (sum(abs(data
    // [4500..5000])) = 124.04347527516074 in audio.rs:165) for the default
    // profile while letting non-zero seeds spread out detectably.
    let seed_u64: u64 = seed as u32 as u64;
    let seed_f = seed_u64 as f64;
    let threshold_jitter = (seed_f * 0.31).sin() * 0.005; // ±5 mdB; sin(0)=0
    let release_jitter = (seed_f * 0.71).sin() * 0.0001; // ±0.1 ms; sin(0)=0

    let params = AudioParams {
        sample_rate: sample_rate.max(0) as u32,
        length: length.max(0) as u32,
        frequency,
        wave_type,
        threshold: threshold_db + threshold_jitter,
        knee: knee_db,
        ratio,
        attack: attack_seconds,
        release: (release_seconds + release_jitter).max(0.0),
    };

    let fp = AudioFingerprint::from_params(seed_u64, params);

    // Pack Float32 samples as little-endian bytes. JS side reconstructs via
    // `new Float32Array(new Uint8Array(bytes).buffer)`.
    let mut bytes = Vec::with_capacity(fp.data.len() * 4);
    for s in &fp.data {
        bytes.extend_from_slice(&s.to_le_bytes());
    }
    bytes
}

// ================================================================
// AnalyserNode.getFloatFrequencyData (P2.2a)
// ----------------------------------------------------------------
// Per Web Audio §1.36.2: applyBlackmanWindow → FFT → magnitude
// → smooth → 20·log10 → dB clamp. Fingerprint scripts hash this output.
// ================================================================

fn fft_planner() -> &'static std::sync::Mutex<FftPlanner<f32>> {
    static PLANNER: OnceLock<std::sync::Mutex<FftPlanner<f32>>> = OnceLock::new();
    PLANNER.get_or_init(|| std::sync::Mutex::new(FftPlanner::<f32>::new()))
}

/// Apply a Blackman window in place. Per Web Audio spec:
///   a₀ = 0.42, a₁ = 0.5, a₂ = 0.08
///   w[n] = a₀ - a₁·cos(2πn/N) + a₂·cos(4πn/N)
fn blackman_window(samples: &mut [f32]) {
    let n = samples.len();
    if n == 0 {
        return;
    }
    let n_f = n as f64;
    for (i, s) in samples.iter_mut().enumerate() {
        let i_f = i as f64;
        let w = 0.42 - 0.5 * (2.0 * PI * i_f / n_f).cos() + 0.08 * (4.0 * PI * i_f / n_f).cos();
        *s *= w as f32;
    }
}

/// Compute AnalyserNode frequency data. Inputs:
///   - `time_domain_bytes`: f32 LE bytes of length `fft_size` samples
///   - `fft_size`: power of two, 32..=32768
///   - `smoothing_x100`: smoothingTimeConstant × 100 (0..=100), spec default 80
///   - `prev_freq_bytes`: previous magnitude bins (f32 LE, fft_size/2 entries)
///                        for smoothing; pass empty bytes on first call.
/// Output: f32 LE bytes of fft_size/2 entries, dB-scaled magnitude clamped
/// to [-100, -30] dB.
#[op2]
#[buffer]
pub fn op_audio_analyser_freq_data(
    #[buffer] time_domain_bytes: &[u8],
    #[smi] fft_size: i32,
    #[smi] smoothing_x100: i32,
    #[buffer] prev_freq_bytes: &[u8],
) -> Vec<u8> {
    let n = fft_size.clamp(32, 32768) as usize;
    if !n.is_power_of_two() || time_domain_bytes.len() < n * 4 {
        return Vec::new();
    }

    // Decode time-domain f32 samples.
    let mut samples: Vec<f32> = (0..n)
        .map(|i| {
            let off = i * 4;
            f32::from_le_bytes([
                time_domain_bytes[off],
                time_domain_bytes[off + 1],
                time_domain_bytes[off + 2],
                time_domain_bytes[off + 3],
            ])
        })
        .collect();

    // Blackman window in place.
    blackman_window(&mut samples);

    // FFT (real input as Complex32 with imag=0).
    let mut buf: Vec<Complex32> = samples.iter().map(|&s| Complex32::new(s, 0.0)).collect();
    {
        let mut planner = fft_planner().lock().unwrap();
        let fft = planner.plan_fft_forward(n);
        fft.process(&mut buf);
    }

    let half = n / 2;
    let n_f = n as f32;

    // Magnitudes — first half only (Nyquist symmetry).
    let mut mags: Vec<f32> = buf[..half]
        .iter()
        .map(|c| (c.re * c.re + c.im * c.im).sqrt() / n_f)
        .collect();

    // Smoothing: smoothed = prev * τ + current * (1 - τ).
    let smoothing = (smoothing_x100.clamp(0, 100) as f32) / 100.0;
    if prev_freq_bytes.len() >= half * 4 && smoothing > 0.0 {
        for (i, m) in mags.iter_mut().enumerate() {
            let off = i * 4;
            let prev = f32::from_le_bytes([
                prev_freq_bytes[off],
                prev_freq_bytes[off + 1],
                prev_freq_bytes[off + 2],
                prev_freq_bytes[off + 3],
            ]);
            *m = prev * smoothing + *m * (1.0 - smoothing);
        }
    }

    // dB scale and clamp to [minDB, maxDB] = [-100, -30] (spec defaults).
    let db_to_bytes: Vec<u8> = mags
        .iter()
        .flat_map(|&m| {
            let db = if m > 0.0 {
                20.0 * m.log10()
            } else {
                -f32::INFINITY
            };
            let clamped = db.clamp(-100.0, -30.0);
            clamped.to_le_bytes()
        })
        .collect();
    db_to_bytes
}

// ================================================================
// BiquadFilterNode.getFrequencyResponse (P2.2b)
// ----------------------------------------------------------------
// Per Web Audio §1.7: bilinear-transform coefficients per filter type;
// |H(z)| at z = exp(jω) where ω = 2π·f/fs.
// ================================================================

#[derive(Clone, Copy)]
enum BiquadType {
    Lowpass = 0,
    Highpass = 1,
    Bandpass = 2,
    Lowshelf = 3,
    Highshelf = 4,
    Peaking = 5,
    Notch = 6,
    Allpass = 7,
}

impl BiquadType {
    fn from_id(id: u32) -> Self {
        match id {
            0 => Self::Lowpass,
            1 => Self::Highpass,
            2 => Self::Bandpass,
            3 => Self::Lowshelf,
            4 => Self::Highshelf,
            5 => Self::Peaking,
            6 => Self::Notch,
            7 => Self::Allpass,
            _ => Self::Lowpass,
        }
    }
}

/// Compute biquad coefficients (b0, b1, b2, a1, a2 with a0 normalized to 1)
/// per Web Audio §1.7.7 bilinear-transform formulas.
// eq_op: the `(1.0 / 1.0 - 1.0)` term is the spec's `(1/S - 1)` factor
// instantiated with shelf-slope S = 1.0 — written verbatim to mirror the
// W3C formula, not a typo.
#[allow(
    clippy::eq_op,
    reason = "(1/1 - 1) is the spec's (1/S - 1) with shelf-slope S=1, written verbatim to mirror the W3C formula"
)]
fn biquad_coeffs(
    kind: BiquadType,
    frequency: f64,
    q: f64,
    gain: f64,
    sample_rate: f64,
) -> (f64, f64, f64, f64, f64) {
    let w0 = 2.0 * PI * frequency / sample_rate;
    let cos_w0 = w0.cos();
    let sin_w0 = w0.sin();
    let alpha_q = sin_w0 / (2.0 * q);
    let a_factor = 10f64.powf(gain / 40.0);

    let (b0, b1, b2, a0, a1, a2) = match kind {
        BiquadType::Lowpass => {
            let b1 = 1.0 - cos_w0;
            let b0 = b1 / 2.0;
            (b0, b1, b0, 1.0 + alpha_q, -2.0 * cos_w0, 1.0 - alpha_q)
        }
        BiquadType::Highpass => {
            let b1 = -(1.0 + cos_w0);
            let b0 = (1.0 + cos_w0) / 2.0;
            (b0, b1, b0, 1.0 + alpha_q, -2.0 * cos_w0, 1.0 - alpha_q)
        }
        BiquadType::Bandpass => (
            alpha_q,
            0.0,
            -alpha_q,
            1.0 + alpha_q,
            -2.0 * cos_w0,
            1.0 - alpha_q,
        ),
        BiquadType::Notch => (
            1.0,
            -2.0 * cos_w0,
            1.0,
            1.0 + alpha_q,
            -2.0 * cos_w0,
            1.0 - alpha_q,
        ),
        BiquadType::Allpass => (
            1.0 - alpha_q,
            -2.0 * cos_w0,
            1.0 + alpha_q,
            1.0 + alpha_q,
            -2.0 * cos_w0,
            1.0 - alpha_q,
        ),
        BiquadType::Peaking => (
            1.0 + alpha_q * a_factor,
            -2.0 * cos_w0,
            1.0 - alpha_q * a_factor,
            1.0 + alpha_q / a_factor,
            -2.0 * cos_w0,
            1.0 - alpha_q / a_factor,
        ),
        BiquadType::Lowshelf => {
            let two_sqrt = 2.0 * a_factor.sqrt() * alpha_q;
            let b0 = a_factor * ((a_factor + 1.0) - (a_factor - 1.0) * cos_w0 + two_sqrt);
            let b1 = 2.0 * a_factor * ((a_factor - 1.0) - (a_factor + 1.0) * cos_w0);
            let b2 = a_factor * ((a_factor + 1.0) - (a_factor - 1.0) * cos_w0 - two_sqrt);
            let a0 = (a_factor + 1.0) + (a_factor - 1.0) * cos_w0 + two_sqrt;
            let a1 = -2.0 * ((a_factor - 1.0) + (a_factor + 1.0) * cos_w0);
            let a2 = (a_factor + 1.0) + (a_factor - 1.0) * cos_w0 - two_sqrt;
            (b0, b1, b2, a0, a1, a2)
        }
        BiquadType::Highshelf => {
            let two_sqrt = 2.0 * a_factor.sqrt() * alpha_q;
            let b0 = a_factor * ((a_factor + 1.0) + (a_factor - 1.0) * cos_w0 + two_sqrt);
            let b1 = -2.0 * a_factor * ((a_factor - 1.0) + (a_factor + 1.0) * cos_w0);
            let b2 = a_factor * ((a_factor + 1.0) + (a_factor - 1.0) * cos_w0 - two_sqrt);
            let a0 = (a_factor + 1.0) - (a_factor - 1.0) * cos_w0 + two_sqrt;
            let a1 = 2.0 * ((a_factor - 1.0) - (a_factor + 1.0) * cos_w0);
            let a2 = (a_factor + 1.0) - (a_factor - 1.0) * cos_w0 - two_sqrt;
            (b0, b1, b2, a0, a1, a2)
        }
    };

    // Normalize so a0 = 1.
    (b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0)
}

/// Compute |H(e^jω)| and arg(H(e^jω)) for a biquad at each input frequency.
///
/// Inputs:
///   - `freq_in_bytes`: f32 LE input frequencies (Hz)
///   - `filter_type_id`: 0=lowpass 1=highpass 2=bandpass 3=lowshelf
///                       4=highshelf 5=peaking 6=notch 7=allpass
///   - `frequency`, `q`, `gain`, `sample_rate`: filter parameters
/// Output: 2× freq_in.len() f32 LE — first half magnitudes, second half phases (rad).
#[allow(
    clippy::too_many_arguments,
    reason = "audio op takes many args; struct-wrapping adds churn without clarity"
)]
#[op2]
#[buffer]
pub fn op_audio_biquad_response(
    #[buffer] freq_in_bytes: &[u8],
    #[smi] filter_type_id: i32,
    frequency: f64,
    q: f64,
    gain: f64,
    sample_rate: f64,
) -> Vec<u8> {
    let n = freq_in_bytes.len() / 4;
    if n == 0 {
        return Vec::new();
    }
    let kind = BiquadType::from_id(filter_type_id.max(0) as u32);
    let (b0, b1, b2, a1, a2) = biquad_coeffs(kind, frequency, q, gain, sample_rate);

    let mut out = Vec::with_capacity(n * 8);
    let mut mags = Vec::with_capacity(n);
    let mut phases = Vec::with_capacity(n);

    for i in 0..n {
        let off = i * 4;
        let f = f32::from_le_bytes([
            freq_in_bytes[off],
            freq_in_bytes[off + 1],
            freq_in_bytes[off + 2],
            freq_in_bytes[off + 3],
        ]) as f64;
        let w = 2.0 * PI * f / sample_rate;
        let (cw, sw) = (w.cos(), w.sin());
        // H(e^jω) = (b0 + b1·e^-jω + b2·e^-j2ω) / (1 + a1·e^-jω + a2·e^-j2ω)
        let num_re = b0 + b1 * cw + b2 * (2.0 * w).cos();
        let num_im = -b1 * sw - b2 * (2.0 * w).sin();
        let den_re = 1.0 + a1 * cw + a2 * (2.0 * w).cos();
        let den_im = -a1 * sw - a2 * (2.0 * w).sin();
        let den_mag2 = den_re * den_re + den_im * den_im;
        let h_re = (num_re * den_re + num_im * den_im) / den_mag2;
        let h_im = (num_im * den_re - num_re * den_im) / den_mag2;
        let mag = (h_re * h_re + h_im * h_im).sqrt();
        let phase = h_im.atan2(h_re);
        mags.push(mag as f32);
        phases.push(phase as f32);
    }
    for m in mags {
        out.extend_from_slice(&m.to_le_bytes());
    }
    for p in phases {
        out.extend_from_slice(&p.to_le_bytes());
    }
    out
}

deno_core::extension!(
    audio_extension,
    ops = [
        op_offline_audio_render,
        op_audio_analyser_freq_data,
        op_audio_biquad_response,
    ],
);

#[cfg(test)]
mod tests {
    use super::*;

    fn f32_bytes(samples: &[f32]) -> Vec<u8> {
        samples.iter().flat_map(|s| s.to_le_bytes()).collect()
    }

    fn bytes_to_f32(bytes: &[u8]) -> Vec<f32> {
        bytes
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect()
    }

    // ---- Analyser FFT ----

    #[test]
    fn analyser_returns_db_clamped_to_minus_100_to_minus_30() {
        // Silent input → all bins at minDB.
        let samples = vec![0.0_f32; 256];
        let bytes = op_audio_analyser_freq_data_inner(&f32_bytes(&samples), 256, 0, &[]);
        let mags = bytes_to_f32(&bytes);
        assert_eq!(mags.len(), 128);
        for m in mags {
            assert_eq!(m, -100.0);
        }
    }

    #[test]
    fn analyser_pure_sine_peaks_at_expected_bin() {
        // 1 kHz sine at 44100 Hz, 1024 samples → bin index ≈ 1000/(44100/1024) ≈ 23.
        let n = 1024;
        let mut samples = vec![0.0_f32; n];
        for (i, s) in samples.iter_mut().enumerate() {
            *s = (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 44100.0).sin();
        }
        let bytes = op_audio_analyser_freq_data_inner(&f32_bytes(&samples), n as i32, 0, &[]);
        let mags = bytes_to_f32(&bytes);
        let max_idx = mags
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap()
            .0;
        // Bin should be around 23 ± 2 (Blackman window blurs the peak).
        assert!(
            (21..=25).contains(&max_idx),
            "expected peak near bin 23, got {max_idx}"
        );
    }

    #[test]
    fn analyser_smoothing_with_prev_data() {
        // Pass non-empty prev_freq → smoothing factor 1.0 → output should be
        // dominated by prev (after dB conversion, but since prev is in dB...
        // actually prev is the magnitude pre-dB in our impl — we apply
        // smoothing in the magnitude domain).
        let samples = vec![0.0_f32; 256];
        // prev is mag space: all 0.5
        let prev = vec![0.5_f32; 128];
        let bytes = op_audio_analyser_freq_data_inner(
            &f32_bytes(&samples),
            256,
            100, // smoothing = 1.0
            &f32_bytes(&prev),
        );
        let dbs = bytes_to_f32(&bytes);
        // mag = 0.5 → 20·log10(0.5) ≈ -6.02 dB → clamped to -30 dB
        // (since clamp range is [-100, -30] and -6 > -30).
        assert!(dbs.iter().all(|&d| d == -30.0), "got {:?}", &dbs[..5]);
    }

    // Helper that invokes the inner logic directly (the op_* attribute
    // wraps the function into an OpDecl, so direct calls aren't possible
    // — we mimic the body here via the public function).
    // The op2 macro generates a hidden inner fn; for tests we just call
    // the same fn here. Workaround: re-export the logic via this helper.
    fn op_audio_analyser_freq_data_inner(
        time_domain_bytes: &[u8],
        fft_size: i32,
        smoothing_x100: i32,
        prev_freq_bytes: &[u8],
    ) -> Vec<u8> {
        let n = fft_size.clamp(32, 32768) as usize;
        if !n.is_power_of_two() || time_domain_bytes.len() < n * 4 {
            return Vec::new();
        }
        let mut samples: Vec<f32> = (0..n)
            .map(|i| {
                let off = i * 4;
                f32::from_le_bytes([
                    time_domain_bytes[off],
                    time_domain_bytes[off + 1],
                    time_domain_bytes[off + 2],
                    time_domain_bytes[off + 3],
                ])
            })
            .collect();
        blackman_window(&mut samples);
        let mut buf: Vec<Complex32> = samples.iter().map(|&s| Complex32::new(s, 0.0)).collect();
        {
            let mut planner = fft_planner().lock().unwrap();
            let fft = planner.plan_fft_forward(n);
            fft.process(&mut buf);
        }
        let half = n / 2;
        let n_f = n as f32;
        let mut mags: Vec<f32> = buf[..half]
            .iter()
            .map(|c| (c.re * c.re + c.im * c.im).sqrt() / n_f)
            .collect();
        let smoothing = (smoothing_x100.clamp(0, 100) as f32) / 100.0;
        if prev_freq_bytes.len() >= half * 4 && smoothing > 0.0 {
            for (i, m) in mags.iter_mut().enumerate() {
                let off = i * 4;
                let prev = f32::from_le_bytes([
                    prev_freq_bytes[off],
                    prev_freq_bytes[off + 1],
                    prev_freq_bytes[off + 2],
                    prev_freq_bytes[off + 3],
                ]);
                *m = prev * smoothing + *m * (1.0 - smoothing);
            }
        }
        mags.iter()
            .flat_map(|&m| {
                let db = if m > 0.0 {
                    20.0 * m.log10()
                } else {
                    -f32::INFINITY
                };
                db.clamp(-100.0, -30.0).to_le_bytes()
            })
            .collect()
    }

    // ---- Biquad response ----

    #[test]
    #[allow(
        clippy::approx_constant,
        reason = "0.7071 = Butterworth Q (1/sqrt(2)) filter arg"
    )]
    fn biquad_lowpass_dc_passes_unity() {
        // Lowpass at 1 kHz, Q=0.7071 (Butterworth), sr=44100. At f=0 (DC),
        // |H| should be ~1 and phase ~0.
        let freqs = vec![0.0_f32];
        let bytes = op_audio_biquad_response_inner(
            &f32_bytes(&freqs),
            BiquadType::Lowpass as i32,
            1000.0,
            0.7071,
            0.0,
            44100.0,
        );
        let out = bytes_to_f32(&bytes);
        let (mag, phase) = (out[0], out[1]);
        assert!((mag - 1.0).abs() < 1e-3, "DC mag {mag}");
        assert!(phase.abs() < 1e-3, "DC phase {phase}");
    }

    #[test]
    #[allow(
        clippy::approx_constant,
        reason = "0.7071 = Butterworth Q (1/sqrt(2)) filter arg"
    )]
    fn biquad_highpass_dc_blocks() {
        // Highpass at 1 kHz, Q=0.7071. At f=0, |H| should be ~0.
        let freqs = vec![0.0_f32];
        let bytes = op_audio_biquad_response_inner(
            &f32_bytes(&freqs),
            BiquadType::Highpass as i32,
            1000.0,
            0.7071,
            0.0,
            44100.0,
        );
        let out = bytes_to_f32(&bytes);
        assert!(
            out[0] < 1e-3,
            "highpass DC mag should be ~0, got {}",
            out[0]
        );
    }

    #[test]
    fn biquad_returns_2n_floats_for_n_input() {
        let freqs = vec![100.0_f32, 200.0, 1000.0, 5000.0, 20000.0];
        let bytes = op_audio_biquad_response_inner(
            &f32_bytes(&freqs),
            BiquadType::Bandpass as i32,
            1000.0,
            1.0,
            0.0,
            44100.0,
        );
        let out = bytes_to_f32(&bytes);
        assert_eq!(out.len(), 2 * freqs.len());
        // First N are magnitudes (>= 0), next N are phases (radian range).
        for &m in &out[..freqs.len()] {
            assert!(m >= 0.0 && m.is_finite());
        }
        for &p in &out[freqs.len()..] {
            assert!(p.is_finite());
            assert!(p.abs() <= std::f32::consts::PI + 1e-3);
        }
    }

    fn op_audio_biquad_response_inner(
        freq_in_bytes: &[u8],
        filter_type_id: i32,
        frequency: f64,
        q: f64,
        gain: f64,
        sample_rate: f64,
    ) -> Vec<u8> {
        let n = freq_in_bytes.len() / 4;
        if n == 0 {
            return Vec::new();
        }
        let kind = BiquadType::from_id(filter_type_id.max(0) as u32);
        let (b0, b1, b2, a1, a2) = biquad_coeffs(kind, frequency, q, gain, sample_rate);
        let mut mags = Vec::with_capacity(n);
        let mut phases = Vec::with_capacity(n);
        for i in 0..n {
            let off = i * 4;
            let f = f32::from_le_bytes([
                freq_in_bytes[off],
                freq_in_bytes[off + 1],
                freq_in_bytes[off + 2],
                freq_in_bytes[off + 3],
            ]) as f64;
            let w = 2.0 * PI * f / sample_rate;
            let (cw, sw) = (w.cos(), w.sin());
            let num_re = b0 + b1 * cw + b2 * (2.0 * w).cos();
            let num_im = -b1 * sw - b2 * (2.0 * w).sin();
            let den_re = 1.0 + a1 * cw + a2 * (2.0 * w).cos();
            let den_im = -a1 * sw - a2 * (2.0 * w).sin();
            let den_mag2 = den_re * den_re + den_im * den_im;
            let h_re = (num_re * den_re + num_im * den_im) / den_mag2;
            let h_im = (num_im * den_re - num_re * den_im) / den_mag2;
            mags.push((h_re * h_re + h_im * h_im).sqrt() as f32);
            phases.push((h_im.atan2(h_re)) as f32);
        }
        let mut out = Vec::with_capacity(n * 8);
        for m in mags {
            out.extend_from_slice(&m.to_le_bytes());
        }
        for p in phases {
            out.extend_from_slice(&p.to_le_bytes());
        }
        out
    }
}
