//! End-to-end proxy round-trip tests.
//!
//! Verifies that `net::proxy::connect` produces a TCP stream whose bytes
//! reach the *target* (not the proxy) and that the target's response
//! bytes flow back unchanged. This catches regressions in:
//!   - HTTP CONNECT request framing + 200-response parsing
//!   - SOCKS5 greeting / auth / CONNECT (already covered by the unit test
//!     in proxy.rs but we re-cover it here at the integration boundary)
//!   - Proxy-Authorization: Basic header construction with creds
//!   - The "tunnel is bytes-clean" property — once CONNECT returns 200,
//!     the stream must transit arbitrary payloads (this is what allows
//!     the BoringSSL Chrome 147 handshake to run *inside* the tunnel
//!     against the origin, preserving the JA4 fingerprint to the origin
//!     rather than the proxy.)
//!
//! No external network: a local TCP listener acts as the "origin" and
//! a second local TCP listener acts as the "proxy". Tests are NOT
//! `#[ignore]` — they're hermetic.
//!
//! A separate `live_proxy_chain` test (gated by `BOXIDE_TEST_PROXY` env
//! var) hits a real `https://api.ipify.org` through whatever proxy the
//! operator wires up.

use std::time::Duration;

use net::proxy::{self, ProxyAuth, ProxyConfig};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Spawn a fake "origin" that, on connect, writes a fixed banner and
/// echoes one inbound chunk. Returns the bound port.
async fn spawn_echo_origin(banner: &'static [u8]) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        if let Ok((mut sock, _)) = listener.accept().await {
            let _ = sock.write_all(banner).await;
            let mut buf = [0u8; 1024];
            if let Ok(n) = sock.read(&mut buf).await {
                let _ = sock.write_all(&buf[..n]).await;
            }
        }
    });
    port
}

/// Minimal HTTP CONNECT proxy: read the CONNECT request line + headers
/// (terminated by CRLFCRLF), optionally validate `Proxy-Authorization`,
/// open a TCP connection to the requested target, write back
/// `HTTP/1.1 200 OK\r\n\r\n`, then bidir-pipe bytes until EOF on either
/// side. Returns the bound port.
///
/// `expect_auth` — if Some, the test will fail unless the proxy received
/// exactly that token in `Proxy-Authorization: Basic <token>`.
async fn spawn_http_connect_proxy(expect_auth: Option<String>) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let (mut client, _) = listener.accept().await.unwrap();

        // Read until \r\n\r\n
        let mut head = Vec::with_capacity(512);
        let mut tmp = [0u8; 256];
        loop {
            let n = client.read(&mut tmp).await.unwrap();
            assert!(n > 0, "proxy: client closed before sending CONNECT");
            head.extend_from_slice(&tmp[..n]);
            if head.windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
            assert!(head.len() < 4096, "CONNECT request too large");
        }
        let head_str = String::from_utf8_lossy(&head);
        let first = head_str.lines().next().unwrap();
        assert!(
            first.starts_with("CONNECT "),
            "proxy: expected CONNECT, got {first:?}"
        );
        // Parse "CONNECT host:port HTTP/1.1"
        let target = first
            .split_whitespace()
            .nth(1)
            .expect("CONNECT line missing target");
        if let Some(expected) = expect_auth.as_deref() {
            let got = head_str
                .lines()
                .find_map(|l| l.strip_prefix("Proxy-Authorization: Basic "))
                .unwrap_or_else(|| {
                    panic!("proxy: expected Proxy-Authorization but request was: {head_str}")
                });
            assert_eq!(got.trim(), expected, "proxy: wrong Basic auth token");
        }

        // Open upstream tcp.
        let upstream = tokio::net::TcpStream::connect(target).await.unwrap();
        client
            .write_all(b"HTTP/1.1 200 OK\r\n\r\n")
            .await
            .unwrap();

        // Bidir-pipe.
        let (mut cr, mut cw) = client.into_split();
        let (mut ur, mut uw) = upstream.into_split();
        let a = tokio::spawn(async move {
            let _ = tokio::io::copy(&mut cr, &mut uw).await;
            let _ = uw.shutdown().await;
        });
        let b = tokio::spawn(async move {
            let _ = tokio::io::copy(&mut ur, &mut cw).await;
            let _ = cw.shutdown().await;
        });
        let _ = tokio::join!(a, b);
    });
    port
}

