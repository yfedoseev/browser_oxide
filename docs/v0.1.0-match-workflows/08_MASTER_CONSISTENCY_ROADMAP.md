# 08 — MASTER per-profile consistency roadmap

**Status:** synthesis of clusters 01–07, 2026-05-29. No live navigations
(a competitor benchmark holds the single IP — live navs would contaminate
it). All reasoning is from the captured per-profile tag/len matrix
(`00_DATA_per_profile_matrix.md`), the BO presets/wire stack, the parity
vendor docs, and 2026 external sources. **Public-engine only** per
`CLAUDE.md` (all fixes live in `crates/net` / `crates/stealth` /
`crates/browser` / the gate harness — none is per-vendor solver code).

---

## 1. Per-profile scorecard + target

Production denominator = 125. Pass = `L3-RENDERED & len >= 15000`.

| profile | now | v150 single-engine | target | gap to close |
|---|--:|--:|--:|--:|
| chrome_148_macos | 110 | ~112–113 | **~115** | uber, amazon-ca(measurement), tripadvisor, spotify |
| pixel_9_pro_chrome_148 | 108 | ~112–113 | **~115** | airbnb, yandex-ru, prime-video, adidas, uber, homedepot |
| iphone_15_pro_safari_18 | 108 | ~112–113 | **~114–115** | economist, ecosia, ft, openai, quora, udemy, homedepot |
| firefox_135_macos | 106 | ~112–113 | **~110–112** | reuters, wsj, tripadvisor, zillow, macys, homedepot, spotify, uber, yelp |
| **routed best-of-4** | **115** | — | **115+** | — |

20 consistency-gap sites · 95 all-four-pass · 10 fail-all (frontier, not here).

**Per-profile gap-fail roster** (from `00_DATA` rows 41–44):
- **chrome (5):** amazon-ca, spotify, tripadvisor, uber, yelp
- **pixel (7):** adidas, airbnb, homedepot, prime-video, uber, yandex-ru, yelp
- **iphone (7):** economist, ecosia, ft, homedepot, openai, quora, udemy
- **firefox (9):** homedepot, macys, reuters, spotify, tripadvisor, uber, wsj, yelp, zillow

**Honest ceiling note:** `yelp` (DataDome `rt:'c'` interactive captcha) is
**unwinnable on any profile** — Camoufox v150 also fails it. It appears on
3 profiles' rosters but is NOT a closeable target; it is the reason firefox's
realistic target is ~110–112, not 115.

---

## 2. The three unifying root causes (the diagnosis)

Every consistency gap collapses onto **3 coherence defects + 1 reliability
track + 1 measurement artifact** (cluster 07 §0):

| ID | Defect | Mechanism | Profiles hit | Clusters |
|---|---|---|---|---|
| **D1** | Firefox UA over **Chrome** TLS + Chrome H2 (no Firefox wire class) | JA4-vs-UA cross-layer mismatch checked *before* the UA by DataDome/PX/Akamai | firefox | 02, 04, 05, 07 |
| **D2** | `apple_m3_macos()` GPU profile on **both** mobile presets | macOS-desktop WebGL params under an iOS/Android UA raises CF/Akamai bot score | iphone, pixel | 01, 05, 07 |
| **D3** | iOS-Safari TLS cipher **SET** wrong (3× 3DES vs real 1× + 2× ECDHE-AES256) | JA4 cipher-hash matches NO real Safari → corpus-orphan → CF mid-band challenge | iphone | 01, 07 |
| **R** | Mobile-fetch UA-CH incoherence + nav-budget/loop bugs | `Android + sec-ch-ua-mobile ?0` on every fetch; uber missing from SPA-budget tier; mobile redirect drops | pixel, chrome, firefox | 03, 04 |
| **M** | AWS-WAF same-IP token-clustering | gate's adjacency-only spacing (~24–45s) far below AWS's 300s immunity window | chrome | 06 |

