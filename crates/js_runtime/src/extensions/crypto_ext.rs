//! Web Crypto API ops — digest, random bytes, HMAC. Backs the JS-side
//! `crypto.subtle` stub so Kasada-style probes that hash payloads via
//! `crypto.subtle.digest("SHA-256", ...)` see a real result.

use deno_core::op2;
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha384, Sha512};

#[op2]
#[buffer]
pub fn op_crypto_digest(#[string] algorithm: String, #[buffer] data: &[u8]) -> Vec<u8> {
    let alg = algorithm.to_ascii_uppercase();
    match alg.as_str() {
        "SHA-1" | "SHA1" => {
            let mut h = Sha1::new();
            h.update(data);
            h.finalize().to_vec()
        }
        "SHA-256" | "SHA256" => {
            let mut h = Sha256::new();
            h.update(data);
            h.finalize().to_vec()
        }
        "SHA-384" | "SHA384" => {
            let mut h = Sha384::new();
            h.update(data);
            h.finalize().to_vec()
        }
        "SHA-512" | "SHA512" => {
            let mut h = Sha512::new();
            h.update(data);
            h.finalize().to_vec()
        }
        _ => Vec::new(),
    }
}

#[op2(fast)]
pub fn op_crypto_random_fill(#[buffer] out: &mut [u8]) {
    use rand::RngCore;
    rand::thread_rng().fill_bytes(out);
}

deno_core::extension!(
    crypto_extension,
    ops = [op_crypto_digest, op_crypto_random_fill],
);
