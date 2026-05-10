//! CSS `calc()` parser — converts a `CssFunction` from `css_parser` into
//! a [`CalcExpr`] tree that [`CalcExpr::evaluate`] can resolve.
//!
//! Implements the full CSS Values 4 math function set Chrome 147 ships:
//! - arithmetic: `+`, `-`, `*`, `/`
//! - comparison: `min(...)`, `max(...)`, `clamp(min, val, max)`
//! - stepped: `round([strategy,] a [, b])`, `mod(a, b)`, `rem(a, b)`
//! - trigonometric: `sin/cos/tan/asin/acos/atan/atan2`
//! - exponential: `pow(b, e)`, `sqrt(x)`, `hypot(...)`, `log(x [, b])`, `exp(x)`
//! - sign: `abs(x)`, `sign(x)`
//! - constants: `pi`, `e`, `infinity`, `-infinity`, `NaN`
//!
//! See `crates/css_values/src/types/length.rs` for the AST + evaluator.
//!
//! Why this exists: Kasada (and many other antibot stacks) inject
//! deeply-nested calc() expressions with sin/cos/tan/sqrt/pi as a
//! browser-fingerprint precision probe — they evaluate the result via
//! `getComputedStyle` and compare against expected Chrome f64 output.
//! Engines that don't implement these functions return `auto` or wrong
//! values and get caught.

use crate::types::length::{
    AngleUnit, CalcExpr, CalcValue, LengthUnit, NumericConstant, RoundStrategy,
};
use css_parser::ast::{ComponentValue, CssFunction};
use css_parser::token::{Token, TokenKind};

#[derive(Debug)]
pub enum CalcParseError {
    Empty,
    UnexpectedToken(String),
    UnknownFunction(String),
    WrongArity { name: String, expected: &'static str, got: usize },
    InvalidUnit(String),
}

impl std::fmt::Display for CalcParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "empty calc() arguments"),
            Self::UnexpectedToken(s) => write!(f, "unexpected token: {s}"),
            Self::UnknownFunction(s) => write!(f, "unknown math function: {s}"),
            Self::WrongArity { name, expected, got } => {
                write!(f, "{name}() arity: expected {expected}, got {got}")
            }
            Self::InvalidUnit(s) => write!(f, "unknown unit: {s}"),
        }
    }
}

impl std::error::Error for CalcParseError {}

