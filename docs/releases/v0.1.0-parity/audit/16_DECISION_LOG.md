# 16 — Decision log

Running log of decisions made during R-FP-AUDIT-2026Q3. Format: dated entry with rationale + outcome.

## 2026-05-27

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
