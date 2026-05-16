use std::collections::HashMap;
use crate::native_fns::{install_native_fp_tostring, IframeRealmStore};
use crate::state::DomState;
use crate::utils::tokens_to_string;
use css_values::calc::resolve_computed_value;
use css_values::types::length::CalcContext;
use deno_core::op2;
use deno_core::v8;
use deno_core::JsRuntime;
use dom::node::NodeId;
use dom::DomElement;

/// Build a `CalcContext` from the current DOM state's stealth profile.
/// Provides viewport + font-size + container dimensions so calc()
/// math functions can resolve relative units (vw, em, etc.) correctly
/// for `getComputedStyle` resolution.
fn calc_context_from(state: &DomState) -> CalcContext {
    let mut ctx = CalcContext::default();
    if let Some(p) = state.stealth_profile.as_ref() {
        ctx.viewport_w = p.inner_width as f64;
        ctx.viewport_h = p.inner_height as f64;
        ctx.container_w = p.inner_width as f64;
        ctx.container_h = p.inner_height as f64;
        // 16px is Chrome's default; profiles don't currently override.
        ctx.root_font_size_px = 16.0;
        ctx.font_size_px = 16.0;
    }
    ctx
}

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
#[serde]
pub fn op_dom_get_all_computed_styles(
    #[state] state: &mut DomState,
    #[smi] node_id: i32,
) -> HashMap<String, String> {
    if state.cached_rules.is_empty() && !state.stylesheets.is_empty() {
        state.update_cached_rules();
    }
    let id = NodeId::from_raw(node_id as u32);
    let dom_el = if let Some(el) = DomElement::new(&state.dom, id) {
        el
    } else {
        return HashMap::new();
    };

    let mut declarations: HashMap<String, (u32, u32, String)> = HashMap::new();
    let mut source_order: u32 = 0;

    for rule in &state.cached_rules {
        for sel in &rule.selectors {
            if css_selectors::matches_selector(&dom_el, sel) {
                let s = css_selectors::compute_specificity(sel);
                let spec = s.a * 10000 + s.b * 100 + s.c;
                for (name, val) in &rule.declarations {
                    let entry = declarations
                        .entry(name.clone())
                        .or_insert((0, 0, String::new()));
                    if spec > entry.0 || (spec == entry.0 && source_order >= entry.1) {
                        *entry = (spec, source_order, val.clone());
                    }
                }
            }
        }
        source_order += 1;
    }

    // Add inline styles (highest specificity)
    if let Some(el) = state.dom.get(id).and_then(|n| n.as_element()) {
        if let Some(style) = el
            .attrs
            .iter()
            .find(|a| a.name.local.eq_ignore_ascii_case("style"))
        {
            for decl in style.value.split(';') {
                if let Some(colon) = decl.find(':') {
                    let name = decl[..colon].trim().to_string();
                    let val = decl[colon + 1..].trim().to_string();
                    declarations.insert(name, (999999, 999999, val));
                }
            }
        }
    }

    // Resolve calc() and CSS Values 4 math functions to their used
    // pixel value before returning — Chrome's getComputedStyle does
    // this. Otherwise antibot probes that compute via calc(... sin(pi)
    // ...) and read the result back (e.g. Kasada's CSS calc precision
    // probe — see docs/CANADA_GOOSE_DIAGNOSIS_2026_05_10.md) catch us
    // returning the unresolved expression text.
    let ctx = calc_context_from(state);
    let res: HashMap<String, String> = declarations
        .into_iter()
        .map(|(k, v)| (k, resolve_computed_value(&v.2, &ctx)))
        .collect();
    res
}

