# Deep Dive: Achieving Ultimate SOTA in Browser Fingerprinting (2026)

This document details the research, algorithms, and implementation strategies required to advance Oxide from structural parity to full behavioral and render-stack realism, defeating ML-based bot mitigation systems (DataDome, PerimeterX/HUMAN, Akamai v13+).

---

## 1. Render Stack Realism (WebGL & Audio Parity)

While structural parity ensures the APIs exist, advanced trackers (like FingerprintJS Pro and CreepJS) execute complex WebGL shaders and AudioContext graphs, hashing the resulting byte output. Identical outputs across different claimed hardware profiles (or output that looks like a known software renderer) result in immediate flagging.

### 1.1 WebGL Fingerprint Spoofing via `wgpu` + Lavapipe
**The Goal:** Produce hardware-consistent, deterministic WebGL renders without requiring actual GPU hardware virtualization per profile.

**Technical Approach:**
1.  **Software Rendering Base:** Replace legacy OSMesa with **Lavapipe** (a CPU-based Vulkan implementation) driven by the `wgpu` crate. This provides a modern, cross-platform WebGL 2.0 capable backend.
2.  **The "Same-Hardware" Problem:** Lavapipe produces the same output bytes regardless of what the `WEBGL_debug_renderer_info` claims.
3.  **Seed-Driven Permutation (The Fix):** 
    *   Instead of modifying the rendering engine deeply, we apply a cryptographic permutation to the `readPixels` buffer immediately before it is returned to the JavaScript context.
    *   **The Algorithm:** Use the profile's `canvas_seed` and the claimed `unmasked_renderer` string to seed a PRNG (e.g., `ChaCha20`). 
    *   Iterate through the pixel buffer and apply subtle, deterministic noise (e.g., ±1 to the least significant bits of RGB channels) or spatial permutations.
    *   **Result:** Every time a specific hardware profile (e.g., "RTX 3080") is requested, the exact same hash is produced. When a different profile is used, the hash changes, perfectly mimicking hardware diversity.

### 1.2 AudioContext Realtime Analysis & Jitter
**The Goal:** Replicate the subtle mathematical inconsistencies (floating-point rounding errors, hardware clock drift) of different audio interfaces.

**Technical Approach:**
1.  **Offline vs. Realtime:** We already handle `OfflineAudioContext`. We must implement the real-time `AudioContext` and nodes like `AnalyserNode` and `BiquadFilterNode` without requiring host audio hardware.
2.  **AnalyserNode Implementation:** Implement `getFloatFrequencyData` using the `rustfft` crate. This is pure math and avoids pulling in heavy audio I/O dependencies.
3.  **Biquad Filter:** Implement closed-form bilinear-transform calculations for the frequency response.
4.  **Per-Profile Jitter Injection:**
    *   Audio fingerprints usually run an oscillator through a compressor.
    *   Use the profile's `audio_seed` to deterministically alter the compressor's parameters at the micro level.
    *   *Algorithm:* `threshold_jitter = sin(seed * 0.31) * 0.005` (±5 mdB). `release_jitter = cos(seed * 0.71) * 0.0001` (±0.1 ms).
    *   This forces the final audio buffer hash to vary realistically across profiles while remaining stable for a single profile.

---

## 2. Behavioral Entropy (Humanizing Inputs)

Structural stealth gets you through the initial gate; behavioral stealth keeps you alive. Systems score the *kinematics* of mouse movements and the *cadence* of typing. Linear interpolation or simple Bezier curves are instantly flagged (often >99% confidence for bots).

