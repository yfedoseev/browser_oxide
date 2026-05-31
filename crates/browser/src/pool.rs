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

        // Warm-path challenge/thin guard. The warm path skips the cold
        // iteration loop (pending-nav follow + cookie-diff retry), so a site
        // that serves a JS interstitial returns the UNSOLVED shell — e.g.
        // reddit's "Please wait for verification" page (an inline-script
        // form-submit challenge) warm-renders 8 KB, while the cold path follows
        // the submit and gets the real 676 KB app. Since the pooled gate (and
        // any library user) takes this result verbatim, every challenge/
        // interstitial site was silently under-counted. Detect a sub-threshold
        // or challenge-flagged warm result and fall back to the authoritative
        // cold `Page::navigate`, which runs the full solve loop. Gated on a thin
        // body (< the 15 KB pass threshold) so benign pages — the warm
        // fast-path's whole purpose — keep the warm result with zero extra cost.
        let thin = {
            let content = page.content();
            crate::engine_classify(&content).len < 15_000
        };
        if thin || page.is_anti_bot_challenge() {
            self.release(page);
            return Page::navigate(url, profile, 3).await;
        }
        Ok(page)
    }
}
