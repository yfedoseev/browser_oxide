# 01 — The architectural principle: zero per-engine runtime logic

## The rule

The browser_oxide runtime must contain **zero per-engine logic**. The words
`kasada`, `akamai`, `wbaas`, `cloudflare`, `datadome`, `perimeterx`, `shape`,
`imperva`, `ddos-guard`, `qrator`, `smartcaptcha` must not appear as string
literals or conditional branches in any production code path under:

- `crates/browser/src/` (Page, navigate, event loop, etc.)
- `crates/js_runtime/src/` (extensions, bootstraps, ops)
- `crates/net/src/` (HTTP client, TLS, cookies)
- `crates/stealth/src/` (profiles, presets)

Per-engine code IS allowed — and expected — in:

- `crates/browser/tests/` (probes, regression tests, debug diagnostics)
- `docs/` (notes, research, per-site writeups)
- `/tmp/` and other scratch locations (one-off experiments)

## Why this rule exists

Every open-source stealth browser that actually passes anti-bot engines does
it this way. The per-engine logic you find in GitHub lives in commercial
remote solvers (Hyper-Solutions `hyper-sdk-js`, RiskByPass demo scripts) that
are not browsers — they're HTTP clients that POST solved payloads to a paid
API and replay the response. No open-source browser runtime has "if this is
Kasada" branches. BotBrowser, Camoufox, undetected-chromedriver, nodriver,
puppeteer-extra-stealth — all generic.

The mechanism that lets them be generic is one specific primitive:

> **Init scripts live at the frame/browsing-context level, and are replayed
> before every new Document's first `<script>` runs.**

In Chromium this is the `Page.addScriptToEvaluateOnNewDocument` CDP command.
In BotBrowser it's C++ patches in `third_party/blink/renderer/modules/` that
run before any JS in the renderer. In Camoufox it's Firefox patches at the
same layer. The result: every `location.reload()`, every `history.pushState`,
every meta-refresh, every cross-origin navigation, every Worker startup —
they all inherit the same fingerprint/capability configuration for free.
Challenge scripts run in that environment, do their thing, write cookies,
and navigate. The browser doesn't need to understand the challenge.

## What "generic" looks like in practice

**Wrong (per-engine):**
```rust
if let Some(kpsdk_ct) = fetch_log.find_kpsdk_token() {
    retry_headers.push(("x-kpsdk-ct", kpsdk_ct));
}
if body.contains("sec-if-cpt-container") {
    // Akamai interstitial
    run_akamai_solver(&body);
}
if let Some(wbaas) = headers.get("status-no-id") {
    eprintln!("WBAAS detected: {wbaas}");
}
```

**Right (generic):**
```rust
// Just navigate. Run scripts. Honor location.reload() / location.href =
// / meta refresh. Loop until stable or max iterations. The scripts do
// whatever they need to do, and cookies set by the scripts flow on the
// next request through the normal cookie jar.
for _ in 0..max_iterations {
    let resp = client.get(current_url).await?;
    let page = build_page_with_scripts(&resp.text(), current_url).await?;
    run_event_loop_until_idle(page).await?;
    if let Some(next_url) = page.take_pending_navigation() {
        current_url = next_url;
        continue;
    }
    return page;
}
```

No detection, no token extraction, no engine-specific anything. The
challenge script runs as normal JS. If it sets `document.cookie`, the cookie
goes to our cookie jar. If it calls `location.reload()`, our runtime honors
the pending navigation signal and does another iteration. If it patches
`window.fetch` and makes an XHR, the XHR goes through our fetch ext. Session
state persists via cookies (in the HttpClient jar) and via localStorage/
sessionStorage (in the V8 isolate, which also survives if we keep the
isolate alive across same-frame navigations).

## The two things we compensate for

Two real differences between our runtime and Chrome that the refactor must
handle:

1. **`location.reload()` and `location.href = ...` are no-ops today.** In
   real Chrome they trigger a new navigation. Until we fix that, challenge
   scripts that call them do nothing and the browser appears stuck on the
   interstitial. Fix: set `globalThis.__pendingNavigation` from the
   Location setter and have the Rust navigate loop watch for it.

2. **Our bootstrap runs once per `JsRuntime` construction.** Real Chrome
   re-runs init scripts for every new Document via CDP
   `Page.addScriptToEvaluateOnNewDocument`. Our rough equivalent: drop the
   old `JsRuntime` on navigation and build a new one, which re-runs
   bootstraps for free. Cookies survive because they're in the HttpClient
   jar, not the V8 isolate. localStorage survives via the stealth profile's
   persistent storage (if we wire that up) or by reusing the same underlying
   storage backend across isolates.

## The hard truth about the tier-1 blockers

Research (see `03_research_landscape.md`) confirmed that **no open-source
browser passes adidas/homedepot (Akamai BMP v3) or canadagoose/hyatt
(Kasada)**. BotBrowser's own test suite specifically avoids those — it uses
aircanada, stubhub, wizzair for equivalent-engine testing. The only tools
that pass those sites are commercial remote solvers that sell per-site
tuning as a product.

This means: **the zero-per-engine refactor will not, by itself, unlock the
tier-1 blockers**. It cleans the architecture but leaves the capability
gaps (T1.1 skia-safe, T1.2 fonts, T1.4 WebGL) untouched. Those are what's
actually between us and those sites. See `05_capability_gaps.md`.

## When you're tempted to violate the rule

You'll hit a site where a small per-engine hack would "obviously" fix it.
Resist. Ask yourself: what capability is missing that, if implemented
generically, would make this hack unnecessary? That's the real thing to
build. Examples:

- "Just forward `x-kpsdk-ct` on retry" → **implement `location.reload()`
  properly**; Kasada scripts do a reload after solving in real Chrome.
- "Just detect the Akamai interstitial and inject a delay" → **implement
  `<meta http-equiv="refresh">` handling**; the interstitial sometimes uses
  that.
- "Just hardcode a user-agent for this specific site" → **improve the
  stealth profile selection**; the UA should match your TLS fingerprint,
  your Sec-CH-UA, and your navigator.userAgentData consistently.

Every hack is a capability gap in disguise. Find the gap, fix it at the
right layer, delete the hack.

## The exception: probes and tests

Test and probe files are explicitly allowed to have per-site/per-engine
content. `crates/browser/tests/adidas_sensor_capture.rs`,
`wildberries_solver_diag.rs`, `blocker_rigorous_probe.rs` all name specific
sites. That's correct — tests should be specific enough to catch
regressions.

The only rule for tests: **they must not import from a per-engine runtime
module, because there shouldn't be one.** If you ever need to write a test
that calls into code you wish existed in the runtime, stop and think: is
this a capability the runtime should have for everyone, or is it a debug
tool the test can reimplement locally?
