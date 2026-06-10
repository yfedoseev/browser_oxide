use crate::css_selectors::Element;
use crate::dom::arena::Dom;
use crate::dom::node::{NodeData, NodeId};

/// A DOM element wrapper that implements `crate::css_selectors::Element`.
///
/// This is a lightweight handle: it borrows the `Dom` and holds a `NodeId`.
/// Created on-the-fly for selector matching.
#[derive(Clone)]
pub struct DomElement<'a> {
    pub dom: &'a Dom,
    pub id: NodeId,
}

impl<'a> DomElement<'a> {
    pub fn new(dom: &'a Dom, id: NodeId) -> Option<Self> {
        let node = dom.get(id)?;
        if node.is_element() {
            Some(Self { dom, id })
        } else {
            None
        }
    }

    pub fn node_id(&self) -> NodeId {
        self.id
    }

    fn node(&self) -> &crate::dom::node::Node {
        self.dom.get(self.id).unwrap()
    }

    fn element_data(&self) -> &crate::dom::node::ElementData {
        self.node().as_element().unwrap()
    }
}

impl<'a> std::fmt::Debug for DomElement<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let data = self.element_data();
        write!(f, "<{}", data.name.local)?;
        for attr in &data.attrs {
            write!(f, " {}=\"{}\"", attr.name.local, attr.value)?;
        }
        write!(f, ">")
    }
}

impl<'a> Element for DomElement<'a> {
    fn local_name(&self) -> &str {
        &self.element_data().name.local
    }

    fn namespace(&self) -> Option<&str> {
        self.element_data().name.ns.as_deref()
    }

    fn id(&self) -> Option<&str> {
        self.element_data()
            .attrs
            .iter()
            .find(|a| a.name.local == "id")
            .map(|a| a.value.as_str())
    }

    fn has_class(&self, name: &str) -> bool {
        self.element_data()
            .attrs
            .iter()
            .find(|a| a.name.local == "class")
            .is_some_and(|a| a.value.split_whitespace().any(|c| c == name))
    }

    fn has_attribute(&self, name: &str) -> bool {
        self.element_data()
            .attrs
            .iter()
            .any(|a| a.name.local.eq_ignore_ascii_case(name))
    }

    fn attribute_value(&self, name: &str) -> Option<&str> {
        self.element_data()
            .attrs
            .iter()
            .find(|a| a.name.local.eq_ignore_ascii_case(name))
            .map(|a| a.value.as_str())
    }

    fn parent_element(&self) -> Option<Self> {
        let mut parent_id = self.node().parent?;
        loop {
            let parent = self.dom.get(parent_id)?;
            if parent.is_element() {
                return Some(DomElement {
                    dom: self.dom,
                    id: parent_id,
                });
            }
            parent_id = parent.parent?;
        }
    }

    fn prev_sibling_element(&self) -> Option<Self> {
        let mut sib_id = self.node().prev_sibling?;
        loop {
            let sib = self.dom.get(sib_id)?;
            if sib.is_element() {
                return Some(DomElement {
                    dom: self.dom,
                    id: sib_id,
                });
            }
            sib_id = sib.prev_sibling?;
        }
    }

    fn next_sibling_element(&self) -> Option<Self> {
        let mut sib_id = self.node().next_sibling?;
        loop {
            let sib = self.dom.get(sib_id)?;
            if sib.is_element() {
                return Some(DomElement {
                    dom: self.dom,
                    id: sib_id,
                });
            }
            sib_id = sib.next_sibling?;
        }
    }

    fn first_child_element(&self) -> Option<Self> {
        let mut child_id = self.node().first_child?;
        loop {
            let child = self.dom.get(child_id)?;
            if child.is_element() {
                return Some(DomElement {
                    dom: self.dom,
                    id: child_id,
                });
            }
            child_id = child.next_sibling?;
        }
    }

    fn last_child_element(&self) -> Option<Self> {
        let mut child_id = self.node().last_child?;
        loop {
            let child = self.dom.get(child_id)?;
            if child.is_element() {
                return Some(DomElement {
                    dom: self.dom,
                    id: child_id,
                });
            }
            child_id = child.prev_sibling?;
        }
    }

    fn is_root(&self) -> bool {
        // Root element is an element whose parent is the Document node
        match self.node().parent {
            Some(parent_id) => self
                .dom
                .get(parent_id)
                .is_some_and(|n| matches!(n.data, NodeData::Document)),
            None => false,
        }
    }

