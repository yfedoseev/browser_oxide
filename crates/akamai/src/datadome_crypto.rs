//! DataDome client-side payload encryption — W3.8 solver primitives.
//!
//! Faithful Rust port of `glizzykingdreko/datadome-encryption`
//! `src/encryption.js` (clean-room Node reference).
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

/// Faithful port of `_createPrng`'s returned closure (stateful).
/// 3 output bytes per `_mixInt` round; result = `state >> (16 - 8*round)`
/// (JS signed `>>`); optional `result ^= --saltState` when `use_alt`;
/// `&255`; a 1-deep cache when `flag=true`.
pub struct DdPrng {
    state: i32,
    round: i32,
    salt_state: i32,
    use_alt: bool,
    cache: Option<u8>,
}

impl DdPrng {
    pub fn new(seed: i32, salt: i32, use_alt: bool) -> Self {
        Self {
            state: seed,
            round: -1,
            salt_state: salt,
            use_alt,
            cache: None,
        }
    }

    /// One PRNG byte. `flag=true` arms a 1-deep cache (the next call
    /// returns the same byte) — used exactly once for the `}` terminator.
    pub fn next(&mut self, flag: bool) -> u8 {
        if let Some(c) = self.cache.take() {
            return c;
        }
        self.round += 1;
        if self.round > 2 {
            self.state = mix_int(self.state);
            self.round = 0;
        }
        let shift = 16 - 8 * self.round; // 16, 8, 0
        let mut result = self.state >> shift; // JS signed >>
        if self.use_alt {
            self.salt_state = self.salt_state.wrapping_sub(1); // pre-decrement
            result ^= self.salt_state;
        }
        let byte = (result & 255) as u8;
        if flag {
            self.cache = Some(byte);
        }
        byte
    }
}

/// `_utf8Xor`: UTF-8 encode `s`, then XOR each byte with `prng.next(false)`.
fn utf8_xor(s: &str, prng: &mut DdPrng, out: &mut Vec<u8>) {
    for b in s.as_bytes() {
        out.push(b ^ prng.next(false));
    }
}

/// JSON-stringify a scalar exactly as JS `JSON.stringify` would for the
/// types DataDome signals use (string / integer / bool). serde_json
/// matches JS for these (ASCII strings, no float exponent edge cases in
/// the signal map). Strings get quoted+escaped; numbers/bools bare.
fn js_json(v: &DdValue) -> String {
    match v {
        DdValue::Str(s) => serde_json::to_string(s).unwrap_or_default(),
        DdValue::Int(n) => n.to_string(),
        DdValue::Bool(b) => b.to_string(),
    }
}

/// A DataDome signal value (the only types `_addSignal` accepts).
#[derive(Debug, Clone)]
pub enum DdValue {
    Str(String),
    Int(i64),
    Bool(bool),
}

/// Full interstitial-path encryptor. Construct with an explicit salt
/// for determinism / byte-parity testing (the interstitial path also
/// supplies the salt externally). `hash` and `cid` are the
/// `dd={...}` interstitial fields.
///
/// **DEAD CODE (FP-Class-A, 2026-05-16).** Byte-verified by its unit
/// tests but has **zero non-test callers** — the live DataDome path is
/// the in-V8 i.js self-solve (FP-E1 / engine docs §11), not this Rust
/// encryptor. Kept as verified "insurance"/reference, NOT wired. If
/// you add a non-test caller, update this label and
/// `crates/akamai/tests/dead_code_labels.rs`.
pub struct DdEncryptor {
    cid: String,
    salt: i32,
    prng: DdPrng,
    buffer: Vec<u8>,
}

impl DdEncryptor {
    pub fn new_interstitial(hash: &str, cid: &str, salt: i32) -> Self {
        let seed = main_prng_seed(hash);
        Self {
            cid: cid.to_string(),
            salt,
            // The main prng is the one created while `_useAlt == true`.
            prng: DdPrng::new(seed, salt, true),
            buffer: Vec::new(),
        }
    }

