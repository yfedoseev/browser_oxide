use crate::extensions::audio_ext::audio_extension;
use crate::extensions::canvas_ext::{canvas_extension, CanvasState};
use crate::extensions::console_ext::console_extension;
use crate::extensions::crypto_ext::crypto_extension;
use crate::extensions::dom_ext::dom_extension;
use crate::extensions::fetch_ext::{fetch_extension, FetchState};
use crate::extensions::input_ext::input_extension;
use crate::extensions::layout_ext::layout_extension;
use crate::extensions::nav_ext::{nav_extension, NavSignal};
use crate::extensions::perf_ext::{perf_extension, PerfState};
use crate::extensions::sse_ext::{sse_extension, SseState};
use crate::extensions::stealth_ext::{stealth_extension, StealthState};
use crate::extensions::timer_ext::{timer_extension, TimerState};
use crate::extensions::webgl_ext::{webgl_extension, WebGLState};
use crate::extensions::websocket_ext::{websocket_extension, WebSocketState};
use crate::extensions::worker_ext::worker_extension;
use crate::state::DomState;
use deno_core::{JsRuntime, RuntimeOptions, SharedArrayBufferStore};
use dom::Dom;
use stealth::StealthProfile;

use std::collections::HashMap;

/// Options for creating a BrowserJsRuntime.
pub struct BrowserRuntimeOptions {
    pub base_url: Option<url::Url>,
    pub stealth_profile: Option<StealthProfile>,
    pub stylesheets: Vec<String>,
    /// Scripts evaluated AFTER all built-in bootstraps but BEFORE any
    /// parsed-HTML `<script>` tags. Mirrors Chromium's
    /// `Page.addScriptToEvaluateOnNewDocument` CDP command — the driver
    /// uses this to carry fingerprint/capability extensions across
    /// navigations within a frame without baking them into the runtime.
    pub init_scripts: Vec<String>,
    /// Persistent storage (localStorage / sessionStorage) carried across navigations.
    pub storage: Option<HashMap<String, HashMap<String, String>>>,
    /// Optional V8 snapshot to speed up startup.
    pub startup_snapshot: Option<&'static [u8]>,
    /// Whether the document satisfies cross-origin isolation requirements
    /// (COOP=same-origin AND COEP=require-corp|credentialless). Drives
    /// `self.crossOriginIsolated` and gates SAB postMessage transfer to
    /// workers — see `crates/net/src/headers.rs::is_cross_origin_isolated`
    /// and gap #30 in docs/GAPS.md. Default false (most pages are not COI).
    pub cross_origin_isolated: bool,
    /// Whether the document URL is a secure context per WICG/secure-contexts
    /// (https/wss/file or http://localhost). Drives `self.isSecureContext`
    /// and gates the ~18 secure-context-only Web Platform APIs (mediaDevices,
    /// serviceWorker, clipboard, credentials, usb, etc.) per the IDL
    /// `[SecureContext]` extended attribute. Phase 7 fix. Default false —
    /// callers (e.g. Page::from_html_with_url) classify the URL scheme.
    pub is_secure_context: bool,
}

impl Default for BrowserRuntimeOptions {
    fn default() -> Self {
        Self {
            base_url: None,
            stealth_profile: None,
            stylesheets: Vec::new(),
            init_scripts: Vec::new(),
            storage: None,
            startup_snapshot: None,
            cross_origin_isolated: false,
            is_secure_context: false,
        }
    }
}

/// Create a deno_core JsRuntime configured with browser extensions.
///
/// **Backward-compatibility shim.** Prefer [`create_runtime_with_signals`]
/// in new code — it also returns the per-runtime [`NavSignal`] so the
/// event loop can short-circuit when JS sets `__pendingNavigation` (gap
/// in 5-second Kasada retry window). Keeps this fn for existing callers
/// that don't need the signal.
pub fn create_runtime(dom: Dom, options: BrowserRuntimeOptions) -> JsRuntime {
    create_runtime_with_signals(dom, options).0
}

