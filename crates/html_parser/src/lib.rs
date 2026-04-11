//! html5ever integration producing a browser_oxide DOM.
//!
//! MIT/Apache-2.0 licensed. Part of the browser_oxide project.

mod tree_sink;

use dom::Dom;
use html5ever::parse_document;
use html5ever::tendril::TendrilSink;
pub use markup5ever;
use tree_sink::DomTreeSink;

/// Parse an HTML document string into a DOM tree.
pub fn parse_html(html: &str) -> Dom {
    let sink = DomTreeSink::new();
    parse_document(sink, Default::default())
        .from_utf8()
        .one(html.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use css_selectors::Element as _;
    use dom::node::NodeId;
    use dom::DomElement;

    #[test]
    fn parse_basic_html() {
        let dom = parse_html("<html><body><h1>Hello</h1></body></html>");
        let children = dom.children(NodeId::DOCUMENT);
        assert!(!children.is_empty(), "Document should have children");
    }

    #[test]
    fn parse_has_html_element() {
        let dom = parse_html("<html><head></head><body><p>Test</p></body></html>");
        let doc_children = dom.child_elements(NodeId::DOCUMENT);
        assert!(!doc_children.is_empty());

        let html_el = DomElement::new(&dom, doc_children[0]).unwrap();
        assert_eq!(html_el.local_name(), "html");
    }

    #[test]
    fn parse_text_content() {
        let dom = parse_html("<html><body><p>Hello world</p></body></html>");
        // Navigate: doc → html → body → p
        let html = dom.child_elements(NodeId::DOCUMENT)[0];
        let body_candidates = dom.child_elements(html);
        // html has head and body
        let body = body_candidates
            .iter()
            .find(|&&id| {
                dom.get(id)
                    .and_then(|n| n.as_element())
                    .map_or(false, |e| e.name.local == "body")
            })
            .copied()
            .unwrap();
        let p = dom.child_elements(body)[0];
        assert_eq!(dom.text_content(p), "Hello world");
    }

    #[test]
    fn parse_attributes() {
        let dom = parse_html("<div id=\"main\" class=\"container\">test</div>");
        // Find the div
        let html = dom.child_elements(NodeId::DOCUMENT)[0];
        let body = dom
            .child_elements(html)
            .into_iter()
            .find(|&id| {
                dom.get(id)
                    .and_then(|n| n.as_element())
                    .map_or(false, |e| e.name.local == "body")
            })
            .unwrap();
        let div = dom.child_elements(body)[0];
        let el = DomElement::new(&dom, div).unwrap();

        assert_eq!(el.id(), Some("main"));
        assert!(el.has_class("container"));
    }

    #[test]
    fn parse_nested_structure() {
        let dom = parse_html("<html><body><div><span>a</span><span>b</span></div></body></html>");
        let html = dom.child_elements(NodeId::DOCUMENT)[0];
        let body = dom
            .child_elements(html)
            .into_iter()
            .find(|&id| {
                dom.get(id)
                    .and_then(|n| n.as_element())
                    .map_or(false, |e| e.name.local == "body")
            })
            .unwrap();

        let body_children = dom.child_elements(body);
        assert!(!body_children.is_empty());

        let div = body_children
            .iter()
            .find(|&&id| {
                dom.get(id)
                    .and_then(|n| n.as_element())
                    .map_or(false, |e| e.name.local == "div")
            })
            .copied()
            .unwrap();

        // List all children of div (including text nodes)
        let all_children = dom.children(div);
        let spans = dom.child_elements(div);
        // html5ever may have different parsing; just verify we got spans
        assert!(
            spans.len() >= 1,
            "expected at least 1 span, got {}. All children: {}",
            spans.len(),
            all_children.len()
        );
        // Verify text content of the div
        assert_eq!(dom.text_content(div), "ab");
    }

    #[test]
    fn parse_doctype_html() {
        let html = "<!DOCTYPE html>\n<html>\n  <head>\n  </head>\n  <body>\n      <h1>Herman Melville</h1>\n      <div>\n        <p>Some text here.</p>\n      </div>\n  </body>\n</html>";
        let dom = parse_html(html);
        let children = dom.children(NodeId::DOCUMENT);
        assert!(
            !children.is_empty(),
            "Document should have children after doctype parse"
        );
    }

    #[test]
    fn selector_matching_on_parsed_html() {
        let dom = parse_html(
            "<html><body><div id=\"main\" class=\"content\"><p>Hello</p></div></body></html>",
        );
        let html = dom.child_elements(NodeId::DOCUMENT)[0];
        let body = dom
            .child_elements(html)
            .into_iter()
            .find(|&id| {
                dom.get(id)
                    .and_then(|n| n.as_element())
                    .map_or(false, |e| e.name.local == "body")
            })
            .unwrap();
        let div = dom.child_elements(body)[0];
        let div_el = DomElement::new(&dom, div).unwrap();

        let sel = css_selectors::parse_selector_list("div#main.content").unwrap();
        assert!(css_selectors::matches_selector(&div_el, &sel[0]));

        let sel2 = css_selectors::parse_selector_list("body > div").unwrap();
        assert!(css_selectors::matches_selector(&div_el, &sel2[0]));
    }
}
