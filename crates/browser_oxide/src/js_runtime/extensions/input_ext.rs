//! Human-like input simulation.
//!
//! Behavioral classifiers can distinguish single-Bezier mouse paths from
//! human motion with high accuracy via Sigma-Lognormal stroke
//! decomposition. We use `crate::stealth::behavior::mouse_trajectory` (Plamondon
//! 1995) which produces 2-7 lognormal velocity strokes — same generator
//! BeCAPTCHA-Mouse benchmarks against.

use crate::stealth::BehaviorProfile;
use deno_core::op2;
use deno_core::OpState;
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};
use serde::Serialize;

/// Per-page seeded RNG state for `op_behavior_random`. Replaces
/// `Math.random()` in humanize.js so synthetic mouse/scroll/key event
/// streams are deterministic per page lifetime (two-level seed pattern —
/// session-stable, page-deterministic).
///
/// Seed source priority (constructor):
///   1. BROWSER_OXIDE_BEHAVIOR_SEED env var (decimal u64) — for tests +
///      reproducibility-pinned harnesses.
///   2. `rand::random::<u64>()` — fresh per page, so two unrelated visits
///      don't produce identical event streams.
pub struct BehaviorRngState {
    rng: StdRng,
}

impl BehaviorRngState {
    pub fn new(seed: u64) -> Self {
        Self {
            rng: StdRng::seed_from_u64(seed),
        }
    }
    pub fn from_env_or_random() -> Self {
        let seed = std::env::var("BROWSER_OXIDE_BEHAVIOR_SEED")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or_else(rand::random::<u64>);
        Self::new(seed)
    }
}

impl Default for BehaviorRngState {
    fn default() -> Self {
        Self::from_env_or_random()
    }
}

/// Per-page seeded random in [0, 1) — drop-in `Math.random` substitute
/// for humanize.js. Each call advances the per-page ChaCha12 stream so
/// the sequence is reproducible given a fixed seed yet not
/// fingerprintable across pages (each page gets its own seed by default).
#[op2(fast)]
pub fn op_behavior_random(s: &mut OpState) -> f64 {
    let s = s.borrow_mut::<BehaviorRngState>();
    s.rng.random::<f64>()
}

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
    let pts = crate::stealth::behavior::mouse_trajectory(
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
/// `crate::stealth::behavior::keystroke_timings` (CMU+Buffalo bigram-aware
/// LogNormal).
#[op2]
#[serde]
pub fn op_human_typing_delays(#[string] text: &str, #[smi] base_wpm: i32) -> Vec<f64> {
    let mut profile = BehaviorProfile::default();
    if base_wpm > 0 {
        profile.typing_wpm_mean = base_wpm as f32;
    }
    let timings = crate::stealth::behavior::keystroke_timings(text, &profile);
    timings
        .into_iter()
        .map(|k| (k.flight_ms + k.dwell_ms) as f64)
        .collect()
}

#[derive(Serialize)]
pub struct KeystrokeSlot {
    /// Printable key (e.g. "a", "A", "1", " ").
    pub key: String,
    /// W3C UI Events `KeyboardEvent.code` (e.g. "KeyA", "Digit1", "Space").
    pub code: String,
    /// Cumulative ms from schedule start when `keydown` should fire.
    pub down_ms: f64,
    /// Cumulative ms from schedule start when `keyup` should fire.
    pub up_ms: f64,
}

fn char_to_code(c: char) -> String {
    match c {
        'a'..='z' | 'A'..='Z' => format!("Key{}", c.to_ascii_uppercase()),
        '0'..='9' => format!("Digit{c}"),
        ' ' => "Space".to_string(),
        '\n' => "Enter".to_string(),
        '\t' => "Tab".to_string(),
        _ => "Unidentified".to_string(),
    }
}

/// Rich keystroke schedule for `text`: per-char `keydown` + `keyup`
/// timings as cumulative ms from start. Powered by the same
/// `crate::stealth::behavior::keystroke_timings` (CMU + Buffalo bigram-aware
/// LogNormal) but exposes the dwell/flight split so JS can dispatch
/// real `KeyboardEvent`s at the right wall-clock offsets — wiring the
/// generator that already exists but previously had no consumer.
#[op2]
#[serde]
pub fn op_human_keystroke_schedule(
    #[string] text: &str,
    #[smi] base_wpm: i32,
) -> Vec<KeystrokeSlot> {
    let mut profile = BehaviorProfile::default();
    if base_wpm > 0 {
        profile.typing_wpm_mean = base_wpm as f32;
    }
    let timings = crate::stealth::behavior::keystroke_timings(text, &profile);
    let mut acc = 0.0_f64;
    let mut out = Vec::with_capacity(timings.len());
    for k in timings {
        acc += k.flight_ms as f64;
        let down = acc;
        let up = acc + k.dwell_ms as f64;
        acc = up;
        out.push(KeystrokeSlot {
            key: k.ch.to_string(),
            code: char_to_code(k.ch),
            down_ms: down,
            up_ms: up,
        });
    }
    out
}

#[derive(Serialize)]
pub struct MousePoint {
    pub x: f64,
    pub y: f64,
    pub delay_ms: f64,
}

deno_core::extension!(
    input_extension,
    ops = [
        op_human_mouse_path,
        op_human_typing_delays,
        op_human_keystroke_schedule,
        op_behavior_random
    ],
);
