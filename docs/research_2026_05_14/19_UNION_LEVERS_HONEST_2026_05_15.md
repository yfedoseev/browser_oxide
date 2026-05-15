# Honest Union-Lever Re-prioritisation (2026-05-15)

## Verified from the post-W1 4-profile sweep (definitive)

The 6 sites that never L3-RENDER on **any** profile (the true
union-ceiling blockers, union = 120/126):

| Site | Vendor | Engine-tractable? | Lever |
|---|---|---|---|
| canadagoose | Kasada | engine surface EXHAUSTED | IP-rep + Playwright-MCP A/B (tool unavailable here) |
| hyatt | Kasada | same as canadagoose | same |
| realtor | Kasada | same as canadagoose | same |
| **homedepot** | **Akamai sec-cpt** | **YES** | **W4.2 sec-cpt PoW solver** |
| yelp | DataDome `t:'bv'` | NO (operational) | IP hard-ban; needs Playwright-MCP A/B |
| douyin | regional | NO | China locale+IP, out of scope (PLAN §1) |

## The correction

W3.8 (DataDome interstitial — etsy/tripadvisor/wsj/reuters) was being
pursued as a 4-site union lever. **It is not.** Per-profile sweep:

| Site | chrome | pixel | iphone | firefox | in union? |
|---|---|---|---|---|---|
| etsy | DataDome-CHL | L3 | L3 | L3 | **YES (3/4)** |
| tripadvisor | L3 | L3 | L3 | DataDome-CHL | **YES (3/4)** |
| wsj | L3 | L3 | L3 | DataDome-CHL | **YES (3/4)** |
| reuters | L3 | L3 | L3 | L3 | **YES (4/4)** |

All four are already in the 120 routing union. W3.8 would raise
per-profile robustness (etsy on chrome, tripadvisor/wsj on firefox)
and cut variance — genuinely valuable for reliability — but it
**does not raise the union ceiling**. The byte-parity-verified
encoder + architecture work is sound and retained; its priority is
re-classified from "union lever" to "robustness hardening."

## The only engine-tractable union lever left: W4.2 (homedepot)

Of the 6 true blockers:
- canadagoose/hyatt/realtor: Kasada engine-controllable inputs are
  **exhausted** (audio FP fixed, behavioral jerk fixed, WebGL/TLS/H2
  verified-correct — doc 17). Residual is IP-reputation (structural,
  ~20-30% ML weight) — the decisive next step is the Playwright-MCP
  A/B from this IP, and that tool is unavailable in this environment
  (flagged for the user).
- yelp: `t:'bv'` = blacklist-verified IP hard-ban; no client solve
  helps (03_DATADOME §3). Operational, not engine.
- douyin: regional, explicitly out of scope.
- **homedepot: Akamai serves an `Akamai-CHL` challenge interstitial
  (sec-cpt PoW). bestbuy on the SAME Akamai stack flips GREEN this
  session — proving our core sensor path works — so homedepot is
  specifically the sec-cpt challenge we don't yet solve. This is
  W4.2: ~250 LOC port of the sec-cpt 428 PoW solver from Hyper SDK
  Go. It is the single highest-value, in-environment, engine-tractable
  lever that moves the union (120 → 121).**

## Loop re-prioritisation

1. **W4.2 — Akamai sec-cpt PoW solver (homedepot)** — primary. Moves
   the union. In-environment tractable (Rust port + unit tests, like
   the DataDome crypto). Check `crates/akamai/src/sec_cpt.rs` (exists)
   for current state.
2. W3.8 DataDome interstitial wiring — secondary (robustness, not
   union). Architecture already leverage-maximised (doc 18).
3. Kasada / yelp — blocked on Playwright-MCP A/B (flag for user); no
   further speculative engine hardening (surface exhausted).
4. douyin — out of scope.

This supersedes the prior loop focus on W3.8 as a union lever. Honest,
data-verified, per the goal's deep-research mandate.

## W4.2 wiring design (ready to apply once the capture lands)

`crates/akamai/src/sec_cpt.rs` is COMPLETE + verified vs
hyper-sdk-go's `generateSecCptAnswers` (`solves_low_difficulty…`
test passes) but `solve_crypto` is **never called** — pure dead code.
Interface:

```rust
pub struct SecCptChallenge { token, timestamp:u64, nonce:String,
    difficulty:u64, count:u32, timeout:u32, cpu:bool, verify_url:String }
pub fn solve_crypto(&SecCptChallenge, sec:&str) -> Vec<String>  // PoW
pub struct SecCptAnswerSubmission { token:String, answers:Vec<String> }
```

Wiring steps (in `page.rs`, the Akamai/nav path):

1. **Detect**: homedepot serves the sec-cpt challenge as a ~2.6 KB
   *page* (per `capture_bmak_js.rs` doc) after sensor rejection — NOT
   a clean HTTP 428 JSON. The capture experiment (in flight,
   `/tmp/homedepot_seccpt.txt`) determines whether the challenge JSON
   is (a) inline in a `<script>` on that page, or (b) at a
   `/_sec/cp_challenge/...` sub-resource the page fetches, plus the
   `sec_cpt` cookie format and `verify_url`.
2. **Parse** the `SecCptChallenge` from whichever the capture shows
   (serde_json::from_str on the inline/sub-resource JSON).
3. **`sec`** = substring of the `sec_cpt` Set-Cookie value before its
   first `~` (per `solve_crypto` doc).
4. **Solve**: `let answers = sec_cpt::solve_crypto(&chal, sec);`
   (difficulty≈15000 ⇒ a few hundred ms on our CPU per the module
   header note).
5. **Verify**: POST `serde_json::to_string(&SecCptAnswerSubmission
   { token: chal.token, answers })` to `chal.verify_url` (resolve
   relative against the homedepot origin), `Content-Type:
   application/json`, through the shared `HttpClient` so the
   validated `sec_cpt` cookie lands in the jar.
6. **Re-issue**: re-fetch the original URL (the existing
   `navigate` iteration loop already re-fetches when a pending nav
   is set; either trigger that or do an explicit re-GET). With the
   sec_cpt cookie validated, Akamai serves real content → homedepot
   flips `Akamai-CHL` → `L3-RENDERED`, union 120 → 121.

Risk/unknowns the capture resolves: exact challenge delivery (inline
vs sub-resource), the `sec_cpt` cookie name/shape, and whether
homedepot uses the `crypto` provider (pure PoW — solver handles this)
vs a `cpu`/WASM provider (would need more). Everything else is
implemented + verified. This is the single highest-value
in-environment union move and is now a precise, bounded wiring task.
