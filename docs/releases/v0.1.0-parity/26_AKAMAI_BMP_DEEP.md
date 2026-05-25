# 26 — Akamai Bot Manager Premier (BMP) deep dive

**Status:** planning
**Cluster:** homedepot (`Akamai-CHL 2754 ×4 profiles` — Camoufox also fails at `2638`) + bestbuy (SPA shell, cross-engine pass) + adidas (firefox-only-pass via massive 1.3 MB body; chrome/pixel/iphone stuck at the 2494-byte interstitial)
**Strategy:** mirror the structure of `07_DATADOME_PRIMITIVES.md` for Akamai. Restore the engine-internal *primitives* in public, and explicitly relegate the vendor encoders (sensor_data v2, sec-cpt PoW, TEA-CBC) to the private `vendor_solvers` companion crate where the post-`aecdf19` policy already puts them.

---

## TL;DR

Akamai Bot Manager Premier (BMP) is the third large cluster on our `02_GAP_ANALYSIS.md` open-set, after AWS WAF (chapter 06) and DataDome (chapter 07). Unlike DataDome — which surrendered to three small generic engine primitives in chapter 07 — Akamai BMP is fundamentally a **vendor-specific obfuscated-script self-solve + a vendor-specific sensor_data POST + a vendor-specific sec-cpt proof-of-work**. The engine cannot ship a generic "Akamai solver" without re-violating the `aecdf19` G6 strip policy.

What public CAN ship safely (and SHOULD for v0.1.0):

| # | Primitive | What it does | Where |
|---|---|---|---|
| A | `_abck` cookie state-machine recognition | Recognize `~-1~` (uncleared) vs `~3~` (verified) and feed into the existing solved-cookie-retry loop from chapter 07's Primitive 3 | `crates/browser/src/page.rs:1638-1663` (extend `started_as_*` flags) + extend `cookies_carry_anti_bot_clearance` from `07 §Primitive 3` |
| B | Persistent `started_as_seccpt_challenge` flag | ALREADY EXISTS at `page.rs:1649-1650`; the doc-20 anti-pattern fix from pre-strip `b623d5d` is still in HEAD. Confirm coverage with a post-restore measurement. | `crates/browser/src/page.rs:1649-1650` |
| C | Detection-only `_abck` / `bm_sz` / sensor_data marker logging | Per `18_ANTI_BOT_VENDOR_COOKBOOK.md §4.1`, add `x-akamai-transformed` / `_abck` cookie / `bm_sz` cookie observations to the vendor-detect logger | `crates/browser/src/page.rs:1054-1069` |

What MUST live in the private `vendor_solvers` crate (NOT public):

| # | Encoder | Pre-strip location (removed by `aecdf19`) |
|---|---|---|
| 1 | sensor_data v2 (TEA-CBC + 58-field tAD array + counter tuple + per-tenant integrity field) | `crates/akamai/src/payload.rs` (412 LOC) + `crates/akamai/src/crypto.rs` (591 LOC) |
| 2 | sensor_data v3 (JSON colon-delimited PRNG-shuffled with `bm_sz`-derived cookieHash key) | `crates/akamai/src/v3_payload.rs` (447 LOC) |
| 3 | sec-cpt rolling-hash PoW (SHA-256 brute-force preimage with `(((output<<8)\|b)&0xFFFFFFFF) % (difficulty+i)` reduction) | `crates/akamai/src/sec_cpt.rs` (243 LOC) |
| 4 | TEA-CBC primitive (Tiny Encryption Algorithm in CBC mode, the encryption layer Akamai chose for v2 envelopes) | `crates/akamai/src/tea_cbc.rs` (216 LOC) |
| 5 | `_abck` cookie state machine + per-host fileHash registry + tenant URL discovery | `crates/akamai/src/{session,lib}.rs` (838 + 307 LOC) |
| 6 | DRAIN_JS — the in-V8 collector that observes mouse/key/touch/scroll counters and exposes them to the Rust encoder | `crates/akamai/src/drain.rs` (204 LOC) |

The post-strip current state (HEAD as of commit `d00bcb2`, 2026-05-24): all six are GONE from the public tree. The engine still has:
- `started_as_seccpt_challenge` body-marker flag (`page.rs:1649-1650`) — pre-strip Increment 7 (`b623d5d`) preserved this as part of the FP-C2 / doc-20 hardening
- `_abck` / `bm_sz` body markers in `v8_html_is_real` (`page.rs:2288-2289`)
- `_abck` / `akam/13` / `pardon our interruption` classifier rows (`classify.rs:96, 103, 104`) with the `AKAMAI_CHALLENGE_COSIGNAL` co-signal gate (`classify.rs:127-134`)
- `__akamai_events` JS collector in `page.rs:1186-1196` — DEAD with respect to a now-empty solver set, but the JS surface still emits counters for any private solver to consume

After all three engine primitives (A/B/C) land **in public**, plus the vendor_solvers re-add (private), the expected delta:

| Site | BO routed best (HEAD) | Post-restoration |
|---|---|---|
| homedepot | `Akamai-CHL 2754` ×4 | `L3-RENDERED ≥ 50 KB` on iphone (per pre-strip `b623d5d`) |
| bestbuy | `L3-RENDERED 7833-7887` (SPA shell, "Choose a country" splash) | unchanged — this is a non-Akamai SPA branch; site detects mobile/desktop and serves a thin splash that loose-L3 already accepts |
| adidas | `L3-RENDERED 2494` ×3 (chrome/pixel/iphone, loose-L3 accepts at < 15 KB) + `L3-RENDERED 1.3 MB` on firefox | firefox-only-pass eliminated (the routed-best already wins); investigate why firefox uniquely flips the 2494 → 1.3 MB threshold (§4) |

Routed strict-pass delta: **+1 site** (108 → 109; homedepot flips on iphone). Combined with chapter 05 (reddit + duolingo, EASY wins), chapter 06 (AWS WAF), chapter 07 (DataDome), this puts BO routed at ≥ 113 — meeting the chapter 12 acceptance bar.

---

## 1. What `aecdf19` removed

The vendor-strip commit (2026-05-21) deleted **every** Akamai-specific source file from the public tree. See `git show --stat aecdf19` and `git show aecdf19 -- crates/akamai/`. The 9 deleted files, in functional order:

| File | LOC | Function |
|---|--:|---|
| `crates/akamai/src/lib.rs` | 838 | Crate root: `BotScoreVector` (`Server-Timing: ak_p` parser), `parse_bm_sz`, `MouseEvent`, `build_sensor_data` orchestrator |
| `crates/akamai/src/crypto.rs` | 591 | `build_v2_bestbuy` / `build_v2_dalphan` (v2 envelope encoders), `sha256_b64`, XOR substitution / shuffle primitives |
| `crates/akamai/src/datadome_crypto.rs` | 407 | DataDome's own encryption (cohabited the crate; moved to its own vendor module pre-strip) |
| `crates/akamai/src/v3_payload.rs` | 447 | v3 JSON cleartext schema (~30 keys) — the modern (post-2024) BMP envelope |
| `crates/akamai/src/payload.rs` | 412 | v2 cleartext field assembler — the 58-element `tAD` array (`-100, -105, -108, …`); 29 (marker, data) pairs |
| `crates/akamai/src/session.rs` | 307 | `AkamaiSession`, `AbckState`, `AkamaiSessionStore` (per-host state machine, mouse buffer, counters) |
| `crates/akamai/src/sec_cpt.rs` | 243 | sec-cpt PoW: rolling-hash reduction (`((output<<8)\|b)&0xFFFFFFFF) % (difficulty+i)`), answer search loop |
| `crates/akamai/src/tea_cbc.rs` | 216 | Tiny Encryption Algorithm in CBC mode |
| `crates/akamai/src/drain.rs` | 204 | `DRAIN_JS` — the in-V8 collector emitted by `Page::with_solvers(...)` setup |
| `crates/akamai/tests/dead_code_labels.rs` | 79 | gates ensuring deleted code stays deleted (the FP-Class-A dead-code labels) |