/// Parse a top-level math function call (`calc`, `min`, `max`, `clamp`,
/// or any other CSS Values 4 math function name) into a [`CalcExpr`].
/// Returns `Ok(None)` if the function name is not a math function — the
/// caller should fall through to its existing parse path for `var()`,
/// `env()`, gradient functions, etc.
pub fn parse_math_function(f: &CssFunction<'_>) -> Result<Option<CalcExpr>, CalcParseError> {
    let name = f.name.to_ascii_lowercase();
    match name.as_str() {
        "calc" => Ok(Some(parse_sum(&filter_ws(&f.arguments))?)),
        "min" | "max" | "hypot" => {
            let parts = split_top_level_commas(&f.arguments);
            if parts.is_empty() {
                return Err(CalcParseError::Empty);
            }
            let exprs = parts
                .into_iter()
                .map(|p| parse_sum(&filter_ws(p)))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Some(match name.as_str() {
                "min" => CalcExpr::Min(exprs),
                "max" => CalcExpr::Max(exprs),
                "hypot" => CalcExpr::Hypot(exprs),
                _ => unreachable!(),
            }))
        }
        "clamp" => {
            let parts = split_top_level_commas(&f.arguments);
            if parts.len() != 3 {
                return Err(CalcParseError::WrongArity {
                    name: "clamp".into(),
                    expected: "3 (min, val, max)",
                    got: parts.len(),
                });
            }
            Ok(Some(CalcExpr::Clamp {
                min: Box::new(parse_sum(&filter_ws(parts[0]))?),
                preferred: Box::new(parse_sum(&filter_ws(parts[1]))?),
                max: Box::new(parse_sum(&filter_ws(parts[2]))?),
            }))
        }
        "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "sqrt" | "exp" | "abs" | "sign" => {
            let parts = split_top_level_commas(&f.arguments);
            if parts.len() != 1 {
                return Err(CalcParseError::WrongArity {
                    name,
                    expected: "1",
                    got: parts.len(),
                });
            }
            let inner = Box::new(parse_sum(&filter_ws(parts[0]))?);
            Ok(Some(match name.as_str() {
                "sin" => CalcExpr::Sin(inner),
                "cos" => CalcExpr::Cos(inner),
                "tan" => CalcExpr::Tan(inner),
                "asin" => CalcExpr::Asin(inner),
                "acos" => CalcExpr::Acos(inner),
                "atan" => CalcExpr::Atan(inner),
                "sqrt" => CalcExpr::Sqrt(inner),
                "exp" => CalcExpr::Exp(inner),
                "abs" => CalcExpr::Abs(inner),
                "sign" => CalcExpr::Sign(inner),
                _ => unreachable!(),
            }))
        }
        "atan2" | "pow" | "mod" | "rem" => {
            let parts = split_top_level_commas(&f.arguments);
            if parts.len() != 2 {
                return Err(CalcParseError::WrongArity {
                    name,
                    expected: "2",
                    got: parts.len(),
                });
            }
            let a = Box::new(parse_sum(&filter_ws(parts[0]))?);
            let b = Box::new(parse_sum(&filter_ws(parts[1]))?);
            Ok(Some(match name.as_str() {
                "atan2" => CalcExpr::Atan2(a, b),
                "pow" => CalcExpr::Pow(a, b),
                "mod" => CalcExpr::Mod(a, b),
                "rem" => CalcExpr::Rem(a, b),
                _ => unreachable!(),
            }))
        }
        "log" => {
            let parts = split_top_level_commas(&f.arguments);
            match parts.len() {
                1 => Ok(Some(CalcExpr::Log {
                    value: Box::new(parse_sum(&filter_ws(parts[0]))?),
                    base: None,
                })),
                2 => Ok(Some(CalcExpr::Log {
                    value: Box::new(parse_sum(&filter_ws(parts[0]))?),
                    base: Some(Box::new(parse_sum(&filter_ws(parts[1]))?)),
                })),
                got => Err(CalcParseError::WrongArity {
                    name: "log".into(),
                    expected: "1 or 2",
                    got,
                }),
            }
        }
        "round" => {
            // round([strategy ,] A [, B])
            let parts = split_top_level_commas(&f.arguments);
            let (strategy, value_idx) = if let Some(first) = parts.first() {
                let toks = filter_ws(first);
                if toks.len() == 1 {
                    if let ComponentValue::Token(Token { kind: TokenKind::Ident(id), .. }) = &toks[0] {
                        match id.to_ascii_lowercase().as_str() {
                            "nearest" => (RoundStrategy::Nearest, 1),
                            "up" => (RoundStrategy::Up, 1),
                            "down" => (RoundStrategy::Down, 1),
                            "to-zero" => (RoundStrategy::ToZero, 1),
                            _ => (RoundStrategy::Nearest, 0),
                        }
                    } else {
                        (RoundStrategy::Nearest, 0)
                    }
                } else {
                    (RoundStrategy::Nearest, 0)
                }
            } else {
                return Err(CalcParseError::Empty);
            };
            let value = Box::new(parse_sum(&filter_ws(parts[value_idx]))?);
            let step: Box<CalcExpr> = if parts.len() > value_idx + 1 {
                Box::new(parse_sum(&filter_ws(parts[value_idx + 1]))?)
            } else {
                Box::new(CalcExpr::Value(CalcValue::Number(1.0)))
            };
            Ok(Some(CalcExpr::Round(strategy, value, step)))
        }
        // Not a math function — caller may handle (var(), env(), etc.).
        _ => Ok(None),
    }
}

// =====================================================================
// Recursive-descent precedence parser for calc()'s grammar:
//   sum     := product (('+'|'-') product)*
//   product := unary (('*'|'/') unary)*
//   unary   := '-' unary | atom
//   atom    := number | dimension | percentage | constant-ident
//            | '(' sum ')' | math-function
// CSS Values 4 requires whitespace around '+'/'-' (already handled by the
// tokenizer producing separate Whitespace + Delim tokens). We expect the
// caller to have already stripped Whitespace via `filter_ws` so the
// grammar can match on Delim positions cleanly.
// =====================================================================

