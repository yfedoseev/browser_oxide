# 07 — IP RECONCILIATION: "I open it in real Chrome" vs "the datacenter IP fails"

**Date:** 2026-05-29
**Author:** behavioral research agent (IP-vs-engine reconciliation)
**Status:** evidence-based reconciliation + per-site classification + the decisive same-IP protocol
**Scope:** Resolve, non-defensively and with evidence, the apparent contradiction between
the user's ground truth ("a human opens bestbuy / ozon / wildberries / yelp in a real
Chrome from their IP and gets in") and the prior project verdicts that read "datacenter IP
fails." Classify each site as **(a) BEHAVIORAL+ENGINE** (passes from *any* IP a real
browser does, including datacenter, once behavior+API+isTrusted are fixed) vs **(b)
IP/GEO/REPUTATION** (needs a residential or in-region IP regardless of engine). Specify the
single experiment that decides it per site, and make every final pass/fail claim
**same-IP-as-the-real-browser**.

> **This doc does not introduce a new gap.** It reconciles two true statements that *look*
> contradictory, names the hidden variable (which IP each statement was measured from),
> and turns "is it the IP or the engine?" from an argument into a per-site experiment with
> a pre-committed decision rule. It is the meta-doc that the §5 ranked fixes in `03`/`05`/
> `06` and the geo verdicts in frontier `04` all depend on for their honesty.

**Reading order / cited prior art (this doc reconciles, does not duplicate):**
- `docs/v0.1.0-frontier-workflows/06_NOCDP_ORACLE_METHOD.md` §1–2 — the no-CDP oracle and
  the captured no-CDP-Chrome PASS evidence. **The decisive instrument this doc invokes.**
- `docs/v0.1.0-frontier-workflows/07_NOCDP_ADVANTAGE.md` §2, §5 — why BO sits in the same
  detection class as a no-CDP real browser, and the per-cluster verdict table.
- `docs/v0.1.0-frontier-workflows/04_GEO_SITES.md` — the wildberries/ozon RU-geo evidence
  (the `data-req-ip` US-Comcast-Xen-VM proof; the Moscow Times / Habr external corpus).
- `docs/v0.1.0-behavioral-workflows/05_BESTBUY_AKAMAI_BEHAVIORAL.md` §3.2, §4 — the bestbuy
  ASN-floor caveat and Capture B (the bestbuy IP disambiguator).
- `docs/v0.1.0-behavioral-workflows/06_YELP_DATADOME_SLIDER.md` §1.1 — yelp: the real user
  is *silently cleared* (`rt:'i'`), almost never sees the slider; BO being shown `rt:'c'`
  means BO scored *worse than the real browser*, not that yelp is IP-banned.
- MEMORY: `state_2026_05_15_playwright_ab_decisive`, `proxy_not_the_problem` — the 2026-05-15
  CDP-confound that produced a *false* "IP-ban, stop engine work" verdict, falsified the
  next day by the no-CDP probe. **The exact mistake this doc exists to prevent recurring.**

---

## 0. TL;DR — the reconciliation in five lines

1. **Both statements are true; they were measured from different IPs.** The user double-clicks
   a link on a **residential** machine. The benchmark agent and the prior "fails" verdicts run
   from a **datacenter VM** (US Comcast/Xen-VM IPv6, captured at `04_GEO_SITES.md` §A.1). "Real
   Chrome passes" and "the configured client fails" are not in conflict once you hold the IP
   constant — they were never measured at the same IP.
2. **The only honest disambiguator is the no-CDP oracle:** run **real Chrome WITHOUT CDP**
   (`nocdp.sh`, zero automation surface) from the **exact IP the benchmark uses**, per site.
   If it PASSES → the gap is engine+behavioral (fix it). If it FAILS with a captured hard
   block → the IP is the gate (document the IP requirement). **Never use Playwright/Patchright/
   Camoufox as the reference** — they are CDP/Juggler and get a poisoned block (the 05-15 trap).
