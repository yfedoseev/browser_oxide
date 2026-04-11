use crate::layers::LayerId;
use css_selectors::Specificity;
use css_values::property::{CssValue, PropertyDeclaration, PropertyId};

/// Cascade origin (§6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Origin {
    UserAgent = 0,
    User = 1,
    Author = 2,
}

/// A declaration annotated with cascade metadata.
#[derive(Debug, Clone)]
pub struct CascadeEntry {
    pub declaration: PropertyDeclaration,
    pub origin: Origin,
    pub layer: Option<LayerId>,
    pub specificity: Specificity,
    pub source_order: u32,
}

/// Sort declarations by cascade precedence and return winning value per property.
pub fn cascade_sort(
    entries: &mut [CascadeEntry],
) -> std::collections::HashMap<PropertyId, CssValue> {
    // Sort by cascade precedence (ascending — last wins)
    entries.sort_by(|a, b| cascade_compare(a, b));

    // Last entry per property wins
    let mut result = std::collections::HashMap::new();
    for entry in entries.iter() {
        result.insert(
            entry.declaration.property.clone(),
            entry.declaration.value.clone(),
        );
    }
    result
}

fn cascade_compare(a: &CascadeEntry, b: &CascadeEntry) -> std::cmp::Ordering {
    let a_important = a.declaration.important;
    let b_important = b.declaration.important;

    // 1. Origin + importance
    let a_priority = origin_priority(a.origin, a_important);
    let b_priority = origin_priority(b.origin, b_important);
    let ord = a_priority.cmp(&b_priority);
    if ord != std::cmp::Ordering::Equal {
        return ord;
    }

    // 2. Layer ordering (within same origin+importance)
    // For normal declarations: later layer wins (higher precedence)
    // For !important: earlier layer wins (reversed)
    // Unlayered beats layered for normal; reversed for important
    match (&a.layer, &b.layer) {
        (None, Some(_)) if !a_important => return std::cmp::Ordering::Greater, // unlayered wins normal
        (Some(_), None) if !a_important => return std::cmp::Ordering::Less,
        (None, Some(_)) if a_important => return std::cmp::Ordering::Less, // unlayered loses important
        (Some(_), None) if a_important => return std::cmp::Ordering::Greater,
        (Some(la), Some(lb)) => {
            let layer_ord = la.cmp(lb);
            if layer_ord != std::cmp::Ordering::Equal {
                return if a_important {
                    layer_ord.reverse() // important: earlier layer wins
                } else {
                    layer_ord // normal: later layer wins
                };
            }
        }
        _ => {}
    }

    // 3. Specificity
    let spec_ord = a.specificity.cmp(&b.specificity);
    if spec_ord != std::cmp::Ordering::Equal {
        return spec_ord;
    }

    // 4. Source order
    a.source_order.cmp(&b.source_order)
}

fn origin_priority(origin: Origin, important: bool) -> u8 {
    // Normal: UA(1) < User(2) < Author(3)
    // Important: Author(4) < User(5) < UA(6)
    if important {
        match origin {
            Origin::Author => 4,
            Origin::User => 5,
            Origin::UserAgent => 6,
        }
    } else {
        match origin {
            Origin::UserAgent => 1,
            Origin::User => 2,
            Origin::Author => 3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use css_values::types::color::Color;

    fn entry(
        origin: Origin,
        specificity: (u32, u32, u32),
        order: u32,
        important: bool,
        value: CssValue,
    ) -> CascadeEntry {
        CascadeEntry {
            declaration: PropertyDeclaration {
                property: PropertyId::Color,
                value,
                important,
            },
            origin,
            layer: None,
            specificity: Specificity::new(specificity.0, specificity.1, specificity.2),
            source_order: order,
        }
    }

    #[test]
    fn higher_specificity_wins() {
        let mut entries = vec![
            entry(
                Origin::Author,
                (0, 1, 0),
                0,
                false,
                CssValue::Color(Color::Rgba {
                    r: 255,
                    g: 0,
                    b: 0,
                    a: 1.0,
                }),
            ),
            entry(
                Origin::Author,
                (1, 0, 0),
                1,
                false,
                CssValue::Color(Color::Rgba {
                    r: 0,
                    g: 0,
                    b: 255,
                    a: 1.0,
                }),
            ),
        ];
        let result = cascade_sort(&mut entries);
        // ID selector (1,0,0) should win over class (0,1,0)
        assert!(matches!(
            result.get(&PropertyId::Color),
            Some(CssValue::Color(Color::Rgba {
                r: 0,
                g: 0,
                b: 255,
                ..
            }))
        ));
    }

    #[test]
    fn source_order_wins_same_specificity() {
        let mut entries = vec![
            entry(
                Origin::Author,
                (0, 1, 0),
                0,
                false,
                CssValue::Color(Color::Rgba {
                    r: 255,
                    g: 0,
                    b: 0,
                    a: 1.0,
                }),
            ),
            entry(
                Origin::Author,
                (0, 1, 0),
                1,
                false,
                CssValue::Color(Color::Rgba {
                    r: 0,
                    g: 255,
                    b: 0,
                    a: 1.0,
                }),
            ),
        ];
        let result = cascade_sort(&mut entries);
        // Later source order wins
        assert!(matches!(
            result.get(&PropertyId::Color),
            Some(CssValue::Color(Color::Rgba {
                r: 0,
                g: 255,
                b: 0,
                ..
            }))
        ));
    }

    #[test]
    fn important_beats_normal() {
        let mut entries = vec![
            entry(
                Origin::Author,
                (1, 0, 0),
                1,
                false,
                CssValue::Color(Color::Rgba {
                    r: 255,
                    g: 0,
                    b: 0,
                    a: 1.0,
                }),
            ),
            entry(
                Origin::Author,
                (0, 0, 1),
                0,
                true,
                CssValue::Color(Color::Rgba {
                    r: 0,
                    g: 0,
                    b: 255,
                    a: 1.0,
                }),
            ),
        ];
        let result = cascade_sort(&mut entries);
        // !important wins even with lower specificity
        assert!(matches!(
            result.get(&PropertyId::Color),
            Some(CssValue::Color(Color::Rgba {
                r: 0,
                g: 0,
                b: 255,
                ..
            }))
        ));
    }

    #[test]
    fn author_beats_ua() {
        let mut entries = vec![
            entry(
                Origin::UserAgent,
                (0, 0, 1),
                0,
                false,
                CssValue::Color(Color::Rgba {
                    r: 255,
                    g: 0,
                    b: 0,
                    a: 1.0,
                }),
            ),
            entry(
                Origin::Author,
                (0, 0, 1),
                1,
                false,
                CssValue::Color(Color::Rgba {
                    r: 0,
                    g: 255,
                    b: 0,
                    a: 1.0,
                }),
            ),
        ];
        let result = cascade_sort(&mut entries);
        assert!(matches!(
            result.get(&PropertyId::Color),
            Some(CssValue::Color(Color::Rgba {
                r: 0,
                g: 255,
                b: 0,
                ..
            }))
        ));
    }
}
