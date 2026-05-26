# v0.1.0 Execution Plan — the working checklist

**This is the action-oriented derivative of `43_STRATEGIC_GAP_ASSESSMENT.md §3`.** Tick boxes as you go. Each fix has: file:line, command to run, expected diff, validation. Cross-link to the source chapter for theory; this doc is for execution.

**Bar to clear** (per `00_README.md` success scorecard): routed best-of-4 median Pass ≥ 115 on the 126-corpus, 3-run aggregated; ≥ 110 on at least one single profile; zero functional regressions.

**Total budget**: 13-16 engineering-days + ~12 h sweep wall-clock.

**Status**: not started.

---

## Pre-flight (Day 0)

Before touching code, capture the pre-fix baseline so every later A/B has a reference.

- [ ] **Pre-flight 1** — Verify all green at current HEAD
  ```bash
  cargo build --workspace 2>&1 | tail -3
  cargo test --workspace -- --test-threads=1 2>&1 | tail -5
  cargo clippy --workspace -- -D warnings 2>&1 | tail -3
  cargo fmt --all -- --check
  ```
- [ ] **Pre-flight 2** — Build release binaries
  ```bash
  cargo build --release -p browser --example sweep_metrics --example classify_stdin
  ```
- [ ] **Pre-flight 3** — Generate the corpus if not present
  ```bash
  python3 -c '
  import re, json
  src = open("crates/browser/tests/holistic_sweep.rs").read()
  pat = re.compile(r"site!\s*\(\s*\w+\s*,\s*\"([^\"]+)\"\s*,\s*\"([^\"]+)\"\s*,\s*\"([^\"]+)\"\s*\)\s*;", re.DOTALL)
  sites = [{"cat":m.group(1),"name":m.group(2),"url":m.group(3)} for m in pat.finditer(src)]
  json.dump(sites, open("/tmp/corpus.json","w"), indent=1)
  print(len(sites))
  ' # expect 126
  ```

---

## The 12-fix execution sequence

Fixes are independent and can land as separate PRs. Recommended order minimizes "fix X depends on fix Y" coupling.

### Fix 1 — WebGL prototype mask sweep (1-2 days, moves 11 vendors)

**Why**: Per `38_VISUAL_AUDIO_FINGERPRINTING.md §5.4`, 11 of 12 anti-bot vendors check `WebGLRenderingContext.prototype` via `Function.prototype.toString`. Current coverage in `crates/js_runtime/src/js/canvas_bootstrap.js:1290-1295` is 7 of ~80 methods. Unmasked methods leak BO source code.

**Files**:
- `crates/js_runtime/src/js/canvas_bootstrap.js:1290-1295` (current mask list)
- `crates/js_runtime/src/js/stealth_bootstrap.js` (the `_maskAsNative` helper)
- `crates/browser/tests/chrome_compat.rs` (add `native_code_mask_audit` test per chapter 16 §5)

**Steps**:
1. Add `native_code_mask_audit` test that enumerates every prototype method on every globalThis interface, calls `String(proto.method)`, asserts `=== "function name() { [native code] }"`. Run it; capture the failure list.
2. For each failure on `WebGLRenderingContext.prototype`, add `_maskAsNative(WebGLRenderingContext.prototype, '<methodName>')`.
3. Re-run the audit test. Iterate until WebGL section passes.
4. Run gap-corpus (chapter 14 §L2) to confirm no regressions; ideally see flips on Kasada-class sites (canadagoose blob count drops).

**Validation**:
```bash
cargo test -p browser --test chrome_compat native_code_mask_audit -- --ignored --test-threads=1 --nocapture
# Compare blob counts before/after on canadagoose (per chapter 08)
```

**Expected impact**: ~3-5 vendors stop scoring us on Function.toString sfc/sdt fields (Kasada specifically).

### Fix 2 — WebGL per-profile golden snapshot (1 day, moves 11 vendors)

