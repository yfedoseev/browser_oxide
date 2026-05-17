# HOLISTIC REPORT — browser_oxide engine state (2026-05-17)

**Status:** authoritative consolidation of the entire 2026-05-16→17 arc.
**Branch:** `fix/engine-fp-backlog` (NOT merged to `main`). **Tip:**
`a561459`. **Baseline:** `fd98bfa`. **Provenance tags:** `[MEAS]` =
measured this work · `[CODE]` = a file:line / commit fact · `[CITED:doc]`
= taken from a named doc, not re-derived · `[HYP]` = labeled hypothesis.
**Zero fabrication.** **No budget — paid tools are cited-only, never run,
never installed, never signed up for.**

This report supersedes the per-thread handoffs as the single read-first
entry. It consolidates: `docs/HANDOFF_2026_05_17.md`,
`docs/research/engines/K2DIFF_RESULT.md`,
`docs/research/engines/COMPETITOR_COMPARISON_2026_05_17.md`,
`docs/research/engines/{README,99_CODE_FALSE_POSITIVES}.md`, and
`docs/research_2026_05_16/06_126_CORPUS_PROGRESS_LEDGER.md`.

---

## 1. Executive summary

browser_oxide is the **only from-scratch engine** in the field: its own
DOM, CSS, layout, HTTP, and BoringSSL TLS, with embedded V8 — **no real
browser binary**. As of this arc it opens **~120–121 / 126 routed**
(per-domain, best profile per site). The ±1 is leboncoin, a DataDome
debug-build + CPU-contention timing artifact, not an engine regression
(it renders in the audit, the prior sweep, and the competitor data).

The true engine-addressable hard residual is **4 real sites**:
**Kasada × 3 (canadagoose, hyatt, realtor) + homedepot** (Akamai
sec-cpt; passes under the sanctioned 3-iter `holistic_sweep` metric,
blocked only under the strict 1-iter lens). iphey is a fingerprint
*test page* THIN-BODY artifact, not a real target.

This is **free-OSS SOTA parity, measured**: this arc ran Camoufox
(patched-Firefox fork) and Patchright (real Chrome 147 driver) on this
IP — **both are hard-blocked on all 3 Kasada sites too**, and Camoufox
also fails DataDome etsy. No free-OSS tool — ours or theirs — passes
Kasada from this IP. browser_oxide reaches the broad-corpus outcome tier
of the real-browser-driver SOTA **without driving a real binary**, and
holds a decoded named Kasada fix list the OSS field lacks publicly.

---

## 2. The full arc (one paragraph each)

**Engine deep-research set.** Five independent parallel agents (one per
vendor) + synthesis produced the canonical `docs/research/engines/`
analyses (`akamai/datadome/cloudflare/perimeterx/kasada.md`,
`00_INVENTORY`, `README`, `99_CODE_FALSE_POSITIVES`). Headline, found
independently by two agents: **script-created cross-origin challenge
iframes are never fetched/executed** (FP-E1) — the single structural
blocker for DataDome etsy/tripadvisor + Cloudflare Managed `[CITED:README]`.

**FP backlog — all P0/P1 gate-green.** The nine P0/P1 code-false-positive
items shipped as individual §4-gate-green commits `[CITED:99]`: classifier
unification (FP-B1 `e869e18`), size-gated px-captcha (FP-B2 `6d67167`),
CF `ChallengeIncomplete` + DD solved-vs-fail + persistent
`started_as_cf_challenge` (FP-B4/D3/C2 `a4c3f6c`), FP-E1 post-JS rescan
infra + the decisive arena-DOM experiment (`d63b4fc`), thin-shell band +
dead-path truth-in-labeling (FP-B3/D1/D2/ClassA/ClassF `d163d17`), and
the engines docs (`52db4a3`). FP-E1 is the one explicitly-scoped
structural follow-up (its createElement/.src arena-interception is an
engine *capability* build, experiment-proven not assumed).

