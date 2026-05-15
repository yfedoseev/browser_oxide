//! Drain `globalThis.__akamai_events` (set up by humanize.js taps in
//! `crates/browser/src/js/humanize.js`) into typed Rust events ready
//! for [`payload::build_cleartext`].
//!
//! The browser_oxide Page driver calls `page.evaluate(DRAIN_JS)` to
//! pull the buffer contents as a JSON string, then [`parse_drained`]
//! deserialises them and returns owned `MouseEvent`/`KeyEvent`/etc.
//! collections that the caller can shove into `AkamaiSession`.

use serde::Deserialize;

use crate::{KeyEvent, MouseEvent, TouchEvent};

/// JS expression that returns the current `__akamai_events` buffer
/// as a JSON string AND clears it (so the next call sees only the
/// next batch). Pass to `page.evaluate(...)` from the Rust side.
pub const DRAIN_JS: &str = r#"
(function() {
    const e = globalThis.__akamai_events;
    if (!e) return '{"mouse":[],"key":[],"touch":[],"scroll":[],"counters":{"key":0,"mouse":0,"touch":0,"scroll":0,"accel":0}}';
    const out = JSON.stringify({
        mouse: e.mouse,
        key: e.key,
        touch: e.touch,
        scroll: e.scroll,
        counters: e.counters,
    });
    // Reset (keep the object reference so humanize.js's _akEvents
    // closure stays valid).
    e.mouse = [];
    e.key = [];
    e.touch = [];
    e.scroll = [];
    return out;
})()
"#;

#[derive(Debug, Deserialize, Default)]
struct WireEvents {
    #[serde(default)]
    mouse: Vec<WireMouse>,
    #[serde(default)]
    key: Vec<WireKey>,
    #[serde(default)]
    touch: Vec<WireTouch>,
    #[serde(default)]
    counters: WireCounters,
}

#[derive(Debug, Deserialize, Default)]
struct WireCounters {
    #[serde(default)]
    key: u32,
    #[serde(default)]
    mouse: u32,
    #[serde(default)]
    touch: u32,
    #[serde(default)]
    scroll: u32,
    #[serde(default)]
    accel: u32,
}

#[derive(Debug, Deserialize)]
struct WireMouse {
    x: i32,
    y: i32,
    t: u64,
    kind: u8,
    button: u8,
}

#[derive(Debug, Deserialize)]
struct WireKey {
    code: String,
    t: u64,
    kind: u8,
}

#[derive(Debug, Deserialize)]
struct WireTouch {
    x: i32,
    y: i32,
    t: u64,
    kind: u8,
}

/// Drained, typed events ready for `AkamaiSession.{mouse_buf,key_buf,touch_buf}`.
#[derive(Debug, Default)]
pub struct Drained {
    pub mouse: Vec<MouseEvent>,
    pub key: Vec<KeyEvent>,
    pub touch: Vec<TouchEvent>,
    pub key_count: u32,
    pub mouse_count: u32,
    pub touch_count: u32,
    pub scroll_count: u32,
    pub accel_count: u32,
}

/// Parse the JSON returned by [`DRAIN_JS`] into typed events. On
/// malformed input returns `Drained::default()` rather than failing
/// — sensor_data with empty buffers is still better than no POST.
pub fn parse_drained(json: &str) -> Drained {
    let raw: WireEvents = serde_json::from_str(json).unwrap_or_default();
    Drained {
        mouse: raw
            .mouse
            .into_iter()
            .map(|m| MouseEvent {
                x: m.x,
                y: m.y,
                t: m.t,
                kind: m.kind,
                button: m.button,
            })
            .collect(),
        key: raw
            .key
            .into_iter()
            .map(|k| KeyEvent {
                code: k.code,
                t: k.t,
                kind: k.kind,
            })
            .collect(),
        touch: raw
            .touch
            .into_iter()
            .map(|t| TouchEvent {
                x: t.x,
                y: t.y,
                t: t.t,
                kind: t.kind,
            })
            .collect(),
        key_count: raw.counters.key,
        mouse_count: raw.counters.mouse,
        touch_count: raw.counters.touch,
        scroll_count: raw.counters.scroll,
        accel_count: raw.counters.accel,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_buffer() {
        let d = parse_drained(
            r#"{"mouse":[],"key":[],"touch":[],"scroll":[],"counters":{"key":0,"mouse":0,"touch":0,"scroll":0,"accel":0}}"#,
        );
        assert!(d.mouse.is_empty());
        assert!(d.key.is_empty());
        assert!(d.touch.is_empty());
        assert_eq!(d.mouse_count, 0);
    }

    #[test]
    fn parse_populated_buffer() {
        let json = r#"{
            "mouse": [
                {"x": 100, "y": 200, "t": 50, "kind": 0, "button": 0},
                {"x": 110, "y": 205, "t": 65, "kind": 0, "button": 0},
                {"x": 120, "y": 210, "t": 80, "kind": 1, "button": 0}
            ],
            "key": [
                {"code": "Tab", "t": 100, "kind": 0},
                {"code": "Tab", "t": 160, "kind": 1}
            ],
            "touch": [],
            "scroll": [],
            "counters": {"key": 2, "mouse": 18, "touch": 0, "scroll": 1, "accel": 323}
        }"#;
        let d = parse_drained(json);
        assert_eq!(d.mouse.len(), 3);
        assert_eq!(d.key.len(), 2);
        assert_eq!(d.mouse[0].x, 100);
        assert_eq!(d.mouse[2].kind, 1);
        assert_eq!(d.key[0].code, "Tab");
        assert_eq!(d.mouse_count, 18);
        assert_eq!(d.scroll_count, 1);
        assert_eq!(d.accel_count, 323);
    }

    #[test]
    fn parse_malformed_returns_empty() {
        let d = parse_drained("not json");
        assert!(d.mouse.is_empty());
        assert_eq!(d.mouse_count, 0);
    }

    #[test]
    fn drain_js_string_is_self_contained() {
        // Sanity: DRAIN_JS is an IIFE that always returns a string.
        // Should not contain unbalanced braces or rely on closures.
        let js = DRAIN_JS;
        let opens = js.matches('(').count();
        let closes = js.matches(')').count();
        assert_eq!(opens, closes);
        assert!(js.contains("globalThis.__akamai_events"));
    }
}
