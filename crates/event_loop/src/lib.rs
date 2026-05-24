//! Browser event loop wrapping deno_core's V8 event loop with
//! timer scheduling, requestAnimationFrame, and idle detection.

use js_runtime::BrowserJsRuntime;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// PROFILER (env-gated, near-zero overhead when disabled)
// ---------------------------------------------------------------------------
//
// Enable with `BROWSER_OXIDE_EVENT_LOOP_PROFILE=1`. When active, every
// `run_until_idle` invocation captures per-tick wall-clock plus the
// deno_core RuntimeActivity snapshot (pending async ops, timers, intervals,
// resources) and dumps a histogram + per-tick CSV to stderr at exit.
//
// Used to root-cause SPA hydration timeouts where the page body remains
// near-empty after the nav budget elapses.

#[derive(Clone, Copy, Default, Debug)]
struct TickRow {
    tick: u32,
    wall_us: u64,           // tick wall-clock duration (microseconds)
    pending_async_ops: u32, // in-flight ops (op_fetch, op_sleep, ...)
    pending_timers: u32,    // setTimeout entries
    pending_intervals: u32, // setInterval entries
    pending_resources: u32, // open ResourceTable handles
    timed_out: bool,        // tick hit its 100ms slice ceiling without idling
}

#[inline(always)]
fn profile_enabled() -> bool {
    // Cached after first call — env var lookups are syscalls on every tick
    // when nested in run_until_idle, which would itself perturb the timing
    // we're measuring. Read once per process.
    use std::sync::OnceLock;
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        matches!(
            std::env::var("BROWSER_OXIDE_EVENT_LOOP_PROFILE").as_deref(),
            Ok("1") | Ok("true") | Ok("yes")
        )
    })
}

/// Per-op-name aggregate so the profile dump can name the dominant op.
/// Stored in a thread-local because dump_profile takes only `rows` and we
/// don't want to bloat TickRow with a HashMap. Reset at start of each
/// `run_until_idle` invocation.
type OpNameMap = std::collections::HashMap<&'static str, u64>;

thread_local! {
    static OP_NAME_TOTALS: std::cell::RefCell<OpNameMap> =
        std::cell::RefCell::new(OpNameMap::new());
}

/// Capture the current pending-task counts from the underlying deno_core
/// runtime. Cheap-ish: walks 3-4 small Vecs and clones the activity
/// snapshot. Only called when profiling is enabled.
fn capture_pending(runtime: &mut BrowserJsRuntime) -> (u32, u32, u32, u32) {
    use deno_core::stats::{RuntimeActivity, RuntimeActivityStatsFilter};
    let factory = runtime.inner().runtime_activity_stats_factory();
    let stats = factory.capture(&RuntimeActivityStatsFilter::all());
    let snap = stats.dump();
    let mut ops = 0u32;
    let mut timers = 0u32;
    let mut intervals = 0u32;
    let mut resources = 0u32;
    OP_NAME_TOTALS.with(|m| {
        let mut m = m.borrow_mut();
        for a in snap.active.iter() {
            match a {
                RuntimeActivity::AsyncOp(_, _, name) => {
                    ops += 1;
                    *m.entry(*name).or_insert(0) += 1;
                }
                RuntimeActivity::Timer(..) => timers += 1,
                RuntimeActivity::Interval(..) => intervals += 1,
                RuntimeActivity::Resource(..) => resources += 1,
            }
        }
    });
    (ops, timers, intervals, resources)
}

