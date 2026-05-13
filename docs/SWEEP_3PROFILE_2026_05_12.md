# 3-Profile Parallel Sweep Comparison — 2026-05-12

First empirical measurement of the 2026-05-12 mobile profiles (Pixel 9 Pro Chrome 147 + iPhone 15 Pro Safari 18) against the existing desktop baseline (Chrome 130 macOS) on the 126-site corpus.

## Headline numbers

| Profile | L3-RENDERED | Blocked / flake | Net vs desktop |
|---|---:|---:|---:|
| **chrome_130_macos** (baseline desktop) | **114 / 126** | 12 | — |
| **pixel_9_pro_chrome_147** (Android) | **117 / 126** | 9 | **+3** |
| **iphone_15_pro_safari_18** (iOS) | **105 / 126** | 21 | **−9** |

**Android delivers a real win** (+3 sites). **iOS profile is a regression** (−9 sites) — confirms the audit warning that a partial mobile profile is worse than honest desktop. Per-block detail below.

## Block breakdown by profile

| Block category | Desktop | Android | iOS |
|---|---:|---:|---:|
| L3-RENDERED (success) | 114 | **117** | 105 |
| Kasada-CHL | 3 | 3 | 3 |
| captcha-CHL | 3 | 1 | 2 |
| THIN-BODY (timing flake) | 2 | 0 | 3 |
| DataDome-CHL | 2 | 2 | **6** |
| Cloudflare-CHL | 1 | 1 | 2 |
| Akamai-CHL | 1 | 1 | 1 |
| PerimeterX-PaH | 0 | 0 | **3** |
| PerimeterX-CHL | 0 | 0 | **1** |
| ERROR | 0 | 1 | 0 |

