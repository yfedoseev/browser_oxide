use crate::ast::*;
use crate::element::Element;
use crate::parser::parse_selector_list;

/// Check if an element matches a selector.
pub fn matches_selector<E: Element>(element: &E, selector: &Selector) -> bool {
    // Components are stored right-to-left.
    // Walk forward = matching from rightmost (the subject) toward the root.
    let components = selector.components();
    match_components(element, components, 0)
}

/// Query the first matching element (depth-first pre-order).
pub fn query_selector<E: Element>(
    root: &E,
    selector_str: &str,
) -> Result<Option<E>, crate::error::SelectorParseError> {
    let selectors = parse_selector_list(selector_str)?;
    Ok(find_first(root, &selectors))
}

/// Query all matching elements (depth-first pre-order).
pub fn query_selector_all<E: Element>(
    root: &E,
    selector_str: &str,
) -> Result<Vec<E>, crate::error::SelectorParseError> {
    let selectors = parse_selector_list(selector_str)?;
    let mut results = Vec::new();
    find_all(root, &selectors, &mut results);
    Ok(results)
}

fn find_first<E: Element>(root: &E, selectors: &SelectorList) -> Option<E> {
    // Check children recursively
    for child in root.child_elements() {
        if matches_any(&child, selectors) {
            return Some(child);
        }
        if let Some(found) = find_first(&child, selectors) {
            return Some(found);
        }
    }
    None
}

fn find_all<E: Element>(root: &E, selectors: &SelectorList, results: &mut Vec<E>) {
    for child in root.child_elements() {
        if matches_any(&child, selectors) {
            results.push(child.clone());
        }
        find_all(&child, selectors, results);
    }
}

/// Check if an element matches any selector in the list.
pub fn matches_any<E: Element>(element: &E, selectors: &SelectorList) -> bool {
    selectors.iter().any(|s| matches_selector(element, s))
}

/// Core matching: walk the components array (right-to-left order).
fn match_components<E: Element>(element: &E, components: &[Component], pos: usize) -> bool {
    // Collect the compound selector at current position
    let mut i = pos;

    // Match all simple selectors in the current compound
    while i < components.len() {
        match &components[i] {
            Component::Combinator(_) => break,
            Component::Simple(simple) => {
                if !matches_simple(element, simple) {
                    return false;
                }
                i += 1;
            }
        }
    }

    // If we consumed all components, it's a match
    if i >= components.len() {
        return true;
    }

    // Next must be a combinator
    let combinator = match &components[i] {
        Component::Combinator(c) => *c,
        _ => return false,
    };
    i += 1; // skip combinator

    // Match the rest against the appropriate relative element
    match combinator {
        Combinator::Child => {
            if let Some(parent) = element.parent_element() {
                match_components(&parent, components, i)
            } else {
                false
            }
        }
        Combinator::Descendant => {
            let mut ancestor = element.parent_element();
            while let Some(anc) = ancestor {
                if match_components(&anc, components, i) {
                    return true;
                }
                ancestor = anc.parent_element();
            }
            false
        }
        Combinator::NextSibling => {
            if let Some(prev) = element.prev_sibling_element() {
                match_components(&prev, components, i)
            } else {
                false
            }
        }
        Combinator::SubsequentSibling => {
            let mut prev = element.prev_sibling_element();
            while let Some(sib) = prev {
                if match_components(&sib, components, i) {
                    return true;
                }
                prev = sib.prev_sibling_element();
            }
            false
        }
    }
}

fn matches_simple<E: Element>(element: &E, simple: &SimpleSelector) -> bool {
    match simple {
        SimpleSelector::Type(name) => element.local_name().eq_ignore_ascii_case(name),
        SimpleSelector::Universal => true,
        SimpleSelector::Id(id) => element.id().is_some_and(|eid| eid == id),
        SimpleSelector::Class(class) => element.has_class(class),
        SimpleSelector::Attribute {
            name,
            operator,
            value,
            case_sensitivity,
        } => match_attribute(element, name, operator, value, case_sensitivity),
        SimpleSelector::PseudoClass(pc) => matches_pseudo_class(element, pc),
        SimpleSelector::PseudoElement(_) => {
            // Pseudo-elements don't affect element matching in querySelectorAll
            true
        }
        SimpleSelector::Nesting => {
            // `&` in matching context: depends on outer context.
            // For standalone matching, treat as universal.
            true
        }
    }
}

