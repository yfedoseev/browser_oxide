# UNBLOCK Kasada — concrete plan for canadagoose.com, hyatt.com, realtor.com

**Author:** Kasada unblock-research agent · **Baseline git HEAD:** `fd98bfa`
· **Created:** 2026-05-16 · **Scope:** the 3 genuinely-blocking Kasada
sites only (macys is a settled FP that renders — out of scope here).

> **⚠ CORRECTION 2026-05-16 (supersedes the "not passable engine-only /
> needs paid farm + behaviour" verdict in this doc):** [MEAS] real
> Chrome 147 via `nocdp.sh` — zero interaction, this datacenter IP —
> **passes all 3** (`ab_harness/nocdp/*.windows.txt` titles = real
> homepages). Same IP + zero behaviour as our engine, yet it passes ⇒
> the gap is **NOT IP, NOT behaviour, NOT a paid-farm requirement** but
> a **passive engine-vs-Chrome-147 surface divergence**, and it is
> engine-addressable. The live-oracle `/tl` reference is **already
> captured** (`ab_harness/tl/hyatt.tl_body.bin` + `canadagoose.pcap`).
> Priority next step = **K2-DIFF**: capture our `/tl` POST, field-diff
> vs the real-Chrome capture → the divergent field is the named bug.
> Read `UNBLOCK_PLAN.md` "canadagoose, hyatt, realtor" for the
> corrected ordered plan; the K1/K3 below stand, K2 is reframed from
> "build a regime" to "capture-ours + diff (oracle in hand)".

**Provenance legend:** **[MECH]** cited external mechanism fact (URL +
accessed date in §7) · **[CODE]** our-code fact (file:line read this
session) · **[HYP]** labeled hypothesis + its discriminating experiment.

**Inputs already settled (do NOT relitigate — see `kasada.md`, master
plan §1/§8.5):**
- The **allow-but-blocked paradox is real & closed as a question**:
  decoded `kasada_error_7.b64` = `{"type":"ab","action":"allow",
  "og":"https://www.canadagoose.com",…"bot1225":{…,"b":1},"time":33}`.
  Client SDK says *allow*; server scores the accumulated `b:1` telemetry
  as bot and serves the 429. [CODE/MEAS prior session]
- **Phase 2 OUTCOME A CLOSED the realm/sentinel/identity line**
  (`kasada_identity_decisive_ours.txt` = `slice:WGEF … globals:WGEF`;
  `kasada_sentinel_clean.json` = 80 tagged / 80 missed,
  `missTaggedElsewhere:0`). Do **not** propose resurrecting it.
- Live audit this session: canadagoose **732 B**, hyatt **737 B**,
  realtor **1764 B** — all still 429 edge-block from this datacenter IP;
  `ab_harness/nocdp.sh` vanilla Chrome 147 opens all three from the
  **same IP** ⇒ engine gap, **not** an IP ban.

---

## 0. The one-paragraph verdict (read this first)

**There is no public engine-only path that flips these 3 sites, and the
2026 web intel converges hard on *why*: every tool that reliably passes
Kasada in 2026 either drives a real browser binary or is a paid hosted
black box, and the residual that produces `bot1225.b:1` on a from-scratch
headless engine is exactly the two surfaces the client `action:"allow"`
does not clear — (1) behavioral-telemetry entropy in the `/tl` payload
and (2) server-side JA4/HTTP-2-vs-UA coherence — plus the headless GPU
string.** Our code already passes stages 1–4 (TLS byte-pinned, HTTP/2,
the VM runs, `/tl` POSTs, we get `action:"allow"`). The honest plan is
therefore **not a code commit that flips the verdict**; it is a small
ordered set of *capability builds* whose effect is only verifiable under
an explicitly-authorized **live-oracle differential `/tl` regime** — the
mandatory network-free §4 gate structurally cannot prove a Kasada flip
because the verdict is computed server-side from live behavioral+TLS
telemetry. The single most actionable, lowest-risk, highest-EV move is
**wire `stealth::behavior` into the navigate path so the `/tl` payload
carries non-zero behavioral entropy** — "zero behavioral variance" is a
*named* 2026 production blocking trigger [MECH ScrapeBadger 2026] and our
navigate path provably emits none.

---

## 1. What 2026 intel says actually defeats Kasada (brutally skeptical)

Eight 2026 sources read this session (full URLs + accessed dates §7).
De-hyped, the consensus is unambiguous:

| Approach | Passes Kasada 2026? | Mechanism | Reproducible by us (engine-only)? | Source |
|---|---|---|---|---|
| Patchright / Camoufox / Nodriver / SeleniumBase-UC | **Yes** [REPORTED] | **Drive a real Chrome/Firefox binary**; only hide the automation control surface — the Kasada VM runs in a *genuine engine* | **No** — they *are* a browser; we *are* the engine | thewebscraping.club LAB #76; Scrapfly 2026; asadfix 2026 |
| Hyper Solutions `hyper-sdk-go` / Takion / Scrapfly / WebScrapingAPI | **Yes** (Scrapfly **99%** on realtor.com; WebScrapingAPI 98%) [REPORTED] | **Thin client to a paid hosted API** that runs `ips.js`/`p.js` in a maintained real-browser farm and returns `x-kpsdk-ct/cd/v` | **No** — paid black box; `hyper-sdk-go` README confirms "get your API key from the dashboard", **no local algorithm exposed** [MEAS WebFetch] | scrapeway 2026 benchmark; github.com/Hyper-Solutions/hyper-sdk-go |
| `curl-impersonate` / `rquest` alone | **No** | TLS/H2 only — "won't bypass Kasada's JavaScript challenges" | n/a (we already do TLS/H2 + run the VM) | Scrapfly 2026 |
| nixbro/Kasada-Solver, ChrisYP, lktop/kpsdk, kpsdk-solver | **Not reproducible** | **Documentation/marketing only — no runnable token algorithm.** nixbro = "purely educational, no code, no solver" [MEAS WebFetch]; lktop = QQ/email sales page; kpsdk-solver = Playwright-driven (a real browser) | **No** | github.com/nixbro/Kasada-Solver [MEAS]; ChrisYP writeup |
| `playwright-stealth` (JS-patched) | **No — explicitly blocked** | Kasada has its `Function.prototype.toString()` patch signatures **catalogued** and "blocks it outright" | (validates our fix — see §4) | asadfix 2026 [MEAS] |

**Load-bearing conclusions for us:**

1. **[MECH] No 2026 source has an open `x-kpsdk-ct` algorithm or an open
   description of what computes `bot1225.b:1`.** Every working approach
   runs a real engine. This is genuinely novel territory *because no OSS
   tool faces it* — they all sidestep it with a real browser. Nothing
   external shortcuts the from-scratch problem. (Confirms `kasada.md` §7.)
2. **[MECH] "Zero behavioral variance" and "headless browser GPU
   (SwiftShader WebGL renderer string)" are *named* production blocking
   triggers** — ScrapeBadger 2026 lists them verbatim; Scrapfly,
   roundproxies, and the WebSearch corpus independently corroborate that
   mouse/scroll/keystroke entropy and the canvas/WebGL readback are
   *folded into the encrypted `/tl` token* and scored server-side. This
   is the most concrete, actionable external fact in the entire corpus.
3. **[MECH] UA↔fingerprint coherence is itself the signal.** asadfix
   2026 (verbatim): *"If your UA says Chrome but JA4 says Python
   urllib3, you flag faster than no spoofing at all — the mismatch is
   the signal."* Kasada server-side re-derives JA4/JA4H + H2-SETTINGS
   and cross-checks against UA. (Same class Akamai pioneered.)
4. **[MECH] Kasada *regenerates the detection logic per page load* and
   "VM emulators break within days".** ⇒ our observational-parity path
   (run the real VM, don't emulate it) is the *only* correct direction;
   it also means **any fix is only as good as a live differential
   measurement**, never an offline assertion.
5. **[MECH] Scrapfly's tested realtor.com success is 99% — but via a
   paid real-browser farm with residential rotation + session warming.**
   The wide variance (99% → 0% across services) confirms Kasada remains
   hard *even for professional paid APIs*; a self-hosted engine with no
   behavioral channel is below the floor those APIs operate at.

There is **no site-specific public lever** for canadagoose/hyatt/realtor
beyond "they are standard strict-tenant Kasada (retail / hospitality /
real-estate-listings)". realtor.com is the only one that appears in the
scrapeway benchmark; it is solved there *only* by paid real-browser
farms. No ticketing-style tenant-specific weakness was found.

---

## 2. Our exact code reality vs. the three feed-points of `b:1`

[CODE] read this session: `crates/stealth/src/kasada.rs`,
`crates/net/src/kasada_session.rs`, `crates/stealth/src/behavior.rs`
(header), `crates/browser/src/page.rs:1214-1246`.

| `b:1` feed (from §1 intel) | Our code state | Verdict |
|---|---|---|
| **Behavioral entropy in `/tl`** | `stealth::behavior` is a **full Sigma-Lognormal/Plamondon model** (`behavior.rs:1` header: Plamondon 1995 strokes, bigram-modulated keystroke flight, exponential scroll decay, ChaCha20 per-session determinism). **But** `Page::navigate` injects **only `js/humanize.js`** (`page.rs:1214-1215`). The rich model is **NOT wired into the navigate path**; a headless nav emits **zero** mouse/scroll/keystroke events before `/tl` fires. | **THE gap.** Textbook "zero behavioral variance" [MECH ScrapeBadger]. Highest-EV, lowest-risk lever. |
| **JA4/H2-vs-UA server coherence** | TLS byte-pinned to verified-real Chrome 147 (`docs/CHROME_147_TLS_REFERENCE_2026_04_29.json`); UA = Chrome 148. Master plan §8.5 G7: 147≡148 on the wire (JA4 doesn't encode the minor; last TLS-stack rev was MLKEM768 @ Chrome 131). That is *our analysis*, not a live differential vs Kasada. | Plausibly already clean; **unverified against Kasada specifically.** Cheap to *measure*, do not "fix" speculatively. |
| **Headless GPU / canvas-WebGL readback** | `/error` blobs 3-6 are real RGBA canvas/WebGL pixel readback — Kasada *does* read the renderer. WebGL vendor/renderer parity asserted elsewhere (doc 17, general) but **never verified against Kasada's specific probe set**. | Likely-clean-but-unverified; a SwiftShader-class string is a *named* trigger [MECH ScrapeBadger]. |
| **PoW token hygiene (self-inflicted `b:1`)** | `compute_cd_header` (`kasada_session.rs:291`) is **WIRED** (`net/lib.rs:692/863/1211/1405`) and runs **in parallel** to ips.js self-solving the strict tenants in V8. `rst`/`d`/aligned-work-time are **synthesized plausibles, only self-replay-tested** (the file's own `UNVERIFIED-VS-LIVE` doc-comment, `kasada_session.rs:282-290`). A Rust-injected single-use `x-kpsdk-cd` competing with ips.js's own is a **plausible self-inflicted replay/`b:1` signal** (`x-kpsdk-cd` is single-use, server-tracked, <5 s). | **L1/FP-K1 hazard.** Not the *primary* lever but a cheap, gate-verifiable hygiene fix that *removes a candidate `b:1` source* — do it regardless. |

**Net:** stages 1–4 pass (we get `action:"allow"` — proof the VM ran and
no client hard-fail). The block is the server scoring stages 5+6
accumulated. Of the four feed-points, exactly **one is a concrete,
in-code, named-by-2026-intel, low-risk gap: the behavioral channel is
empty.**

---

## 3. The concrete ordered plan

Each item: **what · why it addresses `b:1` · difficulty · risk ·
verification regime.** Items K3+K4 are *measurement-gated* — do not
"fix" them blind; the live-oracle capture (K2) tells you whether they
are even contributors.

### K1 — Gate the parallel Rust PoW off when ips.js self-solves (do first) — [CODE]
- **What:** When ips.js is the active in-V8 solver for the strict
  tenant (the `op_net_xhr_sync` `/tl` path is live), ensure
  `compute_cd_header` does **not** also inject a competing single-use
  `x-kpsdk-cd` on the same logical request. One `cd` per request on the
  wire, period.
- **Why `b:1`:** a double/inconsistent single-use PoW is server-tracked
  and replay-defended (<5 s, single-use) — a **self-inflicted bot
  signal candidate**. Removing it deletes a confound *before* we
  measure, so K2's differential isn't polluted by our own footgun.
- **Difficulty:** **S** (a `!has_header` / "ips.js-active" guard +
  one integration assert).
- **Risk:** Low (the Rust path is not load-bearing for the strict
  tenants per §8/L1; gating it cannot regress the in-V8 solve).
- **Verification:** **§4-gate-verifiable** — a network-free test
  asserting only one `x-kpsdk-cd` reaches the wire when the in-V8
  `/tl` path is active. This is the *only* item the offline gate can
  fully prove. (= FP-K1 / `99_CODE_FALSE_POSITIVES.md` Class A, P1.)

### K2 — Build the live-oracle differential `/tl` capture (the unblocker for everything below) — [HYP→experiment]
- **What:** A passive `XMLHttpRequest.prototype.send` / `fetch` hook
  scoped to `/<tenant>/tl` (modeled byte-for-byte on the *clean*
  `kasada_error_blob_capture` pattern at `chrome_compat.rs:4079` — **NO
  `globalThis.Function` wrapper**, the §9.3 confound) that records the
  real `/tl` POST body. Run the **identical** capture in (a) our engine
  and (b) `ab_harness/nocdp.sh` real Chrome 147 on the **same site +
  same IP**. Decode (outer-b64 → JSON → inner-b64 → XOR `omgtopkek`)
  and diff the **behavioral sub-block** and the **server-visible
  JA4/JA4H/H2 SETTINGS**.
- **Why `b:1`:** behavioral entropy + JA4/H2-vs-UA are the **only two
  surfaces `action:"allow"` does not clear** [MECH]. This experiment
  *cannot come back ambiguous*: either there is an entropy/TLS delta
  vs real Chrome (→ K3/K4 are confirmed contributors with a named
  target), or there is no measurable delta (→ residual is pure
  IP/ML-weight + trust-decay, and Kasada is **not engine-flippable
  from this IP regime** — a valid, documented close-out, since
  `nocdp.sh` already argues IP is clean so this outcome would point at
  trust-decay/warming infra, not stealth code).
- **Difficulty:** **M** (the hook is small; the cost is the
  **authorized live-oracle dev loop** + the `nocdp.sh` differential
  harness — a regime, not a commit).
- **Risk:** Low to the engine (passive hook, scoped to `/tl`, gated by
  `is_anti_bot_challenge` so it never runs on benign navs).
- **Verification:** **NOT §4-gate-verifiable** — this *is* the
  live-oracle regime the gate structurally cannot replace. The
  deliverable is the **named delta**, not a flip.

### K3 — Wire `stealth::behavior` into `Page::navigate` before `/tl` fires (the primary lever) — [CODE gap, MECH-named]
- **What:** Route the navigate path's input through the existing
  `stealth::behavior` Plamondon/Sigma-Lognormal model so the page
  receives **real, entropic** mouse trajectories, scroll momentum, and
  (where a field exists) keystroke timing **before** ips.js POSTs
  `/tl`. The model already exists and is per-session deterministic
  (ChaCha20 seed) — the gap is purely that `Page::navigate` injects
  only `humanize.js` (`page.rs:1214-1215`), not the rich model.
- **Why `b:1`:** directly attacks the **#1 named 2026 production
  trigger** — "zero behavioral variance / no mouse jitter, identical
  scroll timing" [MECH ScrapeBadger 2026, corroborated Scrapfly/
  roundproxies/WebSearch]. A headless nav with *no* mouse/scroll at
  all is the textbook case; this converts an empty behavioral block
  into an entropic one.
- **Difficulty:** **M** (model exists; wiring it through navigate +
  ensuring events land in the DOM the VM observes is the work).
- **Risk:** **Medium** — `stealth::behavior` trajectory shape also
  feeds Akamai sensors (`akamai/src/payload.rs`, `__akamai_events`);
  a shape change carries real regression risk to the green 437-test
  gate (this is exactly why master plan G8 *deferred* it). Mitigation:
  prefer **additive `op_behavior_*` wrappers** scoped to the Kasada
  challenge nav, not a global humanize replacement; gate-check per §4.
- **Verification:** **§4 for the wiring/no-regression** (chrome_compat
  ≥437/0, iframe_isolation 5/0, v8_inspector_parity 3/0, v8_natives
  11/0 must stay green); **the flip itself is verifiable only under K2**
  (re-decode `/tl` with behavior wired; confirm the behavioral block is
  now entropic and re-measure the 429↔200). **Sequence: K2 before/around
  K3** so you measure the empty→entropic transition, not guess it.

### K4 — JA4/H2 + GPU parity, *measurement-gated by K2* — [HYP, conditional]
- **What:** (a) Add the self-verifying network-free JA4 drift-guard +
  147≡148 coherence constant (master plan Phase 1 G7 deliverable — a
  test that locks cipher/sigalg/curve/extension/H2-SETTINGS vectors to
  the byte-exact reference; **no UA change, no byte change**). (b) If
  and only if K2 shows a server-visible JA4/H2 or canvas/WebGL-renderer
  delta vs `nocdp.sh` real Chrome, correct the specific delta (re-pin
  to a captured Chrome-148 hello via `capture_chrome_148_hello.rs`;
  or correct the GPU vendor/renderer string for the Kasada probe set).
- **Why `b:1`:** UA↔JA4↔H2 incoherence and a SwiftShader-class WebGL
  string are *named* server-side triggers [MECH asadfix 2026 /
  ScrapeBadger 2026]. But master plan §8.5 G7 credibly argues 147≡148
  is wire-cosmetic — so **do not speculatively re-pin**; K2 decides.
- **Difficulty:** drift-guard **S**; a real re-pin/GPU correction **M**.
- **Risk:** drift-guard ~zero (no byte change). A speculative re-pin is
  *high* risk (reverts a tested UA=148 decision that recovered
  homedepot/hotels/leboncoin) — **only act on a measured K2 delta**.
- **Verification:** drift-guard is **§4-gate-verifiable**; any actual
  re-pin/GPU change requires a fresh K2 re-measure.

### K5 — Session warming, *only after K1-K4 close* — [HYP, LOW-MED]
- **What:** A/B a warmed nav sequence (homepage → intermediary →
  target, 2-5 s human-jittered gaps [MECH Scrapfly/roundproxies]) vs a
  single cold GET, in-engine.
- **Why `b:1`:** trust scores decay / cold deep-GETs score worse
  [MECH Scrapfly 2026]. **But** `nocdp.sh` real Chrome passes with a
  *single cold GET* from this IP ⇒ warming is **not necessary for a
  genuine browser**; it only amplifies an already-borderline score.
- **Difficulty:** S-M. **Risk:** low. **Verification:** live-oracle
  only; meaningless until K3 makes the behavioral block non-empty
  (warming a `b:1` headless FP won't rescue it — order matters).

---

## 4. What 2026 intel *confirms we already did right* (don't redo)

- **[MECH] asadfix 2026 verbatim:** Kasada "specifically tests dozens
  of native functions" via `Function.prototype.toString.call(getter)`
  and "has its toString() signatures catalogued and blocks
  playwright-stealth outright." Our **genuine-native
  `Function.prototype.toString`** (`js_runtime/src/native_fns.rs:130`,
  WIRED) is the *structurally correct* fix — externally re-confirmed.
  Do not regress it; do not re-investigate it.
- **[MECH] The header protocol** (`x-kpsdk-ct` ≈30 min/1000 pts reuse,
  `x-kpsdk-cd` single-use/50 pts/<5 s, `st` binds ct↔cd, `x-kpsdk-h`
  HMAC, `x-kpsdk-v` version pin) is **stable across all 8 sources** ⇒
  our `kasada_session.rs` wire-model shape is correct. The PoW
  algorithm (`kasada.rs`) is byte-correct & self-replay-validated.
- **The realm/sentinel/identity line is CLOSED** (Phase 2 OUTCOME A +
  clean 80/80 sentinel). 2026 intel adds nothing to reopen it; it
  points entirely at behavioral/TLS/GPU — orthogonal surfaces. Do not
  resurrect (master plan §6, `99` FP-F3).

---

## 5. The honest verdict

**Passable engine-only? No — not as a code commit, and the 2026 intel
makes this a *strong* conclusion, not a hedge.** Every reproducible 2026
Kasada pass runs a real browser binary or a paid farm; no open `ct`
algorithm and no open `b:1` recipe exists *because no OSS tool faces the
from-scratch problem*. Our client already passes (`action:"allow"`); the
residual `b:1` is server-scored on the two surfaces the client verdict
does not clear — behavioral entropy (which our navigate path provably
emits **zero** of — the single most actionable finding) and JA4/H2/GPU
coherence (likely-clean-but-unmeasured-vs-Kasada).

**What that means concretely:** the realistic path is a **2-3 item
capability build (K1 hygiene, K2 live-oracle regime, K3 behavioral
wiring), measurement-gated, multi-session** — *not* a single flip. The
mandatory network-free §4 gate **structurally cannot** verify a Kasada
pass (server-computed from live telemetry); only K2's authorized
live-oracle differential can. If K2 shows **no** behavioral/TLS delta
vs `nocdp.sh` real Chrome, the honest close-out is "residual is
IP-regime/trust-decay weight — needs warming/residential infra, not
stealth code" (and `nocdp.sh` passing argues that close-out is
*unlikely* but it must be measured, not assumed). The valuable
deliverable of the next session is **K2's named delta**, never a
fabricated flip.

---

## 6. Top-3 concrete actions (the answer)

1. **K1 — gate the parallel Rust PoW off when ips.js self-solves**
   (S, low risk, **§4-gate-verifiable** — the only item the offline gate
   can fully prove; removes a self-inflicted `b:1`/replay candidate
   before measurement). = `99` FP-K1, P1.
2. **K2 — build the live-oracle differential `/tl` capture** (M; a
   regime not a commit) — passive `/tl`-scoped hook (clean
   `kasada_error_blob_capture` pattern, **no Function wrapper**), run in
   our engine AND `ab_harness/nocdp.sh` real Chrome same-site/same-IP,
   diff the behavioral block + server-visible JA4/H2. This converts
   "holistic tail" into a named, ranked, falsifiable delta. **Nothing
   below is verifiable without it.**
3. **K3 — wire `stealth::behavior` into `Page::navigate` before `/tl`
   fires** (M, medium risk via Akamai-sensor coupling — use additive
   `op_behavior_*` wrappers, §4-gate the wiring). Directly attacks the
   **#1 named 2026 production trigger** ("zero behavioral variance");
   our navigate path provably emits none. Sequence around K2 to measure
   the empty→entropic transition.

---

## 7. Sources (URL — claim used — accessed 2026-05-16)

**External (web research this session):**
- `https://scrapfly.io/blog/posts/how-to-bypass-kasada-anti-scraping-waf`
  — 4-stage detection (TLS-JA3 / IP / HTTP / JS-FP); session warming
  (homepage→intermediary→target, 2-5 s); curl-impersonate "won't bypass
  the JS challenges"; VM emulators break in days; tool list (Nodriver/
  SeleniumBase-UC/Camoufox = real browsers). **Accessed 2026-05-16.**
- `https://scrapebadger.com/kasada-bypass` — p.js VM bytecode (PoW +
  FP + behavioral); ct "expensive session" vs cd "cheap per-request,
  stale/replay → immediate block"; **named triggers: "zero behavioral
  variance — no mouse jitter, identical scroll timing" + "headless
  browser GPU — WebGL renderer reveals SwiftShader"**; behavioral data
  (mouse/scroll/keyboard) folded into the encrypted token; PoW ~2 ms in
  real browser, cold datacenter "100x harder"; **"real-browser
  execution only" — no durable OSS tool. Accessed 2026-05-16.**
- `https://asadfix.github.io/scraping-guide/` — **verbatim:** Kasada
  "specifically tests dozens of native functions" via
  `Function.prototype.toString.call(getter)` and "has its toString()
  signatures catalogued, blocks playwright-stealth outright";
  **"UA says Chrome but JA4 says Python urllib3 → the mismatch is the
  signal"**; PatchRight = source-level patch (nothing in JS runtime to
  toString-inspect); paid infra for scale. **Accessed 2026-05-16.**
