# K2-DIFF RESULT — our Kasada `/tl` sensor, decoded (2026-05-17)

**The decisive Kasada experiment succeeded.** It converts the
"allow-but-blocked / holistic ML tail, no single lever" framing into a
**concrete, enumerated, named divergence set** with a working
decode pipeline. Branch `fix/engine-fp-backlog`.

## Method (what actually worked)

`crates/browser/tests/kasada_tl_capture.rs` (network, `#[ignore]`):
navigate hyatt.com with K1 active (no parallel Rust cd), hook
`TextEncoder.prototype.encode` + fetch/XHR. Decisive observations:

1. **Our engine never POSTs `/tl`.** The only Kasada POSTs are to
   `https://reporting.cdndex.io/error` (~31 KB + 303 B). ips.js hits an
   internal failure during sensor assembly and **diverts to the error
   path** — it never completes the `/tl` handshake. (A raw byte-diff vs
   the captured real-Chrome `hyatt.tl_body.bin` was therefore never the
   right method — corrected.)
2. **The 31 KB error blob decodes cleanly** with the known chain:
   `outer-b64 → JSON .data → b64 → XOR "omgtopkek"` ⇒ **23,793 chars of
   the full plaintext Kasada sensor** (`/tmp/kasada_tl/
   ours_hyatt_sensor_decoded.json`). It maps exactly onto the §6
   taxonomy (`bid:"701d38d"` identical to the doc; `__:` key-list;
   per-probe `{"r":<result>,"b":<flag>}`). **123 fields total.**

## The named divergences (anomalous `r` values vs real Chrome)

**Rigor note on `b`:** 47/123 fields carry `b:1`, but several have
*correct* values (`cnf` shows native `function stringify(){ [native
code] }`; `wgp`="Google Inc. (Apple)"; `ifc` mismatches:0). So `b:1`
is **probe-present/collected, NOT "bot-flagged"** — the load-bearing
signal is the **anomalous `r` content**, not the `b` bit. (Confirming
exact `b` semantics + the `bot1225` aggregate is the one remaining
sub-step; it does not change the named anomalies below.)

Concretely anomalous `r` values our engine emits that a real Chrome
does not — the passive-surface bugs feeding Kasada's score:

| Cluster | Fields | Our `r` (wrong) | Real Chrome |
|---|---|---|---|
| **navigator.webdriver** | `wdt` | `"undefined"` | `false` (defined boolean) |
| **Fn.toString error msg** | `wse` `fsc` `bfe` `npc` `esce` | V8 defaults: `TypeError: Function.prototype.toString requires that 'this' be a Function`; `Class extends value … is not a constructor or null`; `#<C>` | Chrome/Blink's exact (different) strings |
| **`unjzomuy…` VM probe throws** | `smc` `dpv` | `TypeError: Cannot read properties of undefined (reading 'unjzomuybtbyyhwwkdpkxomylnab')` | returns a value (probe resolves) |
| **screen/display = "n/a"** | `dpi` `spd` (+`sos` partial) | `{getter:"n/a",setter:"n/a"}`, all screen dims `"n/a"` | real numeric devicePixelRatio accessor + screen metrics |
| **stack-format leak** | `pev` `dpv` | stacks expose `/[guid]/ips.js?…:5:116` + `eval (%3Canonymous%3E…)` frames | Chrome stack shape (no injected ips.js path frame) |
| **empty/null probes** | `hsp` `tnp` `wbn` `acd` `kbk` `ao` `bas` `loc` `spc` | `null` / `{}` / `[]` / `""` | populated probe results |

The recurring `unjzomuybtbyyhwwkdpkxomylnab` undefined-read (`smc`,
`dpv`) matches the prior `docs/kasada_ips_analysis/
UNJZOMUY_INVESTIGATION_2026_05_12.md` line — now confirmed live in the
decoded sensor as a load-bearing failing VM probe.

## Why this is the decisive outcome

- It **proves the corrected thesis empirically**: the Kasada residual
  is an *engine passive-surface gap*, now an **enumerated named list**
  (webdriver, Fn.toString messages, the unjzomuy VM probe, screen
  "n/a", stack-format), **not** behaviour, **not** IP, **not** a
  holistic mystery.
- It delivers a **reusable decode pipeline** (`kasada_tl_capture.rs`
  → `/tmp/kasada_tl/ours_hyatt_sensor_decoded.json`) so each fix is
  re-measurable against the live sensor.
- It explains the `/tl` non-completion: ips.js throws on probes like
  the `unjzomuy` read and `Fn.toString`-extends, so it never finishes
  the sensor → diverts to `/error` → no `/tl` → server never clears us.

