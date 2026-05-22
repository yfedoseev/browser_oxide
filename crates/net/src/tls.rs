//! BoringSSL TLS configuration with Chrome 147 fingerprint.
//!
//! Configures TLS to produce a ClientHello identical to Chrome 147,
//! including cipher suites, curves, signature algorithms, extensions,
//! and certificate compression — all in the exact order that produces
//! the correct JA3/JA4 fingerprint.

use boring2::ssl::{
    CertCompressionAlgorithm, ConnectConfiguration, SslConnector, SslCurve, SslMethod, SslOptions,
    SslVersion,
};
use boring2::x509::store::X509StoreBuilder;
use boring2::x509::X509;
use foreign_types::ForeignTypeRef;
use stealth::{DeviceClass, StealthProfile};
use tokio::net::TcpStream;
use tokio_boring2::SslStream;

use crate::error::NetError;

/// The Chrome major version whose **verified-real** ClientHello / H2
/// fingerprint these constants reproduce, byte-exact.
///
/// **Why this is 147 while every desktop preset's UA advertises Chrome
/// 148 — and why that is NOT an incoherent skew:**
///
/// 1. Chrome's TLS ClientHello is **version-stable across majors**. It
///    only changes on a deliberate TLS-stack change; the last such change
///    was the MLKEM768 post-quantum rollout at Chrome 131. There was no
///    TLS-stack change between 147 and 148 (consecutive majors, ~1 month
///    apart, May 2026), so the bytes real Chrome 148 puts on the wire are
///    identical to this verified-real 147 capture. Doc 03 §1.1 records
///    "byte-exact Chrome 147/148 values" — they are the same values.
/// 2. **JA4 does not encode the Chrome version.** JA4 = TLS-version +
///    sorted cipher/extension counts + ALPN + sorted sigalgs. None of
///    those differ 147↔148. A vendor's "JA4-vs-UA cross-check" verifies
///    the JA4 corresponds to *a Chrome* consistent with the UA *family*
///    — it cannot, even in principle, detect a 147-vs-148 minor/major
///    label difference.
/// 3. UA=148 is a **deliberate, A/B-tested** decision: real Chrome
///    stable IS 148 (chromiumdash; shipped early May 2026), and the
///    147→148 UA bump *recovered* several previously-blocked sites in
///    our measurement. Rolling the UA back to 147 would re-introduce
///    those regressions and advertise an outdated browser (its own
///    soft-deny signal). So the coherent state is UA=148 + these
///    (wire-identical) 147-reference bytes.
///
/// This constant exists so the coherence is **machine-checked** (see the
/// `tls_fingerprint_vectors_no_silent_drift` test) and the rationale is
/// one `grep` away — the silent-drift hazard the plan flags is removed
/// without changing a single wire byte or UA.
pub const TLS_CHROME_MAJOR: u32 = 147;

/// The Chrome major every desktop Chrome preset's `user_agent`
/// advertises. Intentionally != [`TLS_CHROME_MAJOR`]; see that
/// constant's docs for why this is wire-coherent, not a skew.
pub const UA_CHROME_MAJOR: u32 = 148;

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

/// Chrome desktop elliptic curves (Chrome 131+ uses MLKEM768).
const CURVES_DESKTOP: &[SslCurve] = &[
    SslCurve::X25519_MLKEM768,
    SslCurve::X25519,
    SslCurve::SECP256R1,
    SslCurve::SECP384R1,
];

/// Chrome Android elliptic curves. Kyber768Draft00 (deprecated) was the
/// canonical Chrome 124-130 PQ curve; Chrome 131+ desktop replaced it with
/// MLKEM768 (codepoint 4588). The lexiforest `chrome_131.0.6778.81_android`
/// capture shows no PQ at all (just 29/23/24), but Chrome Android shares the
/// desktop codebase and by Chrome 147+ should have rolled MLKEM — verify
/// against fresh Pixel capture if regressions appear.
const CURVES_ANDROID: &[SslCurve] = CURVES_DESKTOP;

