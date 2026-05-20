//! NGENIX challenge-cookie solver (Russian CDN, testcookie-nginx pattern).
//!
//! NGENIX is a Russian CDN/anti-bot deployed on RU mid-tier e-commerce.
//! Its JS challenge follows the open-source `testcookie-nginx-module`
//! pattern: server issues an AES-128-CBC ciphertext cookie + a small JS
//! script that decrypts it client-side and re-submits as a different
//! cookie to "prove the browser ran JavaScript."
//!
//! Reference algorithm: <https://github.com/kyprizel/testcookie-nginx-module>
//! (the open-source ancestor — algorithm has been stable since 2014).
//!
//! Detection cookies (set by NGENIX server):
//!   `ngenix_jscc_*` — ciphertext + key + IV bundle
//!   `ngenix_jscv_*` — empty placeholder we must populate with the plaintext
//!
//! Flow:
//!   1. GET / → 403 + Set-Cookie: ngenix_jscc_<id>=<base64(iv+key+ct)>
//!   2. Page contains a `<script>` that AES-CBC-decrypts the bundle
//!      and sets `ngenix_jscv_<id>=<plaintext>` via document.cookie
//!   3. Retry GET / with both cookies → 200
//!
//! This module computes the plaintext natively, skipping the JS execution.
//! The cookie value is opaque to NGENIX's verification — they re-decrypt
//! server-side and check it matches.

// ============================================================================
// AES-128 inline implementation (no new dependencies).
// ============================================================================
// Forward S-box (used in key expansion SubWord).
#[rustfmt::skip]
const SBOX: [u8; 256] = [
    0x63,0x7c,0x77,0x7b,0xf2,0x6b,0x6f,0xc5,0x30,0x01,0x67,0x2b,0xfe,0xd7,0xab,0x76,
    0xca,0x82,0xc9,0x7d,0xfa,0x59,0x47,0xf0,0xad,0xd4,0xa2,0xaf,0x9c,0xa4,0x72,0xc0,
    0xb7,0xfd,0x93,0x26,0x36,0x3f,0xf7,0xcc,0x34,0xa5,0xe5,0xf1,0x71,0xd8,0x31,0x15,
    0x04,0xc7,0x23,0xc3,0x18,0x96,0x05,0x9a,0x07,0x12,0x80,0xe2,0xeb,0x27,0xb2,0x75,
    0x09,0x83,0x2c,0x1a,0x1b,0x6e,0x5a,0xa0,0x52,0x3b,0xd6,0xb3,0x29,0xe3,0x2f,0x84,
    0x53,0xd1,0x00,0xed,0x20,0xfc,0xb1,0x5b,0x6a,0xcb,0xbe,0x39,0x4a,0x4c,0x58,0xcf,
    0xd0,0xef,0xaa,0xfb,0x43,0x4d,0x33,0x85,0x45,0xf9,0x02,0x7f,0x50,0x3c,0x9f,0xa8,
    0x51,0xa3,0x40,0x8f,0x92,0x9d,0x38,0xf5,0xbc,0xb6,0xda,0x21,0x10,0xff,0xf3,0xd2,
    0xcd,0x0c,0x13,0xec,0x5f,0x97,0x44,0x17,0xc4,0xa7,0x7e,0x3d,0x64,0x5d,0x19,0x73,
    0x60,0x81,0x4f,0xdc,0x22,0x2a,0x90,0x88,0x46,0xee,0xb8,0x14,0xde,0x5e,0x0b,0xdb,
    0xe0,0x32,0x3a,0x0a,0x49,0x06,0x24,0x5c,0xc2,0xd3,0xac,0x62,0x91,0x95,0xe4,0x79,
    0xe7,0xc8,0x37,0x6d,0x8d,0xd5,0x4e,0xa9,0x6c,0x56,0xf4,0xea,0x65,0x7a,0xae,0x08,
    0xba,0x78,0x25,0x2e,0x1c,0xa6,0xb4,0xc6,0xe8,0xdd,0x74,0x1f,0x4b,0xbd,0x8b,0x8a,
    0x70,0x3e,0xb5,0x66,0x48,0x03,0xf6,0x0e,0x61,0x35,0x57,0xb9,0x86,0xc1,0x1d,0x9e,
    0xe1,0xf8,0x98,0x11,0x69,0xd9,0x8e,0x94,0x9b,0x1e,0x87,0xe9,0xce,0x55,0x28,0xdf,
    0x8c,0xa1,0x89,0x0d,0xbf,0xe6,0x42,0x68,0x41,0x99,0x2d,0x0f,0xb0,0x54,0xbb,0x16,
];

