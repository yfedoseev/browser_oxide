# SITE — Kasada cluster (canadagoose / hyatt / realtor)

**Date:** 2026-05-28
**Branch context:** `fix/v0.1.0-fix4-canvas-parity`
**Author:** research agent (Kasada-cluster deep-dive task)
**Status of cluster:** OPEN FRONTIER — no open-source engine passes all three. BO and Camoufox v150 BOTH fail (tie at the frontier).

> Reading order: this doc → `docs/releases/v0.1.0-parity/08_KASADA_FRONTIER.md` (the prior research arc) →
> `docs/HANDOFF_2026_05_28b.md` §4 (the live-nav drain root cause, which this doc argues is *shared* with Kasada) →
> the internal-repo artifacts under `~/projects/browser_oxide_internal/docs/kasada_ips_analysis/`.

---

## 0. TL;DR — the reassessment

Three findings change the prior picture and make this doc more than a recap:

1. **Two of the four prior "levers" are already shipped or obsolete.**
   - **Lever 2 (CSS calc math) is DONE.** `CalcExpr` now carries the full CSS Values 4 math set
     (`crates/css_values/src/types/length.rs:47-95`: Sin/Cos/Tan/Asin/Acos/Atan/Atan2/Pow/Sqrt/Hypot/Log/Exp/Abs/Sign/Mod/Rem/Round + constants) and the parser
     wires every function name (`crates/css_values/src/calc.rs:66-145`). The 1283-byte calc-precision probe that the
     2026-05-10 capture flagged is no longer an open gap.
   - **Lever 4 (the `unjzomuybtbyyhwwkdpkxomylnab` sentinel) was diagnosed against a code path that no longer
     exists.** The VM trace that found the 5 sentinel-loss `TypeError`s was captured **2026-05-12**
     (`~/projects/browser_oxide_internal/docs/kasada_ips_analysis/VM_TRACE_FINDINGS_2026_05_12.md`). The real V8
     child-realm iframe path (`op_create_child_realm`) only landed **2026-05-15** (commit `456be61`, verified via
     `git log -S op_create_child_realm`). The trace therefore ran against the **old Proxy-based fallback realm**
     (`crates/js_runtime/src/js/dom_bootstrap.js:2965-2992`), which returns proxied/derived references where the
     real child realm now returns a genuine, stable V8 object. **The sentinel-loss conclusion must be re-measured;
     it is very likely stale.**