Plus the wiring in `crates/browser/src/page.rs`:
- `Page::handle_akamai_flow` (the inline orchestrator method)
- `crates/browser/src/solvers/akamai.rs` (137 LOC — the `AkamaiSolver` wrapper that implements the `ChallengeSolver` trait for the akamai crate's `build_sensor_data`)
- All references to `net::HttpClient::send_akamai_sensor_data`, `learn_abck`, `akamai_sessions` field

What was deliberately KEPT in public (the seam):
- `crate::challenge::ChallengeSolver` trait + `ChallengeKind` + `SolveOutcome` (`crates/browser/src/challenge.rs:55-161`)
- `Page::default_solvers()` returning empty `Arc<[]>` (`crates/browser/src/page.rs:850-852` — confirmed at HEAD)
- The body-marker classifier rows (`classify.rs:84, 96, 103-104, 127-134`)
- `started_as_seccpt_challenge` body-marker computation (`page.rs:1649-1650`)
- `__akamai_events` JS collector (`page.rs:1186-1196`) — dead with empty solvers, kept because the JS surface is profile-neutral (any private solver consumes it)
- `_abck` and `bm_sz` markers in `v8_html_is_real` (`page.rs:2288-2289`) — these prevent the V8-refetched body from being accepted as "real content" when it's secretly a re-served challenge document

The commit message is explicit:

> Per-vendor solver IMPLEMENTATIONS now live in the private companion `vendor_solvers` crate (~/projects/browser_oxide_internal). Embedders register them via Page::with_solvers(vendor_solvers::default_solvers()).

This chapter respects that boundary: §3.A/B/C is what goes in public; §6 is what must go in private.

---

## 2. Akamai BMP mechanism

### 2.1 Sub-product taxonomy

Akamai's bot-management product family is bigger than "BMP". Four sub-products appear in the wild, with overlapping cookies/markers:

| Sub-product | Tier | Cookie | Sensor URL | Recognized by |
|---|---|---|---|---|
| **BMP v2** (DalphanDev-class) | Premier | `_abck`, `bm_sz`, `bm_mi`, `bm_sv` | `/<obfuscated-path>` per-tenant | sensor_data POST body is colon-delimited 58-element TEA-CBC ciphertext; "version 3" first field |
| **BMP v3** (post-2024) | Premier | `_abck` (state-keyed by `bm_sz`-derived cookieHash) | `/<obfuscated-path>` per-tenant | sensor_data POST body is PRNG-shuffled JSON with `:` element-swapping per glizzy's reference |
| **sec-cpt** (Strict-Response variant) | Premier+ | `sec_cpt=<sec>~<state>~…` (state `3` = solved) | `/_sec/cp_challenge/verify` | HTTP 428 with `{token, timestamp, nonce, difficulty, count, timeout, cpu, verify_url}` JSON body |
| **BM Edge** | Standard | `bm_sz` only | none | server-side scoring; no JS challenge |
| **Akamai 1.7X** (legacy) | Standard | `ak_bmsc` | `/akam/13/...` | pre-BMP; some smaller sites still on this |

The 126-site corpus hits **BMP v2/v3** (homedepot, bestbuy, adidas, walmart, nike-class, macys, hotels.com, h-m-class) and **sec-cpt** (homedepot under the high-tier rule).

Cross-reference: `18_ANTI_BOT_VENDOR_COOKBOOK.md §2.3` summarizes the same four sub-products in one paragraph; this chapter is the deep-dive equivalent of chapter 07 for DataDome.

### 2.2 Cookie state machines

The `_abck` cookie is the **score-bearing** signal — its value encodes Akamai's trust verdict on the session:

```
_abck = <random-prefix>~<state>~<infix>~<suffix>...
```

States observed in the wild (per `crates/akamai/src/session.rs` pre-strip and `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/state_2026_05_16_phase5_datadome.md`):

| Infix pattern | State name | Meaning | Engine action |
|---|---|---|---|
| `~-1~-1~-1~` | **Favorable** | Sensor accepted, session trusted | Stop; no retry needed |
| `~0~-1~-1~` | **Untrusted** | First set on initial response; sensor needed | POST sensor_data |
| `~0~0~…` | **Provisional** | Sensor received, still scoring | Retry / wait |
| `~3~…` | **Rejected** | Sensor scored as bot | Re-roll session / rotate fingerprint |
| `~ -1 ~ 0 ~ -1 ~` | **Stop-signal threshold** | Slot 1 = number of sensor POSTs before stopping (per the `24c19e3` correction); NOT a trust toggle | Honor as a cap |

Important correction from `24c19e3` (pre-strip): _abck slot 1 is **NOT** a trust toggle. It's a *stop-signal threshold* — the count of sensor POSTs Akamai will accept before refusing. Earlier code treated slot 1 = 0 as "we're good"; that was wrong. The actual trust signal is the absence of `-1~-1` at the end.

The `sec_cpt` cookie is similar:

```
sec_cpt = <sec>~<state>~<timestamp>~<nonce>
```

Where `<state>` is:
- `0` — challenge issued, not yet solved
- `3` — bundle's PoW completed + `chlg_duration` enforced wait satisfied + verify_url POST returned 200

The `~3~` transition is what `crates/akamai/src/sec_cpt.rs:sec_cpt_solved` (pre-strip) checked.

### 2.3 Flow A — BMP v2/v3 sensor_data path

Canonical sequence (verified against bestbuy / homedepot / adidas captures pre-strip):

1. **Initial GET** → Akamai edge returns 200 with HTML body. The body contains `<script src="https://www.example.com/akam/13/<obfuscated-path>">` (the per-tenant bootstrap, often re-routed through the origin to hide the Akamai dependency).
2. **Bootstrap script loads** (~80-150 KB minified, daily-rotating obfuscation). Internally it builds the 58-element `tAD` array (v2) or 30-key JSON object (v3) with:
   - UA + gecko + lang + screen + tz + flags (field `-100` / vAD)
   - Event listeners installed: `do_en, dm_en, t_en` (field `-101` / wAD)
   - Mouse trajectory `i,1,t,x,y;…` accumulated for ~200-500 ms (field `-110` / zdD)
   - Page-URL (field `-112` / cAD)
   - Canvas fingerprint hex + WebGL renderer hex (field `-129` / LRD)
   - **OfflineAudioContext rendered samples** — the DynamicsCompressor + Oscillator output (the largest single hash input per `memory/tier1_priority_for_akamai.md`)
   - ~30 more fields (counters, navigator properties, plugins, timezone, hardwareConcurrency, performance.memory)
3. **TEA-CBC encrypt** the cleartext with a per-script key (v2) or **PRNG-shuffle-then-substitute** with `bm_sz`-derived seeds (v3). Wrap as:
   ```
   POST_BODY = "3;0;1;0;<counter>;<sha256-b64-integrity>;<counter-tuple>;<scrambled-body>"
   ```
4. **Sensor POST** to `https://www.example.com/<obfuscated-path>` (different from the bootstrap URL). Body `{"sensor_data": "<encrypted>"}`.
5. **Server validates** the integrity SHA + decryption-shape + per-field score. On success:
   - Sets `_abck` to a value WITHOUT the `~-1~-1` suffix
   - Sometimes refreshes `bm_sz`
6. **Subsequent GET** carries the upgraded `_abck` → Akamai gate accepts.

Inter-POST cadence: Akamai expects **2 sensor POSTs** for the trust upgrade (per glizzy's deobfuscation). The first POST has counters `"16,0,0,0,0,0"` (only key counter populated from page-load events). The second POST after user activity: `"5,18,0,0,1,323"` (5 keys, 18 mouse, 1 scroll, 323 accel). The pre-strip code only sent the first POST (single-POST mode at the upper edge of the variance band per `51d2abd`).

### 2.4 Flow B — sec-cpt PoW path (homedepot)

Triggered by the "Strict Response" rule (high-tier protection). When Akamai's risk engine thinks the request is high-risk it returns HTTP 428 with this JSON body:

```json
{
  "token":      "AAQAAAAJ...",          // ~430-char base64 token
  "timestamp":  1713283747,
  "nonce":      "ebccdb479fcb92636fbc", // 20-hex-char
  "difficulty": 15000,                  // PoW target
  "count":      1,
  "timeout":    1000,                   // ms per answer attempt
  "cpu":        false,
  "verify_url": "/_sec/cp_challenge/verify"
}
```

The **challenge bundle** (a ~560 KB + ~425 KB pair of obfuscated JS files, served by URL like `/Wjv3<rest>` or `/i4ENwVhj7<rest>` — daily-rotating) is loaded as `<script src=…>`. The bundle:

1. Parses the JSON above.
2. For each of `count` answers, brute-forces a base-16 float `r = "0.<hexdigits>"` such that the rolling-hash reduction below equals 0:
   ```
   input  = sec + str(timestamp) + nonce + str(difficulty + i)
   h      = sha256(input + r)
   output = 0
   for b in h:
       output = ((output << 8) | b) & 0xFFFFFFFF
       output = output % (difficulty + i)
   return r if output == 0 else retry
   ```
3. Waits the `chlg_duration` (5-30 s enforced wait — Akamai's anti-replay rate-limit).
4. POSTs the answer(s) to `verify_url`.
5. Server sets `sec_cpt=<sec>~3~…` AND clears the failing `_abck`.
6. Bundle calls `window.location.reload(true)` → second GET succeeds.

The bundle **self-solves in our V8** — verified pre-strip on homedepot once Increment 7 (`b623d5d`) suppressed the wrong BMP `sensor_data` POST that was racing it. The PoW cost at difficulty=15000 is ~5 ms per answer on one CPU core; the real time floor is the `chlg_duration` enforced wait.

Engine implication: **public code does NOT need a sec-cpt PoW solver in Rust** as long as the bundle is allowed to run to completion. The pre-strip `crates/akamai/src/sec_cpt.rs` `solve_crypto` function had ZERO non-test callers — it was a fallback that was never needed. The §6 plan does NOT re-add it; the engine just needs the `started_as_seccpt_challenge` flag (already present at `page.rs:1649-1650`) to keep the cookie-delta retry loop alive until the bundle's reload lands.

### 2.5 Flow C — _abck-only (BM Edge)

The lightest tier. Akamai sets `bm_sz` on the first response and scores subsequent requests purely on `bm_sz` consistency + IP + UA. No JS challenge. The engine wins automatically as long as `bm_sz` is preserved cookie-jar-wide (it is — `crates/net/src/lib.rs` HttpClient cookie store handles this).

---

## 3. Restore-as-primitives plan

Mirror the chapter 07 §Primitive structure. Three primitives go in public; the vendor encoders stay in `vendor_solvers`.

### 3.A Primitive A — _abck cookie state-machine recognition (public)

**What and why**

Chapter 07 §Primitive 3 introduced `cookies_carry_anti_bot_clearance` — a generic closed-list cookie name matcher. The closed-list in 07's spec already includes `_abck=` and `bm_sz=`:

```rust
const PATTERNS: &[&str] = &[
    "datadome=",
    "_abck=",      // ← Akamai BMP
    "bm_sz=",      // ← Akamai BM Edge
    "cf_clearance=",
    "kpsdk=",
    "sec_cpt=",    // ← Akamai sec-cpt
    "aws-waf-token=",
];
```

What 07 left to this chapter is the **state machine** — distinguishing `_abck=…~-1~` (uncleared, do retry) from `_abck=…~3~…` (verified, stop). The bare cookie-name match in 07 will trigger the cookie-delta retry on EVERY cookie change, which for Akamai means every sensor POST response — even the failing ones — because Akamai always re-sets `_abck` regardless of trust verdict.

**Where to insert**

`crates/browser/src/classify.rs` (new section near `is_cf_challenge_doc`):

```rust
/// Inspect an `_abck` cookie value and report Akamai's trust verdict.
/// Pure parser; closed enum; no I/O. Maps to the per-host state machine
/// pre-strip `AbckState` enum represented (see git show aecdf19 --
/// crates/akamai/src/session.rs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbckTrust {
    /// `~-1~-1~-1~` — favorable (sensor accepted, trusted).
    Favorable,
    /// `~0~-1~-1~` — untrusted (initial state; sensor required).
    Untrusted,
    /// `~0~0~…` — provisional (sensor received, scoring).
    Provisional,
    /// `~3~…` — rejected (scored as bot; re-roll required).
    Rejected,
    /// Anything else — unknown / not an `_abck` value.
    Unknown,
}

pub fn parse_abck_trust(cookie_value: &str) -> AbckTrust {
    // _abck = <prefix>~<state>~<infix>~<suffix>
    let segments: Vec<&str> = cookie_value.split('~').collect();
    let last_three = segments
        .iter()
        .rev()
        .take(3)
        .copied()
        .collect::<Vec<_>>();
    // last_three is reversed (suffix, infix, state) → reverse again for clarity
    match last_three.as_slice() {
        ["-1", "-1", "-1", ..] => AbckTrust::Favorable,
        ["-1", "-1", "0", ..] => AbckTrust::Untrusted,
        // Provisional: state=0, infix=0
        [_, "0", "0", ..] => AbckTrust::Provisional,
        // Rejected: state begins with "3"
        [_, _, s, ..] if s.starts_with('3') => AbckTrust::Rejected,
        _ => AbckTrust::Unknown,
    }
}
```

Then in `page.rs:2125-2185` (the cookie-delta retry block from chapter 07 §Primitive 3), guard the retry by *trust state*, not just cookie presence:

```rust
// Pre-chapter-07: every cookie change retries.
// Chapter 07 §P3: only when cookies_carry_anti_bot_clearance(now) && !engine_classify(body).verdict.is_challenge()
// Chapter 26 §3.A: additionally short-circuit when _abck is already Favorable.

let abck_trust = cookies_after.split(';')
    .find_map(|c| {
        let c = c.trim();
        c.strip_prefix("_abck=").map(crate::classify::parse_abck_trust)
    })
    .unwrap_or(AbckTrust::Unknown);

let should_retry = (cookies_after != cookies_before
    && !cookies_after.is_empty()
    && abck_trust != AbckTrust::Favorable)  // ← stop loop once Favorable
    || (last_accept_ch_upgrade && !accept_ch_retry_done);
```

**Naming discipline** (per CLAUDE.md vendor-naming rules)

- Function `parse_abck_trust` names a public cookie spec, not the vendor product family — `_abck` is a documented cookie name in `18_ANTI_BOT_VENDOR_COOKBOOK.md §1.4`.
- Enum `AbckTrust` is parametric over the cookie value; no flow code is gated on the vendor product.
- This is structurally identical to how chapter 07 lets `cookies_carry_anti_bot_clearance` enumerate cookie names without naming the vendor in flow code. The classifier already names vendors *as classification keys*, and the classifier is the agreed-canonical place for that.

**Verification**

Unit test in `classify.rs::tests`:

```rust
#[test]
fn abck_trust_recognises_canonical_states() {
    assert_eq!(parse_abck_trust("ABCDEF~-1~-1~-1~xyz"), AbckTrust::Favorable);
    assert_eq!(parse_abck_trust("ABCDEF~0~-1~-1~xyz"), AbckTrust::Untrusted);
    assert_eq!(parse_abck_trust("ABCDEF~10~0~0~xyz"), AbckTrust::Provisional);
    assert_eq!(parse_abck_trust("ABCDEF~3~7~9~xyz"), AbckTrust::Rejected);
    assert_eq!(parse_abck_trust(""), AbckTrust::Unknown);
}
```

Plus the integration sweep: homedepot's per-iter cookie trace MUST show `_abck` transitioning from `Untrusted` (iter 0, sec-cpt 428) → `Favorable` (iter 1, after bundle's reload).

### 3.B Primitive B — Persistent `started_as_seccpt_challenge` flag (ALREADY EXISTS)

**Status:** already shipped at `crates/browser/src/page.rs:1649-1650`:

```rust
let started_as_seccpt_challenge =
    html.contains("sec-if-cpt-container") || html.contains("sec-cpt-if");
```

This is the persistent origin-flag (NOT solver-dependent — works with empty `Page::default_solvers()`) that keeps the poll-loop and cookie-delta retry alive after the sec-cpt bundle mutates the DOM. The doc-20 mutable-state-guard fix from pre-strip Increment 7 (`b623d5d`) is preserved in HEAD. Verified by grepping HEAD: `page.rs:1970, 2107, 2110, 2136, 2159` all consume the flag.

**Action for v0.1.0:** confirm it still works post-strip. Pre-strip measurement (per `memory/state_2026_05_16_phase5_datadome.md`):

> homedepot `Akamai-sec-cpt-CHL`→`L3-RENDERED len=2507`. sec-cpt bundles 560 KB+425 KB now fetch 200 OK and self-solve as sole actor.

Post-strip risk: the wrong-BMP-POST suppression branch at `page.rs:2110` checks `if s.name() == "akamai-bmp" && started_as_seccpt_challenge` — with **no solvers registered**, `s.name() == "akamai-bmp"` is never true, so the branch is dead (cannot misfire). The `started_as_seccpt_challenge` flag is therefore safe to KEEP gating the cookie-delta-retry / poll-loop, but it gates ONLY the engine-side retry now (no solver to misfire against). 

**Measurement to run** (post-restoration, network-required):
```bash
cat > /tmp/akamai_corpus.json <<'JSON'
[
  {"cat":"stores","name":"homedepot","url":"https://www.homedepot.com/"},
  {"cat":"stores","name":"bestbuy","url":"https://www.bestbuy.com/"},
  {"cat":"chl-known","name":"adidas","url":"https://www.adidas.com/us"}
]
JSON
target/release/examples/sweep_metrics iphone_15_pro_safari_18 \
    /tmp/akamai_corpus.json /tmp/akamai_iphone.json
```

Expected after Primitive A lands (no vendor_solvers): homedepot stays `Akamai-CHL 2754` because the sec-cpt bundle never fetches (per §4 the engine's iframe / script-load chain on the bundle URL is the suspect failure point post-strip; bisect against pre-strip `b623d5d`).

### 3.C Primitive C — Detection-only marker logging (public)

Per `18_ANTI_BOT_VENDOR_COOKBOOK.md §4.1`, extend the vendor-detect logger at `page.rs:1054-1069` to cover Akamai's documented response markers:

```rust
if let Some(v) = resp.headers.get("x-akamai-transformed") {
    eprintln!("[vendor-detect] akamai-edge {} on {}", v, resp.url);
}
if let Some(v) = resp.headers.get("server-timing") {
    if v.contains("ak_p") {
        eprintln!("[vendor-detect] akamai-bmp ak_p {} on {}", v, resp.url);
    }
}
// _abck / bm_sz set-cookie detection
for (k, val) in &resp.headers {
    if k.eq_ignore_ascii_case("set-cookie") {
        for tag in &["_abck=", "bm_sz=", "sec_cpt=", "ak_bmsc="] {
            if val.contains(tag) {
                eprintln!("[vendor-detect] akamai-cookie {} on {}", tag.trim_end_matches('='), resp.url);
            }
        }
    }
}
```

This is **detection-only**. No flow change. Pure observation that lets post-run analysis split CHL outcomes by Akamai sub-product. Equivalent to the AWS WAF / DataDome / wbaas observation lines that are already in the chain.

The `Server-Timing: ak_p` header carries the **BotScoreVector** (per pre-strip `crates/akamai/src/lib.rs:BotScoreVector`). It's a free regression oracle Akamai puts on every response from a BMP-protected origin:

```
desc="<request_id>_<timestamp>_<score_a>_<score_b>_<score_c>_<score_d>_<score_e>_<score_f>-"
```

Lower scores → more human. A jump in any sub-score across runs is a regression signal that pinpoints which engine fingerprint we just broke. Pre-strip the parser was DEAD (`FP-Class-A` label — zero non-test callers); the v0.1.0 work should at minimum **log** the raw header so post-sweep diff can correlate.

### 3.D Naming discipline summary

Following the chapter 07 model:

| Type | Naming rule | Public engine name | Vendor product name (use ONLY in classifier + this doc) |
|---|---|---|---|
| Body marker | uses the cookie spec or URL substring as the *classification key* | `_abck`, `bm_sz`, `sec_cpt`, `akam/13`, `/_sec/cp_challenge` | Akamai BMP, Akamai sec-cpt, Akamai BM Edge |
| Engine flow gate | uses the closed-list cookie name or response-shape predicate | `cookies_carry_anti_bot_clearance`, `is_challenge_document_response`, `started_as_challenge_doc` | — |
| Cookie state | uses the cookie name; the enum is parametric over the cookie spec | `parse_abck_trust`, `AbckTrust::{Favorable, Untrusted, Provisional, Rejected}` | — |
| Solver impl | lives in `vendor_solvers`; instantiated via `Page::with_solvers(...)` | The `ChallengeSolver` trait method names: `name()`, `detect()`, `solve()`, `solved_signal()`, `relax_response_csp()` | `AkamaiSolver`, `name() == "akamai-bmp"` |

The boundary: public code uses cookie/URL specifications (which are documented in IETF / vendor public docs); vendor product names appear only in the closed-enum classifier tables (`classify.rs:81-156`) — never as flow-control gates.

---

## 4. Per-profile Akamai BMP behavior

### 4.1 The adidas firefox-only-win mystery

**Measurement** (`/tmp/full_sweep_2026_05_24/`):

| Profile | adidas tag | len | ms |
|---|---|--:|--:|
| chrome_148_macos | L3-RENDERED | 2494 | 115298 |
| pixel_9_pro_chrome_148 | L3-RENDERED | 2494 | 115286 |
| iphone_15_pro_safari_18 | L3-RENDERED | 2494 | 116006 |
| **firefox_135_macos** | **L3-RENDERED** | **1314086** | **25338** |
| camoufox | L3-RENDERED | 2384 | 4655 |
| patchright | L3-RENDERED | 10678 | 2931 |
| playwright | L3-RENDERED | 10679 | 3048 |
| playwright_stealth | L3-RENDERED | 10678 | 3607 |

Three observations:

1. **All four BO profiles "pass" loose-L3** (`tag == L3-RENDERED`) because the classifier's `_abck` co-signal gate (`classify.rs:103-104, 127-134`) requires `akam/13` + `(sensor_data|bm-verify|sec-if-cpt-container|sec-cpt-if|/_sec/cp_challenge|pardon our interruption)`. The 2494-byte interstitial probably lacks one of the co-signals → falls through to `L3-RENDERED` despite being a real challenge stub. This is the `measurement_holistic_chl_fp_trap.md` size-gate pattern: ≥30 KB = rendered FP; below = potentially a real stub mis-classified as PASS.
2. **Strict pass (len ≥ 15 KB) is firefox-only.** Per `11_PER_PROFILE_STRATEGY.md §3.2`, adidas is one of the 4 routing-required (1/4) sites. firefox uniquely flips the 2494 → 1.3 MB body threshold; the other 3 BO profiles stay stuck.
3. **Even Camoufox gets only 2384 bytes** (a 2-byte variant of the same interstitial). The 4 Playwright-family engines land 10678/10679 bytes (an Akamai *try-again* page, larger but still not the homepage). **Firefox-on-our-stack is the unique flip across all 8 engines tested.**

**Hypothesis** (sorted by prior):

a) **TLS class.** BO's `firefox_135_macos` profile labels itself `firefox_135` for the `tls_impersonate` codename but currently ships Chrome-class TLS bytes (per `11_PER_PROFILE_STRATEGY.md §7.6` table — "currently same as `chrome_147` desktop (real Gecko TLS deferred)"). So the wire-level TLS handshake matches Chrome. Yet the UA is Firefox. **adidas's Akamai tenant evidently treats `UA=Firefox + TLS=Chrome` as a benign edge case** — perhaps because real Firefox-on-Mac is rare enough that Akamai's risk model doesn't have a confident-bot anchor for it. Pixel + iphone are mobile classes Akamai over-indexes on; chrome is the most-fingerprinted desktop class.

b) **`navigator.vendor=""` + `productSub=20100101`.** Per `presets.rs:413-495`, firefox-macos uniquely sets `vendor=""` and `productSub="20100101"`. The Akamai sensor VM definitely reads these fields (per `tier1_priority_for_akamai.md` instrumentation: `navigator.userAgentData ×2`). If Akamai's risk-scoring weights "navigator.vendor == 'Google Inc.' but TLS == Chrome" as the most-common bot pattern (because every Chrome-impersonating headless writes `vendor=Google Inc.`), then firefox-with-`vendor=""` dodges that specific scoring rule.

c) **No `sec-ch-ua-*` headers at all.** Firefox doesn't speak UA-CH. Akamai's risk model on `UA=Firefox` correctly expects ZERO `sec-ch-ua-*` headers and gets ZERO. Chrome/Pixel/iPhone profiles all emit some `sec-ch-ua-*` (Pixel = `mobile + Android`; iPhone = none, but the rest of the iOS surface is its own risk class). The combination on firefox is internally consistent in a way the others aren't.

