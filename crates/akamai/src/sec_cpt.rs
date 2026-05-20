//! W4.2 — Akamai sec-cpt proof-of-work challenge solver.
//!
//! When Akamai's Bot Manager Premier "Strict Response" threshold fires,
//! the response is HTTP 428 with body containing a sec-cpt challenge:
//!
//! ```json
//! {
//!   "token":      "AAQAAAAJ...",          // ~430-char base64 token
//!   "timestamp":  1713283747,             // unix epoch
//!   "nonce":      "ebccdb479fcb92636fbc", // 20-hex-char random
//!   "difficulty": 15000,                  // PoW target
//!   "count":      1,                      // number of answers required
//!   "timeout":    1000,                   // ms per answer attempt
//!   "cpu":        false,                  // CPU-only (no GPU)
//!   "verify_url": "/_sec/cp_challenge/verify"
//! }
//! ```
//!
//! For the `crypto` provider (pure PoW), each answer is a base-16 float
//! `r = "0.<hexdigits>"` such that:
//!
//! ```text
//! input = sec + str(timestamp) + nonce + str(difficulty + i)
//! h = sha256(input + r)
//! output = 0
//! for b in h: output = ((output << 8) | b) & 0xFFFFFFFF; output = output % (difficulty + i)
//! return r if output == 0 else retry
//! ```
//!
//! Note the rolling-hash reduction (not `int(h, 16) % D`): each byte
//! gets shifted-then-modded-once before the next byte joins. This lets
//! Akamai grow the difficulty per answer index without changing the
//! base sha256 step.
//!
//! Algorithm verified against hyper-sdk-go/akamai/sec_cpt.go's
//! `generateSecCptAnswers`. Typical cost at difficulty=15000 is
//! ~5 ms / answer on one CPU core; Akamai's `chlg_duration` enforced
//! wait (5–30 s) is the real time floor.

use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Task#3: has the in-V8 sec-cpt bundle reached the **solved** state?
///
/// The `sec_cpt` cookie is `sec_cpt=<sec>~<state>~…`; Akamai advances
/// `<state>` to `3` once the obfuscated bundle's PoW + enforced
/// `chlg_duration` wait complete and the challenge is cleared (the
/// `~3~` marker, per docs.hypersolutions.co / 02_AKAMAI.md §5). Bare
/// presence of the cookie is NOT success — it is set unsolved on the
/// 428 too (same false-success class as DataDome's cookie; cf.
/// `datadome_handler::datadome_solved`).
///
/// **Structural invariant (documented, enforced by the navigate loop,
/// not here):** clearing sec-cpt needs **≥2 nav iterations** — iter 0
/// serves the interstitial and the bundle self-solves to `~3~`; the
/// post-solve reload that fetches the real content is hard-gated
/// `iter + 1 < iterations` in `navigate_loop_internal`. A 1-iteration
/// nav can reach `~3~` but structurally cannot render the cleared
/// page. (This is why the strict 1-iter audit shows homedepot blocked
/// while the sanctioned 3-iter `holistic_sweep` metric shows it
/// `L3-RENDERED` — different lenses, not a regression.)
pub fn sec_cpt_solved(cookies: &str) -> bool {
    cookies
        .split(';')
        .map(|c| c.trim())
        .filter_map(|c| c.strip_prefix("sec_cpt="))
        .any(|v| v.contains("~3~"))
}

/// sec-cpt challenge payload as decoded from the 428 response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecCptChallenge {
    pub token: String,
    pub timestamp: u64,
    pub nonce: String,
    pub difficulty: u64,
    pub count: u32,
    #[serde(default = "default_timeout")]
    pub timeout: u32,
    #[serde(default)]
    pub cpu: bool,
    pub verify_url: String,
}

fn default_timeout() -> u32 {
    1000
}

/// Verification body posted to `/_sec/verify?provider=crypto`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecCptAnswerSubmission {
    pub token: String,
    pub answers: Vec<String>,
}

/// Solve the `crypto` provider PoW for the given challenge. The `sec`
/// argument is the leading-`~` portion of the `sec_cpt` cookie (the
/// part before the first `~`). Returns Vec<answer-string> of length
/// `challenge.count`.
///
/// `sec` is the leading portion of the sec_cpt cookie value (before
/// the first `~`). It's used as challenge input prefix; the cookie
/// itself is set by the 428-bearing response.
///
/// **DEAD CODE (FP-Class-A, 2026-05-16).** Byte-verified by its unit
/// tests but has **zero non-test callers** — the live navigate path
/// relies on the obfuscated sec-cpt bundle self-solving in V8, not on
/// this Rust solver (homedepot serves no parseable 428 JSON to feed
/// it). Kept as a verified reference, NOT wired. If you add a non-test
/// caller, update this label and
/// `crates/akamai/tests/dead_code_labels.rs`. Do NOT "wire it" — that
/// is an explicitly-eliminated dead-end (master plan §6).
pub fn solve_crypto(challenge: &SecCptChallenge, sec: &str) -> Vec<String> {
    let mut answers = Vec::with_capacity(challenge.count as usize);
    for i in 0..challenge.count {
        let target_d = challenge.difficulty + i as u64;
        let prefix = format!(
            "{}{}{}{}",
            sec, challenge.timestamp, challenge.nonce, target_d
        );
        let answer = find_answer(&prefix, target_d);
        answers.push(answer);
    }
    answers
}

