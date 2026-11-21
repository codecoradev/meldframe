# Fidelity oracle (E4.2)

Loads what the product export makes (the `export` crate, both forms) in Chromium and compares it with the layout core.
The contract it enforces is `docs/fidelity-contract.md`; the spike harness it grew from is `experiments/fidelity-oracle/`
(history, not run).

## What is compared

For each file (hand-written fixtures in `fixtures.mjs`, seeded fuzz files in `fuzz.mjs`):

1. `oracle-runner` (`crates/core/meldframe-oracle-runner`) lays the file out with the layout core and exports each
   top-level frame with `export_frame`, in the plain CSS form and the Tailwind form, and prints the core rects and the
   exports as JSON.
2. The Tailwind form is built with the Tailwind v4 compiler (`css` of the export is its input, the classes of the
   HTML are the candidates).
3. Each page is loaded as delivered (HTML page, stylesheet, font next to it) and every `[data-node]` box is read with
   `getBoundingClientRect`. Fixtures run at 1280 px DPR 1, 700 px DPR 1 and 1280 px DPR 2; fuzz files at 1280 px DPR 1.
4. Paint (`probes.mjs`): pixel probes at fixed points of the border (inside, center, outside), shadow (offset, spread, order),
   image (cover, contain, fill, clipping by a radius and an ellipse), opacity, gradient (angle, stop positions, radial, over the
   fill and under the image), variable (a fill, a radius and an opacity bound to a variable, in the default mode and a picked one:
   `f20` and `f21` carry `variables` and `modes` to the runner) and no-fill (`f22`) fixtures, on both forms, whose
   screenshots must also be identical pixel for pixel; and the falloff of a blurred shadow against the blur the canvas
   draws (Pixi `BlurFilter` as `Canvas` builds it). That last check does NOT go through the export: it compares a hand-written
   CSS `box-shadow` page with Pixi, i.e. how close the CSS blur model is to the canvas's. The shadows the export writes are
   probed on pixels (offset, spread, order), their blur is not.
5. Text ink (`ink.mjs`): 17 texts drawn by Pixi (the way `apps/web/src/canvas/stage.ts` draws them) and by the exported
   page, the edges of the ink compared. The product canvas draws weight and alignment now (the coverage table says
   rendered), but this page is the oracle's own Pixi text, not the canvas: the rows prove the export against the contract's
   canvas, not against `stage.ts`.
6. The gate (`gate.mjs`) fails when a box is more than 0.05 px from the core rect, when a node is missing, when the font
   did not load, when the Tailwind page differs from the CSS page by more than 0.001 px (boxes) or at all (pixels), when
   a probe fails, when an ink edge is more than 1 px away, or when a blur falloff is more than 20/255 away.

Text: the core asks a measurer for the size of every text. The runner replays a table that this harness records in
Chromium with Pixi `CanvasTextMetrics` and Inter (the way the canvas draws text); a query the table lacks comes back as a
miss, the harness measures it and runs again until nothing is missing (two or three runs).

## Known gaps

`known-gaps.mjs` lists the fixtures the export is known not to match. Each entry names its cause: the export's warning that
must still be there and the only nodes allowed to be off. They are checked in both directions: the gate passes while a
fixture fails in that way, and fails on the day it passes, or when it fails in any other way (a font, Tailwind against CSS, a
node outside the cause, an export error), so the entry is removed and the contract updated.

Outside the contract by rule: fuzz files where a layout frame is smaller than its padding, its children overflow it, or a
fill resolves to under 1 px (rule G3; the contract promises nothing there). They are counted, not gated. Fuzz files
get rule G1 applied (a fill under a hug parent is fixed) because the product has no write-time validator for it yet.

## Run

```bash
cargo build -p meldframe-oracle-runner          # target/debug/oracle-runner (ORACLE_RUNNER overrides the path)
cd tools/fidelity-oracle
npm ci                                          # exact versions: playwright 1.63.0, pixi.js 8.23.0, tailwindcss 4.3.5, Inter 5.3.0
npx playwright install chromium                 # once; Chromium comes with Playwright 1.63.0
node oracle.mjs --seeds 50                      # what a pull request runs (npm run oracle)
node oracle.mjs --seeds 500                     # what the nightly runs
node oracle.mjs --seeds 50 --geometry-only      # skip the paint and ink oracles (the slow part is the browser start)
node oracle.mjs f13-overflow fz-007 --seeds 10  # only these files (geometry only)
npm test                                        # the gate on made-up results and the screenshot retry on a fake page (no browser)
npm run sensitivity                             # the gate fails on a wrong export (mutations in geometry and paint are caught; a mutation whose search string is not in an export counts as missed)
```

The gate also fails when the run checks too little: fewer than 21 fixtures, a paint fixture (`PAINT_FIXTURES` in `gate.mjs`) with no probe in either form or no comparison of its two forms, fewer fuzz files than seeds, fewer than 30% of the
seeds inside the contract, a missing or empty paint, ink or blur section, or `--seeds 0`. A fuzz file that carries `fill_under_hug` fails (rule G1 is applied to the fuzz
files). Inside rule G3 only box deltas are excused.

Chromium now and then fails a screenshot with `Protocol error (Page.captureScreenshot): Unable to capture screenshot` after every
numeric gate has passed. Every screenshot of the harness goes through `screenshot.mjs`, which retries exactly that error up to
twice, after 500 ms, and prints `retry n: screenshot protocol error` each time so a recurring flake stays visible. A gate
failure and any other error are never retried; a third failure of the same call fails the run.

`out/` (pages, `results.json`) is git-ignored. A failing fuzz seed prints its file: add it to `fixtures.mjs`.
`CARGO_TARGET_DIR` is honoured when it is set.
