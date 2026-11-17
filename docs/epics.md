# Epics and milestones

Plan of record for building W1. Status: draft for owner review, 13 Nov 2026. Requirements are in `docs/prd.md` (FR and NFR ids), structure in `docs/architecture.md`, decisions in `docs/decisions.md` and `docs/adr/`. Code is authoritative over this document once building starts; status syncs are dated.

## How the plan works

- **Milestones end on exit criteria, not dates.** A milestone is done when every exit criterion is demonstrated, not when its epics are merged.
- **Epics are vertical where possible.** Each epic lists acceptance criteria that a test or a demonstration can check. Epics become tracker issues when the owner approves this plan; the issue links back here instead of copying the criteria.
- **No separate spikes** (decision 24). An unknown is resolved by the first slice of the epic that meets it, with its exit criteria written before the work starts.
- **Every feature module ships its agent tools** with risk tiers (ADR-0007, ADR-0010). A feature is not done until an agent can use it through MCP.
- **Sizes** are rough: S (a few days of agent work, one or two pull requests), M (about a week, several pull requests), L (more than a week, split before starting).

## Definition of done (every epic)

1. Behaviour is covered by tests at the agreed seam: the core's submit-a-command interface, the MCP surface with a real client and Postgres, the app layer driven by a test harness, or a browser end-to-end test. A failing test comes first for behaviour changes.
2. CI is green: format, lint, tests, layering checks, size ceilings, UI literal lint, licence check, a local Cora agent review of the whole diff, recorded in the pull request, with no major or critical finding left.
3. Docs touched by the change are updated in the same pull request (architecture, ADR, glossary, MCP tool catalogue).
4. Agent-facing text and errors are English with stable codes.
5. Commits are signed off and carry no AI attribution; pull requests stay small (about 400 changed lines and 10 files, one kind of content) and the heavier gates are run locally before the pull request.

## Milestones at a glance

