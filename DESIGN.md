# Design

Status: draft for owner review (16 Nov 2026). This is the design file that the web tokens are compared with, and the Rust ones will be (rule U4 in `docs/standards/modular-thin-ui.md`). It holds the values and the rules; how the editor behaves is in `docs/ui-design.md`; product context is in `PRODUCT.md`.

"UI token" in this page means a value of the editor's own interface (a colour, a size, a duration). It is not a Variable: Variables belong to the person's design system and are described in `docs/tokens-and-variables.md`.

## 1. Direction

The category standard, played straight, at the craft level of the tools designers already know (`PRODUCT.md`). The editor is an operating surface, so scanability, consistency and native expectations outrank expression. It must not look like an AI chat with a canvas attached.

- The canvas is the window. Chrome floats over it and collapses.
- Dark is the default; light is supported with the same roles.
- The interface is neutral. Colour belongs to the design on the canvas, to the selection and to actors (the person and each agent).
- One accent blue. It marks what is selected and what is primary; it is never decoration.

## 2. How the tokens are kept honest

- UI code reads colours, font sizes and durations from the tokens module: `packages/web-tokens` for the web client (`src/tokens.ts` holds the values, `src/tokens.css` the custom properties, `src/tailwind.css` the Tailwind mapping), `crates/ui/meldframe-ui/src/tokens.rs` for Rust. `cargo xtask ui-literals` and `npm run ui-literals` reject literals anywhere else.
- The block at the end of this page is the single list of values. What is enforced today is the web side, by tests in `packages/web-tokens`: `design-file.test.ts` compares the block with `tokens.ts`, and `tokens.test.ts` compares `tokens.ts`, `tokens.css` and `tailwind.css` with each other; a drift in any of the three fails the test. `contrast.test.ts` computes the pairs of section 3.2 and scans the markup for the two known mistakes.
- The Rust side is a placeholder: `tokens.rs` holds three constants and no test compares it with this page yet. Until that test exists (rule U4 of `docs/standards/modular-thin-ui.md` asks for it) the block, `tokens.ts` and `tokens.css` are the values that count.
- To change a value, change the block, `tokens.ts` and `tokens.css` (and `tailwind.css` for a new name) together in one pull request.
- The block lists plain numbers. In `tokens.css` sizes carry `px` and durations carry `ms`, so a custom property can be used directly in a style; the tests add the unit before they compare.

## 3. Colour

Roles, not hues: components ask for `surface` or `text-muted`, never for a hex value. Each role has a dark and a light value.

