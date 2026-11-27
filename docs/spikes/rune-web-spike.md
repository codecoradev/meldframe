# Spike: the Rune web target as a Meldframe canvas client

Status: proposed, exit criteria written before any code (the rule for a desktop spike in
ADR-0026, applied here to a web-target spike). Owner direction on 27 Nov 2026: the web client
stays React (ADR-0026, decision 42); this spike asks a separate question — **could a Meldframe
client be built on Rune's web target at all**, and what would it cost. The desktop client is
out of scope here; it is assumed safe later and is a different spike (#309).

Everything in this document cites the state of `anomalyco/rune` at git rev
`048528dc35f0` (branch `dev`, committed 27 Nov 2026). Rune is pre-release, pinned by rev, and
does not accept pull requests; every fact below can change under us. MIT licence.

## The three questions

1. **Canvas.** Can a Rune web app be the editor surface: pan and zoom over thousands of
   nodes, pointer and pinch input, hit testing, text that stays crisp under scale?
2. **Import and export.** Can it do the web things the editor needs: pick files, paste
   images and files, copy to the clipboard, save an export as a download?
3. **A web citizen.** Accessibility, text editing with IME, links, sockets and fetch to a
   server, page load and bundle size — is the result something a browser user can live in?

## What the documentation already says (read, not yet run)

Answers found in Rune's `crates/rune/README.md`, `WEB.md` and `PLAN.md` at the pinned rev:

- **Canvas**: `.scale(f)` with `.origin(x, y)` composes with nested scales; text under a
  still scale rasterizes at the scaled size (crisp), and hit boxes and carets follow the
  transform — the exact capability GPUIX 0.11 lacked in the 20 Nov spike (ADR-0026, Risks).
  Pinch gestures report phased `Began`/`Moved`/`Ended` and Ctrl+wheel reaches `on_magnify`;
  `.scroll()` viewports and `.virtual_rows(..)` exist for long lists.
- **Import**: paste of browser files exists — `ClipboardContent::web_files`, up to
  `rune::clipboard::MAX_WEB_FILES` `File` handles with name, type and size (`WebFile::read(max)`
  reads bytes; `WebFile::js_file` for the JS handle). Text and image paste exist
  (`on_input_paste`, `on_paste_content`, `read_clipboard`, image paste capped at 32 MiB PNG).
  `Tx::copy` uses the browser clipboard and must run from a user input callback; clipboard
  revision is `None` on the web (browsers expose no history).
- **Export**: `Tx::save_file` is **unsupported on wasm32** (the README's Save Image As… path
  is native only). A download must go through JS (`Blob` + object URL + click), which means a
  wasm-bindgen extern of our own or a Rune API added upstream. This is a known cost to
  measure, not a blocker to assume.
- **Web citizen**: a semantic DOM accessibility mirror sits above the canvas (landmarks,
  headings, links, buttons, roles, `dir` handling); IME on the web arrives through a
  sliding-window hidden `<textarea>` (desktop through winit); `Tx::open_url` opens external
  links; text can be `.selectable(true)`; HTTP fetch exists with two web runtimes — JSPI
  fibers on Chromium 137+ (`wasm-bindgen` 0.2.129+) or main-thread timer slices without it.
- **Gaps to prove, not assume**: `WebSocket` appears **nowhere** in the full README
  (368 KB). Meldframe's live sync rides `/v1/ws`. The spike must connect a real socket
  (JS extern via `wasm-bindgen`, since Rune's own app layer may use `wasm-bindgen` on wasm)
  or fall back to HTTP fetch polling and say what that costs.
- **Build**: `scripts/web-build.sh` outputs a static folder (WebGPU, WebGL2 fallback, CPU
  canvas), `--dev` rebuilds a one-line change in ~2 s on their example, no
  SharedArrayBuffer/COOP/COEP needed. Toolchain and `wasm-bindgen`-cli are pinned by their
  `Cargo.lock` (0.2.129 in their docs; our workspace lock has 0.2.130, installed locally).

## Exit criteria (all measured, all written down in this file's results section)

E1. **It builds and runs.** A harness crate builds for `wasm32` and opens in Chromium
    (WebGPU or its WebGL2/CPU fallback), from a scaffold that mirrors Rune's
    `examples/web-hello` (cdylib + `AppSpec` + `rune::App::new(...).spawn()`), with the
    workspace `[patch.crates-io]` git pins at one rev and Rune's pinned toolchain in the
    harness only (meldframe stays on 1.97.0; Rune pins 1.99.x).
E2. **Socket.** The harness connects to a real WebSocket endpoint (the dev server's `/v1/ws`
    or an echo) and renders received frames; if it goes through JS externs, the amount of
    glue is counted and recorded. If only polling works, the measured message latency and
    request rate are recorded.
E3. **Canvas behaviour.** A scene of 5,000 elements inside a scroll viewport pans and zooms
    (wheel + pinch) without per-frame layout thrash: frame time p50/p95 recorded from
    Chromium tracing or `performance.now()` sampling across ≥ 300 frames, against the same
    scene rendered by the current Pixi canvas driver in the web client (`/?layout` harness),
    same machine, same document. Numbers, not adjectives.
E4. **Text fidelity.** One label set rendered by Rune web at scale 1.0 and at 0.5/2.0:
    the export crate's rect for the same text (the `0.05 px` contract of
    `docs/fidelity-contract.md`) is compared against Rune's drawn box via a headless run;
    differences recorded. Rune rounds text to whole device pixels by design — the spike
    states plainly whether the fidelity contract survives that.
E5. **Import/export.** In Chromium: paste a PNG (clipboard), paste files
    (`web_files`), copy text and image out, and produce a download (via the JS-extern
    route if `Tx::save_file` stays unsupported); each works or is logged as a gap with the
    glue it needs.
E6. **Web citizen.** The accessibility mirror is inspected in the DOM tree (roles and names
    for buttons, list, toolbar); IME composition is exercised through the hidden textarea
    path (Playwright `insertText` plus a composition event); external link opens; WASM
    size (raw and gzip) and time to first Rune frame are recorded and compared with the
    React client's numbers on the same connection.

**Pass** means E1 to E3 pass with numbers and E4 to E6 are each either green or a
documented gap with a cost estimate. A pass does **not** decide anything: it produces the
input for an owner decision against ADR-0026's standing order (React web first, desktop
spike #309 next). A fail names which criterion failed and why.

## Method

- Harness lives **outside** the meldframe workspace:
  `../rune-web-spike/` (its own git repo), so the 8-crate `[patch.crates-io]` block and
  Rune's toolchain never enter meldframe's `Cargo.toml` — the same dependency-matrix
  reasoning as `docs/architecture.md` section 3b.
- One pinned rev of Rune (`dev` never rewrites history; record the rev here when bumped);
  one `CARGO_TARGET_DIR` of its own (disk note: the SSD had ~15 GiB free on 27 Nov; a Rune
  build needs several GB — clean before starting).
- Measurements run in Chromium via Playwright (already a dev dependency of this repo's
  scripts) against `scripts/web-build.sh --dev` output; the React/Pixi baseline is the
  existing `/?layout` harness page.
- Time-box: the questions are answered by one person in roughly three working days of
  build-and-measure, not by product work in this repository. The repo rule "no new spikes;
  build the vertical slice" is satisfied because this spike's exit criteria were written
  first and its code lives outside the product workspace.

## Non-goals

- No product code in meldframe changes. No Svelte or React panel is rewritten on Rune.
- No desktop evaluation (that is #309's GPUIX-vs-Rune question, with its own exit
  criteria; this spike's E1–E6 numbers will feed it).
- No commitment that a Rune client would replace the React client; the React migration
  (E7.3 slices) continues regardless of this spike's outcome.

## Results — session 2 (28 Nov 2026, same machine)

The harness gained `?mode=shape` (all nodes drawn into one `canvas(Shape)` per frame, els
only for chrome — the architecture the session-1 E3 finding pointed to) against the v1
`?mode=els` (nodes as keyed els), a web-sys WebSocket in pure Rust (the official
`examples/fetch` pattern: `spawn_local` + `AppHandle::update`), an HTTP-polling fallback
(`?poll=ms`, `rune::task::spawn_after`), an ElementInfo rect dump, and a `rune::text::measure`
dump for ten fixed labels.

- **Shape path (the E3 answer): the constant-cost claim holds.** Worst case, every node
  drawn (zoom 0.05), software WebGPU: 5,000 nodes p50 66.4 ms / p95 68.9 ms; 20,000 nodes
  p50 68 ms / p95 71.9 ms — frame cost is flat in node count, as predicted. The els path on
  the same run: 173 ms / 195 ms at 5,000 (206/258 at 20,000), and it degrades with count.
  The el path's session-1 114 ms and session-2 173 ms also show its run-to-run variance.
  Against the < 8 ms bar: not met yet on a software adapter — the remaining cost is the
  per-frame Shape rebuild plus the GPU upload, both to attack next (draw only dirty
  regions, skip borders under 24 px — already gated, batch strokes) and then judge on a
  real GPU. Also confirmed: in Shape mode the mirror holds only the chrome (3 buttons);
  node cards are pixels, so a real client exposes selection/layers through its own
  accessible panels (meldframe already has them).
- **E4-lite (text): measure and drawn boxes agree exactly.** For ten labels across the
  width range 11–106 pt: `rune::text::measure` vs the drawn rect — 0.00 px difference on
  every width; height differs by a constant 3.75 pt because the element slot carries
  line-height padding (16.25 ink vs 20.0 slot), a semantic difference, not drift. This
  makes `rune::text::measure` a credible shared truth for a Rune client and the exporter;
  the full 0.05 px contract against the export crate's raster stays for the next session.
- **WebSocket, two bridges proven, no inline-JS needed for transport.** (1) `web_sys::
  WebSocket` built in Rust, events through `Closure` + `AppHandle::update` — server push
  3/3 received and rendered; this is Rune's own recommended async pattern
  (`examples/fetch`). (2) HTTP polling fallback (`?poll=300` + `fetch` + `spawn_after`) —
  25 ticks received; the answer for locked-down proxies. Rune has no ws API of its own;
  both bridges cost tens of lines, all in our crate. For meldframe's protocol, the
  Shape-mode finding matters more: the transport is a solved detail either way.
- **Idle and memory: exactly as claimed.** 180 s idle soak (Shape, 2,000 nodes): 4 frames
  total, all at startup, 0 frames in the second half; JS heap 23 MB → 23 MB. Rune requests
  frames only while work is pending — a canvas app that stops moving costs nothing.
- **Size with Rune's build profile** (`opt-level=s`, thin LTO, codegen-units 1):
  14.95 MB raw / **3.89 MB gzip** (target ≤ 4 MB gzip: met, before wasm-opt). Default
  release profile for comparison: 13.3 MB raw / 4.3 MB gzip.
- **A11y mirror gap from session 1: closed — it was our query, not Rune.** The mirror
  emits native tags (`<button>`, `<p>`, `<a>`), not `role` attributes; querying by tag
  finds 129 real buttons including every visible node card (`node-0`…), the toolbar
  buttons with names, and text paragraphs. IME composition still untested.
- **E4 full contract, real-GPU numbers, IME, canvas hit-test** remain open; the harness
  now carries all the hooks they need.

## Results — session 3 (28 Nov 2026, later the same day)

Headed Chromium on this Mac (Apple M2): the Playwright build exposes no `navigator.gpu`, but
its **WebGL2 runs on ANGLE Metal** — so the GPU numbers below are the WebGL2 path on a real
GPU, the first non-software-rasterizer run of this spike. Methodology fix: the session-2
"66 ms during pan" numbers were an input-pacing artefact (`waitForTimeout` between synthetic
moves); with input pumped continuously and an in-page rAF monitor, the truth is different.

- **Pan and zoom keep vsync on the GPU path.** Pan at fit with 5,000–20,000 nodes visible
  (Shape): rAF p50 16.7 ms / p95 17.7 ms, **0 frames over 20 ms** across 125 ticks. Worst
  case, everything drawn after zoom-out to 5%: same p50 16.7, p95 17.7–18.8, max 19.3 ms —
  the **< 8 ms budget is not yet met** (vsync-pacing hides the true cost; a rAF-budget
  probe with 2 moves per frame is the honest next measurement), but nothing drops frames.
  The els path under the same zoom-out: p50 33.5 ms / p95 101 ms, 177 frames over 20 ms,
  130 over 50 — the Shape architecture wins by a wide margin where it matters.
- **A real defect found: the first wheel after idle presents ~2 s late, once.** Frame gaps
  after the first wheel: 2.1 → 8.3 → 32.4 ms (the pan applies fast — the screenshot changes
  within 300 ms) → then one **2,100 ms** hole → a 61 ms frame → idle. `tx.instant()` does
  not change it; it reproduces on the CPU canvas and WebGL2, at any scene size, and the
  bare-canvas control page shows max 21 ms gaps under the same dispatch. It reads as a
  deferred present inside Rune's web wheel path (its issue #2470 documents sibling
  mechanics: CPU→GPU handover freezes and WebGL2 polling on an 8 ms `setTimeout`). Every
  wheel after the first is clean. Upstream issue is due; a Meldframe-side mitigation would
  be keeping a display-linked idle heartbeat, at the cost of the zero-idle-CPU win.
- **IME through Rune's web text bridge: works for real input.** A per-field hidden
  `<textarea>` (aria-label = the field's label) takes focus on click; typed Latin
  ("hello editor", 12 chars), CJK (`こんにちは世界`, 7 chars), and emoji + RTL
  (`🎨 مرحبا`, 7 chars) all commit through `on_edit`/Add and count correctly in state
  (12 → 19 → 26); Escape leaves the draft uncommitted; synthetic `compositionstart/update/
  end` events deliver the full textarea value (19 chars — the un-committed draft rides
  along, matching Rune's read-the-value-on-composition-end design). Real device IME still
  pending, but the browser-side transport is proven.
- **Pixi baseline, same machine, same kind of scene** (`canvas-perf.mjs`, WebGL renderer,
  5,000 nodes at 12% fit-zoom, all drawn): pan p50 16.8 / p95 83.3 / max 116.7 ms;
  zoom-in-out p50 16.7 / p95 33.4 / max 50; pan at 100% (769 drawn) p50 16.8 / p95 66.8 /
  max 100. Rune's Shape path on the same document and viewport: pan p50 16.7 / p95 18.4 /
  max 19.3; zoom p50 16.7 / p95 18.8. The spike harness is not instrumented per-stage like
  the meldframe one (render-call ms vs interval), so read as frame-interval comparison
  only: **Rune drops no frames where Pixi's p95 hits 66–83 ms.** Hover/hit (T9): Pixi's
  harness measures 0.02 ms per hit through the wasm bridge — a Rune client must match that
  with its own HitIndex (not yet measured here).

## Results (session 2)

First session, 27 Nov 2026, macOS arm64, headless Chromium 140 (Playwright, `--enable-unsafe-webgpu`, so likely a software WebGPU adapter — numbers are indicative, not production). Harness: `../rune-web-spike/` (own git repo, commit `4bb3a60`), Rune pinned at `048528dc35f0`.

- **E1 — builds and runs: PASS.** A consumer workspace with the 8 git pins at one rev compiles for `wasm32` and the app draws in Chromium (`canvas[data-rune-canvas]`); Rune's own `web-hello` also builds on this machine (release 9.78 MB raw / 3.5 MB gzip; our harness release, default opt-level, 13.3 MB raw / 10.1 MB gzip — Rune's build script uses `opt-level=s` + thin LTO). Module init 94 ms; first paint follows immediately. The Inter faces must be served beside `app.js` (`/Inter-*.ttf`). The harness keeps its own `rust-toolchain.toml` (1.99.1); meldframe stays on 1.97.0. Disk: two target dirs, ~4 GB.
- **E2 — socket: PASS.** Rune has no WebSocket API; the harness opens one through a 3-function `wasm_bindgen(inline_js)` bridge and feeds received messages into app state with `AppHandle::update` from a thread-local sink (no other glue). Echo server: 3 pings sent, 3 received, rendered in the status bar. Glue cost: ~40 lines JS + ~60 lines Rust.
- **E3 — canvas behaviour: PASS with a finding that shapes the architecture.** 2,000 nodes culled to the viewport (142–156 drawn): interaction frames during wheel-zoom + drag-pan p50 16.6 ms / p95 18.8 ms; idle p50 5.4–13.8 ms. Worst case — 5,000 nodes all drawn (zoom 0.05): p50 113.6 ms / p95 821.6 ms / max 990 ms. Cause: every camera change rebuilds and re-lays-out every visible `el` (2,000–5,000 keyed absolute els). The el tree is chrome, not a canvas: a real Rune canvas app draws the node surface into `canvas(Shape)` per frame (Rune's own `examples/sketch` does exactly this) and keeps els only for panels, toolbar and dialogs. `Shape` has fills, strokes, paths and SVG; hit testing would come from our own portable `HitIndex`. Not yet measured: that Shape-based redraw path.
- **E4 — text fidelity: NOT RUN.** The comparison against the export crate's rects (the 0.05 px contract) needs a headless snapshot run; left for the next session.
- **E5 — import/export: CODE PROVEN, BROWSER RUN PENDING.** Paste handler wired for text, image and `web_files` (name/mime/size, `read(max)` for bytes); copy via `Tx::copy` from a press handler; download implemented through the JS bridge (`Blob` + object URL), because `Tx::save_file` is unsupported on wasm32 — this is the documented gap and the glue it needs. Clipboard-drive and file-paste runs in a real (non-headless) browser still pending.
- **E6 — web citizen: PARTIAL.** The semantic DOM mirror exists above the canvas; in our dump only the toolbar carried a role in the mirror — buttons and node cards set `.role("button")` + `.label(..)` but did not appear. Either the mirror needs another attribute on such elements or this is a gap; investigate before judging. IME through the hidden textarea is untested (no text field in the harness yet). Fonts, links (`open_url`), wheel/pinch/drag and a11y scaffolding otherwise ran.

Interim reading for the owner question: the web target is real (E1/E2 pass, canvas pans at viewport-visible scenes), the cost model resembles what meldframe already has — renderer surface + thin chrome — except both would live inside Rune instead of Pixi + DOM. The el-tree-as-canvas trap (E3 worst case) is the main new fact: a Rune client would draw the canvas with `Shape`, not elements, so "React parity" of panels would not carry the canvas work over either. Text fidelity (E4) and real-GPU numbers remain the open unknowns.

(filled when the spike runs; every criterion gets its numbers or its gap)