3. **The classification is not uniform — it splits cleanly per site:**
   - **bestbuy (Akamai BMP), yelp (DataDome):** lean **(a) BEHAVIORAL+ENGINE** — the real-Chrome
     pass is a *fingerprint+behavior* pass that BO can in principle reach from the same IP;
     bestbuy carries an honest *unresolved* ASN caveat that Capture B settles.
   - **wildberries (wbaas), ozon (DDoS-Guard):** lean **(b) IP/GEO** — the real-Chrome pass the
     user sees is from a **non-DC / RU-residential** IP; from a US DC IP even real no-CDP Chrome
     is challenged/refused. wildberries has a *partial* engine half (the self-solve drain); ozon
     is essentially all IP-geo.
4. **The decisive experiment already exists and is cheap.** `nocdp.sh` + `tl_capture.sh` are
   built and proven (they produced the canadagoose/hyatt/realtor PASS evidence). The missing
   inputs are (i) running them per-site from the benchmark IP, and (ii) for wildberries/ozon, a
   residential-vs-DC IP A/B. No live nav on the contended benchmark IP is needed (offline replay
   + out-of-band capture).
5. **The standing rule (MEMORY, restated):** *do not assert "IP-bound" without a captured hard
   block from a no-CDP real browser at the benchmark IP, and do not assert "engine-addressable"
   pass without a same-IP no-CDP PASS.* All final verdicts in this cluster must cite which IP
   they were measured from.

---

## 1. What a real browser does (and why "I opened it" is not the whole sentence)

When the user says "I open bestbuy/yelp/wildberries in real Chrome and it works," the implicit,
load-bearing, usually-unstated half of that sentence is **"…from my residential connection."**
A real browser from a residential IP does three things the prior failing measurements did not
all do *together*:

1. **Presents a residential / mobile ASN.** Anti-bot vendors maintain ASN-keyed reputation
   (`05 §3.2`, `04 §A.4`). A residential ASN gets a *trust prior* a datacenter ASN never does.
   This is the single biggest hidden variable between "I opened it" and "it failed."
2. **Presents a correct-enough browser fingerprint** (canvas/WebGL/audio/UA-CH/screen/tz) AND a
   non-CDP control surface. A real Chrome has both for free.
3. **Generates continuous human behavioral entropy** (mouse/scroll/key, `isTrusted=true`) that
   the vendor's behavioral sub-score reads as human (`05 §1.1`).

The prior project "fails" verdicts were frequently measured with **at most one** of these
(BO has #2's fingerprint partially and #3 partially; it lacked #1 — the residential ASN — and
some earlier measurements were *poisoned* by a CDP control channel, which is worse than no-CDP).
So "real Chrome passes" and "our client fails" can both be true while testing **different points
in (ASN × fingerprint × behavior × control-channel) space**. The reconciliation is: name which
axes differ per site, then collapse the ASN axis with a same-IP experiment.

**The critical asymmetry the user's directive correctly insists on:** holding the IP constant,
a *no-CDP* real browser and BO are in the **same detection class** (`07_NOCDP_ADVANTAGE.md` §2).
So for any site where no-CDP real Chrome passes *from the benchmark IP*, the gap is **not** the
IP — it is engine fidelity + behavior, and the user is right to refuse "IP-bound." The only sites
where "IP-bound" survives are those where even no-CDP real Chrome fails from that IP.

---

## 2. What BO does today (file:line) — and what the prior "fails" verdict actually measured

### 2.1 BO is in the no-CDP class (the user's asymmetry is real, code-verified)

`Page::navigate` runs V8 in-process via `deno_core`; there is no CDP endpoint bound on the
navigate path (`07_NOCDP_ADVANTAGE.md` §3.1, verified). The CDP server crate
(`crates/protocol/`) is only ever started by `tests/browser_comparison.rs` — never by
`page.rs`. So when BO fails a site that a *no-CDP* real browser passes from the same IP, the
failure cannot be "BO leaks CDP" — that class of explanation is closed by construction. This is
exactly why the user's "fix the engine, don't retreat to IP" framing is correct *for the no-CDP
class*.

### 2.2 BO does emit behavior — but with three known tells (so "behavior absent" is wrong, "behavior synthetic" is right)

