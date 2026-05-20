//! HTTP/2 frame byte-equivalence regression test (gap #32).
//!
//! Asserts that `net::h2_client::handshake` writes the expected PREFACE +
//! SETTINGS + WINDOW_UPDATE bytes to the wire. These byte sequences are
//! what Akamai's HTTP/2 fingerprint hashes produce
//! `52d84b11737d980aef856699f885ca86` (Chrome 146).
//!
//! Bypasses TLS by feeding `handshake()` a raw `TcpStream` so we can read
//! the wire bytes directly on the listener side.
//!
//! The HEADERS frame body is HPACK-Huffman encoded — that frame's
//! *order* is asserted via the per-profile JA4H test
//! (`ja4h::tests::ja4h_*`), not byte-compared here.

use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

/// HTTP/2 frame types we expect.
const FRAME_TYPE_SETTINGS: u8 = 0x04;
const FRAME_TYPE_WINDOW_UPDATE: u8 = 0x08;

/// SETTINGS identifiers per RFC 9113.
const SETTING_HEADER_TABLE_SIZE: u16 = 0x0001;
const SETTING_ENABLE_PUSH: u16 = 0x0002;
const SETTING_INITIAL_WINDOW_SIZE: u16 = 0x0004;
const SETTING_MAX_HEADER_LIST_SIZE: u16 = 0x0006;

/// Chrome 146 expected values (h2_client.rs:32–36).
const EXPECTED_HEADER_TABLE_SIZE: u32 = 65_536;
const EXPECTED_ENABLE_PUSH: u32 = 0;
const EXPECTED_INITIAL_WINDOW_SIZE: u32 = 6_291_456;
const EXPECTED_MAX_HEADER_LIST_SIZE: u32 = 262_144;
/// Connection-level WINDOW_UPDATE delta = target (15_728_640) − default (65_535) = 15_663_105.
const EXPECTED_CONNECTION_WINDOW_DELTA: u32 = 15_663_105;

