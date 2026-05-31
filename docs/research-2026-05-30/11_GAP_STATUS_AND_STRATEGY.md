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