d) **Mozilla-masked WebGL.** firefox_135_macos uniquely sets `webgl_vendor=Mozilla` + `webgl_renderer=Mozilla` (per `presets.rs:413-495` + `firefox_webgl_is_masked` test at `presets.rs:954`). Real Firefox 113+ masks WebGL by default. Akamai's sensor VM reads canvas FP + WebGL — see field `-129 / LRD` in pre-strip `payload.rs`. **A `Mozilla/Mozilla` masked-WebGL fingerprint is well-known-real-Firefox**; faking it on a Chrome-class engine would be inconsistent. We have actual masked-WebGL because our preset rewrites the WebGL `getParameter` returns.

**First debug step** (per `04_TOOLING_SPEC.md`):

```bash
target/release/examples/sweep_metrics chrome_148_macos /tmp/adidas_only.json /tmp/out_chrome.json --capture adidas
target/release/examples/sweep_metrics firefox_135_macos /tmp/adidas_only.json /tmp/out_firefox.json --capture adidas
# Diff the captured fetches.json — does the firefox run fetch the BMP bootstrap (`/akam/13/...`) and the sensor POST URL?
diff /tmp/capture/bo/chrome_148_macos/adidas/fetches.json /tmp/capture/bo/firefox_135_macos/adidas/fetches.json | head -200
# Diff the cookie writes — does the firefox run get a clean _abck?
diff /tmp/capture/bo/chrome_148_macos/adidas/cookie_writes.json /tmp/capture/bo/firefox_135_macos/adidas/cookie_writes.json
```

