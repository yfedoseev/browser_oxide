//! Navigation-pending signal — fast cross-thread flag the JS bootstrap
//! flips when `__pendingNavigation` (or any equivalent setter) is set.
//!
//! Why this exists: the navigation pipeline in `Page::navigate` runs an
//! iteration loop where each iteration does `event_loop.run_until_idle(30s)`.
//! Without this signal, when a script sets `location.href = ...` the
//! iteration still runs to its 30-second ceiling before the retry GET
//! fires — too late for anti-bot challenges with strict timing windows
//! (Kasada validates protected-GET-after-/tl-response within ~5 seconds).
//!
//! This op flips an `Arc<AtomicBool>` shared with `BrowserEventLoop`. The
//! event loop checks the flag each tick and exits early (after a brief
//! microtask tail to let in-flight `fetch().then(setCookie)` land in the
//! jar before the retry).
//!
//! See `docs/SOTA_ROADMAP_2026.md` and `docs/TIER0_KASADA_RESULTS.md` for
//! the timing-window context.

use deno_core::op2;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// State stored in OpState. Cloned by `BrowserJsRuntime::nav_pending_signal`
/// so the event loop can read the flag without going through V8.
#[derive(Clone, Default)]
pub struct NavSignal(pub Arc<AtomicBool>);

impl NavSignal {
    pub fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    pub fn raise(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn pending(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    pub fn reset(&self) {
        self.0.store(false, Ordering::Relaxed);
    }
}

#[op2(fast)]
pub fn op_set_pending_nav(#[state] s: &NavSignal) {
    s.raise();
}

deno_core::extension!(nav_extension, ops = [op_set_pending_nav],);
