# 07 — homedepot Akamai sec-cpt: the empty verify-URL root cause

**Date:** 2026-05-30  **Mode:** READ-ONLY research (no code edited)
**Site:** homedepot.com — `Akamai-sec-cpt-CHL` (2.6 KB interstitial, never solves)

---

## TL;DR

The sec-cpt PoW bundle computes an **empty verify-POST URL** because
**browser_oxide never sets `document.currentScript`** during external
script execution. `_setCurrentScript()` exists and is even exported,
but **nothing ever calls it** — `document.currentScript` is permanently
`null`. The Akamai sec-cpt bundle reads its own `<script>` element
(via `document.currentScript` and/or `currentScript.src`) to derive its
relative verify endpoint; with `currentScript === null`, that read
yields `undefined`/`""`, the bundle hands `new URL("", base)` an empty
string, and `URL parse error: relative URL without a base` is thrown.
No verify POST → no `sec_cpt` cookie → stuck on the interstitial.

A secondary defect compounds it: even when the bundle falls back to
`document.scripts`/`querySelector('script[src]')`, our
`HTMLScriptElement.src` returns the **raw relative attribute**, not the
IDL-reflected **absolute** URL that real Chrome returns.

---

## Evidence chain

### 1. `document.currentScript` is dead — never set during execution

- `crates/js_runtime/src/js/dom_bootstrap.js:1349-1350`
  ```js
  let _currentScript = null;
  function _setCurrentScript(el) { _currentScript = el; }
  ```
- `dom_bootstrap.js:1397` — `get currentScript() { return _currentScript; }`
- `dom_bootstrap.js:574` — `globalThis.__browser_oxide._setCurrentScript = _setCurrentScript;`
  (exported …)
- **`_setCurrentScript` is invoked by NOTHING.** A full-tree grep
  (`grep -rn _setCurrentScript crates/`) returns exactly the definition
  (1350) and the export (574). No JS caller, no Rust caller. The hook
  was wired up but the call site was never added.

### 2. The script-execution path never wires currentScript

`crates/browser/src/page.rs` executes external scripts at **line 650**:

```rust
match client.get_follow(&full_url, 10).await {
    Ok(resp) => {
        let code = resp.text();
        if let Err(e) = event_loop.execute_script(&code) {   // ← page.rs:650
```

(Mirrored in the warm/init/storage navigate variants and the inline
paths at `page.rs:509/557/660`.) Every one calls
`event_loop.execute_script(code)` / `execute_script_with_name(code,url)`
with **no `_setCurrentScript` bracketing** — the V8 entry point
(`crates/js_runtime/src/lib.rs:93 execute_script`) just compiles & runs
the source string. So while the sec-cpt bundle runs,
`document.currentScript` is `null`.

### 3. The bundle reads currentScript — proven by a sibling Akamai sample

The deobfuscated Akamai bootstrap (`browser_oxide_internal/docs/
akamai_sensor_analysis/samsclub_akam13_bootstrap.deob.js:349`) reads
`document.currentScript` directly:

```js
u['currentScript'] && 'nonce' in u['currentScript'] &&
  u['currentScript']['nonce'] && f['setAttribute']('nonce', u['currentScript']['nonce'])
```

The sec-cpt interstitial is the same family. Its captured shape
(`browser_oxide_internal/docs/research_2026_05_14/
20_VENDOR_CHALLENGE_JS_UNIFIED_2026_05_15.md`) is:

```html
<script type="text/javascript"
  src="/Wjv3muMJul/a-27ijBVRX/bQEmGXt1/PwknTm9wYQE/QB/daSgMCCTMu?v=<uuid>&t=<token>"></script>
<div id="sec-if-cpt-container" …></div>
```

The nonce / difficulty / **verify_url** live *inside* that obfuscated
bundle, and the bundle reconstructs its callback path from **its own
script element** (`currentScript` / `currentScript.src`, parsing the
`?v=&t=` query). That is the standard Akamai/sec-cpt self-location
idiom.

### 4. The empty URL is COMPUTED empty, then fails resolution

The live diagnostic shows two POST classes from the running bundle:

- **succeeds (HTTP 201)** → `https://www.homedepot.com/Wjv3…/…` —
  the *sensor* path, which the bundle holds as a hard-coded/origin-built
  string (the same `o = location.origin` idiom seen at
  `…/samsclub_akam13_bootstrap.deob.js:489-497`). location.href IS set
  in BO, so these resolve fine.
- **fails** → `{"url":"","status":0,"error":"URL parse error: relative
  URL without a base"}` — the *verify* path, which the bundle builds
  from `currentScript(.src)`. With `currentScript === null`, the path
  string is empty.

Both our resolvers (`crates/js_runtime/src/js/fetch_bootstrap.js:191`
and `window_bootstrap.js:3735` XHR `open`) DO resolve relatives against
`location.href` — so an empty *result* URL is not a resolver bug. The
`relative URL without a base` text is the signature of `new URL(x)`
called with `x` empty/relative and **no/empty base** — i.e. the bundle
itself ran `new URL("")` (or `new URL(currentScript.src)` where
`.src === undefined`) **before** handing it to fetch/XHR. The URL is
empty at the bundle's source, exactly as the task states.

### 5. Secondary defect — `.src` is relative, not absolute

`dom_bootstrap.js:650`
```js
get src() { return this.getAttribute("src") || ""; }
```
Real Chrome's `HTMLScriptElement.src` IDL getter returns the **resolved
absolute** URL (`https://www.homedepot.com/Wjv3…?v=…&t=…`). Ours returns
the raw relative attribute (`/Wjv3…?v=…&t=…`). So even the fallback
`document.scripts[i].src` / `querySelector('script[src]').src` path —
which the bundle uses when `currentScript` is unavailable — gives the
bundle a relative string with the `?v=&t=` it needs but no origin; if
the bundle does its own `new URL(scriptEl.src)` (no base) that *also*
throws `relative URL without a base`. This is the same failure by a
second route and must be fixed together.

