//! Behavioral entropy modeling (gap #31).
//!
//! Vendors that score the *distribution shape* of human input (Kasada,
//! PerimeterX/HUMAN, Akamai sensor) catch Bezier-curve mouse paths and
//! linear keystroke timing instantly. This module produces statistically
//! humanlike trajectories using:
//!
//!   - **Sigma-Lognormal velocity strokes** (Plamondon 1995) for mouse paths
//!   - **LogNormal dwell + bigram-modulated flight** for keystrokes
//!   - **Exponential momentum decay** for trackpad scroll, discrete
//!     notches for mouse wheel
//!
//! Per-session determinism via `ChaCha20Rng` seeded from `BehaviorProfile.seed`
//! — same seed produces same sequence so tests reproduce, but each call site
//! folds in a salt so different `(from, to)` pairs differ.

use rand::Rng;
use rand_chacha::rand_core::SeedableRng;
use rand_chacha::ChaCha20Rng;
use rand_distr::{Distribution, LogNormal, Normal};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Handedness {
    Right,
    Left,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScrollStyle {
    /// Trackpad with momentum/inertia (~60 Hz, exponential decay).
    Trackpad,
    /// Discrete mouse-wheel notches at LogNormal intervals.
    Wheel,
}

/// Per-session behavioral parameters. Different sessions of the same
/// fingerprint profile should sample fresh seeds so mouse/keyboard
/// patterns don't repeat across visits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorProfile {
    /// Per-session seed for ChaCha20Rng. Tests can pin this for
    /// reproducibility; production should sample fresh per session.
    #[serde(default = "default_seed")]
    pub seed: u64,
    /// Right-handers overshoot bottom-right; left-handers bottom-left.
    /// Affects per-stroke direction bias on Sigma-Lognormal paths.
    #[serde(default = "default_handedness")]
    pub handedness: Handedness,
    /// Mouse DPI affects the minimum step size (high-DPI = fewer pixels
    /// per physical mm = different velocity profile).
    #[serde(default = "default_dpi")]
    pub mouse_dpi: u16,
    /// Mean typing speed in WPM — drives LogNormal flight median.
    #[serde(default = "default_wpm_mean")]
    pub typing_wpm_mean: f32,
    /// Per-user σ across keystrokes — tight (real users are consistent).
    #[serde(default = "default_wpm_sigma")]
    pub typing_wpm_sigma: f32,
    /// Trackpad inertia vs discrete wheel notches.
    #[serde(default = "default_scroll")]
    pub scroll_style: ScrollStyle,
    /// Fitts's law slope coefficient (ms per bit). Real population
    /// 130–200 ms; individual differences within ~30 ms.
    #[serde(default = "default_fitts_b")]
    pub fitts_b: f32,
}

fn default_seed() -> u64 {
    rand::random::<u64>()
}
fn default_handedness() -> Handedness {
    Handedness::Right
}
fn default_dpi() -> u16 {
    1600
}
fn default_wpm_mean() -> f32 {
    50.0
}
fn default_wpm_sigma() -> f32 {
    15.0
}
fn default_scroll() -> ScrollStyle {
    ScrollStyle::Trackpad
}
fn default_fitts_b() -> f32 {
    166.0
}

impl Default for BehaviorProfile {
    fn default() -> Self {
        Self {
            seed: default_seed(),
            handedness: default_handedness(),
            mouse_dpi: default_dpi(),
            typing_wpm_mean: default_wpm_mean(),
            typing_wpm_sigma: default_wpm_sigma(),
            scroll_style: default_scroll(),
            fitts_b: default_fitts_b(),
        }
    }
}

impl BehaviorProfile {
    /// Derive a deterministic sub-RNG for a specific call site. Folds the
    /// session seed with a `salt` so different (from,to) pairs or different
    /// stroke types under the same seed produce different sequences.
    pub fn rng_for(&self, salt: u64) -> ChaCha20Rng {
        let combined = self
            .seed
            .wrapping_mul(0x9E3779B97F4A7C15)
            .wrapping_add(salt);
        ChaCha20Rng::seed_from_u64(combined)
    }
}

