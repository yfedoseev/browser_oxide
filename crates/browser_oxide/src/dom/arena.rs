use crate::dom::node::*;
use std::collections::HashSet;

/// Arena-allocated DOM tree. All nodes live in a flat Vec, referenced by NodeId.
pub struct Dom {
    nodes: Vec<Option<Node>>,
    free_list: Vec<usize>,
}

/// Tripwire for tree-walking helpers. A correct DOM tree never has cycles
/// and is bounded by `nodes.len()` unique ids. The `visited` guards in these
/// walkers already make a cycle impossible, so this is really a size ceiling:
/// raised to 2M because real pages (usa.gov and other huge gov/data pages)
/// legitimately exceed 100K nodes, and a walker that hits the limit now BAILS
/// (returns partial results) rather than `panic!`ing — a panic here crosses the
/// V8 FFI boundary as a non-unwinding abort that takes down the whole process
/// (observed crashing the 126-gate at a 100K+-node gov page). Graceful bail >
/// process abort.
const WALK_LIMIT: usize = 2_000_000;

/// Bound for ancestor walks used by the cycle assertion at mutation sites.
/// Realistic DOMs are <100 deep; 10K is several orders of magnitude beyond
/// any legitimate document.
const ANCESTOR_LIMIT: usize = 10_000;

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
        if let Some(idx) = self.free_list.pop() {
            let id = NodeId(idx);
            self.nodes[idx] = Some(Node::new(id, data));
            id
        } else {
            let id = NodeId(self.nodes.len());
            self.nodes.push(Some(Node::new(id, data)));
            id
        }
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

    /// Returns true if `candidate` is `target` or an ancestor of `target`.
    /// Used by cycle assertions on tree-mutation entry points.
    fn is_ancestor_or_self(&self, candidate: NodeId, target: NodeId) -> bool {
        if candidate == target {
            return true;
        }
        let mut current = self.get(target).and_then(|n| n.parent);
        let mut depth = 0usize;
        while let Some(id) = current {
            if id == candidate {
                return true;
            }
            depth += 1;
            if depth > ANCESTOR_LIMIT {
                // Pathological parent chain (cycle / corruption): bail rather
                // than panic across the FFI boundary — treat as not-an-ancestor.
                break;
            }
            current = self.get(id).and_then(|n| n.parent);
        }
        false
    }

    /// Append `child` as the last child of `parent`.
    /// Per the DOM spec's pre-insertion validity rules, an attempt to make
    /// `parent` a descendant of itself is a `HierarchyRequestError`. We can't
    /// throw a JS exception from here (called from non-unwinding op fast
    /// paths), so we log and no-op — the script's bug surfaces as a silently
    /// skipped mutation rather than a process abort or a stack overflow.
    pub fn append_child(&mut self, parent: NodeId, child: NodeId) {
        // Both ids must reference live arena nodes. The JS shim returns
        // NodeId(u32::MAX) (-1 cast) for non-Node values; treat unknown ids
        // as a silent no-op rather than partially wiring the link list.
        if self.get(parent).is_none() || self.get(child).is_none() {
            return;
        }
        if self.is_ancestor_or_self(child, parent) {
            eprintln!(
                "[dom] cycle prevented in append_child: child {:?} is parent {:?} or one of its ancestors — skipping",
                child, parent
            );
            return;
        }
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
    /// Same cycle-protection semantics as `append_child`: log and no-op.
    pub fn insert_before(&mut self, parent: NodeId, child: NodeId, reference: NodeId) {
        if self.get(parent).is_none() || self.get(child).is_none() || self.get(reference).is_none()
        {
            return;
        }
        if self.is_ancestor_or_self(child, parent) {
            eprintln!(
                "[dom] cycle prevented in insert_before: child {:?} is parent {:?} or one of its ancestors — skipping",
                child, parent
            );
            return;
        }
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
            .filter(|id| self.get(*id).is_some_and(|n| n.is_element()))
            .collect()
    }

    /// Get text content of a subtree.
    pub fn text_content(&self, id: NodeId) -> String {
        let mut result = String::new();
        self.collect_text(id, &mut result);
        result
    }

    /// Iterative pre-order DFS that concatenates text-node payloads in document order.
    /// `visited` + step counter guard against cycles in the arena (which should be
    /// impossible given `append_child`/`insert_before` cycle assertions, but provides
    /// a clear panic message instead of an OS abort if state ever becomes corrupt).
    fn collect_text(&self, root: NodeId, result: &mut String) {
        let mut stack: Vec<NodeId> = vec![root];
        let mut visited: HashSet<NodeId> = HashSet::with_capacity(64);
        let mut steps: usize = 0;
        while let Some(id) = stack.pop() {
            if !visited.insert(id) {
                continue;
            }
            steps += 1;
            if steps > WALK_LIMIT {
                break; // huge/pathological DOM — return text collected so far
            }
            let node = match self.get(id) {
                Some(n) => n,
                None => continue,
            };
            match &node.data {
                NodeData::Text(t) => result.push_str(t),
                _ => {
                    // Push children in reverse so leftmost child pops first (document order).
                    let mut kids: Vec<NodeId> = Vec::new();
                    let mut child = node.first_child;
                    while let Some(c) = child {
                        kids.push(c);
                        child = self.get(c).and_then(|n| n.next_sibling);
                    }
                    stack.extend(kids.into_iter().rev());
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
                .is_some_and(|a| a.value == id_value)
        })
    }

    /// Find elements by tag name (case-insensitive).
    pub fn get_elements_by_tag_name(&self, root: NodeId, tag: &str) -> Vec<NodeId> {
        let mut results = Vec::new();
        self.collect_elements(
            root,
            &|node| {
                node.as_element()
                    .is_some_and(|e| e.name.local.eq_ignore_ascii_case(tag))
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
                    .is_some_and(|a| a.value.split_whitespace().any(|c| c == class))
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
        let mut kids: Vec<NodeId> = Vec::new();
        let mut child = self.get(id).and_then(|n| n.first_child);
        while let Some(child_id) = child {
            kids.push(child_id);
            child = self.get(child_id).and_then(|n| n.next_sibling);
        }
        for c in kids {
            self.serialize_node(c, &mut out);
        }
        out
    }

    /// Iterative serializer. Uses an explicit work-stack with two kinds of
    /// items: `Open(id)` to begin processing a node, `Close(s)` to emit a
    /// closing tag string after that node's descendants have been serialized.
    /// `visited` + step counter guard against cycles in the arena.
    fn serialize_node(&self, root: NodeId, out: &mut String) {
        enum SerWork {
            Open(NodeId),
            Close(String),
        }
        const VOID_ELEMENTS: &[&str] = &[
            "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param",
            "source", "track", "wbr",
        ];

        let mut stack: Vec<SerWork> = vec![SerWork::Open(root)];
        let mut visited: HashSet<NodeId> = HashSet::with_capacity(64);
        let mut steps: usize = 0;
        while let Some(work) = stack.pop() {
            match work {
                SerWork::Close(s) => out.push_str(&s),
                SerWork::Open(id) => {
                    if !visited.insert(id) {
                        continue;
                    }
                    steps += 1;
                    if steps > WALK_LIMIT {
                        break; // huge/pathological DOM — return partial serialization
                    }
                    let node = match self.get(id) {
                        Some(n) => n,
                        None => continue,
                    };
                    match &node.data {
                        NodeData::Element(elem) => {
                            out.push('<');
                            out.push_str(&elem.name.local);
                            for attr in &elem.attrs {
                                out.push(' ');
                                out.push_str(&attr.name.local);
                                out.push_str("=\"");
                                out.push_str(
                                    &attr.value.replace('&', "&amp;").replace('"', "&quot;"),
                                );
                                out.push('"');
                            }
                            out.push('>');

                            let is_void = VOID_ELEMENTS.contains(&elem.name.local.as_str());
                            // Schedule the close tag first so it pops last (after all descendants).
                            if !is_void {
                                stack.push(SerWork::Close(format!("</{}>", elem.name.local)));
                            }
                            // Then push children in reverse so leftmost pops first.
                            let mut kids: Vec<NodeId> = Vec::new();
                            let mut child = node.first_child;
                            while let Some(c) = child {
                                kids.push(c);
                                child = self.get(c).and_then(|n| n.next_sibling);
                            }
                            for c in kids.into_iter().rev() {
                                stack.push(SerWork::Open(c));
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
                            let mut kids: Vec<NodeId> = Vec::new();
                            let mut child = node.first_child;
                            while let Some(c) = child {
                                kids.push(c);
                                child = self.get(c).and_then(|n| n.next_sibling);
                            }
                            for c in kids.into_iter().rev() {
                                stack.push(SerWork::Open(c));
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    /// Copy a subtree from another Dom into this one. Returns the new root NodeId.
    /// Iterative: each work item is `(source_id, dest_parent_id)`. The root is
    /// created up-front and returned; descendants are appended in document order.
    pub fn merge_subtree(&mut self, source: &Dom, source_root: NodeId) -> NodeId {
        // Helper: clone a source node's data into a fresh node in `self`.
        fn create_from(this: &mut Dom, source: &Dom, src_id: NodeId) -> Option<NodeId> {
            let src = source.get(src_id)?;
            Some(match &src.data {
                NodeData::Element(elem) => {
                    this.create_element(elem.name.clone(), elem.attrs.clone())
                }
                NodeData::Text(t) => this.create_text(t.clone()),
                NodeData::Comment(t) => this.create_comment(t.clone()),
                NodeData::DocumentFragment | NodeData::Document => this.create_document_fragment(),
                _ => this.create_document_fragment(),
            })
        }

        let new_root = match create_from(self, source, source_root) {
            Some(id) => id,
            None => return self.create_document_fragment(),
        };

        // BFS-style queue: process all source children, appending under their
        // mirrored dest parents. visited tracks source ids so a cycle in the
        // source (which would be data corruption) cannot loop forever.
        let mut queue: Vec<(NodeId, NodeId)> = Vec::new();
        let mut visited: HashSet<NodeId> = HashSet::with_capacity(64);
        visited.insert(source_root);

        let mut child = source.get(source_root).and_then(|n| n.first_child);
        while let Some(c) = child {
            queue.push((c, new_root));
            child = source.get(c).and_then(|n| n.next_sibling);
        }

        let mut steps: usize = 0;
        let mut i = 0usize;
        while i < queue.len() {
            let (src_id, dest_parent) = queue[i];
            i += 1;
            steps += 1;
            if steps > WALK_LIMIT {
                break; // huge/pathological subtree — stop merging gracefully
            }
            if !visited.insert(src_id) {
                continue;
            }
            let new_id = match create_from(self, source, src_id) {
                Some(id) => id,
                None => continue,
            };
            self.append_child(dest_parent, new_id);
            let mut child = source.get(src_id).and_then(|n| n.first_child);
            while let Some(c) = child {
                queue.push((c, new_id));
                child = source.get(c).and_then(|n| n.next_sibling);
            }
        }

        new_root
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

    /// Iterative pre-order DFS that returns the first descendant of `root`
    /// (excluding `root` itself) matching `predicate`, in document order.
    fn find_element(&self, root: NodeId, predicate: &dyn Fn(&Node) -> bool) -> Option<NodeId> {
        let mut stack: Vec<NodeId> = Vec::new();
        // Seed with children of root (predicate is not checked against root itself,
        // matching the original recursive semantics).
        let mut child = self.get(root).and_then(|n| n.first_child);
        let mut seed: Vec<NodeId> = Vec::new();
        while let Some(c) = child {
            seed.push(c);
            child = self.get(c).and_then(|n| n.next_sibling);
        }
        stack.extend(seed.into_iter().rev());

        let mut visited: HashSet<NodeId> = HashSet::with_capacity(64);
        let mut steps: usize = 0;
        while let Some(id) = stack.pop() {
            if !visited.insert(id) {
                continue;
            }
            steps += 1;
            if steps > WALK_LIMIT {
                break; // huge/pathological DOM — return None (not found within limit)
            }
            let node = match self.get(id) {
                Some(n) => n,
                None => continue,
            };
            if predicate(node) {
                return Some(id);
            }
            let mut kids: Vec<NodeId> = Vec::new();
            let mut child = node.first_child;
            while let Some(c) = child {
                kids.push(c);
                child = self.get(c).and_then(|n| n.next_sibling);
            }
            stack.extend(kids.into_iter().rev());
        }
        None
    }

    /// Iterative pre-order DFS that pushes every descendant of `root` matching
    /// `predicate` into `results`, in document order.
    fn collect_elements(
        &self,
        root: NodeId,
        predicate: &dyn Fn(&Node) -> bool,
        results: &mut Vec<NodeId>,
    ) {
        let mut stack: Vec<NodeId> = Vec::new();
        let mut seed: Vec<NodeId> = Vec::new();
        let mut child = self.get(root).and_then(|n| n.first_child);
        while let Some(c) = child {
            seed.push(c);
            child = self.get(c).and_then(|n| n.next_sibling);
        }
        stack.extend(seed.into_iter().rev());

        let mut visited: HashSet<NodeId> = HashSet::with_capacity(64);
        let mut steps: usize = 0;
        while let Some(id) = stack.pop() {
            if !visited.insert(id) {
                continue;
            }
            steps += 1;
            if steps > WALK_LIMIT {
                break; // huge/pathological DOM — return elements collected so far
            }
            let node = match self.get(id) {
                Some(n) => n,
                None => continue,
            };
            if predicate(node) {
                results.push(id);
            }
            let mut kids: Vec<NodeId> = Vec::new();
            let mut child = node.first_child;
            while let Some(c) = child {
                kids.push(c);
                child = self.get(c).and_then(|n| n.next_sibling);
            }
            stack.extend(kids.into_iter().rev());
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
    fn append_child_rejects_self_cycle() {
        let mut dom = Dom::new();
        let div = dom.create_element(QualName::new("div"), vec![]);
        dom.append_child(NodeId::DOCUMENT, div);
        // Trying to make div its own child must be a no-op, not silently
        // corrupt the arena.
        dom.append_child(div, div);
        // div is still a top-level child of document; div has no children.
        assert_eq!(dom.children(NodeId::DOCUMENT), vec![div]);
        assert!(dom.children(div).is_empty());
        assert_eq!(dom.get(div).unwrap().parent, Some(NodeId::DOCUMENT));
    }

    #[test]
    fn append_child_rejects_ancestor_cycle() {
        let mut dom = Dom::new();
        let outer = dom.create_element(QualName::new("outer"), vec![]);
        let inner = dom.create_element(QualName::new("inner"), vec![]);
        dom.append_child(NodeId::DOCUMENT, outer);
        dom.append_child(outer, inner);
        // inner is a descendant of outer; making outer a child of inner would cycle.
        dom.append_child(inner, outer);
        // Tree state must be unchanged: inner still under outer, outer still under doc.
        assert_eq!(dom.children(NodeId::DOCUMENT), vec![outer]);
        assert_eq!(dom.children(outer), vec![inner]);
        assert!(dom.children(inner).is_empty());
        assert_eq!(dom.get(outer).unwrap().parent, Some(NodeId::DOCUMENT));
        assert_eq!(dom.get(inner).unwrap().parent, Some(outer));
    }

    #[test]
    fn iterative_walkers_terminate_on_cycle_instead_of_overflowing() {
        // Force a cycle by directly mutating arena pointers (bypassing the
        // public mutation API which would reject this). The original recursive
        // walker would overflow the C stack; the iterative walker's visited
        // set must dedup cycle nodes and complete in bounded time.
        let mut dom = Dom::new();
        let a = dom.create_element(QualName::new("a"), vec![]);
        let b = dom.create_element(QualName::new("b"), vec![]);
        let txt = dom.create_text("hi".to_string());
        dom.append_child(NodeId::DOCUMENT, a);
        dom.append_child(a, b);
        dom.append_child(b, txt);
        // Splice b's first_child to a — a cycle a → b → a (with text under b too).
        dom.get_mut(b).unwrap().first_child = Some(a);

        // Each call must return without overflowing or hanging.
        let _ = dom.text_content(a);
        let _ = dom.serialize_html(a);
        let _ = dom.serialize_inner_html(a);
        let mut results = Vec::new();
        dom.collect_elements(a, &|n| n.is_element(), &mut results);
        assert!(dom.find_element(a, &|_| false).is_none());
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
