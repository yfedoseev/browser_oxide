# css_selectors — Selectors Level 4 Parser + Matcher

MIT/Apache-2.0 alternative to Servo's `selectors` crate (MPL-2.0).

## Scope

Implements the [Selectors Level 4](https://www.w3.org/TR/selectors-4/) specification:

1. **Selector parser** — Parses CSS selector strings into a typed AST
2. **Selector matcher** — Matches selectors against DOM nodes via a generic `Element` trait
3. **Specificity calculator** — Computes (a, b, c) specificity per §17

## Selector Types (what we implement)

### Simple Selectors (§5)

| Selector | Example | Spec Section |
|---|---|---|
| Type | `div`, `span` | §5.1 |
| Universal | `*` | §5.2 |
| Class | `.foo` | §5.4 |
| ID | `#bar` | §5.5 |
| Attribute | `[href]`, `[type="text"]` | §5.3 |
| Attribute operators | `=`, `~=`, `\|=`, `^=`, `$=`, `*=` | §5.3 |
| Attribute case | `[type="text" i]`, `[type="text" s]` | §5.3 |

### Pseudo-classes (§6-14)

| Category | Selectors |
|---|---|
| Linguistic | `:lang(en)` |
| Location | `:any-link`, `:link`, `:visited`, `:local-link`, `:target` |
| User action | `:hover`, `:active`, `:focus`, `:focus-within`, `:focus-visible` |
| Input | `:enabled`, `:disabled`, `:read-write`, `:read-only`, `:checked`, `:default`, `:indeterminate`, `:required`, `:optional`, `:valid`, `:invalid`, `:in-range`, `:out-of-range`, `:placeholder-shown` |
| Tree-structural | `:root`, `:empty`, `:nth-child(An+B)`, `:nth-last-child(An+B)`, `:first-child`, `:last-child`, `:only-child`, `:nth-of-type(An+B)`, `:nth-last-of-type(An+B)`, `:first-of-type`, `:last-of-type`, `:only-of-type` |
| Functional | `:is()`, `:not()`, `:where()`, `:has()` |

### Pseudo-elements (§15)

| Pseudo-element | Notes |
|---|---|
| `::before` | Content generation |
| `::after` | Content generation |
| `::first-line` | Text styling |
| `::first-letter` | Text styling |
| `::placeholder` | Input placeholder |
| `::selection` | User selection |

### Combinators (§16)

| Combinator | Syntax | Meaning |
|---|---|---|
| Descendant | `A B` | B is descendant of A |
| Child | `A > B` | B is direct child of A |
| Next-sibling | `A + B` | B immediately follows A |
| Subsequent-sibling | `A ~ B` | B follows A (not necessarily immediately) |
| Column | `A \|\| B` | Column combinator (Level 4) |

### Selector Lists (§4)

- Comma-separated selector lists: `h1, h2, h3`
- Forgiving selector lists in `:is()`, `:where()` — invalid selectors are dropped, not an error
- Non-forgiving lists in `:not()`, `:has()` — invalid selectors are an error

## API Design

### Generic Element Trait

The matcher works with any DOM representation via a trait:

```rust
/// Implement this for your DOM node type.
pub trait Element {
    type Impl: SelectorImpl;

    // Identity
    fn local_name(&self) -> &str;
    fn namespace(&self) -> Option<&str>;
    fn id(&self) -> Option<&str>;
    fn classes(&self) -> ClassIter<'_>;

    // Attributes
    fn has_attribute(&self, name: &str) -> bool;
    fn attribute(&self, name: &str) -> Option<&str>;

    // Tree traversal
    fn parent(&self) -> Option<Self>;
    fn prev_sibling(&self) -> Option<Self>;
    fn next_sibling(&self) -> Option<Self>;
    fn first_child(&self) -> Option<Self>;
    fn last_child(&self) -> Option<Self>;
    fn children(&self) -> ChildIter<Self>;

    // Tree position
    fn is_root(&self) -> bool;
    fn is_empty(&self) -> bool;

    // Pseudo-class state (consumer provides this)
    fn is_link(&self) -> bool;
    fn is_visited(&self) -> bool;
    fn is_hover(&self) -> bool;
    fn is_active(&self) -> bool;
    fn is_focus(&self) -> bool;
    fn is_enabled(&self) -> bool;
    fn is_disabled(&self) -> bool;
    fn is_checked(&self) -> bool;
}
```

### Parsing

```rust
use css_selectors::{SelectorList, parse_selector_list};

// Parse a selector string
let selectors = parse_selector_list("div.foo > span:first-child, #bar")?;

// Inspect the AST
for selector in &selectors {
    println!("specificity: {:?}", selector.specificity());
    println!("components: {:?}", selector.components());
}

// Parse with error recovery (forgiving, for :is()/:where())
let selectors = parse_selector_list_forgiving("div, ::invalid, span")?;
// Returns [div, span] — ::invalid is dropped
```

### Matching

```rust
use css_selectors::{matches_selector, query_selector, query_selector_all};

// Does this element match?
let matches: bool = matches_selector(&element, &selector);

// querySelector — first match (depth-first pre-order)
let first: Option<NodeRef> = query_selector(&root, "div.content > p")?;

// querySelectorAll — all matches
let all: Vec<NodeRef> = query_selector_all(&root, "a[href^='https']")?;
```

### Specificity

```rust
use css_selectors::Specificity;

let sel = parse_selector("div#main .content > p:first-child")?;
let spec = sel.specificity();
// Specificity(a=1, b=1, c=2)
// a: 1 ID (#main)
// b: 1 class (.content) + 0 pseudo-class (:first-child counts in b)
// Correction: :first-child is a pseudo-class → b
// b: 1 class + 1 pseudo-class = 2
// c: 2 type selectors (div, p)
// → Specificity(1, 2, 2)
```

## Architecture

```
css_selectors/
├── src/
│   ├── lib.rs              # Public API
│   ├── parser.rs           # Selector string → AST
│   ├── ast.rs              # Selector, SimpleSelector, Combinator, etc.
│   ├── specificity.rs      # Specificity computation (§17)
│   ├── matching.rs         # Selector matching against Element trait
│   ├── nth.rs              # An+B microsyntax parser (§14.4)
│   ├── element.rs          # Element trait definition
│   └── error.rs            # Parse errors with source locations
├── tests/
│   ├── parser_tests.rs     # Parsing various selector syntaxes
│   ├── matching_tests.rs   # Match against mock DOM
│   ├── specificity_tests.rs
│   └── nth_tests.rs        # An+B edge cases (odd, even, 3n+1, -n+6, etc.)
└── Cargo.toml
```

## Key Design Decisions

### 1. Generic over DOM representation

The `Element` trait lets css_selectors work with:
- browser_oxide's own DOM
- `html5ever` + `markup5ever_rcdom` trees
- `scraper` crate's nodes
- Any custom tree structure

This makes the crate useful outside browser_oxide.

### 2. Right-to-left matching

Selectors are matched right-to-left (as browsers do). For `div.foo > span`, we:
1. Check if the current element matches `span`
2. Check if its parent matches `div.foo`

This is O(n) in tree depth per selector, with early bailout.

### 3. Bloom filter for fast rejection

For `querySelectorAll`, we maintain a Bloom filter of ancestor class names, IDs, and tag names. This lets us reject selectors that can't possibly match without walking the tree. Borrowed technique from browser engines.

### 4. `:has()` with subject tracking

`:has()` requires matching relative to the subject element (upward/sibling matching). Implementation uses a scoped subtree walk from the subject, with cycle detection for complex cases.

### 5. Depends on css_parser

Selector parsing uses `css_parser`'s tokenizer for the low-level token stream (ident, hash, string, delimiters, etc.). This avoids reimplementing CSS tokenization.

## Conformance Target

- [Selectors Level 4](https://www.w3.org/TR/selectors-4/) — W3C Working Draft
- [An+B microsyntax](https://www.w3.org/TR/css-syntax-3/#anb-microsyntax) — from CSS Syntax Level 3
- Test suite: [web-platform-tests/wpt/css/selectors](https://github.com/nicolo-ribaudo/css-parser-tests)

## What This Crate Does NOT Do

- Parse CSS rules or declarations — that's css_parser
- Compute specificity ordering across multiple stylesheets — that's the cascade/style engine
- Handle CSS property values — that's the style engine
- Provide a DOM — consumers bring their own via the Element trait
