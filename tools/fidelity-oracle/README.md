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
   `f20` and `f21` carry `variables` and `modes` to the runner) no-fill (`f22`) and fill-alpha (`f23`: the alpha of a fill's own colour over white and over a coloured box) fixtures, on both forms, whose
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

7. The importer (`import.mjs`, `gate-import.mjs`; E4.4 slice 8, `docs/fidelity-contract.md` section 10). The runner imports every
   exported frame back with `html_import::map::plan_with` (the css form from its own style block, the Tailwind form with the css
   the export returns), lays the imported file out with the same measurer, and compares it with the original: every box within
   0.05 px, every field the export writes (`compare.rs` of the runner), no refused property beyond the five an export always
   carries (`EXPORT_REFUSED`), and export, import, export giving the same HTML once the ids are renamed. The file the css form
   comes back as is exported again and loaded like any other (`imported` files of the geometry oracle). The hand-written pages
   `h01` onward (`pages.mjs`, and `pages-gaps.mjs` for the known gaps) are loaded as written in Chromium: the box of every
   element with a `data-name` is compared with the layout core's rect of the imported node of that name, and for the pages marked
   `pixels` the pixels of the page against the pixels of the export of its import (inside the root box, none may differ).

Text: the core asks a measurer for the size of every text. The runner replays a table that this harness records in
Chromium with Pixi `CanvasTextMetrics` and Inter (the way the canvas draws text); a query the table lacks comes back as a
miss, the harness measures it and runs again until nothing is missing (two or three runs).

## Known gaps

`known-gaps.mjs` lists the fixtures the export is known not to match. Each entry names its cause: the export's warning that
must still be there and the only nodes allowed to be off. They are checked in both directions: the gate passes while a
fixture fails in that way, and fails on the day it passes, or when it fails in any other way (a font, Tailwind against CSS, a
node outside the cause, an export error), so the entry is removed and the contract updated.

`IMPORT_GAPS` in the same file lists what the importer does not match, in the same two directions: a hand-written page (`kind: 'page'`)
or a fixture imported back from its export (`kind: 'roundtrip'`) with its cause, the finding the importer must still report (or none, when
it is silent), and the only nodes allowed to be off (and `pixels: true` where the gap is a difference of pixels).

Outside the contract by rule: fuzz files where a layout frame is smaller than its padding, its children overflow it, or a
fill resolves to under 1 px (rule G3; the contract promises nothing there). They are counted, not gated. Fuzz files
get rule G1 applied (a fill under a hug parent is fixed) because the product has no write-time validator for it yet.

## Platform

The gate is verified on Linux only: the Chromium of the lockfile on a Linux machine, as in the CI job. As reported in issue 231,
the run on macOS fails with text boxes that are whole pixels off the core (two text-ink cases 2 px off, and the known gap
f14 reported as no longer as stated) with no change to the code, while the paint probes, the equality of the Tailwind and plain
CSS forms and the CSS blur model held. The cause is not found: the candidates are the font file or fallback font that the
Chromium picks, hinting and sub-pixel text positioning, and the text table recorded once per run. So `oracle.mjs` and
`sensitivity.mjs` stop on another platform with exit code 2 and a note (`platform.mjs`), unless `MELDFRAME_ORACLE_ANY_PLATFORM=1`
is set. The run then goes on, and a failure in the geometry or the ink of a text there cannot be told from issue 231, so it neither
clears nor condemns a change: the Linux job decides; a failure in the paint probes or in the equality of the two forms is not that
issue. (`MELDFRAME_ORACLE_TEST_PLATFORM` is for the test of the guard only.) The `oracle` job of a pull request runs it on Linux, which is where a contributor on a Mac gets the answer.

## Run

```bash
cargo build -p meldframe-oracle-runner          # target/debug/oracle-runner (ORACLE_RUNNER overrides the path)
cd tools/fidelity-oracle
npm ci                                          # exact versions: playwright 1.63.0, pixi.js 8.23.0, tailwindcss 4.3.5, Inter 5.3.0
npx playwright install chromium                 # once; Chromium comes with Playwright 1.63.0
node oracle.mjs --seeds 50                      # what a pull request runs (npm run oracle)
node oracle.mjs --seeds 500                     # what the nightly runs
node oracle.mjs --seeds 50 --geometry-only      # skip the paint and ink oracles (the slow part is the browser start)
node oracle.mjs f13-overflow fz-007 --seeds 10  # only these files (geometry only; no import checks)
npm test                                        # the gate on made-up results, the platform guard and the screenshot retry on a fake page (no browser)
npm run sensitivity                             # the gate fails on a wrong export and on a wrong import (mutations in geometry and paint are caught; a mutation whose search string is not in the output counts as missed)
```

The gate also fails when the run checks too little: fewer than 22 fixtures, a paint fixture (`PAINT_FIXTURES` in `gate.mjs`) with no probe in either form or no comparison of its two forms, fewer fuzz files than seeds, fewer than 30% of the
seeds inside the contract, a missing or empty paint, ink or blur section, or `--seeds 0`. A fuzz file that carries `fill_under_hug` fails (rule G1 is applied to the fuzz
files). Inside rule G3 only box deltas are excused.

Chromium now and then fails a screenshot with `Protocol error (Page.captureScreenshot): Unable to capture screenshot` after every
numeric gate has passed. Every screenshot of the harness goes through `screenshot.mjs`, which retries exactly that error up to
twice, after 500 ms, and prints `retry n: screenshot protocol error` each time so a recurring flake stays visible. A gate
failure and any other error are never retried; a third failure of the same call fails the run.

`out/` (pages, `results.json`) is git-ignored. A failing fuzz seed prints its file: add it to `fixtures.mjs`.
`CARGO_TARGET_DIR` is honoured when it is set.
