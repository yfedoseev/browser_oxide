//! MCP (Model Context Protocol) server for browser_oxide.
//!
//! A stealth headless browser an AI agent can drive over stdio JSON-RPC 2.0
//! (no SDK — line-delimited JSON, same shape as the rest of the *_oxide MCPs).
//!
//! Tools:
//!   - `fetch_page`       — render a URL, return html/text + the challenge verdict
//!   - `evaluate`         — render a URL, then run JS in the page realm
//!   - `check_protection` — render a URL, report the anti-bot verdict + vendor tag
//!
//! Wire it into an MCP client (Claude Desktop / Code / Cursor):
//! ```json
//! { "mcpServers": { "browser-oxide": { "command": "browser-oxide-mcp" } } }
//! ```

use browser_oxide::host::{EngineHandle, StealthProfile};
use serde_json::{json, Value};
use std::io::{BufRead, Write};

const SERVER_NAME: &str = "browser-oxide-mcp";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
const PROTOCOL_VERSION: &str = "2024-11-05";

fn main() {
    let engine = EngineHandle::spawn();
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let req: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                write_msg(
                    &mut out,
                    &rpc_error(Value::Null, -32700, &format!("parse error: {e}")),
                );
                continue;
            }
        };

        let id = req.get("id").cloned().unwrap_or(Value::Null);
        let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let params = req.get("params").cloned().unwrap_or(Value::Null);

        match method {
            // Notifications (no id) — acknowledge by staying silent.
            _ if id.is_null() && method.starts_with("notifications/") => {}
            "initialize" => write_msg(&mut out, &rpc_ok(id, initialize_result())),
            "ping" => write_msg(&mut out, &rpc_ok(id, json!({}))),
            "tools/list" => write_msg(&mut out, &rpc_ok(id, tools_list())),
            "tools/call" => {
                let resp = handle_tool_call(&engine, id.clone(), &params);
                write_msg(&mut out, &resp);
            }
            "" => {} // malformed/notification without method
            other => write_msg(
                &mut out,
                &rpc_error(id, -32601, &format!("method not found: {other}")),
            ),
        }
    }
}

fn write_msg(out: &mut impl Write, msg: &Value) {
    let _ = writeln!(out, "{msg}");
    let _ = out.flush();
}

fn rpc_ok(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn initialize_result() -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": { "tools": {} },
        "serverInfo": { "name": SERVER_NAME, "version": SERVER_VERSION }
    })
}

fn tools_list() -> Value {
    let url = json!({ "type": "string", "description": "Absolute URL to load" });
    let profile = json!({
        "type": "string",
        "description": "Browser identity: chrome (default), firefox, iphone, pixel",
        "enum": ["chrome", "firefox", "iphone", "pixel"]
    });
    json!({ "tools": [
        {
            "name": "fetch_page",
            "description": "Render a URL with a real stealth browser engine (own TLS + V8, no Chromium) and return its content. Returns the honest challenge verdict so you know if the body is real content vs. a bot wall.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "url": url,
                    "profile": profile,
                    "format": { "type": "string", "enum": ["text", "html"], "description": "text (default) or full html" }
                },
                "required": ["url"]
            }
        },
        {
            "name": "evaluate",
            "description": "Render a URL, then run a JavaScript expression in the page realm and return its string result.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "url": url,
                    "js": { "type": "string", "description": "JavaScript to evaluate in the page" },
                    "profile": profile
                },
                "required": ["url", "js"]
            }
        },
        {
            "name": "check_protection",
            "description": "Render a URL and report whether it is behind an anti-bot wall and whether a real render got through. Returns the verdict (pass / thin-shell / edge-block / sensor-fail / challenge-incomplete), whether it is a challenge, and the rendered byte size.",
            "inputSchema": {
                "type": "object",
                "properties": { "url": url, "profile": profile },
                "required": ["url"]
            }
        }
    ]})
}

fn profile_from_name(name: Option<&str>) -> StealthProfile {
    use browser_oxide::host::stealth::presets;
    match name.unwrap_or("chrome") {
        "firefox" => presets::firefox_135_macos(),
        "iphone" => presets::iphone_15_pro_safari_18(),
        "pixel" => presets::pixel_9_pro_chrome_148(),
        _ => presets::chrome_148_macos(),
    }
}

/// Wrap a tool's text payload in the MCP `content` envelope.
fn tool_text(id: Value, text: String) -> Value {
    rpc_ok(id, json!({ "content": [{ "type": "text", "text": text }] }))
}

fn tool_err(id: Value, msg: String) -> Value {
    rpc_ok(
        id,
        json!({ "content": [{ "type": "text", "text": msg }], "isError": true }),
    )
}

fn handle_tool_call(engine: &EngineHandle, id: Value, params: &Value) -> Value {
    let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or(Value::Null);
    let url = args.get("url").and_then(|u| u.as_str());
    let profile = profile_from_name(args.get("profile").and_then(|p| p.as_str()));

    let Some(url) = url else {
        return tool_err(id, "missing required argument: url".into());
    };

    match name {
        "fetch_page" => {
            let fmt = args
                .get("format")
                .and_then(|f| f.as_str())
                .unwrap_or("text");
            match engine.navigate(url, profile, 5) {
                Ok(s) => {
                    let body = if fmt == "html" { s.html } else { s.text };
                    tool_text(
                        id,
                        format!(
                            "url: {}\ntitle: {}\nverdict: {}\nbytes: {}\n\n{}",
                            s.url,
                            s.title,
                            s.verdict,
                            body.len(),
                            body
                        ),
                    )
                }
                Err(e) => tool_err(id, format!("navigation failed: {e}")),
            }
        }
        "evaluate" => {
            let Some(js) = args.get("js").and_then(|j| j.as_str()) else {
                return tool_err(id, "missing required argument: js".into());
            };
            if let Err(e) = engine.navigate(url, profile, 5) {
                return tool_err(id, format!("navigation failed: {e}"));
            }
            match engine.evaluate(js) {
                Ok(r) => tool_text(id, r),
                Err(e) => tool_err(id, format!("evaluate failed: {e}")),
            }
        }
        "check_protection" => match engine.navigate(url, profile, 5) {
            Ok(s) => tool_text(
                id,
                json!({
                    "url": s.url,
                    "verdict": s.verdict,
                    "is_challenge": s.is_challenge,
                    "rendered_bytes": s.html.len(),
                    "title": s.title,
                })
                .to_string(),
            ),
            Err(e) => tool_err(id, format!("navigation failed: {e}")),
        },
        other => rpc_error(id, -32602, &format!("unknown tool: {other}")),
    }
}
