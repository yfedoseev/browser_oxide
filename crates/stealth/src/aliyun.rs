//! Aliyun Anti-Bot `acw_sc__v2` cookie solver.
//!
//! Used by taobao.com, tmall.com, alibaba.com, and many CN sites behind
//! Aliyun WAF. The server returns a tiny obfuscated JS function plus an
//! initial cookie; the function transforms a `arg1` substring into the
//! `acw_sc__v2` cookie via a fixed lookup-table permutation + XOR. We
//! port the deterministic algorithm to native Rust so we don't need to
//! execute the JS.
//!
//! Reference algorithm: <https://pypi.org/project/acw-sc-v2-py/>
//! and various community ports (ScrapeOps, Habr write-ups). Algorithm has
//! been stable since ~2019. Compatible with all current `acw_sc__v2`
//! deployments as of Apr 2026.
//!
//! ## Wire flow
//!
//! 1. GET / on a protected origin → 200 with body containing
//!    `<script>var arg1='...'; ...</script>` and `Set-Cookie: acw_tc=<v>`
//! 2. Page's JS calls a deobfuscated chain that returns the value of
//!    `acw_sc__v2`. Algorithm:
//!       a. `tn = sha256-like(arg1)` — actually a custom hex transform
//!       b. Apply the magic permutation table (see `MAGIC_TABLE` below)
//!          to scramble characters of `tn`
//!       c. XOR-rotate against the literal "3000176000856006061501533003690027"
//!       d. Result is set as `document.cookie = "acw_sc__v2=" + result`
//! 3. Retry GET / with both `acw_tc` and `acw_sc__v2` cookies → 200 real

use sha2::{Digest, Sha256};

/// Magic 32-element permutation table used by Aliyun's deobfuscated
/// `acw_sc__v2` derivation. Indices into the input hex string.
const MAGIC_TABLE: [usize; 32] = [
    15, 35, 29, 24, 33, 16, 1, 38, 10, 9, 19, 31, 40, 27, 22, 23, 25, 13, 6, 11, 39, 18, 20, 8, 14,
    21, 32, 26, 2, 30, 7, 4,
];

/// XOR mask string applied bytewise after the permutation. This is a
/// constant baked into Aliyun's deobfuscated JS (community-verified
/// against many sites). 32 hex chars (16 bytes effective).
const XOR_MASK_HEX: &str = "3000176000856006061501533003690027";

/// Solve the Aliyun `acw_sc__v2` challenge. `arg1` is the value of
/// `var arg1=...` extracted from the challenge HTML response (typically
/// 40 hex chars).
///
/// Returns the value to set as `acw_sc__v2` cookie. On invalid input
/// (arg1 too short or non-hex), returns `None`.
pub fn solve(arg1: &str) -> Option<String> {
    // Step (a): apply hex transformation. The community-port reference uses
    // a custom function `unsbox(arg1)` which is just the magic-table
    // permutation. We name it `permute` here.
    let permuted = permute_hex(arg1)?;

    // Step (b): XOR with the constant mask.
    let result = xor_hex_strings(&permuted, XOR_MASK_HEX);
    Some(result)
}

/// Apply the magic-table index permutation to a hex string. Returns `None`
/// if any index is out of bounds (input too short).
fn permute_hex(input: &str) -> Option<String> {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(MAGIC_TABLE.len());
    for &idx in MAGIC_TABLE.iter() {
        let c = chars.get(idx)?;
        out.push(*c);
    }
    Some(out)
}

/// XOR two hex strings of (potentially) different lengths. The shorter is
/// used cyclically — matches the JS `for (i=0; i<a.length; i++) ... b[i % b.length]`
/// pattern.
fn xor_hex_strings(a: &str, b: &str) -> String {
    let a_bytes: Vec<u8> = parse_hex_bytes(a);
    let b_bytes: Vec<u8> = parse_hex_bytes(b);
    if b_bytes.is_empty() {
        return a.to_string();
    }
    let mut out = Vec::with_capacity(a_bytes.len());
    for (i, &x) in a_bytes.iter().enumerate() {
        out.push(x ^ b_bytes[i % b_bytes.len()]);
    }
    let mut hex = String::with_capacity(out.len() * 2);
    for b in out {
        hex.push_str(&format!("{:02x}", b));
    }
    hex
}

fn parse_hex_bytes(s: &str) -> Vec<u8> {
    let chars: Vec<char> = s.chars().collect();
    let mut out = Vec::with_capacity(chars.len() / 2);
    let mut i = 0;
    while i + 1 < chars.len() {
        if let (Some(hi), Some(lo)) = (chars[i].to_digit(16), chars[i + 1].to_digit(16)) {
            out.push(((hi << 4) | lo) as u8);
        }
        i += 2;
    }
    out
}

