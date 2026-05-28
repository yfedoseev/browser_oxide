# 16 — Decision log

Running log of decisions made during R-FP-AUDIT-2026Q3. Format: dated entry with rationale + outcome.

## 2026-05-27

### FIX-D validation sweep — 5/5 AWS WAF sites still stub

**Sweep:** `target/release/examples/sweep_metrics chrome_148_macos` against `[amazon-com, imdb, amazon-fr, amazon-in, amazon-de]` single-run, post-`a8cc691`.

**Result:**
- amazon-com: 2014 bytes stub
- imdb: 1995 bytes stub
- amazon-fr: 2011 bytes stub
- amazon-in: 2011 bytes stub
- amazon-de: **2011 bytes stub** (was 855KB on round 1 — confirms round 1 was WAF state noise)

**Honest read:** FIX-A + FIX-C + FIX-D + FIX-F shipped (all correctness fixes — cross-realm Sec-CH-UA consistency, AudioContext sampleRate pinning, device-memory quantization, M3 GpuProfile fixture alignment). **0/5 AWS WAF sites flipped.** The 2011-byte stub is the AWS WAF response when challenge.js was SERVED but bailed during execution — `getToken()` never called. So:
- challenge.js loaded fine ✅
- challenge.js ran some of its probes ✅
- challenge.js detected SOMETHING fingerprint-wise and bailed BEFORE issuing a token ❌
- AWS WAF serves the 2011-byte stub when no token POST arrives

The four fixes shipped this session address objectively-wrong values, but apparently NONE of those values is the specific conditional challenge.js bails on. **More speculative fixes are not the right next step** — we need to KNOW which conditional bails.

**Pivot: R-AWSWAF-OFFLINE-PROBE.** Handoff §1.7 spec'd this exact task: capture challenge.js once, spin up in BO's V8 with stubbed navigator/window, instrument every code path leading to `getToken()`. Provides a deterministic fingerprint→decision oracle without burning live IP probes.

This is the right next move. Continuing to ship blind fixes against AWS WAF will keep hitting the same 2011-byte wall.

### FIX-D — apple_m3_macos GpuProfile aligned to captured Chrome 147 M3 fixture (commit `a8cc691`)

**Status:** 🔵 in progress — code complete; 29 chrome_compat webgl tests + new snapshot test pass.

**Root cause:** `crates/stealth/src/gpu.rs::apple_m3_macos()` was inconsistent — it claimed WebGL 1.0 version + GLSL ES 1.0 SLV strings, but the 40-entry extension list MIXED WebGL 1 extensions (ANGLE_instanced_arrays, EXT_blend_minmax, OES_element_index_uint, etc. — all built-in to WebGL 2) with WebGL 2 extensions (WEBGL_blend_func_extended, WEBGL_polygon_mode, WEBGL_multi_draw, etc.). Plus the shared `common_params_desktop()` baseline returned `MAX_VIEWPORT_DIMS=[32767,32767]` and `ALIASED_POINT_SIZE_RANGE=[1,8190]` — but real Chrome 147 on M3 returns `[16384,16384]` and `[1,511]` respectively (captured ground truth: `tests/fixtures/chrome147/captured_macos_arm64.json`).

The fixture was committed but `apple_m3_macos` was never validated against it. `chrome_compat.rs::webgl_param_golden_snapshot_chrome_148_macos` only checked unmasked vendor/renderer strings; everything else drifted.

**Fix:**
- Updated `apple_m3_macos` to be a clean WebGL 2 catalog:
  - `version = "WebGL 2.0 (OpenGL ES 3.0 Chromium)"` (was 1.0)
  - `shading_language_version = "WebGL GLSL ES 3.00 (OpenGL ES GLSL ES 3.0 Chromium)"` (was 1.0)
  - `extensions = [...]` — captured 36-entry WebGL 2 list (replacing 40-entry mix)
- New `apple_m3_params()` helper applies M3-specific overrides on top of `common_params_desktop()`:
  - `MAX_VIEWPORT_DIMS (0x0D3A) = [16384, 16384]`
  - `ALIASED_POINT_SIZE_RANGE (0x846D) = [1.0, 511.0]`
