use crate::state::DomState;
use deno_core::op2;
use dom::node::NodeId;
use dom::DomElement;

// Convention: ops that return "nullable NodeId" return i64.
// -1 means null/not found. JS bootstrap converts -1 → null.

// --- Read ops ---

#[op2(fast)]
#[smi]
pub fn op_dom_document_node() -> i32 {
    NodeId::DOCUMENT.to_raw() as i32
}

#[op2]
#[string]
pub fn op_dom_get_tag_name(#[state] state: &DomState, #[smi] node_id: i32) -> String {
    let id = NodeId::from_raw(node_id as u32);
    state
        .dom
        .get(id)
        .and_then(|n| n.as_element())
        .map(|e| e.name.local.clone())
        .unwrap_or_default()
}

#[op2(fast)]
#[smi]
pub fn op_dom_get_node_type(#[state] state: &DomState, #[smi] node_id: i32) -> i32 {
    state.dom.node_type(NodeId::from_raw(node_id as u32)) as i32
}

#[op2]
#[string]
pub fn op_dom_get_text_content(#[state] state: &DomState, #[smi] node_id: i32) -> String {
    state.dom.text_content(NodeId::from_raw(node_id as u32))
}

#[op2]
#[string]
pub fn op_dom_get_inner_html(#[state] state: &DomState, #[smi] node_id: i32) -> String {
    state
        .dom
        .serialize_inner_html(NodeId::from_raw(node_id as u32))
}

#[op2]
#[string]
pub fn op_dom_get_outer_html(#[state] state: &DomState, #[smi] node_id: i32) -> String {
    state.dom.serialize_html(NodeId::from_raw(node_id as u32))
}

#[op2]
#[string]
pub fn op_dom_get_attribute(
    #[state] state: &DomState,
    #[smi] node_id: i32,
    #[string] name: &str,
) -> String {
    let id = NodeId::from_raw(node_id as u32);
    state
        .dom
        .get(id)
        .and_then(|n| n.as_element())
        .and_then(|e| {
            e.attrs
                .iter()
                .find(|a| a.name.local.eq_ignore_ascii_case(name))
                .map(|a| a.value.clone())
        })
        .unwrap_or_default()
}

#[op2(fast)]
pub fn op_dom_has_attribute(
    #[state] state: &DomState,
    #[smi] node_id: i32,
    #[string] name: &str,
) -> bool {
    let id = NodeId::from_raw(node_id as u32);
    state
        .dom
        .get(id)
        .and_then(|n| n.as_element())
        .map_or(false, |e| {
            e.attrs
                .iter()
                .any(|a| a.name.local.eq_ignore_ascii_case(name))
        })
}

/// Returns the names of all attributes on `node_id`, in source order.
/// Used by Proxy ownKeys traps for `element.attributes` and `element.dataset`.
#[op2]
#[serde]
pub fn op_dom_get_attribute_names(
    #[state] state: &DomState,
    #[smi] node_id: i32,
) -> Vec<String> {
    let id = NodeId::from_raw(node_id as u32);
    state
        .dom
        .get(id)
        .and_then(|n| n.as_element())
        .map(|e| e.attrs.iter().map(|a| a.name.local.clone()).collect())
        .unwrap_or_default()
}

/// Returns parent NodeId or -1 if no parent.
#[op2(fast)]
#[smi]
pub fn op_dom_get_parent(#[state] state: &DomState, #[smi] node_id: i32) -> i32 {
    let id = NodeId::from_raw(node_id as u32);
    state
        .dom
        .get(id)
        .and_then(|n| n.parent)
        .map(|p| p.to_raw() as i32)
        .unwrap_or(-1)
}

#[op2]
#[serde]
pub fn op_dom_get_children(#[state] state: &DomState, #[smi] node_id: i32) -> Vec<i32> {
    state
        .dom
        .children(NodeId::from_raw(node_id as u32))
        .iter()
        .map(|id| id.to_raw() as i32)
        .collect()
}

#[op2]
#[serde]
pub fn op_dom_get_children_with_types(#[state] state: &DomState, #[smi] node_id: i32) -> Vec<i32> {
    let id = NodeId::from_raw(node_id as u32);
    let children = state.dom.children(id);
    let mut res = Vec::with_capacity(children.len() * 2);
    for cid in children {
        res.push(cid.to_raw() as i32);
        res.push(state.dom.node_type(cid) as i32);
    }
    res
}

