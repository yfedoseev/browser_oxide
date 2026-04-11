# T1.4 — Finish OSMesa + glow WebGL pipeline

**Task**: #65
**Priority**: P1, but **LOWEST of the T1.x items** per probe evidence
**Effort**: 35-50 hours
**Dependencies**: Must resolve the license question (OSMesa is LGPL)
BEFORE committing implementation time.

## Goal

Ship a real WebGL backend that actually rasterizes shaders and returns
real `readPixels()` data. Replace the current parameter-stub
implementation that returns Chrome-shaped strings for `getParameter`/
`getExtension` but does no actual rendering.

## Why this is LAST in Tier 1

- **The adidas sensor VM makes zero WebGL method calls** (verified by
  probe in `crates/browser/tests/adidas_sensor_api_probes.rs`). T1.4
  does NOT unblock adidas.
- The current parameter stub already passes many fingerprint sites
  because most only check `UNMASKED_VENDOR_WEBGL`/
  `UNMASKED_RENDERER_WEBGL` strings and supported extension lists,
  both of which we emit correctly from the stealth profile.
- Sites that actually hash `readPixels()` output (CreepJS does for
  some test shapes) will detect our stub, but those are minority and
  not in the tier-1 blocker set.
- License situation is genuinely uncertain — OSMesa is LGPL which
  may be blocked.

**Bottom line**: ship T1.1 (Skia canvas) and T1.2 (fonts) first. Only
do T1.4 if a specific known-failing site demands it, OR if we have a
clean license path to SwiftShader.

## Current state

**Files**:
- `crates/canvas/src/webgl.rs` — the parameter stub + `WebGLParams`
  struct with all the `getParameter` constants Chrome returns.
- `crates/canvas/src/osmesa_ffi.rs` — feature-gated (`webgl-render`)
  OSMesa FFI bindings, partially implemented.
- `crates/canvas/src/webgl_render.rs` — feature-gated (`webgl-render`)
  render path using `glow`, partially implemented.
- `crates/js_runtime/src/extensions/webgl_ext.rs` — ops. Has the
  `#[cfg(feature = "webgl-render")]` gates at lines 274, 283, 293
  that currently warn because the feature isn't declared in Cargo.toml.
- `crates/stealth/src/gpu_profile.rs` — GPU catalog with vendor /
  renderer / extension list / shader precision per GPU.

**What we have**: complete parameter-stub path. `gl.getParameter(
gl.VERSION)` returns `"WebGL 1.0 (OpenGL ES 2.0 Chromium)"`,
`getSupportedExtensions()` returns the Chrome list for the selected
GPU profile, `getContextAttributes()` returns plausible defaults. All
of this is Chrome-shape correct because it comes from our stealth
profile's GPU catalog.

**What's missing**: actual rasterization. Any call to `drawArrays`,
`drawElements`, `readPixels`, `copyTexImage2D` returns stub output
(zeros or errors). A fingerprinter that does:

```js
const gl = canvas.getContext('webgl');
// Compile and link a known shader
// ... drawing code ...
const pixels = new Uint8Array(4 * 8 * 8);
gl.readPixels(0, 0, 8, 8, gl.RGBA, gl.UNSIGNED_BYTE, pixels);
const hash = sha256(pixels);
```

...will get all-zero pixels and produce a hash that matches no real
GPU, instantly detectable as a bot.

## License resolution (must do first)

browser_oxide policy: **MIT / Apache-2.0 / BSD-3 only. No MPL, no LGPL,
no GPL.**

