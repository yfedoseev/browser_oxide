# SITE — wildberries.ru (wbaas in-house antibot + adaptive PoW + geo-trust)

**Status:** OPEN. BO `1883b/ERR`, Camoufox **v150 = THIN-39** (v150 FAILS WORSE).
**Vendor:** custom in-house "wbaas" (Wildberries-Antibot-as-a-Service). NOT Akamai/Kasada/DataDome/AWS/Cloudflare.
**Class:** SPA challenge shell (HTTP 498) → JS fingerprint scoring + **adaptive proof-of-work** → `x_wbaas_token` cookie → `location.reload()`.
**Verdict for v0.2.0:** **this is a genuine WIN opportunity** — no engine (incl. v150 + Patchright/Chromium) passes today, and the gate is mostly an **engine async-self-solve-drain** problem (public-engine-addressable), not a fingerprint-fidelity wall. The PoW math itself, if it must be reproduced byte-for-byte, is `vendor_solvers`.

---

## 1. What the repo already concluded (with citations)

- **`docs/vNext/04_R-WBAAS-WILDBERRIES.md`** — captured the 498 shell + the 70 KB ES-module bundle `/__wbaas/challenges/antibot/__static/v1/index-DQJ0L4Mq.js`, site-key `7400bd5df8b843b28254659f10915f31`. Concluded "likely out of scope / custom + geo-bound / single site", recommended **Option A: mark `diagnostic:true`** (the `areyouheadless` mechanism), Option B verify geo with a RU proxy, Option C reverse the bundle in `vendor_solvers` (~1-2 wk). **This document supersedes the "out of scope, just drop it" framing** for the reasons in §4–§6: the bundle is far simpler than the doc assumed (no Worker/Blob/WASM in the orchestrator), and the failure is the same engine drain class we are already fixing for AWS.
- **`docs/releases/v0.1.0-parity/FAILED_SITES_ANALYSIS.md` §C.4** — Stratum C (frontier). "BO chrome 1883b, BO pixel ERR, BO iphone 7900b, Camoufox v135 2710b, v150 **THIN 39**, Patchright 1818b. Different engines see different responses — unstable/geo. Not a stealth problem." Filed **R-CORPUS-WILDBERRIES** ("drop from corpus if geo-blocked", 30 min). The line `v150 THIN 39` is the load-bearing fact: **v150 does WORSE than BO's iphone profile (7900b).**
- **`docs/releases/v0.1.0-parity/18_ANTI_BOT_VENDOR_COOKBOOK.md` §2.12 + table** — conflated wbaas with "Walmart bot-as-a-service" (`x-wbaas-token`, paired with `bm_sz`). **That identification is WRONG for wildberries.ru.** The captured cookie is `x_wbaas_token` (underscore) set by Wildberries' own `/__wbaas/challenges/antibot` endpoint; there is no Walmart/`bm_sz` involvement. The cookbook's Walmart entry and this RU site coincidentally share the `wbaas` string. Detection-only logging is fine but the vendor attribution should be corrected.
- **`docs/releases/v0.1.0-parity/27_VENDOR_COMPETITIVE_MATRIX.md`** — "wildberries (custom RU shell): SPA shell; cross-engine fail at strict-pass. iphone has loose `L3 7900`; Camoufox `L3 8924` (also below 15 KB gate)." Confirms cross-engine failure and the 15 KB strict gate.
- **`docs/releases/v0.1.0-parity/05_SPA_HYDRATION_CLUSTER.md` §5.3** — lists wildberries among "stuck in mount-1 trap" sites that a more permissive hydration-plateau exit might let grow past the gate. (Relevant but secondary — wildberries never hydrates because the challenge never solves; the hydration heuristic is downstream of the real blocker.)
- **`crates/browser/src/page.rs:1156, 1178-1179, 2917`** — the navigate loop already treats `498/403/429` as challenge-shaped and logs `[vendor-detect] wbaas` on an `x-wbaas-token` response header. Detection only; no flow change.

---

## 2. New external findings (cited)

### 2.1 Wildberries' own engineering write-up (decisive)
**Habr — "Как работает антибот в мобильном приложении Wildberries"** (Wildberries company blog): <https://habr.com/ru/companies/wildberries/articles/1032556/>. Their description maps 1:1 onto the captured web bundle:
- Two parts: a **client SDK library** (token procurement) + a **server** (validation + risk scoring). The web bundle is the JS port of that SDK ("`sdkVersion = "js-front-" + getPlatform()`" — see §3).
- **"JS-челлендж"** = device-fingerprint **scoring**: "собирает параметры окружения, нормализует признаки и формирует профиль устройства" (gather environment params, normalize, build a device profile).
- **Proof of Work**: client must "найти определённое количество хэшей, удовлетворяющих условиям" (find N hashes meeting a difficulty condition). **Difficulty is adaptive** — trusted clients get easy tasks, suspicious IPs get hard ones.
- **Adaptive policy**: fewer checks for legit sessions; harder challenges for suspicious ones; **token revocation** post-issuance on anomalous behaviour.
- **ML scoring** on both challenge-solution quality and ongoing behaviour.

