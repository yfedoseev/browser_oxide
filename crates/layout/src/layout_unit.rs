//! Blink-compatible `LayoutUnit`: 32-bit fixed-point with 6 fractional bits.
//!
//! Resolution: 1/64 px = 0.015625. This matches Chromium's
//! `third_party/blink/renderer/platform/geometry/layout_unit.h` precisely.
//! Anti-bot vendors (Akamai pHash, CreepJS rect-rounding probe) hash
//! `getBoundingClientRect` floats — Chrome 147 always emits values that are
//! multiples of 1/64. A 1.3px-wide div in real Chrome returns
//! `width: 1.296875` (= round(1.3 × 64) / 64 = 83/64). Engines that emit raw
//! `f32`/`f64` pixel grids fail the hash.

const SHIFT: i32 = 6;
const ONE_PX: i32 = 1 << SHIFT; // 64
const SCALE_F64: f64 = ONE_PX as f64;

/// 32-bit fixed-point pixel value with 6 fractional bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct LayoutUnit(i32);

impl LayoutUnit {
    pub const ZERO: LayoutUnit = LayoutUnit(0);

    /// Construct from a CSS-pixel `f64`, snapping to the nearest 1/64 px.
    /// Matches Blink's `LayoutUnit::FromFloatRound`.
    pub fn from_f64_px(v: f64) -> Self {
        if !v.is_finite() {
            return LayoutUnit::ZERO;
        }
        let scaled = (v * SCALE_F64).round();
        // Saturating to i32 range to mirror Blink's behaviour on overflow.
        let clamped = scaled.clamp(i32::MIN as f64, i32::MAX as f64) as i32;
        LayoutUnit(clamped)
    }

    /// Construct from a Taffy `f32` layout coordinate, snapping to 1/64 px.
    pub fn from_taffy_f32(v: f32) -> Self {
        Self::from_f64_px(v as f64)
    }

    /// Convert back to CSS pixels as `f64`. The result is always a multiple
    /// of 1/64.
    pub fn to_f64_px(self) -> f64 {
        (self.0 as f64) / SCALE_F64
    }

    /// Raw fixed-point value (for arithmetic without round-trip loss).
    pub fn raw(self) -> i32 {
        self.0
    }
}

impl std::ops::Add for LayoutUnit {
    type Output = LayoutUnit;
    fn add(self, rhs: Self) -> Self::Output {
        LayoutUnit(self.0.saturating_add(rhs.0))
    }
}

impl std::ops::Sub for LayoutUnit {
    type Output = LayoutUnit;
    fn sub(self, rhs: Self) -> Self::Output {
        LayoutUnit(self.0.saturating_sub(rhs.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantizes_to_64ths() {
        // Chrome 147: width:1.3px div returns 1.296875 (= 83/64).
        let v = LayoutUnit::from_f64_px(1.3);
        assert_eq!(v.to_f64_px(), 1.296875);
        assert_eq!(v.raw(), 83);
    }

    #[test]
    fn integer_pixels_round_trip_exactly() {
        let v = LayoutUnit::from_f64_px(100.0);
        assert_eq!(v.to_f64_px(), 100.0);
        assert_eq!(v.raw(), 100 * 64);
    }

    #[test]
    fn half_pixel_quantizes_exact() {
        // 0.5 = 32/64
        let v = LayoutUnit::from_f64_px(0.5);
        assert_eq!(v.to_f64_px(), 0.5);
        assert_eq!(v.raw(), 32);
    }

    #[test]
    fn nan_becomes_zero() {
        let v = LayoutUnit::from_f64_px(f64::NAN);
        assert_eq!(v.raw(), 0);
        assert_eq!(v.to_f64_px(), 0.0);
    }

    #[test]
    fn arithmetic_preserves_quantization() {
        let a = LayoutUnit::from_f64_px(1.3);
        let b = LayoutUnit::from_f64_px(0.7);
        let c = a + b;
        // 1.296875 + 0.703125 = 2.0 (exact, since both are quantized)
        assert_eq!(c.to_f64_px(), 2.0);
    }

    #[test]
    fn from_taffy_f32_quantizes() {
        // Taffy returns f32; a 1.3 f32 maps to ~1.2999999523f64 → quantized 83/64
        let v = LayoutUnit::from_taffy_f32(1.3);
        assert_eq!(v.to_f64_px(), 1.296875);
    }
}
