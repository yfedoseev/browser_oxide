# Performance Report & Benchmarking Guide

This document outlines the current performance baselines from `scraper_oxide` and provides a methodology for testing and optimizing the `browser_oxide` native engine.

## 1. Performance Baselines (c=10)

Measurements taken against a local Axum-based mock server serving deterministic HTML fixtures.

| Metric | spider_rs | scraper_oxide (http) | scraper_oxide (browser) | Target (Native) |
|---|---|---|---|---|
| **Throughput** | ~600-1200 p/s | ~100 p/s | ~4 p/s | **>500 p/s** |
| **P95 Latency** | ~1.2ms | ~160ms | ~1400ms | **<10ms** |
| **Peak RSS** | ~1400MB | ~750MB | ~750MB | **<200MB** |

### Key Observations:
- **Chrome/CDP Overhead:** The "browser" mode (headless Chrome) is the primary bottleneck, limited by the IPC overhead of CDP and the heavy footprint of a full browser process.
- **Extraction Tax:** `scraper_oxide` performance includes full Markdown extraction via `reader_oxide`. For raw engine testing, extraction should be disabled.
- **Memory Efficiency:** `scraper_oxide` uses significantly less memory than `spider_rs`, likely due to better connection pooling and buffer reuse.

## 2. Benchmarking Methodology

To improve `browser_oxide`, we must move away from "wall-clock averages" and track high-fidelity metrics.

### A. The "Fast Mock" Approach
Do not test against external websites or heavy Dockerized mocks (like Wiremock). Use an in-process Axum server:
- Eliminates network jitter.
- Eliminates context switching between Docker containers.
- Allows for 14,000+ requests/sec saturation.

### B. Metrics to Track
1. **Latency Histograms (P50, P95, P99):** Average latency hides "outlier" JS execution stalls. Tracking P99 is critical for identifying V8 isolate warm-up issues.
2. **RSS History:** Sample `VmRSS` from `/proc/self/status` every N pages. This helps detect memory fragmentation in the V8 heap or DOM leaks.
3. **Isolate Reuse:** Measure the difference between "Cold" start (new Page) and "Warm" start (`Page::reload_html`). Reusing the V8 isolate should drop per-page overhead from ~17ms to ~2ms.

## 3. Recommended Test Suite

Create a `benchmarks` crate in `browser_oxide` that implements:
1. **Raw Fetch:** Measure how fast the `net` crate can pull bytes into a buffer.
2. **DOM Construction:** Measure `html_parser` speed on 100KB+ documents.
3. **JS Execution Loop:** Execute a standard "scraper script" (extract title, follow links) across 1000 virtual pages.
4. **Concurrency Stress:** Run at c=10, 50, and 100 to find the locking bottleneck in the `event_loop` or `js_runtime`.

## 4. Optimization Targets for `browser_oxide`
- **Native DOM Access:** Ensure JS can access the DOM without expensive serialization/deserialization across the V8 boundary.
- **Zero-Copy HTML Parsing:** Stream HTML directly into the DOM structure.
- **Resource Blocking:** Native support for skipping image/CSS/font loading to focus strictly on JS-driven content.