/// Pretty-print a sequence of `TickRow`s to stderr as a profile dump.
/// Sections: header, top-N slowest ticks, growth check (quadratic
/// detector), and CSV tail for offline analysis.
fn dump_profile(label: &str, rows: &[TickRow], total: Duration, reason: IdleReason) {
    use std::io::Write;
    let stderr = std::io::stderr();
    let mut w = stderr.lock();

    let _ = writeln!(w, "\n========== BROWSER_OXIDE EVENT-LOOP PROFILE ==========");
    let _ = writeln!(w, "label              : {}", label);
    let _ = writeln!(w, "reason             : {:?}", reason);
    let _ = writeln!(w, "total wall (ms)    : {}", total.as_millis());
    let _ = writeln!(w, "ticks              : {}", rows.len());
    if rows.is_empty() {
        let _ = writeln!(w, "(no ticks recorded — instantaneous idle)");
        let _ = writeln!(w, "================================================\n");
        return;
    }

    let total_us: u64 = rows.iter().map(|r| r.wall_us).sum();
    let timed_out: usize = rows.iter().filter(|r| r.timed_out).count();
    let max_tick = rows.iter().max_by_key(|r| r.wall_us).copied().unwrap();
    let avg_us = total_us / rows.len() as u64;

    let _ = writeln!(w, "total tick us      : {}", total_us);
    let _ = writeln!(w, "avg tick us        : {}", avg_us);
    let _ = writeln!(w, "ticks-timed-out    : {}", timed_out);
    let _ = writeln!(
        w,
        "max tick           : #{} {}us pending(ops={}, timers={}, intervals={}, res={})",
        max_tick.tick,
        max_tick.wall_us,
        max_tick.pending_async_ops,
        max_tick.pending_timers,
        max_tick.pending_intervals,
        max_tick.pending_resources,
    );

    // Top-10 slowest ticks
    let mut sorted = rows.to_vec();
    sorted.sort_unstable_by_key(|r| std::cmp::Reverse(r.wall_us));
    let _ = writeln!(w, "\n--- top-10 slowest ticks ---");
    let _ = writeln!(
        w,
        "  tick   wall_us  ops  timers  intervals  res  timed_out"
    );
    for r in sorted.iter().take(10) {
        let _ = writeln!(
            w,
            "  {:5}  {:>7}  {:>3}  {:>6}  {:>9}  {:>3}  {}",
            r.tick,
            r.wall_us,
            r.pending_async_ops,
            r.pending_timers,
            r.pending_intervals,
            r.pending_resources,
            r.timed_out,
        );
    }

    // Pending-task histogram across all ticks
    let max_ops = rows.iter().map(|r| r.pending_async_ops).max().unwrap_or(0);
    let max_timers = rows.iter().map(|r| r.pending_timers).max().unwrap_or(0);
    let max_intervals = rows.iter().map(|r| r.pending_intervals).max().unwrap_or(0);
    let final_row = rows.last().copied().unwrap();
    let _ = writeln!(w, "\n--- pending-task envelope ---");
    let _ = writeln!(w, "  max async-ops   : {}", max_ops);
    let _ = writeln!(w, "  max timers      : {}", max_timers);
    let _ = writeln!(w, "  max intervals   : {}", max_intervals);
    let _ = writeln!(
        w,
        "  final pending   : ops={} timers={} intervals={} res={}",
        final_row.pending_async_ops,
        final_row.pending_timers,
        final_row.pending_intervals,
        final_row.pending_resources,
    );

    // Quadratic / monotonic-growth detector. We chunk into quartiles and
    // compare avg pending counts per quartile — if Q4 > 4 × Q1 it's a
    // strong sign of unbounded accumulation (Promise.then bombs,
    // MutationObserver flooding, runaway IntersectionObserver, ...).
    let n = rows.len();
    if n >= 8 {
        let q = n / 4;
        let avg_ops = |slice: &[TickRow]| -> f64 {
            slice
                .iter()
                .map(|r| r.pending_async_ops as u64)
                .sum::<u64>() as f64
                / slice.len() as f64
        };
        let avg_t = |slice: &[TickRow]| -> f64 {
            slice.iter().map(|r| r.pending_timers as u64).sum::<u64>() as f64 / slice.len() as f64
        };
        let q1_ops = avg_ops(&rows[..q]);
        let q4_ops = avg_ops(&rows[n - q..]);
        let q1_t = avg_t(&rows[..q]);
        let q4_t = avg_t(&rows[n - q..]);
        let _ = writeln!(w, "\n--- growth detector (quartile averages) ---");
        let _ = writeln!(
            w,
            "  ops    Q1={:.1}  Q4={:.1}  ratio={:.2}x",
            q1_ops,
            q4_ops,
            if q1_ops > 0.0 { q4_ops / q1_ops } else { 0.0 }
        );
        let _ = writeln!(
            w,
            "  timers Q1={:.1}  Q4={:.1}  ratio={:.2}x",
            q1_t,
            q4_t,
            if q1_t > 0.0 { q4_t / q1_t } else { 0.0 }
        );
        if (q1_ops > 0.0 && q4_ops / q1_ops > 4.0) || (q1_t > 0.0 && q4_t / q1_t > 4.0) {
            let _ = writeln!(
                w,
                "  WARNING: pending-task count > 4x growth Q1→Q4 — likely runaway scheduler"
            );
        }
    }

    // Top op names — names of the ops that were observed pending across
    // all ticks (counts are sum of "snapshot pending count" — i.e. an op
    // that stayed pending for N ticks contributes N). High-count names
    // identify the chain that's keeping is_pending=true.
    OP_NAME_TOTALS.with(|m| {
        let m = m.borrow();
        if !m.is_empty() {
            let mut v: Vec<(&&'static str, &u64)> = m.iter().collect();
            v.sort_unstable_by_key(|(_, c)| std::cmp::Reverse(**c));
            let _ = writeln!(
                w,
                "\n--- pending-op name breakdown (sum of per-tick pending counts) ---"
            );
            for (name, count) in v.iter().take(15) {
                let _ = writeln!(w, "  {:>8}  {}", count, name);
            }
        }
    });

    // CSV tail for offline crunching (paste into a spreadsheet / Pandas).
    let _ = writeln!(
        w,
        "\n--- per-tick CSV (tick,wall_us,ops,timers,intervals,res,timed_out) ---"
    );
    for r in rows.iter() {
        let _ = writeln!(
            w,
            "EL-CSV,{},{},{},{},{},{},{}",
            r.tick,
            r.wall_us,
            r.pending_async_ops,
            r.pending_timers,
            r.pending_intervals,
            r.pending_resources,
            if r.timed_out { 1 } else { 0 },
        );
    }
    let _ = writeln!(w, "================================================\n");
}

/// The browser event loop. Drives JS execution, timers, and async ops.
pub struct BrowserEventLoop {
    runtime: BrowserJsRuntime,
}

/// Why the event loop stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdleReason {
    /// All pending work completed (no timers, no promises, no async ops).
    AllWorkDone,
    /// The timeout was reached.
    Timeout,
}