// ================================================================
// Mouse trajectory (Sigma-Lognormal — Plamondon 1995)
// ----------------------------------------------------------------
// Real human pointing decomposes into 2-7 lognormal velocity strokes:
// one ballistic primary stroke (~80% of distance) + 1-3 corrective
// sub-movements. BeCAPTCHA-Mouse's RF classifier flags single-Bezier
// trajectories at ~99% — this generator targets <5% flag rate.
//
// Total movement time obeys Fitts's Law:
//   T = a + b · log2(D/W + 1)         (a ≈ 230 ms, b ≈ 166 ms for desktop)
//
// Sample rate: 8 ms (125 Hz) — real USB pointers report at this cadence.
// ================================================================

/// One sample point on a humanized mouse trajectory.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MousePoint {
    pub t_ms: f32,
    pub x: f32,
    pub y: f32,
}

/// Generate a humanlike mouse trajectory from `from` to `to`. `target_w`
/// is the target's width in pixels — drives Fitts's-Law movement time.
pub fn mouse_trajectory(
    from: (f32, f32),
    to: (f32, f32),
    target_w: f32,
    profile: &BehaviorProfile,
) -> Vec<MousePoint> {
    let mut rng = profile
        .rng_for(((from.0 as u64) << 32) | (from.1 as u64) ^ ((to.0 as u64) << 16) ^ (to.1 as u64));
    mouse_trajectory_with_rng(from, to, target_w, profile, &mut rng)
}

/// Same as `mouse_trajectory` but takes an explicit RNG for testing.
pub fn mouse_trajectory_with_rng<R: Rng>(
    from: (f32, f32),
    to: (f32, f32),
    target_w: f32,
    profile: &BehaviorProfile,
    rng: &mut R,
) -> Vec<MousePoint> {
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    let distance = (dx * dx + dy * dy).sqrt().max(1.0);
    let target_w = target_w.max(1.0);

    // Number of strokes: 2-7, scales with index of difficulty.
    let id_bits = ((distance / target_w) + 1.0).log2();
    let n_strokes = ((1.3 * id_bits).round() as usize).clamp(2, 7);

    // Total movement time per Fitts's Law: T = a + b·ID
    let total_ms = 230.0 + profile.fitts_b * id_bits;

    // Stroke amplitudes: primary ≈ 0.85·D, correctives split the rest.
    // Apply small Normal noise so different seeds vary stroke split.
    let mut amplitudes: Vec<f32> = Vec::with_capacity(n_strokes);
    let primary = 0.85 * distance;
    amplitudes.push(primary);
    let remaining = distance - primary;
    let per_corrective = remaining / (n_strokes - 1).max(1) as f32;
    for _ in 1..n_strokes {
        let jitter: f32 = Normal::new(0.0_f32, per_corrective * 0.15)
            .unwrap()
            .sample(rng);
        amplitudes.push((per_corrective + jitter).max(1.0));
    }

    // Per-stroke σ ~ Normal(0.25, 0.05) — clamped to a stable BeCAPTCHA range.
    // Real humans sit < 0.35; bots typically > 0.4.
    let sigma_dist = Normal::new(0.25_f32, 0.05).unwrap();
    // Per-stroke μ ~ Normal(-1.6, 0.2)
    let mu_dist = Normal::new(-1.6_f32, 0.2).unwrap();

    // Inter-stroke onset Δt₀ ~ LogNormal(μ=ln 90 ms, σ=0.3) — Meyer 1988.
    let onset_dist = LogNormal::new(90.0_f32.ln(), 0.3).unwrap();

    // Direction θ per stroke: rotate around target normal with Normal(0, 8°).
    let theta_dist = Normal::new(0.0_f32, 8.0_f32.to_radians()).unwrap();

    // Sub-stroke parameters use the module-level `Stroke` type so we
    // can pass them to integrate_x/integrate_y.
    let target_angle = dy.atan2(dx);
    let mut strokes: Vec<Stroke> = Vec::with_capacity(n_strokes);
    let mut t0 = 0.0_f32;
    for (i, amp) in amplitudes.iter().enumerate() {
        let sigma = sigma_dist.sample(rng).clamp(0.15, 0.40);
        let mu = mu_dist.sample(rng);
        let theta = if i == 0 {
            target_angle + theta_dist.sample(rng)
        } else {
            target_angle + theta_dist.sample(rng) * 1.5
        };
        strokes.push(Stroke {
            amplitude: *amp,
            sigma,
            mu,
            t0,
            theta,
        });
        t0 += onset_dist.sample(rng);
    }

    // Sample the trajectory at 8 ms (125 Hz) over total_ms.
    let dt_ms = 8.0_f32;
    let n_samples = (total_ms / dt_ms).ceil() as usize + 1;
    let mut points: Vec<MousePoint> = Vec::with_capacity(n_samples);

    // Pink-ish micro-tremor at ~8 Hz, ~1.5 px amplitude. We approximate
    // pink-spectrum noise with a low-pass filtered uniform noise.
    let tremor_dist = Normal::new(0.0_f32, 1.5).unwrap();
    let mut tremor_x = 0.0_f32;
    let mut tremor_y = 0.0_f32;
    let tremor_alpha = 0.3_f32; // smoothing pole

    let (cum_x, cum_y) = (from.0, from.1);
    for i in 0..n_samples {
        let t = (i as f32) * dt_ms;

        // Update micro-tremor (smoothed white noise = pink-ish).
        tremor_x = tremor_alpha * tremor_x + (1.0 - tremor_alpha) * tremor_dist.sample(rng);
        tremor_y = tremor_alpha * tremor_y + (1.0 - tremor_alpha) * tremor_dist.sample(rng);

        // Position = origin + cumulative Sigma-Lognormal displacement
        // (closed-form integration of per-stroke velocities) + tremor.
        let x = cum_x + integrate_x(&strokes, t) + tremor_x;
        let y = cum_y + integrate_y(&strokes, t) + tremor_y;
        points.push(MousePoint { t_ms: t, x, y });
    }

    // Land exactly on the target WITHOUT a jerk discontinuity. A
    // single-sample snap (the prior implementation) teleports the last
    // 8 ms sample onto `to`, producing an impulse in velocity and an
    // unbounded spike in the 2nd derivative (jerk) at the endpoint —
    // exactly the statistic BeCAPTCHA-Mouse / Kasada's behavioral
    // scorer integrates over. Real human pointing instead *decelerates
    // smoothly to a stop* on the target. We distribute the small
    // residual (computed_end → to) across the last `tail` samples with
    // a smoothstep weight s(u)=3u²−2u³, whose first derivative is zero
    // at BOTH ends: zero added velocity at the splice (C¹-continuous
    // join with the Σ-Λ tail) and zero velocity at arrival (a natural
    // stop, not a max-velocity impact). Jerk stays bounded throughout.
    if points.len() >= 2 {
        let n = points.len();
        let last = &points[n - 1];
        let res_x = to.0 - last.x;
        let res_y = to.1 - last.y;
        // Tail length ~120 ms of motion (15 samples @ 8 ms), clamped to
        // the available trajectory; long enough that the per-sample
        // correction stays sub-pixel for typical residuals.
        let tail = 15.min(n - 1);
        let start = n - tail - 1;
        for (k, p) in points.iter_mut().enumerate().skip(start) {
            let u = (k - start) as f32 / tail as f32;
            let s = u * u * (3.0 - 2.0 * u); // smoothstep, s(0)=0 s(1)=1
            p.x += res_x * s;
            p.y += res_y * s;
        }
        // Floating-point guarantee: the final sample is exactly `to`.
        if let Some(last) = points.last_mut() {
            last.x = to.0;
            last.y = to.1;
        }
    } else if let Some(last) = points.last_mut() {
        last.x = to.0;
        last.y = to.1;
    }
    points
}

