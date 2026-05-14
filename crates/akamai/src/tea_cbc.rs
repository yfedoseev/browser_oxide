//! W4.5 — Classic Wheeler/Needham TEA in CBC mode + Kasada key-derivation
//! scaffolding for the `__:` noise-key candidate A hypothesis.
//!
//! Per docs/research_2026_05_14/01_KASADA.md §2.1, the Kasada ips.js
//! VM ships a TEA-CBC cipher (classic 32-round Wheeler/Needham 1994
//! TEA, NOT the 1998 XXTEA). The 128-bit key is derived per-session
//! from the `__:` noise-key blob field (candidate A — highest
//! probability per §2.2).
//!
//! Algorithm reference: Wheeler & Needham (1994). "TEA, a Tiny
//! Encryption Algorithm." Cambridge Computer Lab.
//! <https://www.cl.cam.ac.uk/research/security/tea-xtea/>
//!
//! This module ships the algorithm + the candidate-A key derivation.
//! Static key recovery requires a captured `decrypted_blob_*.json` from
//! a live Kasada session (not present in tree); the wire-up to that
//! capture is the W4.5 follow-up. The library here is empirically
//! testable via the standard TEA test vectors below.

use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// TEA round delta (golden ratio mod 2^32). Wheeler/Needham 1994.
const DELTA: u32 = 0x9E37_79B9;

/// Encrypt one 64-bit block (8 bytes) with the 128-bit `key` using
/// classic 32-round TEA. `block[0..4]` is the left half, `block[4..8]`
/// is the right half — both big-endian per the original C reference.
pub fn tea_encrypt_block(block: &[u8; 8], key: &[u32; 4]) -> [u8; 8] {
    let mut v0 = u32::from_be_bytes([block[0], block[1], block[2], block[3]]);
    let mut v1 = u32::from_be_bytes([block[4], block[5], block[6], block[7]]);
    let mut sum: u32 = 0;
    for _ in 0..32 {
        sum = sum.wrapping_add(DELTA);
        v0 = v0.wrapping_add(
            ((v1 << 4).wrapping_add(key[0])) ^ (v1.wrapping_add(sum)) ^ ((v1 >> 5).wrapping_add(key[1]))
        );
        v1 = v1.wrapping_add(
            ((v0 << 4).wrapping_add(key[2])) ^ (v0.wrapping_add(sum)) ^ ((v0 >> 5).wrapping_add(key[3]))
        );
    }
    let mut out = [0u8; 8];
    out[0..4].copy_from_slice(&v0.to_be_bytes());
    out[4..8].copy_from_slice(&v1.to_be_bytes());
    out
}

/// Decrypt one 64-bit block. Inverse of `tea_encrypt_block`.
pub fn tea_decrypt_block(block: &[u8; 8], key: &[u32; 4]) -> [u8; 8] {
    let mut v0 = u32::from_be_bytes([block[0], block[1], block[2], block[3]]);
    let mut v1 = u32::from_be_bytes([block[4], block[5], block[6], block[7]]);
    let mut sum: u32 = DELTA.wrapping_mul(32);
    for _ in 0..32 {
        v1 = v1.wrapping_sub(
            ((v0 << 4).wrapping_add(key[2])) ^ (v0.wrapping_add(sum)) ^ ((v0 >> 5).wrapping_add(key[3]))
        );
        v0 = v0.wrapping_sub(
            ((v1 << 4).wrapping_add(key[0])) ^ (v1.wrapping_add(sum)) ^ ((v1 >> 5).wrapping_add(key[1]))
        );
        sum = sum.wrapping_sub(DELTA);
    }
    let mut out = [0u8; 8];
    out[0..4].copy_from_slice(&v0.to_be_bytes());
    out[4..8].copy_from_slice(&v1.to_be_bytes());
    out
}

/// Decrypt a TEA-CBC ciphertext. `iv` is the 8-byte initialization
/// vector; ciphertext length must be a multiple of 8. Returns decrypted
/// bytes (caller strips PKCS-7 padding if applicable).
pub fn tea_cbc_decrypt(ciphertext: &[u8], iv: &[u8; 8], key: &[u32; 4]) -> Option<Vec<u8>> {
    if ciphertext.len() % 8 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(ciphertext.len());
    let mut prev = *iv;
    for chunk in ciphertext.chunks_exact(8) {
        let mut block = [0u8; 8];
        block.copy_from_slice(chunk);
        let dec = tea_decrypt_block(&block, key);
        let mut plain = [0u8; 8];
        for i in 0..8 {
            plain[i] = dec[i] ^ prev[i];
        }
        out.extend_from_slice(&plain);
        prev = block;
    }
    Some(out)
}

