#[derive(Debug, thiserror::Error)]
pub enum NetError {
    #[error("HTTP error: {0}")]
    Http(String),

    #[error("URL parse error: {0}")]
    Url(#[from] url::ParseError),

    #[error("WebSocket error: {0}")]
    WebSocket(String),

    #[error("Request failed: {0}")]
    Request(String),

    #[error("QUIC error: {0}")]
    Quic(String),

    #[error("HTTP/3 error: {0}")]
    H3(String),

    #[error("TLS error: {0}")]
    Tls(String),

    #[error("TCP error: {0}")]
    Tcp(String),
}
