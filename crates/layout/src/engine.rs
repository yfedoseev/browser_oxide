use crate::query::DOMRect;
use crate::resolve::ResolveContext;
use crate::style_map::computed_to_taffy;
use crate::viewport::Viewport;
use css_cascade::ComputedStyle;
use css_values::property::{CssValue, PropertyId};
use css_values::types::display::Display;
use dom::node::{NodeData, NodeId};
use dom::Dom;
use std::collections::HashMap;
use taffy::prelude::*;

/// The layout engine. Converts a DOM + styles into positioned elements.
pub struct LayoutEngine {
    tree: TaffyTree,
    dom_to_taffy: HashMap<u32, taffy::NodeId>,
    viewport: Viewport,
    dirty: bool,
    root_taffy: Option<taffy::NodeId>,
}

impl LayoutEngine {
    pub fn new(viewport: Viewport) -> Self {
        Self {
            tree: TaffyTree::new(),
            dom_to_taffy: HashMap::new(),
            viewport,
            dirty: true,
            root_taffy: None,
        }
    }

    /// Mark layout as dirty (needs recomputation).
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Compute layout for the entire DOM tree.
    pub fn compute(&mut self, dom: &Dom) {
        // Clear previous tree
        self.tree = TaffyTree::new();
        self.dom_to_taffy.clear();

        let ctx = ResolveContext {
            font_size: 16.0,
            root_font_size: 16.0,
            viewport_w: self.viewport.width,
            viewport_h: self.viewport.height,
        };

        // Build taffy tree from DOM
        let root = self.build_node(dom, NodeId::DOCUMENT, &ctx);
        self.root_taffy = root;

        // Run layout
        if let Some(root_id) = self.root_taffy {
            let avail = taffy::Size {
                width: AvailableSpace::Definite(self.viewport.width),
                height: AvailableSpace::Definite(self.viewport.height),
            };
            self.tree.compute_layout(root_id, avail).ok();
        }

        self.dirty = false;
    }

    /// Ensure layout is computed (lazy).
    pub fn ensure_computed(&mut self, dom: &Dom) {
        if self.dirty {
            self.compute(dom);
        }
    }

    /// Get the bounding rect of a node.
    pub fn get_bounding_rect(&mut self, dom: &Dom, node_id: NodeId) -> DOMRect {
        self.ensure_computed(dom);

        // Accumulate absolute position by walking up the taffy tree
        let taffy_id = match self.dom_to_taffy.get(&node_id.to_raw()) {
            Some(id) => *id,
            None => return DOMRect::default(),
        };

        let layout = match self.tree.layout(taffy_id) {
            Ok(l) => *l,
            Err(_) => return DOMRect::default(),
        };

        // Get absolute position by summing ancestor positions
        let (abs_x, abs_y) = self.absolute_position(taffy_id);

        DOMRect::new(
            abs_x as f64,
            abs_y as f64,
            layout.size.width as f64,
            layout.size.height as f64,
        )
    }

    /// Get offsetWidth (width including padding + border).
    pub fn get_offset_width(&mut self, dom: &Dom, node_id: NodeId) -> f64 {
        self.ensure_computed(dom);
        self.taffy_size(node_id).0
    }

    /// Get offsetHeight.
    pub fn get_offset_height(&mut self, dom: &Dom, node_id: NodeId) -> f64 {
        self.ensure_computed(dom);
        self.taffy_size(node_id).1
    }

    /// Get offsetTop (position relative to offsetParent).
    pub fn get_offset_top(&mut self, dom: &Dom, node_id: NodeId) -> f64 {
        self.ensure_computed(dom);
        self.taffy_position(node_id).1
    }

    /// Get offsetLeft.
    pub fn get_offset_left(&mut self, dom: &Dom, node_id: NodeId) -> f64 {
        self.ensure_computed(dom);
        self.taffy_position(node_id).0
    }

    // --- Internal ---

