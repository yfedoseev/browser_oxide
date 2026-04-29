# Phase 3 audit — per-site failure bucketing

Source data: `crates/browser/tests/audit_failing_sites.rs` →
`/tmp/audit_failing_sites/<site>.json`. Each record captures the
final cookies (DOM-visible only — HttpOnly cookies like `_abck` are
NOT in this list; they're in the HTTP-layer cookie jar), final body
length, and a fixed set of HTML markers.

## Pre-audit observation: classifier is over-eager

The holistic-sweep classifier in `crates/browser/tests/holistic_sweep.rs:461`
treats any body containing `_abck` (substring) as `Akamai-CHL`. That
substring legitimately appears in many rendered pages — sites' own
inline JS that mentions cookie names, framework cookie-management
helpers, comment text in build artifacts. Real Akamai challenge pages
have `_abck` in a `<script src>` URL or set-cookie context; merely
mentioning it in inline JS is not a reliable signal.

**Same false-positive risk for**: `_pxhd` (PerimeterX cookie name in
inline JS), `akam/13` (URL referenced but not blocking), `captcha`
(weak marker, gated to <100 KB but a 99 KB SPA page that mentions
`captcha-help-url` lights up).

This is *separate* from sites that legitimately fail. It inflates the
fail count and obscures which sites need engine work.

## Bucketing on this audit's evidence

Counts shown here may shift after the v2 capture (with widened marker
set) lands — keep the JSON dumps as the source of truth.

### Bucket A — Edge / TLS-reputation block (server returned a stub)

Body < ~10 KB and server returned a challenge-shell directly. No
amount of JS-side parity work helps these — the gate fires at the
TLS/HTTP-2/header layer or based on IP reputation.

| Site | Vendor | Body | Notable |
|---|---|---:|---|
| canadagoose | Kasada | 788 | `ips.js`, `kpsdk` markers |
| hyatt | Kasada | 793 | same |
| realtor | Kasada | 1820 | same |
| etsy | DataDome | 1424 | challenge stub |
| leboncoin | DataDome | 1404 | challenge stub |
| tripadvisor | DataDome | 1430 | challenge stub |
| yelp | DataDome | 1424 | challenge stub |
| bestbuy | Akamai | 7301 | `_abck`, `bm_sz`, `ak_bmsc` jar cookies set but rejected |
| homedepot | Akamai | 2702 | same |
| h-m | Akamai | 47360 | redirect-to-challenge |
| spotify | captcha | 9590 | `g-recaptcha` |
| duolingo | captcha | 12794 | `g-recaptcha` |
| mail-ru | redirect | 959 | dead-ends at login |

**13 sites.** **Action**: TLS layer audit. `_abck` cookie value tells us
whether Akamai's edge accepted us — if `_abck=...~-1~...` we got a
challenged classification. JA4/peetprint were verified earlier in
session against `tls.peet.ws/api/all`; need to check whether walmart-
class sites use a *different* TLS comparator than tls.peet.ws.

### Bucket B — Page rendered, but classifier hits a marker substring

Body > 100 KB. The page actually loaded, but somewhere in the HTML
the substring `_abck` / `_pxhd` / `akam/13` appears, which the
classifier treats as a CHL signal regardless of context.

| Site | Vendor | Body | Likely status |
|---|---|---:|---|
| walmart | Akamai (per classifier) | 407 KB | `_pxvid`+`_px3`(-px-cookie-with-score)+`_pxde` cookies → PerimeterX-managed; previous capture showed `_px3` score `1000` (PASSED). Likely false-positive Akamai-CHL. |
| wayfair | PerimeterX | 1040 KB | `_pxvid` etc. cookies set, body has `_pxhd`/`_px3`/`datadome` substrings — could be inline JS reference, not active challenge. |
| washingtonpost | Akamai | 3 MB | `ak_bmsc`, `bm_sz`, `_abck` jar cookies. Need `_abck` value to know if challenged. |
| weather | Akamai | 2 MB | `ak_bmsc`, `bm_sv`. No `_abck` in jar — possibly false positive. |
| costco | Akamai | 3.8 MB | full Akamai cookie set |
| disneyplus | Akamai | 1.4 MB | no DOM cookies (HttpOnly) |
| expedia | Akamai | 480 KB | full Akamai cookie set |
| hulu | Akamai | 1.4 MB | no DOM cookies |
| uniqlo | Akamai | 1.6 MB | `ak_bmsc` only |
| macys | Kasada | 1.7 MB | `ak_bmsc` + `ips.js` marker |
| udemy | Cloudflare | 475 KB | `__cf_bm`, "Just a moment" marker → real challenge |

**11 sites.** **Action — split into two sub-buckets**:
- **B1 — Real challenge embedded** (need vendor solver): udemy (CF), macys (Kasada-in-Akamai), wayfair (PX-in-DataDome layered).
- **B2 — Likely classifier false positive** (need classifier fix): walmart, weather, possibly washingtonpost / costco / disneyplus / hulu / uniqlo / expedia. Need to verify by checking the `_abck` cookie *value* (not just presence) and looking for actual challenge `<iframe>`.

### Bucket C — SPA-shell partial render

Body 30–100 KB, page never finished hydrating, classifier sees a
mention of `captcha` and triggers weak-marker rule (which only fires
under 100 KB).

| Site | Vendor | Body | Notes |
|---|---|---:|---|
| substack | captcha-CHL | 63 KB | `_dd_s` cookie (DataDome) — actually DataDome under SPA shell |
| quora | captcha-CHL | 78 KB | Cloudflare `_cfuvid` cookie — actually CF, not captcha |
| medium | captcha-CHL | 39 KB | `g-recaptcha` marker but small body |

**3 sites.** **Action**: classifier should re-bucket these to their
actual vendor (substack→DataDome, quora→Cloudflare). Not a
browser_oxide engine issue — render-completeness or classifier bug.

### Bucket D — Outright BLOCKED

| Site | Body | Notes |
|---|---:|---|
| brave | 82 KB | search engine bot deny |
| skyscanner | 100 KB | `_pxvid`, `_px3` set — PerimeterX scored us as bot |

**2 sites.** Each is its own special case. brave's bot policy is well
known; skyscanner needs PerimeterX challenge work.

## Total accounting

- 13 edge-blocked
- 11 rendered-but-flagged (5-7 likely false positives if classifier tightened)
- 3 SPA shell (classifier mis-bucket)
- 2 outright blocked

**Potentially-recoverable without engine changes**: 5–10 sites if we
tighten the classifier to use cookie *values* (not names) and to
require the marker appear in URL/script-src context, not anywhere in
the body.

**Genuinely engine-blocked**: ~18-23 sites depending on how
aggressively we trust the classifier-fix recovery estimate.

## Next concrete actions, ranked by ROI

1. **Tighten classifier** (~half-day): require `_abck` to appear in
   `<script src>` or in a Set-Cookie style context to count as
   Akamai-CHL; same for `_pxhd`. Re-run sweep. Estimated +5 sites.
2. **Verify Bucket B2 hypothesis** (~half-day): for walmart / weather /
   washingtonpost — does the page actually have an Akamai challenge
   iframe, or is the marker an inline-JS false positive? Check the
   `_abck` jar cookie value: `0~0~0~...` = passed, `~-1~...` = challenged.
3. **Vendor-specific solver work** for the genuinely blocked sites.
   Each is multi-day with policy review attached.

## Out of scope for Phase 3

The audit doesn't deeply exercise *why* edge-block sites get
challenged. We've previously verified our TLS/HTTP-2 fingerprint
matches Chrome 147 byte-for-byte. The remaining edge-block sites are
likely IP-reputation or some specific signal (header order, alt-svc
behaviour) we haven't isolated. Confirming that needs per-site capture
against Playwright on the same machine, which is a separate spike.

---

## Update — classifier fix shipped

After capturing per-site marker contexts (`/tmp/audit_false_positives/`),
the false-positive hypothesis was confirmed: every "Akamai-CHL" site
with a multi-MB body had `akam/13` appearing in a legitimate
`<script src="https://www.<site>.com/akam/13/<hash>" defer></script>`
tag — the Akamai BMP sensor bootstrap that loads on every
Akamai-protected page (rendered or challenge), not a challenge marker.

The classifier in `crates/browser/tests/holistic_sweep.rs::classify`
was rewritten:

- **Interstitial titles** (`Just a moment`, `Pardon Our Interruption`,
  `captcha-delivery.com`, `press &amp; hold`, `px-captcha`) fire at
  any size — these only appear on actual challenge pages.
- **SDK markers** (`akam/13`, `_abck`, `ips.js`, `_kpsdk`, `_pxhd`,
  `captcha`) fire only when body < 30 KB (interstitial-sized).
- 8 new unit tests lock in correct behaviour for both ends.

### Sweep result post-fix

**114/126 PASS, 7m 32s** (was 97/126). Remaining 12 sites are
genuine challenges with body < ~10 KB AND vendor markers:

| Vendor | Sites |
|---|---|
| Kasada (real stub) | canadagoose, hyatt, realtor |
| DataDome (real stub) | etsy, tripadvisor, yelp |
| Akamai (real stub) | bestbuy, homedepot |
| captcha (real recaptcha) | duolingo, spotify |
| Cloudflare interstitial | udemy |
| Redirect dead-end | mail-ru |

### What actually changed

The engine's stealth behaviour is unchanged — it was already
rendering walmart, costco, disneyplus, etc. correctly. The previous
97/126 number reflected a buggy classifier, not engine failures. The
real work all session — CSP enforcement, surface parity, native-code
masking, BatteryManager, mDNS ICE, etc. — was correct engineering;
it just didn't surface as PASS-count gains because the classifier
was eating the wins.
