# 03 — BESTBUY / Akamai BMP frontier: engine-addressable hunt

**Site:** bestbuy.com — Akamai Bot Manager Premier (BMP) on a React SPA.
**Status (corrected):** **Stratum C — no engine tested passes.** BO, Camoufox
v150, AND real-Chromium Patchright all land on the same ~7–8 KB pre-hydration
shell.
**This doc's job (per the frontier mission):** challenge the "out of scope"
verdict, find the concrete engine-addressable path if one plausibly exists,
and be honest where it is genuinely IP/geo-bound. Conclusion up front:
**bestbuy is the one site in this workflow where the evidence points AWAY
from an engine-fidelity lever and TOWARD an IP/ASN trust gate** — but there
is a single cheap, near-free public-engine probe that could still flip it,
and that probe is the recommendation.

**Reading order — extends, does not replace:**
- `docs/v0.1.0-parity-workflows/sites/SITE_bestbuy.md` (the corrected
  cross-engine table + the "Patchright PASS 1246k" debunk — load-bearing)
- `docs/v0.1.0-parity-workflows/external/VENDOR_akamai.md` (the three-provider
  sec-cpt taxonomy, the async-execution-fidelity reframe, the Patchright
  analysis)
- `docs/releases/v0.1.0-parity/26_AKAMAI_BMP_DEEP.md` (the canonical BMP
  mechanism deep dive + the vendor-strip boundary)
- `docs/vNext/03_R-BESTBUY-AKAMAI.md` (the original R-ticket, premised on the
  now-falsified "Patchright passes" read)
- `docs/HANDOFF_2026_05_28b.md` §4 (the AWS live-nav-drain root cause — the
  one shared public lever)

---

## 0. TL;DR — the new synthesis

