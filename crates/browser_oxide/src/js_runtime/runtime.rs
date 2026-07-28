use crate::dom::Dom;
use crate::js_runtime::extensions::audio_ext::audio_extension;
use crate::js_runtime::extensions::canvas_ext::{canvas_extension, CanvasState};
use crate::js_runtime::extensions::console_ext::console_extension;
use crate::js_runtime::extensions::crypto_ext::crypto_extension;
use crate::js_runtime::extensions::dom_ext::dom_extension;
use crate::js_runtime::extensions::fetch_ext::{fetch_extension, FetchState};
use crate::js_runtime::extensions::input_ext::input_extension;
use crate::js_runtime::extensions::layout_ext::layout_extension;
use crate::js_runtime::extensions::nav_ext::{nav_extension, NavSignal};
use crate::js_runtime::extensions::perf_ext::{perf_extension, PerfState};
use crate::js_runtime::extensions::sse_ext::{sse_extension, SseState};
use crate::js_runtime::extensions::stealth_ext::{stealth_extension, StealthState};
use crate::js_runtime::extensions::timer_ext::{timer_extension, TimerState};
use crate::js_runtime::extensions::webgl_ext::{webgl_extension, WebGLState};
use crate::js_runtime::extensions::websocket_ext::{websocket_extension, WebSocketState};
use crate::js_runtime::extensions::worker_ext::worker_extension;
use crate::js_runtime::state::DomState;
use crate::stealth::StealthProfile;
use deno_core::{v8, JsRuntime, RuntimeOptions, SharedArrayBufferStore};

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
/// event loop can short-circuit when JS sets `__pendingNavigation`
/// (some sites expect a navigation to begin within a few seconds).
/// Keeps this fn for existing callers
/// that don't need the signal.
/// Default V8 heap ceiling, in MiB. Overridable via
/// `BROWSER_OXIDE_HEAP_MAX_MB`.
pub const DEFAULT_HEAP_MAX_MB: usize = 4096;

/// Default initial V8 heap reservation, in MiB. Overridable via
/// `BROWSER_OXIDE_HEAP_INITIAL_MB`.
///
/// Not 256 MB: that caused early-growth GC pauses on fingerprint-heavy sites,
/// where a heavy probe allocates well past 256 MB in a single pass and V8 spent
/// time compacting old space before growing the heap. 1 GB skips those early
/// compactions.
pub const DEFAULT_HEAP_INITIAL_MB: usize = 1024;

/// Resolve `(initial, max)` V8 heap limits in bytes.
///
/// Both are environment-tunable, which matters because the right ceiling is a
/// property of the deployment, not of the engine: a 512 MB container and a
/// 64 GB scraping host want very different numbers, and the previous
/// hard-coded 4 GB silently over-committed the former.
///
/// - `BROWSER_OXIDE_HEAP_MAX_MB` — ceiling, default
///   [`DEFAULT_HEAP_MAX_MB`] (4 GB).
/// - `BROWSER_OXIDE_HEAP_INITIAL_MB` — initial reservation, default
///   [`DEFAULT_HEAP_INITIAL_MB`] (1 GB).
///
/// Unparseable or zero values fall back to the defaults rather than failing:
/// a typo in an env var should not take down a scrape. An initial larger than
/// the max is clamped down to the max, since V8 treats that combination as a
/// hard error.
fn heap_limits() -> (usize, usize) {
    fn mb_from_env(key: &str, default_mb: usize) -> usize {
        match std::env::var(key) {
            Ok(raw) => match raw.trim().parse::<usize>() {
                Ok(mb) if mb > 0 => mb,
                _ => {
                    tracing::warn!(
                        env = key,
                        value = %raw,
                        default_mb,
                        "ignoring unparseable/zero heap limit; using default"
                    );
                    default_mb
                }
            },
            Err(_) => default_mb,
        }
    }

    let max_mb = mb_from_env("BROWSER_OXIDE_HEAP_MAX_MB", DEFAULT_HEAP_MAX_MB);
    let initial_mb = mb_from_env("BROWSER_OXIDE_HEAP_INITIAL_MB", DEFAULT_HEAP_INITIAL_MB);
    let initial_mb = initial_mb.min(max_mb);

    const MIB: usize = 1024 * 1024;
    (initial_mb * MIB, max_mb * MIB)
}

