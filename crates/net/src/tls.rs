//! BoringSSL TLS configuration with Chrome 147 fingerprint.
//!
//! Configures TLS to produce a ClientHello identical to Chrome 147,
//! including cipher suites, curves, signature algorithms, extensions,
//! and certificate compression — all in the exact order that produces
//! the correct JA3/JA4 fingerprint.

use boring2::ssl::{
    CertCompressionAlgorithm, ConnectConfiguration, SslConnector, SslCurve, SslMethod, SslVersion,
};
use boring2::x509::store::X509StoreBuilder;
use boring2::x509::X509;
use foreign_types::ForeignTypeRef;
use tokio::net::TcpStream;
use tokio_boring2::SslStream;

use crate::error::NetError;

/// Chrome 147 cipher suite list (order is critical for JA3 fingerprint).
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

/// Chrome 147 signature algorithms (order matters).
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
const CURVES: &[SslCurve] = &[
    SslCurve::X25519_MLKEM768,
    SslCurve::X25519,
    SslCurve::SECP256R1,
    SslCurve::SECP384R1,
];

/// ALPN protocols: h2 + http/1.1
const ALPN_PROTOS: &[u8] = b"\x02h2\x08http/1.1";

use rand::prelude::SliceRandom;

/// Chrome 147 extension permutation (indices into BoringSSL kExtensions table).
/// Matches real headed Chrome 147 on macOS arm64.
///
/// **Chrome Shuffling Buckets**: real Chrome 130+ does NOT shuffle all extensions
/// randomly. It shuffles within three specific buckets.
/// - Bucket A: [0..9] (indices 14, 1, 4, 11, 15, 2, 24, 21, 17) - Shuffled
/// - Bucket B: [9..15] (indices 0, 3, 5, 7, 6, 8) - Shuffled
/// - Bucket C: [15] (index 9) - Fixed at end (signature_algorithms)
const CHROME_EXTENSION_PERMUTATION: &[u8] = &[
    14, // key_share (51)
    1,  // encrypted_client_hello (65037)
    4,  // supported_groups (10)
    11, // certificate_timestamp (18)
    15, // psk_key_exchange_modes (45)
    2,  // extended_master_secret (23)
    24, // application_settings_new (17613)
    21, // cert_compression (27)
    17, // supported_versions (43)
    0,  // server_name (0)
    3,  // renegotiate (65281)
    5,  // ec_point_formats (11)
    8,  // status_request (5)
    7,  // application_layer_protocol_negotiation (16)
    6,  // session_ticket (35)
    9,  // signature_algorithms (13)
    22, // delegated_credential (34)
];

/// Generate a fresh shuffle of the Chrome extension permutation within its buckets.
fn shuffled_chrome_extension_permutation() -> Vec<u8> {
    let mut rng = rand::thread_rng();

    // Bucket A: indices 0..9 (9 items)
    let mut bucket_a = CHROME_EXTENSION_PERMUTATION[0..9].to_vec();
    // Bucket B: indices 9..15 (6 items)
    let mut bucket_b = CHROME_EXTENSION_PERMUTATION[9..15].to_vec();
    // Bucket C: index 15 (1 item - signature_algorithms)
    let bucket_c = vec![CHROME_EXTENSION_PERMUTATION[15]];

    bucket_a.shuffle(&mut rng);
    bucket_b.shuffle(&mut rng);

    let mut permutation = Vec::with_capacity(16);
    permutation.extend(bucket_a);
    permutation.extend(bucket_b);
    permutation.extend(bucket_c);
    permutation
}

