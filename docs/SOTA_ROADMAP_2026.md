# browser_oxide — 2026 SOTA Roadmap

**Created:** 2026-04-26
**Owner:** —
**Scope:** Sequenced implementation plan for closing the P-SOTA gaps catalogued in [`GAPS.md`](GAPS.md) §26–32 (+ deferred §33). Companion to that gap analysis — this doc is the *how* and *when*; GAPS.md is the *what* and *why*.

---

## TL;DR

We are the SOTA *protocol-layer* stealth engine (CDP / TLS / HTTP / `webdriver` / `toString`). We are **not** the SOTA *render-and-behavior* stealth browser yet — Camoufox wins detector panels, CloakBrowser wins render-stack realism. Closing that gap requires three sequenced phases over **~5–7 calendar weeks for one engineer**:

| Phase | Cluster | Calendar | Person-days | Deliverable |
|---|---|---|---|---|
| **1** | JS shims + tests | 1 week | 7–8 | WebAuthn, FedCM, SAB/COI, perf.now jitter, H2 golden test, JA4H test — ships 5 of 7 P-SOTA gaps with no native rendering work |
| **2** | Render stack | 2.5 weeks | 13–19 | wgpu+Lavapipe WebGL execution + audio realtime/Analyser/Biquad/jitter — closes the largest detection surface |
| **3** | Behavioral entropy | 1.5 weeks (MVP) / 2.5 weeks (full) | 7.5 / 12 | Sigma-Lognormal mouse + ChaCha20 RNG + CDP `Input.dispatch*` (MVP); add keystrokes + scroll for full |

After all three phases: detector-panel parity with Camoufox on canvas/WebGL/audio (~95%), BeCAPTCHA RF flag rate <5% on mouse trajectories (vs ~99% on Bezier baselines), all probe-shape WebAuthn/FedCM/COI checks pass, HTTP regressions caught by CI.

What this **still won't beat**: Cloudflare/DataDome's per-customer ML baselines (out of engine scope) and IP/ASN-layer first-touch gates (the "IP is the gate" thesis from `docs/TIER0_KASADA_RESULTS.md` remains true regardless of engine quality).

---

## Recommended-stack table

