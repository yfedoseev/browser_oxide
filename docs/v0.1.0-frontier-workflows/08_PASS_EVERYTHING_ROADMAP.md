# 08 — The Pass-Everything Roadmap + Honest SOTA Ceiling

**Date:** 2026-05-29
**Branch context:** `fix/v0.1.0-fix4-canvas-parity`
**Author:** frontier research agent (synthesis)
**Inputs:** `01`–`07` of this workflow + the cited parity/vNext docs + memory + source
verification (2026-05-29).

This is the synthesis of the 7 frontier deep-dives into one actionable plan. It states
(1) the no-CDP MOAT up front, (2) the per-site classification, (3) the ROI-ranked
engine-path table, (4) the honest SOTA ceiling, (5) the phased plan.

---

## 1. The no-CDP MOAT — the sites BO can pass that the competition structurally cannot

**The thesis (verified, load-bearing):** browser_oxide has **no control channel**. It
embeds V8 in-process via `deno_core` — no CDP, no `Runtime.enable`, no juggler pipe, no
`--remote-debugging-port`, no isolated/utility world, no `cdc_`/`pptr:` surface,
`navigator.webdriver=false` on the prototype, no node `process` leak. **There is nothing
in the CDP/automation-leak class for a vendor to detect** (source-verified: `Page::navigate`
never touches `protocol::`; init scripts run in-process named `<anonymous>`;
`crates/js_runtime/src/runtime.rs:285-297`).

**Every competitor that fails the frontier leaks a control-channel artifact:**

| Engine | Channel | Frontier exposure |
|---|---|---|
| Playwright / Puppeteer | CDP (`Runtime.enable` leak C1) | HIGH — dies at the first probe |
| Patchright / rebrowser / nodriver / UC | CDP, partially patched | MEDIUM — patches one leak, opens another (C5 main-world mismatch) |
| **Camoufox v150** | Firefox **Juggler** pipe (J1–J4) | LOW–MEDIUM — non-CDP, but a self-maintained automation surface + a real Firefox event loop; the only competitor that beats BO on the frontier today |
| **browser_oxide** | **NONE** | **ZERO** — cannot fail the CDP/automation-transport class of check |

**The measured proof:** a no-CDP real Chrome (`nocdp.sh`) PASSES Kasada
(canadagoose / hyatt / realtor) from THIS datacenter IP, zero interaction — while CDP
"real Chrome" (Playwright) gets the empty-title 429 from the same IP. BO sits on the
*passing* side of that line (same detection class as the no-CDP real browser); every CDP
competitor sits on the failing side.

**⇒ There exists a region of the detection space — "no control channel + correct-enough
fingerprint" — that BO can occupy and no CDP-based engine can.** The moat is what makes
the frontier *worth* working: for Kasada, BO is one of only two client classes that can
plausibly pass (it + no-CDP real Chrome), and the only *programmable* one.

**The one caveat that breaks the moat (the must-not-break guard, `07` §3.2):**
`crates/protocol/` ships a full CDP WebSocket server (`Runtime.enable` at
`session.rs:226`). It is a **non-optional dep** of `crates/browser` (`Cargo.toml:36`,
verified) but is started **only** from `tests/browser_comparison.rs` — never on the
navigate path. **Recommend gating it behind an off-by-default `cdp-server` Cargo feature**
so the moat is mechanically enforced (matching the `deny.toml` discipline). Running a
frontier nav with the CDP server bound reintroduces the entire C1–C9 surface BO's moat is
the *absence* of.

**Where the moat does NOT apply:** bestbuy / ozon / wildberries-ceiling (IP/ASN-gated
before any channel probe), yelp (human gate). For those the no-CDP edge is secondary or
irrelevant — be honest (see §2).

---

## 2. Per-site classification (with confidence + the no-CDP evidence)

The decisive question for each: **does a no-CDP real browser pass it?** If yes ⇒
engine-addressable. If even a no-CDP real Chromium/Firefox fails identically from this IP
⇒ IP/geo. If it needs a validated human ⇒ human gate.

