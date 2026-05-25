# 42 — Holistic vision: vendor × technique matrix

**Synthesis chapter.** Distills chapters 06-41 into a single cross-vendor view organized by detection TECHNIQUE not vendor. Reveals which engine fixes move the most vendors at once.

The premise from the iOS-learns-Android research strategy: studying ONE vendor reveals the vendor's tricks; studying TWELVE reveals the **patterns** that recur across the industry. Patterns are where the leverage lives.

## TL;DR — the single picture

After researching 16 anti-bot vendors across 36 detection techniques:

1. **WebGL parameters are the single highest cross-vendor surface** — 11 of 12 corpus vendors read them. One 1-day fix (per-profile golden snapshot) moves 11 vendors at once.
2. **humanize.js has WIRING GAPS** — the Rust generators exist (`crates/stealth/src/behavior.rs:421-464` for keystroke, `:109-115` for two-level seed) but `humanize.js` never calls them. ~5 days of "connect existing code" work moves 5+ behavioral-scoring vendors.
3. **`MessageChannel`/`MessagePort` is a no-op stub** at `window_bootstrap.js:2256-2272` — single ~300 LOC fix unblocks recaptcha enterprise (duolingo blocker per chapter 05) AND helps every Worker-using vendor.
4. **The actual blocker for most sites is the FINGERPRINT GATE before PoW/WASM runs** — not capability gaps. V8 executes any PoW/WASM; vendor JS bails before that based on a fingerprint check. So solving "the fingerprint" is the path, not "solving the challenge."
5. **Chapter 07 primitives cover 3 of 4 Cloudflare products plus DataDome plus parts of Akamai** — already-spec'd generic engine primitives have MASSIVE multi-vendor reach.
6. **Camoufox is NOT strictly better than BO** — measured counterexamples: zillow (BO wins PerimeterX), adidas (BO firefox uniquely flips), wellsfargo (BO passes Akamai). Our differentiator is REAL: byte-perfect Chrome TLS + per-profile rotation.

## 1. The vendor × technique matrix

Rows = detection techniques. Columns = vendors (12 in our universe: 7 corpus + 5 customer-onboarding). Cell = "does vendor X use technique Y" with citation chapter.

Legend: ✓ = confirmed in public research; ✓? = partial / inferred; ? = unknown; – = explicitly does not use this layer.

