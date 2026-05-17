# `docs/research/engines/` — Anti-Bot Engine Deep-Research Set

**Created:** 2026-05-16 · **Baseline git HEAD:** `fd98bfa` · **Method:**
five independent parallel research agents (one per engine) + orchestrator
synthesis, bound to the `00_INVENTORY_AND_METHOD.md` contract.

## Files

| File | What it is |
|---|---|
| `00_INVENTORY_AND_METHOD.md` | The binding contract: goal, mandatory §0–§12 template, rules of engagement, per-engine pointers, FP method |
| `akamai.md` | Akamai Bot Manager (11 sites) — 699 lines |
| `datadome.md` | DataDome (4 sites) — 702 lines |
| `cloudflare.md` | Cloudflare Bot Management (udemy +) — 734 lines |
| `perimeterx.md` | PerimeterX / HUMAN (wayfair) — 303 lines |
| `kasada.md` | Kasada (canadagoose/hyatt/realtor) — 580 lines |
| `99_CODE_FALSE_POSITIVES.md` | Consolidated, de-duplicated code-FP findings + concrete fixes + the regression test each needs |
| `README.md` | This index + the cross-engine synthesis |

Each engine doc follows the same §0–§12 structure (mechanism → our-code
reality → gap analysis → false-positive analysis → pass-guarantee plan →
cited sources), with `[MECH]` / `[CODE]` / `[HYP]` provenance tags.

---

## The single most important finding (independently found by 2 agents)

**Script-created cross-origin challenge iframes are NEVER fetched or
executed.** The DataDome agent and the Cloudflare agent — working
independently — root-caused the *same* structural blocker:

- `find_iframes` (`iframe.rs:255`) is called at exactly **3 static,
  build-time points** (`page.rs:857 / 887 / ~3137`), always over the
  *parsed* DOM. There is **no post-JS DOM re-scan** and **no
  `createElement('iframe')` / `iframe.src=` interception**.
- When a vendor script dynamically `appendChild`s
  `<iframe src="https://geo.captcha-delivery.com/…">` (DataDome) or
  `<iframe src="https://challenges.cloudflare.com/…">` (Cloudflare
  Turnstile) **after** build, the `dom_bootstrap.js` hook only
  fabricates a *synthetic* `contentWindow` / `window[N]` for
  fingerprint parity. **It never performs the HTTP fetch and never
  executes the challenge document's scripts/WASM.**

⇒ This one gap structurally gates **DataDome etsy/tripadvisor** *and*
**every modern Cloudflare Managed Challenge**, independent of IP, and is
the most plausible class-blocker for any vendor whose verification runs
in a script-injected child frame (likely PerimeterX PaH and parts of
Kasada too). It is the highest-leverage single engine investment in the
whole corpus. See `datadome.md §9 G1` and `cloudflare.md §9 G-CF-1`.

---

## Cross-engine synthesis matrix

| Engine | Sites (29-set) | Honest current state | The one blocker | Difficulty |
|---|---|---|---|---|
| **Akamai** | 11 | 10/11 were classifier FPs (render); homedepot flipped `b623d5d` (partial — post-sec-cpt intermediate page) | Phase-1 (TLS/JA4+H2) decides before JS; the only cross-site lever is **JA4-vs-UA coherence** (TLS pinned 147, UA 148) | Hard (homedepot Phase-5 L); JA4 fix is M |
| **DataDome** | 4 | leboncoin renders; yelp = human-captcha (not IP); etsy/tripadvisor blocked | **Script-created challenge iframe never loaded** (the headline finding) | L (irreducible) + a cheaper silent-pass M lever |
| **Cloudflare** | 1 (udemy) | No solver/orchestrator at all; udemy mislabeled "sensor-fail"; IP hypothesis unverified | Same iframe gap (Turnstile iframe never loaded) + no orchestrate/flow driver | L; needs the iframe fix first |
| **PerimeterX** | 1 (wayfair) | **wayfair is a TRUE PASS — pure classifier FP, decisively confirmed** (1.27 MB render, PX cookies accepted) | None for the 29-set; greenfield solver only if a PX site ever flips to blocking | N/A now (L if ever needed) |
| **Kasada** | 3 | Hardest; allow-but-blocked paradox REAL & settled (`b:1` is server ML, no single client lever); realm/sentinel line CLOSED | Behavioral-telemetry absence + JA4/H2-vs-UA + headless-GPU feed `b:1` | Hardest (no single-commit flip; needs live-oracle regime) |

