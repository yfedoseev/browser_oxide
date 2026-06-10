use crate::css_parser::{ComponentValue, Token, TokenKind};

/// Media feature values for @media evaluation.
#[derive(Debug, Clone)]
pub struct MediaFeatures {
    pub width: f64,
    pub height: f64,
    pub device_pixel_ratio: f64,
    pub prefers_color_scheme: ColorScheme,
    pub prefers_reduced_motion: ReducedMotion,
    pub pointer: PointerType,
    pub hover: HoverCapability,
    pub scripting: Scripting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorScheme {
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReducedMotion {
    NoPreference,
    Reduce,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerType {
    None,
    Coarse,
    Fine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoverCapability {
    None,
    Hover,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scripting {
    None,
    Enabled,
}

impl Default for MediaFeatures {
    fn default() -> Self {
        Self {
            width: 1920.0,
            height: 1080.0,
            device_pixel_ratio: 1.0,
            prefers_color_scheme: ColorScheme::Light,
            prefers_reduced_motion: ReducedMotion::NoPreference,
            pointer: PointerType::Fine,
            hover: HoverCapability::Hover,
            scripting: Scripting::Enabled,
        }
    }
}

/// Evaluate a @media prelude against the current features.
///
/// This is a simplified evaluator that handles common media queries:
/// `screen`, `print`, `(min-width: Xpx)`, `(max-width: Xpx)`,
/// `(prefers-color-scheme: dark)`, etc.
pub fn evaluate_media_query(prelude: &[ComponentValue<'_>], features: &MediaFeatures) -> bool {
    let text = prelude_to_text(prelude);
    let text = text.trim().to_ascii_lowercase();

    // Empty media query = all = true
    if text.is_empty() || text == "all" {
        return true;
    }

    // "screen" matches (we're always screen)
    if text == "screen" {
        return true;
    }

    // "print" never matches
    if text == "print" {
        return false;
    }

    // Handle comma-separated media queries (OR logic)
    if text.contains(',') {
        return text
            .split(',')
            .any(|part| evaluate_single_query(part.trim(), features));
    }

    evaluate_single_query(&text, features)
}

fn evaluate_single_query(query: &str, features: &MediaFeatures) -> bool {
    let query = query.trim();

    // Strip "screen and " or "all and " prefix
    let query = query
        .strip_prefix("screen and ")
        .or_else(|| query.strip_prefix("all and "))
        .unwrap_or(query);

    // Handle "not (...)"
    if let Some(inner) = query.strip_prefix("not ") {
        return !evaluate_single_query(inner.trim(), features);
    }

    // Handle parenthesized feature: (feature: value) or (feature > value)
    let query = query.trim_start_matches('(').trim_end_matches(')');

    if let Some((feature, value)) = query.split_once(':') {
        return evaluate_feature(feature.trim(), value.trim(), features);
    }

    // Range syntax: (width > 768px), (width >= 768px)
    if let Some((feature, value)) = query.split_once(">=") {
        return evaluate_range(feature.trim(), ">=", value.trim(), features);
    }
    if let Some((feature, value)) = query.split_once("<=") {
        return evaluate_range(feature.trim(), "<=", value.trim(), features);
    }
    if let Some((feature, value)) = query.split_once('>') {
        return evaluate_range(feature.trim(), ">", value.trim(), features);
    }
    if let Some((feature, value)) = query.split_once('<') {
        return evaluate_range(feature.trim(), "<", value.trim(), features);
    }

    // Boolean feature: just the name
    match query {
        "hover" => features.hover == HoverCapability::Hover,
        "pointer" => features.pointer != PointerType::None,
        "color" => true,
        "scripting" => features.scripting == Scripting::Enabled,
        _ => true, // Unknown features default to true (forward-compat)
    }
}

fn evaluate_feature(feature: &str, value: &str, features: &MediaFeatures) -> bool {
    match feature {
        "min-width" => parse_px(value).is_some_and(|v| features.width >= v),
        "max-width" => parse_px(value).is_some_and(|v| features.width <= v),
        "min-height" => parse_px(value).is_some_and(|v| features.height >= v),
        "max-height" => parse_px(value).is_some_and(|v| features.height <= v),
        "width" => parse_px(value).is_some_and(|v| (features.width - v).abs() < 0.01),
        "height" => parse_px(value).is_some_and(|v| (features.height - v).abs() < 0.01),
        "prefers-color-scheme" => match value {
            "dark" => features.prefers_color_scheme == ColorScheme::Dark,
            "light" => features.prefers_color_scheme == ColorScheme::Light,
            _ => false,
        },
        "prefers-reduced-motion" => match value {
            "reduce" => features.prefers_reduced_motion == ReducedMotion::Reduce,
            "no-preference" => features.prefers_reduced_motion == ReducedMotion::NoPreference,
            _ => false,
        },
        "pointer" => match value {
            "fine" => features.pointer == PointerType::Fine,
            "coarse" => features.pointer == PointerType::Coarse,
            "none" => features.pointer == PointerType::None,
            _ => false,
        },
        "hover" => match value {
            "hover" => features.hover == HoverCapability::Hover,
            "none" => features.hover == HoverCapability::None,
            _ => false,
        },
        _ => true,
    }
}

fn evaluate_range(feature: &str, op: &str, value: &str, features: &MediaFeatures) -> bool {
    let feature_val = match feature {
        "width" => features.width,
        "height" => features.height,
        _ => return true,
    };
    let target = match parse_px(value) {
        Some(v) => v,
        None => return true,
    };
    match op {
        ">" => feature_val > target,
        ">=" => feature_val >= target,
        "<" => feature_val < target,
        "<=" => feature_val <= target,
        _ => true,
    }
}

fn parse_px(s: &str) -> Option<f64> {
    let s = s.trim().trim_end_matches("px").trim();
    s.parse::<f64>().ok()
}

fn prelude_to_text(prelude: &[ComponentValue<'_>]) -> String {
    let mut s = String::new();
    for cv in prelude {
        match cv {
            ComponentValue::Token(Token { kind, .. }) => match kind {
                TokenKind::Ident(v) => s.push_str(v),
                TokenKind::Number { value, .. } => s.push_str(&value.to_string()),
                TokenKind::Dimension { value, unit, .. } => {
                    s.push_str(&value.to_string());
                    s.push_str(unit);
                }
                TokenKind::Whitespace => s.push(' '),
                TokenKind::Colon => s.push(':'),
                TokenKind::Comma => s.push(','),
                TokenKind::Delim(c) => s.push(*c),
                _ => {}
            },
            ComponentValue::SimpleBlock(b) => {
                s.push(b.token);
                s.push_str(&prelude_to_text(&b.value));
                match b.token {
                    '{' => s.push('}'),
                    '[' => s.push(']'),
                    '(' => s.push(')'),
                    _ => {}
                }
            }
            ComponentValue::Function(f) => {
                s.push_str(f.name);
                s.push('(');
                s.push_str(&prelude_to_text(&f.arguments));
                s.push(')');
            }
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn features() -> MediaFeatures {
        MediaFeatures::default() // 1920x1080, light, fine pointer
    }

    fn eval(css: &str) -> bool {
        let input = format!("@media {} {{}}", css);
        let (stylesheet, _) = crate::css_parser::parse_stylesheet(&input);
        if let Some(crate::css_parser::Rule::At(at)) = stylesheet.rules.first() {
            evaluate_media_query(&at.prelude, &features())
        } else {
            panic!("Expected @media rule");
        }
    }

    #[test]
    fn screen() {
        assert!(eval("screen"));
    }

    #[test]
    fn print_false() {
        assert!(!eval("print"));
    }

    #[test]
    fn min_width_matches() {
        assert!(eval("(min-width: 768px)"));
    }

    #[test]
    fn min_width_no_match() {
        assert!(!eval("(min-width: 2000px)"));
    }

    #[test]
    fn max_width_matches() {
        assert!(eval("(max-width: 2000px)"));
    }

    #[test]
    fn prefers_color_scheme_light() {
        assert!(eval("(prefers-color-scheme: light)"));
    }

    #[test]
    fn prefers_color_scheme_dark_no_match() {
        assert!(!eval("(prefers-color-scheme: dark)"));
    }

    #[test]
    fn range_syntax() {
        assert!(eval("(width > 768px)"));
    }
}
