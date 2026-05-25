# 05 — SPA hydration cluster: reddit / duolingo / booking / douyin

**Owner scope:** SPA work.
**Sites:** 4. **Difficulty band:** 1 EASY, 1 EASY (1.7 KB miss), 2 MED.
**Why this chapter exists:** all four sites return HTTP 200 with HTML, classify as
`L3-RENDERED`, but the body never grows past the SPA shell — Camoufox rendered
40 KB – 1.1 MB; we got 6 – 13 KB. Each site has a single concrete blocker that a
contributor with zero context can identify, fix, and validate against a measurable
acceptance bar.

This doc assumes you have already read:

- `00_README.md` (status, doc index)
- `02_GAP_ANALYSIS.md` (this is sections 1–4 of that doc, expanded)
- `03_BENCHMARK_METHODOLOGY.md` (how the 126 sweep + classifier work)
- `04_TOOLING_SPEC.md` (the per-site capture mode — required for sections 2/3/4 below)

If `04_TOOLING_SPEC.md` does not yet exist when you read this, fall back to the
ad-hoc tooling described in each section. The capture/diff tool is the easier
path; the ad-hoc commands always work.

---

## 0. Common context

### 0.1 The cluster pattern

All four sites share a single failure shape:

1. We do a network GET, receive HTTP 200 with a small HTML body (6 – 13 KB).
2. `build_page_with_scripts_init_and_storage` (`crates/browser/src/page.rs:2814-3450`)
   spins up V8, executes inline scripts, fires `DOMContentLoaded`, drains for up
   to 8 s (`page.rs:3400`).
3. The outer iter loop reads `PENDING_NAV_JS` (`page.rs:1956`). If empty AND no
   recognised anti-bot vendor flag is set, the loop returns the current page.
4. `crates/browser/src/classify.rs:177-181` tags `L3-RENDERED`; the size
   threshold `THIN_SHELL_MAX_BYTES = 15 * 1024` (`classify.rs:47`) is **not
   crossed** → bench summary records `Pass=false` for the site.

So every fix in this chapter has to make ONE of these things happen:

- (a) Set `__pendingNavigation` so the outer loop re-fetches a URL with a freshly
      issued cookie / token (reddit pattern).
- (b) Successfully resolve an async chain that triggers SPA hydration fetches so
      the body grows past 15 KB on the same response (duolingo / booking / douyin
      pattern).
- (c) Unblock a primitive (Worker, MessageChannel, async/await on form submit)
      that the SPA's bootstrap is waiting on.

### 0.2 The classifier is mechanical, the bar is fixed

```
crates/browser/src/classify.rs:47
    pub const THIN_SHELL_MAX_BYTES: usize = 15 * 1024;
crates/browser/src/classify.rs:180-181
    "L3-RENDERED" if len < THIN_SHELL_MAX_BYTES => ChallengeVerdict::ThinShell,
    "L3-RENDERED" => ChallengeVerdict::Pass,
```

A site is a strict pass iff body ≥ 15 360 bytes AND tag = `L3-RENDERED`. There is
no fuzz, no fudge. duolingo at 13 327 bytes is **2 033 bytes shy** — even a
single API response stitched into the DOM would flip it.

### 0.3 Recommended workflow per site

1. **Capture both engines side-by-side** with the spec in `04_TOOLING_SPEC.md`
   (per-site mode of `sweep_metrics` that dumps `body.html`, `fetches.json`,
   `script_errors.json`, `cookies.json`).
2. **Diff** the two `fetches.json` files. The first URL that Camoufox fetched and
   BO did not is almost always the missing hydration call. The script that
   *would* have made that call is what failed.
3. **Search BO's `script_errors.json` for the line of code immediately preceding
   the missing fetch.** Recaptcha / DataDome / SPA frameworks rarely surface
   their internal errors to the user, but our top-level uncaught-error trap
   (`page.rs:3406-3412`) catches them.
4. **Apply a hypothesis** from the per-site sections below.
5. **Validate** by running the per-site command in isolation and checking that
   `body_len > acceptance_bar` (see § 6).

### 0.4 Build the sweep binary once

Code locations are in § 7. Build the runner:

```bash
cd /home/yfedoseev/projects/browser_oxide
cargo build --release -p browser --example sweep_metrics
```

Each per-site section below hand-crafts its own 1-entry corpus JSON, so you
don't need `/tmp/corpus.json` for diagnosis (only for the full-sweep regression
gate in § 6).

---

## 1. reddit — `https://www.reddit.com/`  (deepest dive)

**Bar:** body > 100 000 bytes (Camoufox got 1 145 961).

