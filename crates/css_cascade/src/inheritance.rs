use css_values::property::PropertyId;

/// Returns whether a CSS property inherits by default.
pub fn is_inherited(property: &PropertyId) -> bool {
    matches!(
        property,
        PropertyId::Color
            | PropertyId::FontSize
            | PropertyId::FontFamily
            | PropertyId::FontWeight
            | PropertyId::FontStyle
            | PropertyId::LineHeight
            | PropertyId::TextAlign
            | PropertyId::WhiteSpace
            | PropertyId::Visibility
            | PropertyId::Custom(_)
    )
}
