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
use deno_core::{v8, JsRuntime, RuntimeOptions, SharedArrayBufferStore};
use dom::Dom;
use stealth::StealthProfile;

use std::collections::HashMap;

/// Options for creating a BrowserJsRuntime.
#[derive(Default)]
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
    /// workers — see `crates/net/src/headers.rs::is_cross_origin_isolated`.
    /// Default false (most pages are not COI).
    pub cross_origin_isolated: bool,
    /// Whether the document URL is a secure context per WICG/secure-contexts
    /// (https/wss/file or http://localhost). Drives `self.isSecureContext`
    /// and gates the ~18 secure-context-only Web Platform APIs (mediaDevices,
    /// serviceWorker, clipboard, credentials, usb, etc.) per the IDL
    /// `[SecureContext]` extended attribute. Phase 7 fix. Default false —
    /// callers (e.g. Page::from_html_with_url) classify the URL scheme.
    pub is_secure_context: bool,
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
    // Per-Page worker-ownership tracker — every `new Worker(...)` push
    // its id here so `Page::drop` can reap orphans (see
    // `extensions::worker_ext::drain_owned_workers`).
    runtime
        .op_state()
        .borrow_mut()
        .put(crate::extensions::worker_ext::WorkerOwnership::default());

    // Capture the GENUINE `Function.prototype.toString` before any
    // bootstrap replaces it (doc 27). Untagged functions delegate to
    // this so real-JS source / real-native `[native code]` stay exactly
    // V8-correct; tagged host fns get the synthetic native string from
    // the API-function callback (which V8 itself renders `[native code]`
    // in class-extends/NoSideEffectsToString — closing the source leak).
    let orig_fp_tostring: Option<deno_core::v8::Global<deno_core::v8::Function>> = {
        let scope = &mut runtime.handle_scope();
        crate::native_fns::capture_original_fp_tostring(scope)
    };

    // IframeRealmStore: holds genuine child v8::Context instances (one per
    // iframe) so `iframe.contentWindow` returns a real realm instead of a
    // Proxy — defeats Kasada's `addContentWindowProxy` detector (doc 26/27).
    // Store orig_fp_tostring so op_create_child_realm can install the same
    // genuine-native toString into every child context (cross-realm parity).
    {
        let mut realm_store = crate::native_fns::IframeRealmStore::new();
        if let Some(ref orig) = orig_fp_tostring {
            // Clone the Global (separate handle, same V8 heap object).
            let scope = &mut runtime.handle_scope();
            let local = v8::Local::new(scope, orig);
            realm_store.orig_fp_tostring = Some(v8::Global::new(scope, local));
        }
        runtime.op_state().borrow_mut().put(realm_store);
    }

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
        );

        runtime
            .execute_script("<anonymous>", BOOTSTRAP_JS)
            .expect("bootstrap failed");
    }

    // All bootstrap scripts run with name "<anonymous>" so V8 stack
    // frames don't leak browser_oxide-specific tags. Castle.io
    // documented Kasada/DataDome inspecting Error.stack literal format.
    // Always run cleanup to hide internals, even when restoring from snapshot.
    runtime
        .execute_script("<anonymous>", include_str!("js/cleanup_bootstrap.js"))
        .expect("cleanup failed");

    // Capture Symbol.for('__browser_oxide_native__') from the JS global registry
    // AFTER bootstrap runs (stealth_bootstrap.js creates it at startup).
    // This is the CORRECT symbol: v8::Symbol::for_global uses V8's API
    // registry (Symbol::ForApi), which is a DIFFERENT table from the JS
    // global registry (Symbol::For). Tags set via Symbol.for() in JS are
    // invisible to for_global lookups — so we must capture the symbol
    // from JS and pass it into the native FP.toString callback via Array data.
    let native_tag_sym: Option<v8::Global<v8::Symbol>> = {
        let scope = &mut runtime.handle_scope();
        let src = v8::String::new(scope, "Symbol.for('__browser_oxide_native__')");
        src.and_then(|s| {
            let script = v8::Script::compile(scope, s, None)?;
            let val = script.run(scope)?;
            let sym = v8::Local::<v8::Symbol>::try_from(val).ok()?;
            Some(v8::Global::new(scope, sym))
        })
    };

    // Store the symbol in IframeRealmStore so op_create_child_realm can
    // pass it to install_native_fp_tostring for child realm contexts.
    // Two separate blocks avoid double-borrowing `runtime`: the scope borrow
    // must be dropped before the op_state borrow can be taken.
    if let Some(ref sym) = native_tag_sym {
        let sym_clone = {
            let scope = &mut runtime.handle_scope();
            let local = v8::Local::new(scope, sym);
            v8::Global::new(scope, local)
        };
        runtime
            .op_state()
            .borrow_mut()
            .borrow_mut::<crate::native_fns::IframeRealmStore>()
            .native_tag_sym = Some(sym_clone);
    }

    // Install the genuine-native `Function.prototype.toString` (raw
    // v8::FunctionTemplate API function) AFTER all bootstrap/cleanup,
    // replacing the JS-level patch. Closes the structurally-JS-
    // unpatchable [[SourceText]] leak (class-extends TypeError /
    // NoSideEffectsToString / error stacks / eval) — Kasada `fsc`
    // probe. Behaviour preserved via the captured genuine original +
    // the `Symbol.for('__browser_oxide_native__')` tag scheme. doc 27.
    if let Some(ref orig) = orig_fp_tostring {
        let scope = &mut runtime.handle_scope();
        crate::native_fns::install_native_fp_tostring(scope, orig, native_tag_sym.as_ref());
    }

    // Run caller-provided init scripts after built-in cleanup.
    // These run in order before any <script> tags parsed from HTML.
    //
    // Script name is `<anonymous>` (V8's eval-default tag) to avoid
    // leaking browser_oxide identifiers in Error.stack frames if a
    // site script overrides Error.prepareStackTrace and bypasses our
    // filter. A prior VM trace previously captured
    // `at h (<init_script_0>:51:34)` — anti-bot probes literally saw
    // the index. Both index and the `init_script` tag are now scrubbed.
    for code in options.init_scripts.iter() {
        if let Err(e) = runtime.execute_script("<anonymous>", code.clone()) {
            tracing::warn!(error = %e, "init script failed");
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
    // PerfState is required by perf_extension's ops. Without it,
    // worker code that calls `performance.now()` or similar panics
    // inside gotham_state with "required type ... is not present".
    runtime.op_state().borrow_mut().put(PerfState::default());
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

    // W2.7 — every worker bootstrap script runs with name "<anonymous>"
    // (V8's eval-default) so Error.stack frames don't leak our internal
    // tags to Kasada/DataDome.
    //
    // stealth_bootstrap must run first: installs Function.prototype.toString
    // patch and the _nativeTag/_maskFunction/_maskAsNative helpers that
    // worker_bootstrap uses.
    runtime
        .execute_script("<anonymous>", include_str!("js/stealth_bootstrap.js"))
        .expect("worker: stealth bootstrap failed");

    runtime
        .execute_script("<anonymous>", include_str!("js/console_bootstrap.js"))
        .expect("worker: console bootstrap failed");

    runtime
        .execute_script("<anonymous>", include_str!("js/interfaces_bootstrap.js"))
        .expect("worker: interfaces bootstrap failed");

    runtime
        .execute_script("<anonymous>", include_str!("js/shared_apis_bootstrap.js"))
        .expect("worker: shared_apis bootstrap failed");

    runtime
        .execute_script("<anonymous>", include_str!("js/timer_bootstrap.js"))
        .expect("worker: timer bootstrap failed");

    runtime
        .execute_script("<anonymous>", include_str!("js/fetch_bootstrap.js"))
        .expect("worker: fetch bootstrap failed");

    runtime
        .execute_script("<anonymous>", include_str!("js/streams_bootstrap.js"))
        .expect("worker: streams bootstrap failed");

    // event_bootstrap defines Event, MessageEvent, EventTarget, and wires
    // addEventListener / removeEventListener / dispatchEvent onto
    // globalThis. The worker realm needs these because
    // worker_bootstrap.js's parent→worker message pump constructs
    // `new MessageEvent(...)` and dispatches via `self.dispatchEvent(...)`
    // — without event_bootstrap, both throw and the setInterval pump
    // halts after the first incoming message, silently dropping all
    // parent→worker traffic. (Caught by
    // `crates/js_runtime/tests/worker.rs::worker_echo_round_trip`.)
    runtime
        .execute_script("<anonymous>", include_str!("js/event_bootstrap.js"))
        .expect("worker: event bootstrap failed");

    // structuredClone is useful inside workers too — worker code that
    // uses `postMessage` with complex values relies on it, and the
    // impl is self-contained (it gracefully handles the absence of
    // DOMException / Blob via typeof checks).
    runtime
        .execute_script("<anonymous>", include_str!("js/structured_clone.js"))
        .expect("worker: structured_clone bootstrap failed");

    runtime
        .execute_script("<anonymous>", include_str!("js/worker_bootstrap.js"))
        .expect("worker: worker bootstrap failed");

    // canvas_bootstrap installs CanvasRenderingContext2D and the real
    // OffscreenCanvas backed by canvas_ext ops. Safe in workers
    // because its DOM-patch blocks all gate on `globalThis.document?`
    // / `globalThis.Element?` which are undefined in the worker scope.
    runtime
        .execute_script("<anonymous>", include_str!("js/canvas_bootstrap.js"))
        .expect("worker: canvas bootstrap failed");

    // Final cleanup in worker
    runtime
        .execute_script("<anonymous>", include_str!("js/cleanup_bootstrap.js"))
        .expect("worker: cleanup bootstrap failed");

    runtime
}
