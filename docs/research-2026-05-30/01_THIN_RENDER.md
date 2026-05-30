# Thin-Render Cluster — Root Cause & Fixes (2026-05-30)

**Cluster:** douyin, duolingo, adidas, ozon, wildberries — BO renders a 1.8–13 KB
shell where camoufox v150 renders 100 KB–1.57 MB. `homedepot`/`etsy` are a
separate (challenge-fingerprint) cluster and are only summarized here.

**Pass criterion:** `L3-RENDERED && len >= 15000`.

**Headline:** NONE of these are IP blocks (v150 passes the same IP — ozon +
wildberries prove it). The dominant root cause is a **missing ES-module
execution path in the main document**. Modern React/Vue SPAs ship their entry
point as `<script type="module" src="/assets/index-[hash].js">`; BO fetches that
file and runs it through the **classic-script** compiler, where the first
`import`/`export` statement throws `SyntaxError: Cannot use import statement
outside a module` at compile time. The app never bootstraps ⇒ only the server
shell remains.

---

## 1. The exact render pipeline (verified, cold `Page::navigate`)

1. One top-level GET of the document HTML.
2. Parse once: `html_parser::parse_html` then `script_runner::find_scripts`
   walks the **parsed** DOM — `page.rs:3167-3168`. It only sees `<script>`
   tags present in the served HTML.
3. Parallel prefetch of all external `<script src>` + stylesheets —
   `page.rs:3204-3327`. (HTML-looking bodies are dropped, `:3282-3286`.)
4. Execute scripts in document order via
   `event_loop.execute_script_with_name(code, name)` — `page.rs:3653` — with a
   `run_until_idle(50ms)` micro-drain between each (`page.rs:3679`).
5. Fire DOMContentLoaded/load via `setTimeout(0)` (`page.rs:3692-3702`), scan
   meta-refresh, then a single **`run_until_idle(8s)`** build drain
   (`page.rs:3756`).
6. Outer nav loop (`page.rs:2035+`) re-fetches only if JS set
   `__pendingNavigation`; otherwise it serializes `outerHTML` and returns.

`execute_script_with_name` → `BrowserJsRuntime::execute_script`
(`js_runtime/src/lib.rs:93-151`) compiles with **`v8::Script::compile`**
(`lib.rs:122`) — the classic-script entry. There is **no** `compile_module` /
module-graph / dynamic-`import()` path anywhere in the main-document pipeline
(grep-verified).

---

## 2. Root cause per site (failure-mode classification)

| Site | v150 | BO | Failure mode | Primary root cause |
|---|---|---|---|---|
| **douyin** | 981 KB | 6.3 KB | thin shell | ES-module entry never executes (`type=module`) + no SPA budget + IntersectionObserver one-shot (feed) |
| **duolingo** | 1.14 MB | 13.3 KB (BORDERLINE) | near-miss shell | ES-module entry never executes; partial inline hydration produces ~13 KB but the module app bundle is dropped |
| **adidas** | 1.52 MB | 2.5 KB | thin shell + Akamai BMP | ES-module entry never executes; gets the 25 s Akamai tier but the bundle is `import`-gated |
| **ozon** | 106 KB | 156 B | THIN-BODY | ES-module / chunked SPA entry never executes; not in SPA budget allowlist |
| **wildberries** | 1.57 MB | 1.8 KB | thin shell | ES-module / chunked SPA entry never executes; infinite-scroll grid + not in budget allowlist |

All five share **GAP 1** as the load-bearing cause; douyin/wildberries also hit
**GAP 3** (viewport-gated content); none of the five are on the SPA budget
allowlist (**GAP 2**) except adidas (Akamai 25 s tier).

> **Verify-by-instrument next step:** run each site with
> `BROWSER_OXIDE_BUILD_PROFILE` + the `[JS LOG]`/"Script execution error" trace
> (`page.rs:3654`). The hard prediction for all five: at least one external
> `<script type="module">` logs `SyntaxError: Cannot use import statement
> outside a module`. That single log line is the confirmation oracle for GAP 1.

---

## 3. The gaps, with file:line

### GAP 1 — No ES-module / `import()` support in the main document (DOMINANT)

- `find_scripts` does **not** distinguish `type="module"`. The `match
  script_type` at `script_runner.rs:44-54` only skips data blocks (`ld+json`,
  `application/json`, `text/template`, `text/html`, `text/x-template`);
  `Some("module")` falls into `_ => {}` and is queued like a classic script
  (`script_runner.rs:53`).
