# SITE — bestbuy.com (Akamai BMP SPA shell)

**Cluster:** Akamai Bot Manager Premier (BMP) on a React SPA.
**Stratum (corrected):** **C — no engine tested passes.** (The R-ticket's
"Stratum B, Patchright passes 1246k" is a data-misread; see §0.)
**Measured (full sweep 2026-05-27, `/tmp/full_sweep_2026_05_27/`):**

| engine | body | verdict |
|---|--:|---|
| BO chrome_148_macos | 7943 b | ThinShell (loose L3, sub-15 KB) |
| BO pixel_9_pro_chrome_148 | 7889 b | ThinShell |
| BO iphone_15_pro_safari_18 | 7943 b | ThinShell |
| BO firefox_135_macos | 7943 b | ThinShell |
| Camoufox v135 | 7467 b | ThinShell |
| **Camoufox v150** | **7467 b** | **ThinShell** |
| Patchright (real Chromium) | 7105 b | ThinShell |
| Playwright | 7340 b | ThinShell |
| Playwright-stealth | 7340 b | ThinShell |

Every engine — including real-Chromium Patchright and SOTA Camoufox v150 —
lands on the same ~7–8 KB shell. **This is a true open-source frontier
site, not a BO-specific gap.** Flipping it would put BO *ahead* of v150
(net +1 over the field), but nothing in the public corpus tells us *how*,
because no reference engine demonstrates the win.

---

## §0 — Correcting the record (load-bearing)

Two repo docs assert bestbuy is **Stratum B** with **Patchright passing at
1246 k**:

- `docs/vNext/03_R-BESTBUY-AKAMAI.md` lines 12, 42: "Patchright result
  (HANDOFF §1.8): **PASS 1246 k**."
- `docs/releases/v0.1.0-parity/audit/16_DECISION_LOG.md` line 131:
  "Patchright (real Chromium): **PASS 1246 k** per HANDOFF §1.8."

**This is wrong.** The authoritative cross-engine table in
`docs/releases/v0.1.0-parity/FAILED_SITES_ANALYSIS.md` (line 36) shows
**bestbuy Patchright = 7105 b**, and `1246 k` is the **homedepot**
Patchright number on the very next data row (line 38,
`homedepot … Patchright PASS 1246k`). The R-ticket and decision-log lines
copied the wrong row. The FAILED_SITES classifier (line 13, line 36) is
internally consistent and correctly lists bestbuy under **Stratum C: "No
engine tested passes — true open-source frontier … 1 Akamai bestbuy."**

**Consequence for planning:** the R-ticket's entire diagnostic premise —
"Patchright passes, so it IS Chromium-engine-reachable; just diff BO vs
Patchright sensor_data" — collapses. There is no passing reference to diff
against. The 2-day "interactive probe" effort estimate stands, but its
*purpose* changes from "find the BO↔Patchright delta" to "find out whether
bestbuy's full homepage is even reachable headless from a datacenter IP at
all." That is a materially different (and lower-confidence) bet.

A second contradiction to resolve: `26_AKAMAI_BMP_DEEP.md §4.3` confidently
classifies the 7.9 KB body as a **benign "Choose a country" i18n splash**
("NOT an Akamai block … benign content-routing splash … out of scope"),
while `FAILED_SITES_ANALYSIS.md` and the R-ticket treat it as an Akamai
**conditional-hydration block**. These cannot both be the headline cause.
§3 below explains why the truth is probably "both, layered" and how to
disambiguate — this is the central open question for the site.

---

## §1 — What the existing repo docs concluded

### 1.1 `26_AKAMAI_BMP_DEEP.md §4.3` — "benign i18n splash"
- All 8 engines land ~7–8 KB; the doc reads this as Best Buy's
  "Choose a country" splash served to first-time visitors **regardless of
  country/IP/headers**.
- The classifier correctly does **not** flag it `Akamai-CHL`: the body has
  `akam/13` (the BMP bootstrap that loads on *every* Akamai-fronted page)
  but **none** of the `AKAMAI_CHALLENGE_COSIGNAL` markers (`sensor_data`,
  `bm-verify`, `sec-if-cpt-container`, `sec-cpt-if`, `/_sec/cp_challenge`,
  `pardon our interruption`) — so the over-match guard at
  `classify.rs:160-167` keeps it `L3-RENDERED`, not a challenge.
