//! V8 JavaScript runtime with DOM bindings for browser_oxide.
//!
//! MIT/Apache-2.0 licensed. Part of the browser_oxide project.

pub mod extensions;
pub mod runtime;
pub mod state;

use deno_core::JsRuntime;
use dom::Dom;
use runtime::{create_runtime, BrowserRuntimeOptions};
use state::{ConsoleMessage, DomState};
use stealth::StealthProfile;

/// A V8 JavaScript runtime with browser DOM bindings.
pub struct BrowserJsRuntime {
    inner: JsRuntime,
}

impl BrowserJsRuntime {
    /// Create a new runtime with the given DOM (no stealth profile).
    pub fn new(dom: Dom) -> Self {
        Self {
            inner: create_runtime(dom, BrowserRuntimeOptions::default()),
        }
    }

    /// Create with a stealth profile.
    pub fn with_profile(dom: Dom, profile: StealthProfile) -> Self {
        Self {
            inner: create_runtime(
                dom,
                BrowserRuntimeOptions {
                    stealth_profile: Some(profile),
                    ..Default::default()
                },
            ),
        }
    }

    /// Create with full options.
    pub fn with_options(dom: Dom, options: BrowserRuntimeOptions) -> Self {
        Self {
            inner: create_runtime(dom, options),
        }
    }

    /// Execute a JavaScript script and return the string representation of the result.
    ///
    /// Uses V8 directly in a single HandleScope — avoids the overhead of
    /// deno_core's `execute_script` (which allocates a Global handle) and
    /// a second `handle_scope()` call for stringification.
    pub fn execute_script(&mut self, code: &str) -> Result<String, deno_core::error::AnyError> {
        let scope = &mut self.inner.handle_scope();
        let source = deno_core::v8::String::new(scope, code)
            .ok_or_else(|| deno_core::error::AnyError::msg("failed to create V8 string"))?;
        let tc_scope = &mut deno_core::v8::TryCatch::new(scope);
        let script = deno_core::v8::Script::compile(tc_scope, source, None).ok_or_else(|| {
            let exception = tc_scope.exception().unwrap();
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
                let exception = tc_scope.exception().unwrap();
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

    /// Get the inner deno_core JsRuntime.
    pub fn inner(&mut self) -> &mut JsRuntime {
        &mut self.inner
    }

    /// Get the OpState (shared state).
    pub fn op_state(&mut self) -> std::rc::Rc<std::cell::RefCell<deno_core::OpState>> {
        self.inner.op_state()
    }
}
