use dom::Dom;
use layout::{LayoutEngine, Viewport};
use std::collections::HashMap;

/// Shared state stored in deno_core's OpState, accessible by all ops.
pub struct DomState {
    pub dom: Dom,
    pub layout_engine: LayoutEngine,
    pub base_url: Option<url::Url>,
    /// Console output capture
    pub console_output: Vec<ConsoleMessage>,
    /// localStorage / sessionStorage (in-memory)
    pub storage: HashMap<String, HashMap<String, String>>,
    /// CSS from `<style>` blocks, used by getComputedStyle
    pub stylesheets: Vec<String>,
    pub stealth_profile: Option<stealth::StealthProfile>,
}

#[derive(Debug, Clone)]
pub struct ConsoleMessage {
    pub level: ConsoleLevel,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleLevel {
    Log,
    Warn,
    Error,
    Info,
    Debug,
}

impl DomState {
    pub fn new(dom: Dom) -> Self {
        let mut storage = HashMap::new();
        storage.insert("local".to_string(), HashMap::new());
        storage.insert("session".to_string(), HashMap::new());
        Self {
            dom,
            layout_engine: LayoutEngine::new(Viewport::new(1920.0, 1080.0)),
            base_url: None,
            console_output: Vec::new(),
            storage,
            stylesheets: Vec::new(),
            stealth_profile: None,
        }
    }

    pub fn with_base_url(mut self, url: url::Url) -> Self {
        self.base_url = Some(url);
        self
    }
}