fn filter_ws<'a>(tokens: &'a [ComponentValue<'a>]) -> Vec<&'a ComponentValue<'a>> {
    tokens
        .iter()
        .filter(|cv| {
            !matches!(
                cv,
                ComponentValue::Token(Token {
                    kind: TokenKind::Whitespace,
                    ..
                })
            )
        })
        .collect()
}

fn split_top_level_commas<'a>(
    tokens: &'a [ComponentValue<'a>],
) -> Vec<&'a [ComponentValue<'a>]> {
    let mut out = Vec::new();
    let mut start = 0usize;
    for (i, cv) in tokens.iter().enumerate() {
        if matches!(
            cv,
            ComponentValue::Token(Token {
                kind: TokenKind::Comma,
                ..
            })
        ) {
            out.push(&tokens[start..i]);
            start = i + 1;
        }
    }
    if start <= tokens.len() {
        let tail = &tokens[start..];
        // Skip a trailing-only-whitespace tail (no real argument).
        if !tail.iter().all(|cv| matches!(cv, ComponentValue::Token(Token { kind: TokenKind::Whitespace, .. }))) {
            out.push(tail);
        }
    }
    out
}

fn parse_sum<'a>(tokens: &[&'a ComponentValue<'a>]) -> Result<CalcExpr, CalcParseError> {
    if tokens.is_empty() {
        return Err(CalcParseError::Empty);
    }
    let mut pos = 0usize;
    let mut left = parse_product(tokens, &mut pos)?;
    while pos < tokens.len() {
        match tokens[pos] {
            ComponentValue::Token(Token { kind: TokenKind::Delim('+'), .. }) => {
                pos += 1;
                let right = parse_product(tokens, &mut pos)?;
                left = CalcExpr::Add(Box::new(left), Box::new(right));
            }
            ComponentValue::Token(Token { kind: TokenKind::Delim('-'), .. }) => {
                pos += 1;
                let right = parse_product(tokens, &mut pos)?;
                left = CalcExpr::Sub(Box::new(left), Box::new(right));
            }
            other => {
                return Err(CalcParseError::UnexpectedToken(format!("{:?}", other)));
            }
        }
    }
    Ok(left)
}

fn parse_product<'a>(
    tokens: &[&'a ComponentValue<'a>],
    pos: &mut usize,
) -> Result<CalcExpr, CalcParseError> {
    let mut left = parse_unary(tokens, pos)?;
    while *pos < tokens.len() {
        match tokens[*pos] {
            ComponentValue::Token(Token { kind: TokenKind::Delim('*'), .. }) => {
                *pos += 1;
                let right = parse_unary(tokens, pos)?;
                left = CalcExpr::Mul(Box::new(left), Box::new(right));
            }
            ComponentValue::Token(Token { kind: TokenKind::Delim('/'), .. }) => {
                *pos += 1;
                let right = parse_unary(tokens, pos)?;
                left = CalcExpr::Div(Box::new(left), Box::new(right));
            }
            _ => break,
        }
    }
    Ok(left)
}

fn parse_unary<'a>(
    tokens: &[&'a ComponentValue<'a>],
    pos: &mut usize,
) -> Result<CalcExpr, CalcParseError> {
    if *pos >= tokens.len() {
        return Err(CalcParseError::Empty);
    }
    if let ComponentValue::Token(Token { kind: TokenKind::Delim('-'), .. }) = tokens[*pos] {
        *pos += 1;
        let inner = parse_unary(tokens, pos)?;
        return Ok(CalcExpr::Negate(Box::new(inner)));
    }
    if let ComponentValue::Token(Token { kind: TokenKind::Delim('+'), .. }) = tokens[*pos] {
        *pos += 1;
        return parse_unary(tokens, pos);
    }
    parse_atom(tokens, pos)
}

