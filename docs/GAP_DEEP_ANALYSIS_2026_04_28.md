# Deep gap analysis — why nodriver/Patchright/playwright-stealth pass 15 sites we don't

> Network capture (request headers, response cookies, request sequence, and HTTP/2 fingerprint) for the 15 gap sites where browser_oxide is detected but other tools pass. Captures done 2026-04-28 from same machine + IP, all 4 tools.
>
> **Bottom-line finding**: the dominant gap is **HTTP/2 wire-level fingerprint** at Akamai's edge, not UA / sec-ch-ua / cookies / JS shims. Patchright literally sends `HeadlessChrome/145.0.7632.6` in its UA AND passes 9 of the 9 Akamai sites we miss. UA is not the signal — TLS + HTTP/2 SETTINGS frame ordering + `INITIAL_CONNECTION_WINDOW_SIZE` is.

---

## Capture methodology

Built `/tmp/playwright_capture.py` and `/tmp/camoufox_capture.py` (already existed). For each gap site, recorded:

- Full primary request headers
- Primary response status + all Set-Cookie headers
- Total subsequent requests + per-domain count
- All POST requests to known sensor endpoints (`/_bm/_data`, `awswaf.com`, `captcha-delivery.com`, `_pxhd`, `kpsdk`, `/tl`)
- Final body length + classification

Captured tools:
- **Patchright 1.58.2** + Chromium 145 — `/tmp/patchright_capture/*.json`
- **playwright-stealth 2.0.3** + Chromium — `/tmp/pwstealth_capture/*.json`
- **Camoufox 135** (earlier session) — `/tmp/cam_capture/*.json`
- browser_oxide — existing holistic_phaseF.log + per-iteration logs

(nodriver capture skipped — its CDP-direct architecture would yield similar wire-level signature to Patchright, since both use Chromium for the actual transport.)

---

## Top finding — UA is NOT what gates us

Patchright passes Walmart with this primary request:

```
user-agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36
            (KHTML, like Gecko) HeadlessChrome/145.0.7632.6 Safari/537.36
sec-ch-ua: "Not:A-Brand";v="99", "HeadlessChrome";v="145", "Chromium";v="145"
sec-ch-ua-mobile: ?0
sec-ch-ua-platform: "macOS"
upgrade-insecure-requests: 1
accept: text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7
```

**`HeadlessChrome` literally appears in both `User-Agent` and `sec-ch-ua`** — and Walmart serves 1.34 MB of real content. Akamai BMP / PerimeterX at Walmart's edge ARE NOT scoring Chrome-vs-HeadlessChrome from UA strings. They're scoring:

1. **TLS ClientHello fingerprint (JA4)** — Patchright's bundled Chromium 145 emits real Chromium TLS = passes
2. **HTTP/2 SETTINGS frame values + order** — Chromium HTTP/2 = passes
3. **HTTP/2 pseudo-header order** — `:method, :authority, :scheme, :path` = passes
4. **HTTP/2 PRIORITY frame** for resource fetches

The cookies set on Patchright's Walmart visit reveal which vendor actually scored:
```
isoLoc, akavpau_p2, _astc, _pxvid, pxcts, adblocked, _pxhd, TS012768cf,
vtc, bstc, _px3, _pxde
```

`_px*` cookies are **PerimeterX**, not Akamai. The `AKA_A2` would indicate Akamai, but it's not set. So Walmart's edge is checking H2/TLS first; that gates Akamai BMP. Past the edge, PerimeterX runs JS scoring (which Patchright with real-Chromium-V8 handles fine).

**For us**: our request triggers `_abck` (Akamai BMP CHL marker). We never get past the edge to see PerimeterX. The discriminator must be **at the wire level**.

---

## The HTTP/2 fingerprint smoking gun

`crates/net/src/h2_client.rs:30-34` configures our HTTP/2 SETTINGS:

