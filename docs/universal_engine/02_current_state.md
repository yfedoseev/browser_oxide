# 02 — Current state: what works, what's broken

> **⚠️ HISTORICAL — 2026-04-10 baseline.**
>
> For the **current state** (post Sprint 0/1/2/3 + probe diagnosis), read
> [`09_session_2026_04_11_state.md`](09_session_2026_04_11_state.md).
>
> This file is kept as the pre-Sprint-0 baseline for diff value. Everything
> described in "What was shipped in the 2026-04-10 session" below is the
> starting state of the 2026-04-11 work. The "currently-blocked sites"
> table and the `navigate_with_challenges` flow description both still
> apply as baseline history, but **the navigate flow and per-engine logic
> have been removed** and the solver has changed substantially.

## Workspace health

- **`cargo test --workspace -- --test-threads=1`**: all green, zero failures.
- **`cargo check -p js_runtime`**: clean, ~31 warnings (mostly unused args in
  stub functions — not blockers).
- **`cargo fmt --all -- --check`**: has diffs in files that were edited by
  linters automatically; a `cargo fmt --all` pass will fix them.
- **`cargo clippy --workspace -- -D warnings`**: may fail on some pre-existing
  warnings; run with `--fix` if needed.

## The 22 deep-path passing sites

Source of truth: `crates/browser/tests/deep_path_validation.rs` (rigorous
content-marker probe, not just status code). All 22 HOLD across landing page
and a real deep path:

| Site | Status | Deep path |
|---|---|---|
| avito.ru | HOLD | `/moskva/noutbuki` |
| baidu.com | HOLD | `/s?wd=test` |
| bilibili.com | HOLD | `/video/BV1` |
| chatgpt.com | HOLD | `/auth/login` |
| coinbase.com | HOLD | `/price/bitcoin` |
| crunchbase.com | DEGRADE on /search | 403 on deep search path |
| delta.com | HOLD | `/flight-search/book-a-flight` |
| discord.com | HOLD | `/developers/docs/intro` |
| douyin.com | HOLD | `/discover` |
| glassdoor.com | HOLD | `/Job/index.htm` |
| jd.com | HOLD | `/search?keyword=laptop` |
| linkedin.com | HOLD | `/feed/` |
| medium.com | HOLD | `/topic/technology` |
| nike.com | HOLD | `/w/mens-shoes-nik1zy7ok` |
| reddit.com | HOLD | `/r/rust/` |
| stockx.com | HOLD | `/sneakers` |
| taobao.com | HOLD | `/search?q=shoes` |
| tmall.com | HOLD | `/search` |
| turbotax.com | HOLD | `/products.jsp` |
| vk.com | HOLD | `/feed` |
| walmart.com | HOLD | `/search?q=laptop` |
| ya.ru | HOLD | `/search/?text=test` |
| zillow.com | HOLD | `/homes/` |

Amazon also HOLDs on landing but the deep-path URL in the test is a dead
product (`/dp/B08N3TCP5Z` → 404). That's a test data issue, not a bot block.

## The 8 currently-blocked sites

Source of truth: `crates/browser/tests/blocker_rigorous_probe.rs`. Run 3
times to measure stability. All 8 stably FAIL.

| Site | Engine | Baseline (raw GET) | Solver (navigate_with_challenges) | Notes |
|---|---|---|---|---|
| adidas.com | Akamai BMP v3 | INTR 2351b | INTR 2418b | Trust slot ~-1~ never upgrades |
| homedepot.com | Akamai BMP v3 | INTR 2621b | INTR 2694b (1 run passed with 973KB, not repro) | Same engine as adidas |
| canadagoose.com | Kasada | INTR 701b | INTR 752b | Solver runs, gets x-kpsdk-ct, retry still blocked |
| hyatt.com | Kasada | INTR 686b | INTR 737b | Same shape as canadagoose |
| wildberries.ru | WBAAS | INTR 1447b or conn-closed | 1915b or ERR | Flaky; recently started returning `connection closed before headers` — probably rate-limited |
| dns-shop.ru | QRATOR | INTR 6319b | INTR 7472b | Solver sends `POST /__qrator/validate?pow=168&nonce=&qsessid=` with empty nonce/qsessid → 403 |
| ozon.ru | 307 redirect loop | INTR 164b | INTR 156b | Just `307 Temporary Redirect` with a `Location: ?__rr=N+1` header, not actually bot-blocked; solver needs `client.get_follow()` |
| ya.ru | Inconsistent | INTR 0b | INTR 39b or 488356b | One run returned 488KB of real content; my probe's positive markers are wrong |

### Detailed per-site deep-dives

See `site_debugging/` for per-site writeups with reproduction steps and the
full list of what's been tried.

## What the solver does today

