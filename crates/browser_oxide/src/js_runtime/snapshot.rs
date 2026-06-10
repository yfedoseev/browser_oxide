use crate::dom::Dom;
use crate::js_runtime::extensions::audio_ext::audio_extension;
use crate::js_runtime::extensions::canvas_ext::{canvas_extension, CanvasState};
use crate::js_runtime::extensions::console_ext::console_extension;
use crate::js_runtime::extensions::crypto_ext::crypto_extension;
use crate::js_runtime::extensions::dom_ext::dom_extension;
use crate::js_runtime::extensions::fetch_ext::{fetch_extension, FetchState};
use crate::js_runtime::extensions::layout_ext::layout_extension;
use crate::js_runtime::extensions::sse_ext::{sse_extension, SseState};
use crate::js_runtime::extensions::stealth_ext::{stealth_extension, StealthState};
use crate::js_runtime::extensions::timer_ext::{timer_extension, TimerState};
use crate::js_runtime::extensions::webgl_ext::{webgl_extension, WebGLState};
use crate::js_runtime::extensions::websocket_ext::{websocket_extension, WebSocketState};
use crate::js_runtime::extensions::worker_ext::worker_extension;
use crate::js_runtime::state::DomState;
use deno_core::{JsRuntimeForSnapshot, RuntimeOptions};
use std::path::PathBuf;
use std::sync::OnceLock;

static RUNTIME_SNAPSHOT: OnceLock<Box<[u8]>> = OnceLock::new();

/// Path to the on-disk snapshot cache, keyed by the **current executable's**
/// size + mtime so it auto-invalidates whenever the binary is rebuilt (any JS
/// bootstrap change is `include_str!`d, forcing a recompile → a new exe →
/// a new key). This is the multi-process analog of a compile-time
/// `include_bytes!`: the gate spawns a fresh process per site, so without a
/// cache each of the 126 processes *rebuilds* the 1.5 s snapshot from scratch.
/// With it, the first process builds (1.5 s) and every sibling *restores*
/// (~50-100 ms). V8 snapshots are position-independent and tied to the V8
/// build, so a blob produced by one process restores safely in any sibling
/// built from the same binary — exactly the include_bytes! contract.
///
/// Returns `None` (→ in-memory build, no caching) when
/// `BROWSER_OXIDE_NO_SNAPSHOT_CACHE` is set or the exe metadata is
/// unavailable. Override the cache directory with
/// `BROWSER_OXIDE_SNAPSHOT_CACHE` (defaults to the system temp dir).
fn snapshot_cache_path() -> Option<PathBuf> {
    if std::env::var_os("BROWSER_OXIDE_NO_SNAPSHOT_CACHE").is_some() {
        return None;
    }
    let exe = std::env::current_exe().ok()?;
    let meta = std::fs::metadata(&exe).ok()?;
    let len = meta.len();
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::var_os("BROWSER_OXIDE_SNAPSHOT_CACHE")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    Some(dir.join(format!("bo_v8_snapshot_{len}_{mtime}.bin")))
}

