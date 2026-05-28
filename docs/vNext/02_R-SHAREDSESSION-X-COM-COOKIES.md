# 02 — R-SHAREDSESSION-X-COM-COOKIES: per-Page cookie partitioning

**Status:** ⏸️ deferred; multi-day refactor. Env-var workaround available.
**Sites in scope:** x-com (1) directly; broader fix improves general
"second-visit poisoning" robustness for any site that issues bot-detection
cookies.
**Effort:** 3-7 days for the per-Page cookie jar refactor.
**Scope:** public engine.

## TL;DR

When BO visits `twitter.com` it redirects to `x.com`; cookies set during
that visit are reused on the NEXT explicit `x.com` nav (correct
process-wide cookie behaviour). x.com's WAF detects the second-visit
shape (fresh nav already carrying issued cookies) and serves a 69-byte
stub instead of the real page. This session **reproduced and confirmed**
the bug: `BROWSER_OXIDE_NO_SHARED_COOKIES=1` flips x-com from 69b →
273KB. The clean fix is per-Page (Chrome-tab-equivalent) cookie
partitioning, not the current process-wide shared jar.

## Why this matters

- This is a CORRECTNESS issue, not just an x-com workaround. Real
  Chrome's cookie model is per-tab (or per-tab-tuple with Storage
  Partitioning), NOT process-wide. Any site that issues a bot-detection
  cookie on visit-1 and rejects it on visit-2 sees BO as a bot.
- Production scrapers that reuse a single BO process across multiple
  unrelated navs hit this constantly (not just x.com).
- Env-var workaround (`BROWSER_OXIDE_NO_SHARED_COOKIES=1`) regresses
  sites that LEGITIMATELY need the cookie carry-forward (yandex,
  microsoft, duckduckgo, homedepot, quora, adidas per VERIFICATION
  §6d-bis). So can't just default-isolate; need the actual partitioning
  model.

## Current state

What's shipped:

- `BROWSER_OXIDE_NO_SHARED_SESSION` / `_NO_SHARED_COOKIES` /
  `_NO_SHARED_ACCEPT_CH` env-var toggles (commit `197a2da`). A
  diagnostic + production-workaround knob.
- Audit `16_DECISION_LOG.md` §R-SHAREDSESSION-X-COM-COOKIES — full
  reproduction:
  | env | twitter | x-com |
  |-----|---------|-------|
  | default | 273921 ✅ | 69 ❌ |
  | NO_SHARED_COOKIES=1 | 273921 ✅ | 273921 ✅ |

Architecture today (`crates/net/src/lib.rs::SharedSession`):

- A single process-wide `Arc<Mutex<CookieJar>>` shared by every
  `HttpClient::shared(profile)` caller.
- `accept_ch` and `dns_cache` also shared via the same singleton.
- `Page::navigate` → `HttpClient::shared` → SharedSession's jar →
  cookies carry from request N to request N+1 across ANY two navs
  in the same process.

## Next steps (3 ranked fix paths)

### Path 1 — Per-`Page` cookie jar (RECOMMENDED, multi-day)

Match Chrome's tab model: each `Page` instance owns its own
`CookieJar`. Two pages opened independently can't see each other's
cookies. Same-`Page` redirects + sub-resource fetches DO share.

Touch points:
- `crates/net/src/lib.rs::SharedSession` — split cookies off; keep
  `accept_ch` shared (DataDome's accept-ch carry-forward is required for
  several legit sites per the env-var A/B).
- `crates/net/src/lib.rs::HttpClient` — gains a per-instance cookie jar.
- `crates/browser/src/page.rs::Page::navigate` — constructs a Page-
  local `HttpClient` instead of `HttpClient::shared`.
- Cookie persistence (`crates/net/src/cookie_jar.rs` if exists) —
  decide what "persist across Pages" means; might require an
  opt-in `BROWSER_OXIDE_PERSIST_COOKIES=1` flag for production
  scrapers that DO want session continuity.

Risk: this is a wide refactor. Tests that relied on cookies leaking
between Pages will break (audit them). Sites that explicitly
need cross-Page session continuity will need a new opt-in path.

### Path 2 — Chrome Storage Partitioning model (THE RIGHT thing, weeks)

Chrome 2024+ partitions cookies by `(top-level eTLD+1, third-party
origin)` tuple. Even within the same Page, third-party cookies are
keyed by the embedding context. This is the long-term correct model
but a much bigger change (~2 week effort).

### Path 3 — eTLD+1 collision band-aid (SMALLEST, 1 day, fragile)

Specifically for `x.com` ↔ `twitter.com` (and any future re-branding
collision): track a per-origin "fresh-session" flag; on first nav to
the origin in a process lifetime, clear all cookies first. Smallest
diff but fragile — any site doing a similar redirect dance breaks the
same way, and we'd add ad-hoc handling per such case.

## Validation

After landing Path 1 or 2:
- Run the same 2-site sweep that reproduced the bug:
  ```
  target/release/examples/sweep_metrics chrome_148_macos \
    <(echo '[{"cat":"social","name":"twitter","url":"https://www.twitter.com/"},{"cat":"social","name":"x-com","url":"https://x.com/"}]') \
    /tmp/xcom_validation.json
  ```
  Expected: both PASS.
- Run the full corpus sweep — check that legitimate carry-forward sites
  (yandex / microsoft / duckduckgo / homedepot / quora / adidas) still
  pass. If any regress, the partitioning was too aggressive; needs the
  Storage Partitioning model instead.

## Dependencies

- No external deps; pure refactor of `crates/net`.
- The diagnostic env-var toggles are already in place for A/B testing
  during development.

## Sources / references

- `crates/net/src/lib.rs::SharedSession` — current shared cookie jar
- `crates/net/src/lib.rs::HttpClient::shared` — env-var toggles (commit `197a2da`)
- `docs/releases/v0.1.0-parity/VERIFICATION.md` §6d-bis — original A/B
- `docs/releases/v0.1.0-parity/audit/16_DECISION_LOG.md` §R-SHAREDSESSION-X-COM-COOKIES — this session's repro
