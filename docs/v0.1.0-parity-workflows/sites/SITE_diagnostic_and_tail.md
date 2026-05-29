# areyouheadless (diagnostic) + tail-robustness audit

**Scope:** (A) Confirm `areyouheadless` is correctly excluded from the honest
pass-rate and decide whether any engine could ever flip it. (B) Audit the
~107 routed "passes" for fragile / flaky / band-aided / false-positive sites
that the path-to-126 must not regress. Cross-check the holistic `*-CHL`
false-positive trap.

**Author:** research agent, 2026-05-28. Branch `fix/v0.1.0-fix4-canvas-parity`.
**Read first:** `docs/HANDOFF_2026_05_28b.md`,
`docs/releases/v0.1.0-parity/FAILED_SITES_ANALYSIS.md`,
`docs/releases/v0.1.0-parity/audit/17_DELTA_HEADTOHEAD_2026_05_28.md`.

---

## 0. TL;DR

1. **areyouheadless is correctly and *doubly* excluded** from the honest
   pass-rate. It is (a) flagged `diagnostic: true` in `build_corpus_json.py`
   (`DIAGNOSTIC_SITES = {"areyouheadless"}`, line 23) and folded into the
   `production_pass` metric in both `sweep_metrics.rs` and
   `run_sweep_isolated.py`, **and** (b) structurally below the pass-gate
   (`len ≥ 15000`) — its body is **3653 b** for BO / 3662 b Patchright /
   3668 b Camoufox v150, so it can *never* score a PASS regardless of the
   diagnostic flag. The 15-byte spread across engines confirms even the SOTA
   open-source stealth browser (v150) **also fails it** — it is not a BO-only
   gap.

2. **No engine can "one-up everyone" by flipping it on the ledger.** The
   verdict is computed **server-side** (POST to `/bots/scannerareuhead`) after
   `fpCollect.generateFingerprint()` runs; the page DOM is then patched with a
   verdict string. Even a perfect "not headless" verdict leaves the body a
   ~3.6 KB shell — **below the 15 KB pass-gate** — so it would still not count
   as a PASS in the current ledger. Flipping it is therefore *both* a stealth
   problem (solve every surface fp-collect probes) *and* a metric problem (the
   shell never reaches 15 KB). **Verdict: leave it diagnostic; do not chase.**

3. **Tail robustness — the real risk is NOT under-counting (`*-CHL` FP trap is
   already fixed in code), it is the inverse: a *growing* challenge body
   slipping the 30 KB `*-CHL` gate and being mis-counted as a PASS.** The
   `engine_classify` size gates (`crates/browser/src/classify.rs:37-51`) are
   conservative and well-tested (8 FP regression tests), so the historical
   "109 vs 120" under-count the memory warns about is *resolved*. The standing
   risk is the **9 borderline sites** in the thin-shell band (1–15 KB) plus
   the **handful of large-body passes that carry a vendor marker** — these are
   where a refactor (esp. the §5.1 AWS live-nav drain) could flip a number in
   either direction without anyone noticing.

---

## 1. What the existing repo docs already concluded

### 1.1 areyouheadless

- **`FAILED_SITES_ANALYSIS.md` §C.5 (line 177-182)** classifies it as a
  *diagnostic probe — designed to fail*: "passing it cleanly is essentially
  solving every stealth surface at once. By construction, every engine
  including v150 gets the same 3653-3668 byte interstitial." Filed as
  `R-CORPUS-PROBE-BUCKET` (rank 8, "30 min, metric-cleanup").
