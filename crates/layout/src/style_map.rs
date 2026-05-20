use crate::resolve::{resolve_length, resolve_length_percentage, ResolveContext};
use css_cascade::ComputedStyle;
use css_values::property::{CssValue, PropertyId};

/// Convert a ComputedStyle into a taffy::Style.
pub fn computed_to_taffy(style: &ComputedStyle, ctx: &ResolveContext) -> taffy::Style {
    let mut ts = taffy::Style::default();

    // Display
    if let Some(CssValue::Display(d)) = style.get(&PropertyId::Display) {
        use css_values::types::display::Display;
        ts.display = match d {
            Display::None => taffy::Display::None,
            Display::Flex | Display::InlineFlex => taffy::Display::Flex,
            Display::Grid | Display::InlineGrid => taffy::Display::Grid,
            _ => taffy::Display::Block,
        };
    }

    // Position
    if let Some(CssValue::Position(p)) = style.get(&PropertyId::Position) {
        use css_values::types::display::Position;
        ts.position = match p {
            Position::Relative => taffy::Position::Relative,
            Position::Absolute | Position::Fixed => taffy::Position::Absolute,
            _ => taffy::Position::Relative,
        };
    }

    // Width / Height
    ts.size.width = css_to_dimension(style, &PropertyId::Width, ctx);
    ts.size.height = css_to_dimension(style, &PropertyId::Height, ctx);
    ts.min_size.width = css_to_dimension(style, &PropertyId::MinWidth, ctx);
    ts.min_size.height = css_to_dimension(style, &PropertyId::MinHeight, ctx);
    ts.max_size.width = css_to_dimension(style, &PropertyId::MaxWidth, ctx);
    ts.max_size.height = css_to_dimension(style, &PropertyId::MaxHeight, ctx);

    // Margin
    ts.margin.top = css_to_lpa(style, &PropertyId::MarginTop, ctx);
    ts.margin.right = css_to_lpa(style, &PropertyId::MarginRight, ctx);
    ts.margin.bottom = css_to_lpa(style, &PropertyId::MarginBottom, ctx);
    ts.margin.left = css_to_lpa(style, &PropertyId::MarginLeft, ctx);

    // Padding
    ts.padding.top = css_to_lp(style, &PropertyId::PaddingTop, ctx);
    ts.padding.right = css_to_lp(style, &PropertyId::PaddingRight, ctx);
    ts.padding.bottom = css_to_lp(style, &PropertyId::PaddingBottom, ctx);
    ts.padding.left = css_to_lp(style, &PropertyId::PaddingLeft, ctx);

    // Border
    ts.border.top = css_to_border(style, &PropertyId::BorderTopWidth, ctx);
    ts.border.right = css_to_border(style, &PropertyId::BorderRightWidth, ctx);
    ts.border.bottom = css_to_border(style, &PropertyId::BorderBottomWidth, ctx);
    ts.border.left = css_to_border(style, &PropertyId::BorderLeftWidth, ctx);

    // Flex
    if let Some(CssValue::FlexDirection(fd)) = style.get(&PropertyId::FlexDirection) {
        use css_values::types::display::FlexDirection;
        ts.flex_direction = match fd {
            FlexDirection::Row => taffy::FlexDirection::Row,
            FlexDirection::RowReverse => taffy::FlexDirection::RowReverse,
            FlexDirection::Column => taffy::FlexDirection::Column,
            FlexDirection::ColumnReverse => taffy::FlexDirection::ColumnReverse,
        };
    }
    if let Some(CssValue::FlexWrap(fw)) = style.get(&PropertyId::FlexWrap) {
        use css_values::types::display::FlexWrap;
        ts.flex_wrap = match fw {
            FlexWrap::Nowrap => taffy::FlexWrap::NoWrap,
            FlexWrap::Wrap => taffy::FlexWrap::Wrap,
            FlexWrap::WrapReverse => taffy::FlexWrap::WrapReverse,
        };
    }
    if let Some(CssValue::Number(v)) = style.get(&PropertyId::FlexGrow) {
        ts.flex_grow = *v as f32;
    }
    if let Some(CssValue::Number(v)) = style.get(&PropertyId::FlexShrink) {
        ts.flex_shrink = *v as f32;
    }

    // Gap
    if let Some(CssValue::LengthPercentage(lp)) = style.get(&PropertyId::RowGap) {
        ts.gap.height =
            taffy::LengthPercentage::length(resolve_length_percentage(lp, ctx, ctx.viewport_h));
    }
    if let Some(CssValue::LengthPercentage(lp)) = style.get(&PropertyId::ColumnGap) {
        ts.gap.width =
            taffy::LengthPercentage::length(resolve_length_percentage(lp, ctx, ctx.viewport_w));
    }

    // Box sizing
    if let Some(CssValue::BoxSizing(bs)) = style.get(&PropertyId::BoxSizing) {
        use css_values::types::display::BoxSizing;
        ts.box_sizing = match bs {
            BoxSizing::ContentBox => taffy::BoxSizing::ContentBox,
            BoxSizing::BorderBox => taffy::BoxSizing::BorderBox,
        };
    }

    ts
}

