use std::sync::OnceLock;
use deno_core::{RuntimeOptions, JsRuntimeForSnapshot};
use dom::Dom;
use crate::extensions::audio_ext::audio_extension;
use crate::extensions::canvas_ext::{canvas_extension, CanvasState};
use crate::extensions::console_ext::console_extension;
use crate::extensions::crypto_ext::crypto_extension;
use crate::extensions::dom_ext::dom_extension;
use crate::extensions::fetch_ext::{fetch_extension, FetchState};
use crate::extensions::layout_ext::layout_extension;
use crate::extensions::sse_ext::{sse_extension, SseState};
use crate::extensions::stealth_ext::{stealth_extension, StealthState};
use crate::extensions::timer_ext::{timer_extension, TimerState};
use crate::extensions::webgl_ext::{webgl_extension, WebGLState};
use crate::extensions::websocket_ext::{websocket_extension, WebSocketState};
use crate::extensions::worker_ext::worker_extension;
use crate::state::DomState;

static RUNTIME_SNAPSHOT: OnceLock<Box<[u8]>> = OnceLock::new();

/// Get or create the cached V8 snapshot.
pub fn get_snapshot() -> &'static [u8] {
    RUNTIME_SNAPSHOT.get_or_init(|| {
        tracing::info!("Creating cold V8 snapshot");
        
        let mut runtime = JsRuntimeForSnapshot::new(RuntimeOptions {
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
                crate::extensions::input_ext::input_extension::init_ops(),
                worker_extension::init_ops(),
                audio_extension::init_ops(),
            ],
            ..Default::default()
        });

        // Insert states into OpState for bootstrap execution
        {
            let op_state = runtime.op_state();
            let mut op_state = op_state.borrow_mut();
            op_state.put(DomState::new(Dom::new()));
            op_state.put(TimerState::new());
            op_state.put(StealthState::new(None));
            op_state.put(FetchState::new(None));
            op_state.put(CanvasState::new());
            op_state.put(WebSocketState::new());
            op_state.put(WebGLState::new());
            op_state.put(SseState::new());
        }

        // Execute bootstrap JS
        const BOOTSTRAP_JS: &str = concat!(
            include_str!("js/console_bootstrap.js"), "\n",
            include_str!("js/stealth_bootstrap.js"), "\n",
            include_str!("js/interfaces_bootstrap.js"), "\n",
            include_str!("js/instances_bootstrap.js"), "\n",
            include_str!("js/fetch_bootstrap.js"), "\n",
            include_str!("js/timer_bootstrap.js"), "\n",
            include_str!("js/dom_bootstrap.js"), "\n",
            include_str!("js/event_bootstrap.js"), "\n",
            include_str!("js/canvas_bootstrap.js"), "\n",
            include_str!("js/window_bootstrap.js"), "\n",
            include_str!("js/streams_bootstrap.js"), "\n",
            include_str!("js/structured_clone.js"),
        );

        runtime
            .execute_script("<bootstrap>", BOOTSTRAP_JS)
            .expect("snapshot bootstrap failed");

        runtime.snapshot()
    })
}
