# browser_oxide Roadmap — SOTA 2026

> **Status note (2026-04-26):** This document is the *original build roadmap*
> covering Phases 1–7 (CSS → DOM → V8 → Canvas → Stealth → Layout → CDP →
> Hardening). Most of it is shipped or in progress. For the **2026 stealth
> SOTA work** (closing the render-stack, behavioral, and protocol-validation
> gaps that separate us from Camoufox/CloakBrowser), see:
>
> - **`docs/SOTA_ROADMAP_2026.md`** — sequenced 3-phase implementation plan
>   (JS shims → render stack → behavioral entropy)
> - **`docs/GAPS.md`** — gap catalogue P0–P33 with current status
> - **`docs/CAPABILITY_GAPS_2026.md`** — earlier capability audit (partially
>   superseded by SOTA_ROADMAP_2026.md)
> - **`docs/NEXT_STEPS.md`** — site-by-site execution queue
>
> The phase-by-phase roadmap below remains the reference for the underlying
> engine architecture. New SOTA work plugs into the existing crate structure
> rather than creating new phases.

## Phase 1: Foundation (CSS + DOM + HTML Parsing)

**Goal**: Parse HTML → mutable DOM with Shadow DOM → query with CSS selectors → compute styles.

### Milestone 1.1: css_parser
- [ ] CSS Syntax Level 3 tokenizer (all token types)
- [ ] Input preprocessing (BOM, null replacement, CR normalization)
- [ ] Source location tracking (offset, line, column)
- [ ] Parser: stylesheet, rule list, at-rules, qualified rules, declaration list
- [ ] **CSS Nesting**: interleaved declarations and nested rules in qualified rule blocks
- [ ] Component value preservation (functions, simple blocks)
- [ ] Error recovery: skip invalid, balance brackets, never panic
- [ ] Zero-copy tokenizer
- [ ] Test suite

### Milestone 1.2: css_selectors
- [ ] Selector parser consuming css_parser tokens
- [ ] Simple selectors: type, universal, class, ID, attribute (all operators + case flags)
- [ ] Pseudo-classes: structural (:nth-child, :first-child, :last-child, :only-child, :root, :empty)
- [ ] Pseudo-classes: functional (:is(), :not(), :where(), :has())
- [ ] Combinators: descendant, child, next-sibling, subsequent-sibling
- [ ] Selector lists + forgiving parsing
- [ ] An+B microsyntax
- [ ] Specificity computation
- [ ] Generic `Element` trait
- [ ] Right-to-left matching + Bloom filter
- [ ] Test suite

### Milestone 1.3: css_values
- [ ] Property value parser dispatch (property name → parser)
- [ ] Layout properties: display, position, width/height/min/max, margin, padding, border-width, box-sizing, overflow, flex-*, grid-*, gap, align-*, justify-*, float, clear
- [ ] Font properties: font-size, font-family, font-weight, font-style, line-height
- [ ] Visibility: visibility, opacity, z-index, content-visibility
- [ ] Transform functions
- [ ] Color: named, hex, rgb, hsl, oklch, oklab, lab, lch, color-mix(), color()
- [ ] Custom properties: var() with fallbacks, nested var(), env()
- [ ] Math functions: calc(), min(), max(), clamp()
- [ ] Shorthand → longhand expansion
- [ ] @property registration

### Milestone 1.4: css_cascade
- [ ] Cascade sort: origin, @layer, specificity, source order
- [ ] @layer ordering (named, unnamed, nested layers)
- [ ] @media query evaluation (width, height, dpi, prefers-color-scheme, pointer, hover)
- [ ] @supports evaluation
- [ ] @container query evaluation (basic: size queries)
- [ ] Inheritance: inheritable vs non-inheritable properties
- [ ] Custom property resolution (var() substitution)
- [ ] Specified → computed value conversion (em→px, rem→px, %→px, vh/vw→px)

