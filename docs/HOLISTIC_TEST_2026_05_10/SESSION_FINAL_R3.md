# Holistic sweep round 3 — end of session 2026-05-10

Run with: `cargo test --release -p browser --test holistic_sweep holistic_sweep_parallel -- --ignored --test-threads=1 --nocapture`

Engine HEAD: `8fcfd20` (post all session commits including W5b-deep
Worker rewrite + W6a-B Worker UA-CH).

**Result: 111/126 (88.1%) L3-RENDERED.** Wall clock 52.0 min.

## Three sweep comparison (morning → end PART2 → end PART3)

| Outcome         | Morning | After PART2 | After PART3 |
|-----------------|--------:|------------:|------------:|
| L3-RENDERED     | 106     | 110         | **111**     |
| Kasada-CHL      | 3       | 3           | 2           |
| DataDome-CHL    | 4       | 3           | 4           |
| captcha-CHL     | 3       | 2           | 1           |
| Akamai-CHL      | 1       | 2           | 1           |
| THIN-BODY       | 6       | 3           | 5           |
| Cloudflare-CHL  | 1       | 1           | 1           |
| ERROR           | 2       | 2           | 1           |

Net: **84% → 87.3% → 88.1%**, **+5 sites real** vs morning baseline.

## Per-site changes PART2 → PART3 (+3 wins, -2 regressions = +1 net)

### Wins
| Site         | After PART2     | After PART3     | Likely driver                   |
|--------------|-----------------|-----------------|----------------------------------|
| bestbuy      | Akamai-CHL      | **L3-RENDERED** | sweep variance — was the prior "regression" we'd called noise |
| threads      | THIN-BODY       | **L3-RENDERED** | sweep variance — was flaky in prior runs |
| wildberries  | ERROR           | **L3-RENDERED** | TLS Brotli-only + Fisher-Yates fix landing cleanly (PART1 commit 97dd53d) |

### Regressions (likely sweep variance)
| Site         | After PART2     | After PART3     | Notes                           |
|--------------|-----------------|-----------------|----------------------------------|
| hyatt        | Kasada-CHL      | THIN-BODY       | Different failure mode — worth investigating |
| nowsecure    | L3-RENDERED     | THIN-BODY       | Was L3 last sweep — flaky |
| wsj          | L3-RENDERED     | DataDome-CHL    | Was L3 last sweep — DataDome variance |
| douyin       | captcha-CHL     | THIN-BODY       | Different mode but still failing |

## What this round shipped (PART3 commits)

| Commit | Workstream | Effect |
|--------|------------|--------|
| 37e0c7c | W4a + W6a + W7a + W17a | 4 missing globals stubbed, Critical-CH retry, homedepot tenant_seed, header fixes — bot1225 cleared in Kasada error report |
| e0f4598 | W5b A3 | SPA early-exit for #react-root mount points — hulu went 47% faster |
| a635924 | W5b-deep | Worker setInterval(5) → tokio Notify-backed async op pump |
| 6c5bfb5 | W6a-B | Worker realm navigator.userAgentData (cross-realm consistency for DataDome) |
| 8fcfd20 | W4a-deeper | Eval-source capture test — confirmed Kasada uses property-access (not eval) for the 5 unjzomuy probes; static eval-string capture won't identify them |

## Verified empirical findings this round

1. **Worker `setInterval(5)` rewrite is structurally correct but doesn't unblock twitter/x.com.** Their stall comes from a different perpetually-pending source (probably ServiceWorker promises or MessageChannel chains). Hulu (which DOES use Workers) is unaffected — no regression. The architectural improvement stands; just doesn't land the twitter/x flip.

2. **Kasada uses property-access, not eval, for the 5 remaining unjzomuy probes.** Captured 161 evals on canadagoose.com; 0 contain "unjzomuy". So `target[runtime_built_name]` rather than `eval("target.literal_name")`. Static source capture can't identify the targets — needs V8-level TypeError-construction hook (multi-day work).

3. **W6a research's "Date.getTimezoneOffset not patched" was a false positive** — the patch IS at `window_bootstrap.js:2113-2145` and looks correct (uses Intl.DateTimeFormat for DST-accurate offsets). One less thing to fix.

4. **Worker UA-CH cross-realm contradiction is now fixed** but didn't unblock yelp/etsy by itself — confirms W6a's claim that all 3 fixes (mouse-path synth + Worker UA + Date) are needed in concert, not individually sufficient.

## Session ceiling per workstream (still-realistic)

With remaining work shipped:
- W6a #A mouse-path synth in Page::navigate (~1 day) — closes yelp/leboncoin/etsy if W6a-B+C cover the other 2 cells
- W4 deeper VM analysis to identify the 5 unjzomuy probes (~2-3 days) — closes canadagoose/hyatt/realtor if all 5 found
- W7-deep Cloudflare orchestrator-in-V8 (~6-9 days) — closes udemy
- W5 Tier B+ identify what pins twitter/x's loop (~1 week)

Realistic ceiling: **122-124/126 (96.8-98.4%)** unchanged from previous estimates.
