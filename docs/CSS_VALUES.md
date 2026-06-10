# css_values — CSS Property Value Parsing + Computed Values

New crate (not in original docs). Parses CSS property values from component values into typed Rust structs. Sits between css_parser (syntax) and css_cascade (application).

## Why a Separate Crate

css_parser handles CSS syntax generically — it doesn't know what `color: oklch(0.7 0.15 180)` means. css_values knows every CSS property's grammar and produces typed values that the layout engine and JS APIs can consume.

## Scope

### Properties We Must Parse (2026 critical set)

**Layout-affecting (getBoundingClientRect depends on these):**

| Property | Value Grammar | Notes |
|---|---|---|
| `display` | `none \| block \| flex \| grid \| inline \| inline-block \| ...` | Most-read property via getComputedStyle |
| `position` | `static \| relative \| absolute \| fixed \| sticky` | |
| `width`, `height` | `<length-percentage> \| auto \| min-content \| max-content \| fit-content()` | |
| `min-width/height`, `max-width/height` | same + `none` for max | |
| `margin` (shorthand + longhands) | `<length-percentage> \| auto` | |
| `padding` (shorthand + longhands) | `<length-percentage>` | |
| `border-width` (shorthand + longhands) | `<length> \| thin \| medium \| thick` | |
| `box-sizing` | `content-box \| border-box` | Critical — changes how width/height work |
| `overflow`, `overflow-x`, `overflow-y` | `visible \| hidden \| scroll \| auto \| clip` | |
| `flex` shorthand + longhands | `flex-grow`, `flex-shrink`, `flex-basis` | |
| `grid-*` properties | `grid-template-columns/rows`, `grid-area`, etc. | |
| `gap`, `row-gap`, `column-gap` | `<length-percentage>` | |
| `align-*`, `justify-*` | alignment keywords | |
| `float`, `clear` | keywords | |
| `font-size` | `<length-percentage> \| <absolute-size> \| <relative-size>` | Affects text layout |
| `font-family` | comma-separated family names | Font selection |
| `font-weight`, `font-style` | numeric / keywords | Font matching |
| `line-height` | `normal \| <number> \| <length-percentage>` | Text block height |
| `text-align` | keywords | |
| `white-space` | `normal \| nowrap \| pre \| ...` | Text wrapping |
| `transform` | `translate() \| scale() \| rotate() \| matrix() \| ...` | Affects getBoundingClientRect |

**Visibility (anti-bot honeypot detection):**

| Property | Notes |
|---|---|
| `visibility` | `visible \| hidden \| collapse` |
| `opacity` | `<number>` 0-1 |
| `z-index` | `auto \| <integer>` |
| `content-visibility` | `visible \| hidden \| auto` |

**Custom properties:**

| Feature | Notes |
|---|---|
| `var()` | Must resolve references, handle fallbacks, nested var() |
| `env()` | Safe area insets, viewport segments |
| `calc()`, `min()`, `max()`, `clamp()` | Math functions with mixed units |
| `@property` registered types | `syntax`, `initial-value`, `inherits` |

**Color (for getComputedStyle):**

| Function/Syntax | Notes |
|---|---|
| Named colors, `#hex`, `rgb()`, `rgba()` | Legacy |
| `hsl()`, `hsla()` | Legacy |
| `oklch()`, `oklab()`, `lab()`, `lch()` | 2024+ color spaces |
| `color-mix()` | Mix two colors in any color space |
| `color()` | Display-p3, srgb, etc. |
| `currentcolor`, `transparent` | Special values |

### Value Resolution Pipeline

```
Raw CSS text
    │  css_parser
    ▼
Component values (tokens)
    │  css_values
    ▼
Specified values (typed, per property)
    │  css_cascade (inheritance, var() resolution)
    ▼
Computed values (absolute, resolved)
    │  layout
    ▼
Used values (after layout)
```

## API Design

```rust
use css_values::{PropertyValue, parse_property, Color, Length};

// Parse a specific property
let display = parse_property("display", "flex")?;
assert_eq!(display, PropertyValue::Display(Display::Flex));

let color = parse_property("color", "oklch(0.7 0.15 180)")?;
assert_eq!(color, PropertyValue::Color(Color::Oklch { l: 0.7, c: 0.15, h: 180.0 }));

let width = parse_property("width", "calc(100% - 2rem)")?;
// PropertyValue::Width(LengthPercentage::Calc(...))

// Resolve var() references
let resolved = resolve_var("var(--spacing, 1rem)", &custom_properties)?;
```

## Architecture

```
css_values/
├── src/
│   ├── lib.rs
│   ├── property.rs         # PropertyValue enum (one variant per property)
│   ├── parse.rs            # Property name → parser dispatch
│   ├���─ types/
│   │   ├── length.rs       # Length, LengthPercentage, calc()
│   │   ├── color.rs        # Color (all color spaces + color-mix)
│   │   ├── display.rs      # Display values
│   │   ├── position.rs     # Position values
│   │   ���── flex.rs         # Flex shorthand/longhands
│   │   ├── grid.rs         # Grid values
│   │   ├── font.rs         # Font shorthand/longhands
│   │   ├── transform.rs    # Transform functions
│   │   ├── custom.rs       # var(), env(), custom properties
│   │   └── shorthands.rs   # Shorthand → longhand expansion
│   └── resolve.rs          # var() substitution, calc() evaluation
└── Cargo.toml
```
