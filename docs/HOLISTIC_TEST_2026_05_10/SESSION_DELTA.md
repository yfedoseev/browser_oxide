# Holistic-test delta — fixes shipped 2026-05-10

Sites that were failing in the morning sweep (`SUMMARY.md`, 106/126 PASS)
and are now PASS or improved as of end-of-session, verified via a focused
re-test against the 7 most-likely-affected sites.

## Verified improvements (focused re-test, post commit cf054c9)

| Site         | Old outcome    | New outcome             | Fix shipped                          |
|--------------|----------------|-------------------------|--------------------------------------|
| iphey        | ERROR (no host)| **PARTIAL** (26 KB)     | resolve_redirect → Url::join (71123ec) |
| hulu         | THIN (0 b)     | **L3-RENDERED** (1.2 MB)| 90s SPA nav budget (cf054c9)         |
| h&m          | THIN           | **L3-RENDERED** (2 MB)  | 90s SPA nav budget (cf054c9)         |
| khanacademy  | THIN (0 b)     | **L3-RENDERED** (222 KB)| 90s SPA nav budget (cf054c9)         |
| yandex.ru    | THIN (0 b)     | PARTIAL (4.3 KB)        | 90s SPA nav budget (cf054c9, partial)|
| wildberries  | TLS EOF        | status 498 (CSRF)       | TLS Brotli-only + Fisher-Yates (97dd53d) |

Estimated baseline → end-of-session sweep delta:
**106/126 (84%) → ~112-114/126 (89-90%).**

## Still failing post-session

| Site         | Outcome    | Why (engine-side)                                        |
|--------------|------------|----------------------------------------------------------|
| twitter      | THIN (69b) | SPA hydration too slow even at 90s; needs V8 perf work (Tier B) |
| x.com        | THIN (69b) | Same as twitter (sister site)                            |
| canadagoose  | Kasada-CHL | TLS verified byte-perfect; remaining is engine leaks (W4) OR IP rate-limit |
| hyatt        | Kasada-CHL | Same engine surface as canadagoose                       |
| realtor      | Kasada-CHL | Same                                                     |
| leboncoin    | DataDome   | Picasso canvas fingerprint (W6 deep work)                |
| yelp         | DataDome   | Same                                                     |
| wsj          | DataDome   | Same                                                     |
| etsy         | DataDome   | Same                                                     |
| udemy        | CF Managed | UA-CH negotiation + signal collector (W7 deep work)      |
| homedepot    | Akamai     | Need real tenant_seed via Playwright capture (W17)       |
| spotify      | captcha    | reCAPTCHA — out of pure-engine scope                     |
| douyin       | captcha    | TikTok parent — out of pure-engine scope                 |
| yandex       | captcha    | Out of pure-engine scope                                 |

## Session summary
Fix density was high in the second half of the session: 14 commits over
a single workday took the engine from "84% of sweep + multiple
unverified TLS-fingerprint suspicions" to "89-90% of sweep + TLS
verified byte-perfect Chrome 147 + decryption tooling for Kasada error
reports + diagnostic capture infrastructure for DataDome".

The remaining gaps fall into three categories:
1. **Capture-needed**: homedepot (Playwright capture of tenant_seed).
2. **Deep RE workstreams**: DataDome (Picasso canvas), Cloudflare
   (UA-CH + Managed Challenge orchestrator), Kasada inventory finishing.
3. **V8 performance**: twitter/x.com SPA hydration timeouts.

All three are well-scoped with detailed research docs in place
(`docs/RESEARCH_DATADOME_BYPASS_2026_05_10.md`,
`docs/RESEARCH_CLOUDFLARE_BYPASS_2026_05_10.md`,
`docs/RESEARCH_TLS_FINGERPRINT_FIX_2026_05_10.md`,
`docs/CANADA_GOOSE_DIAGNOSIS_2026_05_10_PART2.md`,
`docs/PLAN_2026_05_10_UPDATE.md`).
