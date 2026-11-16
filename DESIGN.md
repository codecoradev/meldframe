# Design

Status: draft for owner review (16 Nov 2026). This is the design file that `crates/ui/meldframe-ui` is compared with (rule U4 in `docs/standards/modular-thin-ui.md`). It holds the values and the rules; how the editor behaves is in `docs/ui-design.md`; product context is in `PRODUCT.md`.

"UI token" in this page means a value of the editor's own interface (a colour, a size, a duration). It is not a Variable: Variables belong to the person's design system and are described in `docs/tokens-and-variables.md`.

## 1. Direction

The category standard, played straight, at the craft level of the tools designers already know (`PRODUCT.md`). The editor is an operating surface, so scanability, consistency and native expectations outrank expression. It must not look like an AI chat with a canvas attached.

- The canvas is the window. Chrome floats over it and collapses.
- Dark is the default; light is supported with the same roles.
- The interface is neutral. Colour belongs to the design on the canvas, to the selection and to actors (the person and each agent).
- One accent blue. It marks what is selected and what is primary; it is never decoration.

## 2. How the tokens are kept honest

- UI code reads colours, font sizes and durations from the tokens module (`crates/ui/meldframe-ui/src/tokens.rs` for Rust, `tokens.css` for the web client). `cargo xtask ui-literals` rejects literals anywhere else.
- The block at the end of this page is the single list of values. A test in `meldframe-ui` compares it with `tokens.rs` and `tokens.css` and fails when they differ.
- To change a value, change the block, `tokens.rs` and `tokens.css` together in one pull request.
- The block lists plain numbers. In `tokens.css` sizes carry `px` and durations carry `ms`, so a custom property can be used directly in a style; the test adds the unit before it compares.

## 3. Colour

Roles, not hues: components ask for `surface` or `text-muted`, never for a hex value. Each role has a dark and a light value.

| Role | Use | Dark | Light |
|---|---|---|---|
| `canvas` | The editor canvas behind frames | `#161618` | `#e9e9ec` |
| `sidebar` | Sidebar and docked bars | `#1b1b1e` | `#f4f4f5` |
| `surface` | Panels, cards and dialogs | `#1e1e21` | `#ffffff` |
| `surface-raised` | Menus, popovers and tooltips | `#26262b` | `#ffffff` |
| `field` | Input fields and segmented controls | `#26262a` | `#f4f4f5` |
| `border` | Outlines of panels and cards | `#2e2e33` | `#e4e4e7` |
| `hairline` | Dividers inside a panel | `#2a2a2e` | `#ececef` |
| `text` | Body text | `#e4e4e7` | `#18181b` |
| `text-strong` | Titles and selected labels | `#f4f4f5` | `#09090b` |
| `text-muted` | Secondary text, never dimmer | `#a1a1aa` | `#52525b` |
| `text-faint` | Decorative only: line numbers, disabled glyphs | `#52525b` | `#a1a1aa` |
| `accent` | Marks and fills that carry no text | `#3b82f6` | `#2563eb` |
| `accent-strong` | Primary button fill | `#2563eb` | `#2563eb` |
| `on-accent` | Text on an accent fill | `#ffffff` | `#ffffff` |
| `selection` | Selection outline, focus ring and handles | `#60a5fa` | `#2563eb` |
| `chip-bg` | Variable chip fill | `#1f2540` | `#eef2ff` |
| `chip-border` | Variable chip outline | `#343d6e` | `#c7d2fe` |
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
| `danger-strong` | Destructive button fill | `#b91c1c` | `#b91c1c` |
| `info` | Information text | `#93c5fd` | `#1d4ed8` |
| `review-bg` | Needs-review fill | `#2b2118` | `#fff7ed` |
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
| Destructive button label | `on-accent` on `danger-strong` | 6.47 pass | 6.47 pass |
| Variable chip | `chip-text` on `chip-bg` | 10.08 pass | 8.88 pass |
| Success text | `success` on `surface` | 9.54 pass | 5.02 pass |
| Warning text | `warning` on `warning-bg` | 12.72 pass | 6.84 pass |
| Error text | `danger` on `danger-bg` | 5.78 pass | 5.91 pass |
| Error text on panels | `danger` on `surface` | 6.01 pass | 6.47 pass |
| Information text | `info` on `surface` | 9.22 pass | 6.70 pass |
| Needs-review text | `review-text` on `review-bg` | 9.34 pass | 6.88 pass |
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

`text-faint` fails on purpose and is allowed only for decoration that carries no meaning (line numbers, disabled glyphs). Do not use `text-muted` on a fill darker or lighter than the roles listed; check the pair first.

## 4. Type