`Page::navigate` injects `crates/browser/src/js/humanize.js` on every navigate
(`page.rs:1081-1097`). It pre-populates a curved Σ-Λ mouse history and runs a 4 s live cycle
(`humanize.js:357-452`, `:292-336`). So a prior "BO fails because it has no behavior" reading is
**false** — `bmak`'s `mact` is non-empty under BO (`05 §2.1`). But three tells remain
(`05 §2.2-2.5`), each a *behavioral-quality* gap, not an absence:
- **`isTrusted` is a detectable per-instance shadow** — `humanize.js:105,429,441` do
  `Object.defineProperty(event,'isTrusted',{value:true,configurable:true})`, a data descriptor on
  the instance, whereas a real event's `isTrusted` is an accessor on `Event.prototype`. The native
  path (`event_bootstrap.js` `Symbol.for('__bo_trusted__')`) exists but humanize does not use it.
- **Live-cycle mouse is linear** (`humanize.js:309-324` `_lerp`), straightness ≈ 1.0 — the #1
  mouse tell.
- **Zero touch / device-motion on mobile profiles** (`humanize.js:71` declares the buffer; no
  generator exists) → `tact===""` while `maxTouchPoints===5` — the one place BO is *worse than
  nothing* on its own iPhone/Pixel profiles.

### 2.3 The open-source engine ships ZERO solvers (so some "fails" are unconsumed-buffer, not IP)

`Page::default_solvers()` returns an empty `Arc<[]>` (`page.rs:979-980,502,738`); `navigate`
forwards it (`page.rs:1144`). So even a perfect `__akamai_events` behavioral buffer is **never
drained into a `sensor_data` POST** in the public engine — the consumer lives in the private
`vendor_solvers` crate, registered via `Page::with_solvers` (`page.rs:952-966`). This means a
bestbuy/homedepot "fail" in the public engine can be a *missing-consumer* fact, **independent of
the IP**. Conflating that with "IP-bound" is a category error the §3 classification avoids.

### 2.4 What the prior "datacenter IP fails" verdicts actually measured (the honest audit)

| Prior reading | What it actually was | Why it is not "IP-bound" by itself |
|---|---|---|
| "Playwright real Chrome gets Kasada 429 from this IP" (05-15) | a **CDP-poisoned** reference (the Juggler/CDP channel was detected) | falsified next day: **no-CDP** real Chrome PASSES same IP (`06 §2`). The IP was clean; the *protocol* was the block. |
| "bestbuy lands on a 7-8 KB shell for BO + Patchright + Camoufox from this IP" (`26 §4.3`) | a real *pattern* — but Patchright/Camoufox are CDP/Juggler, so two of three references are class-(i) poisoned | consistent with an ASN floor **or** with all three sharing a control-channel/fingerprint gap; **only no-CDP Chrome at this IP disambiguates** (`05 §4` Capture B). |
| "wildberries 498 from this IP" | a 498 served to a **visibly US Comcast/Xen-VM IPv6** (`04 §A.1` `data-req-ip`) | the IP *is* a real factor here (adaptive PoW difficulty is IP-scaled) — but it sets *difficulty*, not a hard ban; the engine self-solve is a separate, real half. |
| "ozon thin 156 B body" | the pre-JS **geo-refusal** body | here the IP **is** the gate (RU-geo); confirmed against external evidence (`04 §A.2,C`). |

**The pattern:** some prior "IP" verdicts were CDP-confounds (Kasada — now known clean), some
were ASN-*uncertain* (bestbuy — needs Capture B), one is IP-difficulty-*plus*-engine
(wildberries), and one is genuinely IP-geo (ozon). They were lumped under one "datacenter IP
fails" headline. This doc unlumps them.

---

## 3. The gap + the exact disambiguating experiment (per site)

The "gap" here is **methodological**: the project lacks a *same-IP* measurement for the four
sites, so it cannot honestly attribute failure to IP vs engine. The fix is one experiment,
already-built, run per site from the benchmark IP, with a pre-committed decision rule.

### 3.1 The decisive experiment — the no-CDP oracle at the benchmark IP