`Page::navigate_with_challenges(url, profile, max_retries)` in
`crates/browser/src/page.rs`:

1. GET the URL via `net::HttpClient::get` (no redirect follow)
2. Call `is_challenge_page(status, body, headers)` — pattern match against
   Akamai/Kasada/WBAAS/CF/DataDome markers
3. If NOT a challenge, return the page from the HTML
4. If it IS a challenge, call `build_page_with_scripts` which:
   - Parses HTML
   - Fetches and runs all `<script>` tags in a V8 isolate
   - Installs a fetch hook to log URLs and bodies
   - Fires DOMContentLoaded and load events via setTimeout(0)
   - Runs the "humanize" script (30 mousemoves + 2 clicks + keydown/keyup)
   - Drains the event loop for 30 seconds
5. After the solver runs, scan `window.__fetchLog` for `/tl` POST responses
   and extract `x-kpsdk-ct`/`x-kpsdk-st`/`x-kpsdk-cr` headers into a
   `solver_session_tokens` Vec (Kasada-specific)
6. Do a JS-level retry: inside the same V8 isolate, execute a script that
   does `new XMLHttpRequest(); xhr.open('GET', url)` then falls back to
   `fetch(url)`. Capture the response body as the "retry result"
7. If the JS retry returned a non-challenge page, use that
8. Otherwise, do a Rust-level retry with `get_with_headers(url,
   reload_overrides)` where `reload_overrides` includes:
   - `sec-fetch-site: same-origin`
   - `referer: current_url`
   - Every Kasada token from `solver_session_tokens`
9. Loop up to `max_retries` total

## What's engine-specific in that flow (to be removed per refactor)

- `is_challenge_page` with Akamai/Kasada/WBAAS/CF/DataDome markers (~line
  1200-1240 in page.rs)
- `solver_session_tokens` Vec and the whole Kasada x-kpsdk-* extraction
  (~line 448, 481-487, 732-765)
- WBAAS-specific logging of `status-no-id` and `x-wbaas-token` (~line
  507-527)
- The retry loop exists ONLY because we don't have real `location.reload()`
  or `<meta refresh>` handling

See `04_refactor_plan.md` for the generic replacement.

## What was shipped in the 2026-04-10 session

- **T1.3 Blink DynamicsCompressor + PeriodicWave port** →
  `crates/canvas/src/audio.rs`. Matches FingerprintJS reference sum
  `124.04347527516074` within 60 ppm. Wired through
  `op_offline_audio_render` (new op in
  `crates/js_runtime/src/extensions/audio_ext.rs`), JS-side
  `OfflineAudioContext.startRendering` calls the Rust kernel.
- **T1.5 Real Worker threads** → `crates/js_runtime/src/extensions/
  worker_ext.rs`. Each `new Worker(blob:URL)` spawns a real OS thread with
  its own V8 isolate. Two mpsc channels for bidirectional messaging.
  Round-trip test passes. Note: the adidas sensor VM does NOT use Workers
  (verified by probe), so T1.5 doesn't affect adidas. It's still valuable
  because other sites may need real Workers.
- **OffscreenCanvas** class (was missing — undefined), `MediaDevices`,
  `StorageManager`, `ServiceWorkerContainer`, `Bluetooth`,
  `NetworkInformation`, `Permissions`, `PermissionStatus` — proper class
  prototypes so `Object.getPrototypeOf(nav.X).constructor.name` returns the
  correct WebIDL brand name instead of `Object`.
- **Behavioral humanize script** — 30 Bezier mousemoves + 2 clicks + Tab
  keydown/keyup fired on every navigation. Moved Akamai section 6 event
  counts from `44,92,0,1,13,2139` (implausible ~46 Hz clicking) to more
  realistic-looking values. Did not unblock trust.
- **Cookie-write instrumentation + `_abck` trajectory logging** in page.rs
  for diagnostic visibility.
- **`BOXIDE_DUMP_POST_DIR` env var** in `net::HttpClient::post_bytes_with_
  headers` that writes every POST body to disk for diffing.

## Test artifacts worth keeping

These files exist and are useful — don't delete them:

- `/tmp/adidas_sensor_vm.js` — 438 KB captured Akamai sensor VM used by
  probe instrumentation.
- `/tmp/adidas-cookies.txt` — 56 cookies extracted from a live Playwright
  Chrome session (via CDP Network.getAllCookies), Netscape format, used by
  `adidas_cookie_replay.rs`.
- `/tmp/oxide-sensor-*/` — multiple directories of captured POST body
  snapshots at different stages of the session (baseline, post-infra-fixes,
  post-T1.3, post-humanize). Useful for byte-level diffs.
- `/tmp/chrome-sensor/final.html` — the Akamai WAF hard-block page served
  to headless Playwright (proves the IP is graylisted for Playwright; does
  not affect rquest).