#[op2]
#[serde]
pub fn op_dom_get_child_elements(#[state] state: &DomState, #[smi] node_id: i32) -> Vec<i32> {
    state
        .dom
        .child_elements(NodeId::from_raw(node_id as u32))
        .iter()
        .map(|id| id.to_raw() as i32)
        .collect()
}

#[op2]
#[serde]
pub fn op_dom_get_child_elements_with_types(
    #[state] state: &DomState,
    #[smi] node_id: i32,
) -> Vec<i32> {
    let id = NodeId::from_raw(node_id as u32);
    let children = state.dom.child_elements(id);
    let mut res = Vec::with_capacity(children.len() * 2);
    for cid in children {
        res.push(cid.to_raw() as i32);
        res.push(state.dom.node_type(cid) as i32);
    }
    res
}

#[op2(fast)]
#[smi]
pub fn op_dom_get_first_child(#[state] state: &DomState, #[smi] node_id: i32) -> i32 {
    state
        .dom
        .get(NodeId::from_raw(node_id as u32))
        .and_then(|n| n.first_child)
        .map(|id| id.to_raw() as i32)
        .unwrap_or(-1)
}

#[op2(fast)]
#[smi]
pub fn op_dom_get_last_child(#[state] state: &DomState, #[smi] node_id: i32) -> i32 {
    state
        .dom
        .get(NodeId::from_raw(node_id as u32))
        .and_then(|n| n.last_child)
        .map(|id| id.to_raw() as i32)
        .unwrap_or(-1)
}

#[op2(fast)]
#[smi]
pub fn op_dom_get_next_sibling(#[state] state: &DomState, #[smi] node_id: i32) -> i32 {
    state
        .dom
        .get(NodeId::from_raw(node_id as u32))
        .and_then(|n| n.next_sibling)
        .map(|id| id.to_raw() as i32)
        .unwrap_or(-1)
}

#[op2(fast)]
#[smi]
pub fn op_dom_get_prev_sibling(#[state] state: &DomState, #[smi] node_id: i32) -> i32 {
    state
        .dom
        .get(NodeId::from_raw(node_id as u32))
        .and_then(|n| n.prev_sibling)
        .map(|id| id.to_raw() as i32)
        .unwrap_or(-1)
}

#[op2(fast)]
#[smi]
pub fn op_dom_query_selector(
    #[state] state: &DomState,
    #[smi] node_id: i32,
    #[string] selector: &str,
) -> i32 {
    let id = NodeId::from_raw(node_id as u32);
    let element = match DomElement::new(&state.dom, id) {
        Some(el) => el,
        None => {
            // For Document node, search from first element child
            let children = state.dom.child_elements(id);
            if children.is_empty() {
                return -1;
            }
            match DomElement::new(&state.dom, children[0]) {
                Some(el) => {
                    // Search from root element
                    if let Ok(Some(found)) = css_selectors::query_selector(&el, selector) {
                        return found.node_id().to_raw() as i32;
                    }
                    return -1;
                }
                None => return -1,
            }
        }
    };
    match css_selectors::query_selector(&element, selector) {
        Ok(Some(found)) => found.node_id().to_raw() as i32,
        _ => -1,
    }
}

#[op2]
#[serde]
pub fn op_dom_query_selector_all(
    #[state] state: &DomState,
    #[smi] node_id: i32,
    #[string] selector: String,
) -> Vec<i32> {
    let id = NodeId::from_raw(node_id as u32);
    // For document or element, try to build a DomElement for querying
    let root_el = DomElement::new(&state.dom, id).or_else(|| {
        let children = state.dom.child_elements(id);
        children
            .first()
            .and_then(|&c| DomElement::new(&state.dom, c))
    });
    match root_el {
        Some(el) => css_selectors::query_selector_all(&el, &selector)
            .unwrap_or_default()
            .iter()
            .map(|e| e.node_id().to_raw() as i32)
            .collect(),
        None => vec![],
    }
}

#[op2(fast)]
#[smi]
pub fn op_dom_get_element_by_id(#[state] state: &DomState, #[string] id: &str) -> i32 {
    state
        .dom
        .get_element_by_id(id)
        .map(|n| n.to_raw() as i32)
        .unwrap_or(-1)
}

