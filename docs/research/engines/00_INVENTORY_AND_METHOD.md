# 00 — Engine Deep-Research: Inventory, Method & Contract

**Created:** 2026-05-16 · **Baseline git HEAD:** `fd98bfa` · **Owner:** orchestrator

## Goal (verbatim intent)

Produce a **comprehensive, deeply-analyzed reference doc per anti-bot
engine** — Akamai Bot Manager, DataDome, Cloudflare Bot Management,
PerimeterX/HUMAN, Kasada — that:

1. Consolidates **everything we already know** (our prior research + our
   code) so nothing is lost or rediscovered.
2. Adds **everything we are missing** via independent research: OSS
   projects, GitHub repos, technical articles, vendor docs, CTF/RE
   writeups, web intel, and any feasible local experiments.
3. Performs a rigorous **false-positive analysis of our own code** — both
   our *detection/classification* logic (do we mislabel pass↔block?) and
   our *solver* logic (is it correct, wired, and actually exercised?).
4. Is concrete enough that, combined, the docs form a **pass-guarantee
   playbook** for every site each engine guards.

Output lives in `docs/research/engines/<engine>.md` (one file per engine)
plus a synthesis index and a consolidated code-FP doc (orchestrator).

## Why this matters (load-bearing context)

`docs/research_2026_05_16/00_MASTER_PLAN.md` is the current canonical
state. Two findings make the FP analysis non-optional:

- **Phase 0.2 re-baseline:** 10/11 Akamai sites + macys (Kasada) +
  wayfair (PerimeterX) were **classifier false positives** — they
  actually render; our `is_anti_bot_challenge()` body-substring matcher
  over-matched. The "22 engine-addressable" framing was measurement
  error; true hard set ≈ 6.
- **Dead solver code:** DataDome `DdEncryptor` and Akamai
  `sec_cpt::solve_crypto` are byte-verified but have **zero callers** —
  the engine relies on the vendor bundle self-solving in V8. Research
  depth currently *exceeds* what is wired.

So "do we pass site X" has historically been corrupted by (a) detection
FPs and (b) solvers that exist on paper but are not in the live path.
Every engine doc must separate **mechanism truth** from **our code
reality** from **what is actually exercised at runtime**.

## Mandatory output template (every `<engine>.md` MUST follow this)

```
# <Engine> — Deep Engine Analysis (browser_oxide)

## 0. Executive summary & pass-guarantee thesis
    - 1 paragraph: how this engine decides bot vs human in 2026
    - The single highest-leverage gap blocking us
    - Honest verdict: which guarded sites we pass / block / FP today

## 1. Vendor surface & 2026 deployment
    - Product tiers, script names/paths, versions in the wild, which of
      our target sites use which tier

## 2. Detection pipeline — stage by stage
    - Edge/TLS/H2 → loader → fingerprint JS → behavioral → server ML
    - For each stage: signal collected, how scored, kill vs soft-score

## 3. Challenge / script anatomy (bytes & structure)
    - Interstitial body, obfuscation/VM, WASM, iframe topology, the
      exact success signal (cookie/header/postMessage)

## 4. Fingerprint / sensor payload — field by field
    - Every field we can enumerate, expected real-Chrome value, how a
      mismatch scores

## 5. Crypto / encoding
    - Key derivation, encryption, encoding, daily/seed rotation,
      replay/anti-replay, server-side TLS (JA3/JA4) cross-checks

## 6. Cookie & header lifecycle / state machine
    - Every cookie/header, valid→invalid transitions, what "solved"
      looks like on the wire

## 7. How OSS / commercial tools defeat it
    - Concrete repos/tools/articles WITH citations; what technique each
      uses; what is reproducible vs marketing; what is patched/dead

## 8. What browser_oxide does today (file:line evidence)
    - Exact code path from navigate() to the success signal
    - Mark each component: WIRED & exercised / wired-but-unreached /
      DEAD CODE (zero callers) / MISSING

## 9. GAP ANALYSIS — what we are missing (ranked, concrete)
    - Each gap: evidence (file:line or source URL), blast radius
      (which sites), difficulty, risk, the concrete fix

## 10. FALSE-POSITIVE ANALYSIS of our code
    - 10a. Detection FPs: where classification mislabels (over/under
      match, stale markers, thin-shell "pass", challenge-as-pass)
    - 10b. Solver/logic FPs: dead code, wrong assumptions, guards keyed
      off mutable state, byte-verified-but-never-run, assertions that
      pass offline but the live path differs
    - Each item: file:line, the false claim, why it's false, the test
      that would have caught it

## 11. The concrete pass-guarantee plan
    - Ordered steps to actually pass every site this engine guards
    - The verification regime (what proves it — and note where the
      network-free §4 gate structurally CANNOT verify)

## 12. Sources & experiments
    - Every external source (URL + accessed date) and every local
      experiment run (command + result), so claims are auditable
```

