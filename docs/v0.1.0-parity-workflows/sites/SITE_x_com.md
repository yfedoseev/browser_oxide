# SITE: x.com / twitter.com — cookie-bleed band-aid audit

**Date:** 2026-05-28
**Author:** research agent (x-com deep-dive)
**Branch context:** `fix/v0.1.0-fix4-canvas-parity`
**Verdict in one line:** x-com currently scores **5/5 in the delta harness** and
this is **real** — but it is a **fragile, surface-area-incomplete band-aid**, not
a durable fix. The fix is correct for the *cold* `Page::navigate` path the
benchmark exercises, but it is **bypassed entirely by `PagePool` /
`navigate_warm`** (the recommended production perf path), and it special-cases
exactly one rebrand pair. The durable fix is per-`Page` cookie partitioning
(vNext/02 Path 1), which is also what Camoufox v150 gets for free from Firefox's
`userContextId` model.

---

## 1. What the repo already concluded (cited)

### 1.1 The bug and its reproduction

- **`docs/releases/v0.1.0-parity/FAILED_SITES_ANALYSIS.md` §A.4** (lines
  107–118): x-com shows `THIN-BODY` 69 bytes uniformly across all 4 profiles
  *mid-sweep*, but `L3-RENDERED` 274 KB *in isolation*. "It's the cumulative
  sweep state that breaks it, not the engine." v150 and Patchright pass
  mid-sweep. The row attributes it to "TLS / SharedSession".

- **`docs/releases/v0.1.0-parity/02_GAP_ANALYSIS.md` §10** (lines 218–228):
  original **hypothesis was `accept_ch` bleed**, not cookie bleed — the
  `f62584d` SharedSession rev introduced a process-wide `accept_ch` set that
  picks up `Accept-CH` headers from earlier sites, and "Twitter's WAF heuristic
  eventually rejects the connection." (This hypothesis was later **falsified** —
  see §1.2.)

- **`docs/releases/v0.1.0-parity/VERIFICATION.md` §6d-bis** (lines 304–340):
  the A/B test was run. Results:
  | env | twitter | x-com |
  |-----|---------|-------|
  | default (both shared) | 273921 ✅ | 69 ❌ |
  | `NO_SHARED_SESSION=1` (both isolated) | 273921 ✅ | 273921 ✅ |
  | `NO_SHARED_COOKIES=1` (cookies isolated, accept_ch shared) | 273921 ✅ | 273921 ✅ |

  **Decisive correction:** `NO_SHARED_ACCEPT_CH=1` (accept_ch isolated, cookies
  shared) did **not** fix x-com — so the §10 "accept_ch bleed" theory was wrong.
  It is the **cookie** carry-forward that poisons x-com, not Accept-CH.
  §6d-bis also shows naive global cookie isolation **regresses 3 sites**
  (duckduckgo, microsoft, yandex-ru) plus the docstring-warned set
  (amazon / yandex / homedepot / leboncoin / quora / adidas). So "isolate
  cookies globally" trades +1 for −3 (strict) / up to −8 — a net loss.

### 1.2 The shipped band-aid (commit `383c64a`, Sprint 2.3 Path 3)

- **`docs/vNext/02_R-SHAREDSESSION-X-COM-COOKIES.md`** documents three fix paths:
  - **Path 1** (RECOMMENDED, 3–7 days): per-`Page` cookie jar (Chrome tab model).
  - **Path 2** (THE RIGHT thing, weeks): Chrome 2024+ Storage Partitioning by
    `(top-level eTLD+1, third-party origin)` tuple.
  - **Path 3** (1 day, **explicitly labeled "fragile band-aid"**): eTLD+1
    collision scrub for the `x.com ↔ twitter.com` pair specifically.

  **Path 3 is what shipped.** The ticket header still reads
  "Status: ⏸️ deferred; multi-day refactor" because the *durable* paths (1/2)
  are still open; only the band-aid landed.

