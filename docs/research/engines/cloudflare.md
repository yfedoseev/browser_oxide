# Cloudflare Bot Management — Deep Engine Analysis (browser_oxide)

> **Agent:** Cloudflare deep-research. **Date:** 2026-05-16.
> **Baseline git HEAD:** `fd98bfa`. **Contract:** follows
> `docs/research/engines/00_INVENTORY_AND_METHOD.md` template §0–§12.
> **Convention:** every claim is tagged `[MECHANISM]` (cited external
> fact), `[CODE]` (file:line we actually read), or `[HYPOTHESIS]`
> (inference, labelled). Negative results are first-class.
>
> Sites guarded in our corpus: **udemy** (29-set, the only CF site),
> **economist** + **quora** (wider corpus, iOS-profile only). quora is
> *also* in the master-plan "captcha-gated / out of stealth scope"
> bucket (`00_MASTER_PLAN.md` §2) — for CF purposes it is the same
> Managed Challenge as udemy.

---

## 0. Executive summary & pass-guarantee thesis

**How CF decides bot-vs-human in 2026** `[MECHANISM]`: a three-layer
funnel scored *before and during* a JS challenge. (1) **Edge / network
layer** — JA3+**JA4** TLS fingerprint, HTTP/2 SETTINGS+pseudo-header
order, and (newer) eBPF TCP/IP stack probing (window-scaling, ACK
intervals) correlated against the claimed UA and the **IP ASN
reputation** — datacenter ASNs (AWS/DO/Hetzner) start with a degraded
trust score. (2) **Environment layer** — the always-on JavaScript
Detections (JSD) probe + the Managed-Challenge orchestrator, now a
**Rust→WASM** module, collects screen/navigator/canvas/WebGL/audio/
font/timezone and *runtime-consistency* signals (patched
`navigator.webdriver`, shimmed getters leak). (3) **Behavioral /
proof-of-work layer** — Hashcash-style SHA-256 PoW whose difficulty is
*scaled by the threat score from layers 1-2*, plus ML mouse/timing
biometrics. A `cf_clearance` cookie is minted only after the
orchestrator completes the PoW + posts a fingerprint payload, and it is
HMAC-bound to IP+UA+JA3/JA4+Ray at issuance.

**The single highest-leverage gap blocking us**: `[CODE]` our CF
"solver" is **detection + a 10 s passive event-loop poll** with
**zero code that fetches, runs to completion, or posts back the
orchestrator/JSD payload**. The orchestrator's *cross-origin Turnstile
iframe* (`challenges.cloudflare.com`) is **never fetched** because
dynamically-`appendChild`-inserted iframes are only registered as
synthetic `window[N]` objects (`dom_bootstrap.js:1924-1950`), never
loaded via the real `ChildIframe::from_url` path (which runs only once
at build time, `page.rs:3139-3176`). So `cf_clearance` is structurally
unreachable for any modern Managed Challenge — independent of IP.

**Honest verdict today**:

| Site | Profile | Reality | Verdict bucket |
|---|---|---|---|
| **udemy** | desktop Chrome | Managed Challenge served; orchestrator never completes; no `cf_clearance` | **BLOCK** (engine gap, *not* confirmed IP) |
| **economist** | iOS Safari | Same Managed Challenge; iOS TLS profile incomplete + no PAT | **BLOCK** (engine gap) |
| **quora** | iOS Safari | Same Managed Challenge; also human-captcha bucket in master plan | **BLOCK** (engine + human gate) |

Detection itself is **correct** (`cf-mitigated` + `_cf_chl_opt`
markers, near-zero FP). The failure is 100% on the *solver/logic* side.

---

## 1. Vendor surface & 2026 deployment

`[MECHANISM]` (CF official docs + RE writeups; sources §12)

**Product tiers** (decide *which* defense, not the challenge format):

- **Bot Fight Mode** (Free): static heuristics + JA3; defeated by TLS
  impersonation alone (curl-impersonate still works on BFM).
- **Super Bot Fight Mode** (Pro/Business): + WAF custom rules + JA4.
- **Bot Management** (Enterprise): full ML score 1–99 + **JA4 Signals
  Intelligence** (10 per-fingerprint metrics: `browser_ratio_1h`,
  `cache_ratio_1h`, `h2h3_ratio_1h`, `heuristic_ratio_1h`,
  `uas_rank_1h`, `paths_rank_1h`, `reqs_rank_1h`, `ips_rank_1h`,
  `reqs_quantile_1h`, `ips_quantile_1h`).

**Score → action** (default, customer-tunable per route):
score >70 pass · 30–70 Managed Challenge · <30 Interactive/block.

**Challenge UIs** (the `cType` in `_cf_chl_opt`):

| `cType` | UI | Mechanism | Headless-solvable? |
|---|---|---|---|
| `jsch` | "Checking your browser…" | small JS PoW kernel | yes (rare in 2026) |
| `managed` | "Just a moment…" → invisible *or* Turnstile | CF picks per-request by score | **invisible: yes-in-principle; widget: hard** |
| `non-interactive` | visible auto-completing Turnstile | WASM widget, no gesture | hard |
| `interactive` | mandatory click box | blind-RSA, user gesture | **no (needs captcha service)** |

**Script / path family** (May 2026, `[MECHANISM]` cfresearch
archived 2025-01-20 + 04_CLOUDFLARE.md live capture + roundproxies 2026):

```
/cdn-cgi/challenge-platform/scripts/jsd/main.js          ← static WASM bootstrap (always-on JSD)
/cdn-cgi/challenge-platform/scripts/jsd/api.js           ← public API surface
/cdn-cgi/challenge-platform/h/{b,g}/orchestrate/{chl_page,managed,jsch}/v1?ray=…  ← GET, returns stage-2 JS
/cdn-cgi/challenge-platform/h/{b,g}/flow/ov1/{tok}:{epoch}:{tok}/{ray}/{cf-chl-id} ← POST, v_<ray>=… payload
/cdn-cgi/challenge-platform/h/{b,g}/jsd/oneshot          ← JSD fingerprint POST
/cdn-cgi/challenge-platform/h/{b,g}/cv/result/{token}    ← clearance result POST (variant)
https://challenges.cloudflare.com/turnstile/v0/…         ← cross-origin Turnstile iframe + WASM
```

