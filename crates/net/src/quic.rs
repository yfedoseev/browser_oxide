//! QUIC client for HTTP/3 transport.

use crate::error::NetError;
use quinn::VarInt;
use std::net::ToSocketAddrs;
use std::sync::Arc;
use std::time::Duration;

/// QUIC client with Chrome-like transport parameters.
#[derive(Clone)]
pub struct QuicClient {
    endpoint: quinn::Endpoint,
}

impl QuicClient {
    /// Create a new QUIC client with Chrome-like transport configuration.
    pub fn new() -> Result<Self, NetError> {
        // TLS config: TLS 1.3 with h3 ALPN
        let mut tls_config = rustls::ClientConfig::builder()
            .with_root_certificates(Self::root_certs())
            .with_no_client_auth();
        tls_config.alpn_protocols = vec![b"h3".to_vec()];

        // Chrome-like QUIC transport parameters
        let mut transport = quinn::TransportConfig::default();
        transport.receive_window(VarInt::from_u32(15_728_640)); // 15MB
        transport.stream_receive_window(VarInt::from_u32(6_291_456)); // 6MB
        transport.max_concurrent_bidi_streams(VarInt::from_u32(1000));
        transport.max_concurrent_uni_streams(VarInt::from_u32(1000));
        transport.max_idle_timeout(Some(
            quinn::IdleTimeout::try_from(Duration::from_secs(30))
                .map_err(|e| NetError::Quic(e.to_string()))?,
        ));
        transport.keep_alive_interval(Some(Duration::from_secs(10)));
        // In quinn 0.11+, this is a field or method depending on the exact version.
        // We'll stick to the defaults if the method is missing or use the correct setter.

        let mut client_config = quinn::ClientConfig::new(Arc::new(
            quinn::crypto::rustls::QuicClientConfig::try_from(tls_config)
                .map_err(|e| NetError::Quic(e.to_string()))?,
        ));
        client_config.transport_config(Arc::new(transport));

        let mut endpoint = quinn::Endpoint::client("0.0.0.0:0".parse().unwrap())
            .map_err(|e| NetError::Quic(e.to_string()))?;
        endpoint.set_default_client_config(client_config);

        Ok(Self { endpoint })
    }

    /// Connect to a server via QUIC.
    pub async fn connect(&self, host: &str, port: u16) -> Result<quinn::Connection, NetError> {
        let addr = format!("{}:{}", host, port)
            .to_socket_addrs()
            .map_err(|e| NetError::Quic(format!("DNS resolve {}: {}", host, e)))?
            .next()
            .ok_or_else(|| NetError::Quic(format!("No addresses for {}", host)))?;

        let connection = self
            .endpoint
            .connect(addr, host)
            .map_err(|e| NetError::Quic(e.to_string()))?
            .await
            .map_err(|e| NetError::Quic(e.to_string()))?;

        Ok(connection)
    }

    fn root_certs() -> rustls::RootCertStore {
        let mut roots = rustls::RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        roots
    }
}
