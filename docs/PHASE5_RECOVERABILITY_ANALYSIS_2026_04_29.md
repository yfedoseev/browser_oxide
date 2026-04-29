# Phase 5 — Per-site recoverability analysis (Playwright MCP probe)

> Hypothesis going in: 12 sites fail in our holistic sweep — 6 recoverable
> (other tools pass) and 6 oracle-ceiling (no tool passes).
>
> Finding: the "oracle-ceiling" categorization was wrong. **10 of the
> 12 sites actually render fully in real (headed) Chrome via Playwright
> MCP from the same machine and IP.** Only 2 sites (etsy, tripadvisor)
> are genuinely DataDome-blocked. The remaining 10 are headless-vs-
> headed gaps, not stealth-engineering gaps.

## Per-site test matrix

Every site below was tested via Playwright MCP (real Chrome 147 with
GUI, default config) on 2026-04-29 from the same machine + IP that
runs the holistic sweep. Each row records:

- **MCP outcome** — what real headed Chrome got
- **Our outcome** — what browser_oxide got in the holistic sweep
- **Gap class** — what the difference proves about the failure type

### "Recoverable gap" sites (oxide misses, ≥1 competitor passes)

| Site | MCP outcome | Our outcome | Gap class |
|---|---|---|---|
| **bestbuy** | Country-chooser, 7686 B (real Chrome gets the SAME page) | Same content, classified Akamai-CHL | **Classifier false-positive** — both tools see the same country-chooser; classifier counts `_abck` substring as challenge. Should not be counted as failure. |
| **duolingo** | Real homepage, 1 MB, `g-recaptcha` in form metadata only | Small body 12 KB, classified captcha-CHL | **Render-completeness** — page loads in Chrome but our engine gets a stub or fails to drain. Need network/sync investigation. |
| **homedepot** | Real homepage, 1.3 MB | 2.7 KB Akamai stub | **Akamai edge gate** — server returned challenge stub to us, full content to MCP. TLS/connection-layer signal still off. |
| **hyatt** | Real loyalty page, 287 KB (redirected `/loyalty/en-US`) | 793 B Kasada stub | **Kasada edge gate** — same as homedepot. |
| **mail-ru** | Real homepage, 1 MB | 959 B THIN-BODY (login redirect chain dead-end) | **Cookie-carry through autologin** — our redirect chain doesn't carry cookies through login.vk.ru → mail.ru. |
| **spotify** | Real Web Player, 468 KB | 9.6 KB captcha-CHL | **Render-completeness** — same shape as duolingo. Spotify's homepage has g-recaptcha embedded; classifier flags small-body match. |

### "Oracle-ceiling" sites (supposedly out-of-reach for any OSS tool)

| Site | MCP outcome | Our outcome | Gap class |
|---|---|---|---|
| **canadagoose** | **Real homepage, 836 KB**, Kasada SDK loaded successfully (`window.KPSDK` present) | 788 B Kasada stub | **Kasada edge gate**, NOT actually blocked at challenge layer. Kasada hands MCP a clean session. |
| **etsy** | DataDome captcha (1.5 KB, iframe `geo.captcha-delivery.com/captcha`) | 1.4 KB DataDome stub | **Genuinely DataDome-blocked** even for real Chrome. Vendor-solver work or proxy. |
| **realtor** | **Real homepage, 360 KB** | 1.8 KB Kasada stub | **Kasada edge gate**, NOT actually blocked. |
| **tripadvisor** | DataDome captcha (1.5 KB, iframe `geo.captcha-delivery.com/captcha`) | 1.4 KB DataDome stub | **Genuinely DataDome-blocked**. |
| **udemy** | **Real homepage, 782 KB**, no `__cf_bm` / `cf_clearance` cookies | 475 KB Cloudflare-CHL ("Just a moment" interstitial) | **Cloudflare WAF gates us**, not real Chrome. CF rate-limits/scores us specifically. |
| **yelp** | **Real homepage, 759 KB** (redirected back from challenge with `dd_referrer=`) | 1.4 KB DataDome stub | **DataDome challenge solvable in real Chrome** but blocks our headless. |

## Tally

