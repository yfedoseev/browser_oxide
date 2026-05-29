# VENDOR — Akamai Bot Manager + sec-cpt (homedepot, bestbuy, adidas)

**Scope of this doc:** the Akamai cluster on the 126-corpus — homedepot
(sec-cpt, flaky ~60% BO win, v150 fails, Patchright passes), bestbuy
(SPA shell, passes NO from-scratch engine, Patchright passes), adidas
(firefox-only BO pass). Covers the sensor_data POST, the `_abck` cookie
state machine, the sec-cpt challenge bundle (BMP), and the pixel/cd
challenge surface.

**Audience:** the next engineer on the v0.2.0 close-v150-gap drive.

**Reading order:** this doc extends, it does not replace —
- `docs/releases/v0.1.0-parity/26_AKAMAI_BMP_DEEP.md` (the canonical mechanism deep dive)
- `docs/vNext/01_R-AKAMAI-SECCPT-FLAKE.md` (homedepot ticket)
- `docs/vNext/03_R-BESTBUY-AKAMAI.md` (bestbuy ticket)
- `docs/HANDOFF_2026_05_28b.md` §4 (the live-nav drain root cause — load-bearing for THIS doc)

---

## 0. TL;DR — the new synthesis

Three findings this writeup adds on top of the existing repo docs:

1. **homedepot flakiness is NOT (only) "daily-rotated bundle obfuscation".**
   Per Hyper Solutions' protocol docs, Akamai sec-cpt has **three
   providers** — `crypto` (pure in-VM PoW + enforced `chlg_duration`
   wait), `behavioral` (needs a sensor_data POST, no PoW), and
   `adaptive` (BOTH PoW *and* sensor data). The provider is selected
   per-challenge by Akamai's risk engine and rotates. BO's public engine
   can self-solve **only the `crypto` provider** (the bundle runs the
   PoW in V8). On `behavioral`/`adaptive` days BO has no sensor encoder
   in the public tree (it lives in `vendor_solvers`, and the empty
   `default_solvers()` set means it never runs) → BO fails. **That is
   the real ~60%/~40% split**, not just obfuscation rotation. (New —
   `26_AKAMAI_BMP_DEEP.md` treated sec-cpt as a single crypto-PoW flow.)

2. **The single biggest reliability lever for the `crypto`-provider days
   is the same lever HANDOFF_2026_05_28b §4 found for AWS WAF: the 50 ms
   inter-script `run_until_idle` drain at `page.rs:1678`.** The sec-cpt
   bundle's PoW + the enforced `chlg_duration` wait + the reload all
   happen on `setTimeout`/promise-chain continuations that need the page
   to stay alive *between and after* scripts. A 50 ms drain truncates
   them; the bundle then re-solves on a *later* iteration only if the
   wall-clock budget happens to stack (the b623d5d flip was observed at
   nav_ms ≈ 119 s — "budget-luck", exactly as `page.rs:1933` admits).
   This is **one shared fix** that lifts both the AWS cluster and the
   homedepot crypto-provider reliability.

