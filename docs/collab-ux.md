# Human and agent collaboration UX

Status: **draft for owner review** (13 Nov 2026). Everything marked `[A]` is an assumption, everything marked `[Q]` is an open question, everything marked `[P]` is a proposed decision. The owner has said this needs adjusting; this document is written so the adjustments are easy to record (see the register at the end). Related decisions: `decisions.md` #5, #8, #10, #11, #16.

This is a design document. It describes behavior that is **already built in the spike core/server** (and says so, with a pointer), behavior that is **proposed but not built**, and the **gaps**. Nothing here is product code.

## 1. Scope

Decision 8: no direct human-to-human collaboration at the start (no presence, comments, multi-editor). The web client is a manual canvas; agents arrive through MCP; the UI is updated over WebSocket for both agent and user changes.

So "collaboration" in this document means:

- one human (the workspace member) editing manually in one or more tabs, and
- one or more external agents (Claude Code, Cursor, Codex, ...) editing through MCP in the same file at the same time.

Out of scope: presence between humans, comments, multi-editor permissions, in-app chat and agent runtime (desktop, phase D, over ACP). Where the desktop phase changes an answer, it is called out as a note, not designed here.

Terminology: **actor** = the identity the server stamps on an op (cannot be forged by the client). **Op** = one `Command` applied at one `seq`. **Batch** = the commands of one `apply_commands` call. **Run** = a contiguous stretch of work by one session actor [A: defined by an idle gap, see 5.1].

## 2. What the core and server already give us

The collaboration rules below are not new inventions. They are the spike behavior, summarized here with the UX consequence. Source docs are in the spike repo `wonder-proto`, branch `spike/rust-core`, directory `docs/`.

| Capability | What it does (measured in the spike) | UX consequence | Source |
|---|---|---|---|
| Total order | One `seq` per accepted op per document. Winner of a conflict = highest seq. Patches merge per field; one patch is atomic (no torn `{x,y}`). | The canvas can never show a half-applied patch. Conflicts have one deterministic winner, so "who won" is always answerable from the log. | `s8-multi-agent.md` |
| Server-stamped actor | `actor` on every event is set by the server; a client-supplied `actor` is rejected. Session actors look like `<user>/<label>`, user-token MCP actors like `mcp:<user>`. | Attribution can be trusted. An agent cannot pose as the user. | `s8-multi-agent.md`, `s15-session-binding.md` |
| Per-actor selective undo | Undo reverts only the actor's own steps, per field; fields written later by someone else are left alone; a fully overwritten step is skipped. Undo then redo does not change the document. Undo/redo are ordinary ops. | "Undo" never destroys the agent's later work or the user's. A user undo only affects the user's own steps. | `s12-multiplayer-undo.md` |
| Concurrency window | An undo sent before the client saw a concurrent write can still overwrite it (516 of 16,236 checked fields in the property harness). | Rare, real. The UI must make an overwrite visible after the fact (4.4). | `s12-multiplayer-undo.md` (anomaly 1) |
| `ifSeq` (compare-and-set) | Op rejected with `conflict` if a node it relies on was written by another op with `seq > ifSeq`. Node granularity, not field. Adding a sibling does not conflict; removing a subtree conflicts if anything inside changed. Own batch does not conflict with itself. `stale` if `ifSeq` is older than the retained trace. | The protection mechanism for "do not clobber the user's edit". Today agents do not use it (V1: no real agent used `ifSeq`, idempotency keys or `atomic`). | `s8-multi-agent.md`, `v1-agent-design-eval.md` |
| Idempotency keys | Per actor. Resend of an applied key is answered as duplicate. Same key with a different command is rejected. Batch commands use key `<key>#<i>`. | Retries after a dropped connection never double-apply. Batch identity = key prefix, usable for grouping in the UI. | `s8-multi-agent.md`, `h1-hardening.md` |
| Atomic batch and `batch_status` | `atomic: true` validates the whole batch against a copy of the state and commits in one transaction (all or nothing, 500 commands about 35 ms). Default is `false`. `batch_status {idempotency_key}` returns `state` (`running`, `completed`, `cancelled`, `rejected`, `aborted`, `unknown`), `total`, `applied`. Cancel of a non-atomic batch stops it between commands. | A "batch" is a real unit the user can accept or revert. Non-atomic batches can be partially applied (a prefix); the UI must show "partial". | `h1-hardening.md` |
| Op dependencies | `deps` on an op (set for undo/redo) makes the server reject it with `dependency_rejected` when a premise op was rejected. | The user can occasionally see an undo "not take" because an earlier op of theirs was rejected. Text in 7. | `f2-op-dependencies.md` |
| Sessions and binding | A session token is bound to user + chat, mode `read|write`, expiry, revocation; the UI (not the agent) chooses the active file; calls to another room fail with `room_mismatch`. | One agent session = one actor = one lane in the activity UI. The user, not the agent, decides which file an agent writes to. | `s15-session-binding.md` |
| Quotas per actor | `callsPerMin`, `opsPerMin`, `maxConcurrent`, `maxBatch` (100 default), `maxPayloadBytes`, `nodesPerWindow`, `callTimeoutMs`; errors carry `retryable` and `retryAfterMs`. Reservation is atomic (batch over the limit is refused whole). | A runaway agent is bounded. The user can see remaining quota and why an agent stalled. Isolation under load was NOT achieved (p50 of a victim rose about 3x). | `s15-session-binding.md` |
| Structured errors | `code` in `invalid`, `conflict`, `stale`, `internal`, `dependency_rejected`, `quota_exceeded`, `no_file_selected`, `room_mismatch`, `read_only`, `forbidden`, `call_timeout`, `session_revoked`, `session_expired`, `binding_changed`; MCP also returned `room_required` for `capture`. | Every rejected agent op has a machine reason; the activity panel can show it in plain words. | `s15`, `h1`, `v1` |