- Those queued module scripts are executed via `v8::Script::compile`
  (`js_runtime/src/lib.rs:122`). Any top-level `import`/`export` throws
  `SyntaxError` at compile time → the whole bundle is dropped (logged at
  `page.rs:3654`, then the loop `continue`s).
- Dynamically injected modules hit the same wall: `_onNodeInsertedInner`
  treats `type === 'module'` as `isJs = true` (`dom_bootstrap.js:146`), then
  runs the body via `(0, eval)(code)` (`dom_bootstrap.js:236` sync /
  `:267` async). **Indirect `eval` cannot execute `import` statements** — same
  `SyntaxError`.
- Dynamic `import('./chunk.js')` is unimplemented in the main runtime — there
  is no host import callback registered, so the returned promise rejects.

**Consequence:** the React/Vue/Vite/webpack entry chunk (the thing that fetches
data, hydrates, and mutates the DOM to full content) never runs ⇒ only the
1.8–13 KB server shell survives. This is the single highest-leverage gap for
**5 of 5** sites in this cluster.

**De-risking fact:** the module machinery already exists in this repo — the
**worker** path uses `runtime.load_main_es_module_from_code(specifier, script)`
+ `mod_evaluate` for module workers (`worker_ext.rs:344-360`). deno_core's
module loader is already wired and working for workers; the main document
simply never calls it. The fix is to route module scripts through that same
deno_core module API, plus a custom `module_loader` that resolves relative
import specifiers by fetching through the net stack.

### GAP 2 — SPA event-loop budget allowlist excludes this cluster

The per-host nav budget is a hardcoded host allowlist (`page.rs:1953-2005`):

- 90 s SPA tier: twitter/x/hulu/yandex.ru/hm/khanacademy/spotify/uber
  (`page.rs:1969-1983`).
- 45 s Kasada / homedepot tiers; 25 s Akamai-BMP tier including **adidas**
  (`page.rs:1998`).
- **15 s default** for everything else.

**douyin, duolingo, ozon, wildberries are not on any tier → 15 s default**, and
the inner build drain is capped at **8 s** (`page.rs:3756`). Even once GAP 1 is
fixed, a heavy hydration that needs >8 s build-phase wall-clock gets cut. adidas
gets 25 s outer but still only the 8 s inner build drain.

### GAP 3 — `IntersectionObserver` fires exactly once, never on scroll/mutation

`IntersectionObserver.observe` schedules a **single** `Promise.resolve().then`
reporting `isIntersecting:true`, then never fires again
(`window_bootstrap.js:3550-3566`). `ResizeObserver` is the same
(`:3578-3590`). Infinite-scroll / viewport-gated hydration (douyin feed,
wildberries product grid) that loads further batches *in response to scroll
events* gets one synthetic callback and then nothing. This caps content even
when the entry bundle runs.

### GAP 4 — dynamic `<script>` loader chains are blind `eval` with a depth cap

For *classic* injected scripts, `_onNodeInsertedInner` fetches via
`op_net_fetch_sync` and `(0,eval)`s the body. A hard recursion guard
`_MAX_SYNC_EVAL_DEPTH = 4` (`dom_bootstrap.js:73`, enforced `:210`) silently
degrades nested loader chains (`loader → vendor → app → chunk`) to async at
depth 5, and `MAX_SYNC_FETCH_PER_PAGE = 30` (`fetch_ext.rs:15,494`) caps total
sync fetches per page. Vite/webpack runtime loaders that pull a deep chunk tree
get truncated. (Secondary to GAP 1 — most modern bundles are module-based, so
they never even reach this classic path.)

### NON-gaps (mined context was outdated — do NOT re-chase)

- **Streams API is NOT a stub.** `streams_bootstrap.js` ships real
  `ReadableStream` / `TransformStream` / readers, loaded in both the snapshot
  (`snapshot.rs:144`) and runtime (`runtime.rs:220`), and `Response.body`
  returns a real `ReadableStream` (`fetch_bootstrap.js:48-73`). The stub in
  `window_bootstrap.js:2301` is only a fallback. The remaining limitation is
  that the body is delivered as one chunk (not progressive), which does not
  break SPA hydration. **Streams are not the thin-render cause.**
- **Canvas / audio / WebGL FP** are sound by construction — not the bottleneck
  for these five (they are content/hydration thin, not FP-blocked).

