# HTML/Tailwind fidelity contract

Status: **proposal** for decision 15 (`docs/decisions.md`). Written 2026-11-13 from an experiment on the Wonder spike
(`wonder-proto`, branch `spike/rust-core`, commit `ca74052`). The experiment harness is in
`experiments/fidelity-oracle/` (see its README). Every number below was produced by that harness unless it is marked
"from the spike docs".

Since E4.2 the oracle runs against the product export (the `export` crate) from `tools/fidelity-oracle/`, in pull request CI
and nightly; section 7 says what it checks now and what it cannot prove, and the status of each row of section 4 says what
the product export emits. The sections that describe the experiment (8, 12) keep the numbers of the spike run.

Section 12 lists what the experiment does **not** prove. Read it before quoting any number from this document.

## 1. Purpose

Meldframe promises that the exported HTML/Tailwind is "faithful" to what the canvas shows. Without a definition, that
word cannot be tested, and an agent or a customer cannot rely on it. This contract:

1. defines what "faithful" means, in pixels, so that it can be tested by a machine;
2. lists the model properties that are inside the contract, how each maps to CSS and Tailwind, and which are not;
3. fixes the text and font rules that the geometry depends on;
4. describes the oracle test that enforces all of it in CI.

The layout core (Rust, shared by server and WASM) is the source of truth for geometry. The export is a translation of the
core's result into CSS. The oracle checks the translation against the core, using a real browser as the judge.

## 2. Definition of "faithful"

An export is faithful when, for the same document, in a Chromium-class browser with the shipped font loaded:

| Level | What must hold | Tolerance | How measured |
|---|---|---|---|
| L1 Geometry | For every node, the DOM border box (`getBoundingClientRect`: x, y, width, height) equals the core rect | **<= 0.05 px** (observed maximum 0.034 px); the hard fail line is 1 px | oracle A |
| L2 Text | Same line breaks (same box height), and glyph ink bounding box edges within **1 px** of the canvas text | 1 px | oracle C |
| L3 Paint (solid) | Solid areas of fills, borders (inside/center/outside), shadows (offset, spread, hard edge), image fit (cover/contain/fill) and clipping have the same geometry as the model semantics, and a fill with an alpha is the colour over what is behind it | exact colour at probe points away from edges (tolerance 40/255 per channel to absorb anti-aliasing) | oracle B |
| L3 Paint (soft) | Blurred shadow falloff | within 20/255 intensity of the canvas blur (measured today: 19/255) | oracle D (CSS model against Pixi, not through the export) |

The tolerances held at every viewport width and device pixel ratio tested (1280 px and 700 px viewport, DPR 1 and 2).
Document coordinates are in CSS px; the export has absolute roots, so viewport width does not move anything.

Explicitly **not** promised: anti-aliasing identity (glyph and edge pixels differ by a few intensity levels; the text
oracle measured up to 11/255 mean absolute difference inside text boxes), shader fills (approximated by a CSS gradient,
the export warns), fonts other than Inter Variable, responsive behaviour when the viewport changes, browsers other than
Chromium, and any document that violates a rule in section 5.

## 3. Assumptions

- Design-size output: roots are absolutely positioned at their `x,y`; nothing in the export is responsive.
- One font family, **Inter Variable**, shipped as woff2 next to the export. Canvas text measurement and the browser use
  the same file.
- The core's text measurements (width, height per wrap width) come from the same engine as the canvas (Pixi
  `CanvasTextMetrics` on the same font). The export never re-measures; it pins what the core measured (`line-height`,
  `min-width`).
- The export contains the rects' consequences, not the rects: boxes are produced by CSS flex layout and must agree with the
  core by construction, not by absolute positioning of every node. This keeps the output editable by developers.

## 4. Supported subset

Status legend: **supported** = emitted by the product export (`crates/features/export/export`, E4.1) and checked by the oracle of
`tools/fidelity-oracle` (E4.2: fixtures `f01` to `f23`, the paint probes, the ink cases); **partly** = emitted, checked, and a
known gap is listed in `tools/fidelity-oracle/known-gaps.mjs`; **planned (model)** = not in the model yet (concept.md W1 or
later); **unsupported** = deliberately outside the contract, with the reason. The notes of a row keep what the spike
experiment found; where they say "the spike emitted ..." that was the spike export, which the product export replaced.

"Tailwind" is the utility the product export emits. It uses arbitrary values everywhere (`p-[16px]`), which is exact but not
idiomatic; token-based classes arrive with decision 17.

### 4.1 Structure and geometry