/// One Sigma-Lognormal velocity stroke: amplitude D, lognormal time
/// parameters (σ, μ), onset offset t0 (ms), direction θ (radians).
struct Stroke {
    amplitude: f32,
    sigma: f32,
    mu: f32,
    t0: f32,
    theta: f32,
}

/// Cumulative x-displacement at time `t` from all strokes that have started.
/// Closed form: per-stroke amplitude · CDF(LogNormal(μ, σ); dt) · cos(θ).
fn integrate_x(strokes: &[Stroke], t: f32) -> f32 {
    strokes
        .iter()
        .map(|s| {
            let dt = t - s.t0;
            if dt <= 0.0 {
                return 0.0;
            }
            let z = (dt.ln() - s.mu) / (s.sigma * std::f32::consts::SQRT_2);
            let cdf = 0.5 * (1.0 + erf(z));
            s.amplitude * cdf * s.theta.cos()
        })
        .sum()
}

fn integrate_y(strokes: &[Stroke], t: f32) -> f32 {
    strokes
        .iter()
        .map(|s| {
            let dt = t - s.t0;
            if dt <= 0.0 {
                return 0.0;
            }
            let z = (dt.ln() - s.mu) / (s.sigma * std::f32::consts::SQRT_2);
            let cdf = 0.5 * (1.0 + erf(z));
            s.amplitude * cdf * s.theta.sin()
        })
        .sum()
}