| Gap | Crate / approach | License | Plug-in point |
|---|---|---|---|
| **#26 WebGL** | `wgpu` 29.x + `naga` (GLSL→WGSL) + Lavapipe Vulkan SwAdapter; keep OSMesa as Linux fast-path | MIT/Apache-2.0 | Replace `crates/canvas/src/osmesa_ffi.rs`; wire `webgl_ext.rs` ops from `canvas_bootstrap.js:339–397`; add `op_webgl_postprocess` driven by `canvas_seed` |
| **#27 Audio realtime** | Keep in-tree Blink port (`audio.rs` is bit-accurate; do **not** swap to `web-audio-api-rs`); add `rustfft` Analyser + closed-form Biquad + per-`audio_seed` compressor jitter (±5 mdB threshold, ±0.1 ms release) | MIT (Blink port BSD-3) | New ops `op_audio_analyser_freq_data`, `op_audio_biquad_response`; extend `op_offline_audio_render` |
| **#28 WebAuthn** | Pure JS shim — replace `_navCredentials = {}` at `window_bootstrap.js:345` with full `PublicKeyCredential`, `CredentialsContainer`, `getClientCapabilities`, ~120 ms `NotAllowedError` rejection | n/a | Two new profile fields: `has_platform_authenticator`, `conditional_mediation` |
| **#29 FedCM** | JS shim alongside #28 — `IdentityCredential`, `IdentityProvider.getUserInfo`, identity branch on `credentials.get` rejecting after ~200 ms with `NotAllowedError` | n/a | Same edit block as #28 |
| **#30 SAB / COI** | Parse COOP/COEP in `crates/net/src/headers.rs`; thread `cross_origin_isolated: bool` into `BrowserRuntimeOptions`; set `RuntimeOptions::shared_array_buffer_store = Some(SharedArrayBufferStore::default())` in `runtime.rs:74`; gate SAB *transfer* (not constructor) on COI | n/a | `headers.rs`, `runtime.rs`, `stealth_ext.rs`, `window_bootstrap.js:705,3789` |
| **#31a perf.now jitter** | LogNormal(μ=ln 8 µs, σ=0.4) clamped [0, 35 µs] over 100 µs grid + Bernoulli(1/1024) Exp tail to ≤1.5 ms | MIT (`rand_distr`) | New `crates/js_runtime/src/extensions/perf_ext.rs`; replace `window_bootstrap.js:3725` |
| **#31b Mouse paths** | **Sigma-Lognormal stroke synthesis** (Plamondon 1995); 2–7 strokes from BeCAPTCHA-Mouse priors (CC-BY 4.0); 125 Hz resampling; pink-noise micro-tremor; Fitts-law-obedient total time | MIT + CC-BY-4.0 (attribution) | New `crates/stealth/src/behavior.rs`; **CDP `Input.dispatchMouseEvent` handler** added to `protocol/src/session.rs` (currently missing — pays double dividends for Playwright/Puppeteer compat) |
| **#31c Keystrokes** | LogNormal dwell/flight + 26×26 bigram-flight matrix derived from CMU + Buffalo benchmarks (ship aggregates only — facts, not copyrightable); 1.5% typo rate with backspace burst | MIT | Rewrite `op_human_typing_delays`; add CDP `Input.dispatchKeyEvent` |
| **#31d Scroll** | Trackpad: `v(t) = v₀ · 0.94..0.98^(t/16ms)` momentum decay at 60 Hz; Wheel: discrete 100 px notches at LogNormal intervals | MIT | New `op_human_scroll_burst` + CDP `Input.dispatchMouseWheelEvent` |
| **#32 H2 golden test** | Capture Chrome 146 SETTINGS+WU+HEADERS via Wireshark; commit as binary fixtures; tokio TCP listener test diffs bytes; HPACK-decode HEADERS to assert pseudo order | MIT | `crates/net/tests/h2_frame_bytes.rs` + `crates/net/tests/fixtures/h2/` |
| **#32b JA4H** | ~80-LOC clean-room computer in `crates/net/src/ja4h.rs` (`#[cfg(test)]`-gated for license safety — see §JA4H caveat); per-profile fixture test; optional `#[ignore]` peet.ws cross-check | License caveat | Test-only |
| **#33 QUIC** (deferred) | `quinn-proto::TransportParameters::write` doesn't expose ordering; needs upstream PR or fork. Cloudflare/Akamai still score TLS+H2 first | — | Tracked, not scheduled |

---

## Phase 1 — JS-shim sprint (1 week, 7–8 person-days)

Closes 5 of 7 P-SOTA gaps with **no native rendering work**. Highest velocity-per-day in the roadmap.

### 1.1 WebAuthn + FedCM (#28 + #29) — 2 days

**Profile additions** (`crates/stealth/src/profile.rs`):
```rust
#[serde(default)] pub has_platform_authenticator: bool,    // Mac/Win desktop -> true; Linux -> false
#[serde(default = "default_true")] pub conditional_mediation: bool, // Chrome 130+ desktop -> true
```
Defaults per preset: macOS/Win desktop = `true/true`; Linux desktop = `false/true`; Android = `true/false`. Wire two new keys in `crates/js_runtime/src/extensions/stealth_ext.rs:17`.