/// Create a runtime AND return its NavSignal so the event-loop driver
/// can poll `nav.pending()` between ticks and break out of `run_until_idle`
/// the moment JS triggers a navigation. See `nav_ext.rs`.
pub fn create_runtime_with_signals(
    dom: Dom,
    options: BrowserRuntimeOptions,
) -> (JsRuntime, NavSignal) {
    let mut state = DomState::new(dom);
    state.stylesheets = options.stylesheets;
    if let Some(storage) = options.storage {
        state.storage = storage;
    }
    if let Some(url) = options.base_url {
        state = state.with_base_url(url);
    }
    state.update_cached_rules();

    // Create fetch client from stealth profile if available
    let fetch_state = match &options.stealth_profile {
        Some(profile) => {
            crate::extensions::fetch_ext::init_fetch_client(profile);
            FetchState::with_profile(profile)
        }
        None => FetchState::new(None),
    };

    let stealth_state = StealthState::new_with_flags(
        options.stealth_profile,
        options.cross_origin_isolated,
        options.is_secure_context,
    );

    // Match Chrome 147's renderer heap budget. V8's default ~1.5 GB OOMs
    // on probe sites that build very large fingerprint payloads (creepjs
    // hits `Builtins_ArrayPrototypePush` OOM at ~1.8 GB on macOS arm64
    // — the engine is collecting hundreds of thousands of property
    // descriptors across every WebIDL interface). Real Chrome on a
    // desktop has 4 GB+ available per renderer; we mirror that.
    //
    // HEAP_INITIAL was 256 MB but caused early-growth GC pauses on
    // fingerprint-heavy sites (creepjs allocates well past 256 MB during
    // its lie-detection pass; V8 spent time compacting old space before
    // growing the heap). 1 GB initial skips those early compactions.
    const HEAP_INITIAL: usize = 1024 * 1024 * 1024; // 1 GB initial
    const HEAP_MAX: usize = 4 * 1024 * 1024 * 1024; // 4 GB max
    let create_params = deno_core::v8::CreateParams::default().heap_limits(HEAP_INITIAL, HEAP_MAX);

    let mut runtime = JsRuntime::new(RuntimeOptions {
        extensions: vec![
            console_extension::init_ops(),
            crypto_extension::init_ops(),
            dom_extension::init_ops(),
            timer_extension::init_ops(),
            stealth_extension::init_ops(),
            fetch_extension::init_ops(),
            canvas_extension::init_ops(),
            layout_extension::init_ops(),
            websocket_extension::init_ops(),
            webgl_extension::init_ops(),
            sse_extension::init_ops(),
            input_extension::init_ops(),
            worker_extension::init_ops(),
            audio_extension::init_ops(),
            perf_extension::init_ops(),
            nav_extension::init_ops(),
        ],
        startup_snapshot: options.startup_snapshot,
        create_params: Some(create_params),
        // Enables postMessage transfer of SharedArrayBuffer between isolates.
        // The SAB *constructor* is always exposed by V8; we gate transfer
        // separately on `cross_origin_isolated` (gap #30).
        shared_array_buffer_store: Some(SharedArrayBufferStore::default()),
        ..Default::default()
    });

    // Per-runtime NavSignal — populated by JS via op_set_pending_nav,
    // consumed by BrowserEventLoop to short-circuit run_until_idle.
    let nav_signal = NavSignal::new();

    // Insert states into OpState
    runtime.op_state().borrow_mut().put(state);
    runtime.op_state().borrow_mut().put(TimerState::new());
    runtime.op_state().borrow_mut().put(PerfState::new());
    runtime.op_state().borrow_mut().put(nav_signal.clone());
    runtime.op_state().borrow_mut().put(stealth_state);
    runtime.op_state().borrow_mut().put(fetch_state);
    runtime.op_state().borrow_mut().put(CanvasState::new());
    runtime.op_state().borrow_mut().put(WebSocketState::new());
    runtime.op_state().borrow_mut().put(WebGLState::new());
    runtime.op_state().borrow_mut().put(SseState::new());

    // Execute bootstrap JS only if NOT starting from snapshot
    if options.startup_snapshot.is_none() {
        const BOOTSTRAP_JS: &str = concat!(
            include_str!("js/console_bootstrap.js"),
            "\n",
            include_str!("js/stealth_bootstrap.js"),
            "\n",
            include_str!("js/interfaces_bootstrap.js"),
            "\n",
            include_str!("js/shared_apis_bootstrap.js"),
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
            "\n",
            include_str!("js/cleanup_bootstrap.js"),
        );

        runtime
            .execute_script("<bootstrap>", BOOTSTRAP_JS)
            .expect("bootstrap failed");
    }

    // Always run cleanup to hide internals, even when restoring from snapshot
    runtime
        .execute_script("<cleanup>", include_str!("js/cleanup_bootstrap.js"))
        .expect("cleanup failed");

    // Run caller-provided init scripts after built-in cleanup.
    // These run in order before any <script> tags parsed from HTML.
    for (i, code) in options.init_scripts.iter().enumerate() {
        let name: &'static str = Box::leak(format!("<init_script_{i}>").into_boxed_str());
        if let Err(e) = runtime.execute_script(name, code.clone()) {
            tracing::warn!(script_index = i, error = %e, "init script failed");
        }
    }

    (runtime, nav_signal)
}

