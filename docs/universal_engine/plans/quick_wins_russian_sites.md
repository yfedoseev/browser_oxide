# Quick wins — ozon, ya.ru, dns-shop (QRATOR)

**Priority**: P0 (cheap and concrete)
**Total effort**: 5-10 hours
**Dependencies**: Sprint 0 refactor (for `get_follow`); wildberries
is split out into its own plan because it's more involved.

## Goal

Flip the four easiest-to-fix blocker sites from FAIL to PASS using
minimal code changes. This is the "demonstrate momentum" work before
committing to the larger Tier-1 capability investments.

---

## ozon.ru — 5-15 minutes

**Engine**: None. It's HTTP 307 redirect loops using `?__rr=N`
parameter for round-robin server selection. Not a bot block.

**Fix**: Use `client.get_follow(url, 10)` instead of `client.get(url)`
in the navigate path. This happens automatically as part of the
Sprint 0 refactor (`refactor_generic_navigation.md` Step 3). If you
want to verify ozon specifically:

```bash
cargo test -p browser --test debug_blocked debug_ozon -- \
    --ignored --test-threads=1 --nocapture
```

Currently this prints the 307 Temporary Redirect body. After the
refactor it should print the real ozon home page (~200 KB of HTML).

**Verify passing**:

```bash
cargo test -p browser --test blocker_rigorous_probe tier05_blockers_all \
    -- --ignored --test-threads=1 --nocapture 2>&1 | grep ozon
```

Expected after refactor: `[WIN] baseline=PASS solver=PASS`.

---

## ya.ru — 30-90 minutes

**Engine**: Yandex's own edge. Sometimes empty body on raw GET;
sometimes SmartCaptcha. Our solver sometimes returns 488 KB of real
content, sometimes 39 bytes. Inconsistent.

### Fix 1 — Use `get_follow`

Same as ozon. Part of the Sprint 0 refactor.

### Fix 2 — Fix the probe markers

**File**: `crates/browser/tests/blocker_rigorous_probe.rs`

Current markers:
```rust
&["ya.ru", "yandex"],
&["SmartCaptcha", "smart-captcha", "\"captcha\""],
```

The 488 KB body we got in one run didn't match `["ya.ru", "yandex"]`
because the actual HTML uses different text. Find a stable marker by
running:

```bash
curl -s 'https://ya.ru/' -o /tmp/yaru.html
head -c 5000 /tmp/yaru.html | grep -oP '(class|id|href|src|data-\w+)="[^"]{10,60}"' | sort -u | head -20
```

Pick a marker that's stable across days and user agents. Good
candidates:
- `data-bem` (Yandex uses BEM)
- `<meta name="yandex-verification"`
- `homer_desktop` or similar Yandex-specific CSS class prefixes

Update the markers:
```rust
&["data-bem", "homer", "yandex-verification"],
&["SmartCaptcha"],
```

### Fix 3 — Real HTTP Accept-Language header

If the empty-body response is because Yandex gates content on
`Accept-Language: ru-RU,ru;q=0.9,en;q=0.8`, our `chrome_130_ru`
preset may not set this correctly. Verify:

```bash
grep -r "Accept-Language" crates/stealth/src/presets.rs
```

Make sure `chrome_130_ru` emits `Accept-Language: ru-RU,ru;q=0.9,
en-US;q=0.8,en;q=0.7`. If it doesn't, fix the preset.

### Verify

```bash
cargo test -p browser --test blocker_rigorous_probe tier05_blockers_all \
    -- --ignored --test-threads=1 --nocapture 2>&1 | grep yandex
```

Expected: `[WIN] baseline=PASS or INTR / solver=PASS`.

---

## dns-shop.ru — 4-8 hours (more involved)

**Engine**: QRATOR. Our solver POSTs to `/__qrator/validate?pow=168&
nonce=&qsessid=` — **with empty nonce and qsessid**. The script is
running but not producing values.

### Step 1 — Capture the script (30 min)

