# API — DOM collections, form APIs, and the prototype-masking integrity sweep

**Scope:** structural / API surface + Function.prototype.toString integrity.
**Audience:** anyone closing the Kasada `sfc`/`sdt`, AWS WAF WebGL, DataDome
fetch-trio, and Akamai attribute-shape gaps. This is the cross-cutting masking
chapter; it compounds across ~11 vendors.

**Method note:** every claim below was re-verified against HEAD source on branch
`fix/v0.1.0-fix4-canvas-parity` and, where load-bearing, against a live in-engine
probe run through `Page::evaluate` on a `chrome_148_*` profile (the
`check()` harness in `crates/browser/tests/chrome_compat.rs:16`). Several
conclusions in the older repo docs (17/16) are now **stale** and are corrected
here with the empirical output.

---

## 0. TL;DR — what is actually broken at HEAD

The masking architecture is far more complete than docs 16/17 imply (they were
written before "Fix 3 — Universal prototype mask sweep" landed in
`cleanup_bootstrap.js`). The residual, empirically-confirmed leaks are narrow
but high-value:

| # | Leak (verified in-engine) | Vendors | Root cause | Public engine? |
|--:|---|---|---|---|
| 1 | **Accessor (getter/setter) source leaks**: `Request.signal`, `Response.{status,statusText,ok,headers,url,body,bodyUsed}`, `ReadableStream.locked`, `WritableStream.locked`, `MessagePort.onmessage`, `URLSearchParams.size`, `WebSocket.{bufferedAmount,extensions,protocol,binaryType}` | Kasada `sdt`, DataDome, Akamai | The universal sweep masks **only `desc.value` (data methods)**, never `desc.get`/`desc.set` (`cleanup_bootstrap.js:533`) | ✅ yes |
| 2 | **Constructor body leaks**: `String(WebGLRenderingContext)` / `String(WebGL2RenderingContext)` return `class WebGLRenderingContext {…}` | AWS WAF, CreepJS, Kasada | WebGL ctors are in neither `_sfcNames` nor `_toMask`; the universal sweep masks prototypes, **not the constructor object itself** | ✅ yes |
| 3 | **`Element.attributes` is a `Proxy([])`** → `Object.prototype.toString.call(el.attributes)` = `"[object Array]"`, `.constructor === null` (Chrome: `"[object NamedNodeMap]"`) | Akamai sensor, CreepJS | `get attributes()` returns `new Proxy([], …)` (`dom_bootstrap.js:889`); `NamedNodeMap` global is never instantiated | ✅ yes |
| 4 | **Event-subclass name collapse**: `MouseEvent.name === "Event"`, `String(MouseEvent)` = `function Event() { [native code] }` | Kasada `sdt`, CreepJS name-consistency | Subclasses are masked transitively (tag inherited / shared mask) but the **name tag is wrong** | ✅ yes |
| 5 | **`HTMLCollection` global is `undefined`** (`typeof HTMLCollection === 'undefined'`) | CreepJS, any `'HTMLCollection' in window` probe | Never defined as a class; `getElementsByTagName` returns a snapshot `NodeList` | ✅ yes |
| 6 | **`NodeList`/`HTMLCollection` returned by `getElements*` are NOT live** | SPA hydration + a few vendor liveness probes | `NodeList` snapshots `_ids` at construction (`dom_bootstrap.js:300-316`) | ✅ yes |

