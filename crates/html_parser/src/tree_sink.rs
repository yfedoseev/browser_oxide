use dom::node::*;
use dom::Dom;
use html5ever::tree_builder::{ElementFlags, NodeOrText, QuirksMode, TreeSink};
use html5ever::Attribute as H5Attribute;
use html5ever::ExpandedName;
use html5ever::QualName as H5QualName;
use html5ever::{local_name, namespace_url, ns};
use std::borrow::Cow;
use std::cell::UnsafeCell;
use std::collections::HashMap;

/// TreeSink implementation that builds a browser_oxide DOM.
///
/// Uses `UnsafeCell` because html5ever's `TreeSink` trait takes `&self`
/// but tree building inherently requires mutation.
pub struct DomTreeSink {
    dom: UnsafeCell<Dom>,
    quirks_mode: UnsafeCell<QuirksMode>,
    /// Maps NodeId → original html5ever QualName so elem_name() can return
    /// proper ExpandedName references (required for SVG/MathML integration points
    /// and doctype processing).
    names: UnsafeCell<HashMap<NodeId, H5QualName>>,
}

impl DomTreeSink {
    pub fn new() -> Self {
        Self {
            dom: UnsafeCell::new(Dom::new()),
            quirks_mode: UnsafeCell::new(QuirksMode::NoQuirks),
            names: UnsafeCell::new(HashMap::new()),
        }
    }

    /// Get the built DOM (consumes the sink).
    pub fn into_dom(self) -> Dom {
        self.dom.into_inner()
    }

    fn dom(&self) -> &Dom {
        unsafe { &*self.dom.get() }
    }

    fn dom_mut(&self) -> &mut Dom {
        unsafe { &mut *self.dom.get() }
    }

    fn names(&self) -> &HashMap<NodeId, H5QualName> {
        unsafe { &*self.names.get() }
    }

    fn names_mut(&self) -> &mut HashMap<NodeId, H5QualName> {
        unsafe { &mut *self.names.get() }
    }
}

impl Default for DomTreeSink {
    fn default() -> Self {
        Self::new()
    }
}

fn convert_qualname(name: &H5QualName) -> QualName {
    let ns_str = name.ns.to_string();
    let ns = if ns_str.is_empty() || ns_str == "http://www.w3.org/1999/xhtml" {
        None
    } else {
        Some(ns_str)
    };
    QualName {
        ns,
        local: name.local.to_string(),
    }
}

fn convert_attrs(attrs: Vec<H5Attribute>) -> Vec<Attribute> {
    attrs
        .into_iter()
        .map(|a| {
            let ns_str = a.name.ns.to_string();
            Attribute {
                name: QualName {
                    ns: if ns_str.is_empty() {
                        None
                    } else {
                        Some(ns_str)
                    },
                    local: a.name.local.to_string(),
                },
                value: a.value.to_string(),
            }
        })
        .collect()
}

impl TreeSink for DomTreeSink {
    type Handle = NodeId;
    type Output = Dom;
    type ElemName<'a> = ExpandedName<'a>;

    fn finish(self) -> Self::Output {
        self.dom.into_inner()
    }

    fn parse_error(&self, _msg: Cow<'static, str>) {}

    fn get_document(&self) -> NodeId {
        NodeId::DOCUMENT
    }

    fn elem_name<'a>(&'a self, target: &'a NodeId) -> ExpandedName<'a> {
        if let Some(qn) = self.names().get(target) {
            ExpandedName {
                ns: &qn.ns,
                local: &qn.local,
            }
        } else {
            // Fallback for nodes not in our map (e.g. document node)
            static NS: markup5ever::Namespace = ns!(html);
            static LOCAL: markup5ever::LocalName = local_name!("");
            ExpandedName {
                ns: &NS,
                local: &LOCAL,
            }
        }
    }

    fn create_element(
        &self,
        name: H5QualName,
        attrs: Vec<H5Attribute>,
        _flags: ElementFlags,
    ) -> NodeId {
        let id = self
            .dom_mut()
            .create_element(convert_qualname(&name), convert_attrs(attrs));
        self.names_mut().insert(id, name);
        id
    }

    fn create_comment(&self, text: html5ever::tendril::StrTendril) -> NodeId {
        self.dom_mut().create_comment(text.to_string())
    }

    fn create_pi(
        &self,
        target: html5ever::tendril::StrTendril,
        data: html5ever::tendril::StrTendril,
    ) -> NodeId {
        self.dom_mut()
            .allocate_pi(target.to_string(), data.to_string())
    }

    fn append(&self, parent: &NodeId, child: NodeOrText<NodeId>) {
        let dom = self.dom_mut();
        match child {
            NodeOrText::AppendNode(node_id) => {
                dom.append_child(*parent, node_id);
            }
            NodeOrText::AppendText(text) => {
                // Merge with previous text node if possible
                if let Some(last_child) = dom.get(*parent).and_then(|n| n.last_child) {
                    if let Some(node) = dom.get_mut(last_child) {
                        if let NodeData::Text(ref mut existing) = node.data {
                            existing.push_str(&text);
                            return;
                        }
                    }
                }
                let text_id = dom.create_text(text.to_string());
                dom.append_child(*parent, text_id);
            }
        }
    }

    fn append_based_on_parent_node(
        &self,
        element: &NodeId,
        prev_element: &NodeId,
        child: NodeOrText<NodeId>,
    ) {
        let has_parent = self.dom().get(*element).and_then(|n| n.parent).is_some();
        if has_parent {
            self.append_before_sibling(element, child);
        } else {
            self.append(prev_element, child);
        }
    }

    fn append_doctype_to_document(
        &self,
        name: html5ever::tendril::StrTendril,
        public_id: html5ever::tendril::StrTendril,
        system_id: html5ever::tendril::StrTendril,
    ) {
        let dom = self.dom_mut();
        let doctype = dom.create_doctype(
            name.to_string(),
            public_id.to_string(),
            system_id.to_string(),
        );
        dom.append_child(NodeId::DOCUMENT, doctype);
    }

    fn get_template_contents(&self, target: &NodeId) -> NodeId {
        *target
    }

    fn same_node(&self, x: &NodeId, y: &NodeId) -> bool {
        x == y
    }

    fn set_quirks_mode(&self, mode: QuirksMode) {
        unsafe {
            *self.quirks_mode.get() = mode;
        }
    }

    fn append_before_sibling(&self, sibling: &NodeId, child: NodeOrText<NodeId>) {
        let dom = self.dom_mut();
        let parent = match dom.get(*sibling).and_then(|n| n.parent) {
            Some(p) => p,
            None => return,
        };
        match child {
            NodeOrText::AppendNode(node_id) => {
                dom.insert_before(parent, node_id, *sibling);
            }
            NodeOrText::AppendText(text) => {
                let text_id = dom.create_text(text.to_string());
                dom.insert_before(parent, text_id, *sibling);
            }
        }
    }

    fn add_attrs_if_missing(&self, target: &NodeId, attrs: Vec<H5Attribute>) {
        let dom = self.dom_mut();
        if let Some(node) = dom.get_mut(*target) {
            if let Some(elem) = node.as_element_mut() {
                for attr in convert_attrs(attrs) {
                    if !elem.attrs.iter().any(|a| a.name == attr.name) {
                        elem.attrs.push(attr);
                    }
                }
            }
        }
    }

    fn remove_from_parent(&self, target: &NodeId) {
        self.dom_mut().detach(*target);
    }

    fn reparent_children(&self, node: &NodeId, new_parent: &NodeId) {
        self.dom_mut().reparent_children(*node, *new_parent);
    }
}
