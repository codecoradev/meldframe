# Rune desktop spike: written exit criteria and result record

Status: criteria written for E8.2.1 of `docs/epics.md`, before any Rune code. The result fields are empty. E8.2.3 fills them
with numbers and E8.2.4 adds the verdict and the owner's decision. Nothing in this note is a measurement, a machine or a date.

Sources: E8.2 of `docs/epics.md`, ADR-0027, decision 45, `docs/fidelity-contract.md`. The words are those of `GLOSSARY.md`.

## 1. What this note is, and what a pass means

The desktop front end is native Rune (decision 45). Before any alpha (E8-D) the owner wants a written result for macOS: every
criterion C1 to C10 ends green or as a documented gap with a cost estimate (section 3).

**A pass is input to the owner's decision and decides nothing.** All ten criteria green does not start E8.3, and a gap does
not stop it by itself. The owner reads the result and records go, go with conditions, or stop in `docs/decisions.md`
(E8.2.4). The one exception is not a decision either: a tripwire (section 12) is reported at once and the track does not go
on past the spike without the owner's recorded answer.

How to read the values. Each value or definition carries a tag.

- **[E]** is stated by E8.2.1 or by the fidelity contract. The epic says the owner may change its values before the spike starts.
- **[P]** is proposed here because the epic leaves it open (the number of repetitions, the start of a latency, the way a native
  run makes a throttle). The owner may change it before the spike starts. Section 13 lists every [P] that changes how a result
  reads, so that the owner can answer them in one place.

After the spike starts, a value changes only with the owner's word, an entry in the change log (section 14) and a new run.

## 2. Run record (fields to fill in at the start of E8.2.3)

One record for each machine that produces numbers. Leave a field blank until it is known; never fill one from memory.

| Field | Value |
|---|---|
| Machine (model, chip, memory) | |
| Display (size, scale factor, refresh rate, ProMotion on or off) | |
| Operating system and build | |
| GPU path (adapter name, integrated or discrete, graphics backend as Rune and wgpu report it, a real GPU: yes or no) | |
| Power (plugged in, Low Power Mode off: yes or no) | |
| Rune revision run (pinned by E8.0.4: `048528dc35f0`) | |
| The 8 patch revisions run (wgpu-hal, naga, winit, sctk-adwaita, parley_engine, accesskit_windows, accesskit_atspi_common, accesskit_macos) | |
| Rust toolchain (from `frontends/rune/rust-toolchain.toml`) and Meldframe commit of the run | |
| Reference run: machine, operating system, Chromium and Playwright versions (the Linux oracle, section 3) | |
| Date of the run and operator (a person for C7 and C8) | |

If the Rune revision run differs from the pin, say why; a run on another revision does not count as a result for this note.
A second machine or display gets its own copy of the table.

## 3. How every criterion is measured and recorded

**Scripts.** E8.2.3 commits one entry point for each criterion under `frontends/rune/tools/`, run from a clean checkout of the
repository. They are named `c1-boxes`, `c2-text`, `c3-paint`, `c4-frame`, `c5-hit`, `c6-idle`, `c7-input-method`, `c8-access`,
`c9-first-wheel` and `c10-deps` (with `.sh`; [P], and a rename is allowed if this note is changed in the same pull request).
A script builds what it needs in release mode, takes its inputs from the paths named below, prints its numbers as
`name value unit` lines and one line `Cn verdict: green|gap|tripwire`, and writes the same numbers as JSON to a path the record
names. C7 and C8 have a manual part: the script prepares the file, prompts each step and prints the numbers that can be read
from the server and the app layer; the person's observation is written in the record.

**Reference data.** The fidelity oracle (`tools/fidelity-oracle`) is verified on Linux only (issue 231: on macOS the text
boxes come out whole pixels off the core, cause not found). So the reference numbers for C2 and C3 come from a Linux oracle run
at the same Meldframe commit and are not produced on the Mac. They are a file written by that run (the table of text sizes
recorded in Chromium with Pixi `CanvasTextMetrics` and Inter, the ink boxes of the exported pages, the probe colours and the
CSS blur falloff), never edited by hand, and named in the record with its SHA-256. The Rune side runs on macOS.

**Common inputs.**

