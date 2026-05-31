# CSR thin-render diagnosis — duolingo.com

**Date:** 2026-05-30
**Site:** https://www.duolingo.com/ (React 18 SPA, webpack 5, ~11 KB HTML shell, `id="root"`)
**Symptom:** BO returns `L3-RENDERED len=13327` (tiny shell, `#root` empty, 0 JS errors).
The MessageChannel/adidas async-delivery fix did NOT flip it.
**Verdict:** Root cause found and isolated to a single engine gap (+ one secondary
gap behind it). **flippable: yes** with a 1-line `module_loader.rs` change (+ a small
`Blob.prototype.stream` addition for full robustness). **No vendor_solver — public-engine fix.**

---

## TL;DR root cause

duolingo's `features-91ec4487.js` runs an early **capability probe** for native
dynamic `import()`:

```js
["dynamicImport", () => new Function(
   "return import('data:text/javascript;base64,Cg==') instanceof Promise")()]
```

`Cg==` is base64 for `"\n"` — a minimal empty data-URL module. The probe only
checks `… instanceof Promise` (synchronously true), so it **never `.catch()`es the
import promise.** In real Chrome that import resolves silently. In BO,
`BrowserModuleLoader::load` **rejects every non-`http(s)` specifier**, so the
`import('data:…')` promise rejects with
`TypeError: Module loading is not supported`. Because nothing catches it, it
becomes an **unhandled rejection**, and deno_core surfaces that as an **error out
of `run_event_loop`** — which **aborts the current drain tick**.

That abort happens *before* React's render commit runs. React 18's scheduler
posts a `MessageChannel` macrotask (`port2.postMessage(null)` →
`port1.onmessage = performWorkUntilDeadline`) to commit the fiber tree into
`#root`. The drain dies on the uncaught import rejection, so the scheduler
macrotask never fires → `#root` stays empty.

This is **NOT** the same bug as adidas. The MessageChannel async-delivery fix is
correct and works here; the React scheduler is simply never *reached* because an
earlier uncaught `import('data:…')` rejection kills the event loop.

---

## Evidence ladder (all reproduced)

All scripts fetch + execute cleanly first (confirmed with `BROWSER_OXIDE_SC_TRACE=1`):
`manifest`(13.6 KB), `features`(3.1 KB), `8553`, `polyfills`, `9819`(292 KB),
`strings/en`, `1319`(746 KB, React 18), `app`(1.07 MB). The webpack runtime runs and
even pulls 4 lazy chunks (`1118/2231/8186/4800`) via `op_net_fetch_sync`.

The `<body>`-tail **feature gate** (`supportsES2015`, `supportsEs2019`,
`IntersectionObserverEntry`, `ResizeObserver`, `WebAssembly`, `AbortController`,
`Element.animate`) was suspected but **fully PASSES in BO** — all 7 flags `true`,
including the bleeding-edge `new Function('class ಠ_ಠ extends Array …')` ES2015
probe. So BO is **not** redirected to `/errors/not-supported.html`.

