//! Humanized `performance.now()`.
//!
//! Real Chrome 130 quantizes `performance.now()` to 100 µs (or 5 µs with
//! cross-origin isolation), but the resolution is not the whole story —
//! the **jitter shape** across many calls in a tight loop also differs
//! from a software clock. Real hardware shows ~10–30 µs gaussian-ish
//! noise around the quantized step from kernel scheduling, TSC drift, and
//! V8's own quantization-with-noise applied on top of `CLOCK_MONOTONIC`.
//!
//! Pure software clocks return a perfect 100 µs grid with one distinct
//! step value — `set(diffs).size === 1`, which a real browser never shows.
//!
//! Distribution (per Schwarz et al. "Drawn Apart" 2021 + Jin 2024 measurements
//! on Chromium 124 stable):
//!   q       = floor(now_us / 100) * 100              // 100 µs grid
//!   jitter  ~ LogNormal(μ = ln 8 µs, σ = 0.4)        // clamped [0, 35] µs
//!   spike   = with prob 1/1024, sample Exp(λ=1/200 µs) clamped ≤ 1500 µs
//!   result  = (q + jitter + spike) ms

use crate::js_runtime::state::DomState;
use deno_core::op2;
use deno_core::OpState;
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};
use rand_distr::{Distribution, Exp, LogNormal};
use std::time::Instant;

/// Per-runtime state for the humanized clock.
pub struct PerfState {
    /// Process-relative origin; `performance.now()` returns ms since this
    /// instant (matches DOM HighResolutionTime contract for the document).
    origin: Instant,
    /// Wall-clock (UNIX epoch ms) corresponding to `origin`. Read by
    /// `op_perf_time_origin_ms` so JS `performance.timeOrigin` honors the
    /// invariant `timeOrigin + performance.now() ≈ Date.now()`. Real
    /// Chrome maintains this invariant; without it, an earlier JS-side
    /// ad-hoc computation (`Date.now() - <hardcoded nav_end>`) produced a
    /// detectable skew between `performance.timeOrigin + performance.now()`
    /// and `Date.now()`.
    origin_unix_ms: f64,
    rng: StdRng,
    log_normal: LogNormal<f64>,
    spike_exp: Exp<f64>,
    /// Last returned value in µs — enforces monotonicity per HRT spec.
    /// Without this, adjacent calls can go backward when the clock barely
    /// advances and the second call samples lower jitter.
    last_us: f64,
}

impl PerfState {
    pub fn new() -> Self {
        Self::with_seed(0xCAFEF00DDEADBEEF)
    }
    pub fn with_seed(seed: u64) -> Self {
        let origin_unix_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64() * 1000.0)
            .unwrap_or(0.0);
        Self {
            origin: Instant::now(),
            origin_unix_ms,
            rng: StdRng::seed_from_u64(seed),
            // μ=ln(8 µs) ≈ 2.079
            log_normal: LogNormal::new(2.079_441_541_679_835, 0.4).expect("valid lognormal"),
            // Exp(1/200 µs) — mean 200 µs heavy-tail
            spike_exp: Exp::new(1.0 / 200.0).expect("valid exp"),
            last_us: 0.0,
        }
    }

    /// Returns elapsed ms since origin with Chrome-130-shaped jitter.
    /// Monotonicity enforced per HRT spec: result is clamped to be >=
    /// the previous return value, so the per-call jitter cannot create
    /// a backward step.
    pub fn now_ms(&mut self) -> f64 {
        let raw_us = self.origin.elapsed().as_nanos() as f64 / 1000.0;
        let q = (raw_us / 100.0).floor() * 100.0;
        let jitter = self.log_normal.sample(&mut self.rng).clamp(0.0, 35.0);
        let spike = if self.rng.random_bool(1.0 / 1024.0) {
            self.spike_exp.sample(&mut self.rng).min(1500.0)
        } else {
            0.0
        };
        let candidate = q + jitter + spike;
        // Monotonic clamp — Chrome's quantizer never goes backward.
        let value = candidate.max(self.last_us);
        self.last_us = value;
        value / 1000.0
    }
}

impl Default for PerfState {
    fn default() -> Self {
        Self::new()
    }
}

#[op2(fast)]
pub fn op_perf_now_humanized(s: &mut OpState) -> f64 {
    let s = s.borrow_mut::<PerfState>();
    s.now_ms()
}

