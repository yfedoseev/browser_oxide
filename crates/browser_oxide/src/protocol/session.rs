use crate::protocol::types::*;
use crate::Page;
use std::collections::HashSet;
use std::time::SystemTime;

/// Per-connection CDP session state.
pub struct CdpSession {
    pub enabled_domains: HashSet<String>,
    pub scripts_on_new_document: Vec<String>,
    pub frame_id: String,
    loader_counter: u64,
    request_counter: u64,
    extra_headers: std::collections::HashMap<String, String>,
    request_interception_enabled: bool,
    /// Set by Page.navigate — the server handles the actual page replacement
    /// because V8 isolates must be dropped in LIFO order.
    pub pending_navigate: Option<String>,
    /// Last known mouse coordinates for trajectory generation.
    last_mouse_x: f32,
    last_mouse_y: f32,
    /// Behavioral profile for input humanization.
    behavior: crate::stealth::behavior::BehaviorProfile,
}

impl Default for CdpSession {
    fn default() -> Self {
        Self::new()
    }
}

impl CdpSession {
    pub fn new() -> Self {
        Self {
            enabled_domains: HashSet::new(),
            scripts_on_new_document: Vec::new(),
            frame_id: "main".to_string(),
            loader_counter: 0,
            request_counter: 0,
            extra_headers: std::collections::HashMap::new(),
            request_interception_enabled: false,
            pending_navigate: None,
            last_mouse_x: 0.0,
            last_mouse_y: 0.0,
            behavior: crate::stealth::behavior::BehaviorProfile::default(),
        }
    }

    pub fn next_request_id(&mut self) -> String {
        self.request_counter += 1;
        format!("{}.1", self.request_counter)
    }

    pub fn next_loader_id(&mut self) -> String {
        self.loader_counter += 1;
        format!("loader-{}", self.loader_counter)
    }

    pub fn is_domain_enabled(&self, domain: &str) -> bool {
        self.enabled_domains.contains(domain)
    }

    pub fn enable_domain(&mut self, domain: &str) {
        self.enabled_domains.insert(domain.to_string());
    }

    /// Handle a CDP request and return response + events to emit.
    pub async fn handle_request(
        &mut self,
        page: &mut Page,
        req: &CdpRequest,
        http_client: Option<&crate::net::HttpClient>,
    ) -> (String, Vec<CdpEvent>) {
        // Fast path: Runtime.evaluate is the hot path — handle before allocating events Vec
        if req.method == "Runtime.evaluate" {
            let expression = req
                .params
                .get("expression")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            return match page.evaluate(expression) {
                Ok(result_str) => {
                    let ty = js_type(&result_str);
                    let return_by_value = req
                        .params
                        .get("returnByValue")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);

                    let resp_json = if return_by_value {
                        let raw_value = match ty {
                            "number" | "boolean" => result_str.clone(),
                            "undefined" => "null".to_string(),
                            _ => {
                                if serde_json::from_str::<serde_json::Value>(&result_str).is_ok() {
                                    result_str.clone()
                                } else {
                                    json_escape_string(&result_str)
                                }
                            }
                        };
                        format!(
                            r#"{{"id":{},"result":{{"result":{{"type":"{}","value":{}}}}}}}"#,
                            req.id, ty, raw_value
                        )
                    } else {
                        format!(
                            r#"{{"id":{},"result":{{"type":"{}","value":{}}}}}"#,
                            req.id,
                            ty,
                            json_escape_string(&result_str)
                        )
                    };
                    (resp_json, Vec::new())
                }
                Err(e) => {
                    let resp = to_json(&serde_json::json!({
                        "id": req.id,
                        "result": {
                            "exceptionDetails": {
                                "text": e.to_string(),
                                "lineNumber": 0,
                                "columnNumber": 0,
                            }
                        }
                    }));
                    (resp, Vec::new())
                }
            };
        }

        let mut events = Vec::new();