- Verdict in that doc: "**bestbuy is not in the chapter 26 critical
  path** … out of scope for v0.1.0. Advancing past the splash needs a
  country-select form submit (a UX flow, not anti-bot)."

### 1.2 `FAILED_SITES_ANALYSIS.md` — "Akamai SPA shell, conditional hydration"
- Stratum C (line 13): "1 Akamai bestbuy" among 7 true frontier sites.
- Action item 11 (line 155): "**bestbuy interactive probe** — drive a
  manual Playwright run and see if the SPA hydrates after a click/scroll.
  If yes → behavioural signal needed. If no → trust signal is something we
  haven't identified." Effort: 2 days, fix unknown.
- `audit/15_FIX_PRIORITY_RANKED.md` line 198 ranks R-BESTBUY-AKAMAI #9 of
  12 (2 days, public engine), explicitly *below* the high-ROI x-com / AWS /
  booking work. The summary (line 203) calls bestbuy a "Stratum C residual
  … out-of-public-engine-scope or research-grade."

### 1.3 `audit/16_DECISION_LOG.md §R-BESTBUY-AKAMAI`
- Captured this session: `curl https://www.bestbuy.com/` Chrome-148 UA +
  HTTP/2 → **stream 1 RST, error 0x2 INTERNAL_ERROR**; `--http1.1` → **30 s
  timeout, 0 bytes**. The Akamai edge actively refuses naked-curl-class
  requests from this datacenter IP at the connection layer.
- BO's chrome_147-class TLS impersonation **gets past the edge** (it
  receives the 7 KB shell where naked curl gets nothing) — so the edge
  ClientHello/HTTP-2 gate is *not* BO's blocker.
- Scope decision: **DEFER to v0.2.x+** (premised on the now-falsified
  Patchright-passes claim).

### 1.4 `26_AKAMAI_BMP_DEEP.md §§1–3` — the vendor boundary
- `aecdf19` stripped all 9 Akamai source files (`crates/akamai/*`, 3 565
  LOC) plus the `AkamaiSolver`/`handle_akamai_flow` wiring. **The public
  tree has no Akamai sensor_data encoder, no `_abck` state machine, no
  TEA-CBC, no sec-cpt PoW.** Those live in the private `vendor_solvers`
  crate.
- What survives in public: the `started_as_seccpt_challenge` body flag
  (`page.rs:1649-1650`), `_abck`/`bm_sz` markers in `v8_html_is_real`, the
  classifier rows + co-signal gate, and the now-dead `__akamai_events` JS
  collector (`humanize.js`). bestbuy's flow is **plain BMP sensor_data**
  (no sec-cpt), so none of the surviving sec-cpt machinery applies to it.

---

## §2 — New external findings

External sources confirm the mechanism class and the difficulty, but none
provide a free headless win:

- **Akamai's own Bot Manager docs** confirm BMP scores every request
  starting from the very first connection (0=human … 100=bot) and is built
  specifically for "modern JavaScript-heavy single-page applications like
  React" — exactly bestbuy's shape. (akamai.com/products/bot-manager;
  technologychecker.io lists 177k+ domains, major retailers, on BMP.)
- **The sensor flow is `_abck` + `bm_sz` + a `sensor_data` POST**, and
  every public bypass guide treats Best Buy / Akamai retail as a
  *paid-solver-or-residential-proxy* target, not a fingerprint-only flip:
  - Scrapeless "Bypass Akamai with Playwright": base Playwright "often
    fails against Akamai's sophisticated defenses"; their pitch is a hosted
    Web-Unlocker that *fetches `_abck`/`bm_sz` for you* — i.e. a
    server-side sensor_data generator + proxy rotation, not an engine fix.
  - Bright Data "Bypass Akamai Bot Detection": same conclusion — Akamai
    retail needs proxies + a managed unlocker.
  - ScrapingBee / ZenRows / Decodo Camoufox guides: Camoufox reliably
    flips *Cloudflare*; **none of them claim an Akamai-retail flip**, which
    matches our measurement (v150 = 7467 b, no flip).
- **Datacenter-IP reputation is a first-class BMP input.** The curl-level
  RST on the bare connection (audit/16) is Akamai's edge IP/ASN gate. BO's
  TLS impersonation clears the *edge*, but BMP's *scoring* layer still
  weights the datacenter ASN. This is the single most likely reason *no*
  engine flips bestbuy from this IP — and it is **not** an engine-fixable
  signal.