> Note: the 2026-05-15 doc-20 "wrong-challenge-type BMP-POST conflation"
> theory is **superseded**. That run predates the cookie/FormData fixes
> and the empty-URL diagnostic. The current, sharper signal is the
> bundle's OWN verify POST going out with an empty URL — a missing
> client primitive (`currentScript`), not a Rust driver mis-route.

---

## Root cause (file:line)

| # | Defect | Location |
|---|--------|----------|
| **A (primary)** | `document.currentScript` is `null` during every external (and inline) script run — `_setCurrentScript` is defined+exported but never invoked. The sec-cpt bundle reads `currentScript`/`.src` to build its verify endpoint → empty string → `new URL("")` → no verify POST → no `sec_cpt` cookie. | call site **missing** in `crates/browser/src/page.rs:650` (+ inline siblings 509/557/660); dead hook at `crates/js_runtime/src/js/dom_bootstrap.js:1350` / export `:574` / getter `:1397` |
| **B (secondary)** | `HTMLScriptElement.src` returns the **relative** attribute, not the resolved **absolute** URL Chrome returns — breaks the `document.scripts`/`querySelector` fallback the bundle uses too. | `crates/js_runtime/src/js/dom_bootstrap.js:650` |

There is **no** missing config object (`_sec_cpt`/`_cf_chl`/`bazade…`)
— doc-20 confirmed the sec-cpt interstitial carries **no inline JSON**;
everything (nonce/difficulty/verify_url) is computed inside the bundle
from its script element. There is **no** missing 201-response header /
Location / Set-Cookie that BO discards: the verify never fires because
its URL is empty.

---

## Concrete public-engine fix (no vendor_solvers)

**Fix A — set `document.currentScript` around every script execution.**
At each external-script execution in `page.rs` (the `:650` site and the
inline siblings), bracket the run so the corresponding `<script>` DOM
node is the active `currentScript`. Lowest-risk implementation, done
entirely in the JS pipeline (no new ops):

1. In `find_scripts`/`collect_scripts`
   (`crates/browser/src/script_runner.rs:24/37`) also capture the
   script element's `NodeId` (already walked) so each `ScriptInfo`
   knows its DOM node.
2. Wrap execution: before `execute_script(&code)` at `page.rs:650`
   (and the inline/init/warm variants), run a tiny prologue that locates
   the node and calls the existing hook, e.g.
   ```js
   globalThis.__browser_oxide._setCurrentScript(
       /* wrap NodeId -> element */ );
   ```
   and an epilogue resetting it to `null` (per spec, `currentScript` is
   `null` outside script execution and during async callbacks). The hook
   (`_setCurrentScript`, dom_bootstrap.js:1350) already exists — this is
   purely adding the call site that was never written. For external
   scripts the located element is the `<script src>` node already in the
   DOM; for inline scripts it's the inline `<script>` node.

   If wiring the exact NodeId is heavy, an acceptable minimal first cut
   for the sec-cpt case is: just before executing an **external** script,
   set currentScript to that script's element resolved by src, e.g.
   `document.querySelector('script[src=...]')`, then null it after.

**Fix B — make `HTMLScriptElement.src` (and img/iframe/link href) return
the absolute, base-resolved URL.** `dom_bootstrap.js:650`:
```js
get src() {
    const v = this.getAttribute("src");
    if (!v) return "";
    try { return new URL(v, globalThis.location?.href || undefined).href; }
    catch (e) { return v; }
}
```
This matches Chrome's IDL-reflected `.src` and feeds the bundle a usable
absolute URL through the `document.scripts` fallback too.

With both, the bundle's `currentScript.src` (and `scripts[i].src`)
returns the real absolute `/Wjv3…?v=&t=` endpoint → it builds a
non-empty verify URL → the verify POST lands → Akamai sets `sec_cpt`
(→ `~3~`) → the existing `started_as_seccpt_challenge` /
`is_seccpt_solved` retry loop (`page.rs:259,1915,2345`) re-fetches `/`
→ `Akamai-sec-cpt-CHL` → `L3-RENDERED`.

---

## Validation plan

1. Live homedepot nav with XHR/fetch logging; assert **zero** POSTs
   with `"url":""` and a POST to the absolute `/Wjv3…?v=&t=` verify path.
2. Assert `sec_cpt=` lands in the jar and flips to `~3~`; body grows
   past the 2.6 KB interstitial.
3. Unit: a fixture page `<script src="/x/y?v=1">` whose inline code
   asserts `document.currentScript.src === <absolute>` and
   `document.currentScript !== null`.
4. Full §4 gate — Fix A/B are spec-correct (currentScript + absolute
   `.src` are Chrome invariants) ⇒ expected zero regression, but they
   touch a global path, so run the gate.

---

## Confidence

**High** that the missing `document.currentScript` (Fix A) is the
empty-URL source: `_setCurrentScript` is provably dead code, the sibling
Akamai sample reads `currentScript`, and the error text is the exact
signature of an empty URL string. **Medium-high** that A+B alone flip
the site end-to-end — the verify POST still has to be *accepted* by
Akamai (correct PoW answer from the in-VM bundle), which prior runs
showed the bundle computes; but if any further fingerprint gating
exists it would surface only after the verify URL is non-empty. A+B are
prerequisites either way and are spec-correct, low-risk, public-engine
changes.