iOS introduced **4 NEW PerimeterX detections** (3 PaH + 1 CHL — sites that previously didn't probe us at all) and **doubled DataDome blocks** (2 → 6) and Cloudflare (1 → 2).

## Android — per-site deltas vs desktop (+3 net)

**Wins** (desktop blocked, Android passed) — 4 sites:

| Site | Desktop outcome | Android outcome |
|---|---|---|
| azure | THIN-BODY | L3-RENDERED |
| costco | THIN-BODY | L3-RENDERED |
| etsy | **DataDome-CHL** | L3-RENDERED |
| spotify | captcha-CHL | L3-RENDERED |

**Regressions** (desktop passed, Android blocked) — 1 site:

| Site | Desktop outcome | Android outcome |
|---|---|---|
| leboncoin | L3-RENDERED | DataDome-CHL |

**Lateral move** (both blocked, different reason):
- wildberries: captcha-CHL → ERROR

The Android `etsy` win is notable — that's a verified DataDome regression-test site. Conclusion: **Android profile is shippable as-is** for the 4-win set, with leboncoin as a known DataDome-detection regression to investigate.

## iOS — per-site deltas vs desktop (−9 net)

**Wins** (desktop blocked, iOS passed) — 4 sites:

| Site | Desktop outcome | iOS outcome |
|---|---|---|
| azure | THIN-BODY | L3-RENDERED |
| costco | THIN-BODY | L3-RENDERED |
| homedepot | **Akamai-CHL** | L3-RENDERED |
| spotify | captcha-CHL | L3-RENDERED |

**Regressions** (desktop passed, iOS blocked) — 13 sites:

| Site | Desktop outcome | iOS outcome | New detection? |
|---|---|---|:-:|
| airbnb | L3-RENDERED | THIN-BODY | (timing) |
| bestbuy | L3-RENDERED | Akamai-CHL | NEW |
| bloomberg | L3-RENDERED | PerimeterX-CHL | **NEW** |
| leboncoin | L3-RENDERED | DataDome-CHL | NEW |
| quora | L3-RENDERED | Cloudflare-CHL | NEW |
| reuters | L3-RENDERED | DataDome-CHL | **NEW** |
| skyscanner | L3-RENDERED | THIN-BODY | (timing) |
| tripadvisor | L3-RENDERED | DataDome-CHL | **NEW** |
| trulia | L3-RENDERED | PerimeterX-PaH | **NEW** |
| wayfair | L3-RENDERED | PerimeterX-PaH | **NEW** |
| wellsfargo | L3-RENDERED | THIN-BODY | (timing) |
| wsj | L3-RENDERED | DataDome-CHL | **NEW** |
| zillow | L3-RENDERED | PerimeterX-PaH | **NEW** |

8 of the 13 iOS regressions are **vendor escalations** — DataDome / PerimeterX / Cloudflare / Akamai now actively probing or blocking sites that previously rendered cleanly. PerimeterX's "Press and Hold" challenge appears for the first time on **3 sites** (trulia, wayfair, zillow — all real-estate / e-commerce stacks heavily fingerprinted).

## Why iOS regresses

The audit (`docs/RQUEST_MOBILE_TLS_AUDIT_2026_05_12.md`) explicitly warned: *"Don't ship a partial mobile profile. A mobile UA with mismatched stack is more anomalous than honest desktop."* Empirically confirmed.

What we shipped vs what was needed:

| Layer | iOS profile shipped | iOS profile needed | Status |
|---|---|---|---|
| UA string | Mobile/15E148 Safari/604.1 | same | ✓ |
| JS surface | 16 declined APIs stripped, no userAgentData, window.orientation, DeviceMotionEvent.requestPermission, etc. | same | ✓ |
| TLS cipher list | Safari 18 (20 ciphers + 3DES) | same | ✓ |
| TLS sigalgs | Safari (10 entries with duplicate) | same | ✓ |
| TLS curves | X25519 + P-256 + P-384 + P-521, no PQ | same | ✓ |
| TLS cert compression | zlib | same | ✓ |
| TLS session_ticket | absent (NO_TICKET) | same | ✓ |
| TLS ECH | absent | same | ✓ |
| TLS ALPS | absent | same | ✓ |
| TLS extension order | BoringSSL default (no shuffle) | Safari's specific fixed order | **partial** |
| TLS padding extension | auto when ClientHello > 512B | guaranteed at tail | **deferred** |
| TLS 1.0 record version (0x0301) | 0x0303 (BoringSSL default) | 0x0301 | **deferred per audit** |
| HTTP/2 SETTINGS values | Chrome's `1=65536, 2=0, 4=6291456, 6=262144` | Safari's `2=0, 3=100, 4=2097152, 9=1` | **NOT DONE** |
| HTTP/2 SETTINGS order | Chrome's | Safari's | **NOT DONE** |
| HTTP/2 pseudo-header order | "masp" (Chrome) | "msap" (Safari) | **NOT DONE** |
| HTTP/2 INITIAL_WINDOW_UPDATE | 15663105 (Chrome) | 10420225 (Safari) | **NOT DONE** |
| HTTP request headers | Chrome-shaped (Accept, Accept-Encoding, Accept-Language, sec-ch-ua-* — but iOS branch strips sec-ch-ua-*) | Safari-shaped (no sec-fetch-*, different Accept order, etc.) | **NOT DONE** |

The 5 "NOT DONE" items in the HTTP/2 + headers layer are what's killing iOS. PerimeterX's behavioural fingerprinter and DataDome's HTTP/2 fingerprint both score these heavily — and we currently advertise iOS at the TLS+JS layer but Chrome at the HTTP/2 layer. That's a mismatch real iOS Safari never produces.

## Recommended next steps

### Option A — keep Android, drop iOS (fastest)

Mark `iphone_15_pro_safari_18` as experimental / not-for-production. Ship `pixel_9_pro_chrome_147` as the canonical mobile profile. New baseline: **117/126** (Android beats desktop and is consistent end-to-end since Chrome Android shares the desktop HTTP/2 + headers stack).

Effort: 0 days. Just remove iOS from the production preset list.

### Option B — finish iOS HTTP/2 + headers (~2 days)

Land the 5 NOT-DONE items:
1. Add `safari_headers()` builder in `crates/net/src/headers.rs` (drop sec-fetch-*, change Accept order, no `priority` header, no `upgrade-insecure-requests`)
2. Parameterize `h2_client.rs` SETTINGS by device_class (Safari sends 4: `2=0, 3=100, 4=2097152, 9=1`)
3. Parameterize `h2_client.rs` pseudo-header order ("msap" for iOS)
4. Parameterize INITIAL_WINDOW_UPDATE (10420225 for iOS)
5. Branch `chrome_headers_with_accept_ch` → `dispatch_headers(profile)` that calls Chrome or Safari builder based on browser_name

Then re-sweep. Expected outcome: iOS recovers most/all the 13 regressions, lifts to ~115-117.

### Option C — defer iOS, focus on Android wins (~1 day)

Take Option A baseline (117 Android). Investigate the 1 known regression (leboncoin DataDome). Try `wildberries` ERROR root cause. If wildberries recovers, baseline → 118. Diminishing returns past that.

### Option D — push for iOS parity (~5 days, full Phase 3)

All of Option B, plus:
- Vendor-patch BoringSSL for TLS 1.0 record version (0x0301)
- Raw extension injection for guaranteed padding extension at tail
- Capture real iOS Safari headers from a fresh device, diff against ours
- Add iOS-specific `accept-language` ordering (iOS uses `;q=0.9` after the second token, not the first)

Expected outcome: iOS profile genuinely competitive with Android, may exceed it on PerimeterX-heavy sites that score mobile favorably.

## Reproducibility

All three sweep logs preserved at `/tmp/sweeps/{desktop,android,ios}.log` for this session. Re-run any with:

```bash
BOXIDE_PROFILE=<preset_name> cargo test --release -p browser \
    --test holistic_sweep holistic_sweep_parallel \
    -- --ignored --nocapture | tee /tmp/sweeps/<name>.log

grep -E "^holistic-end:" /tmp/sweeps/<name>.log | awk '{print $5}' | sort | uniq -c | sort -rn
```

Profile names: `chrome_130_macos`, `chrome_130_windows`, `chrome_130_linux`, `firefox_135_macos|windows|linux`, `pixel_9_pro_chrome_147`, `iphone_15_pro_safari_18`.

## Headline takeaway

The empirical 3-profile sweep validates the synthesis-doc hypothesis: **mobile is a real lever, but only when shipped consistently end-to-end**.

- **Android +3 sites** with ~half a day of work (Phase 1 + Phase 2) — confirmed shippable.
- **iOS −9 sites** with ~3 days of work (Phase 3) — confirmed *not* shippable in current state because HTTP/2 layer still Chrome-shaped.

The Phase 3 work shipped TLS + JS surface for iOS but skipped the HTTP/2 layer; that gap is what causes the regression. Either finish Option B (~2 days more) or hide iOS as experimental.

---

## Update — Phase B + D extension permutation results (later same session)

After landing Option B (HTTP/2 + headers) + Option D bonus (Safari extension permutation), re-ran iOS sweep:

| Profile | L3-RENDERED | Δ vs desktop |
|---|---:|---:|
| chrome_130_macos (desktop) | 114/126 | — |
| pixel_9_pro_chrome_147 (Android) | 117/126 | **+3** |
| iphone_15_pro_safari_18 **v1** (Phase 3 TLS only) | 105/126 | −9 |
| **iphone_15_pro_safari_18 v3** (Phase 3 + Phase B + Safari ext perm) | **115/126** | **+1** |

### iOS v1 → v3 per-site changes

**11 recoveries** (all the regressions Phase B was designed to fix):

| Site | v1 | v3 |
|---|---|---|
| airbnb | THIN-BODY | L3-RENDERED |
| bloomberg | PerimeterX-CHL | L3-RENDERED |
| etsy | DataDome-CHL | L3-RENDERED |
| reuters | DataDome-CHL | L3-RENDERED |
| skyscanner | THIN-BODY | L3-RENDERED |
| tripadvisor | DataDome-CHL | L3-RENDERED |
| trulia | PerimeterX-PaH | L3-RENDERED |
| wayfair | PerimeterX-PaH | L3-RENDERED |
| wellsfargo | THIN-BODY | L3-RENDERED |
| wsj | DataDome-CHL | L3-RENDERED |
| zillow | PerimeterX-PaH | L3-RENDERED |

**1 new regression**: h-m.com (Hong Kong H&M) flipped L3 → ERROR. Likely an interaction with one of the H2 changes; needs investigation.

### iOS v3 vs Desktop: per-site delta

iOS wins (4 sites): azure, costco, etsy, homedepot, spotify
Desktop wins (4 sites): bestbuy, h-m, leboncoin, quora
Net: **+1 site for iOS over desktop**.

Phase B (~2 days estimate) delivered as predicted. The audit's diagnosis was correct: HTTP/2 + headers were the bulk of what was killing iOS in v1.

### Total session contribution to mobile profiles

- Android: +3 sites over desktop (117/126)
- iOS: +1 site over desktop (115/126), +10 sites recovered from broken v1
- Both mobile profiles are now production-shippable. Either matches or exceeds desktop.