        let result: Result<serde_json::Value, String> = match req.method.as_str() {
            // --- Target domain ---
            "Target.getTargets" => Ok(serde_json::json!({
                "targetInfos": [{
                    "targetId": "page-1",
                    "type": "page",
                    "title": page.title(),
                    "url": page.url(),
                    "attached": true,
                }]
            })),

            // --- Page domain ---
            "Page.enable" => {
                self.enable_domain("Page");
                Ok(serde_json::json!({}))
            }
            "Page.navigate" => {
                let url = req
                    .params
                    .get("url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("about:blank");
                let loader_id = self.next_loader_id();

                // Signal the server to handle navigation after we release the page borrow.
                // V8 requires isolates to be dropped in LIFO order, so page replacement
                // must happen at the server level where we control the RefCell.
                if url != "about:blank" && http_client.is_some() {
                    self.pending_navigate = Some(url.to_string());
                }

                // Emit lifecycle events
                if self.is_domain_enabled("Page") {
                    events.push(CdpEvent::new(
                        "Page.frameNavigated",
                        serde_json::json!({
                            "frame": {
                                "id": self.frame_id,
                                "loaderId": loader_id,
                                "url": url,
                                "securityOrigin": url,
                                "mimeType": "text/html",
                            }
                        }),
                    ));
                    events.push(CdpEvent::new(
                        "Page.domContentEventFired",
                        serde_json::json!({
                            "timestamp": timestamp()
                        }),
                    ));
                    events.push(CdpEvent::new(
                        "Page.loadEventFired",
                        serde_json::json!({
                            "timestamp": timestamp()
                        }),
                    ));
                }

                Ok(serde_json::json!({
                    "frameId": self.frame_id,
                    "loaderId": loader_id,
                }))
            }
            "Page.getFrameTree" => Ok(serde_json::json!({
                "frameTree": {
                    "frame": {
                        "id": self.frame_id,
                        "loaderId": format!("loader-{}", self.loader_counter),
                        "url": page.url(),
                        "securityOrigin": page.url(),
                        "mimeType": "text/html",
                    },
                    "childFrames": []
                }
            })),
            "Page.addScriptToEvaluateOnNewDocument" => {
                let source = req
                    .params
                    .get("source")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                self.scripts_on_new_document.push(source.to_string());
                Ok(serde_json::json!({
                    "identifier": format!("script-{}", self.scripts_on_new_document.len())
                }))
            }
            "Page.setLifecycleEventsEnabled" => Ok(serde_json::json!({})),
            "Page.createIsolatedWorld" => Ok(serde_json::json!({ "executionContextId": 2 })),

            // --- Runtime domain ---
            "Runtime.enable" => {
                self.enable_domain("Runtime");
                events.push(CdpEvent::new("Runtime.executionContextCreated", serde_json::json!({
                    "context": {
                        "id": 1,
                        "origin": page.url(),
                        "name": "",
                        "auxData": { "isDefault": true, "type": "default", "frameId": self.frame_id }
                    }
                })));
                Ok(serde_json::json!({}))
            }
            // Runtime.evaluate handled in fast path above
            "Runtime.callFunctionOn" => {
                let decl = req
                    .params
                    .get("functionDeclaration")
                    .and_then(|v| v.as_str())
                    .unwrap_or("() => undefined");
                let code = format!("({})()", decl);
                match page.evaluate(&code) {
                    Ok(result_str) => Ok(serde_json::json!({
                        "result": { "type": js_type(&result_str), "value": result_str }
                    })),
                    Err(e) => Ok(serde_json::json!({
                        "exceptionDetails": { "text": e.to_string() }
                    })),
                }
            }
            "Runtime.disable" => Ok(serde_json::json!({})),
            "Runtime.runIfWaitingForDebugger" => Ok(serde_json::json!({})),

            // --- DOM domain ---
            "DOM.enable" => {
                self.enable_domain("DOM");
                Ok(serde_json::json!({}))
            }
            "DOM.getDocument" => {
                // `depth` parameter is honored by real Chrome to bound the
                // returned subtree; we currently return a minimal fixed-
                // depth document, so the value is read for spec
                // compatibility but not yet acted on.
                let _depth = req
                    .params
                    .get("depth")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(1);
                // Return minimal document structure
                Ok(serde_json::json!({
                    "root": {
                        "nodeId": 1,
                        "backendNodeId": 1,
                        "nodeType": 9,
                        "nodeName": "#document",
                        "localName": "",
                        "nodeValue": "",
                        "childNodeCount": 1,
                        "documentURL": page.url(),
                        "baseURL": page.url(),
                    }
                }))
            }
            "DOM.querySelector" => {
                let selector = req
                    .params
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let has = page.has_element(selector);
                Ok(serde_json::json!({
                    "nodeId": if has { 2 } else { 0 }
                }))
            }
            "DOM.querySelectorAll" => {
                let selector = req
                    .params
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                // Count matches via JS
                let count_js = format!(
                    "document.querySelectorAll(\"{}\").length",
                    selector.replace('\\', "\\\\").replace('"', "\\\"")
                );
                let count: usize = page
                    .evaluate(&count_js)
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
                let node_ids: Vec<u32> = (0..count).map(|i| (i + 2) as u32).collect();
                Ok(serde_json::json!({ "nodeIds": node_ids }))
            }
            "DOM.getOuterHTML" => {
                let result = page
                    .evaluate("document.documentElement.outerHTML")
                    .unwrap_or_default();
                Ok(serde_json::json!({ "outerHTML": result }))
            }
            "DOM.disable" => Ok(serde_json::json!({})),

            // --- Network domain ---
            "Network.enable" => {
                self.enable_domain("Network");
                Ok(serde_json::json!({}))
            }
            "Network.getCookies" => Ok(serde_json::json!({ "cookies": [] })),
            "Network.setCookies" => Ok(serde_json::json!({})),
            "Network.disable" => {
                self.enabled_domains.remove("Network");
                Ok(serde_json::json!({}))
            }
            "Network.setExtraHTTPHeaders" => {
                if let Some(headers) = req.params.get("headers").and_then(|h| h.as_object()) {
                    for (k, v) in headers {
                        if let Some(val) = v.as_str() {
                            self.extra_headers.insert(k.clone(), val.to_string());
                        }
                    }
                }
                Ok(serde_json::json!({}))
            }
            "Network.setRequestInterception" => {
                self.request_interception_enabled = req
                    .params
                    .get("enabled")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                Ok(serde_json::json!({}))
            }
            "Network.getResponseBody" => {
                // Stub — return empty body
                Ok(serde_json::json!({
                    "body": "",
                    "base64Encoded": false,
                }))
            }
            "Network.clearBrowserCache" => Ok(serde_json::json!({})),
            "Network.clearBrowserCookies" => Ok(serde_json::json!({})),
            "Network.setCacheDisabled" => Ok(serde_json::json!({})),
            "Network.emulateNetworkConditions" => Ok(serde_json::json!({})),

            // --- Emulation domain ---
            "Emulation.setDeviceMetricsOverride" => Ok(serde_json::json!({})),
            "Emulation.setUserAgentOverride" => Ok(serde_json::json!({})),
            "Emulation.setTouchEmulationEnabled" => Ok(serde_json::json!({})),

            // --- Browser domain ---
            "Browser.getVersion" => Ok(serde_json::json!({
                "protocolVersion": "1.3",
                "product": "browser_oxide/0.1.0",
                "revision": "0",
                "userAgent": "Mozilla/5.0 browser_oxide/0.1.0",
                "jsVersion": "V8",
            })),

            // --- Log domain ---
            "Log.enable" => Ok(serde_json::json!({})),
            "Log.disable" => Ok(serde_json::json!({})),

            // --- Inspector domain ---
            "Inspector.enable" => Ok(serde_json::json!({})),

            // --- Performance domain ---
            "Performance.enable" => Ok(serde_json::json!({})),

            // --- Security domain ---
            "Security.enable" => Ok(serde_json::json!({})),

            // --- Input domain ---
            //
            // Puppeteer/Playwright drive the page via Input.dispatch* CDP
            // methods. Without these handlers, every user interaction script
            // gets "method not found" — full incompatibility. We translate
            // each CDP call into a JS-side event dispatch via page.evaluate.
            //
            // Mouse-path humanization is offered separately via the JS
            // helper `__browserOxide.humanMousePath` (wired to
            // crate::stealth::behavior::mouse_trajectory). Callers that want
            // humanized input call multiple Input.dispatchMouseEvent
            // moves between actions; CDP itself stays event-faithful.
            "Input.dispatchMouseEvent" => {
                let event_type = req
                    .params
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let x = req.params.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                let y = req.params.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                let button = req
                    .params
                    .get("button")
                    .and_then(|v| v.as_str())
                    .unwrap_or("none");
                let buttons = req
                    .params
                    .get("buttons")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                let click_count = req
                    .params
                    .get("clickCount")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(1);
                let modifiers = req
                    .params
                    .get("modifiers")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);

                let js_event = match event_type {
                    "mousePressed" => "mousedown",
                    "mouseReleased" => "mouseup",
                    "mouseMoved" => "mousemove",
                    _ => "",
                };

                if !js_event.is_empty() {
                    let button_n = match button {
                        "left" => 0,
                        "middle" => 1,
                        "right" => 2,
                        "back" => 3,
                        "forward" => 4,
                        _ => 0,
                    };

                    let dx = x - self.last_mouse_x;
                    let dy = y - self.last_mouse_y;
                    let dist = (dx * dx + dy * dy).sqrt();

                    // For 'mouseMoved' with significant distance, generate human trajectory.
                    if event_type == "mouseMoved" && dist > 10.0 {
                        let pts = crate::stealth::behavior::mouse_trajectory(
                            (self.last_mouse_x, self.last_mouse_y),
                            (x, y),
                            30.0, // assumption: target width is 30px if not specified
                            &self.behavior,
                        );

                        for (i, p) in pts.iter().enumerate() {
                            let script = format!(
                                "(() => {{ \
                                  const props = {{ \
                                    bubbles: true, cancelable: true, composed: true, \
                                    clientX: {x}, clientY: {y}, screenX: {x}, screenY: {y}, \
                                    button: {button_n}, buttons: {buttons}, detail: {click_count}, \
                                    ctrlKey: {ctrl}, shiftKey: {shift}, altKey: {alt}, metaKey: {meta}, \
                                    pointerId: 1, width: 1, height: 1, pressure: {pressure}, \
                                    pointerType: 'mouse', isPrimary: true \
                                  }}; \
                                  const e = new PointerEvent('pointermove', props); \
                                  const m = new MouseEvent('mousemove', props); \
                                  const t = (document.elementFromPoint && document.elementFromPoint({x},{y})) || document.body || document; \
                                  t.dispatchEvent(e); \
                                  t.dispatchEvent(m); \
                                }})()",
                                x = p.x,
                                y = p.y,
                                pressure = if buttons != 0 { 0.5 } else { 0.0 },
                                ctrl = (modifiers & 2) != 0,
                                shift = (modifiers & 8) != 0,
                                alt = (modifiers & 1) != 0,
                                meta = (modifiers & 4) != 0,
                            );
                            let _ = page.evaluate(&script);

                            // Real-time delay between points (8ms sample rate in model)
                            if i < pts.len() - 1 {
                                tokio::time::sleep(std::time::Duration::from_millis(8)).await;
                            }
                        }
                    } else {
                        // Single jump for clicks or small moves
                        let pointer_type = match event_type {
                            "mousePressed" => "pointerdown",
                            "mouseReleased" => "pointerup",
                            "mouseMoved" => "pointermove",
                            _ => "pointermove",
                        };
                        let script = format!(
                            "(() => {{ \
                              const props = {{ \
                                bubbles: true, cancelable: true, composed: true, \
                                clientX: {x}, clientY: {y}, screenX: {x}, screenY: {y}, \
                                button: {button_n}, buttons: {buttons}, detail: {click_count}, \
                                ctrlKey: {ctrl}, shiftKey: {shift}, altKey: {alt}, metaKey: {meta}, \
                                pointerId: 1, width: 1, height: 1, pressure: {pressure}, \
                                pointerType: 'mouse', isPrimary: true \
                              }}; \
                              const e = new PointerEvent({pointer_type:?}, props); \
                              const m = new MouseEvent({js_event:?}, props); \
                              const t = (document.elementFromPoint && document.elementFromPoint({x},{y})) || document.body || document; \
                              t.dispatchEvent(e); \
                              t.dispatchEvent(m); \
                            }})()",
                            pressure = if event_type == "mousePressed" || buttons != 0 { 0.5 } else { 0.0 },
                            ctrl = (modifiers & 2) != 0,
                            shift = (modifiers & 8) != 0,
                            alt = (modifiers & 1) != 0,
                            meta = (modifiers & 4) != 0,
                        );
                        let _ = page.evaluate(&script);
                    }

                    self.last_mouse_x = x;
                    self.last_mouse_y = y;
                }
                Ok(serde_json::json!({}))
            }
            "Input.dispatchKeyEvent" => {
                let event_type = req
                    .params
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let key = req.params.get("key").and_then(|v| v.as_str()).unwrap_or("");
                let code = req
                    .params
                    .get("code")
                    .and_then(|v| v.as_str())
                    .unwrap_or(key);
                let text = req
                    .params
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let modifiers = req
                    .params
                    .get("modifiers")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                let js_event = match event_type {
                    "keyDown" | "rawKeyDown" => "keydown",
                    "keyUp" => "keyup",
                    "char" => "keypress",
                    _ => "",
                };
                if !js_event.is_empty() {
                    let script = format!(
                        "(() => {{ \
                          const e = new KeyboardEvent({js_event:?}, {{ \
                            bubbles: true, cancelable: true, \
                            key: {key:?}, code: {code:?}, \
                            ctrlKey: {ctrl}, shiftKey: {shift}, altKey: {alt}, metaKey: {meta} \
                          }}); \
                          (document.activeElement || document.body || document).dispatchEvent(e); \
                          // For 'char' events, also fire an input event so text fields update.
                          {input_extra} \
                        }})()",
                        ctrl = (modifiers & 2) != 0,
                        shift = (modifiers & 8) != 0,
                        alt = (modifiers & 1) != 0,
                        meta = (modifiers & 4) != 0,
                        input_extra = if event_type == "char" && !text.is_empty() {
                            format!(
                                "const ae = document.activeElement; \
                                 if (ae && ('value' in ae)) {{ ae.value = (ae.value || '') + {text:?}; \
                                 ae.dispatchEvent(new Event('input', {{bubbles: true}})); }}"
                            )
                        } else {
                            String::new()
                        },
                    );
                    let _ = page.evaluate(&script);
                }
                Ok(serde_json::json!({}))
            }
            "Input.dispatchTouchEvent" => Ok(serde_json::json!({})),
            "Input.insertText" => {
                let text = req
                    .params
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if !text.is_empty() {
                    let timings = crate::stealth::behavior::keystroke_timings(text, &self.behavior);
                    for (i, t) in timings.iter().enumerate() {
                        // Flight time (delay before this key)
                        if i > 0 {
                            tokio::time::sleep(std::time::Duration::from_millis(
                                t.flight_ms as u64,
                            ))
                            .await;
                        }

                        // Fire keydown
                        let kd_script = format!(
                            "(() => {{ \
                              const e = new KeyboardEvent('keydown', {{ \
                                bubbles: true, cancelable: true, key: {ch:?}, code: 'Key' + {ch:?}.toUpperCase() \
                              }}); \
                              (document.activeElement || document.body || document).dispatchEvent(e); \
                            }})()",
                            ch = t.ch
                        );
                        let _ = page.evaluate(&kd_script);

                        // Dwell time (delay while key is down)
                        tokio::time::sleep(std::time::Duration::from_millis(t.dwell_ms as u64))
                            .await;

                        // Insert character + fire 'input'
                        let script = format!(
                            "(() => {{ const ae = document.activeElement; \
                             if (ae && ('value' in ae)) {{ ae.value = (ae.value || '') + {text:?}; \
                             ae.dispatchEvent(new Event('input', {{bubbles: true}})); }} }})()",
                            text = t.ch.to_string()
                        );
                        let _ = page.evaluate(&script);

                        // Fire keyup
                        let ku_script = format!(
                            "(() => {{ \
                              const e = new KeyboardEvent('keyup', {{ \
                                bubbles: true, cancelable: true, key: {ch:?}, code: 'Key' + {ch:?}.toUpperCase() \
                              }}); \
                              (document.activeElement || document.body || document).dispatchEvent(e); \
                            }})()",
                            ch = t.ch
                        );
                        let _ = page.evaluate(&ku_script);
                    }
                }
                Ok(serde_json::json!({}))
            }
            "Input.setIgnoreInputEvents" => Ok(serde_json::json!({})),

            // Unknown method
            _ => {
                let err = CdpError::method_not_found(req.id, &req.method);
                return (to_json(&err), events);
            }
        };

        match result {
            Ok(value) => (to_json(&CdpResponse::ok(req.id, value)), events),
            Err(msg) => (to_json(&CdpError::internal(req.id, &msg)), events),
        }
    }
}

