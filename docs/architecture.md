---
title: "Architecture: Meldframe"
status: draft for owner review
created: 2026-11-13
inputs: ["docs/prd.md", "GLOSSARY.md", "docs/adr/", "docs/standards/modular-thin-ui.md", "docs/architecture/spike-survey.md", "docs/collab-ux.md", "docs/tokens-and-variables.md", "docs/fidelity-contract.md"]
vocabulary: "codebase-design (module, interface, depth, seam, adapter, leverage, locality)"
---

# Architecture: Meldframe

This document says how the system is shaped and why. Requirements are in `docs/prd.md`; each hard-to-reverse decision has an ADR in `docs/adr/`. Nothing here is product code. Facts about the spikes come from the survey in `docs/architecture/spike-survey.md` and were not re-measured.

## 1. Context and drivers

Meldframe lets a human and an AI agent edit the same design file, and exports it as HTML/Tailwind. The forces that shape the design:

1. **Agents are first-class clients.** They retry, run in parallel and read tool text as instructions. Every write must be idempotent and attributable.
2. **One truth for geometry.** The canvas, the export and the agent's capture must agree.
3. **Variables must reach code.** They are rendered on the canvas and exported, not flattened to literals.
4. **Human control.** A person can take over at any time and can undo agent work without rewriting history.
5. **A solo owner builds and runs it.** Few moving parts; Postgres, one server binary, one web client.
6. **Closed source, private repository, personal use first.** Hosting, pricing and scale are not drivers yet.

## 2. Decision summary

| # | Decision | Status |
|---|---|---|
| ADR-0001 | One `Command` interface for every change (UI, WebSocket, MCP, undo) | accepted |
| ADR-0002 | Rust core shared by native server and WASM client; thin server | accepted |
| ADR-0003 | Core geometry is the source of truth; export is a translation; a browser oracle gates CI | proposed |
| ADR-0004 | Flat variable model (collections, modes, aliases), resolved in the core | proposed |
| ADR-0005 | Agent changes apply live and are reviewed afterwards; the batch is the unit; revert is a new change | proposed |
| ADR-0006 | Workspace is the tenant; authentication through Gatehouse | accepted |
| ADR-0007 | Layered crates with enforced dependency rules; detachable feature modules | accepted |
| ADR-0008 | UI-neutral app layer in Rust (WASM); front ends only render | accepted |
| ADR-0009 | Live events are durable (replayable) or ephemeral (best-effort) | accepted |
| ADR-0010 | One admission gate in front of every change | accepted |

## 3. System shape

```mermaid
flowchart LR
  subgraph Clients
    W[Web editor\nSvelte 5 + PixiJS + WASM core]
    A[Agent\nClaude Code / Cursor / Codex]
  end
  W -- WebSocket ops/events --> S
  A -- MCP tools --> S
  W -- REST snapshot, export --> S
  subgraph S[Server (axum)]
    ADM[Admission\nauth, mode, quota, timeout]
    ROOM[Room actor\none total order per file]
    CAP[Capture hub]
    EXP[Export]
  end
  ADM --> ROOM
  ROOM --> PG[(Postgres\nops, snapshots, assets index)]
  CAP -. request/response .-> W
  G[Gatehouse\nOIDC/OAuth] --> ADM
```

Each file has one room. Every client and every agent talks to that room through the same admission step, so a change made by a person and a change made by an agent take the same path and appear on every open client in the same order.

## 3b. Layers and dependency rules

The structure follows the CodeCora standard `modular-thin-ui` (`docs/standards/modular-thin-ui.md`; ADR-0007 to ADR-0010). Every crate declares a kind and CI enforces the matrix.

```
apps/
  server/                 bin: axum, MCP, REST, WebSocket; composition root
  web/                    bin: Svelte 5 + PixiJS front end (renders meldframe-app state)
crates/
  core/
    meldframe-protocol    wire and model types (TypeScript generated from Rust)
    meldframe-core        document, Command, history, concurrency rules, layout, resolve (render model)
    meldframe-module      Module trait, registry, risk tiers
    meldframe-host        rooms, jobs, store, admission gate, activity log
  engines/                text measurer, rasteriser (traits and adapters)
  integrations/           MCP (rmcp), Gatehouse OIDC/OAuth, Uteke memory
  features/<name>/        <name> (domain) and <name>-ui (view): variables, export, review, capture, assets, import-tokens
  app/
    meldframe-app         UI-neutral state, actions, effects, events (compiled to WASM)
    meldframe-wasm        thin wasm-bindgen glue over core and app
  ui/                     tokens and plain-data *-model crates beside views
xtask/                    deps-check, build-matrix, portable-check, size-check, ui-literals, licences
```

Allowed dependencies: core to core; engine and integration to core and engine; feature to core, engine, integration and (only if marked composite) other features; app to core, engine, integration and feature; ui to core, app and ui. UI toolkits are allowed only in ui, feature-ui and bin crates. Core, engine, integration, feature and app crates must compile for `wasm32`. A product build with each feature alone, and with none, must pass its tests.

