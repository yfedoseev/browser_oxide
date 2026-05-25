# 04 — Tooling spec (BLOCKING for Phases 1-3)

Phases 1-3 (`05_SPA_HYDRATION_CLUSTER`, `06_AWS_WAF_SOLVER`,
`07_DATADOME_PRIMITIVES`) all hinge on **per-site evidence** — a
fetch chain, a script error, a cookie write, a `__pendingNavigation`
flip. Without that evidence, every debug session is the same shape:
add an `eprintln!`, rebuild release (~3 min), re-run, scroll log,
guess again. This chapter specs the three tools that turn that loop
into structured artifacts a human (or another agent) can diff in
seconds.

Three deliverables:

1. **`sweep_metrics --capture <name>`** — a single-URL diagnostic
   mode that emits 7 files into a deterministic directory layout.
2. **`capture_camoufox.py`** — same shape, Camoufox-side, using
   Playwright tracing + custom hooks.
3. **`capture_diff.py BO_DIR CAMOUFOX_DIR`** — auto-diff that turns
   "what does Camoufox do that we don't" into a one-page summary.

> Read first: `03_BENCHMARK_METHODOLOGY.md` (the sweep harness this
> extends), `02_GAP_ANALYSIS.md` (the 10 sites whose captures will
> drive the diagnosis).

## 1. Directory layout

Every capture writes into:

```
/tmp/capture/<engine>/<profile>/<site_name>/
```

Where `<engine>` ∈ `{bo, camoufox}`, `<profile>` ∈ the 4 BO profile
names (Camoufox has one effective profile — use `default`), and
`<site_name>` is the corpus `name` field (e.g. `reddit`,
`amazon-de`).

The 7 files per capture:

```
body.html                # final document.documentElement.outerHTML
fetches.json             # every HTTP request + response observed
script_errors.json       # every JS exception with stack
cookie_writes.json       # every document.cookie= and every Set-Cookie response header
pending_nav_timeline.json # every __pendingNavigation set, with timestamp + setter
console.txt              # console.log/warn/error captured in order
iter_summary.json        # per navigate-loop iteration: body_size, ready_state, etc.
dom_snapshot.json        # final DOM tree as nested {tag, id, classes, n_children}
```

Files are written atomically (`.tmp` then `rename`) so a partial
capture during a crash doesn't poison a later diff.

## 2. BO capture mode — `sweep_metrics --capture`

### Invocation

```bash
# RUN THIS
target/release/examples/sweep_metrics \
    chrome_148_macos \
    /tmp/corpus.json \
    /tmp/unused.json \
    --capture reddit
# Outputs land in /tmp/capture/bo/chrome_148_macos/reddit/
```

The harness:
1. Looks up `reddit` in the corpus JSON.
2. Routes through `Page::navigate_with_init_solvers` (the production
   call) with a **capture context** injected via `OpState`.
3. Writes the 7 files into the deterministic directory.
4. Exits non-zero only if `Page::navigate` panics (so the diff tool
   can still consume partial captures from sites that fail).

### Rust changes — `sweep_metrics.rs`

Add a `Capture` mode that wraps the production call. File path:
`crates/browser/examples/sweep_metrics.rs`. Approximate skeleton:

```rust
// crates/browser/examples/sweep_metrics.rs additions
struct CaptureCtx {
    out_dir: PathBuf,
}

async fn run_capture(profile: stealth::StealthProfile, site: &Site) {
    let out_dir = PathBuf::from(format!(
        "/tmp/capture/bo/{}/{}", profile_name_for(&profile), site.name
    ));
    std::fs::create_dir_all(&out_dir).expect("mkdir");

    // 1. Install a CaptureSink that op_fetch / op_cookie_set / console
    //    ops will tee into. The sink is wired via DomState so any
    //    extension can record to it without changing op signatures.
    browser::capture::install_sink(out_dir.clone());

    // 2. Drive the production navigate path.
    let t0 = Instant::now();
    let res = browser::Page::navigate(&site.url, profile.clone(), 3).await;
    let total_ms = t0.elapsed().as_millis() as u64;

    // 3. Final body + DOM snapshot.
    match res {
        Ok(mut page) => {
            std::fs::write(out_dir.join("body.html"), page.content())
                .expect("write body");
            std::fs::write(
                out_dir.join("dom_snapshot.json"),
                serde_json::to_vec_pretty(&page.dom_snapshot()).unwrap(),
            ).expect("write dom_snapshot");
            // iter_summary and the rest are flushed by the sink on drop.
        }
        Err(e) => {
            std::fs::write(out_dir.join("body.html"),
                format!("<!-- navigate failed: {} -->\n", e)).ok();
        }
    }
    browser::capture::flush_sink(&out_dir, total_ms);
}
```

