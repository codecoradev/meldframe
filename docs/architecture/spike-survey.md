# Spike architecture survey

Status: input for the Meldframe seed decision. Read-only survey of the Rust-core spike.

**Update 13 Nov 2026:** the owner's decisions after this survey (ADR-0007 to ADR-0010 and `docs/architecture.md`) supersede the workspace layout in section 6.4 and refine candidates 1, 2, 3 and 6. Section 8 maps each one. The findings and evidence in sections 1 to 5 and 7 stand.
Vocabulary: this document uses the terms of the `codebase-design` skill. **Module** (anything with an interface and an implementation), **interface** (everything a caller must know: signature, invariants, ordering, error modes, configuration, performance), **seam** (where an interface lives), **adapter** (a thing that satisfies an interface at a seam), **depth** (leverage at the interface), **leverage** and **locality**. One adapter makes a hypothetical seam; two make a real one.

Surveyed code: `wonder-proto`, branch `spike/rust-core` (123 commits, HEAD `ca74052`). Every path below is relative to that repository unless stated. Paths followed by `:line` are evidence; counts come from `grep`/`wc` run during the survey. Section 7 lists what was not checked.

Product context: `docs/concept.md` and `docs/decisions.md` in this repository (decisions 10..19 are mapped in section 6).

## 0. Summary

1. The spike has **one genuinely deep module**: `spike-core` (`crates/core`), whose small `Command` vocabulary (5 variants, `crates/protocol/src/lib.rs:396`) sits in front of validation, fractional ordering, undo, multiplayer rebase and layout. It is shared by the server (native) and the client (WASM), and it is the reason convergence tests are cheap.
2. The server is **deep in the room actor** (`apps/server/src/room.rs`) but **shallow and duplicated around it**: the op submission policy (authorization, mode, quota, timeout, error mapping) is written separately in `ws.rs` (the `Op` arm, `ws.rs:341-415`) and `mcp.rs` (`apply_commands`, `mcp.rs:167-318`), with a third partial copy in `assets.rs` and `capture.rs`.
3. The biggest product risk is not in the server. It is that **there is no single render model**. The canvas (`src/lib/Canvas.svelte`, PixiJS) and the HTML/Tailwind export (`crates/export/src/style.rs`) each re-derive paint from `Node` fields. The M3A merge (`e0da632`) added `fontWeight`, `textAlign`, `border` and `shadows` to the model, the canvas and the MCP description, and **did not touch `crates/export`**: the export still hardcodes `font-weight: 400` (`crates/export/src/style.rs:246`) and has no `box-shadow` or border output. That is the fidelity contract (decision 15) already drifting inside the spike.
4. Persistence has **one adapter** (Postgres, 15 `sqlx::query` call sites in `room.rs`); server integration tests need a live Postgres (`apps/server/tests/common/mod.rs:35`).
5. Capture has **one adapter** (browser relay, `capture.rs` + `src/lib/capture.js`). The seam is real in protocol terms (`CaptureRequest`/`CaptureFailed`) but hypothetical in adapter count, which matches open decision 18.
6. The only seam with three real adapters is `TextMeasurer` (`crates/core/src/lib.rs:26`): `StubMeasurer`, `TableMeasurer` (export), `JsMeasurer` (WASM). It is also where the HTML export's fidelity depends on a browser-recorded table.

Ranked deepening candidates are in section 5; the seed recommendation is in section 6.

## 1. Module map

Sizes are line counts from `wc -l` (source files only, tests listed separately). "Interface items" counts `pub` functions/types a caller sees plus the invariants called out in the text; it is a rough measure, not a metric.

### 1.1 Rust workspace

`Cargo.toml` members: `protocol`, `core`, `core-wasm`, `export`, `pptx`, `apps/server`.

