# 09 — FIX-E3: Linux + Windows + iPhone profile sampler pools

**Status:** ⬜ open. FIX-E + FIX-E2 shipped macOS-only sampler.
**Sites in scope:** improves multi-IP production deployments across all profiles, not specific to any site.
**Effort:** 1-2 days per OS profile.
**Scope:** public engine.

## TL;DR

FIX-E (commit `980a7df`) shipped `chrome_148_macos_sampled()` — an
opt-in per-Page profile sampler for the chrome_148_macos preset.
FIX-E2 (commit `f890b22`) widened that to 3 chip variants (M3 / Pro /
Max). The same sampler primitive should exist for the 3 other shipped
desktop profiles:
- `chrome_148_windows_sampled()`
- `chrome_148_linux_sampled()`
- `iphone_15_pro_safari_18_sampled()` (mobile variant)
- `pixel_9_pro_chrome_148_sampled()` (mobile variant)

Plus the `BROWSER_OXIDE_SAMPLE_PROFILE` flag in `sweep_metrics`
should route to the matching sampler for any of those profiles, not
just macOS.

## Why this matters

- Single-IP debug sweeps DON'T benefit (sweeps in this session showed
  sampler is value-neutral or slightly negative on single-IP). This is
  primarily for **multi-IP production deployments** (rotating proxies).
- AWS WAF / DataDome / Akamai all cluster on `(IP, fingerprint)`
  tuples. A scraper rotating IPs but emitting the same fingerprint
  hits per-fingerprint-rate-limits anyway.
- v150 ships 67 macOS + N linux + N windows variants via BrowserForge.
  BO at HEAD ships 1 preset per OS. The macOS sampler narrows that
  gap; expanding to other OSes narrows it further.

## Current state

Shipped this session:
- `crates/stealth/src/presets.rs::chrome_148_macos_sampled()` +
  `chrome_148_macos_sampled_with_rng()` — sample one of 3 chip families
  × matching cores/RAM/screen pools × random seeds.
- `crates/stealth/src/gpu.rs::apple_m3_family_profile()` — shared
  GpuProfile constructor for M3 / Pro / Max.
- `crates/browser/examples/sweep_metrics.rs` — `BROWSER_OXIDE_SAMPLE_PROFILE=1`
  env var routes to the macOS sampler (only).

Not done:
- Per-OS pools for Windows, Linux, iPhone, Pixel.
- The Camoufox v150 SQLite GPU database
  (`/tmp/camoufox_src/pythonlib/camoufox/webgl/webgl_data.db`) has the
  raw GPU/screen distribution data we'd want.

## Next steps

### Step 1 — Define the Windows pool (~1 day)

Per Camoufox v150's Windows preset distribution (33 entries in their
`fingerprint-presets-v150.json::presets.windows`):
- Common GPUs: nvidia RTX 3060/3080/4070, intel UHD 630, AMD Radeon
- Screens: 1920×1080, 2560×1440, 3840×2160 (4K), some 1366×768 (older
  laptops)
- Cores: 4, 6, 8, 12, 16, 24 (covering low-end laptops to high-end
  desktops)
- RAM: 4, 8, 16, 32, 64 GB

Implement:
- `windows_gpu_profile(family: &str)` per `nvidia_rtx_3060_windows`
  pattern (depends on FIX-D3 [07_FIX-D2-D3-WebGL.md](07_FIX-D2-D3-WebGL.md) for
  per-GPU fixture-validated values).
- `chrome_148_windows_sampled()` — pick one (cores, RAM, screen, GPU
  family) tuple respecting cross-API consistency:
  - 4 cores → low-end GPU (UHD 630 / GTX 1050) + 4-8 GB RAM
  - 8-12 cores → mid GPU (RTX 3060) + 16 GB
  - 16-24 cores → high GPU (RTX 4070+) + 32-64 GB

### Step 2 — Same for Linux + iPhone + Pixel (~1 day each)

Apply same pattern. iPhone and Pixel pools are smaller (fewer
hardware variations); the diversity benefit is smaller too.

### Step 3 — Wire all into sweep_metrics (~few hours)

Update `crates/browser/examples/sweep_metrics.rs` to route
`BROWSER_OXIDE_SAMPLE_PROFILE=1` to the matching sampler based on
`profile_name`. Currently the env var only activates for
`chrome_148_macos`.

### Step 4 — Validate

A multi-IP measurement is the right validator but we can't run that
in CI. Single-IP measurement (as for FIX-E) should NOT regress;
the sampler is opt-in so any single-IP sweep without
`BROWSER_OXIDE_SAMPLE_PROFILE=1` is unaffected.

Multi-IP validation = production deployment metric tracking. Document
the expected impact (per-fingerprint rate limit dispersal) but don't
demand a measurement.

## Dependencies

- FIX-D3 ([07_FIX-D2-D3-WebGL.md](07_FIX-D2-D3-WebGL.md)) ideally lands
  first so the per-GPU `gpu_profile` variants exist before the sampler
  references them.
- The Camoufox SQLite DB for the realistic distribution data.

## Sources / references

- `crates/stealth/src/presets.rs::chrome_148_macos_sampled` + companion
  `_with_rng` — the macOS template
- `crates/stealth/src/gpu.rs::apple_m3_family_profile` — shared GPU
  constructor pattern
- `/tmp/camoufox_src/pythonlib/camoufox/fingerprint-presets-v150.json` —
  raw distribution data per OS
- `/tmp/camoufox_src/pythonlib/camoufox/webgl/webgl_data.db` — SQLite
  GPU data
- audit `16_DECISION_LOG.md` §FIX-E + §FIX-E2 — the macOS implementation lineage
