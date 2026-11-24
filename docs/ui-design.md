# Editor UI design

Status: **draft for owner review** (16 Nov 2026). `[D]` = decided by the owner on 16 Nov 2026, `[P]` = proposed, `[Q]` = open question. Related: `decisions.md` #27 to #32 and #40, `collab-ux.md`, `tokens-and-variables.md` section 10, ADR-0008.

This is a design note, not product code. It records what the first design pass of the editor looks like and why, so the web front end (E1.6, E3.4, E5.1) and a later desktop front end build the same thing. The pictures live in the design canvas "Meldframe UI/UX" (private); this page keeps the decisions. Wonder was the reference for layout and mood, studied from the owner's screenshots and recordings. Nothing is copied: names, accent colour and several features differ.

How the boards were checked, and what a person still has to review, is in `docs/design-qa.md`.

## 1. Principles

1. The canvas is the window. Chrome floats over it and collapses, so a person who only wants to look sees nothing else. `[D]` (decision 27)
2. The editor is an operating surface: neutral, dense and scannable. Colour belongs to the design on the canvas, to selection and to actors. `[P]`
3. The front end only renders app state and sends actions (ADR-0008). Every rule in this page that is about behaviour (what a panel lists, what a view contains) lives in `meldframe-app`.
4. UI colours, sizes and durations come from the tokens module (`crates/ui/meldframe-ui`). The values and the rules are in `DESIGN.md`. `[P]`

## 2. Web editor layout

| Part | Content | Notes |
|---|---|---|
| Canvas | Full window, pan and zoom, frames with names above them | Drawn with PixiJS; see section 9. |
| Top left | Menu, page switcher, button that shows the layers panel | A small floating card. |
| Top right | Button that shows the right panel, the person's account menu (their profile, not a Workspace), Share | A small floating card. Share is a placeholder until sharing is designed (section 6). |
| Bottom centre | Tool bar | Floating, groups separated by a thin rule. |
| Bottom right | Zoom, connection state | The connection state is plain text. The zoom readout opens a menu: zoom in, zoom out, 100%, fit all, zoom to selection. |
| Left panel | Pages and layers | Floating, inset 12 px, hidden by default. |
| Right panel | Two tabs: Design (Variables, Properties) and Activity | Floating, inset 12 px, hidden by default. |

`[D]` for the arrangement (decision 27; the owner reviewed the closed and open states and accepted them), `[P]` for sizes: panels 260 and 288 px wide, tool buttons 36 px.

The canvas does not run under the open panels. Principle 1 says the canvas is the window, and the boards draw the cards over a full-window canvas, but the web client gives the canvas the area between the open cards (each open card reserves its 12 px inset and its width). Why: with the canvas under the 260 px Layers card, the resize handles and the fit of a frame near the left edge were hidden behind the card and could not be used. The consequence: opening or closing a panel moves the canvas edge, so the content shifts, the Pixi renderer resizes, and fit-to-content and capture fit the narrower area. The cards still float, with their border, radius and shadow. Reconsider this when the canvas can pan under a card (the camera would need an inset of its own).

### 2.1 Tool bar

Move (V), Frame (F), Hand (H), Rectangle (R), Ellipse (E), Text (T), Comment (C, hidden until comments exist, section 7), then the view controls:

- **Layers** (Option+1) shows or hides the left panel.
- **Inspector** (Option+2) shows or hides the right panel.
- **Save or choose a view** (+) opens a list: Default (both panels), Review (Activity open), Canvas only (no panels), and "Save current view". A saved view stores which panels are open and which tab is active. `[P]`

Every button has a name and its shortcut in a tooltip, and a pressed state when its panel is open. The top-corner buttons and the tool bar buttons are two controls for the same state.

## 3. Right panel

The panel has two tabs, Design and Activity (section 4). On the Design tab, sections stack from the top and scroll inside the panel; each section collapses.

