use crate::extensions::audio_ext::audio_extension;
use crate::extensions::canvas_ext::{canvas_extension, CanvasState};
use crate::extensions::console_ext::console_extension;
use crate::extensions::crypto_ext::crypto_extension;
use crate::extensions::dom_ext::dom_extension;
use crate::extensions::fetch_ext::{fetch_extension, FetchState};
use crate::extensions::input_ext::input_extension;
use crate::extensions::layout_ext::layout_extension;
use crate::extensions::sse_ext::{sse_extension, SseState};
use crate::extensions::stealth_ext::{stealth_extension, StealthState};
use crate::extensions::timer_ext::{timer_extension, TimerState};
use crate::extensions::webgl_ext::{webgl_extension, WebGLState};
use crate::extensions::websocket_ext::{websocket_extension, WebSocketState};
use crate::extensions::worker_ext::worker_extension;
use crate::state::DomState;
use deno_core::{JsRuntime, RuntimeOptions};
use dom::Dom;
use stealth::StealthProfile;

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
}

impl Default for BrowserRuntimeOptions {
    fn default() -> Self {
        Self {
            base_url: None,
            stealth_profile: None,
            stylesheets: Vec::new(),
            init_scripts: Vec::new(),
        }
    }
}

/// Create a deno_core JsRuntime configured with browser extensions.
pub fn create_runtime(dom: Dom, options: BrowserRuntimeOptions) -> JsRuntime {
    let mut state = DomState::new(dom);
    state.stylesheets = options.stylesheets;
    if let Some(url) = options.base_url {
        state = state.with_base_url(url);
    }

    // Create fetch client from stealth profile if available
    let fetch_state = match &options.stealth_profile {
        Some(profile) => {
            crate::extensions::fetch_ext::init_fetch_client(profile);
            FetchState::with_profile(profile)
        }
        None => FetchState::new(None),
    };

    let stealth_state = StealthState::new(options.stealth_profile);

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
        ],
        ..Default::default()
    });

    // Insert states into OpState
    runtime.op_state().borrow_mut().put(state);
    runtime.op_state().borrow_mut().put(TimerState::new());
    runtime.op_state().borrow_mut().put(stealth_state);
    runtime.op_state().borrow_mut().put(fetch_state);
    runtime.op_state().borrow_mut().put(CanvasState::new());
    runtime.op_state().borrow_mut().put(WebSocketState::new());
    runtime.op_state().borrow_mut().put(WebGLState::new());
    runtime.op_state().borrow_mut().put(SseState::new());

    // Execute bootstrap JS (static strings)
    runtime
        .execute_script(
            "<console_bootstrap>",
            include_str!("js/console_bootstrap.js"),
        )
        .expect("console bootstrap failed");

    runtime
        .execute_script("<interfaces_bootstrap>", include_str!("js/interfaces_bootstrap.js"))
        .expect("interfaces bootstrap failed");

    runtime
        .execute_script("<instances_bootstrap>", include_str!("js/instances_bootstrap.js"))
        .expect("instances bootstrap failed");

    runtime
        .execute_script("<fetch_bootstrap>", include_str!("js/fetch_bootstrap.js"))
        .expect("fetch bootstrap failed");

    runtime
        .execute_script("<timer_bootstrap>", include_str!("js/timer_bootstrap.js"))
        .expect("timer bootstrap failed");

    runtime
        .execute_script("<dom_bootstrap>", include_str!("js/dom_bootstrap.js"))
        .expect("dom bootstrap failed");

    runtime
        .execute_script("<event_bootstrap>", include_str!("js/event_bootstrap.js"))
        .expect("event bootstrap failed");

    runtime
        .execute_script("<canvas_bootstrap>", include_str!("js/canvas_bootstrap.js"))
        .expect("canvas bootstrap failed");

    runtime
        .execute_script("<window_bootstrap>", include_str!("js/window_bootstrap.js"))
        .expect("window bootstrap failed");


    // Streams (ReadableStream/WritableStream/TransformStream) —
    // installs real implementations over the minimal stubs from
    // window_bootstrap. Must come AFTER window_bootstrap (which
    // defines the stubs) so this script replaces them with
    // `_browserOxideReal = true` versions.
    runtime
        .execute_script("<streams_bootstrap>", include_str!("js/streams_bootstrap.js"))
        .expect("streams bootstrap failed");

    // structuredClone must run after window_bootstrap (which installs
    // Blob and DOMException) but can come after the other feature
    // bootstraps — it only depends on DOMException and the global
    // class names for the DataCloneError path.
    runtime
        .execute_script(
            "<structured_clone>",
            include_str!("js/structured_clone.js"),
        )
        .expect("structured_clone bootstrap failed");

    // Run caller-provided init scripts after all built-in bootstraps.
    // These run in order before any <script> tags parsed from HTML.
    // Errors are logged but do not abort runtime construction so a bad
    // init script cannot brick the page.
    for (i, code) in options.init_scripts.iter().enumerate() {
        let name: &'static str = Box::leak(format!("<init_script_{i}>").into_boxed_str());
        if let Err(e) = runtime.execute_script(name, code.clone()) {
            eprintln!("init script {i} failed: {e}");
        }
    }

    // Final cleanup — hides Deno and internal globals from user JS.
    // Must run LAST, after all built-in bootstraps and init_scripts.
    runtime
        .execute_script("<cleanup_bootstrap>", include_str!("js/cleanup_bootstrap.js"))
        .expect("cleanup bootstrap failed");

    runtime
}

