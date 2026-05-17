# UNBLOCK — DataDome (etsy · tripadvisor · yelp)

**Created:** 2026-05-16 · **Baseline git HEAD:** `fd98bfa` (rescan infra
`d63b4fc`) · **Agent:** DataDome unblock-research · **Scope:** concrete
ordered build plan to pass the 3 DataDome-blocked sites.

**Provenance tags:** `[MECH]` = externally-cited mechanism (URL+date) ·
`[CODE]` = our code, file:line I read this session · `[HYP]` =
hypothesis + the experiment that would settle it.

**Companion docs (do not duplicate, this is the action layer):**
`docs/research/engines/datadome.md` (the authoritative analysis — §9
gaps, §11 plan), `99_CODE_FALSE_POSITIVES.md` (FP-E1/FP-D3),
master plan §8.5. **leboncoin (desktop) already PASSES** — it is the
reference for "what a working DataDome path looks like": a silent 200,
no interstitial, fresh `datadome=` cookie, no iframe, no WASM. Our goal
for etsy/tripadvisor is to reach leboncoin's state.

---

## 0. TL;DR — the answer to the three questions

1. **Is there a cheap silent-pass for etsy/tripadvisor that avoids the
   WASM-iframe entirely?** **Partially, and it is the highest-ROI first
   move — but it is *probabilistic, not deterministic*, and NOT
   network-free-verifiable.** etsy's own `x-datadome-riskscore=0.367` on
   the 403 `[MECH]` (datadome.md §4, master-plan-cited live capture) is
   *below 0.5* ⇒ the rejection is **fingerprint/behaviour-driven, not
   IP** ⇒ closing the silent-pass gap removes the interstitial trigger
   entirely (the deep probes only exist *after* you fail silent scoring
   — datadome.md §0/§2). BUT every credible 2026 source converges on:
   **fingerprint spoofing alone is insufficient; behaviour carries equal
   weight** (Scrapfly 2026: nodriver "~25% baseline without proxies",
   "no universal bypass"). So the silent-pass is real and worth the M
   investment, but it buys a *probability lift on a per-tenant ML
   threshold*, not a guaranteed flip — and the only valid verifier is a
   live DataDome oracle, not the §4 gate.

2. **Or do we need the full iframe subsystem?** **Yes, as the
   guaranteed-but-L fallback** — and it is *two* structural gaps, not
   one: (a) FP-E1 — script-created iframes never become real child
   contexts (`createElement('iframe')`/`.src` arena-interception
   missing); (b) **a second, independently-fatal gap found this
   session**: even when a `ChildIframe` *is* built, it is a fully
   isolated runtime with **no postMessage bridge back to the parent's
   V8** `[CODE]` `iframe.rs:22-70` — so the `rt:'i'` success step
   (iframe → `postMessage` → parent writes `datadome=`) physically
   cannot complete even with FP-E1 fixed. Both must land for the real
   chain.

3. **yelp verdict?** **Out of pure-stealth scope — confirmed, not
   assumed.** nocdp real CDP-free Chrome from this exact IP renders the
   yelp homepage title AND surfaces DataDome's **interactive
   "solve-this-task" captcha** (observed on screen) `[MECH]`
   (memory/state_2026_05_15_playwright_ab_decisive §"yelp RESOLVED",
   master plan §1.6, 2026-05-16). A banned IP returns a hard 403; a
   solvable interactive challenge does not. yelp is a **human-gate**
   (`rt:'c'`-class), same bucket as medium/quora/spotify/substack/
   duolingo. **Do zero engine work on yelp**; the only fix is
   label-truth (it is not an engine gap and not an IP ban).

**Net:** the corpus is now 1 PASS (leboncoin desktop) + 1 out-of-scope
(yelp) + 2 real targets sharing one irreducible blocker (etsy +
tripadvisor, identical `rt:'i'` mechanism). The ordered plan is below.

---

## 1. The 2026 mechanism — what we must defeat (verified this session)

