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
| 18 | Headless capture (render without an open UI) may be needed for agents running without a browser. Start with the browser relay; decide on a second renderer in the repository when a real agent workflow needs it. No separate spike. | open |
| 19 | Document model: schema versioning and migrations, components and instances with overrides, variants as duplicated frames or branches. | open |

## Structure (owner decisions, 13 Nov 2026)

| # | Decision | Status |
|---|---|---|
| 20 | Layered crates with kinds and enforced dependency rules; feature modules that never import each other (except composite), one registry read by editor, MCP and REST, a build per feature alone. See ADR-0007 and `docs/standards/modular-thin-ui.md`. | decided |
| 21 | One admission gate in front of every change, with a permit, risk tiers and agent pairing. See ADR-0010. The pairing token is replaced by an OAuth grant (decision 31, ADR-0012). | decided |
| 22 | UI-neutral app layer in Rust compiled to WASM; the web client only renders. See ADR-0008. | decided |
| 23 | Live events are durable (sequence numbers, replay) or ephemeral (best-effort). See ADR-0009. | decided |
| 24 | No new spikes. The first vertical slice of the app layer (the editor shell in M1, see decision 25) is built directly in the product repository with exit criteria written first. | decided |
| 25 | First goal: backend, basic web editor and MCP connected end to end (milestone M1 in `docs/epics.md`). Development tokens behind the gate's principal interface in M1; Gatehouse in M2. From M2 agents connect through OAuth grants (decision 31). | decided |
| 26 | The recommendations in `docs/owner-questionnaire.md` are working defaults until the owner overrides an answer. | decided |

## UI (owner decisions, 16 Nov 2026)

Details in `docs/ui-design.md`.

| # | Decision | Status |
|---|---|---|
| 27 | Editor layout: the canvas fills the window; chrome floats over it; the layers panel and the right panel collapse (Option+1, Option+2); a view control saves and restores panel layouts. | decided |
| 28 | Colour variables are shown as OKLCH by default, hex on request and by copy. Storage and canvas rendering stay open (`tokens-and-variables.md` Q3). | decided |
| 29 | The web editor stays visual: canvas and MCP. Comments on the web are parked. Decision 8 stands. | decided |
| 30 | Desktop front end, layout A (mode switch): a sidebar of repositories (local folder or GitHub) with chats beneath, a chat pane for chats only, and a work area that switches between Design (tabs of designs) and Code (file viewers and terminals as tabs, the file tree on the right). Chats speak ACP to a local agent; design changes go through MCP and the gate. Side by side and dockable layouts are later options. Open: the sidebar word, repository versus workspace file, one working copy per chat. | decided (layout), proposed (details) |
| 31 | Agents connect through OAuth with Gatehouse only, on the web through MCP and on the desktop through the app. No API key, token or pairing token is created or copied by hand. A connection is an OAuth grant: the consent screen lists the risk tiers, access tokens are short-lived, and revoking the grant ends the agent (owner, 16 Nov 2026). The gate, permit and risk tiers of ADR-0010 stand; ADR-0012 replaces its pairing token and rule G4, and E2.4, FR-28 and `collab-ux.md` section 14 follow. | decided |

## Workflow

Branches `feat/`, `fix/`, `docs/`, `chore/` from `develop`; rebase onto `develop`; squash merge with a clean message; commits signed off (DCO); a local Cora agent review before every pull request (Cora Review in CI is disabled); all git artifacts in English.
