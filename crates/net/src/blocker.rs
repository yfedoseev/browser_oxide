//! Resource blocker — short-circuits ad/tracker requests at the op_fetch
//! layer, before HTTP+TLS+JS-execution work happens. Drops the per-site
//! load by ~30% on news/store sites where 1/3 of requests are
//! analytics/ads (verified during Phase A holistic sweep — the
//! `op_net_fetch_sync` log was dominated by gtm.js, gpt.js, doubleclick,
//! cookielaw, etc.).
//!
//! Uses Brave's `adblock` crate which parses Adblock-Plus syntax (the
//! same format as EasyList, EasyPrivacy, uBlock filter lists). We bundle
//! a minimal high-impact baseline and accept extra rules from the
//! `BOXIDE_BLOCKER_RULES` env var for users who want full EasyList
//! integration.
//!
//! Disable entirely via `BOXIDE_NO_BLOCKER=1`. Useful for benchmarking
//! purity or for sites whose challenges depend on tracker cookies being
//! present.

use adblock::lists::{FilterFormat, FilterSet, ParseOptions};
use adblock::request::Request;
use adblock::Engine;
use std::cell::OnceCell;

/// Top tracker / ad-network domains that show up in the holistic-sweep
/// `op_net_fetch_sync` log. This baseline is only ~30 rules — full
/// EasyList has ~100K and provides much broader coverage. To enable full
/// EasyList, set `BOXIDE_BLOCKER_RULES=/path/to/easylist.txt`.
///
/// Format: Adblock-Plus syntax. `||domain^` blocks any request to that
/// domain. The `^` anchor matches end-of-domain or a path separator.
const BUILTIN_RULES: &str = "
||google-analytics.com^
||googletagmanager.com^
||google-tag-manager.com^
||doubleclick.net^
||googlesyndication.com^
||googleadservices.com^
||adservice.google.com^
||facebook.com/tr^
||connect.facebook.net^
||scorecardresearch.com^
||quantserve.com^
||quantcount.com^
||adsystem.amazon.com^
||amazon-adsystem.com^
||criteo.com^
||criteo.net^
||outbrain.com^
||taboola.com^
||adnxs.com^
||adsrvr.org^
||rubiconproject.com^
||pubmatic.com^
||openx.net^
||casalemedia.com^
||cookielaw.org^
||onetrust.com^
||trustarc.com^
||doubleverify.com^
||moatads.com^
||segment.com/v1^
||segment.io^
||mixpanel.com^
||hotjar.com^
||fullstory.com^
||cdn.permutive.com^
||sentry.io^
||bugsnag.com^
||newrelic.com^
||clarity.ms^
||intercom.io^
||intercomcdn.com^
||snapchat.com/p^
||tiktok.com/api/v1/web/report^
||analytics.tiktok.com^
||hs-analytics.net^
||hs-scripts.com^
||stats.wp.com^
||wordpress.com/_static^
";

// `adblock::Engine` is not `Sync` (uses interior mutability for the
// internal regex cache). Use thread-local storage so each worker thread
// in the parallel pager has its own engine. Initialization is one-time
// per thread (~ms cost for a 30-rule list).
thread_local! {
    static ENGINE: OnceCell<Option<Engine>> = const { OnceCell::new() };
}

fn build_engine() -> Option<Engine> {
    // Opt-in by default. Holistic-sweep testing (Phase E) showed the
    // blocker does NOT speed things up materially in the parallel-pager
    // configuration (Phase D already eliminated the dominant network
    // wait via concurrency), AND some sites' challenges depend on
    // tracker cookies being loaded (cookielaw/OneTrust banners,
    // segment.io initialization) — blocking them costs ~2 PASSes.
    //
    // Default off; users who want to drop tracker requests for batch
    // scraping where stealth doesn't matter can set BOXIDE_BLOCKER=1.
    if std::env::var("BOXIDE_BLOCKER").is_err() {
        return None;
    }

    let mut rules = String::from(BUILTIN_RULES);
    if let Ok(path) = std::env::var("BOXIDE_BLOCKER_RULES") {
        match std::fs::read_to_string(&path) {
            Ok(extra) => {
                rules.push('\n');
                rules.push_str(&extra);
            }
            Err(e) => {
                eprintln!(
                    "[blocker] WARN: BOXIDE_BLOCKER_RULES={} failed to read: {}",
                    path, e
                );
            }
        }
    }

    let mut filter_set = FilterSet::new(false);
    let parse_opts = ParseOptions {
        format: FilterFormat::Standard,
        ..Default::default()
    };
    filter_set.add_filter_list(&rules, parse_opts);
    Some(Engine::from_filter_set(filter_set, true))
}