**Today's number:** BO `L3-RENDERED` 8 326 bytes in 316 ms (chrome_148_macos cold).
Source: `/tmp/full_sweep_2026_05_24/bo_chrome_148_macos_cold.json` — entry for
`"name": "reddit"`.

### 1.1 Reproduce

```bash
cat > /tmp/just_reddit.json <<'JSON'
[{"cat":"social","name":"reddit","url":"https://www.reddit.com/"}]
JSON

BROWSER_OXIDE_DEBUG_NAV=1 \
RUST_LOG=js_runtime=trace,browser=debug \
  /home/yfedoseev/projects/browser_oxide/target/release/examples/sweep_metrics \
    chrome_148_macos /tmp/just_reddit.json /tmp/reddit_out.json \
    2>&1 | tee /tmp/reddit.log

grep -E "(navigate\] iter=|pending|reddit|requestSubmit|forms|solution|JS LOG|JS ERROR|JS WARN)" /tmp/reddit.log
```

Expected today: `[navigate] iter=0` line only, no `iter=1`, body length 8 326 in
the output JSON. After a successful fix you should see `iter=1` with a body of
hundreds of KB.

To capture the body bytes BO actually receives (no V8, just our HTTP/TLS stack):

```bash
curl -sL https://www.reddit.com/ -A "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36" -o /tmp/reddit_curl.html
wc -c /tmp/reddit_curl.html
```

### 1.2 What the 8 326 bytes IS

A verification interstitial. Title: `Reddit - Please wait for verification`. Two
critical elements (captured verbatim from `/tmp/reddit_curl.html`):

```html
<form hidden method="GET" action="/">
  <input type="hidden" name="solution" />
  <input type="hidden" name="js_challenge" value="1"/>
  <input type="hidden" name="token" value="bbbe4bf1...ed48b2"/>
  <input type="hidden" name="jsc_orig_r" value=""/>
</form>
<script>document.addEventListener("DOMContentLoaded", async function(){
    var e = document.forms[0],
        n = (e.onsubmit = function(t) { /* copy URL params to inputs */ },
             await (async e => e + e)("80bfd25d73acfab1"));
    e.elements.namedItem("solution").value = n;
    e.requestSubmit();
}, { once: true });</script>
```

Decoded:

1. Wait for `DOMContentLoaded`.
2. `e = document.forms[0]` (the hidden form).
3. Install `e.onsubmit` that copies URL search params into hidden inputs (no-op
   on a fresh nav to `/` — there are no query params).
4. Compute `n = await (async e => e + e)("80bfd25d73acfab1")` — trivial PoE:
   prove the engine can do `async/await` + string concat. Result:
   `"80bfd25d73acfab180bfd25d73acfab1"`.
5. Set `solution` input to that value.
6. Call `e.requestSubmit()` — submits a **GET** form (`action='/'`,
   `method='GET'`) to `https://www.reddit.com/?solution=80bfd...&js_challenge=1&token=...&jsc_orig_r=`.
7. Server validates the `solution` query param, sets a verification cookie,
   returns the real homepage on the redirect.

**Critical correction to prior notes:** the form's `method` is **GET**, not
POST. `HTMLFormElement.submit()` (`dom_bootstrap.js:1090-1093`) takes the GET
branch and the pending nav is a GET URL with query string. Any debugging that
assumes "we should be sending a POST" will lead you astray.

### 1.3 Hypothesis tree (most-likely first)

#### H1 — `requestSubmit()` fires but `__pendingNavigation` is missed by the outer loop

**Probability:** ~40%.

The chain: `build_page` returns → `run_until_idle(8s)` (`page.rs:3400`) →
`setTimeout(0)` DOMContentLoaded fires (`page.rs:3336-3346`) →
addEventListener handler runs → async/await microtask → `e.requestSubmit()` →
`this.submit()` (`dom_bootstrap.js:1108→1067-1107`, GET branch L1090-1093) →
`globalThis.__pendingNavigation` set (L1098-1103, mirrored to
`_browser_oxide.__pendingNavigation` via `window_bootstrap.js:1334-1339`) →
`ops.op_set_pending_nav()` (L1106) wakes the Rust loop.

