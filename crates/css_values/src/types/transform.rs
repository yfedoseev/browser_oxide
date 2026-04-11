use super::length::{Angle, Length, LengthPercentage};

/// CSS `transform` function values.
#[derive(Debug, Clone, PartialEq)]
pub enum TransformFunction {
    Translate(LengthPercentage, LengthPercentage),
    TranslateX(LengthPercentage),
    TranslateY(LengthPercentage),
    Scale(f64, f64),
    ScaleX(f64),
    ScaleY(f64),
    Rotate(Angle),
    SkewX(Angle),
    SkewY(Angle),
    Matrix(f64, f64, f64, f64, f64, f64),
    Matrix3d([f64; 16]),
    Translate3d(LengthPercentage, LengthPercentage, Length),
    Scale3d(f64, f64, f64),
    Rotate3d(f64, f64, f64, Angle),
    Perspective(Length),
    None,
}