/// Encrypt with TEA-CBC. Plaintext length must be a multiple of 8.
pub fn tea_cbc_encrypt(plaintext: &[u8], iv: &[u8; 8], key: &[u32; 4]) -> Option<Vec<u8>> {
    if plaintext.len() % 8 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(plaintext.len());
    let mut prev = *iv;
    for chunk in plaintext.chunks_exact(8) {
        let mut block = [0u8; 8];
        for i in 0..8 {
            block[i] = chunk[i] ^ prev[i];
        }
        let enc = tea_encrypt_block(&block, key);
        out.extend_from_slice(&enc);
        prev = enc;
    }
    Some(out)
}

/// Derive a 128-bit TEA key from the Kasada `__:` noise-key blob per
/// 01_KASADA.md §2.4 candidate A.
///
/// `noise_keys_list` is the comma-separated key-order string from the
/// blob's `__:` field (e.g. `"_xlq,_qnb,_ygj,_lwx,_jfk,..."`).
/// `blob` is the resolved key→value map from the captured
/// `decrypted_blob_*.json`. Values that reference other keys (e.g.
/// `_xlq: "_rxb"`) MUST be resolved before passing — caller does the
/// graph walk.
///
/// Implementation: concatenate values in the order listed in `__:`,
/// SHA-256, take first 16 bytes as the key. The exact mixer is
/// hypothesized (see §2.2); SHA-256 is the highest-probability choice
/// among FNV-1a / custom mixer / SHA. Verify against a captured
/// blob+ciphertext pair to confirm.
pub fn derive_tea_key_candidate_a(
    noise_keys_list: &str,
    blob: &HashMap<String, String>,
) -> [u32; 4] {
    let mut hasher = Sha256::new();
    for key in noise_keys_list.split(',') {
        let key = key.trim();
        if let Some(value) = blob.get(key) {
            hasher.update(value.as_bytes());
        }
    }
    let digest = hasher.finalize();
    let mut key = [0u32; 4];
    for (i, word) in digest.chunks_exact(4).take(4).enumerate() {
        key[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
    }
    key
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Wheeler/Needham 1994 reference vector. Plaintext 8 zero bytes,
    /// key 16 zero bytes. Expected ciphertext from
    /// <https://www.cl.cam.ac.uk/~mgk25/tea.c>.
    #[test]
    fn tea_zero_block_zero_key_known_vector() {
        let key = [0u32; 4];
        let block = [0u8; 8];
        let enc = tea_encrypt_block(&block, &key);
        // Standard TEA(zero_block, zero_key) = 0x41ea3a0a94baa940L 0xc1d8a8df8c6bbf42L
        // big-endian — verified against the reference C source.
        assert_eq!(
            enc,
            [0x41, 0xEA, 0x3A, 0x0A, 0x94, 0xBA, 0xA9, 0x40],
            "TEA zero-block zero-key reference vector mismatch"
        );
    }

    #[test]
    fn tea_round_trip() {
        let key = [0xDEAD_BEEFu32, 0x1234_5678, 0xCAFE_BABE, 0x9ABC_DEF0];
        let plaintext = b"OxideTea";
        let mut block = [0u8; 8];
        block.copy_from_slice(plaintext);
        let enc = tea_encrypt_block(&block, &key);
        let dec = tea_decrypt_block(&enc, &key);
        assert_eq!(&dec, plaintext);
    }

    #[test]
    fn tea_cbc_round_trip() {
        let key = [0x0000_0001u32, 0x0000_0002, 0x0000_0003, 0x0000_0004];
        let iv = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x00, 0x11];
        let plaintext = b"This sentence is sixteen!!!!!!!!"; // 32 bytes = 4 blocks
        let enc = tea_cbc_encrypt(plaintext, &iv, &key).unwrap();
        assert_eq!(enc.len(), plaintext.len());
        let dec = tea_cbc_decrypt(&enc, &iv, &key).unwrap();
        assert_eq!(&dec, plaintext);
    }

    #[test]
    fn tea_cbc_rejects_unaligned_length() {
        let key = [0u32; 4];
        let iv = [0u8; 8];
        assert!(tea_cbc_encrypt(b"odd-len", &iv, &key).is_none());
        assert!(tea_cbc_decrypt(b"odd-len", &iv, &key).is_none());
    }

    #[test]
    fn derive_tea_key_candidate_a_deterministic_and_position_sensitive() {
        let mut blob = HashMap::new();
        blob.insert("_xlq".into(), "value-of-xlq".into());
        blob.insert("_qnb".into(), "value-of-qnb".into());
        blob.insert("_ygj".into(), "value-of-ygj".into());

        let k1 = derive_tea_key_candidate_a("_xlq,_qnb,_ygj", &blob);
        let k2 = derive_tea_key_candidate_a("_xlq,_qnb,_ygj", &blob);
        assert_eq!(k1, k2, "derivation must be deterministic");

        // Position-sensitive (different order → different key).
        let k3 = derive_tea_key_candidate_a("_ygj,_qnb,_xlq", &blob);
        assert_ne!(k1, k3, "derivation must depend on key order");
    }
}