After drain, the outer loop reads `PENDING_NAV_JS` at `page.rs:1956`. If the
async chain completes after the 8 s ceiling (unlikely — it's one microtask),
the loop sees empty `pending_info` and returns the page.

**Test:** add a temporary probe right before `page.rs:1956`:

```rust
if debug_nav {
    let dump = page.event_loop().execute_script(
        r#"JSON.stringify({
            forms: document.forms.length,
            solution: document.forms[0] && document.forms[0].elements && document.forms[0].elements.namedItem ?
                      (document.forms[0].elements.namedItem('solution') ? 'OK' : 'NULL') : 'NO_ELEMENTS',
            pending: globalThis.__pendingNavigation,
            errors: (globalThis.__scriptErrors || []).slice(-5),
        })"#).unwrap_or_default();
    eprintln!("[reddit-probe] {}", dump);
}
```

**If `pending` is non-null but `iter=1` doesn't follow** → loop has a race;
move the `PENDING_NAV_JS` read earlier or re-poll for ≤500 ms.
**If `pending` is null** → submit() never fired; proceed to H2/H3/H4.

**Fix:** extend the drain at `page.rs:3400` from 8 s → 12 s for sites with
async DCL work, OR re-poll `PENDING_NAV_JS` for ≤500 ms after the drain.

#### H2 — `e.elements.namedItem('solution')` throws because `form.elements` is undefined

**Probability:** ~25% — the most likely *single-line* root cause.

`HTMLAllCollection.namedItem` exists at `dom_bootstrap.js:1288-1292`, but the
reddit script calls `e.elements.namedItem('solution')` where `e.elements` is
supposed to be `HTMLFormControlsCollection`. Confirm with:

```bash
grep -n "HTMLFormControlsCollection\|form.elements\|prototype.*elements" \
  /home/yfedoseev/projects/browser_oxide/crates/js_runtime/src/js/dom_bootstrap.js
```

If no `form.elements` getter exists on `HTMLFormElement.prototype`, `e.elements`
is `undefined` and `undefined.namedItem(...)` throws `TypeError`, caught by our
top-level trap (`page.rs:3406`) → recorded in `__scriptErrors`. The async chain
dies silently; `__pendingNavigation` never gets set.

**Test:** grep `RUST_LOG=info` output for `Script errors`; or probe before L1956:

```rust
let probe = page.event_loop().execute_script(r#"
    (function(){
        var f = document.forms[0];
        if (!f) return 'NO_FORM';
        if (!f.elements) return 'NO_ELEMENTS';
        if (!f.elements.namedItem) return 'NO_NAMEDITEM';
        var s = f.elements.namedItem('solution');
        return s ? 'OK:' + (s.value || 'EMPTY') : 'NULL';
    })()"#).unwrap_or_default();
eprintln!("[reddit-probe-H2] {}", probe);
```

**Expected if H2 confirmed:** `NO_ELEMENTS` or `NO_NAMEDITEM`.

**Fix:** add an `HTMLFormElement.prototype.elements` getter to `dom_bootstrap.js`
after L1110. Return `this.querySelectorAll('input,select,textarea,button,fieldset')`
with a `namedItem(name)` method patched onto the NodeList that iterates and
matches `el.name === name || el.id === name`. Real Chrome returns a live
HTMLFormControlsCollection; the snapshot is adequate for reddit's one-shot use.

#### H3 — `async/await` in a `DOMContentLoaded` handler doesn't resolve

**Probability:** ~15%.

V8 supports async/await natively; risk is that our event-loop pump doesn't tick
microtasks between the sync register call and the handler's internal `await`.

**Test for H3:** write a one-off test at `crates/browser/tests/`:

```rust
#[tokio::test(flavor = "current_thread")]
async fn reddit_async_dcl_chain() {
    let html = r#"<!DOCTYPE html><html><body><form id="f"><input id="sol" name="solution"/></form>
        <script>document.addEventListener("DOMContentLoaded", async function() {
            const n = await (async e => e + e)("abc");
            document.getElementById("sol").value = n;
            document.body.appendChild(document.createTextNode("OK:" + n));
        }, { once: true });</script></body></html>"#;
    let profile = stealth::presets::chrome_148_macos();
    let client = net::HttpClient::new().unwrap();
    let page = Page::build_page_with_scripts(html, "https://example.com/", &profile, &client).await.unwrap();
    assert!(page.content().contains("OK:abcabc"));
}
```

Run: `cargo test --release -p browser reddit_async_dcl_chain -- --test-threads=1 --nocapture`.

**Fix if confirmed:** ensure `run_until_idle` ticks microtasks after each
macrotask — see `crates/event_loop/src/lib.rs:285`.

#### H4 — `DOMContentLoaded` doesn't fire within the 8 s drain

**Probability:** ~10%. Lowest because we fire it explicitly via
`setTimeout(0)` at `page.rs:3336-3346` with an 8 s ceiling at L3400.

**Test:** add `window.__dcl_fired = false;` + `addEventListener` setter in the
fired script; read after drain. If false → bump the drain ceiling.

### 1.4 Validation

After applying any fix, the acceptance command is:

```bash
target/release/examples/sweep_metrics chrome_148_macos /tmp/just_reddit.json /tmp/reddit_out.json
jq '.results[] | select(.name=="reddit") | {tag, len, ms}' /tmp/reddit_out.json
```

Bar: `tag == "L3-RENDERED"` AND `len > 100000`. (Camoufox achieved 1 145 961.)

Run the full 4-profile bench (`benchmarks/run_full_sweep.sh`) once to confirm no
regression on the 110+ sites that already pass. The site count must not drop on
any profile.

### 1.5 Risk

Pure JS-side fix → zero risk to the network/TLS stack. The change to
`dom_bootstrap.js` is in the universal JS bootstrap; run the test suite
single-threaded (`cargo test --workspace -- --test-threads=1`) and verify no
regression in `chrome_compat` tests.

---

## 2. duolingo — `https://www.duolingo.com/`

**Bar:** body > 50 000 bytes (Camoufox got 696 885).
**Today's number:** BO `L3-RENDERED` 13 327 bytes in 15 171 ms. **2 033 bytes
shy of the 15 KB gate** — this is the closest miss in the entire 126-site
corpus.

### 2.1 Reproduce

```bash
cat > /tmp/just_duolingo.json <<'JSON'
[{"cat":"misc","name":"duolingo","url":"https://www.duolingo.com/"}]
JSON

BROWSER_OXIDE_DEBUG_NAV=1 \
RUST_LOG=js_runtime::extensions::worker_ext=trace,js_runtime=debug,browser=info \
  target/release/examples/sweep_metrics chrome_148_macos /tmp/just_duolingo.json /tmp/duo_out.json \
  2>&1 | tee /tmp/duo.log

grep -E "(worker|Worker|grecaptcha|recaptcha|MessageChannel|MessagePort|postMessage|JS LOG|JS ERROR|JS WARN)" /tmp/duo.log
```

### 2.2 What we know

duolingo's SPA shell loads three third-party scripts in sequence:

1. `https://www.recaptcha.net/recaptcha/enterprise.js?render=6LcLOdsjAAAAAFfwGusLLnnn492SOGhsCh-uEAvI`
2. `https://www.gstatic.com/recaptcha/releases/Br0hYqpfWeFzYCAXLD4UuCIV/recaptcha__en.js`
3. `https://www.recaptcha.net/recaptcha/enterprise/webworker.js` (loaded into a Worker)

The duolingo SPA gates its first `/api/...` hydration call on
`grecaptcha.execute(siteKey).then(token => fetch('/api/...', {headers:{'X-Recaptcha-Token': token}}))`.
If `grecaptcha.execute()` never resolves, no hydration → page sits at 13 KB.

Camoufox achieves 696 KB by getting through this gate; ours doesn't.

### 2.3 Hypothesis tree

#### H1 — `MessageChannel`/`MessagePort` is a no-op stub that recaptcha relies on

**Probability:** ~50%. This is the strongest single hypothesis.

`crates/js_runtime/src/js/window_bootstrap.js:2256-2272`:

```js
if (!globalThis.MessagePort) {
    globalThis.MessagePort = class MessagePort extends EventTarget {
        constructor() { super(); this.onmessage = null; this.onmessageerror = null; }
        postMessage() {}      // <-- NO-OP
        start() {}
        close() {}
    };
}
if (!globalThis.MessageChannel) {
    globalThis.MessageChannel = class MessageChannel {
        constructor() {
            this.port1 = new MessagePort();
            this.port2 = new MessagePort();
        }
    };
}
```

`port1.postMessage(msg)` does **not** deliver `msg` to `port2.onmessage`. Real
Chrome routes the message through structured-clone serialisation to the paired
port. recaptcha enterprise.js uses MessageChannel for in-process worker handoff
(separate from `new Worker(webworker.js)`).

If recaptcha posts a "compute challenge" message to `port1` and then awaits
`port2.onmessage`, the response never arrives → `grecaptcha.execute()` Promise
never resolves → no hydration.

**Test for H1:**

```bash
grep -A20 "recaptcha enterprise.js" /tmp/duo.log | head -40
# Look for postMessage calls. If our [JS LOG] shows postMessage with no matching onmessage,
# H1 is likely.
```

Or instrument the stub:

```js
// TEMPORARY at window_bootstrap.js:2257 — add to MessagePort:
postMessage(msg) {
    console.log('[MP-stub] postMessage called, peer=', this._peer ? 'PRESENT' : 'NONE');
}
```

**Fix sketch:** wire port1 ↔ port2 with paired-port semantics. Each
`MessagePort` keeps a `_peer` reference; `postMessage(data)` enqueues
`{type:'message', data}` to the peer's microtask via `Promise.resolve().then`;
`onmessage`/`message` listeners on the peer fire. Buffer messages until
`start()` (or implicit start on `onmessage = fn` assignment, per HTML spec).
Wire both in the `MessageChannel` constructor (`port1._peer = port2; port2._peer = port1`).
Reuse the existing `MessageEvent` from `dom_bootstrap.js:2750` for dispatch.

#### H2 — `Worker(webworker.js)` fails to load or run (~20%)

Worker spawn at `worker_ext.rs:214-358` (64 MB-stack OS thread + child JsRuntime).
With `RUST_LOG=js_runtime::extensions::worker_ext=trace` (already set in § 2.1),
grep `/tmp/duo.log` for `worker_id=` to count distinct IDs and find these errors:
`worker module load error`, `worker module eval error`, `worker script error`,
`worker event loop error`.

#### H3 — Missing Worker-context global (~15%)

Candidates: `OffscreenCanvas` (image-rec challenges), `crypto.subtle` in worker,
`Worker.prototype.name` getter (we set `this._name` at `window_bootstrap.js:1883`
but never expose it as `get name()`), `importScripts()` return value
(`worker_bootstrap.js:232-256`).

**Capture recaptcha for inspection:**

```bash
mkdir -p /tmp/recaptcha
curl -sL "https://www.recaptcha.net/recaptcha/enterprise.js?render=6LcLOdsjAAAAAFfwGusLLnnn492SOGhsCh-uEAvI" -o /tmp/recaptcha/enterprise.js
curl -sL "https://www.recaptcha.net/recaptcha/enterprise/webworker.js" -o /tmp/recaptcha/webworker.js
js-beautify /tmp/recaptcha/*.js -r  # npm install -g js-beautify
```

#### H4 — Non-Worker headless sentinel (~15%)

recaptcha probes `navigator.webdriver`, plugin count, fonts list, `chrome.runtime`.
Our stealth profiles cover these but a single leak slips through. Patch the
deobfuscated enterprise.js to dump its fingerprint object pre-network-call, diff
against Camoufox capture.

### 2.4 Validation

```bash
target/release/examples/sweep_metrics chrome_148_macos /tmp/just_duolingo.json /tmp/duo_out.json
jq '.results[] | select(.name=="duolingo") | {tag, len, ms}' /tmp/duo_out.json
```

Bar: `tag == "L3-RENDERED"` AND `len > 50000`. (Camoufox achieved 696 885.)

Because duolingo is **2 033 bytes from the gate**, even a partial fix (e.g. one
extra hydration fetch landing) may flip it. Keep iterating until the bar is
crossed, then check that the gate holds across all 4 profiles.

### 2.5 Why prioritise H1 (MessageChannel)

If the H1 fix lands, it likely also benefits booking (heavy SPA), x.com (heavy
SPA), and several Akamai-instrumented React sites that use MessageChannel for
their telemetry IPC. It's the highest-leverage single change in this chapter.

---

## 3. booking — `https://www.booking.com/`

**Bar:** body > 30 000 bytes (Camoufox got 37 915).
**Today's number:** BO `L3-RENDERED` 8 473 bytes in 15 100 ms (chrome).
iPhone profile shows 3 891 bytes (a *different* shell — booking serves
device-specific SSR). No `iter=1` on any profile.

### 3.1 Reproduce

```bash
cat > /tmp/just_booking.json <<'JSON'
[{"cat":"travel","name":"booking","url":"https://www.booking.com/"}]
JSON

BROWSER_OXIDE_DEBUG_NAV=1 RUST_LOG=js_runtime=debug,browser=info \
  target/release/examples/sweep_metrics chrome_148_macos /tmp/just_booking.json /tmp/booking_out.json \
  2>&1 | tee /tmp/booking.log

grep -E "(iter=|fetch|XHR|JS ERROR|JS WARN|pending)" /tmp/booking.log
```

### 3.2 Hypothesis tree

booking is a heavy server-side React app. The 8 KB response is an SPA bootstrap
with inlined `__INITIAL_STATE__` and a loader that issues XHR/fetch to populate
the page.

#### H1 — Missing prototype method on the bootstrap object (~35%)

Common breakage points: `IntersectionObserver` (the SPA's "start hydration when
hero is visible" gate), `Object.fromEntries`, `String.prototype.replaceAll`,
`requestIdleCallback`, `queueMicrotask`, `CSS.supports`.

```bash
grep -n "class IntersectionObserver\|requestIdleCallback\|CSS\.supports" \
  /home/yfedoseev/projects/browser_oxide/crates/js_runtime/src/js/*.js | head -20
```

If `IntersectionObserver.observe()` doesn't trigger a fake-entry callback, the
hydration trigger never fires.

#### H2 — Missing fetch / response header (~25%)

booking uses `*.bstatic.com` / `r.booking.com` for hydration data. The SPA may
parse a response header we don't expose (`Server-Timing`, `Set-Cookie` echoed
in JSON). **Capture-diff** is the fastest path: BO↔Camoufox `fetches.json`,
sort by URL, `diff`. First URL unique to Camoufox = failure point.

#### H3 — Time-to-hydration > 15 s drain (~20%)

booking's SPA is slow even on real Chrome (3.5 s in Camoufox). Our 15 100 ms
total = full nav budget exhausted without seeing `__pendingNavigation`. Add
per-tick body-length probes in the drain at `page.rs:3400`:

```rust
for tick in 0..16 {
    let _ = event_loop.run_until_idle(Duration::from_millis(500)).await;
    let len = event_loop.execute_script("document.body ? document.body.outerHTML.length : 0").unwrap_or_default();
    eprintln!("[booking-drain] tick={} body_len={}", tick, len);
}
```

If `body_len` grows past tick 16 → extend drain. Flat → blocked on H1/H2.

#### H4 — Missing `window` global (~15%)

React SPAs probe `window.requestIdleCallback`, `window.queueMicrotask`,
`window.CSS.supports('foo:bar')`. See H1 grep.

### 3.3 Validation

```bash
target/release/examples/sweep_metrics chrome_148_macos /tmp/just_booking.json /tmp/booking_out.json
jq '.results[] | select(.name=="booking") | {tag, len, ms}' /tmp/booking_out.json
```

Bar: `tag == "L3-RENDERED"` AND `len > 30000`. Test ALL 4 profiles — booking
serves different SSR per User-Agent, so a chrome-profile fix may not transfer
to iPhone.

---

## 4. douyin — `https://www.douyin.com/`  (TikTok-CN)

**Bar:** body > 100 000 bytes (Camoufox got 1 028 344).
**Today's number:** BO `L3-RENDERED` 6 327 bytes in 3 227 ms (uniform across
profiles). The short time + tiny uniform body suggests this site terminates
fast on a sentinel.