## Cross-engine recurring themes (each detailed in `99_CODE_FALSE_POSITIVES.md`)

1. **"Exists ≠ exercised" dead-code FP — universal.** Every engine has a
   solver that is byte-verified-but-dead or absent: Akamai
   `sec_cpt::solve_crypto` (zero non-test callers) + `BotScoreVector` +
   v2 crypto; DataDome `DdEncryptor` (zero non-test callers); Cloudflare
   — no orchestrator at all, success branches wired-but-unreachable;
   PerimeterX — no solver at all (detection without solver =
   unactionable verdict); Kasada — Rust PoW wired but runs *in parallel*
   to ips.js self-solving in V8 (misleading-as-the-solver, plausible
   self-inflicted `b:1`). The crate's apparent capability massively
   exceeds the live path.
2. **The doc-20 mutable-state guard class.** Guards keyed off
   post-mutation `self.content()`: Akamai sec-cpt (now fenced behind a
   persistent `started_as_seccpt_challenge`) and **Cloudflare's
   cookie-delta retry (`page.rs:1836`) — still unfenced, no persistent
   `started_as_cf_challenge` flag exists**. Same anti-pattern, one site
   still exposed.
3. **Divergent classifiers ⇒ pass↔block disagreement.** `page.rs`
   (ungated strong markers), `holistic_sweep.rs:864 classify()`
   (<30 KB gate), and the vendor handlers (≤4 KB gate) use different
   markers and size thresholds for the *same* signal, producing
   inconsistent site counts and verdicts. The 29-set vs 126-corpus
   number confusion is downstream of this.
4. **Detection over-match residue + thin-shell under-match.** The typed
   `ChallengeVerdict` (`3739da9`) fixed the 12-site over-match, but
   non-size-gated literal strong markers (`px-captcha`,
   `captcha-delivery.com`) can still mislabel a 1 MB rendered page; and
   small "pass" bodies (bestbuy 7.8 KB, spotify 9.6 KB) may be thin
   shells mislabeled as full renders (FP in the other direction).
5. **False-success cookie signal.** `cookies_have_datadome` cannot
   distinguish a *solved* `datadome=` cookie from the *fail* cookie
   DataDome sets on every nav (incl. the 403) — poll/retry/Inc-8 break
   conditions can fire on a false success.

## How to use this set

- **Start here**, then read the engine doc for the vendor you're
  working. `99_CODE_FALSE_POSITIVES.md` is the actionable backlog.
- These docs are the new canonical engine reference; they consolidate
  and, where they conflict, **supersede** older
  `docs/research_2026_05_1{4,5,6}/` vendor docs (those remain as
  historical depth — esp. the 65–107 KB 05-14 dossiers for byte-level
  detail).
- Provenance discipline: a claim tagged `[MECH]` is cited mechanism
  fact; `[CODE]` is a file:line we read; `[HYP]` is a labeled
  hypothesis with its discriminating experiment. Do not promote `[HYP]`
  to fact without running the experiment.

## The honest pass-guarantee verdict

A *guaranteed* pass of all sites is **not one commit away** and the docs
say so plainly:

- **Achievable engine work that lifts the most:** (a) the script-created
  cross-origin iframe loader/executor (unblocks the DataDome + Cloudflare
  class), (b) JA4-vs-UA coherence (touches all 11 Akamai + Kasada edge),
  (c) the code-FP fixes in `99_…` (make verdicts trustworthy — without
  this we cannot even *measure* a pass).
- **Structurally not gate-verifiable offline:** DataDome daily-key,
  Kasada holistic `/tl` ML, Cloudflare PoW-vs-ASN — these need an
  explicitly-authorized **live-oracle dev regime** (captured daily
  challenge / differential `/tl` capture), because the mandatory
  network-free §4 gate cannot verify a daily-rotating-oracle flip.
- **Out of stealth scope (do not spend on):** yelp & the 5 captcha
  sites (human-interaction gate), wayfair (confirmed FP — zero work),
  douyin/wildberries (region-locked).

Net: the corpus splits into *engine-addressable-and-now-precisely-located*
(the iframe class + JA4 + FP fixes), *live-oracle-regime-required*
(DataDome/Kasada/CF deep), and *out-of-scope*. No fabricated guarantee;
the path is mapped and the next concrete move is unambiguous.