3. **Why Patchright passes homedepot + bestbuy cleanly and BO/v150
   don't** is now precisely attributable: Patchright is **real Chromium
   with a real event loop**, driven over CDP with `Runtime.enable`
   removed (so Akamai's CDP-leak probe sees nothing) and execution-context
   isolation. The page's own JS — the sec-cpt bundle, the bestbuy React
   hydration — runs to completion with native timing. BO's gap is not
   fingerprint and not a missing API; it is **async-execution fidelity**:
   BO truncates the page's own async work with fixed-duration drains.
   This reframes both `01_R-AKAMAI-SECCPT-FLAKE` and `03_R-BESTBUY-AKAMAI`
   as instances of the same engine class (self-solve-execution), the
   same class as the AWS cluster.

---

## 1. What the existing repo docs already concluded (cited)

### 1.1 `26_AKAMAI_BMP_DEEP.md`

- **Sub-product taxonomy** (§2.1): BMP v2 (TEA-CBC, 58-element `tAD`
  array), BMP v3 (post-2024 PRNG-shuffled JSON keyed off a `bm_sz`-derived
  cookieHash), sec-cpt (HTTP 428 PoW), BM Edge (`bm_sz`-only server-side
  scoring), Akamai 1.7X legacy (`ak_bmsc`). Corpus hits BMP v2/v3 +
  sec-cpt.
- **`_abck` cookie state machine** (§2.2): `~-1~-1~-1~` = Favorable,
  `~0~-1~-1~` = Untrusted, `~0~0~` = Provisional, `~3~` = Rejected.
  Important correction from commit `24c19e3`: slot 1 is a **stop-signal
  threshold** (count of sensor POSTs Akamai accepts), NOT a trust toggle.
- **sec-cpt flow B** (§2.4): 428 JSON `{token, timestamp, nonce,
  difficulty, count, timeout, cpu, verify_url}`; the bundle brute-forces
  a base-16 float `r` such that a rolling-hash reduction `output % (difficulty+i) == 0`;
  waits `chlg_duration`; POSTs the answer; server sets `sec_cpt=~3~`;
  bundle reloads. **Conclusion: the bundle self-solves in V8 — public
  needs NO Rust PoW solver** as long as the bundle is allowed to run to
  completion.
- **Vendor boundary** (§3, §6, §8): the `aecdf19` strip removed all 9
  Akamai files (sensor_data v2/v3 encoders, TEA-CBC, sec-cpt PoW,
  session state machine, DRAIN_JS). The public engine keeps cookie/URL
  recognition + body-marker classifier rows + the `ChallengeSolver`
  seam; the encoders and per-tenant secrets stay in `vendor_solvers`.
- **adidas firefox-only-win** (§4.1): all 4 BO profiles + Camoufox sit
  at the 2494-byte interstitial; only `firefox_135_macos` flips to the
  1.3 MB body. Hypotheses ranked: (a) TLS class, (b) `navigator.vendor=""`
  + `productSub=20100101`, (c) no `sec-ch-ua-*` headers, (d) Mozilla-masked
  WebGL. Not yet bisected to the single field.
- **bestbuy** (§4.3): ~7-8 KB "Choose a country" i18n splash served to
  every engine; classified as a benign content-routing splash, NOT an
  Akamai block; declared out of the chapter-26 critical path.

### 1.2 `01_R-AKAMAI-SECCPT-FLAKE.md`

- b623d5d's `handle_akamai_flow` defensively suppressed the BMP POST on
  sec-cpt pages so the bundle was the sole actor → homedepot flipped.
  After the `ChallengeSolver` refactor, that suppression only fires when
  a vendor solver is loaded; the public sweep (`default_solvers()` empty)
  never triggers it.
- The bundle still runs in V8 but doesn't complete its self-solve. The
  ticket lists 3 candidate missing primitives: a cookie-observation hook
  on `sec_cpt → ~3~`, a fetch-interception hook for the verify POST, or a
  broken V8/DOM surface. **Prescribed method: an `awswaf_probe`-class
  offline oracle** to find the bailout.

### 1.3 `03_R-BESTBUY-AKAMAI.md`

- bestbuy edge hard-RSTs naked curl (HTTP/2 `INTERNAL_ERROR`); BO's
  chrome_147 TLS gets past the edge to a 7 KB React shell that never
  hydrates; Patchright passes at 1246 KB. Classified Stratum B (Patchright
  passes, neither BO nor v150). Candidate differentials: (a) TLS/HTTP-2
  SETTINGS-frame delta vs Chrome 148, (b) behavioral-signal gap in the
  SPA bootstrap, (c) a JS surface where BO ≠ real Chrome.

### 1.4 HANDOFF_2026_05_28b §4 (load-bearing)

- The AWS WAF cluster gap is **not fingerprint, not WASM** — it is that
  the page's own async self-solve runs in the offline oracle (5 s
  `run_until_idle`) but produces **zero async progress in the live
  navigate path** because `build_page_with_scripts_init_and_storage`
  only does a **50 ms inter-script drain** (`page.rs:~3535`, current HEAD
  `page.rs:1678`). The same root cause applies to homedepot's sec-cpt
  crypto provider (see §4 below).

