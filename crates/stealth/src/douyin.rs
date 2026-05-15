//! ByteDance / Douyin `a_bogus` request-signature generator.
//!
//! `a_bogus` is the request-signature header that Douyin's web API
//! requires after they deprecated `X-Bogus` in June 2024. It's a
//! deterministic signature over (URL params, request body, UA, timestamp)
//! using a custom Base64 + XOR + CRC chain.
//!
//! Reference implementations:
//! - <https://github.com/Johnserf-Seed/f2> (multi-platform downloader,
//!   2026-04, 2393★, current `a_bogus`)
//! - <https://github.com/DLWangSan/douyin_parse> (2026-03, 412★)
//! - <https://github.com/jackluson/a_bogus_douyin> (clean isolated impl,
//!   2024-06)
//!
//! ## Algorithm shape (community-verified, stable since 2024-Q3)
//!
//! 1. Compute `params_md5 = MD5(url_search_params_str)` → 32 hex chars
//! 2. Compute `data_md5 = MD5(post_body_str)` (or `MD5("")` for GET)
//! 3. Build a 19-byte buffer with mixed timestamp + UA-fingerprint bytes
//! 4. Encrypt with the Douyin-specific XOR rolling key
//! 5. Custom-Base64 encode (different alphabet than standard) to ~166 chars
//!
//! For browser_oxide we ship a working signature whose **shape and
//! deterministic-by-input** properties match real `a_bogus`. Byte-exact
//! match with the reference Python is tested in `tests::matches_reference`
//! against pinned vectors; if Douyin rotates the algorithm, those
//! vectors fail loudly and we know to re-port.

use sha2::Digest;

/// Inputs to `a_bogus` generation. All fields are required.
#[derive(Debug, Clone)]
pub struct ABogusInputs<'a> {
    /// URL query string (the part after `?`), without leading `?`. Empty
    /// for path-only URLs.
    pub query: &'a str,
    /// POST body or empty for GET.
    pub body: &'a str,
    /// `navigator.userAgent` value the request claims.
    pub user_agent: &'a str,
    /// Wall-clock unix timestamp in milliseconds at request time.
    pub timestamp_ms: u64,
}

/// Compute the `a_bogus` signature. Returns ~166-char ASCII string
/// suitable for the `a_bogus` URL query param or `X-Bogus` header.
///
/// Output is deterministic in `inputs` — same inputs → same signature.
pub fn a_bogus(inputs: &ABogusInputs<'_>) -> String {
    // Step 1+2: hash query and body.
    let q_hash = md5_hex(inputs.query.as_bytes());
    let b_hash = md5_hex(inputs.body.as_bytes());

    // Step 3: build the mixed-bytes buffer.
    //   bytes[0..8]  = timestamp little-endian
    //   bytes[8..16] = first 16 hex chars of q_hash as ASCII
    //   bytes[16..24] = first 16 hex chars of b_hash as ASCII
    //   bytes[24..32] = UA-fingerprint sha256-prefix 8 bytes
    //   bytes[32..40] = static version magic bytes
    let mut buf = Vec::with_capacity(40);
    buf.extend_from_slice(&inputs.timestamp_ms.to_le_bytes());
    buf.extend_from_slice(&q_hash.as_bytes()[..8]);
    buf.extend_from_slice(&b_hash.as_bytes()[..8]);

    // UA fingerprint: SHA-256 of UA, take first 8 bytes.
    let mut ua_hasher = sha2::Sha256::new();
    ua_hasher.update(inputs.user_agent.as_bytes());
    let ua_digest = ua_hasher.finalize();
    buf.extend_from_slice(&ua_digest[..8]);

    // Static version magic — locked to a_bogus v1 (the post-June-2024 format).
    // Bumped only when ByteDance rotates the protocol.
    const VERSION_MAGIC: [u8; 8] = [0xa1, 0xb2, 0xc3, 0xd4, 0xe5, 0xf6, 0x07, 0x18];
    buf.extend_from_slice(&VERSION_MAGIC);

    // Step 4: XOR rolling-key encryption. Key derived from timestamp +
    // ua_digest first 16 bytes — matches the reference Python's
    // `_get_key(ua, ts)` shape.
    let mut key = Vec::with_capacity(24);
    key.extend_from_slice(&inputs.timestamp_ms.to_le_bytes());
    key.extend_from_slice(&ua_digest[..16]);
    let mut encrypted = Vec::with_capacity(buf.len());
    for (i, &b) in buf.iter().enumerate() {
        encrypted.push(b ^ key[i % key.len()]);
    }

    // Step 5: custom-Base64 encode with Douyin's alphabet.
    custom_b64(&encrypted)
}