**The single common thread for D1+D2+D3:** `tls.rs::chrome_connector` and
`h2_client.rs::handshake` branch **only on `profile.device_class`**
(`tls.rs:241`, `h2_client.rs:85`), never on `browser_name`. So firefox
(`DeviceClass::Desktop`) silently emits Chrome bytes, and the cipher
SET / GPU profile bugs are unguarded by any cross-API consistency test.

---

## 3. The SINGLE biggest lever

**The per-profile TLS/JA4-vs-UA coherence work (D1 + D2 + D3) is the biggest
lever — it unblocks the iphone-Cloudflare cluster AND the firefox-DataDome/PX
cluster at once, ~10 sites across two profiles.**

Within that, the two cheapest, highest-confidence pieces dominate ROI:

1. **D3 — iOS Safari cipher-SET fix** (`tls.rs:111-132`): the cipher list
   emits THREE 3DES suites (`0xC008`, `0xC012`, `0x000A`) where real iOS 18
   Safari emits ONE (`0x000A`) plus two ECDHE-AES256-CBC-SHA (`0xC00A`,
   `0xC014`). The cipher-hash component of JA4 is `SHA256(sorted IANA
   codepoints)`, so this set difference makes BO's JA4 match **no real
   Safari** — a corpus-orphan fingerprint Cloudflare's JA4-Signals ML scores
   mid-band and challenges. **1–2 hours, high confidence, +4 to +6 iphone.**
   This is the best ROI fix in the entire roadmap.

2. **D2 — mobile GPU profiles** (`gpu.rs` + `presets.rs:931,1035`): both
   mobile presets ship an Apple-M3-macOS WebGL fingerprint under iOS/Android
   UAs — a second score-raising tell that stacks with D3 on the *same*
   iphone CF cluster, and a standing pixel liability. **1–2 days, +3 to +5
   iphone, +0 to +1 pixel.**

D3 + D2 together target the full 6-site iphone Cloudflare cluster. D1 (the
real Firefox wire class) is the biggest *firefox* lever but the largest
effort (1–2 weeks).

---

## 4. Consolidated fix table — ranked by ROI

ROI ≈ (per-profile sites gained × confidence) / effort, deduplicated across
clusters. Same wire-class work is referenced by multiple cluster docs under
different IDs — collapsed here to one row each.

