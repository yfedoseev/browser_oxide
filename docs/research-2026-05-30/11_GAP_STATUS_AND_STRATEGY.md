# v150-Gap status & strategy — 2026-05-30 (post SSR-preservation)

**Authoritative cold gate (NOT pooled): BO = 112/126. v150 = 116/126. Net gap = 4.**
Run: `docs/benchmarks/runs/2026-05-30_chrome148macos_COLD_v2.json`.

## This session's engine wins (+2, +1 stabilized)
- **shopify** 0 → 420089, **mail-ru** 0 → 440343 — SSR-preservation fix (commit d2e0554): React-Router/Remix inline module hydrates, wipes the 400KB+ SSR body, fails to rebuild headless → we now restore the server DOM.
- **wsj** stabilized (678KB SSR restore) — same bug, prevents a flake.

## Remaining v150-gap (v150 passes, BO fails) — 8 sites, by class

### A. DataDome "Device Check" WASM — vendor passive-sensor (etsy, tripadvisor, reuters)
All three serve a `geo.captcha-delivery.com` iframe titled **"DataDome Device Check"** (`allow=accelerometer; gyroscope; magnetometer` — a device-sensor fingerprint check). i.js fetches OK; the H2→http/1.1 ALPN log on geo.captcha-delivery is **cosmetic** (the HTTP/1.1 fallback at `net/src/lib.rs:699` already handles it). etsy's risk score is **LOW (0.079)** — DataDome would pass it IF the Device Check WASM completed with a human-looking fingerprint. It's a PASSIVE sensor challenge (Kasada/Akamai-class), not an interactive captcha — engine-addressable only via fingerprint quality (the holistic-ML tail, no single lever). reuters is **flaky** (rendered 1.2MB clean on a re-probe — DataDome risk-rolls the challenge).
**→ Realistic lever: the Firefox profile (#43).** v150 IS camoufox/Firefox; its REAL fingerprint clears the Device Check. This is why all three are v150-passes-BO-fails.

### B. reCAPTCHA-enterprise + CSR mount failure — research-grade (spotify, duolingo)
React app (#main / #root) never mounts; invisible reCAPTCHA enterprise present but NOT the gate (v150 passes without solving it). spotify burns the full 25s build budget churning many small scripts with no single >500ms script — the React bundle never completes the client render headless. Server HTML is a thin shell (no SSR to restore). Needs deep CSR-completion research (missing API / event / worker dependency).

### C. Akamai + code-split lazy chunks (adidas)
Mount stays a 2.5K shell; lazy chunks (IntersectionObserver / unref'd timers) never load headless; 115s nav. Hard.

### D. Interactive / proprietary vendor — out of public-engine scope (wildberries, douyin)
wildberries = WBAAS 498 (now ERROR/flaky); douyin = ByteDance slide-captcha. Per CLAUDE.md, vendor bypass lives in the private `vendor_solvers` crate, not public crates.

## BO edge (BO passes, v150 fails) — 4 sites
amazon-ca, amazon-com, leboncoin, yelp. (Caveat: 8 amazon TLDs run back-to-back → AWS-WAF token-clustering can false-fail some in-gate.)

## Strategy to beat v150 (need ≥117, i.e. +5 from 112)
1. **#43 Firefox TLS+H2 profile (highest leverage).** v150's entire advantage on the remaining gap is the Firefox real-browser fingerprint clearing passive sensor challenges (DataDome Device Check, possibly reCAPTCHA-scored sites). A correct Firefox profile could flip the DataDome cluster (+3) and maybe spotify/duolingo — plausibly +3..+5. Constraint: boring2/BoringSSL Firefox TLS arm is non-trivial infra.
2. CSR-completion research (spotify/duolingo) — separate, deep.
3. Accept B/D (reCAPTCHA interactive tail, vendor captchas) as scope-bounded.

**Conclusion:** the clean public-engine render bugs are now fixed (SSR cluster). The remaining gap is dominated by passive vendor-sensor challenges whose single realistic lever is the Firefox profile (#43), not per-site engine patches.

---

## UPDATE 2 — post Firefox-wire-arm + readyState fix (final session state)

**chrome∪firefox UNION = 113/126 vs v150 116. Net gap = 3.**
Runs: `2026-05-30_chrome148macos_COLD_v2.json` (112), `2026-05-30_firefox135macos_COLD_wirearm.json` (108).

### Engine fixes shipped this session (union 110 → 113)
- **SSR-preservation** (d2e0554): shopify 0→420K, mail-ru 0→440K, wsj stabilized. +2.
- **Firefox TLS+H2 wire arm** (e6923d5): firefox profile now emits a real NSS ClientHello (JA4 t13d1516h2→t13d1715h2, cipher-hash exact; akamai_h2 canonical Firefox). tripadvisor DataDome flipped under it. +1 union.
- **readyState fix** (f0a4334): document.readyState was stuck at 'loading' for EVERY navigated page (build path never advanced it). Fixed → spotify's 25s readyState-spin collapsed to 3s; spec-correct lifecycle. Correctness, no flip.
- **warm-pool undercount** documented: pooled gate under-counts challenges; SOTA must be cold.

### The remaining 7 union-fails are ALL vendor-challenge-bound (the genuine ceiling)
| site | challenge | class |
|---|---|---|
| etsy, reuters | DataDome "Device Check" WASM | passive sensor fingerprint |
| spotify, duolingo | reCAPTCHA-enterprise scoring + CSR | passive telemetry score (invisible reCAPTCHA, execute-ms=30000; app gates mount on the token) |
| adidas | Akamai BMP sensor | passive sensor fingerprint |
| douyin | ByteDance slide-captcha | interactive captcha (out of scope) |
| wildberries | WBAAS 498 | proprietary vendor (out of scope) |

These pass in v150 because **camoufox IS real Firefox** — its genuine browser fingerprint clears the passive sensor/telemetry checks (DataDome Device Check, reCAPTCHA-enterprise scoring, Akamai BMP). BO emulates the browser and carries residual fingerprint tells. Per CLAUDE.md the vendor solvers live in the private `vendor_solvers` crate (out of public scope) and were **measured to add 0 net passes** — the from-scratch engine carries the rate.

### Honest verdict
Every CLEAN public-engine render/correctness bug found this session is fixed (SSR, Firefox wire, readyState, warm-pool). BO union 113 vs v150 116. Closing the final 3 requires either (a) open-ended per-vendor fingerprint quality work (the "holistic ML tail, no single lever" the 2026-05-16 research already concluded), or (b) vendor solvers (out of public scope). Beyond the from-scratch engine's reach without crossing the project's scope boundary.

BO's standing advantages over v150 remain: ~25-60× lighter memory, no-CDP in-process architecture, and 4 BO-edge sites v150 fails (amazon-ca, amazon-com, leboncoin, yelp).

---

## FINAL (clean gate v7, 2026-05-31)

**BO UNION = 113/126 (chrome 111 ∪ firefox-wire-arm 108) vs camoufox v150 = 116. Gap = 3.**
Run: `docs/benchmarks/runs/2026-05-31_chrome148macos_COLD_v7_clean.json` (no crash, full 126).

### Confirmed engine-driven flips this session (vs the 110 starting baseline)
- **shopify, mail-ru, wsj** — SSR-preservation (d2e0554)
- **tripadvisor, leboncoin** — Firefox TLS+H2 wire arm (e6923d5); firefox-unique DataDome flips

### Robustness/process-abort bugs found by running the gate (would crash production)
- DOM arena `panic!` → non-unwinding FFI abort (c9ea80e) — wellsfargo 8.28MB
- readyState navigate-loop guard → GTM/OneTrust runaway OOM (a10a035/aa96337) — zoom
- sync-fetch cache reverted (aa96337) — was unnecessary (guard-removal + 30-cap suffice) and risked stale challenge bodies

### Remaining 6 (v150 passes, union fails) — vendor-bound or flaky
- **adidas, homedepot** — flaky Akamai (risk-roll; pass on good rolls, e.g. homedepot passed v2/989KB, adidas 1.48MB on 4 earlier runs). Not deterministically fixable engine-side.
- **etsy** — DataDome Device Check WASM (vendor passive sensor)
- **duolingo, spotify** — reCAPTCHA-enterprise telemetry score (vendor)
- **wildberries** — WBAAS 498 (proprietary vendor)

### BO edge (union passes, v150 fails): amazon-ca, amazon-com, leboncoin.

### Verdict
Every clean public-engine bug is fixed; the engine is crash-free and stable. The 3-site gap to v150 is vendor passive-sensor/captcha challenges (out of public-crate scope per CLAUDE.md — vendor_solvers measured +0 net) plus Akamai challenge-site flakiness (±2-3 between gates). BO's durable advantages over v150 stand: ~25-60× lighter memory, no-CDP in-process architecture, 3 BO-edge sites.
