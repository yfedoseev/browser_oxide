# VENDOR — Kasada (canadagoose / hyatt / realtor)

**Date:** 2026-05-28
**Author:** research agent (Kasada frontier deep-dive)
**Scope sites (126-corpus):** canadagoose.com, hyatt.com, realtor.com
**Status of these sites:** OPEN-SOURCE FRONTIER. No public stealth engine passes all
three with zero interaction — **including Camoufox v150**, which clears only 4/5 of the
`chl-known` Kasada set. These three are the residual.

> **Reading note.** This document supersedes the scattered Kasada memory notes and
> consolidates with `docs/releases/v0.1.0-parity/08_KASADA_FRONTIER.md`. It re-verifies
> the doc-08 claims against *current* source (most of doc-08's "levers" are now SHIPPED),
> adds 2026 external intelligence, and ends with an HONEST assessment of whether any
> engine-addressable lever remains vs. months of `vendor_solvers` VM work.

---

## 1. What the repo already concluded (cited + re-verified against current source)

### 1.1 The decisive measurement: it is engine fidelity, not IP / behaviour / farm
`08_KASADA_FRONTIER.md` §"Critical correction" and
`memory/state_2026_05_16_kasada_engine_gap_sharpened.md`:

> `nocdp.sh` real Chrome 147 — opens URL, waits, **zero** mouse/scroll/keyboard,
> **this datacenter IP** — **passes all three**. BO: same IP, same zero interaction →
> Kasada 429 / `bot1225.b:1`.

This rules out IP reputation, behavioural absence, and paid-farm requirement. What remains
is a **passive, static engine-vs-real-Chrome surface divergence** measured by `ips.js`.
This is the load-bearing thesis and it is still the right frame (see §4 for the sharpened
2026 version of *why*).

### 1.2 The `/tl` wrapper is cracked
`memory/kasada_wrapper_cracked_and_remaining_leaks.md`:
```
POST_BODY = base64(json({"data": base64(xor(plaintext, b"omgtopkek"))}))
```
9-byte repeating XOR, deployment-wide constant. (TEA-CBC lives inside the bytecode VM
at `kasada_function_bodies.js:129` for the *primary* `/tl` sensor, not the error reports.)
Both halves of the field-diff ("K2-DIFF") already exist as private captures
(`~/projects/browser_oxide_internal/ab_harness/tl/`).

### 1.3 The named error-bearing fields (16, from the decoded error report)
- **`bot1225`/`csc`/`kl`/`dpv`/`smc`** — single biggest trust driver. Root:
  `TypeError: Cannot read properties of undefined (reading 'unjzomuyb…')` — an unimplemented
  Web API surface the VM probes.
- **`sfc`/`sdt`/`wse`/`bfe`** — `Function.prototype.toString` leaking BO's literal JS source
  (incl. deno_core op names like `op_dom_attach_shadow`).
- **`nppm`/`fsc`/`npc`/`ao`/`cbf`** — error-message text parity for `structuredClone`,
  `class X extends <non-constructible>`, spread-non-iterable, etc.
- **`esd`** — leaked private helper name (`_loadGpuProfile`) in error stacks.

### 1.4 Doc-08's "four levers" — STATUS RE-VERIFIED (most are now DONE)
This is the most important update: doc-08 framed four open levers. Against current source,
three are **shipped**, which materially changes the ranked list at the end of this doc.

| Lever (doc-08) | Doc-08 status | **Current source state (verified this session)** |
|---|---|---|
| **L2 — CSS calc math** | "partially shipped" | **SHIPPED.** `CalcExpr` at `crates/css_values/src/types/length.rs:47` now has `Sin/Cos/Tan/Asin/Acos/Atan/Atan2/Pow/Sqrt/Hypot/Log/Round/Mod/Rem`; parser at `crates/css_values/src/calc.rs:101` + `:373` (`pi`/`e` constants); evaluator at `length.rs:304-313`; unit tests `length.rs:374-452`. The 1283-byte calc-precision probe is gone from the captured blob set. |
| **L3 — `_maskAsNative` sweep** | "open, sweep needed" | **SHIPPED + hardened beyond doc-08.** `_maskAsNative` applied across 11 bootstrap files (window 42×, shared_apis 22×, canvas/stealth 9× each, dom 6×, …). Critically, `Function.prototype.toString` is now a **genuine Rust-side V8 callback** — `crates/js_runtime/src/native_fns.rs:141 fp_to_string_cb` — that emits `function <tag>() { [native code] }` for tagged host fns and *delegates to the captured genuine builtin* otherwise (`native_fns.rs:8-22` header, `:234`). This defeats the `class K extends Function.prototype.toString{}` (`fsc`) escape that a pure-JS `.toString` override cannot. |
| **L4 — `bot1225` 28-char API stub** | "open" | **Still open** (needs the private string-table decode to name the API). |
| **L1 — K2-DIFF** | "highest leverage, scoped next step" | **Still the right tool, not yet run end-to-end** as a live BO `/tl` capture + field-diff. The in-VM plaintext-sensor dump tool was specced (`state_2026_05_17_unblock_execution`) but the live diff has not been closed. |

Plus the K1 confound is fixed: there is **no Rust-side `compute_cd` / `x-kpsdk-cd` PoW**
anymore (`crates/stealth/src/kasada.rs` no longer exists). BO now relies entirely on the
bundle **self-solving in V8** and harvests `x-kpsdk-*` from `__fetchLog`
(`crates/browser/src/page.rs:2660-2696`), forwarding them on the post-PoW jittered refetch
(`page.rs:2714-2722`, 250 ms + jitter to dodge Kasada's per-IP rate limiter).

**Net of §1.4:** the three "easy" levers doc-08 listed are largely done. The remaining
gap is therefore *not* a backlog of known small fixes — it is the **long tail the
K2-DIFF was meant to enumerate**, which §4 argues is structurally hard for a from-scratch
V8 engine.

---

## 2. New external findings (2026) — how Kasada works now

### 2.1 The VM is a register/bytecode interpreter, not readable JS
From the nullpt.rs Nike devirtualization series (Nike, canadagoose, hyatt, realtor all run
the same Kasada `ips.js`/`p.js` stack) and the `umasii/ips-disassembler` project:

- `ips.js` is an IIFE that base62-decodes a **~386 KB bytecode string** with a constant
  alphabet, splices out a string table, and boots the VM via `eEA()`.
- The interpreter is a flat dispatch loop —
  `for (;;){ var n = g[r[t.g[0]++]]; if (n===null) break; try { n(t) } catch(e){ O(t,e) } }`
  — ~60 opcodes in array `g`, instruction pointer `t.g[0]`, register file `t.g`, helpers
  `m()` write / `M()` read. Opcodes are atomic (`+ - * / %`, loads, calls).
- Strings are length-prefixed and decoded per-char via
  `String.fromCharCode((4294967232 & l) | ((39*l) & 63))`.
- Obfuscation: virtualization + control-flow flattening + just-in-time string decryption +
  dead code. **Rotated frequently** (quarterly + on detection). Emulating the VM out of
  a browser "breaks within days" (Scrapfly 2026).

Sources: [nullpt.rs devirtualizing Nike VM](https://nullpt.rs/devirtualizing-nike-vm-1),
[umasii/ips-disassembler](https://github.com/umasii/ips-disassembler),
[OPCODES Kasada VM RE](https://opcodes.fr/publications/2021-08/kasada-javascript-vm-obfuscation-reverse-part1).

### 2.2 The token system (2026)
- `x-kpsdk-ct` — **client token**: telemetry + fingerprint, the expensive proof-of-work
  output, session-scoped.
- `x-kpsdk-cd` — **client data**: the cheap per-request PoW solution / hash-collision
  answer (real browsers spend ~2 ms).
- `x-kpsdk-h` — cryptographic signature preventing header tampering.
- `x-kpsdk-r` — request-id / anti-replay.
- `x-kpsdk-v` — version pin.

Sources: [2captcha Kasada deep dive](https://2captcha.com/h/kasada-bypass),
[ScrapeBadger Kasada bypass](https://scrapebadger.com/kasada-bypass).

### 2.3 The trust model is **weighted, and connection-layer signals dominate JS**
This is the single most important NEW finding and it reframes BO's strategy. Scrapfly's
2026 write-up states the JS-fingerprint stage is explicitly **lower-weight** than the
connection layer:

> "[The JS] stage is not a reliable method and results aren't taken [seriously] by
> firewalls … strong performance on TLS/IP/HTTP can succeed regardless of JavaScript
> stage score."

Detection categories, in Kasada's weighting order:
1. **TLS fingerprint** (JA3/JA4: cipher list, extensions, versions).
2. **IP reputation** (residential/mobile +, datacenter −).
3. **HTTP details** (HTTP/2 vs /1.1, header order/casing, H2 SETTINGS frame, HTTP/3).
4. **JS environment** (WebGL, hardwareConcurrency, headless markers) — *advisory*.
5. **Behaviour** (cross-session pattern learning).

Source: [Scrapfly: How to Bypass Kasada 2026](https://scrapfly.io/blog/posts/how-to-bypass-kasada-anti-scraping-waf).

**Implication for the nocdp paradox.** Real Chrome 147 passes from this datacenter IP with
zero interaction (§1.1). If the trust model were IP-dominant, real Chrome would also be
penalized. Since it isn't, the discriminating bit on *this IP* is whichever of TLS/HTTP/JS
diverges between BO's V8 stack and real Chrome. BO's TLS/H2/H3 is byte-verified Chrome
(see §3.1) — so by elimination the residual is the JS-environment fidelity of a
**from-scratch V8 engine**, which is exactly the layer §2.4 says Camoufox never has to fake.

### 2.4 Why Camoufox passes 4/5 and BO/from-scratch engines are harder
DeepWiki on `daijro/camoufox`: Camoufox does **not** spoof in JavaScript. It injects
fingerprints at the **C++ level inside a real Gecko/Firefox engine**
(`nsGlobalWindowInner`, `nsScreen`, `ClientWebGLContext`) and runs the Kasada challenge JS
in **genuine SpiderMonkey**. Therefore:
- `Function.prototype.toString`, property descriptors, error text, stack format,
  `Math`/ICU precision, and every V8/SpiderMonkey-engine quirk are **authentically native**
  — Camoufox has *nothing to reconstruct*.
- Its only Kasada risk is fingerprint-value *inconsistency* (it explicitly warns about
  rotation inconsistencies), not engine-surface authenticity.

BO is the opposite: it reconstructs the *entire* JS surface on top of `deno_core`'s V8.
Every Web API, every error string, every `[native code]` masking, every host-function
identity is hand-built. Kasada's VM is purpose-built to find exactly these reconstruction
seams. **This is why these three sites are the frontier: Camoufox sidesteps the hard
problem by being a real browser; BO must out-fidelity a real browser from scratch.**

Source: [DeepWiki daijro/camoufox](https://deepwiki.com/daijro/camoufox).

---

## 3. BO code-level analysis

### 3.1 Connection layer — STRONG, not the gap
- TLS: `crates/net/src/tls.rs` builds a byte-identical Chrome 147 ClientHello — cipher
  order (`:59`), extension order (`:163-217`), GREASE (`:298`), ECH GREASE + ALPS H2
  SETTINGS (`:372-416`), self-verifying JA4 drift guard (`:465`). JA3/JA4 are Chrome-real.
- ALPN advertises `h2` + `http/1.1` (`tls.rs:185-186`); H2 SETTINGS replayed via ALPS.
- HTTP/3: full QUIC stack present (`crates/net/src/quic.rs`, `h3_request.rs`,
  `alt_svc.rs`; deps `h3 0.0.8` + `h3-quinn 0.0.10`).
- **Old ALPN H2→1.1 downgrade concern** (HANDOFF_2026_05_08) appears resolved — ALPN
  offers both and the stack negotiates H2. Worth one live re-check on canadagoose
  specifically (it historically downgraded), but this is not the load-bearing gap.

**Conclusion:** the layers Kasada weights *highest* are BO's strongest. The gap is in the
layer Kasada weights lowest — JS env — which means the divergence must be a **hard,
binary tell** (a probe that throws / returns the wrong type), not a soft score nudge.
That is consistent with the `bot1225.b:1` hard-fail signature.

### 3.2 JS layer — the real exposure surface
- **Function.toString masking** is now genuinely hardened (`native_fns.rs:141`), closing
  the `sfc`/`sdt`/`fsc`/`wse`/`bfe` class — IF the sweep is complete. The residual risk is
  **coverage**: any host function added since the sweep that isn't tagged leaks its JS
  body. A regression test that enumerates *every* exposed prototype method and asserts
  `[native code]` is the cheap guard (see fix K-2).
- **CSS calc** (`length.rs`, `calc.rs`) closed the calc-precision probe.
- **`bot1225` undefined-receiver** (`unjzomuyb…`) is still open — a missing Web API. This
  is the single biggest trust driver per §1.3 and needs the private string-table decode to
  name. **Public-engine fixable once named** (stub the API in the right `*_bootstrap.js`).

### 3.3 The structural ceiling: deno_core V8 vs Chrome 148 V8
`crates/js_runtime/Cargo.toml:27` pins `deno_core = "0.311"`, while the active profile
claims `Chrome/148.0.0.0` (`crates/stealth/profiles/chrome_148_macos.yaml:16`). deno_core
0.311's bundled V8 is **not** Chrome 148's V8 build. Kasada's VM can probe build-specific
behaviour BO cannot patch from JS:
- Error message *text* and `Error.prototype.stack` *format* (frame layout, anonymous-class
  repr `#<C>`, the `npc`/`fsc`/`ao` fields).
- `Function.prototype.toString` of **true** intrinsics (V8 caches source positions; subtle
  differences across V8 builds).
- `Math`/`Intl`/ICU precision and `Number.prototype.toFixed`/`toPrecision` rounding edges.
- TypedArray/`structuredClone`/`DataView` corner-case throw text (`nppm`).
- Microtask/timer ordering and `performance.now()` resolution + clamping.

These are the fields the captured error report already flagged. Some are JS-patchable
(throw-text rewriting), but **bit-exact parity with a moving Chrome V8 from a pinned
deno_core V8 is a treadmill**, and the masking patches themselves become detectable if
imperfect. This is the honest ceiling.

---

## 4. HONEST assessment — is there an engine-addressable lever?

**Yes, but it is narrow, and the high-leverage levers doc-08 listed are already spent.**

1. **It is genuinely engine-addressable, not IP/behaviour** (nocdp anchor, §1.1). The
   user's instinct is correct: do not write these off as "need residential proxy / farm."
2. **But the cheap wins are done** (§1.4): CSS calc, `_maskAsNative`, genuine-native
   `FP.toString`, K1 PoW-defer. Doc-08's framing of "5-15 small fixes" overstated what
   remains — three of its four levers are shipped.
3. **The remaining JS-env tail is a from-scratch-V8 fidelity problem** (§3.3) that Camoufox
   structurally avoids by being real Gecko (§2.4). Closing it bit-by-bit via K2-DIFF is
   *possible* but is a treadmill against a rotating VM on a pinned V8 — weeks, not days,
   with no guarantee a single fix flips a site (the `bot1225` field may be one of several
   hard tells, and Kasada's weighting means you must zero the JS hard-fails *and* the score
   must clear).
4. **`vendor_solvers` VM emulation is NOT the path either** — CLAUDE.md forbids per-vendor
   bypass in public crates, and external intel (§2.1) says out-of-browser VM emulation
   "breaks within days" on rotation. A native-token solver is months of private work with
   a short half-life.

**The one disciplined, bounded experiment that is still worth running before declaring the
frontier closed: K2-DIFF (fix K-1 below).** It converts "unknown long tail" into a finite,
named list. If K2-DIFF shows ≤3 divergent fields and they're public-engine-stubable → pursue.
If it shows a broad V8-build-quirk spread → declare the frontier a `vendor_solvers`/real-engine
problem and stop spending public-crate time on it. Either way the experiment is the
ROI-maximizing next move because it *bounds the problem*.

> **Expectation setting:** matching Camoufox (BO ties at parity on the easy Kasada cases,
> both fail these 3) is the realistic v0.x outcome. Beating Camoufox *on these three* would
> make BO the open-source SOTA on Kasada — a genuine novel result, not table-stakes. Spend
> bounded effort (K2-DIFF), not open-ended effort.

---

## 5. Ranked fix list (ROI order)

### K-1 — Run K2-DIFF end-to-end (live BO `/tl` capture → field-diff vs real-Chrome)
**What.** Build the in-VM plaintext-sensor dump (intercept `XHR.send`/`fetch` on `/tl`,
capture body PRE-XOR), run hyatt + canadagoose, decrypt with the known `omgtopkek` wrapper,
field-diff vs the captured real-Chrome reference (`internal/ab_harness/tl/*.bin`). Each
divergent field = a named, prioritized bug. **This bounds the entire remaining problem.**
**Effort:** 3–5 days (tool + capture + diff + triage).
**Expected impact:** 0 sites *directly*; produces the finite fix list that gates whether 1–3
Kasada sites are reachable. **Highest ROI because it converts open-ended → bounded.**
**Confidence:** high (both diff halves exist; wrapper cracked).
**Engine:** public (diagnostic only; lives in private internal repo per CLAUDE.md scope for
the decrypted captures).

### K-2 — `[native code]` coverage regression test (lock in the L3 win)
**What.** Enumerate every exposed prototype method across all `*_bootstrap.js` and assert
`String(fn) === "function <name>() { [native code] }"`. Catches any future host fn that
leaks its JS body (the `sfc`/`sdt`/`wse`/`bfe` class regressing). Cheap insurance on an
already-shipped fix.
**Effort:** 1 day.
**Expected impact:** 0 direct flips; prevents regressions that would re-open Kasada (and help
DataDome/Akamai). Hardens parity broadly.
**Confidence:** high.
**Engine:** public.

### K-3 — Name + stub the `bot1225` `unjzomuyb…` API (the biggest trust driver)
**What.** Decode the 28-char identifier via the private string-table decoder, identify the
missing/wrong-signature Web API, stub it in the correct `*_bootstrap.js`. Per §1.3 this is
the single biggest score driver, so it is the most likely *single* lever to move a site.
**Effort:** 2–4 days (decode + identify + implement + re-capture verify). Gated on K-1/decode.
**Expected impact:** potentially flips 1 of {canadagoose, hyatt, realtor} if it's the
dominant hard-fail; uncertain — may be one of several tells.
**Confidence:** medium (high that it's a real fix; low that it alone flips a site).
**Engine:** public (it's a missing API surface, not a bypass).

### K-4 — V8-build error-text / stack-format parity patches (`npc`/`fsc`/`ao`/`nppm`/`cbf`)
**What.** Rewrite throw-text and stack formatting for `class extends non-constructible`,
spread-non-iterable, `structuredClone`, etc. to match Chrome 148's V8 exactly, verified
against a captured Chrome reference.
**Effort:** 1–2 weeks (each field is a separate parity probe; treadmill risk).
**Expected impact:** clears several JS error fields; unlikely to flip a site alone but
necessary to *zero* the JS hard-fail set so the trust score can clear.
**Confidence:** medium (fixable) / low (that it flips a site).
**Engine:** public, but see K-6 — partly bounded by the pinned-V8 ceiling.

### K-5 — Live re-confirm canadagoose ALPN/H2 (cheap connection-layer recheck)
**What.** Since Kasada weights TLS/HTTP highest (§2.3) and canadagoose historically
downgraded H2→1.1, capture one live handshake and confirm BO negotiates H2 (and offers H3
via alt-svc). Rule the connection layer fully out.
**Effort:** 0.5 day.
**Expected impact:** 0 if already H2 (likely); but cheaply removes a high-weight suspect.
**Confidence:** high.
**Engine:** public.

### K-6 — (Frontier, low ROI for public crates) V8-build alignment / vendor_solvers token
**What.** Either align deno_core's V8 to Chrome 148's build for quirk parity (large,
upstream-bound, treadmill) OR implement a native `x-kpsdk-ct/cd` PoW solver in
`vendor_solvers`. Both are months; the VM rotates; out-of-browser emulation "breaks within
days" (§2.1).
**Effort:** weeks–months.
**Expected impact:** could flip all 3 *if* sustained — but high maintenance, short half-life.
**Confidence:** low (ROI), and CLAUDE.md forbids the solver in public crates.
**Engine:** vendor_solvers (token solver) / upstream (V8 alignment). **Recommend NOT pursuing
in public crates; only consider after K-1 proves the tail is small and bit-exact.**

---

## 6. Bottom line

- The Kasada gap is **real and engine-addressable** (not IP/behaviour) — but the cheap
  levers from doc-08 are **already shipped** (CSS calc, native-masking, FP.toString,
  K1 defer). Re-verified against current source.
- The residual is the **JS-environment fidelity of a from-scratch V8 engine**, the exact
  layer Camoufox avoids by running real Gecko. Kasada weights this layer *lowest*, so the
  surviving tells are **hard binary fails** (`bot1225.b:1`), not score nudges.
- **Do K-1 (K2-DIFF) first** — it bounds the problem into a finite named list and decides
  whether 1–3 sites are publicly reachable or whether this is a `vendor_solvers`/real-engine
  problem to stop spending public-crate time on.
- Realistic v0.x outcome: **parity with Camoufox** (both fail these 3). Beating it here would
  be a novel SOTA result; pursue only within the bounded K-1→K-3 envelope.

---

## Sources
- `docs/releases/v0.1.0-parity/08_KASADA_FRONTIER.md`
- `memory/kasada_wrapper_cracked_and_remaining_leaks.md`,
  `memory/kasada_real_blocker_css_calc_math.md`,
  `memory/kasada_akamai_real_blocker_2026_04_17.md`,
  `memory/state_2026_05_16_kasada_engine_gap_sharpened.md`
- [Scrapfly — How to Bypass Kasada 2026](https://scrapfly.io/blog/posts/how-to-bypass-kasada-anti-scraping-waf)
- [2captcha — Kasada deep dive](https://2captcha.com/h/kasada-bypass)
- [ScrapeBadger — Kasada / p.js VM & x-kpsdk](https://scrapebadger.com/kasada-bypass)
- [nullpt.rs — Devirtualizing Nike VM (Part 1)](https://nullpt.rs/devirtualizing-nike-vm-1)
- [umasii/ips-disassembler](https://github.com/umasii/ips-disassembler)
- [OPCODES — Kasada JS VM obfuscation RE](https://opcodes.fr/publications/2021-08/kasada-javascript-vm-obfuscation-reverse-part1)
- [lktop/kpsdk](https://github.com/lktop/kpsdk), [nixbro/Kasada-Solver](https://github.com/nixbro/Kasada-Solver)
- [DeepWiki — daijro/camoufox](https://deepwiki.com/daijro/camoufox)
- BO source: `crates/net/src/tls.rs`, `crates/css_values/src/{calc.rs,types/length.rs}`,
  `crates/js_runtime/src/native_fns.rs`, `crates/js_runtime/src/js/*_bootstrap.js`,
  `crates/browser/src/page.rs:2660-2722`, `crates/js_runtime/Cargo.toml:27`,
  `crates/stealth/profiles/chrome_148_macos.yaml:16`