What the spike does **not** provide (these are the gaps this document designs around):

1. There is no MCP tool for an agent or the user to **undo an agent's work**. Selective undo lives in the client `Replica` and only covers the replica's own actor.
2. The server keeps `writes`/`kids` traces per op (max 50,000, node granularity), but there is no API that answers "who last wrote this field" or "list the batches of the last hour".
3. There is no notion of a **run**, **summary of intent**, **review state**, or **checkpoint**.
4. There is no way to **pause** an agent without revoking its session (revocation also stops SSE only on the next request).
5. Two tabs of the same actor appear as foreign ops to each other (S12 anomaly 7): safe, but histories are separate.
6. Server messages in the spike are in Indonesian. The product UI and tool descriptions must be English (decision 4, and the V1 recommendation).

## 3. Mental model for the user

Three sentences the UI must make true:

1. **Agent changes apply immediately and are always labeled.** There is no hidden draft; what is on the canvas is the document. [P]
2. **Anything an agent did can be reviewed and reverted without touching anyone else's work**: per step, per batch, per run, or back to a checkpoint. [P]
3. **Your own undo is yours.** Cmd/Ctrl+Z undoes your last step only, even if an agent worked after it. Reverting agent work is a separate, explicit action. [P]

Alternative considered and parked: **staged mode** (agent writes to a proposal that is applied only on Accept, as in code-review tools). It matches "review first" but needs document branches (open decision 19) and breaks the live-feedback loop agents rely on (`capture` shows the applied state). Recommended later as an optional per-session "propose only" mode (`[Q1]`).

## 4. Scenarios

Each scenario says what already happens, what the user sees, and what is missing.

### 4.1 The agent edits a node the user is editing

