# 25 — Cloudflare deep dive (the iphone-profile 6-site recovery chapter)

**Status:** planning
**Cluster:** iphone-profile Cloudflare losses — `economist`, `ft`,
`ecosia`, `quora`, `openai`, `udemy` — plus a generalised treatment of
the four Cloudflare anti-bot products that affect us across all four
BO profiles.
**Strategy:** treat the four Cloudflare products as a single
recognise-and-render problem at the engine layer, reuse the three
DataDome primitives (chapter 07), add one Cloudflare-specific
primitive (Turnstile token relay), then close the iphone-profile
fingerprint gap.

---

## TL;DR

iphone is BO's weakest profile (98 / 126 strict-pass) and the leak is
load-bearing on a single vendor — Cloudflare Managed Challenge — that
filters six otherwise-passable sites. Per `11_PER_PROFILE_STRATEGY.md
§2.4`:

| Site | Body on iphone | Other profiles |
|---|---|---|
| `udemy.com` | `Cloudflare-CHL 5929` | passes chrome/pixel/firefox |
| `economist.com` | `Cloudflare-CHL 5869` | passes chrome/pixel/firefox |
| `ft.com` | `Cloudflare-CHL 271064` | passes chrome/pixel/firefox |
| `ecosia.com` | `Cloudflare-CHL 5444` | passes chrome/pixel/firefox |
| `quora.com` | `Cloudflare-CHL 5843` | passes chrome/pixel/firefox |
| `openai.com` | `Cloudflare-CHL 10807` | passes chrome/pixel/firefox |

This chapter:

1. Documents the **four distinct Cloudflare bot products** and how to
   tell them apart (§0).
2. Catalogues **every public recognition marker** (§1) so the engine's
   detection seam is complete — extends today's narrow "9 body markers,
   3 header markers" coverage (per `18_ANTI_BOT_VENDOR_COOKBOOK.md §4`).
3. Walks the **mechanism** of each product so the rendering and retry
   path is unambiguous (§2).
4. Builds a **hypothesis tree** for why iphone-profile specifically
   fails managed-challenge (§3) — the diagnosis must precede the fix.
5. Maps each Cloudflare product onto the **DataDome primitives** from
   chapter 07 (§4) — three of the four products are already covered;
   the new primitive 4 (Turnstile token relay) is Cloudflare-specific.
6. Specifies the **real-Chrome A/B capture** plan (§5) so the
   hypotheses become falsifiable.
7. Catalogues the **open-source bypass research landscape** (§6) so we
   stand on what's already known, and don't lift code we can't ship.
8. Defines a **validation plan** (§7) that's tight on signal and
   doesn't fish in noise (per `docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md`).
9. Draws the **out-of-scope line** to vendor_solvers (§8) and the
   **v0.1.0 acceptance gate** (§9).

After primitives 1+2+3 (already specified in chapter 07) plus the
iphone fingerprint fix (§3) plus the new primitive 4 (§4.4):

| Site | Pre | Post (target) |
|---|---|---|
| economist | Cloudflare-CHL 5869 | L3-RENDERED ≥ 15 KB |
| ft | Cloudflare-CHL 271064 | L3-RENDERED ≥ 15 KB |
| ecosia | Cloudflare-CHL 5444 | L3-RENDERED ≥ 15 KB |
| quora | Cloudflare-CHL 5843 | L3-RENDERED ≥ 15 KB |
| openai | Cloudflare-CHL 10807 | L3-RENDERED ≥ 15 KB |
| udemy | Cloudflare-CHL 5929 | L3-RENDERED ≥ 15 KB |

Routed best-of-4 delta: **+4 sites minimum** (acceptance gate, not
ambitious target). If we land ≥ 5 of 6, iphone-profile gains directly
flow into the routed number because the other profiles already pass
these sites — iphone goes from 98 → 102+, but the **routed gain is the
delta from sites where iphone was the routing fallback** (CHL on
others), measured separately.

---

## 0. Cloudflare's four bot/challenge products

Cloudflare ships four distinct mitigation products that affect the
public engine. They are routinely confused (including in our own
internal notes) because the visible body content overlaps. The
mechanisms, indicators, and required fixes differ. **All four must be
correctly recognised before any of them can be correctly handled.**

### 0.1 — Cloudflare Managed Challenge (the new default)

- **What it is:** Cloudflare's auto-selected challenge type. The
  product picks among an interactive challenge, a non-interactive JS
  PoW, a Turnstile widget, or an outright block based on a per-request
  risk score. Replaced the old "I'm Under Attack" + classic JS
  Challenge as the default in 2022; today is the modal blocker our
  iphone profile hits.
- **Identification:** Response has `cf-mitigated: challenge`
  ([Cloudflare detect-response docs][cf-detect]). Body contains
  `_cf_chl_opt = {…}` (the orchestrator object), plus a `<script
  src="/cdn-cgi/challenge-platform/h/b/orchestrate/...">` reference,
  plus `cf-turnstile` when the widget variant fires
  ([Cloudflare challenges][cf-challenges]).
- **Mechanism:** §2.1 below.
- **Cookie on solve:** `cf_clearance` (~30 min lifetime).
- **BO coverage today:** Body markers caught by `classify.rs:81-156`
  (the `_cf_chl_opt` UNAMBIGUOUS marker at `classify.rs:83` + the
  `cf-turnstile` interactive-cosignal at `classify.rs:148`). The
  `started_as_cf_challenge` boolean is computed at `page.rs:1663` from
  `is_cf_challenge_doc(&html)` and drives the iframe-rematerialize
  poll. Header `cf-mitigated` is **not** logged at `page.rs:1054-1069`
  (the only three logged headers are `x-amzn-waf-action`, `x-datadome`,
  `x-wbaas-token`) — a detection-completeness gap §4.1 of
  `18_ANTI_BOT_VENDOR_COOKBOOK.md` already specs.
- **Fix path:** Primitives 1+2+3 of chapter 07 already engineer the
  rendering capability; for the iphone-profile losses, additionally
  close the fingerprint gap §3.

### 0.2 — Cloudflare Turnstile (the user-facing widget)

- **What it is:** Cloudflare's reCAPTCHA replacement. Renderable as
  `<div class="cf-turnstile" data-sitekey="...">` by any third-party
  site, even non-Cloudflare-proxied sites
  ([Turnstile docs][cf-turnstile-docs]).
- **Identification:** Body contains `cf-turnstile` (the CSS class) or
  `challenges.cloudflare.com/turnstile` (the iframe src) — both
  already in `classify.rs:148-149`. Inside the widget, the host page
  fetches `https://challenges.cloudflare.com/turnstile/v0/api.js` and
  embeds an iframe to `https://challenges.cloudflare.com/cdn-cgi/...`.
- **Mechanism:** §2.2 below.
- **Token on solve:** The widget injects a hidden `<input
  name="cf-turnstile-response" type="hidden" value="<token>">` into the
  parent form ([Cloudflare Turnstile widget configs][cf-turnstile-docs]).
  The site POSTs the form with the token; the site's *server* validates
  via Cloudflare's siteverify API.
- **Distinction from Managed Challenge:** Turnstile is the **widget**;
  Managed Challenge is a **WAF decision** that *may* deploy Turnstile
  as its visible UI. Both exist; they overlap, they're not synonyms.
  Cloudflare's docs say "[the] `cf-mitigated` header set for all
  Challenge Page types" ([detect-response][cf-detect]) — so the
  header is the load-bearing tell, not the body marker.
- **BO coverage today:** Body markers caught; rendering covered by
  Primitive 2 (iframe rematerialize) of chapter 07 when the Turnstile
  is on-page-load. Form-POST Turnstile (the user-driven variant) needs
  Primitive 4 (§4.4).
- **Fix path:** Primitive 4 of §4.4 below.

### 0.3 — Cloudflare Bot Fight Mode

- **What it is:** Per-zone toggle (Pro plan+), described by Cloudflare
  as "a simple, free product that helps detect and mitigate bot
  traffic" ([Bot Fight Mode docs][cf-bfm]). Aggressive: blocks "known
  bot patterns" outright, or issues computationally expensive
  challenges. Distinct from Managed Challenge in two ways: (a) it's a
  *block* not a *challenge* most of the time, (b) it's a per-site
  customer toggle, not a CF-default decision.
- **Identification:** 403 with the Cloudflare-branded HTML; `cf-ray`
  on every response; `cf-mitigated: challenge` **only** when the
  product issues a challenge rather than a hard block. Cloudflare's
  docs say `cf-mitigated` "is set for all Challenge Page types" — so
  a Bot Fight Mode hard block does NOT carry `cf-mitigated: block` or
  any documented header that means "we blocked you"; only `cf-ray` is
  guaranteed.
- **Mechanism:** §2.3 below.
- **BO coverage today:** None specific. A hard-block 403 from BFM is
  classified by body content (likely "Cloudflare Ray ID" + 403
  template); body length will determine if classifier returns
  `Cloudflare-CHL` or `BLOCKED`. There is no engine fix for a hard
  block — only a TLS/UA fingerprint that BFM's heuristics accept.
- **Fix path:** Profile fingerprint quality. If BFM blocks us, the
  ClientHello + UA + IP class together scored bad — fix the
  recognisable bad signal (§3 hypothesis tree).

### 0.4 — Cloudflare JS Challenge (legacy)

