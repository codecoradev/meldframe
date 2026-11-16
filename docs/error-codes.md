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
| `E_TOO_MANY_ENTRIES` | A list field holds more entries than allowed; today `shadows` has more than the limit. | Send fewer entries; merge or drop shadows you do not need. |
| `E_INVALID_FONT_WEIGHT` | `fontWeight` is not 100 to 900 in steps of 100. | Send one of 100, 200, 300, 400, 500, 600, 700, 800 or 900. |
| `E_INVALID_ASSET_ID` | `image.assetId` is not a lowercase hex SHA-256 (64 characters). | Upload the image first and use the asset id the upload returned. |
| `E_IMAGE_ON_TEXT` | An image fill was set on a text node; a text node cannot hold one. | Put the image on a frame or shape behind the text, or remove the fill. |
| `E_MISSING_POS` | A stored node has no `pos`, so its place among its siblings is undefined. Load only: the loader migrates an old file first, so this means a caller used `check_file` without migrating. | Not for an agent. The server refuses to open the file and keeps its bytes; an operator migrates or repairs the stored file. |
| `E_DUPLICATE_POS` | Two stored siblings have the same `pos`, so their order is undefined. Load only: the loader migrates an old file first, so this means a caller used `check_file` without migrating, or on a live tree where concurrent edits made equal `pos` values. | Not for an agent. The server refuses to open the file and keeps its bytes; an operator repairs the stored file. |

## Gate refusal codes

The admission gate and the room refuse an op with a typed `Refusal` (`crates/core/meldframe-host`). A refusal
whose cause is the command itself carries the `E_` code of its `ApplyError` from the table above. The refusals
below are the gate's own: lower-case strings, as `ServerMsg::Error.code` carries them on the wire. They follow
the same rules: a code never changes meaning and is never reused, and a refused op writes nothing.

A test in `crates/core/meldframe-host/tests/refusal_codes.rs` reads this table and fails if a variant has no
row, a row has no variant, or two variants share a code.

| Code | Meaning and typical cause | What an agent should do next |
| --- | --- | --- |
| `forbidden` | The policy for the call's risk tier does not let this principal do it. Today a delete of a frame, of a node that is not in the file, or of a subtree above the small-delete limit is `Ask`, which is refused here; through the approval seam it opens an approval instead (see the approval codes above) (a small delete is Auto; today only a frame can have children, so in practice every non-frame delete is small and every frame delete is Ask, and a relaxed delete whose node became a frame or gained children before the room applied it is refused with this code too); with no readable policy only reads pass. It also covers a call that is not allowed to the tool: a tool the server was not built with (left out of the build), or an op of a higher risk tier than its tool. | Do not retry the same call unchanged. Use a lower-risk call, ask the owner to change the policy, or, where the server offers the approval seam, expect `approval_required` instead and follow it. |
| `quota_exceeded` | The workspace used up its quota of ops. Retryable. | Wait, then send the same batch again; the refused batch was not counted. |
| `conflict` | A node the command touches was changed by another op after the `ifSeq` the caller sent. | Read the node again, rework the command on the new state, and send it with a fresh `ifSeq`. |
| `invalid` | The op is malformed before it is applied: an empty batch, an empty or repeated idempotency key, an op that depends on itself, an empty or repeated dependency key, or more than 512 dependencies. | Fix the op as the message says and send it again. |
| `dependency_rejected` | A dependency was not applied for this actor: it was refused, never sent, or belongs to another actor. | Resend the dependency first, or send the op without it. |

## Approval refusal codes

A call whose policy is Ask can be sent through the approval seam (`Approvals::submit` in
`crates/core/meldframe-approvals`) instead of straight to the gate. The seam adds three codes, same rules as the
gate codes: lower-case strings, never changed in meaning, never reused, and a refused op writes nothing. Every
other refusal passes through with its own code (`forbidden` for a Deny policy, `quota_exceeded`, `invalid`).
Each of the three carries the approval id. A call sent straight to the gate is unchanged: Ask is `forbidden`.

A test in `crates/core/meldframe-approvals/tests/approval_codes.rs` reads this table and fails if a code has no
row or a row has no code.

| Code | Meaning and typical cause | What an agent should do next |
| --- | --- | --- |
| `approval_required` | The policy for the call is Ask and the owner has not approved it. An approval is pending; sending the same op again while it is pending returns the same id. | Wait for the owner's decision, then send the same op again (same key, command, `ifSeq` and dependencies). An approval admits one call, once; a second call asks again. |
| `approval_denied` | The owner denied this op. It stays refused. | Do not retry it. To ask again, send a new op with a new key. |
| `approval_expired` | The approval ran out (pending or approved) before it was used. It stays refused. | Send a new op with a new key to ask again. |

## Transport codes

The HTTP adapter (`crates/integrations/meldframe-http` (tests in `crates/core/meldframe-http-tests`)) answers REST and WebSocket calls with one JSON shape,
`{"error": {"code", "message", "retryable"}}`, and an HTTP status (a WebSocket reply frame carries the same body
plus `id` and `status`). A gate refusal keeps its own code from the tables above; the statuses are `forbidden` 403,
`quota_exceeded` 429, `conflict` 409, `invalid` 400, `dependency_rejected` and every `E_` code 422. The codes below
belong to the transport; same rules: a code never changes meaning and is never reused. A call refused before it reaches the submitter (`unauthenticated`, `rate_limited`, `too_large`, `bad_request`,
`request_timeout`, an over-cap `unavailable`) wrote nothing. `timeout`, `unavailable` and `internal` can come after the
earlier ops of a batch were written (each op is its own durable write), and after `timeout` the job may still finish:
send the batch again with the same keys.

| Code | Status | Meaning and typical cause | What an agent should do next |
| --- | --- | --- | --- |
| `unauthenticated` | 401 | No token, or one that is unknown or revoked. | Send a valid token as `Authorization: Bearer <token>`. |
| `rate_limited` | 429 | The peer address or the principal used up its budget. The refusal comes before the body is read. `Retry-After` says when. | Wait, then send the same call again. |
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

## MCP tool errors

An MCP tool call that fails returns a tool error (`isError: true`) whose structured body is `{"ok": false, "error": {"code", "message", "retryable", ...}}`: the same shape as the transport above. A refusal keeps its code from the tables above (`conflict`, `forbidden`, `E_...`) and adds, when they apply, `index` (the command of an atomic batch that would be refused, from 0), `nodeId` (a conflict) and, from `apply_commands` with `ifSeq`, `currentSeq`, `current` (the node now, `null` if removed) and `changes` (`seq`, `actor`, `command` of each op since `ifSeq` that touched the node). REST carries `nodeId` and `index` too. Two codes are worded by the MCP tools: `invalid` also covers a missing, empty or doubled idempotency key (or `ifSeq`), an `ifSeq` more than 2,000 changes behind the file, an `ifSeq` that cannot guard a command of a non-atomic call (with its `index`), a batch of 0 or more than 100 commands, a bad file name, and an `ids` list of 0 or more than 100; `bad_request` covers arguments that do not fit the input schema (an unknown field, a wrong type) and a command that is not a command (with its `index`). A call refused before MCP sees it (`unauthenticated`, `rate_limited`, `too_large`) is an HTTP answer in the transport shape, not a tool error.
