//! Web Audio ops — `op_offline_audio_render` runs Blink's
//! DynamicsCompressorKernel (Rust port in `canvas::audio`) so the JS
//! OfflineAudioContext in `canvas_bootstrap.js` no longer needs to duplicate
//! the compressor math inline. This is the entry point T1.3 replaces.

use canvas::{AudioFingerprint, AudioParams, WaveType};
use deno_core::op2;

/// Render the default CreepJS/FingerprintJS audio probe pipeline:
/// `OfflineAudioContext(1, length, sample_rate)` → triangle osc at
/// `frequency` Hz → DynamicsCompressor with the parameters the sensor set →
/// destination. Returns the rendered samples as a Float32 buffer (f32 little-
/// endian bytes) which the JS side reinterprets as a `Float32Array`.
#[allow(clippy::too_many_arguments)]
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

    let params = AudioParams {
        sample_rate: sample_rate.max(0) as u32,
        length: length.max(0) as u32,
        frequency,
        wave_type,
        threshold: threshold_db,
        knee: knee_db,
        ratio,
        attack: attack_seconds,
        release: release_seconds,
    };

    // Convert seed from signed i32 to u64 (preserve bit pattern via as).
    let seed_u64: u64 = seed as u32 as u64;
    let fp = AudioFingerprint::from_params(seed_u64, params);

    // Pack Float32 samples as little-endian bytes. JS side reconstructs via
    // `new Float32Array(new Uint8Array(bytes).buffer)`.
    let mut bytes = Vec::with_capacity(fp.data.len() * 4);
    for s in &fp.data {
        bytes.extend_from_slice(&s.to_le_bytes());
    }
    bytes
}

deno_core::extension!(audio_extension, ops = [op_offline_audio_render],);
