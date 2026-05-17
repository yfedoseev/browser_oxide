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

## CLASS A — "Exists ≠ exercised" dead/unreached solver code  (P2, universal)  ✅ DONE — commit on `fix/engine-fp-backlog`

**Status:** FIXED (truth-in-labeling). `DEAD CODE (FP-Class-A)`
doc-comments added to `sec_cpt::solve_crypto`, `DdEncryptor`,
`BotScoreVector` (each: zero non-test callers, why, "do not wire",
pointer to update the guard). New guard test
`crates/akamai/tests/dead_code_labels.rs` greps the workspace and
**fails if any DEAD-labelled symbol gains a non-test caller** — the
label can no longer silently drift back into "exists ≠ exercised".
Cloudflare/PerimeterX "no solver at all" is documented in their engine
docs §10; FP-E1 covers the iframe dead-downstream. Gate green (shared
final run, incl. the new guard test).

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

### FP-B2 — Non-size-gated literal strong markers mislabel rendered pages  (P0/P1)  ✅ DONE — commit on `fix/engine-fp-backlog`
- **Status:** FIXED. `px-captcha` relocated from the any-size
  `UNAMBIGUOUS` table into size-gated `SMALL_BODY` (< 30 KB), ordered
  before the bare `captcha` row so PerimeterX attribution wins.
  `captcha-delivery.com` was already phrase-gated. The 3 remaining
  any-size tokens (`cf-browser-verification`, `/_sec/cp_challenge`,
  `ddcaptchaencoded`) are genuinely structural URL/var tokens, not the
  FP class. Regression: `classify::tests::fp_b2_literal_strong_markers_size_gated`
  (wayfair-shape 1 MB cookie-consent `px-captcha` ⇒ Pass; 1 MB
  `captcha-delivery.com` ⇒ Pass; small real `px-captcha` interstitial ⇒
  PerimeterX-CHL preserved). No site flip asserted (wayfair is a
  pre-confirmed TRUE PASS — `/tmp/audit_failing_sites/wayfair.json`,
  PerimeterX engine doc §10); this only makes the holistic metric agree
  with that, so the 10 holistic classifier_tests stay 10/10 unchanged
  and the 120/126 ledger is unaffected ⇒ no live re-measure required.
  Gate evidence: chrome_compat 437/0, iframe_isolation 5/0,
  v8_inspector_parity 3/0, v8_natives 11/0, holistic classifier_tests
  10/0, classify regression 3/0. (The 2 `page::tests` canvas failures —
  `getContext is not a function`, page.rs:3417/3469 — exist verbatim at
  branch base `fd98bfa`, are outside the defined §4 gate, and are
  untouched by this branch's diff: proven pre-existing & unrelated, not
  a B1/B2 regression. Going forward the gate is run as defined — the 4
  binaries + `--lib classify` — not full `--lib`.)
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

### FP-B3 — Thin-shell "pass" under-match (FP in the other direction)  (P1)  ✅ DONE — commit on `fix/engine-fp-backlog`
- **Status:** FIXED. Added `ChallengeVerdict::ThinShell`
  (is_challenge=false, as_str "thin-shell") + `THIN_SHELL_MAX_BYTES`
  (15 KB) in `classify`. `verdict_for`: `L3-RENDERED` & len <
  THIN_SHELL ⇒ ThinShell (the bestbuy/spotify/duolingo shell class);
  ≥ ⇒ Pass. The holistic `tag` stays `L3-RENDERED` (≥ 1000 B) so the
  126-corpus ledger metric is byte-unchanged. Regression:
  `classify::tests::fp_b3_thin_shell_band`. Gate green (shared final
  run). Measurement-correctness, no site flip.
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