| Model property | CSS | Tailwind | Status | Notes |
|---|---|---|---|---|
| node type frame / rect / ellipse | `div` | `div` | supported | ellipse adds `border-radius:50%` |
| node type text | `p` (margin 0) | `p` | supported | see section 6 |
| `x`, `y` on a root, or on a child of a frame without auto layout | `position:absolute; left; top` | `absolute left-[..] top-[..]` | supported | verified f04, f11. A frame with children and no layout, nested in flex, gets `position:relative` |
| `w`, `h` with mode `fixed` | `width`, `height`; main axis also `flex-shrink:0` | `w-[..] h-[..] shrink-0` | supported | |
| `sw`/`sh` = `hug` on a text node | `width/height:auto`, `min-width:<measured>px`, `line-height:<px>`, `white-space` | `min-w-[..] leading-[..] whitespace-..` | supported | the `min-width` pin makes the box equal to the core's ceil-ed width even when the DOM would be fractional |
| `sw`/`sh` = `hug` on a layout frame | width `fit-content`, height auto | `w-fit` | supported | hug on rect, ellipse or a frame without layout is treated as `fixed` (core does the same) |
| `sw`/`sh` = `fill`, parent is a layout frame, parent not hug on that axis | main axis `flex:1 1 0%; min-width:0` (or `flex:1 1 <core px>` when fill siblings have different padding); cross axis `align-self:stretch` | `flex-1 min-w-[0px]`, `self-stretch` | supported | rule G1 |
| `sw`/`sh` = `fill` where the parent is hug on that axis, or the parent has no layout | none: treated as `fixed` with the node's own `w`/`h` | `w-[..]` | **partly** | the core resolves such a fill to the node's stored `w`/`h` in a frame without layout. In a hug layout frame the core sizes it from what the parent hugs (an equal share among the fill children on the main axis, the whole size on the cross axis, as `align-self: stretch` does), so canvas and export can differ there until rule G1 is applied at write time; `get_tree` says which in the `fill_under_hug` finding. The spike export did this for the main axis only (with a warning) and emitted `stretch` for the cross axis, which failed (section 8.3). Rule G1: the validator normalises it to `fixed`. Product today: the export keeps the node's own size and reports `fill_under_hug`; f17 is a known gap until the write-time validator makes it `fixed`. |
| `layout` row / column | `display:flex; flex-direction` | `flex flex-row/col` | supported | |
| `gap` | `gap` (omitted when 0) | `gap-[..]` | supported | |
| `padding` (one value, all sides) | `padding` (omitted when 0) | `p-[..]` | supported | applies to layout frames only; core ignores padding on frames without layout |
| `align` start / center / end | `align-items: flex-start/center/flex-end` | `items-start/center/end` | supported | cross axis. Negative free space behaves the same in core and CSS |
| `justify` start / between | `justify-content: flex-start / space-between` | `justify-start / justify-between` | supported | |
| `justify` center / end | `justify-content: safe center / safe flex-end` | `[justify-content:safe_center]`, `[justify-content:safe_flex-end]` | supported | the spike emitted plain `center`/`flex-end`; the product emits `safe`. When children overflow the container, the core clamps to the start edge and plain CSS overflows on both sides (f13: 35 px for center, 70 px for end). With `safe` the error is 0.000 px |
| `clip` (frame) | `overflow:hidden` | `overflow-hidden` | supported | boxes verified (f04 has a clipping frame); the paint of the clip is probed only where it meets a radius or an ellipse (f08: the image is clipped at a rounded corner and by an ellipse) |
| `radius` | `border-radius:<px>` | `rounded-[..]` | supported | verified up to radius = half the height (probe). The canvas clamps to `min(radius, w/2, h/2)`; CSS clamps by its own rule, which gives the same shape for a single radius, but radius above half the size was **not** tested |
| `fill` colour (frame/rect/ellipse) | `background-color:#rrggbb`, or `#rrggbbaa` when the colour has an alpha below 1 (the stored `#rgba` and `#rrggbbaa` forms; an alpha of 1 is written `#rrggbb`, and a fully transparent fill is written, not omitted) | `bg-[color:#..]` | supported | the alpha is the colour's own, not the node's `opacity` (which fades the node and what is in it): a half-transparent red over white is pink, a fully transparent fill shows what is behind it. Probed on pixels in f23, both forms. `none` writes no `background-color` |
| `fill` `none` (frame/rect/ellipse) | no `background-color` | no `bg-[..]` | supported | nothing is painted for the node, so what is behind shows, through a frame, an ellipse ring and the letterbox of an image; probed on pixels in f22 (both forms). A node with no `fill` at all takes the default fill `#3f3f46`, probed in the same file |
| `fill` bound to a colour variable | `background-color:var(--collection-name)` | `bg-[color:var(--..)]` | supported | direct, alias, alias across collections, under a gradient and under an image, with the default mode (f20) and a picked mode (f21: `data-mode-<collection>` on `<html>`); a binding that does not resolve paints the default fill and the export warns `unresolved_binding` |
| `fill` on text | `color` (`#rrggbbaa` when the colour has an alpha below 1) | `text-[color:..]` | supported | the alpha is written; it is not probed on pixels |
| `fill` + `fill2` on shader | `background:linear-gradient(135deg, ..)` | `bg-[linear-gradient(..)]` | unsupported | GPU shader has no CSS equivalent; exported as an approximation with a warning. Not part of the fidelity promise |

### 4.2 Typography and decoration (model additions M3A, M3B)

| Model property | CSS | Tailwind | Status | Notes |
|---|---|---|---|---|
| `fontSize` | `font-size:<px>` | `text-[length:..]` | supported | fractional sizes verified (11.7, 13.5, 14.3 px: max 0.034 px) |
| `fontWeight` 100..900 | `font-weight:<n>` | `font-[<n>]` | supported | the spike hard-coded 400. Weight changes the glyphs (as-exported right ink edge 10 px short at 700, 17 px at 900) and the width; the text measure table must be keyed by weight (section 6). With the mapping, ink edges are within 1 px |
| `textAlign` left / center / right | `text-align` | `text-center`, `text-right` | supported | the spike ignored it (as-exported ink off by 83 px centered, 197 px right-aligned in a 300 px box). With the mapping: within 1 px, including wrapped and multi-line text |
| `border` `{width, color, position}`, uniform | `::after{position:absolute; inset:0 / -w/2 / -w; border:w solid color; border-radius; pointer-events:none}` on a `position:relative` node | no named utility (arbitrary `after:` variants) | supported | NOT a CSS `border`: a real border takes space out of the content box of a border-box element and would move children, while the canvas border is paint only (inside/center/outside all verified by probes, 12/12 for border, rounded corner included). Painting above children matches the canvas. The Tailwind form (arbitrary `after:` classes) is built with the Tailwind v4 compiler by the oracle and draws the same pixels as the CSS form (f07) |
| `shadows[]` (x, y, blur, spread, colour) | `box-shadow:` list **reversed** | `shadow-[..]` | supported | model order is first = bottom; CSS first = top. Probes: offset, spread, hard edge, order pass. Blur: close but not identical to the canvas filter (section 8.5) |
| `image` `{assetId, fit}` | `background-image:url(..); background-size: cover / contain / 100% 100%; background-position:center; background-repeat:no-repeat` over `background-color` | `bg-[url(..)] bg-cover / bg-contain / bg-[length:100%_100%] bg-center bg-no-repeat` | supported | cover, contain (letterbox shows the fill), fill, rounded rect and ellipse clipping: 14/14 probes. `assetId` is resolved to `assets/<id>` by the export (`asset_base`); the oracle serves a test image there. The Tailwind form is compiled and draws the same pixels as the CSS form (f08) |
| text `white-space` for hug / wrap / newline | `pre` (hug), `pre-wrap` (wrap) with `word-break:keep-all` | `whitespace-pre`, `whitespace-pre-wrap` | **partly** | f14 is a known gap (breaks at slashes, URLs, CJK and tabs, and a trailing newline); the spike emitted `nowrap` / `pre-line` / default, which collapse repeated spaces; the canvas keeps them (36 px vs 18 px height in f14) |
| text `line-height` for text with explicit `\n` | single-line height in px | `leading-[..]` | **partly** | checked with newlines in f05 and the ink cases (nlL, nlC, nlR); f14 (a trailing newline: Pixi keeps the empty last line, the browser does not) is a known gap. The spike used the height of the whole multi-line text as the per-line height: a 3-line text becomes 153 px instead of 51 px (+102 px) |

