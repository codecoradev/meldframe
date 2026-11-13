# Meldframe concept v1 (13 Nov 2026)

> Superseded for requirements by `prd.md` and for structure by `architecture.md`. Kept as the origin of the decisions in `decisions.md`.

## One sentence
An agent-first design canvas: you and AI agents design on one canvas, the result is faithful HTML/Tailwind, and the same design system can later drive carousels, slides and motion.

## Jobs to be done
1. Design web and mobile screens and export HTML/Tailwind.
2. Let an agent design through MCP while a human refines.
3. Stay consistent with a brand (tokens, optional Uteke brand memory).

## Principles
One `Command` for UI, WebSocket, MCP and undo. Rust core on the server and in WASM. Thin server. HTML/Tailwind output only. A portable layout subset. An agent surface that is idempotent, atomic and returns structured errors. Verification through capture.

## Objects
Workspace, File, Page, Frame (presets: Desktop, Tablet, Mobile; later Carousel, Story, Slide), Node (frame, text, image, shape, component instance), Variables, Components, Assets.

## Differentiators (hypotheses)
1. Tokens that are actually rendered on the canvas and exported as CSS variables (Wonder falls back to literals).
2. The most reliable MCP surface.
3. Portable brand memory through Uteke (optional).

## Spike order
W1 (web + MCP: model round 2 with tokens, margin, per-side padding, absolute children, gradients, opacity, no-fill; HTML/Tailwind + CSS variables export; frame presets; token import; layer-name lint; `design_review` tool; frame duplication as variants; V2 against Wonder). Then carousel, then motion decisions. Desktop and ACP after the repo is established.