| Site | Vendor | No-CDP real browser passes (this IP)? | Classification | Confidence | The evidence / the binding constraint |
|---|---|---|---|---|---|
| **hyatt** | Kasada | **YES** (nocdp real title captured) | **ENGINE-ADDRESSABLE** (public) | engine-addr: high / flip: **medium** | Lowest Kasada tier (Patchright reaches loose-L3 13228 B). Residual = from-scratch-V8 surface fidelity. **Best first flip target.** |
| **canadagoose** | Kasada | **YES** | **ENGINE-ADDRESSABLE** (public) | high / **low** | nocdp passes zero-interaction. Residual = `/tl` sensor field divergence (K2-DIFF target). |
| **realtor** | Kasada | **YES** | **ENGINE-ADDRESSABLE** (public) | high / **low** | Same engine paths; larger interstitial (1764-1772 B) = harder deployment tier. Attempt last. |
| **etsy** | DataDome `rt:'i'` | partial — no-CDP runtime is the documented winning path; **must re-confirm live `rt`** | **ENGINE-ADDRESSABLE** (public plumbing) + vendor_solvers (daily-key only if runtime insufficient) | **medium** | `rt:'i'` is silent/self-solvable (Camoufox model). Load-bearing public bug = **child-iframe isolated cookie jar** (`02` §3.3). BO's zero-transport is the strongest no-CDP lever in the corpus. v150 also fails → this is a *lead over v150*, not parity. |
| **tripadvisor** | DataDome `rt:'i'` | partial | **ENGINE-ADDRESSABLE** (public) + vendor_solvers | medium | Same `rt:'i'` interstitial, same primitives, same cookie-jar bug, same lever as etsy. |
| **douyin** | ByteDance acrawler | **likely YES** ("real browser, same UA+IP, works"); pending R1 | **ENGINE-ADDRESSABLE** (public) | **medium**, pending R1 | Reclassified UP from "Firefox-only". A headless-**Chromium** signer (`carcabot/tiktok-signature`) proves the gate is automation-residue + consistency + builtin-integrity, NOT Gecko value distribution. BO's no-CDP + self-consistent Chrome-macOS UA = a real edge over Patchright. Residual = builtin-integrity (`Function.prototype.toString`/descriptor), same lever that closed Kasada `fsc`. |
| **wildberries** | wbaas (in-house) | **partially** — a *foreign-residential* IP loads it without VPN; our **US DC IPv6** gets the hard challenge | **MIXED**: self-solve/body-growth = ENGINE-ADDRESSABLE (beats v150 THIN-39); trust ceiling = IP-bound | medium | BO owns every primitive (module eval, dynamic-solver inject, 498-iterate, reload/cookie-gain). Gap = live-nav async drain (shared AWS lever). PoW difficulty is IP-trust-scaled; the *easy* branch needs a non-DC IP. |
| **bestbuy** | Akamai BMP | **NO no-CDP pass captured**; Patchright + Camoufox v150 + BO all fail identically (~7–8 KB) from this DC IP | **IP-GEO-BOUND** (primary) + low-confidence engine tail | IP-bound: high / engine tail: **low–medium** | The signature of an IP/ASN trust gate, not a fingerprint gap. Binding constraint = a **Favorable `_abck`** a datacenter ASN cannot reach. "Patchright passes 1246k" is a **debunked homedepot row** — no passing reference exists. Engine tail = the AWS live-nav-drain hypothesis (test for ~1h, free). |
| **ozon** | DDoS-Guard + RU geo | **NO** — "Ozon won't load without a VPN" from abroad; gate fires before any channel/fingerprint read | **IP-GEO-BOUND** | high | Thin ~156 B body = pre-JS geo refusal. BO already handles the DDoS-Guard 307 re-POST shape, but it is downstream of the geo gate. **RU residential IP required.** No engine half. |
| **yelp** | DataDome `rt:'c'` | **NO** (shows interactive captcha; Camoufox v150 also fails) | **HUMAN GATE** (not IP, not stealth-fixable) | high | `rt:'c'` interactive slider needs a validated human mouse path. No silent self-solve exists; even vendor_solvers cannot pass it. Validation must confirm yelp **STAYS** CHL. |