- The layout golden fixture `crates/core/meldframe-layout/tests/fixtures/layout_oracle.json` (15 trees, 280 nodes, stub measurer).
- The oracle fixtures of `tools/fidelity-oracle/fixtures.mjs` (`f01` to `f23`, among them the paint fixtures `f07-decor`,
  `f08-image`, `f18-opacity`, `f19-gradient`, `f20` to `f23`), the 17 ink cases of `ink.mjs`, and the text fixtures of
  `crates/core/meldframe-layout/tests/fixtures/oracle_texts.json`.
- The performance files built by the generator of `tools/hit-perf.mjs` (one page frame, `n / 101` clipping frames with 100
  rectangles each), so that C4, C5 and C6 share files with the existing hit measurement.
- A development server and the credential of the existing recipes (`just dev`, from `.dev-token`, never on a command line).
- The window is 1440 by 900 points unless a script says otherwise [P]; scale factors 1 and 2 where a script says so.

**What an evidence record holds** (one for each criterion and each run; the same fields every time):

1. The criterion, its threshold as written here with the tag, and any change from the change log.
2. The run record of section 2 (or its identifier).
3. The exact command line as run, the exit status, and the paths and SHA-256 of the raw output and of the JSON.
4. Every input by path and SHA-256 (fixtures, files, the reference data of the Linux run).
5. The numbers the criterion asks for, in a table, with the sample size (nodes, ticks, calls, repetitions, seeds).
6. The worst case, named (the node id, the fixture, the tick, the input method), not only the aggregate.
7. The verdict (below), and anything that differs from this note (a skipped fixture) with the reason.

**Verdicts.**

- **Green:** every threshold of the criterion is met in the run that is recorded.
- **Documented gap:** a threshold is missed and the record holds all six of: the number measured; the threshold missed; the
  cause, as far as it is known, with the evidence; the work that would close it (a named change, in our crates, in the Rune
  workspace, or upstream, and whether it needs a fork of Rune); a cost estimate in working days as a range, with who does the
  work; and whether a workaround exists and what it costs (a workaround that breaks another criterion says which). A gap
  missing any of these is not documented and the criterion stays open.
- **Tripwire:** a documented gap on a criterion that section 12 ties to a tripwire. It is a gap with its cost, and also stops
  the track until the owner answers.
- **Open:** not measured, or a gap not yet documented. The result cannot go to the owner with a criterion open.

A criterion with no threshold in the epic (C9, and the build time and size of C10) ends as the numbers and a statement.

## 4. C1 Fidelity of boxes

> **C1 Fidelity of boxes.** For the layout golden fixtures and the oracle fixtures, boxes painted from the core's scene are
> within 0.05 px of the core's rects (they are the core's rects; the criterion proves the painter does not move them).

**What it proves.** The Rune painter and Rune's own transforms do not move, round or resize a box that the core placed.

**Measured by** `frontends/rune/tools/c1-boxes.sh`. For each tree of the layout golden fixture and for each oracle fixture, the
script builds the file in the app layer with the layout on the canvas (`App::set_layout_on_canvas(true)`), with the measurer
the fixture uses (the stub for the golden fixture; the recorded table of the Linux oracle for the oracle fixtures), draws
`App::scene()` through the desktop painter in a headless render, and reads the box of every painted node at the lowest level
Rune exposes (the geometry handed to the renderer after Rune's own transform and scale factor, or the element tree dump with
resolved boxes if that is the lowest level). It compares x, y, width and height with the core's rects. The record says which
stage was read, and where in the chain Rune snaps, if it does.