### 4.1 Reproduce

```bash
cat > /tmp/just_douyin.json <<'JSON'
[{"cat":"chl-known","name":"douyin","url":"https://www.douyin.com/"}]
JSON

BROWSER_OXIDE_DEBUG_NAV=1 RUST_LOG=js_runtime=debug,browser=info \
  target/release/examples/sweep_metrics chrome_148_macos /tmp/just_douyin.json /tmp/douyin_out.json \
  2>&1 | tee /tmp/douyin.log

# Look for ttwid / __ac_signature / mssdk markers
grep -E "(ttwid|__ac_signature|mssdk|verifyfp|JS ERROR|pending|iter=)" /tmp/douyin.log
```

Also dump the body to disk:

```bash
curl -sL https://www.douyin.com/ -A "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36" -o /tmp/douyin_curl.html
wc -c /tmp/douyin_curl.html
# Should be close to 6 327 bytes (the size BO sees)
grep -oE "__ac_signature|ttwid|mssdk|verifyfp" /tmp/douyin_curl.html | sort -u
```

### 4.2 Hypothesis tree

douyin is the China-domestic counterpart of TikTok. The 6 327-byte body is
likely a signature-computation gate: inline `<script>` computes `__ac_signature`
from a fingerprint (UA, timestamp, salt), sets it as cookie or POST body, and
re-fetches.