- **What it is:** The pre-Managed-Challenge classic. Serves a HTML
  page titled "Just a moment..." with an obfuscated JS PoW that
  resolves after ~5 s and sets `cf_clearance`. Cloudflare-blog-named
  "IUAM" (I'm Under Attack Mode) — partially superseded by Managed
  Challenge but still seen on older sites and on `Under Attack`-mode
  zones.
- **Identification:** 503 status, body contains `cf-browser-verification`
  (already UNAMBIGUOUS at `classify.rs:82`) or `Just a moment...` (the
  PHRASE-gated `"just a moment"` at `classify.rs:92`). Body also
  contains `<form id="challenge-form" action="/cdn-cgi/l/chk_jschl"
  method="POST">` with hidden fields `jschl_vc`, `jschl_answer`,
  `pass`.
- **Mechanism:** §2.1 below (shared orchestrator path).
- **Cookie on solve:** `cf_clearance`.
- **BO coverage today:** Body markers caught. The JS-challenge
  computation must complete in our V8 within the 90 s poll deadline
  (`page.rs:1973`) for the cookie to land. Most non-WASM JS challenges
  do, per the pre-strip Phase 5 history. The remaining gap is the
  same `started_as_cf_challenge` poll + cookie-delta retry plumbing
  Primitive 3 of chapter 07 already proposes.

[cf-detect]: https://developers.cloudflare.com/cloudflare-challenges/challenge-types/challenge-pages/detect-response/
[cf-challenges]: https://developers.cloudflare.com/fundamentals/security/cloudflare-challenges/
[cf-turnstile-docs]: https://developers.cloudflare.com/turnstile/get-started/client-side-rendering/widget-configurations/
[cf-bfm]: https://developers.cloudflare.com/bots/get-started/bot-fight-mode/

---

## 1. Recognition markers — every public Cloudflare tell

This section extends `18_ANTI_BOT_VENDOR_COOKBOOK.md §2.5` (which is
intentionally short — one-paragraph summaries). The classifier and the
header-logger in the engine must be able to recognise each marker so
the right rendering path is chosen.

### 1.1 Response headers (precision-first)

| Header | When emitted | Precision | Engine today |
|---|---|---|---|
| `cf-ray: <16hex>-<airport>` | Every Cloudflare-proxied response (passes AND challenges) | Indicator (presence ⇒ CF, not ⇒ challenge) | Not logged |
| `cf-mitigated: challenge` | EVERY challenge-page response (Managed, JS, Turnstile, BFM-with-challenge) | High (single canonical "challenged" signal — per [Cloudflare detect-response docs][cf-detect]) | Not logged at `page.rs:1054-1069` — gap |
| `server: cloudflare` | Most CF-proxied responses (some plans strip it) | Indicator (combine with status) | Not logged |
| `cf-cache-status: <STATE>` | CF cache info on passes (HIT/MISS/DYNAMIC/BYPASS/REVALIDATED) | Indicator (presence ⇒ CF; absence ⇒ probably challenge or non-CF) | Not logged |
| `cf-apo-via` | CF Automatic Platform Optimization (WordPress) | Low | Not logged |
| `cf-edge-cache` | CF edge-cache metadata header | Low | Not logged |
| `cf-bgj` | CF Browser Insights / Google Analytics | Low | Not logged |

The single load-bearing engine miss: **`cf-mitigated: challenge` is
the single canonical challenge signal** per Cloudflare's own docs, and
the engine doesn't log or branch on it. The fix is the 1-line addition
to the header logger at `page.rs:1054-1069` already specified in
`18_ANTI_BOT_VENDOR_COOKBOOK.md §4.1`. Quoted here for completeness:

```rust
// page.rs ~1069 — add to the vendor-detect log block
if let Some(v) = resp.headers.get("cf-mitigated") {
    eprintln!("[vendor-detect] cloudflare-mitigated {} on {}", v, resp.url);
}
if let Some(v) = resp.headers.get("cf-ray") {
    // log conditionally — cf-ray fires on every CF response including passes,
    // so only emit when status is challenge-shaped (403/429/498/503)
    if matches!(resp.status, 403 | 429 | 498 | 503) {
        eprintln!("[vendor-detect] cloudflare-ray {} status={} on {}",
                  v, resp.status, resp.url);
    }
}
```

Detection-only; no flow change. Pure observation. Drives post-sweep
analysis to split CHL outcomes by Cloudflare-vs-other.

### 1.2 Body markers (already partially covered)

Already in `classify.rs:81-156`:

| Marker | Const | Tier |
|---|---|---|
| `_cf_chl_opt` | `UNAMBIGUOUS` at `classify.rs:83` | any size ⇒ Cloudflare-CHL |
| `cf-browser-verification` | `UNAMBIGUOUS` at `classify.rs:82` | any size ⇒ Cloudflare-CHL |
| `just a moment` | `PHRASE` at `classify.rs:92` | small body only |
| `checking your browser` | `PHRASE` at `classify.rs:93` | small body only |
| `cf-turnstile` | `INTERACTIVE_CAPTCHA_COSIGNAL` at `classify.rs:148` | upgrades bare `captcha` ⇒ captcha-CHL |
| `challenges.cloudflare.com/turnstile` | `INTERACTIVE_CAPTCHA_COSIGNAL` at `classify.rs:149` | same |

Not yet in classifier (extend per §4 of `18_ANTI_BOT_VENDOR_COOKBOOK.md`):

| Marker | Source | Why it matters |
|---|---|---|
| `cf-challenge-running` | Pre-2022 IUAM body literal | Some legacy Cloudflare sites still serve this |
| `Cloudflare Ray ID:` (body, plus 16hex) | The Cloudflare-branded error page | Hard-block / 1020 access-denied template — different from a challenge |
| `/cdn-cgi/challenge-platform/h/g/orchestrate/jsch/v1` | The JS-challenge orchestrator URL — per [Cloudflare community thread][cf-jschl-comm] | Disambiguates the JS-challenge variant from Managed |
| `/cdn-cgi/l/chk_jschl` | The legacy chk_jschl form action | Same |
| `jschl_vc` / `jschl_answer` | Hidden form inputs in the JS challenge | Same |
| `/cdn-cgi/challenge-platform/h/b/jsd/...` | Managed Challenge orchestrator JS URL | Disambiguates Managed from JS-challenge |
| `chl_page` | `/cdn-cgi/challenge-platform/h/g/orchestrate/chl_page/v1` — the JS-Detections variant URL | Per [Cloudflare JS Detections docs][cf-jsd] |
| `Sorry, you have been blocked` | Error 1020 (Access Denied via WAF custom rule) | Hard block, not a challenge — different fix path |
| `Error 1015` | Rate-limited by CF | Different fix (back off + slow down), not a stealth/render gap |
| `Error 1006/1007/1008` | IP banned at the CF edge | Out of engine scope — IP rotation |

[cf-jschl-comm]: https://community.cloudflare.com/t/js-challenge-changes-get-request-to-post/300387
[cf-jsd]: https://developers.cloudflare.com/cloudflare-challenges/challenge-types/javascript-detections/

### 1.3 Status codes

| Status | Body shape | Meaning |
|---|---|---|
| `200` | > 50 KB | Real page (passed CF). |
| `200` | 1-15 KB + body markers | Managed Challenge orchestrator shell that ran but never cleared. **Classified `Cloudflare-CHL` `ChallengeIncomplete`** at `classify.rs:189`. |
| `403` | 1-3 KB + Ray ID body | Hard block (BFM or 1020). |
| `403` | > 15 KB | Interactive Turnstile widget. |
| `429` | any | Rate-limited (Error 1015 or burst guard). |
| `503` | < 5 KB | Classic JS Challenge ("Just a moment..."). |
| `503` | > 50 KB | Likely real upstream outage — verify by re-pulling later. |

The navigate loop at `page.rs:1045` treats `403/429/498` identically
for the initial-challenge logging — extend to `503` to ensure JS
Challenge bodies surface in the same diagnostic stream.

### 1.4 Cookies

| Cookie | Lifetime | Meaning |
|---|---|---|
| `cf_clearance` | ~30 min default | The clearance — required to access CF-protected origin after any challenge. |
| `__cf_bm` | session (~30 min) | Bot-management behavioural session token. NOT a clearance — its presence does NOT mean we passed. |
| `cf_chl_2` | brief (~5 min) | Mid-challenge state cookie. |
| `cf_chl_seq` | brief | Per-challenge sequence number. |
| `__cflb` | session | CF load-balancer affinity. Benign. |
| `cf_use_ob` | brief | Origin-Bound: indicates direct-to-origin should be used (rare). |

The clearance predicate that `07_DATADOME_PRIMITIVES.md §Primitive 3`
specs (`cookies_carry_anti_bot_clearance`) already includes
`cf_clearance=` — verify after the change ships that the predicate
also short-circuits the CF poll early-exit, not only DataDome's.

### 1.5 iframe src

When the challenge or Turnstile fires, an iframe is appended whose src
is on `challenges.cloudflare.com`:

```
https://challenges.cloudflare.com/turnstile/v0/api/<key>/light/normal/auto/...
https://challenges.cloudflare.com/cdn-cgi/challenge-platform/h/g/turnstile/if/ov2/...
```

This is the iframe that `rematerialize_iframes` (`page.rs:649-704`)
must fetch and run for the challenge to clear. Today it's gated by
`started_as_cf_challenge` (already body-marker-based) so this path
fires — confirm the gate stays right after the §4 primitives land.

---

## 2. Mechanism deep dive per product

### 2.1 Managed Challenge & JS Challenge (shared orchestrator path)

Both share the `/cdn-cgi/challenge-platform/...` orchestrator URL
infrastructure. The 2026 modal pattern (Managed Challenge):

1. **Initial request** → response `503` (JS Challenge) or `200` (Managed
   Challenge orchestrator shell) + body containing:
   ```html
   <script>
     window._cf_chl_opt = {
       cvId: '3',
       cZone: 'example.com',
       cType: 'managed',
       cRay: '<16hex>',
       cH: '<hash>',
       cFPWv: 'g',
       cITimeS: '<unix>',
       cTplV: 5,
       cTplB: 'cf',
       cRq: { ru: ..., ra: ..., rm: 'GET', d: ..., t: ..., m: ..., i1: ..., i2: ..., zh: ..., uh: ..., hh: ... }
     };
   </script>
   <script src="/cdn-cgi/challenge-platform/h/b/orchestrate/chl_page/v1?ray=<16hex>"></script>
   ```
2. The orchestrator script (`orchestrate/chl_page/v1?ray=<ray>`) is
   fetched from CF; it's a per-zone customised JS bundle. Quote from
   [CaptchaAI walkthrough][captcha-cf-flow]: "The 'initial challenge'
   script is accessed via GET to `/cdn-cgi/challenge-platform/h/g/orchestrate/jsch/v1?ray=<rayID>`,
   where the ray ID is extracted from the initial page response."
3. The bundle reads `_cf_chl_opt`, fingerprints the browser
   (canvas, audio, WebGL, fonts, navigator.permissions surface, timing
   precision, `performance.now()` quantisation, WebAssembly subtle
   differences, hardware concurrency, MIME types, etc.).
4. Depending on the WAF risk score, it either:
   - **Silent path:** computes a JS / WASM PoW (a hash-equality search
     vs a difficulty target), POSTs the answer to a CF endpoint, gets
     `cf_clearance` set, reloads.
   - **Turnstile path:** injects a Turnstile widget iframe (§2.2) and
     waits for callback before reloading.
   - **Block path:** returns to the user with an interactive CAPTCHA or
     an outright deny.
5. Reload of the original URL with `cf_clearance` cookie present →
   real content served.

The classic legacy JS Challenge (`cf-browser-verification`) uses
`/cdn-cgi/l/chk_jschl` form POST instead of the orchestrator — but the
end state (`cf_clearance` set, reload) is identical.

**What BO needs to do** (matches chapter 07 primitives, no
Cloudflare-specific logic required):

| Step | Primitive | Status |
|---|---|---|
| Recognise the response shape | `is_challenge_document_response` per chapter 07 P1 | Spec'd; not yet wired (today's `is_cf_challenge_doc` at `classify.rs:247-251` is the body-only narrower predicate) |
| Relax CSP so orchestrator JS can load `/cdn-cgi/challenge-platform/...` from CF host | Chapter 07 P1 (CSP relax) | Spec'd; not yet wired |
| Materialise the Turnstile iframe (when injected) | Chapter 07 P2 (rematerialize_iframes) | Already wired at `page.rs:1992-1994`; gated by `started_as_cf_challenge` which fires from body marker `classify.rs:247-251` |
| Wait for `cf_clearance` cookie write | Chapter 07 P3 (cookies_carry_anti_bot_clearance) | Spec'd; the cookie name is already in chapter 07 P3's pattern list |
| Re-fetch the original URL | Chapter 07 P3 (cookie-delta retry) | Already wired at `page.rs:2125-2185`; gated by `started_as_cf_challenge` |