## Rules of engagement (all agents)

1. **Verify-don't-assume.** Distinguish *mechanism fact* (cited),
   *our-code fact* (file:line you actually read), and *hypothesis*
   (labeled as such). Never present inference as measurement.
2. **Skeptical of our own docs.** Prior `docs/research_2026_05_1{4,5,6}/`
   files are inputs, not gospel — several older close-outs are already
   falsified (master plan §1). If a prior doc conflicts with current
   code, the code wins; say so.
3. **Cite everything external.** URL + what it claims + accessed date.
   Prefer primary (vendor docs, deobfuscated script, RE writeups,
   maintained OSS) over blog spam. Note when a tool's claim is
   unreproducible or the technique is patched/dead.
4. **Dead-code test.** For every solver/helper in our code, grep for
   callers. If zero non-test callers ⇒ label **DEAD CODE** explicitly in
   §8 and §10b. This is the single most important FP check.
5. **Detection-FP test.** Trace what string/condition flips our
   "blocked" verdict and ask: would a *rendered, benign* page also trip
   it? Would a real challenge slip past as "pass" (thin shell ≥ size
   threshold, challenge marker our matcher misses)?
6. **Experiments:** prefer static source reading + `grep` + WebSearch/
   WebFetch. Heavy `cargo` runs serialize on a build lock and network
   tests are `#[ignore]` — only run a *targeted* test if it is decisive
   and cheap; never block the deliverable on a long sweep. Record any
   command + result in §12.
7. **Honest negative results are valuable.** "We do not have X / X is
   dead / this OSS claim does not reproduce" is a first-class finding.
8. Write **one file only** (your assigned `<engine>.md`). Do not edit
   other agents' files or this inventory.

## Per-engine starting pointers (non-exhaustive — extend them)

### Akamai Bot Manager → `docs/research/engines/akamai.md`
- Sites: bestbuy, costco, disneyplus, expedia, hm, homedepot, hulu,
  uniqlo, walmart, washingtonpost, weather (11). homedepot = sec-cpt PoW
  (recently flipped, `b623d5d`); 10 others were classifier FPs (render).
- Our code: `crates/akamai/src/` — `v3_payload.rs` (451), `sec_cpt.rs`
  (195; `solve_crypto` @ :80 — **check callers**), `crypto.rs` (591),
  `session.rs` (`_abck` state machine, 307), `payload.rs`, `tea_cbc.rs`,
  `drain.rs`, `lib.rs` (834; fileHash registry ~:229-252).
  Live path: `page.rs:450 handle_akamai_flow`, sec-cpt guard ~:459-473
  (keyed off `self.content()` — known mutable-state hazard, see §10b),
  `started_as_seccpt_challenge` @ `page.rs:1432`.
- Prior research: `docs/research_2026_05_14/02_AKAMAI.md` (65 KB),
  `10_AKAMAI_V3_ENVELOPE_DEEP`, `docs/AKAMAI_BMP_V13_FIELD_ENCODING…`,
  `docs/RESEARCH_AKAMAI_BMP_BYPASS_2026_04_29.md`,
  `docs/universal_engine/site_debugging/{homedepot,adidas}_akamai_bmp_v3.md`.

### DataDome → `docs/research/engines/datadome.md`
- Sites: etsy, leboncoin, tripadvisor, yelp (4). leboncoin passes
  (renders); etsy/tripadvisor = WASM-iframe daily-key L endgame; yelp =
  interactive-captcha class (NOT IP ban — master plan §1.6).
- Our code: `crates/browser/src/datadome_handler.rs` (354),
  `crates/akamai/src/datadome_crypto.rs` (401; `DdEncryptor` @ :163 —
  **byte-verified but zero non-test callers, confirm**). Live path:
  `page.rs` `detect_datadome_interstitial` ~:1247, `started_as_dd_challenge`
  @ :1420, Inc 8 self-solve window @ :2224.
- Prior research: `docs/research_2026_05_14/03_DATADOME.md` (107 KB —
  the deepest single doc), `18_DATADOME_ENCRYPTION_REFERENCE`,
  `docs/RESEARCH_DATADOME_BYPASS_2026_05_10.md`,
  `docs/W6a_DATADOME_PROBE_GAP_MATRIX_2026_05_10.md`,
  `docs/LEBONCOIN_ANDROID_DATADOME_2026_05_12.md`.