    /// `_addSignal`: skip `xt1` / empty key. startByte =
    /// `prng() ^ (buffer.empty ? 123 : 44)`, then xor'd key, `:`, value.
    pub fn add(&mut self, key: &str, value: DdValue) {
        if key.is_empty() || key == "xt1" {
            return;
        }
        let key_str = serde_json::to_string(key).unwrap_or_default();
        let val_str = js_json(&value);
        let marker: u8 = if self.buffer.is_empty() { 123 } else { 44 }; // '{' / ','
        let start = self.prng.next(false) ^ marker;
        self.buffer.push(start);
        utf8_xor(&key_str, &mut self.prng, &mut self.buffer);
        let sep = 58u8 ^ self.prng.next(false); // ':'
        self.buffer.push(sep);
        utf8_xor(&val_str, &mut self.prng, &mut self.buffer);
    }

    /// `_buildPayload` + `_encodePayload`. Consumes the accumulated
    /// buffer; returns the custom-base64 payload string.
    pub fn finish(mut self) -> String {
        let cid_seed = cid_prng_seed(&self.cid);
        // cidPrng is created after the main prng → `_useAlt` is false.
        let mut cid_prng = DdPrng::new(cid_seed, self.salt, false);
        let mut out: Vec<u8> = Vec::with_capacity(self.buffer.len() + 1);
        for &b in &self.buffer {
            out.push(b ^ cid_prng.next(false));
        }
        // Terminator: 125 ('}') ^ prng(true) ^ cidPrng().
        let term = 125u8 ^ self.prng.next(true) ^ cid_prng.next(false);
        out.push(term);
        encode_payload(&out, self.salt)
    }
}