#[op2]
#[serde]
pub fn op_dom_get_elements_by_tag_name(
    #[state] state: &DomState,
    #[smi] node_id: i32,
    #[string] tag: String,
) -> Vec<i32> {
    state
        .dom
        .get_elements_by_tag_name(NodeId::from_raw(node_id as u32), &tag)
        .iter()
        .map(|id| id.to_raw() as i32)
        .collect()
}

#[op2]
#[serde]
pub fn op_dom_get_elements_by_class_name(
    #[state] state: &DomState,
    #[smi] node_id: i32,
    #[string] class: String,
) -> Vec<i32> {
    state
        .dom
        .get_elements_by_class_name(NodeId::from_raw(node_id as u32), &class)
        .iter()
        .map(|id| id.to_raw() as i32)
        .collect()
}

// --- Mutation ops ---

#[op2(fast)]
#[smi]
pub fn op_dom_create_element(#[state] state: &mut DomState, #[string] tag: &str) -> i32 {
    state
        .dom
        .create_element(dom::node::QualName::new(tag), vec![])
        .to_raw() as i32
}

#[op2(fast)]
#[smi]
pub fn op_dom_create_text_node(#[state] state: &mut DomState, #[string] text: &str) -> i32 {
    state.dom.create_text(text.to_string()).to_raw() as i32
}

#[op2(fast)]
#[smi]
pub fn op_dom_create_document_fragment(#[state] state: &mut DomState) -> i32 {
    state.dom.create_document_fragment().to_raw() as i32
}

#[op2(fast)]
pub fn op_dom_append_child(#[state] state: &mut DomState, #[smi] parent: i32, #[smi] child: i32) {
    state.dom.append_child(
        NodeId::from_raw(parent as u32),
        NodeId::from_raw(child as u32),
    );
    state.layout_engine.mark_dirty();
}

#[op2(fast)]
pub fn op_dom_insert_before(
    #[state] state: &mut DomState,
    #[smi] parent: i32,
    #[smi] child: i32,
    #[smi] reference: i32,
) {
    state.dom.insert_before(
        NodeId::from_raw(parent as u32),
        NodeId::from_raw(child as u32),
        NodeId::from_raw(reference as u32),
    );
    state.layout_engine.mark_dirty();
}

#[op2(fast)]
pub fn op_dom_remove_child(#[state] state: &mut DomState, #[smi] _parent: i32, #[smi] child: i32) {
    state.dom.detach(NodeId::from_raw(child as u32));
    state.layout_engine.mark_dirty();
}

#[op2(fast)]
pub fn op_dom_set_attribute(
    #[state] state: &mut DomState,
    #[smi] node_id: i32,
    #[string] name: &str,
    #[string] value: &str,
) {
    let id = NodeId::from_raw(node_id as u32);
    if let Some(node) = state.dom.get_mut(id) {
        if let Some(elem) = node.as_element_mut() {
            if let Some(attr) = elem
                .attrs
                .iter_mut()
                .find(|a| a.name.local.eq_ignore_ascii_case(name))
            {
                attr.value = value.to_string();
            } else {
                elem.attrs.push(dom::node::Attribute {
                    name: dom::node::QualName::new(name),
                    value: value.to_string(),
                });
            }
        }
    }
    if name.eq_ignore_ascii_case("style") || name.eq_ignore_ascii_case("class") {
        state.layout_engine.mark_dirty();
    }
}

#[op2(fast)]
pub fn op_dom_remove_attribute(
    #[state] state: &mut DomState,
    #[smi] node_id: i32,
    #[string] name: &str,
) {
    let id = NodeId::from_raw(node_id as u32);
    if let Some(node) = state.dom.get_mut(id) {
        if let Some(elem) = node.as_element_mut() {
            elem.attrs
                .retain(|a| !a.name.local.eq_ignore_ascii_case(name));
        }
    }
    if name.eq_ignore_ascii_case("style") || name.eq_ignore_ascii_case("class") {
        state.layout_engine.mark_dirty();
    }
}

#[op2(fast)]
pub fn op_dom_set_text_content(
    #[state] state: &mut DomState,
    #[smi] node_id: i32,
    #[string] text: &str,
) {
    state
        .dom
        .set_text_content(NodeId::from_raw(node_id as u32), text);
    state.layout_engine.mark_dirty();
}