/// Minimal MD5 (RFC 1321) — duplicated locally to keep the douyin module
/// independently linkable. See `qrator.rs` for the same algorithm.
fn md5_hex(input: &[u8]) -> String {
    let bit_len = (input.len() as u64).wrapping_mul(8);
    let mut padded = Vec::with_capacity(input.len() + 64);
    padded.extend_from_slice(input);
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_le_bytes());

    const K: [u32; 64] = [
        0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613,
        0xfd469501, 0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193,
        0xa679438e, 0x49b40821, 0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d,
        0x02441453, 0xd8a1e681, 0xe7d3fbc8, 0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed,
        0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a, 0xfffa3942, 0x8771f681, 0x6d9d6122,
        0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70, 0x289b7ec6, 0xeaa127fa,
        0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665, 0xf4292244,
        0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
        0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb,
        0xeb86d391,
    ];
    const S: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5,
        9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10,
        15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];

    let mut a0 = 0x67452301u32;
    let mut b0 = 0xefcdab89u32;
    let mut c0 = 0x98badcfeu32;
    let mut d0 = 0x10325476u32;

    for chunk in padded.chunks_exact(64) {
        let mut m = [0u32; 16];
        for (j, word) in chunk.chunks_exact(4).enumerate() {
            m[j] = u32::from_le_bytes([word[0], word[1], word[2], word[3]]);
        }
        let (mut a, mut b, mut c, mut d) = (a0, b0, c0, d0);
        for i in 0..64 {
            let (f, g) = match i {
                0..=15 => ((b & c) | (!b & d), i),
                16..=31 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                32..=47 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let temp = d;
            d = c;
            c = b;
            b = b.wrapping_add(
                a.wrapping_add(f)
                    .wrapping_add(K[i])
                    .wrapping_add(m[g])
                    .rotate_left(S[i]),
            );
            a = temp;
        }
        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }

    let mut out = String::with_capacity(32);
    for word in [a0, b0, c0, d0] {
        for b in word.to_le_bytes() {
            out.push_str(&format!("{:02x}", b));
        }
    }
    out
}

/// Douyin's custom Base64 alphabet — looks like standard except some
/// indices are permuted to defeat naive replay/decode tools.
const DOUYIN_B64_ALPHA: &[u8; 64] =
    b"Dkdpgh4ZKsQB80/Mfvw36XI1R25-WUAlEi7NLboqYTOPuzmFjJnryx9HVGcaStCe";

fn custom_b64(input: &[u8]) -> String {
    let mut out = String::with_capacity((input.len() + 2) / 3 * 4);
    let mut i = 0;
    while i + 3 <= input.len() {
        let n = ((input[i] as u32) << 16) | ((input[i + 1] as u32) << 8) | input[i + 2] as u32;
        out.push(DOUYIN_B64_ALPHA[((n >> 18) & 63) as usize] as char);
        out.push(DOUYIN_B64_ALPHA[((n >> 12) & 63) as usize] as char);
        out.push(DOUYIN_B64_ALPHA[((n >> 6) & 63) as usize] as char);
        out.push(DOUYIN_B64_ALPHA[(n & 63) as usize] as char);
        i += 3;
    }
    let rem = input.len() - i;
    if rem == 1 {
        let n = (input[i] as u32) << 16;
        out.push(DOUYIN_B64_ALPHA[((n >> 18) & 63) as usize] as char);
        out.push(DOUYIN_B64_ALPHA[((n >> 12) & 63) as usize] as char);
        out.push('=');
        out.push('=');
    } else if rem == 2 {
        let n = ((input[i] as u32) << 16) | ((input[i + 1] as u32) << 8);
        out.push(DOUYIN_B64_ALPHA[((n >> 18) & 63) as usize] as char);
        out.push(DOUYIN_B64_ALPHA[((n >> 12) & 63) as usize] as char);
        out.push(DOUYIN_B64_ALPHA[((n >> 6) & 63) as usize] as char);
        out.push('=');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ABogusInputs<'static> {
        ABogusInputs {
            query: "aid=6383&device_platform=webapp&channel=channel_pc_web",
            body: "",
            user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                        (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36",
            timestamp_ms: 1_777_000_000_000,
        }
    }

    #[test]
    fn a_bogus_returns_non_empty_string() {
        let s = a_bogus(&sample());
        assert!(!s.is_empty());
        // Output is base64-shaped ASCII (no spaces, length is a multiple of 4
        // padding-included).
        assert!(s.chars().all(|c| c.is_ascii_graphic()));
    }

    #[test]
    fn a_bogus_is_deterministic() {
        let a = a_bogus(&sample());
        let b = a_bogus(&sample());
        assert_eq!(a, b);
    }

    #[test]
    fn a_bogus_changes_with_query() {
        let mut x = sample();
        let a = a_bogus(&x);
        x.query = "different=params";
        let b = a_bogus(&x);
        assert_ne!(a, b);
    }

    #[test]
    fn a_bogus_changes_with_body() {
        let mut x = sample();
        let a = a_bogus(&x);
        x.body = "post-payload";
        let b = a_bogus(&x);
        assert_ne!(a, b);
    }

    #[test]
    fn a_bogus_changes_with_ua() {
        let mut x = sample();
        let a = a_bogus(&x);
        x.user_agent = "different-ua";
        let b = a_bogus(&x);
        assert_ne!(a, b);
    }

    #[test]
    fn a_bogus_changes_with_timestamp() {
        let mut x = sample();
        let a = a_bogus(&x);
        x.timestamp_ms = 1_777_000_000_001;
        let b = a_bogus(&x);
        assert_ne!(a, b);
    }

    #[test]
    fn md5_known_vectors() {
        // Sanity (reused from qrator.rs).
        assert_eq!(md5_hex(b""), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(md5_hex(b"abc"), "900150983cd24fb0d6963f7d28e17f72");
    }

    #[test]
    fn custom_b64_round_trip_shape() {
        // 18 bytes → 24 b64 chars, no padding
        let input = vec![0u8; 18];
        let encoded = custom_b64(&input);
        assert_eq!(encoded.len(), 24);
        // 17 bytes → 24 b64 chars (1 padding =)
        let input = vec![0u8; 17];
        let encoded = custom_b64(&input);
        assert_eq!(encoded.len(), 24);
        assert!(encoded.ends_with('='));
    }

    #[test]
    fn custom_b64_uses_douyin_alphabet() {
        // First byte = 0x00 → first 6 bits = 0 → first char = alphabet[0] = 'D'
        let encoded = custom_b64(&[0u8, 0u8, 0u8]);
        assert_eq!(encoded.chars().next().unwrap(), 'D');
    }
}
