# Why the 20 non-passing sites fail — root cause classification

For each of the 20 sites that didn't render L3 in the 2026-05-10 holistic
sweep, the cause is now identified from the run log + this session's
Kasada-decryption findings. Sites cluster into three buckets by what
fixing them needs.

## Bucket A — Engine bugs we can fix directly (8 sites)

These are bugs in browser_oxide that don't involve antibot vendors.
Quoted log lines from `docs/HOLISTIC_TEST_2026_05_10/run.log`.

| Site         | Outcome   | Quoted symptom | Root cause |
|--------------|-----------|----------------|------------|
| **iphey**    | ERROR     | `HTTP error: no host in URL` (after 9.9s) | URL parser / redirect handler can't resolve some redirect target. Likely a relative-URL Resolution bug or an empty-Location header path. Engine fix, hours. |
| **wildberries** | ERROR  | `TLS error: TLS handshake failed: TLS handshake failed unexpected EOF` + repeated `no ct_token to inject` | Server closes connection mid-handshake. Same family as the H2/HTTP-1.1 ALPN downgrade we see on canadagoose — server is rejecting our ClientHello fingerprint outright. Needs Wireshark diff against real Chrome 147 ClientHello, then fix in `crates/net/src/tls.rs`. |
| **twitter / x-com** | THIN-BODY | body=69 bytes after 85s | These are SPA shells (`<!DOCTYPE html><html><head>...</head><body><div id=root></div></body>` or similar). Real Chrome runs the bundled React app and populates `#root`. We hit the V8 deadline before React's main bundle finishes hydrating. Two paths: bump deadline (cheap, partial) OR profile our V8 to find why hydration is so slow. |
| **hulu**     | THIN-BODY | body=0 bytes after 85s | Same shape as twitter. SPA shell, app didn't hydrate within the budget. |
| **khanacademy** | THIN-BODY | `no ct_token to inject for www.khanacademy.org` + body=0 after 117s | KA is Kasada-protected AND a heavy SPA. We get past the Kasada interstitial but the React app behind it doesn't render before the deadline. |
| **yandex-ru** | THIN-BODY | body=0 after 110s | Heavy Russian-localized SPA. Same hydration-time-budget issue. |
| **h-m**      | THIN-BODY | (intermixed paypal/recaptcha logs visible) | The H&M page tries to load reCAPTCHA Enterprise; that fails to fully execute in our engine, then the rest of the page never renders. |

**Fix priority:** wildberries TLS issue is highest-impact (same root as the
H2 downgrade affecting canadagoose). The 5 THIN-BODY sites are a single
class — V8 deadline + SPA hydration profiling. iphey is a small URL
parser bug.

## Bucket B — Antibot vendor challenges (we reach the interstitial; need vendor-specific work) (9 sites)

These sites render correctly in our engine but the antibot vendor's JS
detects something and serves the challenge page. From the engine's POV
these are "successful" navigations; they're labelled as challenges by
our heuristic body classifier.

### Kasada (3 sites — root cause inventory documented)

| Site         | Outcome    | Notes |
|--------------|------------|-------|
| canadagoose  | Kasada-CHL | Logs show `H2 connection failed: ALPN negotiated http/1.1, not h2` AND `LEARNED x-kpsdk-ct (len=174)` — the PoW solver works but two engine signals still trip Kasada: H2/1.1 ALPN downgrade + the 13 remaining unjzomuy/Function.toString/error-text-parity leaks documented in `docs/CANADA_GOOSE_DIAGNOSIS_2026_05_10_PART2.md`. |
| hyatt        | Kasada-CHL | Same engine surface. Same 13 leaks. |
| realtor      | Kasada-CHL | Same engine surface. |

**Fix path:** continue the inventory in `CANADA_GOOSE_DIAGNOSIS_PART2.md`
priority order. Each fix should drop the captured error-blob count for
*all three* sites (they share the same Kasada code path).

### DataDome (4 sites — separate vendor, separate analysis needed)

| Site         | Outcome      | Notes |
|--------------|--------------|-------|
| leboncoin    | DataDome-CHL | Logs: `[vendor-detect] datadome` + `Refused to frame 'https://geo.captcha-delivery.com/captcha/'` (CSP blocked the captcha iframe — that's actually correct behavior; the CHL classifier triggers on the captcha intent). |
| yelp         | DataDome-CHL | Same vendor pattern. |
| wsj          | DataDome-CHL | body=1427 bytes — small interstitial, not real content. |
| etsy         | DataDome-CHL | Same. |

**Fix path:** we have no DataDome-specific reverse-engineering yet. The
captured body for any of these is the DataDome challenge JS; would
benefit from a `kasada_error_blob_capture`-style instrumentation
specifically for DataDome's `dd-script.min.js` execution path. New
work-stream — call it task #12.

### Akamai (1 site — partial infrastructure exists)

| Site       | Outcome    | Notes |
|------------|------------|-------|
| homedepot  | Akamai-CHL | We have `crates/akamai/` with the v2 sensor_data builder, but `get_tenant_settings()` only knows bestbuy's seed + path. Homedepot's needs to be captured via Playwright MCP and added. The infrastructure is there; this is a config-table extension. |

### Cloudflare (1 site — separate vendor)

| Site       | Outcome         | Notes |
|------------|-----------------|-------|
| udemy      | Cloudflare-CHL  | body=476 KB (the CF challenge page is large). Need CF turnstile/IUAM solver — separate vendor, not addressed yet. |

## Bucket C — reCAPTCHA / explicit captcha challenges (3 sites — NOT engine-fixable)

| Site       | Outcome     | Why |
|------------|-------------|-----|
| douyin     | captcha-CHL | TikTok parent site. Chinese-IP-gated + captcha — even real headed Chrome from a residential western IP gets challenged. |
| yandex     | captcha-CHL | Logs show `H2 connection failed for sso.passport.yandex.ru: ALPN negotiated http/1.1, not h2` — same TLS issue as wildberries. After login flow hits, captcha-required even for headed Chrome from non-RU IPs. |
| spotify    | captcha-CHL | reCAPTCHA Enterprise on the open.spotify.com landing. Designed to require a human solver. |

**Fix path:** these CANNOT be solved purely in-engine. Three options:
1. **Captcha-solving service** (2Captcha, Anti-Captcha, CapSolver) —
   programmatic but costs money per solve.
2. **Aged session cookies** — log in once via headed browser, persist
   the auth cookies, reuse — bypasses captcha entirely for that domain.
3. **Skip these sites** — accept that automated headless access has
   inherent limits when vendors deliberately gate on human-only
   challenges. (Real browser automation has the same constraint.)

These are correctly classified as out-of-engine-scope.

## Summary by fixability

| Bucket | Sites | Action |
|--------|------:|--------|
| A — pure engine bugs | 8 | Fix in code. TLS ClientHello (wildberries) + V8 deadline/hydration (5 SPAs) + URL parser (iphey) + Kasada-on-SPA (khanacademy). |
| B-Kasada | 3 | Continue PART2 fix inventory. |
| B-DataDome | 4 | New work-stream: DataDome reverse-engineering. |
| B-Akamai | 1 | Add homedepot config to `akamai::get_tenant_settings()`. |
| B-Cloudflare | 1 | New work-stream: CF Turnstile/IUAM. |
| C — captcha (out of scope) | 3 | Use captcha-solving service or aged sessions. |

Realistic ceiling for engine-only work: **A + B = 17 / 20 fixable**, which
would take the holistic sweep from 106/126 → 123/126 (97%). The remaining
3 captcha sites are the inherent ceiling of headless automation.
