//! Iframe support for browser_oxide.
//!
//! Each iframe with `srcdoc` gets its own DOM tree, V8 runtime, and event loop.
//! Communication between parent and child is via serialized postMessage.

use dom::node::{NodeData, NodeId};
use dom::Dom;
use event_loop::BrowserEventLoop;
use js_runtime::runtime::BrowserRuntimeOptions;
use js_runtime::BrowserJsRuntime;
use std::time::Duration;
use tracing;

/// Info about an iframe found in the DOM.
pub struct IframeInfo {
    pub node_id: NodeId,
    pub srcdoc: Option<String>,
    pub src: Option<String>,
}

/// A child iframe with its own V8 runtime and DOM.
pub struct ChildIframe {
    pub node_id: NodeId,
    pub event_loop: BrowserEventLoop,
}

impl ChildIframe {
    /// Create a child iframe from srcdoc HTML.
    pub async fn from_srcdoc(
        node_id: NodeId,
        html: &str,
        profile: &stealth::StealthProfile,
    ) -> Result<Self, deno_core::error::AnyError> {
        let dom = html_parser::parse_html(html);
        let scripts = crate::script_runner::find_scripts(&dom);
        let stylesheet_entries = crate::stylesheet_collector::find_stylesheets(&dom);
        let stylesheets = crate::stylesheet_collector::resolve_inline_only(&stylesheet_entries);

        let runtime = BrowserJsRuntime::with_options(
            dom,
            BrowserRuntimeOptions {
                stealth_profile: Some(profile.clone()),
                stylesheets,
                ..Default::default()
            },
        );
        let mut event_loop = BrowserEventLoop::new(runtime);

        // Execute scripts in the child's own V8 context. W2.7 — Chrome
        // reports `about:srcdoc` for srcdoc iframe stack frames.
        for (i, script) in scripts.iter().enumerate() {
            if script.src.is_some() {
                continue;
            } // Skip external scripts in srcdoc
            if script.code.trim().is_empty() {
                continue;
            }
            if let Err(e) = event_loop.execute_script_with_name(&script.code, "about:srcdoc") {
                tracing::warn!(script_index = i, error = %e, "iframe script error");
            }
        }

        // Run child event loop
        event_loop.run_until_idle(Duration::from_secs(5)).await?;

        Ok(Self {
            node_id,
            event_loop,
        })
    }