#[op2(fast)]
pub fn op_dom_set_inner_html(
    #[state] state: &mut DomState,
    #[smi] node_id: i32,
    #[string] html: &str,
) {
    let id = NodeId::from_raw(node_id as u32);
    let fragment_dom = html_parser::parse_html(&format!("<body>{}</body>", html));
    let body = fragment_dom
        .get_elements_by_tag_name(NodeId::DOCUMENT, "body")
        .into_iter()
        .next();

    // Remove existing children
    let old_children: Vec<NodeId> = state.dom.children(id);
    for child in old_children {
        state.dom.remove(child);
    }

    // Merge fragment children
    if let Some(body_id) = body {
        for child_id in fragment_dom.children(body_id) {
            let new_child = state.dom.merge_subtree(&fragment_dom, child_id);
            state.dom.append_child(id, new_child);
        }
    }
    state.layout_engine.mark_dirty();
}

/// Clone a node. If deep=true, clone all descendants too.
#[op2(fast)]
#[smi]
pub fn op_dom_clone_node(#[state] state: &mut DomState, #[smi] node_id: i32, deep: bool) -> i32 {
    let id = NodeId::from_raw(node_id as u32);
    if deep {
        // merge_subtree does a deep copy from the same DOM
        let cloned = {
            // We need to read from &self and write to &mut self.
            // merge_subtree takes &Dom for source. Build a snapshot of the subtree.
            // Actually, we can use a two-pass: first collect the tree shape, then rebuild.
            clone_subtree_deep(&mut state.dom, id)
        };
        cloned.to_raw() as i32
    } else {
        // Shallow: copy just this node (no children)
        let node = match state.dom.get(id) {
            Some(n) => n,
            None => return -1,
        };
        let new_id = match &node.data {
            dom::node::NodeData::Element(elem) => state
                .dom
                .create_element(elem.name.clone(), elem.attrs.clone()),
            dom::node::NodeData::Text(t) => state.dom.create_text(t.clone()),
            dom::node::NodeData::Comment(t) => state.dom.create_comment(t.clone()),
            _ => state.dom.create_document_fragment(),
        };
        new_id.to_raw() as i32
    }
}

/// Deep clone a subtree within the same Dom.
fn clone_subtree_deep(dom: &mut dom::Dom, root: NodeId) -> NodeId {
    // Collect the tree structure first (read phase)
    let snapshot = collect_subtree(dom, root);
    // Rebuild from snapshot (write phase)
    rebuild_from_snapshot(dom, &snapshot)
}

#[derive(Debug)]
enum SnapshotNode {
    Element {
        name: dom::node::QualName,
        attrs: Vec<dom::node::Attribute>,
        children: Vec<SnapshotNode>,
    },
    Text(String),
    Comment(String),
    Fragment(Vec<SnapshotNode>),
}

fn collect_subtree(dom: &dom::Dom, id: NodeId) -> SnapshotNode {
    let node = match dom.get(id) {
        Some(n) => n,
        None => return SnapshotNode::Fragment(vec![]),
    };
    let children: Vec<SnapshotNode> = dom
        .children(id)
        .iter()
        .map(|&child_id| collect_subtree(dom, child_id))
        .collect();
    match &node.data {
        dom::node::NodeData::Element(elem) => SnapshotNode::Element {
            name: elem.name.clone(),
            attrs: elem.attrs.clone(),
            children,
        },
        dom::node::NodeData::Text(t) => SnapshotNode::Text(t.clone()),
        dom::node::NodeData::Comment(t) => SnapshotNode::Comment(t.clone()),
        _ => SnapshotNode::Fragment(children),
    }
}

fn rebuild_from_snapshot(dom: &mut dom::Dom, snapshot: &SnapshotNode) -> NodeId {
    match snapshot {
        SnapshotNode::Element {
            name,
            attrs,
            children,
        } => {
            let id = dom.create_element(name.clone(), attrs.clone());
            for child in children {
                let child_id = rebuild_from_snapshot(dom, child);
                dom.append_child(id, child_id);
            }
            id
        }
        SnapshotNode::Text(t) => dom.create_text(t.clone()),
        SnapshotNode::Comment(t) => dom.create_comment(t.clone()),
        SnapshotNode::Fragment(children) => {
            let id = dom.create_document_fragment();
            for child in children {
                let child_id = rebuild_from_snapshot(dom, child);
                dom.append_child(id, child_id);
            }
            id
        }
    }
}