#### H1 — Missing `crypto.subtle` primitive (~35%)

`__ac_signature` is commonly HMAC-SHA256 of `(ua + path + ts + iv)`. Confirm:

```bash
grep -rn "crypto.subtle\|importKey\|sign\b" /home/yfedoseev/projects/browser_oxide/crates/js_runtime/src/ | head -10
```

If missing, the script throws → caught by `page.rs:3406-3412` → nothing sets
`__pendingNavigation` → 6 KB returned.

#### H2 — Missing ByteDance SDK shim (~25%)

ByteDance properties (TikTok / douyin / Toutiao) ship `byted_acrawler` /
`acrwt`. Their bootstrap checks for SDK presence; absent → "no acrawler" branch.

```bash
grep -oE "byted[A-Za-z_]*|acrawler|tt[a-z_]*[A-Z][A-Za-z]*" /tmp/douyin_curl.html | sort -u
```

#### H3 — Missing fingerprint primitive (~20%)

douyin checks `navigator.hardwareConcurrency`, `deviceMemory`, `performance.memory.jsHeapSizeLimit`,
`performance.measureUserAgentSpecificMemory`. A single missing one triggers
a "headless / emulator" branch. Cross-reference inline script field-by-field
against stealth profile output.

#### H4 — TLS/HTTP/2 fingerprint mismatch (~10%)

