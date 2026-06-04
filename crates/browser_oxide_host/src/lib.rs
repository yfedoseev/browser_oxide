//! `browser_oxide_host` — a `Send + Sync` handle over the `!Send` engine.
//!
//! `browser::Page` and the V8 isolate it owns are per-thread (`!Send`), so they
//! can never cross a thread boundary. Language bindings (Python, MCP, any FFI)
//! still need a *movable, shareable* handle they can call from an arbitrary
//! caller thread. This crate is that bridge: it owns one dedicated OS thread
//! running a current-thread tokio runtime + `LocalSet`, keeps the live `Page`
//! there, and marshals requests/results across channels. The `Page` never
//! moves; only `Send` snapshots cross back.
//!
//! ```no_run
//! let engine = browser_oxide_host::EngineHandle::spawn();
//! let snap = engine.navigate("https://example.com",
//!                            stealth::presets::chrome_148_macos(), 5).unwrap();
//! println!("{} ({} bytes, {})", snap.title, snap.html.len(), snap.verdict);
//! let ua = engine.evaluate("navigator.userAgent").unwrap();
//! ```

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;
use std::thread::JoinHandle;

pub use stealth::{self, StealthProfile};

/// A `Send` snapshot of a rendered page (the `Page` itself stays on the engine
/// thread).
#[derive(Debug, Clone)]
pub struct PageSnapshot {
    pub url: String,
    pub title: String,
    pub html: String,
    pub text: String,
    /// Honest outcome tag: `pass`, `thin-shell`, `render-incomplete`,
    /// `edge-block`, `sensor-fail`, `challenge-incomplete`.
    pub verdict: String,
    /// Whether `verdict` is one of the challenge classes.
    pub is_challenge: bool,
}

/// Errors surfaced across the channel boundary (all `Send`).
#[derive(Debug, thiserror::Error)]
pub enum HostError {
    #[error("engine thread is no longer running")]
    Disconnected,
    #[error("no current page — call navigate() first")]
    NoPage,
    #[error("engine error: {0}")]
    Engine(String),
}

enum Cmd {
    Navigate {
        url: String,
        profile: Box<StealthProfile>,
        max_iter: u8,
        reply: Sender<Result<PageSnapshot, HostError>>,
    },
    Evaluate {
        js: String,
        reply: Sender<Result<String, HostError>>,
    },
    QueryText {
        selector: String,
        reply: Sender<Result<Option<String>, HostError>>,
    },
    Shutdown,
}

/// A movable, cloneable* handle to the engine thread.
///
/// \*Clone the inner `Sender` if you need multiple producers; the bundled
/// thread join lives on the original handle and runs on `Drop`.
pub struct EngineHandle {
    // `Mutex` makes the handle `Sync` (a bare `mpsc::Sender` is `!Sync`), so it
    // can sit behind an `Arc` shared by the Python `Browser`/`Page` objects and
    // be borrowed across `Python::allow_threads`.
    tx: Mutex<Sender<Cmd>>,
    thread: Option<JoinHandle<()>>,
}

impl EngineHandle {
    /// Spawn the dedicated engine thread and return a handle to it.
    pub fn spawn() -> Self {
        let (tx, rx) = channel::<Cmd>();
        let thread = std::thread::Builder::new()
            .name("browser-oxide-engine".into())
            .spawn(move || engine_loop(rx))
            .expect("failed to spawn browser_oxide engine thread");
        EngineHandle {
            tx: Mutex::new(tx),
            thread: Some(thread),
        }
    }

    /// Navigate (cold path, humanized) and return a snapshot. Blocks the caller
    /// until the engine thread finishes the navigation.
    pub fn navigate(
        &self,
        url: &str,
        profile: StealthProfile,
        max_iter: u8,
    ) -> Result<PageSnapshot, HostError> {
        let (reply, rx) = channel();
        self.tx
            .lock()
            .map_err(|_| HostError::Disconnected)?
            .send(Cmd::Navigate {
                url: url.to_string(),
                profile: Box::new(profile),
                max_iter,
                reply,
            })
            .map_err(|_| HostError::Disconnected)?;
        rx.recv().map_err(|_| HostError::Disconnected)?
    }

    /// Evaluate JS against the page from the most recent `navigate`.
    pub fn evaluate(&self, js: &str) -> Result<String, HostError> {
        let (reply, rx) = channel();
        self.tx
            .lock()
            .map_err(|_| HostError::Disconnected)?
            .send(Cmd::Evaluate {
                js: js.to_string(),
                reply,
            })
            .map_err(|_| HostError::Disconnected)?;
        rx.recv().map_err(|_| HostError::Disconnected)?
    }

    /// `querySelector(selector)?.textContent` against the current page.
    pub fn query_text(&self, selector: &str) -> Result<Option<String>, HostError> {
        let (reply, rx) = channel();
        self.tx
            .lock()
            .map_err(|_| HostError::Disconnected)?
            .send(Cmd::QueryText {
                selector: selector.to_string(),
                reply,
            })
            .map_err(|_| HostError::Disconnected)?;
        rx.recv().map_err(|_| HostError::Disconnected)?
    }
}

impl Drop for EngineHandle {
    fn drop(&mut self) {
        if let Ok(tx) = self.tx.lock() {
            let _ = tx.send(Cmd::Shutdown);
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// The engine thread's main loop. Owns the live `Page` and a current-thread
/// runtime + `LocalSet` (required because the engine spawns `!Send` tasks).
fn engine_loop(rx: Receiver<Cmd>) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to build engine runtime");
    let local = tokio::task::LocalSet::new();

    local.block_on(&rt, async move {
        let mut current: Option<browser::Page> = None;

        // Blocking recv() between commands is fine: this thread does nothing
        // else, and during a navigate `.await` the LocalSet drives the engine's
        // own tasks. Commands are processed strictly one at a time.
        while let Ok(cmd) = rx.recv() {
            match cmd {
                Cmd::Shutdown => break,
                Cmd::Navigate {
                    url,
                    profile,
                    max_iter,
                    reply,
                } => {
                    let res = match browser::Page::navigate(&url, *profile, max_iter).await {
                        Ok(mut page) => {
                            let verdict = page.challenge_verdict();
                            let snap = PageSnapshot {
                                url: page.url().to_string(),
                                title: page.title(),
                                html: page.content(),
                                text: page.text_content(),
                                verdict: verdict.as_str().to_string(),
                                is_challenge: verdict.is_challenge(),
                            };
                            current = Some(page);
                            Ok(snap)
                        }
                        Err(e) => Err(HostError::Engine(e.to_string())),
                    };
                    let _ = reply.send(res);
                }
                Cmd::Evaluate { js, reply } => {
                    let res = match current.as_mut() {
                        Some(p) => p
                            .evaluate(&js)
                            .map_err(|e| HostError::Engine(e.to_string())),
                        None => Err(HostError::NoPage),
                    };
                    let _ = reply.send(res);
                }
                Cmd::QueryText { selector, reply } => {
                    let res = match current.as_mut() {
                        Some(p) => Ok(p.text_of(&selector)),
                        None => Err(HostError::NoPage),
                    };
                    let _ = reply.send(res);
                }
            }
        }
    });
}
