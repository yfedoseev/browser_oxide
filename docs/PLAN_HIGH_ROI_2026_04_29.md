# Plan — High-ROI parity work (post 98/126 baseline)

Three levers ranked by ROI from the 2026-04-29 sweep. This doc breaks each into day-grain tasks so progress is auditable.

## Where we are

- 98/126 PASS (77.8%), 7m 40s wall-clock
- 28 failing sites: 11 Akamai-CHL, 4 Kasada-CHL, 5 captcha-shell, 3 DataDome-CHL, 1 PerimeterX-CHL, 1 Cloudflare-CHL, 2 BLOCKED, 1 THIN-BODY, 1 generic
- Akamai BMP v13 JS sensor: byte-for-byte parity on 7 deterministic fields (regression-locked at `crates/browser/tests/akamai_v13_probe_parity.rs`)
- PerimeterX surface: 13 parity gates locked at `crates/browser/tests/perimeterx_surface_parity.rs`
- 5 sub-1hr Chrome surface fixes shipped today (BatteryManager class, native-code masking, GREASE literal, Touch toStringTag, mDNS ICE)

## Scope discipline

This plan is parity engineering — make our engine match real Chrome on observable Web Platform APIs. We do **not**:
- Forge encrypted sensor payloads or session tokens
- Reverse-engineer HMAC keys or challenge crypto
- Build vendor-specific challenge-bypass solvers in unauthorized contexts

The work is identical to what a Web Platform Tests run or a competing browser engine would do.

---

## Phase 1 — CSP enforcement (≈8 days)

Design: `docs/CSP_ENFORCEMENT_DESIGN_2026_04_29.md`

**Why first**: real Chrome enforces CSP. We don't. So we issue requests Chrome wouldn't (eg. walmart `/akam/13/3e35295b` parser-injected script when CSP has `'strict-dynamic'` + nonce). Sending a request real Chrome wouldn't is itself a cross-vendor bot tell. Fixing this is a single change that affects every site, not just Akamai-protected ones.

### Day 1 — `net::csp` core types + header parser
- New file `crates/net/src/csp.rs`
- `Policy { directives: HashMap<Directive, Vec<Source>>, report_only: bool }`
- `Directive` enum: `ScriptSrc | ConnectSrc | ImgSrc | FrameSrc | StyleSrc | FontSrc | MediaSrc | DefaultSrc | ScriptSrcElem | ScriptSrcAttr | ...`
- `Source` enum: `Self_ | Host(String) | Scheme(String) | Nonce(String) | Hash(Algo, Vec<u8>) | All | None_ | StrictDynamic | UnsafeInline | UnsafeEval | UnsafeHashes | ReportSample`
- `parse_header(s: &str) -> Policy`
- `parse_meta_content(s: &str) -> Policy`
- `merge(other: Policy) -> Policy` (intersect — most-restrictive wins per CSP3)
- Unit tests on Walmart's actual CSP string

### Day 2 — `Policy::allows()` + match logic
- `CheckCtx { directive, url: Url, page_origin: Url, nonce: Option<String>, parser_inserted: bool }`
- `allows(ctx) -> AllowDecision { allowed: bool, matched_directive, violated_source }`
- default-src fallback chain
- `'strict-dynamic'` semantics: when present in script-src, host allowlist is ignored; only nonce/hash-trusted scripts and their dynamically-inserted children pass
- Origin-match: scheme + host + optional port; wildcards
- `'self'` resolves to page origin
- Tests on real-world fixtures including the Walmart script-src

### Day 3 — Plumb policy through `DomState` + meta extraction
- Add `csp_policy: Option<Policy>` and `csp_origin: Option<Url>` to `crates/js_runtime/src/state.rs::DomState`
- Mirror to `OnceLock<Arc<Policy>>` next to `FETCH_CLIENT` in `crates/js_runtime/src/extensions/fetch_ext.rs` (so async ops can read without tokio runtime context)
- After `html_parser::parse_html` in `crates/browser/src/page.rs:1436`, walk `<head>` for `<meta http-equiv="Content-Security-Policy">` and merge with response-header CSP
- Helper: `extract_meta_csps(dom: &Dom) -> Vec<Policy>`

### Day 4 — Script-src enforcement at script_runner
- Modify `crates/browser/src/script_runner.rs:60` `ScriptInfo` to carry `nonce: Option<String>` (read `nonce` attr during HTML walk)
- At `crates/browser/src/page.rs:1489` (the `<script src>` fetch site): wrap `client.get_follow_with_headers` in `policy.allows(CheckCtx { directive: ScriptSrcElem, url, nonce, parser_inserted: true })`
- On block: emit `[csp] BLOCKED <url>` console message, dispatch `securitypolicyviolation` event (Day 7), return early
- Mirror enforcement in `crates/js_runtime/src/extensions/fetch_ext.rs::op_net_fetch_sync` (line ~301) for `<script>` injected via `document.write` / `appendChild`
- **This single change blocks Walmart's `/akam/13/` bootstrap**