**Why**: Per `38 §5.5`, each stealth profile must produce a CONSISTENT and KNOWN-GOOD WebGL parameter set (RENDERER, VENDOR, UNMASKED_*, extension list). Currently injected per-profile but no golden snapshot enforces consistency.

**Files**:
- `crates/stealth/profiles/chrome_148_macos.yaml` (and 3 other profile YAMLs)
- `crates/js_runtime/src/extensions/webgl_ext.rs` (`getParameter` impl)
- `crates/browser/tests/chrome_compat.rs` (add `webgl_param_golden_snapshot` test)

**Steps**:
1. Capture real Chrome 148 macOS WebGL output via Playwright + CDP. Save as `crates/browser/tests/captures/chrome_148_macos.webgl.json`.
2. Repeat for chrome_148 Windows, iPhone Safari 18, Firefox 135.
3. Add `webgl_param_golden_snapshot` test that loads each profile, calls every relevant `getParameter`, asserts equality with the captured JSON.
4. Where BO emits wrong values, fix in `webgl_ext.rs` or profile YAML.

**Validation**:
```bash
cargo test -p browser --test chrome_compat webgl_param_golden_snapshot -- --ignored --test-threads=1
```

### Fix 3 — `Function.prototype.toString` mass mask sweep (2 days, moves 11 vendors)

**Why**: Per `16_STEALTH_FINGERPRINT_AUDIT.md §5` + `08_KASADA_FRONTIER.md` Lever 3 + `41 §4.4`: BO's `Function.prototype.toString` returns the BO source for `attachShadow`, `queueMicrotask`, `fetch`, `HTMLDocument`, `HTMLElement`, etc. Should return `function name() { [native code] }`.

**Files**:
- `crates/js_runtime/src/js/window_bootstrap.js` (`_maskAsNative` calls)
- `crates/js_runtime/src/js/dom_bootstrap.js`
- `crates/js_runtime/src/js/canvas_bootstrap.js`
- `crates/js_runtime/src/js/timer_bootstrap.js`
- All other `*_bootstrap.js` files

**Steps**:
1. Use the audit test from Fix 1, but now check the full prototype enumeration (not just WebGL).
2. Sweep through every failure category by category: HTMLElement methods, Document methods, Window methods, Event constructors, Headers/Request/Response, XHR, Observer constructors, Stream constructors.
3. Add `_maskAsNative` for each.

**Validation**: Same audit test; full pass.

**Expected impact**: Kasada `sfc`/`sdt` blob fields drop in count (per chapter 08 phase 3 inventory).

### Fix 4 — Canvas `toDataURL` golden parity test (2 days, moves 10 vendors)

**Why**: Per `38 §5.6`, 10 of 12 vendors check canvas 2D output hash. Need to confirm pixel-identical output to Chrome on standardized draw sequences.

**Files**:
- `crates/canvas/` (rendering implementation)
- `crates/browser/tests/chrome_compat.rs` (add `canvas_todataurl_parity` test)

**Steps**:
1. Capture real Chrome 148 canvas output for the FingerprintJS + browserleaks + thumbmarkjs standard draw sequences. Save as `crates/browser/tests/captures/canvas_chrome_148.json`.
2. Add `canvas_todataurl_parity` test running each draw sequence in BO, comparing pixel-byte SHA-256 to captured Chrome.
3. Where BO diverges → fix in `crates/canvas/` (text rendering, curves, composite, emoji per chapter 38 §2).

**Validation**:
```bash
cargo test -p browser --test chrome_compat canvas_todataurl_parity -- --ignored --test-threads=1
```

### Fix 5 — Wire keystroke generator (1-2 days, moves 8+ vendors)

**Why**: Per `40_TIMING_BEHAVIORAL.md §3.2` + `26 §3`: Rust keystroke generator exists at `crates/stealth/src/behavior.rs:421-464` (CMU/Buffalo-calibrated LogNormal dwell + bigram-modulated flight) but `humanize.js` NEVER CALLS IT. `_akRecKey` is defined but never invoked.

