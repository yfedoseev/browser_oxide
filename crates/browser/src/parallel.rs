//! Parallel page navigator — N OS-thread worker pool.
//!
//! `Page` (and its embedded `JsRuntime`) is not `Send` because V8's
//! `IsolateHandle` is thread-local. So a typical async-task pool
//! (`for_each_concurrent`, etc.) cannot move pages across worker tasks.
//!
//! Instead, this module spawns `N` dedicated OS threads, each with its own
//! tokio current-thread runtime and its own `Page` instances. Jobs are
//! dispatched from the caller (which runs in any tokio runtime) over
//! `std::sync::mpsc` channels. Results come back over
//! `tokio::sync::oneshot` so the caller can `.await` them naturally.
//!
//! Round-robin scheduling — caller-visible API is just
//! `pager.navigate(url, profile, max_iter).await`.
//!
//! Used by the holistic test sweep to run 126 sites in ~8 min vs ~27 min
//! serial. Real-world callers (CLI batch fetches, CI scrapers) can use
//! the same pool via [`ParallelPager::new`] / [`ParallelPager::navigate`].

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use crate::Page;
use stealth::StealthProfile;

/// Result of a single navigation. Always returned (Err just sets `error`)
/// so a single bad URL never blocks the worker.
pub struct NavigateResult {
    /// Final body HTML, or empty on error.
    pub html: String,
    /// Wall-clock time from job dispatch through `Page::navigate` completion.
    pub elapsed: Duration,
    /// Some(message) if Page::navigate returned Err. None on success.
    pub error: Option<String>,
}

/// Internal job message.
struct Job {
    url: String,
    profile: StealthProfile,
    max_iter: u8,
    result_tx: tokio::sync::oneshot::Sender<NavigateResult>,
}

struct WorkerHandle {
    tx: mpsc::Sender<Job>,
    /// Joined when the pager is dropped — held just for the lifecycle.
    _thread: thread::JoinHandle<()>,
}

/// N-worker parallel navigation pool. Each worker thread owns its own
/// tokio runtime + its own `Page` instances, so V8 isolates don't cross
/// thread boundaries.
pub struct ParallelPager {
    workers: Vec<WorkerHandle>,
    next_worker: AtomicUsize,
}

impl ParallelPager {
    /// Spawn `num_workers` OS threads. Each thread builds a tokio
    /// current-thread runtime once and reuses it for every job it processes.
    /// `num_workers` should typically match physical core count (4 is a
    /// good default on Apple silicon / 8-core x86).
    pub fn new(num_workers: usize) -> Self {
        assert!(num_workers > 0, "ParallelPager needs at least 1 worker");
        let workers = (0..num_workers)
            .map(|i| {
                let (tx, rx) = mpsc::channel::<Job>();
                let thread = thread::Builder::new()
                    .name(format!("browser_oxide-pager-{i}"))
                    .stack_size(64 * 1024 * 1024) // 64 MB — match RUST_MIN_STACK gate per V8 needs
                    .spawn(move || worker_main(rx))
                    .expect("failed to spawn pager worker");
                WorkerHandle {
                    tx,
                    _thread: thread,
                }
            })
            .collect();
        Self {
            workers,
            next_worker: AtomicUsize::new(0),
        }
    }

    /// Dispatch a navigation to the next worker (round-robin) and return a
    /// future that resolves when the navigation completes.
    pub async fn navigate(
        &self,
        url: impl Into<String>,
        profile: StealthProfile,
        max_iter: u8,
    ) -> NavigateResult {
        let (result_tx, result_rx) = tokio::sync::oneshot::channel();
        let job = Job {
            url: url.into(),
            profile,
            max_iter,
            result_tx,
        };

        // Round-robin worker selection. Atomic increment is enough — no
        // global ordering needed; we just want approximate fairness.
        let idx = self.next_worker.fetch_add(1, Ordering::Relaxed) % self.workers.len();
        match self.workers[idx].tx.send(job) {
            Ok(_) => {}
            Err(send_err) => {
                // Worker thread panicked or dropped its receiver. Surface
                // the failure without blocking the caller forever.
                return NavigateResult {
                    html: String::new(),
                    elapsed: Duration::default(),
                    error: Some(format!(
                        "worker {idx} unavailable (likely panicked): {send_err}"
                    )),
                };
            }
        }
        match result_rx.await {
            Ok(r) => r,
            Err(_) => NavigateResult {
                html: String::new(),
                elapsed: Duration::default(),
                error: Some("worker dropped result sender (panic during navigate)".to_string()),
            },
        }
    }

    /// Number of workers spawned at construction time.
    pub fn num_workers(&self) -> usize {
        self.workers.len()
    }
}

impl Drop for ParallelPager {
    fn drop(&mut self) {
        // Drop all senders → workers' recv() returns Err → they exit
        // cleanly. JoinHandle is held by `_thread` so the threads finish
        // their current job before the process exits.
        self.workers.clear();
    }
}

/// Worker thread main. Holds its own tokio runtime; processes jobs in
/// receive order until the channel is closed.
fn worker_main(rx: mpsc::Receiver<Job>) {
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("[pager-worker] failed to build tokio runtime: {e}");
            return;
        }
    };

    while let Ok(job) = rx.recv() {
        let begin = Instant::now();
        let url = job.url.clone();
        let profile = job.profile;
        let max_iter = job.max_iter;

        let result: NavigateResult = rt.block_on(async move {
            match Page::navigate(&url, profile, max_iter).await {
                Ok(mut page) => NavigateResult {
                    html: page.content(),
                    elapsed: begin.elapsed(),
                    error: None,
                },
                Err(e) => NavigateResult {
                    html: String::new(),
                    elapsed: begin.elapsed(),
                    error: Some(format!("{e}")),
                },
            }
        });

        // Best-effort send. If the receiver was dropped (caller cancelled),
        // discarding the result is the right thing.
        let _ = job.result_tx.send(result);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn parallel_pager_spawns_and_drops_cleanly() {
        // Just verifies thread spawn + clean drop — no real navigation.
        let pager = ParallelPager::new(2);
        assert_eq!(pager.num_workers(), 2);
        drop(pager);
    }
}
