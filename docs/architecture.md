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
  server/                 bin: axum, MCP, REST, WebSocket; composition root; `PgStore`, the Postgres adapter of the store seam (sqlx, migrations from `/migrations`)
  web/                    bin: Svelte 5 + PixiJS front end (renders meldframe-app state)
crates/
  core/
    meldframe-protocol    wire and model types (TypeScript generated from Rust)
    meldframe-order       fractional index and sibling order helpers
    meldframe-check       limits, ApplyError and every check on a value or a command (structure, decoration, image)
    meldframe-layout      text measurer seam, Rect and layout
    meldframe-core        applying a command to the file tree (`apply_command`, `plan_command`, `apply_planned`); re-exports the check API; the render model comes later
    meldframe-undo        undo and redo bookkeeping with no tree in it: the steps of a history, their runs, and the rules that rewrite them against a remote command; split from history
    meldframe-history     `FileState` (the in-memory tree of one File) with validated apply, undo and redo per actor, groups and remote ops
    meldframe-history-props (tool) property tests of random undo and redo runs and of undo against a deleted parent; no library code
    meldframe-sync        `Replica`: the optimistic layer over a `FileState` (pending ops, the server's order, rebase, op dependencies)
    meldframe-store-contract (tool) the `Store` seam cases as functions and a `store_contract!` macro; both adapters run them
    meldframe-sync-props  (tool) property tests of replicas against an in-memory server and the host room; no library code
    meldframe-module      Module trait, registry, risk tiers
    meldframe-host        room, store, admission gate; jobs later
    meldframe-rooms       activity log of accepted ops and the room registry (open, close, a bound, idle close by a caller-supplied clock); split from host, which is near its 2,000-line ceiling
    meldframe-approvals   approval path for a call with policy Ask (pending, approved, denied, expired; log of decisions); depends on host and rooms
  engines/                text measurer, rasteriser (traits and adapters)
  integrations/           MCP (rmcp), Gatehouse OIDC/OAuth, Uteke memory
  features/<name>/        <name> (domain) and <name>-ui (view): variables, export, review, capture, assets, import-tokens
                          today only the placeholder `example/` (`example`, `example-ui`) exists
  app/
    meldframe-app         UI-neutral state, actions, effects, events (compiled to WASM)
    meldframe-wasm        thin wasm-bindgen glue over core and app
  ui/                     tokens and plain-data *-model crates beside views
xtask/                    deps-check, build-matrix, portable-check, size-check, ui-literals, licences
```

Allowed dependencies: core to core; engine and integration to core and engine; feature to core, engine, integration and (only if marked composite) other features; app to core, engine, integration and feature; ui to core, app and ui. UI toolkits are allowed only in ui, feature-ui and bin crates. Core, engine, integration, feature and app crates must compile for `wasm32`. A product build with each feature alone, and with none, must pass its tests.

`PgStore` lives in the server crate because sqlx is not portable (L5) and no kind of its own exists for adapters yet. Its methods block on a private runtime, so a transport calls it from a plain thread or `spawn_blocking` (dropping the last clone from an async task panics); each `connect` costs one runtime and one pool, so connect once and clone. The sqlx migrator owns the database: `connect` applies `migrations/` and refuses, with a clear error, a database or schema that holds the tables but no `_sqlx_migrations` (what `psql -f migrations/*.sql` and the dev seed leave behind), rather than failing half way. We chose the refusal over a baseline of such a database: which migrations psql applied is not recorded. So until `just dev-db` loads migrations through the same migrator, point `PgStore` at an empty database or schema (`connect_in_schema`), not at the psql-loaded `meldframe_dev`; `tools/check-migrations.sh` is unaffected. A data error that can never succeed (a NUL character that `jsonb` cannot hold) is reported as `StoreError::Unavailable` with the text "retrying cannot succeed", because the seam has no permanent-failure variant. One `files` row per room carries the counter that an append locks, which makes sequence numbers gap-free per file across connections; the op key is unique per `(file, actor, key)` (migration 0002). `meldframe-store-contract` is a `tool` crate for the same reason as the next one: the cases would not fit `meldframe-host` under its 2,000-line ceiling.

`meldframe-sync-props` is a separate crate of kind `tool` for one reason: the random runs of several replicas against an in-memory server (convergence, undo per actor, refused ops, op dependencies) would have taken `meldframe-sync` over the 2,000-line ceiling with its scenario tests. It depends on the replica crate (and on the host crate for the room test), not the other way round, and has no library code; a default run is 600 seeds (`MELDFRAME_SEEDS` raises it; nightly CI uses 6,000) and `tools/mutation-check-sync.sh` shows that switching a rule off makes a test fail.

The document model was one crate in the first plan. It is now several small core crates (ADR-0011) so that each stays under the 2,000-line ceiling with its tests. The in-memory tree of one File is named `FileState` (see GLOSSARY), because the glossary keeps "document" out of the vocabulary for a File.

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
| Room actor | accept ops in order, apply, persist, broadcast | Deep (proven) | The `Store` seam has two adapters, in-memory (`meldframe-host`) and Postgres (`PgStore`, `apps/server`), both held to one contract suite (`meldframe-store-contract`) |
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

**Snapshot load (decision, issue 125).** A snapshot that reaches a replica is bytes from a network, so the client checks it itself rather than trusting that the server did. `Replica::new` returns `Result<Replica, LoadError>`: it runs `FileState::try_from_nodes` (the old-file migration, then `check_file`) and refuses a bad snapshot (duplicate id, missing parent, text node as parent, cycle, too deep, too many nodes, an invalid node) with the typed error and its stable code (`docs/error-codes.md`); nothing is repaired and no replica exists. A snapshot without `pos`, or with equal `pos` among siblings, is migrated exactly as a stored file is, so `E_MISSING_POS` and `E_DUPLICATE_POS` do not reach a replica through `new`. The server may also validate before it sends; that is a second line of defence, not a replacement. `FileState::from_nodes_unchecked` has no caller in library code; it stays for tests.

Ops are idempotent per actor. The gate decides a batch as a whole; the room does not apply it atomically (7a). `ifSeq` rejects an edit of a node changed after the caller last looked. An op may name dependencies; it is accepted only if every dependency was applied, so an undo never lands on a premise the server rejected. Undo is per actor and recomputed against the current document: a field another actor wrote after me is not overwritten, and a node my undo would put under a parent the other actor deleted goes under the nearest ancestor that survives (the top level if none does); a step whose node was deleted is dropped. An undo or redo that is already on its way when another actor's op is ordered before my edit (a stale op) cannot be recalled, so the replica works out what the rebuilt steps would write and sends a correction: an `UpdateNode` for every node field, `image` included, where the stale op would leave an older value (a stale `Reparent`, `Reorder`, `AddNode` or `RemoveNode` is not corrected). The redo step keeps the fields that the other actor wrote before my edit, and the undo of an edit made later goes back to the other actor's value. A correction is worked out in the replica's model of the rebuilt steps: a later remote op that wrote a field removes it from the correction there, but a correction already sent or queued is not changed, so the server may briefly hold the older value until a follow-up correction restores the remote value. A correction the server refuses is not sent again. Known limits: (1) when the last correction is refused and the stale undo is already confirmed, the queue is empty, nothing is tracked any more and the stale value stays in the confirmed state; (2) when an op is refused or dropped, the replica forgets the stale values that earlier confirmed ops left in the confirmed state, so a later undo may start from them; (3) when two actors have an undo on its way for the same field, a stale op and its correction of one actor are writes that come after the edit of the other, so the field may keep a value of an undone edit. A revert of agent work is a new batch that restores only fields whose last writer is still that agent. These behaviours were verified by property tests; see the spike docs for evidence and the list of what was not proven.

**Approval for Ask (E2.4).** `Gate::submit` and `Gate::submit_with` are unchanged: an Ask call is `forbidden`. A caller that wants an approval path submits one op through `Approvals::submit(gate, who, file, op, facts, at)` (`meldframe-approvals`, not the gate: `meldframe-host` and `meldframe-rooms` are at their size ceilings and the gate holds no state). It runs the gate first; only a refusal whose policy is Ask opens a pending approval (id, actor, file, key, command summary, state, `created_at`, `expires_at = created_at + ttl`) and returns `approval_required` with the id. The owner calls `decide(id, Approve | Deny, by, at)`; the same op sent again (same workspace, actor, file and whole op: key, command, `if_seq` and dependencies, compared by the op's debug text) is then admitted by the same gate with its Ask read as Auto for that one op (the gate comes from `Approvals::gate(policy)`, an `ApprovalGate` that derefs to `Gate`, wraps the policy and has a flag of its own that only `submit` sets, behind a drop guard, so no other gate and no caught panic leaves Ask lifted; chain `.quota(n)` as usual), so the facts, the tier of the permit and the workspace quota are the real ones and the op is counted in `Gate::used`. The approval is consumed at that admission, not when the room applies the op, so an op the room then refuses has spent it; a quota refusal does not spend it. An op that the gate admits without the approval (the policy or the tier changed) consumes a matching approved one too, so it cannot admit the op again later. A second use asks again under a new id, as a consumed approval does not block a new one for the same op. A denied approval stays `approval_denied`, an expired one (pending or approved) `approval_expired`. A Deny policy, quota or shape refusal is never lifted: the approved op is admitted only after the real gate said Ask. Time is the caller's `at`; nothing reads a wall clock. Decisions go to the approval log (`Approvals::decisions`: approval id, actor, file, key, kind, node, state, `by`, `at`), not to the activity log, whose entries carry the sequence number of an accepted op. `by` is a label: the seam does no authorisation, who may decide is the owner's call (E2.4, Gatehouse). The match does not include the facts the tier depends on (the subtree size); `get` and `find` can show a Pending or Approved approval after its ttl until the next `submit`, `decide` or `request` marks it Expired. Approvals are in memory and not pruned (a follow-up: drop finished ones older than the ttl).

### 7a. The dependency rule in the room

An op may name **dependencies**: idempotency keys of ops. The gate and the room enforce one rule; the replica builds its dependencies from its own pending keys and never relies on it for safety. Each clause is pinned by a test in `crates/core/meldframe-rooms/tests/dependencies.rs` (and, against random runs of replicas, `crates/core/meldframe-sync-props/tests/room.rs`).

1. **Same actor only.** A dependency counts only if the room's log holds that key for the same actor of the same file (`Store::find`). A key of another actor, an unknown key and a key that was refused all answer `dependency_rejected`, naming the first missing key. Two actors may use the same key text; they are different keys.
2. **Every dependency, not one.** The op is written only if all of its dependencies were applied.
3. **At most 512 dependencies** (`MAX_OP_DEPS`), no empty or repeated key, and no op that names its own key. The gate refuses these as `invalid` before the room sees the op (exactly 512 passes, 513 does not).
4. **Forward and cyclic dependencies are refused, not deferred.** The room decides when the op arrives: a dependency that is not applied yet makes the op `dependency_rejected`, and the refusal reserves nothing (the op's own key stays free). The client sends the dependency first and the dependent again. A cycle can therefore never be applied. The room has no `dependency_pending` outcome; that word in `wire.rs` is reserved for a transport that holds a dependent op on a connection, and the room never produces it.
5. **A refusal cascades.** An op that was refused is not applied, so every op that names it, directly or through a chain, is refused with `dependency_rejected` until the first one is corrected and applied.
6. **Order within a batch.** The gate decides a batch as a whole, then the room takes its ops one by one in the batch's order. An op may depend on an earlier op of the same batch (it is applied by then); a dependency on a later op of the batch is a forward dependency. A refused op does not stop the ops after it; its dependents are refused by clause 5. A batch is not atomic in the room today.
7. **Applied keys are remembered for ever.** The rule reads the log, not a cache or a time window, so a resend after any disconnect still finds keys confirmed long ago, across a restart, a snapshot and a log longer than one replay page.

**Small delete (ADR-0010).** The tier of a `RemoveNode` depends on the room's tree, so the gate takes facts about it: `Gate::submit_with(who, ops, &dyn NodeFacts)`, and `Gate::submit` is the same with no facts, where every delete is a Delete. `meldframe-rooms` implements `NodeFacts` as `TreeFacts::new(room.nodes())`. A delete is a Change (Auto under the default policy) when the node is in the file, is not a frame, and its subtree (the node and its descendants) has at most `SMALL_DELETE_MAX_NODES` nodes (10, defined in `meldframe-rooms`); every other delete is a Delete, Ask by default and refused as `forbidden`. A node the facts do not know is a Delete (fail closed). Callers pass the facts of the room the ops target. The facts are the tree before the batch: a node added in the same batch is unknown, and a batch that adds or reparents any node gets no relief for its deletes, because it could grow the subtree it then removes. The facts are read at submit time. The room re-checks one thing when it applies a delete whose permit tier is below Delete: the node must still be a non-frame node without children, otherwise it refuses with `forbidden` (Delete, Ask); so a leaf removed and a frame added under the same id cannot be deleted as a small delete. It does not re-count the subtree. Today only a frame can have children (the core refuses a child under a non-frame node), so the 10-node limit is a guard that no tree built by the room reaches: in practice every non-frame delete is small and every frame delete is Ask. The permit carries the tier the gate decided.

8. **A retry is checked before dependencies.** A key that is already applied returns its first sequence number and writes nothing when the command is the same (`invalid` when it is another command), whatever dependencies the retry names: the stored op keeps no dependency list, and the key is the identity.

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
| R10 | The WASM boundary makes UI state exchange too costly | Medium | Medium | Exchange coarse snapshots plus events; build the editor shell (M1) first with written exit criteria; fall back to a TypeScript app layer if needed (ADR-0008) | First slice misses its frame-time criteria |
| R11 | The layering checker slows early work or is skipped | Medium | Medium | Add the checker, build matrix and size ceilings in E0 before feature code; keep rules as a tested pure function | CI exceptions grow |

## 13. Epic structure (feeds the roadmap)

- **E0 Foundation:** workspace layout by kind, CI (`deps-check`, `build-matrix`, `portable-check` for wasm32, `size-check`, `ui-literals`, `licences`), English docs, schema version, protocol pipeline.
- **E1 Core and render model:** seed the core; build the render model; model round 2 properties.
- **E2 Variables:** model, commands, resolution, canvas rendering, import, export.
- **E3 Agent surface:** admission gate (ADR-0010), module registry, MCP tools with risk tiers, actor profiles, batch notes, change queries, durable and ephemeral frames (ADR-0009).
- **E4 Editor:** `meldframe-app` layer (ADR-0008) with the editor shell (layers, canvas, properties) as the first vertical slice in M1, then the variables panel (M3), the activity panel, review and revert (M5). Milestones and acceptance criteria: `docs/epics.md`.
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