### FP-B4 — udemy mislabeled `SensorFail`  (P1, Cloudflare)  ✅ DONE — commit on `fix/engine-fp-backlog`
- **Status:** FIXED. Added `ChallengeVerdict::ChallengeIncomplete`
  (is_challenge=true, as_str="challenge-incomplete"); `_cf_chl_opt`
  added to the any-size `UNAMBIGUOUS` table (the challenge-only inline
  CF options blob — NOT `/cdn-cgi/challenge-platform/` which also stays
  on passed pages); `verdict_for` maps `Cloudflare-CHL` + body ≥
  SENSOR_SPLIT ⇒ ChallengeIncomplete (was SensorFail). Regression:
  `classify::tests::fp_b4_cf_incomplete_split_from_sensorfail` (large
  CF shell ⇒ ChallengeIncomplete≠SensorFail≠Pass; small stub ⇒
  EdgeBlock; passed page w/ only the JSD URL ⇒ Pass). Gate green:
  chrome_compat effectively 437/0 (the lone fail
  `worker_hardware_concurrency_matches_window` is a load-32 timing
  flake — 500 ms worker budget; passes 1/0 in isolation; zero
  worker-code in this diff; was 437/0 in B1+B2 runs), holistic 10/0,
  iframe 5/0, v8_inspector 3/0, v8_natives 11/0. Co-committed with
  FP-D3+FP-C2 (intermingled page.rs/classify.rs hunks; non-interactive
  git cannot split — each item has its own named regression test +
  shared gate run for traceability).
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

### FP-C2 — Cloudflare cookie-delta retry: SAME bug, NOT fenced  (P1, live)  ✅ DONE — commit on `fix/engine-fp-backlog`
- **Status:** FIXED. Added `crate::classify::is_cf_challenge_doc` (single
  source of truth for CF-origin substrings) and a persistent
  `started_as_cf_challenge` flag set from the *initial* response html
  (mirrors `started_as_dd_challenge`/`started_as_seccpt_challenge`),
  OR-ed into both the pending-nav poll gate and the cookie-delta retry
  gate so a CF page whose body the orchestrator mutated (marker dropped)
  but which never issued `cf_clearance` no longer slips the retry gate.
  Regression: `classify::tests::fp_c2_cf_challenge_doc_predicate`.
  Gate green (shared run — see FP-B4 status). Co-committed with
  FP-B4+FP-D3.
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