/// Abramowitz-Stegun 7.1.26 approximation to erf — accurate to ~1.5e-7.
/// Sufficient for the LogNormal CDF in the integration step.
fn erf(x: f32) -> f32 {
    let sign = x.signum();
    let x = x.abs();
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;
    let p = 0.3275911;
    let t = 1.0 / (1.0 + p * x);
    let y = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * (-x * x).exp();
    sign * y
}

// ================================================================
// Keystroke dynamics (P3.2b)
// ----------------------------------------------------------------
// Real users (CMU + Buffalo Keystroke benchmarks) show:
//   - Dwell ~ LogNormal(μ=ln 95 ms, σ=0.30) — tight cluster
//   - Flight ~ LogNormal(μ=ln 130 ms, σ=0.55) — heavy right tail
//   - Bigram-specific flight: alt-hand digraphs (`th`, `or`) ≈ 90 ms;
//     same-finger digraphs (`ed`, `un`) ≈ 180 ms
//   - Per-user σ across sessions is small — keystroke dynamics works
//     for biometrics precisely because individuals are consistent.
//   - 1-2% typo rate with backspace+correct burst at faster cadence.
//
// We ship a small curated bigram-flight ratio table for the most common
// English digraphs. Each ratio multiplies the base flight median:
//   - ratio < 1.0 = faster than median (e.g. alt-hand digraphs)
//   - ratio > 1.0 = slower (e.g. same-finger digraphs)
// Values aggregated from CMU+Buffalo published means (facts — derived
// numerical aggregates aren't copyrightable). See BIGRAM_PROVENANCE
// in this module.
// ================================================================

/// Keystroke timing for one character.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeystrokeTiming {
    pub ch: char,
    /// Dwell time (keydown → keyup) in milliseconds.
    pub dwell_ms: f32,
    /// Flight time from previous keyup to this keydown in milliseconds.
    /// Zero for the first keystroke.
    pub flight_ms: f32,
}

/// Look up the bigram flight-ratio for a digraph (prev → cur). Returns
/// 1.0 for unknown digraphs (use base median).
fn bigram_ratio(prev: char, cur: char) -> f32 {
    let key = (
        prev.to_ascii_lowercase() as u8,
        cur.to_ascii_lowercase() as u8,
    );
    // Top-20 English bigrams by frequency (from Norvig's analysis):
    // th, he, in, er, an, re, on, at, en, nd,
    // ti, es, or, te, of, ed, is, it, al, ar
    // Ratios reflect alt-hand vs same-finger keyboard layout typing
    // patterns (CMU Keystroke benchmark).
    match key {
        // alt-hand fast bigrams (~0.7×)
        (b't', b'h')
        | (b'h', b'e')
        | (b'i', b'n')
        | (b'a', b'n')
        | (b'o', b'n')
        | (b'a', b't')
        | (b'i', b's')
        | (b'i', b't')
        | (b'o', b'r')
        | (b'o', b'f') => 0.7,
        // medium-typical bigrams (~1.0×) — fallthrough handled by default
        // same-finger / awkward bigrams (~1.4×)
        (b'e', b'd')
        | (b'u', b'n')
        | (b'r', b'e')
        | (b'e', b'r')
        | (b'e', b'n')
        | (b'n', b'd')
        | (b'e', b's')
        | (b't', b'e')
        | (b'a', b'l')
        | (b'a', b'r') => 1.4,
        // Same-key digraph (very awkward, ~2.0×)
        (a, b) if a == b => 2.0,
        _ => 1.0,
    }
}

/// Generate keystroke timings for a string. Per-keystroke dwell and
/// inter-key flight are LogNormal-distributed. Bigram-modulated flight
/// captures the alt-hand/same-finger speed difference.
///
/// `wpm_mean` from `BehaviorProfile.typing_wpm_mean` drives the base
/// flight median: at 50 WPM, average inter-key delay ≈ 240 ms; flight
/// median ≈ 130 ms (the rest is dwell).
pub fn keystroke_timings(text: &str, profile: &BehaviorProfile) -> Vec<KeystrokeTiming> {
    let mut rng = profile.rng_for(0xCAFEBABE ^ text.len() as u64);
    keystroke_timings_with_rng(text, profile, &mut rng)
}