#[tokio::test]
async fn h2_handshake_writes_chrome_146_settings_and_window_update() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Server side: accept and read raw bytes. We read enough to capture
    // PREFACE (24) + SETTINGS frame header (9) + SETTINGS payload (24,
    // for 4 settings) + WINDOW_UPDATE frame header (9) + WINDOW_UPDATE
    // payload (4) = 70 bytes minimum. Read up to 256 to tolerate any
    // additional client-side preamble.
    let server = tokio::spawn(async move {
        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = vec![0u8; 256];
        let mut total = 0;
        let deadline = tokio::time::sleep(Duration::from_secs(2));
        tokio::pin!(deadline);

        // Read PREFACE (24 bytes) first so we know the client is up.
        while total < 24 {
            tokio::select! {
                _ = &mut deadline => break,
                r = sock.read(&mut buf[total..]) => {
                    match r {
                        Ok(0) => break,
                        Ok(n) => total += n,
                        Err(_) => break,
                    }
                }
            }
        }

        // Send a minimal server SETTINGS frame (empty payload) so the
        // client's `handshake().await` can complete and its SETTINGS+
        // WINDOW_UPDATE get flushed. Frame: length=0, type=0x04, flags=0,
        // stream_id=0.
        let server_settings: [u8; 9] = [0, 0, 0, 0x04, 0, 0, 0, 0, 0];
        let _ = sock.write_all(&server_settings).await;
        let _ = sock.flush().await;

        // Now read the rest of the client's frames.
        while total < 70 {
            tokio::select! {
                _ = &mut deadline => break,
                r = sock.read(&mut buf[total..]) => {
                    match r {
                        Ok(0) => break,
                        Ok(n) => total += n,
                        Err(_) => break,
                    }
                }
            }
        }
        buf.truncate(total);
        buf
    });

    // Client side: connect plain TCP (no TLS), run handshake, and drive
    // the connection so writes flush.
    let _client = tokio::spawn(async move {
        let tcp = TcpStream::connect(addr).await.unwrap();
        if let Ok((_sender, conn)) = net::h2_client::handshake(tcp).await {
            // Drive the connection — without polling, no frames are written.
            let _ = conn.await;
        }
    });

    let bytes = server.await.unwrap();
    assert!(
        bytes.len() >= 70,
        "captured fewer than 70 bytes ({}), can't validate frames: {:02x?}",
        bytes.len(),
        &bytes[..bytes.len().min(40)]
    );

    // ---- 1. PREFACE: bytes 0..24 ----
    assert_eq!(&bytes[..24], PREFACE, "HTTP/2 connection preface mismatch");

    // ---- 2. SETTINGS frame header: bytes 24..33 ----
    // 9-byte header: length(3) | type(1) | flags(1) | stream_id(4)
    let settings_hdr = &bytes[24..33];
    let settings_len =
        u32::from_be_bytes([0, settings_hdr[0], settings_hdr[1], settings_hdr[2]]) as usize;
    assert_eq!(
        settings_hdr[3], FRAME_TYPE_SETTINGS,
        "first frame is not SETTINGS, got type 0x{:02x}",
        settings_hdr[3]
    );
    assert_eq!(
        settings_hdr[4], 0,
        "SETTINGS frame must have flags=0 (not ACK)"
    );
    assert_eq!(
        u32::from_be_bytes([
            settings_hdr[5],
            settings_hdr[6],
            settings_hdr[7],
            settings_hdr[8]
        ]) & 0x7FFF_FFFF,
        0,
        "SETTINGS frame must use stream_id=0"
    );
    assert_eq!(
        settings_len, 24,
        "Chrome SETTINGS frame must contain exactly 4 settings (24 bytes), got {settings_len}"
    );

    // ---- 3. SETTINGS payload: bytes 33..57 ----
    let settings_payload = &bytes[33..33 + settings_len];
    let settings: Vec<(u16, u32)> = settings_payload
        .chunks_exact(6)
        .map(|s| {
            let id = u16::from_be_bytes([s[0], s[1]]);
            let v = u32::from_be_bytes([s[2], s[3], s[4], s[5]]);
            (id, v)
        })
        .collect();
    assert_eq!(
        settings.len(),
        4,
        "expected 4 settings, got {}",
        settings.len()
    );

    // Order: 1, 2, 4, 6 (Chrome 146)
    assert_eq!(
        settings[0],
        (SETTING_HEADER_TABLE_SIZE, EXPECTED_HEADER_TABLE_SIZE),
        "SETTINGS[0] must be HEADER_TABLE_SIZE=65536"
    );
    assert_eq!(
        settings[1],
        (SETTING_ENABLE_PUSH, EXPECTED_ENABLE_PUSH),
        "SETTINGS[1] must be ENABLE_PUSH=0"
    );
    assert_eq!(
        settings[2],
        (SETTING_INITIAL_WINDOW_SIZE, EXPECTED_INITIAL_WINDOW_SIZE),
        "SETTINGS[2] must be INITIAL_WINDOW_SIZE=6291456"
    );
    assert_eq!(
        settings[3],
        (SETTING_MAX_HEADER_LIST_SIZE, EXPECTED_MAX_HEADER_LIST_SIZE),
        "SETTINGS[3] must be MAX_HEADER_LIST_SIZE=262144"
    );

    // ---- 4. WINDOW_UPDATE frame: bytes 57..70 ----
    let wu_hdr = &bytes[57..66];
    let wu_len = u32::from_be_bytes([0, wu_hdr[0], wu_hdr[1], wu_hdr[2]]) as usize;
    assert_eq!(
        wu_hdr[3], FRAME_TYPE_WINDOW_UPDATE,
        "second frame is not WINDOW_UPDATE, got type 0x{:02x}",
        wu_hdr[3]
    );
    assert_eq!(wu_len, 4, "WINDOW_UPDATE payload must be 4 bytes");
    assert_eq!(
        u32::from_be_bytes([wu_hdr[5], wu_hdr[6], wu_hdr[7], wu_hdr[8]]) & 0x7FFF_FFFF,
        0,
        "Connection-level WINDOW_UPDATE must use stream_id=0"
    );

    let wu_delta = u32::from_be_bytes([bytes[66], bytes[67], bytes[68], bytes[69]]) & 0x7FFF_FFFF;
    assert_eq!(
        wu_delta, EXPECTED_CONNECTION_WINDOW_DELTA,
        "Connection WINDOW_UPDATE delta must be 15_663_105 (Chrome 146)"
    );
}