/// Find a base-16 float `r` such that the rolling-hash reduction of
/// `sha256(prefix + r)` produces 0 modulo `difficulty`. Tries random
/// candidates until one satisfies; expected attempts ≈ difficulty/256.
fn find_answer(prefix: &str, difficulty: u64) -> String {
    let mut rng = rand::thread_rng();
    loop {
        // Real Akamai uses Math.random().toString(16) → "0.<hex>".
        // Generate a hex float of similar length (~13 hex chars after the dot).
        let frac: u64 = rng.gen::<u64>() & 0x000F_FFFF_FFFF_FFFF;
        let candidate = format!("0.{:013x}", frac);

        let mut hasher = Sha256::new();
        hasher.update(prefix.as_bytes());
        hasher.update(candidate.as_bytes());
        let h = hasher.finalize();

        // Rolling-hash reduction per hyper-sdk-go.
        let mut output: u64 = 0;
        for &b in h.iter() {
            output = ((output << 8) | b as u64) & 0xFFFF_FFFF;
            output %= difficulty;
        }
        if output == 0 {
            return candidate;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Task#3 regression: only the `~3~` solved state counts; the bare
    // (unsolved) sec_cpt cookie set on the 428 must NOT read as solved.
    #[test]
    fn sec_cpt_solved_requires_state_3() {
        // Unsolved: cookie present but state != 3 (set on the 428).
        assert!(!sec_cpt_solved("foo=1; sec_cpt=abc~1~xyz; bar=2"));
        assert!(!sec_cpt_solved("sec_cpt=abc~2~xyz"));
        assert!(!sec_cpt_solved("")); // no cookie
        assert!(!sec_cpt_solved("notsec_cpt=abc~3~")); // wrong cookie name
        // Solved: state advanced to 3 by the in-V8 bundle.
        assert!(sec_cpt_solved("foo=1; sec_cpt=abc~3~deadbeef; bar=2"));
        assert!(sec_cpt_solved("sec_cpt=longprefix~3~tail"));
    }

    #[test]
    fn solves_low_difficulty_within_seconds() {
        // Use a difficulty low enough that the test always finishes
        // (~256 attempts on average) but not so trivial it passes by
        // accident. 256 = "first reduction pass terminates immediately".
        let chal = SecCptChallenge {
            token: "test-token".into(),
            timestamp: 1_713_283_747,
            nonce: "ebccdb479fcb92636fbc".into(),
            difficulty: 256,
            count: 1,
            timeout: 1000,
            cpu: false,
            verify_url: "/_sec/cp_challenge/verify".into(),
        };
        let answers = solve_crypto(&chal, "secprefix");
        assert_eq!(answers.len(), 1);
        let r = &answers[0];
        assert!(r.starts_with("0."), "answer should be base-16 float: {r}");

        // Verify the answer satisfies the rolling-hash check.
        let prefix = format!(
            "{}{}{}{}",
            "secprefix", chal.timestamp, chal.nonce, chal.difficulty
        );
        let mut h = Sha256::new();
        h.update(prefix.as_bytes());
        h.update(r.as_bytes());
        let digest = h.finalize();
        let mut output: u64 = 0;
        for &b in digest.iter() {
            output = ((output << 8) | b as u64) & 0xFFFF_FFFF;
            output %= 256;
        }
        assert_eq!(output, 0, "rolling-hash reduction not 0 for answer {r}");
    }

    #[test]
    fn produces_distinct_answers_per_count() {
        let chal = SecCptChallenge {
            token: "test-token".into(),
            timestamp: 1_713_283_747,
            nonce: "ebccdb479fcb92636fbc".into(),
            difficulty: 256,
            count: 3,
            timeout: 1000,
            cpu: false,
            verify_url: "/_sec/cp_challenge/verify".into(),
        };
        let answers = solve_crypto(&chal, "secprefix");
        assert_eq!(answers.len(), 3);
        // Different answer indices have different target difficulties
        // (256, 257, 258) so the answers should not all match.
        assert!(
            !(answers[0] == answers[1] && answers[1] == answers[2]),
            "expected distinct answers, got {answers:?}"
        );
    }

    #[test]
    fn submission_serializes_to_token_answers_shape() {
        let s = SecCptAnswerSubmission {
            token: "AAQ...".into(),
            answers: vec!["0.1abc".into(), "0.2def".into()],
        };
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"token\":\"AAQ...\""));
        assert!(json.contains("\"answers\":[\"0.1abc\",\"0.2def\"]"));
    }
}
