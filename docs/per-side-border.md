# Per-side border: contract and model proposal

Status: **proposal, awaiting the owner**, for GitHub issue 270 (decision 40, item 2, in `docs/decisions.md`). Written 2026-11-24 from a reading of the repository. No code was written for it and nothing in the model, the canvas or the export changes with this note. The only numbers in it are a few CSS facts that were measured by hand in one Chromium (appendix at the end); nothing was run in the repository. Section 12 lists what was not verified. **Owner sign-off: pending.**

Decision 40 keeps per-side border out of W1: the border is one width, colour and position, and the fidelity contract (`docs/fidelity-contract.md`, section 4.3) lists a per-side border as unsupported. It says a contract change comes first, then model, canvas, export and oracle. This note is that contract change written out, with the model options, the geometry that decides what the contract can promise, and a slicing. The words are those of `GLOSSARY.md`.

## 1. Why a per-side border is not in the model today

What the code says today:

- `Border` (`crates/core/meldframe-protocol/src/decoration.rs`) has one `width` (0 to 100,000 px, 0 means no border), one `color` and one `position` (`BorderPosition`: inside, center, outside). A patch replaces the whole border (`NodePatch.border`, `patch.rs`), and a member left out takes its default.
- `meldframe-render` turns it into `PaintedBorder` (`resolve.rs`, `painted_border`): one width, one colour with its alpha, one position. It is none for a width that is not above 0, a colour that is not hex, or a text.
- The canvas draws it as a ring, the outer shape with the inner one cut out (`packages/web-paint/src/border.ts`, `borderRing`; `apps/web/src/canvas/stage.ts`, `updateBorder`), with Pixi `roundRect`, which takes one circular radius.
- The export writes it as paint only, an `::after` on the box (`crates/features/export/export/src/decl.rs`, `after`): `inset` of 0, `-w/2` or `-w`, `border: <w>px solid <colour>`, and a `border-radius` that grows by the share of the width outside the box. One width is written in the `border` shorthand.
- The panel has a width field, a colour field and a position control (`packages/web-panels/src/BorderSection.svelte`), and the app layer has `BorderWidth`, `BorderColor` and `BorderPosition` (`crates/core/meldframe-app-model/src/property.rs`), each of which sends one `UpdateNode` with the whole border.

The contract row for the deferral is "inner shadows, per-side border, dashed border: unsupported, deferred in M3A" (section 4.3). I found no written reason beyond that word. Decision 40 gives one: the export "writes the border as one `::after` ring that cannot take four widths". That is true of `after()` as written, but not of the pseudo box: an `::after` takes `border-width: t r b l` like any box (section 7 and the appendix). What does not stretch to four widths today is the canvas ring (one circular radius), the `Border` value, the check, the panel and the importer's rows. The cost is in those places, and in the corner geometry of section 4.

Padding already did this. ADR-0013 (`docs/adr/0013-model-round-2-additive-fields.md`) added `padT`, `padR`, `padB` and `padL`, each optional and falling back to `padding`, so an old file lays out as before. The proposal below follows that pattern.

## 2. What CSS gives, and the subset worth modelling

CSS gives, for each of the four sides of a box, a width, a style (`solid`, `dashed`, `dotted`, `double`, `none` and others) and a colour, one `border-radius` per corner (each corner may be elliptical, `h / v`), and a join at each corner where two sides meet. A real CSS border also takes room: it is taken out of the content box of a border-box box and moves children. The model's border is paint only (contract row `border`), and that stays.

| CSS | Worth modelling now | Why |
|---|---|---|
| Width per side | **yes** | One side only is a divider, an underline or an accent bar. Unequal sides are the card with a heavier bottom. The corner geometry for one colour is a ring, which the canvas and the export already draw (section 4). |
| Colour per side | **no, not yet** | Two sides of different colour meet in a diagonal join whose pixels the contract cannot promise, and the canvas would need a cut per side (section 3, option B; section 4.3). Every side that has a width can share one colour. |
| Style per side (`dashed`, `dotted`, `double`) | **no** | Already outside the contract ("dashed border", section 4.3); it needs a stroke model the canvas does not have. |
| Position per side | **no** | One position (inside, center, outside) for the whole border keeps the ring a single shape (section 4). |
| Radius per corner | **no** | Already unsupported (section 4.3, "per-corner radius"); a separate change. The geometry of section 4 is written so that it would take per-corner radii without change. |
| `border-image`, `outline` | **no** | Outside the contract. |

## 3. Model shape: options

The three shapes the issue names, checked against the code. "Old files" means a file stored before the change; "old builds" means a build from before it.