### Rust changes — new `browser::capture` module

New file: `crates/browser/src/capture.rs`. Function signatures:

```rust
//! Per-site capture sink — instrument the production navigate path
//! without changing op signatures. Wired into DomState; ops tee
//! records here whenever a sink is installed.
pub fn install_sink(out_dir: std::path::PathBuf);
pub fn flush_sink(out_dir: &std::path::Path, total_ms: u64);

// Recorded by op_fetch (fetch_ext.rs)
pub fn record_fetch(
    method: &str, url: &str, status: u16,
    req_headers: &[(String, String)],
    resp_headers: &std::collections::HashMap<String, String>,
    body_size_recv: usize, request_body_size: usize,
    ms_from_nav_start: u64, ms_response: u64,
);

// Recorded by op_cookie_set + the net layer when Set-Cookie comes in
pub fn record_cookie_write(
    source: &str, // "document.cookie" | "set-cookie"
    url: &str, raw: &str, ms_from_nav_start: u64,
);

// Recorded by op_console_log/warn/error
pub fn record_console(level: &str, msg: &str, ms_from_nav_start: u64);

// Recorded by the V8 exception handler in runtime.rs
pub fn record_script_error(message: &str, stack: &str, source_url: &str,
    line: u32, col: u32, ms_from_nav_start: u64);

// Recorded by every JS that sets globalThis.__pendingNavigation
// (instrument the property setter in stealth_bootstrap.js)
pub fn record_pending_nav(
    url: &str, method: &str, body: &str, setter: &str,
    ms_from_nav_start: u64,
);

// Recorded by the navigate_loop iteration boundary in page.rs
pub fn record_iter(
    iter: u32, body_size: usize, mount_children: usize,
    ready_state: &str, body_html_sha256: &str, ms_from_nav_start: u64,
);
```

Module behavior:
- Uses a process-wide `OnceLock<Mutex<Option<Sink>>>` so the sink
  install is opt-in and zero-cost when unused.
- Records are buffered into a `Vec<Record>` per kind and serialized
  on `flush_sink`.
- Default: NOT installed. Sweep mode is unaffected.

### Integration points

| File | Where to add the hook |
|---|---|
| `crates/js_runtime/src/extensions/fetch_ext.rs:231-381` | At the bottom of `op_fetch`, after `let final_resp = ...` and before `Ok(final_resp)`: `browser::capture::record_fetch(&method, &url, resp.status, &extra_headers, &resp.headers, resp.body.len(), body_bytes.len(), ms_from_nav_start, ms_response);` |
| `crates/js_runtime/src/extensions/fetch_ext.rs:397-410` | `op_cookie_set`: tee into `record_cookie_write("document.cookie", &url, &cookie, ms)` |
| `crates/net/src/lib.rs` (Response handling) | After every response, walk `resp.headers.get_all("set-cookie")` and call `record_cookie_write("set-cookie", &url, raw, ms)` |
| `crates/js_runtime/src/extensions/console_ext.rs:5-26` | At the top of each `op_console_*`, `browser::capture::record_console("log", &msg, ms);` |
| `crates/js_runtime/src/runtime.rs` (exception handler / `run_event_loop` error path) | When V8 surfaces an exception, call `record_script_error(...)` with the message, stack, source, line, col |
| `crates/js_runtime/src/js/stealth_bootstrap.js` | Install a `Object.defineProperty(globalThis, '__pendingNavigation', { set(v) { op_capture_pending_nav(JSON.stringify(v)); _real = v; }, get() { return _real; } })` shim. Wire `op_capture_pending_nav` as a new op_2 op (`#[op2(fast)] pub fn op_capture_pending_nav(#[string] payload: String)`) that calls `record_pending_nav`. |
| `crates/browser/src/page.rs:1545-2700` (`navigate_loop_internal`) | At every iteration boundary, after the per-iter body is computed: `browser::capture::record_iter(iter as u32, body.len(), mount_children, ready_state, body_html_sha256, ms_from_nav_start);` |