### 2.1 Sigma-Lognormal Mouse Trajectories
**The Goal:** Generate mouse movements that obey human biomechanical constraints (Fitts' Law) and muscle activation profiles.

**Technical Approach:**
1.  **The Kinematic Model:** Implement the **Sigma-Lognormal (ΣΛ)** model (Plamondon 1995). Human movement is modeled as the overlapping sum of lognormal velocity profiles (individual muscle impulses).
2.  **Parameter Generation:**
    *   **Number of Strokes:** Derived from Fitts' Law. $N_{strokes} = \text{round}(1.3 \times \log_2(D/W + 1))$, clamped between 2 and 7.
    *   **Inter-stroke Timing:** Time between muscle impulses $\Delta t \sim \text{LogNormal}(\mu=\ln(90\text{ms}), \sigma=0.3)$.
    *   **Trajectory Variation:** Add angular Gaussian noise to the direction of each stroke to create natural arcs and overshoots.
3.  **Micro-Tremor:** Humans cannot hold a mouse perfectly still. Superimpose pink noise (1/f noise) at ~2Hz with a 1-2 pixel amplitude over the entire trajectory.
4.  **CDP Integration:** Expose this directly in the engine via a new CDP command or by overriding `Input.dispatchMouseEvent`. This allows standard Puppeteer/Playwright scripts to automatically inherit human-like movements without modifying the scraper code.

### 2.2 Keystroke Dynamics
**The Goal:** Simulate the complex timing of human typing, including finger travel time, key dwell time, and natural error correction.

**Technical Approach:**
1.  **Dwell & Flight Times:** 
    *   **Dwell:** Time the key is held down. Model as $\text{LogNormal}$.
    *   **Flight:** Time between releasing one key and pressing the next.
2.  **Bigram Matrix:** Flight time depends on the specific key pair (e.g., typing 'th' is faster than typing 'qf'). Implement a 26x26 pre-computed matrix derived from public datasets (e.g., CMU Keystroke Dynamics dataset) to scale flight times based on the actual text being typed.
3.  **Typo & Correction Simulation:** Introduce a ~1.5% probability of a typo. When triggered, type the wrong adjacent key, pause briefly (realizing the error), send a Backspace, and type the correct key.

### 2.3 Scroll Velocity Decay
**The Goal:** Emulate the physics of trackpad momentum and the discrete nature of physical scroll wheels.

**Technical Approach:**
1.  **Trackpad:** Model momentum decay. Velocity $v(t) = v_0 \times (0.95)^{t/16\text{ms}}$. Send events at 60Hz.
2.  **Wheel:** Send discrete events (e.g., `deltaY: 100`) separated by LogNormal time intervals, not a smooth continuous stream.

---

## 3. Advanced JS Shims & Security Contexts

Trackers probe edge-case APIs to test if the browser environment is a headless shell trying to hide.

### 3.1 WebAuthn & FedCM (Simulating User Interaction)
**The Problem:** Headless browsers either throw immediate errors on `navigator.credentials.get()` or lack the APIs entirely. Real users take time to click "Cancel" or "Allow" on native OS dialogs.
**The Solution:**
1.  Implement full JS stubs for `PublicKeyCredential` and `IdentityCredential`.
2.  Crucially, when `.get()` or `.create()` is called, do not reject the Promise synchronously. 
3.  Wait a realistic human reaction time (e.g., 150ms to 3000ms, using the `performance.now` jitter logic) before rejecting with a `NotAllowedError`. This simulates the user dismissing the OS-level biometric/FedCM prompt.

### 3.2 SharedArrayBuffer & Cross-Origin Isolation (COI)
**The Problem:** Trackers check if `SharedArrayBuffer` behaves correctly based on HTTP response headers. Headless setups often blindly enable it everywhere.
**The Solution:**
1.  Engine must parse `Cross-Origin-Opener-Policy` (COOP) and `Cross-Origin-Embedder-Policy` (COEP) headers natively in Rust.
2.  Compute the `crossOriginIsolated` boolean state.
3.  Gate the *transfer* of `SharedArrayBuffer` via `postMessage` based on this state, matching Chrome's exact V8 security policy.

### 3.3 Execution Speed & Timer Jitter (`performance.now()`)
**The Problem:** Bots execute JavaScript too consistently. Timing the execution of a `while` loop or `Math.random()` over 10,000 iterations creates a distinct CPU signature.
**The Solution:**
1.  Override the native `performance.now()` clock.
2.  Snap the time to a realistic grid (e.g., 100µs intervals).
3.  Add continuous jitter: $\text{LogNormal}(\mu=\ln(8\mu\text{s}), \sigma=0.4)$.
4.  Add occasional OS-level thread preemption spikes: A 1-in-1000 chance to add an exponential delay up to 1.5ms, simulating the OS scheduler pulling the browser thread off the CPU.
