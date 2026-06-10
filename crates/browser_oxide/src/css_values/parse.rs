use crate::css_parser::{ComponentValue, Token, TokenKind};
use crate::css_values::error::ValueError;
use crate::css_values::property::*;
use crate::css_values::types::color::{self, Color};
use crate::css_values::types::display::*;
use crate::css_values::types::font::*;
use crate::css_values::types::length::*;

/// Parse a property name + value tokens into a typed PropertyDeclaration.
pub fn parse_property(
    name: &str,
    value: &[ComponentValue<'_>],
    important: bool,
) -> Result<Vec<PropertyDeclaration>, ValueError> {
    let value_trimmed = trim_whitespace(value);

    // CSS-wide keywords
    if let Some(keyword) = try_css_wide_keyword(value_trimmed) {
        return Ok(vec![PropertyDeclaration {
            property: PropertyId::from_name(name),
            value: keyword,
            important,
        }]);
    }

    let name_lower = name.to_ascii_lowercase();

    // Shorthand expansion
    match name_lower.as_str() {
        "margin" => return parse_box_shorthand(value_trimmed, important, "margin"),
        "padding" => return parse_box_shorthand(value_trimmed, important, "padding"),
        "overflow" => return parse_overflow_shorthand(value_trimmed, important),
        _ => {}
    }

    // Longhand properties
    let css_value = match name_lower.as_str() {
        "display" => parse_display(value_trimmed)?,
        "position" => parse_position(value_trimmed)?,
        "width" | "height" | "min-width" | "min-height" => {
            parse_length_percentage_auto(value_trimmed)?
        }
        "max-width" | "max-height" => parse_length_percentage_auto_none(value_trimmed)?,
        "margin-top" | "margin-right" | "margin-bottom" | "margin-left" => {
            parse_length_percentage_auto(value_trimmed)?
        }
        "padding-top" | "padding-right" | "padding-bottom" | "padding-left" => {
            parse_length_percentage(value_trimmed)?
        }
        "border-top-width" | "border-right-width" | "border-bottom-width" | "border-left-width" => {
            parse_border_width(value_trimmed)?
        }
        "box-sizing" => parse_box_sizing(value_trimmed)?,
        "overflow-x" | "overflow-y" => parse_overflow(value_trimmed)?,
        "float" => parse_float(value_trimmed)?,
        "clear" => parse_clear(value_trimmed)?,
        "flex-direction" => parse_flex_direction(value_trimmed)?,
        "flex-wrap" => parse_flex_wrap(value_trimmed)?,
        "flex-grow" | "flex-shrink" => parse_number(value_trimmed)?,
        "flex-basis" => parse_length_percentage_auto(value_trimmed)?,
        "align-items" | "align-self" | "align-content" | "justify-content" | "justify-items"
        | "justify-self" => parse_alignment(value_trimmed)?,
        "gap" | "row-gap" | "column-gap" => parse_length_percentage(value_trimmed)?,
        "font-size" => parse_font_size(value_trimmed)?,
        "font-family" => parse_font_family(value_trimmed)?,
        "font-weight" => parse_font_weight(value_trimmed)?,
        "font-style" => parse_font_style(value_trimmed)?,
        "line-height" => parse_line_height(value_trimmed)?,
        "text-align" => parse_text_align(value_trimmed)?,
        "white-space" => parse_white_space(value_trimmed)?,
        "color" | "background-color" => parse_color(value_trimmed)?,
        "visibility" => parse_visibility(value_trimmed)?,
        "opacity" => parse_number(value_trimmed)?,
        "z-index" => parse_z_index(value_trimmed)?,
        "content-visibility" => parse_content_visibility(value_trimmed)?,
        _ if name_lower.starts_with("--") => {
            CssValue::CustomValue(component_values_to_string(value_trimmed))
        }
        _ => return Err(ValueError::UnknownProperty(name_lower)),
    };

    Ok(vec![PropertyDeclaration {
        property: PropertyId::from_name(name),
        value: css_value,
        important,
    }])
}

// --- Individual parsers ---

fn try_css_wide_keyword(value: &[ComponentValue<'_>]) -> Option<CssValue> {
    if value.len() != 1 {
        return None;
    }
    match &value[0] {
        ComponentValue::Token(Token {
            kind: TokenKind::Ident(name),
            ..
        }) => match name.to_ascii_lowercase().as_str() {
            "initial" => Some(CssValue::Initial),
            "inherit" => Some(CssValue::Inherit),
            "unset" => Some(CssValue::Unset),
            "revert" => Some(CssValue::Revert),
            "revert-layer" => Some(CssValue::RevertLayer),
            _ => None,
        },
        _ => None,
    }
}

fn parse_display(value: &[ComponentValue<'_>]) -> Result<CssValue, ValueError> {
    let ident = expect_single_ident(value)?;
    let d = match ident.to_ascii_lowercase().as_str() {
        "none" => Display::None,
        "block" => Display::Block,
        "inline" => Display::Inline,
        "inline-block" => Display::InlineBlock,
        "flex" => Display::Flex,
        "inline-flex" => Display::InlineFlex,
        "grid" => Display::Grid,
        "inline-grid" => Display::InlineGrid,
        "table" => Display::Table,
        "inline-table" => Display::InlineTable,
        "list-item" => Display::ListItem,
        "flow-root" => Display::FlowRoot,
        "contents" => Display::Contents,
        "table-row" => Display::TableRow,
        "table-cell" => Display::TableCell,
        _ => {
            return Err(ValueError::InvalidValue(format!(
                "invalid display: {}",
                ident
            )))
        }
    };
    Ok(CssValue::Display(d))
}

fn parse_position(value: &[ComponentValue<'_>]) -> Result<CssValue, ValueError> {
    let ident = expect_single_ident(value)?;
    let p = match ident.to_ascii_lowercase().as_str() {
        "static" => Position::Static,
        "relative" => Position::Relative,
        "absolute" => Position::Absolute,
        "fixed" => Position::Fixed,
        "sticky" => Position::Sticky,
        _ => {
            return Err(ValueError::InvalidValue(format!(
                "invalid position: {}",
                ident
            )))
        }
    };
    Ok(CssValue::Position(p))
}

fn parse_length_percentage_auto(value: &[ComponentValue<'_>]) -> Result<CssValue, ValueError> {
    if value.len() == 1 {
        if let Some(ident) = try_ident(&value[0]) {
            if ident.eq_ignore_ascii_case("auto") {
                return Ok(CssValue::LengthPercentageAuto(LengthPercentageAuto::Auto));
            }
        }
        if let Some(lp) = try_length_percentage(&value[0]) {
            return Ok(CssValue::LengthPercentageAuto(match lp {
                LengthPercentage::Length(l) => LengthPercentageAuto::Length(l),
                LengthPercentage::Percentage(p) => LengthPercentageAuto::Percentage(p),
                LengthPercentage::Calc(c) => LengthPercentageAuto::Calc(c),
            }));
        }
    }
    Err(ValueError::InvalidValue(
        "expected length, percentage, or auto".into(),
    ))
}

fn parse_length_percentage_auto_none(value: &[ComponentValue<'_>]) -> Result<CssValue, ValueError> {
    if value.len() == 1 {
        if let Some(ident) = try_ident(&value[0]) {
            if ident.eq_ignore_ascii_case("none") {
                // max-width: none → no limit (represented as auto)
                return Ok(CssValue::LengthPercentageAuto(LengthPercentageAuto::Auto));
            }
        }
    }
    parse_length_percentage_auto(value)
}

fn parse_length_percentage(value: &[ComponentValue<'_>]) -> Result<CssValue, ValueError> {
    if value.len() == 1 {
        if let Some(lp) = try_length_percentage(&value[0]) {
            return Ok(CssValue::LengthPercentage(lp));
        }
    }
    Err(ValueError::InvalidValue(
        "expected length or percentage".into(),
    ))
}

fn parse_border_width(value: &[ComponentValue<'_>]) -> Result<CssValue, ValueError> {
    if value.len() == 1 {
        if let Some(ident) = try_ident(&value[0]) {
            let px = match ident.to_ascii_lowercase().as_str() {
                "thin" => 1.0,
                "medium" => 3.0,
                "thick" => 5.0,
                _ => {
                    return Err(ValueError::InvalidValue(format!(
                        "invalid border-width: {}",
                        ident
                    )))
                }
            };
            return Ok(CssValue::Length(Length::Px(px)));
        }
        if let Some(lp) = try_length_percentage(&value[0]) {
            return Ok(match lp {
                LengthPercentage::Length(l) => CssValue::Length(l),
                _ => {
                    return Err(ValueError::InvalidValue(
                        "border-width must be a length".into(),
                    ))
                }
            });
        }
    }
    Err(ValueError::InvalidValue(
        "expected border-width value".into(),
    ))
}

fn parse_box_sizing(value: &[ComponentValue<'_>]) -> Result<CssValue, ValueError> {
    let ident = expect_single_ident(value)?;
    match ident.to_ascii_lowercase().as_str() {
        "content-box" => Ok(CssValue::BoxSizing(BoxSizing::ContentBox)),
        "border-box" => Ok(CssValue::BoxSizing(BoxSizing::BorderBox)),
        _ => Err(ValueError::InvalidValue(format!(
            "invalid box-sizing: {}",
            ident
        ))),
    }
}

fn parse_overflow(value: &[ComponentValue<'_>]) -> Result<CssValue, ValueError> {
    let ident = expect_single_ident(value)?;
    let o = match ident.to_ascii_lowercase().as_str() {
        "visible" => Overflow::Visible,
        "hidden" => Overflow::Hidden,
        "scroll" => Overflow::Scroll,
        "auto" => Overflow::Auto,
        "clip" => Overflow::Clip,
        _ => {
            return Err(ValueError::InvalidValue(format!(
                "invalid overflow: {}",
                ident
            )))
        }
    };
    Ok(CssValue::Overflow(o))
}

fn parse_float(value: &[ComponentValue<'_>]) -> Result<CssValue, ValueError> {
    let ident = expect_single_ident(value)?;
    let f = match ident.to_ascii_lowercase().as_str() {
        "none" => Float::None,
        "left" => Float::Left,
        "right" => Float::Right,
        _ => {
            return Err(ValueError::InvalidValue(format!(
                "invalid float: {}",
                ident
            )))
        }
    };
    Ok(CssValue::Float(f))
}

fn parse_clear(value: &[ComponentValue<'_>]) -> Result<CssValue, ValueError> {
    let ident = expect_single_ident(value)?;
    let c = match ident.to_ascii_lowercase().as_str() {
        "none" => Clear::None,
        "left" => Clear::Left,
        "right" => Clear::Right,
        "both" => Clear::Both,
        _ => {
            return Err(ValueError::InvalidValue(format!(
                "invalid clear: {}",
                ident
            )))
        }
    };
    Ok(CssValue::Clear(c))
}

fn parse_flex_direction(value: &[ComponentValue<'_>]) -> Result<CssValue, ValueError> {
    let ident = expect_single_ident(value)?;
    let d = match ident.to_ascii_lowercase().as_str() {
        "row" => FlexDirection::Row,
        "row-reverse" => FlexDirection::RowReverse,
        "column" => FlexDirection::Column,
        "column-reverse" => FlexDirection::ColumnReverse,
        _ => {
            return Err(ValueError::InvalidValue(format!(
                "invalid flex-direction: {}",
                ident
            )))
        }
    };
    Ok(CssValue::FlexDirection(d))
}

fn parse_flex_wrap(value: &[ComponentValue<'_>]) -> Result<CssValue, ValueError> {
    let ident = expect_single_ident(value)?;
    let w = match ident.to_ascii_lowercase().as_str() {
        "nowrap" => FlexWrap::Nowrap,
        "wrap" => FlexWrap::Wrap,
        "wrap-reverse" => FlexWrap::WrapReverse,
        _ => {
            return Err(ValueError::InvalidValue(format!(
                "invalid flex-wrap: {}",
                ident
            )))
        }
    };
    Ok(CssValue::FlexWrap(w))
}

fn parse_alignment(value: &[ComponentValue<'_>]) -> Result<CssValue, ValueError> {
    let ident = expect_single_ident(value)?;
    let a = match ident.to_ascii_lowercase().as_str() {
        "normal" => AlignmentValue::Normal,
        "stretch" => AlignmentValue::Stretch,
        "center" => AlignmentValue::Center,
        "start" => AlignmentValue::Start,
        "end" => AlignmentValue::End,
        "flex-start" => AlignmentValue::FlexStart,
        "flex-end" => AlignmentValue::FlexEnd,
        "baseline" => AlignmentValue::Baseline,
        "space-between" => AlignmentValue::SpaceBetween,
        "space-around" => AlignmentValue::SpaceAround,
        "space-evenly" => AlignmentValue::SpaceEvenly,
        _ => {
            return Err(ValueError::InvalidValue(format!(
                "invalid alignment: {}",
                ident
            )))
        }
    };
    Ok(CssValue::Alignment(a))
}

fn parse_number(value: &[ComponentValue<'_>]) -> Result<CssValue, ValueError> {
    if value.len() == 1 {
        if let ComponentValue::Token(Token {
            kind: TokenKind::Number { value: v, .. },
            ..
        }) = &value[0]
        {
            return Ok(CssValue::Number(*v));
        }
    }
    Err(ValueError::InvalidValue("expected number".into()))
}

fn parse_font_size(value: &[ComponentValue<'_>]) -> Result<CssValue, ValueError> {
    if value.len() == 1 {
        if let Some(ident) = try_ident(&value[0]) {
            let px = match ident.to_ascii_lowercase().as_str() {
                "xx-small" => 9.0,
                "x-small" => 10.0,
                "small" => 13.0,
                "medium" => 16.0,
                "large" => 18.0,
                "x-large" => 24.0,
                "xx-large" => 32.0,
                "xxx-large" => 48.0,
                "smaller" | "larger" => {
                    // Relative sizes — need parent context, store as keyword for now
                    return Ok(CssValue::LengthPercentage(LengthPercentage::Length(
                        Length::Em(if ident.eq_ignore_ascii_case("larger") {
                            1.2
                        } else {
                            0.833
                        }),
                    )));
                }
                _ => {
                    return Err(ValueError::InvalidValue(format!(
                        "invalid font-size: {}",
                        ident
                    )))
                }
            };
            return Ok(CssValue::LengthPercentage(LengthPercentage::Length(
                Length::Px(px),
            )));
        }
        if let Some(lp) = try_length_percentage(&value[0]) {
            return Ok(CssValue::LengthPercentage(lp));
        }
    }
    Err(ValueError::InvalidValue("expected font-size value".into()))
}

fn parse_font_family(value: &[ComponentValue<'_>]) -> Result<CssValue, ValueError> {
    let mut families = Vec::new();
    let mut current_name_parts: Vec<String> = Vec::new();

    for cv in value {
        match cv {
            ComponentValue::Token(Token {
                kind: TokenKind::Comma,
                ..
            }) if !current_name_parts.is_empty() => {
                let name = current_name_parts.join(" ");
                families.push(match_generic_or_named(&name));
                current_name_parts.clear();
            }
            ComponentValue::Token(Token {
                kind: TokenKind::Ident(name),
                ..
            }) => {
                current_name_parts.push(name.to_string());
            }
            ComponentValue::Token(Token {
                kind: TokenKind::String(name),
                ..
            }) => {
                families.push(FontFamily::Named(name.to_string()));
            }
            ComponentValue::Token(Token {
                kind: TokenKind::Whitespace,
                ..
            }) => {}
            _ => {}
        }
    }

    if !current_name_parts.is_empty() {
        let name = current_name_parts.join(" ");
        families.push(match_generic_or_named(&name));
    }

    if families.is_empty() {
        return Err(ValueError::InvalidValue("expected font-family".into()));
    }

    Ok(CssValue::FontFamily(families))
}

fn match_generic_or_named(name: &str) -> FontFamily {
    match name.to_ascii_lowercase().as_str() {
        "serif" => FontFamily::Generic(GenericFamily::Serif),
        "sans-serif" => FontFamily::Generic(GenericFamily::SansSerif),
        "monospace" => FontFamily::Generic(GenericFamily::Monospace),
        "cursive" => FontFamily::Generic(GenericFamily::Cursive),
        "fantasy" => FontFamily::Generic(GenericFamily::Fantasy),
        "system-ui" => FontFamily::Generic(GenericFamily::SystemUi),
        "ui-serif" => FontFamily::Generic(GenericFamily::UiSerif),
        "ui-sans-serif" => FontFamily::Generic(GenericFamily::UiSansSerif),
        "ui-monospace" => FontFamily::Generic(GenericFamily::UiMonospace),
        "ui-rounded" => FontFamily::Generic(GenericFamily::UiRounded),
        "emoji" => FontFamily::Generic(GenericFamily::Emoji),
        "math" => FontFamily::Generic(GenericFamily::Math),
        _ => FontFamily::Named(name.to_string()),
    }
}

fn parse_font_weight(value: &[ComponentValue<'_>]) -> Result<CssValue, ValueError> {
    if value.len() == 1 {
        if let Some(ident) = try_ident(&value[0]) {
            let w = match ident.to_ascii_lowercase().as_str() {
                "normal" => FontWeight::Normal,
                "bold" => FontWeight::Bold,
                "bolder" => FontWeight::Bolder,
                "lighter" => FontWeight::Lighter,
                _ => {
                    return Err(ValueError::InvalidValue(format!(
                        "invalid font-weight: {}",
                        ident
                    )))
                }
            };
            return Ok(CssValue::FontWeight(w));
        }
        if let ComponentValue::Token(Token {
            kind: TokenKind::Number { value: v, .. },
            ..
        }) = &value[0]
        {
            return Ok(CssValue::FontWeight(FontWeight::Numeric(*v)));
        }
    }
    Err(ValueError::InvalidValue(
        "expected font-weight value".into(),
    ))
}

fn parse_font_style(value: &[ComponentValue<'_>]) -> Result<CssValue, ValueError> {
    let ident = expect_single_ident(value)?;
    let s = match ident.to_ascii_lowercase().as_str() {
        "normal" => FontStyle::Normal,
        "italic" => FontStyle::Italic,
        "oblique" => FontStyle::Oblique(None),
        _ => {
            return Err(ValueError::InvalidValue(format!(
                "invalid font-style: {}",
                ident
            )))
        }
    };
    Ok(CssValue::FontStyle(s))
}

fn parse_line_height(value: &[ComponentValue<'_>]) -> Result<CssValue, ValueError> {
    if value.len() == 1 {
        if let Some(ident) = try_ident(&value[0]) {
            if ident.eq_ignore_ascii_case("normal") {
                return Ok(CssValue::LineHeight(LineHeight::Normal));
            }
        }
        if let ComponentValue::Token(Token {
            kind: TokenKind::Number { value: v, .. },
            ..
        }) = &value[0]
        {
            return Ok(CssValue::LineHeight(LineHeight::Number(*v)));
        }
        if let Some(lp) = try_length_percentage(&value[0]) {
            return Ok(match lp {
                LengthPercentage::Length(l) => CssValue::LineHeight(LineHeight::Length(l)),
                LengthPercentage::Percentage(p) => CssValue::LineHeight(LineHeight::Percentage(p)),
                _ => return Err(ValueError::InvalidValue("invalid line-height".into())),
            });
        }
    }
    Err(ValueError::InvalidValue(
        "expected line-height value".into(),
    ))
}

fn parse_text_align(value: &[ComponentValue<'_>]) -> Result<CssValue, ValueError> {
    let ident = expect_single_ident(value)?;
    let a = match ident.to_ascii_lowercase().as_str() {
        "left" => TextAlign::Left,
        "right" => TextAlign::Right,
        "center" => TextAlign::Center,
        "justify" => TextAlign::Justify,
        "start" => TextAlign::Start,
        "end" => TextAlign::End,
        _ => {
            return Err(ValueError::InvalidValue(format!(
                "invalid text-align: {}",
                ident
            )))
        }
    };
    Ok(CssValue::TextAlign(a))
}

fn parse_white_space(value: &[ComponentValue<'_>]) -> Result<CssValue, ValueError> {
    let ident = expect_single_ident(value)?;
    let w = match ident.to_ascii_lowercase().as_str() {
        "normal" => WhiteSpace::Normal,
        "nowrap" => WhiteSpace::Nowrap,
        "pre" => WhiteSpace::Pre,
        "pre-wrap" => WhiteSpace::PreWrap,
        "pre-line" => WhiteSpace::PreLine,
        "break-spaces" => WhiteSpace::BreakSpaces,
        _ => {
            return Err(ValueError::InvalidValue(format!(
                "invalid white-space: {}",
                ident
            )))
        }
    };
    Ok(CssValue::WhiteSpace(w))
}

fn parse_color(value: &[ComponentValue<'_>]) -> Result<CssValue, ValueError> {
    // Named color or currentcolor
    if value.len() == 1 {
        if let Some(ident) = try_ident(&value[0]) {
            if let Some(c) = color::named_color(&ident) {
                return Ok(CssValue::Color(c));
            }
        }
        // Hex color
        if let ComponentValue::Token(Token {
            kind: TokenKind::Hash { value: hex, .. },
            ..
        }) = &value[0]
        {
            if let Some(c) = parse_hex_color(hex) {
                return Ok(CssValue::Color(c));
            }
        }
    }
    // rgb() / rgba() / hsl() / hsla() functions
    if value.len() == 1 {
        if let ComponentValue::Function(f) = &value[0] {
            match f.name.to_ascii_lowercase().as_str() {
                "rgb" | "rgba" => return parse_rgb_function(&f.arguments),
                "hsl" | "hsla" => return parse_hsl_function(&f.arguments),
                _ => {}
            }
        }
    }
    Err(ValueError::InvalidValue("expected color value".into()))
}

fn parse_hex_color(hex: &str) -> Option<Color> {
    let hex = hex.trim();
    let chars: Vec<u8> = hex.bytes().collect();
    match chars.len() {
        3 => {
            let r = hex_digit(chars[0])? * 17;
            let g = hex_digit(chars[1])? * 17;
            let b = hex_digit(chars[2])? * 17;
            Some(Color::Rgba { r, g, b, a: 1.0 })
        }
        4 => {
            let r = hex_digit(chars[0])? * 17;
            let g = hex_digit(chars[1])? * 17;
            let b = hex_digit(chars[2])? * 17;
            let a = hex_digit(chars[3])? as f32 * 17.0 / 255.0;
            Some(Color::Rgba { r, g, b, a })
        }
        6 => {
            let r = hex_digit(chars[0])? * 16 + hex_digit(chars[1])?;
            let g = hex_digit(chars[2])? * 16 + hex_digit(chars[3])?;
            let b = hex_digit(chars[4])? * 16 + hex_digit(chars[5])?;
            Some(Color::Rgba { r, g, b, a: 1.0 })
        }
        8 => {
            let r = hex_digit(chars[0])? * 16 + hex_digit(chars[1])?;
            let g = hex_digit(chars[2])? * 16 + hex_digit(chars[3])?;
            let b = hex_digit(chars[4])? * 16 + hex_digit(chars[5])?;
            let a = (hex_digit(chars[6])? * 16 + hex_digit(chars[7])?) as f32 / 255.0;
            Some(Color::Rgba { r, g, b, a })
        }
        _ => None,
    }
}

fn hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn parse_rgb_function(args: &[ComponentValue<'_>]) -> Result<CssValue, ValueError> {
    let nums = extract_numbers(args);
    if nums.len() >= 3 {
        let r = nums[0].clamp(0.0, 255.0) as u8;
        let g = nums[1].clamp(0.0, 255.0) as u8;
        let b = nums[2].clamp(0.0, 255.0) as u8;
        let a = if nums.len() >= 4 { nums[3] as f32 } else { 1.0 };
        Ok(CssValue::Color(Color::Rgba { r, g, b, a }))
    } else {
        Err(ValueError::InvalidValue("invalid rgb() arguments".into()))
    }
}

fn parse_hsl_function(args: &[ComponentValue<'_>]) -> Result<CssValue, ValueError> {
    let nums = extract_numbers(args);
    if nums.len() >= 3 {
        let h = nums[0];
        let s = nums[1];
        let l = nums[2];
        let a = if nums.len() >= 4 { nums[3] as f32 } else { 1.0 };
        Ok(CssValue::Color(Color::Hsl { h, s, l, a }))
    } else {
        Err(ValueError::InvalidValue("invalid hsl() arguments".into()))
    }
}

fn parse_visibility(value: &[ComponentValue<'_>]) -> Result<CssValue, ValueError> {
    let ident = expect_single_ident(value)?;
    let v = match ident.to_ascii_lowercase().as_str() {
        "visible" => Visibility::Visible,
        "hidden" => Visibility::Hidden,
        "collapse" => Visibility::Collapse,
        _ => {
            return Err(ValueError::InvalidValue(format!(
                "invalid visibility: {}",
                ident
            )))
        }
    };
    Ok(CssValue::Visibility(v))
}

fn parse_z_index(value: &[ComponentValue<'_>]) -> Result<CssValue, ValueError> {
    if value.len() == 1 {
        if let Some(ident) = try_ident(&value[0]) {
            if ident.eq_ignore_ascii_case("auto") {
                return Ok(CssValue::Integer(0));
            }
        }
        if let ComponentValue::Token(Token {
            kind: TokenKind::Number {
                int_value: Some(v), ..
            },
            ..
        }) = &value[0]
        {
            return Ok(CssValue::Integer(*v as i32));
        }
    }
    Err(ValueError::InvalidValue("expected z-index value".into()))
}

fn parse_content_visibility(value: &[ComponentValue<'_>]) -> Result<CssValue, ValueError> {
    let ident = expect_single_ident(value)?;
    let cv = match ident.to_ascii_lowercase().as_str() {
        "visible" => ContentVisibility::Visible,
        "hidden" => ContentVisibility::Hidden,
        "auto" => ContentVisibility::Auto,
        _ => {
            return Err(ValueError::InvalidValue(format!(
                "invalid content-visibility: {}",
                ident
            )))
        }
    };
    Ok(CssValue::ContentVisibility(cv))
}

// --- Shorthand parsers ---

fn parse_box_shorthand(
    value: &[ComponentValue<'_>],
    important: bool,
    prefix: &str,
) -> Result<Vec<PropertyDeclaration>, ValueError> {
    // Collect non-whitespace values
    let parts: Vec<&ComponentValue<'_>> = value
        .iter()
        .filter(|v| {
            !matches!(
                v,
                ComponentValue::Token(Token {
                    kind: TokenKind::Whitespace,
                    ..
                })
            )
        })
        .collect();

    let (top, right, bottom, left) = match parts.len() {
        1 => (parts[0], parts[0], parts[0], parts[0]),
        2 => (parts[0], parts[1], parts[0], parts[1]),
        3 => (parts[0], parts[1], parts[2], parts[1]),
        4 => (parts[0], parts[1], parts[2], parts[3]),
        _ => {
            return Err(ValueError::InvalidValue(format!(
                "invalid {} shorthand",
                prefix
            )))
        }
    };

    let sides = ["top", "right", "bottom", "left"];
    let values = [top, right, bottom, left];
    let mut declarations = Vec::new();

    for (side, val) in sides.iter().zip(values.iter()) {
        let prop_name = format!("{}-{}", prefix, side);
        let cv = if let Some(ident) = try_ident(val) {
            if ident.eq_ignore_ascii_case("auto") {
                CssValue::LengthPercentageAuto(LengthPercentageAuto::Auto)
            } else {
                return Err(ValueError::InvalidValue(format!(
                    "invalid {} value",
                    prefix
                )));
            }
        } else if let Some(lp) = try_length_percentage(val) {
            match lp {
                LengthPercentage::Length(l) => {
                    if prefix == "margin" {
                        CssValue::LengthPercentageAuto(LengthPercentageAuto::Length(l))
                    } else {
                        CssValue::LengthPercentage(LengthPercentage::Length(l))
                    }
                }
                LengthPercentage::Percentage(p) => {
                    if prefix == "margin" {
                        CssValue::LengthPercentageAuto(LengthPercentageAuto::Percentage(p))
                    } else {
                        CssValue::LengthPercentage(LengthPercentage::Percentage(p))
                    }
                }
                other => CssValue::LengthPercentage(other),
            }
        } else {
            return Err(ValueError::InvalidValue(format!(
                "invalid {} value",
                prefix
            )));
        };

        declarations.push(PropertyDeclaration {
            property: PropertyId::from_name(&prop_name),
            value: cv,
            important,
        });
    }

    Ok(declarations)
}

fn parse_overflow_shorthand(
    value: &[ComponentValue<'_>],
    important: bool,
) -> Result<Vec<PropertyDeclaration>, ValueError> {
    let parts: Vec<&ComponentValue<'_>> = value
        .iter()
        .filter(|v| {
            !matches!(
                v,
                ComponentValue::Token(Token {
                    kind: TokenKind::Whitespace,
                    ..
                })
            )
        })
        .collect();

    let (x_val, y_val) = match parts.len() {
        1 => (parts[0], parts[0]),
        2 => (parts[0], parts[1]),
        _ => {
            return Err(ValueError::InvalidValue(
                "invalid overflow shorthand".into(),
            ))
        }
    };

    let parse_single = |v: &ComponentValue<'_>| -> Result<CssValue, ValueError> {
        if try_ident(v).is_some() {
            parse_overflow(std::slice::from_ref(v))
        } else {
            Err(ValueError::InvalidValue("expected overflow keyword".into()))
        }
    };

    Ok(vec![
        PropertyDeclaration {
            property: PropertyId::OverflowX,
            value: parse_single(x_val)?,
            important,
        },
        PropertyDeclaration {
            property: PropertyId::OverflowY,
            value: parse_single(y_val)?,
            important,
        },
    ])
}

// --- Helpers ---

fn trim_whitespace<'a, 'b>(value: &'a [ComponentValue<'b>]) -> &'a [ComponentValue<'b>] {
    let start = value
        .iter()
        .position(|v| {
            !matches!(
                v,
                ComponentValue::Token(Token {
                    kind: TokenKind::Whitespace,
                    ..
                })
            )
        })
        .unwrap_or(value.len());
    let end = value
        .iter()
        .rposition(|v| {
            !matches!(
                v,
                ComponentValue::Token(Token {
                    kind: TokenKind::Whitespace,
                    ..
                })
            )
        })
        .map(|i| i + 1)
        .unwrap_or(0);
    if start >= end {
        &[]
    } else {
        &value[start..end]
    }
}

fn expect_single_ident(value: &[ComponentValue<'_>]) -> Result<String, ValueError> {
    if value.len() == 1 {
        if let Some(ident) = try_ident(&value[0]) {
            return Ok(ident);
        }
    }
    Err(ValueError::InvalidValue(
        "expected single identifier".into(),
    ))
}

fn try_ident(cv: &ComponentValue<'_>) -> Option<String> {
    match cv {
        ComponentValue::Token(Token {
            kind: TokenKind::Ident(name),
            ..
        }) => Some(name.to_string()),
        _ => None,
    }
}

fn try_length_percentage(cv: &ComponentValue<'_>) -> Option<LengthPercentage> {
    match cv {
        ComponentValue::Token(Token {
            kind: TokenKind::Dimension { value, unit, .. },
            ..
        }) => {
            let length = match unit.to_ascii_lowercase().as_str() {
                "px" => Length::Px(*value),
                "em" => Length::Em(*value),
                "rem" => Length::Rem(*value),
                "vw" => Length::Vw(*value),
                "vh" => Length::Vh(*value),
                "vmin" => Length::Vmin(*value),
                "vmax" => Length::Vmax(*value),
                "cm" => Length::Cm(*value),
                "mm" => Length::Mm(*value),
                "in" => Length::In(*value),
                "pt" => Length::Pt(*value),
                "pc" => Length::Pc(*value),
                "ch" => Length::Ch(*value),
                "ex" => Length::Ex(*value),
                "cqw" => Length::Cqw(*value),
                "cqh" => Length::Cqh(*value),
                _ => return None,
            };
            Some(LengthPercentage::Length(length))
        }
        ComponentValue::Token(Token {
            kind: TokenKind::Percentage { value, .. },
            ..
        }) => Some(LengthPercentage::Percentage(*value)),
        ComponentValue::Token(Token {
            kind: TokenKind::Number { value, .. },
            ..
        }) if *value == 0.0 => Some(LengthPercentage::Length(Length::Zero)),
        // calc(), min(), max(), clamp(), and the CSS Values 4 math
        // functions. The parser builds a CalcExpr tree; resolution to
        // a concrete pixel value happens at computed-style time via
        // CalcExpr::evaluate(&CalcContext). Carrying the unresolved
        // tree means the parsed property value is correctly populated
        // even when the math depends on layout context (vw/em/etc.) —
        // and getComputedStyle output stays correct under the
        // challenge vendor's calc-fingerprint probe.
        ComponentValue::Function(f) => match crate::css_values::calc::parse_math_function(f) {
            Ok(Some(expr)) => Some(LengthPercentage::Calc(Box::new(expr))),
            _ => None,
        },
        _ => None,
    }
}

fn extract_numbers(args: &[ComponentValue<'_>]) -> Vec<f64> {
    let mut nums = Vec::new();
    for cv in args {
        match cv {
            ComponentValue::Token(Token {
                kind: TokenKind::Number { value, .. },
                ..
            }) => nums.push(*value),
            ComponentValue::Token(Token {
                kind: TokenKind::Percentage { value, .. },
                ..
            }) => nums.push(*value),
            _ => {}
        }
    }
    nums
}

fn component_values_to_string(value: &[ComponentValue<'_>]) -> String {
    let mut s = String::new();
    for cv in value {
        match cv {
            ComponentValue::Token(Token { kind, .. }) => match kind {
                TokenKind::Ident(v) => s.push_str(v),
                TokenKind::String(v) => {
                    s.push('"');
                    s.push_str(v);
                    s.push('"');
                }
                TokenKind::Number { value, .. } => s.push_str(&value.to_string()),
                TokenKind::Dimension { value, unit, .. } => {
                    s.push_str(&value.to_string());
                    s.push_str(unit);
                }
                TokenKind::Percentage { value, .. } => {
                    s.push_str(&value.to_string());
                    s.push('%');
                }
                TokenKind::Whitespace => s.push(' '),
                TokenKind::Comma => s.push_str(", "),
                TokenKind::Colon => s.push(':'),
                TokenKind::Semicolon => s.push(';'),
                TokenKind::Delim(c) => s.push(*c),
                TokenKind::Hash { value, .. } => {
                    s.push('#');
                    s.push_str(value);
                }
                _ => {}
            },
            ComponentValue::Function(f) => {
                s.push_str(f.name);
                s.push('(');
                s.push_str(&component_values_to_string(&f.arguments));
                s.push(')');
            }
            ComponentValue::SimpleBlock(b) => {
                s.push(b.token);
                s.push_str(&component_values_to_string(&b.value));
                match b.token {
                    '{' => s.push('}'),
                    '[' => s.push(']'),
                    '(' => s.push(')'),
                    _ => {}
                }
            }
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::css_parser::parse_declaration_list;

    fn parse_decl(css: &str) -> Vec<PropertyDeclaration> {
        let (decls, _errors) = parse_declaration_list(css);
        let mut results = Vec::new();
        for d in &decls {
            match parse_property(d.name, &d.value, d.important) {
                Ok(props) => results.extend(props),
                Err(e) => panic!("Parse error for '{}': {}", d.name, e),
            }
        }
        results
    }

    #[test]
    fn parse_display_block() {
        let decls = parse_decl("display: block");
        assert_eq!(decls[0].value, CssValue::Display(Display::Block));
    }

    #[test]
    fn parse_display_flex() {
        let decls = parse_decl("display: flex");
        assert_eq!(decls[0].value, CssValue::Display(Display::Flex));
    }

    #[test]
    fn parse_position_absolute() {
        let decls = parse_decl("position: absolute");
        assert_eq!(decls[0].value, CssValue::Position(Position::Absolute));
    }

    #[test]
    fn parse_width_px() {
        let decls = parse_decl("width: 100px");
        assert_eq!(
            decls[0].value,
            CssValue::LengthPercentageAuto(LengthPercentageAuto::Length(Length::Px(100.0)))
        );
    }

    #[test]
    fn parse_width_auto() {
        let decls = parse_decl("width: auto");
        assert_eq!(
            decls[0].value,
            CssValue::LengthPercentageAuto(LengthPercentageAuto::Auto)
        );
    }

    #[test]
    fn parse_width_percentage() {
        let decls = parse_decl("width: 50%");
        assert_eq!(
            decls[0].value,
            CssValue::LengthPercentageAuto(LengthPercentageAuto::Percentage(50.0))
        );
    }

    #[test]
    fn parse_margin_shorthand() {
        let decls = parse_decl("margin: 10px 20px");
        assert_eq!(decls.len(), 4);
        assert_eq!(decls[0].property, PropertyId::MarginTop);
        assert_eq!(decls[1].property, PropertyId::MarginRight);
        assert_eq!(decls[2].property, PropertyId::MarginBottom);
        assert_eq!(decls[3].property, PropertyId::MarginLeft);
    }

    #[test]
    fn parse_color_named() {
        let decls = parse_decl("color: red");
        assert!(matches!(
            decls[0].value,
            CssValue::Color(Color::Rgba {
                r: 255,
                g: 0,
                b: 0,
                ..
            })
        ));
    }

    #[test]
    fn parse_color_hex() {
        let decls = parse_decl("color: #ff0000");
        assert!(matches!(
            decls[0].value,
            CssValue::Color(Color::Rgba {
                r: 255,
                g: 0,
                b: 0,
                ..
            })
        ));
    }

    #[test]
    fn parse_color_hex_short() {
        let decls = parse_decl("color: #f00");
        assert!(matches!(
            decls[0].value,
            CssValue::Color(Color::Rgba {
                r: 255,
                g: 0,
                b: 0,
                ..
            })
        ));
    }

    #[test]
    fn parse_color_rgb() {
        let decls = parse_decl("color: rgb(255, 128, 0)");
        assert!(matches!(
            decls[0].value,
            CssValue::Color(Color::Rgba {
                r: 255,
                g: 128,
                b: 0,
                ..
            })
        ));
    }

    #[test]
    fn parse_font_family_generic() {
        let decls = parse_decl("font-family: sans-serif");
        assert_eq!(
            decls[0].value,
            CssValue::FontFamily(vec![FontFamily::Generic(GenericFamily::SansSerif)])
        );
    }

    #[test]
    fn parse_font_family_multiple() {
        let decls = parse_decl("font-family: \"Helvetica Neue\", Arial, sans-serif");
        if let CssValue::FontFamily(families) = &decls[0].value {
            assert_eq!(families.len(), 3);
            assert_eq!(families[0], FontFamily::Named("Helvetica Neue".into()));
            assert_eq!(families[1], FontFamily::Named("Arial".into()));
            assert_eq!(families[2], FontFamily::Generic(GenericFamily::SansSerif));
        } else {
            panic!("Expected FontFamily");
        }
    }

    #[test]
    fn parse_font_weight_bold() {
        let decls = parse_decl("font-weight: bold");
        assert_eq!(decls[0].value, CssValue::FontWeight(FontWeight::Bold));
    }

    #[test]
    fn parse_font_weight_numeric() {
        let decls = parse_decl("font-weight: 600");
        assert_eq!(
            decls[0].value,
            CssValue::FontWeight(FontWeight::Numeric(600.0))
        );
    }

    #[test]
    fn parse_opacity() {
        let decls = parse_decl("opacity: 0.5");
        assert_eq!(decls[0].value, CssValue::Number(0.5));
    }

    #[test]
    fn parse_z_index() {
        let decls = parse_decl("z-index: 10");
        assert_eq!(decls[0].value, CssValue::Integer(10));
    }

    #[test]
    fn parse_css_wide_keyword_inherit() {
        let decls = parse_decl("color: inherit");
        assert_eq!(decls[0].value, CssValue::Inherit);
    }

    #[test]
    fn parse_custom_property() {
        let decls = parse_decl("--my-color: red");
        assert_eq!(decls[0].property, PropertyId::Custom("--my-color".into()));
        assert!(matches!(decls[0].value, CssValue::CustomValue(_)));
    }

    #[test]
    fn parse_important_flag() {
        let (raw_decls, _) = parse_declaration_list("color: red !important");
        let decls = parse_property(
            raw_decls[0].name,
            &raw_decls[0].value,
            raw_decls[0].important,
        )
        .unwrap();
        assert!(decls[0].important);
    }

    #[test]
    fn parse_line_height_normal() {
        let decls = parse_decl("line-height: normal");
        assert_eq!(decls[0].value, CssValue::LineHeight(LineHeight::Normal));
    }

    #[test]
    fn parse_line_height_number() {
        let decls = parse_decl("line-height: 1.5");
        assert_eq!(
            decls[0].value,
            CssValue::LineHeight(LineHeight::Number(1.5))
        );
    }

    #[test]
    fn parse_zero_margin() {
        let decls = parse_decl("margin: 0");
        assert_eq!(decls.len(), 4);
    }
}
