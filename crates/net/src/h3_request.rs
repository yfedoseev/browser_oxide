//! HTTP/3 request execution using the h3 crate over quinn.

use crate::error::NetError;
use crate::Response;
use bytes::Buf;
use std::collections::HashMap;

/// Send an HTTP/3 GET request over an established QUIC connection.
pub async fn h3_get(
    connection: quinn::Connection,
    url: &url::Url,
    extra_headers: &[(String, String)],
) -> Result<(Response, Option<String>), NetError> {
    let h3_conn = h3_quinn::Connection::new(connection);
    let (mut driver, mut send_request) = h3::client::new(h3_conn)
        .await
        .map_err(|e| NetError::H3(e.to_string()))?;

    // Drive the connection in the background
    tokio::spawn(async move {
        let _ = futures_util::future::poll_fn(|cx| driver.poll_close(cx)).await;
    });

    // Build request
    let authority = url.authority();
    let path = if let Some(q) = url.query() {
        format!("{}?{}", url.path(), q)
    } else {
        url.path().to_string()
    };

    let mut req = http::Request::builder()
        .method("GET")
        .uri(format!("https://{}{}", authority, path))
        .header("host", authority)
        .header(":authority", authority)
        .header(
            "accept",
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        )
        .header("accept-encoding", "gzip, deflate, br")
        .header("accept-language", "en-US,en;q=0.9")
        .header("user-agent", "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36");

    for (k, v) in extra_headers {
        req = req.header(k.as_str(), v.as_str());
    }

    let req = req.body(()).map_err(|e| NetError::H3(e.to_string()))?;

    let mut stream = send_request
        .send_request(req)
        .await
        .map_err(|e| NetError::H3(e.to_string()))?;

    stream
        .finish()
        .await
        .map_err(|e| NetError::H3(e.to_string()))?;

    let resp = stream
        .recv_response()
        .await
        .map_err(|e| NetError::H3(e.to_string()))?;

    let status = resp.status().as_u16();
    let status_text = resp.status().canonical_reason().unwrap_or("").to_string();

    // Collect headers. Set-Cookie is kept separate so multi-value cookies aren't
    // collapsed by the HashMap.
    let mut headers = HashMap::new();
    let mut set_cookies = Vec::new();
    let mut alt_svc_value = None;
    for (key, value) in resp.headers() {
        if let Ok(v) = value.to_str() {
            let k = key.to_string();
            if k.eq_ignore_ascii_case("set-cookie") {
                set_cookies.push(v.to_string());
                continue;
            }
            if k == "alt-svc" {
                alt_svc_value = Some(v.to_string());
            }
            headers.insert(k, v.to_string());
        }
    }

    // Read body
    let mut body = Vec::new();
    while let Some(chunk) = stream
        .recv_data()
        .await
        .map_err(|e| NetError::H3(e.to_string()))?
    {
        let mut buf = chunk;
        while buf.has_remaining() {
            let chunk_bytes = buf.chunk();
            body.extend_from_slice(chunk_bytes);
            let len = chunk_bytes.len();
            buf.advance(len);
        }
    }

    Ok((
        Response {
            status,
            status_text,
            headers,
            set_cookies,
            body,
            url: url.to_string(),
        },
        alt_svc_value,
    ))
}
