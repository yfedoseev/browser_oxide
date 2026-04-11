# 04 — Refactor plan: reaching zero per-engine runtime logic

This is the concrete step-by-step plan to remove all engine-specific code
from browser_oxide's runtime. Total estimate 4-9 hours of focused work.

## Prerequisite: commit the current state as a checkpoint

Before starting any of this, commit the current workspace as a clean
baseline so you have a rollback point. The refactor touches `page.rs`
invasively.

```bash
cd /home/yfedoseev/projects/browser_oxide
git status
cargo test --workspace -- --test-threads=1  # confirm green
git add -A
git commit -m "Checkpoint before zero-per-engine refactor"
```

## Step 1 — Implement real navigation primitives (task #74)

**File**: `crates/js_runtime/src/js/window_bootstrap.js`

Currently `location.reload()` is a no-op and `location.href = url` just
parses the URL into `_locationData` without signaling anything. Real
browsers navigate.

**Implementation sketch:**

```js
// In window_bootstrap.js, around the `location` Proxy:
globalThis.__pendingNavigation = null;  // {url, kind}

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
            _parseLocationUrl(value);
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

**Also handle `<meta http-equiv="refresh">`**: scan the DOM after parsing
for `<meta http-equiv="refresh" content="N;url=...">` tags. If found,
schedule a setTimeout that sets `__pendingNavigation`. Code goes in
`page.rs::build_page_with_scripts` after the DOM is built, before the
event loop drain.

**Acceptance test**: write a new test `navigation_primitives_test.rs` that:

1. Serves a local HTML page with `<script>location.reload()</script>`
2. Verifies that `__pendingNavigation` is set
3. Does the same with `location.href = 'https://example.com'`
4. Does the same with `<meta http-equiv="refresh" content="0;url=...">`
5. Does the same with `location.replace('...')`

## Step 2 — Build a generic `navigate(url, max_iterations)` (task #75)

**File**: `crates/browser/src/page.rs`

Replace the entire `navigate_with_challenges` function. New signature:

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
        for iter in 0..max_iterations {
            eprintln!("[navigate] iter {iter}: GET {current_url}");
            let resp = client.get_follow(&current_url, 10).await
                .map_err(|e| deno_core::error::AnyError::msg(e.to_string()))?;
            let html = resp.text();
            let resp_url = resp.url.clone();

            let mut page = Self::build_page_with_scripts(
                &html, &resp_url, &profile, &client,
            ).await?;

            // Run the event loop so scripts (including challenge scripts)
            // execute and set __pendingNavigation if they want to navigate.
            page.event_loop()
                .run_until_idle(std::time::Duration::from_secs(30))
                .await?;

            // Check for pending navigation.
            let pending_js = page
                .event_loop()
                .execute_script(
                    "JSON.stringify(globalThis.__pendingNavigation || null)",
                )
                .unwrap_or("null".to_string());
            if pending_js == "null" || pending_js.is_empty() {
                return Ok(page);
            }
            // Parse the pending navigation URL and loop.
            if let Ok(val) = deno_core::serde_json::from_str::<
                deno_core::serde_json::Value,
            >(&pending_js) {
                if let Some(url) = val.get("url").and_then(|u| u.as_str()) {
                    current_url = url.to_string();
                    drop(page); // release V8 isolate
                    continue;
                }
            }
            return Ok(page);
        }
        // Max iterations hit — return whatever we have.
        let resp = client.get(url).await
            .map_err(|e| deno_core::error::AnyError::msg(e.to_string()))?;
        Self::build_page_with_scripts(&resp.text(), url, &profile, &client).await
    }
}
```

Key differences from `navigate_with_challenges`:

- **Uses `client.get_follow(url, 10)` instead of `client.get(url)`** — HTTP
  307/302 redirects are followed automatically, which unblocks ozon.ru's
  `__rr` redirect loop and any similar sites.
- **No `is_challenge_page` check** — every response is treated the same:
  run the scripts, see what they do.
- **No Kasada token extraction** — gone entirely.
- **No WBAAS logging** — gone entirely.
- **No JS-level XHR retry** — gone. The scripts run, they set
  `__pendingNavigation`, we navigate again via the HTTP client which uses
  the cookie jar that now contains whatever the scripts set.
- **Bounded loop** via `max_iterations` to prevent infinite redirect loops.

### Behavioral considerations

- **Kasada sites** currently benefit from the per-engine token forwarding.
  After the refactor, they'll work only if Kasada's ips.js eventually calls
  `location.reload()` or sets cookies that the cookie jar picks up. If the
  current Kasada sites regress, that's diagnostic signal: the ips.js isn't
  following the real-browser pattern, or our cookie jar isn't getting the
  tokens. Both are fixable generically.