**Net of external research:** the public consensus is that Best Buy /
Akamai-retail is a **proxy + sensor_data-generator** problem, not a
fingerprint-fidelity problem. That aligns with CLAUDE.md's scope boundary:
the sensor_data generator belongs in `vendor_solvers`, and the proxy/IP
input is outside the engine entirely.

Sources:
- https://www.akamai.com/products/bot-manager
- https://technologychecker.io/technology/akamai-bot-manager
- https://www.scrapeless.com/en/blog/bypss-akamai-with-playwright
- https://brightdata.com/blog/web-data/bypass-akamai-bot-detection
- https://www.scrapingbee.com/blog/how-to-scrape-with-camoufox-to-bypass-antibot-technology/
- https://www.zenrows.com/blog/web-scraping-with-camoufox

---

## §3 — Code-level analysis & the disambiguation that matters

### 3.1 How BO classifies the 7943 b body today
- Thresholds (`crates/browser/src/classify.rs:37-47`):
  `THIN_BODY_MAX_BYTES = 1000`, `THIN_SHELL_MAX_BYTES = 15*1024`,
  `INTERSTITIAL_MAX_BYTES = 30*1024`.
- `engine_classify` (`classify.rs:199-227`) returns `tag="L3-RENDERED"`
  for the 7943 b body because no SMALL_BODY challenge row qualifies (the
  `akam/13` row is gated behind a missing co-signal,
  `small_body_row_qualifies`, `classify.rs:160-167`).
- `classify_challenge` (`classify.rs:178-181`) then maps
  `"L3-RENDERED" if len < THIN_SHELL_MAX_BYTES` → `ChallengeVerdict::ThinShell`.
- **So BO's own ledger already says the right thing:** bestbuy is a
  *thin shell / pre-hydration stub*, not a solved page and not a detected
  challenge. The holistic `tag` stays `L3-RENDERED` (no regression noise),
  but it is **not** a strict pass (needs ≥ 15 KB, `classify.rs:180-181`).

### 3.2 The two competing root-cause hypotheses, and how to tell them apart
The whole site hinges on which of these is true:

**H-A — Benign country/i18n splash (doc 26 §4.3).** The 7 KB body is a
deliberate Best Buy content-routing splash; the real homepage is gated
behind a country-select interaction, not an anti-bot trust signal. If true,
the fix is a **scripted interaction** (click the US locale / dismiss the
splash) and bestbuy becomes a *navigation-flow* problem, fully
public-engine and ~1–2 days.

**H-B — Akamai conditional hydration / trust gate (FAILED_SITES).** The
7 KB body is the React entry shell; hydration fetches only fire after the
`akam/13` sensor_data POST returns an `_abck` *without* the trailing
`~-1~-1` (Favorable). If BO/v150/Patchright all get a *Rejected* or
*Untrusted* `_abck` (likely from the datacenter ASN), no engine hydrates.
If true, the fix is a **sensor_data generator + IP** problem →
`vendor_solvers` + proxy, **not** public-engine.

**These are distinguishable with one capture run.** The disambiguating
probe (this is the concrete "interactive probe" the R-ticket asked for):

```bash
# 1. Capture BO's full request flow + cookies + DOM on bestbuy.
RUST_LOG=net=trace target/release/examples/sweep_metrics chrome_148_macos \
  <(echo '[{"cat":"stores","name":"bestbuy","url":"https://www.bestbuy.com/"}]') \
  /tmp/bestbuy_probe/sweep.json > /tmp/bestbuy_probe/trace.log 2>&1
```
Then answer three yes/no questions from the trace:

1. **Does the 7 KB body contain a country-select / `<form>` / locale
   chooser, or a React root `<div id="root">` with an empty body?**
   `grep -i "choose a country\|us-en\|/site/\|root\b" /tmp/bestbuy_probe/body.html`.
   - Country form present → **H-A**. React root only → **H-B**.
2. **Does the `akam/13` bootstrap fire a `sensor_data` POST, and what
   `_abck` comes back?** Parse the cookie writes for `_abck=…`; classify
   the trailing segments with the *public* state-machine spec from
   `26_AKAMAI_BMP_DEEP.md §3.A` (`~-1~-1~-1~`=Favorable, `~0~-1~-1~`=
   Untrusted, `~3~…`=Rejected).
   - No `sensor_data` POST at all → BO's `humanize.js` collector isn't
     being consumed by anything (expected: empty solver set) **and** the
     site's own `akam/13` bundle isn't self-posting → **H-B, JS-execution
     sub-variant** (the BMP bundle needs its async chain to run, like the
     AWS challenge.js live-nav-drain bug, §3.4).
   - `_abck=…~0~-1~-1~`/`~3~` → **H-B, trust/IP gate** (vendor + proxy).