1. **Variables** (open by default): a count in the title, search and add buttons, a list grouped by type: Color, Dimensions, Font family. Dimensions group further by name prefix (`radius`, `space`), as slash names already imply. The list scrolls inside a fixed height so Properties stays in view. A link in the header opens the full table with modes (`tokens-and-variables.md` 10.3).  The header also has **Import variables**, a dialog with a field for pasted CSS and a file chooser (read in the browser, nothing uploaded), which shows what the import made (collections, modes, variables, aliases), what it skipped and why, or why it was refused; the import only adds, a name that exists is refused whole and named, at most 50 variables are taken at a time (a longer style sheet is refused with a pointer to the agent tool `import_tokens`), the text stays in the field when the dialog closes without an import (Escape is not a Clear; there is a Clear button), and one undo takes it all back, after which the dialog no longer says "Imported". It has **Export variables**, a dialog with every variable of the file as CSS custom properties, one per line, a preview, Copy and Download (`variables.css`), the warnings of what could not be written as stored, and a note that DTCG and `@theme` output come later (`tokens-and-variables.md` 10.3.1). The text is built by the app layer and the dialog opens and closes through it; Escape closes it and the focus goes back to the button. `[P]`
2. **Properties**: for the selected node: its name, then Position, Size, Layout (frames), Margin (a node in an auto layout parent), Appearance, Text (a text), Fill, Border and Shadow. With nothing selected, or with several nodes selected, it says so and shows no fields: the file holds nodes only, so there is no page background, opacity or visibility to edit. **Export** is a section of the card below Properties, shown while exactly one frame is selected.

### 3.1 Properties fields