**Runs.** Scale factor 1 and 2 [E: the oracle's DPR]; camera at zoom 1 with no offset, and once with a fractional pan offset of
0.37 px on each axis, so that a painter that snaps to whole device pixels shows [P]; the fixture `f12-fractional` is in the set.
A pixel cross-check of the edge coverage on solid boxes is printed and does not decide the verdict: the contract does not
promise anti-aliasing identity (`docs/fidelity-contract.md` section 2).

**Pass.** For every node of every tree and fixture, at every run above, the largest absolute difference over x, y, width and
height is at most **0.05 px** [E; the contract's L1 and the oracle's `LIMITS.geometry`].

**The record adds:** per fixture the node count and the largest difference with the node id; the stage at which the
geometry was read; the total node count; the largest difference overall.

**A documented gap** names the fixtures and nodes over 0.05 px, the stage that moved them (Rune's snapping, a transform, the
scene to shape step), and the cost of a painter that bypasses it (a draw path of our own, a patch to a Rune crate, or an
upstream change). C1 is part of tripwire T3 (section 12).

## 5. C2 Text

> **C2 Text.** The Rune measurer agrees with the core's line breaks on every oracle fixture and with the exporter's width within
> 0.05 px on single-line samples, and the ink edges of painted text are within 1 px of the exporter's output
> (`docs/fidelity-contract.md`). If Rune rounds text to whole device pixels, the result says plainly whether the contract survives.

**C2a Line breaks and the width of single lines (the measurer).** Measured by `frontends/rune/tools/c2-text.sh`, which plugs the Rune measurer (the `TextMeasurer` implementation on Rune's text
engine, Inter Variable, weights 100 to 900) into the conformance suite of E8.1.7 (`meldframe_layout::conformance`) over the text
fixtures of `oracle_texts.json`. The reference measurer is the table recorded in Chromium by the Linux oracle (section 3), which
is the table that the exporter's `min-width` and `line-height` pins come from. The suite reports, for each fixture and question,
the width difference, the line count difference and the height of one line.

- Pass, line breaks: the line count difference is **0** for every fixture that the contract covers [E].
- Pass, width: the width differs by at most **0.05 px** from the reference on every single-line sample [E]. The single-line
  samples are the hug texts of the ink cases (`hug400`, `hug700`, `hug900`, `hug600-24`, `hug13`, `hug40`, `hug9`, `oneC`,
  `oneR`) and the hug texts of `f09-typography` and `f15-font-sizes` (8 to 96 px). The comparison is on the width the measurer hands to the app,
  after the host's rounding up to whole pixels (`docs/fidelity-contract.md` section 6a, rule 2); the unrounded value is printed too.
- Reported, not judged: the height of one line [P], and every text outside section 4.3 of the contract (a tab, a URL, CJK, the
  fixture `f14-text-edge`): the suite reports them and they never count as a failure, and the record lists them.
- The weight must reach the width: the same text at 400, 700 and 900 gives three widths equal to the reference's [E: contract
  section 6, rule 5].

**C2b Ink edges (the painter).** Measured by the same script. The 17 texts of `ink.mjs` are laid out by the core with the
recorded table, painted by the Rune painter at scale factor 1 and 2 on a white frame, and the box of the ink (the pixels that
differ from the background, by the same rule `ink.mjs` uses) is read from the readback. The reference is the ink box of the
exported page in Chromium from the Linux run.