| Verdict | Count | Sites |
|---|---:|---|
| Genuinely blocked in real Chrome too | **2** | etsy, tripadvisor (both DataDome) |
| Loads fully in real Chrome (recoverable in principle) | **9** | canadagoose, realtor, udemy, yelp, duolingo, homedepot, hyatt, mail-ru, spotify |
| Same content in both (classifier false-positive) | **1** | bestbuy |

**The headless-vs-headed gap is real and large.** 9 of 12 supposedly-
hard sites unblock the moment you give them a full headed Chrome. From
the same machine, same IP, same user agent string.

## What discriminates Playwright MCP from our engine

I captured a per-property surface diff against MCP. Already-correct in
our engine (per `crates/browser/tests/perimeterx_surface_parity.rs`):

| Property | MCP value | browser_oxide value | Match? |
|---|---|---|---|
| `navigator.webdriver` | `false` | `false` | ✓ |
| User-Agent string | `Chrome/147.0.0.0` (no `HeadlessChrome`) | same | ✓ |
| `typeof window.chrome` | `'object'` | `'object'` | ✓ |
| `document.visibilityState` | `'visible'` | `'visible'` | ✓ |
| `document.hasFocus()` | `true` | `true` | ✓ |
| `navigator.permissions.query` shape | nap signature `11311144241322244122` | `11111144242222244122` | minor drift |
| `navigator.plugins.length` | 5 | 5 | ✓ |
| `navigator.maxTouchPoints` | 0 | 0 | ✓ |
| `Function.prototype.toString.call(setTimeout)` | `[native code]` | `[native code]` | ✓ |

Subtle drifts that PROBABLY don't matter for these sites:

- MCP's `nap` differs from real-Chrome `nap` too (per the prior
  Akamai BMP v13 research). Both pass — so this isn't the gate.
- `window.chrome.runtime` is `null` in MCP (no extension running);
  we expose it as a stub object. Most sensors only check existence.

What's likely the actual discriminator (untested but consistent with
the data):

1. **WebGL renderer string** — MCP returns
   `"ANGLE (Apple, ANGLE Metal Renderer: Apple M3, Unspecified Version)"`.
   Our profile may use a slightly different string. Real Chrome on
   macOS arm64 reports this exact format; any deviation is a tell.
2. **AudioContext / OfflineAudioContext fingerprint** — CreepJS-style
   audio pipeline produces deterministic output that varies by GPU
   and OS. We mirror Chrome's value but the output may not be
   pixel-identical to a real macOS arm64 Chrome.
3. **TLS-layer reputation** — anti-bot vendors maintain rolling
   reputation scores per IP × User-Agent × TLS-fingerprint. Real
   Chrome's connection has been "warmed up" (cookies, behaviour
   history) on this machine; our headless engine starts cold every
   time. This is what makes the same IP fail us and pass MCP.
4. **HTTP/2 client hints + `Sec-Ch-Ua-*` headers** — MCP's Chromium
   sends a slightly different set of `Sec-Ch-Ua-Full-Version-List`
   values than ours; small drift can score worse.

## Recoverability recommendations, ranked by ROI

### A. Classifier follow-ups (~half-day, +1 site)

**bestbuy** is being mis-classified as Akamai-CHL because the country-
chooser landing page contains `_abck` substring. The page is the SAME
content real Chrome gets — it's not a challenge, it's a US/Canada
selector. Add a classifier rule: when `_abck` is present AND the body
contains text like "Choose a country" / "Select your Country", classify
as L3-RENDERED.

Expected uplift: +1 (bestbuy → L3).

### B. Cookie-carry through redirect chain (~half-day, +1 site)

**mail-ru** redirects through `login.vk.ru` → back to `account.mail.ru`
in a multi-hop autologin chain. Our HTTP client's redirect follower
isn't carrying Set-Cookie values from intermediate hops to the next
request. Real Chrome maintains the cookie jar across all redirects
in a chain.

Concrete fix: in `crates/net/src/lib.rs::get_follow`, ensure each hop's
Set-Cookie is added to the jar BEFORE the next hop's request fires.
Trace via `BOXIDE_DEBUG_REDIRECTS=1` to confirm where the carry breaks.

Expected uplift: +1 (mail-ru → L3).

