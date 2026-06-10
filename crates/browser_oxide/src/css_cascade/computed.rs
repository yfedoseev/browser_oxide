use crate::css_cascade::inheritance::is_inherited;
use crate::css_cascade::initial::initial_value;
use crate::css_values::property::{CssValue, PropertyId};
use std::collections::HashMap;

/// Computed style for a single element.
#[derive(Debug, Clone, Default)]
pub struct ComputedStyle {
    properties: HashMap<PropertyId, CssValue>,
}

impl ComputedStyle {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a computed property value.
    pub fn set(&mut self, property: PropertyId, value: CssValue) {
        self.properties.insert(property, value);
    }

    /// Get a computed property value.
    pub fn get(&self, property: &PropertyId) -> Option<&CssValue> {
        self.properties.get(property)
    }

    /// Get a computed property value, falling back to initial.
    pub fn get_or_initial(&self, property: &PropertyId) -> CssValue {
        self.properties
            .get(property)
            .cloned()
            .unwrap_or_else(|| initial_value(property))
    }

    /// Resolve cascaded values with inheritance from parent.
    pub fn resolve(
        cascaded: &HashMap<PropertyId, CssValue>,
        parent: Option<&ComputedStyle>,
    ) -> Self {
        let mut style = ComputedStyle::new();

        // All known properties
        let all_properties = all_property_ids();

        for prop in &all_properties {
            let value = if let Some(cascaded_value) = cascaded.get(prop) {
                match cascaded_value {
                    CssValue::Inherit => parent
                        .and_then(|p| p.get(prop))
                        .cloned()
                        .unwrap_or_else(|| initial_value(prop)),
                    CssValue::Initial => initial_value(prop),
                    CssValue::Unset => {
                        if is_inherited(prop) {
                            parent
                                .and_then(|p| p.get(prop))
                                .cloned()
                                .unwrap_or_else(|| initial_value(prop))
                        } else {
                            initial_value(prop)
                        }
                    }
                    CssValue::Revert | CssValue::RevertLayer => {
                        // Simplified: treat as unset
                        if is_inherited(prop) {
                            parent
                                .and_then(|p| p.get(prop))
                                .cloned()
                                .unwrap_or_else(|| initial_value(prop))
                        } else {
                            initial_value(prop)
                        }
                    }
                    other => other.clone(),
                }
            } else if is_inherited(prop) {
                // No cascaded value, property inherits → use parent
                parent
                    .and_then(|p| p.get(prop))
                    .cloned()
                    .unwrap_or_else(|| initial_value(prop))
            } else {
                // No cascaded value, doesn't inherit → use initial
                initial_value(prop)
            };

            style.set(prop.clone(), value);
        }

        style
    }
}

fn all_property_ids() -> Vec<PropertyId> {
    vec![
        PropertyId::Display,
        PropertyId::Position,
        PropertyId::Width,
        PropertyId::Height,
        PropertyId::MinWidth,
        PropertyId::MinHeight,
        PropertyId::MaxWidth,
        PropertyId::MaxHeight,
        PropertyId::MarginTop,
        PropertyId::MarginRight,
        PropertyId::MarginBottom,
        PropertyId::MarginLeft,
        PropertyId::PaddingTop,
        PropertyId::PaddingRight,
        PropertyId::PaddingBottom,
        PropertyId::PaddingLeft,
        PropertyId::BorderTopWidth,
        PropertyId::BorderRightWidth,
        PropertyId::BorderBottomWidth,
        PropertyId::BorderLeftWidth,
        PropertyId::BoxSizing,
        PropertyId::OverflowX,
        PropertyId::OverflowY,
        PropertyId::Float,
        PropertyId::Clear,
        PropertyId::FlexDirection,
        PropertyId::FlexWrap,
        PropertyId::FlexGrow,
        PropertyId::FlexShrink,
        PropertyId::FlexBasis,
        PropertyId::AlignItems,
        PropertyId::AlignSelf,
        PropertyId::AlignContent,
        PropertyId::JustifyContent,
        PropertyId::JustifyItems,
        PropertyId::JustifySelf,
        PropertyId::Gap,
        PropertyId::RowGap,
        PropertyId::ColumnGap,
        PropertyId::FontSize,
        PropertyId::FontFamily,
        PropertyId::FontWeight,
        PropertyId::FontStyle,
        PropertyId::LineHeight,
        PropertyId::TextAlign,
        PropertyId::WhiteSpace,
        PropertyId::Color,
        PropertyId::BackgroundColor,
        PropertyId::Visibility,
        PropertyId::Opacity,
        PropertyId::ZIndex,
        PropertyId::ContentVisibility,
        PropertyId::Transform,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::css_values::types::color::Color;
    use crate::css_values::types::display::Display;

    #[test]
    fn initial_values_default() {
        let style = ComputedStyle::resolve(&HashMap::new(), None);
        assert_eq!(
            style.get_or_initial(&PropertyId::Display),
            CssValue::Display(Display::Inline)
        );
        assert_eq!(
            style.get_or_initial(&PropertyId::Opacity),
            CssValue::Number(1.0)
        );
    }

    #[test]
    fn inheritance_works() {
        let mut parent = ComputedStyle::new();
        parent.set(
            PropertyId::Color,
            CssValue::Color(Color::Rgba {
                r: 255,
                g: 0,
                b: 0,
                a: 1.0,
            }),
        );

        let child = ComputedStyle::resolve(&HashMap::new(), Some(&parent));
        // Color inherits
        assert_eq!(
            child.get(&PropertyId::Color),
            Some(&CssValue::Color(Color::Rgba {
                r: 255,
                g: 0,
                b: 0,
                a: 1.0
            }))
        );
    }

    #[test]
    fn non_inherited_uses_initial() {
        let mut parent = ComputedStyle::new();
        parent.set(PropertyId::Display, CssValue::Display(Display::Flex));

        let child = ComputedStyle::resolve(&HashMap::new(), Some(&parent));
        // Display does NOT inherit
        assert_eq!(
            child.get(&PropertyId::Display),
            Some(&CssValue::Display(Display::Inline))
        );
    }

    #[test]
    fn explicit_inherit_keyword() {
        let mut parent = ComputedStyle::new();
        parent.set(PropertyId::Display, CssValue::Display(Display::Flex));

        let mut cascaded = HashMap::new();
        cascaded.insert(PropertyId::Display, CssValue::Inherit);

        let child = ComputedStyle::resolve(&cascaded, Some(&parent));
        assert_eq!(
            child.get(&PropertyId::Display),
            Some(&CssValue::Display(Display::Flex))
        );
    }

    #[test]
    fn explicit_initial_keyword() {
        let mut cascaded = HashMap::new();
        cascaded.insert(PropertyId::Color, CssValue::Initial);

        let style = ComputedStyle::resolve(&cascaded, None);
        assert_eq!(
            style.get(&PropertyId::Color),
            Some(&CssValue::Color(Color::Rgba {
                r: 0,
                g: 0,
                b: 0,
                a: 1.0
            }))
        );
    }
}
