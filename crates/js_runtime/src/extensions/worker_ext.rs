//! Web Worker implementation with real thread-based V8 isolates.
//!
//! Each `new Worker(url)` in JS spawns an OS thread that owns its own
//! `JsRuntime` built via `create_worker_runtime`. Messages cross the thread
//! boundary through `std::sync::mpsc` channels: parent↔worker uses two
//! unidirectional channels, one each way.
//!
//! Also hosts a process-global BlobRegistry so that `URL.createObjectURL(blob)`
//! produces a blob: URL whose source text can be resolved when a worker is
//! spawned from it (Akamai's BMP v3 spawns workers via blob: URLs built from
//! inline scripts).

use deno_core::op2;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};

// ============================================================================
// BlobRegistry — backs URL.createObjectURL / .revokeObjectURL / blob: loader.
// ============================================================================

struct BlobEntry {
    data: Vec<u8>,
    content_type: String,
}

struct BlobRegistry {
    blobs: HashMap<String, BlobEntry>,
}

fn blob_registry() -> &'static Mutex<BlobRegistry> {
    static INST: OnceLock<Mutex<BlobRegistry>> = OnceLock::new();
    INST.get_or_init(|| {
        Mutex::new(BlobRegistry {
            blobs: HashMap::new(),
        })
    })
}

/// Register a blob's bytes + MIME type under a blob: URL. Called from
/// `URL.createObjectURL`. `content_type` comes from the `Blob.type`
/// field; may be empty string for unspecified blobs.
#[op2(fast)]
pub fn op_blob_register(
    #[string] url: String,
    #[buffer] data: &[u8],
    #[string] content_type: String,
) {
    let mut reg = blob_registry().lock().unwrap();
    reg.blobs.insert(
        url,
        BlobEntry {
            data: data.to_vec(),
            content_type,
        },
    );
}

/// Fetch a blob's text content (UTF-8 lossy) by blob: URL. Used by
/// worker spawning when the script is loaded from a blob: URL, and by
/// the classic-script `importScripts` path.
#[op2]
#[string]
pub fn op_blob_fetch_text(#[string] url: String) -> String {
    let reg = blob_registry().lock().unwrap();
    match reg.blobs.get(&url) {
        Some(entry) => String::from_utf8_lossy(&entry.data).to_string(),
        None => String::new(),
    }
}

/// Full response shape for `fetch(blob:...)`: raw bytes + MIME. The JS
/// side constructs a synthetic `Response` from this, so the fetch
/// flow doesn't have to reach into the HTTP client for blob: URLs.
#[derive(serde::Serialize)]
pub struct JsBlobResponse {
    /// Raw bytes of the blob. Transported as a Vec<u8> so binary data
    /// survives round-trip (a base64 detour would be lossy for some
    /// encodings and needlessly slow for big buffers).
    pub bytes: Vec<u8>,
    pub content_type: String,
    pub found: bool,
}

/// Binary blob fetch — returns both the bytes and the `Blob.type`
/// string that was passed at registration time. Returns `found=false`
/// for unknown / revoked URLs so the JS side can synthesise a 404.
#[op2]
#[serde]
pub fn op_blob_fetch_bytes(#[string] url: String) -> JsBlobResponse {
    let reg = blob_registry().lock().unwrap();
    match reg.blobs.get(&url) {
        Some(entry) => JsBlobResponse {
            bytes: entry.data.clone(),
            content_type: entry.content_type.clone(),
            found: true,
        },
        None => JsBlobResponse {
            bytes: Vec::new(),
            content_type: String::new(),
            found: false,
        },
    }
}

#[op2(fast)]
pub fn op_blob_revoke(#[string] url: String) {
    let mut reg = blob_registry().lock().unwrap();
    reg.blobs.remove(&url);
}