### Day 5 — Connect-src enforcement at op_fetch
- `op_fetch` and `op_net_fetch_sync` (lines ~161, 167, 301, 413 in fetch_ext.rs) — check before issuing
- On block: return `Response { status: 0, body: "", ok: false }` matching browser CSP-blocked fetch shape
- Add `BOXIDE_CSP_BYPASS=1` env var as escape hatch for benchmarking purity

### Day 6 — Img-src + frame-src + style-src + remaining directives
- Iframe enforcement at `crates/browser/src/iframe.rs:79,177` (`frame-src` directive, falls back to `child-src` then `default-src`)
- Image fetches: probably mostly out of our hot path but cover for completeness
- Style-src for inline styles — likely out of scope (we don't run inline styles separately) but document
- Tests for each directive against real fixtures

### Day 7 — `securitypolicyviolation` event reporting
- Dispatch the event on `document` and `window` per spec
- Event fields: `blockedURI`, `effectiveDirective`, `originalPolicy`, `violatedDirective`, `disposition: "enforce"` (we don't do report-only)
- Console error: `Refused to load the script '<url>' because it violates the following Content Security Policy directive: "<directive>"`
- Test: `page.addEventListener('securitypolicyviolation', e => ...)` catches it

### Day 8 — Behind-flag + integration verification
- Add `profile.enforce_csp: bool` (default `true` for chrome_130_*)
- Live diff: re-run walmart with browser_oxide and confirm `/akam/13/` is not in network log
- 126-site holistic sweep with CSP on vs off
- Document any regressions (some sites might break legitimate flow if their CSP is too tight)

---

## Phase 2 — Remaining Chrome JS-surface parity (≈3-4 days)

Source: `docs/CHROME_JS_SURFACE_PARITY_2026_04_29.md`

The five sub-1hr fixes shipped today. The remaining gaps from that doc:

### Day 9 — `AudioContext` + `OfflineAudioContext`
- Stub in `window_bootstrap.js`: constructor, `currentTime`, `sampleRate=44100`, `createOscillator`, `createDynamicsCompressor`, `createAnalyser`, `createBuffer`, `decodeAudioData`, `destination`
- `OfflineAudioContext.startRendering()` returns deterministic AudioBuffer matching real Chrome's CreepJS audio fingerprint (already known — we have the values from CreepJS parity work)
- Already partially tracked in `MEMORY.md` Critical Findings #5

### Day 10 — `window.visualViewport`, `InputDeviceCapabilities`, `MediaSession`
- `visualViewport`: width/height/scale/offsetTop/offsetLeft, addEventListener stub
- `InputDeviceCapabilities`: constructor with `firesTouchEvents` boolean; expose on Pointer/Touch event prototypes
- `navigator.mediaSession`: real `MediaSession` instance with `playbackState`, `metadata`, `setActionHandler`

### Day 11 — `document.elementFromPoint` viewport-aware
- Currently returns `body` unconditionally — single-call detectable
- Walk DOM looking up which element actually contains a point given our taffy layout
- Match real Chrome behaviour: returns `null` for points outside viewport, returns the topmost hit element for points inside

### Day 12 — Re-run parity probes + 126-site sweep
- Verify all 5 perimeterx_surface_parity gates still green
- v13 byte-parity test still passes
- Holistic sweep — measure delta from CSP+parity work

---

## Phase 3 — Site-specific failure audit (≈3 days)

After Phases 1-2, re-baseline against 126 sites. For each site still failing, capture:
- Network requests (with vs without CSP enforcement)
- Cookie state on landing (`_abck`, `_px3`, `bm_sz`, etc.)
- Any console messages from the site's own JS

Compare against Playwright capture for same site (which works). Identify the **specific** delta per site. Bucket failures:

- **TLS-layer reputation** (`_abck` set unfavorable on first request before any JS) → TLS fingerprint work, distinct stream
- **JS surface still wrong** → patch the specific surface that diverges
- **Behavioral signal absent** → defer to behavioral primitive work
- **Vendor-specific challenge mechanics** → defer to per-vendor module

Day-grain breakdown TBD based on what bucket dominates. Likely 3 days of investigation produces a sharp picture of remaining failure modes.

---

## Phase 4 — Behavioral signals primitive (≈2 days, only if Phase 3 surfaces it)

If multiple sites fail because they require visible mouse/scroll/click events:
- Sigma-lognormal mouse path (literature-standard model for human cursor motion)
- Scroll cadence with realistic deceleration
- One click-on-body warmup
- Wire as default-on in `Page::navigate`

Already roadmapped in earlier docs; may not need this if Phase 1 + 2 alone moves the needle enough.

---

## Total budget

- Phase 1: 8 days (clear scope)
- Phase 2: 4 days (clear scope)
- Phase 3: 3 days (audit, scope-defining)
- Phase 4: 2 days (conditional)

**Worst case ≈17 days, best case ≈12 days.**

Out of scope (deferred to separate streams):
- TLS fingerprint hardening (already mostly done; live `_abck` reputation is a separate research task)
- Vendor-specific challenge solvers (Kasada POW, DataDome JS-VM, captcha) — each is a multi-day spike with policy review attached
- Russian residential-proxy operational work