The sink uses ATOMIC writes (`tempfile + persist`) so a panic mid-write
doesn't corrupt the JSON.

### JSON schemas

#### `fetches.json`

```json
{
  "schema_version": 1,
  "n_fetches": 47,
  "fetches": [
    {
      "i": 0,
      "ms_from_nav_start": 0,
      "ms_response": 142,
      "method": "GET",
      "url": "https://www.reddit.com/",
      "status": 200,
      "request_headers": {
        "user-agent": "Mozilla/5.0 ... Chrome/148.0.0.0 ...",
        "accept": "text/html,...",
        "accept-language": "en-US,en;q=0.9",
        "sec-fetch-mode": "navigate"
      },
      "request_body_size": 0,
      "response_headers": {
        "content-type": "text/html; charset=UTF-8",
        "set-cookie": "redirect_count=1; Path=/; HttpOnly"
      },
      "body_size_recv": 8424
    }
  ]
}
```

#### `script_errors.json`

```json
{
  "schema_version": 1,
  "n_errors": 2,
  "errors": [
    {
      "i": 0,
      "ms_from_nav_start": 312,
      "message": "TypeError: Cannot read properties of null (reading 'value')",
      "stack": "TypeError: ...\n    at HTMLFormElement.<anonymous> (https://www.reddit.com/:7:42)",
      "source_url": "https://www.reddit.com/",
      "line": 7,
      "col": 42
    }
  ]
}
```

#### `cookie_writes.json`

```json
{
  "schema_version": 1,
  "n_writes": 5,
  "writes": [
    {
      "i": 0,
      "ms_from_nav_start": 142,
      "source": "set-cookie",
      "url": "https://www.reddit.com/",
      "raw": "redirect_count=1; Path=/; HttpOnly"
    },
    {
      "i": 1,
      "ms_from_nav_start": 318,
      "source": "document.cookie",
      "url": "https://www.reddit.com/",
      "raw": "edgebucket=ABC; Path=/; Domain=.reddit.com"
    }
  ]
}
```

#### `pending_nav_timeline.json`

```json
{
  "schema_version": 1,
  "n_flips": 3,
  "flips": [
    {
      "i": 0,
      "ms_from_nav_start": 332,
      "url": "https://www.reddit.com/",
      "method": "POST",
      "body_size": 142,
      "setter": "HTMLFormElement.requestSubmit (dom_bootstrap.js:1108)"
    }
  ]
}
```

#### `console.txt` (newline-delimited, NOT JSON; humans read this most)

```
[t=0012ms] [log] [DOM] inline eval start (8 bytes)
[t=0015ms] [log] [DOM] DOMContentLoaded fired
[t=0089ms] [warn] [recaptcha] fallback worker not available
[t=0312ms] [error] TypeError: Cannot read properties of null (reading 'value')
```

#### `iter_summary.json`

```json
{
  "schema_version": 1,
  "total_ms": 1832,
  "n_iters": 2,
  "iters": [
    {
      "iter": 0,
      "ms_from_nav_start": 0,
      "body_size": 8424,
      "mount_children": 3,
      "ready_state": "complete",
      "body_html_sha256": "ab12...",
      "vendor_detected": null,
      "pending_nav_set": true
    },
    {
      "iter": 1,
      "ms_from_nav_start": 480,
      "body_size": 1142312,
      "mount_children": 87,
      "ready_state": "complete",
      "body_html_sha256": "cd34...",
      "vendor_detected": null,
      "pending_nav_set": false
    }
  ]
}
```

#### `dom_snapshot.json`

Recursive tree:

```json
{
  "schema_version": 1,
  "tag": "html",
  "id": "",
  "classes": [],
  "n_children": 2,
  "children": [
    {"tag": "head", "id": "", "classes": [], "n_children": 8, "children": [...]},
    {"tag": "body", "id": "", "classes": ["mob"], "n_children": 12, "children": [...]}
  ]
}
```

`Page::dom_snapshot()` is a new accessor on `crates/browser/src/page.rs`
that walks the live arena and yields a `serde_json::Value`. Existing
`Page::content()` returns serialized HTML — `dom_snapshot()` is
structurally richer for cross-engine diffing.

