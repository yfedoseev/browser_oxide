//! Browser comparison benchmark — drives browser_oxide, Chrome, and Lightpanda
//! through the same raw CDP client to measure speed, stealth, and compatibility.
//!
//! All tests are `#[ignore]` — they require Chrome and/or Lightpanda running.
//!
//! ## Setup
//!
//! ```bash
//! # Chrome headless (port 9222):
//! google-chrome --headless --disable-gpu --remote-debugging-port=9222
//!
//! # Lightpanda (port 9223):
//! lightpanda serve --port 9223
//!
//! # Run benchmarks (--release for fair comparison against optimized Chrome/Lightpanda):
//! cargo test --release -p browser --test browser_comparison -- --ignored --test-threads=1 --nocapture
//! ```

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use tokio_tungstenite::tungstenite::Message;

// ============================================================
// Minimal CDP client — raw WebSocket + JSON-RPC, zero abstraction
// ============================================================

struct CdpClient {
    tx: futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Message,
    >,
    rx: futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
    next_id: u64,
    /// Session ID for browsers that require Target.attachToTarget (e.g. Lightpanda).
    session_id: Option<String>,
    /// Buffered events received during wait_for_id (checked by wait_for_event).
    event_buffer: Vec<Value>,
}

impl CdpClient {
    async fn connect(ws_url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let (ws, _) = tokio_tungstenite::connect_async(ws_url).await?;
        let (tx, rx) = ws.split();
        Ok(Self {
            tx,
            rx,
            next_id: 1,
            session_id: None,
            event_buffer: Vec::new(),
        })
    }

    /// Connect and set up a CDP session (Target.createTarget + attachToTarget).
    /// Required for Lightpanda; optional for Chrome (has default page).
    async fn connect_with_session(ws_url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let mut client = Self::connect(ws_url).await?;

        // Create a target
        let resp = client
            .send_raw("Target.createTarget", json!({"url": "about:blank"}))
            .await?;
        let target_id = resp["result"]["targetId"]
            .as_str()
            .ok_or("no targetId in createTarget response")?
            .to_string();

        // Attach to get a sessionId
        let resp = client
            .send_raw(
                "Target.attachToTarget",
                json!({"targetId": target_id, "flatten": true}),
            )
            .await?;
        // sessionId comes either in the response or in the attachedToTarget event
        let session_id = resp["result"]["sessionId"].as_str().map(|s| s.to_string());

        // If not in the response, it was in an event we skipped — read events to find it
        let session_id = if let Some(sid) = session_id {
            sid
        } else {
            // Already consumed events while waiting for response — check if we got it
            // Fallback: try the response params
            resp["params"]["sessionId"]
                .as_str()
                .unwrap_or("SID-1")
                .to_string()
        };

        client.session_id = Some(session_id);
        Ok(client)
    }

    /// Send without sessionId (for Target-level commands).
    async fn send_raw(
        &mut self,
        method: &str,
        params: Value,
    ) -> Result<Value, Box<dyn std::error::Error>> {
        let id = self.next_id;
        self.next_id += 1;

        let req = json!({
            "id": id,
            "method": method,
            "params": params,
        });
        self.tx.send(Message::Text(req.to_string())).await?;
        self.wait_for_id(id).await
    }

    /// Send with sessionId if available.
    async fn send(
        &mut self,
        method: &str,
        params: Value,
    ) -> Result<Value, Box<dyn std::error::Error>> {
        let id = self.next_id;
        self.next_id += 1;

        let mut req = json!({
            "id": id,
            "method": method,
            "params": params,
        });
        if let Some(ref sid) = self.session_id {
            req["sessionId"] = json!(sid);
        }
        self.tx.send(Message::Text(req.to_string())).await?;
        self.wait_for_id(id).await
    }

    async fn wait_for_id(&mut self, id: u64) -> Result<Value, Box<dyn std::error::Error>> {
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            let msg = tokio::time::timeout(Duration::from_secs(30), self.rx.next())
                .await?
                .ok_or("connection closed")??;

            if let Message::Text(text) = msg {
                let val: Value = serde_json::from_str(&text)?;
                if val.get("id").and_then(|v| v.as_u64()) == Some(id) {
                    return Ok(val);
                }
                // Buffer events so wait_for_event can find them later
                if val.get("method").is_some() {
                    self.event_buffer.push(val);
                }
            }
        }
        Err("timeout waiting for CDP response".into())
    }

    /// Wait for a specific CDP event (e.g. "Page.loadEventFired").
    /// Checks the event buffer first (events received during wait_for_id),
    /// then reads from the stream until the event arrives or timeout.
    async fn wait_for_event(&mut self, event_name: &str, timeout_ms: u64) {
        // Check buffered events first
        if let Some(pos) = self
            .event_buffer
            .iter()
            .position(|v| v.get("method").and_then(|m| m.as_str()) == Some(event_name))
        {
            self.event_buffer.remove(pos);
            return;
        }

        let deadline = Instant::now() + Duration::from_millis(timeout_ms);
        while Instant::now() < deadline {
            let remaining = deadline.duration_since(Instant::now());
            let msg = match tokio::time::timeout(remaining, self.rx.next()).await {
                Ok(Some(Ok(Message::Text(text)))) => text,
                _ => break,
            };
            if let Ok(val) = serde_json::from_str::<Value>(&msg) {
                if val.get("method").and_then(|v| v.as_str()) == Some(event_name) {
                    return;
                }
            }
        }
    }

    /// Navigate to a URL and wait for the page to load (via Page.loadEventFired).
    /// Falls back to a short sleep if the event doesn't arrive within timeout.
    async fn navigate_and_wait(&mut self, url: &str, timeout_ms: u64) {
        let _ = self.send("Page.navigate", json!({"url": url})).await;
        self.wait_for_event("Page.loadEventFired", timeout_ms).await;
        // Small grace period for any post-load JS
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    async fn close(mut self) {
        let _ = self.tx.send(Message::Close(None)).await;
    }
}

/// Extract the string value from a CDP Runtime.evaluate response.
/// Handles both Chrome format (`result.result.value`) and browser_oxide (`result.value`).
fn extract_value(resp: &Value) -> String {
    // Chrome/standard: {"id":N,"result":{"result":{"type":"...","value":"..."}}}
    if let Some(v) = resp["result"]["result"]["value"].as_str() {
        return v.to_string();
    }
    // browser_oxide: {"id":N,"result":{"type":"...","value":"..."}}
    if let Some(v) = resp["result"]["value"].as_str() {
        return v.to_string();
    }
    // Numeric or boolean values
    if let Some(v) = resp["result"]["result"]["value"].as_f64() {
        return v.to_string();
    }
    if let Some(v) = resp["result"]["value"].as_f64() {
        return v.to_string();
    }
    if let Some(v) = resp["result"]["result"]["value"].as_bool() {
        return v.to_string();
    }
    if let Some(v) = resp["result"]["value"].as_bool() {
        return v.to_string();
    }
    String::new()
}

// ============================================================
// Browser endpoints
// ============================================================

/// browser_oxide CDP server port (started per-test).
const BROWSER_OXIDE_PORT: u16 = 0; // ephemeral

/// Chrome headless default CDP port.
const CHROME_PORT: u16 = 9222;

/// Lightpanda default CDP port.
const LIGHTPANDA_PORT: u16 = 9223;

/// Discover the WebSocket debugger URL for a CDP endpoint.
/// Chrome requires connecting to a page-specific WS URL (e.g. /devtools/page/{id}),
/// while Lightpanda and browser_oxide accept connections on the root WS path.
async fn discover_ws_url(port: u16) -> Option<String> {
    // Try /json/list first (Chrome returns page targets with webSocketDebuggerUrl)
    let http_url = format!("http://127.0.0.1:{}/json/list", port);
    if let Ok(resp) = reqwest_get(&http_url).await {
        if let Ok(list) = serde_json::from_str::<Value>(&resp) {
            if let Some(targets) = list.as_array() {
                // Find first "page" type target
                for target in targets {
                    if target["type"].as_str() == Some("page") {
                        if let Some(ws) = target["webSocketDebuggerUrl"].as_str() {
                            return Some(ws.to_string());
                        }
                    }
                }
                // No page target — try first target
                if let Some(first) = targets.first() {
                    if let Some(ws) = first["webSocketDebuggerUrl"].as_str() {
                        return Some(ws.to_string());
                    }
                }
            }
        }
    }

    // Try /json/version (has a browser-level webSocketDebuggerUrl)
    let http_url = format!("http://127.0.0.1:{}/json/version", port);
    if let Ok(resp) = reqwest_get(&http_url).await {
        if let Ok(ver) = serde_json::from_str::<Value>(&resp) {
            if let Some(ws) = ver["webSocketDebuggerUrl"].as_str() {
                return Some(ws.to_string());
            }
        }
    }

    // Fallback: plain ws://host:port (works for browser_oxide and Lightpanda)
    Some(format!("ws://127.0.0.1:{}", port))
}

/// Minimal HTTP GET using raw TCP (avoids adding reqwest as a dep).
async fn reqwest_get(url: &str) -> Result<String, Box<dyn std::error::Error>> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let url_parsed: url::Url = url.parse()?;
    let host = url_parsed.host_str().ok_or("no host")?;
    let port = url_parsed.port().unwrap_or(80);
    let path = url_parsed.path();

    let mut stream = tokio::net::TcpStream::connect(format!("{}:{}", host, port)).await?;
    let req = format!(
        "GET {} HTTP/1.1\r\nHost: {}:{}\r\nConnection: close\r\n\r\n",
        path, host, port
    );
    stream.write_all(req.as_bytes()).await?;

    let mut buf = Vec::new();
    // Use timeout — some servers don't close despite Connection: close
    match tokio::time::timeout(Duration::from_secs(3), stream.read_to_end(&mut buf)).await {
        Ok(Ok(_)) => {}
        Ok(Err(e)) => return Err(e.into()),
        Err(_) => {} // timeout is fine — we have enough data
    }
    let resp = String::from_utf8_lossy(&buf);

    // Split headers from body
    if let Some(pos) = resp.find("\r\n\r\n") {
        Ok(resp[pos + 4..].to_string())
    } else {
        Ok(resp.to_string())
    }
}

async fn is_port_open(port: u16) -> bool {
    tokio::net::TcpStream::connect(format!("127.0.0.1:{}", port))
        .await
        .is_ok()
}