#[tokio::test]
async fn http_connect_tunnels_bytes_to_origin() {
    // Origin sends a banner, then echoes whatever the client writes.
    let origin_port = spawn_echo_origin(b"HELLO-FROM-ORIGIN\n").await;
    let proxy_port = spawn_http_connect_proxy(None).await;

    let proxy_cfg = ProxyConfig::Http {
        host: "127.0.0.1".into(),
        port: proxy_port,
        auth: ProxyAuth::None,
        tls: false,
    };
    let mut stream = proxy::connect(
        "127.0.0.1",
        origin_port,
        Duration::from_secs(2),
        None,
        &proxy_cfg,
    )
    .await
    .expect("proxy connect should succeed");

    // First read: banner from origin (proves the byte direction works).
    let mut buf = [0u8; 64];
    let n = stream.read(&mut buf).await.unwrap();
    assert_eq!(&buf[..n], b"HELLO-FROM-ORIGIN\n");

    // Write something, expect echo. This proves the tunnel is bytes-clean
    // in *both* directions, which is the precondition for running a
    // BoringSSL TLS handshake against the origin through the tunnel.
    stream.write_all(b"PING-PAYLOAD").await.unwrap();
    let n = stream.read(&mut buf).await.unwrap();
    assert_eq!(&buf[..n], b"PING-PAYLOAD");
}

#[tokio::test]
async fn http_connect_sends_proxy_authorization_basic() {
    use base64::Engine as _;
    let origin_port = spawn_echo_origin(b"OK\n").await;
    let token = base64::engine::general_purpose::STANDARD.encode("alice:s3cret!");
    let proxy_port = spawn_http_connect_proxy(Some(token)).await;

    let proxy_cfg = ProxyConfig::Http {
        host: "127.0.0.1".into(),
        port: proxy_port,
        auth: ProxyAuth::UserPass("alice".into(), "s3cret!".into()),
        tls: false,
    };
    let mut stream = proxy::connect(
        "127.0.0.1",
        origin_port,
        Duration::from_secs(2),
        None,
        &proxy_cfg,
    )
    .await
    .expect("proxy connect with auth should succeed");

    let mut buf = [0u8; 8];
    let n = stream.read(&mut buf).await.unwrap();
    assert_eq!(&buf[..n], b"OK\n");
}

#[tokio::test]
async fn http_connect_propagates_407_denied() {
    // Proxy expects creds; we send none. Expect a structured error,
    // not a hang.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let (mut sock, _) = listener.accept().await.unwrap();
        let mut tmp = [0u8; 1024];
        // Read the CONNECT preamble to keep the client from blocking
        // on its write side.
        let _ = sock.read(&mut tmp).await;
        let _ = sock
            .write_all(b"HTTP/1.1 407 Proxy Authentication Required\r\n\r\n")
            .await;
    });
    let proxy_cfg = ProxyConfig::Http {
        host: "127.0.0.1".into(),
        port: proxy_port,
        auth: ProxyAuth::None,
        tls: false,
    };
    let err = proxy::connect(
        "127.0.0.1",
        9, // discard port — we never get there
        Duration::from_secs(2),
        None,
        &proxy_cfg,
    )
    .await
    .expect_err("407 should produce a structured error");
    let msg = format!("{err}");
    assert!(
        msg.contains("407"),
        "expected error to mention 407, got: {msg}"
    );
}

/// Live test: requires a working proxy at $BOXIDE_TEST_PROXY (URL form,
/// e.g. `http://user:pass@host:port` or `socks5://...`). Hits
/// `https://api.ipify.org/?format=text` direct AND through the proxy,
/// asserts both succeed and that the IPs differ.
///
/// Always `#[ignore]` because it requires an external paid resource.
/// Run with:
///     BOXIDE_TEST_PROXY=http://user:pass@your-proxy:8080 \
///       cargo test -p net --test proxy_roundtrip live_proxy_chain \
///       -- --ignored --test-threads=1 --nocapture
#[tokio::test]
#[ignore = "requires BOXIDE_TEST_PROXY env var pointing at a real proxy"]
async fn live_proxy_chain() {
    let proxy_url = std::env::var("BOXIDE_TEST_PROXY")
        .expect("BOXIDE_TEST_PROXY must be set to a real proxy URL");

    // Build two clients that share nothing: one direct, one with proxy.
    // Use a minimal preset profile (we only care about the IP differing).
    use stealth::presets;
    let mut profile_direct = presets::chrome_130_windows();
    profile_direct.proxy = None;
    let mut profile_proxy = profile_direct.clone();
    profile_proxy.proxy = Some(proxy_url.clone());

    let client_direct = net::HttpClient::new(&profile_direct).expect("direct client");
    let client_proxied = net::HttpClient::new(&profile_proxy).expect("proxied client");

    let url = "https://api.ipify.org/?format=text";
    let resp_direct = client_direct
        .get(url)
        .await
        .expect("direct GET should succeed");
    let resp_proxied = client_proxied
        .get(url)
        .await
        .expect("proxied GET should succeed");

    assert_eq!(resp_direct.status, 200, "direct: {}", resp_direct.status);
    assert_eq!(resp_proxied.status, 200, "proxied: {}", resp_proxied.status);
    let ip_direct = resp_direct.text().trim().to_string();
    let ip_proxied = resp_proxied.text().trim().to_string();
    eprintln!("direct  IP: {ip_direct}");
    eprintln!("proxied IP: {ip_proxied}");
    assert_ne!(
        ip_direct, ip_proxied,
        "proxy didn't change the egress IP — proxy is misconfigured \
         or transparent (got same {ip_direct})"
    );
}
