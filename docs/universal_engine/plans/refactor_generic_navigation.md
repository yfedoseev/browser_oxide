# Refactor: generic navigation, zero per-engine logic

**Tasks**: #73, #74, #75, #76
**Priority**: P0 (blocks everything else)
**Effort**: 4-8 hours
**Dependencies**: none — this is the starting point

## Goal

Remove every per-engine branch from the runtime. Replace
`navigate_with_challenges` with a generic `navigate(url, max_iterations)`
that relies on standard navigation primitives (`location.reload()`,
`location.href = ...`, `<meta http-equiv="refresh">`) to loop when a
challenge script needs to re-navigate.

## Why this matters

Per `docs/universal_engine/01_architecture_principle.md` and the
research in `03_research_landscape.md`, every working open-source stealth
browser is generic at the runtime layer. Per-engine code only exists in
commercial remote solvers. Our current per-engine code is an
architectural smell and blocks the structural-advantage thesis we're
pursuing.

## Step 1 — Implement navigation primitives (1-2h)

**File**: `crates/js_runtime/src/js/window_bootstrap.js`

Currently `location.reload()` is a no-op, `location.href = ...` parses
the URL but doesn't signal anything, and `<meta http-equiv="refresh">`
is ignored. Real browsers navigate on all three.

### 1a — location Proxy updates

Find the existing `globalThis.location = new Proxy(_locationData, ...)`
block. Modify it:

```js
// Track pending navigations set by the scripts.
globalThis.__pendingNavigation = null; // {url, kind: 'reload'|'assign'|'replace'}

globalThis.location = new Proxy(_locationData, {
    get(target, prop) {
        if (prop === "assign") {
            return (url) => {
                _parseLocationUrl(url);
                globalThis.__pendingNavigation = {
                    url: _locationData.href,
                    kind: "assign",
                };
            };
        }
        if (prop === "replace") {
            return (url) => {
                _parseLocationUrl(url);
                globalThis.__pendingNavigation = {
                    url: _locationData.href,
                    kind: "replace",
                };
            };
        }
        if (prop === "reload") {
            return () => {
                globalThis.__pendingNavigation = {
                    url: _locationData.href,
                    kind: "reload",
                };
            };
        }
        if (prop === "toString") return () => target.href;
        return target[prop];
    },
    set(target, prop, value) {
        if (prop === "href") {
            _parseLocationUrl(String(value));
            globalThis.__pendingNavigation = {
                url: _locationData.href,
                kind: "assign",
            };
            return true;
        }
        target[prop] = value;
        return true;
    },
});
```

### 1b — meta-refresh parsing

**File**: `crates/browser/src/page.rs::build_page_with_scripts`

After the HTML is parsed but before the event loop drain, scan the DOM
for `<meta http-equiv="refresh">` tags:

```rust
// Scan for <meta http-equiv="refresh" content="N;url=...">
// and schedule a setTimeout that sets __pendingNavigation.
let meta_refresh_script = format!(r#"
    (function() {{
        const metas = document.getElementsByTagName('meta');
        for (let i = 0; i < metas.length; i++) {{
            const m = metas[i];
            const equiv = (m.getAttribute('http-equiv') || '').toLowerCase();
            if (equiv !== 'refresh') continue;
            const content = String(m.getAttribute('content') || '');
            const match = content.match(/^(\d+)(?:\s*;\s*url=(.+))?$/i);
            if (!match) continue;
            const delay = parseInt(match[1], 10) || 0;
            const target = (match[2] || '').trim() || location.href;
            setTimeout(() => {{
                globalThis.__pendingNavigation = {{
                    url: target,
                    kind: 'assign',
                }};
            }}, delay * 1000);
        }}
    }})();
"#);
event_loop.execute_script(&meta_refresh_script).ok();
```

### 1c — Test

Add `crates/browser/tests/navigation_primitives.rs`:

