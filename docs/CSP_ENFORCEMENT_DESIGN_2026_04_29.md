# CSP Enforcement Design — 2026‑04‑29

Author: design audit (no code changes yet)
Status: design + estimate, ready for approval
Trigger: real Playwright Chromium loading `https://www.walmart.com/` blocks the
`/akam/13/3e35295b…` Akamai BMP bootstrap script via CSP, so Chrome never POSTs
to `/akam/13/pixel_*`. browser_oxide today fetches the bootstrap and POSTs the
sensor data, which is itself a strong "this isn't real Chrome" tell.

---

## 1. Current state — we have **zero** CSP machinery

Confirmed by `grep -rni "content-security-policy\|csp" crates/ --include='*.rs'`:
the only matches are unrelated (`tls.rs:75` "OCSP", `tls.rs:116`
`enable_ocsp_stapling`). There is:

- no CSP header parser
- no per‑document policy store
- no enforcement at any of the 41 `client.get/post/HttpClient::new` call sites
- no `<meta http-equiv="Content-Security-Policy">` parsing in
  `crates/html_parser/`
- no `nonce` capture on `<script>` (`crates/browser/src/script_runner.rs:60`
  reads only `src`/`type`)
- no console reporter for `net::ERR_BLOCKED_BY_CSP`

**Every** outbound HTTP we issue from a navigated page bypasses the policy that
real Chrome would have enforced.

## 2. Why this matters — concrete bot tells we currently emit

### 2.1 Walmart Akamai bootstrap (the trigger)

Walmart serves CSP via two channels:

| Channel | Directives present |
|---|---|
| HTTP response header | `frame-ancestors 'self' *.wal.co … ; report-uri https://csp.walmart.com/c/r/gl` |
| `<meta http-equiv="Content-Security-Policy">` in the HTML head | `child-src; connect-src; default-src; font-src; frame-src; img-src; media-src; object-src; script-src; style-src; worker-src` |

The meta‑tag `script-src` is:

```
script-src 'self' 'strict-dynamic' 'wasm-unsafe-eval'
           *.1worldsync.com … *.walmart.com:* …
           www.recaptcha.net 'nonce-MRjHHgrLk9lNoNBv'
```

Three of those tokens together control everything:

1. `'strict-dynamic'` — per W3C CSP3 and confirmed by MDN, Web.dev, and
   content-security-policy.com: when present, the host allowlist (`*.foo.com …`)
   is **ignored**. Trust is conferred only by:
   - a matching nonce on the original `<script>` tag, or
   - propagation: a script already trusted (nonce) creates a new
     `<script>` via `document.createElement` + `appendChild`.
2. `'nonce-MRjHHgrLk9lNoNBv'` — only scripts with that exact nonce attribute
   are trusted as roots.
3. The `/akam/13/3e35295b…` URL does **not** appear in the static HTML returned
   by walmart.com. It is injected at runtime by Akamai's edge through a
   parser‑inserted node that did not carry a nonce, so under `strict-dynamic`
   it is "parser-inserted" and not trusted. Real Chrome blocks the
   `<script src>` fetch with `net::ERR_BLOCKED_BY_CSP` and never executes the
   sensor bootstrap, so it never POSTs to `/akam/13/pixel_*`.

We do execute the bootstrap (no CSP), so we POST. Akamai's server‑side check
notices the POST arrived from a UA that should have been silenced and flags
the session.

### 2.2 Other tells the absence of CSP creates

- **`connect-src`** — Walmart's `connect-src` does not include the same‑origin
  `walmart.com/akam/...` path implicitly (it covers `*.walmart.com:*`, which
  on the spec's host‑match grammar **does** match `www.walmart.com`, so a
  `fetch()` to `/akam/13/pixel_*` would technically pass `connect-src`). But on
  many other Akamai‑protected sites the sensor URL crosses `connect-src`
  (e.g. tenants where the sensor is on `*.akamaihd.net`, which the meta blocks
  for non‑connect contexts). We currently fire those, too.
- **`img-src`** — pixel beacons we don't suppress when the directive would
  forbid them (e.g. `img.youtube.com` on a site that doesn't allowlist YouTube
  imagery).
- **`frame-src` / `child-src`** — third‑party iframes we currently load via
  `iframe::ChildIframe::from_url` (`crates/browser/src/iframe.rs:72`) without
  consulting the parent's CSP.

In aggregate: any anti‑bot vendor that does the cheap test "did the client
fire a request that CSP forbade?" gets a free signal from us.

## 3. Real Chrome's enforcement model — the minimum subset we need

