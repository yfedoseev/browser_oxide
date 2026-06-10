use crate::dom::node::NodeId;
use crate::js_runtime::state::DomState;
use deno_core::op2;
use deno_core::OpState;
use serde::Serialize;

#[derive(Serialize)]
pub struct DOMRectJson {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

/// Get bounding rect using real taffy layout computation.
#[op2]
#[serde]
pub fn op_layout_get_bounding_rect(state: &mut OpState, #[smi] node_id: i32) -> DOMRectJson {
    let state = state.borrow_mut::<DomState>();
    let nid = NodeId::from_raw(node_id as u32);
    let rect = state.layout_engine.get_bounding_rect(&state.dom, nid);
    DOMRectJson {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
        top: rect.y,
        right: rect.x + rect.width,
        bottom: rect.y + rect.height,
        left: rect.x,
    }
}

#[op2(fast)]
#[smi]
pub fn op_layout_get_offset_width(state: &mut OpState, #[smi] node_id: i32) -> i32 {
    let state = state.borrow_mut::<DomState>();
    let nid = NodeId::from_raw(node_id as u32);
    state
        .layout_engine
        .get_offset_width(&state.dom, nid)
        .round() as i32
}

#[op2(fast)]
#[smi]
pub fn op_layout_get_offset_height(state: &mut OpState, #[smi] node_id: i32) -> i32 {
    let state = state.borrow_mut::<DomState>();
    let nid = NodeId::from_raw(node_id as u32);
    state
        .layout_engine
        .get_offset_height(&state.dom, nid)
        .round() as i32
}

#[op2(fast)]
#[smi]
pub fn op_layout_get_offset_top(state: &mut OpState, #[smi] node_id: i32) -> i32 {
    let state = state.borrow_mut::<DomState>();
    let nid = NodeId::from_raw(node_id as u32);
    state.layout_engine.get_offset_top(&state.dom, nid).round() as i32
}

#[op2(fast)]
#[smi]
pub fn op_layout_get_offset_left(state: &mut OpState, #[smi] node_id: i32) -> i32 {
    let state = state.borrow_mut::<DomState>();
    let nid = NodeId::from_raw(node_id as u32);
    state.layout_engine.get_offset_left(&state.dom, nid).round() as i32
}

/// Get computed style — reads from inline style attribute first, falls back to defaults.
/// This op needs DomState but lives here for historical reasons.
/// It's actually registered in dom_ext now — see op_dom_get_computed_style.
/// Kept as fallback for the JS bridge that still calls this name.
#[op2]
#[string]
pub fn op_get_computed_style(#[smi] _node_id: i32, #[string] property: &str) -> String {
    css_default(property)
}

pub fn css_default(property: &str) -> String {
    match property {
        "display" => "block".into(),
        "visibility" => "visible".into(),
        "opacity" => "1".into(),
        "position" => "static".into(),
        "overflow" => "visible".into(),
        "width" | "height" => "auto".into(),
        "color" => "rgb(0, 0, 0)".into(),
        "background-color" => "rgba(0, 0, 0, 0)".into(),
        "font-size" => "16px".into(),
        "font-family" => "\"Times New Roman\"".into(),
        "line-height" => "normal".into(),
        "margin" | "margin-top" | "margin-right" | "margin-bottom" | "margin-left" => "0px".into(),
        "padding" | "padding-top" | "padding-right" | "padding-bottom" | "padding-left" => {
            "0px".into()
        }
        "border-width" => "0px".into(),
        "box-sizing" => "content-box".into(),
        "text-align" => "start".into(),
        "float" => "none".into(),
        "clear" => "none".into(),
        "z-index" => "auto".into(),
        "transform" => "none".into(),
        "cursor" => "auto".into(),
        "pointer-events" => "auto".into(),
        _ => "".into(),
    }
}

deno_core::extension!(
    layout_extension,
    ops = [
        op_layout_get_bounding_rect,
        op_layout_get_offset_width,
        op_layout_get_offset_height,
        op_layout_get_offset_top,
        op_layout_get_offset_left,
        op_get_computed_style,
    ],
);