```rust
#[tokio::test]
async fn location_reload_sets_pending_navigation() {
    // ... build a runtime, execute `location.reload()`,
    //     check globalThis.__pendingNavigation.url === current URL
    //     and .kind === 'reload'
}

#[tokio::test]
async fn location_href_assignment_sets_pending_navigation() {
    // ... execute `location.href = 'https://example.com/other'`,
    //     check __pendingNavigation.url === 'https://example.com/other'
    //     and .kind === 'assign'
}

#[tokio::test]
async fn location_replace_sets_pending_navigation() {
    // ... similar
}

#[tokio::test]
async fn meta_refresh_sets_pending_navigation() {
    // ... build a page with
    //       '<meta http-equiv="refresh" content="0;url=https://target/" />'
    //     drain the event loop briefly
    //     check __pendingNavigation.url === 'https://target/'
}
```

All four tests must pass before moving to step 2.

## Step 2 — Frame-level init script registry (1-2h)

**File**: `crates/js_runtime/src/runtime.rs`

Currently the bootstrap scripts run once when a `JsRuntime` is
constructed. When page.rs does a navigation, the old runtime is
dropped and a new one built via `BrowserJsRuntime::with_options`,
which re-runs the bootstraps. That's mostly the correct behavior
already — this step formalizes it.

Add a `BrowserRuntimeOptions::init_scripts` field that callers can
populate. When the runtime is built, run the bootstraps first, THEN
run the init scripts in order, THEN `<script>` tags from the document.
Update page.rs to carry the init scripts across navigations.

```rust
// crates/js_runtime/src/runtime.rs
pub struct BrowserRuntimeOptions {
    pub base_url: Option<url::Url>,
    pub stealth_profile: Option<StealthProfile>,
    pub stylesheets: Vec<String>,
    pub init_scripts: Vec<String>, // NEW
}

pub fn create_runtime(dom: Dom, options: BrowserRuntimeOptions) -> JsRuntime {
    // ... existing setup ...

    // Run built-in bootstraps first.
    runtime.execute_script("<console_bootstrap>", include_str!("js/console_bootstrap.js")).expect("...");
    // ... other bootstraps ...
    runtime.execute_script("<input_bootstrap>", include_str!("js/input_bootstrap.js")).expect("...");

    // Then run any init scripts the caller supplied. These run in order
    // BEFORE any parsed-HTML <script> tags execute. This matches
    // Chromium's Page.addScriptToEvaluateOnNewDocument semantics.
    for (i, code) in options.init_scripts.iter().enumerate() {
        let name = format!("<init_script_{}>", i);
        // Leak the name string for the 'static lifetime deno_core expects.
        let static_name: &'static str = Box::leak(name.into_boxed_str());
        runtime.execute_script(static_name, code.clone()).ok();
    }

    runtime
}
```

**Page-level wrapper**: Add a `Page::init_scripts` field (Vec<String>)
that survives across navigations. When `navigate` creates a new
`BrowserJsRuntime`, it passes the accumulated init scripts in.

This is the critical primitive that makes every Document in the frame
inherit the same fingerprint/capability bundle. Real Chrome's
`Page.addScriptToEvaluateOnNewDocument` CDP command maps to exactly
this.

## Step 3 — Generic `Page::navigate` (1-2h)

**File**: `crates/browser/src/page.rs`

