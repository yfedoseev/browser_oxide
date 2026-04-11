//! Server-Sent Events (SSE) support for EventSource API.
//!
//! Connects to SSE endpoints, parses the `text/event-stream` format,
//! and delivers events to JavaScript via async ops.

use deno_core::op2;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

lazy_static::lazy_static! {
    static ref SSE_CONNECTIONS: Arc<Mutex<SseStore>> = Arc::new(Mutex::new(SseStore::new()));
}

struct SseStore {
    receivers: HashMap<i32, Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<SseEvent>>>>,
    next_id: i32,
}

impl SseStore {
    fn new() -> Self {
        Self {
            receivers: HashMap::new(),
            next_id: 1,
        }
    }
}

/// SSE connection state (empty — connections stored globally).
pub struct SseState;

impl SseState {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct SseEvent {
    pub event: String,
    pub data: String,
    pub id: String,
    /// Empty string = still open, "error" = connection error, "closed" = done
    pub status: String,
}

#[derive(Serialize)]
pub struct SseConnectResult {
    pub id: i32,
    pub ok: bool,
    pub error: String,
}

/// Connect to an SSE endpoint. Returns a connection ID.
#[op2(async)]
#[serde]
pub async fn op_sse_connect(
    #[string] url: String,
) -> Result<SseConnectResult, deno_core::error::AnyError> {
    let (tx, rx) = mpsc::unbounded_channel::<SseEvent>();

    // Assign ID before spawning
    let id = {
        let mut store = SSE_CONNECTIONS.lock().unwrap();
        let id = store.next_id;
        store.next_id += 1;
        store
            .receivers
            .insert(id, Arc::new(tokio::sync::Mutex::new(rx)));
        id
    };

    // Spawn the SSE reader task
    let url_clone = url.clone();
    tokio::spawn(async move {
        if let Err(e) = sse_reader(&url_clone, &tx).await {
            let _ = tx.send(SseEvent {
                event: "error".to_string(),
                data: e.to_string(),
                id: String::new(),
                status: "error".to_string(),
            });
        }
        // Signal end of stream
        let _ = tx.send(SseEvent {
            event: String::new(),
            data: String::new(),
            id: String::new(),
            status: "closed".to_string(),
        });
    });

    Ok(SseConnectResult {
        id,
        ok: true,
        error: String::new(),
    })
}

/// Read the next SSE event from a connection.
#[op2(async)]
#[serde]
pub async fn op_sse_recv(#[smi] id: i32) -> Result<SseEvent, deno_core::error::AnyError> {
    let rx = {
        let store = SSE_CONNECTIONS.lock().unwrap();
        store.receivers.get(&id).cloned()
    };
    match rx {
        Some(rx) => {
            let mut rx = rx.lock().await;
            match rx.recv().await {
                Some(event) => Ok(event),
                None => Ok(SseEvent {
                    event: String::new(),
                    data: String::new(),
                    id: String::new(),
                    status: "closed".to_string(),
                }),
            }
        }
        None => Ok(SseEvent {
            event: String::new(),
            data: String::new(),
            id: String::new(),
            status: "closed".to_string(),
        }),
    }
}

/// Close an SSE connection.
#[op2(fast)]
pub fn op_sse_close(#[smi] id: i32) {
    let mut store = SSE_CONNECTIONS.lock().unwrap();
    store.receivers.remove(&id);
}

/// Parse and stream SSE events from an HTTP response body.
async fn sse_reader(
    url: &str,
    tx: &mpsc::UnboundedSender<SseEvent>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Use the stealth HTTP client to fetch the SSE endpoint
    let profile = stealth::chrome_130_linux();
    let client =
        net::HttpClient::new(&profile).map_err(|e| format!("failed to create HTTP client: {e}"))?;
    let resp = client
        .get(url)
        .await
        .map_err(|e| format!("SSE fetch failed: {e}"))?;

    let body = resp.text();
    parse_sse_body(&body, tx);
    Ok(())
}

/// Parse SSE text/event-stream body into events.
fn parse_sse_body(body: &str, tx: &mpsc::UnboundedSender<SseEvent>) {
    let mut event_type = String::new();
    let mut data_buf = String::new();
    let mut last_id = String::new();

    for line in body.lines() {
        if line.is_empty() {
            // Empty line = dispatch event
            if !data_buf.is_empty() {
                if data_buf.ends_with('\n') {
                    data_buf.pop();
                }
                let event = SseEvent {
                    event: if event_type.is_empty() {
                        "message".to_string()
                    } else {
                        std::mem::take(&mut event_type)
                    },
                    data: std::mem::take(&mut data_buf),
                    id: last_id.clone(),
                    status: String::new(),
                };
                if tx.send(event).is_err() {
                    return;
                }
            }
            event_type.clear();
            continue;
        }

        if line.starts_with(':') {
            continue; // Comment
        }

        let (field, value) = if let Some(colon_pos) = line.find(':') {
            let field = &line[..colon_pos];
            let value = line[colon_pos + 1..]
                .strip_prefix(' ')
                .unwrap_or(&line[colon_pos + 1..]);
            (field, value)
        } else {
            (line, "")
        };

        match field {
            "event" => event_type = value.to_string(),
            "data" => {
                data_buf.push_str(value);
                data_buf.push('\n');
            }
            "id" => last_id = value.to_string(),
            "retry" => {}
            _ => {}
        }
    }
}

deno_core::extension!(
    sse_extension,
    ops = [op_sse_connect, op_sse_recv, op_sse_close],
);