fn css_to_dimension(
    style: &ComputedStyle,
    prop: &PropertyId,
    ctx: &ResolveContext,
) -> taffy::Dimension {
    match style.get(prop) {
        Some(CssValue::LengthPercentageAuto(lpa)) => {
            use css_values::types::length::LengthPercentageAuto as CssLPA;
            match lpa {
                CssLPA::Length(l) => taffy::Dimension::length(resolve_length(l, ctx)),
                CssLPA::Percentage(p) => taffy::Dimension::percent(*p as f32 / 100.0),
                CssLPA::Auto | CssLPA::Calc(_) => taffy::Dimension::auto(),
            }
        }
        Some(CssValue::LengthPercentage(lp)) => {
            use css_values::types::length::LengthPercentage as CssLP;
            match lp {
                CssLP::Length(l) => taffy::Dimension::length(resolve_length(l, ctx)),
                CssLP::Percentage(p) => taffy::Dimension::percent(*p as f32 / 100.0),
                CssLP::Calc(_) => taffy::Dimension::auto(),
            }
        }
        Some(CssValue::Length(l)) => taffy::Dimension::length(resolve_length(l, ctx)),
        _ => taffy::Dimension::auto(),
    }
}

fn css_to_lpa(
    style: &ComputedStyle,
    prop: &PropertyId,
    ctx: &ResolveContext,
) -> taffy::LengthPercentageAuto {
    use css_values::types::length::LengthPercentageAuto as CssLPA;
    match style.get(prop) {
        Some(CssValue::LengthPercentageAuto(lpa)) => match lpa {
            CssLPA::Length(l) => taffy::LengthPercentageAuto::length(resolve_length(l, ctx)),
            CssLPA::Percentage(p) => taffy::LengthPercentageAuto::percent(*p as f32 / 100.0),
            CssLPA::Auto | CssLPA::Calc(_) => taffy::LengthPercentageAuto::auto(),
        },
        _ => taffy::LengthPercentageAuto::length(0.0),
    }
}

fn css_to_lp(
    style: &ComputedStyle,
    prop: &PropertyId,
    ctx: &ResolveContext,
) -> taffy::LengthPercentage {
    use css_values::types::length::LengthPercentage as CssLP;
    match style.get(prop) {
        Some(CssValue::LengthPercentage(lp)) => match lp {
            CssLP::Length(l) => taffy::LengthPercentage::length(resolve_length(l, ctx)),
            CssLP::Percentage(p) => taffy::LengthPercentage::percent(*p as f32 / 100.0),
            CssLP::Calc(_) => taffy::LengthPercentage::length(0.0),
        },
        _ => taffy::LengthPercentage::length(0.0),
    }
}

fn css_to_border(
    style: &ComputedStyle,
    prop: &PropertyId,
    ctx: &ResolveContext,
) -> taffy::LengthPercentage {
    match style.get(prop) {
        Some(CssValue::Length(l)) => taffy::LengthPercentage::length(resolve_length(l, ctx)),
        _ => taffy::LengthPercentage::length(0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::ResolveContext;
    use css_values::types::display::Display;
    use css_values::types::length::{Length, LengthPercentageAuto};
    use std::collections::HashMap;

    #[test]
    fn default_style_maps() {
        let style = ComputedStyle::resolve(&HashMap::new(), None);
        let ctx = ResolveContext::default();
        let ts = computed_to_taffy(&style, &ctx);
        assert!(matches!(ts.display, taffy::Display::Block));
    }

    #[test]
    fn flex_display_maps() {
        let mut cascaded = HashMap::new();
        cascaded.insert(PropertyId::Display, CssValue::Display(Display::Flex));
        let style = ComputedStyle::resolve(&cascaded, None);
        let ctx = ResolveContext::default();
        let ts = computed_to_taffy(&style, &ctx);
        assert!(matches!(ts.display, taffy::Display::Flex));
    }

    #[test]
    fn width_px_maps() {
        let mut cascaded = HashMap::new();
        cascaded.insert(
            PropertyId::Width,
            CssValue::LengthPercentageAuto(LengthPercentageAuto::Length(Length::Px(200.0))),
        );
        let style = ComputedStyle::resolve(&cascaded, None);
        let ctx = ResolveContext::default();
        let ts = computed_to_taffy(&style, &ctx);
        // taffy 0.8 made Dimension a newtype struct around CompactLength;
        // there are no pattern-matchable variants any more. Compare via
        // equality against the canonical constructor.
        assert_eq!(ts.size.width, taffy::Dimension::length(200.0));
    }

    #[test]
    fn margin_auto_maps() {
        let mut cascaded = HashMap::new();
        cascaded.insert(
            PropertyId::MarginLeft,
            CssValue::LengthPercentageAuto(LengthPercentageAuto::Auto),
        );
        let style = ComputedStyle::resolve(&cascaded, None);
        let ctx = ResolveContext::default();
        let ts = computed_to_taffy(&style, &ctx);
        assert_eq!(ts.margin.left, taffy::LengthPercentageAuto::auto());
    }
}