| Technique \\ Vendor | AWS WAF | DataDome | Akamai BMP | Kasada | CF Mgd | CF Turnstile | PerimeterX | Imperva ABP | F5/Shape | Arkose | Fastly NGW | Radware | Σ vendors |
|---|---|---|---|---|---|---|---|---|---|---|---|---|--:|
| **Visual + audio fingerprinting (chapter 38)** | | | | | | | | | | | | | |
| WebGL params (RENDERER, VENDOR, extensions) | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | – | ✓ | **11** |
| Canvas 2D rendering hash | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓? | ✓ | ✓ | – | ✓ | **11** |
| WebGL pixel readback (`readPixels`) | ✓ | ✓ | ✓? | ✓ | ? | ✓? | ✓ | ? | ✓ | ? | – | ? | **6+** |
| Audio (DynamicsCompressor) | ✓? | ✓ | ✓ | ✓ | ✓? | ✓? | ✓ | ? | ✓ | ? | – | ✓? | **8+** |
| Font enumeration | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ? | – | ✓ | **9** |
| Emoji rasterisation | ✓? | ✓ | ✓? | ✓? | ✓? | ✓? | ✓ | ? | ✓? | ? | – | ? | **8?** |
| **Network-layer fingerprinting (chapter 39)** | | | | | | | | | | | | | |
| TLS JA3/JA4 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓? | ✓ | ✓ | **12** |
| HTTP/2 SETTINGS order | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ? | ✓ | ✓ | **11** |
| HTTP/2 pseudo-header order | ✓? | ✓ | ✓ | ✓? | ✓ | ? | ✓ | ✓ | ✓ | ? | ✓ | ✓ | **10** |
| WebRTC mDNS leak | ? | ✓? | ✓? | ✓ | ✓? | ? | ✓ | ? | ✓ | ? | – | ✓? | **5+** |
| ALPN order | ✓ | ✓ | ✓ | ✓ | ✓ | ? | ✓ | ✓ | ✓ | ? | ✓ | ✓ | **10** |
| **Timing fingerprinting (chapter 40)** | | | | | | | | | | | | | |
| `performance.now()` granularity | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓? | ✓ | ✓? | – | ✓ | **10** |
| `Date.now()` skew vs `performance.timeOrigin + now()` | ✓? | ✓ | ✓ | ✓ | ✓? | ✓? | ✓ | ? | ✓? | ? | – | ✓? | **8** |
| `requestAnimationFrame` cadence + jitter | ✓? | ✓ | ✓ | ✓ | ✓? | ✓? | ✓ | ? | ✓? | ? | – | ✓ | **7+** |
| `setTimeout` nesting clamp (4ms after 5 levels) | ? | ✓? | ✓? | ✓ | ? | ? | ✓? | ? | ? | ? | – | ? | **3+** |
| Background-tab throttle | ? | ? | ✓ | ✓ | ? | ? | ? | ? | ? | ? | – | ? | **2+** |
| **Behavioral biometrics (chapter 40)** | | | | | | | | | | | | | |
| Mouse trajectory (Σ-Λ Plamondon curve fit) | ? | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓? | – | ✓ | **9** |
| Keystroke dynamics (dwell + flight + bigram) | ? | ✓? | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ? | – | ✓ | **8+** |
| Scroll velocity / acceleration | ? | ✓? | ✓ | ✓ | ✓ | ✓ | ✓ | ✓? | ✓ | ? | – | ✓ | **7+** |
| Touch events (mobile) | ? | ✓? | ✓ | ✓ | ? | ? | ✓ | ✓? | ✓ | ? | – | ✓ | **6+** |
| **PoW + WASM + Worker (chapter 41)** | | | | | | | | | | | | | |
| Proof-of-work computation | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ? | ? | – | ✓? | **9** |
| WASM-based challenge | ✓ | ✓ | ✓ | ✓? | ✓? | ✓ | ✓? | ? | ? | ✓? | – | ? | **6+** |
| Worker-context fingerprint | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ? | ✓ | ✓? | – | ✓? | **9+** |
| `MessageChannel`/`MessagePort` use | ? | ✓ | ✓ | ✓ | ✓ (Turnstile) | ✓ | ? | ? | ✓? | ✓? | – | ? | **6+** |
| **Stealth-engine surface (chapter 16)** | | | | | | | | | | | | | |
| `Function.prototype.toString` of patched natives | ✓ | ✓ | ✓ | ✓ (sfc/sdt) | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | – | ✓ | **11** |
| `navigator.webdriver` boolean | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | – | ✓ | **11** |
| `navigator.userAgentData` coherence | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | – | ✓ | **11** |
| `navigator.plugins` / `mimeTypes` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓? | ✓ | ✓? | – | ✓? | **10** |
| `navigator.permissions` query state | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ? | ✓ | ✓? | – | ✓? | **9** |
| `Notification.permission` | ✓ | ✓? | ✓ | ✓ | ✓ | ✓ | ✓ | ? | ✓? | ? | – | ? | **7** |
| `Battery API` (deprecated but checked) | ✓ | ✓? | ✓ | ✓ | ? | ? | ✓ | ? | ✓? | ? | – | ? | **5+** |

### What this matrix reveals

The Σ column is the **cross-vendor leverage score**. Higher Σ = more vendors care = bigger fix multiplier.

**Top-12 surfaces ranked by Σ:**

