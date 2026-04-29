//! Walk a parsed HTML DOM looking for
//! `<meta http-equiv="Content-Security-Policy" content="...">` tags
//! and merge the policies they declare with the response-header CSP.
//!
//! Per CSP3 §3.4.1, meta-tag CSPs:
//! - apply only the "enforce" disposition (cannot be report-only)
//! - cannot relax a stricter response-header CSP — the matcher's
//!   "all policies must allow" semantic handles this naturally
//! - must appear inside `<head>` to take effect
//!
//! Walmart specifically delivers its `script-src 'strict-dynamic'`
//! directive via meta-tag, not via response header — so without this
//! collector we'd never see the strict-dynamic that blocks Akamai.

use dom::node::{NodeData, NodeId};
use dom::Dom;
use net::csp::PolicySet;

/// Build a complete `PolicySet` from response-header CSP value(s) plus
/// any meta-CSP tags present in `<head>`. Either input may be empty.
///
/// The CSP3 spec allows a server to send the header multiple times or
/// comma-separated; we accept both via `header_values` (one entry per
/// header instance).
pub fn collect_csp(header_values: &[&str], dom: &Dom) -> PolicySet {
    let mut set = PolicySet::default();
    for header in header_values {
        set.push_header(header, false);
    }
    walk_head_for_meta_csp(dom, &mut set);
    set
}

/// Same as `collect_csp` but also accepts the report-only header(s).
pub fn collect_csp_with_report_only(
    enforce_headers: &[&str],
    report_only_headers: &[&str],
    dom: &Dom,
) -> PolicySet {
    let mut set = PolicySet::default();
    for header in enforce_headers {
        set.push_header(header, false);
    }
    for header in report_only_headers {
        set.push_header(header, true);
    }
    walk_head_for_meta_csp(dom, &mut set);
    set
}

fn walk_head_for_meta_csp(dom: &Dom, set: &mut PolicySet) {
    // Find <head> by walking children of <html>. If absent, fall back
    // to walking the whole document — some quirks-mode pages leave
    // meta tags as direct children of the synthetic root.
    let head_id = find_head(dom).unwrap_or(NodeId::DOCUMENT);
    visit_for_meta_csp(dom, head_id, set);
}

fn find_head(dom: &Dom) -> Option<NodeId> {
    // <html> is the first element under DOCUMENT in a well-formed page.
    for child in dom.children(NodeId::DOCUMENT) {
        if let Some(node) = dom.get(child) {
            if let NodeData::Element(e) = &node.data {
                if e.name.local.eq_ignore_ascii_case("html") {
                    for h in dom.children(child) {
                        if let Some(hn) = dom.get(h) {
                            if let NodeData::Element(he) = &hn.data {
                                if he.name.local.eq_ignore_ascii_case("head") {
                                    return Some(h);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

fn visit_for_meta_csp(dom: &Dom, node_id: NodeId, set: &mut PolicySet) {
    for child in dom.children(node_id) {
        if let Some(node) = dom.get(child) {
            if let NodeData::Element(e) = &node.data {
                if e.name.local.eq_ignore_ascii_case("meta") {
                    let http_equiv: Option<&str> = e
                        .attrs
                        .iter()
                        .find(|a| a.name.local.eq_ignore_ascii_case("http-equiv"))
                        .map(|a| a.value.as_str());
                    if let Some(eq) = http_equiv {
                        if eq.eq_ignore_ascii_case("content-security-policy") {
                            if let Some(content) = e
                                .attrs
                                .iter()
                                .find(|a| a.name.local.eq_ignore_ascii_case("content"))
                            {
                                set.push_meta(content.value.as_str());
                            }
                        }
                    }
                }
                // Don't recurse into <body>; meta tags inside body are
                // ignored per spec, and recursing wastes work.
                if e.name.local.eq_ignore_ascii_case("body") {
                    continue;
                }
            }
            visit_for_meta_csp(dom, child, set);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use html_parser::parse_html;
    use net::csp::Directive;

    #[test]
    fn extracts_meta_csp_from_head() {
        let dom = parse_html(
            "<html><head>\
             <meta http-equiv=\"Content-Security-Policy\" content=\"script-src 'self' 'strict-dynamic'\">\
             </head><body></body></html>",
        );
        let set = collect_csp(&[], &dom);
        assert_eq!(set.policies.len(), 1);
        assert!(set.policies[0].directives.contains_key(&Directive::ScriptSrc));
    }

    #[test]
    fn header_and_meta_combine_into_policy_set() {
        let dom = parse_html(
            "<html><head>\
             <meta http-equiv=\"Content-Security-Policy\" content=\"script-src 'self'\">\
             </head></html>",
        );
        let set = collect_csp(&["connect-src 'self'"], &dom);
        // Two policies: one from header, one from meta. Both apply.
        assert_eq!(set.policies.len(), 2);
    }

    #[test]
    fn ignores_meta_csp_inside_body() {
        let dom = parse_html(
            "<html><head></head><body>\
             <meta http-equiv=\"Content-Security-Policy\" content=\"script-src 'none'\">\
             </body></html>",
        );
        let set = collect_csp(&[], &dom);
        // Per spec, meta tags inside body must not be honored.
        assert_eq!(set.policies.len(), 0);
    }

    #[test]
    fn case_insensitive_http_equiv_match() {
        let dom = parse_html(
            "<html><head>\
             <META HTTP-EQUIV=\"Content-Security-Policy\" CONTENT=\"script-src 'self'\">\
             </head></html>",
        );
        let set = collect_csp(&[], &dom);
        assert_eq!(set.policies.len(), 1);
    }

    #[test]
    fn no_csp_at_all_returns_empty_set() {
        let dom = parse_html("<html><head></head><body></body></html>");
        let set = collect_csp(&[], &dom);
        assert!(set.is_empty());
    }
}
