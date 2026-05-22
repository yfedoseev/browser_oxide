//! Validates the `AudioFingerprint` output against Chrome's reference value
//! for the CreepJS/FingerprintJS audio probe. Chrome 130 on the standard
//! OfflineAudioContext(1, 5000, 44100) with triangle 10kHz oscillator into
//! the default DynamicsCompressor produces:
//!
//!     sum(abs(data[4500..5000])) = 124.04347527516074

use canvas::AudioFingerprint;

const CHROME_REFERENCE_SUM: f64 = 124.04347527516074;

/// Scans input sine amplitude to find the value that makes the tail sum
/// match Chrome's reference. Backs out what the Blink wavetable oscillator
/// effectively delivers after band-limiting.
#[test]
fn scan_amplitude_for_chrome_match() {
    let sr = 44100u32;
    let len = 5000usize;
    let freq = 10000.0_f32;
    let omega = 2.0 * std::f32::consts::PI * freq / sr as f32;

    println!("\n=== amplitude scan ===");
    println!("  target sum = {CHROME_REFERENCE_SUM}");

    let mut best_delta = f64::INFINITY;
    let mut best_amp = 0.0_f32;
    let mut best_sum = 0.0_f64;

    // Fine scan around the sine sweet spot
    for step in 0..=1000 {
        let amp = 0.40 + (step as f32) * 0.0002;
        let input: Vec<f32> = (0..len).map(|i| amp * (omega * i as f32).sin()).collect();
        let out = AudioFingerprint::compress_debug(&input, sr);
        let sum: f64 = out[4500..5000].iter().map(|s| (*s as f64).abs()).sum();
        let delta = (sum - CHROME_REFERENCE_SUM).abs();
        if delta < best_delta {
            best_delta = delta;
            best_amp = amp;
            best_sum = sum;
        }
    }
    println!("  best amp  = {best_amp}  sum = {best_sum}  delta = {best_delta}");
    println!(
        "  8/π² ≈ {}",
        8.0 / (std::f32::consts::PI * std::f32::consts::PI)
    );
    println!("  2/π ≈ {}", 2.0 / std::f32::consts::PI);

    // Try a naive triangle wave (not band-limited) — for a peak of 1.0 it
    // has lower RMS than a sine of the same peak.
    println!("\n  === naive triangle (amplitude sweep) ===");
    let mut best_tri_delta = f64::INFINITY;
    let mut best_tri_amp = 0.0_f32;
    let mut best_tri_sum = 0.0_f64;
    for step in 0..=120 {
        let amp = 0.30 + (step as f32) * 0.01;
        let input: Vec<f32> = (0..len)
            .map(|i| {
                let phase = (omega * i as f32) / (2.0 * std::f32::consts::PI);
                let frac = phase - phase.floor();
                let sample = if frac < 0.25 {
                    4.0 * frac
                } else if frac < 0.75 {
                    2.0 - 4.0 * frac
                } else {
                    -4.0 + 4.0 * frac
                };
                amp * sample
            })
            .collect();
        let out = AudioFingerprint::compress_debug(&input, sr);
        let sum: f64 = out[4500..5000].iter().map(|s| (*s as f64).abs()).sum();
        let delta = (sum - CHROME_REFERENCE_SUM).abs();
        if delta < best_tri_delta {
            best_tri_delta = delta;
            best_tri_amp = amp;
            best_tri_sum = sum;
        }
    }
    println!("  best triangle amp = {best_tri_amp} sum = {best_tri_sum} delta = {best_tri_delta}");

    // Try a full Fourier-summed triangle with all harmonics under Nyquist
    // (three harmonics at 44.1 kHz rate: 10 kHz, 30 kHz (above!), so really
    // only 10 kHz). But sweep the full FOURIER form too for completeness.
    println!("\n  === Fourier triangle (first 10 odd harmonics, no aliasing guard) ===");
    let mut best_fs_delta = f64::INFINITY;
    let mut best_fs_amp = 0.0_f32;
    let mut best_fs_sum = 0.0_f64;
    for step in 0..=120 {
        let amp = 0.30 + (step as f32) * 0.01;
        let input: Vec<f32> = (0..len)
            .map(|i| {
                let t = i as f32;
                let mut s = 0.0_f32;
                for k in 0..10 {
                    let n = (2 * k + 1) as f32;
                    let sign = if k % 2 == 0 { 1.0 } else { -1.0 };
                    s += sign * (n * omega * t).sin() / (n * n);
                }
                amp * (8.0 / (std::f32::consts::PI * std::f32::consts::PI)) * s
            })
            .collect();
        let out = AudioFingerprint::compress_debug(&input, sr);
        let sum: f64 = out[4500..5000].iter().map(|s| (*s as f64).abs()).sum();
        let delta = (sum - CHROME_REFERENCE_SUM).abs();
        if delta < best_fs_delta {
            best_fs_delta = delta;
            best_fs_amp = amp;
            best_fs_sum = sum;
        }
    }
    println!("  best Fourier amp = {best_fs_amp} sum = {best_fs_sum} delta = {best_fs_delta}");
}

