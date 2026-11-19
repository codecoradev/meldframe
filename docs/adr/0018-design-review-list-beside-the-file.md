---
status: proposed
---
# The critique list of a file is data beside the file, kept per workspace in its own table, and an item links the key prefix of a batch

E5.4 asks for a `design_review` tool that stores a numbered critique list per file and links items to the batches that address them. This records the choices that are hard to reverse.

- **The list is not part of the file.** An item is not an op: nothing is written to the op log, so adding, linking or marking an item is not a design change, is not undone or reverted with one (`revert_batch` does not touch it), and costs no op quota. It lives in a store of its own (`design_review::ReviewStore`, keyed by workspace and file), as the call log and the assets do.
- **One tool, one name, four actions.** `design_review` takes `add`, `list`, `link` and `mark`. The tool is registered as Change tier; its `list` action is a Read and runs under the policy of the Read tier. No action deletes or rewrites an item: an item is marked `addressed` and can be opened again, so a number always names the same text.
- **Numbers are per file, from 1, and never reused.** Items are never removed, so the next number is the highest plus one. In Postgres an add takes a transaction lock on the workspace's lists (`pg_advisory_xact_lock`) before it reads that number and the counts of the caps, so concurrent adds get distinct numbers and the caps hold. A link takes no advisory lock: it locks the row of its item (`for update`), the only row it changes. The test starts eight threads together with a barrier and asserts that every refusal is the cap; with the lock removed it fails (a primary-key clash surfaces as `unavailable`), shown by hand on three runs.
- **Limits:** text of at most 500 characters (plain: no control character but a line break, and none of the invisible formatting characters a file name refuses), 20 items per call, 200 items per file, 20 batches per item. A refused call writes nothing.
- **Caps per workspace:** 2,000 items and 100 files with a list in all. A file may not exist yet (it comes to exist with its first op), so the store counts names that have a list instead of checking that the file exists. Without a cap a workspace could store without bound outside the storage quota of ADR-0014. The caps are in the store seam, so memory and Postgres refuse the same way, with `review_full`. Postgres checks them under the workspace lock; the migration adds bounds of one row (`n` 1 to 200, 500 characters, 20 batches).
- **Policy of `list`:** the list action is run under the Read tier's policy by a fixed rule in the tool, not by the registry (which knows one tier per tool). The policy is static; a deployment cannot yet set it per action.
- **File name length:** the schema says `maxLength` 128, which counts characters; the server counts bytes (128, UTF-8). A longer name is refused as `invalid`, as for every other tool.
- **A batch is named by the key prefix of an `apply_commands` call**, the same name `revert_batch` uses (ADR-0016), so the log needs no batch id. A link is accepted only when the caller wrote that batch: the tool looks for the caller's own op `{prefix}:0` in the file (`batch_not_found` otherwise). A batch of another actor is not linkable. The check is made when the link is written; a batch that is later reverted stays linked.
- **Tenancy:** the workspace and the actor come only from the token; the table's key is workspace, file and number, and an unknown argument (a `workspace`) is refused.
- **Postgres:** table `design_review_items` (migration 0006), one row per item; `memory` for a server without a database.

## Considered options

- Items as ops in the file (a node or a field): rejected; they would grow the op log and the storage quota, join the undo and revert of the design, and a critique is not a change of the design.
- Two tools, a Read `design_review_list` and a Change one: kept open; one name is easier for an agent to learn, and the per-action tier is two lines in the tool.
- A server-side batch id: rejected; the keys of an `apply_commands` call already carry one.

## Consequences

- No route or event shows the list to the editor yet: the activity panel (E5.1) needs a read route and a live hint.
- `add` is not idempotent: after a timeout the agent lists before it adds again.
- A `link` writes a `batch_status` entry in the call log besides its own, because it finds the batch through the same read.
- Items cannot be deleted or edited; a retention rule and a delete (Delete tier, approval, E2.4) are follow-ups.