| Rank | Technique | Σ | Already covered? | Fix effort | Leverage |
|---:|---|--:|---|---|---|
| 1 | TLS JA3/JA4 | 12 | ✓ (chapter 23) but JA4 capture missing | low — chapter 23 §10 acceptance | already-paid |
| 2 | WebGL params | 11 | partial (canvas_bootstrap.js:1290 covers 7/80) | low — 1-day mask sweep | **HIGHEST NEW** |
| 3 | Canvas 2D hash | 11 | partial — per chapter 38 §2.4 | medium — golden snapshot per profile | **HIGH** |
| 4 | HTTP/2 SETTINGS order | 11 | ✓ (boring2-driven) | low — verify Firefox H2 differentiates per chapter 39 | partial-gap |
| 5 | `Function.prototype.toString` masking | 11 | partial (chapter 16 §5 mask sweep plan) | low — 2 days sweep | **HIGH** |
| 6 | `navigator.webdriver` + UA + plugins coherence | 11 | ✓ (stealth profiles) | maintain | already-paid |
| 7 | `performance.now()` granularity | 10 | ✓ (chapter 40 §2.6 — humanized op) | already shipped; needs `timeOrigin` wired | wire-up |
| 8 | HTTP/2 pseudo-header order | 10 | ✓ (boring2 + h2 0.5.17) | verify per profile | maintain |
| 9 | ALPN order | 10 | ✓ (boring2) | maintain | already-paid |
| 10 | Mouse trajectory | 9 | partial (humanize.js, fixed Bezier) | medium-high — Σ-Λ Plamondon impl | **wiring + improvement** |
| 11 | PoW computation | 9 | ✓ (V8 executes) | not a gap — fingerprint-gate is | indirect |
| 12 | Worker-context fingerprint | 9 | NOT AUDITED | medium — chapter 16/41 audit | **HIGH RISK gap** |

## 2. The leverage hierarchy (cross-vendor fix ROI)

Same data, sorted by "fixes per vendor moved":

### Tier S — single fix moves 10+ vendors

| Fix | Vendors moved | Effort | Source |
|---|--:|---|---|
| WebGL prototype mask sweep (currently 7/80 methods) | 11 | 1-2 days | 38 §5.4, 16 §5 |
| WebGL per-profile param golden snapshot | 11 | 1 day | 38 §5.5 |
| `Function.prototype.toString` mask sweep | 11 | 2 days | 16 §5, 08 Lever 3 |
| Canvas `toDataURL` golden parity test | 10 | 1-2 days | 38 §5.6 |

### Tier A — single fix moves 5-9 vendors