| Module | Key files (lines) | Interface (what a caller must know) | Implementation | Depth |
|---|---|---|---|---|
| `spike-protocol` | `crates/protocol/src/lib.rs` (671) | Wire and model types: `Node` (29 pub fields, `:223`), `NodePatch` (25 fields, `:298`), `Command` (5 variants, `:396`), `ClientMsg` (7), `ServerMsg` (8); `deny_unknown_fields` everywhere; `MAX_OP_DEPS = 512`; TS types generated via `ts-rs` into `packages/protocol` and checked by `tools/check-protocol.sh`. | Mostly declarations and serde attributes; `apply_patch` (`:380`). | Shallow by design (a data module). Its value is being the single definition; see friction 4.3 for what it costs. |
| `spike-core` / `apply` | `crates/core/src/apply.rs` (625) | `apply_command(nodes, cmd, limits)`; `check_command`; 10 pub items. Invariants: validate first, mutate after (`apply.rs:1-4`); on error the document is unchanged; sibling order is `(pos, id)`, not list order; `Limits` has 9 caps (`:14`); `ApplyError` has about 20 variants (`:38`). | Validation, cycle and depth checks, fractional index planning (`plan`), subtree removal. | **Deep.** 5 commands in, one rule set out. 30 call sites in `crates/core/tests/apply.rs`, 6 in `apps/server/src/mcp.rs`, 4 in `room.rs`. |
| `spike-core` / `document` | `crates/core/src/document.rs` (598) | `Document`: 23 pub methods. Local history with inverse commands, groups (`begin_group`/`end_group`), keyed coalescing (`apply_keyed`), `apply_remote`, `drain_changes`. Ordering rule: inverses are recomputed from the real state at execution time (`document.rs:~12-15`). | Undo stack, change log, rebalance. | Medium-deep. The interface is wide (23) but each method maps to a user-visible behaviour. |
| `spike-core` / `replica` | `crates/core/src/replica.rs` (284) | `Replica`: 27 pub items. Optimistic local apply, outbox of `Outgoing` ops with idempotency keys and `deps`, `on_event` (confirm/rebase), `on_reject`, `check_consistency`. | Pending queue, rebase onto server events, rejection cascade (F2). | **Deep** for what it hides (rebase, selective undo, dependency rejection). Tested by property tests (`crates/core/tests/replica_props.rs`, 724 lines, 2 `#[test]` functions that run seeded random streams). |
| `spike-core` / layout | `crates/core/src/lib.rs` (294) | `compute_rects(nodes, &dyn TextMeasurer)` and `compute_rects_indexed`; `TextMeasurer` trait (`:26`). Invariant: text measurement is injected, results are deterministic for a given measurer. | Two-phase hug/fill sizing with memo, justify/align placement (`lib.rs:~80-290`). | Deep: two functions in, full auto-layout out. |
| `spike-core` / `fracindex` | `crates/core/src/fracindex.rs` (284) | Fractional position strings; called only through `apply`. | Key generation and ordering. | Deep and internal; 16 tests in `crates/core/tests/fractional.rs`. |
| `spike-core-wasm` | `crates/core-wasm/src/{lib,client,layout}.rs` (156, 202, 150) | Two wasm-bindgen classes: `Doc` (22 pub methods, local editor and live mirror) and `Client` (26 pub methods, connected editor on top of `Replica`). Callers: `Doc` at `src/lib/core-wasm.js:3`, `src/lib/live.js:4`; `Client` at `src/lib/online.js:68`. JS passes a `measureBatch` function; results arrive as `Float64Array` rects. | Adapts `Document`/`Replica` to JS values, batches text measurement (`layout.rs` `JsMeasurer`, key includes `fontWeight`, `:11`). | Shallow-to-medium: about 48 pub methods that are mostly one-line delegations. Two classes expose overlapping surfaces (see candidate 3). |
| `spike-export` | `crates/export/src/{html,tailwind,style,flutter,measure,lib}.rs` (109, 87, 308, 397, 60, 66) | `to_html`, `to_tailwind`, `to_flutter` over `(nodes, ExportOpts)` returning `Exported { code, warnings }`; `TableMeasurer` (recorded browser metrics). Ordering: export asks core for rects first (`style.rs:49`, `:300`). | Re-derives CSS/Dart declarations from node fields (`style::decls`, `style.rs:95-288`), mirrors core layout semantics by hand (comment at `style.rs:3`). | Medium. The interface is small (3 functions) but the implementation re-states layout and paint semantics that live elsewhere (friction 4.2). Flutter is out of scope. |
| `spike-pptx` | `crates/pptx/src/*` (about 950) | Out of scope for the seed (decision: PPTX last). Mentioned only because it also depends on `core` and `protocol` and defines its own `Document` type (`crates/pptx/src/lib.rs:26`), which is a name collision with `spike_core::Document`. | | n/a |

### 1.2 Server (`apps/server`, 4,837 source lines, 14 files)

| Module | File (lines) | Interface | Implementation | Depth |
|---|---|---|---|---|
| Room actor | `room.rs` (880) | `RoomHandle` has 8 methods: `op`, `op_if`, `op_with_deps`, `batch_atomic`, `note_rejected`, `subscribe`, `snapshot`, `poke` (`room.rs:240-302`). Ordering: one task per room serializes ops, so `seq` is a total order. Error modes: `OpError` with 7 variants and machine codes (`:23-65`). Idempotency keys are scoped per actor (`scope_key`, `:137`). Preconditions (`ifSeq`) and dependencies (`deps`) are checked inside. | CAS on `documents.seq`, idempotency lookup, footprint log for `ifSeq`, rejected-key memory (F2), snapshot cadence, LISTEN/NOTIFY catch-up for multi-instance (`room.rs:835`). `handle_op` is 132 lines (`:517-647`), `handle_batch` 138 (`:649-786`). | **Deep.** 8 methods in front of the whole write path. The one place where complexity is correctly concentrated. |
| WebSocket adapter | `ws.rs` (465) | Protocol per `ClientMsg`/`ServerMsg`. Pre-parse frame limiting (`FrameLimiter`), per-connection token bucket, slow-consumer handling. | `handle_msg` is 149 lines (`ws.rs:268-416`). | Medium. Contains transport work (framing, backpressure) and policy work (authz, quota, error mapping) in one function. |
| MCP adapter | `mcp.rs` (354), `capture.rs` (603, MCP part from `:535`), `assets.rs` (493, tools from `:424`) | 4 tools in `mcp.rs` (`get_document`, `whoami`, `batch_status`, `apply_commands`) + `capture` + `upload_asset` + `request_asset_upload`. | `apply_commands` is 152 lines (`mcp.rs:167-318`) and re-implements sequencing, atomic mode, cancellation, timeouts and reporting. | Shallow around a deep room: the adapter contains policy (see 4.1). |
| REST | `http.rs` (107) | `GET /rooms/{room}/snapshot`, `/health`, `/stats`, plus `session::routes()`, `assets::routes()`, `POST /captures/{id}`. **There is no REST write path and no REST export endpoint**; export exists only as a CLI/test (`crates/export/src/bin/spike-export.rs`). | Router, CORS, ingress guard layers. | Thin and appropriate. |
| Sessions and gate | `session.rs` (803) | `gate(app, parts, room, Kind)` (`:734`) for MCP; `resolve`/`resolve_http` (`:478`, `:489`) for HTTP; session binding, revocation, token hashing. | Postgres-backed session rows (7 `sqlx::query` sites), in-process cache, LISTEN/NOTIFY revoke. | Medium. `gate` is a good interface for MCP; WS uses a different function (`session_room_check`, `ws.rs:453`). |
| Quotas | `quota.rs` (416) | `begin_call`, `reserve`, `settle`, `check_size` (`:139-302`). Ordering: reserve, do the work, settle with actual usage. State is in memory, not durable and not shared across instances (`quota.rs:1-3`). | Sliding-window logs. | Deep and self-contained, but callers must remember the reserve/settle protocol (3 call sites outside the module: `ws.rs`, `mcp.rs` x2). |
| Capture hub | `capture.rs` (603) | `CaptureHub::capture(...)` (`:262`) picks a subscribed client, sends `CaptureRequest`, waits for an HTTP upload, returns PNG or a structured error. Error vocabulary documented in `docs/s13-capture-relay.md`. | Per-room slots, failover, upload tokens, rate limits. | Deep. Its interface is one async function plus stats. |
| Assets | `assets.rs` (493) | Content-addressed store keyed by SHA-256, magic-byte sniffing (`:23`), one-time upload tickets. | Postgres rows (5 `sqlx::query` sites). | Medium. Storage is hard-wired to Postgres, decision 12 wants disk then object storage. |
| Ingress / batch / config / state | `ingress.rs` (289), `batch.rs` (120), `config.rs` (177), `state.rs` (93) | Rate guard before parsing; batch status registry; configuration; `AppState` (10 fields). | | Support modules. `AppState` is a service-locator-style bag (`state.rs:57-68`), acceptable at this size. |