/// Guarantee a tokio runtime is entered for the duration of `JsRuntime`
/// construction, falling back to a process-lifetime runtime if the caller has
/// none.
///
/// Required as of `deno_core` 0.408. It captures
/// `tokio::runtime::Handle::try_current()` at isolate-registration time
/// (`runtime/jsruntime.rs`) and spawns V8's *delayed* foreground tasks — GC
/// memory-reducer tasks and friends — on that handle. When the handle is
/// `None`, `runtime/setup.rs::spawn_delayed_task` prints a diagnostic and calls
/// `std::process::abort()`. Upstream aborts rather than panics deliberately:
/// V8 invokes it from C++ frames Rust cannot unwind through.
///
/// Two things worth being precise about, because both misled the 0.1.2
/// investigation (#37):
///
/// 1. **This is not debug-only.** The abort is unconditional. Release builds
///    pass only while V8 happens not to post a delayed task in the window
///    being exercised, which is timing, not safety.
/// 2. **It is not the caller's bug to fix.** `BrowserJsRuntime::new` /
///    `with_profile` / `with_options` are synchronous public API, callable from
///    a plain `fn main` or a `#[test]`. Requiring every embedder to wrap
///    construction in a runtime would be a silent breaking change whose
///    failure mode is a process abort.
///
/// The captured handle must stay valid for the isolate's whole life, so the
/// fallback runtime is a `OnceLock` living to process exit — a temporary would
/// leave the isolate holding a handle to a dropped runtime.
fn ensure_tokio_context() -> Option<tokio::runtime::EnterGuard<'static>> {
    if tokio::runtime::Handle::try_current().is_ok() {
        return None;
    }
    static FALLBACK_RT: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();
    let rt = FALLBACK_RT.get_or_init(|| {
        // Single worker + timer driver is all V8's delayed tasks need: they
        // sleep, push onto the isolate's foreground queue, and wake it. The
        // work itself is drained synchronously by our own event loop.
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_time()
            .thread_name("browser-oxide-v8-delayed")
            .build()
            .expect("failed to build fallback tokio runtime for V8 delayed tasks")
    });
    Some(rt.enter())
}

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

    // P2 — ES-module loader for document `<script type="module">`. Resolves
    // relative specifiers + fetches the import graph through the shared HTTP
    // session (cookie/profile-consistent with the nav). Built before
    // `stealth_profile` is moved into StealthState below. Without it, module
    // SPA bundles throw SyntaxError and are dropped (the thin-render gap).
    let module_loader: Option<std::rc::Rc<dyn deno_core::ModuleLoader>> =
        options.stealth_profile.as_ref().map(|p| {
            std::rc::Rc::new(crate::js_runtime::module_loader::BrowserModuleLoader::new(
                p.clone(),
            )) as std::rc::Rc<dyn deno_core::ModuleLoader>
        });

    // Create fetch client from stealth profile if available
    let fetch_state = match &options.stealth_profile {
        Some(profile) => {
            crate::js_runtime::extensions::fetch_ext::init_fetch_client(profile);
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
    // on sites that build very large fingerprint payloads (a heavy
    // fingerprint probe hits `Builtins_ArrayPrototypePush` OOM at ~1.8 GB
    // on macOS arm64 — the engine is collecting hundreds of thousands of
    // property descriptors across every WebIDL interface). Real Chrome on
    // a desktop has 4 GB+ available per renderer; we mirror that.
    //
    let (heap_initial, heap_max) = heap_limits();
    let create_params = deno_core::v8::CreateParams::default().heap_limits(heap_initial, heap_max);

    // Must outlive the `JsRuntime::new` call below — deno_core captures the
    // current tokio handle during isolate registration. See
    // `ensure_tokio_context`.
    let _tokio_guard = ensure_tokio_context();

    let mut runtime = JsRuntime::new(RuntimeOptions {
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
            input_extension::init(),
            worker_extension::init(),
            audio_extension::init(),
            perf_extension::init(),
            nav_extension::init(),
        ],
        startup_snapshot: options.startup_snapshot,
        create_params: Some(create_params),
        // Enables postMessage transfer of SharedArrayBuffer between isolates.
        // The SAB *constructor* is always exposed by V8; we gate transfer
        // separately on `cross_origin_isolated`.
        shared_array_buffer_store: Some(SharedArrayBufferStore::default()),
        module_loader,
        ..Default::default()
    });

    // Per-runtime NavSignal — populated by JS via op_set_pending_nav,
    // consumed by BrowserEventLoop to short-circuit run_until_idle.
    let nav_signal = NavSignal::new();

    // Insert states into OpState
    runtime.op_state().borrow_mut().put(state);
    runtime.op_state().borrow_mut().put(TimerState::new());
    runtime.op_state().borrow_mut().put(PerfState::new());
    runtime
        .op_state()
        .borrow_mut()
        .put(crate::js_runtime::extensions::input_ext::BehaviorRngState::from_env_or_random());
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
        .put(crate::js_runtime::extensions::worker_ext::WorkerOwnership::default());

    // Capture the GENUINE `Function.prototype.toString` before any
    // bootstrap replaces it. Untagged functions delegate to
    // this so real-JS source / real-native `[native code]` stay exactly
    // V8-correct; tagged host fns get the synthetic native string from
    // the API-function callback (which V8 itself renders `[native code]`
    // in class-extends/NoSideEffectsToString — closing the source leak).
    let orig_fp_tostring: Option<deno_core::v8::Global<deno_core::v8::Function>> = {
        let __ctx = runtime.main_context();
        v8::scope_with_context!(scope, runtime.v8_isolate(), __ctx);
        crate::js_runtime::native_fns::capture_original_fp_tostring(scope)
    };

    // IframeRealmStore: holds genuine child v8::Context instances (one per
    // iframe) so `iframe.contentWindow` returns a real realm instead of a
    // Proxy — matching real Chrome, where contentWindow is a genuine realm.
    // Store orig_fp_tostring so op_create_child_realm can install the same
    // genuine-native toString into every child context (cross-realm parity).
    {
        let mut realm_store = crate::js_runtime::native_fns::IframeRealmStore::new();
        if let Some(ref orig) = orig_fp_tostring {
            // Clone the Global (separate handle, same V8 heap object).
            let __ctx = runtime.main_context();
            v8::scope_with_context!(scope, runtime.v8_isolate(), __ctx);
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
    // frames don't leak browser_oxide-specific tags — real Chrome's
    // Error.stack does not reference internal bootstrap scripts.
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
        let __ctx = runtime.main_context();
        v8::scope_with_context!(scope, runtime.v8_isolate(), __ctx);
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
            let __ctx = runtime.main_context();
            v8::scope_with_context!(scope, runtime.v8_isolate(), __ctx);
            let local = v8::Local::new(scope, sym);
            v8::Global::new(scope, local)
        };
        runtime
            .op_state()
            .borrow_mut()
            .borrow_mut::<crate::js_runtime::native_fns::IframeRealmStore>()
            .native_tag_sym = Some(sym_clone);
    }

    // Install the genuine-native `Function.prototype.toString` (raw
    // v8::FunctionTemplate API function) AFTER all bootstrap/cleanup,
    // replacing the JS-level patch. Closes the structurally-JS-
    // unpatchable [[SourceText]] leak (class-extends TypeError /
    // NoSideEffectsToString / error stacks / eval), which differs from
    // real Chrome. Behaviour preserved via the captured genuine original
    // + the `Symbol.for('__browser_oxide_native__')` tag scheme.
    if let Some(ref orig) = orig_fp_tostring {
        let __ctx = runtime.main_context();
        v8::scope_with_context!(scope, runtime.v8_isolate(), __ctx);
        crate::js_runtime::native_fns::install_native_fp_tostring(
            scope,
            orig,
            native_tag_sym.as_ref(),
        );
    }

    // Run caller-provided init scripts after built-in cleanup.
    // These run in order before any <script> tags parsed from HTML.
    //
    // Script name is `<anonymous>` (V8's eval-default tag) to avoid
    // leaking browser_oxide identifiers in Error.stack frames if a
    // site script overrides Error.prepareStackTrace and bypasses our
    // filter. An earlier trace exposed a frame like
    // `at h (<init_script_0>:51:34)`, leaking the script index — which
    // real Chrome would not show. Both index and the `init_script` tag
    // are now scrubbed.
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
/// DO get canvas (for `OffscreenCanvas`, which sites use inside
/// workers per the WHATWG spec), console, crypto, timers, fetch,
/// and the worker-side ops.
/// `is_secure_context` is inherited from the spawning document: a Worker is a
/// secure context iff its owner is (HTML spec §"secure context"). Without this,
/// the worker realm defaulted to insecure and `cleanup_bootstrap.js` stripped
/// `crypto.subtle` / `crypto.randomUUID` — which silently broke any worker doing
/// SHA-256 proof-of-work (a common pattern in challenge scripts that run in workers).
pub fn create_worker_runtime(
    profile: Option<StealthProfile>,
    is_secure_context: bool,
) -> JsRuntime {
    // Same requirement as the page runtime — worker realms are built on their
    // own threads, which may not have a runtime entered. See
    // `ensure_tokio_context`.
    let _tokio_guard = ensure_tokio_context();

    let mut runtime = JsRuntime::new(RuntimeOptions {
        extensions: vec![
            console_extension::init(),
            crypto_extension::init(),
            timer_extension::init(),
            fetch_extension::init(),
            worker_extension::init(),
            canvas_extension::init(),
            stealth_extension::init(),
            perf_extension::init(),
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
    let mut dom_state = DomState::new(crate::dom::Dom::new());
    dom_state.stealth_profile = profile.clone();
    runtime.op_state().borrow_mut().put(dom_state);

    // StealthState must also carry the profile so op_get_profile_value
    // returns the correct values inside the worker context. is_secure_context
    // is inherited from the parent document so [SecureContext] APIs
    // (crypto.subtle, crypto.randomUUID) survive cleanup_bootstrap in workers
    // spawned from secure (https / blob:https) pages.
    runtime
        .op_state()
        .borrow_mut()
        .put(StealthState::new_with_flags(
            profile,
            false,
            is_secure_context,
        ));

    // Every worker bootstrap script runs with name "<anonymous>"
    // (V8's eval-default) so Error.stack frames don't leak our internal
    // tags — matching real Chrome, whose stacks don't reference them.
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