**Files**:
- `crates/stealth/src/behavior.rs:421-464` (the generator — read its API)
- `crates/js_runtime/src/extensions/?.rs` (add op to call the generator from JS, OR re-export keystroke schedule into JS-side helpers)
- `crates/browser/src/js/humanize.js` (wire into the existing synthetic-event flow)

**Steps**:
1. Add a Rust op (e.g., `op_synth_keystroke_schedule`) that wraps `behavior.rs:421-464` and returns a JS-consumable schedule (`[{key, down_ms, up_ms}, ...]`).
2. In `humanize.js`, on focus events for input fields, call the op + schedule synthetic keystroke events with the returned timings.
3. Validate that synthesized events populate `_akRecKey` per Akamai BMP behavioral tap.

**Validation**:
```bash
# Capture a single-site test that dispatches focus on an input;
# verify _akRecKey buffer is populated with realistic timings.
target/release/examples/sweep_metrics chrome_148_macos /tmp/test_keystroke.json /tmp/out.json
```

**Expected impact**: Akamai BMP + Kasada + Radware IDBA behavioral scores improve.

### Fix 6 — Wire two-level seed (1 day, moves 8+ vendors)

**Why**: Per `40 §5`: two-level seed exists at `crates/stealth/src/behavior.rs:109-115` but `humanize.js` uses `Math.random()` per-page. Effect: visitor across pages looks like N different users (Kasada/Akamai catch).

**Files**:
- `crates/stealth/src/behavior.rs:109-115` (the seed API)
- `crates/js_runtime/src/extensions/?.rs` (op to expose seed)
- `crates/browser/src/js/humanize.js` (consume seed instead of Math.random)

**Steps**:
1. Add op (e.g., `op_behavior_seeded_random`) that returns deterministic-per-session randoms from the BehaviorProfile.seed.
2. Replace `Math.random()` calls in humanize.js with the op.
3. Verify per-session reproducibility: same seed = same synthetic event sequence.

**Validation**: Two cold runs with same profile + same seed produce byte-identical `_akRecMouse` buffers.

### Fix 7 — Wire `performance.timeOrigin` to humanized op (0.5 day, moves 8 vendors)

**Why**: Per `40 §2.6`: `performance.timeOrigin` not currently humanized. Kasada-style probe `origin + now() === Date.now()` catches the inconsistency.

**Files**:
- `crates/js_runtime/src/extensions/perf_ext.rs` (where `op_perf_now_humanized` lives)
- `crates/js_runtime/src/js/window_bootstrap.js` (where `performance.timeOrigin` is defined)

**Steps**:
1. Make `performance.timeOrigin` consistent with `performance.now() + Date.now()` invariant the humanization preserves.
2. Add test: assert `Math.abs((performance.timeOrigin + performance.now()) - Date.now()) < 10` (real Chrome consistency).

**Validation**:
```bash
cargo test -p browser --test chrome_compat perf_origin_now_consistency -- --test-threads=1
```

### Fix 8 — `MessageChannel`/`MessagePort` proper impl (3-5 days, moves 6+ + duolingo)

**Why**: Per `17_WEB_API_PARITY_MATRIX.md` + `41 §4.4`: `MessageChannel`/`MessagePort` is a NO-OP stub at `crates/js_runtime/src/js/window_bootstrap.js:2256-2272`. Blocks recaptcha enterprise (duolingo per chapter 05). Helps every Worker-using vendor.

**Files**:
- `crates/js_runtime/src/js/window_bootstrap.js:2256-2272` (current stub)
- May need a new `crates/js_runtime/src/extensions/message_channel_ext.rs` for backing ops if pure-JS implementation is insufficient
- `crates/browser/tests/chrome_compat.rs` (add `message_channel_paired_ports` test)

