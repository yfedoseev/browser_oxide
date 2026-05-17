# Kasada — Deep Engine Analysis (browser_oxide)

**Author:** Kasada deep-research agent · **Baseline git HEAD:** `fd98bfa`
· **Created:** 2026-05-16 · Follows the §0–12 contract in
`00_INVENTORY_AND_METHOD.md`.

Legend for claim provenance (per rules of engagement):
**[MECH]** cited mechanism fact · **[CODE]** our-code fact (file:line I
read) · **[HYP]** labeled hypothesis · **[MEAS]** an artifact/test
I verified first-hand this session.

---

## ⚠ CORRECTION 2026-05-16 — read first (supersedes the §0/§11 "no engine-only path / behaviour is the lever" verdict for these 3 sites)

[MEAS] `ab_harness/nocdp.sh` (real Chrome 147, opens URL + waits,
**zero** mouse/scroll/keyboard, **this datacenter IP**) **passes all
three** — captured window titles = real homepages (canadagoose
"Luxury Performance Outerwear…", hyatt "Hotel Reservations…", realtor
"Realtor.com®…": `ab_harness/nocdp/*.windows.txt`). Our engine, also
zero-interaction, same IP, gets the 429 / `bot1225.b:1`.

⇒ The differentiator is **NOT IP** (real Chrome passes here), **NOT
behavioural absence** (real Chrome with zero behaviour passes — so
"zero behavioural variance" cannot be the nocdp-delta cause), **NOT
"needs a paid real-browser farm"**. It is a **passive, static
engine-vs-real-Chrome-147 surface divergence** [HYP, sharply
localisable]: the JS env ips.js measures / how ips.js executes in our
V8 vs Chrome's V8 / TLS-JA4 / H2 / GPU-canvas — making the server
score `b:1` for us only. The §(e)/§11 "behavioural capability + paid
farm / unverifiable holistic tail" framing below is **superseded for
canadagoose/hyatt/realtor**: engine-addressable. The live-oracle
reference is **already captured** (`tl_capture.sh` →
`ab_harness/tl/hyatt.tl_body.bin` = 36 KB decrypted real-Chrome `/tl`
sensor + `canadagoose.pcap` + `.keys`). Decisive next experiment =
**K2-DIFF**: capture our engine's `/tl` POST + field-diff vs the
real-Chrome capture → the divergent field is the named, fixable bug.
Ordered plan: `UNBLOCK_PLAN.md` "canadagoose, hyatt, realtor" +
`UNBLOCK_kasada.md`. The realm/sentinel line stays closed (Phase 2
OUTCOME A) — this correction is about the *passive POST-payload /
runtime* surface, not realm identity.

---

## 0. Executive summary & pass-guarantee thesis

**How Kasada decides bot-vs-human in 2026.** Kasada is a *holistic,
server-scored, multi-stage trust model*, not a single client gate. The
client runs `/ips.js` — a polymorphic custom-bytecode VM — which collects
~110 fingerprint probes + behavioral telemetry, runs a SHA-256
proof-of-work, and POSTs an encrypted payload to `/<tenant>/tl`. The
server returns `x-kpsdk-ct` (session token) and the VM keeps emitting
per-request `x-kpsdk-cd` PoW tokens. **The block decision is computed
server-side** by an ML model that scores the *accumulated* signal across
TLS/JA3-JA4, HTTP/2, IP reputation, the fingerprint payload, behavioral
variance, and session-warming history — a weighted average, not a
kill-switch [MECH: Scrapfly 2026, ScrapeBadger 2026; §2 of this doc].

**The single highest-leverage gap blocking us — honest answer: there is
no single lever.** [MEAS] Decoded `crates/browser/kasada_error_7.b64`
first-hand this session: Kasada's own client SDK verdict for
canadagoose is
`{"type":"ab","action":"allow","og":"https://www.canadagoose.com",…
"bot1225":{"r":"{\"ifp\":1604157259,\"ilk\":2418312849}","t":32,"b":1},
"time":33}` — `action:"allow"` **with the `bot1225.b:1` bot-flag set**,
yet canadagoose still serves the 756 B / 429. The client passes; the
**server** scores the accumulated `b:1` telemetry as bot. Phase 2's
network-free differential-identity test
(`kasada_identity_decisive_ours.txt` = `slice:WGEF concat:WGEF
apply:WGEF clz32:WGEF globals:WGEF`, [MEAS] re-read) and the clean
80/80 sentinel (`kasada_sentinel_clean.json`,
`missTaggedElsewhere:0`, [MEAS] re-read) jointly **close** the
realm/sentinel/identity line as not-the-bug. The residual is a
**holistic behavioral/entropy ML tail**: closing it needs a *behavioral
telemetry capability + an authorized live-oracle differential regime*,
not a code commit.

**Honest verdict — guarded sites today:**

| Site | Engine state | Verdict |
|---|---|---|
| canadagoose | 756 B / 429, `action:allow`+`b:1` | **BLOCK** (holistic ML tail; no single lever) |
| hyatt | 737 B / 429 | **BLOCK** (same class) |
| realtor | 1764 B / 429 | **BLOCK** (same class) |
| macys | renders 1.7 MB | **PASS** — was a classifier false positive (master plan Phase 0.2) |

So of the 4 nominal Kasada sites, **1 is an FP that passes; 3 genuinely
block** and are the hardest engine in the corpus. No Kasada site is a
confirmed IP ban — `ab_harness/nocdp.sh` (vanilla Chrome 147, no CDP,
same datacenter IP) opens all three [MECH: doc 21 corrected banner,
re-read this session].

---

## 1. Vendor surface & 2026 deployment