| Option | Shape | Old files | Canvas | Export | Panel |
|---|---|---|---|---|---|
| **A. Four widths, one colour** (recommended) | `Border` gains `widthT`, `widthR`, `widthB`, `widthL`, each optional; a side without its own value takes `width`. `color` and `position` stay single | Read the same: no member, all sides equal `width` | One ring, outer shape less inner shape, with elliptical corners (section 6) | One `::after`, `border-width: t r b l` (section 7) | One field per side beside the existing all-sides field, as padding has |
| **B. A width and a colour per side** | A, plus `colorT`, `colorR`, `colorB`, `colorL`, each optional and falling back to `color` | Read the same | Four regions, each the ring cut by the join line of two sides; a stencil or polygon per side; the join pixels are the browser's choice (section 4.3) | Same `::after` with `border-color: t r b l`; the join is drawn by the browser | Four colour fields (a Fill-like field each) |
| **C. A separate `borders` list** | A new `borders: [{sides, width, color, position}]`, at most N, beside or instead of `border` | Needs both fields kept in agreement (the `fillMode` objection of ADR-0013), or replaces `border` and bumps `SCHEMA_VERSION` to 2 | A ring per list entry; stacking order to decide | One `::after` and one `::before` is all the pseudo boxes there are; a third entry needs an extra child, which breaks "the DOM equals the node tree" (contract, open question 6) | A list like the shadow list, with its add and remove buttons |

**Recommendation: option A.** It is additive (ADR-0013), it holds the cases the issue and the panel name (divider, underline, accent bar, a heavier bottom) with one colour, its export is the form that is already verified for the uniform case with one more property, and its geometry is a ring, not a set of joins. B and C stay possible later because A does not close them: B adds four optional members and C would be its own field. Whether one colour is enough for the files people draw is not known; no count of files, users or designs was made for this note, and none is claimed.

### 3.1 The shape of A

```json
{ "border": { "width": 0, "widthL": 4, "color": "#6366f1", "position": "inside" } }
{ "border": { "width": 0, "widthB": 1, "color": "#e4e4e7" } }
{ "border": { "width": 1, "widthB": 4, "color": "#18181b", "position": "outside" } }
```

The first is an accent bar on the left, the second a divider, the third a thin border with a heavy bottom. The rules:

- The width of a side is its own member, or `width` when it has none (the same rule as `Node::pad` for padding). A helper next to `Border` returns the four, as `Node::pad` returns `Pad`; the existing `Pad` type could be reused or generalised, a naming choice for the slice.
- `width` stays the width of the sides that follow it. A writer that wants a single side sets `width` to 0 and that side's member. This is the convention the panel and the importer would use. It also makes a consumer that has not learned the sides yet draw nothing, not a wrong ring on all four sides, and the export says so (section 11, slice 1).
- There is **no border** when every effective side is 0 (today: when `width` is 0). Text still paints none.
- A patch still replaces the whole `border`, so no three-state member is needed (padding needed one only because it has a single `padding` beside the sides; here the sides live inside the value that is replaced). The existing trap stays: a patch that sets only `color` resets `width`, the sides and `position`. The tool text has to say so (section 10).
- The all-sides `BorderWidth` property sets `width` and clears the four sides, as `PaddingAll` does for padding. Today's behaviour is unchanged because no side exists yet.

### 3.2 Compatibility

- **Old files.** The members are `#[serde(default, skip_serializing_if = "Option::is_none")]`, as `padT` is, so a stored border without them reads the same and serialises the same. The test `old_node_without_optional_fields_is_valid_and_gets_defaults` in `crates/core/meldframe-protocol/tests/wire_compat.rs` is the pattern; a stored border must stay byte for byte as it is when no side is set.
- **`SCHEMA_VERSION` stays 1.** `version.rs` says additive optional fields do not bump it, and ADR-0013 says the same.
- **Old builds reading a new file.** `Border` and `Node` have `deny_unknown_fields`, so a Rust build from before the change refuses a file that holds `widthL`. ADR-0013 says an old front end "only loses what it never drew"; for the Rust side that is not what the code does, and I did not check the TypeScript side. It is the same for every additive member so far, so it is a rule of the project, not something this proposal adds; it is stated here so that the first release with sides says so.
- **Generated types.** `Border.ts` in `packages/protocol/src/generated` gains four optional members (`tools/check-protocol.sh` regenerates and checks them).
- **Undo.** `border` is already in `patch_fields!` (`crates/core/meldframe-undo/src/patch_fields.rs`) and the whole `Border` is the unit, so an undo restores the sides without a change there.

### 3.3 Validation limits