- `https://scrapeway.com/anti-bot-services/kasada` — tested 2026
  benchmark: Scrapfly **99%** / WebScrapingAPI 98% on **realtor.com**
  (only Kasada target benchmarked); **all passing solutions are paid
  hosted APIs, no OSS alternative**; success variance 99%→0%.
  **Accessed 2026-05-16.**
- `https://github.com/Hyper-Solutions/hyper-sdk-go` — [MEAS WebFetch]
  **thin client to a paid hosted API** ("get your API key from the
  dashboard"); `GenerateKasadaPayload()`/`GenerateKasadaPow()` are
  remote calls; **no local algorithm exposed**. **Accessed 2026-05-16.**
- `https://github.com/nixbro/Kasada-Solver` — [MEAS WebFetch]
  **documentation only, no runnable solver code**; confirms ct ≈30 min
  reusable / cd single-use <5 s / `x-kpsdk-h` HMAC binds ct↔cd; "no
  code, no solver, no working implementation". **Accessed 2026-05-16.**
- `https://substack.thewebscraping.club/p/bypassing-kasada-2025-open-source`
  — standard Playwright "miserably fails" (blank screen); Patchright =
  working alternative (a **real browser**); author prefers human-like
  real-browser traffic over token reversal; full script paywalled.
  **Accessed 2026-05-16.**
- `https://www.zenrows.com/blog/kasada-bypass` &
  `https://roundproxies.com/blog/bypass-kasada/` &
  `https://www.kasada.io/solver-services-fraudsters-bypass-bot-management/`
  — corroborate: weighted trust score; behavioral telemetry
  (coordinate sequences / scroll accel / click intervals / keystroke)
  folded into classifiers; Kasada **regenerates detection logic per
  page load** (breaks static RE); vendor strategy = black-box +
  per-load randomization. No new lever; no site-specific weakness for
  canadagoose/hyatt/realtor. **Accessed 2026-05-16.**
- WebSearch (2026-05-16): "canadagoose/hyatt/realtor Kasada 2026" — no
  public site-specific bypass; realtor.com only via paid real-browser
  farms; canadagoose & hyatt confirmed Kasada tenants, no tenant-level
  weakness reported.

**Internal (read first-hand this session):**
- `docs/research/engines/kasada.md` (full), `99_CODE_FALSE_POSITIVES.md`
  (full), `README.md` (full), `docs/research_2026_05_16/00_MASTER_PLAN.md`
  §1 + §8.5.
- `crates/stealth/src/kasada.rs` (full — byte-correct PoW,
  self-replay-validated, `solve_with_realistic_duration`).
- `crates/net/src/kasada_session.rs` (full — `compute_cd_header`
  WIRED + its own `UNVERIFIED-VS-LIVE` doc-comment :282-290; parallel
  to in-V8 self-solve = L1/FP-K1).
- `crates/stealth/src/behavior.rs` (header — full Plamondon/
  Sigma-Lognormal model exists) vs `crates/browser/src/page.rs:1214-
  1246` (`Page::navigate` injects **only `humanize.js`** — the rich
  model is **NOT wired**; K3 gap confirmed in code).

**No heavy `cargo` run** (per constraints; the flip is server-scored,
no cheap network-free decisive test exists — K2 is the regime that
replaces it).
