# Error codes of `meldframe-check`

Every `ApplyError` has a stable machine code, returned by `ApplyError::code()`. A `LoadError` (a stored file refused when it is loaded) has a code too: it reuses the code of the `ApplyError` it wraps, and adds the two codes at the end of the table. A code is a
SCREAMING_SNAKE string that starts with `E_`. Once released, a code never changes meaning and is never
reused; a variant may gain details, but its code stays. The English message may be reworded; match on the
code, not on the message.

A refused command leaves the file unchanged, so an agent can always correct the command and send it again.

A test in `crates/core/meldframe-check/tests/error_codes.rs` reads this table and fails if a variant (of either type) has no
row, a row has no variant, or two variants share a code. Add the row in the same change that adds the
variant.

| Code | Meaning and typical cause | What an agent should do next |
| --- | --- | --- |
| `E_NODE_NOT_FOUND` | A node id in the command does not exist. The id has a typo, or the node was removed by another actor. | Read the file tree again and use an id that exists. |
| `E_DUPLICATE_ID` | An added node uses an id that is already taken. | Choose a fresh id, or edit the existing node instead of adding it. |
| `E_PARENT_NOT_FOUND` | The new parent does not exist. | Read the file tree again and use an existing frame as parent. |
| `E_PARENT_NOT_FRAME` | The new parent exists but is not a frame; only frames hold children. | Add a frame first, or choose a frame as parent. |
| `E_CYCLE` | A reparent would make a node its own ancestor. | Pick a parent that is not the node or one of its descendants. |
| `E_TOO_MANY_NODES` | The file would hold more nodes than the limit. | Remove nodes, or split the work over several files. Read the limit with `whoami`. |
| `E_TOO_DEEP` | The tree would nest deeper than the limit. | Flatten the nesting; put the node higher in the tree. |
| `E_INVALID_ID` | An id is empty, too long, or uses characters other than letters, digits and `_-:.`. | Rewrite the id within those rules. |
| `E_NOT_FINITE` | A number is NaN or infinite. | Send a finite number. |
| `E_OUT_OF_RANGE` | A number is outside the range its field allows. | Send a value inside the range named in the message. |
| `E_TOO_LONG` | A string is longer than its limit. | Shorten the string to the length named in the message. |
| `E_INVALID_COLOR` | A colour is not hex (`#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa`). | Send the colour as hex. |
| `E_INVALID_POS` | A `pos` from a client has bad characters or a bad shape. | Do not send `pos`; use `before` or `after` and let the server compute the position. |
| `E_POS_TOO_LONG` | A `pos` is longer than the limit. A `pos` you sent is simply too long; a computed one means the gap between siblings is too crowded. | If you sent a `pos`, stop sending it and use `before` or `after`. If the server computed it, ask for the siblings to be rebalanced, then send the command again. |
| `E_BAD_REORDER` | A reorder is incoherent: more than one of `before`, `after` and `pos`, or the anchor does not exist or is not a sibling. | Send exactly one of them, and an anchor that is a sibling of the node. |
| `E_TOO_MANY_ENTRIES` | A list field holds more entries than allowed: `shadows` over its limit (4), or the stops of a `gradient` over its limit (8). | Send fewer entries; merge or drop shadows or stops you do not need. |
| `E_INVALID_FONT_WEIGHT` | `fontWeight` is not 100 to 900 in steps of 100. | Send one of 100, 200, 300, 400, 500, 600, 700, 800 or 900. |
| `E_INVALID_ASSET_ID` | `image.assetId` is not a lowercase hex SHA-256 (64 characters). | Upload the image first and use the asset id the upload returned. |
| `E_IMAGE_ON_TEXT` | An image fill was set on a text node; a text node cannot hold one. | Put the image on a frame or shape behind the text, or remove the fill. |
| `E_INVALID_GRADIENT` | A gradient has fewer than 2 stops, or its stop positions go back (a position below the one before). The message says which. A gradient with too many stops is `E_TOO_MANY_ENTRIES`, a position outside 0 to 1 or an angle outside -3600 to 3600 is `E_OUT_OF_RANGE`, a colour that is not hex is `E_INVALID_COLOR`. | Send 2 to 8 stops with positions from 0 to 1 in order (equal positions make a hard edge). |
| `E_GRADIENT_ON_TEXT` | A gradient was set on a text node; a text node is painted with its `fill` colour. | Put the gradient on a frame or shape behind the text, or remove it. |
| `E_MISSING_POS` | A stored node has no `pos`, so its place among its siblings is undefined. Load only: the loader migrates an old file first, so this means a caller used `check_file` without migrating. | Not for an agent. The server refuses to open the file and keeps its bytes; an operator migrates or repairs the stored file. |
| `E_DUPLICATE_POS` | Two stored siblings have the same `pos`, so their order is undefined. Load only: the loader migrates an old file first, so this means a caller used `check_file` without migrating, or on a live tree where concurrent edits made equal `pos` values. | Not for an agent. The server refuses to open the file and keeps its bytes; an operator repairs the stored file. |

