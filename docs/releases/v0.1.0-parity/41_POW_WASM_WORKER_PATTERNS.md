# 41 — Proof-of-Work, WebAssembly, and Worker patterns (cross-cutting)

**Status:** planning, cross-cutting category chapter
**Scope:** organised by **technique**, not by vendor. The per-vendor
chapters (06 AWS WAF, 07 DataDome, 08 Kasada, 25 Cloudflare) own the
fix detail; this chapter owns the **technique inventory** so a
contributor can see "what does BO need across all vendors that use
PoW / WASM / Workers" in one place.
**Cross-links:** 05 (SPA hydration — duolingo Worker / MessageChannel
hypothesis), 06 (AWS WAF — challenge.js loads WASM PoW), 07 (DataDome —
WASM-iframe-daily-key primitive), 08 (Kasada — ips.js VM + `/tl` PoW
sensor), 17 (Web API parity matrix — line-itemised gaps),
18 (vendor inventory), 25 (Cloudflare — JSC + Turnstile WASM),
26 (Akamai BMP — sensor_data v3 envelope).

---

## TL;DR

Three techniques that didn't exist (at industrial scale) in the
2018–2020 anti-bot landscape are now load-bearing across the 2026
vendor set:

1. **Proof-of-Work (PoW)** — server gives client a CPU-bound puzzle;
   client computes; server verifies cheaply. Defeats horizontal
   scaling because every request now costs CPU. Used by Kasada
   (primary mechanism), AWS WAF (HashcashScrypt / SHA-256 /
   NetworkBandwidth flavours per the public reverse-engineering set
   in chapter 06 §0.2), Cloudflare JSC (`jschl-answer`) and Cloudflare
   Managed Challenge (WASM-side), PerimeterX (lighter, gates the
   `_px3` cookie), and Akamai sec-cpt (the brute-force pre-image
   search).
2. **WebAssembly (WASM) challenges** — vendor logic shipped as a
   binary blob instead of obfuscated JS. Gives stronger obfuscation
   (no source to read), faster execution (no V8 deopt), and harder
   patching (need WASM-aware tooling). Used by DataDome (the
   "WASM-iframe-daily-key" referenced in chapter 07), AWS WAF (PoW
   compute), Cloudflare Turnstile, Akamai BMP v3 envelope (per
   chapter 26), and increasingly by Cloudflare Bot Management
   per-zone modules.
3. **Worker-based fingerprinting** — vendors spawn `DedicatedWorker`,
   `SharedWorker`, or `ServiceWorker` contexts and compute their
   fingerprint there, in isolation from the main-thread debugger.
   They then **cross-check** the Worker fingerprint against the main
   thread (UA, canvas, hardwareConcurrency, timing) — any
   inconsistency = bot. Used by reCAPTCHA enterprise (chapter 05 H1
   duolingo blocker), Arkose Labs invisible mode, F5/Shape telemetry,
   Cloudflare Turnstile, and several Akamai-instrumented React sites.

**BO must handle all three** to survive the 2026–2027 vendor evolution.
The good news: V8 (via `deno_core 0.311`) already gives us PoW and
WASM execution at native speed. The bad news: many PoWs are **gated
by a fingerprint check that bails BEFORE the PoW runs** (chapter 06
§0.4 documents this for AWS WAF; the silent-bail is the actual block,
not the PoW), and the Worker IPC primitives (`MessageChannel`,
`MessagePort`, `BroadcastChannel`) are NO-OP stubs (chapter 17 §2.5,
chapter 05 §2.3 H1) — fixing those is the single highest-leverage
change called out in this chapter.

| Technique | # vendors using it | BO capability today | Highest-leverage fix |
|---|--:|---|---|
| PoW | 5–7 | V8 executes any PoW JS at Chrome speed; the **fingerprint gate** is the issue | Close fingerprint gaps (chapter 06 §2, chapter 08 K2-DIFF) |
| WASM | 4–5 | V8 supports WASM natively; same fingerprint-gate problem | Worker-context fingerprint consistency (chapter 06 candidate 13) |
| Worker fingerprint | 3–5 | Main covered; **cross-thread fingerprint NOT audited** | Audit worker_bootstrap.js vs window_bootstrap.js consistency |
| `MessageChannel` paired-port routing | 3+ | **NO-OP STUB** (chapter 17 §2.5, chapter 05 §2.3 H1) | **Implement paired-port `postMessage` (~200–500 LOC)** |

---

## 1. Why these three converge

Modern anti-bot vendors moved beyond the 2018-style "Test fifty navigator
properties, look at canvas hash, done" model for three reasons that
this chapter argues are non-coincidental.

### 1.1 The economic argument for PoW

The classic "headless detection" model is asymmetric in the wrong
direction: defenders spend N cycles per request running fingerprint
checks; attackers spend ~N cycles per request running a headless
browser. Defenders lose if attackers throw cheap cloud cycles at them
(N is small either way; cloud CPU is cheap). PoW flips the asymmetry:
attackers must spend M cycles to compute the puzzle, defenders spend
~0 to verify the answer. M can be tuned (Cloudflare picks ~5 s, AWS
picks ~50–100 ms, Kasada picks ~100–500 ms) so legitimate users barely
notice but scrapers can't afford to scale.

