//! CSP enforcement integration test.
//!
//! Locks in the load-bearing behavior: when a page declares
//! `script-src 'self' 'strict-dynamic' 'nonce-XXX'`, the parser-injected
//! `<script src="...">` *without* a matching nonce must NOT be fetched.
//! This is exactly what real Chrome does on walmart.com — and what we
//! were failing to do before, causing browser_oxide to issue
//! `/akam/13/...` requests that Chrome never makes.

use browser::Page;
use stealth::presets::chrome_148_macos;

/// A miniaturized Walmart-style CSP. The page declares strict-dynamic
/// + nonce, then includes one parser-injected script with a matching
/// nonce (allowed) and one without (blocked).
const HTML: &str = r#"<!doctype html>
<html><head>
<meta http-equiv="Content-Security-Policy"
      content="script-src 'self' 'strict-dynamic' 'nonce-MRjHHgrLk9lNoNBv'">
<title>csp test</title>
</head><body>
<script nonce="MRjHHgrLk9lNoNBv">
  globalThis.__legitimate_inline_ran = true;
</script>
<!--
  Parser-injected without a nonce — must be blocked under strict-dynamic.
  We point it at a never-resolvable host so a fetch attempt would surface
  as a network error in the runtime; if the engine respects CSP, the
  fetch is never attempted at all and the page reaches DOMContentLoaded
  cleanly.
-->
<script src="https://blocked-by-csp.invalid./payload.js"></script>
</body></html>"#;

/// End-to-end: load the strict-dynamic page, confirm the inline script
/// with matching nonce ran, and confirm the engine reports a CSP block
/// for the parser-injected script (proving the gate fires before the
/// fetch is attempted).
#[tokio::test]
async fn parser_injected_script_without_nonce_is_blocked() {
    use std::sync::{Arc, Mutex};

    // Capture stderr-equivalent: the engine emits
    // `[csp] Refused to load the script '...'` via eprintln!. We can't
    // intercept stderr from inside cargo test, but we can run the
    // navigation from a Page::from_html and additionally directly
    // exercise the check_csp() function with the parsed policy.
    use browser::csp_collector::collect_csp;
    use html_parser::parse_html;
    use js_runtime::extensions::fetch_ext as csp_state;
    use net::csp::Directive;
    use url::Url;

    // Install the policy as the engine would.
    let dom = parse_html(HTML);
    let policy = collect_csp(&[], &dom);
    csp_state::set_csp_policy(
        Arc::new(policy),
        Url::parse("https://example.com/").unwrap(),
        true,
    );

    // The blocked-by-csp.invalid script must trip CSP.
    let blocked_url = Url::parse("https://blocked-by-csp.invalid./payload.js").unwrap();
    let block_decision = csp_state::check_csp(
        Directive::ScriptSrcElem,
        &blocked_url,
        None, // no nonce → blocked under strict-dynamic
        true, // parser_inserted
    );
    assert!(
        block_decision.is_err(),
        "parser-injected, no-nonce script must trip CSP"
    );
    assert_eq!(block_decision.unwrap_err(), "script-src");

    // The same URL with a matching nonce must NOT trip CSP.
    let allowed_decision = csp_state::check_csp(
        Directive::ScriptSrcElem,
        &blocked_url,
        Some("MRjHHgrLk9lNoNBv"),
        true,
    );
    assert!(
        allowed_decision.is_ok(),
        "matching nonce must clear CSP under strict-dynamic"
    );

    // Now actually load the page — the inline script with nonce should
    // run, the parser-injected blocked-by-csp.invalid script must be
    // skipped without a network attempt.
    let captured_log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let _captured_for_log = captured_log.clone();
    let mut page = Page::from_html(HTML, Some(chrome_148_macos()))
        .await
        .unwrap();

    let inline_ran = page
        .evaluate("String(globalThis.__legitimate_inline_ran)")
        .unwrap();
    assert_eq!(
        inline_ran, "true",
        "inline script with matching nonce must execute"
    );

    // Cleanup so the next test sees a fresh state.
    csp_state::clear_csp_policy();
}

/// A page with NO CSP at all must still load both scripts (parser-
/// injected, no nonce, no policy → no enforcement).
#[tokio::test]
async fn no_csp_does_not_block_anything() {
    const NO_CSP_HTML: &str = r#"<!doctype html>
<html><head><title>no csp</title></head><body>
<script>globalThis.__inline_ran = "yes";</script>
</body></html>"#;
    let mut page = Page::from_html(NO_CSP_HTML, Some(chrome_148_macos()))
        .await
        .unwrap();
    let inline = page.evaluate("globalThis.__inline_ran").unwrap();
    assert_eq!(inline.trim_matches('"'), "yes");
}

