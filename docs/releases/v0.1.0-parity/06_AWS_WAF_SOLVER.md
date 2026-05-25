# 06 — AWS WAF Solver (challenge.js → token)

**Status:** open research problem. Plan, not a recipe.
**Scope owner:** solver work (Phase 2 of `00_README.md`).
**Sites blocked:** amazon-de, amazon-in, amazon-com-au, amazon-jp, imdb (consistent), plus high-variance regressions on amazon-co-uk / amazon-com / amazon-ca / amazon-fr.
**Bar to declare this chapter done:** amazon-de / amazon-in / amazon-com-au pass strict (`L3-RENDERED` AND body ≥ 15 KB) at ≥ 70 % rate on a 10-run isolated A/B against HEAD. imdb and amazon-jp are stretch.

This chapter is **honest about uncertainty**. The AWS WAF JS challenge is closed-source, the `challenge.js` is signed/minified/obfuscated, and the `getToken()` no-op we observe inside BO is silent — AWS WAF does not say *why* it refuses to issue a token. The plan below is a research plan, not "ship X and it works".

---

## TL;DR

1. The challenge HTML (2011 B for Amazon, 1995 B for IMDb) is verified-correct. It loads a per-tenant `challenge.js` (~50–100 KB, AES-CBC-encrypted body with a WebAssembly proof-of-work module), calls `AwsWafIntegration.saveReferrer()` + `getToken()`, then `location.reload(true)` once the `aws-waf-token` cookie is set ([AWS WAF docs](https://docs.aws.amazon.com/waf/latest/developerguide/waf-js-challenge-api-get-token.html)).
2. BO **does** execute `challenge.js` — proof: a telemetry POST to `awswaf.com/.../report` fires (logged via `crates/browser/src/page.rs:1061`). But `getToken()` never reaches its `then(token => ...)` continuation — no `/verify` POST, no `aws-waf-token` cookie, no `location.reload()`, so the 2011-byte stub is the final body.
3. Conclusion: `challenge.js` runs a fingerprint check **before** issuing the token, our engine fails it, and it silently bails.
4. There are three plausible solver tracks (engine-side stealth patch, Rust-side token POST, V8-side bypass patch). All three target the **private** `vendor_solvers` crate for implementation; the public engine gets only generic primitives (drain caps already exist, CSP relaxation hook already exists per `crates/browser/src/challenge.rs:148`). Per `CLAUDE.md`: do NOT add vendor-specific bypass code to the public engine.
5. The realistic ceiling is **not** 5/5 even with a perfect solver. The corpus shows AWS-side risk-rolling: amazon-co-uk (same code, same IP, four profiles) → chrome 696 KB, pixel 2011 B, iphone 1 MB, firefox 694 KB. Some fraction of the 2011-B stubs are AWS rolling the dice against us, not our fingerprint.

---

## 0. AWS WAF challenge background (grounded)

### 0.1 What the protocol is

Public docs (cited inline) give us this picture:

- AWS WAF's **Challenge** rule action serves a small HTML page that runs a JavaScript proof-of-work. Once the JS produces a valid token, the cookie `aws-waf-token` is set and any subsequent request to a protected endpoint must carry that cookie ([Challenge & CAPTCHA actions blog](https://aws.amazon.com/blogs/networking-and-content-delivery/protect-against-bots-with-aws-waf-challenge-and-captcha-actions/)).
- The token is "encrypted and tamper-proof" and contains a timestamp (default 5-min immunity), a browser-environment fingerprint hash, the puzzle type solved, and the token domain (same blog).
- The integration is installed by adding `<script src="…/challenge.js" defer>` in `<head>`. That script "automatically retrieve[s] a token in the background on page load" ([JS challenge API guide](https://docs.aws.amazon.com/waf/latest/developerguide/waf-js-challenge-api.html)).
- The script exposes a small public surface: `AwsWafIntegration.{fetch, getToken, hasToken}` per the [API specification](https://docs.aws.amazon.com/waf/latest/developerguide/waf-js-challenge-api-specification.html). The HTML stub we receive also calls `saveReferrer()`, `checkForceRefresh()`, and `forceRefreshToken()` — these are documented in the spec implicitly (the AWS sample integrations use them) but not in the public spec page.
- `getToken()`: "If an unexpired token is already available, the call returns it immediately. Otherwise, the call retrieves a new token from the token provider, **waiting for up to 2 seconds** for the token acquisition workflow to complete before timing out. If the operation times out, it throws an error" ([getToken doc](https://docs.aws.amazon.com/waf/latest/developerguide/waf-js-challenge-api-get-token.html)). This 2 s ceiling matters: our 8-second drain window at `crates/browser/src/page.rs:3400` is plenty if the workflow ever progresses; the issue is it *doesn't* progress.

### 0.2 What AWS does NOT publish

The fingerprint signal list is **not** documented. The blog only says the token contains "a generated hash made up of a collection of data points on the client's browser environment". The puzzle types ("Challenge" silent + interactive "CAPTCHA") are documented but the actual JS implementation of either is closed.

Third-party reverse-engineering ([xKiian/awswaf](https://github.com/xKiian/awswaf), [neiii/aws-waf-solver](https://github.com/neiii/aws-waf-solver)) suggests:

- Multiple proof-of-work flavors: HashcashScrypt, SHA-256, NetworkBandwidth.
- A WebAssembly module shipped inside `challenge.js` actually computes the PoW.
- Endpoints are `/inputs`, `/verify`, `/report` on the tenant token host (e.g. `1c5c1ecf7303.d474e66d.us-west-2.token.awswaf.com`). Confirmed in our own internal notes at `~/projects/browser_oxide_internal/docs/DEEP_NEXT_STEPS_2026_04_28.md:34` ("Token endpoint format: `https://<id>.token.awswaf.com/<id>/<id>/inputs?client=browser`. Body: encoded fingerprint payload").
- AES-CBC is involved: `gokuProps.iv` + `gokuProps.key` are AES initialization vector + (encrypted) key material, `gokuProps.context` is an opaque envelope. The plaintext is consumed inside the WASM module.

**Caveat:** none of the above is authoritative for the *current* protocol. AWS WAF rotates obfuscation regularly. Treat anything found in third-party reverse-engineering as a starting hypothesis, not ground truth.

### 0.3 What our code already knows

- Vendor detection: `crates/browser/src/page.rs:1061` logs `[vendor-detect] aws-waf <action> on <url>` whenever the initial response carries `x-amzn-waf-action`. No flow change — pure observation.
- No solver registered: `Page::default_solvers()` at `crates/browser/src/page.rs:850` returns an empty `Arc<[]>`. The public engine ships no AWS-WAF handling code.
- The `ChallengeSolver` trait exists at `crates/browser/src/challenge.rs:103` with the lifecycle (observe_response → prepare_request → detect → solve → relax_response_csp → solved_signal) needed for any AWS-WAF solver in `vendor_solvers`.

---

## 1. Capture the challenge.js (reproducible recipe)

This is the prerequisite for *any* of the solver tracks. Without a deobfuscated `challenge.js` we are guessing what AWS WAF wants.

### 1.1 Pull the stub

```bash
# Amazon DE — the canonical 2011-byte challenge stub.
curl -sS -A 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36' \
     -H 'Accept: text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8' \
     -H 'Accept-Language: en-US,en;q=0.9' \
     'https://www.amazon.de/' \
     -o /tmp/aws_waf_stub.html

wc -c /tmp/aws_waf_stub.html        # expect 2011 (give or take a few; minor goku context jitter)
grep -E 'gokuProps|challenge.js|AwsWafIntegration' /tmp/aws_waf_stub.html
```

If the body is much larger (~700 KB+) you got the real amazon page — re-run; the WAF rolls per-request. Repeat until you see the 2011-B body. (You can also force it with a deliberately-suspicious UA; see §4.3.)

### 1.2 Extract the challenge.js URL

```bash
CHL_URL=$(grep -oE 'https://[a-f0-9]+\.[a-f0-9]+\.[a-z0-9-]+\.token\.awswaf\.com/[^"]*challenge\.js' /tmp/aws_waf_stub.html | head -1)
echo "$CHL_URL"
# Example: https://1c5c1ecf7303.d474e66d.us-west-2.token.awswaf.com/<tenant>/<path>/challenge.js
```

The first two hex segments are the **tenant id** + **integration id**. Region is in the host. These rotate but slowly.

### 1.3 Pull challenge.js

```bash
curl -sS -A 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36' \
     -H 'Referer: https://www.amazon.de/' \
     "$CHL_URL" \
     -o /tmp/challenge.js

wc -c /tmp/challenge.js             # expect 50000–150000
head -c 200 /tmp/challenge.js       # look for the wrapper signature; usually `!function(){...}` or `(function(){...})()`
```

### 1.4 Deobfuscate (manual step — no CLI single-shot)

`challenge.js` is mangled and string-encrypted. Two-stage approach:

```bash
# Stage 1: pretty-print
npx --yes prettier --parser babel /tmp/challenge.js > /tmp/challenge.pretty.js
wc -l /tmp/challenge.pretty.js      # ~3k-10k lines after formatting
```

Stage 2 (manual, in a browser):
- Paste `/tmp/challenge.pretty.js` into [obf-io.deobfuscate.io](https://obf-io.deobfuscate.io/). This tool unwraps string-array indirection and partially un-mangles control-flow flattening. Save the output as `/tmp/challenge.deobf.js`.
- Open `/tmp/challenge.deobf.js` in your editor. Look for:
  - **AwsWafIntegration export**: search for `saveReferrer`, `getToken`, `checkForceRefresh`. They will be properties on an object that ends up bound to `window.AwsWafIntegration`.
  - **The fingerprint collector**: usually one function that builds an object with 50+ keys from `navigator`, `screen`, `window.WebGL*`, `AudioContext`, etc. Sometimes named after a Greek letter or a single letter (`O`, `Ω`).
  - **The fetch sites**: search for `/inputs`, `/verify`, `/report`, `.token.awswaf.com`, `'POST'`. These are the network endpoints the JS hits.
  - **The WASM entry**: search for `WebAssembly.instantiate`, `WebAssembly.compile`, `.wasm`. The binary is typically a base64 blob inside the JS (so `atob('AGFzbQ==…')` is a strong tell — `AGFzbQ` is "\0asm" in base64, the WASM magic header).
  - **Where `getToken` returns early**: this is the bug surface. Search for paths from `getToken` to `Promise.resolve(undefined)` / unhandled `reject(...)`. If `getToken` resolves with no value when a fingerprint check fails, that's why `then(token => location.reload())` never fires (the docs say it *throws* on timeout — but if our code never invokes `reject` either, the promise stays pending forever; consistent with our observation).

Save findings into `/tmp/challenge.notes.md` keyed by hash of `/tmp/challenge.js` so you can re-use after future rotations.

### 1.5 Capture both engines side-by-side

The fingerprint mismatch is what we want to isolate. Use the BO↔Camoufox auto-diff tool spec'd in `04_TOOLING_SPEC.md` (chapter 04 in this same release directory). Run amazon-de under both engines, with `BROWSER_OXIDE_DEBUG_NAV=1`. The diff should show:

- **What requests does Camoufox make that BO doesn't?** Camoufox's run will include `POST .../verify` (or `.../inputs` for the input-collector flavor) and a `200 OK` setting `Set-Cookie: aws-waf-token=...; Path=/`. BO's run will only show the `POST .../report` telemetry.
- **What body does Camoufox send to /verify or /inputs?** This is the gold seam — every field in that payload is a fingerprint our engine has to either match or not appear to *fail*.
- **What is the response to /verify?** Usually a small JSON `{ "token": "<JWT-like blob>" }` plus the `Set-Cookie` header.

Capture artifacts:

```bash
# Camoufox side (Playwright trace dump)
python /tmp/cam_capture.py amazon-de --har /tmp/cam_amazon_de.har

# BO side  
RUST_LOG=net=trace,browser=debug,js_runtime=debug \
  BROWSER_OXIDE_DEBUG_NAV=1 \
  target/release/examples/sweep_metrics chrome_148_macos \
    /tmp/just_amazon_de.json /tmp/bo_amazon_de.json 2>&1 | tee /tmp/bo_amazon_de.log

# Diff helpers
jq '.log.entries[] | select(.request.url | test("awswaf|amazon\\.de"))' /tmp/cam_amazon_de.har > /tmp/cam_amazon_de.requests.json
grep -E 'awswaf|amazon\.de' /tmp/bo_amazon_de.log > /tmp/bo_amazon_de.requests.log
```

The diff result is a research artifact — file under `docs/research_2026_05_24/awswaf/` (create) and reference from `15_OPEN_QUESTIONS.md`.

### 1.6 Re-pull after rotation

`challenge.js` body rotates. Add to a regression script:

```bash
HASH=$(sha256sum /tmp/challenge.js | cut -c1-12)
mkdir -p docs/research_2026_05_24/awswaf/captures
cp /tmp/challenge.js docs/research_2026_05_24/awswaf/captures/challenge.$HASH.js
```

When the hash changes, re-run the deobfuscate pass and diff. AWS's typical rotation cadence is monthly-ish; the fingerprint-check shape rarely changes within a rotation, just the obfuscation layer.

---

## 2. Identify the fingerprint check

Once `/tmp/challenge.deobf.js` is readable, the goal is: find **the one signal** (or N signals) that BO emits incorrectly. Below are the candidate signals AWS WAF *plausibly* checks, paired with where BO produces them — so a contributor can grep, compare to a real Chrome capture, and confirm or rule out.

For each candidate the workflow is:
1. Find the read in `challenge.deobf.js` (grep for the property name).
2. Find the corresponding BO emission point (file:line below).
3. Capture the value real Chrome 148 emits (in DevTools console on a clean profile, *no extensions*).
4. Diff. If different → record the discrepancy.

### Candidate signals (ordered by historical hit-rate against WAFs)

| # | Signal | BO emission point | How to verify |
|---|---|---|---|
| 1 | `navigator.webdriver` | `crates/js_runtime/src/js/window_bootstrap.js:991-995` (`Navigator.prototype.webdriver` getter returns `false`, masked native) — also re-applied in child realm at `:1657` and worker scope at `crates/js_runtime/src/js/worker_bootstrap.js:124`. | `console.log(Object.getOwnPropertyDescriptor(Navigator.prototype, 'webdriver').get.toString())` — must be `function get webdriver() { [native code] }`. |
| 2 | `navigator.userAgent` vs `navigator.userAgentData` vs sec-ch-ua | UA string at `crates/stealth/profiles/chrome_148_macos.yaml:16`. UA-CH headers built in `crates/net/src/headers.rs`. JS surface in `window_bootstrap.js` (search `userAgentData`). | The UA in JS, the UA in the HTTP header, and the sec-ch-ua brand list must all describe the same browser. Mismatch = guaranteed flag. |
| 3 | `navigator.permissions.query({name:'notifications'})` | `crates/js_runtime/src/js/window_bootstrap.js:5106` notes "do not override navigator.permissions.query here"; the impl is in `cleanup_bootstrap.js:272-278`. | Real Chrome 148 on a fresh profile returns `{state: 'default'}` for notifications, microphone, camera. NOT `'denied'`. |
| 4 | `navigator.hardwareConcurrency` / `navigator.deviceMemory` | `window_bootstrap.js:974, 1031` (`_defNav('hardwareConcurrency', ...)`). Profile values at `chrome_148_macos.yaml:35-36` (8 cores, 8 GB). | Chrome on a real M3 Mac returns `hardwareConcurrency: 8`, `deviceMemory: 8`. These match; low-risk. |
| 5 | Canvas hash (`<canvas>.toDataURL()` + `getImageData`) | `crates/canvas/` + `crates/js_runtime/src/js/canvas_bootstrap.js`. Seeded by `canvas_seed` (profile YAML line 68). | The seed is stable per-profile; the question is whether it *matches a real Chrome distribution*. WAFs often blacklist seeds that show up in too many requests. Rotate `canvas_seed` randomly per-Page (currently profile-static). |
| 6 | WebGL `getParameter(UNMASKED_VENDOR_WEBGL)` / `UNMASKED_RENDERER_WEBGL` | `canvas_bootstrap.js:443` (`getParameter`). Profile values at `chrome_148_macos.yaml:40-41` ("Google Inc. (Apple)" / "ANGLE (Apple, ANGLE Metal Renderer: Apple M3, Unspecified Version)"). | These are well-formed but check capitalization/spacing matches a real M3 Chrome dump byte-for-byte. WAFs match exact strings. |
| 7 | WebGL `getSupportedExtensions()` | `canvas_bootstrap.js:485` (`getExtension`) + the supported-list builder. | Real Chrome 148 returns ~30 extensions in a specific order. Capture with: `(()=>{const c=document.createElement('canvas');return c.getContext('webgl').getSupportedExtensions().join('\n')})()` — diff against BO output. Order matters. |
| 8 | `AudioContext` fingerprint (offline render of a DynamicsCompressor) | `audio_seed` in profile YAML:69. Audio impl is in `crates/audio/` (per workspace) and the DynamicsCompressor port mentioned in `memory/tier1_priority_for_akamai.md`. | The mirror-realm fix passes creepjs; that doesn't mean it passes AWS. Capture an offline-context buffer hash from real Chrome 148 and compare. |
| 9 | `performance.now()` granularity / monotonicity | `crates/js_runtime/src/js/timer_bootstrap.js:169-188`. `_origNow` at `window_bootstrap.js:2815`. Worker copy at `worker_bootstrap.js:137`. | Chrome 148 quantizes `performance.now()` to 100 µs in cross-origin-isolated contexts and 5 µs otherwise (Spectre mitigation). BO's `op_perf_now_humanized` may quantize differently — verify it matches Chrome's 5 µs floor on top-level pages. |
| 10 | Font enumeration (`document.fonts.check`) | Unknown — needs grep in `crates/css_parser/` and `dom_bootstrap.js`. | If BO has *no* fonts loaded vs Chrome's bundled web fonts, that's a signal. Low priority — most WAFs find this too noisy. |
| 11 | `navigator.getBattery()` | `cleanup_bootstrap.js:15` (delete), `window_bootstrap.js:1163, 5331` (stub registration + native masking). | Battery API is deprecated in Chrome 148 (returns a stub). Make sure we either return the stub or are absent consistently. |
| 12 | `WebAssembly.{compile, instantiate}` | `window_bootstrap.js:17-27` (`instantiateStreaming`, `compileStreaming` wrappers). | The challenge.js *runs* a WASM module. If our WASM impl differs at any spec edge (e.g. SIMD opcode coverage, Threads & Atomics, exception handling), the PoW could fail silently. Cross-check the WASM features exposed: `WebAssembly.validate(new Uint8Array([0,97,115,109,1,0,0,0]))` should be true, but feature-detect `simd128` / `mutable-globals` / `bulk-memory` to know what challenge.js can use. |
| 13 | `Worker` / `MessageChannel` round-trip | `crates/js_runtime/src/extensions/worker_ext.rs`. Stealth bits in `worker_bootstrap.js`. | Some `challenge.js` builds spin up a Worker to compute the PoW off-main-thread. If our Worker has the wrong `self.navigator` shape (or the `MessagePort` cycle doesn't behave like Chrome's), the in-Worker code may bail. duolingo confirms Worker is a real surface — see chapter 05. |
| 14 | `document.referrer` + `saveReferrer()` | The stub calls `AwsWafIntegration.saveReferrer()` *before* `getToken()`. If our `document.referrer` is the wrong shape (empty when it should be the entry URL, etc.), the saved-referrer state may invalidate the token request. | Cross-check `document.referrer` value on the second iteration after a `location.reload()`. |
| 15 | Touch events / pointer events (`ontouchstart in window`) | Profile YAML:37 (`max_touch_points: 0`). `dom_bootstrap.js:2706` (in interface registration list). | macOS Chrome has `maxTouchPoints: 0`, `ontouchstart` undefined. Mobile profiles flip both. WAF cross-checks UA vs touch capability. |
| 16 | Plugin enumeration | `window_bootstrap.js` `_navPlugins`. Profile YAML:60-62 (`pdf_viewer_enabled: true`, 5 plugins, 2 MIMEs). | Chrome 148 returns a specific 5-plugin shape (Chrome PDF Viewer + 4 alias entries). Verify exact name/filename strings match. |
| 17 | `Notification.permission` | (Search in window_bootstrap.js.) | Should be `'default'` on a secure context — diff vs real Chrome. |
| 18 | TLS ClientHello / HTTP/2 frame ordering | `crates/net/src/tls.rs:1-687` (boring2 + Chrome 147 ClientHello). | Already verified byte-for-byte against real Chrome 147 (per `memory/session_delta_2026_05_10.md`). LOW probability this is the issue, but worth re-verifying with `tls.peet.ws` and an H2 frame dump. |

### How to actually run this

Open one terminal, run the deobfuscated `challenge.js` in a real Chrome devtools console while paused on its first instruction (set a breakpoint on `AwsWafIntegration.getToken`). Walk the call stack. Every property read on `navigator`, `window`, `screen`, `document` is a candidate signal. List them, then cross-check each against BO using the table above.

This is **slow, manual reverse-engineering**. Budget: 1–3 engineer-days for the first pass. Subsequent rotations should take an hour because the structure (collector → encrypt → POST /verify) is stable; only the property-name mangling changes.

### Plan B if deobfuscation is too hard

Run BO with a **deliberately-broken** stealth profile (e.g. set `navigator.webdriver` to `true`, force `hardwareConcurrency: 1`, fake the WebGL renderer). Bisect: see which single bad value makes the existing borderline pass (e.g. amazon-co-uk) firmly fail. That gives you the leverage set in reverse without ever reading `challenge.js`.

---

## 3. Solver design — three alternatives

All three target the **private `vendor_solvers` crate** for the bypass-bearing code. Per `CLAUDE.md`: "Per-vendor challenge solving is out of scope here. The engine exposes a `browser::ChallengeSolver` trait + `Page::navigate_with_solvers` hook; the concrete Akamai/Kasada/DataDome/Cloudflare implementations live in the private `vendor_solvers` companion crate."

`vendor_solvers` already has the four legacy solvers (see `~/projects/browser_oxide_internal/crates/vendor_solvers/src/lib.rs`); an AWS WAF solver would be a fifth (`aws_waf_solver.rs`) registered via the existing `default_solvers()` factory.

### Alternative A — Engine-side stealth patch (PUBLIC engine)

**Idea:** find the fingerprint signal AWS WAF rejects us on (§2). Fix the emission inside the public engine (e.g. align WebGL extension list to Chrome 148, fix performance.now quantization). No vendor-specific code; the fix is "be more faithfully Chrome".

**Pros:**
- Helps **every** AWS-WAF-protected site, not just amazon. Probably helps a handful of other WAFs (Akamai BMP, Kasada) that score the same signals.
- Zero new vendor dispatch logic. The existing engine flow just works once the page completes.
- Stays inside the OSS license boundary.
- This is what the engine is *supposed* to do — it's the principled fix.

**Cons:**
- Requires the specific mismatch be *identified*, not just hypothesized. §2 is multi-day exploratory work.
- If the mismatch is structural (e.g. our Worker really doesn't satisfy Chrome's MessageChannel semantics), the fix is large and engine-wide.
- Doesn't help if AWS WAF rotates to a different signal next month.

**Effort:** 3 d to identify, 1–5 d to fix per signal. Budget 1 engineer-week per signal candidate.

**Confidence this works:** medium — there *is* a fingerprint mismatch (proven by the `getToken` no-op). Once found, this is the right fix. Probability of finding it cleanly: 50–70 %.

### Alternative B — Rust-side token request (PRIVATE `vendor_solvers`)

**Idea:** detect the AWS-WAF stub in `vendor_solvers::AwsWafSolver::detect()`, extract `gokuProps`, compute the PoW in pure Rust (reimplement the WASM proof-of-work in `rust-crypto`/`sha2`/`scrypt` crates), POST to `/verify`, set the `aws-waf-token` cookie in the shared jar, return `SolveOutcome::Solved` so the navigate loop re-fetches with the cookie.

The skeleton looks like:

```rust
// In ~/projects/browser_oxide_internal/crates/vendor_solvers/src/aws_waf_solver.rs
use async_trait::async_trait;
use browser::{ChallengeKind, ChallengeSolver, Page, SolveOutcome};

#[derive(Default)]
pub struct AwsWafSolver { /* per-origin token cache */ }

#[async_trait(?Send)]
impl ChallengeSolver for AwsWafSolver {
    fn name(&self) -> &'static str { "aws-waf" }

    fn detect(&self, resp: &net::Response, html: &str) -> Option<ChallengeKind> {
        let has_action = resp.headers.iter()
            .any(|(k, _)| k.eq_ignore_ascii_case("x-amzn-waf-action"));
        let has_stub = html.len() < 4096
            && html.contains("AwsWafIntegration")
            && html.contains("gokuProps");
        (has_action || has_stub).then(|| ChallengeKind::new("aws-waf", "stub"))
    }

    async fn solve(&self, _page: &mut Page, client: &net::HttpClient, _kind: ChallengeKind)
        -> SolveOutcome
    {
        // 1. parse gokuProps {key, iv, context} out of `page.html()`.
        // 2. compute PoW (TBD: hashcash-scrypt? sha256? unknown until §1 done).
        // 3. POST <tenant>/verify with the right envelope.
        // 4. on 200, parse {"token":"..."} from body OR read Set-Cookie.
        // 5. inject `aws-waf-token=<...>` into client's jar for this origin.
        // 6. return SolveOutcome::Solved.
        SolveOutcome::Unsolvable
    }
}
```

**Pros:**
- Bypasses fingerprinting entirely — AWS WAF sees only a POST with the right shape.
- Decoupled from V8 / engine churn.
- Cookie ends up in the same shared jar (`net::HttpClient::shared`); next nav-loop iteration sees the cookie and gets the real page.

**Cons:**
- **WASM reimplementation is the hard part.** AWS's PoW is `wasm-shipped`. Either we (a) compile their WASM, run it in `wasmtime` with shimmed imports — bypassing JS entirely; (b) re-implement the PoW algorithms (`hashcash-scrypt`, `sha256`, `network-bandwidth`) in pure Rust. Both routes work for static challenges; both break when AWS rotates the PoW shape.
- The `/verify` body shape and signing scheme is undocumented. We have to reverse it (§1) and re-reverse on every rotation.
- High maintenance: AWS WAF updates the obfuscation regularly; expect 1 break every ~30 days.
- License risk: if we end up *executing* AWS's WASM, that's clearly fine (we're just running code we received over HTTP). If we *re-implement* it, no IP concern but high reverse-eng cost.

**Effort:** 1–2 engineer-weeks for the initial implementation. ~2 days per rotation thereafter.

**Confidence this works:** medium-high if §1 deobfuscation lands cleanly. The protocol is fixed (POST to `/verify` with a known body), so once we know the body shape this is just plumbing.

**Recommended detail variant: load + run the WASM under `wasmtime`.** Capture the WASM blob from the deobfuscated `challenge.js`, write a thin Rust harness that:

```rust
// pseudo-code
let wasm_bytes = pull_wasm_from_challenge_js(&challenge_js)?;
let engine = wasmtime::Engine::default();
let module = wasmtime::Module::new(&engine, wasm_bytes)?;
let mut store = wasmtime::Store::new(&engine, ());
// stub the imports the WASM expects (usually `env.memory`, `env.abort`, a few math fns)
let instance = wasmtime::Instance::new(&mut store, &module, &shimmed_imports)?;
let solve_fn = instance.get_typed_func::<(i32, i32), i32>(&mut store, "solve")?;
let result = solve_fn.call(&mut store, (key_ptr, iv_ptr))?;
```

This sidesteps the "reimplement scrypt" problem entirely. Risk: AWS's WASM may have `js-host`-specific imports (e.g. `getRandomValues`) we have to shim convincingly.

### Alternative C — V8-side token request (PRIVATE `vendor_solvers`)

**Idea:** before `challenge.js` runs, patch out the fingerprint gate so `getToken()` always proceeds to the POST. Rewrite `challenge.js` in-flight (in `vendor_solvers` via a `prepare_request` hook that intercepts the subresource fetch for `*.token.awswaf.com/.../challenge.js` and rewrites the response body) to no-op the check.

Mechanically: identify the regex of the bail line in `challenge.js`. Inject a `script-text-rewriter` on the `net::HttpClient` GET response for `*.token.awswaf.com/*/challenge.js` that:

1. Detects the obfuscated version's bail-pattern (e.g. `if(_0x42(_0x3a)){return Promise.reject(...)}`).
2. Replaces it with `if(false){...}`.
3. Returns the rewritten body to V8.

V8 then runs the patched script; `getToken()` proceeds; the legit POST goes out; cookie comes back; loop re-fetches; pass.

**Pros:**
- Minimal Rust work — most of the logic is `regex.replace_all`.
- Doesn't require us to understand the WASM or `/verify` body shape.
- The PoW *does* still happen (so AWS sees a real token they minted), just without the fingerprint gate.

**Cons:**
- **Brittle.** AWS rotates `challenge.js`'s minifier seed regularly; the regex breaks every ~30 days.
- Requires reliable pattern detection across rotations (probably need 3–5 regex variants).
- If the fingerprint gate isn't in JS but in the WASM (plausible — the WASM could refuse to compute the PoW), this approach fails entirely.
- Net-layer body rewriting needs a new hook in `net::HttpClient::get` — currently there isn't one (BO doesn't have a request-response interception story).

**Effort:** 3–5 days for initial implementation. ~1 day per rotation.

**Confidence this works:** medium — depends entirely on whether the fingerprint gate is in JS or WASM. If in WASM, this fails and you have to fall back to A or B.

### Trade-off matrix

| Criterion | A. Engine stealth | B. Rust token POST | C. JS rewrite |
|---|---|---|---|
| Code lives | PUBLIC `crates/js_runtime`, `crates/stealth` | PRIVATE `vendor_solvers/aws_waf_solver.rs` | PRIVATE `vendor_solvers/aws_waf_solver.rs` + `net` hook |
| Initial effort | 1–2 wk per signal | 1–2 wk + WASM harness | 3–5 d |
| Rotation maintenance | none | 2 d/month | 1 d/month |
| Helps non-amazon WAF sites | yes | only AWS-WAF | only AWS-WAF |
| Risk of silent failure | low (you'd see the fingerprint diff) | medium (PoW could change) | high (regex breaks) |
| Confidence | 50–70 % | 60–80 % once §1 done | 40–60 % |
| Composability with each other | n/a | composes with A | composes with A |

**Recommended order:** A first (it pays off everywhere), B if A doesn't land within 2 weeks, C only as a stopgap during a B rotation.

---

## 4. Validation plan

### 4.1 Per-fix A/B

For *any* change (engine-side OR vendor solver), the validation harness is:

```bash
# Capture baseline (HEAD before fix) — run 10 times, isolated cookie jar each time.
cat > /tmp/just_amazon_de.json <<'JSON'
[{"cat":"shopping","name":"amazon-de","url":"https://www.amazon.de/"}]
JSON

for i in $(seq 1 10); do
  rm -rf /tmp/jar_$i && mkdir /tmp/jar_$i
  target/release/examples/sweep_metrics chrome_148_macos \
    /tmp/just_amazon_de.json /tmp/baseline_$i.json 2>/dev/null
done

# Switch to fix branch, rebuild, repeat.
cargo build --release -p browser --examples
for i in $(seq 1 10); do
  rm -rf /tmp/jar_$i && mkdir /tmp/jar_$i
  target/release/examples/sweep_metrics chrome_148_macos \
    /tmp/just_amazon_de.json /tmp/fix_$i.json 2>/dev/null
done

# Compare strict-pass rates.
pass_rate() { 
  grep -c '"verdict": *"L3-RENDERED"' "$@" 2>/dev/null
}
echo "Baseline pass rate: $(pass_rate /tmp/baseline_*.json)/10"
echo "Fix pass rate:      $(pass_rate /tmp/fix_*.json)/10"
```

**Acceptance per-site:** baseline rate must be ≤ 1/10 (amazon-de is consistently the 2011-B stub today; if baseline shows passes, something else moved). Fix rate must be ≥ 7/10 to declare success. Below that = AWS rolling.

### 4.2 Cross-site validation

The full target set:

| Site | Today (5/24 sweep) | Acceptance |
|---|---|---|
| amazon-de | 2011 B | ≥ 7/10 pass |
| amazon-in | 2011 B | ≥ 7/10 pass |
| amazon-com-au | 2011 B | ≥ 7/10 pass |
| amazon-jp | 2011 B (most runs) | ≥ 5/10 pass (stretch) |
| imdb | 1995 B | ≥ 5/10 pass (stretch) |
| amazon-co-uk | variable (passes ~50 %) | rate goes UP, not down |
| amazon-com | variable | rate goes UP, not down |
| amazon-ca | variable | rate goes UP, not down |
| amazon-fr | variable | rate goes UP, not down |

Run the same 10-run A/B per site. **Important:** route best-of-N across BO profiles (chrome_148_macos / pixel / iphone / firefox) — the gap-analysis (`02_GAP_ANALYSIS.md:188-191`) showed per-profile variation, so a fix that lifts `chrome_148_macos` may not lift `pixel`. Run all four.

### 4.3 Negative test (broken-profile bisect)

Confirm we have isolated the signal, not coincidentally lucked into a pass. Run with the fix + one deliberately-broken signal at a time:

```bash
# E.g. force navigator.webdriver = true via init script
target/release/examples/sweep_metrics chrome_148_macos \
  /tmp/just_amazon_de.json /tmp/broken.json \
  --init-script 'Object.defineProperty(Navigator.prototype, "webdriver", {get:()=>true});'
```

If the fix-with-broken-signal goes back to 2011 B, the signal you broke matters. If it still passes, the signal isn't load-bearing for AWS WAF — find the real one.

### 4.4 Regression gate

After validation, add an `#[ignore]`-by-default network-bound integration test:

```rust
// crates/browser/tests/aws_waf.rs (new)
#[tokio::test(flavor = "current_thread")]
#[ignore = "live network; AWS-side risk roll; expect 7/10 pass"]
async fn amazon_de_passes_with_aws_waf_solver() {
    // requires `vendor_solvers` registered.
    // Run 10 times, assert ≥ 7 returned body ≥ 15 KB.
}
```

Per CLAUDE.md it stays `#[ignore]` (network tests do). Run weekly in the cron'd regression sweep called out in `00_README.md` Success Scorecard.

### 4.5 The honesty test

Before declaring this chapter "done", run the **full 126-site sweep**, 3-run median, with the AWS WAF solver registered. The success criterion is **not** "amazon-de passes once". It is:

- Net change vs HEAD ≥ +4 strict passes (3 amazon variants + 1 imdb at minimum).
- No regression > -2 on any other site (the WAF rolls; small variance is real).
- 3-run median, not best-of-3 — single-run lucky passes don't count.

---

## 5. Out-of-scope (do NOT add to public engine)

Per `CLAUDE.md` and `SCOPE.md`, the bypass-bearing code stays out of the open-source engine. Specifically:

### What MUST NOT go into PUBLIC

- ❌ Any code that calls `awswaf.com` endpoints.
- ❌ Any code that knows the shape of `gokuProps` (key/iv/context).
- ❌ Any AES-CBC / scrypt / hashcash routines that exist to forge a token.
- ❌ Any string match on `AwsWafIntegration` or `aws-waf-token` outside the existing detection logger.
- ❌ Any in-flight rewriting of `challenge.js` body content.

### What MAY go into PUBLIC (engine primitives)

These help any vendor — DataDome, Cloudflare, Akamai included — and have no AWS-WAF-specific knowledge:

- ✅ **Detection-only logging** — already exists at `crates/browser/src/page.rs:1061-1063` for `x-amzn-waf-action`. Pure observation, no flow change.
- ✅ **Longer drain ceiling on small bodies with embedded script-driven reload** — the 8-s drain at `page.rs:3400` already covers AWS WAF's documented 2-s `getToken` wait + comfortable margin. Don't extend further; that just slows passing sites.
- ✅ **CSP relaxation hook for any-vendor challenge document** — already exists via `ChallengeSolver::relax_response_csp` at `crates/browser/src/challenge.rs:148`. AWS WAF stubs don't typically need this (the origin's CSP usually allows `*.awswaf.com`), but if a future capture shows a 403 CSP blocking the challenge.js fetch, the hook is there.
- ✅ **Cookie-write retry on 4xx-after-Set-Cookie** — handy for vendors that issue a token cookie but the next request fires before the jar update commits. Generic primitive; helps any vendor.
- ✅ **Adaptive iteration budget** — already exists (the navigate loop short-circuits when body is small + readyState complete). No change needed; works in our favor for AWS WAF (stops burning the full budget on the 2011-B stub).

### What goes into PRIVATE `vendor_solvers`

- The `AwsWafSolver` impl of `ChallengeSolver`.
- The `gokuProps` parser.
- The PoW computation (whether reimplemented in Rust or executed via `wasmtime`).
- The `/verify` / `/inputs` POST + body builder.
- Per-origin token cache.
- The WASM blob, if pre-extracted (avoid: re-pull it dynamically from the live `challenge.js` each rotation to stay current).

### Where the seam is

The existing `Page::navigate_with_solvers` at `crates/browser/src/page.rs:961` is the entry point. Embedders register:

```rust
// In an embedder's binary (not in browser_oxide proper)
use vendor_solvers::default_solvers;  // now includes AwsWafSolver

let page = browser::Page::navigate_with_solvers(
    "https://www.amazon.de/",
    stealth::presets::chrome_148_macos(),
    3,
    default_solvers(),
).await?;
```

The public engine never imports `vendor_solvers`. The doc-test in `crates/browser/src/page.rs:849` already calls this pattern out.

---

## 6. Files / references

### In the public engine (read-only for this chapter)

- `crates/browser/src/page.rs:1061-1063` — AWS WAF detection logger (the only AWS-aware code we keep public).
- `crates/browser/src/page.rs:828-852` — `Page::with_solvers` / `Page::default_solvers` — the registration seam.
- `crates/browser/src/page.rs:961-970` — `Page::navigate_with_solvers` — the entry point embedders call.
- `crates/browser/src/page.rs:3400` — the 8-second drain ceiling. AWS WAF's documented 2-s `getToken` wait fits inside, with margin.
- `crates/browser/src/challenge.rs:1-204` — the full `ChallengeSolver` trait + lifecycle docs.
- `crates/browser/src/challenge.rs:148` — `relax_response_csp` hook, available if a future AWS-WAF tenant needs CSP suspension.
- `crates/net/src/tls.rs:1-687` — ClientHello impersonation (boring2, Chrome 147). Already byte-perfect vs real Chrome; almost certainly NOT the AWS-WAF bug surface.
- `crates/net/src/headers.rs` — header builder. UA, sec-ch-ua, accept-language; these are what AWS sees pre-JS.
- `crates/stealth/profiles/chrome_148_macos.yaml` — the editable Chrome 148 profile; this is the data side of every JS-visible fingerprint signal §2 lists.
- `crates/stealth/src/presets.rs` — the four built-in presets (chrome_148_macos, pixel, iphone, firefox). Each gets exercised via routing.
- `crates/js_runtime/src/js/window_bootstrap.js:983-995, 1648-1660` — `navigator.webdriver` definition (the #1 historical fingerprint signal).
- `crates/js_runtime/src/js/window_bootstrap.js:5325-5360` — native-code masking. Anti-bot detectors check `Function.prototype.toString` of polyfilled APIs; this masks them.
- `crates/js_runtime/src/js/canvas_bootstrap.js:443-485` — WebGL `getParameter` + `getExtension`. The renderer string + supported-extensions list is a high-value AWS WAF signal.
- `crates/js_runtime/src/js/timer_bootstrap.js:169-188` — `performance.now()` shim. Quantization granularity matters (Spectre mitigation).
- `crates/js_runtime/src/js/cleanup_bootstrap.js:261-279` — `navigator.permissions.query` shim.
- `crates/js_runtime/src/js/worker_bootstrap.js:115-137` — Worker scope's separate `hardwareConcurrency`, `webdriver`, `performance.now`. If `challenge.js` spawns a Worker, this is where it lives.
- `crates/browser/tests/holistic_sweep.rs` — the 126-site corpus definition (canonical pass/fail authority).
- `crates/browser/src/classify.rs` — verdict classifier; defines `L3-RENDERED`, `THIN-BODY`, etc.

### In the private `vendor_solvers` repo (where the work happens)

- `~/projects/browser_oxide_internal/crates/vendor_solvers/src/lib.rs:1-66` — current factory. Add a fifth `Arc::new(AwsWafSolver::new())` to `default_solvers()`.
- `~/projects/browser_oxide_internal/crates/vendor_solvers/src/kasada_solver.rs:1-53` — clean template to copy for `aws_waf_solver.rs`. Same trait, same shape.
- `~/projects/browser_oxide_internal/docs/DEEP_NEXT_STEPS_2026_04_28.md:19-37` — the original 2026-04-28 capture of an AWS WAF interaction. Has the `/inputs?client=browser` URL shape and 795 ms Camoufox timing reference.

### Commit references

- `aecdf19` — the vendor-strip commit. Look here for what was previously public and is now private. If a future contributor wonders "did the engine ever have AWS WAF code?": the answer is no, never. AWS WAF support was scoped into `vendor_solvers` from the start, and the OSS engine has only ever shipped the detection logger.
- `f62584d` — SharedSession; explains why the cookie jar is process-wide. Relevant for vendor_solvers: the solver MUST write `aws-waf-token` into the shared jar, not a per-page jar, or the post-solve reload won't see it.
- `d00bcb2` — `BOXIDE` → `BROWSER_OXIDE` env-var rename. Use `BROWSER_OXIDE_DEBUG_NAV=1` (not `BOXIDE_DEBUG_NAV`) in any new capture scripts.

### External documentation

- AWS — [Using the intelligent threat JavaScript API](https://docs.aws.amazon.com/waf/latest/developerguide/waf-js-challenge-api.html). The integration overview: how challenge.js is meant to be installed, what the API surface is.
- AWS — [Intelligent threat API specification](https://docs.aws.amazon.com/waf/latest/developerguide/waf-js-challenge-api-specification.html). The official method list: `fetch`, `getToken`, `hasToken`. Note: `saveReferrer`, `checkForceRefresh`, `forceRefreshToken` (which the live stub calls) are NOT in this public spec — they are documented only via the AWS sample integrations on GitHub.
- AWS — [How to use the integration `getToken`](https://docs.aws.amazon.com/waf/latest/developerguide/waf-js-challenge-api-get-token.html). Documents the 2-second timeout, the `aws-waf-token` cookie name, and the cross-domain `x-aws-waf-token` header fallback.
- AWS — [Protect against bots with AWS WAF Challenge and CAPTCHA actions](https://aws.amazon.com/blogs/networking-and-content-delivery/protect-against-bots-with-aws-waf-challenge-and-captcha-actions/). The "what's in the token" description (timestamp, browser-env hash, puzzle type, domain).
- AWS — [Optimizing web application user experiences with AWS WAF JavaScript integrations](https://aws.amazon.com/blogs/networking-and-content-delivery/optimizing-web-application-user-experiences-with-aws-waf-javascript-integrations/). High-level: confirms PoW + token, doesn't enumerate fingerprint signals.

### Third-party reverse engineering (treat as hypotheses, NOT ground truth)

- [xKiian/awswaf](https://github.com/xKiian/awswaf) — Go/Python solver. Claims support for "type 'token' / 'Invisible'" and CAPTCHA. Useful as a structural reference for the protocol shape; its specific endpoint paths and crypto routines may already be stale.
- [neiii/aws-waf-solver](https://github.com/neiii/aws-waf-solver) — independent reimplementation. Documents the four-step flow: "discover challenge url → solve PoW (scrypt / sha256 / bandwidth) → generate fingerprint → submit for token". Confirms the multi-PoW-flavor design.
- Both repos are open-research; their licenses and update freshness must be checked before any code is *copied*. Reading them for protocol understanding is fine; lifting code is not, per `CLAUDE.md` license rules (only MIT/Apache-2.0 allowed in our tree; vendor_solvers is private so its license is whatever the embedder chooses, but copying from a GPL'd reverse-eng repo is still a license problem).

### Internal historical context

- `~/projects/browser_oxide_internal/docs/HANDOFF_2026_04_28_98_sites.md:156` — explains the `[vendor-detect]` logger origin; pure observation, used for triage.
- `~/projects/browser_oxide_internal/docs/VERIFICATION_REPORT_2026_04_26.md:342` — notes "first time we have evidence of clean AWS WAF passage" via HuggingFace. Means: AWS WAF is not *uniformly* blocking us — Amazon-tenant specifically is. Worth investigating: is the Amazon WAF configuration more aggressive than the default, or is it the same configuration that just happens to fingerprint amazon traffic harder?
- `memory/state_2026_05_24_*` (your auto-memory) — the up-to-date corpus state; cross-check before declaring numbers.

---

## 7. Risks and uncertainties

This section is explicit because the rest of the doc is a plan, not a prescription. The following are known unknowns:

1. **We do not know what fingerprint signal triggers the bail.** §2 is exploratory. Worst case: it's a structural signal (e.g. `WebAssembly` SIMD feature absence), not a property-getter signal, and the fix is engine-architecture-deep.
2. **The `/verify` or `/inputs` body shape is undocumented.** §1 captures it; §3.B's effort estimate assumes the capture is informative. If the body is fully encrypted by `gokuProps.key` (and not just the PoW result), we have to recover the encryption protocol, which is harder.
3. **AWS rotates aggressively.** Even a working solver becomes a maintenance commitment of ~2 days/month indefinitely. Factor into the v0.1.0 carry-cost.
4. **Non-determinism caps the ceiling.** Even a perfect engine won't see 10/10 on amazon-de. The empirical ceiling (from amazon-co-uk pre-fix behavior) is probably ~85 %. Plan acceptance criteria accordingly: 7/10 is good, 5/10 is "maybe AWS is in a bad mood today, re-run", 3/10 is "did the fix even land".
5. **The fix may move-but-not-flip.** It's possible to push amazon-de from "always 2011 B" to "sometimes 50 KB". 50 KB is still under our strict-pass `15 KB` floor only if it's a partial render; the classifier needs careful inspection. Capture body samples at each fix step.
6. **Engine-side fixes (A) may break other sites.** Tightening `performance.now` quantization, for instance, can affect timing-fingerprint sites that currently pass. The regression gate at §4.5 is non-negotiable.
7. **The deobfuscator may be wrong.** `obf-io.deobfuscate.io` is a best-effort tool. It can mis-unwrap control-flow flattening, leaving identifiers shuffled. Always cross-check by running the deobfuscated `challenge.js` in a real Chrome devtools console and verifying behavior matches the original obfuscated version.
8. **AWS may already have detected the engine class.** If AWS's anti-bot infrastructure shares signal with their other (non-WAF) products (e.g. Amazon's own retail anti-bot, the IMDb-specific layer), our IP / TLS class may be flagged regardless of fingerprint. Cross-check by running BO from a completely different IP — does the 2011 B persist? If yes → it's our engine. If no → it's IP + fingerprint compound, and the engine fix alone won't lift it.

---

## 8. Acceptance summary

This chapter is **done** when ALL of the following hold:

- [ ] `/tmp/challenge.deobf.js` exists, committed to `docs/research_2026_05_24/awswaf/captures/`, has the fingerprint-collector + bail point identified by file location in a `challenge.notes.md`.
- [ ] One of A/B/C is implemented and validated.
- [ ] amazon-de + amazon-in + amazon-com-au all pass at ≥ 7/10 on a 10-run isolated A/B (chrome_148_macos profile).
- [ ] The full 126-site sweep (3-run median, routed best-of-4 profiles) shows net + ≥ 4 strict-pass change vs HEAD, with no individual-site regression > -2.
- [ ] If implementation is in `vendor_solvers`, the public engine has NO new AWS-WAF-specific code beyond the existing logger at `page.rs:1061`.
- [ ] If implementation is engine-side (Alt A), the change is documented as a Chrome-faithfulness fix, not as an AWS-WAF bypass — and it must verify cleanly against a real Chrome 148 capture for the signal in question.
- [ ] A new `#[ignore]` integration test exists in `crates/browser/tests/aws_waf.rs` that the weekly regression sweep runs.
- [ ] `15_OPEN_QUESTIONS.md` updated with whichever risks (§7) remain unresolved after the work lands.

amazon-jp and imdb remain stretch targets — they are listed in `02_GAP_ANALYSIS.md` "hard residual" because AWS-side risk-rolling on those two is empirically harder. Acceptance does NOT require them, but a partial pass (≥ 5/10) on either is a meaningful additional win.