| Role | Use | Dark | Light |
|---|---|---|---|
| `canvas` | The editor canvas behind frames | `#161618` | `#e9e9ec` |
| `sidebar` | Sidebar and docked bars | `#1b1b1e` | `#f4f4f5` |
| `surface` | Panels, cards and dialogs | `#1e1e21` | `#ffffff` |
| `surface-raised` | Menus, popovers and tooltips | `#26262b` | `#ffffff` |
| `field` | Input fields and segmented controls | `#26262a` | `#f4f4f5` |
| `control` | Secondary button, hover of an icon button, menu row hover | `#2e2e34` | `#ececef` |
| `control-selected` | Selected segment, pressed control | `#3a3a40` | `#e4e4e7` |
| `border` | Outlines of panels and cards | `#2e2e33` | `#e4e4e7` |
| `border-strong` | Outline of a hovered field | `#4a4a52` | `#a1a1aa` |
| `menu-border` | Outline of menus, popovers and keycaps | `#3a3a42` | `#d4d4d8` |
| `divider` | Rule between groups in a bar, default field outline | `#34343a` | `#d4d4d8` |
| `hairline` | Dividers inside a panel | `#2a2a2e` | `#ececef` |
| `text` | Body text | `#e4e4e7` | `#18181b` |
| `text-strong` | Titles and selected labels | `#f4f4f5` | `#09090b` |
| `text-muted` | Secondary text, never dimmer | `#a1a1aa` | `#52525b` |
| `text-secondary` | Body text one step under `text`, for rows and notes on a panel | `#d4d4d8` | `#27272a` |
| `text-faint` | Decorative only: line numbers, empty glyphs. Never text | `#52525b` | `#a1a1aa` |
| `icon-muted` | Chevrons and row icons; a part, never text | `#71717a` | `#71717a` |
| `accent` | Marks and fills that carry no text | `#3b82f6` | `#2563eb` |
| `accent-strong` | Primary button fill | `#2563eb` | `#2563eb` |
| `accent-hover` | Hover of a primary button (proposed) | `#1d4ed8` | `#1d4ed8` |
| `accent-pressed` | Pressed primary button (proposed) | `#1e40af` | `#1e40af` |
| `on-accent` | Text on an accent fill | `#ffffff` | `#ffffff` |
| `selection` | Selection outline, focus ring and handles | `#60a5fa` | `#2563eb` |
| `selection-text` | Text on a selected row | `#bfdbfe` | `#1e40af` |
| `selection-bg` | Selected row fill | `rgba(96,165,250,0.18)` | `#dbeafe` |
| `chip-bg` | Variable chip fill | `#1f2540` | `#e0e7ff` |
| `chip-border` | Variable chip outline | `#343d6e` | `#a5b4fc` |
| `chip-text` | Variable chip text | `#c7d2fe` | `#3730a3` |
| `person` | The signed-in person as an actor | `#6e60ea` | `#5b4bd6` |
| `agent-1` | First agent actor | `#fb923c` | `#c2410c` |
| `agent-2` | Second agent actor | `#2dd4bf` | `#0e7490` |
| `agent-3` | Third agent actor | `#c084fc` | `#7e22ce` |
| `on-agent` | Text on an agent colour | `#1c1917` | `#ffffff` |
| `success` | Success text and dots | `#4ade80` | `#15803d` |
| `warning` | Warning text | `#fde68a` | `#92400e` |
| `warning-bg` | Warning fill | `#26221a` | `#fffbeb` |
| `danger` | Error text | `#f87171` | `#b91c1c` |
| `danger-bg` | Error fill | `#2a1f1f` | `#fef2f2` |
| `danger-text` | Text on `danger-bg` and on panels where `danger` is too saturated | `#fca5a5` | `#991b1b` |
| `danger-strong` | Destructive button fill | `#b91c1c` | `#b91c1c` |
| `danger-hover` | Hover of a destructive button (proposed) | `#991b1b` | `#991b1b` |
| `danger-pressed` | Pressed destructive button (proposed) | `#7f1d1d` | `#7f1d1d` |
| `info` | Information text | `#93c5fd` | `#1d4ed8` |
| `review-bg` | Needs-review fill | `#2b2118` | `#ffedd5` |
| `review-text` | Needs-review text | `#fdba74` | `#9a3412` |
| `diff-add` | Added lines in a diff | `rgba(74,222,128,0.10)` | `rgba(21,128,61,0.10)` |
| `diff-delete` | Removed lines in a diff | `rgba(248,113,113,0.10)` | `rgba(185,28,28,0.10)` |
| `scrim` | Backdrop behind a dialog | `rgba(0,0,0,0.5)` | `rgba(0,0,0,0.35)` |

### 3.1 Actors

The person and each agent have a stable colour, used for the layers dot, the touch ring, the review tint and the Activity lane. An actor is always also named in text; colour is never the only signal. Colours that must be told apart differ in lightness as well as in hue. When more than three agents are active, the colours repeat and the name carries the difference.

### 3.2 Contrast

Text must reach 4.5:1 (3:1 at 24 px and above) and interface parts such as outlines and dots 3:1 (WCAG 2.2 AA). The pairs below are the real ones used by the editor, with ratios computed from the values above.