### 4.3 Not in the model today, and the contract position

| Property | Status | Position |
|---|---|---|
| opacity | in the model and drawn on the canvas (E3.2, ADR-0013); exported as `opacity` and probed on pixels (f18: a node and what is inside it, both forms) | maps to `opacity`; supported (f18). Note: opacity creates a stacking context, check overlapping children |
| gradients | in the model and on the canvas (E3.2: `gradient`, linear and radial; one gradient per node, over the fill and under the image) | `linear-gradient(<angle>deg, ..)` (no angle is 180: 0 points up, 90 right) and `radial-gradient(circle farthest-corner, ..)` as a layer over the fill and under the image; supported, probed on pixels in f19 (both forms): the angle at 0, 45, 90 and 180 (other angles are not probed), stop positions, a circle on a wide box, transparent stops over a fill, clipping by a radius and an ellipse, and the letterbox of an image. A stop colour cannot bind a variable yet |
| multiple fills | planned (model, W1) | `linear-gradient`/`radial-gradient` layers; needs probes for angle conventions |
| per-side padding | in the model and the layout (E3.2: `padT`, `padR`, `padB`, `padL`); exported and checked on boxes (f16) | `padding: t r b l`; the S9 spike docs report that 12 of 25 frames of a hand-made shop page fell back to free layout when captured, mostly 9 buttons with `padding: 12px 20px` |
| absolute child inside an auto layout frame | in the model (`absolute`); exported and checked on boxes (f16) | `position:absolute` child of a `position:relative` flex container; removes the child from flow, as `ignoreAutoLayout` in the model would |
| margin | in the model and the layout (E3.2: `marT`, `marR`, `marB`, `marL`); exported and checked on boxes (f16) | the spike capture turns margins into gap/padding; per-child margin only if round trip is required |
| variables as CSS custom properties | exported (E3.3 and E4.1: `var(--collection-name)`; the `fill`, `radius` and `opacity` fields bind); supported, probed on pixels in f20 and f21: a bound fill (above), a bound radius as `calc(var(--..) * 1px)` and a bound opacity, in the default mode and a picked one | `var(--token)`; the oracle resolves variables in the browser, so geometry tests are unchanged |
| per-corner radius, rotation, scale, skew | unsupported until proposed | rotation changes `getBoundingClientRect`; the oracle would compare untransformed layout boxes |
| flex-wrap, CSS grid, min/max size, aspect ratio, space-around, baseline alignment, sticky/fixed | unsupported until proposed | each needs a core implementation and its own oracle fixtures first |
| letter-spacing, text decoration, italic, `lineHeight` field, `fontFamily`, text runs | unsupported until proposed | font metrics rules in section 6 would have to be extended |
| inner shadows, per-side border, dashed border | unsupported | deferred in M3A |
| text with CJK, long slash/URL tokens, tab characters | unsupported | Pixi and CSS choose different break points and tab widths (section 8.3, pattern 4); text in these scripts must not be promised |
| content that overflows its auto layout parent, fills that resolve to zero, boxes smaller than their own padding | unspecified | section 5, rule G3 |

## 5. Geometry rules

- **G1 Fill under hug.** `fill` on an axis where the parent is `hug` (or where the parent is not a layout frame) is stored as
  is but exported, and validated, as `fixed`. Reason: the core uses the node's own `w`/`h`; CSS resolves stretch and flex
  against the parent's max-content. Measured: 8 of 105 fuzz documents that otherwise fit failed only because of this (26
  nodes); with rule G1 applied, 0 of 399 nodes fail (section 8.2).
- **G2 Safe alignment.** `justify: center` and `end` are exported as `safe center` / `safe flex-end`.
- **G3 No overflow.** The validator flags a layout frame whose children (plus gaps and padding) exceed its content box, a
  `fill` that resolves to less than 1 px, and a box smaller than its own padding. In that regime the core can produce
  geometry that CSS cannot (a flex item cannot be smaller than its padding; wrapped text at zero width breaks differently).
  Behaviour there is **unspecified**: the oracle does not gate it, the export should warn. Today 161 of 1065 nodes in
  overflow/degenerate fuzz documents still differ after rules G1 and G2.
- **G4 Pinning.** Text boxes are pinned by the core's measurements (`min-width`, `line-height`), never left to the browser.
- **G5 Rounding.** No rounding in the export; both sides keep fractional px. The browser snaps layout to 1/64 px, which
  explains every non-zero delta seen (0.005 to 0.034 px, e.g. three fill siblings sharing 333.33 px).

