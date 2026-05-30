use dom::node::{NodeData, NodeId};
use dom::Dom;

/// Information about a <script> element found in the DOM.
pub struct ScriptInfo {
    pub code: String,
    pub src: Option<String>,
    /// Value of the `nonce` attribute, if any. Required by CSP3
    /// `'nonce-...'` source matching — when the active policy uses
    /// `'strict-dynamic'`, only nonce-tagged parser-inserted scripts
    /// are authorized to load. Captured here at HTML-walk time so the
    /// fetch path (`page.rs::navigate_with_init`) can pass it to
    /// `net::csp::CheckCtx`.
    pub nonce: Option<String>,
    /// `<script type="module">` — must be executed via the ES-module path
    /// (`load_main_es_module` + `mod_evaluate`) NOT classic `execute_script`,
    /// which throws `SyntaxError: Cannot use import statement outside a module`
    /// and silently drops modern Vite/React/Vue bundles. (P2 / thin-render fix.)
    pub is_module: bool,
}

/// Find all <script> elements in the DOM and extract their content.
/// Returns both inline scripts (code) and external scripts (src URL).
pub fn find_scripts(dom: &Dom) -> Vec<ScriptInfo> {
    let mut scripts = Vec::new();
    collect_scripts(dom, NodeId::DOCUMENT, &mut scripts);
    for (i, s) in scripts.iter().enumerate() {
        if let Some(src) = &s.src {
            tracing::debug!(index = i, src = %src, "Found external script");
        } else {
            tracing::debug!(index = i, code_len = s.code.len(), "Found inline script");
        }
    }
    scripts
}

fn collect_scripts(dom: &Dom, node_id: NodeId, scripts: &mut Vec<ScriptInfo>) {
    let children = dom.children(node_id);
    for child_id in children {
        if let Some(node) = dom.get(child_id) {
            if let NodeData::Element(elem) = &node.data {
                if elem.name.local.eq_ignore_ascii_case("script") {
                    // Skip non-JS script types (JSON-LD, templates, etc.)
                    let script_type = elem
                        .attrs
                        .iter()
                        .find(|a| a.name.local == "type")
                        .map(|a| a.value.as_str());
                    match script_type {
                        Some("application/ld+json")
                        | Some("application/json")
                        | Some("text/template")
                        | Some("text/html")
                        | Some("text/x-template") => {
                            collect_scripts(dom, child_id, scripts);
                            continue;
                        }
                        _ => {}
                    }

                    let src = elem
                        .attrs
                        .iter()
                        .find(|a| a.name.local == "src")
                        .map(|a| decode_html_entities(a.value.as_str()));

                    let nonce = elem
                        .attrs
                        .iter()
                        .find(|a| a.name.local == "nonce")
                        .map(|a| a.value.to_string())
                        .filter(|n| !n.is_empty());

                    // `type="module"` (and the rarer `type="text/javascript;
                    // version=module"` is not a thing — only the exact "module"
                    // token) routes to the ES-module path. `type="importmap"`
                    // is handled separately (skipped here, not executable code).
                    let is_module = script_type == Some("module");
                    if script_type == Some("importmap") || script_type == Some("speculationrules")
                    {
                        collect_scripts(dom, child_id, scripts);
                        continue;
                    }

                    if src.is_some() {
                        // External script — store the URL for fetching
                        scripts.push(ScriptInfo {
                            code: String::new(),
                            src,
                            nonce,
                            is_module,
                        });
                    } else {
                        // Inline script
                        let code = dom.text_content(child_id);
                        if !code.trim().is_empty() {
                            scripts.push(ScriptInfo {
                                code,
                                src: None,
                                nonce,
                                is_module,
                            });
                        }
                    }
                }
            }
            collect_scripts(dom, child_id, scripts);
        }
    }
}

fn decode_html_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}