    fn is_empty(&self) -> bool {
        // Empty = no child elements and no non-empty text nodes
        let mut child_id = self.node().first_child;
        while let Some(id) = child_id {
            if let Some(child) = self.dom.get(id) {
                match &child.data {
                    NodeData::Element(_) => return false,
                    NodeData::Text(t) if !t.is_empty() => return false,
                    _ => {}
                }
                child_id = child.next_sibling;
            } else {
                break;
            }
        }
        true
    }

    fn is_link(&self) -> bool {
        let name = self.local_name();
        (name == "a" || name == "area") && self.has_attribute("href")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::arena::Dom;
    use crate::dom::node::{Attribute, QualName};

    fn build_test_dom() -> Dom {
        let mut dom = Dom::new();
        let html = dom.create_element(QualName::new("html"), vec![]);
        dom.append_child(NodeId::DOCUMENT, html);

        let body = dom.create_element(QualName::new("body"), vec![]);
        dom.append_child(html, body);

        let div = dom.create_element(
            QualName::new("div"),
            vec![
                Attribute {
                    name: QualName::new("id"),
                    value: "main".to_string(),
                },
                Attribute {
                    name: QualName::new("class"),
                    value: "container active".to_string(),
                },
            ],
        );
        dom.append_child(body, div);

        let p = dom.create_element(QualName::new("p"), vec![]);
        dom.append_child(div, p);

        let text = dom.create_text("Hello world".to_string());
        dom.append_child(p, text);

        dom
    }

    #[test]
    fn element_local_name() {
        let dom = build_test_dom();
        let _div_id = dom.child_elements(dom.children(NodeId::DOCUMENT)[0])[0]; // body's first child
        let _body_id = dom.child_elements(NodeId::DOCUMENT)[0]; // html
        let html_el = DomElement::new(&dom, dom.children(NodeId::DOCUMENT)[0]).unwrap();
        assert_eq!(html_el.local_name(), "html");
    }

    #[test]
    fn element_id_and_class() {
        let dom = build_test_dom();
        let html = dom.children(NodeId::DOCUMENT)[0];
        let body = dom.children(html)[0];
        let div = dom.children(body)[0];
        let el = DomElement::new(&dom, div).unwrap();

        assert_eq!(el.id(), Some("main"));
        assert!(el.has_class("container"));
        assert!(el.has_class("active"));
        assert!(!el.has_class("inactive"));
    }

    #[test]
    fn parent_and_children() {
        let dom = build_test_dom();
        let html = dom.children(NodeId::DOCUMENT)[0];
        let body = dom.children(html)[0];
        let div = dom.children(body)[0];
        let p = dom.children(div)[0];

        let p_el = DomElement::new(&dom, p).unwrap();
        let parent = p_el.parent_element().unwrap();
        assert_eq!(parent.local_name(), "div");

        let div_el = DomElement::new(&dom, div).unwrap();
        let first_child = div_el.first_child_element().unwrap();
        assert_eq!(first_child.local_name(), "p");
    }

    #[test]
    fn is_root() {
        let dom = build_test_dom();
        let html = dom.children(NodeId::DOCUMENT)[0];
        let body = dom.children(html)[0];

        let html_el = DomElement::new(&dom, html).unwrap();
        assert!(html_el.is_root());

        let body_el = DomElement::new(&dom, body).unwrap();
        assert!(!body_el.is_root());
    }

    #[test]
    fn is_empty() {
        let dom = build_test_dom();
        let html = dom.children(NodeId::DOCUMENT)[0];
        let body = dom.children(html)[0];
        let div = dom.children(body)[0];
        let p = dom.children(div)[0];

        let div_el = DomElement::new(&dom, div).unwrap();
        assert!(!div_el.is_empty()); // has child <p>

        // p has text "Hello world" → not empty
        let p_el = DomElement::new(&dom, p).unwrap();
        assert!(!p_el.is_empty());
    }

    #[test]
    fn selector_matching_integration() {
        let dom = build_test_dom();
        let html = dom.children(NodeId::DOCUMENT)[0];
        let body = dom.children(html)[0];
        let div = dom.children(body)[0];

        let div_el = DomElement::new(&dom, div).unwrap();

        let selectors = crate::css_selectors::parse_selector_list("div#main.container").unwrap();
        assert!(crate::css_selectors::matches_selector(
            &div_el,
            &selectors[0]
        ));

        let selectors2 = crate::css_selectors::parse_selector_list("body > div").unwrap();
        assert!(crate::css_selectors::matches_selector(
            &div_el,
            &selectors2[0]
        ));

        let selectors3 = crate::css_selectors::parse_selector_list(".nonexistent").unwrap();
        assert!(!crate::css_selectors::matches_selector(
            &div_el,
            &selectors3[0]
        ));
    }
}