/// Synchronous HTTP(S) fetch for worker `importScripts(url)`. Classic
/// workers spec the call as blocking: JS stays on-thread until the
/// response arrives. Because the worker thread is already inside its
/// own tokio `block_on`, we can't reuse that runtime — spinning up a
/// fresh single-threaded runtime on a short-lived helper thread
/// avoids the nested-block_on panic.
///
/// Returns the response body as UTF-8 (lossy on invalid sequences).
/// Empty string means "not fetched" — the JS side interprets that as
/// a network error and throws.
#[op2]
#[string]
pub fn op_worker_sync_fetch(#[string] url: String) -> String {
    // Clone the process-global fetch client so the helper thread
    // inherits profile + cookie state. Falls back to a default
    // chrome_130_linux client if no profile was wired (matches the
    // main-thread fetch_ext fallback).
    let client = match crate::extensions::fetch_ext::fetch_client() {
        Some(c) => c,
        None => match net::HttpClient::new(&stealth::chrome_130_linux()) {
            Ok(c) => c,
            Err(_) => return String::new(),
        },
    };

    let (tx, rx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(_) => {
                let _ = tx.send(String::new());
                return;
            }
        };
        let body = rt.block_on(async move {
            match client.get(&url).await {
                Ok(resp) if resp.ok() => resp.text(),
                _ => String::new(),
            }
        });
        let _ = tx.send(body);
    });

    // Block the worker thread until the helper returns. Max wait
    // 30 seconds to match the page event-loop timeout.
    rx.recv_timeout(std::time::Duration::from_secs(30))
        .unwrap_or_default()
}

// ============================================================================
// Worker registry (parent side).
// ============================================================================

struct WorkerSlot {
    to_worker: Sender<String>,
    from_worker: Receiver<String>,
    terminate: Arc<AtomicBool>,
}

fn worker_registry() -> &'static Mutex<HashMap<u32, WorkerSlot>> {
    static INST: OnceLock<Mutex<HashMap<u32, WorkerSlot>>> = OnceLock::new();
    INST.get_or_init(|| Mutex::new(HashMap::new()))
}

static NEXT_WORKER_ID: AtomicU32 = AtomicU32::new(1);

// ============================================================================
// Per-thread worker "self" state — populated when a worker thread starts.
// ============================================================================

struct WorkerSelf {
    to_parent: Sender<String>,
    from_parent: Receiver<String>,
}

thread_local! {
    static WORKER_SELF: RefCell<Option<WorkerSelf>> = const { RefCell::new(None) };
}

// ============================================================================
// Ops — parent side.
// ============================================================================

#[op2(fast)]
#[smi]
pub fn op_worker_spawn(
    #[string] script: String,
    #[string] _name: String,
    is_module: bool,
) -> i32 {
    let (to_worker_tx, to_worker_rx) = std::sync::mpsc::channel::<String>();
    let (to_parent_tx, to_parent_rx) = std::sync::mpsc::channel::<String>();
    let terminate = Arc::new(AtomicBool::new(false));
    let worker_id = NEXT_WORKER_ID.fetch_add(1, Ordering::Relaxed);

    {
        let mut reg = worker_registry().lock().unwrap();
        reg.insert(
            worker_id,
            WorkerSlot {
                to_worker: to_worker_tx,
                from_worker: to_parent_rx,
                terminate: terminate.clone(),
            },
        );
    }

    let thread_result = std::thread::Builder::new()
        .name(format!("worker-{worker_id}"))
        .spawn(move || {
            // Install per-thread worker state BEFORE any ops run.
            WORKER_SELF.with(|w| {
                *w.borrow_mut() = Some(WorkerSelf {
                    to_parent: to_parent_tx,
                    from_parent: to_worker_rx,
                });
            });

            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    eprintln!("worker {worker_id}: tokio build error: {e}");
                    return;
                }
            };

            let local = tokio::task::LocalSet::new();
            local.block_on(&rt, async move {
                let mut runtime = crate::runtime::create_worker_runtime();

                // Execute the worker script inside the worker's isolate.
                // Module workers go through `load_main_es_module_from_code`
                // so top-level `import.meta` and module-scoped evaluation
                // work the way sites expect. Classic workers stick with
                // the direct `execute_script` path.
                if is_module {
                    let specifier = deno_core::ModuleSpecifier::parse(&format!(
                        "worker-oxide://{worker_id}/main.mjs"
                    ))
                    .expect("worker-oxide URL parses");
                    match runtime
                        .load_main_es_module_from_code(&specifier, script)
                        .await
                    {
                        Ok(mod_id) => {
                            let eval_fut = runtime.mod_evaluate(mod_id);
                            // Drive the event loop alongside evaluation so
                            // async top-level work in the module body
                            // resolves. Ignore the eval result here — we
                            // want to continue even if the module throws
                            // so the worker stays alive for onmessage.
                            if let Err(e) = eval_fut.await {
                                eprintln!(
                                    "worker {worker_id} module eval error: {e}"
                                );
                            }
                        }
                        Err(e) => {
                            eprintln!("worker {worker_id} module load error: {e}");
                        }
                    }
                } else if let Err(e) = runtime.execute_script("<worker_script>", script) {
                    eprintln!("worker {worker_id} script error: {e}");
                }

                // Drive the event loop until terminated. A small polling
                // cadence lets us observe both parent messages and terminate
                // signals.
                while !terminate.load(Ordering::Acquire) {
                    let fut = Box::pin(
                        runtime.run_event_loop(deno_core::PollEventLoopOptions::default()),
                    );
                    let tick =
                        tokio::time::timeout(std::time::Duration::from_millis(25), fut).await;
                    match tick {
                        Ok(Ok(())) => {
                            // All pending work done — yield briefly and check
                            // again for incoming parent messages / terminate.
                            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                        }
                        Ok(Err(e)) => {
                            eprintln!("worker {worker_id}: event loop error: {e}");
                            break;
                        }
                        Err(_) => {
                            // Tick timeout — event loop still has work.
                            continue;
                        }
                    }
                }

                // Clear thread-local worker state.
                WORKER_SELF.with(|w| *w.borrow_mut() = None);
            });
        });

    if let Err(e) = thread_result {
        eprintln!("worker {worker_id}: thread spawn failed: {e}");
        worker_registry().lock().unwrap().remove(&worker_id);
        return 0;
    }

    worker_id as i32
}