Expected outcome: firefox fetches the BMP bootstrap, runs it to completion, and the sensor POST returns `_abck=…~-1~-1~-1~` (Favorable). The other 3 profiles either don't reach the sensor POST or get `_abck=…~0~-1~-1~` (Untrusted). The diff identifies which fingerprint field differs.

Once identified, the question is: do we **promote** the firefox-only-passing fingerprint field into the chrome/pixel/iphone presets (would defeat Akamai's bot-pattern correlation but risk introducing new inconsistencies elsewhere), or leave it as a per-profile routing key (per `11_PER_PROFILE_STRATEGY.md §4.1 rule 1` — "Akamai class → try firefox"). For v0.1.0 the latter is the cheap answer; the former is post-v0.1.0 research.

### 4.2 homedepot failure across all profiles

**Measurement**:

| Profile | homedepot tag | len | ms |
|---|---|--:|--:|
| chrome_148_macos | Akamai-CHL | 2754 | 135207 |
| pixel_9_pro_chrome_148 | Akamai-CHL | 2754 | 135491 |
| iphone_15_pro_safari_18 | Akamai-CHL | 2754 | 135654 |
| firefox_135_macos | Akamai-CHL | 2754 | 135245 |
| camoufox | Akamai-CHL | 2638 | 4611 |
| patchright | L3-RENDERED | 1245838 | 6047 |
| playwright | L3-RENDERED | 1077510 | 6352 |
| playwright_stealth | L3-RENDERED | 1076915 | 6338 |

Two surprises:

1. **All BO profiles AND Camoufox fail** — homedepot is currently in the "hard residual" set per `02_GAP_ANALYSIS.md`. Camoufox is at 2638 bytes (the sec-cpt interstitial); BO is at 2754 (a 116-byte variant).
2. **Playwright family PASSES** at 1+ MB body. This is the inverted-default scenario: anti-bot vendors that punish CDP-driver detection (chapter 12 §3.5 lists Amazon's AWS WAF) here do the opposite. Akamai's homedepot tenant evidently TRUSTS real Chrome enough that even a CDP-detected Playwright passes — and PUNISHES anything that isn't perfectly real Chrome.

The pre-strip iphone-profile pass on homedepot (per `memory/state_2026_05_16_phase5_datadome.md` Increment 7 / commit `b623d5d`) flipped via:

> doc-20 anti-pattern fix: `handle_akamai_flow`'s sec-cpt guard keyed off `self.content()` (post-bundle-mutation DOM); after the sec-cpt bundle mutates the DOM the guard misses → engine fires the WRONG BMP `sensor_data` POST → `_abck=…~-1~` 201-loops AND (doc 20) blocks the bundle's own self-solve. **Inc 7 suppresses BMP for any nav that STARTED sec-cpt (persistent `started_as_seccpt_challenge`).**

That `started_as_seccpt_challenge` flag is still present in HEAD (`page.rs:1649-1650`). But the wrong-POST suppression branch (`page.rs:2110`) was guarded on `s.name() == "akamai-bmp"` — with no solvers registered (post-strip), the branch is dead but ALSO the BMP POST that the branch was suppressing is dead. So the pre-strip behaviour should have survived as a no-op intersection.

Then why does HEAD still fail homedepot? The remaining hypothesis (verified-needed):

The pre-strip sec-cpt pass relied on the bundle being ALLOWED to fetch + run. The post-strip removal of `crates/browser/src/solvers/akamai.rs` removed `AkamaiSolver::relax_response_csp` which voted `true` on sec-cpt bodies — meaning the bundle's `<script src="/Wjv3…">` is now likely blocked by the origin's CSP. Same shape as DataDome Primitive 1 — the bundle can't load → can't self-solve → `Akamai-CHL 2754` stays.

**Acceptance step:** if chapter 07 §Primitive 1 (engine-side `is_challenge_document_response` + CSP relax) ships, then on a small Akamai sec-cpt body the CSP is also relaxed (sec-cpt body is `< INTERSTITIAL_MAX_BYTES` and `/_sec/cp_challenge` is a classifier UNAMBIGUOUS row — see `classify.rs:84`). The bundle would then be allowed to fetch + run. Then `started_as_seccpt_challenge` keeps the cookie-delta retry alive until the bundle's reload lands. Expected delta: homedepot flips on iphone (consistent with pre-strip `b623d5d` measurement).

If after chapter 07 Primitive 1 + chapter 26 Primitive A homedepot is STILL stuck, the diagnostic per `04_TOOLING_SPEC.md`:

```bash
target/release/examples/sweep_metrics iphone_15_pro_safari_18 /tmp/homedepot.json /tmp/out.json --capture homedepot
# Check fetches.json — does the sec-cpt bundle fetch?
jq '.[] | select(.url | contains("Wjv3") or contains("/_sec/") or contains("sec_cpt"))' /tmp/capture/bo/iphone_15_pro_safari_18/homedepot/fetches.json
# Check script_errors.json — does the bundle throw?
cat /tmp/capture/bo/iphone_15_pro_safari_18/homedepot/script_errors.json
```

### 4.3 bestbuy — non-Akamai-vulnerability cluster

**Measurement**:

| Profile | bestbuy tag | len | ms |
|---|---|--:|--:|
| chrome_148_macos | L3-RENDERED | 7887 | 25185 |
| pixel_9_pro_chrome_148 | L3-RENDERED | 7833 | 25243 |
| iphone_15_pro_safari_18 | L3-RENDERED | 7833 | 25356 |
| firefox_135_macos | L3-RENDERED | 7887 | 25144 |
| camoufox | L3-RENDERED | 7465 | 4893 |
| patchright | L3-RENDERED | 7103 | 20763 |
| playwright | L3-RENDERED | 7340 | 22657 |
| playwright_stealth | L3-RENDERED | 7340 | 21705 |

**Every engine** lands at ~7-8 KB — that's the "Choose a country" i18n splash (per `classify.rs:120-126` comment that documents this exact case as the reason for `AKAMAI_CHALLENGE_COSIGNAL`). Bestbuy serves this splash to first-time visitors regardless of country/IP/headers. It's NOT an Akamai block; it's a benign content-routing splash.

The splash is below the strict-pass threshold (15 KB). The loose-L3 classifier accepts it because no `_abck` or other Akamai marker is present in the body — `classify.rs:103-104` correctly skips it (akam/13-only without co-signal). So **bestbuy is not in the chapter 26 critical path**; it's a chapter 11 / `02_GAP_ANALYSIS.md` hard-residual entry, not a chapter 26 entry. Including for completeness because §2.3 of doc 18 lists it as an Akamai site.

To actually advance past the splash → real homepage requires either a country-select form submit (a UX flow, not anti-bot) or the bestbuy mobile-app deep-link that bypasses the splash. Out of scope for v0.1.0.

---

## 5. Public research on Akamai BMP

Anchoring on `18_ANTI_BOT_VENDOR_COOKBOOK.md §2.3` and §6.2.

### 5.1 Open-source projects (reverse / solvers)

| URL | What's there | Reusable? |
|---|---|---|
| https://github.com/xiaoweigege/akamai2.0-sensor_data | sensor_data + telemetry + sbsd + bm_s generator, claims 100% pass at 100 concurrency (per readme). The DalphanDev-class v2 reference. **License: not stated** — research-reference only per CLAUDE.md ("DO NOT copy their code into public crates"). | Protocol description: yes. Code: no. |
| https://github.com/Edioff/akamai-analysis | Signal taxonomy — which navigator/canvas/audio/font fields go into the sensor body, sorted by hash weight. | Yes — informational. |
| https://github.com/i7solar/Akamai | Go-based cookie generator for legacy Akamai 1.7X sites. Smaller scope (no BMP). | Yes for 1.7X mechanism; out of scope for BMP. |
| https://github.com/cirleamihai/akamai-1.7-cookie-generator | Python `requests`-based v1.7 generator. | Same — 1.7X only. |
| https://github.com/Hyper-Solutions/hyper-sdk-js | Commercial SDK covering Akamai BMP alongside Incapsula/Kasada/DataDome. Free demo, paid for prod use. | Protocol docs (their `docs.hypersolutions.co` site) are extensive and accurate. |
| https://pkg.go.dev/github.com/FRIS-Solutions-Vault/akamai-sdk-go | Go-based Akamai SDK — well-typed sensor_data envelope. | Yes — informational reference for field types. |
| https://github.com/Bobby-coder/Akamai-Cookie-Generator | Older Akamai cookie generator. Mostly 1.7X. | Lower priority. |

### 5.2 Walkthroughs and research papers

| URL | What's there |
|---|---|
| [glizzykingdreko / "Akamai v3 Sensor Data: Deep Dive into Encryption, Decryption, and Bypass Tools"](https://medium.com/@glizzykingdreko/akamai-v3-sensor-data-deep-dive-into-encryption-decryption-and-bypass-tools-da0adad2a784) | The canonical v3 deobfuscation walkthrough; covers `bm_sz`-derived cookieHash, `elementSwapping(':')`, alphabet-membership substitution. **The pre-strip v3 envelope encoder was a port of this reference**, per commits `f88b92e` / `e14c1ba` / `c53fa56`. |
| [Scrapfly: Akamai bypass](https://scrapfly.io/bypass/akamai) | Commercial bypass guide; high-level mechanism overview + their CAPTCHA-solving-service offering. |
| [ZenRows: Akamai bypass 2026](https://www.zenrows.com/blog/bypass-akamai) | Update of the same; lists 5 sub-products. |
| [`docs.hypersolutions.co`](https://docs.hypersolutions.co) | Commercial SDK's protocol docs — the most accurate public reference for BMP v3 + sec-cpt + `_abck` state machine. |
| [Akamai's own "Bot Manager Premier" product page](https://www.akamai.com/products/bot-manager) | Vendor pitch; no protocol details. |

### 5.3 Internal capture data (BO repo)

Pre-strip captures from `ab_harness/` and `tests/`:

- `crates/akamai/tests/` (deleted by `aecdf19`) — once contained `bestbuy_v2_byte_perfect.rs`, `dead_code_labels.rs`, `homedepot_filehash_capture.rs`, `homedepot_sec_cpt_probe.rs`. The byte-perfect parity test against glizzy's reference (`2efb307`, `f88b92e`) showed our v3 envelope was identical to the reference implementation.
- `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/tier1_priority_for_akamai.md` — 2026-04-10 instrumentation of the adidas sensor VM showing exactly which navigator/canvas/audio APIs are read (the 25-call Function.prototype.toString count, the zero canvas pixel extraction count, the 11 paint ops, etc.). Re-read this before doing any adidas work.

### 5.4 What's documented vs reusable

| Topic | Documented in public? | Reusable in public engine? |
|---|---|---|
| `_abck` cookie state machine (state values, what `~3~` means) | Yes — `docs.hypersolutions.co` + `18 §1.4` | Yes — §3.A `parse_abck_trust` |
| sensor_data v2 envelope shape ("3;0;1;0;count;sha256;tuple;body") | Yes — glizzy + xiaoweigege + Edioff | No — vendor solver only |
| TEA-CBC encryption | Yes — the algorithm is from 1994 academia | Algorithm yes; *use* for vendor bypass no |
| sec-cpt PoW algorithm (rolling-hash reduction) | Yes — hyper-sdk-go source + `crates/akamai/src/sec_cpt.rs` pre-strip docstring | No — vendor solver only; ALSO the bundle self-solves in V8 (per Phase 5) so the PoW solver is rarely needed anyway |
| `bm_sz` cookie format | Yes — glizzy | Yes — parse-only, as part of the cookie state machine |
| Per-tenant file hash registry (which tenant uses which obfuscated bundle URL) | Pre-strip code had a small registry (`crates/akamai/src/lib.rs:per_host_filehash_registry`); xiaoweigege has a larger one | No — vendor solver only |

The boundary is consistent with chapter 07: **public knows the cookie + URL shape; private knows the encryption + the per-tenant secrets**.

---

## 6. Restoration sequence

Phased over post-v0.1.0 with strict gating per `14_TESTING_VALIDATION.md` and `03_BENCHMARK_METHODOLOGY.md`. Order matters: each step is gate-green and shippable on its own.

### Step 1 — Land Primitive A (public, low-risk)

`crates/browser/src/classify.rs`: add `AbckTrust` enum + `parse_abck_trust`. Wire into chapter 07 §Primitive 3's cookie-delta retry as the "stop loop when Favorable" guard.

- Unit tests in `classify.rs::tests::abck_trust_*`
- 126-site holistic sweep must not regress (zero new BLOCKED, zero new THIN-BODY, max ±5 sites per noise-floor)
- Expected delta: **0 sites** in isolation. Primitive A is a *correctness* preservation, not a new pass. The value comes from preventing the cookie-delta retry from looping on failing sensor POSTs once a private solver IS registered.

### Step 2 — Land Primitive C (public, zero-risk)

`crates/browser/src/page.rs:1054-1069`: extend the vendor-detect logger with `x-akamai-transformed` / `Server-Timing: ak_p` / `_abck` / `bm_sz` / `sec_cpt` / `ak_bmsc` set-cookie observations.

- No flow change; pure logging.
- Expected delta: **0 sites**. Value is post-sweep telemetry.

### Step 3 — Verify Primitive B is still wired (public, smoke check)

`page.rs:1649-1650` (the persistent `started_as_seccpt_challenge` flag) is already present at HEAD. Confirm by:

- Running the 3-site Akamai corpus on iphone_15_pro_safari_18 and capturing `iter_summary.json`.
- The capture should show the poll-loop and cookie-delta retry block firing on homedepot (even though they currently DON'T result in a pass — the missing piece is the bundle being allowed to fetch under chapter 07 §Primitive 1's CSP relax).

### Step 4 — Land chapter 07 §Primitive 1 (CSP relax) — cross-chapter dependency

Chapter 07's `is_challenge_document_response` predicate already covers the sec-cpt body shape via the `/_sec/cp_challenge` UNAMBIGUOUS classifier row (`classify.rs:84`). With Primitive 1 wired, the sec-cpt body's CSP is relaxed → the obfuscated bundle is allowed to load.

Expected delta after Step 4: homedepot flips on iphone to `L3-RENDERED ≥ 50 KB` (consistent with pre-strip `b623d5d` measurement).

### Step 5 — Re-add sensor_data v2 encoder to vendor_solvers (PRIVATE)

In `~/projects/browser_oxide_internal/vendor_solvers/src/akamai/`:

- `payload.rs` — the 58-element tAD array (port from pre-strip `crates/akamai/src/payload.rs`)
- `crypto.rs` — TEA-CBC + XOR shuffle/substitute (port from pre-strip)
- `session.rs` — `AkamaiSession`, `AkamaiSessionStore`, the per-host MouseEvent buffer (port from pre-strip)
- `solver.rs` — implements `ChallengeSolver` for `AkamaiSolver { v2_encoder, store }`:
  - `name() → "akamai-bmp"`
  - `detect(resp, html)` — returns `Some(ChallengeKind::new("akamai-bmp", "sensor-data"))` when `_abck=…~0~-1~-1~` is present in cookies AND body is the bootstrap (not sec-cpt)
  - `solve(page, client, kind)` — builds the cleartext via `payload::build_cleartext(profile, session, page.url)`, encrypts via `crypto::build_v2_*`, POSTs to the per-tenant URL, observes the `_abck` response

Expected delta after Step 5: sites that fail because the bundle's in-V8 self-solve is unreliable (adidas might be one) gain a Rust-side reliable encoder.

### Step 6 — Re-add sensor_data v3 encoder to vendor_solvers (PRIVATE)

`vendor_solvers/src/akamai/v3.rs` — port from pre-strip `crates/akamai/src/v3_payload.rs`. The `parse_bm_sz` cookieHash extraction + JSON-shuffled envelope. Verify byte-parity with glizzy's reference (the pre-strip test `2efb307` is the regression gate; port it as a private unit test).

### Step 7 — Re-add sec-cpt fallback PoW to vendor_solvers (PRIVATE, LAST PRIORITY)

`vendor_solvers/src/akamai/sec_cpt.rs` — port the rolling-hash PoW. **This is the lowest priority** because the sec-cpt bundle self-solves in V8 (per pre-strip Phase 5 measurement). The Rust solver is a fallback for the case where the bundle doesn't execute correctly — but with chapter 07 §P1 + chapter 26 §3.B both in place, the bundle should execute correctly.

### Step 8 — Re-validate against the full 126 sweep

Compare HEAD-pre-restore to HEAD-post-restore on the 3-run aggregate:

| Site | Pre | Step 1+2+3 | Step 4 (07 P1) | Step 5 | Step 6 |
|---|---|---|---|---|---|
| homedepot | Akamai-CHL 2754 | (unchanged) | **L3-RENDERED iphone** | (unchanged) | (unchanged) |
| adidas | L3 2494 ×3 + L3 1.3M firefox | (unchanged) | (unchanged) | possibly L3 ≥ 50 KB on chrome too | possibly L3 ≥ 50 KB on chrome too |
| bestbuy | L3 7833-7887 (splash, not an Akamai miss) | (unchanged) | (unchanged) | (unchanged) | (unchanged) |

Routed strict-pass delta after Step 4: **+1 site** (108 → 109). After Steps 5+6 (private solvers): possibly **+2** if adidas pulls in on a second profile. After all chapters (05/06/07/08/26): the bar is ≥ 113 routed per chapter 12 §1.

---

## 7. Acceptance for v0.1.0

- [ ] **Mechanism documented in same depth as 07/06/08**: this chapter, with the 4-flow taxonomy in §2 and the 8-step restore sequence in §6.
- [ ] **adidas firefox-only-win identified**: capture diff per §4.1 between chrome and firefox `fetches.json` / `cookie_writes.json` recorded in a `docs/research_2026_05_XX/adidas_firefox_diff.md`. The §4.1 hypothesis (a/b/c/d) is narrowed to the one fingerprint field that uniquely flips the trust score.
- [ ] **`parse_abck_trust` / `AbckTrust` lands in public** (`crates/browser/src/classify.rs`) with unit tests; the cookie-delta retry block consumes it.
- [ ] **Vendor-detect logger extended** (`page.rs:1054-1069`) per §3.C with `x-akamai-transformed`, `Server-Timing: ak_p`, and the `_abck`/`bm_sz`/`sec_cpt`/`ak_bmsc` set-cookie observations.
- [ ] **homedepot reproducibly tested before-and-after** the vendor_solvers re-add (Step 5+6+7) on iphone_15_pro_safari_18. The pre-strip Phase 5 measurement (`L3-RENDERED 2507`) is the ceiling target.
- [ ] **`started_as_seccpt_challenge` flag verified live** — the iter trace from §3.B's measurement step shows the flag firing on the homedepot 428 response.
- [ ] **126-site regression gate green** after each of Steps 1, 2, 3, 4 (Steps 5+6+7 are private and gated separately in `vendor_solvers`).
- [ ] **No vendor solver code in public**: `git diff --stat aecdf19..HEAD -- crates/akamai crates/browser/src/datadome_handler.rs crates/browser/src/solvers/ crates/stealth/src/{kasada,cloudflare,qrator,aliyun,douyin,ngenix}.rs crates/net/src/kasada_session.rs` shows ZERO additions.

---

## 8. Out of scope — what stays private

Per the CLAUDE.md scope rule + the `aecdf19` G6 commit message:

> Per-vendor solver IMPLEMENTATIONS now live in the private companion `vendor_solvers` crate.

Concretely:

- **sensor_data v2 encoder** (`crates/akamai/src/payload.rs` + `crypto.rs` pre-strip) — 412 + 591 LOC. Tested for byte-perfect parity against glizzy's reference (commits `2efb307`, `f88b92e`). Stays private. The public engine just needs the cookie state machine to know whether to retry (Primitive A).
- **sensor_data v3 encoder** (`crates/akamai/src/v3_payload.rs` pre-strip) — 447 LOC. Modern post-2024 envelope. JSON-shuffled per glizzy + `bm_sz` cookieHash. Stays private.
- **TEA-CBC** (`crates/akamai/src/tea_cbc.rs` pre-strip) — 216 LOC. The algorithm is 1994 academia (Tiny Encryption Algorithm), but our *use* is specifically vendor encryption — stays in the private side for naming-discipline reasons.
- **sec-cpt PoW solver** (`crates/akamai/src/sec_cpt.rs` pre-strip) — 243 LOC. Rolling-hash brute-force preimage search. The pre-strip code's `solve_crypto` had ZERO non-test callers — the bundle self-solves in our V8 once the CSP allows it to fetch. So even the private vendor_solvers doesn't *need* this for v0.1.0; keep it as a fallback for bundle-load-failure cases.
- **DRAIN_JS** (`crates/akamai/src/drain.rs` pre-strip) — 204 LOC. The in-V8 collector that observes mouse/key/touch/scroll counters and exposes them to the Rust encoder via `globalThis.__akamai_events`. The JS surface (`__akamai_events`) is profile-neutral and stays in public at `page.rs:1186-1196`; the Rust harvester stays private.
- **Per-host file-hash registry** (pre-strip `crates/akamai/src/lib.rs` per-host registry) — the map from tenant hostname → expected obfuscated bundle URL. Each entry is a per-vendor secret. Stays private.

The boundary holds: public engine carries the **cookie spec recognition + body-marker detection + CSP relaxation primitive + cookie-delta retry primitive**; private engine carries the **vendor encryption + per-tenant secrets + sensor encoding**.

---

## 9. Open questions

Tracked in `15_OPEN_QUESTIONS.md` as cross-references:

- **Q26.1** Does the firefox-only adidas pass survive a chrome-class-TLS swap (i.e. is it the TLS or the JS surface)? Test: run firefox_135_macos with `BROWSER_OXIDE_TLS_OVERRIDE=chrome_147` and re-measure.
- **Q26.2** Is the homedepot iphone-only pass reproducible under HEAD with chapter 07 §P1 + chapter 26 §3.A applied, BEFORE the private vendor_solvers re-add? (The pre-strip Phase 5 hypothesis was: yes — the bundle self-solves, no solver needed.)
- **Q26.3** What's the actual `Server-Timing: ak_p` BotScoreVector breakdown for our 4 profiles on homedepot? Does adidas show a different per-profile score split?
- **Q26.4** Does Akamai's v3 envelope rotate at a faster cadence (`18_ANTI_BOT_VENDOR_COOKBOOK.md §5` says ~weekly for BMP)? Does the vendor_solvers test fixture need a quarterly refresh per `18 §5.2`?
- **Q26.5** Are there sites in our 126-corpus other than homedepot/adidas/bestbuy that show Akamai markers (`_abck`/`bm_sz`/`Server-Timing: ak_p`) and would benefit from this work? Per `18 §2.3` corpus list: walmart, nike-class, macys, hotels.com, h&m, footlocker may also be affected. Measure with the §3.C vendor-detect logger after Step 2.

---

## 10. Files referenced

### Public engine source (HEAD)

- `crates/browser/src/page.rs:1054-1069` — vendor-detect header logger (§3.C extends)
- `crates/browser/src/page.rs:1186-1196` — `__akamai_events` JS collector (kept post-strip; consumed by private solvers)
- `crates/browser/src/page.rs:1638-1663` — `started_as_dd_challenge` / `started_as_seccpt_challenge` / `started_as_cf_challenge` (Primitive B confirmed alive)
- `crates/browser/src/page.rs:1649-1650` — `started_as_seccpt_challenge` body-marker detection (the doc-20 fix from `b623d5d`)
- `crates/browser/src/page.rs:1969-1972` — challenge-poll entry gate (consumes `started_as_seccpt_challenge`)
- `crates/browser/src/page.rs:2007-2025` — DD-specific poll early-exit (Primitive A target — narrow to AbckTrust::Favorable)
- `crates/browser/src/page.rs:2107-2120` — wrong-BMP-POST suppression branch (DEAD with empty solvers — kept for the private vendor_solvers re-add)
- `crates/browser/src/page.rs:2125-2185` — cookie-delta retry block (Primitive A target — consume `AbckTrust`)
- `crates/browser/src/page.rs:2283-2293` — `v8_html_is_real` body-marker guard (keeps `_abck`, `bm_sz`, `/149e9513-`)
- `crates/browser/src/page.rs:2590-2593` — secondary `_abck` / `bm_sz` body check (the post-retry "is the body still a challenge" gate)
- `crates/browser/src/classify.rs:84` — `/_sec/cp_challenge` UNAMBIGUOUS row → `Akamai-sec-cpt-CHL`
- `crates/browser/src/classify.rs:96` — `pardon our interruption` PHRASE row → `Akamai-CHL`
- `crates/browser/src/classify.rs:103-104` — `akam/13` + `_abck` SMALL_BODY rows → `Akamai-CHL`
- `crates/browser/src/classify.rs:127-134` — `AKAMAI_CHALLENGE_COSIGNAL` (the FP-Tier1 gate so `akam/13` alone doesn't FP on benign Akamai pages)
- `crates/browser/src/classify.rs:160-168` — `small_body_row_qualifies` (the co-signal gate consumer)
- `crates/browser/src/challenge.rs:55-161` — `ChallengeSolver` trait + `ChallengeKind` + `SolveOutcome` (the seam private vendor_solvers binds to)
- `crates/stealth/src/presets.rs:413-495` — `firefox_135_macos()` preset (§4.1 candidates)
- `crates/stealth/src/presets.rs:120-196` — `chrome_148_macos()` preset (§4.1 control)
- `crates/stealth/src/presets.rs:954` — `firefox_webgl_is_masked` test (the WebGL-masking invariant in §4.1 hypothesis d)
- `crates/stealth/src/presets.rs:887` — `http3_disabled_by_default_on_all_presets` (background)

### Public engine docs

- `docs/releases/v0.1.0-parity/02_GAP_ANALYSIS.md` — homedepot + bestbuy + adidas in the hard-residual / recoverable buckets
- `docs/releases/v0.1.0-parity/04_TOOLING_SPEC.md` — `--capture` mode, `fetches.json`, `cookie_writes.json`, `script_errors.json`, `iter_summary.json`
- `docs/releases/v0.1.0-parity/06_AWS_WAF_SOLVER.md` — sibling vendor deep dive
- `docs/releases/v0.1.0-parity/07_DATADOME_PRIMITIVES.md` — sibling vendor deep dive (the structure this chapter mirrors); especially §Primitive 1 (`is_challenge_document_response` + CSP relax) and §Primitive 3 (`cookies_carry_anti_bot_clearance` + retry) which §3.A/B/C in this doc cross-link into
- `docs/releases/v0.1.0-parity/08_KASADA_FRONTIER.md` — sibling vendor deep dive (post-v0.1.0)
- `docs/releases/v0.1.0-parity/11_PER_PROFILE_STRATEGY.md §3.2` — adidas in the 4 routing-required sites; §4.1 rule 1 "Akamai class → try firefox"
- `docs/releases/v0.1.0-parity/12_COMPETITIVE_LANDSCAPE.md §3.1-3.5` — competitive matrix for the 3 Akamai sites
- `docs/releases/v0.1.0-parity/14_TESTING_VALIDATION.md` — ±5-site noise floor; the 3-run aggregate gate
- `docs/releases/v0.1.0-parity/18_ANTI_BOT_VENDOR_COOKBOOK.md §1.3, §1.4, §2.3, §4.1, §4.3, §5, §6.2` — the encyclopedic Akamai entry this chapter deepens
- `docs/releases/v0.1.0-parity/19_PROFILE_EXPANSION_PLAN.md` — desktop Safari macOS candidate (relevant to the adidas/firefox-only-win hypothesis if Akamai treats Safari class differently)
- `docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md` — ±5-site WAF variance discipline
- `CLAUDE.md` — vendor scope rules; the boundary this doc respects
- `SCOPE.md` — in/out scope statements

### Pre-strip (recoverable via git show aecdf19)

- `crates/akamai/src/lib.rs` (838 LOC) — `BotScoreVector` parser, `parse_bm_sz`, `build_sensor_data` orchestrator, per-host file-hash registry
- `crates/akamai/src/crypto.rs` (591 LOC) — `build_v2_bestbuy` / `build_v2_dalphan`, `sha256_b64`, XOR substitution / shuffle primitives
- `crates/akamai/src/payload.rs` (412 LOC) — v2 cleartext field assembler (58-element tAD array)
- `crates/akamai/src/v3_payload.rs` (447 LOC) — v3 JSON cleartext schema (~30 keys)
- `crates/akamai/src/session.rs` (307 LOC) — `AkamaiSession`, `AbckState`, `AkamaiSessionStore`
- `crates/akamai/src/sec_cpt.rs` (243 LOC) — sec-cpt PoW rolling-hash solver
- `crates/akamai/src/tea_cbc.rs` (216 LOC) — TEA-CBC primitive
- `crates/akamai/src/drain.rs` (204 LOC) — `DRAIN_JS` collector
- `crates/akamai/src/datadome_crypto.rs` (407 LOC) — DataDome cohabited the akamai crate pre-strip; that's a separate chapter (07)
- `crates/browser/src/solvers/akamai.rs` (137 LOC) — `AkamaiSolver` wrapper that implemented `ChallengeSolver` for the akamai crate
- `crates/browser/src/datadome_handler.rs` (423 LOC) — chapter 07's restoration target; cited here because DD + Akamai shared `crypto.rs`

### Pre-strip pivotal commits (research history — `git show <hash>`)

- `aecdf19` — the G6 strip itself
- `b623d5d` — Inc 7 doc-20 anti-pattern fix: persistent `started_as_seccpt_challenge` (the fix that flipped homedepot pre-strip; the pattern §3.B continues)
- `7929a4b` — docs commit recording the homedepot flip ("HOMEDEPOT FLIPPED — Inc 7 doc-20 fix")
- `8c4afae` — "deterministic homedepot sec-cpt self-solve" (the K1 unblock execution)
- `376483b` — "sec-cpt guard — don't POST BMP sensor_data to sec-cpt verify" (the earlier guard, refined by `b623d5d`)
- `1f0900c` — "detect sec-cpt interstitial in is_anti_bot_challenge (homedepot)" (the initial detection)
- `24c19e3` — "_abck parser — slot 1 is a stop-signal threshold, not a trust toggle" (the correction informing the §2.2 state-machine table)
- `2efb307` — "end-to-end byte-perfect parity with glizzy's encryptSensorData" (the regression gate to port to vendor_solvers)
- `f88b92e` — "v3 substitute uses alphabet-membership lookup — byte-perfect parity with glizzy"
- `e14c1ba` — "v3 envelope per glizzy reference — elementSwapping(':') + cookieHash field 5"
- `c53fa56` — "wire v3 JSON cleartext into build_sensor_data — load-bearing"
- `6a6804e` — "v3_payload module — JSON cleartext schema (30 keys)"
- `9f9bb97` — "parse_bm_sz returns single cookieHash from index 2 — was wrong two-seed model"
- `51d2abd` — "default N=1 single POST — upper-edge of variance band per multi-run sample"
- `6636f3e` — "BOXIDE_AKAMAI_FILE_HASHES env var override for per-host fileHash"
- `0aa38b4` — "update homedepot fileHash 8806534 → 2900615 — rotation observed"
- `994ad34` — "capture_bmak_js uses get_follow for redirect chains"
- `cd6509a` — "pin homedepot fileHash 8806534 — captured via challenge-page bmak path"
- `da720ad` — "pin macys fileHash 2752023 + add hotels/h-m capture tests"
- `dd9c464` — "pin bestbuy fileHash 6249250 from live bmak.js capture"
- `e0dc62a` — "per-host fileHash registry + build_v3_for_host plumbing"

### External research (URLs cited in this doc)

- https://github.com/xiaoweigege/akamai2.0-sensor_data — v2 reference
- https://github.com/Edioff/akamai-analysis — signal taxonomy
- https://github.com/i7solar/Akamai — 1.7X reference
- https://github.com/cirleamihai/akamai-1.7-cookie-generator — 1.7X reference
- https://github.com/Hyper-Solutions/hyper-sdk-js — commercial multi-vendor SDK
- https://pkg.go.dev/github.com/FRIS-Solutions-Vault/akamai-sdk-go — Go-based reference
- https://github.com/Bobby-coder/Akamai-Cookie-Generator — 1.7X
- https://medium.com/@glizzykingdreko/akamai-v3-sensor-data-deep-dive-into-encryption-decryption-and-bypass-tools-da0adad2a784 — the canonical v3 walkthrough
- https://docs.hypersolutions.co — commercial SDK protocol docs (the most accurate public reference for BMP v3 + sec-cpt)
- https://scrapfly.io/bypass/akamai — commercial bypass overview
- https://www.zenrows.com/blog/bypass-akamai — commercial bypass overview
- https://www.akamai.com/products/bot-manager — vendor pitch (no protocol detail)

### Measurement data

- `/tmp/full_sweep_2026_05_24/bo_chrome_148_macos_cold.json` — `homedepot Akamai-CHL 2754`, `adidas L3 2494`, `bestbuy L3 7887`
- `/tmp/full_sweep_2026_05_24/bo_pixel_9_pro_chrome_148_cold.json` — same as chrome for homedepot/adidas; bestbuy `7833`
- `/tmp/full_sweep_2026_05_24/bo_iphone_15_pro_safari_18_cold.json` — same pattern; iphone is the pre-strip homedepot winner
- `/tmp/full_sweep_2026_05_24/bo_firefox_135_macos_cold.json` — **adidas L3 1314086** (the firefox-only-win); homedepot still Akamai-CHL 2754
- `/tmp/full_sweep_2026_05_24/comp_camoufox.json` — `homedepot Akamai-CHL 2638` (cross-engine fail), `adidas L3 2384`, `bestbuy L3 7465`
- `/tmp/full_sweep_2026_05_24/comp_patchright.json` — `homedepot L3 1245838` (Playwright family wins!), `adidas L3 10678`, `bestbuy L3 7103`
- `/tmp/full_sweep_2026_05_24/comp_playwright.json` — `homedepot L3 1077510`
- `/tmp/full_sweep_2026_05_24/comp_playwright_stealth.json` — `homedepot L3 1076915`

### Memory (auto-context, persistent across sessions)

- `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/MEMORY.md` — entry-point index
- `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/state_2026_05_16_phase5_datadome.md` — pre-strip Phase 5 homedepot flip (the Increment 7 / `b623d5d` record; the load-bearing prior measurement)
- `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/tier1_priority_for_akamai.md` — the 2026-04-10 adidas sensor VM instrumentation (which navigator/canvas/audio APIs the VM actually reads — the §4.1 hypothesis d evidence)
- `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/kasada_akamai_real_blocker_2026_04_17.md` — the 2026-04-17 correction that Kasada/Akamai blocks are headless-fingerprint, NOT IP-reputation
- `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/measurement_holistic_chl_fp_trap.md` — the size-gate ≥30 KB rule that explains the adidas `L3 2494` measurement-correctness caveat in §4.1
- `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/proxy_not_the_problem.md` — vindication that real Chrome from same IP passes (the engineering surface is fingerprint, not network)
- `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/state_2026_05_17_unblock_execution.md` — UNBLOCK round including K1 deferral of parallel Rust `compute_cd_header` (Kasada parallel; Akamai pattern equivalent for `started_as_seccpt_challenge`)