- **`FAILED_SITES_ANALYSIS.md` data table (line 41):** BO 3653 b on all 4
  profiles, Camoufox v135 3668 b, v150 3668 b, Patchright 3662 b — i.e. *every*
  tested engine returns the same sub-4 KB body. It sits in Stratum **C** ("no
  engine tested passes — true open-source frontier"), grouped with Kasada /
  etsy / bestbuy / wildberries as out-of-public-engine-scope or
  research-grade (line 13).
- **`build_corpus_json.py:18-23`** documents the rationale: "Counting them as
  failures drags every engine's raw pass-rate down equally; reporting
  production (raw minus diagnostic) gives the honest 'real-browsable' number."
- The decision is tracked as **§R-CORPUS-DIAGNOSTIC-FLAG** in
  `docs/releases/v0.1.0-parity/audit/16_DECISION_LOG.md` (referenced from
  `build_corpus_json.py:11` and `sweep_metrics.rs:36`).

### 1.2 The `*-CHL` FP-trap and the tail

- **MEMORY `measurement_holistic_chl_fp_trap`** (10 days old, flagged
  point-in-time): the *old* render classifier tagged any body containing a
  vendor marker as `*-CHL` including multi-MB rendered pages (costco 3.7 MB,
  disneyplus 1.5 MB, homedepot 1 MB), under-counting BO **109 vs the correct
  120/126**. Rule of thumb in the memory: size-gate `*-CHL` at **≥30 KB =
  rendered PASS, <30 KB = genuine block**.
- **`classify.rs` module doc (lines 1-31)** confirms this trap was the root of
  the inflated "22 engine-addressable" count and that the three divergent
  classifiers were **unified** (FP-B1) onto the most FP-hardened policy, then
  hardened further (FP-B2/B3/B4 + FP-Tier1). This is the *fix* the memory's
  branch (`fix/engine-fp-backlog`) was chasing — it has landed.

---

## 2. New external findings (areyouheadless internals)

Source: live fetch of `arh.antoinevastel.com` + Vastel's published research.

- **The detection runs server-side.** `arh.antoinevastel.com/javascripts/areuheadless.js`
  waits for `DOMContentLoaded`, calls `fpCollect.generateFingerprint()`
  (Vastel's invasive fp-collect library), attaches a UUID, and **POSTs the
  fingerprint to `/bots/scannerareuhead`**. The verdict ("You are / are not
  Chrome headless") is decided by the **server** and written back into the
  result element. There is no client-side boolean we can satisfy by patching a
  single JS surface — the server cross-checks the whole fp-collect bundle.
  (Source: `arh.antoinevastel.com/javascripts/areuheadless.js`; corroborated by
  the search summary "generates an invasive browser fingerprint using fp-collect
  … then makes a POST request to the /bots/scannerareuhead endpoint".)
- **What fp-collect probes** (Vastel, "Detecting Chrome headless v2/v3", and the
  fp-collect repo): `navigator.webdriver`, `window.chrome` presence + the
  `chrome.runtime`/`chrome.loadTimes` shape, `navigator.plugins`/`mimeTypes`
  length and structure, `navigator.languages`, the
  **`Notification.permission` vs `navigator.permissions.query({name:'notifications'})`
  state mismatch** (the classic v2 headless tell), WebGL vendor/renderer,
  `navigator.connection`, touch support vs UA, screen/`outerHeight==0`,
  codecs, and the error-stack / `iframe.contentWindow` shape. The verdict is a
  *correlated* judgement over the whole bundle, explicitly: "verifies if
  browsers pretending to be Chromium-based are who they pretend to be."
  (Source: antoinevastel.com bot-detection posts 2018-01-17, 2019-07-19,
  2023-02-19.)
- **Implication:** the body is a static ~3.6 KB shell whose only mutable part
  is the verdict text. The 15-byte spread (3653 BO vs 3668 v150) is just the
  difference in the injected verdict string / minor markup — **not** a
  difference in pass-ability. Even v150, whose entire raison d'être is fp-collect
  / fingerprint-suite-grade stealth (Camoufox is built by the same lineage that
  authors fingerprint-suite), does not get scored "not headless" here as a
  rendered ≥15 KB page.

**Sources:**
- [Are you chrome headless? (live)](https://arh.antoinevastel.com/bots/areyouheadless)
- [areuheadless.js (live)](https://arh.antoinevastel.com/javascripts/areuheadless.js)
- [Detecting Chrome headless, the game goes on (v3)](https://antoinevastel.com/bot%20detection/2019/07/19/detecting-chrome-headless-v3.html)
- [New headless Chrome near-perfect fingerprint](https://antoinevastel.com/bot%20detection/2023/02/19/new-headless-chrome.html)

---

## 3. BO code-level analysis

### 3.1 The diagnostic-exclusion wiring (verified correct, 3 call sites)

| File | Lines | What it does |
|---|---|---|
| `benchmarks/build_corpus_json.py` | 23, 62-63 | `DIAGNOSTIC_SITES = {"areyouheadless"}`; sets `entry["diagnostic"]=True`. Single source that injects the flag into `corpus.json`. |
| `crates/browser/examples/sweep_metrics.rs` | 38-39, 66-75, 282-295 | `Site.diagnostic` (`#[serde(default)]` so old corpus JSONs still parse). Computes `diagnostic_n`, `production_n = total - diagnostic_n`, `production_pass` (excludes diagnostic), `production_pass_pct`. |
| `benchmarks/run_sweep_isolated.py` | 123-131, 144-147, 176 | Mirrors the Rust schema: `diagnostic_names` set from corpus, `production_n`, `production_pass`, `production_pass_pct`. This is the **gate aggregator** the routed-best-of-4 number is built from. |

**Conclusion:** the diagnostic exclusion is wired into both the Rust sweep
binary and the Python gate aggregator, with `#[serde(default)]` keeping it
backward-compatible. It is **correctly excluded from the honest
(`production_*`) pass-rate**. The `pass`/`pass_pct` *raw* fields still count it
as a non-pass — that is intentional (raw vs production dual-metric).

### 3.2 areyouheadless cannot pass the gate even if flipped — the double lock

The pass-gate is identical in all three aggregators:

```
PASS  ⇔  tag == "L3-RENDERED"  AND  len >= 15000
```
- `sweep_metrics.rs:262-263` (raw pass), `:288` (production pass)
- `run_sweep_isolated.py:102`, `:129-130`
- `classify.rs:47` `THIN_SHELL_MAX_BYTES = 15 * 1024` — a rendered body below
  this is `ChallengeVerdict::ThinShell`, not `Pass` (`verdict_for`, lines 180-181).

areyouheadless renders at **3653 b** → `tag = "L3-RENDERED"` (≥1000 b, so not
THIN-BODY) but `verdict = ThinShell` (<15 KB). It is therefore **excluded twice
over**: by the diagnostic flag *and* by the size gate. There is no code path in
which areyouheadless counts as a ledger PASS, even with a perfect server
verdict.

### 3.3 The actual FP-trap surface in BO today (the inverse risk)

The memory warns about *under*-counting big rendered pages as `*-CHL`. Reading
`classify.rs`, that trap is closed:

- **`UNAMBIGUOUS` (lines 81-86)** — only 4 any-size tokens remain, all genuine
  challenge-only URL/var strings (`cf-browser-verification`, `_cf_chl_opt`,
  `/_sec/cp_challenge`, `ddcaptchaencoded`). `px-captcha` was deliberately
  *removed* (the wayfair FP) and relocated to the 30 KB-gated `SMALL_BODY`.
- **`PHRASE` (91-97)** and **`SMALL_BODY` (102-118)** only fire when
  `len < INTERSTITIAL_MAX_BYTES` (30 KB) — `engine_classify` lines 209-220.
- **FP-Tier1 co-signal gates (127-168):** `akam/13` and bare `captcha` need a
  structural co-signal, killing the bestbuy-i18n-splash and spotify/duolingo
  invisible-reCAPTCHA-v3 FPs.

So a multi-MB rendered page that merely *mentions* a vendor marker is correctly
`L3-RENDERED` → PASS (regression-pinned by `fp_b2_*`, `fp_t1_*` tests, lines
355-568). **The remaining trap is the opposite direction:**

> A genuine, unsolved challenge whose body **grows past 30 KB** (or past 50 KB
> for the `SENSOR_SPLIT`) drops out of the `*-CHL` gate and is scored
> `L3-RENDERED` → potential **false PASS**.

Two concrete instances of this risk class live in the code's own comments:
- `classify.rs:189` — a **large Cloudflare orchestrator shell** (udemy class)
  is only saved from a false-PASS by the `_cf_chl_opt` *any-size* token in
  `UNAMBIGUOUS`. If a vendor ships a CF challenge variant *without*
  `_cf_chl_opt` in a >30 KB body, it would score PASS. (Mitigated for CF by
  `is_cf_challenge_doc` persistence, `classify.rs:247-251`, used by the
  navigate loop.)
- Any DataDome/PerimeterX challenge that renders a large interstitial (image
  carousel captcha, multi-asset "press & hold") above 30 KB would slip the
  phrase/SMALL_BODY gates. None currently observed in the corpus, but this is
  the gate to watch when AWS/duolingo bodies start *growing* after the §5.1
  fix.

### 3.4 Tail census — the 9 borderline / band-aided "passes" at regression risk

From `FAILED_SITES_ANALYSIS.md` data table + `17_DELTA_HEADTOHEAD` + the
classify thin-shell band, here are the sites whose "pass/fail" verdict sits
within ±a few KB of a gate, or rests on a band-aid. These are the ones a
refactor can silently flip:

| site | current state | why fragile | gate proximity |
|---|---|---|---|
| **x-com** | PASS 294 k (Sprint 2.3 cookie band-aid) | "cookie band-aid" per MEMORY/sprint12; SharedSession A/B never refactored (`R-SHAREDSESSION-X-COM`). 3/3 in delta but rests on a hack. | far from size gate, but logic-fragile |
| **homedepot** | flaky win 3/5 (1.16 MB) | daily-rotated Akamai sec-cpt; `is_seccpt_solved` is ~60% (`R-AKAMAI-SECCPT-FLAKE`). The "994 KB flip" did **not** reproduce (delta §2). When solved it's a clean 1.16 MB PASS; when not, Akamai-CHL 2 k. | binary: huge-PASS or 2 k-fail |
| **bestbuy** | FAIL 7.9 k i18n splash | sits in thin-shell band; FP-Tier1 correctly does *not* count it. If a future change pushes the splash >15 KB it would falsely PASS. | thin-shell band (1-15 KB) |
| **spotify** | PASS-ish ~9.6 k | invisible reCAPTCHA-v3 shell; classified `ThinShell` (not PASS, not challenge). Borderline — a hydration change either way flips it. | thin-shell band |
| **duolingo** | FAIL 13.3 k | **just below** the 15 KB gate; reCAPTCHA Worker (`R-DUO-WORKER`). A few KB of hydration would flip it to PASS *without solving the worker* — a measurement artifact risk. | **2 KB below the 15 KB gate** |
| **booking** | FAIL 8 k SPA shell | SPA fetch-chain (`R-SPA-BOOKING-FETCH-CHAIN`); same live-nav-drain class as AWS. | thin-shell band |
| **wildberries** | unstable (1.8 k / ERR / 7.9 k) | different body per engine; likely geo-blocked (`R-CORPUS-WILDBERRIES`, candidate for corpus removal). | varies; never near gate |
| **amazon-fr/jp/com-au** | reliability PASSes (1/3–2/3) | huge bodies (819 k–928 k) when the probabilistic token roll lands; **flaky**, not deterministic. Counted PASS only on lucky trials. | far from gate, but variance-fragile |
| **amazon-com / amazon-ca** | IP/probabilistic (1/3 / 0/3) | v150 also flaky/fails; **don't chase** — but don't let a "fix" claim credit for an IP-driven flip. | far from gate |

The single most dangerous one for the path-to-126 is **duolingo**: at 13.3 KB
it is only ~1.7 KB under the 15 KB gate, so any unrelated hydration change (or
the §5.1 longer drain letting more of the shell render) could push it to PASS
**without actually solving the reCAPTCHA Worker** — a textbook
measurement-artifact false win. Pin its body size in a regression assertion
before/after the §5.1 work.

### 3.5 The areyouheadless "true headless tell" question

Could BO *actually* pass it to one-up everyone? Mechanically:
- The body would still be ~3.6 KB → **never a ledger PASS** (§3.2). So even a
  perfect verdict does not move the headline number. This alone makes it
  not-worth-chasing for the path-to-126.
- To get the server to write "not headless" you must satisfy *all* of
  fp-collect's correlated checks simultaneously (§2). BO's stealth surface
  already covers most (webdriver hidden, `window.chrome` present, plugins
  spoofed, languages, WebGL via the new FIX-D2 split). The classic remaining
  tells fp-collect keys on are the **`Notification.permission` /
  `permissions.query` consistency** and the **plugin/mimeType object shape**.
  These are worth a one-off in-VM probe *for stealth-quality reasons*
  (creepjs/DataDome benefit), but **scoring it as a corpus PASS is gated out by
  size regardless**, so it must not be tracked as a path-to-126 lever.

---

## 4. Cross-check against the holistic FP trap (explicit)

The memory's "size-gate `*-CHL` ≥30 KB = PASS" rule is **already encoded** and
is *more* nuanced in current code:
- 30 KB (`INTERSTITIAL_MAX_BYTES`) gates `PHRASE` + `SMALL_BODY` markers
  (`classify.rs:209`).
- 15 KB (`THIN_SHELL_MAX_BYTES`) is the PASS floor (anything rendered below is
  `ThinShell`, not PASS).
- 50 KB (`SENSOR_SPLIT_BYTES`) splits EdgeBlock vs SensorFail/ChallengeIncomplete.

So the cross-tool comparison the memory worried about (unfairly under-counting
BO's big rendered bodies as blocked) is fixed. **The honest pass-rate is the
`production_pass` field** (raw minus the 1 diagnostic). When re-running the
gate, always read `production_pass` / `production_pass_pct`, never the raw
`pass` (which still counts areyouheadless as a non-pass).

**One methodology caution carried from the memory:** the broad 126 sweep must
be a **release** build (`run_sweep_isolated.py` / `sweep_metrics --release`);
debug + concurrent load causes 60 s+ contention stalls and duplicate
holistic-end re-emission that corrupts the count. This is unrelated to areyouheadless
but is the live trap for the *whole tail* census.

---

## 5. Ranked fix list (ROI order)

All are public-engine (corpus/metric or measurement-hardening) **except where
noted**; none require `vendor_solvers`.

1. **Pin a regression test on the diagnostic exclusion + the tail body sizes.**
   Add a unit test asserting `corpus.json` flags exactly `{areyouheadless}`
   diagnostic, that `production_n == 125`, and snapshot the thin-shell-band
   sites' body sizes (duolingo ~13.3 k, bestbuy ~7.9 k, spotify ~9.6 k,
   booking ~8 k) so a future refactor that nudges any across the 15 KB gate
   **fails CI loudly** instead of silently claiming a flip. *Effort: 0.5 day.
   Impact: protects all ~107 passes + locks the honest metric. Confidence:
   high. Public engine.*

2. **areyouheadless — keep diagnostic, close `R-CORPUS-PROBE-BUCKET` as
   "already done".** The wiring (§3.1) and the size lock (§3.2) mean it is
   already correctly excluded. The only remaining work is a one-line doc note
   that v150 *also* fails it (3668 b), so it is not a competitive gap. **Do
   NOT** open a stealth ticket to "pass" it — gated out by size. *Effort: 15
   min. Impact: 0 sites flip (correctly), removes a false TODO. Confidence:
   high. Public engine (metric-cleanup).*

3. **Guard the 30 KB `*-CHL` gate against *growing* challenge bodies (the
   inverse FP trap).** Before/after the §5.1 AWS live-nav drain lands, assert
   that AWS-WAF / DataDome / CF bodies that *grow* past 30 KB but are still
   unsolved do not flip to a false PASS. Concretely: extend `is_cf_challenge_doc`
   pattern to AWS (`gokuProps` / `awsWafCookieDomainList` / `challenge.js`
   any-size origin token) so a >30 KB AWS challenge shell can't score PASS the
   way a >50 KB CF shell almost could. *Effort: 1 day. Impact: protects the
   AWS cluster verdict integrity (7 sites) during the §5.1 refactor. Confidence:
   medium. Public engine.*

4. **duolingo body-size assertion before §5.1.** It is 1.7 KB under the gate;
   add an explicit guard that a duolingo PASS must coincide with a solved
   reCAPTCHA Worker token (not merely a >15 KB hydrated shell). Prevents a
   measurement-artifact "win." *Effort: 0.5 day. Impact: 1 site verdict
   integrity. Confidence: medium. Public engine.*

5. **homedepot sec-cpt rotation hardening (`R-AKAMAI-SECCPT-FLAKE`).** Already
   a BO-beats-v150 site but flaky ~60%; harden `is_seccpt_solved` for the
   daily-rotated bundle so the win is reliable 5/5. *Effort: 2-3 days. Impact:
   1 site, defends a win we already hold over v150. Confidence: medium. Public
   engine.*

6. **wildberries — verify geo-block, drop from canonical corpus if 4xx/5xx
   from the datacenter IP (`R-CORPUS-WILDBERRIES`).** Different body per engine
   ⇒ likely unreachable, not an engine miss. Removing it cleans the denominator
   honestly. *Effort: 30 min. Impact: corpus denominator −1 (honest). Confidence:
   medium. Public engine (corpus cleanup).*

7. **x-com SharedSession refactor (`R-SHAREDSESSION-X-COM`).** Currently a 3/3
   PASS resting on a cookie band-aid; refactor to real per-session isolation so
   the pass is robust and the band-aid can't regress other social sites.
   *Effort: 3 days. Impact: hardens 1 held PASS (+ de-risks the social
   category). Confidence: medium. Public engine.*

---

## 6. Open questions

- Does the §5.1 longer post-script drain push **duolingo** over 15 KB without
  solving the worker (false win) — needs the §3.4 size-vs-token assertion to
  decide. (Highest-priority pre-flight check before §5.1.)
- Are there corpus sites currently scored PASS whose body carries a vendor
  marker between 30–50 KB (the band most exposed to the inverse FP trap)? Not
  observed in the 2026-05-27 table, but should be swept once after §5.1 since
  AWS bodies will start growing.
- Is wildberries reachable at all from the datacenter IP (geo vs engine)? A
  one-shot curl resolves whether it stays in the denominator.