### Cloudflare Bot Management → `docs/research/engines/cloudflare.md`
- Sites: udemy (1 in the 29-set; economist/quora-iOS in wider corpus).
  "datacenter-ASN IP block" is an **unverified hypothesis** (no clean
  nocdp hard-403 capture) — treat engine-addressable.
- Our code: `crates/stealth/src/cloudflare.rs`, CF flow in
  `page.rs` ~:301/:351-441 (`cf_clearance` polling — orchestrator
  dependency; **check whether the orchestrator exists/is wired**).
- Prior research: `docs/research_2026_05_14/04_CLOUDFLARE.md` (60 KB),
  `docs/RESEARCH_CLOUDFLARE_BYPASS_2026_05_10.md`,
  `docs/W7_CLOUDFLARE_V1_2026_05_10.md`. (Thinner than Akamai/DD/Kasada
  — this agent must do the most external research.)

### PerimeterX / HUMAN → `docs/research/engines/perimeterx.md`
- Sites: wayfair (1). wayfair was a classifier FP (renders 1.3 MB) —
  but **we have ZERO solver code** (only `px-captcha` substring detect
  at `page.rs:181`). Greenfield.
- Our code: detection-only. `crates/browser/tests/perimeterx_surface_parity.rs`.
- Prior research: `docs/research_2026_05_14/05_PERIMETERX.md` (67 KB).
  This agent must establish: is wayfair actually a problem, or purely an
  FP? And design the greenfield `_px3`/PoW solver if needed.

### Kasada → `docs/research/engines/kasada.md`
- Sites: canadagoose, hyatt, realtor (macys was FP/renders). The
  hardest. Allow-but-blocked paradox is REAL and settled
  (master plan §8.5 Phase 0.3): client SDK says `action:"allow"` with
  `bot1225.b:1` set, server 429s anyway ⇒ server ML scoring `b:1`, no
  single client lever. Phase 2 OUTCOME A: realm/sentinel/identity line
  CLOSED as not-the-bug — **do not resume the realm hunt**.
- Our code: `crates/stealth/src/kasada.rs` (357),
  `crates/net/src/kasada_session.rs` (538),
  `crates/browser/tests/{kasada_identity_decisive,tier0_kasada}.rs`.
- Prior research: `docs/research_2026_05_16/04_KASADA_CONSOLIDATED…`
  (THE authoritative Kasada doc — read first),
  `docs/research_2026_05_14/01_KASADA.md` (65 KB), `09_KASADA_DEEP`,
  `docs/research_2026_05_15/{26,27}_*REALM*`, `kasada_ips_analysis/`.
  Net new value for this agent: the `b:1` derivation (what accumulated
  signal computes it) + the decisive differential-identity experiment
  design in `04 §(f)` — refine it, don't re-chase closed lines.

## Shared known FP/code-reality findings (seed for §10 — verify & extend)

- `is_anti_bot_challenge()` `page.rs:263` + matcher `page.rs:175-208`:
  body-substring over-match (`bm_sz`, `captcha-delivery.com`, etc.) on
  benign pages ⇒ historical false "blocked". Typed `ChallengeVerdict`
  (`page.rs:120-291`, commit `3739da9`) tightened it — verify it
  actually fixed the 12 FP sites and find residual over/under-match.
- `holistic_sweep.rs:864 classify()` and `audit_failing_sites.rs` use
  *different* classifiers than `page.rs` — quoting the wrong one gives
  inconsistent site counts (master plan §reconciliation). Document the
  divergence per engine.
- "thin-shell pass": `pass` = rendered + no challenge marker on a
  1-iteration nav; small-body passes (bestbuy 7.8 KB, spotify 9.6 KB)
  may be shells, not real content — an FP in the *other* direction.
- Akamai sec-cpt guard keyed off `self.content()` (post-mutation DOM) —
  the doc-20 bug class; confirm Inc 7 `started_as_seccpt_challenge`
  fully closed it and no analogous mutable-state guard remains.
- DataDome `DdEncryptor` / Akamai `solve_crypto`: byte-verified by unit
  tests but **not in the live navigate() path** — the canonical
  "exists ≠ exercised" FP.

## Synthesis (orchestrator, after agents return)
- `docs/research/engines/README.md` — index + cross-engine matrix
- `docs/research/engines/99_CODE_FALSE_POSITIVES.md` — consolidated,
  de-duplicated code-FP findings with concrete fixes + the regression
  test each needs.
