use crate::node::*;

/// Arena-allocated DOM tree. All nodes live in a flat Vec, referenced by NodeId.
pub struct Dom {
    nodes: Vec<Option<Node>>,
    free_list: Vec<usize>,
}

impl Dom {
    /// Create a new DOM with a Document node at index 0.
    pub fn new() -> Self {
        let doc_node = Node::new(NodeId::DOCUMENT, NodeData::Document);
        Self {
            nodes: vec![Some(doc_node)],
            free_list: Vec::new(),
        }
    }

    /// Get the document node ID.
    pub fn document(&self) -> NodeId {
        NodeId::DOCUMENT
    }

    /// Get a reference to a node by ID.
    pub fn get(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id.0).and_then(|n| n.as_ref())
    }

    /// Get a mutable reference to a node by ID.
    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.get_mut(id.0).and_then(|n| n.as_mut())
    }

    /// Total number of live nodes.
    pub fn len(&self) -> usize {
        self.nodes.iter().filter(|n| n.is_some()).count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    // --- Node creation ---

    fn allocate(&mut self, data: NodeData) -> NodeId {
        let id = if let Some(idx) = self.free_list.pop() {
            let id = NodeId(idx);
            self.nodes[idx] = Some(Node::new(id, data));
            id
        } else {
            let id = NodeId(self.nodes.len());
            self.nodes.push(Some(Node::new(id, data)));
            id
        };
        id
    }

    pub fn create_element(&mut self, name: QualName, attrs: Vec<Attribute>) -> NodeId {
        self.allocate(NodeData::Element(ElementData {
            name,
            attrs,
            shadow_root: None,
        }))
    }

    pub fn create_text(&mut self, text: String) -> NodeId {
        self.allocate(NodeData::Text(text))
    }

    pub fn create_comment(&mut self, text: String) -> NodeId {
        self.allocate(NodeData::Comment(text))
    }

    pub fn create_document_fragment(&mut self) -> NodeId {
        self.allocate(NodeData::DocumentFragment)
    }

    /// Create a shadow root and attach it to the given host element.
    pub fn create_shadow_root(&mut self, host: NodeId, mode: ShadowRootMode) -> NodeId {
        let shadow_id = self.allocate(NodeData::ShadowRoot { mode, host });
        if let Some(node) = self.get_mut(host) {
            if let Some(elem) = node.as_element_mut() {
                elem.shadow_root = Some(shadow_id);
            }
        }
        shadow_id
    }

    pub fn allocate_pi(&mut self, target: String, data: String) -> NodeId {
        self.allocate(NodeData::ProcessingInstruction { target, data })
    }

    pub fn create_doctype(&mut self, name: String, public_id: String, system_id: String) -> NodeId {
        self.allocate(NodeData::DocumentType {
            name,
            public_id,
            system_id,
        })
    }

    // --- Tree mutation (all O(1)) ---

    /// Append `child` as the last child of `parent`.
    pub fn append_child(&mut self, parent: NodeId, child: NodeId) {
        self.detach(child);

        let old_last = self.get(parent).and_then(|n| n.last_child);

        if let Some(node) = self.get_mut(child) {
            node.parent = Some(parent);
            node.prev_sibling = old_last;
            node.next_sibling = None;
        }

        if let Some(old_last_id) = old_last {
            if let Some(node) = self.get_mut(old_last_id) {
                node.next_sibling = Some(child);
            }
        } else {
            if let Some(node) = self.get_mut(parent) {
                node.first_child = Some(child);
            }
        }

        if let Some(node) = self.get_mut(parent) {
            node.last_child = Some(child);
        }
    }

    /// Insert `child` before `reference` (which must be a child of `parent`).
    pub fn insert_before(&mut self, parent: NodeId, child: NodeId, reference: NodeId) {
        self.detach(child);

        let prev = self.get(reference).and_then(|n| n.prev_sibling);

        if let Some(node) = self.get_mut(child) {
            node.parent = Some(parent);
            node.prev_sibling = prev;
            node.next_sibling = Some(reference);
        }

        if let Some(node) = self.get_mut(reference) {
            node.prev_sibling = Some(child);
        }

        if let Some(prev_id) = prev {
            if let Some(node) = self.get_mut(prev_id) {
                node.next_sibling = Some(child);
            }
        } else {
            if let Some(node) = self.get_mut(parent) {
                node.first_child = Some(child);
            }
        }
    }

    /// Detach a node from its parent (but keep it in the arena).
    pub fn detach(&mut self, id: NodeId) {
        let (parent, prev, next) = match self.get(id) {
            Some(node) => (node.parent, node.prev_sibling, node.next_sibling),
            None => return,
        };

        if let Some(prev_id) = prev {
            if let Some(node) = self.get_mut(prev_id) {
                node.next_sibling = next;
            }
        } else if let Some(parent_id) = parent {
            if let Some(node) = self.get_mut(parent_id) {
                node.first_child = next;
            }
        }

        if let Some(next_id) = next {
            if let Some(node) = self.get_mut(next_id) {
                node.prev_sibling = prev;
            }
        } else if let Some(parent_id) = parent {
            if let Some(node) = self.get_mut(parent_id) {
                node.last_child = prev;
            }
        }

        if let Some(node) = self.get_mut(id) {
            node.parent = None;
            node.prev_sibling = None;
            node.next_sibling = None;
        }
    }

    /// Remove a node from the arena entirely (recycles its slot).
    pub fn remove(&mut self, id: NodeId) {
        self.detach(id);
        if id.0 < self.nodes.len() {
            self.nodes[id.0] = None;
            self.free_list.push(id.0);
        }
    }

    /// Move all children of `source` to become children of `target`.
    pub fn reparent_children(&mut self, source: NodeId, target: NodeId) {
        loop {
            let child = self.get(source).and_then(|n| n.first_child);
            match child {
                Some(child_id) => self.append_child(target, child_id),
                None => break,
            }
        }
    }

    // --- Traversal helpers ---

    /// Iterate over child node IDs.
    pub fn children(&self, parent: NodeId) -> Vec<NodeId> {
        let mut result = Vec::new();
        let mut current = self.get(parent).and_then(|n| n.first_child);
        while let Some(id) = current {
            result.push(id);
            current = self.get(id).and_then(|n| n.next_sibling);
        }
        result
    }

    /// Iterate over child element node IDs (skip text/comment nodes).
    pub fn child_elements(&self, parent: NodeId) -> Vec<NodeId> {
        self.children(parent)
            .into_iter()
            .filter(|id| self.get(*id).map_or(false, |n| n.is_element()))
            .collect()
    }

    /// Get text content of a subtree.
    pub fn text_content(&self, id: NodeId) -> String {
        let mut result = String::new();
        self.collect_text(id, &mut result);
        result
    }

    fn collect_text(&self, id: NodeId, result: &mut String) {
        if let Some(node) = self.get(id) {
            match &node.data {
                NodeData::Text(t) => result.push_str(t),
                _ => {
                    let mut child = node.first_child;
                    while let Some(child_id) = child {
                        self.collect_text(child_id, result);
                        child = self.get(child_id).and_then(|n| n.next_sibling);
                    }
                }
            }
        }
    }

    // --- Phase 2: Methods for JS integration ---

    /// Set text content: remove all children, add a single text node.
    pub fn set_text_content(&mut self, id: NodeId, text: &str) {
        // Remove all children
        let children: Vec<NodeId> = self.children(id);
        for child in children {
            self.remove(child);
        }
        // Add text node
        if !text.is_empty() {
            let text_id = self.create_text(text.to_string());
            self.append_child(id, text_id);
        }
    }

    /// Find an element by ID attribute (tree walk from document root).
    pub fn get_element_by_id(&self, id_value: &str) -> Option<NodeId> {
        self.find_element(NodeId::DOCUMENT, &|node| {
            node.as_element()
                .and_then(|e| e.attrs.iter().find(|a| a.name.local == "id"))
                .map_or(false, |a| a.value == id_value)
        })
    }

    /// Find elements by tag name (case-insensitive).
    pub fn get_elements_by_tag_name(&self, root: NodeId, tag: &str) -> Vec<NodeId> {
        let mut results = Vec::new();
        self.collect_elements(
            root,
            &|node| {
                node.as_element()
                    .map_or(false, |e| e.name.local.eq_ignore_ascii_case(tag))
            },
            &mut results,
        );
        results
    }

    /// Find elements by class name.
    pub fn get_elements_by_class_name(&self, root: NodeId, class: &str) -> Vec<NodeId> {
        let mut results = Vec::new();
        self.collect_elements(
            root,
            &|node| {
                node.as_element()
                    .and_then(|e| e.attrs.iter().find(|a| a.name.local == "class"))
                    .map_or(false, |a| a.value.split_whitespace().any(|c| c == class))
            },
            &mut results,
        );
        results
    }

    /// Serialize a subtree as HTML.
    pub fn serialize_html(&self, id: NodeId) -> String {
        let mut out = String::new();
        self.serialize_node(id, &mut out);
        out
    }

    /// Serialize children of a node as HTML (for innerHTML getter).
    pub fn serialize_inner_html(&self, id: NodeId) -> String {
        let mut out = String::new();
        let mut child = self.get(id).and_then(|n| n.first_child);
        while let Some(child_id) = child {
            self.serialize_node(child_id, &mut out);
            child = self.get(child_id).and_then(|n| n.next_sibling);
        }
        out
    }

    fn serialize_node(&self, id: NodeId, out: &mut String) {
        let node = match self.get(id) {
            Some(n) => n,
            None => return,
        };
        match &node.data {
            NodeData::Element(elem) => {
                out.push('<');
                out.push_str(&elem.name.local);
                for attr in &elem.attrs {
                    out.push(' ');
                    out.push_str(&attr.name.local);
                    out.push_str("=\"");
                    out.push_str(&attr.value.replace('&', "&amp;").replace('"', "&quot;"));
                    out.push('"');
                }
                out.push('>');

                // Serialize children
                let mut child = node.first_child;
                while let Some(child_id) = child {
                    self.serialize_node(child_id, out);
                    child = self.get(child_id).and_then(|n| n.next_sibling);
                }

                // Closing tag (skip void elements)
                let void_elements = [
                    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta",
                    "param", "source", "track", "wbr",
                ];
                if !void_elements.contains(&elem.name.local.as_str()) {
                    out.push_str("</");
                    out.push_str(&elem.name.local);
                    out.push('>');
                }
            }
            NodeData::Text(text) => {
                out.push_str(
                    &text
                        .replace('&', "&amp;")
                        .replace('<', "&lt;")
                        .replace('>', "&gt;"),
                );
            }
            NodeData::Comment(text) => {
                out.push_str("<!--");
                out.push_str(text);
                out.push_str("-->");
            }
            NodeData::DocumentType { name, .. } => {
                out.push_str("<!DOCTYPE ");
                out.push_str(name);
                out.push('>');
            }
            NodeData::Document | NodeData::DocumentFragment => {
                let mut child = node.first_child;
                while let Some(child_id) = child {
                    self.serialize_node(child_id, out);
                    child = self.get(child_id).and_then(|n| n.next_sibling);
                }
            }
            _ => {}
        }
    }

    /// Copy a subtree from another Dom into this one. Returns the new root NodeId.
    pub fn merge_subtree(&mut self, source: &Dom, source_root: NodeId) -> NodeId {
        let source_node = match source.get(source_root) {
            Some(n) => n,
            None => return self.create_document_fragment(),
        };

        let new_id = match &source_node.data {
            NodeData::Element(elem) => self.create_element(elem.name.clone(), elem.attrs.clone()),
            NodeData::Text(t) => self.create_text(t.clone()),
            NodeData::Comment(t) => self.create_comment(t.clone()),
            NodeData::DocumentFragment | NodeData::Document => self.create_document_fragment(),
            _ => self.create_document_fragment(),
        };

        // Recursively copy children
        let mut child = source_node.first_child;
        while let Some(child_id) = child {
            let new_child = self.merge_subtree(source, child_id);
            self.append_child(new_id, new_child);
            child = source.get(child_id).and_then(|n| n.next_sibling);
        }

        new_id
    }

    /// Get the node type number (matches DOM spec nodeType).
    pub fn node_type(&self, id: NodeId) -> u32 {
        match self.get(id).map(|n| &n.data) {
            Some(NodeData::Element(_)) => 1,
            Some(NodeData::Text(_)) => 3,
            Some(NodeData::ProcessingInstruction { .. }) => 7,
            Some(NodeData::Comment(_)) => 8,
            Some(NodeData::Document) => 9,
            Some(NodeData::DocumentType { .. }) => 10,
            Some(NodeData::DocumentFragment) => 11,
            Some(NodeData::ShadowRoot { .. }) => 11,
            None => 0,
        }
    }

    // --- Internal helpers ---

    fn find_element(&self, root: NodeId, predicate: &dyn Fn(&Node) -> bool) -> Option<NodeId> {
        let mut child = self.get(root).and_then(|n| n.first_child);
        while let Some(child_id) = child {
            if let Some(node) = self.get(child_id) {
                if predicate(node) {
                    return Some(child_id);
                }
                if let Some(found) = self.find_element(child_id, predicate) {
                    return Some(found);
                }
            }
            child = self.get(child_id).and_then(|n| n.next_sibling);
        }
        None
    }

    fn collect_elements(
        &self,
        root: NodeId,
        predicate: &dyn Fn(&Node) -> bool,
        results: &mut Vec<NodeId>,
    ) {
        let mut child = self.get(root).and_then(|n| n.first_child);
        while let Some(child_id) = child {
            if let Some(node) = self.get(child_id) {
                if predicate(node) {
                    results.push(child_id);
                }
                self.collect_elements(child_id, predicate, results);
            }
            child = self.get(child_id).and_then(|n| n.next_sibling);
        }
    }
}

