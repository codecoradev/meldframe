---
status: proposed
---
# Revert and restore are worked out from the log in the app layer, and a field reverts only while its newest writer is the batch

ADR-0005 makes a revert a new batch of the person that never rewrites the log. This records how it is computed.

- **A batch is the ops of one actor whose keys are `{prefix}:{n}`.** The log keeps no batch id; the keys of one `apply_commands` call already carry one, so no migration is needed. A batch made of ops from the editor (one key per op) is not a batch in this sense.
- **A field reverts only when the newest op that wrote it belongs to the batch.** ADR-0005 says "still the last writer is that agent". The rule is stricter: a later batch of the same agent is another batch, so reverting the first one leaves what the second wrote. The place of a node (parent and order) is one unit. A node the batch added goes only when nothing outside the batch touched its subtree.
- **The rules are pure functions over a tree and a slice of the log** (`meldframe-revert`, kind core, portable), so the app layer, a server and a test give the same answer. The app layer keeps the confirmed ops since it loaded the file (newest 2,000) and works out a revert or a restore from them. The output is a list of commands that applies whole, applied as one history group of the person: it is undoable with the person's own undo, and the log only grows.
- **Checkpoints are numbers in the log** (`seq`), not copies of the file. A restore is the difference between the file at that number and now, as commands. The app takes one at the start of another actor's run (5 minutes without an op of that actor ends a run), before a revert and before a restore.

## Considered options

- Keep the old value of every field beside the file (a projection per field): rejected; it is a second state to keep right, and the log already holds the truth.
- Compute the revert on the server: kept open. The same functions run there; the app layer is first because the editor holds the person's pending ops, which are part of "last writer".

## Consequences

- A revert reaches back only as far as the ops the app holds. A server query of changes and a server-held checkpoint list are still to build; the functions need no change for them.
- The ops of a revert are sent one by one like any edit, so the room is not atomic for them (the batch check of `meldframe-batch` is not on this path); a refusal in the middle leaves the earlier commands written. A frame removal inside a revert is a Delete-tier op and is refused until approved (ADR-0010).
