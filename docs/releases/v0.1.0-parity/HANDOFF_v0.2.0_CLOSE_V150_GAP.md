# v0.2.0 Handoff — Close the Camoufox v150 Gap

**Author:** prior session, 2026-05-27
**Status:** open. v0.1.0-parity gate failed under strict HANDOFF §6 (routed median 107 < 113 bar). Camoufox v150 (released 2026-05-11, beta-only) reaches 115 — that is the new bar.
**Goal:** ship engine-side changes that bring browser_oxide's routed best-of-4 median from **107 → ≥ 115** to match or exceed Camoufox v150. 12 R-* tasks queued; full per-site root-cause map exists. This doc tells you everything needed to start.

---

## 0. Read these first (in this exact order)

1. **[VERIFICATION.md](VERIFICATION.md)** — the 2026-05-27 verification ledger. Pre-conditions, per-sweep matrix, per-profile medians, final routed best-of-4, competitor head-to-head, and the §6d-bis SharedSession A/B that proved cookies (not accept_ch) bleed across to x-com. **Skim §3, read §4 + §5 + §6 + §6d-bis fully**, glance at §10 acceptance verdict.
2. **[FAILED_SITES_ANALYSIS.md](FAILED_SITES_ANALYSIS.md)** — per-site root cause + concrete action items for each of the 19 sites BO doesn't pass. Three strata: A (11 sites Camoufox v150 passes — engine-addressable), B (1 site Patchright-only), C (7 sites no engine passes). **Read all of it** — every R-* in §1 below maps to a section here.
3. **[HANDOFF.md](HANDOFF.md)** — the prior session's handoff for the 12-fix v0.1.0-parity stack. §1 shows the branch structure (`fix/v0.1.0-fix4-canvas-parity` integration HEAD = `f625ab6`); §2 the per-fix shipped; §7 known issues (R-V8-TERM still open). **Read for context** — your work continues on top of this branch.
4. **[EXECUTION_PLAN.md](EXECUTION_PLAN.md)** — the prior-session per-fix execution template. Each fix had: file:line pointers, commands, expected diff, validation. **Use this as your spec template** for each R-* you take on.
5. **[02_GAP_ANALYSIS.md](02_GAP_ANALYSIS.md)** — per-site evidence + root cause for the original 10 Camoufox-only-pass sites (some now also v150-only-pass). Each entry has observed-flow → hypothesis → first-debug-step. **Reference per-site as you start an investigation.**
6. **[15_OPEN_QUESTIONS.md](15_OPEN_QUESTIONS.md)** — research backlog convention. New R-* you discover go here.
7. **[CLAUDE.md](../../../CLAUDE.md) + [SCOPE.md](../../../SCOPE.md)** — what's in/out of scope for the public engine vs `vendor_solvers` private repo. Critical: AWS WAF / DataDome WASM / Akamai sensor_data / Kasada solvers belong to `vendor_solvers`. Fingerprint surface improvements belong here.
8. **Per-cluster chapters** (read when you take on a related R-*):
   - [05_SPA_HYDRATION_CLUSTER.md](05_SPA_HYDRATION_CLUSTER.md) — booking, douyin, duolingo
   - [06_AWS_WAF_SOLVER.md](06_AWS_WAF_SOLVER.md) — amazon variants, imdb
   - [07_DATADOME_PRIMITIVES.md](07_DATADOME_PRIMITIVES.md) — etsy
   - [08_KASADA_FRONTIER.md](08_KASADA_FRONTIER.md) — canadagoose, hyatt, realtor
   - [16_STEALTH_FINGERPRINT_AUDIT.md](16_STEALTH_FINGERPRINT_AUDIT.md) — the fingerprint surface inventory (R-FP-AUDIT input)
   - [17_WEB_API_PARITY_MATRIX.md](17_WEB_API_PARITY_MATRIX.md) — JS API surface coverage
   - [25_CLOUDFLARE_DEEP.md](25_CLOUDFLARE_DEEP.md), [26_AKAMAI_BMP_DEEP.md](26_AKAMAI_BMP_DEEP.md), [28_AWS_WAF_EXTENDED.md](28_AWS_WAF_EXTENDED.md) — per-vendor deep dives
   - [14_TESTING_VALIDATION.md](14_TESTING_VALIDATION.md) — L1-L5 validation layers; §L5 3-run aggregated sweep is how you verify any fix lands

**If you only have 30 minutes:** read VERIFICATION.md §6 + §10, FAILED_SITES_ANALYSIS.md §TL;DR + §"Suggested execution order", and this doc's §1-2.

---

## 1. The 15 tasks this session generated (12 open + 3 completed)

15 R-* tasks were filed in the 2026-05-27 verification + analysis session. Three are already done (§1.0); twelve remain (§1.1 — §1.12). Each open task includes: **what**, **why it's the right cluster**, **file pointers**, **commands**, **expected outcome**, **effort**, **scope** (public engine vs vendor_solvers), **sites in scope**.

### Quick-reference table (full details below)