/// Create a minimal JsRuntime suitable for a Web Worker.
///
/// Workers do not get DOM, layout, SSE, WebSocket, or input APIs. They
/// DO get canvas (for `OffscreenCanvas`, which sites probe inside
/// workers per the WHATWG spec), console, crypto, timers, fetch,
/// and the worker-side ops.
pub fn create_worker_runtime() -> JsRuntime {
    let mut runtime = JsRuntime::new(RuntimeOptions {
        extensions: vec![
            console_extension::init_ops(),
            crypto_extension::init_ops(),
            timer_extension::init_ops(),
            fetch_extension::init_ops(),
            worker_extension::init_ops(),
            canvas_extension::init_ops(),
        ],
        ..Default::default()
    });

    // Populate minimum states required by the enabled extensions.
    runtime.op_state().borrow_mut().put(TimerState::new());
    runtime
        .op_state()
        .borrow_mut()
        .put(FetchState::new(None));
    runtime.op_state().borrow_mut().put(CanvasState::new());

    runtime
        .execute_script(
            "<console_bootstrap>",
            include_str!("js/console_bootstrap.js"),
        )
        .expect("worker: console bootstrap failed");

    runtime
        .execute_script("<timer_bootstrap>", include_str!("js/timer_bootstrap.js"))
        .expect("worker: timer bootstrap failed");

    runtime
        .execute_script("<fetch_bootstrap>", include_str!("js/fetch_bootstrap.js"))
        .expect("worker: fetch bootstrap failed");

    runtime
        .execute_script(
            "<worker_bootstrap>",
            include_str!("js/worker_bootstrap.js"),
        )
        .expect("worker: worker bootstrap failed");

    // structuredClone is useful inside workers too — worker code that
    // uses `postMessage` with complex values relies on it, and the
    // impl is self-contained (it gracefully handles the absence of
    // DOMException / Blob via typeof checks).
    runtime
        .execute_script(
            "<structured_clone>",
            include_str!("js/structured_clone.js"),
        )
        .expect("worker: structured_clone bootstrap failed");

    // canvas_bootstrap installs CanvasRenderingContext2D and the real
    // OffscreenCanvas backed by canvas_ext ops. Safe in workers
    // because its DOM-patch blocks all gate on `globalThis.document?`
    // / `globalThis.Element?` which are undefined in the worker scope.
    runtime
        .execute_script(
            "<canvas_bootstrap>",
            include_str!("js/canvas_bootstrap.js"),
        )
        .expect("worker: canvas bootstrap failed");

    // Final cleanup in worker
    runtime
        .execute_script("<cleanup_bootstrap>", include_str!("js/cleanup_bootstrap.js"))
        .expect("worker: cleanup bootstrap failed");

    runtime
}