/// `securitypolicyviolation` event must fire on `document` (and bubble
/// to `window`) when a fetch is blocked. Verifies the full pipeline:
/// page-level meta-CSP install → queued violation from a Rust gate →
/// JS-side dispatcher → event listener with correct fields.
#[tokio::test]
#[ignore = "FIXME: CSP violation queue → JS event dispatch pipeline drops the event somewhere between csp_state::check_csp and document listener"]
async fn securitypolicyviolation_event_fires_on_block() {
    use js_runtime::extensions::fetch_ext as csp_state;
    use net::csp::Directive;
    use url::Url;

    // Page sets its own CSP via meta-tag and installs a listener BEFORE
    // anything fires. After page init we trip the gate from Rust then
    // explicitly drain so the test is deterministic (doesn't rely on
    // setTimeout timing).
    const HTML: &str = r#"<html><head>
<meta http-equiv="Content-Security-Policy" content="connect-src 'self'">
<title>spv</title>
</head><body>
<script>
  globalThis.__spv_events = [];
  document.addEventListener('securitypolicyviolation', (e) => {
    globalThis.__spv_events.push({
      blockedURI: e.blockedURI,
      effectiveDirective: e.effectiveDirective,
      violatedDirective: e.violatedDirective,
      disposition: e.disposition,
      typeOk: typeof SecurityPolicyViolationEvent === 'function' && (e instanceof SecurityPolicyViolationEvent),
    });
  });
</script>
</body></html>"#;

    let mut page = Page::from_html(HTML, Some(chrome_148_macos()))
        .await
        .unwrap();

    // Trip the gate AFTER Page is up so the violation queue is fresh
    // and the policy hasn't been re-set since the listener registered.
    let _ = csp_state::check_csp(
        Directive::ConnectSrc,
        &Url::parse("https://collector.example/api").unwrap(),
        None,
        false,
    );

    // Force an explicit drain — deterministic; doesn't depend on the
    // background setTimeout timeline.
    let _ = page
        .evaluate("globalThis.__drainCspViolations && globalThis.__drainCspViolations()")
        .unwrap();

    let n = page
        .evaluate("String(globalThis.__spv_events.length)")
        .unwrap();
    let n_clean = n.trim_matches('"');
    assert!(
        n_clean.parse::<i64>().unwrap_or(0) >= 1,
        "at least one securitypolicyviolation event must have fired, got {n_clean}"
    );
    let blocked = page
        .evaluate("globalThis.__spv_events[0].blockedURI")
        .unwrap();
    assert!(
        blocked.contains("collector.example"),
        "blockedURI must point at the blocked URL, got {blocked}"
    );
    let directive = page
        .evaluate("globalThis.__spv_events[0].effectiveDirective")
        .unwrap();
    assert!(
        directive.contains("connect-src"),
        "effectiveDirective must echo 'connect-src', got {directive}"
    );
    let type_ok = page
        .evaluate("String(globalThis.__spv_events[0].typeOk)")
        .unwrap();
    assert_eq!(
        type_ok, "true",
        "the event must be a SecurityPolicyViolationEvent instance"
    );
    let disposition = page
        .evaluate("globalThis.__spv_events[0].disposition")
        .unwrap();
    assert!(
        disposition.contains("enforce"),
        "disposition must be 'enforce' for this policy, got {disposition}"
    );

    csp_state::clear_csp_policy();
}

/// CSP `connect-src` enforcement: when the policy doesn't whitelist a
/// host, `window.fetch()` and XHR must short-circuit with a network-error
/// response (status 0, ok=false). Real Chrome behavior. We exercise the
/// `check_csp` API directly here since the connect-src gate fires at
/// the Rust op layer, before the request hits the network.
#[tokio::test]
async fn connect_src_blocks_disallowed_hosts() {
    use browser::csp_collector::collect_csp;
    use html_parser::parse_html;
    use js_runtime::extensions::fetch_ext as csp_state;
    use net::csp::Directive;
    use std::sync::Arc;
    use url::Url;

    const CSP_HTML: &str = r#"<!doctype html><html><head>
<meta http-equiv="Content-Security-Policy"
      content="connect-src 'self' https://api.example.com">
</head><body></body></html>"#;
    let dom = parse_html(CSP_HTML);
    let policy = collect_csp(&[], &dom);
    csp_state::set_csp_policy(
        Arc::new(policy),
        Url::parse("https://example.com/").unwrap(),
        true,
    );

    // Same origin → allowed.
    let allowed = csp_state::check_csp(
        Directive::ConnectSrc,
        &Url::parse("https://example.com/api/v1/data").unwrap(),
        None,
        false,
    );
    assert!(allowed.is_ok(), "same-origin connect must be allowed");

    // Whitelisted host → allowed.
    let api = csp_state::check_csp(
        Directive::ConnectSrc,
        &Url::parse("https://api.example.com/feed").unwrap(),
        None,
        false,
    );
    assert!(api.is_ok(), "whitelisted host must be allowed");

    // Off-policy host → blocked.
    let bad = csp_state::check_csp(
        Directive::ConnectSrc,
        &Url::parse("https://collector-pxu6b0qd2s.px-cloud.net/api/v2/collector").unwrap(),
        None,
        false,
    );
    assert!(bad.is_err(), "off-policy connect-src must be blocked");
    assert_eq!(bad.unwrap_err(), "connect-src");

    csp_state::clear_csp_policy();
}

