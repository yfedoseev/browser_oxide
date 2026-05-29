# 01 — KASADA DEEP (canadagoose / hyatt / realtor): the no-CDP frontier

**Date:** 2026-05-29
**Branch context:** `fix/v0.1.0-fix4-canvas-parity`
**Author:** frontier research agent (Kasada cluster deep-dive)
**Mission frame:** challenge the "out of scope / vendor_solvers only" verdict on Kasada and
hunt for the concrete *engine-addressable* path, exploiting BO's structural no-CDP advantage.

> **Reading order / non-duplication.** This doc *extends* and *re-bases* the prior arc; it does
> not repeat it. Read in this order:
> 1. `docs/v0.1.0-parity-workflows/external/VENDOR_kasada.md` — the cracked wrapper, the VM, the
>    trust-weighting model, the Camoufox structural contrast. (Canonical vendor reference.)
> 2. `docs/v0.1.0-parity-workflows/sites/SITE_kasada_cluster.md` — the 2026-05-28 re-assessment
>    (CSS calc DONE, sentinel trace stale, drain hypothesis).
> 3. `docs/releases/v0.1.0-parity/08_KASADA_FRONTIER.md` — the research arc + the 4 levers.
> 4. `docs/vNext/06_R-KASADA-FRONTIER.md` — the "deferred, vendor_solvers" tracker (which this doc
>    argues is **partly wrong about the public-engine surface** — see §3.2, §3.5).
> 5. memory: `kasada_wrapper_cracked_and_remaining_leaks.md`,
>    `kasada_real_blocker_css_calc_math.md`, `state_2026_05_16_kasada_engine_gap_sharpened.md`.
>
> **What is NEW in this doc (verified against current source 2026-05-29):**
> - The full reconstructed Kasada flow (ips.js bootstrap → VM → `/tl` sensor → token negotiation)
>   in one place, mapped to the exact BO code paths that handle each stage.
> - A **correction to the SITE_kasada_cluster "FIX-K1 drain" claim**: the path Kasada sites
>   actually hit (cold `navigate`) **already drains 45 s** (`page.rs:1952`), explicitly built for
>   "Kasada KPSDK takes 30+ seconds" (`page.rs:2097`). The 50 ms/500 ms drain is the *warm* path,
>   which by its own doc-comment (`page.rs:1415-1421`) does **not** handle anti-bot pages. So the
>   drain is **not** the load-bearing Kasada lever (it *is* for the AWS warm-shell case).
> - A concrete, inspectable **child-realm population gap** (`dom_ext.rs:1217-1247`): the iframe
>   child global exposes only 7 names + FP.toString — no `document`, `navigator`, constructors,
>   or timers — where real Chrome's `iframe.contentWindow` exposes the full surface. This is a
>   *binary tell* if Kasada walks `contentWindow.<API>`.
> - The honest re-classification: **the no-CDP advantage is REAL and already clears the layer
>   that sinks Camoufox/Patchright** — but the residual is from-scratch-V8 surface fidelity, and
>   the decisive bounded experiment (K2-DIFF) is *gated on tooling that does not yet exist in the
>   public tree*.

---

## 0. The reconstructed Kasada flow (end-to-end), mapped to BO code

This is the consolidated picture from the cracked findings + 2026 external RE, with each stage
annotated by **which BO code path owns it** and **where BO can diverge from real Chrome**.