impl Default for Dom {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_dom_has_document() {
        let dom = Dom::new();
        assert_eq!(dom.len(), 1);
        assert!(dom.get(NodeId::DOCUMENT).is_some());
    }

    #[test]
    fn create_and_append() {
        let mut dom = Dom::new();
        let div = dom.create_element(QualName::new("div"), vec![]);
        dom.append_child(NodeId::DOCUMENT, div);

        assert_eq!(dom.children(NodeId::DOCUMENT), vec![div]);
        assert_eq!(dom.get(div).unwrap().parent, Some(NodeId::DOCUMENT));
    }

    #[test]
    fn append_multiple_children() {
        let mut dom = Dom::new();
        let a = dom.create_element(QualName::new("a"), vec![]);
        let b = dom.create_element(QualName::new("b"), vec![]);
        let c = dom.create_element(QualName::new("c"), vec![]);
        dom.append_child(NodeId::DOCUMENT, a);
        dom.append_child(NodeId::DOCUMENT, b);
        dom.append_child(NodeId::DOCUMENT, c);

        assert_eq!(dom.children(NodeId::DOCUMENT), vec![a, b, c]);
        assert_eq!(dom.get(a).unwrap().next_sibling, Some(b));
        assert_eq!(dom.get(b).unwrap().prev_sibling, Some(a));
        assert_eq!(dom.get(b).unwrap().next_sibling, Some(c));
    }