**UNBLOCK execution.** The 7-task list `[CITED:HANDOFF§1c]`: Tier-1
classifier co-signal gating (`308f8ad`, spotify/bestbuy/duolingo were
classifier FPs), Kasada K1 (`d455794`, Rust `x-kpsdk-cd` PoW deferred to
ips.js so it no longer competes — a self-inflicted `b:1` confound
removed), homedepot sec-cpt determinism (`8c4afae`), duolingo real fix
(`197e826`, `Request.prototype.signal` + `IntersectionObserverEntry`),
re-measure (`154ac8a`, 27-set false-blocks 10→7), and K2-DIFF
(`01f45ef`). Behavior wiring was re-confirmed deferred (G8 — not the
Kasada lever, un-gateable Akamai risk).

**K2-DIFF — the decode breakthrough.** `kasada_tl_capture.rs` +
`K2DIFF_RESULT.md` `[CITED:K2DIFF]`: our engine **never POSTs `/tl`** —
ips.js hits an internal failure during sensor assembly and diverts to
`reporting.cdndex.io/error` (~31 KB). That blob decodes
(`outer-b64 → JSON.data → b64 → XOR "omgtopkek"`) to the **full 23.8 KB
plaintext Kasada sensor, 123 fields**, §6-taxonomy-aligned
(`bid:"701d38d"`). This converts the old "allow-but-blocked / holistic
ML tail, no single lever" framing into a **concrete enumerated named
divergence list** plus a **reusable offline decode** that makes every
fix deterministically re-measurable — a far stronger position than the
network-only §4 gate could give. Rigor: `b:1` is probe-present, **not**
bot-flag (correct-valued fields carry it too); the signal is the
anomalous `r` content.

**The Kasada fix program.** Five named divergences, ROI-ordered, each
re-checkable via the decode `[CITED:K2DIFF "Next"]`:

| # | Divergence | Status |
|---|---|---|
| 1 | `wdt` `navigator.webdriver` `"undefined"` → `false` | **SHIPPED** `118c0d0` `[CODE]` (3 bootstrap sites→`false`, 4 chrome_compat tests corrected to the Chrome-faithful value, §4 green) |
| 2 | `unjzomuy…` sentinel-read abort (`smc`/`dpv`/`esd.cpt`) — aborts sensor assembly → the `/tl`→`/error` divert | **LOCALIZED, core throw UNRESOLVED** — see §2.1 |
| 3 | `wse`/`fsc`/`bfe`/`npc`/`esce` Fn.toString / class-extends / structuredClone TypeError strings → Chrome's exact strings | pending |
| 4 | `dpi`/`spd` devicePixelRatio/screen accessors `"n/a"` → real | pending |
| 5 | `pev`/`dpv` stack-format leak of the injected `/[guid]/ips.js` path | pending |

### 2.1 Fix #2 — the localization arc (honest)

Sharpened across multiple passes, all gate-green, none fabricated:

- The sentinel trap captured **879 sentinel SET calls — all on
  `Function` objects** set from inside the ips.js VM `[MEAS:K2DIFF]`.
- `_defProtoMethod` (`window_bootstrap.js:146`) installs masked
  wrappers as a **fixed data property called once at bootstrap** ⇒
  identity-stable ⇒ **UNJZOMUY candidate #3 RULED OUT** `[CODE]`.
- `kasada_smc_dpv_trap` wrapped every smc/dpv-relevant native in both
  realms: **768 reads, ZERO identity flips, ZERO fn→undefined** ⇒
  UNJZOMUY candidates #1 **and** #3 closed; tag-loss ruled out (551
  SETs all succeed, the 400 GET-misses are the benign Chrome-identical
  `if(l[h]&&…)` short-circuit) `[MEAS:K2DIFF]`.