---

## 3. Consolidated engine-path table — ranked by ROI (sites × confidence / effort)

Each row: the no-CDP leverage + the file:line lever + the oracle-diff validation step.
Effort is rough engineering days. ROI rank weights (sites it can flip) × (confidence) /
(effort), and whether it is a shared multi-site lever.

| Rank | Lever | Sites | No-CDP leverage | File:line | Effort | Confidence | Oracle-diff validation |
|---|---|---|---|---|---|---|---|
| **1** | **Build the no-CDP oracle** (generalize `awswaf_probe.rs`→`nocdp_oracle.rs` + per-vendor pre-encryption interceptor; move `nocdp.sh`/`tl_capture.sh`/`probe_title.sh` to public `tools/oracle/`) | ALL | It is the instrument that *exploits* the moat — obtains a passing reference no CDP competitor can produce | `crates/browser/examples/awswaf_probe.rs`, `aws_capture.rs`; `net/src/lib.rs:680` (`/tl` log) | 3–5 | high (mostly built) | n/a — it IS the validation harness |
| **2** | **K-A: Populate the iframe child global** with the full parent API surface (document/navigator/constructors/timers/fetch/storage), realm-distinct | hyatt, canadagoose, realtor | The most probable live cause of `bot1225`/`unjzomuy` hard-fail; BO is past the CDP gate so the residual IS this kind of surface tell | `crates/js_runtime/src/extensions/dom_ext.rs:1217-1247` (**verified near-empty: only Window/window/self/globalThis/frames/length/opener + FP.toString**) | 2–4 | high (bug real) / **medium** (flips hyatt) | re-run `kasada_vm_dispatcher_trace` against the child-realm path; assert `unjzomuy` TypeError count → 0 |
| **3** | **K-B: K2-DIFF** — build the in-VM `/tl` plaintext dump (hook fetch/XHR on `/tl` pre-XOR, env-gated op) + field-diff vs the captured real-Chrome ref | hyatt, canadagoose, realtor | Diffs against the no-CDP-passing real `/tl` (`hyatt.tl_body.bin`, 36 KB) — the only trustworthy reference, impossible for a CDP engine to capture | new tool in `fetch_bootstrap.js` behind `BROWSER_OXIDE_KASADA_TL_DUMP=1`; decoders in private `browser_oxide_internal` | 3–5 | high (bounds the problem) | error-report half (XOR `omgtopkek`) diffs today; primary half (TEA-CBC) via the in-VM plaintext hook |
| **4** | **DD-2: Child-iframe cookie-jar sharing** — make `ChildIframe::from_url`/`from_srcdoc` use the shared session for the child V8 `FETCH_CLIENT` + audit/restore the thread-local | etsy, tripadvisor (+ unblocks CF Turnstile / any iframe-clearance vendor) | no-CDP gets the *self-solvable* `rt:'i'`; this lets BO actually *bank* the clearance the silent challenge produces | `runtime.rs:84-90` → `fetch_ext.rs:42-44` → `net/lib.rs:308` (fresh isolated jar) vs `net/lib.rs:363-368` (shared); `page.rs:2474` retry | 2–3 | high (bug real) / **low-alone** (necessary precondition) | offline child-realm oracle: assert a child-set `datadome=` is visible via `parent.cookies_for_url` post-fix |
| **5** | **AWS/WB live-nav drain** — replace the warm-rebuild 50 ms inter-script drain with an "async-in-flight" predicate; advance the self-solve chain in the final per-iter drain | wildberries (body growth, beats v150) + AWS cluster + low-conf bestbuy tail | Fully in-process — moat-safe (no external scheduler/protocol) | `page.rs:1703-1707` (50 ms warm drain), `:3661` (wb 50 ms inter-script), `:3674-3684` (DCL setTimeout); `:1881-2129` loop | 3–5 | high (AWS) / medium (wb) | offline self-solve probe reaches cookie-write but live nav doesn't ⇒ pure drain ⇒ public fix |
| **6** | **D-R1+R2: douyin builtin-integrity diff** — trace every builtin `sign()` toString/descriptor-inspects (offline oracle), diff vs no-CDP real Chrome 148, fix via `_maskAsNative`/`_maskFunction` | douyin | BO's no-CDP + self-consistent UA leaves no "cheap" tell → forces any residual to a builtin-shape diff BO can fix; Patchright cannot win this (CDP detected first) | `awswaf_probe.rs` on `/tmp/awswaf/douyin.html`; `stealth_bootstrap.js:25-104`; reload plumbing `window_bootstrap.js:1402-1410` | 3–5 | medium, pending R1 (B1 vs B2) | R1 decides `sign()` throws (B1) vs returns-and-rejects (B2); R2 diff until BO's dump = real Chrome's for the touched surface |
| **7** | **K-D: `[native code]` coverage regression test** across all realms (main + child + worker) | hyatt/canadagoose/realtor + DataDome + Akamai (hygiene) | locks in the L3 win that keeps BO clean where CDP engines leak | enumerate every exposed prototype method; assert `String(fn)` native + no `op_*` substring | 1 | high | the test IS the validation |
| **8** | **bestbuy Capture A + B** (fold into AWS drain work, ~1h) — does `akam/13` self-post under a 30 s oracle drain but not the 50 ms warm drain? + does no-CDP real Chrome hydrate from this DC IP? | bestbuy | the inverted no-CDP test: if even no-CDP Chromium fails, it's IP-bound (honest stop) | fork `awswaf_probe.rs`→`akamai_probe.rs`; `page.rs:1703-1707`; `nocdp.sh` datacenter vs residential | ~1 | low–medium | datacenter no-CDP hydrates ⇒ engine-addressable (reopen); fails ⇒ confirm IP-bound and stop |
| **9** | **DD-5: harden DataDome detection + multi-cookie solve** (insurance vs body-shape rotation) | etsy, tripadvisor | keeps the moat-enabled `rt:'i'` path robust as the target moves | broaden `is_datadome_challenge` (`page.rs:208`); extend `is_datadome_solved` to `_pxhd`/`_px3` | 1 | high / 0-direct | parallel-sweep etsy+tripadvisor + a clean DD site + yelp; confirm yelp STAYS CHL |
| **—** | **vendor_solvers tails** (Kasada `x-kpsdk-*` PoW, DD `ddCaptchaEncodedPayload` daily key, Akamai BMP PoW, douyin acrawler signer) | guaranteed-flip only | n/a | private `vendor_solvers` crate via `ChallengeSolver` | wks–mo, short half-life | low (per CLAUDE.md, forbidden in public crates) | only after the public oracle proves the residual is small/bit-exact |
| **—** | **IP/geo inputs** (bestbuy residential/mobile, ozon RU residential, wildberries non-DC) | bestbuy, ozon, wb-ceiling | n/a — IP-bound | infra, not engine | n/a | high (the binding constraint) | no-CDP IP A/B: residential vs DC confirms the gate |