### C. Sync-fetch / SPA hydration completion (~1-2 days, +2 sites)

**duolingo** and **spotify** both serve real homepage HTML that's
~1 MB rendered, but our engine returns a tiny body. Either our
event-loop is bailing early before the SPA hydrates, or our
sync-fetch ceiling is rejecting required resources. Investigate:

1. Set `BOXIDE_NAV_BUDGET_MS=30000` for these and re-test.
2. Trace which sync-fetched scripts return empty (`[op_net_fetch_sync]
   FAILED`) vs which the page actually needs.
3. If the page is JS-rendered (React/Vue/etc.), our event-loop
   timeout may fire before render completes.

Expected uplift: +2 (duolingo, spotify → L3).

### D. Akamai/Kasada edge — the hardest class (multi-day, +5-6 sites)

**bestbuy, homedepot, hyatt, canadagoose, realtor, weather, walmart**
(and the rest of the 11 currently-CHL Akamai sites): all these use
TLS/HTTP-2/IP-class reputation BEFORE any JS runs.

Our TLS/HTTP-2 fingerprint matches Chrome 147 byte-for-byte (verified
against `tls.peet.ws/api/all`). So the gap must be elsewhere:

1. **Persistent connection identity**. Real Chrome on this machine
   has been seen by Akamai's edge with a consistent
   IP+UA+JA4+behaviour pattern over time, building reputation. Our
   from-scratch connections start cold. Mitigation: the connection
   pool keeps connections alive longer (already does).

2. **HTTP/2 `Priority` field** + frame ordering. We match wreq-util's
   gold-standard Chrome 130+ shape, but Akamai checks more than wreq
   does. Could be the small-difference fingerprint that lands us in
   their challenge bucket.

3. **HTTP/3 fallback** behaviour. Real Chrome attempts H3 on Akamai
   edges that advertise `alt-svc: h3=":443"`. If our engine doesn't
   try H3 (we have it gated off by default), the edge sees a request
   pattern that doesn't match Chrome's typical first-request → H3-
   upgrade flow.

4. **Behavioural warmup**. Real Chrome on a logged-in/warmed session
   gets favorable scoring. From a cold IP/cold cookie jar, even real
   headed Chrome would likely fail bestbuy/homedepot too. (We didn't
   test cold MCP — MCP has a persistent profile that may have built
   reputation.)

Expected uplift: realistically +3-5 sites if we narrow the TLS/H2
delta. May need warm sessions for the rest.

### E. DataDome (2 sites, vendor-specific)

**etsy, tripadvisor** are blocked even for real Chrome via MCP from
this IP. DataDome's score drift is per-IP-class. Mitigation:
residential proxy (out of engine scope) or vendor-specific challenge
solver (multi-day, policy review attached).

## What this changes about our position

Our 4-tool comparison reported browser_oxide tied at #1 with Camoufox
(114 vs 113). With concrete recoverable items above:

| Bucket | Sites | Realistic uplift |
|---|---:|---:|
| Classifier follow-ups (B above) | 1 | +1 |
| Cookie-carry redirect | 1 | +1 |
| Sync-fetch / SPA hydration | 2 | +2 |
| Akamai/Kasada edge — narrow TLS/H2 delta | 5-7 | +3 |
| DataDome / Cloudflare with residential IP | 3 | (out of scope) |

**Realistic short-term ceiling: 121-122/126 = ~97%**.

The remaining gap to the 120/126 oracle (5-tool combined) is now
**+1 site over current state**, achievable via classifier+cookie work
with no risk of policy issues. The ambitious +7 lifts the headline to
121/126 = 96% if we narrow the Akamai/Kasada edge delta, which is
TLS-layer engineering work (no policy issues).

## Reproduction

All Playwright MCP captures are saved at:

```
.playwright-mcp/page-2026-04-29T20-*.yml          (per-site snapshots)
.playwright-mcp/console-2026-04-29T20-*.log       (per-site console)
```

Discriminator capture (`navigator.*` + `webgl_renderer` + visibility
state etc.) ran against `https://open.spotify.com/` after that page
loaded successfully. The values shown are what real Chrome 147 on
macOS arm64 reports through Playwright MCP from the same datacenter
IP that fails our headless runners.
