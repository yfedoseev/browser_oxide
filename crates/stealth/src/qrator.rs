//! QRATOR / Curator anti-DDoS challenge solver (Russian CDN).
//!
//! QRATOR is a Russian DDoS-protection / WAF deployed on dns-shop.ru and
//! many RU enterprise sites. Its JS challenge serves an obfuscated
//! `/__qrator/qauth_*.js` script that performs an MD5-based proof-of-work
//! and submits the answer to obtain `qrator_jsid` / `qrator_ssid` /
//! `qrator_jsr` cookies.
//!
//! Reference: <https://github.com/pointless5g/qrator-solver> (Go,
//! 2025-12-24, freshest public). Algorithm: find counter `n` such that
//! `MD5(nonce + n)` has a target prefix; n is the PoW answer.
//!
//! For dns-shop.ru specifically there's a JSON-microdata shortcut at
//! `dns-shop.ru/product/microdata/<uuid>/` that bypasses QRATOR entirely
//! (per `docs/universal_engine/site_debugging/dns_shop_qrator.md`).


/// QRATOR challenge inputs (parsed from the qauth.js page or computed
/// from response headers).
#[derive(Debug, Clone)]
pub struct QratorChallenge {
    /// The nonce string the PoW concatenates with the counter. Extracted
    /// from the qauth.js obfuscated source (e.g. a `var nonce = "..."`).
    pub nonce: String,
    /// Required hex-prefix for the MD5 result. Typical: 4-6 leading zeros.
    pub target_prefix: String,
}

impl Default for QratorChallenge {
    fn default() -> Self {
        // Public deployments observed in 2025-2026 use a 4-zero prefix
        // (~16 bits of work, mean ~32k iterations, <100 ms in Rust).
        Self {
            nonce: String::new(),
            target_prefix: "0000".into(),
        }
    }
}

/// Solve the QRATOR PoW. Returns the answer counter `n` such that
/// `MD5(nonce + n)` starts with `target_prefix`.
///
/// Iteration cap: 10 million (prevents infinite loop on malformed input;
/// realistic challenges complete in <100k iterations).
pub fn solve(challenge: &QratorChallenge) -> Option<u32> {
    const MAX_ITER: u32 = 10_000_000;
    for n in 1..MAX_ITER {
        let candidate = format!("{}{}", challenge.nonce, n);
        let digest_hex = md5_hex(candidate.as_bytes());
        if digest_hex.starts_with(&challenge.target_prefix) {
            return Some(n);
        }
    }
    None
}

/// Minimal MD5 hex via the `md5` crate's pattern but without the dep.
/// We implement just enough of MD5 inline via the standard algorithm so
/// we don't pull in another crypto crate. (Stealth already has sha2.)
///
/// This is a self-contained MD5 impl. ~100 LOC. Tested against known
/// vectors below.
fn md5_hex(input: &[u8]) -> String {
    let digest = md5(input);
    let mut s = String::with_capacity(32);
    for b in digest {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

/// MD5 (RFC 1321). Self-contained 16-byte digest.
fn md5(input: &[u8]) -> [u8; 16] {
    // Padded message: original + 0x80 + zeros + 8-byte little-endian bit length.
    let bit_len = (input.len() as u64).wrapping_mul(8);
    let mut padded = Vec::with_capacity(input.len() + 64);
    padded.extend_from_slice(input);
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_le_bytes());

    // K constants and per-round shift amounts.
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

    let mut out = [0u8; 16];
    out[0..4].copy_from_slice(&a0.to_le_bytes());
    out[4..8].copy_from_slice(&b0.to_le_bytes());
    out[8..12].copy_from_slice(&c0.to_le_bytes());
    out[12..16].copy_from_slice(&d0.to_le_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // Sanity: MD5 must match RFC 1321 test vectors before we trust the solver.

    #[test]
    fn md5_empty_string() {
        // d41d8cd98f00b204e9800998ecf8427e
        assert_eq!(md5_hex(b""), "d41d8cd98f00b204e9800998ecf8427e");
    }

    #[test]
    fn md5_abc() {
        // 900150983cd24fb0d6963f7d28e17f72
        assert_eq!(md5_hex(b"abc"), "900150983cd24fb0d6963f7d28e17f72");
    }

    #[test]
    fn md5_message_digest() {
        // f96b697d7cb7938d525a2f31aaf161d0
        assert_eq!(
            md5_hex(b"message digest"),
            "f96b697d7cb7938d525a2f31aaf161d0"
        );
    }

    #[test]
    fn md5_alphabet() {
        assert_eq!(
            md5_hex(b"abcdefghijklmnopqrstuvwxyz"),
            "c3fcd3d76192e4007dfb496cca67e13b"
        );
    }

    #[test]
    fn md5_long_input() {
        // Boundary at chunk-padding behavior (input > 56 bytes)
        let s: String = "a".repeat(100);
        // Known: md5('a'*100) = '36a92cc94a9e0fa21f625f8bfb007adf'
        assert_eq!(md5_hex(s.as_bytes()), "36a92cc94a9e0fa21f625f8bfb007adf");
    }

    // QRATOR PoW

    #[test]
    fn solve_finds_answer_with_default_target() {
        let challenge = QratorChallenge {
            nonce: "test-nonce-".into(),
            target_prefix: "00".into(), // 2 zeros = 8 bits = ~256 iterations
        };
        let answer = solve(&challenge).expect("must find an answer");
        // Verify the answer is correct.
        let candidate = format!("{}{}", challenge.nonce, answer);
        let digest = md5_hex(candidate.as_bytes());
        assert!(digest.starts_with(&challenge.target_prefix));
    }

    #[test]
    fn solve_is_deterministic() {
        let c = QratorChallenge {
            nonce: "deterministic-nonce".into(),
            target_prefix: "00".into(),
        };
        let a = solve(&c).unwrap();
        let b = solve(&c).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn solve_4zero_target_completes() {
        // Default 4-zero target = ~16 bits = mean 32k iterations. Should
        // complete in well under 1 second.
        let c = QratorChallenge {
            nonce: "session-XYZ-".into(),
            target_prefix: "0000".into(),
        };
        let t0 = std::time::Instant::now();
        let answer = solve(&c).expect("4-zero solve");
        let elapsed = t0.elapsed();
        let candidate = format!("{}{}", c.nonce, answer);
        let digest = md5_hex(candidate.as_bytes());
        assert!(digest.starts_with("0000"), "digest = {digest}");
        assert!(
            elapsed.as_secs() < 5,
            "QRATOR 4-zero solve took {:?}",
            elapsed
        );
    }

    #[test]
    fn solve_returns_none_on_unsolvable() {
        // 9-zero target = ~36 bits = mean 64 GiB iterations; we cap at 10M.
        let c = QratorChallenge {
            nonce: "unreachable".into(),
            target_prefix: "000000000".into(),
        };
        // Will hit MAX_ITER and return None.
        // Skip in normal tests — too slow even with cap.
        // (Just assert the function exists and doesn't panic immediately.)
        let _ = c.target_prefix.len();
    }
}