- **Product:** Kasada Bot Defense (KPSDK). Client SDK script served from
  a first-party path, historically `/ips.js` (older) / `p.js` (2026
  alias), version-pinned via `x-kpsdk-v` (e.g. `j-1.1.0`) [MECH: Hyper
  Solutions docs; ChrisYP writeup].
- **Endpoints (per-tenant prefixed; tenant prefix is a GUID pair, e.g.
  `/149e9513-…/2d206a39-…`):**
  - `/fp?x-kpsdk-v=…` — fingerprint bootstrap; returns the HTML carrying
    `window.KPSDK`, the script path, and `x-kpsdk-im` (init marker).
  - `/ips.js` (`p.js`) — the polymorphic VM bytecode interpreter.
  - `/<tenant>/tl` — telemetry POST; the VM submits the encrypted
    fingerprint+PoW+behavioral payload; response carries `x-kpsdk-ct`,
    `x-kpsdk-st`, `x-kpsdk-fc`.
  - `/<tenant>/mfc` — feature config (stricter tenants); returns
    `x-kpsdk-fc` / `x-kpsdk-h` ("Flow 2", Hyper Solutions).
  - error/telemetry sink: `cdndex.io/error`, `cdndex.io/r` (the
    `/error` POST our blob-capture intercepts) [CODE: chrome_compat.rs
    `capture_init` filters `cdndex.io/error` / `cdndex.io/r`].
- **Blocked response:** HTTP **429** with a tiny body carrying the
  script path (canadagoose 756 B, hyatt 737 B, realtor 1764 B); a hard
  rejection is a bodyless **403** [MECH: doc 03 §2.1; matches our
  captured sizes].
- **Headers (full set, [CODE] `kasada_session.rs`):** `x-kpsdk-ct`
  (session token, ~30 min, reusable, "consumes 1000 points"),
  `x-kpsdk-cd` (per-request PoW, single-use, "consumes 50 points",
  must be <5 s old — replay-defended), `x-kpsdk-st` (server time, binds
  ct↔cd), `x-kpsdk-h` (HMAC), `x-kpsdk-v` (version pin), `x-kpsdk-r`
  (request id / challenge indicator), `x-kpsdk-fc`/`x-kpsdk-im`/
  `x-kpsdk-dt` (Flow-2 tokens) [MECH: nixbro overview, ChrisYP, Hyper
  docs — point values cross-confirmed].
- **2026 tiering:** canadagoose/hyatt/VEVE share the strict
  `/149e9513…/2d206a39…` template (requires `/mfc` Flow-2 + ct echo
  as a request header) [CODE: kasada_session.rs:34-50 doc comment,
  empirically verified 2026-04-27 per the inline note].

---

## 2. Detection pipeline — stage by stage

Kasada is a **weighted-average trust score across 6 stages**; each is a
soft score (not a kill) except where noted [MECH: Scrapfly 2026,
ScrapeBadger 2026, ZenRows 2026 — three independent 2026 writeups
agree on the stage list].

| # | Stage | Signal | How scored | Kill vs soft |
|---|---|---|---|---|
| 1 | **TLS** | JA3/JA4 of ClientHello (ciphers, versions, extensions, ALPN) | flagged when the *unordered set* doesn't match a real browser, or JA3↔UA conflict | soft (≈−0.4 for JA4 mismatch [MECH doc 03 §1]); JA3↔UA contradiction is heavily weighted |
| 2 | **IP reputation** | datacenter vs residential vs mobile ASN | datacenter = negative | soft, but large weight |
| 3 | **HTTP** | HTTP/2 expected (HTTP/1.1 flagged); header order/value coherence with UA | anomaly score | soft |
| 4 | **`ips.js` VM execution** | did the VM run, complete PoW, emit a well-formed `/tl` payload | structural | **kill if absent/malformed** (no `ct`) |
| 5 | **JS fingerprint** | canvas/WebGL GPU vendor+renderer, navigator, screen, plugins, audio FP, ~110 probes triangulated across realms | per-field anomaly → contributes to `bot1225` | soft (Scrapfly: "isn't a reliable method, not taken with a grain of salt") |
| 6 | **Behavioral variance** | mouse path/accel, scroll accel, click intervals, keystroke timing, dwell — collected into the encrypted token | **"zero behavioral variance" is an explicit blocking trigger** | soft→hard when variance ≈ 0 [MECH ScrapeBadger 2026] |

**The load-bearing 2026 facts for us:**

- Stages 1–4 we pass (TLS byte-pinned to Chrome 147≡148, HTTP/2, the VM
  runs and POSTs `/tl`, we get an `action:"allow"` verdict — proof the
  VM executed and the client-side checks did not hard-fail).
- The block is the **server scoring stages 5+6 accumulated into
  `bot1225.b:1`**. ScrapeBadger lists the two production blocking
  triggers most relevant to a from-scratch headless engine: **"zero
  behavioral variance"** and **"headless browser GPU"**. Scrapfly adds
  that trust scores **decay over time / require session warming**.
- "VM emulators break within days of any Kasada update" — confirmed by
  3 independent 2026 sources. Our path is observational parity, not
  emulation. [MECH: Scrapfly, doc 03 §2.2.]

---

## 3. Challenge / script anatomy (bytes & structure)

- **Interstitial:** 429, body < 2 KB, contains the script path +
  `window.KPSDK` bootstrap. Our classifier keys Kasada off
  `body.contains("ips.js") && body.contains("kpsdk")` [CODE: page.rs:173]
  and `holistic_sweep.rs:68-69` keys off `_kpsdk` / `ips.js`.
