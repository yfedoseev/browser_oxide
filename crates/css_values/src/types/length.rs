/// A CSS length value.
#[derive(Debug, Clone, PartialEq)]
pub enum Length {
    Px(f64),
    Em(f64),
    Rem(f64),
    Vw(f64),
    Vh(f64),
    Vmin(f64),
    Vmax(f64),
    Cqw(f64),
    Cqh(f64),
    Cm(f64),
    Mm(f64),
    In(f64),
    Pt(f64),
    Pc(f64),
    Ch(f64),
    Ex(f64),
    Calc(Box<CalcExpr>),
    Zero,
}

/// A length or percentage.
#[derive(Debug, Clone, PartialEq)]
pub enum LengthPercentage {
    Length(Length),
    Percentage(f64),
    Calc(Box<CalcExpr>),
}

/// A length, percentage, or `auto`.
#[derive(Debug, Clone, PartialEq)]
pub enum LengthPercentageAuto {
    Length(Length),
    Percentage(f64),
    Auto,
    Calc(Box<CalcExpr>),
}

/// A CSS math expression: `calc()`, `min()`, `max()`, `clamp()`.
#[derive(Debug, Clone, PartialEq)]
pub enum CalcExpr {
    Value(CalcValue),
    Add(Box<CalcExpr>, Box<CalcExpr>),
    Sub(Box<CalcExpr>, Box<CalcExpr>),
    Mul(Box<CalcExpr>, Box<CalcExpr>),
    Div(Box<CalcExpr>, Box<CalcExpr>),
    Negate(Box<CalcExpr>),
    Min(Vec<CalcExpr>),
    Max(Vec<CalcExpr>),
    Clamp {
        min: Box<CalcExpr>,
        preferred: Box<CalcExpr>,
        max: Box<CalcExpr>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum CalcValue {
    Length(f64, LengthUnit),
    Percentage(f64),
    Number(f64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LengthUnit {
    Px,
    Em,
    Rem,
    Vw,
    Vh,
    Vmin,
    Vmax,
    Cm,
    Mm,
    In,
    Pt,
    Pc,
    Ch,
    Ex,
    Cqw,
    Cqh,
}

/// An angle value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Angle {
    Deg(f64),
    Rad(f64),
    Grad(f64),
    Turn(f64),
}

impl Angle {
    pub fn to_degrees(&self) -> f64 {
        match self {
            Angle::Deg(d) => *d,
            Angle::Rad(r) => r * 180.0 / std::f64::consts::PI,
            Angle::Grad(g) => g * 0.9,
            Angle::Turn(t) => t * 360.0,
        }
    }
}