We use BoringSSL with Chrome 148 ClientHello (`crates/net/src/tls.rs`) so this
should match. If it doesn't, the 6 327 response itself is a block page.

```bash
RUST_LOG=net::tls=trace target/release/examples/sweep_metrics chrome_148_macos \
  /tmp/just_douyin.json /tmp/x.json 2>&1 | grep -E "ClientHello|cipher|ALPN" | head -10
```

### 4.3 Defer recommendation

douyin / Weibo / Toutiao / Tmall use China-specific anti-bot stacks that don't
appear elsewhere in the corpus. A fix is unlikely to transfer. For v0.1.0-parity,
douyin is **optional** if the other 3 sites flip — see § 6.

### 4.4 Validation

```bash
target/release/examples/sweep_metrics chrome_148_macos /tmp/just_douyin.json /tmp/douyin_out.json
jq '.results[] | select(.name=="douyin") | {tag, len, ms}' /tmp/douyin_out.json
```

Bar: `tag == "L3-RENDERED"` AND `len > 100000`. (Camoufox achieved 1 028 344.)

---

## 5. Generic SPA-hydration improvements

The current SPA-fast-exit logic at `crates/browser/src/page.rs:1925-1949`
exits the navigate loop early if ANY common SPA mount has ≥1 child AND we're
not on a recognised anti-bot challenge page. The selector list is
`['#react-root','#__next','#app','#root','[data-reactroot]','#main-app','#mount-point']`
and the threshold is `children.length > 0`.

