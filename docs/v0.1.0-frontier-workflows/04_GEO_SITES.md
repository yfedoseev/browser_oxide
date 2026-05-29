# 04 — GEO / IN-HOUSE FRONTIER: wildberries.ru + ozon.ru

**Scope of this doc:** the two Russian-market frontier sites —
`wildberries.ru` (custom in-house "wbaas" antibot, HTTP 498 +
`Почти готово…` shell) and `ozon.ru` (DDoS-Guard, thin ~156 B body).
**Mission framing:** challenge "out of scope / IP-geo only" and find the
engine-addressable lever where one exists — but stay evidence-based.

**Headline verdict (honest):**

| Site | Block class | No-CDP real browser passes? | Engine-addressable lever | Verdict |
|------|-------------|------------------------------|--------------------------|---------|
| **wildberries** | IP-trust-**adaptive** PoW + JS device scoring (custom wbaas) | **YES, partially** — a *residential* RU/non-DC IP loads it WITHOUT VPN; a US DC IP gets a hard challenge | **YES** — the live-nav async-drain so the wbaas self-solve completes (shared with AWS lever). PoW math itself = `vendor_solvers`. | **MIXED: ENGINE-ADDRESSABLE for the solve-chain + body growth; the *trust ceiling* is IP-reputation-bound** |
| **ozon** | DDoS-Guard JS+cookie gate **layered on a hard foreign-IP geo-gate** | **NO from a foreign/DC IP** — "Ozon won't load without a Russian IP from abroad" | minimal — the `__ddg` cookie/JS hop is small but the geo-gate dominates | **IP-GEO-BOUND** (RU residential IP required; engine work cannot substitute for it) |

The central no-CDP thesis **does** apply here, but with a critical
caveat the user's hypothesis must absorb: **wbaas/DDoS-Guard are not
primarily CDP-sniffers** (unlike Kasada/DataDome). The dominant signal
is **IP reputation + geo**, with JS fingerprint/PoW as a secondary,
*IP-trust-scaled* layer. BO's no-CDP advantage helps it look like a real
user *once the IP is acceptable*, but it cannot manufacture a trusted RU
residential IP. So this cluster is the one place where "partly IP-geo"
is the honest answer — quantified per site below.

---

## A. Evidence base (captured + external, cited)

### A.1 The wildberries capture (`/tmp/awswaf/wb.html`, 1.4 KB)
```html
<html data-theme="light"
      data-req-uuid="1b5afd23c72fae0be032065b3994d2ff"
      data-req-ip="2001:569:728c:f600:216:3eff:feef:8cf3">   <!-- the gate's view of US -->
  <meta http-equiv="refresh" content="60">                    <!-- 60s self-reload -->
  <title>Почти готово...</title>                              <!-- "Almost ready" -->
  <script type="module" crossorigin
          src="/__wbaas/challenges/antibot/__static/v1/index-DQJ0L4Mq.js"></script>
  <link rel="stylesheet" crossorigin href="/__wbaas/.../index-BuoI5IWB.css">
  ...
  <div id="wait_msg"></div><div id="c_cont"></div>            <!-- mount points -->
  <b id="s-key" data-site-key="7400bd5df8b843b28254659f10915f31"></b>
</body>
```