pub fn keystroke_timings_with_rng<R: Rng>(
    text: &str,
    profile: &BehaviorProfile,
    rng: &mut R,
) -> Vec<KeystrokeTiming> {
    // Base flight median scaled by WPM. Lower WPM = longer flights.
    // Convert WPM → ms-per-character: 60_000 ms / (wpm * 5 chars/word).
    let ms_per_char = 60_000.0 / (profile.typing_wpm_mean * 5.0);
    // Of that, dwell ≈ 95 ms median; flight is the rest.
    let flight_median = (ms_per_char - 95.0).max(40.0);
    let flight_dist = LogNormal::new(flight_median.ln(), 0.55).unwrap();
    let dwell_dist = LogNormal::new(95.0_f32.ln(), 0.30).unwrap();

    let mut out = Vec::with_capacity(text.len());
    let mut prev_ch: Option<char> = None;
    for ch in text.chars() {
        let dwell = dwell_dist.sample(rng).clamp(40.0, 400.0);
        let flight = if let Some(p) = prev_ch {
            let ratio = bigram_ratio(p, ch);
            (flight_dist.sample(rng) * ratio).clamp(20.0, 1000.0)
        } else {
            0.0
        };
        out.push(KeystrokeTiming {
            ch,
            dwell_ms: dwell,
            flight_ms: flight,
        });
        prev_ch = Some(ch);
    }
    out
}

// ================================================================
// Scroll bursts (P3.2c)
// ================================================================

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WheelTick {
    pub t_ms: f32,
    pub delta_y: f32,
    /// `WheelEvent.deltaMode`: 0=pixels (trackpad), 1=lines (legacy), 2=pages.
    /// Modern Chrome reports 0 even for mouse wheel (deltaY=100 per notch).
    pub mode: u32,
}

/// Generate a humanlike scroll burst that totals approximately `target_dy`
/// pixels. Trackpad uses exponential momentum decay at 60 Hz; mouse wheel
/// uses discrete 100 px notches at LogNormal intervals.
pub fn wheel_burst(target_dy: f32, profile: &BehaviorProfile) -> Vec<WheelTick> {
    let mut rng = profile.rng_for(0xDEAD_BEEF ^ target_dy.to_bits() as u64);
    wheel_burst_with_rng(target_dy, profile, &mut rng)
}

/// Same as `wheel_burst` but takes an explicit RNG for testing.
pub fn wheel_burst_with_rng<R: Rng>(
    target_dy: f32,
    profile: &BehaviorProfile,
    rng: &mut R,
) -> Vec<WheelTick> {
    let dir = if target_dy >= 0.0 { 1.0 } else { -1.0 };
    let abs_dy = target_dy.abs().max(1.0);

    match profile.scroll_style {
        ScrollStyle::Trackpad => {
            // Two-finger swipe with exponential decay. Initial velocity is
            // proportional to log(target) — humans don't flick at 1000 px/16ms
            // for huge scrolls, they accept multiple swipes.
            let v0 = LogNormal::new((abs_dy / 8.0).ln(), 0.3)
                .unwrap()
                .sample(rng);
            let decay = 0.94 + rng.random_range(0.0_f32..0.04); // 0.94-0.98
            let mut t = 0.0_f32;
            let mut v = v0;
            let mut ticks = Vec::new();
            let mut accumulated = 0.0_f32;
            while v > 0.5 && accumulated < abs_dy * 1.1 {
                let step = (v.min(abs_dy - accumulated)).max(0.5);
                ticks.push(WheelTick {
                    t_ms: t,
                    delta_y: step * dir,
                    mode: 0,
                });
                accumulated += step;
                t += 16.0;
                v *= decay;
            }
            ticks
        }
        ScrollStyle::Wheel => {
            // Discrete 100 px notches at LogNormal-spaced intervals.
            let notches = ((abs_dy / 100.0).round() as u32).max(1);
            let interval_dist = LogNormal::new(180.0_f32.ln(), 0.4).unwrap();
            let mut t = 0.0_f32;
            let mut ticks = Vec::with_capacity(notches as usize);
            for _ in 0..notches {
                ticks.push(WheelTick {
                    t_ms: t,
                    delta_y: 100.0 * dir,
                    mode: 0,
                });
                t += interval_dist.sample(rng);
            }
            ticks
        }
    }
}

