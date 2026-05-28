# 07 — FIX-D2 + FIX-D3: WebGL cross-API conflation + per-non-macOS GPU validation

**Status:** ⬜ open. FIX-D shipped this session for `apple_m3_macos`; the broader cleanup is here.
**Sites in scope:** up to 7 — the AWS WAF Stratum-A cluster (`amazon-com`, `amazon-ca`, `amazon-com-au`, `amazon-fr`, `amazon-in`, `amazon-jp`, `imdb`), plus boost on the cross-API consistency probes used by DataDome + Cloudflare.
**Effort:** FIX-D2 = 2-3 days; FIX-D3 = 1-2 days per non-macOS profile.
**Scope:** public engine.

## TL;DR

Two related WebGL gaps that FIX-D (apple_m3_macos fixture alignment,
commit `a8cc691`) only fixed for the macOS preset:

- **FIX-D2** — `canvas_bootstrap.js::HTMLCanvasElement::getContext`
  returns the SAME `WebGLRenderingContext` instance for both
  `"webgl"` and `"webgl2"` requests. A site requesting WebGL 1 gets
  WebGL 2 strings + extensions + parameters back. Real Chrome has
  TWO distinct context types with different surfaces.

- **FIX-D3** — `nvidia_rtx_3060_windows` / `apple_m2_pro_macos` /
  `intel_uhd_630_linux` all still use the unmodified
  `common_params_desktop()` baseline that had the WRONG values FIX-D
  found (`MAX_VIEWPORT_DIMS=[32767,32767]` instead of per-hardware
  truth; `ALIASED_POINT_SIZE_RANGE=[1,8190]` instead of per-hardware
  truth). Plus their extension lists are still the WebGL-1-mixed
  state apple_m3_macos was in before FIX-D.

## Why this matters

AWS WAF's challenge.js reads at least:
- `getParameter(MAX_VIEWPORT_DIMS)`
- `getParameter(ALIASED_POINT_SIZE_RANGE)`
- `getParameter(MAX_TEXTURE_SIZE)`
- `getParameter(VENDOR)` / `RENDERER`
- `getParameter(UNMASKED_VENDOR_WEBGL)` / `UNMASKED_RENDERER_WEBGL`
- `getSupportedExtensions()` (extension list shape)
- `getContextAttributes()` (per-attribute booleans)
- `getShaderPrecisionFormat()`

Any of those mismatching real Chrome on the claimed GPU is a
fingerprint divergence the WAF can cluster on. FIX-D fixed macOS;
the same cluster fix for `chrome_148_windows` and `chrome_148_linux`
profiles probably flips amazon-de / amazon-fr / amazon-in / amazon-jp
on those profiles' sweeps.

FIX-D2 (context conflation) is more subtle but affects ANY site that
checks `getContext("webgl")` distinct from `"webgl2"`. Less common
than WebGL 2, but the AWS WAF challenge.js does emit both.

## Current state

### FIX-D2 specifics

`crates/js_runtime/src/js/canvas_bootstrap.js:925-964`:

```javascript
getContext(type) {
    if (type === "2d") return new CanvasRenderingContext2D(this.#canvasId);
    if (type === "webgl" || type === "webgl2" || type === "experimental-webgl") {
        const gl = new WebGLRenderingContext();
        gl.canvas = this;
        ...
    }
}
```

The same `WebGLRenderingContext` class is returned for any of the 3
type strings. Behaviour:
- A site calls `canvas.getContext("webgl")` and expects WebGL 1
  shape: `getParameter(VERSION) = "WebGL 1.0 (...)"`, `MAX_VIEWPORT_DIMS`
  uses WebGL 1 enum, etc.
- BO returns the apple_m3_macos `WebGL 2.0` strings instead → WebGL 1
  request sees WebGL 2 values.

Real Chrome:
- `getContext("webgl")` returns a `WebGLRenderingContext` (WebGL 1).
- `getContext("webgl2")` returns a `WebGL2RenderingContext`
  (subclasses `WebGLRenderingContext`, has WebGL 2 surface).

### FIX-D3 specifics

`crates/stealth/src/gpu.rs`:

- `nvidia_rtx_3060_windows()` (line 53) — still WebGL 1 strings +
  pre-FIX-D mixed extension list + shared `common_params_desktop()`.
