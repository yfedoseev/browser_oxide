# canvas — Canvas 2D API + WebGL Parameter Stubs

New crate. Provides real Canvas 2D rendering (not stubs) for anti-bot challenges, plus WebGL parameter responses for fingerprinting.

## Why Real Rendering Is Required

The previous plan used fake canvas hashes. This fails in 2026:

- **Cloudflare Turnstile** renders specific shapes to canvas and verifies pixel data
- **DataDome** renders canvas multiple times and compares for artificial noise injection
- **Akamai** uses canvas + font rendering as a hardware fingerprint
- A fake `toDataURL()` hash is trivially detected by checking `getImageData()` pixel values

We need **actual 2D rendering** — but not a full GPU pipeline. CPU-based rendering via tiny-skia is sufficient.

## Canvas 2D: tiny-skia Backend

| Property | Value |
|---|---|
| Crate | `tiny-skia` |
| License | MIT/Apache-2.0 |
| Backend | CPU-based (no GPU required) |
| API | Subset of Skia (Google's 2D library used in Chrome) |
| Supports | Paths, fills, strokes, gradients, patterns, clipping, compositing, anti-aliasing |

### Canvas 2D Operations Needed

Anti-bot systems use these Canvas 2D operations:

| Operation | tiny-skia Support | Anti-bot Usage |
|---|---|---|
| `fillRect()` | Yes | Basic rendering |
| `fillText()` / `strokeText()` | Via cosmic-text/rustybuzz | **Primary fingerprint** — font rendering varies by OS/GPU |
| `arc()`, `bezierCurveTo()` | Yes (path API) | Shape rendering |
| `createLinearGradient()` | Yes | Gradient rendering |
| `createRadialGradient()` | Yes | Gradient rendering |
| `drawImage()` | Yes (pixmap compositing) | Image compositing |
| `getImageData()` | Yes (read pixmap) | Pixel-level verification |
| `toDataURL()` / `toBlob()` | Yes (encode to PNG/JPEG) | **The fingerprint extraction** |
| `globalCompositeOperation` | Yes (blend modes) | Composite rendering |
| `shadowBlur/Color/Offset` | Manual (blur + composite) | Shadow rendering |
| `clip()` | Yes (clipping paths) | Clipping |
| `transform()` / `setTransform()` | Yes (matrix transform) | Transform rendering |
| `globalAlpha` | Yes (opacity) | Alpha compositing |
| `imageSmoothingEnabled` | Yes (pixmap sampling) | Anti-aliasing control |

### Text Rendering (The Critical Part)

Canvas fingerprinting is primarily based on **text rendering** — `fillText("Cwm fjordbank glyphs vext quiz", 2, 15)` produces different pixel output on different OS/GPU/font combinations.

We need real font rendering:

| Crate | License | Purpose |
|---|---|---|
| `fontdb` | MIT | System font database — find fonts by family name, weight, style |
| `rustybuzz` | MIT | Text shaping — HarfBuzz port, converts text to positioned glyphs |
| `cosmic-text` | MIT/Apache-2.0 | Text layout — line breaking, alignment, bidi, shaping |
| `ab_glyph` | Apache-2.0 | Glyph rasterization — convert outlines to bitmaps |

Pipeline: `text string` → `cosmic-text` (layout) → `rustybuzz` (shaping) → `ab_glyph` (rasterize) → `tiny-skia` (composite onto canvas pixmap)

### Deterministic Rendering per Profile

For the same `StealthProfile`, canvas output must be **deterministic** (same input → same pixels). Anti-bot runs canvas tests multiple times and compares.

Factors we control:
- **Font selection**: Profile specifies available fonts and default font
- **Font hinting**: Profile specifies hinting mode (match target OS)
- **Sub-pixel rendering**: Profile specifies LCD vs grayscale anti-aliasing
- **Floating-point determinism**: Use consistent rounding (IEEE 754 compliant)

## WebGL: Parameter Stubs + Minimal Rendering

### Do Anti-Bot Systems Actually Render with WebGL?

Research shows anti-bot primarily **queries WebGL parameters** rather than rendering:

- `getParameter(UNMASKED_VENDOR_WEBGL)` → GPU vendor string
- `getParameter(UNMASKED_RENDERER_WEBGL)` → GPU model string
- `getSupportedExtensions()` → extension list
- `getParameter(MAX_TEXTURE_SIZE)` → GPU capability
- `getParameter(MAX_RENDERBUFFER_SIZE)` → GPU capability
- `getShaderPrecisionFormat()` → shader precision

Some advanced checks do render a triangle and hash the output. For this we have two options:

### Option A: Parameter stubs only (MVP)

Return pre-computed values from the StealthProfile:

```rust
fn get_parameter(&self, pname: u32) -> Value {
    match pname {
        UNMASKED_VENDOR_WEBGL => self.profile.gpu.vendor.clone(),
        UNMASKED_RENDERER_WEBGL => self.profile.gpu.renderer.clone(),
        MAX_TEXTURE_SIZE => self.profile.gpu.max_texture_size,
        // ... 40+ parameters
    }
}
```

### Option B: Real WebGL via wgpu (future)

For sites that actually render:
- `wgpu` (MIT/Apache-2.0) — Rust GPU abstraction (Vulkan/Metal/DX12/OpenGL)
- Can create offscreen render targets
- Would provide real WebGL rendering with profile-matching GPU output
- Significant complexity, deferred to later phase

**Recommendation**: Start with Option A (parameter stubs). Most anti-bot passes with correct parameters. Add Option B if specific sites require actual WebGL rendering.

## AudioContext Fingerprint

`OfflineAudioContext` renders audio and hashes the output. Similar to canvas — varies by hardware/driver.

Implementation: Generate deterministic audio samples from the profile seed. The `OfflineAudioContext.startRendering()` call returns pre-computed `AudioBuffer` data.

## Architecture

```
canvas/
├── src/
│   ├── lib.rs
│   ├── canvas2d.rs         # CanvasRenderingContext2D — state machine + tiny-skia
│   ├── text.rs             # Text rendering pipeline (cosmic-text + rustybuzz + ab_glyph)
│   ├── gradient.rs         # Linear/radial gradient
│   ├── pattern.rs          # Pattern fill
│   ├── image_data.rs       # ImageData (getImageData, putImageData)
│   ├── encoding.rs         # toDataURL (PNG/JPEG encoding)
│   ├── webgl.rs            # WebGLRenderingContext parameter stubs
│   ├── audio.rs            # OfflineAudioContext fingerprint stub
│   └── offscreen.rs        # OffscreenCanvas support
├── tests/
│   ├── canvas2d_tests.rs   # Verify rendering output
│   ├── text_rendering.rs   # Font rendering consistency
│   └── fingerprint.rs      # Verify deterministic fingerprints
└── Cargo.toml
```
