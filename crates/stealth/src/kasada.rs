//! Kasada `x-kpsdk-cd` proof-of-work solver — native Rust port.
//!
//! Kasada-protected sites (canadagoose.com, hyatt.com, ticketmaster.com,
//! etc.) require an `x-kpsdk-cd` request header on every protected
//! resource fetch. The header is `JSON.stringify({workTime, id, answers,
//! duration})` where `answers` is the SHA-256 proof-of-work solution to
//! a small challenge issued in the response of the prior `/tl` POST.
//!
//! Algorithm source: deobfuscated Kasada `ips.js` (`Humphryyy/Kasada-
//! Deobfuscated`, function `ovida`). The token is `x-kpsdk-ct` (issued by
//! Kasada's `/tl` endpoint after we POST the fingerprint payload). The PoW
//! solution proves we did the work.
//!
//! Spec (canonical, public for years):
//!
//! ```text
//! ovida(challenge, workTime, id):
//!   jonta = sha256(challenge.platformInputs + ", " + workTime + ", " + id)
//!   cayleen = challenge.difficulty / challenge.subchallengeCount
//!   answers = []
//!   for i in 0..subchallengeCount:
//!     anyjha = 1
//!     loop:
//!       quashanna = sha256(anyjha + ", " + jonta)
//!       if hashDifficulty(quashanna) >= cayleen:
//!         answers.push(anyjha)
//!         jonta = quashanna
//!         break
//!       anyjha += 1
//!   return { answers, finalHash: jonta }
//!
//! hashDifficulty(h) = 0x10000000000000 / (parseInt(h[0..13], 16) + 1)
//! ```
//!
//! For canadagoose.com (and most public Kasada deployments as of Apr 2026):
//! `difficulty=10, subchallengeCount=2, platformInputs="tp-v2-input"`.
//! Per-subchallenge difficulty = 5 → mean ~32 SHA-256 iterations per
//! subchallenge → <50 ms total on a laptop.
//!
//! See:
//! - <https://github.com/Humphryyy/Kasada-Deobfuscated>
//! - <https://docs.antibot.to/reference/kasada/x-kpsdk-cd>
//! - <https://github.com/lktop/kpsdk>

use rand::RngExt;
use serde::Serialize;
use sha2::{Digest, Sha256};

/// Kasada PoW challenge inputs.
#[derive(Debug, Clone)]
pub struct KasadaChallenge {
    /// Total difficulty target (split across subchallenges). Public deployments
    /// use 10 in 2026.
    pub difficulty: u32,
    /// Number of subchallenges (each gets `difficulty / subchallenge_count`).
    /// Public deployments use 2.
    pub subchallenge_count: u32,
    /// First field of the seed string. Public deployments use "tp-v2-input".
    pub platform_inputs: String,
}

impl Default for KasadaChallenge {
    fn default() -> Self {
        Self {
            difficulty: 10,
            subchallenge_count: 2,
            platform_inputs: "tp-v2-input".into(),
        }
    }
}

/// Solved PoW. Serialize this (with optional duration/st/rst) as the JSON
/// value for the `x-kpsdk-cd` request header.
///
/// Per the Antibot.to spec the server may require `st` (server timestamp,
/// echoed back from `x-kpsdk-st`) and `rst` (request start time, the
/// browser-local `KPSDK.start` value) in addition to `workTime/id/answers`.
/// Both are populated when known; absent otherwise.
#[derive(Debug, Clone, Serialize)]
pub struct KasadaSolution {
    /// Local clock at solve start (ms since Unix epoch), adjusted by
    /// `serverOffset = serverTimeMs - localTimeMs` from `x-kpsdk-st`.
    #[serde(rename = "workTime")]
    pub work_time: i64,
    /// Random 32-char lowercase-hex session id.
    pub id: String,
    /// PoW counter for each subchallenge.
    pub answers: Vec<u32>,
    /// Wall-clock solve duration in milliseconds. Optional but recommended:
    /// Kasada flags solves that complete implausibly fast or in suspiciously
    /// uniform time. Sample from a Sigma-Lognormal-ish distribution to match
    /// human + Chrome JIT solve times (median ~1500 ms).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<u32>,
    /// Server timestamp in ms (echo of `x-kpsdk-st`). Required by some
    /// Kasada deployments to validate `workTime` against server clock.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub st: Option<i64>,
    /// Request start time in ms (typically `KPSDK.start * 1000` rounded —
    /// the page-relative `performance.now()` at the moment ips.js began
    /// solving). Some deployments require this for replay-attack defense.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rst: Option<f64>,
    /// Kasada version (typically 1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub v: Option<u32>,
    /// Estimated clock drift (rst - st).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub d: Option<i64>,
}

impl KasadaSolution {
    /// Serialize as a single-line JSON string suitable for the
    /// `x-kpsdk-cd` request header value.
    pub fn to_header_value(&self) -> String {
        // Manual ordering per the canonical Kasada output:
        //   {"workTime":...,"id":"...","answers":[...],"duration":...}
        // We use serde_json which preserves insertion order in the derive.
        serde_json::to_string(self).unwrap_or_default()
    }
}