/// Insert HTML at a position relative to an element.
/// position: "beforebegin", "afterbegin", "beforeend", "afterend"
#[op2(fast)]
pub fn op_dom_insert_adjacent_html(
    #[state] state: &mut DomState,
    #[smi] node_id: i32,
    #[string] position: &str,
    #[string] html: &str,
) {
    let id = NodeId::from_raw(node_id as u32);
    let fragment_dom = html_parser::parse_html(&format!("<body>{}</body>", html));
    let frag_body = fragment_dom
        .get_elements_by_tag_name(NodeId::DOCUMENT, "body")
        .into_iter()
        .next();
    let frag_children: Vec<NodeId> = frag_body
        .map(|b| fragment_dom.children(b))
        .unwrap_or_default();
    if frag_children.is_empty() {
        return;
    }

    match position {
        "beforebegin" => {
            // Insert before this element (as previous sibling)
            if let Some(parent) = state.dom.get(id).and_then(|n| n.parent) {
                for &child_id in &frag_children {
                    let new_child = state.dom.merge_subtree(&fragment_dom, child_id);
                    state.dom.insert_before(parent, new_child, id);
                }
            }
        }
        "afterbegin" => {
            // Insert as first child
            let first = state.dom.get(id).and_then(|n| n.first_child);
            for child_id in frag_children.iter().rev() {
                let new_child = state.dom.merge_subtree(&fragment_dom, *child_id);
                if let Some(ref_child) = first {
                    state.dom.insert_before(id, new_child, ref_child);
                } else {
                    state.dom.append_child(id, new_child);
                }
            }
        }
        "beforeend" => {
            // Append as last child (same as appendChild)
            for &child_id in &frag_children {
                let new_child = state.dom.merge_subtree(&fragment_dom, child_id);
                state.dom.append_child(id, new_child);
            }
        }
        "afterend" => {
            // Insert after this element (as next sibling)
            if let Some(parent) = state.dom.get(id).and_then(|n| n.parent) {
                let next = state.dom.get(id).and_then(|n| n.next_sibling);
                for &child_id in &frag_children {
                    let new_child = state.dom.merge_subtree(&fragment_dom, child_id);
                    if let Some(ref_child) = next {
                        state.dom.insert_before(parent, new_child, ref_child);
                    } else {
                        state.dom.append_child(parent, new_child);
                    }
                }
            }
        }
        _ => {}
    }
    state.layout_engine.mark_dirty();
}

#[op2]
#[serde]
pub fn op_dom_document_write(#[state] state: &mut DomState, #[string] html: &str) -> Vec<i32> {
    let body_id = state
        .dom
        .get_elements_by_tag_name(NodeId::DOCUMENT, "body")
        .into_iter()
        .next();
    let body_id = match body_id {
        Some(id) => id,
        None => return vec![],
    };
    let fragment_dom = html_parser::parse_html(&format!("<body>{}</body>", html));
    let frag_body = fragment_dom
        .get_elements_by_tag_name(NodeId::DOCUMENT, "body")
        .into_iter()
        .next();
    let mut new_ids = Vec::new();
    if let Some(frag_body_id) = frag_body {
        for child_id in fragment_dom.children(frag_body_id) {
            let new_child = state.dom.merge_subtree(&fragment_dom, child_id);
            state.dom.append_child(body_id, new_child);
            new_ids.push(new_child.to_raw() as i32);
        }
    }
    state.layout_engine.mark_dirty();
    new_ids
}

#[op2(fast)]
pub fn op_dom_class_list_add(
    #[state] state: &mut DomState,
    #[smi] node_id: i32,
    #[string] class: &str,
) {
    let id = NodeId::from_raw(node_id as u32);
    if let Some(node) = state.dom.get_mut(id) {
        if let Some(elem) = node.as_element_mut() {
            let current = elem
                .attrs
                .iter()
                .find(|a| a.name.local == "class")
                .map(|a| a.value.clone())
                .unwrap_or_default();
            if !current.split_whitespace().any(|c| c == class) {
                let new_val = if current.is_empty() {
                    class.to_string()
                } else {
                    format!("{} {}", current, class)
                };
                if let Some(attr) = elem.attrs.iter_mut().find(|a| a.name.local == "class") {
                    attr.value = new_val;
                } else {
                    elem.attrs.push(dom::node::Attribute {
                        name: dom::node::QualName::new("class"),
                        value: new_val,
                    });
                }
            }
        }
    }
}

