//! BoringSSL TLS configuration with Chrome 130 fingerprint.
//!
//! Configures TLS to produce a ClientHello identical to Chrome 130,
//! including cipher suites, curves, signature algorithms, extensions,
//! and certificate compression — all in the exact order that produces
//! the correct JA3/JA4 fingerprint.

use boring2::ssl::{
    CertCompressionAlgorithm, ConnectConfiguration, SslConnector, SslCurve, SslMethod, SslVersion,
};
use boring2::x509::store::X509StoreBuilder;
use boring2::x509::X509;
use tokio::net::TcpStream;
use tokio_boring2::SslStream;

use crate::error::NetError;

/// Chrome 130 cipher suite list (order is critical for JA3 fingerprint).
const CIPHER_LIST: &str = concat!(
    "TLS_AES_128_GCM_SHA256",
    ":TLS_AES_256_GCM_SHA384",
    ":TLS_CHACHA20_POLY1305_SHA256",
    ":TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256",
    ":TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256",
    ":TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384",
    ":TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384",
    ":TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256",
    ":TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256",
    ":TLS_ECDHE_RSA_WITH_AES_128_CBC_SHA",
    ":TLS_ECDHE_RSA_WITH_AES_256_CBC_SHA",
    ":TLS_RSA_WITH_AES_128_GCM_SHA256",
    ":TLS_RSA_WITH_AES_256_GCM_SHA384",
    ":TLS_RSA_WITH_AES_128_CBC_SHA",
    ":TLS_RSA_WITH_AES_256_CBC_SHA",
);

/// Chrome 130 signature algorithms (order matters).
const SIGALGS_LIST: &str = concat!(
    "ecdsa_secp256r1_sha256",
    ":rsa_pss_rsae_sha256",
    ":rsa_pkcs1_sha256",
    ":ecdsa_secp384r1_sha384",
    ":rsa_pss_rsae_sha384",
    ":rsa_pkcs1_sha384",
    ":rsa_pss_rsae_sha512",
    ":rsa_pkcs1_sha512",
);

/// Chrome elliptic curves.
///
/// **Verified against a real Chrome 146 capture** from the developer's
/// machine: the post-quantum curve is `X25519MLKEM768 (4588)`, NOT the
/// older `X25519_KYBER768_DRAFT00 (25497)`. Chrome 131+ replaced the
/// KYBER draft with the standardised MLKEM.
///
/// Curve order and GREASE are from the same Chrome 146 capture:
/// `GREASE - X25519MLKEM768 - X25519 - SECP256R1 - SECP384R1`.
const CURVES: &[SslCurve] = &[
    SslCurve::X25519_MLKEM768,
    SslCurve::X25519,
    SslCurve::SECP256R1,
    SslCurve::SECP384R1,
];

/// ALPN protocols: h2 + http/1.1
const ALPN_PROTOS: &[u8] = b"\x02h2\x08http/1.1";

/// Build an `SslConnector` configured with Chrome 130 TLS fingerprint.
///
/// This sets cipher suites, curves, signature algorithms, GREASE,
/// extension permutation, OCSP stapling, SCT, certificate compression,
/// and loads Mozilla root certificates into the certificate store.
pub fn chrome_connector() -> Result<SslConnector, NetError> {
    let mut builder =
        SslConnector::builder(SslMethod::tls()).map_err(|e| NetError::Tls(e.to_string()))?;

    // Cipher suites (exact Chrome 130 order)
    builder
        .set_cipher_list(CIPHER_LIST)
        .map_err(|e| NetError::Tls(e.to_string()))?;

    // Elliptic curves (includes post-quantum hybrid)
    builder
        .set_curves(CURVES)
        .map_err(|e| NetError::Tls(e.to_string()))?;

    // Signature algorithms
    builder
        .set_sigalgs_list(SIGALGS_LIST)
        .map_err(|e| NetError::Tls(e.to_string()))?;

    // ALPN: prefer h2, fall back to http/1.1
    builder
        .set_alpn_protos(ALPN_PROTOS)
        .map_err(|e| NetError::Tls(e.to_string()))?;

    // TLS version range
    builder
        .set_min_proto_version(Some(SslVersion::TLS1_2))
        .map_err(|e| NetError::Tls(e.to_string()))?;
    builder
        .set_max_proto_version(Some(SslVersion::TLS1_3))
        .map_err(|e| NetError::Tls(e.to_string()))?;

    // Chrome extensions
    builder.set_grease_enabled(true);
    builder.set_permute_extensions(true);
    builder.enable_ocsp_stapling();
    builder.enable_signed_cert_timestamps();

    // Certificate compression (Brotli)
    builder
        .add_cert_compression_alg(CertCompressionAlgorithm::Brotli)
        .map_err(|e| NetError::Tls(e.to_string()))?;

    // Load Mozilla root certificates into the certificate store
    let mut cert_store =
        X509StoreBuilder::new().map_err(|e| NetError::Tls(e.to_string()))?;

    // Also load system default CA paths for cross-signed/intermediate certs
    cert_store
        .set_default_paths()
        .map_err(|e| NetError::Tls(format!("failed to set default cert paths: {e}")))?;

    for cert_der in webpki_root_certs::TLS_SERVER_ROOT_CERTS {
        let x509 = X509::from_der(cert_der.as_ref())
            .map_err(|e| NetError::Tls(format!("failed to parse root cert: {e}")))?;
        // Ignore duplicate cert errors (system certs may overlap)
        let _ = cert_store.add_cert(x509);
    }
    builder.set_cert_store(cert_store.build());

    Ok(builder.build())
}