**Implication:** From a US/CA datacenter IPv6 (the capture shows `data-req-ip="2001:569:728c:..."`, a non-RU DC range), wildberries assigns a **low trust score** → **high PoW difficulty** and a strict fingerprint-scoring profile. This is exactly why every engine fails: it is partly IP-reputation-adaptive.

### 2.2 The mobile/Android variant is documented as PoW + SDK
The Habr piece and **antibot.blog** (<https://antibot.blog/>) corpus confirm the SDK pattern (request → 401/denial → SDK solves challenge → token → retry). No public web-side solver for `x_wbaas_token` exists (search returned only general anti-bot guides and a stale MySQL/proxy scraper `github.com/berenzorn/wildberries` that predates wbaas). So **there is no off-the-shelf bypass to copy** — but also **no competitor passes**, which is the win opening.

### 2.3 Camoufox v150 THIN-39 explained
Camoufox is Firefox-based (Juggler, no CDP). The wbaas shell is served to it too. v150 returning **39 bytes** (vs BO iphone 7900b) means Firefox's network/JS stack got an even thinner challenge response — most likely the adaptive policy served Firefox a hostile/empty branch, or v150's headless-Firefox fingerprint scored so low the server short-circuited. Either way **BO is already ahead of the open-source SOTA here**, and a partial solve would be an outright win.

Sources: [Habr/Wildberries antibot](https://habr.com/ru/companies/wildberries/articles/1032556/), [antibot.blog](https://antibot.blog/), [berenzorn/wildberries scraper (stale)](https://github.com/berenzorn/wildberries), [Scrapfly anti-bot guide](https://scrapfly.io/blog/posts/how-to-bypass-anti-bot-protection-when-web-scraping).

---

## 3. Code-level deobfuscation of the captured bundle

Artifacts: `/tmp/wbaas_probe/wb.html` (1447 B challenge shell), `/tmp/wbaas_probe/wb_challenge.js` (70138 B, `index-DQJ0L4Mq.js`).

### 3.1 The shell (`wb.html`)
```html
<html data-req-uuid="bd4ad6..." data-req-ip="2001:569:728c:...">  <!-- non-RU DC IPv6 -->
<meta http-equiv="refresh" content="60">                          <!-- 60s self-reload -->
<title>Почти готово...</title>                                    <!-- "Almost ready" -->
<script type="module" crossorigin src="/__wbaas/.../index-DQJ0L4Mq.js"></script>
...
<div id="wait_msg"></div><div id="c_cont"></div>                  <!-- mount points -->
<b id="s-key" data-site-key="7400bd5df8b843b28254659f10915f31"></b>
```

### 3.2 The orchestrator bundle — verified marker counts (`grep`)
- `cookie` ×20, `fetch(` ×12, `JSON.stringify` ×13, `setTimeout` ×4, `localStorage` ×4, `navigator.userAgent` ×2, `TextEncoder` ×3.
- **`WebAssembly` ×0, `new Worker` ×0, `new Blob` ×0, `crypto`/`subtle`/`digest` ×0, `import(` ×0, `export` ×0.**

This **contradicts the AWS-cluster assumption baked into vNext** that wbaas needs a blob-URL PoW worker. The orchestrator does NOT do the hashing itself and uses **no Web Worker, no WASM**. The PoW hash routine lives in a **separately downloaded solver module** (§3.4).

### 3.3 The boot + token flow (deobfuscated)
On `DOMContentLoaded` (registered via `addEventListener("DOMContentLoaded", async()=>{...})`):
1. **Cookie test** `fe()`: writes `cookietest=<rand>`, reads it back, deletes it. If cookies disabled → renders the "включите cookies" warning and stops.
2. **Try-counter gate**: `me` key in `localStorage`, `{tries, lastTryTime}`. `tries < 3` increments and proceeds; `tries >= 3` blocks unless `Date.now()-lastTryTime >= 60000` (60 s), then resets. **So at most 3 token attempts per 60 s window** — important for BO's retry loop (don't burn the budget).
3. Reads `#s-key` `data-site-key`, builds the SDK: `new pe({baseUrl:"/__wbaas/challenges/antibot", siteKey})`. `cookieName="x_wbaas_token"`, `sdkVersion="js-front-"+getPlatform()`.
4. Checks for an existing `x_wbaas_token` cookie (`"; "+document.cookie .split("; x_wbaas_token=")`). If present+valid → no challenge.
5. `createToken()` → `createTokenWithRetries({}, ee.CT, "/api/v1/create-token")`:
   - First POST `/api/v1/create-token` (no `secureToken`). Server returns **`498` with JSON body `{challenge}`** → the bundle throws a typed `s` error: `if(498===r){const{challenge:t}=await e.json(); throw new s(t,"Request to solve next challenge")}`.
   - Caught → `initChallengeSolver()` → `challengeSolver.solve(challenge)` → `enrichPayload(e, solution)` adds `{secureToken}` → **recursive** `createTokenWithRetries(o,...)` retry POST.
   - On `2xx` → `parseByContentType` → returns token. `challengeSolver.finish()`.
6. On success the token is written to cookie and the page reloads:
   ```js
   document.cookie = "x_wbaas_token="+token+"; secure; SameSite=None; max-age=1209600";  // 14 days
   document.location.reload();
   ```

### 3.4 The dynamically-loaded solver module (the PoW)
```js
async initChallengeSolver(){
  const e = (await this.loadSettings()).solverPath;     // from POST /api/v1/find-frontend-settings
  return new (await _(this.baseUrl + e))(this.baseUrl);  // _ = dynamic <script> injector
}
```
`_` = a script-injection loader (NOT ESM `import()`):
```js
const r=document.createElement("script");
r.src=i; r.async=!0; r.type="text/javascript";
r.addEventListener("load",()=>e(!0));
r.addEventListener("error",e=>t(e));
(document.head||document.body).appendChild(r);
// then poll window[<global>] via setInterval until the solver class appears
```
**Endpoints (verified strings):** `/api/v1/find-frontend-settings`, `/api/v1/create-token`, `/api/v1/create-one-time-token`, `/api/v1/frontend-analytics`, `/api/v1/report`.

The solver class exposes `.solve(challenge)` / `.finish()` / `.close()`. The PoW hashing (the "find N hashes" loop from §2.1) lives in **that** module, which we did NOT capture (the orchestrator only references it by the runtime-fetched `solverPath`). It uses `TextEncoder` (present in orchestrator imports) — most likely a SHA-256/argon-style loop on the main thread (no Worker), gated by an adaptive `difficulty` field inside `challenge`.

### 3.5 UA parser (NOT the gate)
The bundle bundles a bowser-style UA parser (`getBrowserName/getOSName/getEngineName/getPlatform`, with a table containing `PhantomJS/PingdomBot/Puffin`). The 4× `phantom` hits are this parser's bot-name table, used only for `/api/v1/frontend-analytics` + `/api/v1/report` telemetry, **not** a headless gate. So a "phantom/headless string" leak is not the blocker.

### 3.6 What gets POSTed to the server (scoring signals)
The analytics/report payload (`Oe`, `report`) sends: `ua`, parsed `browser/browser_v/os/os_v/engine/platform`, `viewport`, `screen`, `timeZone` (`Intl.DateTimeFormat().resolvedOptions().timeZone`), `navigator.onLine`, `cookieEnabled`. The `create-token` payload carries `{secureToken, challenge, action?, userScope?}`. **These are exactly the "environment params → device profile" from the Habr write-up** — the server scores them and sets the PoW difficulty.

---

## 4. Why BO fails today — code-level localization in BO

The orchestrator IS fetched and IS executable in BO (it is `type="module"` but has no `import`/`export`, so BO's classic-script executor runs it). The failure is the **async self-solve never completing in the live navigate path** — the same drain class identified for AWS in `HANDOFF_2026_05_28b.md §4`, with two wbaas-specific wrinkles:

### 4.1 The whole flow is `DOMContentLoaded`-gated and deeply async
BO fires `DOMContentLoaded` via `setTimeout(0)` (cold path `crates/browser/src/page.rs:585-588`; warm path `:1693-1699`). The bundle's entire chain runs **inside** that handler and is a long promise chain:
`find-frontend-settings (fetch)` → `inject solver <script> + poll setInterval` → `create-token (fetch, 498)` → `solve() (CPU PoW loop)` → `create-token retry (fetch)` → `cookie write` → `location.reload()`.

For this to complete, BO must keep the isolate draining across **multiple network round-trips + a CPU PoW loop + a setInterval poll**. The **per-iteration drain at `crates/browser/src/page.rs:2045-2057` already uses the full remaining nav budget (floored at 8 s) with a `V8DeadlineWatcher`** — that part is fine. The problem is upstream:
- The **inter-script drain inside `build_page_with_scripts_init_and_storage` is only 50 ms** (`page.rs:1676-1679` warm path; `:3544+` cold path interleave). The module executes, registers the DOMContentLoaded handler, and returns. DOMContentLoaded itself only fires on the `setTimeout(0)` *after* the script loop. The handler's async chain then has to fully advance during the final per-iter drain.
- **The try-counter gate (`localStorage me {tries}`)**: each BO build iteration is a **fresh page with storage carried via `current_storage`** (`page.rs:2024, 2841`). If storage is NOT persisted correctly across iterations, every iter is `tries=1` (fine); if it IS persisted but the 60 s window logic mis-times against BO's fast retries, BO can trip the `tries>=3` block and the bundle **renders nothing and stops** → stays at the 1883 B shell. Verify which.

### 4.2 The solver `<script>` injection path — verify it actually fetches+evals
The solver is loaded via `document.createElement("script")` + `r.src=...` + `appendChild`. BO's dynamic-script handling:
- `Document.createElement("script")` (`crates/js_runtime/src/js/dom_bootstrap.js:1430-1455`) overrides the `src` setter but **only stores `_src` + sets the attribute — it does NOT fetch/exec**. Fetch+eval happens on **insertion** via `_onNodeInsertedInner` (`dom_bootstrap.js:142-229`).
- `_onNodeInsertedInner` DOES fetch a `script` child with a `src` and eval it, then **dispatches `new Event('load')`** (`:189-203, 218-219`), which satisfies the bundle's `r.addEventListener("load",...)`. **So the primitive exists.** BUT: it is gated by `_MAX_SYNC_EVAL_DEPTH=4` and `_syncFetchInFlight` dedup, and falls back to an **async** fetch+eval when nested (`:210-227`). The solver injection happens *inside* the DOMContentLoaded async chain — confirm it routes through the async branch and that the `setInterval`-poll for `window[<solverGlobal>]` actually sees the class after eval. **If the injected solver evals but the bundle's poll runs out before the next drain tick, the chain stalls.** This is the highest-value thing to instrument.

### 4.3 The cookie-write + `location.reload()` → BO's re-fetch primitive
On success the bundle sets `x_wbaas_token` and calls `document.location.reload()`. BO already has the right primitives:
- `location.reload()` sets `__pendingNavigation` → the navigate loop's **in-V8 re-fetch with `credentials:'include'`** (`page.rs:2780-2836`) re-requests the URL *carrying the jar cookies*, and accepts the body if it grew and lacks challenge markers (`looks_real`, `:2825-2831`).
- The **cookies-gained signal** (`page.rs:2002-2016`) is the F5 primitive that already flips Kasada-style "solve-but-no-reload" sites. wbaas DOES call reload explicitly, so this should chain cleanly **once the token is actually computed**.

**Net:** BO's reload/re-fetch/cookie-gain machinery is sufficient. The two unknowns are (a) does the solver module fetch+eval+resolve inside one drain window, and (b) does the PoW loop actually run to completion under BO's `V8DeadlineWatcher` budget. Both are answerable with the existing AWS oracle, repurposed.

### 4.4 The `498` initial handling
`page.rs:1156` and `:2917` already classify `498` as challenge-shaped and keep iterating (don't abort). Good — no change needed there. The vendor-detect logging keys off the `x-wbaas-token` **response header** (`:1178`); the live shell delivers the token via **`document.cookie` + body**, not a response header, so the detect log may not fire — cosmetic only.

---

## 5. The offline-oracle experiment to run first (cheap, decisive)

Mirror the AWS oracle (`HANDOFF_2026_05_28b §5.1`, `crates/browser/examples/aws_capture.rs` + `awswaf_probe.rs`):
1. `aws_capture "https://www.wildberries.ru/" /tmp/wbaas/wb_stub.html` (faithful TLS) to refresh the shell + bundle.
2. Build a `wbaas_probe_inject.js` that traps: `fetch` calls (URL+status), `createElement('script')`+append (solver URL), `crypto/TextEncoder` usage, and whether `challengeSolver.solve` is reached + returns, and whether `document.cookie` gets `x_wbaas_token`.
3. Run the stub through an offline page build with `run_until_idle(5s)` (the oracle drain). **If the chain reaches `create-token` retry / cookie-write offline but NOT in the live nav path → it is purely the drain (public-engine fix, §6 W1).** **If it stalls offline at `solve()` → the solver module needs the PoW reproduced (`vendor_solvers`, §6 V1).**

This single experiment partitions the work between public-engine and `vendor_solvers` and should take ~1 day with the AWS oracle as a template.

---

## 6. Ranked fix list (ROI order)

| ID | Fix | Effort | Expected impact | Confidence | Engine |
|----|-----|--------|-----------------|-----------|--------|
| **WB-W1** | **Run the offline oracle (§5)** to split drain-vs-PoW. Then, if drain: extend the live-nav drain so the full `DOMContentLoaded → find-settings → solver-inject → create-token×2 → reload` chain advances (same lever as AWS §5.1: longer post-script drain + ensure the prefetched module's async continuation isn't dropped). | 2-4 days (shares code with AWS lever) | wildberries flips to ≥ shell-grown / real content. **Outright WIN vs v150 (THIN-39).** Likely also unblocks the AWS cluster + booking (same drain class). | medium | **public** |
| **WB-W2** | **Verify solver `<script>` inject path** (`dom_bootstrap.js:142-229`): confirm the async fallback fetch+evals the runtime solver and that the bundle's `setInterval` poll sees the global before timeout. Add a drain tick / microtask flush after dynamic-script eval if the poll misses. | 1-2 days | Removes the most likely stall point; enabler for WB-W1. | medium | **public** |
| **WB-W3** | **Verify try-counter storage** (`localStorage me {tries}` vs BO's `current_storage` carry, `page.rs:2024/2841`): ensure fast BO retries don't trip `tries>=3` and blank the page. Cap BO's wbaas iterations to ≤2 within 60 s if needed. | 0.5 day | Prevents a self-inflicted block that masks a real solve. | medium | **public** |
| **WB-W4** | **Geo/trust A/B** (vNext Option B): rerun from a Russian-IP proxy. If RU IP serves real content with no challenge → the residual is pure IP reputation (out of engine scope); if RU IP still 498s → the bundle is the gate and WB-W1/W2 are the whole fix. Disambiguates how much of THIN-39 vs 7900b is IP-adaptive PoW difficulty. | 0.5 day (needs RU proxy) | Sets the ceiling; tells us whether a public-engine fix can fully pass or only partially grow. | high | infra |
| **WB-V1** | **Reproduce the adaptive PoW** if WB-W1 shows `solve()` stalls (server demands a hash the JS solver can't complete in budget at our IP's difficulty). Capture + reverse the `solverPath` module; port the hash loop. Per CLAUDE.md this is **per-vendor solving → `vendor_solvers`**. Only pursue if WB-W4 shows the engine can otherwise reach the page and the PoW is the sole wall. | 1-2 weeks | Full pass even at high difficulty. | low | **vendor_solvers** |
| **WB-X** | **Correct the cookbook vendor attribution** (`18_ANTI_BOT_VENDOR_COOKBOOK.md §2.12`): wildberries `wbaas` ≠ Walmart `bm_sz`. Detection-only cleanup. | 15 min | Doc accuracy; avoids future misrouting. | high | public (docs) |
| ~~WB-A~~ | ~~Mark `diagnostic:true` and drop (vNext Option A)~~ | 15 min | **Do NOT do this yet.** v150 fails worse (THIN-39); a partial BO solve is a defensible win. Only fall back to this if WB-W4 proves a hard RU-only IP gate AND WB-W1 can't grow the body past the gate. | — | corpus |

**Recommended sequence:** WB-X (free) → WB-W1's §5 oracle (decisive, shares AWS tooling) → WB-W2/W3 (the two likely stall points) → WB-W4 (set the ceiling) → only then WB-V1 if a hard PoW wall remains.

**Why this beats "drop it":** the orchestrator has no WASM/Worker/blob wall (§3.2) — far simpler than the vNext doc assumed; BO already owns the reload/cookie-gain/in-V8-refetch primitives (§4.3) and the 498-keep-iterating logic (§4.4). The remaining gap is the **same live-nav async-drain lever already scheduled for AWS**, so wildberries is a near-free rider on that work — and since **v150 fails worse**, any forward progress is a head-to-head win.

---

## 7. Open questions / unknowns to resolve in the oracle
1. Does the live nav path even reach `find-frontend-settings`, or does the module stall before `DOMContentLoaded` fires? (instrument fetch trap)
2. Does the runtime solver module fetch+eval, and does its global appear before the `setInterval` poll times out? (WB-W2)
3. Is `solve()` reached, and does it return within `V8DeadlineWatcher` budget at our IP's difficulty, or does the PoW loop blow the deadline? (the public-vs-vendor fork)
4. RU-IP behaviour (WB-W4): is the challenge served at all from a trusted geo, or is THIN-39/7900b purely difficulty-scaling?