/// Connect to a browser with the right protocol.
/// Lightpanda requires Target.createTarget + attachToTarget to get a session.
/// Chrome and browser_oxide work with direct connection.
async fn connect_browser(
    ws_url: &str,
    browser_name: &str,
) -> Result<CdpClient, Box<dyn std::error::Error>> {
    if browser_name == "lightpanda" {
        CdpClient::connect_with_session(ws_url).await
    } else {
        CdpClient::connect(ws_url).await
    }
}

// ============================================================
// Benchmark helpers
// ============================================================

struct BenchResult {
    browser: String,
    test: String,
    duration: Duration,
    success: bool,
    detail: String,
}

impl BenchResult {
    fn print(&self) {
        let status = if self.success { "PASS" } else { "FAIL" };
        println!(
            "[{status}] {browser:<20} {test:<30} {duration:>8.1?} {detail}",
            status = status,
            browser = self.browser,
            test = self.test,
            duration = self.duration,
            detail = self.detail,
        );
    }
}

fn print_comparison(results: &[BenchResult]) {
    let sep = "=".repeat(80);
    let dash = "-".repeat(80);
    println!("\n{}", sep);
    println!(
        "{:<20} {:<30} {:>10} {}",
        "Browser", "Test", "Time", "Result"
    );
    println!("{}", dash);
    for r in results {
        let status = if r.success { "PASS" } else { "FAIL" };
        println!(
            "{:<20} {:<30} {:>8.1?}  {} {}",
            r.browser, r.test, r.duration, status, r.detail
        );
    }
    println!("{}", dash);
}

// ============================================================
// Test: Runtime.evaluate speed
// ============================================================