#[op2(fast)]
pub fn op_dom_class_list_remove(
    #[state] state: &mut DomState,
    #[smi] node_id: i32,
    #[string] class: &str,
) {
    let id = NodeId::from_raw(node_id as u32);
    if let Some(node) = state.dom.get_mut(id) {
        if let Some(elem) = node.as_element_mut() {
            if let Some(attr) = elem.attrs.iter_mut().find(|a| a.name.local == "class") {
                let new_val: String = attr
                    .value
                    .split_whitespace()
                    .filter(|c| *c != class)
                    .collect::<Vec<_>>()
                    .join(" ");
                attr.value = new_val;
            }
        }
    }
}

/// Get computed style for an element.
/// Checks: 1) inline style attribute, 2) `<style>` block rules, 3) CSS defaults.
/// Uses selector matching for style block rules. Higher specificity wins.
#[op2]
#[string]
pub fn op_dom_get_computed_style(
    #[state] state: &DomState,
    #[smi] node_id: i32,
    #[string] property: &str,
) -> String {
    let id = NodeId::from_raw(node_id as u32);

    // 1. Check inline style (highest specificity)
    let inline_val = get_inline_style_value(&state.dom, id, property);
    if let Some(val) = &inline_val {
        if !val.is_empty() {
            return val.clone();
        }
    }

    // 2. Check <style> block rules (matched by selector)
    if let Some(val) = get_stylesheet_value(state, id, property) {
        return val;
    }

    // 3. CSS inheritance — walk up the DOM for inherited properties
    const INHERITED: &[&str] = &[
        "color",
        "font-family",
        "font-size",
        "font-style",
        "font-weight",
        "font-variant",
        "line-height",
        "letter-spacing",
        "word-spacing",
        "text-align",
        "text-indent",
        "text-transform",
        "white-space",
        "direction",
        "visibility",
        "cursor",
        "list-style-type",
        "list-style-position",
        "list-style-image",
        "list-style",
        "border-collapse",
        "border-spacing",
        "caption-side",
        "empty-cells",
        "quotes",
        "orphans",
        "widows",
        "text-decoration-color",
    ];

    if INHERITED.contains(&property) {
        let mut current = id;
        while let Some(parent_id) = state.dom.get(current).and_then(|n| n.parent) {
            if let Some(val) = get_inline_style_value(&state.dom, parent_id, property) {
                if !val.is_empty() {
                    return val;
                }
            }
            if let Some(val) = get_stylesheet_value(state, parent_id, property) {
                return val;
            }
            current = parent_id;
        }
    }

    // 4. CSS default
    crate::extensions::layout_ext::css_default(property)
}

/// Extract a property value from an element's inline style attribute.
fn get_inline_style_value(dom: &dom::Dom, id: NodeId, property: &str) -> Option<String> {
    let style_attr = dom.get(id).and_then(|n| n.as_element()).and_then(|e| {
        e.attrs
            .iter()
            .find(|a| a.name.local.eq_ignore_ascii_case("style"))
            .map(|a| a.value.clone())
    })?;

    for decl in style_attr.split(';') {
        let decl = decl.trim();
        if decl.is_empty() {
            continue;
        }
        if let Some(colon) = decl.find(':') {
            let prop = decl[..colon].trim();
            let val = decl[colon + 1..].trim();
            if prop.eq_ignore_ascii_case(property) {
                return Some(val.to_string());
            }
        }
    }
    None
}

/// Search <style> block rules for a matching declaration.
/// Returns the value from the highest-specificity matching rule.
fn get_stylesheet_value(state: &DomState, id: NodeId, property: &str) -> Option<String> {
    let dom_el = DomElement::new(&state.dom, id)?;

    // Collect all matching declarations: (specificity, source_order, value)
    let mut matches: Vec<(u32, u32, String)> = Vec::new();
    let mut source_order: u32 = 0;

    for css_text in &state.stylesheets {
        let (stylesheet, _errors) = css_parser::parse_stylesheet(css_text);
        for rule in &stylesheet.rules {
            if let css_parser::ast::Rule::Qualified(qr) = rule {
                // Convert prelude tokens to selector string
                let selector_str = tokens_to_string(&qr.prelude);
                if selector_str.is_empty() {
                    continue;
                }

                // Parse selector and check if it matches
                let selectors = match css_selectors::parse_selector_list(&selector_str) {
                    Ok(s) => s,
                    Err(_) => continue,
                };

                let mut matched = false;
                let mut best_spec: u32 = 0;
                for sel in &selectors {
                    if css_selectors::matches_selector(&dom_el, sel) {
                        matched = true;
                        let s = css_selectors::compute_specificity(sel);
                        let spec = s.a * 10000 + s.b * 100 + s.c;
                        if spec > best_spec {
                            best_spec = spec;
                        }
                    }
                }

                if matched {
                    // Check declarations for the requested property
                    for decl in &qr.declarations {
                        if decl.name.eq_ignore_ascii_case(property) {
                            let val = tokens_to_string(&decl.value).trim().to_string();
                            matches.push((best_spec, source_order, val));
                        }
                    }
                }
                source_order += 1;
            }
        }
    }

    if matches.is_empty() {
        return None;
    }

    // Sort by specificity (ascending), then source order — last wins
    matches.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

    // Winner is the last entry (highest specificity, latest source order)
    matches.last().map(|(_, _, val)| val.clone())
}