#[op2]
#[string]
pub fn op_dom_get_computed_style(
    #[state] state: &mut DomState,
    #[smi] node_id: i32,
    #[string] property: &str,
) -> String {
    if state.cached_rules.is_empty() && !state.stylesheets.is_empty() {
        state.update_cached_rules();
    }
    let id = NodeId::from_raw(node_id as u32);
    let ctx = calc_context_from(state);

    // 1. Check inline style (highest specificity)
    let inline_val = get_inline_style_value(&state.dom, id, property);
    if let Some(val) = &inline_val {
        if !val.is_empty() {
            return resolve_computed_value(val, &ctx);
        }
    }

    // 2. Check <style> block rules (matched by selector)
    if let Some(val) = get_stylesheet_value(state, id, property) {
        return resolve_computed_value(&val, &ctx);
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
                    return resolve_computed_value(&val, &ctx);
                }
            }
            if let Some(val) = get_stylesheet_value(state, parent_id, property) {
                return resolve_computed_value(&val, &ctx);
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

    for rule in &state.cached_rules {
        let mut matched = false;
        let mut best_spec: u32 = 0;
        for sel in &rule.selectors {
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
            if let Some(val) = rule.declarations.get(property) {
                matches.push((best_spec, source_order, val.clone()));
            }
        }
        source_order += 1;
    }

    if matches.is_empty() {
        return None;
    }

    // Sort by specificity (ascending), then source order — last wins
    matches.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

    // Winner is the last entry (highest specificity, latest source order)
    matches.last().map(|(_, _, val)| val.clone())
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

// ──────────────────────────────────────────────────────────────────
// Child-realm support (§4 of 2026-05-15 handoff / doc 27 §5.2)
// ──────────────────────────────────────────────────────────────────

/// Window constructor callback — throws per the spec ("Illegal constructor").
/// Used only to create a real, named `Window` function whose `.name === "Window"`
/// and whose `.prototype.constructor === Window`.  Kasada never calls `new Window()`,
/// so the throw body is never reached; we keep it for spec correctness.
fn _window_ctor_cb(
    scope: &mut v8::HandleScope,
    _args: v8::FunctionCallbackArguments,
    mut _rv: v8::ReturnValue,
) {
    if let Some(msg) = v8::String::new(scope, "Illegal constructor") {
        let e = v8::Exception::type_error(scope, msg);
        scope.throw_exception(e);
    }
}

/// Create (or return cached) a genuine `v8::Context` child realm for an iframe's
/// `contentWindow`.  Returns the child global as a live JS object — NOT a Proxy.
///
/// The child context gets:
/// - Real, realm-distinct native intrinsics (`Object`/`Function`/`Array`/… ≠ parent's)
///   — defeats Kasada's `addContentWindowProxy` + realm-divergence bail (doc 26).
/// - `[[Prototype]] === Window.prototype` → `cw.constructor.name === "Window"`.
/// - Genuine-native `Function.prototype.toString` (same API-fn recipe as the main window).
/// - Standard self-referential globals (`window`, `self`, `globalThis`, `frames`).
///
/// JS completes the setup by setting `document`, `location`, `navigator`, `fetch`,
/// `devicePixelRatio` (accessor), etc. on the returned object.
#[op2]
pub fn op_create_child_realm<'s>(
    scope: &mut v8::HandleScope<'s>,
    #[smi] realm_id: i32,
) -> v8::Local<'s, v8::Value> {
    let rid = realm_id as u32;

    // Access OpState via the isolate-level state (public, stable in 0.311).
    // op_state_from takes &Isolate; HandleScope auto-derefs there.
    let op_state_rc = JsRuntime::op_state_from(scope);

    // Fast path: cached realm — return the previously-created global.
    {
        let op_state = op_state_rc.borrow();
        if let Some(store) = op_state.try_borrow::<IframeRealmStore>() {
            if let Some(global) = store.globals.get(&rid) {
                return v8::Local::new(scope, global).into();
            }
        }
    }

    // Clone the `orig_fp_tostring` and `native_tag_sym` Globals into new
    // handles BEFORE entering the child ContextScope (requires parent scope).
    let orig_fpt: Option<v8::Global<v8::Function>>;
    let native_tag_sym: Option<v8::Global<v8::Symbol>>;
    {
        let op_state = op_state_rc.borrow();
        if let Some(store) = op_state.try_borrow::<IframeRealmStore>() {
            orig_fpt = store.orig_fp_tostring.as_ref().map(|g| {
                let local = v8::Local::new(scope, g);
                v8::Global::new(scope, local)
            });
            native_tag_sym = store.native_tag_sym.as_ref().map(|g| {
                let local = v8::Local::new(scope, g);
                v8::Global::new(scope, local)
            });
        } else {
            orig_fpt = None;
            native_tag_sym = None;
        }
    }

    // Create the child context (vanilla v8::Context — full native intrinsics).
    let child_ctx = v8::Context::new(scope, v8::ContextOptions::default());

    // Copy parent's security token to child so V8 treats the contexts as
    // same-origin (about:blank inherits the parent origin in Chrome).
    // Without this, accessing child-realm objects from the parent scope
    // throws "TypeError: no access" via V8's cross-context security check.
    let parent_ctx = scope.get_current_context();
    let parent_tok = parent_ctx.get_security_token(scope);
    child_ctx.set_security_token(parent_tok);

    // Set up the child context.  Returns None on any fatal V8 allocation
    // failure (extremely rare); the outer code falls back to undefined.
    let child_global_g: Option<v8::Global<v8::Object>> = {
        let cs = &mut v8::ContextScope::new(scope, child_ctx);

        // Build a real `Window` function (FunctionTemplate → native `[native code]`)
        // so the child global is typed: `constructor.name === "Window"`.
        let window_tmpl = v8::FunctionTemplate::new(cs, _window_ctor_cb);
        if let Some(n) = v8::String::new(cs, "Window") {
            window_tmpl.set_class_name(n);
        }
        let window_fn = match window_tmpl.get_function(cs) {
            Some(f) => f,
            None => return v8::undefined(cs).into(),
        };
        if let Some(n) = v8::String::new(cs, "Window") {
            window_fn.set_name(n);
        }

        // child_global.[[Prototype]] = Window.prototype
        // → child_global.constructor.name === "Window"
        if let Some(pk) = v8::String::new(cs, "prototype") {
            if let Some(proto_val) = window_fn.get(cs, pk.into()) {
                let child_global = child_ctx.global(cs);
                child_global.set_prototype(cs, proto_val);
            }
        }

        let child_global = child_ctx.global(cs);

        // Expose Window on child global (Kasada reads `contentWindow.Window`).
        if let Some(k) = v8::String::new(cs, "Window") {
            child_global.set(cs, k.into(), window_fn.into());
        }

        // Standard self-referential globals (all point to child_global).
        for key in &["window", "self", "globalThis", "frames"] {
            if let Some(k) = v8::String::new(cs, key) {
                child_global.set(cs, k.into(), child_global.into());
            }
        }
        // length = 0  (avoid borrow-twice by staging the value first)
        if let Some(k) = v8::String::new(cs, "length") {
            let zero = v8::Integer::new(cs, 0);
            child_global.set(cs, k.into(), zero.into());
        }
        // opener = null
        if let Some(k) = v8::String::new(cs, "opener") {
            let null = v8::null(cs);
            child_global.set(cs, k.into(), null.into());
        }

        // Install genuine-native Function.prototype.toString in child realm.
        // Closes the [[SourceText]] leak for child-realm functions too.
        // Pass native_tag_sym (JS global registry) so tagged host fns
        // in the child realm stringify correctly via the Array-data path.
        if let Some(ref orig) = orig_fpt {
            install_native_fp_tostring(cs, orig, native_tag_sym.as_ref());
        }

        Some(v8::Global::new(cs, child_global))
    };

    let child_global_g = match child_global_g {
        Some(g) => g,
        None => return v8::undefined(scope).into(),
    };

    // Build Local from Global BEFORE moving Global into the store.
    let local: v8::Local<'s, v8::Value> = v8::Local::new(scope, &child_global_g).into();

    // Persist context (keeps it alive) and cache global in OpState.
    {
        let mut op_state = op_state_rc.borrow_mut();
        if let Some(store) = op_state.try_borrow_mut::<IframeRealmStore>() {
            store.contexts.insert(rid, v8::Global::new(scope, child_ctx));
            store.globals.insert(rid, child_global_g);
        }
    }

    local
}