/// Compute the difficulty of a SHA-256 hex digest. Higher = "harder".
/// We accept iff `hash_difficulty(h) >= per_subchallenge_target`.
fn hash_difficulty(hex_digest: &str) -> f64 {
    // Take first 13 hex chars (52 bits — fits in u64), parse as integer,
    // return 2^52 / (n + 1).
    let prefix = &hex_digest[..13];
    let n = u64::from_str_radix(prefix, 16).unwrap_or(0);
    (1u64 << 52) as f64 / (n as f64 + 1.0)
}

fn sha256_hex(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    hex::encode(h.finalize())
}

/// Solve the PoW. `work_time_ms` should be `Date.now() + server_offset`,
/// where `server_offset = parse(x-kpsdk-st) - localNow`. `id` is a 32-char
/// lowercase-hex session id (use [`generate_session_id`]).
///
/// **Measures real wall-clock duration** of the SHA-256 loop and stamps it
/// into `solution.duration`. Per Apr 2026 research (deobfuscated `ovida`
/// from Humphryyy/Kasada-Deobfuscated, plus stricter-tenant behavior on
/// the `/149e9513-.../2d206a39-.../tl` template — VEVE, canadagoose,
/// hyatt), the server validates `duration` for plausibility against the
/// PoW difficulty. A constant or randomly-sampled value can fail.
pub fn solve(challenge: &KasadaChallenge, work_time_ms: i64, id: &str) -> KasadaSolution {
    let t0 = std::time::Instant::now();
    let target_per_sub = challenge.difficulty as f64 / challenge.subchallenge_count.max(1) as f64;

    // Initial seed: sha256("tp-v2-input, <alignedWorkTime>, <id>")
    // Kasada ips.js uses Math.round(Date.now() / 18000081) * 10 as part of the seed.
    // We match this alignment to ensure our PoW answers are valid for the current window.
    let aligned_work_time = (work_time_ms as f64 / 18000081.0).round() as i64 * 10;

    let mut jonta = sha256_hex(&format!(
        "{}, {}, {}",
        challenge.platform_inputs, aligned_work_time, id
    ));

    let mut answers: Vec<u32> = Vec::with_capacity(challenge.subchallenge_count as usize);
    for _ in 0..challenge.subchallenge_count {
        let mut anyjha: u32 = 1;
        loop {
            let quashanna = sha256_hex(&format!("{}, {}", anyjha, jonta));
            if hash_difficulty(&quashanna) >= target_per_sub {
                answers.push(anyjha);
                jonta = quashanna;
                break;
            }
            anyjha = anyjha.checked_add(1).unwrap_or_else(|| {
                // u32 overflow — would mean > 4 billion iterations; shouldn't
                // happen at difficulty=10 (mean ~32 per sub). Bail with the
                // last value so caller sees the stuck state.
                u32::MAX
            });
            if anyjha == u32::MAX {
                break;
            }
        }
    }

    // Real solve duration. Kasada compares this against PoW difficulty to
    // catch implausibly fast solves; the natural per-machine variance gives
    // us a believable distribution without needing to sample manually.
    let duration_ms = t0.elapsed().as_millis().min(u32::MAX as u128) as u32;

    KasadaSolution {
        work_time: work_time_ms,
        id: id.to_string(),
        answers,
        duration: Some(duration_ms),
        st: None,
        rst: None,
        v: None,
        d: None,
    }
}

/// Generate a Kasada-style session id: 32 lowercase hex chars (16 random
/// bytes). Per the deobfuscated `makeId()` in `ips.js`.
pub fn generate_session_id<R: rand::Rng>(rng: &mut R) -> String {
    let mut bytes = [0u8; 16];
    rng.fill(&mut bytes);
    hex::encode(bytes)
}

/// Convenience: solve with default challenge params (the public Kasada
/// deployment values: difficulty=10, subchallenges=2, "tp-v2-input").
pub fn solve_default(work_time_ms: i64, id: &str) -> KasadaSolution {
    solve(&KasadaChallenge::default(), work_time_ms, id)
}

