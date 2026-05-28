# vNext — open work after the v0.2.0 fix-burst

This directory tracks deferred items from the v0.2.0 R-FP-AUDIT-2026Q3
session (commits `b5ae11a..b04c187`, May 2026). Each open task gets
its own file with: **why it matters**, **current state** (what's
known + what's done), **next concrete steps**, **sites in scope**,
**effort**, **dependencies**, and **scope** (public engine vs
`vendor_solvers`).

Predecessors and source-of-truth:
- [`../releases/v0.1.0-parity/HANDOFF_v0.2.0_CLOSE_V150_GAP.md`](../releases/v0.1.0-parity/HANDOFF_v0.2.0_CLOSE_V150_GAP.md) — the 12-task plan that drove the 2026-05-27 session.
- [`../releases/v0.1.0-parity/audit/16_DECISION_LOG.md`](../releases/v0.1.0-parity/audit/16_DECISION_LOG.md) — running log of decisions from the audit cycle. Every `R-*` filename here cross-references an `§R-*` section there.
- [`../releases/v0.1.0-parity/audit/15_FIX_PRIORITY_RANKED.md`](../releases/v0.1.0-parity/audit/15_FIX_PRIORITY_RANKED.md) — yield × effort table including all shipped FIX-* and deferred items.
- [`../../CLAUDE.md`](../../CLAUDE.md) + [`../../SCOPE.md`](../../SCOPE.md) — public-engine vs `vendor_solvers` boundary.

## Quick map

| Doc | Cluster | Sites | Effort | Scope |
|-----|---------|------:|--------|-------|
| [01_R-AKAMAI-SECCPT-FLAKE.md](01_R-AKAMAI-SECCPT-FLAKE.md) | Akamai sec-cpt bundle self-solve regression | 1 (homedepot) | 1-3 days oracle + fix | public engine |
| [02_R-SHAREDSESSION-X-COM-COOKIES.md](02_R-SHAREDSESSION-X-COM-COOKIES.md) | Per-Page cookie partitioning | 1 (x-com) | 3-7 days refactor | public engine |
| [03_R-BESTBUY-AKAMAI.md](03_R-BESTBUY-AKAMAI.md) | Akamai BMP sensor_data POST diff | 1 (bestbuy) | 1-2 days trace + 2-5 days fix | public engine |
| [04_R-WBAAS-WILDBERRIES.md](04_R-WBAAS-WILDBERRIES.md) | Wildberries custom antibot + geo | 1 (wildberries) | multi-day | out of scope likely |
| [05_R-SPA-DOUYIN-SIG.md](05_R-SPA-DOUYIN-SIG.md) | `__ac_signature` reverse-engineering | 1 (douyin) | 1-2 weeks open-ended | public engine if tractable |
| [06_R-KASADA-FRONTIER.md](06_R-KASADA-FRONTIER.md) | Holistic ML classifier residual | 3 (canadagoose, hyatt, realtor) | months | `vendor_solvers` |
| [07_FIX-D2-D3-WebGL.md](07_FIX-D2-D3-WebGL.md) | webgl/webgl2 conflation + per-GPU validation | up to 7 (AWS WAF cluster cleanup) | 2-3 days + per-GPU 1d each | public engine |
| [08_FIX-G-canvas-noise.md](08_FIX-G-canvas-noise.md) | Canvas noise detectability decision | 0-3 (cross-vendor) | 1-2 days research + 1 day fix | public engine |
| [09_FIX-E3-profile-pools.md](09_FIX-E3-profile-pools.md) | Linux + Windows + iPhone sampler pools | 0-2 (multi-OS production) | 1-2 days per OS | public engine |
| [10_URL-polyfill-blob.md](10_URL-polyfill-blob.md) | `new URL("blob:…")` returns empty protocol / "null" origin | 0-1 (DataDome iframe + duolingo worker secondary) | 1-2 days | public engine |
| [11_R-AWSWAF-FIX-J-deep.md](11_R-AWSWAF-FIX-J-deep.md) | Behavioural + per-region per-IP signal residuals | 5-7 (AWS WAF cluster final mile) | 1-2 weeks | public engine + behavioural |
| [12_R-DATADOME-WASM.md](12_R-DATADOME-WASM.md) | DataDome WASM daily-key solver | 1-2 (etsy, yelp) | 1-2 weeks | `vendor_solvers` |

## Reading order

If you have 30 minutes and want to know which item to start: read this
file + the audit `15_FIX_PRIORITY_RANKED.md`. The "yield × effort"
column is your tiebreaker.

If you want to **beat Camoufox v150** (115 routed median) — the most
likely flippers are:
1. Land **FIX-D2** + **FIX-D3** (cleanup the WebGL cross-API surface
   for the remaining AWS WAF stub sites) — see [07_FIX-D2-D3-WebGL.md](07_FIX-D2-D3-WebGL.md)
2. Ship the **R-AWSWAF behavioural cluster** ([11_R-AWSWAF-FIX-J-deep.md](11_R-AWSWAF-FIX-J-deep.md))
3. Per-Page cookie partitioning ([02_R-SHAREDSESSION-X-COM-COOKIES.md](02_R-SHAREDSESSION-X-COM-COOKIES.md))
4. Akamai sec-cpt bundle self-solve oracle ([01_R-AKAMAI-SECCPT-FLAKE.md](01_R-AKAMAI-SECCPT-FLAKE.md))

Those four items combined target 7-10 of the 11 Stratum-A sites and
fit a 2-3 week sustained effort.

## Status legend

- ✅ shipped (linked commit)
- 🔵 in progress (current session)
- ⬜ open, not yet started
- ⏸️ paused / deferred with reason
- ❌ dropped (with reason in audit/16)

Every doc starts with a status line. If it says ⏸️ deferred, the
"why deferred" is in the body — that's the bar to clear before
restarting work on it.

## Conventions

- File names start with a 2-digit prefix matching the priority map above.
- Each doc has a fixed structure (§ TL;DR, § Why this matters,
  § Current state, § Next steps, § Sites in scope, § Effort,
  § Dependencies, § Scope).
- Cross-reference the audit log with `audit/16 §<tag>` where
  applicable so the lineage stays connected.
- Don't duplicate code from the audit log here — link to it. This
  directory is the **forward-looking** view; the audit log is the
  **backward-looking** evidence trail.