Server tests: 16 files in `apps/server/tests` with 110 `tokio::test` functions plus a 248-line harness (`common/mod.rs`, 21 pub helpers) that boots the real router on a random port against Postgres.

### 1.3 Client (`src/`, 3,107 lines of JS/Svelte, plus `packages/protocol`)

| Module | File (lines) | Notes | Depth |
|---|---|---|---|
| Document mirror | `src/lib/doc.svelte.js` (412) | Svelte state mirror of the WASM core; "only door for mutations" is `dispatch` (`:129`). `sync()` applies `drain_changes` (`:100-127`). | Medium. Good single door, but local persistence (`localStorage`, `:8`) and live modes (`live`/`online` state, `:66-70`) share the file. |
| Transport | `src/lib/net.js` (246), `online.js` (93), `live.js` (147) | `online.js` wires `Client` to WS; `live.js` is a second, read-only transport using `Doc` as a mirror. | Two transports with overlapping reconnect/replay logic (not diffed in detail). |
| Renderer | `src/lib/Canvas.svelte` (787) + `shaders.js`, `shader.worker.js`, `imagefill.js`, `images.js` | PixiJS v8. Paints fills, text, border, shadows, image fills, shaders. | Large, single-file renderer; the only consumer of paint semantics on the canvas side. |
| Capture client | `src/lib/capture.js` (87) | Renders a region offscreen through the Pixi renderer registered by `Canvas.svelte` (`setCaptureRenderer`, `:12`), uploads PNG. | Small and deep for what it hides (seq sync, image settle, size limits). |
| Layout oracle | `src/lib/layout.js` (125) + `layout.test.mjs` | A JS layout implementation kept as the oracle for the Rust port (`crates/core/tests/layout_oracle.rs`) and for the browser-metrics recorder (`tools/capture/measure`). Its last real change was Q1.5; M3A did not touch it. | A third copy of layout semantics (see 4.2). |

### 1.4 Spike docs

`docs/` has 16 files (1,889 lines). They are valuable as rationale and measured results (S8 multi-agent, S9 design/code, S12 multiplayer undo, S13 capture, S15 sessions, S16 fractional index, F1/F2 undo anomalies, H1 hardening, M3A/M3B model growth). They are in Indonesian; the product repo language is English (decision 4, workflow).

## 2. Seams and adapter counts

| Seam | Where | Real adapters | Verdict |
|---|---|---|---|
| `Command` / `apply_command` | `crates/core/src/apply.rs:567`; `Command` in `crates/protocol/src/lib.rs:396` | Consumers, not adapters: server room (`room.rs`), `Document`/`Replica` (client), CLI export. The seam is the interface of a deep module, not a port. | Healthy as a module interface. It is the "one `Command` for UI, WebSocket, MCP and undo" principle of `docs/concept.md`. |
| `TextMeasurer` | `crates/core/src/lib.rs:26` | **3**: `StubMeasurer` (`lib.rs:35`), `TableMeasurer` (`crates/export/src/measure.rs:50`), `JsMeasurer` (`crates/core-wasm/src/layout.rs:58`). | **Real seam.** Note `MeasureEntry` (`measure.rs:13`) has no `fontWeight`, while `JsMeasurer`'s key does (`layout.rs:11`), so the table adapter and the browser adapter disagree on what identifies a measurement since M3A. |
| Render model (canvas vs HTML export) | Not a seam today. `Canvas.svelte` and `style.rs` each read `Node` fields directly. | 2 consumers, **0 shared module**. | **Missing seam.** Two implementations of "what a node looks like" with no common interface; this is where drift is observed (section 4.2). |
| Capture | `ClientMsg::Capabilities`/`CaptureFailed`, `ServerMsg::CaptureRequest` (`protocol/src/lib.rs:443-540`); `CaptureHub` (`capture.rs`) | **1** (browser relay, `src/lib/capture.js`). | Hypothetical as a port. The protocol is stable and documented, but there is no second renderer. Decision 18 compares relay vs native vs headless Chromium. Do not introduce a `Capturer` trait before a second adapter exists. |
| Room persistence | `room.rs` (15 direct `sqlx::query` sites) over `migrations/0001_init.sql` (documents, ops, snapshots) | **1** (Postgres). Tests also use real Postgres. | Hypothetical. A seam here would only be justified by an in-memory adapter for fast tests or by the desktop phase (decision 5). Today the persistence statements are interleaved with CAS logic (`room.rs:~575-620`), which is cohesive but unswappable. |
| Asset storage | `assets.rs` (5 `sqlx::query` sites) | **1** (Postgres). | Decision 12 (local disk now, object storage later) makes this a candidate for a real seam with two adapters; not yet present. |
| Transports over the room | WS (`ws.rs`), MCP (`mcp.rs`), REST snapshot (`http.rs`) | 3 callers of `RoomHandle`, but each re-implements admission (see 4.1). | The seam that exists, `RoomHandle`, is at the wrong altitude for policy: callers must reproduce policy before calling it. |
| Auth / principal | `TokenInfo` static map (`config.rs:8`), session tokens (`session.rs`), `resolve` (`session.rs:478`) | 2 token kinds produce the same `TokenInfo` / `Principal`. | Decision 10 (Gatehouse OIDC/OAuth) would be a third adapter behind `resolve`. Good insertion point already. |
| Wire types to TS | `ts-rs` generated files in `packages/protocol/src/generated`, drift check in `tools/check-protocol.sh` | 2 languages. | Real and enforced. |
| Core to browser | `crates/core-wasm` (wasm-bindgen), built by `tools/build-wasm.sh` | 2 targets (web, node for tests: `pkg/`, `pkg-node/`). | Real. |

