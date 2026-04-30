# Kasada `/tl` ct_token investigation — canadagoose.com trace

**Date:** 2026-04-29 (Phase 7 follow-up T2A)
**Test:** `cargo test --release -p browser --test chrome_compat -- --ignored --nocapture kasada_canadagoose_diagnostic`
**Sites that fail:** canadagoose, hyatt, realtor (all 3 use Kasada)
**Symptom in holistic sweep:** `[kasada] no ct_token to inject for <host>`

## Hypothesis tested

Per the T2A plan, three possible root causes:
1. The Kasada `ips.js` JS-VM never calls `/tl` because a Phase 7 secure-context API change makes one of its environment probes throw.
2. `/tl` POST fires but the response is stripped of `x-kpsdk-ct` because our request shape is rejected upstream.
3. `/tl` POST fires and ct_token is in the response, but `kasada_session.rs::learn` misses it (regression).

## Findings

**Root cause: hypothesis #1 — `/tl` is never called.**

The captured net-trace from `kasada_canadagoose_diagnostic` shows three POST destinations:

| URL | Body | Status | Kpsdk headers |
|---|---:|---:|---|
| `https://reporting.cdndex.io/error` | 67555 B | **419** | `{}` |
| `https://www.canadagoose.com/.../r` | 188 B | 200 | `{}` |
| `https://reporting.cdndex.io/error` | 335 B | 200 | `{}` |

No `/tl` URL appears anywhere in the trace. Kasada's `ips.js` ran its bootstrap (`KPSDK state: {now, start, scriptStart}` populated) but bailed to the **error-report path** (`reporting.cdndex.io/error`) instead of completing the `/tl` POW handshake. The first error report was 67 KB — Kasada's JS-VM packs a verbose environment-introspection blob into errors, which is how they detect bot signatures even on the failure path.

The `/r` POST to canadagoose is Kasada's **runtime/report** endpoint (not `/tl`), used for heartbeat/instrumentation. It returns 200 but issues no `x-kpsdk-ct`.

The 419 status on the first `/error` POST is Kasada's own signal that the request shape was rejected at the validator. Combined with the second `/error` returning 200 (Kasada accepts the second-attempt error report), this strongly indicates the JS-VM's environment introspection found an inconsistency, packaged it into an error blob, and the Kasada validator confirmed the inconsistency on round 1.

## What probably triggered the JS-VM to bail

Kasada's `ips.js` runs environment probes against ~50 surface points. The Phase 7 follow-up commits included **two heavy changes** that could trip a probe Kasada didn't previously hit:

1. **Bulk-registered ~498 missing global constructors** as `_illegalCtor` stubs in `interfaces_bootstrap.js` (Phase 7 follow-up `a0027ac`). Some of these stubs are surfaces Kasada probes: e.g., `BrowserCaptureMediaStreamTrack`, `Highlight`, `CSSAnimation`, `Animation`, `Profiler`, `RTCDataChannel`. If Kasada calls `new <Ctor>()` and our stub throws "Illegal constructor" but the **prototype shape** doesn't match Chrome's exact API surface, that's a tell.
2. **`addEventListener` / `removeEventListener` / `dispatchEvent` moved to `Window.prototype`** (Phase 7 follow-up-2 `4be7eb4`). If Kasada checks `Object.getOwnPropertyDescriptor(window, 'addEventListener')` it now sees `undefined` (correct: real Chrome inherits from EventTarget.prototype) — but if any other Phase 7 change did NOT update *its own* `_winAddListener.call(globalThis, ...)` reference path, behavior could diverge.

The 67 KB error blob in the first error POST is the smoking gun — that's Kasada serialising what its JS-VM saw as anomalous. To pinpoint, we'd need to capture and decode that blob.

## Hyatt and realtor

Same Kasada vendor; `kasada_session_store` was empty for all three sites in the holistic sweep, with the same `[kasada] no ct_token to inject for <host>` log. Highly likely the same root cause: JS-VM bails before `/tl` on each.

## Cross-check (deferred)

Per plan: run the same probe via Playwright MCP with **real** Chrome 147, capture canadagoose's network trace, and confirm Chrome's run *does* hit `/tl`. If MCP gets `/tl` but oxide doesn't, the divergence is on our JS-environment side (one of the Phase 7 changes). If neither does, Kasada has changed deployment policy and `/tl` is gated on something else now.

This cross-check is straightforward via the playwright MCP tools but was not run as part of T2A — it's the first step in T2B.

## Recommended follow-up: T2B (deferred)

**Scope:** narrow down which Phase 7 change tripped Kasada's JS-VM probe. Estimated effort: 1–2 days.

