# PRD Request — Vector / PDF Rendering Backend

**Requested by:** pdf_oxide (sibling project, same author).
**Date filed:** 2026-04-18.
**Status:** Request for PRD — **not yet a committed roadmap item**. Filed so the browser_oxide team can evaluate scope, decide whether to accept, and (if accepted) write a real PRD.
**Tracking issue (downstream):** pdf_oxide#248 — "[Feature]: CSS support" (HTML+CSS → PDF in Python via pdf_oxide).

---

## TL;DR

pdf_oxide is evaluating browser_oxide as the rendering backend for an HTML+CSS → PDF feature. browser_oxide already owns the hard parts of a renderer (html5ever-based parsing, Stylo-grade CSS cascade with `@media`/specificity/inheritance, taffy-backed flex/block layout, fontdb + rustybuzz + swash for fonts). The blocker is that the canvas pipeline rasterizes to Skia immediately — there is no public stage at which a downstream consumer can intercept "draw glyph G of font F at (x,y)" or "fill rect (x,y,w,h)" as **vector** commands before they become pixels. PDF emission needs vectors, not pixels.

This document asks the browser_oxide team to consider exposing a **vector display-list backend** alongside the existing Skia raster backend, plus a small set of paged-media features (CSS `@page`, `page-break-*`, multi-page layout) that PDF output requires but a screen browser does not.

If accepted, downstream impact: pdf_oxide ships a credible WeasyPrint alternative without re-implementing a CSS engine. browser_oxide gains a second consumer (PDF) that exercises the layout/CSS/font stack on a different output surface, which generally surfaces correctness bugs that screen rendering hides.

---

## Why we are asking browser_oxide instead of building this in pdf_oxide

We surveyed the Rust HTML→PDF landscape (April 2026). The realistic options:

| Option | What we'd integrate | Why we'd reject it |
|---|---|---|
| **A. Stylo + Taffy + Krilla** (Blitz / hyper-render approach) | Mozilla's CSS engine + Taffy + a parallel PDF writer | Krilla is a *second* PDF writer competing with pdf_oxide's own — splits the project, duplicates font/image/encryption work |
| **B. Stylo + Taffy → pdf_oxide's writer** | Same CSS engine, our own PDF backend | Viable but means importing & maintaining Stylo's surface; we don't own the engine and have no leverage when CSS bugs surface |
| **C. browser_oxide → pdf_oxide's writer** | Your CSS/layout/font stack, our PDF backend | Best strategic fit: one author, two projects, shared engine. Blocked only by the items in this document |

We prefer (C). This document is what (C) requires from browser_oxide.

---

## What pdf_oxide brings to the table

So you can scope your side accurately, here's what we already have on the PDF emission side (verified in current codebase):

- **Near-raw PDF operator builder** — `pdf_oxide::writer::content_stream::ContentStreamBuilder` exposes ~60 PDF operators (`Tf`, `Tj`, `Td`, `Tm`, `re`, `f`, `S`, `B`, `q`, `Q`, `cm`, `W`, `Do`, ...) plus a `Raw(String)` escape hatch. Vector primitives in, PDF bytes out.
- **Image embedding** — JPEG/PNG/raw, DeviceGray / DeviceRGB / DeviceCMYK, alpha via SMask. ICC profile-driven colour conversion (qcms) shipped in v0.3.34.
- **Page model** — `PdfWriter::add_page(width, height) → PageBuilder`, arbitrary page sizes, multi-page documents.
- **Coordinate-system handling** — pdf_oxide expects callers to flip Y (PDF is bottom-left origin); we will own that flip in the integration glue.

### What pdf_oxide is **missing** (and will build itself, regardless of this request)

These are on pdf_oxide's roadmap independently — we are not asking browser_oxide to provide them:

- TrueType / OpenType font embedding with subsetting
- CIDFont (Type 0) for full Unicode coverage
- Glyph-id-indexed text-showing operators (so we can consume your shaped glyph runs directly)

This work is gated by demand from features beyond just HTML→PDF (signed PDFs, form rendering, custom-font generation), so it happens regardless. Mentioning it for completeness — pdf_oxide is not asking browser_oxide to pick this up.

---

## What we're asking browser_oxide to consider building

In priority order. Items 1 and 2 are the hard requirements; items 3 and 4 are the "useful first slice" of paged media; items 5+ are stretch.

### 1. Vector display-list output from layout — **hard requirement**