- Each side: a finite number from 0 to `max_size` (100,000, `meldframe-check/src/limits.rs`), the limit `border.width` has. The error names the member, as `border.width` does today: `border.widthT`, `border.widthR`, `border.widthB`, `border.widthL` (`check_border` in `crates/core/meldframe-check/src/validate.rs`, two lines each).
- No limit on the sum of the sides: a box smaller than its borders is already handled by the CSS rule the canvas follows (section 4, rule 5).
- Border widths are plain numbers; a variable cannot be bound to them (only `fill`, `radius` and `opacity` are bindable, `coverage.rs`), and the colour takes a literal only. Unchanged.
- No quota changes. A border is one field of a node, not a list.

### 3.4 The Command and the op

No new `Command`. A change is the `UpdateNode` that exists, `{type: "updateNode", id, patch: {border: {...}}}`, as the panel's `borderWidth` already sends it through `crates/core/meldframe-app-parts/src/property.rs`. An op carries the same command, so the gate, the log, review, revert and undo need no change. The app layer gets one more property (`BorderSide {side, value}`, reusing `Side` of `meldframe-app-model`, as `Padding {side, value}` does) that builds the same patch (section 9).

## 4. Position, unequal sides and corner radius

This is the hard part. The rules below are what the canvas and the export would both implement. They are the current uniform rules written per side; with equal sides they give the current output exactly.

Notation: the box is `w` x `h`, its radius is `r = min(radius, w/2, h/2)` (as the canvas and `after()` clamp it), the four widths are `t`, `rt`, `b`, `l` (top, right, bottom, left; `rt` is written for the right side to keep it apart from the radius). For each side, `g` is how far the ring reaches **out of** the box on that side: 0 for inside, half the width for center, the whole width for outside.

