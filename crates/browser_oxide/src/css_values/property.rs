use crate::css_values::types::color::Color;
use crate::css_values::types::custom::VarReference;
use crate::css_values::types::display::*;
use crate::css_values::types::font::*;
use crate::css_values::types::length::*;
use crate::css_values::types::transform::TransformFunction;

/// Identifies a CSS property by name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PropertyId {
    Display,
    Position,
    Width,
    Height,
    MinWidth,
    MinHeight,
    MaxWidth,
    MaxHeight,
    MarginTop,
    MarginRight,
    MarginBottom,
    MarginLeft,
    PaddingTop,
    PaddingRight,
    PaddingBottom,
    PaddingLeft,
    BorderTopWidth,
    BorderRightWidth,
    BorderBottomWidth,
    BorderLeftWidth,
    BoxSizing,
    OverflowX,
    OverflowY,
    Float,
    Clear,
    FlexDirection,
    FlexWrap,
    FlexGrow,
    FlexShrink,
    FlexBasis,
    AlignItems,
    AlignSelf,
    AlignContent,
    JustifyContent,
    JustifyItems,
    JustifySelf,
    Gap,
    RowGap,
    ColumnGap,
    FontSize,
    FontFamily,
    FontWeight,
    FontStyle,
    LineHeight,
    TextAlign,
    WhiteSpace,
    Color,
    BackgroundColor,
    Visibility,
    Opacity,
    ZIndex,
    ContentVisibility,
    Transform,
    Custom(String),
}

impl PropertyId {
    pub fn from_name(name: &str) -> Self {
        match name.to_ascii_lowercase().as_str() {
            "display" => Self::Display,
            "position" => Self::Position,
            "width" => Self::Width,
            "height" => Self::Height,
            "min-width" => Self::MinWidth,
            "min-height" => Self::MinHeight,
            "max-width" => Self::MaxWidth,
            "max-height" => Self::MaxHeight,
            "margin-top" => Self::MarginTop,
            "margin-right" => Self::MarginRight,
            "margin-bottom" => Self::MarginBottom,
            "margin-left" => Self::MarginLeft,
            "padding-top" => Self::PaddingTop,
            "padding-right" => Self::PaddingRight,
            "padding-bottom" => Self::PaddingBottom,
            "padding-left" => Self::PaddingLeft,
            "border-top-width" => Self::BorderTopWidth,
            "border-right-width" => Self::BorderRightWidth,
            "border-bottom-width" => Self::BorderBottomWidth,
            "border-left-width" => Self::BorderLeftWidth,
            "box-sizing" => Self::BoxSizing,
            "overflow-x" => Self::OverflowX,
            "overflow-y" => Self::OverflowY,
            "float" => Self::Float,
            "clear" => Self::Clear,
            "flex-direction" => Self::FlexDirection,
            "flex-wrap" => Self::FlexWrap,
            "flex-grow" => Self::FlexGrow,
            "flex-shrink" => Self::FlexShrink,
            "flex-basis" => Self::FlexBasis,
            "align-items" => Self::AlignItems,
            "align-self" => Self::AlignSelf,
            "align-content" => Self::AlignContent,
            "justify-content" => Self::JustifyContent,
            "justify-items" => Self::JustifyItems,
            "justify-self" => Self::JustifySelf,
            "gap" => Self::Gap,
            "row-gap" => Self::RowGap,
            "column-gap" => Self::ColumnGap,
            "font-size" => Self::FontSize,
            "font-family" => Self::FontFamily,
            "font-weight" => Self::FontWeight,
            "font-style" => Self::FontStyle,
            "line-height" => Self::LineHeight,
            "text-align" => Self::TextAlign,
            "white-space" => Self::WhiteSpace,
            "color" => Self::Color,
            "background-color" => Self::BackgroundColor,
            "visibility" => Self::Visibility,
            "opacity" => Self::Opacity,
            "z-index" => Self::ZIndex,
            "content-visibility" => Self::ContentVisibility,
            "transform" => Self::Transform,
            name if name.starts_with("--") => Self::Custom(name.to_string()),
            _ => Self::Custom(name.to_string()),
        }
    }
}

/// A typed CSS property value.
#[derive(Debug, Clone, PartialEq)]
pub enum CssValue {
    // CSS-wide keywords
    Initial,
    Inherit,
    Unset,
    Revert,
    RevertLayer,

    // Display & layout
    Display(Display),
    Position(Position),
    BoxSizing(BoxSizing),
    Overflow(Overflow),
    Float(Float),
    Clear(Clear),
    Visibility(Visibility),
    ContentVisibility(ContentVisibility),

    // Sizing
    Length(Length),
    LengthPercentage(LengthPercentage),
    LengthPercentageAuto(LengthPercentageAuto),

    // Numeric
    Number(f64),
    Integer(i32),

    // Color
    Color(Color),

    // Flex
    FlexDirection(FlexDirection),
    FlexWrap(FlexWrap),

    // Alignment
    Alignment(AlignmentValue),

    // Font
    FontFamily(Vec<FontFamily>),
    FontWeight(FontWeight),
    FontStyle(FontStyle),

    // Text
    TextAlign(TextAlign),
    WhiteSpace(WhiteSpace),

    // Line height
    LineHeight(LineHeight),

    // Transform
    Transform(Vec<TransformFunction>),

    // Custom property (raw text)
    CustomValue(String),

    // Unresolved var() reference
    Var(VarReference),
}

/// Line-height can be a number, length, percentage, or `normal`.
#[derive(Debug, Clone, PartialEq)]
pub enum LineHeight {
    Normal,
    Number(f64),
    Length(Length),
    Percentage(f64),
}

/// A property declaration with its typed value.
#[derive(Debug, Clone, PartialEq)]
pub struct PropertyDeclaration {
    pub property: PropertyId,
    pub value: CssValue,
    pub important: bool,
}