- Pass: each of the four edges of each ink box is within **1 px** of the reference [E; the contract's L2 and `LIMITS.ink`].
- The `cjk` case is outside the contract: it is reported and does not count [P].

**C2c Rounding to device pixels.** The script paints one single-line text at horizontal offsets 0, 0.25, 0.5 and 0.75 px, at
scale factor 1 and 2, and prints the glyph run origins as Rune hands them to the renderer and the ink box of each readback. If
the origins or the ink are the same for several offsets, Rune rounds the text to whole device pixels. There is no pass line
for this measure. The record ends it with one sentence, in these words: "The contract survives: yes | no | only with
<conditions>", and the numbers behind it. If Rune rounds, the sentence says whether C2b still holds at the fractional positions
the layout produces (`f12-fractional`, `f15-font-sizes`).

**The record adds:** the fixtures run and skipped with the reason; by case, the width difference, the line count difference and
the four ink edge differences; the rounding sentence; the font file and its hash.

**A documented gap** says which texts break differently or sit more than 1 px away, the cause (shaping, hinting, rounding,
the font fallback), and the cost of the fallback named in ADR-0027: painting text from the core's own measure and positions
(the text drawn at the core's boxes, not placed by Rune's text layout), or a measurer table. C2 is part of tripwire T3.

## 6. C3 Paint

> **C3 Paint.** Rounded clip, border per side, gradients and blurred shadows are within the contract's pixel limits against the
> oracle's paint probes.

**Reading of "border per side".** The model has one border (one width, colour and position: inside, center, outside), and the
contract lists a per-side border as unsupported (`docs/fidelity-contract.md` section 4.3; decision 40 item 2;
`docs/per-side-border.md` is a proposal that awaits the owner). This note reads the phrase as the border on every side in all three
positions, which the oracle probes in `f07-decor`. It does not add a per-side border to the model. If the owner means a
border of four different widths, that is a change of the contract first and C3 is changed in the change log before the spike
starts (section 13, point 5).

**Measured by** `frontends/rune/tools/c3-paint.sh`. The script paints each paint fixture through the desktop painter in a
headless render at scale factor 1 (and 2 for the blur) and reads the pixels at the coordinates of the oracle's probes.

- **Rounded clip, border, hard shadows:** every probe of `f07-decor` (border inside, center and outside; the rounded corner is
  clipped; the rounded border on the straight edge; hard shadow offset, spread and order).
- **Gradients:** every probe of `f19-gradient` (linear at 0, 45, 90 and 180 degrees; stop positions; radial; gradient over a
  fill; the clip of a gradient by a radius and an ellipse).
- **Blurred shadows:** the falloff to the left of a 120 by 80 box at blur 8, 20 and 40 px, read at 1, 3, 5, 8, 12, 16, 20 and
  30 px from the box (the geometry of the blur check in `probes.mjs`), against the CSS `box-shadow` of the same geometry from
  the Linux run.

**Pass.**

- Each probe point is within **40/255 per channel** of its expected colour (the oracle's `near`, the contract's L3 solid
  tolerance) [E].
- The blur falloff is within **20/255** of the CSS reference at scale factor 1 and within **28/255** at scale factor 2 [E; contract
  section 2 and section 7; the 28 is the allowance the contract gives to the canvas on a 2x display].
- Reported, not judged [P]: the probes of `f08-image`, `f18-opacity` and `f20` to `f23`, for what the spike view draws.

**The record adds:** probes passed over probes run for each fixture; the colour read at each failed probe; the blur table
(each distance, Rune and reference); whether the node's own box is cut out of its shadow, as the contract says.

**A documented gap** names the failing probes or blur sizes and the cost of a different painter for that feature (a path of our
own for the clip, a shader for the blur, a rendered texture). C3 is part of tripwire T3.

## 7. C4 Frame

> **C4 Frame.** Pan and zoom over 5,000 and 20,000 visible nodes on a real GPU path: no frame over 20 ms in at least 300 ticks
> and p95 at or under 18 ms, with input pumped continuously and a frame monitor, and a 4x CPU throttle run that stays free of
> dropped frames.

**Files.** Two files from the generator of `tools/hit-perf.mjs`: 5,000 nodes and "20,000", built as 19,999 nodes because a file
holds at most 20,000 nodes in all and the generator's page frame counts as one (section 13, point 1). The record states the
real node counts. The camera starts at fit, so that every node is visible, and the layout is on the canvas.

**Measured by** `frontends/rune/tools/c4-frame.sh <5000|20000> [--throttle 4]`, on the real GPU path of the run record (a
physical Mac with a display and Metal; not a virtual machine, not a software adapter; the record says what Rune and wgpu
report).

- A **tick** is one step of the driver: one input event (a drag step or a wheel event) pumped into the window's event loop,
  then one frame requested and awaited until it is presented [P]. Input is pumped continuously with no pause between ticks.
- Two gestures: **pan** (a drag that moves the content back and forth by 25 percent of the window width at 1,200 px/s) and
  **zoom** (wheel steps in and out between fit and twice fit around the window centre) [P]. The paths are fixed in the script.
- At least **300 ticks** for each file and each gesture [E], and the script also runs 1,200 so that the worst frame of a longer
  run is seen [P].
- The **frame monitor** records for each tick the interval between two presented frames, the CPU time of the update and encode
  step, and the GPU time if the backend gives it. The display runs at 60 Hz if the machine allows it (if not, the refresh
  rate is in the record, and the interval limits below stay as written) [P].
- The monitor also records how many times `App::scene()` did layout work and the scene epoch at the first and last tick. This
  is evidence and not a threshold: ADR-0027 reports that resolving the layout on every frame is what a Rune front end must not do.

**The 4x CPU throttle.** A native window has no throttle like the one a browser's tools have. The script makes one: after the
CPU work of each frame (update and encode, not the wait for the GPU) it spins the main thread for three times that CPU time, so
that the CPU part of the frame costs four times as much, and the GPU part is unchanged [P]. A **dropped frame** is an interval
between two presented frames of more than 20 ms [P; the limit of this criterion].

**Pass** [E], for each file and each gesture:

1. No frame interval over **20 ms** in at least 300 ticks.
2. p95 of the frame interval at most **18 ms**.
3. In the throttled run, the same two lines hold: no dropped frame, and p95 at most 18 ms [P: the epic says "free of dropped
   frames" and gives no p95 for this run].

**The record adds:** for each file, gesture and run (plain, throttled): ticks, p50, p95, p99, maximum, count over 20 ms, CPU and
GPU time p95 if known, the layout counter and the epochs; the refresh rate, window size and scale factor.

**A documented gap** says which file, gesture and run missed, the split between CPU and GPU time, the cause as far as
measured, and the cost of the way out (an encoding of the scene that is cheaper to rebuild, a culling pass in the painter, a
cache of a texture of the scene, work upstream). C4 is tripwire T2.

## 8. C5 Hit

> **C5 Hit.** One `App::hit` call through the native app: p99 at or under a quarter of a frame at 60 Hz (4.17 ms) at 1,000, 5,000
> and 19,999 nodes (the thresholds of `tools/hit-perf.mjs`), and a click shows its result within 120 ms.

**Measured by** `frontends/rune/tools/c5-hit.sh`, a native (not wasm) run of the app layer, in two parts.

*C5a, the call.* The same files and rows as `tools/hit-perf.mjs`: 1,000, 5,000 and 19,999 nodes, with the layout on the canvas
on and off; a call over a box (the scan stops at the first box from the front), a call over nothing (the scan reads every
box, the worst case), and the first call after an edit (it builds the scene and the index again). Each row is 4,000 calls
(the default of `hit-perf.mjs`) with its seeded generator, so the points are the same as the wasm run's [P]. The script calls
the `App::hit` of the native app in the process that draws, across no bridge.

*C5b, the click.* In the real window, on the 5,000 node file and the 19,999 node file, the driver sends a pointer down over a
node and the script measures from the arrival of the event in the event loop to the presentation of the first frame that
shows the selection outline and the selection state in the app [P: the epic names no start and no end]. At least 50 clicks for
each file, on different nodes, with the app at rest before each.

**Pass.**

- p99 of a call over a box and of a call over nothing at most **4.17 ms** (a quarter of 16.7 ms) at every size, layout on and
  off [E]. The row for the first call after an edit is printed and not judged, as `hit-perf.mjs` does, and is in the record [P].
- Every click of C5b shows its result within **120 ms** [E; the number of clicks is P].

**The record adds:** for each row: nodes, calls, mean, p99 and maximum in ms; for C5b: clicks, median, p99 and maximum. The
wasm numbers of `docs/front-end-rules-decisions.md` may be shown as a comparison, marked as another run.

**A documented gap** says which row missed and the cost of an index or a scene change. A miss in the call is a problem of the
app layer and is not a Rune problem: the record says so and does not count it against Rune.

## 9. C6 Idle

> **C6 Idle.** With no input, no frame is drawn in 60 s after the first paint, and memory is flat over 180 s.

**Measured by** `frontends/rune/tools/c6-idle.sh`. The script starts the desktop app against a development server, loads the 5,000
node file [P], waits for the first presented frame with the scene (the first paint), and then sends no input at all. The window
stays focused and visible, the server connected, and nothing animates.

- **Frames:** the frame monitor of C4 counts the frames presented after the first paint. Pass: **0 frames in 60 s** [E]. The
  script also prints the count over the full 180 s window.
- **Memory:** the resident set size of the process (and the memory of the GPU if the backend reports it) is sampled once a
  second for 180 s [E]. Pass: the size at 180 s is within 2 percent of the size at 30 s, and the least squares slope over the
  window from 30 s to 180 s is at most 10 KiB per second [P: the epic says "flat" and gives no number].
- Printed, not judged: the CPU use of the process over the window, and the number of wake-ups.

The record says whether a workaround for C9 (a periodic redraw) was on. A workaround that draws a frame at rest fails C6.

**The record adds:** the frame counts, the memory series (first, 30 s, last, slope), the CPU use, and what ran beside the app.

**A documented gap** names what drew (a blinking caret, a pending animation, a socket heartbeat turned into a redraw) or what
grew, and the cost of a fix.

## 10. C7 Input method

> **C7 Input method.** With real input methods on macOS (Japanese kana to kanji, Chinese pinyin, Korean, the emoji picker, dead
> keys) composition shows inline and commits once as one undo step, Escape keeps the draft out of the document, and a node
> deleted by someone else closes the field.

C7 is a manual criterion with a script around it: `frontends/rune/tools/c7-input-method.sh` creates a new file on a development
server, adds a frame and a text node, opens the app on it, prompts each step below, and after each step prints the numbers a
person cannot read: the count of ops of the file on the server and the undo stack of the app layer (which owns
`state.textEdit`, T12). The sign-off is a person on a Mac with the real input sources; an agent can run the script and
cannot judge a candidate window.

**Input sources, each run on its own** [E]:

1. Japanese, Kana, typing romaji or kana and converting to kanji with the space bar, choosing a candidate, Return.
2. Chinese, Simplified, Pinyin, choosing a candidate with a number key and with the arrow keys.
3. Korean, 2-Set, composing a syllable with jamo.
4. The Emoji and Symbols picker (Control, Command, Space) and an emoji inserted at the caret.
5. Dead keys (U.S. International PC: an accent key then a letter; and Option-e then a vowel), a dead key then a space, and a
   dead key then a key that cannot take the accent.

**Checks, for each source** [E]:

- **K1, inline.** While composing, the marked text shows in the field at the caret with the input method's underline, and the
  candidate window sits at the caret (a mispositioned candidate window is a fail).
- **K2, one commit, one undo.** After the commit the file has exactly **one** new op for the whole text, and one undo (Command
  Z) restores the text from before the edit. No op is sent for the marked text, and none while it changes.
- **K3, Escape.** Escape while marked text is shown lets the input method cancel it (the field keeps the text from before it);
  Escape again with nothing marked closes the field, and the file has **no** new op and the draft is not in the file.
- **K4, deleted by someone else.** With the field open and marked text on screen, a second credential with an actor of its own
  (the flow of the M1 demonstration) deletes the node. The field closes, the app layer shows the notice that the node is
  gone, no op is sent for the draft, and the input method is not left in composition.

**Pass:** all four checks hold for all five sources. One failed check on one source is a failure of C7. The record gives the
input source and keyboard layout names as macOS shows them.

**The record adds:** a table of five sources by four checks with the observation, the op count and the undo depth before and
after each step, and a recording (named, with its SHA-256, kept outside the repository unless the owner asks otherwise).

**A documented gap** names the source and the check, whether the fault is in Rune's input method support (winit and its
patch), in the app layer's text edit rule, or in the view, and the cost of a fix in the Rune workspace or a patch. C7 is
tripwire T1.

## 11. C8 Accessibility, C9 Upstream defect, C10 Dependencies

### C8 Accessibility

> **C8 Accessibility.** The AccessKit tree exposes the toolbar, the panels and the node list by name and role, and VoiceOver
> reads the Layers panel.

**Measured by** `frontends/rune/tools/c8-access.sh` and a VoiceOver pass. The script dumps the AccessKit tree of the running
window (the tree Rune gives to `accesskit_macos`) as role, name, and the node's state, for the toolbar, each panel and the node
list; and compares it with an expected table that the record holds before the run [P: the epic gives the elements and not
the names]. The expected table has at least: a toolbar with a name, two panels with the names Layers and Properties, a list with
one list item for each node of the file named with the node's name made safe to display (U9; the hostile names scenario of
`hostile-names-e2e.mjs`), the selected item marked as selected, and the buttons with their names.

