((globalThis) => {
    const ops = Deno.core.ops;

    // -- Canvas-based font detection support -----------------------------
    // Akamai/Kasada/PerimeterX fingerprint sensors detect installed fonts
    // by comparing measureText widths across candidate families: if
    // measureText("...", "Arial") differs from measureText("...", "sans-serif")
    // the family is reported as installed. Our font_database.rs aliases
    // every Chrome-on-OS family to bundled Liberation Sans/Serif/Mono,
    // so without this shim every probe collapses to identical widths and
    // the sensor reports `fonts=null`. Inject a deterministic, sub-pixel
    // family-derived delta so distinct family names produce distinct
    // widths — exactly what real Chrome does naturally because each face
    // ships with its own metrics.
    const _fontProbeFnvHash = (str) => {
        let h = 2166136261 >>> 0;
        for (let i = 0; i < str.length; i++) {
            h ^= str.charCodeAt(i);
            h = (h + ((h << 1) + (h << 4) + (h << 7) + (h << 8) + (h << 24))) >>> 0;
        }
        return h;
    };
    // Mirror the fonts present on Chrome for each OS — keep in sync with
    // `window_bootstrap.js` `Font enumeration spoofing` block.
    const _FONT_LIST_BY_OS = {
        "Windows": new Set([
            "arial","arial black","calibri","cambria","comic sans ms","consolas",
            "courier new","georgia","impact","lucida console","segoe ui","tahoma",
            "times new roman","trebuchet ms","verdana",
        ]),
        "macOS": new Set([
            "arial","arial black","courier new","georgia","helvetica",
            "helvetica neue","lucida grande","menlo","monaco","sf pro",
            "times new roman","trebuchet ms","verdana",
        ]),
        "Linux": new Set([
            "arial","courier new","dejavu sans","dejavu sans mono","dejavu serif",
            "liberation mono","liberation sans","liberation serif","noto sans",
            "times new roman","ubuntu","verdana",
        ]),
    };
    const _resolveInstalledFonts = () => {
        try {
            const has = ops.op_has_stealth_profile && ops.op_has_stealth_profile();
            const os = has ? (ops.op_get_profile_value("os_name") || "Linux") : "Linux";
            return _FONT_LIST_BY_OS[os] || _FONT_LIST_BY_OS["Linux"];
        } catch (_e) {
            return _FONT_LIST_BY_OS["Linux"];
        }
    };
    const _GENERIC_FAMILIES = new Set(["sans-serif","serif","monospace","cursive","fantasy","system-ui","ui-sans-serif","ui-serif","ui-monospace"]);
    const _primaryFontFamily = (fontStr) => {
        if (!fontStr) return null;
        // Strip CSS font shorthand prefix (style/variant/weight/stretch/size/line-height).
        // The family list is everything after the last whitespace following the size token.
        const sizeMatch = fontStr.match(/(\d+(?:\.\d+)?)(px|pt|em|rem|%|vh|vw)\s+(.+)$/);
        const familyList = sizeMatch ? sizeMatch[3] : fontStr;
        const first = familyList.split(",")[0] || "";
        return first.replace(/["']/g, "").trim().toLowerCase();
    };
    // 0.0 .. ~3.5 px deterministic delta. Sub-character-width so layout
    // stays stable, large enough to clear 1e-3 fingerprint comparisons.
    const _fontFamilyWidthDelta = (family) => {
        if (!family) return 0;
        if (_GENERIC_FAMILIES.has(family)) return 0; // generics are baselines
        if (!_resolveInstalledFonts().has(family)) return 0; // not installed on this OS
        const h = _fontProbeFnvHash(family);
        return (h % 7000) / 2000; // 0.0 .. 3.5 px
    };

    // Parse CSS color to [r, g, b, a]
    function _parseColor(str) {
        const named = { red:[255,0,0,255], green:[0,128,0,255], blue:[0,0,255,255],
            black:[0,0,0,255], white:[255,255,255,255], yellow:[255,255,0,255],
            cyan:[0,255,255,255], magenta:[255,0,255,255], transparent:[0,0,0,0] };
        if (named[str]) return named[str];
        if (str.startsWith('#')) {
            const h = str.slice(1);
            if (h.length === 3) return [parseInt(h[0]+h[0],16), parseInt(h[1]+h[1],16), parseInt(h[2]+h[2],16), 255];
            if (h.length === 6) return [parseInt(h.slice(0,2),16), parseInt(h.slice(2,4),16), parseInt(h.slice(4,6),16), 255];
        }
        const m = str.match(/rgba?\((\d+),\s*(\d+),\s*(\d+)(?:,\s*([\d.]+))?\)/);
        if (m) return [+m[1], +m[2], +m[3], m[4] !== undefined ? Math.round(+m[4]*255) : 255];
        return [0, 0, 0, 255];
    }

    class CanvasRenderingContext2D {
        #id;
        constructor(id) { this.#id = id; }

        // Style
        set fillStyle(v) {
            if (v && typeof v === "object" && v._type) {
                // Gradient object
                const stops = (v._stops || []).map(s => {
                    const c = _parseColor(s.color);
                    return [s.offset, c[0], c[1], c[2], c[3]];
                });
                let coords;
                if (v._type === "linear") {
                    coords = [v._x0, v._y0, v._x1, v._y1];
                } else {
                    coords = [v._x0, v._y0, v._r0, v._x1, v._y1, v._r1];
                }
                ops.op_canvas_set_fill_gradient(this.#id, v._type, JSON.stringify({ coords, stops }));
            } else {
                ops.op_canvas_set_fill_style(this.#id, String(v));
            }
        }
        set strokeStyle(v) { ops.op_canvas_set_stroke_style(this.#id, String(v)); }
        set lineWidth(v) { ops.op_canvas_set_line_width(this.#id, +v); }
        set globalAlpha(v) { ops.op_canvas_set_global_alpha(this.#id, +v); }
        set font(v) { this._font = String(v); ops.op_canvas_set_font(this.#id, this._font); }
        get font() { return this._font || "10px sans-serif"; }

        // Rectangles
        fillRect(x, y, w, h) { ops.op_canvas_fill_rect(this.#id, x, y, w, h); }
        strokeRect(x, y, w, h) { ops.op_canvas_stroke_rect(this.#id, x, y, w, h); }
        clearRect(x, y, w, h) { ops.op_canvas_clear_rect(this.#id, x, y, w, h); }

        // Path
        beginPath() { ops.op_canvas_begin_path(this.#id); }
        moveTo(x, y) { ops.op_canvas_move_to(this.#id, x, y); }
        lineTo(x, y) { ops.op_canvas_line_to(this.#id, x, y); }
        fill() { ops.op_canvas_fill(this.#id); }
        stroke() { ops.op_canvas_stroke(this.#id); }
        closePath() { ops.op_canvas_close_path(this.#id); }
        arc(x, y, r, startAngle, endAngle, counterclockwise) {
            ops.op_canvas_arc(this.#id, x, y, r, startAngle, endAngle, !!counterclockwise);
        }
        arcTo(x1, y1, x2, y2, r) {
            ops.op_canvas_arc_to(this.#id, x1, y1, x2, y2, r);
        }
        bezierCurveTo(cp1x, cp1y, cp2x, cp2y, x, y) {
            ops.op_canvas_bezier_curve_to(this.#id, cp1x, cp1y, cp2x, cp2y, x, y);
        }
        quadraticCurveTo(cpx, cpy, x, y) {
            ops.op_canvas_quadratic_curve_to(this.#id, cpx, cpy, x, y);
        }
        ellipse(x, y, rx, ry, rotation, startAngle, endAngle, counterclockwise) {
            ops.op_canvas_ellipse(this.#id, x, y, rx, ry, rotation, startAngle, endAngle, !!counterclockwise);
        }
        rect(x, y, w, h) { this.moveTo(x,y); this.lineTo(x+w,y); this.lineTo(x+w,y+h); this.lineTo(x,y+h); this.closePath(); }

        // Text
        fillText(text, x, y) { ops.op_canvas_fill_text(this.#id, text, x, y); }
        strokeText(text, x, y) { ops.op_canvas_stroke_text(this.#id, text, x, y); }
        measureText(text) {
            // Full 13-field TextMetrics shaped in Rust (T1.2 font stack).
            // actualBoundingBox* come from the real glyph run, not a
            // derived ratio — this is what fingerprint sites probe.
            const m = ops.op_canvas_measure_text_full(this.#id, text);
            // Per-family micro-delta so canvas-based font detection works.
            // See `_fontFamilyWidthDelta` for rationale.
            const fam = _primaryFontFamily(this._font);
            const deltaPerChar = _fontFamilyWidthDelta(fam);
            const len = (typeof text === "string") ? text.length : 0;
            const widthDelta = deltaPerChar * Math.max(1, len) * 0.25;
            return {
                width: m.width + widthDelta,
                actualBoundingBoxLeft: m.actual_bounding_box_left,
                actualBoundingBoxRight: m.actual_bounding_box_right + widthDelta,
                actualBoundingBoxAscent: m.actual_bounding_box_ascent,
                actualBoundingBoxDescent: m.actual_bounding_box_descent,
                fontBoundingBoxAscent: m.font_bounding_box_ascent,
                fontBoundingBoxDescent: m.font_bounding_box_descent,
                emHeightAscent: m.em_height_ascent,
                emHeightDescent: m.em_height_descent,
                alphabeticBaseline: m.alphabetic_baseline,
                hangingBaseline: m.hanging_baseline,
                ideographicBaseline: m.ideographic_baseline,
            };
        }

        // Transform
        save() { ops.op_canvas_save(this.#id); }
        restore() { ops.op_canvas_restore(this.#id); }
        translate(x, y) { ops.op_canvas_translate(this.#id, x, y); }
        rotate(angle) { ops.op_canvas_rotate(this.#id, angle); }
        scale(x, y) { ops.op_canvas_scale(this.#id, x, y); }
        setTransform(a, b, c, d, e, f) {
            // Spec also accepts a single DOMMatrix-init dict; handle both shapes.
            if (typeof a === "object" && a !== null) {
                ops.op_canvas_set_transform(
                    this.#id, a.a ?? 1, a.b ?? 0, a.c ?? 0, a.d ?? 1, a.e ?? 0, a.f ?? 0
                );
            } else {
                ops.op_canvas_set_transform(this.#id, a, b, c, d, e, f);
            }
        }
        resetTransform() { ops.op_canvas_reset_transform(this.#id); }
        getTransform() { return {a:1,b:0,c:0,d:1,e:0,f:0}; }

        // Image data — real pixel ops
        getImageData(x, y, w, h) {
            const raw = ops.op_canvas_get_image_data(this.#id, x, y, w, h);
            return { data: new Uint8ClampedArray(raw), width: w, height: h };
        }
        putImageData(imageData, dx, dy) {
            ops.op_canvas_put_image_data(this.#id, imageData.data, dx, dy, imageData.width, imageData.height);
        }
        createImageData(w, h) { return { data: new Uint8ClampedArray(w * h * 4), width: w, height: h }; }
        drawImage(source, dx, dy) {
            // source can be another canvas element — get its internal ID
            if (source && source._canvasId !== undefined) {
                ops.op_canvas_draw_image(this.#id, source._canvasId, dx || 0, dy || 0);
            }
        }

        // Gradient — JS-side objects that track color stops
        createLinearGradient(x0, y0, x1, y1) {
            const stops = [];
            return {
                addColorStop(offset, color) { stops.push({ offset, color }); },
                _stops: stops, _type: 'linear', _x0: x0, _y0: y0, _x1: x1, _y1: y1,
            };
        }
        createRadialGradient(x0, y0, r0, x1, y1, r1) {
            const stops = [];
            return {
                addColorStop(offset, color) { stops.push({ offset, color }); },
                _stops: stops, _type: 'radial', _x0: x0, _y0: y0, _r0: r0, _x1: x1, _y1: y1, _r1: r1,
            };
        }
        createPattern(image, repetition) { return { _image: image, _repetition: repetition || 'repeat' }; }

        // Clip
        clip() {}
        isPointInPath() { return false; }
        isPointInStroke() { return false; }
    }

    // WebGL — routes through Canvas2D backend for real pixel output.
    // Anti-bot fingerprinters call readPixels() after clearColor()+clear() and expect real data.
    class WebGLRenderingContext {
        // WebGL constants
        static COLOR_BUFFER_BIT = 0x4000;
        static DEPTH_BUFFER_BIT = 0x0100;
        static STENCIL_BUFFER_BIT = 0x0400;
        static TRIANGLES = 4;
        static TRIANGLE_STRIP = 5;
        static TRIANGLE_FAN = 6;
        static LINES = 1;
        static LINE_STRIP = 3;
        static POINTS = 0;
        static RGBA = 0x1908;
        static UNSIGNED_BYTE = 0x1401;
        static FLOAT = 0x1406;
        static ARRAY_BUFFER = 0x8892;
        static ELEMENT_ARRAY_BUFFER = 0x8893;
        static FRAGMENT_SHADER = 0x8B30;
        static VERTEX_SHADER = 0x8B31;
        static COMPILE_STATUS = 0x8B81;
        static LINK_STATUS = 0x8B82;
        // Parameter pname constants — anti-bot probes call e.g. gl.getParameter(gl.MAX_TEXTURE_SIZE).
        static VENDOR = 0x1F00;
        static RENDERER = 0x1F01;
        static VERSION = 0x1F02;
        static SHADING_LANGUAGE_VERSION = 0x8B8C;
        static MAX_TEXTURE_SIZE = 0x0D33;
        static MAX_CUBE_MAP_TEXTURE_SIZE = 0x851C;
        static MAX_RENDERBUFFER_SIZE = 0x84E8;
        static MAX_3D_TEXTURE_SIZE = 0x8073;
        static MAX_VERTEX_ATTRIBS = 0x8869;
        static MAX_VERTEX_UNIFORM_VECTORS = 0x8DFB;
        static MAX_VARYING_VECTORS = 0x8DFD;
        static MAX_FRAGMENT_UNIFORM_VECTORS = 0x8DFC;
        static MAX_TEXTURE_IMAGE_UNITS = 0x8872;
        static MAX_VERTEX_TEXTURE_IMAGE_UNITS = 0x8B4D;
        static MAX_COMBINED_TEXTURE_IMAGE_UNITS = 0x8B4C;
        static ALIASED_POINT_SIZE_RANGE = 0x846D;
        static ALIASED_LINE_WIDTH_RANGE = 0x846E;
        static MAX_VIEWPORT_DIMS = 0x0D3A;
        static DEPTH_BITS = 0x0D56;
        static STENCIL_BITS = 0x0D57;
        static SAMPLE_BUFFERS = 0x80AA;
        static SAMPLES = 0x80A9;
        // Shader-precision-format types
        static LOW_FLOAT = 0x8DF0;
        static MEDIUM_FLOAT = 0x8DF1;
        static HIGH_FLOAT = 0x8DF2;
        static LOW_INT = 0x8DF3;
        static MEDIUM_INT = 0x8DF4;
        static HIGH_INT = 0x8DF5;

        constructor(canvasId, width, height) {
            this._canvasId = canvasId;
            this._width = width || 300;
            this._height = height || 150;
            this._clearColor = [0, 0, 0, 0];
            this.canvas = null;
            this.drawingBufferWidth = this._width;
            this.drawingBufferHeight = this._height;
            // Copy constants to instance
            for (const k of Object.getOwnPropertyNames(WebGLRenderingContext)) {
                if (typeof WebGLRenderingContext[k] === 'number') this[k] = WebGLRenderingContext[k];
            }
        }

        // --- Real operations via Canvas2D backend ---
        clearColor(r, g, b, a) {
            this._clearColor = [Math.round(r*255), Math.round(g*255), Math.round(b*255), a];
        }
        clear(mask) {
            if (mask & 0x4000 && this._canvasId !== undefined) { // COLOR_BUFFER_BIT
                const [r, g, b, a] = this._clearColor;
                const color = `rgba(${r},${g},${b},${a})`;
                ops.op_canvas_set_fill_style(this._canvasId, color);
                ops.op_canvas_fill_rect(this._canvasId, 0, 0, this._width, this._height);
            }
        }
        readPixels(x, y, w, h, format, type, pixels) {
            if (this._canvasId === undefined || !pixels) return;
            // Canvas2D stores pixels top-down, WebGL is bottom-up — flip Y
            const flippedY = this._height - y - h;
            const data = ops.op_canvas_get_image_data(this._canvasId, x, Math.max(0, flippedY), w, h);
            for (let i = 0; i < data.length && i < pixels.length; i++) {
                pixels[i] = data[i];
            }
        }
        viewport(x, y, w, h) {
            this._width = w || this._width;
            this._height = h || this._height;
        }

        // --- Parameter queries (anti-bot fingerprint values) ---
        //
        // All values come from the active StealthProfile's gpu_profile entry.
        // Loaded lazily the first time getParameter is called and cached on
        // the WebGLRenderingContext constructor itself (shared across instances).
        _loadGpuProfile() {
            if (WebGLRenderingContext._gpuCache) return WebGLRenderingContext._gpuCache;
            // Defaults — used when no stealth profile is active. Must match
            // stealth::gpu::common_params_desktop() so probes that check for
            // non-zero MAX_TEXTURE_SIZE etc. don't see `null` in headless mode.
            // Defaults match captured Chrome 147 on macOS arm64
            // (tests/fixtures/chrome147/captured_macos_arm64.json).
            let vendor = "WebKit";
            let renderer = "WebKit WebGL";
            let version = "WebGL 2.0 (OpenGL ES 3.0 Chromium)";
            let shadingLang = "WebGL GLSL ES 3.00 (OpenGL ES GLSL ES 3.0 Chromium)";
            let unmaskedVendor = "Google Inc. (Apple)";
            let unmaskedRenderer = "ANGLE (Apple, ANGLE Metal Renderer: Apple M3, Unspecified Version)";
            let extensions = [];
            let params = {
                0x0D33: 16384,         // MAX_TEXTURE_SIZE
                0x851C: 16384,         // MAX_CUBE_MAP_TEXTURE_SIZE
                0x84E8: 16384,         // MAX_RENDERBUFFER_SIZE
                0x8073: 2048,          // MAX_3D_TEXTURE_SIZE
                0x8869: 16,            // MAX_VERTEX_ATTRIBS
                0x8DFB: 1024,          // MAX_VERTEX_UNIFORM_VECTORS
                0x8DFD: 15,            // MAX_VARYING_VECTORS
                0x8DFC: 1024,          // MAX_FRAGMENT_UNIFORM_VECTORS
                0x8872: 16,            // MAX_TEXTURE_IMAGE_UNITS
                0x8B4D: 16,            // MAX_VERTEX_TEXTURE_IMAGE_UNITS
                0x8B4C: 32,            // MAX_COMBINED_TEXTURE_IMAGE_UNITS
                // ALIASED_POINT_SIZE_RANGE — captured Chrome 147 macOS: [1, 511] typical
                0x846D: [1.0, 511.0],
                0x846E: [1.0, 1.0],    // ALIASED_LINE_WIDTH_RANGE — Chrome ANGLE on every OS = [1,1]
                0x0D3A: [16384, 16384],// MAX_VIEWPORT_DIMS — captured Chrome 147 macOS
                0x0D56: 8,             // DEPTH_BITS
                0x0D57: 8,             // STENCIL_BITS
                0x80AA: 2,             // SAMPLE_BUFFERS
                0x80A9: 4,             // SAMPLES
            };
            let shaderPrec = {};
            try {
                if (ops.op_has_stealth_profile()) {
                    const s = (k) => ops.op_get_profile_value(k);
                    unmaskedVendor = s("webgl_unmasked_vendor") || unmaskedVendor;
                    unmaskedRenderer = s("webgl_unmasked_renderer") || unmaskedRenderer;
                    version = s("webgl_version") || version;
                    shadingLang = s("webgl_shading_language_version") || shadingLang;
                    const extsJson = s("webgl_extensions");
                    if (extsJson) {
                        try { extensions = JSON.parse(extsJson); } catch {}
                    }
                    const paramsJson = s("webgl_params");
                    if (paramsJson) {
                        try {
                            const arr = JSON.parse(paramsJson);
                            // Array of [glenum, value] pairs → keyed object
                            for (const [k, v] of arr) params[k] = v;
                        } catch {}
                    }
                    const spJson = s("webgl_shader_precision");
                    if (spJson) {
                        try {
                            // Array of [shader_type, precision_type, [min, max, precision]]
                            const arr = JSON.parse(spJson);
                            for (const [st, pt, v] of arr) {
                                shaderPrec[`${st}:${pt}`] = { rangeMin: v[0], rangeMax: v[1], precision: v[2] };
                            }
                        } catch {}
                    }
                }
            } catch {}
            WebGLRenderingContext._gpuCache = {
                vendor, renderer, version, shadingLang,
                unmaskedVendor, unmaskedRenderer,
                extensions, params, shaderPrec,
            };
            return WebGLRenderingContext._gpuCache;
        }
        getParameter(pname) {
            const gpu = this._loadGpuProfile();
            // String-valued parameters
            if (pname === 0x1F00) return gpu.vendor;                // VENDOR
            if (pname === 0x1F01) return gpu.renderer;              // RENDERER
            if (pname === 0x1F02) return gpu.version;               // VERSION
            if (pname === 0x8B8C) return gpu.shadingLang;           // SHADING_LANGUAGE_VERSION
            if (pname === 0x9245) return gpu.unmaskedVendor;        // UNMASKED_VENDOR_WEBGL
            if (pname === 0x9246) return gpu.unmaskedRenderer;      // UNMASKED_RENDERER_WEBGL
            // Runtime-dependent values (not from the catalog)
            if (pname === 0x0BA2) return [0, 0, this._width, this._height]; // VIEWPORT
            // Catalog-sourced numeric/array parameters
            if (gpu.params[pname] !== undefined) return gpu.params[pname];
            return null;
        }
        getSupportedExtensions() {
            const gpu = this._loadGpuProfile();
            // Fallback if the catalog is empty (no profile active).
            // Captured from real Chrome 147 on macOS arm64 — 36 extensions, exact list.
            // See tests/fixtures/chrome147/captured_macos_arm64.json.
            if (!gpu.extensions.length) {
                return [
                    "EXT_clip_control","EXT_color_buffer_float","EXT_color_buffer_half_float",
                    "EXT_conservative_depth","EXT_depth_clamp","EXT_disjoint_timer_query_webgl2",
                    "EXT_float_blend","EXT_polygon_offset_clamp","EXT_render_snorm",
                    "EXT_texture_compression_bptc","EXT_texture_compression_rgtc",
                    "EXT_texture_filter_anisotropic","EXT_texture_mirror_clamp_to_edge",
                    "EXT_texture_norm16","KHR_parallel_shader_compile",
                    "NV_shader_noperspective_interpolation","OES_draw_buffers_indexed",
                    "OES_sample_variables","OES_shader_multisample_interpolation",
                    "OES_texture_float_linear","WEBGL_blend_func_extended",
                    "WEBGL_clip_cull_distance","WEBGL_compressed_texture_astc",
                    "WEBGL_compressed_texture_etc","WEBGL_compressed_texture_etc1",
                    "WEBGL_compressed_texture_pvrtc","WEBGL_compressed_texture_s3tc",
                    "WEBGL_compressed_texture_s3tc_srgb","WEBGL_debug_renderer_info",
                    "WEBGL_debug_shaders","WEBGL_lose_context","WEBGL_multi_draw",
                    "WEBGL_polygon_mode","WEBGL_provoking_vertex",
                    "WEBGL_render_shared_exponent","WEBGL_stencil_texturing",
                ];
            }
            return gpu.extensions.slice();
        }
        getExtension(name) {
            if (name === "WEBGL_debug_renderer_info") return { UNMASKED_VENDOR_WEBGL: 0x9245, UNMASKED_RENDERER_WEBGL: 0x9246 };
            // Any supported extension gets a non-null stub. Fingerprinters
            // call getExtension(name) after getSupportedExtensions to verify.
            const gpu = this._loadGpuProfile();
            if (gpu.extensions.includes(name)) return {};
            return null;
        }
        // getContextAttributes — returns the WebGLContextAttributes used at
        // creation. Real Chrome returns these specific defaults.
        getContextAttributes() {
            return {
                alpha: true,
                antialias: true,
                depth: true,
                failIfMajorPerformanceCaveat: false,
                powerPreference: "default",
                premultipliedAlpha: true,
                preserveDrawingBuffer: false,
                stencil: false,
                desynchronized: false,
                xrCompatible: false,
            };
        }
        isContextLost() { return false; }
        getShaderPrecisionFormat(shaderType, precisionType) {
            const gpu = this._loadGpuProfile();
            const key = `${shaderType}:${precisionType}`;
            if (gpu.shaderPrec[key]) return gpu.shaderPrec[key];
            // Fallback for unknown combinations — float-style values (our old behavior)
            return { rangeMin: 127, rangeMax: 127, precision: 23 };
        }

        // --- Shader/program stubs (needed for API surface) ---
        createShader() { return { _id: 1 }; }
        shaderSource() {}
        compileShader() {}
        getShaderParameter() { return true; }
        createProgram() { return { _id: 1 }; }
        attachShader() {}
        linkProgram() {}
        getProgramParameter() { return true; }
        useProgram() {}
        getUniformLocation() { return { _id: 0 }; }
        getAttribLocation() { return 0; }
        uniform1f() {}
        uniform1i() {}
        uniform2f() {}
        uniform3f() {}
        uniform4f() {}
        uniformMatrix4fv() {}
        createBuffer() { return { _id: 1 }; }
        bindBuffer() {}
        bufferData() {}
        enableVertexAttribArray() {}
        disableVertexAttribArray() {}
        vertexAttribPointer() {}
        drawArrays() {}
        drawElements() {}
        createTexture() { return { _id: 1 }; }
        bindTexture() {}
        texImage2D() {}
        texParameteri() {}
        activeTexture() {}
        generateMipmap() {}
        createFramebuffer() { return { _id: 1 }; }
        bindFramebuffer() {}
        framebufferTexture2D() {}
        createRenderbuffer() { return { _id: 1 }; }
        bindRenderbuffer() {}
        renderbufferStorage() {}
        framebufferRenderbuffer() {}
        checkFramebufferStatus() { return 0x8CD5; } // FRAMEBUFFER_COMPLETE
        enable() {}
        disable() {}
        blendFunc() {}
        blendEquation() {}
        depthFunc() {}
        depthMask() {}
        colorMask() {}
        scissor() {}
        pixelStorei() {}
        getError() { return 0; }
        flush() {}
        finish() {}
        deleteShader() {}
        deleteProgram() {}
        deleteBuffer() {}
        deleteTexture() {}
        deleteFramebuffer() {}
        deleteRenderbuffer() {}
        isContextLost() { return false; }
    }

    // AudioContext + OfflineAudioContext
    // Simulates the pipeline used by CreepJS/FingerprintJS for audio fingerprinting:
    //   OscillatorNode → DynamicsCompressorNode → destination
    
    class AudioNode extends EventTarget {
        constructor() { super(); }
        connect() {}
        disconnect() {}
    }

    class AudioScheduledSourceNode extends AudioNode {
        constructor() { super(); }
        start() {}
        stop() {}
    }

    class OscillatorNode extends AudioScheduledSourceNode {
        _type = "sine";
        constructor(context) {
            super();
            this._context = context;
            this.frequency = {
                _value: 440,
                get value() { return this._value; },
                set value(v) { this._value = v; if (context._setOscFreq) context._setOscFreq(v); }
            };
            this.detune = { value: 0 };
        }
        get type() { return this._type; }
        set type(v) { this._type = v; if (this._context._setOscType) this._context._setOscType(v); }
    }

    class AudioParam {
        constructor(val, context, setter) {
            this._value = val;
            this._context = context;
            this._setter = setter;
        }
        get value() { return this._value; }
        set value(v) { this._value = v; if (this._setter) this._setter(v); }
        setValueAtTime() { return this; }
        linearRampToValueAtTime() { return this; }
        exponentialRampToValueAtTime() { return this; }
        setTargetAtTime() { return this; }
        setValueCurveAtTime() { return this; }
        cancelScheduledValues() { return this; }
        cancelAndHoldAtTime() { return this; }
    }

    class GainNode extends AudioNode {
        constructor() {
            super();
            this.gain = new AudioParam(1);
        }
    }

    class DynamicsCompressorNode extends AudioNode {
        constructor(context) {
            super();
            this.threshold = new AudioParam(-24, context, v => { if (context._setCompThreshold) context._setCompThreshold(v); });
            this.knee = new AudioParam(30, context, v => { if (context._setCompKnee) context._setCompKnee(v); });
            this.ratio = new AudioParam(12, context, v => { if (context._setCompRatio) context._setCompRatio(v); });
            this.attack = new AudioParam(0.003, context, v => { if (context._setCompAttack) context._setCompAttack(v); });
            this.release = new AudioParam(0.25, context, v => { if (context._setCompRelease) context._setCompRelease(v); });
        }
    }

    class BiquadFilterNode extends AudioNode {
        constructor() {
            super();
            this.type = "lowpass";
            this.frequency = new AudioParam(350);
            this.detune = new AudioParam(0);
            this.Q = new AudioParam(1);
            this.gain = new AudioParam(0);
        }
        getFrequencyResponse(freqArr, magOut, phaseOut) {
            if (!(freqArr instanceof Float32Array)) return;
            const _typeIds = {
                lowpass: 0, highpass: 1, bandpass: 2, lowshelf: 3,
                highshelf: 4, peaking: 5, notch: 6, allpass: 7,
            };
            const tid = _typeIds[this.type] ?? 0;
            const sr = (this._sampleRate || 44100);
            const inBytes = new Uint8Array(freqArr.buffer, freqArr.byteOffset, freqArr.byteLength);
            const out = ops.op_audio_biquad_response(
                inBytes, tid,
                this.frequency.value, this.Q.value,
                this.gain.value, sr
            );
            const result = new Float32Array(out.buffer, out.byteOffset, out.byteLength / 4);
            const n = freqArr.length;
            const lenM = Math.min(magOut.length, n);
            const lenP = Math.min(phaseOut.length, n);
            for (let i = 0; i < lenM; i++) magOut[i] = result[i];
            for (let i = 0; i < lenP; i++) phaseOut[i] = result[n + i];
        }
    }

    class AnalyserNode extends AudioNode {
        constructor() {
            super();
            this.fftSize = 2048;
            this.smoothingTimeConstant = 0.8;
            this.minDecibels = -100;
            this.maxDecibels = -30;
            this._timeDomain = null;
            this._prevFreq = null;
        }
        get frequencyBinCount() { return this.fftSize / 2; }
        getByteFrequencyData(arr) {
            const f = new Float32Array(this.frequencyBinCount);
            this.getFloatFrequencyData(f);
            const range = this.maxDecibels - this.minDecibels;
            const len = Math.min(arr.length, f.length);
            for (let i = 0; i < len; i++) {
                const norm = (f[i] - this.minDecibels) / range;
                arr[i] = Math.max(0, Math.min(255, Math.round(norm * 255)));
            }
        }
        getFloatFrequencyData(arr) {
            if (!this._timeDomain || this._timeDomain.length < this.fftSize) {
                for (let i = 0; i < arr.length; i++) arr[i] = this.minDecibels;
                return;
            }
            const tdBytes = new Uint8Array(this._timeDomain.buffer, 0, this.fftSize * 4);
            const prevBytes = this._prevFreq
                ? new Uint8Array(this._prevFreq.buffer)
                : new Uint8Array(0);
            const out = ops.op_audio_analyser_freq_data(
                tdBytes, this.fftSize,
                Math.round(this.smoothingTimeConstant * 100),
                prevBytes
            );
            const result = new Float32Array(out.buffer, out.byteOffset, out.byteLength / 4);
            const len = Math.min(arr.length, result.length);
            for (let i = 0; i < len; i++) arr[i] = result[i];
            this._prevFreq = result.slice();
        }
        getByteTimeDomainData(arr) {
            if (!this._timeDomain) {
                for (let i = 0; i < arr.length; i++) arr[i] = 128;
                return;
            }
            const len = Math.min(arr.length, this._timeDomain.length);
            for (let i = 0; i < len; i++) {
                arr[i] = Math.max(0, Math.min(255, Math.round((this._timeDomain[i] + 1) * 127.5)));
            }
        }
        getFloatTimeDomainData(arr) {
            if (!this._timeDomain) {
                for (let i = 0; i < arr.length; i++) arr[i] = 0;
                return;
            }
            const len = Math.min(arr.length, this._timeDomain.length);
            for (let i = 0; i < len; i++) arr[i] = this._timeDomain[i];
        }
    }

    class AudioDestinationNode extends AudioNode {
        constructor() { super(); this.maxChannelCount = 2; }
    }

    class AudioContext extends EventTarget {
        constructor() {
            super();
            this.sampleRate = 44100;
            this.state = "running";
            this.currentTime = 0;
            this.destination = new AudioDestinationNode();
        }
        createOscillator() { return new OscillatorNode(this); }
        createDynamicsCompressor() { return new DynamicsCompressorNode(this); }
        createAnalyser() { return new AnalyserNode(); }
        createGain() { return new GainNode(); }
        createBiquadFilter() { return new BiquadFilterNode(); }
        createBufferSource() {
             return { connect() {}, start() {}, stop() {}, buffer: null, loop: false };
        }
        createBuffer(channels, length, sampleRate) {
            const bufs = [];
            for (let c = 0; c < channels; c++) bufs.push(new Float32Array(length));
            return {
                numberOfChannels: channels, length, sampleRate,
                duration: length / sampleRate,
                getChannelData(c) { return bufs[c]; }
            };
        }
        decodeAudioData() { return Promise.resolve(); }
        close() { return Promise.resolve(); }
        resume() { return Promise.resolve(); }
        suspend() { return Promise.resolve(); }
    }

    class OfflineAudioContext extends AudioContext {
        constructor(channels, length, sampleRate) {
            super();
            this._channels = channels || 1;
            this._length = length || 44100;
            this.sampleRate = sampleRate || 44100;
            this._oscType = "triangle";
            this._oscFreq = 10000;
            this._compThreshold = -24;
            this._compKnee = 30;
            this._compRatio = 12;
            this._compAttack = 0.003;
            this._compRelease = 0.25;
        }
        _setOscType(v) { this._oscType = v; }
        _setOscFreq(v) { this._oscFreq = v; }
        _setCompThreshold(v) { this._compThreshold = v; }
        _setCompKnee(v) { this._compKnee = v; }
        _setCompRatio(v) { this._compRatio = v; }
        _setCompAttack(v) { this._compAttack = v; }
        _setCompRelease(v) { this._compRelease = v; }

        startRendering() {
            const self = this;
            return new Promise((resolve) => {
                const sr = self.sampleRate;
                const len = self._length;
                const freq = self._oscFreq;
                const type = self._oscType;
                const waveTypeId = type === "sine" ? 0
                    : type === "square" ? 2
                    : type === "sawtooth" ? 3
                    : 1; // triangle

                let seed = 0;
                try {
                    if (typeof Deno !== 'undefined' && Deno.core?.ops?.op_get_profile_value) {
                        const raw = Deno.core.ops.op_get_profile_value("audio_seed");
                        if (raw) {
                            const parsed = parseInt(raw, 10);
                            if (!Number.isNaN(parsed)) seed = parsed | 0;
                        }
                    }
                } catch (e) {}

                let data;
                try {
                    const bytes = ops.op_offline_audio_render(
                        seed, sr | 0, len | 0, freq, waveTypeId,
                        self._compThreshold, self._compKnee, self._compRatio,
                        self._compAttack, self._compRelease,
                    );
                    data = new Float32Array(bytes.buffer, bytes.byteOffset, len);
                } catch (e) {
                    data = new Float32Array(len);
                }

                const buf = {
                    numberOfChannels: self._channels,
                    length: len,
                    sampleRate: sr,
                    duration: len / sr,
                    getChannelData() { return data; },
                };
                resolve(buf);
            });
        }
    }

    // HTMLCanvasElement: getContext returns the right context
    class HTMLCanvasElement {
        #canvasId;
        #attrs;
        constructor(width = 300, height = 150) {
            this.#canvasId = ops.op_canvas_create(width, height);
            this.#attrs = { width: String(width), height: String(height) };
            Object.defineProperty(this, 'width', { value: width, writable: true, enumerable: true, configurable: true });
            Object.defineProperty(this, 'height', { value: height, writable: true, enumerable: true, configurable: true });
            // Element base properties — fpCollect and bot.sannysoft expect these.
            // Use defineProperty because Element.prototype (which we chain into
            // at the bottom of this file) has tagName/nodeName/etc. as getters
            // with no setters — direct assignment would fail.
            Object.defineProperty(this, 'tagName', { value: 'CANVAS', configurable: true, writable: true });
            Object.defineProperty(this, 'nodeName', { value: 'CANVAS', configurable: true, writable: true });
            Object.defineProperty(this, 'nodeType', { value: 1, configurable: true, writable: true });
            Object.defineProperty(this, 'style', { value: { cssText: "" }, configurable: true, writable: true });
            Object.defineProperty(this, 'classList', {
                value: { add() {}, remove() {}, toggle() {}, contains() { return false; } },
                configurable: true, writable: true,
            });
            Object.defineProperty(this, 'dataset', { value: {}, configurable: true, writable: true });
            Object.defineProperty(this, 'childNodes', { value: [], configurable: true, writable: true });
            Object.defineProperty(this, 'children', { value: [], configurable: true, writable: true });
        }
        // Attribute API — required by canvas fingerprinters that do
        // `canvas.setAttribute('width', 200)` before drawing.
        setAttribute(name, value) {
            this.#attrs[name] = String(value);
            if (name === "width") {
                Object.defineProperty(this, 'width', { value: parseInt(value, 10) || this.width, writable: true, enumerable: true, configurable: true });
            }
            if (name === "height") {
                Object.defineProperty(this, 'height', { value: parseInt(value, 10) || this.height, writable: true, enumerable: true, configurable: true });
            }
        }
        getAttribute(name) { return this.#attrs[name] !== undefined ? this.#attrs[name] : null; }
        removeAttribute(name) { delete this.#attrs[name]; }
        hasAttribute(name) { return name in this.#attrs; }
        getContext(type) {
            if (type === "2d") return new CanvasRenderingContext2D(this.#canvasId);
            if (type === "webgl" || type === "webgl2" || type === "experimental-webgl") {
                const gl = new WebGLRenderingContext();
                gl.canvas = this;
                gl.drawingBufferWidth = this.width;
                gl.drawingBufferHeight = this.height;
                return gl;
            }
            return null;
        }
        toDataURL(type) { return ops.op_canvas_to_data_url(this.#canvasId); }
        toBlob(cb, type) { cb(new Blob([this.toDataURL()])); }
        // Minimal Node API
        appendChild(child) { this.childNodes.push(child); return child; }
        removeChild(child) {
            const i = this.childNodes.indexOf(child);
            if (i >= 0) this.childNodes.splice(i, 1);
            return child;
        }
        addEventListener() {}
        removeEventListener() {}
        dispatchEvent() { return true; }
        // Clone / get bounding box — fingerprint probes may call these
        cloneNode() { return new HTMLCanvasElement(this.width, this.height); }
        getBoundingClientRect() {
            return { x: 0, y: 0, width: this.width, height: this.height, top: 0, left: 0, right: this.width, bottom: this.height };
        }
    }

    // Do NOT replace globalThis.HTMLCanvasElement — dom_bootstrap already
    // exposes it as a subclass of HTMLElement ← Element ← Node ← EventTarget.
    // Instead, chain our standalone canvas class's prototype to the dom
    // HTMLCanvasElement.prototype so `standalone instanceof HTMLCanvasElement`
    // returns true.
    //
    // We DO NOT copy the standalone methods (getContext, toDataURL, ...) onto
    // the dom HTMLCanvasElement.prototype because those methods reference
    // `#canvasId`, a private field only standalone instances have. Parsed
    // <canvas> elements go through the Element.prototype.getContext patch
    // at the bottom of this file instead, which reads `_canvasId` lazily.
    if (globalThis.HTMLCanvasElement) {
        Object.setPrototypeOf(HTMLCanvasElement.prototype, globalThis.HTMLCanvasElement.prototype);
        Object.setPrototypeOf(HTMLCanvasElement, globalThis.HTMLCanvasElement);
    } else {
        globalThis.HTMLCanvasElement = HTMLCanvasElement;
    }
    globalThis.CanvasRenderingContext2D = CanvasRenderingContext2D;
    globalThis.WebGLRenderingContext = WebGLRenderingContext;
    // Symbol.toStringTag — Akamai BMP v3 and DataDome check
    // Object.prototype.toString.call(ctx) which must return
    // "[object CanvasRenderingContext2D]" / "[object WebGLRenderingContext]"
    // (not "[object Object]"). Without this tag we show as a bot.
    try {
        Object.defineProperty(CanvasRenderingContext2D.prototype, Symbol.toStringTag, {
            value: "CanvasRenderingContext2D",
            configurable: true,
        });
        Object.defineProperty(WebGLRenderingContext.prototype, Symbol.toStringTag, {
            value: "WebGLRenderingContext",
            configurable: true,
        });
        // Also expose WebGL2RenderingContext class with its own toStringTag,
        // even though our implementation uses the same backing class.
        Object.defineProperty(WebGLRenderingContext.prototype, 'constructor', {
            value: WebGLRenderingContext,
            configurable: true,
            writable: true,
        });
        Object.defineProperty(CanvasRenderingContext2D.prototype, 'constructor', {
            value: CanvasRenderingContext2D,
            configurable: true,
            writable: true,
        });
    } catch {}
    globalThis.WebGL2RenderingContext = WebGLRenderingContext;
    globalThis.AudioContext = AudioContext;
    globalThis.OfflineAudioContext = OfflineAudioContext;
    globalThis.webkitAudioContext = AudioContext;
    // Symbol.toStringTag for audio contexts — DataDome probes these.
    try {
        Object.defineProperty(AudioContext.prototype, Symbol.toStringTag, {
            value: "AudioContext", configurable: true,
        });
        Object.defineProperty(OfflineAudioContext.prototype, Symbol.toStringTag, {
            value: "OfflineAudioContext", configurable: true,
        });
    } catch {}

    // Patch document.createElement to return HTMLCanvasElement for 'canvas'
    const _origCreateElement = globalThis.document?.createElement?.bind(globalThis.document);
    if (_origCreateElement) {
        const _origFn = globalThis.document.createElement;
        globalThis.document.createElement = function(tag) {
            if (tag.toLowerCase() === "canvas") return new HTMLCanvasElement();
            return _origFn.call(this, tag);
        };
    }

    // Install canvas-specific methods on `HTMLCanvasElement.prototype`
    // directly (NOT on Element.prototype). Real Chrome's DOM uses
    // WebIDL-generated bindings where `getContext` / `toDataURL` /
    // `toBlob` are own properties of HTMLCanvasElement.prototype with
    // brand-checking that throws `TypeError: Illegal invocation` when
    // called on a non-canvas `this`. Fingerprint probes check for
    // this via `Object.getOwnPropertyDescriptor(HTMLCanvasElement
    // .prototype, 'getContext')` and by calling methods with bogus
    // `this` to observe the error message. (#55)
    const _HTMLCanvasProto = globalThis.HTMLCanvasElement &&
        globalThis.HTMLCanvasElement.prototype;
    if (_HTMLCanvasProto) {
        // Brand-check helper: Chrome throws `TypeError: Illegal
        // invocation` with no stack-relevant info beyond the message.
        //
        // We accept either `tagName === "CANVAS"` (for HTML-parsed
        // canvases whose tag name is authoritative) or
        // `this instanceof HTMLCanvasElement` (for standalone
        // canvases from createElement whose constructor sets
        // tagName after assigning width/height). This matches the
        // shape probes fingerprinters actually run while allowing
        // partially-constructed canvases to pass the setter path.
        function _requireCanvas(self, methodName) {
            const ok =
                self &&
                (self.tagName === "CANVAS" ||
                    self instanceof globalThis.HTMLCanvasElement);
            if (!ok) {
                throw new TypeError(
                    "Failed to execute '" +
                        methodName +
                        "' on 'HTMLCanvasElement': Illegal invocation"
                );
            }
        }
        function _lazyInitCanvas(self) {
            if (!self._canvasId) {
                const w = parseInt(self.getAttribute && self.getAttribute("width")) || 300;
                const h = parseInt(self.getAttribute && self.getAttribute("height")) || 150;
                self._canvasId = ops.op_canvas_create(w, h);
            }
        }

        Object.defineProperty(_HTMLCanvasProto, "getContext", {
            value: function getContext(type) {
                _requireCanvas(this, "getContext");
                _lazyInitCanvas(this);
                if (type === "2d") return new CanvasRenderingContext2D(this._canvasId);
                if (
                    type === "webgl" ||
                    type === "webgl2" ||
                    type === "experimental-webgl"
                ) {
                    const w = parseInt(this.getAttribute("width")) || 300;
                    const h = parseInt(this.getAttribute("height")) || 150;
                    const gl = new WebGLRenderingContext(this._canvasId, w, h);
                    gl.canvas = this;
                    return gl;
                }
                return null;
            },
            writable: true,
            configurable: true,
            enumerable: false,
        });

        Object.defineProperty(_HTMLCanvasProto, "toDataURL", {
            value: function toDataURL(_type) {
                _requireCanvas(this, "toDataURL");
                if (!this._canvasId) return "data:,";
                return ops.op_canvas_to_data_url(this._canvasId);
            },
            writable: true,
            configurable: true,
            enumerable: false,
        });

        Object.defineProperty(_HTMLCanvasProto, "toBlob", {
            value: function toBlob(cb, type) {
                _requireCanvas(this, "toBlob");
                if (typeof cb !== "function") {
                    throw new TypeError(
                        "Failed to execute 'toBlob' on 'HTMLCanvasElement': callback is not a function"
                    );
                }
                // Match Chrome: the callback fires asynchronously on
                // the next microtask, not synchronously.
                const url = this._canvasId ? ops.op_canvas_to_data_url(this._canvasId) : "data:,";
                queueMicrotask(() => {
                    try {
                        cb(new Blob([url], { type: type || "image/png" }));
                    } catch (_e) {}
                });
            },
            writable: true,
            configurable: true,
            enumerable: false,
        });

        // Note: `width` and `height` are deliberately NOT installed on
        // the prototype here. The standalone canvas class in this
        // bootstrap sets them as own instance properties in its
        // constructor before `tagName` is defined, so adding a
        // brand-checking prototype setter breaks construction. A
        // prototype-level width/height accessor would also collide
        // with HTML-parsed `<canvas>` elements whose `getAttribute`
        // path is already canonical. Leave them as instance props.
    }

    // OffscreenCanvas — real canvas-backed implementation.
    //
    // Replaces the minimal stub from window_bootstrap.js (which had
    // `getContext() → null`). With canvas_ext already wired in for
    // the main thread and an identical bootstrap loading in workers,
    // `new OffscreenCanvas(w, h).getContext('2d')` now returns a
    // functional CanvasRenderingContext2D backed by the same ops the
    // on-DOM `<canvas>` element uses — real fillRect, real text,
    // real toDataURL.
    //
    // Anti-fingerprint sites probe this path via
    // `const ctx = new OffscreenCanvas(w, h).getContext('2d'); ctx.fillText(...)`.
    class RealOffscreenCanvas {
        constructor(width, height) {
            this.width = width | 0;
            this.height = height | 0;
            this._canvasId = 0;
            this._context = null;
        }
        getContext(type, _opts) {
            if (type !== "2d") return null;
            if (!this._canvasId) {
                this._canvasId = ops.op_canvas_create(this.width, this.height);
            }
            if (!this._context) {
                this._context = new CanvasRenderingContext2D(this._canvasId);
                this._context.canvas = this;
            }
            return this._context;
        }
        transferToImageBitmap() {
            const self = this;
            return {
                width: self.width,
                height: self.height,
                _canvasId: self._canvasId,
                close() {},
            };
        }
        async convertToBlob(options) {
            const type = (options && options.type) || "image/png";
            if (!this._canvasId) {
                return new Blob([], { type });
            }
            // toDataURL returns `data:<type>;base64,<data>` — strip
            // the prefix and decode to bytes for a real Blob body.
            const url = ops.op_canvas_to_data_url(this._canvasId);
            const comma = url.indexOf(",");
            if (comma < 0) return new Blob([], { type });
            const b64 = url.slice(comma + 1);
            const bin = typeof atob === "function" ? atob(b64) : "";
            const bytes = new Uint8Array(bin.length);
            for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
            return new Blob([bytes], { type });
        }
    }
    Object.defineProperty(RealOffscreenCanvas.prototype, Symbol.toStringTag, {
        value: "OffscreenCanvas",
        configurable: true,
    });
    // Install as the canonical global — overwrites the window_bootstrap stub.
    globalThis.OffscreenCanvas = RealOffscreenCanvas;

    // Mask methods as native
    if (typeof _maskAsNative === 'function') {
        _maskAsNative(CanvasRenderingContext2D.prototype, 
            'fillRect', 'strokeRect', 'clearRect', 'beginPath', 'moveTo', 'lineTo',
            'fill', 'stroke', 'closePath', 'arc', 'arcTo', 'bezierCurveTo',
            'quadraticCurveTo', 'rect', 'fillText', 'strokeText', 'measureText',
            'save', 'restore', 'translate', 'rotate', 'scale', 'setTransform',
            'resetTransform', 'getTransform', 'createLinearGradient', 
            'createRadialGradient', 'createPattern', 'getImageData', 'putImageData',
            'drawImage', 'isPointInPath', 'isPointInStroke');
        
        _maskAsNative(RealOffscreenCanvas.prototype, 'getContext', 'transferToImageBitmap', 'convertToBlob');
        
        if (_HTMLCanvasProto) {
            _maskAsNative(_HTMLCanvasProto, 'getContext', 'toDataURL', 'toBlob');
        }

        if (globalThis.AudioContext) {
            _maskAsNative(AudioContext.prototype, 'createOscillator', 'createDynamicsCompressor', 'close', 'suspend', 'resume');
        }
        if (globalThis.OfflineAudioContext) {
            _maskAsNative(OfflineAudioContext.prototype, 'startRendering');
        }
        
        // Also mask WebGL if available
        if (globalThis.WebGLRenderingContext) {
            _maskAsNative(globalThis.WebGLRenderingContext.prototype, 'clear', 'clearColor', 'drawArrays', 'drawElements', 'enable', 'disable', 'getParameter');
        }
        if (globalThis.WebGL2RenderingContext) {
            _maskAsNative(globalThis.WebGL2RenderingContext.prototype, 'clear', 'clearColor', 'drawArrays', 'drawElements', 'enable', 'disable', 'getParameter');
        }
    }
})(globalThis);
