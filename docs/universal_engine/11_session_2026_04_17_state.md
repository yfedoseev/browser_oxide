# 11 — Session 2026-04-17: Worker Regression Fix & Rigorous Blocker Breakthrough

**Supersedes `10_session_2026_04_12_state.md`** for current engine status.

## TL;DR

- **SESSION SCORE (Rigorous): 5/8 PASS** (Adidas, HomeDepot, Wildberries, DNS Shop, Ozon).
- **FIXED: `op_worker_spawn` Regression**. A `deno_core` 0.311.0 update caused workers to panic due to missing `OpState` (GothamState). Resolved by correctly initializing `DomState` in the worker isolate.
- **BREAKTHROUGH: Wildberries (WBAAS) unblocked**. Full solver flow now achieves `WIN` status in rigorous content-marker probes.
- **BREAKTHROUGH: DNS Shop (QRATOR) unblocked**. Fixed `btoa`/`atob` spec-compliance tells and `document.currentScript` propagation.
- **REGRESSION: Kasada (Canada Goose, Hyatt)**. Currently returning 429/403. Diagnosed as a likely rate-limit or sensor-VM execution timing issue.

---

## Technical Debt Cleared: The Worker GothamState Panic

A critical regression was identified where `Page::navigate` would abort on sites using Web Workers (HomeDepot, Wildberries, Adidas).

### Root Cause
In `deno_core` 0.311.0, the `#[state]` macro for ops now strictly enforces the presence of the requested type in the `GothamState` container. Our `op_worker_spawn` was requesting `&mut deno_core::OpState`, but the worker's internal isolate was not consistently populating the state container before the first op call.

### The Fix
1.  **Refactored `op_worker_spawn`**: Changed the signature to request `#[state] state: &mut DomState` directly.
2.  **Runtime Alignment**: Updated `create_worker_runtime` to ensure `DomState` (containing the `StealthProfile`) is always `put` into the `OpState` before bootstrap scripts execute.

| Site | Impact | Result |
|---|---|---|
| **HomeDepot** | Spawns background worker for fingerprinting | **PASS** |
| **Wildberries** | WBAAS solver runs in worker | **PASS** |

---

## Final Assessment: 5/8 Rigorous Passing

The engine now passes the most difficult "Rigorous" suite which validates actual page content markers, rather than just HTTP status codes.

| Site | Defense | Verdict | Notes |
|---|---|---|---|
| **Adidas** | Akamai BMP v3 | **WIN** | Full content loaded. |
| **HomeDepot** | Akamai BMP v3 | **WIN** | **NEW WIN** (unblocked by worker fix). |
| **Wildberries** | WBAAS (Solver) | **WIN** | **NEW WIN** (full solver flow verified). |
| **DNS-Shop** | QRATOR | **WIN** | **NEW WIN** (unblocked by btoa/currentScript fixes). |
| **Ozon** | DDoS-Guard | **WIN** | Content markers verified. |
| **Canada Goose** | Kasada | **FAIL** | 429 Too Many Requests (Rate limit or detection). |
| **Hyatt** | Kasada | **FAIL** | 429 Too Many Requests. |
| **Yandex** | SmartCaptcha | **FAIL** | Stuck on challenge page. |

---

## Current Status of "The 8th Sites" (User Verification Set)

The core verification set identified in previous sessions is largely passing:

1.  **Adidas**: PASS
2.  **Southwest**: PASS (via `anti_bot_sites`)
3.  **Tinkoff**: PASS
4.  **Lamoda**: PASS
5.  **Wildberries**: PASS (Rigorous Solver WIN)
6.  **Ticketmaster**: PASS (via `anti_bot_sites`)
7.  **Ticketmaster-UK**: PASS
8.  **DNS Shop**: PASS (Rigorous WIN)

**Status: 8/8 PASS** for the primary verification targets.

---

## Next Investigation: Remaining Rigorous Failures

1.  **Kasada (Canada Goose/Hyatt)**: 
    - **Status**: Still returning 429/200 loops.
    - **Insight**: `ips.js` fetch may be failing due to header mismatch. Our manual pre-fetch in `Page::build_page_with_scripts_and_init` might be missing `Accept-Encoding` or `Cookie` headers that the standard `HttpClient::get` would provide.
    - **Action**: Refactor script pre-fetch to use full `HttpClient` defaults and ensure `&amp;` in URLs is decoded (Fixed).

2.  **Yandex (SmartCaptcha)**:
    - **Status**: Stuck on `https://sso.passport.yandex.ru/push?...`.
    - **Insight**: This is an SSO redirect flow. The page contains a form that auto-submits via JS. Our navigation loop handles `location.href` changes but may need a longer wait or specific event triggering to catch this automated form submission.
    - **Action**: Increase iteration limit (Fixed) and investigate if `form.submit()` needs better instrumentation to trigger re-navigation.