## 3. Camoufox capture — `benchmarks/capture_camoufox.py`

Same output shape as BO. Uses Playwright tracing API + custom listeners
to capture all 7 file types. Skeleton:

```python
#!/usr/bin/env python3
"""Camoufox-side capture matching the BO capture mode output shape.

Usage: capture_camoufox.py <site_name> [out_root]
   out_root defaults to /tmp/capture/camoufox/default/
"""
import asyncio, hashlib, json, os, sys, time
from pathlib import Path
from camoufox.async_api import AsyncCamoufox

CORPUS = json.load(open(os.environ.get("CORPUS_FILE", "/tmp/corpus.json")))
OUT_ROOT = Path(os.environ.get("CAPTURE_OUT_ROOT",
    "/tmp/capture/camoufox/default"))
NAV_TIMEOUT_MS = int(os.environ.get("NAV_TIMEOUT_MS", "45000"))
SETTLE_MS = int(os.environ.get("SETTLE_MS", "5000"))

async def capture(site_name: str):
    site = next(s for s in CORPUS if s["name"] == site_name)
    out = OUT_ROOT / site_name
    out.mkdir(parents=True, exist_ok=True)

    fetches, errors, cookies, console_lines, pending = [], [], [], [], []
    t0 = time.perf_counter()

    def ms(): return int((time.perf_counter() - t0) * 1000)

    async with AsyncCamoufox(headless=True) as browser:
        page = await browser.new_page()

        # --- Request / response capture ---
        req_start = {}  # id -> ms
        def on_request(req):
            req_start[req] = ms()
        def on_response(resp):
            req = resp.request
            try:
                body_bytes = -1  # filled later via body() if affordable
            except Exception:
                body_bytes = -1
            fetches.append({
                "i": len(fetches),
                "ms_from_nav_start": req_start.get(req, 0),
                "ms_response": ms(),
                "method": req.method,
                "url": req.url,
                "status": resp.status,
                "request_headers": dict(req.headers),
                "request_body_size": len(req.post_data or ""),
                "response_headers": dict(resp.headers),
                "body_size_recv": body_bytes,
            })
            # Tee Set-Cookie headers
            for raw in resp.headers_array():
                if raw["name"].lower() == "set-cookie":
                    cookies.append({
                        "i": len(cookies), "ms_from_nav_start": ms(),
                        "source": "set-cookie", "url": resp.url,
                        "raw": raw["value"],
                    })
        page.on("request", on_request)
        page.on("response", on_response)

        # --- Console / errors ---
        page.on("console", lambda m: console_lines.append(
            f"[t={ms():>4d}ms] [{m.type}] {m.text}"))
        page.on("pageerror", lambda e: errors.append({
            "i": len(errors), "ms_from_nav_start": ms(),
            "message": str(e), "stack": getattr(e, "stack", ""),
            "source_url": "", "line": 0, "col": 0,
        }))

        # --- document.cookie writes — inject a setter shim before nav ---
        await page.add_init_script("""
        (function() {
          const orig = Object.getOwnPropertyDescriptor(
            Document.prototype, 'cookie');
          let last = '';
          Object.defineProperty(Document.prototype, 'cookie', {
            get() { return orig.get.call(this); },
            set(v) {
              window.__capCookieWrites = window.__capCookieWrites || [];
              window.__capCookieWrites.push({t: performance.now(), raw: v});
              orig.set.call(this, v);
            }
          });
        })();
        """)

        # --- Navigate ---
        try:
            await page.goto(site["url"], wait_until="load",
                timeout=NAV_TIMEOUT_MS)
            await page.wait_for_timeout(SETTLE_MS)
        except Exception as e:
            errors.append({
                "i": len(errors), "ms_from_nav_start": ms(),
                "message": f"goto: {e}", "stack": "", "source_url": "",
                "line": 0, "col": 0,
            })

        # --- Body + DOM ---
        html = await page.content()
        (out / "body.html").write_text(html)
        body_sha = hashlib.sha256(html.encode()).hexdigest()[:16]

        dom = await page.evaluate("""() => {
          const walk = (n, d) => {
            if (!n || d > 6) return null;
            return {
              tag: (n.tagName || '').toLowerCase(),
              id: n.id || '',
              classes: n.classList ? [...n.classList] : [],
              n_children: n.children ? n.children.length : 0,
              children: n.children ? [...n.children].slice(0, 20)
                .map(c => walk(c, d + 1)).filter(Boolean) : [],
            };
          };
          return walk(document.documentElement, 0);
        }""")
        (out / "dom_snapshot.json").write_text(
            json.dumps({"schema_version": 1, **dom}, indent=1))

        # --- Drain captured document.cookie writes ---
        cap = await page.evaluate("window.__capCookieWrites || []")
        for c in cap:
            cookies.append({
                "i": len(cookies), "ms_from_nav_start": int(c["t"]),
                "source": "document.cookie", "url": page.url,
                "raw": c["raw"],
            })

        # --- pendingNavigation: Camoufox has no equivalent; record any
        #     location.assign / replace / href via init-script shim instead.
        await page.evaluate("""() => {}""")

    total_ms = ms()

    # --- Write all the artifact files ---
    (out / "fetches.json").write_text(json.dumps(
        {"schema_version": 1, "n_fetches": len(fetches),
         "fetches": fetches}, indent=1))
    (out / "script_errors.json").write_text(json.dumps(
        {"schema_version": 1, "n_errors": len(errors),
         "errors": errors}, indent=1))
    (out / "cookie_writes.json").write_text(json.dumps(
        {"schema_version": 1, "n_writes": len(cookies),
         "writes": cookies}, indent=1))
    (out / "pending_nav_timeline.json").write_text(json.dumps(
        {"schema_version": 1, "n_flips": len(pending),
         "flips": pending}, indent=1))
    (out / "console.txt").write_text("\n".join(console_lines) + "\n")
    (out / "iter_summary.json").write_text(json.dumps({
        "schema_version": 1, "total_ms": total_ms, "n_iters": 1,
        "iters": [{
            "iter": 0, "ms_from_nav_start": 0,
            "body_size": len(html), "mount_children": dom["n_children"],
            "ready_state": "complete",
            "body_html_sha256": body_sha,
            "vendor_detected": None, "pending_nav_set": False,
        }]
    }, indent=1))

if __name__ == "__main__":
    asyncio.run(capture(sys.argv[1]))
```