---

## 2. New external findings (cited)

### 2.1 sec-cpt has THREE providers — the missing axis

Source: [Hyper Solutions — Handling 428 (SEC-CPT)](https://docs.hypersolutions.co/akamai-web/handling-428-status-code-sec-cpt)
and [hyper-sdk-py `akamai/sec_cpt.py`](https://github.com/Hyper-Solutions/hyper-sdk-py/blob/master/hyper_sdk/akamai/sec_cpt.py).

The 428 challenge response (HTML iframe `id="sec-cpt-if"` with a
base64-encoded `challenge` attribute + `data-duration`) selects one of
three providers:

| Provider | Steps | Needs sensor_data? | BO public self-solve? |
|---|---|---|---|
| **crypto** | wait `chlg_duration` (server-enforced) → POST `{token, answers}` to `/_sec/verify?provider=crypto` → GET `/_sec/cp_challenge/verify` → `sec_cpt=~3~` | NO (pure PoW) | **YES** if drained long enough — the bundle does PoW in V8 |
| **behavioral** | fetch branding page (`branding_cust_url`) → fetch Akamai script endpoint → POST sensor data (1-3 POSTs) → GET dynamic `verify_url` → `sec_cpt=~3~` | **YES** | **NO** — needs the (private) sensor encoder |
| **adaptive** | wait `chlg_duration` → POST PoW to `/_sec/verify?provider=adaptive` (count answers) → fetch branding + submit sensors → GET `/_sec/cp_challenge/verify` → `sec_cpt=~3~` | **YES (+ PoW)** | **NO** — needs both |

**This is the single most important new fact in this doc.** It explains
why homedepot is ~60% in BO and not 100% even on "good" days, and why
hardening alone (cleaner `is_seccpt_solved`) can't push past ~the
crypto-provider fraction: on `behavioral`/`adaptive` provider days the
public engine *structurally* cannot solve it without the private
sensor encoder. `26_AKAMAI_BMP_DEEP.md` §2.4 modeled sec-cpt as a single
crypto flow and so over-promised "+1 site deterministic".

### 2.2 Patchright = real Chromium, real event loop, no CDP leak

Source: [deepwiki Kaliiiiiiiiii-Vinyzu/patchright](https://github.com/Kaliiiiiiiiii-Vinyzu/patchright),
[anti-detect-browser-tools-tech-comparison/patchright.md](https://github.com/pim97/anti-detect-browser-tools-tech-comparison/blob/master/patchright.md),
[scrapewise: Playwright stealth 2026](https://scrapewise.ai/blogs/playwright-stealth-2026).

Patchright is a drop-in Playwright fork that patches **the Chromium
binary / driver**, not the page:
- **Removes `Runtime.enable`** (the CDP command Akamai/Cloudflare/Kasada/
  DataDome all probe for) — it manages execution contexts manually via
  `globalThis` evaluation + `CRExecutionContext`.
- Disables the Console domain (`Console.enable` leak), pierces closed
  shadow roots, strips `--enable-automation` / adds
  `--disable-blink-features=AutomationControlled`.
- **Crucially: it does NOT modify the page's timing or event-loop
  drain.** The page's own JS — the sec-cpt bundle, bestbuy's React
  hydration — runs in real Chromium with native scheduling and runs to
  completion. (deepwiki notes the only timing-attack surface is init
  script injection, "low risk".)

So Patchright passes homedepot/bestbuy for two compounding reasons: (1)
no CDP detection signal, and (2) **perfect async-execution fidelity**.
BO has reason (1) covered (no CDP at all — it's a from-scratch engine,
not driving Chrome), but **lacks reason (2)**: BO truncates the page's
own async work. This is the precise, code-level reframe of both
`01_R-AKAMAI-SECCPT-FLAKE` and `03_R-BESTBUY-AKAMAI`.

> Note the homedepot inversion (`26 §4.2`): even CDP-detectable vanilla
> Playwright PASSES homedepot at 1+ MB. Akamai's homedepot tenant
> *trusts real Chrome* and punishes anything that isn't perfectly real
> Chrome — including BO's truncated execution. The lever is execution
> fidelity, not stealth.

### 2.3 sec-cpt protocol constants (for the classifier + the private solver)

- Success marker is universally `sec_cpt` cookie containing `~3~`
  (confirmed across all 3 providers) — matches BO's `is_seccpt_solved`
  at `page.rs:242-247`.
- The challenge iframe is `id="sec-cpt-if"` / container `sec-if-cpt-container`
  — matches BO's markers at `page.rs:1857-1858` and `classify.rs:130-131`.
- The crypto PoW endpoint is `/_sec/verify?provider=crypto`; the static
  verify GET is `/_sec/cp_challenge/verify` — matches BO's classifier
  row at `classify.rs:84`.
- `chlg_duration` (the `data-duration` attribute, seconds) is
  server-enforced and cannot be shortened. This is the floor on
  homedepot's wall-clock — BO's nav budget MUST exceed it (see §4.2).

### 2.4 Other Akamai references (already in 26, re-confirmed live)

- xiaoweigege/akamai2.0-sensor_data (v2 reference), Edioff/akamai-analysis
  (signal taxonomy), glizzykingdreko's v3 Medium walkthrough — all
  still the canonical public deobfuscation references; all are
  research-reference-only per CLAUDE.md (no code copy into public crates).

---

## 3. BO code-level analysis (file:line, current HEAD)

### 3.1 What HEAD has that doc 26 did not (doc 26 is partly stale)

`26_AKAMAI_BMP_DEEP.md` was written against an earlier tree. Since then
**`is_seccpt_solved` was already added** (R-AKAMAI-SECCPT-FLAKE Sprint
2.4) at `crates/browser/src/page.rs:242-247`:

```rust
fn is_seccpt_solved(cookies: &str, body: &str) -> bool {
    cookies.contains("sec_cpt=")
        && cookies.contains("~3~")
        && !body.contains("sec-if-cpt-container")
        && !body.contains("sec-cpt-if")
}
```

Wired into the poll-loop break at `page.rs:2254-2276` (gated by
`started_as_seccpt_challenge`, so zero non-sec-cpt regression). This is
the public-engine "recognize the success state even with no solver"
primitive that doc 26 §3.A/§3.B prescribed — **it already shipped.** So
the remaining gap is NOT recognition; it is making the bundle reach
`~3~` in the first place.

### 3.2 The drain truncation — the actual blocker (page.rs:1678)

`build_page_with_scripts_init_and_storage` runs external + inline
scripts in document order with a **fixed 50 ms** `run_until_idle`
between each (`page.rs:1676-1679`):

```rust
let _ = self
    .event_loop
    .run_until_idle(Duration::from_millis(50))
    .await;
```

followed by a single **500 ms** final drain (`page.rs:1728-1731`). The
sec-cpt crypto bundle's continuation chain is: install listeners →
`setTimeout(chlg_duration*1000)` enforced wait → compute PoW (the bundle
may also offload to a worker, like AWS) → fetch the verify endpoint →
`location.reload()`. **None of that fits in 50 ms + 500 ms.** The bundle
only completes if a *later* navigate iteration's per-iteration drain
(`page.rs:2051-2055`, floored at 8 s, capped by `nav_budget`) happens to
give it enough contiguous wall-clock — which is the "budget-luck" the
code comments at `page.rs:1933-1937` openly acknowledge.

The per-iteration drain budget for homedepot is set to 45 s at
`page.rs:1938` (vs 25 s for plain BMP and 15 s default). That is the
right order of magnitude (it must exceed `chlg_duration`, typically
5-30 s), but it only applies to the *navigate-loop* drain — the
*warm-rebuild* path (`build_page_with_scripts_init_and_storage`, used
when a pending nav / reload fires) still truncates at 50 ms/500 ms.
**When the bundle's own `location.reload()` triggers a warm rebuild, the
re-served sec-cpt bundle is truncated again** → the ~3~ transition is
lost → the loop retries from scratch → flaky.

### 3.3 The `__akamai_events` behavioral surface exists but is unfed for sec-cpt

`humanize.js` (`crates/browser/src/js/humanize.js:69-99`) installs
`globalThis.__akamai_events` with `mouse/key/touch/scroll` buffers +
`counters`. `page.rs:1362-1372` resets them across navigations. This is
the profile-neutral surface doc 26 §8 kept in public for a private
solver to harvest. **For the sec-cpt `behavioral`/`adaptive` providers,
this is exactly the input the (private) sensor encoder would read.** But:
(a) no public solver consumes it, and (b) on the warm/sec-cpt path the
events generated are minimal (humanize.js fires synthetic moves on a
timer; `page.rs:341` notes the buffer may have "only 1-2" entries). For
`behavioral` provider days, a richer event stream is a prerequisite even
for a private solver to succeed.

### 3.4 bestbuy — not Akamai-challenge, it's hydration + edge TLS

`classify.rs:548-551` documents the bestbuy 7.9 KB "Choose a country"
splash as `akam/13`-bootstrap-only with NO challenge co-signal → it
correctly does NOT classify as Akamai-CHL (`classify.rs:127-134`
`AKAMAI_CHALLENGE_COSIGNAL` requires `sec-if-cpt-container` / `sec-cpt-if`
/ `bm-verify` / `pardon our interruption`). So bestbuy is **not in the
sec-cpt path at all.** Per `03_R-BESTBUY-AKAMAI` + §2.2 above, the
bestbuy gap is the React SPA shell never hydrating — which is again the
same **async-execution-fidelity** class (the bootstrap's fetch-chain +
hydration needs a real drain, not 50 ms), possibly compounded by an
edge-tier TLS/HTTP-2 SETTINGS delta vs Chrome 148.

### 3.5 The `started_as_seccpt_challenge` flag + budget tier (correct, present)

- Detection: `page.rs:1857-1858` (`sec-if-cpt-container` / `sec-cpt-if`).
- Budget tier: `page.rs:1938` homedepot → 45 s (vs 25 s plain BMP).
- Poll-break on `~3~`: `page.rs:2254-2276`.
- BMP-POST suppression on sec-cpt: `page.rs:2332-2336` (dead with empty
  solvers, kept for the private re-add — correct).
- Trace: `page.rs:2381-2394` (`BROWSER_OXIDE_SC_TRACE` / `DEBUG_NAV`).

All the recognition + budget scaffolding is right. The missing piece is
purely: **let the bundle's async self-solve actually run to completion
on every relevant path.**

---

## 4. Concrete hardening plan for homedepot sec-cpt reliability

Ordered by ROI. The goal: take homedepot from ~60% → reliable on
**crypto-provider days** (the structural ceiling without the private
sensor encoder), and surface `behavioral`/`adaptive` days as a clean
NODATA rather than a silent flaky fail.

### Fix 1 (highest ROI, shared with AWS) — replace the fixed 50 ms/500 ms warm-rebuild drain with an adaptive challenge-aware drain

**What:** in `build_page_with_scripts_init_and_storage` (`page.rs:1657-1731`),
when the response body carries a sec-cpt marker (or any
`is_challenge_document_response` shape, reusing the AWS/DataDome
predicate), extend the post-script + final drain to a budget comparable
to the navigate-loop drain (≥ `chlg_duration` + PoW + reload, e.g.
30-45 s with a `V8DeadlineWatcher` hard cap), instead of 50 ms/500 ms.
For non-challenge pages keep the fast 50 ms drain (zero perf regression
on the 99% fast-site majority).

**Why it works:** lets the crypto bundle finish its
`setTimeout(chlg_duration)` → PoW → verify POST → reload chain on the
warm-rebuild path, so the `~3~` transition is not lost on the bundle's
own reload. This is the **same lever** HANDOFF_2026_05_28b §5.1 prescribes
for the AWS cluster — implement once, harvest both.

- **Effort:** 2-3 days (the predicate already exists; the risk is not
  regressing fast-site latency — gate strictly on the challenge marker +
  use the existing `V8DeadlineWatcher` cap).
- **Expected impact:** homedepot crypto-provider days go reliable
  (~60% → the crypto fraction, est. 90%+ of crypto days); AWS cluster
  (imdb, amazon-in/fr/jp/com-au) advances in lockstep; duolingo; likely
  booking. This is the single biggest multi-site lever in the corpus.
- **Confidence:** high (root cause is measured in §4 of the handoff; the
  oracle already runs challenge.js fully under a 5 s drain).
- **Public engine:** YES.

### Fix 2 — build the sec-cpt offline oracle (R-AKAMAI-SECCPT-FLAKE Step 1-3) to *confirm* provider + bailout

**What:** fork `crates/browser/examples/awswaf_probe.rs` →
`seccpt_probe.rs`: load a captured homedepot 428 response via
`Page::from_html_with_url`, pre-inject the instrumentation Proxy, run
`run_until_idle(30s)`, dump the access trace + `document.cookie` +
`__scriptErrors` + whether the bundle spawned a worker / fired the verify
POST. Decode the base64 `challenge` attribute to read the **provider**
field directly.

**Why:** confirms Fix 1's hypothesis (under a long oracle drain the
crypto bundle reaches `~3~`) and, decisively, **reads which provider
the day's challenge uses** — the §2.1 axis that's currently invisible.
This is the difference between "we can self-solve this" and "this needs
the private encoder".

- **Effort:** 1-2 days (oracle template exists).
- **Expected impact:** 0 direct flips; it is the diagnostic that
  validates Fix 1 and classifies the flaky-fail days. Prerequisite for
  trusting the hardening.
- **Confidence:** high.
- **Public engine:** YES (oracle is a dev tool).

### Fix 3 — make the homedepot reload a same-isolate continuation, not a fresh warm rebuild

**What:** when the sec-cpt bundle calls `location.reload()` after setting
`sec_cpt=~3~`, ensure the next GET carries the upgraded cookie AND that
the engine does not throw away the bundle's still-pending continuations.
Today the reload routes through the warm-rebuild path which re-truncates
(Fix 1 addresses the drain; Fix 3 ensures the *cookie* is read at the
exact reload boundary). Verify `is_seccpt_solved` is checked at the
top of the post-reload iteration (it is, `page.rs:2254`), and that
`cookies_for_url` is re-read after the bundle's `document.cookie` write
lands (the 50 ms drain may also truncate the cookie write — Fix 1 covers
this too).

- **Effort:** 1 day (mostly verification on top of Fix 1).
- **Expected impact:** removes the residual "solved but the engine
  didn't notice" flake. Part of pushing crypto days to ~95%.
- **Confidence:** medium (depends on Fix 1 landing first).
- **Public engine:** YES.

### Fix 4 — feed `__akamai_events` a richer pre-solve event stream

**What:** before the sec-cpt bundle runs, ensure `humanize.js` has
populated `__akamai_events` with a plausible mouse trajectory + a few
key/scroll events (it currently may have "only 1-2" per `humanize.js:341`).
This is a public-engine behavioral-realism improvement (it's the engine
generating its own synthetic input, not a vendor bypass).

- **Effort:** 1-2 days.
- **Expected impact:** prerequisite for any future `behavioral`/`adaptive`
  provider solve (which lives in `vendor_solvers`); 0 direct public flips
  but unblocks the private path and may marginally improve `_abck`
  scoring on plain-BMP sites (adidas/bestbuy SPA).
- **Confidence:** medium.
- **Public engine:** YES (the event generation); the encoder that reads
  it is `vendor_solvers`.

### Fix 5 — `_abck` trust-state parser `parse_abck_trust` (doc 26 §3.A, NOT yet landed)

**What:** add `AbckTrust` enum + `parse_abck_trust` to `classify.rs`
(doc 26 §3.A spec) and gate the cookie-delta retry on
`AbckTrust != Favorable`. Confirmed NOT present at HEAD (only the
`_abck`/`bm_sz` *classifier* rows exist, `classify.rs:104`).

- **Effort:** 0.5 day (pure parser + unit tests).
- **Expected impact:** 0 sites in isolation (correctness preservation);
  prevents the retry loop from spinning on every failing sensor POST
  once a private BMP solver is registered. Low priority until the
  private solver is re-added.
- **Confidence:** high.
- **Public engine:** YES.

### Fix 6 (vendor_solvers, the structural ceiling) — re-add the sec-cpt behavioral/adaptive sensor encoder

**What:** in the private `vendor_solvers` crate, port the pre-strip
sensor_data v2/v3 encoder + the sec-cpt `behavioral`/`adaptive` flow
(branding-page fetch → sensor POST → dynamic verify). Register via
`Page::with_solvers(vendor_solvers::default_solvers())`. The public
engine already exposes everything it needs: the `ChallengeSolver` seam
(`challenge.rs:55-161`), the `__akamai_events` surface, the
`started_as_seccpt_challenge` flag, and the `is_seccpt_solved` break.

- **Effort:** 3-5 days (port + byte-parity test against the glizzy/xiaoweigege
  reference, gated privately).
- **Expected impact:** flips homedepot on `behavioral`/`adaptive`
  provider days — the ~40% the public engine structurally cannot reach.
  This is what closes homedepot to 100% and what would push it past
  Camoufox v150 reliably (v150 has no Akamai encoder either).
- **Confidence:** medium (rotating bundle obfuscation means the encoder
  needs maintenance; per-tenant fileHash registry rotates ~weekly).
- **Public engine:** NO — `vendor_solvers` (forbidden in public crates
  per CLAUDE.md / `aecdf19` policy).

### bestbuy (separate track, §2.2 / §3.4)

Not a sec-cpt site. Lever is the same async-execution-fidelity (Fix 1
class applied to the SPA-hydration fetch-chain) plus a possible
chrome_147→chrome_148 TLS/HTTP-2 SETTINGS refresh (`net/src/tls.rs`).
Diagnose with a BO-vs-Patchright packet diff (R-BESTBUY-AKAMAI Step 2)
**after** Fix 1 lands — the hydration may simply complete once the drain
is long enough. Effort 2-5 days, confidence low until the packet diff is
done, public engine (TLS) + possibly behavioral.

---

## 5. Classification of the three sites

| Site | Class | Lever | Engine |
|---|---|---|---|
| homedepot (crypto-provider days) | self-solve-execution (drain truncation) | Fix 1 + 3 | public |
| homedepot (behavioral/adaptive days) | JS-gap (missing sensor encoder) | Fix 6 | vendor_solvers |
| bestbuy | self-solve-execution (SPA hydration) + possible edge TLS | Fix 1 class + TLS | public (+ behavioral) |
| adidas | per-profile fingerprint scoring (firefox-only flip) | route-to-firefox now; bisect field later (26 §4.1) | public (stealth) |

**Direct answer to "bestbuy passes NO engine — classify it":** it is
**behavioral-signal + JS-gap, NOT frontier.** Patchright passing proves
it is Chromium-engine-reachable (not an unsolvable frontier). The
from-scratch engines (BO + v150) fail because the React SPA never
hydrates under their execution model — the same async-execution-fidelity
class as homedepot's crypto days, with a possible edge-TLS contributor.
It is solvable in the public engine via the Fix 1 drain class; it does
not require a vendor solver.

---

## 6. Ranked fix list (ROI order) — summary

1. **Fix 1** — adaptive challenge-aware warm-rebuild drain (shared with
   AWS). 2-3 d, high conf, public. Biggest multi-site lever.
2. **Fix 2** — sec-cpt offline oracle (confirm provider + bailout). 1-2 d,
   high conf, public (dev tool).
3. **Fix 3** — same-isolate reload + cookie read at boundary. 1 d, med
   conf, public.
4. **Fix 6** — vendor_solvers behavioral/adaptive sensor encoder (the
   ceiling). 3-5 d, med conf, vendor_solvers.
5. **Fix 4** — richer `__akamai_events` stream. 1-2 d, med conf, public.
6. **Fix 5** — `parse_abck_trust` (low priority until private solver). 0.5 d,
   high conf, public.
7. **bestbuy track** — Fix 1 class + chrome_148 TLS refresh + packet diff.
   2-5 d, low conf, public.

---

## 7. Open questions

- **Q1:** What fraction of homedepot 428s are crypto vs behavioral vs
  adaptive? (Resolve with Fix 2's oracle decoding the base64 `challenge`
  attribute over several days.) This sets the public-engine ceiling.
- **Q2:** Does the sec-cpt crypto bundle offload its PoW to a blob-URL
  Web Worker (like AWS challenge.js)? If so, the worker secure-context
  fix (commit `5216336`) is also a prerequisite here — verify in the
  oracle.
- **Q3:** Does Fix 1 alone flip homedepot deterministically on a
  crypto-provider day, BEFORE any vendor_solvers re-add? (Doc 26 §Q26.2
  hypothesized yes; §2.1's provider split says "yes, but only on crypto
  days".)
- **Q4:** Is bestbuy's non-hydration purely drain (fixed by Fix 1) or
  does it also need a chrome_148 TLS/HTTP-2 SETTINGS refresh? (R-BESTBUY
  Step 2 packet diff, after Fix 1.)
- **Q5:** Which single fingerprint field flips adidas on firefox only?
  (Doc 26 §4.1 a/b/c/d still un-bisected; the §4.1 capture diff is the
  experiment.)

---

## 8. Sources

External:
- [Hyper Solutions — Handling 428 Status Code (SEC-CPT)](https://docs.hypersolutions.co/akamai-web/handling-428-status-code-sec-cpt) — the three-provider taxonomy (NEW load-bearing finding)
- [hyper-sdk-py — akamai/sec_cpt.py](https://github.com/Hyper-Solutions/hyper-sdk-py/blob/master/hyper_sdk/akamai/sec_cpt.py)
- [hyper-sdk-go akamai package](https://pkg.go.dev/github.com/Hyper-Solutions/hyper-sdk-go/akamai)
- [Kaliiiiiiiiii-Vinyzu/patchright](https://github.com/Kaliiiiiiiiii-Vinyzu/patchright) — Runtime.enable removal + execution-context isolation + real-event-loop fidelity
- [pim97/anti-detect-browser-tools-tech-comparison — patchright.md](https://github.com/pim97/anti-detect-browser-tools-tech-comparison/blob/master/patchright.md)
- [scrapewise — Best Playwright Stealth 2026 vs Cloudflare & Akamai](https://scrapewise.ai/blogs/playwright-stealth-2026)
- glizzykingdreko v3 Medium walkthrough, xiaoweigege/akamai2.0-sensor_data, Edioff/akamai-analysis (already catalogued in 26 §5)

Internal:
- `docs/releases/v0.1.0-parity/26_AKAMAI_BMP_DEEP.md`
- `docs/vNext/01_R-AKAMAI-SECCPT-FLAKE.md`, `docs/vNext/03_R-BESTBUY-AKAMAI.md`
- `docs/HANDOFF_2026_05_28b.md` §4 (live-nav drain root cause)
- `crates/browser/src/page.rs:242-247` (`is_seccpt_solved`), `:1657-1731` (warm-rebuild drain, the 50 ms truncation at `:1678`), `:1857-1858` (sec-cpt detection), `:1938` (homedepot 45 s budget tier), `:2051-2055` (per-iteration drain), `:2254-2276` (poll-break on `~3~`), `:2332-2336` (BMP-POST suppression)
- `crates/browser/src/classify.rs:84` (`/_sec/cp_challenge`), `:104` (`_abck`), `:127-134` (`AKAMAI_CHALLENGE_COSIGNAL`), `:548-551` (bestbuy splash)
- `crates/browser/src/js/humanize.js:69-99, 341` (`__akamai_events` surface)
- `crates/browser/src/challenge.rs:55-161` (`ChallengeSolver` seam)
