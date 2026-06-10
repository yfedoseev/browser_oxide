use std::collections::HashMap;

pub type LayerId = u32;

/// Tracks @layer declaration order for cascade sorting.
pub struct LayerOrder {
    name_to_id: HashMap<String, LayerId>,
    order: Vec<LayerId>,
    next_id: LayerId,
}

impl LayerOrder {
    pub fn new() -> Self {
        Self {
            name_to_id: HashMap::new(),
            order: Vec::new(),
            next_id: 1,
        }
    }

    /// Register a layer name. Returns its ID.
    /// If already registered, returns existing ID.
    pub fn register(&mut self, name: &str) -> LayerId {
        if let Some(&id) = self.name_to_id.get(name) {
            return id;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.name_to_id.insert(name.to_string(), id);
        self.order.push(id);
        id
    }

    /// Get the layer ID for a name, or None if not registered.
    pub fn get(&self, name: &str) -> Option<LayerId> {
        self.name_to_id.get(name).copied()
    }

    /// Compare two layers for cascade ordering.
    /// Earlier declared layers have lower precedence (are overridden by later ones).
    /// Returns Ordering::Less if `a` loses to `b` in the cascade.
    pub fn compare(&self, a: LayerId, b: LayerId) -> std::cmp::Ordering {
        let a_pos = self.order.iter().position(|&id| id == a);
        let b_pos = self.order.iter().position(|&id| id == b);
        a_pos.cmp(&b_pos)
    }
}

impl Default for LayerOrder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    #[test]
    fn layer_ordering() {
        let mut layers = LayerOrder::new();
        let base = layers.register("base");
        let components = layers.register("components");
        let utilities = layers.register("utilities");

        assert_eq!(layers.compare(base, components), Ordering::Less);
        assert_eq!(layers.compare(components, utilities), Ordering::Less);
        assert_eq!(layers.compare(base, utilities), Ordering::Less);
    }

    #[test]
    fn duplicate_registration() {
        let mut layers = LayerOrder::new();
        let id1 = layers.register("base");
        let id2 = layers.register("base");
        assert_eq!(id1, id2);
    }
}
