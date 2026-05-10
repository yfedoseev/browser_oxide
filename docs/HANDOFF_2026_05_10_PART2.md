# Session Handoff PART 2 — 2026-05-10 (continued from HANDOFF_2026_05_10.md)

This is the second half of the 2026-05-10 session. The first half is at
`docs/HANDOFF_2026_05_10.md` (covered: 84% → 87.3% sweep, TLS verified
byte-perfect Chrome 147, Kasada wrapper crack, CSS calc math, 21 commits).

This part adds **5 more workstreams progressed in parallel** with 3
research agents reporting and 3 engine fixes shipped.

## What landed this round

### W17a — Akamai homedepot tenant_seed CAPTURED ✅
Used Playwright MCP to navigate homedepot.com, intercepted the
sensor_data POST, extracted:
- **tenant_seed**: `3,420,213` (per the 5th field of the v2 envelope
  `3;0;1;0;3420213;...`)
- **post_path**: `/R8CjSca6_7i6/TepMG7/yyZyaB/1z5kQJkkNz4V0tS1fY/IjUxRBpiDAI/KRkJCEx/PelsB`

Added to `akamai::get_tenant_settings()`. Verified across 2 captured
POSTs in the same session. Should flip homedepot from Akamai-CHL to
L3 next sweep (assuming the rest of the v2 sensor_data envelope works,
which it does for bestbuy).

### W7a — Cloudflare UA-CH negotiation ✅
Two fixes per `docs/RESEARCH_CLOUDFLARE_BYPASS_2026_05_10.md`:

1. **Critical-CH spec compliance**: per W3 Client Hints Reliability spec,
   when a server sends `Critical-CH: <hint-list>`, the client MUST
   immediately retry the request with those hints BEFORE rendering. We
   weren't honoring this. `learn_accept_ch` now also detects Critical-CH
   and `get_with_headers` does a one-shot retry. Cloudflare Managed
   Challenge (udemy) and DataDome both send Critical-CH; without the
   immediate retry they treat us as non-Chrome.

2. **Two missing high-entropy hints added**:
   - `sec-ch-ua-full-version` (singular, deprecated in favor of
     `-full-version-list` but Cloudflare's critical-ch on udemy still
     lists it — verified live via `curl -sI https://www.udemy.com/`).
   - `sec-ch-device-memory` (DataDome's accept-ch demands it on
     yelp/leboncoin/etsy/wsj).

### W4a — Kasada probe inventory + 4 stubs ✅
Per `docs/W4a_KASADA_PROBE_IDENTIFICATION_2026_05_10.md`, the
`unjzomuybtbyyhwwkdpkxomylnab` is a per-session-randomized property
name Kasada writes to a target object during init then reads back. The
target objects we were missing have now been stubbed in
`window_bootstrap.js`:

- `PressureObserver` + `PressureRecord` (Compute Pressure API,
  Chrome 125+, Kasada `esd.cpt` probe)
- `MediaSourceHandle` (Worker-transferable MediaSource wrapper,
  Kasada `smc.o` probe)
- `DocumentPictureInPicture` + `globalThis.documentPictureInPicture`
  (Document PiP API, Chrome 116+, Kasada `dpv` probe)
- `UserActivation` + `navigator.userActivation` (UserActivation
  interface, Kasada `bot1225` walker)

Each is a class with `Symbol.toStringTag` + functional or stub methods
that return defined-but-non-functional results. Probes that read
properties off these no longer throw `Cannot read properties of
undefined (reading 'unjzomuy...')`.

### W6a — DataDome probe-gap matrix (research only)
Per `docs/W6a_DATADOME_PROBE_GAP_MATRIX_2026_05_10.md` (980 lines):

- **Major surprise**: DataDome's `tags.js` v5.6.3 (110 KB live capture)
  does NOT execute Picasso canvas rasterization in the main probe path —
  that's only in the `interstitial.captcha-delivery.com/i.js` which loads
  AFTER the score fails. So our canvas pipeline is ready but currently
  not interrogated.
- **Top 3 actionable gaps** (each days, not weeks — total ~2 weeks):
  1. **Empty mouse-path coords (1 day)**: `crates/stealth/src/behavior.rs`
     exists but isn't auto-invoked on `Page::navigate` to seed mouse
     events before the first DD POST. This is the decisive signal — DD
     scores a 31-feature path vector and an empty array is a tell.
  2. **Worker-realm `navigator.userAgentData` missing (~30 LOC)**: the
     captured tags.js has Worker code that reads
     `navigator.userAgentData ? navigator.userAgentData.mobile : "NA"`.
     Main thread returns `false`, our worker returns `"NA"` — a direct
     cross-realm contradiction. Easy fix in `worker_bootstrap.js:54-77`.
  3. **`Date.prototype.getTimezoneOffset` not patched (~150 LOC)**:
     `Intl.DateTimeFormat.resolvedOptions()` is profile-overridden but
     V8's intrinsic `Date.prototype.getTimezoneOffset` is untouched.
     DD's `tzp` (Date) vs `tz` (Intl) probes detect the mismatch.

### W5b — SPA hydration profile + A3 early-exit ✅
Per `docs/W5b_SPA_HYDRATION_PROFILE_2026_05_10.md` (~360 lines).

