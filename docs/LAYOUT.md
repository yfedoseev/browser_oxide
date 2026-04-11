# layout — Box Model for getBoundingClientRect

Provides layout computation so JS APIs like `getBoundingClientRect()`, `offsetWidth`, `offsetHeight` return meaningful values.

## Why We Need Layout (Without Rendering)

Many websites and anti-bot systems call layout APIs:

```javascript
// SPA frameworks (React, Vue) check element dimensions
const rect = element.getBoundingClientRect();
if (rect.width === 0) { /* element not visible, skip */ }

// Anti-bot checks (visibility verification)
const nav = document.querySelector('nav');
if (nav.offsetHeight === 0) { /* suspicious — bot doesn't have layout */ }

// Lazy loading
if (entry.isIntersecting) { loadImage(); }
```

If these return `0` or `undefined`, sites break or flag us as a bot.

## Core: taffy

| Property | Value |
|---|---|
| Crate | `taffy` |
| License | MIT |
| Algorithms | CSS Block, Flexbox, Grid |
| Used by | Dioxus, Zed editor, Bevy UI |

taffy takes a tree of nodes with `Style` structs and computes `Layout` (position + size) for each node.

## Architecture

```
layout/
├── src/
│   ├── lib.rs              # LayoutEngine — compute + query
│   ├── engine.rs           # DOM → taffy tree conversion
│   ├── style_map.rs        # CSS computed styles → taffy::Style
│   ├── viewport.rs         # Virtual viewport (1920x1080 default)
│   ├── fonts.rs            # Font metrics (character widths for text sizing)
│   └── query.rs            # getBoundingClientRect, offset*, client*, scroll*
├── tests/
│   ├── basic_layout.rs
│   ├── flexbox.rs
│   └── bounding_rect.rs
└── Cargo.toml
```

## How It Works

```
DOM tree + computed styles
        │
        ▼
  ┌─────────────┐
  │ Convert DOM  │  Map each DOM element to a taffy node with
  │ → taffy tree │  Style { display, width, height, padding, margin, ... }
  └──────┬──────┘
         │
         ▼
  ┌─────────────┐
  │ taffy layout │  Compute position (x, y) and size (w, h) for every node
  │  algorithm   │
  └──────┬──────┘
         │
         ▼
  ┌─────────────┐
  │ Layout cache │  Store results, invalidate on DOM mutation
  └─────────────┘
```

## Font Metrics

To compute text layout, taffy needs to know how wide text is. We need basic font metrics without full font rendering:

| Crate | License | Purpose |
|---|---|---|
| `fontdb` | MIT | System font database (find fonts by family name) |
| `rustybuzz` | MIT | Text shaping (compute glyph advances/widths) |
| `ttf-parser` | MIT/Apache-2.0 | Parse TrueType/OpenType font files |

We load system fonts (or bundle a default), measure text widths, and feed them to taffy's `MeasureFunc`.

## JS API Mapping

| JS API | Implementation |
|---|---|
| `getBoundingClientRect()` | taffy layout position/size, offset by scroll position |
| `offsetWidth` / `offsetHeight` | taffy layout size including padding + border |
| `clientWidth` / `clientHeight` | taffy layout size including padding, excluding border + scrollbar |
| `offsetTop` / `offsetLeft` | Position relative to `offsetParent` |
| `scrollWidth` / `scrollHeight` | Content overflow dimensions |
| `window.innerWidth` | Virtual viewport width (default 1920) |
| `window.innerHeight` | Virtual viewport height (default 1080) |

## Lazy Computation

Layout is expensive. We only compute it when JS actually calls a layout API:

1. DOM mutation marks layout as dirty
2. `getBoundingClientRect()` triggers layout if dirty
3. Layout result is cached until next DOM mutation
4. Only the dirty subtree is re-laid-out (incremental)

## Virtual Viewport

No real screen. We simulate one:

```rust
pub struct Viewport {
    pub width: f32,          // 1920.0
    pub height: f32,         // 1080.0
    pub device_pixel_ratio: f32,  // 1.0
    pub scroll_x: f32,      // 0.0
    pub scroll_y: f32,      // 0.0
}
```

This matches the stealth profile's `screen` configuration.
