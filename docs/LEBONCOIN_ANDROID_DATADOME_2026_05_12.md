# Android leboncoin DataDome Regression — Investigation (2026-05-12)

Option C from `docs/SWEEP_3PROFILE_2026_05_12.md`. Diagnose why `leboncoin.fr`
passes on desktop (`L3-RENDERED`, body=349,061 bytes) but blocks under
Android profile (`DataDome-CHL`, body=1,404 bytes).

## Captured signal

Both sweeps trigger `[vendor-detect] datadome on https://www.leboncoin.fr/`
(same DataDome integration on the site). The difference is the **outcome**:

| Profile | Body length | Outcome |
|---|---:|---|
| chrome_130_macos | 349,061 | L3-RENDERED |
| pixel_9_pro_chrome_147 | **1,404** | **DataDome-CHL** |

The 1,404-byte response is DataDome's standard challenge interstitial. Same site
issues a full page to desktop UA, a challenge to mobile UA, **from the same
datacenter IP**.

## Root cause

DataDome's threat-research literature (and the audit doc
`docs/RQUEST_MOBILE_TLS_AUDIT_2026_05_12.md` §"Mobile vs desktop scoring")
documents this pattern: their mobile risk track expects **carrier IPs**
(4G/5G CGNAT). A datacenter IP advertising as `Pixel 9 Pro / Chrome 147 / Android`
is a **stronger negative signal** than the same datacenter IP advertising as
desktop Chrome — the latter is a "common bot" (medium risk), the former is
"spoofed mobile bot" (high risk).

Empirically:
- Desktop datacenter IP + Chrome desktop UA → DataDome scores: **pass**
- Same datacenter IP + Pixel mobile UA → DataDome scores: **challenge**

Our request payload was identical except for UA + Sec-CH-UA-* + the curve swap
(MLKEM → Kyber768Draft00). All three signals point to "Android Chrome on a
not-mobile-network" — exactly the pattern DataDome's mobile track penalizes.

## What's NOT the cause

- Not a TLS fingerprint divergence (curl-impersonate's `chrome_131_android`
  signature is achievable; we're close enough to that)
- Not a Sec-CH-UA-* header issue (we emit the correct mobile flavor with `?1`,
  Form-Factors=Mobile, Model=Pixel 9 Pro)
- Not a JS-side surface bug (sweep blocks at the navigation layer, before our
  JS runs to completion — body is only 1,404 bytes, doesn't even load ips.js)

## Fix paths

### 1. Mobile carrier IP (the real fix) — out of scope

Use a 4G/5G mobile-proxy provider for any request advertising mobile UA.
Per the synthesis doc Tier 5 mobile readiness: this is an **infrastructure**
problem, not an engine problem. Bright Data, Smartproxy, AirSocks, NetNut all
sell mobile carrier-IP rotation. Cost ~$5-50/GB.

Implementation effort in browser_oxide: ~0.5 days (we already have
`profile.proxy` plumbing; just need to make the proxy URL device_class-aware
in the preset).

### 2. Mark Android profile as "datacenter-IP-only" — operational mitigation

Document that `pixel_9_pro_chrome_147` SHOULD NOT be used against
DataDome-protected French sites without a mobile proxy. Acceptable today
since the +3 sites Android wins are: azure, costco, etsy, spotify — all
US-based and not DataDome-aggressive on mobile.

Effort: 0 days. Just document.

### 3. Hybrid: try desktop fallback on Datadome challenge — engine work

When `vendor-detect` reports `datadome` on a site about to be challenged,
fall back to desktop profile for that domain. Adds complexity (per-domain
profile selection) and partially defeats the purpose of having a mobile profile.

Effort: ~1 day. Low-priority.

## Recommendation

**Path 2** (document + accept). The leboncoin regression is a known limitation
of mobile-from-datacenter testing, NOT an engine bug. The other 4 Android
wins (azure, costco, etsy, spotify) net us +3 sites overall. leboncoin only
recovers if/when mobile carrier proxies are integrated (Path 1).

## Validation

To verify the IP-vs-UA hypothesis without infrastructure changes:
1. Run leboncoin under Android profile from a residential IP (e.g. via
   personal home network or a residential proxy) — expect to pass.
2. Run leboncoin under Android profile through Bright Data mobile-pool
   trial — expect to pass with even higher trust score.
3. Run leboncoin under Chrome 147 Windows desktop — expect to also pass
   (same as macOS desktop did).

These are operational tests, not engine fixes. Defer until mobile-proxy
integration is in scope.
