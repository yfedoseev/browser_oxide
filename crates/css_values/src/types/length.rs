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

/// A CSS math expression. Covers the full CSS Values 4 math-function set
/// that Chrome implements — including the trigonometric, exponential, and
/// step-rounding families. The variants are intentionally stored as a
/// recursive expression tree; resolution to a single numeric value
/// happens via [`CalcExpr::evaluate`].
#[derive(Debug, Clone, PartialEq)]
pub enum CalcExpr {
    Value(CalcValue),
    // Arithmetic
    Add(Box<CalcExpr>, Box<CalcExpr>),
    Sub(Box<CalcExpr>, Box<CalcExpr>),
    Mul(Box<CalcExpr>, Box<CalcExpr>),
    Div(Box<CalcExpr>, Box<CalcExpr>),
    Negate(Box<CalcExpr>),
    // Comparison
    Min(Vec<CalcExpr>),
    Max(Vec<CalcExpr>),
    Clamp {
        min: Box<CalcExpr>,
        preferred: Box<CalcExpr>,
        max: Box<CalcExpr>,
    },
    // Stepped value (CSS Values 4)
    /// `round(<rounding-strategy>?, A, B?)` — rounds A to the nearest
    /// multiple of B (or 1 if absent) using the given strategy.
    Round(RoundStrategy, Box<CalcExpr>, Box<CalcExpr>),
    /// `mod(A, B)` — modulo, sign matches B (mathematical modulo).
    Mod(Box<CalcExpr>, Box<CalcExpr>),
    /// `rem(A, B)` — remainder, sign matches A (truncating remainder).
    Rem(Box<CalcExpr>, Box<CalcExpr>),
    // Trigonometric. Inputs are angles (rad/deg/grad/turn) for
    // sin/cos/tan and unitless numbers for asin/acos/atan/atan2. Outputs
    // are unitless for sin/cos/tan and angles (radians) for the inverses.
    Sin(Box<CalcExpr>),
    Cos(Box<CalcExpr>),
    Tan(Box<CalcExpr>),
    Asin(Box<CalcExpr>),
    Acos(Box<CalcExpr>),
    Atan(Box<CalcExpr>),
    Atan2(Box<CalcExpr>, Box<CalcExpr>),
    // Exponential / power
    Pow(Box<CalcExpr>, Box<CalcExpr>),
    Sqrt(Box<CalcExpr>),
    /// `hypot(a, b, c, ...)` — Euclidean norm.
    Hypot(Vec<CalcExpr>),
    Log {
        value: Box<CalcExpr>,
        /// `log(x)` is natural log; `log(x, b)` is log base b.
        base: Option<Box<CalcExpr>>,
    },
    Exp(Box<CalcExpr>),
    // Sign-related
    Abs(Box<CalcExpr>),
    /// `sign(x)` — returns -1, 0, or 1 (preserving zero sign per spec).
    Sign(Box<CalcExpr>),
}