### Milestone 1.5: DOM
- [ ] Arena-allocated node tree (NodeId, parent/child/sibling links)
- [ ] Node types: Document, DocumentType, Element, Text, Comment, DocumentFragment
- [ ] html5ever TreeSink implementation
- [ ] Element attributes (get, set, remove, has)
- [ ] Tree mutation: appendChild, removeChild, insertBefore, replaceChild
- [ ] innerHTML getter/setter (parse + serialize)
- [ ] textContent getter/setter
- [ ] querySelector / querySelectorAll via css_selectors
- [ ] getElementById, getElementsByClassName, getElementsByTagName
- [ ] classList (DOMTokenList), dataset
- [ ] **Shadow DOM**: attachShadow, ShadowRoot, slot distribution, flat tree
- [ ] Implement css_selectors::Element trait for DOM nodes

**Deliverable**: Parse HTML → DOM (with Shadow DOM) → query selectors → compute styles. No JS yet.

---

## Phase 2: V8 JavaScript Engine

**Goal**: Execute page JavaScript with full DOM access, WASM support, and correct Web API surface.

### Milestone 2.1: V8 Runtime Core
- [ ] rusty_v8 + deno_core integration
- [ ] V8 Isolate creation/destruction
- [ ] V8 heap snapshot support (fast page startup)
- [ ] Extension system for grouping ops
- [ ] Module loader (ES modules via net crate)
- [ ] Error propagation (JS ↔ Rust)

### Milestone 2.2: DOM Bindings (V8 ops)
- [ ] Node interface ops
- [ ] Element interface ops (including Shadow DOM)
- [ ] Document interface ops
- [ ] HTMLElement ops (style, dataset, offset*, checkVisibility)
- [ ] EventTarget + Event + addEventListener + dispatchEvent
- [ ] MutationObserver, IntersectionObserver, ResizeObserver
- [ ] DOMParser, XMLSerializer

### Milestone 2.3: Core Web APIs
- [ ] console.log/warn/error → Rust tracing
- [ ] URL, URLSearchParams, Headers, Request, Response
- [ ] fetch() → net crate
- [ ] XMLHttpRequest
- [ ] FormData, Blob, File
- [ ] TextEncoder, TextDecoder, atob(), btoa()
- [ ] crypto.getRandomValues(), crypto.subtle (basic)
- [ ] WebSocket client → tokio-tungstenite

### Milestone 2.4: Event Loop
- [ ] Task queue + microtask queue
- [ ] setTimeout / setInterval / clearTimeout / clearInterval
- [ ] Promise integration (V8 microtask scheduling)
- [ ] queueMicrotask()
- [ ] requestAnimationFrame (simulated 60fps)
- [ ] requestIdleCallback
- [ ] run_until_idle with timeout + idle detection

### Milestone 2.5: iframes
- [ ] Separate DOM + V8 Context per iframe
- [ ] Same-origin: parent ↔ iframe DOM access
- [ ] Cross-origin: isolation, contentDocument returns null
- [ ] window.postMessage() with Structured Clone
- [ ] MessageEvent with origin checking
- [ ] srcdoc attribute
- [ ] sandbox attribute (allow-scripts, allow-same-origin, etc.)

**Deliverable**: Execute page JS, WASM runs natively, iframes work, fetch/XHR/WebSocket work. SPAs render their content.

---

## Phase 3: Canvas + Rendering Stubs

**Goal**: Pass canvas/WebGL/AudioContext fingerprint checks.

### Milestone 3.1: Canvas 2D (tiny-skia)
- [ ] CanvasRenderingContext2D state machine
- [ ] Path operations: rect, arc, bezierCurveTo, lineTo, moveTo, closePath
- [ ] Fill/stroke with color, gradient, pattern
- [ ] Text rendering: fillText, strokeText, measureText (cosmic-text + rustybuzz + ab_glyph)
- [ ] Image operations: drawImage, createImageData, getImageData, putImageData
- [ ] toDataURL (PNG/JPEG encoding)
- [ ] toBlob
- [ ] Compositing (globalCompositeOperation, globalAlpha)
- [ ] Transform: translate, rotate, scale, setTransform
- [ ] Clipping paths
- [ ] Shadow rendering
- [ ] OffscreenCanvas