---

## 4. The honest SOTA ceiling

Of the **10 frontier sites**, with the no-CDP advantage + the oracle, on THIS IP, in the
**public engine** (no `vendor_solvers`):

### Reachable in the public engine (the credible flip set): ~5–6
- **hyatt, canadagoose, realtor** (Kasada) — engine-addressable with **high confidence on
  the diagnosis** (nocdp proves it is not IP/behaviour), **medium (hyatt) to low
  (canadagoose/realtor) on a single-lever flip**. The cheap historical levers are spent
  (CSS calc DONE, FP.toString genuine-native, `_maskAsNative` across 12 files, cold-path
  45 s drain); the live lever is the child-realm population fix (`01` §3.2) → K2-DIFF.
- **etsy, tripadvisor** (DataDome `rt:'i'`) — engine-addressable with **medium confidence**,
  gated on the child-iframe cookie-jar bug (`02` §3.3). v150 also fails → any flip is a
  *lead over v150*. **Must re-confirm the live `rt` value** (if it has moved to `'c'`, it
  becomes a human gate).
- **douyin** — engine-addressable with **medium confidence, pending the decisive R1 probe**.
  A 6th if R1/R2 land.

This would be a **novel open-source SOTA result**: passing Kasada `rt:'i'` DataDome from a
public, no-CDP, from-scratch engine — sites that no CDP competitor (Playwright, Patchright)
and not even Juggler-based Camoufox v150 reliably pass.

