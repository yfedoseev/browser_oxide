//! HTTP/2 client with Chrome 147 SETTINGS fingerprint.
//!
//! Uses the `http2` crate (wreq's fork of h2) which supports custom
//! SETTINGS order, pseudo-header order, and stream priority — all
//! required for anti-bot fingerprint matching.
//!
//! Verified byte-for-byte against a fresh Chrome 147 (147.0.0.0)
//! capture on macOS arm64 from `tls.peet.ws/api/all` via Playwright
//! MCP, 2026-04-29:
//! ```text
//! akamai_fingerprint: "1:65536;2:0;4:6291456;6:262144|15663105|0|m,a,s,p"
//! priority: { weight: 256, depends_on: 0, exclusive: 1 }
//! ```

use bytes::Bytes;
use http2::client::{Builder, Connection, SendRequest};
use http2::frame::{PseudoId, PseudoOrder, SettingId, SettingsOrder, StreamDependency, StreamId};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::error::NetError;

/// Chrome HTTP/2 SETTINGS values.
///
/// **Verified against a fresh Chrome 147 capture** from the developer's
/// machine via Playwright MCP → `tls.peet.ws/api/all`, 2026-04-29:
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
// `initial_connection_window_size` is the lib's CONFIGURED target — the
// http2 lib sends a WINDOW_UPDATE of (target - 65535) on the wire to
// raise the connection window from the protocol default (65535) up to
// the configured value. So 15_728_640 here → 15_663_105 on the wire,
// which is what real Chrome 147 emits. Verified against wreq-util's
// chrome profile (the gold-standard Rust impl,
// `0x676e67/wreq-util/src/emulate/profile/chrome/http2.rs`).
const INITIAL_CONNECTION_WINDOW_SIZE: u32 = 15_728_640; // → wire 15_663_105 = Chrome match

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

    // Chrome 130+ sends SETTINGS in a specific order on the wire. The lib
    // uses this `SettingsOrder` to determine the position of each ID in
    // the SETTINGS frame, regardless of which ones actually carry a value.
    // wreq-util's chrome profile (`0x676e67/wreq-util/src/emulate/profile/chrome/http2.rs`)
    // — gold-standard Rust impersonator — uses the 8-entry order below for
    // Chrome v100 through v146+. macOS Chrome 130+ emits a SUBSET of these
    // (typically 1, 2, 4, 6) but the order field still must include all 8
    // for the SETTINGS frame layout to match Chrome's expectation when one
    // of the larger entries gets sent (e.g. `0x9 NO_RFC7540_PRIORITIES`
    // on Windows/Linux Chrome 130+, used by Akamai for bot detection).
    //
    // Earlier in this session we sent only [1, 2, 4, 6]. That matches
    // mac Chrome 143+'s ON-WIRE values, BUT a different `SettingsOrder`
    // configuration — specifically NOT including 3, 5, 8, 9 — produces a
    // different Akamai-FP hash than real Chrome (which knows the order
    // even if the values aren't sent).
    let settings_order = SettingsOrder::builder()
        .push(SettingId::HeaderTableSize)
        .push(SettingId::EnablePush)
        .push(SettingId::MaxConcurrentStreams)
        .push(SettingId::InitialWindowSize)
        .push(SettingId::MaxFrameSize)
        .push(SettingId::MaxHeaderListSize)
        .push(SettingId::EnableConnectProtocol)
        .push(SettingId::NoRfc7540Priorities)
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
            // Chrome 147 sends weight 256 (wire byte 255), exclusive=true,
            // depends_on=0 — verified against the 2026-04-29 Chrome 147
            // capture from `tls.peet.ws/api/all` (priority block:
            // `weight: 256, depends_on: 0, exclusive: 1`). The earlier
            // value 219 in this file was for Chrome 130–146 per
            // wreq-util's gold-standard reference, but Chrome 147 reverts
            // to weight 256.
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
