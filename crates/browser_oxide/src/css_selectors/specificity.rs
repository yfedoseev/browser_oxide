use crate::css_selectors::ast::*;

/// Compute the specificity of a selector.
pub fn compute_specificity(selector: &Selector) -> Specificity {
    let mut spec = Specificity::default();
    for component in &selector.components {
        match component {
            Component::Combinator(_) => {}
            Component::Simple(simple) => {
                spec += simple_specificity(simple);
            }
        }
    }
    spec
}

pub(crate) fn simple_specificity_pub(simple: &SimpleSelector) -> Specificity {
    simple_specificity(simple)
}

fn simple_specificity(simple: &SimpleSelector) -> Specificity {
    match simple {
        SimpleSelector::Id(_) => Specificity::new(1, 0, 0),
        SimpleSelector::Class(_) => Specificity::new(0, 1, 0),
        SimpleSelector::Attribute { .. } => Specificity::new(0, 1, 0),
        SimpleSelector::Type(_) => Specificity::new(0, 0, 1),
        SimpleSelector::Universal => Specificity::default(),
        SimpleSelector::Nesting => Specificity::default(),
        SimpleSelector::PseudoClass(pc) => pseudo_class_specificity(pc),
        SimpleSelector::PseudoElement(pe) => pseudo_element_specificity(pe),
    }
}

fn pseudo_class_specificity(pc: &PseudoClass) -> Specificity {
    match pc {
        // :where() contributes zero specificity
        PseudoClass::Where(_) => Specificity::default(),

        // :is(), :not(), :has() contribute the max specificity of their arguments
        PseudoClass::Is(list) | PseudoClass::Not(list) => list
            .iter()
            .map(|s| s.specificity())
            .fold(Specificity::default(), Specificity::max),
        PseudoClass::Has(relatives) => relatives
            .iter()
            .map(|r| r.selector.specificity())
            .fold(Specificity::default(), Specificity::max),

        // :nth-child(An+B of S) — the An+B part contributes (0,1,0), plus max of S
        PseudoClass::NthChild(_, Some(list)) | PseudoClass::NthLastChild(_, Some(list)) => {
            let s_spec = list
                .iter()
                .map(|s| s.specificity())
                .fold(Specificity::default(), Specificity::max);
            Specificity::new(0, 1, 0) + s_spec
        }

        // All other pseudo-classes contribute (0,1,0)
        _ => Specificity::new(0, 1, 0),
    }
}

fn pseudo_element_specificity(pe: &PseudoElement) -> Specificity {
    match pe {
        PseudoElement::Slotted(inner) => Specificity::new(0, 0, 1) + inner.specificity(),
        _ => Specificity::new(0, 0, 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sel(components: Vec<Component>) -> Selector {
        let spec = compute_specificity_from_components(&components);
        Selector::new(components, spec)
    }

    fn compute_specificity_from_components(components: &[Component]) -> Specificity {
        let mut spec = Specificity::default();
        for c in components {
            if let Component::Simple(s) = c {
                spec += simple_specificity(s);
            }
        }
        spec
    }

    #[test]
    fn id_selector() {
        let s = sel(vec![Component::Simple(SimpleSelector::Id("foo".into()))]);
        assert_eq!(s.specificity(), Specificity::new(1, 0, 0));
    }

    #[test]
    fn class_selector() {
        let s = sel(vec![Component::Simple(SimpleSelector::Class("foo".into()))]);
        assert_eq!(s.specificity(), Specificity::new(0, 1, 0));
    }

    #[test]
    fn type_selector() {
        let s = sel(vec![Component::Simple(SimpleSelector::Type("div".into()))]);
        assert_eq!(s.specificity(), Specificity::new(0, 0, 1));
    }

    #[test]
    fn universal_selector() {
        let s = sel(vec![Component::Simple(SimpleSelector::Universal)]);
        assert_eq!(s.specificity(), Specificity::new(0, 0, 0));
    }

    #[test]
    fn compound_selector() {
        // div#main.content → (1, 1, 1)
        let s = sel(vec![
            Component::Simple(SimpleSelector::Type("div".into())),
            Component::Simple(SimpleSelector::Id("main".into())),
            Component::Simple(SimpleSelector::Class("content".into())),
        ]);
        assert_eq!(s.specificity(), Specificity::new(1, 1, 1));
    }

    #[test]
    fn attribute_selector() {
        let s = sel(vec![Component::Simple(SimpleSelector::Attribute {
            name: "href".into(),
            operator: None,
            value: None,
            case_sensitivity: CaseSensitivity::Default,
        })]);
        assert_eq!(s.specificity(), Specificity::new(0, 1, 0));
    }

    #[test]
    fn where_zero_specificity() {
        let inner = sel(vec![Component::Simple(SimpleSelector::Id("foo".into()))]);
        let s = sel(vec![Component::Simple(SimpleSelector::PseudoClass(
            PseudoClass::Where(vec![inner]),
        ))]);
        assert_eq!(s.specificity(), Specificity::new(0, 0, 0));
    }

    #[test]
    fn pseudo_element() {
        let s = sel(vec![Component::Simple(SimpleSelector::PseudoElement(
            PseudoElement::Before,
        ))]);
        assert_eq!(s.specificity(), Specificity::new(0, 0, 1));
    }
}