### Need vendor_solvers (private) for a GUARANTEED flip
The token/PoW/daily-key tails: Kasada `x-kpsdk-ct/cd` PoW, DataDome `ddCaptchaEncodedPayload`
daily key, Akamai BMP `sensor_data` PoW, douyin acrawler signing. Pursue ONLY after the
public oracle proves the residual is small and bit-exact. Short half-life (rotates daily on
several); forbidden in public crates per CLAUDE.md.

### Genuinely IP/GEO-BOUND — out of public-engine reach: 3 (name the exact need)
- **bestbuy** — needs a **residential or mobile IP** (datacenter ASN cannot reach a
  Favorable `_abck`; Patchright + v150 + BO all fail identically from this DC IP).
- **ozon** — needs a **Russian residential IP** (foreign/DC IP gets the thin pre-JS body).
- **wildberries trust ceiling** — needs a **non-DC / foreign-residential IP (ideally RU)**
  to reach the easy PoW branch. (Its self-solve *half* is engine-addressable and already
  beats v150's THIN-39 even from a DC IP — a defensible partial win.)

### Human gate — out of reach for ALL headless engines: 1
- **yelp** — `rt:'c'` interactive captcha. Not IP, not stealth-fixable; Camoufox v150 also
  fails; even vendor_solvers cannot pass it. Validation must confirm yelp **STAYS** CHL
  (do not false-flip the human gate).

### The number
**Realistic public-engine ceiling: 5–6 of 10** (Kasada×3 + DataDome `rt:'i'`×2 + douyin as
a probe-gated 6th). **3 IP/geo-bound. 1 human gate.** The remaining guaranteed-flip tails
are private `vendor_solvers`. The no-CDP moat is what makes the 5–6 *uniquely BO's* — no
CDP engine can occupy that region.

---

## 5. The phased plan

### Phase 0 — Build the no-CDP oracle (the enabler, do first)
- Generalize `awswaf_probe.rs` → `nocdp_oracle.rs` with per-vendor pre-encryption
  interceptors; generalize `aws_capture.rs` → `nocdp_capture.rs`.
- Move `nocdp.sh`/`tl_capture.sh`/`probe_title.sh` to public `tools/oracle/` (pure
  capture/observe — no bypass code); keep `*.pcap`/`*.keys`/`*.bin` private.
- Add `tools/oracle/decode_kasada_tl.py`.
- **Moat standing test:** adopt rebrowser-bot-detector offline; assert BO passes all 5 by
  construction; assert it FAILS once `CdpServer::start_ephemeral` is bound (proves the
  moat = "CDP server off"). **Gate `crates/protocol` behind an off-by-default Cargo feature.**

### Phase 1 — Kasada K2-DIFF (highest-value, evidence-backed engine gap)
1. **K-A child-realm population** (`dom_ext.rs:1217-1247`) — the single most probable cause
   of the biggest trust driver (`bot1225`). Re-run the dispatcher trace against the
   child-realm path first to confirm before building the fix.
2. **K-B K2-DIFF** end-to-end — build the in-VM `/tl` plaintext dump, decode + field-diff
   vs the captured no-CDP real `/tl`. Each divergent field = a named, prioritized bug.
3. **K-D** `[native code]` coverage regression test (hygiene; also helps DataDome/Akamai).
4. Pass criterion: no error POSTs + Kasada serves the real `<title>` (matching the nocdp
   evidence). **hyatt first** (lowest tier), then canadagoose, then realtor.
5. Do **NOT** spend Kasada effort on the drain (mis-aimed — the cold path already drains
   45 s) or on a `vendor_solvers` VM port (short half-life, forbidden public) until K-B
   proves the tail is small + bit-exact.

### Phase 2 — DataDome etsy / tripadvisor
1. **DD-2 child-iframe cookie-jar sharing** (the gate everything depends on; also unblocks
   CF Turnstile and any iframe-clearance vendor).
2. **DD-5** harden detection + multi-cookie solve (insurance vs body-shape rotation).
3. Live-nav `[datadome-trace]` experiment to disambiguate drain vs cookie-trap vs
   fingerprint (run on a free IP window).
4. **Re-confirm the live `rt` value** before investing — if etsy moved to `'c'`, it is a
   human gate this quarter. yelp stays CHL by design.

### Phase 3 — bestbuy + douyin (the inverted-thesis + the reclassified probe)
1. **bestbuy** — fold Capture A (`akam/13` self-post under long vs 50 ms warm drain) + Capture
   B (no-CDP Chrome datacenter vs residential) into the AWS drain work (~1h). If Capture A
   shows the drain branch → land the shared fix → +1 ahead of v150 for free. Otherwise
   confirm IP-bound and **stop**. Correct the two docs that mis-cite "Patchright PASS 1246k".
2. **douyin** — R1 offline acrawler trace (B1 vs B2, decisive, ~1d) → R2 builtin-integrity
   diff vs no-CDP real Chrome → R3 confirm zero automation residue. Keep behind the AWS
   drain (7–8 sites) and homedepot hardening in absolute priority, but the classification is
   now "engine-addressable, worth the cheap R1," not "out of scope."

### Phase 4 — geo / yelp (honest flags)
1. **wildberries** — run the offline self-solve oracle (shares AWS tooling); ship the public
   live-nav drain fix; run the IP A/B (foreign-residential vs US DC) to set the ceiling. Any
   forward progress beats v150's THIN-39. Set Accept-Language `ru-RU` for fidelity.
2. **ozon** — capture + confirm the pre-JS geo refusal, then mark `diagnostic:true` and drop
   from the production denominator. Re-open only with a RU residential IP.
3. **yelp** — confirm it STAYS CHL; document as a human gate; do not chase.

### Cross-cutting guards
- **Never run live navs on the contended IP while a competitor benchmark holds it** — the
  oracle is offline-capture-first; gate the single live steps on a free IP window.
- **Never use Playwright/Patchright/MCP as the real-browser oracle** for these CDP-sniffers
  — they are detected and poison the reference. Use `nocdp.sh` (Tier 0) only.
- **Keep `crates/protocol`'s CDP server OFF the navigate path** (gate behind a feature).
- **Do not re-add per-vendor bypass code to public crates** (CLAUDE.md / `aecdf19`).
- **Never assert "IP ban" without a captured hard-403 from no-CDP** (memory rule).

---

## 6. Sources
The 7 frontier deep-dives `01`–`07` (this directory) and their cited prior art:
`docs/v0.1.0-parity-workflows/external/{VENDOR_kasada,VENDOR_datadome,VENDOR_akamai,
ENGINE_camoufox_v150,ENGINE_chromium_stealth,DETECT_vectors,NETWORK_fingerprint}.md`;
`docs/v0.1.0-parity-workflows/sites/SITE_{kasada_cluster,etsy,bestbuy,wildberries,douyin}.md`;
`docs/vNext/{06_R-KASADA-FRONTIER,12_R-DATADOME-WASM,04_R-WBAAS-WILDBERRIES,
05_R-SPA-DOUYIN-SIG,03_R-BESTBUY-AKAMAI}.md`; `docs/HANDOFF_2026_05_28b.md` §4–§5.1.
Source verified 2026-05-29: `dom_ext.rs:1217-1247` (near-empty child global),
`crates/browser/Cargo.toml:36` (non-optional `protocol` dep). Memory: the Kasada notes,
`proxy_not_the_problem`, `state_2026_05_15_playwright_ab_decisive`,
`measurement_holistic_chl_fp_trap`.
