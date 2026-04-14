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
    pub fn new(max_size: usize) -> Self {
        Self {
            idle_pages: Arc::new(Mutex::new(VecDeque::with_capacity(max_size))),
            max_size,
        }
    }

    /// Acquire a page from the pool or create a new one.
    /// The page is sanitized (swapped to empty DOM) before being returned.
    pub async fn acquire(&self, profile: Option<StealthProfile>) -> Result<Page, deno_core::error::AnyError> {
        let mut pages = self.idle_pages.lock().unwrap();
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
        let mut pages = self.idle_pages.lock().unwrap();
        if pages.len() < self.max_size {
            pages.push_back(page);
        }
    }
}
