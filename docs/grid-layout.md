# Grid layout: plan for an epic box

Status: **plan, awaiting the owner**. Written 2026-11-24 for GitHub issue 271 (decision 40, item 1, `docs/decisions.md`; the owner approved decision 40 on 24 Nov 2026 and #271 is its follow-up for grid), after reading the repository; the state of `develop` read for the order of work in 11.1 is the commit that records decision 41 (the scoped HTML importer, epic E4.4). Nothing in the model, the layout core, the canvas or the export changes with this note, and nothing here is built. The owner approves it by merging the pull request that carries it, or answers differently; the epic box it proposes (section 12) goes into `docs/epics.md` in a pull request of its own, after the other edits to that file.

What was checked, and how, is in section 14. Short version: the claims about the code come from reading it (paths are cited). The CSS grid behaviour that the plan relies on was measured on 2026-11-24 in Chrome for Testing 153.0.8010.12, the same Chromium major the fidelity contract measures, with a throwaway script that is **not** in the repository (section 14 says what it did, so it can be written again). The Tailwind classes of section 7 were compiled with Tailwind 4.3.5, the version `tools/fidelity-oracle/package.json` pins. No number below is a measurement of Meldframe itself; sizes of slices are estimates by analogy to the earlier model slices (#220, #221, #222).

## 1. Purpose and non-goals

Meldframe has two flows, row and column, and a free frame. Real pages are often laid out with CSS grid: a card grid, a form with labels and fields in two columns, a dashboard with a wide tile beside two narrow ones. Today an agent or a person has to build these as rows inside columns, which loses the shape (a child cannot span two columns) and exports as nested flex. This note plans a **grid**: a frame whose children sit in the cells of explicit columns and rows.

What this note is not:

- **Not a model of all of CSS grid.** Section 3 lists what is in and what stays out, with the reason for each.
- **Not a change of decision 40.** Grid stays out of the W1 Properties panel (`docs/ui-design.md` section 3.1a); this plans the epic that decision 40 asks for after W1.
- **No change to the fidelity promise.** The core stays the source of truth for geometry (ADR-0003) and the export a translation of it. A grid row of the contract becomes **supported** only with a fixture and an oracle run that passes (contract section 10, item 4).

## 2. Why there is no grid today

The reasons are in the code, not in a decision against grid:

- **The model has no place for it.** `LayoutMode` is `none`, `row` or `column` (`crates/core/meldframe-protocol/src/enums.rs`). A node has `gap`, `padding` (and `padT` to `padL`), `align`, `justify`, `sw`, `sh` and margins, but no list of columns or rows and no place for a child in a cell (`crates/core/meldframe-protocol/src/node.rs`). The 16 inputs of layout in the coverage table (`FROM_LAYOUT` in `crates/core/meldframe-render/src/coverage.rs`, asserted as 16 in `crates/core/meldframe-render/tests/coverage.rs`) are all about one axis at a time.
- **The layout core is two-valued.** What each site does with a third value today (read from the code): `tree.rs` (`is_layout_frame`) says a frame lays out when its layout is not `none`, so a grid frame would lay out; `place.rs` and `hug_width` in `size.rs` read `n.layout == LayoutMode::Row` and treat every other value as a column; `hug_height` in `size.rs` tests `== LayoutMode::Column` and lets everything else fall into its row branch, so a grid would be sized as a row for height and as a column for width; `decl.rs` writes `flex-direction` as `row` for a row and `column` for anything else; in the app layer `drop.rs` (`index_at`, `slot_line`) takes a row as horizontal and anything else as vertical, while `arrow.rs` matches only (Column, Up or Down) and (Row, Left or Right) and ends in `_ => return Vec::new()`, so a grid would simply get no move. So a new variant would compile and be laid out inconsistently, silently, apart from the arrow keys, which would do nothing. Section 5.7 lists every such place; they are why the first slice refuses `layout: grid` until the core can lay it out, and why each site needs an explicit arm (section 11).
- **The contract has no row for it.** `docs/fidelity-contract.md` section 4.3 lists "flex-wrap, CSS grid, min/max size, ..." as "unsupported until proposed: each needs a core implementation and its own oracle fixtures first". `docs/import-html.md` section 3.4 therefore refuses `display: grid` and turns the frame into a column.
- **The panel left it out on purpose.** `docs/ui-design.md` section 3.1 has the Layout direction as free, row or column; 3.1a lists "Grid layout (flow "grid", columns, rows)" as not in W1, and decision 40 says it needs its own epic box and "a model round like ADR-0013".

## 3. The subset to model first

The question for each CSS grid feature is the same as for the earlier layout rows: can the core compute the same box as the browser, with arithmetic a person can read, and can the model hold it without a second way to say the same thing.

| Feature | In or out | Reason |
|---|---|---|
| `display: grid` as a layout mode | **in** | `docs/ui-design.md` 3.1a lists it beside free, row and column as the missing fourth direction (the first design pass had a Flow control of "free, row, column, grid", before #264 replaced it with the panel as built), and the reference panel described in `docs/tokens-and-variables.md` (line 313) has four modes too. |
| Explicit columns and explicit rows as lists of tracks | **in** | The one thing a flow cannot say. |
| Fixed track (a size in px) | **in** | Exact in the core and in CSS. |
| Fill track (a share of the free space, `fr`) | **in**, as `minmax(0, <w>fr)` | A bare `1fr` is `minmax(auto, 1fr)`: a track cannot shrink below the min-content of what is in it. Measured: two `1fr` columns with a long unbreakable text in the first came out 597 px and 0 px wide in a 300 px frame; with `minmax(0, 1fr)` they are 150 and 150. The core has no min-content, so the model's fill must be the second form. |
| Hug track (as large as what is in it, `max-content`) | **in** | The same idea as `hug` in a flow, and the way a label column sizes itself. It is `max-content`, not `auto`: an `auto` track is stretched when the frame is wider than its tracks (measured: two `auto` columns holding items of 50 and 30 px, in a 400 px frame, came out 210 and 190 px wide, so the second began at x = 210), and `max-content` is not. |
| Gaps | **in**, one `gap` for both axes first; `colGap` and `rowGap` as a later slice | One value is what the model has (`import-html.md` 3.4 notes the same for flex). Different gaps per axis are common and cheap, but they add two three-state fields; open decision 3. |
| Placement of a child (column, row) and spans | **in** | Without it a grid is a wrapped row. |
| Auto-placement, row by row, one cursor (CSS `grid-auto-flow: row`, sparse) | **in**, as the rule for a child with no cell | The default of CSS; a child with no `cell` goes to the next free cell in sibling order. |
| Rows past the explicit ones | **in**, as hug rows | CSS creates implicit rows; `grid-auto-rows: max-content` makes them hug, as a flow's children are. |
| `align` and `justify` of the frame as the place of a child in its cell (start, center, end) | **in** | The CSS `align-items` and `justify-items` the contract's alignment rows already resemble. `between` has no meaning in a grid and counts as start. |
| Margins of a child | **in** | A cell holds the child's margin box, as a flow does. Checked in the throwaway script. |
| Absolute children of a grid frame | **in**, unchanged | They leave the flow of a layout frame today (`Node::absolute`); in a grid they take no cell. |
| Named areas (`grid-template-areas`, `grid-area: name`) | **out** | A second way to say placement, with names to keep in step with the tracks. The numbers of `cell` say the same. |
| Auto-placement beyond row by row (`grid-auto-flow: column`, `dense`), a cell with only a column or only a row | **out** | `dense` changes the order of children on the canvas; a half-placed cell has an algorithm of its own in CSS. A `cell` has both `col` and `row` or neither. |
| `subgrid`, `masonry` | **out** | Nested track sizing across frames; not in browsers uniformly. |
| `minmax(a, b)` other than `minmax(0, <w>fr)`, `fit-content()`, `%` tracks, `repeat(auto-fill)` and `auto-fit`, `min-content` as a track | **out** | Each needs its own rule in the core and its own probes. A fixed track and a fill track cover the cases a design tool draws. |
| A track list with named lines (`[name]`) | **out** | Line names are a part of named placement. |
| `justify-content`, `align-content` of the frame (space between tracks) | **out** | Free space is not shared out between tracks in v1: tracks sit at the start, as CSS does for tracks that are not `auto`. |
| A child that spans two or more **hug** tracks | **in the model, outside the promise** | CSS grows the tracks by a rule with several steps. A simple rule (5.1, step 2) matched Chromium on every random case with one hug track under a span and failed on 6 of 9,000 with two or more. It is listed as unspecified, next to rule G3, with a finding. |

## 4. The model

### 4.1 Fields

All additive, as ADR-0013 does for the round 2 fields: each has a default that reproduces today's behaviour, no existing field changes its type, a file written before them loads unchanged.

| Where | Field | Shape | Default | Meaning |
|---|---|---|---|---|
| frame | `layout` | gains the value `grid` | `none` | The frame lays its flow children out in cells. |
| frame | `columns` | list of tracks | `[]` | The explicit columns. A grid frame with an empty list has one fill column, so a new grid is usable at once. |
| frame | `rows` | list of tracks | `[]` | The explicit rows. Rows past the list, as many as the children need, are hug. |
| frame | `colGap`, `rowGap` | optional number, px | absent: follows `gap` | Later slice (section 11, slice 9). Three states in a patch, as `padT`: absent keeps, `null` unsets, a number sets. |
| any node | `cell` | optional `{col?, row?, colSpan, rowSpan}` | absent: auto-placed, 1 by 1 | The place of a child of a grid frame. `col` and `row` are column and row numbers counted from 1 and come together or not at all. `colSpan` and `rowSpan` default to 1. Three states in a patch, as `image` and `gradient`. |

A **track** is `{kind, size?}` with `kind` one of `fixed`, `fill` or `hug`. `size` is the width or height in px for `fixed` (required), the share for `fill` (optional, default 1, at least 1) and absent for `hug` (a `size` on a hug track is refused, as the model refuses an unknown field rather than ignoring it, `crates/core/meldframe-protocol/src/lib.rs`). A share below 1 is refused because CSS gives such tracks less than all the free space (measured: `0.5fr` and `0.25fr` in 400 px came out 200 and 100), and a share must mean a share.

`align` and `justify` keep their fields and get a meaning in a grid: `justify` is where a child sits in its cell horizontally and `align` vertically (the CSS `justify-items` and `align-items`), each start, center or end, clamped at the start edge when the child is larger than the cell (the contract's rule G2 for the main axis of a flow, here on both axes). In a column flow `align` is horizontal, so the tool text has to say this in one sentence (section 10).

Other fields keep their meaning: `padding` and the four sides, `gap`, `clip`, margins (`marT` to `marL`), `absolute`, `sw` and `sh` of the frame and of a child. A frame that is not a grid keeps `columns`, `rows` and `cell` if they were set and ignores them (the same as padding on a frame without layout), so switching a frame from grid to row and back, and undo of either, lose nothing.

Example, a card grid with a wide header tile:

```json
{"id": "g", "type": "frame", "name": "Plan grid", "layout": "grid", "w": 600, "sh": "hug", "padding": 16, "gap": 12,
 "columns": [{"kind": "fill"}, {"kind": "fill"}, {"kind": "fixed", "size": 160}],
 "rows": [{"kind": "fixed", "size": 64}]}
{"id": "head", "type": "rect", "parentId": "g", "sw": "fill", "sh": "fill", "cell": {"col": 1, "row": 1, "colSpan": 3}}
{"id": "a", "type": "frame", "parentId": "g", "sw": "fill", "h": 120}
```

`head` is placed first at the top row across the three columns; `a` has no cell, so it takes the first free cell in row 2, column 1, in a hug row (rows past the one explicit row are hug).

### 4.2 Compatibility with existing files

- A node with none of the new fields serialises as it does today: `columns` and `rows` are skipped when empty, `cell` and `colGap` and `rowGap` when absent (the pattern of `padT` and `gradient` in `node.rs`). A stored file and a stored op from before load unchanged; no migration of Postgres is needed (`SCHEMA_VERSION` stays 1, `crates/core/meldframe-protocol/src/version.rs`, which says a purely additive optional field does not bump it).
- **The one point that is not purely additive:** `layout` gains a value. ADR-0013's text says "an old client reading a new file only loses what it never drew". That is not what the code does: `Node` and `NodePatch` are `deny_unknown_fields` and `LayoutMode` is a closed enum, so an older server or wasm build **refuses** a node that has `columns` or `layout: "grid"`. This is true of every field since ADR-0013, and it matters only when a build is replaced by an older one after a file has used the new fields. The ADR of this epic should say it plainly and say that web, wasm and server ship together. (Whether to bump `SCHEMA_VERSION` for the new enum value is open decision 1; the recommendation is no, as for the fields.)
- The generated TypeScript (`packages/protocol/src/generated/`, `tools/check-protocol.sh`) gains the types and the value; the web code holds no layout logic except the four options of the Layout control (`packages/web-document/src/layout.ts`).

### 4.3 Limits and validation

Limits live in `Limits` (`crates/core/meldframe-check/src/limits.rs`); every number below is a **proposal**, not measured, and the owner can change it.

| Limit | Proposed | Why |
|---|---|---|
| `max_grid_tracks`: columns, and rows, each | 24 | A 12-column grid twice over. The cost of layout and the length of the exported `grid-template-columns` follow it; the export limit is 256 KiB (`MAX_OUTPUT_BYTES`). |
| Track `size` (fixed) | 0 to `max_size` (100,000, as every size) | As `w` and `h`. |
| Track `size` (fill share) | 1 to 100, refused as `E_OUT_OF_RANGE` outside it | A share is a small whole-looking number; an upper bound keeps the arithmetic sane. |
| `cell.col`, `cell.row` (numbers), `colSpan`, `rowSpan` | 1 to 256 | Bounds the implicit rows a single command can ask for, and so the work of placement. A cell that reaches past the last column is allowed to be written (the column list can change after the child was placed) and is handled by the core, 5.1. |

Checks in `crates/core/meldframe-check/src/validate.rs` (the file has room: 280 lines of 500). They follow the existing pattern: numbers through `num` (`E_NOT_FINITE`, `E_OUT_OF_RANGE`), list length through `TooMany` (`E_TOO_MANY_ENTRIES`, the code the shadows and gradient stops use), and **one new code**, `E_INVALID_GRID` with a short reason (as `E_INVALID_GRADIENT`), for the structural errors only: a fixed track with no size, a size on a hug track, a `cell` with only one of `col` and `row`. Every range goes through `num`: a fixed size outside 0 to `max_size`, a fill share below 1 or above 100, a column or row number or a span outside 1 to 256 is `E_OUT_OF_RANGE` (and a number that is not finite `E_NOT_FINITE`). What is **not** refused at write, because it depends on another node or can change later: a `cell` on a child of a frame that is not a grid, a cell past the last column, two children in the same cell. The core copes with each (5.1) and the findings report them (5.6). `docs/error-codes.md` gets the one code.

### 4.4 The Command and the op

No new `Command` variant: one vocabulary (ADR-0001). A grid is edited with `UpdateNode` patches, so the canvas, the Properties panel, the agent tools and undo share one path.

- `layout`, `columns` and `rows` go in `NodePatch`. `columns` and `rows` are plain fields in `patch_fields!` (`crates/core/meldframe-undo/src/patch_fields.rs`): a patch **replaces the whole list**, as `shadows` does, and the inverse holds the old list. The compile-time guard in that file (every `NodePatch` field named, no `..`) fails the build if one is missed.
- `cell` is three-state (absent, `null`, object) like `image`; `colGap` and `rowGap` like `padT`. They need the hand-written lines in `merge_patch`, `drop_fields`, `patch_is_empty`, `inverse_patch` and `Node::apply_patch` (`patch_ops.rs`, `patch.rs`) and a name in the guard.
- Consequence for concurrent edits: two actors changing different tracks of one frame are two writes of the same field. The replica applies them in the room's order, so the later list replaces the earlier, and per-actor undo drops the fields another actor also wrote (`remote.rs`: "fields the remote command also wrote are dropped"), here the whole list, as for `shadows`. Per-track commands would avoid it at the price of new variants through history, sync and revert; recommended no for v1 (open decision 2).
- `ifSeq` guards a write of `columns` through the frame node and a write of `cell` through the child.

## 5. The layout core

Where: `crates/core/meldframe-layout`, 679 lines today in `place.rs` (180), `size.rs` (113), `findings.rs` (164) and small files. Grid goes into new modules (`grid.rs` for cells and tracks, `grid_size.rs` or the like for sizes) so that no file nears 500 lines; the crate stays well inside its band. The entry points `compute_rects` and `compute_rects_indexed` and the memo of `size_of` keep their signatures; `is_layout_frame` stays "layout is not `none`", and the two-valued branches in `place.rs` and `size.rs` gain a `Grid` arm that calls the new modules.

### 5.1 The algorithm, for a grid frame F whose own size is known

`flow` is the children of F that are not absolute, in sibling order (`flow_children`: position, then id). `N` is the number of columns: `columns.len()`, or 1 when it is empty (one fill column).

1. **Cells.** A child is placed at the column and row numbers of its `cell` when `col` and `row` are both present and `col + colSpan - 1 <= N`: these go first, in sibling order, and may overlap each other (both boxes are drawn, as CSS does). Every other child is auto-placed, in sibling order, with a cursor that starts at row 1, column 1: if `colSpan` (clamped to `N`) does not fit in the rest of the row, go to column 1 of the next row; if the cells it would cover are taken, move one column right and test again; when free, place the child there and leave the cursor on that cell. This is the sparse row-by-row algorithm of CSS Grid Level 1, section 8.5, for the cases the model can write. A cell that reaches past `N` is auto-placed with its span clamped, and `grid_cell_outside` reports it. The row count `R` is the larger of `rows.len()` and the last occupied row; rows past `rows.len()` are hug.
2. **Column sizes**, from the width of the content box (F's width minus horizontal padding):
   - a **fixed** column is its `size`;
   - a **hug** column (and a **fill** column when F hugs on the horizontal axis, rule G6 below) starts at the largest *outer width* (width plus horizontal margins) among the children that start in it and span one column; a fill child counts at its stored `w` (the stored size is what the core uses for a fill that has nothing to share, as in `size_of`);
   - children that span **two or more columns** and cross no fill column (one that is not treated as hug) then raise the hug columns they cross, shortest spans first. For one child and one group of equal span: it asks for `max(0, outer width - (the sum of the columns it crosses + the gaps between them))`; the hug columns it crosses take that equally, none past its *limit* (the largest single-column contribution it had, infinite if no child started in it), and what is left goes equally to all the hug columns it crosses; each column then grows by the largest increase any child of the group asked for. A child that crosses a fill column contributes nothing to the hug columns (CSS leaves it to the flexible track). With one hug column under a span this is "that column takes the whole extra"; with two or more it is the rule that failed on 6 of 9,000 random grids (section 14), which is why it is outside the promise (5.5);
   - **fill** columns share what is left: `free = max(0, content width - fixed - hug - gaps)`, `each = weight * (free / sum of weights)`, with the sum taken in column order. A negative `free` gives 0, as `minmax(0, <w>fr)` does and as `fill_share` does for flows (`fill.rs`).
3. **Child widths.** A child that is `fill` on the horizontal axis gets `max(0, width of its columns + the gaps inside the span - its horizontal margins)`, except that rule G6 makes it fixed (its stored `w`) when its span holds a hug column or a fill column that was treated as hug. Any other child keeps `size_of(c, None, None).0`: a fixed width, a hug text from the measurer, a hug frame from its content. The clamp at 0 matters: the CSS box cannot be negative and the flow code of `place.rs` does not clamp (its `inner_cross - cs - ce` can go below zero; the contract is silent, section 14).
4. **Row sizes**, the same way with heights, after step 3, because a hug text in a fill column wraps at its column width and that decides its height (`size_of(c, Some(width), None).1`, which the memo already keys on the given width). Rows past the explicit ones are hug. A fill row with F hugging vertically is hug (G6).
5. **F's own size when it hugs.** Width: sum of the columns + gaps + horizontal padding. Height: sum of the rows + gaps + vertical padding. This replaces `hug_width` and `hug_height` for a grid frame; the memo key `(index, given width, given height)` stays as it is.
6. **Boxes.** The origin of column `i` is `padding left + sum of the earlier columns + i * gap`, likewise for rows. A child's cell is the rectangle of its columns and rows (spans add the gaps inside). A fill axis takes the cell minus margins; any other axis keeps the child's size. Where a child is smaller than its cell, `justify` and `align` place it: start at the cell edge plus the start margin, center and end by the free space of the cell **minus the margins**, clamped at 0 so that a child larger than its cell starts at the cell's start edge (CSS `safe`). `justify: between` counts as `start`.
7. **Absolute children** are placed as in a flow, at their own `x` and `y` from the frame's top left (`place.rs`), and take no cell and no gap.

Tracks never shrink: fixed tracks wider than F overflow its content box to the right or the bottom, and fill tracks go to 0. Measured in Chromium, the same.

### 5.2 Hug and fill, track by track

| Case | Core | CSS the export writes |
|---|---|---|
| Fixed column or row | its size | `120px` |
| Fill column or row, F fixed on that axis | its share of the free space | `minmax(0, <w>fr)` |
| Hug column or row | the largest outer size of the children in it | `max-content` |
| Fill column or row, F **hug** on that axis | treated as hug (rule G6, a). A finding says so | `max-content` |
| Child `fill` on an axis, its span is all fixed and fill tracks | the span, minus margins | no `width`, and `justify-self: stretch` (or `align-self: stretch`) |
| Child `fill` on an axis, its span holds a hug track (or a fill track treated as hug) | **fixed** at its stored size (rule G6, b). A finding says so | `width: <px>` |
| Child `hug` (text, or a frame with layout) | its content size | text: `width: auto; min-width: <measured>` as today, frame: `fit-content`; both left in their cell by `justify-items` |
| Child `hug` that is not text and not a layout frame | fixed, as the core treats it today | `width: <px>` |
| F `hug` on an axis, no fill track | sum of tracks, gaps, padding | `width: fit-content`, `height: auto` as for a flow |

The row of rule G6 (b), a fill child whose span holds a hug track, is the reason the export cannot let CSS decide: in CSS a stretched item in a `max-content` track contributes its own max-content, not its stored size.

### 5.3 Determinism

The result is a function of the node list and the measurer only, as it is for flows:

- sibling order is (position, id) and the order of children, tracks and cells is that order; nothing iterates a hash map to decide a size or a place (the `kids` and `memo` maps of `lib.rs` are looked up, not walked);
- occupancy is a plain array of rows, the cursor moves forward only, so a placement is the same on every run;
- sums are taken in track order and the only division is `free / sum of weights`, in `f64`, with no transcendental function; native and WASM give the same bits (`just wasm-test` already runs the tests of `meldframe-layout` under WASM from the same sources);
- two files that differ only in the order of the node list lay out the same (a test shuffles the list);
- the text measurer is the seam it is today: the same measurer, the same boxes.

### 5.4 Cost

Placement is linear in the children plus the cells they cover (a column span is at most 24 wide, and the cursor never goes back); sizing visits each child a constant number of times and the memo of `size_of` serves the repeats. NFR-1 asks 5,000 nodes in single-digit milliseconds (`docs/prd.md`, "spike measurement, to be re-measured on the product model"); no figure exists for grid. The slice that adds the core measures a 5,000 child grid and says the figure.

### 5.5 The regime that stays unspecified

G3 (`docs/fidelity-contract.md` section 5) is about content that does not fit. For a grid the picture is better than for a flow in one case and the same in two:

- Fixed tracks that are wider than F, and fill tracks that go to 0, are **specified**: the core and CSS agree (measured). A fixture gates it.
- A child **smaller than its own padding** in a cell of width 0 (a zero fill track) and a **wrapped text at 0 width** break differently in CSS and in Pixi, as in a flow. Unspecified, as G3 says.
- A child that spans **two or more hug tracks** (5.1, step 2) is unspecified in v1, with the finding `span_over_hug`.

### 5.6 Findings

`layout_findings` (`findings.rs`) reports, never changes the layout. For grid frames, reusing the codes where the meaning is the same:

| Code | When | New or reused |
|---|---|---|
| `overflow` | the tracks and gaps of a fixed grid frame exceed its content box on an axis (the message says which) | reused |
| `zero_fill` | a child that is fill resolves to under 1 px; or a fill track does | reused |
| `smaller_than_padding` | as today | reused |
| `fill_under_hug` | G6 (b): a fill child whose span holds a hug track is kept at its own size | reused, message says "its span holds a hug track" |
| `fill_track_under_hug` | G6 (a): a fill track of a frame that hugs on that axis is sized as hug | new |
| `grid_cell_outside` | a `cell` reaches past the last column (it is auto-placed, span clamped) | new |
| `span_over_hug` | a child spans two or more hug tracks (5.5) | new |

Frames with a hug text among their flow children are skipped by the size rules, as today. A `cell` on a child whose parent is not a grid is **not** a finding (stale fields are kept on purpose, 4.1); the Properties panel notes it (section 9). The codes appear in `get_tree` warnings (`crates/integrations/meldframe-http/src/mcp/tools.rs` calls `layout_findings`) and in the export report.

### 5.7 Places that read `LayoutMode` and must not treat grid as a column

Found by `grep` on `LayoutMode`; each needs an explicit arm, and a test that fails without it:

`crates/core/meldframe-layout/src/{place,size,findings,tree}.rs` (`hug_height` falls into its row branch, 2); `crates/features/export/export/src/{decl,emit}.rs` (`is_layout`, the `flex-direction` branch, `fill_basis`, `is_flow_parent`, `unequal_fills`); `crates/core/meldframe-app-parts/src/layout.rs` (`is_layout`, `layout_notes`); `crates/core/meldframe-app-rules/src/{drop,arrow}.rs` (`arrow.rs` gives a grid no move, `drop.rs` a vertical one); `crates/core/meldframe-app-model/src/{property,state}.rs`. Several of these copy the test "a frame whose layout is not `none`" (`tree.rs`, `decl.rs`, `layout.rs`): the grid slices may fold them into one function in the core, which is a small cleanup and not required.

## 6. The canvas

The canvas draws what the render step gives it. With the host flag `layout_on_canvas` on (the default since 21 Nov 2026, decision 37), `App::scene` reads `resolve_laid_out`, which calls `compute_rects` (`crates/app/meldframe-app/src/canvas.rs`, `crates/core/meldframe-render/src/laid_out.rs`). So **grid is drawn by the core change alone**: no change in `apps/web` or in the Pixi code, and the web code holds no layout rule (a `grep` for `'column'` in `apps` and `packages` finds the layout values only in `packages/web-document/src/layout.ts`, in the generated `LayoutMode.ts` and in tests). With the flag off the canvas draws the stored `x`, `y`, `w` and `h`, for grid as for flows.

What needs work is interaction, because the rules of decision 37 are written for one axis:

- **Drop** (`drop.rs`): `index_at` and `flow_siblings` take every layout that is not a row as a column and compare the pointer's y with the middle of each sibling. In a grid that would reorder by y alone, with no visible reason. Proposed rule for the first interaction slice: a drop inside a grid is a **reorder by cell order** (the children's boxes in row-by-row order; the insertion line is drawn before the cell the pointer is over), and a child that has an explicit `cell` keeps it (so the reorder changes nothing for it, and a note says why). Setting a cell by dragging to it needs the track rectangles, which the core does not expose; that is a later slice with its own API (open decision 4).
- **Arrow keys** (`arrow.rs`): left and right move a flow child one place along a row; in a grid they should move one place in cell order; up and down by one row of cells is later. Proposed: left and right only in the first slice, as for a row, up and down do nothing.
- **Handles**: resizing a grid child sets that axis to fixed at the drawn size (the rule for a hug frame of decision 37 applies unchanged); the cell stays.
- **Position fields** are disabled for a grid child (`position_locked`, `in_layout`): both come from "the parent has layout", which is true for grid with no change (`is_layout` in `meldframe-app-parts`).
- **Track lines** (an overlay that shows columns and rows of the selected grid frame) are not planned for v1; they need the track rectangles too.

## 7. The export

`export_frame` writes HTML in two forms, plain CSS (one rule per `[data-node]`) and Tailwind v4 classes (`crates/features/export/export/src/`). `decl.rs` is 485 lines against the cap of 500, so grid goes in a new file (`grid.rs`) with a hook in `style`.

### 7.1 Declarations

On a grid frame (written in this order, after the box and size declarations, replacing the `display:flex; flex-direction` pair):

- `display:grid`
- `grid-template-columns:<tracks>`: always. Empty list: `minmax(0,1fr)`.
- `grid-template-rows:<tracks>`: only when `rows` is not empty.
- `grid-auto-rows:max-content`: always, so rows past the explicit ones hug (and `auto` rows do not stretch).
- `gap` (when above 0), `padding`: as for a flow today.
- `justify-items` and `align-items`: always, `start`, `safe center` or `safe end` from `justify` and `align` (`between` is `start`).
- the track text: `120px` for fixed, `minmax(0,<w>fr)` for fill, `max-content` for hug; a fill track of a frame that hugs on that axis is written `max-content` (G6 a).

On a child of a grid frame that is not absolute:

- a fixed axis: `width` or `height` in px, and **no** `flex-shrink` (the code that adds it for a main axis, `decl.rs`, applies to flow parents only);
- a fill axis (G6 does not turn it fixed): nothing for the size, and `justify-self:stretch` (width) or `align-self:stretch` (height); this is a new case beside `FillMain` and `FillCross` in `decl.rs` (`Axis::FillCell`). It must not write `flex: 1 1 0%`;
- a hug text: `width:auto; min-width:<measured>` as today, which stays correct because the frame's `justify-items` is not `stretch` (measured: with the default `stretch` a hug text is stretched to its cell, 100 px instead of its text width);
- margins: `margin`, as today;
- `cell`: `grid-column:<col> / span <n>` and `grid-row:<row> / span <n>` when the cell is explicit (`span 1` is written as the bare line number); `grid-column:span <n>` and `grid-row:span <n>` for an auto-placed child with a span above 1; nothing otherwise. A cell that reaches past the last column is written as the core placed it (auto, span clamped), so the HTML and the canvas agree.

Placement is **not** written for auto-placed children: CSS auto-places them, so the HTML stays editable by a developer (contract section 3: "boxes are produced by CSS layout ... not by absolute positioning of every node"). The price is that the core's cursor must equal the browser's; the oracle's grid fuzz (section 8) is what checks it, and if it ever finds a case where they differ, the export can write explicit lines for that case without changing the model (open decision 5).

### 7.2 Tailwind, with the existing `tw.rs` style

`tw.rs` maps a declaration to a named utility when the contract names one and to an arbitrary-value utility otherwise. Every class below was compiled with the Tailwind 4.3.5 compiler on 2026-11-24 and produced the declaration shown:

| Declaration | Class | New arm in `tw.rs` |
|---|---|---|
| `display:grid` | `grid` | `("display","grid") => "grid"` |
| `grid-template-columns:120px minmax(0,1fr) max-content` | `grid-cols-[120px_minmax(0,1fr)_max-content]` | prefix `grid-template-columns` => `grid-cols-` |
| `grid-template-rows:64px minmax(0,2fr)` | `grid-rows-[64px_minmax(0,2fr)]` | prefix `grid-template-rows` => `grid-rows-` |
| `grid-auto-rows:max-content` | `auto-rows-max` | `("grid-auto-rows","max-content") => "auto-rows-max"` |
| `gap:12px` | `gap-[12px]` | none (exists) |
| `column-gap:8px`, `row-gap:4px` (slice 9) | `gap-x-[8px]`, `gap-y-[4px]` | prefixes `column-gap` => `gap-x-`, `row-gap` => `gap-y-` |
| `justify-items:start` | `justify-items-start` | `("justify-items","start")` |
| `justify-items:safe center` | `[justify-items:safe_center]` | none (generic fallback) |
| `align-items:start` | `[align-items:start]` | none (`items-start` is `flex-start` in Tailwind, the same in a grid but another text; the exact property is used so the two forms say the same thing) |
| `justify-self:stretch` | `justify-self-stretch` | `("justify-self","stretch")` |
| `align-self:stretch` | `self-stretch` | none (exists) |
| `grid-column:2 / span 3` | `col-[2_/_span_3]` | prefix `grid-column` => `col-` |
| `grid-row:span 2` | `row-[span_2]` | prefix `grid-row` => `row-` |

`arb()` already turns a space into `_`, which is how `2 / span 3` becomes `2_/_span_3`. The fallback form `[grid-template-columns:...]` also compiles, so a missed arm degrades to a longer class and not to a missing style.

### 7.3 The round trip with the importer

The export writes `grid-template-columns` and so on in the forms above, and `docs/import-html.md` is "the inverse of `tw.rs`" for the Tailwind forms. Section 13 lists the rows the importer gets. What an export of a grid comes back as, in the terms of its section 3.7: ids, `pos` strings and the stored `w` and `h` of fill and hug children are not written (as today); a **grid with `columns: []` comes back with one explicit fill column** (the export writes `minmax(0,1fr)`), a normalisation and not a loss of meaning; auto-placed children come back auto-placed, explicit cells explicit; `layout` and the tracks round trip exactly.

## 8. The fidelity contract and the oracle

### 8.1 Rows to add to `docs/fidelity-contract.md` section 4.1 (status "planned (model)" until the oracle passes, then "supported")

| Model property | CSS | Tailwind | Notes |
|---|---|---|---|
| `layout` grid | `display:grid` | `grid` | the frame's children are in cells; absolute children take none |
| `columns`, `rows`: fixed, fill, hug | `120px`, `minmax(0,<w>fr)`, `max-content` in `grid-template-columns` and `-rows` | `grid-cols-[..]`, `grid-rows-[..]` | `minmax(0,..)` because a bare `fr` cannot shrink below min-content; `max-content` because `auto` stretches |
| rows past `rows` | `grid-auto-rows:max-content` | `auto-rows-max` | hug |
| `cell` `{col,row,colSpan,rowSpan}` | `grid-column`, `grid-row` | `col-[..]`, `row-[..]` | auto-placed children are not written; the core's cursor equals the browser's by the oracle's grid fuzz |
| `align`, `justify` in a grid | `align-items`, `justify-items`: `start`, `safe center`, `safe end` | `[..]` forms | `between` is `start`; clamp at the start edge on both axes |
| a fill child in a grid | `justify-self:stretch`, `align-self:stretch`, no size | `justify-self-stretch`, `self-stretch` | rule G6 (b) when its span holds a hug track |

And in section 4.3 the sentence "flex-wrap, CSS grid, min/max size, ..." loses "CSS grid".

### 8.2 Rule G6 (proposed), and the change to G3

- **G6 Fill in a hug grid.** (a) A fill column or row of a grid frame that hugs on that axis is stored as it is, but laid out and exported as a hug track. (b) A child that is `fill` on an axis whose span holds a hug track (or a track treated as hug by (a)) is laid out and exported as fixed on that axis. Reason, as for G1: CSS resolves a stretch against the max-content of the track and the core against the stored size; measured and stated in 5.2.
- **G3** gains the three lines of 5.5: what a grid specifies (fixed overflow, fill to 0), and the two regimes it leaves unspecified (a child smaller than its padding in a zero track, and a child spanning two or more hug tracks).
- Section 3 of the contract says "boxes are produced by CSS flex layout"; it should say flex or grid.

### 8.3 What the oracle needs (`tools/fidelity-oracle`, `crates/core/meldframe-oracle-runner`)

The runner needs no change (it lays out whatever file it is given). The harness needs:

- **Fixtures** (names proposed, after `f23`; the importer plan uses `h01` onward): `f24-grid-fixed-fill` (the first test of slice 2a, 11.2: fixed and weighted fill columns, a gap, padding, auto-placement across rows), `f25-grid-hug` (hug columns and rows with texts and frames, a hug frame, G6 (a) and (b)), `f26-grid-cells` (explicit cells, spans, overlap, a cell past the last column, auto children around explicit ones, rows past `rows`), `f27-grid-align` (the nine places of `justify` and `align`, a child larger than its cell, margins), `f28-grid-overflow` (fixed tracks wider than the frame, fill tracks at 0, `clip`), `f29-grid-nested` (a grid in a row in a grid, an absolute child, a hug text that wraps in a fill column). The contract's floor `FLOORS.fixtures` in `gate.mjs` (22 today) rises with them.
- **A separate fuzz generator** `fuzzGrid(seed)` in `fuzz.mjs`, named `fg-NNN`, with its own seed range. The existing `fuzz(seed)` must not change: a failing seed is printed as `fz-NNN` and added to the fixtures, and changing the generator would change what every old seed means. The grid generator covers the cases the throwaway script covered (random tracks, spans, explicit and auto cells, alignment, margins, hug and fixed frames) plus text and nested frames. Pull requests would run a number of grid seeds beside the 50 of `fz`; the number and the cost are to be measured, not guessed.
- **`geometry.mjs`**: `overflowing()` (the G3 judge) reads `n.layout === 'row'` and treats every other layout as a column; it needs a grid branch (tracks wider than the frame are **not** G3; a child smaller than padding, a fill under 1 px and a span over two hug tracks are). `normaliseG1()` in `fuzz.mjs` takes `layout !== 'none'` for flex and needs G6.
- **Probes.** Grid adds no paint, so no pixel probe is needed beyond what the Tailwind form already gets for free: the gate compares the Tailwind page with the CSS page (boxes within 0.001 px).
- **Sensitivity** (`sensitivity.mjs`): mistakes the gate must catch, each with a fixture in which it matters: `minmax(0,1fr)` written as `1fr` (a long unbreakable text in a fill column), `max-content` written as `auto` (a hug column in a wider frame), `safe center` written as `center` (a child larger than its cell), the `/ span n` dropped from a cell, `justify-self:stretch` left out (a fill child).
- **Known gaps** (`known-gaps.mjs`): none expected for the specified regimes. A gap that shows up is listed with its cause, as `f14` and `f17` are.

Pull requests run the fixtures and the fuzz; the nightly runs more seeds; both exist (contract section 7, `nightly.yml`). The oracle is verified on Linux only (issue 231), as today.

## 9. The Properties panel and the app layer

The panel draws the app layer's `Properties` and sends `Action::SetProperty`, which the app layer turns into `UpdateNode` (`crates/core/meldframe-app-parts/src/property.rs`); an agent sends the same `UpdateNode` through `apply_commands`. So the panel and the tool share the command by construction. Grid needs, in order:

- **App layer** (`meldframe-app-model`, `meldframe-app-parts`): `Property::Layout` already takes a `LayoutMode`, so Grid is the new value. New properties, in the pattern of the shadows list (`SetShadow`, `AddShadow`, `RemoveShadow` with `max_shadows` and the codes `limit` and `not_found`): `SetTrack {axis, index, track}`, `AddTrack {axis}` (a fill track, up to `max_grid_tracks`, refused as `limit`), `RemoveTrack {axis, index}` (`min_tracks` at one column), and `SetCell` for a child (column, row, spans; clearing makes it auto). `Properties` gains `columns`, `rows`, the cell of the selected child and `max_tracks`. `LayoutNote` gains: a cell on a child of a frame that is not a grid has no effect; `between` has no effect in a grid; a fill child in a hug track is fixed (G6). `meldframe-app-model` is already 2,524 lines in `src` by `wc`, above the 2,500 at which D1 calls a crate a candidate for a planned split (`AGENTS.md`, decision 35): the slice that adds these types should split it first or at the same time (the slicing of section 11 puts it in its own slice).
- **Front end** (`packages/web-panels`, `packages/web-document`): the Layout direction control (`LAYOUT_MODES` in `layout.ts`, a `Segmented` in `LayoutSection.svelte`) gains **Grid** as a fourth option, which is the "Flow (free, row, column, grid)" that the first design pass of `docs/ui-design.md` drew. For a grid frame the section shows the gap and padding as for a flow, then two lists, **Columns** and **Rows**, in the pattern of `ShadowSection.svelte`: one row per track with its kind (Fixed, Fill, Hug, a `Segmented` or select) and its value (px, or the share) and a remove button, and an Add button disabled at the limit. A selected child of a grid frame gets a **Cell** group beside Placement (`PlacementSection.svelte`): column, row, column span, row span, and a clear button. `Align` and `Justify` are labelled for the grid by their axis (Horizontal, Vertical) rather than "across" and "along". Every control reports an action and holds no state (`web-panels` rule); a field takes its text on Enter or blur as the others do.
- Checks in the pattern of the existing ones: `packages/web-panels/src/layout.test.ts`, the app layer's `meldframe-app-inspector-tests`, and a browser check in the style of `packages/web-panels/scripts/text-style-e2e.mjs`, including the 24 px hit areas that `packages/web-chrome/scripts/states-e2e.mjs` asserts.

## 10. The MCP tool text for agents

There is no grid tool: `apply_commands` and `get_nodes` already carry any node field. But the text of `apply_commands` (`crates/core/meldframe-mcp-defs/src/lib.rs`, mirrored in `docs/mcp-tools.json` and `skills/meldframe/SKILL.md`, whose test `meldframe-mcp-tests/tests/skill.rs` checks that every tool and code it names exists) **names `padding`, `padT` to `padL`, the margins and `absolute`, and not `layout`, `sw`, `sh`, `gap`, `align` or `justify`** (checked: the strings are not in the text). An agent learns those from `get_nodes`. The grid slice should add one sentence for flows and grid together, so grid does not arrive as the only documented layout. Proposed wording for the node description of `addNode` (to be refined with the tests):

> layout (none, row, column or grid), gap, sw and sh (fixed, hug or fill), align and justify. In a grid frame, columns and rows are lists of tracks {kind fixed with size in px, fill with an optional share of 1 or more, or hug}, up to 24 each (no columns: one fill column; rows past the list are hug); a child sits where its cell {col, row, colSpan, rowSpan} says (numbers from 1, col and row together), and with no cell it takes the next free cell, row by row; justify places a child in its cell horizontally and align vertically (start, center or end). A fill child in a hug track or a hug frame is treated as fixed. updateNode replaces the whole columns or rows list; cell null removes the cell.

And the `get_tree` text names the new findings next to `overflow`, `fill_under_hug`, `zero_fill` and `smaller_than_padding`; `skills/meldframe/SKILL.md` line 17 and its layout paragraph do the same, and gain one example (the card grid of 4.1) so an agent can copy a working shape. `docs/error-codes.md` gets `E_INVALID_GRID`. Per `AGENTS.md` ("Skill for agents") the skill changes with the tool text.

## 11. Recommendation, the first slice, and the slicing

### 11.1 Recommendation

Build it, as a model round with its own ADR, in the order below. Two things are already ordered: Gatehouse is next after the canvas layout (decision 36), and the scoped HTML importer is ordered as epic E4.4 (decision 41, issue 272), which per that decision starts now, in parallel with other work and with no wait for Gatehouse (the decision marks that timing as the author's reading, reversible). Grid is not ordered yet; this note recommends:

1. **Gatehouse keeps its priority.** Grid touches `meldframe-protocol`, `-check`, `-undo`, `-layout`, `export` and the app layer; Gatehouse touches sign-in, the gate and the server, and the importer touches new crates (`meldframe-css`, `html-import`) and, in its slice 7, the tool crate. So grid and Gatehouse do not share files. Grid and the importer do share some: importer slice 7 edits `docs/mcp-tools.json`, `docs/error-codes.md`, `skills/meldframe/SKILL.md` and `apps/server`, and its slice 8 edits `tools/fidelity-oracle` and `docs/fidelity-contract.md`, which grid slices 1, 4 and 5 edit too (and `meldframe-mcp-defs`). The overlap is in those shared files, and the pull requests that touch them rebase in turn. Grid can start after Gatehouse is under way, not after it ends; the owner's order of priority stands.
2. **Grid slices 0 to 3 (decision and ADR, model, layout core, export), then 4 and 5 (oracle, agent surface).** They can run in parallel with the importer's slices 1 to 3 (shared CSS readers, HTML tree builder, CSS cascade).
3. **The importer's layout mapping (E4.4 slice 4) maps grid at once if grid slice 4 has landed by then (slice 3 is enough to write the rows as approximated).** If it has not, the importer lands first with the refusal of `display: grid` that its section 3.4 has today, and the rows of section 13 replace it later (slice 8). Both work; the first saves one pass over the mapping.
4. **Grid slices 6 and 7 (app layer, panel)** after that. A person editing a grid by hand is the part with the most to decide (section 15, items 4 and 6), and an agent already has the whole of grid from slice 5.
5. Both grid slice 4 and E4.4 slice 8 add fixtures to `tools/fidelity-oracle/fixtures.mjs` (`f24` onward and `h01` onward) and text to `docs/fidelity-contract.md`: they should not be open at once, or the second rebases over the first.

The alternative, grid after the importer, is also sound: the importer's report names every `grid-*` property it refused, which is evidence of how much real pages need grid. Nothing in this repository counts grid in real pages today, and this note does not claim a number.

### 11.2 The first slice, its size and its failing test

**Slice 1, the model: `layout: grid`, `columns`, `rows`, `cell`, their checks, undo and generated types.** Estimated 550 to 650 changed lines outside the generated TypeScript (the margin slice #220 added 483 lines, gradients #222 added 708), one kind of content (Rust). **Files:** about 15 to 17 without the generated ones (`packages/protocol/src/generated/`, which do not count under `AGENTS.md`), against the limit of 15 and the ceiling of 20. The list: `meldframe-protocol` (`enums.rs`, `node.rs`, `patch.rs`, a new `grid.rs` for `Track`, `TrackKind`, `Cell`), `meldframe-check` (`limits.rs`, `validate.rs`, `error.rs`), `meldframe-undo` (`patch_fields.rs`, `patch_ops.rs`), the coverage table of `meldframe-render` (`coverage.rs`) and the tests that read it (`meldframe-render/tests/coverage.rs`: `full_node()` and `change()`; the export tests' `coverage.rs`), the new tests (`meldframe-core/tests/grid_model.rs`, a history test for undo as `meldframe-history/tests/margin_undo.rs` had, and, as #220 did, an MCP test in `meldframe-mcp-tests`), `docs/error-codes.md`. #220 touched 21 files in all, including those tests and about seven generated or doc files. If the count goes over 15, the pull request says why under `## Why` (the file list is what a compile guard and the coverage table force into one change); if it would pass 20, split it as slice 1a (`layout`, `columns`, `rows`) and slice 1b (`cell`, whose three-state handling is the largest piece).

**The coverage rows in slice 1 are not the final ones.** The tests of `meldframe-render/tests/coverage.rs` fail for a row marked `FROM_LAYOUT` unless changing the field changes the output of `resolve_laid_out` (`the_layout_fields_the_canvas_does_not_render_by_default_are_rendered_by_resolve_laid_out`), and the export tests fail for a row marked rendered without a proof, and until slices 2a and 3 the core and the export ignore the new fields. So slice 1 adds the rows (the test that the table lists exactly the fields of a node needs them) with `Unsupported("not laid out yet")` for the canvas and for the export, a mark that is true at that point, plus the `cell` that `full_node()` must set. Slice 2a changes the canvas mark of `columns`, `rows` and `cell` to `FROM_LAYOUT` (the count of 16 becomes 19 there, with a grid base frame in `change()`), and slice 3 changes the export mark to rendered with a proof for each. Slice 1 also adds the **guard**: `check_node` and `check_patch` refuse `layout: grid` ("grid layout is not available yet", `E_INVALID_GRID`) until slice 3 removes the line, so the window in which the model accepts a grid that the core would lay out as a column never exists on `develop`.

**The failing test first** (the agreed seam for a model change is the core's submit-a-command interface, as `crates/core/meldframe-core/tests/margin.rs`): `crates/core/meldframe-core/tests/grid_model.rs`. A frame takes `{"columns":[{"kind":"fixed","size":120},{"kind":"fill"}],"rows":[{"kind":"hug"}]}` in an `UpdateNode`; the stored node reads them back; undo returns the empty lists; a node JSON from before (no such fields) loads with empty lists and serialises without them; 25 columns are refused `E_TOO_MANY_ENTRIES`; a hug track with a size, a fixed track with no size and a `cell` with `col` but no `row` are each refused `E_INVALID_GRID`; a fill share of 0.5 is refused `E_OUT_OF_RANGE`; `layout: grid` is refused by the guard (`E_INVALID_GRID`). Without the change the test does not compile (no `columns` field), which is the failing state.

**The first failing test of slice 2a** (the layout core, `crates/core/meldframe-layout/tests/layout_grid.rs` (planned, not written yet), with the stub measurer): a frame `g` of layout grid, 400 by 200, padding 0, gap 10, columns fixed 100, fill 1, fill 2, holding four rects that are `sw: fill`, height 30, with no `cell`. Expected: `a` at (0, 0) 100 by 30; `b` at (110, 0) 93.3333 by 30; `c` at (213.3333, 0) 186.6667 by 30; `d` in the next row at (0, 40) 100 by 30 (the free space is 400 - 100 - 20 = 280, shared 1 to 2; the second row is hug). Chromium gives the same boxes within its 1/64 px snap (93.3281 for `b`, measured), and the same file is fixture `f24` of the oracle. Before the slice the test fails because the core lays a grid frame out as a column.

### 11.3 The slices, each under 1,000 changed lines, in dependency order

| # | Slice | Where, kind | Estimate | Depends on |
|---|---|---|---|---|
| 0 | Owner text: the decision (wording in section 16), the ADR (the next free number at the time: 0025 is proposed on another branch, so 0026 if that lands first; 0024 is the importer's) that records the fields of section 4 and the compatibility point of 4.2, the contract rows and G6 as "proposed", the epic box, the glossary terms | `docs/`, docs only | about 300 lines | this note approved |
| 1 | Model, checks, undo, generated types, coverage rows marked not laid out yet, the guard (11.2) | Rust | 550 to 650 | 0 |
| 2a | Layout core (and the canvas mark `FROM_LAYOUT` of the new fields): cells, fixed and fill tracks, gap, padding, `justify` and `align`, margins, absolute children, fixed grid frame, the findings that need no hug (`overflow`, `zero_fill`, `grid_cell_outside`) | `meldframe-layout` (`layout_grid.rs` in its `tests/`, planned) | about 650 (330 code, 320 tests) | 1 |
| 2b | Layout core: hug tracks and hug frames, spans over hug tracks, G6 and its findings, `span_over_hug`, the shuffle test and the 5,000 child timing | same | about 600 | 2a |
| 3 | Export: `grid.rs` in the `export` crate, the `tw.rs` arms, the export's coverage (marks rendered, with proofs) and geometry tests; **removes the guard** | `export`, `meldframe-export-tests` | about 550 | 2b |
| 4 | Oracle: fixtures `f24` to `f29`, `fuzzGrid`, `geometry.mjs` and `fuzz.mjs` changes, sensitivity, the contract rows to "supported", G6 and G3 text | `tools/fidelity-oracle`, `docs/fidelity-contract.md` | about 700 (JS 450, docs 150, tests) | 3 |
| 5 | Agent surface: tool text and schema, `docs/mcp-tools.json`, the skill, `get_tree` warnings, tests over MCP against the real server | `meldframe-mcp-defs`, `meldframe-http`, `skills/`, `meldframe-mcp-tests` | about 300 | 3 |
| 6 | App layer: split `meldframe-app-model` below 2,500 lines (a move, which does not count), then `Property` variants for tracks and cells, `Properties` fields, `LayoutNote`s, and the drop and arrow rules of section 6 | `meldframe-app-*`, `meldframe-app-layout-tests` | about 900, or two slices (6a properties, 6b drop and arrow) | 3 |
| 7 | The panel: the Grid option, the Columns and Rows lists, the Cell group, labels for the axes; unit tests and a browser check | `packages/web-panels`, `packages/web-document` | about 500 | 6 |
| 8 | Importer rows (section 13): the mapping in `html-import` and its fixtures, and the edit of `docs/import-html.md`; two pull requests, docs and code | importer crate, docs | about 400 and 100 | 4, and the importer's layout mapping |
| 9 | Later, each its own decision: `colGap` and `rowGap`, the track rectangles from the core for a cell by drag and for track lines on the canvas, `repeat()` sugar in the panel | | not scheduled | |

Slices 4 and 5 are independent of each other; 6 can start after 3. Slice 6 is the one most likely to be split, because the app layer is the biggest of the touched crates.

## 12. Where the epic box sits in `docs/epics.md`

A new box **E3.6 Grid layout (M)** after E3.5 in M3 ("Model round 2 and variables": the layout and paint model), marked as **after W1** in the draft, as decision 40 item 1 and `docs/ui-design.md` 3.1a say, and outside the M3 exit criteria (whether work starts before W1 ends, as 11.1 recommends in parallel with the importer, is open decision 8, not settled by the words "after W1"), with `docs/grid-layout.md` as the plan and the ADR as its design. The wording proposed is in section 16; `docs/epics.md` is not edited in this pull request. The earlier reason (another change was editing the E4.3 and E4.4 text) is gone, since decision 41 and E4.4 are on `develop`; the box goes in slice 0 on purpose, a deviation from the acceptance line of issue 271 ("a section in `docs/epics.md` for the epic box"): a box in the plan of record should come with the owner's answer to this note, so it is not ticked, ordered or numbered before the owner has said whether and when. The milestone table row for M3 ("E3.1 to E3.5") becomes "E3.1 to E3.6" in the same pull request. The alternative is a new milestone after M6; it is not needed, because M3 is about the model and E3.6 is a model round.

## 13. HTML import: what changes in `docs/import-html.md`

Today section 3.4 has one row: "`display: grid`, `inline-grid`, and every `grid-*` property: the frame is a `column` of its children in DOM order: **refused**". That row is replaced by the rows below once the contract's grid rows are **supported**, which is grid slice 4 (the oracle), not slice 3: `docs/import-html.md` section 3.0 defines "supported" as a value that reaches a field the contract lists as supported and round-trips, and 8.1 keeps the rows "planned (model)" until the oracle passes. Until slice 4, an importer built earlier keeps the refusal, or the rows are written as approximated. Statuses follow that note's legend. Proposed rows, not an edit:

| Source | Maps to | Status | Reason |
|---|---|---|---|
| `display: grid` | `layout: grid` | supported | section 7 |
| `display: inline-grid` | as `grid` | approximated | the outer inline-ness is dropped, as for `inline-flex` |
| `grid-template-columns` and `-rows` of `<n>px`, `minmax(0, <n>fr)`, `max-content`, and `repeat(<k>, ...)` of those, at most 24 tracks | fixed, fill with that share, hug | supported | the export writes exactly these |
| `<n>fr`, `auto`, `min-content`, and line names in `[ ]` | fill, hug, hug, names dropped | approximated | a bare `fr` can be pushed wider by its content, `auto` stretches in a wide frame, which the model's tracks do not; reported |
| `minmax(a, b)` other than `minmax(0, <n>fr)`, `%`, `fit-content()`, `repeat(auto-fill\|auto-fit, ...)`, `subgrid`, `masonry`, more than 24 tracks | the first track forms it can read, else one fill column | refused | section 3 |
| `grid-template-areas`, `grid-area: <name>`, named lines in `grid-column` and `grid-row` | none | refused | named placement is out |
| `grid-column`, `grid-row`, `-start`, `-end`, and `grid-area: r / c / r / c` with positive whole lines and `span <n>`, when both axes are given or neither is | `cell` | supported | |
| the same with one axis given | the span, auto-placed, the line dropped | approximated | a cell needs both |
| negative lines | dropped | refused | |
| `grid-auto-flow: row` | none | supported | the default |
| `grid-auto-flow: column`, `dense` | none | refused | section 3 |
| `grid-auto-rows: max-content` or `auto` | none (rows past the list are hug) | supported | |
| other `grid-auto-rows`, `grid-auto-columns` | none | refused | |
| `gap`, `row-gap`, `column-gap`, equal | `gap` | supported | |
| `row-gap` and `column-gap` that differ | one of them | approximated, supported after slice 9 | |
| `justify-items`, `align-items`: `start`, `center`, `end`, `flex-start`, `flex-end` | `justify`, `align` | supported | |
| `justify-items: stretch` or `normal`, `align-items: stretch` | children that have no size are `fill` | approximated | as for flex (`align-items: stretch`, section 3.4) |
| `justify-self`, `align-self` on one child, `place-self` | dropped | refused | the frame holds the alignment |
| `justify-content`, `align-content`, `place-content` of `start` or `normal` | none | supported | |
| other `justify-content`, `align-content` on a grid | dropped | refused | free space between tracks is out |
| Tailwind: `grid`, `grid-cols-[..]`, `grid-rows-[..]`, `auto-rows-max`, `col-[..]`, `row-[..]`, `gap-x-[..]`, `gap-y-[..]`, the integer forms `grid-cols-<n>` (`repeat(n, minmax(0,1fr))`), `col-span-<n>`, `row-span-<n>`, `col-start-<n>`, `row-start-<n>` | as the CSS rows above | supported | the first group is the inverse of `tw.rs`; the integer forms need no theme (compiled and checked) |

Effect on the counts of its section 3.8 (111 rows: 41 supported, 34 approximated, 36 refused): one refused row leaves and about two dozen rows enter, so the counts and the sentence "a refused row is often one property of a common kind (`line-height`, grid)" are to be taken again with `grep`, as that section says. Effect on 3.7: the export of a grid comes back as a grid (section 7.3 above). The importer's own slices and sizes (its section 6) do not change; slice 4, the layout mapping, gains the grid rows and waits for slice 3 above.

## 14. What this note does not verify, and where the contract is silent

**Checked by reading the code:** every path, line count and function name above. The `LayoutMode` sites are from `grep`; the file sizes from `wc`; the tool text from `docs/mcp-tools.json` and `crates/core/meldframe-mcp-defs/src/lib.rs`.

**Measured, on 2026-11-24, in Chrome for Testing 153.0.8010.12, by hand:** the CSS behaviours quoted in sections 3 and 5 (auto tracks stretch and `max-content` does not; `1fr` blows out and `minmax(0,1fr)` does not; fill shares below 1; hug items stretch under the default `justify-items`; overflow of fixed tracks, fill to 0; equal growth of two max-content tracks under a spanning child). And a **throwaway prototype of the algorithm of 5.1** in JavaScript, run inside the browser page against real grids: random frames of 1 to 4 columns and 0 to 3 explicit rows with fixed, fill (shares 1 to 3) and hug tracks, gaps 0 to 10.5, padding 0 to 16, fixed or hug frames, `start`, `center` or `end` in both axes (`safe`), 1 to 7 children of fixed or fill size with spans up to 3 and 30% explicit cells, and 40% of the children with margins of 0 to 6. Results, by run. Without margins and with no restriction: 9,000 random grids (three seeds of 3,000), 6 differed from Chromium by more than 0.05 px, all with a child spanning two or more hug tracks. With margins, leaving out the cases where a child contributes to two or more hug tracks (about 60% of the generated grids had one, which says how common the case is when hug tracks are this frequent): 6,012 grids compared over four seeds of 4,000, none differed, worst 0.022 px (the 1/64 px snap of the browser, rule G5). The first version of the margins run did not clamp a fill child at 0 and 11 of 3,037 compared failed, all on a fill child narrower than its margins, which is why 5.1 clamps. That script is **not** in the repository (this issue changes no code) and its numbers are one run each; it covers no text, no hug frame as a child, no nested grid, no absolute child, no per-axis gap and no other browser. The oracle fixtures and the grid fuzz of section 8 are what would turn it into a gate, and they would cover those cases.

**Compiled with Tailwind 4.3.5 (not in the oracle):** every class of 7.2 and the forms of section 13.

**Not verified at all:** the speed of the grid core against NFR-1 (5.4); Firefox and Safari (the contract promises Chromium only); the cost of the grid fuzz in CI; how the Properties panel looks with four options in the Layout control at 1280 by 800 (the check of slice 7); whether real pages in the owner's test set use grid (no count exists).

**Where the contract is silent** (each needs a sentence in the contract with the slice that decides it): the meaning of `align` and `justify` in a grid (this note proposes one); what a `hug` frame's `width: fit-content` does when its content is wider than the space the page gives an absolutely positioned root (the flex export has the same exposure, and no fixture covers it); the flow code's negative fill size (`place.rs` does not clamp `inner_cross - cs - ce`; the grid code does, and a flow fixture would show the difference); whether a hug text in a grid cell keeps its pinned `min-width` under every `justify-items` (it does under `start`, which is what the export writes; no fixture yet); and the growth rule for a span over two or more hug tracks (the simple rule of 5.1 is not Chromium's in every case, the exact rule is the spec's track sizing algorithm, section 11.5 of CSS Grid Level 1, which a later slice can port).

## 15. Open decisions for the owner

1. **Is `layout: "grid"` an additive change, with `SCHEMA_VERSION` 1?** Recommendation: yes, as for the fields since ADR-0013, with the compatibility sentence of 4.2 in the ADR (an older build refuses a file that uses grid; web, wasm and server ship together).
2. **Whole-list `columns` and `rows`, or a command per track.** Recommendation: whole list (as `shadows`), because a new `Command` variant goes through history, sync and revert. The cost is last writer wins on the whole list when two actors edit one frame's tracks at once.
3. **One `gap` first, `colGap` and `rowGap` later.** Recommendation: yes; the fields are a slice of their own (9).
4. **How a person sets a cell on the canvas.** Recommendation for v1: in the panel, and the canvas drop is a reorder by cell order; dragging into a cell and drawing track lines need the track rectangles from the core, which is a later slice with its own API.
5. **Auto-placement: written by CSS or written by the export.** Recommendation: CSS places auto children (the HTML stays editable), the oracle's grid fuzz checks the core's cursor against it; if it finds a difference the export writes explicit lines for that case.
6. **Names.** Recommendation: **Grid**, **Track** (one column or one row, with a kind and a size) and **Cell** (the place of a child) as glossary terms; `columns`, `rows`, `cell`, `colSpan`, `rowSpan` as field names. The glossary has no word for them today; `GLOSSARY.md` is the owner's file.
7. **Limits.** 24 tracks an axis, shares 1 to 100, cell numbers and spans 1 to 256 (4.3): proposed, not measured.
8. **Order, and when E3.6 starts.** The epic is "after W1" by decision 40, and 11.1 recommends starting its first slices alongside the importer; the owner decides which. Recommendation in 11.1: Gatehouse keeps its priority, grid starts in step with the importer (E4.4), and the importer's layout mapping follows slice 3 of grid if it can.

Owner sign-off on the subset: pending.

## 16. Proposed wording for the files this note does not edit

For the owner's text in slice 0, as drafts. None of these files is changed by this note.

**`docs/epics.md`**, a new box after E3.5, and "E3.1 to E3.5" becoming "E3.1 to E3.6" in the table of milestones:

> ### E3.6 Grid layout (M)
> After W1 by default (decision 40, item 1; the start date is the owner's, see the open decisions of the plan). Plan: `docs/grid-layout.md` (status plan, awaiting the owner); the design is the ADR of its slice 0. Not started. Outside the W1 gate and the M3 exit criteria.
> - [ ] A frame can be a grid: `layout: grid`, `columns` and `rows` of fixed, fill and hug tracks, and a `cell` for a child (column, row, spans), as additive fields that are validated, undoable and refused past their limits; a file written before them loads unchanged.
> - [ ] The layout core places and sizes a grid by the algorithm of `docs/grid-layout.md` section 5, the same on native and WASM, with findings for what does not fit; the canvas draws it with no change to Pixi.
> - [ ] The export writes `display:grid` in plain CSS and in Tailwind; the grid rows of the fidelity contract and rule G6 are supported, with fixtures, a grid fuzz and sensitivity checks, and the oracle passes.
> - [ ] An agent reads and writes a grid through `apply_commands` (tool text, skill, `E_INVALID_GRID`), tested over MCP against the real server.
> - [ ] The Properties panel has Grid in the layout direction, lists of columns and rows, and the cell of a child; a drop in a grid is a reorder in cell order; checked in a browser.
> - [ ] HTML import maps `display: grid` (when E4.4 exists).

**`docs/decisions.md`**, a row after 41 (the number is the next free one at the time: 42 when this was written; status proposed, awaiting the owner):

> 42 | Grid layout, planned in `docs/grid-layout.md` and scheduled as E3.6 (after W1 as the default; the start is open decision 8). (1) A grid is `layout: grid` with `columns` and `rows` of fixed, fill (`minmax(0, <share>fr)`) and hug (`max-content`) tracks and a `cell` on a child; auto-placement is row by row. (2) Additive fields on the one `UpdateNode`: no new `Command`; a patch replaces the whole track list. (3) Named areas, `subgrid`, `masonry`, `dense`, other `minmax` forms and free-space distribution between tracks stay out. (4) The slice that adds the model refuses `layout: grid` until the core and the export lay it out. (5) Order: Gatehouse keeps its priority; grid starts in step with the importer (decision 41), and the importer's layout mapping follows the export slice if it can. | proposed (awaiting the owner)

**`docs/prd.md`**, a requirement after FR-4 (named FR-4a so as not to take a number that FR-31 onward may need): "FR-4a Grid layout: columns and rows of fixed, fill and hug tracks, a cell and spans for a child, exported as CSS grid | P2, after W1 | Not started".

**`docs/ui-design.md`**, in 3.1a the row "Grid layout (flow "grid", columns, rows)" keeps its text, and its last column, which ends with "(#271)", gains "E3.6, `docs/grid-layout.md`"; when slice 7 lands, 3.1 Layout reads "Direction (free, row, column, grid); for a grid: gap, padding, align and justify as the place of a child in its cell, the Columns and Rows lists (kind and size of each track), and for a child of a grid the Cell group".

**`docs/roadmap.md`**, a row in the frontier: "Approve `docs/grid-layout.md` and its open decisions (section 15) | decision (owner) | the grid decision above (42 when written), issue 271".

**`GLOSSARY.md`**, three terms in Structure:

> **Grid**: A frame whose children sit in the cells of its columns and rows. _Avoid_: Table, layout grid
> **Track**: One column or one row of a grid, with a kind (fixed, fill or hug) and a size. _Avoid_: strip
> **Cell**: The place of one child in a grid: a column and a row, or none and then the next free place, and how many columns and rows it spans. _Avoid_: Area. ("Slot" stays for the place a drop would put a node, as in `drop_slot`.)

**`README.md`**, one line in the Docs list after the HTML import line: "Grid layout, plan for an epic box: [docs/grid-layout.md](docs/grid-layout.md)."

**`docs/import-html.md`**: the rows of section 13, and the counts of its 3.8. Its text of 24 Nov 2026 (decision 41) keeps section 3.4's grid row as a refusal until slice 3 of grid has landed.