/// Solve with default params AND attach a plausible `duration` field
/// sampled from a LogNormal distribution (median ~1500 ms, σ=0.4) that
/// matches real human-Chrome solve times. Per Apr 2026 research: Kasada
/// flags solves that complete in implausibly fast or suspiciously uniform
/// time. Use this in production wire-ups; use [`solve_default`] in tests
/// where determinism matters.
pub fn solve_with_realistic_duration<R: rand::Rng>(
    work_time_ms: i64,
    id: &str,
    rng: &mut R,
) -> KasadaSolution {
    use rand_distr::{Distribution, LogNormal};
    let mut sol = solve_default(work_time_ms, id);
    // LogNormal(μ=ln 1500, σ=0.4) clamped to [400, 8000] ms — covers the
    // observed Chrome solve-time distribution from public Kasada captures.
    let dist = LogNormal::new(1500.0_f64.ln(), 0.4).expect("valid lognormal");
    let raw = dist.sample(rng);
    let duration = raw.clamp(400.0, 8000.0) as u32;
    sol.duration = Some(duration);
    sol
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn hash_difficulty_known_values() {
        // h[0..13] = "0000000000000" → n=0 → 2^52/1 = huge → very high difficulty
        let easy =
            hash_difficulty("00000000000000000000000000000000000000000000000000000000ffffffff");
        assert!(easy > 1e15);
        // h[0..13] = "fffffffffffff" → n = 2^52-1 → 2^52/2^52 ≈ 1
        let hard =
            hash_difficulty("ffffffffffffff0000000000000000000000000000000000000000000000ffff");
        assert!(hard < 2.0);
    }

    #[test]
    fn solve_default_produces_two_answers() {
        let sol = solve_default(1_777_000_000_000, "abcdef0123456789abcdef0123456789");
        assert_eq!(sol.answers.len(), 2);
        // Each answer must be > 0
        assert!(sol.answers.iter().all(|&a| a > 0));
        // Default difficulty=10, subchallenge_count=2 → per-sub target = 5.
        // Mean iterations per subchallenge ≈ 2^52 / target ≈ uniform sample
        // from [1, 2^52/5] but with rejection — empirically <500 per sub.
        // Sanity bound:
        assert!(
            sol.answers.iter().all(|&a| a < 100_000),
            "implausible answer: {:?}",
            sol.answers
        );
    }

    #[test]
    fn solve_is_deterministic() {
        let a = solve_default(1_777_000_000_000, "abcdef0123456789abcdef0123456789");
        let b = solve_default(1_777_000_000_000, "abcdef0123456789abcdef0123456789");
        assert_eq!(a.answers, b.answers);
        assert_eq!(a.work_time, b.work_time);
        assert_eq!(a.id, b.id);
    }

    #[test]
    fn solve_differs_for_different_inputs() {
        let a = solve_default(1_777_000_000_000, "00000000000000000000000000000001");
        let b = solve_default(1_777_000_000_000, "00000000000000000000000000000002");
        assert_ne!(a.answers, b.answers);
    }

    #[test]
    fn header_value_is_valid_json_with_required_fields() {
        let sol = solve_default(1_777_000_000_000, "abcdef0123456789abcdef0123456789");
        let header = sol.to_header_value();
        let parsed: serde_json::Value = serde_json::from_str(&header).expect("valid JSON");
        assert!(parsed["workTime"].is_i64());
        assert!(parsed["id"].is_string());
        assert!(parsed["answers"].is_array());
        assert!(parsed["answers"].as_array().unwrap().len() == 2);
        // solve() now stamps real wall-clock duration of the SHA-256 loop.
        // For difficulty=10 it's typically <5 ms but never absent.
        assert!(parsed["duration"].is_u64());
    }

    #[test]
    fn header_value_includes_duration_when_set() {
        let mut sol = solve_default(1_777_000_000_000, "abcdef0123456789abcdef0123456789");
        sol.duration = Some(1432);
        let header = sol.to_header_value();
        assert!(header.contains("\"duration\":1432"));
    }

    #[test]
    fn generate_session_id_is_32_hex_chars() {
        let mut rng = rand_chacha::ChaCha20Rng::seed_from_u64(42);
        let id = generate_session_id(&mut rng);
        assert_eq!(id.len(), 32);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn solve_completes_in_reasonable_time() {
        // Confirms the algorithm isn't silently looping forever.
        let t0 = std::time::Instant::now();
        let _ = solve_default(1_777_000_000_000, "deadbeefcafef00ddeadbeefcafef00d");
        let elapsed = t0.elapsed();
        assert!(
            elapsed.as_millis() < 5000,
            "solve took {} ms — should be <500 ms",
            elapsed.as_millis()
        );
    }

    #[test]
    fn answers_satisfy_difficulty_target_when_replayed() {
        // Re-derive the chain from the answers and verify each step satisfies
        // the per-sub difficulty target. This proves the answers are valid.
        let challenge = KasadaChallenge::default();
        let work_time = 1_777_000_000_000_i64;
        let id = "abcdef0123456789abcdef0123456789";
        let sol = solve(&challenge, work_time, id);

        let aligned_work_time = (work_time as f64 / 18000081.0).round() as i64 * 10;

        let mut jonta = sha256_hex(&format!(
            "{}, {}, {}",
            challenge.platform_inputs, aligned_work_time, id
        ));
        let target = challenge.difficulty as f64 / challenge.subchallenge_count as f64;
        for &ans in &sol.answers {
            let q = sha256_hex(&format!("{}, {}", ans, jonta));
            assert!(
                hash_difficulty(&q) >= target,
                "answer {ans} does not satisfy target {target} (hash diff {})",
                hash_difficulty(&q)
            );
            jonta = q;
        }
    }
}
