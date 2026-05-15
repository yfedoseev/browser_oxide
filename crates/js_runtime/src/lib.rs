//! V8 JavaScript runtime with DOM bindings for browser_oxide.
//!
//! MIT/Apache-2.0 licensed. Part of the browser_oxide project.

pub mod extensions;
pub mod native_fns;
pub mod runtime;
pub mod snapshot;
pub mod state;
pub mod utils;

use deno_core::JsRuntime;
use dom::Dom;
use extensions::nav_ext::NavSignal;
use runtime::{create_runtime_with_signals, BrowserRuntimeOptions};
use state::{ConsoleMessage, DomState};
use stealth::StealthProfile;

/// A V8 JavaScript runtime with browser DOM bindings.
pub struct BrowserJsRuntime {
    inner: JsRuntime,
    /// Per-runtime navigation-pending signal. JS sets it via
    /// `op_set_pending_nav` (called from window_bootstrap.js whenever
    /// `__pendingNavigation` is assigned). The event loop polls it to
    /// short-circuit `run_until_idle` for fast nav handoff (gap: Kasada
    /// 5-second retry window).
    nav_signal: NavSignal,
}

impl BrowserJsRuntime {
    /// Create a new runtime with the given DOM (no stealth profile).
    pub fn new(dom: Dom) -> Self {
        let (inner, nav_signal) =
            create_runtime_with_signals(dom, BrowserRuntimeOptions::default());
        Self { inner, nav_signal }
    }

    /// Create with a stealth profile.
    pub fn with_profile(dom: Dom, profile: StealthProfile) -> Self {
        let (inner, nav_signal) = create_runtime_with_signals(
            dom,
            BrowserRuntimeOptions {
                stealth_profile: Some(profile),
                ..Default::default()
            },
        );
        Self { inner, nav_signal }
    }

    /// Create with full options.
    pub fn with_options(dom: Dom, mut options: BrowserRuntimeOptions) -> Self {
        if options.startup_snapshot.is_none() {
            options.startup_snapshot = Some(snapshot::get_snapshot());
        }
        let (inner, nav_signal) = create_runtime_with_signals(dom, options);
        Self { inner, nav_signal }
    }

    /// Returns true iff JS has set a pending navigation since the last
    /// reset. Cheap (atomic load); safe to poll from the event loop.
    pub fn nav_pending(&self) -> bool {
        self.nav_signal.pending()
    }

    /// Reset the pending-navigation flag. Called by the event loop after
    /// it has acted on the signal (e.g., before starting a fresh iteration).
    pub fn reset_nav_pending(&self) {
        self.nav_signal.reset();
    }

    /// Get a thread-safe handle to the V8 isolate. Used to call
    /// `terminate_execution()` from a watcher thread when a wall-clock
    /// deadline expires — preempts CPU-bound JS spin loops that
    /// `tokio::time::timeout` cannot interrupt because they never yield
    /// to the tokio scheduler. The returned handle is `Send + Sync`.
    pub fn isolate_handle(&mut self) -> deno_core::v8::IsolateHandle {
        self.inner.v8_isolate().thread_safe_handle()
    }

    /// Cancel a previously-issued `terminate_execution()`. Required if
    /// you want the runtime to be usable for further script execution
    /// after a deadline fired. Without this, the next `execute_script`
    /// returns "Uncaught Error: execution terminated".
    pub fn cancel_terminate_execution(&mut self) {
        self.inner.v8_isolate().cancel_terminate_execution();
    }

