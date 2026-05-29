# 00 — Frontier Workflows: the No-CDP Pass-Everything Investigation (index + thesis)

**Date:** 2026-05-29
**Branch context:** `fix/v0.1.0-fix4-canvas-parity`
**Scope:** the 10 "frontier" sites browser_oxide (and usually every other engine)
fails — and the hunt for the concrete ENGINE-ADDRESSABLE path to each, exploiting
BO's structural no-CDP advantage. This index ties the 9 docs together and states
the headline SOTA ceiling.

> **This workflow challenges the prior "frontier / out-of-scope / vendor_solvers-only"
> verdict.** It does not accept "out of scope" — it finds the concrete public-engine
> path where one plausibly exists, and is honest where a site truly needs an IP/geo
> input or a private `vendor_solvers` token solver.

---

## The thesis (the user's key insight — the spine of all 9 docs)

**browser_oxide has NO CDP.** It embeds V8 in-process via `deno_core`. There is no
Chrome DevTools Protocol on the navigate path, no `Runtime.enable`, no juggler/Marionette
pipe, no `--remote-debugging-port`, no isolated/utility world, no `cdc_`/`pptr:` surface,
no `navigator.webdriver=true`. **There is nothing in the CDP/automation-leak class for a
vendor to detect.**

Every COMPETITOR that fails these sites drives a real browser over a *control channel*:
- Playwright / Patchright / Puppeteer / nodriver / undetected-chromedriver = **CDP**.
- Camoufox v150 = the Firefox **Juggler** pipe (non-CDP but still an automation surface, J1–J4).

Modern Kasada / DataDome / Akamai aggressively fingerprint that channel's residue
(C1–C9 in `07`). **So BO occupies a detection region — "no control channel + a
programmable fingerprint" — that no CDP competitor can reach and that Camoufox reaches
only from a different, self-maintained Juggler position.**

**The load-bearing evidence (verified, captured, repeatable):** a real Chrome launched
*without* CDP (`nocdp.sh`) PASSES Kasada (canadagoose / hyatt / realtor) from THIS
datacenter IP, zero interaction. CDP "real Chrome" (Playwright) gets the empty-title 429
from the same IP. ⇒ The Kasada block is **NOT** IP reputation, **NOT** behaviour, **NOT**
a paid proxy — it is an **engine-fidelity gap**, and BO sits on the *passing* side of the
detection line (same class as the no-CDP real browser).

**Therefore the no-CDP advantage is the path to pass what no CDP engine can** — provided
BO closes the residual *fingerprint/runtime parity* and keeps `crates/protocol`'s CDP
server OFF the navigate path (the one caveat that can break the moat — see `07` §3.2).

---

## The 9 documents (read in this order)