/// `_encodePayload`: 3-byte groups → 4×6-bit, with a per-byte
/// decrementing-salt XOR (`255 & --n ^ byte`), custom-base64 encoded,
/// padding trimmed by `len % 3`.
fn encode_payload(bytes: &[u8], salt: i32) -> String {
    let mut n = salt;
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len() / 3 * 4 + 4);
    let mut i = 0;
    while i + 2 < bytes.len() {
        n = n.wrapping_sub(1);
        let b0 = ((255 & n) as u8) ^ bytes[i];
        n = n.wrapping_sub(1);
        let b1 = ((255 & n) as u8) ^ bytes[i + 1];
        n = n.wrapping_sub(1);
        let b2 = ((255 & n) as u8) ^ bytes[i + 2];
        let chunk = ((b0 as i32) << 16) | ((b1 as i32) << 8) | (b2 as i32);
        out.push(encode_6bits((chunk >> 18) & 63));
        out.push(encode_6bits((chunk >> 12) & 63));
        out.push(encode_6bits((chunk >> 6) & 63));
        out.push(encode_6bits(chunk & 63));
        i += 3;
    }
    // Trailing 1 or 2 bytes (mod) — JS still processes the partial group
    // (reading past-end as undefined→NaN→0 after `& byte`), then trims
    // `3 - mod` output chars. We mirror by processing remaining bytes as
    // 0-filled then trimming.
    let rem = bytes.len() - i;
    if rem > 0 {
        let mut g = [0u8; 3];
        for (k, slot) in g.iter_mut().enumerate() {
            n = n.wrapping_sub(1);
            let src = if i + k < bytes.len() { bytes[i + k] } else { 0 };
            *slot = ((255 & n) as u8) ^ src;
        }
        let chunk = ((g[0] as i32) << 16) | ((g[1] as i32) << 8) | (g[2] as i32);
        out.push(encode_6bits((chunk >> 18) & 63));
        out.push(encode_6bits((chunk >> 12) & 63));
        out.push(encode_6bits((chunk >> 6) & 63));
        out.push(encode_6bits(chunk & 63));
        // mod = rem; drop (3 - rem) chars.
        out.truncate(out.len() - (3 - rem));
    }
    // Custom alphabet codepoints are already ASCII-safe bytes.
    String::from_utf8_lossy(&out).into_owned()
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

    #[test]
    fn prng_first_round_shifts_hand_traced() {
        // seed = mix_int(1) = 270369 = 0x00042021. Rounds 0,1,2 (before
        // any re-mix) shift by 16,8,0 then &255:
        //   >>16 = 0x0004 → 4
        //   >>8  = 0x0420 → 0x20 = 32
        //   >>0  = 0x42021 → 0x21 = 33
        let mut p = DdPrng::new(270_369, 0, false);
        assert_eq!(p.next(false), 4);
        assert_eq!(p.next(false), 32);
        assert_eq!(p.next(false), 33);
    }

    #[test]
    fn prng_use_alt_xors_predecremented_salt() {
        // useAlt: result ^= --saltState. saltState 1000 → 999;
        // first result 4 ^ 999 = 0x3E3; &255 = 0xE3 = 227.
        let mut p = DdPrng::new(270_369, 1000, true);
        assert_eq!(p.next(false), 227);
    }

    #[test]
    fn prng_cache_repeats_on_flag() {
        // flag=true arms a 1-deep cache: the NEXT call returns the same
        // byte without advancing state.
        let mut p = DdPrng::new(270_369, 0, false);
        // next(true): round 0 → byte 4, cache armed (round stays 0).
        let a = p.next(true);
        // cache replay: returns 4 again WITHOUT advancing round.
        let b = p.next(false);
        assert_eq!(a, b, "flag cache must repeat the byte");
        assert_eq!(a, 4);
        // Stream resumes at round 1 (byte 32) — the cached call did not
        // consume a round.
        assert_eq!(p.next(false), 32);
    }

    #[test]
    fn encryptor_deterministic_for_pinned_salt() {
        // Same (hash,cid,salt,signals) ⇒ identical payload. Salt pinned
        // so the result is fully deterministic (interstitial path
        // supplies an explicit salt; byte-parity vs the Node reference
        // is a separate fixed-salt-fixture test, see doc 18 §Port-plan).
        let mk = || {
            let mut e = DdEncryptor::new_interstitial(
                "14D062F60A4BDE8CE8647DFC720349",
                "test-cid-abc",
                424242,
            );
            e.add("iaG6RD", DdValue::Str("1.16.2".into()));
            e.add("PUuTxz", DdValue::Int(0));
            e.add("flagk", DdValue::Bool(true));
            e.add("xt1", DdValue::Int(9)); // must be skipped
            e.finish()
        };
        let a = mk();
        let b = mk();
        assert_eq!(a, b, "encryptor must be deterministic for a pinned salt");
        assert!(!a.is_empty());
        // Custom-base64 output is ASCII printable.
        assert!(a.bytes().all(|c| (45..=122).contains(&c)));
    }

    #[test]
    fn byte_parity_vs_glizzykingdreko_node_reference() {
        // Fixture captured 2026-05-15 from a one-off run of the
        // authoritative Node reference
        // (glizzykingdreko/datadome-encryption src/encryption.js,
        // challengeType='interstitial') with a PINNED salt so the
        // result is fully deterministic:
        //
        //   const e = new DataDomeEncryptor(
        //       "14D062F60A4BDE8CE8647DFC720349", "test-cid-abc",
        //       424242, 'interstitial');
        //   e.add("iaG6RD","1.16.2"); e.add("PUuTxz",0);
        //   e.add("flagk",true);      e.add("xt1",9);  // skipped
        //   e.encrypt()  ->  the EXPECTED string below (len 58)
        //
        // This is the load-bearing correctness test: byte-exact match
        // with the reference proves our port (primitives + PRNG +
        // buffer + payload) is faithful end-to-end.
        const EXPECTED: &str = "r-aCk21w_gy22p95upHvmSEliaBlfgcgzBeEwuIr3dk0D6HtzqBFgBcQtE";
        let mut e =
            DdEncryptor::new_interstitial("14D062F60A4BDE8CE8647DFC720349", "test-cid-abc", 424242);
        e.add("iaG6RD", DdValue::Str("1.16.2".into()));
        e.add("PUuTxz", DdValue::Int(0));
        e.add("flagk", DdValue::Bool(true));
        e.add("xt1", DdValue::Int(9));
        let got = e.finish();
        assert_eq!(
            got, EXPECTED,
            "Rust port diverges from glizzykingdreko Node reference"
        );
    }
}