### 5.1 Why this is suboptimal for booking

booking's shell ships `<div id="__next">` containing a `<noscript>` fallback +
a 1-element loading skeleton. `mount_populated = 1` returns true, loop exits,
body stays at 8 KB. The mount has children — they're shell content, not
hydrated content.

### 5.2 Proposed improvement

Replace the single-condition early-exit with a multi-signal heuristic:

```rust
// Pseudo-code (current location: page.rs:1925-1949)
let mount_children = ...; // existing query
let body_len = page.content().len();
let ready_state = page.event_loop().execute_script("document.readyState").unwrap_or_default();

let likely_hydrated =
    mount_children >= 5                                 // shell rarely has ≥5 children
    || (mount_children >= 1 && body_len > 50 * 1024)    // big body + populated mount
    || (ready_state == "complete" && body_len > 30 * 1024 && !growing_recently);

if likely_hydrated { return Ok(page); }
```

`growing_recently` = sample `body_len` every 200 ms; flat-for-1 s = plateaued.

### 5.3 Risk and validation

Touches the navigate-loop exit condition for every site. Run the full 4-profile
bench before/after. Expected: no regression on the 110+ passing sites (the
change is more conservative for sites with sparse mounts); booking / wildberries
/ a few other "stuck in mount-1 trap" sites should grow past the gate.

Any single site dropping > 1 KB in `len` → heuristic is too aggressive, narrow
the conditions.

---

## 6. Acceptance checklist (per site)

After your fix, run:

```bash
cd /home/yfedoseev/projects/browser_oxide
cargo build --release -p browser --example sweep_metrics
# Single-site validations
target/release/examples/sweep_metrics chrome_148_macos /tmp/just_reddit.json   /tmp/r.json && \
  jq '.results[] | select(.name=="reddit")   | .len > 100000' /tmp/r.json
target/release/examples/sweep_metrics chrome_148_macos /tmp/just_duolingo.json /tmp/d.json && \
  jq '.results[] | select(.name=="duolingo") | .len > 50000'  /tmp/d.json
target/release/examples/sweep_metrics chrome_148_macos /tmp/just_booking.json  /tmp/b.json && \
  jq '.results[] | select(.name=="booking")  | .len > 30000'  /tmp/b.json
target/release/examples/sweep_metrics chrome_148_macos /tmp/just_douyin.json   /tmp/y.json && \
  jq '.results[] | select(.name=="douyin")   | .len > 100000' /tmp/y.json
```

Each `jq` must print `true`.

### Per-site bars

- [ ] **reddit** — `tag == "L3-RENDERED"` AND `len > 100 000` bytes
- [ ] **duolingo** — `tag == "L3-RENDERED"` AND `len > 50 000` bytes
- [ ] **booking** — `tag == "L3-RENDERED"` AND `len > 30 000` bytes (test all 4 profiles, not just chrome)
- [ ] **douyin** — `tag == "L3-RENDERED"` AND `len > 100 000` bytes

### Headline-bench impact

