//! HTTP/1.1 fallback client using httparse.
//!
//! Used when ALPN negotiates `http/1.1` instead of `h2`.

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::error::NetError;

/// Raw HTTP/1.1 response before decompression.
pub struct RawResponse {
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// Send an HTTP/1.1 GET request over a stream.
pub async fn send_get<S>(
    stream: &mut S,
    host: &str,
    path: &str,
    headers: &[(String, String)],
) -> Result<RawResponse, NetError>
where
    S: AsyncReadExt + AsyncWriteExt + Unpin,
{
    send_request(stream, "GET", host, path, headers, None).await
}

/// Send an HTTP/1.1 POST request over a stream.
pub async fn send_post<S>(
    stream: &mut S,
    host: &str,
    path: &str,
    headers: &[(String, String)],
    body: &[u8],
) -> Result<RawResponse, NetError>
where
    S: AsyncReadExt + AsyncWriteExt + Unpin,
{
    send_request(stream, "POST", host, path, headers, Some(body)).await
}

async fn send_request<S>(
    stream: &mut S,
    method: &str,
    host: &str,
    path: &str,
    headers: &[(String, String)],
    body: Option<&[u8]>,
) -> Result<RawResponse, NetError>
where
    S: AsyncReadExt + AsyncWriteExt + Unpin,
{
    // Build the request
    let mut request = format!("{method} {path} HTTP/1.1\r\nHost: {host}\r\n");
    if let Some(body) = body {
        request.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    for (name, value) in headers {
        request.push_str(&format!("{name}: {value}\r\n"));
    }
    request.push_str("Connection: keep-alive\r\n\r\n");

    stream
        .write_all(request.as_bytes())
        .await
        .map_err(|e| NetError::Http(format!("failed to write request: {e}")))?;

    if let Some(body) = body {
        stream
            .write_all(body)
            .await
            .map_err(|e| NetError::Http(format!("failed to write body: {e}")))?;
    }

    stream
        .flush()
        .await
        .map_err(|e| NetError::Http(format!("failed to flush: {e}")))?;

    // Read the response
    read_response(stream).await
}

async fn read_response<S>(stream: &mut S) -> Result<RawResponse, NetError>
where
    S: AsyncReadExt + Unpin,
{
    let mut buf = Vec::with_capacity(8192);
    let header_len;

    // Read until we find the end of headers (\r\n\r\n)
    loop {
        let mut tmp = [0u8; 4096];
        let n = stream
            .read(&mut tmp)
            .await
            .map_err(|e| NetError::Http(format!("read error: {e}")))?;
        if n == 0 {
            return Err(NetError::Http("connection closed before headers".to_string()));
        }
        buf.extend_from_slice(&tmp[..n]);

        if let Some(pos) = find_header_end(&buf) {
            header_len = pos + 4; // include \r\n\r\n
            break;
        }

        if buf.len() > 65536 {
            return Err(NetError::Http("headers too large".to_string()));
        }
    }

    // Parse headers
    let mut parsed_headers = [httparse::EMPTY_HEADER; 128];
    let mut response = httparse::Response::new(&mut parsed_headers);
    response
        .parse(&buf[..header_len])
        .map_err(|e| NetError::Http(format!("failed to parse response: {e}")))?;

    let status = response.code.unwrap_or(0);
    let status_text = response.reason.unwrap_or("").to_string();

    let mut headers: Vec<(String, String)> = Vec::new();
    let mut content_length: Option<usize> = None;
    let mut chunked = false;

    for header in response.headers.iter() {
        let name = header.name.to_lowercase();
        let value = String::from_utf8_lossy(header.value).to_string();
        if name == "content-length" {
            content_length = value.parse().ok();
        }
        if name == "transfer-encoding" && value.contains("chunked") {
            chunked = true;
        }
        headers.push((name, value));
    }

    // Read the body
    let body_start = &buf[header_len..];
    let body = if chunked {
        read_chunked_body(stream, body_start).await?
    } else if let Some(len) = content_length {
        read_content_length_body(stream, body_start, len).await?
    } else {
        // Read until connection close
        read_until_close(stream, body_start).await?
    };

    Ok(RawResponse {
        status,
        status_text,
        headers,
        body,
    })
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

async fn read_content_length_body<S>(
    stream: &mut S,
    initial: &[u8],
    content_length: usize,
) -> Result<Vec<u8>, NetError>
where
    S: AsyncReadExt + Unpin,
{
    let mut body = Vec::with_capacity(content_length);
    body.extend_from_slice(initial);

    while body.len() < content_length {
        let mut tmp = vec![0u8; std::cmp::min(8192, content_length - body.len())];
        let n = stream
            .read(&mut tmp)
            .await
            .map_err(|e| NetError::Http(format!("body read error: {e}")))?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&tmp[..n]);
    }

    Ok(body)
}

async fn read_chunked_body<S>(
    stream: &mut S,
    initial: &[u8],
) -> Result<Vec<u8>, NetError>
where
    S: AsyncReadExt + Unpin,
{
    let mut raw = Vec::from(initial);
    let mut decoded = Vec::new();

    loop {
        // Ensure we have enough data for the next chunk header
        while !contains_crlf(&raw) {
            let mut tmp = [0u8; 4096];
            let n = stream
                .read(&mut tmp)
                .await
                .map_err(|e| NetError::Http(format!("chunked read error: {e}")))?;
            if n == 0 {
                return Ok(decoded);
            }
            raw.extend_from_slice(&tmp[..n]);
        }

        // Parse chunk size
        let crlf_pos = raw.windows(2).position(|w| w == b"\r\n").unwrap();
        let size_str = std::str::from_utf8(&raw[..crlf_pos])
            .map_err(|e| NetError::Http(format!("invalid chunk size: {e}")))?;
        // Handle chunk extensions (size;ext=val)
        let size_str = size_str.split(';').next().unwrap_or(size_str).trim();
        let chunk_size = usize::from_str_radix(size_str, 16)
            .map_err(|e| NetError::Http(format!("invalid chunk size '{size_str}': {e}")))?;

        if chunk_size == 0 {
            break; // Last chunk
        }

        // Consume the size line
        raw = raw[crlf_pos + 2..].to_vec();

        // Read chunk data
        while raw.len() < chunk_size + 2 {
            let mut tmp = [0u8; 8192];
            let n = stream
                .read(&mut tmp)
                .await
                .map_err(|e| NetError::Http(format!("chunk data read error: {e}")))?;
            if n == 0 {
                return Err(NetError::Http("unexpected EOF in chunked body".to_string()));
            }
            raw.extend_from_slice(&tmp[..n]);
        }

        decoded.extend_from_slice(&raw[..chunk_size]);
        raw = raw[chunk_size + 2..].to_vec(); // skip data + trailing \r\n
    }

    Ok(decoded)
}

fn contains_crlf(buf: &[u8]) -> bool {
    buf.windows(2).any(|w| w == b"\r\n")
}

async fn read_until_close<S>(
    stream: &mut S,
    initial: &[u8],
) -> Result<Vec<u8>, NetError>
where
    S: AsyncReadExt + Unpin,
{
    let mut body = Vec::from(initial);
    loop {
        let mut tmp = [0u8; 8192];
        let n = stream
            .read(&mut tmp)
            .await
            .map_err(|e| NetError::Http(format!("read error: {e}")))?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&tmp[..n]);
    }
    Ok(body)
}
