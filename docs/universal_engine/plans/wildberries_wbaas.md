# Wildberries — WBAAS solver completion

**Tasks**: #10 (WB retry GET accepted with x_wbaas_token), #21
(Reverse-engineer WBAAS challenge_fingerprint_v1.0.23.js)
**Priority**: P0 — closest of any blocker site to passing
**Effort**: 6-12 hours total
**Dependencies**: Sprint 0 refactor is strongly recommended first
(the `location.reload()` primitive may solve #10 for free).

## Why wildberries is the highest-ROI blocker

Per `docs/universal_engine/site_debugging/wildberries_wbaas.md`:

1. **We're already past the hardest step**: our solver POSTs to
   `/__wbaas/challenges/antibot/api/v1/create-token` TWICE. The
   first returns 498 (expected; settings check), the second returns
   **200**. WBAAS has accepted our fingerprint.
2. **Only the final navigation is missing**: after the solver
   writes `x_wbaas_token=...` via `document.cookie = ...`, the
   retry GET still returns 498.
3. **WBAAS has no public commercial solver**. Solving it produces
   durable capability (Akamai's sensor VM regenerates daily;
   WBAAS version-bumps slowly).
4. **It's a model for other Russian sites**: QRATOR on dns-shop,
   DDoS-Guard on ozon (though ozon turned out to be plain 307 loops).

## Task #10 — WB retry GET accepted with x_wbaas_token

**Effort**: 2-4 hours
**Goal**: Make the retry after the solver run succeed.

### Hypothesis

The `x_wbaas_token` cookie is set via `document.cookie = ...` from
JS. Task #8 (completed) unified `document.cookie` with the
`net::HttpClient` cookie jar via `op_cookie_set`. If that works
correctly, the next HTTP request should include the cookie.

So either:
1. The cookie propagation from JS to the jar isn't working for this
   specific cookie.
2. The cookie IS propagating but WBAAS expects additional state
   (specific headers, a cookie set on a different domain, a
   TLS-session-bound token, etc.).
3. The cookie is set AFTER the Rust retry happens (race condition
   between the solver's async write and our synchronous retry).

### Step 1 — Diagnose (1h)

**File**: `crates/browser/src/page.rs`

In the retry loop (after the refactor, this is the next iteration
of `Page::navigate`), add temporary diagnostic logging:

```rust
// Before retrying, print the cookies the jar has for this URL.
let jar_cookies = client.cookies_for_url(&parsed_url).await.unwrap_or_default();
eprintln!("[retry] jar cookies for {url}: {jar_cookies}");

// Also print what JS sees for document.cookie to confirm the JS<->jar
// sync is working.
let js_cookies = page.event_loop()
    .execute_script("document.cookie")
    .unwrap_or_default();
eprintln!("[retry] JS document.cookie: {js_cookies}");
```

Expected outcome:
- If both print `x_wbaas_token=1.1000...`, the token is in the jar
  and the server is rejecting it for some other reason. Move to
  step 2.
- If JS shows the token but the jar doesn't, task #8's `op_cookie_set`
  isn't firing for this path. Fix by inspecting
  `crates/js_runtime/src/extensions/fetch_ext.rs::op_cookie_set` and
  `crates/js_runtime/src/js/dom_bootstrap.js::Document set cookie`.
- If neither shows the token, the solver didn't actually finish — maybe
  it errored or our event loop didn't drain long enough. Check
  `window.__scriptErrors` and `window.__asyncErrors`.

### Step 2 — Fix the propagation (1-2h)

**If the cookie isn't in the jar**: trace the path from
`document.cookie = ...` to `HttpClient::set_cookie_str`. The chain is:

```
JS: document.cookie = 'x_wbaas_token=...'
  → Document.prototype set cookie setter
  → op_cookie_set(location.href, raw_cookie)
  → crates/js_runtime/src/extensions/fetch_ext.rs::op_cookie_set
  → HttpClient::set_cookie_str(&url, raw_cookie)
  → net::cookies::CookieJar::set_cookies
```

Verify each step. Common bugs:
- The `set cookie` setter is defined on the wrong prototype (Document
  vs document instance) and gets overwritten later.
- The `location.href` passed to `op_cookie_set` is `"about:blank"`
  instead of the real URL (we have this guard: `if (url && url !==
  "about:blank")`, but if `location.href` isn't being updated correctly,
  the cookie isn't associated with the right domain).
- Cookie parsing in `CookieJar::set_cookies` rejects the cookie because
  the Domain attribute conflicts with the URL host.

### Step 3 — Fix any header mismatches (1-2h)

**If the cookie IS in the jar but the retry still 498s**: WBAAS is
expecting something else. Common possibilities:

- **`Origin` or `Referer` header**: our retry might be sending
  `Origin: null` or missing Referer. Check the `reload_overrides` in
  page.rs.
- **`Sec-Fetch-Site: none` vs `same-origin`**: real Chrome sends
  `same-origin` on a reload. We set this but verify it actually lands
  on the wire.
- **`User-Agent` mismatch**: our retry should use the exact same UA
  as the initial GET. Verify the stealth profile's UA is consistent.
- **`Accept-Language` inconsistency**: for a Russian site, real
  browsers typically have ru-RU in Accept-Language. Verify
  `stealth::presets::chrome_130_ru` sets this correctly.

Capture the headers on the wire:

```bash
# Run with env logging:
RUST_LOG=net=debug BOXIDE_DUMP_POST_DIR=/tmp/wb-debug \
  cargo test -p browser --test blocker_rigorous_probe \
  tier05_blockers_all -- --ignored --test-threads=1 --nocapture \
  2>&1 | grep -E "wildberries|headers"
```

### Step 4 — Verify

```bash
cargo test -p browser --test blocker_rigorous_probe tier05_blockers_all \
  -- --ignored --test-threads=1 --nocapture 2>&1 | grep -A 3 wildberries
```

Expected: `[WIN] baseline=INTR solver=PASS (>= 80000b)` — the real
wildberries home page is ~200 KB of HTML.

## Task #21 — Reverse-engineer challenge_fingerprint_v1.0.23.js

**Effort**: 4-8 hours
**Goal**: Understand exactly what fingerprint fields WBAAS hashes so
we can verify ours are correct (and durable as the script version
bumps).

### Step 1 — Fetch the latest version (15 min)

```bash
curl -s 'https://www.wildberries.ru/__wbaas/challenges/antibot/statics/challenge_fingerprint_v1.0.23.js' -o /tmp/wbaas_fp.js
wc -l /tmp/wbaas_fp.js
```

Also fetch the solver:

```bash
curl -s 'https://www.wildberries.ru/__wbaas/challenges/antibot/statics/challenge_solver_v1.0.4.js' -o /tmp/wbaas_solver.js
```

### Step 2 — Pretty-print (10 min)

```bash
# With prettier
prettier --parser babel /tmp/wbaas_fp.js > /tmp/wbaas_fp.pretty.js
prettier --parser babel /tmp/wbaas_solver.js > /tmp/wbaas_solver.pretty.js

# Or with js-beautify
js-beautify /tmp/wbaas_fp.js > /tmp/wbaas_fp.pretty.js
```

### Step 3 — Read the pretty-printed source (2-4h)

Look for:

1. **The POST body structure**: find the `fetch(...)` or `XMLHttpRequest`
   call to `/create-token`. Read what JSON object it builds. Document
   each field.
2. **Canvas fingerprint**: look for `createElement('canvas')`,
   `getContext('2d')`, `fillText`, `toDataURL`. Extract the exact
   drawing sequence and the specific text being rendered. Compare
   to the adidas probe pattern from `adidas_sensor_api_probes.rs`.
3. **WebGL fingerprint**: look for `getContext('webgl')`,
   `getParameter`, `getSupportedExtensions`. Does WBAAS hash the
   UNMASKED_VENDOR/RENDERER strings or the extension list?
4. **Audio fingerprint**: look for `OfflineAudioContext` or
   `AudioContext`. Does WBAAS use the CreepJS pipeline (oscillator
   → compressor)?
5. **Font fingerprint**: look for `measureText` loops. WBAAS may
   test specific characters at specific font sizes.
6. **Navigator fingerprint**: which navigator properties are read?
7. **Timing fingerprint**: `performance.now()` calls around
   specific operations.
8. **The POW / verification logic**: does WBAAS do any crypto-style
   work (SHA, HMAC, base64) on top of the fingerprint?

### Step 4 — Document findings (1-2h)

Update `docs/universal_engine/site_debugging/wildberries_wbaas.md`
with a section "What challenge_fingerprint_v1.0.23.js does" — modeled
on the adidas probe findings. This is the reference for future version
bumps.

### Step 5 — Verify our fingerprint matches

Run our solver, capture the POST body, compare to what the script is
supposed to produce. Any mismatched field is a bug in our runtime.

## Why #21 is worth doing even if #10 solves the site

1. **Version bumps**: WBAAS will ship `v1.0.24` eventually.
   Understanding the schema means you can detect breakage early.
2. **Durable capability**: unlike Akamai's rotating VM, WBAAS is
   stable enough that the knowledge amortizes.
3. **Template for QRATOR**: the techniques (capture, pretty-print,
   read, instrument) apply directly to dns-shop.ru's QRATOR script
   in the Russian sites cluster.

## Acceptance criteria

1. `cargo test -p browser --test blocker_rigorous_probe tier05_blockers_all`
   shows wildberries as `solver=PASS` with body > 80 KB.
2. `docs/universal_engine/site_debugging/wildberries_wbaas.md` is
   updated with the reverse-engineered fingerprint schema and any
   fixes that were required.
3. Re-running the test 3 times in a row consistently passes (no
   flaky behavior).

## Risks

- **IP rate limiting**: our IP has been hammered by repeated
  wildberries testing. If you see "connection closed before headers",
  wait 30-60 minutes or use a different network.
- **Solver internals may error silently**: the script has try/catch
  wrappers. Use `window.__scriptErrors` and `__asyncErrors` to catch
  them.

## Related tasks (historical)

- #2, #15-20 WB validation steps [done]
- #16 High-entropy Client Hints [done]
- #17 navigator.userAgentData.getHighEntropyValues [done]
- #19 HPACK dynamic table [done]
- Task #10 is the closest-to-done of any remaining task.