**JS shim** (replaces `window_bootstrap.js:345`'s `_navCredentials = {}` stub) — see GAPS.md §28 for the full sketch with `PublicKeyCredential`, `CredentialsContainer`, `IdentityCredential`, `IdentityProvider`. All masked via existing `_maskAsNative` pattern.

**Tests** (`crates/js_runtime/tests/webauthn_probe.rs`, `fedcm_probe.rs`):
- `typeof PublicKeyCredential === "function"`
- `await PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable()` matches profile bool
- `await navigator.credentials.create({publicKey:{}})` rejects with `NotAllowedError` after ≥100 ms
- `Function.prototype.toString.call(PublicKeyCredential.isUVPAA)` ends with `"{ [native code] }"`
- FedCM: `typeof IdentityCredential === "function"`; `credentials.get({identity:{...}})` rejects after ≥150 ms

### 1.2 SAB + crossOriginIsolated (#30) — 2 days

**Three coordinated changes:**

**A. Parse COOP/COEP** in `crates/net/src/headers.rs`:
```rust
pub struct DocumentPolicy { pub coop: CoopValue, pub coep: CoepValue }
pub fn is_cross_origin_isolated(p: &DocumentPolicy) -> bool {
    matches!(p.coop, CoopValue::SameOrigin)
    && matches!(p.coep, CoepValue::RequireCorp | CoepValue::Credentialless)
}
```

**B. Propagate to runtime** — extend `BrowserRuntimeOptions` (`crates/js_runtime/src/runtime.rs:23`) with `pub cross_origin_isolated: bool`. Add op `op_cross_origin_isolated() -> bool`. Replace `window_bootstrap.js:705,3789`:
```js
Object.defineProperty(globalThis, 'crossOriginIsolated', {
    get: () => ops.op_cross_origin_isolated(), configurable: true });
```

**C. Enable real SAB transfer** in `runtime.rs:74`:
```rust
RuntimeOptions {
    shared_array_buffer_store: Some(deno_core::SharedArrayBufferStore::default()),
    ..Default::default()
}
```

V8 always exposes the SAB constructor; we mirror Chrome by gating only `postMessage`-transfer of SABs to workers in `worker_ext.rs` (throw `DataCloneError` when `!cross_origin_isolated`).

**Tests** (`crates/js_runtime/tests/coi_probe.rs`):
- `cross_origin_isolated=false` → `self.crossOriginIsolated === false`; SAB transfer rejects with `DataCloneError`
- `cross_origin_isolated=true` → `self.crossOriginIsolated === true`; SAB transfer succeeds; `Atomics.wait(new Int32Array(new SharedArrayBuffer(4)),0,0,1)` returns `"timed-out"`
- Plus a `headers.rs` unit test for COOP/COEP parsing matrix

### 1.3 `performance.now()` jitter (#31a) — 1.5 days

```rust
// crates/js_runtime/src/extensions/perf_ext.rs (new)
#[op2(fast)]
pub fn op_perf_now_humanized(#[state] s: &mut PerfState) -> f64 {
    let raw_us = s.origin.elapsed().as_nanos() as f64 / 1000.0;
    let q = (raw_us / 100.0).floor() * 100.0;                       // 100µs grid
    let jitter = s.rng.sample(LogNormal::new(2.08, 0.4).unwrap())  // µ=ln 8
                  .min(35.0).max(0.0);
    let spike = if s.rng.gen_bool(1.0/1024.0) {
        s.rng.sample(Exp::new(1.0/200.0).unwrap()).min(1500.0)
    } else { 0.0 };
    (q + jitter + spike) / 1000.0   // ms, JS contract
}
```

JS shim replaces `window_bootstrap.js:3725`:
```js
_defProtoMethod(_PProto, 'now', function now() {
    return Deno.core.ops.op_perf_now_humanized();
});
```

**Validation:** capture 10 000 samples from real Chrome 130 (chrome.exe `--headless=new` driving a hot loop), commit as `crates/js_runtime/tests/fixtures/perf_now_chrome130.bin`, KS-test our distribution against it. Pass: D < 0.04, modal-step ≈ 100 µs ±5 µs, P99 ≥ 250 µs.

### 1.4 H2 golden test + JA4H (#32) — 2 days

**H2 byte-equivalence test** (`crates/net/tests/h2_frame_bytes.rs`):
```rust
#[tokio::test]
async fn h2_settings_window_headers_match_chrome146() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut sock, _) = listener.accept().await.unwrap();
        let mut buf = vec![0u8; 4096]; let mut total = 0;
        while total < 120 { total += sock.read(&mut buf[total..]).await.unwrap(); }
        buf.truncate(total); buf
    });
    let client = tokio::spawn(async move {
        let tcp = TcpStream::connect(addr).await.unwrap();
        let (mut sender, conn) = net::h2_client::handshake(tcp).await.unwrap();
        tokio::spawn(conn);
        let _ = net::h2_client::send_get(&mut sender, "http://localhost/", "localhost", &[]).await;
    });
    let bytes = server.await.unwrap(); let _ = client.await;

    assert_eq!(&bytes[..24], include_bytes!("fixtures/h2/chrome146_preface.bin"));
    let exp = include_bytes!("fixtures/h2/chrome146_settings.bin");
    assert_eq!(&bytes[24..24+exp.len()], exp);
    // ... WINDOW_UPDATE, then HPACK-decode HEADERS to assert pseudo order
}
```

Capture Chrome 146 frames once via `tshark -r capture.pcap -Y 'tcp.stream eq 0' -T fields -e tcp.payload`. Document URL/UA in `crates/net/tests/fixtures/h2/CAPTURE.md`.

**JA4H computer** — see §JA4H license caveat below before writing this. ~80 LOC, `#[cfg(test)]`-gated, per-profile fixture asserting `lang4` differs and `hdr_hash12` is identical for navigation requests.

### Phase 1 totals

- 5 of 7 P-SOTA gaps closed
- 5 new test files (gives CI regression coverage we don't have today)
- Zero new heavy dependencies
- ~7.5 person-days elapsed

---

## Phase 2 — Render stack (2.5 weeks, 13–19 person-days)

The largest detection-surface gap. WebGL dominates the budget; audio is mostly cleanup.

### 2.1 WebGL execution via wgpu + Lavapipe (#26) — 10–14 days

**Stack choice:** `wgpu` 29.x + `naga` (GLSL→WGSL) + Lavapipe Vulkan SwAdapter on Linux/macOS/Windows. Keep OSMesa as `#[cfg(target_os = "linux")]` fast-path or delete.

**Why not alternatives:**
- **OSMesa-only (current):** No Windows/macOS support. macOS deprecated full OpenGL in 10.14. Dead end.
- **`surfman`:** MIT/Apache itself but pulls Servo subgraph. Skip.
- **Servo's `canvas/webgl`:** MPL — banned by `CLAUDE.md`.

**Critical caveat — per-pixel byte-stability across hosts is not free.** Lavapipe + LLVM ≠ ANGLE D3D11 — same shader produces subtly different bytes (rounding modes, scanline order, MSAA resolve). The honest design is:

> Render with Lavapipe to get *plausible*, *deterministic-per-host* bytes. Then post-process the readPixels buffer through a per-`canvas_seed` permutation that matches the **statistical signature** the chosen `GpuProfile` would produce. Not bit-equal to a real RTX 3060 — just consistent within the claimed profile.

This matches what Camoufox does at the Firefox-fork level.

**Where it plugs in:**

| Layer | File | Change |
|---|---|---|
| Native context | `crates/canvas/src/webgl_render.rs` | Replace OSMesa init with `wgpu::Instance::new(InstanceDescriptor { backends: VULKAN \| METAL \| DX12, .. })` + `RequestAdapterOptions { force_fallback_adapter: true, .. }`. Keep `glow`-shaped public API. |
| FFI module | `crates/canvas/src/osmesa_ffi.rs` | Linux-only fast path, or delete. |
| Cargo features | `crates/canvas/Cargo.toml:11` | `webgl-render = ["wgpu", "naga", "bytemuck"]`; remove `glow` from required deps; ship feature **on by default** for Tier-0 builds. |
| Ops | `crates/js_runtime/src/extensions/webgl_ext.rs` | Already has ~25 ops — extend with `op_webgl_get_uniform_location`, `op_webgl_uniform_matrix4fv`, `op_webgl_buffer_data`, `op_webgl_tex_image_2d` (~25 more for FingerprintJS coverage). |
| JS shim | `canvas_bootstrap.js:163–397` | Constructor calls `op_webgl_create_context`; every stubbed method (`createShader`, `drawArrays`, etc.) routes to the corresponding op. |
| Profile bridge | `crates/js_runtime/src/extensions/stealth_ext.rs` | New op `op_webgl_postprocess(ctx_id, &mut [u8])` applies seed-driven permutation in `webgl_render::reshape_to_profile()`. |

**`#[op2]` signature sketch:**
```rust
#[op2]
#[buffer]
pub fn op_webgl_read_pixels_postprocessed(
    #[state] state: &WebGLState,
    #[smi] ctx_id: i32, #[smi] x: i32, #[smi] y: i32,
    #[smi] w: i32, #[smi] h: i32,
    #[smi] format: u32, #[smi] type_: u32,
    #[smi] canvas_seed: i32,
    unmasked_renderer: &str,
) -> Vec<u8> { ... }
```

**Determinism:** `StealthProfile.canvas_seed` (already at `profile.rs:96`) drives an XOR/permutation on the readback buffer. Same renderer string → same permutation → byte-identical output across runs of the same profile. Different `unmasked_renderer` strings deterministically fork the seed (`hash(seed, renderer_str)`).

**Tests** (`crates/canvas/tests/webgl_fingerprint.rs`):
```rust
#[test]
fn fingerprintjs_canonical_webgl_hash_stable() {
    let profile = stealth::presets::nvidia_rtx_3060_windows();
    let h1 = render_fpjs_webgl_canonical(&profile);
    let h2 = render_fpjs_webgl_canonical(&profile);
    assert_eq!(h1, h2, "deterministic within profile");
    assert_ne!(h1, render_fpjs_webgl_canonical(&apple_m2_pro_macos()));
}
```

Port the FPjs WebGL probe shader from `fingerprintjs/src/sources/canvas.ts` verbatim into `tests/fixtures/fpjs_webgl.glsl`.

**Effort breakdown:**
- 2d wgpu wiring + cross-platform feature flags
- 2d remaining ops (~25 more)
- 3d JS shim full passthrough + brand-checks (`Symbol.toStringTag`, illegal-invocation throws on every method)
- 2d seed-driven postprocess function design + tuning
- 2d FPjs/CreepJS canonical-shader fixture + test harness
- 1–2d cross-OS CI (Lavapipe via MoltenVK on macOS is fiddly)

**Risks:**
1. macOS Lavapipe install is non-trivial (`brew install molten-vk vulkan-loader`). CI must download Lavapipe explicitly. Document in `CLAUDE.md`.
2. Lavapipe is not bit-deterministic across hosts — LLVM codegen differs subtly per host CPU. Mitigation: seed permutation runs *before* hashing, so host noise becomes invisible.
3. `wgpu` adds ~3 MB compile time and ~80 MB build deps. Acceptable next to deno_core's V8 cost.
4. `force_fallback_adapter` not always honored — assert resulting adapter info contains `"llvmpipe"` or `"Microsoft Basic Render"` to catch mistakes.
5. WebGL2-specific (transform feedback, sampler objects) — defer; FPjs doesn't probe.

### 2.2 Audio realtime + Analyser + Biquad + per-seed jitter (#27) — 3–5 days

**Keep the in-tree Blink port.** Do **not** swap to `web-audio-api-rs` — it's spec-compliant but not Blink-bit-accurate; swapping would regress the FPjs hash.

**Plug-in points:**

| Surface | Strategy |
|---|---|
| `OfflineAudioContext.startRendering` | ✅ Already done. Keep `op_offline_audio_render`. |
| `AnalyserNode.getFloatFrequencyData` | Port FFT (use `rustfft`, already a dep). ~80 LOC. |
| `BiquadFilterNode.getFrequencyResponse` | Closed-form bilinear-transform — ~40 LOC. |
| `AudioContext` (real-time) | Wrap `web-audio-api` 1.2 with `default-features = false` (avoid `cpal`, no audio hardware needed); use its OfflineAudioContext internals for Analyser probes. |
| Per-profile compressor jitter | Add `audio_seed`-driven nudge to threshold/release in `op_offline_audio_render` — currently only nudges phase. |

**`#[op2]` additions:**
```rust
#[op2]
#[buffer]
pub fn op_audio_analyser_freq_data(
    #[buffer] time_domain: &[u8],   // f32 LE bytes
    #[smi] fft_size: i32,
    #[smi] smoothing_x100: i32,     // 0..100 → 0.0..1.0
) -> Vec<u8> { /* rustfft → magnitude → dB */ }

#[op2(fast)]
pub fn op_audio_biquad_response(
    freq_in: &[f64], mag_out: &mut [f64], phase_out: &mut [f64],
    filter_type: u32, frequency: f64, q: f64, gain: f64,
    sample_rate: f64,
) { /* closed form */ }
```

**Per-seed jitter** — replace the fixed phase nudge in `op_offline_audio_render`:
```rust
let threshold_jitter = ((seed as f32 * 0.31).sin()) * 0.005;  // ±5 mdB
let release_jitter   = ((seed as f32 * 0.71).cos()) * 0.0001;  // ±0.1 ms
```
This makes the FPjs AudioContext hash differ across `audio_seed` values while staying within Blink's observed cross-machine variance band.

**Tests** (`crates/canvas/tests/audio_fingerprint.rs` — extend existing):
```rust
#[test]
fn fpjs_audio_hash_matches_chrome_131_for_seed_0() {
    let fp = AudioFingerprint::default_fingerprint();
    let sum: f32 = fp.data[4500..5000].iter().map(|x| x.abs()).sum();
    assert!((sum - 124.04347527516074).abs() < 1e-3);  // captured Chrome 131 reference
}
```

**Risks:**
1. `web-audio-api` 1.2 spawns a `RenderThread` for `AudioContext` — using `cpal` requires audio hardware. Mitigation: `default-features = false`; use only OfflineAudioContext internals or take public node types and drive them ourselves.
2. `rustfft` is deterministic across hosts (already used for periodic wave). ✅
3. No FFI risk — pure Rust, MIT.

### Phase 2 totals

- WebGL byte-output and audio realtime hashes return per-profile diversity instead of stub bytes
- ~16 person-days elapsed
- Adds wgpu+Lavapipe + web-audio-api dependencies (all MIT/Apache)

---

## Phase 3 — Behavioral entropy (1.5–2.5 weeks)

The highest-leverage gap for surviving Kasada, PerimeterX (HUMAN), Akamai sensor-data sites — those vendors score the *distribution shape* of human inputs, not just any inputs.

### 3.1 MVP (1.5 weeks, 7.5 person-days)

The 80% subset:

**3.1a `performance.now()` jitter** — already in Phase 1 (#31a). Counts toward MVP coverage.

**3.1b Sigma-Lognormal mouse paths** — 5 days
- New `crates/stealth/src/behavior.rs` with `SigmaLognormalSampler::mouse_trajectory(from, to, target_w, profile, rng) -> Vec<(t_ms, x, y)>`
- Per-stroke parameters from Plamondon 1995 + BeCAPTCHA-Mouse priors:
  - N strokes: round(1.3 · log₂(D/W + 1)) clamped [2, 7]
  - σ ~ Normal(0.25, 0.05); μ ~ Normal(-1.6, 0.2)
  - Inter-stroke Δt₀ ~ LogNormal(μ=ln 90 ms, σ=0.3)
  - Direction θᵢ rotated around target normal with Gaussian(0, 8°)
  - Pink-noise micro-tremor at 2 Hz, 1.5 px amplitude
  - Resample to 8 ms intervals (125 Hz, real USB pointer rate)
  - Total time enforced via Fitts: T = 230 + 166·log₂(D/W+1) ms
- `input_ext.rs::op_human_mouse_path` becomes a thin wrapper
- **CDP `Input.dispatchMouseEvent` handler in `protocol/src/session.rs`** — currently missing entirely. Without this, no Puppeteer/Playwright user benefits. Pays double dividends.

**3.1c Per-session ChaCha20 RNG** — 1 day
- `BehaviorProfile { seed: u64, handedness, mouse_dpi, typing_wpm_mean, typing_wpm_sigma, scroll_style, fitts_b }` added to StealthProfile
- `rand_chacha::ChaCha20Rng::seed_from_u64(profile.seed)` — reproducible tests, deterministic per-session
- Per-call sub-RNG: fold call site into `rng.gen::<[u8;32]>()` so different (from, to) pairs produce different trajectories under the same seed
- **Audit `humanize.js` for `Math.random()` usage** — verify `window_bootstrap.js:920+` reseeds Math.random per profile; if not, replace with `op_seeded_random()` ops

**3.1d Validation against BeCAPTCHA classifier** — 1 day
- Generate 1 000 trajectories of varying D/W in `crates/stealth/tests/behavior_stats.rs`
- Run BeCAPTCHA's published Random-Forest classifier (open-source sklearn) on outputs
- Success: <5% flagged as bot vs their reported 99% on Bezier baselines
- KS-test trajectory features against BeCAPTCHA-Mouse public dataset (CC-BY 4.0, ship as fixture under attribution)

### 3.2 Full (additional 1 week, 4.5 person-days)

**3.2a Keystroke dynamics** (#31c) — 2 days
- LogNormal dwell + LogNormal flight + 26×26 bigram-flight matrix
- Bigram matrix derived from CMU + Buffalo benchmarks via `crates/stealth/build/` script aggregating public CSVs into a `[[f32; 27]; 27]` const (~3 KB; aggregates are facts, not copyrightable)
- 1.5% typo rate with backspace burst at faster cadence (panic correction is real)
- Rewrite `op_human_typing_delays` to return `Vec<(dwell_ms, flight_ms)>`
- CDP `Input.dispatchKeyEvent` handler emits `keydown` at t, `keyup` at t+dwell, next at t+dwell+flight

**3.2b Scroll velocity decay** (#31d) — 1.5 days
- Trackpad: exponential momentum decay at 60 Hz; Wheel: discrete 100 px notches
- `WheelEvent.deltaMode` semantics correct
- `crates/stealth/src/behavior.rs::wheel_burst()` + `op_human_scroll_burst`
- CDP `Input.dispatchMouseWheelEvent` handler

**3.2c Integration tests against BeCAPTCHA classifier** — 1 day

### Risks & open questions

1. **Validation without paying vendors.** BeCAPTCHA's RF classifier is the best public proxy. Direct test: run trajectories against `bot.incolumitas.com`'s mouse-test page (free, scored). Without HUMAN/Kasada API access, A/B against a real Playwright Chromium baseline on the same Kasada-protected URL (we already do this in `tier0_kasada.rs`).
2. **Sigma-Lognormal fitter is non-trivial.** Berio et al.'s iterative LM solver is 300 LOC; failure mode (non-converging fits) needs handling. Use BeCAPTCHA's pre-fitted parameter distributions as Gaussian priors instead of porting the full fitter.
3. **Determinism vs realism.** Same seed must produce *different* trajectories for different (from, to) pairs — fold call site into sub-RNG.
4. **CDP `Input.dispatch*` not implemented today.** Adding it is necessary for Playwright/Puppeteer compat anyway — this work pays double.
5. **Dataset licenses.** BeCAPTCHA-Mouse is CC-BY-4.0 (compatible with attribution in NOTICE). CMU keystroke benchmark — ship only derived numerical aggregates (facts). Buffalo — derived bigram matrix only, with citation in `crates/stealth/data/BIGRAM_PROVENANCE.md`.

---

## JA4H license caveat (read before implementing #32b)

JA4H is patent-pending under **FoxIO License 1.1** (non-commercial only). The algorithm is published in `python/ja4h.py` on the FoxIO repo, but copying the reference is license-encumbered, and any commercial use of JA4H by *us* may infringe.

**Implementation paths in order of safety:**

1. **Skip JA4H entirely**, validate via `tls.peet.ws/api/all` as a remote oracle (no JA4H code in our tree).
2. **Clean-room JA4H in `#[cfg(test)]`** scoped to our regression test — fits FoxIO's "internal testing/evaluation" carve-out. Patent risk persists if we ever ship JA4H computation in a commercial product.
3. **Ship JA4H computation in production** — patent risk; requires legal review.

**Recommendation:** Path 1 for CI (network-gated `#[ignore]` test) **plus** Path 2 for offline test, with a `LICENSE-NOTE.md` next to `ja4h.rs` documenting the carve-out. Do not export the function publicly.

Spec recap (per FoxIO):
```
JA4H = {method2}{ver2}{c|n}{r|n}{hdr_count2}{lang4}_{hdr_hash12}_{ck_hash12}_{ck_val_hash12}
```
- `hdr_count2`: zero-padded count, **excluding** Cookie/Referer/pseudo-headers, capped at 99
- `lang4`: first `accept-language` token, `-`/`;` stripped, lowercased, padded to 4 (`enus`, `ruru`)
- `hdr_hash12`: first 12 hex of `sha256(",".join(header_names_in_request_order_excluding_cookie_referer_pseudo))`
- `ck_hash12` / `ck_val_hash12`: first 12 hex of sha256 of cookie names sorted, and `name=value` pairs sorted

---

## Honest expected outcome after all phases land

- ✅ Detector-panel parity with Camoufox on canvas/WebGL/audio probes (~95% — last 5% is ANGLE byte-exactness which no from-scratch engine achieves without per-shader fixtures)
- ✅ BeCAPTCHA RF classifier flag rate <5% on mouse trajectories vs ~99% on Bezier baselines
- ✅ All probe-shape detection of WebAuthn/FedCM/COI passes (existence + shape checks, not functional auth)
- ✅ HTTP-stack regressions caught by CI golden tests
- ❌ **Still won't beat:** Cloudflare/DataDome's per-customer ML baselines that score traffic *consistency*, and IP/ASN-layer gates on first-touch (the "IP is the gate" thesis from `docs/TIER0_KASADA_RESULTS.md` remains true regardless of engine quality).

---

## Sources

Per-gap deep-research deliverables (this roadmap is the synthesis):
- WebGL: [gfx-rs/wgpu](https://github.com/gfx-rs/wgpu), [LLVMpipe](https://docs.mesa3d.org/drivers/llvmpipe.html), [grovesNL/glow](https://github.com/grovesNL/glow), [FingerprintJS canvas.ts](https://github.com/fingerprintjs/fingerprintjs/blob/master/src/sources/canvas.ts)
- Audio: [orottier/web-audio-api-rs](https://github.com/orottier/web-audio-api-rs), [WebAudio spec issue #1500](https://github.com/WebAudio/web-audio-api/issues/1500)
- WebAuthn: [W3C WebAuthn L3](https://www.w3.org/TR/webauthn-3/), [web.dev client capabilities](https://web.dev/articles/webauthn-client-capabilities)
- FedCM: [Chrome FedCM overview](https://developer.chrome.com/docs/identity/fedcm/overview), [FedCM updates Chrome 143](https://developer.chrome.com/blog/fedcm-chrome-143-updates)
- SAB/COI: [web.dev COOP/COEP](https://web.dev/articles/coop-coep), [deno_core RuntimeOptions](https://docs.rs/deno_core/latest/deno_core/struct.RuntimeOptions.html)
- Behavioral: [BeCAPTCHA-Mouse paper](https://arxiv.org/pdf/2005.00890), [BiDAlab/BeCAPTCHA-Mouse repo](https://github.com/BiDAlab/BeCAPTCHA-Mouse), [CMU Keystroke Dynamics](https://www.cs.cmu.edu/~keystroke/), Plamondon (1995) "A kinematic theory of rapid human movements", [Akamai sensor_data v3 reverse](https://medium.com/@glizzykingdreko/akamai-v3-sensor-data-deep-dive-into-encryption-decryption-and-bypass-tools-da0adad2a784), [klenne/akamai-sensor-data-tools](https://github.com/klenne/akamai-sensor-data-tools), [bot.incolumitas.com](https://bot.incolumitas.com/)
- HTTP fingerprint: [wreq](https://github.com/0x676e67/wreq), [curl-impersonate](https://github.com/lwthiker/curl-impersonate), [FoxIO JA4 spec](https://github.com/FoxIO-LLC/ja4), [Cloudflare JA4 signals](https://blog.cloudflare.com/ja4-signals/)
- 2026 SOTA landscape: [Castle.io: Puppeteer-stealth to nodriver](https://blog.castle.io/from-puppeteer-stealth-to-nodriver-how-anti-detect-frameworks-evolved-to-evade-bot-detection/), [DataDome 2025 Bot Security Report](https://datadome.co/resources/bot-security-report/), [Halluminate BrowserBench](https://github.com/Halluminate/BrowserBench)

Internal:
- [`GAPS.md`](GAPS.md) §26–33 — gap definitions
- [`CAPABILITY_GAPS_2026.md`](CAPABILITY_GAPS_2026.md) — earlier capability audit (this roadmap supersedes its T1.3 audio and T1.4 WebGL sections)
- [`NEXT_STEPS.md`](NEXT_STEPS.md) — site-by-site execution queue
- [`ANTIBOT_RESEARCH_2026.md`](ANTIBOT_RESEARCH_2026.md) — vendor-by-vendor research archive
- [`TIER0_KASADA_RESULTS.md`](TIER0_KASADA_RESULTS.md) — empirical "IP is the gate" findings

---

## When to update this doc

- **On completing a phase:** move the phase's tasks to a "Shipped" subsection at the top, update calendar estimates for remaining phases.
- **On discovering a new SOTA gap:** add as P34+ in `GAPS.md` first, then add a Phase 4 here if it doesn't fit existing phases.
- **On license changes:** the JA4H caveat assumes FoxIO License 1.1 — re-check before any commercial use.
- **On vendor detection-vector shift:** if WebGPU adapter probing or behavioral biometrics escalate to hard gates, re-prioritize Phase 2 vs Phase 3.
- Archive when all P-SOTA gaps are closed and the README claims `18/18 / 71/71` are backed by reproducible IP-disclosed benchmarks.