## 3. Deletion test

| Module | If deleted... | Verdict |
|---|---|---|
| `spike-core` (`apply`/`replica`/layout) | Validation, ordering, undo, rebase would have to be rewritten in the server, in WASM glue and in the JS mirror: 3 places. | **Earns its keep** (complexity reappears in at least 3 callers). |
| `RoomHandle` / room actor | Total ordering, idempotency, CAS, `ifSeq`, `deps` would reappear in `ws.rs`, `mcp.rs` and `batch.rs`. | **Earns its keep.** |
| `CaptureHub` | Failover, per-room slots, token handling would move into `mcp.rs` and `ws.rs`. | Earns its keep. |
| `quota.rs` | Reserve/settle arithmetic would move into 3 call sites. | Earns its keep, but its protocol leaks (4.1). |
| `spike-core-wasm::Doc` | `Client` could serve local mode by running without a server: the logic reappears nowhere else. | **Mostly pass-through** of `Document` plus layout (22 delegating methods); the second class mainly exists because local and connected modes were built in different spikes (candidate 3). |
| `crates/export/src/tailwind.rs` (87 lines) | Replace by `utility(prop, val)` mapping in `html.rs`; complexity is small and local. | Thin but cohesive; not a priority. |
| `src/lib/layout.js` | Oracle tests and the metric recorder lose their reference implementation; production code does not use it. | Test-only. Keep until Rust is the sole oracle, then delete. |
| `http.rs` | Routes move into modules. | Pass-through, which is fine for a router. |
| `state.rs` `AppState` | Replaced by passing individual handles. | Pass-through bag; acceptable. |
| `spike-pptx`, `crates/export/src/flutter.rs` | Out of scope. | n/a |

## 4. Duplication and friction

### 4.1 The op submission policy is written three times

Evidence from `grep -c` over `apps/server/src`:

| Concern | `ws.rs` | `mcp.rs` | `session.rs` | `assets.rs` | `capture.rs` | `http.rs` |
|---|---|---|---|---|---|---|
| room authorization (`allows`/`session_room_check`/`gate`) | `ws.rs:344-349, 453-466` | via `gate` (4 tool sites) | `gate` `:734`, `resolve_http` | 2 `gate` + 1 `allows` | 1 `gate` + `resolve_room` + `info.allows` (`:~560`) | `snapshot` (`:~48`) |
| session end/revocation check (`end_reason()`) | `:353-357` | 2 sites (`:~185`, `:~300`) | 5 | 1 | | |
| read-only mode check | `ws.rs:358` | inside `gate` | `gate` | | | |
| quota reserve/settle protocol | `ws.rs:361-385,~397` | `mcp.rs:~205-215, ~308` | | `quotas.get` 1 | | |
| call timeout (`call_timeout_ms`) | `ws.rs:~395` | 3 sites (`:~220, ~270, ~300`) | | | | |
| `OpError` to wire error mapping | `ws.rs:404-413` | JSON built inline 5 times | | | | |

Specific duplicated logic:
- The "file selected in UI differs from `room`" rule is written twice with the same message and the same codes: `ws.rs:453-466` (`room_mismatch`, `no_file_selected`, `forbidden`) and `session.rs:748-765` (`gate`). They return different types (`ServerMsg` vs `CallToolResult`), so a rule change must be made in two places and tested through two transports.
- `matches!(c, Command::AddNode { .. })` is counted to charge node quota in `ws.rs` (1 site) and `mcp.rs` (3 sites).
- The quota protocol (`begin_call` -> `reserve` -> run -> `settle`) must be reproduced by every caller, including on early-return paths (`mcp.rs:~232-238` settles by hand before returning an error).
- Batch sequencing, partial application, cancellation and timeout live in `mcp.rs:167-318`, but the REST/WS paths have a different (single-op) policy with different error JSON. Agents see MCP error shapes `{index, message, retryable, code}` while WS clients see `ServerMsg::Error`.
- Not atomic by default: `apply_commands` applies a prefix unless `atomic: true` (`mcp.rs:51-53`). This is a documented behaviour (`docs/s8-multi-agent.md`), but it is part of the interface callers must know, and the product principle says "idempotent, atomic" (`docs/concept.md`). The default may need to change in the product.

### 4.2 Three copies of "what a node looks like", already drifting

1. Layout semantics: `crates/core/src/lib.rs` (authoritative), `src/lib/layout.js` (oracle), and a hand-mirrored description in `crates/export/src/style.rs:3` and its flex-basis workarounds (`style.rs:182` onward). `docs/s9-design-code.md` records five places where CSS/Flutter needed special handling to match core.
2. Paint semantics: `Canvas.svelte` (PixiJS) vs `style::decls`. Measured drift: `git show --stat e0da632` lists 27 files for the M3A feature and none under `crates/export`. In `style.rs`, `font-weight` is a constant 400 (`:246`), and a grep for `box-shadow` returns nothing; `border` appears only as `border-radius` (`:268-279`).
3. Text measurement identity: `JsMeasurer` keys include `fontWeight` (`core-wasm/src/layout.rs:11`), `TableMeasurer` does not (`export/src/measure.rs:13-27`), so a bold text recorded by the browser cannot be replayed by the export.