**The `data-req-ip` is decisive.** `2001:569:728c:f600:216:3eff:feef:8cf3`:
- `2001:569::/32` is an **ARIN / Comcast (US)** allocation
  ([IPQS AS33491](https://www.ipqualityscore.com/asn-details/AS33491/comcast-cable-communications-llc),
  [ARIN](https://www.arin.net/resources/)). **Not Russian.**
- The interface ID `…:0216:3eff:feef:8cf3` is a textbook **EUI-64**
  (`02:16:3e:ff:fe:ef:8c:f3` → MAC `00:16:3e:ef:8c:f3`, the Xen/QEMU
  `00:16:3e` OUI). So the box exposed a **SLAAC IPv6 derived from a VM
  MAC** and wildberries echoed it back. This is a **datacenter VM on a
  US Comcast IPv6** — exactly the profile wbaas scores as low-trust.

So the 498 in our capture is being served to a *visibly* US datacenter
IPv6. That is the single most important fact in this whole document.

### A.2 wildberries IS reachable from abroad — without VPN (external, load-bearing)
[The Moscow Times, 2026-04-15](https://www.themoscowtimes.com/2026/04/15/russian-websites-begin-blocking-vpn-users-as-internet-controls-tighten-a92511)
and [Meduza, 2026-04-30](https://meduza.io/en/feature/2026/04/30/russia-blocks-vpn-access-to-major-platforms-moves-to-charge-for-mobile-vpn-traffic),
plus the Russian-press coverage
([overclockers.ru](https://overclockers.ru/blog/kosmos_news/show/253042/),
[rtvi.com](https://rtvi.com/news/wildberries-i-ozon-nachali-blokirovat-dostup-pokupatelyam-s-vpn/),
[hi-tech.mail.ru](https://hi-tech.mail.ru/news/146844-ozon-i-wildberries-nachali-puskat-polzovatelej-s-vpn-posle-snizheniya-prodazh/))
establish:
- **"Wildberries opens outside the country without VPN."** A *foreign
  residential* IP loads the store; a **VPN / datacenter IP** triggers
  throttling — "product listings, images, descriptions fail to load."
- The marketplaces **detect VPN/DC traffic by IP reputation + traffic
  pattern** and *throttle* (not hard-403). That throttle is exactly the
  high-difficulty branch of the adaptive PoW (A.4).
- **Ozon, by contrast, "would not load without a VPN" from abroad** —
  i.e. ozon enforces a *harder* foreign-IP gate than wildberries.

This is the no-CDP-oracle answer in disguise: a **real Chrome on a
foreign *residential* IP passes wildberries**. So wildberries' wall is
**not** "real browsers can't pass" — it is "our *IP* is wrong (US DC),
which raises the challenge difficulty to a level the engine then has to
actually solve." Both halves are addressable, but only one of them is
engine work.

### A.3 The wbaas bundle — already deobfuscated (cited, not re-done here)
`docs/v0.1.0-parity-workflows/sites/SITE_wildberries.md` §3 contains a
full deobfuscation of `index-DQJ0L4Mq.js` (70 KB). Key verified facts
re-used here:
- It is `type="module"` **but has `import ×0 / export ×0`** — it never
  uses static ESM syntax (SITE_wildberries §3.2).
- **`WebAssembly ×0, new Worker ×0, new Blob ×0, crypto/subtle/digest
  ×0`** — no WASM/worker/blob wall (contradicts the older
  `vNext/04_R-WBAAS-WILDBERRIES.md` assumption).
- Flow: `DOMContentLoaded` → cookie-test → `me {tries}` localStorage gate
  (≤3 tries / 60 s) → `POST /api/v1/create-token` → **498 with
  `{challenge}` JSON** → load solver via `document.createElement('script')`
  (NOT `import()`) → `solve()` (main-thread PoW) → retry create-token →
  set `x_wbaas_token` cookie → `location.reload()`.
- Signals POSTed: `ua, browser/os/engine/platform, viewport, screen,
  timeZone, onLine, cookieEnabled` → server scores → **adaptive PoW
  difficulty** (Habr write-up,
  [habr.com/.../1032556](https://habr.com/ru/companies/wildberries/articles/1032556/)).

### A.4 The adaptive-difficulty mechanism (Wildberries' own write-up)
The Wildberries engineering blog ([Habr 1032556](https://habr.com/ru/companies/wildberries/articles/1032556/),
and the AntiDDoS meetup [Habr 1018490](https://habr.com/ru/companies/wildberries/articles/1018490/))
describe wbaas as: device-fingerprint **scoring** + **adaptive PoW**
("trusted clients get easy tasks, suspicious IPs get hard ones") + token
revocation + ongoing ML behaviour scoring. **The IP/trust score sets the
PoW difficulty.** A US DC IPv6 ⇒ high difficulty ⇒ a PoW loop heavy
enough that even if BO drains correctly it may blow the
`V8DeadlineWatcher` budget.

### A.5 ozon = DDoS-Guard + foreign-IP geo (cited)
- BO's net stack already knows ozon's protocol shape:
  `crates/net/src/lib.rs:979` — *"DDoS-Guard (ozon.ru) returns 307 on
  POST /abt/result, which requires re-POSTing the body to the new
  location."* So the cookie/JS hop (`__ddg*` cookies, `/abt/result`) is
  a known small protocol step.
- DDoS-Guard is a Russian CDN/anti-DDoS provider
  ([Wikipedia](https://en.wikipedia.org/wiki/DDoS-Guard);
  bypass attempts are TLS-fingerprint-based,
  [github.com/1337tr/DDos-Guard-Bypass](https://github.com/1337tr/DDos-Guard-Bypass)).
- But the dominant ozon fact from external evidence (A.2) is the
  **foreign-IP geo-gate**: ozon won't even render from abroad without a
  RU IP. BO's thin ~156 B body matches the "blocked before the JS gate"
  outcome. `docs/v0.1.0-parity-workflows/03_OUTPERFORM_V150_ANALYSIS.md:31`
  already files both ozon and wildberries under **"(geo)"** shared
  frontier (both engines fail).

---

## B. SITE 1 — wildberries.ru

### B.1 (1) Exact detection mechanism blocking BO
Two stacked gates:
1. **IP-trust gate (dominant).** wbaas scores the source IP. Our capture
   IP is a **US Comcast/Xen-VM IPv6** (A.1) → low trust → **(a)** the
   adaptive PoW difficulty is set high, and **(b)** the device-scoring
   profile is strict. This is server-side, IP-keyed; no engine signal
   changes it.
2. **Self-solve gate (engine-addressable).** Even at whatever difficulty
   we're given, BO must run the full async chain to completion inside the
   live nav path: `DOMContentLoaded → POST create-token (498) → inject
   solver <script> + poll → solve() PoW → retry create-token → set
   x_wbaas_token → location.reload()`. **This is the same live-nav
   async-drain class blocking AWS** (`docs/HANDOFF_2026_05_28b.md §4-5.1`).

BO today stays at the 1883 B / 498 shell because gate 2 doesn't complete
(and possibly because gate 1 hands us a PoW too heavy to finish in
budget — that fork is exactly what the §B.4 oracle resolves).

### B.2 (2) Does a no-CDP real browser pass it? → engine-addressable?
**Partially YES — and this is the strongest no-CDP argument in the
cluster.** External evidence (A.2) is explicit: **real Chrome on a
foreign *residential* IP loads wildberries WITHOUT VPN.** So:
- It is **not** "no engine passes" (Camoufox v150 = THIN-39, worse than
  BO's iphone 7900b — SITE_wildberries §2.3 / FAILED_SITES_ANALYSIS §C.4).
  Any forward progress here is a head-to-head **win vs v150**.
- The wall that remains for *our* IP is (a) IP reputation (NOT engine)
  and (b) the self-solve drain (ENGINE).

The no-CDP structural advantage is real but **secondary** here: wbaas is
not a CDP-sniffer the way Kasada is. BO's edge is that it can run the
self-solve like a real in-process browser (no Runtime.enable / juggler
residue to detect) — which matters once the IP is acceptable, and is why
BO can plausibly beat v150's THIN-39 even from a DC IP.

### B.3 (3) Concrete engine path (file:line) + how no-CDP helps

The good news: **BO already has every primitive.** Verified this pass:

- **ES-module support — the bundle DOES run.** `find_scripts`
  (`crates/browser/src/script_runner.rs:37-75`) collects
  `<script type="module" src=...>` as an external script (it only skips
  `application/ld+json` / templates; `module` is *not* skipped). The
  fetched code is evaluated by
  `BrowserJsRuntime::execute_script` (`crates/js_runtime/src/lib.rs:93-122`)
  via **`v8::Script::compile` — classic-script semantics, NOT a V8
  module instantiation.** ⚠️ **Load-bearing caveat:** a *true* ESM with
  `import`/`export` statements would **fail to compile** in BO (classic
  eval rejects top-level `import`/`export`). The wbaas bundle survives
  *only because* it has `import ×0 / export ×0` (A.3). So "BO executes
  the ES module" is true for wbaas but **not** a general ESM capability —
  see §B.6 future-proofing.
- **Dynamic solver `<script>` injection works.** The solver is loaded
  via `document.createElement('script')` + `r.src=…` + `appendChild`.
  BO fetches+evals on insertion in `_onNodeInsertedInner`
  (`crates/js_runtime/src/js/dom_bootstrap.js:142-234`), accepts
  `type === 'module'` (`:146`), and **dispatches `new Event('load')`
  after eval** (`:189-203, 218-219`) which satisfies the bundle's
  `r.addEventListener("load", …)`. Nested injection degrades to an async
  fetch+eval at `_MAX_SYNC_EVAL_DEPTH=4` (`:210-227`).
- **498-keep-iterating + reload/cookie-gain + in-V8 refetch** already
  exist: `crates/browser/src/page.rs:1179` and `:2994` classify 498 as
  challenge-shaped and keep iterating; `location.reload()` →
  `__pendingNavigation` → in-V8 refetch with `credentials:'include'`
  (page.rs reload path); the cookies-gained F5 signal flips solve-then-
  reload sites. The XHR/fetch instrumentation even names wbaas
  explicitly: `page.rs:3554` — *"critical for SDKs like WBAAS that use
  sync XHR for token fetches."*

**The actual gap is the drain budget between steps:**
- The **inter-script drain is only 50 ms** (`crates/browser/src/page.rs:3661`:
  `run_until_idle(Duration::from_millis(50))` between each prefetched
  script). The module executes, registers the `DOMContentLoaded`
  handler, returns. DOMContentLoaded then fires via `setTimeout(0)`
  *after* the script loop (`page.rs:3674-3684`).
- The whole token chain runs **inside** that DOMContentLoaded handler
  and must fully advance during the *final* per-iteration nav drain.
  That per-iter drain uses the full remaining budget floored at 8 s with
  a `V8DeadlineWatcher` (per SITE_wildberries §4.1) — adequate for I/O,
  but a high-difficulty PoW loop can exhaust it.

**Engine lever (public):** extend/guarantee the live-nav drain so the
multi-round-trip chain (find-settings → solver-inject → create-token×2 →
reload) advances — the *same* lever queued for AWS in
`HANDOFF_2026_05_28b §5.1`. wildberries is a near-free rider on that
work. Concretely: after the dynamic solver `<script>` evals, ensure a
microtask/drain tick so the bundle's `setInterval` poll for the solver
global sees it before timing out (SITE_wildberries WB-W2), and verify
the `me {tries}` storage carry across BO iterations doesn't self-trip
the `tries≥3` blank (`page.rs` `current_storage` carry; SITE_wildberries
WB-W3).

### B.4 (4) No-CDP-oracle capture + diff validation plan
Mirror the AWS oracle (`crates/browser/examples/aws_capture.rs` +
`awswaf_probe.rs`), per SITE_wildberries §5:
1. **Refresh capture (faithful TLS):** `aws_capture
   "https://www.wildberries.ru/" /tmp/wbaas/wb_stub.html` to re-pull the
   shell + bundle from our current IP. Record the served PoW difficulty
   field inside the 498 `{challenge}` body.
2. **Offline self-solve probe:** build `wbaas_probe_inject.js` trapping
   `fetch`/XHR (URL+status), `createElement('script')`+append (solver
   URL), `TextEncoder`/PoW entry, whether `challengeSolver.solve()` is
   reached + returns, and whether `document.cookie` gains
   `x_wbaas_token`. Run the stub through an offline build with
   `run_until_idle(5s)`.
   - **Reaches create-token-retry/cookie-write offline but NOT in live
     nav ⇒ pure drain ⇒ public engine fix (B.3 lever).**
   - **Stalls offline at `solve()` ⇒ PoW too heavy for budget ⇒ split:
     either the difficulty is IP-driven (need better IP, §B.5) or the
     PoW must be reproduced in `vendor_solvers`.**
3. **No-CDP IP A/B (the geo ceiling — the one external input we lack):**
   the valid oracle is **real Chrome (no CDP) from a foreign
   *residential* IP** (per A.2 it should load with NO challenge) vs the
   same from our US DC IP (498 + hard PoW). Capture both shells with a
   passive proxy/HAR. This quantifies how much of the gap is IP-trust
   (difficulty delta) vs engine (solve completion). **Do NOT use
   Playwright/CDP** — though wbaas is less CDP-sensitive than Kasada, a
   residential-vs-DC IP comparison is the only thing that matters here.

### B.5 (5) Honest verdict — wildberries
**MIXED, leaning ENGINE-ADDRESSABLE for the contestable part:**
- **The self-solve chain is ENGINE-ADDRESSABLE (public).** BO owns all
  primitives; the gap is the live-nav drain shared with AWS. Closing it
  grows the body past the shell and beats v150's THIN-39 — a defensible
  **win**, even from a DC IP, *if* the served PoW is completable in
  budget.
- **The trust ceiling is IP-reputation-bound (NOT engine).** From a US
  DC IPv6 we get the hostile branch. To reach the *easy* branch a real
  user enjoys (A.2: foreign residential loads with no challenge) needs a
  **non-DC IP** — ideally RU residential, at minimum a foreign
  *residential* IP. No header/Accept-Language/fingerprint trick
  substitutes for that; the gate keys on IP reputation, not on a JS
  signal BO can forge.
- **The PoW math, if it must be reproduced byte-for-byte at high
  difficulty, is `vendor_solvers`** (per CLAUDE.md per-vendor boundary) —
  but only pursue after the §B.4 oracle proves `solve()` is the wall AND
  a better IP can't lower the difficulty.

**Do NOT mark `diagnostic:true` and drop it** (the old vNext Option A):
v150 fails worse, and the engine half is real. Recommended order:
§B.4 oracle (decisive, shares AWS tooling) → public drain fix →
IP-A/B to set the ceiling → only then `vendor_solvers` if a hard PoW
wall remains at every IP.

### B.6 Future-proofing note (not blocking, worth filing)
Because BO evals module scripts as **classic scripts** (§B.3), a future
wbaas (or any site) that ships a *real* ESM bundle with static
`import`/`export` would **silently fail to compile** in BO. Today wbaas
dodges this (zero import/export). If a frontier site moves to true ESM,
BO needs `v8` module instantiation
(`ModuleSpecifier`/`load_main_module`) in the
`find_scripts`→eval path, not `v8::Script::compile`. Track as a latent
engine gap; not load-bearing for wildberries today.

---

## C. SITE 2 — ozon.ru

### C.1 (1) Exact detection mechanism blocking BO
**A hard foreign-IP geo-gate (dominant), with DDoS-Guard's JS+cookie
challenge layered behind it.** From a foreign / DC IP, ozon returns a
**thin ~156 B body** — the response *before* the JS/cookie gate even
runs, i.e. ozon refuses to serve the real document to a non-RU IP. BO's
net stack knows the *protocol* of the gate (DDoS-Guard's `307` on
`POST /abt/result`, `crates/net/src/lib.rs:979`, `__ddg*` cookies) but
that protocol only matters once the IP is acceptable.

### C.2 (2) Does a no-CDP real browser pass it? → engine-addressable?
**NO, from a foreign/DC IP.** External evidence (A.2) is explicit and
contrasts ozon with wildberries: **"Ozon would not load without a VPN"**
from abroad — and inside Russia it *blocks* VPN/DC traffic. So a real
Chrome (no CDP) from our US DC IP also fails. There is **no no-CDP
advantage to exploit** because the gate fires before any
CDP/automation/fingerprint signal is read — it is keyed on IP geo/
reputation. The CDP thesis does not rescue ozon.

### C.3 (3) Concrete engine path + no-CDP relevance
**Minimal.** The only engine-side surface is the DDoS-Guard cookie/JS
hop, and BO already handles the `307`-re-POST shape
(`crates/net/src/lib.rs:979-1003`). Completing the `__ddg` cookie
acquisition would not help while the geo-gate returns the thin body —
there is no document to grow. No engine lever moves ozon from a US DC
IP.

### C.4 (4) No-CDP-oracle capture + diff validation plan
1. Capture ozon's thin body + headers from our IP
   (`/tmp/ozon/ozon_stub.html`) to confirm it is the pre-JS geo-refusal
   (look for DDoS-Guard `Server:` / `__ddg` Set-Cookie vs an outright
   geo-block page).
2. **No-CDP IP A/B:** real Chrome (no CDP) from a **RU residential IP**
   vs our US DC IP. Per A.2 the RU residential should serve the real
   store; the DC IP should give the thin body. If RU-IP still thin →
   escalate; if RU-IP serves content → the residual is **100 % IP-geo**.
   (Strong prior: ozon needs a RU IP.)

### C.5 (5) Honest verdict — ozon
**IP-GEO-BOUND.** A **Russian residential IP is effectively required**;
a foreign/DC IP (even real Chrome, no CDP) gets the thin body. No
engine/fingerprint/header lever substitutes for the IP. The DDoS-Guard
cookie hop is already protocol-handled in BO and is downstream of the
geo-gate, so it is not the blocker. **Recommend: mark ozon
`diagnostic:true` / drop from the production pass-rate denominator** —
unlike wildberries, there is no engine half to win, and counting it
drags the rate without representing a fixable gap. Re-open only if a RU
residential IP becomes available (infra, not engine).

---

## D. Direct answers to the task's investigation questions

**(a) Does the 498 depend on IP alone, or also on TLS/fingerprint/headers?**
The 498 is served because of the **IP** (US Comcast/Xen-VM IPv6, A.1) —
that's what sets the low trust score and triggers the challenge at all.
But the 498 is **not purely IP**: it is the *entry* to an adaptive
challenge whose **difficulty** is IP-scaled and whose **scoring** also
reads JS device signals (UA/screen/timezone/viewport, A.3/A.4). So:
**IP decides *whether* and *how hard* you're challenged; fingerprint
decides your score *within* that band.** From a trusted (foreign
residential) IP, real Chrome gets little/no challenge (A.2). TLS/headers
are not the documented gate (wbaas scores the JS-collected profile, not
the ClientHello), though a Chrome-faithful TLS is table stakes to not be
trivially flagged.

**(b) Does BO execute the wbaas ES module, and would solving it help if
the IP is wrong?**
**Yes, BO executes it** — `find_scripts` collects the
`type="module"` script (`script_runner.rs:37-75`) and evals it via
`v8::Script::compile` (`js_runtime/src/lib.rs:93-122`), which works
*only because the bundle has no static import/export* (A.3; classic eval
would reject real ESM — see §B.6). The dynamic solver-injection and
498-iterate/reload/cookie-gain primitives all exist (§B.3). **Would
solving it help with the wrong IP?** **Partially.** Completing the
self-solve would grow BO's body past the 1883 B shell and beat v150's
THIN-39 (a win), *provided* the IP-scaled PoW difficulty is completable
in `V8DeadlineWatcher` budget. But it would **not** lift BO to the
trusted/easy experience a real user on a residential IP gets — the
server may still throttle/under-serve a US DC IP even with a valid
token (A.2 "throttle" + A.4 token revocation on anomalous IPs). So the
engine fix is necessary and worth doing, but **not sufficient** for full
parity from a DC IP.

**(c) Is there ANY engine path without a RU IP (Accept-Language / region
headers / geo-check JS)?**
For **wildberries: a partial one** — the engine self-solve chain (§B.3)
is real, IP-independent, and beats v150 even from a DC IP; and a
**foreign *residential*** IP (not necessarily RU) already passes per
A.2, so the requirement is "non-DC / non-VPN IP," not strictly "RU IP."
Accept-Language / region headers are **already wired** (per-region
accept-language, `crates/net` `get_with_headers`; script fetches inherit
it, `page.rs:3216-3218`) and are worth setting to `ru-RU` for fidelity,
but they do **not** flip the IP-trust gate — wbaas scores IP reputation,
not the header. For **ozon: no** — the geo-gate returns the thin body
before any engine surface is reachable; a RU residential IP is required.

---

## E. Net recommendation (ROI order)

1. **wildberries — run the §B.4 offline oracle** (decisive, shares the
   AWS tooling). Splits the work into public-engine (drain) vs
   `vendor_solvers` (PoW) vs IP-trust (infra). Highest value: it is the
   only frontier site where forward progress is a clean **win vs v150
   (THIN-39)**.
2. **wildberries — ship the public live-nav drain fix** (same lever as
   AWS §5.1) + verify solver-inject poll (WB-W2) + `me {tries}` storage
   carry (WB-W3). Set Accept-Language `ru-RU` for fidelity.
3. **wildberries — IP A/B** (foreign residential vs US DC) to set the
   ceiling. This is infra, not engine; it tells us whether the public
   fix fully passes or only grows the body.
4. **ozon — capture + confirm geo-refusal, then mark `diagnostic:true`
   and drop** from the production denominator. No engine half exists.
   Re-open only with a RU residential IP.
5. **File the latent ESM gap (§B.6)** — BO evals modules as classic
   scripts; a future true-ESM bundle would silently fail to compile.

---

## F. Sources / references
- Captured: `/tmp/awswaf/wb.html` (this session); SITE_wildberries §3
  bundle deobfuscation (`/tmp/wbaas_probe/wb_challenge.js`).
- Code: `crates/browser/src/script_runner.rs:37-75` (module-script
  collection), `crates/js_runtime/src/lib.rs:93-122`
  (`v8::Script::compile` classic eval),
  `crates/js_runtime/src/js/dom_bootstrap.js:142-234` (dynamic script
  inject + load event), `crates/browser/src/page.rs:1179, 2994` (498
  iterate), `:3554` (wbaas XHR note), `:3661` (50 ms inter-script
  drain), `:3674-3684` (DOMContentLoaded setTimeout),
  `crates/net/src/lib.rs:979-1003` (ozon DDoS-Guard 307 re-POST).
- Repo docs: `docs/v0.1.0-parity-workflows/sites/SITE_wildberries.md`
  (supersedes the drop-it framing), `docs/vNext/04_R-WBAAS-WILDBERRIES.md`
  (original capture + Options A/B/C),
  `docs/v0.1.0-parity-workflows/03_OUTPERFORM_V150_ANALYSIS.md:31`
  (wildberries+ozon = "(geo)" shared frontier),
  `docs/HANDOFF_2026_05_28b.md §4-5.1` (the shared live-nav drain lever).
- External: [Moscow Times 2026-04-15](https://www.themoscowtimes.com/2026/04/15/russian-websites-begin-blocking-vpn-users-as-internet-controls-tighten-a92511),
  [Meduza 2026-04-30](https://meduza.io/en/feature/2026/04/30/russia-blocks-vpn-access-to-major-platforms-moves-to-charge-for-mobile-vpn-traffic),
  [overclockers.ru](https://overclockers.ru/blog/kosmos_news/show/253042/),
  [rtvi.com](https://rtvi.com/news/wildberries-i-ozon-nachali-blokirovat-dostup-pokupatelyam-s-vpn/),
  [hi-tech.mail.ru](https://hi-tech.mail.ru/news/146844-ozon-i-wildberries-nachali-puskat-polzovatelej-s-vpn-posle-snizheniya-prodazh/),
  [Habr 1032556 (wbaas write-up)](https://habr.com/ru/companies/wildberries/articles/1032556/),
  [Habr 1018490 (AntiDDoS meetup)](https://habr.com/ru/companies/wildberries/articles/1018490/),
  [DDoS-Guard (Wikipedia)](https://en.wikipedia.org/wiki/DDoS-Guard),
  [DDos-Guard-Bypass](https://github.com/1337tr/DDos-Guard-Bypass),
  IP allocation: [IPQS AS33491 Comcast](https://www.ipqualityscore.com/asn-details/AS33491/comcast-cable-communications-llc),
  [ARIN](https://www.arin.net/resources/).
</content>
</invoke>
