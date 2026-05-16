# 00 — MASTER PLAN (2026-05-16): pass the full anti-bot site set

**Read this first.** Synthesis of the four 2026-05-16 deep-research
deliverables (02 codebase gaps, 03 OSS/vendor techniques, 04 Kasada
consolidated, 05 non-Kasada vendors). Honest status, the corrections that
invalidate older handoffs, a per-vendor gap matrix, and a phased,
regression-gated plan to pass all 29 sites.

Baseline: git HEAD `8b12977`. Goal restated: open every site a
**non-CDP real Chrome opens from this same datacenter IP**. `nocdp.sh`
proves a real Chrome passes canadagoose/hyatt/realtor from this exact IP
⇒ the IP is clean and every remaining blocker is an **engine gap**, not
reputation. `[[proxy_not_the_problem]]` stands, re-confirmed by external
2026 intel (03 §1: no public tool emulates a vendor VM and survives —
the only winners drive a real binary or pay a browser farm; our path is
*observational parity*, not VM emulation or patch-hiding).

---

## 1. The corrections that supersede the older handoffs

These are agreed by ≥2 independent research agents against current code —
treat the older docs as historical where they conflict.

1. **The "bounded §4 child-realm wiring pass" is DONE and LIVE — not
   pending.** `00_HANDOFF.md §4/§7` and `27_…md "Remaining wiring"` sell
   the genuine `v8::Context` child realm as the remaining work. It is
   already implemented and is the **primary path**:
   `op_create_child_realm` / `op_set_child_realm_prop` /
   `op_eval_in_child_realm` exist (`dom_ext.rs:1136/1291/1342`), are
   registered, and `_getIframeWindow` (`dom_bootstrap.js:2520`) calls
   them first; `new Proxy(iframeLocals,…)` is dead fallback-only code.
   Verified empirically: `iframe_fp_diag` now shows `cw_ctor:"Window"`,
   `instanceof Window`, distinct native `fetch`/`Array.prototype`,
   native-code `devicePixelRatio` accessor. **Docs 22/24's "contentWindow
   is a fake Proxy = THE Kasada gap" is superseded by code.** The
   blocker moved downstream.

2. **The Kasada "allow-but-blocked" paradox is the #1 unknown** (04 §(b)).
   Decoded final blobs `kasada_error_{7,8}.b64` show Kasada's *own client
   verdict* is `{"type":"ab","action":"allow","og":"…canadagoose.com"}`
   with `bot1225.b:1` set — yet canadagoose still serves the 756-byte
   429 interstitial, 6/6 reproducible. The production sentinel trap is
   *healthy* (`kasada_sentinel_clean.json`: 80 tagged VM closures, 80
   misses all genuine V8 builtins, `missTaggedElsewhere:0`, no identity
   loss). So the block is **not** the sentinel-loss / fake-Proxy-realm
   failure the prior multi-session effort assumed (those are fixed).
   Either the `b:1` bad-flag is recorded server-side and the `/tl` ML
   scores it bot regardless of client "allow", or the captured blobs are
   a `§9.3` test artifact not reflecting the real production flow. This
   must be settled by measurement before more Kasada code (see Phase 2).

3. **"homedepot renders 5/5" is STALE** (05 §"homedepot"). The decisive
   clean-production retest shows homedepot still serves the 2.6 KB
   Akamai **sec-cpt** PoW interstitial post-all-fixes. CLAUDE-memory
   `state_2026_05_15_playwright_ab_decisive` overstates this. (Memory
   corrected at end of this session.)

4. **Two older Kasada close-outs are falsified** (04 §(b)): doc 14's
   "Regime 2 ⇒ Audio-FP is THE single lever, VM framing retired" —
   audio parity shipped (123.97≈124.04), canadagoose still 429;
   and doc 17's "engine exhausted ⇒ residual is 20-30% IP-reputation" —
   contradicted by `nocdp.sh`. Do not resume either line.

5. **The realm faithfulness inverted** (02 Gap 1): the iframe child
   realm is now *more* Chrome-faithful than the **main window**.
   `window.constructor` is `undefined` on the main window because
   `window_bootstrap.js:77-86` deliberately refuses to set `globalThis`'s
   prototype to `Window.prototype` (it regressed deno_core ops). Kasada's
   disassembled dual-realm `globalThis[x]!==window[x]` / constructor
   identity bail now fires on the **main** window. This is a new,
   concrete, high-value target — and it is *not* the wiring pass.

