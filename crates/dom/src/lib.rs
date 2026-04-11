//! Arena-allocated DOM tree with Shadow DOM support.
//!
//! MIT/Apache-2.0 licensed. Part of the browser_oxide project.
//!
//! # Features
//! - Arena allocation: O(1) node access, no Rc<RefCell<>>
//! - NodeId is Copy — lightweight handles
//! - Implements `css_selectors::Element` trait for selector matching
//! - Tree mutation: appendChild, insertBefore, detach, remove, reparentChildren

pub mod arena;
pub mod element;
pub mod node;

pub use arena::Dom;
pub use element::DomElement;
pub use node::*;
