# Editor UI design

Status: **draft for owner review** (16 Nov 2026). `[D]` = decided by the owner on 16 Nov 2026, `[P]` = proposed, `[Q]` = open question. Related: `decisions.md` #27 to #30, `collab-ux.md`, `tokens-and-variables.md` section 10, ADR-0008.

This is a design note, not product code. It records what the first design pass of the editor looks like and why, so the web front end (E1.6, E3.4, E5.1) and a later desktop front end build the same thing. The pictures live in the design canvas "Meldframe UI/UX" (private); this page keeps the decisions. Wonder was the reference for layout and mood, studied from the owner's screenshots and recordings. Nothing is copied: names, accent colour and several features differ.

## 1. Principles

1. The canvas is the window. Chrome floats over it and collapses, so a person who only wants to look sees nothing else. `[D]` (decision 27)
2. The editor is an operating surface: neutral, dense and scannable. Colour belongs to the design on the canvas, to selection and to actors. `[P]`
3. The front end only renders app state and sends actions (ADR-0008). Every rule in this page that is about behaviour (what a panel lists, what a view contains) lives in `meldframe-app`.
4. UI colours, sizes and durations come from the tokens module (`crates/ui/meldframe-ui`). The values in this page are proposals to replace the placeholders there. `[P]`

## 2. Web editor layout

| Part | Content | Notes |
|---|---|---|
| Canvas | Full window, pan and zoom, frames with names above them | Drawn with PixiJS; see section 9. |
| Top left | Menu, page switcher, button that shows the layers panel | A small floating card. |
| Top right | Button that shows the right panel, the person's account menu (their profile, not a Workspace), Share | A small floating card. Share is a placeholder until sharing is designed (section 6). |
| Bottom centre | Tool bar | Floating, groups separated by a thin rule. |
| Bottom right | Zoom, connection state | Plain text. |
| Left panel | Pages and layers | Floating, inset 12 px, hidden by default. |
| Right panel | Two tabs: Design (Variables, Properties) and Activity | Floating, inset 12 px, hidden by default. |

`[D]` for the arrangement (decision 27; the owner reviewed the closed and open states and accepted them), `[P]` for sizes: panels 260 and 288 px wide, tool buttons 36 px.

### 2.1 Tool bar

Move (V), Frame (F), Hand (H), Rectangle (R), Text (T), Comment (C, hidden until comments exist, section 7), then the view controls:

- **Layers** (Option+1) shows or hides the left panel.
- **Inspector** (Option+2) shows or hides the right panel.
- **Save or choose a view** (+) opens a list: Default (both panels), Review (Activity open), Canvas only (no panels), and "Save current view". A saved view stores which panels are open and which tab is active. `[P]`

Every button has a name and its shortcut in a tooltip, and a pressed state when its panel is open. The top-corner buttons and the tool bar buttons are two controls for the same state.

## 3. Right panel

The panel has two tabs, Design and Activity (section 4). On the Design tab, sections stack from the top and scroll inside the panel; each section collapses.

1. **Variables** (open by default): a count in the title, search and add buttons, a list grouped by type: Color, Dimensions, Font family. Dimensions group further by name prefix (`radius`, `space`), as slash names already imply. The list scrolls inside a fixed height so Properties stays in view. A link in the header opens the full table with modes (`tokens-and-variables.md` 10.3). `[P]`
2. **Properties**: for the selected node: Position, Layout, Size, Appearance, Fill, Border, Shadow, Export. With nothing selected it shows the page: background colour, opacity and visibility.

### 3.1 Properties fields

| Section | Fields |
|---|---|
| Position | X, Y |
| Layout | Flow (free, row, column, grid); gap and alignment for flex; columns, rows and gap for grid; padding (two fields, expands to four sides); margin (four sides) |
| Size | Width and height with a unit and a sizing mode (fixed, hug, fill) |
| Appearance | Opacity, corner radius, clip content, visibility |
| Fill | Type (solid, linear gradient, radial gradient, image, none), value or variable |
| Border | Width, sides, colour, visibility, remove |
| Shadow | Up to four, each with X, Y, blur, spread, colour, visibility, remove |
| Export | File name, scale (0.5x to 4x), format, "Export as PNG"; a button to copy HTML and Tailwind |

The fields follow the round 2 model (E3.2): margin, per-side padding, gradients, opacity and no fill. Layout changes its fields with the flow.

### 3.2 Variables in fields

Every bindable field has a variable button at its end. It opens a picker filtered by type and scope, searchable, and typing `$` in a field opens the same picker. A bound field shows an indigo chip with the variable name and, on hover, the resolved value; a detach button returns it to the literal. A missing variable shows in red with its name kept. Adding a variable asks for type, name (with the exported name shown, `--surface-raised`), the value for the first mode, and what it may be used for. A new mode copies the first mode's values and shows them as inherited until edited.

### 3.3 Colour display

Colour variables show as OKLCH by default, for example `OKLCH(0.99 0.004 150)`, to match the Tailwind v4 palette. The colour picker also offers hex, and "copy as hex" is always available. A Preferences switch changes the default. `[D]` (decision 28). This is display only: how colour is stored and rendered on the canvas is still `tokens-and-variables.md` Q3.

## 4. Agents in the interface