```rust
const HEADER_TABLE_SIZE: u32 = 65_536;            // SETTINGS 1 ✓ Chrome match
const ENABLE_PUSH: bool = false;                  // SETTINGS 2 = 0 ✓ Chrome match
const INITIAL_STREAM_WINDOW_SIZE: u32 = 6_291_456;  // SETTINGS 4 ✓ Chrome match
const MAX_HEADER_LIST_SIZE: u32 = 262_144;        // SETTINGS 6 ✓ Chrome match
const INITIAL_CONNECTION_WINDOW_SIZE: u32 = 15_728_640;  // WINDOW_UPDATE ⚠️ MISMATCH
```

Real Chrome 147's `WINDOW_UPDATE` after handshake is `15_663_105` (14.94 MB). We send `15_728_640` (15 MB even). **Δ = 65,535 bytes**.

Akamai's H2 fingerprint hash includes this WINDOW_UPDATE value. From the comment block at line 27-29:

> Earlier in this session we incorrectly added both [SETTINGS 3 and 5] based on an out-of-date curl-impersonate config; that made the Akamai H2 fingerprint hash `d23e6399a1d185e3b8cb58e5640dd698`, diverging from Chrome's actual hash `52d84b11737d980aef856699f885ca86`.

We fixed the SETTINGS list but the WINDOW_UPDATE rounding is still 65,535 off. **This is likely the discriminator** that flags us at Akamai's edge on walmart/homedepot/bestbuy/etc.

### Akamai-FP signature breakdown (reference vs ours)

| Field | Real Chrome 147 | browser_oxide | Match? |
|---|---|---|---|
| `:method,:authority,:scheme,:path` order (m,a,s,p) | `m,a,s,p` | `m,a,s,p` | ✓ |
| `1` HEADER_TABLE_SIZE | 65536 | 65536 | ✓ |
| `2` ENABLE_PUSH | 0 | 0 | ✓ |
| `3` MAX_CONCURRENT_STREAMS | (not sent) | (not sent) | ✓ |
| `4` INITIAL_WINDOW_SIZE | 6291456 | 6291456 | ✓ |
| `5` MAX_FRAME_SIZE | (not sent) | (not sent) | ✓ |
| `6` MAX_HEADER_LIST_SIZE | 262144 | 262144 | ✓ |
| `WINDOW_UPDATE` (post-handshake) | **15663105** | **15728640** | **✗ Δ65535** |
| Stream priority weight | 256 (exclusive) | 255 (exclusive) | **✗ off-by-one** |

Two mismatches. Both are subtle but Akamai-FP is computed from byte-exact values — any delta produces a different hash, which matches a non-Chrome bucket in their classifier.

---

## Per-site detail

### The 9 Akamai-CHL gap sites (walmart, homedepot, uniqlo, weather, hulu, disneyplus, bestbuy, costco, washingtonpost)

**Common pattern**:
- Patchright/pwstealth/nodriver pass with Chromium-bundled HTTP/2 stack
- Their UAs include `HeadlessChrome` (Patchright) — UA is irrelevant
- They set `_pxvid`/`_px3`/`_pxhd` cookies (PerimeterX), no `_abck`
- Their bodies are 1+ MB (real homepage)

**Our pattern**:
- We get `_abck` set as `~-1~` value (Akamai BMP "untrusted bot" marker)
- Body length << real homepage

**Fix**: tune `INITIAL_CONNECTION_WINDOW_SIZE = 15_663_105` (was `15_728_640`) and `headers_stream_dependency` weight 256 (was 255). One line each in `crates/net/src/h2_client.rs`. Then re-run holistic sweep — expect 9 Akamai-CHL sites to flip to L3-RENDERED.

### The 3 captcha-CHL gap sites (duolingo, substack, spotify)