OSMesa status: the Mesa project is **MIT/X11 licensed** (per
https://www.mesa3d.org/license.html). The specific Mesa drivers
(i915, iris, radeonsi) are MIT. **OSMesa itself is MIT.** The
previous concern about LGPL was incorrect — Mesa is MIT with some
drivers using BSD-style licenses.

**VERIFY** before starting:
```bash
# Ubuntu/Debian:
apt show libosmesa6-dev 2>/dev/null | grep -i license
# Or from source:
curl -sL https://gitlab.freedesktop.org/mesa/mesa/-/raw/main/docs/license.rst | head -30
```

If Mesa is confirmed MIT, OSMesa is usable. Document the verification
in this plan file and proceed.

Alternative paths if OSMesa is unusable:

1. **SwiftShader** — Google's pure-software GL. Apache-2.0. Requires
   vendoring or cross-compiling from the Chromium source. More work
   but definitely clean licensing.
2. **Vulkan + Lavapipe** — Mesa's software Vulkan. Use `wgpu` on top
   which auto-selects Lavapipe on Linux headless. Adds a large
   dependency graph but `wgpu` is the standard Rust GPU abstraction.
3. **ANGLE** — Google's cross-platform GL → Vulkan/D3D translator.
   BSD-3. Requires vendoring similar to SwiftShader.

**Recommendation**: if OSMesa is confirmed MIT, use it (simplest and
most battle-tested). Otherwise use SwiftShader + glow. Avoid wgpu for
this — the dependency graph is too heavy for a fingerprint-only use
case.

## Step-by-step implementation (assuming OSMesa)

### Step 1 — Enable the feature flag (1h)

**File**: `crates/canvas/Cargo.toml`

```toml
[features]
default = []
webgl-render = ["osmesa-sys", "glow"]

[dependencies]
osmesa-sys = { version = "0.1", optional = true }
glow = { version = "0.14", optional = true }
```

**File**: `crates/canvas/src/lib.rs`

Check that the `#[cfg(feature = "webgl-render")]` gates around
`osmesa_ffi` and `webgl_render` module declarations still compile
after the feature is declared. Fix the existing warnings about
unexpected cfg values.

### Step 2 — OSMesa context creation (3-5h)

**File**: `crates/canvas/src/osmesa_ffi.rs`

```rust
use osmesa_sys::*;
use std::os::raw::c_void;

pub struct OsMesaContext {
    ctx: OSMesaContext,
    buffer: Vec<u32>,
    width: i32,
    height: i32,
}

impl OsMesaContext {
    pub fn new(width: u32, height: u32) -> Result<Self, OsMesaError> {
        // Create a 3.3 core profile context.
        let attribs = [
            OSMESA_FORMAT, OSMESA_RGBA,
            OSMESA_DEPTH_BITS, 24,
            OSMESA_STENCIL_BITS, 8,
            OSMESA_ACCUM_BITS, 0,
            OSMESA_PROFILE, OSMESA_CORE_PROFILE,
            OSMESA_CONTEXT_MAJOR_VERSION, 3,
            OSMESA_CONTEXT_MINOR_VERSION, 3,
            0,
        ];
        let ctx = unsafe {
            OSMesaCreateContextAttribs(attribs.as_ptr(), std::ptr::null_mut())
        };
        if ctx.is_null() {
            return Err(OsMesaError::ContextCreation);
        }
        let buffer = vec![0u32; (width * height) as usize];
        Ok(OsMesaContext {
            ctx,
            buffer,
            width: width as i32,
            height: height as i32,
        })
    }

    pub fn make_current(&mut self) -> Result<(), OsMesaError> {
        let ok = unsafe {
            OSMesaMakeCurrent(
                self.ctx,
                self.buffer.as_mut_ptr() as *mut c_void,
                GL_UNSIGNED_BYTE,
                self.width,
                self.height,
            )
        };
        if ok == 0 { Err(OsMesaError::MakeCurrent) } else { Ok(()) }
    }

    pub fn pixels(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(
                self.buffer.as_ptr() as *const u8,
                self.buffer.len() * 4,
            )
        }
    }
}

impl Drop for OsMesaContext {
    fn drop(&mut self) {
        unsafe { OSMesaDestroyContext(self.ctx); }
    }
}
```

### Step 3 — glow binding (2-3h)

**File**: `crates/canvas/src/webgl_render.rs`

```rust
use glow::{HasContext, Context};

pub struct WebGLRenderer {
    gl: Context,
    context: OsMesaContext,
    width: u32,
    height: u32,
}

impl WebGLRenderer {
    pub fn new(width: u32, height: u32) -> Result<Self, WebGLError> {
        let mut context = OsMesaContext::new(width, height)?;
        context.make_current()?;
        let gl = unsafe {
            Context::from_loader_function(|symbol| {
                OSMesaGetProcAddress(symbol.as_ptr() as *const i8) as *const _
            })
        };
        Ok(Self { gl, context, width, height })
    }

    pub fn gl(&self) -> &Context {
        &self.gl
    }
}
```

### Step 4 — Implement the WebGL ops (20-30h)

**File**: `crates/js_runtime/src/extensions/webgl_ext.rs`

This is the bulk of the work. Each WebGL 1.0 method maps to a glow
call. There are ~100 methods to implement. Group by category:

**Context state** (2h):
- `op_webgl_viewport`, `op_webgl_scissor`, `op_webgl_clear`,
  `op_webgl_clear_color`, `op_webgl_clear_depth`, `op_webgl_clear_stencil`

**Buffers** (3h):
- `op_webgl_create_buffer`, `op_webgl_delete_buffer`,
  `op_webgl_bind_buffer`, `op_webgl_buffer_data`,
  `op_webgl_buffer_sub_data`

**Shaders and programs** (4h):
- `op_webgl_create_shader`, `op_webgl_shader_source`,
  `op_webgl_compile_shader`, `op_webgl_get_shader_parameter`,
  `op_webgl_get_shader_info_log`, `op_webgl_delete_shader`,
  `op_webgl_create_program`, `op_webgl_attach_shader`,
  `op_webgl_link_program`, `op_webgl_get_program_parameter`,
  `op_webgl_use_program`, `op_webgl_delete_program`

**Uniforms and attributes** (4h):
- `op_webgl_get_uniform_location`, `op_webgl_uniform_1f`,
  `op_webgl_uniform_4fv`, `op_webgl_uniform_matrix_4fv`,
  `op_webgl_get_attrib_location`,
  `op_webgl_enable_vertex_attrib_array`,
  `op_webgl_vertex_attrib_pointer`

**Textures** (4h):
- `op_webgl_create_texture`, `op_webgl_bind_texture`,
  `op_webgl_tex_image_2d`, `op_webgl_tex_parameter_i`,
  `op_webgl_active_texture`, `op_webgl_generate_mipmap`,
  `op_webgl_delete_texture`

**Framebuffers** (3h):
- `op_webgl_create_framebuffer`, `op_webgl_bind_framebuffer`,
  `op_webgl_framebuffer_texture_2d`, `op_webgl_create_renderbuffer`,
  `op_webgl_bind_renderbuffer`, `op_webgl_renderbuffer_storage`,
  `op_webgl_framebuffer_renderbuffer`,
  `op_webgl_check_framebuffer_status`

**Drawing** (2h):
- `op_webgl_draw_arrays`, `op_webgl_draw_elements`

**Reading** (2h):
- `op_webgl_read_pixels` — this is the important one for
  fingerprint matching

**State queries** (3h):
- `op_webgl_get_parameter` — already exists as a stub returning
  profile data, now needs to intersect real GL queries where the
  real value matters (e.g., `gl.getParameter(gl.VIEWPORT)` must
  return the actual viewport, not a stub).

### Step 5 — WebGL 2.0 additions (5-8h)

If WebGL 2 is needed (task #65 says "glow WebGL pipeline"), add:
- Vertex array objects (VAO)
- Texture 3D / Texture 2D arrays
- Framebuffer objects with multiple attachments
- Uniform buffer objects
- Transform feedback
- Sampler objects
- Sync objects

This is a lot. If your time budget is tight, ship WebGL 1 first and
defer WebGL 2 to a follow-up.

### Step 6 — Tests (3-5h)

**File**: `crates/canvas/tests/webgl_render.rs` (gate behind
`#[cfg(feature = "webgl-render")]`)

```rust
#[test]
fn clear_produces_expected_pixels() {
    let mut renderer = WebGLRenderer::new(100, 100).unwrap();
    let gl = renderer.gl();
    unsafe {
        gl.clear_color(1.0, 0.0, 0.0, 1.0); // pure red
        gl.clear(glow::COLOR_BUFFER_BIT);
    }
    let pixels = renderer.read_pixels(0, 0, 100, 100);
    // First pixel should be (255, 0, 0, 255)
    assert_eq!(pixels[0..4], [255, 0, 0, 255]);
}

#[test]
fn triangle_rasterizes() {
    let mut renderer = WebGLRenderer::new(100, 100).unwrap();
    // Load a known vertex+fragment shader.
    // Draw a red triangle.
    // readPixels and verify the triangle is visible.
}

#[test]
fn fingerprint_shader_deterministic() {
    // The CreepJS WebGL fingerprint shader, which draws a specific
    // scene and hashes the output. Run it twice and verify the
    // hash is deterministic across runs.
}
```

### Step 7 — Regression gate (1h)

Run the existing `deep_path_validation.rs` and verify no sites
regress. Also run `fingerprint_scorers.rs` (if still present) and
check if CreepJS trust score improves.

## Acceptance criteria

1. **Workspace green** with and without `--features webgl-render`.
2. **New WebGL render tests pass**.
3. **Deep-path regression**: 22 passing sites still HOLD.
4. **Binary size**: adds ~5 MB for glow + OSMesa static libs.
5. **CreepJS WebGL trust score moves in the right direction**.

## Risks

**Risk 1: OSMesa licensing**. Verify BEFORE starting.

**Risk 2: glow isn't 100% Chrome-GL-compatible**. glow is a thin
binding; it does whatever the underlying GL driver does. OSMesa's
software rasterizer may produce pixels that differ from Chrome's
swiftshader-on-Windows at the LSB level. If fingerprint sites
compare bit-exactly, we'll still be detectable.

**Risk 3: ThreadLocalCurrent context**. OSMesa is context-per-thread
like the rest of OpenGL. Our multi-worker setup needs to make sure
each thread that touches GL has its own context. Forward from main
thread to worker thread via ops — probably fine, but adds complexity.

**Risk 4: Compile time**. skia-safe already adds ~2 minutes to cold
builds. Adding osmesa-sys + glow compilation could push that to 3-4
minutes. Mitigation: cache in CI aggressively.

## Alternative: `wgpu` + Lavapipe

If you want a more modern abstraction, `wgpu` supports Lavapipe on
Linux headless and offers a safe Rust API. Cost: 20-30 MB of
dependencies, much larger compile times, but more actively
maintained than OSMesa bindings. Only worth it if you plan to do
more graphics work beyond fingerprinting.

For a fingerprint-only use case, OSMesa + glow is the right
tradeoff.

## After this ships

- Task #65 is complete.
- Verify CreepJS and FingerprintJS WebGL trust scores improve.
- Manual check on canadagoose and hyatt — their Kasada ips.js may
  probe WebGL; see if the new render improves the trust level
  (though probably not enough to pass on its own).

## Related

- `crates/canvas/src/webgl.rs` — existing parameter stub
- `crates/stealth/src/gpu_profile.rs` — GPU catalog
- Task #27, #29, #30 (completed) — GPU profile audit and extension
  lists
- `docs/universal_engine/site_debugging/adidas_akamai_bmp_v3.md` —
  confirms WebGL is NOT on adidas critical path
