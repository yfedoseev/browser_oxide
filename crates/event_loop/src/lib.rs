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

    /// Run the event loop until idle or timeout.
    ///
    /// "Idle" means deno_core's event loop has no more pending work
    /// (no timers, no unresolved promises, no pending async ops).
    pub async fn run_until_idle(
        &mut self,
        timeout: Duration,
    ) -> Result<IdleReason, deno_core::error::AnyError> {
        let deadline = Instant::now() + timeout;

        loop {
            // Check timeout
            if Instant::now() >= deadline {
                return Ok(IdleReason::Timeout);
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
                    // Continue looping.
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