    /// Create a child iframe by fetching src URL via HTTP client.
    pub async fn from_url(
        node_id: NodeId,
        url: &str,
        client: &net::HttpClient,
        stealth_profile: Option<&stealth::StealthProfile>,
    ) -> Result<Self, deno_core::error::AnyError> {
        // CSP `frame-src` enforcement (falls back to child-src then
        // default-src). Real Chrome refuses to navigate iframes whose
        // src violates the parent's CSP, surfacing the same network-
        // error shape we return on op_fetch blocks.
        if let Ok(parsed_url) = url::Url::parse(url) {
            if let Err(violated) = js_runtime::extensions::fetch_ext::check_csp(
                net::csp::Directive::FrameSrc,
                &parsed_url,
                None,
                false,
            ) {
                eprintln!(
                    "[csp] Refused to frame '{}' because it violates the following Content Security Policy directive: \"{}\".",
                    url, violated
                );
                return Err(deno_core::error::AnyError::msg(format!(
                    "iframe blocked by CSP: {}",
                    url
                )));
            }
        }

        let resp = client
            .get(url)
            .await
            .map_err(|e| deno_core::error::AnyError::msg(format!("iframe fetch error: {}", e)))?;

        if !resp.ok() {
            return Err(deno_core::error::AnyError::msg(format!(
                "iframe fetch {} returned {}",
                url, resp.status
            )));
        }

        let html = resp.text();
        // Skip if response looks like non-HTML (binary, error page)
        if html.trim().is_empty() {
            return Self::from_srcdoc(
                node_id,
                "<html><body></body></html>",
                stealth_profile.unwrap(),
            )
            .await;
        }

        let dom = html_parser::parse_html(&html);
        let scripts = crate::script_runner::find_scripts(&dom);
        let stylesheet_entries = crate::stylesheet_collector::find_stylesheets(&dom);

        // Fetch external stylesheets
        let mut stylesheets = Vec::new();
        for entry in &stylesheet_entries {
            match entry {
                crate::stylesheet_collector::StylesheetEntry::Inline(css) => {
                    stylesheets.push(css.clone());
                }
                crate::stylesheet_collector::StylesheetEntry::External(href) => {
                    let full_url = if href.starts_with("http") {
                        href.clone()
                    } else if href.starts_with('/') {
                        if let Ok(base) = url::Url::parse(url) {
                            format!(
                                "{}://{}{}",
                                base.scheme(),
                                base.host_str().unwrap_or(""),
                                href
                            )
                        } else {
                            continue;
                        }
                    } else {
                        continue;
                    };
                    if let Ok(resp) = client.get(&full_url).await {
                        if resp.ok() {
                            let text = resp.text();
                            if !text.trim_start().starts_with("<!") {
                                stylesheets.push(text);
                            }
                        }
                    }
                }
            }
        }

        let mut options = BrowserRuntimeOptions {
            stylesheets,
            is_secure_context: crate::page::is_secure_url(url),
            ..Default::default()
        };
        if let Some(profile) = stealth_profile {
            options.stealth_profile = Some(profile.clone());
        }

        let runtime = BrowserJsRuntime::with_options(dom, options);
        let mut event_loop = BrowserEventLoop::new(runtime);

        // Set location
        let url_js = url.replace('\\', "\\\\").replace('\'', "\\'");
        event_loop
            .execute_script(&format!("location.href = '{}';", url_js))
            .ok();

        // Execute scripts, fetching external ones
        for (i, script) in scripts.iter().enumerate() {
            let code = if let Some(src) = &script.src {
                let full_url = if src.starts_with("http") {
                    src.clone()
                } else if src.starts_with('/') {
                    if let Ok(base) = url::Url::parse(url) {
                        format!(
                            "{}://{}{}",
                            base.scheme(),
                            base.host_str().unwrap_or(""),
                            src
                        )
                    } else {
                        continue;
                    }
                } else {
                    continue;
                };
                match client.get(&full_url).await {
                    Ok(resp) if resp.ok() => {
                        let text = resp.text();
                        if text.trim_start().starts_with("<!") {
                            continue;
                        }
                        text
                    }
                    _ => continue,
                }
            } else {
                script.code.clone()
            };

            if code.trim().is_empty() {
                continue;
            }
            // W2.7 — name scripts by their actual URL (external src or
            // the iframe document URL for inline). Chrome stack frames
            // are URL-tagged, not anonymous.
            let name = if let Some(src) = &script.src {
                src.clone()
            } else {
                url.to_string()
            };
            if let Err(e) = event_loop.execute_script_with_name(&code, &name) {
                tracing::warn!(script_index = i, error = %e, "iframe script error");
            }
        }

        // Run child event loop (shorter timeout for iframes)
        event_loop.run_until_idle(Duration::from_secs(10)).await?;

        Ok(Self {
            node_id,
            event_loop,
        })
    }

    /// Evaluate JS in the child's V8 context.
    pub fn evaluate(&mut self, js: &str) -> Result<String, deno_core::error::AnyError> {
        self.event_loop.execute_script(js)
    }

    /// Query the child's DOM for text content of a selector match.
    pub fn query_text(&mut self, selector: &str) -> Option<String> {
        self.evaluate(&format!(
            r#"(() => {{ const el = document.querySelector("{}"); return el ? el.textContent : ""; }})()"#,
            selector.replace('"', "\\\"")
        )).ok().filter(|s| !s.is_empty())
    }
}

/// Find all `<iframe>` elements in the DOM.
pub fn find_iframes(dom: &Dom) -> Vec<IframeInfo> {
    let mut iframes = Vec::new();
    collect_iframes(dom, NodeId::DOCUMENT, &mut iframes);
    iframes
}

fn collect_iframes(dom: &Dom, node_id: NodeId, iframes: &mut Vec<IframeInfo>) {
    let children = dom.children(node_id);
    for child_id in children {
        if let Some(node) = dom.get(child_id) {
            if let NodeData::Element(elem) = &node.data {
                if elem.name.local.eq_ignore_ascii_case("iframe") {
                    let srcdoc = elem
                        .attrs
                        .iter()
                        .find(|a| a.name.local == "srcdoc")
                        .map(|a| a.value.clone());
                    let src = elem
                        .attrs
                        .iter()
                        .find(|a| a.name.local == "src")
                        .map(|a| a.value.clone());
                    iframes.push(IframeInfo {
                        node_id: child_id,
                        srcdoc,
                        src,
                    });
                }
            }
            collect_iframes(dom, child_id, iframes);
        }
    }
}
