# 99 — Consolidated Code False-Positive Backlog

**Created:** 2026-05-16 · **Baseline git HEAD:** `fd98bfa` · Synthesis of
the §10 findings from all five engine docs, de-duplicated, prioritized,
each with a concrete fix and the regression test that would have caught
it. This is the actionable backlog.

**Priority key.** **P0** = corrupts our ability to *measure* a pass
(everything downstream is untrustworthy until fixed). **P1** = blocks or
silently mis-routes a real site. **P2** = capability-truth / hygiene
(misleads future work but doesn't itself flip a verdict).

---

## CLASS A — "Exists ≠ exercised" dead/unreached solver code  (P2, universal)

The crate advertises far more capability than the live `navigate()` path
exercises. Each item: byte-verified or present, but **not in the live
path**. The fix is almost never "wire it" (the live strategy is in-V8
bundle self-solve) — it is **truth-in-labeling** so future sessions stop
treating these as the solver.

| Engine | Symbol | Location | Reality | Fix |
|---|---|---|---|---|
| Akamai | `sec_cpt::solve_crypto` | `crates/akamai/src/sec_cpt.rs:80` | Zero non-test callers (only a *comment* in `capture_bmak_js.rs:230`). Also un-feedable: homedepot serves no parseable 428 JSON. | Doc-comment `//! DEAD: not in live path; homedepot self-solves in V8`. Do **not** wire. |
| Akamai | `BotScoreVector::parse` | `crates/akamai/src/lib.rs` | Parses `ak_p` (a free regression oracle) then the result is discarded. | Either log it as a drift oracle in the nav loop, or mark dead. |
| Akamai | v2 crypto / `reverse_*` / `tea_cbc` (misfiled Kasada) | `crates/akamai/src/{crypto,payload,tea_cbc}.rs` | DEAD; inflates apparent capability. | Mark dead; move `tea_cbc` out of the akamai crate or doc why it's there. |
| DataDome | `DdEncryptor` | `crates/akamai/src/datadome_crypto.rs:163` | Byte-verified by unit tests; **zero non-test callers** (grep-proven: own file + one doc comment + `pub mod` export). | Doc-comment `//! DEAD/insurance: live path is in-V8 i.js self-solve`. |
| Cloudflare | (no solver exists) | — | No code fetches `/orchestrate/`, POSTs `/flow/ov1/…`, or sets `cf_clearance`. `handle_cloudflare_flow` is a 10 s passive jar poll. | Document "detector-only, no solver" so it's not mistaken for one. |
| PerimeterX | (no solver exists) | — | Only the `px-captcha` substring detector. Detection without solver ⇒ any "PX-blocked" verdict is structurally unactionable. | Document greenfield status; do not emit an actionable-looking PX verdict. |
| Kasada | Rust PoW (`compute_cd_header`) | `crates/stealth/src/kasada.rs`, wired `net/lib.rs:692/863/1211/1405` | **WIRED but runs in parallel** to ips.js self-solving in V8. Not dead — *worse*: a Rust-injected single-use `x-kpsdk-cd` competing with ips.js's own is a plausible **self-inflicted `b:1`/replay signal**. | See FP-K1 (P1) — gate it off when ips.js is present. |

**Regression test for the class:** a `#[test]` that greps the workspace
for non-test callers of each symbol on this list and fails if a symbol
documented "DEAD" gains a caller without the doc being updated (keeps
labels honest).

---

## CLASS B — Detection / classifier false positives  (P0 — corrupts measurement)

These directly cause pass↔block mislabeling. Until fixed, *no site count
is trustworthy* (this is exactly how "22 engine-addressable" was inflated
from a true ~6).

### FP-B1 — Three divergent classifiers, different size gates  (P0)  ✅ DONE — commit on `fix/engine-fp-backlog`
- **Status:** FIXED. New `crates/browser/src/classify.rs::engine_classify`
  is the single source of truth (canonical policy = byte-for-byte the
  ledger-authoritative `holistic_sweep::classify`). `page.rs::{body_has_challenge_marker,
  challenge_verdict}` and `holistic_sweep::classify` are now thin
  delegates. Regression: `classify::tests::all_call_sites_agree`
  (cross-call-site identical-verdict table) + `verdict_mapping_is_consistent`.
  Gate green: chrome_compat 437/0, iframe_isolation 5/0,
  v8_inspector_parity 3/0, v8_natives 11/0, holistic classifier_tests
  10/10 unchanged (ledger equivalence proven). Known *intended*
  consequence (deferred to FP-B4): udemy now classifies the same as
  holistic already did (L3-RENDERED) — surfaces the real CF policy bug
  B4 fixes. No site flip asserted ⇒ no live re-measure required for B1.
- **Where:** `page.rs` strong-marker set (ungated) vs
  `holistic_sweep.rs:864 classify()` (<30 KB gate) vs the vendor
  handlers (≤4 KB gate) — all classify the *same* markers
  (`captcha-delivery.com`, `bm_sz`, `px-captcha`, `sec-cpt`)
  differently.
- **False claim:** "site X is blocked/passed" — but *which* classifier?
  The 29-set vs 126-corpus number confusion is downstream of this.
- **Why false:** the same body can be `Pass` in one classifier and
  `*-CHL` in another; counts quoted from the wrong one are inconsistent.
- **Fix:** one shared `engine_classify(body, headers, size) -> Verdict`
  used by `page.rs`, `holistic_sweep`, and the audit harness. Single
  source of truth for marker set + size gates.
- **Regression test:** table test feeding the same fixtures through all
  three call sites; assert identical verdict.

### FP-B2 — Non-size-gated literal strong markers mislabel rendered pages  (P0/P1)
- **Where:** `page.rs:181` (`px-captcha` in the *unconditional* strong
  set, unlike `_pxhd`/`human security` which are `stub_sized`-gated);
  `holistic_sweep.rs:875` (`px-captcha` any-body-size);
  `captcha-delivery.com` similarly ungated in `page.rs`.
- **False claim:** "this 1 MB page is a PerimeterX/DataDome challenge."
- **Why false:** a fully rendered page that merely *contains the literal
  substring* (CSS class, analytics key, cookie-consent JSON manifest —
  this is exactly the historical wayfair FP root) is mis-flagged.
- **Fix:** size-gate every strong marker, or require a structural
  co-signal (challenge-unique element id like `sec-if-cpt-container`,
  not a bare substring).
- **Regression test:** `classify(1 MB body containing literal
  "px-captcha"/"captcha-delivery.com" in an inline string) == Pass`.
  Note: the existing `medium_body_with_pxhd_substring_is_not_chl` only
  guards `_pxhd` — extend it to `px-captcha` and `captcha-delivery.com`.

### FP-B3 — Thin-shell "pass" under-match (FP in the other direction)  (P1)
- **Where:** `ChallengeVerdict::Pass` = rendered + no challenge marker on
  a 1-iteration nav.
- **False claim:** "bestbuy/spotify/duolingo pass" (7.8 / 9.6 / 13 KB).
- **Why false:** a small body below the challenge size but above noise
  can be a thin shell / SPA pre-hydration stub, not the real content —
  counted as a win it isn't.
- **Fix:** add a `ThinShell` verdict band (rendered, no challenge, but
  body < content-floor AND no expected content landmark); don't count
  it as a full pass without a content-depth check.
- **Regression test:** fixtures of known thin shells vs real renders;
  assert `ThinShell` vs `Pass` split.

### FP-B4 — udemy mislabeled `SensorFail`  (P1, Cloudflare)
- **Where:** `page.rs:175/277` classifies udemy's 476 KB body as
  `SensorFail`.
- **False claim:** "udemy fails the fingerprint/sensor stage."
- **Why false:** `/cdn-cgi/challenge-platform/` is size-independent and
  Cloudflare's always-on JSD URL is in *every* CF body; udemy's
  orchestrator simply **never completed** (structural — see FP-C2/G-CF-1),
  it is not a fingerprint failure. The label misdirects future work to
  fingerprint tuning instead of the iframe/orchestrate gap.
- **Fix:** distinguish `CfChallengeIncomplete` (orchestrator did not
  finish: no `cf_clearance`, no 302) from `SensorFail`.
- **Regression test:** CF "just a moment" fixture ⇒ `CfChallengeIncomplete`,
  not `SensorFail`.

---

## CLASS C — Mutable-state guard ("doc-20") class  (P1)

Guards keyed off post-mutation `self.content()` instead of a persistent
"this nav *started* as challenge X" flag — the bundle mutates the DOM,
the marker disappears, the guard misses, the wrong traffic fires.

### FP-C1 — Akamai sec-cpt guard still reads mutable `self.content()`  (P1, latent)
- **Where:** `handle_akamai_flow` sec-cpt guard, `page.rs:~470-476`.
- **State:** *dead in practice* — Inc 7 fenced the BMP POST behind the
  persistent `started_as_seccpt_challenge` (`page.rs:1432`). But the
  guard itself still reads `self.content()`; a **direct caller of
  `handle_akamai_flow`** (tests, future code) re-triggers the doc-20
  bug.
- **Fix:** make the guard take the persistent challenge-origin flag as a
  parameter; never re-derive from current DOM.
- **Regression test:** call `handle_akamai_flow` directly on a
  post-mutation DOM (sec-cpt marker gone) with origin=sec-cpt; assert no
  BMP POST.

### FP-C2 — Cloudflare cookie-delta retry: SAME bug, NOT fenced  (P1, live)
- **Where:** `page.rs:1836` cookie-delta retry gated on mutable
  post-build `self.content()`. **There is no persistent
  `started_as_cf_challenge` flag** (unlike DD/sec-cpt).
- **False claim:** "this CF page is no longer a challenge, stop
  retrying."
- **Why false:** CF's orchestrator mutates the body; the `/cdn-cgi/`
  marker can drop from `self.content()` while `cf_clearance` was never
  issued ⇒ a body-mutated-but-unsolved page silently passes the retry
  gate.
- **Fix:** add `started_as_cf_challenge` (parallel to
  `started_as_dd_challenge`/`started_as_seccpt_challenge`) set at first
  detection; gate the retry on it.
- **Regression test:** CF nav whose body mutates but yields no
  `cf_clearance` ⇒ retry stays active / verdict ≠ Pass.

---

## CLASS D — Wired-but-unreachable success paths  (P1)

Code paths whose success branch can never execute given the upstream
structural gap — they make the engine *look* capable and defeat loose
tests.

### FP-D1 — Inc-8 DataDome self-solve window unreachable for etsy  (P1)
- **Where:** `page.rs:2224` (Inc-8 45 s self-solve poll), gated behind a
  pending GET nav branch.
- **Why false:** the captured etsy trace shows i.js **never sets** that
  pending GET nav; the function returns at `page.rs:~2111` *before*
  reaching `:2224`. The increment is real code that, for its target
  site, never runs.
- **Fix:** confirm the gating predicate against the live etsy trace;
  either re-gate Inc-8 on `started_as_dd_challenge` directly (not the
  pending-nav branch) or document it as reachable only for the
  homedepot-class DD flow.
- **Regression test:** etsy-shaped trace fixture ⇒ assert the Inc-8 poll
  is entered (currently it is not).

### FP-D2 — Cloudflare `cf_clearance`-in-jar success branches dead  (P1)
- **Where:** `page.rs:351-441` — the "cf_clearance already present /
  issued" success branches.
- **Why false:** nothing in the codebase ever produces `cf_clearance`
  (Class A — no orchestrator; Class E — Turnstile iframe never loaded),
  so these branches are structurally unreachable. The
  `cloudflare_udemy.rs` "scaffolding ran" assertion is too loose to
  catch it.
- **Fix:** keep the branches but add a hard test asserting that, given
  the current engine, `cf_clearance` is never produced — so the dead
  path is *documented as dead* until G-CF-1/2 land.
- **Regression test:** end-to-end CF fixture asserting `cf_clearance`
  absent ⇒ verdict `CfChallengeIncomplete` (ties to FP-B4).

### FP-D3 — `cookies_have_datadome` false-success  (P0/P1)
- **Where:** `crate::datadome_handler::cookies_have_datadome` (used by
  the Inc-8 break condition and the poll/retry).
- **False claim:** "a `datadome=` cookie ⇒ challenge solved."
- **Why false:** DataDome sets a `datadome=` cookie on **every** nav
  including the failing 403; presence ≠ solved. Break/poll can fire on a
  false success and report a pass.
- **Fix:** distinguish solved vs fail — check the post-cookie response is
  not the interstitial (status, body marker), or decode the cookie
  state, before treating it as success.
- **Regression test:** feed the fail-cookie + 403 body ⇒
  `cookies_have_datadome_solved == false`.

---

## CLASS E — Structural engine gap surfaced by FP analysis  (P1 — the headline)

### FP-E1 — Script-created cross-origin challenge iframe never loaded
- **Where:** `find_iframes` (`iframe.rs:255`) called only at build time
  (`page.rs:857 / 887 / ~3137`) over the parsed DOM; no post-JS rescan,
  no `createElement('iframe')`/`iframe.src=` hook. `dom_bootstrap.js`
  only fabricates a synthetic `contentWindow`.
- **Why it's an FP, not just a gap:** every DataDome/Cloudflare success
  path, retry primitive, and self-solve window downstream of "the
  challenge iframe runs" is **code that can never succeed** for
  etsy/tripadvisor/udemy — they read as elaborate capability but are
  unreachable. Cataloged here so the dead downstream paths are understood
  as *contingent on this fix*, not independently debuggable.
- **Fix (the single highest-leverage engine investment):** post-JS DOM
  re-scan + `createElement('iframe')`/`.src` interception that performs a
  real cross-origin fetch and executes the challenge document
  (DataDome `geo.captcha-delivery.com`, Cloudflare
  `challenges.cloudflare.com`) in a real child context.
- **Regression test:** a page that JS-injects a cross-origin
  `<iframe src>` post-load ⇒ assert the iframe document is fetched and
  its scripts execute (currently: synthetic shim only, no fetch).

---

## CLASS F — Tests that pass offline but the live path differs  (P2)

| ID | Where | The false comfort | Fix |
|---|---|---|---|
| FP-F1 | `kasada_session.rs` `rst`/`d`/`alignedWorkTime` | Synthetic guesses asserted "matches real ips.js" but only self-replay-tested | Label as unverified-vs-live; needs a live `/tl` differential capture, not a self-test |
| FP-F2 | `perimeterx_surface_parity.rs` | Green surface-parity ≠ passing PX (encrypted payload + behavioral + server-ML + TLS/IP never exercised; `chrome_130_macos` only, iOS consistency unverified) | Rename/doc the test as "surface parity only"; add the FP-B2 px-captcha render test as the real PX regression |
| FP-F3 | `VM_TRACE_FINDINGS` / `UNJZOMUY_INVESTIGATION` "5 sentinel throws = the bug" | Falsified by clean 80/80 sentinel + Phase 2 OUTCOME A | Mark the historical docs as **eliminated**; do not resurrect (already in master plan §6) |

---

## Suggested fix order (maximizes trustworthy measurement first)

1. **FP-B1 + FP-B2 + FP-B4** (P0): unify the classifier and size-gate
   strong markers — *without this every other result is unmeasurable.*
2. **FP-D3** (P0/P1): `cookies_have_datadome` solved-vs-fail — stops
   false passes in the DD retry.
3. **FP-C2** (P1): add `started_as_cf_challenge` — closes the one
   still-live doc-20 instance.
4. **FP-E1** (P1, big): the script-created cross-origin iframe
   loader/executor — unblocks the DataDome + Cloudflare class.
5. **FP-B3 / FP-D1 / FP-D2** (P1): thin-shell band, Inc-8 reachability,
   CF dead-path documentation.
6. **CLASS A + CLASS F** (P2): truth-in-labeling sweep — make the crate's
   advertised capability match the live path so the next session isn't
   misled again.

Every P0/P1 item ships as its own gate-checked commit per the project's
verify-don't-assume / revert-if-not-green discipline; the regression test
named with each item is the green bar.