    #[test]
    fn insert_before() {
        let mut dom = Dom::new();
        let a = dom.create_element(QualName::new("a"), vec![]);
        let c = dom.create_element(QualName::new("c"), vec![]);
        dom.append_child(NodeId::DOCUMENT, a);
        dom.append_child(NodeId::DOCUMENT, c);

        let b = dom.create_element(QualName::new("b"), vec![]);
        dom.insert_before(NodeId::DOCUMENT, b, c);

        assert_eq!(dom.children(NodeId::DOCUMENT), vec![a, b, c]);
    }

    #[test]
    fn detach() {
        let mut dom = Dom::new();
        let a = dom.create_element(QualName::new("a"), vec![]);
        let b = dom.create_element(QualName::new("b"), vec![]);
        let c = dom.create_element(QualName::new("c"), vec![]);
        dom.append_child(NodeId::DOCUMENT, a);
        dom.append_child(NodeId::DOCUMENT, b);
        dom.append_child(NodeId::DOCUMENT, c);

        dom.detach(b);
        assert_eq!(dom.children(NodeId::DOCUMENT), vec![a, c]);
        assert_eq!(dom.get(a).unwrap().next_sibling, Some(c));
        assert_eq!(dom.get(c).unwrap().prev_sibling, Some(a));
    }

