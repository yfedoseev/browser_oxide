use css_values::property::{CssValue, LineHeight, PropertyId};
use css_values::types::color::Color;
use css_values::types::display::*;
use css_values::types::font::*;
use css_values::types::length::*;

/// Get the initial (default) value for any CSS property.
pub fn initial_value(property: &PropertyId) -> CssValue {
    match property {
        PropertyId::Display => CssValue::Display(Display::Inline),
        PropertyId::Position => CssValue::Position(Position::Static),
        PropertyId::Width | PropertyId::Height => {
            CssValue::LengthPercentageAuto(LengthPercentageAuto::Auto)
        }
        PropertyId::MinWidth | PropertyId::MinHeight => {
            CssValue::LengthPercentageAuto(LengthPercentageAuto::Auto)
        }
        PropertyId::MaxWidth | PropertyId::MaxHeight => {
            CssValue::LengthPercentageAuto(LengthPercentageAuto::Auto)
        }
        PropertyId::MarginTop
        | PropertyId::MarginRight
        | PropertyId::MarginBottom
        | PropertyId::MarginLeft => {
            CssValue::LengthPercentageAuto(LengthPercentageAuto::Length(Length::Zero))
        }
        PropertyId::PaddingTop
        | PropertyId::PaddingRight
        | PropertyId::PaddingBottom
        | PropertyId::PaddingLeft => {
            CssValue::LengthPercentage(LengthPercentage::Length(Length::Zero))
        }
        PropertyId::BorderTopWidth
        | PropertyId::BorderRightWidth
        | PropertyId::BorderBottomWidth
        | PropertyId::BorderLeftWidth => {
            CssValue::Length(Length::Px(3.0)) // medium
        }
        PropertyId::BoxSizing => CssValue::BoxSizing(BoxSizing::ContentBox),
        PropertyId::OverflowX | PropertyId::OverflowY => CssValue::Overflow(Overflow::Visible),
        PropertyId::Float => CssValue::Float(Float::None),
        PropertyId::Clear => CssValue::Clear(Clear::None),
        PropertyId::FlexDirection => CssValue::FlexDirection(FlexDirection::Row),
        PropertyId::FlexWrap => CssValue::FlexWrap(FlexWrap::Nowrap),
        PropertyId::FlexGrow => CssValue::Number(0.0),
        PropertyId::FlexShrink => CssValue::Number(1.0),
        PropertyId::FlexBasis => CssValue::LengthPercentageAuto(LengthPercentageAuto::Auto),
        PropertyId::AlignItems => CssValue::Alignment(AlignmentValue::Normal),
        PropertyId::AlignSelf => CssValue::Alignment(AlignmentValue::Normal),
        PropertyId::AlignContent => CssValue::Alignment(AlignmentValue::Normal),
        PropertyId::JustifyContent => CssValue::Alignment(AlignmentValue::Normal),
        PropertyId::JustifyItems => CssValue::Alignment(AlignmentValue::Normal),
        PropertyId::JustifySelf => CssValue::Alignment(AlignmentValue::Normal),
        PropertyId::Gap | PropertyId::RowGap | PropertyId::ColumnGap => {
            CssValue::LengthPercentage(LengthPercentage::Length(Length::Zero))
        }
        PropertyId::FontSize => {
            CssValue::LengthPercentage(LengthPercentage::Length(Length::Px(16.0)))
        }
        PropertyId::FontFamily => {
            CssValue::FontFamily(vec![FontFamily::Generic(GenericFamily::Serif)])
        }
        PropertyId::FontWeight => CssValue::FontWeight(FontWeight::Normal),
        PropertyId::FontStyle => CssValue::FontStyle(FontStyle::Normal),
        PropertyId::LineHeight => CssValue::LineHeight(LineHeight::Normal),
        PropertyId::TextAlign => CssValue::TextAlign(TextAlign::Start),
        PropertyId::WhiteSpace => CssValue::WhiteSpace(WhiteSpace::Normal),
        PropertyId::Color => CssValue::Color(Color::Rgba {
            r: 0,
            g: 0,
            b: 0,
            a: 1.0,
        }),
        PropertyId::BackgroundColor => CssValue::Color(Color::Transparent),
        PropertyId::Visibility => CssValue::Visibility(Visibility::Visible),
        PropertyId::Opacity => CssValue::Number(1.0),
        PropertyId::ZIndex => CssValue::Integer(0),
        PropertyId::ContentVisibility => CssValue::ContentVisibility(ContentVisibility::Visible),
        PropertyId::Transform => CssValue::Transform(vec![]),
        PropertyId::Custom(_) => CssValue::Inherit, // custom properties inherit by default
    }
}
