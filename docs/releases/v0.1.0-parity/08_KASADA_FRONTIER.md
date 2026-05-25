# 08 — Kasada frontier (research-bound)

**Status:** NOT in v0.1.0 scope. This doc preserves the research arc so future contributors can pick up where we left off.

**Path note (2026-05-24 update):** All capture artifacts and analysis scripts referenced below (`ab_harness/tl/`, `docs/kasada_ips_analysis/`) live in the **private** `~/projects/browser_oxide_internal/` repo, NOT in this public repo. Per CLAUDE.md scope, Kasada VM analysis + decrypted real-Chrome captures are private. Public contributors who want to reproduce this work need either internal-repo access OR to re-capture from real Chrome themselves.

## Scope

Three sites in the 126-corpus are protected by Kasada and consistently fail across every BO profile:

| Site | BO result | Camoufox result |
|---|---|---|
| canadagoose.com | Kasada-CHL 740 bytes | Kasada-CHL 740 bytes |
| hyatt.com | Kasada-CHL 745 bytes | Kasada-CHL 745 bytes |
| realtor.com | Kasada-CHL 1764-1772 bytes | Kasada-CHL 1772 bytes |

Even Camoufox (the documented Kasada SOTA in the open-source space) only gets **4 of 5** in our `chl-known` category. **canadagoose / hyatt / realtor are the open-source frontier**. Solving them is novel research, not engineering on a known-good design.

## Critical correction (2026-05-16 user-driven)

Prior research had concluded these were "no engine-only path / need behavioural capability + paid farm / unverifiable holistic tail". **That is superseded.** The decisive measurement (re-confirmed from `~/projects/browser_oxide_internal/ab_harness/nocdp/*.windows.txt`):

> `nocdp.sh` real Chrome 147 — opens URL, waits, **zero** mouse/scroll/keyboard, **this datacenter IP** — **passes all three** (window titles = real homepages).  
> Our engine: same IP, same zero interaction → Kasada 429 / `bot1225.b:1`.

What this rules out:
- **NOT IP reputation** — real Chrome on the same datacenter IP passes
- **NOT behavioural absence** — real Chrome with zero behaviour passes (earlier "wire `behavior.rs`, zero-variance" plan is NOT the load-bearing lever)
- **NOT paid-farm requirement** — same IP, no farm

What this leaves (hypothesis): **passive, static engine-vs-real-Chrome-147 surface divergence**. JS env that ips.js measures / how ips.js executes in our V8 vs Chrome's V8 / TLS-JA4 / H2 settings / GPU-canvas hash. The discriminating signal is in the `/tl` POST payload (the Kasada sensor), and we already have ground truth.

See `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/state_2026_05_16_kasada_engine_gap_sharpened.md` for the full sharpened thesis.

## Research arc — what's already done

### Phase 1 (April-May 2026): wrapper cracked

`memory/kasada_wrapper_cracked_and_remaining_leaks.md` (2026-05-10):

Kasada's `/tl` POST body uses a known wrapper:
```
POST_BODY = base64(json({"data": base64(xor(plaintext, b"omgtopkek"))}))
```
9-byte repeating XOR. Key is deployment-wide constant, not per-session. Decryptor: `~/projects/browser_oxide_internal/docs/kasada_ips_analysis/scratch/decrypt_report.py`.

(TEA-CBC IS in the bytecode VM at `kasada_function_bodies.js:129` but for the *primary* `/tl` sensor POST, not the error reports.)

### Phase 2 (May 2026): CSS calc math gap identified

`memory/kasada_real_blocker_css_calc_math.md` (2026-05-10):

The decoded Kasada error report blob #0 (1283 bytes raw text) contains deeply nested CSS `calc()` expressions like:
```css
calc( 1px * ( ( 2.71828 * 0.987654321 - ... )
              + 0.5555555 / sin( sin( 10000.1 * tan( 50000 ) / tan( 20000 )
                                 + 1.0 / pi * 5.0 - 0.1111 ) / 100.0
                          + tan( 30000 + 40000 * 50000 + 0.0001 ) / 9999.9 * pi )
              - ... ) )
```

CSS Values 4 math-function precision fingerprint probe. Chrome implements `sin/cos/tan/asin/acos/atan/atan2/sqrt/pow/exp/log/hypot/abs/sign/mod/rem/round` plus `pi/e/infinity/NaN` constants in `calc()`. Our `CalcExpr` enum at `crates/css_values/src/types/length.rs:43-57` (as of 2026-05-10) only had `Add/Sub/Mul/Div/Negate/Min/Max/Clamp` — none of the math functions.