---

## 4. homedepot / etsy (challenge cluster — out of this cluster's fix scope)

- **homedepot** (Akamai sec-cpt, 2.7 KB CHL): `is_seccpt_solved` requires
  `sec_cpt=` + `~3~` + body no longer showing the sec-cpt container
  (`page.rs:242-247`). The classic sec-cpt PoW bundle *does* execute but never
  produces `~3~` in the live-nav drain (per HANDOFF_2026_05_28b: zero async
  progress on the 50 ms inter-script drain). Fix is wiring the verified
  `sec_cpt::solve_crypto` into the live drain, not a hydration fix.
- **etsy** (DataDome, 1.4 KB interstitial): documented bypass is **Firefox-NSS
  TLS** (DataDome weights Chromium-TLS=bot) OR FP-E1 cross-origin challenge
  iframe execution. Both are separate structural builds (see the wire-surface /
  DataDome research). Not a hydration fix.

---

## 5. Ranked fixes (ROI order)

### FIX-1 (HIGH / large) — Main-document ES-module execution path
Flips **all 5** cluster sites (douyin, duolingo, ozon, adidas, wildberries) and
many off-cluster SPAs (likely reuters/zillow/etc. that ship module entries).

- Tag module scripts at walk time: in `script_runner.rs:38-54`, capture
  `type === "module"` into `ScriptInfo` (new `is_module: bool`) instead of
  letting it fall through to the classic queue.
- In the executor at `page.rs:3624-3655`, for module scripts call a new
  `event_loop.execute_module(code, url)` that wraps deno_core's
  `load_main_es_module_from_code(specifier, code)` + `mod_evaluate` — the
  exact API already proven in `worker_ext.rs:348-360`. Provide a custom
  `module_loader` in `runtime.rs:113` (`RuntimeOptions.module_loader`) that
  resolves relative import specifiers against the document URL and fetches the
  module source through the net stack (recursively prefetching the import
  graph), and a dynamic-`import()` host callback.
- Make `dom_bootstrap.js:146/164-288` route `type=module` injected scripts to
  the same module API rather than `(0,eval)` (`:236`/`:267`).
- Effort: 1–2 sessions (module loader + import-graph fetch + dynamic-import
  callback). The worker precedent removes most of the deno_core risk.
- Files: `script_runner.rs:19-92`, `js_runtime/src/lib.rs:93-151`,
  `js_runtime/src/runtime.rs:113-139`, `page.rs:3621-3680`,
  `dom_bootstrap.js:142-288`.

### FIX-2 (MEDIUM / small) — Extend SPA budget to this cluster (or make adaptive)
Add douyin.com, duolingo.com, ozon.ru, wildberries.ru to the 90 s SPA tier
(`page.rs:1969-1983`), and raise the inner build drain above 8 s for those
hosts (`page.rs:3756`). Better long-term: make the budget content-adaptive
(extend while DOM size is still growing per drain tick) instead of a static
host list. Necessary even after FIX-1 so the hydrated app has wall-clock to
finish. Effort: hours.

### FIX-3 (MEDIUM / small) — Real IntersectionObserver re-fire
Drive `IntersectionObserver`/`ResizeObserver` callbacks on synthesized scroll
and on DOM mutation, not just a one-shot `Promise.resolve().then`
(`window_bootstrap.js:3550-3566`, `:3578-3590`). Unblocks infinite-scroll
content for douyin/wildberries beyond the first batch. Effort: hours. Lower ROI
than FIX-1/2 because without FIX-1 the observer never gets installed.

### FIX-4 (LOW) — Relax dynamic classic-loader caps
Raise `_MAX_SYNC_EVAL_DEPTH` (`dom_bootstrap.js:73`) and
`MAX_SYNC_FETCH_PER_PAGE` (`fetch_ext.rs:15`) or make degraded-async chains
awaited within an extended drain. Only matters for the minority of classic
(non-module) deep loader chains. Effort: minutes; low expected flips.

---

## 6. Confirmation oracle (do this first, it's cheap)

`benchmarks/run_delta_headtohead.py` for these 5 hosts with
`BROWSER_OXIDE_BUILD_PROFILE=1` and trace on. Expected signature confirming
GAP 1: a `SyntaxError: Cannot use import statement outside a module` logged for
the site's entry chunk, and `document.body.outerHTML.length` flat across the
8 s drain. If that signature appears, FIX-1 is the correct and sufficient first
move.