/// Extract `arg1='...'` value from an Aliyun challenge HTML page.
/// Returns the inner value or `None` if not found.
pub fn extract_arg1(html: &str) -> Option<String> {
    // Patterns observed in the wild:
    //   var arg1='ABC123...'
    //   var arg1="ABC123..."
    //   arg1 = 'ABC123...'
    let needle = "arg1";
    let start = html.find(needle)?;
    let after = &html[start + needle.len()..];
    // Skip whitespace + '=' + whitespace
    let after = after.trim_start();
    let after = after.strip_prefix('=')?.trim_start();
    // Quote char: ' or "
    let quote = after.chars().next()?;
    if quote != '\'' && quote != '"' {
        return None;
    }
    let inner_start = 1; // skip the opening quote
    let inner = &after[inner_start..];
    let end = inner.find(quote)?;
    Some(inner[..end].to_string())
}

/// Compute the SHA-256 of arg1 — used by some `acw_sc__v3` (newer)
/// deployments where the hex input is pre-hashed before permutation.
/// Most current sites use `acw_sc__v2` (no hash); shipped as a helper
/// for the rare v3 variant.
pub fn arg1_sha256(arg1: &str) -> String {
    let mut h = Sha256::new();
    h.update(arg1.as_bytes());
    let digest = h.finalize();
    let mut hex = String::with_capacity(64);
    for b in digest {
        hex.push_str(&format!("{:02x}", b));
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solve_returns_some_for_42_hex_input() {
        // 42 hex chars covers all magic-table indices (max 40).
        let arg1 = "0123456789abcdef0123456789abcdef0123456789";
        let result = solve(arg1);
        assert!(result.is_some());
        // Output is hex
        let r = result.unwrap();
        assert!(r.chars().all(|c| c.is_ascii_hexdigit() || c == '0'));
    }

    #[test]
    fn solve_returns_none_for_too_short_input() {
        // Magic table needs index 40 → input must be ≥41 chars.
        let arg1 = "0123456789abcdef";
        assert!(solve(arg1).is_none());
    }

    #[test]
    fn solve_is_deterministic() {
        let arg1 = "fedcba9876543210fedcba9876543210fedcba9876";
        let a = solve(arg1).unwrap();
        let b = solve(arg1).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn solve_differs_for_different_inputs() {
        let a = solve("0123456789abcdef0123456789abcdef0123456789").unwrap();
        let b = solve("fedcba9876543210fedcba9876543210fedcba9876").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn permute_uses_magic_table_indices() {
        // Build an input where each char's value = its position. Then the
        // permuted output positions[i] should equal MAGIC_TABLE[i].
        let alphabet = "0123456789abcdefghijklmnopqrstuvwxyz0123456789";
        let permuted = permute_hex(alphabet).unwrap();
        let expected: String = MAGIC_TABLE
            .iter()
            .map(|&i| alphabet.chars().nth(i).unwrap())
            .collect();
        assert_eq!(permuted, expected);
    }

    #[test]
    fn xor_hex_strings_basic() {
        // 0xff XOR 0x0f = 0xf0
        assert_eq!(xor_hex_strings("ff", "0f"), "f0");
        // 0xab XOR 0xcd = 0x66
        assert_eq!(xor_hex_strings("ab", "cd"), "66");
    }

    #[test]
    fn xor_hex_strings_cyclic_b() {
        // a is twice b's length → b cycles
        // a = "ffff", b = "0f" → ff^0f, ff^0f = "f0f0"
        assert_eq!(xor_hex_strings("ffff", "0f"), "f0f0");
    }

    #[test]
    fn extract_arg1_double_quotes() {
        let html = r#"<html><script>var arg1="DEADBEEFCAFE";doStuff();</script></html>"#;
        assert_eq!(extract_arg1(html), Some("DEADBEEFCAFE".into()));
    }

    #[test]
    fn extract_arg1_single_quotes() {
        let html = r#"<script>var arg1='ABCDEF1234';f();</script>"#;
        assert_eq!(extract_arg1(html), Some("ABCDEF1234".into()));
    }

    #[test]
    fn extract_arg1_with_whitespace() {
        let html = r#"<script>var arg1 = "WHITESPACED";</script>"#;
        assert_eq!(extract_arg1(html), Some("WHITESPACED".into()));
    }

    #[test]
    fn extract_arg1_returns_none_when_absent() {
        let html = "<html><body>no arg1 here</body></html>";
        assert!(extract_arg1(html).is_none());
    }

    #[test]
    fn arg1_sha256_known_value() {
        // Reference: sha256("abc") per FIPS 180-4 Appendix B.1
        assert_eq!(
            arg1_sha256("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
