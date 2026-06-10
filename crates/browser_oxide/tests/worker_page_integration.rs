//! Verify that Workers are functional in the full Page bootstrap (not just
//! the bare BrowserJsRuntime). Catches interference from later bootstrap
//! scripts that might overwrite globalThis.Worker, URL.createObjectURL, etc.
//!
//! Run: cargo test -p browser --test worker_page_integration -- --test-threads=1 --nocapture

use std::time::Duration;

#[tokio::test]
async fn worker_works_in_page_bootstrap() {
    use browser_oxide::event_loop::BrowserEventLoop;
    use browser_oxide::js_runtime::BrowserJsRuntime;

    let dom = browser_oxide::html_parser::parse_html(
        "<html><head></head><body><div id=\"out\"></div></body></html>",
    );
    let mut evloop = BrowserEventLoop::new(BrowserJsRuntime::new(dom));

    // 1) Capability probes: verify the real Worker and blob URL functions exist.
    evloop
        .execute_script(
            r#"window.__probe = JSON.stringify({
                workerType: typeof Worker,
                createUrlType: typeof URL.createObjectURL,
                blobType: typeof Blob,
                workerProto: (Worker && Worker.prototype && Worker.prototype[Symbol.toStringTag]) || null,
            });"#,
        )
        .unwrap();
    let probe = evloop.execute_script("window.__probe").unwrap();
    println!("[probe] {probe}");
    assert!(
        probe.contains("\"workerType\":\"function\""),
        "Worker should be a function, got {probe}"
    );
    assert!(
        probe.contains("\"createUrlType\":\"function\""),
        "URL.createObjectURL should be a function, got {probe}"
    );

    // 2) Functional test: spawn a worker, send a message, expect a reply.
    evloop
        .execute_script(
            r#"
            (function(){
                const src = `
                    self.onmessage = function(e) {
                        self.postMessage('page:' + e.data);
                    };
                `;
                const blob = new Blob([src], { type: 'text/javascript' });
                const url = URL.createObjectURL(blob);
                const w = new Worker(url);
                w.onmessage = function(e) {
                    document.querySelector('#out').textContent = e.data;
                    w.terminate();
                };
                setTimeout(() => w.postMessage('ping'), 20);
            })();
            "#,
        )
        .unwrap();

    evloop
        .run_until_idle(Duration::from_millis(2000))
        .await
        .unwrap();

    let out = evloop
        .execute_script("document.querySelector('#out').textContent")
        .unwrap();
    assert_eq!(
        out, "page:ping",
        "worker should echo in full page bootstrap"
    );
}
