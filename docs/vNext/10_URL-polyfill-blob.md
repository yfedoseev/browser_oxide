# 10 — URL polyfill: `new URL("blob:…")` returns empty `.protocol` / "null" `.origin`

**Status:** ⬜ open. Caught as side-finding during R-DUO-WORKER (commit `967b4dc`).
**Sites in scope:** indirect — affects any DataDome iframe rendering + duolingo worker recaptcha path beyond what FIX-W already addresses.
**Effort:** 1-2 days.
**Scope:** public engine.

## TL;DR

BO's URL polyfill at `crates/js_runtime/src/js/shared_apis_bootstrap.js`
(class `URL`) doesn't correctly parse `blob:` scheme URLs. For a URL
like `blob:null/7aeb61c9-…`:
- Real Chrome: `.protocol = "blob:"`, `.origin = "null"`, `.href` = the
  full URL.
- BO: `.protocol = ""`, `.origin = "null"`, `.href` = the full URL.

The `.href` works (mostly). `.origin` accidentally matches Chrome's
"null" output. But `.protocol = ""` is wrong; real Chrome returns
`"blob:"`. Caught when adding the `worker_self_location` test in
[FIX-W](../releases/v0.1.0-parity/audit/16_DECISION_LOG.md) — the test
had to relax its `.protocol` assertion.

## Why this matters

- `blob:` URLs are how cross-origin Workers receive their script (real
  browsers create a Blob → `URL.createObjectURL(blob)` → spawn worker
  from the blob:URL). The Worker's `self.location.protocol` is then
  expected to be `"blob:"`.
- Recaptcha enterprise's webworker reads `self.location` to verify
  the origin matches; FIX-W populates `self.location` but the URL
  polyfill bug means `self.location.protocol === ""` while real
  Chrome returns `"blob:"`. Sensors that cross-check on protocol see
  the inconsistency.
- DataDome's iframe-served challenge documents are also commonly
  spawned via blob: URLs (per the iframe materialization in
  FIX-DD); the URL polyfill needs to handle them.

This is a SECONDARY gap to FIX-DD + FIX-W. Neither will fully close
until the URL polyfill is fixed.

## Why this is subtle

The URL polyfill at `shared_apis_bootstrap.js:233-303` is a
hand-rolled WHATWG URL parser. blob: URLs have a SPECIAL scheme handling
in the spec — the path is an opaque host followed by `/` + the blob
UUID. Plain HTTP URL parsers don't handle the opaque-host shape
correctly.

Reference: https://url.spec.whatwg.org/#blob-url

## Current state

The polyfill:
- `shared_apis_bootstrap.js:233-303` — URL class
- Handles http, https, ws, wss correctly
- Returns empty `.protocol` for blob: (and probably data:, file: too —
  needs verification)

Workaround in FIX-W test:
- `crates/js_runtime/tests/worker.rs::worker_self_location_populated_from_construction_url`
  has a TODO comment + a relaxed assertion. Search for "URL polyfill
  currently returns empty .protocol".

## Next steps

### Step 1 — Map the scope (~few hours)

Test which schemes are broken:
```javascript
['blob:null/uuid', 'data:text/html,x', 'file:///x', 'ws://x/', 'wss://x/'].forEach(u => {
    try { const p = new URL(u); console.log(u, '→', p.protocol, p.origin); }
    catch (e) { console.log(u, '→', e.message); }
});
```

Real Chrome reference:
- `blob:null/uuid` → `protocol="blob:"`, `origin="null"`
- `data:text/html,x` → `protocol="data:"`, `origin="null"`
- `file:///x` → `protocol="file:"`, `origin="null"`
- `ws://x/` → `protocol="ws:"`, `origin="ws://x"`
- `wss://x/` → `protocol="wss:"`, `origin="wss://x"`

### Step 2 — Fix the URL polyfill (~1 day)

Add scheme-prefix detection BEFORE the http-style parsing:
```javascript
function URL(input, base) {
    const s = String(input);
    // Opaque-scheme handling (WHATWG URL spec)
    for (const opaque of ['blob:', 'data:', 'javascript:', 'about:']) {
        if (s.startsWith(opaque)) {
            this.protocol = opaque;
            this.origin = 'null';
            this.href = s;
            this.pathname = s.slice(opaque.length);
            this.search = '';
            this.hash = '';
            this.host = '';
            this.hostname = '';
            this.port = '';
            return;
        }
    }
    // ... existing http/https/ws parsing
}
```

`file:` is half-opaque — `file:///path` is treated as a "special scheme"
with empty host. Handle separately if needed.

### Step 3 — Tests

In `crates/js_runtime/tests/` or via chrome_compat, add:
- `url_blob_scheme_protocol_is_blob_colon`
- `url_data_scheme_protocol_is_data_colon`
- `url_blob_scheme_origin_is_null`
- `url_blob_scheme_href_preserved`

Relax / fix the existing relaxed assertion in the FIX-W test
(`worker_self_location_populated_from_construction_url`):
```rust
assert_eq!(v["protocol"], "blob:", "protocol from URL parse");
```

### Step 4 — Validate

Run the full chrome_compat suite. No regressions expected — the
opaque-scheme handling is additive.

## Dependencies

- None. Self-contained polyfill fix.

## Sources / references

- `crates/js_runtime/src/js/shared_apis_bootstrap.js:233-303` — URL polyfill
- `crates/js_runtime/tests/worker.rs::worker_self_location_populated_from_construction_url` — the relaxed-assertion test
- WHATWG URL spec: https://url.spec.whatwg.org/#concept-url-parser
- Commit `967b4dc` (FIX-W) — discovery context