    #[test]
    fn remove_recycles() {
        let mut dom = Dom::new();
        let a = dom.create_element(QualName::new("a"), vec![]);
        dom.append_child(NodeId::DOCUMENT, a);
        dom.remove(a);

        // Slot recycled
        let b = dom.create_element(QualName::new("b"), vec![]);
        assert_eq!(b.0, a.0); // reused slot
    }

    #[test]
    fn text_content() {
        let mut dom = Dom::new();
        let div = dom.create_element(QualName::new("div"), vec![]);
        let text1 = dom.create_text("Hello ".to_string());
        let span = dom.create_element(QualName::new("span"), vec![]);
        let text2 = dom.create_text("world".to_string());

        dom.append_child(NodeId::DOCUMENT, div);
        dom.append_child(div, text1);
        dom.append_child(div, span);
        dom.append_child(span, text2);

        assert_eq!(dom.text_content(div), "Hello world");
    }

    #[test]
    fn child_elements_skips_text() {
        let mut dom = Dom::new();
        let div = dom.create_element(QualName::new("div"), vec![]);
        let text = dom.create_text("hello".to_string());
        let span = dom.create_element(QualName::new("span"), vec![]);

        dom.append_child(NodeId::DOCUMENT, div);
        dom.append_child(div, text);
        dom.append_child(div, span);

        assert_eq!(dom.child_elements(div), vec![span]);
    }