/// Build an `SslConnector` configured with Chrome 147 TLS fingerprint.
pub fn chrome_connector() -> Result<SslConnector, NetError> {
    let mut builder =
        SslConnector::builder(SslMethod::tls()).map_err(|e| NetError::Tls(e.to_string()))?;

    // Cipher suites
    builder
        .set_cipher_list(CIPHER_LIST)
        .map_err(|e| NetError::Tls(e.to_string()))?;

    // Elliptic curves
    builder
        .set_curves(CURVES)
        .map_err(|e| NetError::Tls(e.to_string()))?;

    // Signature algorithms
    builder
        .set_sigalgs_list(SIGALGS_LIST)
        .map_err(|e| NetError::Tls(e.to_string()))?;

    // ALPN
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

    builder.set_permute_extensions(false);

    builder.enable_ocsp_stapling();
    builder.enable_signed_cert_timestamps();

    // Chrome 131+ sends both X25519MLKEM768 and X25519 key shares.
    builder.set_key_shares_limit(2);

    // Certificate compression (Brotli + Zlib)
    builder
        .add_cert_compression_alg(CertCompressionAlgorithm::Brotli)
        .map_err(|e| NetError::Tls(e.to_string()))?;
    builder
        .add_cert_compression_alg(CertCompressionAlgorithm::Zlib)
        .map_err(|e| NetError::Tls(e.to_string()))?;

    // Load Mozilla root certificates into the certificate store
    let mut cert_store = X509StoreBuilder::new().map_err(|e| NetError::Tls(e.to_string()))?;
    for cert_der in webpki_root_certs::TLS_SERVER_ROOT_CERTS {
        let x509 = X509::from_der(cert_der.as_ref())
            .map_err(|e| NetError::Tls(format!("failed to parse root cert: {e}")))?;
        let _ = cert_store.add_cert(x509);
    }
    builder.set_cert_store(cert_store.build());

    let connector = builder.build();
    
    // Apply shuffled permutation directly to the context.
    let permutation = shuffled_chrome_extension_permutation();
    unsafe {
        boring_sys2::SSL_CTX_set_extension_permutation(
            connector.context().as_ptr(),
            permutation.as_ptr(),
            permutation.len(),
        );
    }

    Ok(connector)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shuffled_permutation_buckets() {
        let p1 = shuffled_chrome_extension_permutation();
        let p2 = shuffled_chrome_extension_permutation();
        
        assert_eq!(p1.len(), 16);
        assert_eq!(p2.len(), 16);
        
        // Bucket C (signature_algorithms) must always be at index 15
        assert_eq!(p1[15], 9);
        assert_eq!(p2[15], 9);
        
        // Buckets A and B must contain the same set of elements
        let mut b1_a = p1[0..9].to_vec();
        let mut b2_a = p2[0..9].to_vec();
        b1_a.sort();
        b2_a.sort();
        let expected_a = vec![1, 2, 4, 11, 14, 15, 17, 21, 24];
        assert_eq!(b1_a, expected_a);
        assert_eq!(b2_a, expected_a);
        
        let mut b1_b = p1[9..15].to_vec();
        let mut b2_b = p2[9..15].to_vec();
        b1_b.sort();
        b2_b.sort();
        let expected_b = vec![0, 3, 5, 6, 7, 8];
        assert_eq!(b1_b, expected_b);
        assert_eq!(b2_b, expected_b);
        
        // Probabilistically, they should be different
        assert_ne!(p1, p2, "Shuffles should be non-deterministic");
    }
}

/// Configure a per-connection TLS session with ALPS, ECH GREASE, and SNI.
pub fn configure_connection(
    connector: &SslConnector,
    domain: &str,
) -> Result<ConnectConfiguration, NetError> {
    let mut config = connector
        .configure()
        .map_err(|e| NetError::Tls(e.to_string()))?;

    // ECH GREASE
    config.set_enable_ech_grease(true);

    // Application-layer settings (ALPS) for HTTP/2
    // Chrome 147 Headless sends 4 settings: 1, 2, 4, 6.
    let alps_payload: &[u8] = &[
        // SETTINGS frame (Length 24, Type 4, Flags 0, Stream 0)
        0x00, 0x00, 0x18, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00,
        // ID 1: 65536
        0x00, 0x01, 0x00, 0x01, 0x00, 0x00,
        // ID 2: 0
        0x00, 0x02, 0x00, 0x00, 0x00, 0x00,
        // ID 4: 6291456
        0x00, 0x04, 0x00, 0x60, 0x00, 0x00,
        // ID 6: 262144
        0x00, 0x06, 0x00, 0x04, 0x00, 0x00,
        // Empty ACCEPT_CH frame (Length 0, Type 0x89, Flags 0, Stream 0)
        0x00, 0x00, 0x00, 0x89, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    unsafe {
        if boring_sys2::SSL_add_application_settings(
            config.as_ptr(),
            b"h2".as_ptr(),
            2,
            alps_payload.as_ptr(),
            alps_payload.len(),
        ) != 1
        {
            return Err(NetError::Tls("failed to add ALPS settings".into()));
        }
    }
    config.set_alps_use_new_codepoint(true);

    // SNI
    let sni_domain = domain.trim_start_matches('[').trim_end_matches(']');
    if sni_domain.parse::<std::net::IpAddr>().is_ok() {
        config.set_use_server_name_indication(false);
    } else {
        config
            .set_hostname(sni_domain)
            .map_err(|e| NetError::Tls(e.to_string()))?;
    }

    Ok(config)
}

/// Establish a TLS connection to `domain` using the provided `SslConnector`.
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