Consequence for decision 15 (oracle test, fidelity contract): the spike's oracle (`docs/s9-design-code.md`, A5: 0.000 px over 84 nodes, 0.96/255 mean pixel difference) was measured before M3A and has not been re-run on the richer model. This survey did not re-run it.

Shotgun surgery: adding a node property touches `Node` and `NodePatch` (54 `pub` fields combined), `apply_patch`, `check_node`/`check_patch`, the TS generated types, `Canvas.svelte`, `Props.svelte`, the MCP tool description text, and (not done in M3A) the export. The model round 2 in `docs/concept.md` (margin, per-side padding, absolute children, gradients, opacity, tokens) will multiply this.

### 4.3 Tests that must pass through a heavy interface

- Server behaviour is tested only through a real Postgres plus a real HTTP/WS server (`apps/server/tests/common/mod.rs:35`, `AppState::connect` runs migrations). The interface used by tests (`TestServer`, 21 helpers) is the transport; there is no way to test the room's CAS/idempotency logic without a database. Cost: slower and more fragile CI; benefit: tests are honest about the one adapter.
- The same room rules (ifSeq, deps, idempotency) are tested with WS in `s8.rs` (1,597 lines), `s15.rs` (1,192 lines) and again through MCP in `s7.rs` and others. These files are large because each test builds its own scenario through the transport.
- Core tests are good: they pass through `Document`, `Replica` and `apply_command` directly (`crates/core/tests/*`, 68 `#[test]` functions in `crates/core/tests`; the property test runs many seeded streams inside each).

### 4.4 What is hard for an AI agent to navigate

- File names carry spike ids, not domain: tests are `s6.rs`, `s8.rs`, `s15.rs`, `f2.rs`, `h1_batch.rs`, `m3b_refs.rs`; docs are `s13-capture-relay.md`. An agent has to read `README.md` to learn that S15 means "sessions".
- Two classes (`Doc`, `Client`) with overlapping method names in WASM and two transports (`live.js`, `online.js`) in JS; two `Limits` types in one crate graph (`spike_core::Limits` at `apply.rs:14` and `quota::Limits` at `quota.rs:15`), and two `Document` types in the workspace (`spike_core::Document`, `spike_pptx::Document`).
- Language: Rust comments, error messages returned to agents and docs are mostly Indonesian (for example `room tertutup`, `dibatalkan sebelum diterapkan; tidak ada perubahan` at `room.rs:~55`, `mcp.rs:~178`), while MCP tool descriptions are English (Q1 translated them). Decision 4 (English UI, global developers) and agent-facing error text imply translating messages that agents read.
- The longest functions are the policy-heavy ones: `apply_commands` (152 lines), `handle_msg` (149), `handle_batch` (138), `handle_op` (132). A change to admission policy requires reading all of them.
- Large single files: `Canvas.svelte` (787), `session.rs` (803), `room.rs` (880), and JS state in `doc.svelte.js` (412) mixing three modes (local, live, online).
- Positive: the `Command` vocabulary, `deny_unknown_fields`, generated TS types and the per-feature docs with measured results make most claims checkable.

## 5. Deepening candidates, ranked

Ranking is value (leverage and locality gained, product risk removed) against cost (rewrite size and risk). Dependency categories are from `DEEPENING.md`.

| Rank | Candidate | Value | Cost | Category |
|---|---|---|---|---|
| 1 | Shared render model for canvas and HTML/Tailwind export | Very high (decision 15, 17, 19) | High | In-process |
| 2 | Op submission pipeline (one admission and execution module for WS, MCP, REST) | High | Medium | In-process plus remote-but-owned (room) |
| 3 | One WASM client (`Client` with an offline mode; delete `Doc`) | Medium | Low-medium | In-process |
| 4 | Persistence port for the room (`OpLog`: Postgres + in-memory) | Medium | Medium | Local-substitutable |
| 5 | Asset storage port (Postgres/disk/object) | Medium, due when decision 12 is acted on | Low-medium | Local-substitutable |
| 6 | Capture seam (`Capturer`) | Low until a second adapter exists | Low | True external (browser) |
| 7 | Retire `layout.js` and spike-id naming | Low | Low | n/a |

### Candidate 1. Shared render model (canvas and export)

**Problem.** Paint semantics live in two places (section 4.2). Every model feature must be implemented twice and the second implementation is not gated: M3A skipped it. Decisions 15 and 17 cannot be met if "what a node looks like" has no single definition: token resolution, fallbacks and the fidelity contract all need one place to be stated.

**Proposed new interface (small).** One function in `spike-core`:

```
resolve(nodes: &[Node], tokens: &Tokens, m: &dyn TextMeasurer) -> Vec<Resolved>
```

`Resolved` is the fully resolved, token-free description of one node: world rect, resolved fills (solid and later gradients), border, shadows, opacity, corner radii, clip, resolved text style (family, size, weight, align, line height), image fill, and a list of `Degradation` notes for features with no exact HTML equivalent. Invariants: deterministic for equal inputs; token references are resolved here and nowhere else; list order is draw order.

**Design it twice.**

*Design A: resolved-node list (data in, data out).* The interface is the single `resolve` function above plus the `Resolved` type. The canvas (via WASM, as a typed array or JSON diff) and the HTML exporter both consume `Resolved`; neither reads `Node` paint fields. Strength: very small interface (one function, one type), maximum locality (a new property is added to `Resolved` once; both consumers fail to compile or ignore it visibly). Weakness: `Resolved` becomes the widest type in the system, and the Pixi side has to cross the WASM boundary with it on every change (cost concern on 1,000-node documents; the spike already optimised `all_rects` as a `Float64Array`, `core-wasm/src/client.rs:177`).