    fn build_node(
        &mut self,
        dom: &Dom,
        node_id: NodeId,
        ctx: &ResolveContext,
    ) -> Option<taffy::NodeId> {
        let node = dom.get(node_id)?;

        match &node.data {
            NodeData::Document | NodeData::DocumentFragment => {
                // Container node: just build children
                let children = self.build_children(dom, node_id, ctx);
                let style = taffy::Style {
                    display: taffy::Display::Block,
                    size: taffy::Size {
                        width: Dimension::Length(ctx.viewport_w),
                        height: Dimension::Auto,
                    },
                    ..Default::default()
                };
                let taffy_id = self.tree.new_with_children(style, &children).ok()?;
                self.dom_to_taffy.insert(node_id.to_raw(), taffy_id);
                Some(taffy_id)
            }
            NodeData::Element(elem) => {
                // Get computed style (simplified: use default cascade)
                let computed = ComputedStyle::resolve(&HashMap::new(), None);

                // Check for inline style attribute
                let inline_style = self.parse_inline_style(elem);
                let computed = if !inline_style.is_empty() {
                    ComputedStyle::resolve(&inline_style, None)
                } else {
                    computed
                };

                // Skip display:none
                if let Some(CssValue::Display(Display::None)) = computed.get(&PropertyId::Display) {
                    return None;
                }

                let taffy_style = computed_to_taffy(&computed, ctx);
                let children = self.build_children(dom, node_id, ctx);
                let taffy_id = self.tree.new_with_children(taffy_style, &children).ok()?;
                self.dom_to_taffy.insert(node_id.to_raw(), taffy_id);
                Some(taffy_id)
            }
            NodeData::Text(text) => {
                // Text node: fixed size based on content
                let char_count = text.chars().count() as f32;
                let width = char_count * ctx.font_size * 0.6;
                let height = ctx.font_size * 1.2;
                let style = taffy::Style {
                    size: taffy::Size {
                        width: Dimension::Length(width),
                        height: Dimension::Length(height),
                    },
                    ..Default::default()
                };
                let taffy_id = self.tree.new_leaf(style).ok()?;
                self.dom_to_taffy.insert(node_id.to_raw(), taffy_id);
                Some(taffy_id)
            }
            _ => None,
        }
    }

    fn build_children(
        &mut self,
        dom: &Dom,
        parent: NodeId,
        ctx: &ResolveContext,
    ) -> Vec<taffy::NodeId> {
        dom.children(parent)
            .into_iter()
            .filter_map(|child_id| self.build_node(dom, child_id, ctx))
            .collect()
    }

    fn parse_inline_style(&self, elem: &dom::node::ElementData) -> HashMap<PropertyId, CssValue> {
        let mut map = HashMap::new();
        let style_attr = elem.attrs.iter().find(|a| a.name.local == "style");
        if let Some(attr) = style_attr {
            let (decls, _) = css_parser::parse_declaration_list(&attr.value);
            for decl in &decls {
                if let Ok(props) =
                    css_values::parse_property(decl.name, &decl.value, decl.important)
                {
                    for prop in props {
                        map.insert(prop.property, prop.value);
                    }
                }
            }
        }
        map
    }

    fn absolute_position(&self, taffy_id: taffy::NodeId) -> (f32, f32) {
        let mut x = 0.0f32;
        let mut y = 0.0f32;
        let mut current = taffy_id;
        loop {
            if let Ok(layout) = self.tree.layout(current) {
                x += layout.location.x;
                y += layout.location.y;
            }
            match self.tree.parent(current) {
                Some(parent) => current = parent,
                None => break,
            }
        }
        (x, y)
    }

    fn taffy_size(&self, node_id: NodeId) -> (f64, f64) {
        match self.dom_to_taffy.get(&node_id.to_raw()) {
            Some(taffy_id) => match self.tree.layout(*taffy_id) {
                Ok(layout) => (layout.size.width as f64, layout.size.height as f64),
                Err(_) => (0.0, 0.0),
            },
            None => (0.0, 0.0),
        }
    }