fn match_attribute<E: Element>(
    element: &E,
    name: &str,
    operator: &Option<AttributeOperator>,
    value: &Option<String>,
    case_sensitivity: &CaseSensitivity,
) -> bool {
    match operator {
        None => element.has_attribute(name),
        Some(op) => {
            let attr_val = match element.attribute_value(name) {
                Some(v) => v,
                None => return false,
            };
            let expected = match value {
                Some(v) => v.as_str(),
                None => return false,
            };

            let ci = matches!(case_sensitivity, CaseSensitivity::CaseInsensitive);

            match op {
                AttributeOperator::Exact => str_eq(attr_val, expected, ci),
                AttributeOperator::Includes => attr_val
                    .split_whitespace()
                    .any(|word| str_eq(word, expected, ci)),
                AttributeOperator::DashMatch => {
                    str_eq(attr_val, expected, ci)
                        || (if ci {
                            attr_val
                                .to_ascii_lowercase()
                                .starts_with(&format!("{}-", expected.to_ascii_lowercase()))
                        } else {
                            attr_val.starts_with(&format!("{}-", expected))
                        })
                }
                AttributeOperator::Prefix => {
                    if ci {
                        attr_val
                            .to_ascii_lowercase()
                            .starts_with(&expected.to_ascii_lowercase())
                    } else {
                        attr_val.starts_with(expected)
                    }
                }
                AttributeOperator::Suffix => {
                    if ci {
                        attr_val
                            .to_ascii_lowercase()
                            .ends_with(&expected.to_ascii_lowercase())
                    } else {
                        attr_val.ends_with(expected)
                    }
                }
                AttributeOperator::Substring => {
                    if ci {
                        attr_val
                            .to_ascii_lowercase()
                            .contains(&expected.to_ascii_lowercase())
                    } else {
                        attr_val.contains(expected)
                    }
                }
            }
        }
    }
}

fn str_eq(a: &str, b: &str, case_insensitive: bool) -> bool {
    if case_insensitive {
        a.eq_ignore_ascii_case(b)
    } else {
        a == b
    }
}

fn matches_pseudo_class<E: Element>(element: &E, pc: &PseudoClass) -> bool {
    match pc {
        PseudoClass::Hover => element.is_hover(),
        PseudoClass::Active => element.is_active(),
        PseudoClass::Focus => element.is_focus(),
        PseudoClass::FocusWithin => element.is_focus_within(),
        PseudoClass::FocusVisible => element.is_focus_visible(),
        PseudoClass::Link => element.is_link(),
        PseudoClass::Visited => element.is_visited(),
        PseudoClass::AnyLink => element.is_any_link(),
        PseudoClass::Target => element.is_target(),
        PseudoClass::Enabled => element.is_enabled(),
        PseudoClass::Disabled => element.is_disabled(),
        PseudoClass::Checked => element.is_checked(),
        PseudoClass::Default => element.is_default(),
        PseudoClass::Indeterminate => element.is_indeterminate(),
        PseudoClass::Required => element.is_required(),
        PseudoClass::Optional => element.is_optional(),
        PseudoClass::Valid => element.is_valid(),
        PseudoClass::Invalid => element.is_invalid(),
        PseudoClass::InRange => element.is_in_range(),
        PseudoClass::OutOfRange => element.is_out_of_range(),
        PseudoClass::ReadWrite => element.is_read_write(),
        PseudoClass::ReadOnly => element.is_read_only(),
        PseudoClass::PlaceholderShown => element.is_placeholder_shown(),

        PseudoClass::Root => element.is_root(),
        PseudoClass::Empty => element.is_empty(),

        PseudoClass::FirstChild => element.sibling_index() == 1,
        PseudoClass::LastChild => element.sibling_index_from_end() == 1,
        PseudoClass::OnlyChild => {
            element.sibling_index() == 1 && element.sibling_index_from_end() == 1
        }

        PseudoClass::FirstOfType => element.sibling_type_index() == 1,
        PseudoClass::LastOfType => element.sibling_type_index_from_end() == 1,
        PseudoClass::OnlyOfType => element.sibling_type_count() == 1,

        PseudoClass::NthChild(nth, of_sel) => {
            let index = match of_sel {
                None => element.sibling_index(),
                Some(sel_list) => nth_of_index(element, sel_list, false),
            };
            nth.matches(index)
        }
        PseudoClass::NthLastChild(nth, of_sel) => {
            let index = match of_sel {
                None => element.sibling_index_from_end(),
                Some(sel_list) => nth_of_index(element, sel_list, true),
            };
            nth.matches(index)
        }
        PseudoClass::NthOfType(nth) => nth.matches(element.sibling_type_index()),
        PseudoClass::NthLastOfType(nth) => nth.matches(element.sibling_type_index_from_end()),

        PseudoClass::Lang(langs) => {
            if let Some(el_lang) = element.lang() {
                langs.iter().any(|l| {
                    el_lang.eq_ignore_ascii_case(l)
                        || el_lang
                            .to_ascii_lowercase()
                            .starts_with(&format!("{}-", l.to_ascii_lowercase()))
                })
            } else {
                false
            }
        }

        PseudoClass::Is(list) | PseudoClass::Where(list) => matches_any(element, list),
        PseudoClass::Not(list) => !matches_any(element, list),

        PseudoClass::Has(relatives) => {
            for rel in relatives {
                // Check descendants
                if has_matching_descendant(element, &rel.selector) {
                    return true;
                }
            }
            false
        }
    }
}

