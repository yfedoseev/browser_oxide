# Web API parity deep dive — Workers, crypto, blob

**Scope:** Web Worker / SharedWorker / Worklet spawn + lifecycle;
MessageChannel / MessagePort paired routing; postMessage +
structuredClone; Blob + `URL.createObjectURL(blob)` for blob-worker
URLs; `crypto.subtle` (digest / sign / verify / encrypt / deriveKey);
`crypto.getRandomValues` / `randomUUID`; worker-realm fingerprint
identity (worker `navigator` must match `window`).

**Why it matters (corpus):** the AWS-WAF cluster (amazon-ca/com/com-au/
fr/in/jp + imdb = 7 sites) and duolingo. Per `docs/HANDOFF_2026_05_28b.md`
§4, challenge.js's proof-of-work runs in a **blob-URL Web Worker** (no
`WebAssembly.*` at all — `new Worker` ×2 + `new Blob` ×2). The PoW path
is gated by the engine's worker + async-drain behaviour, not by
fingerprint.

**Author's verification stance:** every BO claim below is checked against
source at `file:line`. Several prior-doc conclusions are now **STALE** and
are corrected in §1.

---

## 0. Headline (read this first)

1. **The single biggest correction to prior docs:** `MessageChannel` /
   `MessagePort` paired-port routing is **NO LONGER a no-op stub.** It was
   implemented on 2026-05-26 in commit `f3ea599` ("feat(window): real
   MessageChannel/MessagePort routing — v0.1.0-parity Fix 8"),
   `crates/js_runtime/src/js/window_bootstrap.js:2314-2427`. Docs
   `41_POW_WASM_WORKER_PATTERNS.md` §4.3/§6 (Fix 1) and
   `17_WEB_API_PARITY_MATRIX.md` §2.5 that call this "🔴 NO-OP STUB
   (window_bootstrap.js:2256-2272)" are **out of date.** The fix is
   Stage-1 only (same-thread channels); the Stage-2 gap (transferring a
   port across the real Worker thread boundary) remains — see §4.
2. **The blob: URL polyfill bug (vNext/10) appears FIXED.** The worker
   test `worker_self_location_populated_from_construction_url` now asserts
   `protocol == "blob:"` and `origin == "null"` (no longer relaxed) —
   `crates/js_runtime/tests/worker.rs:136-141`. vNext/10 should be
   re-validated and closed.
3. **The worker secure-context fix is in** (commit `5216336`): workers
   spawned from https/blob:https pages now inherit `is_secure_context`, so
   `crypto.subtle` / `crypto.randomUUID` survive `cleanup_bootstrap.js`'s
   SecureContext gate inside workers. Confirmed at
   `worker_ext.rs:240,296` → `runtime.rs:314-358`.
4. **The real remaining AWS-WAF lever is the live-nav async drain**
   (`crates/browser/src/page.rs:3566`, a 50 ms inter-script
   `run_until_idle`). The offline oracle runs challenge.js to
   `forceRefreshToken`; the live navigate path produces **zero** async
   progress (no worker spawn, no token POST). This is public-engine,
   highest-ROI, and is the top fix in §6.
5. **`crypto.subtle` is digest-only.** `sign/verify/encrypt/decrypt/
   deriveKey/deriveBits/generateKey/importKey/exportKey/wrapKey/unwrapKey`
   all return rejected Promises (`NotSupportedError`). True in BOTH the
   window realm (`window_bootstrap.js:3177-3182`) and the worker/shared
   realm (`shared_apis_bootstrap.js:120-125`). This is a latent parity
   gap, not currently corpus-blocking, but is a cross-realm consistency
   risk and a forward-looking liability.

---

## 1. What the existing repo docs concluded (and what is now stale)

### 1.1 `41_POW_WASM_WORKER_PATTERNS.md`

The canonical cross-cutting chapter. Its conclusions, audited:

- §4.1–§4.4: workers give vendors three properties (less interception,
  less observability, cross-thread fingerprint consistency check). The
  **cross-thread consistency check** is the load-bearing risk: vendors
  compare `self.navigator.*` in the worker to `window.navigator.*` and
  flag divergence. **Still accurate.** (Sources cited there: Castle
  Security blog, arxiv:2406.07647 FP-Inconsistent.)
- §4.3 "What's missing": lists `MessageChannel`/`MessagePort` as
  "🟡 NO-OP STUB" and ranks it Fix 1 (§6). **STALE** — implemented in
  `f3ea599` (see §0.1 above and §4 below).
- §2.5 "PoW gate is upstream": claims AWS/Kasada bail on a *fingerprint*
  gate before PoW. **Superseded for AWS** by `HANDOFF_2026_05_28b` §4:
  AWS does NOT fingerprint-bail — challenge.js proceeds to
  `forceRefreshToken` with BO's fingerprint in the oracle. The blocker is
  the live-nav drain, not a fingerprint gate. (Kasada's post-PoW sensor
  verify is a separate story, doc 08.)
- §3 WASM: V8 runs WASM natively; the AWS challenge.js was assumed
  WASM-PoW. **Corrected** by `HANDOFF_2026_05_28b` §4.3: `grep
  challenge.js` shows **zero `WebAssembly.*`** — the PoW is a JS
  blob-worker, not WASM. The §3.4 "WASM threads & atomics / COOP-COEP"
  gap is real but not AWS-relevant.
- §6 Fix 2 "Worker fingerprint audit": still valid and partly done in
  `worker_bootstrap.js` (UA, UAData, hardwareConcurrency, deviceMemory,
  languages, platform, webdriver, performance.memory all re-applied).

### 1.2 `17_WEB_API_PARITY_MATRIX.md` §2.5

Marks MessageChannel paired routing as the headline gap. **STALE** for the
same reason. `SharedWorker`, `ServiceWorker` (real), `BroadcastChannel`,
`OffscreenCanvas`-in-worker remain stubs — those statuses are still
accurate (verified §4.6 below).

### 1.3 `vNext/10_URL-polyfill-blob.md`

Side-finding during R-DUO-WORKER (`967b4dc`): `new URL("blob:…").protocol`
returned `""` instead of `"blob:"`. Step-2 fix (opaque-scheme detection in
the URL polyfill at `shared_apis_bootstrap.js`). **Appears DONE** — the
worker test now asserts `protocol == "blob:"` un-relaxed
(`worker.rs:132-141`). Recommend closing vNext/10 after a confirmation run
of the §2 scheme-matrix probe.

### 1.4 `WORKERS.md`

Design doc for a (never-built) standalone `workers/` crate. The shipped
implementation lives instead in
`crates/js_runtime/src/extensions/worker_ext.rs` (Rust) +
`crates/js_runtime/src/js/worker_bootstrap.js` (worker-realm JS) +
`window_bootstrap.js` (the `Worker`/`MessageChannel`/etc. constructors).
The "own V8 Isolate per worker" design from WORKERS.md was realised — each
`new Worker(url)` spawns an OS thread with a child `JsRuntime`
(`worker_ext.rs:268-362`).

### 1.5 `HANDOFF_2026_05_28b.md` §4 / §5.1 — the live state

Four-layer peel of the AWS gap:
1. NOT a fingerprint bail (oracle reaches `forceRefreshToken`).
2. NOT WebGL (FIX-D2 changed nothing — imdb/amazon-in unchanged).
3. NOT WASM (challenge.js has zero `WebAssembly.*`; PoW is a blob worker;
   prerequisite worker `crypto.subtle` was `undefined` → fixed `5216336`).
4. **The lever:** in the live navigate path, challenge.js produces zero
   async progress because `build_page_with_scripts_init_and_storage` runs
   external scripts with only a **50 ms inter-script drain** (page.rs:3566)
   and then the nav loop re-fetches before challenge.js's
   `checkForceRefresh().then(...)` chain (which creates the worker) can
   advance. The offline oracle gives it `run_until_idle(5 s)` and it
   completes.

---

## 2. External findings (fresh research)

### 2.1 AWS WAF challenge.js mechanics (confirms HANDOFF §4)

- challenge.js is injected with the **`defer` attribute** and runs after
  parse; it decodes encrypted params (`awsKey`, `awsIv`, `awsContext`,
  `awsChallengeJS`) and runs a client-side PoW
  ([AWS WAF JS challenge API](https://docs.aws.amazon.com/waf/latest/developerguide/waf-js-challenge-api.html);
  [AWS WAF challenge/CAPTCHA blog](https://aws.amazon.com/blogs/networking-and-content-delivery/protect-against-bots-with-aws-waf-challenge-and-captcha-actions/)).
- PoW flavours: **HashcashScrypt, SHA-256, NetworkBandwidth**
  ([neiii/aws-waf-solver](https://github.com/neiii/aws-waf-solver),
  [Switch3301/Aws-Waf-Solver](https://github.com/Switch3301/Aws-Waf-Solver)).
  Solvers explicitly note challenge.js "processes the proof-of-work
  challenges using **Web Workers or Blob-based approaches** to offload
  computation" — exactly the blob-worker path BO must drive.
- Success → time-sensitive `aws-waf-token` cookie; WAF returns **HTTP 202**
  on the interstitial. **Implication for BO:** the cookie-gain re-fetch
  (the `__pendingNavigation` path) only fires if the worker computes the
  token and POSTs it *before* the drain ends. This is the §6 Fix 1 target.

The `defer` + worker offload is the crux: a `defer` script's
`.then()`-chain plus a `new Worker(blobURL)` requires the page event loop
to keep ticking for hundreds of ms, but BO caps the inter-script drain at
50 ms and the final drain at 8 s — except the worker is created *inside*
the deferred chain that hasn't advanced when the 50 ms cap fires.

### 2.2 Camoufox worker-realm parity (the SOTA comparison)

Via deepwiki on `daijro/camoufox`, the `navigator-spoofing.patch`:

- Patches **`WorkerNavigator::GetPlatform / GetUserAgent /
  HardwareConcurrency`** in C++ (`WorkerNavigator.cpp`), reading per-
  context values from `NavigatorManager` keyed by `userContextId`, with a
  3-tier fallback (per-context → `CAMOU_CONFIG` → vanilla Firefox).
- **Blob: URL workers** resolve `userContextId` via
  `WorkerPrivate::GetOriginAttributes()`, which inherits the creating
  context's `BrowsingContext` — so a blob-worker reports the **same**
  spoofed platform/UA/hardwareConcurrency as the parent page.
- **Notable Camoufox gap:** the patch does **NOT** hook
  `navigator.deviceMemory` or `navigator.languages` in the worker realm.

**Parity verdict:** BO's `worker_bootstrap.js` re-applies UA, appVersion,
language, **languages**, platform, hardwareConcurrency, **deviceMemory**,
userAgentData, webdriver=false (`worker_bootstrap.js:86-161`). So on
`deviceMemory` and `languages` in the worker realm, **BO is actually more
complete than Camoufox.** The risk for BO is *consistency of the source of
truth* (window vs worker reading the same profile), not coverage — see
§4.4.

### 2.3 Cross-thread fingerprint inconsistency is a real detector

[Castle Security](https://blog.castle.io/roll-your-own-bot-detection-fingerprinting-javascript-part-1/):
"Detection systems can identify inconsistencies across JavaScript
execution contexts: main page, iframes, and web workers." DataDome's
tags.js v5.6.3 specifically reads `navigator.userAgentData?.mobile` in a
worker — BO already handles this exact case (worker returns `false` to
match main, per the comment at `worker_bootstrap.js:90-93`).
[arxiv:2406.07647 FP-Inconsistent](https://arxiv.org/pdf/2406.07647)
systematises cross-space inconsistency detection.

### 2.4 HTML spec references (for the fixes in §6)

- [Web Messaging — MessagePort entanglement & queue enable/dispatch](https://html.spec.whatwg.org/multipage/web-messaging.html):
  ports are entangled; each port's message queue is disabled until
  `start()` or an `onmessage` setter / `addEventListener('message')`
  enables it; messages dispatch via a **task on the event loop**.
- [Web Workers](https://html.spec.whatwg.org/multipage/workers.html):
  workers inherit the owner's secure context; transferred `MessagePort`s
  in the structured-clone transfer list must be re-created entangled on
  the receiving side.
- [WHATWG URL — blob URLs](https://url.spec.whatwg.org/#blob-url):
  opaque-scheme parsing (`blob:`, `data:`) — protocol must be the scheme,
  origin `"null"`.

---

## 3. BO code-level analysis — crypto + blob

### 3.1 crypto Rust ops — `crates/js_runtime/src/extensions/crypto_ext.rs`

Two ops only:
- `op_crypto_digest` (`crypto_ext.rs:9-36`): SHA-1/256/384/512 via the
  `sha1`/`sha2` crates. Returns `Vec::new()` for **unknown algorithms** —
  silent empty buffer, not an error. A vendor passing `"SHA-3-256"` or a
  malformed name would get a zero-length digest, not a throw. (Real Chrome
  rejects with `NotSupportedError`.) Minor fidelity gap.
- `op_crypto_random_fill` (`crypto_ext.rs:38-42`): `rand::rng().fill_bytes`
  — a real CSPRNG. Good.

There is **no Rust op for HMAC, sign, verify, encrypt, decrypt, or key
derivation** despite the file docstring mentioning HMAC.

### 3.2 crypto JS surface — window realm (`window_bootstrap.js:3131-3214`)

- `crypto.subtle.digest` (`:3163-3173`): bridges to `op_crypto_digest`,
  returns `Promise<ArrayBuffer>`. Correct shape. Coerces `BufferSource`
  via `_toBytes` (`:3155-3161`).
- **All other SubtleCrypto methods are rejection stubs**
  (`:3177-3182`): `sign, verify, encrypt, decrypt, generateKey, importKey,
  exportKey, deriveKey, deriveBits, wrapKey, unwrapKey` →
  `Promise.reject(new DOMException(name+" not implemented",
  "NotSupportedError"))`.
- `getRandomValues` (`:3185-3196`): fills via op; enforces 64 KiB quota;
  returns the input view. **Does not validate the typed-array element
  type** (spec throws `TypeMismatchError` for `Float32Array` etc.) — minor.
- `randomUUID` (`:3197-3206`): correct RFC-4122 v4 formatting from 16
  random bytes.
- `subtle` getter (`:3207`) returns the shared `_subtleInstance`.

### 3.3 crypto JS surface — worker / shared realm (`shared_apis_bootstrap.js:92-142`)

Identical surface, **separately defined** (digest works; everything else
rejects). Run inside workers via `runtime.rs:380`. So a blob-worker that
does `crypto.subtle.digest("SHA-256", bytes)` for AWS's SHA-256 PoW
flavour **will get a real digest** — confirmed by the HANDOFF in-VM probe
("worker now has subtle (digest 32B)"). But a worker doing
HashcashScrypt that calls `deriveBits`/`deriveKey` would get a rejection.
AWS's "SHA-256" flavour is satisfiable; the "HashcashScrypt" flavour is
**not** (scrypt would have to be JS-side in challenge.js, which is fine —
the worker just needs `digest`).

**Cross-realm consistency note:** the window and worker SubtleCrypto are
two independent class definitions with the same behaviour, so
`digest`-equality across realms holds (both call the same Rust op on the
same bytes → identical output). Good — this is exactly the §2.3 cross-
thread check vendors run.

### 3.4 Secure-context gating — `cleanup_bootstrap.js:52-74`

`crypto.subtle` and `crypto.randomUUID` are `[SecureContext]`. A Proxy on
`crypto` returns `undefined` for `subtle`/`randomUUID` when the context is
NOT secure (`:60,65,70,74`). Pre-`5216336`, workers never inherited the
secure flag, so the proxy hid `subtle` in every worker → AWS blob-worker's
`crypto.subtle` was `undefined` → silent bail. **Fixed:** `is_secure_context`
flows `op_worker_spawn` (`worker_ext.rs:240`) → `create_worker_runtime(...,
is_secure_context)` (`worker_ext.rs:296`) → `StealthState::new_with_flags`
(`runtime.rs:351-358`). The worker `cleanup_bootstrap` now sees secure=true
on https pages and leaves `subtle` exposed.

### 3.5 Blob + `URL.createObjectURL` — `worker_ext.rs:23-115`

A process-global `BlobRegistry` (`worker_ext.rs:32-43`) backs
`URL.createObjectURL(blob)`:
- `op_blob_register` (`:48-62`): stores bytes + content-type under a
  `blob:` URL string (the URL itself is minted JS-side at
  `window_bootstrap.js:4399` / `shared_apis_bootstrap.js:313` as
  `blob:<origin>/<uuid>`).
- `op_blob_fetch_text` (`:67-75`) / `op_blob_fetch_bytes` (`:93-108`):
  resolve a blob URL to its source (text for worker spawn / importScripts;
  bytes for `fetch(blob:)`).
- `op_blob_revoke` (`:111-115`).

When `new Worker(blobURL)` runs, `_resolveWorkerScript`
(`window_bootstrap.js:1860-1877`) calls `op_blob_fetch_text` to get the
worker source, then `op_worker_spawn` (`:1916`). This is the exact AWS
path. **It works** (HANDOFF: "Basic blob-URL worker round-trips already
worked"; tests `worker_echo_round_trip` + `worker_addeventlistener_roundtrip`
in `worker.rs:46,67` spawn via `URL.createObjectURL`).

### 3.6 Blob URL polyfill protocol (vNext/10)

`new URL("blob:null/uuid")` now returns `protocol="blob:"`,
`origin="null"` (test `worker.rs:132-141` asserts this un-relaxed). The
worker installs `self.location` from this URL
(`worker_bootstrap.js:38-63`) so `self.location.protocol === "blob:"`
matches real Chrome — closing the recaptcha-enterprise cross-check that
vNext/10 flagged.

---

## 4. BO code-level analysis — Workers + MessageChannel

### 4.1 Worker spawn + lifecycle — `worker_ext.rs:219-374`

`op_worker_spawn`:
- Resolves the stealth profile (`:232-235`) and inherits secure-context
  (`:240`).
- Two unidirectional `std::sync::mpsc` channels (`:241-242`) + an
  `AtomicBool` terminate flag (`:243`) + a `tokio::sync::Notify`
  (`:244`).
- Spawns an OS thread with a **64 MB stack** (`:268-271`, justified for
  deep native-shim recursion), builds a current-thread tokio runtime
  (`:282-291`) and a child `JsRuntime` via `create_worker_runtime`
  (`:295-296`).
- Module vs classic worker (`:303-331`): `type:'module'` →
  `load_main_es_module_from_code`; classic → `execute_script`.
- **Event-loop pump** (`:336-357`): `run_event_loop` in 25 ms timeout
  ticks, sleeping 5 ms between idle ticks, until terminated.

**Worker reaping** (`drain_owned_workers`, `:430-438`) is wired into
`Page::drop` (`page.rs:284`) so workers don't leak their 64 MB stack +
isolate.

### 4.2 Parent↔worker messaging — `worker_ext.rs:376-547`

- `op_worker_post_to_worker` (`:376-382`): fire-and-forget, takes a
  `#[string] data` (JSON). **No transferable handling** — this is the
  Stage-2 limitation (§4.5).
- `op_worker_poll_from_worker` (`:387-398`): sync `try_recv`.
- `op_worker_await_message` (`:457-499`): async, awaits the `Notify` — the
  W5b-deep fix that stopped the perpetual `setInterval(5)` from pinning
  the event loop and blocking SPA hydration detection.
- Worker side: `op_worker_self_post` (`:505-516`) notifies the parent;
  `op_worker_self_recv` (`:518-528`) drained by the worker's
  `setInterval(drainOnce, 5)` (`worker_bootstrap.js:261`).
- `op_worker_self_url` (`:537-547`) backs `self.location` (§3.6).

Messages are JSON-wrapped (`{data: <wire>}`) where `<wire>` is produced by
`serializeForWire` (structured_clone.js) — so ArrayBuffer/TypedArray/
Map/Set/Date/RegExp survive the JSON hop. DataCloneError propagates for
functions/symbols (`structured_clone.js:48-101`).

### 4.3 MessageChannel / MessagePort — `window_bootstrap.js:2314-2427` (IMPLEMENTED, Stage 1)

The actual current implementation (NOT the stub prior docs describe):
- Four `WeakMap`s track per-port state: `_PortPaired`, `_PortQueue`,
  `_PortEnabled`, `_PortClosed` (`:2330-2333`).
- `_enable(port)` (`:2344-2356`): sets the started flag and **drains the
  pre-start queue synchronously** (a deliberate deviation from the spec's
  event-loop-task dispatch — see Risk below).
- `_deliver(port, data)` (`:2358-2374`): if not enabled, buffer in the
  queue; else dispatch a real `MessageEvent` via `port.dispatchEvent`.
- `MessagePort` (`:2376-2411`): `set onmessage` enables dispatch (`:2386`);
  `postMessage` clones via `structuredClone` and delivers **to the paired
  port** (`:2388-2395`); `start()` enables (`:2396`); `close()` detaches
  the pair (`:2397-2404`); `addEventListener('message', …)` also enables
  (`:2405-2410`).
- `MessageChannel` (`:2419-2426`): constructs two ports and entangles them
  (`_PortPaired.set(port1, port2)` + reverse).

**Verdict:** Stage-1 (same-thread) paired-port routing per
`41_…PATTERNS.md` §6 Fix 1 is **DONE**. The duolingo H1 fix described
there is shipped. Any re-test of duolingo must NOT assume this is missing.

**Spec-fidelity risks in the current impl:**
1. **Synchronous queue drain** (`:2350-2355`): the HTML spec dispatches via
   an event-loop **task**, not inline. A site that does
   `port.postMessage(x); port.onmessage = fn;` and expects `fn` to fire on
   a *later* turn (not re-entrantly) could see ordering differences. The
   comment acknowledges this trade-off (deno_core microtask drain across
   `execute_script` boundaries is unreliable). Low risk but a candidate
   inconsistency probe.
2. **No `messageerror`** path for un-cloneable data.
3. **`close()` is one-sided** (`:2402-2403`): it deletes only `this`'s
   pairing; the peer can still attempt `_deliver` to a closed port (guarded
   by `_PortClosed`, so it's a silent drop — acceptable).

### 4.4 Worker-realm fingerprint identity — `worker_bootstrap.js:86-161`

Re-applies, reading from the same stealth profile as the window:
`userAgent, appVersion, language, languages, platform, onLine,
cookieEnabled, hardwareConcurrency, deviceMemory, appName, product,
productSub, vendor, vendorSub, doNotTrack, pdfViewerEnabled, webdriver=false,
userAgentData` (full `WorkerNavigatorUAData` with `getHighEntropyValues`).
Plus `performance.now()` humanization (`:163-170`) and `performance.memory`
(`:172-186`), Intl timezone/locale sync (`:65-84`), and MediaSource/
MediaRecorder `isTypeSupported` (`:291-322`).

**Consistency assessment vs window:** because both realms read the same
`StealthProfile` via `op_get_profile_value`, the values are sourced
identically → window/worker agreement holds for all of the above. This is
**stronger than Camoufox**, which omits worker `deviceMemory`/`languages`
(§2.2). The residual risks:
- `userAgentData` worker default UA string fallback uses
  `"Chrome/147.0.0.0"` (`worker_bootstrap.js:140`) vs the window's profile
  UA — if `op_get_profile_value("user_agent")` returns empty in a worker,
  the fallback diverges from the window fallback. Verify the profile is
  always populated in worker `DomState` (it is set at `runtime.rs:343`, so
  OK when a profile is wired; the bare-default path is the risk).
- No `crypto` consistency issue (§3.3): digest matches across realms.
- `OffscreenCanvas` returns `null` context in BOTH realms
  (`window_bootstrap.js:4561-4578`), so a canvas hash is *consistently
  absent* — a vendor that demands a worker canvas hash sees `null` in both
  (consistent, but a "real Chrome would produce a hash" signal). Not
  corpus-blocking today.

### 4.5 The Stage-2 gap — transferring a MessagePort INTO a real Worker

`worker.postMessage(msg, [channel.port2])` should re-create `port2`
entangled on the worker side. BO's path:
- `Worker.postMessage` (`window_bootstrap.js:1975-2017`) validates
  transferables but only accepts **ArrayBuffer / views** (`:1984-1993`) —
  a `MessagePort` in the transfer list would **throw `TypeError`**.
- Even if accepted, `serializeForWire` has no MessagePort branch and
  `op_worker_post_to_worker` is a `#[string]` JSON hop — there is no
  cross-thread port registry. So a transferred port is dropped.

**Impact:** recaptcha-enterprise's `webworker.js` pattern where the
MessageChannel is created on the **main thread and both ports stay on the
main thread** (the worker uses its own parent↔worker channel) is covered by
Stage 1. But the pattern where a port is **transferred into** the worker
(`worker.postMessage({port}, [port])`) is NOT. Per `41_…PATTERNS.md` §6
Fix 1 Stage-2 note, this is ~300 LOC additional and needs a shared port-id
registry across threads. Not known to be corpus-blocking, but it is the
honest remaining hole in worker IPC.

### 4.6 Still-stub APIs (verified accurate)

- `SharedWorker` (`window_bootstrap.js:2050-2068`): `port` is a literal
  no-op object. No shared isolate. Not corpus-blocking.
- `ServiceWorker` (`:2069-2079`) + `ServiceWorkerContainer.register`
  (`:842-889`): registration resolves but is a no-op; no `fetch`
  interception. `navigator.serviceWorker` is SecureContext-gated
  (`:1023`).
- `BroadcastChannel` (`:2306-2312`): constructor + empty `postMessage`. No
  cross-context dispatch.
- `OffscreenCanvas` (`:4560-4578`): `getContext()` returns `null`.
- **Worklets** (`audioWorklet`, `paintWorklet`, etc.): not present —
  searched, no `Worklet` class. Not corpus-blocking but a presence-probe
  gap.

---

## 5. The AWS-WAF live-nav drain — the actual lever (`page.rs`)

`build_page_with_scripts_init_and_storage` executes scripts in document
order (`page.rs:~3500-3567`). After each external/inline script it runs
`event_loop.run_until_idle(Duration::from_millis(50))` (`page.rs:3566`).
After the loop it runs `cleanup_bootstrap`, schedules DOMContentLoaded/load,
the meta-refresh scanner, and a final `run_until_idle` capped at 8 s
(`page.rs:3640` region).

**Why AWS stalls:** challenge.js is a `defer` script (§2.1). Its body
registers a `.then()` chain (`checkForceRefresh().then(...)`) that — only
after async resolution — does `new Worker(blobURL)` to run the PoW. The
50 ms inter-script drain returns long before that chain advances. The
final 8 s drain *should* help, but per HANDOFF §4.4 the **nav loop
re-fetches the stub before the chain advances**, and/or the deferred
script's continuation is dropped between the build phase and the nav loop.
The offline oracle (`from_html_with_url` + `run_until_idle(5 s)` as a
single uninterrupted drain) runs it to `forceRefreshToken`.

**The fix space (public engine):**
1. Detect AWS-WAF challenge pages (the interstitial markers / the
   `challenge.js` src / the 202 status) and give them a **single long
   post-script drain** (≥3–5 s, `.unref()`-aware so benign pages still
   exit fast) instead of the 50 ms-then-nav-loop model.
2. Ensure the deferred-script `.then()` continuation + its
   `new Worker(blob)` survive into the final drain — i.e. don't let the
   nav loop re-fetch until the worker has had a chance to POST the token
   or the drain budget is exhausted.
3. Keep the worker pump alive during that drain (it is — the worker thread
   runs independently; the gate is the *parent* page drain, not the worker
   loop).

This is **public-engine** (Chrome-faithful async/defer semantics, no
vendor bypass code) and is the highest-ROI item.

---

## 6. Ranked fix list (ROI order)

### Fix W-1 — AWS-WAF live-nav async drain (the lever)
- **What:** challenge.js (and any deferred PoW blob-worker) needs the page
  event loop to keep ticking until the worker spawns, computes, POSTs the
  `aws-waf-token`, and the cookie-gain re-fetch fires. Replace the 50 ms
  inter-script cap + premature nav-loop re-fetch with a long unref-aware
  post-script drain for challenge-class pages; gate the nav-loop re-fetch
  on either token-acquired or budget-exhausted.
- **Where:** `crates/browser/src/page.rs:3566` (inter-script drain) +
  `:3640` (final drain) + the nav loop (`:1951-2185`).
- **Effort:** 2-4 days (instrument with `worker_ext`/`fetch_ext` tracing
  per HANDOFF §5.1, then tune the drain model + add an AWS-WAF page
  classifier).
- **Expected impact:** AWS-WAF cluster = amazon-ca/com/com-au/fr/in/jp +
  imdb (7 sites); likely also booking (HANDOFF §"same live-nav drain
  class"). Plausibly duolingo if its recaptcha worker chain has the same
  shape.
- **Confidence:** high (root cause is localized and reproduced offline vs
  live).
- **Engine:** public.

### Fix W-2 — Update the stale docs + close vNext/10
- **What:** correct `17_WEB_API_PARITY_MATRIX.md` §2.5 and
  `41_POW_WASM_WORKER_PATTERNS.md` §4.3/§6 to reflect that MessageChannel/
  MessagePort is implemented (`f3ea599`); close `vNext/10_URL-polyfill-blob`
  (fix shipped per `worker.rs:132-141`). Prevents future agents from
  re-implementing solved work (a real cost — this audit found the doc-41
  Fix 1 "highest-leverage gap" already shipped).
- **Effort:** 0.5 day.
- **Expected impact:** 0 sites directly; high process ROI.
- **Confidence:** high.
- **Engine:** public (docs).

### Fix W-3 — Implement the rest of `crypto.subtle` (HMAC + sign/verify, then AES/derive)
- **What:** add Rust ops for HMAC-SHA256 sign/verify (cheap, `hmac` crate),
  then `importKey`/`deriveBits` (PBKDF2/HKDF via `pbkdf2`/`hkdf`), then
  AES-GCM encrypt/decrypt (`aes-gcm`). Wire into both
  `window_bootstrap.js:3177-3182` and `shared_apis_bootstrap.js:120-125`
  (keep them identical for cross-realm consistency). Also make
  `op_crypto_digest` reject unknown algorithms instead of returning an
  empty buffer (`crypto_ext.rs:34`).
- **Where:** `crates/js_runtime/src/extensions/crypto_ext.rs` (new ops) +
  the two SubtleCrypto stub blocks.
- **Effort:** 3-5 days (HMAC+sign/verify ~1 day; derive+AES ~2-3 days).
- **Expected impact:** 0 known corpus sites today (AWS's SHA-256 flavour
  needs only `digest`, which works). Forward-looking: any PoW/sign flow
  that uses HMAC or deriveKey (HashcashScrypt-adjacent vendors, some
  token-signing SDKs). Removes a cross-realm "subtle.sign throws" tell.
- **Confidence:** medium (no measured flip; pre-emptive parity).
- **Engine:** public (Chrome-faithful Web Crypto).

### Fix W-4 — Worker fingerprint consistency probe + regression gate
- **What:** ship the `41_…PATTERNS.md` §4.4 probe (`fp()` run in window +
  worker), diff BO vs a real-Chrome-148 capture, and gate
  `worker_bootstrap.js` against window divergence. Verify the
  `userAgentData` worker fallback (`worker_bootstrap.js:140`) never
  diverges from window when the profile is unset; verify `webdriver`,
  `platform`, `languages`, `deviceMemory` equality.
- **Where:** new test in `crates/js_runtime/tests/` + capture tooling.
- **Effort:** 2-3 days.
- **Expected impact:** 0 direct flips expected (BO already matches on the
  audited fields, and beats Camoufox on deviceMemory/languages), but it is
  the cheapest insurance against a silent worker/window divergence
  regression and against vendors like DataDome tags.js cross-checks.
- **Confidence:** medium.
- **Engine:** public.

### Fix W-5 — MessagePort transfer across the Worker boundary (Stage 2)
- **What:** allow `worker.postMessage(msg, [port])` to re-create the port
  entangled on the worker side. Needs: accept MessagePort in the transfer
  validation (`window_bootstrap.js:1984-1993`), a process-global port-id
  registry bridging the two threads, and a serialize/deserialize protocol
  for ports in `serializeForWire`/`op_worker_post_to_worker`.
- **Where:** `worker_ext.rs` (registry + op signature change from
  `#[string]` to a structured payload) + `structured_clone.js` +
  `window_bootstrap.js` Worker.postMessage.
- **Effort:** 4-6 days (~300 LOC + cross-thread lifecycle care).
- **Expected impact:** unknown; no corpus site confirmed to require it
  (Stage 1 covers the same-thread recaptcha pattern). Pure
  forward-looking robustness for transfer-into-worker IPC.
- **Confidence:** low (speculative impact).
- **Engine:** public.

### Fix W-6 — OffscreenCanvas-in-Worker real context
- **What:** make `OffscreenCanvas.getContext('2d'|'webgl')` return a real
  context in the worker realm so a worker canvas hash matches the window.
- **Where:** `window_bootstrap.js:4560-4578` + `crates/canvas/` (DOM-tied
  today).
- **Effort:** 1-2 weeks (canvas render path is main-thread/DOM-tied;
  needs an off-DOM bitmap path or a route-to-main-thread shim).
- **Expected impact:** duolingo / recaptcha image-rec *if* a worker canvas
  hash is a load-bearing secondary check (unproven). Currently consistent-
  null across realms, so not an active flag.
- **Confidence:** low.
- **Engine:** public.

### Out of scope / defer
- `SharedWorker` real impl, `ServiceWorker` fetch interception,
  `BroadcastChannel` cross-context dispatch, Worklets, WASM
  threads+atomics (COOP/COEP): no corpus driver today
  (`41_…PATTERNS.md` §6 Fixes 4-6 agree). Defer post-v0.1.0.
- Per-vendor AWS-WAF PoW reimplementation in Rust: stays in the private
  `vendor_solvers` crate per `CLAUDE.md`. The §6 Fix W-1 engine-drain fix
  is the public, Chrome-faithful alternative and is preferred.

---

## 7. Open questions / verification TODO

1. Does the AWS-WAF challenge.js worker ever spawn under a longer live
   drain? (HANDOFF §5.1 task: instrument `build_page_with_scripts_init_and_storage`
   with `worker_ext`/`fetch_ext` tracing on a live imdb nav.)
2. Confirm vNext/10 closure with the §2 scheme-matrix probe
   (`data:`/`file:`/`ws:`/`wss:` parity) — the test only covers `blob:`.
3. Does `op_get_profile_value` ever return empty inside a worker for a
   wired profile? If so, the `worker_bootstrap.js:140` UA fallback could
   diverge from window. (Fix W-4 probe answers this.)
4. Is any corpus site using `worker.postMessage(msg, [port])`
   transfer-into-worker? (Decides Fix W-5 priority.)
5. Does `op_crypto_digest` returning an empty buffer for an unknown algo
   cause any vendor to read a 0-length hash as a "valid" zero rather than
   an error? (Fix W-3 hardening.)

---

## 8. Files referenced (all paths absolute-relative to repo root)

- `crates/js_runtime/src/extensions/worker_ext.rs` — worker spawn,
  blob registry, parent↔worker IPC, reaping.
- `crates/js_runtime/src/extensions/crypto_ext.rs` — digest + random ops.
- `crates/js_runtime/src/js/worker_bootstrap.js` — worker-realm `self`,
  navigator, location, postMessage, importScripts.
- `crates/js_runtime/src/js/window_bootstrap.js` — Worker/SharedWorker/
  ServiceWorker/BroadcastChannel/MessageChannel/MessagePort/OffscreenCanvas
  constructors + crypto.subtle (window realm).
- `crates/js_runtime/src/js/shared_apis_bootstrap.js` — crypto.subtle +
  URL polyfill (worker/shared realm).
- `crates/js_runtime/src/js/structured_clone.js` — structuredClone +
  serializeForWire/deserializeFromWire.
- `crates/js_runtime/src/js/cleanup_bootstrap.js` — SecureContext gate on
  crypto.subtle/randomUUID.
- `crates/js_runtime/src/runtime.rs:314-423` — `create_worker_runtime`
  (bootstrap order + secure-context wiring).
- `crates/browser/src/page.rs:3500-3640` + `:1951-2185` — script-execution
  loop, the 50 ms inter-script drain, final drain, nav loop (Fix W-1).
- `crates/js_runtime/tests/worker.rs` — worker round-trip + self-location
  + blob-protocol tests.
- Commits: `f3ea599` (MessageChannel), `5216336` (worker secure-context),
  `967b4dc` (R-DUO-WORKER / vNext-10 origin).
