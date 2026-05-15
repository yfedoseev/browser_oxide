# Honest Union-Lever Re-prioritisation (2026-05-15)

## Verified from the post-W1 4-profile sweep (definitive)

The 6 sites that never L3-RENDER on **any** profile (the true
union-ceiling blockers, union = 120/126):

| Site | Vendor | Engine-tractable? | Lever |
|---|---|---|---|
| canadagoose | Kasada | engine surface EXHAUSTED | IP-rep + Playwright-MCP A/B (tool unavailable here) |
| hyatt | Kasada | same as canadagoose | same |
| realtor | Kasada | same as canadagoose | same |
| **homedepot** | **Akamai sec-cpt** | **YES** | **W4.2 sec-cpt PoW solver** |
| yelp | DataDome `t:'bv'` | NO (operational) | IP hard-ban; needs Playwright-MCP A/B |
| douyin | regional | NO | China locale+IP, out of scope (PLAN §1) |

## The correction

W3.8 (DataDome interstitial — etsy/tripadvisor/wsj/reuters) was being
pursued as a 4-site union lever. **It is not.** Per-profile sweep:

| Site | chrome | pixel | iphone | firefox | in union? |
|---|---|---|---|---|---|
| etsy | DataDome-CHL | L3 | L3 | L3 | **YES (3/4)** |
| tripadvisor | L3 | L3 | L3 | DataDome-CHL | **YES (3/4)** |
| wsj | L3 | L3 | L3 | DataDome-CHL | **YES (3/4)** |
| reuters | L3 | L3 | L3 | L3 | **YES (4/4)** |

All four are already in the 120 routing union. W3.8 would raise
per-profile robustness (etsy on chrome, tripadvisor/wsj on firefox)
and cut variance — genuinely valuable for reliability — but it
**does not raise the union ceiling**. The byte-parity-verified
encoder + architecture work is sound and retained; its priority is
re-classified from "union lever" to "robustness hardening."

## The only engine-tractable union lever left: W4.2 (homedepot)

Of the 6 true blockers:
- canadagoose/hyatt/realtor: Kasada engine-controllable inputs are
  **exhausted** (audio FP fixed, behavioral jerk fixed, WebGL/TLS/H2
  verified-correct — doc 17). Residual is IP-reputation (structural,
  ~20-30% ML weight) — the decisive next step is the Playwright-MCP
  A/B from this IP, and that tool is unavailable in this environment
  (flagged for the user).
- yelp: `t:'bv'` = blacklist-verified IP hard-ban; no client solve
  helps (03_DATADOME §3). Operational, not engine.
- douyin: regional, explicitly out of scope.
- **homedepot: Akamai serves an `Akamai-CHL` challenge interstitial
  (sec-cpt PoW). bestbuy on the SAME Akamai stack flips GREEN this
  session — proving our core sensor path works — so homedepot is
  specifically the sec-cpt challenge we don't yet solve. This is
  W4.2: ~250 LOC port of the sec-cpt 428 PoW solver from Hyper SDK
  Go. It is the single highest-value, in-environment, engine-tractable
  lever that moves the union (120 → 121).**

## Loop re-prioritisation

1. **W4.2 — Akamai sec-cpt PoW solver (homedepot)** — primary. Moves
   the union. In-environment tractable (Rust port + unit tests, like
   the DataDome crypto). Check `crates/akamai/src/sec_cpt.rs` (exists)
   for current state.
2. W3.8 DataDome interstitial wiring — secondary (robustness, not
   union). Architecture already leverage-maximised (doc 18).
3. Kasada / yelp — blocked on Playwright-MCP A/B (flag for user); no
   further speculative engine hardening (surface exhausted).
4. douyin — out of scope.

This supersedes the prior loop focus on W3.8 as a union lever. Honest,
data-verified, per the goal's deep-research mandate.
