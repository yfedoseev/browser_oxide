//! PNG byte-determinism gate. With `flate2/zlib-rs` selected and the
//! filter strategy pinned (Paeth + Adaptive), the same canvas must
//! encode to byte-identical PNGs across runs and across platforms.
//!
//! This is the prerequisite for canvas-fingerprint stability: anti-bot
//! probes (CreepJS lies-canvas, Akamai pHash) hash the data URL, so
//! engine PNG output that drifts between runs breaks fingerprint
//! reproducibility — which itself is a bot-detection signal.
//!
//! Full Chrome byte parity (engine SHA matches a real Chrome run) is
//! NOT what we assert here — that requires text rasterization to match
//! Chrome's Skia (rustybuzz+swash vs Skia produce different alpha
//! masks for the same glyph). We assert ENGINE determinism, which is
//! the smaller half of the problem and the part we can verify without
//! needing Chrome's byte output.

use canvas::Canvas2D;
use sha2::{Digest, Sha256};

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{:02x}", b)).collect()
}

fn fixture_canvas() -> Canvas2D {
    // Fixed scene that exercises fillRect (no text, no glyphs — those
    // depend on font rasterization which is independently variable).
    let mut c = Canvas2D::new(100, 30, "Linux".to_string()).expect("canvas 100x30");
    c.set_fill_color(255, 100, 0, 1.0);
    c.fill_rect(10.0, 5.0, 80.0, 20.0);
    c.set_fill_color(0, 102, 153, 0.7);
    c.fill_rect(20.0, 10.0, 60.0, 10.0);
    c
}

/// Encoding the same scene twice must produce byte-identical output.
/// Without zlib-rs (e.g. with miniz_oxide) this passes — both runs
/// hit the same algorithm. With miniz_oxide we'd still be deterministic
/// within a single binary build, so this check is a regression gate
/// for "did someone introduce nondeterminism" rather than a backend
/// validation.
#[test]
fn png_encoding_is_byte_deterministic() {
    let bytes_a = fixture_canvas().to_png_bytes();
    let bytes_b = fixture_canvas().to_png_bytes();
    assert_eq!(
        bytes_a, bytes_b,
        "PNG encoding must be byte-deterministic across calls"
    );
}

/// Pin the SHA-256 of the fixture canvas's PNG output. If anyone
/// changes the encoder settings (filter strategy, compression level,
/// flate2 backend) the hash will change loudly — preventing silent
/// drift of canvas fingerprint output.
///
/// On hash mismatch: investigate which setting drifted (zlib-rs vs
/// miniz_oxide via `cargo tree`, or png crate version, or filter
/// strategy in canvas2d.rs::to_png_bytes). If the change is
/// intentional, update this hash.
#[test]
fn png_fixture_hash_is_pinned() {
    let bytes = fixture_canvas().to_png_bytes();
    let hex = sha256_hex(&bytes);
    eprintln!("PNG fixture SHA-256: {hex}");
    eprintln!("PNG fixture byte length: {}", bytes.len());
    // PINNED HASH — update only when the encoder settings change
    // intentionally.
    assert_eq!(
        hex, "b47ef99603dc7e65e116cafad6011b994d3a3a7f2b292df212f1979dd4b1a579",
        "PNG byte output drifted; check flate2 backend, png crate version, or filter strategy"
    );
    assert_eq!(bytes.len(), 197, "PNG fixture byte length must be stable");
}
