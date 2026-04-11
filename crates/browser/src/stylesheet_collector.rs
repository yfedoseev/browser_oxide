use dom::node::{NodeData, NodeId};
use dom::Dom;

/// A stylesheet entry — either inline CSS or an external URL to fetch.
#[derive(Debug, Clone)]
pub enum StylesheetEntry {
    /// CSS text from a `<style>` block.
    Inline(String),
    /// href URL from `<link rel="stylesheet" href="...">`.
    External(String),
}

/// Find all stylesheets in the DOM: `<style>` blocks and `<link rel="stylesheet">` tags.
/// Returns entries in document order. Mirrors the `script_runner::find_scripts` pattern.
pub fn find_stylesheets(dom: &Dom) -> Vec<StylesheetEntry> {
    let mut entries = Vec::new();
    collect_stylesheets(dom, NodeId::DOCUMENT, &mut entries);
    entries
}

fn collect_stylesheets(dom: &Dom, node_id: NodeId, entries: &mut Vec<StylesheetEntry>) {
    let children = dom.children(node_id);
    for child_id in children {
        if let Some(node) = dom.get(child_id) {
            if let NodeData::Element(elem) = &node.data {
                // <style> blocks
                if elem.name.local.eq_ignore_ascii_case("style") {
                    let type_attr = elem
                        .attrs
                        .iter()
                        .find(|a| a.name.local == "type")
                        .map(|a| a.value.as_str());
                    match type_attr {
                        None | Some("text/css") | Some("") => {
                            let css = dom.text_content(child_id);
                            if !css.trim().is_empty() {
                                entries.push(StylesheetEntry::Inline(css));
                            }
                        }
                        _ => {}
                    }
                }

                // <link rel="stylesheet" href="...">
                if elem.name.local.eq_ignore_ascii_case("link") {
                    let is_stylesheet = elem.attrs.iter().any(|a| {
                        a.name.local.eq_ignore_ascii_case("rel")
                            && a.value.to_lowercase().contains("stylesheet")
                    });
                    if is_stylesheet {
                        if let Some(href) = elem
                            .attrs
                            .iter()
                            .find(|a| a.name.local.eq_ignore_ascii_case("href"))
                            .map(|a| a.value.clone())
                        {
                            if !href.trim().is_empty() {
                                entries.push(StylesheetEntry::External(href));
                            }
                        }
                    }
                }
            }
            collect_stylesheets(dom, child_id, entries);
        }
    }
}

/// Resolve inline entries into CSS strings, leaving externals as-is.
/// For use in contexts without an HTTP client (from_html, with_profile).
pub fn resolve_inline_only(entries: &[StylesheetEntry]) -> Vec<String> {
    entries
        .iter()
        .filter_map(|e| match e {
            StylesheetEntry::Inline(css) => Some(css.clone()),
            StylesheetEntry::External(_) => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_inline_style() {
        let dom = html_parser::parse_html(
            "<html><head><style>.a { color: red; }</style></head><body></body></html>",
        );
        let entries = find_stylesheets(&dom);
        assert_eq!(entries.len(), 1);
        assert!(matches!(&entries[0], StylesheetEntry::Inline(css) if css.contains("color: red")));
    }

    #[test]
    fn finds_link_stylesheet() {
        let dom = html_parser::parse_html(
            r#"<html><head><link rel="stylesheet" href="/style.css"></head><body></body></html>"#,
        );
        let entries = find_stylesheets(&dom);
        assert_eq!(entries.len(), 1);
        assert!(matches!(&entries[0], StylesheetEntry::External(href) if href == "/style.css"));
    }

    #[test]
    fn finds_both_in_order() {
        let dom = html_parser::parse_html(
            r#"<html><head>
                <link rel="stylesheet" href="/a.css">
                <style>.b { color: blue; }</style>
                <link rel="stylesheet" href="/c.css">
            </head><body></body></html>"#,
        );
        let entries = find_stylesheets(&dom);
        assert_eq!(entries.len(), 3);
        assert!(matches!(&entries[0], StylesheetEntry::External(h) if h == "/a.css"));
        assert!(matches!(&entries[1], StylesheetEntry::Inline(_)));
        assert!(matches!(&entries[2], StylesheetEntry::External(h) if h == "/c.css"));
    }

    #[test]
    fn ignores_non_stylesheet_links() {
        let dom = html_parser::parse_html(
            r#"<html><head><link rel="icon" href="/favicon.ico"></head><body></body></html>"#,
        );
        let entries = find_stylesheets(&dom);
        assert!(entries.is_empty());
    }

    #[test]
    fn resolve_inline_only_skips_external() {
        let entries = vec![
            StylesheetEntry::External("/a.css".into()),
            StylesheetEntry::Inline(".b { color: blue }".into()),
            StylesheetEntry::External("/c.css".into()),
        ];
        let resolved = resolve_inline_only(&entries);
        assert_eq!(resolved.len(), 1);
        assert!(resolved[0].contains("color: blue"));
    }
}