The VoiceOver pass is a person: VoiceOver on, move into the Layers panel, step through three nodes, select one, and copy the
spoken text from the VoiceOver caption panel verbatim into the record.

**Pass:** every element of the expected table is in the tree with the expected role and a non-empty name; VoiceOver speaks the
panel, then each node's name and role, and announces the change of selection. The record adds the tree dump (path and SHA-256),
the expected table, the diff and the verbatim VoiceOver text.

**A documented gap** names the elements missing or unnamed and the cost (in the view crate, in Rune, or in the AccessKit patch).

### C9 Upstream defect

> **C9 Upstream defect.** Whether anomalyco/rune#2549 (the first wheel after idle presents late) occurs in the native window,
> recorded either way.

**Measured by** `frontends/rune/tools/c9-first-wheel.sh`. In the native window on the 5,000 node file, the script lets the app
rest with no input for 5 s, 30 s and 60 s, then sends one wheel event, and measures from the arrival of the event in the event
loop to the first presented frame that shows its effect. It sends a second wheel event 100 ms later and measures it the same
way. Ten cycles for each rest time [P], with the app in front and focused.

**Occurs** if the first-wheel latency is more than 100 ms and more than three times that of the second wheel event, in at least
one cycle [P]. There is no pass line: the criterion is the answer. The record gives yes or no and the latencies of every
cycle, and, if yes, whether a workaround removes the late frame and what it costs for C6 (a redraw at rest is a C6 failure).
Nothing is posted upstream without the owner's approval of the wording (ADR-0027).