Items 1–5 are all 0.5–1 day fixes in public crates. **Reddit
(`HTMLFormElement.elements`) is already fixed** (Fix 11) and is no longer a
blocker — doc 17 §3 #1 is stale; verified `form.elements instanceof
HTMLFormControlsCollection === true` in-engine.

---

## 1. What the existing repo docs already concluded

### 1.1 `17_WEB_API_PARITY_MATRIX.md` — the "missing API" inventory

Doc 17 is the inverse of doc 16: "do we expose the right thing at all?". Its
top-20 gotcha list (§3) flagged, in priority order:

1. `HTMLFormElement.elements` + `HTMLFormControlsCollection` — **reddit**, P0.
2. `MessagePort`/`MessageChannel` paired routing — duolingo, P0.
3. Event subclass constructors masked — Kasada, P0.
4. `Headers`/`Request`/`Response` prototypes masked — DataDome, P1.
5. `XMLHttpRequest.prototype` masked — Akamai, P1.
6. `WebGLRenderingContext.prototype.getParameter` masked — AWS WAF, P1.
20. `NamedNodeMap` returned by `Element.attributes` — Akamai, P2.

**Corrections from this pass (doc 17 is partly stale):**

- **#1 is DONE.** `HTMLFormElement.prototype.elements` now exists
  (`dom_bootstrap.js:1158-1211`) returning an object whose prototype is
  `HTMLFormControlsCollection.prototype`, with `item`/`namedItem`/
  `Symbol.iterator`. In-engine: `els instanceof HTMLFormControlsCollection ===
  true`. The reddit `e.elements.namedItem('solution')` flow works. Confirmed by
  `FAILED_SITES_ANALYSIS.md:215` ("Fix 11 … confirmed measurable target-site
  flip — 1 site recovered").
- **#3 is mostly DONE but with a name bug** — see §0 item 4 / §4.4 below.
- **#4 (constructors) DONE, (accessors) NOT** — `Headers/Request/Response` *constructors*
  are masked (`fetch_bootstrap.js:391-393`) and the universal sweep masks their
  *data methods*, but their **getters still leak** (§0 item 1).
- **#6 partly DONE** — `getParameter` itself **is** masked
  (`function getParameter() { [native code] }`, verified), but the
  **`WebGLRenderingContext` constructor** still leaks its class body (§0 item 2).
- **#20 NOT done** — `Element.attributes` is still a `Proxy([])` (§0 item 3).

### 1.2 `16_STEALTH_FINGERPRINT_AUDIT.md` — the masking primitive

Doc 16 documents the masking primitive precisely and is still accurate on the
mechanism:

- `stealth_bootstrap.js:13` — `_nativeTag = Symbol.for('__browser_oxide_native__')`
  (global registry so cross-realm masking works).
- `stealth_bootstrap.js:25-48` — the `Function.prototype.toString` patch, using
  **method-shorthand** (`{toString(){…}}.toString`) so it is non-constructable
  (matching native, defeats Kasada `fsc`), with a re-entrant guard
  (`_inPatchedToStr`) and the patched toString **itself tagged** so
  `Function.prototype.toString.call(Function.prototype.toString)` resolves to
  native.
- `stealth_bootstrap.js:51-80` — `_maskFunction(fn, name)` sets own `name` +
  the `_nativeTag` Symbol, and **deliberately installs NO own `toString`** (a
  prior self-inflicted FP — see §3.2).
- `stealth_bootstrap.js:82-104` — `_maskAsNative(obj, …names)` walks the proto
  chain, masking `desc.get`/`desc.set`/`desc.value` correctly **when called
  explicitly**. (The universal sweep does NOT use this for accessors — that's
  bug #1.)

Doc 16 §2.3 ranked the headline gaps: Event ctors, fetch trio, XHR, WebGL,
Observers, Worker.postMessage, Streams, History, Storage. **Most of these were
subsequently closed by the universal sweep** (§2 below). Doc 16's §5 audit-script
plan is the right tool and should be promoted to a committed golden test
(§5, acceptance criteria).

### 1.3 `08_KASADA_FRONTIER.md` — why this matters

Lever 3 (`08_KASADA_FRONTIER.md:138-144`): Kasada's `sfc`/`sdt` error fields
record `Function.prototype.toString.call(SomeWebAPI)` whenever it doesn't match
`/^function \w+\(\) \{ \[native code\] \}$/`. The captured decrypted blob showed
BO leaking `op_dom_attach_shadow`, raw `class Worker {…}`, and
`log(...args){core.ops.op_console_log(...)}`. Console (`ofc`, the single biggest
ML weight) is fixed (`stealth_bootstrap.js:171-181`). The residuals in §0 are
the remaining `sdt`/`sfc` contributors.

---

## 2. The actual masking architecture at HEAD (three sweeps, not one)

Docs 16/17 describe one curated sweep in `dom_bootstrap.js`. There are in fact
**three** layers; understanding all three is required to place a fix correctly.

### 2.1 Layer A — `stealth_bootstrap.js` (snapshot, FIRST)

Installs the `Function.prototype.toString` patch + the three helpers + masks all
~19 console methods + `eval`. Runs first in the snapshot
(`snapshot.rs:70`) and first in every worker (`runtime.rs:341-343`).

### 2.2 Layer B — `dom_bootstrap.js:3073-3158` (snapshot, curated)

The hand-curated `_toMask` list (`dom_bootstrap.js:3095-3132`, ~90 names) +
`_topLevelFns` list (`:3142-3151`). `_walkProto` (`:3075-3090`) masks the
constructor **and** every prototype `desc.value`/`desc.get`/`desc.set`. **This
layer DOES mask accessors** — but only for the ~90 explicitly-listed classes.
WebGL, Headers/Request/Response, Streams, WebSocket, MessagePort are **not** in
this list, so they fall through to Layer C.

### 2.3 Layer C — `cleanup_bootstrap.js:376-540` (per-page, LAST) — "Fix 3"

This is the layer docs 16/17 predate. Two parts:

1. **`_sfcNames` curated list** (`cleanup_bootstrap.js:463-499`, ~110 names)
   masks the **constructor object** of each. Includes de-aliasing
   (`clearTimeout!==clearInterval`, `DOMMatrix!==DOMMatrixReadOnly`) and the
   `_natMethod` non-constructability fix (strips illegal `.prototype` from
   `setTimeout` etc.). **WebGLRenderingContext / WebGL2RenderingContext are
   absent from this list** → bug #2.
2. **Universal prototype sweep** (`cleanup_bootstrap.js:516-540`): iterates
   **every** `globalThis` function with a `.prototype`, collects every own
   property whose **`desc.value` is a function**, and calls `_maskAsNative`.

   ```js
   for (const _n of _ns) {
       if (_n === 'constructor') continue;
       let _d; try { _d = Object.getOwnPropertyDescriptor(_p, _n); } catch { continue; }
       if (_d && typeof _d.value === 'function') _methods.push(_n);   // <-- value ONLY
   }
   if (_methods.length) { try { _mask(_p, ..._methods); } catch {} }
   ```

   **The bug (#1):** it pushes only `desc.value` methods; it never collects
   `desc.get`/`desc.set`. So every getter/setter defined with
   `get x(){…}` / `set x(){…}` in a JS class on a constructor that is NOT in
   Layer B's `_toMask` leaks its raw source. `_maskAsNative` itself handles
   accessors fine — it's the **collection step** that drops them.

### 2.4 Why the residual list is exactly what it is

Cross-referencing the three layers against the empirical audit:

- `Request`/`Response` — ctor masked (Layer C `_sfcNames`), data methods masked
  (universal sweep), **getters leak** (`signal`, `status`, `ok`, `headers`,
  `url`, `body`, `bodyUsed`) because they are `desc.get` and the class is not in
  Layer B.
- `ReadableStream`/`WritableStream`/`TransformStream` — ctor masked, methods
  masked, `get locked` / `get _browserOxideReal` leak.
- `MessagePort` — `get/set onmessage` leak (it's not the same `MessagePort` path
  as Layer B's `_toMask` entry — the window_bootstrap stub defines the accessor).
- `WebSocket` — `get bufferedAmount/extensions/protocol/binaryType`,
  `set binaryType` leak.
- `URLSearchParams` — `get size` leaks.
- `WebGLRenderingContext`/`WebGL2RenderingContext` — **constructor** body leaks
  (methods are fine).

---

## 3. New external findings (2026 SOTA detector behavior)

### 3.1 Camoufox's structural advantage (why BO must work harder)

Per the Camoufox wiki (deepwiki query, `daijro/camoufox`): Camoufox spoofs at
the **C++/Firefox-core layer** — `navigator` props, `nsScreen`, the
`ClientWebGLContext` parameters are all patched below the JS boundary. The JS
engine **never sees a non-native function**, so `Function.prototype.toString`,
`getOwnPropertyNames`, descriptor checks, and toString-of-toString recursion all
pass *for free*. There is no JS shim to detect.

**Implication for BO:** BO's every Web API is a JS class/function, so masking is
a perpetual arms race — *any* JS-defined surface that escapes all three sweeps is
a tell. This is the single biggest architectural reason BO trails v150 on
holistic detectors (Kasada/DataDome ML). The mitigation is twofold:
(a) make the universal sweep **exhaustive** (cover accessors + constructors), and
(b) add a **committed golden audit test** so regressions can't reintroduce
leaks (doc 16 §5 / §5 below). BO cannot match Camoufox's "free" integrity, but it
can reach **zero JS-source leaks**, which is the only thing the detectors
actually measure.

### 3.2 CreepJS lie-detector probe set (the precise bar to clear)

Per the CreepJS source (deepwiki `abrahamjuliot/creepjs`,
`src/lies/index.ts` `queryLies` / `getLies`), each probed native function is run
through the following lie tests — **this is the exact spec our masking must
satisfy**:

- **`getToStringLie`** — `String(fn)` AND `String(fn.toString)` must both match
  the native-code pattern (covers the toString-of-toString recursion). BO passes
  this via the tagged patched toString (`stealth_bootstrap.js:41`).
- **`getOwnPropertyNamesLie`** — `Object.getOwnPropertyNames(fn)` must be exactly
  `['length','name']` (plus `prototype` for constructors). BO's `_maskFunction`
  is **correct here**: it sets `name` (own, configurable) and `_nativeTag` (a
  **Symbol**, invisible to `getOwnPropertyNames`), and installs **no own
  `toString`** — so the own-name set stays clean. (This was a real prior FP, now
  fixed; see `stealth_bootstrap.js:62-75`.)
- **`getOwnPropertyLie` / `getDescriptorLie`** — `arguments`, `caller`,
  `prototype`, `toString` must not be **own** props (with the constructor
  `prototype` exception). The `_natMethod` shorthand-replacement in
  `cleanup_bootstrap.js:421-437` is precisely the fix for the
  `setTimeout`-has-own-`prototype` case CreepJS catches.
- **`getPrototypeInFunctionLie`** — a non-constructor function must not have an
  own `prototype`. Again handled for the global-fn set by `_natMethod`; **NOT
  systematically enforced** for every masked method (low risk — masked methods
  are method-shorthand or arrow-derived and usually lack `prototype`, but worth
  asserting in the golden).
- **`getIncompatibleProxyTypeErrorLie` / `getToStringIncompatibleProxyTypeErrorLie`**
  — accessing `fn.arguments` / `fn.caller` must throw a `TypeError` with the
  Chrome-shaped message; `fn.toString.arguments` likewise. This is a **proxy
  detector**: it's why BO must avoid wrapping masked functions in `Proxy`
  (BO uses tag-based masking, not proxies — good). But note `Element.attributes`
  **is** a `Proxy` (§0 item 3) and would be caught by the object-level variant.
- **`getNewObjectToStringTypeErrorLie`** — `Function.prototype.toString.call({})`
  must throw with a Chrome-exact stack (no `at Proxy.`, no `_inPatchedToStr`).
  BO's existing `check_tostring_audit_full` test
  (`chrome_compat.rs:4914-4936`) already asserts the stack is clean of
  `_inPatchedToStr`/`_nativeTag`/`_origFnToStr`/`_patchedFnToStr` — keep it.

CreepJS does **not** specifically target `HTMLFormElement.elements`, `Event`
constructors, or `NamedNodeMap` for *lie* detection (deepwiki confirmed), but it
**does** iterate `Element`, `Document`, `HTMLCanvasElement`, `AnalyserNode`,
`Date` prototypes through `searchLies` — so any accessor leak on `Element`/
`Document` (none currently — Layer B covers them) would be caught, and the
fetch-trio/WebGL/stream leaks are caught by holistic vendors that mirror this
exact `getToStringLie` logic.

### 3.3 rebrowser / patchright

`rebrowser-patches` (deepwiki) focuses on the CDP `Runtime.Enable` leak,
`sourceURL`, and utility-world naming — not JS-source masking (BO is not
CDP-driven, so these are N/A). The takeaway is orthogonal: BO's snapshot script
name is already `<anonymous>` (`snapshot.rs:97`), which is the BO-equivalent of
the `sourceURL` fix and is correctly handled.

### 3.4 WHATWG / Chrome ground truth for the collection shapes

- `Element.attributes` is a **live `NamedNodeMap`**;
  `Object.prototype.toString.call(el.attributes) === "[object NamedNodeMap]"` in
  Chrome (verified via MDN/spec + the jsdom issue thread where the
  `[object Object]` deviation was treated as a bug). BO returns
  `"[object Array]"` — a hard tell.
  Sources: [MDN Element.attributes](https://developer.mozilla.org/en-US/docs/Web/API/Element/attributes),
  [jsdom#853](https://github.com/jsdom/jsdom/issues/853),
  [Chrome DOM-attributes-on-prototype-chain](https://developers.google.com/web/updates/2015/04/DOM-attributes-now-on-the-prototype-chain).
- `getElementsByTagName`/`getElementsByClassName`/`Element.children` return
  **live `HTMLCollection`**; `document.querySelectorAll` returns a **static
  `NodeList`**. BO returns a snapshot `NodeList` for all of them and never
  defines `HTMLCollection`.
  Source: [MDN HTMLCollection](https://developer.mozilla.org/en-US/docs/Web/API/HTMLCollection),
  [WHATWG DOM §4.2.10.2 live collections](https://dom.spec.whatwg.org/#concept-collection).

---

## 4. Concrete BO code-level analysis

### 4.1 Accessor leak (bug #1) — `cleanup_bootstrap.js:529-534`

The universal sweep's collection loop drops accessors. **Fix:** also push
`get`/`set` and pass distinct names to `_maskAsNative` (which already names them
`get x`/`set x`). Minimal patch:

```js
for (const _n of _ns) {
    if (_n === 'constructor') continue;
    let _d; try { _d = Object.getOwnPropertyDescriptor(_p, _n); } catch { continue; }
    if (_d && typeof _d.value === 'function') _methods.push(_n);
    // NEW: accessors leak source too (Request.signal, Response.status, …)
    if (_d && (typeof _d.get === 'function' || typeof _d.set === 'function')) _methods.push(_n);
}
```

`_maskAsNative(_p, name)` already resolves the descriptor and masks `desc.get`
as `get name` and `desc.set` as `set name` (`stealth_bootstrap.js:94-95`), so no
further change is needed. Idempotent on V8 natives. This single 2-line change
clears every accessor in §0 item 1.

### 4.2 WebGL constructor leak (bug #2) — `cleanup_bootstrap.js:463-491`

`WebGLRenderingContext`/`WebGL2RenderingContext` are absent from `_sfcNames`.
The universal sweep masks **prototype methods** (`_v.prototype` walk), never the
**constructor object `_v`** itself. **Fix (either):**
- add `'WebGLRenderingContext','WebGL2RenderingContext'` to `_sfcNames`
  (`cleanup_bootstrap.js:463`); **or**
- in the universal sweep, also `_mask(_v, _gname)` the constructor (broader —
  fixes any future ctor leak; but must guard against the de-alias names and
  legacy webkit aliases that `_sfcNames` handles specially). Prefer adding to
  `_sfcNames` for a surgical, low-risk change, plus a general
  `_maskFunction(_v, _gname)` in the universal loop guarded by "name matches the
  global key" to catch future regressions.

Note `canvas_bootstrap.js:1361-1369` already masks the CRC2D prototype methods
by hand and `getParameter` is covered by the universal sweep — so only the two
constructor objects are outstanding.

### 4.3 `Element.attributes` → real `NamedNodeMap` (bug #3) — `dom_bootstrap.js:877-914`

Today `get attributes()` returns `new Proxy([], handler)` — hence
`[object Array]` and `constructor === null`. `NamedNodeMap` is in Layer B's
`_toMask` (`dom_bootstrap.js:3099`) but **no instance is ever created from it**.
**Fix:** define a real `NamedNodeMap` class (or reuse the existing global stub)
with `length`, `item`, `getNamedItem`, `setNamedItem`, `removeNamedItem`,
`getNamedItemNS`, numeric index access, `Symbol.iterator`, and
`Symbol.toStringTag = "NamedNodeMap"`; back it with the existing
`op_dom_get_attribute_names`/`op_dom_get_attribute` ops; return an instance from
`get attributes()`. Keep it live (re-query ops on each access — already the
pattern). This removes the `[object Array]` tell and gives
`el.attributes instanceof NamedNodeMap === true`. ~1 day (the ops already exist;
it is a JS-class wiring job + a `_tag(NamedNodeMap, "NamedNodeMap")` call near
`dom_bootstrap.js:1773`).

### 4.4 Event-subclass name collapse (bug #4) — `event_bootstrap.js` + masking layers

`String(MouseEvent)` returns `function Event() { [native code] }` and
`MouseEvent.name === "Event"`. The subclasses (`class MouseEvent extends
UIEvent`, etc.) are reported as native (good) but carry the **wrong name tag**
— almost certainly because they are masked transitively through the base or via
a shared `_maskFunction(Event,'Event')` whose tag is being read up the proto/
mirror chain, while the subclass never gets its own `name`/`_nativeTag`. Real
Chrome: `MouseEvent.name === "MouseEvent"`, `String(MouseEvent) ===
"function MouseEvent() { [native code] }"`. **Fix:** in `event_bootstrap.js`
after the `globalThis.X = X` assignments (`:490-512`), add an explicit
per-class mask loop:

```js
for (const nm of ['Event','CustomEvent','UIEvent','MouseEvent','KeyboardEvent',
    'InputEvent','FocusEvent','PointerEvent','WheelEvent','TouchEvent',
    'MessageEvent','ErrorEvent','ProgressEvent','AnimationEvent','TransitionEvent',
    'ClipboardEvent','PopStateEvent','HashChangeEvent','StorageEvent',
    'PageTransitionEvent','BeforeUnloadEvent','DragEvent',
    'SecurityPolicyViolationEvent']) {
    try { globalThis._maskFunction(globalThis[nm], nm); } catch (_) {}
}
```

`_maskFunction` sets the own `name` AND the `_nativeTag` to the correct
per-subclass name, fixing both the name and the toString. Also add these names
to Layer B `_toMask` so the prototype `desc.value` methods (e.g.
`initMouseEvent`) report native. ~0.5 day. Clears the `sdt` Event surface.

### 4.5 `HTMLCollection` missing + non-live collections (bugs #5, #6)

`dom_bootstrap.js:300` defines `NodeList` (snapshot) and `globalThis.NodeList`;
there is **no `HTMLCollection`**. `getElementsByTagName`/`getElementsByClassName`
(`dom_bootstrap.js:709-712`, `:1417-1420`), `Element.children`, and the
`document.forms/images/links/scripts` getters all return a snapshot `NodeList`.
Two distinct problems:

- **Existence (#5):** `'HTMLCollection' in window` is `false`; CreepJS and most
  vendor "browser shape" audits expect it. **Fix:** define a `HTMLCollection`
  class (array-like with `length`, `item`, `namedItem`, numeric index,
  `Symbol.iterator`, `Symbol.toStringTag="HTMLCollection"`), add to Layer B
  `_toMask` + `_tag`, and return it from `getElementsByTagName`/
  `getElementsByClassName`/`children`/`document.forms` etc. ~1 day.
- **Liveness (#6):** real `HTMLCollection`/live-`NodeList` re-query the DOM on
  every access. BO snapshots. For the 126-corpus this matters far less than the
  shape tells (SPA code rarely depends on liveness mid-frame because BO drains
  to idle), so liveness is a **P3** follow-on, not a blocker. The shape fix (#5)
  is the value.

### 4.6 What is already correct (do not touch)

- `Function.prototype.toString` patch, re-entrancy guard, tagged-toString
  recursion (`stealth_bootstrap.js:25-48`) — verified clean by
  `check_tostring_audit_full`.
- `_maskFunction` own-property hygiene (no own `toString`, Symbol tag invisible
  to `getOwnPropertyNames`) — satisfies CreepJS `getOwnPropertyNamesLie`.
- Console `ofc` masking (`stealth_bootstrap.js:171-181`).
- Non-constructability of global fns via `_natMethod` shorthand replacement
  (`cleanup_bootstrap.js:421-437`).
- `HTMLFormElement.elements` + `HTMLFormControlsCollection` (reddit) —
  `dom_bootstrap.js:1158-1211`, verified `instanceof` true.
- `Symbol.toStringTag` on the ~44 DOM element classes + `NodeList`/`DOMTokenList`
  + WebGL/CRC2D contexts (`dom_bootstrap.js:1722-…`, `canvas_bootstrap.js:1116`).

---

## 5. Acceptance / regression — promote the golden audit

Doc 16 §5 proposed a `native_code_mask_audit` test that dumps every
constructor + prototype member failing the native-code regex. **This pass
effectively ran that audit ad-hoc and produced the §0 list.** The remaining work
is to **commit it as a golden** so the three masking layers can't silently
regress:

- Add `native_code_mask_audit` to `crates/browser/tests/chrome_compat.rs`,
  walking `globalThis` ctors + prototype `value`/`get`/`set`, asserting each
  matches `/^function .+\(\) \{ \[native code\] \}$/` (note: must allow `get `/
  `set ` prefixes and multi-word names — my probe regex `^function \w+\(\)` was
  too strict and produced false "leaks" for correctly-masked accessors; use the
  laxer pattern).
- Also assert: `MouseEvent.name === 'MouseEvent'`,
  `Object.prototype.toString.call(el.attributes) === '[object NamedNodeMap]'`,
  `typeof HTMLCollection === 'function'`,
  `String(WebGLRenderingContext).includes('[native code]')`.
- Commit `/tmp/bo_mask_audit.json` as `…/fixtures/mask_audit_golden.json`,
  empty (or documented-exceptions only) on a release HEAD run.

---

## 6. Ranked fix list (ROI order)

All six are fixable in the **public engine** (no vendor_solvers); none are
per-vendor bypass code — they are Chrome-parity corrections to BO's own API
surface, explicitly in scope per `SCOPE.md`.

### Fix 1 — accessor masking in the universal sweep (bug #1)
- **What:** add `desc.get`/`desc.set` to the collection loop at
  `cleanup_bootstrap.js:529-534` (2 lines).
- **Effort:** 1–2 hours incl. test.
- **Impact:** clears ~15 leaking accessors across `Request`/`Response`/
  `ReadableStream`/`WritableStream`/`MessagePort`/`URLSearchParams`/`WebSocket`.
  Reduces Kasada `sdt`, DataDome fetch-trio score. No single-site flip alone but
  compounds across ~11 vendors.
- **Confidence:** high. **Public engine:** yes.

### Fix 2 — Event-subclass name + toString (bug #4)
- **What:** explicit per-class `_maskFunction` loop in `event_bootstrap.js:512`
  + add the 23 Event names to Layer B `_toMask`.
- **Effort:** 0.5 day.
- **Impact:** fixes `MouseEvent.name`/`String(MouseEvent)` for all 23 subclasses
  — the canonical Kasada `sdt` Event leak (doc 16 §2.3 #1, doc 08 Lever 3).
- **Confidence:** high. **Public engine:** yes.

### Fix 3 — WebGL constructor masking (bug #2)
- **What:** add `'WebGLRenderingContext','WebGL2RenderingContext'` to `_sfcNames`
  (`cleanup_bootstrap.js:463`); optionally mask `_v` generally in the universal
  sweep.
- **Effort:** 1 hour.
- **Impact:** removes the `class WebGLRenderingContext {…}` leak probed by AWS
  WAF (`06_AWS_WAF_SOLVER.md:504`) and CreepJS. Contributes to the AWS-WAF
  cluster (amazon-*/imdb) static-surface cleanliness — though note
  `HANDOFF_2026_05_28b` finds the AWS blocker is primarily the live-nav drain,
  not fingerprint; this is a prerequisite, not the flip.
- **Confidence:** high. **Public engine:** yes.

### Fix 4 — `Element.attributes` real `NamedNodeMap` (bug #3)
- **What:** define a real `NamedNodeMap` class backed by existing attribute ops;
  return an instance from `get attributes()` (`dom_bootstrap.js:877`);
  `_tag`+`_toMask` it.
- **Effort:** 1 day.
- **Impact:** removes `[object Array]`/`constructor===null` tell read by the
  Akamai BMP attribute audit + CreepJS; makes `el.attributes instanceof
  NamedNodeMap` true. Secondary Akamai static-surface improvement.
- **Confidence:** high. **Public engine:** yes.

### Fix 5 — define `HTMLCollection` + return it from live-collection APIs (bug #5)
- **What:** new `HTMLCollection` class; return from `getElementsByTagName`/
  `getElementsByClassName`/`Element.children`/`document.forms` etc.; `_tag` +
  `_toMask`.
- **Effort:** 1 day.
- **Impact:** `typeof HTMLCollection === 'function'` + correct toStringTag for
  the collection objects SPA + vendor shape-probes expect. Broad shape-parity.
- **Confidence:** medium-high (need care so existing `NodeList`-consumers in
  bootstrap code keep working). **Public engine:** yes.

### Fix 6 — commit the `native_code_mask_audit` golden (regression guard)
- **What:** §5 — committed test + golden fixture; lax native-code regex; the
  four extra assertions.
- **Effort:** 0.5 day.
- **Impact:** no site flip; prevents the three masking layers from regressing
  and turns "is the surface clean?" into a CI gate. Highest *durability* ROI.
- **Confidence:** high. **Public engine:** yes.

### Liveness of collections (bug #6) — deferred P3
- **What:** make `HTMLCollection`/`getElements*`-`NodeList` re-query ops on each
  access instead of snapshotting.
- **Effort:** 1–2 days.
- **Impact:** low for the 126-corpus (idle-drain hides most liveness needs).
  Defer until a specific site is shown to depend on it.
- **Confidence:** low (uncertain payoff). **Public engine:** yes.

---

## 7. Files referenced

| File:line | Role |
|---|---|
| `crates/js_runtime/src/js/stealth_bootstrap.js:13` | `_nativeTag` Symbol |
| `stealth_bootstrap.js:25-48` | `Function.prototype.toString` patch (method-shorthand, re-entrant, self-tagged) |
| `stealth_bootstrap.js:51-80` | `_maskFunction` (own-name hygiene; no own toString) |
| `stealth_bootstrap.js:82-104` | `_maskAsNative` (handles get/set/value) |
| `stealth_bootstrap.js:171-181` | console `ofc` mask |
| `dom_bootstrap.js:300-346` | `NodeList` (snapshot, not live) |
| `dom_bootstrap.js:877-914` | `get attributes()` → `Proxy([])` (bug #3) |
| `dom_bootstrap.js:1158-1211` | `HTMLFormElement.elements` (reddit — DONE) |
| `dom_bootstrap.js:1722-1773` | `Symbol.toStringTag` `_tag` sweep (no HTMLCollection/NamedNodeMap-instance/Event) |
| `dom_bootstrap.js:3073-3158` | Layer B curated `_toMask` + `_topLevelFns` |
| `event_bootstrap.js:490-512` | Event subclass exports (no per-class name mask — bug #4) |
| `fetch_bootstrap.js:391-393` | Headers/Request/Response **constructor** masks |
| `canvas_bootstrap.js:1110` | `WebGLRenderingContext` export (ctor unmasked — bug #2) |
| `canvas_bootstrap.js:1361-1369` | CRC2D prototype method masks |
| `cleanup_bootstrap.js:421-437` | `_natMethod` non-constructability fix |
| `cleanup_bootstrap.js:463-499` | Layer C `_sfcNames` ctor masks (no WebGL — bug #2) |
| `cleanup_bootstrap.js:516-540` | Universal prototype sweep (value-only — bug #1) |
| `crates/browser/tests/chrome_compat.rs:16` | `check()` probe harness |
| `crates/browser/tests/chrome_compat.rs:4834-4953` | `check_tostring_audit_full` (keep + extend) |

## 8. Sources

- CreepJS lie-detection set: deepwiki `abrahamjuliot/creepjs` (`src/lies/index.ts`,
  `getToStringLie`/`getOwnPropertyNamesLie`/`getDescriptorLie`/
  `getIncompatibleProxyTypeErrorLie`/`getNewObjectToStringTypeErrorLie`).
- Camoufox C++-level spoofing (no JS shim → toString native for free):
  deepwiki `daijro/camoufox` (Fingerprinting & Privacy).
- rebrowser scope (CDP-focused, orthogonal): deepwiki `rebrowser/rebrowser-patches`.
- [MDN Element.attributes (NamedNodeMap)](https://developer.mozilla.org/en-US/docs/Web/API/Element/attributes)
- [MDN HTMLCollection](https://developer.mozilla.org/en-US/docs/Web/API/HTMLCollection)
- [WHATWG DOM — live collections](https://dom.spec.whatwg.org/#concept-collection)
- [jsdom#853 — Element.attributes should be NamedNodeMap](https://github.com/jsdom/jsdom/issues/853)
- [Chrome — DOM attributes on the prototype chain](https://developers.google.com/web/updates/2015/04/DOM-attributes-now-on-the-prototype-chain)
- Repo: `16_STEALTH_FINGERPRINT_AUDIT.md`, `17_WEB_API_PARITY_MATRIX.md`,
  `08_KASADA_FRONTIER.md`, `06_AWS_WAF_SOLVER.md`, `FAILED_SITES_ANALYSIS.md:215`,
  `HANDOFF_2026_05_28b.md`.