/// Run a closure with access to this thread's engine, or `None` if the
/// engine is disabled. Used internally by `should_block`.
fn with_engine<R>(f: impl FnOnce(Option<&Engine>) -> R) -> R {
    ENGINE.with(|cell| {
        let opt = cell.get_or_init(build_engine);
        f(opt.as_ref())
    })
}

/// Hint to the filter engine about what kind of request this is.
/// Standard adblock filters distinguish script/image/xhr/etc.
pub fn classify_request_type(url: &str, hint: Option<&str>) -> &'static str {
    if let Some(h) = hint {
        return match h {
            "image" | "img" => "image",
            "script" | "js" => "script",
            "stylesheet" | "css" => "stylesheet",
            "xhr" | "fetch" => "xmlhttprequest",
            "document" => "document",
            "subdocument" => "subdocument",
            "media" => "media",
            "font" => "font",
            "websocket" => "websocket",
            _ => "other",
        };
    }
    // URL-extension fallback heuristic.
    let lower = url.to_lowercase();
    if lower.ends_with(".js") || lower.contains(".js?") {
        "script"
    } else if lower.ends_with(".css") || lower.contains(".css?") {
        "stylesheet"
    } else if lower.ends_with(".png")
        || lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".gif")
        || lower.ends_with(".webp")
        || lower.ends_with(".svg")
    {
        "image"
    } else {
        "xmlhttprequest"
    }
}

/// Returns true if the request URL matches a block rule.
/// `source_url` is the page that made the request (for first-party vs
/// third-party scoring); pass empty string if unknown.
pub fn should_block(url: &str, source_url: &str, request_type: &str) -> bool {
    with_engine(|opt_eng| {
        let Some(eng) = opt_eng else { return false };
        // Falls back to `false` on any parse error — fail-open is safer
        // than fail-closed for an opt-in blocker.
        let req = match Request::new(url, source_url, request_type) {
            Ok(r) => r,
            Err(_) => return false,
        };
        eng.check_network_request(&req).matched
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_off_means_does_not_block() {
        // BOXIDE_BLOCKER not set in cargo test env → engine() returns
        // None → all should_block calls return false. This guarantees
        // the default test/run path doesn't get any blocker behaviour.
        assert!(!should_block(
            "https://www.google-analytics.com/analytics.js",
            "https://www.example.com/",
            "script"
        ));
    }

    // Tests that exercise the actual engine require BOXIDE_BLOCKER=1 to
    // be set in the env. Run them via:
    //   BOXIDE_BLOCKER=1 cargo test -p net --lib blocker -- --ignored
    #[test]
    #[ignore]
    fn blocks_known_tracker_when_enabled() {
        // Manual verification: with BOXIDE_BLOCKER=1, google-analytics
        // is blocked when called from a third-party page.
        std::env::set_var("BOXIDE_BLOCKER", "1");
        assert!(should_block(
            "https://www.google-analytics.com/analytics.js",
            "https://www.example.com/",
            "script"
        ));
    }

    #[test]
    #[ignore]
    fn allows_legitimate_request_when_enabled() {
        std::env::set_var("BOXIDE_BLOCKER", "1");
        // First-party CDN-fetched JS should NOT match (no rule covers it).
        assert!(!should_block(
            "https://www.example.com/static/main.js",
            "https://www.example.com/",
            "script"
        ));
    }
}
