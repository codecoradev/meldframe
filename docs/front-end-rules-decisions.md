# Front end rules of lower priority: what to decide before the React migration

Status: **decided, 26 Nov 2026**: the owner accepted every recommendation, questions Q1 to Q11, as written; the record is decision 43 in `docs/decisions.md`, and the text below is the analysis it rests on, unchanged. For GitHub issue 324 (findings T7, T8, T9, T11, T12, T13, T14, T15, T16 and T17 of `docs/audits/2026-11-25-ts-thin-ui.md`; tracking: epic #305, thin UI audit #307, React web front end #308). Written 26 Nov 2026 from a reading of `develop` at `4a3a5b4`. The note was written as a proposal: nothing in the code changed with it, and nothing was built, run or measured for it. Section 4 lists what was not verified. The answers went into `docs/decisions.md` as decision 43, and each item that moves becomes an issue under 1,000 lines with a failing test at the app layer first (section 3).

Why now: `docs/react-web-plan.md` (section 8) says the findings of #324 are decided before slice S12, and the React slices port the files that hold these rules (S4 the palette, S5 and S9 and S10 the panels and dialogs with open flags, S11 the canvas, S12 the Editor page). A rule that moves after its slice is ported twice. Rules used: U1, U6, U8 and M2 of `docs/standards/modular-thin-ui.md` (U1: the app layer holds screen state, rules, actions, effects and events as plain data, and a front end reads state, dispatches actions and carries out effects; U6: input to the app layer is UI-neutral, such as a key press with modifiers; U8: a second front end can drive the same state machine without moving logic; M2: one registry, read by every front end). ADR-0026 (direction A) puts every rule and piece of state of the editor in portable Rust; ADR-0008 says state crosses the WASM boundary as coarse reads plus events and is revisited if that costs too much. The words are those of `GLOSSARY.md`.

How to read the line numbers: they are those of `develop` at `4a3a5b4`, and a few differ from the audit's, which was written at `05a6da8`. The "lines" figures are the audit's estimates; I did not recount them.

## 1. Summary

| Id | What | Recommendation | Lines (audit) | React slices it touches | Owner |
|---|---|---|---|---|---|
| T7a | Editor keys (tools, delete, undo, redo, arrows) | Move: a key input to the app layer | 60 | S12 | Q1 |
| T7b | Panel and overlay keys (Option+1, Option+2, `?`, Ctrl+K) | Move, together with T11 | in T11 | S4, S12 | Q1 |
| T7c | Shortcut table and palette catalogue and ranking | Stay in TypeScript as presentation; revisit at the desktop spike | 230 | S4 | Q2 |
| T8 | Creation, resize and preview geometry | Move, after T9: world-space intents and a resize preview query | 140 | S12 | Q3 |
| T9 | What is under the pointer; scene bounds | Move: one definition in `meldframe-render`, asked through a query | 110 | S11 | Q4 |
| T11 | Open state of dialogs and the two overlays | Move dialogs and overlays; menus, tooltips and popovers stay with the library | 60 | S4, S5, S9, S10, S12 | Q5 |
| T12 | The text being edited | Move begin, commit and cancel; the field's keystrokes stay | 70 | S12 | Q6 |
| T13 | PNG plan and capture request flow | Move, after T16: the plan and the decisions; drawing and upload stay effects | 210 | S2 (done), S12 | Q7 |
| T14 | Subscribe retry and reload guard | Move the subscribe retry; the reload guard stays | 50 of 65 | none | Q8 |
| T15 | Camera, viewport, pointer state machine | Stay in TypeScript as presentation, with one carve-out (selection on press) | 670 | S11, S12 | Q9 |
| T16 | Paint geometry written twice | Move the reach into `PaintedBox`; the shapes stay for now | 45 | none | Q10 |
| T17 | Asset cache policy | Stay in the host; drop the second once-per-id guard | 30 | S11, S12 | Q11 |

About 745 lines would move and about 945 stay if every recommendation is taken (sums of the audit's estimates). Order of the moves, from the dependencies found below: T11 first (S4 and S5 wait for it), then T9 and T8 together, T16 then T13, T7a and T12 any time before S12, T14 at any time (the transport package has no Svelte). Whether the moves strictly come before the port, or panel by panel beside it, is question 5 of the plan; this note assumes the plan's table.

## 2. The items

### T7. Keys, shortcut table, palette (U6, M2, U8)

**Today.** Three places answer a key. (1) `packages/web-document/src/edit.ts`: `toolKeys` (:206) and `keyAction` (:261) map V F R E T to `setTool`, Delete and Backspace to `delete` (`deleteAction`, :244), Ctrl or Cmd Z, Shift Z and Y to `undo` and `redo`, and a plain arrow key with a selection to `arrowKey` (the app decides whether anything moves). `ownsKeys` (:237) and `isControl` (:222) ask the DOM whether a field or a dialog keeps the key. (2) `packages/web-shell/src/keys.ts`: `layoutKey` (:18: Option+1, Option+2 by physical key, `?`, Ctrl or Cmd K) and `readOverlays` and `paletteBlocked` (:31, :40: a DOM scan for `role=menu` and `role=dialog`, plus whether the Export dialog is open). (3) `packages/web-canvas/src/interaction.ts`: `Interaction.key` (:349) answers Space (pan), `+`, `-`, Shift+1 and Shift+2 (fit) and Escape (abandons a gesture). The order the three are tried in is `apps/web/src/Editor.svelte` `onKey` (:266 to :294); the host passes the selection from state into `keyAction`. The table of shortcuts is `packages/web-shell/src/shortcuts.ts` (`shortcutGroups`, :30 to :76), checked against the handlers by `shortcuts.test.ts` (a row nothing answers fails there; a new key is not forced onto the sheet). The palette is `packages/web-shell/src/palette.ts`: `paletteEntries` (:67, the catalogue; `enabled` comes from flags the page passes in: `canUndo`, `canRedo`, `canDelete`, `canExport`, `canVariant`), `score` and `filterEntries` (:88, :106, the ranking) and `paletteKey` (:122). The app layer has `Action::ArrowKey` and the actions the keys map to (`crates/core/meldframe-app-model/src/action.rs`) but no key input, so `meldframe-app-harness` cannot press a key.

**Options.** (a) The key to action map: stay in TypeScript, or move to the app layer as one input that carries the key, the physical code and the modifiers (U6 as written). (b) The panel and overlay keys: they toggle panels (app state: `TogglePanel`) and the help sheet and palette (page state, see T11), so they follow T11. (c) The shortcut table and the palette: stay, or move the catalogue (ids, enabled flags) to the app layer as a list every front end reads (M2), leaving the words (U5) and the ranking to the front end.

**Recommendation.** Split the finding, as the audit's two priorities do. (a) Move. A key press is not per frame, so the boundary cost of ADR-0008 does not apply; the host stops passing state back into a rule (`keyAction(e, selection)`), and the harness and a desktop front end get the keys. Two things stay in the host and are presentation: whether the key belongs to a field or a dialog (`ownsKeys`, a DOM question: the host does not send the key), and the keys of the camera and the gesture (Space, `+`, `-`, Shift+1, Shift+2, Escape), which follow T15. One design point for the issue: the host must call `preventDefault` synchronously, and today it does so for an arrow key only when the file changed (`Editor.svelte` :284 to :291, `documentChanged`); `dispatch` is synchronous, so an event raised in the same dispatch ("the key was used") is enough. (b) Move with T11. (c) Stay: the palette is a list of calls to actions the tool bar and the menus already make (its header says it decides nothing about the file), `enabled` is derived from state the page already receives, and the ranking is the front end's own search. What a second front end lacks is the list, which costs about 230 lines to write again; revisit when the desktop spike (#309) writes its plan of what is shared. Consequence to accept: the zoom and fit rows of the table are handled in the canvas, so one table cannot feed every handler, and the rows need a note of who answers them; `shortcuts.test.ts` must then read the editor keys from the app layer through the bridge instead of from handlers.

**Cost in slices.** (a) S12 ports a smaller `onKey`, with no `keyAction`; moved after S12, one more edit of the React Editor page and a port of `edit.test.ts` cases. S4 ports the palette and the sheet as they are. (c) is nothing now.

**Failing test first, for (a):** a key input `z` with Ctrl makes an undo step; `v` sets the select tool; `Delete` with an empty selection does nothing; an arrow key with a selection and the layout off does nothing and raises no "used" event.

**Owner decision needed:** Q1, move the editor keys and the panel keys to the app layer (the recommendation: yes)? Q2, is the palette catalogue part of the one registry of M2 now (the recommendation: not before a second front end exists)?

### T8. Creation, resize and preview geometry (U1, U6)

**Today.** `packages/web-document/src/edit.ts` computes the geometry of a gesture before the app sees it: `resizedBox` (:19, the edge a handle moves, the opposite one stays, a size clamped at `MIN_SIZE` 1), `worldOrigin` (:59, the origins of the parents added up from the scene's drawn boxes), `containerFor` (:82, the frame under the press, else the nearest frame above it), `createActionFor` (:94, world point to the parent's space, a click sends no size, a drag sends at least `MIN_SIZE`) and `resizeActionFor` (:133). The previews are `movePreviewFor` (:155, which asks the app through `Engine.dropPreview`, once per animation frame through `perFrame` in `coalesce.ts`), `resizePreviewFor` (:168), `topOf` and `previewScene` (:106, :183). The app side is uneven: `Action::DropNodes` carries world coordinates and `meldframe-app-rules/src/drop.rs` decides target and space; `Action::CreateNode` carries a point in the parent's space and a parent id that TypeScript worked out; `Action::Resize` carries a box, and `meldframe-app-rules/src/resize.rs` then decides which hug or fill side becomes fixed.

**Options.** (a) Stay: a gesture's geometry is presentation. (b) Move: intents in world units (a create by drag with the tool and the box; a resize by handle with the id, the handle and the travel), the app picks the container and does the arithmetic, and a resize preview query sits beside `drop_preview`. (c) Move only the final action and keep `resizedBox` for the preview: the same arithmetic twice, which is what U1 is about.

**Recommendation.** (b), after T9, because the container is picked from what is under the press and the app should find that itself from the world point, not be told an id. `previewScene` and `topOf` stay: they only move boxes in the scene being drawn. The preview of a resize becomes a query, once per animation frame, as the drop preview already is; that is one more call of an existing kind, which I could not measure here.

**Cost in slices.** S12 ports the three handlers (`onMove`, `onResize`, `onCreate`, `Editor.svelte` :241 to :263) as thin calls; moved after S12 they are rewritten, with the tests of `edit.test.ts`.

**Failing test first:** a create by drag in a nested frame converts to the parent's space; over a text it picks the nearest frame above; a click with no box uses the default size; a flat box gets a size of 1; the north-west handle moves the origin and keeps the opposite edge; a resize below the minimum keeps the edge.

**Owner decision needed:** Q3, move the geometry of creation and resize to the app layer, in world-space intents (the recommendation: yes, after T9)?

### T9. What is under the pointer, and scene bounds (U1, U8)

**Today.** There are two answers to "what is there". `packages/web-canvas/src/scene.ts`: `resolveAll` (:135, world rectangles, the clip rectangle a node inherits, inherited visibility at :171), `hitTest` (:212, the front-most visible node, a clipped-away part does not hit) and `rectsOf`, `contentBounds`, `captureRectOf`, `captureBounds` (:225 to :277, the last two with the reach of `web-paint`). `Canvas.svelte` hands `hitTest` to the interaction (:198, :297); it is asked on a press (`interaction.ts` :174), for the tool that creates (:185) and on every idle pointer move for the hover (:337). The Rust one is `frame_under` in `crates/core/meldframe-app-rules/src/drop.rs` (:248): the front-most frame under the pointer, with no clip and no visibility, used by a drop. So a click and a drop disagree about clipped parts. The visibility clause of the TypeScript one is dead today: `visible` is read at `scene.ts:171` but `packages/web-link/src/scene.ts` does not copy it and `PaintedBox` (`crates/core/meldframe-render/src/resolve.rs`) has no such field; ADR-0025 (proposed) would add it, and then a hidden node must not be hit in one place and be hit in the other.

**Options.** (a) Both stay, written down as two questions (a click wants any node, a drop wants a frame). (b) One definition in `meldframe-render`, next to `PaintedBox`, asked through a small query that reads no state (the point in, an id out); press, create, drop and hover all use it, and the TypeScript `hitTest` goes. (c) One rule, two implementations, pinned by shared fixtures, as `crates/core/meldframe-render/tests/fixtures/scene.json` already pins `sceneFromBoxes` between Rust and TypeScript.

**Recommendation.** (b). The press, the create and the drop happen once per gesture, so the boundary cost is nothing. The hover is the only per-pointer-move caller: ask it once per animation frame, on the latest point, as the drop preview is. Gate: the slice measures the hover at a stated node count (the editor's own `canvas-perf.mjs` and `shadow-perf.mjs` are the places to extend); if it does not fit a frame, hover alone uses (c) and the rule stays in one place with a test. `resolveScene` and its diff (the part that keeps unchanged nodes by identity for the drawing) are a rendering optimisation and stay. `contentBounds` and `captureBounds` move with T16.

**Cost in slices.** S11 wraps `Canvas` with `env.hit` as a bridge call; moved after S11, that wrapper changes again.

**Failing test first**, in `meldframe-render`: the front-most node wins; a clipped-away part does not hit; a property that the frame found for a drop is the frame the hit test finds (`drop_frame_under_agrees_with_hit_test`), and, once ADR-0025 is accepted, a hidden node does not hit.

**Owner decision needed:** Q4, one definition of "under the pointer" in Rust, with the hover measured before it is final (the recommendation: yes)?

### T11. Open state of dialogs and overlays (U1, U8)

**Today.** Some open states are the app's and some are the page's. The app owns the Export dialog (`state.export.dialog`, `Action::OpenExport` and `CloseExport`, `action.rs` :248, :254), the Export variables dialog (`state.variablesIo.export`, `OpenVariablesExport` and `CloseVariablesExport`, :257, :259), the bind picker and the image picker. The page owns the Manage view and the Import variables dialog (`packages/web-panels/src/VariablesPanel.svelte`: `manageOpen` at :33 with the rule that opening it clears the search, :35 to :38, and `importOpen` at :41) and the help sheet and the palette (`Editor.svelte` :81, `overlay`). `Action::CloseVariablesImport` (:271) exists with no matching open action, so the open and the close of one dialog are on two sides. The palette key is blocked by `paletteBlocked(readOverlays(document), snap.state.export.dialog !== null)` (`Editor.svelte` :272, `keys.ts` :31 to :42), which mixes a scan of the DOM with one flag of app state.

**Options.** (a) All dialogs and the two overlays become app state (an overlay field; open actions for Manage and Import; a rule that opening Manage clears the search). (b) Stay as page state. (c) Dialogs in the app; menus, tooltips and popovers stay with the component library, because their open state is focus and DOM behaviour.

**Recommendation.** (c): dialogs and overlays move, popups do not. The rule "Ctrl+K is ignored while a dialog is open" then reads app state only; an open menu is handled as `ownsKeys` is, by the host not sending the key. The state-driven dialog already works in React: the S2 Export dialog is driven by `state.export.dialog` on Base UI's Dialog, with focus return handled by the page (`packages/web-chrome/src/focus.ts`), which stays presentation.

**Cost in slices.** The open flags are in S4 (palette), S5 (`VariablesPanel`), S9 (Manage), S10 (Import) and S12 (`Editor`). Landing T11 first turns each `$state` into a prop; landing it after turns five already ported files over again. It is the item with the largest effect on the order of the plan.

**Failing test first:** opening Manage clears the search; Ctrl+K is ignored while the Export dialog is open (today that needs a DOM); one overlay is open at a time; closing Import with a result clears the result.

**Owner decision needed:** Q5, dialogs and the two overlays in app state, popups in the library (the recommendation: yes, before S4)?

### T12. The text being edited (U1)

**Today.** `Editor.svelte` :137 to :168 holds the edit: `editing` (the node and the text it had), `editedNode` (:139), `editBox` (:142, the field's place), an effect that closes the field when the node is deleted (:151), `startEdit` (:154, selects then opens) and `endEdit` (:160). The rules are in `packages/web-document/src/textedit.ts`: `textTarget` (:10, only a text node), `editKeyTarget` (:19, Enter on exactly one selected text, not on a control) and `commitText` (:27, an unchanged text makes no action). `textBoxStyle` (:44, camera to display pixels) is placement. The field is `apps/web/src/TextEditor.svelte` (73 lines). The app layer has `Action::EditText { id, text }` and nothing for beginning or cancelling.

**Options.** (a) Stay: a draft in a field is view state. (b) Move begin, commit and cancel to the app layer (`state.textEdit`, closed by the app when the node goes), and keep what is typed in the field: the caret, the keystrokes and the composition of an input method never cross the bridge.

**Recommendation.** (b). The rules (only text nodes, unchanged text makes no undo step, a deleted node closes the field) are editor rules a second front end would write again, and the app already knows when the node is deleted. The triggers are Enter (T7a) and a double click (`onActivate` of the canvas).

**Cost in slices.** S12 ports the Editor page and `TextEditor` with a thinner edit; after S12 the edit is ported twice.

**Failing test first:** beginning an edit on a rectangle is refused; committing unchanged text makes no undo step; deleting the edited node closes the edit; a commit makes one `EditText`.

**Owner decision needed:** Q6, the text edit's begin, commit and cancel in the app layer (the recommendation: yes)?

### T13. PNG plan and capture request flow (U1)

**Today.** `packages/web-document/src/export.ts`: the scales (`EXPORT_SCALES`, :8), the pixel limit (`EXPORT_MAX_PIXELS`, :12, 4096 by 4096), the rounding of the size, `planExport` (:21), `largestFittingScale` and `exportFileName` (:31, :45). `packages/web-export/src/png.ts`: `pngPlan` (:29), `pngText` (:45) and `savePng` (:62). The chosen scale is page state (`Editor.svelte` :114, `pngScale`) and is handed to the Export section and dialog as props, with the plan (`planOf`, :115), which reads the canvas's capture box (`canvas.captureBox`, which is T9 and T16). The agent's capture request: the transport package passes the `captureRequest` frame straight to the page (`web-transport/src/shell.ts` :315, `main.ts` :48), so the app layer does not know a request exists; `packages/web-document/src/capture.ts` (`planCapture`, :8) and `capture-serve.ts` (`serveCapture`, :31: plan, wait for `minSeq` with `waitForSeq` :56, plan again, draw, upload, the codes `node_not_found`, `too_large`, `stale_client`, `internal`) hold the sequence. The server checks the limit too (`crates/features/capture/capture/src/relay.rs` :307) and sends it in the request as `maxPixels` (`meldframe-protocol` `wire.rs` :240), so the capture flow needs no constant of its own; the PNG export does (T3, #321 is about copied constants).

**Options.** (a) Stay: it is the host's work (draw, save, upload). (b) Move the plan and the decisions: `state.export` gains the scale and a plan, a capture request becomes a frame the app decides on (is the file at `minSeq`, is the node there, does it fit), and the drawing and the upload stay as an effect that the host answers (`Effect::RenderCapture { box, scale }`).

**Recommendation.** (b), after T16: the plan needs the capture box, so it needs the paint reach in Rust (`paint_bounds`). The decision part is rule: when to answer `stale_client`, when to plan again, which code to send. The drawing needs Pixi and stays an effect. A headless capture later (decision 18) would need the same decisions without the canvas, which is a further reason.

**Cost in slices.** S2 is merged: its section and dialog take `scale`, `onScale` and `plan` as props, so moving the plan into state edits the two switch files and the React copies' props (small; the rules in `png.ts` and `ui.ts` are shared, not forked). S12 ports the capture subscription (`Editor.svelte` :98 to :109) thinner. If it does not move, nothing changes.

**Failing test first:** the plan for a too large frame names the largest scale that fits; a capture request waits until `minSeq`; a request for a node deleted meanwhile fails with `node_not_found`.

**Owner decision needed:** Q7, the PNG plan and the capture decisions in the app layer, after T16 (the recommendation: yes)?

### T14. Retry and reload policies (U1)

**Today.** `packages/web-transport/src/shell.ts`: `MAX_SUBSCRIBE_RETRIES` 3 and `RETRY_FALLBACK_MS` (:59, :60), and `refused` (:363 to :382): `rate_limited` and `unavailable` wait `retryAfterMs` (else 1 second) and subscribe again up to three times, then `lost()`; `bad_request` waits for the resync marker and warns once; anything else is lost. At :421 an error body with no `retryable` flag counts as retryable. `packages/web-document/src/file.ts` holds the reload guard (`RELOAD_LIMIT` 3 in 60 seconds, :4, `shouldReload` :13, `reloadOnDivergence` :26), called by `apps/web/src/main.ts:42` with `sessionStorage`. The app layer already has `Link` and `backoff` (`crates/core/meldframe-app-rules/src/link.rs`) and `Retries` with `MAX_RETRIES` 3 for refused ops (`retry.rs`), so the number three is decided in two languages.

**Options.** (a) Stay in the transport package. (b) Move the subscribe refusal: the error frame goes to the app, which answers with a resubscribe effect or `Disconnected`, and the default for an error with no flag is its rule; the Rust harness, a real second adapter of the effect seam, tests it. The reload guard is a different case: its counter has to survive the page reload it limits, so it lives in `sessionStorage`, which only the host can reach.

**Recommendation.** Move the subscribe retry and the default for an unreadable error (about 50 lines); keep the reload guard in the host and record that as the reason (about 15 lines). When the guard says no, the app is already in its halted connection state (`Connection::Halted`, set on `Event::Diverged`, `crates/app/meldframe-app/src/frames.rs:81`) and the page shows the banner with a Reload button (`packages/web-feedback/src/connection.ts:38`), so nothing a person sees depends on where the guard lives.

**Cost in slices.** None for React: the transport package has no Svelte, and `main.ts` changes in S12 by one line in either case.

**Failing test first**, in `meldframe-app-harness`: a subscribe refused as rate limited three times goes offline; an error with no retry flag is retried by the app's rule.

**Owner decision needed:** Q8, move the subscribe retry to the app and keep the reload guard in the host (the recommendation: yes)?

### T15. Camera, viewport and the pointer state machine (U1, U6, U8)

**Today.** `packages/web-canvas/src/camera.ts` (131 lines: the transform, zoom limits 0.02 to 64, `fitRect`, wheel and pinch meaning), `interaction.ts` (384: the modes idle, pan, press, move, resize, create, pinch and dead; the 3 px drag threshold at :15; selection on press at :166 to :182 and on release at :276 to :283; the keys of the camera at :349), `overlay.ts` (handle sizes and reach), `packages/web-document/src/fit.ts` (when to fit again once the fonts have settled), `packages/web-shell/src/zoom.ts` (the zoom menu's arithmetic) and the fit and refit bookkeeping of `Editor.svelte` (:176 to :227). The camera is a `$state` of the page bound to the canvas (`Editor.svelte` :66, `bind:camera`). No camera or gesture state is in `AppState`. The packages have no framework and no DOM types: input arrives as structural events through `packages/web-input/src/hostinput.ts`. `Engine.dispatch` (`packages/web-link/src/engine.ts` from :80) serialises the action to JSON, reads the events and, when there is any event, reads the whole `AppState` again and tells every subscriber; a camera that is an action would pay that at each pan or wheel event (the audit's argument; not measured here).

**Options.** (a) Stay in TypeScript as presentation, in packages that keep their structural input so a desktop front end on React can import them, and say what the cost is. (b) Move the camera and the pointer machine to the app layer, with a cheap path for per-frame events. (c) Stay for now and decide when a front end that cannot run TypeScript is planned.

**Recommendation.** (a), with one carve-out that moves: what a press selects (and so the hit, T9), which is not per frame, so that the selection rule is the app's. The camera, the viewport, the wheel and pinch meaning, the drag threshold, the handle geometry and the pointer modes stay: they are what a pointing device does, they change per frame, and they are not file or editor state. The honest costs, to be recorded with the decision: a second front end must reuse these packages or write about 670 lines again (the audit's figure); the Rust harness cannot drive a pointer gesture (U8 is met for keys and actions, not for gestures); and the camera is the one piece of state outside the app layer, an exception to direction A of ADR-0026 that is recorded as one. The desktop front end of #309 (GPUIX with React) can reuse them only if the spike confirms that these packages run in its runtime, which I did not verify. A form that keeps the door open: keep `web-canvas` and `web-input` free of the DOM and of any framework (the boundary check of `npm run boundaries` already enforces the framework half), and revisit when a front end that is not JavaScript appears. The form of the record (a decisions row, a note in ADR-0026 or an ADR of its own) is the owner's.

**Cost in slices.** None extra: S11 wraps the unchanged canvas, and `camera` stays a controlled prop with a callback, as the plan says. The carve-out is part of T9.

**Owner decision needed:** Q9, the camera and the pointer machine stay in TypeScript as an exception to direction A, with the cost recorded above (the recommendation: yes), and in what form is the exception recorded?

### T16. Paint geometry written twice (U1, fidelity)

**Today.** `packages/web-paint/src/border.ts` (`borderRing`, `borderReach`), `shadow.ts` (`shadowShape`, `blurReach`, `shadowBounds`, `blurFilterSettings`), `gradient.ts` and `image.ts` (248 lines) work out "as CSS does" what the canvas draws; the export (`crates/features/export/export`, `decl.rs`, `tw.rs`) writes declarations and lets the browser compute the shapes, so the export holds no such geometry and Rust holds none either. The one place where the geometry decides something beyond the drawing is the reach: `paintedRect` in `scene.ts` (:251) decides the rectangle a capture and a PNG export cover. One correction to the issue and the audit, which say the fidelity oracle covers the agreement: I read `tools/fidelity-oracle` and found no run of `web-paint` or of the canvas; its paint probes check the exported pages in Chromium against the model semantics of the contract (`probes.mjs`, line 3), and the contract says the canvas path is not run by the oracle (`docs/fidelity-contract.md`, section 7, "What the oracle cannot prove"). The canvas is checked against the same semantics by `packages/web-e2e/scripts/border-e2e.mjs` and `shadow-e2e.mjs`, run by hand, plus the unit tests of `web-paint`; the one automatic canvas-against-CSS comparison is the blur falloff.

**Options.** (a) Stay as it is. (b) Move the reach: `PaintedBox` gets a `paint_bounds` rectangle computed in `meldframe-render`, and `scene.ts` reads it (about 45 lines). (c) Move the shapes as well (`RingShape`, `ShadowShape`, the gradient line, the image rectangle) into Rust, ship them in the scene, and have the oracle compare those numbers with what Chromium paints from the exported CSS.

**Recommendation.** (b) now. (c) is the only way to make "one place" true for the shapes, and it would also make the canvas's agreement with CSS an automatic check instead of a hand-run one; but it grows the scene JSON per node with a border or a shadow, moves more code to the app layer (the audit counts 248 lines for `web-paint` and 45 for the reach), and needs a new kind of oracle probe, so it deserves its own decision. What stays under both: drawing with Pixi, the blur filter's settings (they depend on the zoom) and the caps of the reach.

**Cost in slices.** None: `web-paint` is not ported and S11 wraps the unchanged Pixi code.

**Failing test first**, in `meldframe-render`: `paint_bounds_include_outside_border_and_blur` for the border positions and for a shadow with offset, spread and blur, compared with the numbers `web-paint` gives today.

**Owner decision needed:** Q10, the reach moves into `PaintedBox` now (the recommendation: yes); do the shapes follow later so that the oracle can check the canvas (the recommendation: decide after the reach has landed)? The acceptance line of #324 ("one place for the paint geometry that the export and the canvas share") should be restated as one of these two, since the export has no geometry to share.

### T17. Asset cache policy (U1)

**Today.** `packages/web-assets/src/assets.ts`: the cache of decoded images with `RETRY_AFTER_MS` 10 seconds (:35), `retryProblems` (:78) and a report-once set (`reported`, :41, :125); `apps/web/src/canvas/Canvas.svelte` has the request timeout (`ASSET_TIMEOUT_MS` 15 seconds, :81) and the retry interval (`ASSET_RETRY_EVERY_MS` 5 seconds, :82); `Editor.svelte` has a second once-per-id set (`shownAssets`, :195, `onAssetProblem` :196). The asset id check exists twice (`assets.ts:29` and `web-transport`, T3). The app layer has asset state for the picker only (`state.assets`).

**Options.** (a) Stay in the host. (b) Move the problem reports into the app layer.

**Recommendation.** (a). A cache of decoded `ImageBitmap`s, a fetch with a timeout and a retry timer are rendering resources and transport, with no editor rule and no state a second front end would share; the notice's words are the front end's (U5). Do remove the second guard: `AssetCache` already calls `onProblem` once per id, so `shownAssets` is a duplicate unless the cache is rebuilt on a remount, and the issue should find out which (not checked).

**Cost in slices.** None: S11 wraps the canvas, S12 drops one set.

**Owner decision needed:** Q11, the asset cache policy stays in the host (the recommendation: yes)?

## 3. Issues to open after the owner answers

Each is one issue under 1,000 lines with the failing test first, at the app layer or in `meldframe-render`, and each moves a rule the answer says moves: the panel and overlay state with the editor and panel keys (T11, T7b; first, S4 and S5 wait for it), the editor key input (T7a), the hit test in `meldframe-render` (T9) and then the creation and resize intents (T8), the paint reach (T16) and then the PNG plan and the capture decisions (T13), the text edit (T12), the subscribe retry (T14). T7c, T15, T17 and the reload guard produce no code issue, only the record of why they stay.

## 4. What was not verified

- Nothing was built, run or measured. The cost of the bridge (a state read at each dispatch that raises an event, a hover query per animation frame) is from reading `packages/web-link/src/engine.ts`, not from a timing.
- The "lines" figures are the audit's estimates, not recounted; the audit's line numbers were re-read and some corrected.
- That the desktop front end (#309) can import the TypeScript packages as they are is not known; it is the spike's question.
- Whether `Editor.svelte`'s `shownAssets` is needed for a remount of the canvas was not checked.

## 5. Questions for the owner, in one list

1. T7a and T7b: editor keys and panel keys move to the app layer as a key input? (Recommended: yes.)
2. T7c: the palette catalogue and the shortcut table stay in TypeScript until a second front end exists? (Recommended: yes.)
3. T8: creation and resize geometry moves, as world-space intents, after T9? (Recommended: yes.)
4. T9: one definition of "under the pointer" in `meldframe-render`, hover measured before it is final? (Recommended: yes.)
5. T11: dialogs and the two overlays in app state, popups with the library, before S4? (Recommended: yes.)
6. T12: the text edit's begin, commit and cancel in the app layer? (Recommended: yes.)
7. T13: the PNG plan and the capture decisions in the app layer, after T16? (Recommended: yes.)
8. T14: the subscribe retry moves, the reload guard stays? (Recommended: yes.)
9. T15: the camera and the pointer machine stay in TypeScript as a recorded exception to direction A, and in what form? (Recommended: yes; the form is yours.)
10. T16: the reach moves now; do the shapes follow later? (Recommended: reach yes; shapes after.)
11. T17: the asset cache policy stays in the host? (Recommended: yes.)