fn parse_atom<'a>(
    tokens: &[&'a ComponentValue<'a>],
    pos: &mut usize,
) -> Result<CalcExpr, CalcParseError> {
    let cv = tokens[*pos];
    *pos += 1;
    match cv {
        ComponentValue::Token(t) => match &t.kind {
            TokenKind::Number { value, .. } => Ok(CalcExpr::Value(CalcValue::Number(*value))),
            TokenKind::Percentage { value, .. } => {
                Ok(CalcExpr::Value(CalcValue::Percentage(*value)))
            }
            TokenKind::Dimension { value, unit, .. } => {
                if let Some(u) = parse_length_unit(unit) {
                    Ok(CalcExpr::Value(CalcValue::Length(*value, u)))
                } else if let Some(u) = parse_angle_unit(unit) {
                    Ok(CalcExpr::Value(CalcValue::Angle(*value, u)))
                } else {
                    Err(CalcParseError::InvalidUnit((*unit).to_string()))
                }
            }
            TokenKind::Ident(id) => match id.to_ascii_lowercase().as_str() {
                "pi" => Ok(CalcExpr::Value(CalcValue::Constant(NumericConstant::Pi))),
                "e" => Ok(CalcExpr::Value(CalcValue::Constant(NumericConstant::E))),
                "infinity" => Ok(CalcExpr::Value(CalcValue::Constant(NumericConstant::Infinity))),
                "-infinity" => Ok(CalcExpr::Value(CalcValue::Constant(
                    NumericConstant::NegInfinity,
                ))),
                "nan" => Ok(CalcExpr::Value(CalcValue::Constant(NumericConstant::NaN))),
                other => Err(CalcParseError::UnexpectedToken(format!("ident `{other}`"))),
            },
            other => Err(CalcParseError::UnexpectedToken(format!("{:?}", other))),
        },
        ComponentValue::SimpleBlock(b) if b.token == '(' => {
            // Parenthesized sub-expression. Recurse on the inner tokens.
            let inner = filter_ws(&b.value);
            parse_sum(&inner)
        }
        ComponentValue::Function(inner_fn) => {
            // Nested math-function call (sin/cos/etc. inside calc()).
            match parse_math_function(inner_fn)? {
                Some(expr) => Ok(expr),
                None => Err(CalcParseError::UnknownFunction(inner_fn.name.to_string())),
            }
        }
        other => Err(CalcParseError::UnexpectedToken(format!("{:?}", other))),
    }
}

fn parse_length_unit(unit: &str) -> Option<LengthUnit> {
    Some(match unit.to_ascii_lowercase().as_str() {
        "px" => LengthUnit::Px,
        "em" => LengthUnit::Em,
        "rem" => LengthUnit::Rem,
        "vw" => LengthUnit::Vw,
        "vh" => LengthUnit::Vh,
        "vmin" => LengthUnit::Vmin,
        "vmax" => LengthUnit::Vmax,
        "cm" => LengthUnit::Cm,
        "mm" => LengthUnit::Mm,
        "in" => LengthUnit::In,
        "pt" => LengthUnit::Pt,
        "pc" => LengthUnit::Pc,
        "ch" => LengthUnit::Ch,
        "ex" => LengthUnit::Ex,
        "cqw" => LengthUnit::Cqw,
        "cqh" => LengthUnit::Cqh,
        _ => return None,
    })
}