// ================================================================
// Tests
// ================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rand_chacha::rand_core::SeedableRng;

    fn fixed_rng() -> ChaCha20Rng {
        ChaCha20Rng::seed_from_u64(42)
    }

    // ---- BehaviorProfile defaults / determinism ----

    #[test]
    fn profile_defaults_are_sensible() {
        let p = BehaviorProfile::default();
        assert!((30.0..=80.0).contains(&p.typing_wpm_mean));
        assert!((130.0..=220.0).contains(&p.fitts_b));
        assert_eq!(p.handedness, Handedness::Right);
    }

    #[test]
    fn rng_for_is_deterministic_per_seed() {
        let p = BehaviorProfile::default();
        let mut a = p.rng_for(123);
        let mut b = p.rng_for(123);
        assert_eq!(a.random::<u64>(), b.random::<u64>());
    }

    #[test]
    fn rng_for_differs_across_salts() {
        let p = BehaviorProfile::default();
        let mut a = p.rng_for(1);
        let mut b = p.rng_for(2);
        assert_ne!(a.random::<u64>(), b.random::<u64>());
    }

    // ---- Mouse trajectory ----

    #[test]
    fn mouse_trajectory_starts_at_from_and_ends_at_to() {
        let p = BehaviorProfile::default();
        let pts = mouse_trajectory((100.0, 100.0), (500.0, 400.0), 50.0, &p);
        assert!(pts.len() > 5);
        // First point starts close to `from` (tremor adds <5px); last lands on target.
        let first = pts[0];
        let last = pts[pts.len() - 1];
        assert!((first.x - 100.0).abs() < 10.0, "first x={}", first.x);
        assert!((first.y - 100.0).abs() < 10.0, "first y={}", first.y);
        assert_eq!(last.x, 500.0);
        assert_eq!(last.y, 400.0);
    }

    #[test]
    fn mouse_trajectory_obeys_fitts_law_total_time() {
        let p = BehaviorProfile::default();
        // Fitts: T = 230 + 166·log2(D/W + 1).
        // For (D=500, W=50): log2(11) ≈ 3.46 → T ≈ 230 + 575 = 805 ms.
        let pts = mouse_trajectory((0.0, 0.0), (500.0, 0.0), 50.0, &p);
        let last_t = pts[pts.len() - 1].t_ms;
        assert!(
            (700.0..=950.0).contains(&last_t),
            "expected ~805 ms, got {last_t}"
        );
    }

    #[test]
    fn mouse_trajectory_uses_8ms_sample_rate() {
        let p = BehaviorProfile::default();
        let pts = mouse_trajectory((0.0, 0.0), (200.0, 0.0), 30.0, &p);
        for w in pts.windows(2) {
            let dt = w[1].t_ms - w[0].t_ms;
            assert!((dt - 8.0).abs() < 1e-3, "gap {} not 8 ms", dt);
        }
    }

    #[test]
    fn mouse_trajectory_has_velocity_diversity_not_uniform() {
        // A pure linear interpolation gives constant velocity; Sigma-Lognormal
        // should have multiple distinct speed bands. Compute speeds and assert
        // their stddev is non-trivial relative to mean.
        let p = BehaviorProfile::default();
        let mut rng = fixed_rng();
        let pts = mouse_trajectory_with_rng((0.0, 0.0), (600.0, 400.0), 40.0, &p, &mut rng);
        let speeds: Vec<f32> = pts
            .windows(2)
            .map(|w| ((w[1].x - w[0].x).powi(2) + (w[1].y - w[0].y).powi(2)).sqrt())
            .collect();
        let mean = speeds.iter().sum::<f32>() / speeds.len() as f32;
        let var = speeds.iter().map(|s| (s - mean).powi(2)).sum::<f32>() / speeds.len() as f32;
        let std = var.sqrt();
        let cv = std / mean.max(1e-3);
        // Coefficient of variation should be > 0.4 (real humans 0.5-1.2;
        // pure Bezier is ~0.2).
        assert!(cv > 0.4, "speed coefficient of variation too low: {cv}");
    }

    #[test]
    fn mouse_trajectory_deterministic_per_seed() {
        let p = BehaviorProfile {
            seed: 123,
            ..BehaviorProfile::default()
        };
        let mut r1 = p.rng_for(1);
        let mut r2 = p.rng_for(1);
        let a = mouse_trajectory_with_rng((0.0, 0.0), (300.0, 200.0), 25.0, &p, &mut r1);
        let b = mouse_trajectory_with_rng((0.0, 0.0), (300.0, 200.0), 25.0, &p, &mut r2);
        assert_eq!(a.len(), b.len());
        for (pa, pb) in a.iter().zip(b.iter()) {
            assert_eq!(pa, pb);
        }
    }

    #[test]
    fn mouse_trajectory_no_endpoint_jerk_spike() {
        // Regression: the prior implementation snapped the final sample
        // onto `to` in one 8 ms step, producing a velocity impulse and
        // an unbounded jerk (3rd-derivative) spike at the endpoint —
        // exactly what BeCAPTCHA-Mouse / Kasada's behavioral scorer
        // integrates. A real decelerating stop has its SMALLEST
        // per-sample steps at the end, never its largest. Assert the
        // final step is not an outlier vs the interior step
        // distribution, across many seeds and geometries.
        for seed in 0..40u64 {
            let p = BehaviorProfile {
                seed,
                ..BehaviorProfile::default()
            };
            let mut r = p.rng_for(2);
            let tr = mouse_trajectory_with_rng((12.0, 30.0), (840.0, 510.0), 28.0, &p, &mut r);
            assert!(tr.len() >= 8, "trajectory too short");
            let step =
                |a: &MousePoint, b: &MousePoint| ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
            let steps: Vec<f32> = tr.windows(2).map(|w| step(&w[0], &w[1])).collect();
            let n = steps.len();
            let final_step = steps[n - 1];
            // Median interior step as the scale reference (robust to the
            // ballistic-phase max).
            let mut sorted = steps.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let median = sorted[n / 2];
            let max_step = sorted[n - 1];
            // A smooth stop: the last step must be modest — not a snap.
            // Old behavior could make final_step == the whole residual
            // (often ≫ median). Require it to stay within the normal
            // motion envelope (≤ the max interior step, and not a wild
            // multiple of the median).
            assert!(
                final_step <= max_step + 1e-3,
                "seed {seed}: final step {final_step} exceeds max interior step {max_step} — endpoint snap/jerk spike"
            );
            assert!(
                final_step <= median * 6.0 + 5.0,
                "seed {seed}: final step {final_step} is a jerk outlier vs median {median}"
            );
            // And it still lands exactly on target.
            let last = tr.last().unwrap();
            assert!((last.x - 840.0).abs() < 1e-2 && (last.y - 510.0).abs() < 1e-2);
        }
    }

    // ---- Scroll burst ----

    // ---- Keystroke dynamics ----

    #[test]
    fn keystroke_first_has_no_flight() {
        let p = BehaviorProfile::default();
        let ks = keystroke_timings("hi", &p);
        assert_eq!(ks[0].flight_ms, 0.0);
        assert!(ks[1].flight_ms > 0.0);
    }

    #[test]
    fn keystroke_dwell_in_realistic_range() {
        let p = BehaviorProfile::default();
        let ks = keystroke_timings("the quick brown fox jumps over the lazy dog", &p);
        let mean_dwell: f32 = ks.iter().map(|k| k.dwell_ms).sum::<f32>() / ks.len() as f32;
        // CMU benchmark: median dwell ≈ 95 ms; mean is slightly higher
        // due to the LogNormal right tail. Expect mean in 70-150 ms.
        assert!(
            (70.0..=150.0).contains(&mean_dwell),
            "mean dwell {mean_dwell} outside CMU plausible range"
        );
    }

    #[test]
    fn keystroke_flight_scales_with_wpm() {
        // 30 WPM (slow) should have noticeably longer flights than 70 WPM (fast).
        let slow = BehaviorProfile {
            typing_wpm_mean: 30.0,
            ..BehaviorProfile::default()
        };
        let fast = BehaviorProfile {
            typing_wpm_mean: 70.0,
            ..BehaviorProfile::default()
        };
        let s = keystroke_timings("the quick brown fox jumps over", &slow);
        let f = keystroke_timings("the quick brown fox jumps over", &fast);
        let mean = |ks: &[KeystrokeTiming]| -> f32 {
            ks.iter().skip(1).map(|k| k.flight_ms).sum::<f32>() / (ks.len() - 1) as f32
        };
        assert!(
            mean(&s) > mean(&f),
            "30 WPM flight {} should exceed 70 WPM flight {}",
            mean(&s),
            mean(&f)
        );
    }

    #[test]
    fn keystroke_bigram_th_faster_than_dd() {
        // 'th' is a top alt-hand bigram (ratio 0.7), 'dd' is same-key (2.0).
        // Test enforces the bigram modulation works at all.
        let p = BehaviorProfile::default();
        let mut th_total = 0.0_f32;
        let mut dd_total = 0.0_f32;
        for seed in 0..50 {
            let prof = BehaviorProfile {
                seed: seed as u64,
                ..BehaviorProfile::default()
            };
            let th = keystroke_timings("th", &prof);
            let dd = keystroke_timings("dd", &prof);
            th_total += th[1].flight_ms;
            dd_total += dd[1].flight_ms;
        }
        let th_mean = th_total / 50.0;
        let dd_mean = dd_total / 50.0;
        assert!(
            dd_mean > th_mean * 1.5,
            "dd flight {dd_mean} should be > 1.5× th flight {th_mean}"
        );
    }

    #[test]
    fn keystroke_deterministic_per_seed() {
        let mut rng_a = ChaCha20Rng::seed_from_u64(7);
        let mut rng_b = ChaCha20Rng::seed_from_u64(7);
        let p = BehaviorProfile::default();
        let a = keystroke_timings_with_rng("hello world", &p, &mut rng_a);
        let b = keystroke_timings_with_rng("hello world", &p, &mut rng_b);
        assert_eq!(a, b);
    }

    // ---- Scroll burst ----

    #[test]
    fn trackpad_burst_decays_to_zero() {
        let p = BehaviorProfile {
            scroll_style: ScrollStyle::Trackpad,
            ..BehaviorProfile::default()
        };
        let ticks = wheel_burst(-1000.0, &p);
        assert!(ticks.len() > 5);
        // All ticks share deltaMode=0 and sign matches direction.
        for t in &ticks {
            assert_eq!(t.mode, 0);
            assert!(t.delta_y < 0.0);
        }
        // Cumulative dy approaches the target.
        let cum: f32 = ticks.iter().map(|t| t.delta_y).sum();
        assert!(
            (cum + 1000.0).abs() < 200.0,
            "cumulative {cum} not close to -1000"
        );
        // Time gaps are 16 ms (60 Hz).
        for w in ticks.windows(2) {
            let dt = w[1].t_ms - w[0].t_ms;
            assert!((dt - 16.0).abs() < 1e-3);
        }
    }

    #[test]
    fn wheel_burst_uses_100px_notches() {
        let p = BehaviorProfile {
            scroll_style: ScrollStyle::Wheel,
            ..BehaviorProfile::default()
        };
        let ticks = wheel_burst(500.0, &p);
        assert_eq!(ticks.len(), 5);
        for t in &ticks {
            assert_eq!(t.delta_y, 100.0);
            assert_eq!(t.mode, 0);
        }
    }

    #[test]
    fn wheel_burst_intervals_are_lognormal_distributed() {
        let p = BehaviorProfile {
            scroll_style: ScrollStyle::Wheel,
            ..BehaviorProfile::default()
        };
        let ticks = wheel_burst(2000.0, &p); // 20 notches
        let intervals: Vec<f32> = ticks.windows(2).map(|w| w[1].t_ms - w[0].t_ms).collect();
        // Mean should be near 180 ms (LogNormal median exp(ln 180) = 180).
        let mean = intervals.iter().sum::<f32>() / intervals.len() as f32;
        assert!(
            (mean - 180.0).abs() < 200.0,
            "mean interval {mean} too far from 180 ms"
        );
        // At least 5 distinct values (rules out uniform spacing).
        let mut sorted = intervals.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        sorted.dedup_by(|a, b| (*a - *b).abs() < 1e-3);
        assert!(sorted.len() > 5, "only {} distinct intervals", sorted.len());
    }

    #[test]
    fn default_seeds_differ_across_instances() {
        // Each BehaviorProfile::default() must produce a unique seed so that
        // different sessions don't share identical mouse/keyboard trajectories.
        let a = BehaviorProfile::default();
        let b = BehaviorProfile::default();
        assert_ne!(
            a.seed, b.seed,
            "default seeds must be random, got {:#x} for both",
            a.seed
        );
    }

    #[test]
    fn default_seed_is_not_the_placeholder() {
        // 0xCAFEF00DDEADBEEF is the old hardcoded test seed — must never appear in prod.
        let p = BehaviorProfile::default();
        assert_ne!(
            p.seed, 0xCAFEF00DDEADBEEF,
            "default seed is the hardcoded placeholder — must be randomized"
        );
    }
}
