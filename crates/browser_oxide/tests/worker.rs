//! Smoke tests for the real Web Worker implementation.
//!
//! Run: cargo test -p js_runtime --test worker -- --test-threads=1 --nocapture

use browser_oxide::js_runtime::BrowserJsRuntime;
use std::time::Duration;

fn drive_runtime(code: &str, wait_ms: u64) -> String {
    let dom = browser_oxide::html_parser::parse_html(
        "<html><head></head><body><div id=\"out\"></div></body></html>",
    );
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = tokio::task::LocalSet::new();
    local.block_on(&rt, async move {
        let mut runtime = BrowserJsRuntime::new(dom);
        runtime.execute_script(code, None).unwrap();
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
                runtime.execute_script("document.querySelector('#out').textContent || ''", None)
            {
                if !val.is_empty() {
                    return val;
                }
            }
        }
        runtime
            .execute_script("document.querySelector('#out').textContent || ''", None)
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

/// `self.location` must be populated from the URL the
/// worker was constructed with. Recaptcha enterprise's webworker reads
/// `self.location.origin` to verify it was loaded from a trusted
/// recaptcha.net URL; an undefined/missing location bails the token flow.
///
/// Uses a blob: URL so the worker source is deterministic across runs;
/// the `URL.createObjectURL` registers a real `blob:` scheme URL, which
/// `op_worker_self_url` echoes back, and `new URL(blob:…)` parses it
/// into origin/protocol/etc.
#[test]
fn worker_self_location_populated_from_construction_url() {
    let code = r#"
        const src = `
            self.onmessage = function(e) {
                self.postMessage(JSON.stringify({
                    has_location: typeof self.location === 'object' && self.location !== null,
                    href: self.location && self.location.href,
                    protocol: self.location && self.location.protocol,
                    origin: self.location && self.location.origin,
                    toString_works: self.location && (self.location + '') === self.location.href,
                }));
            };
        `;
        const blob = new Blob([src], { type: 'text/javascript' });
        const url = URL.createObjectURL(blob);
        const w = new Worker(url);
        w.onmessage = function(e) {
            document.querySelector('#out').textContent = e.data;
            w.terminate();
        };
        setTimeout(() => w.postMessage('go'), 20);
    "#;
    let out = drive_runtime(code, 2000);
    let v: serde_json::Value =
        serde_json::from_str(&out).unwrap_or_else(|e| panic!("invalid JSON: {e}; raw={out}"));
    assert_eq!(v["has_location"], true, "self.location must exist: {out}");
    assert!(
        v["href"].as_str().unwrap_or("").starts_with("blob:"),
        "href must echo the blob: URL: {out}"
    );
    // Load-bearing for recaptcha-class probes: location.toString() === href.
    assert_eq!(
        v["toString_works"], true,
        "location toString must equal href: {out}"
    );
    // vNext/10 URL polyfill blob: fix — real Chrome on a blob:null/uuid URL
    // returns `.protocol === "blob:"` and `.origin === "null"`. Pre-fix
    // the polyfill emitted "" for protocol; post-fix it matches Chrome.
    assert_eq!(
        v["protocol"], "blob:",
        "blob: URL must report protocol=\"blob:\": {out}"
    );
    assert_eq!(
        v["origin"], "null",
        "blob: URL must report origin=\"null\": {out}"
    );
}
