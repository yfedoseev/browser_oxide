//! Human-like input simulation.
//!
//! Anti-bot ML models (Kasada, PerimeterX/HUMAN, Akamai sensor) detect
//! single-Bezier mouse paths with ~99% accuracy via Sigma-Lognormal stroke
//! decomposition. We use `stealth::behavior::mouse_trajectory` (Plamondon
//! 1995) which produces 2-7 lognormal velocity strokes — same generator
//! BeCAPTCHA-Mouse benchmarks against.
//!
//! See docs/SOTA_ROADMAP_2026.md §3.1.

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

/// Generate a cubic Bezier mouse path with human-like characteristics.
fn generate_bezier_path(x1: f64, y1: f64, x2: f64, y2: f64, steps: u32) -> Vec<MousePoint> {
    let mut points = Vec::with_capacity(steps as usize);

    let dx = x2 - x1;
    let dy = y2 - y1;
    let dist = (dx * dx + dy * dy).sqrt();

    // Random control points for the Bezier curve
    // Use a simple LCG PRNG seeded from coordinates (deterministic per path)
    let mut rng = SimpleRng::new(
        ((x1 * 1000.0 + y1 * 7.0 + x2 * 13.0 + y2 * 19.0) as u64).wrapping_mul(6364136223846793005),
    );

    // Control point 1: offset perpendicular to the line
    let perp_x = -dy / dist.max(1.0);
    let perp_y = dx / dist.max(1.0);
    let cp1_offset = (rng.next_f64() - 0.5) * dist * 0.3;
    let cp1x = x1 + dx * 0.25 + perp_x * cp1_offset;
    let cp1y = y1 + dy * 0.25 + perp_y * cp1_offset;

    // Control point 2: slight overshoot toward target
    let cp2_offset = (rng.next_f64() - 0.5) * dist * 0.2;
    let cp2x = x1 + dx * 0.75 + perp_x * cp2_offset;
    let cp2y = y1 + dy * 0.75 + perp_y * cp2_offset;

    // Base timing: faster for short distances, slower for long
    let base_delay = 2.0 + (dist / 500.0) * 5.0; // 2-7ms per step

    for i in 0..steps {
        let t = i as f64 / (steps - 1) as f64;

        // Ease-in-out (acceleration at start, deceleration at end)
        let eased = if t < 0.5 {
            2.0 * t * t
        } else {
            -1.0 + (4.0 - 2.0 * t) * t
        };

        // Cubic Bezier: B(t) = (1-t)^3*P0 + 3*(1-t)^2*t*P1 + 3*(1-t)*t^2*P2 + t^3*P3
        let it = 1.0 - eased;
        let x = it * it * it * x1
            + 3.0 * it * it * eased * cp1x
            + 3.0 * it * eased * eased * cp2x
            + eased * eased * eased * x2;
        let y = it * it * it * y1
            + 3.0 * it * it * eased * cp1y
            + 3.0 * it * eased * eased * cp2y
            + eased * eased * eased * y2;

        // Add micro-jitter (1-2px noise)
        let jitter_x = (rng.next_f64() - 0.5) * 2.0;
        let jitter_y = (rng.next_f64() - 0.5) * 2.0;

        // Variable delay (slower at start/end, faster in middle)
        let speed_factor = 1.0 + 0.5 * (1.0 - (2.0 * t - 1.0).abs());
        let delay = base_delay / speed_factor + rng.next_f64() * 2.0;

        points.push(MousePoint {
            x: (x + jitter_x).round(),
            y: (y + jitter_y).round(),
            delay_ms: delay,
        });
    }

    // Ensure last point is exactly the target
    if let Some(last) = points.last_mut() {
        last.x = x2;
        last.y = y2;
    }

    points
}

/// Generate realistic inter-key typing delays.
fn generate_typing_delays(text: &str, base_wpm: f64) -> Vec<f64> {
    let base_ms = 60_000.0 / (base_wpm.max(10.0) * 5.0); // ms per character

    let mut rng = SimpleRng::new(text.len() as u64 * 7 + 42);
    let mut delays = Vec::with_capacity(text.len());
    let mut prev_char = ' ';

    for ch in text.chars() {
        let mut delay = base_ms;

        // Variation: ±30%
        delay *= 0.7 + rng.next_f64() * 0.6;

        // Slower for: uppercase (shift key), punctuation, numbers
        if ch.is_uppercase() {
            delay *= 1.3;
        }
        if ch.is_ascii_punctuation() {
            delay *= 1.5;
        }
        if ch == ' ' {
            delay *= 0.8; // Spacebar is fast
        }

        // Consecutive same-hand keys are faster
        if same_hand(prev_char, ch) {
            delay *= 0.9;
        }

        // Occasional pause (thinking)
        if rng.next_f64() < 0.03 {
            delay += 200.0 + rng.next_f64() * 300.0;
        }

        delays.push(delay);
        prev_char = ch;
    }

    delays
}

fn same_hand(a: char, b: char) -> bool {
    let left = "qwertasdfgzxcvb";
    let right = "yuiophjklnm";
    let a = a.to_ascii_lowercase();
    let b = b.to_ascii_lowercase();
    (left.contains(a) && left.contains(b)) || (right.contains(a) && right.contains(b))
}

/// Simple LCG PRNG (no external dependencies)
struct SimpleRng(u64);

impl SimpleRng {
    fn new(seed: u64) -> Self {
        Self(seed.wrapping_add(1))
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }

    fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

deno_core::extension!(
    input_extension,
    ops = [op_human_mouse_path, op_human_typing_delays],
);