| # | Doc | What it settles |
|---|---|---|
| **00** | `00_README.md` (this file) | Index + thesis + SOTA-ceiling headline. |
| **08** | `08_PASS_EVERYTHING_ROADMAP.md` | **The synthesis.** Per-site classification, ROI-ranked engine-path table, the no-CDP moat up front, the honest SOTA ceiling, the phased plan. **Start here for the plan.** |
| **06** | `06_NOCDP_ORACLE_METHOD.md` | **The enabler.** The capture-and-diff harness (no-CDP real browser + offline BO replay) that makes every "field-diff vs a passing payload" step trustworthy. Build this first. |
| **07** | `07_NOCDP_ADVANTAGE.md` | The competitive moat thesis: C1–C9 / J1–J4 leak catalog, the source-verified proof BO leaks none of them, and the must-not-break guard list (gate `crates/protocol`). |
| **01** | `01_KASADA_DEEP.md` | canadagoose / hyatt / realtor = ENGINE-ADDRESSABLE. Top new lever = populate the near-empty iframe child global (`dom_ext.rs:1217-1247`, best `bot1225`/`unjzomuy` candidate). K2-DIFF is the bounding experiment. Drain is mis-aimed for Kasada. |
| **02** | `02_DATADOME_DEEP.md` | etsy + tripadvisor (`rt:'i'` silent) = ENGINE-ADDRESSABLE, gated on the child-iframe isolated-cookie-jar bug (§3.3). yelp (`rt:'c'`) = human gate, no headless engine passes. Daily-key encoder = vendor_solvers only if runtime fidelity is insufficient. |
| **03** | `03_BESTBUY_AKAMAI.md` | The one site where the no-CDP thesis breaks: Patchright + Camoufox v150 + BO all fail identically (~7–8 KB) from this DC IP = IP/ASN trust gate. The "Patchright passes 1246k" claim is a debunked homedepot row. Low-confidence drain tail. |
| **04** | `04_GEO_SITES.md` | wildberries = MIXED (self-solve chain addressable + beats v150 THIN-39; trust ceiling IP-bound — US DC IPv6 is the root cause). ozon = IP-GEO-BOUND (RU residential IP required). |
| **05** | `05_DOUYIN_SIGNATURE.md` | douyin `__ac_signature`/acrawler reclassified UP from "Firefox-only / out-of-scope" to ENGINE-ADDRESSABLE: a working headless-Chromium signer proves the gate is automation-residue + consistency + builtin-integrity, where BO's no-CDP is a genuine edge. |

---

## The SOTA-ceiling headline (the honest answer)

Of the **10 frontier sites**, realistically in the **public engine** (no-CDP advantage +
the oracle, no `vendor_solvers`, this IP):

- **ENGINE-ADDRESSABLE (public) — the credible flip set: 5** —
  **hyatt, canadagoose, realtor** (Kasada; nocdp-proven engine gap; hyatt is the best
  first flip), **etsy, tripadvisor** (DataDome `rt:'i'`; gated on the cookie-jar bug).
  Plus **douyin** as a 6th, medium-confidence, pending the decisive R1 probe.
  *Confidence that the cluster is engine-addressable: high. Confidence that any single
  lever flips a given site: medium for hyatt/etsy, low for the rest.*
- **vendor_solvers (private) only — for a GUARANTEED flip: the token/PoW/daily-key tails** —
  Kasada `x-kpsdk-*` PoW, DataDome `ddCaptchaEncodedPayload` daily key, Akamai BMP
  `sensor_data` PoW, douyin acrawler signing. Needed only after the public oracle proves
  the residual is small/bit-exact. Short half-life; forbidden in public crates.
- **Genuinely IP/GEO-BOUND — out of public-engine reach: 3** —
  **bestbuy** (Akamai BMP needs a Favorable `_abck` a datacenter ASN cannot reach;
  residential/mobile IP required), **ozon** (RU residential IP required), **wildberries
  trust ceiling** (non-DC / foreign-residential, ideally RU, IP — though its self-solve
  *half* is engine-addressable and already beats v150's THIN-39).
- **Human gate — out of reach for ALL headless engines (not IP, not stealth): 1** —
  **yelp** (DataDome `rt:'c'` interactive captcha; Camoufox v150 also fails).

**Net realistic ceiling:** **~5–6 of the 10 are reachable in the public engine** with the
no-CDP advantage + the oracle (Kasada×3 + DataDome `rt:'i'`×2, plus douyin as a probe-gated
6th) — a **novel open-source SOTA result no CDP engine can match**. **3 are IP/geo-bound**
(name the exact need: bestbuy = residential/mobile, ozon = RU residential, wildberries
ceiling = non-DC). **1 is a human gate** (yelp). The remaining guaranteed-flip tails live in
private `vendor_solvers`.

**The single highest-EV move:** build the no-CDP oracle (`06`), then run **K2-DIFF on
Kasada starting from the child-realm population check** (`01` §3.2) — the most
evidence-backed engine gap on the entire frontier. See `08` for the phased plan.
