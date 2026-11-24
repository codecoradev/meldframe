# Canvas

The PixiJS v8 canvas of the web client. It draws the scene it is given and reports what the user does as
intents; it holds no document state and applies no edit (`docs/standards/modular-thin-ui.md`, U1 and U2).

| File | Role | Pixi |
|---|---|---|
| `scene.ts` | `Scene` of `SceneNode`s (`space: 'parent'` or `'world'`), `resolveScene` with an incremental patch, depth-ordered `hitTest` | no |
| `camera.ts` | pan, zoom around a point, limits (2% to 6400%), fit, wheel and pinch, zoom readout | no |
| `overlay.ts` | selection box, handle positions, size badge text and place, touch ring geometry and fade | no |
| `interaction.ts` | pointer, wheel and key state machine: input in, intents out | no |
| `packages/web-input/src/hostinput.ts` | the host element's DOM events turned into calls on the interaction (listeners, pointer capture, cursor, double click, wheel, keys) | no |
| `stage.ts` | world container (render group, the camera), one display object per node, manual culling | yes |
| `overlayview.ts` | paints the overlay (not scaled by the camera) | yes |
| `Canvas.svelte` | props, events, resize and device pixel ratio, frame loop, cleanup | yes |
| `../harness/` + `src/harness/` | development page `harness/canvas.html` (not in the product build) | via Canvas |

Scene space: a node's `x`, `y` are relative to its parent (`'parent'`, the document model's form) or already
world coordinates (`'world'`, the form of the engine's layout rects). Both resolve to the same result.

Hit testing is done on the scene model (`hitTest`), not by Pixi's event system, so Pixi display objects take no
pointer events (`eventMode = 'none'`). One pointer move costs a scan of the scene, not a Pixi traversal.

## Scripts (need the dev server and a global Playwright; not part of `npm test`)

- `node packages/web-e2e/scripts/canvas-e2e.mjs [url]`: click, drag, cancel, resize, wheel, pinch, hover, no leaked listeners over
  three mounts, device pixel ratio 2 and the 2D canvas fallback.
- `node packages/web-e2e/scripts/canvas-perf.mjs [url] [nodes]`: build time and frame times with generated scenes.
- `node packages/web-e2e/scripts/shadow-perf.mjs`: frame times of a canvas of 10, 100 and 500 nodes with a blurred, a hard and no shadow (idle, pan, zoom); it starts its own server and Vite (needs the server binary and the WASM bridge) and, in a container, measures a software rasteriser: read the growth with N, not the milliseconds.
- `node packages/web-e2e/scripts/canvas-shot.mjs <out.png> [query] [url]`: a screenshot of the harness.

## Performance (measured 16 Nov 2026)

Headless Chromium, software GL (SwiftShader, no GPU), 4 vCPU, 1440 x 900 at device pixel ratio 1, generated
scene of frames with 50 rectangles or texts each (`src/harness/sample.ts`, `manyScene`).

| | 500 nodes | 5,000 nodes |
|---|---|---|
| Build the scene and draw the first frame | 65 ms | 331 ms |
| Nodes drawn at fit (zoom 12%) / at 100% | 500 / 0 | 5,000 / 769 |
| Pan at fit: JS time in the render call, median / p95 | 0.6 / 1.8 ms | 2.1 / 12.8 ms |
| Pan at 100%: JS time in the render call, median / p95 | 0.2 / 0.6 ms | 1.1 / 13.9 ms |
| Culling pass, median / max | 0.5 / 1.1 ms | 3.2 to 5.9 / 10.8 to 44.9 ms |
| Overlay drawing, p95 | under 1 ms | 1.1 ms |
| Frame interval during pan, median / p95 | 16.7 / 117 ms | 16.7 / 267 ms |

The frame interval (requestAnimationFrame to requestAnimationFrame) is dominated by the software rasteriser:
half the frames hit the 60 Hz clock and the slow ones are the CPU drawing the scene pixel by pixel. It is an
upper bound for a machine with a GPU and does not predict one; the JS rows are the part this code controls.
Not measured: a real GPU, WebGPU, a phone, text-heavy scenes.