| § | ID | Status | Sites | Effort | Scope | Section |
|---|---|---|---|---|---|---|
| 1.0 | R-CORPUS-WILDBERRIES | ✅ done | — | 30 min | corpus | (completed; see §1.0) |
| 1.0 | R-CORPUS-PROBE-BUCKET | ✅ done | — | 30 min | corpus | (completed; see §1.0) |
| 1.0 | R-SHAREDSESSION-X-COM | ✅ done | (diagnosis) | 2h | public | (completed; see §1.0) |
| **1.1** | **R-FP-AUDIT-2026Q3** | open | **up to 8** | 2-3 weeks | public | [→](#11--r-fp-audit-2026q3-highest-leverage-multi-week) |
| 1.2 | R-SHAREDSESSION-X-COM-COOKIES | open | 1 (x-com) | 1 week | public | [→](#12--r-sharedsession-x-com-cookies-1-site-multi-day-investigation) |
| 1.3 | R-DUO-WORKER | open | 1 (duolingo) | 1 week | public | [→](#13--r-duo-worker-1-site-multi-day-investigation) |
| 1.4 | R-SPA-BOOKING-FETCH-CHAIN | open | 1 (booking) | 3-5 days | public | [→](#14--r-spa-booking-fetch-chain-1-site-3-5-days) |
| 1.5 | R-AKAMAI-SECCPT-FLAKE | open | 1 (homedepot) | 2-3 days | public | [→](#15--r-akamai-seccpt-flake-1-site-2-3-days) |
| 1.6 | R-SPA-DOUYIN-SIG | open | 1 (douyin) | 1-2 weeks | public | [→](#16--r-spa-douyin-sig-1-site-1-2-weeks) |
| 1.7 | R-AWSWAF-OFFLINE-PROBE | open | 0 (enabler) | 1 week | public | [→](#17--r-awswaf-offline-probe-0-sites-1-week--enabler) |
| 1.8 | R-BESTBUY-AKAMAI | open | 0-1 (bestbuy) | 2 days | public | [→](#18--r-bestbuy-akamai-1-site-2-days-investigation) |
| 1.9 | R-CORPUS-DIAGNOSTIC-FLAG | open | 0 (metric) | 1 day | public | [→](#19--r-corpus-diagnostic-flag-corpus-cleanup-1-day) |
| 1.10 | R-WBAAS-WILDBERRIES | open | 1 (wildberries) | unknown | likely vendor_solvers | [→](#110--r-wbaas-wildberries-1-site-multi-day-research) |
| 1.11 | R-KASADA-FRONTIER | open | 3 | months | vendor_solvers | [→](#111--r-kasada-frontier-3-sites-months-vendor_solvers) |
| 1.12 | R-DATADOME-DAILY-KEY | open | 1 (etsy) | unknown | vendor_solvers | [→](#112--r-datadome-daily-key-1-site-unknown-vendor_solvers) |

### 1.0 — Already completed in prior session (3 tasks; do NOT redo)

| ID | What was done | Outcome | Where to read |
|---|---|---|---|
| **R-CORPUS-WILDBERRIES** | Verified wildberries from the datacenter IP — NOT geo-blocked. Returns HTTP 498 + 1447-byte body loading `/__wbaas/challenges/antibot/__static/v1/index-DQJ0L4Mq.js` (Wildberries' own wbaas antibot with site-key `7400bd5df8b843b28254659f10915f31`). Title: "Почти готово…". Same response Camoufox v150 + Patchright get. | Reclassified as `wbaas` antibot cluster (not corpus-quality issue). **Follow-up R-WBAAS-WILDBERRIES still open** — see §1.10. | `VERIFICATION.md` decision log, this doc §1.10 |
| **R-CORPUS-PROBE-BUCKET** | Documented `areyouheadless` as a diagnostic-only probe (by-design failure). Defined the dual-metric convention: **raw pass-rate** = strict/126, **production pass-rate** = strict/(126 − N_probe) where N_probe = 1. | BO routed median production rate = 107/125 = 85.6%; Camoufox v150 production = 115/125 = 92.0%. Doc-only convention; **structural refactor still open** as R-CORPUS-DIAGNOSTIC-FLAG (§1.9). | `VERIFICATION.md §6e` |
| **R-SHAREDSESSION-X-COM** | A/B-tested SharedSession on/off for x-com. **Confirmed isolation makes x-com pass** (THIN-BODY 69 → L3-RENDERED 273922). Then narrower A/B with `NO_SHARED_ACCEPT_CH=1` (cookies shared, accept_ch isolated): **x-com still fails** → accept_ch is NOT the bug; **cookies are**. Originally proposed fix (per-origin accept_ch scoping) was wrong. | Env-var diagnostic toggles `BROWSER_OXIDE_NO_SHARED_SESSION` / `_COOKIES` / `_ACCEPT_CH` landed in `crates/net/src/lib.rs::HttpClient::shared` (uncommitted). **Follow-up cookie investigation** is **R-SHAREDSESSION-X-COM-COOKIES** (§1.2). | `VERIFICATION.md §6d-bis` |

### 1.1 — R-FP-AUDIT-2026Q3 (highest leverage, multi-week)

**What:** Compare browser_oxide's fingerprint surface against Camoufox v150's source. Enumerate every `navigator.*` / `screen.*` / `WebGLRenderingContext.*` / `AudioContext.*` / `AnalyserNode.*` / `MediaDevices.*` getter; for each, identify whether BO leaks the engine identity, returns a wrong per-profile value, or fails cross-API correlation (e.g. UA says iPhone but `deviceMemory` says 32).

**Why it's the right cluster:** Camoufox v150's "Hardware Spoofing" lineage (`v146-hardware` → `v150.0.2-beta.25`) flipped 7 AWS WAF sites + booking + douyin without vendor solvers. **The threshold is fingerprint-class, not vendor-WAF-class.** This is the single highest-leverage item — up to 8 sites recovered.

**Empirical evidence — the EXACT 8 sites v150 gained over v135 in the 2026-05-27 sweep:**

```
+ adidas         (was already passing on BO; v135 had a regression v150 reverted)
+ amazon-ca      (was 5524b on v135; v150 = PASS 215k)
+ amazon-co-uk   (was passing on v135 too — possibly noise here)
+ amazon-com     (was 2008b on v135; v150 = PASS 1286k)
+ amazon-in      (was 2005b on v135; v150 = PASS 708k)
+ amazon-jp      (was 2008b on v135; v150 = PASS 850k)
+ booking        (was 8403b on v135; v150 = PASS 513k)
+ zillow         (was already passing on BO; v150 brought zillow back to passing too)
```

**The EXACT 11 sites v150-vs-BO routed-median delta (= the v0.2.0 target list):**

```
amazon-ca        (BO 2011b across 4 profiles  | v150 PASS 215k)
amazon-com       (BO 2011-14b across 4 profiles | v150 PASS 1286k)
amazon-com-au    (BO 2011-14b ; pixel run1 PASS 1041k once | v150 PASS 945k)
amazon-fr        (BO 2011b uniformly | v150 PASS 871k)
amazon-in        (BO 2011b uniformly | v150 PASS 708k)
amazon-jp        (BO 2011b ; iphone run2 PASS 850k once | v150 PASS 850k)
imdb             (BO 1995b uniformly | v150 PASS 1068k)
booking          (BO 8473b ; iphone 3891b | v150 PASS 513k)
douyin           (BO 6327b EXACTLY across all 4 profiles → deterministic FP detection | v150 PASS 1020k)
duolingo         (BO 13327-13566b — 1.7KB shy of 15KB strict | v150 PASS 697k)
x-com            (BO THIN 69 mid-sweep | v150 PASS 379k) — also covered by R-SHAREDSESSION-X-COM-COOKIES (cookie bleed, not fingerprint)
```

**Files:**
- `crates/stealth/src/` — current stealth profile definitions
- `crates/js_runtime/src/js/dom_bootstrap.js` — JS-side getter installations + `Function.toString` masks (Fix 3 reference)
- `crates/js_runtime/src/js/cleanup_bootstrap.js` — Fix 1+3 universal mask sweep (reference for "how we patch a prototype")
- `crates/browser/src/page.rs` — engine-side hardware injection
- `docs/releases/v0.1.0-parity/16_STEALTH_FINGERPRINT_AUDIT.md` — the existing inventory; **start here, augment with v150 deltas**
- **Camoufox v150 source:** https://github.com/daijro/camoufox/tree/v150.0.2-beta.25 — diff against. Their `additions/` directory contains the Firefox patches.
- `~/.cache/camoufox/` — currently has the v150 binary installed (live test target); v135 backup at `~/.cache/camoufox.v135.bak/`

**Commands:**
```bash
# Reproduce the v150 sweep that established the 115 baseline
CORPUS_FILE=/tmp/corpus.json PLAYWRIGHT_BROWSERS_PATH=/home/yfedoseev/.cache/ms-playwright \
  /tmp/bo-venv/bin/python /home/yfedoseev/projects/browser_oxide/benchmarks/bench_corpus_v2.py camoufox \
  /tmp/full_sweep_2026_05_27/comp_camoufox_v150_repro.json

# Per-site delta v150-vs-BO (live)
python3 /tmp/cmp_v150.py    # script staged from prior session; if missing see VERIFICATION.md §5e-bis for the source
```

**Expected outcome:** a structured report (extend `16_STEALTH_FINGERPRINT_AUDIT.md`) of every fingerprint surface where BO and v150 differ. For each diff, mark: (a) cross-engine fingerprintable? (b) appears in AWS WAF / DataDome / Cloudflare challenge JS? (c) shippable fix complexity. Pick top 5-10 by Site-yield × Effort and implement as a stacked v0.2.0 fix series, following the EXECUTION_PLAN.md template.

**Effort:** 2-3 weeks for the audit + initial fixes. Subsequent fixes can stream in as smaller PRs.

**Scope:** public engine. The fingerprint-surface work is explicitly in `SCOPE.md` ("stealth by design").

**Sites in scope:** amazon-ca, amazon-com, amazon-com-au, amazon-fr, amazon-in, amazon-jp, imdb, booking, douyin (9 of the 11 Stratum-A sites).

---

### 1.2 — R-SHAREDSESSION-X-COM-COOKIES (1 site, multi-day investigation)

**What:** Identify which cookie set by the twitter visit (corpus site 25) poisons the subsequent x-com visit (corpus site 26 — same canonical site). Propose a fix that doesn't regress yandex / duckduckgo / microsoft / homedepot / quora / adidas (the cookie-history-dependent sites).

**Why:** Prior session A/B (VERIFICATION.md §6d-bis) **disproved** the original accept_ch hypothesis. With `BROWSER_OXIDE_NO_SHARED_ACCEPT_CH=1` x-com still fails. With `BROWSER_OXIDE_NO_SHARED_SESSION=1` it passes (but yandex/microsoft/duckduckgo regress). Therefore the leak is cookie-state. Likely a bot-detection cookie (`_twitter_sess` / `guest_id` / similar) that, when sent on the immediately-following x-com request, trips x.com's WAF.

**Files:**
- `crates/net/src/lib.rs` — `SharedSession.cookies` (line ~134), `HttpClient::shared` (line ~326, with the env-var toggles for A/B testing already landed); `learn_accept_ch` / `has_accept_ch` (lines ~382 / 410)
- `crates/net/src/cookie_jar.rs` (or wherever `CookieJar` lives — search)
- `docs/releases/v0.1.0-parity/02_GAP_ANALYSIS.md §10` — original (now-superseded) hypothesis
- `docs/releases/v0.1.0-parity/VERIFICATION.md §6d-bis` — the A/B result that revised the hypothesis

**Commands:**
```bash
# The diagnostic toggles already landed in crates/net/src/lib.rs. Use them:
BROWSER_OXIDE_NO_SHARED_COOKIES=1 target/release/examples/sweep_metrics chrome_148_macos \
  /tmp/corpus.json /tmp/fix12_gate/chrome_no_cookies.json
# If x-com PASSES under cookies-isolated but accept_ch-shared → confirms cookies are the bug

# Then capture what cookies twitter set:
RUST_LOG=net=trace target/release/examples/sweep_metrics chrome_148_macos \
  <(echo '[{"cat":"social","name":"twitter","url":"https://www.twitter.com/"},{"cat":"social","name":"x-com","url":"https://x.com/"}]') \
  /tmp/twitter_xcom_pair.json 2>&1 | grep -iE "set-cookie|cookie:" > /tmp/twitter_xcom_cookies.log
```

**Expected outcome:** identify the specific cookie(s). Fix path is likely **per-tab cookie partitioning** (Chrome's Storage Partitioning model — third-party cookies are partitioned by the top-level frame). For x-com: a fresh tab visiting x.com doesn't have first-party cookies from a prior twitter.com tab, even though the canonical site is the same. The fix is one of:
1. Per-`Page`/per-tab cookie jar instead of process-wide (largest change; closest to real Chrome)
2. eTLD+1 collision handling for x.com↔twitter.com specifically (smaller, ad-hoc)
3. Bot-token cookie identification + filtered re-send (narrowest, fragile)

**Effort:** 1-2 days investigation + 3-5 days implementation depending on approach chosen.

**Scope:** public engine.

**Sites in scope:** x-com (1).

---

### 1.3 — R-DUO-WORKER (1 site, multi-day investigation)

**What:** Build a standalone in-VM oracle for reCAPTCHA's Worker (`recaptcha/enterprise/webworker.js`). Spawn it in BO's worker context with instrumented `postMessage` / `Worker` surfaces; diff vs v150's behaviour. duolingo currently sits at L3 13.5KB — 1.7KB shy of the 15KB strict threshold.

**Why:** Fix 8 (MessageChannel) targeted this cluster but didn't crack it. The Worker's view of `navigator`, `Worker.prototype.postMessage`, structured-clone surface is the suspect.

**Specifics from prior research (`02_GAP_ANALYSIS.md §duolingo` for full lineage):**

- Recaptcha enterprise loads three scripts in order:
  - `https://www.recaptcha.net/recaptcha/enterprise.js?render=6LcLOdsjAAAAAFfwGusLLnnn492SOGhsCh-uEAvI`
  - `https://www.gstatic.com/recaptcha/releases/Br0hYqpfWeFzYCAXLD4UuCIV/recaptcha__en.js`
  - `https://www.recaptcha.net/recaptcha/enterprise/webworker.js` ← **the Worker that needs to succeed**
- duolingo SPA hydrates only after `grecaptcha.execute()` resolves with a token. In BO, this never fires.
- The 1.7 KB gap (13.5KB → 15KB) means duolingo's loose response is close enough that even a small forced hydration of the lessons-list mount could push past 15KB. So even a partial Worker-context fix may flip the site.
- Fix 8 was the MessageChannel implementation, verified by 3 unit tests (`message_channel_paired_routing`, `_queue_then_start`, `_close_detaches`) — those pass; the duolingo-recaptcha flow specifically doesn't use the new MessageChannel path the way we hoped.

**Files:**
- `crates/js_runtime/src/extensions/worker_ext.rs` — Worker implementation entry point
- `crates/js_runtime/src/js/dom_bootstrap.js` (search for `MessageChannel`, `Worker` installs)
- `docs/releases/v0.1.0-parity/02_GAP_ANALYSIS.md §duolingo` — original hypothesis
- Recaptcha JS to capture: `https://www.recaptcha.net/recaptcha/enterprise/webworker.js`

**Commands:**
```bash
# Single-site sweep with worker-extension trace
RUST_LOG=js_runtime::extensions::worker_ext=trace,info \
  target/release/examples/sweep_metrics chrome_148_macos \
  <(echo '[{"cat":"misc","name":"duolingo","url":"https://www.duolingo.com/"}]') \
  /tmp/duo.json 2>&1 | tee /tmp/duo.log
grep -E "worker|Worker|grecaptcha|recaptcha" /tmp/duo.log
```

**Expected outcome:** Identify the Worker-context API mismatch. Fix probably one of: `navigator` shape inside Worker, `Worker.prototype.postMessage` shape, missing `ImageBitmap` / `OffscreenCanvas` in Worker context, structured-clone gap.

**Effort:** 1 week investigation + variable fix.

**Scope:** public engine.

**Sites in scope:** duolingo (1).

---

### 1.4 — R-SPA-BOOKING-FETCH-CHAIN (1 site, 3-5 days)

**What:** Capture `fetches.json` for booking.com from BO + Camoufox v150 + Patchright. Diff to identify the missing `/api/...` fetch chain in BO. booking body 8473b deterministic across all 4 BO profiles, no hydration. v150 + Patchright both reach 465-513KB.

**Why it's the right cluster:** booking is one of v150's 8-site gains over v135 (full picture: §1.1 empirical evidence). v135 was at 8403b (~same as BO); v150 = PASS 513k. Whatever v150 added to handle booking's React-SPA initial-hydration chain is the gap to close. Patchright also passes (Chromium can do it too), so it's not Firefox-only — likely a fetch-dispatch event-loop bug in BO's nav loop.

**Files:**
- `crates/browser/src/page.rs` (look for the navigation-loop fetch dispatch)
- `docs/releases/v0.1.0-parity/04_TOOLING_SPEC.md` — `fetches.json` capture spec
- `docs/releases/v0.1.0-parity/02_GAP_ANALYSIS.md §booking` — original hypothesis (SPA bootstrap fetch chain fails)
- `docs/releases/v0.1.0-parity/05_SPA_HYDRATION_CLUSTER.md` — the cluster spec

**Commands:**
```bash
# BO capture
RUST_LOG=net=debug target/release/examples/sweep_metrics chrome_148_macos \
  <(echo '[{"cat":"travel","name":"booking","url":"https://www.booking.com/"}]') \
  /tmp/booking_bo.json 2>&1 > /tmp/booking_bo.log
# Extract fetch URLs in order
grep "sending H[12] request" /tmp/booking_bo.log | sed 's/.*to //' | awk '{print $1}'

# v150 capture — use Camoufox via playwright instrumentation
/tmp/bo-venv/bin/python <<'PY'
import asyncio, json
from camoufox.async_api import AsyncCamoufox
async def main():
    async with AsyncCamoufox(headless=True) as b:
        p = await b.new_page()
        fetches = []
        p.on("request", lambda r: fetches.append({"url": r.url, "method": r.method}))
        await p.goto("https://www.booking.com/", wait_until="networkidle", timeout=45000)
        json.dump(fetches, open("/tmp/booking_v150.json","w"), indent=1)
asyncio.run(main())
PY

# Diff
diff <(jq -r '.[].url' /tmp/booking_v150.json | sort -u) <(grep "sending" /tmp/booking_bo.log | awk '{print $5}' | sort -u)
```

**Expected outcome:** identify the missing fetch chain (likely some `/api/...` endpoint behind a JS-emitted XHR). Fix probably an event-loop / hydration-completion bug.

**Effort:** 3-5 days.

**Scope:** public engine.

**Sites in scope:** booking (1).

---

### 1.5 — R-AKAMAI-SECCPT-FLAKE (1 site, 2-3 days)

**What:** Git-bisect from current HEAD back to `b623d5d` (the Inc-7 fix that flipped homedepot chrome via persistent sec-cpt BMP-suppression, per `memory/state_2026_05_16_phase5_datadome.md`). Identify the regressing commit.

**Why it's the right cluster:** homedepot is **Stratum B** — the only site Patchright passes that neither BO nor Camoufox v150 do (Patchright = `PASS 1246k`; BO + v150 both `Akamai-CHL`). This is a Chromium-vs-Firefox split: Akamai's sec-cpt has a Firefox-rejecting heuristic. Prior session (Phase 5) had a working chrome solution that regressed sometime after `b623d5d`. Finding the regressing commit is mechanical work with high success probability — and gives us +1 site without inventing anything new.

**Files:**
- `crates/canvas/src/` — the BMP/canvas suppression code (search for `sec-cpt` or `BMP`)
- `crates/browser/src/page.rs` — challenge handling
- `docs/releases/v0.1.0-parity/26_AKAMAI_BMP_DEEP.md` — Akamai BMP technical reference
- `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/state_2026_05_16_phase5_datadome.md` — Inc-7 b623d5d background

**Commands:**
```bash
# Single-site homedepot test
target/release/examples/sweep_metrics chrome_148_macos \
  <(echo '[{"cat":"stores","name":"homedepot","url":"https://www.homedepot.com/"}]') \
  /tmp/homedepot.json

# Bisect script
git bisect start
git bisect bad HEAD
git bisect good b623d5d
git bisect run bash -c "cargo build --release -p browser --example sweep_metrics 2>&1 >/dev/null && \
  target/release/examples/sweep_metrics chrome_148_macos /tmp/homedepot_single.json /tmp/out.json && \
  jq -e '.results[]|select(.name==\"homedepot\")|select(.tag==\"L3-RENDERED\" and .len>=15000)' /tmp/out.json"
```

**Expected outcome:** identify regressing commit, revert or repair the specific lineage. homedepot was passing on iphone too — see if both profiles can recover.

**Effort:** 2-3 days.

**Scope:** public engine.

**Sites in scope:** homedepot (1).

---

### 1.6 — R-SPA-DOUYIN-SIG (1 site, 1-2 weeks)

**What:** Reverse-engineer douyin's `__ac_signature` JS that BO's V8 computes wrong. douyin returns *exactly* 6327 bytes across all 4 BO profiles (deterministic detection — server-side response based on signature check). v135 + v150 pass at 1MB. Patchright fails 8601b — **Firefox-only solve** (this is the key observation).

**Why it's the right cluster:** douyin's Firefox-only solve is unusual — it tells us douyin's antibot has a Firefox-vs-Chromium asymmetry. Either douyin specifically allows Firefox traffic to bypass the signature requirement, OR Firefox's `crypto.getRandomValues` / `AudioContext` / `Math.random` distribution somehow produces a different output that satisfies the verifier. BO mimics Chrome by design, so this is the hardest of the SPA cluster sites. Lower priority than booking/duolingo unless we want to explicitly target the TikTok ecosystem.

**Specifics:**

- The deterministic 6327-byte response means douyin sees the same input from all 4 BO profiles. Camoufox (Firefox base) gets 1MB, Patchright (Chromium base) fails. So either douyin specifically blocks Chromium-class fingerprints OR there's a JS-side computation that runs ok in Firefox but not in BO's V8 (despite BO mimicking Chrome).
- Strings to grep for in the response: `__ac_signature`, `ttwid`, `mssdk_`, `msToken` (these are the documented TikTok/Douyin antibot tokens).
- The signature is typically a function of: User-Agent string, screen dimensions, timezone, AudioContext fingerprint, mouse/keyboard event sequence, performance.now() drift.

**Files:**
- `crates/js_runtime/src/extensions/crypto_ext.rs` (if exists; else search for `crypto`)
- `crates/js_runtime/src/extensions/audio_ext.rs` (AudioContext for signature inputs)
- `docs/releases/v0.1.0-parity/02_GAP_ANALYSIS.md §douyin`

**Commands:**
```bash
# Capture the 6327-byte response for analysis
curl -sS -A "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36" \
  https://www.douyin.com/ > /tmp/douyin_response.html
grep -oE "__ac_signature|ttwid|mssdk_" /tmp/douyin_response.html | sort -u
# Then instrument BO with JS trace + run douyin in isolation
```

**Expected outcome:** identify which AudioContext / Crypto / Math.random / Performance.now call the signature reads where BO's value diverges from real Chrome. Fix is likely a per-profile-deterministic-yet-real-looking value.

**Effort:** 1-2 weeks (signature reverse-engineering is open-ended).

**Scope:** public engine.

**Sites in scope:** douyin (1).

---

### 1.7 — R-AWSWAF-OFFLINE-PROBE (0 sites, 1 week — enabler)

**What:** Capture AWS WAF challenge.js once; spin it up in BO's V8 with stubbed `navigator` / `window`; instrument every code path leading to `getToken()`. Provides a reproducible fingerprint→decision oracle without going through the live IP each iteration.

**Why:** Enabler for R-FP-AUDIT. Faster iteration than live amazon-de sweeps.

**Specifics from session memory (`02_GAP_ANALYSIS.md §5-8 AWS WAF cluster`):**

- The 2011-byte AWS WAF stub body is captured verbatim in `02_GAP_ANALYSIS.md`. It contains:
  ```js
  window.awsWafCookieDomainList = [];
  window.gokuProps = {
    "key": "AQIDAHj...",
    "iv": "A6wUCgAhZgAB...",
    "context": "glDLQwA7fqdXeYzd6QoT..."
  };
  <script src="https://1c5c1ecf7303.d474e66d.us-west-2.token.awswaf.com/.../challenge.js"></script>
  ```
- The challenge.js URL pattern is `https://<id>.<random>.<region>.token.awswaf.com/.../challenge.js`. Region varies by site (us-west-2, eu-west-1, ap-northeast-1 …).
- challenge.js fingerprints the browser → computes a WebAssembly proof-of-work token → POSTs to `<host>.token.awswaf.com/.../verify` → response sets `aws-waf-token` cookie. Then `location.reload(true)` re-fetches WITH the cookie → AWS WAF lets it through.
- BO observes the `[vendor-detect] aws-waf` marker fire at `page.rs:1050` AND the `/report` telemetry endpoint POSTs. But **`getToken()` is silently not called** — challenge.js detected something fingerprint-wise and bailed.

**Files:**
- `crates/js_runtime/src/lib.rs` — V8 entry point for a one-off JS execution
- `crates/browser/src/page.rs:1050` — the `[vendor-detect] aws-waf` marker (use as a hook for the oracle)
- `docs/releases/v0.1.0-parity/06_AWS_WAF_SOLVER.md` + `28_AWS_WAF_EXTENDED.md`

**Commands:**
```bash
# Capture a fresh challenge.js
curl -sSL -A "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36" \
  https://www.amazon.de/ > /tmp/amzn_de.html
# Extract the challenge.js URL + download
grep -oE 'https://[^"]+token\.awswaf\.com[^"]+challenge\.js' /tmp/amzn_de.html
```

**Expected outcome:** a unit-test harness that takes a `navigator`/`window` stub and reports whether AWS WAF would issue a token. Then R-FP-AUDIT iterates against this oracle locally.

**Effort:** 1 week.

**Scope:** public engine (the harness is engine-side tooling).

**Sites in scope:** indirect — enables 7 AWS WAF sites via R-FP-AUDIT.

---

### 1.8 — R-BESTBUY-AKAMAI (1 site, 2 days investigation)

**What:** Drive a manual Playwright run for bestbuy. If the SPA hydrates after a click/scroll → behavioural signal needed. If not → unknown trust signal. bestbuy fails on ALL engines including v150 + Patchright, but the body is the same 7KB SPA shell, not a CHL interstitial.

**Why it's the right cluster:** bestbuy is currently filed in **Stratum C** (universal hard frontier) — every engine tested fails it. But it's worth a 2-day investigation because the 7KB body is a *SPA shell*, not a *vendor challenge* — meaning it's potentially engine-addressable (a hydration / event-loop fix) rather than vendor-solver. Outcome of this task is either: (a) flip Stratum C → Stratum A with a clear engine-side fix path, OR (b) confirm Stratum C and move bestbuy to the v0.3.0+ research bucket.

**Files:**
- `docs/releases/v0.1.0-parity/26_AKAMAI_BMP_DEEP.md`
- `crates/browser/src/page.rs` (event-loop, interaction simulation)

**Commands:**
```bash
/tmp/bo-venv/bin/python <<'PY'
import asyncio
from playwright.async_api import async_playwright
async def main():
    async with async_playwright() as p:
        b = await p.chromium.launch(headless=False)
        page = await b.new_page()
        await page.goto("https://www.bestbuy.com/", wait_until="domcontentloaded")
        await page.wait_for_timeout(3000)
        len_before = len(await page.content())
        await page.mouse.click(400, 300)
        await page.wait_for_timeout(3000)
        len_after = len(await page.content())
        print(f"before click: {len_before}, after click: {len_after}")
        await b.close()
asyncio.run(main())
PY
```

**Expected outcome:** classify bestbuy as either (a) behavioural-signal-needed (engine-addressable via humanizer fixes, low effort), (b) JS-API-gap (engine-addressable via 17_WEB_API_PARITY_MATRIX additions, medium effort), or (c) cross-engine impossible (move to Stratum C frontier).

**Effort:** 2 days investigation, fix unknown.

**Scope:** public engine.

**Sites in scope:** bestbuy (1).

---

### 1.9 — R-CORPUS-DIAGNOSTIC-FLAG (corpus cleanup, ~1 day)

**What:** Add `diagnostic: true` flag to specific corpus sites (currently only `areyouheadless` qualifies). Update `site!` macro in `crates/browser/tests/holistic_sweep.rs`, `/tmp/corpus.json` generator, `sweep_metrics` summary, `bench_corpus_v2.py` summary, and the aggregator to compute both `raw_pass_rate` (/126) and `production_pass_rate` (/125).

**Why it's the right cluster:** Cosmetic but high-value for the v0.2.0 announcement. `areyouheadless` (and any future probes added) drag every engine's score down equally; reporting both raw and production rates lets the project make accurate "% of real sites we can browse" claims without methodology games. The prior-session doc-only convention (`VERIFICATION.md §6e`) is fine for one report; the structural flag is what makes every downstream report honest.

**Files:**
- `crates/browser/tests/holistic_sweep.rs` — search for `site!` macro definition + `areyouheadless` entry (line ~615)
- `crates/browser/examples/sweep_metrics.rs` — summary code
- `benchmarks/bench_corpus_v2.py` — summary code
- `/tmp/aggregate_fix12_gate.py` — aggregator (staged from prior session)

**Commands:**
```bash
# After implementing, regenerate corpus + verify the new flag appears
grep -n "diagnostic" /tmp/corpus.json    # expect exactly 1 hit (areyouheadless)
# Verify summary has both pass_pct and production_pass_pct
target/release/examples/sweep_metrics chrome_148_macos /tmp/corpus.json /tmp/out.json
jq '.summary | {pass, n, pass_pct, production_pass, production_n, production_pass_pct}' /tmp/out.json
```

**Expected outcome:** every numeric report can be both methodology-strict (/126) and audience-friendly (/125). Production pass-rate for BO routed median becomes 107/125 = 85.6%.

**Effort:** ~1 day.

**Scope:** public engine (testing harness).

**Sites in scope:** metric quality (no site flips).

---

### 1.10 — R-WBAAS-WILDBERRIES (1 site, multi-day research)

**What:** Investigate Wildberries' own `wbaas` antibot challenge (`/__wbaas/challenges/antibot/__static/v1/index-DQJ0L4Mq.js`, site-key `7400bd5df8b843b28254659f10915f31`). Every engine fails it (BO, Camoufox v135/v150, Patchright). Custom antibot, not a known 3rd-party WAF.

**Why it's the right cluster:** wildberries was originally suspected (and filed under) as either geo-blocked or universal-SPA-shell. The prior session's R-CORPUS-WILDBERRIES task disproved both — the datacenter IP receives an HTTP 498 with a structured antibot challenge (`Почти готово…` "Almost ready"). The challenge JS is loaded but every engine fails the same way → likely a real engine-addressable antibot, not corpus-quality issue. Low priority (only 1 site, cross-engine miss, not in v150's gain set), but should be classified properly so it stops appearing as a "mystery failure".

**Files:**
- The captured challenge response: `/tmp/wildberries_check2.html` (1447 bytes from prior session; may be wiped — recapture with the curl below)
- Pull `/__wbaas/challenges/antibot/__static/v1/index-DQJ0L4Mq.js` for deobfuscation

**Commands:**
```bash
# Reproduce the challenge response
curl -sS -A "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36" \
  --max-time 30 "https://www.wildberries.ru/" > /tmp/wbaas_response.html
# Extract challenge JS URL + site-key
grep -oE 'index-[A-Za-z0-9]+\.js|data-site-key="[^"]+"' /tmp/wbaas_response.html
# Download the challenge JS for deobfuscation
curl -sS -A "Mozilla/5.0 (...)" \
  "https://www.wildberries.ru/__wbaas/challenges/antibot/__static/v1/index-DQJ0L4Mq.js" > /tmp/wbaas.js
wc -c /tmp/wbaas.js
```

**Expected outcome:** classify as engine-addressable vs vendor_solvers vs research.

**Effort:** multi-day.

**Scope:** likely `vendor_solvers` per `CLAUDE.md` (custom site-specific antibot).

**Sites in scope:** wildberries (1).

---

### 1.11 — R-KASADA-FRONTIER (3 sites, months, vendor_solvers)

**What:** canadagoose / hyatt / realtor. Every engine fails — Camoufox v150 hardware spoofing didn't move it. Per prior research (`memory/state_2026_05_16_phase0_rebaseline.md`): realm/sentinel/identity line CLOSED as not-the-bug; residual is "holistic ML tail, no single lever". Open-ended research.

**Specifics from session memory (lineage of prior dead-ends — do NOT re-investigate these):**

- The realm/sentinel/identity hunt is CLOSED (`state_2026_05_16_phase0_rebaseline.md` §Phase 2 Outcome A). All 4 global paths in BO's Kasada-relevant V8 are byte-identical to a Chrome invariant. The Kasada `everTaggedId` was -1 in all measurements — sentinel chain matches Chrome.
- The K2-DIFF investigation (`state_2026_05_17_unblock_execution.md`) was the scoped next step: capture our `/tl` POST body + field-diff vs real Chrome. That work is in-flight in the prior research; the captures are at `/tmp/k2_diff/` if still present from prior sessions.
- The conclusion from `state_2026_05_16_phase0_rebaseline.md` was the **Kasada residual = holistic ML tail**: no single fingerprint surface lever moves canadagoose/hyatt/realtor; their decision is a classifier over many low-signal features. Camoufox v150's hardware-spoofing adds (which moved AWS WAF) didn't move Kasada — confirms ML-classifier nature.
- Patchright on hyatt gets 13228b (loose L3, sub-15KB) — partial progress, suggests hyatt's Kasada threshold is the lowest of the three. canadagoose + realtor are full Kasada-CHL across every engine.

**Why it's the right (or rather: the wrong-for-this-release) cluster:** Kasada was the prior multi-week ceiling-buster effort. Two findings: (1) realm/sentinel/identity closures eliminated single-lever fixes, (2) Camoufox v150's hardware-spoofing didn't move Kasada either — confirms the residual is an ML classifier over many low-signal features. **No single PR will move these three sites.** Real progress requires either a vendor_solvers-side full Kasada interactive token computation (open-ended research) or a cross-engine fingerprint corpus + classifier training (research project scale). Out of scope for v0.2.0; tracked as the visible long-term ceiling.

**Commands (diagnostic only — for the next person picking up the prior research):**
```bash
# Reproduce the per-engine Kasada-CHL signal
target/release/examples/sweep_metrics chrome_148_macos \
  <(echo '[{"cat":"chl-known","name":"canadagoose","url":"https://www.canadagoose.com/"},{"cat":"chl-known","name":"hyatt","url":"https://www.hyatt.com/"},{"cat":"realestate","name":"realtor","url":"https://www.realtor.com/"}]') \
  /tmp/kasada3.json
# Check the prior K2-DIFF capture artifacts (if /tmp/k2_diff/ survives)
ls /tmp/k2_diff/ 2>&1
```

**Expected outcome:** none in the v0.2.0 window. Goal is to keep the prior research lineage intact so a future Kasada-focused session can pick up where Phase 2 / K2-DIFF left off.

**Effort:** months, open-ended research. Belongs to `vendor_solvers` per `CLAUDE.md`.

**Scope:** `vendor_solvers` per `CLAUDE.md`. **Do NOT reintroduce vendor bypass code into public crates.**

**Files:** 
- `docs/releases/v0.1.0-parity/08_KASADA_FRONTIER.md`
- `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/state_2026_05_16_phase0_rebaseline.md` (Phase 2 closure)
- `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/state_2026_05_17_unblock_execution.md` (K2-DIFF scoping)
- `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/state_2026_05_15_session_synthesis.md` (Kasada ceiling synthesis)

**Sites in scope:** canadagoose, hyatt, realtor (3).

---

### 1.12 — R-DATADOME-DAILY-KEY (1 site, unknown, vendor_solvers)

**What:** etsy WASM-iframe daily-key rotation. v135 had 7913b loose-L3 (partial completion), v150 + BO both stuck at full DataDome-CHL.

**Why it's the right (or wrong-for-this-release) cluster:** DataDome's WASM-iframe is the daily-key endgame for this vendor. Pre-strip (commit `aecdf19`), BO had a working v135-class path. The WASM solver itself is `vendor_solvers` scope. BUT three engine-side primitives (CSP relaxation on challenge docs, iframe materialization, datadome-cookie-write detection) are public-engine scope and should be restored — they were measured to take BO from full block → loose 7-8KB (the v135 partial-pass behaviour). Without them, no DataDome path can succeed regardless of vendor_solvers work.

**Commands (diagnostic):**
```bash
# Reproduce the etsy DataDome-CHL signal
target/release/examples/sweep_metrics chrome_148_macos \
  <(echo '[{"cat":"stores","name":"etsy","url":"https://www.etsy.com/"}]') \
  /tmp/etsy.json
# Capture the dd-script.js + cross-origin iframe response
curl -sS -A "Mozilla/5.0 (...)" https://www.etsy.com/ > /tmp/etsy_initial.html
grep -oE 'https://[^"]+captcha-delivery\.com[^"]*\.(js|html)' /tmp/etsy_initial.html
```

**Expected outcome (public-engine portion):** restore the 3 primitives so etsy moves from full block → loose 7-8KB. Then `vendor_solvers` can add the WASM solver in a separate private PR for the strict pass.

**Effort:** 1 week for the 3 primitives (public engine); WASM solver effort unknown and out of public-engine scope.

**Specifics from session memory:**

- DataDome interstitial loads `dd-script.js` from `captcha-delivery.com` + a cross-origin iframe to `geo.captcha-delivery.com/captcha/?...`. The iframe runs a WASM-based challenge and POSTs a result that yields a `datadome=` cookie.
- Pre-strip (before commit `aecdf19`), BO had a path through the DataDome WASM-iframe that worked daily — see `memory/state_2026_05_16_phase5_datadome.md`. The key was rotating daily; the path broke after the strip.
- Three engine-side primitives needed to restore the v135-era partial pass (these belong in the public engine, not `vendor_solvers`):
  1. Relax CSP on the challenge document (DataDome's interstitial blocks its own iframe under strict CSP — real Chrome's interstitial handling permits this)
  2. Materialize the cross-origin iframe contents (BO has `rematerialize_iframes` at `page.rs:1981` but it's gated by `started_as_dd_challenge` which depends on the removed handler)
  3. Recognize the `datadome=` cookie write as a solve signal (would trigger a re-fetch)
- The **actual solver** (WASM key computation + POST) is `vendor_solvers` scope. The 3 primitives above are public-engine scope.

**Scope:** mixed — 3 primitives public engine; WASM solver = `vendor_solvers` per `CLAUDE.md`.

**Files:** 
- `docs/releases/v0.1.0-parity/07_DATADOME_PRIMITIVES.md`
- `crates/browser/src/page.rs:1981` (`rematerialize_iframes`)
- `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/state_2026_05_16_phase5_datadome.md`

**Sites in scope:** etsy (1).

---

## 2. Quick-reference reproduction recipe

### 2.1 — Environment

```bash
# Branch
git checkout fix/v0.1.0-fix4-canvas-parity
# Code HEAD = f625ab6; integration HEAD = 5e06a56 (docs only); plus this session's accept_ch/cookies env-var
# toggles in crates/net/src/lib.rs (uncommitted in prior session — see §3 below).

# Build
cargo build --release -p browser --example sweep_metrics --example classify_stdin

# Corpus
ls /tmp/corpus.json    # if missing, regenerate from holistic_sweep.rs per
                        # ~/projects/browser_oxide_internal/benchmarks/baselines/2026-05-24/README.md
```

### 2.2 — Competitor harness

```bash
# Once-only setup (~30 min wall, ~600 MB disk)
python3 -m venv /tmp/bo-venv
/tmp/bo-venv/bin/pip install playwright patchright 'camoufox[geoip]' playwright-stealth
PLAYWRIGHT_BROWSERS_PATH=~/.cache/ms-playwright /tmp/bo-venv/bin/playwright install chromium firefox
/tmp/bo-venv/bin/python -m camoufox fetch

# For Camoufox v150 (not yet pip-promoted):
mkdir -p /tmp/camoufox_v150 && cd /tmp/camoufox_v150
curl -sLO 'https://github.com/daijro/camoufox/releases/download/v150.0.2-beta.25/camoufox-150.0.2-alpha.26-lin.x86_64.zip'
unzip -q camoufox-150.0.2-alpha.26-lin.x86_64.zip -d v150
mv ~/.cache/camoufox ~/.cache/camoufox.v135.bak    # back up v135 (we already have it backed up from prior session)
mv v150 ~/.cache/camoufox
echo '{"version":"150.0.2","release":"beta.25"}' > ~/.cache/camoufox/version.json
# NOTE: Playwright 1.60 has a coreBundle bug on Firefox pageError. If you hit a Cannot read properties of undefined
# (reading 'url') in coreBundle.js, downgrade: /tmp/bo-venv/bin/pip install 'playwright<1.55' && \
#                                              /tmp/bo-venv/bin/playwright install firefox
```

### 2.3 — Single-profile sweep

```bash
target/release/examples/sweep_metrics chrome_148_macos /tmp/corpus.json /tmp/out.json
# ~30-50 min wall. JSON has full per-site results. Use the aggregator script to compute strict counts.
```

### 2.4 — Full Fix-12-style gate (3 runs × 4 profiles, 6-10h)

```bash
# Queue script staged at /tmp/run_fix12_gate.sh (also inlined in HANDOFF.md §5)
/tmp/run_fix12_gate.sh > /tmp/fix12_gate.log 2>&1
# Output to /tmp/fix12_gate/*.{json,log}
```

### 2.5 — Competitor sweep

```bash
# All 4 engines (Playwright/Patchright/Camoufox + Stealth) ~40 min wall total
/tmp/run_competitors.sh > /tmp/run_competitors.runlog 2>&1
# Output to /tmp/full_sweep_2026_05_27/*.{json,log}
```

### 2.6 — Aggregation

```bash
python3 /tmp/aggregate_fix12_gate.py
# Prints: per-(profile,run) strict, per-profile median (2-of-3), routed best-of-4 median, decision rule
```

### 2.7 — Per-site comparison (BO routed median vs Camoufox v150)

```bash
python3 /tmp/cmp_v150.py
# (recreate from VERIFICATION.md §5e-bis if /tmp is wiped)
```

### 2.8 — Diagnostic SharedSession A/B toggles (already landed in net/src/lib.rs)

```bash
# Full isolation (BOTH cookies + accept_ch)
BROWSER_OXIDE_NO_SHARED_SESSION=1 target/release/examples/sweep_metrics ...

# Just cookies isolated (accept_ch still shared)
BROWSER_OXIDE_NO_SHARED_COOKIES=1 target/release/examples/sweep_metrics ...

# Just accept_ch isolated (cookies still shared)
BROWSER_OXIDE_NO_SHARED_ACCEPT_CH=1 target/release/examples/sweep_metrics ...
```

---

## 3. Uncommitted work from prior session

**As of the prior session's end, these changes are on disk but NOT committed:**

1. **`docs/releases/v0.1.0-parity/VERIFICATION.md`** — the full 2026-05-27 verification ledger (new file)
2. **`docs/releases/v0.1.0-parity/FAILED_SITES_ANALYSIS.md`** — per-site root cause + action items (new file)
3. **`docs/releases/v0.1.0-parity/HANDOFF_v0.2.0_CLOSE_V150_GAP.md`** — this file (new file)
4. **`crates/net/src/lib.rs`** — env-var toggles `BROWSER_OXIDE_NO_SHARED_SESSION` / `_COOKIES` / `_ACCEPT_CH` in `HttpClient::shared` (~10 LoC). Used for SharedSession A/B diagnosis. Safe to commit — pure no-op when env vars absent.

**Recommended commit order** (separate PRs):

```bash
# PR 1: verification docs (no code)
git add docs/releases/v0.1.0-parity/{VERIFICATION,FAILED_SITES_ANALYSIS,HANDOFF_v0.2.0_CLOSE_V150_GAP}.md
git commit -m "docs(release): v0.1.0-parity verification + v0.2.0 handoff for closing Camoufox v150 gap"

# PR 2: diagnostic toggles
git add crates/net/src/lib.rs
git commit -m "feat(net): BROWSER_OXIDE_NO_SHARED_{SESSION,COOKIES,ACCEPT_CH} env-var toggles for SharedSession A/B"
```

---

## 4. Suggested execution order (by leverage × effort)

| # | Task | Sites | Effort | Scope | Prerequisites |
|---|---|--:|---|---|---|
| 1 | **R-FP-AUDIT-2026Q3** | up to 8 (7 AWS WAF + booking + douyin) | 2-3 weeks | public | (optional: R-AWSWAF-OFFLINE-PROBE first) |
| 2 | **R-CORPUS-DIAGNOSTIC-FLAG** | 0 (metric only) | 1 day | public | none |
| 3 | **R-AKAMAI-SECCPT-FLAKE** | 1 (homedepot) | 2-3 days | public | git history access |
| 4 | **R-SHAREDSESSION-X-COM-COOKIES** | 1 (x-com) | 1 week | public | env-var toggles (already landed) |
| 5 | **R-SPA-BOOKING-FETCH-CHAIN** | 1 (booking) | 3-5 days | public | competitor harness |
| 6 | **R-DUO-WORKER** | 1 (duolingo) | 1 week | public | competitor harness |
| 7 | **R-BESTBUY-AKAMAI** | 0-1 (bestbuy) | 2 days | public | competitor harness |
| 8 | **R-AWSWAF-OFFLINE-PROBE** | 0 (enabler) | 1 week | public | curl access to amazon-de |
| 9 | **R-SPA-DOUYIN-SIG** | 1 (douyin) | 1-2 weeks | public | competitor harness |
| 10 | **R-WBAAS-WILDBERRIES** | 1 (wildberries) | unknown | likely vendor_solvers | none |
| 11 | **R-DATADOME-DAILY-KEY** | 1 (etsy) | unknown | vendor_solvers | DataDome capture history |
| 12 | **R-KASADA-FRONTIER** | 3 | months | vendor_solvers | Kasada research lineage |

**If items 1-2 land: BO routed median 107 → estimated 114-116** → matches Camoufox v150's 115.
**If items 1-6 land: 116-119** → exceeds v150.

---

## 5. Validation gate for each shipped fix

For every R-* you ship, follow `14_TESTING_VALIDATION.md`:

- **L1 build**: `cargo build --workspace`
- **L2 clippy**: `cargo clippy --all-targets --workspace -- -D warnings`
- **L3 fmt**: `cargo fmt --all -- --check`
- **L4 workspace tests**: `cargo test --workspace --no-fail-fast -- --test-threads=1` (1508 pass / 1 known-fail pre-v0.1.0)
- **L4 per-fix tests**: add a `crates/browser/tests/chrome_compat.rs` test specifically validating your fix (see Fix 11 / `form_elements_collection` as a template)
- **L5 single-profile sweep**: `target/release/examples/sweep_metrics chrome_148_macos /tmp/corpus.json /tmp/out.json` — confirm your target site flipped, no other sites regressed
- **L5 routed gate** (optional, for stack-level claims): `/tmp/run_fix12_gate.sh` — full 3-run × 4-profile × 126-site gate, ~10 h

**Decision rule for v0.2.0 tag** (proposed; user should confirm before tagging):
- Routed best-of-4 median ≥ 115 (matches v150) → tag `v0.2.0-parity-rc1`
- Routed median 113-114 → tag `v0.2.0-parity` (parity with HANDOFF's static bar but not v150)
- Routed median < 113 → no tag, reprioritize

---

## 6. Methodology caveats (from prior session)

1. **R-V8-TERM** (`15_OPEN_QUESTIONS.md`): some sweeps hit the 50-min `timeout` wrapper due to V8 deadline-escape on Tealium `utag.v.js`. Mitigation = the cap; root cause = open. Affects firefox-profile sweeps consistently (last 10-13 tail sites truncated).
2. **Cap-truncated sweeps**: 10 of 12 prior gate sweeps hit the cap. Prior-session analysis (`VERIFICATION.md §3+§6c`) found this is NOT the bottleneck for the routed median (uncapped firefox added only 1-2 unique sites). Don't waste effort fighting the cap unless a specific profile-specific fix needs it.
3. **WAF state noise**: ±5 sites per `docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md`. Always use 3-run median for acceptance claims (L5).
4. **Camoufox v150 is beta**: not yet pip-promoted per maintainer ("1-2 weeks of testing"). Re-check `https://github.com/daijro/camoufox/releases` periodically — when v150 promotes to pip, the bar may rise further.
5. **Single-IP serial competitor sweeps**: `benchmarks/run_full_sweep.sh` notes "single-IP serial to avoid cross-engine WAF rate-limit contamination". Don't run BO + competitors concurrently on the same IP.

---

## 7. Risks and decision points

- **Cookie partitioning (R-SHAREDSESSION-X-COM-COOKIES)** is the deepest unknown. If the right fix is full per-tab partitioning, that's a multi-PR effort touching session lifecycle. Worth a scope discussion before committing.
- **Camoufox v150 source diff (R-FP-AUDIT-2026Q3)** depends on Camoufox's source readability + license compatibility. Camoufox is MPL-2.0 (Firefox-derived); reading for inspiration is fine but copying patches verbatim into BO (MIT/Apache) needs a license-clean re-implementation. Use it as a "what to look at" map, not a copy-paste source.
- **The `vendor_solvers` line** (`CLAUDE.md`): per-vendor solvers (AWS WAF token POST, DataDome WASM, Kasada K2, Akamai sensor_data) belong to the private repo. Engine-side **fingerprint-surface** work that happens to enable those vendors to issue tokens is public-engine and is the bulk of R-FP-AUDIT — don't conflate the two.
- **v0.1.0 tag still open**: prior-session user decision was "no tag, reprioritize". If after R-FP-AUDIT + R-SHAREDSESSION the routed median hits 115, revisit whether to tag v0.1.0-parity retroactively or only the new v0.2.0.

---

## 8. Contact + state

**Branch:** `fix/v0.1.0-fix4-canvas-parity` (local, unpushed)
**Code HEAD:** `f625ab6`
**Integration HEAD (docs-only commits since):** `5e06a56`
**Memory cache:** `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/MEMORY.md` — read the entries dated 2026-05-* for the lineage.
**Live test artifacts:** `/tmp/fix12_gate/*.{json,log}` (BO 12 gate sweeps + uncapped firefox + no-shared-session A/B + no-accept-ch targeted A/B), `/tmp/full_sweep_2026_05_27/*.{json,log}` (4 competitor sweeps + Camoufox v150).
**Internal baselines:** `~/projects/browser_oxide_internal/benchmarks/baselines/2026-05-24/` (cached 4-engine + 4-profile-BO single-run baseline; reference only — superseded by 2026-05-27 fresh sweep).

Good luck. The hard part is done — the gap is mapped, the diagnostic toolkit is in place, and the right work is identified. **Start with R-FP-AUDIT-2026Q3** — it's the single biggest lever and aligns most cleanly with project scope.