// Inverse S-box (used in InvSubBytes).
#[rustfmt::skip]
const SBOX_INV: [u8; 256] = [
    0x52,0x09,0x6a,0xd5,0x30,0x36,0xa5,0x38,0xbf,0x40,0xa3,0x9e,0x81,0xf3,0xd7,0xfb,
    0x7c,0xe3,0x39,0x82,0x9b,0x2f,0xff,0x87,0x34,0x8e,0x43,0x44,0xc4,0xde,0xe9,0xcb,
    0x54,0x7b,0x94,0x32,0xa6,0xc2,0x23,0x3d,0xee,0x4c,0x95,0x0b,0x42,0xfa,0xc3,0x4e,
    0x08,0x2e,0xa1,0x66,0x28,0xd9,0x24,0xb2,0x76,0x5b,0xa2,0x49,0x6d,0x8b,0xd1,0x25,
    0x72,0xf8,0xf6,0x64,0x86,0x68,0x98,0x16,0xd4,0xa4,0x5c,0xcc,0x5d,0x65,0xb6,0x92,
    0x6c,0x70,0x48,0x50,0xfd,0xed,0xb9,0xda,0x5e,0x15,0x46,0x57,0xa7,0x8d,0x9d,0x84,
    0x90,0xd8,0xab,0x00,0x8c,0xbc,0xd3,0x0a,0xf7,0xe4,0x58,0x05,0xb8,0xb3,0x45,0x06,
    0xd0,0x2c,0x1e,0x8f,0xca,0x3f,0x0f,0x02,0xc1,0xaf,0xbd,0x03,0x01,0x13,0x8a,0x6b,
    0x3a,0x91,0x11,0x41,0x4f,0x67,0xdc,0xea,0x97,0xf2,0xcf,0xce,0xf0,0xb4,0xe6,0x73,
    0x96,0xac,0x74,0x22,0xe7,0xad,0x35,0x85,0xe2,0xf9,0x37,0xe8,0x1c,0x75,0xdf,0x6e,
    0x47,0xf1,0x1a,0x71,0x1d,0x29,0xc5,0x89,0x6f,0xb7,0x62,0x0e,0xaa,0x18,0xbe,0x1b,
    0xfc,0x56,0x3e,0x4b,0xc6,0xd2,0x79,0x20,0x9a,0xdb,0xc0,0xfe,0x78,0xcd,0x5a,0xf4,
    0x1f,0xdd,0xa8,0x33,0x88,0x07,0xc7,0x31,0xb1,0x12,0x10,0x59,0x27,0x80,0xec,0x5f,
    0x60,0x51,0x7f,0xa9,0x19,0xb5,0x4a,0x0d,0x2d,0xe5,0x7a,0x9f,0x93,0xc9,0x9c,0xef,
    0xa0,0xe0,0x3b,0x4d,0xae,0x2a,0xf5,0xb0,0xc8,0xeb,0xbb,0x3c,0x83,0x53,0x99,0x61,
    0x17,0x2b,0x04,0x7e,0xba,0x77,0xd6,0x26,0xe1,0x69,0x14,0x63,0x55,0x21,0x0c,0x7d,
];

// Round constants for key expansion.
const RCON: [u8; 10] = [0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80, 0x1b, 0x36];

/// GF(2^8) multiply (the AES field, polynomial 0x11b).
fn gf_mul(mut a: u8, mut b: u8) -> u8 {
    let mut p = 0u8;
    for _ in 0..8 {
        if b & 1 != 0 {
            p ^= a;
        }
        let carry = a & 0x80;
        a <<= 1;
        if carry != 0 {
            a ^= 0x1b;
        }
        b >>= 1;
    }
    p
}