### 3.1 Where Chrome enforces

CSP enforcement in Chromium is split:

| Layer | What it gates | Where in our codebase the analogue is |
|---|---|---|
| Renderer (Blink) — pre‑request | inline scripts, eval, navigation, `<script src>` resolution, `<img src>`, `<link>` href, `<iframe src>`, `<form action>`, `WebSocket()`, `EventSource()`, dynamic import | `crates/browser/src/{page,script_runner,stylesheet_collector,iframe}.rs`, `crates/js_runtime/src/js/window_bootstrap.js` (XHR/fetch/sendBeacon stubs) |
| Network service — request loader | `connect-src`, `script-src` for the actual GET, redirect re‑checks | `crates/net/src/lib.rs` (`HttpClient::get/post/get_follow/...`), `crates/js_runtime/src/extensions/fetch_ext.rs` (`op_fetch`, `op_net_fetch_sync`, `op_net_xhr_sync`) |

For our purposes (we don't have a separate sandboxed renderer; everything runs
in one V8), the **network‑side** check dominates: stop the GET before it leaves
the box. Renderer‑side checks (inline script blocking, `eval` blocking) are
secondary and largely unnecessary because we don't currently signal back into
JS — see §4.6.

### 3.2 Directives we must implement (priority ordered)

| Directive | Gates | Priority | Notes |
|---|---|---|---|
| `script-src` | `<script src>`, dynamic imports, worker scripts, sync XHR scripts | **P0** | Akamai bootstrap blocker; must understand `'strict-dynamic'`, nonces, `'self'` |
| `connect-src` | `fetch()`, XHR, `EventSource`, `WebSocket`, `navigator.sendBeacon` | **P0** | Pixel POSTs and beacon fan‑out |
| `default-src` | fallback for any `*-src` not specified | **P0** | parser must resolve fallback chain |
| `img-src` | `<img>`, `<link rel="preload" as="image">`, CSS `url(...)` images | **P1** | tracker pixels |
| `frame-src` (with `child-src` fallback) | `<iframe src>` | **P1** | iframe loaders |
| `worker-src` | `new Worker(url)`, `SharedWorker`, service workers | **P2** | currently `worker_ext.rs` |
| `style-src` | `<link rel=stylesheet>`, `<style>`, inline style attrs | **P2** | low signal — we already differ in CSS |
| `font-src` | `@font-face url(...)` | **P3** | we don't fetch fonts today |
| `media-src`, `object-src`, `form-action` | `<video>`, `<embed>`, `<form>` | **P3** | rarely fired by anti‑bot vendors |
| `frame-ancestors` | who can iframe US — N/A as a client | skip | server/header concern |
| `report-uri` / `report-to` | violation reporting endpoint | **P2** if we want maximum realism (real Chrome posts to it) |

### 3.3 Source-expression grammar minimum

Per CSP3 spec we need a parser that handles:

- keywords: `'none'`, `'self'`, `'unsafe-inline'`, `'unsafe-eval'`,
  `'wasm-unsafe-eval'`, `'strict-dynamic'`
- scheme sources: `https:`, `data:`, `blob:`, `mediastream:`, `filesystem:`
- host sources with wildcards: `*.example.com`, `https://*.example.com:*`,
  `https://example.com/path/*`
- nonce sources: `'nonce-<base64>'`
- hash sources: `'sha256-<b64>'`, `'sha384-<b64>'`, `'sha512-<b64>'`

Inline‑script hashes (`'sha256-…'` matching the body of an inline `<script>`)
are **out of scope for v1** — we don't currently re‑execute inline scripts
through a CSP gate. Flag for v2.

## 4. Proposed implementation — file by file

### 4.1 New module `crates/net/src/csp.rs`

The header parser + matcher. Belongs in `net` (not `browser`) so the
`HttpClient` and `op_fetch` can both reach it without circular deps.