1. **Outer shape.** The box grown by `g` on each side, so it starts at `(-gl, -gt)` and is `w + gl + gr` by `h + gt + gb`. Each outer corner is elliptical, with the reach of the two sides that meet there added to `r`, horizontal then vertical: top left `(r + gl, r + gt)`, top right `(r + gr, r + gt)`, bottom right `(r + gr, r + gb)`, bottom left `(r + gl, r + gb)`. A corner with `r = 0` stays square (a square corner is not made round by a border, as in the uniform rule).
2. **Inner shape.** The outer shape inset by the four widths. Each inner corner radius is the outer one less the width of the side it is measured against, per axis, never below 0: top left `(max(0, r + gl - l), max(0, r + gt - t))`, and likewise for the others. For inside, `g` is 0 and this is `r - l` by `r - t`; for outside it is `r` by `r`, which is the box's own corner, so the ring hugs the box; for center it is `r - l/2` by `r - t/2`.
3. **The ring** is the outer shape less the inner one, in one colour. With one colour there is no join to draw: the four sides are parts of one ring.
4. **A side of width 0** adds nothing on that side and does not move that edge of the inner shape. A bottom border on a rounded card is a crescent: full width at the bottom middle and thinner up the two bottom corners, down to 0 where the corner arc leaves the bottom band. This is what CSS draws too (appendix, measurement 5).
5. **A box smaller than its borders.** CSS makes a box at least as wide as `l + rt` and as tall as `t + b`, from the top left (the uniform rule is "at least twice the width on each axis" in `borderRing`). The same rule per axis: the outer width is `max(w + gl + gr, l + rt)` and the height `max(h + gt + gb, t + b)`, and the inner shape is empty when its width or height is not above 0. Measured in Chromium for a 20 px tall box with a top and a bottom width of 15: the ring is 30 px tall from the top (appendix, measurement 4).
6. **Ellipse.** The export writes `border-radius: 50%` for an ellipse; that form stays right for a uniform ring (outer semi-axes grow by the width, the inner edge is the node's ellipse). It is **not** right for unequal sides: with 50% the horizontal radius of both left corners is half of the grown box, so the inner edge is not the node's ellipse. The export would write the four corners in px, `(w/2 + gl)` and so on, which is rule 1 with `r` taken as the half sizes. The canvas draws an ellipse as a pill (contract, row `border`, difference 3), so for a box that is not a circle canvas and export differ already today, and they would differ the same way for unequal sides; for a circle they agree.

### 4.1 What the contract could promise

- Every side's width and the ring's outer and inner edges as rules 1 to 3 say, in one colour, with solid areas probed away from edges (the tolerance of L3 stays: 40/255 per channel at points away from edges, anti-aliasing not promised).
- The bands of straight edges are exact at their midpoints for every position, with or without a radius, and a side of 0 shows nothing.
- The border takes no room: no box in the export moves, so L1 (geometry, 0.05 px) is unchanged. The `::after` is not a `[data-node]` box and the oracle reads only those.
- The corner shape of rules 1 and 2, probed at points about 2 px or more from the arc (an elliptical inner corner against a circular one is a visible difference, section 4.3).

### 4.2 What it could not promise

- Pixels on the arc, and any join between two sides of different colour (not in option A, and why B is not recommended).
- A border wider than half the box on one axis (rule 5) beyond what unit tests and the CSS rule give: the contract already says this for the uniform border (row `border`, point 4: "follows the CSS rules and unit tests, no browser probe covers it yet"); the per-side form inherits that sentence, and it is silent on the cases a side of 0 adds.
- An ellipse that is not a circle (rule 6), as today.
- The opacity difference of the row `border` (point 2), the clip difference (point 1) and the rest of that row: unchanged and still listed there.
- Corners where the radius is larger than half a side after the borders are applied are not specified by the contract (it is silent on radius above half the size, section 4.1); rules 1 and 2 never let the sums of radii pass the box, because `r` is clamped first, but this was not probed.

### 4.3 What was measured

In one Chromium (appendix): the inner corner of a real CSS border with unequal widths is elliptical (measurement 1); the `::after` of the export takes four widths and, with elliptical outer radii `r + l` by `r + t`, its inner edge matches the box's own circular corner (measurement 3); with one circular outer radius instead (the widest side added to `r`) the two shapes differ in 178 pixels of a 60 x 60 corner by more than 8/255 (measurement 3, circular variant), so the elliptical form is needed, not a refinement. For two colours, the join is the straight line from the corner of the box to the inner corner of the padding box, the same with a radius (measurement 2); that is what the canvas would have to copy for option B, and the line is anti-aliased over about two pixels, which no probe at 40/255 would pin.

## 5. Contract rows to add or change

In `docs/fidelity-contract.md` (not edited here; proposed wording in the pull request that carries the contract change, slice 0 of section 11):

| Where | Today | Proposed |
|---|---|---|
| Section 2, L3 paint (solid) | "borders (inside/center/outside)" | "borders (inside/center/outside, each side with its own width)" |
| Section 4.2, row `border` `{width, color, position}`, uniform | One row, supported | Keep it for the equal-sides form, unchanged byte for byte. Add its generalisation in the next row |
| Section 4.2, new row `border` with `widthT`, `widthR`, `widthB`, `widthL` | none | CSS: `::after{position:absolute; inset:-gt -gr -gb -gl (0 inside); border-style:solid; border-color; border-width:t r b l; border-radius: <h h h h> / <v v v v>}`. Tailwind: arbitrary properties on `after:` (`after:[border-width:4px_12px_4px_12px]`). Status **planned (model)**, then **supported** with its oracle fixture. Notes: the rules of section 4 of this note; the differences of the row above apply; the ellipse is written in px, not `50%`, when the sides differ; the corners of a side of 0 |
| Section 4.3, "inner shadows, per-side border, dashed border" | unsupported, deferred in M3A | "inner shadows, dashed border"; per-side border leaves the list |
| Section 4.3, new line | none | "per-side border colour, border styles other than solid: unsupported until proposed" (the join of two colours is not promised) |
| Section 7, "The canvas border is checked against the model, not against the export" | inside, centre, outside, radius, ... | add unequal sides, a side of 0, and the elliptical corner to the pixels `border-e2e.mjs` reads |
| Section 7, paint probes list | "border inside, center and outside" | add "border sides" (the fixture of section 8) |
| Section 11, question 6 (border output: `::after` or an extra child) | open | Answer it as `::after`: it takes four widths, so the DOM stays equal to the node tree |

Where the contract is silent today and this proposal would have to decide it: the inner shape of a border with unequal sides (rule 2), the shape of a side of width 0 on a rounded box (rule 4), the size rule of a box smaller than its borders per axis (rule 5), and the ellipse with unequal sides (rule 6). Section 4.1's radius text ("radius above half the size was not tested") stays true.

## 6. The canvas form

`packages/web-paint/src/border.ts` is pure and is where the change is. Today `SceneBorder` has one `width`, `borderRing` returns two `RingShape`s with one radius `r`, `borderReach` returns one number and `sameBorder` compares one width. For option A:

- `SceneBorder` carries the four widths (`top`, `right`, `bottom`, `left`) instead of `width`; `sameBorder` compares four; `packages/web-link/src/scene.ts` (`toSceneNode`) copies them from `PaintedBorder`, which `meldframe-render` fills from the effective widths (the canvas never sees the fallback rule).
- `borderRing(w, h, radius, b)` returns `outer` and `inner` with rules 1 and 2: a `RingShape` with a horizontal and a vertical radius per corner (`rx` and `ry`, four each) instead of one `r`, and rule 5 for the sizes. With equal sides it returns today's shapes, so the unit tests in `border.test.ts` that pin them stay.
- `borderReach(b)` becomes per side (the reach on each of the four sides: the width for outside, half for center, none inside). It is used by `paintedRect` in `packages/web-canvas/src/scene.ts` to size a capture, so a capture of a node is the node grown by its own reach on each side, no longer a square band. The tool text says "larger than the node by the band of its outside border" and stays true.
- `stage.ts` (`updateBorder`) draws the two shapes. Pixi `roundRect(x, y, w, h, r)` takes one circular radius, so a shape whose radii are not all equal needs a path of four lines and four quarter ellipses (`moveTo`, `lineTo` and `bezierCurveTo` with the usual quarter-ellipse constant 0.5523; a cubic in place of an exact ellipse, a standard approximation off by a few hundredths of a percent of the radius, not measured here). When all eight radii are equal the code keeps `roundRect`, so the uniform border is pixel for pixel what it is today. Whether Pixi's fill and `cut()` handle the path as they handle `roundRect` is not verified (section 12).
- Interaction, hit testing and the selection overlay do not read the border and are unchanged. The border still lies over the node and its children and is not cut by the node's own `clip`; opacity still fades it part by part. All differences listed in the row `border` stay.
- Size: `border.ts` is 61 lines and `stage.ts` 370; the change is inside the 500 line ceiling of both.

## 7. The export form, HTML and Tailwind

The current form (`decl.rs`, `after`; Tailwind in `tw.rs`) writes, on the `::after` of a `position: relative` node:

```
content:""; position:absolute; pointer-events:none; inset:<0 | -w/2 | -w>; border:<w>px solid <colour>; border-radius:<r + grow>
```

and in Tailwind `after:content-['']`, `after:absolute`, `after:pointer-events-none`, `after:inset-[..]`, `after:[border:3px_solid_#ff0000]` and `after:rounded-[9.5px]` (tests in `crates/core/meldframe-export-tests/tests/paint.rs`). For unequal sides:

```
content:""; position:absolute; pointer-events:none;
inset:<-gt -gr -gb -gl>;
border-style:solid; border-color:<colour>; border-width:<t rt b l>;
border-radius:<rh1 rh2 rh3 rh4> / <rv1 rv2 rv3 rv4>
```

- **Equal sides keep today's text byte for byte** (including the `border:` shorthand), so every existing test, fixture and exported file is unchanged. Only a border with unequal sides takes the new text.
- **Not a real CSS border.** A border on the node itself would take room and move children, the reason in the contract row for not using it. The `::after` stays (open question 6).
- **Radius.** The four corners are written in px, horizontal values then vertical, as rule 1 says. A radius bound to a variable keeps its form, `calc(min(var(--x) * 1px, w/2, h/2) + grow)`, with a different `grow` per corner axis. An ellipse gets px values from its size (rule 6), where it now gets `50%`. `rect` in `after()` is the core's rect, so the size is known.
- **Tailwind.** `tw.rs` already turns any declaration it has no utility for into an arbitrary property, `[border-width:4px_12px_4px_12px]`, so `border-style`, `border-color`, `border-width` and a four-value `inset` need no change. `border-radius` is mapped to `rounded-[..]`; whether Tailwind v4 accepts the slash form inside that bracket was not tried (no compiler is installed here). If it does not, one line in `tw.rs` sends the slash form to `[border-radius:..]`. The oracle compiles the Tailwind form with the Tailwind v4 compiler and requires the same pixels as the CSS form (f07 is the precedent), so this is settled by the oracle fixture of section 8.
- **Reports.** `border_clipped` (a border outside or centred on a frame that clips) stays, now raised only when a side that lies outside has a width above 0. A text with sides still reports `on_text`. Two places read `n.border.width > 0.0` directly (`decl.rs`, the `border` flag at the top of `style` and the `on_text` list) and have to read the sides.
- **File size.** `decl.rs` is 485 lines, under the 500 ceiling by 15. The new code goes into a new file in the crate (`ring.rs`, with `after()` moved there), not into `decl.rs`; the crate is 1,578 lines, inside the 1,500 to 2,500 band that `AGENTS.md` aims for (the hard ceiling is 4,000).

### 7.1 The round trip with the importer

`docs/import-html.md` is a spike note and the importer does not exist, so nothing breaks; what would change in its mapping table (section 3.6, the border rows) if both land:

- The row "`border` with equal `width`, `style: solid` and `color` on four sides" stays, as **approximated** (the width is added to the padding).
- A new row: widths that differ on `border-top`, `border-right`, `border-bottom`, `border-left`, all `solid`, with one colour on the sides that have a width, become `border {width: 0, widthT..widthL for the sides that have one, color, position: inside}`, and each side's width is added to its own padding (`padT` to `padL` exist). **Approximated**, as the uniform row is. A `border-left` alone is the accent bar of section 3.1.
- Sides of different colour: **approximated** with the colour of the widest side (the first of top, right, bottom, left on a tie), counted in the report; today they are refused. This is the loss that option A accepts.
- The row for per-side borders that differ is refused today ("Per-side borders that differ; dashed, dotted, double; border-image; outline"): it would shrink to "dashed, dotted, double; border-image; outline".
- The export's own form (the row "The export's border: `[data-node="x"]::after{...}`") would have to read `border-width` with four values and an `inset` of four values, and the slash form of `border-radius`; the position is recovered from the insets (all 0, each minus half its width, or each minus its width); an inset that mixes the three is not something the export writes and is not read as a border. That is the round trip of the export, and a property test of the importer (a generated border with sides, exported, imported, equal) would cover it.
- The row for `box-sizing: content-box` ("`w`, `h` grown by padding and border") grows the width by `l + rt` and the height by `t + b`.

## 8. What the oracle needs

`tools/fidelity-oracle` checks the export, not the canvas (the contract says so in section 7). For sides:

- **A fixture**, `f24-border-sides`, in `fixtures.mjs` beside `f07-decor`, on a white stage, with `bd`-style helpers extended for sides. Boxes of 120 x 80: a left accent only (`width` 0, `widthL` 6); a bottom divider only; unequal sides on a square box and on a box with a radius, inside; the same centred; the same outside; a rounded card with a bottom border only (the crescent); a circle with unequal sides; a frame that is an auto layout row with sides, to check the border moves nothing. Geometry (oracle A) is checked on every fixture for free: if the ring moved a box, the 0.05 px gate would fail.
- **Paint probes** in `probes.mjs`, in the `CASES` table, as `['f24-border-sides', {width, height}, [[what, x, y, colour], ...]]`: the midpoint of each side's band (exact colour), a point beyond the outer edge (white), a point on the fill after the band, and for the rounded boxes two points on either side of the inner corner arc, at least 2 px from it. A hand-worked example for a 120 x 80 box at (40, 40), radius 20, widths top 4, right 12, bottom 4, left 12, inside, red ring on green: the inner top left corner is an ellipse of radii (8, 16) centred at about (60, 60); the page point (53, 47) is outside it (ring, red) and (56, 51) is inside it (fill, green). These numbers are worked by hand and are to be re-checked when the fixture exists.
- **Sensitivity cases** in `sensitivity.mjs` (`PAINT_MUTATIONS`), so the probes are shown to fail on a wrong export: "the sides are written as one width" (the `border-width` text replaced by its largest value), "the corner radius becomes circular" (the slash form replaced by one radius, the 178 pixel difference above says a corner probe will see it), and "left and right are swapped". Each must change both forms, or the tracker of that file says the search string is missing.
- **Known gaps** (`known-gaps.mjs`): none expected. The ellipse that is not a circle is a canvas against export difference and the oracle does not read the canvas, so it is not a geometry gap of the export; the contract row lists it.
- **The fuzz generator** makes no borders (contract, section 7), so it does not change.
- **The canvas** is checked by `packages/web-e2e/scripts/border-e2e.mjs` (hand-computed colours of a real Chromium canvas, run by hand) and by the unit tests of `border.ts`; both would gain the same boxes.

## 9. The Properties panel and the app layer

`docs/ui-design.md` section 3.1a lists "Border per side, and the sides field" as not in W1, "a contract change first, then model, canvas, export and oracle". The panel follows the Layout section's padding, which is the precedent (`packages/web-panels/src/LayoutSection.svelte`):

- The **Border width** field stays and means all sides. It shows the value the four sides share, and is empty with the placeholder "Mixed" when they differ (the app layer's `paddingUniform` is the pattern; a `borderUniform` of the same kind would be added to `Properties` in `meldframe-app-parts/src/properties.rs`). Committing it sends `BorderWidth`, which sets `width` and clears the four sides.
- A **Sides** control beside it (a button that opens four fields, top, right, bottom and left, in the order of the padding fields, with `prop-border-top` and so on as test ids). It opens by itself while the sides differ. Each field sends the new `BorderSide {side, value}` property; the page keeps no state for it beyond whether the control is open, which is view state, as a collapsed section is.
- The position control shows while any side is above 0 (today: while the width is above 0); the colour field is unchanged and applies to every side that has a width.
- A width of 0 on every side is still "no border", through the same commands an agent sends. The visibility switch and the Remove button stay out (decision 40, items 3 and 6).
- The pure functions in `packages/web-document/src/decoration.ts` get `borderSideAction`; `borderWidthAction` compares with the uniform value, not `p.border.width`.
- A panel control and the tool share one command: `BorderSide` builds `UpdateNode` with `patch.border` in `meldframe-app-parts/src/property.rs`, which is the patch an agent sends with `apply_commands`.
- `docs/ui-design.md`, the Border row of 3.1, and the 3.1a row would be rewritten when the panel slice lands (section 11, slice 4), not before.

## 10. Tool text for agents

The tool text of `apply_commands` (`crates/core/meldframe-mcp-defs/src/lib.rs`, listed in `docs/mcp-tools.json`) names the fields of `addNode` and `updateNode` (`padT`, `gradient` and others) but **names no `border` member and no `shadows`**; an agent learns the border from `get_nodes`. That gap is worth closing with or before this change. Proposed wording to add after `opacity`:

> border {width (px, the width of every side that has no value of its own; 0 for none), color (#hex), position (inside, center or outside), widthT, widthR, widthB, widthL (px, one side each, in place of width for that side)}; a border is paint only and takes no room, so add padding if the children must keep clear of it; updateNode replaces the whole border, so send every member you want to keep.

The `export_frame` text lists what `unsupported` carries; with slice 1 it also lists a border with sides drawn at its `width` on every side until slice 3. `skills/meldframe/SKILL.md` (checked by `crates/core/meldframe-mcp-tests/tests/skill.rs`, which cross-checks every tool and code it names and the "canvas does not draw" sentence) would get a short example, a divider as `border {width: 0, widthB: 1, color: "#e4e4e7"}`, and the sentence that a side is not drawn on the canvas until slice 2 if that is when it is shipped. `docs/mcp-tools.json` is compared with the tool text by a test, so both change together.

## 11. Recommendation, first slice, slicing

**Adopt option A**, in six steps, each its own pull request, each one kind of content and under the ceilings of `AGENTS.md` (15 files, 1,000 changed lines; the line counts below are estimates from the files and the size of earlier slices, not measurements).

| Slice | Content | Files | Changed lines (estimate) |
|---|---|---|---|
| 0 | **Docs**: the contract rows of section 5; decision 40 item 2 (the reason corrected) and its status; `docs/ui-design.md` and `docs/import-html.md` wording; a box in `docs/epics.md` | 5 | 120 to 200 |
| 1 | **Model, check, render model**: the four members of `Border` and the effective widths helper, `check_border`, `PaintedBorder` with the four widths (its `width` keeps meaning what it means now), `painted_border` (none when every side is 0), the generated `Border.ts`, and the `export` report that says a border with sides is drawn at its `width` on every side | 8 to 10 | 250 to 400 |
| 2 | **Canvas**: `border.ts` rules 1, 2, 5, per-side reach, `web-link` mapping, `stage.ts` path, unit tests, `border-e2e.mjs` probes | 6 to 8 | 350 to 500 |
| 3 | **Export and oracle**: `ring.rs` (moved `after()` plus the sides), the ellipse in px, the two reads of `border.width`, `f24-border-sides`, probes, sensitivity cases, the contract rows turned to supported | 8 to 10 | 500 to 800 |
| 4 | **Panel, app layer, tool text**: `BorderSide`, `borderUniform`, the Sides control, `borderSideAction`, the tool text, the skill, `mcp-tools.json`, the Border rows of `ui-design.md`, the panel e2e | 8 to 12 | 400 to 600 |
| 5 | **Importer rows**, only when `import_html` exists (the rows of section 7.1 and the property test); not part of the first release of sides | | |

Between slice 1 and slice 3 a file can hold sides that nothing draws as sides; ADR-0013 allowed that for E3.2 ("drawn on the canvas and exported as unsupported" until the oracle probe), and slice 1 makes the export say so. If the owner prefers that no such state ever ships, slices 1 to 3 can be one series of commits reviewed together (about 1,100 to 1,700 lines with tests, above the 1,000 line ceiling, so it would need the written reason `AGENTS.md` asks for); the order below is the one that keeps each pull request reviewable.

### 11.1 The first build slice and its failing test

**Slice 1.** Size: 8 to 10 files and 250 to 400 changed lines with tests (estimate). It is behaviour and so test first, at the seams that exist:

1. The first failing test is in `crates/core/meldframe-core/tests/decoration_check.rs`, beside `border_width_and_colour_are_validated`: a border `{"width": 0, "widthT": 4}` is accepted, `{"widthT": -1}` and `{"widthL": 100001}` are refused and the message names `border.widthT` and `border.widthL`. It fails today because `Border` has `deny_unknown_fields` and no such member, which is the failure that proves the field is new.
2. The test that carries the behaviour is in `crates/core/meldframe-render/tests/resolve.rs`, beside `a_border_is_painted_with_its_width_colour_alpha_and_position`: `{"width": 0, "widthL": 6, "color": "#ff0000"}` paints a border with the four widths `(0, 0, 0, 6)`; `{"width": 2, "widthB": 5}` paints `(2, 2, 5, 2)`; `{"width": 0}` and `{"width": 0, "widthT": 0}` paint none; a text paints none.
3. The compatibility test is in `crates/core/meldframe-protocol/tests/wire_compat.rs`: a border read from `{"width":2}` serialises without any side member, and a border with sides round trips.
4. The export test is in `crates/core/meldframe-export-tests/tests/paint.rs`: a border with sides is reported unsupported and the `::after` is written as for its `width`.
5. `meldframe-render/tests/coverage.rs` and `meldframe-export-tests/tests/coverage.rs` need no new row (the table lists `Node` fields; `border` is one), and must still pass; they change only if the owner wants a nested row.

Nothing in slice 1 reaches the canvas or the panel, and no tool text names the new members yet, so nobody is asked to use a field that is not drawn.

## 12. What was not verified, and open decisions

Not verified:

- **Pixi.** The quarter-ellipse path of section 6, and that `fill` then `cut()` work on it as on `roundRect`, were not tried. A small unit test of the path points and `border-e2e.mjs` pixels settle it in slice 2.
- **Tailwind.** The Tailwind v4 compile of `after:[border-width:..]`, a four-value `inset` and the slash radius was not run (no installed compiler here, and `npm ci` needs the network). The oracle does it in slice 3.
- **The oracle and the tests were not run**; no Cargo command was run for this note. The line counts and file counts of section 11 are estimates.
- **The CSS facts** of the appendix come from one system Chromium (153.0.8010.12), by hand, one run each, not the Playwright build the oracle pins (Playwright 1.63.0). The contract's own experiments also used Chromium 153.
- **The specification** was not re-read; statements about CSS are what that browser drew.
- **The TypeScript side of an old front end reading a new file** (section 3.2) was not checked.
- Whether one colour per border is enough for the files people draw: no data exists and none is invented here.

Open decisions for the owner:

1. Option A (four widths, one colour, one position), with B left for later. B would add four colour members to the same value and a per-side cut to the canvas.
2. The writer convention that `width` is 0 and the sides carry the widths (section 3.1), against `width` as the largest side.
3. Sides of different colour on import: approximated with the widest side's colour, or refused (section 7.1).
4. Whether slices 1 to 3 may ship separately, with the export saying a border with sides is drawn at its `width`, or reviewed together.
5. Whether per-corner radius, which section 4 is written to take, is the next change.

## Appendix: how the CSS facts were measured

One Chromium, 153.0.8010.12 (the system `google-chrome`), driven with Playwright, device scale factor 1, `*,::before,::after{box-sizing:border-box}` and a white page, pixels read from a PNG of the page. One run each, by hand, not kept in the repository. In each case a box is placed at the given position.

1. **Inner corner with unequal widths.** A box at (20, 20), 100 x 60, `border-radius: 20px`, `border: solid #f00`, `border-width: 4px 12px 4px 12px`. The inner top left corner should be an ellipse of radii (8, 16) centred at (40, 40); its 45 degree point is (34.3, 28.7). The pixel (34, 28) is a mix (255,130,130), (33, 27) is red and (35, 30) is white. A circular corner of radius 8 or 16 would have put (34, 28) wholly inside or outside.
2. **The join of two colours.** A box at (20, 20), no radius, `border-width: 10px 0 0 30px`, top red, left blue. On the pixel row 5 px below the top, the left is blue to x = 15 and red from x = 17 (a mix at x = 16): the diagonal from the box corner (0, 0) to the inner corner (30, 10), which at y = 5.5 is x = 16.5. With `border-radius: 20px` the same row and the column 10 px from the left give the same line (blue to x = 15, mix at 16 and 17; down the column, a mix at y = 3 and 4: 10.5 x 10 / 30 = 3.5).
3. **The `::after` of the export with unequal sides.** A blue node at (40, 40), 100 x 60, `border-radius: 20px`; `#n::after{content:""; position:absolute; inset:-4px -12px -4px -12px; border:solid #f00; border-width:4px 12px 4px 12px; border-radius:32px / 24px}`. The ring's inner edge follows the node's corner arc: (47, 47) and (46, 47) are blue, (45, 45) is red, where the circular arc of radius 20 centred at (60, 60) is at about (45.9, 45.9). The same with `border-radius: 32px` (one circular radius, `r` plus the widest side): 178 pixels of the 60 x 60 corner (20 to 80) differ from the first by more than 8/255, at most by 255.
4. **A box smaller than its borders.** A blue node at (40, 40), 100 x 20; `::after{position:absolute; inset:0; border:solid #f00; border-width:15px 0 15px 0}`. Down the column x = 90, red covers y = 40 to 69, 30 px from the top of the node, not 20.
5. **A side of 0.** A blue node at (40, 40), 100 x 60; `::after{inset:0; border:solid #f00; border-width:0 0 6px 0}`. Down the column x = 60, blue is y = 40 to 93 and red y = 94 to 99; the rows beside the box are white. (The crescent of rule 4 follows from measurement 1's corner rule; it was not probed on a rounded box.)