### Milestone 3.2: WebGL Stubs
- [ ] WebGLRenderingContext with parameter stubs (40+ getParameter values)
- [ ] WEBGL_debug_renderer_info extension (GPU vendor/renderer from profile)
- [ ] getSupportedExtensions() from profile
- [ ] getShaderPrecisionFormat() from profile
- [ ] Basic createShader/compileShader/linkProgram (don't fail, return valid objects)

### Milestone 3.3: AudioContext
- [ ] AudioContext / OfflineAudioContext constructors
- [ ] OscillatorNode, DynamicsCompressorNode, AnalyserNode (stubs)
- [ ] startRendering() → deterministic AudioBuffer from profile seed
- [ ] BaseAudioContext.destination, sampleRate

### Milestone 3.4: Font Support
- [ ] fontdb integration: load system fonts, parse web fonts (@font-face)
- [ ] rustybuzz text shaping
- [ ] Font metric measurement for layout
- [ ] document.fonts API (FontFaceSet: ready, check, load)
- [ ] CSS font-face download + registration

**Deliverable**: Canvas fingerprint passes (real rendering). WebGL parameters match profile GPU. AudioContext returns consistent output.

---

## Phase 4: Networking + Stealth

**Goal**: Full stealth HTTP stack with HTTP/3 and complete fingerprint profiles.

### Milestone 4.1: HTTP Client
- [ ] rquest integration (HTTP/1.1 + HTTP/2 + BoringSSL)
- [ ] Browser TLS profiles (JA4 matching for Chrome/Firefox/Safari)
- [ ] HTTP/2 fingerprint (SETTINGS, WINDOW_UPDATE, PRIORITY)
- [ ] Header templates (order, case, sec-ch-ua-* Client Hints)
- [ ] Cookie jar (cookie_store)
- [ ] Redirect handling
- [ ] Proxy support (SOCKS5, HTTP CONNECT)

### Milestone 4.2: HTTP/3
- [ ] quinn + h3 integration
- [ ] QUIC transport parameter profiles (match Chrome's QUIC settings)
- [ ] Alt-Svc caching (upgrade to HTTP/3 when server advertises it)
- [ ] Protocol negotiation (QUIC → TCP fallback)

### Milestone 4.3: Stealth Profiles
- [ ] StealthProfile with full consistency validation (all 100+ properties)
- [ ] Pre-built profiles: Chrome 130 × Windows/macOS/Linux
- [ ] Pre-built profiles: Firefox 133, Safari 18
- [ ] Random profile generator with statistical realism
- [ ] Full navigator.* surface (50+ properties)
- [ ] window.chrome object
- [ ] performance.now() with Chrome resolution
- [ ] performance.memory (Chrome-specific)
- [ ] Permissions API realistic states
- [ ] MediaDevices enumeration stubs
- [ ] speechSynthesis.getVoices() with OS-appropriate voices
- [ ] Prototype integrity (Function.prototype.toString returns "[native code]")

### Milestone 4.4: Workers
- [ ] Web Workers (separate V8 Isolates)
- [ ] Structured Clone Algorithm
- [ ] Worker global scope (postMessage, importScripts, fetch, timers, WASM)
- [ ] Service Worker API surface (register, ready, controller — stub lifecycle)
- [ ] MessagePort / MessageChannel

**Deliverable**: Full stealth stack. TLS/HTTP2/HTTP3 fingerprints match real Chrome. All anti-bot API checks pass.

---

## Phase 5: Layout

**Goal**: getBoundingClientRect() and offset* APIs return accurate values.

### Milestone 5.1: Layout Computation
- [ ] DOM → taffy tree conversion
- [ ] Computed styles → taffy::Style mapping
- [ ] Font metrics for text sizing (fontdb + rustybuzz)
- [ ] Web font download + metric extraction
- [ ] Viewport (1920x1080 default, configurable per profile)
- [ ] Layout cache with dirty tracking
- [ ] Incremental re-layout on DOM mutation

### Milestone 5.2: JS Layout APIs
- [ ] getBoundingClientRect()
- [ ] offsetWidth, offsetHeight, offsetTop, offsetLeft, offsetParent
- [ ] clientWidth, clientHeight, clientTop, clientLeft
- [ ] scrollWidth, scrollHeight, scrollTop, scrollLeft
- [ ] window.innerWidth, window.innerHeight, window.outerWidth, window.outerHeight
- [ ] getComputedStyle() (backed by css_cascade)
- [ ] element.checkVisibility()
- [ ] IntersectionObserver (real geometry checks)

**Deliverable**: Layout APIs work. Sites checking element visibility/position function correctly.

---

## Phase 6: CDP Protocol Server

**Goal**: Puppeteer and Playwright can drive browser_oxide as a drop-in Chrome replacement.

### Milestone 6.1: CDP Core
- [ ] WebSocket server (tokio-tungstenite)
- [ ] JSON-RPC message handling
- [ ] Session management
- [ ] Method routing
- [ ] Auto-generated types from protocol.json

### Milestone 6.2: CDP Domains
- [ ] Target (createTarget, closeTarget, attachToTarget)
- [ ] Page (navigate, reload, addScriptToEvaluateOnNewDocument, lifecycle events)
- [ ] Runtime (evaluate, callFunctionOn, consoleAPICalled) — **no Runtime.enable leak** because QuickJS/V8 in our context has no side effects
- [ ] DOM (getDocument, querySelector, getOuterHTML)
- [ ] Network (enable, setCookies, getCookies, request/response events)
- [ ] Fetch (request interception)
- [ ] Input (dispatchMouseEvent, dispatchKeyEvent)
- [ ] Emulation (setDeviceMetricsOverride, setUserAgentOverride)

### Milestone 6.3: Compatibility Testing
- [ ] Puppeteer test suite (target: 80%+ pass rate)
- [ ] Playwright test suite (target: 80%+ pass rate)
- [ ] scraper_oxide integration (replace chaser-oxide)

**Deliverable**: Drop-in replacement for headless Chrome in scraping workflows.

---

## Phase 7: Production Hardening

- [ ] Fuzzing: css_parser, css_selectors, html parsing, JS evaluation, WASM
- [ ] Memory leak testing (long-running sessions)
- [ ] Concurrent page stress testing (100+ pages in parallel)
- [ ] Benchmarks vs Chrome headless, Lightpanda
- [ ] Anti-bot test suite: Cloudflare Turnstile, DataDome, Akamai, HUMAN, Kasada
- [ ] Documentation + examples
- [ ] crates.io publishing (individual crates)
- [ ] CI/CD (GitHub Actions)

---

## Timeline Estimate (SOTA)

| Phase | Effort | Cumulative |
|---|---|---|
| Phase 1: Foundation (CSS + DOM) | 8-10 weeks | 8-10 weeks |
| Phase 2: V8 + JS + iframes | 10-14 weeks | 18-24 weeks |
| Phase 3: Canvas + rendering stubs | 4-6 weeks | 22-30 weeks |
| Phase 4: Networking + Stealth + Workers | 6-8 weeks | 28-38 weeks |
| Phase 5: Layout | 4-6 weeks | 32-44 weeks |
| Phase 6: CDP Server | 4-6 weeks | 36-50 weeks |
| Phase 7: Hardening | 3-4 weeks | 39-54 weeks |

**Total: ~9-13 months** for one full-time developer.

## Critical Path

```
Phase 1 (CSS + DOM) ──→ Phase 2 (V8 + JS) ──→ Phase 6 (CDP)
                                │
                                ├──→ Phase 3 (Canvas)
                                ├──→ Phase 5 (Layout)
                                └──→ Phase 4 (Net + Stealth + Workers)
```

Phase 1 → 2 is sequential (JS needs DOM). After Phase 2, Phases 3-5 can be parallelized. Phase 6 needs Phase 2 + 4.

## Parallel Development Opportunities

- Phase 1 (CSS + DOM) and Phase 4.1-4.2 (HTTP client) are independent
- Phase 3 (Canvas) and Phase 5 (Layout) are independent
- Phase 4.4 (Workers) can start once Phase 2.1 (V8 core) is done
- Multiple developers can work on different crates simultaneously
