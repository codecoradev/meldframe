# Product

Status: draft for owner review (13 Nov 2026). Facts marked **decided** come from owner decisions recorded in `docs/decisions.md`; everything else is proposed.

## Platform

web (first). Desktop shell with a local agent over ACP is a later phase.

## Stack

Rust core shared by server (native) and client (WASM); Svelte 5 + PixiJS v8 web client; axum + sqlx + Postgres server; MCP via rmcp; auth through Gatehouse (OIDC/OAuth). Validated by spikes in a separate repository (`rust-core-spikes`).

## Users

1. The owner (dogfood): designs Uteke Cloud web screens, Uteke Mobile screens (phone frames) and marketing visuals, working with an AI agent (Claude Code) and refining by hand.
2. Developers worldwide who use Claude Code, Cursor or Codex and want an agent to design real UI against their own design system, then take the result as HTML/Tailwind. English-language product.

Not users now: teams that need live multi-human collaboration, non-technical designers who never touch an agent, enterprises that need SSO or SCIM.

## Product Purpose

Meldframe is a design canvas that an AI agent can drive reliably and that a human can take over at any moment. The agent works through MCP; every change appears on the canvas live; the human reviews, keeps or reverts it. The design system (variables) is rendered on the canvas and exported as CSS variables, and the exported HTML/Tailwind matches the canvas to a measured tolerance.

Success: the owner scores the result 4/5 or better on 3 of 4 real screens in a blind comparison against Wonder output (decided), and an agent that has never seen the product can produce a usable screen from the MCP tool descriptions alone.

## Positioning

The design surface agents can use without breaking things (idempotent, atomic, structured errors, undo per actor), with variables that survive all the way to code. Carousel, slides and motion extend the same design system later; they are not launch scope (decided).

## Operating Context

- Web editor for humans: canvas, layers, properties, variables.
- MCP server for agents: read, edit, capture, review.
- REST for snapshot and export.
- One workspace per tenant; one file is one live room; changes from the agent and the human flow through the same Command path and arrive on every open client over WebSocket.
- No live multi-human editing at the start (decided).

## Capabilities and Constraints

- Open source, public repository until the MVP; closed source afterwards, because CI needs infrastructure of its own. The repository has no licence yet: `NOTICE` applies, all rights are reserved and the code is readable, not licensed (decided).
- Output is HTML/Tailwind and CSS variables, PNG, PDF. No Flutter or Dart. PPTX last, only if ordered.
- Import is variables from a Tailwind config or CSS variables, and, in progress, a scoped HTML importer (decision 41, `docs/import-html.md`): HTML and CSS text that an agent has read, no URL fetched and no browser. Importing a web page by URL (`import_webpage`), Figma and PSD stay out (decided).
- Auth through Gatehouse; workspace is the tenant; quotas per workspace; assets on local disk first (decided).
- Pricing and hosting are out of scope until the product exists for personal use (decided).
- UI and agent-facing text are English (decided).

## Brand Commitments

Undecided: logo, visual identity, pricing. The name is Meldframe (decided); the trademark search is the owner's responsibility.

Proposed direction for the editor UI (values and rules in `DESIGN.md`, draft for owner review): the category standard played straight, at the craft level of the tools designers already know. The editor is an operating surface, so scanability, consistency and native expectations outrank expression. Must not feel like an AI chat with a canvas attached.

## Evidence on Hand

- Spikes S1 to S16, M1 to M3B, H1, V1, Q1, F1 to F3: layout core, WASM bridge, server, MCP, capture relay, ACP, session tokens, fractional order, undo under concurrency, assets and image fills. Numbers are in the spike repository docs.
- V1: real agent runs against the MCP surface, with the failures that led to Q1.
- Oracle experiment (see `docs/fidelity-contract.md` once merged): 12 of 15 fixtures within 0.034 px of a real browser.
- No users, no testimonials, no metrics, no logo. Never invent them.

## Product Principles

1. One `Command` for UI, WebSocket, MCP and undo.
2. The agent surface is a contract: idempotent, atomic where asked, structured errors, English text.
3. The core decides geometry; the export is a translation of it; a browser oracle keeps both honest.
4. Every agent change is visible and attributed, and can be reverted as a new change, never by rewriting history.
5. Variables are first-class: rendered on the canvas, exported as CSS variables.
6. Verify by looking: agents check their own work through capture.
7. Carry only what the spikes proved; design the rest before building it. No new spikes: unknowns are resolved by the first vertical slice in the repository.
8. Features are detachable modules behind one registry and one admission gate; the editor UI is a thin view of a UI-neutral app layer.
