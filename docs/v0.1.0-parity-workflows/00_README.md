# 00 — README: v0.1.0 Parity Workflows Index

## Purpose

This folder is the deep-research output of the v0.1.0 parity drive: a per-site,
per-vendor, and per-API teardown aimed at outperforming Camoufox v150 and pushing
browser_oxide toward all 126 corpus sites. Every doc cites BO source at `file:line`
and pairs it with external 2026 detection intelligence, ending in ROI-ranked fixes
tagged public-engine `[P]`, vendor `[V]`, or guard `[G]`. The single converging
finding across all docs: v150 does not beat BO on fingerprint fidelity — it wins by
running anti-bot challenge JS to async completion, the live-nav drain gap that the
roadmap's `M-1` lever closes.

## Scorecard (trustworthy same-IP baseline, 2026-05-28)

- Same-IP contested delta (12 sites): **BO 5 / v150 8** → **BO behind by -3**.
- Corpus 126; honest production denominator 125 (`areyouheadless` excluded by design).
- BO routed best-of-4 passing ~107; **~18 non-passing (~19 incl. areyouheadless)**.
- The entire -3 deficit is Stratum A (AWS-WAF cluster + booking + duolingo), one async-drain class.

## Recommended reading order

1. **`01_MASTER_ROADMAP_PATH_TO_126.md`** — START HERE. Synthesis of all 26 docs: executive scorecard, the consolidated ROI-ranked fix table (M-1…TAIL-PIN), the 3-phase plan, and the honest "can we hit 126?" verdict.
2. `external/ENGINE_camoufox_v150.md` — why v150 wins 8/12 and how to copy-or-beat it.
3. `external/VENDOR_awswaf.md` + `sites/SITE_awswaf_cluster.md` + `sites/SITE_booking.md` — the -3 deficit cluster (the drain lever in detail).
4. The remaining `sites/` docs (durability + frontier), then `external/` vendor + network/behavioral context, then the `api/` parity chapters as the cross-cutting mask/coherence reference.

## sites/ — per-site root cause + fix plan

- `SITE_awswaf_cluster.md` — AWS-WAF (amazon-{ca,com,com-au,fr,in,jp} + imdb): challenge.js stub never advances live; classify-as-challenge so poll/retry fires.
- `SITE_booking.md` — not an SPA gap; it is the same AWS-WAF byte-identical challenge.js cluster.
- `SITE_duolingo.md` — reCAPTCHA-Enterprise invisible worker/iframe hydration; needs cross-origin iframe realms (F1/F2/F3).
- `SITE_homedepot.md` — Akamai sec-cpt; BO ~3/5 beats v150 0/5, harden long-timer drain to reliable 5/5.
- `SITE_x_com.md` — cookie-bleed band-aid scores 5/5 on cold path but bypassed by PagePool; needs per-Page cookie partitioning.
- `SITE_wildberries.md` — wbaas in-house PoW; v150 fails WORSE (THIN-39) — a free outright-win via the drain lever.
- `SITE_etsy.md` — DataDome; child-iframe cookie-jar isolation bug is public-fixable, daily-key solver is vendor scope.
- `SITE_bestbuy.md` — Akamai BMP SPA; NO engine passes (incl. Patchright/v150); the "Patchright passes" premise was a data misread.
- `SITE_douyin.md` — ByteDance acrawler `__ac_signature` VM gate; Firefox-vs-Chromium asymmetric; needs native-builtin integrity.
- `SITE_kasada_cluster.md` — canadagoose/hyatt/realtor open frontier; both BO and v150 fail; do K2-DIFF to bound reachability.
- `SITE_diagnostic_and_tail.md` — confirms `areyouheadless` is doubly-excluded by design; audits the ~107 passes for flaky/band-aided sites.

## external/ — vendor + ecosystem intelligence

- `ENGINE_camoufox_v150.md` — reverse-engineers v150's runtime-completeness win (not fingerprint fidelity).
- `ENGINE_chromium_stealth.md` — Patchright/rebrowser/nodriver tier; ~80% of their work is free for BO (no CDP channel); nothing here explains the AWS/homedepot gap.
- `VENDOR_awswaf.md` — `challenge.js` → `aws-waf-token`; blob-URL PoW Web Worker, not a fingerprint gap.
- `VENDOR_akamai.md` — Bot Manager + sec-cpt sensor_data / `_abck` state machine (homedepot/bestbuy/adidas).
- `VENDOR_datadome.md` — etsy/yelp/tripadvisor; 3 public primitives shipped, daily-key WASM solver deferred to vendor.
- `VENDOR_kasada.md` — canadagoose/hyatt/realtor frontier; most doc-08 levers now shipped; honest engine-vs-vendor assessment.
- `NETWORK_fingerprint.md` — Chrome-class TLS/H2 byte-perfect (the moat); the one real leak is the Firefox profile's cross-layer JA4-vs-UA mismatch.
- `BEHAVIORAL_biometrics.md` — mouse/keystroke/scroll motion axis; better shape than stale docs claim; live mouse cycle still bypasses the Sigma-Lambda generator.
- `DETECT_vectors.md` — master detection-vector catalog (CreepJS/BotD/sannysoft/etc.) mapped to BO coverage with leak verdicts.

## api/ — Web API parity deep dives (cross-cutting)

- `API_workers_crypto.md` — Worker/SharedWorker lifecycle, MessagePort routing, blob workers, `crypto.subtle`; the PoW substrate for AWS + duolingo.
- `API_timing.md` — performance/rAF/timers/Observers + the `run_until_idle` drain model that couples to the AWS live-nav finding.
- `API_navigator_hardware.md` — navigator/screen/hardware values + cross-API coherence (deviceMemory clamp, vendor leak, orientation).
- `API_graphics.md` — WebGL1/2, Canvas2D noise policy, WebGPU adapter, OffscreenCanvas.
- `API_audio.md` — AudioContext sampleRate/latency, OfflineAudioContext render hash, DynamicsCompressor math.
- `API_dom_proto_masks.md` — DOM collections + `Function.prototype.toString` integrity sweep; compounds across ~11 vendors (Kasada/AWS/DataDome/Akamai).