`[MECH]` Sources accessed **2026-05-16** (full list §6). Cross-checked
against our `datadome.md` §1–5; only the *new/confirmed* deltas here.

- **The `rt:'i'` chain (etsy/tripadvisor)** `[MECH]` glizzykingdreko
  Medium (2026-05-16): the 403 body carries the `dd={…}` dict (`rt`,
  `cid`, `hsh`, `b`, `s`, `e`, `host:'geo.captcha-delivery.com'`,
  `cookie`); `i.js` *"generates the actual challenge URL and injects it
  as an iframe on the page"* → the iframe loads its own bundle → runs
  the **WASM `boring_challenge`**: *"pick a random 32-bit seed (between
  10 million and 20 million)"* + a **CPU-core-count concurrency hint** +
  *"a massive, nested loop of bit-twiddling, XORs, shifts, rotates and
  magic constants"* — explicitly a **"CPU tax" targeting headless
  browsers, NOT a fingerprint** → result + signals encrypted to
  `ddCaptchaEncodedPayload`, GET to captcha-delivery.com → **a
  `datadome` cookie is returned on success**. Confirms datadome.md §3
  byte-for-byte.
- **Daily 6-char key rotation** `[MECH]` ZenRows / Capsolver
  (2026-05-16): *"every day the keys in the signals dictionary change to
  a random six-character string"* — the **moving target**. The §5
  encoder (our byte-verified `DdEncryptor`) is *static*; the daily
  *wire-key dictionary* and the server `e`/HMAC + JA4 cross-check are
  not. Any offline poster must re-extract the dict daily.
