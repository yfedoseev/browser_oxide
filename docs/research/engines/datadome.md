# DataDome — Deep Engine Analysis (browser_oxide)

**Created:** 2026-05-16 · **Baseline git HEAD:** `fd98bfa` · **Agent:** DataDome deep-research
**Contract:** follows `docs/research/engines/00_INVENTORY_AND_METHOD.md` template (§0–§12).
**Fact tagging:** `[MECH]` = externally-cited mechanism fact · `[CODE]` = our code, file:line I read · `[HYP]` = hypothesis/inference (labeled).

---

## 0. Executive summary & pass-guarantee thesis

**How DataDome decides bot vs human in 2026** `[MECH]`: a layered pipeline —
(1) edge TLS/JA3+JA4 + IP-reputation tier (datacenter = strong negative,
residential = neutral, mobile-carrier = strong positive) + HTTP/2 frame +
header-order; (2) the `tags.js` silent sensor (110 KB, lightly obfuscated,
no VM) collecting a ~40-field fingerprint + a behavioral event array,
encrypted with a static dual-XOR/PRNG + custom-base64 scheme and POSTed to
`api-js.datadome.co/js/`; (3) per-customer ML scoring (DataDome states
85,000+ customer/use-case models, 5T signals/day) yielding a 0..1 risk;
(4) above a tenant threshold the silent path is replaced by a ~770 B
**interstitial** `var dd={...}` body that loads `ct.captcha-delivery.com/i.js`
(invisible Device Check, `rt:'i'`) or `c.js` (slider, `rt:'c'`). Since
2026-02-11 the **Device Check + Slider payloads run inside a custom
bytecode VM** layered on top of the existing WebAssembly `boring_challenge`
and continuous dynamic regeneration — a three-layer defense with no public
spec. The single decisive design fact: **the deep probes (Picasso canvas,
audio render hash, WASM PoW, daily-rotated 6-char signal keys) live ONLY
in the post-failure challenge payload — closing the silent-pass gap removes
the trigger entirely.** `[MECH]` (03_DATADOME §3.12, §14)

**The single highest-leverage gap blocking us:** browser_oxide has **no
real cross-origin child browsing context for a *script-created* iframe**.
Our iframe handling (`page.rs:3130-3176`, `iframe::find_iframes`) scans
the *static parsed DOM once at build time* and instantiates a one-shot
`ChildIframe::from_url`; a `<iframe>` injected by `i.js` via
`document.createElement` + `iframe.src=…geo.captcha-delivery.com…` only
gets the synthetic `contentWindow` *fingerprint-parity shim*
(`dom_bootstrap.js:2034+`), which does **not** fetch or execute the
challenge document, run its VM/WASM, or postMessage a cookie back. So the
`rt:'i'` chain physically cannot complete in-engine. `[CODE]`

**Honest verdict — the 4 DataDome sites (etsy, leboncoin, tripadvisor, yelp):**

