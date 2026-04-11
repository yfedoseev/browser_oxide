# 06 — Test infrastructure: how to measure progress

This file is the runbook for all the probes, regression tests, and debug
tools that exist in browser_oxide. New contributors should read this
before changing anything so they know how to check their work.

## The hierarchy of tests

browser_oxide has three kinds of tests, in increasing strictness:

1. **Unit tests** — `cargo test --workspace -- --test-threads=1`. ~600
   tests covering DOM parsing, CSS selectors, layout, chrome_compat (296
   tests checking specific JS APIs), computed style, etc. These must
   always be green.
2. **Network tests** — `#[ignore]` tests that hit live sites. Run with
   `--ignored` and single-threaded. These are the source of truth for
   "does site X work?"
3. **Debug probes** — targeted investigation tools that capture specific
   data from specific sites. Not regression tests; one-shot diagnostics.

## The non-network regression run

```bash
cd /home/yfedoseev/projects/browser_oxide
cargo test --workspace -- --test-threads=1
```

Must be green before you commit anything. Expected output: "296 passed;
0 failed" for chrome_compat.rs, plus various other passing counts across
20+ test binaries. If anything is red, fix it before moving on.

**Typical runtime**: 1-3 minutes on a modern machine. The slowest chunk
is chrome_compat.rs at about 13-24 seconds.

## The deep-path validation run (the 22 passing sites)

```bash
cargo test -p browser --test deep_path_validation -- \
    --ignored --test-threads=1 --nocapture
```

This is the most important regression gate. It hits 24 real sites and
checks both the landing page AND a real deep path (product page, search,
feed, etc.) with content markers. Current expected output (as of
2026-04-10):

```
[HOLD   ] 22 sites
[DEGRADE] 2 sites (amazon dead URL, crunchbase /search 403)
```

Any additional DEGRADE or both-fail after your changes = regression.

**Typical runtime**: 30-60 seconds. Single sequential probe per site.

**Source file**: `crates/browser/tests/deep_path_validation.rs`

## The blocker probe (the 8 failing sites)

```bash
cargo test -p browser --test blocker_rigorous_probe tier05_blockers_all \
    -- --ignored --test-threads=1 --nocapture
```

This is the strict content-marker probe for the 8 known-problematic
sites: adidas, homedepot, canadagoose, hyatt, wildberries, dns_shop,
ozon, yandex. It runs each site through both a baseline (raw `get()`)
and the full solver (`Page::navigate_with_challenges`).

Expected output as of 2026-04-10:

```
Summary: 0/8 solver-PASS, 0 baseline-only PASS, 8 FAIL
```

Any site moving to WIN = progress.

**Typical runtime**: 8-10 minutes (slow because some sites have long
solver drains).

**Source file**: `crates/browser/tests/blocker_rigorous_probe.rs`

**Stability note**: `homedepot` produced one spurious `solver=PASS
(973378b)` in the first session run but then stabilized at FAIL in
subsequent runs. Akamai's verdict is stochastic near the borderline.
Run the probe 3 times before believing any single result. Helper:

```bash
bash /tmp/run-probe.sh  # runs 3 times, prints summary lines
```

## Per-site debug probes

These are not regression tests — they're diagnostic tools for specific
sites. Look in `crates/browser/tests/` for the full list:

| File | What it captures |
|---|---|
| `adidas_cookie_replay.rs` | GETs adidas with a captured Playwright cookie set to test cookie portability |
| `adidas_fetch_sensor_vm.rs` | Downloads the current Akamai sensor VM to /tmp for inspection |
| `adidas_sensor_api_probes.rs` | Instruments every Ctx2D/Canvas/AudioContext method + every globalThis getter + Function.prototype.toString, then runs the captured sensor VM under instrumentation |
| `adidas_sensor_capture.rs` | Runs the full navigate_with_challenges flow against adidas with the `BOXIDE_DUMP_POST_DIR` env var, dumping every POST body to disk |
| `wildberries_solver_diag.rs` | Traces the WBAAS solver flow with cookie state logging |
| `blocker_rigorous_probe.rs` | The 8-site rigorous probe (this is the regression gate above) |
| `debug_blocked.rs` | Old per-site debug probes (tripadvisor, airbnb, amazon, ozon, ya.ru, dns-shop) |
| `tier0_kasada.rs` | Kasada POC probes (kick.com, canadagoose, hyatt) |

Run any of them with:

```bash
cargo test -p browser --test <file_name> -- --ignored --test-threads=1 --nocapture
```

## The BOXIDE_DUMP_POST_DIR env var

Added to `crates/net/src/lib.rs::post_bytes_with_headers`. Setting it
causes every HTTP POST to be written to disk as `NNN.body` (raw request
body) and `NNN.meta.json` (URL + request headers).

Usage:

```bash
BOXIDE_DUMP_POST_DIR=/tmp/my-capture cargo test -p browser \
    --test adidas_sensor_capture -- --ignored --test-threads=1 --nocapture
```

Then look at `/tmp/my-capture/`:

```
001.body       277 bytes  (initial ping POST)
001.meta.json  metadata
002.body       3762 bytes (full sensor POST)
002.meta.json  metadata
```

Use Python or jq to diff body contents across runs:

