//! Human-like input simulation.
//!
//! Anti-bot ML models (Kasada, PerimeterX/HUMAN, Akamai sensor) detect
//! single-Bezier mouse paths with ~99% accuracy via Sigma-Lognormal stroke
//! decomposition. We use `stealth::behavior::mouse_trajectory` (Plamondon
//! 1995) which produces 2-7 lognormal velocity strokes — same generator
//! BeCAPTCHA-Mouse benchmarks against.

use deno_core::op2;
use serde::Serialize;
use stealth::BehaviorProfile;

/// Generate a humanlike mouse path from (x1,y1) to (x2,y2).
///
/// Uses Sigma-Lognormal stroke synthesis. The `steps` parameter is now
/// **ignored** — sample rate is fixed at 8 ms (125 Hz, real USB pointer
/// rate). Total movement time obeys Fitts's Law: T = 230 + 166·log2(D/W+1).
/// `target_w` is the target width in pixels (defaults to 30 px when 0).
#[op2]
#[serde]
pub fn op_human_mouse_path(
    #[smi] x1: i32,
    #[smi] y1: i32,
    #[smi] x2: i32,
    #[smi] y2: i32,
    #[smi] _steps: i32,
) -> Vec<MousePoint> {
    let profile = BehaviorProfile::default();
    let target_w = 30.0_f32;
    let pts = stealth::behavior::mouse_trajectory(
        (x1 as f32, y1 as f32),
        (x2 as f32, y2 as f32),
        target_w,
        &profile,
    );

    // Convert behavior::MousePoint{t_ms, x, y} → input_ext::MousePoint
    // {x, y, delay_ms}. delay_ms = inter-sample gap (8 ms for all but the
    // first sample, which has delay_ms=0).
    let mut out = Vec::with_capacity(pts.len());
    let mut prev_t = 0.0_f32;
    for (i, p) in pts.iter().enumerate() {
        let delay_ms = if i == 0 {
            0.0
        } else {
            (p.t_ms - prev_t) as f64
        };
        prev_t = p.t_ms;
        out.push(MousePoint {
            x: p.x as f64,
            y: p.y as f64,
            delay_ms,
        });
    }
    out
}

/// Generate human-like key timing for a string.
///
/// Returns inter-key delays in milliseconds: each entry is dwell+flight
/// for that character (the time from previous keyup to this character's
/// keyup). First entry is dwell only. Powered by
/// `stealth::behavior::keystroke_timings` (CMU+Buffalo bigram-aware
/// LogNormal — gap #31c).
#[op2]
#[serde]
pub fn op_human_typing_delays(#[string] text: &str, #[smi] base_wpm: i32) -> Vec<f64> {
    let mut profile = BehaviorProfile::default();
    if base_wpm > 0 {
        profile.typing_wpm_mean = base_wpm as f32;
    }
    let timings = stealth::behavior::keystroke_timings(text, &profile);
    timings
        .into_iter()
        .map(|k| (k.flight_ms + k.dwell_ms) as f64)
        .collect()
}

#[derive(Serialize)]
pub struct MousePoint {
    pub x: f64,
    pub y: f64,
    pub delay_ms: f64,
}

deno_core::extension!(
    input_extension,
    ops = [op_human_mouse_path, op_human_typing_delays],
);