| Site | Real state today | Why |
|---|---|---|
| **leboncoin** | **PASSES (renders)** on desktop profile | DataDome silently allows desktop-Chrome-on-this-IP (HTTP 200, 348 KB). Only the *mobile* profile blocks it, and that is an **IP-tier** problem (mobile-UA-on-datacenter-IP), not an engine bug. `[MECH]` 03 §9.2 / LEBONCOIN doc |
| **etsy** | **BLOCKED** — `rt:'i'` interstitial, ~779 B, `DataDome-CHL` | i.js loads (200/15 KB, CSP-fix works) but the script-created challenge iframe → WASM → postMessage → daily-key chain never completes in-engine. `[CODE]`+master plan §8.5 |
| **tripadvisor** | **BLOCKED** — same `rt:'i'` interstitial as etsy | Identical mechanism + chain to etsy. `[MECH]` 03 §9.3 |
| **yelp** | **OUT OF STEALTH SCOPE** — interactive-captcha class | 2026-05-16 nocdp: real CDP-free Chrome from this exact IP got a *solvable interactive "solve-this-task"* DataDome captcha, not a hard 403 ⇒ **NOT an IP ban** (03 §9.4's `t:'bv'` IP-ban claim is **falsified**); it is a human-interaction gate, same bucket as the 5 captcha sites. `[MECH]` master plan §1.6 (supersedes 03) |

**Pass-guarantee thesis:** 1 of 4 already passes (leboncoin desktop). yelp
is out of scope by definition (human captcha). etsy + tripadvisor share
**one** irreducible blocker: the vendor's `rt:'i'` obfuscated bundle must
fully self-solve in our V8 against a *live daily-rotating* oracle. The
two viable strategies (in §11) are **(A) close the silent-pass gap so the
interstitial is never served** [cheapest, the doc-03/18 strategic intent]
and **(B) build a real script-created cross-origin child context + run the
VM/WASM** [the unbuilt L capability]. The mandatory network-free §4 gate
**structurally cannot verify either flip** (both need a live daily-keyed
DataDome oracle).

---

## 1. Vendor surface & 2026 deployment

`[MECH]` (03_DATADOME §1–2; vendor changelog; ZenRows/Scrapfly 2026)

- **Product tiers:** Bot Protect (the JS sensor + ML), CAPTCHA / Device
  Check (the challenge payloads), Account Protect, mobile SDK
  (`api-sdk.datadome.co/sdk/`, out of web scope).
- **Script names / hosts:**
  - `js.datadome.co/tags.js` (or first-party-proxied `/<rand>/tags.js`)
    — the silent sensor, **v5.6.x**, ~110 KB, *string-table obfuscation
    only, NO VM* `[MECH]` 03 §1.2/§14.1.
  - `ct.captcha-delivery.com/i.js` — interstitial Device Check loader
    (live etsy capture 2026-05-16: **200, 15,014 bytes** — this is the
    *small loader*, NOT the VM bundle, which the iframe fetches) `[CODE]`
    master plan §8.5.
  - `ct.captcha-delivery.com/c.js` — slider loader.
  - `geo.captcha-delivery.com/interstitial/` and `/captcha/` (+`/captcha/check`)
    — the cross-origin challenge iframe + submission endpoints.
  - `api-js.datadome.co/js/` — the silent-path sensor POST endpoint.
- **Sensor encryption is a STATIC target** `[MECH]`: the
  `glizzykingdreko/datadome-encryption` README states the algorithm "has
  remained virtually unchanged since the first release of their captcha
  challenge" (repo last pushed **2025-12-14**, verified via `gh api`).
  Only the *6-char signal-key dictionary* rotates *daily*.
- **2026-02-11 VM rollout** `[MECH]` (datadome.co/changelog/vm-based-obfuscation;
  Security Boulevard 2026-02): VM bytecode added to **Device Check +
  Slider** (i.js/c.js), NOT tags.js. Three-layer = VM + WASM +
  continuous regeneration; "no public spec, no documentation, no existing
  RE tools"; opcodes are 16-bit (4919/5003/4961). xKiian/datadome-vm
  (107★, last push 2026-02-16) is a WIP disassembler PoC.
- **Per-tenant build/ruleset stamp** `x-dd-b` `[MECH]` 03 §5.3: etsy/tripadvisor=259
  (newest), yelp=2, wsj/reuters=1 (legacy CloudFront-Lambda@Edge cluster).
- **Our target sites:** etsy (`rt:'i'`, x-dd-b 259, riskscore 0.367),
  tripadvisor (`rt:'i'`, x-dd-b 259, Max-Age 31104000=360 d),
  leboncoin (silent 200 desktop), yelp (interactive captcha class).

---

## 2. Detection pipeline — stage by stage

`[MECH]` 03 §1–4, §10–12; ZenRows/Scrapfly 2026

| Stage | Signal collected | Scoring | Kill vs soft |
|---|---|---|---|
| **Edge TLS** | JA3 + **JA4** (cipher/extension/version order), record version | IP-tier-weighted | soft-score; coherence with UA matters |
| **Edge IP** | ASN class: datacenter≈53% pass, residential≈91%, mobile-carrier≈97% (2025 DataDome benchmark) | dominant prior | near-kill for datacenter on aggressive tenants |
| **Edge HTTP/2** | frame fingerprint, header order, HTTP/1.1-vs-h2 | soft-score | h1-only is a strong tell |
| **Headers** | UA / Sec-CH-UA-* / Accept-Language / Accept; `accept-ch` negotiation requested *on the 403 itself* | cross-checked vs TLS+JS | missing requested client-hints on retry scores worse |
| **`tags.js` silent JS** | ~40-field fp (navigator scalars, screen, storage, codec mask, WebGL main+Worker, matchMedia, typeof-mask, PerfNavTiming) + behavioral event array | encrypted POST → ML | individual fields soft; `webdriver`/empty `languages`/empty `plugins`/Worker-vs-main WebGL mismatch = kill |
| **Behavioral** | `_initialCoordsList` (load→first interaction) + `_coordsList` (slider) | 31 movement features (curvature, jerk, velocity variance…) | **empty `_initialCoordsList` is a *stronger* kill than an imperfect one** (03 §3.11 — the decisive silent-pass gap) |
| **Server ML** | all of the above × per-customer model | 0..1 risk; tenant threshold picks silent / `rt:'i'` / `rt:'c'` / hard | `x-datadome-riskscore` is emitted on rejection by some tenants (etsy=0.367 ⇒ *fingerprint-driven, not IP*) |
| **Challenge payload** (post-failure only) | Picasso canvas hash, OfflineAudioContext hash, WASM `boring_challenge` PoW, dynamic hash-chain, `Function.toString` native checks, CDP/Runtime.enable probe | VM-evaluated in the iframe | any monkey-patch / CDP attach = kill |

**Load-bearing:** stages 1–6 happen *before* any challenge. If silent
score < threshold, NO interstitial, NO VM, NO WASM, NO daily key. browser_oxide
is structurally immune to the headless/CDP/automation-sentinel class (we
embed V8, no CDP, not Playwright, not an extension) `[MECH]` 03 §3.2/§7.

---

## 3. Challenge / script anatomy (bytes & structure)

`[MECH]` 03 §6, verified against our parser test fixture `[CODE]`
`datadome_handler.rs:247` (real reuters body).

The interstitial body (~770–779 B inner HTML; etsy 779, tripadvisor 778,
yelp 776) is exactly:

```html
<html lang="en"><head><title><site></title>
<style>#cmsg{animation:A 1.5s;}@keyframes A{0%{opacity:0}99%{opacity:0}100%{opacity:1}}</style></head>
<body style="margin:0"><p id="cmsg">Please enable JS and disable any ad blocker</p>
<script data-cfasync="false">var dd={'rt':'i','cid':'…==','hsh':'<30hex>','b':2096871,'s':<int>,
 'e':'<96hex>','rr':'','qp':'','host':'geo.captcha-delivery.com','cookie':'<base64>'}</script>
<script data-cfasync="false" src="https://ct.captcha-delivery.com/i.js"></script></body></html>
```

Field semantics (03 §6.1): `rt` `'i'`=Device Check / `'c'`=slider; `t`
(slider only) `'fe'`=solvable / `'bv'`=IP-banned; `cid`=client id;
`hsh`=120-bit server hash; `b`=build stamp; `s`=per-request salt
(feeds the §5 encoder); `e`=48-byte server HMAC binding (IP,UA,time);
`host`=`geo.captcha-delivery.com`; `cookie`=base64 of current datadome.

**Flow & success signal** `[MECH]` 03 §2/§6.3/§7:
i.js loads → builds challenge URL → injects **cross-origin iframe**
`geo.captcha-delivery.com/interstitial/?initialCid=…&hash=…&cid=…&s=…&b=…&dm=cd`
→ iframe loads its own VM-protected bundle → runs Picasso canvas + audio
hash + **WASM `boring_challenge`** (seed 10–20 M, CPU-core hint,
nested XOR/shift/rotate busy-loop returning a 64-bit result — a CPU-tax
PoW, NOT crypto) + dynamic hash-chain → **postMessage** result to parent
→ parent writes `datadome=` cookie → `window.location` reload. Success =
`{"cookie":"datadome=…","view":"redirect","url":"…"}` 03 §4.5.

The interstitial body classifies as `ChallengeVerdict::EdgeBlock` (marker
+ <50 KB) `[CODE]` `page.rs:277-285`.

---

## 4. Fingerprint / sensor payload — field by field

`[MECH]` 03 §3. Full table is in 03_DATADOME §3.1–3.11; the load-bearing
points for browser_oxide:

- **Kill signals (any one fails):** `navigator.webdriver` truthy;
  `navigator.languages.length===0`; `navigator.plugins.length===0`;
  `'openDatabase' in window` true on Chrome 116+ (must be **false**);
  main-thread WebGL vendor/renderer ≠ Worker/OffscreenCanvas WebGL
  vendor/renderer; UA contains `HeadlessChrome`; any automation sentinel
  (`_phantom`, `$cdc_…`, `__playwright…`, etc.). browser_oxide is
  naturally clean on the whole automation-sentinel class `[MECH]` 03 §3.2.
- **Coherence (HIGH risk if disagree):** `ua` ↔ TLS JA4/ALPN ↔ Sec-CH-UA;
  `pf` (platform) ↔ UA; `tzp` (`getTimezoneOffset`) ↔ `tz`
  (`Intl…timeZone`); `mob` ↔ UA ↔ Sec-CH-UA-Mobile.
- **Worker cross-checks:** the tag spawns a Blob Worker that re-reads
  `userAgentData`, WebGL via `new OffscreenCanvas(1,1).getContext('webgl')`,
  `Intl…timeZone`; mismatch vs main thread = kill `[MECH]` 03 §3.7.
  Prior W6a-matrix flagged our Worker `userAgentData` returned "NA"
  (03 §17.1 item 3) — **must verify current state** (see §9 G4).
- **typeof-mask:** ~30 globals (`MozMobileMessageManager`,
  `CSS2Properties.MozOSXFontSmoothing` = Firefox tells;
  `ContactsManager` = Chrome-Android-only). Our
  `interfaces_bootstrap.js` ships 600+ entries; no asserting regression
  test that the 32-bit mask byte-matches real Chrome 147 (03 §3.9).
- **`x-datadome-riskscore` diagnostic** `[MECH]` 03 §5.3: etsy emits
  **0.367** on the 403 — *below 0.5 ⇒ the rejection is fingerprint-driven
  and engine-fixable*, NOT IP. This is the strongest evidence the
  silent-pass route (§11 strategy A) is viable for etsy.

---

## 5. Crypto / encoding

`[MECH]` 03 §4 + `18_DATADOME_ENCRYPTION_REFERENCE` + glizzykingdreko README;
`[CODE]` byte-verified port in `crates/akamai/src/datadome_crypto.rs`.

- **Constants** (`datadome_crypto.rs:19-21`): `MAIN_PRNG_CONSTANT
  9_959_949_970`; `HASH_XOR_INTERSTITIAL -883_841_716` (captcha path
  uses `-1_748_112_727`); `CID_PRNG_CONSTANT 1_809_053_797`. Matches
  doc 18 table exactly.
- **Algorithm** (faithfully ported): `_customHash` djb2-×31 (empty ⇒
  sentinel 1789537805) → `_mixInt` xorshift32 (JS signed `>>`) →
  stateful `DdPrng` (3 bytes/round, `result = state >> (16-8*round)`,
  optional `^= --saltState` when `useAlt`, 1-deep cache on `flag`) →
  per-signal buffer (`{`/`,` start marker XOR, key XOR, `:` XOR, value
  XOR) → second `cidPrng` global XOR pass → `}` terminator → custom
  base64 with a decrementing-salt XOR. `[CODE]` `datadome_crypto.rs:25-259`.
- **Byte-parity proof** `[CODE]` `datadome_crypto.rs:371`
  `byte_parity_vs_glizzykingdreko_node_reference`: pinned salt 424242,
  expected `r-aCk21w_gy22p95upHvmSEliaBlfgcgzBeEwuIr3dk0D6HtzqBFgBcQtE`
  (len 58). This is a **real correctness proof of the encoder** — but
  see §10b: the encoder is **DEAD CODE** (zero non-test callers).
- **Daily key rotation** `[MECH]` 03 §4.4 / ZenRows 2026: the *signal
  names* on the wire are daily-rotated random 6-char strings (e.g.
  `bcl`, `cssH`, `dp0`). The dictionary "internal-name → today's wire
  name" is built inside `tags.js`/`i.js` with literal assignments
  (Babel-AST extractable). **This is the moving target** — the encoder
  is static; the *key map* and the *server `e`/HMAC + JA4 cross-check*
  are not. A pre-built offline poster must re-extract the dict daily.
- **Anti-replay / server-side:** the `datadome` cookie is an opaque
  *server-side* encrypted blob (NOT the §4 client encryption) with a
  rolling timestamp; **IP-bound** (cookie minted on IP A rejected from
  IP B); no Privacy Pass / blind tokens ⇒ **no offline minting** — every
  fresh session must hit DataDome at least once `[MECH]` 03 §5.4-5.5.

---

## 6. Cookie & header lifecycle / state machine

`[MECH]` 03 §5; `[CODE]` `cookies_have_datadome` `datadome_handler.rs:220`.

- **`datadome` cookie**: single source of truth, `Max-Age=31536000`
  (tripadvisor 31104000), `Domain=.<site>; Path=/; Secure; SameSite=Lax`
  (wsj anomalously no `Secure`); HttpOnly varies by tenant. Issued on
  *every* navigation (always a `Set-Cookie`, even on the 403).
- **Auxiliary:** `dd_testcookie` (transient cookie-works probe),
  `ddSession` (localStorage dup when `sessionByHeader:true`),
  `ddOriginalReferrer` (sessionStorage, post-solve redirect target).
- **"Solved" on the wire** = a *new* `datadome=` value lands (from the
  iframe postMessage → parent `document.cookie` write) AND the reloaded
  top-level request returns 200 with full content.
- **Our state model is a thin boolean**, not a real state machine:
  `cookies_have_datadome(jar)` (true iff a `datadome=` token exists)
  `[CODE]` `datadome_handler.rs:220-224`, used as the break/retry signal
  at `page.rs:1756`, `1861`, `2238`. **It cannot distinguish a
  *failed/low-trust* datadome cookie (always issued, even on the 403)
  from a *solved high-trust* one** — see §10b FP-2.

---

## 7. How OSS / commercial tools defeat it

Citations: accessed **2026-05-16**. Skeptical assessment.

| Source | Technique | Reproducible? | Patched/dead? |
|---|---|---|---|
| `github.com/glizzykingdreko/datadome-encryption` (40★, last push **2025-12-14**) | Clean-room Node port of the §5 client encoder; documents captcha vs interstitial constants | **YES** — we byte-verified our Rust port against it (`datadome_crypto.rs:371`) | Encoder unchanged since first release; **NOT dead**. But README itself says it is *payload encryption only*, "incomplete for real-world deployment" (author now sells TakionAPI) `[MECH]` |
| `github.com/glizzykingdreko/Datadome-Deobfuscator` | Babel-AST deobf of `tags.js` (string decode, dead-code, rename) | YES for tags.js (no VM) | Maintained; tags.js still string-table-only |
| `github.com/glizzykingdreko/Datadome-Interstitial-Deobfuscator` (34★, last push **2024-01-12**) | Deobf of `i.js` | **NO — STALE.** Predates the 2026-02-11 VM rollout | Effectively **dead** vs current VM-protected i.js |
| `github.com/xKiian/datadome-vm` (107★, last push **2026-02-16**) | PoC VM bytecode *disassembler* (`disasm.js`); documents dispatcher + ~3 of 16-bit opcodes | Partial — WIP, ~3 opcodes; not a working solver | Active but far from complete; "build a disassembler from scratch" is the cost DataDome intended `[MECH]` vendor changelog |
| `github.com/manjustice/datadome-vm-internals` | VM internals notes (surfaced 2026-02 search) | Notes only | Research-stage |
| `github.com/glizzykingdreko/Datadome-Interstital-Encryptor`, `…/datadome-encryption-python`, `combo23/datadome_generator`, `d-suter/datadome-bp`, `romainp12/datadome-gen`, `66niko99/PyDatadome` | Cookie-gen / request-shape helpers | Mostly request-shape only | Several self-admit "may not work anymore" / target tag 4.6.x (now 5.6.x) — **largely dead** |
| `github.com/rebrowser/rebrowser-patches` | Disables `Runtime.Enable` CDP leak on every frame | YES for CDP-driven automation | **N/A to us** — we don't speak CDP; the entire leak class is naturally absent (a structural win) `[MECH]` 03 §16.3 |
| Hyper Solutions SDK (`hyper-sdk-go/py/js`), Capsolver, CapMonster, 2Captcha, TakionAPI, Scrapfly (claims 96%), Bright Data Web Unlocker | Server-side sensor gen / human-farm slider solve + bundled mobile proxy | B2B, closed-source; useful only for *endpoint/cookie shape reference* | Pay-per-call; not clean-room; success claims are vendor self-reported, no third-party verification (eldorar/Surfsky 99.4% is **promotional, unverified**) |
| nodriver / SeleniumBase-CDP / Camoufox / curl-impersonate (Scrapfly 2026) | Real-browser stealth + TLS-impersonation + residential/mobile proxy + warm-up nav | Partially — Scrapfly: nodriver "~25% baseline without proxies"; "no universal bypass, each site is a different challenge" | Living arms race; no offline solver claimed to work for VM-protected i.js |

**Net external truth (skeptical):** the *silent-path encoder* is a solved
static problem (glizzykingdreko, our port verified). The *post-2026-02 VM
challenge payload* has **no public working solver** — only a partial
disassembler. Every credible 2026 source converges on: **win on the
silent path / run the real vendor JS in a real browser context; do not
expect an offline interstitial solver to work or stay working.** This
*exactly matches* doc-18's "ARCHITECTURE PIVOT" (let i.js self-solve
in-engine, encoder = insurance).

