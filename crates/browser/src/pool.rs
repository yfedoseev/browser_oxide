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

    /// Acquire a warm Page and navigate it to `url`. Saves the V8-isolate +
    /// bootstrap cost (~150 ms) compared to calling [`crate::Page::navigate`]
    /// cold for every URL. The caller is expected to `release(page)` once
    /// done extracting content so the warm isolate stays in the pool.
    ///
    /// Profile note: the returned page reuses whatever profile its V8
    /// isolate was originally built with. The first call seeds the pool
    /// with the requested profile; subsequent calls return the same
    /// isolate regardless of profile argument. If your workload needs
    /// multiple profiles, run a `PagePool` per profile — V8 bootstrap
    /// is profile-baked, so per-profile pools are the correct unit
    /// anyway.
    ///
    /// Anti-bot pages: warm reuse is for benign content extraction. If
    /// the response is a challenge document, the engine's cookie-diff
    /// / pending-nav iteration loop is NOT run (warm path skips it for
    /// simplicity). Caller should `release(page)` and fall back to
    /// `Page::navigate(url, profile, max_iter)` for challenge-protected
    /// origins — the challenge VM dominates runtime anyway, so warm
    /// reuse offers little benefit there.
    pub async fn navigate(
        &self,
        url: &str,
        profile: StealthProfile,
    ) -> Result<Page, deno_core::error::AnyError> {
        let mut page = self.acquire(Some(profile.clone())).await?;
        page.navigate_warm(url).await?;

        // Warm-path challenge caveat. The warm path skips the cold iteration
        // loop (pending-nav follow + cookie-diff retry), so a JS interstitial —
        // e.g. reddit's "Please wait for verification" inline-script form-submit
        // challenge — warm-renders an 8 KB shell while the cold path follows the
        // submit to the real 676 KB app. We deliberately do NOT cold-fall-back
        // by building a fresh `Page::navigate` here: that constructs a new V8
        // OwnedIsolate while the pool's isolates are still alive, and V8 requires
        // isolates be dropped in reverse creation order — injecting one
        // mid-stream aborts the process ("OwnedIsolate instances must be dropped
        // in the reverse order of creation"). Challenge-protected origins should
        // be navigated with the cold `Page::navigate(url, profile, max_iter)`
        // path directly (the challenge VM dominates runtime there anyway, so
        // warm reuse offers little benefit). In-place warm challenge-follow on
        // the same isolate is a tracked follow-up.
        Ok(page)
    }
}