```
  ┌─ 1. Stub HTML (740 B canadagoose / 745 B hyatt / 1764-1772 B realtor) ────────────┐
  │    <script src="/ips.js">  +  inline boot                                          │
  │    BO sees this as Kasada-CHL (classify.rs:105-106 → _kpsdk / ips.js marker)       │
  └────────────────────────────────────────────────────────────────────────────────────┘
                                   │  GET /ips.js
                                   ▼
  ┌─ 2. ips.js bootstrap (IIFE) ─────────────────────────────────────────────────────┐
  │    base62-decode ~386 KB bytecode (constant alphabet) → splice string table → eEA()│
  │    String decrypt per-char:  fromCharCode((4294967232 & l) | ((39*l) & 63))        │
  │    BO owns: running this IN V8 (deno_core 0.311 V8, NOT Chrome 148 V8 — see §3.4)  │
  └────────────────────────────────────────────────────────────────────────────────────┘
                                   │  boots
                                   ▼
  ┌─ 3. The VM (register/bytecode interpreter) ──────────────────────────────────────┐
  │    flat dispatch:  for(;;){ n=g[r[t.g[0]++]]; if(n===null)break; try{n(t)}catch(e){O(t,e)} }
  │    ~60 opcodes, IP=t.g[0], register file=t.g, m()=write M()=read; TEA-CBC inside   │
  │    The VM PROBES the JS env and BUILDS the sensor payload. THIS is the divergence  │
  │    surface (§1). Probes: canvas hash, WebGL vendor/renderer, navigator, screen,    │
  │    plugins, AudioContext fp, CSS calc precision, FP.toString, error text,          │
  │    iframe/realm identity, property descriptors.                                    │
  └────────────────────────────────────────────────────────────────────────────────────┘
                                   │  produces
                                   ▼
  ┌─ 4. /tl sensor POST  (the decisive artifact — the K2-DIFF target) ────────────────┐
  │    PRIMARY sensor body  = TEA-CBC(plaintext) inside the VM (key derived in bytecode)│
  │      → captured real-Chrome ref: hyatt.tl_body.bin (36 KB, ENCRYPTED — see §4.2)   │
  │    ERROR reports         = base64(json({data: base64(xor(plaintext,"omgtopkek"))})) │
  │      9-byte deployment-wide XOR; THIS half is trivially decryptable (the 16 fields) │
  └────────────────────────────────────────────────────────────────────────────────────┘
                                   │  Kasada responds with
                                   ▼
  ┌─ 5. Token negotiation (headers on EVERY subsequent protected request) ────────────┐
  │    x-kpsdk-ct  client token  — telemetry + fingerprint + PoW output, session-scoped │
  │    x-kpsdk-cd  client data   — cheap per-request PoW answer (~2 ms real browser)    │
  │    x-kpsdk-h   signature      — anti-tamper over the headers                         │
  │    x-kpsdk-r   request-id     — anti-replay                                          │
  │    x-kpsdk-v   version pin                                                           │
  │    BO owns: NO Rust-side PoW (crates/stealth/src/kasada.rs deleted). Relies on      │
  │    ips.js SELF-SOLVING in V8, harvests x-kpsdk-* from __fetchLog                    │
  │    (page.rs:2616-2650 harvest; :2677-2679 forward on the retry GET).                │
  └────────────────────────────────────────────────────────────────────────────────────┘
```

**The single load-bearing fact:** if the sensor payload (stage 4) or the self-solve (stage 3→5)
diverges from real Chrome by even one *binary* field, Kasada issues a low-trust / invalid token
and the protected GET returns the interstitial again (`bot1225.b:1`). The whole game is making
stage 4 byte-acceptable.

---

## 1. The EXACT detection mechanism blocking BO

The block is **not** a single named bug; it is whichever fields in the stage-4 `/tl` sensor the
VM (stage 3) computes *differently in BO's reconstructed V8 surface than in real Chrome's*.
From the decrypted **error-report** half (XOR, fully cracked) the named divergence fields are
(memory `kasada_wrapper_cracked_and_remaining_leaks.md`, re-stated with current status):