```bash
curl -s 'https://www.dns-shop.ru/' -o /tmp/dns-shop.html
# Extract the inline script block(s):
python3 -c "
import re
html = open('/tmp/dns-shop.html').read()
scripts = re.findall(r'<script[^>]*>(.*?)</script>', html, re.DOTALL)
for i, s in enumerate(scripts):
    if 'qrator' in s.lower() or 'nonce' in s.lower() or 'pow' in s.lower():
        open(f'/tmp/qrator-{i}.js', 'w').write(s)
        print(f'wrote /tmp/qrator-{i}.js ({len(s)} bytes)')
"
ls -la /tmp/qrator-*.js
```

### Step 2 — Pretty-print and analyze (1-2h)

```bash
# Requires 'prettier' installed: npm i -g prettier
prettier --parser babel /tmp/qrator-0.js > /tmp/qrator-0.pretty.js
wc -l /tmp/qrator-0.pretty.js
```

Look for:
- The `nonce` computation (usually a loop that hashes until a
  condition is met)
- The `qsessid` source (usually a global variable or cookie)
- Any capability checks (`typeof Worker`, `navigator.hardwareConcurrency`,
  `window.crypto.subtle`, etc.)

### Step 3 — Run under instrumentation (1-2h)

Write a probe test similar to `adidas_sensor_api_probes.rs`:

**File**: `crates/browser/tests/dns_shop_qrator_probe.rs` (new)

```rust
#[tokio::test]
#[ignore]
async fn qrator_script_probe() {
    // Load /tmp/qrator-0.js
    // Install wrappers on:
    //   - globalThis.crypto.subtle.digest
    //   - globalThis.crypto.getRandomValues
    //   - Math.random
    //   - BigInt / BigInt64Array
    //   - Date.now, performance.now
    //   - setTimeout, setInterval
    //   - document.cookie (get and set)
    //   - fetch, XMLHttpRequest
    // Run the script
    // Drain event loop 5s
    // Dump:
    //   - which APIs were called
    //   - what was set on document.cookie
    //   - what URLs were fetched
    //   - any async errors
}
```

Run it and see what's happening.

### Step 4 — Fix the missing capability (2-4h)

Expected findings (prioritized by likelihood):

**Most likely**: the script needs a capability we don't implement.
Common QRATOR-specific requirements:
- `crypto.subtle.digest` with large inputs — we have this via
  `op_crypto_digest`.
- `BigInt` arithmetic — V8 has this natively.
- `Math.random` with specific entropy — V8 has this.
- A specific `document.cookie` value (`qsessid`) that was set by a
  previous `Set-Cookie` header from the server. If we're not parsing
  the response headers correctly, qsessid is undefined in JS.

**Action for last case**: verify that `Set-Cookie: qsessid=...` from
the initial GET is being stored in our cookie jar and readable via
`document.cookie` from JS.

```bash
# Capture the full response:
curl -sv 'https://www.dns-shop.ru/' 2>&1 | grep -i 'set-cookie'
```

If the server sets `qsessid`, make sure our `build_response` parses
multi-value Set-Cookie correctly (task #9 was about this — verify it
still works for multi-header Set-Cookie responses).

### Step 5 — Verify

```bash
cargo test -p browser --test blocker_rigorous_probe tier05_blockers_all \
    -- --ignored --test-threads=1 --nocapture 2>&1 | grep dns_shop
```

Expected: the POST to `/__qrator/validate` now has real nonce and
qsessid values, and the response is 200 instead of 403.

## Related tasks

- #14 Pass QRATOR challenge on dns-shop.ru [pending]
- #13 Debug probe dns-shop.ru [done — identified the empty-nonce
  issue]
- #12 Debug probe ozon.ru [done]
- #11 Debug probe ya.ru [done]

## What this sprint gives you

After shipping all three (ozon, yandex, dns-shop):

- **24-26/24 deep-path passing** (all current + ozon + yandex;
  dns-shop isn't in deep_path so it doesn't count there but we gain
  it in the blocker probe)
- **3/8 blocker sites moved to PASS** — leaves just adidas,
  homedepot, canadagoose, hyatt, wildberries as the hard ones
- **Concrete visible progress** without committing to 100+ hours of
  capability work

Good next milestone to ship before starting T1.2.