Add a new function (don't delete the old one yet — step 4 does that):

```rust
impl Page {
    pub async fn navigate(
        url: &str,
        profile: stealth::StealthProfile,
        max_iterations: u8,
    ) -> Result<Self, deno_core::error::AnyError> {
        let client = net::HttpClient::new(&profile)
            .map_err(|e| deno_core::error::AnyError::msg(e.to_string()))?;
        js_runtime::extensions::fetch_ext::set_fetch_client(client.clone());

        let mut current_url = url.to_string();
        let mut final_page: Option<Self> = None;

        for iter in 0..max_iterations {
            eprintln!("[navigate] iter={iter} url={current_url}");

            // Use get_follow to handle 302/307 redirects natively.
            let resp = client.get_follow(&current_url, 10).await
                .map_err(|e| deno_core::error::AnyError::msg(e.to_string()))?;
            let html = resp.text();
            let resp_url = resp.url.clone();

            let mut page = Self::build_page_with_scripts(
                &html, &resp_url, &profile, &client,
            ).await?;

            // Drain the event loop so scripts run.
            page.event_loop()
                .run_until_idle(std::time::Duration::from_secs(30))
                .await
                .ok(); // don't fail the whole navigation on one timeout

            // Did a script set __pendingNavigation?
            let pending_url = page
                .event_loop()
                .execute_script(
                    "globalThis.__pendingNavigation ? \
                     String(globalThis.__pendingNavigation.url || '') : ''"
                )
                .unwrap_or_default();

            if pending_url.is_empty() {
                return Ok(page);
            }

            eprintln!("[navigate] pending navigation to {pending_url}");
            // Clear the flag so next iter can detect new navigation.
            page.event_loop()
                .execute_script("globalThis.__pendingNavigation = null;")
                .ok();

            current_url = pending_url;
            final_page = Some(page); // keep in case we hit max_iterations
            // Drop happens at end of loop body — new V8 isolate next iter.
        }

        // Hit the iteration limit — return the last page we built.
        if let Some(page) = final_page {
            return Ok(page);
        }
        // Shouldn't happen, but fall back.
        let resp = client.get(url).await
            .map_err(|e| deno_core::error::AnyError::msg(e.to_string()))?;
        Self::build_page_with_scripts(&resp.text(), url, &profile, &client).await
    }
}
```

**Key differences vs the old `navigate_with_challenges`**:

- `client.get_follow(url, 10)` instead of `client.get(url)` — handles
  ozon's 307 redirect loop generically.
- No `is_challenge_page` check. Every response is handled the same.
- No Kasada token extraction.
- No WBAAS logging.
- No JS-level XHR retry. Scripts set `__pendingNavigation` directly.
- Bounded by `max_iterations` to prevent infinite loops.

## Step 4 — Delete the old per-engine code (1-2h)

**File**: `crates/browser/src/page.rs`

Once step 3's `Page::navigate` works, delete:

1. `fn is_challenge_page(...)` and all its markers.
2. `solver_session_tokens: Vec<(String, String)>` and every place that
   populates or consumes it.
3. The `for (k, v) in &solver_session_tokens` block in the retry path.
4. The `wbaas_status`/`wbaas_hint` extraction.
5. The Kasada `/tl` POST scanner (the
   `if let Ok(tokens_json) = solver_page.event_loop().execute_script(...)`
   block around line 734).
6. The JS-level `globalThis.__retryHtml` / `__retryStatus` / `__retryUrl`
   / `__xhrSendPatched` machinery inside `navigate_with_challenges`.
7. Mark `navigate_with_challenges` as `#[deprecated]` and have it call
   through to `navigate`:
   ```rust
   #[deprecated(note = "use Page::navigate instead")]
   pub async fn navigate_with_challenges(
       url: &str,
       profile: stealth::StealthProfile,
       max_retries: u8,
   ) -> Result<Self, deno_core::error::AnyError> {
       Self::navigate(url, profile, max_retries).await
   }
   ```

After this step, `grep -i "kasada\|kpsdk\|wbaas\|akamai\|abck\|\
ips\.js\|sec-if-cpt\|cf-ray\|datadome\|perimeterx" crates/browser/src/` should return **zero results** (except in comments that reference
them as historical context).

## Step 5 — Decide on the humanize script (30 min, task #76)

**File**: `crates/browser/src/page.rs::build_page_with_scripts`

The humanize script (~line 1025-1120) fires synthetic mouse/keyboard
events on every navigation. It's generic but semantically is "anti-bot
workaround". Three options:

### Option A — Delete it entirely

Remove the humanize block. Risk: Akamai sites flag "zero mouse events"
as a weak bot signal. Observed: this doesn't currently flip any site
from passing to failing (the adidas verdict is the same either way).

### Option B — Make it opt-in (recommended)

Add a new API:

```rust
impl Page {
    pub async fn navigate_humanized(
        url: &str,
        profile: stealth::StealthProfile,
        max_iterations: u8,
    ) -> Result<Self, deno_core::error::AnyError> {
        // same as navigate, but also pass an init script that
        // installs the humanize machinery
        let humanize_script = include_str!("js/humanize.js").to_string();
        Self::navigate_with_init(url, profile, max_iterations, vec![humanize_script]).await
    }
}
```

Move the humanize JS into a dedicated file `crates/browser/src/js/
humanize.js`. Tests that currently rely on humanize behavior call
`navigate_humanized`; default `navigate` is clean.

### Option C — Keep as-is in default path

Pragmatic. Violates the architecture principle slightly. Not
recommended.

**Recommendation**: Option B. The research found BotBrowser and
Camoufox don't auto-generate input events — they rely on fingerprint
accuracy. Matching that is architecturally cleanest. But keep the
humanize code available via an explicit API in case a specific test
needs it.

## Step 6 — Regression gate

After all of the above, run:

```bash
cd /home/yfedoseev/projects/browser_oxide

# 1. Workspace must be green.
cargo test --workspace -- --test-threads=1

# 2. The 22 deep-path passing sites must still HOLD.
cargo test -p browser --test deep_path_validation -- \
    --ignored --test-threads=1 --nocapture 2>&1 \
    | grep -E "HOLD|DEGRADE|both-fail"

# 3. The blockers must be same or better.
cargo test -p browser --test blocker_rigorous_probe tier05_blockers_all \
    -- --ignored --test-threads=1 --nocapture 2>&1 | tail -20
```

**Acceptance criteria**:

- Zero workspace test failures.
- All 22 deep-path sites HOLD (same as before).
- ozon.ru moves from INTR to PASS (because of `get_follow`).
- canadagoose.com and hyatt.com: no worse than before. Ideally they
  pass because the Kasada ips.js's `location.reload()` now actually
  reloads.
- adidas.com, homedepot.com: same as before (expected — they need
  capability work, not architecture).

## Step 7 — Commit and update docs

```bash
git add -A
git commit -m "Remove per-engine logic from navigate, add generic navigation primitives

- Implement location.reload/href/replace and <meta refresh> as real navigation
  signals via globalThis.__pendingNavigation
- Add frame-level init script registry (addScriptToEvaluateOnNewDocument equivalent)
- Replace navigate_with_challenges with generic navigate(url, max_iterations)
- Delete Kasada token forwarding, WBAAS logging, is_challenge_page detection
- Make humanize script opt-in via navigate_humanized API
- Closes #73, #74, #75, #76"
```

Update:

- `docs/universal_engine/02_current_state.md` — new blocker probe
  results.
- `docs/universal_engine/TODO.md` — mark #73-76 as ☑.
- `docs/universal_engine/site_debugging/<any_site>.md` — if the
  refactor flipped a site's state, update the file.

## Risks and fallbacks

### Risk 1: Kasada sites regress

If canadagoose/hyatt stop returning `x-kpsdk-cr: true` on the `/tl`
POST after the refactor, the per-engine code was actually doing
something load-bearing that the generic path isn't. Debug approach:

1. Print all fetch log entries after the solver runs.
2. Verify the `/tl` POST body is the same before and after refactor.
3. Verify the `/tl` POST response is parsed identically (same
   `Set-Cookie` extraction).
4. If the Kasada ips.js isn't calling `location.reload()` — maybe
   it's using some other navigation trigger — capture what it IS
   calling and handle that generically.

### Risk 2: V8 isolate reuse issues

Dropping and rebuilding the `JsRuntime` on each iteration may surface
issues we haven't seen before (e.g., memory leaks, stale fetch client
references). Mitigation: run the full workspace test suite plus a
manual `cargo test -p browser --test deep_path_validation` cycle
before and after to verify stability.

### Risk 3: Scripts that call reload() in a loop

Pathological case: a script calls `location.reload()` in an event
handler that fires on every navigation. With `max_iterations=5` we
hit the limit and return the last page. User sees "stuck in a loop"
but the browser doesn't crash.

## After this refactor

You can start Sprint 1 (cheap wins) and Sprint 2 (T1.x capability
work) with confidence that the architectural foundation is clean.

See `TODO.md` for the sprint plan and `plans/` for the next items.