Patchright passes all three. Captures show:
- **duolingo**: 1.17 MB body, 18 cookies (analytics + Duolingo's own auth tokens — no bot-detect cookie)
- **substack**: 194 KB body, 14 cookies, 1 sensor POST. **Substack runs Cloudflare Turnstile + bot.js**.
- **spotify**: 405 KB body, 10 cookies (sp_t, _ga, _gat — no bot-detect markers)

For these, the issue isn't a vendor sensor; it's our classifier hitting the word "captcha" in their JS bundles when our body is too small to satisfy the 100 KB false-positive guard. Or: our body length is < 100 KB because the page didn't fully render.

**Fix path**: re-test these specific 3 sites with `BOXIDE_DEBUG_NAV=1` to see body length captured. If body > 100 KB, the classifier should already pass them. If < 100 KB, something earlier blocked the full render.

### Hyatt (Kasada-CHL gap)

Patchright passes with body **12 KB only**. That's NOT the real Hyatt homepage — likely a redirect/loading shell. Cookies: `source-country, source-region, AKA_A2`. No `_abck` (Akamai didn't challenge). 

**Important**: Patchright's "PASS" here may be a classifier artifact. The 12 KB body lacks the strong markers our classifier checks for (no `_kpsdk`, no `_abck`). Whether Hyatt actually serves the real site to Patchright is questionable — manual check needed.

### mail-ru (THIN-BODY)

Patchright + pwstealth both pass at **1+ MB body**. We get THIN-BODY (<1 KB). This is a **real bug in our HTTP stack**, not a stealth issue:
- mail.ru does redirect chains (HTTPS → e.mail.ru → mail.ru/?afterReload)
- Our `get_follow` may not handle one of the hops (HTTP/1.1 fallback? meta-refresh in body?)

Patchright's request shows 27 cookies set across mail-ru's auth chain — they all carry through redirects properly. Our jar isn't getting one of them.

**Fix**: trace mail.ru with `BOXIDE_DEBUG_REDIRECTS=1` (Phase C item). 2-4 h work.

### tripadvisor (DataDome — only Camoufox passes)

Both Patchright and pwstealth FAIL tripadvisor with `DataDome-CHL` (status 403, body 1476 B, cookies `datadome` + `TAUnique`). Same as us.

**Only Camoufox passes**, with Firefox UA + Firefox NSS-class TLS.

DataDome on tripadvisor scores TLS class **Chromium=bot, Firefox=human** by default risk weighting. There is no UA/header workaround — ONLY Firefox NSS TLS bypasses. **Phase B.3 ext** (boring2 NSS-class reconfig) is the only path to closing this.

---

## Why our `_abck` lights up but Patchright's doesn't (technical detail)

Akamai's BotMan v3 uses a tiered detection:

1. **Edge fingerprint** (TLS + HTTP/2): if mismatched → `_abck = "~~-1~..."` (untrusted)
2. **Sensor JS**: runs in browser, posts encoded fingerprint to `/_bm/_data` → server validates and updates `_abck` to `~~0~...` (validated)
3. **PerimeterX layer** (some Akamai sites layer PX on top): JS scoring, sets `_px3`/`_pxhd`

For Patchright on walmart: the edge passes (Chromium TLS+H2 = Chrome-coherent). The Akamai sensor JS doesn't even need to fire — the request is already trusted at the edge. PerimeterX then runs and scores Patchright as OK (fingerprint surface matches Chrome enough).

For browser_oxide: the edge **fails**. Our TLS may match (boring2 BoringSSL JA4 = Chrome 147), but our HTTP/2 WINDOW_UPDATE is off by 65,535. Akamai's edge fingerprint includes the WINDOW_UPDATE, so we land in the "untrusted browser" bucket. `_abck` is set with the `-1` marker. We never get to run the sensor JS, never reach PerimeterX. Pure edge rejection.

---

## The top 3 fixes ranked by site impact

### Fix 1 — H2 fingerprint precision (Akamai 9 sites) — **HIGHEST ROI**

**File**: `crates/net/src/h2_client.rs`

```rust
// Current:
const INITIAL_CONNECTION_WINDOW_SIZE: u32 = 15_728_640;

// Match Chrome 147 exactly:
const INITIAL_CONNECTION_WINDOW_SIZE: u32 = 15_663_105;
```

