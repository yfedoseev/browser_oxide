# 43 — Strategic gap assessment + 2027 forecast

**Synthesis chapter.** Translates chapter 42's holistic vision into concrete prioritization: what to build for v0.1.0, what to defer to v0.2.0, what to research for v0.3.0+.

If chapter 42 is "the map", this is "the route."

## TL;DR — the strategic picture

After integrating 42 chapters of vendor + technique research:

1. **v0.1.0 is achievable in ~25 engineering-days** of focused work — none of it speculative; all has existing measured signal.
2. **The single highest-ROI investment** is the WebGL prototype mask sweep (1-2 days; moves 11 of 12 vendors). Do this FIRST.
3. **The "wiring gap" cluster** (humanize.js doesn't call existing Rust generators) costs ~5 days for ~5-vendor lift. Second-highest ROI.
4. **MessageChannel implementation** (~3-5 days) unblocks duolingo + 5 other Worker-using vendors. Third.
5. **AWS WAF + Cloudflare Managed Challenge work is high-effort, vendor-specific** (weeks). Defer to v0.2.0 unless customer needs amazon-specifically.
6. **Kasada (canadagoose/hyatt/realtor) stays research-bound** — chapter 08's K2-DIFF is months of work for 3 sites. Beyond v0.1.0 scope.
7. **By 2027** network-layer fingerprinting will collapse (ECH + Privacy Sandbox PAT). Behavioral biometrics will become THE blocker. **Invest in humanize.js now; the wiring + curve-improvement work has long-term ROI.**

## 1. BO capability per technique vs industry-standard

For each Tier-S / Tier-A technique from chapter 42 §2, BO's current capability quantified:

| Technique | Industry standard | BO capability | Gap | Effort to close |
|---|---|---|---|---|
| TLS JA4 | Real Chrome 148 | boring2 4.15.15 codename chrome_147; UA-148 split intentional | small — needs JA4 ground-truth capture (chapter 23 §10) | 2-3 days |
| HTTP/2 SETTINGS | Chrome 148: 65536/0/6291456/262144 + 15663105 window | h2 0.5.17, profile-driven | small — Firefox H2 differentiation gap (chapter 39 §3.5) | 2 days |
| WebGL params | Per-real-GPU consistent triple (VENDOR + RENDERER + UNMASKED_*) | stealth profile injects per-profile but coverage is 7/80 prototype methods | **medium** — mask sweep + golden snapshot | 2-3 days |
| Canvas 2D hash | Pixel-identical real Chrome rendering | Skia-based via `crates/canvas` — text/curve coverage TBD per chapter 38 | **medium** — emoji + composite + per-profile snapshot | 3-5 days |
| Audio fingerprint | Chrome WebKit DynamicsCompressor output | Blink port at `crates/canvas/src/audio.rs:237-689`; 3.6ppm at -24dB, 16% off at -50dB | **medium** — calibrate -50dB case | 5-7 days |
| `Function.prototype.toString` mask | All natives return `[native code]` | `_maskAsNative` exists; 7/80 WebGL covered; Event subclasses unmasked; Headers/Request/Response unmasked | **HIGH** — mass sweep | 2 days |
| `performance.now()` granularity | 5µs cross-origin isolated, 100µs default | `op_perf_now_humanized` with LogNormal jitter + spike (chapter 40 §2.6) | **small** — wire `timeOrigin` to humanized op | 0.5 day |
| Mouse trajectory | Σ-Λ Plamondon curves with per-user variance | humanize.js fixed Bezier, ~30 events, `Math.random()` per page | **HIGH** — Σ-Λ impl + two-level seed wiring | 5-10 days |
| Keystroke dynamics | CMU/Buffalo dwell + bigram flight | Rust generator EXISTS at `behavior.rs:421-464`, NOT WIRED | **WIRING ONLY** | 1-2 days |
| Touch events on mobile | Real iOS/Android event stream | ZERO touch events on iPhone/Pixel profiles | **HIGH** — synthesis + wiring | 5 days |
| `MessageChannel`/`MessagePort` | Real paired-port routing | NO-OP stub at `window_bootstrap.js:2256-2272` | **CRITICAL** — full impl | 3-5 days |
| Worker-context fingerprint | Identity with main thread | NOT AUDITED (chapter 41 §4.4) | **UNKNOWN — HIGH RISK** | audit 3 days; fix unknown |
| Canvas `webgl-render` feature | ANGLE-Metal real GPU output | OFF by default; even when ON, software-OSMesa output | **HIGH** — but requires real GPU | 1-3 months |
| WebRTC mDNS | Real Chrome `<random>.local` candidate | Implemented at `window_bootstrap.js:4983-4996` | **VERIFY** — chapter 39 §4 | 1 day |
| Vendor-detect markers (cf-mitigated etc.) | Log on response → branch on cookie/body | 3 of 11 vendor headers logged | **TRIVIAL** — extend page.rs:1049-1057 | 1 hour |

## 2. Top 10 cross-vendor leverage fixes ranked by ROI

`ROI = (vendors moved × confidence × site-flip probability) / engineer-days`

| # | Fix | Vendors moved | Days | Confidence | Source |
|---|---|--:|--:|---|---|
| **1** | WebGL prototype mask sweep (canvas_bootstrap.js:1290-1295 → all 80 methods) | 11 | 1-2 | HIGH | 38 §5.4, 16 §5 |
| **2** | WebGL per-profile param golden snapshot | 11 | 1 | HIGH | 38 §5.5 |
| **3** | `Function.prototype.toString` mass mask sweep (Event subclasses, Headers/Request/Response, XHR, WebGLRenderingContext) | 11 | 2 | HIGH | 16 §5, 08 Lever 3, 41 §4.4 |
| **4** | Canvas `toDataURL` golden parity test per profile | 10 | 2 | MEDIUM | 38 §5.6 |
| **5** | Wire keystroke generator into humanize.js (Rust exists; JS missing call) | 8+ | 1-2 | HIGH | 40 §3.2, 26 §3 |
| **6** | Wire two-level seed into humanize.js (Rust exists; JS uses Math.random) | 8+ | 1 | HIGH | 40 §5 |
| **7** | Wire `performance.timeOrigin` to humanized op | 8 | 0.5 | HIGH | 40 §2.6 |
| **8** | `MessageChannel`/`MessagePort` proper paired-port impl | 6 + unblocks duolingo | 3-5 | HIGH | 17, 41 §4.4 |
| **9** | RAF jitter (currently 16ms deterministic; needs variance) | 7+ | 1 | HIGH | 40 §2.3 |
| **10** | Vendor-detect markers extension at page.rs:1049-1057 (cf-mitigated + x-akamai-transformed + x-perimeterx-id + 5 more) | (detection coverage) | 1 hour | HIGH | 18 §4, 25 §1, 26 §3.C |

**Total Tier-1+2 budget for top 10: ~13.5 days.**

## 3. v0.1.0 prioritization

The v0.1.0 success scorecard (per chapter 00) requires:
- routed best-of-4 strict pass ≥ 115 (Camoufox = 113)
- ≥ 110 strict on at least one single profile
- zero functional regressions
- memory < 1.5× honest Camoufox
- no panics (wellsfargo pool fixed)

### v0.1.0 MUST-HAVE — 12-15 days

These are the fixes that get us to the bar.

| Fix | Days | Expected site-flip | Source chapter |
|---|--:|---|---|
| WebGL prototype mask sweep | 1-2 | sweep variance reduces; multi-vendor lift | 38, 16 |
| WebGL per-profile param golden snapshot | 1 | sweep variance reduces | 38 |
| `Function.prototype.toString` mass mask sweep | 2 | Kasada blob count drops; multi-vendor | 16, 08, 41 |
| Wire keystroke generator | 1-2 | Akamai BMP + Kasada + Radware behavioral score | 40 |
| Wire two-level seed | 1 | cross-vendor per-session coherence | 40 |
| Wire `performance.timeOrigin` | 0.5 | Kasada origin-skew probe defeated | 40 |
| Vendor-detect markers extension | 0.04 | detection-coverage; enables retry logic | 18, 25, 26 |
| reddit fix — implement HTMLFormElement.elements | 0.5 | reddit flips (Camoufox-only-pass site) | 05 H2, 17 |
| MessageChannel + MessagePort proper impl | 3-5 | duolingo flips (Camoufox-only-pass site) + Worker-using vendors | 17, 41 |
| 3-run aggregated baseline sweep | 1 (~10h wall) | establishes honest pre-fix baseline | 14 §L5 |
| Apply fixes + 3-run validation | 2 | confirms ≥ 115 routed | 14 |

**Total: ~13-16 days engineering + ~20h wall-clock for sweeps.**

### v0.1.0 SHOULD-HAVE — 5-10 days additional

These are non-blocking for the 115 bar but raise per-profile quality.

| Fix | Days | Expected | Source |
|---|--:|---|---|
| RAF jitter (Kasada-class cadence stddev) | 1 | Kasada gap closes (per blob capture) | 40 §2.3 |
| Touch event synthesis on iPhone/Pixel profiles | 5 | iphone/pixel uplift on mobile-targeted sites | 40 §3.4 |
| Cloudflare cf-mitigated header detection + iphone fix | 1 hour + capture | iphone parity (98 → 102+) | 25 |
| Canvas emoji golden snapshot per profile | 2 | reduces canvas-emoji variance across profiles | 38 §2.4 |

### v0.1.0 NICE-TO-HAVE — schedule permitting

| Fix | Days | Source |
|---|--:|---|
| Audio DynamicsCompressor -50dB calibration | 5-7 | 38 §3.3 |
| Worker-context fingerprint audit (no fix, just discover the gap) | 3 | 41 §4.4 |
| JA4 ground-truth capture | 2-3 | 23 §10, 39 §2 |
| Firefox H2 differentiation | 2 | 39 §3.5 |

### NOT in v0.1.0 (explicit defer)

| Reason | Items | Target |
|---|---|---|
| Vendor-specific solver (per CLAUDE.md, → vendor_solvers private) | AWS WAF token, Kasada PoW, DataDome WASM | private repo |
| Open research frontier | Kasada K2-DIFF + 16 fields (chapter 08) | v0.3.0+ |
| Long-tail vendor coverage | F5/Shape, Arkose, Imperva captures | v0.2.0 |
| Big infrastructure | V8 snapshot warming (21 §A), parallel cold (21 §B) | v0.2.0 |
| Customer-onboarding work | k8s YAMLs, Lambda handler (22) | v0.2.0 |

## 4. v0.2.0 roadmap (3-6 months post v0.1.0)

After v0.1.0 ships at ≥ 115 routed, the next horizon:

### v0.2.0 themes

**Theme A — solver expansion (private vendor_solvers crate work)**
- AWS WAF challenge solver (chapter 06 Alternative B or C) — recovers amazon-de/in/com-au/jp/imdb on most rolls
- DataDome WASM-iframe-daily-key solver — recovers etsy/tripadvisor/yelp
- Akamai BMP sensor_data v2/v3 — restore what aecdf19 stripped, recovers adidas-class + maybe homedepot

**Theme B — measurement infrastructure**
- Capture mode in sweep_metrics (chapter 04) — production-quality
- Multi-run aggregator (chapter 14 §L5) — fully wired to CI
- Per-vendor test harnesses (chapter 34) — at least 3 vendors

**Theme C — perf**
- V8 snapshot warming (chapter 21 §A) — -100ms/cold + -30MB
- Parallel cold sweep harness (chapter 21 §B) — 4× sweep speedup
- Pool DOM arena shrink_to_fit (chapter 09 §6) — pool RSS < 800 MB

**Theme D — production maturity**
- k8s Deployment YAML reference
- Lambda runtime + handler skeleton
- healthz/readyz endpoints
- Customer onboarding playbook tested with real customer

### v0.2.0 budget — 3-6 months

Solver work is week-each. Infra is days-each. Production is weeks total.

## 5. v0.3.0 and 2027 forecast

### v0.3.0 themes (6-12 months out)

**Theme A — Kasada SOTA push**
- K2-DIFF execution (chapter 08 Lever 1) — capture our `/tl` vs real Chrome
- 16-field error-blob fix list (chapter 08 §Phase 3)
- CSS calc math completion (chapter 08 Lever 2)
- _maskAsNative completeness (chapter 08 Lever 3)
- Goal: pass at least 1 of canadagoose/hyatt/realtor

**Theme B — Profile expansion (chapter 19)**
- safari_18_macos (new TLS branch needed)
- chrome_148_windows (already coded — just wire into sweep_metrics)
- chrome_148_linux (already coded)
- routed best-of-7 target: 118-122

**Theme C — Behavioral biometrics depth**
- Σ-Λ Plamondon mouse trajectory implementation
- Per-session coherence (already partially done in v0.1.0 wiring)
- Touch on iPhone/Pixel (deferred from v0.1.0)
- Goal: defeat Radware IDBA + Akamai BMP behavioral scoring

### 2027 forecast — multi-year strategic bets

#### Network-layer fingerprinting will erode

**Driver**: ECH (Encrypted ClientHello) rolling out: Chrome 117+ stable, Firefox 119+, growing CDN support. When >50% of HTTPS sites support ECH, JA4-by-SNI dies. Vendors lose their cheapest fingerprint.

**Implication for BO**: chapter 23's TLS work has finite shelf life. Don't over-invest. The boring2 4.15.x line is fine for 2026-2027; revisit only if Chrome lands ECH-related ClientHello changes that diverge from boring2.

**Opportunity**: BO can lean on its post-network-layer differentiators (JS env coherence + behavioral) when network-layer collapses. Camoufox doesn't get this advantage because Firefox's ECH support trails Chrome's.

#### Behavioral biometrics will become THE blocker

**Driver**: per chapter 40 §7, ML detection budget growing faster than ML synthesis budget. Vendors can train better detectors than open-source can train generators.

**Implication for BO**: humanize.js wiring + Σ-Λ Plamondon are GROUND-FLOOR investments for 2027. The Rust keystroke generator (currently unwired) is the right primitive — needs activation and improvement.

**Risk**: at extreme, no engine can defeat next-gen behavioral models. Some sites will become unreachable. Honest pitch: "we get you to 90% of sites; the last 10% need either captcha-solving services or human-in-the-loop."

#### WebGPU becomes the new WebGL fingerprint

**Driver**: Chrome 113+ ships WebGPU. Vendors will add probes within 2026. New API surface = new fingerprint dimensions (GPU adapter, shader compilation, compute pipeline timing).

**Implication for BO**: chapter 17 §parity matrix needs WebGPU coverage. Currently NOT in our priority list. Should track in chapter 15 open questions for 2026-Q4 revisit.

#### Privacy Sandbox PAT replaces some CAPTCHAs

**Driver**: Apple-Google-Cloudflare push to replace user-facing CAPTCHAs with attestation tokens. Browser+device combination proves "human-ness" without solving puzzles.

**Implication for BO**: we will NOT pass sites that require PAT. Either get PAT support (requires real device + OS attestation = not in scope for headless) OR accept these as residual.

**Honest pitch**: "PAT-protected sites are out-of-scope; this is industry-wide reality, not a BO limitation."

#### Anti-bot vendor consolidation continues

Per chapter 33 + 12 + 27:
- Imperva → Thales (2023)
- Shape → F5 (2020)
- Signal Sciences → Fastly (2020)
- Distil → Imperva (2019)
- ShieldSquare → Radware (2019)

**Predicted 2026-2027**:
- Castle: M&A target (smaller, fundable)
- Arkose Labs: IPO or acquisition
- DataDome: IPO track (large, growing)
- Camoufox: likely stays open-source community-led, but watch for commercial fork

**Implication for BO**: vendor product changes accelerate post-acquisition. Track via chapter 33 quarterly probe-rotation log.

## 6. Risk-adjusted strategic bets

What's safe vs speculative:

### SAFE bets (high confidence, ship by v0.1.0)
- WebGL mask sweep — public-research-backed, mechanical work
- humanize.js wiring fixes — engineering exists, just connect
- MessageChannel impl — well-understood spec
- Vendor-detect markers extension — trivial

### SPECULATIVE bets (lower confidence, plan for v0.2.0 minimum)
- AWS WAF solver — vendor rotates, solver-arms-race
- DataDome restoration — was working pre-aecdf19; should work again
- Kasada K2-DIFF — research-bound, may not yield site flips

### LONG-TERM positioning bets (v0.3.0+)
- Σ-Λ Plamondon mouse curves — bet on 2027 behavioral biometrics dominance
- WebGPU probe coverage — bet on emerging vendor probe
- ECH adaptation — bet on TLS encryption rollout

### EXPLICIT NON-BETS (don't pursue)
- Real-device attestation (PAT) — out of scope for headless
- ML-based behavioral synthesis (vs procedural) — too expensive vs ROI
- Per-tenant custom solvers — customer-specific work, not engine

## 7. Customer pitch lines per persona

After all this synthesis, what's the honest customer story:

### To a price-sensitive scraper at scale
"BO pool path is ~14 pages/min, 4× less RAM than Playwright. For benign-content sites your cost-per-1M-pages is $0.012. We pass 108 routed strict on the 126-corpus today; ≥115 post-v0.1.0."

### To a customer with amazon-specific needs
"We pass 4-7 of 8 amazon variants depending on AWS WAF's risk roll. Camoufox passes 6 of 8 deterministically. If 100% amazon coverage is critical, Camoufox today; us post-v0.2.0 with private AWS WAF solver."

### To a customer with high-value sites that use Akamai BMP
"We uniquely win adidas on Firefox profile — Camoufox FAILS it. Routed best-of-4 finds this without thinking. For homedepot specifically, even Camoufox fails — the only thing that works is Playwright (which fails most other vendor categories). The honest answer: pick BO for breadth, Playwright for homedepot specifically."

### To a customer worried about CDP-driver detection
"We have NO CDP — invisible to CDP-detection heuristics. Patchright spends engineering effort hiding CDP; we don't have CDP to hide. We lead the CDP-driver tier by 15 Pass on the 126-corpus."

### To a customer scraping Kasada-protected sites (real-estate, premium retail)
"Kasada is the open-source SOTA frontier. Camoufox gets 4 of 5; we get 1 of 3 today. Honest: this is post-v0.1.0 research. Use Camoufox today for Kasada-heavy workloads; us when K2-DIFF lands."

### To a customer that needs Cloudflare-heavy sites
"We pass Cloudflare Managed Challenge on chrome/pixel/firefox profiles. iphone profile uniquely loses 6 sites (chapter 11). Use routed best-of-4 OR explicitly route AROUND iphone for Cloudflare sites."

### To a security team evaluating for compliance
"In-process Rust binary, no Chrome dependency, no CDP. License: MIT OR Apache-2.0 mechanically enforced. Memory bounded with pool recycle policy (chapter 22). Per chapter 24 risk register, our largest dependency risks are deno_core (currently 90 versions behind upstream — tracked) and boring2 (4.15.x line, no 5.x upgrade path)."

## 8. Honesty section — what this assessment doesn't promise

- **115 routed strict in v0.1.0 is a TARGET not a guarantee.** WAF variance is ±5; the 3-run aggregation gate (chapter 14 §L5) is the source of truth. If the post-fix sweep median lands at 113, we should ship and call it parity, not chase the +2.
- **WebGL mask sweep moves "11 vendors" in expectation** — actual site-flip count is variable. Could be 0-5 sites flip. The leverage is "this fix is the input to many vendors' fingerprint check" — whether each vendor's threshold actually flips on this fix depends on their model.
- **Wiring fixes are validated as "code exists" but NOT as "behavior works end-to-end".** Wire-up has its own bugs. Each wiring fix needs the per-vendor test harness (chapter 34) to confirm behavior.
- **Kasada K2-DIFF success is genuinely unknown.** Chapter 08 documents the plan; whether it converges in 1-3 months is research-bound.
- **AWS WAF non-determinism ceiling is ~85% (chapter 06)** — even a perfect solver caps at this because AWS's risk-roll is server-side probabilistic.
- **Behavioral biometrics is an arms race we're not in front of** — vendors have ML budget we don't. The honest 2028+ pitch may include "some sites are unreachable for any non-real-browser engine."

## 9. The 30-second elevator version

If you only have 30 seconds with leadership:

> "BO is 5 sites behind Camoufox on a 126-site corpus today. We have 13 engineering-days of mechanical fixes that should close that gap and pull us 2-5 sites ahead. The fixes are all in cross-cutting fingerprint coverage that helps 5-11 vendors at a time — not vendor-specific solver work. Most are 'wire existing Rust code into JS' work, not new code. We've never been closer to leadership on this benchmark, and the strategic moat (cross-layer coherence + in-process + 4× lighter memory) is real."

## 10. Files referenced

### The synthesis sources
- 42_HOLISTIC_VISION.md (the matrix this strategic plan ranks against)
- All chapters 03-41 (the research material)

### Engine source for v0.1.0 fixes
- `crates/js_runtime/src/js/canvas_bootstrap.js:1290-1295` (Fix #1, #2)
- `crates/js_runtime/src/js/window_bootstrap.js:2256-2272` (Fix #8 MessageChannel)
- `crates/js_runtime/src/js/window_bootstrap.js:4983-4996` (verify WebRTC mDNS)
- `crates/stealth/src/behavior.rs:109-115` (Fix #6 two-level seed)
- `crates/stealth/src/behavior.rs:421-464` (Fix #5 keystroke generator)
- `crates/browser/src/js/humanize.js` (the wiring layer that needs all the wire-ups)
- `crates/js_runtime/src/extensions/perf_ext.rs` (Fix #7 timeOrigin)
- `crates/browser/src/page.rs:1049-1057` (Fix #10 vendor-detect markers extension)
- `crates/js_runtime/src/js/dom_bootstrap.js:1080-1110` (reddit HTMLFormElement.elements impl point)
- `crates/canvas/src/audio.rs:237-689` (NICE-TO-HAVE -50dB calibration)
- `crates/browser/src/page.rs:3389` (already fixed — build_page drain restored)
- `crates/browser/src/page.rs:216` (already fixed — worker reap)

### Sweep baseline
- `~/projects/browser_oxide_internal/benchmarks/baselines/2026-05-24/` — pre-fix baseline
- Future: `<same>/2026-MM-DD/` after fixes land

### Synthesis cross-links
- 00 README — success scorecard (update to reflect 115 + per-fix items)
- 14 testing validation — 3-run aggregation gate
- 15 open questions — research backlog (update with bets that didn't pan out)
- 24 risk register — track deno_core / boring2 / Chrome 149 timing risks

## How to use this plan

For v0.1.0 release management:
1. Print the §3 "MUST-HAVE" table — that's the project plan
2. Track in `15_OPEN_QUESTIONS.md` any blockers per fix
3. After each fix lands, run chapter 34's per-vendor test harness + chapter 14 §L3 spotcheck
4. After all 11 must-have fixes land, run chapter 14 §L5 3-run aggregated sweep
5. If routed median ≥ 115: tag v0.1.0. If 113-114: ship and call it parity. If <113: investigate per-fix non-yield and reprioritize.

For v0.2.0 planning: read §4. For 2027 strategic positioning: read §5 + §6.

If you're a contributor about to implement ONE fix: pick from §2 ranked list, then read the source chapter for the fix (e.g., Fix #5 → chapter 40 §3.2).