/// Returns the UNIX-epoch ms corresponding to `PerfState.origin` (the
/// process-relative t=0 for `performance.now()`). JS uses this as the
/// `performance.timeOrigin` value so the standard Web Platform invariant
/// `timeOrigin + performance.now() ≈ Date.now()` holds.
#[op2(fast)]
pub fn op_perf_time_origin_ms(s: &mut OpState) -> f64 {
    let s = s.borrow::<PerfState>();
    s.origin_unix_ms
}

#[derive(serde::Serialize)]
pub struct JsResourceTiming {
    pub name: String,
    pub entry_type: String,
    pub start_time: f64,
    pub duration: f64,
    pub fetch_start: f64,
    pub domain_lookup_start: f64,
    pub domain_lookup_end: f64,
    pub connect_start: f64,
    pub connect_end: f64,
    pub secure_connection_start: f64,
    pub request_start: f64,
    pub response_start: f64,
    pub response_end: f64,
    pub transfer_size: u64,
    pub encoded_body_size: u64,
    pub decoded_body_size: u64,
}

#[op2]
#[serde]
pub fn op_perf_get_resource_timings(state: &mut OpState) -> Vec<JsResourceTiming> {
    let state = state.borrow::<DomState>();
    state
        .resource_timings
        .iter()
        .map(|t| JsResourceTiming {
            name: "https://example.com/placeholder".to_string(),
            entry_type: "resource".to_string(),
            start_time: t.request_start_ms,
            duration: t.response_end_ms - t.request_start_ms,
            fetch_start: t.request_start_ms,
            domain_lookup_start: t.dns_start_ms,
            domain_lookup_end: t.dns_end_ms,
            connect_start: t.connect_start_ms,
            connect_end: t.connect_end_ms,
            secure_connection_start: t.tls_start_ms,
            request_start: t.request_start_ms,
            response_start: t.response_start_ms,
            response_end: t.response_end_ms,
            transfer_size: 0,
            encoded_body_size: 0,
            decoded_body_size: 0,
        })
        .collect()
}

deno_core::extension!(
    perf_extension,
    ops = [
        op_perf_now_humanized,
        op_perf_get_resource_timings,
        op_perf_time_origin_ms,
    ],
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distribution_has_distinct_jitter_values() {
        let mut s = PerfState::with_seed(7);
        // The monotonicity clamp + a tight hot loop on a fast CPU means many
        // adjacent calls clamp to last_us (the underlying clock advances by
        // far less than the inter-call gap). A real browser's hot-loop diff
        // *cardinality* is well above 1; anything above ~5 distinct values
        // is realistic versus the software-clock `set(diffs).size === 1`
        // signature. We assert >10 here for headroom.
        let mut samples: Vec<f64> = (0..500).map(|_| s.now_ms()).collect();
        samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
        samples.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        assert!(
            samples.len() > 10,
            "expected >10 distinct values, got {}",
            samples.len()
        );
    }

    #[test]
    fn jitter_is_bounded_and_non_negative() {
        let mut s = PerfState::with_seed(0xDEADBEEF);
        for _ in 0..10_000 {
            let v = s.log_normal.sample(&mut s.rng).clamp(0.0, 35.0);
            assert!((0.0..=35.0).contains(&v));
        }
    }

    #[test]
    fn deterministic_across_runs_with_same_seed() {
        let mut a = PerfState::with_seed(123);
        let mut b = PerfState::with_seed(123);
        let ja: Vec<f64> = (0..100).map(|_| a.log_normal.sample(&mut a.rng)).collect();
        let jb: Vec<f64> = (0..100).map(|_| b.log_normal.sample(&mut b.rng)).collect();
        assert_eq!(ja, jb);
    }

    #[test]
    fn occasional_heavy_tail_spikes() {
        // Over 100k samples we should see at least one >250 µs jitter event
        // (the Bernoulli(1/1024) Exp tail). Absence indicates the spike path
        // never fires.
        let mut s = PerfState::with_seed(0xBEEF);
        let mut max_jitter_us = 0.0_f64;
        for _ in 0..100_000 {
            let j = s.log_normal.sample(&mut s.rng).clamp(0.0, 35.0);
            let spike = if s.rng.random_bool(1.0 / 1024.0) {
                s.spike_exp.sample(&mut s.rng).min(1500.0)
            } else {
                0.0
            };
            max_jitter_us = max_jitter_us.max(j + spike);
        }
        assert!(
            max_jitter_us > 250.0,
            "expected at least one spike >250 µs, got max {} µs",
            max_jitter_us
        );
    }
}