1. **The "Patchright passes bestbuy at 1246k" claim is FALSE and load-bearing.**
   `SITE_bestbuy.md §0` already corrected it: `1246k` is the **homedepot**
   Patchright number copied onto the wrong data row in
   `docs/vNext/03_R-BESTBUY-AKAMAI.md:42` and `audit/16_DECISION_LOG.md:131`.
   The authoritative table (`FAILED_SITES_ANALYSIS.md:36`, re-confirmed in
   `26_AKAMAI_BMP_DEEP.md:503`) shows **bestbuy Patchright = 7105 b** — a
   thin shell, same as everyone. **There is no passing reference engine to
   diff against.** Every diagnostic in the R-ticket ("just diff BO vs
   Patchright sensor_data") is built on a phantom.

2. **This breaks the central thesis's usual logic — and that's the honest
   finding.** The frontier mission's thesis is: "no-CDP real Chrome passes
   these sites, every CDP competitor fails, so BO's structural no-CDP
   advantage is the lever." For Kasada (canadagoose/hyatt/realtor) the prior
   evidence is strong: a `nocdp.sh`-launched real Chrome PASSES zero-interaction
   from this very IP (memory: `proxy_not_the_problem`,
   `state_2026_05_15_playwright_ab_decisive`). **For bestbuy there is NO such
   captured no-CDP pass.** What we DO have is the opposite: real-Chromium
   Patchright (which strips `Runtime.enable` and has a real event loop — the
   closest thing to a no-CDP real browser we've measured) **also fails at
   7105 b.** When the SOTA stealth engine (v150) *and* a real Chromium *and*
   BO all fail identically from the same datacenter IP, the residual is
   overwhelmingly **IP/ASN reputation + a vendor sensor_data requirement**,
   not a fingerprint-fidelity lever.

3. **bestbuy is NOT a sec-cpt site and NOT classified as an Akamai challenge
   by BO's own ledger.** Its 7.9 KB body carries the `akam/13` BMP bootstrap
   but **none** of the `AKAMAI_CHALLENGE_COSIGNAL` markers (`sensor_data`,
   `bm-verify`, `sec-cpt-if`, `sec-if-cpt-container`, `/_sec/cp_challenge`,
   `pardon our interruption`) — so `classify.rs:174` correctly keeps it
   `L3-RENDERED` (a thin shell), not a challenge. None of the sec-cpt
   machinery (`is_seccpt_solved`, the 45 s homedepot budget tier, the PoW
   self-solve) touches bestbuy. It is **plain BMP sensor_data + `_abck`**,
   the same mechanism class as the curl-RST at the edge.

4. **The one public-engine lever worth a near-zero bet:** the AWS-cluster
   `live-nav-drain` fix (HANDOFF_2026_05_28b §5.1). IF (and only if) the
   `akam/13` bundle is the same shape as AWS's `challenge.js` — an external,
   async, self-posting script that runs in the offline oracle but produces
   zero async progress live because of the 50 ms warm-rebuild drain — then
   bestbuy flips for free when that fix lands. The probe to test this costs
   ~1 hour folded into AWS work. **This is the only credible engine-addressable
   path, and its confidence is low–medium.**

---

## 1. The exact detection mechanism blocking BO

### 1.1 The Akamai BMP v3 flow, reconstructed (cited)

Akamai BMP scores **every request from the first connection** (0=human …
100=bot) and is purpose-built for "modern JavaScript-heavy single-page
applications like React" — exactly bestbuy's shape (akamai.com/products/bot-manager;
`SITE_bestbuy.md:124-126`). The flow, in order:

1. **Edge gate (ASN/TLS/HTTP-2).** The Akamai edge scores the raw connection.
   On this datacenter IP, naked curl is hard-RST: HTTP/2 stream 1 reset with
   `0x2 INTERNAL_ERROR`; HTTP/1.1 → 30 s timeout, 0 bytes
   (`audit/16_DECISION_LOG.md` §R-BESTBUY-AKAMAI; `SITE_bestbuy.md:96-99`).
   **BO's `chrome_147`-class TLS impersonation clears this edge** — it
   receives the 7.9 KB shell where curl gets nothing. So the ClientHello /
   HTTP-2 SETTINGS gate is **not** BO's blocker (confirmed; `SITE_bestbuy.md:98-99`).

2. **The `akam/13` bootstrap.** The shell ships
   `<script src="https://www.bestbuy.com/akam/13/62321f80" defer>` — the BMP
   sensor collector (the ~512 KB obfuscated `bmak` bundle that the `akam/`
   path serves; WebSearch: scrapfly/roundproxies). It collects the device
   fingerprint + behavioral buffers and POSTs `sensor_data`.

3. **The `sensor_data` POST → `_abck` lifecycle.** Per the Hyper Solutions
   protocol docs cited in `VENDOR_akamai.md §2` and the BMP field taxonomy in
   `26_AKAMAI_BMP_DEEP.md §2`: the bundle POSTs an obfuscated `sensor_data`
   blob to a tenant endpoint; the server returns/updates the `_abck` cookie.
   The `_abck` state machine (`26 §2.2`, corrected by commit `24c19e3`):
   - `~-1~-1~-1~` = **Favorable** (cleared — real content unlocks)
   - `~0~-1~-1~` = **Untrusted** (needs more / better sensor data)
   - `~0~0~` = Provisional, `~3~…` = **Rejected**
   - slot 1 is a **stop-signal threshold** (count of sensor POSTs Akamai
     will accept), NOT a simple trust toggle.
   Typically **1–3 sensor POSTs** clear `_abck` to Favorable *if the score
   is good*; a datacenter-ASN + bot-pattern fingerprint never reaches
   Favorable no matter how many POSTs.

4. **Conditional hydration.** bestbuy's React app fires its real
   content-fetch chain **only after** it sees a Favorable `_abck`. A
   Rejected/Untrusted `_abck` → the shell never hydrates → the ~7–8 KB body
   everyone sees.

### 1.2 Which of (a) behavioral / (b) JS-execution / (c) holistic blocks BO

The task asks to choose between three root causes. The evidence forces a
layered answer, with the dominant cause being a **fourth** one the task
options under-weight (IP/ASN):

| Candidate | For | Against | Verdict |
|---|---|---|---|
| **(a) behavioral-signal gate** (needs mouse/keystroke entropy in sensor_data) | BMP field-65 mouse-trajectory model is real (`26 §2.3`); humanize.js synthetic events are `isTrusted`-faked | **Patchright = real Chromium with REAL events also fails (7105 b)** — if trusted behavioral entropy were the gate, Patchright would pass | **Not the dominant cause.** Real trusted events don't flip it. |
| **(b) JS-execution gap** (the `akam/13` bundle doesn't fully run in V8) | This is the AWS live-nav-drain class; the bundle is external+async+self-posting | We have **no capture** showing BO's `akam/13` is fetched-but-never-posts. **Unverified.** | **The one testable engine lever (§3).** Low–med confidence. |
| **(c) Akamai holistic scoring** | BMP is explicitly a holistic 0–100 scorer | Holistic score is dominated by the IP/ASN input here | Real, but a *symptom* of (d). |
| **(d) IP/ASN reputation** (NOT in the task's 3 options) | Datacenter ASN = "significant negative trust score" (scrapfly, roundproxies, zenrows, all WebSearch sources); the edge already RSTs raw curl from this IP; **v150 AND real Chromium both fail identically from this IP** | — | **The dominant cause, by elimination.** |

**The decisive logical step (the no-CDP test, inverted):** the frontier
thesis says "if a no-CDP real browser passes, it's engine-addressable; if it
needs interaction, it's behavioral." We can run a sharper version: **does a
real Chromium with a real event loop and stripped CDP pass?** Patchright IS
that (it removes `Runtime.enable`, drives a real event loop —
`VENDOR_akamai.md §2.2`). Patchright **fails at 7105 b**. Therefore the
blocker is neither CDP-residue (BO has none anyway) nor execution-fidelity
that a real browser provides — it is something a real Chromium *also* lacks
from this IP: **a Favorable `_abck`, which on a datacenter ASN it cannot
get.** That is the honest read.

---

## 2. Does a no-CDP real browser pass it? → the engine-addressability test

**This is the crux, and the honest answer separates bestbuy from the Kasada
cluster.**

- **Kasada (canadagoose/hyatt/realtor):** prior captured evidence —
  `nocdp.sh` real Chrome PASSES zero-interaction from this datacenter IP
  (memory: `state_2026_05_15_playwright_ab_decisive`,
  `proxy_not_the_problem`). ⇒ NOT IP, ⇒ engine-fidelity gap, ⇒
  engine-addressable. The thesis holds.

- **bestbuy:** **NO captured no-CDP real-browser pass exists.** The closest
  measured proxy for a no-CDP real browser — Patchright (real Chromium,
  `Runtime.enable` stripped, real event loop) — **fails at 7105 b**
  (`FAILED_SITES_ANALYSIS.md:36`, `26 §4.3:503`). Camoufox v150 (real
  Firefox engine) **fails at 7467 b**. Vanilla Playwright/Playwright-stealth
  (real Chromium) **fail at 7340 b**.

**Inference:** when a real Chromium *and* a real Firefox *and* the SOTA
stealth engine *and* BO all land on the identical thin shell from the same
IP, the differentiator that every one of them shares — and that a passing
visitor would not — is **the datacenter ASN.** The frontier-mission
"engine-addressable because no-CDP real Chrome passes" inference **does not
apply to bestbuy**, because no-CDP real Chrome is NOT demonstrated to pass it.

**The missing measurement that would settle it (and which the repo has NOT
run):** a genuinely no-CDP real Chrome (the `nocdp.sh` approach, NOT
Playwright/Patchright) navigating bestbuy zero-interaction:
- **from this datacenter IP** → if it PASSES, bestbuy flips to
  engine-addressable and the whole verdict changes (this would be a major
  finding; treat as the highest-value experiment).
- **from a residential IP** → controls for the ASN. If it passes residential
  but fails datacenter, the IP gate is confirmed and bestbuy is IP-bound.

Until that capture exists, the evidence-weighted verdict is **IP/ASN-bound,
with a low-probability engine-addressable tail (the §3 drain hypothesis).**

---

## 3. The concrete engine path (file:line) + how BO's no-CDP advantage helps

There is exactly one engine-addressable hypothesis with a code path. It is
the AWS-cluster lever, applied to bestbuy.

### 3.1 The hypothesis: bestbuy is gated by the warm-rebuild 50 ms drain

`HANDOFF_2026_05_28b §4` proved the AWS WAF cluster's blocker is **not**
fingerprint and **not** WASM — it is that `challenge.js` runs fully in the
offline oracle (5 s `run_until_idle`) but makes **zero async progress in the
live navigate path** because the warm-rebuild path drains only 50 ms between
scripts. `akam/13` is the **same shape**: external, async, self-posting.

**The code (current HEAD):**
- `build_page_with_scripts_init_and_storage` runs external+inline scripts
  with a **fixed 50 ms** `run_until_idle` between each
  (`crates/browser/src/page.rs:1703-1707`) + a single **500 ms** final drain
  (`page.rs:1755-1758`). This is the truncation.
- The external script bodies ARE fetched and executed in document order on
  this path (`page.rs:1687-1707`, `prefetched.get(&i)` → `execute_script_with_name`).
  So `akam/13` *runs* — the question is whether its async self-post chain
  (`setTimeout`/`fetch`/promise continuations) completes before the drain
  truncates it.
- **Important nuance (corrects the R-ticket's worry):** the *navigate-loop*
  per-iteration drain is **NOT** 50 ms — it is adaptive, floored at 8 s and
  capped at `nav_budget` (`page.rs:2102-2106`), and bestbuy already gets the
  **25 s plain-BMP budget tier** (`page.rs:1977-1985`). So on the *cold*
  first navigate, `akam/13` gets up to ~25 s of contiguous drain. The 50 ms
  truncation bites specifically on the **warm-rebuild path** (used when a
  pending nav/reload fires) — i.e. if the React app issues a soft
  navigation/reload after the first sensor POST, the re-served bundle is
  truncated. This is a narrower window than the R-ticket implied, which
  **lowers** the confidence that drain alone is bestbuy's blocker.

### 3.2 How BO's no-CDP advantage helps (where it does)

BO's structural no-CDP cleanliness IS a real asset for the BMP **device
fingerprint** sub-score: there is no `Runtime.enable`, no juggler, no `cdc_`
vars, no `webdriver`, no automation protocol surface for `bmak` to probe
(the thesis's structural advantage — `VENDOR_akamai.md §2.2` confirms BMP
probes all of these). **BO covers reason (1) — no CDP detection signal —
for free.** That is exactly why BO ties real Chromium at the shell instead
of being hard-RST: its fingerprint is clean enough to clear the edge and run
the bundle.

**But on bestbuy that advantage is necessary, not sufficient:** Patchright
*also* removes the CDP signal and *also* only gets the shell. The fingerprint
sub-score is not the binding constraint here — the **IP/ASN sub-score is.**
BO's no-CDP advantage is the lever that wins Kasada (where the fingerprint
sub-score IS binding and the IP is clean); it is spent-but-insufficient on
bestbuy.

### 3.3 The humanize.js behavioral surface — present, sophisticated, but inert

If the probe lands on the behavioral branch (§4 Q3), the relevant code is
`crates/browser/src/js/humanize.js` (458 lines), which is **better than the
R-ticket assumed:**
- `globalThis.__akamai_events` buffer — `mouse[]/key[]/touch[]/scroll[]` +
  `counters` (`humanize.js:69-77`), the exact buffer the stripped
  `crates/akamai/src/payload.rs::field_mouse_trajectory` read.
- Synthetic events have `isTrusted` **forced to true** via
  `Object.defineProperty` (`humanize.js:105, 429, 441`) — so the
  `isTrusted=false` weakness the R-ticket and `SITE_bestbuy.md:246` worried
  about is **already mitigated** for handlers that gate on it.
- Trajectories are sourced from a **sigma-lognormal Plamondon generator**
  (`op_behavior_mouse_trajectory` → `crates/stealth/src/behavior.rs`,
  `humanize.js:360-376`) — curved, variable-velocity, target-seeking, NOT
  naive linear `scrollBy`. This is exactly the field-65-quality trajectory
  model `26 §2.3` called for.

**The dead end:** with `default_solvers()` returning `Arc<[]>`, **nothing in
the public engine reads `__akamai_events` and POSTs it as `sensor_data`.**
The `akam/13` bundle does its own collection+POST, but humanize.js's rich
buffer is consumed only by the (private) `vendor_solvers` encoder. So the
behavioral surface is a **prerequisite the private solver consumes, not a
standalone public lever** — and crucially, **even perfect behavioral data
will not flip bestbuy if the IP/ASN sub-score is the binding constraint**
(Patchright's real trusted events prove this).

---

## 4. The no-CDP-oracle capture + diff validation plan

The dispositive experiment the repo has NEVER run (the R-ticket deferred on
the false "Patchright passes" read; `SITE_bestbuy.md §0`). Three captures,
one verdict. **Do NOT run live navs on the contended IP while a competitor
benchmark holds it** — fold this into the AWS §5.1 capture window or run on a
spare IP.

### Capture A — BO offline oracle: does `akam/13` self-post under a long drain?
Fork `crates/browser/examples/awswaf_probe.rs` → `akamai_probe.rs`:
1. Capture a live bestbuy 7.9 KB shell + the `akam/13` bundle body (HAR or
   `sweep_metrics --capture bestbuy`).
2. `Page::from_html_with_url` the shell, pre-inject the instrumentation
   Proxy, `run_until_idle(30 s)`.
3. Dump: did the bundle `fetch()` a `sensor_data` POST? What `_abck` (if any)
   came back? Did it spawn a blob-URL worker (like AWS challenge.js → if so
   the worker secure-context fix `5216336` is a prerequisite here too)?
   `document.cookie`, `__scriptErrors`, access trace.
- **If the bundle POSTs sensor_data under 30 s but not under the live 50 ms
  warm drain → §3.1 drain hypothesis CONFIRMED → public-engine fixable
  (scenario 2).** This is the one outcome that flips bestbuy for free.
- **If it never POSTs even under 30 s → JS-surface bailout** (decode the
  bailout from the trace; may be a private-encoder dependency).

### Capture B — the no-CDP real-browser oracle (the engine-addressability test)
The VALID oracle per the mission (NEVER Playwright/Patchright for the *pass*
question — they're for the shell-comparison only):
1. `nocdp.sh`-launched real Chrome (normal user browser, no automation
   protocol) → bestbuy.com, zero interaction, passive mitmproxy/HAR capture.
2. Run twice: **from this datacenter IP** and **from a residential IP.**
3. Read: does the body exceed ~15 KB (hydrate)? What `_abck` segments
   (`~-1~-1~-1~` Favorable vs `~0~`/`~3~`)?
- **Datacenter no-CDP Chrome hydrates → bestbuy is ENGINE-ADDRESSABLE**
  (major finding; diff its `sensor_data`/fingerprint vs BO's bundle output).
- **Datacenter fails, residential passes → IP/ASN-bound** (confirmed; out of
  public-engine scope). This is the most likely outcome given Patchright also
  fails datacenter.
- **Both fail → bestbuy needs residential IP + sensor_data generator
  (vendor + proxy); not winnable in any engine from any tested IP.**

### Capture C — field diff (only if Capture B shows a datacenter pass)
Diff the no-CDP-passing Chrome's `sensor_data` POST + `_abck` against BO's
`akam/13` output from Capture A: TLS/JA4 (already cleared), HTTP-2 SETTINGS,
the `sensor_data` field-by-field (field 65 mouse, the device fields), and the
`_abck` returned. Whatever field flips Favorable is the public-engine fix
target.

---

## 5. Honest verdict

| Layer | Class | Lever | Engine |
|---|---|---|---|
| Edge ClientHello/HTTP-2 | already cleared by BO | none needed | public (done) |
| `akam/13` async self-post (IF drain-truncated) | self-solve-execution | AWS live-nav-drain fix (`page.rs:1703-1707` 50 ms → adaptive on the warm-rebuild path) | **public** |
| BMP device fingerprint sub-score | BO's no-CDP cleanliness | already clean (ties real Chromium) | public (done) |
| BMP behavioral sub-score | `__akamai_events` + Plamondon trajectories present | needs a sensor_data encoder to consume it | **vendor_solvers** (encoder) |
| `_abck` Favorable on datacenter ASN | IP/ASN reputation | residential/mobile proxy | **IP-geo-bound (out of engine)** |

**Primary verdict: IP-GEO-BOUND, with a low-confidence public-engine tail.**

The dispositive fact is that **real-Chromium Patchright AND Camoufox v150 AND
BO all fail identically (7105 / 7467 / 7943 b) from the same datacenter IP.**
That pattern is the signature of an IP/ASN trust gate, not an engine-fidelity
gap — and it is precisely the case where the frontier thesis's "no-CDP real
Chrome passes ⇒ engine-addressable" inference **does not hold**, because no
no-CDP real Chrome has been shown to pass bestbuy. This is the honest place
to say "the engine is not the binding constraint here," in contrast to the
Kasada cluster where the prior `nocdp.sh` evidence genuinely proves it is.

**The one engine-addressable path worth pursuing (Capture A + §3.1):** if the
AWS live-nav-drain fix is being built anyway (it is the single biggest
multi-site lever — homedepot crypto days, the AWS cluster, booking,
duolingo), spend ~1 hour running Capture A on a bestbuy shell. If the
`akam/13` bundle self-posts under a long drain but not under the 50 ms warm
drain, bestbuy flips **for free** and BO goes **ahead of v150 by +1** —
because v150 has no such fix. **This is the only credible "BO beats the field"
scenario for bestbuy, and it is near-zero-cost to test.** Confidence
low–medium (the cold path already gives 25 s; the truncation only bites the
warm-reload sub-path, so the window is narrow).

**Do NOT:** (a) chase the phantom "diff BO vs Patchright sensor_data" — there
is no passing reference; (b) invest in richer humanize trajectories as a
standalone lever — they are already sophisticated and are inert without a
`vendor_solvers` encoder, and would not flip the IP-bound score anyway; (c)
re-add an Akamai `sensor_data` generator to a public crate (forbidden per
CLAUDE.md / `aecdf19` — it belongs in `vendor_solvers`).

**Recommended action:** fold Capture A + Capture B into the AWS §5.1 work.
If Capture A shows the drain branch → land the shared drain fix and claim the
+1. Otherwise, run Capture B (no-CDP Chrome, datacenter vs residential) to
confirm the IP gate, then **re-classify bestbuy as IP-geo-bound + vendor_solvers
and stop** — exactly as `audit/15_FIX_PRIORITY_RANKED.md` (#9, "research-grade")
and `SITE_bestbuy.md §4` already conclude. Correct the two docs that mis-cite
"Patchright PASS 1246k" (`docs/vNext/03_R-BESTBUY-AKAMAI.md:42`,
`audit/16_DECISION_LOG.md:131`) so no future planner chases the phantom diff.

---

## 6. Open questions (the experiments that would change the verdict)

- **Q1 (verdict-flipping):** Does a `nocdp.sh` real Chrome hydrate bestbuy
  zero-interaction **from this datacenter IP**? (Capture B.) If YES →
  engine-addressable, re-open everything. Most likely NO given Patchright
  also fails.
- **Q2 (the public lever):** Does `akam/13` self-post `sensor_data` under a
  30 s oracle drain but not under the live 50 ms warm drain? (Capture A.) If
  YES → the AWS drain fix flips bestbuy for free.
- **Q3:** Does `akam/13` offload to a blob-URL Web Worker (like AWS
  `challenge.js`)? If so the worker secure-context fix (`5216336`) is a
  prerequisite. (Read from Capture A.)
- **Q4:** What `_abck` segment does BO's bundle actually receive — `~0~-1~-1~`
  (Untrusted, score-limited) or no POST at all (execution-bailed)? This
  single observation separates the IP/score branch from the drain branch.

---

## 7. Sources

External:
- [Akamai — Bot Manager](https://www.akamai.com/products/bot-manager) — holistic 0–100 scoring from first connection; built for React SPAs
- [Scrapfly — How to Bypass Akamai (2026)](https://scrapfly.io/blog/posts/how-to-bypass-akamai-anti-scraping) — four-stage detection; datacenter IP = "significant negative trust score"
- [Roundproxies — Bypass Akamai (2026)](https://roundproxies.com/blog/bypass-akamai/) — `_abck` generated by POSTing valid `sensor_data` to `/_sec/cp_challenge/verify`; 512 KB obfuscated JS collector; datacenter IPs strongly penalized
- [The Web Scraping Club — Bypassing Akamai for free](https://substack.thewebscraping.club/p/bypassing-akamai-for-free) — TLS+HTTP/2 mimicry (scrapy-impersonate) clears edge; does not claim a datacenter-IP zero-interaction homepage pass
- [ZenRows — Bypass Akamai](https://www.zenrows.com/blog/bypass-akamai), [Scrapeless — Bypass Akamai with Playwright](https://www.scrapeless.com/en/blog/bypss-akamai-with-playwright) — public consensus: Akamai retail = managed unlocker (server-side sensor_data generator) + proxy rotation, not a fingerprint-only flip
- Hyper Solutions sec-cpt/sensor_data protocol docs, glizzykingdreko v3 walkthrough, xiaoweigege/akamai2.0-sensor_data, Edioff/akamai-analysis — catalogued in `26 §5` / `VENDOR_akamai.md §8` (research-reference-only per CLAUDE.md)

Internal:
- `docs/v0.1.0-parity-workflows/sites/SITE_bestbuy.md` §0 (Patchright-1246k debunk), §3.2 (H-A/H-B), §3.4 (drain analogy), §4 (winnability)
- `docs/v0.1.0-parity-workflows/external/VENDOR_akamai.md` §2.1 (sec-cpt providers), §2.2 (Patchright = real Chromium no-CDP, still fails), §3.4 (bestbuy = hydration+edge)
- `docs/releases/v0.1.0-parity/26_AKAMAI_BMP_DEEP.md` §2 (BMP field taxonomy + `_abck` state machine), §4.3 (bestbuy splash + cross-engine table at :503), §1 (`aecdf19` strip boundary)
- `docs/vNext/03_R-BESTBUY-AKAMAI.md` (R-ticket, premised on falsified Patchright pass)
- `docs/HANDOFF_2026_05_28b.md` §4 (AWS live-nav-drain root cause), §5.1 (the shared fix)
- `crates/browser/src/classify.rs:115-116` (`akam/13`/`_abck` rows), `:139-143` (`AKAMAI_CHALLENGE_COSIGNAL`), `:174` (akam/13 co-signal gate), `:560-572` (bestbuy shell test fixture)
- `crates/browser/src/page.rs:1687-1707` (warm-rebuild script loop + 50 ms drain — the truncation), `:1755-1758` (500 ms final drain), `:1977-1985` (bestbuy 25 s plain-BMP budget tier), `:2102-2106` (adaptive per-iteration drain, floored 8 s / capped nav_budget)
- `crates/browser/src/js/humanize.js:69-77` (`__akamai_events`), `:105,429,441` (`isTrusted=true` override), `:360-376` (Plamondon sigma-lognormal trajectory) — present + sophisticated but inert without a `vendor_solvers` encoder
- `crates/stealth/src/behavior.rs::mouse_trajectory` (the Plamondon generator backing humanize.js)