/// Convert CSS component values back to a string.
fn tokens_to_string(values: &[css_parser::ast::ComponentValue]) -> String {
    use css_parser::token::TokenKind;
    let mut s = String::new();
    for v in values {
        match v {
            css_parser::ast::ComponentValue::Token(t) => match &t.kind {
                TokenKind::Ident(name) => s.push_str(name),
                TokenKind::String(val) => {
                    s.push('"');
                    s.push_str(val);
                    s.push('"');
                }
                TokenKind::Hash { value, .. } => {
                    s.push('#');
                    s.push_str(value);
                }
                TokenKind::Number { value, .. } => s.push_str(&value.to_string()),
                TokenKind::Percentage { value, .. } => {
                    s.push_str(&value.to_string());
                    s.push('%');
                }
                TokenKind::Dimension { value, unit, .. } => {
                    s.push_str(&value.to_string());
                    s.push_str(unit);
                }
                TokenKind::Whitespace => s.push(' '),
                TokenKind::Delim(c) => s.push(*c),
                TokenKind::Colon => s.push(':'),
                TokenKind::Semicolon => s.push(';'),
                TokenKind::Comma => s.push(','),
                TokenKind::OpenParen => s.push('('),
                TokenKind::CloseParen => s.push(')'),
                TokenKind::OpenSquare => s.push('['),
                TokenKind::CloseSquare => s.push(']'),
                TokenKind::Function(name) => {
                    s.push_str(name);
                    s.push('(');
                }
                _ => {}
            },
            css_parser::ast::ComponentValue::Function(f) => {
                s.push_str(f.name);
                s.push('(');
                s.push_str(&tokens_to_string(&f.arguments));
                s.push(')');
            }
            css_parser::ast::ComponentValue::SimpleBlock(b) => {
                s.push_str(&tokens_to_string(&b.value));
            }
        }
    }
    s
}

// --- Shadow DOM ops ---

/// Attach a shadow root to an element. Returns the shadow root node ID.
#[op2(fast)]
#[smi]
pub fn op_dom_attach_shadow(
    #[state] state: &mut DomState,
    #[smi] node_id: i32,
    #[string] mode: &str,
) -> i32 {
    let id = NodeId::from_raw(node_id as u32);
    let shadow_mode = match mode {
        "closed" => dom::node::ShadowRootMode::Closed,
        _ => dom::node::ShadowRootMode::Open,
    };
    let shadow_id = state.dom.create_shadow_root(id, shadow_mode);
    shadow_id.to_raw() as i32
}

/// Get shadow root of an element (-1 if none).
#[op2(fast)]
#[smi]
pub fn op_dom_get_shadow_root(#[state] state: &DomState, #[smi] node_id: i32) -> i32 {
    let id = NodeId::from_raw(node_id as u32);
    state
        .dom
        .get(id)
        .and_then(|n| n.as_element())
        .and_then(|e| e.shadow_root)
        .map(|sr| sr.to_raw() as i32)
        .unwrap_or(-1)
}

// --- CSSOM ops ---

#[op2(fast)]
pub fn op_dom_get_stylesheet_count(#[state] state: &DomState) -> i32 {
    state.stylesheets.len() as i32
}

#[derive(serde::Serialize)]
pub struct CSSRuleJson {
    pub selector_text: String,
    pub css_text: String,
    pub rule_type: u8,
}

