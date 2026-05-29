# 00 — browser_oxide behavioral workflows: index + thesis

This directory answers one question: **why can a human open bestbuy / yelp /
Kasada-protected stores in a real Chrome from a given IP, while browser_oxide
cannot from the *same* IP — and exactly what engine change closes the gap?**

## Thesis (one line)

For any site a real browser opens from a given IP, BO must too. Holding the IP
constant, the gap is **API-surface completeness + behavioral simulation +
`isTrusted` authenticity** — NOT the IP, except a small set of genuine geo/IP
cases (ozon) that are flagged honestly and separately. Never assert "IP-bound"
without a captured hard block from a **no-CDP real Chrome at the exact benchmark
IP** (the 2026-05-15 CDP-confound trap).

## The layered plan (headline)

- **Layer 1 — Input-event APIs + native trusted-event minting (THE FOUNDATION).**
  Make `isTrusted` an unforgeable native-masked prototype accessor backed by a
  page-unreachable WeakSet, add `op_dispatch_trusted_event` to mint trusted
  events outside page JS, and fix event-object shape (prototype getters,
  getModifierState, keyCode). Nothing behavioral is creditable until this lands —
  a perfect trajectory carrying a forgeable `isTrusted` own-prop still fails. This
  is BO's **structural moat**: an in-process engine can mint native trusted events
  where a CDP tool cannot.
- **Layer 2 — The behavioral engine.** The Σ-Λ mouse / bigram keystroke /
  wheel_burst math is **already shipped and best-in-public-tier**; wire it into
  `humanize.js` from nav start (live mouse off `_lerp` onto the Rust generator,
  session seed, proactive keystroke, scroll cadence, click/visibility) — wiring +
  scheduling, not new algorithms.
- **Layer 3 — Per-site behavioral payloads.** bestbuy Akamai bmak (public input
  entropy; private encoder), yelp DataDome (public Path A = dodge the slider via
  the etsy cookie-jar cluster; the shown slider is private), Kasada (public
  ingredients; private `/tl`), mobile touch.
- **Layer 4 — Full API-surface completion.** Narrow gaps only: Generic Sensor
  API (mobile bmak), `ondevicemotion/orientation` Window handlers, WebGPU
  coherence, RTCPeerConnection depth.

## Document index

| Doc | Subject | Headline |
|---|---|---|
| `00_README.md` | this index | thesis + layered plan |
| `01_FULL_API_SURFACE_GAP.md` | API completeness | surface far more complete than assumed; real gaps are narrow — Sensor API, WebGPU coherence, RTCPeerConnection depth, devicemotion handlers, bot1225 |
| `02_INPUT_EVENT_APIS.md` | input event classes | classes dispatch but: all-JS (no native dispatch), isTrusted forgeable own-prop, props are own data not prototype getters, getModifierState=false, keyCode=0 |
| `03_BEHAVIORAL_ENGINE.md` | the engine design | math is shipped + best-in-public-tier; the gap is WIRING + scheduling + a trusted-event primitive (FIX-T/A/B/C/E/F/D) |
| `04_ISTRUSTED_AUTHENTICITY.md` | the isTrusted crux | 3 independently-fatal IP/behavior-independent tells; fix = native-masked prototype getter (R1, ~30 LOC) + op_dispatch_trusted_event (R2) — the unused moat |
| `05_BESTBUY_AKAMAI_BEHAVIORAL.md` | bestbuy / Akamai bmak | BO emits a sophisticated synthetic stream into `__akamai_events`; 3 bmak tells remain (isTrusted shadow, linear live mouse, zero mobile touch); encoder is private; ASN floor caveat |
| `06_YELP_DATADOME_SLIDER.md` | yelp / DataDome | the win is rt:'i' self-solve (etsy cluster), NOT a slider solver; a trusted drag is public but the landing-x CV + daily key is vendor_solvers-only |
| `07_IP_RECONCILIATION.md` | IP vs engine | the no-CDP same-IP oracle is the only legitimate disambiguator; bestbuy/yelp lean engine+behavioral, wildberries = IP+engine, ozon = genuine geo |
| `08_PASS_AS_REAL_BROWSER_ROADMAP.md` | **synthesis** | thesis + 4-layer plan + consolidated ranked fix table + isTrusted moat + same-IP protocol + honest ceiling |

## Scope guard

Everything actionable lives in the **public** crates (`crates/stealth`,
`crates/js_runtime`, `crates/browser`) and is generic input-humanization +
API-surface correctness. Per-vendor challenge solving (Kasada `/tl`, AWS
PoW-worker driver, yelp slider CV + daily-key encoder) stays in the private
`vendor_solvers` crate and is flagged at every appearance.

## Honest framing

No fix in this directory flips a *currently-confirmed* v0.1.0 corpus site by
itself — the present corpus blockers are the AWS live-nav async-drain and SPA
hydration. Layer 1+2 is **correctness hardening + the v0.2.0 prerequisite**,
executed after the AWS/SPA work, with `isTrusted` (rank 1) as the single
highest-leverage change. Start with `08`.