/// Set a property on the INNER GLOBAL of a child realm.
///
/// The global proxy's own property dict is NOT visible to code running inside
/// the child realm (which reads from the inner global's scope chain). Setting
/// on the proxy via `proxy.set()` from Rust only writes to the proxy's own
/// Two-path write to guarantee visibility from both inside and outside the realm:
///
/// 1. `create_data_property` on the inner global (the JSGlobalObject behind the
///    GlobalProxy): makes the property an own property of the inner global, so
///    scope-chain lookups from scripts running INSIDE the realm find it.
///
/// 2. `proxy.set()` on the GlobalProxy: puts the property in the proxy's own
///    dictionary, so cross-context reads from the parent (`cw.screen`) find it.
///
/// Both paths are necessary: V8's API `Object::Set()` on a GlobalProxy writes to
/// the proxy's own dict (not the inner global), so scope-chain lookups inside the
/// realm miss it. Conversely, `create_data_property` on the inner global is NOT
/// reachable from a cross-context `proxy.property` read (the proxy's own dict is
/// checked first and exclusively for cross-context callers without the interceptor).
#[op2]
pub fn op_set_child_realm_prop<'s>(
    scope: &mut v8::HandleScope<'s>,
    #[smi] realm_id: i32,
    key: v8::Local<v8::Value>,
    value: v8::Local<v8::Value>,
) -> v8::Local<'s, v8::Value> {
    let rid = realm_id as u32;
    let op_state_rc = JsRuntime::op_state_from(scope);

    let child_ctx_g: Option<v8::Global<v8::Context>> = {
        let op_state = op_state_rc.borrow();
        op_state.try_borrow::<IframeRealmStore>().and_then(|store| {
            store.contexts.get(&rid).map(|g| {
                let local = v8::Local::new(scope, g);
                v8::Global::new(scope, local)
            })
        })
    };
    let child_ctx_g = match child_ctx_g {
        Some(g) => g,
        None => return v8::undefined(scope).into(),
    };

    let child_ctx = v8::Local::new(scope, &child_ctx_g);
    let cs = &mut v8::ContextScope::new(scope, child_ctx);
    let child_proxy = child_ctx.global(cs);

    // Path 1: inner global own property (inside-realm scope chain visibility).
    if let Some(inner) = child_proxy
        .get_prototype(cs)
        .and_then(|p| v8::Local::<v8::Object>::try_from(p).ok())
    {
        if let Ok(k) = v8::Local::<v8::Name>::try_from(key) {
            inner.create_data_property(cs, k, value);
        }
    }

    // Path 2: proxy own property (cross-context parent-side visibility).
    child_proxy.set(cs, key, value);

    v8::undefined(cs).into()
}