What happens today:
- Different fields merge cleanly (agent changes `fill`, user drags `x,y`).
- Same field: higher `seq` wins; the loser's value is overwritten. The user's local view keeps their pending ops on top of server state, so the node does not flicker mid-drag; after the drag commits, the agent's concurrent write to `x,y` is superseded.
- If the agent sent `ifSeq` from its last `get_document`, an agent `updateNode` on a node the user touched since is rejected `conflict` naming node, actor, seq; the agent should re-read and retry.
- `removeNode` by an agent is rejected `conflict` if anything in the subtree changed after its `ifSeq` (it would erase the user's work).

What the user sees (proposed):
- A short colored **touch ring** on the node (agent color) when an agent op lands, and an entry in the activity panel. If the user is currently dragging or typing in that node, the ring appears but the node does not jump; a small chip "Agent also changed: fill" appears in the inspector.
- If the agent's write overwrote a field the user wrote within the last N seconds, or the user's op overwrote the agent's: the activity row gets a warning mark "overwrote your change to fill" with a one-click "Restore my value" (a revert of that single step, see 4.4). [A: N = 30 s]

Gaps and proposals:
- **Implicit `ifSeq` [Q2]**: the MCP adapter could stamp `ifSeq` = the seq of the session's last `get_document` on every mutating call that targets an existing node (`updateNode`, `reparent`, `removeNode`), opt-out per call. This protects the user's edits by default. Cost: agents that never re-read hit `conflict` and spend a turn. Recommendation [P]: default ON for `removeNode` and `reparent`, default ON for `updateNode` only when the node was changed by a non-agent actor since the session's last read (needs the trace the server already keeps). Needs a measurement with real agents; V1 shows they do not use these fields by themselves.
- **Soft focus hint**: when the user has a node selected or is mid-gesture, expose it to agents through `get_document`/`whoami` as `userFocus: [nodeIds]` (read-only hint, not a lock). Agents that honor it avoid the node. No locking in v1 [P]: locks block agents and need release semantics.

### 4.2 The agent works for a long time

Facts: a call has `callTimeoutMs` (default 30 s); a batch is up to `maxBatch` (100 default) commands; the agent works as a series of calls (V1: 4 to 7 tool calls, 14 to 66 s per design). Wonder reports generations up to 30 minutes and timeouts leave unknown partial state (research doc `research-wonder-deep-pricing-mcp-acp-nov-2026`, primary sources `[P]` in that doc).

What the user sees (proposed):
- A **live run row** at the top of the activity panel: agent name (session label), a state (`working`, `waiting`, `idle`, `stopped`, `error`), elapsed time, ops applied so far, last batch summary. State is derived from the event stream and session metadata: `working` while ops arrived in the last 10 s [A], `idle` after 60 s [A].
- Canvas updates are **per batch, not per token** (the granularity of an MCP call). To make progress feel alive the agent guidance (system prompt/skill we ship) says: send small batches, top-down, section by section.
- A **Stop** button on the run row. Two possible semantics: (a) revoke the session (exists: `DELETE /sessions/{id}`; later calls get `session_revoked`; an open SSE stream is only refused on the next request, and an in-flight non-atomic batch is cancelled by the MCP cancel path, not by revocation), (b) **pause** (new: the session stays valid, writes return `paused` with `retryable: true` and a `retryAfterMs` hint, user can Resume). Recommendation [P]: ship (a) first (exists), add (b) when users ask. `[Q3]`
- `batch_status` gives the truth after a stall: on timeout the UI shows "Batch 2 of 3: 41 of 100 steps applied" instead of "unknown". This directly answers a known Wonder gap (timeouts leave unknown partial state).

### 4.3 Several agents at once

Facts: 24 agents x 150 calls measured at 1,720 to 1,890 op/s, ack p99 20 to 23 ms; distinct actors; FIFO queue shared; quotas are per actor but do not fully isolate latency (S15).

What the user sees (proposed):
- One **lane per actor** in the activity panel and a stable color per actor in layers/canvas. Lanes are collapsed to a header (name, state, counters) with the most active expanded.
- Soft ownership by **recent touch**: nodes touched by actor X in the last 10 s carry X's ring. No explicit claims in v1 [P]. If agents collide (both rejecting with `conflict` repeatedly), the panel shows a banner "Agent A and Agent B are changing the same nodes" with "Pause B" [needs 4.2 pause, `[Q3]`].
- Page-level hint for agents: the user can bind each chat session to a different **frame or page** as a work area (S15 binds to a file; narrowing to a frame is new) `[Q4]`. This is the cheapest way to stop agents from colliding: partition by frame.

### 4.4 The user reverts agent work

Three granularities, all built on the same inverse mechanism:

| Unit | Definition | How it is identified in the log |
|---|---|---|
| Step | one op (one command) | `seq` |
| Batch | one `apply_commands` call | keys with the same prefix `<key>#<i>` (and `batch_status` for state) |
| Run | all batches of one session actor between two idle gaps or between two checkpoints | actor + time/seq range |

Mechanism [P]: **revert = new ops authored by the user**, computed with the S12 rules applied to the *agent's* ops: for each field the agent wrote, restore the pre-value **only if the agent's op is still the last writer of that field**. Fields written later by anyone else are kept and reported ("kept 3 later edits by you"). Structural rules from S12 apply (deleted-by-others nodes are not resurrected inside deleted parents, restored nodes go to the nearest surviving parent). A fully overwritten step is a no-op and says so.

Why author as the user and not delete the agent's ops: the log stays append-only, the revert is itself undoable by the user's own Cmd/Ctrl+Z (it is the user's step), and attribution stays truthful ("reverted by you").

Not built: the spike has this logic only inside `Replica` for own-actor history. Needed: a core function `revert(ops) -> (commands, skipped)` that works on a seq range of any actor, run server-side (needs the log) or client-side after fetching the range (needs `GET changes`, section 8). Property tests from S12 (convergence, "does not overwrite later writes", undo-then-redo identity) extend naturally. `[Q5]`: should revert be one atomic batch (all-or-nothing)? Recommendation [P]: atomic, since a partial revert is confusing.

User-visible outcome of a revert: a toast "Reverted 14 steps from Claude Code / chat1. Kept 2 later edits." with "Undo".

### 4.5 Other scenarios worth handling

| Scenario | What happens | UX |
|---|---|---|
| Agent adds a child to a node the user just deleted | `invalid` ("parent does not exist"); nothing written | Activity row: failed, reason; agent is told and retries elsewhere |
| User deletes a subtree while an agent is building in it | Agent ops into it fail `invalid`; user delete wins by order | Row "3 steps could not be applied: the frame was deleted" |
| User edit rejected (e.g. validation) | user op `invalid` | inline error at the field |
| Agent session expires or is revoked mid-run | `session_expired` / `session_revoked` | run row turns `stopped`; ops already applied stay |
| User switches the active file during a run | calls already running finish in the old file; new calls get `no_file_selected` or go to the new binding (S15 rule 1) | run row shows "still writing to <old file>" until it finishes (this is the S15 UX proposal) |
| Same user in two tabs | each tab is the same actor but separate replica history (S12 anomaly 7) | v1: warn on second tab "Editing in another tab: undo history is per tab" [P] |
| Agent offline/crashed mid-batch | non-atomic: prefix applied; atomic: nothing | state from `batch_status` |
| Agent changes tokens that nodes use | bound nodes change appearance everywhere | the activity summary counts "affects 42 nodes" (see `tokens-and-variables.md`) |
| Quota exhausted | `quota_exceeded` + `retryAfterMs` | run row `waiting`, countdown |

## 5. What the user sees

### 5.1 Activity panel (agent activity in real time)

Docked at the bottom of the left sidebar or as a collapsible right-hand tab [Q6]. Built only from data that exists or is proposed in section 8. Wireframe (text):

```
+--------------------------------------------------------------+
| Activity                              [Filter v] [Follow: off]|
+--------------------------------------------------------------+
| (o) Claude Code / chat1        working   02:14   58 steps    |
|     "Hero: tighten spacing, bind brand colors"      [Stop]   |
|   +- Batch 3  hero, 12 steps          applied     10:42:31   |
|   |    ~ Hero (padding, gap)   ~ Hero Title (font size)      |
|   |                         [Show] [Revert] [Accept]         |
|   +- Batch 2  pricing cards, 30 steps partial 18/30  10:42:10|
|   |    ! stopped: call timeout (retry sent)                  |
|   +- Batch 1  page skeleton, 16 steps     applied   10:41:55 |
|                                              [Revert run]    |
+--------------------------------------------------------------+
| (o) Cursor / chat2             idle      00:41    9 steps    |
|     collapsed                                                |
+--------------------------------------------------------------+
| You                                                           |
|   10:42:40  Moved Pricing Card (x, y)                         |
|   10:42:20  Changed fill on Hero      ! overwrote Claude's    |
+--------------------------------------------------------------+
| Needs review: 2 batches                [Accept all] [Review]  |
+--------------------------------------------------------------+
```

Rules:
- Entries are **grouped by batch**, newest first, with a one-line summary: counts of added / changed / removed nodes and the first few node names (layer names are semantic where possible, which is why the layer-name lint exists, see concept section 12).
- A batch can carry an **agent-provided note** (an optional `note` argument on `apply_commands`, stored with the batch). Without it the UI generates the summary from the commands. [P]
- Rows link to the nodes: **Show** selects and frames them on the canvas; hover highlights them.
- Counters and state are derived from events; no polling.
- Rendering is throttled: events are applied to the canvas immediately but activity rows are coalesced per batch every 250 ms [A] (agent bursts reach about 1,700 op/s).

### 5.2 Attribution in layers and history

Layers panel: a small colored dot per node showing the **last writer** (hover: "Last changed by Claude Code / chat1, 2 min ago"); a filter "Show only nodes changed by an agent" (and by a specific agent). Nodes added by an agent in the current run carry a "new" badge until accepted.

```
Layers
  v Landing Page Optimized V2
    v Hero                     (o) Claude  2m
        Hero Title             (o) Claude  2m
        Hero CTA               (*) You     5m
    v Pricing
        Pricing Card           (o) Cursor  just now  [new]
```

Inspector: next to a field value, a subtle marker when the last writer is not the user: "fill: set by Claude Code / chat1 (batch 3)", with "Revert this field" (a step-level revert of one field).

Needed data: last writer **per node** is enough for the dots; per field for the inspector. The client can track it for events seen since it connected; persistent attribution needs a server projection (section 8).

### 5.3 Canvas cursor and highlight

- Agents have no pointer; show a **touch ring** (2 px stroke, actor color, 1.5 s fade) on every node an op touched, plus a persistent ring while the actor's state is `working` on the most recent batch's top-level nodes. [A]
- **Follow agent** (off by default): the viewport pans to the latest touched area. Off by default because a jumping viewport while the user edits is worse than missing a change. The "Show" button in the panel does one-shot framing. `[Q7]`
- Do not draw agent "cursors" that imitate human presence (decision 8 excludes presence): the activity is object-centric, not pointer-centric.
- Reduced motion preference: no animation, static outline for 2 s.

### 5.4 Review mode: accept or revert, per batch or per step

Because changes are applied live (3), review is **post-hoc**:

- Every agent batch starts as **needs review** (unreviewed). Accepting only clears the marker (metadata, no document change). Reverting applies the revert described in 4.4.
- Controls: per step (expand a batch), per batch, per run, **Accept all**, and a bulk bar at the top of the panel when unreviewed batches exist.
- **Review view**: toggle "Highlight unreviewed changes" tints changed nodes by actor on the canvas and, optionally, shows a **before/after** (split or overlay) for the selected batch by rendering the document at `seq_before` and `seq_after` (replay of snapshot plus ops, which the server already can do). `[Q8]`: before/after compare is a strong feature but needs a render-at-seq path; may be deferred.
- Review state is stored per file (not per user, since there is one human by decision 8) [A]; with multiple workspace members later it becomes per user or per file with "reviewed by".
- Unreviewed batches never block anything: export, further agent work and edits all proceed. A file-level counter shows "2 unreviewed batches" in the header; export dialogs mention it [P].

### 5.5 Checkpoints

- **Auto checkpoints** [P]: at the start of each run (first op of a session actor after an idle gap), before a revert, and before restoring a checkpoint. A checkpoint is a `seq` marker plus a label; the log and periodic snapshots already allow rebuilding the state at that seq.
- **Manual checkpoints**: "Save checkpoint" (Cmd/Ctrl+Shift+S, label).
- **Restore checkpoint** = compute the difference between the state at the checkpoint and the current state, apply it as new ops by the user in one atomic batch (nothing is rewritten in the log). It undoes everything since by all actors, so it always asks for confirmation and creates an auto checkpoint first. Needs a core `diff(state_a, state_b) -> commands` (new).
- Alternative worth comparing: Wonder's approach of duplicating the frame before editing (research doc: "menduplikasi sebelum mengedit"; concept section 12: side-by-side variants). Variants and checkpoints are complementary: variants for exploration, checkpoints for safety. Agent guidance can ask the agent to duplicate a frame before a large rewrite. `[Q9]`
- v0 and Figma point of comparison: v0 creates a version per message and restoring appends a new latest version rather than rewriting history (search snippets, accessed 13 Nov 2026, third-party mirrors, unverified); Figma's version history applies to agent edits per a third-party directory claim (unverified). Our proposal is consistent with both: append-only restore.

## 6. Conflict rules (user-facing contract)

Written so they can go into help text and into the agent guidance.

1. The last accepted write to a **field** wins; fields merge independently; one patch is applied whole.
2. Structure is validated at apply time: deleting a parent removes its subtree; adding into a deleted parent fails; moves that would create a cycle fail.
3. Your undo only reverts your own steps, and only the fields nobody changed afterwards.
4. Agents can protect your work with `ifSeq`; if they do not, you can revert them (4.4).
5. A batch is atomic only when the agent asks for it (`atomic: true`). Otherwise a prefix can be applied; the panel shows how much.
6. Re-sending a request never duplicates it.
7. An agent can only write to the file you selected for its chat, within its quota, for as long as its session lives.

Open: should the product **require** `atomic: true` for batches above N commands, or default it on? Atomic blocks the room actor for the duration (about 35 ms per 500 commands in the spike) and is all-or-nothing, which suits review. Recommendation [P]: default on for session tokens, per-call opt-out, to be validated with real agents (`[Q10]`).

## 7. Errors the user sees

The server returns a `code`. The UI maps it to plain English. Texts below are proposals (English, no jargon, say what happened and what is safe).

| Code | Who sees it | Where | Text (proposed) |
|---|---|---|---|
| `conflict` | agent (user sees in activity) | activity row, grey | "Skipped: you changed Hero after the agent last looked. The agent will re-read and retry." |
| `stale` | agent | activity row | "Skipped: the agent's view was too old. It is reloading the file." |
| `invalid` | agent; user for own ops | activity row; inline for own edits | Reason from the server, translated: "The parent frame no longer exists." / "That would put a frame inside itself." / "That id is already used." |
| `dependency_rejected` | user | toast on undo/redo | "Couldn't undo that: an earlier change of yours was rejected. Your document is unchanged." |
| `quota_exceeded` (retryable) | agent; user sees state | run row `waiting` + countdown | "Agent paused by its rate limit. Resumes in 12 s." |
| `quota_exceeded` (not retryable) | user | run row `stopped` + link | "This request is larger than the agent's limit (max 100 steps per call). Ask it to send smaller batches or raise the limit in session settings." |
| `call_timeout` | user | run row | "A request timed out after 30 s. 41 of 100 steps were applied. The agent will resend; nothing is duplicated." |
| `no_file_selected` | user (composer) | banner | "No file selected. Pick a file to let this agent edit." (same text the agent receives) |
| `room_mismatch` | user | activity row | "Blocked: the agent tried to edit <other file>, but this chat is bound to <this file>." |
| `read_only` | user | run row | "This session is read-only. Agents can look but not edit." |
| `forbidden` | user | banner | "You no longer have access to this file, so the agent was stopped." |
| `session_revoked` | user | run row `stopped` | "Session stopped." (after Stop) |
| `session_expired` | user | run row `stopped` | "Session expired. Start a new one to continue." |
| `binding_changed` | client (internal) | none; resubscribe | Banner only if resubscribe fails: "Reconnecting to the file ..." |
| `internal` | user | toast + retry | "Something went wrong on our side. Your work is safe; retrying." |
| `room_required` (spike `capture` friction) | agent | n/a | should not exist: `capture` must use the session binding (V1 recommendation 1) |

Rules for wording: never blame the agent or the user; always say whether the document changed; always say what happens next. The server must send stable codes plus an English message (the spike's messages are Indonesian: an i18n/cleanup item).

Connection state: if the WebSocket drops, the canvas keeps accepting user edits as pending ops (S12 `Replica`), shows "Offline: changes will sync" and resends unconfirmed ops with their idempotency keys on reconnect (verified in S12 integration tests). Agent activity pauses visibly ("Reconnecting...").

## 8. What the protocol and product need (new work)

Grouped by priority. "Protocol" = Rust protocol crate (additive, regenerate TS types, run the protocol check); "Server" = endpoint or projection; "Core" = pure function in the shared Rust core; "Client" = UI only.

| # | Item | Kind | Why | Priority |
|---|---|---|---|---|
| N1 | **Actor profile**: stable display name, color, kind (`user`, `agent`) per actor; session label from the chat/connector name | Server (`whoami`, session create, subscribe info) | attribution, lanes | P0 |
| N2 | **Batch grouping and note**: persist per batch: key prefix, actor, first/last seq, command count, optional agent `note` (new MCP argument), `atomic` flag | Server table or projection; MCP arg | activity rows, review | P0 |
| N3 | **Changes query**: `GET /rooms/{room}/changes?sinceSeq&actor&node&limit` returning compact ops with actor and batch id | Server | activity after reload, per-node attribution, revert input | P0 |
| N4 | **Last-writer projection** per node (and per field if cheap) | Server or client from N3 | layer dots, inspector markers | P1 |
| N5 | **`revert(ops)`** core function (S12 rules on any actor's seq range), returning commands and skipped list; exposed to the client; applied as one atomic batch | Core + client | 4.4 | P0 |
| N6 | **Revert is a local gesture**: the produced commands enter the user's `Replica` as one undoable step | Core (`Replica`) | undo of a revert | P1 |
| N7 | **Review state** per batch (`unreviewed`, `accepted`, `reverted`) | Server table + event | review UI | P1 |
| N8 | **Checkpoints**: markers (seq, label, kind auto/manual); `state_at(seq)` and `diff(a,b)` | Server + Core | 5.5 | P1 |
| N9 | **Pause/resume** of a session (`paused` error) | Server | 4.2, 4.3 | P2 |
| N10 | **Implicit `ifSeq`** for session actors (adapter default) and a `userFocus` hint in `get_document` | MCP adapter | 4.1 | P1, needs measurement |
| N11 | **Activity events** over WS: a `batchApplied`-style summary message so the UI need not infer batches from ops (additive `ServerMsg` variant; old strict clients would fail, which is acceptable for a single first-party client but must be versioned) | Protocol | cheaper UI | P2 |
| N12 | **Work-area binding**: bind a session to a page/frame within the file | Server (S15 extension) | 4.3 | P2 |
| N13 | **Render at seq** for before/after | Server/Core + capture | 5.4 | P2 |
| N14 | **English messages** and stable code list as a contract; remove `room_required` | Server/MCP | 7 | P0 |
| N15 | `describe_schema`/capability tool listing model limits (V1: agents silently work around missing features) | MCP | agent honesty | P1 |
| N16 | `design_review` tool and critique list integration (section 9) | MCP + Core | iteration loop | P1 |

Compatibility notes: all new `Command`-level fields are additive; `ServerMsg` is `deny_unknown_fields` tagged, so new variants need a client/server version handshake (not designed here). Anything stored per batch should be keyed by the same `idempotency_key` prefix the log already uses, so existing documents get grouping for free where keys exist (UI-originated ops have per-op keys; fall back to grouping by actor and time gap).

## 9. Critique list and `design_review`

Concept section 12: owner iterates with numbered critique lists (points 6, 9, 10, 11, 12 in the screenshot analysis), and a structured `design_review` MCP tool is an opportunity. Collaboration-UX angle:

- A **critique** is `{id, severity, rule, nodeIds, message, suggestion?, status}`. Sources: (1) automated checks run by the server (layer-name lint, contrast, spacing off the 4/8 grid, hard-coded colors that match a token, missing fills, frames with no children; V1 measured exactly these: contrast 9 of 57 failing in P3-bare, 39% to 72% default layer names in unguided runs), (2) items typed by the human, (3) items produced by an agent reviewing the design.
- Stable numbering per file (`#1`, `#2`, ...) so the human can say "do 6, 9 and 10" and the agent can answer "fixed 6 (batch 3), 9 (batch 3), skipped 10: needs a decision".
- **Link critiques to batches**: the optional `note` on a batch may carry `fixes: [6, 9]`. In the UI a critique row shows its fixing batches and status `open`, `addressed` (an agent batch claims it, needs review), `verified` (re-run of the automated check passes), `dismissed`.
- Panel (tab next to Activity):

```
+------------------------------------------------------+
| Review (12)           [Run checks] [Add note]        |
+------------------------------------------------------+
| #6  warn  Contrast 3.3:1 on "Subheading"   open      |
|           [Show] [Ask agent to fix] [Dismiss]        |
| #9  info  Layer "Frame 12" has a default name        |
|           suggestion: "Pricing Card"  [Apply]        |
| #10 warn  8 literals match token color/brand/500     |
|           [Bind all]                  addressed (b3) |
+------------------------------------------------------+
```

- "Ask agent to fix" has no agent runtime on web (decision 5). It produces a ready-to-paste prompt/copy-to-clipboard and, via MCP, the agent can call `design_review` to fetch open items itself. [P]
- Checks are deterministic server/core functions, so they are testable and the same on web and (later) desktop.

## 10. Proposed decisions vs open

**Proposed [P]** (to confirm)

1. P-1 Changes apply live and are labeled; review is post-hoc (accept/revert), not staged.
2. P-2 One session = one actor = one lane; actor profile (name, color, kind) is a server concept.
3. P-3 The unit of review and revert is the **batch** (key prefix), with step and run as finer/coarser views.
4. P-4 Revert = new ops authored by the user, computed with S12 field-level rules, atomic, itself undoable; never rewrites the log.
5. P-5 User undo stays own-actor only; revert of agent work is a separate explicit action.
6. P-6 Checkpoints are seq markers; restore appends a diff batch; auto checkpoint at run start and before revert/restore.
7. P-7 No locks. Soft hints only (`userFocus`, touch rings). `ifSeq` is the protection; default behavior to be measured.
8. P-8 Agent guidance (shipped skill/prompt) asks for small top-down batches, semantic layer names, `note` per batch, `atomic` for multi-command edits and `ifSeq` for edits of existing nodes.
9. P-9 Error codes are a stable contract with English messages; texts in section 7.
10. P-10 Follow-agent viewport is off by default; touch rings on; no human-presence-style cursors.

**Open [Q]**

- Q1 Add a "propose only" (staged) session mode later?
- Q2 Implicit `ifSeq` default for session actors: on, off, or partly (4.1)?
- Q3 Stop = revoke only, or add pause/resume?
- Q4 Bind a session to a frame/page, not just a file?
- Q5 Is a partial revert acceptable, or always atomic?
- Q6 Where does the activity panel live (left sidebar bottom, right tab, bottom drawer) given the Wonder-like right panel is already dense (Properties, Variables, Icons)? Proposed in `ui-design.md` section 4: the second tab of the right panel, next to Design.
- Q7 Should "Follow agent" exist in v1? Proposed in `ui-design.md`: a Preferences setting, off by default.
- Q8 Before/after compare in v1 or later?
- Q9 Ask agents to duplicate a frame before large rewrites (Wonder behavior)?
- Q10 `atomic` default on for session tokens?
- Q11 Review state: per file now, per user once workspaces have several editors?
- Q12 Retention: how long are per-batch metadata and checkpoints kept, and does it count against workspace quota?
- Q13 What is the exact definition of a run (idle gap, session boundary, user action)? [A: 60 s idle gap]
- Q14 Do we expose the agent's `note` verbatim (prompt-injection and noise risk) or only a sanitized summary? Treat it as untrusted text: render as plain text, length-limited, never as markup. [P]

## 11. Assumptions

- A1 One human editor per workspace file at the start; the same user may have several tabs.
- A2 Agents are external and use session tokens (S15) or user tokens; actor strings follow the spike (`<user>/<label>`, `mcp:<user>`, `user:<id>`); the final scheme depends on Gatehouse subject ids and workspace as tenant.
- A3 Time constants (10 s working, 60 s idle, 30 s overwrite window, 250 ms coalescing) are placeholders to tune.
- A4 The log (`ops`) and snapshots are kept long enough to support seq-range queries; retention is not yet decided.
- A5 `note`, run, review state and checkpoints are metadata, not document content, and are not part of `Command`.

## 12. Tests and oracles needed

- Property tests: extend the S12 harness with agent-actor batches; invariants: revert of a seq range never overwrites a later write; revert then redo-of-revert is identity; atomic revert is all-or-nothing; convergence of all replicas.
- Integration: two agents plus a user, scripted overlap on the same node; assert activity grouping equals `batch_status` and the log.
- Error-text table test: every server `code` has an English UI string and a test that no code is unmapped.
- UI e2e (Playwright): live run row updates, Stop, revert toast and undo of revert, checkpoint restore, throttling under a 1,000-op burst.
- Mutation checks as in the spike: disabling the "still last writer" check in `revert` must fail a test.

## 13. Sources

Spike (read-only, branch `spike/rust-core` of `wonder-proto`, read 13 Nov 2026): `docs/s8-multi-agent.md`, `s12-multiplayer-undo.md`, `f1-core-undo-anomalies.md`, `f2-op-dependencies.md`, `s15-session-binding.md`, `h1-hardening.md`, `v1-agent-design-eval.md`, `crates/protocol/src/lib.rs`. Project memory (Uteke room `project:meldframe`): `research-design-tool-competitors-nov-2026`, `research-wonder-deep-pricing-mcp-acp-nov-2026`, concept v1 section 12. Owner screenshot `20.png` (Wonder desktop, layers with semantic names).

Web (accessed 13 Nov 2026). The sandbox blocked direct page fetches (`help.figma.com`, `www.w3.org`, `tailwindcss.com`, `penpot.app` did not resolve), so these are **search-result summaries only, not read at the source**:

- Figma MCP server, `use_figma` writes through the Plugin API: https://developers.figma.com/docs/figma-mcp-server/tools-and-prompts and https://www.figma.com/blog/the-figma-canvas-is-now-open-to-agents/ (snippets only). Claim that agent actions go through version history: third-party directory, **unverified**. No official description of live display of agent edits found: **unverified**.
- v0 versions and restore: https://action.v0.build/docs/versions and https://community.vercel.com/t/restoring-previous-versions-after-branch-updates-in-v0/31990 (search snippets, third-party mirrors, **unverified**). No per-change accept/reject UI found for v0.
- Competitor and Wonder facts come from the project memory documents above, which mark their own primary `[P]` and secondary `[S]` sources.

## 14. Alignment with architecture decisions (13 and 16 Nov 2026)

Decisions made after this draft (`docs/decisions.md` items 20 to 24 and 31; ADR-0007 to ADR-0010 and ADR-0012):

- **Where the panels live.** The activity panel, review and revert state and the follow or touch hints are rules of the UI-neutral app layer (`meldframe-app`, ADR-0008), not of the Svelte client. The web client only renders that state and dispatches actions. The editor shell is the first app-layer slice (M1); the activity panel follows in M5 (`docs/epics.md`).
- **Touch rings and `userFocus` are ephemeral events** (ADR-0009): best-effort, no sequence number, never replayed, and a full client queue drops them and sends a resync marker. Accepted ops and batch notes are durable frames with sequence numbers.
- **Risk tiers and the gate (ADR-0010).** Every agent change passes one admission gate with risk tiers Read, Change and Delete. This draft assumes live apply for all agent changes (P-1) and does not say what Delete means for an agent that removes a whole frame or a large subtree. That mapping is open and is asked in `docs/owner-questionnaire.md` section 4.
- **Agent grants.** An agent connects through an OAuth grant through Gatehouse (decision 31), with no token copied by hand; the stop button (Q3) revokes that grant. Actor profiles are derived from the authorised agent.
- **Features as modules.** `review` (activity, revert, checkpoints) is a feature module that registers its agent tools (`revert`, change queries, notes) with risk tiers in the module registry (ADR-0007).
- **No separate spikes.** Items marked "needs measurement" (for example the implicit `ifSeq` default) are measured in the repository on real runs.

