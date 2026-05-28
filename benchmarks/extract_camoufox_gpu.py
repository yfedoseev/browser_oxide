#!/usr/bin/env python3
"""Extract one Camoufox `webgl_data.db` row and emit Rust GpuProfile literal.

vNext/07 §FIX-D3 helper: produces the per-OS GpuProfile constants for
`chrome_148_windows` (NVIDIA D3D11) and `chrome_148_linux` (Intel UHD)
by transforming Camoufox's captured-real-Chrome SQLite into the Rust
shape `crates/stealth/src/gpu.rs::GpuProfile` expects.

Usage:
    extract_camoufox_gpu.py --vendor 'Google Inc. (NVIDIA)' \\
        --renderer 'GTX 980' --surface webgl1
    extract_camoufox_gpu.py --vendor 'Intel' --renderer 'HD Graphics' \\
        --surface webgl1 --target-os lin

Output: a Rust `GpuProfile { ... }` literal printed to stdout. Paste
into `crates/stealth/src/gpu.rs` (after manual review of renderer-string
substitution — the captured row's renderer may need to be retargeted
to the desired GPU shape, e.g. GTX 980 → RTX 3060).

The Camoufox DB is at `crates/stealth/fixtures/camoufox_webgl/webgl_data.db`.
Vendored under MPL-2.0 with attribution; see `LICENSE.camoufox`.
"""
from __future__ import annotations

import argparse
import json
import sqlite3
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_DB = REPO_ROOT / "crates/stealth/fixtures/camoufox_webgl/webgl_data.db"


def rust_string(s: str) -> str:
    """Escape `s` for use in a Rust `"..."` literal."""
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


def rust_value(v) -> str:
    """Emit a serde_json::Value literal from a Python JSON value.
    Camoufox encodes ints as `int`, floats as `float`, bools as `bool`,
    arrays as `list`, etc. We emit `serde_json::json!(...)` for
    structural values and integer/float literals for scalars."""
    if isinstance(v, bool):
        return f"serde_json::json!({str(v).lower()})"
    if isinstance(v, int):
        return f"serde_json::json!({v})"
    if isinstance(v, float):
        return f"serde_json::json!({v})"
    if isinstance(v, str):
        return f"serde_json::json!({rust_string(v)})"
    if isinstance(v, list):
        return f"serde_json::json!({json.dumps(v)})"
    if isinstance(v, dict):
        return f"serde_json::json!({json.dumps(v)})"
    if v is None:
        return "serde_json::json!(null)"
    raise ValueError(f"unsupported value: {v!r}")