**Current state (verified file:line):**
- `crates/canvas/src/canvas2d.rs:799–821` — `fill_rect` calls directly into `skia_safe::Canvas` via `with_canvas()` (canvas2d.rs:1063–1079).
- `crates/canvas/src/text/raster.rs` — glyphs are rasterized via `swash` immediately after shaping; the output is RGBA pixels.
- `crates/canvas/src/text/shaper.rs:59–76` — `ShapedRun { glyphs: Vec<Glyph { glyph_id, x_advance, y_advance, x_offset, y_offset, cluster }> }` is `pub` (text/mod.rs:24), so the shaped form *is* extractable, but only if a consumer reaches in before `raster.rs` runs.

**What we need:**

A trait-based paint backend that the layout traversal calls into, so Skia is one implementor and a vector display list is another. Strawman:

```rust
pub trait PaintBackend {
    fn fill_rect(&mut self, rect: Rect, color: Color);
    fn stroke_rect(&mut self, rect: Rect, color: Color, width: f32);
    fn fill_path(&mut self, path: &Path, color: Color, fill_rule: FillRule);
    fn stroke_path(&mut self, path: &Path, color: Color, stroke: &StrokeStyle);
    fn draw_glyph_run(&mut self, run: &ShapedRun, font: &FontFace, size: f32, origin: Point, color: Color);
    fn draw_image(&mut self, image: &ImageData, dest: Rect, src: Option<Rect>);
    fn push_clip(&mut self, path: &Path);
    fn pop_clip(&mut self);
    fn push_transform(&mut self, transform: Affine);
    fn pop_transform(&mut self);
}
```

Two implementors ship: `SkiaPaintBackend` (existing behaviour, no perf regression) and `DisplayListBackend` (records into a `Vec<PaintCmd>` for downstream consumers like pdf_oxide).

The exact API shape is yours to design; this is just to anchor scope. The non-negotiable property is: **a downstream crate can consume layout output as vector commands without forking `crates/canvas` and without pulling in Skia.**

**Acceptance criterion:** pdf_oxide can call `browser_oxide::render_to_display_list(&dom, &stylesheet, viewport) → DisplayList` and translate every command to a `ContentStreamBuilder` call.

### 2. Pre-rasterization access to shaped glyph runs — **hard requirement**

`ShapedRun` already carries the right data. What we need is for the public render entry points to either (a) emit glyph runs into the paint backend (item 1), *or* (b) expose a "shape-only, do not raster" mode. Either is fine; (a) is cleaner because it falls out of item 1 naturally.

We also need access to the resolved `FontFace` (file path or `&'static [u8]` is enough — pdf_oxide will subset and embed it). `canvas/text/mod.rs:69` already returns `&'static [u8]`; we just need that surfaced on the public API.

### 3. Paged-media CSS, minimum viable subset

A screen browser does not need pagination; PDF output is built around it. The minimum useful subset:

- `@page { size: A4 | letter | <length> <length>; margin: ...; }` — page size & margins
- `@page :first`, `@page :left`, `@page :right` — first / verso / recto distinction
- `page-break-before`, `page-break-after`, `page-break-inside` (and their `break-*` modern equivalents) on block-level elements
- `@media print { ... }` evaluates to true under the PDF backend

`media.rs:57–101` already handles `@media print` (currently always returns false) — flip this to true when rendering for paged output.

**Out of scope for v1** (defer to a later PRD): GCPM-level features — `@page` margin boxes, `string-set`/`string()`, named pages, footnotes, running headers, `target-counter()`. WeasyPrint takes years to get these right; we don't need them for the first slice.

### 4. Page-break fragmentation in the layout engine

`crates/layout/src/engine.rs:56–62` runs a single `compute_layout()` pass for one viewport. PDF needs the layout tree fragmented across pages: a tall block becomes N page-sized fragments, with `page-break-*` rules respected and orphans/widows considered.

This is the genuinely hard bit. Taffy itself does not provide fragmentation. Realistic options for browser_oxide to evaluate:

1. **Post-pass fragmentation** — run layout once at infinite height, then walk the box tree and slice it into page-height chunks at allowed break points. Simpler, has known correctness issues with floats and abs-pos elements that span breaks.
2. **Iterative layout** — run layout per page with a "remaining content" continuation. More correct, much harder to implement on top of taffy.

WeasyPrint does (2). For a v1, (1) is acceptable and aligns with what most "HTML → PDF" tools actually ship. The choice is yours; we are flagging that the choice exists.

### 5. CSS coverage gaps that block real-world HTML → PDF

Verified gaps from the current `css_values::Property` enum (css_values:property.rs:10–65) that production HTML reports rely on:

- **Tables** (`display: table`, `table-row`, `table-cell`) — invoices, financial reports, scientific papers all use HTML tables. Currently absent from the property enum.
- **CSS Grid** (`display: grid`, `grid-template-*`, `grid-area`) — modern report layouts. Taffy supports it; just needs to be plumbed through cascade + layout dispatch.
- **`@font-face`** — currently the parser recognises the at-rule (css_parser:ast.rs:35–55) but there is no implementation. Web fonts are how branded reports get their typography.
- **`calc()`** — currently a stub (css_values/resolve.rs:41 `// TODO`). Pervasive in modern stylesheets.
- **`background-image: url(...)`**, `linear-gradient`, `border-radius`, `box-shadow` — visual polish that "report-style" HTML almost always uses.

Triage: **tables and `@font-face` are blockers** for a credible v1; the rest are nice-to-haves that can land incrementally.

### 6. Stretch — image format coverage for paint backend

When the paint backend hands an image to the consumer (item 1, `draw_image`), it should pass through the original encoded bytes when possible (JPEG → PDF can embed the JPEG byte stream directly via `DCTDecode`, no recompression). browser_oxide currently treats images as decoded pixel buffers for Skia consumption. Making the original bytes accessible saves us a lossy re-encode round trip.

---

## What good looks like

A single end-to-end example, runnable from pdf_oxide's tree:

```rust
let html = std::fs::read_to_string("invoice.html")?;
let css  = std::fs::read_to_string("invoice.css")?;

let display_list = browser_oxide::render::render_paged(
    &html,
    &css,
    PageSize::A4,
    Margins::uniform(20.0),
)?;

let mut pdf = pdf_oxide::PdfWriter::new();
for page in display_list.pages() {
    let mut builder = pdf.add_page(page.width, page.height);
    pdf_oxide::html_glue::emit(&mut builder, page);
    builder.finish();
}
let bytes = pdf.finish()?;
std::fs::write("invoice.pdf", bytes)?;
```

`pdf_oxide::html_glue::emit` lives in pdf_oxide and is purely a translation layer: `PaintCmd::FillRect` → `ContentStreamBuilder::rect().fill()`, `PaintCmd::DrawGlyphRun` → font registration + `Tf`/`Tj`/`Td`. We own that layer; you don't need to know it exists.

---

## Things this request explicitly does **not** ask for

Listed so you can confidently say "out of scope" if reviewers raise them:

- A PDF writer inside browser_oxide. pdf_oxide owns PDF emission.
- Font subsetting / Type 0 CID fonts. pdf_oxide owns font embedding.
- Encryption, forms, signatures, tagged PDF, PDF/A, ICC colour management. pdf_oxide owns these.
- Headless-Chrome compatibility, JS-driven rendering, network fetching for the PDF path. The PDF path is a sync, parse-string-in / commands-out pipeline; the JS/CDP/stealth machinery stays where it is and is not invoked from this code path.
- MathML, SVG filters, CSS animations, transitions — not needed for static-document HTML→PDF.

---

## Open questions for the browser_oxide team

We don't expect answers in this document — these are what a real PRD would resolve.

1. Is the paint-backend trait (item 1) acceptable architecturally, or do you prefer a different decoupling (e.g., a "renderer" that takes a `&dyn PaintBackend` parameter, vs a feature flag, vs a fork of `canvas` into `canvas-skia` and `canvas-vector`)?
2. Do you want pagination (items 3–4) inside `crates/layout` (where the box tree lives), or as a separate `crates/paged` consumer of layout output?
3. Tables and `@font-face` (item 5) — are these on browser_oxide's own roadmap independently? If yes, we just need to align timing; if no, this is the lift we're asking for.
4. License posture — pdf_oxide is MIT/Apache-2.0. browser_oxide README states "no MPL." Confirm that nothing in items 1–6 forces an MPL dependency (Stylo is MPL; we are *not* asking you to depend on Stylo, just flagging).
5. What's the right way for pdf_oxide to consume browser_oxide — workspace path dependency, git dependency, published crate? Affects release coupling.

---

## Decision requested

From the browser_oxide maintainers, one of:

- **Accept.** Pick a target release, write the real PRD, and we'll align pdf_oxide's v0.3.x cadence to land both sides together.
- **Accept partially.** Identify which subset (e.g., items 1–2 only) you'll commit to; pdf_oxide implements pagination and paged-media CSS itself on top.
- **Decline.** pdf_oxide falls back to option (B) above (Stylo + Taffy → pdf_oxide writer) and revisits browser_oxide integration in a later release.

Any of the three is a fine answer. The point of filing this is to make the choice explicit rather than have pdf_oxide silently drift toward option (B) by default.
