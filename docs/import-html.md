# HTML import (code to design): spike note

Status: **proposal, ordered by the owner on 24 Nov 2026 (decision 41)**, for GitHub issue 256 ("import HTML as editable nodes, symmetric with `export_frame`"), now tracked as epic E4.4 (issue 272, `docs/epics.md`). Written 2026-11-24 from a reading of the repository. No code was written or run for it, no number below was measured, and nothing here is built. "Spike" is the issue's word: this is a written mapping and a proposed tool signature, with no prototype, as decision 24 and D7 of `docs/standards/modular-thin-ui.md` ask (no new spikes; the first vertical slice is built in the repository with exit criteria written first). **The owner's order of 24 Nov 2026 covers the subset (decision 41).**

Decision 6 (`docs/decisions.md`) said "HTML import is deferred", and `PRODUCT.md` (Capabilities and Constraints), `docs/prd.md` (the out-of-scope list) and `docs/roadmap.md` (Out of scope) repeated it. This note prepared the lifting of that decision; decision 41 lifts it for the scoped importer of this note and for nothing else, and those three files now say what stays out. The seams and limits are recorded in ADR-0024 (`docs/adr/0024-html-import-with-a-bounded-parser-of-our-own-and-a-shared-css-crate.md`).

## 1. Purpose, direction and non-goals

The export goes from the canvas to code (`export_frame`, `docs/fidelity-contract.md`). This note is the other direction: HTML and CSS text, read by an agent from the project, becomes Nodes of a File as one batch of Commands, so an agent working in an existing codebase can pull a section onto the canvas, bind Variables to it (`docs/tokens-and-code.md`), edit it and export it again.

What it is not:

- **Not a URL fetch.** The tool takes text. `import_webpage` (a server that fetches a URL) is a later slice and needs an egress policy first: D3 of the standard wants every outbound call behind one gate with an Offline switch, and nothing like that exists yet. No `<link>`, `@import`, `url()` or `src` is ever followed.
- **No browser in the core, the server or the tool.** Geometry stays the layout core's (ADR-0003); the fidelity contract rejects "browser CSS as the source of truth" (section 9, approach (b)) because every non-browser consumer of geometry would need a browser. The importer therefore reads the CSS the author wrote. It does not compute the web page the way a browser does. Section 2 says what that costs and the one recipe that moves the cost to the agent.
- **No new write route.** The tool ends in `Shared::submit_ops`, the handler every transport ends in (`docs/architecture.md` section 3b, the MCP paragraph), as `import_tokens` and `make_variant` do. One `Command` pipeline for MCP, the web editor and undo (ADR-0001).
- **No code runs.** Scripts, event handlers and styles are text. `<script>` is not read.

## 2. Input contract

### 2.1 What is sent

An HTML fragment or a complete HTML text (with `<html>`, `<head>`, `<body>`), as one string, and optionally extra CSS as a second string. Both are text.

**Limit: 256 KiB for the two together, counted in bytes, not characters.** Reasons:

- It is the one precedent: `import_tokens` takes at most 262,144 bytes of CSS and refuses more as `E_IMPORT_TOO_LARGE` before it reads anything (`docs/tokens-and-code.md` section 1; `docs/error-codes.md`). The export output limit is the same 256 KiB (`MAX_OUTPUT_BYTES`, `E_EXPORT_TOO_LARGE`).
- It sits under the 1 MiB body limit of `Shared` (`max_body: 1 << 20` in `meldframe-http`) and under the MCP request size that `upload_asset` documents (about 1 MiB), so the limit is ours and not the transport's.
- Parse work is bounded by the input size, which is what lets the tool run under the call's slot and time limit on the blocking pool, as `import_tokens` does. Rate limits are spent before the body is parsed (NFR-5, `docs/architecture.md` section 9).
- The write limits are tighter than the input anyway (section 4: 100 Commands a call), so a larger limit would only buy larger refusals.

Proposed new code: `E_IMPORT_HTML_TOO_LARGE` (a code never changes meaning, so `E_IMPORT_TOO_LARGE`, which is about `css` of `import_tokens`, is not reused).

### 2.2 What is parsed

| Input | Read as |
|---|---|
| Tags and attributes | A tolerant, bounded tree builder over a documented subset: void elements, the usual implied end tags (`p`, `li`, `td` and friends), character references, comments dropped. Not the full HTML parsing algorithm (open decision 5). |
| `style="..."` | Declarations, highest priority of the author rules. |
| `<style>` blocks and the `css` argument | Rules with simple selectors (section 3.3). Same grammar for both. |
| `class="..."` | Matched against the rules above, and decoded as Tailwind utilities for the forms our export emits (below). |
| `<title>` | The name of the root frame, unless the call gives one. |
| `data-name` | The name of a node, so an export imports back with its node names. |
| `data-node` | Read for the round trip of an export, never used as the new id: the ids of an import are fresh (section 4), because importing an export into the file it came from would otherwise be `E_DUPLICATE_ID`. |
| Everything else (`script`, `link`, `meta`, event attributes, comments) | Not read; counted once by kind in the report. |

**Tailwind forms.** The export in `format: "tailwind"` writes a fixed vocabulary (`crates/features/export/export/src/tw.rs`): named utilities (`flex`, `flex-row`, `flex-col`, `shrink-0`, `overflow-hidden`, `self-stretch`, `items-start|center|end`, `justify-start|between`, `text-center|right`, `whitespace-pre|pre-wrap`, `w-fit`, `w-auto`, `h-auto`, `absolute`, `relative`, `pointer-events-none`, `content-['']`), arbitrary-value utilities (`left-[..]`, `top-[..]`, `inset-[..]`, `w-[..]`, `h-[..]`, `min-w-[..]`, `min-h-[..]`, `gap-[..]`, `p-[..]`, `m-[..]`, `flex-[..]`, `text-[length:..]`, `font-[..]`, `leading-[..]`, `text-[color:..]`, `bg-[color:..]`, `rounded-[..]`, `opacity-[..]`, `shadow-[..]`), arbitrary properties (`[prop:value]`, `_` for a space, `\_` for an underscore) and the `after:` variant for the border. The importer decodes exactly that vocabulary back into declarations and then runs the one mapping of section 3. That is what makes `export_frame` then `import_html` a round trip in both formats. Utilities from Tailwind's own scale and palette (`p-4`, `text-lg`, `bg-blue-500`, `rounded-lg`) and any other variant (`hover:`, `md:`, `dark:`) are not decoded: the class is listed as unresolved. An agent that has the project's compiled CSS sends it in `css`, where those classes are ordinary rules.

