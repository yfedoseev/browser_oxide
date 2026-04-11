# dom — Mutable DOM Tree + Web APIs + Shadow DOM + iframes

The core data structure of browser_oxide. Every other crate either produces, queries, or mutates this tree.

## Scope

Implements the subset of [DOM Living Standard](https://dom.spec.whatwg.org/), [HTML Living Standard](https://html.spec.whatwg.org/), and [Shadow DOM](https://dom.spec.whatwg.org/#shadow-trees) needed for SOTA 2026 web scraping.

## Node Types

```rust
pub enum NodeData {
    Document {
        url: Url,
        title: String,
        mode: DocumentMode,     // quirks, limited-quirks, no-quirks
    },
    DocumentType {
        name: String,
        public_id: String,
        system_id: String,
    },
    Element {
        name: QualName,
        attrs: Vec<Attribute>,
        template_contents: Option<NodeId>,
        shadow_root: Option<NodeId>,    // Shadow DOM
        custom_element_state: CustomElementState,
    },
    Text(String),
    Comment(String),
    ProcessingInstruction { target: String, data: String },
    DocumentFragment,
    ShadowRoot {
        mode: ShadowRootMode,          // Open or Closed
        host: NodeId,                  // Element this shadow is attached to
        delegates_focus: bool,
    },
}

pub enum ShadowRootMode { Open, Closed }
pub enum CustomElementState { Undefined, Failed, Uncustomized, Precustomized, Custom }
```

## Tree Storage: Arena Allocation

```rust
pub struct Dom {
    nodes: Vec<Node>,
    free_list: Vec<NodeId>,
}

pub struct Node {
    pub data: NodeData,
    pub parent: Option<NodeId>,
    pub first_child: Option<NodeId>,
    pub last_child: Option<NodeId>,
    pub prev_sibling: Option<NodeId>,
    pub next_sibling: Option<NodeId>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(usize);
```

Arena gives us: O(1) node access, cache-friendly traversal, no Rc<RefCell<>> overhead, easy JS engine integration (NodeId is a lightweight handle).

## Shadow DOM

~20% of page loads use Custom Elements (2026). Anti-bot widgets (Turnstile, hCaptcha) may use Shadow DOM for isolation. Frameworks like Lit, Ionic, and Angular use it heavily.

### What We Implement

1. **`Element.attachShadow({mode})`** — Creates a ShadowRoot node attached to the element
2. **ShadowRoot** — DocumentFragment-like node, root of shadow tree
3. **`<slot>` content distribution** — Light DOM children distributed into shadow tree slots
   - Default slot (unnamed)
   - Named slots (`<slot name="header">`)
   - `HTMLSlotElement.assignedNodes({flatten: true})`
   - `slotchange` event
4. **CSS scoping** — Styles in shadow tree don't leak out; external styles don't penetrate in
   - `:host`, `:host()`, `::slotted()`, `::part()`
   - CSS custom properties DO cross shadow boundaries
5. **Event retargeting** — Events from shadow tree retarget to host element at shadow boundary
   - `event.composedPath()` reveals full path
   - Only `composed: true` events cross boundaries (click, focus, input do; custom events don't by default)
6. **`element.shadowRoot`** — Returns ShadowRoot for `mode: 'open'`, null for `mode: 'closed'`

### Flat Tree

For layout and selector matching, we need the **flat tree** — the composed view that combines light DOM and shadow DOM through slots:

```
Light DOM:          Shadow DOM:           Flat Tree:
<my-card>           #shadow-root          <my-card>
  <h2>Title</h2>     <div class="wrap">    #shadow-root
  <p>Body</p>          <slot></slot>         <div class="wrap">
</my-card>           </div>                    <h2>Title</h2>  (slotted)
                                               <p>Body</p>     (slotted)
                                             </div>
```

## iframes / Nested Browsing Contexts

iframes are **critical** for anti-bot — Cloudflare Turnstile, reCAPTCHA, and hCaptcha all run inside cross-origin iframes.

### What We Implement

1. **Separate DOM + JS context per iframe** — Each iframe gets its own Document, Window, and V8 Context
2. **Same-origin access** — Parent can access `iframe.contentWindow` and `iframe.contentDocument`
3. **Cross-origin isolation** — `contentDocument` returns null. Only `postMessage()` communication
4. **`window.postMessage()`** — MessageEvent with structured clone data, origin checking
5. **`srcdoc` attribute** — Inline HTML content (`about:srcdoc` origin)
6. **`sandbox` attribute** — `allow-scripts`, `allow-same-origin`, `allow-forms`, etc.
7. **`contentWindow` / `contentDocument`** — Primary JS access APIs
8. **Origin checking** — Same-Origin Policy: protocol + host + port must match
9. **Lazy loading** — Defer offscreen iframes; eagerly load in-viewport and JS-accessed iframes

### Anti-bot iframe pattern

```
Page loads Turnstile widget
  → Creates cross-origin iframe (challenges.cloudflare.com)
  → iframe runs WASM proof-of-work + canvas fingerprint + env checks
  → iframe sends token back to parent via postMessage
  → Parent includes token in form submission
```

Without working iframes + postMessage, Turnstile/reCAPTCHA/hCaptcha are completely broken.

## Web API Interfaces

### Priority 0 — Anti-bot critical

| Interface | Key Methods/Properties |
|---|---|
| `document.hasFocus()` | Must return `true` (anti-bot checks this) |
| `EventTarget` | `addEventListener()`, `removeEventListener()`, `dispatchEvent()` |
| `Event` | `type`, `target`, `composedPath()`, `preventDefault()`, `stopPropagation()` |
| `MessageEvent` | For postMessage/iframe communication |
| `Window.postMessage()` | Cross-origin iframe communication |
| `HTMLIFrameElement` | `contentWindow`, `contentDocument`, `src`, `srcdoc`, `sandbox` |

### Priority 1 — Required for basic scraping

| Interface | Key Methods/Properties |
|---|---|
| `Node` | `nodeType`, `parentNode`, `childNodes`, `firstChild`, `lastChild`, `textContent`, `appendChild()`, `removeChild()`, `insertBefore()`, `cloneNode()`, `contains()` |
| `Element` | `tagName`, `id`, `className`, `classList`, `getAttribute()`, `setAttribute()`, `innerHTML`, `outerHTML`, `children`, `querySelector()`, `querySelectorAll()`, `matches()`, `closest()`, `getBoundingClientRect()`, `attachShadow()`, `shadowRoot` |
| `Document` | `documentElement`, `head`, `body`, `getElementById()`, `getElementsByClassName()`, `getElementsByTagName()`, `querySelector()`, `querySelectorAll()`, `createElement()`, `createTextNode()`, `createDocumentFragment()`, `title`, `URL`, `cookie`, `readyState`, `fonts` |
| `HTMLElement` | `style`, `dataset`, `offsetWidth`, `offsetHeight`, `offsetTop`, `offsetLeft`, `scrollTop`, `scrollLeft`, `click()`, `focus()`, `blur()`, `checkVisibility()` |

### Priority 2 — Required for SPAs

| Interface | Key Methods/Properties |
|---|---|
| `MutationObserver` | `observe()`, `disconnect()`, `takeRecords()` |
| `IntersectionObserver` | `observe()`, `unobserve()`, `disconnect()` |
| `ResizeObserver` | `observe()`, `unobserve()`, `disconnect()` |
| `DOMTokenList` | `add()`, `remove()`, `toggle()`, `contains()`, `replace()` |
| `NodeList` / `HTMLCollection` | `item()`, `length`, `forEach()` |
| `DOMParser` | `parseFromString()` |
| `XMLSerializer` | `serializeToString()` |
| `HTMLSlotElement` | `assignedNodes()`, `assignedElements()`, `assign()` |
| `ShadowRoot` | `mode`, `host`, `innerHTML`, `querySelector()` |
| `HTMLTemplateElement` | `content` (DocumentFragment) |

### Priority 3 — Required for complex sites

| Interface | Key Methods/Properties |
|---|---|
| `TreeWalker` | Full traversal API |
| `Range` | Selection and manipulation |
| `FormData` | Form serialization |
| `CustomElementRegistry` | `define()`, `get()`, `whenDefined()` (for Web Components) |

## innerHTML / outerHTML

Critical for SPAs — frameworks build UI by assigning HTML strings:

1. Parse HTML fragment with html5ever (fragment parsing algorithm)
2. Remove existing children
3. Append parsed nodes
4. Trigger MutationObserver callbacks

## Events

Full [DOM Events](https://dom.spec.whatwg.org/#events) with three phases:

1. **Capture** — root → target
2. **Target** — at the target
3. **Bubble** — target → root
4. **Composed** — events cross shadow boundaries when `composed: true`

```rust
struct EventListener {
    event_type: String,
    callback: JsFunction,  // V8 function handle
    capture: bool,
    once: bool,
    passive: bool,
}
```

## Design Decisions

1. **Arena allocation** — NodeId is Copy, no lifetime issues, integrates with V8 GC via weak handles
2. **Lazy HTMLCollection / NodeList** — Re-traverse on access, not pre-computed
3. **Flat tree caching** — Shadow DOM flat tree computed lazily, invalidated on slot/mutation
4. **iframe isolation** — Each iframe's DOM is a separate `Dom` instance with its own arena