fn parse_angle_unit(unit: &str) -> Option<AngleUnit> {
    Some(match unit.to_ascii_lowercase().as_str() {
        "deg" => AngleUnit::Deg,
        "rad" => AngleUnit::Rad,
        "grad" => AngleUnit::Grad,
        "turn" => AngleUnit::Turn,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::length::CalcContext;

    /// Parse a math expression source (e.g. `"calc(1px + 2px)"`) into a
    /// `CalcExpr` for testing. Wraps it in a synthetic declaration list
    /// so the css_parser entry point accepts it.
    fn parse_calc(src: &str) -> CalcExpr {
        let css = format!("width: {src};");
        let (decls, _errs) = css_parser::parse_declaration_list(&css);
        let decl = decls.first().expect("at least one decl parsed");
        for c in &decl.value {
            if let ComponentValue::Function(f) = c {
                let parsed = parse_math_function(f).expect("parse ok");
                return parsed.expect("math fn recognised");
            }
        }
        panic!("no function in {src}");
    }

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} ≉ {b}");
    }

    #[test]
    fn calc_basic_arithmetic() {
        let ctx = CalcContext::default();
        approx(parse_calc("calc(1 + 2 * 3)").evaluate(&ctx), 7.0);
        approx(parse_calc("calc((1 + 2) * 3)").evaluate(&ctx), 9.0);
        approx(parse_calc("calc(10 - 4 - 1)").evaluate(&ctx), 5.0);
        approx(parse_calc("calc(20 / 4 / 5)").evaluate(&ctx), 1.0);
    }

    #[test]
    fn calc_with_lengths() {
        let ctx = CalcContext::default();
        approx(parse_calc("calc(10px + 5px)").evaluate(&ctx), 15.0);
        approx(parse_calc("calc(2 * 8px)").evaluate(&ctx), 16.0);
    }

    #[test]
    fn min_max_clamp() {
        let ctx = CalcContext::default();
        approx(parse_calc("min(10, 5, 7)").evaluate(&ctx), 5.0);
        approx(parse_calc("max(10, 5, 7)").evaluate(&ctx), 10.0);
        approx(parse_calc("clamp(0, 99, 10)").evaluate(&ctx), 10.0);
        approx(parse_calc("clamp(0, 5, 10)").evaluate(&ctx), 5.0);
    }

    #[test]
    fn trig_functions() {
        let ctx = CalcContext::default();
        approx(parse_calc("cos(0)").evaluate(&ctx), 1.0);
        approx(parse_calc("sin(0)").evaluate(&ctx), 0.0);
        approx(parse_calc("tan(0)").evaluate(&ctx), 0.0);
        approx(parse_calc("sin(pi)").evaluate(&ctx), 0.0); // within tolerance
        approx(parse_calc("cos(pi)").evaluate(&ctx), -1.0);
        approx(parse_calc("atan2(1, 1)").evaluate(&ctx), std::f64::consts::FRAC_PI_4);
    }

    #[test]
    fn power_log_exp() {
        let ctx = CalcContext::default();
        approx(parse_calc("pow(2, 10)").evaluate(&ctx), 1024.0);
        approx(parse_calc("sqrt(81)").evaluate(&ctx), 9.0);
        approx(parse_calc("hypot(3, 4)").evaluate(&ctx), 5.0);
        approx(parse_calc("hypot(3, 4, 12)").evaluate(&ctx), 13.0);
        approx(parse_calc("log(e)").evaluate(&ctx), 1.0);
        approx(parse_calc("log(100, 10)").evaluate(&ctx), 2.0);
        approx(parse_calc("exp(0)").evaluate(&ctx), 1.0);
    }

    #[test]
    fn round_strategies_via_parser() {
        let ctx = CalcContext::default();
        approx(parse_calc("round(up, 1.1, 1)").evaluate(&ctx), 2.0);
        approx(parse_calc("round(down, 1.9, 1)").evaluate(&ctx), 1.0);
        approx(parse_calc("round(to-zero, -1.9, 1)").evaluate(&ctx), -1.0);
        approx(parse_calc("round(2.5)").evaluate(&ctx), 2.0); // ties-to-even
    }

    #[test]
    fn nested_calc_kasada_style() {
        // A simplified mirror of the captured Kasada probe — nested
        // calc(1px * (... + sin(...) ...)).
        let ctx = CalcContext::default();
        let v = parse_calc("calc(1px * (2.71828 * 0.5 + sin(pi / 2)))").evaluate(&ctx);
        approx(v, 1.0 * (2.71828 * 0.5 + 1.0));
    }

    #[test]
    fn unary_negation() {
        let ctx = CalcContext::default();
        approx(parse_calc("calc(-5)").evaluate(&ctx), -5.0);
        approx(parse_calc("calc(0 - -5)").evaluate(&ctx), 5.0);
    }

    #[test]
    fn angle_units_in_trig() {
        let ctx = CalcContext::default();
        approx(parse_calc("sin(90deg)").evaluate(&ctx), 1.0);
        approx(parse_calc("cos(180deg)").evaluate(&ctx), -1.0);
        approx(parse_calc("sin(0.25turn)").evaluate(&ctx), 1.0);
    }
}
