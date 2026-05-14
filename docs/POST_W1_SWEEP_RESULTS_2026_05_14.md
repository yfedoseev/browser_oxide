# Post-W1 Sweep Results — 2026-05-14

Two-iteration sweep verifying (1) the Akamai `_abck` auto-POST recovery in
`crates/browser/src/page.rs::handle_akamai_flow`, and (2) the classifier
false-positive fix in `crates/browser/tests/holistic_sweep.rs::classify`.

## Headline

| Profile                  | L3       | vs baseline 05-13 | vs pre-classifier today |
| ------------------------ | -------- | ----------------- | ----------------------- |
| chrome_130_macos         | 115      | +2                | +1                      |
| pixel_9_pro_chrome_147   | 119      | +4                | +6                      |
| iphone_15_pro_safari_18  | 115      | 0                 | +2                      |
| firefox_135_macos        | 113      | +1                | +4                      |
| **Routing union**        | **121/126** | **+1**         | **+2**                  |

Baseline 05-13 reference: `docs/CHROME_148_SWEEP_RESULTS_2026_05_13.md`
(union 120/126).

## What landed

### 1. Akamai POST gate — `crates/browser/src/page.rs`

`handle_akamai_flow` now short-circuits on `NeedsSensor` without POSTing.
Background:

- W1.3 (`crates/akamai/src/session.rs`) rewrote the `_abck` parser using
  Hyper-SDK `IsCookieValid` semantics — slot 1 is a stop-signal threshold,
  slot 3 is invalidation. Pre-fix the canonical first response
  `<hex>~-1~<blob>~-1~-1~-1~-1~-1` was silently mapped to `Favorable` and
  we never POSTed.
- With the parser fix wired to the existing `build_v2_bestbuy`
  envelope (static DalphanDev seeds `3_289_904` / `3_683_632`), Akamai's
  2025+ detector flagged the payload as bot-shaped, escalating
  homedepot / macys / bestbuy-FF / medium-iOS L3 → Akamai-CHL in the
  post-W1.3 sweep.
- The parser is correct; the envelope is not. Until W2.3 ships the
  per-session `bm_sz`-seeded v3 envelope, the POST is gated off via an
  early-return. Re-enabling is a ~15-line restoration.

Cost: bestbuy-Pixel-Android (which had accepted the v2 POST on that one
profile) reverts to Akamai-CHL.
Recovery: homedepot recovers on multiple profiles; reuters/medium are
unaffected because they were never on the POST path.

### 2. Classifier false-positive — `crates/browser/tests/holistic_sweep.rs`

The `classify()` function's interstitial-title check had no body-size
gate, so English phrases that legitimately appear in rendered content
("just a moment", "checking your browser", "press &amp; hold", etc.)
triggered CHL classifications. Split into:

- **`unambiguous_titles`** (fires at any size): CSS classes / URL paths /
  encoded variables that don't legitimately appear in rendered content —
  `cf-browser-verification`, `/_sec/cp_challenge`, `ddcaptchaencoded`,
  `px-captcha`.
- **`phrase_titles`** (gated by 30 KB threshold): English phrases that
  can appear in body text — `just a moment`, `checking your browser`,
  `captcha-delivery.com`, `press &amp; hold`, `pardon our interruption`.

Two new regression tests cover the reuters-style case (1.1 MB body
containing "just a moment" must classify as `L3-RENDERED`). All 10
classifier unit tests pass.

## Key flips

- **reuters**: 4/4 profiles flipped CF-CHL → L3-RENDERED.
  Body 1.1 MB on all profiles — verified via curl from this IP that
  reuters is actually DataDome-protected; the engine reaches a fully
  rendered page. The "CF-CHL" classification was a phrase-title trip in
  legitimate news copy.
- **udemy**: 3/4 profiles flipped CF-CHL → L3-RENDERED.
  chrome / pixel / firefox bodies are 476 KB (rendered, phrase-title
  false-positive). iPhone body is 5822 B (genuine CF interstitial; still
  correctly classified CHL).
- **wildberries pixel**: 1518 B response (marginal — just above the
  1000 B THIN threshold) without captcha markers. Classifier records L3;
  signal quality borderline.

## Universal blocks (5, down from 7)

| Site         | Vendor    | Status                                              |
| ------------ | --------- | --------------------------------------------------- |
| canadagoose  | Kasada    | W1.1 `_buildRemoteRealm` memoization insufficient   |
| hyatt        | Kasada    | Same root cause                                     |
| realtor      | Kasada    | Same root cause                                     |
| douyin       | Regional  | Out of scope (Chinese captcha + regional ML)        |
| homedepot    | Akamai    | Cost of POST-disable until W2.3 v3 envelope         |

reuters / udemy / wildberries / leboncoin all left the universal-block
set this iteration.

## Open work

1. **Kasada Patch #2 + #3** (next session): iframe `getOwnPropertyDescriptor`
   Proxy descriptor memoization + `_defProtoMethod` idempotency guard per
   `docs/research_2026_05_14/01_KASADA.md` §1.4. W1.1's realm cache wasn't
   the sentinel-loss site — these two are the remaining candidates.
2. **W2.3 v3 envelope** (re-enables Akamai POST): per-session `bm_sz`-seeded
   shuffle + substitute seeds. ~150 LOC per `docs/research_2026_05_14/02_AKAMAI.md` §3.
3. **fetch_one inline classifier** (lines 64-115 of holistic_sweep.rs)
   has the same unambiguous-vs-phrase issue. Single-site tests using the
   `site!()` macro will still false-positive until mirrored.
4. **Variance characterization** (PLAN.md §6 W4.3): 5-run sweep tool so
   ±2 per-profile noise stops contaminating diagnosis.

## Files touched this iteration

- `crates/browser/src/page.rs` — `handle_akamai_flow` early-return on
  `NeedsSensor`.
- `crates/browser/tests/holistic_sweep.rs` — `classify()` split into
  unambiguous / phrase markers + 2 new regression tests.
- `docs/POST_W1_SWEEP_RESULTS_2026_05_14.md` — this document.

Prior session's W1.1–W1.10 patches (in 8 other files) remain uncommitted
and are not part of this commit's scope.