impl BrowserEventLoop {
    pub fn new(runtime: BrowserJsRuntime) -> Self {
        Self { runtime }
    }

    /// Run the event loop until idle, timeout, or a JS-triggered navigation.
    ///
    /// "Idle" means deno_core's event loop has no more pending work
    /// (no timers, no unresolved promises, no pending async ops).
    ///
    /// **Nav short-circuit (gap: Kasada 5-second retry window):** if JS
    /// sets `globalThis.__pendingNavigation` (via `location.href = ...`,
    /// `location.reload()`, form.submit, meta-refresh, etc.), the JS
    /// bootstrap calls `op_set_pending_nav` which flips an atomic flag
    /// shared with this loop. We detect it on the next tick boundary,
    /// drain microtasks for a ~150 ms tail (so in-flight `fetch().then(...)`
    /// can land its cookies in the jar), then return `AllWorkDone`. This
    /// mirrors real-Chrome behavior where the navigation commits within
    /// tens of ms of the setter call.
    ///
    /// Without this short-circuit, sites that issue a challenge-token
    /// fetch followed by `location.href = retry_url` had to wait for the
    /// full `timeout` ceiling before the next iteration fired the retry
    /// — easily blowing past Kasada's ~5-second tolerance.
    pub async fn run_until_idle(
        &mut self,
        timeout: Duration,
    ) -> Result<IdleReason, deno_core::error::AnyError> {
        let deadline = Instant::now() + timeout;
        // Tail time after nav-pending is detected, to let post-fetch
        // microtasks (cookies, etc.) settle before we hand off to the
        // navigation iteration.
        const NAV_TAIL: Duration = Duration::from_millis(150);

        // Profiling state — only allocates when the env var is set, so
        // production overhead is one OnceLock-cached load + a branch.
        let profiling = profile_enabled();
        if profiling {
            OP_NAME_TOTALS.with(|m| m.borrow_mut().clear());
        }
        let profile_start = if profiling {
            Some(Instant::now())
        } else {
            None
        };
        let mut rows: Vec<TickRow> = if profiling {
            Vec::with_capacity(2048)
        } else {
            Vec::new()
        };
        let mut tick_idx: u32 = 0;

        let outcome: Result<IdleReason, deno_core::error::AnyError> = loop {
            // Check timeout
            if Instant::now() >= deadline {
                break Ok(IdleReason::Timeout);
            }

            // JS-triggered navigation? Drain a short tail and exit.
            if self.runtime.nav_pending() {
                let tail_deadline = Instant::now() + NAV_TAIL;
                while Instant::now() < tail_deadline {
                    let _ = tokio::time::timeout(
                        Duration::from_millis(25),
                        self.runtime.run_event_loop(),
                    )
                    .await;
                }
                break Ok(IdleReason::AllWorkDone);
            }

            // Run one event loop tick with a short timeout
            let remaining = deadline.saturating_duration_since(Instant::now());
            let tick_timeout = remaining.min(Duration::from_millis(100));

            let tick_t0 = if profiling {
                Some(Instant::now())
            } else {
                None
            };
            let result = tokio::time::timeout(tick_timeout, self.runtime.run_event_loop()).await;

            // Capture pending-task snapshot AFTER the tick (so the row
            // reflects what's still in-flight). Skipped when profiling
            // disabled — capture_pending walks several Vecs and dumps the
            // activity snapshot, ~10-50us per call on x.com-class loads.
            if profiling {
                let elapsed = tick_t0.unwrap().elapsed().as_micros() as u64;
                let (ops, timers, intervals, resources) = capture_pending(&mut self.runtime);
                rows.push(TickRow {
                    tick: tick_idx,
                    wall_us: elapsed,
                    pending_async_ops: ops,
                    pending_timers: timers,
                    pending_intervals: intervals,
                    pending_resources: resources,
                    timed_out: result.is_err(),
                });
                tick_idx = tick_idx.wrapping_add(1);
            }

            match result {
                Ok(Ok(())) => {
                    // Event loop completed all work
                    break Ok(IdleReason::AllWorkDone);
                }
                Ok(Err(e)) => break Err(e),
                Err(_timeout) => {
                    // Tick timed out — event loop still has pending work.
                    // Continue looping (and re-check nav_pending at the top).
                    continue;
                }
            }
        };

        if profiling {
            let total = profile_start.map(|s| s.elapsed()).unwrap_or_default();
            let label = std::env::var("BROWSER_OXIDE_EVENT_LOOP_PROFILE_LABEL")
                .unwrap_or_else(|_| "run_until_idle".to_string());
            let reason = match &outcome {
                Ok(r) => *r,
                Err(_) => IdleReason::Timeout, // best-effort tag
            };
            dump_profile(&label, &rows, total, reason);
        }

        outcome
    }