| Milestone | Outcome | Epics | Exit criteria (summary) |
|---|---|---|---|
| M0 Foundation | A repository an agent can build in safely | E0.1 to E0.4 | CI enforces the standard on an empty workspace |
| **M1 Connected skeleton** (owner's first goal) | Backend, basic web editor and MCP connected end to end | E1.1 to E1.7 | An external agent and a person edit one file live, through one gate |
| M2 Platform and agent identity | Real login, workspaces, quotas, agent grants, assets | E2.1 to E2.5 | Gatehouse login and MCP OAuth work; data is tenant-scoped |
| M3 Model round 2 and variables | The layout and paint model of W1, with variables rendered | E3.1 to E3.5 | A Tailwind design system is imported and rendered on the canvas |
| M4 Export and fidelity | HTML/Tailwind with CSS variables that match the canvas | E4.1 to E4.3 | The oracle gate runs in CI and passes |
| M5 Agent review and design quality | People can see, review and revert agent work | E5.1 to E5.5 | The review loop works on a real agent run |
| M6 V2 and the W1 gate | Evidence that the product is good enough | E6.1 to E6.2 | Owner score 4/5 or better on 3 of 4 real screens |

## M0 Foundation

Goal: the skeleton every feature plugs into, with the standard enforced from the first commit.

### E0.1 Workspace layout by kind (S)
Crates under `crates/{core,engines,integrations,features,app,ui}`, apps under `apps/{server,web}`, each crate declaring its kind (ADR-0007).
- [x] Every crate declares `kind` (and `group` for features) in its manifest; a crate without one fails the check.
- [x] `cargo xtask deps-check` enforces the matrix in `docs/architecture.md` section 3b; its rules are a pure function with unit tests, including one failing case per rule.
- [x] `cargo xtask build-matrix` builds the product with no features and with each feature alone.
- [x] `cargo xtask portable-check` compiles core, engine, integration, feature and app crates for `wasm32-unknown-unknown`.
- [x] `unsafe` is denied workspace-wide.

### E0.2 CI and review gates (S)
- [x] CI runs format, lint (warnings as errors), tests, `deps-check`, `build-matrix`, `portable-check`, `size-check`, `ui-literals`, `licences --check`. (The fast set runs on every pull request; `build-matrix`, `portable-check`, `size-check`, `ui-literals` and `licences --check` run nightly and on demand; `just check` runs all of them except `licences --check`, which is run as `cargo xtask licences --check`.)
- [x] `size-check` enforces 500 lines per file and 2,000 per crate with an allow-list that only shrinks.
- [x] A local Cora agent review is run and recorded for every pull request (CI Cora is disabled); branch-name and message-standard checks reject AI attribution and wrong prefixes.
- [x] `develop` requires CI to pass before merge; `main` exists and receives release tags only.

### E0.3 Protocol and schema pipeline (S)
- [x] Wire and model types live in `meldframe-protocol`; TypeScript types are generated and a CI check fails on drift.
- [x] Every stored document and snapshot carries a schema version from the first migration.
- [x] Postgres migrations run from a clean database in CI.

### E0.4 Local development environment (S)
- [x] One command starts Postgres and the server for development; one command starts the web client against it. (`just dev` is smoke-tested (`tools/dev-smoke.sh`, in the migrations CI job); `just web` builds the WASM bridge when it is missing and starts the editor on `file_dev` with `/v1` proxied to that server through the guarded dev proxy. Proven on 17 Nov 2026 with `apps/web/scripts/editor-e2e.mjs`: against the real server and the real WASM the page connects and draws the saved file, shows a change of an agent without a reload, and reconnects after the server was stopped and started again.)
- [x] A seeded development workspace, file and token exist for agents and tests (development only, never in a release build).
- [x] `AGENTS.md` lists the commands; a new session can run the full test suite from a fresh clone.

**M0 exit criteria:** a pull request that breaks a layering rule, a size ceiling or a UI literal is rejected by CI; a fresh clone builds and tests green with one command.

## M1 Connected skeleton (owner's first goal)

Goal: backend, a basic web editor and MCP connected end to end on the carried spike core. Authentication uses development tokens behind the gate's principal interface; Gatehouse arrives in M2 without changing callers.

### E1.1 Core seed (M)
Carry the document core from the spike (decision 14, survey section 6.1): `Command`, validation, history with per-actor undo, replica with rebase and op dependencies, fractional order, layout with the text measurer seam.
- [x] The spike's property tests (convergence, undo, rebase, dependencies) run in this repository and pass with at least 600 seeds per run in CI and more nightly. (600 seeds per run in `cargo test --workspace`; 6,000 in the nightly `property-tests` job.)
- [x] Mutation check: disabling the dependency rule or the per-actor undo rule makes a test fail. (`tools/mutation-check-sync.sh`: eighteen mutations, all killed.)
- [x] Names follow `GLOSSARY.md`; no spike ids remain in file or type names; comments are English.
- [x] The core compiles natively and to WASM with the same tests. (`just wasm-test` passes locally: 233 tests, the six core crates plus the two property-test crates, from the same sources. The `wasm-test` job of the nightly workflow passed on a runner: run 4129, job 13095.)

### E1.2 Room, persistence and snapshots (M)
- [x] One room per file accepts ops in one total order, persists them append-only with actor, idempotency key and sequence number, and snapshots periodically. (`Server` opens one room per file through `meldframe-rooms::Rooms` with a store per backing, `PgStore` or memory; `apps/server/tests/rooms.rs` proves two files stay apart, a retried key writes nothing and a closed file returns with its content; the store contract, snapshots and Postgres cases are in `pg_contract.rs`. Caveat: files are namespaced by workspace in the store key, not in a schema column, until E2.3; the binary picks `PgStore` when `DATABASE_URL` is set (`just dev`, `tools/dev-smoke.sh`).)
- [x] Restarting the server restores every file from snapshot plus log with no change in content (test). (`crates/core/meldframe-server-tests/tests/pg_server.rs` builds a `Server` on Postgres, writes 520 ops to one file (past the 500-op snapshot interval) and one to another, drops the server, builds a new one on a fresh connection and compares both trees and sequence numbers, then checks that old keys are still remembered and the next number continues. It skips without `DATABASE_URL`; with `MELDFRAME_REQUIRE_PG` a missing URL fails. The restart test restarts a `Server` on real Postgres, not only a `Room`. A second test restarts and checks that another workspace opening the same file name gets an empty file. Caveats: workspace namespacing is by key, not a schema column (E2.3); the binary now picks `PgStore` from `DATABASE_URL` (`tools/dev-smoke.sh`).)
- [x] A retried op with the same key returns the same sequence number and writes nothing (test). (At the `Store` and `Room` seams. The store refuses the same key with another command (`KeyReused`) and the room reports it as `invalid`. `PgStore` passes the same cases through the shared `meldframe-store-contract` suite, which the in-memory adapter also runs.)
- [x] A dependent op is accepted only if every dependency was applied (carried F3 behaviour, test). (In `Room::accept`, with a property test of replicas fed by the real gate and room; applied keys are kept in the log. The rule is written down in `docs/architecture.md` 7a, one test per clause in `meldframe-rooms/tests/dependencies.rs`.)

### E1.3 Admission gate (M)
ADR-0010; the survey's candidate 2.
- [x] WebSocket, MCP and REST submit through one gate; the room accepts a write only with a permit the gate creates, so a path that skips the gate does not compile. (Proven: the permit and the compile-fail proof in `meldframe-rooms`, and `Server::submit_tool` takes a file and goes Gate, Admission, `Rooms::accept` with no `&mut Room` exposed. The WebSocket and REST transports now call `Server::submit_tool` through one handler in `meldframe-http` and have no other write route (tests). MCP calls the same `Shared::submit_ops` on the same router state; `meldframe-mcp-tests/tests/gate.rs` proves one submitter for an MCP and a REST write and one rate budget per principal across both.)
- [x] Refusals have stable codes and English messages (`forbidden`, `quota_exceeded`, `conflict`, `invalid`, `dependency_rejected`, ...), listed in one table that a test checks against the code. (`docs/error-codes.md` has the gate table; `tests/refusal_codes.rs` in `meldframe-host` checks it.)
- [x] Rate limiting happens before parsing; per-call timeouts apply (carried H1 behaviour, tests). (`meldframe-http`: per-address and per-principal token buckets with an injected clock, spent before the body is read, proven with a body that is never polled; body size limit; per-call timeout. Limits: the budgets are in memory per process, and a timed-out call may still write.)
- [x] The principal comes from development tokens through an interface that Gatehouse implements in M2. (`Authenticator` trait; `DevTokens` reads `MELDFRAME_DEV_TOKENS` from the environment, never argv.)

### E1.4 Live events (S)
ADR-0009. Status: the stream logic is the `meldframe-stream` crate (pure, no transport; `crates/core/meldframe-stream/tests/stream.rs`). `Server::submit_tool` publishes each op the room wrote (not a retried key) to the hub of its file, and `Server::subscribe` reads the range older than the window from the store (`crates/core/meldframe-live-tests/tests/live.rs`). The transport is `GET /v1/live/{file}?after={seq}` in `meldframe-http-live` (built on `meldframe-http`; `live_router` or `routes`; one socket per file, so the handshake answers 401, 429 or 503 as a status and one connection owns one stream), with the `resync` and `heartbeat` frames in the protocol crate, and a timer loop (`Timer::run`, spawned by the binary) that calls `Server::tick(now)` on a schedule while client traffic calls `Server::seen`; it is tested over a real socket in `crates/core/meldframe-live-tests/tests/ws.rs` (the basic paths), `wsflow.rs` (wake-up, replay and drain, slow reader, cut replay, several readers, a flooding client) and `wsrobust.rs` (a poll that times out, a stuck lock, `subscribe` rules, a stream dropped before its socket registered, a failing resubscribe). Left: the producers of ephemeral frames (capture requests in E1.7, touch hints), which today only a test publishes through `SharedServer::publish_ephemeral`.
- [x] Accepted ops reach every open client as durable frames with sequence numbers (proven with two clients of one file, a drain of 150 frames, and an op published during the replay); a client that reconnects with its last sequence number receives exactly the missed frames (also after a poll that timed out and closed the socket; within the retention: a gap longer than the replay limit of 1024 ends with a `resync` marker, and the client continues from it).
- [x] Ephemeral frames (capture requests, touch hints) carry no sequence number and are never replayed.
- [x] A slow client whose queue is full gets a resync marker and recovers; the room never waits for it (test with a stalled client, and with a replay cut at its limit). A client that stops reading is dropped by the send timeout and the writers go on (test); it reconnects with its last number.
- [x] Concurrent streams are capped; heartbeats are tested against a real clock.

### E1.5 MCP surface v1 (M)
- [x] Tools: read the file tree and nodes, apply commands (atomic by default for multi-command calls), report limits (`whoami`), batch status. Descriptions are English and name the batch limit. (`get_tree`, `get_nodes`, `apply_commands`, `whoami`, `batch_status`; `surface.rs` drives each with an rmcp client; atomic is a dry run under the server lock, see `docs/architecture.md`.)
- [x] Idempotency keys work in both spellings; `ifSeq` rejects a stale edit with `conflict` and says what changed. (`idempotencyKey` and `idempotency_key`, `ifSeq` and `if_seq`; the conflict lists the changes, the node's current data and the file's seq. Limits: an atomic call guards every node a command touches for the first time; `ifSeq` must be within 2,000 changes of the file; see `docs/architecture.md`.)
- [ ] A real MCP client (the rmcp client in tests, and Claude Code by hand) completes the M1 demonstration below. (Proven with the rmcp client in `demo.rs`: steps 1, 2 and 6 in part, with the person editing over REST. Steps 3, 4 and 5 are proven headless by `meldframe-capture-tests/tests/m1_headless.rs` against the composed server: the person is a `meldframe-app` editor on the live WebSocket (ops over REST); its undo reverts only its own move and the agent's nodes stay; a dropped socket reconnects with its last number and receives exactly the missed ops and converges; the agent's `capture` returns the PNG the editor uploads; the log shows an actor for every write, but it holds no gate decision. Open: Claude Code by hand, and the same steps in the real browser editor.)
- [x] Tool text and schemas are listed in a catalogue that a test compares with the server. (`docs/mcp-tools.json`; `catalogue.rs` compares it with the tools an rmcp client lists and with the registry; regenerate with the ignored `regenerate` test.)

### E1.6 App layer and basic web editor (L, split into slices)
ADR-0008. First vertical slice of the app layer: the editor shell.
- [x] `meldframe-app` holds selection, current tool, panel state, connection state and the history buttons as plain data with actions, effects and events; a test harness drives it without the web client. (Tests: `crates/core/meldframe-app-unit-tests/tests`, and `crates/core/meldframe-app-tests` against the real in-memory server: two editors and an agent, ops in flight at a drop and sent again, a real refusal, concurrent edits, a restart with a reused prefix, undo of a selected node, heartbeats past the silence limit. Snapshot reload after `Halted` is `Action::Reload` (issue 90); the shell loading the snapshot after `ReplayOverflow`, the WASM export and the web client are open in later slices.)
- [ ] The web client (Svelte 5 + PixiJS) shows layers on the left, the canvas in the centre and properties on the right, and only renders app state and dispatches actions. (In: the canvas (`packages/web-canvas`, `apps/web/src/canvas`), the shell (`packages/web-link`), the WASM bridge and the editor page with the connection state and selection. Open: the layers panel, the properties panel and the tool bar.)
- [ ] A person can create a frame, a rectangle and a text node, move and resize them, edit text and basic properties, and undo or redo their own changes.
- [ ] UI colours, fonts and sizes come from the tokens module; the literal lint passes.
- [ ] Changes from an agent appear without reload and are labelled with the actor in the layers panel. (Server side: `GET /v1/files/{file}` returns each node with `lastActor` and `lastSeq`, and live streams carry an ephemeral `touch` frame for the touch ring. Editor side: a change of an agent appears without a reload, proven in the browser (`apps/web/scripts/editor-e2e.mjs`); the actor label in the layers panel and the touch ring are open.)
- [ ] Browser end-to-end tests cover create, edit, undo, a dropped connection and the reconnect.

### E1.7 Capture relay (S)
Carried from the spike (S13); headless capture waits for a real need (decision 18).
- [x] An agent can capture a node or a frame as PNG while an editor has the file open; without one it gets `no_capable_client`. (Proven end to end with a fake editor client on the real live WebSocket in `meldframe-capture-tests`; the web editor's own capture client arrives with E1.6.)
- [x] Capture uses the file bound to the session; a different file is refused with `room_mismatch`. (The session is the principal: the first call that names a file binds it; see `meldframe-http-capture`.)
- [x] Capture is a feature module that registers its tool with a Read risk tier.

**M1 exit criteria (demonstration, recorded in the pull request that closes the milestone):**
1. Claude Code, connected through MCP, builds a landing hero (a frame with heading, text and a button) in one atomic batch; the open editor shows it without reload.
2. The person moves the heading; the agent's next read shows the new position; the agent's stale edit of the heading is rejected with `conflict`.
3. The person presses undo: only their move is undone; the agent's work stays.
4. The browser drops its connection and reconnects: it receives exactly the missed changes.
5. The agent captures the frame and receives a PNG.
6. Every write in the run passed the one gate (log shows actor and decision for each).

## M2 Platform and agent identity

Goal: real users, tenants and agents (decisions 10 to 12; ADR-0006, ADR-0010).

### E2.1 Gatehouse login (M)
- [ ] People sign in with Gatehouse (OIDC); the principal carries user and workspace.
- [ ] Sessions are revocable; logout ends the WebSocket.
- [ ] Development tokens are disabled in release builds. (Partly: a release build refuses to start with dev tokens unless `MELDFRAME_ALLOW_DEV_TOKENS=1`, `check_dev_tokens_allowed`; open until Gatehouse replaces them and the override goes.)

### E2.2 MCP OAuth through Gatehouse (M)
First platform slice (risk R9).
- [ ] Claude Code connects to the MCP endpoint through an OAuth flow with Gatehouse and acts as an authorised agent of the signed-in user.
- [ ] The client type and scopes chosen in the questionnaire are documented in an ADR.

### E2.3 Workspaces and quotas (M)
- [ ] Every file, variable collection, asset and counter belongs to one workspace; a query without the workspace in the principal returns nothing (tests per table).
- [ ] Quotas per workspace (ops per minute, batch size, storage) with stable refusal codes; counters survive a restart.

### E2.4 Agent grants, risk tiers and activity log (M)
- [ ] An agent connects through an OAuth grant (decision 31): no API key or token is copied by hand, access tokens are short-lived, and revoking the grant ends the session at once so the next call is refused. The consent screen lists the risk tiers.
- [ ] The release build refuses development tokens (decision 25, ADR-0012).
- [ ] Calls carry risk tiers (Read, Change, Delete) with the default policy from the questionnaire; an agent deleting a whole frame or a large subtree is asked first.
- [ ] Every call is logged with actor, tool, risk, decision and outcome; logs carry no content.

### E2.5 Assets on local disk (S)
- [ ] Upload png, jpeg, webp or gif (SVG refused); content-addressed; stored on local disk behind a blob interface.
- [ ] Image fills (cover, contain, fill) render on the canvas; a missing asset is reported as `asset_not_found`.

**M2 exit criteria:** a signed-in person and an authorised Claude Code session work in one workspace; another workspace cannot see that file; revoking the agent stops it on its next call.

## M3 Model round 2 and variables

Goal: the layout and paint model of W1 and the design system on the canvas (FR-4, FR-5, FR-9 to FR-11; ADR-0003, ADR-0004).

### E3.1 Render model (M)
The survey's candidate 1.
- [ ] One `resolve` step turns nodes and variables into painted boxes; the canvas and export both consume it.
- [ ] A coverage test fails when a node field is rendered by one consumer and not the other, unless it is marked unsupported.

### E3.2 Layout and paint round 2 (M)
- [ ] Margin, padding per side, absolute children inside auto layout, gradients, opacity and no-fill are commands, validated, undoable and rendered.
- [ ] The validator normalises fill under hug and warns on overflow (contract rules G1 to G3).
- [ ] Frame presets Desktop, Tablet and Mobile.

### E3.3 Variables in the core (M)
- [ ] Collections, modes, variables and aliases with stable ids; bindings `T | {var: id}`; alias cycles and missing variables are refused at command time.
- [ ] Variable commands use the same undo, `ifSeq` and atomic batches as nodes.
- [ ] The canvas renders resolved variables; switching mode re-renders without changing the document.

### E3.4 Variables feature: import and panel (M)
- [ ] Import from CSS variables and Tailwind v4 `@theme` text produces collections, modes and aliases with an import report; no user code runs on the server.
- [ ] An agent tool `import_tokens` (Change tier) accepts text that the agent read locally.
- [ ] The Variables panel lists, searches, edits and binds variables; its rules live in the app layer.
- [ ] A real Tailwind design system (the Uteke Cloud theme) is the import test corpus.

### E3.5 Layer names (S)
- [ ] A lint flags default layer names; MCP descriptions ask agents for semantic names.

**M3 exit criteria:** the Uteke Cloud design system is imported, its variables are bound to a frame by an agent, and switching light and dark mode re-renders the frame on the canvas.

## M4 Export and fidelity

Goal: exported HTML/Tailwind that matches the canvas (FR-12, FR-13, NFR-2; ADR-0003).

### E4.1 Export feature (M)
- [ ] HTML and Tailwind v4 with CSS variables, generated from the render model, with `var(--...)` for bound fields and an unsupported-property report.
- [ ] Every M3 property is in the supported table of the fidelity contract with a fixture and a probe.
- [ ] An agent tool exports a frame (Read tier).

### E4.2 Oracle gate in CI (M)
- [ ] The harness from `experiments/fidelity-oracle` runs against the product export in CI: geometry within 0.05 px, text ink within 1 px, paint probes, Tailwind equal to plain CSS.
- [ ] Pull requests run fixtures plus 50 fuzz seeds; nightly runs 500 or more; a known-gap fixture that starts passing fails the job.

### E4.3 PNG export (S)
- [ ] A frame exports to PNG at 1x and 2x from the editor and through MCP.

**M4 exit criteria:** the oracle gate is required on `develop` and passes; a landing frame exported to HTML/Tailwind renders the same boxes as the canvas within the contract.

## M5 Agent review and design quality

Goal: people can follow, review and revert agent work (FR-16 to FR-18, FR-21; ADR-0005).

### E5.1 Activity panel (M)
- [ ] One lane per agent session with batches, notes (plain text, length-limited) and status; its rules live in the app layer.
- [ ] Touch hints show which nodes an agent is changing (ephemeral frames).

### E5.2 Review and revert (M)
- [ ] Reverting a batch restores only the fields whose last writer is still that agent, as one atomic change authored by the person, which the person can undo.
- [ ] A test fails if the "still last writer" check is removed.

### E5.3 Checkpoints (S)
- [ ] Automatic checkpoint at the start of an agent run and before a revert; restore appends a change and never rewrites history.

### E5.4 Design review and variants (M)
- [ ] A `design_review` tool stores a numbered critique list per file and links items to the batches that address them.
- [ ] Frame duplication as a variant from the editor and through MCP.

### E5.5 Agent guidance (S)
- [ ] A shipped skill or prompt tells agents to use small top-down batches, semantic names, notes, `atomic` for multi-command edits and `ifSeq` for edits of existing nodes, and to duplicate a frame before a large rewrite.

**M5 exit criteria:** on a real Claude Code run, the person sees each batch with its note, reverts one batch, and the rest of the agent's work stays intact.

## M6 V2 and the W1 gate

### E6.1 V2 evaluation (M)
- [ ] Four real screens (proposed: Uteke Cloud landing, pricing, compare page, and one Uteke Mobile screen) are designed through MCP and compared blind with Wonder output.
- [ ] The owner scores each; results and the runs are recorded in `docs/evaluations/`.

### E6.2 Fix round and release (M)
- [ ] Findings from V2 are triaged into issues; blocking ones are fixed.
- [ ] `main` receives the first tag (v0.1.0) with a changelog.

**W1 gate:** owner score 4/5 or better on 3 of 4 screens (decision 9).

## After W1 (not planned in detail)

Carousel and slides (C1), motion (Mo1), headless capture (decision 18), components and instances (decision 19), desktop with a local agent over ACP (phase D), brand memory through Uteke, PDF export, responsive export.