#[op2(fast)]
pub fn op_worker_post_to_worker(#[smi] worker_id: i32, #[string] data: String) {
    let reg = worker_registry().lock().unwrap();
    if let Some(slot) = reg.get(&(worker_id as u32)) {
        let _ = slot.to_worker.send(data);
    }
}

/// Return the next pending message from a worker, or the empty string if none.
/// Empty string is safe as a sentinel because our JS wrapper JSON-encodes
/// every payload — an empty JSON encoding of a real message is never "".
#[op2]
#[string]
pub fn op_worker_poll_from_worker(#[smi] worker_id: i32) -> String {
    let reg = worker_registry().lock().unwrap();
    if let Some(slot) = reg.get(&(worker_id as u32)) {
        match slot.from_worker.try_recv() {
            Ok(msg) => return msg,
            Err(_) => return String::new(),
        }
    }
    String::new()
}

#[op2(fast)]
pub fn op_worker_terminate(#[smi] worker_id: i32) {
    let mut reg = worker_registry().lock().unwrap();
    if let Some(slot) = reg.get(&(worker_id as u32)) {
        slot.terminate.store(true, Ordering::Release);
    }
    reg.remove(&(worker_id as u32));
}

// ============================================================================
// Ops — worker side (read from thread-local WORKER_SELF).
// ============================================================================

#[op2(fast)]
pub fn op_worker_self_post(#[string] data: String) {
    WORKER_SELF.with(|w| {
        if let Some(s) = w.borrow().as_ref() {
            let _ = s.to_parent.send(data);
        }
    });
}

#[op2]
#[string]
pub fn op_worker_self_recv() -> String {
    WORKER_SELF.with(|w| {
        if let Some(s) = w.borrow().as_ref() {
            match s.from_parent.try_recv() {
                Ok(msg) => msg,
                Err(_) => String::new(),
            }
        } else {
            String::new()
        }
    })
}

deno_core::extension!(
    worker_extension,
    ops = [
        op_blob_register,
        op_blob_fetch_text,
        op_blob_fetch_bytes,
        op_blob_revoke,
        op_worker_sync_fetch,
        op_worker_spawn,
        op_worker_post_to_worker,
        op_worker_poll_from_worker,
        op_worker_terminate,
        op_worker_self_post,
        op_worker_self_recv,
    ],
);