    /// Execute a JavaScript script and return the string representation of the result.
    ///
    /// Uses V8 directly in a single HandleScope — avoids the overhead of
    /// deno_core's `execute_script` (which allocates a Global handle) and
    /// a second `handle_scope()` call for stringification.
    pub fn execute_script(
        &mut self,
        code: &str,
        name: Option<&str>,
    ) -> Result<String, deno_core::error::AnyError> {
        let scope = &mut self.inner.handle_scope();
        let source = deno_core::v8::String::new(scope, code)
            .ok_or_else(|| deno_core::error::AnyError::msg("failed to create V8 string"))?;

        let mut script_origin = None;
        if let Some(n) = name {
            let n_v8 = deno_core::v8::String::new(scope, n).unwrap();
            let resource_name = n_v8.into();
            script_origin = Some(deno_core::v8::ScriptOrigin::new(
                scope,
                resource_name,
                0,
                0,
                false,
                0,
                None,
                false,
                false,
                false,
                None,
            ));
        }

        let tc_scope = &mut deno_core::v8::TryCatch::new(scope);
        let script = deno_core::v8::Script::compile(tc_scope, source, script_origin.as_ref())
            .ok_or_else(|| {
                let exception = match tc_scope.exception() {
                    Some(exc) => exc,
                    None => return deno_core::error::AnyError::msg("script compilation failed"),
                };
                let msg = exception
                    .to_string(tc_scope)
                    .map(|s| s.to_rust_string_lossy(tc_scope))
                    .unwrap_or_default();
                deno_core::error::AnyError::msg(msg)
            })?;
        match script.run(tc_scope) {
            Some(value) => Ok(value
                .to_string(tc_scope)
                .map(|s| s.to_rust_string_lossy(tc_scope))
                .unwrap_or_default()),
            None => {
                let exception = match tc_scope.exception() {
                    Some(exc) => exc,
                    None => return Err(deno_core::error::AnyError::msg("script execution failed")),
                };
                let msg = exception
                    .to_string(tc_scope)
                    .map(|s| s.to_rust_string_lossy(tc_scope))
                    .unwrap_or_default();
                Err(deno_core::error::AnyError::msg(msg))
            }
        }
    }

    /// Run the V8 event loop until all pending work is done.
    pub async fn run_event_loop(&mut self) -> Result<(), deno_core::error::AnyError> {
        self.inner
            .run_event_loop(deno_core::PollEventLoopOptions::default())
            .await
    }

    /// Get console output captured so far.
    pub fn console_output(&mut self) -> Vec<ConsoleMessage> {
        let state = self.inner.op_state();
        let state = state.borrow();
        state.borrow::<DomState>().console_output.clone()
    }

    /// Replace the DOM in this runtime with a new one.
    /// Used for CDP Page.navigate to avoid recreating the V8 isolate.
    pub fn replace_dom(&mut self, dom: Dom, stylesheets: Vec<String>) {
        let state = self.inner.op_state();
        let mut state = state.borrow_mut();
        // Replace DomState — ops will pick up the new DOM on next call
        let mut dom_state = DomState::new(dom);
        dom_state.stylesheets = stylesheets;
        dom_state.update_cached_rules();
        state.put(dom_state);
        // Reset timer state (clear pending timers from old page)
        state.put(extensions::timer_ext::TimerState::new());
    }

    /// Take the DOM out of the runtime (consumes self).
    pub fn take_dom(mut self) -> Dom {
        let state = self.inner.op_state();
        let mut state = state.borrow_mut();
        state.take::<DomState>().dom
    }

    /// Snapshot the current localStorage and sessionStorage contents.
    /// Used by the navigation loop to carry storage across same-origin reloads.
    pub fn get_storage(
        &mut self,
    ) -> std::collections::HashMap<String, std::collections::HashMap<String, String>> {
        let state = self.inner.op_state();
        let state = state.borrow();
        state.borrow::<DomState>().storage.clone()
    }

    /// Get the inner deno_core JsRuntime.
    pub fn inner(&mut self) -> &mut JsRuntime {
        &mut self.inner
    }

    /// Get the OpState (shared state).
    pub fn op_state(&mut self) -> std::rc::Rc<std::cell::RefCell<deno_core::OpState>> {
        self.inner.op_state()
    }

    pub fn record_resource_timing(&mut self, timings: net::TimingStats) {
        let op_state = self.inner.op_state();
        let mut state = op_state.borrow_mut();
        extensions::fetch_ext::record_resource_timing(&mut state, timings);
    }
}