- Sans: Inter. Mono: JetBrains Mono, for numbers, ids, code and variable values. Both fall back to the system fonts.
- Scale: caption 11 (section labels, weight 600), small 12 (dense rows and fields), body 13 (default), title 16, heading 22, display 24. Weights: regular 400, medium 500, semibold 600, bold 700.
- Line height: tight 1.2 for headings, body 1.45, relaxed 1.5 for explanatory text.
- Colour values show as OKLCH by default (decision 28), in the mono face.

## 5. Space, size and shape

- Space step is 4 px: 4, 8, 12, 16, 24, 32.
- Controls are 24, 28 (default), 32 and 36 px high (the tool bar uses 36). The minimum target is 24 px.
- Panel widths: sidebar 248, layers 260, inspector 288, chat 352, file tree 280.
- Radii: 4 (chips), 6, 8 (buttons and fields), 12 (panels and dialogs), pill 999.
- Strokes: selection 1.5 px; focus ring 2 px with a 2 px offset.

## 6. Elevation

Floating panels carry the float shadow, menus and popovers the popover shadow, and a dialog sits on the scrim. Nothing else casts a shadow: panels inside a surface are separated by a hairline.

| Shadow | Use | Dark | Light |
|---|---|---|---|
| `shadow-float` | Floating panels | `0 8px 24px rgba(0,0,0,0.35)` | `0 8px 24px rgba(0,0,0,0.12)` |
| `shadow-popover` | Menus and popovers | `0 12px 32px rgba(0,0,0,0.5)` | `0 12px 32px rgba(0,0,0,0.18)` |

## 7. Motion

Durations: fast 120 ms (hover, press), base 200 ms (panels, menus), slow 320 ms (dialogs), with the standard easing. An agent touch ring fades over 1500 ms. With reduced motion, nothing animates: a touch ring becomes a static outline held for 2000 ms and panels appear without a transition.

## 8. Components

- **Buttons.** Primary: `accent-strong` fill with `on-accent` text, weight 600. Secondary: `field` fill. Destructive: `danger-strong` fill. Icon-only buttons have an accessible name and a pressed state when they toggle a panel.
- **Fields.** `field` fill, radius 8, a 2 px `selection` ring on focus. A field bound to a Variable uses the chip colours and shows the name and the resolved value; a missing Variable shows the error colours and keeps the name.
- **Tabs and segmented controls.** The selected item is `surface-raised` on a `field` track; the selected tab of a panel also gains `text-strong`.
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
colour.dark.border = #2e2e33
colour.dark.hairline = #2a2a2e
colour.dark.text = #e4e4e7
colour.dark.text-strong = #f4f4f5
colour.dark.text-muted = #a1a1aa
colour.dark.text-faint = #52525b
colour.dark.accent = #3b82f6
colour.dark.accent-strong = #2563eb
colour.dark.on-accent = #ffffff
colour.dark.selection = #60a5fa
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
colour.dark.danger-strong = #b91c1c
colour.dark.info = #93c5fd
colour.dark.review-bg = #2b2118
colour.dark.review-text = #fdba74
colour.dark.diff-add = rgba(74,222,128,0.10)
colour.dark.diff-delete = rgba(248,113,113,0.10)
colour.dark.scrim = rgba(0,0,0,0.5)
colour.dark.shadow-float = 0 8px 24px rgba(0,0,0,0.35)
colour.dark.shadow-popover = 0 12px 32px rgba(0,0,0,0.5)
colour.light.canvas = #e9e9ec
colour.light.sidebar = #f4f4f5
colour.light.surface = #ffffff
colour.light.surface-raised = #ffffff
colour.light.field = #f4f4f5
colour.light.border = #e4e4e7
colour.light.hairline = #ececef
colour.light.text = #18181b
colour.light.text-strong = #09090b
colour.light.text-muted = #52525b
colour.light.text-faint = #a1a1aa
colour.light.accent = #2563eb
colour.light.accent-strong = #2563eb
colour.light.on-accent = #ffffff
colour.light.selection = #2563eb
colour.light.chip-bg = #eef2ff
colour.light.chip-border = #c7d2fe
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
colour.light.danger-strong = #b91c1c
colour.light.info = #1d4ed8
colour.light.review-bg = #fff7ed
colour.light.review-text = #9a3412
colour.light.diff-add = rgba(21,128,61,0.10)
colour.light.diff-delete = rgba(185,28,28,0.10)
colour.light.scrim = rgba(0,0,0,0.35)
colour.light.shadow-float = 0 8px 24px rgba(0,0,0,0.12)
colour.light.shadow-popover = 0 12px 32px rgba(0,0,0,0.18)
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
size.control-xl = 36
size.sidebar = 248
size.layers = 260
size.inspector = 288
size.chat = 352
size.files = 280
stroke.selection = 1.5
stroke.focus = 2
stroke.focus-offset = 2
duration.fast = 120
duration.base = 200
duration.slow = 320
duration.touch-ring = 1500
duration.reduced-motion-hold = 2000
easing.standard = cubic-bezier(0.2, 0, 0, 1)
```
<!-- tokens:end -->