Approach:
1. Verify with Playwright MCP that real Chrome 147 hits `/tl` on canadagoose (~15 min).
2. Capture and decode the 67 KB error blob from `reporting.cdndex.io/error` — base64-decode + inflate (Kasada compresses) to read what Kasada's JS-VM detected.
3. Bisect: re-run `kasada_canadagoose_diagnostic` after reverting:
   - `4be7eb4` (EventTarget proto move): if `/tl` returns, root cause = #2 above.
   - `a0027ac` (bulk-register 498 ctors): if `/tl` returns, root cause = #1.
4. If neither revert helps, bisect further across earlier Phase 7 commits.
5. Once located, the fix is either to omit the offending stub or to match Chrome's exact prototype shape on it.

## Risk assessment

- **Sweep regression**: zero. The 3 Kasada sites failed before Phase 7 and still fail after — the holistic sweep score is unchanged at 114/126.
- **Defense-in-depth value of Phase 7 follow-up still holds**: the 661 missing-global registrations and EventTarget cleanup match Chrome's enumeration count and prototype layout for fingerprint scripts that compare counts. Kasada specifically went deeper and probed an interface we got slightly wrong — that's a known-and-bounded fix, not a Phase 7 mistake.
- **Reverting to make Kasada happy** would re-open the `Object.getOwnPropertyNames(window).length` 372 vs 980 gap (a known fingerprint tell). Better to fix forward by identifying the specific stub that Kasada flags.

## Status

T2A complete. T2B started, then **revised** based on cross-checks (next section).

---

## Update 2026-04-29 (T2B started, revised hypothesis)

After approving T2B I cross-checked canadagoose via Playwright MCP and reviewed git history. Two findings invalidated the original "Phase 7 broke Kasada" hypothesis:

### 1. Real Chrome 147 from Playwright MCP gets through canadagoose

Final URL `/ca/en/home-page`, page rendered fully. Every API call carried both:
- `x-kpsdk-ct: 03mRTMXkIlfwlOcna0DA2Rk0oKM5wSlpbrRWKNGb8pS0PcsWbG8Bv9UV7DUzgIeXusg9AmD8GN9uclmWF31gsIn6FIVA6wBuR1w45CNK2H50wl9OxvsQ4ZbMMDGJbrca9b1ZEQGdb5qA7f5Dx6GWsVZY1dkXnjK5ZHoNb16jfsKfkk` (session token)
- `x-kpsdk-cd: {"workTime":1777510852166,"id":"124c57a1e483d45ddeff119c041392af","answers":[10,6],"duration":34.8,"d":-17,"st":1777495262983,"rst":1777495262966}` (PoW solution)

So Kasada **is** active on canadagoose; real Chrome just sails through it.

### 2. The 3 Kasada sites have failed since well before Phase 7

Git history shows the holistic sweep at 114/126 with the same Kasada-CHL trio failing across many earlier checkpoints (`f683e4f` = 98/126; `afaaed3` = HTTP/2 priority fix; etc.). Phase 7 didn't introduce the failure; Phase 7 also didn't fix it.

### Revised diagnosis: edge-classifier divergence, not JS-VM divergence

The split happens **before JS runs**. Two scenarios fit the evidence:

- **(a) Cold-cache visitor**: Real Chrome on this machine has populated KP_UIDz cookies from prior real-user browsing. Kasada's edge sees those and serves the homepage directly. Oxide starts fresh every test → Kasada serves the JS-VM challenge → our JS-VM doesn't pass it (Kasada deobfuscation is closed-source and rotating). The 67 KB error POST is incidental — what Kasada's bootstrap emits when it can't complete the handshake.
- **(b) Residential vs datacenter IP reputation**: same machine, different sockets, possibly different edge-node verdicts. Less likely on a single home network.

Either way, **the JS-VM bailing isn't the root cause** — it's a symptom. Even a perfect JS-VM solver wouldn't help if Kasada's edge already decided to challenge us.

### What would actually help

| Approach | Effort | Likelihood |
|---|---|---|
| Run T1C proxy (`BOXIDE_PROXY=socks5://...residential...`) and re-test the 3 Kasada sites | 5 min ops | **High** — many residential IPs are pre-trusted |
| Persist cookies across runs via `BOXIDE_COOKIE_JAR=path` for the holistic sweep — re-run a few times to build trust | 15 min ops | Medium — slow accumulator |
| Reverse-engineer Kasada's JS-VM and emit a valid `x-kpsdk-cd` PoW solution + correct challenge response | weeks | Required for cold-start without proxies |

### Recommendation

T2B as defined (bisect Phase 7 commits) is unproductive — those commits aren't the root cause. **Replace with T2C: a 5-minute operational test using T1C proxy on a residential IP.** If that flips the 3 Kasada sites to PASS, the cold-start hypothesis is confirmed and the engineering deferral (full JS-VM solver) is the right call. If it doesn't, we have new data narrowing the search.

The Phase 7 follow-up commits (`a0027ac`, `4be7eb4`) are NOT being reverted — they remain net-positive defense-in-depth (probe parity 35.9% → 99.0%, ownPropertyNames 372 → 985).