## 6. Font and text rules

1. One family, Inter Variable (`font-weight: 100 900`, `font-display: block`), shipped as woff2 with every export. The
   metrics are those of that file. Documents must not reference other families until a font registry exists (decision 19
   open area).
2. `line-height` is always an explicit px value taken from a measured **single line** (never `normal`). The spike's value
   comes from measuring the whole text, which is wrong for text with `\n` (section 8.3, pattern 1).
3. Hug text: `white-space:pre` and `min-width:<ceil(measured width)>`. The core uses `ceil` of the canvas measurement; the
   DOM width is fractional; without the pin the error accumulates (S9 spike: 1.9 px over three footer columns).
4. Wrapping text: `white-space:pre-wrap; word-break:keep-all`. Canvas text (Pixi `whiteSpace:'pre'` default) preserves
   repeated and trailing spaces; CSS `normal` collapses them. Explicit `\n` is a line break in both.
5. The measure table / cache is keyed by `(text, fontSize, wrapWidth, fontWeight)`. The spike's `TableMeasurer` has no weight
   in the key and the export CLI cannot see the weight at all. Wrap width should be quantised (for example to 1/64 px):
   in 2 of 200 fuzz documents the Rust core asked for a wrap width that differed in the last bit from the one the table
   recorded (119.33333333333333 vs the TypeScript value) and the lookup failed.
   The browser host's measurer (`packages/web-link/src/measurer.ts`) keys its cache on that grid and measures at the gridded
   width, so a last-bit difference is one query. The grid rounds to the nearest 1/64 px, so a line that is within 1/128 px
   of the width of its box can be measured as fitting and drawn wrapped (the oracle's `TableMeasurer` has the same edge).
6. Glyphs missing from Inter fall back to system fonts, which differ per machine. Text relying on fallback fonts is outside
   the contract. (In the experiment the CJK sample matched because both renderers ran on the same machine, which says
   nothing about other machines.)
7. Break opportunities: space and explicit newline only are guaranteed. Hyphens match (tested), slash tokens, URLs, CJK and
   tabs do not (section 8.3, pattern 4).

## 7. Running the oracle in CI

The oracle is `tools/fidelity-oracle/` (README there), run by the `oracle` job of `ci.yml` on every pull request and by
`fidelity-oracle` in `nightly.yml`. The `oracle` job is not a required context yet; the owner decides when it becomes one
(E4.2 is ticked on the owner's order of 21 Nov 2026; the exit line 'required on `develop`' of M4 stays open).

The gate is verified on Linux only. On macOS the same code gives text boxes that are whole pixels off the core (issue 231, cause not found), so the oracle stops there with a note unless `MELDFRAME_ORACLE_ANY_PLATFORM=1` (the README of the oracle has the details; a text failure on a Mac cannot be told from that issue); the `oracle` job of a pull request is the check for a contributor on a Mac.

How it works:

1. `oracle-runner` (`crates/core/meldframe-oracle-runner`, kind tool) reads fixture files as JSON, lays the file out with
   the layout core (`compute_rects`) and calls `export_frame` for each top-level frame, in the CSS form and the Tailwind
   form. It prints the core rects and the exports. The geometry the browser is compared with is the core's, computed
   separately from the export's own layout call.
2. Text: the core asks a measurer for every text. The runner replays a table recorded in Chromium with Pixi
   `CanvasTextMetrics` and Inter (`ExportOptions::measurer` is that table), keyed by text, size, weight and wrap width on a
   1/64 px grid. A query the table lacks is answered by the stub measurer and returned as a miss; the harness measures it
   in the browser and runs again, until no query is missing (2 or 3 runs).
3. Tailwind: the v4 compiler (`css` of the export is the input, the classes of the HTML are the candidates).
4. Each page is loaded as delivered (HTML page, stylesheet, the font and the test image served where the page asks for them)
   in Chromium at 1280 px DPR 1, 700 px DPR 1 and 1280 px DPR 2 (fuzz files at 1280 px DPR 1), and every `[data-node]`
   box is read.
5. Gates (exit code 1 on any violation, `tools/fidelity-oracle/gate.mjs`):
   - every fixture outside `known-gaps.mjs`: every box within 0.05 px of the core rect, in both forms and all settings,
     no node missing, the font loaded;
   - the Tailwind page equals the CSS page: boxes within 0.001 px, and the screenshots of the paint fixtures identical;
   - fuzz files (rule G1 applied to them) that do not fall under rule G3: every box within 0.05 px; files under G3 are
     counted and not gated; a failing seed prints its file, to be added as a fixture;
   - paint probes (border inside, center and outside, shadow offset, spread and order, image cover, contain and fill,
     clipping, opacity, gradients, fills bound to a variable in two modes with a bound radius and opacity, no fill, the alpha of a fill's colour), text ink edges within 1 px, and the CSS blur model within 20/255 of the Pixi blur (that last check compares a hand-written CSS page with Pixi and does not go through the export: it says how close the blur the export writes would be, not that the export wrote it; the shadow probes through the export cover offset, spread and order, not blur);
   - a fixture in `known-gaps.mjs` that starts passing fails the job, as does an entry that is no longer a fixture: the entry
     is removed and this contract updated.
6. Pull requests run the fixtures and 50 fuzz seeds, the nightly 500. Playwright 1.63.0, Pixi 8.23.0, Tailwind 4.3.5 and
   Inter 5.3.0 are exact in `package.json` and the lockfile; a browser upgrade is its own change that must pass the gates.

Known gaps today: `f14-text-edge` (break opportunities outside section 6.7) and `f17-fill-under-hug` (rule G1).

Measured on the development machine (software rendering): the fixtures and 50 seeds take about 10 to 17 s in the browser,
500 seeds about 40 s, plus the build of the runner and `npm ci`; the CI time is measured on the first runs of the job and is not known yet.

What the oracle cannot prove (the list for the product oracle; the experiment's own is section 12):

- **The "canvas" side is the layout core, not the editor.** The export and the layout findings call the layout core
  (`compute_rects`), and since 21 Nov 2026 so does the editor by default: the app layer holds the flag `layout_on_canvas` and the
  page turns it on ("Auto layout on canvas", on by default, remembered per browser, `/?canvaslayout=0` forces it off), so
  the canvas draws the core's boxes (`resolve_laid_out` with the measurer the host gives; the stub where a host gives none)
  instead of the stored `x`, `y`, `w` and `h`. The owner accepted the one-time visual change for files that have layout set. With the
  flag off the canvas paints the stored boxes as before, and the oracle's claim does not reach it. The browser host injects the
  measurer of this contract (Pixi `CanvasTextMetrics` on Inter, keyed and rounded as section 6 says, `Engine.setMeasurer`), so the
  canvas lays text out as the oracle's table does, from a live measurement instead of a recorded one;
  `packages/web-e2e/scripts/measurer-e2e.mjs` checks in Chromium that a hug text box is Pixi's width within 1 px and is not
  wrapped by Pixi when drawn, and `layout-interaction-e2e.mjs` that the interaction rules hold in a real browser. The oracle
  does not run that path: it draws its own text and boxes (a Pixi page that follows this contract, not `stage.ts`), so what it
  shows is that the export reproduces the core's layout in a browser; that the canvas draws the core's boxes with
  Pixi's measurer is the product's own claim, checked by those two scripts and not by the oracle. The Export dialog
  of the browser measures with the measurer the host gave the app (the same Pixi measurer, so a hug text exports the width the
  canvas draws; the stub when the host gave none), and an open dialog is built again when the host gives another. The MCP
  export tool runs on the server, which has no fonts, and stays on the stub; its result warns `text_size_estimated` (one warning, naming the texts whose box the stub decides: a hug width or a height that is not fixed) until a real measurer exists there. An absent warning does not make a text exact: the `line-height` of every text is the stub's. Whether the boxes stored in a file equal the
  core's layout is not checked by the oracle.
- **The text table is Chromium's measurer by assumption.** The core and the export agree on text because both use the one
  measurer they are given: the export pins `min-width` and `line-height` from it and the core sizes the boxes with it.
  Here that is a table recorded with Pixi in Chromium, so the boxes are right when the browser breaks lines as Pixi
  does (section 6). The MCP export tool uses `StubMeasurer` (half an em per character, breaks at spaces only) because the
  server has no browser (the Export dialog of the browser does not: it uses the app's injected measurer): its pinned widths and line heights are the stub's, and a browser that wraps real text
  differently gives other heights. The oracle does not measure that distance. The tool says so in its result: it sets `ExportOptions::estimated_text`, so a text whose box the stub decides is named in a `text_size_estimated` warning (a text of fixed size is not, and its line height is still the stub's), and the Export dialog, which measures with the page's fonts, does not set it.
- **Text ink compares against a Pixi page that follows the contract, not the app.** The oracle draws its own Pixi text; the
  product canvas (`stage.ts`) draws weight and alignment too (coverage table: rendered) but the
  oracle does not read the canvas, so these cases test the export against the contract's canvas.
- **One browser, one machine class.** Chromium 153 headless with software rendering, Inter latin only, three viewport and
  DPR settings. No other browser or OS. Glyphs outside Inter fall back to system fonts.
- **The fuzz generator** produces auto layout, hug, fill, fixed, text, rect and ellipse; not borders, shadows, images,
  weights, alignment, margins, variables or gradients. Those are covered by hand-written fixtures and probes only, so
  their interactions are not.
- **Rule G3 files are not gated**, and the export's own `overflow` and `smaller_than_padding` warnings skip every frame
  that holds a hug text: of 500 seeds, 207 are under G3 and for 89 of them the export gave no such warning.
- **Pixel probes check points, not whole images.** Shaders have no probe (the contract does not promise them). A gradient stop and a text colour cannot bind a variable yet, and no probe reads a shadow or a border colour bound to one.

## 8. Experiment results

Environment: Chromium 153.0.8010.12 (Playwright 1.63.0, headless, Linux, software rendering), Node 22, Pixi 8.23.0,
Tailwind 4.3.5, Inter Variable woff2 from the spike (`tools/capture/assets/inter.woff2`, sha256 `3100e775...`), spike
commit `ca74052`, export CLI built in release mode into a private target dir.

"Core rect" = reference layout from the spike's TypeScript layout (`src/lib/layout.js`) with a weight-aware Pixi
`CanvasTextMetrics` measurer, which the spike proves equal to the WASM core. I also ran the Rust core (via
`spike-export --format rects`, table recorded from the same measurer): Rust equals the reference to 0 on all 15 fixtures
and on 198 of 200 fuzz documents (the other 2 hit the table-miss in section 6, item 5).

### 8.1 Oracle A on hand-written fixtures (as exported by the spike, 274 nodes)

Max |delta| over x, y, w, h of the DOM box versus the core rect. The three columns are viewport 1280 DPR 1, viewport 700
DPR 1, viewport 1280 DPR 2; the results were identical across them, so one number is shown. Tailwind output equals plain
CSS output to 0.000 px in every fixture (rects compared; the S9 spike also compared computed styles).

| Fixture | Nodes (text) | Max delta px as exported | Nodes > 1 px | With candidate fixes |
|---|---|---|---|---|
| f01 row, fixed, padding, gap, align center | 5 (0) | 0.000 | 0 | 0.000 |
| f02 column hug/hug, nested hug row | 7 (4) | 0.000 | 0 | 0.000 |
| f03 fill on both axes, unequal-padding fill sibling | 13 (0) | 0.010 | 0 | 0.010 |
| f04 justify x4, column align end, free frame, clip | 22 (0) | 0.000 | 0 | 0.000 |
| f05 wrapping text, newline, long word, narrow column | 9 (7) | **102.000** | 3 | 0.000 |
| f06 realistic card | 11 (5) | 0.000 | 0 | 0.000 |
| f07 border / shadow / radius (properties not exported) | 12 (0) | 0.000 | 0 | 0.000 |
| f08 image fills (not exported) | 5 (0) | 0.000 | 0 | 0.000 |
| f09 weights and text-align (not exported) | 11 (6) | 0.000 | 0 | 0.000 |
| f10 three fill cards, hug row | 13 (6) | 0.010 | 0 | 0.010 |
| f11 spike landing page (unchanged) | 84 (44) | 0.000 | 0 | 0.000 |
| f12 fractional sizes, gaps, fonts | 15 (3) | 0.034 | 0 | 0.034 |
| f13 overflow, justify center/end/between, fill with no space | 22 (0) | **70.000** | 4 | 0.000 |
| f14 text edge cases | 22 (20) | **72.000** | 11 | **36.000** (11 nodes) |
| f15 font sizes 8 to 96 px | 23 (21) | 0.000 | 0 | 0.000 |

Reading: 12 of 15 fixtures, including the 84-node landing page, match to <= 0.034 px as exported. f07, f08 and f09 match
only because the spike export ignores the new properties (they do not change boxes); their paint is covered by oracles B and C.
f05, f13 and f14 fail for the reasons in 8.3. "Candidate fixes" = rules from section 6 and G2 applied as post-processing in the
harness (`--safe --ws`).

Rounding: every non-zero delta in a passing fixture is 0.005 to 0.034 px, the browser's 1/64 px layout unit. Neither the
viewport nor DPR 2 changed any value.

### 8.2 Oracle A on seeded fuzz documents (200 random trees, 1476 nodes)

Random auto-layout trees (depth up to 4, 1 to 4 children, row/column, fixed/hug/fill, random alignments, integer and half
pixel paddings, random text from a 23-word list at 9 sizes, rectangles and ellipses; 2 to 29 nodes each). Documents for which the Rust
table lookup failed (2 in the raw variant, 1 in the others) are excluded from the rows (section 6, item 5). "Fit" = every child lies inside its parent's content box and no fill is
below 1 px, measured on the core rects.

| Variant | Fit documents | Fit nodes > 0.5 px | Non-fit documents | Non-fit nodes > 0.5 px |
|---|---|---|---|---|
| raw export | 97 / 105 pass | 26 / 409 | 23 / 93 pass | 524 / 1039 |
| + G1 (fill under hug becomes fixed) | **104 / 104** pass | **0 / 399** | 33 / 95 pass | 311 / 1065 |
| + G1 + safe alignment | 104 / 104 | 0 / 399 | 50 / 95 pass | 161 / 1065 |

Residual failures after G1 and G2, by topmost failing node (95 non-fit documents, 45 failing): box smaller than its own
padding (29), text `fill` at zero width (10), other cases (6: 1 overflow, 3 with an auto layout parent, 2 with a hug-height root; one
root traced by hand to a zero-width text child). These are the unspecified regime of rule G3.

### 8.3 Failure patterns, measured

1. **Multi-line text line-height** (f05: p4, p5): the export uses the height of the whole text as `line-height`: 51 px for a
   3-line text, so the DOM paints 3 x 51 = 153 px. +102 px on the text and everything after it. Fixed by the single-line
   height: 0.000 px, and text ink 0 px off (oracle C, rows nlL/nlC/nlR; as exported: 20 px vertical, 70 px horizontal).
2. **Fill under hug** (fuzz): width 242 in the core against 593 px in the DOM for a hug column whose children include fill
   items (+351 px, one fuzz document), because CSS hug sizes from the children's max-content (a text's unwrapped width) and the core uses the stored `w`. Fixed by G1.
3. **Overflowing justify** (f13): center 35 px, end 70 px. Fixed by `safe`.
4. **Whitespace and break opportunities** (f14, 20 text nodes): repeated spaces and trailing spaces collapse in CSS
   (fixed by `pre-wrap`). Not fixed by the harness: a path/URL string (core 3 lines, DOM 4), CJK text (core keeps it on one
   line, `keep-all` still breaks on punctuation: 2 lines), a tab (core width 139 px, DOM 168.9 px).
5. **Box below padding / zero fill** (fuzz): the core produces a 0 px wide frame inside padding 8; a CSS box cannot be
   smaller than its padding (core 0, DOM 16 to 25 px). Wrapped text at zero width breaks per word in CSS and differently in Pixi.
6. **Weight and alignment ignored** (oracle C, as exported): right edge 10 px short at weight 700, 17 px at 900; centered
   text 83 px left of where the canvas draws it, right-aligned text 197 px.
7. **Native measurement path** (Rust CLI with recorded table): 2 of 200 documents failed with "missing text query" because
   a fractional wrap width (119.33333333333333) differed from the recorded one in the last bit. The WASM path (calls
   JS per batch) is not affected, a server-side core with a table or a native measurer would be.
8. **No rounding or viewport effect**: 1/64 px layout snapping only; DPR 2 and a 700 px viewport changed nothing.

### 8.4 Text ink, Pixi as `Canvas.svelte` draws it vs the exported DOM (oracle C, 17 cases, DPR 1)

Pixi `Text` created with the same style, wrap width and slack-based alignment as `src/lib/Canvas.svelte` (read from the
code, not the running app), drawn with WebGL at resolution 1, against the exported HTML plus the candidate mappings. Ink
bounding box edge differences DOM minus Pixi (left, right, top, bottom), px:

- hug 400 / 13 / 9 px, wrapped left: 0 / 0 / 0 / 0, intensity difference 0.
- hug 700, 900: right edge -1; hug 600 at 24 px and hug 40 px: top and bottom -1; wrapped centered (400 and 700): top -1;
  one-line center: right -1; one-line right: left +1.
- explicit-newline texts (left/center/right) after the line-height fix: 0 to 1 px; CJK sample 0.
- Every edge delta is at most 1 px; mean absolute pixel difference inside the text box 0 to 11 out of 255 (glyph
  rasterisation and sub-pixel placement differ between the canvas text and the DOM).

### 8.5 Paint probes (oracles B and D)

- **B (35 probes, DOM against model semantics from the M3A/M3B docs):** 35 / 35 pass. Border inside/center/outside stripes
  at hand-computed columns, never outside the box for `inside`, rounded corner clipped; hard shadow, spread, absent on the
  opposite side, list order; image cover/contain/fill, rounded and elliptical clipping.
- **D (blur, Pixi `BlurFilter` built as `decorate()` in `Canvas.svelte` does, vs CSS `box-shadow`, same geometry):** max
  intensity difference outside the box 19/255 (blur 8 px), 19/255 (blur 20 px), 15/255 (blur 40 px). The CSS shadow is
  lighter in the tail (falls off faster) at all three blur sizes; at 40 px blur the CSS shadow is darker right at the edge (1 px away: Pixi 146, CSS 134).
  The CSS result follows a Gaussian with sigma = blur/2 to within 7/255 (35/35 includes this probe). Conclusion: blur is
  close but not pixel-identical; it is in the L3 soft tolerance.

### 8.6 Speed (rough wall-clock, one machine; not a benchmark)

| Nodes | Rust core via CLI, wall ms (minus 3.3 ms process baseline) | Browser: load exported page | Browser: full relayout + read every box |
|---|---|---|---|
| 1002 | 6.3 (3.0) | 46 ms | 1.2 ms |
| 5006 | 20.3 (17.0) | 154 ms | 5.9 ms |
| 20008 | 84.4 (81.2) | 418 ms | 22.9 ms |

The CLI figure includes JSON parse and serialisation of the whole document and is therefore not the cost of the layout
itself. From the spike docs (M3A report, 5025 nodes, WASM, mean of 3 runs): move 1.6 ms, text 3.1 ms, gap 1.6 ms. Neither of
these two series was measured under the same harness, so they should not be compared to the decimal; both say that layout
speed is not what separates the approaches.

## 9. Comparing the approaches

(a) **Contract subset + oracle** (core stays the source of truth). (b) **Browser CSS as the source of truth**: delete the
core's layout, drive the canvas from boxes read from a DOM built from the document. (c) **Hybrid**: core is the truth, the
browser is a judge and an optional runtime verifier.

| Criterion | (a) contract + oracle | (b) DOM as truth | (c) hybrid |
|---|---|---|---|
| Measured accuracy | 0.034 px on 12 of 15 fixtures; 0 of 399 nodes off in fuzz documents that satisfy the rules; known failure classes listed and named | exact for the export by construction (it is the export). The canvas inherits whatever the browser computes: no measurement exists for canvas-vs-DOM drift between machines | same as (a); plus a runtime check that reports drift |
| Portability off the browser (server, WASM, headless agents) | full: the Rust core runs anywhere. Open point: a deterministic text measurer for non-browser targets (section 6, item 5) | none: every consumer of geometry (agent tools that return rects, validators, capture, server-side checks) needs a browser. Decision 18 would be forced to "headless Chromium" | full, browser only for verification |
| Speed on large frames | core 1.6 to 3 ms per edit at 5k nodes (spike docs); no round trip | DOM relayout and read 5.9 ms at 5k, 22.9 ms at 20k here; plus an async measure loop, DOM build 154 ms at 5k, DOM nodes for every design node kept in sync | as (a) at edit time |
| Complexity | two implementations of fill/hug/align (core and CSS) kept in step by the oracle; the rules in section 5 are the cost | one implementation, but the model must be exactly what CSS can express (no core-only semantics such as fill under hug, zero fills, clamped justify), the canvas needs an off-screen DOM, hit testing and collab sync need rects after every change | (a) plus a verifier |
| Text and font risk | measurement is by the same engine as the canvas; export pins the results. Risk: a different OS or font build between measuring and viewing | text is measured by the DOM, the canvas text then differs from DOM text unless it is also drawn from DOM line boxes; Pixi `CanvasTextMetrics` and the DOM can disagree on breaks (8.3 pattern 4), so (b) changes the canvas' own text wrapping | as (a) |
| Determinism for agents | the same document gives the same rects in server and client | depends on browser version and fonts; two collaborators can lay out differently | as (a) |

Why not (b): in this experiment the two engines agreed to 0.034 px wherever the subset was respected, so there is no
accuracy to buy by deleting the core, and the cost is that every non-browser consumer of geometry disappears, which
conflicts with the concept (Rust core on server and in WASM, agent surface with structured errors, headless capture still
undecided). (b) is attractive only if the model is allowed to shrink to exactly what CSS flex can express, rules G1 and G3
move towards that without giving up the core.

## 10. Recommendation

Adopt **(c)**: approach (a) as the contract, with the oracle as a mandatory CI gate and, later, an optional
`verify export` step for agents when a headless browser is available (decision 18).

1. Make the core's geometry the single source of truth and treat the export as a pure translation of it.
2. Implement in the real export: the planned rows of section 4 (weight, text align, border pseudo-element, shadows, image,
   `pre`/`pre-wrap`, single-line `line-height`, `safe` alignment), rules G1 and G2, and a weight-aware, quantised
   measure key.
3. Put the validator behind rule G1 and G3 (normalise fill under hug on write; warn about overflow and zero fills) instead
   of trying to make CSS reproduce the core's behaviour in those regimes.
4. Move this harness into the product repository against the Meldframe export, with the gates in section 7. Each new model
   property enters the supported table only together with a fixture, a probe and, for geometry, a place in the fuzz generator.
5. Decide (open questions 1 to 3) whether the core should instead change to match CSS in the degenerate regimes.

What would change this: if a deterministic non-browser text measurer proves impossible and no consumer needs geometry
outside a browser, the cost side of (b) shrinks. Nothing measured here argues for (b) before that.

## 11. Open questions for the owner

1. Tolerance: is 0.05 px (gate) / 1 px (fail line) for geometry and 1 px for text ink acceptable, with anti-aliasing
   identity explicitly not promised?
2. May CI and, optionally, runtime verification depend on headless Chromium (Playwright)? This interacts with decision 18.
3. Degenerate regimes: should the validator forbid overflow, zero fills and fill-under-hug (rules G1, G3), or should the
   core change to match CSS (floor boxes at their padding, no clamping of `justify`)? The contract currently picks the validator.
4. Is text outside Latin Inter (CJK, URLs without spaces, tabs) in scope? If yes, the core needs the browser's line breaking
   or its own implementation of it.
5. Fonts: after Inter, how are other families registered? Every font needs its own metrics file and `@font-face`.
6. Border output: pseudo-element `::after` (what was verified) or an extra child element? The first keeps the DOM equal to the
   node tree but is awkward in Tailwind.
7. How is `image.assetId` exposed in an export: relative file next to the HTML, data URI, or a hosted URL?
8. Shadow blur: adjust the canvas calibration to the CSS curve, or accept up to 19/255?
9. Must the Tailwind output be idiomatic (`p-4`, token classes) or is exact arbitrary-value output enough? Idiomatic output
   needs a token mapping (decision 17) and makes the oracle compare resolved values, not class names.
10. Responsive output (breakpoints, variants, decision 19) is outside this contract. Is it a later separate contract?

## 12. What this experiment does not prove

- **Only the spike's export was tested** (commit `ca74052`), not a Meldframe export, which does not exist yet. The mappings
  marked planned were applied as post-processing on its HTML by my own harness (`overlay.mjs`), as plain CSS. They were not
  compiled through Tailwind (border pseudo-element, image utilities) and are proposals, not shipped code.
- **The "canvas" is not the running app.** Core rects come from the TypeScript layout with a Pixi measurer in my page; the
  Rust core (through the table) agreed to 0. I did not run the app's WASM core with its JS measurer and did not screenshot
  the real canvas. Pixi text and blur were drawn by a harness that copies the logic of `Canvas.svelte` from reading the
  code. Canvas rendering of borders and images was **not** compared; those probes check the DOM against the model
  semantics written in the M3A and M3B reports, not against canvas pixels.
- **One browser, one machine**: Chromium 153 headless on Linux with software rendering, one build of Inter. No Firefox,
  Safari, macOS or Windows. Text measured on one OS and viewed on another may differ in sub-pixel widths and glyph
  rasterisation; the `min-width` pin protects boxes, not ink. Viewports 1280 and 700 px, DPR 1 and 2 only.
- **Sample sizes are modest.** 15 fixtures (274 nodes), 200 fuzz documents (1476 nodes), 104 fit documents (399 nodes) for the
  headline "0 failures"; 0 of 399 is not a proof, and the confidence bound on a failure rate from 399 clean nodes is still
  about 0.75% (rule of three) when treating nodes as independent, which they are not. The fuzz generator does not produce
  images, borders, shadows, weights, text alignment, fractional font sizes, depth above 4, or documents above 29 nodes.
- **The fit/non-fit split is mine.** It is computed from core rects, and rule G1 changes which documents count as fit, so
  the rows of the fuzz table do not describe the same documents. The G3 definition may be too strict or too lax.
- **Hand-written fixtures were written by someone who had read the export.** f11 (landing) was authored by the spike's
  authors against the same export. The fuzz documents are the least biased evidence; text-heavy edge cases (f14) are the
  most adverse and are expected to fail.
- **Speed numbers are wall-clock from single runs on a shared machine**, with an unequal harness (the CLI includes JSON IO). They
  say that DOM layout is not slow, nothing more. No memory, no incremental relayout, no canvas drawing cost were measured.
- **Tailwind equivalence** was checked on rects for 15 fixtures and 200 fuzz documents, not on computed styles, and only
  for the classes the spike emits.
- **Text ink** used 17 cases; the pixel differences up to 11/255 were not investigated further.
- The residual classification in 8.2 is by the topmost failing node only; one "other" document was traced by hand to a
  zero-width text and the rest of the residual was not examined individually.

## 13. Alignment with architecture decisions (13 Nov 2026)

Decisions made after this draft (`docs/decisions.md` items 20 to 24; ADR-0003, ADR-0007, ADR-0010):

- **Where the oracle lives.** The oracle is the CI test of the `export` feature module (E4.2): `tools/fidelity-oracle/` (Node harness) and `crates/core/meldframe-oracle-runner` (kind tool). `experiments/fidelity-oracle/` stays as the history of the spike run and is not run.
- **Export is a feature module.** It registers its agent tool (for example an export tool, Read tier) in the module registry and can be left out of a build (ADR-0007). The geometry source of truth stays in the core (ADR-0003).
- **Headless Chromium.** CI may depend on it (open question 2 stays with the owner). Runtime verification by a headless browser is part of decision 18, which is decided in the repository when a real agent workflow needs it; no separate spike.
- **Generated results.** The per-node oracle output is not committed (`tools/fidelity-oracle/out/`, git-ignored). The numbers quoted in section 8 are the recorded results of the spike run on 13 Nov 2026.