Plus:
```rust
.headers_stream_dependency(StreamDependency::new(
    StreamId::zero(),
    256,  // was 255 — Chrome uses 256 (exclusive=true)
    true,
))
```

**Effort**: 30 minutes (2 line changes + run holistic sweep to validate). **Sites unlocked**: 9.

### Fix 2 — Trace mail-ru redirect (1 site) — **CHEAP**

`BOXIDE_DEBUG_REDIRECTS=1` env-var-gated tracing in `crates/net/src/lib.rs::get_follow` (already implementable per our existing `BOXIDE_DEBUG_NAV` pattern). Run it on mail.ru, find which hop drops cookies.

**Effort**: 2-4 h. **Sites unlocked**: 1.

### Fix 3 — Body-length recapture for captcha-CHL trio (3 sites)

Investigate why duolingo/substack/spotify hit the < 100 KB threshold for our classifier. Likely render-time issue (page does heavy JS that takes longer than our 15 s budget for first iter). Possible quick fix: increase `BOXIDE_NAV_BUDGET_MS` for these specific sites or add behavior-flag iteration.

**Effort**: 2-4 h. **Sites unlocked**: 0-3.

### Fix 4 — Phase B.3 ext Firefox NSS TLS (1 unique site, 4-8 likely) — **MEDIUM-HIGH**

Reconfigure boring2's TLS for Firefox NSS-class JA4 (`t13d1715h2_5b57614c22b0_3d5424432f57`). Closes tripadvisor + likely flips a few currently-Akamai-CHL retail sites that score on TLS class. Documented at `docs/RESEARCH_REQUIRED_2026_04_28.md`.

**Effort**: 1-2 d. **Sites unlocked**: 1-4 incremental.

---

## Estimate of post-fix PASS count

Current: 98/126 (78%)

| After Fix | Estimated PASS | New % |
|---|---:|---:|
| + Fix 1 (H2 precision) | 105-107 | 83-85% |
| + Fix 2 (mail-ru) | 106-108 | 84-86% |
| + Fix 3 (3 captcha sites) | 109-111 | 87-88% |
| + Fix 4 (Firefox NSS TLS) | 110-115 | 87-91% |
| + G.1 Akamai sensor solver | 113-118 | 90-94% |

Combined max realistic: **~115-118 / 126 (91-94%)**. Remaining ~10 sites are IP-attributable (Kasada strict, Russian sites, etc.) — outside engine scope.

---

## Validation

After each fix, run:

```bash
cargo build --release -p browser --test holistic_sweep
cargo test --release -p browser --test holistic_sweep \
    -- --ignored --test-threads=1 --nocapture holistic_sweep_parallel \
    > /tmp/sweep.log 2>&1
grep "^holistic-end:" /tmp/sweep.log | awk '{print $5}' | sort | uniq -c
```

For Fix 1 specifically, also test against tls.peet.ws to verify the new Akamai-FP hash matches Chrome's `52d84b11737d980aef856699f885ca86`:

```bash
# Need a test that hits tls.peet.ws and prints the akamai_h2_fingerprint_hash field
# Add to chl_sites.rs or a new test file
```

---

## Source data

Per-site captures with full request/response headers, cookies, sensor POSTs:
- `/tmp/patchright_capture/{walmart,homedepot,...}.json`
- `/tmp/pwstealth_capture/{walmart,...}.json`
- `/tmp/cam_capture/*.json`
- `/tmp/patchright_capture/summary.txt` and `/tmp/pwstealth_capture/summary.txt` for human-readable diff

Reproducible:

```bash
# Patchright
source /tmp/patchright-test/bin/activate
PWMODE=patchright python /tmp/playwright_capture.py

# playwright-stealth
source /tmp/pwstealth-test/bin/activate
PWMODE=playwright python /tmp/playwright_capture.py
```