    #[test]
    fn reparent_children() {
        let mut dom = Dom::new();
        let source = dom.create_element(QualName::new("source"), vec![]);
        let target = dom.create_element(QualName::new("target"), vec![]);
        let a = dom.create_element(QualName::new("a"), vec![]);
        let b = dom.create_element(QualName::new("b"), vec![]);

        dom.append_child(NodeId::DOCUMENT, source);
        dom.append_child(NodeId::DOCUMENT, target);
        dom.append_child(source, a);
        dom.append_child(source, b);

        dom.reparent_children(source, target);

        assert!(dom.children(source).is_empty());
        assert_eq!(dom.children(target), vec![a, b]);
    }

    #[test]
    fn node_id_raw_roundtrip() {
        let id = NodeId::from_raw(42);
        assert_eq!(id.to_raw(), 42);
    }

    #[test]
    fn set_text_content() {
        let mut dom = Dom::new();
        let div = dom.create_element(QualName::new("div"), vec![]);
        let span = dom.create_element(QualName::new("span"), vec![]);
        let text = dom.create_text("old".to_string());
        dom.append_child(NodeId::DOCUMENT, div);
        dom.append_child(div, span);
        dom.append_child(span, text);

        dom.set_text_content(div, "new text");
        assert_eq!(dom.text_content(div), "new text");
        assert_eq!(dom.child_elements(div).len(), 0); // span removed
    }