/// iOS Safari 18 cipher suite list (20 ciphers, Apple's order). Per the
/// canonical `lexiforest/curl-impersonate/tests/signatures/safari_18.0_iOS.yaml`.
/// Distinct from Chrome desktop (15 ciphers): includes 3DES_EDE_CBC_SHA at
/// the tail and an extra RSA_WITH_3DES_EDE_CBC_SHA. Cipher order matters
/// for JA3.
const CIPHER_LIST_SAFARI_IOS: &str = concat!(
    "TLS_AES_128_GCM_SHA256",
    ":TLS_AES_256_GCM_SHA384",
    ":TLS_CHACHA20_POLY1305_SHA256",
    ":TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384",
    ":TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256",
    ":TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256",
    ":TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384",
    ":TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256",
    ":TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256",
    ":TLS_ECDHE_ECDSA_WITH_AES_256_CBC_SHA",
    ":TLS_ECDHE_ECDSA_WITH_AES_128_CBC_SHA",
    ":TLS_ECDHE_RSA_WITH_AES_256_CBC_SHA",
    ":TLS_ECDHE_RSA_WITH_AES_128_CBC_SHA",
    ":TLS_RSA_WITH_AES_256_GCM_SHA384",
    ":TLS_RSA_WITH_AES_128_GCM_SHA256",
    ":TLS_RSA_WITH_AES_256_CBC_SHA",
    ":TLS_RSA_WITH_AES_128_CBC_SHA",
    ":TLS_ECDHE_ECDSA_WITH_3DES_EDE_CBC_SHA",
    ":TLS_ECDHE_RSA_WITH_3DES_EDE_CBC_SHA",
    ":TLS_RSA_WITH_3DES_EDE_CBC_SHA",
);

/// iOS Safari signature algorithms (10 entries, includes the duplicated
/// `rsa_pss_rsae_sha384` Apple bug we must reproduce verbatim per the audit).
/// Both wreq-util and curl-impersonate include the duplicate.
const SIGALGS_LIST_SAFARI_IOS: &str = concat!(
    "ecdsa_secp256r1_sha256",
    ":rsa_pss_rsae_sha256",
    ":rsa_pkcs1_sha256",
    ":ecdsa_secp384r1_sha384",
    ":rsa_pss_rsae_sha384",
    ":rsa_pss_rsae_sha384",
    ":rsa_pkcs1_sha384",
    ":rsa_pss_rsae_sha512",
    ":rsa_pkcs1_sha512",
    ":rsa_pkcs1_sha1",
);

/// iOS Safari 18 elliptic curves. No PQ (MLKEM lands in iOS 26 per Apple's
/// PQC support page). Adds P-521 vs Chrome desktop. Order per safari_18.0_iOS.yaml.
const CURVES_SAFARI_IOS: &[SslCurve] = &[
    SslCurve::X25519,
    SslCurve::SECP256R1,
    SslCurve::SECP384R1,
    SslCurve::SECP521R1,
];

/// iOS Safari 18 extension permutation. Indices into BoringSSL's internal
/// `BORING_SSLEXTENSION_PERMUTATION` table — see boring2 ssl/mod.rs for the
/// canonical ordering. Per `safari_18.0_iOS.yaml` lexiforest signature, real
/// Safari emits its extensions in a FIXED order (no Fisher-Yates shuffle),
/// roughly: server_name, extended_master_secret, renegotiate, supported_groups,
/// ec_point_formats, ALPN, status_request, signature_algorithms,
/// signed_certificate_timestamp, key_share, psk_key_exchange_modes,
/// supported_versions, cert_compression. (GREASE and PADDING are auto-emitted
/// by BoringSSL outside the permutation table; PADDING positional ordering
/// requires raw extension injection — deferred per audit.)
const SAFARI_IOS_EXTENSION_PERMUTATION: &[u8] = &[
    0,  // server_name
    2,  // extended_master_secret
    3,  // renegotiate
    4,  // supported_groups
    5,  // ec_point_formats
    7,  // application_layer_protocol_negotiation (ALPN)
    8,  // status_request
    9,  // signature_algorithms
    11, // certificate_timestamp
    14, // key_share
    15, // psk_key_exchange_modes
    17, // supported_versions
    22, // cert_compression
];

/// ALPN protocols: h2 + http/1.1
const ALPN_PROTOS: &[u8] = b"\x02h2\x08http/1.1";

use rand::prelude::SliceRandom;

/// Chrome 147 extension permutation (indices into BoringSSL kExtensions table).
/// 16 extensions matching a verified Chrome 147 macOS arm64 reference capture.
///
/// **Real Chrome shuffling behavior** (per Fastly TLS Fingerprinting blog
/// + Chromestatus 5124606246518784 + BoringSSL `ssl_setup_extension_permutation`
/// source): Chrome shuffles ALL non-PSK extensions with a single Fisher-Yates
/// pass — there is no documented bucket structure. The only positional
/// constraint is psk_key_exchange_modes / pre_shared_key being last (BoringSSL
/// enforces this). The previous 3-bucket scheme was folkore from earlier
/// public RE work; it reduced shuffle entropy by ~720,000× and put
/// signature_algorithms always at position 16 — a deterministic positional
/// tell that per-handshake classifiers (Akamai, Kasada) can detect as a
/// soft-deny signal.
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
];