| Field group | Root cause | Current source status |
|---|---|---|
| `bot1225`/`csc`/`kl`/`dpv`/`smc` | `TypeError: Cannot read properties of undefined (reading 'unjzomuybtbyyhwwkdpkxomylnab')` — VM walks a path that returns `undefined` in BO. **Single biggest trust driver.** | **MUST be re-measured.** The 5-TypeError trace was captured 2026-05-12, **3 days before** the real V8 child realm (`op_create_child_realm`) landed (`456be61`, 05-15). The trace ran against the old Proxy fallback. Likely partly stale — but see §3.2: the child realm is built *nearly empty*, which can re-fire a *different* undefined-walk. |
| `sfc`/`sdt`/`wse`/`bfe` | `Function.prototype.toString` leaking BO's literal JS source (incl. deno op names like `op_dom_attach_shadow`) | **Substantially closed.** `Function.prototype.toString` is now a genuine Rust V8 callback (`native_fns.rs:141 fp_to_string_cb`, emits `function <tag>() { [native code] }` at `:198`/`:223`, delegates to genuine builtin otherwise). `_maskAsNative` applied across **12** bootstrap files (verified `grep -l` 2026-05-29). Residual risk = *coverage completeness* (any un-tagged host fn leaks). |
| `nppm`/`fsc`/`npc`/`ao`/`cbf` | V8-build error-message text + stack-format parity (`structuredClone` throw text, `class X extends <non-constructible>`, spread-non-iterable, anonymous-class `#<C>` repr) | **Open, partly bounded by the pinned-V8 ceiling** (§3.4). JS-patchable throw-text rewrites are possible but a treadmill against a moving Chrome V8 from `deno_core = "0.311"` (`js_runtime/Cargo.toml:27`). |
| `esd` | leaked private helper name `_loadGpuProfile` in error stacks | Open; cheap (rename to a native-looking frame / strip the stack). |
| CSS calc precision (was a 1283-B probe) | `calc(sin(…)/tan(…)+1/pi*…)` precision fingerprint | **DONE/SHIPPED.** `CalcExpr` now has Sin/Cos/Tan/…/Hypot (`css_values/src/types/length.rs:74-85`, eval `:304-313`, tests `:374-452`); parser wires the names (`calc.rs`). The probe blob is gone from the captured set. |

The **primary** sensor half (stage 4, TEA-CBC) is the *real* gate and carries the full fingerprint
inventory (canvas hash, WebGL strings, audio fp, navigator/screen, plugins). We have a real-Chrome
*ciphertext* reference (`hyatt.tl_body.bin`, 36 KB) but it is TEA-CBC-encrypted with a key derived
inside the rotating bytecode — so the **field-level** diff requires capturing BO's *plaintext*
pre-encryption (the K2-DIFF tool, §4) and the *VM internal* TEA key, OR diffing the cracked
error-report half only.

**Net:** the block is a holistic, weighted trust deficit (Scrapfly 2026: JS stage is the
*lowest-weighted* layer behind TLS→IP→HTTP→JS→behaviour). Because BO's TLS/H2/H3 is byte-verified
Chrome (§3.3) and the IP is clean (nocdp passes — §2), the *only* layer that can carry the
discriminating bit on this IP is the JS-env fidelity, which means the surviving tells are **hard
binary fails** (a probe that throws / returns the wrong type), not soft score nudges — consistent
with the `bot1225.b:1` hard-fail signature.

---

## 2. Does a no-CDP real browser pass it? → YES → engine-addressable

**This is the load-bearing evidence and it is freshly re-verified.** The captures still exist:

`~/projects/browser_oxide_internal/ab_harness/nocdp/{canadagoose,hyatt,realtor}.windows.txt`
(verified present + read this session) record real Chrome launched **without CDP** (`nocdp.sh`),
opening each URL, waiting, **zero** mouse/scroll/keyboard, **from this datacenter IP**, and the
top-level window titles are the **real homepages**:

```
canadagoose: "Luxury Performance Outerwear & Clothing | Canada Goose - Google Chrome"
hyatt:       "Hotel Reservations | Book Hotel Rooms Online - Hyatt Hotels and Resorts - Google Chrome"
realtor:     "Realtor.com® | Homes for Sale, Apartments & Houses for Rent - Google Chrome"
```

BO, same IP, same zero interaction → Kasada 429 / `bot1225.b:1`.