*Design B: property registry with paired emitters (declarative table).* Keep consumers reading `Node`, but add in core a table: for each paint property, `{ name, validate, to_css(value) -> Decl, canvas_hint }`, and an oracle test that fails if any `Node` field is not listed or lacks a CSS emitter or an explicit "unsupported" entry. Interface: a registry (a few items per property) and the test. Strength: low cost, no data-flow change, fits what exists. Weakness: the canvas still interprets properties on its own, so the duplication of paint logic remains; the registry only guards coverage, not correctness, and wide registry rows are a larger interface per property than Design A.

**Comparison.** Design A has higher depth (one function replaces two implementations) and gives the fidelity contract a place to live; Design B has higher short-term safety at low cost. Recommended: design for A, but land B's coverage test first (cheap, it would have caught M3A) and treat A as the W1 model-round-2 vehicle, because model round 2 (tokens, gradients, per-side padding, absolute children) rewrites both sides anyway. A hybrid works well: `Resolved` for HTML export and server-side use immediately, and the canvas migrates field by field.

**Leverage and locality.** One implementation serves canvas, export, capture verification and later carousel/slide frames. Token resolution (decision 17) and the fidelity contract (decision 15) get a single home.

**Risks.** Canvas performance across the WASM boundary; scope creep if `Resolved` absorbs layout inputs; PixiJS-specific features (shaders, filters) that never map to CSS must be explicit `Degradation` entries; the existing oracle numbers must be re-measured on the new model.

### Candidate 2. Op submission pipeline

**Problem.** Admission and execution policy is written in `ws.rs`, `mcp.rs` and partially in `assets.rs`/`capture.rs` (section 4.1). Rules and error shapes must be changed in several places, tests must pass through two transports, and decision 10 (Gatehouse) and 11 (workspace tenant) will add more checks to every copy.

**Proposed new interface (small).** One module in the server, transport-agnostic:

```
Submission { principal: &Principal, room: Option<&str>, kind: Read|Write|Meta,
             commands: Vec<Command>, key: Option<String>, if_seq: Option<u64>,
             deps: Vec<String>, atomic: bool }
submit(app: &AppState, s: Submission, cancel: CancellationToken) -> Report
Report { applied: Vec<(index, seq, duplicate)>, rejected: Option<Rejection { index, code, message, retryable, extra }>, latest_seq }
```

Invariants: authorization, session end, read-only mode, size and quota are all checked before any side effect; quota is reserved and settled inside; timeout and cancellation are handled inside; one error vocabulary (`code` strings already shared by `OpError::code()`, `room.rs:42`).

**Design it twice.**

*Design A: one `submit` entry point (the interface above).* WS `Op` builds a `Submission` with one command; MCP `apply_commands` builds one with N; a future REST write endpoint builds the same. Strength: depth is maximal (policy sits behind one function), tests for authz, quota and conflict codes are written once against `submit` with an in-process `AppState`. Weakness: single-op WS needs per-op acks and idempotency, so the report shape must carry per-command results; the WS hot path pays batch-shaped allocation (to be measured, not assumed).

*Design B: composable admission, thin executor.* Two small modules: `Admission::admit(principal, room, kind, cost) -> Admitted` (authz, session, mode, quota guard; RAII `settle` on drop) and the existing `RoomHandle` calls. Transports remain responsible for sequencing and error mapping. Strength: no new batch shape, smaller step, RAII removes the manual settle on early returns (`mcp.rs:~232`). Weakness: batch sequencing, cancellation and timeout stay duplicated in `mcp.rs`; error mapping stays per transport, so one of the three duplicated concerns survives.

**Comparison.** B is a safe first move (it deletes `session_room_check` vs `gate` duplication and the manual reserve/settle), A is the end state. Recommended: build B's `Admission` as the first deliverable inside A's module, then fold sequencing and report building into `submit` when the REST/MCP surface is reworked for agents. Error mapping to MCP and WS become two small formatters over `Rejection`.

**Leverage and locality.** One place for tenant and quota rules (decisions 10, 11), one place to define agent-facing error shape (concept principle: structured errors), testable without a socket.

**Risks.** Regression in the 110 server tests that encode current behaviour; WS performance on the hot path; subtle semantic differences between WS (single op) and MCP (prefix-applied batch) that tests encode and that need a deliberate decision, not a silent unification.

### Candidate 3. One WASM client

**Problem.** `Doc` (22 pub methods) and `Client` (26) both wrap `Document`/`Replica` plus `Layouter`; JS picks between them in `core-wasm.js:3-20` and `live.js:4`. Local mode, live read-only mode and connected mode each have their own transport (`live.js`, `online.js`).

**Proposed interface.** `Client::new(actor, measure)` with `connect-less` operation (local apply without an outbox, or an outbox that is simply never drained). One class, one transport (online). Live read-only becomes a role of the same transport.

**Benefit.** Removes about 22 delegating methods and a duplicate reconnect/replay path; one place for layout cache behaviour.

**Risks.** Local autosave and offline mode in `doc.svelte.js` depend on `Doc`; the product decision is "web is purely visual, collaboration via server", so local-only mode may not be needed at all (decision 8). Verify before folding.

### Candidate 4. Persistence port for the room

**Problem.** `room.rs` mixes CAS logic and SQL (15 `sqlx::query` sites); server tests need Postgres.

**Proposed interface.** `OpLog` with about 5 operations (append-with-CAS, find-by-key, read-since, load-snapshot, save-snapshot) and a notification stream. Adapters: Postgres (production) and in-memory (tests, and potentially desktop).

**Benefit.** Room invariants (idempotency, CAS, `ifSeq`, `deps`) become testable in milliseconds; two adapters make the seam real.

**Risks.** The room relies on Postgres semantics (CTE with `update ... returning` for the CAS at `room.rs:~585-605`, LISTEN/NOTIFY at `room.rs:835`) that an in-memory adapter must reproduce faithfully; do not build until the test-speed pain is felt, or until a desktop phase needs a second store.