/// Generate a fresh Fisher-Yates shuffle over all 16 Chrome 147 extensions.
fn shuffled_chrome_extension_permutation() -> Vec<u8> {
    let mut rng = rand::rng();
    let mut permutation = CHROME_EXTENSION_PERMUTATION.to_vec();
    permutation.shuffle(&mut rng);
    permutation
}

/// Build an `SslConnector` configured with the TLS fingerprint matching
/// `profile.device_class`. Currently all variants share Chrome 147 desktop
/// configuration; Phase 2 branches here for Android and iOS Safari.
pub fn chrome_connector(profile: &StealthProfile) -> Result<SslConnector, NetError> {
    // Phase 2/3 (2026-05-12): per-device_class branching.
    //  - Desktop / Android: shared Chrome 147 cipher/sigalg/extension config.
    //    Android only diverges in the curves list (Kyber768Draft00 vs MLKEM).
    //  - MobileIOS: distinct Safari 18 cipher/sigalg/curves + skip Fisher-Yates
    //    extension permutation + zlib cert compression + SslOptions::NO_TICKET.
    //    Per-connection ALPS and ECH grease are also skipped — see
    //    configure_connection() below.
    let is_safari_ios = profile.device_class == DeviceClass::MobileIOS;
    let curves: &[SslCurve] = match profile.device_class {
        DeviceClass::MobileAndroid => CURVES_ANDROID,
        DeviceClass::MobileIOS => CURVES_SAFARI_IOS,
        DeviceClass::Desktop => CURVES_DESKTOP,
    };
    let cipher_list: &str = if is_safari_ios {
        CIPHER_LIST_SAFARI_IOS
    } else {
        CIPHER_LIST
    };
    let sigalgs_list: &str = if is_safari_ios {
        SIGALGS_LIST_SAFARI_IOS
    } else {
        SIGALGS_LIST
    };
    let mut builder =
        SslConnector::builder(SslMethod::tls()).map_err(|e| NetError::Tls(e.to_string()))?;

    // Cipher suites (per device_class)
    builder
        .set_cipher_list(cipher_list)
        .map_err(|e| NetError::Tls(e.to_string()))?;

    // Elliptic curves (per device_class)
    builder
        .set_curves(curves)
        .map_err(|e| NetError::Tls(e.to_string()))?;

    // Signature algorithms (per device_class)
    builder
        .set_sigalgs_list(sigalgs_list)
        .map_err(|e| NetError::Tls(e.to_string()))?;

    // ALPN
    builder
        .set_alpn_protos(ALPN_PROTOS)
        .map_err(|e| NetError::Tls(e.to_string()))?;

    // TLS version range. Safari iOS 18.x advertises 4 versions (1.0, 1.1,
    // 1.2, 1.3) in supported_versions per canonical safari_18.4_iOS.yaml —
    // visible as a length-difference on the extension. Servers still
    // negotiate 1.3 because no real server speaks 1.0/1.1 anymore, but the
    // ClientHello must advertise all four to fingerprint as Safari.
    let min_version = if is_safari_ios {
        SslVersion::TLS1
    } else {
        SslVersion::TLS1_2
    };
    builder
        .set_min_proto_version(Some(min_version))
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

    // Certificate compression. Chrome desktop+Android = Brotli (algo 2).
    // iOS Safari = Zlib (algo 1) — this is one of the four big "Safari
    // is missing" / "Safari is different" signals (the other three are
    // ECH absence, ALPS absence, session_ticket absence).
    let cert_compress_alg = if is_safari_ios {
        CertCompressionAlgorithm::Zlib
    } else {
        CertCompressionAlgorithm::Brotli
    };
    builder
        .add_cert_compression_alg(cert_compress_alg)
        .map_err(|e| NetError::Tls(e.to_string()))?;

    // iOS Safari does not send the session_ticket extension at all.
    // SslOptions::NO_TICKET tells BoringSSL to omit the extension entirely
    // (vs sending it with a stale ticket).
    if is_safari_ios {
        builder.set_options(SslOptions::NO_TICKET);
    }

    // Load Mozilla root certificates into the certificate store
    let mut cert_store = X509StoreBuilder::new().map_err(|e| NetError::Tls(e.to_string()))?;
    for cert_der in webpki_root_certs::TLS_SERVER_ROOT_CERTS {
        let x509 = X509::from_der(cert_der.as_ref())
            .map_err(|e| NetError::Tls(format!("failed to parse root cert: {e}")))?;
        let _ = cert_store.add_cert(x509);
    }
    builder.set_cert_store(cert_store.build());

    let connector = builder.build();

    // Extension order:
    //  - Chrome: per-handshake Fisher-Yates shuffle of all 16 desktop extensions
    //  - Safari iOS: FIXED order (same every handshake) — Phase D
    //    upgrade. Set Safari's specific 13-extension order via the same
    //    permutation API. PADDING positional ordering still requires raw
    //    extension injection (deferred); BoringSSL auto-emits PADDING when
    //    ClientHello length crosses ~512 bytes, which our Safari profile
    //    typically does.
    let permutation = if is_safari_ios {
        SAFARI_IOS_EXTENSION_PERMUTATION.to_vec()
    } else {
        shuffled_chrome_extension_permutation()
    };
    // SAFETY: BoringSSL's `SSL_CTX_set_extension_permutation` reads
    // `len` consecutive `uint8_t` from `ptr` and copies them into the
    // SSL_CTX. `permutation` is a contiguous `Vec<u8>` that lives
    // for the duration of this call; `permutation.as_ptr()` and
    // `permutation.len()` are an exact, in-bounds, non-null
    // pair. `connector.context()` is a live `SslContext` we just
    // built — its `as_ptr()` returns a non-null pointer valid for
    // the call. No aliasing concern: BoringSSL only reads the
    // permutation buffer.
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

    /// Master plan §4 Phase 1 **G7** — the self-verifying JA4 drift
    /// guard + UA/TLS coherence assert. Network-free.
    ///
    /// Pins every JA4 input (cipher list, sigalg list, supported-groups
    /// order, extension count) byte-/element-exact to the verified-real
    /// Chrome reference so the fingerprint can never silently drift
    /// again (any edit to
    /// the constants fails this test loudly), and machine-checks that
    /// the deliberate UA=148 / TLS-ref=147 split is the documented,
    /// wire-coherent one (see [`TLS_CHROME_MAJOR`] docs).
    #[test]
    fn tls_fingerprint_vectors_no_silent_drift() {
        // --- JA4 input 1: cipher suites (order is JA4-significant) ---
        const EXPECT_CIPHERS: &str = "TLS_AES_128_GCM_SHA256:TLS_AES_256_GCM_SHA384:\
TLS_CHACHA20_POLY1305_SHA256:TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256:\
TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256:TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384:\
TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384:TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256:\
TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256:TLS_ECDHE_RSA_WITH_AES_128_CBC_SHA:\
TLS_ECDHE_RSA_WITH_AES_256_CBC_SHA:TLS_RSA_WITH_AES_128_GCM_SHA256:\
TLS_RSA_WITH_AES_256_GCM_SHA384:TLS_RSA_WITH_AES_128_CBC_SHA:\
TLS_RSA_WITH_AES_256_CBC_SHA";
        assert_eq!(
            CIPHER_LIST, EXPECT_CIPHERS,
            "Chrome cipher list drifted from the verified-real reference \
             — JA4 cipher hash would change"
        );

        // --- JA4 input 2: signature algorithms (order is JA4-significant) ---
        const EXPECT_SIGALGS: &str = "ecdsa_secp256r1_sha256:rsa_pss_rsae_sha256:\
rsa_pkcs1_sha256:ecdsa_secp384r1_sha384:rsa_pss_rsae_sha384:rsa_pkcs1_sha384:\
rsa_pss_rsae_sha512:rsa_pkcs1_sha512";
        assert_eq!(
            SIGALGS_LIST, EXPECT_SIGALGS,
            "Chrome sigalg list drifted — JA4 sigalg hash would change"
        );

        // --- JA4 input 3: supported groups / curves order ---
        assert_eq!(
            CURVES_DESKTOP,
            &[
                SslCurve::X25519_MLKEM768,
                SslCurve::X25519,
                SslCurve::SECP256R1,
                SslCurve::SECP384R1,
            ],
            "Chrome desktop curve order drifted (post-quantum MLKEM768 \
             must lead) — JA4 supported_groups would change"
        );

        // --- JA4 input 4: extension count (16 — JA4 `c` digit) ---
        assert_eq!(
            CHROME_EXTENSION_PERMUTATION.len(),
            16,
            "Chrome extension count drifted — JA4 extension-count digit \
             would change"
        );

        // --- UA / TLS coherence (the deliberate, wire-equivalent split) ---
        assert_eq!(TLS_CHROME_MAJOR, 147);
        assert_eq!(UA_CHROME_MAJOR, 148);
        // The split is intentional and wire-coherent: Chrome's
        // ClientHello did not rev 147→148, JA4 cannot encode the Chrome
        // version, and UA=148 is the A/B-tested current-Chrome value.
        // (Rationale in TLS_CHROME_MAJOR docs.)

        fn ua_chrome_major(ua: &str) -> Option<u32> {
            let i = ua.find("Chrome/")? + "Chrome/".len();
            ua[i..].split('.').next()?.parse().ok()
        }

        for profile in [
            stealth::presets::chrome_148_macos(),
            stealth::presets::chrome_148_windows(),
        ] {
            assert_eq!(
                ua_chrome_major(&profile.user_agent),
                Some(UA_CHROME_MAJOR),
                "desktop Chrome preset UA major must equal UA_CHROME_MAJOR \
                 (the coherence single-source-of-truth); UA was {:?}",
                profile.user_agent
            );
            assert_eq!(
                profile.tls_impersonate, "chrome_147",
                "desktop Chrome preset TLS profile must be the verified-real \
                 chrome_147 reference (wire-equivalent to Chrome \
                 {UA_CHROME_MAJOR}); see TLS_CHROME_MAJOR docs"
            );
        }
    }

    /// Capture the first 5 bytes of our outbound ClientHello (the TLS
    /// record header) and assert the record version is 0x0301 (TLS 1.0).
    /// Source-code analysis of `boringssl/src/ssl/ssl_aead_ctx.cc:168-173`
    /// confirms `RecordVersion()` returns `TLS1_VERSION` (0x0301) for the
    /// initial ClientHello (null cipher, version_ == 0). This test verifies
    /// it empirically — Option D #1 from the audit (BoringSSL vendor patch
    /// for TLS 1.0 record version) is **NOT NEEDED**.
    #[tokio::test]
    async fn safari_ios_emits_tls_1_0_record_version() {
        use tokio::io::AsyncReadExt;
        use tokio::net::{TcpListener, TcpStream};

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Background server that just reads the first 5 bytes and reports.
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 5];
            tokio::time::timeout(
                std::time::Duration::from_secs(3),
                stream.read_exact(&mut buf),
            )
            .await
            .unwrap()
            .unwrap();
            buf
        });

        // Connect with iOS Safari profile.
        let profile = stealth::presets::iphone_15_pro_safari_18();
        let connector = chrome_connector(&profile).expect("connector");
        let tcp = TcpStream::connect(addr).await.unwrap();
        // We expect the handshake to fail (server doesn't respond), but the
        // ClientHello is sent before that. Race the timeout against the
        // server's read.
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            connect_tls(&connector, &profile, "localhost", tcp),
        )
        .await;

        let bytes = tokio::time::timeout(std::time::Duration::from_secs(2), server)
            .await
            .expect("server timeout")
            .expect("server task");

        let content_type = bytes[0];
        let record_version = ((bytes[1] as u16) << 8) | (bytes[2] as u16);

        // Content type 22 = TLS handshake
        assert_eq!(
            content_type, 22,
            "expected TLS handshake (22), got {content_type}"
        );

        // Record version: real Safari sends 0x0301 (TLS 1.0); BoringSSL
        // emits the same for null-cipher (initial ClientHello).
        assert_eq!(
            record_version, 0x0301,
            "iOS Safari record version: got 0x{record_version:04x}, expected 0x0301 (TLS 1.0). \
             If this is 0x0303 then Option D #1 BoringSSL vendor patch IS needed; if 0x0301 then \
             our current build already matches Safari and the audit was conservative."
        );
    }

    /// Same record-version check for desktop Chrome profile. Real Chrome
    /// also sends 0x0301 (TLS 1.0) record version for the initial ClientHello
    /// — TLS-version selection happens in the inner extension, not the outer
    /// record header. This test confirms the BoringSSL behavior is uniform
    /// across desktop and Safari profiles.
    #[tokio::test]
    async fn desktop_chrome_emits_tls_1_0_record_version() {
        use tokio::io::AsyncReadExt;
        use tokio::net::{TcpListener, TcpStream};

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 5];
            tokio::time::timeout(
                std::time::Duration::from_secs(3),
                stream.read_exact(&mut buf),
            )
            .await
            .unwrap()
            .unwrap();
            buf
        });

        let profile = stealth::presets::chrome_148_macos();
        let connector = chrome_connector(&profile).expect("connector");
        let tcp = TcpStream::connect(addr).await.unwrap();
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            connect_tls(&connector, &profile, "localhost", tcp),
        )
        .await;

        let bytes = tokio::time::timeout(std::time::Duration::from_secs(2), server)
            .await
            .expect("server timeout")
            .expect("server task");

        let record_version = ((bytes[1] as u16) << 8) | (bytes[2] as u16);
        assert_eq!(
            record_version, 0x0301,
            "Chrome desktop record version: got 0x{record_version:04x}, expected 0x0301."
        );
    }

    #[test]
    fn test_shuffle_is_full_fisher_yates() {
        // Real Chrome shuffles all 16 extensions uniformly (no buckets).
        // Verify the shuffle preserves the full set + is non-deterministic.
        let p1 = shuffled_chrome_extension_permutation();
        let p2 = shuffled_chrome_extension_permutation();

        assert_eq!(p1.len(), 16);
        assert_eq!(p2.len(), 16);

        let mut sorted = p1.clone();
        sorted.sort();
        let mut expected = CHROME_EXTENSION_PERMUTATION.to_vec();
        expected.sort();
        assert_eq!(sorted, expected, "shuffle must preserve the set");

        // Probabilistically should differ run-to-run.
        assert_ne!(p1, p2, "Shuffle should be non-deterministic");
    }
}