/// Get or create the V8 snapshot. Within a process the result is cached in a
/// `OnceLock`; across sibling processes it is cached on disk (see
/// `snapshot_cache_path`) so per-site gate processes and production worker
/// pools pay the ~1.5 s build cost once instead of every spawn.
pub fn get_snapshot() -> &'static [u8] {
    RUNTIME_SNAPSHOT.get_or_init(|| {
        let cache_path = snapshot_cache_path();

        // Fast path: restore a previously-built blob from the disk cache.
        // The cache file only ever appears via an atomic rename (below), so a
        // concurrent build in a sibling process can never expose a partial read.
        if let Some(ref path) = cache_path {
            if let Ok(bytes) = std::fs::read(path) {
                if !bytes.is_empty() {
                    tracing::info!(
                        "Restored V8 snapshot from cache ({} bytes): {}",
                        bytes.len(),
                        path.display()
                    );
                    return bytes.into_boxed_slice();
                }
            }
        }

        tracing::info!("Creating cold V8 snapshot");

        let mut runtime = JsRuntimeForSnapshot::new(RuntimeOptions {
            extensions: vec![
                console_extension::init(),
                crypto_extension::init(),
                dom_extension::init(),
                timer_extension::init(),
                stealth_extension::init(),
                fetch_extension::init(),
                canvas_extension::init(),
                layout_extension::init(),
                websocket_extension::init(),
                webgl_extension::init(),
                sse_extension::init(),
                crate::js_runtime::extensions::input_ext::input_extension::init(),
                worker_extension::init(),
                audio_extension::init(),
                // Must match runtime.rs's extension list EXACTLY (same set + order)
                // — V8 bakes op external-references into the snapshot, and a
                // restore with extra/missing ops segfaults on deno_core 0.403.
                crate::js_runtime::extensions::perf_ext::perf_extension::init(),
                crate::js_runtime::extensions::nav_ext::nav_extension::init(),
            ],
            // Match runtime.rs's heap config so the snapshot deserializes into an
            // identically-configured V8 heap (candidate fix for the V8-149
            // restore segfault).
            create_params: Some(
                deno_core::v8::CreateParams::default()
                    .heap_limits(1024 * 1024 * 1024, 4 * 1024 * 1024 * 1024),
            ),
            ..Default::default()
        });

        // Insert states into OpState for bootstrap execution
        {
            let op_state = runtime.op_state();
            let mut op_state = op_state.borrow_mut();
            op_state.put(DomState::new(Dom::new()));
            op_state.put(TimerState::new());
            // Build the snapshot with `is_secure_context = true` so the
            // bootstrap registers ALL secure-context-only APIs (getBattery,
            // caches, cookieStore, IdleDetector, EyeDropper, WebTransport).
            // Per-page gating runs lazily — navigator getters call _secure()
            // at access time, and cleanup_bootstrap.js strips the rest on
            // insecure pages. Phase 7.
            op_state.put(StealthState::new_with_flags(None, false, true));
            op_state.put(FetchState::new(None));
            op_state.put(CanvasState::new());
            op_state.put(WebSocketState::new());
            op_state.put(WebGLState::new());
            op_state.put(SseState::new());
            op_state.put(crate::js_runtime::extensions::perf_ext::PerfState::new());
            op_state.put(crate::js_runtime::extensions::nav_ext::NavSignal::new());
        }

        // Execute bootstrap JS
        const BOOTSTRAP_JS: &str = concat!(
            include_str!("js/console_bootstrap.js"),
            "\n",
            include_str!("js/stealth_bootstrap.js"),
            "\n",
            include_str!("js/interfaces_bootstrap.js"),
            "\n",
            include_str!("js/instances_bootstrap.js"),
            "\n",
            include_str!("js/fetch_bootstrap.js"),
            "\n",
            include_str!("js/timer_bootstrap.js"),
            "\n",
            include_str!("js/dom_bootstrap.js"),
            "\n",
            include_str!("js/event_bootstrap.js"),
            "\n",
            include_str!("js/canvas_bootstrap.js"),
            "\n",
            include_str!("js/window_bootstrap.js"),
            "\n",
            include_str!("js/streams_bootstrap.js"),
            "\n",
            include_str!("js/structured_clone.js"),
        );

        // Script name "<anonymous>" matches V8's eval-default so
        // Error.stack frames from inside the bootstrap don't leak
        // `<bootstrap>` (a non-Chrome filename) — real Chrome's stacks
        // don't reference internal bootstrap scripts.
        runtime
            .execute_script("<anonymous>", BOOTSTRAP_JS)
            .expect("snapshot bootstrap failed");

        let snapshot = runtime.snapshot();

        // Persist for sibling processes. Write to a per-PID temp file then
        // atomically rename into place, so concurrent builders (parallel gate)
        // never expose a half-written blob and last-writer-wins is harmless
        // (all blobs from the same binary are equivalent).
        if let Some(ref path) = cache_path {
            let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
            if std::fs::write(&tmp, &snapshot).is_ok() {
                if let Err(e) = std::fs::rename(&tmp, path) {
                    tracing::debug!("snapshot cache rename failed: {e}");
                    let _ = std::fs::remove_file(&tmp);
                } else {
                    tracing::info!("Cached V8 snapshot to {}", path.display());
                }
            }
        }

        snapshot
    })
}