When Kasada injects a `calc()` with `sin()`, reads the computed value via `getComputedStyle`, and compares against the expected Chrome f64 result, ours differs (or returns `auto`). The probe throws, the VM catches it, POSTs the error report, and Kasada refuses to trust the issued `x-kpsdk-ct` token.

**Status:** The CSS calc fix was partially shipped (commits `347ab0d`, `82bafb6` — verified the 1283-byte raw-text probe disappeared from the captured error blob set, 9 → 8 blobs). Remaining work: the OTHER 16 fields in the decrypted error report (see below).

### Phase 3 (May 2026): 16 remaining error-bearing fields

From the same memory (`kasada_wrapper_cracked_and_remaining_leaks`):

1. **`bot1225` / `csc` / `kl` / `dpv` / `smc` (5 fields, 1 root)**: `TypeError: Cannot read properties of undefined (reading 'unjzomuybtbyyhwwkdpkxomylnab')`. `bot1225` is the single biggest trust-score driver. The 28-char identifier is an obfuscated property from Kasada's VM string table — need to find what Web API surface is being probed and stub it.
2. **`sfc` / `sdt`**: `Function.prototype.toString` returns our literal JS source for `attachShadow`, `queueMicrotask`, `fetch`, `HTMLDocument`, `HTMLElement`, etc. — including the deno_core op name `op_dom_attach_shadow`. Need a sweeping `_maskAsNative` audit across `crates/js_runtime/src/js/*_bootstrap.js`.
3. **`nppm`**: `new structuredClone()` thrown text format check.
4. **`fsc` / `npc`**: error-message text parity for `class X extends Y` when Y is non-constructible (V8 anonymous-class repr `#<C>`).
5. **`esd`**: leaks our private `_loadGpuProfile` helper name through error stacks.
6. **`wse` / `bfe`**: `Function.prototype.toString` thrown text check.
7. **`ao`**: spread non-iterable error text differs subtly from Chrome.
8. **`cbf`**: `Cannot read properties of undefined (reading 'toString')` — some slot we should populate is empty.

### Phase 4 (2026-05-14): SOTA audit fixes shipped

`memory/state_2026_05_14_sota_kasada_audit_fixes.md`: 5 audit-group fixes landed — namedItem source leak, mediaCapabilities, structuredClone error, smc codec aliases, requestVideoFrameCallback, transferControlToOffscreen.

`memory/state_2026_05_14_v3_envelope_measured.md`: union sweep 121 → 120/126 (variance). Architecturally cracked homedepot universal-block on iphone. firefox -4 worth investigating.

### Phase 5 (2026-05-16): K2-DIFF identified as decisive next step

`memory/state_2026_05_16_kasada_engine_gap_sharpened.md` (the canonical current-state):

> The live-oracle reference is **ALREADY captured** — `tl_capture.sh` recorded real Chrome 147's decrypted Kasada `/tl` sensor POST:
> - `~/projects/browser_oxide_internal/ab_harness/tl/hyatt.tl_body.bin` (36 KB) — decrypted plaintext
> - `~/projects/browser_oxide_internal/ab_harness/tl/canadagoose.pcap` (15 MB) + `.keys`
>
> Decisive next experiment = **K2-DIFF**: capture our engine's `/tl` POST for hyatt/canadagoose (we run ips.js in V8) and field-by-field diff vs the real-Chrome capture → the divergent field(s) = the named, fixable engine bug.

**Precede with K1**: gate the parallel Rust `compute_cd_header` PoW off when ips.js self-solves (self-inflicted-`b:1` confound).

K1 was executed (per `memory/state_2026_05_17_unblock_execution.md` — branch `fix/engine-fp-backlog`): "Tier-1 classifier FP fix, Kasada K1 (defer Rust cd to ips.js), homedepot deterministic sec-cpt, re-measure (false blocks 10→7), duolingo real fix (Request.signal + IntersectionObserverEntry). behavior-wiring deferred (honest); **Kasada K2-DIFF = the one scoped remaining item (in-VM plaintext-sensor dump, NOT byte-diff)**."

## The four levers to crack Kasada

In priority order. Each is a multi-day to multi-week effort.

### Lever 1 — K2-DIFF (HIGHEST LEVERAGE)