3. **Does real Chromium (Playwright, same datacenter IP) hydrate past
   7 KB if you script a 2-second mouse-move + scroll + click on the US
   tile?** Use the Playwright MCP (already available) to drive
   `bestbuy.com` interactively from the same box:
   `browser_navigate` → `browser_wait_for(2s)` → `browser_evaluate(document.body.innerText.length)`.
   - Jumps to ≫ 7 KB after interaction → **behavioural-signal gate**
     (humanize.js needs richer trajectories; public-engine fixable).
   - Stays ~7 KB even in real Chromium with real interaction → **trust/IP
     gate**; confirms no public-engine lever exists from this IP.

This single run + MCP interaction collapses three unknowns (splash vs
trust vs behavioural) into one answer. It is the missing experiment; the
R-ticket never ran it (it deferred on the false "Patchright passes" read).

### 3.3 The behavioural surface BO already ships (`humanize.js`)
If the probe lands on the behavioural branch, the relevant code is
`crates/browser/src/js/humanize.js` (458 lines):
- Maintains `globalThis.__akamai_events` (line 69-77): `mouse[]`, `key[]`,
  `touch[]`, `scroll[]`, `counters{key,mouse,touch,scroll,accel}` — exactly
  the buffer the stripped Akamai sensor encoder used to read.
- Dispatches `mousemove`+`pointermove` pairs (lines 239-256, 416-450),
  `wheel`+`scroll` pairs (lines 273-285), `focus`/`visibilitychange`
  (lines 294-295). These are real `dispatchEvent` calls on
  `window`/`document`/`body`, but synthesized `MouseEvent`s have
  `isTrusted=false` (line 39 acknowledges this).
- **Two known weaknesses for BMP behavioural scoring:** (a) `isTrusted`
  is false — BMP's higher tiers can weight this; real Chromium events are
  trusted (this is part of why Patchright > BO on *some* Akamai sites, but
  here Patchright also fails, so it's not the dominant signal on bestbuy);
  (b) the synthesized trajectories are simple linear/`scrollBy` motions,
  not the curved, variable-velocity, target-seeking trajectories BMP's
  field-65 mouse model scores (`26_AKAMAI_BMP_DEEP.md §2.3`).
- **But:** with an **empty solver set** (`Page::default_solvers()`
  returns `Arc<[]>`, per `26 §1`), nothing in the public engine actually
  *reads* `__akamai_events` and *posts* it to Akamai. So even a perfect
  trajectory model produces no sensor_data POST in the public build. The
  behavioural surface is a *prerequisite* the private solver consumes, not
  a standalone lever.

### 3.4 The AWS-WAF live-nav-drain analogy (the one public-engine lever)
`HANDOFF_2026_05_28b §4` found the AWS cluster's real blocker: an external
challenge.js *runs fully in the offline oracle* but produces **zero async
progress in the live navigate path** because
`build_page_with_scripts_init_and_storage` (`crates/browser/src/page.rs`
~3340-3620) only does a **50 ms inter-script `run_until_idle` drain**
(~line 3535) vs the oracle's 5 s idle drain — so the deferred worker /
promise chain never advances before the nav loop re-fetches.

**bestbuy's `akam/13` bundle is the same shape:** an external, async,
self-posting script. If probe question 2 shows the bundle is fetched but
never posts `sensor_data` live, bestbuy is plausibly **gated by the same
50 ms-drain bug**, not by fingerprint or trust. That would make it
**public-engine-fixable for free** as a side-effect of §5.1 of the AWS
work (the live-nav drain fix) — the single highest-leverage scenario for
this site. This is the one hypothesis worth eagerly testing because it
costs ~nothing if the AWS lever is being built anyway.

### 3.5 The nav budget is already tuned for bestbuy
`page.rs:1940-1948` gives `*.bestbuy.com` (and nike/adidas/samsclub/
walmart) the 25 s "plain-BMP" budget; the comment at line 1936-1937
explicitly notes "bestbuy is the benign i18n splash — Task#1 — so it stays
in the plain-BMP tier." So a *drain-duration* fix would need bestbuy bumped
to a longer budget tier *if* §3.4 turns out to be the cause. Cheap (1-line
host match), but only worth it if the probe confirms the drain branch.

---

## §4 — Is bestbuy winnable at all, and how?

**Honest answer: probably not in the public engine from a datacenter IP,
and definitely not as a quick win.** The dispositive fact is that **Camoufox
v150 AND real-Chromium Patchright both fail identically (7467 b / 7105 b).**
When the SOTA stealth engine *and* a real browser both fail from the same
IP, the residual is overwhelmingly **IP/ASN reputation + a vendor
sensor_data requirement**, neither of which is a public-engine fingerprint
lever.

The only public-engine-winnable scenarios, in descending likelihood of
being real:

1. **H-A benign country splash** → scripted US-locale interaction flips it.
   *Winnable, public-engine, cheap* — but doc 26 §4.3's own logic (real
   Chromium *also* gets 7 KB) argues against a pure-UX splash, because a
   real browser would just render the splash's interactive content and a
   human/Playwright click would advance it. Test with probe Q1+Q3.