- **Actor colour.** Each actor has a stable colour: the person blue, agents orange, teal, purple. It appears on the layer dot, the touch ring, the review tint and the Activity lane. Actors also differ by name, never by colour alone. `[P]`
- **Layers panel.** A dot and a relative time for the last writer, a "new" badge for nodes an agent added in the current run, and a filter that shows only agent changes (`collab-ux.md` 5.2).
- **Canvas.** A touch ring (2 px, actor colour, fades over 1.5 s; static for 2 s with reduced motion) and a name label. Selection is a 1.5 px outline with square handles and a size badge. Rings are ephemeral events (ADR-0009).
- **Activity** is the second tab of the right panel, next to Design, with a count of batches that need review. This settles `collab-ux.md` Q6. `[P]`
- **Follow agent** is a Preferences setting, off by default (Q7 in that page stays open for v1).
- **Messages.** Every message says what happened, whether the file changed and what happens next; the texts in `collab-ux.md` section 7 apply. A connection bar shows connected, reconnecting (edits are kept), catching up and offline.

## 5. Pages outside the editor

- **Files**: workspace switcher, tabs (All, Recent, Needs review), grid or list, search, New folder, New file, cards with a thumbnail, owner, edit time and a review count.
- **Settings**: Account (the person's profile), Preferences, Connections and, for the workspace, Agents and Members (Members is a placeholder, later). Preferences hold appearance (System, Light, Dark), colour values (OKLCH or hex), canvas selection behaviour (Classic or Deep), small nudge (1) and large nudge (10), Follow agent, reduced motion for touch rings, and links in the desktop app.

## 6. Left out on purpose

Outline, Filters, Mask, shader fills and an Icons library are in the Wonder panel but not in the plan. Upgrade, plans and usage banners are out until pricing is decided (decision 13). Public and private files, and the Share button's behaviour, are out because sharing is not designed; decision 8 stands.

## 7. Comments

The web editor stays a visual surface driven by MCP. Comments on the web are parked `[D]` (decision 29); decision 8 stands. If they return, the design is a note from the person to an agent: pinned to a node, `@` mentions an agent, a thread is Waiting for agent, Agent replied or Resolved, and an agent reply can link to the batch that addressed it. Panel: tabs All, Unread, Agents. The design exists in the canvas and is not part of any epic.

## 8. Desktop front end (phase D)

After the web editor is stable (decision 5). The layout is **A, mode switch** `[D]` (decision 30; owner, 16 Nov 2026); the details are `[P]`. Two other layouts were drawn and set aside: B shows Design and Code side by side and needs about 1680 px, so it can come back as a saved view for wide screens; C lets any tab go in any group by dragging, which is the most work and can come later from A's tabs.

Four panes, left to right:

1. **Sidebar** (248 px): New chat; a list of repositories (working name "Projects", see Q4), each a local folder or a GitHub repository, with its chats beneath and diff counts `+N -M`; Quick chats; a Next step card (for example "2 batches need review before you export"); the account.
2. **Chat pane** (352 px): chats only. Its tab row holds chat tabs and its + is New chat. It has an agent picker, messages with batch cards (Show, Revert, Accept), a files-changed line, tool prompts from the agent (allow once, always in this chat, deny; these are the agent's own prompts, separate from a gate Approval), and the composer with context chips (file, node, variable), a mode (Ask, Edit, Auto), and model and effort. The composer lives here in both modes.
3. **Work area, Design mode:** tabs for designs, with the Files home as the first tab. The + menu offers New design; presentation and motion appear in it as disabled "later" items (decision 3). The canvas fills the area; the inspector and the tool bar float over it exactly as on the web.
4. **Work area, Code mode:** on the left, tabs for file viewers (a file opened once is a preview tab, in italics) and terminals; the + menu offers Open file and New terminal. A viewer marks changes against the base as green and red lines and has an Open in editor control. On the right, the file tree (280 px) with the chat's branch, tabs All files, Changes (with a count) and Exports, and a button to hide it. An export that writes HTML and Tailwind into the working copy appears under Changes.

The title bar is split into three cells by vertical rules: window controls and sidebar toggle; repository and chat name with a collapse button for the chat pane; and the work area's tab strip with the **Design** and **Code** switch at the right. The switch is two labelled buttons; shortcuts show only in a tooltip on hover. Each mode remembers its own panel state, and a saved view stores the mode and which panes are open.

Agents speak ACP to a program on the person's machine, Claude Code first, with a custom ACP command allowed. Design changes from a chat arrive over MCP like any agent's: they pass the gate and appear as batches, so Review and Revert work unchanged. The chat adds conversation, code access and tool prompts only.

Open: whether the sidebar word is Projects or Repositories (Q4); whether a repository is only where code is read and written while the design stays in the workspace; whether one chat means one working copy and branch; whether Start preview (run the repository's app) is in scope; whether Code mode is in the first desktop build.

## 9. Rendering `[P]`

Svelte 5 with Tailwind draws the chrome and Bits UI style primitives where needed; PixiJS v8 draws the canvas. One world container flagged as a render group acts as the camera, so pan and zoom change one matrix. Selection, size badge and touch rings sit in an overlay container that is not scaled, so strokes stay 1.5 px at any zoom. PixiJS v8 does not cull by itself: run the culler when pan or zoom ends. Frames take pointer events; overlays take none. Text on the canvas is measured by the core and drawn by PixiJS; editing text puts a DOM field over the node.

## 10. Open questions

- Q1 How are OKLCH and Display P3 stored and drawn (`tokens-and-variables.md` Q3)?
- Q2 What does a saved view store beyond open panels and the active tab?
- Q3 Is the Review view's before and after compare in v1 (`collab-ux.md` Q8)?
- Q4 Desktop: the sidebar word (GLOSSARY lists Project as a word to avoid for File), repository versus workspace file, and the working copy per chat (section 8).
- Q5 Do comments return on the web, and if so as notes to an agent only (section 7)?