**Steps**:
1. Implement `MessageChannel` constructor returning `{port1, port2}` paired.
2. Implement `MessagePort.prototype.postMessage(data)` — delivers to the paired port's message queue.
3. Implement `MessagePort.prototype.start()` + `onmessage` setter / `addEventListener('message')` dispatch.
4. Implement `MessagePort.prototype.close()`.
5. Verify message-passing semantics: ports are paired bidirectionally; messages queued before `start()` are delivered after `start()`; structured-clone semantics for data.
6. Validate against duolingo: recaptcha enterprise.js spawns webworker.js using MessageChannel for token relay. After this fix, duolingo body should grow past 15 KB.

**Validation**:
```bash
cargo test -p browser --test chrome_compat message_channel_paired_ports -- --test-threads=1
# Then duolingo capture
target/release/examples/sweep_metrics chrome_148_macos /tmp/just_duolingo.json /tmp/out.json
# Expect duolingo body > 50 KB
```

**Expected impact**: duolingo flips (Camoufox-only-pass site). +1 strict pass. Plus side benefit for booking + other React SPAs that use MessageChannel.

### Fix 9 — RAF jitter (1 day, moves 7+ vendors)

**Why**: Per `40 §2.3`: `requestAnimationFrame` fires every 16ms via `setTimeout`, ZERO variance. Kasada-class cadence stddev probe catches it.

**Files**:
- `crates/js_runtime/src/js/timer_bootstrap.js` (RAF impl)
- `crates/stealth/src/behavior.rs` (may have a jitter generator already; check)

**Steps**:
1. Add jitter to RAF cadence: instead of exactly 16ms, use 16ms ± small_jitter (e.g., Gaussian σ=0.5ms centered on 16ms, clamped to ≥1ms).
2. Use the two-level seed from Fix 6 for determinism per session.
3. Validate: 60Hz target maintained on average; jitter measurable.

**Validation**:
```bash
# Stats test: 1000 RAF callbacks, assert mean ≈ 16.67ms, stddev > 0.2ms, max < 33ms
cargo test -p browser --test chrome_compat raf_cadence_jitter -- --test-threads=1
```

### Fix 10 — Vendor-detect markers extension (1 hour, detection coverage)

**Why**: Per `18 §4` + `25 §1` + `26 §3.C`: only 3 of 11 vendor headers logged at `crates/browser/src/page.rs:1049-1057`. Missing: `cf-mitigated`, `cf-ray`, `x-akamai-transformed`, `x-perimeterx-id`, `x-iinfo`, `set-cookie:visid_incap_*`, `server:cloudflare`, `via:varnish` (Fastly).

**Files**:
- `crates/browser/src/page.rs:1049-1057` (current marker logger)
- `crates/browser/src/page.rs:2273-2293` (body-content markers — also extend per chapter 18 §4.1)

**Steps**:
1. Extend the `if let Some(...)` chain to log every marker in chapter 18's vendor catalog.
2. Extend body-content marker list per chapter 18 §4.1 (add `_px3`, `_pxhd`, `arkose`, `forter`, `castle`, `imperva`, etc.).
3. No code-flow change yet — just observability.

**Validation**: Run any sweep; check log output contains the expected `[vendor-detect]` lines for known protected sites.

### Fix 11 — reddit `HTMLFormElement.elements` (0.5 day, reddit flip)

**Why**: Per `15 §Q1` (resolved) + `17_WEB_API_PARITY_MATRIX.md §2` + `05_SPA_HYDRATION_CLUSTER.md`: reddit's challenge calls `e.elements.namedItem('solution')`. `HTMLFormElement.prototype.elements` is currently **missing** → `undefined.namedItem(...)` throws TypeError → silently caught at `page.rs:3406` → `__pendingNavigation` never set → no iter 1.

**Files**:
- `crates/js_runtime/src/js/dom_bootstrap.js:1080-1110` (HTMLFormElement section)

**Steps**:
1. Implement `HTMLFormElement.prototype.elements` getter returning an `HTMLFormControlsCollection`-like wrapper.
2. The wrapper must support:
   - Numeric index access (`form.elements[0]`)
   - Length property (`form.elements.length`)
   - `namedItem(name)` method
   - Iteration via Symbol.iterator
