//! Browser event loop wrapping deno_core's V8 event loop with
//! timer scheduling, requestAnimationFrame, and idle detection.

use js_runtime::BrowserJsRuntime;
use std::time::{Duration, Instant};

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

        loop {
            // Check timeout
            if Instant::now() >= deadline {
                return Ok(IdleReason::Timeout);
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
                return Ok(IdleReason::AllWorkDone);
            }

            // Run one event loop tick with a short timeout
            let remaining = deadline.saturating_duration_since(Instant::now());
            let tick_timeout = remaining.min(Duration::from_millis(100));

            let result = tokio::time::timeout(tick_timeout, self.runtime.run_event_loop()).await;

            match result {
                Ok(Ok(())) => {
                    // Event loop completed all work
                    return Ok(IdleReason::AllWorkDone);
                }
                Ok(Err(e)) => return Err(e),
                Err(_timeout) => {
                    // Tick timed out — event loop still has pending work.
                    // Continue looping (and re-check nav_pending at the top).
                    continue;
                }
            }
        }
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
    pub fn get_storage(&mut self) -> std::collections::HashMap<String, std::collections::HashMap<String, String>> {
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
