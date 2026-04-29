//! HTTP/2 client with Chrome 130 SETTINGS fingerprint.
//!
//! Uses the `http2` crate (wreq's fork of h2) which supports custom
//! SETTINGS order, pseudo-header order, and stream priority — all
//! required for anti-bot fingerprint matching.

use bytes::Bytes;
use http2::client::{Builder, Connection, SendRequest};
use http2::frame::{PseudoId, PseudoOrder, SettingId, SettingsOrder, StreamDependency, StreamId};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::error::NetError;

/// Chrome HTTP/2 SETTINGS values.
///
/// **Verified against a real Chrome 146 capture** from the developer's
/// machine via `tls.peet.ws/api/all`:
/// ```text
/// 1:65536;2:0;4:6291456;6:262144|15663105|0|m,a,s,p
/// ```
/// Only four settings — Chrome does NOT send `3 MAX_CONCURRENT_STREAMS`
/// or `5 MAX_FRAME_SIZE` on its client SETTINGS frame. Those defaults
/// are negotiated from the server side.
///
/// Earlier in this session we incorrectly added both of those based on
/// an out-of-date curl-impersonate config; that made the Akamai H2
/// fingerprint hash `d23e6399a1d185e3b8cb58e5640dd698`, diverging from
/// Chrome's actual hash `52d84b11737d980aef856699f885ca86`. The
/// reference capture corrected us.
const HEADER_TABLE_SIZE: u32 = 65_536; // SETTINGS 1
const ENABLE_PUSH: bool = false; // SETTINGS 2 = 0
const INITIAL_STREAM_WINDOW_SIZE: u32 = 6_291_456; // SETTINGS 4 = 6 MB
const MAX_HEADER_LIST_SIZE: u32 = 262_144; // SETTINGS 6 = 256 KB
// Real Chrome 147 emits WINDOW_UPDATE = 15_663_105 bytes (NOT 15_728_640
// = 15 MB even). The 65,535-byte delta corresponds to the
// (initial_max_data - default_initial_window_size) calculation Chrome
// does internally. Akamai-FP and other H2 fingerprint hashes include the
// exact byte value of WINDOW_UPDATE; rounding to 15 MB lands us in a
// non-Chrome bucket and triggers `_abck=~-1~...` (Akamai BMP "untrusted")
// at the edge before the sensor JS even runs. Closes 9 retail-Akamai
// sites (walmart, target, homedepot, costco, bestbuy, wayfair, h-m,
// uniqlo, zara) per docs/GAP_DEEP_ANALYSIS_2026_04_28.md.
const INITIAL_CONNECTION_WINDOW_SIZE: u32 = 15_663_105; // ~14.94 MB (WINDOW_UPDATE)

/// Perform an HTTP/2 handshake over a TLS stream and return a sender + connection.
///
/// The connection must be driven by spawning it onto a tokio task.
/// The sender is used to send requests.
pub async fn handshake<T>(io: T) -> Result<(SendRequest<Bytes>, Connection<T, Bytes>), NetError>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    // Chrome 130 pseudo-header order: :method, :authority, :scheme, :path
    let pseudo_order = PseudoOrder::builder()
        .push(PseudoId::Method)
        .push(PseudoId::Authority)
        .push(PseudoId::Scheme)
        .push(PseudoId::Path)
        .build();

    // Chrome SETTINGS frame order: 1, 2, 4, 6 (only these four —
    // SETTINGS 3 and 5 are NOT sent by Chrome, verified via
    // tls.peet.ws capture of the developer's Chrome 146 browser).
    let settings_order = SettingsOrder::builder()
        .push(SettingId::HeaderTableSize)
        .push(SettingId::EnablePush)
        .push(SettingId::InitialWindowSize)
        .push(SettingId::MaxHeaderListSize)
        .build();

    let mut builder = Builder::new();
    builder
        .header_table_size(HEADER_TABLE_SIZE)
        .enable_push(ENABLE_PUSH)
        .initial_window_size(INITIAL_STREAM_WINDOW_SIZE)
        .max_header_list_size(MAX_HEADER_LIST_SIZE)
        .initial_connection_window_size(INITIAL_CONNECTION_WINDOW_SIZE)
        .headers_pseudo_order(pseudo_order)
        .settings_order(settings_order)
        .headers_stream_dependency(StreamDependency::new(
            StreamId::zero(),
            // Wire byte 255 represents HTTP/2 weight 256 (RFC 7540 §5.3 —
            // "Add one to the value to obtain a weight between 1 and 256").
            // Chrome 147's PRIORITY frame for the implicit headers stream
            // has wire byte 255 = weight 256, exclusive=true.
            255,
            true,
        ));

    builder
        .handshake(io)
        .await
        .map_err(|e| NetError::Http(format!("HTTP/2 handshake failed: {e}")))
}