3. Add test: load reddit's verify-page HTML, verify `document.forms[0].elements.namedItem('solution')` returns the input element.

**Validation**:
```bash
# Single-site reddit test
target/release/examples/sweep_metrics chrome_148_macos /tmp/just_reddit.json /tmp/reddit_out.json
# Expect: reddit body > 100 KB, iter count > 1
```

**Expected impact**: reddit flips (Camoufox-only-pass site). +1 strict pass.

### Fix 12 — 3-run baseline + validation (~12 h sweep wall + 2 days analysis)

**Why**: Per `14_TESTING_VALIDATION.md §L5`: single-run sweeps are ±5 noise. Multi-run median is the source of truth. This is the v0.1.0 acceptance gate.

**Steps**:
1. **Before fixes**: capture 3-run baseline at current HEAD (post-fixes-A/B/C). Skip if already have one.
2. **After all 11 fixes land** on a branch: capture 3-run sweep on that branch.
3. Aggregate per chapter 14 §L5 (median tag + len + ms per (profile, site)).
4. Compare:
   - Routed best-of-4 median Pass: must be ≥ 115 (bar) and ≥ baseline + 3 (real win)
   - Per-profile median Pass: must be ≥ 110 on at least one profile
   - Per-site regressions: ≤ 1 acceptable (allows for variance)
5. If gate passes: tag `v0.1.0-parity-rc1`. If gate fails: bisect per-fix.

**Commands**:
```bash
mkdir -p /tmp/3run_post
for run in 1 2 3; do
  for profile in chrome_148_macos pixel_9_pro_chrome_148 iphone_15_pro_safari_18 firefox_135_macos; do
    target/release/examples/sweep_metrics $profile /tmp/corpus.json /tmp/3run_post/${profile}_run${run}.json
  done
done
python3 tools/aggregate_runs.py /tmp/3run_post  # per chapter 14 §L5
python3 tools/regression_check.py /tmp/3run_post --baseline ~/projects/browser_oxide_internal/benchmarks/baselines/2026-05-24/
```

Then move the new aggregated baseline into `~/projects/browser_oxide_internal/benchmarks/baselines/<DATE>/` and update `01_CURRENT_STATE.md` with the new headline numbers.

---

## SHOULD-HAVE (5-10 days additional, schedule-permitting)

These don't move the routed-Pass needle directly but raise per-profile quality + close known gaps:

- [ ] **Touch event synthesis on iPhone/Pixel profiles** (~5 days) — `40 §3.4`. Mobile profiles currently emit ZERO touch events.
- [ ] **Cloudflare `cf-mitigated` header detection + iphone parity fix** (1 hour + capture) — `25`. iphone profile recovers 6 sites to parity.
- [ ] **Canvas emoji per-profile golden snapshot** (~2 days) — `38 §2.4`. Reduces canvas-emoji variance across profiles.
- [ ] **JA4 ground-truth capture** (~2-3 days) — `23 §10`, `39 §2`. Closes the chapter-23 outstanding acceptance.
- [ ] **Firefox H2 differentiation** (~2 days) — `39 §3.5`. Currently emits Chrome H2 even on firefox_135_macos profile.

## NICE-TO-HAVE (schedule permitting)

- [ ] **Audio DynamicsCompressor -50dB calibration** (~5-7 days) — `38 §3.3`. Closes 16% gap at CreepJS/Kasada -50dB test case.
- [ ] **Worker-context fingerprint audit** (~3 days) — `41 §4.4` + `15 §Q5`. HIGHEST RISK unknown — discover the gap even if you don't fix it.

---

## EXPLICIT DEFERS (NOT in v0.1.0)