/// Get parsed rules for a stylesheet by index.
#[op2]
#[serde]
pub fn op_dom_get_stylesheet_rules(
    #[state] state: &DomState,
    #[smi] index: i32,
) -> Vec<CSSRuleJson> {
    let idx = index as usize;
    if idx >= state.stylesheets.len() {
        return vec![];
    }
    let (stylesheet, _errors) = css_parser::parse_stylesheet(&state.stylesheets[idx]);
    let mut rules = Vec::new();
    for rule in &stylesheet.rules {
        if let css_parser::ast::Rule::Qualified(qr) = rule {
            let selector_text = tokens_to_string(&qr.prelude);
            if selector_text.is_empty() {
                continue;
            }
            let decl_parts: Vec<String> = qr
                .declarations
                .iter()
                .map(|d| {
                    let val = tokens_to_string(&d.value).trim().to_string();
                    if d.important {
                        format!("{}: {} !important", d.name, val)
                    } else {
                        format!("{}: {}", d.name, val)
                    }
                })
                .collect();
            let css_text = format!("{} {{ {} }}", selector_text, decl_parts.join("; "));
            rules.push(CSSRuleJson {
                selector_text: selector_text.trim().to_string(),
                css_text,
                rule_type: 1, // CSSStyleRule
            });
        }
    }
    rules
}

#[op2]
#[string]
pub fn op_dom_get_base_url(#[state] state: &DomState) -> String {
    state
        .base_url
        .as_ref()
        .map(|u| u.to_string())
        .unwrap_or_else(|| "about:blank".to_string())
}

#[op2]
#[string]
pub fn op_dom_storage_get(
    #[state] state: &DomState,
    #[string] area: String,
    #[string] key: String,
) -> Option<String> {
    state.storage.get(&area).and_then(|m| m.get(&key)).cloned()
}

#[op2(fast)]
pub fn op_dom_storage_set(
    #[state] state: &mut DomState,
    #[string] area: String,
    #[string] key: String,
    #[string] value: String,
) {
    if let Some(m) = state.storage.get_mut(&area) {
        m.insert(key, value);
    }
}

#[op2(fast)]
pub fn op_dom_storage_remove(
    #[state] state: &mut DomState,
    #[string] area: String,
    #[string] key: String,
) {
    if let Some(m) = state.storage.get_mut(&area) {
        m.remove(&key);
    }
}

#[op2(fast)]
pub fn op_dom_storage_clear(#[state] state: &mut DomState, #[string] area: String) {
    if let Some(m) = state.storage.get_mut(&area) {
        m.clear();
    }
}

#[op2]
#[serde]
pub fn op_dom_storage_keys(#[state] state: &DomState, #[string] area: String) -> Vec<String> {
    state
        .storage
        .get(&area)
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default()
}

deno_core::extension!(
    dom_extension,
    ops = [
        op_dom_document_node,
        op_dom_get_tag_name,
        op_dom_get_node_type,
        op_dom_get_text_content,
        op_dom_get_inner_html,
        op_dom_get_outer_html,
        op_dom_get_attribute,
        op_dom_has_attribute,
        op_dom_get_attribute_names,
        op_dom_get_parent,
        op_dom_get_children,
        op_dom_get_children_with_types,
        op_dom_get_child_elements,
        op_dom_get_child_elements_with_types,
        op_dom_get_first_child,
        op_dom_get_last_child,
        op_dom_get_next_sibling,
        op_dom_get_prev_sibling,
        op_dom_query_selector,
        op_dom_query_selector_all,
        op_dom_get_element_by_id,
        op_dom_get_elements_by_tag_name,
        op_dom_get_elements_by_class_name,
        op_dom_create_element,
        op_dom_create_text_node,
        op_dom_create_document_fragment,
        op_dom_append_child,
        op_dom_insert_before,
        op_dom_remove_child,
        op_dom_set_attribute,
        op_dom_remove_attribute,
        op_dom_set_text_content,
        op_dom_set_inner_html,
        op_dom_document_write,
        op_dom_clone_node,
        op_dom_insert_adjacent_html,
        op_dom_class_list_add,
        op_dom_class_list_remove,
        op_dom_get_computed_style,
        op_dom_get_stylesheet_count,
        op_dom_get_stylesheet_rules,
        op_dom_attach_shadow,
        op_dom_get_shadow_root,
        op_dom_get_base_url,
        op_dom_storage_get,
        op_dom_storage_set,
        op_dom_storage_remove,
        op_dom_storage_clear,
        op_dom_storage_keys,
    ],
);