def emit_profile(data: dict, surface: str, fn_name: str, override_renderer: str | None) -> str:
    """Build a Rust `GpuProfile { ... }` literal for the given surface
    (`webgl1` or `webgl2`)."""
    prefix = "webGl:" if surface == "webgl1" else "webGl2:"
    renderer = override_renderer or data[f"{prefix}renderer"]
    vendor = data[f"{prefix}vendor"]
    version = (
        "WebGL 1.0 (OpenGL ES 2.0 Chromium)"
        if surface == "webgl1"
        else "WebGL 2.0 (OpenGL ES 3.0 Chromium)"
    )
    slv = (
        "WebGL GLSL ES 1.0 (OpenGL ES GLSL ES 1.0 Chromium)"
        if surface == "webgl1"
        else "WebGL GLSL ES 3.00 (OpenGL ES GLSL ES 3.0 Chromium)"
    )
    extensions = data[f"{prefix}supportedExtensions"]
    params = data[f"{prefix}parameters"]
    shader_precision = data[f"{prefix}shaderPrecisionFormats"]

    out: list[str] = []
    out.append("/// Extracted from Camoufox's `webgl_data.db` (vendored under MPL-2.0).")
    out.append(f"/// Source row: vendor={rust_string(vendor)} renderer={rust_string(data[f'{prefix}renderer'])}")
    out.append(f"/// WebGL surface: {surface}")
    out.append(f"pub fn {fn_name}() -> GpuProfile {{")
    out.append("    GpuProfile {")
    out.append('        vendor: "WebKit".into(),')
    out.append('        renderer: "WebKit WebGL".into(),')
    out.append(f"        version: {rust_string(version)}.into(),")
    out.append(f"        shading_language_version: {rust_string(slv)}.into(),")
    out.append(f"        unmasked_vendor: {rust_string(vendor)}.into(),")
    out.append(f"        unmasked_renderer: {rust_string(renderer)}.into(),")
    out.append("        extensions: vec![")
    for ext in extensions:
        out.append(f"            {rust_string(ext)}.into(),")
    out.append("        ],")
    out.append("        params: vec![")
    # Sort keys numerically (they are GL enum integers in string form).
    # If --renderer-override is set, substitute params[37446] (UNMASKED_
    # RENDERER_WEBGL = 0x9246) with the override so the param-read path
    # and the unmasked_renderer field don't diverge.
    for k in sorted(params.keys(), key=lambda x: int(x)):
        v = params[k]
        if override_renderer is not None and k in ("37446", "9246"):
            v = override_renderer
        out.append(f"            ({k}u32, {rust_value(v)}),")
    out.append("        ],")
    out.append("        shader_precision: vec![")
    # Camoufox shaderPrecisionFormats keys are decimal GL enum pairs
    # like "35633,36336" (VERTEX_SHADER=0x8B31=35633, HIGH_FLOAT=0x8DF2=36338).
    # Values are dicts with rangeMin / rangeMax / precision.
    for k, v in shader_precision.items():
        try:
            st_str, pt_str = k.split(",")
            st = int(st_str)
            pt = int(pt_str)
        except ValueError:
            sys.stderr.write(
                f"warn: unrecognized shaderPrecisionFormats key {k!r}; skipping\n"
            )
            continue
        if isinstance(v, dict):
            rng = [int(v["rangeMin"]), int(v["rangeMax"]), int(v["precision"])]
        elif isinstance(v, list):
            rng = [int(v[0]), int(v[1]), int(v[2])]
        else:
            sys.stderr.write(
                f"warn: unrecognized shaderPrecisionFormats value {v!r}; skipping\n"
            )
            continue
        out.append(
            f"            ({st:#x}u32, {pt:#x}u32, [{rng[0]}, {rng[1]}, {rng[2]}]),"
        )
    out.append("        ],")
    out.append("    }")
    out.append("}")
    return "\n".join(out) + "\n"


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("--db", type=Path, default=DEFAULT_DB)
    p.add_argument("--vendor", required=True, help="substring of `vendor` to match")
    p.add_argument("--renderer", required=True, help="substring of `renderer` to match")
    p.add_argument("--target-os", choices=["win", "mac", "lin"], default=None)
    p.add_argument(
        "--surface",
        choices=["webgl1", "webgl2"],
        default="webgl1",
        help="emit WebGL 1 or WebGL 2 surface",
    )
    p.add_argument(
        "--fn-name",
        default="profile_from_camoufox",
        help="Rust function name to emit",
    )
    p.add_argument(
        "--renderer-override",
        default=None,
        help="substitute the unmasked_renderer string (e.g. GTX 980 → RTX 3060)",
    )
    p.add_argument(
        "--list",
        action="store_true",
        help="list matching rows and exit; do not emit Rust",
    )
    args = p.parse_args()
    conn = sqlite3.connect(args.db)
    sql = (
        "SELECT vendor, renderer, win, mac, lin, data "
        "FROM webgl_fingerprints WHERE vendor LIKE ? AND renderer LIKE ?"
    )
    rows = conn.execute(sql, (f"%{args.vendor}%", f"%{args.renderer}%")).fetchall()
    if args.target_os:
        col = {"win": 2, "mac": 3, "lin": 4}[args.target_os]
        rows = [r for r in rows if r[col] > 0]
    if not rows:
        sys.stderr.write("no matching rows\n")
        sys.exit(1)
    if args.list:
        for r in rows:
            print(f"  vendor={r[0]!r}  renderer={r[1]!r}  win={r[2]} mac={r[3]} lin={r[4]}")
        return
    # Pick the highest-weighted row for the target OS (or first match)
    if args.target_os:
        col = {"win": 2, "mac": 3, "lin": 4}[args.target_os]
        rows.sort(key=lambda r: r[col], reverse=True)
    row = rows[0]
    sys.stderr.write(
        f"[extract] picked vendor={row[0]!r} renderer={row[1]!r} "
        f"win={row[2]} mac={row[3]} lin={row[4]}\n"
    )
    data = json.loads(row[5])
    print(emit_profile(data, args.surface, args.fn_name, args.renderer_override))


if __name__ == "__main__":
    main()