**This rules out, decisively:**
- ❌ IP reputation — real Chrome on the *same datacenter IP* passes all three.
- ❌ Behavioural absence — real Chrome with zero behaviour passes (so `humanize.js` / behavior
  wiring is NOT the lever; vNext-06's anti-recommendation against more humanize patterns is right).
- ❌ Paid-farm / residential-proxy requirement — same IP, no farm.

**What remains = a passive, static engine-vs-real-Chrome surface divergence** that ips.js measures
in stage 3 and reports in stage 4. **⇒ This is engine-addressable, full stop.** The user's instinct
is correct; the vNext-06 "deferred, vendor_solvers only, no public-engine work" framing is too
pessimistic about the *diagnosis* (it conflates "holistic ML tail" with "not engine-fixable"). The
honest nuance is in §5: it is engine-addressable but the remaining tail is from-scratch-V8 fidelity,
and we have not yet *bounded* it (K2-DIFF never ran end-to-end).

> **Crucial caveat that updates the nocdp framing:** Camoufox v150 (real Gecko) and Patchright
> (CDP-patched real Chromium) results muddy "even real engines fail." Per
> [daijro/camoufox #318], Camoufox now 429s on canadagoose; per [THE LAB #76], Patchright
> *reportedly still passes* Kasada where vanilla Playwright fails. The reconciliation: the
> discriminating layer is **real-Chromium-runtime fidelity + a good-enough trust posture**, which
> a real Chrome (nocdp) and a real-Chromium Patchright both have, and which Camoufox (Firefox) and
> BO (reconstructed-V8) both lack in different ways. **This sharpens the target: be byte-identical
> to a real Chrome *runtime*, not "add more stubs."**

---

## 3. The concrete engine path (file:line) + how no-CDP helps

### 3.0 The no-CDP advantage — quantified, and it is ALREADY paying off

BO has **no CDP, no juggler, no automation protocol** — it is in-process V8. The CDP-detection
layer that modern Kasada fingerprints (Runtime.enable side effects, CDP timing, `cdc_` vars,
`webdriver`, juggler artifacts) **does not exist for BO to leak.** Concretely:

- Every CDP-based engine (Playwright/Patchright = CDP; Camoufox = juggler) carries an
  automation-protocol surface Kasada can probe. Vanilla Playwright *fails* Kasada precisely here.
- BO clears that entire layer **for free** — there is nothing to detect. This is BO's structural
  edge and it is why the residual is *purely* fingerprint fidelity, not "automation residue."
- **Implication:** if BO can get its stage-4 sensor to parity with a real Chrome, it can plausibly
  pass Kasada *more cleanly than Patchright* (which still carries CDP residue that Kasada tolerates
  today but could tighten on). The no-CDP edge is a durable moat; the fingerprint-fidelity gap is
  the only thing standing between BO and a novel open-source SOTA result here.

This is the thesis to exploit: **BO does not need to out-stealth the automation layer (it has none)
— it only needs to out-fidelity the JS surface, and three of the four historical levers there are
already shipped.**

### 3.1 The drain — CORRECTION to SITE_kasada_cluster FIX-K1 (it is NOT the Kasada lever)

`SITE_kasada_cluster.md` §3.1/§FIX-K1 calls the 50 ms inter-script / 500 ms final drain the
"highest-confidence" Kasada lever, shared with AWS. **Against current source this is mis-aimed for
Kasada** (it remains correct for the AWS *warm-shell* case):

- The 50 ms/500 ms drains are in the **warm path** `navigate_warm`/`build_page_with_scripts_*`
  (`page.rs:1705`, `:1757`). Its own doc-comment (`page.rs:1415-1421`) states it **"does NOT run
  the cookie-diff / pending-nav iteration loop that `Page::navigate` does for anti-bot pages."**
  Kasada sites are not scraped through the warm path.
- The path a Kasada nav actually takes is the **cold** `navigate` → `navigate_with_init_solvers`
  → the iteration loop, whose drain is **`remaining.max(Duration::from_secs(8))`** (`page.rs:2104`)
  under a **45 s host budget** for canadagoose/hyatt (`page.rs:1946-1952`), with the comment at
  `page.rs:2096-2099`: *"Kasada KPSDK takes 30+ seconds … so that heavy PoW challenges can complete
  their /tl POST AFTER the PoW finishes."* The PoW-worker self-solve already has its window.
- So FIX-K1's premise ("the PoW cannot complete in 500 ms") is true only for the warm path, which
  Kasada never uses. **Do not spend Kasada effort on the drain.** (Do ship it for AWS per
  HANDOFF_2026_05_28b §5.1 — that cluster genuinely hits the short warm drain.)

**Action (cheap):** add a one-line assertion/trace to confirm a cold canadagoose nav actually
reaches the 45 s budget and that ips.js's worker spawns + POSTs `/tl` within it (the
`__fetchLog` harvest at `page.rs:2616` will show the `x-kpsdk-*` headers if it did). If the worker
*doesn't* spawn even with 45 s, the gap is the self-solve *throwing* (a stage-3 probe TypeError),
which loops back to §3.2 — not the budget.

### 3.2 The child-realm population gap (NEW, concrete, inspectable) — best single lever

`op_create_child_realm` (`crates/js_runtime/src/extensions/dom_ext.rs:1137-1256`) builds a **real
`v8::Context`** for `iframe.contentWindow` (correct architecture — fixes the old Proxy fallback).
But it populates the child global with **only 7 names + FP.toString** (`dom_ext.rs:1217-1247`,
verified this session):

```
Window, window, self, globalThis, frames, length(=0), opener(=null)
+ install_native_fp_tostring(...)
```

Real Chrome's `about:blank` `iframe.contentWindow` exposes the **entire** global surface —
`document`, `navigator`, every constructor (`HTMLElement`, `XMLHttpRequest`, …), `setTimeout`,
`fetch`, `localStorage`, etc. **If the Kasada VM walks `iframe.contentWindow.<API>`** (a classic
"fresh realm = pristine intrinsics" probe used to defeat patched-prototype stealth) **it finds
`undefined` in BO where Chrome returns a function.** That is *exactly* the shape of the
`bot1225` `Cannot read properties of undefined (reading 'unjzomuybtbyyhwwkdpkxomylnab')` fail —
walking `contentWindow.X.Y` where `X` is undefined.

This is **the most likely live re-incarnation** of the (now-stale) sentinel finding, and it is a
clean, public-engine, inspectable fix:

- **Fix:** in `dom_ext.rs:1217-1247`, after creating the child context, copy the parent global's
  full own-property surface into the child global (constructors, `document`, `navigator`, timers,
  `fetch`, storage) — ideally by re-running the bootstrap against the child context, or by
  mirroring the parent's intrinsics so `contentWindow.<API>` resolves identically. Preserve realm
  distinctness (each must be a *child-realm* object, not the parent's, per Chrome semantics — the
  child's `FP.toString` is already installed realm-distinct at `:1245`).
- **Why it's the best lever:** it is the single most probable cause of the biggest trust driver
  (`bot1225`), it is binary (throws or doesn't), it is squarely public-engine, and it is
  measurable without the full TEA decrypt (the error-report XOR half will show `bot1225`
  disappearing).

### 3.3 Connection layer — confirmed STRONG, not the gap (rule it out cheaply)

`crates/net/src/tls.rs` builds a byte-identical Chrome ClientHello (cipher order, extension order,
GREASE, ECH GREASE + ALPS H2 SETTINGS, self-verifying JA4 drift guard); ALPN offers `h2`+`http/1.1`;
full QUIC/H3 stack present (`crates/net/src/{quic,h3_request,alt_svc}.rs`). Kasada weights TLS/HTTP
*highest* and these are BO's strongest layers. **One cheap live recheck** (canadagoose historically
downgraded H2→1.1): capture one handshake, confirm H2 negotiated + H3 offered via alt-svc. Expected
no-op, but it removes a high-weight suspect for ~0.5 day.

### 3.4 The structural ceiling: `deno_core 0.311` V8 ≠ Chrome 148 V8

`js_runtime/Cargo.toml:27` pins `deno_core = "0.311"`; the active profile claims `Chrome/148`
(`stealth/profiles/chrome_148_macos.yaml:16`). The bundled V8 build differs, so a class of
fields is **not patchable from JS to bit-exactness**: error-message *text*, `Error.prototype.stack`
*format*, `FP.toString` of *true* intrinsics (source-position caching), `Math`/`Intl`/ICU precision,
TypedArray/`structuredClone`/`DataView` throw text, microtask/timer ordering, `performance.now()`
resolution. These are the `npc`/`fsc`/`ao`/`nppm`/`cbf` fields. Some are JS-rewritable (throw text),
but bit-exact parity with a *moving* Chrome V8 from a *pinned* deno_core V8 is a treadmill, and the
masking patches themselves become tells if imperfect. **This is the honest hard ceiling** and the
reason a *guaranteed* full pass may need either V8-build alignment (upstream, large) or a
`vendor_solvers` token path (§5).

### 3.5 `[native code]` coverage — lock in the shipped L3 win

The FP.toString masking is genuinely hardened (Rust callback `native_fns.rs:141`), but coverage
completeness is unverified: any host fn added after the sweep that isn't tagged leaks its body
(`sfc`/`sdt` regression). **Fix:** a test enumerating *every* exposed prototype method across all
12 `*_bootstrap.js` (and the child + worker realms) asserting
`String(fn) === "function <name>() { [native code] }"` and no `op_*` substring. 1 day, parity
hygiene, also helps DataDome/Akamai. Cheap insurance.

---

## 4. The no-CDP-oracle capture + diff validation plan (K2-DIFF)

This is **the** decisive experiment and the one thing that converts the open-ended tail into a
finite named list. It has been specced repeatedly but **never run end-to-end** — and crucially,
**the in-VM plaintext-sensor dump tool does not exist in the public tree** (it is referenced as
"to build" in every prior doc). That is the actual blocker, not analysis.

### 4.1 The oracle (no-CDP real Chrome) — already captured
- `~/projects/browser_oxide_internal/ab_harness/tl/hyatt.tl_body.bin` (36 KB, **TEA-CBC
  ciphertext** of the primary `/tl` sensor) + `.hex`.
- `~/projects/browser_oxide_internal/ab_harness/tl/canadagoose.pcap` (15 MB) + `.keys`
  (TLS keylog) + `canadagoose.tl_bodies.tsv`.
- `~/projects/browser_oxide_internal/ab_harness/nocdp/*.windows.txt` (the pass proof, §2).
- Capture scripts: `ab_harness/tl_capture.sh`, `tl_capture2.sh`, `nocdp.sh`, `nocdp_multi.sh`.

These are the VALID oracle (real Chrome, no CDP). **Never substitute Playwright/Patchright/MCP** —
they are CDP and Kasada detects them, giving a false "real Chrome also fails" reading.

### 4.2 The two-tier diff (the primary sensor is encrypted — this is the real subtlety)
- **Error-report half (XOR, EASY):** `base64(json({data: base64(xor(pt,"omgtopkek"))}))`,
  9-byte deployment-wide key. Decryptor exists:
  `~/projects/browser_oxide_internal/docs/kasada_ips_analysis/scratch/decrypt_report.py`. This is
  where the 16 named fields live (`bot1225` etc.). **Diff this first — it is fully tractable today.**
- **Primary sensor half (TEA-CBC, HARD):** the TEA key is derived *inside* the rotating bytecode
  VM (`kasada_function_bodies.js:129` per memory). To field-diff the *plaintext*, you cannot
  decrypt the captured ciphertext without the per-deployment VM key — instead capture BO's
  **plaintext pre-encryption** by hooking inside the VM, and capture real Chrome's plaintext the
  same way (or accept that the primary-sensor diff requires the in-browser hook on both sides).

### 4.3 The tool to build (the missing public piece)
In-VM plaintext-sensor dump: hook `XMLHttpRequest.send` + `fetch` on any URL containing `/tl`,
and capture the request body **before** it is XOR/TEA-wrapped (the JS side holds plaintext briefly).
Wire it into `crates/js_runtime/src/js/fetch_bootstrap.js` (or a debug-gated bootstrap) to stash
`globalThis.__lastTlPlaintext`, exposed to Rust via an `op` and dumped behind an env flag
(`BROWSER_OXIDE_KASADA_TL_DUMP=1`). Keep the decrypted captures + decoder in the **private**
`browser_oxide_internal` repo per CLAUDE.md scope; the *tool* itself is a diagnostic and can live
public behind the env gate.

### 4.4 The loop
1. Build the dump tool (4.3). Run hyatt + canadagoose through cold `navigate` (45 s budget).
2. Decrypt BO's `/tl` (XOR half via `decrypt_report.py`; primary half = the plaintext we hooked).
3. Field-diff vs the real-Chrome reference. Decode obfuscated identifiers via
   `~/projects/browser_oxide_internal/docs/kasada_ips_analysis/scratch/decode_strings.js`.
4. Each divergent field = a named, prioritized bug. Re-run; each fix should drop a blob/field.
5. **Pass criterion:** no error POSTs + Kasada serves the real `<title>` (matching §2 nocdp).

### 4.5 Re-port the in-tree diagnostics (also missing publicly)
`kasada_vm_dispatcher_trace` + `kasada_error_blob_capture` are referenced in the internal repo but
**absent from the current public `crates/browser/tests/chrome_compat.rs`** (verify + re-port). Run
the dispatcher trace against the **child-realm path** (not the 05-12 Proxy path) and re-count the
`unjzomuybtbyyhwwkdpkxomylnab` TypeErrors — this is how you confirm §3.2 cheaply *before* building
the population fix.

---

## 5. Honest verdict (engine / vendor_solvers / IP-geo)

**Per-site classification (all three, with confidence):**

| Site | Classification | Confidence | Note |
|---|---|---|---|
| canadagoose | **ENGINE-ADDRESSABLE** (public) | high (engine-addressable) / low (single lever flips it) | nocdp passes from this IP; residual = from-scratch-V8 surface fidelity |
| hyatt | **ENGINE-ADDRESSABLE** (public) | high / medium | lowest Kasada tier (Patchright reaches 13228 B loose-L3); best first flip target |
| realtor | **ENGINE-ADDRESSABLE** (public), harder tier | high / low | larger interstitial (1764-1772 B) ⇒ harder deployment; attempt last |

**The verdict in prose:**
- It is **genuinely engine-addressable, NOT IP/geo, NOT behaviour** (§2 nocdp anchor). The
  vNext-06 "vendor_solvers only / no public-engine work" framing **understates the public surface**:
  there is concrete public-engine work (§3.2 child-realm population, §4 K2-DIFF, §3.5 coverage test,
  §3.1 drain-trace ruling-out).
- BUT the **cheap historical levers are already spent** (CSS calc DONE; FP.toString genuine-native;
  `_maskAsNative` across 12 files; K1 Rust-PoW deferred; cold-path 45 s drain). The residual is the
  **JS-environment fidelity of a from-scratch V8**, the exact layer Camoufox sidesteps by being real
  Gecko and that the pinned-`deno_core`-V8 ceiling (§3.4) caps for a class of error-text/precision
  fields.
- **BO's no-CDP advantage is real and already clears the automation-detection layer** that sinks
  vanilla Playwright (and that Camoufox/Patchright only survive by carrying *real-engine* runtimes).
  This is the durable moat: BO competes on *one* axis (fingerprint fidelity), not two.
- **The ROI-correct next move is the bounded K2-DIFF (build the tool → diff → name the list),
  starting with the child-realm population check (§3.2)**, NOT the drain (§3.1, mis-aimed for
  Kasada) and NOT a `vendor_solvers` VM port (which "breaks within days" on rotation per Scrapfly,
  and is forbidden in public crates).
- **`vendor_solvers` (private) classification applies only to the guaranteed-flip path:** a native
  `x-kpsdk-ct/cd` PoW solver or live-VM driver. Out of scope for public crates per CLAUDE.md; high
  maintenance, short half-life; only worth it after K2-DIFF proves the tail is small + bit-exact.

**Realistic outcome:** parity with Camoufox (both fail today) is the floor; **flipping hyatt via
the child-realm population fix + the cleared no-CDP layer is a plausible, novel open-source SOTA
result** if `bot1225` is the dominant tell and resolves to the empty child global. That single
bounded bet (§3.2 → §4.5 verify) is the highest-EV move on the entire frontier.

### Ranked engine fixes (ROI order)
1. **K-A (NEW, top) — Populate the iframe child global** (`dom_ext.rs:1217-1247`) with the full
   parent API surface; re-run the dispatcher trace against the child-realm path to confirm
   `bot1225`/`unjzomuy` resolves. *Public. ~2-4 d. Best single-lever shot at a flip.*
2. **K-B — Build the in-VM `/tl` plaintext dump tool + run K2-DIFF** (§4). *Public tool; private
   captures. ~3-5 d. Bounds the entire remaining problem into a named list.*
3. **K-C — Drain-trace ruling-out** (§3.1): confirm cold canadagoose reaches the 45 s budget +
   worker spawns; correct the SITE doc. *Public. ~0.5 d. Removes a mis-aimed suspect.*
4. **K-D — `[native code]` coverage regression test** across all realms (§3.5). *Public. ~1 d.
   Hygiene; locks in L3; helps DataDome/Akamai.*
5. **K-E — V8-build error-text/stack-format parity** (`npc`/`fsc`/`ao`/`nppm`/`cbf`/`esd`) (§3.4).
   *Public but pinned-V8-bounded. 1-2 wk. Necessary to zero the JS hard-fail set; unlikely to flip
   alone.*
6. **K-F (NOT public) — `vendor_solvers` PoW/VM token path** (§5). *Private. Weeks-months; short
   half-life; only after K-B proves bit-exactness.*

---

## Sources
- BO source (verified 2026-05-29): `crates/browser/src/page.rs:1415-1421` (warm-path scope),
  `:1946-1952` (45 s Kasada host budget), `:2096-2104` (cold-path 8 s+ drain, "Kasada 30+ s"),
  `:2616-2650` (`x-kpsdk-*` harvest), `:2677-2679` (token forward on retry);
  `crates/js_runtime/src/extensions/dom_ext.rs:1137-1256` (child realm; near-empty global at
  `:1217-1247`); `crates/js_runtime/src/native_fns.rs:141` (`fp_to_string_cb`),
  `:198`/`:223` (`[native code]` emission); `crates/css_values/src/types/length.rs:74-85`,`:304-313`
  (CSS calc math DONE); `crates/js_runtime/Cargo.toml:27` (`deno_core = "0.311"`);
  `crates/stealth/profiles/chrome_148_macos.yaml:16`; `crates/browser/src/classify.rs:105-106`.
- Internal captures (private, verified present): `ab_harness/nocdp/*.windows.txt` (the pass proof),
  `ab_harness/tl/hyatt.tl_body.bin` + `canadagoose.pcap/.keys/.tsv`,
  `docs/kasada_ips_analysis/scratch/{decrypt_report.py,decode_strings.js}`.
- Repo docs: `docs/v0.1.0-parity-workflows/external/VENDOR_kasada.md`,
  `docs/v0.1.0-parity-workflows/sites/SITE_kasada_cluster.md`,
  `docs/releases/v0.1.0-parity/08_KASADA_FRONTIER.md`, `docs/vNext/06_R-KASADA-FRONTIER.md`,
  `docs/HANDOFF_2026_05_28b.md` §4-§5.1.
- Memory: `kasada_wrapper_cracked_and_remaining_leaks.md`, `kasada_real_blocker_css_calc_math.md`,
  `state_2026_05_16_kasada_engine_gap_sharpened.md`, `state_2026_05_15_playwright_ab_decisive.md`,
  `proxy_not_the_problem.md`.
- External (2026): [Scrapfly — How to Bypass Kasada](https://scrapfly.io/blog/posts/how-to-bypass-kasada-anti-scraping-waf)
  (trust-weighting; multi-second PoW for fresh DC sessions),
  [2captcha](https://2captcha.com/h/kasada-bypass), [ScrapeBadger](https://scrapebadger.com/kasada-bypass),
  [ZenRows](https://www.zenrows.com/blog/kasada-bypass), [nullpt.rs Nike VM](https://nullpt.rs/devirtualizing-nike-vm-1),
  [umasii/ips-disassembler](https://github.com/umasii/ips-disassembler),
  [OPCODES Kasada VM RE](https://opcodes.fr/publications/2021-08/kasada-javascript-vm-obfuscation-reverse-part1),
  [daijro/camoufox #318](https://github.com/daijro/camoufox/issues/318) (Camoufox 429s canadagoose),
  [THE LAB #76](https://substack.thewebscraping.club/p/bypassing-kasada-2025-open-source) (Patchright passes),
  [DeepWiki daijro/camoufox](https://deepwiki.com/daijro/camoufox) (C++-level fingerprinting, real SpiderMonkey).
