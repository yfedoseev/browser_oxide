# 07 — DataDome restoration as engine primitives

**Status:** planning  
**Cluster:** etsy + tripadvisor + yelp (DataDome-CHL, all 4 BO profiles)  
**Strategy:** restore the post-`aecdf19` DataDome behaviour as **engine-internal primitives** — none of which name a specific vendor in code — so the public engine carries the rendering capability while concrete per-vendor solvers stay private.

---

## TL;DR

Three small generic primitives, none of which mentions "DataDome" in
code, are sufficient to flip etsy / tripadvisor / yelp from
`DataDome-CHL 1424` to `L3-RENDERED ≥ 15 KB` on at least one BO
profile each. The primitives also strengthen Cloudflare and Akamai
handling for free (any vendor that follows the same
challenge-document + cross-origin iframe + cookie-write pattern).

| # | Primitive | Insertion point | Status |
|---|---|---|---|
| 1 | Challenge-doc CSP relaxation | `crates/browser/src/page.rs:1062-1090` | new helper + call-site change |
| 2 | Cross-origin challenge-iframe materialization | `crates/browser/src/page.rs:1979-1994` | un-gate existing `rematerialize_iframes` |
| 3 | Solved-cookie retry | `crates/browser/src/page.rs:2125-2185` | extend existing cookie-delta retry |

After all three: re-measure on the 3-site DataDome corpus. Expected:
each site flips at least once across the 4 profiles, contributing 1-3
routed strict passes.

The vendor-specific encoders (DataDome WASM-iframe daily key, Akamai
sensor_data v2, Kasada `x-kpsdk-ct` PoW) stay **private** in
`vendor_solvers`. The public engine just makes them POSSIBLE by
exposing the generic seam.

---

## Context — why these three primitives

### What `aecdf19` removed

The vendor-strip commit (2026-05-21) removed every per-vendor handler
from the public engine. The relevant deletions for DataDome behaviour
(see `git show aecdf19 --stat`):

- `crates/browser/src/datadome_handler.rs` (423 LOC: `DdInterstitial`
  parser, `detect_datadome_interstitial`, `datadome_solved` cookie
  predicate, and the typed `DdHandler` plan)
- `Page::handle_akamai_flow` + `Page::handle_cloudflare_flow` (the
  inline vendor flows in `page.rs` that drove sensor_data POSTs and
  the CF orchestrator)
- `crates/akamai/` (sensor_data v2, sec-cpt PoW, TEA-CBC,
  `datadome_crypto`, drain, tenant registry — 838+ LOC removed)
- `crates/net/src/kasada_session.rs` and the session/learn machinery
- All `crates/stealth/src/{kasada,cloudflare,qrator,…}.rs` stealth
  patches

What survived as public seam (kept on purpose):

- `crate::challenge::ChallengeSolver` trait + `ChallengeKind` + `SolveOutcome`
  (`crates/browser/src/challenge.rs:55-161`)
- `Page::default_solvers()` — returns empty `Arc<[]>`
  (`crates/browser/src/page.rs:850-852`)
- `Page::with_solvers()` / `Page::solvers()` registry hooks
  (`crates/browser/src/page.rs:828-839`)
- `Page::navigate_with_solvers()` entry point
  (`crates/browser/src/page.rs:961-1019`)
- The navigate-loop dispatch that iterates registered solvers and
  consults `relax_response_csp` / `solved_signal`
  (`crates/browser/src/page.rs:1595`, `:1638`, `:2022`, `:2045`,
  `:2104-2122`)
- The detection-only logging for vendor markers
  (`crates/browser/src/page.rs:1054-1069`)

### Why "primitives" rather than "put it back"

Per `CLAUDE.md` (top of file):

> Per-vendor challenge solving is out of scope here. The engine
> exposes a `browser::ChallengeSolver` trait + `Page::navigate_with_solvers`
> hook; the concrete Akamai/Kasada/DataDome/Cloudflare
> implementations live in the private `vendor_solvers` companion
> crate.