Not detachable (inner): the document model, the `Command` interface and the render model. Split concepts: variable bindings are in core; import, export and the panel are the `variables` feature. Server internals (admission, room, sessions) stay modules inside the server crates until a second binary needs them.

This layout replaces the one suggested in the survey (`docs/architecture/spike-survey.md`, section 6.4), which kept crates few and could not prove that a feature can be left out.

## 4. Modules and seams

Using deep-module vocabulary. A module is **deep** when much behaviour sits behind a small interface.

| Module | Interface (what callers know) | Depth | Adapters at its seam |
|---|---|---|---|
| Document core | submit a command; read resolved state; undo/redo for an actor; apply a remote op | Deep (proven in spikes) | Server (native) and client (WASM) consume it; not a port |
| Render model | resolve nodes and variables into painted boxes | Deep once built; **missing in the spike** | Two consumers: canvas and export (a real seam because two consumers exist) |
| Text measurer | measure text for a font, size, weight and wrap width | Small | Three adapters in the spike: stub, recorded table, browser |
| Admission gate | submit an op or batch with a principal; get a report or a typed refusal | Deep once consolidated; duplicated in the spike across WebSocket, MCP and REST | One gate with three transports on top; the room accepts a write only with a permit the gate creates (ADR-0010) |
| Module registry | register commands, settings, jobs, agent tools and panels; read them | Small interface, wide leverage | Read by editor, MCP and REST; leaving a feature out removes it everywhere (ADR-0007) |
| App layer | dispatch an action; read state; carry out effects; receive events | Deep once built; **does not exist in the spike** (rules live in the Svelte client) | Front ends: web now; desktop or mobile later; a test harness as the second driver (ADR-0008) |
| Room actor | accept ops in order, apply, persist, broadcast | Deep (proven) | Persistence has one adapter (Postgres); do not add a port until a second consumer exists |
| Agent surface | MCP tools contributed by feature modules, over the gate | Thin by design | Adapter over the gate |
| Capture | request an image of a node or frame | Small | One adapter (browser relay); a second renderer is decision 18, so no port yet |
| Export | resolved model in, HTML/Tailwind/CSS variables and a warnings list out | Deep | Consumer of the render model |
| Blob store | put and get image bytes by content hash | Small | Local disk first; object storage later gives the second adapter |

Principles applied: the interface is the test surface; one adapter means a hypothetical seam, two means a real one; delete a pass-through module and see where its complexity lands.

## 5. Data architecture

**Hierarchy.** Workspace has Files; a File has Pages; a Page has Frames; a Frame is a tree of Nodes. Variables live in Collections at workspace level; files bind to them. Components and instances are planned (decision 19).

**Node.** Stable id; type; parent and fractional order key (position among siblings; ties broken by id); geometry and layout fields; paint fields; text fields; optional image reference by content hash; bindings from property to variable.

**History.** An append-only log of ops per file with the actor, idempotency key, sequence number and the command; periodic snapshots for fast load. Every document and snapshot carries a **schema version** from the first release (the spike has none; adding one is part of the seed work).

**Variables.** Collection, Mode, Variable with stable opaque ids; values per mode are literals or aliases; names are slash paths for CSS and Tailwind mapping. Details and open questions: `docs/tokens-and-variables.md`.

**Assets.** Content-addressed by hash; png, jpeg, webp and gif only; metadata in Postgres; bytes on local disk first.

**Tenancy.** Every file, variable collection, asset and quota counter belongs to one workspace; no row is readable without the workspace in the principal.

## 6. Interfaces

- **WebSocket:** subscribe to a file; submit ops with idempotency keys and optional dependency keys; receive durable frames (accepted ops, with sequence numbers, replayable from a cursor) and ephemeral frames (agent touch hints, capture requests, progress; best-effort, never replayed). A full per-stream queue drops the frame and sends a resync marker (ADR-0009). The wire types are generated from Rust to TypeScript and a CI check fails on drift.
- **MCP:** tools to read, apply, capture, list limits, check batch status, manage assets and run design review. Tool text is English and part of the contract. Errors have stable codes.
- **REST:** snapshot, export, asset upload and session management.
- **Gatehouse:** OIDC for people; OAuth for MCP clients; the principal carries user and workspace.

## 7. Change semantics (carried from the spikes)

Ops are idempotent per actor. A batch may be atomic. `ifSeq` rejects an edit of a node changed after the caller last looked. An op may name dependencies; it is accepted only if every dependency was applied, so an undo never lands on a premise the server rejected. Undo is per actor and recomputed against the current document. A revert of agent work is a new batch that restores only fields whose last writer is still that agent. These behaviours were verified by property tests; see the spike docs for evidence and the list of what was not proven.

## 8. Fidelity and verification

The core's layout result is the source of truth. The export translates it to CSS. An oracle test renders the export in Chromium and compares boxes, text ink and paint probes with the core. The contract, tolerances and measured results are in `docs/fidelity-contract.md`. Documents that fall outside the contract (fill under hug, overflow, zero-width fill) are normalised or warned on write instead of being matched in CSS.

