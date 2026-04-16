use deno_core::op2;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

// Global WS connection store — needed because async ops can't access #[state]
lazy_static::lazy_static! {
    static ref WS_CONNECTIONS: Arc<Mutex<WsStore>> = Arc::new(Mutex::new(WsStore::new()));
}

struct WsStore {
    outgoing: HashMap<i32, mpsc::UnboundedSender<String>>,
    incoming: HashMap<i32, Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<String>>>>,
    next_id: i32,
}

impl WsStore {
    fn new() -> Self {
        Self {
            outgoing: HashMap::new(),
            incoming: HashMap::new(),
            next_id: 1,
        }
    }
}

/// WebSocket state (empty — connections stored globally).
pub struct WebSocketState;

impl WebSocketState {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Serialize)]
pub struct WsConnectResult {
    pub id: i32,
    pub ok: bool,
    pub error: String,
}

/// Connect to a WebSocket server. Returns connection ID.
#[op2(async)]
#[serde]
pub async fn op_ws_connect(
    #[string] url: String,
) -> Result<WsConnectResult, deno_core::error::AnyError> {
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::Message;

    let connect_result = tokio_tungstenite::connect_async(&url).await;

    match connect_result {
        Ok((ws_stream, _response)) => {
            let (mut sink, mut stream) = ws_stream.split();

            // Outgoing: JS → server
            let (out_tx, mut out_rx) = mpsc::unbounded_channel::<String>();
            // Incoming: server → JS
            let (in_tx, in_rx) = mpsc::unbounded_channel::<String>();

            // Assign ID
            let id = {
                let mut store = WS_CONNECTIONS.lock().unwrap_or_else(|e| e.into_inner());
                let id = store.next_id;
                store.next_id += 1;
                store.outgoing.insert(id, out_tx);
                store
                    .incoming
                    .insert(id, Arc::new(tokio::sync::Mutex::new(in_rx)));
                id
            };

            // Spawn send task
            tokio::spawn(async move {
                while let Some(msg) = out_rx.recv().await {
                    if sink.send(Message::Text(msg.into())).await.is_err() {
                        break;
                    }
                }
            });

            // Spawn receive task
            tokio::spawn(async move {
                while let Some(Ok(msg)) = stream.next().await {
                    match msg {
                        Message::Text(text) => {
                            let _ = in_tx.send(text.to_string());
                        }
                        Message::Binary(bin) => {
                            let _ = in_tx.send(String::from_utf8_lossy(&bin).to_string());
                        }
                        Message::Close(_) => break,
                        _ => {}
                    }
                }
            });

            Ok(WsConnectResult {
                id,
                ok: true,
                error: String::new(),
            })
        }
        Err(e) => Ok(WsConnectResult {
            id: -1,
            ok: false,
            error: e.to_string(),
        }),
    }
}

/// Send a message on a WebSocket connection.
#[op2(fast)]
pub fn op_ws_send(#[smi] id: i32, #[string] data: &str) {
    let store = WS_CONNECTIONS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(sender) = store.outgoing.get(&id) {
        let _ = sender.send(data.to_string());
    }
}

/// Receive the next message from a WebSocket connection.
#[op2(async)]
#[string]
pub async fn op_ws_recv(#[smi] id: i32) -> Result<String, deno_core::error::AnyError> {
    let rx = {
        let store = WS_CONNECTIONS.lock().unwrap_or_else(|e| e.into_inner());
        store.incoming.get(&id).cloned()
    };
    match rx {
        Some(rx) => {
            let mut rx = rx.lock().await;
            match rx.recv().await {
                Some(msg) => Ok(msg),
                None => Ok(String::new()), // connection closed
            }
        }
        None => Ok(String::new()),
    }
}

/// Close a WebSocket connection.
#[op2(fast)]
pub fn op_ws_close(#[smi] id: i32) {
    let mut store = WS_CONNECTIONS.lock().unwrap_or_else(|e| e.into_inner());
    store.outgoing.remove(&id);
    store.incoming.remove(&id);
}

deno_core::extension!(
    websocket_extension,
    ops = [op_ws_connect, op_ws_send, op_ws_recv, op_ws_close],
);