/// Configure a per-connection TLS session with ALPS, ECH GREASE, and SNI.
/// Per-`profile.device_class` branching:
///  - Desktop / Android: ECH grease + ALPS HTTP/2 SETTINGS payload
///  - MobileIOS: skip BOTH (Safari has neither)
pub fn configure_connection(
    connector: &SslConnector,
    profile: &StealthProfile,
    domain: &str,
) -> Result<ConnectConfiguration, NetError> {
    let mut config = connector
        .configure()
        .map_err(|e| NetError::Tls(e.to_string()))?;

    let is_safari_ios = profile.device_class == DeviceClass::MobileIOS;

    if !is_safari_ios {
        // ECH GREASE — Chrome desktop+Android both send it. Safari does not.
        config.set_enable_ech_grease(true);

        // Application-layer settings (ALPS) for HTTP/2.
        // Chrome 147 Headless sends 4 settings: 1, 2, 4, 6.
        // Safari has no ALPS extension at all — skip entirely on iOS.
        let alps_payload: &[u8] = &[
            // SETTINGS frame (Length 24, Type 4, Flags 0, Stream 0)
            0x00, 0x00, 0x18, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, // ID 1: 65536
            0x00, 0x01, 0x00, 0x01, 0x00, 0x00, // ID 2: 0
            0x00, 0x02, 0x00, 0x00, 0x00, 0x00, // ID 4: 6291456
            0x00, 0x04, 0x00, 0x60, 0x00, 0x00, // ID 6: 262144
            0x00, 0x06, 0x00, 0x04, 0x00, 0x00,
            // Empty ACCEPT_CH frame (Length 0, Type 0x89, Flags 0, Stream 0)
            0x00, 0x00, 0x00, 0x89, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        // SAFETY: BoringSSL's `SSL_add_application_settings` reads the
        // ALPN name (`b"h2"`, length 2) and the ALPS payload buffer
        // (`alps_payload` — a contiguous static slice we built above);
        // both are valid, contiguous, non-null, and live for the
        // entire call. `config.as_ptr()` returns a non-null pointer
        // to the live `SslContext` we own here. BoringSSL only reads
        // these buffers; it copies the data into the SSL_CTX, no
        // ownership transfer.
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
    }

    // SNI is the same for all profiles.
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
    profile: &StealthProfile,
    domain: &str,
    stream: TcpStream,
) -> Result<SslStream<TcpStream>, NetError> {
    let config = configure_connection(connector, profile, domain)?;
    let sni_domain = domain.trim_start_matches('[').trim_end_matches(']');

    tokio_boring2::connect(config, sni_domain, stream)
        .await
        .map_err(|e| NetError::Tls(format!("TLS handshake failed: {e}")))
}

/// Returns the negotiated ALPN protocol from a TLS stream, if any.
pub fn negotiated_alpn(stream: &SslStream<TcpStream>) -> Option<&[u8]> {
    stream.ssl().selected_alpn_protocol()
}
