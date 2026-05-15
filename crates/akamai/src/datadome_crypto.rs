//! DataDome client-side payload encryption — W3.8 solver primitives.
//!
//! Faithful Rust port of `glizzykingdreko/datadome-encryption`
//! `src/encryption.js` (clean-room Node reference). Full algorithm +
//! validation vectors: `docs/research_2026_05_14/18_DATADOME_ENCRYPTION_REFERENCE_2026_05_15.md`.
//!
//! This module currently implements the deterministic, independently
//! unit-testable primitives (steps 2 of the port plan). The PRNG
//! closure, buffer construction, and `_encodePayload` assembly are
//! staged for follow-up iterations and validated against the repo's
//! fixed-salt fixture before wiring into `datadome_handler`.
//!
//! All integer math mirrors JS 32-bit semantics: `|0` ⇒ `i32` wrapping,
//! `<<` ⇒ `wrapping_shl`, `>>` ⇒ arithmetic (signed) shift.

/// Interstitial-path encryption constants (the `rt:'i'` path —
/// etsy/tripadvisor/wsj/reuters). Captcha-path differs only in
/// `HASH_XOR` (-1748112727); we target interstitial.
pub const MAIN_PRNG_CONSTANT: i64 = 9_959_949_970;
pub const HASH_XOR_INTERSTITIAL: i32 = -883_841_716;
pub const CID_PRNG_CONSTANT: i32 = 1_809_053_797;

/// `_customHash` — djb2-variant (×31). Empty / zero ⇒ sentinel
/// 1789537805. JS: `hash = (hash << 5) - hash + ch | 0`.
pub fn custom_hash(s: &str) -> i32 {
    if s.is_empty() {
        return 1_789_537_805;
    }
    let mut hash: i32 = 0;
    // JS charCodeAt yields UTF-16 code units; for the ASCII cid/hash
    // inputs DataDome uses these equal Unicode scalar values. Encode as
    // UTF-16 to be faithful for any non-ASCII (defensive).
    for u in s.encode_utf16() {
        hash = hash
            .wrapping_shl(5)
            .wrapping_sub(hash)
            .wrapping_add(u as i32);
    }
    if hash != 0 {
        hash
    } else {
        1_789_537_805
    }
}

/// `_mixInt` — xorshift32 with JS signed shift semantics.
/// `v ^= v<<13; v ^= v>>17; v ^= v<<5`.
pub fn mix_int(mut v: i32) -> i32 {
    v ^= v.wrapping_shl(13);
    v ^= v >> 17; // arithmetic (signed) — matches JS `>>`
    v ^= v.wrapping_shl(5);
    v
}

/// `_encode6Bits` — custom base64 codepoint map. Input 0..=63.
pub fn encode_6bits(value: i32) -> u8 {
    let c = if value > 37 {
        59 + value
    } else if value > 11 {
        53 + value
    } else if value > 1 {
        46 + value
    } else {
        50 * value + 45
    };
    c as u8
}

/// PRNG seed for the main stream:
/// `_mainPrngConstant ^ _customHash(hash) ^ _hashXorConstant`.
/// JS does this in doubles then the PRNG uses 32-bit ops; the XOR chain
/// is performed at 32-bit width (the values are within i32 except
/// MAIN_PRNG_CONSTANT which JS truncates via the `^` to int32).
pub fn main_prng_seed(hash: &str) -> i32 {
    let m = MAIN_PRNG_CONSTANT as i32; // JS `^` coerces to int32
    m ^ custom_hash(hash) ^ HASH_XOR_INTERSTITIAL
}

/// cid PRNG seed: `_cidPrngConstant ^ _customHash(cid)`.
pub fn cid_prng_seed(cid: &str) -> i32 {
    CID_PRNG_CONSTANT ^ custom_hash(cid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_hash_empty_is_sentinel() {
        assert_eq!(custom_hash(""), 1_789_537_805);
    }

    #[test]
    fn custom_hash_djb2_variant_x31() {
        // Hand-computed: hash = 31*hash + ch.
        assert_eq!(custom_hash("a"), 97); // 31*0 + 97
        assert_eq!(custom_hash("ab"), 3105); // 31*97 + 98
        assert_eq!(custom_hash("abc"), 96354); // 31*3105 + 99
    }

    #[test]
    fn mix_int_known_vector() {
        // v=1: v^=1<<13=8192 → 8193; v^=8193>>17=0 → 8193;
        // v^=8193<<5=262176 → 2^18+2^13+2^5+2^0 = 270369.
        assert_eq!(mix_int(1), 270_369);
        // Deterministic / pure.
        assert_eq!(mix_int(1), mix_int(1));
        assert_ne!(mix_int(1), mix_int(2));
    }

    #[test]
    fn encode_6bits_branch_boundaries() {
        assert_eq!(encode_6bits(0), 45); // 50*0+45
        assert_eq!(encode_6bits(1), 95); // 50*1+45
        assert_eq!(encode_6bits(2), 48); // 46+2
        assert_eq!(encode_6bits(11), 57); // 46+11
        assert_eq!(encode_6bits(12), 65); // 53+12
        assert_eq!(encode_6bits(37), 90); // 53+37
        assert_eq!(encode_6bits(38), 97); // 59+38
        assert_eq!(encode_6bits(63), 122); // 59+63
    }

    #[test]
    fn seeds_are_deterministic() {
        let h = "14D062F60A4BDE8CE8647DFC720349";
        assert_eq!(main_prng_seed(h), main_prng_seed(h));
        assert_ne!(main_prng_seed(h), cid_prng_seed("somecid"));
    }
}
