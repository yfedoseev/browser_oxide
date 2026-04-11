/// Opaque, lightweight handle to a node in the DOM arena.
/// Copy + Eq + Hash so it can be used as a key/value everywhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub(crate) usize);

impl NodeId {
    /// The document node is always at index 0.
    pub const DOCUMENT: NodeId = NodeId(0);

    /// Create a NodeId from a raw u32 (for JS interop).
    pub fn from_raw(v: u32) -> Self {
        Self(v as usize)
    }

    /// Get the raw u32 value (for JS interop).
    pub fn to_raw(self) -> u32 {
        self.0 as u32
    }
}

/// A qualified name (namespace + local name).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualName {
    pub ns: Option<String>,
    pub local: String,
}

impl QualName {
    pub fn new(local: impl Into<String>) -> Self {
        Self {
            ns: None,
            local: local.into(),
        }
    }

    pub fn with_ns(ns: impl Into<String>, local: impl Into<String>) -> Self {
        Self {
            ns: Some(ns.into()),
            local: local.into(),
        }
    }
}

/// An HTML/XML attribute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    pub name: QualName,
    pub value: String,
}

/// Node data — the payload for each node in the arena.
#[derive(Debug, Clone)]
pub enum NodeData {
    Document,
    DocumentType {
        name: String,
        public_id: String,
        system_id: String,
    },
    Element(ElementData),
    Text(String),
    Comment(String),
    ProcessingInstruction {
        target: String,
        data: String,
    },
    DocumentFragment,
    ShadowRoot {
        mode: ShadowRootMode,
        host: NodeId,
    },
}

#[derive(Debug, Clone)]
pub struct ElementData {
    pub name: QualName,
    pub attrs: Vec<Attribute>,
    pub shadow_root: Option<NodeId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowRootMode {
    Open,
    Closed,
}

/// A node in the arena.
#[derive(Debug, Clone)]
pub struct Node {
    pub id: NodeId,
    pub data: NodeData,
    pub parent: Option<NodeId>,
    pub first_child: Option<NodeId>,
    pub last_child: Option<NodeId>,
    pub prev_sibling: Option<NodeId>,
    pub next_sibling: Option<NodeId>,
}

impl Node {
    pub fn new(id: NodeId, data: NodeData) -> Self {
        Self {
            id,
            data,
            parent: None,
            first_child: None,
            last_child: None,
            prev_sibling: None,
            next_sibling: None,
        }
    }

    /// Returns true if this is an element node.
    pub fn is_element(&self) -> bool {
        matches!(self.data, NodeData::Element(_))
    }

    /// Returns the element data, if this is an element.
    pub fn as_element(&self) -> Option<&ElementData> {
        match &self.data {
            NodeData::Element(data) => Some(data),
            _ => None,
        }
    }

    /// Returns mutable element data.
    pub fn as_element_mut(&mut self) -> Option<&mut ElementData> {
        match &mut self.data {
            NodeData::Element(data) => Some(data),
            _ => None,
        }
    }

    /// Returns the text content if this is a text node.
    pub fn as_text(&self) -> Option<&str> {
        match &self.data {
            NodeData::Text(t) => Some(t),
            _ => None,
        }
    }
}
