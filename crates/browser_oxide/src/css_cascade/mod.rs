//! CSS cascade, specificity, inheritance, @layer, @media evaluation.
//!
//! MIT/Apache-2.0 licensed. Part of the browser_oxide project.

pub mod cascade;
pub mod computed;
pub mod inheritance;
pub mod initial;
pub mod layers;
pub mod media;

pub use cascade::{cascade_sort, CascadeEntry, Origin};
pub use computed::ComputedStyle;
pub use inheritance::is_inherited;
pub use initial::initial_value;
pub use layers::{LayerId, LayerOrder};
pub use media::{evaluate_media_query, MediaFeatures};