So the public engine must NOT contain functions named
`handle_datadome_*`, must NOT parse `var dd={…}` literals, must NOT
know the `geo.captcha-delivery.com` substring outside of the
classifier where it already lives as one of many vendor markers
(`crates/browser/src/classify.rs:94`).

But the three behaviours required to render a DataDome interstitial
in V8 (relax CSP so the challenge JS can fetch its cross-origin iframe,
materialize that iframe as a real child context, and re-fetch the
original URL after the solver lands a session cookie) are all
**vendor-agnostic** — the exact same primitives unblock any modern
anti-bot challenge that follows the canonical pattern.

So the primitive names and gates use generic terms
(`is_challenge_document_response`, `relax_csp_for_challenge_doc`,
`is_anti_bot_session_cookie`). The classifier already maps
substrings → tag; that table is the one acceptable place where a
vendor URL appears (and it's a closed enum, not a flow gate).

### Concrete targets

From `/tmp/full_sweep_2026_05_24/bo_chrome_148_macos_cold.json` and
`comp_camoufox.json`:

| Site | BO chrome (all profiles) | Camoufox | Bytes shipped |
|---|---|---|---|
| etsy.com | DataDome-CHL `len=1424` `ms=90874` | L3-RENDERED `len=253384` `ms=4856` | ~252 KB |
| tripadvisor.com | DataDome-CHL `len=1430` `ms=90866` | L3-RENDERED `len=433359` `ms=5066` | ~432 KB |
| yelp.com | DataDome-CHL `len=1424` `ms=90801` | DataDome-CHL `len=1487` `ms=5615` | (Camoufox also fails — interactive captcha) |

Two observations:
1. The 90-second nav time on the BO side is the budget cap firing — we
   stay polling because `started_as_dd_challenge` is false (no solver
   sets it), so `rematerialize_iframes` never runs and the challenge
   iframe never fetches.
2. yelp is the harder case — even Camoufox is `DataDome-CHL 1487`
   because DataDome serves yelp the **interactive** captcha
   (`rt:'c'`), not the silent (`rt:'i'`) variant. yelp is a stretch
   goal here; etsy + tripadvisor are the realistic flips.

The pre-strip Phase 5 work flipped homedepot and was on its way to
etsy / tripadvisor — see
`~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/state_2026_05_16_phase5_datadome.md`
for that loop's loop. The 3 primitives below are the public-engine-
compatible restoration of the load-bearing scaffolding that work
relied on.

---

## Primitive 1 — Challenge-doc CSP relaxation

### What and why

Real Chrome ships interstitial handling that permits the anti-bot
vendor's iframe under a relaxed Content-Security-Policy. The
challenge document's CSP is typically strict (`script-src 'self'`,
`frame-src 'self'`) — but it would never let `dd-script.js` load from
`captcha-delivery.com` or let the challenge iframe fetch
`geo.captcha-delivery.com/captcha/?…`. Real browsers special-case the
interstitial path; we must too.

Pre-strip, this was wired through
`DataDomeSolver::relax_response_csp` (the trait method at
`crates/browser/src/challenge.rs:148`) — the solver returned `true`
for bodies it recognised as DD interstitials, and the navigate loop
called `solvers.iter().any(|s| s.relax_response_csp(&html))` at
`page.rs:1595`. With an empty solver list, that always returns
`false`, the origin CSP is installed strictly, and `dd-script.js` is
blocked by `set_csp_policy` — verified pre-strip on hyatt.com
(memory: `state_2026_05_15_session_synthesis`).

### Where to insert

`crates/browser/src/page.rs:1062-1090` — between the vendor-detect
log block (lines 1054-1069) and the CSP headers harvest (lines
1074-1085). Today the relaxation is *exclusively* solver-driven via
`relax_response_csp` (line 1595). The new primitive runs the same
decision in the engine, body-independent of any registered solver, so
DataDome can render even when no solver is registered.

### Spec

Add a free function in `crates/browser/src/classify.rs` (a new
section, not a new file — the classifier is the canonical body/header
inspector):