2. **Camoufox v150 ALSO fails canadagoose (Kasada 429).** Confirmed live by
   [daijro/camoufox issue #318](https://github.com/daijro/camoufox/issues/318) (canadagoose started 429-ing Camoufox
   ~mid-2025, same IP that passes in the user's real browser). Camoufox is *Firefox*-based. So the residual signal is
   **not** a Chrome-source-leak that BO uniquely emits — it is a holistic trust-score deficit that hits both engines.
   This reframes the goal: we are not chasing a single BO bug; we are chasing the **last few points of a holistic
   trust score that even the open-source SOTA cannot clear**. Interestingly,
   [Patchright (CDP-patched Chromium) reportedly still passes Kasada](https://substack.thewebscraping.club/p/bypassing-kasada-2025-open-source)
   where vanilla Playwright fails — i.e. a *real-Chromium-shaped* runtime with good IP can still clear it. That points
   the lever toward "be more byte-identical to real Chromium's runtime + complete the PoW", not "add more stubs".

3. **The likely #1 blocker is now the SAME root cause as the AWS-WAF cluster: the live-nav async drain budget.**
   Kasada's `x-kpsdk-ct` token is a **proof-of-work computed inside ips.js's VM** (often spun into a worker), which
   takes a real browser ~2 ms but escalates to *seconds* for fresh/datacenter sessions
   ([Scrapfly 2026](https://scrapfly.io/blog/posts/how-to-bypass-kasada-anti-scraping-waf),
   [ScrapeBadger](https://scrapebadger.com/kasada-bypass)). BO's live navigate path drains only **50 ms between
   scripts and 500 ms at the end** (`crates/browser/src/page.rs:1660-1680` and `:1728-1731`) versus the **8 s** cold
   path (`page.rs:611`). HANDOFF_2026_05_28b §4 already proved this exact 50 ms-vs-multi-second drain gap kills the AWS
   challenge.js self-solve. Kasada's PoW self-solve runs in the same starved window. **This is testable and
   public-engine-addressable.**

The honest plausibility verdict: **medium-low** that any single public-engine lever flips a Kasada site, but the
drain fix (shared with AWS) is high-ROI to *try* because it is cheap, already half-built, and gated by a measurement
we can run. Everything beyond that is vendor_solvers territory (token farm / VM emulation) — explicitly out of scope
for public crates per CLAUDE.md.

---

## 1. What the existing repo docs already concluded (with citations)

| Source | Conclusion | Current validity |
|---|---|---|
| `docs/releases/v0.1.0-parity/08_KASADA_FRONTIER.md` | 3 sites fail across all BO profiles; Camoufox = open-source SOTA but only 4/5; canadagoose/hyatt/realtor are the frontier. 4 levers: K2-DIFF, CSS calc, `_maskAsNative` sweep, `bot1225` stub. | Levers 2 & 4 superseded (see §0). K2-DIFF + masking still relevant. |
| `memory/kasada_real_blocker_css_calc_math.md` (2026-05-10) | CSS Values 4 math (`sin/cos/tan/pi/e`) in `calc()` was the load-bearing blocker; `CalcExpr` only had Add/Sub/Mul/Div/Min/Max/Clamp. | **FIXED.** `CalcExpr` now has the full set (`length.rs:47-95`); parser at `calc.rs:101-145`. The calc probe blob is gone. |
| `memory/kasada_wrapper_cracked_and_remaining_leaks.md` (2026-05-10) | `/tl` error-report wrapper = `base64(json({data: base64(xor(plaintext, "omgtopkek"))}))`, 9-byte deployment-wide key. 16 residual error fields; `bot1225` is the biggest trust driver. | Wrapper cracking still valid (decryptor lives in internal repo). The 16-field list predates the child-realm + secure-context fixes. |
| `~/projects/browser_oxide_internal/.../VM_TRACE_FINDINGS_2026_05_12.md` | 5 engine-divergence `TypeError`s, all `Cannot read properties of undefined (reading 'unjzomuybtbyyhwwkdpkxomylnab')`. This is a **Kasada-internal sentinel tag**, NOT a Web API. Root cause hypothesis: our scope-chain / iframe-realm emulation returns fresh objects on re-access, dropping the tag Kasada hung on a prior access. | **Captured 3 days before the real child realm landed.** Must be re-run. Likely partially-to-fully stale (see §0.1). |
| `~/projects/browser_oxide_internal/.../UNJZOMUY_INVESTIGATION_2026_05_12.md` | 3 candidate divergence sites: (1) mediaDevices object-literal method extraction (`window_bootstrap.js`), (2) iframe `getOwnPropertyDescriptor` Proxy handler, (3) `_defProtoMethod` per-call wrapper recreation. Sniff tests proposed. | Candidate 2 (Proxy handler) now only on the *fallback* path; candidate 3 (`_defProtoMethod`) still live — see §3.3. Sniff tests never ran. |
| `memory/state_2026_05_16_kasada_engine_gap_sharpened.md` | DECISIVE: `nocdp.sh` real Chrome 147, same datacenter IP, zero interaction, **passes all three**. ⇒ NOT IP, NOT behaviour, NOT paid-farm. It is a passive static engine-vs-Chrome surface divergence. K2-DIFF (capture our `/tl`, field-diff vs real-Chrome capture) is the decisive next step. | Anchor still holds. But "passive static surface" must now be widened to include the **async execution surface** (PoW completion), which the 05-16 framing did not consider. |
| `docs/HANDOFF_2026_05_28b.md` §4 | AWS challenge.js self-solve produces zero async progress in the live navigate path because of the 50 ms inter-script / vs 5 s oracle drain. Public-engine-addressable. | **Directly applicable to Kasada PoW** — same starved window (§0.3, §3.1). |

Prior **CLOSED** dead-ends (do not re-open): the Kasada realm/sentinel/identity *as a global-path-identity* hunt
(Phase 2 OUTCOME A, `state_2026_05_16_phase0_rebaseline.md`) — all 4 global paths were Chrome-identical. Note this is
distinct from the *sentinel-tag-persistence* question in §0.1, which is about whether a property *set* on a realm
object survives re-read, not about global-path identity.

---

## 2. New external findings (2026, cited)

- **Kasada is a VM-bytecode PoW system.** p.js / ips.js ships a custom interpreter; the bytecode (string-encrypted,
  control-flow-flattened, polymorphic, rotated frequently) contains fingerprint collection + a proof-of-work puzzle.
  Three header tokens must all be valid on every protected request: `x-kpsdk-ct` (expensive PoW token),
  `x-kpsdk-cd` (cheap per-request token), `x-kpsdk-v` (version pin).
  Sources: [ScrapeBadger](https://scrapebadger.com/kasada-bypass), [2captcha](https://2captcha.com/h/kasada-bypass),
  [ZenRows 2026](https://www.zenrows.com/blog/kasada-bypass).
- **The PoW difficulty escalates for untrusted sessions.** "Real browsers spend ~2 ms solving it ... a fresh
  datacenter IP can face challenges that take seconds or even time out, while a warmed residential session faces
  near-instant challenges." This is decisive for BO: a multi-second PoW cannot complete inside a 500 ms drain.
  Source: [Scrapfly 2026](https://scrapfly.io/blog/posts/how-to-bypass-kasada-anti-scraping-waf).
- **Token payload contents:** canvas hash, WebGL GPU vendor/renderer, navigator values, screen props, plugin lists,
  audio-context fingerprint — all folded into the encrypted token. (Confirms our prior field inventory direction.)
  Source: [search synthesis, scrapebadger/zenrows].
- **Camoufox (Firefox-based) now 429s on canadagoose** ([daijro/camoufox #318](https://github.com/daijro/camoufox/issues/318))
  — same IP passes in the user's real browser. ⇒ residual signal is not a BO-only Chrome-source leak; it is a holistic
  trust gap shared by the open-source SOTA.
- **Patchright (CDP-patched Chromium) reportedly still passes Kasada** where vanilla Playwright fails
  ([THE LAB #76](https://substack.thewebscraping.club/p/bypassing-kasada-2025-open-source)). ⇒ a real-Chromium-shaped
  runtime + decent IP can clear it; the lever is fidelity-to-real-Chromium + PoW completion, not more stubbing.
- **Reverse-engineering references** (for any vendor_solvers VM work): [umasii/ips-disassembler](https://github.com/umasii/ips-disassembler)
  (full disassembly into an asm-like language; the project already cross-references this in
  `opcode_table.md` — 62/164 of our captured handler bodies auto-matched), [nullpt.rs](https://nullpt.rs/) VM writeups,
  [Humphryyy/Kasada-Deobfuscated](https://github.com/Humphryyy/Kasada-Deobfuscated). Emulating the VM out-of-browser
  "breaks within days" because the bytecode rotates — confirming any solver must run the live VM, not a static port.

---

## 3. BO code-level analysis

### 3.1 The live-nav async drain (HIGHEST-CONFIDENCE new lever — shared with AWS)

`crates/browser/src/page.rs`:
- Cold path final settle: **8 s** — `self.event_loop.run_until_idle(Duration::from_secs(8))` at `page.rs:611`.
- Warm / live-nav path: inter-script drain **50 ms** (`page.rs:1676-1679`), final drain **500 ms** (`page.rs:1728-1731`),
  cookie-sync **50 ms** (`page.rs:1651-1654`).

Kasada's `x-kpsdk-ct` PoW runs inside ips.js (frequently via a worker). For a fresh datacenter session the PoW is
*seconds*. The token POST + page re-fetch therefore cannot complete in 500 ms. This is mechanically identical to the
AWS challenge.js failure HANDOFF_2026_05_28b §4 localized. The public engine relies on **ips.js self-solving the PoW
in V8** (the concrete `x-kpsdk-*` solver lives in private `vendor_solvers` per CLAUDE.md; the public trait hooks are
in `crates/browser/src/challenge.rs:116`). If the self-solve is starved, no token, 429.

**Why this wasn't seen before:** the 2026-05-16 thesis framed the gap as "passive static surface" and the
nocdp/oracle captures used long drains. The live navigate path's short drain is a 2026-05-28-era discovery (AWS).
Nobody has yet re-tested Kasada under a generous live-nav drain.

### 3.2 The child-realm iframe path (makes the sentinel-loss finding stale)

`op_create_child_realm` (`crates/js_runtime/src/extensions/dom_ext.rs:1137-1256`) builds a **real `v8::Context`**
(`dom_ext.rs:1179`), copies the parent security token (`:1186-1187`), types the child global as `Window`
(`:1196-1215`), and **caches the child global in `IframeRealmStore.globals`** so repeat `contentWindow` access returns
the identical V8 object (`:1148-1154`, and JS-side cache in the `_iframeState` WeakMap,
`dom_bootstrap.js:2112`, `:2909-2912`). A property set on a function fetched from this realm **persists** on re-read —
unlike the Proxy fallback (`dom_bootstrap.js:2965-2982`) where `getOwnPropertyDescriptor` synthesizes a fresh
descriptor and `get` returns derived references. The 2026-05-12 sentinel-loss `TypeError`s were measured against the
*Proxy* path. **Action: re-run the VM dispatcher trace against the child-realm path** before spending any more effort
on the sentinel hypothesis.

Caveat (real residual risk): the child realm is built **nearly empty** — only `Window/window/self/globalThis/frames/length/opener`
and native `Function.prototype.toString` (`dom_ext.rs:1217-1247`). Real Chrome's about:blank child window exposes the
*full* global surface (every constructor, `document`, `navigator`, timers, etc.). If Kasada walks
`contentWindow.<API>` and finds it missing (rather than a tagged frame), that is a *different* divergence than the
2012 sentinel one and would still fire. The Proxy fallback masks this by falling through to `globalThis`
(`dom_bootstrap.js:2968-2969`), but the primary child-realm path does **not** populate the child global with the
parent's APIs. This is a concrete, inspectable gap.

### 3.3 `_defProtoMethod` per-call wrapper (UNJZOMUY candidate #3, still live)

`crates/js_runtime/src/js/window_bootstrap.js:146-167`:
```js
const _defProtoMethod = (proto, name, fn) => {
    ...
    const wrapped = ({ [name](...args) { return fn.apply(this, args); } })[name];
    ...
};
```
This creates a fresh function object per *installation* (not per access — it's called once at bootstrap and the
result is installed on the proto). So as long as a given method is installed exactly once, the reference is stable and
a tag would persist. The 05-12 doc flagged the risk of *double-installation* under the same name. Worth a one-line
audit (grep for duplicate `_defProtoMethod(proto, 'X', ...)`), but lower priority than §3.1/§3.2.

### 3.4 Function.prototype.toString masking (Lever 3) — broad but already substantially done

`_maskAsNative`/`_maskFunction` is applied across 10 bootstrap files
(`grep -l _maskAsNative crates/js_runtime/src/js/` → console/event/timer/fetch/interfaces/window/shared_apis/
cleanup/dom/stealth). The child realm installs native `Function.prototype.toString` too (`dom_ext.rs:1245-1247`).
The remaining risk is *completeness* — any one unmasked Web API method whose `toString` leaks BO source (or a deno op
name like `op_dom_attach_shadow`) is a binary tell. A systematic enumeration test (`String(API.prototype.method)`
asserts `[native code]` for *every* exposed method) is the right closure. Given v150 also fails, this is unlikely to
be the *sole* lever, but it is a parity hygiene requirement and cheap.

### 3.5 What is confirmed NOT the gap

- IP reputation (nocdp real Chrome passes from this exact datacenter IP — `state_2026_05_16_kasada_engine_gap_sharpened.md`).
- Behavioural absence (nocdp passes with zero mouse/scroll/keyboard).
- CSS calc math (shipped — §0.1, §1).
- Global-path realm identity (Phase 2 OUTCOME A, CLOSED).

---

## 4. Ranked fix list (ROI order)

> Confidence reflects probability the lever *flips at least one* of the three sites. All three sites failing in
> Camoufox v150 too means even a perfect public-engine fix may only narrow, not close, the holistic gap.

### FIX-K1 — Generous live-nav async drain for Kasada (and AWS) hosts  ★ highest ROI
- **What:** In the warm/live-nav path (`page.rs:1660-1731`), replace the 50 ms inter-script and 500 ms final drains
  with a host-aware budget (e.g. 8 s, matching the cold path `page.rs:611`) when the host is a known Kasada host
  *or* whenever an unresolved challenge marker is present. Mirror the AWS fix from HANDOFF_2026_05_28b §4 — they share
  the mechanism, so do them as one change. Add a worker-spawn / token-POST detector to the drain loop so it polls
  until `x-kpsdk-ct` posts or the budget expires (don't blindly burn 8 s on every nav).
- **Effort:** 1-2 days (the AWS work already scopes most of it; this generalizes the host gate).
- **Expected impact:** Necessary-not-sufficient. If the PoW self-solves under a long drain, could flip canadagoose
  and/or hyatt and simultaneously unblock the AWS cluster (7 sites) + duolingo. If it does not flip, it cleanly
  isolates the residual to the trust-score tail (§FIX-K3/K4) — a valuable negative result.
- **Confidence:** medium (high that it's *necessary*; medium that it's *sufficient* for any Kasada site).
- **Engine:** PUBLIC.

### FIX-K2 — Re-run the VM dispatcher trace against the child-realm path; populate the child global
- **What:** (a) Port `kasada_vm_dispatcher_trace` + `kasada_error_blob_capture` back into
  `crates/browser/tests/chrome_compat.rs` (they are referenced in the internal repo but absent from the current public
  test file). (b) Run against the live `op_create_child_realm` path and re-count the `unjzomuybtbyyhwwkdpkxomylnab`
  `TypeError`s. (c) If the child global is the gap, populate it with the parent's full API surface (constructors,
  `document`, `navigator`, timers) in `dom_ext.rs:1217-1247` so `contentWindow.<API>` lookups resolve like real
  Chrome's about:blank. (d) Add the three UNJZOMUY sniff tests (identity-stability of mediaDevices/iframe-descriptor/
  navigator-method re-access).
- **Effort:** 2-4 days (test port + investigation + child-global population).
- **Expected impact:** Removes up to 5 error-blob fields (`bot1225/csc/kl/dpv/smc`) IF they still fire. `bot1225` is
  the single biggest trust-score driver per prior analysis — closing it is the best shot at a holistic-tail flip.
- **Confidence:** medium-low (the original finding may already be fixed by the child realm; if so this is a quick
  negative result, if not it's the highest-value bug).
- **Engine:** PUBLIC.

### FIX-K3 — K2-DIFF: capture BO's `/tl` plaintext, field-diff vs the real-Chrome reference
- **What:** Build the in-VM plaintext-sensor dump (hook `fetch`/`XMLHttpRequest.send` on `/tl`, capture pre-XOR body
  to a Rust log), run hyatt + canadagoose, diff field-by-field against the captured real-Chrome reference at
  `~/projects/browser_oxide_internal/ab_harness/tl/hyatt.tl_body.bin` (36 KB) and `canadagoose.pcap`+`.keys`. Each
  divergent field is a named, fixable bug. Decode obfuscated identifiers via the internal string-table decoder.
- **Effort:** 3-5 days (tool build + diff loop; each fix is incremental).
- **Expected impact:** Systematically enumerates *every* remaining divergence (canvas hash, WebGL strings, audio fp,
  navigator values). The most thorough path, but slow and the target rotates quarterly.
- **Confidence:** medium (highest-information experiment; flips depend on what it finds).
- **Engine:** PUBLIC for the diffing + fixes; the capture tooling + decryptor are private (live in
  `browser_oxide_internal`).

### FIX-K4 — Function.toString native-mask completeness sweep
- **What:** A test that enumerates every exposed Web API prototype method and asserts
  `String(method) === "function NAME() { [native code] }"` (and no `op_*` substring). Patch any leak. Extend to the
  child realm and worker realms.
- **Effort:** 1-2 days.
- **Expected impact:** Closes `sfc/sdt/wse/bfe` error-report fields if any still leak; parity hygiene. Unlikely to be
  the sole flip lever (v150 fails too) but cheap insurance.
- **Confidence:** low (for flipping a site) / high (for correctness).
- **Engine:** PUBLIC.

### FIX-K5 (NOT public) — `vendor_solvers` Kasada PoW solver / VM driver
- **What:** A concrete `x-kpsdk-ct`/`x-kpsdk-cd` solver in the private `vendor_solvers` crate that drives ips.js's VM
  to completion (or replays a session-warmed token), using the umasii disassembly + the internal opcode table as
  reference. This is the only path that *guarantees* a valid token if the in-VM self-solve cannot be made to complete
  in the public engine. Out of scope for public crates per CLAUDE.md (`crates/browser/src/challenge.rs` exposes only
  the `ChallengeSolver` trait + `navigate_with_solvers` hook).
- **Effort:** 1-3 weeks, ongoing maintenance (bytecode rotates; "breaks within days" if statically ported — must
  drive the live VM).
- **Expected impact:** Could flip all three IF combined with a residential/mobile IP and session warming. Alone, with
  this datacenter IP, the trust score still depends on the holistic vectors.
- **Confidence:** medium-high IF paired with good IP + warming; low from this datacenter IP alone.
- **Engine:** VENDOR_SOLVERS (private) — flag accordingly.

---

## 5. Honest plausibility statement

- The strongest *public-engine* hypothesis is **FIX-K1 (the drain)** — it is cheap, shares a fix with the AWS cluster,
  and is the one lever that was never tested because the live-nav short-drain behavior is a brand-new (2026-05-28)
  discovery. It is necessary; whether sufficient for Kasada is unknown until measured.
- **FIX-K2** is the highest-value *bug* if it still reproduces, but it likely regressed-to-fixed when the real child
  realm landed (the finding predates it by 3 days). Re-measure first; do not build fixes on the stale trace.
- Because **Camoufox v150 also fails**, no public-engine lever is guaranteed to flip a site; the residual is a
  holistic trust score. The realistic public-engine outcome is "tie v150, possibly flip one site if the PoW completes
  under a longer drain". A *guaranteed* flip needs **FIX-K5 in vendor_solvers + a better IP** — explicitly out of scope
  for the public crates.
- realtor.com returns a larger interstitial (1764-1772 B vs 740/745 B for canadagoose/hyatt), suggesting a
  different/harder Kasada deployment tier; treat canadagoose or hyatt as the first flip target, not realtor.

## 6. Acceptance (if pursued)
- [ ] FIX-K1 shipped; Kasada hosts get a multi-second live-nav drain; measure whether `x-kpsdk-ct` posts.
- [ ] VM dispatcher trace + error-blob capture re-ported to public `chrome_compat.rs` and re-run on the child-realm path.
- [ ] `unjzomuybtbyyhwwkdpkxomylnab` TypeError count re-counted (expected: lower than 5, possibly 0).
- [ ] Child global populated with full API surface if §3.2 caveat confirmed.
- [ ] K2-DIFF tool built; one divergent `/tl` field named + fixed.
- [ ] Function.toString completeness test green across main + child + worker realms.
- [ ] canadagoose OR hyatt serves real `<title>` content end-to-end, ≥2/3 runs.

## Adjacent files / artifacts
- `crates/browser/src/page.rs:611` (cold 8 s drain), `:1651-1731` (warm 50 ms/500 ms drains) — FIX-K1
- `crates/js_runtime/src/extensions/dom_ext.rs:1137-1256` (child realm; near-empty global) — FIX-K2 §3.2
- `crates/js_runtime/src/js/dom_bootstrap.js:2112` `_iframeState` WeakMap, `:2965-2992` Proxy fallback realm
- `crates/js_runtime/src/js/window_bootstrap.js:146-167` `_defProtoMethod` — UNJZOMUY candidate #3
- `crates/css_values/src/types/length.rs:47-95` + `crates/css_values/src/calc.rs:66-145` — CSS calc (DONE)
- `crates/browser/src/challenge.rs:116` — public `ChallengeSolver` trait hook (concrete Kasada solver is private)
- `crates/browser/src/classify.rs:105-106` — `_kpsdk`/`ips.js` → `Kasada-CHL` classification
- `~/projects/browser_oxide_internal/docs/kasada_ips_analysis/{VM_TRACE_FINDINGS,UNJZOMUY_INVESTIGATION}_2026_05_12.md`
- `~/projects/browser_oxide_internal/ab_harness/tl/{hyatt.tl_body.bin, canadagoose.pcap+.keys}` — real-Chrome `/tl` reference
- `~/projects/browser_oxide_internal/docs/kasada_ips_analysis/opcode_table.md` (62/164 matched vs umasii)
- `docs/HANDOFF_2026_05_28b.md` §4 (the shared drain root cause)