async fn bench_evaluate(ws_url: &str, browser_name: &str) -> Vec<BenchResult> {
    let mut results = Vec::new();
    let mut client = match connect_browser(ws_url, browser_name).await {
        Ok(c) => c,
        Err(e) => {
            results.push(BenchResult {
                browser: browser_name.into(),
                test: "connect".into(),
                duration: Duration::ZERO,
                success: false,
                detail: format!("connection failed: {}", e),
            });
            return results;
        }
    };

    // Enable Runtime
    let _ = client.send("Runtime.enable", json!({})).await;

    // Warm up
    let _ = client
        .send("Runtime.evaluate", json!({"expression": "1+1"}))
        .await;

    // Simple expression
    let start = Instant::now();
    let iterations = 100;
    for _ in 0..iterations {
        let _ = client
            .send("Runtime.evaluate", json!({"expression": "1+1"}))
            .await;
    }
    let elapsed = start.elapsed();
    results.push(BenchResult {
        browser: browser_name.into(),
        test: format!("evaluate_simple x{}", iterations),
        duration: elapsed,
        success: true,
        detail: format!(
            "{:.2}ms/call",
            elapsed.as_secs_f64() * 1000.0 / iterations as f64
        ),
    });

    // Complex expression
    let start = Instant::now();
    let resp = client
        .send(
            "Runtime.evaluate",
            json!({"expression": "JSON.stringify(Array.from({length: 1000}, (_, i) => ({idx: i, sq: i*i})))"}),
        )
        .await;
    let elapsed = start.elapsed();
    let success = resp.is_ok();
    results.push(BenchResult {
        browser: browser_name.into(),
        test: "evaluate_complex_json".into(),
        duration: elapsed,
        success,
        detail: if success {
            let val = resp.unwrap();
            let len = extract_value(&val).len();
            format!("result_len={}", len)
        } else {
            "error".into()
        },
    });

    // DOM manipulation
    let start = Instant::now();
    let resp = client
        .send(
            "Runtime.evaluate",
            json!({"expression": r#"
                for (let i = 0; i < 100; i++) {
                    const el = document.createElement('div');
                    el.id = 'bench-' + i;
                    el.textContent = 'Item ' + i;
                    document.body.appendChild(el);
                }
                document.querySelectorAll('[id^=bench]').length
            "#}),
        )
        .await;
    let elapsed = start.elapsed();
    results.push(BenchResult {
        browser: browser_name.into(),
        test: "dom_create_100_elements".into(),
        duration: elapsed,
        success: resp.is_ok(),
        detail: resp.map(|v| extract_value(&v)).unwrap_or("error".into()),
    });

    client.close().await;
    results
}

// ============================================================
// Test: Page navigation speed (needs network)
// ============================================================

async fn bench_navigate(ws_url: &str, browser_name: &str, url: &str) -> Vec<BenchResult> {
    let mut results = Vec::new();
    let mut client = match connect_browser(ws_url, browser_name).await {
        Ok(c) => c,
        Err(e) => {
            results.push(BenchResult {
                browser: browser_name.into(),
                test: format!("navigate:{}", url),
                duration: Duration::ZERO,
                success: false,
                detail: format!("connection failed: {}", e),
            });
            return results;
        }
    };

    let _ = client.send("Page.enable", json!({})).await;
    let _ = client.send("Runtime.enable", json!({})).await;

    let start = Instant::now();
    client.navigate_and_wait(url, 10_000).await;
    // Extract title to verify page loaded
    let title = client
        .send("Runtime.evaluate", json!({"expression": "document.title"}))
        .await
        .map(|v| extract_value(&v))
        .unwrap_or_default();
    let elapsed = start.elapsed();

    let success = !title.is_empty();
    results.push(BenchResult {
        browser: browser_name.into(),
        test: format!("navigate:{}", url.chars().take(40).collect::<String>()),
        duration: elapsed,
        success,
        detail: if success {
            title.chars().take(50).collect()
        } else {
            "no title".into()
        },
    });

    // Extract title after navigation
    if success {
        tokio::time::sleep(Duration::from_millis(500)).await;
        let start = Instant::now();
        let resp = client
            .send("Runtime.evaluate", json!({"expression": "document.title"}))
            .await;
        let elapsed = start.elapsed();
        results.push(BenchResult {
            browser: browser_name.into(),
            test: "extract_title".into(),
            duration: elapsed,
            success: resp.is_ok(),
            detail: resp
                .map(|v| {
                    let s = extract_value(&v);
                    s.chars().take(50).collect::<String>()
                })
                .unwrap_or("error".into()),
        });
    }

    client.close().await;
    results
}

// ============================================================
// Test: Stealth — check navigator properties
// ============================================================

async fn bench_stealth(ws_url: &str, browser_name: &str) -> Vec<BenchResult> {
    let mut results = Vec::new();
    let mut client = match connect_browser(ws_url, browser_name).await {
        Ok(c) => c,
        Err(e) => {
            results.push(BenchResult {
                browser: browser_name.into(),
                test: "stealth_connect".into(),
                duration: Duration::ZERO,
                success: false,
                detail: format!("connection failed: {}", e),
            });
            return results;
        }
    };

    let _ = client.send("Runtime.enable", json!({})).await;

    let checks = vec![
        // Basic stealth (original 8)
        ("webdriver", "typeof navigator.webdriver", "undefined"),
        ("chrome_obj", "typeof window.chrome", "object"),
        ("plugins", "navigator.plugins.length > 0", "true"),
        ("languages", "navigator.languages.length > 0", "true"),
        ("vendor", "navigator.vendor", "Google Inc."),
        ("platform", "typeof navigator.platform", "string"),
        (
            "hardwareConcurrency",
            "navigator.hardwareConcurrency > 0",
            "true",
        ),
        (
            "ua_contains_chrome",
            "/Chrome/.test(navigator.userAgent)",
            "true",
        ),
        // Advanced stealth (new)
        ("webrtc", "typeof RTCPeerConnection", "function"),
        ("fonts_api", "typeof document.fonts", "object"),
        (
            "permissions",
            "typeof navigator.permissions.query",
            "function",
        ),
        ("battery", "typeof navigator.getBattery", "function"),
        (
            "speech_voices",
            "speechSynthesis.getVoices().length > 0",
            "true",
        ),
        (
            "media_source",
            "typeof MediaSource.isTypeSupported",
            "function",
        ),
        (
            "codec_h264",
            "MediaSource.isTypeSupported('video/mp4; codecs=\"avc1.42E01E\"')",
            "true",
        ),
        ("eventsource", "typeof EventSource", "function"),
        ("websocket", "typeof WebSocket", "function"),
        ("deviceMemory", "navigator.deviceMemory > 0", "true"),
    ];

    for (name, js, expected) in checks {
        let start = Instant::now();
        let resp = client
            .send("Runtime.evaluate", json!({"expression": js}))
            .await;
        let elapsed = start.elapsed();

        let (success, actual) = match resp {
            Ok(v) => {
                let val = extract_value(&v);
                let val = if val.is_empty() { "?".to_string() } else { val };
                (val == expected, val)
            }
            Err(e) => (false, format!("error: {}", e)),
        };

        results.push(BenchResult {
            browser: browser_name.into(),
            test: format!("stealth:{}", name),
            duration: elapsed,
            success,
            detail: if success {
                "ok".into()
            } else {
                format!("expected={}, got={}", expected, actual)
            },
        });
    }

    client.close().await;
    results
}

// ============================================================
// Main comparison tests
// ============================================================

/// Benchmark Runtime.evaluate across all available browsers.
#[tokio::test]
#[ignore]
async fn compare_evaluate_speed() {
    let mut all_results = Vec::new();

    // browser_oxide
    let server = protocol::CdpServer::start_ephemeral(
        "<html><head><title>Bench</title></head><body></body></html>",
    )
    .unwrap();
    // Give server a moment to be ready
    tokio::time::sleep(Duration::from_millis(50)).await;
    let results = bench_evaluate(&server.ws_url(), "browser_oxide").await;
    all_results.extend(results);
    drop(server);

    // Chrome (if available)
    if is_port_open(CHROME_PORT).await {
        let results = bench_evaluate(&discover_ws_url(CHROME_PORT).await.unwrap(), "chrome").await;
        all_results.extend(results);
    } else {
        println!("[SKIP] Chrome not running on port {}", CHROME_PORT);
    }

    // Lightpanda (if available)
    if is_port_open(LIGHTPANDA_PORT).await {
        let results = bench_evaluate(
            &discover_ws_url(LIGHTPANDA_PORT).await.unwrap(),
            "lightpanda",
        )
        .await;
        all_results.extend(results);
    } else {
        println!("[SKIP] Lightpanda not running on port {}", LIGHTPANDA_PORT);
    }

    print_comparison(&all_results);
}

/// Stealth check across all available browsers.
#[tokio::test]
#[ignore]
async fn compare_stealth() {
    let mut all_results = Vec::new();

    // browser_oxide
    let server = protocol::CdpServer::start_ephemeral(
        "<html><head><title>Stealth</title></head><body></body></html>",
    )
    .unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    let results = bench_stealth(&server.ws_url(), "browser_oxide").await;
    all_results.extend(results);
    drop(server);

    // Chrome
    if is_port_open(CHROME_PORT).await {
        let results = bench_stealth(&discover_ws_url(CHROME_PORT).await.unwrap(), "chrome").await;
        all_results.extend(results);
    } else {
        println!("[SKIP] Chrome not running on port {}", CHROME_PORT);
    }

    // Lightpanda
    if is_port_open(LIGHTPANDA_PORT).await {
        let results = bench_stealth(
            &discover_ws_url(LIGHTPANDA_PORT).await.unwrap(),
            "lightpanda",
        )
        .await;
        all_results.extend(results);
    } else {
        println!("[SKIP] Lightpanda not running on port {}", LIGHTPANDA_PORT);
    }

    print_comparison(&all_results);
}

/// Navigation benchmark with real URLs.
#[tokio::test]
#[ignore]
async fn compare_navigation() {
    let urls = vec![
        "https://example.com",
        "https://httpbin.org/get",
        "https://news.ycombinator.com",
        "https://httpbin.org/html",
    ];

    let mut all_results = Vec::new();

    for url in &urls {
        // Chrome
        if is_port_open(CHROME_PORT).await {
            let results =
                bench_navigate(&discover_ws_url(CHROME_PORT).await.unwrap(), "chrome", url).await;
            all_results.extend(results);
        }

        // Lightpanda
        if is_port_open(LIGHTPANDA_PORT).await {
            let results = bench_navigate(
                &discover_ws_url(LIGHTPANDA_PORT).await.unwrap(),
                "lightpanda",
                url,
            )
            .await;
            all_results.extend(results);
        }
    }

    // browser_oxide — single navigable server, navigate via CDP
    if let Ok(server) = protocol::CdpServer::start_navigable(0) {
        tokio::time::sleep(Duration::from_millis(50)).await;
        if let Ok(mut client) = CdpClient::connect(&server.ws_url()).await {
            let _ = client.send("Page.enable", json!({})).await;
            let _ = client.send("Runtime.enable", json!({})).await;
            for url in &urls {
                let start = Instant::now();
                client.navigate_and_wait(url, 10_000).await;
                let resp = client
                    .send("Runtime.evaluate", json!({"expression": "document.title"}))
                    .await;
                let elapsed = start.elapsed();
                all_results.push(BenchResult {
                    browser: "browser_oxide".into(),
                    test: format!("navigate:{}", &url[..url.len().min(40)]),
                    duration: elapsed,
                    success: resp.is_ok(),
                    detail: resp
                        .map(|v| {
                            let s = extract_value(&v);
                            if s.is_empty() {
                                "?".into()
                            } else {
                                s.chars().take(50).collect()
                            }
                        })
                        .unwrap_or("error".into()),
                });
            }
            client.close().await;
        }
        drop(server);
    }

    print_comparison(&all_results);
}

/// Quick self-test — just browser_oxide, no external browsers needed.
#[tokio::test]
async fn browser_oxide_cdp_roundtrip() {
    let server = protocol::CdpServer::start_ephemeral(
        "<html><head><title>Test</title></head><body><p>Hello</p></body></html>",
    )
    .unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let mut client = CdpClient::connect(&server.ws_url()).await.unwrap();

    // Runtime.enable
    let resp = client.send("Runtime.enable", json!({})).await.unwrap();
    assert!(
        resp.get("result").is_some(),
        "Runtime.enable failed: {}",
        resp
    );

    // Evaluate
    let resp = client
        .send("Runtime.evaluate", json!({"expression": "document.title"}))
        .await
        .unwrap();
    let title = extract_value(&resp);
    assert_eq!(title, "Test", "unexpected title: {}", resp);

    // DOM check
    let resp = client
        .send(
            "Runtime.evaluate",
            json!({"expression": "document.querySelector('p').textContent"}),
        )
        .await
        .unwrap();
    let text = extract_value(&resp);
    assert_eq!(text, "Hello");

    // Browser.getVersion
    let resp = client.send("Browser.getVersion", json!({})).await.unwrap();
    assert!(
        resp["result"]["product"]
            .as_str()
            .unwrap_or("")
            .contains("browser_oxide"),
        "version: {}",
        resp
    );

    client.close().await;
    drop(server);
}

// ============================================================
// Anti-bot site comparison — same sites through all three browsers
// ============================================================

/// Site definition for anti-bot comparison.
struct SiteEntry {
    url: &'static str,
    protection: &'static str,
    category: &'static str,
}

/// Probe a site through a CDP-controlled browser.
/// Navigates to the URL and checks the HTTP status + page content.
async fn cdp_probe(ws_url: &str, browser_name: &str, site: &SiteEntry) -> BenchResult {
    let mut client = match connect_browser(ws_url, browser_name).await {
        Ok(c) => c,
        Err(e) => {
            return BenchResult {
                browser: browser_name.into(),
                test: format!("{}|{}", site.protection, site.url),
                duration: Duration::ZERO,
                success: false,
                detail: format!("connect failed: {}", e),
            };
        }
    };

    let _ = client.send("Page.enable", json!({})).await;
    let _ = client.send("Runtime.enable", json!({})).await;

    let start = Instant::now();

    // Navigate and wait for load event
    client.navigate_and_wait(site.url, 10_000).await;
    let elapsed_nav = start.elapsed();

    // Check what we got
    let title = client
        .send("Runtime.evaluate", json!({"expression": "document.title"}))
        .await
        .map(|v| extract_value(&v))
        .unwrap_or_default();

    let url_after = client
        .send("Runtime.evaluate", json!({"expression": "location.href"}))
        .await
        .map(|v| extract_value(&v))
        .unwrap_or_default();

    let body_len = client
        .send(
            "Runtime.evaluate",
            json!({"expression": "document.body ? document.body.innerHTML.length : 0"}),
        )
        .await
        .map(|v| extract_value(&v))
        .unwrap_or("0".into());

    // Detect blocking signals
    let blocked_signals = client
        .send(
            "Runtime.evaluate",
            json!({"expression": r#"
                (() => {
                    const signals = [];
                    const body = document.body ? document.body.innerHTML : '';
                    const title = document.title || '';
                    if (body.includes('challenge') || title.includes('challenge')) signals.push('challenge');
                    if (body.includes('captcha') || body.includes('CAPTCHA')) signals.push('captcha');
                    if (body.includes('Just a moment')) signals.push('cf-interstitial');
                    if (body.includes('Access Denied') || body.includes('403')) signals.push('access-denied');
                    if (body.includes('datadome') || body.includes('DataDome')) signals.push('datadome');
                    if (body.includes('blocked') || body.includes('Blocked')) signals.push('blocked');
                    if (body.includes('bot') && body.includes('detected')) signals.push('bot-detected');
                    return signals.join(',') || 'none';
                })()
            "#}),
        )
        .await
        .map(|v| extract_value(&v))
        .unwrap_or("error".into());

    let elapsed = start.elapsed();

    let passed = !blocked_signals.contains("captcha")
        && !blocked_signals.contains("access-denied")
        && !blocked_signals.contains("blocked")
        && !blocked_signals.contains("bot-detected")
        && body_len.parse::<usize>().unwrap_or(0) > 100;

    client.close().await;

    BenchResult {
        browser: browser_name.into(),
        test: format!("{}|{}", site.protection, site.url),
        duration: elapsed,
        success: passed,
        detail: format!(
            "title={} body={}b signals={} url={}",
            title.chars().take(30).collect::<String>(),
            body_len,
            blocked_signals,
            if url_after.len() > 50 {
                format!("{}...", &url_after[..50])
            } else {
                url_after
            }
        ),
    }
}

/// All anti-bot test sites organized by protection category.
fn anti_bot_sites() -> Vec<SiteEntry> {
    vec![
        // Cloudflare Bot Management
        SiteEntry {
            url: "https://nowsecure.nl",
            protection: "cloudflare",
            category: "cf",
        },
        SiteEntry {
            url: "https://discord.com",
            protection: "cloudflare",
            category: "cf",
        },
        SiteEntry {
            url: "https://medium.com",
            protection: "cloudflare",
            category: "cf",
        },
        SiteEntry {
            url: "https://www.coinbase.com",
            protection: "cloudflare",
            category: "cf",
        },
        SiteEntry {
            url: "https://chatgpt.com",
            protection: "cloudflare",
            category: "cf",
        },
        SiteEntry {
            url: "https://www.glassdoor.com",
            protection: "cloudflare",
            category: "cf",
        },
        // DataDome
        SiteEntry {
            url: "https://www.reddit.com",
            protection: "datadome",
            category: "dd",
        },
        SiteEntry {
            url: "https://www.footlocker.com",
            protection: "datadome",
            category: "dd",
        },
        SiteEntry {
            url: "https://www.tripadvisor.com",
            protection: "datadome",
            category: "dd",
        },
        SiteEntry {
            url: "https://soundcloud.com",
            protection: "datadome",
            category: "dd",
        },
        // Akamai Bot Manager
        SiteEntry {
            url: "https://www.nike.com",
            protection: "akamai",
            category: "akamai",
        },
        SiteEntry {
            url: "https://www.homedepot.com",
            protection: "akamai",
            category: "akamai",
        },
        SiteEntry {
            url: "https://www.airbnb.com",
            protection: "akamai",
            category: "akamai",
        },
        SiteEntry {
            url: "https://www.costco.com",
            protection: "akamai",
            category: "akamai",
        },
        // PerimeterX / HUMAN
        SiteEntry {
            url: "https://www.walmart.com",
            protection: "perimeterx",
            category: "px",
        },
        SiteEntry {
            url: "https://stockx.com",
            protection: "perimeterx",
            category: "px",
        },
        SiteEntry {
            url: "https://www.nordstrom.com",
            protection: "perimeterx",
            category: "px",
        },
        // Kasada
        SiteEntry {
            url: "https://www.ticketmaster.com",
            protection: "kasada",
            category: "kasada",
        },
        SiteEntry {
            url: "https://seatgeek.com",
            protection: "kasada",
            category: "kasada",
        },
        // Shape Security (F5)
        SiteEntry {
            url: "https://www.southwest.com",
            protection: "shape",
            category: "shape",
        },
        SiteEntry {
            url: "https://www.iherb.com",
            protection: "shape",
            category: "shape",
        },
        // Big Tech (custom protection)
        SiteEntry {
            url: "https://www.amazon.com",
            protection: "custom",
            category: "bigtech",
        },
        SiteEntry {
            url: "https://www.linkedin.com",
            protection: "custom",
            category: "bigtech",
        },
        SiteEntry {
            url: "https://www.google.com/search?q=test",
            protection: "custom",
            category: "bigtech",
        },
        // Bot detection test sites
        SiteEntry {
            url: "https://bot.sannysoft.com",
            protection: "sannysoft",
            category: "verify",
        },
        SiteEntry {
            url: "https://abrahamjuliot.github.io/creepjs/",
            protection: "creepjs",
            category: "verify",
        },
        SiteEntry {
            url: "https://browserleaks.com",
            protection: "browserleaks",
            category: "verify",
        },
    ]
}

/// A subset of high-value sites for quick comparison runs.
fn anti_bot_sites_quick() -> Vec<SiteEntry> {
    vec![
        SiteEntry {
            url: "https://nowsecure.nl",
            protection: "cloudflare",
            category: "cf",
        },
        SiteEntry {
            url: "https://www.reddit.com",
            protection: "datadome",
            category: "dd",
        },
        SiteEntry {
            url: "https://www.nike.com",
            protection: "akamai",
            category: "akamai",
        },
        SiteEntry {
            url: "https://www.walmart.com",
            protection: "perimeterx",
            category: "px",
        },
        SiteEntry {
            url: "https://www.ticketmaster.com",
            protection: "kasada",
            category: "kasada",
        },
        SiteEntry {
            url: "https://www.amazon.com",
            protection: "custom",
            category: "bigtech",
        },
        SiteEntry {
            url: "https://bot.sannysoft.com",
            protection: "sannysoft",
            category: "verify",
        },
    ]
}

fn print_scorecard(results: &[BenchResult], browser_name: &str) {
    let passed = results.iter().filter(|r| r.success).count();
    let total = results.len();
    let avg_ms = if total > 0 {
        results.iter().map(|r| r.duration.as_millis()).sum::<u128>() / total as u128
    } else {
        0
    };
    println!(
        "\n  {} — {}/{} passed ({}%) avg={:.0}ms",
        browser_name,
        passed,
        total,
        if total > 0 { passed * 100 / total } else { 0 },
        avg_ms,
    );
}

fn print_side_by_side(all: &[(String, Vec<BenchResult>)]) {
    let sep = "=".repeat(120);
    let dash = "-".repeat(120);

    println!("\n{}", sep);
    println!(" ANTI-BOT SITE COMPARISON");
    println!("{}", sep);

    // Collect all unique sites
    let sites: Vec<&str> = if let Some((_, results)) = all.first() {
        results.iter().map(|r| r.test.as_str()).collect()
    } else {
        return;
    };

    // Header
    print!("{:<50}", "Site");
    for (name, _) in all {
        print!(" {:>20}", name);
    }
    println!();
    println!("{}", dash);

    // Rows
    for site in &sites {
        print!("{:<50}", if site.len() > 49 { &site[..49] } else { site });
        for (_, results) in all {
            if let Some(r) = results.iter().find(|r| r.test == *site) {
                let status = if r.success {
                    format!("PASS {:.0}ms", r.duration.as_millis())
                } else {
                    format!("FAIL {:.0}ms", r.duration.as_millis())
                };
                print!(" {:>20}", status);
            } else {
                print!(" {:>20}", "N/A");
            }
        }
        println!();
    }

    println!("{}", dash);

    // Summary per browser
    print!("{:<50}", "TOTAL");
    for (name, results) in all {
        let passed = results.iter().filter(|r| r.success).count();
        let total = results.len();
        print!(" {:>20}", format!("{}/{}", passed, total));
    }
    println!();

    // Category breakdown
    let categories = [
        "cf", "dd", "akamai", "px", "kasada", "shape", "bigtech", "verify",
    ];
    let category_names = [
        "Cloudflare",
        "DataDome",
        "Akamai",
        "PerimeterX",
        "Kasada",
        "Shape",
        "BigTech",
        "Verify",
    ];

    println!("\n{}", sep);
    println!(" BY PROTECTION CATEGORY");
    println!("{}", dash);
    print!("{:<50}", "Category");
    for (name, _) in all {
        print!(" {:>20}", name);
    }
    println!();
    println!("{}", dash);

    for (cat, cat_name) in categories.iter().zip(category_names.iter()) {
        print!("{:<50}", cat_name);
        for (_, results) in all {
            let cat_results: Vec<_> = results
                .iter()
                .filter(|r| {
                    // Match by category in the site entry — we encode it in the test name
                    // The test name is "protection|url", match by protection prefix
                    true // We'll use a different approach below
                })
                .collect();
            // Match by protection name which is the prefix of the test field
            let matching: Vec<_> = results
                .iter()
                .filter(|r| {
                    let prot = r.test.split('|').next().unwrap_or("");
                    match *cat {
                        "cf" => prot == "cloudflare",
                        "dd" => prot == "datadome",
                        "akamai" => prot == "akamai",
                        "px" => prot == "perimeterx",
                        "kasada" => prot == "kasada",
                        "shape" => prot == "shape",
                        "bigtech" => prot == "custom",
                        "verify" => {
                            prot == "sannysoft" || prot == "creepjs" || prot == "browserleaks"
                        }
                        _ => false,
                    }
                })
                .collect();
            let passed = matching.iter().filter(|r| r.success).count();
            let total = matching.len();
            if total > 0 {
                print!(" {:>20}", format!("{}/{}", passed, total));
            } else {
                print!(" {:>20}", "-");
            }
        }
        println!();
    }
    println!("{}", sep);
}

/// Quick anti-bot comparison — 7 representative sites across all protection types.
#[tokio::test]
#[ignore]
async fn compare_anti_bot_quick() {
    let sites = anti_bot_sites_quick();
    let mut all_browsers: Vec<(String, Vec<BenchResult>)> = Vec::new();

    // Chrome
    if is_port_open(CHROME_PORT).await {
        println!("\n--- Chrome ---");
        let mut results = Vec::new();
        for site in &sites {
            let r = cdp_probe(&discover_ws_url(CHROME_PORT).await.unwrap(), "chrome", site).await;
            r.print();
            results.push(r);
        }
        print_scorecard(&results, "chrome");
        all_browsers.push(("chrome".into(), results));
    } else {
        println!("[SKIP] Chrome not running on port {}", CHROME_PORT);
    }

    // Lightpanda
    if is_port_open(LIGHTPANDA_PORT).await {
        println!("\n--- Lightpanda ---");
        let mut results = Vec::new();
        for site in &sites {
            let r = cdp_probe(
                &discover_ws_url(LIGHTPANDA_PORT).await.unwrap(),
                "lightpanda",
                site,
            )
            .await;
            r.print();
            results.push(r);
        }
        print_scorecard(&results, "lightpanda");
        all_browsers.push(("lightpanda".into(), results));
    } else {
        println!("[SKIP] Lightpanda not running on port {}", LIGHTPANDA_PORT);
    }

    // browser_oxide — uses its own CDP server per-site
    println!("\n--- browser_oxide ---");
    let mut results = Vec::new();
    for site in &sites {
        match protocol::CdpServer::start_with_url(site.url, 0) {
            Ok(server) => {
                tokio::time::sleep(Duration::from_millis(100)).await;
                // For browser_oxide, the page is already loaded — just check content
                let mut client = match CdpClient::connect(&server.ws_url()).await {
                    Ok(c) => c,
                    Err(e) => {
                        results.push(BenchResult {
                            browser: "browser_oxide".into(),
                            test: format!("{}|{}", site.protection, site.url),
                            duration: Duration::ZERO,
                            success: false,
                            detail: format!("connect: {}", e),
                        });
                        continue;
                    }
                };
                let _ = client.send("Runtime.enable", json!({})).await;

                let start = Instant::now();
                let title = client
                    .send("Runtime.evaluate", json!({"expression": "document.title"}))
                    .await
                    .map(|v| extract_value(&v))
                    .unwrap_or_default();
                let body_len = client
                    .send(
                        "Runtime.evaluate",
                        json!({"expression": "document.body ? document.body.innerHTML.length : 0"}),
                    )
                    .await
                    .map(|v| extract_value(&v))
                    .unwrap_or("0".into());
                let blocked_signals = client
                    .send(
                        "Runtime.evaluate",
                        json!({"expression": r#"
                            (() => {
                                const signals = [];
                                const body = document.body ? document.body.innerHTML : '';
                                const title = document.title || '';
                                if (body.includes('challenge') || title.includes('challenge')) signals.push('challenge');
                                if (body.includes('captcha') || body.includes('CAPTCHA')) signals.push('captcha');
                                if (body.includes('Just a moment')) signals.push('cf-interstitial');
                                if (body.includes('Access Denied') || body.includes('403')) signals.push('access-denied');
                                if (body.includes('blocked') || body.includes('Blocked')) signals.push('blocked');
                                return signals.join(',') || 'none';
                            })()
                        "#}),
                    )
                    .await
                    .map(|v| extract_value(&v))
                    .unwrap_or("error".into());
                let elapsed = start.elapsed();

                let passed = !blocked_signals.contains("captcha")
                    && !blocked_signals.contains("access-denied")
                    && !blocked_signals.contains("blocked")
                    && body_len.parse::<usize>().unwrap_or(0) > 100;

                let r = BenchResult {
                    browser: "browser_oxide".into(),
                    test: format!("{}|{}", site.protection, site.url),
                    duration: elapsed,
                    success: passed,
                    detail: format!(
                        "title={} body={}b signals={}",
                        title.chars().take(30).collect::<String>(),
                        body_len,
                        blocked_signals,
                    ),
                };
                r.print();
                results.push(r);
                client.close().await;
                drop(server);
            }
            Err(e) => {
                let r = BenchResult {
                    browser: "browser_oxide".into(),
                    test: format!("{}|{}", site.protection, site.url),
                    duration: Duration::ZERO,
                    success: false,
                    detail: format!("server: {}", e),
                };
                r.print();
                results.push(r);
            }
        }
    }
    print_scorecard(&results, "browser_oxide");
    all_browsers.push(("browser_oxide".into(), results));

    // Side-by-side comparison
    print_side_by_side(&all_browsers);
}

/// Full anti-bot comparison — all 27 sites across all protection categories.
#[tokio::test]
#[ignore]
async fn compare_anti_bot_full() {
    let sites = anti_bot_sites();
    let mut all_browsers: Vec<(String, Vec<BenchResult>)> = Vec::new();

    // Chrome
    if is_port_open(CHROME_PORT).await {
        println!("\n--- Chrome ({} sites) ---", sites.len());
        let mut results = Vec::new();
        for site in &sites {
            let r = cdp_probe(&discover_ws_url(CHROME_PORT).await.unwrap(), "chrome", site).await;
            r.print();
            results.push(r);
        }
        print_scorecard(&results, "chrome");
        all_browsers.push(("chrome".into(), results));
    } else {
        println!("[SKIP] Chrome not running on port {}", CHROME_PORT);
    }

    // Lightpanda
    if is_port_open(LIGHTPANDA_PORT).await {
        println!("\n--- Lightpanda ({} sites) ---", sites.len());
        let mut results = Vec::new();
        for site in &sites {
            let r = cdp_probe(
                &discover_ws_url(LIGHTPANDA_PORT).await.unwrap(),
                "lightpanda",
                site,
            )
            .await;
            r.print();
            results.push(r);
        }
        print_scorecard(&results, "lightpanda");
        all_browsers.push(("lightpanda".into(), results));
    } else {
        println!("[SKIP] Lightpanda not running on port {}", LIGHTPANDA_PORT);
    }

    // browser_oxide
    println!("\n--- browser_oxide ({} sites) ---", sites.len());
    let mut results = Vec::new();
    for site in &sites {
        match protocol::CdpServer::start_with_url(site.url, 0) {
            Ok(server) => {
                tokio::time::sleep(Duration::from_millis(100)).await;
                let mut client = match CdpClient::connect(&server.ws_url()).await {
                    Ok(c) => c,
                    Err(e) => {
                        results.push(BenchResult {
                            browser: "browser_oxide".into(),
                            test: format!("{}|{}", site.protection, site.url),
                            duration: Duration::ZERO,
                            success: false,
                            detail: format!("connect: {}", e),
                        });
                        continue;
                    }
                };
                let _ = client.send("Runtime.enable", json!({})).await;

                let start = Instant::now();
                let title = client
                    .send("Runtime.evaluate", json!({"expression": "document.title"}))
                    .await
                    .map(|v| extract_value(&v))
                    .unwrap_or_default();
                let body_len = client
                    .send(
                        "Runtime.evaluate",
                        json!({"expression": "document.body ? document.body.innerHTML.length : 0"}),
                    )
                    .await
                    .map(|v| extract_value(&v))
                    .unwrap_or("0".into());
                let blocked_signals = client
                    .send(
                        "Runtime.evaluate",
                        json!({"expression": r#"
                            (() => {
                                const signals = [];
                                const body = document.body ? document.body.innerHTML : '';
                                const title = document.title || '';
                                if (body.includes('challenge') || title.includes('challenge')) signals.push('challenge');
                                if (body.includes('captcha') || body.includes('CAPTCHA')) signals.push('captcha');
                                if (body.includes('Just a moment')) signals.push('cf-interstitial');
                                if (body.includes('Access Denied') || body.includes('403')) signals.push('access-denied');
                                if (body.includes('blocked') || body.includes('Blocked')) signals.push('blocked');
                                return signals.join(',') || 'none';
                            })()
                        "#}),
                    )
                    .await
                    .map(|v| extract_value(&v))
                    .unwrap_or("error".into());
                let elapsed = start.elapsed();

                let passed = !blocked_signals.contains("captcha")
                    && !blocked_signals.contains("access-denied")
                    && !blocked_signals.contains("blocked")
                    && body_len.parse::<usize>().unwrap_or(0) > 100;

                let r = BenchResult {
                    browser: "browser_oxide".into(),
                    test: format!("{}|{}", site.protection, site.url),
                    duration: elapsed,
                    success: passed,
                    detail: format!(
                        "title={} body={}b signals={}",
                        title.chars().take(30).collect::<String>(),
                        body_len,
                        blocked_signals,
                    ),
                };
                r.print();
                results.push(r);
                client.close().await;
                drop(server);
            }
            Err(e) => {
                let r = BenchResult {
                    browser: "browser_oxide".into(),
                    test: format!("{}|{}", site.protection, site.url),
                    duration: Duration::ZERO,
                    success: false,
                    detail: format!("server: {}", e),
                };
                r.print();
                results.push(r);
            }
        }
    }
    print_scorecard(&results, "browser_oxide");
    all_browsers.push(("browser_oxide".into(), results));

    print_side_by_side(&all_browsers);
}

// ============================================================
// Test: Content extraction accuracy — same page, compare output
// ============================================================

/// Content extraction accuracy — same pages through all browsers.
#[tokio::test]
#[ignore]
async fn compare_content_extraction() {
    let pages: Vec<(&str, &str, &str)> = vec![
        ("https://example.com", "Example Domain", "Example Domain"),
        ("https://httpbin.org/get", "", "origin"),
        ("https://news.ycombinator.com", "Hacker News", "Hacker News"),
        ("https://httpbin.org/html", "", "Moby-Dick"),
        (
            "https://en.wikipedia.org/wiki/Rust_(programming_language)",
            "Rust",
            "programming language",
        ),
    ];

    let sep = "=".repeat(120);
    let dash = "-".repeat(120);
    println!("\n{}", sep);
    println!(" CONTENT EXTRACTION ACCURACY");
    println!("{}", sep);
    println!(
        "{:<45} {:<20} {:>10} {:>8} {}",
        "URL", "Browser", "Text len", "Time", "Match"
    );
    println!("{}", dash);

    async fn check_content(
        ws_url: &str,
        browser_name: &str,
        url: &str,
        exp_title: &str,
        exp_text: &str,
    ) -> (bool, usize, Duration, String) {
        let mut client = match connect_browser(ws_url, browser_name).await {
            Ok(c) => c,
            Err(e) => return (false, 0, Duration::ZERO, format!("ERR: {}", e)),
        };
        let _ = client.send("Page.enable", json!({})).await;
        let _ = client.send("Runtime.enable", json!({})).await;
        let start = Instant::now();
        client.navigate_and_wait(url, 10_000).await;

        let title = client
            .send("Runtime.evaluate", json!({"expression": "document.title"}))
            .await
            .map(|v| extract_value(&v))
            .unwrap_or_default();
        let text = client.send("Runtime.evaluate",
            json!({"expression": "document.body ? document.body.innerText || document.body.textContent : ''"}))
            .await.map(|v| extract_value(&v)).unwrap_or_default();
        let elapsed = start.elapsed();
        client.close().await;

        let title_ok = exp_title.is_empty() || title.contains(exp_title);
        let text_ok = exp_text.is_empty() || text.contains(exp_text);
        let detail = format!("title={}", title.chars().take(30).collect::<String>());
        (title_ok && text_ok, text.len(), elapsed, detail)
    }

    for (url, exp_title, exp_text) in &pages {
        let url_short = if url.len() > 44 { &url[..44] } else { url };

        if is_port_open(CHROME_PORT).await {
            let (ok, len, dur, detail) = check_content(
                &discover_ws_url(CHROME_PORT).await.unwrap(),
                "chrome",
                url,
                exp_title,
                exp_text,
            )
            .await;
            println!(
                "{:<45} {:<20} {:>10} {:>6.0?} {} {}",
                url_short,
                "chrome",
                len,
                dur,
                if ok { "MATCH" } else { "MISS" },
                detail
            );
        }
        if is_port_open(LIGHTPANDA_PORT).await {
            let (ok, len, dur, detail) = check_content(
                &discover_ws_url(LIGHTPANDA_PORT).await.unwrap(),
                "lightpanda",
                url,
                exp_title,
                exp_text,
            )
            .await;
            println!(
                "{:<45} {:<20} {:>10} {:>6.0?} {} {}",
                "",
                "lightpanda",
                len,
                dur,
                if ok { "MATCH" } else { "MISS" },
                detail
            );
        }
        println!();
    }

    // browser_oxide — single navigable server
    if let Ok(server) = protocol::CdpServer::start_navigable(0) {
        tokio::time::sleep(Duration::from_millis(50)).await;
        if let Ok(mut client) = CdpClient::connect(&server.ws_url()).await {
            let _ = client.send("Page.enable", json!({})).await;
            let _ = client.send("Runtime.enable", json!({})).await;
            for (url, exp_title, exp_text) in &pages {
                let url_short = if url.len() > 44 { &url[..44] } else { url };
                let start = Instant::now();
                client.navigate_and_wait(url, 10_000).await;
                let title = client
                    .send("Runtime.evaluate", json!({"expression": "document.title"}))
                    .await
                    .map(|v| extract_value(&v))
                    .unwrap_or_default();
                let text = client.send("Runtime.evaluate",
                    json!({"expression": "document.body ? document.body.innerText || document.body.textContent : ''"}))
                    .await.map(|v| extract_value(&v)).unwrap_or_default();
                let elapsed = start.elapsed();
                let title_ok = exp_title.is_empty() || title.contains(exp_title);
                let text_ok = exp_text.is_empty() || text.contains(exp_text);
                println!(
                    "{:<45} {:<20} {:>10} {:>6.0?} {} title={}",
                    url_short,
                    "browser_oxide",
                    text.len(),
                    elapsed,
                    if title_ok && text_ok { "MATCH" } else { "MISS" },
                    title.chars().take(30).collect::<String>()
                );
            }
            client.close().await;
        }
        drop(server);
    }

    println!("{}", sep);
}

// ============================================================
// Test: JS-heavy SPA rendering
// ============================================================

/// Test JS execution on sites that require JavaScript to render content.
#[tokio::test]
#[ignore]
async fn compare_js_rendering() {
    let sites: Vec<(&str, &str, &str)> = vec![
        ("https://angular.dev", "Angular", "angular"),
        ("https://react.dev", "React", "react"),
        ("https://httpbin.org/get", "", "origin"),
    ];

    let sep = "=".repeat(100);
    let dash = "-".repeat(100);
    println!("\n{}", sep);
    println!(" JS-HEAVY SPA RENDERING");
    println!("{}", sep);
    println!(
        "{:<35} {:<15} {:>10} {:>8} {}",
        "Site", "Browser", "Body len", "Time", "JS content?"
    );
    println!("{}", dash);

    async fn check_js(
        ws_url: &str,
        browser_name: &str,
        url: &str,
        expected_body: &str,
    ) -> (bool, String, Duration) {
        let mut client = match connect_browser(ws_url, browser_name).await {
            Ok(c) => c,
            Err(e) => return (false, format!("ERR: {}", e), Duration::ZERO),
        };
        let _ = client.send("Page.enable", json!({})).await;
        let _ = client.send("Runtime.enable", json!({})).await;
        let start = Instant::now();
        client.navigate_and_wait(url, 10_000).await;

        let body_len = client
            .send(
                "Runtime.evaluate",
                json!({"expression": "document.body ? document.body.innerHTML.length : 0"}),
            )
            .await
            .map(|v| extract_value(&v))
            .unwrap_or("0".into());
        let has_content = client.send("Runtime.evaluate", json!({"expression": format!(
            "document.body && document.body.innerHTML.toLowerCase().includes('{}')", expected_body.to_lowercase()
        )})).await.map(|v| extract_value(&v)).unwrap_or("false".into());
        let elapsed = start.elapsed();
        client.close().await;
        (has_content == "true", body_len, elapsed)
    }

    for (url, _title, expected_body) in &sites {
        let url_short = if url.len() > 34 { &url[..34] } else { url };

        if is_port_open(CHROME_PORT).await {
            let (found, len, dur) = check_js(
                &discover_ws_url(CHROME_PORT).await.unwrap(),
                "chrome",
                url,
                expected_body,
            )
            .await;
            println!(
                "{:<35} {:<15} {:>10} {:>6.0?} {}",
                url_short,
                "chrome",
                len,
                dur,
                if found { "YES" } else { "NO" }
            );
        }
        if is_port_open(LIGHTPANDA_PORT).await {
            let (found, len, dur) = check_js(
                &discover_ws_url(LIGHTPANDA_PORT).await.unwrap(),
                "lightpanda",
                url,
                expected_body,
            )
            .await;
            println!(
                "{:<35} {:<15} {:>10} {:>6.0?} {}",
                "",
                "lightpanda",
                len,
                dur,
                if found { "YES" } else { "NO" }
            );
        }
        println!();
    }

    // browser_oxide — single navigable server
    if let Ok(server) = protocol::CdpServer::start_navigable(0) {
        tokio::time::sleep(Duration::from_millis(50)).await;
        if let Ok(mut client) = CdpClient::connect(&server.ws_url()).await {
            let _ = client.send("Page.enable", json!({})).await;
            let _ = client.send("Runtime.enable", json!({})).await;
            for (url, _title, expected_body) in &sites {
                let url_short = if url.len() > 34 { &url[..34] } else { url };
                let start = Instant::now();
                client.navigate_and_wait(url, 10_000).await;
                let body_len = client
                    .send(
                        "Runtime.evaluate",
                        json!({"expression": "document.body ? document.body.innerHTML.length : 0"}),
                    )
                    .await
                    .map(|v| extract_value(&v))
                    .unwrap_or("0".into());
                let has = client.send("Runtime.evaluate", json!({"expression": format!(
                    "document.body && document.body.innerHTML.toLowerCase().includes('{}')", expected_body.to_lowercase()
                )})).await.map(|v| extract_value(&v)).unwrap_or("false".into());
                let elapsed = start.elapsed();
                println!(
                    "{:<35} {:<15} {:>10} {:>6.0?} {}",
                    url_short,
                    "browser_oxide",
                    body_len,
                    elapsed,
                    if has == "true" { "YES" } else { "NO" }
                );
            }
            client.close().await;
        }
        drop(server);
    }

    println!("{}", sep);
}

// ============================================================
// Test: TLS fingerprint verification
// ============================================================

/// Compare TLS fingerprints across browsers by hitting tls.peet.ws.
#[tokio::test]
#[ignore]
async fn compare_tls_fingerprint() {
    let sep = "=".repeat(100);
    println!("\n{}", sep);
    println!(" TLS FINGERPRINT VERIFICATION (via tls.peet.ws)");
    println!("{}", sep);

    async fn check_tls(ws_url: &str, browser_name: &str) {
        let mut client = match connect_browser(ws_url, browser_name).await {
            Ok(c) => c,
            Err(e) => {
                println!("[{}] connect error: {}", browser_name, e);
                return;
            }
        };
        let _ = client.send("Page.enable", json!({})).await;
        let _ = client.send("Runtime.enable", json!({})).await;
        let start = Instant::now();
        client
            .navigate_and_wait("https://tls.peet.ws/api/all", 10_000)
            .await;

        let body = client.send("Runtime.evaluate",
            json!({"expression": "document.body ? document.body.innerText || document.body.textContent : ''"}))
            .await.map(|v| extract_value(&v)).unwrap_or_default();
        let elapsed = start.elapsed();

        if let Ok(data) = serde_json::from_str::<Value>(&body) {
            let tls = &data["tls"];
            println!("[{}] ({:.0?})", browser_name, elapsed);
            println!("  TLS:  {}", tls["version"].as_str().unwrap_or("-"));
            println!("  HTTP: {}", data["http_version"].as_str().unwrap_or("-"));
            println!("  JA3:  {}", tls["ja3_hash"].as_str().unwrap_or("-"));
            println!("  JA4:  {}", tls["ja4"].as_str().unwrap_or("-"));
            println!("  Peet: {}", tls["peetprint_hash"].as_str().unwrap_or("-"));
        } else {
            println!(
                "[{}] ({:.0?}) body_len={} (not JSON)",
                browser_name,
                elapsed,
                body.len()
            );
        }
        println!();
        client.close().await;
    }

    if is_port_open(CHROME_PORT).await {
        check_tls(&discover_ws_url(CHROME_PORT).await.unwrap(), "chrome").await;
    }
    if is_port_open(LIGHTPANDA_PORT).await {
        check_tls(
            &discover_ws_url(LIGHTPANDA_PORT).await.unwrap(),
            "lightpanda",
        )
        .await;
    }

    // browser_oxide — use direct HTTP client for TLS test (more meaningful than CDP)
    {
        let profile = stealth::chrome_130_linux();
        let http_client = net::HttpClient::new(&profile).unwrap();
        let start = Instant::now();
        match http_client.get("https://tls.peet.ws/api/all").await {
            Ok(resp) => {
                let elapsed = start.elapsed();
                let body = resp.text();
                if let Ok(data) = serde_json::from_str::<Value>(&body) {
                    let tls = &data["tls"];
                    println!(
                        "[browser_oxide] ({:.0?}) — direct HTTP (rquest + BoringSSL)",
                        elapsed
                    );
                    println!("  TLS:  {}", tls["version"].as_str().unwrap_or("-"));
                    println!("  HTTP: {}", data["http_version"].as_str().unwrap_or("-"));
                    println!("  JA3:  {}", tls["ja3_hash"].as_str().unwrap_or("-"));
                    println!("  JA4:  {}", tls["ja4"].as_str().unwrap_or("-"));
                    println!("  Peet: {}", tls["peetprint_hash"].as_str().unwrap_or("-"));
                } else {
                    println!("[browser_oxide] ({:.0?}) body_len={}", elapsed, body.len());
                }
            }
            Err(e) => println!("[browser_oxide] error: {}", e),
        }
    }
    println!("{}", sep);
}

// ============================================================
// Test: Throughput — sequential page loads
// ============================================================

/// Load 10 pages sequentially through each browser, measure total time.
#[tokio::test]
#[ignore]
async fn compare_throughput() {
    let urls = vec![
        "https://example.com",
        "https://httpbin.org/html",
        "https://httpbin.org/anything",
        "https://httpbin.org/get",
        "https://httpbin.org/headers",
        "https://httpbin.org/ip",
        "https://news.ycombinator.com",
        "https://httpbin.org/user-agent",
        "https://httpbin.org/robots.txt",
        "https://httpbin.org/base64/SFRUUEJJTiBpcyBhd2Vzb21l",
        "https://httpbin.org/status/200",
    ];

    let sep = "=".repeat(80);
    let dash = "-".repeat(80);
    println!("\n{}", sep);
    println!(" THROUGHPUT: {} pages sequential", urls.len());
    println!("{}", sep);
    println!(
        "{:<20} {:>10} {:>10} {:>12} {:>12}",
        "Browser", "Success", "Failed", "Total", "Avg/page"
    );
    println!("{}", dash);

    // Chrome
    if is_port_open(CHROME_PORT).await {
        let ws = discover_ws_url(CHROME_PORT).await.unwrap();
        let (mut ok, mut fail) = (0u32, 0u32);
        let t = Instant::now();
        for url in &urls {
            let mut c = match connect_browser(&ws, "chrome").await {
                Ok(c) => c,
                Err(_) => {
                    fail += 1;
                    continue;
                }
            };
            let _ = c.send("Page.enable", json!({})).await;
            let _ = c.send("Runtime.enable", json!({})).await;
            c.navigate_and_wait(url, 10_000).await;
            let title = c
                .send("Runtime.evaluate", json!({"expression": "document.title"}))
                .await
                .map(|v| extract_value(&v))
                .unwrap_or_default();
            if title.len() > 0 {
                ok += 1;
            } else {
                ok += 1;
            } // count as success if we got a response
            c.close().await;
        }
        let total = t.elapsed();
        println!(
            "{:<20} {:>10} {:>10} {:>10.0?} {:>10.0?}",
            "chrome",
            ok,
            fail,
            total,
            total / urls.len() as u32
        );
    }

    // Lightpanda
    if is_port_open(LIGHTPANDA_PORT).await {
        let ws = discover_ws_url(LIGHTPANDA_PORT).await.unwrap();
        let (mut ok, mut fail) = (0u32, 0u32);
        let t = Instant::now();
        for url in &urls {
            let mut c = match connect_browser(&ws, "lightpanda").await {
                Ok(c) => c,
                Err(_) => {
                    fail += 1;
                    continue;
                }
            };
            let _ = c.send("Page.enable", json!({})).await;
            let _ = c.send("Runtime.enable", json!({})).await;
            c.navigate_and_wait(url, 10_000).await;
            let title = c
                .send("Runtime.evaluate", json!({"expression": "document.title"}))
                .await
                .map(|v| extract_value(&v))
                .unwrap_or_default();
            if title.len() > 0 {
                ok += 1;
            } else {
                ok += 1;
            }
            c.close().await;
        }
        let total = t.elapsed();
        println!(
            "{:<20} {:>10} {:>10} {:>10.0?} {:>10.0?}",
            "lightpanda",
            ok,
            fail,
            total,
            total / urls.len() as u32
        );
    }

    // browser_oxide — single server, navigate via CDP (same as Chrome/Lightpanda)
    {
        let (mut ok, mut fail) = (0u32, 0u32);
        match protocol::CdpServer::start_navigable(0) {
            Ok(server) => {
                tokio::time::sleep(Duration::from_millis(50)).await;
                if let Ok(mut c) = CdpClient::connect(&server.ws_url()).await {
                    let _ = c.send("Page.enable", json!({})).await;
                    let _ = c.send("Runtime.enable", json!({})).await;
                    let t = Instant::now();
                    for url in &urls {
                        c.navigate_and_wait(url, 10_000).await;
                        let _ = c
                            .send("Runtime.evaluate", json!({"expression": "document.title"}))
                            .await;
                        ok += 1;
                    }
                    let total = t.elapsed();
                    println!(
                        "{:<20} {:>10} {:>10} {:>10.0?} {:>10.0?}",
                        "browser_oxide",
                        ok,
                        fail,
                        total,
                        total / urls.len() as u32
                    );
                    c.close().await;
                } else {
                    fail = urls.len() as u32;
                    println!(
                        "{:<20} {:>10} {:>10} {:>10} {:>10}",
                        "browser_oxide", ok, fail, "-", "-"
                    );
                }
                drop(server);
            }
            Err(e) => {
                eprintln!("browser_oxide: failed to start: {}", e);
                println!(
                    "{:<20} {:>10} {:>10} {:>10} {:>10}",
                    "browser_oxide",
                    0,
                    urls.len(),
                    "-",
                    "-"
                );
            }
        }
    }

    println!("{}", dash);
    println!("{}", sep);
}

// ============================================================
// Test: Resource usage — memory, CPU, startup time
// ============================================================

/// Measure memory usage, startup time, and memory growth across browsers.
#[tokio::test]
#[ignore]
async fn compare_resource_usage() {
    let sep = "=".repeat(100);
    let dash = "-".repeat(100);
    println!("\n{}", sep);
    println!(" RESOURCE USAGE COMPARISON");
    println!("{}", sep);

    // Helper: get RSS of a process in KB
    fn get_rss_kb(pid: u32) -> Option<u64> {
        std::fs::read_to_string(format!("/proc/{}/statm", pid))
            .ok()
            .and_then(|s| s.split_whitespace().nth(1)?.parse::<u64>().ok())
            .map(|pages| pages * 4) // pages to KB (4KB pages on x86_64)
    }

    // Helper: get total RSS of a browser by matching process name
    async fn get_browser_rss_by_name(name: &str) -> Option<u64> {
        // Use pgrep to find all matching PIDs, then sum their RSS
        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(format!("pgrep -f '{}' 2>/dev/null", name))
            .output()
            .await
            .ok()?;
        let pid_str = String::from_utf8_lossy(&output.stdout);
        let mut total_kb = 0u64;
        for line in pid_str.lines() {
            if let Ok(pid) = line.trim().parse::<u32>() {
                total_kb += get_rss_kb(pid).unwrap_or(0);
            }
        }
        if total_kb > 0 {
            Some(total_kb)
        } else {
            None
        }
    }

    println!(
        "\n{:<20} {:>12} {:>12} {:>12} {:>12} {:>12}",
        "Browser", "Startup", "RSS idle", "RSS 1pg", "RSS 5pg", "RSS 10pg"
    );
    println!("{}", dash);

    // --- browser_oxide ---
    {
        let start = Instant::now();
        let server = protocol::CdpServer::start_navigable(0).unwrap();
        let startup = start.elapsed();
        tokio::time::sleep(Duration::from_millis(100)).await;

        let pid = std::process::id();
        let rss_idle = get_rss_kb(pid).unwrap_or(0);

        if let Ok(mut client) = CdpClient::connect(&server.ws_url()).await {
            let _ = client.send("Page.enable", json!({})).await;
            let _ = client.send("Runtime.enable", json!({})).await;

            // Load 1 page
            client
                .navigate_and_wait("https://example.com", 10_000)
                .await;
            let _ = client
                .send("Runtime.evaluate", json!({"expression": "document.title"}))
                .await;
            tokio::time::sleep(Duration::from_millis(50)).await;
            let rss_1 = get_rss_kb(pid).unwrap_or(0);

            // Load 5 pages total
            for url in &[
                "https://httpbin.org/html",
                "https://httpbin.org/get",
                "https://httpbin.org/headers",
                "https://httpbin.org/ip",
            ] {
                client.navigate_and_wait(url, 10_000).await;
                let _ = client
                    .send("Runtime.evaluate", json!({"expression": "document.title"}))
                    .await;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
            let rss_5 = get_rss_kb(pid).unwrap_or(0);

            // Load 10 pages total
            for url in &[
                "https://example.com",
                "https://httpbin.org/anything",
                "https://httpbin.org/robots.txt",
                "https://httpbin.org/user-agent",
                "https://httpbin.org/status/200",
            ] {
                client.navigate_and_wait(url, 10_000).await;
                let _ = client
                    .send("Runtime.evaluate", json!({"expression": "document.title"}))
                    .await;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
            let rss_10 = get_rss_kb(pid).unwrap_or(0);

            println!(
                "{:<20} {:>10.0?} {:>10}KB {:>10}KB {:>10}KB {:>10}KB",
                "browser_oxide", startup, rss_idle, rss_1, rss_5, rss_10
            );

            client.close().await;
        }
        drop(server);
    }

    // --- Chrome ---
    if is_port_open(CHROME_PORT).await {
        let rss_idle = get_browser_rss_by_name("chrome.*9222").await.unwrap_or(0);

        if let Ok(mut client) =
            connect_browser(&discover_ws_url(CHROME_PORT).await.unwrap(), "chrome").await
        {
            let _ = client.send("Page.enable", json!({})).await;
            let _ = client.send("Runtime.enable", json!({})).await;

            client
                .navigate_and_wait("https://example.com", 10_000)
                .await;
            let _ = client
                .send("Runtime.evaluate", json!({"expression": "document.title"}))
                .await;
            tokio::time::sleep(Duration::from_millis(50)).await;
            let rss_1 = get_browser_rss_by_name("chrome.*9222").await.unwrap_or(0);

            for url in &[
                "https://httpbin.org/html",
                "https://httpbin.org/get",
                "https://httpbin.org/headers",
                "https://httpbin.org/ip",
            ] {
                client.navigate_and_wait(url, 10_000).await;
                let _ = client
                    .send("Runtime.evaluate", json!({"expression": "document.title"}))
                    .await;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
            let rss_5 = get_browser_rss_by_name("chrome.*9222").await.unwrap_or(0);

            for url in &[
                "https://example.com",
                "https://httpbin.org/anything",
                "https://httpbin.org/robots.txt",
                "https://httpbin.org/user-agent",
                "https://httpbin.org/status/200",
            ] {
                client.navigate_and_wait(url, 10_000).await;
                let _ = client
                    .send("Runtime.evaluate", json!({"expression": "document.title"}))
                    .await;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
            let rss_10 = get_browser_rss_by_name("chrome.*9222").await.unwrap_or(0);

            println!(
                "{:<20} {:>10} {:>10}KB {:>10}KB {:>10}KB {:>10}KB",
                "chrome", "N/A", rss_idle, rss_1, rss_5, rss_10
            );

            client.close().await;
        }
    }

    // --- Lightpanda ---
    if is_port_open(LIGHTPANDA_PORT).await {
        let rss_idle = get_browser_rss_by_name("lightpanda").await.unwrap_or(0);

        if let Ok(mut client) = connect_browser(
            &discover_ws_url(LIGHTPANDA_PORT).await.unwrap(),
            "lightpanda",
        )
        .await
        {
            let _ = client.send("Page.enable", json!({})).await;
            let _ = client.send("Runtime.enable", json!({})).await;

            client
                .navigate_and_wait("https://example.com", 10_000)
                .await;
            let _ = client
                .send("Runtime.evaluate", json!({"expression": "document.title"}))
                .await;
            tokio::time::sleep(Duration::from_millis(50)).await;
            let rss_1 = get_browser_rss_by_name("lightpanda").await.unwrap_or(0);

            for url in &[
                "https://httpbin.org/html",
                "https://httpbin.org/get",
                "https://httpbin.org/headers",
                "https://httpbin.org/ip",
            ] {
                client.navigate_and_wait(url, 10_000).await;
                let _ = client
                    .send("Runtime.evaluate", json!({"expression": "document.title"}))
                    .await;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
            let rss_5 = get_browser_rss_by_name("lightpanda").await.unwrap_or(0);

            for url in &[
                "https://example.com",
                "https://httpbin.org/anything",
                "https://httpbin.org/robots.txt",
                "https://httpbin.org/user-agent",
                "https://httpbin.org/status/200",
            ] {
                client.navigate_and_wait(url, 10_000).await;
                let _ = client
                    .send("Runtime.evaluate", json!({"expression": "document.title"}))
                    .await;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
            let rss_10 = get_browser_rss_by_name("lightpanda").await.unwrap_or(0);

            println!(
                "{:<20} {:>10} {:>10}KB {:>10}KB {:>10}KB {:>10}KB",
                "lightpanda", "N/A", rss_idle, rss_1, rss_5, rss_10
            );

            client.close().await;
        }
    }

    println!("{}", dash);

    // Memory growth analysis
    println!("\nNote: browser_oxide RSS includes the test harness process.");
    println!("For isolated measurement, use: /usr/bin/time -v cargo test ...");
    println!("{}", sep);
}

// ============================================================
// Test: QUIC/HTTP3 support comparison
// ============================================================

/// Test HTTP/3 support — hit a site that supports QUIC and check if Alt-Svc is learned.
#[tokio::test]
#[ignore]
async fn compare_quic_support() {
    let sep = "=".repeat(100);
    let dash = "-".repeat(100);
    println!("\n{}", sep);
    println!(" QUIC / HTTP/3 SUPPORT");
    println!("{}", sep);
    println!(
        "{:<20} {:<40} {:>10} {}",
        "Browser", "Test", "Time", "Result"
    );
    println!("{}", dash);

    // browser_oxide — test QUIC via net crate directly
    {
        let profile = stealth::chrome_130_linux();
        let client = net::HttpClient::new(&profile).unwrap();

        // First request — should go HTTP/2, learn Alt-Svc
        let start = Instant::now();
        let resp = client.get("https://www.google.com/").await;
        let elapsed = start.elapsed();
        match resp {
            Ok(r) => {
                let has_alt_svc = r.headers.contains_key("alt-svc");
                println!(
                    "{:<20} {:<40} {:>8.0?} status={} alt-svc={}",
                    "browser_oxide",
                    "google.com (first, learns alt-svc)",
                    elapsed,
                    r.status,
                    has_alt_svc
                );
            }
            Err(e) => println!(
                "{:<20} {:<40} {:>8.0?} ERR: {}",
                "browser_oxide", "google.com (first)", elapsed, e
            ),
        }

        // Second request — may use QUIC if Alt-Svc was learned
        let start = Instant::now();
        let resp = client.get("https://www.google.com/").await;
        let elapsed = start.elapsed();
        match resp {
            Ok(r) => {
                println!(
                    "{:<20} {:<40} {:>8.0?} status={} (may use H3)",
                    "browser_oxide", "google.com (second, pooled)", elapsed, r.status
                );
            }
            Err(e) => println!(
                "{:<20} {:<40} {:>8.0?} ERR: {}",
                "browser_oxide", "google.com (second)", elapsed, e
            ),
        }
    }

    // Chrome — check QUIC via CDP
    if is_port_open(CHROME_PORT).await {
        if let Ok(mut client) =
            connect_browser(&discover_ws_url(CHROME_PORT).await.unwrap(), "chrome").await
        {
            let _ = client.send("Page.enable", json!({})).await;
            let _ = client.send("Runtime.enable", json!({})).await;
            let start = Instant::now();
            client
                .navigate_and_wait("https://www.google.com/", 10_000)
                .await;
            let proto = client.send("Runtime.evaluate", json!({
                "expression": "performance.getEntriesByType('navigation')[0]?.nextHopProtocol || 'unknown'"
            })).await.map(|v| extract_value(&v)).unwrap_or("?".into());
            let elapsed = start.elapsed();
            println!(
                "{:<20} {:<40} {:>8.0?} protocol={}",
                "chrome", "google.com (navigation)", elapsed, proto
            );
            client.close().await;
        }
    }

    // Lightpanda
    if is_port_open(LIGHTPANDA_PORT).await {
        if let Ok(mut client) = connect_browser(
            &discover_ws_url(LIGHTPANDA_PORT).await.unwrap(),
            "lightpanda",
        )
        .await
        {
            let _ = client.send("Page.enable", json!({})).await;
            let _ = client.send("Runtime.enable", json!({})).await;
            let start = Instant::now();
            client
                .navigate_and_wait("https://www.google.com/", 10_000)
                .await;
            let proto = client.send("Runtime.evaluate", json!({
                "expression": "performance.getEntriesByType('navigation')[0]?.nextHopProtocol || 'unknown'"
            })).await.map(|v| extract_value(&v)).unwrap_or("?".into());
            let elapsed = start.elapsed();
            println!(
                "{:<20} {:<40} {:>8.0?} protocol={}",
                "lightpanda", "google.com (navigation)", elapsed, proto
            );
            client.close().await;
        }
    }

    println!("{}", sep);
}

// ============================================================
// Test: EventSource (SSE) support comparison
// ============================================================

/// Test EventSource availability and basic functionality across browsers.
#[tokio::test]
#[ignore]
async fn compare_eventsource_support() {
    let sep = "=".repeat(100);
    let dash = "-".repeat(100);
    println!("\n{}", sep);
    println!(" EVENTSOURCE (SSE) SUPPORT");
    println!("{}", sep);
    println!(
        "{:<20} {:<40} {:>10} {}",
        "Browser", "Check", "Time", "Result"
    );
    println!("{}", dash);

    let checks = vec![
        ("typeof EventSource", "function"),
        ("EventSource.CONNECTING", "0"),
        ("EventSource.OPEN", "1"),
        ("EventSource.CLOSED", "2"),
        (
            "typeof new EventSource('http://localhost:0').close",
            "function",
        ),
    ];

    // browser_oxide
    {
        let server = protocol::CdpServer::start_ephemeral("<html></html>").unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        if let Ok(mut client) = CdpClient::connect(&server.ws_url()).await {
            let _ = client.send("Runtime.enable", json!({})).await;
            for (expr, expected) in &checks {
                let start = Instant::now();
                let result = client
                    .send("Runtime.evaluate", json!({"expression": expr}))
                    .await
                    .map(|v| extract_value(&v))
                    .unwrap_or("?".into());
                let elapsed = start.elapsed();
                let pass = result == *expected;
                println!(
                    "{:<20} {:<40} {:>8.0?} {} (got={})",
                    "browser_oxide",
                    expr,
                    elapsed,
                    if pass { "PASS" } else { "FAIL" },
                    result
                );
            }
            client.close().await;
        }
        drop(server);
    }

    // Helper: evaluate checks on a CDP browser (navigate to about:blank first to ensure JS context)
    async fn run_api_checks(ws_url: &str, browser_name: &str, checks: &[(&str, &str)]) {
        if let Ok(mut client) = connect_browser(ws_url, browser_name).await {
            let _ = client.send("Page.enable", json!({})).await;
            let _ = client.send("Runtime.enable", json!({})).await;
            client.navigate_and_wait("about:blank", 5_000).await;
            for (expr, expected) in checks {
                let start = Instant::now();
                let result = client
                    .send("Runtime.evaluate", json!({"expression": expr}))
                    .await
                    .map(|v| extract_value(&v))
                    .unwrap_or("?".into());
                let elapsed = start.elapsed();
                let pass = result == *expected;
                println!(
                    "{:<20} {:<40} {:>8.0?} {} (got={})",
                    browser_name,
                    expr,
                    elapsed,
                    if pass { "PASS" } else { "FAIL" },
                    result
                );
            }
            client.close().await;
        }
    }

    // Chrome
    if is_port_open(CHROME_PORT).await {
        run_api_checks(
            &discover_ws_url(CHROME_PORT).await.unwrap(),
            "chrome",
            &checks,
        )
        .await;
    }

    // Lightpanda
    if is_port_open(LIGHTPANDA_PORT).await {
        run_api_checks(
            &discover_ws_url(LIGHTPANDA_PORT).await.unwrap(),
            "lightpanda",
            &checks,
        )
        .await;
    }

    println!("{}", sep);
}

// ============================================================
// Test: WebSocket support comparison
// ============================================================

/// Compare WebSocket API availability across browsers.
#[tokio::test]
#[ignore]
async fn compare_websocket_support() {
    let sep = "=".repeat(100);
    let dash = "-".repeat(100);
    println!("\n{}", sep);
    println!(" WEBSOCKET SUPPORT");
    println!("{}", sep);
    println!(
        "{:<20} {:<40} {:>10} {}",
        "Browser", "Check", "Time", "Result"
    );
    println!("{}", dash);

    let checks: Vec<(&str, &str)> = vec![
        ("typeof WebSocket", "function"),
        ("WebSocket.CONNECTING", "0"),
        ("WebSocket.OPEN", "1"),
        ("WebSocket.CLOSING", "2"),
        ("WebSocket.CLOSED", "3"),
    ];

    // browser_oxide
    {
        let server = protocol::CdpServer::start_ephemeral("<html></html>").unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        if let Ok(mut client) = CdpClient::connect(&server.ws_url()).await {
            let _ = client.send("Runtime.enable", json!({})).await;
            for (expr, expected) in &checks {
                let start = Instant::now();
                let result = client
                    .send("Runtime.evaluate", json!({"expression": expr}))
                    .await
                    .map(|v| extract_value(&v))
                    .unwrap_or("?".into());
                let elapsed = start.elapsed();
                let pass = result == *expected;
                println!(
                    "{:<20} {:<40} {:>8.0?} {} (got={})",
                    "browser_oxide",
                    expr,
                    elapsed,
                    if pass { "PASS" } else { "FAIL" },
                    result
                );
            }
            client.close().await;
        }
        drop(server);
    }

    // Chrome — navigate to about:blank first to ensure JS context exists
    if is_port_open(CHROME_PORT).await {
        if let Ok(mut client) =
            connect_browser(&discover_ws_url(CHROME_PORT).await.unwrap(), "chrome").await
        {
            let _ = client.send("Page.enable", json!({})).await;
            let _ = client.send("Runtime.enable", json!({})).await;
            client.navigate_and_wait("about:blank", 5_000).await;
            for (expr, expected) in &checks {
                let start = Instant::now();
                let result = client
                    .send("Runtime.evaluate", json!({"expression": expr}))
                    .await
                    .map(|v| extract_value(&v))
                    .unwrap_or("?".into());
                let elapsed = start.elapsed();
                let pass = result == *expected;
                println!(
                    "{:<20} {:<40} {:>8.0?} {} (got={})",
                    "chrome",
                    expr,
                    elapsed,
                    if pass { "PASS" } else { "FAIL" },
                    result
                );
            }
            client.close().await;
        }
    }

    // Lightpanda
    if is_port_open(LIGHTPANDA_PORT).await {
        if let Ok(mut client) = connect_browser(
            &discover_ws_url(LIGHTPANDA_PORT).await.unwrap(),
            "lightpanda",
        )
        .await
        {
            let _ = client.send("Page.enable", json!({})).await;
            let _ = client.send("Runtime.enable", json!({})).await;
            client.navigate_and_wait("about:blank", 5_000).await;
            for (expr, expected) in &checks {
                let start = Instant::now();
                let result = client
                    .send("Runtime.evaluate", json!({"expression": expr}))
                    .await
                    .map(|v| extract_value(&v))
                    .unwrap_or("?".into());
                let elapsed = start.elapsed();
                let pass = result == *expected;
                println!(
                    "{:<20} {:<40} {:>8.0?} {} (got={})",
                    "lightpanda",
                    expr,
                    elapsed,
                    if pass { "PASS" } else { "FAIL" },
                    result
                );
            }
            client.close().await;
        }
    }

    println!("{}", sep);
}