**A documented gap** (yes, with an effect on use) states the worst latency and the cost of a workaround or of waiting for an
upstream fix. C9 is not a tripwire. A fix upstream is a re-check trigger of the exception for a git source (E8.0.4).

### C10 Dependencies

> **C10 Dependencies.** The exact pins, the licences of every crate reached, the patches that remain, the build time and the
> size of the binary.

**Measured by** `frontends/rune/tools/c10-deps.sh`, which reads the Rune workspace and prints:

- The exact pins: the Rune revision and the revision of each of the 8 `[patch.crates-io]` entries, and every git source of
  `frontends/rune/Cargo.lock` with its URL and revision.
- The licences: `cargo xtask licences --check` over the crates the Rune workspace reaches (every one on the allow list), and
  `cargo deny check` in `frontends/rune` (with its own `deny.toml` that lists the exact git URLs and revisions in `allow-git`).
- The patches that remain: the number beyond the 8, and which of the 8 are still needed at the end of the spike.
- The build time: a clean release build of `apps/desktop` and an incremental one after a one-line change in a `feature-ui` crate,
  in seconds, on the machine of the run record, with the cache state said.
- The size of the release binary, stripped, in bytes (and the bundle if one is built).
- Each bump of the Rune revision made during the spike: the date, the old and new revision, and the working time the bump cost
  (E8.0.4 records each).