[captcha-cf-flow]: https://blog.captchaai.com/cloudflare-challenge-session-flow-walkthrough

### 2.2 Turnstile widget (third-party-embeddable)

Distinct from Managed Challenge — Turnstile is a **widget that any
site can embed**, even non-Cloudflare-proxied sites. Per [Cloudflare
widget docs][cf-turnstile-docs]:

1. Site embeds:
   ```html
   <script src="https://challenges.cloudflare.com/turnstile/v0/api.js" async defer></script>
   <form action="/submit" method="POST">
     <div class="cf-turnstile"
          data-sitekey="0x4AAAAAAAxxxxxxxx"
          data-callback="onTurnstileSuccess"></div>
     <button type="submit">Submit</button>
   </form>
   ```
2. The `api.js` scans for `<div class="cf-turnstile">` elements and
   renders an iframe pointing at `challenges.cloudflare.com`.
3. The iframe runs the same fingerprinting + PoW infrastructure as
   Managed Challenge.
4. On success, the widget:
   - Injects a hidden input `<input type="hidden" name="cf-turnstile-response" value="<token>">` into the parent form (auto-mode).
   - Fires the JS callback `onTurnstileSuccess(token)` if registered.
   - The token has 300 s lifetime, single-use.
5. The site's form POST carries `cf-turnstile-response=<token>` to its
   own backend, which validates the token via Cloudflare's
   `siteverify` API.

**Two variants in the wild that drive the BO failure mode split:**

- **Implicit-render auto-mode** — fires on page load, no user
  interaction. The widget invokes the success callback the moment it
  has a token. Our P2 rematerialize_iframes path handles this — when
  the cf-turnstile iframe fetches and runs, the widget self-completes
  and the form (if any) becomes submittable / the callback fires. If
  the parent page only needs the token in `document.cookie` or the
  hidden form input, no further action.
- **Form-POST mode** — the site requires the user (or our engine) to
  call `e.requestSubmit()` after the token lands. This is the case
  where Primitive 4 (§4.4) is required: we must execute the parent
  form's submit() programmatically after the Turnstile callback fires,
  OR our engine must auto-submit on `cf-turnstile-response` input
  appearance.

### 2.3 Bot Fight Mode (hard block path)

Per [Cloudflare BFM docs][cf-bfm], BFM "identifies traffic matching
patterns of known bots and issues computationally expensive challenges
that force the requesting client to perform CPU-intensive
calculations." But the per-zone product also has a hard-block path:

1. Request lands; CF runs heuristics on ClientHello (TLS fingerprint),
   UA, IP class, behavioural history.
2. If the score crosses a threshold, CF returns 403 + Cloudflare-branded
   HTML with `Cloudflare Ray ID: <16hex>` in the body. There is NO
   challenge to solve in this path — only a hard block.
3. Some BFM-on-Challenge variants drop the response back through the
   Managed Challenge path; in those cases, `cf-mitigated: challenge`
   appears and §2.1 applies.

**Why this matters for the iphone-profile losses:** if any of the 6
sites are returning hard blocks (no `cf-mitigated: challenge` header,
just 403 + Ray ID body), then there is nothing the engine can
"render" to fix it — the fix is fingerprint quality (§3). Distinguish
via the §1.1 `cf-mitigated` log.

### 2.4 Subproducts comparison

| Aspect | Managed Challenge | Turnstile widget | Bot Fight Mode | JS Challenge (legacy) |
|---|---|---|---|---|
| Owner | CF WAF decision | Site embeds | Per-zone toggle | CF WAF decision |
| Trigger | Risk score | Site's own form | Per-zone toggle + heuristics | Risk score (older sites) |
| Visible UI | Often none; sometimes Turnstile | Always widget | Block page | "Just a moment..." page |
| Clearance | `cf_clearance` | `cf-turnstile-response` token | None (hard block) or `cf_clearance` (challenged variant) | `cf_clearance` |
| Header | `cf-mitigated: challenge` | None (widget-side) | `cf-mitigated: challenge` (challenged variant), else just `cf-ray` | `cf-mitigated: challenge` |
| Status | 200 (orch shell) or 403 | 200 | 403 (block) or 503 (challenged) | 503 |
| Body marker | `_cf_chl_opt` | `cf-turnstile` + `challenges.cloudflare.com/turnstile` | `Cloudflare Ray ID:` + 403 template | `cf-browser-verification`, `Just a moment...` |
| BO P1+P2+P3 covers | Yes | Yes (if implicit-render) | No (no challenge to render) | Yes |
| Needs primitive 4 | No (cookie-based reload) | Yes (form-POST mode) | No | No |
| Needs fingerprint fix | If iphone hits it (§3) | Same | Always (block ⇒ score) | If iphone hits it |

---

## 3. Why iphone profile fails the 6 Cloudflare sites

Per `11_PER_PROFILE_STRATEGY.md §5.3` ("iphone — the specialist"):

> Cloudflare's risk model weights `cf-bm` cookie + JA4 + UA combination.
> iOS Safari produces a JA4 that's "Safari on mobile" — a much smaller
> real-traffic share than Chrome desktop, so the model has fewer
> confident-real samples to anchor on, and falls back to challenge.
> BO's iOS profile has distinct TLS (`safari_18_ios` codename, distinct
> ciphers/sigalgs/curves per `tls.rs:111-183`) — so the JA4 is
> plausibly real. The problem is the *combination* (JA4 + UA + no UA-CH
> + traffic source) doesn't match a CF-trusted-class.

That paragraph names a syndrome, not a root cause. This section
hypothesises root causes in priority order and specifies the debug
step that falsifies each.

### Hypothesis tree (most likely first)

#### H1 — iOS Safari class is "elevated risk" by default

**Claim:** Cloudflare's risk model classifies all iOS Safari traffic as
slightly elevated risk relative to Chrome-desktop, irrespective of
whether the fingerprint is "real". The 6 sites we lose are those
where the zone's challenge threshold sits between "real iOS Safari"
and "Chrome desktop class". Real iOS Safari hitting the same sites
from the same IP class would *also* get the challenge.

This is the **null hypothesis** — it's the "we're operating correctly,
the environment is just harder for this profile" interpretation.

**Falsification step:**