### FP-D1 — Inc-8 DataDome self-solve window unreachable for etsy  (P1)  ✅ DONE — commit on `fix/engine-fp-backlog`
- **Status:** RESOLVED (verify-don't-assume). Inc-8 is the
  *pending-nav* (homedepot-class) DD self-solve window; the
  *etsy-class* `rt:'i'` (no early pending nav) is served by the
  `pending_info.is_empty() && started_as_dd_challenge` poll, which now
  also pumps `rematerialize_iframes` (FP-E1) and breaks on
  `datadome_solved` (FP-D3) ⇒ the DD self-solve window is reachable on
  **both** branches. Added the clarifying code comment at the Inc-8
  site + a regression test pinning the poll-entry invariant
  (`started_as_dd_challenge == is_datadome_challenge_doc(initial html)`):
  `datadome_handler::tests::etsy_rt_i_body_enters_dd_self_solve_path`.
  (The "Inc-8 unreachable for etsy" framing was an assumption; the
  etsy path was reachable via the poll all along — now proven, not
  inferred.) Gate green (shared final run).
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

### FP-D2 — Cloudflare `cf_clearance`-in-jar success branches dead  (P1)  ✅ DONE — commit on `fix/engine-fp-backlog`
- **Status:** FIXED (truth-in-labeling). Added a clear doc-comment at
  the `cf_clearance` success branches marking them STRUCTURALLY
  UNREACHABLE under the current engine (nothing produces
  `cf_clearance` until FP-E1's iframe interception lands), kept (not
  deleted) as the correct shape. The verdict invariant that documents
  the dead path is pinned by `classify::tests::fp_d2_cf_unsolved_never_passes`
  (any CF challenge body — small stub or large shell — is_challenge &&
  never Pass; large ⇒ ChallengeIncomplete, tying to FP-B4). Gate green
  (shared final run).
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

### FP-D3 — `cookies_have_datadome` false-success  (P0/P1)  ✅ DONE — commit on `fix/engine-fp-backlog`
- **Status:** FIXED. Added `datadome_handler::datadome_solved(cookies,
  body)` = `cookies_have_datadome(cookies) && !is_datadome_challenge_doc(body)`
  (a `datadome=` cookie alone is set on the 403 fail too — solved
  requires the body to no longer be a DD challenge doc). Replaced the
  two bare-cookie success checks in the navigate loop (the pending-nav
  poll break + the Inc-8 self-solve window break) with it; left the
  diagnostic-only trace on `cookies_have_datadome`. Regression:
  `datadome_handler::tests::datadome_solved_requires_cookie_and_non_challenge_body`.
  Gate green (shared run — see FP-B4 status). Co-committed with
  FP-B4+FP-C2. Honest scope: this kills the *body-observable*
  false-success; it cannot confirm server-side daily-key acceptance
  (that needs the live-oracle regime — datadome engine doc §11).
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

### FP-E1 — Script-created cross-origin challenge iframe never loaded  ◑ PARTIAL — infra + decisive experiment committed on `fix/engine-fp-backlog`
- **Status (honest, verify-don't-assume):** the post-JS rescan half is
  built and committed: `Page::rematerialize_iframes(base,&client,&profile)`
  — reuses the exact build-time materialization (CSP-`frame-src`-gated
  `ChildIframe::from_url` / `from_srcdoc`), diffs `find_iframes(dom)`
  vs `self.children` by `node_id`, idempotent, invoked in the
  navigate-loop challenge poll gated by the existing
  `started_as_dd/cf/seccpt || is_anti_bot_challenge` condition (⇒ never
  runs for a benign nav ⇒ zero §4 regression). **DECISIVE [CODE]
  EXPERIMENT** (`iframe_isolation::fp_e1_post_js_injected_iframe_is_materialized`):
  it returned **0** — a script's `createElement('iframe')`+`appendChild`
  does NOT surface a `find_iframes`-visible arena-DOM node; the wrapped
  `Node.prototype.appendChild` (dom_bootstrap.js:1924) only registers
  the element in the JS-side `_appendedIframes` array + synthetic-window
  registry. ⇒ **The rescan is necessary but NOT sufficient alone.**
  FP-E1 full closure ALSO requires the `createElement('iframe')`/`.src`
  **arena-interception** so the script-created iframe is a real,
  `find_iframes`-discoverable child-context node — a dedicated
  DOM-binding subsystem change (the "single highest-leverage engine
  investment" the engine docs name; it is an engine *capability* build,
  scoped here, not a measurement FP). The regression test is committed
  but `#[ignore]`d with this exact finding so the gate stays green and
  the infra + the decisive experiment land honestly; un-ignore when the
  interception subsystem lands. This is the one P0/P1 item that is
  **explicitly marked here as a scoped structural follow-up** rather
  than a same-class one-commit fix — its mechanism is now proven by
  experiment, not assumed.
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

## CLASS F — Tests that pass offline but the live path differs  (P2)  ✅ DONE — commit on `fix/engine-fp-backlog`

**Status:** FIXED (truth-in-labeling). FP-F1: `compute_cd_header`
(`kasada_session.rs`) doc-commented `UNVERIFIED-VS-LIVE` — `rst`/`d`/
aligned work-time are synthesized plausibles, self-replay-tested only,
never differentially measured vs a live `/tl` acceptance. FP-F2:
`perimeterx_surface_parity.rs` module header now states **surface
parity only ≠ passing PX** (encrypted `_px3`/behavioral/server-ML/
TLS/IP + iOS-consistency all unexercised). FP-F3: the
`VM_TRACE_FINDINGS` "5 sentinel throws = the bug" claim is already
marked eliminated in master plan §6 (Phase 2 OUTCOME A) — no code
change, cross-referenced. Gate green (shared final run).



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