The exact instrument is `06_NOCDP_ORACLE_METHOD.md` §3.1 Tier-0, run **from the same IP/ASN the
benchmark uses** (not the user's residential connection):

```bash
# Ground-truth verdict, zero automation surface, observed out-of-band (undetectable):
#   ab_harness/nocdp.sh <slug> <url> 25
chrome --user-data-dir=<fresh> --no-sandbox --ozone-platform=x11 \
       --no-first-run --no-default-browser-check --window-size=1366,900 <URL>
# observe purely out-of-band: `xprop WM_NAME` (mirrors document.title) + `import -window root` screenshot
```

- **No `--remote-debugging-port`, no `--headless`, no automation flag** → BO's detection class.
- **Fresh `--user-data-dir`** → first-visit, exactly the user's "double-click a link" test.
- **`--no-sandbox` / `--ozone-platform=x11` are NOT JS-observable** (verified, `06 §3.1`) — they
  change the OS sandbox + render backend, not any `navigator`/`window` property.
- **FORBIDDEN as the reference:** Playwright, Patchright, Puppeteer, Selenium, Camoufox, or any
  `--remote-debugging-port` launch (`06 §3.1`). All are CDP/Juggler and poison the reference —
  this is the literal 05-15 mistake.

**The pre-committed decision rule (this is the whole point):**

| No-CDP real Chrome at the **benchmark IP** | Verdict | Action |
|---|---|---|
| **PASSES** (non-empty product `<title>` + content screenshot) | Gap is **ENGINE + BEHAVIORAL** | Fix the engine. Capture the real passing payload (`tl_capture.sh`), replay the same challenge through BO offline (`awswaf_probe.rs`-pattern), field-diff → named gap list (`06 §3-4`). **Do NOT say "IP-bound."** |
| **FAILS** with a **captured hard block** (Kasada empty-`<title>` 429 / hard-403 / geo-refusal body) | Gap is **IP/GEO/REPUTATION** | Document the IP requirement (residential / in-region). Re-test with a residential IP A/B. **Do NOT chase a phantom engine divergence.** |

For sites with an **IP-difficulty** component (wildberries), add the **residential-vs-DC IP A/B**
(`04 §B.4` step 3): no-CDP real Chrome from a foreign *residential* IP vs from the DC IP. This
quantifies how much of the gap is IP-trust (difficulty delta) vs engine (solve completion).

### 3.2 What this experiment costs (and why it does not touch the contended benchmark IP)

The verdict step (Tier-0) is a single headful Chrome launch observed out-of-band — it does **not**
require driving live navs from BO on the contended benchmark IP. The BO side is the **offline
replay** (`crates/browser/examples/awswaf_probe.rs` pattern: `Page::from_html_with_url` +
`run_until_idle`), which uses no network egress at all (`06 §3.3`). So the entire disambiguation
runs without competing for the single benchmark IP the live competitor harness holds. The only
network step is the passive `tl_capture.sh` (SSLKEYLOGFILE + tcpdump, changes nothing on the wire)
of one real passing session — a few seconds.

---

## 4. Per-site classification (which IP each "pass" was measured from)

This is the reconciliation table. Each row states the block class, what the user's "I opened it"
pass was *actually measured from*, the same-IP no-CDP prediction, and the resulting class.

| Site | Vendor | What the user's "it works" pass is | No-CDP real Chrome at **benchmark (DC) IP** | Class | Engine half? |
|---|---|---|---|---|---|
| **bestbuy** | Akamai BMP | residential ASN + real fingerprint + human behavior | **UNKNOWN — Capture B decides** (`05 §4`). Patchright+Camoufox+BO all hit the 7-8 KB shell, but those are CDP/Juggler; no-CDP Chrome at the DC IP is untested. | **(a) BEHAVIORAL+ENGINE, with an honest ASN caveat** | YES — FIX-1..4 behavioral; FIX-5 sensor encoder (private). Flips only if Capture B shows DC no-CDP hydrates. |
| **yelp** | DataDome | residential user **silently cleared** (`rt:'i'`), almost never sees the slider (`06 §1.1`, DataDome: <0.01% see the slider) | likely PASS / silent-clear from a clean IP; BO being shown `rt:'c'` = BO scored *worse than the real browser*, not an IP ban | **(a) BEHAVIORAL+ENGINE** (earn `rt:'i'` like etsy) — the slider itself, if forced, is `vendor_solvers` | YES — the `rt:'i'` trust-score path (shared with etsy) is public-engine + behavioral. Slider drag = visual puzzle (`vendor_solvers`). |
| **wildberries** | wbaas (in-house) | a **foreign *residential*** IP loads it WITHOUT VPN (`04 §A.2`, Moscow Times / Habr); the user is on such an IP | **partial / challenged** — a US DC IPv6 gets the hostile high-difficulty PoW branch (`04 §A.1,A.4`); not a hard ban, a throttle | **(b) IP/GEO (dominant) + (a) engine half** | YES (partial) — the self-solve live-nav drain (shared with AWS, `04 §B.3`); beats v150's THIN-39. PoW math at high difficulty = `vendor_solvers`. IP-trust ceiling = infra. |
| **ozon** | DDoS-Guard + RU geo | a **RU residential / in-RU** connection; "ozon won't load without a VPN" from abroad (`04 §A.2,C`) | **FAILS** — thin ~156 B pre-JS geo-refusal body from a foreign/DC IP | **(b) IP/GEO-BOUND** | NO meaningful engine half — the geo-gate fires before any engine surface is reachable. RU residential IP required. |

**Reading the table against the user's directive:**
- The user is **right** about bestbuy and yelp: these are NOT purely IP-bound — they are
  fingerprint+behavior passes a real (no-CDP) browser achieves, and BO is in that same class.
  The honest residual is "is there *also* an ASN floor on bestbuy?" → Capture B settles it; do
  not pre-assert IP-bound.
- The user's residential pass on **wildberries/ozon** is real but is **the residential-IP pass**,
  not a same-IP-as-benchmark pass. From the DC benchmark IP, wildberries hands a hostile PoW
  difficulty (IP-scaled) and ozon refuses pre-JS. These genuinely carry an IP requirement —
  flagged honestly and **separately** (the directive's explicit carve-out for "genuine geo-IP
  cases like a RU-only site"). wildberries still has a real engine half worth shipping (beats
  v150); ozon does not.

---

## 5. Ranked actions (effort + confidence) — collapse the ambiguity, then fix the right half

The ranking is ordered to **measure before asserting**: run the cheap, contention-free
disambiguators first so every downstream fix is aimed at the proven half.

| # | Action | What it resolves | Effort | Confidence | Public? |
|---|---|---|---|---|---|
| **1** | **bestbuy Capture B** — no-CDP real Chrome at the benchmark DC IP, zero-interaction. DC hydrates ⇒ engine/behavioral-addressable; only residential hydrates ⇒ ASN floor confirmed. | The bestbuy (a)-vs-(b) question — the single most contested site | 0.5 day (one headful launch + out-of-band observe) | **high** (the experiment is decisive) | public (capture tooling) |
| **2** | **yelp no-CDP probe at the benchmark IP** — confirm a clean IP is silently cleared (`rt:'i'`/Allow), and capture which trust band BO lands in (`rt:'i'` vs `rt:'c'`). | yelp = trust-score gap (engine+behavior) vs slider (vendor_solvers) | 0.5 day | **high** | public |
| **3** | **wildberries residential-vs-DC IP A/B** (`04 §B.4` step 3) — quantify the PoW-difficulty delta between a foreign-residential IP and the DC IP. | How much of wildberries is IP-trust vs the engine self-solve | 1 day (needs one residential egress for the A side) | medium (needs an IP we don't currently have) | public |
| **4** | **FIX-1 trusted native events** — route humanize dispatch through `Symbol.for('__bo_trusted__')`, delete the `defineProperty(...,'isTrusted',...)` shadow (`humanize.js:105,429,441`; `05 §3.1` FIX-1). | Removes the `isTrusted` descriptor tell for **every** behaviorally-scored vendor — the prerequisite that makes any later same-IP pass *honest* (a real-looking trust signal). | 0.5 day | **high** (closes the tell); low (flips a site alone) | public |
| **5** | **FIX-2 curved live-cycle mouse** — route `runCycle` through `op_behavior_mouse_trajectory` (`humanize.js:309-324`; `05 §3.1` FIX-2). | Drops live `mact` straightness 1.0→~0.4; raises bestbuy/yelp behavioral sub-score toward the real-browser pass. | 1-2 days | **high** (closes the tell) | public |
| **6** | **ozon geo-confirm + mark `diagnostic:true`** — capture the thin body, confirm pre-JS geo-refusal (`04 §C.4-C.5`), drop from the production denominator. Re-open only with a RU residential IP. | Stops ozon dragging the rate as a non-fixable gap; honest IP-geo classification. | 0.5 day | **high** | public |
| **7** | **wildberries live-nav drain** (shared with AWS §5.1) — guarantee the self-solve chain advances (`04 §B.3`, `page.rs:3661` 50 ms inter-script drain). | The engine half of wildberries; a clean win vs v150 THIN-39 even from a DC IP. | shared with AWS drain work | medium | public |
| **—** | **Residential/mobile egress IP** for bestbuy `_abck` ASN sub-score + wildberries trust ceiling + ozon RU-geo. | The IP half, *only where #1/#3 prove it binds.* | n/a | medium-high (where confirmed) | **out of engine scope (infra)** |

**Recommended sequence:** #1 + #2 (decide bestbuy/yelp class, contention-free) → #4 + #5 (land
the cross-vendor trusted/curved behavioral wins regardless of the verdict, since they are
profile-neutral public quality gains) → #3 (wildberries IP A/B) → #6 (ozon honest drop) → #7
(wildberries engine half). **Do not** assert "IP-bound" for bestbuy or yelp before #1/#2 return a
captured hard block at the benchmark IP — that is the exact 05-15 mistake.

---

## 6. The standing rule (so this never has to be re-litigated)

Every final pass/fail claim in this cluster MUST be annotated with the IP it was measured from,
and MUST satisfy one of:

1. **"ENGINE-ADDRESSABLE"** ⇒ backed by a **same-IP no-CDP real-Chrome PASS** (Tier-0 verdict,
   `06 §4` step 1) at the benchmark IP. Without it, "the engine can fix this" is an assertion.
2. **"IP/GEO-BOUND"** ⇒ backed by a **captured hard block** (empty-`<title>` 429 / hard-403 /
   pre-JS geo-refusal body) from a **no-CDP real Chrome at the benchmark IP** (MEMORY rule,
   `state_2026_05_15`). Without it, "it's the IP" is an assertion — and historically a wrong one
   (Kasada was a CDP-confound, not an IP ban).
3. **"UNRESOLVED"** ⇒ the same-IP no-CDP experiment has not been run. This is the honest state
   for bestbuy's ASN question today; it is resolved by action #1, not by argument.

Never compare BO to a **CDP/Juggler** reference (Playwright/Patchright/Camoufox) and call the
result an IP verdict — those are a *different, more-detected* detection class (`07_NOCDP_ADVANTAGE.md`
§1), and a block of *them* says nothing about whether the **IP** is clean for BO's class.

---

## 7. Direct answers to the reconciliation questions

**Q: How can the user open bestbuy/ozon/wildberries/yelp in real Chrome while the project says
the datacenter IP fails?**
Because the two were measured from **different IPs**. The user is on a **residential** connection;
the prior verdicts are from a **datacenter VM** (captured US Comcast/Xen-VM IPv6, `04 §A.1`).
Holding the IP constant removes the contradiction. For bestbuy/yelp the *no-CDP* class makes the
user's "fix the engine" framing correct (same detection class as the passing real browser); for
ozon (and partly wildberries) the residential IP itself is part of why the user's Chrome passes.

**Q: Is it the IP or the engine — per site?**
- **bestbuy:** UNRESOLVED → run Capture B. Leans engine+behavioral (the user's framing), with an
  honest, testable ASN-floor caveat. Do not pre-assert IP.
- **yelp:** ENGINE+BEHAVIORAL — the real user is silently cleared; BO being shown the slider means
  BO scored worse, not that the IP is banned. Earn `rt:'i'` like etsy.
- **wildberries:** BOTH — IP-trust sets PoW difficulty (dominant, the residential IP is why the
  user passes), but a real engine self-solve half exists and beats v150.
- **ozon:** IP/GEO-BOUND — RU residential IP effectively required; the geo-gate fires before any
  engine surface. No meaningful engine half.

**Q: What is the decisive experiment, and does it touch the contended benchmark IP?**
`nocdp.sh <slug> <url>` from the benchmark IP (out-of-band observed, zero automation surface),
with the §3.1 decision rule; for wildberries add the residential-vs-DC IP A/B. The verdict step
is a single headful launch; the BO side is **offline replay** (`awswaf_probe.rs` pattern, no
egress). So it does **not** compete for the live benchmark IP the competitor harness holds.

---

## 8. Sources

**Internal (this repo):**
- `docs/v0.1.0-frontier-workflows/06_NOCDP_ORACLE_METHOD.md` §1.1, §2, §3.1, §3.3, §4 (the
  oracle, the captured no-CDP PASS, the decision rule, the offline-replay BO side).
- `docs/v0.1.0-frontier-workflows/07_NOCDP_ADVANTAGE.md` §1-2, §3.1, §5 (BO = no-CDP detection
  class; per-cluster verdict; the moat caveat that `crates/protocol` stays off the navigate path).
- `docs/v0.1.0-frontier-workflows/04_GEO_SITES.md` §A.1 (`data-req-ip` US-Comcast/Xen-VM proof),
  §A.2 (Moscow Times / Habr: foreign-residential loads wildberries without VPN; ozon needs a VPN),
  §B.3-B.5 (wildberries engine half + IP ceiling), §C.1-C.5 (ozon geo-bound).
- `docs/v0.1.0-behavioral-workflows/05_BESTBUY_AKAMAI_BEHAVIORAL.md` §2.1-2.5 (BO behavior emitted
  + the three tells), §3.2 (the ASN caveat), §4 Capture B (the bestbuy IP disambiguator), §6.
- `docs/v0.1.0-behavioral-workflows/06_YELP_DATADOME_SLIDER.md` §1.1 (real user silently cleared,
  <0.01% see the slider; `rt:'c'` to BO = scored worse, not IP-banned).
- BO source (verified this session): `crates/browser/src/page.rs` — `default_solvers()` empty
  `:979-980,502,738`, `navigate` forwards it `:1144`, `with_solvers` `:952-966`, humanize inject
  `:1081-1097`; `crates/browser/src/js/humanize.js:105,429,441` (the `isTrusted` defineProperty
  shadow), `:309-324` (linear lerp live cycle), `:71` (touch buffer, no generator);
  `crates/browser/examples/awswaf_probe.rs` (the offline BO-replay pattern).
- MEMORY: `state_2026_05_15_playwright_ab_decisive` (the CDP-confound / false IP-ban verdict,
  falsified by the no-CDP probe), `proxy_not_the_problem`, `measurement_holistic_chl_fp_trap`.

**External (2024-2026):**
- DataDome CAPTCHA alternative — "<0.01% of users ever encounter the slider"
  (https://datadome.co/products/captcha-alternative/); DataDome Device Check / `rt` routing
  (Hyper Solutions slider doc, https://docs.hypersolutions.co/datadome/slider-captcha).
- Wildberries adaptive-PoW write-up (Habr, https://habr.com/ru/companies/wildberries/articles/1032556/);
  RU marketplaces throttle VPN/DC, foreign-residential loads without VPN
  (The Moscow Times 2026-04-15, https://www.themoscowtimes.com/2026/04/15/russian-websites-begin-blocking-vpn-users-as-internet-controls-tighten-a92511;
  rtvi.com, https://rtvi.com/news/wildberries-i-ozon-nachali-blokirovat-dostup-pokupatelyam-s-vpn/).
- IP allocation of the captured `data-req-ip` (IPQS AS33491 Comcast,
  https://www.ipqualityscore.com/asn-details/AS33491/comcast-cable-communications-llc).
- rebrowser-bot-detector / CDP-leak class (the reference class BO is NOT in,
  https://github.com/rebrowser/rebrowser-bot-detector).
