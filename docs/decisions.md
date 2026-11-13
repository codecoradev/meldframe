# Decisions

Status of each item: **decided** (owner confirmed), **proposed** (needs owner confirmation), **open**.

## Product

| # | Decision | Status |
|---|---|---|
| 1 | Name: Meldframe. Trademark search is the owner's responsibility. | decided |
| 2 | Closed source, private repository. | decided |
| 3 | Core promise: a design surface that agents can drive reliably, with tokens that survive to code. Carousel, slides and motion are extensions, on hold until the core is solid. | decided |
| 4 | Second user: global developers using Claude Code / Cursor / Codex. UI language: English. | decided |
| 5 | Web is purely visual: manual canvas, MCP for external agents, REST (snapshot, export). No agent runtime or ACP on web. Desktop (Tauri 2, local agent over ACP) is a later phase after web is stable. | decided |
| 6 | Export first (HTML/Tailwind, CSS variables, PNG/PDF). HTML import is deferred. | decided |
| 7 | Initial design system is imported from a Tailwind config or CSS variables. | decided |
| 8 | No direct human-to-human collaboration at the start (no presence, comments, multi-editor). Agents via MCP; the UI is updated over WebSocket, including user changes. | decided |
| 9 | Quality gate V2: owner score >= 4/5 on 3 of 4 real screens, blind comparison against Wonder output. | decided |

## Platform

| # | Decision | Status |
|---|---|---|
| 10 | Authentication: Gatehouse (OIDC/OAuth), including OAuth for MCP. | decided |
| 11 | Multi-tenant: workspace is the tenant. Quotas per workspace. | decided |
| 12 | Asset storage: local disk for now; object storage later. | decided |
| 13 | Pricing and hosting: later. The product is for personal use first. | decided |

## Engineering

| # | Decision | Status |
|---|---|---|
| 14 | Rust core (server native + WASM client), Svelte 5 + PixiJS v8 client, axum + sqlx + Postgres server, MCP via rmcp. | decided (from spikes) |
| 15 | HTML/Tailwind fidelity is verified by an oracle test: render the export in Chromium and compare with the canvas. The supported CSS subset is defined in a fidelity contract. | proposed, doc pending |
| 16 | Human and agent collaboration UX (attribution, review, accept/revert) needs its own design, built on the existing per-actor undo, `ifSeq` and op dependencies. | open, doc pending |
| 17 | Token and variable system: reference Wonder's Variables and brand panels; define modes, aliases, scales and Tailwind import. | open, doc pending |
| 18 | Headless capture (render without an open UI) is likely required for agents running without a browser. Decide after the cost comparison (relay vs native renderer vs headless Chromium). | open |
| 19 | Document model: schema versioning and migrations, components and instances with overrides, variants as duplicated frames or branches. | open |

## Workflow

Branches `feat/`, `fix/`, `docs/`, `chore/` from `develop`; rebase onto `develop`; squash merge with a clean message; commits signed off (DCO); Cora Review CI; all git artifacts in English.