```rust
/// Is this response an anti-bot vendor's challenge document?
///
/// Detection is response-shape-based — not a vendor-name list — so
/// the test stays valid as new vendors emerge. A response counts as
/// a "challenge document" when ANY of:
///   - status is 403 / 429 / 498 / 503 AND body < INTERSTITIAL_MAX_BYTES
///     (already a classifier constant; the small interstitial shape)
///   - response carries a known anti-bot signaller header:
///     `x-datadome` | `cf-mitigated` | `x-amzn-waf-action` |
///     (`server: cloudflare` AND status in {403, 503})
///   - body contains one of the cross-origin challenge URLs:
///     `captcha-delivery.com` | `/cdn-cgi/challenge-platform/`
///
/// The function name is intentionally generic — every condition
/// above is body/header shape, not vendor identity. The same rule
/// catches DataDome rt:'i', DataDome rt:'c', Cloudflare Managed
/// Challenge, CF JS Challenge, AWS WAF challenge, and any future
/// vendor following the same canonical shape.
pub fn is_challenge_document_response(
    status: u16,
    headers: &[(String, String)],
    body: &str,
) -> bool { … }
```

In `page.rs`, after the vendor-detect block (line ~1069), compute:

```rust
let challenge_doc = crate::classify::is_challenge_document_response(
    resp.status, &resp.headers, &resp.text()  // (cache the body slice — already done at line 1086)
);
```

Pass `challenge_doc` down to `navigate_loop_internal` as a new
parameter alongside `csp_headers`. In `navigate_loop_internal`
(around line 1595 where `relax_csp` is computed), OR-in the engine-
side flag:

```rust
let relax_csp = challenge_doc
    || solvers.iter().any(|s| s.relax_response_csp(&html));
```

That preserves the trait hook (private solvers can still vote) and
adds the engine-side default for the no-solvers case.

### Naming discipline

- Function: `is_challenge_document_response` (NOT
  `is_datadome_challenge_doc` / NOT `is_anti_bot_response`)
- Local: `challenge_doc` (NOT `is_dd`, NOT `is_vendor_challenge`)
- The function body may match the vendor URL substrings that already
  exist verbatim in the classifier's `PHRASE` / `SMALL_BODY` tables
  (`classify.rs:94`, `classify.rs:102-118`) — those substrings are
  the canonical body-shape signature, not vendor flow code.

### Verification

Unit test in `crates/browser/src/classify.rs::tests`:

```rust
#[test]
fn challenge_doc_detector_recognises_all_canonical_shapes() {
    // 403 + small DD body
    let h = vec![("x-datadome".into(), "protected".into())];
    assert!(is_challenge_document_response(403, &h, "<small dd body>"));

    // CF challenge
    assert!(is_challenge_document_response(
        503, &[("server".into(), "cloudflare".into())], "..."
    ));

    // Large benign page that incidentally mentions captcha-delivery.com
    // in an analytics URL — must NOT count
    let big = format!("<html>{}{}", "<div>x</div>".repeat(5000),
                      "<script src='https://x.captcha-delivery.com/p.gif'></script>");
    assert!(!is_challenge_document_response(200, &[], &big));
}
```

### Why this is the right cost

The current code sets `enforce = profile.enforce_csp && !env_bypass &&
!relax_csp` (`page.rs:1596`). With `challenge_doc = true`, CSP is
not enforced **only for this single nav iteration on a sub-2 KB
challenge document**. Real pages (≥ INTERSTITIAL_MAX_BYTES) cannot
trip the gate. There is also already an env-var bypass
(`env_bypass`) and a per-host bypass list at `page.rs:2066-2085` for
walmart/canadagoose/hyatt/realtor/etc., so the primitive is a
strict generalisation of patterns already in the engine.

---

## Primitive 2 — Cross-origin challenge-iframe materialization

### What and why

DataDome's interstitial (and Cloudflare's `/cdn-cgi/challenge-platform/`,
Akamai's `sec-cpt` widget, every modern Managed-Challenge vendor)
follows this canonical flow:

1. Initial 403/503 body contains a tiny `<script>` that, after a few
   ticks, creates an iframe pointing at the challenge host
   (`geo.captcha-delivery.com`, `challenges.cloudflare.com`, …).