- **`ips.js` VM:** ~530 KB polymorphic; a custom register-machine
  bytecode interpreter built via
  `new Function('return function(n,e,a,v,i,r){…}')()`. Disassembled
  186,690/187,426 instructions recovered (`umasii/ips-disassembler`),
  23,467-char string pool decoded [MECH: doc 26 §1-4, our prior RE].
- **Realm technique:** the SDK appends a cross-origin `<iframe src=.../fp>`;
  the VM runs in `/fp`, mints `iframe.srcdoc` child iframes, reads
  pristine natives off `contentWindow`, and **triangulates every probe
  across `window` × `globalThis` × child realm** by *identity*
  (`0x12 GET WINDOW PROP` ×1051; `j()` return-this primitive iterates
  `Object.keys(e.console)` and compares `r === e.console[f[c]]`)
  [MECH: doc 26 §2-4; primary disassembly].
- **Sentinel:** opcode `0x32 INIT FUNCTION STATE` writes
  `closure[sentinel] = {I:closure, E,Q,k}`; call opcodes (handlers
  34/54/167) branch on `if (l[sentinel] && l[sentinel].I===l)` to
  decide "my VM trampoline" vs "real native, `.apply()` directly". The
  per-load sentinel string this capture: `unjzomuybtbyyhwwkdpkxomylnab`,
  reconstructed at runtime from the XOR'd bytecode string table (never
  an eval literal — eval-source interception is a confirmed dead end)
  [MECH: doc 04 §(d)/(g); 5-year invariant].
- **Success signal:** a valid `x-kpsdk-ct` from `/tl` + the protected
  GET re-issued with `ct`+fresh `cd` headers returns 200 with real
  content (not the 429 stub). There is **no success cookie** — it is
  header-bound (`KP_UIDz-ssn` is only a session id, not the trust
  grant) [MECH: ChrisYP; CODE: kasada_session.rs:46-50 comment, verified
  on hyatt 2026-04-27].

---

## 4. Fingerprint / sensor payload — field by field

The decoded `/error` report (`kasada_error_{1,2}.b64`, ~110 fields;
decode chain: outer-b64 → JSON `.data` → inner-b64 → XOR `omgtopkek`)
is the closest thing we have to the payload field map.

**[MEAS] Verified this session — the `/error` blobs are
production-representative, NOT a §9.3 test artifact.** I read
`kasada_error_blob_capture`'s `capture_init` at
`chrome_compat.rs:4085-4170` in full: it wraps **only**
`TextEncoder.prototype.encode`, `XMLHttpRequest.prototype.{send,open}`,
and `globalThis.fetch` — every hook is a passive
`.apply(this, arguments)` pass-through filtered to `cdndex.io/error`/`/r`.
**There is no `globalThis.Function` wrapper and no sentinel trap in this
test.** (The `globalThis.Function = TracedFn` wrapper is in a
*different* test, `kasada_vm_dispatcher_trace`, `chrome_compat.rs:3448`
— do not conflate them; doc 04 §(c) did, and master plan §8.5 already
corrected that.) So the field readings below are real.