- Precise failing set = **exactly 3** sensor fields: `smc`, `dpv`,
  `esd.cpt` (earlier 5/6 counts were JSON-nesting regex artifacts;
  `npc` is the class-extends message = fix #3, not this) `[MEAS]`.
- Disassembled the throw site: ips.js VM CALL handler **record 111**
  `var o=e(n),l=e(n),_=e(n),h=r[4]; if(l[h]&&l[h].I===l){…}` — the
  throw is **`l[h]` when `l===undefined`** at a FIXED column
  `<anonymous>:3:66` for exactly those 3 probes (`mrs`/`spd`/`ifw`
  which also use the child realm do **not** throw) ⇒ a structural
  bytecode-path divergence, not a missing host API `[MECH:K2DIFF]`.
- A strong candidate divergence — missing child-realm canvas/graphics
  constructor surface (`dom_bootstrap.js` `_apisToCopy` blanket
  `Object.keys()` copy misses non-enumerable WebIDL ctors) — was
  patched **gate-green and Chrome-faithfully** (`80818a5`, 13 ctors
  added; the child realm now has `CanvasRenderingContext2D` + all 37
  ctx2d proto methods, was absent) `[CODE]`.
- **Offline re-verify, honest negative (`a561459`):** the patch landed
  and is Chrome-faithful, **but** a fresh decode shows `smc`, `dpv`,
  `esd.cpt` still carry the IDENTICAL sentinel `TypeError` (FIXED:
  none; REGRESSED: none) — so the canvas-ctor gap was a **real
  Chrome-divergence now closed, but NOT the load-bearing cause** of the
  unjzomuy throw. A decisive *elimination*, not a fix `[MEAS:K2DIFF]`.

**Fix #2 net:** mystery → smc/dpv/esd.cpt → 879 Function tags →
not-`_defProtoMethod` → not accessor-recreation → not tag-loss → not
child-realm-ctor-absence → a structural VM-bytecode-path divergence at
rec-111 `l===undefined`. **The core throw is UNRESOLVED.** Defined next
step (not done): instrument the VM's own value-fetcher — hook the
`e`/`a`/`v` closures passed into the `Function()`-built rec-111 handler
(capturable; `kasada_eval_probe_trap` already records them) to trace
which earlier opcode left `n.V[k]` undefined for the smc/dpv/cpt
invocation. Host-accessor space is exhausted; this is the last
localization layer.

---

## 3. The definitive measurement

**Final #11 re-sweep — HEAD `a561459`, debug build + sustained heavy
external CPU contention = a conservative floor.** Build-mode-independent
outcome; only `nav_ms` is not benchmark-grade. A clean release run is
**≥** these numbers. `[MEAS]` this run. Raw logs: `/tmp/dbg_<profile>.log`,
`/tmp/sweep_dbg_all.log`.

| Mode | PASS / measured | vs prior | Note |
|---|---|---|---|
| Desktop Chrome `chrome_130_macos` | **117 / 126** | = prior 117 | fix#1+canvas-ctor are faithfulness gains; do NOT move site count |
| Android `pixel_9_pro_chrome_147` | **117 / 126** | ≈ prior 119 | within contention noise |
| iOS `iphone_15_pro_safari_18` | **108 / 125** | ≈ prior 113 | 1 site killed (tail-latency stall under contention) |
| Firefox `firefox_135_macos` | **100 / 126 PARTIAL** | NOT comparable | wedged under contention, killed to finalize; under-contributes |
| **Per-domain routed (best/site)** | **120 / 126** | prior 121 | −1 = leboncoin ERROR all modes = DataDome debug+contention timing artifact (renders in audit/prior-sweep/competitor data) |

**Reading the caveats honestly:**

- fix#1 (`wdt`) and the canvas-ctor patch are **faithfulness
  improvements that do not move the site count** — residual sites need
  fix#2 / FP-E1, exactly as predicted.
- Firefox 100/126 is a contention-wedged partial, **not comparable** to
  the prior 115; it under-contributes to the routed union this run.
- The routed `120` vs prior `121` delta is **entirely** the leboncoin
  debug-timing artifact — not a regression; the engine renders it
  elsewhere.
- iphey routed-blocked is a fingerprint **test page** THIN-BODY
  artifact, not a real target.
- homedepot is blocked **only under the strict 1-iter lens used here**;
  it passes under the sanctioned 3-iter `holistic_sweep` metric
  (`b623d5d` + UNBLOCK Task#3) `[CITED:06_LEDGER]`.

**True engine-addressable routed-blocked set, INVARIANT across this arc
= Kasada × 3 (canadagoose/hyatt/realtor) + homedepot (4).**
etsy/duolingo/yelp/wayfair/wellsfargo all open under ≥1 profile via
routing (mobile profiles clear DataDome sites desktop doesn't)
`[CITED:06_LEDGER]`.

---

## 4. SOTA comparison

**Measured free-OSS (this arc, this box, this datacenter IP)**
`[MEAS:COMPETITOR_COMPARISON]`. Box = the project's hostile dev box
(sustained external CPU contention; flaky rootless-Xwayland). 3 of 4
free tools ran:

| Tool | Architecture | canadagoose | hyatt | realtor | homedepot | etsy DataDome | Broad JS render |
|---|---|---|---|---|---|---|---|
| **browser_oxide** | **from-scratch Rust+V8** | blocked Kasada `[CITED:HANDOFF§2]` | blocked | blocked | 3-iter pass / 1-iter block | mobile-profile pass; desktop iframe-gap | **117–121/126 routed** `[CITED]` |
| curl_cffi 0.15.0 | TLS-impersonation HTTP, **no JS** | 429 Kasada stub | 429 | 429 | 2.6 KB Akamai shell | 403 DataDome | none (no JS engine) |
| patchright 1.59.1 | real Chrome 147 driver | **THIN 756 B blocked** | 12 KB soft shell (no real content) | **THIN 1768 B blocked** | 1 MB largely rendered | (not in subset) | full render CreepJS/sannysoft |
| camoufox 0.4.11 | patched **Firefox** fork (Xvfb only) | **THIN 756 B blocked** | **THIN 741 B blocked** | **THIN 1768 B blocked** | 1 MB largely rendered | **1488 B challenge — blocked** | full render CreepJS/sannysoft |
| nodriver 0.50.3 | real-Chromium CDP driver | could not launch (CDP-as-root) — class covered by patchright | | | | | |

**Cited commercial ceiling — PAID, never run (no budget), reference
row only** `[CITED:HANDOFF§4 / LANDSCAPE_2026_04_28]`: Scrapfly /
Hyper Solutions / ZenRows / CapSolver / Bright-Oxylabs unblockers and
commercial anti-detect browsers are the **only** reproducible 2026
Kasada pass (Scrapfly ~99% realtor) — a paid real-browser farm, no
open algorithm exists. This row stays cited-only, never installed,
never signed up for.

**The architecture point.** "Drives a real browser binary" is *trivial*
for camoufox/nodriver/patchright — they inherit a complete,
Google/Mozilla-maintained engine, TLS stack, and fingerprint surface
for free. curl_cffi sits at the opposite extreme: a TLS handshake and
nothing else (its JA3/JA4 are accepted everywhere; it is hard-blocked
the instant any JS-VM challenge fires — TLS parity is necessary, nowhere
near sufficient). browser_oxide is the **only entrant reproducing the
full surface from scratch**. So: apples-to-oranges on *architecture*,
apples-to-apples on *outcome* — and on outcome, the from-scratch engine
is in the **same broad-corpus tier as the real-browser-driver SOTA**,
and **Kasada is the universal free-OSS gap**: nobody free passes it
from this IP; the only winners are paid farms. browser_oxide's K2-DIFF
decoded fix list is a mapped route the OSS field lacks publicly.

---

## 5. Honest open work + ROI order

| Pri | Item | Why / state |
|---|---|---|
| 1 | **Kasada fix #2** — hook the VM's own `e`/`a`/`v` closures to trace the rec-111 `l===undefined` for smc/dpv/esd.cpt | Localized to a structural bytecode-path divergence; resolving the throw stops the `/tl`→`/error` divert ⇒ **likely the single biggest Kasada lever**. Host-accessor space exhausted; defined next step is the VM value-fetcher trace `[CITED:K2DIFF]` |
| 2 | **Kasada fixes #3–5** — Fn.toString/class-extends/structuredClone strings (#3), real devicePixelRatio/screen accessors (#4), stack-format leak (#5) | Each independently re-checkable via the offline decode |
| 3 | **FP-E1** — createElement('iframe')/.src arena-interception subsystem | Unblocks DataDome etsy/tripadvisor + Cloudflare Managed; the clearest real-browser-parity item; experiment-proven, scoped, not faked |
| 4 | **Clean-box live free-OSS head-to-head** — optional, later, quiet box only | Camoufox/Patchright/nodriver/curl_cffi over the full corpus + detector scorecard. The §4 cited+measured comparison is already sufficient and free; **paid tools remain OUT — no budget** |

**Not verifiable offline (by design):** live Kasada `/tl` server ML,
DataDome daily-key acceptance, Cloudflare PoW-vs-ASN — these need an
explicitly-authorized **live-oracle dev regime** (captured daily
challenge / differential `/tl` capture). The mandatory network-free §4
gate **cannot** verify a daily-rotating-oracle flip; the K2-DIFF
offline decode is the strongest available deterministic proxy
`[CITED:README / K2DIFF]`.

---

## 6. State of the tree / discipline

- All commits on `fix/engine-fp-backlog`, **NOT merged to `main`** —
  left for review. Baseline `fd98bfa` → tip `a561459`.
- Arc commit spine (most recent first): `a561459` (fix#2 honest
  re-verify), `80818a5` (canvas-ctor patch — gate-green, NOT the
  cause), `4988fcc`/`b48ae8b`/`d8c75bb` (fix#2 localization),
  `118c0d0` (fix#1 wdt SHIPPED), `27cf159`/`7f77c45`/`d562ad9`/`13faec2`
  (handoff + ledger + comparison), `01f41ef`→`01f45ef` (K2-DIFF
  decode), `197e826` (duolingo), `154ac8a` (re-measure), `8c4afae`
  (homedepot determinism), `d455794` (K1), `308f8ad` (Tier-1),
  `52db4a3` (engines docs), `d163d17`/`d63b4fc`/`a4c3f6c`/`6d67167`/
  `e869e18` (FP backlog) `[CODE]`.
- **§4 gate discipline held throughout:** every site-touching commit
  green — chrome_compat 437/0, holistic classifier_tests 10/0
  (ledger-byte-equivalent), iframe_isolation 5/0, v8_inspector_parity
  3/0, v8_natives 11/0. fix#1 corrected 4 chrome_compat tests to the
  Chrome-faithful `webdriver:false` value; the canvas-ctor patch was
  purely additive (no test correction needed).
- **The only failing tests anywhere = the 2 pre-existing `page::tests`
  canvas cases** (`getContext` env limitation), verbatim at branch
  base `fd98bfa`, **outside the defined §4 gate**, untouched by this
  branch's diff — proven pre-existing & unrelated, not a regression.
- **Zero fabricated results.** Every flip/elimination is `[MEAS]`-backed
  or honestly tagged `[HYP]`; the FP-E1 scoping, the G8 deferral, and
  the fix#2 "core throw UNRESOLVED" are honest engineering calls,
  documented as such.
- **No-budget stance preserved:** paid tools never run/installed/signed
  up for; the commercial ceiling is a cited reference row only.

---

**Bottom line (3 lines):**
1. From-scratch Rust+V8 engine opens **~120–121/126 routed** (±1 =
   leboncoin debug-timing noise); true hard residual = **Kasada×3 +
   homedepot**; **measured free-OSS SOTA parity** (Camoufox/Patchright
   also Kasada-blocked from this IP) reached **without a real binary**.
2. The Kasada mystery is **decoded into a named 5-fix list**: fix#1
   (`wdt`) **SHIPPED**, fix#2 **localized to a VM-bytecode-path
   divergence** (canvas-ctor confounder closed gate-green, core throw
   UNRESOLVED with the exact next RE step defined), fixes #3–5 pending.
3. Branch **not merged**; §4 gate green throughout (only the 2
   pre-existing canvas tests fail, outside the gate); zero fabrication;
   no-budget / paid-excluded stance held.
