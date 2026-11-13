---
status: proposed
---
# Agent changes apply live and are reviewed afterwards; the batch is the unit; revert is a new change

Agent edits appear on the canvas immediately and are labelled by actor. The human reviews batches afterwards and keeps or reverts them. There is no staged "propose only" mode at launch. A revert is a new batch authored by the human, computed with the field-level undo rules, atomic, itself undoable, and it never rewrites the log. Undo by a person still touches only that person's own changes.

## Considered options

- Staged proposals that need acceptance before applying: deferred; it adds a second document state and slows the agent loop. Revisit if live apply proves too risky.
- Locks on nodes: rejected; soft hints and `ifSeq` are enough and do not stall agents.

## Consequences

The server needs actor profiles, batch notes, a query for changes and last writers, and a revert operation. Details: `docs/collab-ux.md`.