/// Execute a JavaScript string inside a child realm's context.
///
/// Compiles and runs `code` in the child context scope. Returns the result
/// (coerced to string) or `undefined` on compile/runtime error. Used for
/// cases where `op_set_child_realm_prop` cannot express the required
/// descriptor shape (e.g. accessor properties with a getter function).
#[op2]
#[string]
pub fn op_eval_in_child_realm<'s>(
    scope: &mut v8::HandleScope<'s>,
    #[smi] realm_id: i32,
    #[string] code: String,
) -> Option<String> {
    let rid = realm_id as u32;
    let op_state_rc = JsRuntime::op_state_from(scope);

    let child_ctx_g: Option<v8::Global<v8::Context>> = {
        let op_state = op_state_rc.borrow();
        op_state.try_borrow::<IframeRealmStore>().and_then(|store| {
            store.contexts.get(&rid).map(|g| {
                let local = v8::Local::new(scope, g);
                v8::Global::new(scope, local)
            })
        })
    };
    let child_ctx_g = child_ctx_g?;

    let child_ctx = v8::Local::new(scope, &child_ctx_g);
    let cs = &mut v8::ContextScope::new(scope, child_ctx);

    let src = v8::String::new(cs, &code)?;
    // G12 (master plan §4 Phase 1): a swallowed compile/runtime error
    // here means the child realm is silently under-populated (a missing
    // shim → Kasada/DataDome bail "score is not numeric" / undefined
    // receiver). Surface it to an opt-in diagnostic channel
    // (`BOXIDE_DEBUG_CHILD_REALM`) WITHOUT changing behavior: still
    // best-effort runs the script, still returns `None`.
    let tc = &mut v8::TryCatch::new(cs);
    let ok = match v8::Script::compile(tc, src, None) {
        Some(script) => script.run(tc).is_some(),
        None => false,
    };
    if !ok && std::env::var("BOXIDE_DEBUG_CHILD_REALM").is_ok() {
        let msg = tc
            .exception()
            .and_then(|e| e.to_string(tc))
            .map(|s| s.to_rust_string_lossy(tc))
            .unwrap_or_else(|| "<no exception object>".to_string());
        let snippet: String = code.chars().take(160).collect();
        eprintln!(
            "[child-realm:{rid}] eval error: {msg} | code[..160]={snippet:?}"
        );
    }
    None
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
        op_dom_get_all_computed_styles,
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
        op_create_child_realm,
        op_set_child_realm_prop,
        op_eval_in_child_realm,
    ],
);