/// Create a minimal JsRuntime suitable for a Web Worker.
///
/// Workers do not get DOM, layout, SSE, WebSocket, or input APIs. They
/// DO get canvas (for `OffscreenCanvas`, which sites probe inside
/// workers per the WHATWG spec), console, crypto, timers, fetch,
/// and the worker-side ops.
pub fn create_worker_runtime(profile: Option<StealthProfile>) -> JsRuntime {
    let mut runtime = JsRuntime::new(RuntimeOptions {
        extensions: vec![
            console_extension::init_ops(),
            crypto_extension::init_ops(),
            timer_extension::init_ops(),
            fetch_extension::init_ops(),
            worker_extension::init_ops(),
            canvas_extension::init_ops(),
            stealth_extension::init_ops(),
            perf_extension::init_ops(),
        ],
        ..Default::default()
    });

    // Populate minimum states required by the enabled extensions.
    runtime.op_state().borrow_mut().put(TimerState::new());
    runtime.op_state().borrow_mut().put(FetchState::new(None));
    runtime.op_state().borrow_mut().put(CanvasState::new());
    // Inject DomState even in workers (stubbed) to hold the stealth profile
    // so op_has_stealth_profile() works in the worker isolate.
    let mut dom_state = DomState::new(dom::Dom::new());
    dom_state.stealth_profile = profile.clone();
    runtime.op_state().borrow_mut().put(dom_state);

    // StealthState must also carry the profile so op_get_profile_value
    // returns the correct values inside the worker context.
    runtime
        .op_state()
        .borrow_mut()
        .put(StealthState::new(profile));

    // stealth_bootstrap must run first: installs Function.prototype.toString patch
    // and the _nativeTag/_maskFunction/_maskAsNative helpers that worker_bootstrap uses.
    runtime
        .execute_script(
            "<stealth_bootstrap>",
            include_str!("js/stealth_bootstrap.js"),
        )
        .expect("worker: stealth bootstrap failed");

    runtime
        .execute_script(
            "<console_bootstrap>",
            include_str!("js/console_bootstrap.js"),
        )
        .expect("worker: console bootstrap failed");

    runtime
        .execute_script(
            "<interfaces_bootstrap>",
            include_str!("js/interfaces_bootstrap.js"),
        )
        .expect("worker: interfaces bootstrap failed");

    runtime
        .execute_script(
            "<shared_apis_bootstrap>",
            include_str!("js/shared_apis_bootstrap.js"),
        )
        .expect("worker: shared_apis bootstrap failed");

    runtime
        .execute_script("<timer_bootstrap>", include_str!("js/timer_bootstrap.js"))
        .expect("worker: timer bootstrap failed");

    runtime
        .execute_script("<fetch_bootstrap>", include_str!("js/fetch_bootstrap.js"))
        .expect("worker: fetch bootstrap failed");

    runtime
        .execute_script("<streams_bootstrap>", include_str!("js/streams_bootstrap.js"))
        .expect("worker: streams bootstrap failed");

    // structuredClone is useful inside workers too — worker code that
    // uses `postMessage` with complex values relies on it, and the
    // impl is self-contained (it gracefully handles the absence of
    // DOMException / Blob via typeof checks).
    runtime
        .execute_script("<structured_clone>", include_str!("js/structured_clone.js"))
        .expect("worker: structured_clone bootstrap failed");

    runtime
        .execute_script("<worker_bootstrap>", include_str!("js/worker_bootstrap.js"))
        .expect("worker: worker bootstrap failed");

    // canvas_bootstrap installs CanvasRenderingContext2D and the real
    // OffscreenCanvas backed by canvas_ext ops. Safe in workers
    // because its DOM-patch blocks all gate on `globalThis.document?`
    // / `globalThis.Element?` which are undefined in the worker scope.
    runtime
        .execute_script("<canvas_bootstrap>", include_str!("js/canvas_bootstrap.js"))
        .expect("worker: canvas bootstrap failed");

    // Final cleanup in worker
    runtime
        .execute_script(
            "<cleanup_bootstrap>",
            include_str!("js/cleanup_bootstrap.js"),
        )
        .expect("worker: cleanup bootstrap failed");

    runtime
}
