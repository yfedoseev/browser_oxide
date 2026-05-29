# SITE: etsy.com — DataDome interstitial root-cause + fix plan

**Verdict matrix (FAILED_SITES_ANALYSIS.md:35):**

| Engine | Result |
|---|---|
| BO chrome_148_macos | `DataDome-CHL` (~1424 b, ~90 s budget cap) |
| BO firefox_135 / iphone / pixel | `DataDome-CHL` (all 4 profiles) |
| Camoufox **v150** | `DataDome-CHL` |
| Patchright | `DataDome-CHL` |
| Camoufox **v135** | `7913b` (loose L3 — partial interstitial progress, NOT strict pass) |

**Stratum:** C — "no engine tested passes; true open-source frontier" (FAILED_SITES_ANALYSIS.md:13). etsy is the lone DataDome site in the residual set. yelp/tripadvisor are the sibling cluster (07_DATADOME_PRIMITIVES.md:4).

**Headline:** This is **NOT** a fingerprint or stealth gap. The public engine already ships the 3 rendering primitives needed to let DataDome's own bundle self-solve. The site stays CHL for **two** reasons, only one of which is engine-addressable: (A) a **child-iframe cookie-jar isolation bug** in `iframe.rs` means a `datadome=` cookie landed by the device-check round-trip never reaches the parent jar that gates the retry — **public-engine fixable**; (B) the actual encrypted device-check POST requires the **daily-rotating signals key + behavioral signals** — **vendor_solvers scope**, and unsolved by v150/Patchright too.

---

## 1. What the existing repo docs already concluded

### 07_DATADOME_PRIMITIVES.md — the 3 public-engine primitives (SHIPPED)
The vendor-strip commit `aecdf19` deleted `crates/browser/src/datadome_handler.rs` (423 LOC) and all per-vendor flows. Doc 07 specified restoring the *load-bearing scaffolding* as vendor-agnostic primitives:

1. **Challenge-doc CSP relaxation** — the origin's restrictive 403 CSP would refuse `captcha-delivery.com`; relax it for the interstitial so the bundle can fetch its cross-origin assets (07:131).
2. **Cross-origin challenge-iframe materialization** — un-gate `rematerialize_iframes` so a script-injected `<iframe src="geo.captcha-delivery.com/...">` is actually fetched + executed instead of getting only a synthetic `contentWindow` shim. This is **FP-E1**, "the single highest-leverage rendering gap" (07:262, page.rs:691).
3. **Solved-cookie retry** — re-fetch the original URL once the jar gains a `datadome=` clearance cookie (07:408).

Doc 07 predicted these flip etsy + tripadvisor to `L3-RENDERED` on ≥1 profile (07:577) and explicitly scoped the **DataDome WASM-iframe daily-key solver** OUT to `vendor_solvers` (07:622).

### 12_R-DATADOME-WASM.md — the boundary, post-ship
Confirms FIX-DD (**commit `78a1241`**) shipped all 3 primitives + 4 unit tests. The boundary is laid out as a 10-step flow (12:53): public engine handles steps 1-6 + 9 (fetch interstitial, detect, relax CSP, run dd-script, materialize iframe, cookie-watch break, cookie-diff retry); **vendor_solvers** handles steps 7-8 (the WASM/signals computation that produces the token the device-check endpoint accepts). Two flagged public follow-ups (12:75): (a) the 50 KB detection gate may miss newer 5-10 KB interstitials; (b) the cookie-watch should handle the multi-cookie `datadome` + `_pxhd`/`_px3` pattern.

### FAILED_SITES_ANALYSIS.md — v135 regression note
Camoufox v135 reached `7913b` (partial progress past the initial CHL) but **v150 and BO both regressed to CHL** (FAILED_SITES:161). DataDome changed something that broke v135's old behavior too — i.e. this is a moving target, not a static BO deficiency. Filed as **R-DATADOME-DAILY-KEY**, scope `vendor_solvers` (FAILED_SITES:200). Estimated corpus impact of this single site: routed median is gated by the Stratum-A/B set, not by etsy (FAILED_SITES:203,216).

---

## 2. New external findings (live research, 2026-05-28)

The DataDome **interstitial (5 s) challenge** flow, corroborated across multiple independent sources:

- 403 response carries a small HTML body with a `dd` object (`{rt, cid, hsh, b, s, host}`) and a reference to `https://ct.captcha-delivery.com/i.js`. (`rt:'i'` = silent interstitial; `rt:'c'` = interactive captcha — yelp gets the latter, which is unsolvable headlessly.) — [Hyper Solutions: Interstitial](https://docs.hypersolutions.co/datadome/interstitial), [glizzykingdreko — Breaking Down DataDome](https://medium.com/@glizzykingdreko/breaking-down-datadome-captcha-waf-d7b68cef3e21)
- The script builds a **deviceLink**: `https://geo.captcha-delivery.com/interstitial/?initialCid={cid}&hash={hsh}&cid={datadomeCookie}&referer={referer}&s={s}&b={b}&dm=cd`, GETs it to receive the device-check document, then **POSTs encrypted form-data signals** back to `https://geo.captcha-delivery.com/interstitial/`. The response sets `Set-Cookie: datadome=...; Domain=.example.com; Max-Age=31536000; Secure; SameSite=Lax`. — [Hyper Solutions: Interstitial](https://docs.hypersolutions.co/datadome/interstitial)
- **The hard part = daily key rotation:** "every day, the keys in the signals dictionary change to a random six-character string; if you don't match them correctly the solve is invalid." Plus the payload encodes Picasso-canvas + audio fingerprint inputs and behavioral signals (mouse/scroll/hover). — [glizzykingdreko — Breaking Down DataDome](https://medium.com/@glizzykingdreko/breaking-down-datadome-captcha-waf-d7b68cef3e21), [Kameleo — Bypassing DataDome 2025](https://kameleo.io/blog/guide-to-bypassing-datadome)
- Public deobfuscators/encryptors exist as references for the signal format ([glizzykingdreko/Datadome-Interstitial-Deobfuscator](https://github.com/glizzykingdreko/Datadome-Interstitial-Deobfuscator), [/Datadome-Interstital-Encryptor](https://github.com/glizzykingdreko/Datadome-Interstital-Encryptor)) — these are exactly the kind of per-vendor encoder CLAUDE.md keeps out of public crates.

**Architectural implication for BO:** the interstitial's `i.js` typically runs **inline in the main document** (it then *creates* the device-check sub-context). Whether DataDome currently uses an `<iframe src="geo.captcha-delivery.com/interstitial/">` or an inline `fetch()`/XHR varies by deployment and date. **Both paths converge on the same BO failure** (see §3): the clearance cookie is written into an isolated jar, not the parent's.

---

## 3. BO code-level analysis — where it actually breaks

The 3 primitives are present and correctly wired. Tracing the live flow:

### 3a. Detection + CSP relax — WIRED, but gate is brittle
```
crates/browser/src/page.rs:208  fn is_datadome_challenge(html) -> html.len() < 50_000 && html.contains("captcha-delivery.com")
crates/browser/src/page.rs:1794 let relax_csp = solvers.iter().any(|s| s.relax_response_csp(&html)) || is_datadome_challenge(&html);
crates/browser/src/page.rs:1845 let started_as_dd_challenge = ... || is_datadome_challenge(&html);
```
This fires for the etsy interstitial (small body + `captcha-delivery.com`). **Gap (minor):** the detector keys on the literal `captcha-delivery.com`. If DataDome serves an interstitial whose initial body references only `ct.captcha-delivery.com` via the `dd` object's `host` field or only `i.js` without the bare apex string, detection silently fails and none of the 3 primitives run. The 50 KB upper gate is fine for etsy's ~1.4 KB body but per 12:75 could miss 5-10 KB variants.

### 3b. Iframe materialization (FP-E1) — WIRED
```
crates/browser/src/page.rs:2175-2202  poll gate (started_as_dd_challenge) → rematerialize_iframes() each 200ms tick
crates/browser/src/page.rs:705-760    rematerialize_iframes(): find_iframes() on post-JS DOM, ChildIframe::from_url() for new ones
```
`rematerialize_iframes` correctly walks the post-JS DOM, dedups against `self.children` (page.rs:720), CSP-`frame-src`-gates (iframe.rs:84), and fetches+executes the child. **This part is sound.**

### 3c. THE PUBLIC-ENGINE BUG — child iframe runs on an ISOLATED cookie jar
This is the load-bearing finding and is **not documented in 07 or 12.**

`ChildIframe::from_url` (`crates/browser/src/iframe.rs:73-238`) builds the child context with `BrowserJsRuntime::with_options` (iframe.rs:173). That path, in `crates/js_runtime/src/runtime.rs:84-90`, does:
```rust
let fetch_state = match &options.stealth_profile {
    Some(profile) => {
        crate::extensions::fetch_ext::init_fetch_client(profile);   // ← brand-new HttpClient
        FetchState::with_profile(profile)                            // ← ANOTHER brand-new HttpClient
    }
    ...
};
```
`FetchState::with_profile` calls `net::HttpClient::new(profile)` (fetch_ext.rs:42-46) — a **fresh client with a fresh, empty cookie jar**, entirely independent of the parent `Page`'s `client`. The `FETCH_CLIENT` thread-local (fetch_ext.rs:58-60) is *also re-initialized*, clobbering the parent's during the child build.

Consequences for the DataDome flow:
1. The child iframe's GET of the device-check page and its (would-be) POST to `geo.captcha-delivery.com/interstitial/` go through the **child's** client. Any `Set-Cookie: datadome=...` lands in the **child's** jar.
2. The parent's cookie-diff retry reads the **parent** jar:
   ```
   page.rs:2363  let cookies_after = client.cookies_for_url(p).await...   // parent client
   page.rs:2397  let should_retry = cookies_after != cookies_before && !cookies_after.is_empty();
   ```
   Because the child never wrote to the parent jar, `cookies_after == cookies_before` → `should_retry = false` → no re-fetch → the engine returns the 1.4 KB interstitial → **`DataDome-CHL`**.
3. The poll's break-on-solve has the same blindness:
   ```
   page.rs:2215-2239  if started_as_dd_challenge { let now = client.cookies_for_url(p)...; if is_datadome_solved(&now, &body) break; }
   ```
   `is_datadome_solved` (page.rs:221) needs `datadome=` in the parent jar — never present from a child solve.

**Net:** even with a *hypothetically perfect* DataDome solver running inside the child iframe, BO would still return CHL, because the clearance cookie is structurally trapped in the child's jar. This is a genuine public-engine correctness gap in the iframe model, independent of the vendor solver.

> Note the thread-local clobber (fetch_ext.rs:53-59 comment) is by design to isolate *parallel pager workers* — but it also means a synchronously-built child iframe transiently replaces the parent's `FETCH_CLIENT`. Whether the parent's is restored after the child build returns needs a targeted check; if not, this can also corrupt the parent's subsequent same-thread fetches during the challenge poll.

### 3d. The device-check execution / signals — VENDOR SCOPE
Even with the cookie plumbing fixed, the child iframe must:
- run `i.js` / the device-check JS to completion (canvas Picasso + audio fingerprint + behavioral signals),
- compute the **daily-rotating 6-char signal keys**,
- encrypt + POST the form-data the way DataDome expects.

None of that is in the public engine and (per CLAUDE.md) must not be. It is the WASM/signals solver tracked in 12_R-DATADOME-WASM.md → `vendor_solvers`. v150 and Patchright also fail here, so closing it is a *frontier* lift, not a v150-parity requirement.

### 3e. Why v135 reached `7913b` and v150/BO don't
v135 predates the daily-key signal rotation hardening; its older interstitial handling completed against the older signal format. DataDome's key rotation (external research §2) broke it. This is consistent with FAILED_SITES:161 and confirms etsy is a *moving vendor target*, not a fixed BO bug — argues for putting the actual solve in `vendor_solvers` (where it can rotate independently of the engine) and keeping only the cookie/jar plumbing in public.

---

## 4. Ranked fix list (ROI order)

### FIX-1 — Propagate child-iframe `Set-Cookie` into the parent cookie jar (PUBLIC ENGINE)
**The one engine-addressable lever that unblocks the whole DataDome retry path.** Make `ChildIframe::from_url`/`from_srcdoc` share the parent's `HttpClient` (and thus its cookie jar) instead of minting a fresh one, OR after the child build, merge child-jar `Set-Cookie` deltas for the origin's registrable domain back into the parent client. Real browsers share one cookie store across frames of the same eTLD+1; BO must too. Then `is_datadome_solved` (page.rs:221) and the cookie-diff retry (page.rs:2397) actually see the clearance cookie.
- **Effort:** 2-3 days (thread the parent `&net::HttpClient` into `ChildIframe::from_url`; or add `client.merge_cookies_from(&child_client, origin)`; audit the `FETCH_CLIENT` thread-local restore in §3c note; add a unit test that a child-set `datadome=` is visible via `parent.cookies_for_url`).
- **Expected impact:** Necessary precondition for etsy/tripadvisor to *ever* flip. Alone it flips nothing (still needs the solver), but it is the gate everything else depends on — and it likely also unblocks **Cloudflare Turnstile / CF Managed Challenge** and any vendor whose iframe lands the clearance cookie. Confidence the *bug is real*: **high**. Confidence it flips etsy by itself: **low** (needs FIX-3).
- **Scope:** PUBLIC ENGINE.

### FIX-2 — Harden DataDome interstitial detection + multi-cookie clearance (PUBLIC ENGINE)
Broaden `is_datadome_challenge` (page.rs:208) beyond the bare `captcha-delivery.com` literal to also match `ct.captcha-delivery.com`, the `i.js` reference, and the `dd` object shape (`var dd=` / `"rt":"i"`); raise the size gate handling per 12:75 for 5-10 KB bodies; and extend `is_datadome_solved` (page.rs:221) to the `datadome`+`_pxhd`/`_px3` multi-cookie pattern.
- **Effort:** 1 day (extend two predicates + their 4 unit tests at page.rs:3797-3843).
- **Expected impact:** Prevents silent detection misses when DataDome rotates the interstitial body shape (the exact failure mode that broke v135). 0 direct flips today; insurance against regression and a prerequisite for FIX-1/FIX-3 firing at all.
- **Scope:** PUBLIC ENGINE.

### FIX-3 — DataDome interstitial signals solver (VENDOR_SOLVERS — frontier)
The daily-key signal encryption + canvas/audio/behavioral payload + encrypted POST to `geo.captcha-delivery.com/interstitial/`. Reference format: the public deobfuscator/encryptor repos cited in §2. Must live in `vendor_solvers` per CLAUDE.md; would register via the `ChallengeSolver` trait (challenge.rs:55) and use `solved_signal` to break the poll (already consumed at page.rs:2235).
- **Effort:** 1-2 weeks (per 12:6), plus ongoing maintenance against daily key rotation.
- **Expected impact:** etsy + tripadvisor flip to L3-RENDERED (only AFTER FIX-1 lands so the cookie propagates). yelp stays CHL (interactive `rt:'c'` captcha — human gate, unsolvable; v135/v150 also fail). So **+1-2 frontier sites**, beyond v150.
- **Scope:** VENDOR_SOLVERS (NOT public). Lower ROI for the v150-parity goal since v150 also fails etsy — this is a *lead over* v150, not a *catch up to* it.

### FIX-4 — Verify the device-check JS executes in the child realm (PUBLIC ENGINE, diagnostic)
Before investing in FIX-3, instrument a child iframe built from a captured etsy device-check document to confirm `i.js` runs to completion in BO's child realm (canvas ops, audio ops, `crypto.subtle` — the worker secure-context fix `5216336` should already cover any worker the bundle spawns). Reuse the `aws_capture`/`awswaf_probe` oracle pattern (HANDOFF_2026_05_28b §6) adapted to DataDome. If `i.js` bails early in the child realm (missing API, CSP block on a sub-resource), that's a cheaper public-engine fix than FIX-3.
- **Effort:** 1-2 days (capture + oracle harness; reuse existing oracle scaffolding).
- **Expected impact:** De-risks FIX-3; may surface a cheap missing-primitive that lets the bundle progress further (the v135→`7913b` behavior suggests partial completion is reachable).
- **Scope:** PUBLIC ENGINE (diagnostic/harness).

---

## 5. Strategic note
For the **v150-parity** goal, etsy is **not** on the critical path: v150 also gets `DataDome-CHL`, so etsy contributes 0 to closing the BO−v150 gap. The trustworthy gap (HANDOFF_2026_05_28b §3) is concentrated in the AWS-WAF cluster + booking + imdb — chase those first (§5.1 live-nav drain). etsy is a **frontier lift** (outperform v150). The right sequencing: land **FIX-1** (cheap, broad, also helps Cloudflare) and **FIX-2** (insurance) in the public engine now; defer **FIX-3** to `vendor_solvers` after the AWS cluster lands. FIX-4 gates whether FIX-3 is even worth starting.

## Files referenced
- `crates/browser/src/page.rs:208,221,1794,1845,2175-2202,2215-2239,2363,2397` — detection, primitives, retry
- `crates/browser/src/page.rs:705-760` — `rematerialize_iframes` (FP-E1, sound)
- `crates/browser/src/iframe.rs:73-238` — `ChildIframe::from_url` (the isolated-jar bug, iframe.rs:173)
- `crates/js_runtime/src/runtime.rs:84-90` — `init_fetch_client` + `FetchState::with_profile` (fresh client/jar)
- `crates/js_runtime/src/extensions/fetch_ext.rs:42-60` — `with_profile` mints `HttpClient::new`; `FETCH_CLIENT` thread-local
- `crates/net/src/lib.rs:1340` — `cookies_for_url` (parent jar read)
- `docs/releases/v0.1.0-parity/07_DATADOME_PRIMITIVES.md`, `docs/vNext/12_R-DATADOME-WASM.md`, `docs/releases/v0.1.0-parity/FAILED_SITES_ANALYSIS.md:13,35,161,200,203,216`
- External: [Hyper Solutions Interstitial](https://docs.hypersolutions.co/datadome/interstitial), [glizzykingdreko Medium](https://medium.com/@glizzykingdreko/breaking-down-datadome-captcha-waf-d7b68cef3e21), [Kameleo 2025 guide](https://kameleo.io/blog/guide-to-bypassing-datadome), [Deobfuscator](https://github.com/glizzykingdreko/Datadome-Interstitial-Deobfuscator), [Encryptor](https://github.com/glizzykingdreko/Datadome-Interstital-Encryptor)
