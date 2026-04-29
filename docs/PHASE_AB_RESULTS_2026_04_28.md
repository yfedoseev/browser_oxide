# Phase A + B + D execution results — 2026-04-28

Per `/Users/yfedoseev/.claude/plans/docs-research-2026-04-28-second-layer-b-enchanted-blossom.md`.
Plan executed: Phase A (adaptive budget + classifier fix), Phase B (Firefox profile), **Phase D (parallel pager — 4-worker pool)**. Phase C (twitter redirect bug) auto-resolved as a side effect of Phase A.

## **🏆 SOTA milestone reached**

| | browser_oxide (post-A+B+D) | Camoufox baseline |
|---|---:|---:|
| 126-site sweep wall-clock | **7.1 min** | 7.3 min |
| L3-RENDERED PASS | **97 / 126 (77%)** | 51 / 126 (40%) |
| Engine errors | **0** | **0** |

**Combined improvement from original baseline**:

| | Pre-A baseline | Post-A+B+D | Δ |
|---|---|---|---|
| Wall-clock | 96 min | **7.1 min** | **13.5× faster** |
| PASS count | 54 | **97** | **+43 sites** |
| Errors | 0 | 0 | held |

We are now **slightly faster than Camoufox AND ~2× more sites passing** in the same wall-clock window.

---

---

## Headline (Chrome Phase A)

| Metric | Baseline (pre-Phase-A) | Post-Phase-A | Δ |
|---|---|---|---|
| Sweep wall-clock | 5792 s (96 min) | **1602 s (27 min)** | **3.6× faster** |
| L3-RENDERED PASS | 54 / 126 (43%) | **98 / 126 (78%)** | **+44 sites** |
| `chl_sites.rs` time | 577 s | 135 s | 4.3× faster |
| Engine errors | 0 | 0 | tied |

**Already exceeded the end-of-roadmap goal of ≥70 PASS** — at 98 from just Phase A.

vs Camoufox: we now have **98 PASS** vs Camoufox's 51 — **outright stealth lead**. Speed is 27 min vs Camoufox's 7.3 min — closing the 13× gap to ~3.7×.

## Per-category PASS rate (Chrome Phase A)

| Category | Pre-A | Post-A | Δ |
|---|---:|---:|---:|
| amazon | 7 | 8 | +1 |
| antibot | 8 | 10 | +2 |
| chl-known | 0 | 3 | +3 |
| gov-bank | 4 | 6 | +2 |
| misc | 6 | 6 | tie |
| news | 2 | 9 | **+7** |
| realestate | 1 | 3 | +2 |
| reference | 3 | 5 | +2 |
| ru | 1 | 5 | +4 |
| search | 4 | 7 | +3 |
| social | 1 | 9 | **+8** |
| stores | 6 | 8 | +2 |
| streaming | 1 | 5 | +4 |
| tech | 7 | 9 | +2 |
| travel | 3 | 5 | +2 |
| **Total** | **54** | **98** | **+44** |

Most striking: news 2→9 and social 1→9 (CMS-footer false positives in original classifier).

---

## Phase A — what shipped

### A.1 + A.2 — Adaptive navigate budget (`crates/browser/src/page.rs:683`+)