**Pass:** the pins equal those of E8.0.4; **0** patches beyond the 8; every licence on the allow list; `cargo deny check` passes
and the main workspace's `unknown-git = "deny"` is unchanged; no main workspace crate depends on a `frontends/rune` crate
(the check of E8.0.4). The build times and the binary size have no line in the epic and are recorded as numbers for the
owner (section 13, point 6).

**A documented gap** is a licence off the list, a patch beyond the 8 (a fork of Rune), or a failing `cargo deny check`, with the
cost of replacing the dependency or carrying the patch. A fork of Rune beyond the 8 pins is tripwire T4.

## 12. Failing conditions that stop the track, and who decides

The tripwires are those of ADR-0027 and of the E8 intro of `docs/epics.md`. They stop the Rune track at the spike or the alpha,
with the fallback named and not a silent slip.

| Id | The track stops when | Seen in | Detected by |
|---|---|---|---|
| T1 | Input method composition fails on macOS with real input methods | C7 | any failed check on any of the five sources |
| T2 | The frame cost of the reference scenes is not met on a real GPU | C4 | a missed threshold on either file, either gesture or the throttled run |
| T3 | The fidelity contract (0.05 px boxes, 1 px text ink) cannot be met by painting from the core's own measure | C1, C2, C3 | a documented gap in C1 or C2 that survives the fallback of painting text from the core's own measure and positions |
| T4 | A fork of Rune is needed beyond the 8 pins | C10 | a patch beyond the 8 in the lock, or a gap that only a fork closes |
| T5 | Rune API churn costs more than one working day a bump over three bumps in a row | C10 (the bump log) | three consecutive bumps, each over one working day, in the log of E8.0.4 |

A C1 or C2 miss that the fallback removes is a documented gap and not T3 (section 13, point 9). If the spike makes no bump,
T5 is not tested and the record says so.

**Who decides.** The owner. The operator reports a tripwire to the owner as soon as a script prints `tripwire`, in writing,
with the evidence record. Until the owner answers, nothing in E8.3 starts. The result note (E8.2.4) says go, go with
conditions, or stop, and this note's default reading is that a tripwire means **stop with the fallback named**: the frozen React
web client (resume S12 and S13), the Svelte page that stays the default, and any other desktop front end on the same protocol
(`/v1/ws`, REST, MCP). Only the owner can record anything else, in `docs/decisions.md`, with the conditions. A stop undoes
nothing in the main workspace: E8.1 and the second workspace stay (E8.2.4).

## 13. Open points and the order of work

**Points the owner may settle before the spike starts** (each is where this note reads the epic, or fills a gap, and says so):

1. C4 says 20,000 visible nodes; a file holds at most 20,000 nodes, page and clipping frames included. The note builds the largest
   file the generator makes, 19,999 nodes, as C5 does.