---

## 8. What browser_oxide does today (file:line evidence)

**Exact path from `navigate()` to a DataDome success signal:**

1. `page.rs:1247` — on the initial response body,
   `datadome_handler::detect_datadome_interstitial(&html)` parses the
   `var dd={…}` literal (≤4096 B gate). `[CODE]` **WIRED & exercised.**
2. `page.rs:1258` — `plan_datadome_solve(&dd,&resp.url)`: builds a
   `DdSolvePlan` for `rt:'i'` (challenge_hosts + renav_url). **Log-only
   `eprintln!` — the plan is printed, never acted on.** `[CODE]`
   **wired-but-INERT** (`datadome_handler.rs:191-208`; the returned
   struct's fields are never consumed).
3. `page.rs:1367-1376` & `:1420` — `is_datadome_challenge_doc(&html)`
   (= detector `.is_some()`); when true the **origin's restrictive 403
   CSP is NOT enforced** so i.js can reach captcha-delivery.com
   (`enforce = … && !dd_challenge_doc`, `page.rs:1368`). `[CODE]`
   **WIRED & exercised** (Phase-5 Inc 1, live-verified: i.js now loads
   200/15 KB).
4. `page.rs:1420` — `started_as_dd_challenge` computed *once* from the
   original body (persistent flag). `[CODE]` **WIRED.**
5. `page.rs:1729-1766` — if `started_as_dd_challenge`, the pending-nav
   poll stays active up to 90 s and breaks early when a `datadome=`
   cookie appears. `[CODE]` **WIRED.**
6. Build phase (`build_page_with_scripts_*` → ~`page.rs:3010-3068`):
   inline `<script src=i.js>` executes in V8, 8 s `run_until_idle`
   (`page.rs:3066`). `[CODE]` **WIRED — i.js loads & runs.**
7. `page.rs:3130-3176` — **iframes processed by `find_iframes` over the
   *static parsed DOM* once**, then `ChildIframe::from_url`
   (`page.rs:3148`). `[CODE]` **WIRED but MISSING the needed behavior:**
   only finds iframes present at scan time; a script-created challenge
   iframe is not converted to a real child context (see §9 G1, §10b FP-4).
8. `page.rs:1836-2018` — universal cookie-diff retry; the
   `v8_html_is_real` gate at `page.rs:2008-2018` **explicitly excludes
   `captcha-delivery.com`** so the post-i.js body is never accepted as
   "real".
9. `page.rs:2185-2248` — same-host-reload in-V8 refetch; Phase-5 **Inc 8
   self-solve window** at `page.rs:2224-2248` (45 s pump, break on
   `datadome=`) — gated behind `same_host_reload` which requires a
   pending GET **and** `is_anti_bot_challenge()` (see §10b FP-3:
   likely unreachable for etsy).
10. `crates/akamai/src/datadome_crypto.rs` `DdEncryptor` — byte-verified
    `[CODE]` **DEAD CODE: zero non-test callers** (grep proof in §10b).

**Component status table:**

| Component | file:line | Status |
|---|---|---|
| interstitial detector/parser | `datadome_handler.rs:67` | WIRED & exercised |
| `DdSolvePlan` / `plan_datadome_solve` | `datadome_handler.rs:191` | wired-but-INERT (log-only `page.rs:1259`) |
| CSP-exempt challenge doc | `page.rs:1368` | WIRED & exercised (Inc 1) |
| `started_as_dd_challenge` routing | `page.rs:1420,1731,1837,2224` | WIRED & exercised |
| `cookies_have_datadome` break/retry | `page.rs:1756,2238` | WIRED (but coarse, FP-2) |
| `dd_flow_summary` trace | `page.rs:1859` | WIRED, debug_nav-only |
| script-created cross-origin child iframe | — | **MISSING** (the core gap) |
| WASM `boring_challenge` exec in iframe realm | — | **MISSING** (no iframe ⇒ never reached) |
| `DdEncryptor` client encoder | `datadome_crypto.rs:163` | **DEAD CODE** (0 callers) |
| daily 6-char wire-key extractor | — | **MISSING** (only matters for the offline-poster path, which is not taken) |
| silent-path sensor poster to `api-js.datadome.co/js/` | — | **MISSING** (by design — strategy is in-engine self-solve) |

---

## 9. GAP ANALYSIS — what we are missing (ranked, concrete)

**G1 — No real child browsing context for a *script-created* iframe.**
`[CODE]` evidence: `page.rs:3137` `find_iframes(&dom_state.dom)` is the
*only* iframe-discovery call in the build path; `iframe.rs:255-287`
walks the DOM tree (so a script-inserted `<iframe>` *node* would be
found *if* re-scanned), but the scan runs **once** at build time and
`page.rs` has **no post-JS re-scan** and **no hook on
`createElement('iframe')`/`iframe.src=`** that spins up a real
`ChildIframe` with its own isolate+network. The synthetic
`contentWindow` (`dom_bootstrap.js:2034+`) is a *fingerprint-parity
shim*, not a fetching/executing context.
- **Blast radius:** etsy + tripadvisor (the entire `rt:'i'` chain). Also
  the latent enabler for any future cross-origin-iframe challenge.
- **Difficulty:** L (large). **Risk:** high — touches the build/iframe
  architecture; could regress the iframe-isolation §4-gate tests.
- **Concrete fix:** after the build drain (`page.rs:3066`), and again
  inside the `started_as_dd_challenge` self-solve pump, **re-run
  `find_iframes` and instantiate `ChildIframe::from_url` for any new
  script-created iframe whose src host ∈ challenge_hosts**, then keep a
  parent↔child `postMessage` bridge alive so the child's posted cookie
  reaches the parent jar. (Note: even built, the child still has to run
  the VM/WASM against the daily-key/JA4 oracle — see G2.)

**G2 — No in-iframe execution of the VM-protected `boring_challenge`
chain.** `[MECH]` 03 §7/§14: post-2026-02 i.js/c.js payloads are VM
bytecode + WASM + dynamic regen. We *have* native V8 WASM (doc 18 notes
`WebAssembly.*` functional) and a Chrome-faithful navigator/audio
surface, so *if* G1 is built the child could in principle run it — but
the daily-rotating key + server-side `e`/HMAC + JA4 cross-check are an
external live oracle.
- **Blast radius:** etsy + tripadvisor. **Difficulty:** L. **Risk:**
  high + unverifiable offline. **Fix:** depends on G1; then live-oracle
  dev loop (not §4-gate verifiable).

**G3 — Silent-pass closure (the *cheaper, doc-sanctioned* strategy is
not pursued).** etsy's `x-datadome-riskscore=0.367` `[MECH]` 03 §9.1
proves the block is **fingerprint-driven**, and 03 §3.12 proves the
challenge is only served *after* failing silent scoring. So a cheaper
path than G1/G2 is to **never trigger the interstitial**: (a) synthesize
`_initialCoordsList` (5–10 mousemove events between nav and onload —
03 §3.11/§17.1.6, "the decisive gap"); (b) honor the `accept-ch` on
retry (send all 7 Sec-CH-UA-* hints); (c) Worker `userAgentData` parity;
(d) `getTimezoneOffset`↔`Intl.timeZone` agreement; (e) Worker-vs-main
WebGL identity. **None of these are DataDome-specific RE; all are
fingerprint hygiene.**
- **Blast radius:** etsy + tripadvisor *and* cross-vendor (Akamai/Kasada
  behavioral). **Difficulty:** M (mostly existing surfaces). **Risk:**
  low (no challenge-bundle dependency). **Fix:** wire
  `crates/stealth/src/behavior.rs` mouse-path emitter into the
  nav→onload window; audit Sec-CH-UA retry; verify the 4 coherence
  shims. **Caveat:** still cannot be *verified* by the network-free §4
  gate (needs the live oracle), but it is the lowest-risk, highest-cross-
  vendor-leverage gap and matches doc-03/18 intent.

**G4 — `DdSolvePlan` is computed then thrown away.** `[CODE]`
`page.rs:1258-1269`: `plan_datadome_solve` returns a plan whose
`challenge_hosts`/`renav_url` are only `eprintln!`'d. The CSP-exemption
that *does* fire is keyed off `is_datadome_challenge_doc` independently
(`page.rs:1367`), so the plan is decorative. **Difficulty:** S.
**Risk:** none. **Fix:** either consume the plan (drive host-allow +
renav from it) or delete it and the wiring claim it implies (it
currently overstates how wired the solver is — a doc/measurement FP,
see §10b FP-1).

**G5 — `DdEncryptor` is dead code (and, given the in-engine strategy,
likely *should not* be wired).** See §10b FP-5. The gap is not "wire
it" — doc 18's pivot says the offline poster needs the daily wire-key
dict + signal map + server `e`/JA4 we don't have, which is *why* the
strategy is in-engine self-solve. The real gap is **truth-in-labeling**
(call it insurance/dead, not "the solver").

**leboncoin — does it truly pass?** `[MECH]` 03 §9.2 + LEBONCOIN doc:
**YES on desktop** — HTTP 200, 348 KB, fresh datadome cookie, no
challenge. The *mobile* profile blocks (1404 B `DataDome-CHL`) but that
is mobile-UA-on-datacenter-IP (DataDome's mobile track expects carrier
IPs), an **infrastructure** problem with no engine fix. For the
desktop-profile sweep leboncoin is a genuine pass, not an FP. **Verify
in the live sweep that the desktop profile is the one used** (if the
sweep runs an Android profile, leboncoin will spuriously count as a
DataDome block — a measurement artifact, not an engine gap).

**yelp — scope.** `[MECH]` master plan §1.6 (newer; supersedes 03 §9.4):
real CDP-free Chrome from this IP gets a *solvable interactive captcha*,
not a hard 403 ⇒ **not IP-banned, not stealth-fixable** — it is a
human-interaction gate (same class as duolingo/medium/quora/spotify/
substack). yelp is correctly **out of stealth scope**; counting it as a
DataDome engine failure is a scope error. (Standing rule: do not assert
"IP ban" without a captured hard-403 from nocdp.)

---

## 10. FALSE-POSITIVE ANALYSIS of our code

### 10a. Detection FPs (do we mislabel pass↔block?)

**FP-A — three divergent DataDome classifiers with *different
`captcha-delivery.com` size gates*.** `[CODE]`

| Classifier | DataDome trigger | Size gate |
|---|---|---|
| `page.rs:186-187` `body_has_challenge_marker` (drives `is_anti_bot_challenge` `:263`, `challenge_verdict` `:274`) | `captcha-delivery.com` OR `dd-script` | **NONE — fires at ANY body size** (only the weak `dd_engagement` at `:193` is gated `<50 KB`) |
| `holistic_sweep.rs:892` `classify` (the metric behind the 120/126 ledger & the directive's re-measure) | `captcha-delivery.com` = *phrase* marker | only `< 30 KB`; `ddcaptchaencoded` `:874` any size |
| `datadome_handler.rs:71` `detect_datadome_interstitial` | `captcha-delivery.com` **AND** `var dd=` | **≤ 4096 B** |

- **False claim:** "a page mentioning captcha-delivery.com is a DataDome
  challenge."
- **Why false:** a *fully-rendered* DataDome-protected page legitimately
  inlines `js.datadome.co/tags.js` and may reference
  `captcha-delivery.com` assets while serving full content. Under
  `holistic_sweep` (>30 KB ⇒ `L3-RENDERED`=pass) it is a **pass**; under
  `page.rs` (no size gate on the strong marker) the same body is
  `SensorFail` / `is_anti_bot_challenge()==true`. So the *same site* can
  be "pass" by the ledger metric and "blocked" by the live-path guard —
  exactly the Phase-0 over-match class (master plan G1). The
  `page.rs:2015` `v8_html_is_real` exclusion of `captcha-delivery.com`
  *also* means a genuinely-rendered post-solve etsy body would be
  rejected as "not real" purely for containing the substring.
- **Catching test:** a regression asserting all three classifiers agree
  on (i) the real 779 B interstitial fixture ⇒ challenge, and (ii) a
  >100 KB rendered body that contains `captcha-delivery.com` ⇒ NOT a
  challenge. `datadome_handler.rs:263`
  (`ignores_large_body_with_captcha_delivery_substring`) only covers its
  own ≤4 KB-gated function — the *page.rs* path has no such test and is
  the one that over-matches.

**FP-B — thin-shell "pass" in the other direction.** `[CODE]`
`page.rs:286-292`: no challenge marker + `len < 5*1024` ⇒
`RenderIncomplete`; `len ≥ 5 KB` ⇒ `Pass`. A DataDome interstitial is
~770 B and *does* carry `captcha-delivery.com`, so it is caught — **but**
a post-i.js DOM where i.js *removed* its own `var dd` + `<script
src=i.js>` (i.js mutates the live DOM) and left a >5 KB shell with no
marker would classify `Pass` while no real content rendered. The Inc-8
comment at `page.rs:1408-1419` explicitly documents this DOM-mutation
hazard for the *poll/retry* path; `challenge_verdict` has the symmetric
exposure for the *pass* verdict. **Catching test:** post-i.js
synthetic-DOM fixture (marker removed, body 6 KB, no real content) must
NOT classify `Pass`.

**FP-C — `started_as_dd_challenge` cannot regress non-DD sites (verified
negative).** `[CODE]` `page.rs:1421` = `is_datadome_challenge_doc` which
requires the ≤4 KB `var dd=`+`captcha-delivery.com` shape ⇒ false for
every non-DataDome body incl. the entire §4 gate. This narrow gating
claim **holds** (a true negative — good).

### 10b. Solver / logic FPs

**FP-1 — "the DataDome solver is wired" is FALSE; it is detect +
log-only + CSP-tweak.** `[CODE]` `page.rs:1258-1269` only `eprintln!`s
the `DdSolvePlan`. The module's own header comment
(`datadome_handler.rs:23-33`) still says "Full solver is staged for a
follow-up commit". No code POSTs a sensor, runs the challenge to
completion, or consumes a solved cookie deterministically. **Why false:**
the only behavioral effects are (a) CSP-exemption so i.js can load and
(b) keeping the poll/retry active longer — neither *solves* anything;
the master plan's own live measurement (§8.5) confirms etsy still
returns the 403/805 interstitial. **Catching test:** an integration
assertion that, given the reuters interstitial fixture, *some*
deterministic state transition occurs beyond logging — there is none, so
the test would (correctly) fail today, exposing the overstatement.

**FP-2 — `cookies_have_datadome` cannot tell a *failed* datadome cookie
from a *solved* one.** `[CODE]` `datadome_handler.rs:220-224` returns
true iff a `datadome=` token exists. But 03 §5.1 `[MECH]`: **DataDome
issues a `Set-Cookie: datadome=` on *every* navigation, including the
403 interstitial itself.** So the break conditions at `page.rs:1756-1762`
(break poll when `datadome` appears and wasn't there before) and
`page.rs:2238` (Inc-8 break) can fire on the **fail cookie that came
with the interstitial**, not a solved one — a *false success* signal
that ends the wait early and proceeds to a reload that re-blocks.
**Why false:** presence ≠ trust; only a 200 + full body proves solve.
**Catching test:** a flow test where `cookies_before` is empty,
`cookies_after` contains the *interstitial's own* datadome cookie, and
the next fetch still returns the 403 — the helper says "gained" /
`cookie_gained=true` (`datadome_handler.rs:239`) while the site is still
blocked. (`dd_flow_summary` already encodes `cookie_gained = after &&
!before`, which is exactly this falsifiable-by-fail-cookie predicate.)

**FP-3 — the Inc-8 self-solve window is likely UNREACHABLE for etsy.**
`[CODE]` `page.rs:2224-2248` sits behind `same_host_reload`
(`page.rs:2193`) = `pending_method=="GET" && page.is_anti_bot_challenge()
&& same-host`, and the whole pending-nav block returns early at
`page.rs:2111-2113` if `pending_url` is empty. The Inc-8 comment
*assumes* "a DataDome `rt:'i'` nav sets a reload `__pendingNavigation`
EARLY" (`page.rs:2210-2214`) — but the master plan's **deepest etsy
trace** (§8.5) states i.js loads, connects to geo.captcha-delivery.com,
and **creates no iframe and no pending navigation**. If i.js sets no
pending nav, `pending_url` is empty ⇒ function returns at `page.rs:2111`
**before** the self-solve window. Additionally `is_anti_bot_challenge()`
reads the *post-i.js mutated DOM* (`page.rs:264`); i.js removes its own
`var dd`/script, so if no `captcha-delivery.com`/`dd-script` substring
survives, the guard flips false and the window is skipped even with a
pending nav. **Why false:** the increment's premise (early pending nav)
contradicts the captured trace. **Catching test:** a flow test with the
etsy-shaped interstitial where i.js sets no `__pendingNavigation` — the
Inc-8 `eprintln!("self-solve window…")` must be asserted reached; it
will not be, proving the window is dead for the actual etsy behavior.
(Honest negative: Inc 8 is gate-green and harmless, but it is *inert*
for the site it was written for — a "exists ≠ exercised" FP.)

**FP-4 — `find_iframes` over the static DOM means the challenge iframe
is structurally never built.** `[CODE]` `page.rs:3137` + `iframe.rs:255`:
the scan is one-shot at build time; even though it walks the DOM tree
(would find a script-inserted node *if re-run*), there is no re-scan and
no `createElement('iframe')` hook. Combined with the synthetic-shim
`contentWindow` (`dom_bootstrap.js:2034+`, a parity object), DataDome's
`iframe.src = geo.captcha-delivery.com/interstitial/?…` does **nothing
real**. **Why false (the implicit claim):** comments throughout the
DataDome path imply "i.js runs in our V8 and the round-trip completes" —
the round-trip *cannot* complete because its central step (a fetching,
executing, postMessaging cross-origin child) does not exist. **Catching
test:** an instrumented flow on the reuters/etsy interstitial asserting a
`ChildIframe` for `geo.captcha-delivery.com` is created — none is.
(This is the master plan §8.5 "ZERO child iframe is ever created",
here root-caused to `page.rs:3130-3176`.)

**FP-5 — `DdEncryptor` byte-verified but ZERO non-test callers (DEAD
CODE).** `[CODE]` grep proof (commands in §12): `DdEncryptor`,
`new_interstitial`, `main_prng_seed`, `cid_prng_seed`, `DdPrng`,
`encode_payload` appear **only** inside `crates/akamai/src/datadome_crypto.rs`
(definitions + its own `#[cfg(test)]` module) and one *doc comment*
(`datadome_crypto.rs:11`). The akamai crate exports it
(`crates/akamai/src/lib.rs:56 pub mod datadome_crypto;`) and `page.rs:4
use akamai;` exists, but **no `akamai::datadome_crypto::` path is ever
called anywhere.** **False claim:** the byte-parity test
(`datadome_crypto.rs:371`) proves *correctness of the encoder*, NOT that
encryption happens in the live navigate() path. The canonical
"exists ≠ exercised" FP (inventory §"Shared known FP", master plan G5).
**Important nuance (honest):** per doc-18's ARCHITECTURE PIVOT, the
encoder is *intentionally* "insurance" and the strategy is in-engine
self-solve — so the fix is **NOT** "wire it" (that needs the daily
wire-key dict + §3 signal map + server `e`/JA4 we don't have). The fix
is **truth-in-labeling**: mark it dead/insurance in code+docs, add an
`#[allow(dead_code)]` with a comment, OR delete it; and stop any doc
implying DataDome has a wired solver.

---

## 11. The concrete pass-guarantee plan (4 sites)

**leboncoin — already passes (desktop).** Action: *measurement* only —
ensure the sweep uses the desktop Chrome profile for leboncoin; if a
mobile profile is in play, exclude leboncoin from the DataDome failure
count (it is an IP-tier artifact, not an engine gap; mobile-carrier-IP
infra is out of engine scope). **Verification:** §4 gate cannot test
network; the holistic sweep desktop run already shows `L3-RENDERED`.

**yelp — out of scope, fix the label.** Action: classify yelp as
human-interactive-captcha (master plan §1.6), not DataDome-engine and
not IP-ban. No engine work. **Verification:** the nocdp data point
(already captured) is the proof; nothing for the §4 gate to do.

**etsy + tripadvisor — two ordered strategies (do A first; A is cheaper,
lower-risk, cross-vendor):**

**Strategy A — close the silent-pass gap so the interstitial is never
served** (doc-03 §3.12 / doc-18 intent; etsy riskscore 0.367 proves
fingerprint-driven):
1. Wire a behavioral mouse-path emitter (`crates/stealth/src/behavior.rs`,
   Catmull-Rom/Bezier, ~60 Hz jittered) into the nav→onload window so
   DataDome's `addEventListener('mousemove')` populates a non-empty
   `_initialCoordsList` (03 §3.11 — "the decisive gap"). Cross-vendor
   (also helps Akamai/Kasada behavioral).
2. Audit Sec-CH-UA-* retry: every retry after the `accept-ch` 403 must
   carry all 7 hints (`crates/stealth/src/presets.rs` + net layer).
3. Verify the 4 coherence shims: Worker `userAgentData` (not "NA"),
   Worker-vs-main WebGL identity, `getTimezoneOffset`↔`Intl.timeZone`,
   `mob`↔UA↔Sec-CH-UA-Mobile.
4. Add the asserting regression test for the typeof 32-bit mask vs a
   captured real Chrome 147 bitmask (03 §3.9).

**Strategy B — build the real script-created cross-origin child (only if
A insufficient; this is the unbuilt L capability):**
5. After the build drain (`page.rs:3066`) and inside the
   `started_as_dd_challenge` pump, re-run `find_iframes`; for any new
   iframe whose host ∈ challenge_hosts, instantiate a real
   `ChildIframe::from_url` with its own isolate + network + a live
   parent↔child `postMessage` bridge; keep it alive across the pump.
6. Let the child fetch+run the VM-protected bundle on our native V8
   WASM + Chrome-faithful surface; capture the posted cookie into the
   shared jar; let the existing cookie-diff retry re-issue the original
   URL — **after fixing FP-2** (distinguish solved vs fail cookie:
   require a subsequent 200 + >threshold body, not mere `datadome=`
   presence).
7. Consume (not just log) `DdSolvePlan` (G4); remove the
   `captcha-delivery.com` exclusion in `v8_html_is_real`
   (`page.rs:2015`) so a genuinely-rendered post-solve body is accepted.
8. Keep `DdEncryptor` as documented insurance (label it; do not claim
   it solves).

**The verification regime — and where the §4 gate structurally cannot
go:** Strategy A's *fingerprint hygiene* parts (shims, mask, header
order) ARE network-free-§4-gate-verifiable via parity tests. But the
*flip itself* (etsy/tripadvisor → rendered) is gated by a **live,
daily-rotating DataDome oracle + server-side JA4** — the network-free §4
gate (`chrome_compat`, `iframe_isolation`, `v8_*`, `datadome` unit)
**cannot** verify it. The directive's own `holistic_sweep` live
re-measure is the only valid verifier and it requires an
explicitly-authorized live-oracle dev loop (a captured daily challenge
as fixture). This is the honest structural ceiling, identical to the
master plan §8.5 Phase-5 conclusion — restated here as the DataDome
engine doc's own finding, not inherited assumption.

---

## 12. Sources & experiments

### External (URL · what it claims · accessed 2026-05-16)

- `datadome.co/changelog/vm-based-obfuscation/` · 2026-02-11 VM rollout,
  Device Check + Slider, three-layer, no public spec.
- `securityboulevard.com/2026/02/datadome-releases-vm-based-obfuscation…` ·
  syndication of the above; "build a disassembler from scratch."
- `github.com/glizzykingdreko/datadome-encryption` (+ `/blob/main/README.md`) ·
  clean-room §5 encoder; "virtually unchanged since first release";
  payload-encryption-only; 40★. `gh api`: pushed **2025-12-14**.
- `github.com/xKiian/datadome-vm` · VM disassembler PoC; `gh api`: 107★,
  pushed **2026-02-16**, not archived.
- `github.com/glizzykingdreko/Datadome-Interstitial-Deobfuscator` ·
  `gh api`: 34★, pushed **2024-01-12** ⇒ stale vs 2026 VM.
- `medium.com/@glizzykingdreko/breaking-down-datadome-captcha-waf-d7b68cef3e21` ·
  i.js→iframe, boring_challenge (seed 10–20 M, CPU-core hint, CPU-tax),
  31 movement features, daily 6-char keys, dual-XOR; "movement
  validation is the hardest to reproduce."
- `scrapfly.io/blog/posts/how-to-bypass-datadome-anti-scraping` (2026) ·
  JA3/IP-tier/HTTP2; nodriver ~25% baseline; "no universal bypass."
- `zenrows.com/blog/datadome-bypass` (2026) · daily 6-char key rotation
  confirmed; i.js/c.js; WASM CPU fingerprint.
- `eldorar.com/post/how-to-bypass-datadome-in-2026-…` · JA4/HTTP3/geo-
  coherence + VM claims — assessed **promotional/unverified** (Surfsky
  99.4% has no third-party data).
- `docs.hypersolutions.co/datadome/…`, Capsolver/CapMonster/2Captcha/
  TakionAPI docs · endpoint/cookie-shape reference only (B2B, closed).

### Local (read static + grep — no cargo run)

- Read in full: `00_INVENTORY_AND_METHOD.md`,
  `crates/browser/src/datadome_handler.rs`,
  `crates/akamai/src/datadome_crypto.rs`,
  `docs/research_2026_05_14/18_DATADOME_ENCRYPTION_REFERENCE_2026_05_15.md`,
  `docs/LEBONCOIN_ANDROID_DATADOME_2026_05_12.md`; key spans of
  `crates/browser/src/page.rs` (115-293, 1230-1450, 1700-2018,
  2110-2300, 3000-3176), `docs/research_2026_05_14/03_DATADOME.md`
  (§1-9, §14-18), `docs/research_2026_05_16/00_MASTER_PLAN.md` (§1.6,
  §2, §8.5), `crates/browser/tests/holistic_sweep.rs::classify`,
  `crates/browser/src/iframe.rs:255-287`,
  `crates/js_runtime/src/js/dom_bootstrap.js` (iframe handling spans).
- Dead-code grep (the decisive FP-5 proof):
  `rg -n "DdEncryptor|new_interstitial|main_prng_seed|cid_prng_seed|DdPrng|encode_payload" --type rust -g '!*/datadome_crypto.rs'`
  → only `crates/akamai/src/datadome_crypto.rs:11` (a doc comment).
  `rg -rn "datadome_crypto" … -g '!datadome_crypto.rs'` →
  only `crates/akamai/src/lib.rs:56 pub mod datadome_crypto;`.
  **Zero non-test callers — confirmed DEAD CODE.**
- `datadome_handler` callers grep → `page.rs:1247,1258,1367,1421,
  1756-1757,1859-1862,2238` (detect/plan/csp/cookie/trace only — no
  solve).
- `find_iframes` callers grep → exactly `page.rs:857,887,3137` (all
  static-DOM, build-time; **no post-JS re-scan**).
- Repo freshness via `gh api` / `gh repo view` (dates above).
- **No cargo executed** (build-lock / network `#[ignore]` per
  constraints); the byte-parity test `datadome_crypto.rs:371` was *read*,
  not run — it is a pre-existing, already-green proof of encoder
  correctness, orthogonal to the dead-code finding.
