//! WebSocket CDP server — accepts connections and dispatches to CdpSession.
//!
//! `Page` is `!Send` (V8 internals use `Rc`), so the server runs on a dedicated
//! thread with a single-threaded tokio runtime. CDP clients connect via WebSocket
//! from any thread.

use crate::session::{json_escape_string, CdpSession};
use crate::types::*;
use futures_util::{SinkExt, StreamExt};
use std::cell::RefCell;
use std::rc::Rc;
use stealth;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

/// A running CDP server. Stops when dropped.
pub struct CdpServer {
    port: u16,
    shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl CdpServer {
    /// Start a CDP WebSocket server on `127.0.0.1:{port}`.
    ///
    /// Spawns a dedicated thread with a single-threaded tokio runtime (required
    /// because `Page` is `!Send`). The HTML is used to create a fresh Page on
    /// that thread.
    pub fn start(html: &str, port: u16) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let html = html.to_string();
        let shutdown = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let shutdown_clone = shutdown.clone();
        let (port_tx, port_rx) = std::sync::mpsc::channel();

        let thread = std::thread::spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    tracing::error!("CdpServer: failed to build tokio runtime: {}", e);
                    return;
                }
            };
            let local = tokio::task::LocalSet::new();

            local.block_on(&rt, async move {
                let page =
                    match browser::Page::from_html(&html, None::<stealth::StealthProfile>).await {
                        Ok(p) => p,
                        Err(e) => {
                            tracing::error!("CdpServer: failed to create page: {}", e);
                            port_tx.send(0).ok();
                            return;
                        }
                    };
                let page = Rc::new(RefCell::new(page));

                let listener = match TcpListener::bind(format!("127.0.0.1:{}", port)).await {
                    Ok(l) => l,
                    Err(e) => {
                        tracing::error!("CdpServer: failed to bind CDP port: {}", e);
                        port_tx.send(0).ok();
                        return;
                    }
                };
                let actual_port = listener.local_addr().unwrap().port();
                port_tx.send(actual_port).ok();

                accept_loop(listener, page, None, shutdown_clone).await;
            });
        });

        let actual_port = port_rx.recv().map_err(|e| {
            Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("server thread failed to start: {}", e),
            )) as Box<dyn std::error::Error + Send + Sync>
        })?;

        if actual_port == 0 {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "server initialization failed",
            )));
        }

        Ok(Self {
            port: actual_port,
            shutdown,
            thread: Some(thread),
        })
    }

    /// Start with a URL and stealth profile (fetches the page on the server thread).
    /// The HTTP client is kept alive for subsequent Page.navigate calls.
    pub fn start_with_url(
        url: &str,
        port: u16,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let url = url.to_string();
        let shutdown = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let shutdown_clone = shutdown.clone();
        let (port_tx, port_rx) = std::sync::mpsc::channel();

        let thread = std::thread::spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    tracing::error!("CdpServer: failed to build tokio runtime: {}", e);
                    return;
                }
            };
            let local = tokio::task::LocalSet::new();

            local.block_on(&rt, async move {
                let profile = stealth::chrome_130_linux();
                // Test raw HTTP first to diagnose connection issues
                let client = match net::HttpClient::new(&profile) {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::error!("CdpServer: failed to create HTTP client: {}", e);
                        port_tx.send(0).ok();
                        return;
                    }
                };
                let page = match client.get(&url).await {
                    Ok(resp) => {
                        let html = resp.text();
                        match browser::Page::with_profile(&html, &url, profile).await {
                            Ok(p) => p,
                            Err(e) => {
                                tracing::error!(
                                    "CdpServer: failed to create page for {}: {}",
                                    url,
                                    e
                                );
                                port_tx.send(0).ok();
                                return;
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("CdpServer: HTTP request failed for {}: {}", url, e);
                        port_tx.send(0).ok();
                        return;
                    }
                };
                let page = Rc::new(RefCell::new(page));
                let http_client = Some(Rc::new(client));

                let listener = match TcpListener::bind(format!("127.0.0.1:{}", port)).await {
                    Ok(l) => l,
                    Err(e) => {
                        tracing::error!("CdpServer: failed to bind CDP port: {}", e);
                        port_tx.send(0).ok();
                        return;
                    }
                };
                let actual_port = listener.local_addr().unwrap().port();
                port_tx.send(actual_port).ok();

                accept_loop(listener, page, http_client, shutdown_clone).await;
            });
        });

        let actual_port = port_rx.recv().map_err(|e| {
            Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("server thread failed to start: {}", e),
            )) as Box<dyn std::error::Error + Send + Sync>
        })?;

        // Port 0 signals that page creation/navigation failed
        if actual_port == 0 {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "failed to create page or navigate",
            )));
        }

        Ok(Self {
            port: actual_port,
            shutdown,
            thread: Some(thread),
        })
    }

    /// Start with an empty page and an HTTP client ready for Page.navigate.
    ///
    /// Unlike `start_with_url`, this doesn't fetch any URL upfront.
    /// The client sends `Page.navigate` via CDP to load pages, and the same
    /// server instance can navigate to multiple URLs without restarting.
    pub fn start_navigable(port: u16) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let shutdown = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let shutdown_clone = shutdown.clone();
        let (port_tx, port_rx) = std::sync::mpsc::channel();

        let thread = std::thread::spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    tracing::error!("CdpServer: failed to build tokio runtime: {}", e);
                    return;
                }
            };
            let local = tokio::task::LocalSet::new();

            local.block_on(&rt, async move {
                let profile = stealth::chrome_130_linux();
                let client = match net::HttpClient::new(&profile) {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::error!("CdpServer: failed to create HTTP client: {}", e);
                        port_tx.send(0).ok();
                        return;
                    }
                };

                let page = match browser::Page::from_html(
                    "<html><body></body></html>",
                    None::<stealth::StealthProfile>,
                )
                .await
                {
                    Ok(p) => p,
                    Err(e) => {
                        tracing::error!("CdpServer: failed to create empty page: {}", e);
                        port_tx.send(0).ok();
                        return;
                    }
                };
                let page = Rc::new(RefCell::new(page));
                let http_client = Some(Rc::new(client));

                let listener = match TcpListener::bind(format!("127.0.0.1:{}", port)).await {
                    Ok(l) => l,
                    Err(e) => {
                        tracing::error!("CdpServer: failed to bind CDP port: {}", e);
                        port_tx.send(0).ok();
                        return;
                    }
                };
                let actual_port = listener.local_addr().unwrap().port();
                port_tx.send(actual_port).ok();

                accept_loop(listener, page, http_client, shutdown_clone).await;
            });
        });

        let actual_port = port_rx.recv().map_err(|e| {
            Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("server thread failed to start: {}", e),
            )) as Box<dyn std::error::Error + Send + Sync>
        })?;

        if actual_port == 0 {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "failed to create HTTP client",
            )));
        }

        Ok(Self {
            port: actual_port,
            shutdown,
            thread: Some(thread),
        })
    }

    /// Start on port 0 (OS-assigned) — useful for tests.
    pub fn start_ephemeral(html: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::start(html, 0)
    }

    /// The port the server is listening on.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// WebSocket URL for CDP clients.
    pub fn ws_url(&self) -> String {
        format!("ws://127.0.0.1:{}", self.port)
    }
}