```python
import json
with open('/tmp/my-capture/002.body') as f:
    sd = json.loads(f.read())['sensor_data']
parts = sd.split(';')
for i, p in enumerate(parts[:10]):
    print(f'[{i}] ({len(p)}) {p[:60]!r}')
```

## The API probe pattern (for investigating a new site)

When investigating a new site, the highest-signal approach is to:

1. Download its challenge script via a one-shot test
   (like `adidas_fetch_sensor_vm.rs`)
2. Load it into a probe test that wraps every API the script might touch
   (like `adidas_sensor_api_probes.rs`)
3. Run the script under instrumentation
4. Inspect the access counts to see which APIs it uses
5. Only then decide which capability to fix

This is how we discovered that the adidas sensor VM reads `WeakRef` 7
times but never calls any canvas pixel extraction method. Don't guess —
instrument. Template to copy:

```rust
use event_loop::BrowserEventLoop;
use js_runtime::BrowserJsRuntime;
use std::time::Duration;

#[tokio::test]
#[ignore]
async fn probe_new_site() {
    let script = std::fs::read_to_string("/tmp/the_challenge.js").unwrap();
    let dom = html_parser::parse_html(
        "<html><head></head><body><div id=\"out\"></div></body></html>",
    );
    let mut evloop = BrowserEventLoop::new(BrowserJsRuntime::with_profile(
        dom,
        stealth::chrome_130_macos(),
    ));

    // Install global access counters before running the script.
    evloop.execute_script(r#"
        globalThis.__apiProbes = {};
        function _probe(obj, key) {
            const original = obj[key];
            globalThis.__apiProbes[key] = { count: 0, type: typeof original };
            Object.defineProperty(obj, key, {
                configurable: true,
                get() { globalThis.__apiProbes[key].count++; return original; },
                set(v) {},
            });
        }
        // Probe the APIs you suspect.
        _probe(globalThis, 'Worker');
        _probe(globalThis, 'OffscreenCanvas');
        _probe(globalThis, 'crossOriginIsolated');
        if (globalThis.navigator) {
            _probe(navigator, 'permissions');
            _probe(navigator, 'connection');
            _probe(navigator, 'userAgentData');
        }
    "#).unwrap();

    evloop.execute_script(&format!("try {{ (function(){{{script}}})(); }} catch(e) {{ console.error(e) }}")).unwrap();
    evloop.run_until_idle(Duration::from_secs(5)).await.unwrap();

    let probes = evloop.execute_script(
        "JSON.stringify(globalThis.__apiProbes)"
    ).unwrap();
    println!("{probes}");
}
```

## Cookie diagnosis: the `_abck` trajectory

`page.rs::navigate_with_challenges` (temporary; will be removed in the
refactor) prints the full `_abck` cookie after each solver pass:

```
_abck FULL (797 chars): E283832BD...~-1~YAAQZZ...~-1~-1~1775876009~AAQAAAAF...
_abck still ~-1~ (untrusted)
```

Trust slots meaning:

| Value | Meaning |
|---|---|
| `~-1~` | Untrusted / bot suspected |
| `~0~` | Valid / stop-signal set (trusted) |
| `~1~` | Intermediate (rare, some tenants) |

Source: Hyper Solutions public docs, Kameleo glossary.

## Audio reference test

```bash
cargo test -p canvas --test audio_reference reports_current_sum -- --nocapture
```

Prints our current sum for the CreepJS probe pipeline against Chrome's
reference. Expected:

```
sum(abs(data[4500..5000])) = 124.03601119903033
chrome reference            = 124.04347527516074
absolute delta              = 0.007464...
relative delta              = 0.00006017...
```

Any delta > 0.5 means the audio calibration drifted. Investigate before
assuming it's OK.

## Worker integration tests

```bash
cargo test -p js_runtime --test worker -- --test-threads=1
cargo test -p browser --test worker_page_integration -- --test-threads=1
```

These verify that `new Worker(URL.createObjectURL(blob))` works end-to-
end and that the Worker can receive + respond to messages. Must stay
green after any work on `worker_ext.rs` or the bootstrap files.

## Clippy and format

```bash
cargo clippy --workspace -- -D warnings  # strict mode
cargo fmt --all -- --check               # check only
cargo fmt --all                          # apply
```

As of 2026-04-10, there are ~31 warnings in the js_runtime lib, mostly
from unused args in stub functions (e.g., `webgl_ext.rs::op_clearColor`'s
r/g/b/a). These are pre-existing and non-blocking. New warnings you
introduce should be fixed.

## The "quick full verification" command

```bash
cd /home/yfedoseev/projects/browser_oxide && \
cargo test --workspace -- --test-threads=1 && \
cargo test -p browser --test deep_path_validation -- \
    --ignored --test-threads=1 --nocapture 2>&1 | grep -E "HOLD|DEGRADE|both-fail" && \
cargo test -p browser --test blocker_rigorous_probe tier05_blockers_all -- \
    --ignored --test-threads=1 --nocapture 2>&1 | grep -E "WIN|BASE|FAIL" | tail -15
```

Runs in ~10-15 minutes. Tells you: green workspace + 22 passing sites
hold + 8 blocker sites current state.