- New snapshot test `apple_m3_matches_captured_chrome_147_fixture` asserts identity strings + extension count + M3-specific param overrides + shader precision. Any drift between preset and fixture will fail the test.
- Doc comment on the preset explicitly notes the WebGL 1 vs WebGL 2 distinction and the canvas_bootstrap.js `getContext("webgl") == getContext("webgl2")` conflation as a follow-up (FIX-D2).

**Validation:**
- `cargo test -p stealth -- --test-threads=1 apple` — 2/2 pass (new snapshot test + existing apple_profile_has_astc_extension)
- `cargo test -p stealth -- --test-threads=1` — 46/46 pass
- `cargo clippy -p stealth -- -D warnings` ✅
- `cargo test -p browser --test chrome_compat -- --test-threads=1 webgl gpu` — 29/29 pass (no regression)
- `cargo build -p stealth -p browser --example sweep_metrics` ✅

**Risk:**
- Other GPU presets (nvidia_rtx_3060_windows, apple_m2_pro_macos, intel_uhd_630_linux) STILL use the unmodified `common_params_desktop()` with the wrong M3-shaped overrides. Their VIEWPORT_DIMS / POINT_SIZE_RANGE may also be wrong per-GPU. Same fixture-validation work needs to happen for them (FIX-D3 across non-macos profiles).
- The webgl1 vs webgl2 context-type conflation in canvas_bootstrap.js means sites requesting `getContext("webgl")` see WebGL 2 values returned — that's an existing bug, not introduced by FIX-D, but worth flagging for FIX-D2.

### FIX-F — Sec-CH-Device-Memory W3-spec quantization (commit `8d8c067`)

**Status:** 🔵 in progress — code complete; net tests pass.

**Root cause:** `headers.rs:317` used `(profile.device_memory as f64).clamp(0.25, 8.0)`. That correctly capped above-spec values (16 → 8) but **emitted unquantized intermediate values** — a profile with `device_memory = 6` produced `sec-ch-device-memory: 6`, which is NOT in the W3 spec set `{0.25, 0.5, 1, 2, 4, 8}`. Real Chrome's `GetApproximateDeviceMemory` rounds DOWN to the largest spec value ≤ device RAM (6 GB → 4; 3 GB → 2; 0.7 GB → 0.5). Sending a non-spec value is a fingerprint tell.

**Fix:** new `quantize_device_memory(gb: f64) -> f64` helper that does the spec-rounding. `headers.rs:325-329` calls it before emitting the header. Two new tests: `device_memory_quantizes_to_w3_spec_set` (function unit test, 14 cases) and `sec_ch_device_memory_emits_quantized_value` (integration test: 16 GB → "8", 6 GB → "4").

**Validation:** `cargo test -p net --lib -- --test-threads=1 device_memory` — 2/2 pass.