### Candidate 5. Asset storage port

Decision 12 asks for local disk now and object storage later; `assets.rs` is Postgres-only (5 query sites, `:23` sniffing and ticket logic are separable). A small `AssetBlobs { put(id, bytes), get(id) }` port with disk and Postgres adapters is the second adapter that makes the seam real; add it when the seed implements decision 12.

### Candidate 6. Capture seam

Keep the relay and its protocol as they are. Define a `Capturer` trait only when decision 18 yields a second adapter (native renderer or headless Chromium). Until then the relay's interface (`CaptureHub::capture`) is already deep. Note the relay's fidelity limit: it renders the canvas, not the HTML export, so it verifies Pixi, not the exported artifact; this reinforces candidate 1.

### Candidate 7. Housekeeping

Rename spike-id files (`s8.rs` -> behaviour names), translate agent-visible strings and docs, delete `layout.js` once Rust is the only oracle, and give the two `Limits`/`Document` types distinct names. Cheap, and it directly improves AI navigation.

## 6. Seed recommendation (no product code is written here)

### 6.1 Carry as is

- `crates/core` (`apply`, `document`, `replica`, `fracindex`, layout) including its tests (68 `#[test]` functions including the multiplayer and fractional property tests). Rename `spike-*` crates to `meldframe-*`; translate comments.
- `crates/protocol` and the `ts-rs` pipeline with `tools/check-protocol.sh`.
- `crates/core-wasm` build scripts (`tools/build-wasm*.sh`).
- The server's room actor (`room.rs`) logic and its tests, ingress guard (`ingress.rs`), quota module (`quota.rs`), capture hub (`capture.rs` core) and the capture client (`src/lib/capture.js`), because each is deep and measured (S6, S13, H1 docs).
- Docs `s12`, `s16`, `f1`, `f2`, `s13`, `h1` as rationale, translated to English into `docs/architecture` or ADRs.

### 6.2 Carry with refactor

- Server admission (`ws.rs`, `mcp.rs`, `session.rs::gate`, parts of `assets.rs`, `capture.rs`): refactor into candidate 2 before adding Gatehouse and workspace tenancy.
- `crates/export` (`html`, `tailwind`, `style`, `measure`): keep as the first consumer of the render model (candidate 1); add `fontWeight` to `MeasureEntry`; drop `flutter.rs`.
- `crates/core-wasm`: merge `Doc` into `Client` if the product has no local-only mode (candidate 3).
- `AssetStore`: add a blob port when decision 12 is implemented (candidate 5).
- `apps/server` sessions: keep the semantics (binding, revocation, modes) but replace static `TokenInfo` tokens (`config.rs:8`) with the Gatehouse principal (decision 10).
- Spike docs and tests: rename away from spike ids.

### 6.3 Rewrite

- `Canvas.svelte` (787 lines) as a consumer of the render model, split by concern (scene, selection, tools, capture hook); `doc.svelte.js` modes (local/live/online) reduced to one connected mode plus an offline-free viewer.
- The model round 2 itself (tokens, margin, per-side padding, absolute children, gradients, opacity, no-fill, components, schema version): `Node` has no schema version field and no component/instance concept (`protocol/src/lib.rs:223-296`; a grep for `component` and `schema_version` found nothing). Treat as new design (decision 19), not as an extension of the spike's flat model.
- Workspace/tenant model (decision 11): rooms are free-form strings matched by glob (`TokenInfo::allows`, `config.rs:15`); there is no workspace table.
- Anything on Flutter and PPTX: not carried (`crates/pptx`, `crates/export/src/flutter.rs`, `flutter_check/`, `docs/s11-pptx.md`). Keep only as reference in the spike repository.
- ACP/agent panel (`src/lib/AgentPanel.svelte`, `agent.svelte.js`, `tools/acp-e2e`, `docs/s14-acp.md`): web has no agent runtime (decision 5); leave for the desktop phase.

### 6.4 Suggested workspace layout (superseded by `docs/architecture.md` section 3b)

```
crates/
  meldframe-protocol/     wire + model types (ts-rs source of truth)
  meldframe-core/         apply, document, replica, fracindex, layout, resolve (render model)
  meldframe-core-wasm/    wasm-bindgen glue (one Client)
  meldframe-export/       HTML, Tailwind, CSS variables (consumes resolved model)
apps/
  server/                 axum, MCP (rmcp), REST, WS; internal modules: admission, room, sessions, capture, assets
  web/                    Svelte 5 + PixiJS client
packages/
  protocol/               generated TypeScript types
tools/                    wasm build, protocol check, metric recorder
docs/                     architecture, decisions, ADRs, fidelity contract
```

Notes: keep crates few (the survey found no benefit from more than one `core`); `export` stays separate from `core` because the product exports in the server and the CLI/tests, but depends only on the `resolve` interface. Put server internals as modules, not crates, until a second binary needs them.

### 6.5 Map to decisions 10..19

