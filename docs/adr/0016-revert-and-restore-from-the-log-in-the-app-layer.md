---
status: proposed
---
# Revert and restore are worked out from the log in the app layer, and a field reverts only while its newest writer is the batch

ADR-0005 makes a revert a new batch of the person that never rewrites the log. This records how it is computed.

- **A batch is the ops of one actor whose keys are `{prefix}:{n}`.** The log keeps no batch id; the keys of one `apply_commands` call already carry one, so no migration is needed. A batch made of ops from the editor (one key per op) is not a batch in this sense.
- **A field reverts only when the newest op that wrote it belongs to the batch.** ADR-0005 says "still the last writer is that agent". The rule is stricter: a later batch of the same agent is another batch, so reverting the first one leaves what the second wrote. The place of a node (parent and order) is one unit. A node the batch added goes only when nothing outside the batch touched its subtree.
- **The rules are pure functions over a tree and a slice of the log** (`meldframe-revert`, kind core, portable), so the app layer, a server and a test give the same answer. The app layer keeps the confirmed ops since it loaded the file (`ConfirmedLog`: 2,000 to 2,255; when it reaches 2,256 the oldest are folded into the base tree in one pass, down to 2,000, so the cost is the size of the file once per 256 ops) and works out a revert or a restore from them. The output is a list of commands that applies whole, applied as one history group of the person: it is undoable with the person's own undo, and the log only grows.
- **Checkpoints are numbers in the log** (`seq`), not copies of the file, and each is of the confirmed state: the person's own pending ops (sent, not yet confirmed) are not in it. A checkpoint older than the record is dropped when the record moves; past 100, the ones the app took go before the saved ones. A restore is the difference between the file at that number and now, as commands. The app takes one at the start of another actor's run (5 minutes without an op of that actor ends a run; another person's run counts too, as `OtherActorRun`), before a revert and before a restore.

## Considered options

- Keep the old value of every field beside the file (a projection per field): rejected; it is a second state to keep right, and the log already holds the truth.
- Compute the revert on the server: kept open. The same functions run there; the app layer is first because the editor holds the person's pending ops, which are part of "last writer".

## Consequences

- A revert reaches back only as far as the ops the app holds. A server query of changes and a server-held checkpoint list are still to build; the functions need no change for them.
- The ops of a revert are sent one by one like any edit, so the room is not atomic for them (the batch check of `meldframe-batch` is not on this path); a refusal in the middle leaves the earlier commands written. A frame removal inside a revert is a Delete-tier op that the gate does not admit alone (ADR-0010): it is refused and dropped, and the app tells the person which removal (`OpRejected` carries the node).
- A restore removes nodes leaves first, so each removal is judged by the gate alone: the removal of a non-frame node is small and passes, while the removal of a frame is Delete tier even when it is empty (ADR-0010) and is refused here. Nothing files an approval request for it: the refused op is dropped, so restoring to before an agent added a frame removes its content and leaves the empty frame in the file until an approval path exists (E2.4); the person is told through `OpRejected` with the node. The same holds for a revert of an added frame.
- A restore checks its own result: it replays the commands on the current tree and compares with the checkpoint's tree, sibling order included, before returning them. A difference (a moved node whose target has no order, say) is `log_mismatch` and nothing is sent.
- `Reverted` and `Restored` report the commands as `requested`: the sends are answered one by one, so the events say what was asked, not what the server accepted. A refused command is an `OpRejected`, and the ops that depend on it fall with it (tested). A batch whose first ops were folded into the base is refused (`batch_incomplete`) rather than reverted in part.