> The `pending_nav_timeline.json` from Camoufox will usually be empty
> (real browsers don't expose a "next navigation queued" flag the way
> `__pendingNavigation` does in BO). That's fine — the diff tool only
> compares per-file shapes, not contents, for files that are
> engine-specific.

## 4. Diff tool — `benchmarks/capture_diff.py`

### Invocation

```bash
# RUN THIS
python3 benchmarks/capture_diff.py \
    /tmp/capture/bo/chrome_148_macos/reddit \
    /tmp/capture/camoufox/default/reddit \
    > /tmp/reddit_diff.txt
cat /tmp/reddit_diff.txt
```

### What it surfaces

The diff is opinionated. It does NOT line-diff the files (that would
be useless — UUIDs, timestamps, headers reordered). It pairs and
summarizes:

#### Section 1 — Fetches

For each fetch in either side, pair by `(method, url_prefix_until_query)`.

- **Only in BO**: fetch URLs BO emits that Camoufox doesn't. These
  are false-positive emits (we're fetching something a real browser
  wouldn't).
- **Only in Camoufox**: fetch URLs Camoufox emits that BO doesn't.
  This is the gold finding — the missing fetch is usually the SPA
  hydration call or the WAF token POST.
- **Status differs**: same URL, one side got 200 and the other got
  403 / 429 / 0. Reveals per-engine WAF treatment.
- **Body size differs by > 2× or > 10 KB**: same URL, vastly
  different body — usually a CSR/SSR fork.

#### Section 2 — Script errors

- All BO errors not present in Camoufox.
- All Camoufox errors not present in BO (usually empty).

#### Section 3 — Cookie writes

- **Only in BO**: cookies we set that Camoufox doesn't (likely a
  bootstrap-injected stealth cookie that the WAF flags).
- **Only in Camoufox**: cookies Camoufox got that BO didn't (the
  **solved-cookie signal** — DataDome's `datadome=`, AWS WAF's
  `aws-waf-token`, CF's `cf_clearance`).

#### Section 4 — Iter / body summary