- `apple_m2_pro_macos()` (line 169) — same as nvidia, still WebGL 1.
- `intel_uhd_630_linux()` (line 223) — same.

`common_params_desktop()` returns:
- `MAX_VIEWPORT_DIMS = [32767, 32767]` (CONFIRMED WRONG for M3; need
  per-hardware verification for nvidia / intel)
- `ALIASED_POINT_SIZE_RANGE = [1.0, 8190.0]` (same)

`apple_m3_macos()` overrides these via `apple_m3_params()` (FIX-D).
Other profiles do not.

## Next steps

### Step 1 — Get real Chrome 148 captures for nvidia + intel (~1 day per)

Need fixture JSON like `tests/fixtures/chrome147/captured_macos_arm64.json`
but for:
- Windows 10 + NVIDIA RTX (~80 GPU param values + 36-ish extensions)
- Linux + Intel UHD (~80 GPU param values + ~30 extensions)

Sources:
- Run a real Chrome instance on a Windows machine, dump WebGL params
  via a `getParameter` enumeration script.
- BrowserForge / Camoufox's `pythonlib/camoufox/webgl/webgl_data.db`
  SQLite — has captured fingerprints for NVIDIA / Intel / AMD per OS
  (verified Camoufox v150 source survey, audit `02_CAMOUFOX_V150_OVERVIEW.md`).
- Public WebGL fingerprint databases (browserleaks.com / creepjs).

### Step 2 — Ship FIX-D3 for nvidia_rtx_3060_windows (~1-2 days)

Mirror apple_m3_macos's structure:
- Use `nvidia_family_profile(chip_name)` if multiple Windows GPUs
  share the ANGLE D3D11 driver (most do).
- Custom `nvidia_rtx_3060_params()` overriding the shared baseline
  for nvidia-specific values.
- New snapshot test `nvidia_matches_captured_chrome_147_windows_fixture`.

### Step 3 — Same for intel_uhd_630_linux (~1-2 days)

Mirror with intel/Linux specifics. Intel UHD has different
extension list (no s3tc on Linux per Apple/Intel licensing).

### Step 4 — FIX-D2: separate WebGL2RenderingContext class

Bigger architectural change (~2-3 days):

- Add `class WebGL2RenderingContext extends WebGLRenderingContext`
  in canvas_bootstrap.js with the WebGL-2-specific strings + extension
  list + parameters.
- `getContext("webgl")` returns the parent class instance (WebGL 1
  strings + WebGL 1 extension list filtered to those built into 2);
  `getContext("webgl2")` returns the new subclass.
- Profile schema needs to support BOTH surfaces — either two
  GpuProfile fields (`gpu_profile_webgl1`, `gpu_profile_webgl2`) or
  extend the existing one to carry both extension lists + version
  strings.

Camoufox's MaskConfig.hpp has the same dichotomy: `webGl:parameters`
vs `webGl2:parameters`, `webGl:supportedExtensions` vs
`webGl2:supportedExtensions`. Model BO's schema on that.

### Step 5 — Validate

- Each per-GPU profile gets a snapshot test (mirror
  `apple_m3_matches_captured_chrome_147_fixture`).
- A 4-profile single-trial sweep against the 8 AWS WAF sites (with
  the new gate's results as baseline) — expect lift on Windows /
  Linux profile flips for the amazon-de / amazon-fr cluster.

## Dependencies

- Real Chrome 148 captures for nvidia / intel platforms (Step 1).
- FIX-D already shipped (commit `a8cc691`) — the apple_m3_macos
  pattern is the template.

## Sources / references

- `crates/stealth/src/gpu.rs:111-202` — apple_m3_macos + apple_m3_family_profile (the template)
- `crates/js_runtime/src/js/canvas_bootstrap.js:443-457` — getParameter dispatch
- `crates/js_runtime/src/js/canvas_bootstrap.js:925-964` — getContext (FIX-D2 site)
- `tests/fixtures/chrome147/captured_macos_arm64.json` — the FIX-D fixture template
- audit `15_FIX_PRIORITY_RANKED.md` row 4a + 4b — FIX-D2/D3 entries
- audit `16_DECISION_LOG.md` §FIX-D — apple_m3_macos pattern
