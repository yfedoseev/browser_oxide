# Session Handoff PART 3 — 2026-05-10 (verification + remaining gaps)

Continuation of `HANDOFF_2026_05_10_PART2.md`. This part captures the
empirical verification of the round-2 fixes and lays out the precise
remaining work needed to flip each still-failing site.

## Verification results (focused re-test post-PART2 commits)

### W17a + W7a verification (commits 37e0c7c)
Ran focused re-test against the 4 sites where these fixes should help:

| Site | Outcome | Body | Time | Change vs morning baseline |
|------|---------|-----:|-----:|----------------------------|
| homedepot | Akamai-CHL | 2,679 b | 5 s | unchanged |
| udemy | CF-CHL | 476,580 b | 60 s | unchanged |
| yelp | DataDome-CHL | 1,450 b | 60 s | unchanged |
| etsy | DataDome-CHL | 1,450 b | 60 s | unchanged |

**The header / Critical-CH / config-table fixes are in the wire, but
the underlying challenge mechanisms still reject us.** This is consistent
with the W6a research finding that DataDome's rejection is multi-signal
(empty mouse path, Worker UA-CH contradiction, Date/Intl mismatch — all
3 in addition to CH negotiation), and with the Cloudflare research
finding that Managed Challenge runs an orchestrator JS that we'd need
to execute fully (not just send the right headers).

So shipping these fixes was correct (necessary), but not sufficient.
The deeper fixes per W6a/W7 docs are needed for these sites to flip.

### W4a Kasada stub verification (commits 37e0c7c)
Re-ran `kasada_error_blob_capture`, decrypted blob #0:

- Total error fields: 13 (same count as pre-fix)
- **`bot1225` field cleared** — this was the AGGREGATE walker probe
  that fired when any sub-probe threw. Now it returns a real value,
  meaning at least one of the 4 stubs (PressureObserver,
  MediaSourceHandle, DocumentPictureInPicture, UserActivation)
  matched a real Kasada probe target.
- Individual probes csc / kl / dpv / smc.o / esd.cpt **still throw**
  the unjzomuy TypeError — meaning the agent's per-probe target
  guesses (Cookie Store / Keyboard.lock / DocumentPiP /
  MediaSourceHandle / PressureObserver) don't match what those probes
  actually read in our engine.

Verified all 11 stubs are present at runtime (PressureObserver,
PressureRecord, MediaSourceHandle, DocumentPictureInPicture,
documentPictureInPicture, UserActivation, navigator.userActivation,
cookieStore, Keyboard, navigator.keyboard, navigator.keyboard.lock).
None of them is the bottleneck for the 5 still-throwing probes.

### W5b A3 verification (commit e0f4598)
Re-ran 3 SPA sites:

| Site | Before | After | Change |
|------|--------|-------|--------|
| twitter | THIN 69b in 150s | THIN 69b in 119s | -31s wall but still THIN |
| x.com | THIN 69b in 150s | THIN 69b in 116s | -34s wall but still THIN |
| hulu | L3 1.2MB in 75s | L3 1.2MB in 40s | **-35s (47% faster)** |

A3 helps on sites whose React mount actually gets populated (hulu,
khanacademy, h&m, yandex.ru). It does NOT help twitter/x.com because
their React mount NEVER gets populated within the deadline — confirms
the W5b finding that the Worker `setInterval(5)` polling pins
`is_pending=true` and React's hydration microtasks never fire.

## What the remaining 16 failing sites really need

The pass-rate ceiling per site, with realistic effort estimates:

### Engine-fixable (10 sites = ceiling of +9 over 110/126 = 119/126 = 94%)

1. **twitter, x.com (THIN-BODY)**: W5b-deep — replace Worker `setInterval(5)`
   at `window_bootstrap.js:1633` with op-backed message pump. Estimated
   ~30 LOC + ~100 LOC Rust op + per-page Notify wiring. Single highest-
   leverage remaining fix.

2. **canadagoose, hyatt, realtor (Kasada-CHL)**: Identify the 5 remaining
   unjzomuy probe targets via VM-level tracing. Need to instrument V8's
   eval-string handling to capture what string the Kasada VM passes to
   eval before each TypeError. ~1 day Rust work + per-probe stub.

3. **yelp, leboncoin, etsy (DataDome-CHL)**: Three W6a fixes (~2 weeks
   total) — mouse-path event synthesis on `Page::navigate`,
   Worker-realm `navigator.userAgentData` (~30 LOC), `Date.getTimezoneOffset`
   patch (~150 LOC). All three need to ship to flip these sites.

4. **udemy (CF-CHL)**: W7-deep — run Cloudflare's orchestrator JS to
   completion in our V8. Per the research doc, ~6-9 days (the Critical-CH
   retry I shipped in PART2 is just the first prerequisite).

### Out-of-engine (3 sites)
5. **douyin, spotify, sometimes iphey (captcha-CHL)**: reCAPTCHA-gated.
   Real Chrome from any non-residential IP gets the same. Need
   captcha-solving service or aged session cookies.

### Misc / flaky (3 sites — neither bucket)
6. **wsj, threads, sometimes others**: flaky between sweeps; engine is
   correct, content delivery is variable. Not a code workstream.

## Final session count

- **24 commits** (a1c0735 → 0728243 → working tree clean)
- **3 holistic sweeps** run: morning baseline (106/126), end-of-session (110/126), focused-retests-only
- **Estimated next-session impact**: with W5b-deep ship + the 5 remaining
  unjzomuy probes identified + W6a #B Worker UA-CH, projected pass
  reaches 116-118/126 (92-94%) from today's 110/126 (87.3%).

## What to verify next session before any new work

1. Re-run `holistic_sweep_parallel` to confirm 110+ baseline holds
   (none of this session's fixes regressed).
2. Re-run `kasada_error_blob_capture` and decrypt to confirm bot1225
   is still cleared and no new fields appeared.
3. Run `BOXIDE_EVENT_LOOP_PROFILE=1 cargo test --release ... twitter` to
   confirm the Worker `setInterval(5)` is the dominant pending future
   (the W5b instrumentation will dump it as the top-10 hottest tick).

## How to start W5b-deep (priority next-session task)

1. Read `crates/js_runtime/src/extensions/worker_ext.rs:344-366`
   (current `op_worker_post_to_worker` and `op_worker_poll_from_worker`).
2. Switch the per-worker `from_worker` channel from
   `std::sync::mpsc::channel` to `tokio::sync::mpsc::unbounded_channel`
   (lines 217 / 247 in the same file).
3. Add a new async op `op_worker_await_message(worker_id) -> String`
   that does `recv().await` on the tokio receiver. Returns `""` when
   the worker dies.
4. In `window_bootstrap.js:1633`, replace the `setInterval(5)` polling
   loop with a recursive Promise chain:
   ```js
   const _drainOnce = () => {
       _wops.op_worker_await_message(self._id).then((raw) => {
           if (!raw) return; // worker died
           // ... existing payload handling ...
           _drainOnce(); // chain next await
       });
   };
   _drainOnce();
   ```
5. Verify with `BOXIDE_EVENT_LOOP_PROFILE=1` against twitter.com that
   the perpetual `pending_timers > 0` is replaced by `pending_ops` that
   only spike when there's actual work.