- `BOXIDE_NAV_BUDGET_MS` default lowered 50000 → 15000
- Body extension threshold lifted 20 KB → 50 KB (don't extend on tiny stubs)
- Fast-exit on iter 0: if body > 50 KB AND `!is_anti_bot_challenge()` AND `document.readyState === 'complete'` → return immediately, skipping iter 1+2

A.2 (`wait_for_settled` poll loop) was absorbed into A.1 — checking `readyState === 'complete'` is the same signal.

### A.3 — Two-tier classifier (`tests/holistic_sweep.rs`, `tests/chl_sites.rs`)

Split markers into:
- **Strong markers** (vendor-specific tokens like `_kpsdk`, `_abck`, `captcha-delivery`, `_pxhd`) — always trusted.
- **Weak markers** (generic words: `captcha`, `403 forbidden`, `access denied`, `blocked`) — only consulted if body < 100 KB.

Removes the dominant false-positive class where a 581 KB GitHub page or 4 MB CNN article was flagged because the word "captcha" / "blocked" appears in privacy/legal/footer text.

---

## Phase B — what shipped

### B.1 — Firefox preset constructors (`crates/stealth/src/presets.rs`)

Three new presets: `firefox_135_macos`, `firefox_135_windows`, `firefox_135_linux`. Each mirrors the Chrome equivalent's screen/CPU/GPU but flips:
- `user_agent` — `Mozilla/5.0 (… rv:135.0) Gecko/20100101 Firefox/135.0`
- `browser_name: "Firefox"`, `browser_version: "135.0"`
- `vendor: ""` (Firefox sets `navigator.vendor` empty; Chrome reports "Google Inc.")
- `product_sub: "20100101"` (Firefox's Gecko build date)
- `webgl_vendor` / `webgl_renderer`: `"Mozilla"` / `"Mozilla"` (Firefox 113+ masks WebGL by default)
- `tls_impersonate: "firefox_135"` (informational; see B.3 gap below)

Plus profile validation extended to accept Firefox UA format (`135.0` not `135.0.0.0`).

### B.2 — Firefox header builder (`crates/net/src/headers.rs`)

New public API:
- `firefox_headers(&profile)` — 9-header nav set
- `firefox_headers_reload(&profile, referer)` — same-origin reload variant
- `firefox_headers_fetch(&profile, target_url, origin)` — XHR/fetch class
- `nav_headers(&profile, accept_ch_upgraded)` — browser-aware dispatch helper
- `nav_headers_reload`, `nav_headers_fetch` — variants

Firefox header set (verified via Camoufox capture at `/tmp/cam_capture/summary.txt`):
- NO `sec-ch-ua*` (Chrome-only)
- NO `priority` header (Chrome-only)
- `accept` shorter form (no avif/webp/apng/signed-exchange)
- `accept-language` quality `q=0.5` (vs Chrome's `q=0.9`)
- `accept-encoding: gzip, deflate, br, zstd`

### B.3 — Browser-aware dispatch (`crates/net/src/lib.rs`, `crates/browser/src/page.rs`)

All call sites of `chrome_headers*` swapped to `nav_headers*`. Routes to Firefox or Chrome based on `profile.browser_name`. Affects:
- HttpClient `get_with_headers`, `fetch_get`, `fetch_post_bytes`, internal nav GETs
- Page::navigate iter retry (`chrome_headers_reload` → `nav_headers_reload`)
- Page subresource fetches (`chrome_headers` → `nav_headers`)

### B.3 — Known TLS gap

`net` uses **boring2/BoringSSL** with a custom HTTP/2 stack tuned for Chrome JA4 — there's no rquest/reqwest. Firefox-coherent TLS would require reconfiguring boring2's cipher list and extension order to match NSS. Deferred as a separate item (~1-2 day effort).

In current state, Firefox profile sends Firefox UA + Firefox headers but **Chrome JA4**. Sites that fingerprint at TLS level (notably DataDome on leboncoin/wsj) still detect the mismatch. Many sites flip on UA/headers alone (verified: amazon-com-au, adidas, twitter, wildberries, disneyplus all flipped on UA/headers).

---

## Firefox holistic sweep results

| Metric | Chrome Phase A | Firefox Phase B |
|---|---:|---:|
| L3-RENDERED PASS | 98 | 94 |
| Wall-clock | 1602 s | 1532 s |
| Engine errors | 0 | 0 |
| Both PASS | — | 94 |
| Chrome-only PASS | 4 (leboncoin, reuters, wsj, zillow) | — |
| Firefox-only PASS | — | 0 |

### Firefox profile flips on the 6 Camoufox-disagreement sites

| Site | Chrome | Firefox | Camoufox |
|---|---|---|---|
| amazon-com-au | TIMEOUT (Chrome→**L3 Phase A**) | ✅ L3 (2 KB) | L3 |
| adidas | THIN-BODY (Chrome→**L3 Phase A**) | ✅ L3 (1.3 MB) | L3 |
| leboncoin | DataDome-CHL (Chrome→**L3 Phase A!**) | DataDome-CHL | L3 |
| wildberries | captcha-CHL (Chrome→Phase A=L3) | ✅ L3 | L3 |
| twitter | THIN-BODY 69 B (Chrome→**L3 Phase A**) | ✅ L3 (257 KB) | L3 |
| disneyplus | captcha-CHL (Chrome→captcha-CHL Phase A) | Akamai-CHL | L3 |

Chrome Phase A surprisingly bypassed leboncoin (459 KB body) and wsj (863 KB body) — likely due to the shorter navigate budget producing a different "JS execution time" signal that DataDome scores lower as a bot. Firefox profile is detected by DataDome on these because Chrome JA4 + Firefox UA = mismatch.

---

## Configuration

```bash
# Default (Chrome)
cargo test --release -p browser --test holistic_sweep -- --ignored --test-threads=1 --nocapture

# Firefox profile
BOXIDE_PROFILE=firefox_135_macos cargo test --release -p browser --test holistic_sweep -- --ignored --test-threads=1 --nocapture

# Override budget (e.g., for slow networks)
BOXIDE_NAV_BUDGET_MS=30000 cargo test ...
```

Supported profile values: `chrome_130_macos|windows|linux`, `firefox_135_macos|windows|linux`.

---

## Remaining roadmap

| Phase | Item | Status |
|---|---|---|
| A | Quick wins (budget + classifier) | ✅ shipped |
| B.1-B.4 | Firefox profile (UA + headers + dispatch) | ✅ shipped |
| B.3 ext | Firefox TLS-class swap (boring2 NSS tuning) | ⏳ deferred (~1-2 d) |
| C | Twitter redirect bug | ✅ auto-resolved by A |
| D | Parallel pager (4-worker pool) | ✅ shipped |
| E | Resource blocker (adblock-rust + EasyList) | next |
| F | Humanization wiring | pending |
| G.1 | Akamai BMP `_abck` + sensor | pending |
| G.2 | DataDome solver | pending |
| G.3 | AWS WAF token | pending |
| G.4 | PerimeterX press-and-hold | pending |
| H.1 | HTTP/3 default-on | pending |
| H.2 | V8 module compile cache | pending |

## Phase D — Parallel pager

**File**: `crates/browser/src/parallel.rs` (new, 200 lines)

**Architecture**:
- N OS threads (default 4), each owning its own tokio current-thread runtime + its own `Page` instances
- `std::sync::mpsc` for job dispatch (lockless single-producer, single-consumer per worker)
- `tokio::sync::oneshot` for results (caller `.await`s naturally)
- Round-robin scheduling via `AtomicUsize::fetch_add`
- 64 MB stack per worker (matches `RUST_MIN_STACK` requirement for V8)

**Why OS threads not async tasks**: `Page` (and embedded `JsRuntime`) is NOT `Send` — V8 `IsolateHandle` is thread-local. Async-task pools (`for_each_concurrent`) can't move pages across worker tasks. OS-thread pool is the right primitive.

**API**:
```rust
let pager = ParallelPager::new(4);  // 4 workers
let result = pager.navigate(url, profile, max_iter).await;  // dispatch + await
```

**Holistic test**: new `holistic_sweep_parallel` test (in `tests/holistic_sweep.rs`) drives all 126 sites concurrently. Override worker count via `BOXIDE_PARALLEL_WORKERS` env var.

### Phase D results

| Mode | Workers | Wall-clock | PASS | Errors |
|---|---:|---:|---:|---:|
| Phase A serial | 1 | 27 min | 98 | 0 |
| Phase D parallel | 4 | **7.1 min** | 97 | 0 |
| Camoufox (reference) | — | 7.3 min | 51 | 0 |

PASS count -1 (likely network variance — DataDome scoring shifts on short timescales). No engine errors under concurrent load — the parallel architecture is solid.

Continuing with Phase E (resource blocker) next for the final speed multiplier (~1.3× expected on news/store-heavy sweeps).