/// Expand a 128-bit key into 11 round keys (AES-128 key schedule).
fn aes128_key_expand(key: &[u8; 16]) -> [[u8; 16]; 11] {
    let mut w = [[0u8; 4]; 44];
    for i in 0..4 {
        w[i].copy_from_slice(&key[i * 4..i * 4 + 4]);
    }
    for i in 4..44 {
        let mut tmp = w[i - 1];
        if i % 4 == 0 {
            // RotWord
            tmp = [tmp[1], tmp[2], tmp[3], tmp[0]];
            // SubWord
            for b in &mut tmp {
                *b = SBOX[*b as usize];
            }
            tmp[0] ^= RCON[i / 4 - 1];
        }
        w[i] = [
            w[i - 4][0] ^ tmp[0],
            w[i - 4][1] ^ tmp[1],
            w[i - 4][2] ^ tmp[2],
            w[i - 4][3] ^ tmp[3],
        ];
    }
    let mut round_keys = [[0u8; 16]; 11];
    for (rk, words) in round_keys.iter_mut().zip(w.chunks_exact(4)) {
        for (j, word) in words.iter().enumerate() {
            rk[j * 4..j * 4 + 4].copy_from_slice(word);
        }
    }
    round_keys
}

/// AES-128 single-block decrypt.
///
/// AES state is 4×4 column-major: state[col*4 + row].
fn aes128_decrypt_block(round_keys: &[[u8; 16]; 11], block: &[u8; 16]) -> [u8; 16] {
    let mut s = *block;

    // AddRoundKey with last round key.
    for (b, k) in s.iter_mut().zip(round_keys[10].iter()) {
        *b ^= k;
    }

    for round in (1..10).rev() {
        inv_shift_rows(&mut s);
        inv_sub_bytes(&mut s);
        for (b, k) in s.iter_mut().zip(round_keys[round].iter()) {
            *b ^= k;
        }
        inv_mix_columns(&mut s);
    }

    // Final round (no InvMixColumns).
    inv_shift_rows(&mut s);
    inv_sub_bytes(&mut s);
    for (b, k) in s.iter_mut().zip(round_keys[0].iter()) {
        *b ^= k;
    }

    s
}

fn inv_sub_bytes(s: &mut [u8; 16]) {
    for b in s.iter_mut() {
        *b = SBOX_INV[*b as usize];
    }
}

/// InvShiftRows: right-rotate each row by its index.
fn inv_shift_rows(s: &mut [u8; 16]) {
    // Row 1 indices (column-major): 1, 5, 9, 13 — right rotate by 1
    let tmp = s[13];
    s[13] = s[9];
    s[9] = s[5];
    s[5] = s[1];
    s[1] = tmp;
    // Row 2 indices: 2, 6, 10, 14 — right rotate by 2 (swap pairs)
    s.swap(2, 10);
    s.swap(6, 14);
    // Row 3 indices: 3, 7, 11, 15 — right rotate by 3 (= left rotate by 1)
    let tmp = s[3];
    s[3] = s[7];
    s[7] = s[11];
    s[11] = s[15];
    s[15] = tmp;
}

/// InvMixColumns: each column multiplied by the AES inverse mix matrix in GF(2^8).
fn inv_mix_columns(s: &mut [u8; 16]) {
    for col in 0..4 {
        let i = col * 4;
        let (s0, s1, s2, s3) = (s[i], s[i + 1], s[i + 2], s[i + 3]);
        s[i] = gf_mul(0x0e, s0) ^ gf_mul(0x0b, s1) ^ gf_mul(0x0d, s2) ^ gf_mul(0x09, s3);
        s[i + 1] = gf_mul(0x09, s0) ^ gf_mul(0x0e, s1) ^ gf_mul(0x0b, s2) ^ gf_mul(0x0d, s3);
        s[i + 2] = gf_mul(0x0d, s0) ^ gf_mul(0x09, s1) ^ gf_mul(0x0e, s2) ^ gf_mul(0x0b, s3);
        s[i + 3] = gf_mul(0x0b, s0) ^ gf_mul(0x0d, s1) ^ gf_mul(0x09, s2) ^ gf_mul(0x0e, s3);
    }
}