**Capture our `/tl` POST for hyatt + canadagoose, field-by-field diff vs the real-Chrome reference.**

Both halves of the diff exist:
- Real-Chrome ground truth: `~/projects/browser_oxide_internal/ab_harness/tl/hyatt.tl_body.bin` + `canadagoose.pcap+.keys`
- Our engine output: run BO against hyatt with a test that intercepts the `/tl` POST and decrypts it (use the XOR wrapper from `~/projects/browser_oxide_internal/docs/kasada_ips_analysis/scratch/decrypt_report.py`)

Each divergent field is a named bug we can fix. The 8 categories from Phase 3 are the working list of what to expect; K2-DIFF will tell us if there are MORE.

**Concrete steps:**
1. Build the in-VM plaintext-sensor dump tool (mentioned in `state_2026_05_17_unblock_execution`). It should: intercept `XMLHttpRequest.send` and `fetch` on URL `/tl`, capture the body PRE-encryption (since the JS-side has it in plaintext before XOR), dump to a Rust-side log.
2. Run hyatt → get our plaintext sensor.
3. Diff vs `~/projects/browser_oxide_internal/ab_harness/tl/hyatt.tl_body.bin`.
4. For each divergent field, look up the obfuscated identifier in `~/projects/browser_oxide_internal/docs/kasada_ips_analysis/scratch/decode_strings.js` → identify the Web API → fix.
5. Re-run hyatt. Expected: each fix reduces blob count + error-field count. Pass = no error POSTs + Kasada serves real `<title>` content.

### Lever 2 — CSS calc math completion

Per Phase 2: ship the rest of CSS Values 4 math functions in `crates/css_values/src/types/length.rs`.

**Required (Chrome implements):**
- `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`
- `sqrt`, `pow`, `exp`, `log`, `hypot`
- `abs`, `sign`, `mod`, `rem`, `round`
- Constants: `pi`, `e`, `infinity`, `NaN`

**Concrete steps:**
1. Extend `CalcExpr` enum in `crates/css_values/src/types/length.rs` with each variant.
2. Update the calc parser (`crates/css_values/src/calc.rs`) to recognize each function-call syntax.
3. Update the evaluator with Rust f64 std math (libm-equivalent — should match Chrome `<cmath>`, but verify bit-exactly with a captured Chrome reference).
4. Run `cargo test -p browser --test chrome_compat kasada_error_blob_capture -- --ignored --test-threads=1 --nocapture`; move generated `kasada_error_*.b64` to `~/projects/browser_oxide_internal/docs/kasada_ips_analysis/scratch/`; run `python3 ~/projects/browser_oxide_internal/docs/kasada_ips_analysis/scratch/decrypt_report.py`; verify the calc-precision blob is GONE from the inventory.

### Lever 3 — _maskAsNative sweep (Function.toString leaks)

Per Phase 3 finding 2: our `Function.prototype.toString` returns BO's literal JS source for `attachShadow`, `queueMicrotask`, `fetch`, `HTMLDocument`, `HTMLElement`, etc. Chrome returns `function attachShadow() { [native code] }`.

**Concrete steps:**
1. Audit every `crates/js_runtime/src/js/*_bootstrap.js` for functions that should appear as `[native code]`. The existing `_maskAsNative(proto, ...methodNames)` helper from `window_bootstrap.js` does this; need to apply to every Web API surface.
2. Catalog: enumerate every JS API we expose (DOM, BOM, Web Workers, fetch, IndexedDB, Worker, etc.) → for each, verify its prototype methods are masked. Use a test that does `String(SomeAPI.prototype.someMethod)` and asserts `===` to the `[native code]` form.

### Lever 4 — `bot1225` 'unjzomuybtbyyhwwkdpkxomylnab' Web API stub

The single biggest trust-score driver. The 28-char obfuscated identifier maps to a Web API surface Kasada probes that we don't implement (or implement with the wrong signature).

**Concrete steps:**
1. `grep -r 'unjzomuybtbyyhwwkdpkxomylnab' ~/projects/browser_oxide_internal/docs/kasada_ips_analysis/`
2. Run the string-table decoder: `node ~/projects/browser_oxide_internal/docs/kasada_ips_analysis/scratch/decode_strings.js`
3. Identify the missing API.
4. Stub it in the appropriate `*_bootstrap.js`.
5. Re-run blob capture, verify `bot1225` field disappears.

## If you pick this up — where to start