- BO total iters vs Camoufox (BO loops on `__pendingNavigation`,
  Camoufox doesn't; the comparison is "did BO finish iteration N or
  did it stop at iteration 1").
- Final body size ratio.
- DOM root child count.

#### Section 5 — Verdict

A one-line classification of the gap:

```
GAP CLASSIFICATION: missing-fetch
  - Camoufox issued 14 fetches we didn't.
  - Top missing prefix: https://www.recaptcha.net/recaptcha/api2/anchor
  - Most likely cluster: SPA hydration (cluster 05 in 02_GAP_ANALYSIS.md)
  - Suggested debug step: re-run BO capture with
      RUST_LOG=js_runtime::extensions::worker_ext=trace
    and inspect console.txt for grecaptcha.execute failures.
```

### Skeleton

```python
#!/usr/bin/env python3
"""Pairwise capture diff for BO vs Camoufox.

Usage: capture_diff.py BO_DIR CAMOUFOX_DIR
Outputs: human-readable report on stdout.
"""
import json, sys
from pathlib import Path
from urllib.parse import urlsplit

def load(d, name):
    p = Path(d) / name
    if not p.exists(): return None
    return json.loads(p.read_text())

def url_key(url: str) -> str:
    """Pair-by key — host + path, strip query string and trailing slash."""
    s = urlsplit(url)
    return f"{s.scheme}://{s.netloc}{s.path.rstrip('/')}"

def diff_fetches(bo, cf):
    bo_by = {}
    for f in (bo or {}).get("fetches", []):
        bo_by.setdefault(url_key(f["url"]), []).append(f)
    cf_by = {}
    for f in (cf or {}).get("fetches", []):
        cf_by.setdefault(url_key(f["url"]), []).append(f)
    only_bo = sorted(set(bo_by) - set(cf_by))
    only_cf = sorted(set(cf_by) - set(bo_by))
    status_diff = []
    for k in set(bo_by) & set(cf_by):
        bs = bo_by[k][0]["status"]
        cs = cf_by[k][0]["status"]
        if bs != cs:
            status_diff.append((k, bs, cs))
    return only_bo, only_cf, status_diff

def diff_cookies(bo, cf):
    bo_set = {(c["source"], c["raw"].split(";")[0])
              for c in (bo or {}).get("writes", [])}
    cf_set = {(c["source"], c["raw"].split(";")[0])
              for c in (cf or {}).get("writes", [])}
    return sorted(bo_set - cf_set), sorted(cf_set - bo_set)

def classify_gap(only_cf_fetches, only_cf_cookies):
    if any("captcha-delivery.com" in u for u in only_cf_fetches):
        return ("datadome", "DataDome solve fetch missing — chapter 07")
    if any("awswaf.com" in u for u in only_cf_fetches):
        return ("aws-waf", "AWS WAF token fetch missing — chapter 06")
    if any("recaptcha" in u for u in only_cf_fetches):
        return ("spa-recaptcha", "recaptcha worker / hydration — chapter 05")
    if any(c[1].startswith("datadome=") for c in only_cf_cookies):
        return ("datadome-cookie", "DataDome solved-cookie signal missing")
    if any(c[1].startswith("aws-waf-token=") for c in only_cf_cookies):
        return ("aws-waf-cookie", "AWS WAF solved-cookie signal missing")
    if only_cf_fetches:
        return ("missing-fetch", f"Camoufox issued {len(only_cf_fetches)} fetches we didn't")
    return ("unknown", "no obvious fetch / cookie delta")

def main():
    bo, cf = sys.argv[1], sys.argv[2]
    f_bo, f_cf = load(bo, "fetches.json"), load(cf, "fetches.json")
    only_bo, only_cf, status_diff = diff_fetches(f_bo, f_cf)
    c_bo, c_cf = load(bo, "cookie_writes.json"), load(cf, "cookie_writes.json")
    only_bo_c, only_cf_c = diff_cookies(c_bo, c_cf)
    e_bo, e_cf = load(bo, "script_errors.json"), load(cf, "script_errors.json")
    i_bo, i_cf = load(bo, "iter_summary.json"), load(cf, "iter_summary.json")
    print("=" * 72)
    print(f"CAPTURE DIFF — {bo}  vs  {cf}")
    print("=" * 72)
    print(f"\n--- Fetches ---")
    print(f"  BO  : {len((f_bo or {}).get('fetches', []))} fetches")
    print(f"  CF  : {len((f_cf or {}).get('fetches', []))} fetches")
    print(f"  Only in BO ({len(only_bo)}):")
    for u in only_bo[:20]: print(f"    + {u}")
    print(f"  Only in Camoufox ({len(only_cf)}):")
    for u in only_cf[:20]: print(f"    - {u}")
    print(f"  Status differs ({len(status_diff)}):")
    for u, b, c in status_diff[:10]:
        print(f"    {u}\n      BO={b} CF={c}")
    print(f"\n--- Cookies ---")
    print(f"  Only in BO ({len(only_bo_c)}):")
    for s, n in only_bo_c[:20]: print(f"    + [{s}] {n}")
    print(f"  Only in Camoufox ({len(only_cf_c)}):")
    for s, n in only_cf_c[:20]: print(f"    - [{s}] {n}")
    print(f"\n--- Script errors ---")
    print(f"  BO  : {len((e_bo or {}).get('errors', []))} errors")
    print(f"  CF  : {len((e_cf or {}).get('errors', []))} errors")
    for e in (e_bo or {}).get("errors", [])[:5]:
        print(f"    BO @t={e['ms_from_nav_start']}ms: {e['message'][:120]}")
    print(f"\n--- Body / iter ---")
    if i_bo and i_cf:
        bo_last = i_bo["iters"][-1]
        cf_last = i_cf["iters"][-1]
        print(f"  BO  iters={i_bo['n_iters']} body={bo_last['body_size']} children={bo_last['mount_children']}")
        print(f"  CF  iters={i_cf['n_iters']} body={cf_last['body_size']} children={cf_last['mount_children']}")
    klass, msg = classify_gap(only_cf, only_cf_c)
    print(f"\n--- VERDICT ---")
    print(f"  Cluster: {klass}")
    print(f"  Reason : {msg}")
    print()

if __name__ == "__main__":
    main()
```

## 5. Acceptance — reddit reference output

The tools are complete when running:

```bash
# RUN THIS
target/release/examples/sweep_metrics chrome_148_macos /tmp/corpus.json \
    /tmp/unused.json --capture reddit
/tmp/bo-venv/bin/python benchmarks/capture_camoufox.py reddit
python3 benchmarks/capture_diff.py \
    /tmp/capture/bo/chrome_148_macos/reddit \
    /tmp/capture/camoufox/default/reddit
```

produces (approximate, structured):

```
========================================================================
CAPTURE DIFF — /tmp/capture/bo/chrome_148_macos/reddit  vs
               /tmp/capture/camoufox/default/reddit
========================================================================

--- Fetches ---
  BO  : 3 fetches
  CF  : 38 fetches
  Only in BO (0):
  Only in Camoufox (35):
    - https://www.reddit.com/svc/shreddit/account-switcher
    - https://www.reddit.com/r/popular
    - https://www.reddit.com/r/all
    - https://www.redditstatic.com/desktop2x/...js
    - https://www.reddit.com/api/v1/...
    [...30 more...]
  Status differs (0):

--- Cookies ---
  Only in BO (0):
  Only in Camoufox (4):
    - [set-cookie] edgebucket
    - [set-cookie] csv
    - [set-cookie] session_tracker
    - [set-cookie] token_v2

--- Script errors ---
  BO  : 1 errors
  CF  : 0 errors
    BO @t=312ms: TypeError: Cannot read properties of null (reading 'value')

--- Body / iter ---
  BO  iters=1 body=8326 children=1
  CF  iters=1 body=1142312 children=87

--- VERDICT ---
  Cluster: missing-fetch
  Reason : Camoufox issued 35 fetches we didn't
```

### Acceptance checklist

For the tools to be "done", every one of these must be true on the
reddit capture:

- [ ] `/tmp/capture/bo/chrome_148_macos/reddit/` exists and contains
      all 7 files listed in §1.
- [ ] `/tmp/capture/camoufox/default/reddit/` exists and contains the
      same 7 file names (pending_nav_timeline.json may be empty).
- [ ] `body.html` for BO is ≥ 1 KB and ≤ 20 KB (it's the verification
      stub, see `02_GAP_ANALYSIS.md` §1).
- [ ] `body.html` for Camoufox is ≥ 500 KB.
- [ ] BO `fetches.json` has `n_fetches >= 1` and entry [0].method
      == "GET", entry [0].url starts with "https://www.reddit.com/".
- [ ] BO `script_errors.json` has at least one error
      whose `message` mentions either `requestSubmit` or `namedItem`
      or `forms` (per `02_GAP_ANALYSIS.md` §1 hypothesis 3/4).
- [ ] BO `pending_nav_timeline.json` has zero or one flip; if zero,
      the diagnosis confirms hypothesis 1 (the form-submit chain
      didn't fire).
- [ ] `capture_diff.py` exits 0 and produces a `--- VERDICT ---`
      section whose `Cluster` is `missing-fetch` and whose `Reason`
      mentions ≥ 30 missing fetches.
- [ ] The capture mode must be a no-op for the production sweep
      path: running `sweep_metrics` without `--capture` must produce
      byte-identical JSON to the pre-instrumentation version on a
      regression-corpus subset.

## 6. Estimated work

| Item | Approx LOC | Files touched |
|---|--:|---|
| `crates/browser/src/capture.rs` (new) | 250 | 1 new |
| `sweep_metrics --capture` | 80 | `examples/sweep_metrics.rs` |
| `Page::dom_snapshot()` | 60 | `crates/browser/src/page.rs` |
| `fetch_ext.rs` integration | 15 | `crates/js_runtime/src/extensions/fetch_ext.rs` |
| `cookie_ext.rs` + net Set-Cookie integration | 25 | `crates/js_runtime/src/extensions/fetch_ext.rs:397-410`, `crates/net/src/lib.rs:557 +` |
| `console_ext.rs` integration | 9 | `crates/js_runtime/src/extensions/console_ext.rs` |
| `runtime.rs` exception integration | 30 | `crates/js_runtime/src/runtime.rs` |
| `stealth_bootstrap.js` pending-nav shim + new op | 40 | `crates/js_runtime/src/js/stealth_bootstrap.js`, `crates/js_runtime/src/extensions/dom_ext.rs` |
| `page.rs` iter recording | 15 | `crates/browser/src/page.rs:1545-2700` |
| `benchmarks/capture_camoufox.py` | 200 | 1 new |
| `benchmarks/capture_diff.py` | 200 | 1 new |
| Tests + regression gate | 100 | `crates/browser/tests/capture_smoke.rs` (new) |

Total: **~1000 LOC, 1-2 dev-days**. Blocking for Phases 1-3.

## Files referenced

- `crates/browser/examples/sweep_metrics.rs:1-286` — current sweep
  harness; extend with `--capture` mode
- `crates/browser/src/page.rs:200-3389` — `Page` struct, navigate
  family, integration points for `record_iter`
- `crates/js_runtime/src/extensions/fetch_ext.rs:200-410` — `op_fetch`,
  `op_cookie_set`, `op_cookie_get` — integration points for fetch +
  cookie recording
- `crates/js_runtime/src/extensions/console_ext.rs:1-31` —
  `op_console_log/warn/error` integration points
- `crates/js_runtime/src/state.rs:7-12` — `DomState.console_output`
- `crates/js_runtime/src/js/stealth_bootstrap.js:120-130` — example
  of installing a fake getter/setter pattern (use as template for the
  `__pendingNavigation` shim)
- `crates/js_runtime/src/js/dom_bootstrap.js:1098-1110` — `submit()` /
  `requestSubmit()` (the path reddit's challenge exercises)
- `crates/net/src/lib.rs:60-178` — `Response` struct, `HttpClient` —
  integration point for Set-Cookie capture
- `crates/event_loop/src/lib.rs:11-378` — existing
  `BROWSER_OXIDE_EVENT_LOOP_PROFILE` instrumentation; pattern to
  follow for capture
- `docs/releases/v0.1.0-parity/02_GAP_ANALYSIS.md` — the 10 sites this
  tooling exists to debug; reddit acceptance shape comes from §1
- `docs/releases/v0.1.0-parity/03_BENCHMARK_METHODOLOGY.md` — sweep
  harness, JSON schema conventions, env-var inventory
- `docs/releases/v0.1.0-parity/14_TESTING_VALIDATION.md` — regression
  gates that wrap this tooling