**Risk:** zero for all currently-shipped presets (BO's presets use 0/8/16 — all already quantize correctly). Defensive against future presets that accidentally specify non-spec values.

### FIX-C — AudioContext.sampleRate / baseLatency / outputLatency profile-pinned (commit `93c8ed4`)

**Status:** 🔵 in progress — code complete; build/clippy/stealth+js_runtime tests pass; awaiting chrome_compat audio tests + release-build validation.

**Root cause:** `canvas_bootstrap.js:751-762` randomized AudioContext.sampleRate via `Math.random() < 0.80 ? 44100 : 48000`, baseLatency via `0.005 + Math.random()*0.025`, outputLatency similarly. Per-IIFE meant **per page load**. Real Chrome on a given device reports STABLE values across page loads (the sampleRate reflects actual audio output device). Two pages in the same BO `SharedSession` got DIFFERENT sampleRates — AWS WAF / DataDome telemetry POSTs catch this inconsistency. Found in cross-API agent report 2026-05-27.

**Fix:**
- Added `audio_sample_rate: u32` field to `StealthProfile` (`crates/stealth/src/profile.rs`) with default `44100` via `default_audio_sample_rate()`. `#[serde(default)]` keeps legacy YAMLs readable.
- Validation: `audio_sample_rate ∈ {44100, 48000, 96000, 192000}` (the only values real audio output devices report).
- Updated all 10 preset constructors in `presets.rs` to set the field (44100 for most, **48000 for chrome_148_macos** — Apple Silicon native).
- Updated `chrome_148_macos.yaml` to include `audio_sample_rate: 48000`.
- Updated `stealth_ext.rs::op_get_profile_value` to expose the new key.
- `canvas_bootstrap.js:744-797` now reads `audio_sample_rate` from profile. `baseLatency` / `outputLatency` derive deterministically from bits 0-9 / 10-19 of `audio_seed` — looks like real hardware variation, stays stable across page loads.

**Validation:**
- `cargo build --workspace` ✅
- `cargo clippy -p stealth -p js_runtime --all-targets -- -D warnings` ✅
- `cargo test -p stealth -- --test-threads=1` — 45/45 pass
- `cargo test -p js_runtime --lib -- --test-threads=1` — 13/13 pass
- `cargo fmt --all -- --check` ✅
- Pending: `cargo test -p browser --test chrome_compat -- audio sample_rate` (in flight)
- Pending: `target/release/examples/sweep_metrics chrome_148_macos <amazon-com>` single-site sweep (release build in flight)

**Risk:** very low. Default audio_sample_rate of 44100 reproduces the previous 80%-of-the-time path. Only profiles that EXPLICITLY set audio_sample_rate=48000 (chrome_148_macos preset + chrome_148_macos.yaml) change behaviour, and those are the profiles where 48000 is the correct Apple-Silicon-native value.

### FIX-A + FIX-C combined validation sweep — 1/3 AWS WAF sites flipped

**Sweep:** `target/release/examples/sweep_metrics chrome_148_macos` against `[amazon-com, imdb, amazon-de]` single-run, post-`93c8ed4`.

**Result:**
- amazon-com: L3-RENDERED **2011 bytes** (still blocked at AWS WAF stub)
- imdb: L3-RENDERED **1995 bytes** (still blocked at AWS WAF stub)
- **amazon-de: L3-RENDERED 855,735 bytes (FLIPPED)** ✅

**Interpretation:**
- amazon-de PASS at full content is a real, encouraging signal.
- Single-run, no pre-fix baseline run on the same IP/window — could be WAF-state noise per `docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md` (±5 sites variability).
- amazon-com + imdb identical-byte stubs reproduce the prior block exactly — those WAF endpoints have stricter probes than amazon-de.
- The fixes are CORRECTNESS fixes regardless of single-run yield: cross-realm Sec-CH-UA arch/bitness inconsistency was objectively wrong; AudioContext.sampleRate per-load randomization was objectively wrong. They ship.

**Round 2 sweep (post-FIX-F, single-run) against `[amazon-com, imdb, amazon-fr, amazon-in]`:**
- amazon-com: 2014 bytes stub
- imdb: 1995 bytes stub
- amazon-fr: 2011 bytes stub
- amazon-in: 2011 bytes stub
- All 4 stuck at AWS WAF challenge.js bailout. **Round 1's amazon-de flip did NOT reproduce — almost certainly WAF state noise**, consistent with `docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md` (±5 sites variance per single sweep).

**Honest read:** FIX-A + FIX-C + FIX-F address real correctness bugs but are **not sufficient** to flip the 7 AWS WAF sites in single-trial sweeps. The cross-realm Sec-CH-UA bug, the AudioContext.sampleRate randomization, and the device-memory quantization were all wrong and worth fixing — but AWS WAF's challenge.js has additional discrimination logic these fixes don't reach.

**What's still on the table:**
- FIX-D — GpuProfile params (MAX_TEXTURE_SIZE etc.) validation against real Chrome capture. AWS WAF challenge.js reads ~10 WebGL parameters; if our values don't match real Chrome on M3 specifically (vs the common_params_desktop() shared baseline that all 4 GPU presets use), that's the next candidate.
- Canvas noise (FIX-G) — Camoufox v150 disabled it (commit `e4528a2`). Decision pending — research the actual detection vector before deciding.
- Behavioral signal (mouse/keyboard) — out of this audit's scope, but possibly the missing dimension.
- Per-site AWS WAF challenge.js capture (`R-AWSWAF-OFFLINE-PROBE`) — best ROI is probably to just capture amazon-com's actual challenge.js, instrument it, and see which conditional bails. That's the offline-oracle work in the v0.2.0 handoff §1.7.

**Next:** FIX-D (GpuProfile validation) OR pivot to R-AWSWAF-OFFLINE-PROBE for a precise diagnosis. Decision pending after a fresh round of source survey.

### FIX-A — Sec-CH-UA-Arch/Bitness/Wow64 now profile-driven (commit `960b55f`)

**Status:** ✅ landed.

**Root cause** (cross-API agent finding, audit `03_HARDWARE_SPOOFING_DIFF.md`): `crates/net/src/headers.rs:254` derived `Sec-CH-UA-Arch` from `profile.platform` via `cpu_arch_for()`. For `MacIntel` it returned `"x86"`. But `MacIntel` is reported by Chrome on BOTH Intel and Apple Silicon Macs — it's a legacy fossil. Real Chrome on M3 emits `Sec-CH-UA-Arch: arm` while keeping `navigator.platform: MacIntel`. The JS-side `navigator.userAgentData.getHighEntropyValues` reads `profile.cpu_architecture` (the YAML field, set to `"arm"` on chrome_148_macos). So HTTP said `x86` and JS said `arm` — AWS WAF challenge.js cross-checks and rejects.

`Sec-CH-UA-Bitness` was hardcoded `"64"` (same problem — should read profile.cpu_bitness for 32-bit Windows profiles). `Sec-CH-UA-Wow64` was hardcoded `"?0"`.

Profile.rs:78-84 already documented the intent ("Now profile-driven so HTTP Sec-CH-UA-* headers and navigator.userAgentData stay consistent") but headers.rs had never been updated to follow through.

**Fix:** `headers.rs:253-300` now reads `profile.cpu_architecture` / `profile.cpu_bitness` / `profile.ua_wow64` directly. Deleted the obsolete `cpu_arch_for()` helper. Replaced `cpu_arch_recognizes_arm` test with two new tests that assert profile fields are propagated correctly.

**Validation:**
- `cargo build -p net` ✅
- `cargo clippy -p net --all-targets -- -D warnings` ✅
- `cargo test -p net --lib -- --test-threads=1 headers` — 24/24 pass
- `cargo fmt --all -- --check` ✅
- Site impact: pending L5 sweep against 7 AWS WAF sites (amazon-ca/com/com-au/fr/in/jp + imdb).

**Risk:** none — the change makes BO's behaviour MATCH what `profile.cpu_architecture` already declared. Profiles that didn't explicitly set `cpu_architecture` get the `default_cpu_architecture()` = `"x86"`, which is identical to the previous hardcoded behaviour for non-Apple-Silicon profiles. **No behavior change for chrome_148_linux / chrome_148_windows.** Only chrome_148_macos (and any future Apple-Silicon-explicit preset) gets the correct `arm`.

### Camoufox v150 source survey

**Decision:** clone `daijro/camoufox @ v150.0.2-beta.25` to `/tmp/camoufox_src` (1.5 GB). Use it as read-only reference for diff + grep. **Do NOT copy verbatim** — Camoufox is MPL-2.0, browser_oxide is MIT/Apache. Inspiration only.

**Outcome:** the source survey resolved several things:
- `additions/camoucfg/MaskConfig.hpp` — the env-var JSON → typed-getter machinery.
- `patches/*.patch` — 32 unified diffs against vanilla Firefox. The hardware-relevant 9 are: `navigator-spoofing`, `screen-spoofing`, `webgl-spoofing`, `audio-context-spoofing`, `audio-fingerprint-manager`, `fingerprint-injection`, `media-device-spoofing`, `font-list-spoofing`, `anti-font-fingerprinting`.
- `pythonlib/camoufox/fingerprint-presets-v150.json` — 67 macOS + N linux + N windows presets. Each preset has `{navigator, screen, webgl}` keys.
- `pythonlib/camoufox/webgl/webgl_data.db` — SQLite catalogue (not yet inspected; expected to contain per-GPU `getParameter` value tables).

**Key insight:** v150 stays as Firefox identity. Browser_oxide claims Chrome 148. v150's success on AWS WAF sites does NOT come from UA-spoofing; it comes from **internal hardware-coherence** + Firefox's natural cross-API behaviour. Therefore BO's gap is hardware-coherence within a Chrome identity claim.

### Camoufox v146 → v150 git-log lineage

**Outcome:** unshallowed the clone. Got the relevant commits:

| Tag | Date | Commit | Significance |
|---|---|---|---|
| v146-hardware | 2026-03-14 | `c6a6c20` | "Camoufox 2.0: Hardware Spoofing + Python Package Updates" — the big lineage. Per-context spoofing across NavigatorManager, WebGLParamsManager, ScreenDimensionManager, AudioFingerprintManager + the critical bug fix #443 (v135 patched only WorkerNavigator, not main-window). |
| (v147-149 = build fixes for Firefox 146 compatibility) | | | non-functional |
| v150.0.2-beta.25 | 2026-05-11 | `0ac611c` | Windows support. |
| v150.0.2-beta.25 (during) | 2026-04-14 | `e4528a2` | **Disabled Canvas Noise.** Implication: BO should review canvas-noise injection (`crates/canvas/src/canvas2d.rs:1092-1145` + `webgl_render.rs:407-445`). |
| v150.0.2-beta.25 (during) | 2026-04 | `b7f6c8d` | "Spoof system-ui font resolution to match target OS" (#599) |
| v150.0.2-beta.25 (during) | 2026-04 | `5f3c1f2` | "Allow disabling font spacing perturbation (seed=0 no-op, matching audio)" |
| v150.0.2-beta.25 (during) | 2026-04 | `88f5b0c` | "Fix worker timezone leak when using geoip/proxy" |
| v150.0.2-beta.25 (during) | 2026-04 | `a7becc0` | "Fix brotli/zstd decompression broken by Accept-Encoding override" |

### Decisions deferred to follow-up audits

- **Canvas noise** (`08_CANVAS2D_DIFF.md` open): Camoufox v150 disabled it. BO has 5% per-pixel PCG32 jitter on both 2D + WebGL canvases. Statistical clustering risk. **Cost/benefit pending:** disabling makes BO's canvas hash deterministic per-device-class — which is what real Chrome does, but it removes a defense against canvas-fingerprint tracking. The right answer is probably: **off by default, on as opt-in via profile flag**. File as FIX-G.
- **AudioContext.sampleRate randomization** (FIX-C, see `03_HARDWARE_SPOOFING_DIFF.md §implications`): currently `Math.random() < 0.80 ? 44100 : 48000` at `canvas_bootstrap.js:751`. Real Chrome on a given device returns the SAME sampleRate across page loads. With BO's SharedSession, two pages in the same session see DIFFERENT sampleRates → telemetry-detectable inconsistency. **Fix:** derive deterministically from `audio_seed` (already wired) so the value is stable per-profile but still distributed-random per-instance.
- **Sec-CH-UA brand order randomization** (the cross-API agent flagged it): HTTP sends fixed `[Google Chrome, Not.A/Brand, Chromium]` order; JS `navigator.userAgentData.brands` returns a randomized shuffle. Need to confirm real Chrome behaviour — if Chrome randomizes both independently per-emission (Chrome's GREASE protocol does this), they don't need to match. If real Chrome randomizes ONCE per page and uses the same order for both surfaces, BO is wrong. **Decision pending** — capture real Chrome 148 output and compare.

## How to add a new entry

```
### YYYY-MM-DD — <name> (commit `<sha>`)

**Status:** ⬜ pending | 🔵 in progress | ✅ landed | ❌ dropped

**Root cause:** <what we observed + measurement that proved it>

**Fix:** <file:line + behaviour change>

**Validation:** <build/clippy/fmt + L5 sweep delta>

**Risk:** <regression surface, mitigation>
```