If 3 of 4 flip (douyin most likely to stay), we add **+3 sites** to the routed
count: 113 (Camoufox) → 113 (BO) becomes 110+3 = **113** on at least one
single profile, or 111+3 = **114** on routed. Combined with chapter 06
(AWS WAF, +4 sites possible) and chapter 07 (DataDome, +1-3 sites), we cross
the **≥ 115** routed bar from `00_README.md`.

### Regression gate

After any fix, run the full 126-site sweep on ALL 4 profiles and confirm no
existing pass becomes a fail. If you don't have `/tmp/corpus.json`:

```bash
python3 /home/yfedoseev/projects/browser_oxide/benchmarks/bench_corpus_v2.py --emit-corpus-only > /tmp/corpus.json
# Then:
/home/yfedoseev/projects/browser_oxide/benchmarks/run_full_sweep.sh
```

The script writes per-profile JSONs under `/tmp/full_sweep_$(date +%Y_%m_%d)/`.
Diff against the 2026-05-24 baseline (`/tmp/full_sweep_2026_05_24/`):

```bash
for f in bo_chrome_148_macos_cold bo_pixel_9_pro_chrome_148_cold \
         bo_iphone_15_pro_safari_18_cold bo_firefox_135_macos_cold; do
    diff <(jq -r '.results[] | "\(.name)\t\(.tag)\t\(.len)"' /tmp/full_sweep_2026_05_24/$f.json | sort) \
         <(jq -r '.results[] | "\(.name)\t\(.tag)\t\(.len)"' /tmp/full_sweep_$(date +%Y_%m_%d)/$f.json | sort) \
         | head -50
done
```

A clean diff = only the sites you targeted appear, and only with improvements.
Any reduction in `len` for a non-targeted site indicates a regression.

---

## 7. Files referenced

| File:line | What it is |
|---|---|
| `crates/browser/src/classify.rs:47, 177-181` | 15 KB bar, L3-RENDERED → Pass/ThinShell |
| `crates/browser/src/page.rs:1545` | `navigate_loop_internal` |
| `crates/browser/src/page.rs:1624-1629` | `PENDING_NAV_JS` definition |
| `crates/browser/src/page.rs:1925-1949` | SPA-fast-exit heuristic (§ 5) |
| `crates/browser/src/page.rs:1956, 1997` | `PENDING_NAV_JS` reads (loop + poll) |
| `crates/browser/src/page.rs:2814` | `build_page_with_scripts_init_and_storage` |
| `crates/browser/src/page.rs:3336-3346, 3400, 3406-3412` | DOMContentLoaded fire, 8 s drain, error capture |
| `crates/js_runtime/src/js/dom_bootstrap.js:1067-1110` | `HTMLFormElement.submit/requestSubmit` |
| `crates/js_runtime/src/js/dom_bootstrap.js:1282-1303` | `HTMLAllCollection.namedItem` |
| `crates/js_runtime/src/js/window_bootstrap.js:1334-1339` | `__pendingNavigation` mirror |
| `crates/js_runtime/src/js/window_bootstrap.js:1879-1975` | `Worker` class |
| `crates/js_runtime/src/js/window_bootstrap.js:2256-2272` | `MessagePort`/`MessageChannel` (NO-OP — duolingo H1) |
| `crates/js_runtime/src/js/worker_bootstrap.js` | Worker-side `self`, postMessage, importScripts |
| `crates/js_runtime/src/extensions/worker_ext.rs:214-358` | `op_worker_spawn` |
| `crates/js_runtime/src/extensions/nav_ext.rs:44` | `op_set_pending_nav` |
| `crates/event_loop/src/lib.rs:285` | `run_until_idle` |
| `crates/browser/tests/holistic_sweep.rs:303, 470, 668, 685` | reddit / booking / douyin / duolingo corpus |
| `crates/browser/examples/sweep_metrics.rs` | Sweep runner |
| `benchmarks/bench_corpus_v2.py`, `benchmarks/run_full_sweep.sh` | Corpus builder + full sweep |
| `/tmp/full_sweep_2026_05_24/{bo_chrome_148_macos_cold,comp_camoufox}.json` | Baselines |

---

## 8. Estimated effort

| Site | Best-case fix | Worst-case fix | Risk |
|---|---|---|---|
| reddit | 1 line (add `HTMLFormElement.elements`) | 1 day (drain + microtask ordering) | LOW |
| duolingo | 50 lines (paired MessageChannel) | 3 days (Worker IPC + recaptcha) | MED |
| booking | 200 lines (mount-heuristic) | 1 week (capture-diff per missing API) | MED |
| douyin | 1 week (China-specific research) | open-ended | HIGH |

Realistic budget: **1-2 weeks** to flip 3/4. douyin can be deferred without
blocking v0.1.0.