| Item | Reason | Target |
|---|---|---|
| AWS WAF challenge solver (amazon recovery) | Vendor-specific bypass per CLAUDE.md → `vendor_solvers` private | v0.2.0 |
| DataDome WASM-iframe-daily-key solver (etsy recovery) | Same | v0.2.0 |
| Akamai BMP sensor_data v2/v3 restoration | Same | v0.2.0 |
| Kasada K2-DIFF + 16-field error fixes | Open research per chapter 08; months of work | v0.3.0+ |
| V8 snapshot warming | Perf optimization, not Pass-rate | v0.2.0 |
| Parallel cold sweep | Throughput, not Pass-rate | v0.2.0 |
| Profile expansion (safari/windows/linux) | Most already coded per chapter 19; just needs wiring | v0.2.0 or 3 |

---

## How to use this doc

1. **Pick a fix** — start with #1 (lowest coupling), then in any order.
2. **Read the source chapter** — each fix references its `[chapter] §[section]` for theory.
3. **Implement on a feature branch** — one branch per fix; one PR per fix.
4. **Per-PR validation** — at minimum: `cargo test --workspace -- --test-threads=1` + the per-fix validation command above.
5. **After all 11 fixes**: run Fix #12 (3-run validation gate). If green, tag `v0.1.0-parity-rc1`.

## Tracking

| Fix | Status | Owner | PR | 3-run pass impact | Notes |
|---:|---|---|---|---|---|
| 1 — WebGL prototype mask sweep | completed | claude | branch `fix/v0.1.0-fix1-webgl-mask-sweep` @ `06a515e` | pending (Fix 12) | audit 120→0 failures; +0 regressions |
| 2 — WebGL per-profile snapshot | not started | — | — | — | — |
| 3 — Function.toString mass sweep | completed | claude | branch `fix/v0.1.0-fix3-function-tostring-mass` @ `51c8c75` | pending (Fix 12) | audit 270→0 across 67 prototypes; +0 regressions |
| 4 — Canvas toDataURL parity | not started | — | — | — | — |
| 5 — Wire keystroke generator | not started | — | — | — | — |
| 6 — Wire two-level seed | completed | claude | branch `fix/v0.1.0-fix6-two-level-seed` @ `35db6fc` | pending (Fix 12 sweep) | 15 Math.random→_rand sites; new behavior_rand test passes; +0 regressions |
| 7 — Wire performance.timeOrigin | completed | claude | branch `fix/v0.1.0-fix7-time-origin` @ `3532444` | pending (Fix 12 sweep) | drift 230→0.5 ms; new perf_origin_now_consistency test passes |
| 8 — MessageChannel + MessagePort | not started | — | — | — | — |
| 9 — RAF jitter | completed | claude | branch `fix/v0.1.0-fix9-raf-jitter` @ `57b4d7a` | pending (Fix 12 sweep) | seeded Gaussian σ=0.5ms over 16.67ms mean, max<33ms; +0 regressions |
| 10 — Vendor-detect markers extension | completed | claude | branch `fix/v0.1.0-fix10-vendor-detect-markers` @ `775bd2a` | n/a (observability) | +9 vendor header markers, +11 body markers; +0 regressions |
| 11 — reddit HTMLFormElement.elements | completed | claude | branch `fix/v0.1.0-fix11-form-elements` @ `b74c266` | pending (Fix 12 sweep) | new form_elements_collection test passes; +0 regressions |
| 12 — 3-run baseline validation | not started | — | — | — | gate |

Update the status column as PRs land. Keep the doc current.

## Acceptance gate

When all 12 rows above are "completed" AND Fix 12's 3-run aggregation shows:
- Routed best-of-4 median Pass ≥ 115 — tag `v0.1.0-parity-rc1`
- Routed best-of-4 median Pass 113-114 — tag `v0.1.0-parity` (parity declared, not exceed)
- Routed best-of-4 median Pass < 113 — investigate per-fix non-yield, file under `15_OPEN_QUESTIONS.md`, re-prioritize

Per `00_README.md` Success scorecard for the full v0.1.0 checklist (including memory, throughput, stability).