| Pair | Roles | Dark | Light |
|---|---|---|---|
| Body text on the canvas | `text` on `canvas` | 14.24 pass | 14.62 pass |
| Body text on panels | `text` on `surface` | 13.11 pass | 17.72 pass |
| Body text in fields | `text` on `field` | 11.88 pass | 16.12 pass |
| Body text in menus | `text` on `surface-raised` | 11.87 pass | 17.72 pass |
| Titles on panels | `text-strong` on `surface` | 15.13 pass | 19.90 pass |
| Secondary text on the canvas | `text-muted` on `canvas` | 7.05 pass | 6.38 pass |
| Secondary text on panels | `text-muted` on `surface` | 6.49 pass | 7.73 pass |
| Secondary text in fields | `text-muted` on `field` | 5.88 pass | 7.03 pass |
| Secondary text in menus | `text-muted` on `surface-raised` | 5.88 pass | 7.73 pass |
| Primary button label | `on-accent` on `accent-strong` | 5.17 pass | 5.17 pass |
| Primary button label, hover and pressed | `on-accent` on `accent-hover`, `accent-pressed` | 6.70, 8.72 pass | 6.70, 8.72 pass |
| Destructive button label | `on-accent` on `danger-strong` | 6.47 pass | 6.47 pass |
| Text on a control | `text` on `control` | 10.63 pass | 15.03 pass |
| Secondary text on a control | `text-muted` on `control` | 5.26 pass | 6.56 pass |
| Selected segment label | `text-strong` on `control-selected` | 10.28 pass | 15.68 pass |
| Row text on panels | `text-secondary` on `surface` | 11.25 pass | 14.89 pass |
| Text on a selected row | `selection-text` on `selection-bg` over `surface` | 8.59 pass | 7.15 pass |
| Error text, pale | `danger-text` on `danger-bg` | 8.42 pass | 7.60 pass |
| Variable chip | `chip-text` on `chip-bg` | 10.08 pass | 8.06 pass |
| Success text | `success` on `surface` | 9.54 pass | 5.02 pass |
| Warning text | `warning` on `warning-bg` | 12.72 pass | 6.84 pass |
| Error text | `danger` on `danger-bg` | 5.78 pass | 5.91 pass |
| Error text on panels | `danger` on `surface` | 6.01 pass | 6.47 pass |
| Information text | `info` on `surface` | 9.22 pass | 6.70 pass |
| Needs-review text | `review-text` on `review-bg` | 9.34 pass | 6.38 pass |
| Label on agent 1 | `on-agent` on `agent-1` | 7.73 pass | 5.18 pass |
| Label on agent 2 | `on-agent` on `agent-2` | 9.39 pass | 5.36 pass |
| Label on agent 3 | `on-agent` on `agent-3` | 6.62 pass | 6.98 pass |
| Initials on the person colour | `on-accent` on `person` | 4.64 pass | 6.14 pass |

| Interface part | Roles | Dark | Light |
|---|---|---|---|
| Selection outline on panels | `selection` on `surface` | 6.54 pass | 5.17 pass |
| Selection outline on the canvas | `selection` on `canvas` | 7.11 pass | 4.27 pass |
| Person dot | `person` on `surface` | 3.58 pass | 6.14 pass |
| Agent 1 dot or ring | `agent-1` on `surface` | 7.35 pass | 5.18 pass |
| Agent 2 dot or ring | `agent-2` on `surface` | 8.93 pass | 5.36 pass |
| Agent 3 dot or ring | `agent-3` on `surface` | 6.29 pass | 6.98 pass |
| Muted icon on panels | `icon-muted` on `surface` | 3.44 pass | 4.83 pass |
| Muted icon in fields | `icon-muted` on `field` | 3.12 pass | 4.40 pass |

`icon-muted` is for icons on a plain panel or field; on a hovered row (`hairline`), a selected row (`selection-bg`) or a `control` fill it is 2.34 to 2.96 in dark and misses 3:1, so icons on those fills use `text-muted` (4.41 or more). `text-muted` on `control-selected` is 4.41 in dark: it is for icons (3:1) and never for text there; text on a selected control is `text-strong`. `text-faint` fails on purpose (2.15 on `surface` in dark) and is never used for text; it is for decoration that carries no meaning (line numbers). A disabled control keeps its text colour and drops to half opacity (section 5.1). The brand `accent` (`#3b82f6`) carries no text: white on it is 3.68, so every text button uses `accent-strong` (5.17). Do not use `text-muted` on a fill darker or lighter than the roles listed; check the pair first. The pairs of this section are computed by `packages/web-tokens/src/contrast.test.ts`, which also scans the markup for `text-faint` as text and for `on-accent` on `accent`.

## 4. Type

- Sans: Inter. Mono: JetBrains Mono, for numbers, ids, code and variable values. Both fall back to the system fonts.
- Scale: caption 11 (section labels, weight 600), small 12 (dense rows and fields), body 13 (default), title 16, heading 22, display 24. Weights: regular 400, medium 500, semibold 600, bold 700.
- Line height: tight 1.2 for headings, body 1.45, relaxed 1.5 for explanatory text.
- Colour values show as OKLCH by default (decision 28), in the mono face.

## 5. Space, size and shape