6. **The two "IP-banned" labels are unverified and at least one is
   falsified** (2026-05-16 nocdp data point — resolves part of Open Q #4).
   A real CDP-free Chrome (`ab_harness/nocdp.sh`) pointed at **yelp** from
   this exact datacenter IP returned `document.title="yelp.com"` (the real
   homepage title is "Yelp") and rendered DataDome's **interactive
   "solve-this-task" captcha** (observed directly on screen). A banned
   datacenter IP returns a hard 403 / "Access Denied", *not* a solvable
   interactive challenge — so **"yelp = IP-banned, out of engine scope" is
   wrong**. yelp is **DataDome interactive-captcha class** (same bucket as
   the 5 captcha sites): out of *stealth* scope, but for a different
   reason than the docs state, and *not* an IP problem. **udemy** could
   not be cleanly retested (the headless box's rootless-Xwayland +
   command-backgrounding fights long nocdp runs), so its "primarily
   datacenter-ASN IP block" remains an **unverified hypothesis**, not an
   established fact. General rule going forward: do **not** assert "IP
   ban" anywhere without a captured hard-403 from nocdp; until then a
   blocked site is assumed engine-addressable (this is just the §1
   `[[proxy_not_the_problem]]` thesis applied consistently). The user's
   standing observation — a non-CDP real Chrome opens *most* of these
   sites from this IP — is the *foundation* of this plan (lines 9–16),
   not a contradiction of it; it specifically invalidates the 2 stray
   "IP" tags, nothing else.

---

## 2. Site set status (29 canonical, `audit_failing_sites.rs`)

Full per-site table with root cause + fix + difficulty: **`01_SITE_SET_STATUS.md`**.
Vendor rollup:

| Vendor | Sites | n | Block class | State |
|---|---|---|---|---|
| **Akamai** Bot Manager | bestbuy, costco, disneyplus, expedia, hm, homedepot, hulu, uniqlo, walmart, washingtonpost, weather | 11 | sensor-VM score / sec-cpt PoW | Envelope crypto byte-verified; **cleartext payload ~90% static placeholder** + fileHash 3/11 hosts. The big lever. |
| **Kasada** | canadagoose, hyatt, macys, realtor | 4 | `/tl` holistic ML score | Realm + native-toString + sentinel all fixed/healthy; allow-but-blocked paradox open. Hardest. |
| **DataDome** | etsy, leboncoin, tripadvisor, yelp | 4 | interstitial + i.js VM | Encryptor byte-verified but **dead code, zero callers**. yelp = interactive-captcha class (2026-05-16 nocdp: real Chrome from this IP gets DataDome solve-task, *not* an IP ban — see §1.6). |
| **captcha-gated** | duolingo, medium, quora, spotify, substack | 5 | interactive captcha | Out of stealth scope (human-interaction gate); deprioritize. |
| **Cloudflare** BM | udemy | 1 | edge + Turnstile | "Primarily datacenter-ASN IP block" is an **unverified hypothesis** (no clean nocdp capture yet — §1.6); `cf_clearance` never obtained. Treat as engine-addressable until a hard-403 is captured. |
| **PerimeterX**/HUMAN | wayfair | 1 | `_px3` sensor + PaH | **Zero solver code** — greenfield. |
| **BLOCKED / THIN-BODY** | brave, skyscanner, mail-ru | 3 | deny-list / redirect | Mostly classifier false-positives — verify first (cheap). |

**The math:** Akamai is 11/29 and the single highest-ROI surface. Kasada
is 4/29 and the hardest. The interactive-human-gate class is now **6/29**
(5 captcha + yelp's DataDome solve-task, per §1.6) and is not a stealth
problem. **Zero** of the 29 are a *confirmed* IP ban (udemy unverified;
yelp falsified). The remaining ~8 split across DataDome/CF/PX/edge.

---

## 3. Cross-vendor gap matrix (prioritized)

| # | Gap | Vendors/sites blocked | Evidence (file:line) | Diff | Risk |
|---|---|---|---|---|---|
| G1 | **Measurement hygiene**: `is_anti_bot_challenge()` body-substring classifier over-matches (`bm_sz`,`captcha`,`datadome` on benign pages) ⇒ false "blocked" verdicts; `/error` blobs may be `§9.3` test artifacts | ALL (corrupts every verdict) | `page.rs:175-208` | S | low |
| G2 | **Main-window realm inversion**: `window.constructor===undefined`, `globalThis[x]!==window[x]` on main window | Kasada (4), likely Akamai/DD holistic | `window_bootstrap.js:77-86` | L | high |
| G3 | **Akamai cleartext payload static**: `wsl`/`din`/`mst`/`fpt` hardcoded constants, not read from live surface | Akamai (11) | `v3_payload.rs` (per 05 §1) | M | med |
| G4 | **Akamai fileHash registry 3/11 hosts**: 8 hosts fall back to wrong value ⇒ edge can't reverse-shuffle | Akamai (8 of 11) | `lib.rs:229-252` | L | med |
| G5 | **DataDome solver unwired**: verified `DdEncryptor`/`sec-cpt` solver = dead code, no i.js exec path | DataDome (4) | `page.rs:1162` log-only; `datadome_crypto.rs:371` | M | med |
| G6 | **Kasada allow-but-blocked paradox** unresolved (the decisive experiment, Phase 2) | Kasada (4) | `04_…md §(b),(f)` | L | — |
| G7 | **TLS/H2 profile pinned to Chrome 147 capture, every preset UA = Chrome 148** ⇒ JA4-vs-UA cross-check tell (≈ −0.4 score; TLS+H2+hdr alone ≈ −1.2 pre-JS) | ALL edge gates | 02 Gaps 7/8; 03 §TLS | S/M | med |
| G8 | **Behavioral model bifurcated**: Plamondon/Sigma-Lognormal model only on CDP path; `Page::navigate` uses weak `humanize.js` | Akamai/DD/PX behavioral score | 02 Gap 3 | M | low |
| G9 | **V8 inspector/runtime traps untested** (2026 Proxy-prototype `console.groupEnd`, error-stack getter) — engine-portable, cheap to verify | Kasada/DD/CF | 03 §"new intel" | S | low |
| G10 | **PerimeterX: no solver at all** (only `px-captcha` substring detect) | wayfair (1) | `page.rs:196` | L | — |
| G11 | **Cloudflare/udemy datacenter-ASN IP block**; orchestrator never gets `cf_clearance` | udemy (1) | 05 §CF | M | high (IP) |
| G12 | `op_eval_in_child_realm` swallows all errors ⇒ under-populated child realm invisible | Kasada/DD diagnosis | `dom_ext.rs:1342` | S | low |

---

## 4. The plan — phased, regression-gated

**Discipline (mandatory, proven for fixes #6/#7):** implement → run the
regression gate → keep if green, **revert if not**. Gate =
`chrome_compat` (≥437/0) + `v8_natives` 11/11 + `iframe_isolation` 5/5 +
`iframe_fp_diag` (cw real natives, ctor "Window", not Proxy) +
`chrome147_parity` == pre-existing baseline. Never leave unverified
engine-wide breakage. V8 ⇒ `--test-threads=1`.

### Phase 0 — Measurement hygiene (S, do first, unblocks everything)

You cannot fix what you can't measure, and several "blocked" verdicts are
false positives (G1).

1. Tighten `is_anti_bot_challenge()` (`page.rs:175-208`): require
   structural challenge markers (challenge iframe `src`, `_abck=…~-1~`,
   sec-cpt JSON, 756-byte body), not bare substrings. Add a
   `verdict_reason` enum (edge-block / sensor-fail / render-incomplete /
   pass) so every site gets a *typed* outcome.
2. Re-run `audit_failing_sites::audit_all_failing_sites` (network) to get
   a clean current baseline of the 29 with typed reasons. Identify the
   render-incomplete false positives (cheap "wins" with zero stealth work).
3. Settle whether `kasada_error_*.b64` blobs are production or `§9.3`
   test artifacts (instrument the real `/error` POST in a clean navigate,
   not the Fn-wrap test path). This decides if the Kasada paradox (item 2
   of §1) is real.

### Phase 1 — Cheap engine-parity wins (S/M, high confidence)

4. **G7 TLS/UA coherence:** re-pin the TLS+H2 fingerprint to a Chrome
   **148** capture (we have `capture_chrome_148_hello.rs`) or roll every
   preset UA back to 147 — they must agree. Add a self-verifying JA4
   assertion test so this can't silently drift again (03 §8 top action).
5. **G9 V8 inspector parity:** add regression tests for the 2026
   Proxy-prototype `console.groupEnd(Object.create(proxyOwnKeysTrap))`
   trap and the error-stack getter trap (03 §"new intel"). These are V8
   inspector behaviors a from-scratch engine *should* pass — verify, don't
   assume. Cheap, high-ROI, likely already-green confidence builders.
6. **G12:** make `op_eval_in_child_realm` surface errors to a diagnostic
   channel so under-populated child realms become visible (prereq for
   diagnosing Kasada/DD child-realm probes).
7. **G8 behavioral unify:** route `Page::navigate` through the existing
   `stealth::behavior` Plamondon/Sigma-Lognormal model instead of the
   weaker `humanize.js`. Code already exists on the CDP path; this is
   wiring, not research.

### Phase 2 — Kasada: the decisive experiment, then follow (L)

8. Run **the differential identity experiment** (04 §(f), fully
   specified): a `§9.3`-safe `Object.prototype` sentinel trap that, at
   the exact throwing site (handler 167, `h[sentinel]`, `h===undefined`),
   records per builtin (`slice/concat/apply/clz32`) the object identity
   resolved via 4 acquisition paths (`window` / `globalThis` /
   `(0,eval)('this')` / `Function('return this')()`), run **identically
   in our engine AND CDP-free real Chrome 147 via `probe_title.sh`**. Four
   pre-registered outcomes (A reject Root 3 / B identity fragmentation /
   C wrap-and-rebind / D `.tl`-flow fallback) — it cannot return
   ambiguous. This is the first *differential* identity measurement; all
   prior Kasada experiments measured our engine in isolation.
9. Branch on the outcome:
   - **B (paths fragment where Chrome's are equal):** fix is
     `globalThis[X]===window[X]===eval-global[X]===child[X]` — directly
     tied to **G2** (main-window realm inversion). Implement the Rust
     global-proxy `[[Prototype]]`/identity unification mirroring what
     `op_create_child_realm` already does for child contexts.
   - **A (identity matches Chrome):** close the realm/sentinel line for
     good; pivot to the holistic-FP tail (`crs`/`wse`/`bfe`) verified
     CDP-free, since `action:"allow"` says no single hard fail.
   - **C/D:** per 04 §(f).

### Phase 3 — Akamai cleartext payload (M+L, covers 11 sites)

The envelope crypto is byte-verified; the *content* is fake. This is the
single highest-ROI engine investment.

10. **G3:** populate `wsl` (top scoring vector per doc 10), `din`, `mst`,
    `fpt` in `v3_payload.rs` from the live Chrome surface (navigator,
    timing, events) instead of hardcoded constants.
11. **G4:** auto-extract the per-host `fileHash` from the live bundle
    instead of the 3-host hardcode (`lib.rs:229-252`) so the edge can
    reverse-shuffle for all 11 hosts.
12. **sec-cpt** (homedepot et al.): wire the already-byte-verified
    `sec_cpt::solve_crypto`; prefer letting the in-page bundle self-solve
    (do **not** fire the BMP POST — doc 20 proved that fails), then
    re-issue. `solve_crypto` is the fallback once params are extractable.

### Phase 4 — DataDome / PerimeterX / Cloudflare

13. **G5 DataDome:** wire the verified `DdEncryptor` + add an i.js
    execution path in the nav pipeline (`page.rs:1162` is currently
    log-only). Covers etsy/leboncoin/tripadvisor structurally. (yelp:
    confirm IP-ban vs engine via `nocdp.sh` before spending effort.)
14. **G10 PerimeterX/wayfair:** greenfield `_px3` (AES-CBC-256+HMAC) +
    `_pxhd` sensor module; the unified bundle-self-solve path (Phase 5)
    is the cheaper route than a full RE.
15. **G11 Cloudflare/udemy:** confirm it is an IP block (datacenter ASN)
    via `nocdp.sh`; if so it is out of engine scope until residential
    egress exists — document, don't burn budget.

### Phase 5 — Unified "execute the vendor bundle in-engine" capability (L, strategic)

The correct long-term architecture (docs 19/20, 03 thesis): a nav-pipeline
feature that **runs the vendor's own obfuscated bundle in our
Chrome-faithful engine and lets it build+POST its own sensor**, consume
the Set-Cookie, and re-issue. One feature covers Akamai sec-cpt, DataDome
i.js, and PerimeterX sensor simultaneously — and is the only approach that
survives daily key rotation (DataDome shipped a JS VM 2026-01-14 with
6-char daily key rotation; HTTP-only solvers are dead). This is the
endgame; Phases 0-4 are the de-risking path to it.

---

## 5. Open questions / decisive experiments (ranked)

1. **Are the `/error` blobs production or `§9.3` test artifacts?** (Phase
   0.3) — gates whether the Kasada "allow-but-blocked" paradox is real.
2. **The differential identity experiment** (Phase 2.8) — settles the
   80/80 disjoint-set root cause; cannot return ambiguous.
3. **Is the main-window realm inversion (G2) load-bearing for Kasada?**
   — likely answered as a side effect of experiment 2 outcome B.
4. **yelp / udemy: IP-ban or engine?** — *partially resolved 2026-05-16*
   (§1.6): yelp = DataDome interactive captcha in real Chrome (not IP);
   udemy still needs one clean `nocdp.sh` run (box environment blocked
   it). Prevents wasted L-effort.
5. **Which of the 29 are pure render-incomplete false positives?** (Phase
   0.2) — converts unknown count of "blocked" into free passes.

---

## 6. Eliminated dead-ends (do not retry — 04 §(g) + prior)

IP/proxy; CDP-based references; synthetic Kasada solver tweaks; W1.1
`_buildRemoteRealm` memoization; eval-source interception for the sentinel
key; FN_TRACE/Function-wrap traces as production evidence; **audio-FP as
the sole Kasada lever (falsified)**; doc 17 IP-exhaustion close-out
(contradicted by `nocdp.sh`); init-script filename leak; JS-only
`[[SourceText]]` patching (structurally closed by native fix #6);
"replace the Proxy contentWindow" (**done**); `dpi:"n/a"` as a divergence
(matches Chrome); near-term VM emulation (treadmill — external intel
re-confirms it loses within days).

---

## 7. Document index — `docs/research_2026_05_16/`

- **00_MASTER_PLAN.md** — this file (entry point).
- **01_SITE_SET_STATUS.md** — per-site table: vendor, block class, root
  cause, fix, difficulty, decisive check.
- **02_CODEBASE_GAP_ANALYSIS.md** — engine gaps mapped to file:line;
  stale-doc ledger; the realm-inversion finding.
- **03_OSS_STEALTH_VENDOR_TECHNIQUES_2026.md** — external 2026 intel;
  proven-vs-folklore; per-vendor surface; TLS/H2 SOTA; sources appendix.
- **04_KASADA_CONSOLIDATED_AND_DECISIVE_EXPERIMENT.md** — all Kasada
  knowledge; proven vs hypothesis; the allow-but-blocked paradox; the
  fully-specified decisive experiment; eliminated dead-ends.
- **05_NON_KASADA_VENDORS_STATUS_AND_FIX.md** — Akamai/DataDome/CF/PX
  per-vendor code reality + concrete fixes + cross-vendor priority table.

Prior context (still valid where not superseded by §1):
`docs/research_2026_05_15/00_HANDOFF.md` (entry), `26/27` (disassembled
Kasada VM + V8 realm recipe), `docs/research_2026_05_14/09..11`.

---

## 8.5 Execution status log (updated as phases land)

**Phase 0.3 — SETTLED 2026-05-16 (the blobs are PRODUCTION).** Primary
evidence, not inference:
- `kasada_error_*.b64` is written by exactly one test:
  `kasada_error_blob_capture` (`chrome_compat.rs:4079`). Its
  `capture_init` wraps **only** `TextEncoder.prototype.encode`,
  `XMLHttpRequest.prototype.{send,open}`, and `globalThis.fetch` — every
  hook is a passive `.apply(this, arguments)` pass-through. There is **no
  `globalThis.Function` wrapper and no sentinel trap** in the injected
  script.
- `git log -L 4079,4090:…chrome_compat.rs` (full function history):
  introduced in `7c4e56e`, modified once in `f537c06` (added the
  TextEncoder hook). At **no commit** did this test's `capture_init`
  wrap `globalThis.Function`.
- Decoded `kasada_error_7.b64` (outer-b64 → JSON `.data` → b64 → XOR
  `omgtopkek`) = `{"type":"ab","action":"allow","og":
  "https://www.canadagoose.com",…,"bot1225":{"r":"{\"ifp\":1604157259,
  \"ilk\":2418312849}","t":32,"b":1},"time":33}`.

⇒ **Doc 04 §(c) / §1.2's "kasada_error_blob_capture wraps
`globalThis.Function` (the §9.3 confound)" is FALSE against the code at
every commit.** The blobs are production-representative. **The
allow-but-blocked paradox is REAL**: Kasada's own client SDK verdict is
`action:"allow"` *with* `bot1225.b:1` set, while canadagoose still serves
the 429. There is **no single client-side hard-fail** (consistent with
`action:"allow"` and the healthy 80/80 clean sentinel). The block is the
server scoring the `b:1` bot-flag / holistic `/tl` ML. **Open Q #1
RESOLVED.** Reframes Phase 2: the differential-identity experiment is
still worth running (it tells us whether an identity divergence *feeds*
`b:1`), but the target is "what makes our accumulated signal compute
`b:1`", not a hypothetical single client probe kill.

**Phase 1 G7 — REFRAMED 2026-05-16 (do NOT roll UA→147).** Evidence
found while scoping: `docs/CHROME_148_SWEEP_RESULTS_2026_05_13.md` is an
empirical 126-site A/B of UA 147→148. UA=148 is a **deliberate, tested,
primary-source-grounded** decision: real Chrome stable IS 148
(chromiumdash; shipped early May 2026), and the bump *recovered*
homedepot/hotels/leboncoin (Akamai/DataDome score "current Chrome" more
favorably). Chrome's TLS ClientHello does not rev across 147→148 (the
last TLS-stack change was the MLKEM768 rollout at Chrome 131); JA4
encodes TLS-version/cipher-count/ext-count/ALPN, **not** the Chrome
minor/major — so the verified-real Chrome-147 bytes (`docs/
CHROME_147_TLS_REFERENCE_2026_04_29.json`; doc 03 §1.1 "byte-exact Chrome
147/148 values") are exactly what real Chrome **148** emits on the wire.
The "147-vs-148 skew" is a cosmetic label, **not** a wire-observable
divergence. ⇒ Rolling UA→147 (the plan's fallback branch) would revert a
tested decision, re-introduce the documented regressions, and advertise
an outdated browser. **Correct G7 deliverable = the plan's stated top
action only:** a network-free self-verifying drift-guard test that locks
the cipher/sigalg/curve/extension/H2-SETTINGS vectors to the documented
byte-exact reference and a single source-of-truth coherence constant
documenting 147≡148. No UA change, no byte change ⇒ zero regression risk,
satisfies "self-verifying JA4 assert", removes the silent-drift hazard.

**Phase 0/1 readiness snapshot (2026-05-16 session):**
- Phase 0.1 code landed (typed `ChallengeVerdict`, tightened structural
  classifier, audit harness wired); regression gate running, commit
  pending green per §4 discipline.
- Phase 0.3 settled (blobs production; paradox real) — §8.5 above.
- Phase 0.2: harness now emits typed verdicts; network re-baseline is
  the run itself.
- Phase 1 scoped & evidence-checked: **G7** = drift-guard self-assert +
  147≡148 coherence constant (NOT a UA roll — see reframe above);
  **G9** = 3 V8-inspector parity asserts (error-stack getter lazy,
  `console.debug(errWithStackGetter)` no-fire, `console.groupEnd(
  Object.create(proxyOwnKeysTrap))` no-fire) added to `chrome_compat`;
  **G12** = `op_eval_in_child_realm` (`dom_ext.rs:1366`
  `let _ = script.run(cs)`) → capture+surface the exception behind a
  debug channel; **G8** = route `Page::navigate`'s `humanize.js` through
  the existing `stealth::behavior` model. Each lands as its own
  gate-checked commit.

**Phase 0.2 — DONE 2026-05-16 (the decisive false-positive result).**
Typed re-baseline of all 29 (`audit_all_failing_sites`,
`Page::navigate(url, chrome_130_macos(), 1)`, committed `3739da9`
classifier). `/tmp/audit_failing_sites/_index.json` tally:

| verdict | n |
|---|---|
| **pass** (rendered >5 KB, no structural challenge) | **18** |
| edge-block (challenge stub served) | 8 |
| sensor-fail (large body + challenge marker) | 2 |
| render-incomplete (thin stub, nav bug) | 1 |

- **pass (18)**: bestbuy, brave, costco(3.7 MB), disneyplus(1.5 MB),
  duolingo, expedia(495 KB), h-m, hulu(1.4 MB), leboncoin(460 KB),
  **macys(1.7 MB — was Kasada)**, quora, skyscanner, spotify,
  uniqlo(1.6 MB), walmart(354 KB), **washingtonpost(2.9 MB — the
  suspected wapo FP, confirmed)**, **wayfair(1.3 MB — was greenfield
  PerimeterX)**, weather(2 MB).
- **edge-block (8)**: canadagoose(732 B), hyatt(737 B), realtor(1764 B)
  [genuine Kasada 429]; homedepot(2766 B) [genuine Akamai **sec-cpt** —
  confirms §1.3 "5/5 STALE"]; etsy(1424 B), tripadvisor(1430 B),
  yelp(1424 B) [genuine DataDome interstitial]; medium(39 KB) [captcha].
- **sensor-fail (2)**: udemy(476 KB, "just a moment" CF), substack(62 KB).
- **render-incomplete (1)**: mail-ru(959 B) — nav/redirect bug, not
  stealth (matches §2).

**Headline: 10 of 11 Akamai sites, plus macys (Kasada) and wayfair
(PerimeterX), were classifier false positives** — they render. The old
"22 engine-addressable" framing was inflated by measurement error. The
**true stealth-addressable hard set is ~6**: canadagoose/hyatt/realtor
(Kasada), homedepot (Akamai sec-cpt), etsy/tripadvisor (DataDome); udemy
is CF (sensor-fail, body renders); medium/yelp/substack are human-gate.
This is exactly the cheap Phase-0 win the plan predicted; Phases 2–4
should target the ~6, not 22. **Caveat:** `pass` = "rendered, no
structural challenge marker on a 1-iteration nav"; small-body passes
(bestbuy 7.8 KB, spotify 9.6 KB, duolingo 13 KB) are likely thin shells —
verify content depth before declaring a site fully won. The big-body
passes (costco/disneyplus/weather/wapo/uniqlo/hulu/macys/wayfair, all
0.4–3.7 MB) are real renders.

**Phase 1 G8 — DEFERRED 2026-05-16 (scoped follow-up, rationale).**
G7/G9/G12 landed (commits `b728840`, `<G12>`). G8 (route
`Page::navigate`'s `humanize.js` through `stealth::behavior`) is
deliberately deferred, not skipped, because: (1) `humanize.js` *already*
implements a sigma-lognormal Plamondon model inline (its header docs the
exact `v(t)` curve) — the gap vs `behavior.rs` is model *richness*
(handedness/Fitts/ChaCha determinism), a parity nicety, not a missing
capability; (2) per the Phase 0.2 re-baseline the behavioral score is
**not** the blocker for any of the true hard-6 (Kasada =
allow-but-blocked identity paradox; homedepot = sec-cpt PoW;
etsy/tripadvisor = DataDome i.js) — G8 flips zero hard-6 sites; (3) it
is the one M-difficulty Phase-1 item and it feeds
`__akamai_events`/`akamai/src/payload.rs`, so a trajectory-shape change
carries real regression risk to the green 437-test gate for no
site-flip return. Goal-optimal per "close out as many sites as
possible" + mandatory revert-if-not-green: proceed to Phase 2 (the
decisive Kasada experiment — highest leverage, 3 of the hard-6) rather
than spend the heavy build/gate budget destabilizing a green engine on
a non-hard-6 refactor. **Precise scoped follow-up when revisited:** add
`op_behavior_mouse_trajectory` / `op_behavior_keystroke_timings`
(thin wrappers over `behavior::mouse_trajectory_with_rng` /
`keystroke_timings_with_rng`, seeded by `BehaviorProfile::rng_for`),
have `humanize.js` *prefer* them with its current model as fallback
(additive, gate-safe), then re-baseline the Akamai sensor tests.

**Phase 2 — DONE 2026-05-16: the decisive experiment returned OUTCOME
A. The Kasada realm/sentinel/identity line is CLOSED as not-the-bug.**
`kasada_identity_decisive::kasada_global_identity_invariant_holds`
(network-free, deterministic — the Chrome reference is the ECMAScript
"one realm ⇒ one set of intrinsics" *spec invariant*, no live capture
needed) measured, in our engine, the four global-acquisition paths
Kasada's disassembled VM compares (`window` / `globalThis` /
`(0,eval)('this')` / `Function('return this')()`) for the exact
intrinsics it reads (`Array.prototype.slice/concat`,
`Function.prototype.apply`, `Math.clz32`):

  `slice:WGEF concat:WGEF apply:WGEF clz32:WGEF globals:WGEF`

— every path resolves the **identical** object for every builtin, and
the four globals are mutually `===`. This is exactly Chrome's pattern.
Per the doc 04 §(f) Step-4 pre-registration this is **OUTCOME A: H1
(Root 3, identity fragmentation) REJECTED**; the disjoint 80/80 clean
sentinel is Chrome-faithful, not a bug. Converging evidence: child
realm genuine (doc 02 §0), clean sentinel healthy
(`missTaggedElsewhere:0`), client verdict `action:"allow"` (Phase 0.3).
⇒ **The Kasada block on canadagoose/hyatt/realtor is the holistic
Root-2 ML tail** (`bot1225.b:1` scored server-side from accumulated FP;
no single client lever — and audio, the one named Root-2 input, already
shipped without flipping). The realm/sentinel/identity line that 3+
prior sessions chased is **definitively eliminated**. Actionable
redirect: the 3 Kasada sites are not cheaply engine-flippable (holistic
diminishing-returns, master-plan §6 dead-ends); spend remaining budget
on the concrete, addressable hard-6: **Phase 3 homedepot (Akamai
sec-cpt) + Phase 4 etsy/tripadvisor (DataDome i.js)**. The live
`#[ignore] kasada_identity_decisive_live_canadagoose` is corroboration
only — the network-free invariant + the §6 evidence already settle it.

**Phase 3/4/5 — SCOPED 2026-05-16 (honest, post-rebaseline; no
speculative code added).** With Phase 0.2's false-positive elimination
and Phase 2's Kasada-line closure, the true hard-6 reduce to two
problem classes, both verified against code:

- **Akamai (Phase 3) collapses to homedepot only.** 10/11 Akamai sites
  render (Phase 0.2 `pass`). The BMP-vs-sec-cpt skip guard the docs
  flagged is **already correctly implemented** (`page.rs:469-477`
  returns `NeedsSecCpt`, skips the wrong POST). homedepot serves the
  **rotating-obfuscated-bundle** sec-cpt variant: the 2.6 KB body is
  `<div id="sec-if-cpt-container">` + `<script src="/Wjv3…">` with
  **no parseable 428 JSON / no inline nonce·difficulty·verify_url** —
  `sec_cpt::solve_crypto` (byte-verified) **cannot be fed without
  executing the bundle** (confirmed: doc 05 §1a + `SecCptChallenge`
  schema vs the body shape). G3 (live `wsl`/`din`) and G4 (fileHash
  auto-extract) target the 10 already-rendering sites → flip zero
  hard-6 and add akamai-sensor regression risk. Wiring `solve_crypto`
  for the classic-428 variant is possible but **no target site serves
  that variant** and there is **no captured parseable-428 fixture** to
  verify an extractor against — building it would be unverifiable
  speculative nav code (violates revert-if-not-green / verify-don't-
  assume). ⇒ homedepot is a **Phase 5** problem.
- **DataDome (Phase 4) = same shape.** `DdEncryptor` byte-verified,
  zero callers; etsy/tripadvisor need in-engine **i.js + WASM
  `boring_challenge` execution + same-origin round-trip**; DataDome
  shipped a JS VM 2026-01-14 with 6-char **daily** key rotation —
  HTTP-only solving is dead (doc 05 §2). Also **Phase 5**.

**Net:** the master plan's own §4/§5 thesis is confirmed by code —
"Phases 0-4 are the de-risking path to Phase 5 (the unified in-engine
vendor-bundle self-solve)". Post-rebaseline there is **no safely-
gateable single-commit site flip remaining**; the residual hard set
(homedepot, etsy, tripadvisor) all require the Phase 5 capability
(L, strategic), and Kasada×3 is the holistic tail with no lever. The
decisive engineering win of this work is **measurement integrity**: the
"22 engine-addressable / blocked" framing was inflated by classifier
error; the engine already renders 18/29, the Kasada realm hunt is
closed, and the real residual is a small, sharply-characterized Phase-5
problem — not 22 scattered unknowns. Next session: build Phase 5
directly (unified bundle-self-solve), starting with DataDome
etsy/tripadvisor (M, the cheapest Phase-5 entry per doc 05 §2d) using a
live capture as the dev oracle; do **not** re-chase Kasada realm/
identity, Akamai wsl/din, or `solve_crypto` static-param extraction
(all eliminated above).

## 8. One-line summary for the next session

The realm wiring is **done** (older handoffs are stale); the engine is
far more Chrome-faithful than the docs claim. The remaining work is:
(0) fix the measurement so verdicts are trustworthy, (1) cheap parity
wins (TLS/UA coherence, V8-inspector tests, behavioral unify), (2) the
**one decisive Kasada differential-identity experiment** that resolves
the allow-but-blocked paradox and the main-window realm inversion
together, (3) make Akamai's cleartext payload real (covers 11/29), then
(4/5) wire the already-built DataDome solver and build the unified
in-engine bundle-self-solve capability.