- **Akamai sites** currently don't pass anyway. No expected regression.
- **Wildberries** relies on x_wbaas_token being set via `document.cookie =
  ...` from the solver script. Our task #8 already unified
  `document.cookie` writes with the `net::HttpClient` cookie jar. So the
  refactor should work for WB in principle.

## Step 3 — Delete the engine-specific code (task #73)

Once step 2's `navigate` works, remove the old code:

- Delete `is_challenge_page` function and all its markers.
- Delete `solver_session_tokens` Vec and all code that populates or
  consumes it.
- Delete the WBAAS `status-no-id` and `x-wbaas-token` header logging.
- Delete the Kasada `__fetchLog` scanning for `/tl` POST responses.
- Delete the JS-level XHR/fetch retry block (`globalThis.__retryHtml`,
  `__retryStatus`, `__retryCookies` etc.) — it's no longer needed because
  the Rust navigate loop handles iterative navigation generically.
- Mark `navigate_with_challenges` as deprecated and re-export it as an
  alias for `navigate` during the transition. After one release remove it.

## Step 4 — Decide on the humanize script (task #76)

The humanize script at `page.rs` ~line 1025-1120 is generic (fires for all
sites) but semantically is "anti-bot workaround". Three options:

**Option A: Delete it.** Closest to real Chrome. Will not regress any
currently-passing site (we verified this in the session — removing it
didn't flip any site from passing to failing). May slightly worsen the
Akamai section 6 event counts (which already don't unblock trust anyway).

**Option B: Make it opt-in.** Add `Page::navigate(url, profile,
max_iterations)` as the default (no humanize), and
`Page::navigate_humanized(url, profile, max_iterations)` that calls
humanize before running scripts. Tests can opt in explicitly. Production
scrapers default to the clean path.

**Option C: Keep as-is.** Pragmatic but impure.

**Recommended: Option B.** It preserves the observed fingerprint impact on
Akamai sites without putting synthetic user behavior in the default
navigation path. Make the default clean; let tests request the humanized
version.

## Step 5 — Regression gate

After all of the above, run in sequence:

```bash
# Workspace must still be green.
cargo test --workspace -- --test-threads=1

# The 22 deep-path passing sites must still HOLD.
cargo test -p browser --test deep_path_validation -- \
    --ignored --test-threads=1 --nocapture

# The 8 blockers should be the same or better.
cargo test -p browser --test blocker_rigorous_probe tier05_blockers_all -- \
    --ignored --test-threads=1 --nocapture
```

**Acceptance criteria**:

- Zero workspace test failures.
- All 22 deep-path sites HOLD with no DEGRADE on the deep paths (except
  the known-bad amazon product URL and crunchbase search).
- The 8 blockers are at least as good as before — stable INTR, not ERR.
  Ideally some of them move from INTR to PASS (e.g., ozon should move
  because of `get_follow`).

## Step 6 — Commit and document

```bash
git add -A
git commit -m "Remove per-engine logic from navigate path, replace with \
generic navigate() loop honoring location.reload/href/meta-refresh"
```

Update `docs/universal_engine/02_current_state.md` with the new baseline
numbers.

## Non-goals for this refactor

Things that should **not** be part of this refactor:

- T1.1 skia-safe canvas — separate workstream, 25-35 hours.
- T1.2 cosmic-text font stack — separate workstream, 50-70 hours.
- T1.4 OSMesa WebGL — separate workstream, 35-50 hours.
- Web Worker `location` API — Workers already have their own bootstrap
  that doesn't need the navigation primitives.
- CDP `addScriptToEvaluateOnNewDocument` at the protocol level — we don't
  expose CDP to the outside, our equivalent is the bootstrap-script-at-
  runtime-construction pattern which already works for us.

## What the refactor won't fix

Per the research in `03_research_landscape.md`, the following sites are
unlikely to pass after the refactor:

- adidas, homedepot (Akamai BMP v3) — need bit-accurate canvas/audio/font
  values beyond what we have today.
- canadagoose, hyatt (Kasada) — need bit-accurate canvas/WebGL and
  possibly specific cookie handling for the Kasada `KP_UIDz` session
  cookie.
- dns-shop.ru (QRATOR) — needs a PoW solver (nonce and qsessid are
  currently empty in the POST body, indicating the solver script isn't
  running to completion, possibly missing an API we don't implement).

These need the capability work in `05_capability_gaps.md`, not
architectural cleanup.