```bash
# Capture real-iOS-Safari traffic to each of the 6 sites from the
# SAME source IP (this datacenter), via a real iPhone tethered to
# this network. Or via BrowserStack iOS device on a known-clean IP.
#
# For each site:
#   - Does real iOS Safari get a challenge too?
#   - If yes ⇒ H1 confirmed; nothing engine-side to fix.
#   - If no  ⇒ H1 falsified; move to H2.
#
# Practical proxy if no real iPhone available: capture from
# Mobile Safari on macOS via the Develop > User Agent > iOS Safari
# menu, with macOS Safari's "Develop > Enter Responsive Design Mode"
# at iPhone 15 Pro dimensions. This captures real Safari TLS +
# real Safari UA-CH (which is "none emitted"), only the JA4 will be
# macOS-Safari not iOS-Safari (acceptable approximation; CF's JA4
# database has both classes).
```

**Likely fix location if confirmed:** None engine-side. Accept that
iphone profile is a specialist; route around per chapter 11 §4.3.

#### H2 — iOS UA + UA-CH combo triggers a heuristic rule

**Claim:** Cloudflare runs a heuristic that flags requests when the UA
claims `iPhone/iOS` but the request omits Sec-CH-UA headers in a
specific way (e.g. wrong order, presence/absence of Sec-CH-UA-Mobile
vs Sec-CH-UA-Platform, or some pixel-tracking telemetry like Save-Data
or Viewport-Width sec-ch fields). Real iOS Safari omits UA-CH entirely
(per `11_PER_PROFILE_STRATEGY.md §1.3`); our profile correctly
emits no UA-CH (verified at preset construction). But there may be
*other* headers that CF expects for iOS-class traffic that we omit.

**Falsification step:**

```bash
# Capture the exact request headers our iphone profile sends to one
# of the 6 sites:
RUST_LOG=net=trace target/release/examples/sweep_metrics \
    iphone_15_pro_safari_18 /tmp/cf_corpus.json /tmp/out.json \
    2>&1 | tee /tmp/iphone_cf.log
grep "Request headers" /tmp/iphone_cf.log | head -20

# Then capture real iOS Safari → economist.com on the same IP class
# (via tethered iPhone + tcpdump on the laptop's interface):
# - Compare every request header byte-by-byte
# - Compare every TLS extension order
# - Compare every cipher (already verified in `tls.rs:111-148`)
# - Compare every sigalg (already verified in `tls.rs:137-148`)

# Any header present on real iOS Safari but NOT in our request ⇒
# candidate root cause. Test: synthesise that header in our preset,
# re-run sweep, observe pass/fail.
```

**Likely fix location if confirmed:** `crates/net/src/headers.rs` (add
the missing header to the iOS Safari branch) or
`crates/stealth/src/presets.rs:795-875` (the iphone preset).

#### H3 — WebGL renderer "Apple GPU" is recognised as headless

**Claim:** Real iOS Safari reports `Apple GPU` as the WebGL renderer
(literal string — Apple strips per-model identification per
`11_PER_PROFILE_STRATEGY.md §1.3`). Our iphone preset does the same
(verified at `presets.rs:795-875`). But the *combination* of
`Apple GPU` + missing WEBGL_debug_renderer_info extension support +
missing real WebGL2 extension list specific to A17 Pro GPU + missing
EXT_color_buffer_float behaviour might be the tell.

**Falsification step:**

```bash
# Run a creepjs-style probe in BO's iphone profile and compare to
# real iOS Safari's output.
#
# 1. Add /tmp/probe.html: instrument every WebGL+Canvas+Audio call
#    a CF orchestrator might call (per the orchestrator URL hashes).
# 2. Load via BO iphone vs real iOS Safari (BrowserStack OK).
# 3. Diff outputs.

# If WebGL extension list differs in any line ⇒ H3 candidate
# confirmation. Check whether the gap is on the specific extensions
# that creepjs detects as "missing on real iOS Safari but present
# in our impl" (e.g. EXT_disjoint_timer_query that mobile Safari
# blocks but desktop Safari has).
```

**Likely fix location if confirmed:** `crates/canvas/src/webgl_render.rs`
(extension list per profile) + `crates/stealth/src/gpu.rs` (GpuProfile
overrides).

#### H4 — TLS impersonate `safari_18_ios` has a mismatch

**Claim:** `crates/net/src/tls.rs:107-183` implements the iOS Safari
ClientHello — distinct ciphers, sigalgs, curves, fixed extension
permutation. Per chapter 23 (`23_TLS_HTTP_FINGERPRINT_REFERENCE.md`),
the iOS Safari reference is `lexiforest/curl-impersonate
safari_18.0_iOS.yaml`. If any single byte differs from real iOS Safari
148+ (Apple's iOS 18 ships with mid-cycle TLS updates), CF's JA4
database flags us.

**Falsification step:**

```bash
# Capture our iphone-profile ClientHello bytes:
TLS_DEBUG=1 target/release/examples/sweep_metrics \
    iphone_15_pro_safari_18 /tmp/cf_corpus.json /tmp/out.json \
    2>&1 | tee /tmp/iphone_tls.log

# Compare against:
# - lexiforest/curl-impersonate safari_18.X_iOS.yaml (refresh source)
# - A live capture from a real iPhone running iOS 18.X (tcpdump)
# - The JA4 fingerprint via wireshark + tshark

# Any byte-level diff ⇒ likely root cause. Most likely candidates:
# - extension order (our `tls.rs:169-183` is FIXED — verify against
#   the current real-iOS capture, since Apple changes order across
#   point releases)
# - cipher count / order
# - sigalgs (the duplicated rsa_pss_rsae_sha384 bug per `tls.rs:137`
#   — verify it's still present in iOS 18.X)
# - GREASE values & positions
```

**Likely fix location if confirmed:** `crates/net/src/tls.rs:107-183`.
See chapter 23 for the playbook on safari_18 TLS evolution.

### Priority order for v0.1.0

| Hypothesis | Confidence prior | Falsification cost | Order |
|---|---|---|---|
| H1 (null) | 30% | high (need real iPhone on this IP) | 4th |
| H2 (header mismatch) | 30% | low (header capture is free) | 1st |
| H4 (TLS mismatch) | 25% | medium (need wireshark capture) | 2nd |
| H3 (WebGL fingerprint) | 15% | medium (need creepjs probe) | 3rd |

Start with H2 because it's the cheapest falsify-or-confirm. If H2
turns up nothing, run H4. If H4 turns up nothing, run H3. If all three
turn up nothing, H1 is the residual — accept the routing-loss and
update chapter 11 §4.3 to permanently document it as out-of-scope.

---

## 4. Restore-as-primitives plan (Cloudflare lens on chapter 07)

DataDome primitives in `07_DATADOME_PRIMITIVES.md` are deliberately
generic (function names contain no vendor identifier). Three of the
four primitives already cover the Cloudflare path; this section
re-states the overlap so the implementation order is clear, then adds
the one Cloudflare-specific primitive (token relay).

### 4.1 Primitive 1 reused — relax_response_csp on challenge docs

**Chapter 07 spec:** `is_challenge_document_response(status, headers,
body)` (new function in `classify.rs`). One of its conditions is
"response carries a known anti-bot signaller header" with
`cf-mitigated` already in the list (`07_DATADOME_PRIMITIVES.md §131-160`
and `18_ANTI_BOT_VENDOR_COOKBOOK.md §4.3`).

**Cloudflare-specific impact:**

- Cloudflare challenge interstitials use the origin's own CSP, which
  typically forbids `script-src` from external hosts. The orchestrator
  bundle is loaded from `/cdn-cgi/challenge-platform/...` on the same
  origin (relative URL), but the Turnstile iframe loads from
  `challenges.cloudflare.com` cross-origin — requires `frame-src
  challenges.cloudflare.com` which most origins do not whitelist.
- Cloudflare's own docs state: "Content Security Policy needs to
  ensure that anything under `/cdn-cgi/challenge-platform/` is
  allowed" (per the [JavaScript Detections docs][cf-jsd]).
- With P1's relax-CSP-for-challenge-docs primitive, BO's CSP enforcer
  (`crates/js_runtime/src/extensions/fetch_ext.rs::set_csp_policy`)
  skips installation on these responses, the orchestrator runs, and
  Turnstile iframes are not pre-emptively blocked.

**No new code required for Cloudflare** — Primitive 1 covers it.

### 4.2 Primitive 2 reused — cross-origin challenge iframe materialization

**Chapter 07 spec:** un-gate `rematerialize_iframes` (`page.rs:649-704`)
via a generic `started_as_challenge_doc` flag computed from Primitive 1.

**Cloudflare-specific impact:**

- The Turnstile iframe is loaded from
  `https://challenges.cloudflare.com/turnstile/v0/api/<key>/.../...`.
  Today the iframe is materialised when `started_as_cf_challenge`
  fires from `is_cf_challenge_doc(&html)` body marker
  (`classify.rs:247-251`). That covers Managed Challenge orchestrator
  shells (`_cf_chl_opt` present) but NOT third-party-embedded
  Turnstile widgets where the body has `cf-turnstile` but lacks
  `_cf_chl_opt`.
- P2's generalisation widens the gate to include any
  `is_challenge_document_response`-true response, which covers
  third-party Turnstile too.

**Verification once P2 lands** (single-site capture):

```bash
# Synthetic Turnstile harness — a static HTML page that embeds Turnstile:
cat > /tmp/ts_harness.html <<'HTML'
<!DOCTYPE html>
<html><body>
  <script src="https://challenges.cloudflare.com/turnstile/v0/api.js"></script>
  <form id="f"><div class="cf-turnstile" data-sitekey="3x00000000000000000000FF"></div></form>
</body></html>
HTML
# (data-sitekey 3x00000000000000000000FF is Cloudflare's "always pass" test key.)

# Verify the iframe gets fetched + executed by BO:
RUST_LOG=browser=debug target/release/examples/sweep_metrics \
    iphone_15_pro_safari_18 /tmp/ts_corpus.json /tmp/out.json
grep -E "challenges.cloudflare.com|turnstile-response" /tmp/iphone_ts.log
```

### 4.3 Primitive 3 reused — solved-cookie retry

**Chapter 07 spec:** `cookies_carry_anti_bot_clearance(cookies)` —
closed-list cookie names including `cf_clearance=`.

**Cloudflare-specific impact:** Already covered. After the orchestrator
solves the JS PoW / fingerprint check, CF sets
`Set-Cookie: cf_clearance=...`, the predicate fires, the page-content
"is no longer a challenge" guard checks `is_cf_challenge_doc` is now
false, and the cookie-delta retry at `page.rs:2125-2185` re-fetches
the original URL with the clearance cookie.

**No new code required** — Primitive 3 covers it.

**One verification step to do post-implementation:**

After Primitive 3 lands, capture the cookie jar at the moment the
poll early-exit fires for a Cloudflare site. Confirm `cf_clearance=`
is present AND the body is no longer a CF challenge doc. Without the
body guard, we'd risk an early exit on a `__cf_bm` write (the
behavioural session cookie that CF sets even on failed challenges).