Capture lets an agent check its own work. The first adapter relays the request to an open editor; headless rendering is decided later with measured cost.

## 9. Security and privacy

- Authentication through Gatehouse; agent sessions are short-lived, bound to one file selected in the UI, and revocable at once.
- Rate limiting happens before parsing; quotas per workspace; per-call timeouts.
- Uploads: image types only; SVG rejected until sanitisation exists.
- No secrets in process arguments, logs or tool output; no code from user input runs on the server (imports accept text; config files written in JavaScript are resolved by the agent locally).
- Agent-written text (batch notes, layer names) is untrusted: rendered as plain text, length-limited.
- Audit: every op has an actor and is kept in the log.

## 10. Non-functional approach

Performance is a property of the core running in WASM on the client; the server stays thin. Reliability comes from idempotent ops and a log that can rebuild state. Observability: structured logs with workspace, file, actor and sequence number; counters for rejects by code. Backups are database backups. Scaling beyond one server process is not a driver now; the room actor assumes one writer per file.

## 11. Seed from the spikes

Carry as is: the document core with its tests, the protocol types and generation pipeline, the WASM build scripts, the room actor, ingress guard, quota logic and the capture relay. Carry with refactor: admission consolidation, export (becomes the first consumer of the render model), WASM client merge, blob port, Gatehouse principal. Rewrite: canvas component (as a view of `meldframe-app` state), model round 2 (variables, margin, per-side padding, absolute children, gradients, opacity, no-fill, components, schema version), workspace and tenant model. Not carried: PPTX, Flutter, ACP and the agent panel (desktop phase). Full table: `docs/architecture/spike-survey.md`.

## 12. Risk register

| ID | Risk | Likelihood | Impact | Mitigation | Trigger |
|---|---|---|---|---|---|
| R1 | Export drifts from canvas as model grows | High (already observed in the spike) | High | Render model; coverage test that every node field is rendered by both or marked unsupported; oracle in CI | Any new node field without a fixture |
| R2 | Variable model is too small or too large | Medium | High | Prototype the binding and resolution before committing; import a real Tailwind config as the corpus | Import report shows large unmapped share |
| R3 | Agents are rejected too often by conservative dependency rules | Medium | Medium | Measure rejection rate on real agent runs, not only the hostile harness | Rejection rate on V2 runs above a threshold set by the owner |
| R4 | Capture requires an open editor | Medium | Medium | Decide headless capture with measured cost | Agent workflows without a browser |
| R5 | Human takes over and the agent keeps editing the same nodes | Medium | Medium | `ifSeq`, visible touch hints, stop by revoking the session | Frequent conflict codes in logs |
| R6 | Layout rules do not match CSS in degenerate documents | Medium | Medium | Validator normalises or warns on write | Oracle failures in overflow or fill-under-hug cases |
| R7 | One solo owner is the bottleneck | High | Medium | Small scope (W1), docs before code, agents do bounded work and the owner reviews | Review queue grows |
| R8 | Name or trademark conflict | Unknown | High | Trademark search by the owner before any public use | Before any domain, account or package |
| R9 | Gatehouse OAuth for MCP clients is not yet proven end to end | Medium | Medium | Build the MCP OAuth flow as the first platform slice in the repository (no separate spike) | First failed MCP client login |
| R10 | The WASM boundary makes UI state exchange too costly | Medium | Medium | Exchange coarse snapshots plus events; build the agent activity panel first with written exit criteria; fall back to a TypeScript app layer if needed (ADR-0008) | First slice misses its frame-time criteria |
| R11 | The layering checker slows early work or is skipped | Medium | Medium | Add the checker, build matrix and size ceilings in E0 before feature code; keep rules as a tested pure function | CI exceptions grow |

## 13. Epic structure (feeds the roadmap)

- **E0 Foundation:** workspace layout by kind, CI (`deps-check`, `build-matrix`, `portable-check` for wasm32, `size-check`, `ui-literals`, `licences`), English docs, schema version, protocol pipeline.
- **E1 Core and render model:** seed the core; build the render model; model round 2 properties.
- **E2 Variables:** model, commands, resolution, canvas rendering, import, export.
- **E3 Agent surface:** admission gate (ADR-0010), module registry, MCP tools with risk tiers, actor profiles, batch notes, change queries, durable and ephemeral frames (ADR-0009).
- **E4 Editor:** `meldframe-app` layer (ADR-0008) with the agent activity panel as the first vertical slice, then layers, properties, variables panel, review and revert.
- **E5 Export and oracle:** HTML/Tailwind/CSS variables, oracle gate in CI.
- **E6 Platform:** Gatehouse login, workspaces, quotas, local asset storage.
- **E7 Evaluation:** V2 against Wonder on four real screens.

## 14. Approval record

| Item | Status |
|---|---|
| This document | draft, awaiting owner review |
| ADR-0001, 0002, 0006 | accepted (decisions already made) |
| ADR-0007, 0008, 0009, 0010 | accepted by the owner on 13 Nov 2026 |
| ADR-0003, 0004, 0005 | proposed, awaiting owner decision (see `docs/owner-questionnaire.md`) |