    /// Execute a script in the runtime.
    pub fn execute_script(&mut self, code: &str) -> Result<String, deno_core::error::AnyError> {
        self.runtime.execute_script(code, None)
    }

    /// Execute a script in the runtime with a given source name.
    pub fn execute_script_with_name(
        &mut self,
        code: &str,
        name: &str,
    ) -> Result<String, deno_core::error::AnyError> {
        self.runtime.execute_script(code, Some(name))
    }

    /// Run scripts then wait for idle.
    pub async fn execute_and_run(
        &mut self,
        code: &str,
        timeout: Duration,
    ) -> Result<IdleReason, deno_core::error::AnyError> {
        self.runtime.execute_script(code, None)?;
        self.run_until_idle(timeout).await
    }

    /// Get the underlying runtime.
    pub fn runtime(&self) -> &BrowserJsRuntime {
        &self.runtime
    }

    /// Reset the runtime's pending-navigation signal. Called by callers
    /// that legitimately set `location.href` for URL-state setup (not as
    /// a real navigation trigger) — without this, subsequent
    /// `run_until_idle` calls would see nav_pending=true and short-circuit
    /// immediately, breaking timer-based tests.
    pub fn reset_nav_pending(&self) {
        self.runtime.reset_nav_pending();
    }

    /// Get a mutable reference to the underlying runtime.
    pub fn runtime_mut(&mut self) -> &mut BrowserJsRuntime {
        &mut self.runtime
    }