2. **§3.4 live-nav-drain** → the AWS-cluster drain fix incidentally lets
   the `akam/13` bundle complete its async self-post. *Winnable,
   public-engine, near-free if AWS work proceeds.* Test with probe Q2.
3. **H-B behavioural gate** → richer humanize trajectories + a sensor_data
   generator. *Generator is `vendor_solvers` (out of public scope); the
   trajectory model is public but inert without the generator.* Even then,
   the IP gate likely dominates.
4. **H-B trust/IP gate** → *not winnable in any engine from this IP.*
   Needs residential/mobile proxy + sensor_data generator. Out of
   public-engine scope entirely.

**Recommendation:** do **not** invest the 2-day probe as a standalone
task. Instead, **fold the bestbuy capture into the AWS live-nav-drain work
(HANDOFF §5.1)** — when that instrumentation is live, run the three probe
questions on bestbuy in ~1 hour. If Q2 shows the drain branch (scenario 2),
bestbuy flips for free and BO beats v150 by +1. If not, **re-classify
bestbuy as Stratum-C / vendor_solvers+proxy and stop**, exactly as the
audit/15 ranking (#9, "research-grade") already implies. Keep it out of the
headline pass-rate the way `26 §4.3` advised.

---

## §5 — Ranked fix list (ROI order)

| # | Fix | Effort | Expected site impact | Confidence | Public engine? |
|---|---|---|---|---|---|
| 1 | **Run the 3-question disambiguation probe** (§3.2) — capture BO flow + `_abck` state + Playwright-MCP interactive hydration, same IP. Decides which of H-A / drain / H-B / IP is true. | ~1 hr (fold into AWS §5.1 capture) | 0 directly (enabler); unblocks the right fix | high | yes (diagnostic) |
| 2 | **Reuse the AWS live-nav-drain fix** (HANDOFF §5.1; `page.rs` ~3535 50 ms→long drain for async-script pages) and verify bestbuy's `akam/13` bundle then self-posts `sensor_data`. | near-zero *if* AWS work proceeds; else folded into that 3-5 day task | bestbuy flips → BO **ahead of v150 by +1** — only if Q2 = drain branch | low-med | yes |
| 3 | **Scripted US-locale / splash-dismiss interaction** in the nav flow, gated on the bestbuy body shape (only if probe Q1 shows a country form). | 1-2 days | bestbuy flips if H-A true | low | yes |
| 4 | **Richer humanize.js trajectories** (curved, variable-velocity, target-seeking mouse; trusted-event path) to satisfy BMP field-65. | 1 week | 0 alone (inert without a sensor_data generator); helps the whole Akamai cluster *if* a private solver is registered | low | yes (surface only) |
| 5 | **Akamai sensor_data generator + `_abck` state machine** (the stripped `crates/akamai/*`). | 2-4 weeks | bestbuy + the Akamai cluster, *with* a clean IP | med | **NO — `vendor_solvers`** |
| 6 | **Residential/mobile proxy for the bestbuy fetch.** | infra, not code | likely necessary for any engine to flip from this ASN | med-high | NO (out of engine) |

**Bottom line:** the highest-ROI move is fix #1+#2 piggy-backed on the AWS
work — it is the only path that could flip bestbuy *for free in the public
engine* and put BO ahead of v150. Everything else is either out-of-scope
(`vendor_solvers`/proxy) or a low-confidence bet on the benign-splash
hypothesis. Correct the two docs that mis-cite "Patchright PASS 1246k"
(§0) so future planners don't chase a non-existent Stratum-B diff.