| Fix | Vendors moved | Effort | Source |
|---|--:|---|---|
| Wire keystroke generator (Rust exists, JS doesn't call) | 8+ | 1-2 days | 40 §3.2, 26 §3 |
| Worker-context fingerprint audit + fix | 9+ | 3-5 days | 41 §4.4, 16 §6 |
| Mouse curve Σ-Λ Plamondon impl | 9 | 5-10 days | 40 §3.1 |
| Wire `performance.timeOrigin` to humanized op | 8 | 0.5 day | 40 §2.6 |
| Wire two-level seed (Rust exists) for per-session coherence | 8+ | 1 day | 40 §5 |
| RAF jitter (currently 16ms deterministic) | 7+ | 1 day | 40 §2.3 |

### Tier B — single fix moves 2-5 vendors

| Fix | Vendors moved | Effort | Source |
|---|--:|---|---|
| `MessageChannel`/`MessagePort` proper impl | 6+ (+ unblocks duolingo!) | 3-5 days | 17, 41 §4.4 |
| Touch event synthesis on iPhone/Pixel | 6 | 5 days | 40 §3.4 |
| WebRTC mDNS proper handling (already at `window_bootstrap.js:4983-4996` — verify) | 5 | verify | 39 §4 |
| Audio DynamicsCompressor -50dB calibration (currently 16% off) | 4+ | 5-7 days | 38 §3.3 |
| Firefox H2 differentiation (currently emits Chrome H2 even on firefox profile) | (per-profile) | 2 days | 39 §3 |

### Tier C — vendor-specific (chapter-bound)

| Fix | Vendors / sites | Effort | Source |
|---|---|---|---|
| AWS WAF fingerprint bisection + close gates | amazon-de/in/com-au/jp/imdb | 2-4 weeks | 06 |
| Cloudflare `cf-mitigated` detection | iphone 6 sites (quality, not Pass) | 1 hour (literally) | 25 |
| DataDome primitive restoration (relax CSP + iframe materialize + cookie retry) | etsy + tripadvisor + yelp | 1-2 weeks | 07 |
| Kasada K2-DIFF + 16-field error fixes | canadagoose + hyatt + realtor | 1-3 months | 08 |

## 3. Cross-vendor patterns

Patterns that recur across 5+ vendors. These are the "Android patterns the iOS dev didn't see before."

### Pattern 1 — "Cross-layer coherence"

Every modern vendor checks that ALL layers tell the same story:
- TLS JA4 says Chrome 148 → HTTP/2 SETTINGS must match Chrome 148 → UA-CH brand must match → JS env (canvas/audio/WebGL) must match.

Camoufox passes because it IS real Firefox at every layer. Playwright fails because the TLS layer says Chrome but `navigator.webdriver=true`. **BO succeeds when every layer is byte-perfect Chrome — we mostly are, but each gap is a vendor's catch.**

**The unique structural argument** (per chapter 39 §7): BO is the ONLY engine that has to build cross-layer coherence from scratch. Competitors get it free from being a real browser. This is the bar we have to clear, and it's high.

### Pattern 2 — "Fingerprint gate before challenge"

For PoW + WASM + JS challenges:
- challenge.js loads, runs early-fingerprint check
- If fingerprint smells wrong → bail SILENTLY (no PoW attempt)
- If fingerprint passes → run PoW, POST result, succeed

**Implication**: V8 capability to RUN the challenge doesn't help if the gate bails first. Fixing capability is fixing 0% of these sites; fixing fingerprint is fixing 100%. (Chapter 06 §3, 41 §2.4.)

### Pattern 3 — "Engineering exists, wiring doesn't"

Multiple times in this research we found Rust generators in `crates/stealth/src/behavior.rs` that JS code doesn't call:
- Keystroke generator (`behavior.rs:421-464`) — never wired
- Two-level seed (`behavior.rs:109-115`) — humanize.js uses `Math.random()` instead
- `performance.timeOrigin` — not wired to humanized op

**Estimated total cost to wire ALL of these: ~5 days.** Estimated vendor lift: 5-8 behavioral-scoring vendors.

### Pattern 4 — "The 4-product family"

Every major vendor sells multiple products:
- AWS WAF: Challenge + CAPTCHA + Bot Control + ATP+ACFP (4)
- Cloudflare: Managed Challenge + Turnstile + Bot Fight + JSC (4)
- DataDome: Web + Account Protect
- Akamai: BMP + Account Protector
- Imperva: WAF + ABP
- F5: BIG-IP ASM + Distributed Cloud Bot Defense

**Implication for our coverage**: detection markers need to identify which PRODUCT is firing, not just which vendor. Chapter 18 cookbook needs vendor + product columns; chapter 14 testing needs per-product harnesses.

### Pattern 5 — "Cross-thread fingerprint identity check"

Vendors with Worker fingerprinting (9 of 12) verify that the WORKER context returns the SAME fingerprint as the main thread. If main says "Chrome 148 Linux" but Worker says "stub" → bot.

**BO status**: chapter 16 documented main-thread coverage; Worker-context coverage NOT audited (chapter 41 §4.4). **This is the biggest unknown in the entire matrix.** A single failing check on a single API in Worker context could explain WHY duolingo/recaptcha fails despite our main-thread API surface looking complete.

### Pattern 6 — "Daily rotation"

Per chapter 33 quarterly probe-rotation log:
- DataDome: keys rotate DAILY
- Akamai: weekly per-tenant
- Cloudflare: weekly
- Kasada: weekly minor / quarterly major

**Implication**: every fix has a half-life. A fix that lands Monday may be stale by Friday. **CI must include nightly capture-diff per chapter 33 § 7** to catch silent drift before customers do.

### Pattern 7 — "Behavioral biometrics is the post-2026 frontier"

8 of 12 vendors deploy behavioral scoring (Σ for mouse + keystroke + scroll + touch combined). Static fingerprints are increasingly defeated by all engines (Camoufox + BO + Patchright). Behavioral is where the new line is being drawn.

**Implication**: ECH (Encrypted ClientHello), Privacy Sandbox, Private State Tokens will collapse network-layer fingerprinting over 2027-2028. Behavioral biometrics will become THE blocker. **BO's humanize.js is the long-term battleground.** Worth investing in even though the wiring gaps are short-term.

## 4. Engine-by-engine cross-cut

What chapter 27 showed in the per-vendor matrix, condensed:

| Vendor cluster | BO routed | Camoufox | PW/Patchright | PW-Stealth | Notes |
|---|--:|--:|--:|--:|---|
| AWS WAF | 4-7 of 8 amazon | 6 of 8 | 1 of 8 | 1 of 8 | BO non-deterministic; PW family blocked |
| DataDome | 0 of 3 | 3 of 3 | 0 of 3 | 0 of 3 | post-strip gap; chapter 07 restoration moves us |
| Akamai BMP | 1 of 3 (adidas firefox-only) | 0 of 3! (!) | **3 of 3** (homedepot) | 3 of 3 | The inversion — PW family WINS this cluster |
| Cloudflare Managed | mostly pass | pass | pass | pass | iphone uniquely loses |
| Kasada | 1 of 5 | 4 of 5 (open SOTA) | 0 of 5 | 0 of 5 | Camoufox's strength |
| PerimeterX (zillow) | **wins** | fails | pass | pass | BO's structural win |
| Behavioral (no specific corpus site) | partial | full | partial | partial | humanize wiring gaps |

**Reads:**
- **The PW family BEATS Camoufox + BO on Akamai homedepot.** That's the strangest finding from chapter 27 — Akamai's homedepot tenant configuration trusts real Chrome enough that CDP-driver detection isn't enough to block. Worth investigating because it implies our chrome-class TLS + UA might be CLOSE but not exact in one specific dimension.
- **Camoufox loses zillow** because PerimeterX has a specific Firefox-detection heuristic. BO wins because we ship Chrome.
- **BO + Camoufox both lose Akamai homedepot** — neither real Firefox nor BO chrome-class TLS is enough. There's a SPECIFIC Akamai-tenant-config detection happening here. Capture + diff needed.

## 5. The cross-layer coherence thesis (chapter 39 §7 expanded)

The single load-bearing argument for BO's strategic moat:

```
Coherent fingerprint = vendor cannot distinguish bot from real browser
─────────────────────────────────────────────────────────────────────
TLS(JA4)  ∧  HTTP/2(SETTINGS)  ∧  UA-CH(brand list)  ∧  JS-env(everything) → coherent
                                                                            
Camoufox achieves: REAL Firefox at every layer (no work)
Playwright family achieves: REAL Chromium core + adds detected signals (CDP, webdriver, runtime.evaluate)
BO achieves: byte-perfect Chrome 148 at TLS+HTTP/2 (boring2) + Chrome-shape JS via deno_core
            ↑ This is the work product. Each chapter is one piece.
```

**Why this is the moat**: Cross-layer coherence is hard. Many open-source attempts fail. BO's net+stealth+js stack is the closest non-real-browser implementation. Camoufox's REAL-Firefox approach is the alternative — and Firefox itself loses some sites to vendors that distrust Firefox (zillow, possibly homedepot variants).

**The implication for product positioning**: BO is the bet that the FUTURE of scraping is "engineered Chrome coherence" not "wrapped real Firefox." When ECH and Privacy Sandbox kill the network-layer signal, only the JS-env + behavioral layers will matter — and that's exactly where BO is competing.

## 6. Customer-perspective synthesis

By customer use case (per chapter 22 production deployment), what to recommend:

| Customer says | Recommend | Why |
|---|---|---|
| "I scrape amazon" | BO routed 4-profile | AWS WAF non-determinism — routing wins |
| "I scrape news + SaaS" | BO single chrome | parity sites; cheapest |
| "I scrape e-commerce (Akamai-protected)" | BO firefox profile FIRST | adidas wins; chapter 11 routing |
| "I scrape real-estate (Kasada)" | Camoufox or wait for K2-DIFF | open frontier per chapter 08 |
| "I scrape login/auth flows" | Not BO yet | ATO vendors (chapter 36) need solver |
| "I need lowest cold-start" | BO (in-process) | 0 launch overhead vs Playwright 2.4s |
| "I run at scale (10M+ pages/month)" | BO pool path | 4× less RAM than Playwright tree |
| "I need 100% pass" | No engine | Even Camoufox is 113/126; hard residual exists |

## 7. Forward 2-year forecast

Where the field is going through 2027:

### Network-layer fingerprinting will erode
- **ECH (Encrypted ClientHello)** rolling out (Chrome 117+, Firefox 119+ stable). When this hits 50%+ deployment, JA4-by-SNI dies.
- **HTTP/3 + QUIC** taking share; new fingerprint surface (QUIC SETTINGS, transport params) less mature
- **Privacy Sandbox PAT (Private Access Token)** — Apple-Google-Cloudflare push to replace CAPTCHA with attestation token

### Behavioral biometrics will rise
- Per chapter 40 §7 forecast: ML detection budget growing faster than synthesis
- Convergence: only behavioral-undetectable bots survive
- Solution: high-fidelity human-model generators (the humanize.js post-wiring + post-improvement story)

### WebGPU will be the new WebGL fingerprint
- Chrome 113+ ships WebGPU
- New API surface = new fingerprint vectors (GPU adapter info, shader compilation)
- Vendors will add WebGPU probes within 2026

### WebNN may become the new fingerprint
- Web Neural Network API in development
- Allows on-device ML inference fingerprinting

### Anti-bot vendor consolidation continues
- Imperva → Thales (2023)
- Shape → F5 (2020)
- Signal Sciences → Fastly (2020)
- Distil → Imperva (2019)
- ShieldSquare → Radware (2019)
- **Likely 2026-2027 acquisitions**: Castle (smaller, M&A target); Arkose Labs (IPO or acquisition); DataDome (large, IPO-track)

## 8. What this changes about chapters 03-15

The original chapters 03-15 should be re-read through this synthesis lens. Key updates needed (out-of-scope for this chapter but flagged):

- **00 README success scorecard** should add: "WebGL prototype mask sweep complete" + "humanize.js wiring gaps closed" + "MessageChannel implemented" as v0.1.0 must-haves
- **02 Gap analysis** should add: cross-layer coherence verification as a transverse acceptance, not per-site
- **11 Per-profile strategy** should add: per-profile WebGL golden snapshot requirement
- **15 Open questions** Q26.x (chapter 26's open questions about adidas/homedepot) should be elevated — these are STRATEGIC counterexamples to "Camoufox is better"

## 9. Files referenced

### Synthesis sources (every chapter that fed this matrix)
- 06 AWS WAF solver, 07 DataDome primitives, 08 Kasada frontier
- 11 per-profile strategy, 12 competitive landscape
- 16 stealth fingerprint audit, 17 Web API parity matrix
- 18 anti-bot vendor cookbook
- 25 Cloudflare deep, 26 Akamai BMP deep, 27 vendor competitive matrix
- 28 AWS WAF extended, 29 F5/Shape, 30 Arkose, 31 Fastly, 32 Radware
- 33 quarterly probe-rotation log, 34 per-vendor test harness
- 35 Imperva ABP, 36 ATO specialists, 37 Reblaze/Sucuri/DD-AP
- 38 visual/audio fingerprinting
- 39 network-layer fingerprinting
- 40 timing + behavioral biometrics
- 41 PoW + WASM + Worker patterns

### Engine source (file:line) referenced in the matrix
- `crates/js_runtime/src/js/canvas_bootstrap.js:1290-1295` (WebGL mask current coverage)
- `crates/js_runtime/src/js/window_bootstrap.js:2256-2272` (MessageChannel stub)
- `crates/js_runtime/src/js/window_bootstrap.js:4983-4996` (WebRTC mDNS)
- `crates/stealth/src/behavior.rs:109-115` (two-level seed, unwired)
- `crates/stealth/src/behavior.rs:421-464` (keystroke generator, unwired)
- `crates/browser/src/js/humanize.js` (the wiring layer that doesn't wire)
- `crates/js_runtime/src/extensions/perf_ext.rs` (humanized perf.now — needs timeOrigin)
- `crates/net/src/tls.rs` (boring2 4.15.15 — chapter 23)
- `crates/browser/src/page.rs:1049-1057` (current vendor-detect markers)
- `crates/browser/src/page.rs:2273-2293` (body-content vendor markers)

### Sweep baseline (the data behind every Σ above)
- `~/projects/browser_oxide_internal/benchmarks/baselines/2026-05-24/` (per chapter 14)
- Per-engine pass-rate validates the matrix predictions chapter-by-chapter

## How to use this synthesis

This is the strategic-prioritization input for chapter 43. Read this first to understand WHICH technique to invest in; read 43 to understand WHEN + IN WHAT ORDER.

If you only have time for ONE chapter from the whole release plan, read this one and chapter 43.