- Space step is 4 px: 4, 8, 12, 16, 24, 32.
- Controls are 24, 28, 32 (default), 34 (wide action buttons) and 36 px high (section 5.1). The minimum target is 24 px.
- Panel widths: sidebar 248, layers 260, inspector 288, activity 360, chat 352, file tree 280. A top card is 52 high. The variables list in the inspector is 192 high and scrolls inside, so Properties stays on screen (the ModeSwitch board draws 150; 192 shows the rows of two types).
- Radii: 4 (chips), 6, 8 (buttons and fields), 12 (panels, menus and dialogs), pill 999.
- Disabled controls draw at 0.5 opacity.
- Strokes: selection 1.5 px; icons 1.5 px; focus ring 2 px with a 2 px offset.

### 5.1 Where the boards disagree: the rule written here

The canvas boards draw some sizes more than one way. These are the rules chosen for the web client; the owner can change one by changing this list and the token together.

- **Control height.** 24 `control-sm`: icon button in a row or a section header, and the smallest hit area. 28 `control-md`: dense cells (segmented cell, table row, small button). 32 `control-lg`: the default for fields, buttons, menu rows and panel toggles (Properties, Panels, StateMatrix and EditorLight agree). 34 `control-wide`: a button that spans a panel or a dialog (Export as PNG, Copy as HTML). 36 `control-xl`: the floating bars (tool bar, top card). Board heights 26 and 30 round to 28, 33 to 32; a segmented control is a 2 px track around 28 px cells, so 32 in all.
- **Radius.** 4 chip, 6 inner part (segment thumb, layer row, tooltip), 8 button and field, 12 panel, menu, popover and dialog, 999 pill. Board radius 7 becomes 8 and 10 becomes 12; the 14 of one card becomes 12.
- **Type.** Only the six sizes of section 4. Board 14 and 15 become 13; 12.5 becomes 12.
- **Spacing.** Only the 4 px steps. An off-step board value rounds half up: 6 to 8, 10 to 12, 14 to 16. The 2 px padding of a segmented track is half a step (`p-0.5`).
- **Panel side.** One right panel card with the tabs `Design | Activity`. The Design side is 288 (`size.inspector`, Panels, Properties and Inspector boards), the Activity tab 360 (`size.activity`, Activity board). The card keeps the width of its side, so the Activity tab is the wider one.
- **Hover and pressed.** The StateMatrix values are adopted (marked proposed above): ghost and secondary hover is `control`, selected and pressed `control-selected`, primary `accent-hover` and `accent-pressed`, destructive `danger-hover` and `danger-pressed`, a layer row hovers on `hairline`.
- **Selected segment, light.** EditorLight draws a white thumb with a 1 px shadow; the web client uses `control-selected` (`#e4e4e7`) on the `field` track with `text-strong` and weight 600, so a selected item is not told by colour alone.
- **Foundations against the other boards.** Foundations draws the default control at 28; Panels, Properties, StateMatrix and EditorLight draw 32, which is the rule above. Foundations also shows dark `text-muted` as `#8B8B94` and the person colour `You` as `#7C6CF0` against a `#6E60EA` swatch; this file keeps `#a1a1aa` and `#6e60ea`, the values whose contrast is computed in 3.2.
- **Actors in the Layers panel.** A row last changed by another actor shows the actor's name in text, always, and a dot whose colour is one of the three agent colours chosen by a stable hash of the actor id (a person gets the person colour). With more than three agents two of them share a colour: the name is what tells them apart.
- **Segmented control, light.** `control-selected` on `field` is about 1.1:1, so the selected cell carries a 1 px `border-strong` outline as well (the board's white thumb with a shadow is not used).
- **Field outline.** The default outline is `divider`, a hovered one `border-strong`, a focused one `selection` with the focus ring, an error one `danger`.

## 6. Elevation

Floating panels carry the float shadow, menus and popovers the popover shadow, and a dialog the dialog shadow on the scrim. Nothing else casts a shadow: panels inside a surface are separated by a hairline.

| Shadow | Use | Dark | Light |
|---|---|---|---|
| `shadow-float` | Floating panels | `0 8px 24px rgba(0,0,0,0.35)` | `0 8px 24px rgba(24,24,27,0.12)` |
| `shadow-popover` | Menus and popovers | `0 12px 32px rgba(0,0,0,0.5)` | `0 12px 32px rgba(24,24,27,0.18)` |
| `shadow-dialog` | Dialogs on the scrim | `0 24px 64px rgba(0,0,0,0.6)` | `0 24px 64px rgba(24,24,27,0.22)` (proposed: the boards draw no light dialog) |

## 7. Motion

Durations: fast 120 ms (hover, press), base 200 ms (panels, menus), slow 320 ms (dialogs), with the standard easing. An agent touch ring fades over 1500 ms. With reduced motion, nothing animates: a touch ring becomes a static outline held for 2000 ms and panels appear without a transition.

## 8. Components

- **Buttons.** Primary: `accent-strong` fill with `on-accent` text, weight 600. Secondary: `field` fill. Destructive: `danger-strong` fill. Icon-only buttons have an accessible name and a pressed state when they toggle a panel.
- **Fields.** `field` fill, radius 8, a 2 px `selection` ring on focus. A field bound to a Variable uses the chip colours and shows the name and the resolved value; a missing Variable shows the error colours and keeps the name.
- **Tabs and segmented controls.** The selected cell of a segmented control is `control-selected` with a 1 px `border-strong` outline on the `field` track, with `text-strong` and weight 600. The selected tab of a panel is `hairline` with `text-strong`, and the other tabs are `text-muted`.
- **Menus and popovers.** `surface-raised`, radius 12, the popover shadow, 32 to 34 px rows. A tooltip appears only on hover or focus and is never drawn as a standing element.
- **Dialogs.** `surface` on the scrim, radius 12; the primary action is on the right; a destructive action says what is deleted and that it cannot be undone.
- **Messages.** Say what happened, whether the file changed, and what happens next. Warning and error fills pair with their text colours above.
- **Diffs.** Added and removed lines use `diff-add` and `diff-delete` with a 2 px bar in `success` and `danger`.

## 9. Canvas overlays

Overlays are drawn in a container that is not scaled, so strokes stay the same width at any zoom. Selection is a `selection` outline of 1.5 px with square handles and a size badge. An agent touch ring is 2 px in the agent colour with the agent's name in a label. Frames take pointer events; overlays take none.

## 10. Accessibility

- Contrast as in section 3.2; check any new pair before use.
- Every control is reachable by keyboard in a logical order; the focus ring is never removed.
- Colour is never the only signal (actors, status, diffs).
- Respect reduced motion (section 7).
- Text can be selected. Sizes are in pixels, and browser zoom must scale the whole editor.

## 11. Not decided yet

- Brand: the logo and any wordmark styling (`PRODUCT.md`).
- Whether Inter and JetBrains Mono are self-hosted and shipped; both are open licences, but the licence check does not cover fonts yet.
- Display P3 or OKLCH for the interface palette itself (`docs/tokens-and-variables.md` Q3 is about the user's Variables).
- A fine-tuned light appearance; the values above keep the roles and pass contrast, but have had less review than the dark ones.

## 12. Token values

Machine-readable and compared by a test. One `key = value` per line, no quotes.

<!-- tokens:start -->
```text
colour.dark.canvas = #161618
colour.dark.sidebar = #1b1b1e
colour.dark.surface = #1e1e21
colour.dark.surface-raised = #26262b
colour.dark.field = #26262a
colour.dark.control = #2e2e34
colour.dark.control-selected = #3a3a40
colour.dark.menu-border = #3a3a42
colour.dark.divider = #34343a
colour.dark.border = #2e2e33
colour.dark.border-strong = #4a4a52
colour.dark.hairline = #2a2a2e
colour.dark.text = #e4e4e7
colour.dark.text-strong = #f4f4f5
colour.dark.text-muted = #a1a1aa
colour.dark.text-secondary = #d4d4d8
colour.dark.text-faint = #52525b
colour.dark.icon-muted = #71717a
colour.dark.accent = #3b82f6
colour.dark.accent-strong = #2563eb
colour.dark.accent-hover = #1d4ed8
colour.dark.accent-pressed = #1e40af
colour.dark.on-accent = #ffffff
colour.dark.selection = #60a5fa
colour.dark.selection-text = #bfdbfe
colour.dark.chip-bg = #1f2540
colour.dark.chip-border = #343d6e
colour.dark.chip-text = #c7d2fe
colour.dark.person = #6e60ea
colour.dark.agent-1 = #fb923c
colour.dark.agent-2 = #2dd4bf
colour.dark.agent-3 = #c084fc
colour.dark.on-agent = #1c1917
colour.dark.success = #4ade80
colour.dark.warning = #fde68a
colour.dark.warning-bg = #26221a
colour.dark.danger = #f87171
colour.dark.danger-bg = #2a1f1f
colour.dark.danger-text = #fca5a5
colour.dark.danger-strong = #b91c1c
colour.dark.danger-hover = #991b1b
colour.dark.danger-pressed = #7f1d1d
colour.dark.info = #93c5fd
colour.dark.review-bg = #2b2118
colour.dark.review-text = #fdba74
colour.dark.diff-add = rgba(74,222,128,0.10)
colour.dark.diff-delete = rgba(248,113,113,0.10)
colour.dark.selection-bg = rgba(96,165,250,0.18)
colour.dark.scrim = rgba(0,0,0,0.5)
colour.dark.shadow-float = 0 8px 24px rgba(0,0,0,0.35)
colour.dark.shadow-popover = 0 12px 32px rgba(0,0,0,0.5)
colour.dark.shadow-dialog = 0 24px 64px rgba(0,0,0,0.6)
colour.light.canvas = #e9e9ec
colour.light.sidebar = #f4f4f5
colour.light.surface = #ffffff
colour.light.surface-raised = #ffffff
colour.light.field = #f4f4f5
colour.light.control = #ececef
colour.light.control-selected = #e4e4e7
colour.light.menu-border = #d4d4d8
colour.light.divider = #d4d4d8
colour.light.border = #e4e4e7
colour.light.border-strong = #a1a1aa
colour.light.hairline = #ececef
colour.light.text = #18181b
colour.light.text-strong = #09090b
colour.light.text-muted = #52525b
colour.light.text-secondary = #27272a
colour.light.text-faint = #a1a1aa
colour.light.icon-muted = #71717a
colour.light.accent = #2563eb
colour.light.accent-strong = #2563eb
colour.light.accent-hover = #1d4ed8
colour.light.accent-pressed = #1e40af
colour.light.on-accent = #ffffff
colour.light.selection = #2563eb
colour.light.selection-text = #1e40af
colour.light.chip-bg = #e0e7ff
colour.light.chip-border = #a5b4fc
colour.light.chip-text = #3730a3
colour.light.person = #5b4bd6
colour.light.agent-1 = #c2410c
colour.light.agent-2 = #0e7490
colour.light.agent-3 = #7e22ce
colour.light.on-agent = #ffffff
colour.light.success = #15803d
colour.light.warning = #92400e
colour.light.warning-bg = #fffbeb
colour.light.danger = #b91c1c
colour.light.danger-bg = #fef2f2
colour.light.danger-text = #991b1b
colour.light.danger-strong = #b91c1c
colour.light.danger-hover = #991b1b
colour.light.danger-pressed = #7f1d1d
colour.light.info = #1d4ed8
colour.light.review-bg = #ffedd5
colour.light.review-text = #9a3412
colour.light.diff-add = rgba(21,128,61,0.10)
colour.light.diff-delete = rgba(185,28,28,0.10)
colour.light.selection-bg = #dbeafe
colour.light.scrim = rgba(0,0,0,0.35)
colour.light.shadow-float = 0 8px 24px rgba(24,24,27,0.12)
colour.light.shadow-popover = 0 12px 32px rgba(24,24,27,0.18)
colour.light.shadow-dialog = 0 24px 64px rgba(24,24,27,0.22)
font-family.sans = Inter, system-ui, sans-serif
font-family.mono = JetBrains Mono, ui-monospace, monospace
font-size.caption = 11
font-size.small = 12
font-size.body = 13
font-size.title = 16
font-size.heading = 22
font-size.display = 24
font-weight.regular = 400
font-weight.medium = 500
font-weight.semibold = 600
font-weight.bold = 700
line-height.tight = 1.2
line-height.body = 1.45
line-height.relaxed = 1.5
space.1 = 4
space.2 = 8
space.3 = 12
space.4 = 16
space.5 = 24
space.6 = 32
radius.xs = 4
radius.sm = 6
radius.md = 8
radius.lg = 12
radius.pill = 999
size.control-sm = 24
size.control-md = 28
size.control-lg = 32
size.control-wide = 34
size.control-xl = 36
size.sidebar = 248
size.layers = 260
size.inspector = 288
size.activity = 360
size.chat = 352
size.files = 280
size.top-card = 52
size.variables-list = 192
stroke.selection = 1.5
stroke.icon = 1.5
stroke.focus = 2
stroke.focus-offset = 2
opacity.disabled = 0.5
duration.fast = 120
duration.base = 200
duration.slow = 320
duration.touch-ring = 1500
duration.reduced-motion-hold = 2000
easing.standard = cubic-bezier(0.2, 0, 0, 1)
```
<!-- tokens:end -->