    fn taffy_position(&self, node_id: NodeId) -> (f64, f64) {
        match self.dom_to_taffy.get(&node_id.to_raw()) {
            Some(taffy_id) => match self.tree.layout(*taffy_id) {
                Ok(layout) => (layout.location.x as f64, layout.location.y as f64),
                Err(_) => (0.0, 0.0),
            },
            None => (0.0, 0.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dom::node::{Attribute, QualName};

    fn make_dom_with_styled_div(style: &str) -> Dom {
        let mut dom = Dom::new();
        let html = dom.create_element(QualName::new("html"), vec![]);
        dom.append_child(NodeId::DOCUMENT, html);
        let body = dom.create_element(QualName::new("body"), vec![]);
        dom.append_child(html, body);
        let div = dom.create_element(
            QualName::new("div"),
            vec![Attribute {
                name: QualName::new("style"),
                value: style.to_string(),
            }],
        );
        dom.append_child(body, div);
        dom
    }

    #[test]
    fn layout_basic_div() {
        let dom = make_dom_with_styled_div("width: 200px; height: 100px");
        let viewport = Viewport::new(1920.0, 1080.0);
        let mut engine = LayoutEngine::new(viewport);
        engine.compute(&dom);

        // Find the div (it's the child of body, which is child of html, which is child of document)
        let html = dom.child_elements(NodeId::DOCUMENT)[0];
        let body = dom.child_elements(html)[0];
        let div = dom.child_elements(body)[0];

        let rect = engine.get_bounding_rect(&dom, div);
        // Width includes border (default 3px medium border on each side)
        // Content: 200px + border: 3+3 = 206px (content-box)
        assert!(
            rect.width >= 200.0,
            "width should be >= 200, got {}",
            rect.width
        );
        assert!(
            rect.height >= 100.0,
            "height should be >= 100, got {}",
            rect.height
        );
    }

    #[test]
    fn layout_text_node_has_size() {
        let mut dom = Dom::new();
        let html = dom.create_element(QualName::new("html"), vec![]);
        dom.append_child(NodeId::DOCUMENT, html);
        let body = dom.create_element(QualName::new("body"), vec![]);
        dom.append_child(html, body);
        let text = dom.create_text("Hello world".to_string());
        dom.append_child(body, text);

        let viewport = Viewport::new(1920.0, 1080.0);
        let mut engine = LayoutEngine::new(viewport);
        engine.compute(&dom);

        let (w, h) = engine.taffy_size(text);
        assert!(w > 0.0, "text width should be > 0, got {}", w);
        assert!(h > 0.0, "text height should be > 0, got {}", h);
    }

    #[test]
    fn layout_offset_width() {
        let dom = make_dom_with_styled_div("width: 300px; height: 150px");
        let viewport = Viewport::new(1920.0, 1080.0);
        let mut engine = LayoutEngine::new(viewport);

        let html = dom.child_elements(NodeId::DOCUMENT)[0];
        let body = dom.child_elements(html)[0];
        let div = dom.child_elements(body)[0];

        let w = engine.get_offset_width(&dom, div);
        assert!(w >= 300.0, "offsetWidth should be >= 300, got {}", w);
        let h = engine.get_offset_height(&dom, div);
        assert!(h >= 150.0, "offsetHeight should be >= 150, got {}", h);
    }

    #[test]
    fn dirty_tracking() {
        let dom = make_dom_with_styled_div("width: 100px");
        let viewport = Viewport::new(1920.0, 1080.0);
        let mut engine = LayoutEngine::new(viewport);

        assert!(engine.dirty);
        engine.compute(&dom);
        assert!(!engine.dirty);
        engine.mark_dirty();
        assert!(engine.dirty);
    }

    #[test]
    fn dom_rect_from_layout() {
        let layout = taffy::Layout::new();
        let rect = DOMRect::from_taffy_layout(&layout);
        assert_eq!(rect.width, 0.0);
    }
}