| Field | Decoded value (per doc 04 §(c), spot-checked) | Read |
|---|---|---|
| `bot1225` | `{"r":"{ifp,ilk}","t":32,"b":1}` (no `e:1`) | **The bot verdict.** `b:1` = bot-flagged. The single biggest trust-score contributor [MECH: prior diagnosis]. |
| `cnf`,`wdt`,`ifw`,`wgp`,`nl`,`sas`,`puam`,`ifc`,`npf`,`fif`,`ecp`,`gua`,`csd`,`mnt` | all clean / `[native code]` / Chrome-consistent | These passed — incl. the namedItem source leak (FIXED) and full Window enumeration. |
| `fsc` | `TypeError: Class extends value function toString() { [native code] }…` | NOW Chrome-faithful (the genuine-native `Function.prototype.toString` fix #6 worked). |
| `crs`,`wse`,`bfe`,`dpi` | non-clean but unverified vs CDP-free Chrome | Small, holistic-tail; `action:"allow"` says none is a hard fail. |
| `smc`,`dpv` | `unjzomuy…` TypeError | Function-wrapper amplified in the *trace* test, not the blob test; not a production hard-fail. |

**The decisive structural fact: the report is overwhelmingly clean and
the client verdict is `action:"allow"`.** The bot decision is therefore
**not** a single failed probe in this payload. The fields the payload
does *not* fully carry to a passive `/error` capture — the **behavioral
telemetry** (mouse/scroll/keystroke entropy) and the **TLS/JA3-JA4 +
HTTP/2 server cross-check** — are the un-instrumented surface and the
prime suspects for what computes `b:1` (see §9).

GPU/canvas: blobs 3-6 are base64 RGBA pixel arrays (canvas/WebGL
readback) — Kasada *does* read the renderer; a SwiftShader/headless GPU
string is a named 2026 trigger [MECH ScrapeBadger; doc 03 §3.1 for the
analogous Akamai `0x0000C0DE` flag — same class of tell].

---

## 5. Crypto / encoding

- **PoW (`x-kpsdk-cd`)** — fully ported, byte-correct [CODE:
  `crates/stealth/src/kasada.rs`]. `jonta = sha256(platformInputs ", "
  alignedWorkTime ", " id)`; per-sub `quashanna = sha256(anyjha ", "
  jonta)` accepted iff `2^52 / (parseInt(h[0..13],16)+1) >=
  difficulty/subchallengeCount`. Public params: difficulty=10,
  subchallenges=2, `tp-v2-input`. `alignedWorkTime =
  round(workTime/18000081)*10`. Output JSON
  `{workTime,id,answers,duration,st,rst,v,d}` is the `x-kpsdk-cd`
  header value. Unit tests prove replay-validity (kasada.rs:332).
- **`x-kpsdk-ct`** — opaque server-issued token; the VM-encrypted `/tl`
  payload is the input. We do **not** generate `ct` — it comes from the
  live `/tl` response (the only way; commercial solvers run `ips.js` in
  a browser farm to get it — there is no open `ct` algorithm; lktop
  README is a sales page [MEAS: WebFetch returned a QQ/email sales blurb,
  no algorithm]).
- **`/error` blob encoding:** outer-b64 → JSON → inner-b64 → XOR with
  static key `omgtopkek` [MEAS: I decoded blob 7 with exactly this chain
  this session and got valid JSON].
- **Replay/anti-replay:** `cd` is single-use, server tracks the id, `st`
  binds `cd` to the session; `cd` must be < ~5 s old [MECH: nixbro,
  ChrisYP].
- **Server-side TLS cross-check:** JA3/JA4 + HTTP/2 SETTINGS are
  re-derived server-side and cross-checked against the UA / `ct` —
  Akamai pioneered this, Kasada applies the same class [MECH: doc 03
  §1; WebSearch 2026]. We pin TLS byte-exact to verified-real Chrome
  147 (≡148 on the wire — master plan §8.5 Phase 1 G7).

---

## 6. Cookie & header lifecycle / state machine

[CODE: `kasada_session.rs` + `net/src/lib.rs:301-326,478-482,690-735,
857-865,1211,1405`; `fetch_ext.rs:470,615`]

```
GET site  ──► 429 + x-kpsdk-cr:true + x-kpsdk-st           (edge)
           └─► learn_kasada(): cache server_offset, session id,
               tenant_prefix, h/v/r/fc/ct tokens                [lib.rs:793/932/1309/1462]
ips.js runs in V8  ──► (strict tenant) GET /mfc → x-kpsdk-fc
                   ──► POST /<tenant>/tl  (VM-encrypted payload via op_net_xhr_sync)
                       └─► resp: x-kpsdk-ct, x-kpsdk-st        [learned again]
retry top-level GET  ──► inject x-kpsdk-cd (fresh PoW), x-kpsdk-ct,
                         x-kpsdk-fc, x-kpsdk-h/v/r/im/dt        [lib.rs:690-735]
                     └─► server scores accumulated /tl signal
                         ── action:allow + bot1225.b:1 ──► STILL 429
```

"Solved" on the wire = the retry GET (carrying `ct`+fresh `cd`) returns
200 + real body. **We never reach that for canadagoose/hyatt/realtor**:
the server-side ML scores `b:1` and keeps returning the 429 *even though
the client tokens are valid and `action:"allow"`*. The valid→invalid
transition that matters is **server-internal** (the trust score), not a
client cookie/header we can observe or forge.

---

## 7. How OSS / commercial tools defeat it

Skeptical, citation-backed. **Every tool that reliably passes Kasada in
2026 either drives a real browser binary or is a paid black box.**

| Tool | Passes? | Mechanism | Portable to us? | Source (accessed 2026-05-16) |
|---|---|---|---|---|
| Patchright / Camoufox / Nodriver / SeleniumBase-UC | Yes [REPORTED] | **Real Chrome/Firefox binary**; only hides the automation control surface — the Kasada VM runs in a genuine engine | **No** (they *are* a browser; we are the engine) | Scrapfly 2026; thewebscraping.club THE LAB #76 |
| Hyper Solutions / Takion / commercial API | Yes [REPORTED] | Run `ips.js` in a maintained browser farm, return `x-kpsdk-ct/cd` | No (paid; treadmill) | docs.hypersolutions.co/k4sada |
| curl-impersonate / rquest alone | **No** | TLS/H2 only — "won't bypass Kasada's JS challenges" | n/a | Scrapfly 2026 |
| lktop/kpsdk, ChrisYP, catcha8/Kasada.io-API, unicorn-aio/kpsdk | **Not reproducible** | Marketing/sales pages or shape-only notes; no working `ct` algorithm | No | github.com/lktop/kpsdk (sales page, [MEAS]); ChrisYP writeup |
| 0x6a69616e/kpsdk-solver | Partial | **Playwright-driven** (a real browser) — not an algorithm | No | github.com (search 2026-05-16) |

**Confirmed reusable findings (not shortcuts, but they validate our
direction):**
- The asadfix guide states Kasada **"specifically fingerprints
  playwright-stealth by calling `Function.prototype.toString()`"** and
  silently 403s — this externally **confirms** our genuine-native
  `Function.prototype.toString` fix (native_fns.rs) was the correct
  structural fix and that JS-only `toString` patching is a named Kasada
  tell [MECH: doc 03 §2.1].
- The header protocol (`x-kpsdk-ct/cd/st/v/h/r`, point costs 1000/50,
  single-use `cd`, 30-min `ct`) is **stable** across every source —
  evidence our `kasada_session.rs` wire model is correct in shape.
- **No external source has an open algorithm for `ct` or for what
  computes `bot1225.b:1`.** This is genuinely novel territory because no
  OSS tool faces it — they all run a real engine. Nothing external
  shortcuts the from-scratch problem.

---

## 8. What browser_oxide does today (file:line evidence)

**Headline: our Kasada code is WIRED (not dead), but its design assumes
HTTP-replay solving while the live path is in-V8 self-solve — they run
in parallel and the Rust PoW is mostly moot for the strict tenants.**

| Component | file:line | Status |
|---|---|---|
| PoW solver `solve`/`solve_default`/`solve_with_realistic_duration` | `stealth/src/kasada.rs:149/212/222` | **WIRED & exercised** — called by `compute_cd_header` |
| `KasadaSessionStore` (per-host token cache) | `net/src/kasada_session.rs` | **WIRED** — instantiated in `HttpClient`, shared across fetch clients |
| `compute_cd_header` → `kasada_cd_header` | `kasada_session.rs:281` → `net/src/lib.rs:478` | **WIRED** — called at `lib.rs:692,863,1211,1405` (every GET path) |
| `learn` / `learn_kasada` (token harvest from responses) | `kasada_session.rs:108` ← `net/src/lib.rs:309`, called `:793,932,1309,1462` | **WIRED & exercised** on every response |
| ct/fc/h/v/r/im/dt header injection on GET | `net/src/lib.rs:690-735` | **WIRED** |
| In-V8 `/tl` POST via sync XHR (so PoW loop doesn't starve the event loop) | `fetch_ext.rs:op_net_xhr_sync` (~:560-660); shares `kasada_sessions()` | **WIRED & exercised** — this is the *real* solve path: ips.js POSTs `/tl` itself |
| `op_net_fetch_sync` (ips.js script fetch, shares session) | `fetch_ext.rs:470` | **WIRED** |
| x-kpsdk-* harvest from `__fetchLog` + retry-GET forwarding | `page.rs:2026-2090,2147-2200,2382` | **WIRED** (navigation retry loop) |
| Genuine-native `Function.prototype.toString` (the named Kasada tell fix) | `js_runtime/src/native_fns.rs:130` | **WIRED & exercised** — structurally closes the `[[SourceText]]` leak |
| Real `v8::Context` child realm (`op_create_child_realm`/`_set`/`_eval`) | `js_runtime/src/extensions/dom_ext.rs:1136/1291/1342`; primary path `dom_bootstrap.js:2509` | **WIRED & exercised** — Proxy fallback (`:2879`) is now dead-only |
| `kasada_global_identity_invariant_holds` (Phase 2 decisive test) | `browser/tests/kasada_identity_decisive.rs:47` | **WIRED test, network-free** — OUTCOME A (identity matches Chrome) |
| `kasada_identity_decisive_live_canadagoose` | same file:133 | `#[ignore]` network — corroboration only |
| `tier0_kasada.rs` (L1/L2/L3 site assertions) | `browser/tests/tier0_kasada.rs` | all `#[ignore]` (network) — diagnostic only |
| `kasada_vm_dispatcher_trace` (Function-wrapper VM trace) | `chrome_compat.rs:3448` | `#[ignore]`; **§9.3 confounded** — diagnostic only, NOT production evidence |

**MISSING:** behavioral telemetry feeding the `/tl` payload (mouse/
scroll/keystroke entropy as real signal, not zero-variance); see §9 G1.

---

## 9. GAP ANALYSIS — what feeds `bot1225.b:1` (ranked, concrete)

The genuinely open question. `action:"allow"` + healthy 80/80 sentinel +
identity-invariant OUTCOME A means **no single client probe is the
kill**; `b:1` is an *accumulated server ML score*. Ranked hypotheses
for what accumulates it, each with the discriminating experiment.

### G1 — Behavioral telemetry absence / zero-variance [HYP, HIGH]
**Evidence FOR:** ScrapeBadger 2026 names **"zero behavioral variance"**
an explicit production blocking trigger; Scrapfly 2026 says the VM
collects mouse/scroll/keystroke into the encrypted token and trust
decays without session-warming. Our `Page::navigate` path uses the
weaker `humanize.js`, not the richer `stealth::behavior`
Plamondon/Sigma-Lognormal model (master plan G8, **deferred** — never
wired for Kasada). A headless nav with **no mouse/scroll/keystroke at
all** is the textbook zero-variance case. The `/error` payload is
fingerprint-only in our passive capture; the behavioral channel is
exactly the un-instrumented surface that `action:"allow"` does *not*
clear.
**Evidence AGAINST:** none direct (un-instrumented).
**Discriminating experiment:** instrument the `/tl` POST body (not
`/error`) in a clean navigate and diff the behavioral sub-structure
between (a) our headless nav, (b) our nav with synthetic
`stealth::behavior` trajectories injected, (c) `nocdp.sh` real Chrome
with real mouse movement. If (a) carries an empty/constant behavioral
block where (c) carries entropic samples → G1 confirmed; the fix is
wiring `stealth::behavior` mouse/scroll/keystroke into the page before
`/tl` fires.

### G2 — TLS/JA3-JA4 + HTTP/2 server cross-check vs UA/ct [HYP, MED-HIGH]
**Evidence FOR:** Kasada server-side re-derives JA3/JA4+H2 and
cross-checks against UA/headers (3 independent 2026 sources); a JA4↔UA
or H2-SETTINGS↔UA contradiction is heavily weighted and is *server-only*
(invisible to the client `action:"allow"`). Our presets advertise UA
Chrome 148 while TLS is the Chrome-147 capture — master plan §8.5
argues 147≡148 on the wire (JA4 doesn't encode the minor), but that is
*our* analysis, not a live differential measurement against Kasada.
**Evidence AGAINST:** master plan §8.5 G7 reframe (147≡148 is
wire-cosmetic) — credible but unverified against Kasada specifically.
**Discriminating experiment:** capture our exact ClientHello + H2
SETTINGS on a live canadagoose connection and diff JA4/JA4H byte-for-byte
against `nocdp.sh` real Chrome's on the same site/IP. Equal ⇒ reject G2.
Any delta ⇒ G2 is a contributor; fix is re-pinning TLS to a captured
Chrome-148 hello (`capture_chrome_148_hello.rs` exists).

### G3 — Headless-derived FP signal not yet found (GPU/canvas/timing) [HYP, MED]
**Evidence FOR:** ScrapeBadger names **"headless browser GPU"** a
production trigger; blobs 3-6 are real canvas/WebGL pixel readback —
Kasada *does* read the renderer. Akamai's analogous
SwiftShader/`0x0000C0DE` flag is "permanently flagged" (doc 03 §3.1) —
same class. Our WebGL vendor/renderer string parity is asserted
elsewhere but never verified *against Kasada's specific probe set*.
**Evidence AGAINST:** WebGL was verified-correct in doc 17 (general,
not Kasada-specific); the FP report is mostly clean.
**Discriminating experiment:** decode the canvas/WebGL pixel blobs and
the `wgp` field from a clean (non-Function-wrapped) live capture and
diff vs `nocdp.sh` real Chrome on the same machine. Any pixel-hash or
renderer-string delta is a contributor.

### G4 — Main-window realm inversion holistic contribution [HYP, MED]
**Evidence FOR:** master plan §1.5 / Gap G2: `window.constructor ===
undefined` on the **main** window (window_bootstrap.js:77-86 refuses to
set `globalThis`'s `[[Prototype]]` to `Window.prototype`); Kasada's
disassembled dual-realm `globalThis[x]!==window[x]` / constructor-name
check now fires on the *main* window even though the child realm is
faithful. This is a real, named, holistic-score contributor (not the
*single* lever — Phase 2 OUTCOME A proved the four global-acquisition
paths are mutually `===` for the intrinsics Kasada *reads by identity*;
but `constructor`/prototype shape is a *different* probe).
**Evidence AGAINST:** Phase 2 OUTCOME A closed the *identity-of-reads*
line; G4 is the orthogonal *prototype-shape* surface, untested against
Kasada. NOT a resurrection of the closed line — different probe.
**Discriminating experiment:** add a network-free assert that
`window.constructor === Window` and `globalThis.__proto__ ===
Window.prototype` on the main window (currently fails per §1.5); then a
live re-decode to see if `b:1` clears. Risk: high (window_bootstrap
prototype change regressed deno_core ops historically).

### G5 — Session-warming / trust-decay history absent [HYP, LOW-MED]
**Evidence FOR:** Scrapfly 2026: trust scores decay; warming
(homepage → intermediary → target) is a named technique; a single cold
GET to a deep page scores worse. Our nav is a single cold request.
**Evidence AGAINST:** `nocdp.sh` real Chrome passes with a *single cold
GET* from this IP — so warming is not *necessary* for a genuine browser.
**Discriminating experiment:** A/B a warmed sequence vs a cold GET in
our engine; only meaningful after G1-G3 are closed (warming amplifies an
already-borderline score; it won't rescue a `b:1` headless FP).

### Refinement of the doc 04 §(f) decisive experiment (do NOT re-run as-is)
The original §(f) experiment tested **identity fragmentation of the
intrinsics Kasada reads**. Phase 2 already returned **OUTCOME A**
(network-free): `slice:WGEF concat:WGEF apply:WGEF clz32:WGEF
globals:WGEF` — every acquisition path resolves the identical object;
identity model matches Chrome. **That line is closed; do not re-run the
identity probe.** The *refined* decisive experiment is **differential
`/tl`-payload capture** (not `/error`, not the Function-wrapped trace):
instrument the real `/tl` POST body in a clean navigate, run the
**identical** capture in `ab_harness/nocdp.sh` real Chrome on the same
site+IP, and diff the **behavioral block (G1)** and **the TLS/JA4 the
server sees (G2)** — the only two surfaces `action:"allow"` does not
clear. This converts "holistic tail" into a named, ranked delta or
proves the residual is pure IP/ML weight (which `nocdp.sh` already
argues against). It requires an **authorized live-oracle regime**, not a
code commit — see §11.

---

## 10. FALSE-POSITIVE ANALYSIS of our code

### 10a. Detection FPs

| # | file:line | The false claim it can produce | Why false | Catching test |
|---|---|---|---|---|
| D1 | `page.rs:173` `body.contains("ips.js") && body.contains("kpsdk")` | A rendered page that *inline-mentions* both strings → false "blocked"; or a Kasada 429 whose stub omits one token → false "pass" | AND-of-two-substrings is stub-agnostic; the 756 B 429 reliably has both, but a future variant or an inline analytics ref could over/under-match | A fixture test: 756 B canadagoose 429 → EdgeBlock; a 1.7 MB macys body that inline-mentions `kpsdk` → Pass (this is the macys FP class) |
| D2 | `holistic_sweep.rs:68-69` keys Kasada off `_kpsdk` / `ips.js` substrings, **a different classifier than `page.rs`** | The ledger ("120/126") and `page.rs` can disagree per-site → inconsistent Kasada counts (macys counted as CHL by one, Pass by other) | The inventory §"Shared findings" flags exactly this divergence; `holistic_sweep` excludes some via len-gate (`:102`,`:111`) but the substring set differs from `page.rs:163-200` | A cross-classifier consistency test over the 4 Kasada bodies asserting `page.rs` verdict == `holistic_sweep::classify` bucket |
| D3 | macys "Kasada" label in older docs | "macys is a Kasada hard site" | Phase 0.2 re-baseline: macys renders 1.7 MB, no structural challenge — it is an **FP that passes**; only canadagoose/hyatt/realtor genuinely block | Already caught by Phase 0.2 typed re-baseline; keep macys out of the Kasada hard set in all future docs |
| D4 | thin-shell direction | A 756 B body is unambiguous, BUT a hypothetical "Kasada served a 5 KB shell that rendered nothing" would pass the `< 50 KB` weak-marker gate only if it lacked the strong markers | low risk for Kasada (strong markers are reliable in the 429) — noted for completeness | n/a (Kasada strong markers robust); the real thin-shell risk is on *other* engines |

### 10b. Solver / logic FPs

| # | file:line | The false claim | Why false | Catching test |
|---|---|---|---|---|
| **L1 (most important)** | `kasada.rs` PoW solver + `kasada_session.rs:compute_cd_header` is **WIRED** (`net/lib.rs:692/863/1211/1405`) and the doc-comment says "Kasada validates duration / required by stricter tenants" | The Rust solver runs **in parallel to ips.js self-solving in V8**. For the strict canadagoose/hyatt/realtor tenants the *real* `cd`/`ct` come from the in-V8 `op_net_xhr_sync` `/tl` POST (fetch_ext.rs). The Rust `compute_cd_header` only fires when a `KasadaSession` exists *and* the GET doesn't already carry `x-kpsdk-cd` (`!has_header`). It is **not the load-bearing path** for the hard sites, yet it is presented as the solver. It is not dead code, but it is **misleading-as-the-solver** — a "solver exists" FP in the *direction the inventory warns about*: exists & runs ≠ is what solves the hard sites. | Worse: a Rust-computed `x-kpsdk-cd` injected onto a GET that ips.js *also* solves could send a **second, inconsistent PoW** (Kasada `cd` is single-use, server-tracked) — a potential self-inflicted replay/`b:1` signal. Unverified but structurally plausible. | A test asserting that when ips.js is the active solver (in-V8 `/tl` path), `compute_cd_header` does NOT also inject a competing `x-kpsdk-cd` on the same logical request; or an integration assert that only one `cd` per request reaches the wire |
| L2 | `kasada_session.rs:297` `rst = rng.gen_range(2000..8000)`, `:303` `d = work_time - server_st_ms` | Comment claims these match "real ips.js" (page-relative `rst`, replay `d`). These are **synthetic guesses**, never validated against a real ips.js `/tl` payload. If ips.js self-solves, our synthetic `rst`/`d` are irrelevant; if the Rust path *is* used, a wrong `rst`/`d` is itself a `b:1` signal | The doc-comments assert correctness ("per Apr 2026 research") with no captured-payload verification; this is an assertion that "passes offline" (unit tests only check JSON shape) while the live path differs | Capture a real ips.js `/tl` body and assert our synthetic `rst`/`d` distribution matches; until then label these **unverified** in the code |
| L3 | `kasada.rs:156` `alignedWorkTime = round(workTime/18000081)*10` | Asserted as the ips.js seed alignment; only unit-tested against itself (kasada.rs:332 replays our own output) | Self-consistent ≠ matches Kasada. The replay test proves *our* answers satisfy *our* difficulty target, not that Kasada accepts them | A live L3 test (tier0_kasada is `#[ignore]`) — currently unverifiable network-free; document as unverified |
| L4 | `kasada_vm_trace.json` / `VM_TRACE_FINDINGS` / `UNJZOMUY_INVESTIGATION` "5 sentinel throws = the bug" | These conclude the sentinel-loss is the root cause | **Falsified.** They come from `kasada_vm_dispatcher_trace` which wraps `globalThis.Function` (the §9.3 confound, chrome_compat.rs:3448). The clean `kasada_sentinel_clean.json` (`missTaggedElsewhere:0`, [MEAS] re-read) + Phase 2 OUTCOME A prove the sentinel is healthy. Treat these three docs as historical/eliminated | Already caught by the clean sentinel + identity-invariant tests; do not resurrect this line (inventory rule + doc 04 §(g)) |

**The captured `kasada_error_*.b64` blobs are production, NOT a §9.3
artifact** — [MEAS] verified first-hand this session by reading
`capture_init` at chrome_compat.rs:4085-4170 (no Function wrapper) and
decoding blob 7 to the `action:"allow"`+`b:1` JSON. doc 04 §(c)'s
"blob-capture wraps globalThis.Function" claim is **false against the
code**; master plan §8.5 already corrected it; this doc re-confirms it.

---

## 11. The concrete pass-guarantee plan (honest)

**This is the hardest engine in the corpus and there is no single-commit
flip.** The client passes (`action:"allow"`); the block is a server ML
score on accumulated telemetry. Pretending a code change flips it would
violate verify-don't-assume. The honest plan:

**Step 0 — Accept the regime reality.** The mandatory network-free §4
gate **structurally cannot** verify a Kasada flip: the verdict is
server-computed from live behavioral+TLS telemetry. Progress *requires*
an explicitly-authorized **live-oracle differential regime**: clean
`/tl`-payload capture from our engine vs `ab_harness/nocdp.sh` real
Chrome on the same site+IP. Without that, every "fix" is unfalsifiable.

**Step 1 — Refined decisive experiment (§9 refinement).** Instrument the
real `/tl` POST body (a passive `XMLHttpRequest.prototype.send` /
`fetch` hook scoped to `/<tenant>/tl`, modeled on the *clean*
`kasada_error_blob_capture` pattern — NO Function wrapper). Run it in
our engine **and** in `nocdp.sh` real Chrome. Diff the **behavioral
block (G1)** and the server-visible **JA4/JA4H/H2 (G2)**. This names the
delta; it cannot come back ambiguous (either there's an entropy/TLS
delta or `b:1` is pure IP/ML weight, which `nocdp.sh` argues against).

**Step 2 — Branch on the delta:**
- **Behavioral block empty/constant where Chrome's is entropic (G1):**
  wire `stealth::behavior` (Plamondon/Sigma-Lognormal mouse, scroll,
  keystroke — the model already exists, master plan G8 deferred) into
  `Page::navigate` *before* `/tl` fires. Gate-checked per §4 discipline
  (it feeds Akamai sensors too — regression risk; additive
  `op_behavior_*` wrappers preferred, master plan §8.5 G8 follow-up).
- **JA4/H2 server-delta (G2):** re-pin TLS to a captured Chrome-148
  hello (`capture_chrome_148_hello.rs`); add the self-verifying JA4
  drift-guard (master plan Phase 1 G7).
- **Canvas/WebGL/renderer delta (G3):** correct the GPU vendor/renderer
  string to a real hardware string for the Kasada probe set.
- **No measurable delta:** the residual is IP/ML-weight + trust-decay;
  Kasada is then **not engine-flippable from this IP regime** and the
  honest close-out is "needs residential egress or session-warming
  infrastructure", documented, not burned-budget.

**Step 3 — Fix L1 (the parallel-solver hazard) regardless.** Ensure the
Rust `compute_cd_header` does **not** inject a competing single-use
`x-kpsdk-cd` when ips.js is the active in-V8 solver for the strict
tenants — a double/inconsistent PoW is a self-inflicted `b:1` candidate
and is cheap to gate.

**Verification regime:** §4 network-free gate for Step 3 + any
behavioral/TLS code (must stay green: `chrome_compat` ≥437/0,
`v8_natives` 11/11, `iframe_isolation` 5/5, `iframe_fp_diag`,
`v8_inspector_parity` 3/3). The *flip itself* is verifiable **only**
under the live-oracle regime of Step 1 — state this explicitly; the
gate cannot prove a Kasada pass.

**Expected honest outcome:** this is a multi-session,
capability-building effort (behavioral telemetry + a live-oracle dev
loop), not a commit. The valuable deliverable is the named delta from
Step 1, not a fabricated flip.

---

## 12. Sources & experiments

**External (URL — claim — accessed 2026-05-16):**
- `https://scrapfly.io/blog/posts/how-to-bypass-kasada-anti-scraping-waf`
  — 6-stage weighted trust model; trust-decay/session-warming; VM
  emulators break in days; tool list. Primary 2026.
- `https://scrapebadger.com/kasada-bypass` — p.js VM bytecode; ct/cd/v
  token roles; **"zero behavioral variance" + "headless browser GPU"
  named blocking triggers**; behavioral data in encrypted token. Primary
  2026.
- `https://www.zenrows.com/blog/kasada-bypass` — Kasada weighted scoring;
  JA3↔UA conflict heavily weighted; tool reproducibility.
- `https://docs.hypersolutions.co/k4sada/flow-2-fingerprint-endpoint` —
  endpoint+header protocol (`/fp`,`/tl`,`/mfc`, ct/cd/st/im/fc/h);
  PROVEN protocol shape, no algorithm.
- `https://github.com/nixbro/Kasada-Solver` — ct "1000 pts"/cd "50 pts
  single-use", st binds ct↔cd, HMAC `x-kpsdk-h`, ~30 min ct.
- `https://github.com/ChrisYP/ChrisYP.github.io/blob/main/en-US/kasada.md`
  — /fp→ips.js→/tl flow; point costs; `KP_UIDz-ssn` is a session id not
  a trust grant.
- `https://github.com/lktop/kpsdk` — [MEAS] sales page (QQ/email), **no
  algorithm** — confirms the inventory's skeptical note that public
  "Kasada solvers" are paid/dead.
- `https://substack.thewebscraping.club/p/bypassing-kasada-2025-open-source`
  — Patchright (real browser) works; standard Playwright fails;
  behavioral mimicry preferred.
- WebSearch (2026-05-16): Kasada server-side ML / 429-after-allow / JA4
  cross-check corpus (Scrapfly Akamai/DataDome/Imperva 2026, FoxIO JA4).

**Internal docs read:** `00_INVENTORY_AND_METHOD.md`;
`research_2026_05_16/{00_MASTER_PLAN,04_KASADA_CONSOLIDATED…,
03_OSS_STEALTH…}.md`; `research_2026_05_14/{01_KASADA,09_KASADA_DEEP,
21_PLAYWRIGHT_AB_DECISIVE}.md`;
`research_2026_05_15/{22,25,26,27}_*REALM*` (referenced via 04);
`kasada_ips_analysis/{UNJZOMUY_INVESTIGATION,VM_TRACE_FINDINGS}.md`
(historical/eliminated — L4).

**Local experiments run this session (auditable):**
- `grep` caller-trace of `compute_cd_header`/`kasada_cd_header`/
  `solve*`/`learn_kasada` across `crates/` → established §8 WIRED/DEAD
  table (Rust PoW is wired but parallel to in-V8 self-solve = L1).
- Read `chrome_compat.rs:4079-4170` (`kasada_error_blob_capture`
  `capture_init`) in full → confirmed **no `globalThis.Function`
  wrapper / no sentinel trap**; the `TracedFn` wrapper is the *separate*
  `kasada_vm_dispatcher_trace` at `:3448`. Blobs = production.
- `python3` decode of `crates/browser/kasada_error_7.b64` (b64 → JSON
  `.data` → b64 → XOR `omgtopkek`) → verbatim
  `{"type":"ab","action":"allow",…"bot1225":{…,"b":1},"time":33}` —
  allow-but-blocked paradox confirmed first-hand.
- Re-read `crates/browser/kasada_sentinel_clean.json`
  (`tags:80 miss:80`, tagSample = VM trampolines, missSample = native
  builtins) and `kasada_identity_decisive_ours.txt`
  (`slice:WGEF … globals:WGEF`) → realm/sentinel line confirmed CLOSED;
  not resurrected.
- No heavy `cargo` run (build-lock; network tests `#[ignore]`) — per
  the rules of engagement; no decisive cheap test available
  network-free (the flip is server-scored).
