//! Response body decompression based on Content-Encoding header.

use std::io::Read;

use crate::net::error::NetError;

/// Decompress a response body based on the Content-Encoding value.
pub fn decompress(body: &[u8], encoding: &str) -> Result<Vec<u8>, NetError> {
    match encoding {
        "gzip" => decompress_gzip(body),
        "deflate" => decompress_deflate(body),
        "br" => decompress_brotli(body),
        "zstd" => decompress_zstd(body),
        "" | "identity" => Ok(body.to_vec()),
        other => Err(NetError::Http(format!("unsupported encoding: {other}"))),
    }
}

fn decompress_gzip(body: &[u8]) -> Result<Vec<u8>, NetError> {
    let mut decoder = flate2::read::GzDecoder::new(body);
    let mut out = Vec::new();
    decoder
        .read_to_end(&mut out)
        .map_err(|e| NetError::Http(format!("gzip decompression failed: {e}")))?;
    Ok(out)
}

fn decompress_deflate(body: &[u8]) -> Result<Vec<u8>, NetError> {
    let mut decoder = flate2::read::DeflateDecoder::new(body);
    let mut out = Vec::new();
    decoder
        .read_to_end(&mut out)
        .map_err(|e| NetError::Http(format!("deflate decompression failed: {e}")))?;
    Ok(out)
}

fn decompress_brotli(body: &[u8]) -> Result<Vec<u8>, NetError> {
    let mut decoder = brotli::Decompressor::new(body, 4096);
    let mut out = Vec::new();
    match decoder.read_to_end(&mut out) {
        Ok(_) => Ok(out),
        Err(e) => {
            // Fallback: some servers (yandex, a few CDNs) advertise Content-Encoding: br
            // in headers but actually send identity body when the body is already small
            // or when an intermediary strips the encoding. If the body looks like HTML/text
            // or is empty, return it as-is instead of failing the whole request.
            if body.is_empty() {
                return Ok(Vec::new());
            }
            let looks_like_text = body
                .iter()
                .take(32)
                .all(|b| b.is_ascii_graphic() || b.is_ascii_whitespace());
            if looks_like_text {
                return Ok(body.to_vec());
            }
            Err(NetError::Http(format!(
                "brotli decompression failed: {e} (body len={}, first 8 bytes={:02x?})",
                body.len(),
                &body[..body.len().min(8)]
            )))
        }
    }
}

fn decompress_zstd(body: &[u8]) -> Result<Vec<u8>, NetError> {
    let out = zstd::stream::decode_all(body)
        .map_err(|e| NetError::Http(format!("zstd decompression failed: {e}")))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn gzip_roundtrip() {
        let original = b"hello world, this is a test of gzip compression";
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(original).unwrap();
        let compressed = encoder.finish().unwrap();
        let result = decompress(&compressed, "gzip").unwrap();
        assert_eq!(result, original);
    }

    #[test]
    fn brotli_roundtrip() {
        let original = b"hello world, this is a test of brotli compression";
        let mut compressed = Vec::new();
        {
            let mut encoder = brotli::CompressorWriter::new(&mut compressed, 4096, 6, 22);
            encoder.write_all(original).unwrap();
        }
        let result = decompress(&compressed, "br").unwrap();
        assert_eq!(result, original);
    }

    #[test]
    fn identity_passthrough() {
        let data = b"no compression";
        let result = decompress(data, "identity").unwrap();
        assert_eq!(result, data);
    }

    #[test]
    fn empty_encoding_passthrough() {
        let data = b"no encoding header";
        let result = decompress(data, "").unwrap();
        assert_eq!(result, data);
    }
}