The Cloudflare blog post ["How CloudFlare client-side DDOS detection
works"](https://khromov.wordpress.com/2013/02/05/cloudflare-client-side-ddos-detection-how-does-it-work/)
captured the design intent in 2013: "force the client's CPU to solve
a problem before serving content." Twelve years later this is the
default architecture pattern across the vendor set.

### 1.2 The obfuscation argument for WASM

JavaScript obfuscation is reversible — even the heaviest tools
(`obf-io.deobfuscate.io` per chapter 06 §1.4 prescription) unwrap
string-array indirection and control-flow flattening in minutes. Source
maps for the obfuscator typically leak from one careless deploy and
get archived. The reverse-engineering community at large can pool
analyses across vendors.

WASM ships as a binary. The mainline tools (`wasm2wat`,
[WebAssembly Binary Toolkit / wabt](https://github.com/WebAssembly/wabt))
produce a textual representation, but the lifted text is at the
instruction level (~100,000 instructions per typical PoW module),
without identifier names, function signatures, or type information.
[arxiv:2508.21219 ("The WASM Cloak")](https://arxiv.org/html/2508.21219v1)
quantifies this: "development of WASM-aware detectors that can
operate directly on WebAssembly binaries is a key direction, which
could leverage static analysis to identify suspicious instruction
patterns" — i.e. WASM analysis is a research problem, not a tooling
problem. Per chapter 06 §1.4: hours-per-vendor reverse-engineering
cost. DataDome's strategy doc ([VM-Based Obfuscation
2026](https://datadome.co/changelog/vm-based-obfuscation/)) makes the
goal explicit: "combined with existing dynamic and WASM protections,
this three-layer defense represents the cutting edge of client-side
detection security."

### 1.3 The interception argument for Workers

DedicatedWorker, SharedWorker, and ServiceWorker each get their **own
JavaScript global**, their **own debugger context**, and their **own
event loop**. A defender wiring a fingerprint computation in a Worker
gets three properties that the main thread can't offer:

- **Less interception** — main-thread shims (e.g. Object.defineProperty
  hooks installed by a Chrome extension or a Tampermonkey script) don't
  apply in the Worker scope. A vendor that runs `navigator.userAgent`
  inside a Worker reads from the engine, not from the page's user
  shim.
- **Less observability** — DevTools' "Sources" tab shows worker scripts
  but in a separate panel; the attention budget of a reverse engineer is
  spent on the main thread by default.
- **Cross-thread consistency check** — see §4.4. The Worker computes its
  own copy of the fingerprint (UA, canvas, AudioContext, timing) and
  the vendor compares it to the main-thread copy. If only the main
  copy was patched, the discrepancy flags the bot. Per
  [Castle Security's bot-detection writeup](https://blog.castle.io/roll-your-own-bot-detection-fingerprinting-javascript-part-1/):
  "Detection systems can identify inconsistencies across JavaScript
  execution contexts: main page, iframes, and web workers… the more
  relationships you can validate between fingerprint attributes, the
  harder it becomes for attackers to spoof everything correctly."
  [FP-Inconsistent (arxiv:2406.07647)](https://arxiv.org/pdf/2406.07647)
  systematises the same point as "a data-driven approach to discover
  rules to detect fingerprint inconsistencies across space".

### 1.4 Why all three at once

PoW-only is solved with cheap cloud CPU (pay the bill). WASM-only is
reverse-engineered in hours and reimplemented. Worker-only still
exposes logic to a determined patcher. Vendors combining all three —
PoW WASM loaded inside a Worker that cross-checks the main thread —
force attackers to rent CPU, reverse WASM, patch Worker scope, AND
keep main-thread fingerprint consistent. The combined cost is what
made Kasada the open-source SOTA frontier (chapter 08).

BO does not currently combine all three. We execute PoW (V8 runs JS
and WASM at Chrome speed) but our Worker scope is partially stubbed
and our `MessageChannel` is a no-op — so a Worker-routed PoW that
needs main↔worker handshake breaks silently. This chapter inventories
the fixes that close that gap.

---

## 2. Proof-of-Work mechanisms

### 2.1 What is a PoW challenge

A PoW challenge is a one-way function chained with a verification step:

1. **Server** picks parameters `(c, T)` where `c` is a per-request
   challenge and `T` is a difficulty target. Sends `(c, T)` to the
   client in the challenge page (usually as JSON inside an inline
   script tag).
2. **Client** runs an algorithm `f(c, nonce)` over varying `nonce` until
   `f(c, nonce) < T`. The found `nonce` is the "answer". The classic
   form is `f = sha256(c || nonce)` and the test is "leading N hex
   zeros" but vendors use richer shapes (scrypt for memory-hardness,
   network-bandwidth tests for connection profiling).
3. **Client** POSTs `(c, nonce)` (sometimes wrapped in a fingerprint
   envelope) to the verify endpoint.
4. **Server** runs `f(c, nonce)` once, checks `< T`, sets a clearance
   cookie or returns a token. ~1 ms server-side; the work was
   client-side.

The asymmetric cost is the whole point. The
[Cloudflare clearance docs](https://developers.cloudflare.com/cloudflare-challenges/concepts/clearance/)
state it plainly: "the proof-of-work involves computing a hash that
meets Cloudflare's difficulty target — similar to cryptocurrency
mining but much simpler". For a vendor to deploy a PoW, the **only**
requirement is that legitimate users tolerate the latency. Cloudflare
JS Challenge sits at ~5 s (deliberately high — it's a deterrent, not a
gate). AWS WAF sits at ~50–100 ms. Kasada sits at 100–500 ms (per the
[2captcha Kasada deep-dive](https://2captcha.com/h/kasada-bypass):
"real browsers spend ~2 ms; primitive bots can take seconds or fail
outright" — the "~2 ms" is the wall-clock time on a real desktop,
not the actual CPU work; the CPU work is ~100–500 ms).

### 2.2 Per-vendor PoW implementations

#### Kasada — `x-kpsdk-ct` and the `/tl` sensor

**See chapter 08** for the full research arc; this is the technique
sketch.

- **Bootstrap:** the protected origin serves a stub that loads
  `/ips.js` (or a path-rotated variant). `ips.js` is a polymorphic
  obfuscated JS bundle containing a **bytecode VM** (per
  [ChrisYP's analysis](https://github.com/ChrisYP/ChrisYP.github.io/blob/main/en-US/kasada.md)
  and [lktop/kpsdk](https://github.com/lktop/kpsdk): "the core logic
  is compiled to bytecode that is run by the embedded VM, with all
  strings encrypted and decrypted while the program is running").
- **PoW compute:** runs inside the VM. The output is a token `x-kpsdk-ct`
  that the protected origin's subsequent fetches must carry as a
  header (the unicorn-aio/kpsdk repo and the
  [2captcha Kasada solver doc](https://2captcha.com/p/kasada-solver)
  enumerate the header set: `x-kpsdk-{ct,cd,cr,r,v,st}`). The
  "Security Token" `x-kpsdk-ct` is the load-bearing one for clearance;
  the others carry telemetry that feeds Kasada's anti-bot ML.
- **Sensor envelope:** the VM also POSTs a `/tl` sensor payload — a
  serialised fingerprint vector encoded with the 9-byte XOR wrapper
  `omgtopkek` (cracked in `memory/kasada_wrapper_cracked_and_remaining_leaks.md`
  per chapter 08 Phase 1). The plaintext sensor is the **ground
  truth** for K2-DIFF (chapter 08 Lever 1).
- **Cost:** 100–500 ms real-browser CPU per token. Per-zone difficulty
  varies.
- **BO status:** V8 executes `ips.js` and the VM. The PoW resolves
  (we've seen `x-kpsdk-ct` issued). The block comes from the
  **post-issue verify** — Kasada checks the sensor envelope, rejects
  it because of fingerprint divergence, and refuses to honour the
  issued token. Chapter 08 K2-DIFF is the next concrete step.
- **Cookie / clearance:** `_kpsdk` family + `x-kpsdk-ct` header on
  subsequent requests.

#### AWS WAF — challenge.js + WASM PoW

**See chapter 06** for the full plan.

- **Bootstrap:** `<script src="…/challenge.js" defer>` injected by the
  WAF when the rule action is `Challenge` (per the
  [AWS WAF JS Challenge API docs](https://docs.aws.amazon.com/waf/latest/developerguide/waf-js-challenge-api.html)).
- **PoW flavours:** per
  [neiii/aws-waf-solver](https://github.com/neiii/aws-waf-solver)
  reverse-engineering and chapter 06 §0.2: HashcashScrypt, SHA-256,
  NetworkBandwidth. Each is picked per-request by the WAF based on
  difficulty target.
- **WASM module:** ships embedded as a base64 blob inside `challenge.js`
  (chapter 06 §1.4: "atob('AGFzbQ==…') is a strong tell — `AGFzbQ` is
  '\0asm' in base64, the WASM magic header"). The WASM module
  actually computes the PoW.
- **Cost:** ~50–100 ms.
- **BO status:** **detected but NOT solved.** The telemetry POST to
  `awswaf.com/.../report` fires (chapter 06 TL;DR), so `challenge.js`
  ran, but `getToken()` never reaches its continuation. Chapter 06
  §0.4 diagnosis: a fingerprint gate inside `challenge.js` (or inside
  the WASM module) bails before the PoW runs. The PoW machinery would
  work; we never get to invoke it.
- **Cookie / clearance:** `aws-waf-token` (per the
  [AWS WAF challenge & CAPTCHA blog](https://aws.amazon.com/blogs/networking-and-content-delivery/protect-against-bots-with-aws-waf-challenge-and-captcha-actions/)).

#### Cloudflare JS Challenge — `jschl-answer`

**See chapter 25 §0.4 and §2.1** for the full handling.

- **Bootstrap:** 503 response with body containing `<form id="challenge-form"
  action="/cdn-cgi/l/chk_jschl" method="POST">` and hidden inputs
  `jschl_vc`, `jschl_answer`, `pass`.
- **Algorithm:** the page runs an obfuscated JS computation that takes
  ~5 s to resolve (deliberately slow to deter scaling). Per
  [Khromov's 2013 writeup](https://khromov.wordpress.com/2013/02/05/cloudflare-client-side-ddos-detection-how-does-it-work/):
  "the jschl_vc form field uniquely identifies the challenge to
  CloudFlare, so that the backend knows what the answer should be.
  If jschl_answer is interpreted as being the correct result, a
  cookie called cf_clearance is created with a unique id that
  identifies the user as having verified the challenge."
- **Status:** **legacy.** Cloudflare moved to Managed Challenge
  (chapter 25 §0.1) as the default in 2022; JSC still appears on
  older sites and on "Under Attack" mode zones.
- **Cost:** ~5–10 s wall-clock.
- **BO status:** V8 executes the JS; the body markers
  `cf-browser-verification` and `Just a moment...` are caught at
  `crates/browser/src/classify.rs:82,92`. The PoW computation must
  complete inside the 90 s poll deadline (chapter 25 §0.4); most
  do. The rendering primitives in chapter 07 are the gating fix.
- **Cookie / clearance:** `cf_clearance` (~30 min default).

#### Cloudflare Managed Challenge / Turnstile — WASM PoW

**See chapter 25 §0.1 (Managed Challenge) and §0.2 (Turnstile)** for
detail.

- **Bootstrap:** orchestrator JS fetched from `/cdn-cgi/challenge-platform/h/b/orchestrate/...`
  reads `_cf_chl_opt` and decides between silent PoW path, Turnstile
  widget, or block.
- **PoW:** WASM-based inside the orchestrator bundle. Cloudflare does
  not publicly document the algorithm; the
  [scaredos/cfresearch repo](https://github.com/scaredos/cfresearch)
  has open-source analyses but specific PoW shapes rotate.
- **Cost:** sub-second for the silent path; up to 5 s if the WASM is
  cold.
- **BO status:** rendering covered by chapter 07 primitives once they
  land. The actual PoW compute should "just work" once the
  orchestrator script can fetch and the iframe can materialize.
- **Cookie / clearance:** `cf_clearance` (silent path) or
  `cf-turnstile-response` token in a form field (widget path).

#### Akamai sec-cpt — brute-force preimage search

- **Bootstrap:** the protected origin serves `/_sec/cp_challenge/...`
  with a bundle that requires a brute-force preimage search
  (sometimes called "hash chain" — find a value `x` such that
  `H(prefix || x)` matches a target prefix).
- **PoW:** per memory `state_2026_05_16_phase5_datadome.md` and chapter
  07 §"Out of scope" — the sec-cpt bundle **self-solves in our V8
  today** (homedepot was flipped pre-strip when the sec-cpt bundle
  ran end-to-end). The actual brute-force loop is the bundle's own
  responsibility; we just need to not interfere.
- **Cost:** ~50–500 ms depending on the difficulty target the origin
  sets.
- **BO status:** working when the rendering primitives don't block
  it. The body marker `/_sec/cp_challenge` is UNAMBIGUOUS at
  `crates/browser/src/classify.rs:84`.
- **Cookie / clearance:** the bundle sets a session cookie that the
  origin checks on retry.

#### PerimeterX (HUMAN) — lightweight PoW gating `_px3`

- **Bootstrap:** a stub script loads from `*.perimeterx.net` or the
  customer's CDN. The stub computes a lighter PoW (smaller difficulty
  than Kasada) plus a fingerprint, gates issuing the `_px3` cookie on
  the PoW result.
- **Cost:** ~10–50 ms.
- **BO status:** detected via body markers `_pxhd`, `px-captcha`,
  `press & hold` (chapter 18 §"Body markers"). No specific solver
  registered; the chapter-07 generic primitives plus a stealth
  profile that doesn't flag should be enough for the lightweight
  path. PerimeterX Press-and-Hold widget is interactive and falls into
  the "interactive captcha" out-of-scope category.

#### Akamai V3 envelope — sensor_data post-aecdf19

- **See chapter 26** for the full Akamai BMP story.
- The pre-strip private code had a partial V3 envelope implementation.
  The envelope is **WASM-derived** (per `memory/state_2026_05_14_v3_envelope_measured.md`):
  the sensor_data POST body is encoded with a transform whose state is
  initialised from a WASM-shipped key. The actual sensor collection is
  JS; the envelope is WASM.
- **BO status:** the public engine ships no V3 encoder. Akamai V2
  (the older `_abck`-based envelope) was handled in the now-removed
  `crates/akamai/` per chapter 07 §"Context — what aecdf19 removed".

### 2.3 Solving PoW in BO — three architectures

A given PoW can be handled in three ways. Each is appropriate for a
different class of vendor.

**Option A — let V8 execute the PoW.** Default. BO's V8 (deno_core
0.311) runs JS and WASM at Chrome speed (per `Cargo.toml` workspace
member declaration; per chapter 06 §"BO PoW capability assessment"
the math says BO is at Chrome speed because the same V8 builds power
both). For any PoW where the gating logic isn't a separate fingerprint
check that bails first, Option A is correct and zero-engineering.

This is **already what Kasada gets** (chapter 08: the VM runs, `x-kpsdk-ct`
is issued). What blocks Kasada is the post-PoW sensor verify, not the
PoW compute.

This is **also what Cloudflare JSC and Managed Challenge get** once
the chapter-07 rendering primitives land — the orchestrator script
runs, the WASM PoW runs, the cookie lands, the cookie-delta retry
re-fetches. No PoW-specific engine work needed.

**Option B — extract + reimplement in Rust.** The chapter 06 §3
alternative B path for AWS WAF. Capture the WASM blob from the live
`challenge.js`, run it under [wasmtime](https://wasmtime.dev/) in
the private `vendor_solvers` crate with shimmed JS-host imports.
Or reimplement the PoW (scrypt, sha256, hashcash) in pure Rust using
the `sha2` / `scrypt` crates.

Pros: bypasses fingerprinting entirely (AWS sees only the POST).
Cons: brittle (chapter 06 §3 Alternative B "high maintenance: AWS
WAF updates the obfuscation regularly; expect 1 break every ~30 days").
License: stays in the private `vendor_solvers` crate per `CLAUDE.md`.

**Option C — detect + skip.** Some sites have non-PoW alternatives
(e.g. an unprotected `/sitemap.xml` or a mobile-app API that doesn't
go through the JS WAF). Not relevant for the v0.1.0 corpus (every
target is the main site landing page) but worth noting for future
work.

### 2.4 BO PoW capability assessment

| Capability | Status | Source of truth |
|---|---|---|
| V8 executes JS PoW | ✅ deno_core 0.311 = Chrome-speed | `crates/js_runtime/src/runtime.rs:1-130` |
| V8 executes WASM PoW | ✅ V8 native (verified by `WebAssembly.validate(new Uint8Array([0,97,115,109,1,0,0,0]))` returning `true` per chapter 06 §2 candidate 12) | same |
| `WebAssembly.instantiateStreaming` / `compileStreaming` | ✅ wrapped in `crates/js_runtime/src/js/window_bootstrap.js:17-27` (chapter 17 §2 line 526 confirmed) | `window_bootstrap.js:17-27` |
| WASM SIMD opcode coverage | ❓ V8 default config exposes it; not feature-checked at runtime | see §3.4 |
| WASM threads & atomics | ❓ requires `SharedArrayBuffer` + cross-origin-isolated context — verify | (verify) |
| WASM exception handling proposal | ❓ enabled by default in modern V8 | (verify) |
| WASM bulk-memory | ✅ shipped in V8 ≥ 8.0 | (verified by spec compliance) |

**The structural risk** flagged in chapter 06 §2 candidate 12: "the
challenge.js runs a WASM module. If our WASM impl differs at any spec
edge (e.g. SIMD opcode coverage, Threads & Atomics, exception
handling), the PoW could fail silently." Recommendation per chapter 06:
feature-detect at runtime and capture the bit-pattern for any spec
edge that a vendor module exercises.

**The non-structural risk** is the fingerprint gate. Per chapter 06
§0.4: "challenge.js runs a fingerprint check **before** issuing the
token, our engine fails it, and it silently bails." The PoW path
never gets entered; closing the fingerprint gap is what unlocks PoW.

### 2.5 The "PoW gate is upstream" pattern

A subtle implementation invariant: PoW vendors don't actually want
robots to **fail** the PoW; they want robots to **avoid** the PoW
(robots that fail PoW are still detected; robots that succeed reveal
themselves by completing impossibly fast). The actual filter is
upstream:

```
request → fingerprint check (cheap, server- or client-side)
       ├─ PASS → issue PoW → client computes → verify → clearance
       └─ FAIL → silently bail / serve interstitial forever
```

Three of the four PoW vendors above use this pattern:

- AWS WAF: silent bail in `getToken()` (chapter 06 TL;DR).
- Kasada: post-PoW sensor envelope rejection (chapter 08 K2-DIFF).
- Cloudflare Managed Challenge: per-zone risk model gates whether to
  serve the silent path, the Turnstile path, or the block path
  (chapter 25 §2.1).

The implication for BO: **PoW capability is necessary but not
sufficient.** Adding faster WASM / better PoW execution / Rust-side
PoW reimplementation does not lift any of the failing sites in the
126-corpus today. The bottleneck is the fingerprint gate. This is why
chapter 06 spends ~150 lines on §2 (candidate signal table) and
~30 lines on §3 Alternative B (the actual PoW): the fingerprint
investigation is the leverage.

---

## 3. WebAssembly-based challenges

### 3.1 What WASM gives vendors

[The WebAssembly Core Specification](https://webassembly.org/specs/)
defines WASM as a "binary instruction format for a stack-based
virtual machine" with a "compact binary format that enables near-native
performance and takes advantage of common hardware capabilities". The
properties that matter for the anti-bot use case:

1. **Binary obfuscation.** The spec defines a binary format; the
   textual `.wat` form is purely a tooling convention. A vendor
   ships only the binary, and the bin → wat lifter (`wasm2wat` in
   [WABT](https://github.com/WebAssembly/wabt)) produces instruction-
   level text without function names, parameter names, or type names
   (only numeric type indices). The
   [arxiv:2508.21219 paper](https://arxiv.org/html/2508.21219v1)
   "Evaluating Browser Fingerprinting Defenses Under WebAssembly based
   Obfuscation" measures the gap quantitatively: defenders that hook
   JS getters get bypassed when fingerprinting moves to WASM.
2. **Faster execution.** V8 compiles WASM with a single-pass baseline
   (Liftoff) plus optimising tier (TurboFan). No tier-up deoptimisation
   spikes mid-fingerprint-compute. A vendor that uses WASM gets
   deterministic per-call timing — useful both for the actual compute
   and for **timing-based fingerprint probes** (chapter 06 §2
   candidate 9: `performance.now()` granularity / monotonicity).
3. **Smaller payload than equivalent JS.** Typical 5–20× compression
   for the same algorithm. The WASM PoW module shipped inside AWS WAF
   `challenge.js` is ~30–50 KB; the equivalent obfuscated JS would
   easily be 200+ KB.
4. **Harder to patch.** Patching WASM requires understanding the
   binary format (LEB128 integers, type-section indices, function-
   section indices). A regex on the source — which works on
   obfuscated JS (chapter 06 §3 Alternative C) — doesn't apply to
   WASM. A patcher needs `wasm-edit`-style tooling.

### 3.2 Per-vendor WASM use

#### AWS WAF — WASM PoW module

- **Where:** embedded as a base64 blob inside `challenge.js`. Decoded
  at runtime via `atob('AGFzbQ==…')` (chapter 06 §1.4) →
  `WebAssembly.instantiate` → exports a `solve()` function.
- **What it does:** computes the actual PoW (HashcashScrypt /
  SHA-256 / NetworkBandwidth per the
  [Switch3301/Aws-Waf-Solver analysis](https://github.com/Switch3301/Aws-Waf-Solver)).
  May also do per-call fingerprint reads via JS imports.
- **BO status:** if V8 executes the WASM (and it does), the PoW
  computes. Issue is upstream (chapter 06 §2.5 "PoW gate is upstream").

#### DataDome — the "WASM-iframe-daily-key"

- **Where:** loaded inside the cross-origin DataDome iframe (chapter 07
  §"Primitive 2" — the iframe materialization path). The iframe's
  document fetches a WASM module whose **key rotates daily**
  ([glizzykingdreko's deep-dive](https://medium.com/@glizzykingdreko/breaking-down-datadome-captcha-waf-d7b68cef3e21):
  "every day, the keys in the signals dictionary change to a random
  six-character string. If you don't match them correctly, the
  solving will be invalid").
- **What it does:** computes a hash of the fingerprint envelope plus
  the day-key, embeds it in the verify POST. The day-key rotation
  defeats long-term solver caches.
- **VM layer:** as of 2026-01-14, DataDome added an additional
  VM-based obfuscation layer on top of the WASM
  ([DataDome changelog](https://datadome.co/changelog/vm-based-obfuscation/),
  [Security Boulevard analysis 2026-02](https://securityboulevard.com/2026/02/datadome-releases-vm-based-obfuscation-the-next-evolution-in-client-side-detection-security/)).
  The community reverse-engineering set has caught up
  ([xKiian/datadome-vm](https://github.com/xKiian/datadome-vm)) but
  the moving target is faster than the analysis cadence.
- **BO status:** rendering covered by chapter 07's primitives once
  they land. The actual WASM compute requires the iframe to fetch
  and run (primitive 2). The day-key rotation makes a Rust-side
  solver impractical (a new key every 24h means a new analysis pass
  every 24h); the V8-executes-WASM path is structurally sound.

#### Akamai V3 envelope — WASM-derived sensor encoder

- **See chapter 26.** The V3 envelope (post the V2 `_abck` family) is
  WASM-derived. The sensor_data POST body bytes are produced by a
  transform initialised from WASM-shipped state.
- **BO status:** the public engine ships no encoder; per chapter 26
  the work is in the private `vendor_solvers`.

#### Cloudflare — Turnstile and Bot Management

- Cloudflare's WASM use is per-zone customised. Turnstile (chapter 25
  §0.2) ships a WASM module inside the
  `challenges.cloudflare.com/turnstile/v0/api.js` bundle. The module
  computes the PoW and the fingerprint envelope.
- Bot Management (per-zone, paid Cloudflare plan) ships its own WASM
  modules per zone — the per-zone obfuscation means the same code
  changes per customer.
- **BO status:** same as DataDome — V8 runs the WASM, rendering
  primitives gate it.

### 3.3 BO WASM capability

deno_core 0.311 ships V8 with full WASM support. Per chapter 17
§2 line 526 ("WebAssembly.instantiateStreaming byte-exact roundtrip
✅ V8 native") the basic surface is covered.

The wrappers in `crates/js_runtime/src/js/window_bootstrap.js:17-27`:

```js
if (globalThis.WebAssembly) {
    WebAssembly.instantiateStreaming = async function(source, importObject) {
        const response = await source;
        const bytes = await response.arrayBuffer();
        return WebAssembly.instantiate(bytes, importObject);
    };
    WebAssembly.compileStreaming = async function(source) {
        const response = await source;
        const bytes = await response.arrayBuffer();
        return WebAssembly.compile(bytes);
    };
}
```

These exist because V8's native `WebAssembly.{instantiate,compile}Streaming`
expects a `Response` object with proper `Content-Type: application/wasm`
headers; our `fetch_ext.rs` may not always set the MIME correctly,
so the wrappers fall back through the byte-array path. This is a
correctness fix, not a performance regression.

### 3.4 The WASM-edge feature-detect gap

Per chapter 06 §2 candidate 12, the risk surface for "WASM-detected-as-
non-Chrome" is at the proposal feature edges:

| Proposal | V8 default | BO state today | Chrome 148 emits |
|---|---|---|---|
| `simd128` | enabled | inherited from V8; not feature-tested | yes |
| `mutable-globals` | enabled (since V8 6.5) | inherited | yes |
| `bulk-memory` | enabled (since V8 8.0) | inherited | yes |
| `reference-types` | enabled (since V8 8.5) | inherited | yes |
| `multi-value` | enabled (since V8 8.6) | inherited | yes |
| `tail-call` | enabled (since V8 11.1) | inherited | yes |
| `threads & atomics` | requires SharedArrayBuffer + cross-origin-isolated | needs `Cross-Origin-Opener-Policy: same-origin` + `Cross-Origin-Embedder-Policy: require-corp` — not set by BO today | yes (in cross-origin-isolated contexts) |
| `exception-handling` | enabled | inherited | yes |
| `gc` | enabled (since V8 11.9) | inherited | yes |

The single load-bearing gap: **threads & atomics**. A vendor WASM that
uses `SharedArrayBuffer` for shared memory between main thread and
WASM workers requires the cross-origin-isolated environment — which
in turn requires the COOP/COEP headers that BO doesn't currently
respect for its synthetic iframes. If a vendor's PoW uses threads,
BO's WASM runs single-threaded fall-back (if the vendor handles that
path) or throws (if not). Acceptance suggestion §8: add a feature-
detect probe to the diagnostic capture suite, verify against real
Chrome 148.

### 3.5 Reverse engineering WASM challenges — process

For research work (not production solving — per `CLAUDE.md` that stays
in `vendor_solvers`). Pipeline: pull the WASM blob (either a separate
`.wasm` asset, or base64 embedded as `atob('AGFzbQ==…')` — the
`AGFzbQ` magic header is the tell, chapter 06 §1.4); sanity-check
`xxd` shows `00 61 73 6d 01 00 00 00`; lift with
[wasm2wat from WABT](https://github.com/WebAssembly/wabt); grep
imports/exports to find the JS-host interface surface; grep for known
PoW constants (SHA-256 H[0..2] = `1116352408`, `1899447441`,
`3049323471`; scrypt blockmix = `0x05a2`, `0x6cf3`, `0x1bf6`).

Cost per vendor: hours to days first pass; less for rotations
(structure is stable; only obfuscation layer changes). Internal repo
`~/projects/browser_oxide_internal/` may have prior captures (per
chapter 08).

Tools: WABT (`wasm2wat`, `wat2wasm`, `wasm-objdump`, `wasm-decompile`,
`wasm-strip`, `wasm-validate`); [Wasmtime](https://wasmtime.dev/)
to execute captured modules with shimmed imports (per chapter 06 §3
Alternative B); [wasmer](https://wasmer.io/) as alternative runtime.

---

## 4. Worker-based fingerprinting

### 4.1 Why Workers

[The HTML Spec's Web Workers chapter](https://html.spec.whatwg.org/multipage/workers.html)
defines three worker classes:

- **DedicatedWorker** — single owner (the spawning page); messaging is
  point-to-point.
- **SharedWorker** — multiple owners across same-origin contexts;
  messaging via `MessagePort`.
- **ServiceWorker** — long-lived, intercepts network requests for its
  scope; offline support, push notifications.

Each is a separate JavaScript global with its own event loop, its
own debugger context, its own access to a (restricted) set of Web
APIs. The WHATWG spec language: workers "execute in a separate
parallel execution environment".

Vendors use them for the reasons §1.3 listed: less interception, less
observability, plus a fourth reason — **off-thread compute doesn't
block the page's main loop**. A 100ms PoW compute on the main thread
shows up as jank; the same compute in a Worker is invisible to the
user.

### 4.2 Per-vendor Worker use

#### reCAPTCHA enterprise (chapter 05 H1 duolingo)

The single best-documented Worker use case in the BO corpus, because
it's the load-bearing blocker on duolingo.

- **The Worker:** `https://www.recaptcha.net/recaptcha/enterprise/webworker.js`
  loaded by `recaptcha/enterprise.js` (chapter 05 §2.2).
- **Internal IPC:** the enterprise.js bootstrap creates a
  `MessageChannel`, passes `port2` to the Worker via `worker.postMessage`
  with `[port2]` as the transfer list, retains `port1`. Subsequent
  fingerprint + token exchanges go through the paired ports, NOT
  through the Worker's parent↔worker `onmessage` channel. This is the
  exact pattern the spec's [Web Messaging entanglement section](https://html.spec.whatwg.org/multipage/web-messaging.html)
  describes: "associate the two ports to be entangled, so that they
  form the two parts of a new channel."
- **BO blocker** (per chapter 05 §2.3 H1, repeated in §4.3 below):
  our `MessageChannel`/`MessagePort` is a no-op stub
  (`crates/js_runtime/src/js/window_bootstrap.js:2256-2272`).
  `port1.postMessage(msg)` does NOT deliver `msg` to `port2.onmessage`.
  So recaptcha enterprise.js posts "compute challenge" to `port1`,
  awaits `port2.onmessage`, the response never arrives,
  `grecaptcha.execute()` Promise never resolves, no hydration, page
  sits at 13 KB — 2 033 bytes shy of the chapter-05 strict-pass
  threshold.

#### Arkose Labs (chapter 30)

Arkose's invisible-mode product runs an image-classification model
locally (the visible PuzzleCAPTCHA variant ships the model server-side
and only does the rendering locally). The local model loads via
WebAssembly inside a Worker. For BO purposes the Worker stub
(`window_bootstrap.js:1879-2074`) is functional enough that the
classification can run if it ever does (we haven't measured an Arkose-
protected site flipping on a Worker fix; not corpus-blocking).

#### F5/Shape Defense (chapter 29)

F5/Shape's telemetry collector runs a Worker that pulls timing samples
across an extended observation window (multiple minutes). The Worker
holds `performance.now()` samples between batched POSTs. BO's Worker
scope has its own `performance.now()` (chapter 17 §2.8 line 285:
"worker: `worker_bootstrap.js:134` uses `op_perf_now_humanized`") so
the collector runs; what matters is the timing **consistency** vs
main thread (§4.4).

#### Cloudflare Turnstile (chapter 25 §0.2)

The Turnstile widget can run in a Worker context per its renderer
configuration. The main `api.js` is on the main thread; the actual
PoW + fingerprint compute is in either a Worker or an iframe. The
chapter-07 P2 iframe materialization path covers the iframe variant;
the Worker variant requires the Worker stub to function — which it
does (the duolingo H2 hypothesis, ~20% probability, would tell us if
Worker spawn fails; per chapter 05 it's not the load-bearing issue).

#### Akamai BMP (chapter 26)

Several Akamai-instrumented React sites use a Worker for their
telemetry IPC — per chapter 05 §2.5 "If the H1 fix lands, it likely
also benefits booking (heavy SPA), x.com (heavy SPA), and several
Akamai-instrumented React sites that use MessageChannel for their
telemetry IPC."

### 4.3 BO Worker capability — read of `crates/js_runtime/src/extensions/worker_ext.rs`

`worker_ext.rs` is 532 lines; the architectural pieces:

**Real Worker spawning (✅).** `op_worker_spawn`
(`worker_ext.rs:212-359`) creates a fresh OS thread with a 64 MB
stack and a child `JsRuntime` (via `crate::runtime::create_worker_runtime`,
`worker_ext.rs:281`). The 64 MB stack is justified inline
(`worker_ext.rs:253-254`): "V8's default stack guard isn't large
enough for some anti-bot probes that recurse deeply through wrapped
natives. Chrome's renderer threads also run with ~16 MB stacks; we
go larger because our shim adds more JS frames per native call."
Per-thread `tokio` current-thread runtime (`worker_ext.rs:268-272`).

**Message channels parent↔worker (✅).** Two unidirectional
`std::sync::mpsc` channels (`worker_ext.rs:228-229`).
`op_worker_post_to_worker` (`worker_ext.rs:361-367`) is fire-and-
forget; `op_worker_poll_from_worker` (`worker_ext.rs:372-383`) does
`try_recv`; `op_worker_await_message` (`worker_ext.rs:442-484`) is
the async variant that wakes on a `tokio::sync::Notify` rather than
polling (the W5b-deep fix that unblocked SPA hydration completion
per the file's own comments at line 437-441).

**BlobRegistry for `blob:` URLs (✅).** `op_blob_register` /
`op_blob_fetch_text` / `op_blob_fetch_bytes` / `op_blob_revoke`
(`worker_ext.rs:48-115`). Akamai BMP v3 spawns Workers from blob
URLs (the docstring at `worker_ext.rs:11` cites this directly:
"Akamai's BMP v3 spawns workers via blob: URLs built from inline
scripts").

**Module-worker support (✅).** `op_worker_spawn` accepts an
`is_module: bool` parameter (`worker_ext.rs:221`) and routes through
`load_main_es_module_from_code` when true (`worker_ext.rs:289-313`).
Top-level `import.meta` and module-scoped evaluation work.

**Sync XHR for `importScripts` (✅).** `op_worker_sync_fetch`
(`worker_ext.rs:127-167`) spins a helper thread + a fresh tokio
runtime to avoid nested-block_on panics. Cookie state inherits from
the process-global fetch client.

**Worker reaping on Page::drop (✅).** `WorkerOwnership`
(`worker_ext.rs:431-434`) + `drain_owned_workers` (`worker_ext.rs:415-423`)
ensure Workers don't outlive their parent page. The docstring at
`worker_ext.rs:407-414`: "Without this, workers created via `new
Worker(blob)` keep their OS thread + child `JsRuntime` alive for the
lifetime of the process, leaking ~30 MB per worker."

**What's missing:**

- **`MessageChannel` / `MessagePort` paired-port routing (🟡 NO-OP STUB).**
  `crates/js_runtime/src/js/window_bootstrap.js:2256-2272`. Chapter 17
  §2.5 marks this in bold red as the blocker for duolingo. This is the
  highest-leverage single fix in this chapter.
- **`SharedWorker` (🟡 stub).** `window_bootstrap.js:2045-2063`.
  The `port` field is a literal `{onmessage:null, postMessage(){}, …}`.
  No actual shared isolate. Per chapter 17 §2.5 line 241: "(rare)" —
  not load-bearing for any 126-corpus site.
- **`ServiceWorker` (🟡 stub).** `window_bootstrap.js:2065-2077`.
  No `fetch` event interception. `ServiceWorkerContainer.register`
  resolves but the registration is a no-op
  (`window_bootstrap.js:842-889`).
- **`BroadcastChannel` (🟡 stub).** `window_bootstrap.js:2248-2253`.
  Constructor + `postMessage()` empty body. No cross-context dispatch.
- **`OffscreenCanvas` in Worker (🟡 returns null context).**
  `window_bootstrap.js:4367-4384`. Chapter 17 §2.5 line 251:
  "duolingo image-rec, recaptcha".

### 4.4 The cross-thread fingerprint

The most subtle Worker pattern: **vendors compare the Worker fingerprint
to the main-thread fingerprint and flag inconsistencies as bots.**

Real Chrome:
- `self.navigator.userAgent` in a DedicatedWorker == `window.navigator.userAgent`
- `self.navigator.hardwareConcurrency` == `window.navigator.hardwareConcurrency`
- `self.navigator.deviceMemory` == `window.navigator.deviceMemory`
- `self.navigator.platform` == `window.navigator.platform`
- `self.crypto.subtle.digest(...)` of the same input ≡ main thread's
- Canvas hashing (via `OffscreenCanvas` in Worker) ≡ main thread's canvas

If a defender's profile only patches the main-thread `navigator` and
leaves the Worker `self.navigator` defaulted, the discrepancy flags
the bot. Per [Castle Security's bot detection writeup](https://blog.castle.io/roll-your-own-bot-detection-fingerprinting-javascript-part-1/):
"the more relationships you can validate between fingerprint
attributes, the harder it becomes for attackers to spoof everything
correctly".

#### Audit: what BO emits in main vs Worker

BO's main-thread navigator stack (in `window_bootstrap.js`):

| Property | Main-thread source | Worker source |
|---|---|---|
| `userAgent` | `_defNav('userAgent', () => profile.ua)` (`window_bootstrap.js:~970`) | `worker_bootstrap.js:124` re-applies the masked getter |
| `webdriver` | `window_bootstrap.js:991-995` + `:1657` child realm + `worker_bootstrap.js:124` (chapter 06 §2 candidate 1) | ✅ matched in worker |
| `hardwareConcurrency` | `window_bootstrap.js:974` (chapter 06 §2 candidate 4) | `worker_bootstrap.js:115-137` per chapter 17 §2.8 line 285 |
| `deviceMemory` | `window_bootstrap.js:1031` | (verify in `worker_bootstrap.js`) |
| `userAgentData` | `_defNav('userAgentData', ...)` (`window_bootstrap.js:1841-1844`) | ❓ check — likely missing in worker scope |
| `platform` | `_defNav('platform', ...)` (around `:970`) | ❓ check |
| `language` / `languages` | `_defNav('language', ...)` | ❓ check |
| `permissions` | `cleanup_bootstrap.js:272-278` (chapter 06 §2 candidate 3) | ❓ check |
| `geolocation` | (verify) | n/a (workers don't expose) |
| `serviceWorker` | (chapter 17 §2.5 line 243) | n/a |
| `performance.now()` | `timer_bootstrap.js:170-173` ms-granularity | `worker_bootstrap.js:134` `op_perf_now_humanized` |
| `performance.memory` | `window_bootstrap.js:2333-2337` | `worker_bootstrap.js:142-153` per chapter 17 §2.8 line 290 |
| `crypto.subtle.digest` | `window_bootstrap.js:2969-2979` | (verify worker has it) |
| Canvas hash | `canvas_bootstrap.js:266-585` | only via `OffscreenCanvas` (returns null context — chapter 17 §2.5 line 251) |
| WebGL context | full main-thread impl | only via `OffscreenCanvas` (broken) |
| AudioContext | `audio_seed` (chapter 06 §2 candidate 8) | n/a (typically not in workers) |

**The single concrete inconsistency risk in this table:**
`OffscreenCanvas` in a Worker. A vendor that hashes a canvas in both
contexts compares the hashes; ours returns null in Worker, throws,
and the inconsistency is the flag. Chapter 17 §2.5 line 251 already
calls this out for duolingo + recaptcha.

**Verification step (acceptance §8):**

Write a minimal fingerprint diff probe that runs in both contexts and
asserts equality. Use it as a regression gate:

```js
// probe.js (loadable in both contexts)
function fp() {
    const n = (typeof self !== 'undefined' && self.navigator) || navigator;
    const p = (typeof self !== 'undefined' && self.performance) || performance;
    return {
        ua: n.userAgent,
        hw: n.hardwareConcurrency,
        dm: n.deviceMemory,
        wd: n.webdriver,
        platform: n.platform,
        languages: n.languages && Array.from(n.languages),
        perfMemSize: p.memory && (typeof p.memory.usedJSHeapSize === 'number'),
        // Skip performance.now() — values differ by call order; check
        // only the *type* and ordering monotonicity separately.
    };
}
```

On the main thread: `console.log(JSON.stringify(fp()))`. In a Worker:
post the same JSON to the parent. Compare. Any field that differs
between BO and real Chrome is a candidate fix.

### 4.5 The "Worker that needs main-thread DOM" anti-pattern

A subtle Worker pattern that bites BO today: a Worker computes
something that **needs** a main-thread DOM read (e.g. an
`OffscreenCanvas` snapshot, a font-metrics measurement). In real
browsers, the main thread does the DOM read, postMessage's the result
to the Worker, the Worker hashes it. In BO, the
`MessageChannel`/`MessagePort` no-op breaks the round-trip — the
Worker never receives the DOM read result.

This is the **structural shape** of the duolingo H1 blocker.
recaptcha enterprise.js does:

```js
// Main thread (recaptcha enterprise.js)
const channel = new MessageChannel();
worker.postMessage({cmd: 'compute', port: channel.port2}, [channel.port2]);
channel.port1.onmessage = (e) => {
    // receive computed challenge from worker via the paired port
    if (e.data.token) resolveExecute(e.data.token);
};
// Now main thread does DOM reads and posts results
channel.port1.postMessage({type: 'dom-snapshot', data: someDomData});
```

In real Chrome: `port2` is entangled with `port1`; the Worker receives
`{type:'dom-snapshot', ...}` on `port2.onmessage`; computes; posts
back on `port2`; main thread receives the token on `port1.onmessage`.

In BO today: `port2.postMessage` (from the Worker side, via the
transferred port) is a no-op (our `MessagePort.postMessage` is empty).
Main thread never receives the token. `grecaptcha.execute()` Promise
never resolves.

This is why the chapter 05 §2.5 prioritisation argument holds: fixing
`MessageChannel` paired-port semantics is a **single change** that
fans out across recaptcha + booking + x.com + several Akamai-React
sites + any Worker-PoW vendor that uses paired-port IPC.

---

## 5. The cross-vendor leverage matrix

Restating the TL;DR matrix with concrete vendor counts from the 126
corpus + chapter 18 cookbook inventory:

| Technique | Vendors using it (in our corpus) | Headline sites blocked by gap | BO capability |
|---|---|---|---|
| **PoW execution** | Kasada (3 sites: canadagoose/hyatt/realtor), AWS WAF (5+: amazon-de/in/com-au/jp + imdb), Cloudflare JSC (legacy, rare), Cloudflare MC + Turnstile (6+ on iphone profile per chapter 25), PerimeterX (1: ticketmaster), Akamai sec-cpt (1: homedepot — already self-solves) | None directly — PoW path completes when fingerprint gate doesn't bail | ✅ V8-execute at Chrome speed; **fingerprint gate is the issue** |
| **WASM compute** | AWS WAF (challenge.js), DataDome (3: etsy/tripadvisor/yelp), Cloudflare (Turnstile + Bot Management), Akamai (V3 envelope) | None directly — V8 runs the WASM if the iframe materializes | ✅ V8-native; chapter 17 §line 526 confirms instantiateStreaming round-trips byte-exact |
| **Worker fingerprint computation** | reCAPTCHA enterprise (1: duolingo, plus indirect on every recaptcha-protected site), F5/Shape (rare in corpus), Cloudflare Turnstile (Worker-variant), Akamai BMP (multiple React sites) | duolingo (2,033 bytes shy of pass per chapter 05) | 🟡 Main covered; Worker scope partially stubbed; **cross-thread fingerprint NOT audited** |
| **`MessageChannel` / `MessagePort` paired-port** | recaptcha enterprise, Cloudflare Turnstile (Worker variant), Akamai-React telemetry, Adobe Analytics workers, Sentry browser SDK | duolingo (load-bearing); booking + x.com (suspected) | 🔴 **NO-OP STUB** (window_bootstrap.js:2256-2272) — chapter 17 §2.5 marks this as the highest-leverage gap |
| **`OffscreenCanvas` in Worker** | recaptcha image-rec, duolingo image-rec | duolingo (if Worker canvas hash is the secondary check) | 🟡 stub returns null context (window_bootstrap.js:4367-4384) |
| **`SharedWorker`** | (none in corpus today; growing in adoption) | None | 🟡 stub; out of scope for v0.1.0 |
| **`ServiceWorker` (real)** | (some SPAs use it for offline; rarely the bot detector) | None | 🟡 stub; out of scope for v0.1.0 |
| **`BroadcastChannel`** | (cross-tab sync; rare in corpus) | None | 🟡 stub; out of scope for v0.1.0 |
| **`SharedArrayBuffer` + WASM threads** | (Cloudflare per-zone customs may exercise; rare) | Unknown | ❓ needs COOP/COEP setup; not done today |

The matrix shape: **PoW + WASM are runtime-capability checked
already (V8 handles both)**. **Worker fingerprint consistency + IPC
primitives are the engine work.**

---

## 6. Concrete fixes ranked by leverage

Each fix is sized in lines-of-code (a rough but useful proxy) plus
the vendor/site fan-out. Sorted by `fan-out / loc`.

### Fix 1 — Implement `MessageChannel` + `MessagePort` paired-port routing

**Size:** ~200–500 LOC in JavaScript (no Rust required — the runtime
already gives us `Promise.resolve().then` for microtask scheduling
and `EventTarget` for dispatch).

**Where:** `crates/js_runtime/src/js/window_bootstrap.js:2256-2272`
(replace the no-op stub). Per chapter 05 §2.3 H1 the spec sketch:

> Each `MessagePort` keeps a `_peer` reference; `postMessage(data)`
> enqueues `{type:'message', data}` to the peer's microtask via
> `Promise.resolve().then`; `onmessage`/`message` listeners on the
> peer fire. Buffer messages until `start()` (or implicit start on
> `onmessage = fn` assignment, per HTML spec). Wire both in the
> `MessageChannel` constructor (`port1._peer = port2; port2._peer = port1`).
> Reuse the existing `MessageEvent` from `dom_bootstrap.js:2750` for
> dispatch.

Per the [HTML spec's web-messaging entanglement section](https://html.spec.whatwg.org/multipage/web-messaging.html):
"entangling two `MessagePort` objects, the user agent must associate
the two ports to be entangled, so that they form the two parts of a
new channel" plus the queue semantics: "Each port maintains a 'port
message queue' that can be enabled or disabled. Messages are initially
queued but only dispatched once the queue is enabled — either by
calling `start()` or by setting an `onmessage` handler."

Spec edge to get right: **transferable ports across Worker boundary.**
When `port2` is passed in the transfer list of `worker.postMessage({...},
[port2])`, the Worker receives `port2` as its own MessagePort with the
SAME entanglement to `port1`. This requires the Worker's incoming-
message decoder to recognise transferred ports and recreate them on
the worker side. Our current `worker_ext.rs:362-367` `op_worker_post_to_worker`
is `#[op2(fast)]` taking `#[string] data` — no transferable handling.
Either:

- **Stage 1 (simpler):** implement paired-port for SAME-thread channels
  only (no transfer). This is what the duolingo H1 fix needs because
  recaptcha's `MessageChannel` is created on the main thread and BOTH
  ports stay on the main thread (the Worker uses its own direct
  parent↔worker channel separately). Covers most reCAPTCHA + many
  Akamai-React patterns. ~200 LOC.
- **Stage 2 (more invasive):** also support transferring ports across
  Worker boundaries. Requires a serialisation protocol that the Worker
  side can decode + a per-port registry indexed by an opaque port-id
  that both threads share. ~300 LOC additional.

**Validates against:** the duolingo recipe in chapter 05 §2.4. Bar:
`tag == "L3-RENDERED"` AND `len > 50000`. Today's `len=13327` is
2,033 bytes shy; even a partial hydration fetch landing flips it.

**Fan-out:** duolingo (chapter 05 H1, ~50% probability the fix lands
the site), booking (heavy SPA, suspected), x.com (heavy SPA,
suspected), every Akamai-instrumented React site using MessageChannel
for telemetry IPC (per chapter 05 §2.5), recaptcha-protected forms
on any site, Cloudflare Turnstile (Worker variant), Adobe Analytics
worker bundles, Sentry browser SDK. **Single highest-leverage change
in the chapter.**

**Risk:** message-ordering bugs (microtask vs macrotask choice
matters). The HTML spec says messages dispatch via a task on the
event loop — `Promise.resolve().then` is microtask, not macrotask.
Use `queueMicrotask` if the spec compliance matters (some sites care);
fall back to `setTimeout(0)` if microtask ordering causes re-entrancy.

### Fix 2 — Audit Worker-thread fingerprint consistency vs main-thread

**Size:** ~few days of measurement work; ~50–200 LOC of fixes per
inconsistency found.

**Where:** add a diagnostic probe to the capture tooling (chapter 04
spec); compare BO worker_bootstrap.js output to main-thread output and
to real Chrome 148 capture (same probe).

**Process:**

1. Load the §4.4 probe (`fp()` function) in both contexts.
2. In real Chrome 148: capture `(main_fp, worker_fp)` and compute the
   diff dict — these are the fields that SHOULD differ between main
   and worker in a legitimate browser. (Workers don't expose
   `geolocation`, may not expose `userAgentData`, etc.)
3. In BO: capture the same `(main_fp, worker_fp)` diff dict.
4. The **delta of the deltas** = fields that BO inconsistently emits
   between contexts where real Chrome is consistent. Each = a candidate
   bug.
5. Fix each in `crates/js_runtime/src/js/worker_bootstrap.js`.

**Fan-out:** any vendor that runs the cross-thread fingerprint
consistency check. Cited by Castle Security, FP-Inconsistent (arxiv:
2406.07647), and implicitly used by every modern WAF.

**Risk:** measurement-heavy and slow. Plan budget: 2–3 engineer-days
for the audit + 1–3 days per fix.

### Fix 3 — `OffscreenCanvas` Worker-side rendering

**Size:** ~200–500 LOC in Rust + a worker-context wrapper.

**Where:** `crates/js_runtime/src/js/window_bootstrap.js:4367-4384`
(today returns null context); `crates/canvas/` for the actual render
path (which is main-thread-tied today).

**The challenge:** our canvas implementation in `crates/canvas/` is
DOM-tied. An `OffscreenCanvas` in a Worker has no DOM. We'd need to
either (a) move the canvas render path off the DOM (canvas-as-bitmap
type), or (b) route the canvas op back to the main thread via the
sync-fetch helper pattern (`op_worker_sync_fetch` model) for each
draw call (slow but correct).

**Fan-out:** duolingo + recaptcha image-rec, some F5/Shape telemetry,
emerging Cloudflare per-zone modules.

**Risk:** higher engine surface area; potential V8 isolate boundary
issues.

### Fix 4 — `SharedWorker` real impl

**Size:** ~500–1000 LOC (a new process-global registry + IPC).

**Where:** `crates/js_runtime/src/extensions/worker_ext.rs` (new section);
`crates/js_runtime/src/js/window_bootstrap.js:2045-2063` (replace stub).

**Fan-out:** none in corpus today; emerging vendor pattern (chapter
17 §2.5 line 241: "rare").

**Risk:** complexity. Defer to post-v0.1.0.

### Fix 5 — `BroadcastChannel` real impl

**Size:** ~100–200 LOC.

**Where:** `window_bootstrap.js:2248-2253`.

**Fan-out:** chapter 17 §2.5 line 248: "rare; cross-tab sync". Not
load-bearing.

**Risk:** low.

### Fix 6 — WASM threads & atomics (COOP/COEP headers)

**Size:** ~100 LOC in `crates/net/src/headers.rs` + `crates/browser/src/page.rs`
to wire COOP/COEP awareness.

**Where:** the iframe materialization path needs to honour the COOP +
COEP headers from the parent document; `SharedArrayBuffer` availability
depends on cross-origin-isolated state.

**Fan-out:** unknown — no corpus site currently observed using WASM
threads. Speculative.

**Risk:** low engineering risk; high uncertainty whether any vendor
exercises this in 2026.

### Priority order for v0.1.0

| # | Fix | LOC | Fan-out | Confidence | v0.1.0? |
|---|---|---|---|---|---|
| 1 | MessageChannel paired-port | 200–500 | 5+ sites | high | **YES** |
| 2 | Worker fingerprint audit | few days | most vendors | medium | **YES** (regression gate, not necessarily ship fixes) |
| 3 | OffscreenCanvas in Worker | 200–500 | 2 sites | medium | stretch |
| 4 | SharedWorker real | 500–1000 | 0 today | low | NO |
| 5 | BroadcastChannel real | 100–200 | 0 today | low | NO |
| 6 | WASM threads & atomics | 100 + uncertainty | speculative | low | NO |

---

## 7. Forward-looking — 2027 vendor evolution

Speculative; flagged for anyone picking up this chapter post-v0.1.0.

- **WebGPU compute in Workers.** Chrome 113+ ships
  [`navigator.gpu`](https://developer.mozilla.org/en-US/docs/Web/API/GPU)
  (chapter 17 §2.10 line 316: "❌ MISSING"). Compute shaders are
  orders of magnitude faster than WASM for parallel workloads → stronger
  PoW; GPU-feature-exercising shaders hash to per-GPU values (harder
  to spoof than `UNMASKED_RENDERER_WEBGL` strings); `GPUDevice` is
  usable in Worker context (combines with §4 Worker isolation).
  Adding `navigator.gpu` is multi-month work (wgpu-rs integration or
  fake-GPU shim). Not v0.1.0.
- **WASI in browser.** [WASI](https://wasi.dev/) — WASM with
  filesystem-like / network-like interfaces. Browser-side WASI is
  draft; vendors could use it for richer host interactions. Possibly
  2027+.
- **Workers as primary execution context.** Modern frameworks
  (Astro, Qwik, Cloudflare Workers deployment) thin the main thread.
  If vendors follow, worker_bootstrap.js consistency (§4.4) becomes
  more critical than window_bootstrap.js.
- **PoW + ML hybrid.** Per the 2captcha Kasada writeup, ML models
  "deconstruct the Kasada script, returning perfectly generated
  x-kpsdk-ct and x-kpsdk-cd headers on the fly." Vendors fold ML into
  the verification step — the PoW answer **distribution** (clusters
  around bot-shaped paths?) becomes a signal. Likely 2027.
- **Encrypted Client Hello (ECH) + JA4 evolution.** As ECH rollout
  proceeds, the TLS ClientHello surface JA4 reads becomes less
  informative. Vendors will weight in-V8 signals more — strengthens
  the case for Worker + WASM fingerprinting (above TLS).

---

## 8. Acceptance for v0.1.0

This chapter lands when ALL of the following hold:

- [ ] **MessageChannel + MessagePort implemented with paired-port
  routing** (chapter 05 H1 fix), at least at Stage 1 (same-thread
  channels). Verified by:
  - Unit test in `crates/js_runtime/tests/` that exercises
    `new MessageChannel(); ch.port1.onmessage = fn; ch.port2.postMessage("hello");`
    and asserts `fn` was called with the message.
  - Spec compliance: messages buffer until `start()` or
    `onmessage = fn` assignment, per HTML spec.
  - duolingo strict pass (`len > 50000` per chapter 05 §2.4) OR a
    documented falsification that H1 was not the load-bearing blocker
    (in which case H2/H3/H4 are next).
- [ ] **Worker-thread fingerprint audited for consistency with
  main-thread.** Deliverable: a probe script + a captured BO ↔ real
  Chrome 148 diff matrix. Fields that BO emits inconsistently between
  main and Worker (where real Chrome is consistent) are documented in
  `15_OPEN_QUESTIONS.md`. Top-3 inconsistencies fixed in
  `worker_bootstrap.js`.
- [ ] **WASM execution tested with the AWS WAF challenge.js**
  (chapter 06 §1). Specifically:
  - V8 executes the WASM module loaded by `challenge.js` (verify via
    network-trace + breakpoint at `WebAssembly.instantiate`).
  - Time the WASM `solve()` call against real Chrome 148 in DevTools
    on the same machine; BO should be within 2× (we won't be faster
    because Chrome has GPU-acceleration paths for some operations
    that V8 standalone doesn't use, but 2× wall-clock margin is
    acceptable).
  - Document any WASM feature edge that BO and Chrome 148 differ on
    (per the §3.4 table).
- [ ] **Detection-only logging** for the cross-cutting signals:
  - Log `[vendor-detect] worker-spawn` in `worker_ext.rs:212-359` so
    runs can be triaged by Worker activity. (Today no logging at
    spawn.)
  - Log `[vendor-detect] wasm-instantiate <size>` when V8's
    `WebAssembly.instantiate` is invoked from inside V8 (via the
    bootstrap-wrapped path).
- [ ] **Regression gate**: the `crates/browser/tests/holistic_sweep.rs`
  126-site sweep shows no net regression from the MessageChannel fix
  (target: +1 site = duolingo; tolerable variance ±5 per
  `docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md`).
- [ ] **`15_OPEN_QUESTIONS.md` updated** with the residual gaps:
  WebGPU absence, SharedWorker / ServiceWorker stub-only status,
  COOP/COEP not wired, BroadcastChannel no cross-context dispatch.

Stretch goals (NOT required for v0.1.0):

- [ ] `OffscreenCanvas` in Worker returns a real context (fix 3).
- [ ] WebGPU `navigator.gpu` stub that returns plausible adapter info
  (would help anti-bot probes that test for WebGPU presence; not a
  full implementation).
- [ ] `BroadcastChannel` real cross-context dispatch (fix 5).

---

## 9. Files referenced

### In the public engine (read-only for this chapter)

- `crates/js_runtime/src/extensions/worker_ext.rs:1-532` — the Worker
  Rust impl. All of: `op_worker_spawn` (`:212-359`),
  `op_worker_post_to_worker` (`:361-367`),
  `op_worker_poll_from_worker` (`:372-383`),
  `op_worker_terminate` (`:385-388`),
  `op_worker_await_message` (`:442-484`),
  `op_worker_self_post` (`:490-501`),
  `op_worker_self_recv` (`:503-513`),
  `op_worker_sync_fetch` (`:127-167`),
  blob registry (`:48-115`),
  `WorkerOwnership` + `drain_owned_workers` (`:415-434`).
- `crates/js_runtime/src/js/window_bootstrap.js:17-27` —
  WebAssembly.{instantiate,compile}Streaming wrappers.
- `crates/js_runtime/src/js/window_bootstrap.js:1857-2044` —
  `Worker` class (real impl on top of `op_worker_spawn`).
- `crates/js_runtime/src/js/window_bootstrap.js:2045-2063` —
  `SharedWorker` stub.
- `crates/js_runtime/src/js/window_bootstrap.js:2064-2074` —
  `ServiceWorker` stub.
- `crates/js_runtime/src/js/window_bootstrap.js:2075-2082` —
  `WorkerGlobalScope` / `DedicatedWorkerGlobalScope` stubs.
- `crates/js_runtime/src/js/window_bootstrap.js:2248-2253` —
  `BroadcastChannel` stub.
- `crates/js_runtime/src/js/window_bootstrap.js:2256-2272` —
  **`MessageChannel` + `MessagePort` no-op stubs (the fix-1 target)**.
- `crates/js_runtime/src/js/window_bootstrap.js:842-889` —
  `ServiceWorkerContainer` stub on navigator.
- `crates/js_runtime/src/js/window_bootstrap.js:4367-4384` —
  `OffscreenCanvas` stub (returns null context).
- `crates/js_runtime/src/js/worker_bootstrap.js` — Worker-side `self`,
  postMessage, importScripts, performance wrappers, navigator
  re-application.
- `crates/js_runtime/src/runtime.rs:1-130` — JsRuntime construction
  + extension list. `worker_extension::init_ops()` at line 127.
- `crates/browser/src/classify.rs:81-167` — current vendor marker
  tables.
- `crates/browser/src/page.rs:1054-1069` — vendor-detect log block.

### Other v0.1.0-parity chapters

- `05_SPA_HYDRATION_CLUSTER.md` — chapter 05 §2 duolingo recipe,
  §2.3 H1 MessageChannel hypothesis, §2.5 leverage argument.
- `06_AWS_WAF_SOLVER.md` — chapter 06 §0 protocol, §1 challenge.js
  capture, §2 fingerprint candidate signals (including §2 candidates
  12 + 13 for WASM and Workers), §3 solver alternatives.
- `07_DATADOME_PRIMITIVES.md` — chapter 07 Primitives 1 (CSP relax),
  2 (iframe materialize), 3 (clearance cookie retry). The WASM-iframe-
  daily-key is the §2 reference.
- `08_KASADA_FRONTIER.md` — chapter 08 §"Research arc" Phase 1–5,
  K2-DIFF (Lever 1), CSS calc math (Lever 2), `_maskAsNative` sweep
  (Lever 3), `bot1225` Web API (Lever 4).
- `17_WEB_API_PARITY_MATRIX.md` — chapter 17 §2.5 Workers
  (line 240–253), §2.8 Performance, §"WebAssembly" line 56 + 526.
- `18_ANTI_BOT_VENDOR_COOKBOOK.md` — chapter 18 vendor inventory
  (PoW + WASM + Worker users).
- `25_CLOUDFLARE_DEEP.md` — chapter 25 §0 four products, §2.1
  Managed/JSC shared orchestrator, §2.2 Turnstile, §4.4 Primitive 4
  (token relay).
- `26_AKAMAI_BMP_DEEP.md` — chapter 26 V3 envelope (WASM-derived).

### External documentation

- [WebAssembly Core Specification](https://webassembly.org/specs/) —
  binary format, instruction set, host interface.
- [HTML Living Standard — Web Workers](https://html.spec.whatwg.org/multipage/workers.html)
  — DedicatedWorker, SharedWorker, ServiceWorker definitions; event
  loop; same-origin policy.
- [HTML Living Standard — Web Messaging](https://html.spec.whatwg.org/multipage/web-messaging.html)
  — `MessageChannel`, `MessagePort`, entanglement, message-queue
  enable / dispatch semantics.
- [W3C Service Workers spec](https://www.w3.org/TR/service-workers/)
  — `ServiceWorkerContainer.register`, `Cache`, `FetchEvent`.
- [W3C OffscreenCanvas spec](https://html.spec.whatwg.org/multipage/canvas.html#the-offscreencanvas-interface)
  — `OffscreenCanvas`, `transferControlToOffscreen`, worker-side
  rendering.
- [WebAssembly Binary Toolkit (WABT)](https://github.com/WebAssembly/wabt)
  — `wasm2wat`, `wat2wasm`, `wasm-objdump`, `wasm-decompile`.
- [Wasmtime](https://wasmtime.dev/) — standalone WASM runtime, used
  in chapter 06 §3 Alternative B for the load-the-WASM-out-of-V8
  pattern.
- [lwthiker/curl-impersonate](https://github.com/lwthiker/curl-impersonate)
  — TLS impersonation reference (relevant context for the
  fingerprint-class layering BO sits inside; per chapter 23).

### Anti-bot research (per-technique reverse engineering)

#### Kasada

- [2captcha — Kasada deep-dive](https://2captcha.com/h/kasada-bypass)
  — VM architecture, PoW timing, x-kpsdk header set.
- [ScrapeBadger — Kasada bypass](https://scrapebadger.com/kasada-bypass)
  — p.js VM analysis, x-kpsdk-ct/cd computation.
- [ChrisYP/ChrisYP.github.io kasada.md](https://github.com/ChrisYP/ChrisYP.github.io/blob/main/en-US/kasada.md)
  — VM bytecode decoder, string-table layout.
- [lktop/kpsdk](https://github.com/lktop/kpsdk) — x-kpsdk-ct +
  x-kpsdk-cd encryption algorithm analysis.
- [unicorn-aio/kpsdk](https://github.com/unicorn-aio/kpsdk) — full
  ips.js compat layer.

#### DataDome

- [glizzykingdreko — Breaking down Datadome](https://medium.com/@glizzykingdreko/breaking-down-datadome-captcha-waf-d7b68cef3e21)
  — daily-key rotation, signals dictionary structure.
- [DataDome changelog — VM-based obfuscation 2026](https://datadome.co/changelog/vm-based-obfuscation/)
  — vendor's announcement of the third defence layer (JS VM + WASM
  + dynamic).
- [Security Boulevard analysis 2026-02](https://securityboulevard.com/2026/02/datadome-releases-vm-based-obfuscation-the-next-evolution-in-client-side-detection-security/)
  — third-party walkthrough.
- [xKiian/datadome-vm](https://github.com/xKiian/datadome-vm) —
  reverse engineering of the new DataDome VM.
- [glizzykingdreko/Datadome-Interstitial-Deobfuscator](https://github.com/glizzykingdreko/Datadome-Interstitial-Deobfuscator)
  — JS deobfuscator toolchain.

#### AWS WAF

- [AWS — JS challenge API](https://docs.aws.amazon.com/waf/latest/developerguide/waf-js-challenge-api.html)
  — challenge.js integration spec.
- [AWS — challenge & CAPTCHA actions blog](https://aws.amazon.com/blogs/networking-and-content-delivery/protect-against-bots-with-aws-waf-challenge-and-captcha-actions/)
  — token + fingerprint hash + puzzle-type description.
- [neiii/aws-waf-solver](https://github.com/neiii/aws-waf-solver) —
  HashcashScrypt / SHA-256 / NetworkBandwidth flavours.
- [Switch3301/Aws-Waf-Solver](https://github.com/Switch3301/Aws-Waf-Solver)
  — alternate solver.
- [arxiv:2508.21219 — The WASM Cloak](https://arxiv.org/html/2508.21219v1)
  — academic, browser fingerprinting under WASM obfuscation.

#### Cloudflare

- [Cloudflare — JavaScript Detections](https://developers.cloudflare.com/cloudflare-challenges/challenge-types/javascript-detections/)
- [Cloudflare — Clearance concept](https://developers.cloudflare.com/cloudflare-challenges/concepts/clearance/)
- [Cloudflare — challenges overview](https://developers.cloudflare.com/fundamentals/security/cloudflare-challenges/)
- [Cloudflare — Detect response](https://developers.cloudflare.com/cloudflare-challenges/challenge-types/challenge-pages/detect-response/)
- [Cloudflare — Turnstile widget configurations](https://developers.cloudflare.com/turnstile/get-started/client-side-rendering/widget-configurations/)
- [Cloudflare — Bot Fight Mode](https://developers.cloudflare.com/bots/get-started/bot-fight-mode/)
- [CaptchaAI — CF challenge session flow walkthrough](https://blog.captchaai.com/cloudflare-challenge-session-flow-walkthrough)
- [Khromov 2013 — How CF client-side DDoS detection works](https://khromov.wordpress.com/2013/02/05/cloudflare-client-side-ddos-detection-how-does-it-work/)
  — JSC algorithm overview, jschl_vc/jschl_answer semantics.
- [scaredos/cfresearch](https://github.com/scaredos/cfresearch) —
  open research repo on anti-DDoS systems incl. CF.

#### Cross-thread fingerprinting

- [Castle Security — roll your own bot detection part 1](https://blog.castle.io/roll-your-own-bot-detection-fingerprinting-javascript-part-1/)
  — cross-context validation, navigator consistency checks.
- [arxiv:2406.07647 — FP-Inconsistent](https://arxiv.org/pdf/2406.07647)
  — measurement of fingerprint inconsistencies in evasive bot traffic.
- [niespodd/browser-fingerprinting](https://github.com/niespodd/browser-fingerprinting)
  — countermeasures catalogue.
- [BotBrowser — WebGL fingerprinting](https://botbrowser.io/en/blog/webgl-fingerprinting/)
  — OffscreenCanvas-in-Worker fingerprint consistency.

### Internal historical context

- `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/kasada_wrapper_cracked_and_remaining_leaks.md`
  — Kasada wrapper analysis (Phase 1 of chapter 08).
- `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/kasada_real_blocker_css_calc_math.md`
  — CSS calc math gap (Phase 2 of chapter 08).
- `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/state_2026_05_14_v3_envelope_measured.md`
  — Akamai V3 envelope measurement.
- `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/state_2026_05_16_phase5_datadome.md`
  — DataDome phase 5 (homedepot flip, sec-cpt self-solve confirmed).
- `~/projects/browser_oxide_internal/` — private vendor_solvers crate
  + captures (per chapters 06 § 5 and 08).

### Commit references

- `aecdf19` — the vendor-strip commit (chapter 07 §"Context").
  Removed `crates/akamai/`, `crates/net/src/kasada_session.rs`, the
  vendor handlers from `page.rs`. The boundary this chapter respects.
- `f62584d` — SharedSession (cookie jar process-wide). Relevant for
  any vendor solver that needs to write clearance cookies.
- `d00bcb2` — `BOXIDE` → `BROWSER_OXIDE` env-var rename. Use
  `BROWSER_OXIDE_DEBUG_NAV=1` in any new capture scripts.

---

## 10. The honesty section

This chapter is **planning, not prescription**. Caveats:

1. **Fan-out numbers are estimates.** "5+ sites" for AWS WAF assumes
   the amazon variants + imdb all benefit from one fix; they share a
   vendor, not necessarily the load-bearing signal. Chapter 06 §4
   acceptance is per-site.
2. **MessageChannel fix may not be load-bearing for duolingo.** Per
   chapter 05 §2.3, H1 is at ~50%. H2/H3/H4 are the alternatives.
   Even if H1 lands the fix, duolingo is 2,033 bytes shy — a single
   additional hydration call needs to land, not just an unblock.
   Plan acceptance as "duolingo flips" but tolerate "duolingo grows
   past 30 KB" as progress.
3. **WASM execution speed claims are unverified.** "BO V8 ~Chrome
   speed" rests on deno_core 0.311 using the same V8 Chrome builds
   ship. Actual measurement is a v0.1.0 task; if BO is materially
   slower, §2.5's "PoW capability is necessary but not sufficient"
   conclusion may be wrong and PoW speed becomes a bottleneck.
4. **Worker fingerprint audit may surface 20+ inconsistencies.** The
   §4.4 table is illustrative; the actual audit is exploratory. The
   "top-3 inconsistencies fixed" acceptance criterion is the floor; a
   thorough audit may take an engineer-month.
5. **§7 is speculative.** WebGPU compute-shader fingerprinting and
   WASI-in-browser may not become mainstream in 2027. Flagged so the
   multi-month engine work can be budgeted early if needed.
6. **Vendor rotation invalidates research half-lives.** DataDome's
   2026-01-14 VM rollout reset the reverse-engineering set; Kasada's
   Phase 3 16-field inventory is from May 2026. Re-pull captures
   before relying on field-level conclusions.
7. **License boundary is firm.** Per `CLAUDE.md` and chapter 06
   §"Out-of-scope": no AWS-WAF / DataDome / Kasada bypass code in the
   public engine. The MessageChannel fix, Worker fingerprint audit,
   and WASM feature-detect probe are engine-generic and DO belong
   public. Test: does the fix have a Chrome-faithfulness
   justification independent of any vendor? Yes → public. No →
   private `vendor_solvers`.
8. **This chapter does NOT propose a new vendor solver.** It proposes
   capability improvements to the engine. Vendor-specific work stays
   in chapters 06 / 07 / 08 / 25 / 26 and in `vendor_solvers`.

Acceptance is gated by §8. Anything beyond is stretch.

---