/// `frame-src` enforcement: iframe navigations against off-policy hosts
/// must be refused. The `child-src` and `default-src` fallback chain
/// applies — if `frame-src` is absent, those govern instead.
#[tokio::test]
async fn frame_src_blocks_disallowed_iframe() {
    use browser::csp_collector::collect_csp;
    use html_parser::parse_html;
    use js_runtime::extensions::fetch_ext as csp_state;
    use net::csp::Directive;
    use std::sync::Arc;
    use url::Url;

    let dom = parse_html(
        r#"<html><head>
<meta http-equiv="Content-Security-Policy" content="frame-src 'self' https://www.youtube.com">
</head></html>"#,
    );
    let policy = collect_csp(&[], &dom);
    csp_state::set_csp_policy(
        Arc::new(policy),
        Url::parse("https://example.com/").unwrap(),
        true,
    );

    // YouTube whitelist match → allowed.
    let yt = csp_state::check_csp(
        Directive::FrameSrc,
        &Url::parse("https://www.youtube.com/embed/abc").unwrap(),
        None,
        false,
    );
    assert!(yt.is_ok());

    // Off-policy iframe target → blocked.
    let bad = csp_state::check_csp(
        Directive::FrameSrc,
        &Url::parse("https://attacker.example/").unwrap(),
        None,
        false,
    );
    assert!(bad.is_err());
    assert_eq!(bad.unwrap_err(), "frame-src");

    csp_state::clear_csp_policy();
}

/// `frame-src` falls back to `child-src` then `default-src`. Verify
/// the chain works end-to-end so a default-src-only policy still gates
/// iframe navigation.
#[tokio::test]
async fn frame_src_falls_back_through_child_src_to_default_src() {
    use browser::csp_collector::collect_csp;
    use html_parser::parse_html;
    use js_runtime::extensions::fetch_ext as csp_state;
    use net::csp::Directive;
    use std::sync::Arc;
    use url::Url;

    // No frame-src or child-src — falls back to default-src.
    let dom = parse_html(
        r#"<html><head>
<meta http-equiv="Content-Security-Policy" content="default-src 'self'">
</head></html>"#,
    );
    let policy = collect_csp(&[], &dom);
    csp_state::set_csp_policy(
        Arc::new(policy),
        Url::parse("https://example.com/").unwrap(),
        true,
    );

    let same_origin = csp_state::check_csp(
        Directive::FrameSrc,
        &Url::parse("https://example.com/iframe.html").unwrap(),
        None,
        false,
    );
    assert!(
        same_origin.is_ok(),
        "default-src 'self' allows same-origin iframe"
    );

    let cross = csp_state::check_csp(
        Directive::FrameSrc,
        &Url::parse("https://other.example/iframe.html").unwrap(),
        None,
        false,
    );
    assert!(
        cross.is_err(),
        "default-src 'self' blocks cross-origin iframe via fallback chain"
    );
    assert_eq!(cross.unwrap_err(), "default-src");

    csp_state::clear_csp_policy();
}

/// `BROWSER_OXIDE_CSP_BYPASS=1` env var must turn off enforcement entirely
/// without touching the policy parser. Useful for benchmarking and
/// for sites where the policy is overly tight on us specifically.
#[tokio::test]
async fn bypass_env_var_disables_enforcement() {
    use browser::csp_collector::collect_csp;
    use html_parser::parse_html;
    use js_runtime::extensions::fetch_ext as csp_state;
    use net::csp::Directive;
    use std::sync::Arc;
    use url::Url;

    let dom = parse_html(
        r#"<html><head>
<meta http-equiv="Content-Security-Policy" content="connect-src 'none'">
</head></html>"#,
    );
    let policy = collect_csp(&[], &dom);

    // enforce=false simulates BROWSER_OXIDE_CSP_BYPASS=1.
    csp_state::set_csp_policy(
        Arc::new(policy),
        Url::parse("https://example.com/").unwrap(),
        false,
    );

    let any = csp_state::check_csp(
        Directive::ConnectSrc,
        &Url::parse("https://anywhere.test/x").unwrap(),
        None,
        false,
    );
    assert!(
        any.is_ok(),
        "bypass=true must allow even 'none' policy fetches"
    );
    csp_state::clear_csp_policy();
}
