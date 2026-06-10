use serde::{Deserialize, Serialize};

/// A CDP JSON-RPC request from the client.
#[derive(Debug, Deserialize)]
pub struct CdpRequest {
    pub id: u64,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

/// A CDP JSON-RPC response to the client.
#[derive(Debug, Serialize)]
pub struct CdpResponse {
    pub id: u64,
    pub result: serde_json::Value,
}

/// A CDP event pushed to the client (no id).
#[derive(Debug, Serialize, Clone)]
pub struct CdpEvent {
    pub method: String,
    pub params: serde_json::Value,
}

/// A CDP error response.
#[derive(Debug, Serialize)]
pub struct CdpError {
    pub id: u64,
    pub error: CdpErrorDetail,
}

#[derive(Debug, Serialize)]
pub struct CdpErrorDetail {
    pub code: i32,
    pub message: String,
}

impl CdpResponse {
    pub fn ok(id: u64, result: serde_json::Value) -> Self {
        Self { id, result }
    }

    pub fn empty(id: u64) -> Self {
        Self {
            id,
            result: serde_json::json!({}),
        }
    }
}

impl CdpError {
    pub fn method_not_found(id: u64, method: &str) -> Self {
        Self {
            id,
            error: CdpErrorDetail {
                code: -32601,
                message: format!("'{}' wasn't found", method),
            },
        }
    }

    pub fn internal(id: u64, msg: &str) -> Self {
        Self {
            id,
            error: CdpErrorDetail {
                code: -32603,
                message: msg.to_string(),
            },
        }
    }
}

impl CdpEvent {
    pub fn new(method: &str, params: serde_json::Value) -> Self {
        Self {
            method: method.to_string(),
            params,
        }
    }
}

/// Serialize to JSON string for WebSocket transmission.
pub fn to_json(msg: &impl Serialize) -> String {
    serde_json::to_string(msg).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_request() {
        let json =
            r#"{"id": 1, "method": "Page.navigate", "params": {"url": "https://example.com"}}"#;
        let req: CdpRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.id, 1);
        assert_eq!(req.method, "Page.navigate");
        assert_eq!(req.params["url"], "https://example.com");
    }

    #[test]
    fn serialize_response() {
        let resp = CdpResponse::ok(1, serde_json::json!({"frameId": "main"}));
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"id\":1"));
        assert!(json.contains("\"frameId\":\"main\""));
    }

    #[test]
    fn serialize_event() {
        let event = CdpEvent::new(
            "Page.loadEventFired",
            serde_json::json!({"timestamp": 123.456}),
        );
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("Page.loadEventFired"));
        assert!(!json.contains("\"id\"")); // Events have no id
    }

    #[test]
    fn serialize_error() {
        let err = CdpError::method_not_found(5, "Unknown.method");
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("-32601"));
        assert!(json.contains("Unknown.method"));
    }

    #[test]
    fn empty_params() {
        let json = r#"{"id": 1, "method": "Page.enable"}"#;
        let req: CdpRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.method, "Page.enable");
        assert!(req.params.is_null());
    }
}