fn nth_of_index<E: Element>(element: &E, sel_list: &SelectorList, from_end: bool) -> i32 {
    let mut index = 1;
    let sib_fn = if from_end {
        Element::next_sibling_element
    } else {
        Element::prev_sibling_element
    };
    let mut sib = sib_fn(element);
    while let Some(s) = sib {
        if matches_any(&s, sel_list) {
            index += 1;
        }
        sib = sib_fn(&s);
    }
    index
}

fn has_matching_descendant<E: Element>(element: &E, selector: &Selector) -> bool {
    for child in element.child_elements() {
        if matches_selector(&child, selector) {
            return true;
        }
        if has_matching_descendant(&child, selector) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Mock DOM for testing ---

    #[derive(Debug, Clone)]
    struct MockElement {
        tag: String,
        id: Option<String>,
        classes: Vec<String>,
        attrs: Vec<(String, String)>,
        parent: Option<Box<MockElement>>,
        children: Vec<MockElement>,
        prev_sibling: Option<Box<MockElement>>,
        next_sibling: Option<Box<MockElement>>,
        is_root: bool,
    }

    impl MockElement {
        fn new(tag: &str) -> Self {
            Self {
                tag: tag.to_string(),
                id: None,
                classes: Vec::new(),
                attrs: Vec::new(),
                parent: None,
                children: Vec::new(),
                prev_sibling: None,
                next_sibling: None,
                is_root: false,
            }
        }

        fn with_id(mut self, id: &str) -> Self {
            self.id = Some(id.to_string());
            self
        }

        fn with_class(mut self, class: &str) -> Self {
            self.classes.push(class.to_string());
            self
        }

        fn with_attr(mut self, name: &str, value: &str) -> Self {
            self.attrs.push((name.to_string(), value.to_string()));
            self
        }

        #[allow(clippy::wrong_self_convention)] // test builder: by-value chaining
        fn as_root(mut self) -> Self {
            self.is_root = true;
            self
        }
    }

    impl Element for MockElement {
        fn local_name(&self) -> &str {
            &self.tag
        }
        fn id(&self) -> Option<&str> {
            self.id.as_deref()
        }
        fn has_class(&self, name: &str) -> bool {
            self.classes.iter().any(|c| c == name)
        }
        fn has_attribute(&self, name: &str) -> bool {
            self.attrs.iter().any(|(n, _)| n == name)
        }
        fn attribute_value(&self, name: &str) -> Option<&str> {
            self.attrs
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, v)| v.as_str())
        }
        fn parent_element(&self) -> Option<Self> {
            self.parent.as_ref().map(|p| *p.clone())
        }
        fn prev_sibling_element(&self) -> Option<Self> {
            self.prev_sibling.as_ref().map(|s| *s.clone())
        }
        fn next_sibling_element(&self) -> Option<Self> {
            self.next_sibling.as_ref().map(|s| *s.clone())
        }
        fn first_child_element(&self) -> Option<Self> {
            self.children.first().cloned()
        }
        fn last_child_element(&self) -> Option<Self> {
            self.children.last().cloned()
        }
        fn is_root(&self) -> bool {
            self.is_root
        }
        fn is_empty(&self) -> bool {
            self.children.is_empty()
        }
        fn child_elements(&self) -> Vec<Self> {
            self.children.clone()
        }
    }

    #[test]
    fn match_type() {
        let el = MockElement::new("div");
        let sels = parse_selector_list("div").unwrap();
        assert!(matches_selector(&el, &sels[0]));
        let sels2 = parse_selector_list("span").unwrap();
        assert!(!matches_selector(&el, &sels2[0]));
    }

    #[test]
    fn match_class() {
        let el = MockElement::new("div").with_class("foo");
        let sels = parse_selector_list(".foo").unwrap();
        assert!(matches_selector(&el, &sels[0]));
        let sels2 = parse_selector_list(".bar").unwrap();
        assert!(!matches_selector(&el, &sels2[0]));
    }

    #[test]
    fn match_id() {
        let el = MockElement::new("div").with_id("main");
        let sels = parse_selector_list("#main").unwrap();
        assert!(matches_selector(&el, &sels[0]));
    }

    #[test]
    fn match_attribute_presence() {
        let el = MockElement::new("a").with_attr("href", "https://example.com");
        let sels = parse_selector_list("[href]").unwrap();
        assert!(matches_selector(&el, &sels[0]));
    }

    #[test]
    fn match_attribute_exact() {
        let el = MockElement::new("input").with_attr("type", "text");
        let sels = parse_selector_list("[type=\"text\"]").unwrap();
        assert!(matches_selector(&el, &sels[0]));
        let sels2 = parse_selector_list("[type=\"password\"]").unwrap();
        assert!(!matches_selector(&el, &sels2[0]));
    }

    #[test]
    fn match_attribute_prefix() {
        let el = MockElement::new("a").with_attr("href", "https://example.com");
        let sels = parse_selector_list("[href^=\"https\"]").unwrap();
        assert!(matches_selector(&el, &sels[0]));
    }

    #[test]
    fn match_attribute_suffix() {
        let el = MockElement::new("a").with_attr("href", "style.css");
        let sels = parse_selector_list("[href$=\".css\"]").unwrap();
        assert!(matches_selector(&el, &sels[0]));
    }

    #[test]
    fn match_attribute_substring() {
        let el = MockElement::new("a").with_attr("href", "https://example.com/page");
        let sels = parse_selector_list("[href*=\"example\"]").unwrap();
        assert!(matches_selector(&el, &sels[0]));
    }

    #[test]
    fn match_universal() {
        let el = MockElement::new("anything");
        let sels = parse_selector_list("*").unwrap();
        assert!(matches_selector(&el, &sels[0]));
    }

    #[test]
    fn match_compound() {
        let el = MockElement::new("div")
            .with_id("main")
            .with_class("content");
        let sels = parse_selector_list("div#main.content").unwrap();
        assert!(matches_selector(&el, &sels[0]));
    }

    #[test]
    fn match_child_combinator() {
        let parent = MockElement::new("div").with_class("parent");
        let child = MockElement {
            parent: Some(Box::new(parent)),
            ..MockElement::new("span")
        };
        let sels = parse_selector_list("div > span").unwrap();
        assert!(matches_selector(&child, &sels[0]));
    }

    #[test]
    fn match_descendant_combinator() {
        let grandparent = MockElement::new("div").with_class("outer");
        let parent = MockElement {
            parent: Some(Box::new(grandparent)),
            ..MockElement::new("section")
        };
        let child = MockElement {
            parent: Some(Box::new(parent)),
            ..MockElement::new("p")
        };
        let sels = parse_selector_list("div p").unwrap();
        assert!(matches_selector(&child, &sels[0]));
    }

    #[test]
    fn match_root() {
        let el = MockElement::new("html").as_root();
        let sels = parse_selector_list(":root").unwrap();
        assert!(matches_selector(&el, &sels[0]));
    }

    #[test]
    fn match_empty() {
        let el = MockElement::new("div");
        let sels = parse_selector_list(":empty").unwrap();
        assert!(matches_selector(&el, &sels[0]));

        let el_with_child = MockElement {
            children: vec![MockElement::new("span")],
            ..MockElement::new("div")
        };
        assert!(!matches_selector(&el_with_child, &sels[0]));
    }

    #[test]
    fn match_first_child() {
        let el = MockElement::new("li"); // no prev sibling = first child
        let sels = parse_selector_list(":first-child").unwrap();
        assert!(matches_selector(&el, &sels[0]));

        let el_with_prev = MockElement {
            prev_sibling: Some(Box::new(MockElement::new("li"))),
            ..MockElement::new("li")
        };
        assert!(!matches_selector(&el_with_prev, &sels[0]));
    }

    #[test]
    fn match_not() {
        let el = MockElement::new("div").with_class("visible");
        let sels = parse_selector_list(":not(.hidden)").unwrap();
        assert!(matches_selector(&el, &sels[0]));

        let hidden_el = MockElement::new("div").with_class("hidden");
        assert!(!matches_selector(&hidden_el, &sels[0]));
    }

    #[test]
    fn match_is() {
        let el = MockElement::new("h2");
        let sels = parse_selector_list(":is(h1, h2, h3)").unwrap();
        assert!(matches_selector(&el, &sels[0]));

        let p = MockElement::new("p");
        assert!(!matches_selector(&p, &sels[0]));
    }

    #[test]
    fn query_selector_first() {
        let root = MockElement {
            children: vec![
                MockElement::new("div").with_class("a"),
                MockElement::new("div").with_class("b"),
            ],
            ..MockElement::new("body")
        };

        let result = query_selector(&root, "div").unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().local_name(), "div");
    }

    #[test]
    fn query_selector_all_multiple() {
        let root = MockElement {
            children: vec![
                MockElement::new("div").with_class("a"),
                MockElement::new("span"),
                MockElement::new("div").with_class("b"),
            ],
            ..MockElement::new("body")
        };

        let results = query_selector_all(&root, "div").unwrap();
        // The mock child_elements() returns self.children which is correct.
        // All 3 children are checked, 2 are divs.
        assert_eq!(results.len(), 2);
    }
}