/// AES-128-CBC decrypt + PKCS#7 unpad. Returns `None` if ciphertext length is
/// not a multiple of 16 or PKCS#7 padding is invalid.
fn aes128_cbc_decrypt(key: &[u8; 16], iv: &[u8; 16], ct: &[u8]) -> Option<Vec<u8>> {
    if ct.is_empty() || !ct.len().is_multiple_of(16) {
        return None;
    }
    let round_keys = aes128_key_expand(key);
    let mut plaintext = Vec::with_capacity(ct.len());
    let mut prev = *iv;

    for chunk in ct.chunks_exact(16) {
        let block: [u8; 16] = chunk.try_into().unwrap();
        let mut dec = aes128_decrypt_block(&round_keys, &block);
        for (d, p) in dec.iter_mut().zip(prev.iter()) {
            *d ^= p;
        }
        plaintext.extend_from_slice(&dec);
        prev = block;
    }

    // PKCS#7 unpad.
    let pad = *plaintext.last()? as usize;
    if pad == 0 || pad > 16 || pad > plaintext.len() {
        return None;
    }
    let pad_start = plaintext.len() - pad;
    if !plaintext[pad_start..].iter().all(|&b| b as usize == pad) {
        return None;
    }
    plaintext.truncate(pad_start);
    Some(plaintext)
}

// ============================================================================
// Public API
// ============================================================================

/// Decoded NGENIX challenge bundle (parsed from the `ngenix_jscc_<id>` cookie).
#[derive(Debug, Clone)]
pub struct NgenixChallenge {
    /// 16-byte AES-128 key.
    pub key: [u8; 16],
    /// 16-byte IV.
    pub iv: [u8; 16],
    /// Ciphertext (multiple of 16 bytes, PKCS#7-padded plaintext).
    pub ciphertext: Vec<u8>,
}

/// Parse the `ngenix_jscc_<id>` cookie value into a challenge bundle.
///
/// The bundle layout (after base64-url decode) is:
///   `[iv:16][key:16][ciphertext:N*16]`
///
/// This is the canonical testcookie-nginx layout. Returns `None` if the
/// input is too short or not valid base64.
pub fn parse_jscc(cookie_value: &str) -> Option<NgenixChallenge> {
    let bytes = decode_base64_url_or_std(cookie_value)?;
    if bytes.len() < 32 || (bytes.len() - 32) % 16 != 0 {
        return None;
    }
    let mut iv = [0u8; 16];
    let mut key = [0u8; 16];
    iv.copy_from_slice(&bytes[..16]);
    key.copy_from_slice(&bytes[16..32]);
    let ciphertext = bytes[32..].to_vec();
    Some(NgenixChallenge {
        key,
        iv,
        ciphertext,
    })
}

/// Decode either standard or URL-safe base64.
fn decode_base64_url_or_std(s: &str) -> Option<Vec<u8>> {
    let mut buf = Vec::with_capacity((s.len() * 3) / 4);
    let mut bits: u32 = 0;
    let mut nbits: u32 = 0;
    for c in s.chars() {
        let v = match c {
            'A'..='Z' => c as u32 - 'A' as u32,
            'a'..='z' => c as u32 - 'a' as u32 + 26,
            '0'..='9' => c as u32 - '0' as u32 + 52,
            '+' | '-' => 62,
            '/' | '_' => 63,
            '=' => break,
            _ => continue,
        };
        bits = (bits << 6) | v;
        nbits += 6;
        if nbits >= 8 {
            nbits -= 8;
            buf.push((bits >> nbits) as u8);
            bits &= (1 << nbits) - 1;
        }
    }
    Some(buf)
}