impl Drop for CdpServer {
    fn drop(&mut self) {
        self.shutdown
            .store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

async fn accept_loop(
    listener: TcpListener,
    page: Rc<RefCell<browser::Page>>,
    http_client: Option<Rc<net::HttpClient>>,
    shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    loop {
        if shutdown.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }

        let accept =
            tokio::time::timeout(std::time::Duration::from_millis(100), listener.accept()).await;

        match accept {
            Ok(Ok((stream, addr))) => {
                let page = page.clone();
                let client = http_client.clone();
                tokio::task::spawn_local(async move {
                    if let Err(e) = handle_connection(stream, page, client).await {
                        tracing::warn!("CDP connection from {} error: {}", addr, e);
                    }
                });
            }
            Ok(Err(e)) => {
                tracing::warn!("CDP accept error: {}", e);
            }
            Err(_) => {
                // Timeout — check shutdown flag again
            }
        }
    }
}

async fn handle_connection(
    stream: tokio::net::TcpStream,
    page: Rc<RefCell<browser::Page>>,
    http_client: Option<Rc<net::HttpClient>>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Peek enough bytes to check for WebSocket upgrade header
    let mut buf = [0u8; 512];
    let n = stream.peek(&mut buf).await?;
    let peek = String::from_utf8_lossy(&buf[..n]);

    // Plain HTTP requests (no Upgrade header) go to the JSON discovery endpoints
    if peek.starts_with("GET ") && !peek.contains("Upgrade:") && !peek.contains("upgrade:") {
        return handle_http(stream, &page).await;
    }

    // WebSocket upgrade — avoid split() overhead since CDP is request-response
    let mut ws_stream = tokio_tungstenite::accept_async(stream).await?;
    let mut session = CdpSession::new();

    loop {
        let msg = match ws_stream.next().await {
            Some(Ok(msg)) => msg,
            Some(Err(e)) => return Err(e.into()),
            None => break,
        };
        match msg {
            Message::Text(text) => {
                // Fast path for Runtime.evaluate — avoid full JSON parse + async overhead
                if let Some(resp) = fast_evaluate(&text, &page) {
                    ws_stream.send(Message::Text(resp.into())).await?;
                    continue;
                }

                let req: CdpRequest = match serde_json::from_str(&text) {
                    Ok(r) => r,
                    Err(e) => {
                        let err_msg = format!(
                            r#"{{"error":{{"code":-32700,"message":"Parse error: {}"}}}}"#,
                            e.to_string().replace('"', "'")
                        );
                        ws_stream.send(Message::Text(err_msg.into())).await?;
                        continue;
                    }
                };

                let (response, events) = {
                    let mut page_ref = page.borrow_mut();
                    let client_ref = http_client.as_deref();
                    session
                        .handle_request(&mut *page_ref, &req, client_ref)
                        .await
                };

                // Handle pending navigation (Page.navigate).
                // Uses reload_html to swap DOM in the existing V8 isolate —
                // avoids the 17ms cost of creating a new isolate.
                if let Some(url) = session.pending_navigate.take() {
                    if let Some(client) = http_client.as_deref() {
                        if let Ok(resp) = client.get(&url).await {
                            let html = resp.text();
                            let mut borrow = page.borrow_mut();
                            borrow.reload_html(&html, &url);
                            // Re-inject scripts registered via addScriptToEvaluateOnNewDocument
                            for script in &session.scripts_on_new_document {
                                let _ = borrow.evaluate(script);
                            }
                        }
                    }
                }

                // Send events first (matches Chrome behavior)
                for event in events {
                    ws_stream.send(Message::Text(to_json(&event).into())).await?;
                }
                // Then send the response
                ws_stream.send(Message::Text(response.into())).await?;
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    Ok(())
}

/// Fast path for Runtime.evaluate — extracts id+expression with string scanning
/// instead of full JSON parse, evaluates directly, formats response manually.
/// Returns None if the message isn't a simple Runtime.evaluate.
fn fast_evaluate(text: &str, page: &Rc<RefCell<browser::Page>>) -> Option<String> {
    // Quick check: must contain Runtime.evaluate
    if !text.contains("Runtime.evaluate") {
        return None;
    }
    // Must not have returnByValue or awaitPromise (use slow path for those)
    if text.contains("returnByValue") || text.contains("awaitPromise") {
        return None;
    }

    // Extract "id" field
    let id_start = text.find("\"id\"")?;
    let colon = text[id_start + 4..].find(':')?;
    let after_colon = &text[id_start + 4 + colon + 1..];
    let id_str = after_colon.trim_start();
    let id_end = id_str.find(|c: char| !c.is_ascii_digit())?;
    let id: u64 = id_str[..id_end].parse().ok()?;

    // Extract "expression" field value
    let expr_key = text.find("\"expression\"")?;
    let colon2 = text[expr_key + 12..].find(':')?;
    let after_colon2 = &text[expr_key + 12 + colon2 + 1..];
    let after_trim = after_colon2.trim_start();
    if !after_trim.starts_with('"') {
        return None;
    }
    // Parse the JSON string value (handles escapes)
    let expression: String =
        serde_json::from_str(&after_trim[..find_json_string_end(after_trim)?]).ok()?;

    // Evaluate
    let mut page_ref = page.borrow_mut();
    match page_ref.evaluate(&expression) {
        Ok(result_str) => {
            let js_t = match result_str.as_str() {
                "undefined" => "undefined",
                "null" => "object",
                "true" | "false" => "boolean",
                s if s.parse::<f64>().is_ok() => "number",
                _ => "string",
            };
            let escaped = json_escape_string(&result_str);
            Some(format!(
                r#"{{"id":{},"result":{{"type":"{}","value":{}}}}}"#,
                id, js_t, escaped
            ))
        }
        Err(e) => {
            let msg = e.to_string().replace('"', "'");
            Some(format!(
                r#"{{"id":{},"result":{{"exceptionDetails":{{"text":"{}","lineNumber":0,"columnNumber":0}}}}}}"#,
                id, msg
            ))
        }
    }
}

/// Find the end of a JSON string literal (including the closing quote).
fn find_json_string_end(s: &str) -> Option<usize> {
    if !s.starts_with('"') {
        return None;
    }
    let mut i = 1;
    let bytes = s.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            i += 2; // skip escaped char
        } else if bytes[i] == b'"' {
            return Some(i + 1);
        } else {
            i += 1;
        }
    }
    None
}

async fn handle_http(
    stream: tokio::net::TcpStream,
    page: &Rc<RefCell<browser::Page>>,
) -> Result<(), Box<dyn std::error::Error>> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut stream = stream;
    let mut buf = vec![0u8; 4096];
    let n = stream.read(&mut buf).await?;
    let request = String::from_utf8_lossy(&buf[..n]);
    let path = request
        .lines()
        .next()
        .unwrap_or("")
        .split_whitespace()
        .nth(1)
        .unwrap_or("/");

    let addr = stream.local_addr()?;
    let ws_url = format!("ws://127.0.0.1:{}", addr.port());

    let body = match path {
        "/json/version" => serde_json::json!({
            "Browser": "browser_oxide/0.1.0",
            "Protocol-Version": "1.3",
            "User-Agent": "browser_oxide/0.1.0",
            "V8-Version": "12.x",
            "WebKit-Version": "0",
            "webSocketDebuggerUrl": ws_url,
        })
        .to_string(),
        "/json" | "/json/list" => {
            let (title, url) = {
                let mut page_ref = page.borrow_mut();
                let title = page_ref.title();
                let url = page_ref.url().to_string();
                (title, url)
            };
            serde_json::json!([{
                "description": "",
                "devtoolsFrontendUrl": "",
                "id": "page-1",
                "title": title,
                "type": "page",
                "url": url,
                "webSocketDebuggerUrl": ws_url,
            }])
            .to_string()
        }
        _ => {
            let resp = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
            stream.write_all(resp.as_bytes()).await?;
            return Ok(());
        }
    };

    let resp = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream.write_all(resp.as_bytes()).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_starts_and_stops() {
        let server = CdpServer::start_ephemeral("<html><body>Hello</body></html>").unwrap();
        assert!(server.port() > 0);
        assert!(server.ws_url().contains("127.0.0.1"));
        drop(server);
    }

    #[test]
    fn bench_page_creation_overhead() {
        // Measure pure V8 isolate + DOM creation time (no network)
        let rt = tokio::runtime::Runtime::new().unwrap();
        let html = "<html><head><title>Test</title></head><body><p>Hello</p></body></html>";

        // Warm up
        rt.block_on(async {
            let _ = browser::Page::from_html(html, None::<stealth::StealthProfile>).await;
        });

        let mut times = Vec::new();
        for _ in 0..10 {
            rt.block_on(async {
                let start = std::time::Instant::now();
                let page = browser::Page::from_html_fast(
                    html,
                    "https://example.com",
                    stealth::presets::chrome_130_ru(),
                )
                .await
                .unwrap();
                times.push(start.elapsed());
                drop(page);
            });
        }

        for (i, t) in times.iter().enumerate() {
            println!("  page {}: {:?}", i, t);
        }
        let avg = times.iter().map(|t| t.as_micros()).sum::<u128>() / times.len() as u128;
        println!("  avg: {}µs ({}ms)", avg, avg / 1000);
    }

    #[test]
    #[ignore] // requires network
    fn navigate_via_cdp_multi() {
        // Measure time for multiple navigations to same page
        let server = CdpServer::start_navigable(0).unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let (ws, _) = tokio_tungstenite::connect_async(&server.ws_url()).await.unwrap();
            let (mut tx, mut rx) = ws.split();
            use futures_util::{SinkExt, StreamExt};

            tx.send(Message::Text(r#"{"id":1,"method":"Page.enable","params":{}}"#.into())).await.unwrap();
            let _ = rx.next().await;
            tx.send(Message::Text(r#"{"id":2,"method":"Runtime.enable","params":{}}"#.into())).await.unwrap();
            let _ = rx.next().await;

            let urls = [
                "https://example.com",
                "https://httpbin.org/html",
                "https://news.ycombinator.com",
                "https://httpbin.org/get",
                "https://example.com",
            ];

            let total_start = std::time::Instant::now();
            for (i, url) in urls.iter().enumerate() {
                let start = std::time::Instant::now();
                let id = (i * 2 + 3) as u64;
                tx.send(Message::Text(format!(
                    r#"{{"id":{},"method":"Page.navigate","params":{{"url":"{}"}}}}"#,
                    id, url
                ).into())).await.unwrap();
                // Read until response
                loop {
                    let msg = rx.next().await.unwrap().unwrap();
                    if let Message::Text(t) = &msg {
                        let v: serde_json::Value = serde_json::from_str(t).unwrap();
                        if v.get("id").and_then(|i| i.as_u64()) == Some(id) { break; }
                    }
                }
                // Eval title
                let eval_id = id + 1;
                tx.send(Message::Text(format!(
                    r#"{{"id":{},"method":"Runtime.evaluate","params":{{"expression":"document.title"}}}}"#,
                    eval_id
                ).into())).await.unwrap();
                let msg = rx.next().await.unwrap().unwrap();
                let elapsed = start.elapsed();
                if let Message::Text(t) = msg {
                    let v: serde_json::Value = serde_json::from_str(&t).unwrap();
                    let title = v["result"]["value"].as_str().unwrap_or("?");
                    println!("  {} => {:?} ({:?})", url, title, elapsed);
                }
            }
            println!("  TOTAL: {:?}, avg {:?}", total_start.elapsed(), total_start.elapsed() / urls.len() as u32);

            tx.send(Message::Close(None)).await.ok();
        });
        drop(server);
    }

    #[test]
    #[ignore] // requires network
    fn navigate_via_cdp() {
        // Test that Page.navigate actually loads a new page
        let server = CdpServer::start_navigable(0).unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let (ws, _) = tokio_tungstenite::connect_async(&server.ws_url())
                .await
                .unwrap();
            let (mut tx, mut rx) = ws.split();
            use futures_util::{SinkExt, StreamExt};

            // Enable Page
            tx.send(Message::Text(
                r#"{"id":1,"method":"Page.enable","params":{}}"#.into(),
            ))
            .await
            .unwrap();
            let _ = rx.next().await; // response

            // Enable Runtime
            tx.send(Message::Text(
                r#"{"id":2,"method":"Runtime.enable","params":{}}"#.into(),
            ))
            .await
            .unwrap();
            let _ = rx.next().await; // response

            // Navigate to example.com
            tx.send(Message::Text(
                r#"{"id":3,"method":"Page.navigate","params":{"url":"https://example.com"}}"#
                    .into(),
            ))
            .await
            .unwrap();

            // Read all messages until we get id:3 response
            loop {
                let msg = rx.next().await.unwrap().unwrap();
                if let Message::Text(t) = &msg {
                    println!("  recv: {}", &t[..t.len().min(200)]);
                    let v: serde_json::Value = serde_json::from_str(t).unwrap();
                    if v.get("id").and_then(|i| i.as_u64()) == Some(3) {
                        break;
                    }
                }
            }

            // Evaluate document.title
            tx.send(Message::Text(
                r#"{"id":4,"method":"Runtime.evaluate","params":{"expression":"document.title"}}"#
                    .into(),
            ))
            .await
            .unwrap();
            let msg = rx.next().await.unwrap().unwrap();
            if let Message::Text(t) = msg {
                println!("  title response: {}", t);
                assert!(
                    t.contains("Example Domain"),
                    "Expected 'Example Domain' in: {}",
                    t
                );
            }

            tx.send(Message::Close(None)).await.ok();
        });
        drop(server);
    }

    #[test]
    #[ignore] // requires network
    fn server_start_with_url_example_com() {
        match CdpServer::start_with_url("https://example.com", 0) {
            Ok(server) => {
                println!("OK: port={}", server.port());
                drop(server);
            }
            Err(e) => {
                println!("ERR: {}", e);
                panic!("start_with_url failed: {}", e);
            }
        }
    }
}
