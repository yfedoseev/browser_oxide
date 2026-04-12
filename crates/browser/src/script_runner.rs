use dom::node::{NodeData, NodeId};
use dom::Dom;

/// Information about a <script> element found in the DOM.
pub struct ScriptInfo {
    pub code: String,
    pub src: Option<String>,
    pub is_module: bool,
}

/// Find all <script> elements in the DOM and extract their content.
/// Returns both inline scripts (code) and external scripts (src URL).
pub fn find_scripts(dom: &Dom) -> Vec<ScriptInfo> {
    let mut scripts = Vec::new();
    collect_scripts(dom, NodeId::DOCUMENT, &mut scripts);
    for (i, s) in scripts.iter().enumerate() {
        if let Some(src) = &s.src {
            eprintln!("[find_scripts] found external script {}: {}", i, src);
        } else {
            eprintln!("[find_scripts] found inline script {} (len={})", i, s.code.len());
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

                    let is_module = script_type == Some("module");

                    let src = elem
                        .attrs
                        .iter()
                        .find(|a| a.name.local == "src")
                        .map(|a| a.value.clone());

                    if src.is_some() {
                        // External script — store the URL for fetching
                        scripts.push(ScriptInfo {
                            code: String::new(),
                            src,
                            is_module,
                        });
                    } else {
                        // Inline script
                        let code = dom.text_content(child_id);
                        if !code.trim().is_empty() {
                            scripts.push(ScriptInfo {
                                code,
                                src: None,
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