The server runs this load check in `Room::open` on the stored snapshot (after the old-file migration, so a snapshot written before position keys still opens; only the copy in memory is migrated). A file that fails it is not opened (`RoomError::BadSnapshot`, carrying the snapshot's sequence number and the code of the failure), its bytes are kept, and a call that needs the file is answered `unavailable` 503 (REST, MCP and the live socket alike); the code is in the server log for REST and MCP (the live handshake logs the error in its debug form), not in the reply. This case does not clear by itself: the file stays refused until an operator repairs it, so a caller that keeps retrying will keep getting the same answer.

## Gate refusal codes

The admission gate and the room refuse an op with a typed `Refusal` (`crates/core/meldframe-host`). A refusal
whose cause is the command itself carries the `E_` code of its `ApplyError` from the table above. The refusals
below are the gate's own: lower-case strings, as `ServerMsg::Error.code` carries them on the wire. They follow
the same rules: a code never changes meaning and is never reused, and a refused op writes nothing.

A test in `crates/core/meldframe-host/tests/refusal_codes.rs` reads this table and fails if a variant has no
row, a row has no variant, or two variants share a code.

| Code | Meaning and typical cause | What an agent should do next |
| --- | --- | --- |
| `forbidden` | The policy for the call's risk tier does not let this principal do it. Today a delete of a frame, of a node that is not in the file, or of a subtree above the small-delete limit is `Ask`, which the gate refuses here; a single-op delete through the server opens an approval instead (see the approval codes below) (a small delete is Auto; today only a frame can have children, so in practice every non-frame delete is small and every frame delete is Ask, and a relaxed delete whose node became a frame or gained children before the room applied it is refused with this code too); with no readable policy only reads pass. It also covers a call that is not allowed to the tool: a tool the server was not built with (left out of the build), or an op of a higher risk tier than its tool. | Do not retry the same call unchanged. Use a lower-risk call, ask the owner to change the policy, or, where the server offers the approval seam, expect `approval_required` instead and follow it. |
| `quota_exceeded` | The workspace used up its quota of ops. Retryable. | Wait, then send the same batch again; the refused batch was not counted. |
| `quota_ops_per_minute` | The workspace sent more ops in this minute than its rate allows (the count starts again at the next minute). Retryable. HTTP 429 with `Retry-After`. | Wait the time in the message (or `Retry-After`), then send the same batch again; the refused batch was not counted. |
| `batch_too_large` | One call carries more ops than the batch limit (100), or more than the ops per minute of the workspace (no minute can hold it). HTTP 413. | Split the batch into calls of at most the limit in the message. |
| `quota_storage` | The workspace used up its storage: the bytes of the ops it wrote. The op log only grows, so removing content does not free room, and a remove is refused too once the storage is used up. HTTP 413. | Do not retry. Ask the owner for more room. |
| `conflict` | A node the command touches was changed by another op after the `ifSeq` the caller sent. | Read the node again, rework the command on the new state, and send it with a fresh `ifSeq`. |
| `invalid` | The op is malformed before it is applied: an empty batch, an empty or repeated idempotency key, an op that depends on itself, an empty or repeated dependency key, or more than 512 dependencies. | Fix the op as the message says and send it again. |
| `dependency_rejected` | A dependency was not applied for this actor: it was refused, never sent, or belongs to another actor. | Resend the dependency first, or send the op without it. |

## Approval refusal codes

A call whose policy is Ask can be sent through the approval seam (`Approvals::submit` in
`crates/core/meldframe-approvals`) instead of straight to the gate. The seam adds three codes, same rules as the
gate codes: lower-case strings, never changed in meaning, never reused, and a refused op writes nothing. Every
other refusal passes through with its own code (`forbidden` for a Deny policy, `quota_exceeded`, `invalid`).
Each of the three carries the approval id. A call sent straight to the gate is unchanged: Ask is `forbidden`.

Besides the three codes the seam has one more, `approval_limit`: too many approvals are open (20 for an actor, 100
for a workspace, 2,000 in all), and an op key of more than 100 characters opens no approval and is refused
as `forbidden` (over MCP the op key is the call key plus `:` and the index of the command, so a call key near 100
characters is refused too). None of the codes is a reason to retry by itself: the call stays refused until the owner has
answered. `Approvals::decide` takes the owner's answer; `Approvals::answer` does the same for a principal and refuses
the actor that asked and an approval of another workspace. A pending or approved approval lives for the ttl the
caller gives it; `Approvals::prune` keeps a finished one one more ttl, so a retry in that time meets
`approval_denied` or `approval_expired`, and after it the same op asks again. An approval covers the exact op:
the same key, command, `ifSeq` and dependencies. A new idempotency key makes a new op, so a denied ask can be
asked again with a new key. An approval is not tied to the state of the tree: a retry without `ifSeq` is admitted
even if the node gained content after the ask. An approved op that the room then refuses (`conflict`, `invalid`) has
spent its approval, and after a server restart the approvals are gone: the same call asks again under a new id.

The server uses the seam for a call of exactly one op that deletes (a frame, or a subtree above the small-delete
limit) through a Change tool, over REST, WebSocket and MCP. The codes answer with HTTP 403 and `approvalId` in the
error (`approval_limit`: HTTP 429, no id) and none is `retryable`: a client must not send the call again by itself,
only after the owner has answered. A call of several ops that holds such a delete is still `forbidden`: an approval
covers one op, and the ops of the call are counted as sent (a retry of ops already written still counts them). A
delete of a node that is not in the file is not asked about and keeps its refusal. The owner answers with
`Server::decide_approval` (approve or deny; an actor cannot answer its own request); no route or tool offers that
yet, because the server cannot tell an owner from an agent until sign-in exists. The ttl is 15 minutes
(`APPROVAL_TTL_MS`). The retry must be the same op, and the approval is not tied to the state of the tree: a retry
without `ifSeq` can delete a frame that gained content after the ask, so send the retry with the same `ifSeq` (tying
an approval to the subtree it was asked about is a follow-up). Each step is in the call log: `approval_required` is
an ask, `approval_denied`, `approval_expired` and `approval_limit` are denies, and the owner's answer is an entry of
the tool `decide_approval`.

A test in `crates/core/meldframe-approvals/tests/approval_codes.rs` reads this table and fails if a code has no
row or a row has no code.

| Code | Meaning and typical cause | What an agent should do next |
| --- | --- | --- |
| `approval_required` | The policy for the call is Ask and the owner has not approved it. An approval is pending; sending the same op again while it is pending returns the same id. | Wait for the owner's decision, then send the same op again (same key, command, `ifSeq` and dependencies). An approval admits one call, once; a second call asks again. |
| `approval_denied` | The owner denied this op. It stays refused. | Do not retry it. To ask again, send a new op with a new key. |
| `approval_expired` | The approval ran out (pending or approved) before it was used. It stays refused. | Send a new op with a new key to ask again. |
| `approval_limit` | Too many approvals are open: 20 for the actor, 100 for the workspace, 2,000 in all. A transport answers HTTP 429. Not retryable by itself. | Wait until the owner answers one of the open approvals or one runs out, then ask again. Do not send more deletes meanwhile. |

## Transport codes

The HTTP adapter (`crates/integrations/meldframe-http` (tests in `crates/core/meldframe-http-tests`)) answers REST and WebSocket calls with one JSON shape,
`{"error": {"code", "message", "retryable"}}`, and an HTTP status (a WebSocket reply frame carries the same body
plus `id` and `status`). A gate refusal keeps its own code from the tables above; the statuses are `forbidden` 403,
`quota_exceeded` and `quota_ops_per_minute` 429, `batch_too_large` and `quota_storage` 413, `conflict` 409, `invalid` 400, `dependency_rejected` and every `E_` code 422. The codes below
belong to the transport; same rules: a code never changes meaning and is never reused. A call refused before it reaches the submitter (`unauthenticated`, `rate_limited`, `too_large`, `bad_request`,
`request_timeout`, an over-cap `unavailable`) wrote nothing. `timeout`, `unavailable` and `internal` can come after the
earlier ops of a batch were written (each op is its own durable write), and after `timeout` the job may still finish:
send the batch again with the same keys.

| Code | Status | Meaning and typical cause | What an agent should do next |
| --- | --- | --- | --- |
| `unauthenticated` | 401 | No token, or one that is unknown or revoked. | Send a valid token as `Authorization: Bearer <token>`. |
| `rate_limited` | 429 | The peer address or the principal used up its budget. The refusal comes before the body is read. `Retry-After` says when. | Wait, then send the same call again. |
| `not_found` | 404 | `GET /v1/files/{file}` named a file that has no ops in the caller's workspace (a file of another workspace looks the same). Reading writes nothing. | Write to the file first, or use the right name. |
| `too_large` | 413 | The body or frame is over the size limit. | Send fewer ops per call. |
| `bad_request` | 400 | The body or frame is not the documented JSON (also an unknown field). | Fix the call as the message says. |
| `request_timeout` | 408 | The body of the request took too long to arrive. | Send the call again. |
| `timeout` | 504 | The call took longer than the time limit. | Send the same batch again with the same keys; ops already written come back as `written: false`. |
| `unavailable` | 503 | The store is down, the server is full, or too many calls are in flight (global or per principal, REST and the live handshake; the live calls after the handshake have their own slots and are skipped and tried again, never answered with this code); not a refusal of the call. A store error that says retrying cannot succeed is `invalid` 400, not retryable. | Wait, then send the same call again. |
| `internal` | 500 | The call failed inside the server. | Send it again later; report it if it repeats. |

Notes on the transport. `timeout` covers the time from the start of the blocking job, including any wait for the
server's lock; the job is not cancelled and keeps its in-flight slot until it ends. A WebSocket reply frame carries
`id`, `status` and the body but not `Retry-After`. On a WebSocket the per-address limit is spent at the handshake only;
frames are limited per principal, and a connection is closed after several limited frames in a row or when idle.
`GET /v1/live/{file}?after={seq}` answers its handshake with these codes (`unavailable` 503 when the stream cap is reached or the store cannot be read; `bad_request` 400 for a bad file name or a non-numeric `after`); after the upgrade, a message it cannot take gets an `error` frame with `bad_request` and the socket stays open (`subscribe` before a `resync` marker is one). A `subscribe` that costs more than the budget has left gets `rate_limited` (retryable, socket open, `retryAfterMs` says when); one the server cannot take now gets `unavailable` (retryable, socket open, `retryAfterMs` is the poll interval) when the server is busy, and costs one frame then, not four, or `unavailable` and a close when the stream could not be restarted or ended before the socket was ready: the text is fixed, and the client connects again with its last sequence number. A socket closes without a frame when a poll timed out or failed.
A browser cannot set `Authorization` on a WebSocket, so there is no browser client on this path until Gatehouse; an
`Origin` check belongs with it.

## Server limits

`Server` (`apps/server`) checks these before it opens a room, so a refused call costs no store read. A name the server refuses is `RegistryError::InvalidName`; a
batch the server refuses is a `Refusal::BatchTooLarge` (code `batch_too_large`, HTTP 413, never retryable); a room limit is a
`RegistryError`, which a transport answers as `unavailable` (503, retryable) because it clears when a room closes
(`Server::close`, or idle eviction after 5 minutes). There is no `Retry-After` on these 503s yet: `Failure::Unavailable` carries no hint and `meldframe-http` is at its size ceiling.

| Check | Typed error | Transport code | What a caller should do next |
| --- | --- | --- | --- |
| Workspace or file name is empty, over 128 bytes, only whitespace, or holds a control character (NUL included) or an invisible formatting character (bidi controls, zero-width marks, U+2028/2029, U+FEFF); a file is also not `.` or `..` (`Server::open`, `submit_tool` and `subscribe`; `with_room` finds nothing; a transport sends `invalid` 400, not `unavailable`). The REST and WebSocket adapters check file names further (no slash) before this. | `RegistryError::InvalidName` | `invalid` | Use a valid name. |
| A call has more than 100 ops (`MAX_BATCH_OPS`, the MCP `batchLimit`; this applies to REST too). Checked before any key is looked up and, for an atomic batch, before the room is opened (`Server::precheck`). The same code, with the rate as the limit, for a call with more ops than the ops per minute of its workspace (no minute can hold it); the gate decides that one after the room is opened and the keys looked up, and ops that replay an earlier key do not count. | `Refusal::BatchTooLarge` | `batch_too_large` | Split the batch. |
| A Read-tier tool submits ops that are writes whatever the tree (anything but a delete). Refused before the room is opened, with the same `forbidden` refusal as before; so a Read tool that sends already-applied non-delete ops again gets `forbidden`, not `written: false`. | `Refusal::Forbidden` | `forbidden` | Use a Change-tier tool. |
| The workspace already holds `MAX_ROOMS_PER_WORKSPACE` (32, `Server::with_workspace_rooms`) open rooms. Other workspaces are not affected. Rooms idle for 5 minutes are closed first when an open hits a cap (and by `Server::tick`), so a workspace that only reads or streams gets its slots back. | `RegistryError::WorkspaceFull` | `unavailable` | Wait, or close a file. |
| The server holds `MAX_OPEN_ROOMS` (256) open rooms in all. | `RegistryError::Full` | `unavailable` | Wait, then send the call again. |

## MCP tool errors

An MCP tool call that fails returns a tool error (`isError: true`) whose structured body is `{"ok": false, "error": {"code", "message", "retryable", ...}}`: the same shape as the transport above. A refusal keeps its code from the tables above (`conflict`, `forbidden`, `E_...`) and adds, when they apply, `index` (the command of an atomic batch that would be refused, from 0), `nodeId` (a conflict), `approvalId` (an approval refusal, but not `approval_limit`) and, from `apply_commands` with `ifSeq`, `currentSeq`, `current` (the node now, `null` if removed) and `changes` (`seq`, `actor`, `command` of each op since `ifSeq` that touched the node). REST carries `nodeId` and `index` too. Two codes are worded by the MCP tools: `invalid` also covers a missing, empty or doubled idempotency key (or `ifSeq`), an `ifSeq` more than 2,000 changes behind the file, an `ifSeq` that cannot guard a command of a non-atomic call (with its `index`), a batch of 0 or more than 100 commands, a bad file name, and an `ids` list of 0 or more than 100; `bad_request` covers arguments that do not fit the input schema (an unknown field, a wrong type) and a command that is not a command (with its `index`). A call refused before MCP sees it (`unauthenticated`, `rate_limited`, `too_large`) is an HTTP answer in the transport shape, not a tool error.

## Replica codes

The client-side `Replica` (`crates/core/meldframe-sync`) refuses a new local op (`apply`, `undo`, `redo`) with a `SubmitError`, and a `resync` with a `ResyncError`. An invalid command keeps the `E_` code of its `ApplyError` from the table above, and a bad snapshot keeps the code of its `LoadError`; the replica adds the two codes below, with the same rules: a code never changes meaning and is never reused, and a refused call changes nothing (the op is not applied, queued or recorded in the history; the replica is left as it was).

The app layer (`meldframe-app`) words a refusal of a document action as `Event::Refused { code }` and has no wire code for these: a full queue is `outbox_full` (a list action, such as moving a selection, is checked whole first), and a refused `Action::Reload` carries the `E_` code of the `ResyncError`.

A test in `crates/core/meldframe-sync-scenarios/tests/outbox_cap.rs` reads this table and fails if the rows differ from the codes, or if the section stops naming the app's `Event::Refused` and `outbox_full`.

| Code | Meaning and typical cause | What an agent should do next |
| --- | --- | --- |
| `E_OUTBOX_FULL` | The replica already holds as many ops as its cap (`Replica::max_pending`, 10,000 by default) that the server has not confirmed: the connection is down or the server is slow. `Replica::undo` and `redo` check it only when there is a step to take. In the app layer the same refusal is `Event::Refused` with the code `outbox_full`. | Do not send more. Wait for the connection to come back and the queue to drain (or reload after `Halted`), then send the command again. |
| `E_SNAPSHOT_BEHIND` | `Replica::resync` (the app's `Action::Reload`) was given a snapshot older than the events the replica already applied: loading it would go back. | Load the file again (a newer snapshot) and retry. |

## Asset codes

The `assets` feature crate (E2.5, ADR-0015) fails with these codes. `POST /v1/assets` and `GET /v1/assets/{id}` answer them as `{"error": {"code", "message", "retryable"}}` with these statuses: `invalid` 400, `asset_too_large` 413, `asset_type_refused` 415, `asset_not_found` 404, `asset_corrupt` 422, `unavailable` 503. The routes also answer the codes of every route (`unauthenticated` 401, `rate_limited` 429), and `quota_storage` 413 when a new image does not fit the storage limit of the workspace (assets and ops count together), `bad_request` 400 when the body cannot be read, `request_timeout` 408 when an upload takes longer than a minute to arrive (retryable), and `unavailable` 503 when every upload place is taken (retryable). The `upload_asset` MCP tool answers the same codes as tool errors (`{"ok": false, "error": {"code", "message", "retryable"}}`), plus `forbidden` when the policy does not allow it, `invalid` when `data` is not base64 and `bad_request` when the arguments do not fit its input schema.

| Code | Meaning and typical cause | What a caller should do next |
| --- | --- | --- |
| `asset_type_refused` | The bytes are not a png, jpeg, webp or gif image; SVG is refused. | Convert the image to one of the four types. |
| `asset_too_large` | The upload is over 10 MiB. | Make the image smaller. |
| `asset_not_found` | No asset with this id in the caller's workspace (another workspace's asset is not found either). | Upload the image first and use the id the upload returned. |
| `invalid` | The upload is empty, or an id is not a lowercase hex SHA-256. | Send the bytes of the image, or the id the upload returned. |
| `asset_corrupt` | The stored file no longer matches its id (changed on disk, or cut short by a crash). Not retryable as a read. | Upload the same image again: that writes the file anew. |
| `unavailable` | The disk failed. | Try again later; report it if it stays. |

## Variant codes

A variant of a frame (`meldframe-variant`, E5.4) fails with these codes before any command is sent: the copy is applied to a copy of the file first, so it is refused whole or applied whole. The app layer reports them as `Event::Refused` with its own codes (`not_found`, `not_a_frame`; a copy that does not fit is `limit`, or `invalid` when its place would be past the coordinate limit; a copy of more nodes than the queue of pending ops holds, 10,000, is `outbox_full` and never fits).

| Code | Meaning and typical cause | What to do next |
| --- | --- | --- |
| `E_NODE_NOT_FOUND` | No node with the id. | Use the id of a frame that exists. |
| `E_NOT_A_FRAME` | The node is not a frame; only a frame has a variant. | Pick the frame that holds the node. |
| the code of the command check (`E_TOO_MANY_NODES` and the like) | The copy would not fit the file. | Make room, or make a variant of a smaller frame. |

## Review and checkpoint codes

A revert of a batch and a restore of a checkpoint (`meldframe-revert`, E5.2 and E5.3) are worked out in the app layer from the ops it has seen since it loaded the file, before any command is sent, so a refusal changes nothing. The app layer words them as `Event::Refused` with its own codes (a queue too full for the commands is `outbox_full`, as above; a skipped part of a revert is not a refusal: it is listed in `Event::Reverted`; a label of a checkpoint over 128 characters is `limit`). The server has no wire code for them: what the commands do is judged by the gate like any edit of the person. A refused op is dropped and nothing files an approval request for it: a frame removal (Delete tier) in a revert or restore is refused, the frame stays empty until an approval path exists (E2.4), and `Event::OpRejected` carries the `node` of a refused removal.

| Code | Meaning and typical cause | What to do next |
| --- | --- | --- |
| `unknown_batch` | No confirmed op of that actor has a key `{prefix}:{n}` since the file was loaded (a reload starts the record again, and the app keeps the newest 2,000 to 2,255 ops). | Pick a batch from the list of recent batches; a batch from before the load cannot be reverted from here. |
| `batch_incomplete` | Some ops of the batch were folded into the base tree because the record outgrew its size, so a revert from the ops that are left would take back only the end of the batch. Nothing is sent. | The batch cannot be reverted from here; restore a checkpoint taken after it, or undo by hand. |
| `nothing_to_revert` | Every field of the batch was written by someone else since, or the nodes are gone. | Nothing to do: the batch no longer owns anything. |
| `log_mismatch` | The ops the app kept do not replay on its base tree, or a restore could not be worked out (the commands do not give the checkpoint's tree, such as a moved node whose order cannot be written back). | Load the file again (`Action::Reload`) and retry. |
| `unknown_checkpoint` | No checkpoint of this session has that id, or it was dropped: a checkpoint older than the record is dropped when the record moves on or the file is loaded again, and past 100 the oldest the app took goes first. | Use an id from `AppState.checkpoints`. |
| `not_in_log` | The checkpoint is older than the ops the app still keeps. The app drops such checkpoints when the record moves, so this is a guard that no sequence of actions reaches today. | Restore from a newer checkpoint. |
| `nothing_to_restore` | The file is as the checkpoint has it. | Nothing to do. |

A checkpoint is of the file as the server confirmed it: the person's own ops that are sent and not yet confirmed are not in it. A restore works out its commands against the person's view, pending ops included.

## Design review codes

The `design_review` tool (feature module `design-review`, E5.4; Change tier, with the list action a Read) fails with these codes, as `{"ok": false, "error": {"code", "message", "retryable"}}`. It also answers `forbidden` when the policy of the tier does not allow the call and `bad_request` for an argument that does not fit its schema (an unknown one, a workspace among them: the workspace comes from the token only), with the meaning they have above. A refused call changes nothing, so an agent can correct it and send it again, except `add`, which is not idempotent: after a timeout, list before adding again.

A test in `crates/features/design-review/design-review/tests/memory.rs` reads this table and fails if a row has no code or a code has no row.

| Code | Meaning and typical cause | What an agent should do next |
| --- | --- | --- |
| `invalid` | An argument is wrong: an `add` with no items, more than 20, or a text that is empty, over 500 characters or holds a control character other than a line break; a `link` without `item` or `batch`, or with a batch key that is empty, over 100 characters or holds a control character; a `mark` with a status other than `open` or `addressed`; an argument the action does not take; a bad file name. | Fix the call as the message says. |
| `review_full` | The file already holds 200 items (an `add` that would pass 200 writes none of its items), or the item already links 20 batches. Not retryable. | Mark the items that are done and write fewer, broader items; link the batch to another item, or stop linking more. |
| `review_item_not_found` | The file has no item with that number in your workspace (an item of another file or workspace is not found either). | List the file and use a number it shows. |
| `batch_not_found` | None of your `apply_commands` calls in this file used that idempotency key (a batch written by another actor is not yours to link). | Pass the `idempotencyKey` of your own call, and check it with `batch_status`. |
| `unavailable` | The store failed or the server is busy. Retryable. | Try again later; list first if the call was an `add`. |

## Capture codes

The `capture` tool (feature module `capture`, Read tier) fails with these codes. A client message in `render_failed` is cut to 200 bytes and stripped of control characters. The strings are shared with the
transport codes above where the name is the same (`timeout`, `too_large`, `invalid`); here they describe the
capture, not the HTTP call. A capture changes nothing, so an agent can always try again.

A test in `crates/features/capture/capture/tests/registration.rs` reads this table and fails if the table and
`capture::CODES` differ.

| Code | Meaning and typical cause | What an agent should do next |
| --- | --- | --- |
| `no_file_selected` | The session is bound to no file and the call named none. | Select a file for the session first, then capture. |
| `room_mismatch` | The call named a file other than the one bound to the session. A capture always uses the bound file. | Capture the bound file, or change the session's file first. |
| `no_capable_client` | No editor that can capture has the bound file open. Returned at once, nothing is waited for. | Ask the person to open the file in the editor, then try again. |
| `invalid` | The request is malformed: a scale not above 0 and at most 4, or a request id that is already pending. | Fix the call as the message says. |
| `timeout` | The editor did not answer before the deadline. | Try again; check that the editor tab is open and visible. |
| `too_large` | The image is over the size limit, or the editor refused to render it that large. | Capture a smaller node or use a lower scale. |
| `node_not_found` | The editor has no node with that id (removed, or not synced yet). | Read the file again and use an id that exists. |
| `stale_client` | The editor had not yet caught up to the file's latest change. | Try again in a moment. |
| `render_failed` | The editor could not render, or sent something that is not a PNG. | Try again; if it repeats, report it. |
| `client_disconnected` | The editor closed before it answered. | Try again once the file is open in an editor. |
| `busy` | Too many captures are pending (in all, for the workspace, for you, or on one editor), the editor was refused as one too many for its workspace, or the file could not be read just now. Retryable. | Wait a moment, then try again. |
| `forbidden` | The policy for the capture tool does not allow the call, or the file is not yours to read. | Do not retry the same call; ask the owner to change the policy. |
