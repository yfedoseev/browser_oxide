//! Human-like input simulation — Bezier curve mouse movements, variable typing speed.
//!
//! Anti-bot ML models detect straight-line mouse paths and constant typing speed.
//! This module generates realistic trajectories with:
//! - Cubic Bezier curves with random control points
//! - Acceleration/deceleration (ease-in-out)
//! - Micro-corrections and overshoot
//! - Variable inter-key timing for typing

use deno_core::op2;
use serde::Serialize;

/// Generate a human-like mouse path from (x1,y1) to (x2,y2).
/// Returns a series of (x, y, delay_ms) points.
#[op2]
#[serde]
pub fn op_human_mouse_path(
    #[smi] x1: i32,
    #[smi] y1: i32,
    #[smi] x2: i32,
    #[smi] y2: i32,
    #[smi] steps: i32,
) -> Vec<MousePoint> {
    generate_bezier_path(
        x1 as f64,
        y1 as f64,
        x2 as f64,
        y2 as f64,
        (steps.max(5)) as u32,
    )
}

/// Generate human-like key timing for a string.
/// Returns inter-key delays in milliseconds.
#[op2]
#[serde]
pub fn op_human_typing_delays(#[string] text: &str, #[smi] base_wpm: i32) -> Vec<f64> {
    generate_typing_delays(text, base_wpm as f64)
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