- **Commit `383c64a` ("Sprint 2.3 — x.com / twitter.com cookie isolation
  band-aid")** root-caused it precisely: a `twitter.com → x.com` redirect chain
  populates cookie buckets for **both** eTLD+1 identities (379 bytes each, with
  **different `guest_id` values** per identity). On the next explicit `x.com`
  nav, x.com's WAF reads the already-issued `guest_id` + Cloudflare `__cf_bm`
  as a "previously-issued session" and serves the 69-byte stub. Clearing only
  the sister identity is insufficient — x.com's *own* (redirect-populated)
  cookies still poison it, so the band-aid scrubs **both**.

- **`docs/HANDOFF_2026_05_28b.md` §3** (line 60) and the auto-memory
  `state_2026_05_28_delta_headtohead.md` (line 27): trustworthy same-IP delta =
  **x-com BO 5/5 == v150 5/5 → "SOLVED / parity", "Sprint 2.3 band-aid holds."**

**Bottom line of prior work:** the bug is cookie carry-forward (not Accept-CH),
the band-aid flips the delta number, and the durable fix (per-Page jar) is
filed but unbuilt.

---

## 2. New external findings (cited)

External research corroborates the cookie-identity root cause and shows why
v150 doesn't have the problem.

- **x.com is a Cloudflare-WAF-fronted React SPA.** Scrapfly (2026) rates X
  "Hard (4/5)" specifically for **Cloudflare WAF + login wall + aggressive rate
  limiting**, and notes X "migrated from in-house bot detection to Cloudflare's
  Turnstile." The raw HTTP response is "a loading skeleton" — meaningful content
  only appears after JS hydration + authenticated GraphQL. This is exactly BO's
  `THIN-BODY 69` (pre-hydration shell) vs `L3-RENDERED 274 KB` (post-hydration)
  split. Source: https://scrapfly.io/blog/posts/how-to-scrape-twitter ;
  https://webparsers.com/how-to-scrape-x-com-twitter-in-2026/

- **Guest-token / cookie binding to fingerprint + IP (Jan 2025).** Scrapfly
  reports X now binds the guest token to browser fingerprint and validates that
  a token is used from the same IP that requested it. This is the mechanism
  behind the "previously-issued session" rejection: presenting a `guest_id`
  that was minted in a *different* navigation context (the twitter→x redirect)
  reads as a session/context mismatch.
  Source: https://scrapfly.io/blog/posts/how-to-scrape-twitter

- **`__cf_bm` is per-site, short-lived, and HttpOnly/Secure/SameSite=None.**
  Cloudflare docs: "A separate `__cf_bm` cookie is generated for each site… does
  not track users from site to site or session to session," with a ~30-minute
  lifetime. A `__cf_bm` carried forward from an earlier point in the sweep (or
  from the twitter leg of the redirect) is exactly the kind of stale
  bot-management token that fails revalidation on a fresh nav.
  Source: https://developers.cloudflare.com/fundamentals/reference/policies-compliances/cloudflare-cookies/ ;
  https://www.cookie.is/__cf_bm

- **Why Camoufox v150 never hits this (deepwiki `daijro/camoufox`).** Camoufox
  isolates cookies and storage **per browser context** via Firefox's
  `userContextId`, and **does not share cookies across launches or contexts**
  (confirmed by its own tests: cookies set in one context are invisible in
  another; closing the browser drops them; `userDataDir` persistence is
  explicitly "Not supported"). v150 has **no process-wide shared cookie jar at
  all** — so the "second-visit poisoning" class of bug cannot occur. BO's
  `SharedSession` is the architectural outlier here, and the per-`Page`
  partitioning fix (vNext/02 Path 1) simply makes BO match the model the SOTA
  reference already uses.

---

## 3. BO code-level analysis

### 3.1 The shared cookie jar (root cause)

- `crates/net/src/lib.rs:133` — `struct SharedSession { cookies:
  Arc<Mutex<CookieJar>>, accept_ch: …, dns: …, alt_svc: … }`.
- `crates/net/src/lib.rs:140` — `static SHARED_SESSION: OnceLock<SharedSession>`
  — a **single process-wide** instance.
- `crates/net/src/lib.rs:326` `HttpClient::shared(profile)` — every navigate
  call grabs the same `s.cookies` Arc (line 340) unless an env toggle is set
  (lines 333–346). **Default = shared jar across all navs in the process.** This
  is the bug surface: cookies from nav N are visible to nav N+1 regardless of
  origin, the opposite of Chrome's per-tab / Camoufox's per-context model.

### 3.2 The band-aid (commit `383c64a`)

- `crates/net/src/cookies.rs:143` — `CookieJar::clear_for_domain(target) ->
  usize`: evicts every stored-domain bucket that host-suffix-matches `target`
  (via `domain_matches`, `cookies.rs:162`). Unit tests pass — **verified this
  session**: `cargo test -p net --lib clear_for_domain` → 3/3 ok
  (`clear_for_domain_evicts_exact_and_subdomains`,
  `_no_match_returns_zero`, `_ignores_leading_dot_and_case`).
- `crates/net/src/lib.rs:1366` — `HttpClient::clear_cookies_for_domain` async
  wrapper over the jar mutex.
- `crates/browser/src/page.rs:1088–1142` — the actual band-aid, inside
  `Page::navigate_with_init_solvers`. On nav entry, if the URL host is in the
  `x.com` / `twitter.com` family **and** the jar already holds cookies for
  **both** identities (`has_twitter && has_x`, page.rs:1126–1129), it clears
  both buckets (`clear_cookies_for_domain("twitter.com")` +
  `("x.com")`, lines 1130–1131). Opt-out: `BROWSER_OXIDE_NO_XCOM_ISOLATION=1`.

### 3.3 Why this is fragile — four concrete code-level problems

1. **The fix is on the WRONG navigate method for production.** It lives only in
   `navigate_with_init_solvers` (page.rs:1078), which `navigate` /
   `navigate_with_init` / `navigate_pure` / `navigate_humanized` /
   `navigate_with_solvers` all funnel through. **But the `PagePool` perf path
   does NOT.** `PagePool::navigate` (`crates/browser/src/pool.rs:78` →
   `:84 page.navigate_warm(url)`) calls `Page::navigate_warm`
   (`page.rs:1399`), which grabs `HttpClient::shared` at `page.rs:1426` and
   **contains none of the band-aid scrub code**. So any embedder using the
   pool — the documented production model and what `sweep_metrics` uses when
   `BROWSER_OXIDE_SWEEP_POOL=1` (`examples/sweep_metrics.rs:186-188`) — gets the
   *unpatched* 69-byte stub. The delta harness only passes because
   `run_delta_headtohead.py` runs `sweep_metrics` **without** `SWEEP_POOL`, so
   `use_pool=false` (`sweep_metrics.rs:123`) routes to the cold
   `Page::navigate` (`sweep_metrics.rs:202`) — the one path the band-aid covers.
   **The 5/5 is real but path-specific.** A production scraper on the pool
   regresses silently.

2. **Single hard-coded pair.** The collision check is literally `h == "x.com"
   || h.ends_with(".x.com") || h == "twitter.com" || h.ends_with(".twitter.com")`
   (page.rs:1112–1115). Any other rebrand / multi-domain identity that issues a
   session cookie on one host and revalidates on a sibling host (e.g.
   `fb.com`↔`facebook.com`, regional `amazon.*`, future X sub-brands) breaks the
   same way and needs a new hard-coded arm. This is explicitly the
   "fragile band-aid" Path 3 warned about in vNext/02 §Path 3.

3. **The `has_twitter && has_x` gate is brittle.** It uses `cookies_for_url`
   (lib.rs:1340 → `cookies_for`, cookies.rs:70), which applies `secure` and
   `path` filtering. If x.com sets a path-scoped or future SameSite-partitioned
   cookie that `cookies_for` filters out, the probe can return `None` and the
   scrub **silently won't fire** even though a poisoning cookie exists. The
   detection predicate and the eviction predicate (`clear_for_domain`,
   unconditional bucket eviction) are **not symmetric**, so they can disagree.

4. **It's a destructive scrub, not isolation.** The band-aid deletes the
   twitter+x cookies for the whole process. A legitimate workflow that *wants*
   X session continuity (logged-in scraping across navs) loses it on every
   collision-firing nav. Per-`Page` partitioning would preserve same-`Page`
   continuity while isolating across `Page`s — the band-aid cannot.

### 3.4 Is the SharedSession issue "properly fixed or papered over"?

**Papered over.** The underlying process-wide shared `CookieJar`
(`lib.rs:140`) is untouched. The band-aid is a targeted *deletion* keyed on one
domain pair, applied on one code path. It does not change the cookie model; it
patches one symptom of it. The auto-memory framing "SOLVED / parity" is true
*only* for the cold-nav delta harness; it is **not** robust for the pool path or
for any other domain that exhibits second-visit poisoning.

---

## 4. Ranked fix list (ROI order)

> Engine-scope note: all paths below are **public-engine** changes to
> `crates/net` + `crates/browser`. None require `vendor_solvers` — there is no
> per-vendor bypass here; this is cookie-model correctness (CLAUDE.md-clean).

### FIX-X1 — Mirror the band-aid into `navigate_warm` (stop the silent pool regression)
- **What:** lift the `383c64a` collision-scrub block (page.rs:1088–1142) into a
  shared helper (e.g. `fn xcom_collision_scrub(client, url)`) and call it from
  **both** `navigate_with_init_solvers` (page.rs:1085) **and** `navigate_warm`
  (after `HttpClient::shared` at page.rs:1426). Add a regression test that runs
  the pool path against the twitter→x sequence.
- **Effort:** 0.5 day.
- **Expected site impact:** makes x-com robust under `PagePool` (today: silently
  broken). 0 *delta-harness* flips (it's already 5/5 there) but closes a real
  production-correctness hole and is a prerequisite for trusting the 5/5 as
  durable.
- **Confidence:** high. **Public engine.**

### FIX-X2 — Per-`Page` cookie jar (vNext/02 Path 1) — the durable fix
- **What:** each `Page` owns its own `CookieJar`; redirects + sub-resource
  fetches within one `Page` share, but two independently-opened `Page`s do not
  see each other's cookies (Chrome tab model = Camoufox `userContextId` model).
  Keep `accept_ch` shared (VERIFICATION §6d-bis proved isolating *cookies*
  alone fixes x-com; the legit carry-forward sites need cookie history within
  their own Page, which they keep). Touch points (from vNext/02 §Path 1):
  `lib.rs::SharedSession` (split cookies off), `HttpClient` (per-instance jar),
  `page.rs` navigate paths (construct a Page-local client), plus an opt-in
  `BROWSER_OXIDE_PERSIST_COOKIES=1` for scrapers that *want* cross-Page
  continuity.
- **Effort:** 3–7 days.
- **Expected site impact:** x-com durable across **all** nav paths; deletes the
  band-aid entirely; **generalizes** to every future "second-visit poisoning"
  site (the class, not the x.com instance) without per-pair hard-coding. Risk:
  regress sites that currently rely on cookies leaking *between* Pages
  (duckduckgo / microsoft / yandex-ru / amazon / yandex / homedepot / leboncoin
  / quora / adidas per §6d-bis) — but those each navigate within their own Page
  in the sweep, so same-`Page` history is preserved; must re-run the full
  corpus to confirm none regress.
- **Confidence:** high that it fixes x-com durably; medium that it's zero-regression
  on the carry-forward set (needs the full-corpus A/B per vNext/02 Validation).
  **Public engine.**

### FIX-X3 — Generalize the collision scrub to a per-origin "fresh-session on first touch" rule
- **What:** if FIX-X2 is too large for the v0.2.0 window, replace the hard-coded
  twitter/x pair with a general rule: track a per-origin "first-touch-this-
  process" set; on the first nav to an eTLD+1 in a process lifetime, scrub any
  pre-existing cookies for that eTLD+1 *and its known rebrand siblings*. Make
  the predicate symmetric with `clear_for_domain` (drop the path/secure-filtered
  `cookies_for_url` probe; check bucket presence directly).
- **Effort:** 1–1.5 days.
- **Expected site impact:** removes the single-pair brittleness (problem #2/#3
  in §3.3) without the full refactor; still inferior to FIX-X2 because it's
  destructive, not isolating (problem #4 remains).
- **Confidence:** medium. **Public engine.** *Only do this if FIX-X2 slips;
  otherwise FIX-X2 supersedes it.*

### FIX-X4 — Chrome Storage Partitioning model (vNext/02 Path 2) — long-term correctness
- **What:** partition cookies by `(top-level eTLD+1, third-party origin)` tuple,
  matching Chrome 2024+. The most faithful model; subsumes FIX-X2.
- **Effort:** ~2 weeks.
- **Expected site impact:** strictly correct cookie model; best long-term
  fingerprint-parity posture. Low marginal site-flip ROI over FIX-X2 for the
  current corpus — defer until a site is found that needs *intra-Page*
  third-party partitioning.
- **Confidence:** medium. **Public engine.** Lowest ROI now; highest correctness ceiling.

---

## 5. Recommendation

1. **Land FIX-X1 immediately** (0.5 day) — the 5/5 is being reported as
   "SOLVED/parity" while the production pool path is silently unpatched; close
   that gap before the v0.2.0 certification run, or the headline number is
   path-dependent.
2. **Schedule FIX-X2** (per-Page jar) as the durable replacement; it deletes the
   band-aid, matches Camoufox's model, and generalizes the fix. Gate it behind
   the full-corpus A/B (vNext/02 Validation) to confirm the carry-forward set
   (duckduckgo/microsoft/yandex-ru/amazon/…) doesn't regress.
3. **Skip FIX-X3** unless FIX-X2 slips the release window.
4. **Defer FIX-X4** until a corpus site demonstrably needs intra-Page
   third-party partitioning.

---

## Appendix — verification performed this session
- Read commit `383c64a` full diff (band-aid implementation + 3 unit tests).
- Confirmed band-aid lives only in `navigate_with_init_solvers` (page.rs:1088)
  and is absent from `navigate_warm` (page.rs:1399) / `PagePool::navigate`
  (pool.rs:78-86).
- Confirmed delta harness (`benchmarks/run_delta_headtohead.py:93`) invokes
  `sweep_metrics` without `BROWSER_OXIDE_SWEEP_POOL`, so `use_pool=false`
  (sweep_metrics.rs:123) → cold `Page::navigate` (sweep_metrics.rs:202) = the
  band-aid-covered path. Explains the 5/5.
- Ran `cargo test -p net --lib clear_for_domain` → 3/3 pass.
- deepwiki confirmed Camoufox isolates cookies per `userContextId`, no
  process-wide shared jar.