## Next (the fix program, ROI-ordered — each re-checkable via the decode)

1. `wdt`: `navigator.webdriver` → `false` (defined), not `undefined`.
   Smallest, highest-certainty. **✅ DONE — commit `118c0d0`**
   (3 bootstrap definition sites → `false`; 4 tests corrected to the
   Chrome-faithful value; §4 gate green). Live re-verify (decode
   shows `wdt.r`→"false") pending a fresh `kasada_tl_capture` run.
2. `unjzomuy…` (`smc`/`dpv`): find why
   `…reading 'unjzomuybtbyyhwwkdpkxomylnab'` throws in our V8 (a
   Kasada VM-internal property read) — the single most load-bearing
   (it aborts sensor assembly → the `/error` divert).
   **SHARPENED 2026-05-17 (diagnosed, NOT yet fixed — deep VM RE):**
   `smc`+`dpv` throw the *identical* error;
   `unjzomuybtbyyhwwkdpkxomylnab` is Kasada's **per-build sentinel
   tag** (ips.js tags a native via its VM, re-reads `obj.<sentinel>`
   to detect tamper) ⇒ **`obj` itself is `undefined`** at that read.
   `smc.v:false` for `video/mp4` is a **symptom not a bug** —
   `chrome_compat::kasada_smc_isTypeSupported_must_be_true_for_mp4`
   passes 437/0; it reads false in the live sensor only because the
   sentinel read throws & aborts the probe ⇒ **ONE root cause**,
   consistent with UNJZOMUY candidate #3 (`_maskFunction`/
   `_defProtoMethod` recreate wrapper objects per-access ⇒ the object
   ips.js tagged ≠ the one it re-reads ⇒ a deeper access is
   `undefined` ⇒ the sentinel read throws). Not patched this session
   (deep Kasada-VM RE, not a speculative tail-of-marathon change).
   **Precise next RE step:** extend `kasada_tl_capture.rs` with a
   TypeError/stack trap (or wrap the smc/dpv probe entry) recording
   the *receiver + full stack* at the first
   `unjzomuybtbyyhwwkdpkxomylnab` access → names the exact `undefined`
   object, turning fix #2 into a concrete patch. Highest-leverage
   remaining Kasada work.

   **TRAP RESULT 2026-05-17 (`kasada_sentinel_trap.rs`, committed):**
   879 sentinel SET calls captured — **ALL on `Function` objects**,
   set from inside the ips.js VM (`eval at N (…/ips.js…)`). ⇒ ips.js
   tags 879 functions with its per-build sentinel then re-reads them;
   the `TypeError` (receiver `undefined`) ⇒ a **function-valued
   property that yields a valid fn on first access but `undefined` on
   a later access** in the smc/dpv path — empirically confirms
   UNJZOMUY candidate #3 (`_maskFunction`/`_defProtoMethod` recreate
   function objects per access; it is NOT mere tag-loss on a fresh
   wrapper — the receiver itself goes `undefined`). **Patch family =
   identity-stable masked-function wrappers** (memoize per
   (target,name) so `obj.m===obj.m` and ips.js-set props persist).
   **Still needed before patching (one more localization pass):** the
   exact failing property — the engine-thrown `TypeError`'s stack is
   truncated to `at eval`; capture a full-fidelity throw stack (an
   engine stack-capture tweak, not JS) to name the specific accessor,
   else an engine-wide "make all wrappers stable" change is
   speculative + very high regression surface (chrome_compat's
   native-masking/function-identity suite is exactly what it touches).
   ⇒ Fix #2 = a careful own focused effort (localize → targeted patch
   → full §4 gate), NOT a tail-of-session edit. Real progress: from
   "mystery" to a named mechanism + concrete patch family.

   **CODE AUDIT 2026-05-17 (narrowed further):** `_defProtoMethod`
   (window_bootstrap.js:146) installs the masked wrapper as a FIXED
   DATA property (`Object.defineProperty(proto,name,{value:wrapped})`)
   called ONCE at bootstrap ⇒ `obj.m===obj.m` is identity-stable ⇒
   **UNJZOMUY candidate #3 RULED OUT** for _defProtoMethod methods.
   The 879-Function-tag throw is therefore a **getter that returns a
   function then `undefined` on a later access** within the *specific*
   smc/dpv probe path — i.e. UNJZOMUY candidate #1 territory
   (navigator.mediaDevices / MediaSource for `smc`; the devtools-probe
   accessor for `dpv`), or an object-getter that rebuilds. Throw-stack
   localization is blocked (ips.js controls its own Error stack
   formatting → only `at eval`). **Precise next RE step (defined, not
   done):** a *targeted* instrumentation capture wrapping just the
   smc/dpv-relevant native accessors (MediaSource, MediaSource.
   isTypeSupported, navigator.mediaDevices + its methods, the
   devtools-probe globals) to log receiver+return on each access and
   catch which one yields fn-then-`undefined`. Narrower than the
   sentinel trap; that names the exact accessor → minimal targeted
   patch. Status: iterative deep VM RE, advanced again, not complete —
   each session has cut the search space (mystery → smc/dpv → 879
   Function tags → not-_defProtoMethod → getter-in-media/devtools-path).

   **★ 2026-05-17 — SEARCH SPACE EXHAUSTED + ONE REAL DIVERGENCE
   CLOSED (throw NOT yet resolved — honest) ★**
   (A confidently-named *candidate* root cause was patched gate-green
   and Chrome-faithfully, then the offline decode proved it is NOT the
   load-bearing cause — see the RE-VERIFY block. Net: a decisive
   elimination + reusable tooling, not a fix of the throw.)

   New tooling (committed, all `#[ignore]` network):
   `kasada_smc_dpv_trap.rs`, `kasada_eval_probe_trap.rs`,
   `kasada_childrealm_smc_probe.rs`, `kasada_proto_surface_probe.rs`.

   1. **Accessor-recreation DEFINITIVELY RULED OUT.** [MEAS]
      `kasada_smc_dpv_trap` wrapped every smc/dpv-relevant native in
      BOTH realms (MediaSource(.isTypeSupported), MediaRecorder,
      navigator.mediaDevices + methods, Fn.prototype.toString, chrome,
      child-realm equivalents) logging return identity per access:
      **768 reads, ZERO identity flips, ZERO fn→undefined.** UNJZOMUY
      candidates #1 AND #3 closed. The 551/879 sentinel SETs are ALL
      ips.js's OWN VM `Function`s in the MAIN realm (a uid+realm stamp
      proved it); the 400 sentinel GET-"misses" are ALL benign host
      built-ins (`push`,`charCodeAt`,`call`,…) — the *normal*
      `if(l[h]&&…)` short-circuit, exactly Chrome's behaviour.
   2. **Precise failing set (offline decode):** EXACTLY 3 sensor
      fields — `smc`, `dpv`, `esd.cpt` (earlier 5/6 counts were
      JSON-nesting regex artifacts; `npc` is the *class-extends* msg =
      fix #3, not this). [MEAS]
   3. **Disassembled the throw site** (`kasada_eval_probe_trap`
      captured ips.js's `Function()`-built VM handlers). CALL handler
      (record 111): `var o=e(n),l=e(n),_=e(n),h=r[4];
      if(l[h]&&l[h].I===l){…} else n.V[2]=l.apply(o,_)`. The throw is
      **`l[h]` when `l===undefined`** — `h`=sentinel, `l`=the callable
      the probe fetched via the VM value-fetcher `e(n)`. [MECH]
   4. **A strong candidate divergence (later disproven as THE
      cause).** `kasada_childrealm_smc_probe` +
      `kasada_proto_surface_probe`: in an iframe **child realm**,
      `CanvasRenderingContext2D` was **`undefined`** (a function in
      main; ALL 37 ctx2d prototype methods absent on the child realm)
      and `HTMLMediaElement.prototype` near-empty. [CODE]
      `dom_bootstrap.js` `_apisToCopy` (the constructor list copied
      into the child realm; WebIDL ctors are non-enumerable so the
      `Object.keys()` blanket-copy misses them) **omitted the
      canvas/graphics constructor surface**. HYPOTHESIS [HYP]: the
      child-realm `esd.cpt`/`smc`/`dpv` VM path fetches such a
      ctor/method → `undefined` → rec-111 `undefined[sentinel]`. This
      was a real Chrome-divergence worth closing regardless; the
      offline re-verify (below) then **falsified it as the
      load-bearing cause** (throw persists) — a clean elimination.
   5. **PATCH (minimal, Chrome-faithful):** added
      `CanvasRenderingContext2D, HTMLCanvasElement, OffscreenCanvas,
      ImageData, Path2D, ImageBitmap, WebGLRenderingContext,
      WebGL2RenderingContext, DOMMatrix, DOMMatrixReadOnly, DOMPoint,
      DOMRect, DOMRectReadOnly` to `_apisToCopy`
      (`crates/js_runtime/src/js/dom_bootstrap.js`). Each is a genuine
      main-realm global (verified); the copy loop already skips
      `undefined`, so this exposes the real Chrome iframe-realm
      surface — not a stub. One list, the exact missing constructors;
      NOT an engine-wide speculative change.

   **§4 GATE: GREEN** (commit `80818a5`). chrome_compat 437/0,
   holistic 10/0 (ledger byte-equiv), iframe_isolation 5/0 (+1
   ignored fp_e1), v8_inspector_parity 3/0, v8_natives 11/0. No
   chrome_compat test needed correction (the patch is purely
   additive). Only failures anywhere = the 2 pre-existing
   `page::tests` canvas tests (`page.rs` untouched by the patch —
   verified `git diff --stat`; env `getContext` limit; outside the
   gate — the documented acceptable exception).

   **OFFLINE RE-VERIFY: HONEST NEGATIVE (decisive elimination).** [MEAS]
   `kasada_proto_surface_probe` on the patched build confirms the fix
   LANDED — child realm now has `CanvasRenderingContext2D` as a
   function with **all 37 ctx2d prototype methods** (`bothProto=37,
   missing=0`; was `child=undefined, missing=37`); `HTMLCanvasElement`
   fully populated. **BUT** a fresh `kasada_tl_capture` + decode
   (`/tmp/kasada_tl/decoded_POSTPATCH.json` vs
   `decoded_PREPATCH_baseline.json`): `smc`, `dpv`, `esd.cpt` STILL
   carry the IDENTICAL sentinel `TypeError` (FIXED: none; REGRESSED:
   none; `esd.cpt` stack still `at eval (<anonymous>:3:66) ← at
   U(ips.js)`). So the missing child-realm canvas/graphics
   constructor surface was **a real Chrome-divergence (now closed,
   Chrome-faithful, gate-green) but NOT the load-bearing cause** of
   the unjzomuy throw. This is a *decisive elimination*, not a
   regression: it removes a confounder and proves the `undefined`
   does NOT come from a missing child-realm constructor/proto method.

   **What is now RULED OUT for the unjzomuy throw (hard data):**
   (a) UNJZOMUY cand #1/#3 accessor-recreation — 768 reads, 0 flips;
   (b) sentinel tag-loss on ips.js's own VM frames — 551 SETs all
   succeed (main realm, uid-stamped); the 400 GET-"misses" are the
   benign host `if(l[h]&&…)` short-circuit (Chrome-identical);
   (c) child-realm constructor/prototype-surface absence — patched &
   re-verified, throw persists; (d) `smc.v:false` symptom — unchanged
   diagnosis (chrome_compat passes).

   **SHARPENED NEXT STEP (defined, not done).** The throw is
   `l[h]` with `l===undefined` in CALL handler rec 111
   (`var o=e(n),l=e(n),_=e(n),h=r[4]`). `l` is fetched by the VM
   value-fetcher `e(n)` from `n.V` (the VM value array). For EXACTLY
   `smc`/`dpv`/`esd.cpt` (3 of 123; `mrs`/`spd`/`ifw` which also use
   the child realm do NOT throw), `e(n)` yields `undefined` at a
   FIXED column (`<anonymous>:3:66`) ⇒ a structural code-shape /
   VM-bytecode-path divergence, NOT a missing host API (a missing API
   would vary by probe). Next: instrument the VM **value-fetcher
   itself** — wrap/shadow `n.V` element reads (or the `e`/`v` fn
   refs passed into the rec-111 handler) to log, at the first
   `l===undefined`, the preceding `a(n,…)` writes into that VM slot
   for the smc/dpv/cpt invocation — i.e. trace WHY `n.V[k]` is
   `undefined` (which earlier opcode failed to populate it). That is
   the last localization layer; it requires hooking the VM's own
   `e`/`a`/`v` closures (capturable: they are args to the
   `Function()`-built handler bodies `kasada_eval_probe_trap` already
   records) rather than any host accessor — host-accessor space is
   now exhausted.
3. `wse`/`fsc`/`bfe`/`npc`/`esce`: align `Function.prototype.toString`
   / class-extends / structuredClone TypeError messages to Chrome's
   exact strings.
4. `dpi`/`spd`: expose real devicePixelRatio/screen accessors (no
   "n/a").
5. `pev`/`dpv` stack format: don't leak the injected `/[guid]/ips.js`
   path / `eval` frame shape.

Re-run `kasada_tl_capture` after each ⇒ the anomalous `r` flips to a
Chrome-faithful value; when ips.js stops throwing it should complete
`/tl` instead of `/error`. The §4 network-free gate cannot verify the
live flip (Kasada server), but the **decoded-sensor delta is an
offline, deterministic check** for every fix — a far stronger position
than "holistic tail".
