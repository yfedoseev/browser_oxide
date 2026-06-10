# css_cascade — Cascade, Specificity, Inheritance, @layer, @media

New crate. Applies CSS rules to the DOM tree to produce computed styles per element.

## Scope

Implements the cascade algorithm from [CSS Cascading and Inheritance Level 4](https://www.w3.org/TR/css-cascade-4/) plus modern at-rule evaluation.

## What We Implement

### Cascade Algorithm

Given all applicable style rules for an element, determine which declaration wins:

1. **Origin & Importance** — User-agent < user < author; `!important` reverses order
2. **@layer ordering** — Unlayered > named layers (in declaration order). `!important` within layers reverses
3. **Specificity** — (a, b, c) from css_selectors
4. **Source order** — Last declaration wins

### @layer (Cascade Layers) — Critical for 2026

Tailwind v4 and modern CSS frameworks use `@layer`:

```css
@layer base, components, utilities;

@layer base {
    h1 { font-size: 2rem; }
}
@layer utilities {
    .text-lg { font-size: 1.125rem; }
}
```

Implementation:
- Track layer declaration order
- Anonymous layers (unnamed) are ordered after named layers in their parent
- Nested layers: `@layer framework.base { ... }`
- `@import url(...) layer(name)` — import into a layer

### @media Query Evaluation

Must evaluate against the virtual viewport from the stealth profile:

| Media Feature | Source |
|---|---|
| `width`, `height` | Viewport (default 1920x1080) |
| `device-pixel-ratio` / `resolution` | StealthProfile |
| `prefers-color-scheme` | StealthProfile (default `light`) |
| `prefers-reduced-motion` | StealthProfile (default `no-preference`) |
| `pointer`, `hover` | StealthProfile (`fine`/`hover` for desktop) |
| `orientation` | Derived from viewport w/h |
| `color`, `color-gamut` | `srgb` |
| `scripting` | `enabled` |

### @container Query Evaluation

Container queries check the size of an ancestor container element:

```css
.card-container { container-type: inline-size; }

@container (min-width: 400px) {
    .card { flex-direction: row; }
}
```

Implementation:
- Track elements with `container-type: size | inline-size`
- Evaluate container queries against the container's computed size (requires layout)
- Re-evaluate after layout changes (two-pass: layout → container query → re-layout)

### @supports Evaluation

```css
@supports (display: grid) { ... }
@supports not (container-type: inline-size) { ... }
@supports selector(:has(*)) { ... }
```

Check if a property:value or selector is supported. Must reflect our actual capabilities.

### Custom Property Resolution

`var()` substitution must happen during cascade:

```css
:root { --spacing: 1rem; }
.box { padding: var(--spacing); }           /* → 1rem */
.box { padding: var(--missing, 0.5rem); }   /* → 0.5rem (fallback) */
.box { padding: var(--a, var(--b, 1px)); }  /* → nested fallback */
```

`@property` registered properties get typed defaults and inheritance behavior:

```css
@property --color-primary {
    syntax: "<color>";
    initial-value: blue;
    inherits: true;
}
```

### Inheritance

Some properties inherit by default (color, font-*, line-height, etc.), others don't (margin, padding, border, etc.). Implementation:

1. If a property has a cascaded value → use it
2. Else if the property inherits → use parent's computed value
3. Else → use the property's initial value
4. `@property` registered custom properties may override inherit behavior

### Computed Value Resolution

Convert specified values → computed values:
- `em` → `px` (relative to parent font-size)
- `rem` → `px` (relative to root font-size)
- `%` → resolved against containing block
- `vh`/`vw` → resolved against viewport
- `currentcolor` → resolved to actual color value
- `var()` → substituted
- `calc()` → simplified where possible

## API Design

```rust
use css_cascade::{StyleEngine, ComputedStyle};

let mut engine = StyleEngine::new(viewport, media_features);

// Add stylesheets (parsed by css_parser)
engine.add_stylesheet(user_agent_styles, Origin::UserAgent);
engine.add_stylesheet(page_styles, Origin::Author);

// Compute style for an element
let style = engine.compute_style(&dom, element_id);
assert_eq!(style.display(), Display::Flex);
assert_eq!(style.color(), Color::Rgb(0, 0, 0));
assert_eq!(style.get_custom_property("--spacing"), Some("1rem"));
```

## Architecture

```
css_cascade/
├── src/
│   ├── lib.rs
│   ├── engine.rs           # StyleEngine — main entry point
│   ├── cascade.rs          # Cascade sort (origin, layer, specificity, order)
│   ├── layers.rs           # @layer ordering and resolution
│   ├── media.rs            # @media query evaluation
│   ├── container.rs        # @container query evaluation
│   ├── supports.rs         # @supports evaluation
│   ├── inheritance.rs      # Property inheritance
│   ├── computed.rs         # Specified → computed value resolution
│   ├── custom_properties.rs # var() substitution + @property
│   ├── matching.rs         # Rule → element matching (uses css_selectors)
│   └── initial.rs          # Initial values for all properties
└── Cargo.toml
```