2. C4 says "4x CPU throttle" and "dropped frames"; a native window has neither. Section 7 defines both [P].
3. C2 says "the core's line breaks" without saying with which measurer. The note reads it as the core's layout with the table
   recorded in Chromium by the Linux oracle (the oracle on macOS fails for a cause not found, issue 231), and does not count
   the fixtures outside the contract (`f14`, `cjk`), as the conformance suite does not.
4. T5 is read as three consecutive bumps that each cost more than one working day.
5. C3 says "border per side"; the model has one border and the contract says per-side is unsupported (section 6).
6. C6 says "flat", C9 gives no pass line, and C10 gives no line for build time and binary size. The note proposes numbers for C6
   and records the others as numbers.
7. C5 says "a click shows its result within 120 ms" without a start and an end. Section 8 defines them [P].
8. C7 and C8 need a text field wired to the app layer's text edit, a toolbar, two panels and a node list with names. The skeleton
   of E8.2.2 lists a window, one cached shape, a Move and an Undo button and a status. The criteria cannot be measured
   until a view with those parts exists. This note does not say which issue builds it (E8.2.2, or E8.2.3 as a spike-only view
   in `frontends/rune/tools/`); the owner or the E8.2.2 pull request settles it before C7 and C8 are measured.
9. A tripwire hit and a "documented gap" are two readings of one miss (C4, C7, C10). Section 12 reads it as both, the owner
   deciding. It also reads T3 as tripped only when painting from the core's own measure (the fallback of the epic's risks) cannot
   meet the contract either.

**Order of work.**

1. **E8.2.1, this note** (docs only; depends on E8.0.1, and ADR-0027 is accepted). Merged before any Rune code.
2. **E8.0.4**, the Rune workspace and the git source exception (`frontends/rune/`, its toolchain, lock, the 8 pins at
   `048528dc35f0`, its `deny.toml` and CI job). It waits for the owner's decision on who bumps Rune.
3. **E8.1.6**, the cached scene and the epoch, and **E8.1.7**, the text measurer seam and its conformance suite. Neither needs
   Rune. E8.1.6 is needed by the skeleton and by C4; E8.1.7 by C2.
4. **E8.2.2**, the skeleton (depends on E8.0.4 and E8.1.6): a window, a connection, `App::scene()` as one cached shape, a click
   through `App::hit`, Move and Undo, a status, and a headless render and tree dump stored as goldens.
5. **E8.2.3**, measure (depends on E8.2.2, E8.2.1 and E8.1.7): one script per criterion under `frontends/rune/tools/`, the Rune
   measurer plugged into the conformance suite, and the results written into this note with the run record of section 2.
6. **E8.2.4**, the result and decision record (docs only; depends on E8.2.3): each criterion green or a documented gap with its
   cost, go, go with conditions or stop with the fallback named, and the owner's decision in `docs/decisions.md`.

Writing a result into this note is not a decision and changes no criterion; a criterion changes only through the change log.

## 14. Results and change log (empty until E8.2.3)

| Criterion | Threshold (section) | Measured | Verdict | Gap cost (working days) | Evidence record |
|---|---|---|---|---|---|
| C1 Fidelity of boxes | 0.05 px (4) | | | | |
| C2 Text | 0 line count difference; 0.05 px width; 1 px ink (5) | | | | |
| C3 Paint | 40/255 probes; 20/255 and 28/255 blur (6) | | | | |
| C4 Frame | none over 20 ms in 300 ticks; p95 18 ms; throttled (7) | | | | |
| C5 Hit | p99 4.17 ms; click 120 ms (8) | | | | |
| C6 Idle | 0 frames in 60 s; flat over 180 s (9) | | | | |
| C7 Input method | five sources by four checks (10) | | | | |
| C8 Accessibility | tree and VoiceOver (11) | | | | |
| C9 Upstream defect | recorded either way (11) | | | | |
| C10 Dependencies | pins, 0 extra patches, licences (11) | | | | |

Tripwires hit: T1 ____ T2 ____ T3 ____ T4 ____ T5 ____ (yes, no or not tested).

Result (E8.2.4): go | go with conditions | stop with the fallback named: ____. The owner's decision, row in `docs/decisions.md`: ____.

| Change to a value or a definition | By whom (the owner) | Reason | Before or after the spike started | Scripts run again |
|---|---|---|---|---|
| | | | | |