**Hottest path identified**: Worker constructor's `setInterval(5)`
polling loop at `crates/js_runtime/src/js/window_bootstrap.js:1633`
pins `is_pending=true` perpetually, so `run_event_loop` never returns
`Poll::Ready` for any page that creates a Worker. This was THE reason
twitter/x.com/khanacademy/etc. waited the full nav budget for nothing.

**Instrumentation shipped**: `BOXIDE_EVENT_LOOP_PROFILE=1` env-gated
profiler in `crates/event_loop/src/lib.rs`. Captures per-tick wall_us +
pending async-ops/timers/intervals/resources via
`deno_core::stats::RuntimeActivityStatsFactory`. Zero behavior change
when disabled (single OnceLock load + branch).

**A3 fix shipped**: SPA early-exit in `Page::navigate`. When
`iter==0` AND body < 50KB AND no challenge marker, also check whether
any common SPA mount point (`#react-root`, `#__next`, `#app`, `#root`,
`[data-reactroot]`, `#main-app`, `#mount-point`) has children. If so,
return early. **Verified to help hulu** (was passing already but now
40s instead of 75s) but **does NOT unblock twitter/x.com** — their
mount point apparently never gets populated by our V8 because the
Worker setInterval blocks microtask drain BEFORE React mounts.

**Next-level fix needed**: A1 from the research doc — replace Worker's
`setInterval(5)` with an op-backed promise queue (~30 LOC, low risk).
This is the single highest-leverage SPA fix; would unblock twitter/x
and any other Worker-using page. Tracked as W5b-deep for next session.

## Empirical results from focused re-test

| Site | Before this round | After this round | Notes |
|------|-------------------|------------------|-------|
| twitter | THIN (69b, 150s) | THIN (69b, 119s) | Faster but still THIN — needs Worker rewrite |
| x.com | THIN (69b, 150s) | THIN (69b, 116s) | Same |
| hulu | L3 (1.2MB, 75s) | **L3 (1.2MB, 40s)** | A3 helped: 47% faster |

## Commits this round (5 new, 26 total session)

```
e0f4598 feat(browser): SPA early-exit on populated #react-root / mount points (W5b A3)
[batch] feat: W4a+W6a+W7a+W17a fixes + 3 research docs
14e776b docs: HANDOFF_2026_05_10 — canonical session writeup
cf60ea5 docs: regressions investigation — neither bestbuy nor threads is real
3dd09c7 docs: end-of-session holistic sweep — 110/126 (87.3%) confirmed
... (21 prior — see HANDOFF_2026_05_10.md)
```

## Updated workstream status

| W#  | Site count | Status                                                             |
|-----|-----------:|---------------------------------------------------------------------|
| W17a | 1 (homedepot) | ✅ DONE — tenant_seed captured + applied; verify on next sweep |
| W7a  | 1 (udemy) | ✅ DONE — Critical-CH retry + 2 missing hints; verify on next sweep |
| W4a  | 3 (canadagoose / hyatt / realtor) | ✅ partial — 4 missing globals stubbed; some unjzomuy probes should now pass; deeper VM RE still open for `kl` and `bot1225` walker |
| W6a  | 3 (yelp / leboncoin / etsy) | research done; **no Picasso needed** (huge); 3 actionable gaps (~2 weeks total): mouse-path synth, Worker UA-CH, Date.getTimezoneOffset |
| W5b  | 2 (twitter / x.com) | A3 early-exit shipped (helps speed); **deeper fix needed** — replace Worker setInterval(5) with op-backed promise queue (~30 LOC); this unblocks both |

**Realistic ceiling — updated projection** (after W5b-deep + W6a Top-3 ship):
**122-124/126 (96.8-98.4%)** — same as PART 1 estimate; the 5
remaining workstreams together get us there.

## Recommended next-session order

1. **Verify the homedepot + udemy fixes** with a focused holistic
   re-run (~30 min). Both are config/header-only changes that should
   flip immediately if the captured values are correct.

2. **W5b-deep — Worker setInterval(5) rewrite (~1 day)**: highest
   single-fix leverage of any remaining workstream. Unblocks twitter/x,
   speeds up every Worker-using page. Replace the polling interval with
   a deno_core op that flushes pending Worker messages on a schedule
   the runtime drives, not a JS-level setInterval.

3. **W6a #B Worker `userAgentData` (~30 LOC, 30 min)**: one of the 3
   DataDome gaps; cheapest first.

4. **W6a #A behavior synthesis (1 day)**: hook `Page::navigate` to
   invoke `crates/stealth/src/behavior.rs` to seed mouse events before
   the first antibot POST.

5. **W6a #C Date.getTimezoneOffset patch (~150 LOC)**: chrono-tz
   lookup for profile timezone; medium-size fix.

6. **Continue W4a deeper VM analysis** for the remaining unjzomuy
   probes (`kl`, possibly `bot1225` walker components). Use the now-cracked
   wrapper to capture fresh error reports after each stub lands; track
   which fields disappear.

## Documents added this round

- `docs/W4a_KASADA_PROBE_IDENTIFICATION_2026_05_10.md` (669 lines)
- `docs/W5b_SPA_HYDRATION_PROFILE_2026_05_10.md` (~360 lines)
- `docs/W6a_DATADOME_PROBE_GAP_MATRIX_2026_05_10.md` (980 lines)
- `docs/HANDOFF_2026_05_10_PART2.md` (this file)

Cross-reference to part 1: `docs/HANDOFF_2026_05_10.md`.