```rust
// crates/net/src/csp.rs (new file, ~600 LOC including tests)

#[derive(Debug, Clone)]
pub struct Policy {
    pub directives: HashMap<Directive, Vec<SourceExpr>>,
    pub report_only: bool,
    pub report_uri: Vec<String>,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum Directive {
    DefaultSrc, ScriptSrc, ConnectSrc, ImgSrc, FrameSrc,
    ChildSrc, StyleSrc, FontSrc, MediaSrc, ObjectSrc,
    WorkerSrc, FormAction, FrameAncestors, BaseUri,
    // ...
}

#[derive(Debug, Clone)]
pub enum SourceExpr {
    None_,
    Self_,
    UnsafeInline,
    UnsafeEval,
    WasmUnsafeEval,
    StrictDynamic,
    Scheme(String),                 // "https"
    Host { scheme: Option<String>, host: String, port: Option<String>, path: Option<String> },
    Nonce(String),                  // base64 payload
    Hash { alg: HashAlg, b64: String },
}

#[derive(Debug, Clone, Copy)]
pub enum RequestKind {
    Script, Connect, Image, Frame, Style, Font, Media, Object, Worker, Manifest,
}

pub struct CheckCtx<'a> {
    pub kind: RequestKind,
    pub url: &'a Url,
    pub origin: &'a Url,            // page origin for 'self'
    pub nonce: Option<&'a str>,     // attached to <script nonce=...>
    pub parser_inserted: bool,      // true for static HTML <script src>
}

impl Policy {
    pub fn parse_header(value: &str, report_only: bool) -> Self { ... }
    pub fn parse_meta_content(value: &str) -> Self { ... }
    pub fn merge(&mut self, other: Policy) { ... } // stack multiple policies
    pub fn allows(&self, ctx: &CheckCtx<'_>) -> Decision { ... }
}

pub enum Decision { Allow, BlockEnforced, BlockReport }
```

Tests live alongside in the same file, modeled on the W3C web‑platform‑tests
CSP corpus (~30 fixtures cover the long tail).

### 4.2 Extract policy at the navigation layer

`crates/browser/src/page.rs:632` — the initial `client.get_follow(url, 10)`
already gives us `resp.headers` (a `HashMap<String, String>` per
`crates/net/src/lib.rs:51`). After the fetch, before
`navigate_loop_internal`:

```rust
// new
let mut policy = net::csp::Policy::default();
if let Some(h) = resp.headers.get("content-security-policy") {
    policy.merge(net::csp::Policy::parse_header(h, false));
}
if let Some(h) = resp.headers.get("content-security-policy-report-only") {
    policy.merge(net::csp::Policy::parse_header(h, true));
}
```