/// Solve the NGENIX challenge: AES-128-CBC decrypt the ciphertext, PKCS#7-unpad,
/// and return the plaintext hex-encoded — the value for `ngenix_jscv_<id>`.
///
/// Returns an empty string on any decryption error so the caller can fall back
/// to JS execution without crashing.
pub fn solve(challenge: &NgenixChallenge) -> String {
    match aes128_cbc_decrypt(&challenge.key, &challenge.iv, &challenge.ciphertext) {
        Some(plain) => hex::encode(plain),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── NIST SP 800-38A CBC-AES128 Known-Answer Test (F.2.1 / F.2.2) ─────────
    // Key:  2b7e151628aed2a6abf7158809cf4f3c
    // IV:   000102030405060708090a0b0c0d0e0f
    // PT:   6bc1bee22e409f96e93d7e117393172a  (block 1)
    //       ae2d8a571e03ac9c9eb76fac45af8e51  (block 2)
    // CT:   7649abac8119b246cee98e9b12e9197d  (block 1)
    //       5086cb9b507219ee95db113a917678b2  (block 2)
    //
    // We test CBC decrypt: CT → PT (no PKCS#7 padding — raw block test).

    fn hex_to_bytes(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn aes128_block_decrypt_nist() {
        // NIST FIPS 197 Appendix B (decrypt direction):
        // Key: 2b7e151628aed2a6abf7158809cf4f3c
        // CT:  3925841d02dc09fbdc118597196a0b32
        // PT:  3243f6a8885a308d313198a2e0370734
        let key: [u8; 16] = hex_to_bytes("2b7e151628aed2a6abf7158809cf4f3c")
            .try_into()
            .unwrap();
        let ct: [u8; 16] = hex_to_bytes("3925841d02dc09fbdc118597196a0b32")
            .try_into()
            .unwrap();
        let rk = aes128_key_expand(&key);
        let pt = aes128_decrypt_block(&rk, &ct);
        assert_eq!(hex::encode(pt), "3243f6a8885a308d313198a2e0370734");
    }

    #[test]
    fn aes128_cbc_decrypt_nist_two_blocks() {
        let key: [u8; 16] = hex_to_bytes("2b7e151628aed2a6abf7158809cf4f3c")
            .try_into()
            .unwrap();
        let iv: [u8; 16] = hex_to_bytes("000102030405060708090a0b0c0d0e0f")
            .try_into()
            .unwrap();
        // Two CT blocks from NIST SP 800-38A F.2.2 (no PKCS#7 — raw test).
        let ct = hex_to_bytes("7649abac8119b246cee98e9b12e9197d5086cb9b507219ee95db113a917678b2");
        // We skip PKCS#7 check for this raw test by padding the PT manually.
        // Treat the whole thing as raw CBC by bypassing the unpad call.
        let round_keys = aes128_key_expand(&key);
        let mut result = Vec::new();
        let mut prev = iv;
        for chunk in ct.chunks_exact(16) {
            let block: [u8; 16] = chunk.try_into().unwrap();
            let mut dec = aes128_decrypt_block(&round_keys, &block);
            for (d, p) in dec.iter_mut().zip(prev.iter()) {
                *d ^= p;
            }
            result.extend_from_slice(&dec);
            prev = block;
        }
        assert_eq!(
            hex::encode(&result),
            "6bc1bee22e409f96e93d7e117393172aae2d8a571e03ac9c9eb76fac45af8e51"
        );
    }

    #[test]
    fn solve_roundtrip_with_pkcs7() {
        // Construct a challenge: encrypt "hello" (5 bytes) with PKCS#7 padding
        // using a known key/IV, then solve() should return "hello" hex.
        //
        // Plaintext after PKCS#7 pad: 68 65 6c 6c 6f 0b 0b 0b 0b 0b 0b 0b 0b 0b 0b 0b
        // We compute the ciphertext by encrypting manually.
        let key = [0u8; 16];
        let iv = [0u8; 16];
        let pt_padded: [u8; 16] = [
            b'h', b'e', b'l', b'l', b'o', 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b, 0x0b,
            0x0b, 0x0b,
        ];
        // Encrypt: AES-128-ECB(pt XOR iv) with all-zero key/IV.
        // CBC-encrypt a single block: ct = AES_enc(pt XOR IV).
        let round_keys = aes128_key_expand(&key);
        let mut block = pt_padded;
        for (b, i) in block.iter_mut().zip(iv.iter()) {
            *b ^= i;
        }
        let ct = aes128_encrypt_block(&round_keys, &block);

        let challenge = NgenixChallenge {
            key,
            iv,
            ciphertext: ct.to_vec(),
        };
        let result = solve(&challenge);
        assert_eq!(result, hex::encode(b"hello"));
    }

    #[test]
    fn solve_is_deterministic() {
        let c = NgenixChallenge {
            key: [1u8; 16],
            iv: [2u8; 16],
            // 32-byte CT: valid length but likely bad padding → empty string result
            ciphertext: vec![3u8; 32],
        };
        let a = solve(&c);
        let b = solve(&c);
        assert_eq!(a, b);
    }

    #[test]
    fn decode_base64_standard() {
        let out = decode_base64_url_or_std("SGVsbG8sIFdvcmxkIQ==").unwrap();
        assert_eq!(out, b"Hello, World!");
    }

    #[test]
    fn decode_base64_url_safe() {
        let out = decode_base64_url_or_std("PDw_Pj4=").unwrap();
        assert_eq!(out, b"<<?>>");
    }

    #[test]
    fn parse_jscc_too_short_returns_none() {
        let too_short = "QUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQQ==";
        assert!(parse_jscc(too_short).is_none());
    }

    #[test]
    fn parse_jscc_valid_layout() {
        let bundle: Vec<u8> = (0..48u8).collect();
        let b64: String = encode_b64(&bundle);
        let challenge = parse_jscc(&b64).expect("parses");
        assert_eq!(
            challenge.iv,
            [0u8, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
        );
        assert_eq!(
            challenge.key,
            [16u8, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31]
        );
        assert_eq!(challenge.ciphertext.len(), 16);
    }

    // AES-128 encrypt (forward direction) — used only in tests to construct
    // known ciphertext for roundtrip verification.
    fn aes128_encrypt_block(round_keys: &[[u8; 16]; 11], block: &[u8; 16]) -> [u8; 16] {
        let mut s = *block;
        for b in s.iter_mut().zip(round_keys[0].iter()) {
            *b.0 ^= b.1;
        }
        for round in 1..=10 {
            sub_bytes(&mut s);
            shift_rows(&mut s);
            if round < 10 {
                mix_columns(&mut s);
            }
            for (b, k) in s.iter_mut().zip(round_keys[round].iter()) {
                *b ^= k;
            }
        }
        s
    }

    fn sub_bytes(s: &mut [u8; 16]) {
        for b in s.iter_mut() {
            *b = SBOX[*b as usize];
        }
    }

    fn shift_rows(s: &mut [u8; 16]) {
        // Row 1: left rotate by 1
        let tmp = s[1];
        s[1] = s[5];
        s[5] = s[9];
        s[9] = s[13];
        s[13] = tmp;
        // Row 2: left rotate by 2
        s.swap(2, 10);
        s.swap(6, 14);
        // Row 3: left rotate by 3
        let tmp = s[15];
        s[15] = s[11];
        s[11] = s[7];
        s[7] = s[3];
        s[3] = tmp;
    }

    fn mix_columns(s: &mut [u8; 16]) {
        for col in 0..4 {
            let i = col * 4;
            let (s0, s1, s2, s3) = (s[i], s[i + 1], s[i + 2], s[i + 3]);
            s[i] = gf_mul(0x02, s0) ^ gf_mul(0x03, s1) ^ s2 ^ s3;
            s[i + 1] = s0 ^ gf_mul(0x02, s1) ^ gf_mul(0x03, s2) ^ s3;
            s[i + 2] = s0 ^ s1 ^ gf_mul(0x02, s2) ^ gf_mul(0x03, s3);
            s[i + 3] = gf_mul(0x03, s0) ^ s1 ^ s2 ^ gf_mul(0x02, s3);
        }
    }

    fn encode_b64(input: &[u8]) -> String {
        const ALPHA: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::with_capacity((input.len() + 2) / 3 * 4);
        let mut i = 0;
        while i + 3 <= input.len() {
            let n = ((input[i] as u32) << 16) | ((input[i + 1] as u32) << 8) | input[i + 2] as u32;
            out.push(ALPHA[((n >> 18) & 63) as usize] as char);
            out.push(ALPHA[((n >> 12) & 63) as usize] as char);
            out.push(ALPHA[((n >> 6) & 63) as usize] as char);
            out.push(ALPHA[(n & 63) as usize] as char);
            i += 3;
        }
        let rem = input.len() - i;
        if rem == 1 {
            let n = (input[i] as u32) << 16;
            out.push(ALPHA[((n >> 18) & 63) as usize] as char);
            out.push(ALPHA[((n >> 12) & 63) as usize] as char);
            out.push('=');
            out.push('=');
        } else if rem == 2 {
            let n = ((input[i] as u32) << 16) | ((input[i + 1] as u32) << 8);
            out.push(ALPHA[((n >> 18) & 63) as usize] as char);
            out.push(ALPHA[((n >> 12) & 63) as usize] as char);
            out.push(ALPHA[((n >> 6) & 63) as usize] as char);
            out.push('=');
        }
        out
    }
}
