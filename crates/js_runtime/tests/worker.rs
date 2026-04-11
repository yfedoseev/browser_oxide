//! Smoke tests for the real Web Worker implementation.
//!
//! Run: cargo test -p js_runtime --test worker -- --test-threads=1 --nocapture

use js_runtime::BrowserJsRuntime;
use std::time::Duration;

fn drive_runtime(code: &str, wait_ms: u64) -> String {
    let dom =
        html_parser::parse_html("<html><head></head><body><div id=\"out\"></div></body></html>");
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = tokio::task::LocalSet::new();
    local.block_on(&rt, async move {
        let mut runtime = BrowserJsRuntime::new(dom);
        runtime.execute_script(code).unwrap();
        // Drive the event loop with a bounded timeout, allowing setInterval
        // polling (Worker uses 5 ms poll) time to deliver the reply.
        let deadline = std::time::Instant::now() + Duration::from_millis(wait_ms);
        loop {
            if std::time::Instant::now() >= deadline {
                break;
            }
            let remaining = deadline - std::time::Instant::now();
            let tick = remaining.min(Duration::from_millis(50));
            let fut = Box::pin(runtime.run_event_loop());
            let _ = tokio::time::timeout(tick, fut).await;
            // Check if we got an answer yet.
            if let Ok(val) =
                runtime.execute_script("document.querySelector('#out').textContent || ''")
            {
                if !val.is_empty() {
                    return val;
                }
            }
        }
        runtime
            .execute_script("document.querySelector('#out').textContent || ''")
            .unwrap_or_default()
    })
}

#[test]
fn worker_echo_round_trip() {
    let code = r#"
        const src = `
            self.onmessage = function(e) {
                self.postMessage('echo:' + e.data);
            };
        `;
        const blob = new Blob([src], { type: 'text/javascript' });
        const url = URL.createObjectURL(blob);
        const w = new Worker(url);
        w.onmessage = function(e) {
            document.querySelector('#out').textContent = e.data;
            w.terminate();
        };
        setTimeout(() => w.postMessage('hello'), 20);
    "#;
    let out = drive_runtime(code, 2000);
    assert_eq!(out, "echo:hello", "worker should echo 'echo:hello'");
}

#[test]
fn worker_addeventlistener_roundtrip() {
    let code = r#"
        const src = `
            self.addEventListener('message', function(e) {
                self.postMessage({ type: 'reply', n: e.data.n + 1 });
            });
        `;
        const blob = new Blob([src], { type: 'text/javascript' });
        const url = URL.createObjectURL(blob);
        const w = new Worker(url);
        w.addEventListener('message', function(e) {
            document.querySelector('#out').textContent = JSON.stringify(e.data);
            w.terminate();
        });
        setTimeout(() => w.postMessage({ n: 41 }), 20);
    "#;
    let out = drive_runtime(code, 2000);
    assert_eq!(out, "{\"type\":\"reply\",\"n\":42}");
}