| Decision | State in the spike | Seed action |
|---|---|---|
| 10 Gatehouse auth (OIDC/OAuth, MCP OAuth) | Static tokens (`config.rs:8`) and `wsess_` session tokens (`session.rs:34`); both resolve into `TokenInfo`/`Principal` at `session.rs:478-498` | Add Gatehouse as another principal source behind `resolve`; do it after candidate 2 so policy is in one place. |
| 11 Multi-tenant, workspace quotas | Quotas per actor, in memory, not shared across instances (`quota.rs:1-3`); no workspace concept | New workspace entity; quota keyed by workspace; durable or shared counters (rewrite of the storage of `quota.rs`, keep the sliding-window logic). |
| 12 Assets on local disk | Postgres rows (`assets.rs`) | Blob port with disk adapter first (candidate 5). |
| 13 Pricing/hosting later | none | none |
| 14 Rust core, Svelte 5 + PixiJS v8, axum + sqlx + Postgres, rmcp | Confirmed by the spike | Carry as in 6.1/6.2. |
| 15 Fidelity contract and oracle | Oracle numbers from S9 pre-date M3A; export lacks M3A properties (4.2) | Candidate 1 first; make the "every `Node` field is either rendered in both or marked unsupported" coverage test a CI gate; re-run the Chromium oracle on the seed model. |
| 16 Human and agent collaboration UX | Per-actor selective undo (`replica.rs`), `ifSeq` (`room.rs:68-76`), `deps` (F2), actor naming `mcp:<user>` and `<user>/<label>` | Build UX on these; the interface already carries attribution (`ServerMsg::Event.actor`). Not surveyed beyond that. |
| 17 Tokens and variables | Colors are hex literals validated in core (`apply.rs:53`, `:116`); no variable or mode concept | Token resolution belongs in `resolve` (candidate 1); validation rules change from "must be hex" to "hex or token reference". |
| 18 Headless capture | Relay only (S13: server 2.4-3.1 ms CPU per capture, vs 867 MB RSS for headless Chromium in the spike's measurement; latency measured on a busy 4 vCPU machine) | Keep relay; do the cost comparison with candidate 1 in place (a headless adapter can then capture the exported HTML, which relay cannot). |
| 19 Document model (versioning, components, variants) | Flat nodes, no schema version, no components (4.2 and 6.3) | New design; add `schema_version` to documents and snapshots (`migrations/0001_init.sql` has none) before the first release. |

## 7. What was not checked, and what is only a guess

Not done:
- No test, benchmark, build or `cargo`/`npm` command was run. All numbers are file/function sizes, `grep` counts and `git show --stat` output, or figures quoted from the spike's own docs (S9, S13) which I did not reproduce.
- I did not read every file. Read in full or in the relevant parts: `crates/core/src/{lib,apply}.rs` (apply, check and exec parts), `crates/protocol/src/lib.rs` (command and message types), `crates/export/src/{lib,measure}.rs` and parts of `style.rs`, `crates/core-wasm/src/{lib,client,layout}.rs` (signatures and measurer), `apps/server/src/{lib,state,http,ws (`handle_msg`, `forward`),mcp,room (handle_op, handle handles),session (gate, whoami),capture (hub and tool)}.rs`, `src/lib/{capture,core-wasm,live}.js` and the top of `doc.svelte.js`, `docs/s9`, `docs/s13` (first parts). Skimmed by `grep` only: `assets.rs`, `quota.rs` (interface), `ingress.rs`, `batch.rs`, `config.rs`, `document.rs`/`replica.rs` (signatures), `net.js`, `online.js`, `Canvas.svelte` (only the M3A parts), `Props.svelte`, all other docs, `e2e/`, `tools/`, `eval/`, `spikes/`, `flutter_check/`.
- `crates/pptx`, `crates/export/src/flutter.rs`, ACP (`docs/s14-acp.md`), the agent panel and Flutter are out of scope and were not assessed.
- The worktree `.claude/worktrees/f3` was not opened.

Claims that are inference, not verified:
- "Export has no border/shadow/textAlign output" is based on `grep` of `style.rs` for `box-shadow`, `border` and `font-weight`; I did not run the exporter on a document with those properties. The M3A merge diff touching no file under `crates/export` is verified; whether a later commit on the branch added export support is not (I only looked at `e0da632` and the current tree).
- "The S9 oracle has not been re-run since M3A" is an inference from the doc (`docs/s9-design-code.md` predates M3A in the history) and from the diff; there may be a later run I did not find.
- The depth ratings are judgments; the item counts are measured but "interface items" are not weighted by complexity.
- The three-times duplication in 4.1 is established from reading `ws.rs:341-415`, `mcp.rs:167-318` and `session.rs:734-787`; the exact behavioural differences between WS and MCP (for example which checks run in which order) were not tested. The line numbers marked with `~` are approximate.
- The WS hot-path cost of a batch-shaped `submit` (candidate 2, design A), the canvas cost of consuming a resolved model across WASM (candidate 1, design A) and the feasibility of an in-memory `OpLog` that matches Postgres CAS semantics (candidate 4) are untested hypotheses.
- Whether the product needs a local-only (no server) mode, which determines candidate 3, is a product question (decision 8 suggests no), not a code finding.
- Duplication between `live.js` and `online.js` is suspected from their purposes and sizes; I did not diff them.
- Multi-instance behaviour (LISTEN/NOTIFY, CAS retries) was read, not exercised.

## 8. Alignment with decisions made after this survey (13 Nov 2026)

| Survey item | Outcome |
|---|---|
| Candidate 1, shared render model | Kept. It is the `meldframe-core` resolve step with two consumers, canvas and export (ADR-0003, `docs/architecture.md` section 4). |
| Candidate 2, op submission pipeline | Strengthened into the single admission gate with a permit, risk tiers and agent pairing (ADR-0010). The room accepts a write only through the gate. |
| Candidate 3, one WASM client | Reframed. The web client renders a UI-neutral `meldframe-app` layer compiled to WASM (ADR-0008). A local-only mode is not needed: the web product is connected and purely visual. |
| Candidates 4 and 5, persistence and asset ports | Unchanged: add a port only when a second adapter exists. The asset blob port arrives with the local-disk adapter first. |
| Candidate 6, capture seam | Unchanged. Start with the browser relay; decide on a second renderer in the repository when a real agent workflow needs it. No separate spike (decision 24). |
| Candidate 7, housekeeping | Unchanged; done while seeding the repository. |
| Section 6.4, layout "keep crates few" | Superseded. Features are crates with declared kinds, enforced by CI so that a feature can be left out of a build (ADR-0007, `docs/architecture.md` section 3b). Server internals (admission, room, sessions) stay modules inside the server crates until a second binary needs them. |
| Not in this survey | Live events split into durable and ephemeral frames (ADR-0009); the spike client keeps its rules in Svelte, which the app layer replaces. |