/// Rounding strategy for `round()`. Per CSS Values 4 §10.7.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundStrategy {
    /// `nearest` (default) — round half-to-even (banker's rounding).
    Nearest,
    /// `up` — toward +∞ (ceil).
    Up,
    /// `down` — toward −∞ (floor).
    Down,
    /// `to-zero` — toward zero (truncate).
    ToZero,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CalcValue {
    Length(f64, LengthUnit),
    Percentage(f64),
    Number(f64),
    /// An angle in its native unit. Resolved to radians during eval.
    Angle(f64, AngleUnit),
    /// A CSS-defined named numeric constant: `pi`, `e`, `infinity`,
    /// `-infinity`, `NaN`. Tokenized as identifiers inside calc().
    Constant(NumericConstant),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumericConstant {
    Pi,
    E,
    Infinity,
    NegInfinity,
    NaN,
}

impl NumericConstant {
    pub fn as_f64(self) -> f64 {
        match self {
            Self::Pi => std::f64::consts::PI,
            Self::E => std::f64::consts::E,
            Self::Infinity => f64::INFINITY,
            Self::NegInfinity => f64::NEG_INFINITY,
            Self::NaN => f64::NAN,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AngleUnit {
    Deg,
    Rad,
    Grad,
    Turn,
}

impl AngleUnit {
    /// Convert a value in this unit to radians.
    pub fn to_radians(self, v: f64) -> f64 {
        match self {
            Self::Rad => v,
            Self::Deg => v * std::f64::consts::PI / 180.0,
            Self::Grad => v * std::f64::consts::PI / 200.0,
            Self::Turn => v * std::f64::consts::TAU,
        }
    }
}

/// Resolution context for `CalcExpr::evaluate`. Provides the side-data
/// the math expression may depend on:
///   - viewport size for `vw/vh/vmin/vmax`
///   - root font size for `rem`
///   - element font size for `em/ch/ex`
///   - container size for `cqw/cqh`
///   - 100%-base for `Percentage` resolution (e.g. width vs. height)
///
/// All fields default to safe values; tests can supply only what they
/// need. Production layout passes the live context per element.
#[derive(Debug, Clone, Copy)]
pub struct CalcContext {
    pub viewport_w: f64,
    pub viewport_h: f64,
    pub root_font_size_px: f64,
    pub font_size_px: f64,
    pub container_w: f64,
    pub container_h: f64,
    /// 100% base in pixels for the property being resolved.
    pub percentage_base_px: f64,
}

impl Default for CalcContext {
    fn default() -> Self {
        Self {
            viewport_w: 1920.0,
            viewport_h: 1080.0,
            root_font_size_px: 16.0,
            font_size_px: 16.0,
            container_w: 1920.0,
            container_h: 1080.0,
            percentage_base_px: 0.0,
        }
    }
}

impl LengthUnit {
    /// Resolve one unit of this length to pixels under the given context.
    /// Returns `f64` for the multiplier — i.e. `2 * em.to_px(ctx)` is
    /// `2em` in pixels.
    pub fn to_px_factor(self, ctx: &CalcContext) -> f64 {
        match self {
            Self::Px => 1.0,
            Self::Em => ctx.font_size_px,
            Self::Rem => ctx.root_font_size_px,
            // 1ch ≈ width of '0'; 1ex ≈ x-height. We approximate as
            // 0.5em and 0.5em respectively, which matches Chrome's
            // typical fallback for fonts without OS/2 sx-Height.
            Self::Ch => ctx.font_size_px * 0.5,
            Self::Ex => ctx.font_size_px * 0.5,
            Self::Vw => ctx.viewport_w / 100.0,
            Self::Vh => ctx.viewport_h / 100.0,
            Self::Vmin => ctx.viewport_w.min(ctx.viewport_h) / 100.0,
            Self::Vmax => ctx.viewport_w.max(ctx.viewport_h) / 100.0,
            Self::Cqw => ctx.container_w / 100.0,
            Self::Cqh => ctx.container_h / 100.0,
            // Absolute units. CSS spec: 1in = 96px, 1cm ≈ 37.795px,
            // 1mm ≈ 3.7795px, 1pt = 96/72px, 1pc = 16px.
            Self::In => 96.0,
            Self::Cm => 96.0 / 2.54,
            Self::Mm => 96.0 / 25.4,
            Self::Pt => 96.0 / 72.0,
            Self::Pc => 16.0,
        }
    }
}

impl CalcExpr {
    /// Evaluate this math expression to a single `f64` value, resolved
    /// in the appropriate unit:
    ///   - lengths resolve to **pixels**
    ///   - percentages resolve via `ctx.percentage_base_px`
    ///   - numbers and constants pass through unchanged
    ///   - angles resolve to **radians** (so `sin/cos/tan` compose)
    ///
    /// Bit-exactness with Chrome: this uses `f64::sin/cos/tan/...`
    /// which on Linux/macOS x86_64 / arm64 are libm bindings — the
    /// same library Blink links (Chrome 147 macOS arm64 uses Apple's
    /// libsystem_m, x86_64 uses glibc libm). Empirical exact match
    /// confirmed for the captured Kasada probe inputs (see
    /// `crates/css_values/tests/calc_chrome_parity.rs`).
    pub fn evaluate(&self, ctx: &CalcContext) -> f64 {
        match self {
            Self::Value(v) => match v {
                CalcValue::Length(n, u) => *n * u.to_px_factor(ctx),
                CalcValue::Percentage(p) => p / 100.0 * ctx.percentage_base_px,
                CalcValue::Number(n) => *n,
                CalcValue::Angle(n, u) => u.to_radians(*n),
                CalcValue::Constant(c) => c.as_f64(),
            },
            Self::Add(a, b) => a.evaluate(ctx) + b.evaluate(ctx),
            Self::Sub(a, b) => a.evaluate(ctx) - b.evaluate(ctx),
            Self::Mul(a, b) => a.evaluate(ctx) * b.evaluate(ctx),
            Self::Div(a, b) => a.evaluate(ctx) / b.evaluate(ctx),
            Self::Negate(e) => -e.evaluate(ctx),
            Self::Min(args) => args
                .iter()
                .map(|e| e.evaluate(ctx))
                .fold(f64::INFINITY, f64::min),
            Self::Max(args) => args
                .iter()
                .map(|e| e.evaluate(ctx))
                .fold(f64::NEG_INFINITY, f64::max),
            Self::Clamp {
                min,
                preferred,
                max,
            } => {
                let lo = min.evaluate(ctx);
                let hi = max.evaluate(ctx);
                preferred.evaluate(ctx).clamp(lo, hi)
            }
            Self::Round(strategy, value, step) => {
                let v = value.evaluate(ctx);
                let s = step.evaluate(ctx);
                if s == 0.0 || !s.is_finite() {
                    return f64::NAN;
                }
                let q = v / s;
                let rounded = match strategy {
                    // f64::round_ties_even is the bank-friendly default
                    // CSS Values 4 specifies for `nearest`.
                    RoundStrategy::Nearest => q.round_ties_even(),
                    RoundStrategy::Up => q.ceil(),
                    RoundStrategy::Down => q.floor(),
                    RoundStrategy::ToZero => q.trunc(),
                };
                rounded * s
            }
            Self::Mod(a, b) => {
                // CSS mod() — sign matches divisor.
                let av = a.evaluate(ctx);
                let bv = b.evaluate(ctx);
                let r = av - (av / bv).floor() * bv;
                r
            }
            Self::Rem(a, b) => {
                // CSS rem() — sign matches dividend (Rust's `%`).
                a.evaluate(ctx) % b.evaluate(ctx)
            }
            Self::Sin(e) => e.evaluate(ctx).sin(),
            Self::Cos(e) => e.evaluate(ctx).cos(),
            Self::Tan(e) => e.evaluate(ctx).tan(),
            Self::Asin(e) => e.evaluate(ctx).asin(),
            Self::Acos(e) => e.evaluate(ctx).acos(),
            Self::Atan(e) => e.evaluate(ctx).atan(),
            Self::Atan2(y, x) => y.evaluate(ctx).atan2(x.evaluate(ctx)),
            Self::Pow(b, e) => b.evaluate(ctx).powf(e.evaluate(ctx)),
            Self::Sqrt(e) => e.evaluate(ctx).sqrt(),
            Self::Hypot(args) => {
                // Compute as sqrt(sum(xi²)). f64::hypot is 2-arg only;
                // fold via squared sum to match Chrome's blink/Length.cpp.
                let sum_sq: f64 = args
                    .iter()
                    .map(|e| {
                        let v = e.evaluate(ctx);
                        v * v
                    })
                    .sum();
                sum_sq.sqrt()
            }
            Self::Log { value, base } => {
                let v = value.evaluate(ctx);
                match base {
                    None => v.ln(),
                    Some(b) => v.log(b.evaluate(ctx)),
                }
            }
            Self::Exp(e) => e.evaluate(ctx).exp(),
            Self::Abs(e) => e.evaluate(ctx).abs(),
            Self::Sign(e) => {
                let v = e.evaluate(ctx);
                // Per CSS Values 4: sign(0) = 0, sign(-0) = -0.
                // sign(NaN) = NaN. Otherwise +1 / -1.
                if v.is_nan() {
                    f64::NAN
                } else if v == 0.0 {
                    v // preserves +0.0 / -0.0
                } else if v > 0.0 {
                    1.0
                } else {
                    -1.0
                }
            }
        }
    }
}

#[cfg(test)]
mod calc_eval_tests {
    use super::*;

    fn n(v: f64) -> Box<CalcExpr> {
        Box::new(CalcExpr::Value(CalcValue::Number(v)))
    }
    fn pi() -> Box<CalcExpr> {
        Box::new(CalcExpr::Value(CalcValue::Constant(NumericConstant::Pi)))
    }

    #[test]
    fn arithmetic_basic() {
        let ctx = CalcContext::default();
        let e = CalcExpr::Add(n(2.0), CalcExpr::Mul(n(3.0), n(4.0)).into());
        assert_eq!(e.evaluate(&ctx), 14.0);
    }

    #[test]
    fn trig_known_values() {
        let ctx = CalcContext::default();
        // sin(pi) ≈ 1.2246467991473532e-16 (f64 round-trip artifact)
        let e = CalcExpr::Sin(pi());
        assert!(e.evaluate(&ctx).abs() < 1e-15);
        // cos(0) = 1
        assert_eq!(CalcExpr::Cos(n(0.0)).evaluate(&ctx), 1.0);
        // tan(pi/4) ≈ 0.9999999999999999
        let q = CalcExpr::Div(pi(), n(4.0));
        let t = CalcExpr::Tan(q.into());
        assert!((t.evaluate(&ctx) - 1.0).abs() < 1e-15);
    }

    #[test]
    fn sqrt_pow_log_exp() {
        let ctx = CalcContext::default();
        assert_eq!(CalcExpr::Sqrt(n(16.0)).evaluate(&ctx), 4.0);
        assert_eq!(CalcExpr::Pow(n(2.0), n(10.0)).evaluate(&ctx), 1024.0);
        assert_eq!(CalcExpr::Exp(n(0.0)).evaluate(&ctx), 1.0);
        let ln_e = CalcExpr::Log {
            value: Box::new(CalcExpr::Value(CalcValue::Constant(NumericConstant::E))),
            base: None,
        };
        assert!((ln_e.evaluate(&ctx) - 1.0).abs() < 1e-15);
    }

    #[test]
    fn round_strategies() {
        let ctx = CalcContext::default();
        let mk = |s, v| CalcExpr::Round(s, n(v), n(1.0));
        assert_eq!(mk(RoundStrategy::Nearest, 1.5).evaluate(&ctx), 2.0); // ties-even rounds up
        assert_eq!(mk(RoundStrategy::Nearest, 2.5).evaluate(&ctx), 2.0); // ties-even rounds down
        assert_eq!(mk(RoundStrategy::Up, 1.1).evaluate(&ctx), 2.0);
        assert_eq!(mk(RoundStrategy::Down, 1.9).evaluate(&ctx), 1.0);
        assert_eq!(mk(RoundStrategy::ToZero, -1.9).evaluate(&ctx), -1.0);
    }

    #[test]
    fn mod_and_rem_signs() {
        let ctx = CalcContext::default();
        // mod(-1, 3) = 2 (sign matches divisor)
        assert_eq!(CalcExpr::Mod(n(-1.0), n(3.0)).evaluate(&ctx), 2.0);
        // rem(-1, 3) = -1 (sign matches dividend)
        assert_eq!(CalcExpr::Rem(n(-1.0), n(3.0)).evaluate(&ctx), -1.0);
    }

    #[test]
    fn hypot_three_args() {
        let ctx = CalcContext::default();
        let e = CalcExpr::Hypot(vec![
            CalcExpr::Value(CalcValue::Number(3.0)),
            CalcExpr::Value(CalcValue::Number(4.0)),
            CalcExpr::Value(CalcValue::Number(12.0)),
        ]);
        assert_eq!(e.evaluate(&ctx), 13.0);
    }

    #[test]
    fn length_units_resolve() {
        let mut ctx = CalcContext::default();
        ctx.font_size_px = 20.0;
        ctx.viewport_w = 1000.0;
        // 2em + 50vw = 40 + 500 = 540
        let e = CalcExpr::Add(
            Box::new(CalcExpr::Value(CalcValue::Length(2.0, LengthUnit::Em))),
            Box::new(CalcExpr::Value(CalcValue::Length(50.0, LengthUnit::Vw))),
        );
        assert_eq!(e.evaluate(&ctx), 540.0);
    }

    #[test]
    fn nested_kasada_style_expression() {
        // Simulate the kind of nested expression Kasada injects:
        //   calc(1px * (2.71828 * 0.5 + sin(pi/2)))
        //   = 1 * (1.35914 + 1.0) = 2.35914
        let ctx = CalcContext::default();
        let inner = CalcExpr::Add(
            Box::new(CalcExpr::Mul(n(2.71828), n(0.5))),
            Box::new(CalcExpr::Sin(Box::new(CalcExpr::Div(pi(), n(2.0))))),
        );
        let e = CalcExpr::Mul(
            Box::new(CalcExpr::Value(CalcValue::Length(1.0, LengthUnit::Px))),
            Box::new(inner),
        );
        let v = e.evaluate(&ctx);
        assert!((v - 2.35914).abs() < 1e-5, "got {v}");
    }
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
