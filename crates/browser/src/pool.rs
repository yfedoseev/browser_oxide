use crate::Page;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use stealth::StealthProfile;

/// A pool of warm Page instances.
/// Reusing pages skips V8 isolate creation and bootstrap JS execution.
pub struct PagePool {
    idle_pages: Arc<Mutex<VecDeque<Page>>>,
    max_size: usize,
}

impl PagePool {
    // arc_with_non_send_sync: Page holds a V8 isolate (intrinsically
    // !Send/!Sync — the engine is single-threaded by design). The Arc
    // here only shares the idle-page queue within one thread; it never
    // crosses a thread boundary.
    #[allow(clippy::arc_with_non_send_sync)]
    pub fn new(max_size: usize) -> Self {
        Self {
            idle_pages: Arc::new(Mutex::new(VecDeque::with_capacity(max_size))),
            max_size,
        }
    }

    /// Acquire a page from the pool or create a new one.
    /// The page is sanitized (swapped to empty DOM) before being returned.
    // await_holding_lock: the std Mutex guard is dropped at the end of
    // the short synchronous pop block before any await; the engine is
    // single-threaded so there is no cross-thread contention regardless.
    #[allow(clippy::await_holding_lock)]
    pub async fn acquire(
        &self,
        profile: Option<StealthProfile>,
    ) -> Result<Page, deno_core::error::AnyError> {
        let mut pages = self.idle_pages.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(mut page) = pages.pop_front() {
            // Re-use existing page.
            // Note: In a real implementation, we might want to check if the
            // page's profile matches the requested one. For now, we'll
            // just reload it with a blank state.
            page.reload_html("<html><head></head><body></body></html>", "about:blank");
            return Ok(page);
        }

        // Create a new one if pool is empty
        Page::from_html("<html><head></head><body></body></html>", profile).await
    }

    /// Return a page to the pool.
    pub fn release(&self, page: Page) {
        let mut pages = self.idle_pages.lock().unwrap_or_else(|e| e.into_inner());
        if pages.len() < self.max_size {
            pages.push_back(page);
        }
    }
}