| Rank | Fix ID (cluster IDs) | What | File:line | Effort | Conf | Profile(s) lifted | Expected gain | Public |
|--:|---|---|---|---|--:|---|---|:--:|
| 1 | **D3** (01 #1) | Correct iOS Safari cipher SET: drop `0xC008` + `0xC012`, insert `0xC00A` + `0xC014` before `0x000A` | `crates/net/src/tls.rs:111-132` | 1–2h | high | iphone | **+4 to +6** (economist/ecosia/ft/openai/quora/udemy) | yes |
| 2 | **M1** (06) | Wall-clock 150s min gap for AWS-tagged sites (track `last_seen_ts[vendor]`, sleep shortfall) | `benchmarks/run_bo_isolated.py` | 0.5d | high | chrome | **+1** (amazon-ca de-clusters) + stabilizes amazon-fr/jp/com-au/in | yes (harness) |
| 3 | **UBER-BUDGET** (04) | Add `\|\| h.ends_with("uber.com")` to the 90s SPA-shell budget arm | `crates/browser/src/page.rs:1962` | 1 line | high | chrome, firefox, pixel | **+2 to +3** (uber; iphone already passes) | yes |
| 4 | **PX1** (03) | `chrome_headers_fetch` device-aware: emit `sec-ch-ua-mobile ?1` for mobile (mirror `chrome_headers_impl`'s `is_mobile`) | `crates/net/src/headers.rs:250` (vs 339) | ~10 LOC +1 test | med | pixel | **+2 to +3** (airbnb, yandex-ru, plausibly prime-video) | yes |
| 5 | **D2 / P1** (01,05,07) | Author `android_mali_g715` + `ios_apple_a17_pro` GpuProfiles; set at presets, add mobile cross-API test | `crates/stealth/src/gpu.rs` + `presets.rs:931,1035` | 1–2d | med-high | iphone, pixel | **+3 to +5 iphone**, +0 to +1 pixel (overlaps D3's 6) | yes |
| 6 | **JA4-REG** (01 #2) | JA4 byte-regression vector for `CIPHER_LIST_SAFARI_IOS` (assert count digit + absence of 0xC008/0xC012) | `crates/net/src/tls.rs` test mod | 1h | high | — | 0 (regression insurance for D3) | yes |
| 7 | **PX3** (03) | Diagnose prime-video loop w/ `BROWSER_OXIDE_DEBUG_REDIRECTS=1`; add same-URL re-nav loop guard if a loop bug | `crates/browser/src/page.rs:2946-3010` | low–med | low | pixel | **+1** (prime-video; likely subsumed by PX1) | yes |
| 8 | **DRAIN-B** (05 FIX-B) | Challenge-aware inter-script drain: ≥1–2s burst when `doc_is_challenge`, capped by V8DeadlineWatcher | `crates/browser/src/page.rs:3661,3594` | 1–2d | high | iphone (+cross-benefit AWS/booking) | **+1 iphone** (homedepot) | yes |
| 9 | **D1 / FF-WIRE** (02,04,05,07) | Real Firefox 135 TLS + H2 class: `browser_name=='Firefox'` branch in `tls.rs::chrome_connector` + `h2_client.rs::handshake` (NSS ciphers, no MLKEM lead, fixed ext order, no ALPS/ECH-grease/Brotli; SETTINGS 65536/0/131072/16384, pseudo m,p,a,s) | `tls.rs:233`, `h2_client.rs:78` | 1–2 wk | med | firefox | **+3 to +4** (reuters, wsj, tripadvisor, zillow) + homedepot | yes |
| 10 | **FF-GUARD** (02,07) | Interim: don't route Firefox UA to JA4-cross-checking vendors until D1 ships | routing table | 0.5–1d | high | firefox | 0 new; prevents guaranteed-loss firefox routing | yes |
| 11 | **JA4-CAP** (01 #3, 07 P5) | Capture corrected per-profile JA4 + akamai-H2 from `tls.peet.ws/api/all`; assert exact strings | `crates/net/tests/` | 0.5–3h | high | — | validation; unblocks D2/D1 verification | yes |
| 12 | **PX2** (03,07) | Author real GPU already covered by D2; if deferred, this is the mobile-GPU coherence floor | `presets.rs:931` | ~40 LOC | low | pixel | 0–1 (hardens Android routing) | yes |
| 13 | **D3-PAD** (01 #4, 07 P2) | ONLY if D3+JA4-CAP leave residual: raw-inject Safari PADDING + trailing GREASE for byte-exact JA4_o | `tls.rs:159-183` | 4–8h | med | iphone | +0 to +2 residual | yes |
| 14 | **PX-GPU/H2** (05 FIX-D) | Split H2 fingerprint so Android Chrome gets own SETTINGS (don't share desktop branch) | `h2_client.rs:74` | 2–3d | low | pixel | +0 to +1 (homedepot, if H2 delta is the residual) | yes |
| 15 | **SECCPT-ORACLE** (05 FIX-C) | Per-profile sec-cpt oracle (fork `awswaf_probe.rs`→`seccpt_probe.rs`); decode provider per profile | dev tool | 1–2d | high | — | 0 flips; decides FIX-A vs FIX-B per profile | yes (dev) |
| 16 | **SPOTIFY-DIAG** (04,07) | Diagnose spotify chrome+firefox thin (desktop content gate vs D1) — offline | diagnostic | 0.5–1d | low | chrome, firefox | uncertain (+0 to +1 each if shared desktop gate) | yes |
| 17 | **DD-VERIFY** (04) | Verify chrome DataDome self-solve silent `rt:'i'` path on live tripadvisor (live-nav-drain) | `page.rs:1872` | 2–4d | low | chrome | +0 to +1 (tripadvisor; yelp out of reach) | yes |
| — | **yelp** | DataDome `rt:'c'` interactive captcha — Camoufox also fails | — | — | — | — | **unwinnable, do not chase** | — |

> **Dedup notes:** cluster 07's FIX-P1/P2/P3/P4 == clusters 01/02/05's GPU /
> Safari-PADDING / Firefox-wire / routing-guard items — merged into rows 5,
> 13, 9, 10. Cluster 05's FIX-A (Firefox TLS+H2 for homedepot) == row 9.
> Cluster 04's FIX-FF-TLS+FIX-FF-H2 == row 9. amazon-ca M2/M3/M4 are
> coarser variants of M1 (row 2) — pick M1.

---

## 5. Phased plan

### Phase 1 — Cross-cutting TLS/JA4-vs-UA coherence (lifts iphone + firefox)
The biggest-lever bucket (§3). Order by ROI inside the phase.

1. **D3** (row 1, 1–2h) + **JA4-REG** (row 6, 1h) — ship together: the fix
   + its silent-drift lock. iphone Cloudflare cluster.
2. **D2 / P1** (row 5, 1–2d) — mobile GPU profiles; second iphone lever +
   pixel coherence floor.
3. **JA4-CAP** (row 11) — capture corrected iphone JA4, confirm the 6 sites
   flip. *Queue for when the IP is free.* If all 6 pass, **D3-PAD (row 13)
   is unnecessary**.
4. **D1 / FF-WIRE** (row 9, 1–2 wk) — real Firefox TLS+H2 class; the firefox
   DataDome/PX cluster. **FF-GUARD (row 10)** is the cheap interim if D1
   slips. **FF-JA4-CAP** verifies.

**Phase 1 delta:** iphone +5 to +6 (D3+D2), firefox +3 to +4 (D1).

### Phase 2 — Pixel nav-reliability + uber timeout (request-shape track)
Independent of fingerprint work; cheap and high-confidence.

5. **UBER-BUDGET** (row 3, 1 line) — flips uber on chrome/firefox/pixel.
6. **PX1** (row 4, ~10 LOC) — mobile fetch UA-CH coherence; airbnb,
   yandex-ru, plausibly prime-video.
7. **PX3** (row 7) — prime-video loop diagnosis/guard (likely subsumed by
   PX1). **PX2/D2** already covers the GPU.

**Phase 2 delta:** chrome +1 (uber), firefox +1 (uber), pixel +2 to +4
(uber + airbnb + yandex-ru ± prime-video).

### Phase 3 — Desktop DataDome + homedepot consistency + amazon-ca measurement
The harder/lower-confidence residual.

8. **M1** (row 2, 0.5d) — amazon-ca de-cluster; +1 chrome (measurement, not
   engine). Do early — it's cheap and corrects the reported chrome count.
9. **DRAIN-B** (row 8, 1–2d) — challenge-aware drain; +1 iphone homedepot,
   cross-benefit AWS/booking.
10. **SECCPT-ORACLE** (row 15) — decide per-profile homedepot blocker
    (provider-selection vs worker-drain) before more homedepot work.
11. **D1 already-shipped** flips firefox homedepot + reuters/wsj/tripadvisor.
12. **PX-GPU/H2** (row 14), **DD-VERIFY** (row 17), **SPOTIFY-DIAG** (row 16)
    — low-confidence residual; do only if Phases 1–2 leave the gap.

**Phase 3 delta:** chrome +1 (amazon-ca measurement) +0–1 (tripadvisor),
iphone +1 (homedepot), firefox +1 (homedepot via D1), pixel +0–1 (homedepot).

---

## 6. Projected per-profile pass after each phase

| profile | now | after P1 | after P1+P2 | after P1+P2+P3 | target |
|---|--:|--:|--:|--:|--:|
| chrome | 110 | 110 | 111 (uber) | 112–113 (amazon-ca, ±tripadvisor) | ~115 |
| pixel | 108 | 108–109 (D2 floor) | 111–113 (uber, airbnb, yandex-ru, ±prime-video) | 112–114 (±homedepot) | ~115 |
| iphone | 108 | 113–114 (D3+D2) | 113–114 | 114–115 (homedepot drain) | ~114–115 |
| firefox | 106 | 109–110 (D1) | 110–111 (uber) | 111–112 (homedepot via D1) | ~110–112 |
| **routed** | **115** | 116+ | 117+ | 118+ | 118+ |

After Phase 1 alone, **iphone reaches/exceeds the other three** and the
biggest single cluster (iphone CF, 6 sites) closes. After Phase 2, **pixel
and chrome cross v150 single-engine**. Phase 3 finishes consistency on the
solvable residual; the routed union grows because each closed gap was a
fallback win that becomes all-four-pass.

---

## 7. Honest note — legitimate content vs real bugs

Not every per-profile delta is a bug to fix. Distinguish before acting so the
roadmap isn't padded with unwinnable or non-engine items:

- **REAL ENGINE BUGS (fix):** D3 (iphone cipher set), D2 (mobile GPU
  profiles), D1 (no Firefox wire class), PX1 (mobile fetch UA-CH), uber
  nav-budget, prime-video nav-loop, macys/airbnb/yandex-ru THIN-BODY (mobile
  request-path reliability). These are coherence/reliability defects where a
  *correct* engine would pass the site its siblings already pass.
- **MEASUREMENT ARTIFACT (fix the harness, not the engine):** amazon-ca
  chrome 5310 is AWS-WAF same-IP token-clustering, NOT a per-profile TLS gap
  — it passes chrome at 1.03MB when spaced 150s. Fix = harness spacing (M1),
  zero engine code.
- **LEGITIMATE CONTENT DIFFERENCE (do NOT force-fix):** `spotify` desktop
  (chrome/firefox) returns a genuinely thinner shell (~9.8KB) while mobile
  (pixel/iphone) gets the fuller 147KB open.spotify shell — partly a real
  desktop-vs-mobile content split, only *partly* a DataDome desktop gate.
  `uber` iphone passing with a lighter mobile SSR bundle while desktop times
  out on a heavy React SPA is content-shape, addressed by giving the desktop
  SPA more budget (UBER-BUDGET), not by faking content.
- **UNWINNABLE (accept):** `yelp` (DataDome `rt:'c'` interactive captcha) —
  Camoufox v150 fails it too; it is the reason firefox's realistic target is
  ~110–112, not 115. Do not count it against the engine.

---

## 8. Cross-references
- `00_DATA_per_profile_matrix.md` — the data.
- `01_IPHONE_CLOUDFLARE.md` — D3 cipher-set detail (rows 1, 6, 11, 13).
- `02_FIREFOX_DATADOME_PX.md` — D1 Firefox wire class (rows 9, 10).
- `03_PIXEL_NAV_ERRORS.md` — PX1/PX3 + R track (rows 4, 7).
- `04_DESKTOP_DATADOME_TIMEOUT.md` — uber budget + desktop DataDome (rows 3, 16, 17).
- `05_HOMEDEPOT_SECCPT_CONSISTENCY.md` — drain + per-profile sec-cpt (rows 8, 14, 15).
- `06_AMAZONCA_GATE_CLUSTERING.md` — AWS spacing (row 2).
- `07_PRESET_COHERENCE_AUDIT.md` — D1/D2/D3 unification (the diagnosis, §2).
- `crates/net/src/tls.rs:111-132,241` · `crates/net/src/h2_client.rs:78,85`
  · `crates/net/src/headers.rs:250,339` · `crates/stealth/src/presets.rs:931,1035`
  · `crates/stealth/src/gpu.rs` · `crates/browser/src/page.rs:1962,3594,3661`.