Offline harness (real chunks loaded inline, driven through `BrowserEventLoop` +
`run_until_idle`), instrumented with a `window.__P` side-channel (immune to the
app's `console` monkey-patch in module 72398) and a patched React scheduler
(`window.__SCHED`):

| Stage probe | Baseline | After neutralizing `dynamicImport` probe | + `Blob.prototype.stream` shim |
|---|---|---|---|
| webpack entry module 22716 runs | yes | yes | yes |
| bootstrap IIFE → `createRoot/render(p)` called | **yes** (`render-CALLED`, `iife-RESOLVED`) | yes | yes |
| plain `setTimeout(fn,0)` fires in drain | **no** (`st:0`) | **yes** (`st:1`) | yes |
| React scheduler MessageChannel `posted / fired` | `posted:1, fired:0` | `posted:1, fired:1` | `posted:2, fired:2` |
| `run_until_idle` result | `Err(TypeError: Module loading is not supported … data:text/javascript;base64,Cg==)` | `Err(TypeError: …stream is not a function)` | `Ok(AllWorkDone)` |
| React attaches to `#root` (`_reactRootContainer`) | yes | yes | yes |

**Isolation test** (single statement, no bundle): an uncaught
`new Function("return import('data:text/javascript;base64,Cg==') instanceof Promise")()`
followed by `setTimeout(()=>fired=1,0)` →
`run_until_idle` returns `Err(TypeError: Module loading is not supported …)` and
**`fired=0`**. A plain `setTimeout` in a clean runtime (no import) → `fired=1`.
This is the smoking gun: the uncaught data-URL `import()` rejection alone aborts the
drain and starves macrotasks.

After the import probe is neutralized AND `Blob.prototype.stream` is shimmed, the
drain is clean (`Ok`) and the React scheduler cycles fully (`fired:2`). `#root`
remained empty *in the offline harness only* because that harness has no network —
the logged-out splash component awaits config/experiment fetches that 404 offline.
That residual is a harness limitation, not the live blocker; live BO has network.

---

## Precise file:line evidence

1. **PRIMARY blocker — `data:` module imports rejected.**
   `crates/js_runtime/src/module_loader.rs:67-71`:
   ```rust
   // Only http(s) modules are network-fetchable; data: modules are inlined
   // by V8 before reaching the loader.
   if !(url.starts_with("http://") || url.starts_with("https://")) {
       return ModuleLoadResponse::Sync(Err(AnyError::msg(format!(
           "module loader: unsupported specifier {url}"
       ))));
   }
   ```
   The comment’s assumption ("data: modules are inlined by V8 before reaching the
   loader") is **false** for `import('data:…')` in this deno_core 0.311 build — the
   data-URL reaches `load` and is rejected. Chrome supports data-URL module imports
   (`import('data:text/javascript,…')`).

2. **React scheduler delivery (works, not the bug):**
   `crates/js_runtime/src/js/window_bootstrap.js:2381-2413` (`_deliver` schedules the
   paired-port `onmessage` on a macrotask via `__bgSetTimeout||setTimeout`). Verified
   firing once the drain is no longer aborted (`fired:2`).

3. **SECONDARY blocker (surfaces only after #1 is fixed) — `Blob.prototype.stream`
   missing.** `crates/js_runtime/src/js/shared_apis_bootstrap.js:339-361`: the `Blob`
   class implements `text()`, `arrayBuffer()`, `slice()` but **not `stream()`**.
   duolingo's state persister (app module 30839/97755) compresses saved state via
   `new Blob([JSON.stringify(state)]).stream().pipeThrough(new CompressionStream('gzip'))`
   (app.js + 1319.js). With `stream` absent this throws
   `TypeError: (intermediate value).stream is not a function`, again aborting the drain.

4. **TERTIARY latent gap (not load-bearing for first paint) — `CompressionStream`/
   `DecompressionStream` are non-functional stubs.**
   `crates/js_runtime/src/js/window_bootstrap.js:2487-2501` construct empty
   `ReadableStream`/`WritableStream` with no gzip codec. The persister’s gzip
   save path and the decode path (`d` in module 30839, gated on a pre-existing
   compressed `localStorage["duo.state"]` value, i.e. only on a *return* visit)
   would silently produce empty output. First-load render does not depend on this.

5. **Self-navigation symptom (explains the 3 navigate iterations in the live log).**
   During bootstrap the app assigns `location.href` (the engine setter at
   `window_bootstrap.js:1353-1357` writes `__pendingNavigation={url,kind:"assign"}`
   and signals `op_set_pending_nav`). The live navigate loop (`page.rs` nav loop)
   then re-fetches the **same** `https://www.duolingo.com/` for iter 1 and iter 2.
   This is a consequence of the aborted render (the app never reaches steady state),
   not an independent cause; it does not need a separate fix once render commits.

---

## Recommended fix (public engine, minimal)

**Primary (flips the site):** make `BrowserModuleLoader::load` handle `data:` module
specifiers instead of rejecting them. In `crates/js_runtime/src/module_loader.rs:65`,
before the http(s) guard, add a `data:` branch that decodes the URL
(`text/javascript[;base64],<payload>`) and returns a synchronous
`ModuleSource::new(ModuleType::JavaScript, ModuleSourceCode::String(decoded), &spec, None)`.
This makes `import('data:text/javascript;base64,Cg==')` resolve (to an empty module),
exactly like Chrome, so duolingo’s capability probe leaves no uncaught rejection and
the drain survives to run React’s commit.

Belt-and-suspenders: also ensure deno_core is created with an unhandled-rejection
handler that does **not** terminate the event loop (so a single stray rejection
elsewhere can’t starve a page’s render). The `data:` fix removes this specific
trigger; the handler hardens against the whole class.

**Secondary (robustness for the persister + general parity):**
- Add `stream()` to `Blob` in `shared_apis_bootstrap.js:339` returning a
  `ReadableStream` that enqueues `this._data` then closes.
- Give `CompressionStream`/`DecompressionStream`
  (`window_bootstrap.js:2487-2501`) a real gzip/deflate TransformStream (or back
  them with a Rust op) so persisted-state round-trips work on return visits.

**Expected outcome:** with the `data:`-import fix alone, the live drain stops aborting,
React’s MessageChannel commit fires, and `#root` populates with the duolingo
marketing/splash tree → BO renders full content instead of the 13 KB shell. The
`Blob.stream`/`CompressionStream` additions remove the next gap the persister hits
and prevent regressions on return-visit (warm-localStorage) loads.

## Cross-site note

This `import('data:…')`-probe-aborts-the-drain mechanism is **framework-agnostic**
(it is a bundler/feature-detect idiom, not duolingo-specific). The same
`features.js`-style `dynamicImport` capability probe — or any uncaught data-URL
`import()` — will starve render on other webpack/Vite SPAs. Worth re-testing the
other thin-render sites (douyin/ozon/wildberries) for the identical
`Err(Module loading is not supported … data:…)` signature in their drain.