    #[test]
    fn get_element_by_id() {
        let mut dom = Dom::new();
        let div = dom.create_element(
            QualName::new("div"),
            vec![Attribute {
                name: QualName::new("id"),
                value: "main".to_string(),
            }],
        );
        dom.append_child(NodeId::DOCUMENT, div);

        assert_eq!(dom.get_element_by_id("main"), Some(div));
        assert_eq!(dom.get_element_by_id("nonexistent"), None);
    }

    #[test]
    fn get_elements_by_tag_name() {
        let mut dom = Dom::new();
        let div = dom.create_element(QualName::new("div"), vec![]);
        let p1 = dom.create_element(QualName::new("p"), vec![]);
        let p2 = dom.create_element(QualName::new("p"), vec![]);
        let span = dom.create_element(QualName::new("span"), vec![]);
        dom.append_child(NodeId::DOCUMENT, div);
        dom.append_child(div, p1);
        dom.append_child(div, span);
        dom.append_child(div, p2);

        let ps = dom.get_elements_by_tag_name(NodeId::DOCUMENT, "p");
        assert_eq!(ps, vec![p1, p2]);
    }

    #[test]
    fn get_elements_by_class_name() {
        let mut dom = Dom::new();
        let a = dom.create_element(
            QualName::new("div"),
            vec![Attribute {
                name: QualName::new("class"),
                value: "foo bar".to_string(),
            }],
        );
        let b = dom.create_element(
            QualName::new("div"),
            vec![Attribute {
                name: QualName::new("class"),
                value: "baz".to_string(),
            }],
        );
        dom.append_child(NodeId::DOCUMENT, a);
        dom.append_child(NodeId::DOCUMENT, b);

        assert_eq!(
            dom.get_elements_by_class_name(NodeId::DOCUMENT, "foo"),
            vec![a]
        );
        assert_eq!(
            dom.get_elements_by_class_name(NodeId::DOCUMENT, "baz"),
            vec![b]
        );
    }

    #[test]
    fn serialize_html_basic() {
        let mut dom = Dom::new();
        let div = dom.create_element(
            QualName::new("div"),
            vec![Attribute {
                name: QualName::new("id"),
                value: "main".to_string(),
            }],
        );
        let text = dom.create_text("Hello".to_string());
        dom.append_child(NodeId::DOCUMENT, div);
        dom.append_child(div, text);

        assert_eq!(dom.serialize_html(div), "<div id=\"main\">Hello</div>");
    }

    #[test]
    fn serialize_inner_html() {
        let mut dom = Dom::new();
        let div = dom.create_element(QualName::new("div"), vec![]);
        let span = dom.create_element(QualName::new("span"), vec![]);
        let text = dom.create_text("hi".to_string());
        dom.append_child(NodeId::DOCUMENT, div);
        dom.append_child(div, span);
        dom.append_child(span, text);

        assert_eq!(dom.serialize_inner_html(div), "<span>hi</span>");
    }

    #[test]
    fn merge_subtree() {
        let mut source = Dom::new();
        let div = source.create_element(QualName::new("div"), vec![]);
        let text = source.create_text("copied".to_string());
        source.append_child(NodeId::DOCUMENT, div);
        source.append_child(div, text);

        let mut target = Dom::new();
        let parent = target.create_element(QualName::new("body"), vec![]);
        target.append_child(NodeId::DOCUMENT, parent);

        let new_div = target.merge_subtree(&source, div);
        target.append_child(parent, new_div);

        assert_eq!(target.text_content(new_div), "copied");
        assert_eq!(
            target
                .get(new_div)
                .unwrap()
                .as_element()
                .unwrap()
                .name
                .local,
            "div"
        );
    }

    #[test]
    fn node_type() {
        let mut dom = Dom::new();
        assert_eq!(dom.node_type(NodeId::DOCUMENT), 9);
        let el = dom.create_element(QualName::new("div"), vec![]);
        assert_eq!(dom.node_type(el), 1);
        let text = dom.create_text("hi".to_string());
        assert_eq!(dom.node_type(text), 3);
        let comment = dom.create_comment("c".to_string());
        assert_eq!(dom.node_type(comment), 8);
    }
}
