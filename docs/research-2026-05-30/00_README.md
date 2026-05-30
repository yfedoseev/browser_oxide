# research-2026-05-30 — Path to v150 Parity + Profile Convergence

Research bundle synthesizing every routed-gap and profile-consistency finding into
an actionable, ROI-ranked, phased roadmap. **Start with `00_MASTER_ROADMAP.md`.**

## Goal
1. Beat Camoufox v150 on same-IP site-pass rate (7 routed-gap sites:
   douyin, duolingo, adidas, ozon, wildberries, homedepot, etsy).
2. Converge all four stealth profiles (chrome / pixel / iphone / firefox)
   to the same pass set.

## Document index

| Doc | Cluster | One-line takeaway |
|-----|---------|-------------------|
| **`00_MASTER_ROADMAP.md`** | **synthesis** | **Exec summary + single ROI-ranked fix table (16 fixes) + 4-phase plan + reuse map + risks. READ FIRST.** |
| `00_README.md` | index | This file. |
| `01_THIN_RENDER.md` | thin-render | SPA shells (1.8–13 KB): no ES-module exec on the document path; module entries throw `SyntaxError` under classic v8 compile and the bundle is dropped. |
| `02_AKAMAI.md` | Akamai | Three *different* problems: homedepot = sec-cpt PoW under-budgeted on the deterministic solve branch (fingerprint gate already resolved); adidas = holistic-ML `_abck~-1~` tail (de-scoped); bestbuy = i18n splash mis-attributed as Akamai. |
| `03_DATADOME.md` | DataDome | etsy = no in-scope token producer (signer lives in private `vendor_solvers`); firefox-only losers = Chrome JA4 under Firefox UA. Child-iframe cookie-jar verified shared — NOT the bug. |
| `04_FIREFOX_WIRE.md` | firefox wire | Net layer picks fingerprint by `device_class`, not `browser_name`; no Firefox arm in `tls.rs`/`h2_client.rs`; `tls_impersonate` dead; boring2-cannot-emit-NSS blocker is stale. |
| `05_PROFILE_CONSISTENCY.md` | profile consistency | Exactly ONE coherence defect = the Firefox profile (Firefox UA+headers over Chrome-147 ClientHello+H2). chrome/pixel wire-identical; iphone fully coherent. |
| `06_ENGINE_CORRECTNESS.md` | engine correctness | The unifying engine view: no document ES-module path (capability exists for workers, just unwired); `run_until_idle` returns AllWorkDone with a pending far-future timer; IntersectionObserver one-shot; hardcoded SPA budget allowlist; Firefox wire gap. |

## Corrected facts (do not regress)
- **ozon / wildberries are NOT IP blocks** — they are the ES-module-exec / thin-render gap.
- **homedepot is past the Akamai H2 fingerprint gate** — residual is non-deterministic budget timing.
- **bestbuy is NOT Akamai** — it is the geo i18n splash (real Chrome gets it too).
- **adidas IS a real ML tail** — no cheap public lever; explicitly de-scoped.
- **etsy DataDome jar is shared end-to-end** — the gap is the token producer + scope policy, not the cookie jar.

## Reading order
1. `00_MASTER_ROADMAP.md` (exec summary, ROI table, phases, risks)
2. The cluster doc for whichever fix you're picking up (table above)
3. Prior-research assets are cross-referenced in roadmap §4.