- **`t` parameter** `[MECH]` Capsolver / recaptchaUser README
  (2026-05-16): in the challenge URL, `t='it'`=interstitial,
  `t='fe'`=captcha (human), **`t='bv'`=IP-ban**. (etsy/tripadvisor =
  `rt:'i'` invisible; yelp's nocdp shows the *interactive* class.)
- **Paid-solver requirements (the real-world constraint set)** `[MECH]`
  TakionAPI / Capsolver docs (2026-05-16): a solver needs **matching
  TLS to the exact Chrome version + matching header order**, the **same
  proxy/IP for solving and for the page** (the `datadome` cookie is
  **IP-bound**), and posts the challenge HTML to a *server* (no
  clean-room offline solver shipped). This corroborates datadome.md §5.5
  "no offline minting; every fresh session must hit DataDome once".
- **OSS landscape — re-verified via `gh api` 2026-05-16 (unchanged from
  datadome.md §7, no new working solver):**
  - `glizzykingdreko/datadome-encryption` — 40★, pushed
    **2025-12-14**, not archived. The §5 *encoder*, static. We
    byte-verified our port (`datadome_crypto.rs:371`). **Encoder only;
    author README self-states "incomplete for real-world deployment".**
  - `xKiian/datadome-vm` — 107★, pushed **2026-02-16**, not archived.
    WIP **disassembler PoC** (~3 of 16-bit opcodes). **Not a solver.**
  - `glizzykingdreko/Datadome-Interstitial-Deobfuscator` — 34★, pushed
    **2024-01-12** ⇒ **stale**, predates the 2026-02 VM rollout.
  - `glizzykingdreko/Datadome-Deobfuscator` — 39★, pushed **2023-10-07**
    (tags.js only; tags.js has no VM so still applicable for the
    *silent-path* signal-map extraction).
  - `recaptchaUser/datadome-Interstitial-solver` — **1★, pushed
    2023-11-21, DEAD** — a thin Capsolver-API wrapper, not a solver.
  - **Net:** the silent-path encoder is a solved *static* problem; the
    post-2026-02 VM-protected `i.js` has **no public working offline
    solver**, only a partial disassembler — exactly datadome.md §7's
    "win on the silent path / run the real vendor JS in a real browser
    context; do not expect an offline interstitial solver to work or
    stay working." **No source contradicts this; multiple corroborate.**
- **Silent-200 vs interstitial decision** `[MECH]` Scrapfly 2026 (the
  most skeptical, least promotional source): a **multi-layer trust
  score** — TLS JA3/JA4, HTTP/2-vs-1.1, IP ASN tier
  (datacenter=negative, residential/mobile=positive), JS fingerprint,
  **behaviour weighted as heavily as fingerprint** — feeds the
  per-tenant ML; **"browser fingerprint spoofing alone is not enough."**
  No source gives an exact threshold or signal weight (DataDome does not
  publish it; the vendor "engine-level guide" blogs are
  promotional/empty — eldorar, startertutorials confirmed content-free
  this session). ⇒ The silent-pass is **a probability lift, not a
  formula** — important for the verification regime (§5).

---

## 2. Our code reality — the precise blockers (file:line, this session)

`[CODE]` Read this session: `datadome_handler.rs` (full),
`page.rs:{855-997,1820-1990,2300-2410,3270-3305}`, `iframe.rs:1-130 +
255-287`, `dom_bootstrap.js:{487-512,1367-1400,1915-1980,2034-2070}`,
`dom_ext.rs:345-389`, `iframe_isolation.rs:110-180`.

**B1 — FP-E1 root-caused precisely (script iframe never a child
context).** `op_dom_create_element('iframe')` (`dom_ext.rs:347`) +
`op_dom_append_child` (`dom_ext.rs:367`) **do** insert a real arena-DOM
node, and `find_iframes` walks the arena DOM from `NodeId::DOCUMENT`
(`iframe.rs:255-287`) — so in principle a rescan *should* see it. The
committed FP-E1 decisive experiment
(`iframe_isolation.rs:144`, `#[ignore]`d) nonetheless returns **0**.
`rematerialize_iframes` (`page.rs:940-997`) is correct, gated, idempotent
infra — but **necessary, not sufficient**: the JS `appendChild` wrapper
(`dom_bootstrap.js:1924`) registers the iframe in the JS-side
`_appendedIframes` array + synthetic-window registry for fingerprint
parity; the path that makes a script-created iframe a *real,
find_iframes-discoverable, fetching/executing child* is missing —
this is the **`createElement('iframe')`/`.src` arena-interception
subsystem** (datadome.md §9 G1, the named "single highest-leverage
engine investment").

**B2 — NEW this session, independently fatal: no parent↔child
postMessage bridge.** `iframe.rs:4` *claims* "Communication between
parent and child is via serialized postMessage" — but
`ChildIframe::{from_url,from_srcdoc}` (`iframe.rs:29-145`) build a
**fresh isolated `BrowserJsRuntime` with its own DOM + event loop and
zero bridge**. Grep `postMessage|parent|bridge` in `iframe.rs` → only
the doc comment + a CSP comment; **no code**. The only postMessage
machinery is the synthetic-shim `_getIframeWindow` in-realm stub
(`dom_bootstrap.js:2692-2698,2846-2848`) that `dispatchEvent`s to the
**same** context's `globalThis`, not across runtimes. ⇒ Even if B1 is
fixed and the child runs the WASM and computes the cookie, the
`rt:'i'` success step **iframe → `parent.postMessage(result)` → parent
writes `document.cookie='datadome=…'` → reload** has **no channel to
cross**. The real chain needs *both* B1 and B2.

**B3 — `DdSolvePlan` computed then discarded** `[CODE]`
`page.rs:1333-1344` (`eprintln!` only; the CSP-exemption that *does*
fire is keyed independently off `is_datadome_challenge_doc`
`page.rs:1451`). Decorative. datadome.md §9 G4.

**B4 — what already works (do not rebuild):** interstitial
detector/parser (`datadome_handler.rs:67`, exercised); CSP-exempt
challenge doc so `i.js` loads (`page.rs:1451`, live-verified 200/15 KB);
`started_as_dd_challenge` routing into the 90 s poll
(`page.rs:1827-1885`) which now pumps `rematerialize_iframes` + breaks
on `datadome_solved` (FP-D3 fixed — `datadome_handler.rs:239`, no longer
false-passes on the 403's own cookie). The detect/CSP/poll scaffold is
sound; the gap is purely B1+B2.

**B5 — `DdEncryptor` is DEAD/insurance** `[CODE]`
`datadome_crypto.rs` (0 non-test callers, datadome.md §10b FP-5,
99-doc Class A DONE). Correctly *not* wired — wiring it needs the daily
wire-key dict + signal map + server `e`/JA4 we do not have. Keep as
labelled insurance; **do not** make wiring it part of any plan.

---

## 3. THE ORDERED BUILD PLAN

Three tracks. **Do Track A first (cheapest, lowest-risk,
cross-vendor).** Track B is the guaranteed-but-L fallback only if A is
insufficient. Track C is label-only.

### TRACK A — Silent-pass / fingerprint+behaviour coherence (avoid the interstitial entirely)

*Rationale:* etsy `riskscore=0.367` ⇒ fingerprint/behaviour-driven, not
IP `[MECH]`; the deep probes only exist *after* silent failure
(datadome.md §0). This is the doc-03/18-sanctioned intent and is
**cross-vendor** (also lifts Akamai/Kasada behavioural). Honest caveat:
buys a probability lift on a per-tenant ML threshold, not a guaranteed
flip; only a live oracle verifies the flip (§5).

| # | Action | Where (concrete) | Diff | Risk | Verifies |
|---|---|---|---|---|---|
| **A1** | **Behavioural mouse-path emitter into the nav→onload window** so DataDome's `addEventListener('mousemove')` populates a non-empty `_initialCoordsList` (datadome.md §2/§4 — *"empty `_initialCoordsList` is a **stronger** kill than an imperfect one"*; glizzykingdreko: 31 movement features over this list). | Wire `crates/stealth/src/behavior.rs` (Catmull-Rom/Bézier, ~60 Hz jittered, sigma-lognormal already present per master-plan G8 note) into `Page::navigate`'s nav→onload window; dispatch synthetic `mousemove`/`pointermove` before `run_until_idle` drains. | **M** | Low (additive; gate-neutral — no benign-nav behaviour change beyond emitting events nothing reads offline) | §4-gate verifies the *emitter* (deterministic ChaCha path parity test); the **flip** needs live oracle |
| **A2** | **Sec-CH-UA-* retry coherence.** DataDome requests `accept-ch` *on the 403 itself* (datadome.md §2); the retry after a 403 must carry **all 7** `Sec-CH-UA-*` hints, consistent with TLS JA4 + UA. | Audit `crates/stealth/src/presets.rs` + the net retry path; assert the post-403 re-request emits the full client-hint set. | **S** | Low | §4-gate verifiable (header-set parity assert) |
| **A3** | **The 4 coherence shims** (datadome.md §4 "HIGH risk if disagree"): Worker `userAgentData` ≠ "NA"; Worker-vs-main WebGL vendor/renderer identical; `getTimezoneOffset` ↔ `Intl…timeZone`; `mob` ↔ UA ↔ `Sec-CH-UA-Mobile`. Prior W6a flagged Worker `userAgentData`="NA" (datadome.md §4) — **verify current state first**. | `dom_bootstrap.js` Worker bootstrap + `interfaces_bootstrap.js`; add asserting parity tests. | **S–M** | Low | **§4-gate verifiable** (network-free parity tests — the strongest part of Track A) |
| **A4** | **typeof-mask byte-parity regression** vs a captured real Chrome 148 32-bit bitmask (datadome.md §4 — 600+ entries shipped, no asserting test). | `interfaces_bootstrap.js` + new `chrome_compat` assert. | **S** | None | §4-gate verifiable |

**A-exit criterion:** run the holistic live re-measure (the only valid
verifier) against etsy+tripadvisor. If still `rt:'i'` after A1–A4 →
proceed to Track B. If silent-200 → **done** (matches leboncoin's path;
no iframe/WASM ever needed). `[HYP]` etsy at 0.367 is *close to* the
silent boundary ⇒ A has a credible chance; the discriminating
experiment is the live re-measure, which **requires an explicitly
authorized live-oracle dev loop** (§5).

### TRACK B — The real `rt:'i'` chain (guaranteed-but-L; only if A insufficient)

*This is two structural engine builds (B1+B2 from §2), both required.
This is the `99_…` FP-E1 "single highest-leverage engine investment"
plus a newly-identified second gap. Scope it as a dedicated subsystem
project, not a same-class commit.*

| # | Action | Where (concrete, scoped against our code) | Diff | Risk |
|---|---|---|---|---|
| **B1** | **`createElement('iframe')`/`.src` arena-interception** so a script-created iframe is a real, `find_iframes`-discoverable, fetching/executing child. Today `createElement` only special-cases `script` (`dom_bootstrap.js:1367-1390`); extend with an `iframe` branch that, on `.src`/`setAttribute('src')` set to a cross-origin URL, triggers `rematerialize_iframes` (already correct+gated infra, `page.rs:940`) for that node — OR have the appendChild wrapper (`dom_bootstrap.js:1924`) signal the Rust side. Root-caused: the arena node *exists*; the missing piece is the post-JS trigger that drives `ChildIframe::from_url` for it. | `dom_bootstrap.js` createElement/appendChild iframe branch + a Rust op to request rematerialization; reuse `rematerialize_iframes`. | **L** | High — touches DOM-binding + build/iframe arch; must stay gated behind `started_as_dd/cf/seccpt` so §4 iframe-isolation tests (5/0) don't regress |
| **B2** | **Parent↔child postMessage bridge** (NEW, independently fatal). `ChildIframe` is an isolated runtime with no bridge (`iframe.rs:22-145`). Add a serialized message channel: child `parent.postMessage(x)` → host marshals → parent `window` `MessageEvent`; and `window.parent`/`frameElement` in the child resolve across runtimes. Without this the iframe's solved-cookie `postMessage` to the parent (the `rt:'i'` success step) cannot cross. | New bridge in `iframe.rs` + `dom_bootstrap.js` `_getIframeWindow`/postMessage stubs (currently same-context only, `2692-2698`). | **L** | High — cross-runtime serialization; risk of realm-identity leaks (Kasada-class) if the bridged objects aren't native to the child realm |
| **B3** | **Run the VM-protected bundle in the child** on our native V8 WASM (datadome.md §9 G2 notes `WebAssembly.*` functional) + Chrome-faithful surface; capture the posted `datadome=` into the shared jar; let the existing cookie-diff retry re-issue the URL — gated by the **already-fixed** `datadome_solved` (FP-D3, requires cookie **AND** body no longer a challenge doc, not bare cookie). | depends on B1+B2; then a **live-oracle dev loop**. | **L** | High + **NOT offline-verifiable** — daily key + server `e`/HMAC + JA4 are an external live oracle |
| **B4** | Consume (not log) `DdSolvePlan` (drive host-allow+renav from it, B3-§2); remove the `captcha-delivery.com` exclusion in `v8_html_is_real` (`page.rs`, datadome.md §11) so a genuinely-rendered post-solve body is accepted. | `page.rs` | **S** | Low (only reachable once B1–B3 land) |

**Honest B-verdict:** even fully built, B3 cannot be *verified* offline —
the flip is gated by a live daily-rotating DataDome oracle + server-side
JA4. This is the irreducible structural ceiling (datadome.md §11,
master plan §8.5). Track B *guarantees the capability exists*; it does
not guarantee a §4-green flip — only the live-oracle regime can show
that.

### TRACK C — yelp (label-only, zero engine work)

yelp is **DataDome interactive-captcha / human-gate class** (`rt:'c'`-
type), proven by nocdp real Chrome from this IP getting a *solvable
interactive task*, not a hard 403 `[MECH]` (memory state_2026_05_15
playwright_ab_decisive §"yelp RESOLVED", master plan §1.6, 2026-05-16).
**Action:** ensure every doc/metric classifies yelp as
human-interaction-gate (same bucket as medium/quora/spotify/substack/
duolingo), **not** DataDome-engine-gap and **not** IP-ban. Counting
yelp as a DataDome engine failure is a scope error. No code. (datadome.md
§9/§11 already states this; this doc reaffirms with the nocdp evidence.)

---

## 4. Difficulty / risk / what-verifies summary

| Track | Item | Diff | Risk | Offline §4-verifiable? | Only live-oracle verifies? |
|---|---|---|---|---|---|
| A | A1 mouse emitter | M | Low | Emitter parity only | The flip: **yes** |
| A | A2 Sec-CH-UA retry | S | Low | **Yes** | — |
| A | A3 coherence shims | S–M | Low | **Yes** (strongest part) | — |
| A | A4 typeof-mask | S | None | **Yes** | — |
| B | B1 iframe arena-intercept | L | High | Network-free srcdoc proof (FP-E1 test, un-ignore) | cross-origin path: **yes** |
| B | B2 postMessage bridge | L | High | srcdoc round-trip test | — |
| B | B3 VM/WASM in child | L | High | **No** | **Yes** (daily key+JA4) |
| B | B4 plan-consume / v8_html_is_real | S | Low | Yes | — |
| C | yelp label | — | None | n/a (doc) | n/a |

**The structural ceiling (state it plainly):** the network-free §4 gate
**cannot** verify the etsy/tripadvisor *flip* under either track — Track
A's flip is a per-tenant ML probability move, Track B's is a
daily-key/JA4 live oracle. The §4 gate verifies the *hygiene* (A2/A3/A4,
B1-srcdoc, B2-srcdoc); the *flip* needs an explicitly-authorized
live-oracle dev loop with a captured daily challenge as fixture
(datadome.md §11, master plan §8.5). No fabricated guarantee.

---

## 5. Verification regime — and where only a live daily-key oracle works

- **§4-gate-green (do now, zero live dependency):** A2 (header-set
  parity), A3 (4 coherence parity tests — the highest-confidence
  network-free deliverable in Track A), A4 (typeof-mask bitmask
  assert), B1's srcdoc half (un-ignore
  `iframe_isolation::fp_e1_post_js_injected_iframe_is_materialized`
  once the arena-interception lands — its `#[ignore]` reason becomes
  obsolete), B2's srcdoc round-trip.
- **Live-oracle-only (must be explicitly authorized):** the
  etsy/tripadvisor *flip* itself, under **both** tracks. Use the
  directive's `holistic_sweep` live re-measure with a captured daily
  `geo.captcha-delivery.com` challenge as the fixture/oracle. Track A's
  exit criterion (silent-200 vs still-`rt:'i'`) and Track B's B3
  (cookie accepted) are **only** observable here. This is identical to
  the master plan §8.5 Phase-5 conclusion — restated as this doc's own
  finding, not inherited.
- **Never** treat a bare `datadome=` cookie as success (FP-D3, already
  fixed: `datadome_solved` requires cookie **AND** non-challenge body).
  Re-confirm any future success branch uses `datadome_solved`, not
  `cookies_have_datadome`.

---

## 6. Sources (URL · claim · accessed 2026-05-16)

**External (web):**
- `medium.com/@glizzykingdreko/breaking-down-datadome-captcha-waf-d7b68cef3e21`
  · `i.js`→injected iframe; `boring_challenge` WASM (32-bit seed
  10–20 M, CPU-core hint, nested XOR/shift/rotate "CPU tax targeting
  headless", NOT a fingerprint); daily 6-char signal-key rotation;
  `_initialCoordsList` + 31 movement features; `ddCaptchaEncodedPayload`
  GET → `datadome` cookie on success. **Confirms datadome.md §3.**
- `scrapfly.io/blog/posts/how-to-bypass-datadome-anti-scraping` (2026,
  fetched via 403 on direct WebFetch — content via WebSearch summary +
  prior datadome.md capture) · multi-layer trust score; **"browser
  fingerprint spoofing alone is not enough; behaviour weighted as
  heavily as fingerprint"**; nodriver **~25% baseline without
  proxies**; **"no universal bypass."** The skeptical anchor for §0(1).
- `zenrows.com/blog/datadome-bypass` (2026) · daily 6-char key rotation
  confirmed; WASM CPU fingerprint; i.js/c.js; x-dd-b a 403 response
  header.
- `docs.takionapi.tech/datadome/examples` + Capsolver/CapMonster docs ·
  solver needs **TLS matching exact Chrome version + header order**,
  **same proxy/IP for solve and page** (cookie **IP-bound**), challenge
  HTML posted to a *server*; `t='it'`/`'fe'`/`'bv'` semantics. **No
  clean-room offline solver — corroborates datadome.md §5.5/§7.**
- `github.com/recaptchaUser/datadome-Interstitial-solver` (gh api:
  **1★, 2023-11-21, dead** — Capsolver wrapper, not a solver).
- `eldorar.com/...senior-scraper-s-guide`,
  `startertutorials.com/...ultimate-engine-level-guide` ·
  **assessed promotional/content-free this session** (no reproducible
  mechanism; Surfsky product placement). Cited only to record they were
  checked and rejected.
- Repo freshness via `gh api` (2026-05-16):
  `glizzykingdreko/datadome-encryption` 40★ 2025-12-14;
  `xKiian/datadome-vm` 107★ 2026-02-16 (WIP disasm, not a solver);
  `…/Datadome-Interstitial-Deobfuscator` 34★ 2024-01-12 (stale);
  `…/Datadome-Deobfuscator` 39★ 2023-10-07 (tags.js only). **No new
  working offline solver since datadome.md was written.**

**Local (read static, no cargo):**
`docs/research/engines/datadome.md` (full), `99_CODE_FALSE_POSITIVES.md`
(full), `README.md`, `00_MASTER_PLAN.md` §1.6/§8.5;
`crates/browser/src/datadome_handler.rs` (full),
`crates/akamai/src/datadome_crypto.rs` (header/FP-5 region),
`crates/browser/src/page.rs:{855-997,1820-1990,2300-2410,3270-3305}`,
`crates/browser/src/iframe.rs:{1-130,255-287}`,
`crates/js_runtime/src/js/dom_bootstrap.js:{487-512,1367-1400,
1915-1980,2034-2070,2692-2698,2846-2848}`,
`crates/js_runtime/src/extensions/dom_ext.rs:345-389`,
`crates/browser/tests/iframe_isolation.rs:110-180`,
`memory/state_2026_05_15_playwright_ab_decisive.md` (yelp RESOLVED).
**No cargo executed** (per constraints).

---

## 7. Top-3 concrete next actions (the unambiguous move)

1. **Track A3 + A2 + A4 first** (S–M, low-risk, §4-gate-verifiable,
   cross-vendor): land the 4 coherence shims + Sec-CH-UA-retry +
   typeof-mask parity tests. These are pure fingerprint hygiene, lift
   Akamai/Kasada too, and are the only Track-A parts the offline gate
   can prove green.
2. **Track A1** (M): wire `stealth::behavior.rs` into the nav→onload
   window so `_initialCoordsList` is non-empty — datadome.md's "decisive
   silent-pass gap" and the single behavioural lever; then run the
   authorized live-oracle re-measure on etsy+tripadvisor (the A-exit
   criterion). This is the highest-ROI shot at avoiding the WASM-iframe
   entirely.
3. **Only if A insufficient: scope Track B as a subsystem project** —
   B1 (createElement/`.src` arena-interception) **and** B2 (parent↔child
   postMessage bridge) are *both* required and *both* L; B2 was missed
   by prior analyses and is independently fatal. Un-ignore the FP-E1
   srcdoc test as the network-free milestone; the cross-origin flip
   stays live-oracle-only.

**yelp:** no engine work — confirmed human-interaction captcha
(`rt:'c'`-class) by nocdp real Chrome from this IP; fix the label only.