### 4.4 NEW Primitive 4 — Turnstile token relay

This is the one Cloudflare-specific primitive. The other three
primitives cover the "cookie-clearance" path (Managed Challenge, JS
Challenge, BFM-with-challenge) where `cf_clearance` is the unblock.
Turnstile-form-POST mode is different: the unblock is a *token in a
form field*, and the engine must submit the form for the unblock to
take effect.

**Two sub-cases:**

#### 4.4a — Turnstile auto-mode (on-page-load implicit render)

The widget self-completes on load, fires the registered callback,
which the site has wired to e.g. enable a submit button or auto-call
`form.requestSubmit()`. Our existing `dom_bootstrap.js:1098-1110`
form-submit handler (already gives us submit() + requestSubmit() per
chapter 02 §1) wires through. So this sub-case needs no extra code if
Primitives 1+2 are in place — the iframe gets fetched, runs, fires
the callback, the site's own JS submits the form.

**No new code required IF chapter 02 §1 reddit fix lands** (it ensures
requestSubmit reaches submit() reaches the navigate loop's
__pendingNavigation).

#### 4.4b — Turnstile manual-mode (form-POST waiting on user)

The widget completes, the hidden `<input name="cf-turnstile-response">`
is populated, but the form is NOT auto-submitted — it waits for the
user to click a submit button. Two ways to handle this in an
engine-generic way:

**Option A — Auto-submit on cf-turnstile-response appearance** (recommended):

When the engine's `MutationObserver`-equivalent or the
`rematerialize_iframes` post-tick check detects that a
`<input name="cf-turnstile-response">` field has been populated in any
form on the page, automatically call that form's submit() (already in
`dom_bootstrap.js:1098-1110`).

```rust
// page.rs poll body, after rematerialize_iframes:
let turnstile_check = r#"
    (function () {
        const inputs = document.querySelectorAll(
            'input[name="cf-turnstile-response"]'
        );
        for (const inp of inputs) {
            if (inp.value && inp.value.length > 10 && inp.form) {
                // Heuristic: only auto-submit if no other "explicit
                // submit handler" is registered (avoid double-submit).
                // For v0.1.0, just submit unconditionally.
                inp.form.requestSubmit
                    ? inp.form.requestSubmit()
                    : inp.form.submit();
                return inp.form.action || location.href;
            }
        }
        return '';
    })()
"#;
let submitted_to: String = page.event_loop()
    .execute_script(turnstile_check)
    .unwrap_or_default();
if !submitted_to.is_empty() {
    // PENDING_NAV_JS will pick up the submission on the next tick
    // (auto-submit() sets __pendingNavigation per dom_bootstrap.js:1107).
}
```

This is **engine-generic**: the function name is `auto_submit_on_token_input`
and the marker `cf-turnstile-response` is data, not code identity. The
same primitive would auto-submit on `g-recaptcha-response`,
`h-captcha-response`, etc. (a future hook for Recaptcha v2 / hCaptcha
form-POST sites — left out of v0.1.0 scope but the architecture
permits it).

**Option B — Document-only "manual" Turnstile is out of scope**:

If the form genuinely requires user interaction (click), there is no
non-controversial automation path in a headless engine. Treat as out
of scope. The 6 sites in §3 do not appear to use the manual variant
(they're all WAF-deployed Managed Challenge, not site-embedded
Turnstile form-protection); Option B suffices for v0.1.0 acceptance.

**Recommendation:** Implement Option A for v0.1.0 because it costs ~30
lines of code and unblocks any future site that wires Turnstile to
form-protect a login/signup. Restrict to single-input-per-form to
avoid double-submit; gate on `is_challenge_document_response` so it
only runs when the engine already believes we're on a challenge poll.

### 4.5 Naming discipline

Per chapter 07's naming rules (generic names, vendor-identifier-free):

| Code identifier | Notes |
|---|---|
| `is_challenge_document_response` | Already chapter 07 P1. Vendor-name-free. |
| `cookies_carry_anti_bot_clearance` | Already chapter 07 P3. Closed-list cookie names (`cf_clearance=` is one of seven). |
| `auto_submit_on_token_input` | **New for primitive 4.** Implementation matches `cf-turnstile-response`, `g-recaptcha-response`, `h-captcha-response` (closed-list). Vendor-name-free. |

The bypass list at `page.rs:2066-2080` (selective CSP bypass for
walmart/canadagoose/hyatt/realtor/footlocker/ticketmaster/udemy)
should NOT be extended to add the 6 iphone-Cloudflare sites — that's
a per-host hack; the generic primitive should subsume it. Once §3
fingerprint fix + §4 primitives ship, udemy should fall out of that
list (or stay only on the chance some other vector needs it).

---

## 5. Real-Chrome A/B reference (capture spec)

The hypotheses in §3 are falsified by side-by-side capture per
`04_TOOLING_SPEC.md`. This section is the Cloudflare-specific
extension of that spec.

### 5.1 Single-site capture

For each of the 6 sites, capture twice (BO iphone + Camoufox iphone
profile + real iOS Safari if available):

```bash
CF_SITES='economist.com ft.com ecosia.com quora.com openai.com udemy.com'

cat > /tmp/cf_corpus.json <<JSON
[
  {"cat":"news","name":"economist","url":"https://www.economist.com/"},
  {"cat":"news","name":"ft","url":"https://www.ft.com/"},
  {"cat":"search","name":"ecosia","url":"https://www.ecosia.org/"},
  {"cat":"social","name":"quora","url":"https://www.quora.com/"},
  {"cat":"tech","name":"openai","url":"https://www.openai.com/"},
  {"cat":"misc","name":"udemy","url":"https://www.udemy.com/"}
]
JSON

# BO capture, per profile, with full fetch + cookie + script-error logging
for site in $(jq -r '.[].name' /tmp/cf_corpus.json); do
  target/release/examples/sweep_metrics iphone_15_pro_safari_18 \
      /tmp/cf_corpus.json /tmp/bo_iphone_${site}.json --capture $site
done

# Camoufox iOS-Safari-profile capture (per chapter 04 §3)
for site in $(jq -r '.[].name' /tmp/cf_corpus.json); do
  python /tmp/cam_capture.py --profile ios_safari $site --har /tmp/cam_${site}.har
done
```

### 5.2 What to compare

For each site, line up these artifacts side-by-side:

| Artifact | Where to find | What to look for |
|---|---|---|
| Request headers (initial GET) | BO `/tmp/capture/.../fetches.json[0]`; Camoufox HAR `entries[0].request.headers` | Missing / extra / mis-ordered headers (H2 falsify) |
| Response headers | Same files | `cf-mitigated: challenge` presence/absence; `cf-ray` value; `cf-cache-status` (passes only); `set-cookie cf_clearance` (passes only) |
| Initial body | `body.html` | Body length, `_cf_chl_opt` presence, `cf-turnstile` presence |
| Orchestrator JS fetch | `fetches.json` later entries | URL like `/cdn-cgi/challenge-platform/h/g/orchestrate/...`. BO should fetch it after rematerialize. |
| Turnstile iframe fetch | Same | URL like `challenges.cloudflare.com/turnstile/v0/...`. BO should fetch it after P2. |
| `cf_clearance` Set-Cookie | `cookie_writes.json` | Camoufox shows it; BO at present doesn't. After P1+P2+P3, BO should match. |
| Re-fetched real content | `fetches.json` last entry | Body > 50 KB and no `_cf_chl_opt` ⇒ passed. |
| Script errors | `script_errors.json` | Look for "Refused to execute script" (CSP violation — falsify P1 effectiveness), "iframe blocked" (falsify P2). |
| ClientHello bytes | Wireshark .pcap via tcpdump | JA4 string — H4 falsification |

### 5.3 The H4 capture

ClientHello bytes are not in BO's fetch log; they're below the HTTP
layer. Capture via:

```bash
# Run the sweep under tcpdump:
sudo tcpdump -i any -w /tmp/bo_iphone_cf.pcap port 443 &
TCPDUMP_PID=$!
sleep 1
target/release/examples/sweep_metrics iphone_15_pro_safari_18 \
    /tmp/cf_corpus.json /tmp/bo_iphone_cf.json
sleep 2
sudo kill $TCPDUMP_PID

# Compute JA4 per site:
for site_host in www.economist.com www.ft.com www.ecosia.org \
                www.quora.com www.openai.com www.udemy.com; do
  echo "=== $site_host ==="
  tshark -r /tmp/bo_iphone_cf.pcap -Y "tls.handshake.type==1 && \
        tls.handshake.extensions_server_name==\"$site_host\"" \
        -T fields -e tls.handshake.ja4 -c 1
done

# Compare to real iOS Safari ja4 (BrowserStack OK, or local iPhone).
# Diff each.
```

If the JA4 differs ⇒ H4 confirmed; chapter 23 has the playbook.
If JA4 matches ⇒ H4 falsified; the gap is layered above TLS.

---

## 6. Open-source Cloudflare bypass research

This section catalogues what the public open-source ecosystem has
done. The point is *informational* (so we don't rediscover known
mechanisms) and *boundary-marking* (we don't lift code into the
public engine per `CLAUDE.md`).

### 6.1 Active projects (status as of 2026 Q2)