**No browser, so the agent resolves what a browser would.** Like `import_tokens` ("resolve a `tailwind.config.js`, an `@import` or a `url()` yourself first"), the agent inlines linked style sheets into `css`. An agent that has a browser can go one step further and send HTML whose styles are inline and already computed (px sizes, no percentages), which this importer reads best. That recipe is a suggestion for the tool text; no script in the repository does it, and its fidelity has not been measured.

### 2.3 Errors

The same structured errors as the rest of the surface: a tool error `{"ok": false, "error": {"code", "message", "retryable"}}` as `import_tokens` writes it (`meldframe-http-variables-import`), with the `E_` codes of `docs/error-codes.md`. Everything the commands can be refused for is refused by the same checks as `apply_commands`, so the error carries the `E_` code and the `index` of the Command, and the importer adds `source` (the element the Command came from). New codes proposed (the table in `docs/error-codes.md` would get its rows in the slice that adds them, as that file's test requires):

| Code | When |
|---|---|
| `E_IMPORT_HTML_TOO_LARGE` | `html` and `css` together are over 256 KiB. Nothing is read. |
| `E_IMPORT_HTML_TOO_MANY` | The import would write more than 100 Commands (open decision 3). The message says how many nodes the text makes and tells the agent to send one section at a time. Nothing is written. |
| `E_IMPORT_HTML_EMPTY` | No element makes a node (all `display:none`, only scripts, or no markup). |
| `E_IMPORT_HTML_UNSUPPORTED` | `strict` is true and at least one property was refused. The result lists them; nothing is written. |

Malformed HTML is not an error: the builder recovers by its documented rules and says so in the report when it had to.

## 3. Mapping

### 3.0 Reading the tables

- **supported**: the value reaches a model field that the contract lists as supported (`docs/fidelity-contract.md` section 4), and a Frame imported this way and exported again gives the same value. Its status is the contract's: a row the contract marks "partly" counts as supported here, and the Reason column names the contract's caveat (there is no fourth status).
- **approximated**: the value reaches the nearest thing the model holds. The node lists it under `approximated` with the source value and what was written. Fidelity to the source web page is not promised; fidelity to the imported File is (the export contract applies to the File, not to the page it came from).
- **refused**: not carried. The node lists the property under `unsupported`, with its value cut to 80 characters, and the fallback in the "Reason" column is what happens instead. A property the contract does not model is always refused here, never guessed: the contract is the list of what the canvas, the layout core and the export agree on, so a field outside it would have no oracle.

In the tables, "element" is an HTML element and "node" is a Node of the model.

Column "Maps to" names the node field by its JSON name (the rows of `NODE_FIELDS` in `crates/core/meldframe-render/src/coverage.rs`) and the contract section.

### 3.1 Model defaults that differ from CSS

An `addNode` that leaves a field out gets the default of `meldframe-protocol` (`node.rs`), and several defaults are not CSS's. The importer therefore writes these fields explicitly:

| Field | Model default if omitted | CSS initial value | The importer writes |
|---|---|---|---|
| `fill` | `#3f3f46` (`DEFAULT_FILL`) | transparent | `none` unless a background colour is authored |
| `clip` | `true` | `overflow: visible` | `false` unless `overflow` clips |
| `padding` | 16 | 0 | 0 unless authored |
| `gap` | 8 | 0 | 0 unless authored |
| `fontSize` | 16 | 16 px (medium) | the inherited size |
| `fill` of a text | `#3f3f46` | inherited `color` | the inherited `color` |
| `w`, `h` | 100 | auto | the authored size, else the stub measure for text; ignored by layout where a mode is hug or fill |
| `sw`, `sh` | `fixed` | auto | by the sizing rows below |

### 3.2 HTML elements

| Source | Maps to | Status | Reason and pointer |
|---|---|---|---|
| `div`, `section`, `article`, `header`, `footer`, `main`, `nav`, `aside`, `form`, `fieldset`, `figure`, `figcaption`, `address`, `ul`, `ol`, `li`, `dl`, `dt`, `dd`, `blockquote`, `body` | `frame` | **supported** | Node type frame is `div` in the export (contract 4.1). Unknown elements are read as one of these. |
| `p`, `h1` to `h6`, `span`, `a`, `label`, `strong`, `b`, `em`, `i`, `small`, `code`, `pre` with text and no box decoration | `text` | **supported** | Node type text is `p` in the export. With a background, border, shadow or padding the element is a `frame` with a `text` child, as a designer would draw it. |
| Inline children of different style inside one block (`<p>a <b>b</b></p>`) | one `text` with the block's style | **approximated** | The model has one style per text; text runs are "unsupported until proposed" (contract 4.3). The text is joined; the first differing property is listed. |
| `br` | `\n` in the text | **supported** | An explicit newline is a line break in the canvas and in CSS (contract section 6, item 4). |
| `img` | `rect` of the authored size, `fill: none`; the `src` is listed under `assets` | **approximated** | An image fill needs an asset id of the workspace (`E_INVALID_ASSET_ID`: 64 hex digits). Nothing is fetched; the agent calls `upload_asset` and then `updateNode` with `image`. `data:` URIs are not decoded in v1 (open decision 8). |
| `svg`, `canvas`, `video`, `audio`, `iframe`, `object`, `embed` | `rect` placeholder of the authored size, `fill: none` | **refused** | Content is not carried. SVG upload is refused until sanitisation exists (`docs/architecture.md` section 9). The placeholder keeps the room the element took in its parent. |
| `button` | `frame` (row, centred) with a `text` child | **approximated** | The browser's own button chrome (border, padding, background) is not applied: only what the author wrote. |
| `input` (text-like), `textarea`, `select` | `frame` with its authored box and a `text` child holding `value`, else `placeholder` | **approximated** | The native control is drawn by the browser, not by CSS; its look is lost. `placeholder` colour is the browser's, so the text takes the authored `color`. |
| `input` checkbox, radio, range, file, `option` | not created, counted under `skipped` | **refused** | No model equivalent. |
| `table`, `thead`, `tbody`, `tr`, `td`, `th` | `table` a column `frame`, `tr` a row `frame`, cells `frame`s that share the row equally (`sw: fill`) | **approximated** | Column widths shared across rows are lost; `colspan` and `rowspan` are refused. No table layout in the contract. |
| `ul`, `ol` list markers (bullets, numbers) | none | **refused** | Markers are generated content. The item's text is kept. |
| `display: none`, the `hidden` attribute | no node, counted under `skipped` | **supported** | Not painted in the source either. |
| `visibility: hidden` | the node with `opacity: 0` | **approximated** | The box keeps its room in CSS; the model has no visibility, only opacity (contract 4.3). |
| `::before`, `::after` with `content` (other than the export's border form, row in 3.6) | none | **refused** | Generated content has no node. Counted. |
| `a href`, `id`, `class`, `title`, `alt`, `data-*`, `aria-*`, `role` | used for names only | **refused** | The model has no links or attributes. One count per attribute name, not a line per node. `data-name` (else `id`, else the first class, else the element's role) is the node name, cut to 256 characters, with control and hidden characters removed as a batch note's are (GLOSSARY, Note). Semantic names matter: a default name comes back under `warnings` from `apply_commands`. |
| `script`, `style`, `link`, `meta`, `head`, `noscript`, `template`, comments | not nodes | **supported** | `style` and `title` are read; the rest are dropped and never run. |
| An element with no content, no authored size and no `display: flex` (only padding, border or background) | `frame` with `layout: column`, `sh: hug`, and `sw` by the block row below (`fill` under a block or column parent, else `hug`) | **approximated** | Hug is real only on a layout frame; on a rect or a frame without layout the core treats it as fixed (contract 4.1, `hug` row). The contract is silent on the size of an empty hug frame: the core gives it its padding and border, and the size is not promised. |
| An element with no children and no text, an authored px size, no `display: flex` and no padding | `rect` | **approximated** | The export writes `div` for a rect and for a frame (contract 4.1), so a childless frame without layout is not told from a rect. A rect holds no children and ignores layout and padding. |
| Content larger than an authored fixed `width` or `height` (overflow) | the authored size kept; the layout finding `overflow` is returned | **approximated** | Rule G3: overflow is unspecified, the oracle does not gate it (contract section 5). |
| A wrapper with no box-affecting declaration and one in-flow child; `display: contents` | no node; the child goes to the wrapper's parent | **approximated** | Keeps the tree under the depth limit of 32 and the count under 100. The node tree differs from the source. |

### 3.3 Selectors, cascade and values

| Source | Maps to | Status | Reason and pointer |
|---|---|---|---|
| `style` attribute | declarations | **supported** | |
| Rules with type, `.class`, `#id`, `[attr]`, `[attr=value]`, `*`, compounds of those, descendant and child combinators, selector lists, and `[data-node="id"]` with CSS escapes | declarations by specificity, then source order; `!important` over normal | **supported** | The export's `css` form selects `[data-node="<id>"]` (ADR-0019), and its escaping (hex escapes) must be read back. |
| Inheritance of `color`, `font-size`, `font-weight`, `text-align`, `white-space` | inherited values | **supported** | Text nodes need them (contract 4.2). `font-family` and `line-height` inherit too, and are refused where they land (3.5). |
| Browser default styles (`h1` size and weight, `strong` weight, `p`, `ul`, `ol` and `blockquote` margins, `body` margin 8 px) | a small fixed table | **approximated** | Needed for a web page with no reset; the table is part of the slice and is listed in the tool text. Anything not in it uses CSS initial values, not the browser's. |
| `:hover`, `:focus`, `:first-child`, `:nth-*`, `:not()`, `+`, `~` | rule skipped, selector label in the report | **refused** | State and structure selectors have no static meaning here. Precedent: a non-root rule is skipped with its selector in the report (ADR-0020). |
| `@media`, `@supports`, `@container` | rule skipped | **refused** | Responsive output is outside the contract (contract open question 10); no viewport is known. The base rules apply. |
| `@layer` | transparent | **supported** | As ADR-0020. |
| `@import`, `@font-face`, `@keyframes`, `@property`, `<link rel=stylesheet>`, `url()` | never followed, listed | **refused** | Nothing is fetched. Only Inter Variable is available (contract section 6). |
| `--x` and `var(--x[, fallback])` inside the input | resolved to the literal | **approximated** | The binding to a Variable of the File is not made in v1 (open decision 9); the literal is written. Chains of more than 16 steps and cycles skip the property (ADR-0020). |
| `calc()`, `min()`, `max()`, `clamp()`, `env()`, `attr()` | refused | **refused** | Not evaluated (ADR-0020). One form is read: `calc(var(--x) * 1px)`, which our export writes for a bound radius, when `--x` resolves. |
| `px` | number | **supported** | |
| `rem` (16), `em` (the element's font size), `pt` | px | **approximated** | Reported as a conversion, as ADR-0020 does for `rem`. |
| `%` | see the sizing, radius and gradient rows | **approximated** | Only where the row says. Elsewhere refused. |
| `vw`, `vh`, `vmin`, `vmax`, `ch`, `ex`, `lh` | refused | **refused** | No viewport; no font metrics beyond the measurer. |
| Colours: `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, `rgb()`, `rgba()`, `hsl()`, `hsla()`, named colours, `transparent`, `currentcolor` | hex (eight digits when alpha is below 1) | **supported** | The model takes hex only (`E_INVALID_COLOR`). Conversions are listed, as in ADR-0020. |
| `oklch()`, `oklab()` | hex, clamped to sRGB | **approximated** | Compiled Tailwind v4 CSS commonly carries oklch values. ADR-0020 sets the clamp rule and reports it. |
| `lab()`, `lch()`, `hwb()`, `color()`, `color-mix()`, `light-dark()`, system colours | refused | **refused** | No exact hex. |
| Tailwind classes in the export's vocabulary (section 2.2) | the declarations they stand for | **supported** | Inverse of `tw.rs`. |
| Other Tailwind utilities and variants | class listed as unresolved | **refused** | The scale and palette come from a theme the importer does not have; send the compiled CSS in `css`. |

### 3.4 Structure, layout and sizing

| Source | Maps to | Status | Reason and pointer |
|---|---|---|---|
| `display: flex`, `flex-direction: row \| column` | `layout` row or column | **supported** | Contract 4.1. |
| `flex-direction: row-reverse \| column-reverse` | in DOM order | **refused** | No reverse in the model. |
| `display: block`, `inline-block`, `inline`, `list-item`, `flow-root` with in-flow children | `layout: column`, `gap: 0`, `align: start`; a block child with `width: auto` is `sw: fill`, a text block is `fill` wide and `hug` high | **approximated** | Block flow, inline formatting, margin collapsing and floats are not modelled; the model's flows are row and column (contract 4.1). Block children stretching to the parent is `align-self: stretch` (rule G1 gives the same boxes). |
| `display: inline-flex` | as `flex` | **approximated** | The outer inline-ness is dropped. |
| `display: grid`, `inline-grid`, and every `grid-*` property | the frame is a `column` of its children in DOM order | **refused** | Grid is "unsupported until proposed" (contract 4.3). The refusal names each `grid-*` property it saw. |
| `gap` equal on both axes, or only the main axis set | `gap` | **supported** | Contract 4.1. |
| `row-gap` and `column-gap` that differ | the main-axis value | **approximated** | The model has one `gap`. |
| `padding`, per side, in px | `padding`, `padT`, `padR`, `padB`, `padL` | **supported** | Contract 4.3 (per-side padding in the model; applies to layout frames, which every container here is). Percent padding refused. |
| `margin`, per side, in px, on a child of a flex or block container | `marT`, `marR`, `marB`, `marL` | **supported** | Contract 4.3. A frame without layout ignores margin, as the contract says. |
| Negative margin; `margin: auto` | dropped | **refused** | `marT` and the others start at 0 (`docs/mcp-tools.json`, `apply_commands`). `margin: 0 auto` as centring is not carried. |
| Collapsing margins between block siblings | not collapsed | **approximated** | Boxes can be taller than the source. |
| `align-items: flex-start \| start`, `center`, `flex-end \| end` (with or without `safe`) | `align` | **supported** | Contract 4.1, rule G2. |
| `align-items: stretch` (the flex default), `align-self: stretch` | children with an auto cross size are `fill` on the cross axis | **approximated** | The contract names `align-self: stretch` on a child (4.1 `fill` row) and is silent on `stretch` as a parent value, so the parent value is carried as that child rule. |
| `align-items: baseline`; `align-self` other than `stretch` on one child | dropped | **refused** | Baseline is unsupported (contract 4.3); the model has `align` on the parent only. |
| `justify-content: flex-start \| start \| normal`, `center`, `flex-end \| end` (with or without `safe`), `space-between` | `justify` | **supported** | Contract 4.1, rule G2. A plain `center` overflows both sides in CSS and the core clamps; the contract's rule G3 says that regime is unspecified. |
| `justify-content: space-around \| space-evenly` | `start` | **refused** | Unsupported (contract 4.3). |
| `flex-wrap: wrap \| wrap-reverse` | no wrap | **refused** | Unsupported (contract 4.3). |
| `flex: 1`, `flex-grow: 1`, `flex: 1 1 0%`, `flex: 1 1 <px>` on a flex child | `sw` or `sh` `fill` on the main axis | **supported** | Contract 4.1, rule G1; the `<px>` form is what the export writes when fill siblings differ in padding. |
| `flex-grow` other than 1, `flex-basis` px with grow 0 | `fill` with an equal share; a fixed size | **approximated** | The model's fill shares equally. |
| `flex-shrink`, `order` | ignored; `order` dropped | **refused** | The model never shrinks; no order field (DOM order only). |
| `width`, `height` in px | `w`, `h`, `fixed` | **supported** | Contract 4.1. |
| `width: auto` on a text, or on a frame under a parent that hugs on that axis | `hug` | **supported** | Contract 4.1; rule G1 (a fill under a hug parent would be exported as fixed), so the importer writes `hug`, which CSS shrink-to-fit also gives. |
| `width: fit-content \| max-content` | `hug` | **supported** | What the export writes for a hug layout frame. |
| `width: min-content` | `hug` | **approximated** | |
| `width: 100%`, `height: 100%` | `fill` under a layout parent that is not hug on that axis; else `fixed` at the parent's px when it is known | **approximated** | Rule G1. Other percentages are refused. |
| `min-width`, `max-width`, `min-height`, `max-height`, `aspect-ratio` | dropped | **refused** | Unsupported (contract 4.3). Exception: `width: auto; min-width: <px>` on a text, the pin the export writes (rule G4), is read as `hug`: the importer's own measure replaces it. |
| `box-sizing: content-box` (the browser default) with px padding and border | `w`, `h` grown by padding and border | **approximated** | The model's size is the border box, as the export's `box-sizing: border-box` reset makes it. |
| `position: absolute` with `left` and `top` in px | `x`, `y`; with `absolute: true` when the parent is a flex container | **supported** | Contract 4.1 (`x`, `y` of a child of a frame without layout) and 4.3 (`absolute`). |
| `right`, `bottom`, `inset` with no `left` and `top` | dropped, the node sits at 0, 0 | **refused** | The parent's size is not known without layout. |
| `position: relative` with offsets; `position: fixed`, `sticky` | offsets dropped, in flow | **refused** | Sticky and fixed are unsupported (contract 4.3). `relative` as the origin of absolute children works as it is. |
| `z-index` | dropped | **refused** | Paint order is sibling order (`pos`). |
| `transform`, `rotate`, `scale`, `translate`, `skew` | dropped | **refused** | Unsupported until proposed (contract 4.3: rotation changes the box the oracle compares). |
| `float`, `clear`, `columns`, `vertical-align`, `object-position` | dropped | **refused** | No model field. |
| `overflow: hidden \| clip` | `clip: true` | **supported** | Contract 4.1. |
| `overflow: visible` | `clip: false` | **supported** | Written explicitly: the node default is `true`. |
| `overflow: auto \| scroll`, `overflow-x` and `overflow-y` that differ | `clip: true` | **approximated** | Scrolling is not modelled. |
| The root: `html` or `body`, or the top-level elements of a fragment | one `frame` that is a column; `w` from the CSS, else the `width` argument (1440); `sh: hug`; `body` background, margin and padding on it | **approximated** | No viewport and no responsive behaviour exist (contract section 3: design-size output, nothing responsive). Same default as the Desktop preset in the `apply_commands` text. |

### 3.5 Text

| Source | Maps to | Status | Reason and pointer |
|---|---|---|---|
| `font-size` in px | `fontSize` | **supported** | Contract 4.2 (fractional sizes verified). `rem` and `em` follow the conversion row in 3.3. Keywords (`large`) and percentages are refused. |
| `font-weight`: 100 to 900 in steps of 100, `normal` (400), `bold` (700) | `fontWeight` | **supported** | Contract 4.2. |
| `font-weight` such as 450 or 550 | the nearest step of 100 | **approximated** | Anything else is `E_INVALID_FONT_WEIGHT`. `bolder` and `lighter` are refused. |
| `text-align: left \| center \| right` | `textAlign` | **supported** | Contract 4.2. |
| `text-align: start \| end \| justify` | `left`, `right`, `left` | **approximated** | Left-to-right only; the model has three values. |
| `color`, inherited | `fill` of the text | **supported** | Contract 4.1 (`fill` on text), alpha included. |
| `white-space: nowrap \| pre` | text with `hug` width | **supported** | Contract 4.2: hug is `pre`. |
| `white-space: pre-wrap`, and a fixed or fill width | wrapping text | **supported** | Contract 4.2 (`pre-wrap`); row marked "partly" there (known gap `f14-text-edge`: break opportunities at slashes, URLs, CJK and tabs). |
| `white-space: normal \| pre-line` (the default) | collapsed spaces and newlines in the stored text | **approximated** | The canvas keeps repeated spaces (contract section 6, item 4), so the importer collapses them as the browser would. |
| `text-transform: uppercase \| lowercase \| capitalize` | applied to the characters | **approximated** | The style is lost; the look is kept. |
| Text with a tab, a URL without spaces, or CJK | copied as written | **approximated** | The contract says such text "must not be promised" (4.3; section 6, items 6 and 7; known gap `f14-text-edge`): Pixi and CSS break it differently. The result carries the `text_outside_contract` warning the export gives. |
| `font-family` | Inter Variable | **refused** | One family only (contract section 6, item 1); other families need a font registry (decision 19 open area). Text widths can differ from the source rendering. |
| `line-height` | dropped | **refused** | No `lineHeight` field (contract 4.3); the export pins the measured single-line height (section 6, item 2). |
| `letter-spacing`, `text-decoration`, `font-style: italic`, `text-indent`, `text-overflow`, line clamps, `word-break` | dropped | **refused** | Unsupported until proposed (contract 4.3). |
| `text-shadow`, `-webkit-text-stroke`, `background-clip: text` | dropped | **refused** | A text paints no shadow, gradient or border (export `on_text`). |

### 3.6 Paint

| Source | Maps to | Status | Reason and pointer |
|---|---|---|---|
| `background-color` | `fill` | **supported** | Contract 4.1 (`fill` colour). |
| No background, `transparent` | `fill: none` | **supported** | Contract 4.1 (`fill: none`, probed in f22); the node default would paint `#3f3f46`. |
| `linear-gradient()` with 2 to 8 stops, angle in `deg` or `to <side>`, stop positions in `%` (else evenly spaced) | `gradient` linear | **supported** | Contract 4.3 (angle probed at 0, 45, 90 and 180 only). Positions must not go back (`E_INVALID_GRADIENT`). |
| `radial-gradient(circle farthest-corner, ...)` | `gradient` radial | **supported** | The form the export writes (`gradient_css`). |
| `radial-gradient` with `ellipse`, a size keyword or `at <position>`; `conic-gradient`; `repeating-*`; stops in px; colour hints | not carried | **refused** | No model field for them. The `fill` remains. |
| More than one gradient, or more than one image, or more than 8 stops | the first gradient, the first image | **approximated** | The model holds one gradient and one image, over `fill` (contract 4.3: multiple fills planned). |
| `background-image: url(assets/<64 hex>)`, alone or followed by a gradient (`url(..),linear-gradient(..)`), with `background-size: cover \| contain \| 100% 100%` (two values, `cover,100% 100%`, when a gradient is the second layer), `background-position: center` and `background-repeat: no-repeat` | `image {assetId, fit}` and, when present, `gradient` | **supported** | The form the export writes (`decl.rs`: the image first, then the gradient, then size, position and repeat; `asset_base`, contract 4.2). `background-position: center` and `background-repeat: no-repeat` are the export's own single values and are written only with an image; `background-size` has two comma-separated values when a gradient is the second layer. Existence of the asset is not checked at write, as for `apply_commands`; the canvas reports `asset_not_found`. |
| Any other `url()`, and any other value of `background-size`, `background-position` or `background-repeat` | listed under `assets`; not carried | **refused** | Not fetched; the model has the three fits only. |
| `border` with equal `width`, `style: solid` and `color` on four sides | `border {width, color, position: inside}`, and the width added to the padding | **approximated** | A CSS border takes room; the model's border is paint only (contract 4.2, `border` row). Adding the width to the padding gives the same content box. |
| The export's border: `[data-node="x"]::after{content:""; position:absolute; pointer-events:none; inset: 0 \| -w/2 \| -w; border: w solid c; border-radius}` | `border {width, color, position}`, no padding change | **supported** | Contract 4.2; `inset` 0, `-w/2`, `-w` are inside, center, outside. This is the round trip of the export. |
| Per-side borders that differ; `dashed`, `dotted`, `double`; `border-image`; `outline` | no border | **refused** | Per-side and dashed borders are unsupported (contract 4.3). |
| `border-radius` equal on four corners, px | `radius` | **supported** | Contract 4.1. The canvas clamps to half the size. |
| `border-radius: 50%` on an element with no children | node type `ellipse` | **supported** | What the export writes for an ellipse (contract 4.1; `decl.rs` writes `50%` only for `NodeKind::Ellipse`). On a non-square box the canvas draws a pill and the export a true ellipse (contract 4.2, `border` row, difference 3). |
| `border-radius: 50%` on an element with children | `frame` with `radius` at half of the smaller authored side in px | **approximated** | Only a frame holds children (`E_PARENT_NOT_FRAME`), so it cannot be an `ellipse`. A box that is not square becomes a pill, not an ellipse. If a size is not authored in px the radius is refused and listed. |
| Different corner values; other percentages | no radius | **refused** | Per-corner radius is unsupported (contract 4.3). |
| `box-shadow`: up to 4 outer shadows, `x y blur spread color` in px | `shadows`, in reverse order | **supported** | Contract 4.2 (`shadows[]`; model first is bottom, CSS first is top). Blur is within 20/255 of the canvas, not exact. |
| `box-shadow` with `inset`, or a fifth and later shadow | dropped | **refused** | Inner shadows are unsupported (contract 4.3); the model holds 4 (`E_TOO_MANY_ENTRIES`). The four topmost are kept. |
| `opacity` (0 to 1, or a percentage) | `opacity` | **supported** | Contract 4.3 (probed in f18). The canvas fades each part alone, CSS fades the node as one image (contract 4.2, `border` row, difference 2). |
| `filter`, `backdrop-filter`, `mix-blend-mode`, `mask`, `clip-path`, `isolation` | dropped | **refused** | No model field. |
| `cursor`, `transition`, `animation`, `pointer-events`, `user-select`, `will-change` | dropped | **refused** | Behaviour, not paint. One count each in the report. The `pointer-events: none` of the export's own border form is part of that form (row above) and is not counted. |

### 3.7 What an export comes back as

For a File the export covers (contract section 4), `import_html` of its HTML gives the same node fields, with these exceptions, which the round-trip test (section 5) states as losses and does not hide:

- the ids (fresh), the `pos` strings (recomputed from the order) and the stored `w` and `h` of a node that is `hug` or `fill` on that axis (the export does not write them and the layout core ignores them);
- a `shader`: the export writes a gradient from `fill` to `fill2` (`shader_approximated`), so it comes back as a `rect` with a gradient;
- a Frame with no children and no layout comes back as a `rect` when it has a px size and no padding (the export writes `div` for both; see the row of 3.2). An empty Frame with layout is written with `display: flex` and comes back as a Frame;
- a binding to a Variable comes back as the literal (row in 3.3);
- an `image` whose asset is not in the workspace keeps its id and shows `asset_not_found`, as before.

### 3.8 Counts

The tables of 3.2 to 3.6 hold 111 rows: 41 supported, 34 approximated and 36 refused (the 8 rows of 3.1 are defaults, not mappings). These are row counts, not a measure of coverage: several rows group many properties, and a refused row is often one property of a common kind (`line-height`, grid). The count was taken with `grep` on the Status column and is to be taken again when a row changes.

## 4. Proposed tool: `import_html`

Change tier, Policy Auto by default (GLOSSARY: Risk tier, Policy), registered by a feature module like every tool (M1, M4 of the standard). It adds Nodes and never updates or removes one. It is served only under the Auto policy, as `import_tokens` is.

### 4.1 Arguments

| Name | Type | Notes |
|---|---|---|
| `file` | string, 1 to 128 | As every tool. |
| `html` | string, required | HTML text. |
| `css` | string, optional | Extra rules, same grammar as `<style>`. `html` and `css` together at most 262,144 bytes. |
| `parentId` | string, optional | An existing Frame to add into. Omitted: a new top-level Frame placed to the right of the others, as `make_variant` places its copy (`docs/epics.md`, E5.4). Not a frame: `E_PARENT_NOT_FRAME`; absent: `E_PARENT_NOT_FOUND`. |
| `name` | string, optional, at most 256 | Name of the root Frame. Default `<title>`, else "Imported HTML". |
| `width` | number, optional | Width of the root Frame when the source gives none. Default 1440. |
| `idempotencyKey` (or `idempotency_key`) | string, 1 to 100, required | As `apply_commands`. |
| `note` | string, optional | As `apply_commands`: at most 500 characters and 3 line breaks, no hidden characters. |
| `dryRun` | boolean, default false | Writes nothing; returns the same result, so an agent reads what would be refused before it writes. |
| `strict` | boolean, default false | True: any refused property fails the call as `E_IMPORT_HTML_UNSUPPORTED` before anything is written. |

No `ifSeq` and no `part`: see 4.3 and open decisions 3 and 4.

### 4.2 Result

```text
{
  "ok": true, "dryRun": false, "replayed": false,
  "seq": <file seq after the write>, "batch": {"n": 7, "note": "kept"},
  "root": "<id of the root frame>", "commands": 37,
  "nodes": [
    {"id": "...", "name": "...", "type": "frame", "source": "section.hero#2",
     "approximated": [{"property": "display", "from": "block", "to": "layout column"}],
     "unsupported": [{"property": "line-height", "value": "1.5", "code": "refused_not_modelled"}]}
  ],
  "skipped": [{"source": "div.modal#9", "code": "display_none"}],
  "assets": [{"node": "<id>", "use": "image", "src": "<as written, cut to 200>"}],
  "warnings": [ ... ],
  "report": {"counts": {...}, "entries": [...], "omitted": 0}
}
```

- `nodes` has one row per node written, at most 100, each with its source element (tag, first class, id and position) so an agent can map what it sent to what it got. Each node lists its `approximated` and `unsupported` properties, so a refusal is never silent and is attached to the node it affects.
- `warnings` carries what `apply_commands` and `export_frame` already return: default node names, the layout findings of `meldframe_layout::layout_findings` (`overflow`, `zero_fill`, `smaller_than_padding`, `fill_under_hug`), `text_outside_contract`, and `text_size_estimated`. The server has the stub measurer only (contract section 7), so the stored `w` and `h` of hug text are estimates; the editor with layout on the canvas computes its own with the page's font.
- `report` follows the bounds of `import_tokens`: at most 200 entries and 32 KiB of entries, a message cut to 240 characters, a value to 80, control and bidi characters dropped from anything echoed; `omitted` counts the rest.
- Error shape: section 2.3.

### 4.3 How it is applied

- The tool builds the Commands (an `addNode` per node, parent before child, in DOM order, so sibling order is the source order and `pos` is the server's to compute) and writes them with `Shared::submit_ops`: the same gate, quota, limits, atomic dry run (`meldframe-batch::check`) and call log as `apply_commands` (`docs/architecture.md` section 3b, the MCP paragraph; `import_tokens` is the precedent, ADR-0020, "The tool").
- **Atomic, one call, one Batch.** Up to 100 Commands (the batch limit, `docs/mcp-tools.json` `batchLimit`): all or none, one numbered Batch with the call's `note`, so the Activity panel shows one row and a Revert undoes one import. More than 100 is refused (open decision 3) rather than written in parts, because a part is its own Batch: a web page in several parts would be several rows, and a revert of the first would meet nodes that later parts added under it (`meldframe-revert` leaves a node alone when anything else touched its subtree, `docs/epics.md` E5.2).
- **Idempotency.** The op of Command `i` has the key `{idempotencyKey}:{digest}:{i}`, where the digest is a hash of the input text (ADR-0020 sets the form), so a call sent again after a timeout writes nothing twice (`replayed: true`) and the same key with other text is another call. Node ids are `imp-<hash of key and digest>-<i>`: at most 64 characters, letters, digits and `-`, so the same call gives the same ids, and the same text under another key gives other ids (a second import of one web page is legal; the ids do not come from the text alone).
- **`ifSeq`.** `apply_commands` guards a Command with `ifSeq` through the nodes it touches, and for `addNode` that is the new id only (`meldframe_batch::touches`). An add-only call therefore has nothing for `ifSeq` to protect: a parent that was deleted meanwhile fails the add (`E_PARENT_NOT_FOUND`), a parent that was edited meanwhile is not a conflict. The proposal is to leave `ifSeq` out of v1 and to say why in the tool text, which is an honest difference from the issue's wording (open decision 4).
- **Undo.** The ops are the actor's own and undo per actor as any op does. Undoing the creation of a Frame is answered `approval_required` by the server today, because a frame delete is Ask (`docs/epics.md` E3.2, the presets row): the tool text must tell the agent that an import it wants to take back is a Revert by a person, not an undo it can do.
- **Budgets.** Rate limits and the per-principal budget are spent by the middleware before the tool reads the body; the parse runs on the blocking pool under the call's slot and time limit (`Shared::blocking`); each Command is one op against the workspace's 6,000 a minute (`import_tokens` text), so one import is at most 100, about 1.7% of a minute. The parse is bounded before it builds anything: input bytes (256 KiB), elements (proposed 5,000), nesting depth (proposed 64, the export's own `MAX_DEPTH`; a node tree deeper than the file limit of 32 is `E_TOO_DEEP` from the same checks), rules (proposed 5,000), a value or attribute (proposed 1,000 bytes), and character-reference expansion. These numbers are proposals to be fixed with hostile-input tests, not measured limits.
- **Files.** A new file name is a new empty file, as for every tool. The file limits (20,000 nodes, text 20,000 characters, name 256, depth 32) are checked by the same code as every write; `E_TOO_LONG` on a text longer than the limit refuses the call and names the source.
- **The front end.** The importer is a pure function from text to Commands and a report, in a portable feature crate, so a later editor dialog can call it exactly as the Import variables dialog calls `variables-import` (decision 38: the browser reads the text, the result is the person's own undoable step, no new route). That dialog is not part of this proposal.

## 5. Validation plan

In the repository's style: tests first at the agreed seam, the contract's rows as the probes, the oracle as the judge, hostile text as a test class (ADR-0020), run under WASM too.

1. **One test per row of section 3** in a new tool crate (`meldframe-import-html-tests`, kind tool, no library code, run under WASM and added to the four places that list the WASM test crates (the `just wasm-test` recipe, both `nightly.yml` copies and the list in AGENTS.md), as AGENTS.md says for any new WASM test crate). The text in is the probe, the Commands out are asserted. A row marked refused asserts the property is listed on the node and that the field keeps the value written by section 3.1.
2. **A coverage test** like the one for `NODE_FIELDS`: every field of `Node` has a row in an import table that says supported, approximated or refused, and the test fails when a new field has none. It reads `NODE_FIELDS` and does not change `meldframe-render`.
3. **Round trip (Rust):** for fixtures in the supported subset (the fixtures of `tools/fidelity-oracle/fixtures.mjs`, f01 to f10 and f12 to f23, are the starting set, except f13 (the unspecified overflow regime of rule G3) and the known gaps f14-text-edge and f17-fill-under-hug (`known-gaps.mjs`), which are run as expected losses and not as round-trip passes) and for generated files: `export_frame`, then `import_html`, then compare every field of every node except the losses in 3.7; then `export_frame` again and compare the HTML after ids are renamed in order (a fixed point). Both formats, CSS and Tailwind.
4. **Oracle, two extensions** (`tools/fidelity-oracle`, `docs/fidelity-contract.md` section 7):
   - *round trip in the browser:* lay out the imported File with `compute_rects` and compare each node's rect with the original's within 0.05 px; load `export_frame` of the imported File as the gate already does;
   - *the source page is the judge:* for hand-written HTML in the supported subset (new fixtures `h01` onward), load the HTML as written in Chromium, read the box of each element, and compare it with the layout core's rect of the imported node (matched through `source` in the result), under the same measurer table, within the existing 0.05 px gate. This is the check that says the importer's reading of CSS agrees with a browser's, which no other test can. Fixtures that need the approximated rows are listed with their causes in `known-gaps.mjs`, in the way it already works (a gap that starts passing fails the job).
5. **Property tests** over generated HTML inside the supported subset (a generator of nested containers, rows, columns, text, fills, borders, radii, shadows, gradients, margins, padding, inline styles, `<style>` rules and the Tailwind forms): the Commands apply atomically to an empty file with no `E_` refusal (so the importer never emits what the checks refuse), the result is deterministic, ids are unique and fresh, the node count equals the Command count, a round trip is a fixed point, and a refused property never changes a written field. Seeds: 600 native, 20 under WASM, as the other property tests; `MELDFRAME_SEEDS` raises them. The generator is deliberately small, because the contract says the oracle fuzz "does not produce borders, shadows, images, weights, alignment, margins, variables or gradients" (section 7): the first extension of this one is those.
6. **Hostile text** (as `hostile.rs` of the variables import): every prefix of a document, unclosed tags and quotes, a million nested elements, a megabyte attribute, entity bombs, control and bidi characters in names and text, a `</style>` inside a string, `javascript:` values (never followed, never echoed unescaped). Bounded work is asserted with a counter, as `TEXT_WORK` does for the CSS scan.
7. **MCP tests against the real server** (`meldframe-import-html-http-tests`, tool): the answer shape, `dryRun`, replay, the codes of 2.3, the call log, the catalogue (`docs/mcp-tools.json` compared with what an rmcp client lists), the `build-matrix` check that the server without the feature has no `import_html` tool, and `meldframe-mcp-tests/tests/skill.rs` for the line in `skills/meldframe/SKILL.md`.
8. **Portability:** `cargo xtask portable-check` (wasm32) and `cargo xtask deps-check` for the new crates; `cargo xtask size-check` (500 lines a file, 4,000 a crate).

What this plan cannot show (so it is stated now): fidelity to web pages that use anything section 3 refuses, browsers other than the oracle's Chromium, the measurer on the server (stub, not Inter), and the rows marked "partly" by the contract.

## 6. Slicing into pull requests

Each is one kind of content and under 1,000 changed lines (AGENTS.md; lines moved do not count). Order matters: each depends on the one before.

| # | Slice | Crate or file, kind | Rough size |
|---|---|---|---|
| 0 | The owner's decision and the ADR: lift decision 6 for a scoped importer, the subset of section 3, the numbers of section 4 | `docs/decisions.md`, a new `docs/adr/00NN-*.md`, edits to PRD and roadmap (owner text) | 200 lines of docs |
| 1 | Shared CSS values: the colour and length readers move out of `variables-import` (`color.rs`, `value.rs`) into one crate both importers use, and `export`'s `hex_colour` follows if it fits | new `meldframe-css` (kind core, portable); `variables-import` depends on it. A pure move for `color.rs`; of `value.rs` only the number and length reading moves (see ADR-0024) | below 460 moved, about 150 changed |
| 2 | Tolerant HTML tree builder with bounds and hostile tests | new `html-import` (kind feature, group `html-import`, portable), modules `html/*` | about 800 |
| 3 | CSS rules, selectors, cascade, inheritance, defaults table, Tailwind decoder | same crate, modules `css/*`, `tw.rs` | about 900 |
| 4 | Mapping: structure, layout, sizing, text, to Commands, with the report | same crate, `map/*` | about 900 |
| 5 | Mapping: paint (fill, gradient, border, radius, shadow, opacity, image placeholders) | same crate, `paint/*` | about 700 |
| 6 | The tests of slices 2 to 5, split by layer so each slice lands with its own (a test first per slice, not a slice of its own); the crate `meldframe-import-html-tests` is created in slice 2 | tool crate | counted with each slice |
| 7 | The MCP tool: arguments, plan, `submit_ops`, answer, error codes, `docs/mcp-tools.json`, `docs/error-codes.md`, `skills/meldframe/SKILL.md`, the `html-import` cargo feature of `apps/server` | new `meldframe-http-html-import` (kind bin, as `meldframe-http-variables-import`) | about 600, plus tests |
| 8 | The oracle extensions and the fixtures `h01` onward; the contract gets a section on import and the known gaps | `crates/core/meldframe-oracle-runner`, `tools/fidelity-oracle`, `docs/fidelity-contract.md` | about 800 |
| 9 | Later, each its own decision: parts for HTML over 100 nodes, `data:` images through the assets seam, binding to Variables by export name, Tailwind's standard scale, an Import HTML dialog in the app layer, `import_webpage` | not scheduled | |

Layers and kinds, checked against `docs/architecture.md` section 3b and `docs/standards/modular-thin-ui.md` (L1 to L5):

- The importer is a **feature** crate: it contributes one agent tool through the registry (M1, M2), can be left out of a build (M3: the `build-matrix` builds the server with it and without it), and is not imported by another feature (L4). It may depend on core (`meldframe-protocol`, `meldframe-check`, `meldframe-layout` for the measurer and the layout findings) and on `meldframe-css`.
- **It must stay portable to `wasm32-unknown-unknown`** (L5, `portable-check`): no file or network access, no threads, no clock, no browser or DOM dependency, and any third-party crate must build for wasm32 and carry a licence on the allow list of `deny.toml` (MIT, Apache-2.0, Unicode-3.0, Zlib, BSD-3-Clause). Portability is not a nicety here: it is what lets the app layer reuse the importer in the browser later (section 4.3).
- Shared colour and length code goes to a **core** crate and not into a feature, because a feature crate may not import another feature of another group unless it is `composite` (L4), and a composite dependence would tie `html-import` to `variables-import` in every build (open decision 6).
- The tool adapter is a **bin** crate over `meldframe-http`, as the other tools are, because axum and rmcp are not portable.
- Sizes: the crate is planned at about 3,300 lines, inside the band above 2,500 where D1 calls it a candidate for a split; split `html-import` into a tree builder and a mapper at the point the second passes 1,500 lines, as `meldframe-app-parts` and others were.

## 7. Open decisions for the owner

Answered on 24 Nov 2026 by decision 41. The owner did not answer the ten items one by one: the order to go ahead is taken as adopting each recommendation below, and each is open to change. The text of each item is kept as it was written; each ends with its "Answered" marker.

1. **Lift decision 6 ("HTML import is deferred")?** It is in `docs/decisions.md`, `PRODUCT.md`, `docs/prd.md` and `docs/roadmap.md`. Recommendation: yes, for a scoped importer (this note), scheduled after Gatehouse (the item decision 36 puts next, after the canvas layout) as a new epic of M4, "E4.4 HTML import", and recorded as a new decision that supersedes 6 for this scope. `import_webpage`, Figma import and PSD import stay out. Until the owner answers, slices 1 onward do not start. **Answered:** decision 41 lifts decision 6 for the scoped importer, with E4.4 as its epic. The owner gave no timing: starting now, in parallel, with no wait for Gatehouse, is the author's reading of the order, not something the owner said, and it is reversible.
2. **Input: the CSS the author wrote, not a computed web page.** Recommendation: yes. The importer reads authored CSS with the subset of section 3, applies no browser algorithm, and the tool text suggests that an agent with a browser sends inline computed styles. The alternative, a headless browser on the server, is the approach the contract's section 9 (b) rejects and goes against the thin server of ADR-0002; decision 18 (headless capture) is still open and does not decide it. **Answered: adopted.**
3. **HTML that makes over 100 nodes: refuse, or write in parts?** Recommendation: refuse in v1 (`E_IMPORT_HTML_TOO_MANY`), one atomic Batch and one Revert, and have the agent send one section at a time under a `parentId`. Parts, as `import_tokens` has them, are not atomic as a whole and make one row per part in the Activity panel; decide after the first real use. **Answered: adopted.**
4. **`ifSeq` on an add-only call.** Recommendation: leave it out (section 4.3), because `addNode` touches only the new id and the argument would suggest a guard it does not give. The issue's wording asks for it; this needs the owner's yes. **Answered: adopted**, so `import_html` has no `ifSeq`; this is the one answer that differs from the wording of issue 256, and the owner may want to confirm it.
5. **The HTML parser: our own bounded, tolerant subset, or a crate such as `html5ever`.** Recommendation: our own, for the reasons of ADR-0020 (a small dependency tree, bounded by construction, nothing followed, built for wasm32, licences on the allow list), with the risk named: real-world HTML error recovery is large, and the subset will mis-nest some malformed input. The repository holds no HTML or CSS parser crate today (checked in `Cargo.lock`). Decide in the ADR of slice 0; not measured here. **Answered: adopted**, recorded in ADR-0024.
6. **Where the shared colour and length readers live.** Recommendation: a new core crate by a pure move out of `variables-import` (slice 1). The alternatives are a `composite` dependence of `html-import` on `variables-import`, or a second copy; both are worse for detachability and for drift. **Answered: adopted**, with a correction: `value.rs` is only partly moved (see ADR-0024), so the move is not pure for it.
7. **Refused properties: report them (default) or fail the call?** Recommendation: report, with `strict: true` for the agent that wants a failure. Every refusal is listed on its node either way. **Answered: adopted.**
8. **Images.** Recommendation: a placeholder `rect` and the `src` under `assets` in v1, nothing fetched, `data:` images and SVG later (SVG waits for sanitisation). A fetch is the later `import_webpage` slice and needs the egress gate of D3. **Answered: adopted.**
9. **Variables on import.** Recommendation: literals in v1 (a `var(--x)` is resolved from the input and written as its value), and a later option to bind by export name (`--<collection>-<name>`, ADR-0019) where the File already has the Variable, so an export imports back bound. It touches the file's Variables, so it is its own decision. **Answered: adopted.**
10. **Risk tier, registration and defaults.** Recommendation: Change tier, Policy Auto, its own cargo feature `html-import`, default on in `apps/server` like `export`; the Tailwind forms of section 2.2 only in v1 (the standard scale and palette later, once `css` carries a compiled theme). **Answered: adopted.**

The owner's order of 24 Nov 2026 covers the subset (decision 41).

## 8. What this note does not verify

- No importer exists. Every "approximated" row is a design, and its visible error against a real web page is not measured. The oracle extension of section 5 is what would measure it, and it is not built.
- The claims about the model come from reading `crates/core/meldframe-protocol/src/node.rs`, `meldframe-check/src/limits.rs`, `meldframe-render/src/coverage.rs`, the export crate and the tool text in `docs/mcp-tools.json`; the claims about CI and crates come from `AGENTS.md`, `docs/architecture.md` and `deny.toml`. Nothing was built or run to confirm them.
- Where the contract is silent: the size of an empty hug frame, percentages, a browser's default styles, `align-items: stretch` as a parent value, inline formatting, and overflow beyond rule G3 (unspecified there). Each has a row in the tables above, marked approximated or refused, and the importer's choice in it is a proposal.
- The proposed limits (elements, depth, rules, value length) and the codes `E_IMPORT_HTML_*` are the starting numbers and names that ADR-0024 accepts (decision 41), to be tuned by the first measurements; no limit has a measurement behind it yet.
- Whether `html5ever` or another crate builds for wasm32 or fits the licence list was not checked.
