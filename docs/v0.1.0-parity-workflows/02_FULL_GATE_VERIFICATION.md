# 02 — Full-set verification: browser_oxide vs all competitors (CLEAN-ROOM)

**Date:** 2026-05-30. **Status: corrected & honest.** An earlier version of this
doc reported "BO leads v150 by +16" — that was wrong, caused by tooling artifacts
in the competitor measurements (§3). After fixing them, **camoufox v150 leads BO
on raw site-pass.** BO's real advantages are memory and the no-CDP architecture
(§5), not pass-rate.

**Method:** every engine, all 126 corpus sites, run **one site at a time**,
strictly sequential, memory-gated (>6-7 GB free per launch) with retries.
Harness: `benchmarks/run_cleanroom.py` + a **patched playwright-firefox driver**
(`benchmarks/patch_playwright_ff_driver.sh`, §3). Classified with BO's own
`classify_stdin`. Pass = `L3-RENDERED && len >= 15000`; denominator 125.
Raw data: `docs/benchmarks/runs/` (gitignored).

---

## 1. Final standings (fair)

| engine | **PASS /125** | ERR | notes |
|---|--:|--:|---|
| **camoufox v150** | **116** | 0 | leads on pass-rate |
| **BO routed best-of-4** | **113** | — | union of the 4 BO profiles |
| camoufox v135 | 113 | 0 | |
| **BO iphone_15** | 111 | 1 | BO best single profile |
| **BO pixel_9** | 110 | 0 | |
| **BO chrome_148** | 107 | 0 | |
| **BO firefox_135** | 105 | 1 | |
| playwright_stealth | 89 | 3* | *H2 fairness un-probed (§4) |
| patchright | 89 | 3* | |
| playwright | 88 | 3* | |

**camoufox v150 (116) > BO best single (111) by +5, > BO routed best-of-4 (113)
by +3** — and with a *shorter* settle (12 s) than BO's adaptive 15-90 s budget,
so it is not getting a time advantage. The goal of "outperform Camoufox v150 on
site-pass" is **not met**.

---

## 2. The two axes — state both, don't conflate

**Site-pass (this corpus):** camoufox v150 leads (116 vs BO 113/111).

**Architecture & cost (BO's real, durable wins):**
- **No CDP / in-process V8** — the structural moat. camoufox is CDP/playwright-
  driven and is detectable/unusable where CDP is sniffed; BO is not.
- **Memory ~25-60× lighter** — BO median **78 MB** / max ~200 MB vs camoufox
  Firefox **3-4.8 GB**; BO never OOMs on the 14 GB box, camoufox OOMs past ~2
  concurrent. See `docs/benchmarks/MEMORY_FOOTPRINT.md`.
- **Lower challenge rate than the CDP-chromium tier** — BO draws 7-11 CHALLENGE
  pages vs playwright/patchright's 25.

These are real and matter for deployability/density/cost — but they are not the
same thing as passing more sites.

---

## 3. Why the earlier numbers were wrong (TWO tooling artifacts)

The competitor numbers were depressed by **measurement bugs, not real blocks**:

1. **CPU/OOM contamination (first sweep).** The 2026-05-29 sweep ran at load ~55
   with the box memory-starved; camoufox's Firefox was OOM-killed mid-run
   (`dmesg`: 4.6-4.8 GB kills). The clean-room re-run (one-at-a-time, memory-
   gated) removed this.
2. **playwright-firefox DRIVER BUG (the big one).** Even clean, camoufox showed
   22 (v150) / 26 (v135) `Connection closed while reading from the driver`
   errors. Root cause: when a page fires an uncaught JS error with **no source
   location** (ad/tracker/cross-origin scripts on bing/yahoo/microsoft/cnn/aws/
   …), playwright's Node driver does `pageError.location.url` → `TypeError`, and
   the protocol validator requires `location.url` to be a string →
   `expected string, got undefined` → **the whole driver process crashes**, so
   every later op returns "Connection closed". This is also the same fault
   behind the documented "camoufox driver crashes after 3-30 pages".

   **Fix** (`benchmarks/patch_playwright_ff_driver.sh`): default the missing
   location to `('', 0, 0)`. Verified bing/microsoft/spotify/aws/stripe/yahoo
   all flip CRASH→OK.

   **Impact:** v150 97→**116** (0 err), v135 86→**113** (0 err).

BO's numbers were essentially unaffected by either artifact (in-process, no
driver, never OOMs) — so BO's measurements were trustworthy throughout; it was
the **competitor** numbers that were unfairly low. Fixing them is what flipped
the conclusion.

---

## 4. Known remaining un-probed fairness item (chromium)

All three chromium engines fail the same 3 sites (hotels/costco/washingtonpost)
with `ERR_HTTP2_PROTOCOL_ERROR`, and cnn = THIN-BODY 0 — possibly a harness/H2
artifact (parallel to the camoufox bug) or a real Akamai RST. A diagnostic probe
was scripted (`benchmarks/fairness_requeue.sh`) but **not run** (stopped). If the
chromium failures are our harness, chromium's ~89 is understated by up to ~3-4.
This does not affect the BO-vs-camoufox comparison.

---

## 5. BO fixes this cycle (real engine improvements, verified live)

Commits `446d950..e3c5f92`, validated against target sites:
- **#26 iOS-Safari `compress_certificate`** (JA4 t13d2013h2→t13d2014h2): flipped
  5/6 iphone-only Cloudflare challenges (ecosia, ft, openai, quora, udemy).
- **#28 pixel UA-CH coherence**: airbnb + prime-video; pixel → 110-111.
- **#29 uber SPA budget**: uber renders on pixel + iphone (~700 KB).
- **#23 disk-snapshot** (warm init 1812→261 ms), **#24 worker-leak** (pool RSS
  steady ~77 MB), **#25 parallel runner**, **#30/#31 unforgeable isTrusted**,
  **#32 Σ-Λ mouse engine** — all verified.

Residual BO gaps (real, not tooling): economist (Cloudflare), yandex-ru (pixel),
wildberries (IP-geo), adidas (shell), amazon-in/amazon-com-au (AWS nav-loop, 1
profile each). Firefox wire class (#27), Kasada child-realm (#34), DataDome
cookie-jar (#35) remain documented turnkey items.

---

## 6. Bottom line

On clean, fair, one-at-a-time measurement: **camoufox v150 (116) outperforms BO
(routed 113 / best 111) on site-pass over this corpus.** BO's durable
differentiators are **~25-60× lower memory** and the **no-CDP/in-process
architecture** — not pass-rate. The honest next step to actually close the
site-pass gap is a per-site analysis of where v150 passes and BO does not
(grouped by TLS / JS-API / behavioral / budget cause).

— 2026-05-30, clean-room + driver-fix (`docs/benchmarks/runs/`).