/// Fine-grained scan of `BLINK_OSCILLATOR_SCALE` using the real
/// `PeriodicWave → DynamicsCompressor` pipeline (via
/// `AudioFingerprint::from_params`). Surfaces the exact scale factor
/// that closes the residual 50 ppm delta.
#[test]
#[ignore = "calibration helper — run on demand when tuning T1.3b"]
fn fine_scan_blink_oscillator_scale() {
    use canvas::{AudioFingerprint, AudioParams, WaveType};

    // Re-implement `from_params` here but with a tunable scale so we
    // can sweep without touching the library constant. Matches the
    // internal pipeline exactly.
    fn render_with_scale(scale: f32) -> Vec<f32> {
        use canvas::periodic_wave::{PeriodicWave, StandardWaveType};
        let sample_rate = 44100u32;
        let length = 5000usize;
        let padded_len = length.div_ceil(32) * 32;
        let wave = PeriodicWave::new(StandardWaveType::Triangle, sample_rate as f32);
        let frequency = 10000.0_f32;
        let phase_increment = frequency / sample_rate as f32;
        let mut phase = 0.0_f32;
        let mut input = vec![0.0f32; padded_len];
        for slot in input.iter_mut().take(length) {
            *slot = wave.sample(phase, frequency) * scale;
            phase += phase_increment;
            if phase >= 1.0 {
                phase -= 1.0;
            }
        }
        let mut out = AudioFingerprint::compress_debug(&input, sample_rate);
        out.truncate(length);
        out
    }

    const TARGET: f64 = 124.04347527516074;
    let mut best_delta = f64::INFINITY;
    let mut best_scale = 0.0_f32;
    let mut best_sum = 0.0_f64;
    // 0.47600..=0.47640 in steps of 5e-6 (~ppm resolution).
    for step in 0..=80 {
        let scale = 0.47600 + step as f32 * 5.0e-6;
        let out = render_with_scale(scale);
        let sum: f64 = out[4500..5000].iter().map(|s| (*s as f64).abs()).sum();
        let delta = (sum - TARGET).abs();
        if delta < best_delta {
            best_delta = delta;
            best_scale = scale;
            best_sum = sum;
        }
    }
    println!("\n=== blink-oscillator-scale fine scan ===");
    println!("  best scale = {best_scale}");
    println!("  best sum   = {best_sum}");
    println!("  best delta = {best_delta}");
    println!("  ppm        = {}", best_delta / TARGET * 1e6);
    // Use `_ = (best_scale, best_sum);` via the printlns.
    let _ = AudioParams {
        wave_type: WaveType::Triangle,
        ..Default::default()
    };
}

#[test]
fn reports_current_sum() {
    let fp = AudioFingerprint::default_fingerprint();
    assert_eq!(fp.data.len(), 5000);
    let sum: f64 = fp.data[4500..5000].iter().map(|&s| (s as f64).abs()).sum();
    let delta = (sum - CHROME_REFERENCE_SUM).abs();
    let rel = delta / CHROME_REFERENCE_SUM;

    let peak_full: f32 = fp.data.iter().copied().fold(0.0_f32, |a, b| a.max(b.abs()));
    let peak_tail: f32 = fp.data[4500..5000]
        .iter()
        .copied()
        .fold(0.0_f32, |a, b| a.max(b.abs()));
    let mean_tail_abs: f64 = fp.data[4500..5000]
        .iter()
        .map(|s| (*s as f64).abs())
        .sum::<f64>()
        / 500.0;

    println!("\n=== AudioFingerprint (seed=0) reference check ===");
    println!("  our   sum(abs(data[4500..5000])) = {sum}");
    println!("  chrome                            = {CHROME_REFERENCE_SUM}");
    println!("  absolute delta                    = {delta}");
    println!("  relative delta                    = {rel}");
    println!("  full buffer peak                  = {peak_full}");
    println!("  tail peak                         = {peak_tail}");
    println!("  tail mean(|x|)                    = {mean_tail_abs}");
    println!(
        "  chrome tail mean(|x|)             = {}",
        CHROME_REFERENCE_SUM / 500.0
    );
    println!(
        "  first 5 tail samples              : {:?}",
        &fp.data[4500..4505]
    );
    println!(
        "  last 5 tail samples               : {:?}",
        &fp.data[4995..5000]
    );
}

#[test]
fn scan_threshold_for_chrome_parity() {
    use canvas::{AudioFingerprint, AudioParams};
    let _sr = 44100u32;
    let _len = 5000usize;

    println!("\n=== threshold scan ===");
    for threshold in [-24.0, -30.0, -40.0, -50.0] {
        let params = AudioParams {
            threshold,
            ..Default::default()
        };
        let fp = AudioFingerprint::from_params(0, params);
        let sum: f64 = fp.data[4500..5000].iter().map(|&s| (s as f64).abs()).sum();
        println!("  threshold = {threshold:5.1}  sum = {sum}");
    }
}