This is the list QA checks the panel against: the table is what is built, and section 3.1a is what is not in W1 and will not be found in the panel. A control that appears from 3.1a, or a row of the table that is missing, is a defect or a change of scope that needs the decision named there (`decisions.md` #40, proposed).

| Section | Fields (built) |
|---|---|
| Name | The name of the node, in the header of the panel. |
| Position | X, Y (shown as drawn; disabled, with the reason "Set by the auto layout of the parent.", for a node that an auto layout parent places) |
| Size | Width and height in px, and for each a sizing mode (fixed, hug, fill) with the notes that say where a mode has no effect |
| Layout | Frames only. Direction (free, row, column); for a row or a column: gap, padding (all sides in one field, which says Mixed when the sides differ, and each side), align (start, center, end) and justify (start, center, end, between) |
| Margin | A node in an auto layout parent, and elsewhere only while it has a margin or is absolute: margin on each of four sides, and Absolute position |
| Appearance | Opacity; corner radius (frame, rectangle and shader; an ellipse and a text have none); Clip content (frames only) |
| Text | Texts only: font size, font weight (the nine weights 100 to 900, a select) and text alignment (left, center, right) |
| Fill | Type (solid, linear gradient, radial gradient, image, none; a text offers solid and none), a colour or the stops of a gradient, or the image picker (section 3.2a) |
| Border | One uniform border: width in px (0 is no border), colour, and position (inside, center, outside) once the width is above 0 |
| Shadow | Up to four, each with X, Y, blur, spread and colour, and a button to remove it; a button to add one |
| Export | Below Properties, for one selected frame: PNG scale (1x or 2x) and Export as PNG, with the file name and the size in pixels shown before saving; HTML + Tailwind, which opens the dialog (Tailwind or plain CSS, preview, Copy, Download, and the PNG tab) |

The fields follow the round 2 model (E3.2): margin, per-side padding, gradients, opacity and no fill. Layout changes its fields with the direction. A variable button sits at the end of the Fill colour row and on the caption row of Opacity and Corner radius, the three fields that can be bound; border and shadow colours take a literal only. An agent can set the same fields with `apply_commands`, and a panel control and the tool share one command.

### 3.1a Not in W1 `[P]`

Each item is recommended in decision 40 and waits for the owner. The one-line reason is the same in every case: the model, the canvas or the export would have to change first, and the panel only dispatches what the model already holds (principle 3).

| Not in the panel | Why | What it would take |
|---|---|---|
| Grid layout (flow "grid", columns, rows) | `layout` is free, row or column; the layout core and the export have no grid, and the fidelity contract lists CSS grid as unsupported until the core and oracle fixtures exist | Its own epic box after W1: model fields, layout core, canvas, export rows and oracle fixtures |
| Border per side, and the "sides" field | The border is one uniform width, colour and position; per-side border is unsupported in the fidelity contract | A contract change first, then model, canvas, export and oracle |
| A visibility switch on a node, a border or a shadow, and a page-level visibility | The model has no visible field on a node, a border or a shadow, and no page-level fields | A model field with render, layout and export meaning: an ADR first |
| A Remove button on the border | A width of 0 is no border and the width field takes 0, with the same command an agent sends; a shadow already has Remove | A small slice if QA finds 0 hard to find (one existing command) |
| PNG scales other than 1x and 2x (0.5x to 4x) | 1x and 2x are what the E4.3 box was checked at; the area limit of 4096 x 4096 px refuses a large frame at the higher scales (a 1440 x 900 frame is over it at 4x) | A change of one constant in `packages/web-document/src/export.ts`, a check in a browser at each scale and the E4.3 text; no export crate change |
| A file name field in Export | The file is named from the frame's name (rename the frame), the Tailwind style sheet has the name the HTML links, and the browser may offer its own name at save | Not planned |
| Shader presets (`shader`, `fill2`) | The canvas draws a shader as its plain box and the export writes one gradient; section 6 leaves shader fills out | Not planned |

### 3.2 Variables in fields

Every bindable field has a variable button at its end. It opens a picker filtered by type and scope, searchable, and typing `$` in a field opens the same picker. A bound field shows an indigo chip with the variable name and, on hover, the resolved value; a detach button returns it to the literal. A missing variable shows in red with its name kept. Adding a variable asks for type, name (with the exported name shown, `--surface-raised`), the value for the first mode, and what it may be used for. A new mode copies the first mode's values and shows them as inherited until edited.

### 3.2a Image fills

The Image type of the Fill switch (offered where the app layer says the node takes an image, `imageAllowed`: every kind but text) opens the image picker below the switch; it lists the images the workspace already holds and does not upload (decision 36: an agent uploads through `upload_asset`, and a browser upload waits for sign-in, M2). An image has no name: a tile shows its thumbnail, the first 8 characters of its id as the visible label, and the type and size in its accessible name (`Image 3f2a91bc, PNG, 12 KB`). The list is in the order of the ids, 24 at a time (each tile fetches its image, and the server's per-client burst is 60 calls), and a Load more button reads the next page; the list stops at 500 images, and then says "The first 500 images are listed; there are more in this workspace." and has no Load more. An empty workspace says "No images in this workspace yet. An agent can upload one."; a failed read says so in an alert with Try again (it asks again for the page that failed, and the app layer decides which, so a page that was not valid is tried again too), and keeps the tiles read; focus stays in the picker after Try again, so Escape still closes it; a thumbnail that did not load (a busy server answers 429 or 503) says "No preview", and a Retry previews button under the grid asks for them again (opening the picker again does too). The tile of the image the node has carries `aria-current`; no tile is a toggle, since a press chooses the image and closes the picker. A tile sets the image of the node as one undo step with the fit the node has (the app layer keeps it, and uses cover for a node that has none) and closes the picker; a node that has an image shows its thumbnail and short id, a Change image button and a Cover, Contain or Fill switch for the fit. The picker is open while the app state says so (`AppState::assets`), and closes when another node is selected. Keyboard: the grid is one tab stop, the arrow keys move by one tile and by a row, Home and End go to the first and the last tile read so far (not of a row), nothing moves while Alt, Ctrl or Meta is held (Alt+Left still goes back), Enter or Space chooses, Escape closes, and focus goes to the first tile on opening and back to the type switch on closing. The thumbnails are the full files (there is no thumbnail route yet), so a workspace with many large images loads slowly.

### 3.3 Colour display

Colour variables show as OKLCH by default, for example `OKLCH(0.99 0.004 150)`, to match the Tailwind v4 palette. The colour picker also offers hex, and "copy as hex" is always available. A Preferences switch changes the default. `[D]` (decision 28). This is display only: how colour is stored and rendered on the canvas is still `tokens-and-variables.md` Q3.

## 4. Agents in the interface

- **Actor colour.** Each actor has a stable colour: the person blue, agents orange, teal, purple. It appears on the layer dot, the touch ring, the review tint and the Activity lane. Actors also differ by name, never by colour alone. `[P]`
- **Layers panel.** A dot and a relative time for the last writer, a "new" badge for nodes an agent added in the current run, and a filter that shows only agent changes (`collab-ux.md` 5.2).
- **Canvas.** A touch ring (2 px, actor colour, fades over 1.5 s; with reduced motion it stays fully drawn until the app layer drops it, 1.5 s after the touch) and a name label. Selection is a 1.5 px outline with square handles and a size badge. Dragging a child of an auto layout frame shows a 2 px accent line across the gap it would land in (the box stays where it is drawn); a drop on the page or in a frame with no layout moves the box with the pointer. Several selected children of one frame drag as one block to that line, in order; the nodes that are not in a flow follow the pointer, and flow children of several frames stay where they are. An arrow key along the flow moves the selected child one place (the key across does nothing). A handle drags the drawn box, a hug or fill side that changes becomes fixed, and a child of an auto layout frame keeps its drawn origin while a west or north handle moves (the flow places it). The field that edits a text carries its weight and alignment. "Auto layout on canvas" is a switch in the command palette (Ctrl+K or Command+K): on by default, remembered in the browser, and `/?canvaslayout=0` or `=1` forces it for a page; with it off the canvas draws the stored boxes and none of the above applies. Rings are ephemeral events (ADR-0009).
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

1. **Sidebar** (248 px): New chat; a list of Projects (decision 32), each one repository, a local folder or a GitHub repository, with its chats beneath and diff counts `+N -M`; Quick chats; a Next step card (for example "2 batches need review before you export"); the account.
2. **Chat pane** (352 px): chats only. Its tab row holds chat tabs and its + is New chat. It has an agent picker, messages with batch cards (Show, Revert, Accept), a files-changed line, tool prompts from the agent (allow once, always in this chat, deny; these are the agent's own prompts, separate from a gate Approval), and the composer with context chips (file, node, variable), a mode (Ask, Edit, Auto), and model and effort. The composer lives here in both modes.
3. **Work area, Design mode:** tabs for designs, with the Files home as the first tab. The + menu offers New design; presentation and motion appear in it as disabled "later" items (decision 3). The canvas fills the area; the inspector and the tool bar float over it exactly as on the web.
4. **Work area, Code mode:** on the left, tabs for file viewers (a file opened once is a preview tab, in italics) and terminals; the + menu offers Open file and New terminal. A viewer marks changes against the base as green and red lines and has an Open in editor control. On the right, the file tree (280 px) with the chat's branch, tabs All files, Changes (with a count) and Exports, and a button to hide it. An export that writes HTML and Tailwind into the working copy appears under Changes.

The title bar is split into three cells by vertical rules: window controls and sidebar toggle; repository and chat name with a collapse button for the chat pane; and the work area's tab strip with the **Design** and **Code** switch at the right. The switch is two labelled buttons; shortcuts show only in a tooltip on hover. Each mode remembers its own panel state, and a saved view stores the mode and which panes are open.

Agents speak ACP to a program on the person's machine, Claude Code first, with a custom ACP command allowed. Design changes from a chat arrive over MCP like any agent's: they pass the gate and appear as batches, so Review and Revert work unchanged. The chat adds conversation, code access and tool prompts only.

**First run** `[P]`, five steps, drawn on board M2.1 (first-run flow) of the design canvas:

1. Sign in through Gatehouse. The app opens the browser and returns; there is no password field in the app.
2. Pick or create a Workspace (picked for the person when there is only one).
3. Authorize this computer: an OAuth consent that lists the risk tiers (Read and Change Auto, Delete Ask) for agents the app starts. The app keeps the short-lived token in the system keychain; nothing is shown or copied. The person revokes it under Settings, Agents (board M2.3).
4. Connect the agent: the app finds Claude Code and starts it over ACP. The agent keeps its own sign-in and Meldframe never sees those credentials; if the agent is signed out, the button starts the agent's own sign-in.
5. The base page: repositories and chats on the left, the chat pane, and the design files, each with an empty state that offers the next action.

The web editor has no steps 3 and 4. An agent that runs elsewhere gets the MCP endpoint, adds it, and starts its own OAuth sign-in; the browser shows a consent screen with the same tiers, and the agent then appears as an actor in Activity. Nobody creates or stores an API key (decision 31). Both paths end in the same room and pass the same gate, so Activity, Review and Revert behave the same. Each step also has a waiting, failed or empty state, drawn on board D.4 (desktop first launch).

Decided (decision 32): the sidebar word is Projects, and one Project is one repository. Open: whether a repository is only where code is read and written while the design stays in the workspace (the working assumption); whether one chat means one working copy and branch; whether Start preview (run the repository's app) is in scope; whether Code mode is in the first desktop build.

## 9. Rendering `[P]`

Svelte 5 with Tailwind draws the chrome and Bits UI style primitives where needed; PixiJS v8 draws the canvas. One world container flagged as a render group acts as the camera, so pan and zoom change one matrix. Selection, size badge and touch rings sit in an overlay container that is not scaled, so strokes stay 1.5 px at any zoom. PixiJS v8 does not cull by itself: run the culler when pan or zoom ends. Frames take pointer events; overlays take none. Text on the canvas is measured by the core and drawn by PixiJS; editing text puts a DOM field over the node.

## 10. Open questions

- Q1 How are OKLCH and Display P3 stored and drawn (`tokens-and-variables.md` Q3)?
- Q2 What does a saved view store beyond open panels and the active tab?
- Q3 Is the Review view's before and after compare in v1 (`collab-ux.md` Q8)?
- Q4 Desktop: the working copy per chat, Start preview and Code mode in the first build (section 8). The sidebar word and one project per repository are decided (decision 32).
- Q5 Do comments return on the web, and if so as notes to an agent only (section 7)?