```bash
# 1. Read the canonical state
cat ~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/state_2026_05_16_kasada_engine_gap_sharpened.md
cat ~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/kasada_wrapper_cracked_and_remaining_leaks.md
cat ~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/kasada_real_blocker_css_calc_math.md

# 2. Verify nothing changed since 2026-05-16
grep -A20 'pub enum CalcExpr' crates/css_values/src/types/length.rs  # is sin/cos there yet?
ls ~/projects/browser_oxide_internal/docs/kasada_ips_analysis/ 2>/dev/null  # is the analysis dir still there?
ls ~/projects/browser_oxide_internal/ab_harness/tl/ 2>/dev/null              # is the real-Chrome reference still captured?

# 3. Set up the K2-DIFF tool — build the in-VM plaintext sensor dump
# (interception path: window_bootstrap.js -> fetch wrapper -> if url contains '/tl' POST, JSON.stringify the request body to globalThis.__lastTlPlaintext)

# 4. Run hyatt with K2-DIFF, get our plaintext
cargo run --release --example sweep_metrics chrome_148_macos /tmp/hyatt_only.json /tmp/out.json

# 5. Diff against the captured real-Chrome
python3 ~/projects/browser_oxide_internal/docs/kasada_ips_analysis/scratch/diff_tl_payload.py ~/projects/browser_oxide_internal/ab_harness/tl/hyatt.tl_body.bin /tmp/our_hyatt_tl.bin

# 6. Each divergent field is a fix. Loop until convergence.
```

## Why this isn't in v0.1.0

Kasada is the **open-source SOTA frontier**. Even Camoufox doesn't solve all three of canadagoose/hyatt/realtor. Cracking it requires:

- Days/weeks of fingerprint-level investigation (not bug-fixing)
- Likely a sequence of 5-15 small fixes across multiple crates (CSS, DOM, JS runtime)
- Each fix needs validation against the captured real-Chrome reference
- Re-testing against the live oracle (which moves — Kasada rotates probes quarterly)

For v0.1.0 the realistic frame is: BO ties Camoufox on Kasada (both 1/3 to 2/3 of these sites). We do NOT need a Kasada solver to beat Camoufox 113 strict — the 10 recoverable sites (per `02_GAP_ANALYSIS.md`) are enough.

## Adjacent files / artifacts

- `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/state_2026_05_16_kasada_engine_gap_sharpened.md`
- `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/kasada_real_blocker_css_calc_math.md`
- `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/kasada_wrapper_cracked_and_remaining_leaks.md`
- `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/state_2026_05_14_sota_kasada_audit_fixes.md`
- `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/state_2026_05_14_v3_envelope_measured.md`
- `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/state_2026_05_15_playwright_ab_decisive.md`
- `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/state_2026_05_17_unblock_execution.md`
- `docs/CANADA_GOOSE_DIAGNOSIS_2026_05_10.md` (full diagnosis writeup)
- `docs/CANADA_GOOSE_DIAGNOSIS_2026_05_10_PART2.md` (16-field inventory)
- `~/projects/browser_oxide_internal/docs/kasada_ips_analysis/` (analysis directory — verify still present)
- `~/projects/browser_oxide_internal/ab_harness/tl/` (real-Chrome captures — verify still present)
- `crates/css_values/src/types/length.rs:43-57` (`CalcExpr` enum — verify if math fns added since 05-10)
- `crates/css_values/src/calc.rs` (calc parser)
- `crates/browser/tests/chrome_compat.rs::kasada_error_blob_capture` (the diagnostic test)
- `crates/js_runtime/src/js/window_bootstrap.js` (`_maskAsNative` helper)
- `crates/js_runtime/src/js/*_bootstrap.js` (audit targets for Lever 3)

## Acceptance — IF pursued post-v0.1.0

- [ ] K2-DIFF in-VM plaintext sensor dump tool built + documented
- [ ] hyatt + canadagoose plaintext captured + diffed vs real-Chrome
- [ ] Each divergent field categorized + fix tracked in a working issue
- [ ] CSS calc math fns (sin/cos/tan + 12 more) shipped, calc-precision blob disappears
- [ ] `_maskAsNative` sweep complete, no JS source leaks in `sfc`/`sdt` fields
- [ ] `bot1225` field disappears (the 28-char API identified + stubbed)
- [ ] hyatt OR canadagoose serves real `<title>` content end-to-end
- [ ] Multi-run stability (3 runs, ≥2 pass)