/// Fast JSON string escaping — avoids serde_json::to_string overhead.
pub fn json_escape_string(s: &str) -> String {
    // Fast path: if no special chars, just quote it
    if s.bytes().all(|b| b > 31 && b != b'"' && b != b'\\') {
        let mut out = String::with_capacity(s.len() + 2);
        out.push('"');
        out.push_str(s);
        out.push('"');
        return out;
    }
    // Slow path: escape special characters
    let mut out = String::with_capacity(s.len() + 8);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 32 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn timestamp() -> f64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

fn js_type(value: &str) -> &'static str {
    match value {
        "undefined" => "undefined",
        "null" => "object",
        "true" | "false" => "boolean",
        s if s.parse::<f64>().is_ok() => "number",
        _ => "string",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn handle_page_enable() {
        let mut session = CdpSession::new();
        let mut page = Page::from_html("<html><head></head><body></body></html>", None)
            .await
            .unwrap();
        let req = CdpRequest {
            id: 1,
            method: "Page.enable".to_string(),
            params: serde_json::Value::Null,
        };
        let (resp, _events) = session.handle_request(&mut page, &req, None).await;
        assert!(resp.contains("\"id\":1"));
        assert!(session.is_domain_enabled("Page"));
    }

    #[tokio::test]
    async fn handle_runtime_evaluate() {
        let mut session = CdpSession::new();
        let mut page = Page::from_html("<html><head></head><body></body></html>", None)
            .await
            .unwrap();
        let req = CdpRequest {
            id: 2,
            method: "Runtime.evaluate".to_string(),
            params: serde_json::json!({"expression": "1 + 2"}),
        };
        let (resp, _) = session.handle_request(&mut page, &req, None).await;
        assert!(resp.contains("3"), "response: {}", resp);
    }

    #[tokio::test]
    async fn handle_runtime_enable_emits_context() {
        let mut session = CdpSession::new();
        let mut page = Page::from_html("<html><head></head><body></body></html>", None)
            .await
            .unwrap();
        let req = CdpRequest {
            id: 3,
            method: "Runtime.enable".to_string(),
            params: serde_json::Value::Null,
        };
        let (_, events) = session.handle_request(&mut page, &req, None).await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].method, "Runtime.executionContextCreated");
    }

    #[tokio::test]
    async fn handle_page_navigate() {
        let mut session = CdpSession::new();
        session.enable_domain("Page");
        let mut page = Page::from_html("<html><head></head><body></body></html>", None)
            .await
            .unwrap();
        let req = CdpRequest {
            id: 4,
            method: "Page.navigate".to_string(),
            params: serde_json::json!({"url": "about:blank"}),
        };
        let (resp, events) = session.handle_request(&mut page, &req, None).await;
        assert!(resp.contains("frameId"));
        assert!(resp.contains("loaderId"));
        // Should emit frameNavigated + domContentEventFired + loadEventFired
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].method, "Page.frameNavigated");
        assert_eq!(events[1].method, "Page.domContentEventFired");
        assert_eq!(events[2].method, "Page.loadEventFired");
    }

    #[tokio::test]
    async fn handle_dom_get_document() {
        let mut session = CdpSession::new();
        let mut page = Page::from_html("<html><head></head><body></body></html>", None)
            .await
            .unwrap();
        let req = CdpRequest {
            id: 5,
            method: "DOM.getDocument".to_string(),
            params: serde_json::json!({}),
        };
        let (resp, _) = session.handle_request(&mut page, &req, None).await;
        assert!(resp.contains("\"nodeType\":9"));
        assert!(resp.contains("#document"));
    }

    #[tokio::test]
    async fn handle_dom_query_selector() {
        let mut session = CdpSession::new();
        let mut page = Page::from_html(
            "<html><head></head><body><div id='test'></div></body></html>",
            None,
        )
        .await
        .unwrap();
        let req = CdpRequest {
            id: 6,
            method: "DOM.querySelector".to_string(),
            params: serde_json::json!({"nodeId": 1, "selector": "#test"}),
        };
        let (resp, _) = session.handle_request(&mut page, &req, None).await;
        assert!(resp.contains("\"nodeId\":2"), "response: {}", resp); // found
    }

    #[tokio::test]
    async fn handle_unknown_method() {
        let mut session = CdpSession::new();
        let mut page = Page::from_html("<html><head></head><body></body></html>", None)
            .await
            .unwrap();
        let req = CdpRequest {
            id: 99,
            method: "Unknown.method".to_string(),
            params: serde_json::Value::Null,
        };
        let (resp, _) = session.handle_request(&mut page, &req, None).await;
        assert!(resp.contains("-32601"), "response: {}", resp);
        assert!(resp.contains("Unknown.method"));
    }

    #[tokio::test]
    async fn handle_browser_get_version() {
        let mut session = CdpSession::new();
        let mut page = Page::from_html("<html><head></head><body></body></html>", None)
            .await
            .unwrap();
        let req = CdpRequest {
            id: 7,
            method: "Browser.getVersion".to_string(),
            params: serde_json::Value::Null,
        };
        let (resp, _) = session.handle_request(&mut page, &req, None).await;
        assert!(resp.contains("browser_oxide"));
    }

    #[tokio::test]
    async fn handle_add_script_on_new_document() {
        let mut session = CdpSession::new();
        let mut page = Page::from_html("<html><head></head><body></body></html>", None)
            .await
            .unwrap();
        let req = CdpRequest {
            id: 8,
            method: "Page.addScriptToEvaluateOnNewDocument".to_string(),
            params: serde_json::json!({"source": "window.__test = true;"}),
        };
        let (resp, _) = session.handle_request(&mut page, &req, None).await;
        assert!(resp.contains("identifier"));
        assert_eq!(session.scripts_on_new_document.len(), 1);
    }
}
