use dom::Dom;
use layout::{LayoutEngine, Viewport};
use std::collections::HashMap;
use std::sync::Arc;

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
    /// Parsed and simplified CSS rules for fast lookup
    pub cached_rules: Vec<CachedRule>,
    pub stealth_profile: Option<stealth::StealthProfile>,
    /// Active Content Security Policy. Built from the response
    /// `Content-Security-Policy` header(s) plus any
    /// `<meta http-equiv="Content-Security-Policy">` tags found in the
    /// parsed HTML. None means no policy applies (e.g. about:blank,
    /// from_html with no header). The policy applies to ALL fetches —
    /// `<script src>`, `op_fetch`, `op_net_fetch_sync`, iframes — until
    /// the next top-level navigation.
    pub csp_policy: Option<Arc<net::csp::PolicySet>>,
    /// Origin used to resolve `'self'` in CSP source matching. Equals
    /// the document's origin (scheme + host + port of the navigated
    /// URL). None for opaque/about:blank documents — those bypass CSP.
    pub csp_origin: Option<url::Url>,
    /// Resource timings for performance.getEntriesByType('resource')
    pub resource_timings: Vec<net::TimingStats>,
}

#[derive(Debug, Clone)]
pub struct CachedRule {
    pub selector_str: String,
    pub selectors: css_selectors::SelectorList,
    pub declarations: HashMap<String, String>,
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
            cached_rules: Vec::new(),
            stealth_profile: None,
            csp_policy: None,
            csp_origin: None,
            resource_timings: Vec::new(),
        }
    }

    pub fn update_cached_rules(&mut self) {
        use crate::utils::tokens_to_string;
        self.cached_rules.clear();
        for css_text in &self.stylesheets {
            let (stylesheet, _errors) = css_parser::parse_stylesheet(css_text);
            for rule in &stylesheet.rules {
                if let css_parser::ast::Rule::Qualified(qr) = rule {
                    let selector_str = tokens_to_string(&qr.prelude);
                    if selector_str.is_empty() {
                        continue;
                    }
                    let mut declarations = HashMap::new();
                    for d in &qr.declarations {
                        declarations.insert(
                            d.name.to_string(),
                            tokens_to_string(&d.value).trim().to_string(),
                        );
                    }
                    let selectors =
                        css_selectors::parse_selector_list(&selector_str).unwrap_or_default();
                    self.cached_rules.push(CachedRule {
                        selector_str,
                        selectors,
                        declarations,
                    });
                }
            }
        }
    }

    pub fn with_base_url(mut self, url: url::Url) -> Self {
        self.base_url = Some(url);
        self
    }
}