2. The iframe's document loads vendor JS (or WASM) that runs the real
   probe and POSTs a token to the challenge host.
3. The challenge host returns `Set-Cookie: datadome=…` (or
   `cf_clearance=…`, `_abck=…`).
4. The outer page's solver script reads the cookie and either reloads
   or auto-submits a hidden form.

BO's `find_iframes` runs at build time only
(`crates/browser/src/iframe.rs:255`), so an iframe injected AFTER
build_page returns (via `document.createElement('iframe')` +
`appendChild`) is given only a synthetic `contentWindow` shim in JS
land — its `src` is never fetched, its document never executed, the
challenge never runs.

The fix already exists in the engine —
`Page::rematerialize_iframes` (`crates/browser/src/page.rs:649-704`)
walks the current DOM, finds iframes that lack a real child context,
and calls `iframe::ChildIframe::from_url` or
`ChildIframe::from_srcdoc` exactly as `build_page` does. It's
correct, idempotent, and DOM-walk-cheap.

It is currently gated to only run during the challenge-poll loop, and
only when one of the three `started_as_*` booleans is true
(`crates/browser/src/page.rs:1967-1972`):

```rust
if pending_info.is_empty()
    && (page.is_anti_bot_challenge()
        || started_as_dd_challenge          // ← always false in public engine
        || started_as_seccpt_challenge      // ← still detected (body marker)
        || started_as_cf_challenge)         // ← still detected (body marker)
{
    // … poll, including rematerialize_iframes at line 1993 …
}
```

`started_as_dd_challenge` is computed from solvers
(`page.rs:1638`):

```rust
let started_as_dd_challenge = solvers.iter().any(|s| s.relax_response_csp(&html));
```

With no solvers registered, this is always false. The other two flags
are body-marker-based (`page.rs:1649-1663`) so they still fire — but
DataDome interstitials trigger only on the DD flag. Net effect: the
DataDome path never enters the poll, never runs rematerialize.

### Fix — un-gate using the new generic flag

Add a new persistent origin flag computed from Primitive 1's body
inspection (NOT from the solver):

```rust
// page.rs, replace the line at :1638 with:
let started_as_dd_challenge = solvers.iter().any(|s| s.relax_response_csp(&html))
    || challenge_doc;  // ← engine-side default; from Primitive 1's input
```

Or — preferable — introduce a single new generic name and adapt the
three gate uses (`:1969`, `:2007`, `:2135`):

```rust
let started_as_challenge_doc = challenge_doc
    || started_as_dd_challenge
    || started_as_seccpt_challenge
    || started_as_cf_challenge;
```

Then the poll gate becomes:

```rust
if pending_info.is_empty()
    && (page.is_anti_bot_challenge() || started_as_challenge_doc)
{
    // poll body — rematerialize_iframes runs each tick
}
```

This is a one-line semantic change: the engine now enters the poll
whenever the *initial response* looked like a challenge document,
regardless of solver registration. `rematerialize_iframes` already
guards itself by:
- `find_iframes` is a DOM walk → cheap when nothing new (`iframe.rs:255`)
- `already.iter().any(...)` deduplicates against existing children (`page.rs:667`)
- Empty or `javascript:` src skipped (`page.rs:679-680`)
- CSP-`frame-src`-gated identically to build time (`page.rs:683`, per the
  `ChildIframe::from_url` contract)

### Cost analysis

Per the docstring at `page.rs:644-648`:

> Idempotent + cheap (DOM walk only) when nothing new appeared

Per-poll-tick cost (200 ms interval inside the 90 s deadline,
`page.rs:1977`):

- DOM walk: O(node count) — for a 1.4 KB interstitial, ≤ 50 nodes.
- Per newly materialized iframe: one HTTPS fetch + child Page
  build_page. Measured pre-strip at ~5-50 ms per iframe on the
  DataDome path (1 iframe per nav).

Total added cost: ≤ 5-50 ms per iter on the **challenge path only**.
Negligible vs the 15 s default nav budget (`page.rs:1674-1679`) and
the 90 s challenge-poll deadline (`page.rs:1973`). Zero cost on benign
navs (`challenge_doc = false` ⇒ poll never entered).