| Project | URL | Approach | Covers | Status 2026 | Reusable? |
|---|---|---|---|---|---|
| [`VeNoMouS/cloudscraper`](https://github.com/VeNoMouS/cloudscraper) | github.com/VeNoMouS/cloudscraper | JS execution via js2py/Node, integrates with paid CAPTCHA-solver APIs | JS challenge v1/v2/v3 + Turnstile (via 2captcha/CapSolver/anticaptcha) | v3.0.0 (Jun 2025); 6.5k stars; actively maintained but largely "obsolete for bypassing modern Cloudflare" per [ZenRows 2026][zen-2026] | No — MIT-licensed but its approach (paid solver integration) is out of scope per `CLAUDE.md`. Read for protocol details. |
| [`sarperavci/CloudflareBypassForScraping`](https://github.com/sarperavci/CloudflareBypassForScraping) | (same) | "Request mirroring" — runs a local bypass server that forwards requests through and returns `cf_clearance` | cf_clearance generation, SSL/TLS fingerprint | Active 2026 | No — runs a separate browser instance under the hood, would be a circular dependency for BO. Read for mechanism. |
| [`FlareSolverr/FlareSolverr`](https://github.com/FlareSolverr/FlareSolverr) | (same) | Selenium + undetected-chromedriver proxy server | JS challenge, Managed Challenge (basic) | Maintained but "55-70% pass on tests" per [ScrapeOps][scrapeops] | No — Selenium + undetected-chromedriver dependency; not LICENSE-compatible (some sub-deps unclear) |
| Byparr | not yet on github topic search (Camoufox-backed FlareSolverr replacement) | Camoufox + same FlareSolverr API | Turnstile (claimed highest pass rate) | Active; recommended for Turnstile per [Scrapfly Turnstile guide][scrapfly-turnstile] | No — Camoufox dep; not in-process |
| [`ultrafunkamsterdam/undetected-chromedriver`](https://github.com/ultrafunkamsterdam/undetected-chromedriver) | (same) | Patched ChromeDriver to defeat headless detection | Indirect (used by FlareSolverr, cloudscraper) | Largely community-maintained | No — Chrome/Chromedriver dep |
| [`g1879/DrissionPage`](https://github.com/g1879/DrissionPage) | (same) | Python lib combining requests + browser automation; cross-mode session sharing | Cloudflare passes by impersonating browser TLS / sharing browser cookies | Active | No — Chinese-developer project, Chrome dep |
| [`vvanglro/cf-clearance`](https://github.com/vvanglro/cf-clearance) | (same) | Playwright-based cf_clearance harvester | cf_clearance specifically | Reference | No — Playwright dep. Read for "same IP same UA when reusing cookie" rule. |
| [`x404xx/Turnstile-Solver`](https://github.com/x404xx/Turnstile-Solver) | (same) | REST API for Turnstile solve | Turnstile widget | Active | No |
| [`scaredos/cfresearch`](https://github.com/scaredos/cfresearch) | (same) | Reverse-engineering research on CF challenge protocols | All CF products | Reference (no solver code) | **Yes** — protocol-level descriptions are research material, not code |
| [Hyper-Solutions](https://github.com/Hyper-Solutions) | (commercial) | Commercial SDK | All major vendors | Commercial | No |

### 6.2 What's reusable

**For BO's public engine** (per `CLAUDE.md` licensing + scope):

- **Protocol knowledge** from `scaredos/cfresearch` — read to understand
  the orchestrator URL structure (`/cdn-cgi/challenge-platform/h/{g,b}/orchestrate/{chl_page,jsch}/v1?ray=...`)
  so the engine's body markers / vendor-detect logger covers
  every variant.
- **Header-emission rules** from `cloudscraper` source — the project
  meticulously matches Chrome's request header surface; we already do
  this in `crates/net/src/headers.rs` but it's a useful cross-check
  reference for `Accept`, `Accept-Language`, `Accept-Encoding`
  ordering, `Sec-Fetch-*` family per nav type.
- **Cookie reuse semantics** from `vvanglro/cf-clearance` — the
  documented "must reuse same IP + same UA when reusing the `cf_clearance`
  cookie" rule. We already preserve cookies across iter via the
  cookie jar; verify we also preserve UA + don't switch IPs mid-nav
  (we don't — single HttpClient per nav).

**For BO's private `vendor_solvers` crate** (per `CLAUDE.md` scope —
solver code lives there if/when we ship one):

- Image-classification models from `2captcha/anti-captcha/CapSolver`
  community projects (these are paid integrations, off the table for
  the public engine).
- JS interpreter logic from `cloudscraper` — not needed since BO
  already runs a real V8.

**Off the table entirely:**

- Lifting code from any of the above into public crates. Read for
  understanding, do not copy. License + scope rules in `CLAUDE.md`.

[zen-2026]: https://www.zenrows.com/blog/bypass-cloudflare
[scrapeops]: https://scrapeops.io/web-scraping-playbook/how-to-bypass-cloudflare/
[scrapfly-turnstile]: https://scrapfly.io/blog/posts/how-to-bypass-cloudflare-turnstile

### 6.3 Independent walkthroughs and research

- [Cloudflare Challenge Page Session Flow walkthrough — CaptchaAI blog][captcha-cf-flow] — best-in-class walkthrough of the chl_page orchestrator URL family + JS PoW flow
- [How to build a JSD solver to bypass Cloudflare 2026 — RoundProxies](https://roundproxies.com/blog/jsd-solver-cloudflare/) — JS-Detections (JSD, the lightweight CF cookie-set probe) deep dive
- [Bypassing Cloudflare's "bot fight mode" — Codeforces blog](https://codeforces.com/blog/entry/134322) — first-person account of writing a JS interpreter to clear BFM challenges; reads as the canonical "what's the actual PoW" article
- [ZenRows 2026 Cloudflare bypass guide][zen-2026] — ecosystem state of the art as of 2026 Q1
- [Scrapfly 2026 Cloudflare guide](https://scrapfly.io/blog/posts/how-to-bypass-cloudflare-anti-scraping)
- [Scrape.do 2026 bypass guide](https://scrape.do/blog/bypass-cloudflare/) — comparison table that surfaces Byparr's leadership on Turnstile pass rate

---

## 7. Validation plan

After Primitives 1+2+3 land (per chapter 07) + Primitive 4 (§4.4) +
iphone fingerprint fix (§3 — H2 / H4 / H3 in priority order):

### 7.1 Acceptance gate

Run iphone profile against the 6-site Cloudflare corpus (§5.1 above):

```bash
cat > /tmp/cf_corpus.json <<'JSON'
[
  {"cat":"news","name":"economist","url":"https://www.economist.com/"},
  {"cat":"news","name":"ft","url":"https://www.ft.com/"},
  {"cat":"search","name":"ecosia","url":"https://www.ecosia.org/"},
  {"cat":"social","name":"quora","url":"https://www.quora.com/"},
  {"cat":"tech","name":"openai","url":"https://www.openai.com/"},
  {"cat":"misc","name":"udemy","url":"https://www.udemy.com/"}
]
JSON

# Run iphone profile, 3 times for stability per
# 03_BENCHMARK_METHODOLOGY.md
cargo build --release --workspace
for run in 1 2 3; do
  target/release/examples/sweep_metrics iphone_15_pro_safari_18 \
      /tmp/cf_corpus.json /tmp/iphone_cf_run${run}.json
done

# Aggregate: count L3-RENDERED with len ≥ 15000 (strict pass) per site
python3 - <<'PY'
import json
from collections import defaultdict
counts = defaultdict(int)
for run in (1, 2, 3):
    for entry in json.load(open(f'/tmp/iphone_cf_run{run}.json')):
        if entry['tag'] == 'L3-RENDERED' and entry['len'] >= 15000:
            counts[entry['name']] += 1
for site, c in sorted(counts.items()):
    print(f"{site:12s} {c}/3 passes")
PY
```

**Acceptance:** ≥ 4 of 6 sites pass strict in ≥ 2 of 3 runs.

The relaxed bar (4/6, not 6/6) absorbs the H1 null-hypothesis residual
— if Cloudflare's risk model genuinely categorises iOS Safari as
elevated risk, some of the 6 will not flip even with a perfect
fingerprint, and we should not gate the release on that.

### 7.2 Regression gate (the full 126-corpus)

After every primitive lands:

```bash
cargo test --workspace -- --test-threads=1
cargo test --workspace --test holistic_sweep -- --test-threads=1
cargo clippy --workspace
cargo fmt --all -- --check

# Full sweep — 4 profiles, 126 sites:
benchmarks/run_full_sweep.sh
```

**Acceptance:** zero new BLOCKED, zero new THIN-BODY, max ±5 sites
variance per `docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md`. Re-run 3× and
aggregate per `03_BENCHMARK_METHODOLOGY.md`.

### 7.3 Routed-best-of-4 impact

Even if iphone flips all 6 sites to pass, the **routed** count gain
is whatever subset of those 6 was previously CHL on at least one
other profile. Per `11_PER_PROFILE_STRATEGY.md §2.4`, all 6 are
already passing on chrome+pixel+firefox — so iphone passing them adds
0 to routed-best-of-4 (they're already counted in the 108).

**Where the routed gain comes from:** new sites that **only** iphone
can flip after this work — none in the 6 above, but the H2 / H4
fingerprint fixes might also unlock sites currently failing on ALL
four profiles where iOS class is the right fingerprint. Candidates
(per `02_GAP_ANALYSIS.md` 18-residual list): unclear in advance;
re-measure after the fix.

**Honest framing:** the primary v0.1.0 value of this chapter is
**iphone parity** (single-profile count: 98 → 102+) which is a
defensible quality claim independent of routed-best-of-4. The routed
number gain is a possible side-benefit, not a hard target.

### 7.4 Per-product subset validation

After detection markers extend (§1.1 + §4 of `18_ANTI_BOT_VENDOR_COOKBOOK.md`):

- Verify `[vendor-detect] cloudflare-mitigated challenge` appears in
  the per-site log for at least 5 of the 6 sites (the sixth may be
  a hard block ⇒ `cf-mitigated` absent, only `cf-ray` present, and
  H1 confirmed for that site)
- Verify `[vendor-detect] cloudflare-ray ...` appears with status
  403/503 on hard-block paths
- Verify `cf_clearance=` Set-Cookie writes appear in `cookie_writes.json`
  on the post-P1+P2+P3 runs

### 7.5 What "done" looks like

| Metric | Pre | Post (target) |
|---|---|---|
| iphone strict pass | 98 | ≥ 102 |
| iphone Cloudflare CHL count | 6 | ≤ 2 |
| Engine `cf-mitigated` header logged | No | Yes |
| Engine `cf-ray` (on 4xx/5xx) logged | No | Yes |
| `is_challenge_document_response` exists | No | Yes (chapter 07 P1) |
| `cookies_carry_anti_bot_clearance` exists | No | Yes (chapter 07 P3) |
| `auto_submit_on_token_input` exists | No | Yes (this chapter P4) |
| Full 126-corpus regressions | n/a | 0 |
| Routed best-of-4 | 108 | ≥ 108 (no regression; possible upside) |

---

## 8. Out of scope — what stays in `vendor_solvers`

Per `CLAUDE.md`:

> Per-vendor challenge solving is out of scope here. The engine
> exposes a `browser::ChallengeSolver` trait + `Page::navigate_with_solvers`
> hook; the concrete Akamai/Kasada/DataDome/Cloudflare implementations
> live in the private `vendor_solvers` companion crate.

Cloudflare-specific items that stay private:

### 8.1 Turnstile widget solving (image classification)

When Turnstile presents an interactive image-grid challenge (~1% of
loads, per [Cloudflare blog research][cf-turnstile-research]), the
puzzle requires image classification. That's solver-level, paid-API
territory (CapSolver/2captcha/anti-captcha integration). Not in the
public engine.

[cf-turnstile-research]: https://blog.cloudflare.com/turnstile-private-captcha-alternative/

### 8.2 Browser-as-a-Service Cloudflare bypass

Third-party services (ScrapingBee, ZenRows, Bright Data, Apify) offer
Cloudflare-bypass APIs. Integration with these is an application
concern (caller-side), not engine-side. The engine provides the
`ChallengeSolver` trait surface (`crates/browser/src/challenge.rs:55-161`)
that an integration can implement; the integration code stays private.

### 8.3 cf_clearance cookie harvesting

A cookie-harvesting service is a separate operational concern (rotate
cookies across IPs, refresh on expiry, share across requests). The
engine's HttpClient already supports cookie injection per nav; the
harvesting orchestration belongs in the consumer / `vendor_solvers`.

### 8.4 JSD solver-by-Rust

A pure-Rust solver for the JS-Detections variant (the simplest CF
PoW — just a hash-search) is conceptually small enough to write but
falls in scope only if measured to be load-bearing. Per the §3
hypothesis tree, the iphone losses are diagnosed as fingerprint /
header gaps, not as JSD-PoW failures (we have V8; the PoW runs in V8).
If H1-H4 falsify and a JSD-PoW gap is the actual diagnosis (engine
runs the PoW but the answer is rejected — unlikely), revisit.

---

## 9. Acceptance for v0.1.0

- [ ] **All 4 Cloudflare products documented** with mechanism + markers
      (this chapter §0 + §2)
- [ ] **iphone profile recovers ≥ 4 of 6 Cloudflare-managed-challenge sites**
      per §7.1 acceptance gate
- [ ] **Detection markers extended** in `page.rs:1054-1069` to include
      `cf-mitigated`, `cf-ray` (status-gated), per §1.1
- [ ] **`is_challenge_document_response`** function lands in
      `classify.rs` per chapter 07 P1 (covers all 4 Cloudflare products
      via the `cf-mitigated` header arm AND the `cf-ray + 4xx/5xx` shape)
- [ ] **`cookies_carry_anti_bot_clearance`** function lands in
      `classify.rs` per chapter 07 P3 (already covers `cf_clearance`
      verbatim)
- [ ] **`auto_submit_on_token_input`** function lands per §4.4 (handles
      `cf-turnstile-response`, hooked into the challenge-poll loop)
- [ ] **iphone fingerprint hypothesis tree** (§3) has at least one of
      H2/H4 falsified-or-confirmed via the capture spec in §5; if both
      falsified, H3 captured next; if all three falsified, H1 documented
      in chapter 11 §4.3 as permanent specialist-routing-tax
- [ ] **No regression on the 88-site universal-pass set** (per
      `11_PER_PROFILE_STRATEGY.md §3.1`) — chapter 07 primitives are
      narrowly-gated; verify via the full sweep
- [ ] **Cross-link bidirectionally** to chapter 07 (primitives), 11
      (profile strategy), 18 (vendor cookbook), 23 (TLS reference), 02
      (gap analysis), 04 (capture tooling)

---

## 10. Files referenced

### BO source code

- `crates/browser/src/page.rs:1045-1069` — initial-challenge header
  logger (currently 3 vendors; this chapter §1.1 adds cf-mitigated +
  cf-ray-status-gated)
- `crates/browser/src/page.rs:1054-1057` — vendor-detect log block
- `crates/browser/src/page.rs:1062` — `[vendor-detect] aws-waf` log
  line (pattern to follow for cf-mitigated)
- `crates/browser/src/page.rs:1064` — `[vendor-detect] datadome` log
  line (pattern)
- `crates/browser/src/page.rs:1067` — `[vendor-detect] wbaas` log line
  (pattern)
- `crates/browser/src/page.rs:1663` — `started_as_cf_challenge =
  classify::is_cf_challenge_doc(&html)` (the body-marker boolean today)
- `crates/browser/src/page.rs:1967-1972` — challenge-poll entry gate
  (one of the 3 booleans is `started_as_cf_challenge`)
- `crates/browser/src/page.rs:1979-1994` — `rematerialize_iframes` call
  site (covers Cloudflare iframes too — the comment at line 1981-1988
  references `challenges.cloudflare.com`)
- `crates/browser/src/page.rs:2066-2080` — selective CSP bypass list
  including `udemy.com` (extends the §3 fingerprint hypothesis; consider
  removing udemy from this list after primitives land)
- `crates/browser/src/page.rs:2125-2185` — cookie-delta retry block
  (`started_as_cf_challenge` is one of the gates; chapter 07 P3 extends)
- `crates/browser/src/page.rs:2281-2293` — `v8_html_is_real` guard
  including `!v8_html.contains("/cdn-cgi/challenge-platform/")` at
  line 2293 (already in place)
- `crates/browser/src/page.rs:649-704` — `rematerialize_iframes` impl
  (the engine seam that fetches Turnstile iframes)
- `crates/browser/src/classify.rs:74-86` — UNAMBIGUOUS table including
  `_cf_chl_opt` (line 83) and `cf-browser-verification` (line 82)
- `crates/browser/src/classify.rs:91-97` — PHRASE table including
  `just a moment` (line 92) and `checking your browser` (line 93)
- `crates/browser/src/classify.rs:144-156` — INTERACTIVE_CAPTCHA_COSIGNAL
  table including `cf-turnstile` (line 148) and
  `challenges.cloudflare.com/turnstile` (line 149)
- `crates/browser/src/classify.rs:183-192` — verdict_for() rule including
  the `Cloudflare-CHL` + large body ⇒ `ChallengeIncomplete` split (line 189)
- `crates/browser/src/classify.rs:247-251` — `is_cf_challenge_doc(body)`
  (the body-only predicate; chapter 07 P1 generalises to status + headers)
- `crates/browser/src/challenge.rs:55-161` — `ChallengeSolver` trait
  surface (private `vendor_solvers` binds here; line 58 comment names
  `"cloudflare-managed"` and `"cloudflare-turnstile"` as kinds)
- `crates/browser/src/iframe.rs:255` — `find_iframes` (the build-time
  walker; the rematerialize companion is `page.rs:649`)
- `crates/stealth/src/presets.rs:795-875` — iphone preset (H3 fix
  location if WebGL gap is confirmed)
- `crates/net/src/tls.rs:107-148` — iOS Safari cipher list + sigalgs
  (H4 fix location)
- `crates/net/src/tls.rs:152-157` — iOS Safari curves
- `crates/net/src/tls.rs:169-183` — iOS Safari extension permutation
  (FIXED order — verify against current real iOS 18.X capture)
- `crates/net/src/headers.rs:143` — Sec-CH-UA generation entry (H2 fix
  location if a missing header is the gap)
- `crates/js_runtime/src/extensions/fetch_ext.rs` — CSP enforcer
  (`set_csp_policy` / `clear_csp_policy`); Primitive 1 of chapter 07
  toggles via this

### Sibling docs (cross-link both ways)

- `docs/releases/v0.1.0-parity/02_GAP_ANALYSIS.md` — the 10 recoverable
  + 8 hard-residual sites; Cloudflare cluster touched but not detailed
- `docs/releases/v0.1.0-parity/03_BENCHMARK_METHODOLOGY.md` — corpus +
  classifier rules + 3-run aggregation (the §7 validation procedure)
- `docs/releases/v0.1.0-parity/04_TOOLING_SPEC.md` — capture-mode tooling
  (the `--capture <name>` flag the §5 A/B capture uses)
- `docs/releases/v0.1.0-parity/05_SPA_HYDRATION_CLUSTER.md` — reddit /
  duolingo / booking / douyin; the duolingo Worker / recaptcha analysis
  is adjacent to Cloudflare's Turnstile worker patterns
- `docs/releases/v0.1.0-parity/07_DATADOME_PRIMITIVES.md` — chapter 07
  Primitives 1/2/3 that this chapter reuses for Cloudflare
- `docs/releases/v0.1.0-parity/08_KASADA_FRONTIER.md` — the open
  frontier (not Cloudflare-related but the methodology section is the
  template for §3's hypothesis tree)
- `docs/releases/v0.1.0-parity/11_PER_PROFILE_STRATEGY.md` — iphone is
  the specialist; §2.4 lists the 6 sites this chapter recovers, §5.3
  documents the syndrome this chapter diagnoses
- `docs/releases/v0.1.0-parity/13_FILE_LOCATIONS_INDEX.md` — file:line
  lookup for everything (cross-check the §10 list when files move)
- `docs/releases/v0.1.0-parity/14_TESTING_VALIDATION.md` — drift gate
  (the §7.2 regression criteria refer to L5 there)
- `docs/releases/v0.1.0-parity/15_OPEN_QUESTIONS.md` — research backlog
  (file H1 / H4 follow-ups here if they need more time)
- `docs/releases/v0.1.0-parity/16_STEALTH_FINGERPRINT_AUDIT.md` — the
  signal-level audit; §3 H2 / H3 fixes integrate with its findings
- `docs/releases/v0.1.0-parity/18_ANTI_BOT_VENDOR_COOKBOOK.md` — §2.5
  Cloudflare summary (this chapter is its expansion); §4.1 / §4.2 /
  §4.3 detection-extension spec applies directly
- `docs/releases/v0.1.0-parity/23_TLS_HTTP_FINGERPRINT_REFERENCE.md` —
  the safari_18_ios TLS branch reference (H4 fix path)
- `docs/releases/v0.1.0-parity/24_RISK_REGISTER.md` — log Cloudflare
  rotation-cadence risk (weekly orchestrator URL update; quarterly
  challenge-protocol shifts) per `18_ANTI_BOT_VENDOR_COOKBOOK.md §5.1`

### Memory (auto-context — read for prior-art continuity)

- `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/state_2026_05_16_phase5_datadome.md`
  — Phase 5 DataDome history (set the iframe-rematerialize + cookie-retry
  blueprint that chapter 07 codifies)
- `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/state_2026_05_17_unblock_execution.md`
  — branch `fix/engine-fp-backlog` history including the homedepot
  sec-cpt deterministic fix (the pattern Primitive 3 of chapter 07
  formalises)
- `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/state_2026_05_16_engines_research_set.md`
  — engine-research per-vendor docs; the Cloudflare doc there is the
  pre-this-chapter reference

### External — Cloudflare official documentation

- [Detect a Challenge Page response][cf-detect] — `cf-mitigated:
  challenge` is the canonical detection signal
- [Cloudflare Challenges overview][cf-challenges] — the four products
  + when each fires
- [Cloudflare Turnstile docs — Embed the widget](https://developers.cloudflare.com/turnstile/get-started/client-side-rendering/)
- [Cloudflare Turnstile widget configurations][cf-turnstile-docs]
- [Cloudflare Turnstile concepts & widgets](https://developers.cloudflare.com/turnstile/concepts/widget/)
- [Cloudflare Bot Fight Mode — Get started][cf-bfm]
- [Cloudflare bot solutions overview](https://developers.cloudflare.com/bots/)
- [Cloudflare JavaScript Detections][cf-jsd]
- [Cloudflare supported browsers](https://developers.cloudflare.com/cloudflare-challenges/reference/supported-browsers/)
- [Cloudflare cdn-cgi endpoint reference](https://developers.cloudflare.com/fundamentals/reference/cdn-cgi-endpoint/)
- [Cloudflare bots detection IDs](https://developers.cloudflare.com/bots/additional-configurations/detection-ids/)
- [Cloudflare WAF — Challenge bad bots](https://developers.cloudflare.com/waf/custom-rules/use-cases/challenge-bad-bots/)
- [Cloudflare Turnstile — Server-side validation (siteverify)](https://developers.cloudflare.com/turnstile/get-started/server-side-validation/)
- [Cloudflare Super Bot Fight Mode blog post](https://blog.cloudflare.com/super-bot-fight-mode/)
- [Cloudflare Turnstile launch blog post](https://blog.cloudflare.com/turnstile-private-captcha-alternative/)

### External — research and walkthroughs

- [CaptchaAI — Cloudflare Challenge Page Session Flow walkthrough][captcha-cf-flow]
- [RoundProxies — How to build a JSD solver](https://roundproxies.com/blog/jsd-solver-cloudflare/)
- [Codeforces — Bypassing Cloudflare bot fight mode (JS interpreter)](https://codeforces.com/blog/entry/134322)
- [ZenRows — 2026 Cloudflare bypass guide][zen-2026]
- [ScrapeOps — 2026 Cloudflare bypass guide][scrapeops]
- [Scrapfly — 2026 Cloudflare bypass guide](https://scrapfly.io/blog/posts/how-to-bypass-cloudflare-anti-scraping)
- [Scrapfly — Bypass Cloudflare Turnstile][scrapfly-turnstile]
- [Scrape.do — Bypass Cloudflare 2026](https://scrape.do/blog/bypass-cloudflare/)
- [RoundProxies — Bypass Cloudflare 2026](https://roundproxies.com/blog/bypass-cloudflare/)
- [BrightData — Bypass Cloudflare 2026](https://brightdata.com/blog/web-data/bypass-cloudflare)
- [Browserless — Cloudflare scraping 2026](https://www.browserless.io/blog/how-to-bypass-cloudflare-scraping)
- [Cf-Mitigated — HTTP header guide](https://http.dev/cf-mitigated)
- [RoundProxies — cf_clearance harvesting 2026](https://roundproxies.com/blog/cf-clearance/)
- [Scrapfly — Alternatives to Cloudscraper](https://scrapfly.io/blog/posts/what-is-cloudscraper-and-new-alternatives)
- [CapSolver — Solve Cloudflare in 2026](https://www.capsolver.com/blog/Cloudflare/solve-cloudflare-in-2026)
- [CapSolver — Cloudflare 5s challenge solver](https://www.capsolver.com/blog/Cloudflare/how-to-solve-cloudflare-challenge-5s)
- [uCaptcha — Cloudflare challenge pages explained](https://ucaptcha.net/blog/cloudflare-challenge-pages/)
- [Nstbrowser — Headless browser with Cloudflare bypass](https://www.nstbrowser.io/en/wiki/headless-browser-cloudflare-bypass-anti-fingerprint)
- [Nstbrowser — Cloudflare human verification bypass](https://www.nstbrowser.io/en/wiki/how-does-a-headless-browser-handle-cloudflare-human-verification-bypass)

### External — open-source reference projects (read for protocol, do NOT copy code)

- [`VeNoMouS/cloudscraper`](https://github.com/VeNoMouS/cloudscraper) — Python JS-PoW + paid CAPTCHA integration (MIT)
- [`sarperavci/CloudflareBypassForScraping`](https://github.com/sarperavci/CloudflareBypassForScraping) — Request-mirroring + local bypass server
- [`FlareSolverr/FlareSolverr`](https://github.com/FlareSolverr/FlareSolverr) — Selenium + undetected-chromedriver reverse proxy
- [`ultrafunkamsterdam/undetected-chromedriver`](https://github.com/ultrafunkamsterdam/undetected-chromedriver) — Patched ChromeDriver
- [`g1879/DrissionPage`](https://github.com/g1879/DrissionPage) — Python requests + browser combo
- [`vvanglro/cf-clearance`](https://github.com/vvanglro/cf-clearance) — Playwright cf_clearance harvester
- [`x404xx/Turnstile-Solver`](https://github.com/x404xx/Turnstile-Solver) — REST API Turnstile solver
- [`scaredos/cfresearch`](https://github.com/scaredos/cfresearch) — Protocol reverse-engineering research
- [`cloudflare-turnstile-bypass` GitHub topic](https://github.com/topics/cloudflare-turnstile-bypass)
- [Hyper-Solutions](https://github.com/Hyper-Solutions) — Commercial SDK
- Byparr (Camoufox-backed FlareSolverr replacement, recommended for Turnstile per Scrapfly 2026 guide)
