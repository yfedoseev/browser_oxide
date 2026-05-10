# browser_oxide — Project Context & Entry Point

Welcome to **browser_oxide**, a from-scratch, Rust-based headless browser engine designed to achieve 100% State-of-the-Art (SOTA) stealth and bypass the world's most advanced ML-based bot mitigation systems (DataDome, Kasada, PerimeterX, Akamai v13+).

If you are a new team member or a new AI session joining this project, **this is your starting point.**

## 1. Project Status & Architecture
*   **Current State:** We have achieved **100% Structural and Protocol-Layer Parity** with real Chrome 147. We perfectly spoof 1188 global properties, mask internal engine state, and generate byte-perfect TLS (JA4) and HTTP/2 handshakes. We pass the most aggressive fingerprint scanners (CreepJS, PixelScan, BrowserLeaks) with zero inconsistencies.
*   **Architecture:** See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for a breakdown of the 15 workspace crates (e.g., `net`, `js_runtime`, `stealth`, `canvas`, `browser`). We use `deno_core` (V8) for JavaScript execution and a custom networking stack to ensure perfect fingerprinting.
*   **Licensing:** Strictly MIT/Apache-2.0. We do **not** use MPL dependencies (e.g., no Servo crates).

## 2. Where to Find Context
When picking up a new task or trying to understand the current trajectory of the project, read these files in order:

1.  **[`docs/HANDOFF_2026_05_09.md`](docs/HANDOFF_2026_05_09.md)**: The latest handoff document. It details exactly what was achieved in the most recent session (structural parity, 1188 properties, Kasada `x-kpsdk-im`/`dt` token fixes) and what the immediate next steps are (Residential Proxies).
2.  **[`docs/RESEARCH_DEEP_DIVE_SOTA_2026.md`](docs/RESEARCH_DEEP_DIVE_SOTA_2026.md)**: A crucial deep-dive into the *next* major frontiers: **Render Stack Realism** (wgpu+Lavapipe WebGL spoofing, AudioContext jitter) and **Behavioral Entropy** (Sigma-Lognormal mouse kinematics, Keystroke dynamics). This contains the algorithms and strategies for defeating behavioral/render trackers.
3.  **[`docs/SOTA_ROADMAP_2026.md`](docs/SOTA_ROADMAP_2026.md)**: The sequenced, multi-week implementation plan for closing the remaining gaps to become the undisputed #1 stealth browser.
4.  **[`docs/GAPS.md`](docs/GAPS.md)**: An honest, granular checklist of all known fingerprinting gaps compared to a real Chrome 147 installation. Many of these have been closed recently, but it remains the master list of known tells.
5.  **[`CLAUDE.md`](CLAUDE.md)**: A brief summary of build/test commands and key coding conventions (e.g., `cargo test --workspace -- --test-threads=1` because V8 isolates are per-thread).

## 3. Development Workflow
*   **Tests:** Network tests are often marked `#[ignore]` and require a live connection to anti-bot protected sites. To run Kasada diagnostics: `cargo test -p browser --test tier0_kasada -- --nocapture --ignored`.
*   **Stealth Profiles:** Hardware and OS configurations live in `crates/stealth/src/profile.rs` and `crates/stealth/src/presets.rs`. When spoofing, *everything* (User-Agent, Client Hints, TCP TTL, WebGL strings, Audio jitter) stems from these profiles.
*   **JS Environment:** The V8 environment is seeded by files in `crates/js_runtime/src/js/`. *Crucially*, we do not just inject properties; we ensure their prototype chains, descriptors (enumerable/configurable/writable), and `.toString()` outputs perfectly match native Chrome.

**If you are starting work:** Look at the "Action Plan" in the latest `HANDOFF_` document or pick a phase from the `RESEARCH_DEEP_DIVE_SOTA_2026.md` / `SOTA_ROADMAP_2026.md`.