`cFPWv` (`'b'`/`'g'`) selects the variant tree; **rotates per CF
deploy, not per request** — observed `g`(05-10) → `b`(05-14). `cvId:'3'`
= modern JS-VM/WASM orchestrator (legacy IUAM was cvId 1/2; the old
`jschl_vc`/`jschl_answer` math challenge is effectively retired —
do **not** build a homemade IUAM solver).

**Our target sites' tier** `[MECHANISM]` (04_CLOUDFLARE.md §6 live
capture, 2026-05-14): udemy, economist, quora all serve **identical
`cType:'managed'`, `cvId:'3'`, `cFPWv:'b'`** with byte-identical CSP
modulo nonce. They differ only in (invisible) WAF custom rules + score
thresholds. udemy = "wall for every UA"; economist/quora escalate to
Turnstile for iOS-claiming UAs.

---

## 2. Detection pipeline — stage by stage

`[MECHANISM]` synthesized from CF JA4-signals blog, the 2026 Tapscape
"core-level" guide, the Medium "Advanced Evasion … 2026" architecture
piece, scrapfly, and CF docs (URLs §12).

| Stage | Signal collected | How scored | Kill vs soft |
|---|---|---|---|
| **TLS (ClientHello)** | JA3 (GREASE-noisy, weak) + **JA4** (extensions *sorted* before hash → sees through Chrome's Fisher-Yates shuffle). Cipher list, sigalgs, curves (MLKEM vs Kyber vs none), ALPS/session-ticket/ECH presence, cert-compression id. | Correlated to UA+ASN. JA4 shared with `python-requests`/`go-http`/`curl` ⇒ low `browser_ratio_1h`. | **soft-score** (feeds ML); a non-browser JA4 with browser UA can be a near-kill. |
| **HTTP/2** | SETTINGS frame values, WINDOW_UPDATE delta, **pseudo-header order** (`m,p,a,s` Chrome vs `m,s,a,p` Safari), PRIORITY frames. Hashed as JA4H-adjacent. | Mismatch vs claimed UA. | soft-score, strong weight. |
| **TCP/IP (eBPF)** *(newer 2025-26)* | window-scaling, ACK intervals, MSS — "headless Linux servers handle these differently than consumer Win11/macOS". | ASN-correlated. | soft-score. |
| **IP / ASN reputation** | datacenter ASN list, historical human:bot ratio for that IP across all CF zones. | **"bedrock of the risk model"**; datacenter starts degraded. | soft, but caps the achievable score. |
| **JSD probe** (always-on) | `__CF$cv$params` seed → `main.js` collects Screen, Navigator (platform/languages/HW concurrency/deviceMemory), Timezone, WebGL vendor+renderer, Canvas hash, Audio hash, font enumeration. LZ-string compress → base64 → urlencode → POST `…/jsd/oneshot` body `v_<r>=…`. | result in `cf.bot_management.js_detection.passed`; **only enforced if the zone wrote a WAF rule consuming it**. | usually soft (opt-in enforcement). |
| **Orchestrator (Managed)** | same fingerprint surface + runtime-consistency: patched `navigator.webdriver`, shimmed getter leaks via `Object.getOwnPropertyDescriptor`/`Function.prototype.toString`, `error.stack` shape, WASM-vs-JS timing envelope. | feeds the PoW difficulty. | soft → escalates challenge. |
| **Proof-of-Work** | Hashcash SHA-256 prefix-match, **difficulty scaled by the score so far** (clean residential = trivial; datacenter = hard). | gate. | **kill if not completed** (no `cf_clearance`). |
| **Behavioral ML** | mouse curvature/accel (perfect Bézier or straight line = fail; needs micro-jitter entropy), focus/blur/scroll timing, keystroke 50–150 ms variance. | interaction score. | soft on invisible, gate on widget. |

Key implication: **layers 1-4 set the PoW difficulty before our V8
runs a line of JS.** A perfect DOM that completes a *hard* PoW slowly,
from a datacenter ASN, can still be denied.

---

## 3. Challenge / script anatomy (bytes & structure)

`[MECHANISM]` 04_CLOUDFLARE.md §6 live capture + captchaai 8-step flow
+ cfresearch + roundproxies 2026.

**Stage 0 — interstitial** (the ~5.5 KB body, HTTP 403/503):

```
HTTP/2 403
server: cloudflare
cf-mitigated: challenge                ← canonical detection signal
cf-ray: <16hex>-<colo>
set-cookie: __cf_bm=…; HttpOnly; SameSite=None; Secure; Domain=<zone>
content-security-policy: default-src 'none'; script-src 'nonce-…'
   'unsafe-eval' https://challenges.cloudflare.com; … frame-src 'self'
   https://challenges.cloudflare.com blob:; worker-src blob: …
accept-ch / critical-ch: <17 UA-CH hints incl Sec-CH-UA-Full-Version-List>
server-timing: chlray;desc="<ray>"
<title>Just a moment...</title>
<script>window._cf_chl_opt = { cFPWv,cType,cRay,cZone,cN(nonce),
   fa,mdrd,cvId,cUPMDTk,cH,md … };
  a=document.createElement('script'); a.nonce=cN;
  a.src='/cdn-cgi/challenge-platform/h/<v>/orchestrate/<type>/v1?ray=…';
  document.head.appendChild(a);</script>
```

**Stage 1 — orchestrate GET** returns obfuscated JS that builds the
`cf-challenge-id` and the WASM glue.
**Stage 2 — flow POST**: `POST …/flow/ov1/<tok>:<epoch>:<tok>/<ray>/
<cf-chl-id>` body `v_<ray>=<encoded fingerprint+PoW>` header
`cf-challenge:`. Managed escalation spawns the **cross-origin
`https://challenges.cloudflare.com/` Turnstile iframe**; ≥2 further
flow POSTs with referer = Turnstile URL.
**Stage 3 — clearance**: a final POST to the target (fields incl. `md`,
`sh`, `aw`, `cf_ch_cp_return`) → response `Set-Cookie: cf_clearance=…`
then **302 → original URL**.

**The exact success signal**: `Set-Cookie: cf_clearance=…;
SameSite=None; Secure; Partitioned` `[MECHANISM]` (04 §2.2). No
postMessage success token is needed for the *invisible* path — it is
purely the cookie + the 302. (Turnstile-widget pre-clearance mode mints
`cf_clearance` *alongside* a `cf-turnstile-response` token.)

**WASM**: the orchestrator + Turnstile widget download a Rust-compiled
WASM module from `challenges.cloudflare.com` and call into it for
native-speed fingerprint extraction. `[MECHANISM]` CF's 2026 "most-seen
UI" blog: they "reimplement standard UI interactions using lower-level
browser APIs like `document.getElementById`, `createElement`, and
`appendChild`" because the WASM talks to the DOM through manual
JS-bridged calls. **We cannot stub the WASM inputs** — it runs against
our real DOM/Web APIs or not at all.

---

## 4. Fingerprint / sensor payload — field by field

`[MECHANISM]` (roundproxies JSD 2026 + 04_CLOUDFLARE.md §1.4):

`window['__CF$cv$params'] = { r, t, m, s }`:
- `r` — first 16 hex of `cf-ray` (decryption/binding key).
- `t` — base64 unix timestamp (e.g. `MTcwMjU4NjQ0Ny4wMDAwMDA=`).
- `m` — encoded metadata.
- `s` — per-load seed pair `[0x.., 0x..]` mixed into the fingerprint.

JSD/orchestrator fingerprint object → `LZString.compress(JSON) →
base64 → urlencode`, POSTed as `v_<r>=…`:

| Field | Expected real-Chrome-Linux value | How a mismatch scores |
|---|---|---|
| `Screen {Width,Height,ColorDepth}` | real monitor, colorDepth 24 | impossible combo (0×0, 32-bit) ⇒ kill |
| `Navigator.Platform` | `"Linux x86_64"` | must match UA platform |
| `Navigator.Languages` | `["en-US","en"]` matching `Accept-Language` | header/JS mismatch ⇒ soft kill |
| `Navigator.HardwareConcurrency` | 4–16; **PoW worker count = `ceil(hc*0.75 \|\| 6)`** | 0/1 ⇒ worker stalls; 128 ⇒ fake |
| `Navigator.deviceMemory` | 4/8 | must be a Chrome-allowed bucket |
| `Timezone` offset | consistent with IP geo | mismatch ⇒ soft |
| `WebGL {Vendor,Renderer}` | real GPU string (e.g. `ANGLE (…)` / Mesa) | "too clean"/programmatic ⇒ soft |
| `Canvas` hash | hardware-noisy, stable per session | perfectly deterministic ⇒ soft |
| `Audio` hash | DynamicsCompressor float noise | absent/zero ⇒ soft |
| `navigator.webdriver` | `false` (and `'webdriver' in navigator` semantics) | shim leak ⇒ kill-class |
| runtime consistency | getters/`toString` look native | `Object.getOwnPropertyDescriptor` reveals shim ⇒ kill-class |

`[HYPOTHESIS]` Our JS-level spoofs in `crates/stealth/src/gpu.rs` etc.
risk being "too clean / programmatic" relative to CF's real-Chrome-Linux
corpus (04 §8.2) — unverified, would need a real Chrome 147/148-Linux
capture to benchmark; not on the critical path until §3-iframe is fixed.

---

## 5. Crypto / encoding

`[MECHANISM]`:

- **No reversible client-side crypto for us to port.** Payload =
  `LZString.compress(JSON.stringify(fp))` → base64 → urlencode. The
  *PoW* is Hashcash SHA-256 prefix-match; difficulty + the exact target
  rotate **per Ray-ID / per ~30-min key window**. Re-implementing it in
  Rust is explicitly the wrong move (04 §0, RESEARCH_…05_10 §0) — the
  correct design is to run the orchestrator JS/WASM to completion.
- **`cf_clearance` is opaque & HMAC-bound** at issuance over: source IP,
  exact UA string, JA3 (JA4 on Enterprise), challenge Ray-ID, issuing
  colo. **Cannot be minted offline or hot-swapped between sessions.**
  It *must* be obtained inside the same TCP+TLS+IP session that reuses
  it. (CF clearance docs + Intsights/cf_clearance README.)
- **Invalidation**: IP change · UA change · TLS-fingerprint change ·
  Challenge-Passage TTL expiry (default 30 min) · CF policy push. JA4
  is *more lenient than JA3 noise* because it sorts extensions — works
  in our favour (we already shuffle like Chrome).
- **Anti-replay**: Turnstile token TTL = exactly 300 s, single-use,
  idempotent. The flow `v_` payload is bound to `r`(ray) + `s`(seed) +
  `t`(timestamp) — replay across loads fails.
- **Server-side cross-check**: every subsequent request carrying
  `cf_clearance` is re-validated against live JA3/JA4 — a non-browser
  TLS on the *next* fetch re-challenges even with a valid cookie.

---

## 6. Cookie & header lifecycle / state machine

`[MECHANISM]` (CF cookies doc + 04 §2 + captchaai flow):

```
first GET (no cookies)
  └─► 403 + Set-Cookie __cf_bm  (set on EVERY CF-BM zone, challenge or not;
        30-min SLIDING TTL; HttpOnly SameSite=None Secure; per-zone)
  └─► interstitial body (_cf_chl_opt)
orchestrate GET ──► flow POST(s) ──► [Turnstile iframe if escalated]
  └─► clearance POST ──► Set-Cookie cf_clearance
        (SameSite=None; Secure; Partitioned[CHIPS]; TTL=Challenge-Passage)
  └─► 302 → original URL
re-GET with cf_clearance + __cf_bm ──► 200 origin content
```

Valid→invalid transitions: `__cf_bm` lapses after 30 min idle ⇒ whole
ladder re-triggers · `cf_clearance` rejected on IP/UA/TLS change or TTL
expiry. Other cookies: `_cfuvid`/`__cfruid` (rate-limit visitor id,
session), `__cflb` (LB stickiness).

**"Solved" on the wire** = `Set-Cookie: cf_clearance=` on the clearance
POST response **then** the next top-level GET returns origin HTML
(no `cf-mitigated`, body not the interstitial).

---

## 7. How OSS / commercial tools defeat it

`[MECHANISM]` — citations + accessed 2026-05-16. **Skeptical read:
the pure-HTTP era is over; almost everything that works is "real
browser + stealth + residential IP".**

| Tool | Technique | 2026 status / reproducibility |
|---|---|---|
| **cloudscraper** (VeNoMouS) | Python, js2py/VM emulation of the challenge | **~dead for Managed Challenge.** Modern cvId:3 WASM defeats js2py. Repo still pushes "move deprecated CF v1" commits; "v1/v2/v3 solving" toggles exist but don't pass real Managed Challenge. codemanki original archived 2020. **Do not model on this.** |
| **FlareSolverr** | sidecar → undetected-chromedriver | **Maintenance/deprecated;** team announced no longer supported. "Same issues as undetected-chromedriver" — looped challenges, ~50% (down from ~92% in 2024). Can still work on weak WAF rules. |
| **undetected-chromedriver** | Selenium JS-surface patches | Deprecated Feb-2025; declining; CDP/Runtime.enable + patch leakage detected. |
| **curl-impersonate / curl_cffi** (lexiforest fork) | TLS+H2 fingerprint impersonation only, **no JS** | **Active; ~80% on Bot Fight Mode / cheap WAF, ~0% on Managed Challenge.** "TLS impersonation alone is no longer enough for most CF sites in 2026." |
| **nodriver** (ultrafunkamsterdam) | async raw-CDP, no Selenium, avoids Runtime.enable | Active; ScrapeOps-recommended; ~85% with good IPs. Real Chromium. |
| **SeleniumBase UC Mode** | Selenium + UC patches, built-in Turnstile helpers | Active, production; ~80%. |
| **Patchright** (Kaliiiiiiiiii-Vinyzu) | source-patched Playwright, no Runtime.enable | Active; ~85% on managed-challenge; engine under Theyka/Turnstile-Solver. |
| **Camoufox** (daijro) | custom **Firefox** build, **C++-level** fingerprint spoof | **Highest in field ~90%.** Spoofs nav/webgl/audio/canvas before JS sees it. Constrained to "be Firefox" (Firefox JA3/JA4 ≠ Chrome). |
| **Theyka/Turnstile-Solver, Body-Alhoha/turnaround, sarperavci/CloudflareBypassForScraping, x404xx** | Patchright/real-Chromium + scrape `[name=cf-turnstile-response]` | Active ~90% **only when the sitekey is known & widget is the gate**; do **not** solve the initial Managed gate / no WASM RE. |
| **xKiian/cloudflare-jsd** (Go) | reverse-engineered **JSD** `oneshot` POST, no browser | Active; **solves only the invisible JSD layer**, explicitly *not* Turnstile. Useful *only* on zones that gate purely on JSD (rare). The closest thing to a portable algorithm — but JSD ≠ Managed Challenge. |
| Scrapfly / Browserless / Bright Data | cloud browser farm + residential | ~95%, paid; bundles IP + clearing browser. |

**What CF patched that killed older bypasses** `[MECHANISM]`:
(1) JA4 (sorts extensions → JA3 GREASE/shuffle tricks moot, 2024).
(2) cvId:3 WASM orchestrator (2025) → js2py/pure-JS emulation dead.
(3) Runtime-consistency + Castle.io-class checks → naive
`navigator.webdriver=false`/`window.chrome` JS patches leak.
(4) PoW difficulty scaled by score → datacenter IPs get
computationally throttled even when fingerprint-perfect.
(5) eBPF TCP-stack probing (2025-26) → headless-Linux TCP behaviour
itself is a tell.

**browser_oxide's natural class**: a **Camoufox analog in Rust** — no
CDP, byte-level TLS control, real V8. Our edge: multi-profile. Our gap:
DOM/iframe/WASM-bridge maturity (§8/§10) — Camoufox wins today because
its iframe + engine-level surface is complete and ours is not.

---

## 8. What browser_oxide does today (file:line evidence)

**Detector** — `crates/stealth/src/cloudflare.rs` (370 lines):
- `CfChallengeKind` (`Managed|Jsch|NonInteractive|Interactive|Unknown`),
  `v1_solvable()` gates out `Interactive` (`:67-73`).
- `detect_challenge(headers,body)` (`:140-190`): fires on
  `cf-mitigated:challenge` **OR** (`_cf_chl_opt`|`/cdn-cgi/
  challenge-platform/`) AND `server:cloudflare`. Extracts
  `cType,cRay,cZone,cN,fa,mdrd,cFPWv`. **WIRED & exercised** (called
  from the live path) — *but see §10a on the synthetic-header hack*.
- 11 unit tests, all synthetic, **no network**.

**Live path** — `crates/browser/src/page.rs`:
- `handle_cloudflare_flow(&client)` `:314-445`. Called once per nav
  iteration at `:1824` (in `navigate_loop_internal`, *after*
  `handle_akamai_flow`, *after* the build phase). Flow:
  1. `:318` read `self.content()` (post-build mutated DOM, **not the
     raw 403 body** — see §10a #2).
  2. `:322-329` **synthesize `server:cloudflare`** into a fake header
     map (real response headers are *not* available here).
  3. `:331` `detect_challenge`; `:343-349` bail if not `v1_solvable`.
  4. `:351-368` if `cf_clearance` already in jar → return (assume win).
  5. `:376-403` inject ≤30 ticks of synthetic mousemove/scroll/keyup
     **`window.dispatchEvent` (untrusted) noise**.
  6. `:408-437` poll cookie jar for `cf_clearance` / `_boxide.
     __pendingNavigation` every 250 ms for **10 s**, then return.
  - Component status: **WIRED but ineffective.** It is a *passive
    waiter*, not a solver. It never fetches `/orchestrate/…`, never
    POSTs the flow/JSD payload, never loads the Turnstile iframe.
- Classifier — `body_has_challenge_marker` `:167-194` +
  `challenge_verdict` `:274-293`. `/cdn-cgi/challenge-platform/` is a
  *strong* (always-on, size-independent) marker → see §10a #1.
- Cookie-delta retry `:1836-1841`: re-issues the same URL next
  iteration iff `is_anti_bot_challenge()` (or DD/sec-cpt flags) AND
  iterations remain. **This is the only thing that would consume a
  `cf_clearance`** — and it only fires because the body still has the
  marker, not because clearance arrived.

**Iframe loader** — `crates/browser/src/iframe.rs`:
- `ChildIframe::from_url` `:73-104` **does** cross-origin fetch + CSP
  `frame-src` enforcement. Capable in principle.
- Invoked only at `page.rs:3139-3176` inside
  `build_page_with_scripts_init_and_storage`, iterating
  `iframe::find_iframes(&dom_state.dom)` — a **one-shot static DOM
  scan at build time**.
- Dynamic insertion hook `dom_bootstrap.js:1924-1950`
  (`Node.prototype.appendChild`): an `appendChild`'d
  `HTMLIFrameElement` is pushed to a JS array and given a **synthetic
  on-demand `window[N]`** via `_getIframeWindow` — **no HTTP fetch,
  no `from_url`, no inner-script execution.** → **MISSING** for the CF
  Turnstile iframe.

**Dynamic script loader** — `dom_bootstrap.js:142-258`
(`_onNodeInsertedInner`) + `fetch_ext.rs:411-430` (`op_net_fetch_sync`):
an `appendChild`'d `<script src>` **is** sync-fetched & `eval`'d
(with depth/CSP guards). So the *inline `_cf_chl_opt` script* that
appends the orchestrator `<script>` **will** run and the orchestrator
JS **will** be fetched+eval'd. **WIRED.** The chain then dies at the
WASM/iframe/flow-POST stage, not at script loading.

**WASM** — `crates/browser/tests/wasm_smoke.rs` (commit `8f89e39`)
confirms V8 WebAssembly + streaming work. WASM is a V8-native feature
(no Rust shim needed). **WIRED** (capability present; whether the CF
WASM's *DOM bridge* gets everything it needs is the open risk, §9).

**Cookie jar** — `crates/net/src/cookies.rs`: parses `domain`, `path`,
`secure`, `httponly` (`:167-215`); keyed by domain→(name→cookie)
(`:10`). **`SameSite` and `Partitioned`/CHIPS are NOT parsed or
honored.** → §10b #4.

**Test profile** — `cloudflare_udemy.rs:22` uses
`stealth::chrome_130_linux()` while all CF research targets Chrome
147/148. Stale → §10b #5.

**No dedicated CF "orchestrator" module exists.** The word
"orchestrator" in our code/docs refers to *Cloudflare's* script that we
hope runs inside our V8 — **there is no browser_oxide component that
produces `cf_clearance`.** It is entirely "let CF's JS self-solve in
our engine + poll the jar". `[CODE]` confirmed by exhaustive grep:
zero code fetches `/orchestrate/`, posts `/flow/`, or sets
`cf_clearance`.

---

## 9. GAP ANALYSIS — what we are missing (ranked, concrete)

> Distinguishing IP from engine: **udemy's "datacenter-ASN IP block"
> is an unverified hypothesis** (`00_MASTER_PLAN.md` §1.6 — no clean
> `nocdp` hard-403 capture; box environment blocked the retest).
> Per the standing rule, treat engine-addressable until a captured
> hard-403 from `nocdp.sh` proves otherwise. The gaps below are
> ordered so the *engine* gaps (which we can fix and which gate
> *everything* even with a perfect IP) come first.

**G-CF-1 — Cross-origin Turnstile iframe is never loaded.**
`[CODE]` Evidence: `dom_bootstrap.js:1924-1950` (appendChild iframe →
synthetic `window[N]`, no fetch) vs `page.rs:3139-3176`
(`from_url` only at build-time static scan).
Blast radius: **every Managed Challenge that escalates to Turnstile**
= udemy/economist/quora and the entire CF class.
Difficulty: **High** (needs a post-build dynamic-iframe loader that
calls `ChildIframe::from_url` with the parent's CSP, runs the inner
challenges.cloudflare.com document in an isolated V8 origin, and
round-trips `postMessage`).
Risk: medium (iframe machinery touches Kasada/DataDome realm tests —
must be gated/regression-checked).
Fix: in `handle_cloudflare_flow`'s poll loop (or a MutationObserver
hook), detect newly-inserted `<iframe src^="https://challenges.
cloudflare.com">`, call `from_url`, wire its `contentWindow.postMessage`
to the parent. This is the **single highest-leverage** engine fix.

**G-CF-2 — No active orchestrate/flow driver; we only wait 10 s.**
`[CODE]` `page.rs:408-437` is a passive cookie poll.
Even for the **invisible** (no-widget) Managed path the orchestrator
must complete its `/orchestrate/…` GET → `/flow/ov1/…` POST chain
inside our V8. We never verify it ran or inspect why it stalled.
Difficulty: Medium. Fix: add a `__fetchLog` assertion (the DataDome
trace primitive at `page.rs:1869-1875` already exists) to enumerate
every fetch/XHR the orchestrator issued; identify the first missing
call (likely `Worker` w/ `worker-src blob:`, `MessageChannel`,
`navigator.sendBeacon`, or the WASM `instantiateStreaming` of a
cross-origin module). Then fill that primitive.

**G-CF-3 — `cf_clearance` cookie attributes (Partitioned/SameSite)
not parsed.** `[CODE]` `cookies.rs:167-215`. Even if G-CF-1/2 land and
CF sets `cf_clearance; SameSite=None; Secure; Partitioned`, our jar
drops the attributes; a clearance set *inside the cross-site Turnstile
iframe* (CHIPS-partitioned) may not be presented on the parent's
re-GET. Difficulty: **Low** (parse + honor `SameSite`/`Partitioned`).
Risk: low.

**G-CF-4 — Real response headers not threaded to the detector.**
`[CODE]` `page.rs:322-329` fabricates `server:cloudflare`; the
authoritative `cf-mitigated`, `cf-ray`, `server-timing:chlray`,
`accept-ch/critical-ch` headers are unavailable at the hook.
Blast radius: degraded telemetry + `cType` mis-read as `Unknown`
post-mutation (W7 doc observed this). Difficulty: Low (mechanical
plumb-through, ~1 h). Risk: none.

**G-CF-5 — `navigator.hardwareConcurrency` PoW-worker formula
unverified.** `[MECHANISM]` PoW workers = `ceil(hc*0.75 || 6)`.
`[HYPOTHESIS]` if our DOM returns 0/1 the worker bundle stalls (no
PoW → no clearance); if 128 we look fake. Difficulty: Low (assert our
value is a realistic 4–16). Risk: low.

**G-CF-6 — iOS Safari TLS profile incomplete (economist/quora).**
`[MECHANISM]` (04 §5/§7, RQUEST_MOBILE_TLS_AUDIT): need 20 ciphers,
10 sigalgs w/ duplicate, 4 curves (no PQ +P-521), **no ALPS / no
session_ticket / no ECH**, zlib cert-compression, H2 SETTINGS
`2=0,3=100,4=2097152,9=1`, WINDOW_UPDATE 10420225, pseudo-header
`m,s,a,p`, and **drop Sec-CH-UA-\*** (Safari has none) + don't
advertise PAT support. Blast radius: economist+quora iOS-only.
Difficulty: Medium (2–3 dev-days, recipe exists). Risk: low — gated
by profile.

**G-CF-7 — IP / ASN reputation (the honest IP component).**
`[MECHANISM]` datacenter ASN = degraded trust = harder PoW; "bedrock
of the risk model". `[HYPOTHESIS]` for udemy specifically this may be
*load-bearing on top of* G-CF-1/2 — but it is **not established**
(no hard-403 capture). **Evidence that would settle it**: one clean
`ab_harness/nocdp.sh` run to `https://www.udemy.com/` from this IP —
if a real CDP-free Chrome **also** gets a hard 403/"Access Denied"
(not a solvable challenge), the IP is load-bearing; if it renders
udemy, the gap is 100% engine (G-CF-1/2). Until then: **fix the
engine gaps first** (they gate even a perfect IP), add a per-site
residential-proxy *config flag* as plumbing (no engine change).
Difficulty: trivial plumbing / unbounded if it's truly IP. Risk: n/a.

---

## 10. FALSE-POSITIVE ANALYSIS of our code

### 10a. Detection FPs

**FP-D1 — `/cdn-cgi/challenge-platform/` is a *strong, size-independent*
marker → a *solved* CF page can still be classed a challenge.**
`[CODE]` `page.rs:175` (`body_has_challenge_marker`, strong-marker arm,
**not** gated by `stub_sized`) + `:277-285` (`challenge_verdict`).
- False claim: "body contains `/cdn-cgi/challenge-platform/` ⇒
  blocked/challenge".
- Why false: after a Managed Challenge *partially or fully* runs, CF's
  own `scripts/jsd/main.js` (`…/challenge-platform/scripts/jsd/…`) is
  injected into **every CF page including a successfully-cleared one**
  (JSD is always-on, §1). The W7 live run (`W7_…05_10.md` §"Live
  result") observed exactly this: `body length: 476222`, **title =
  the real "Udemy: Online Courses…"** (orchestrator ran, document.title
  set), yet `still on challenge page: true` *only because the JSD URL
  string remained in the body*. With body 476 KB ≥ 50 KB,
  `challenge_verdict` returns **`SensorFail`** — this is the master
  plan's "udemy = sensor-fail (476 KB body renders)" line.
- **Verdict on the master-plan question**: this is a **partial FP**.
  The 476 KB is *not* a clean pass (no `cf_clearance`, the 302-to-origin
  never fired, it's still the challenge document with title patched) —
  so calling it "blocked" is *directionally correct*. **But** labelling
  it `SensorFail` ("vendor JS ran and sensor scored us bot") is the
  **wrong sub-class**: the orchestrator did not *fail a sensor*, it
  *never completed the flow* (no iframe, no flow POST). The correct
  verdict is "challenge-incomplete / orchestrator-stalled", a distinct
  bucket. Mislabelling it `SensorFail` sends future work toward
  fingerprint tuning when the real gap is G-CF-1/2 (structural).
- Catching test: a fixture with a 476 KB body containing
  `/cdn-cgi/challenge-platform/scripts/jsd/main.js` **and** a real
  `<title>` **and** *no* `_cf_chl_opt` blob ⇒ assert verdict is a new
  `ChallengeIncomplete`, not `SensorFail`. (Also: a genuinely cleared
  CF page that still ships JSD would be a true FP — needs the
  `cf_clearance`-in-jar check to override the marker.)

**FP-D2 — synthetic `server:cloudflare` header makes the *post-build*
content drive detection.** `[CODE]` `page.rs:318` (`self.content()`)
+ `:322-329`.
- False claim: the detector sees "the Cloudflare challenge response".
- Why false: it sees the **post-orchestrator-mutated DOM**, not the
  raw 403. W7 observed `cf=Unknown ray= zone=` on iter 1+ because the
  inline `_cf_chl_opt` blob was already mutated away; only the JSD URL
  survived. `cType` (which gates `v1_solvable` and the
  interactive-fast-fail) is lost ⇒ an *interactive* Turnstile could be
  mis-read as `Unknown` (which `v1_solvable()` treats as solvable) ⇒
  we waste the 10 s budget on an unsolvable challenge.
- Catching test: feed `handle_cloudflare_flow` a page whose raw body
  was `cType:'interactive'` but whose mutated `content()` lost the
  blob; assert we still fast-fail interactive. Requires G-CF-4 (thread
  real headers / capture the raw first-response body).

**FP-D3 — `detect_challenge` requires `server:cloudflare` OR
`cf-mitigated`; downstream proxy stripping `server` + a
non-mitigated challenge variant ⇒ under-match (false PASS).**
`[CODE]` `cloudflare.rs:159`. Low-likelihood (the synthetic-header hack
at `page.rs:328` masks it in practice), documented for completeness.
Catching test: body with `_cf_chl_opt`, headers with neither `server`
nor `cf-mitigated` → currently `None` (miss).

### 10b. Solver / logic FPs

**FP-L1 — "wired-but-unreachable": the `cf_clearance` path dead-ends
because no component produces the cookie.** `[CODE]`
`page.rs:351-441`. The code *reads* `cf_clearance` from the jar in two
places and logs "orchestrator likely succeeded" / "cf_clearance issued
— outer loop will retry". 
- False claim (in code comments + W7 doc): "the orchestrator typically
  either 302s or assigns `location.href` after issuing clearance" /
  "let the orchestrator run to completion and poll".
- Why false: for a modern cvId:3 Managed Challenge the orchestrator
  **cannot complete** in our engine — the cross-origin Turnstile iframe
  is never fetched (FP/G-CF-1) and no flow POST is driven (G-CF-2). The
  `cf_clearance`-in-jar branches are **unreachable in practice** for
  the only sites this code exists for. This is the canonical
  "exists ≠ exercised" FP, exactly the class the inventory §calls out
  (DdEncryptor/solve_crypto analog) — except here it's not dead *code*,
  it's a **dead success-path**: the solver's success condition can
  never be met by the solver itself.
- Catching test: an integration assertion that, given a real CF
  Managed Challenge fixture, `handle_cloudflare_flow` results in a
  `cf_clearance` cookie — it would **fail today**, exposing the
  dead-end. (The existing `cloudflare_udemy.rs` test's pass criterion
  is deliberately loose — "scaffolding ran, no panic" — so it
  **cannot** catch this. That looseness is itself the FP-enabler.)

**FP-L2 — untrusted synthetic events as "behavioral noise".**
`[CODE]` `page.rs:376-403` dispatches `new MouseEvent(...)` via
`window.dispatchEvent` ⇒ `event.isTrusted === false`, and the motion is
a deterministic arithmetic path (`100 + (i*7)%400`).
- False claim (comment `:370-375`): this satisfies a "25-event
  phase-1 gate".
- Why false: `[MECHANISM]` 2026 behavioral ML explicitly fails
  "straight line / perfect Bézier" and requires non-linear entropy +
  micro-jitter; CF largely ignores untrusted events for *credit* but
  the *deterministic pattern* of any events it does see is a negative
  signal. This noise is at best inert, at worst a tell. It is not a
  solver step.
- Catching test: assert the injected coordinates have realistic
  jitter/variance (and ideally are trusted) — or remove the claim that
  it satisfies a gate.

**FP-L3 — cookie-delta retry is gated on `is_anti_bot_challenge()`,
which is keyed off the mutable post-build `self.content()`.**
`[CODE]` `page.rs:1836` (`page.is_anti_bot_challenge()`) →
`:263-266` → `self.content()`. Same mutable-state hazard class as the
Akamai doc-20 sec-cpt bug (inventory §"shared findings"). If the
orchestrator mutates the body such that `/cdn-cgi/challenge-platform/`
is removed but clearance was **not** obtained, the retry **does not
fire** and we silently accept a non-cleared page (false PASS). Inverse
of FP-D1.
- Catching test: post-mutation body with the marker stripped but no
  `cf_clearance` in jar ⇒ assert we still retry (gate on a persistent
  `started_as_cf_challenge` flag, mirroring `started_as_seccpt_challenge`
  at `page.rs:1812`, **which does not exist for CF**).

**FP-L4 — `cf_clearance` would not survive being set in the Turnstile
iframe (CHIPS).** `[CODE]` `cookies.rs:167-215` ignores
`SameSite`/`Partitioned`. Pre-emptive FP: even after G-CF-1/2, a
clearance `Set-Cookie` issued inside the cross-site
`challenges.cloudflare.com` iframe is CHIPS-partitioned; our jar would
either store it un-partitioned (and possibly not send it on the parent
GET) or, with strict matching, drop it. Catching test: parse a
`Set-Cookie: cf_clearance=x; SameSite=None; Secure; Partitioned` and
assert it is presented on the partition-correct parent request.

**FP-L5 — stale test profile.** `[CODE]` `cloudflare_udemy.rs:22`
`chrome_130_linux()` vs Chrome 147/148 everywhere else. A Chrome-130
UA in 2026 is itself a mild bot signal (version too old) and makes the
one e2e test non-representative. Catching: align to the current
profile constructor used by the rest of the suite.

---

## 11. The concrete pass-guarantee plan

Ordered. Engine-first (gates everything; cheap to verify offline last).

1. **G-CF-4 (1 h, mechanical):** thread the *raw first-response*
   headers + body into `handle_cloudflare_flow` (don't re-derive from
   mutated `content()`). Unblocks correct `cType`, fixes FP-D2,
   enables interactive fast-fail, and gives real telemetry. Add a
   persistent `started_as_cf_challenge` flag (mirror
   `started_as_seccpt_challenge` `page.rs:1812`) → fixes FP-L3 +
   correct cookie-delta retry gating.
2. **G-CF-2 (instrument, 0.5 d):** reuse the `__fetchLog` primitive
   (`page.rs:1869-1875`) to dump every fetch/XHR the orchestrator
   issues against a live udemy challenge. Identify the **first missing
   call** (expected: cross-origin iframe, `Worker` blob, or
   `instantiateStreaming`). This is measurement, not a fix — it tells
   us exactly which of G-CF-1/5/WASM to do.
3. **G-CF-1 (high, 2–4 d): the load-bearing fix.** Post-build
   dynamic-iframe loader: on `appendChild` of
   `<iframe src^="https://challenges.cloudflare.com">`, call
   `ChildIframe::from_url` with the parent CSP, execute the inner doc
   in an isolated V8 origin, and bridge `postMessage` both ways.
   Regression-gate against Kasada/DataDome iframe-realm tests.
4. **G-CF-3 + FP-L4 (low, 0.5 d):** parse + honor
   `SameSite`/`Partitioned` in `cookies.rs`; present partitioned
   `cf_clearance` on the matching parent request.
5. **G-CF-5 (low):** assert `navigator.hardwareConcurrency` ∈ [4,16].
6. **FP-L2 (low):** replace deterministic untrusted noise with
   jittered (ideally trusted) motion, or drop the "satisfies gate"
   claim.
7. **G-CF-6 (2–3 d, economist/quora):** ship the iOS Safari TLS/H2
   profile (RQUEST_MOBILE_TLS_AUDIT recipe); drop Sec-CH-UA-\* for iOS;
   don't advertise PAT.
8. **G-CF-7 (plumbing):** add a per-site residential-proxy config
   flag (no engine change). **Do not** invoke it as the explanation
   until step 9.
9. **Verification regime:**
   - **Decisive IP/engine settler:** one clean `ab_harness/nocdp.sh`
     run to `https://www.udemy.com/`. Hard-403 ⇒ IP load-bearing;
     renders ⇒ pure engine (G-CF-1/2). **This single experiment
     resolves master-plan Open-Q #4 for udemy.** (Could not be run
     this session — box rootless-Xwayland + command-backgrounding
     fights long nocdp runs; flagged, not assumed.)
   - **Network-free §4 gate structurally CANNOT verify CF**: the gate
     has no live CF endpoint, the challenge PoW/WASM rotates per
     Ray-ID, and `cf_clearance` is HMAC-bound to a live IP+TLS
     session. Offline unit tests can only assert *detection* and
     *cookie-attribute parsing* + the FP-catching fixtures above. End
     state must be proven by a live `--ignored` udemy run showing
     `Set-Cookie: cf_clearance` then a 200 origin body — there is no
     offline substitute.
   - Tighten `cloudflare_udemy.rs`: replace the "scaffolding ran"
     pass criterion with "`cf_clearance` present AND body is real
     udemy AND no `cf-mitigated`" so FP-L1 cannot hide again.

**Honest expectation:** even with steps 1–6 perfect, udemy may still
require step 7 (residential IP) because PoW difficulty scales with ASN
reputation — but steps 1–3 are *prerequisite regardless of IP* and are
the only part fully in our control. economist/quora are higher
probability (>70% per 04 §9.2) once G-CF-6 lands, because their gap is
the iOS TLS profile, not the Turnstile-iframe path (they only escalate
to Turnstile *because* the iOS profile is wrong).

---

## 12. Sources & experiments

### External (URL — claim — accessed 2026-05-16)

**Cloudflare official:**
- developers.cloudflare.com/cloudflare-challenges/concepts/clearance/ — `cf_clearance` device/IP-bound, non-transferable.
- developers.cloudflare.com/cloudflare-challenges/challenge-types/javascript-detections/ — JSD always-on, enforcement opt-in.
- developers.cloudflare.com/cloudflare-challenges/challenge-types/turnstile/ — Turnstile = challenge-platform tech; PoW/proof-of-space.
- developers.cloudflare.com/bots/concepts/bot-score/ — ML majority of detections.
- developers.cloudflare.com/bots/additional-configurations/ja3-ja4-fingerprint/signals-intelligence/ — 10 JA4 metrics.
- developers.cloudflare.com/fundamentals/reference/policies-compliances/cloudflare-cookies/ — `__cf_bm` 30-min sliding, encrypted, per-zone.
- blog.cloudflare.com/the-most-seen-ui-on-the-internet-redesigning-turnstile-and-challenge-pages/ — Rust→WASM UI; manual createElement/appendChild DOM bridge; 5.35 B challenges/day 2025.
- blog.cloudflare.com/ja4-signals/ — JA4 sorts extensions; Rust `client-hello-parser`; 15 M unique JA4.

**RE / flow:**
- github.com/scaredos/cfresearch (archived 2025-01-20) — orchestrate GET → flow/ov1 POST `v_<ray>=…` → clearance POST (`md/sh/aw/cf_ch_cp_return`) → `cf_clearance`. **Era 2024-25; uncertain for 2026 — labelled accordingly.**
- roundproxies.com/blog/jsd-solver-cloudflare/ (2026) — `__CF$cv$params {r,t,m,s}`; LZString→base64→urlencode; `…/jsd/oneshot`; fingerprint field list.
- blog.captchaai.com/cloudflare-challenge-session-flow-walkthrough — 8-step flow, cookie table, IP binding.
- tapscape.com/cloudflare-turnstile-bypass-2026-the-core-level-stealth-guide/ — 3-layer model (JA4+/eBPF, runtime-consistency, ML biometrics); why JS-injection leaks.
- medium.com/@ayushaggarwal42003/…cloudflare-bot-management…2026… — ASN reputation = "bedrock"; JSD identifies headless.
- scrapfly.io/blog/posts/how-to-bypass-cloudflare-anti-scraping — FlareSolverr/UC dead; nodriver/SeleniumBase/Camoufox current; TLS-only ~0% on Managed.
- github.com/VeNoMouS/cloudscraper + github.com/codemanki/cloudscraper — codemanki archived 2020; VeNoMouS still pushing "deprecate CF v1"; js2py dead on cvId:3.
- github.com/FlareSolverr/FlareSolverr (issues/releases) — deprecated/maintenance; looped challenges.
- github.com/xKiian/cloudflare-jsd — Go, JSD-only, explicitly not Turnstile.
- (corroborating, from 04_CLOUDFLARE.md §12, re-validated): ijazurrahim.com 2026 Turnstile internals; johal.in PoW-without-user; daijro/camoufox; Kaliiiiiiiiii-Vinyzu/patchright; Theyka/Turnstile-Solver; lexiforest/curl-impersonate; Intsights/cf_clearance.

### Internal docs consumed
- `docs/research/engines/00_INVENTORY_AND_METHOD.md` (contract).
- `docs/research_2026_05_16/00_MASTER_PLAN.md` §1.6, §2 (udemy unverified-IP rule).
- `docs/research_2026_05_14/04_CLOUDFLARE.md` (60 KB; live captures, solver table).
- `docs/RESEARCH_CLOUDFLARE_BYPASS_2026_05_10.md`; `docs/W7_CLOUDFLARE_V1_2026_05_10.md` (V1 scope + the diagnostic-gold live run).

### Local experiments (command — result)
- `grep -rn` for `handle_cloudflare_flow|cf_clearance|orchestrat|challenge-platform|__pendingNavigation|from_url` across `crates/` → confirmed: **zero code fetches `/orchestrate/`, posts `/flow/`, or sets `cf_clearance`**; iframe `from_url` only at build-time static scan (`page.rs:3139-3176`); appendChild-iframe hook is synthetic-only (`dom_bootstrap.js:1924-1950`); appendChild-`<script src>` IS sync-fetched+eval'd (`dom_bootstrap.js:142-258`, `fetch_ext.rs:411-430`).
- Read `crates/stealth/src/cloudflare.rs` (full, 370 ln), `crates/browser/src/page.rs` (:120-445, :1790-1895, :3125-3204), `crates/browser/src/iframe.rs` (:22-141), `crates/net/src/cookies.rs` (:1-345), `crates/browser/tests/cloudflare_udemy.rs` (full).
- `git log --grep` confirmed CF work landed `6dbd44e` (W7-deep V1), `9d5f96d` (udemy CSP bypass); no CF commit since.
- **Not run** (per constraints — heavy/network, flagged not assumed): live `udemy_cloudflare_orchestrator --ignored`; `ab_harness/nocdp.sh` udemy (box environment blocks long nocdp runs — this is the one decisive missing experiment, see §11.9).

---

*End. The CF gap is structural, not fingerprint-cosmetic: we detect
the challenge correctly and then have no machinery to complete it. Fix
order: G-CF-4 → G-CF-2(measure) → G-CF-1 → G-CF-3 → verify with one
clean nocdp udemy run. Negative result of record: "browser_oxide has
no Cloudflare solver — only a detector and a 10 s passive poll whose
success branch is unreachable for modern Managed Challenge."*