The DOM parser must also yield `<meta http-equiv="Content-Security-Policy">`
content (Walmart's primary delivery channel). Hook in
`crates/html_parser/src/` or, more cheaply, after parse in
`crates/browser/src/page.rs:1436` (`parse_html(html)` is followed
immediately by `find_scripts`/`find_stylesheets` — add a `find_csp_meta(&dom)`
sibling collector). We must apply meta‑CSP **before** `find_scripts` returns
so script fetches honor it.

### 4.3 Plumb the policy into runtime state

`crates/js_runtime/src/state.rs:5` — extend `DomState`:

```rust
pub struct DomState {
    // ... existing
    pub csp_policy: Option<net::csp::Policy>,
    pub csp_origin: Option<url::Url>,    // page origin for 'self' resolution
}
```

This makes the policy reachable from every `#[op2]` op via `#[state]`. For the
async ops (`op_fetch`) that can't hold OpState across awaits, add a parallel
`OnceLock<Mutex<Policy>>` next to `FETCH_CLIENT` in
`crates/js_runtime/src/extensions/fetch_ext.rs:43`.

### 4.4 Per-call-site enforcement

41 net call sites in 8 files. Group them by `RequestKind`:

| Site | File:line | RequestKind | Change |
|---|---|---|---|
| Top-level navigation | `page.rs:633` | (skip — navigation isn't subject to *parent's* CSP for top-level) | none |
| Script pre-fetch | `page.rs:1489` | Script | wrap fetch with `policy.allows(ScriptCheckCtx { nonce: script.nonce, parser_inserted: true })`; on Block, drop without HTTP |
| Stylesheet fetch | `page.rs:1453` | Style | same pattern |
| Iframe fetch | `iframe.rs:79`, `iframe.rs:177` | Frame | check parent's policy before `client.get` |
| Iframe stylesheet fetch | `iframe.rs:129` | Style | check the iframe's own policy (after fetch) |
| `op_fetch` | `fetch_ext.rs:161-167` | Connect | `policy.allows(...)` BEFORE `fetch_get`/`fetch_post_bytes` |
| `op_net_fetch_sync` | `fetch_ext.rs:301` | Script | this op is invoked by `document.write(<script src>)` and `appendChild(script)` — both *script* requests under CSP |
| `op_net_xhr_sync` | `fetch_ext.rs:413,423` | Connect | XHR is connect‑src |
| SSE | `sse_ext.rs:148` | Connect | `EventSource` is connect‑src |
| WebSocket | `websocket_ext.rs` | Connect | gate the connection request |
| Worker fetch | `worker_ext.rs:154` | Worker (or Script if `worker-src` absent) | check |
| `sendBeacon` | `window_bootstrap.js:709` (compiles down to `op_fetch`) | Connect | already covered by `op_fetch` gate |

The script‑pre‑fetch site (`page.rs:1489`) is the **single highest‑ROI hook**:
it's where the Walmart/Akamai bootstrap is currently fetched. With
`'strict-dynamic'` semantics implemented:

1. `script_runner::find_scripts` must capture the `nonce` attribute (one‑line
   addition at `script_runner.rs:60`).
2. Static `<script src>` in the parsed HTML is `parser_inserted=true`.
3. `Policy::allows` returns `BlockEnforced` for any parser‑inserted script
   without the page nonce when `'strict-dynamic'` is in effect — Akamai's
   bootstrap fits the pattern only when injected by the edge (which is
   parser‑inserted from the HTML the client received). If injected dynamically
   from a trusted root script via DOM API, the trust propagates and we let it
   through (matching Chrome).

### 4.5 Origin computation for `'self'`

`'self'` matches the page's origin (scheme+host+port). Today the origin is
tracked piecemeal: `fetch_ext.rs:128` reads an `x-boxide-origin` pseudo header
from JS, but the navigation layer doesn't authoritatively own it. The CSP
work needs a single source of truth. Add `csp_origin: url::Url` to `DomState`
(§4.3), set it once in `navigate_loop_internal` from `resp_url`, and let
`Policy::allows` read it from there.

### 4.6 Reporting / "blocked by CSP" surface

For maximum bot‑evasion realism (some vendors check whether
`window.SecurityPolicyViolationEvent` fired), we need:

- **Console error**: route a `ConsoleMessage` into
  `DomState.console_output` with the exact Chrome wording:
  `"Refused to load the script '<url>' because it violates the following
  Content Security Policy directive: \"<directive>\". …"`
  Add a helper in `crates/js_runtime/src/extensions/console_ext.rs`.
- **JS event**: dispatch `securitypolicyviolation` on `document` (or `Window`).
  Add a small JS bridge: `op_csp_report(blockedURI, violatedDirective, ...)`
  that pushes the event. Implementation: ~30 LOC in
  `window_bootstrap.js` + a new op in `fetch_ext.rs` or a sibling
  `csp_ext.rs`.
- **Report-uri POST** (P2): when `report-uri` is set, fire an `op_fetch`‑style
  POST to it with the standard JSON body. Many sites use this telemetry as a
  positive bot signal too — *if* we send malformed/missing reports, we look
  bot‑like; if we send no reports when Chrome would, same problem.
  Implementing this faithfully is non‑trivial (need correct content type
  `application/csp-report` and JSON shape).

## 5. Edge cases and open questions

| # | Edge case | Plan |
|---|---|---|
| E1 | Multiple `Content-Security-Policy` headers on one response | Spec: each is an independent policy; **all** must allow. Implement as `Vec<Policy>` and `all(p.allows(...))`. |
| E2 | `<meta http-equiv="Content-Security-Policy">` after the first byte of body — only directives encountered before the request matter | Parse meta tags during HTML parse; for v1 it's acceptable to apply meta‑CSP to *all* sub‑resources of that document since we batch‑fetch (not streaming). Document this gap. |
| E3 | `report-only` mode | Compute `Decision::BlockReport`; do NOT block; do report. |
| E4 | Nonces and hashes | Nonces: P0 (Akamai/Walmart pattern). Inline‑script hashes: P3, flag and skip in v1. |
| E5 | Inline script vs fetch | We don't currently sandbox inline scripts behind CSP. Flag — anti‑bot vendors that fingerprint via inline `eval` blocks may detect difference. |
| E6 | Redirects re‑check `script-src` against the *final* URL too | Today `client.get_follow` returns the final URL; we must check policy at each hop. Cleanest: a wrapper in `net::HttpClient` that takes a `Policy` and re‑checks after each redirect. |
| E7 | Service workers cache CSP from the install response | Skip — we don't run service workers anyway. |
| E8 | `frame-ancestors` is a *server*‑side directive | We never enforce it (it gates whether *we* can be framed). N/A. |
| E9 | The CSP allowlist contains `*.walmart.com:*` — port wildcard semantics | Standard host‑source grammar; a tested parser handles this. |
| E10 | `'strict-dynamic'` interaction with Trusted Types | Trusted Types policy is its own header (`Require-Trusted-Types-For`). Ignore for v1. |
| E11 | `default-src` does NOT fall back for `frame-ancestors`, `form-action`, `base-uri`, `report-uri`, `report-to`, `sandbox`, `plugin-types`, `navigate-to` | Bake into `Policy::allows` directive‑lookup helper. |

## 6. Effort estimate by phase

Estimates assume a Rust engineer familiar with the repo. "Day" = 6 productive
hours.

| Phase | Scope | Effort | Risk |
|---|---|---|---|
| **P0a — Header parser** | `crates/net/src/csp.rs` with parser + matcher + 30‑40 unit tests covering source‑expr grammar, nonce/hash, `'strict-dynamic'`, default‑src fallback | **3 days** | Low — well‑specified |
| **P0b — Meta‑tag extraction** | `find_csp_meta(&dom)` collector; merge into `Policy` | **0.5 day** | Low |
| **P0c — Policy plumbing** | `DomState.csp_policy`, `csp_origin`; `OnceLock` mirror for async ops; pass‑through in `navigate_with_init` | **1 day** | Low |
| **P0d — Script‑src enforcement** | `script_runner.rs` capture nonce; gate `page.rs:1489`, `fetch_ext.rs op_net_fetch_sync`, worker fetch | **1.5 days** | Medium — `'strict-dynamic'` parser‑inserted detection requires care |
| **P0e — Connect‑src enforcement** | gate `op_fetch`, `op_net_xhr_sync`, SSE, WebSocket | **1 day** | Low |
| **P1 — Image / frame / style** | gate stylesheet/iframe/img sub‑resources | **1 day** | Low |
| **P1 — Console error reporting** | wired through `console_ext.rs` + JS `securitypolicyviolation` event | **1 day** | Low |
| **P2 — Report‑uri POST** | match Chrome's body shape and content‑type | **1 day** | Medium — easy to look fake |
| **P2 — Redirect re‑check** | wrap `client.*_follow` to re‑gate at each hop | **0.5 day** | Medium |
| **Integration tests** | Walmart‑like fixture: meta‑CSP with `'strict-dynamic'` + a parser‑inserted `<script src>` without nonce — assert no GET fires; same fixture but injected via trusted root → assert GET fires | **1.5 days** | Low |
| **Browser‑comparison test** | run `walmart.com` end‑to‑end and assert `/akam/13/3e35295b…` is NOT fetched | **0.5 day** | Low |
| **Total P0** (must‑ship to fix Walmart) | parser + meta + plumbing + script‑src + connect‑src + tests | **~8 days** | |
| **Total P0+P1** (full near‑Chrome parity for sub‑resources + reporting) | + img/frame/style + console events | **~10 days** | |
| **Total P0+P1+P2** (faithful reporting + redirect re‑check) | + report‑uri + redirect | **~12 days** | |

## 7. Risks and gotchas

1. **`'strict-dynamic'` parser‑inserted detection** — getting this wrong in
   either direction breaks sites or fails to fix them. Test against a
   known‑good list (Walmart, Akamai BMP, Cloudflare Turnstile pages).
2. **Meta‑CSP timing** — strictly the policy applies only to resources
   discovered *after* the meta tag is parsed. For v1, since we batch‑fetch
   sub‑resources after full HTML parse, we can apply it uniformly. Document.
3. **No CSP today means our "blocked by CSP" surface is currently invisible
   to detectors.** The risk is that we *under‑block* (look bot‑like) AND that
   we *over‑block* and break legitimate sites. Mitigate by gating behind a
   stealth flag (`profile.enforce_csp = true|false`) for the first few releases
   so we can A/B against the existing 17 L3‑rendered baseline.
4. **Redirect re‑checking** — Chrome re‑applies `script-src`/`connect-src`
   to the *final* URL. If we only check the initial URL we'll over‑allow
   sneaky 30x indirects through a CSP‑allowlisted host to a forbidden one.
5. **Nonce extraction** — `script_runner.rs` decodes HTML entities for `src`
   but we'd need the same for `nonce`. Single‑attribute change; flag in PR.
6. **Performance** — `Policy::allows` is called once per fetch; cost is
   negligible (sub‑microsecond regex‑free string matching). No risk.

---

## Sources

- [strict-dynamic in CSP — content-security-policy.com](https://content-security-policy.com/strict-dynamic/)
- [Mitigate XSS with a strict CSP — web.dev](https://web.dev/articles/strict-csp)
- [CSP: script-src — MDN](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Content-Security-Policy/script-src)
- [Content Security Policy Level 3 — W3C Editor's Draft](https://w3c.github.io/webappsec-csp/)