/// Send a GET request over an HTTP/2 connection.
///
/// `headers` is an ordered list of (name, value) pairs to include.
pub async fn send_get(
    sender: &mut SendRequest<Bytes>,
    uri: &str,
    _host: &str,
    headers: &[(String, String)],
) -> Result<(http::response::Parts, Vec<u8>), NetError> {
    let mut ready_sender = sender
        .clone()
        .ready()
        .await
        .map_err(|e| NetError::Http(format!("HTTP/2 not ready: {e}")))?;

    // In HTTP/2, :authority is derived from the URI automatically.
    // Do NOT add an explicit `host` header — some servers (nginx) reject it.
    let mut request = http::Request::builder().method(http::Method::GET).uri(uri);

    for (name, value) in headers {
        request = request.header(name.as_str(), value.as_str());
    }

    let request = request
        .body(())
        .map_err(|e| NetError::Http(format!("failed to build request: {e}")))?;

    let (response, _) = ready_sender
        .send_request(request, true) // true = end of stream (no body)
        .map_err(|e| NetError::Http(format!("failed to send request: {e}")))?;

    let response = response
        .await
        .map_err(|e| NetError::Http(format!("HTTP/2 response error: {e}")))?;

    let (parts, mut body) = response.into_parts();

    // Read the response body
    let mut data = Vec::new();
    while let Some(chunk) = body.data().await {
        let chunk = chunk.map_err(|e| NetError::Http(format!("body read error: {e}")))?;
        let _ = body.flow_control().release_capacity(chunk.len());
        data.extend_from_slice(&chunk);
    }

    Ok((parts, data))
}

/// Send a POST request over an HTTP/2 connection.
pub async fn send_post(
    sender: &mut SendRequest<Bytes>,
    uri: &str,
    _host: &str,
    headers: &[(String, String)],
    body: &[u8],
) -> Result<(http::response::Parts, Vec<u8>), NetError> {
    let mut ready_sender = sender
        .clone()
        .ready()
        .await
        .map_err(|e| NetError::Http(format!("HTTP/2 not ready: {e}")))?;

    let mut request = http::Request::builder()
        .method(http::Method::POST)
        .uri(uri)
        .header("content-length", body.len().to_string());

    for (name, value) in headers {
        request = request.header(name.as_str(), value.as_str());
    }

    let request = request
        .body(())
        .map_err(|e| NetError::Http(format!("failed to build request: {e}")))?;

    let (response, mut send_stream) = ready_sender
        .send_request(request, false) // false = body follows
        .map_err(|e| NetError::Http(format!("failed to send request: {e}")))?;

    // Send the body
    send_stream
        .send_data(Bytes::copy_from_slice(body), true)
        .map_err(|e| NetError::Http(format!("failed to send body: {e}")))?;

    let response = response
        .await
        .map_err(|e| NetError::Http(format!("HTTP/2 response error: {e}")))?;

    let (parts, mut resp_body) = response.into_parts();

    let mut data = Vec::new();
    while let Some(chunk) = resp_body.data().await {
        let chunk = chunk.map_err(|e| NetError::Http(format!("body read error: {e}")))?;
        let _ = resp_body.flow_control().release_capacity(chunk.len());
        data.extend_from_slice(&chunk);
    }

    Ok((parts, data))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // requires network
    async fn h2_get_httpbin() {
        let connector = crate::tls::chrome_connector().unwrap();
        let tcp = crate::tcp::connect("httpbin.org", 443, std::time::Duration::from_secs(10))
            .await
            .unwrap();
        let tls = crate::tls::connect_tls(&connector, "httpbin.org", tcp)
            .await
            .unwrap();

        // Verify ALPN negotiated h2
        assert_eq!(crate::tls::negotiated_alpn(&tls), Some(b"h2".as_slice()));

        let (mut sender, conn) = handshake(tls).await.unwrap();
        tokio::spawn(async move {
            if let Err(e) = conn.await {
                eprintln!("H2 connection error: {e}");
            }
        });

        let (parts, body) = send_get(&mut sender, "https://httpbin.org/get", "httpbin.org", &[])
            .await
            .unwrap();

        assert_eq!(parts.status, 200);
        assert!(!body.is_empty());
        let text = String::from_utf8_lossy(&body);
        assert!(text.contains("httpbin.org"), "Response: {text}");
    }
}