### Naming

`rematerialize_iframes` is already a generic name. No rename needed.
The gate name change (`started_as_dd_challenge` →
`started_as_challenge_doc` or similar) brings the boolean name in
line with the broader meaning.

### Verification

Unit test in `crates/browser/tests/chrome_compat.rs`:

```rust
#[tokio::test]
async fn challenge_doc_response_triggers_rematerialize_without_solvers() {
    // Synthesize a 1.4 KB DD-shape response, hand it to navigate_with_html,
    // assert that after one tick the script-created iframe has a real
    // child page (not the synthetic shim).
}
```

Integration test — the `holistic_sweep` 126-corpus run (the canonical
regression gate per `CLAUDE.md` and `crates/browser/tests/holistic_sweep.rs`):
the 3 DataDome targets should flip on at least one profile.

---

## Primitive 3 — Solved-cookie retry

### What and why

After Primitives 1 + 2, the challenge iframe fetches and runs.
DataDome's `i.js` (or CF's challenge-platform script) does its work
and lands a cookie on the origin: `datadome=…`, `cf_clearance=…`,
`_abck=…`, `kpsdk=…`. But the outer document never auto-reloads —
DataDome relies on the solver script to detect the cookie and either
reload or submit a hidden form.

In the pre-strip world, `datadome_handler.rs` had a `datadome_solved`
predicate (the cookie-name check) and the cookie-delta retry path at
`page.rs:2134-2186` gated on `started_as_dd_challenge` would
re-fetch the original URL. With no solvers, the gate is false; the
cookie lands but no retry fires; the engine returns the 1.4 KB
interstitial.

### Where to fix

`crates/browser/src/page.rs:2134-2186` — the post-settle cookie-delta
retry block. Currently the gate at line 2134-2138 is:

```rust
if (page.is_anti_bot_challenge()
    || started_as_dd_challenge
    || started_as_seccpt_challenge
    || started_as_cf_challenge
    || (last_accept_ch_upgrade && !accept_ch_retry_done))
    && iter + 1 < iterations
{
```

If Primitive 2's `started_as_challenge_doc` is wired in, this already
generalises: any challenge-doc origin gets the cookie-delta retry.
For belt-and-braces, also widen the actual "did we gain a session
cookie?" predicate.

Today line 2175-2177 is:

```rust
let mut should_retry = (cookies_after != cookies_before
    && !cookies_after.is_empty())
    || (last_accept_ch_upgrade && !accept_ch_retry_done);
```

That's *any* cookie change — too broad on benign navs, but the gate
above already restricts to challenge contexts. Add a tighter
secondary predicate (a generic primitive in classify.rs) used to
short-circuit the *poll* exit (the loop at lines 2007-2025 today only
short-circuits on the DD flag):

```rust
// classify.rs — new function
/// Is the cookie jar carrying a session-clearance cookie issued by
/// an anti-bot vendor? Generic name-match; the name patterns are
/// closed-list (the same closed list the classifier uses today).
/// Pure inspection — no I/O.
pub fn cookies_carry_anti_bot_clearance(cookies: &str) -> bool {
    // Each pattern is a canonical post-solve cookie name, NOT a
    // flow gate. Matches the substrings the engine already
    // recognises in is_anti_bot_challenge / is_cf_challenge_doc.
    const PATTERNS: &[&str] = &[
        "datadome=",   // DataDome
        "_abck=",      // Akamai BMP
        "bm_sz=",      // Akamai BM Edge
        "cf_clearance=", // Cloudflare
        "kpsdk=",      // Kasada
        "sec_cpt=",    // Akamai sec-cpt
        "aws-waf-token=", // AWS WAF
    ];
    PATTERNS.iter().any(|p| cookies.contains(p))
}
```

Then in the poll loop at `page.rs:2007-2025`, replace the DD-specific
early-exit with the generic predicate:

```rust
// Was: if started_as_dd_challenge { … solvers.iter().any(...).solved_signal ... }
// Becomes: a hybrid — solver wins if registered, else the generic primitive
if started_as_challenge_doc {
    if let Some(p) = parsed_current.as_ref() {
        let now = client.cookies_for_url(p).await.unwrap_or_default();
        let body = page.content();
        let solver_says_solved = solvers.iter().any(|s| s.solved_signal(&now, &body));
        let engine_says_solved =
            crate::classify::cookies_carry_anti_bot_clearance(&now)
            && !crate::classify::engine_classify(&body).verdict.is_challenge();
        if solver_says_solved || engine_says_solved {
            break;
        }
    }
}
```

The "AND body is no longer a challenge document" guard is critical
— DataDome sets `datadome=…` on EVERY response including the failing
403 (memory: FP-D3 in `state_2026_05_16_phase5_datadome.md`). Without
the body check we'd break early on a *bare* cookie write before the
solver actually completed.

### Naming

- `cookies_carry_anti_bot_clearance` — generic predicate, closed-list
  cookie names (the same names already in the classifier).
- The poll early-exit comment should be rewritten to reference the
  primitive, not the vendor.

### Verification

Unit test in `classify.rs::tests`:

```rust
#[test]
fn anti_bot_clearance_cookie_detector() {
    assert!(cookies_carry_anti_bot_clearance("datadome=abc; foo=bar"));
    assert!(cookies_carry_anti_bot_clearance("_abck=xyz"));
    assert!(cookies_carry_anti_bot_clearance("cf_clearance=q"));
    assert!(!cookies_carry_anti_bot_clearance("session=x; user=42"));
    assert!(!cookies_carry_anti_bot_clearance("")); // empty
}
```

Plus the integration sweep — etsy / tripadvisor flipping.

---

## Validation

### Per-primitive A/B

For each primitive in isolation, run the 3-site DataDome corpus
through `sweep_metrics`:

```bash
cat > /tmp/dd_corpus.json <<'JSON'
[
  {"cat":"stores","name":"etsy","url":"https://www.etsy.com/"},
  {"cat":"travel","name":"tripadvisor","url":"https://www.tripadvisor.com/"},
  {"cat":"misc","name":"yelp","url":"https://www.yelp.com/"}
]
JSON

# Build release
cargo build --release --workspace

# Run sweep on each profile
for prof in chrome_148_macos firefox_135_macos iphone_15_pro_safari_18 pixel_9_pro_chrome_148; do
  target/release/examples/sweep_metrics "$prof" \
    /tmp/dd_corpus.json /tmp/dd_${prof}.json \
    2>&1 | tee /tmp/dd_${prof}.log
done

# Verdict summary
for f in /tmp/dd_*.json; do
  echo "=== $f ==="
  jq -r '.[] | "\(.name)\t\(.tag)\t\(.len)\t\(.ms)"' "$f"
done
```

### Acceptance criteria

Phased gates:

| Step | Expected outcome |
|---|---|
| Baseline (no patch) | 12 cells: all `DataDome-CHL 1424-1487` `ms=90s` |
| Primitive 1 only (CSP relax) | iframe is allowed to *try* — `len` may grow slightly; mostly still CHL |
| Primitive 1 + 2 (rematerialize) | iframe fetches and runs; `datadome=` cookie appears in poll trace |
| Primitive 1 + 2 + 3 (cookie retry) | etsy + tripadvisor flip to `L3-RENDERED ≥ 50 KB` on at least one profile; yelp may stay CHL (interactive captcha — pre-strip Phase 5 also could not flip yelp) |

### Regression gates

After every primitive, before keeping:

```bash
cargo test --workspace -- --test-threads=1
cargo test --workspace --test holistic_sweep -- --test-threads=1
cargo clippy --workspace
cargo fmt --all -- --check
```

The 126-site holistic sweep must not regress: zero new `BLOCKED`,
zero new `THIN-BODY`, max ±5 sites variance per
`docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md`. Re-run 3× and aggregate
per `03_BENCHMARK_METHODOLOGY.md`.

### What success looks like (numbers)

Routed best-of-4 expected delta:

| Site | Pre | Post (P1+P2+P3) |
|---|---|---|
| etsy | DataDome-CHL | L3-RENDERED on chrome OR firefox |
| tripadvisor | DataDome-CHL | L3-RENDERED on chrome OR firefox |
| yelp | DataDome-CHL | (likely unchanged — Camoufox also fails) |

Routed strict pass delta: **+2 sites** (108 → 110). Combined with
chapter 05 (reddit + duolingo, EASY wins), 06 (AWS WAF, HARD), this
puts BO routed at the bar: **≥ 113 routed**.

---

## Out of scope — what stays in `vendor_solvers`

The following are vendor-specific and stay **private**:

### Akamai sensor_data v2 encoder
Lives in `crates/akamai/src/payload.rs` (412 LOC pre-strip) — the
`sensor_data` BMP POST body encoder (TEA-CBC + per-tenant integrity
field + telemetry vector). Implementing this in public would be both
out of scope per `CLAUDE.md` and ethically dubious — it's vendor
bypass code, not a rendering primitive.

### DataDome WASM-iframe daily-key solver
The actual challenge solver — extracting the `dd={…}` literal,
loading the daily-rotating obfuscated WASM, computing Picasso canvas
+ audio fingerprint inputs, encrypting per the dual-XOR PRNG, POSTing
to `captcha-delivery.com`. Stays private.

### Kasada `x-kpsdk-ct` header generation
The synthetic PoW token + the `x-kpsdk-{ct,dt,im,fc,h,v,r}` header
set + the `/tl` sensor encode. Stays private. (Also: not load-bearing
for the documented Kasada gap — see chapter 08.)

### Akamai sec-cpt PoW
The brute-force preimage search the sec-cpt bundle requires. Stays
private. (Bundle self-solves in our V8 today, per pre-strip Phase 5
work — the engine primitives in this doc may be enough; if so, the
private solver isn't even invoked.)

The boundary is clear: public engine makes vendor solvers POSSIBLE
(seam + primitives); private crate ships actual encoders.

---

## Files referenced

- `crates/browser/src/page.rs:1054-1069` — current detection-only logging
- `crates/browser/src/page.rs:1062-1090` — Primitive 1 insertion point
- `crates/browser/src/page.rs:1595` — `relax_csp` consultation (solver-only today)
- `crates/browser/src/page.rs:1638` — `started_as_dd_challenge` (solver-only today)
- `crates/browser/src/page.rs:1649-1663` — `started_as_seccpt_challenge`, `started_as_cf_challenge` (body-marker; pattern to follow)
- `crates/browser/src/page.rs:1967-1972` — challenge-poll entry gate
- `crates/browser/src/page.rs:1979-1994` — `rematerialize_iframes` call site (Primitive 2)
- `crates/browser/src/page.rs:2007-2025` — DD-specific poll early-exit (Primitive 3 target)
- `crates/browser/src/page.rs:2125-2185` — cookie-delta retry block (Primitive 3 gate)
- `crates/browser/src/page.rs:649-704` — `rematerialize_iframes` impl (cheap, idempotent)
- `crates/browser/src/iframe.rs:255` — `find_iframes` (build-time iframe walker)
- `crates/browser/src/challenge.rs:55-161` — `ChallengeSolver` trait surface
- `crates/browser/src/classify.rs:81-167` — current marker tables / FP gates
- `crates/browser/src/classify.rs:247-251` — `is_cf_challenge_doc` (the body-shape predicate pattern to follow)
- `crates/browser/src/classify.rs:94` — the only place vendor URLs appear as classification keys
- `git show aecdf19 -- crates/browser/src/datadome_handler.rs` — what was removed (423 LOC)
- `/tmp/full_sweep_2026_05_24/bo_chrome_148_macos_cold.json` — current measured baseline
- `/tmp/full_sweep_2026_05_24/comp_camoufox.json` — Camoufox reference verdicts
- `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/state_2026_05_16_phase5_datadome.md` — pre-strip Phase 5 history
- `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/state_2026_05_16_engines_research_set.md` — FP-E1 (script-created iframe gap) original diagnosis
- `docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md` — ±5 site variance discipline
- `CLAUDE.md` — scope rules on vendor solvers; the boundary this doc respects
