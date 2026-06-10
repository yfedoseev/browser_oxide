# css_parser — CSS Tokenizer + Parser (with Nesting)

MIT/Apache-2.0 alternative to Servo's `cssparser` (MPL-2.0).

## Scope

Implements the [CSS Syntax Level 3](https://www.w3.org/TR/css-syntax-3/) specification plus the [CSS Nesting Module](https://www.w3.org/TR/css-nesting-1/):

1. **Tokenizer** — CSS source text → token stream
2. **Parser** — Token stream → tree of rules, declarations, and component values
3. **Nesting** — Qualified rules inside qualified rules (native CSS nesting, not Sass)

This crate does **not** interpret specific CSS properties or at-rules semantically. That's the job of css_values (property parsing) and css_cascade (at-rule evaluation).

## What We Implement

### Tokenizer (CSS Syntax §4)

All token types per spec:

| Token | Example | Spec Section |
|---|---|---|
| `Ident` | `color`, `--custom` | §4.3.11 |
| `Function` | `rgb(`, `var(`, `env(` | §4.3.4 |
| `AtKeyword` | `@media`, `@layer`, `@container`, `@scope`, `@property` | §4.3.3 |
| `Hash` | `#fff`, `#id` | ��4.3.6 (id vs unrestricted) |
| `String` | `"hello"`, `'world'` | §4.3.5 |
| `Number` | `42`, `3.14`, `1e10` | §4.3.12 |
| `Percentage` | `50%` | §4.3.13 |
| `Dimension` | `10px`, `2em`, `100vh`, `1cqw` | §4.3.14 |
| `Url` | `url(image.png)` | §4.3.6 |
| `Whitespace` | ` `, `\t`, `\n` | §4.3.1 |
| `Delim` | `+`, `>`, `~`, `&` | catch-all single code point |
| `Colon`, `Semicolon`, `Comma` | `:`, `;`, `,` | |
| `OpenSquare/CloseSquare` | `[`, `]` | |
| `OpenParen/CloseParen` | `(`, `)` | |
| `OpenCurly/CloseCurly` | `{`, `}` | |
| `CDO` / `CDC` | `<!--`, `-->` | legacy |
| `BadString` / `BadUrl` | unterminated | error recovery |

### Parser (CSS Syntax §5 + Nesting Module)

Parses token stream into CSS grammar structures:

- **Stylesheet** — list of rules
- **QualifiedRule** — selector + block, may contain **nested rules** interleaved with declarations
- **AtRule** — at-rules with optional prelude and block
- **Declaration** — property: value (with `!important` flag)
- **ComponentValue** — preserved tokens, functions, simple blocks
- **Function** — name + list of component values
- **SimpleBlock** — `{...}`, `[...]`, or `(...)` with contents

### CSS Nesting (Critical for 2026)

Native CSS nesting is now shipped in all browsers and actively used by frameworks (Tailwind v4, PostCSS output). The parser must handle:

```css
.card {
    color: black;

    /* Nested rule — & refers to parent selector */
    & .title { font-weight: bold; }

    /* Implicit nesting (starts with combinator/selector) */
    .title { font-weight: bold; }

    /* Nested at-rule */
    @media (width > 768px) {
        padding: 2rem;
    }

    /* Deeply nested */
    &:hover {
        color: blue;
        & .icon { opacity: 1; }
    }
}
```

This changes the parsing algorithm: a qualified rule's block contains **both declarations and nested rules**, interleaved. The parser must distinguish between `property: value;` and a nested `selector { ... }`.

### At-Rules We Must Parse Syntactically

The parser handles these at the syntax level (semantic evaluation is in css_cascade):

| At-Rule | Prelude | Block | 2026 Importance |
|---|---|---|---|
| `@media` | media query list | rule list | Critical — responsive design |
| `@layer` | layer name(s) | rule list or statement | Critical — Tailwind v4 |
| `@container` | container query | rule list | High — component libraries |
| `@supports` | condition | rule list | High — feature detection |
| `@font-face` | none | declaration list | High — web fonts |
| `@keyframes` | name | keyframe rule list | Medium — animations |
| `@property` | none (dashed-ident in block) | declaration list | Medium — typed custom props |
| `@scope` | scope start/end | rule list | Growing |
| `@import` | url + conditions | none (statement) | Medium |
| `@charset` | encoding | none (statement) | Legacy |
| `@namespace` | prefix + url | none (statement) | Legacy |

### Error Recovery (§3.2)

CSS is forward-compatible. The parser must:
- Skip unknown at-rules without crashing
- Skip invalid declarations
- Balance brackets even in error cases
- Never panic on malformed input
- Preserve source locations for error reporting

## API Design

```rust
use css_parser::{Tokenizer, Token, Parser, Stylesheet, Rule};

// Low-level: tokenize
let mut tokenizer = Tokenizer::new("h1 { color: red; }");
for token in &mut tokenizer {
    println!("{:?}", token);
}

// High-level: parse stylesheet
let stylesheet = Parser::parse_stylesheet(css_source)?;
for rule in &stylesheet.rules {
    match rule {
        Rule::Qualified(qr) => {
            println!("selector: {:?}", qr.prelude);
            println!("declarations: {:?}", qr.declarations);
            println!("nested rules: {:?}", qr.rules);  // nested!
        }
        Rule::At(at) => {
            println!("@{} ...", at.name);
        }
    }
}

// Parse inline style attribute
let decls = Parser::parse_declaration_list("color: red; font-size: 16px")?;

// Parse a single component value
let value = Parser::parse_component_value("rgb(255, 0, 0)")?;
```

## Architecture

```
css_parser/
├── src/
│   ├── lib.rs              # Public API re-exports
│   ├── tokenizer.rs        # CSS Syntax §4 — code-point-by-code-point tokenizer
│   ├── token.rs            # Token enum + associated data
│   ├── parser.rs           # CSS Syntax §5 + Nesting — rules, declarations, values
│   ├── ast.rs              # Stylesheet, Rule, QualifiedRule, AtRule, Declaration
│   ├── error.rs            # ParseError with source location (line, column, offset)
│   ��── source.rs           # SourcePosition tracking, input preprocessing (§3.3)
├── tests/
│   ├── tokenizer_tests.rs  # Token-by-token spec conformance
│   ├── parser_tests.rs     # Rule/declaration parsing
│   ├── nesting_tests.rs    # CSS nesting edge cases
│   └── error_recovery.rs   # Malformed input handling
└── Cargo.toml
```

## Key Design Decisions

1. **Zero-copy where possible** — Tokens borrow from input `&str`. `.to_owned()` when needed.
2. **Streaming tokenizer** — `Iterator<Item = Token<'a>>`. No buffering the full stream.
3. **Spec-literal implementation** — Each spec algorithm maps to a named method for easy auditing.
4. **Source locations on everything** — `SourceLocation { offset, line, column }` on every token and AST node.
5. **No `unsafe`** — Performance via zero-copy borrows, not unsafe.
6. **Nesting-aware** — QualifiedRule blocks contain interleaved declarations and nested rules.

## Conformance Targets

- [CSS Syntax Level 3](https://www.w3.org/TR/css-syntax-3/) — W3C Candidate Recommendation
- [CSS Nesting Module Level 1](https://www.w3.org/TR/css-nesting-1/) — W3C Candidate Recommendation
- [CSS Values and Units Level 4](https://www.w3.org/TR/css-values-4/) — for numeric/dimension tokens
- Test suite: css-parser-tests JSON fixtures