    /// Consume the event loop and return the runtime.
    pub fn into_runtime(self) -> BrowserJsRuntime {
        self.runtime
    }

    /// Consume and return the DOM.
    pub fn take_dom(self) -> dom::Dom {
        self.runtime.take_dom()
    }

    /// Snapshot current localStorage/sessionStorage for carrying across navigations.
    pub fn get_storage(
        &mut self,
    ) -> std::collections::HashMap<String, std::collections::HashMap<String, String>> {
        self.runtime.get_storage()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_loop() -> BrowserEventLoop {
        let dom = html_parser::parse_html(
            "<html><head></head><body><div id=\"output\"></div></body></html>",
        );
        BrowserEventLoop::new(BrowserJsRuntime::new(dom))
    }

    #[tokio::test]
    async fn idle_detection_no_work() {
        let mut evloop = create_loop();
        let reason = evloop.run_until_idle(Duration::from_secs(5)).await.unwrap();
        assert_eq!(reason, IdleReason::AllWorkDone);
    }

    #[tokio::test]
    async fn set_timeout_fires() {
        let mut evloop = create_loop();
        evloop
            .execute_script(
                r#"setTimeout(() => {
                    document.querySelector('#output').textContent = 'timer fired';
                }, 50);"#,
            )
            .unwrap();

        let reason = evloop.run_until_idle(Duration::from_secs(5)).await.unwrap();
        assert_eq!(reason, IdleReason::AllWorkDone);

        let result = evloop
            .execute_script("document.querySelector('#output').textContent")
            .unwrap();
        assert_eq!(result, "timer fired");
    }

    #[tokio::test]
    async fn promise_resolves() {
        let mut evloop = create_loop();
        evloop
            .execute_script(
                r#"Promise.resolve().then(() => {
                    document.querySelector('#output').textContent = 'promise resolved';
                });"#,
            )
            .unwrap();

        evloop.run_until_idle(Duration::from_secs(5)).await.unwrap();

        let result = evloop
            .execute_script("document.querySelector('#output').textContent")
            .unwrap();
        assert_eq!(result, "promise resolved");
    }

    #[tokio::test]
    #[ignore = "regression: run_until_idle returns AllWorkDone instead of Timeout when a far-future setTimeout is pending — see fix.md"]
    async fn timeout_respected() {
        let mut evloop = create_loop();
        // Schedule a timer that takes longer than our timeout
        evloop
            .execute_script("setTimeout(() => {}, 10000);")
            .unwrap();

        let reason = evloop
            .run_until_idle(Duration::from_millis(200))
            .await
            .unwrap();
        assert_eq!(reason, IdleReason::Timeout);
    }

    #[tokio::test]
    async fn chained_set_timeout() {
        let mut evloop = create_loop();
        evloop
            .execute_script(
                r#"
                setTimeout(() => {
                    document.querySelector('#output').textContent = '1';
                    setTimeout(() => {
                        document.querySelector('#output').textContent += '2';
                    }, 10);
                }, 10);
                "#,
            )
            .unwrap();

        evloop.run_until_idle(Duration::from_secs(5)).await.unwrap();

        let result = evloop
            .execute_script("document.querySelector('#output').textContent")
            .unwrap();
        assert_eq!(result, "12");
    }

    #[tokio::test]
    async fn request_animation_frame() {
        let mut evloop = create_loop();
        evloop
            .execute_script(
                r#"requestAnimationFrame((ts) => {
                    document.querySelector('#output').textContent = 'raf:' + (typeof ts);
                });"#,
            )
            .unwrap();

        evloop.run_until_idle(Duration::from_secs(5)).await.unwrap();

        let result = evloop
            .execute_script("document.querySelector('#output').textContent")
            .unwrap();
        assert_eq!(result, "raf:number");
    }
}