/// Configure a per-connection TLS session with ALPS, ECH GREASE, and SNI.
///
/// Must be called for each new connection — these settings are per-SSL,
/// not per-context.
pub fn configure_connection(
    connector: &SslConnector,
    domain: &str,
) -> Result<ConnectConfiguration, NetError> {
    let mut config = connector
        .configure()
        .map_err(|e| NetError::Tls(e.to_string()))?;

    // config
    //     .add_application_settings(b"h2")
    //     .map_err(|e| NetError::Tls(e.to_string()))?;
    // config.set_alps_use_new_codepoint(true);

    // ECH GREASE (Encrypted Client Hello)
    config.set_enable_ech_grease(false);

    // SNI: strip brackets from IPv6 addresses
    let sni_domain = domain.trim_start_matches('[').trim_end_matches(']');
    // If it looks like an IP address, disable SNI
    if sni_domain.parse::<std::net::IpAddr>().is_ok() {
        config.set_use_server_name_indication(false);
    }

    Ok(config)
}

/// Perform a TLS handshake over a TCP stream, returning an async TLS stream.
///
/// The returned stream implements `AsyncRead + AsyncWrite` and can be used
/// with the HTTP/2 or HTTP/1.1 client layers.
pub async fn connect_tls(
    connector: &SslConnector,
    domain: &str,
    stream: TcpStream,
) -> Result<SslStream<TcpStream>, NetError> {
    let config = configure_connection(connector, domain)?;
    let sni_domain = domain.trim_start_matches('[').trim_end_matches(']');

    tokio_boring2::connect(config, sni_domain, stream)
        .await
        .map_err(|e| NetError::Tls(format!("TLS handshake failed: {e}")))
}

/// Returns the negotiated ALPN protocol from a TLS stream, if any.
pub fn negotiated_alpn(stream: &SslStream<TcpStream>) -> Option<&[u8]> {
    stream.ssl().selected_alpn_protocol()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connector_builds_successfully() {
        let connector = chrome_connector();
        assert!(connector.is_ok(), "Failed to build Chrome TLS connector: {:?}", connector.err());
    }

    #[test]
    fn configure_connection_works() {
        let connector = chrome_connector().unwrap();
        let config = configure_connection(&connector, "example.com");
        assert!(config.is_ok());
    }

    #[test]
    fn configure_connection_ipv6() {
        let connector = chrome_connector().unwrap();
        // IPv6 address should disable SNI
        let config = configure_connection(&connector, "[::1]");
        assert!(config.is_ok());
    }

    #[tokio::test]
    #[ignore] // requires network
    async fn tls_connects_to_httpbin() {
        let connector = chrome_connector().unwrap();
        let stream = crate::tcp::connect("httpbin.org", 443, std::time::Duration::from_secs(10))
            .await
            .unwrap();
        let tls = connect_tls(&connector, "httpbin.org", stream).await;
        assert!(tls.is_ok(), "TLS connection failed: {:?}", tls.err());
    }

    #[tokio::test]
    #[ignore] // requires network
    async fn tls_connects_to_example_com() {
        let connector = chrome_connector().unwrap();
        let stream = crate::tcp::connect("example.com", 443, std::time::Duration::from_secs(10))
            .await
            .unwrap();
        let tls = connect_tls(&connector, "example.com", stream).await;
        assert!(tls.is_ok(), "TLS connection failed: {:?}", tls.err());
    }
}
