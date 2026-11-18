---
name: meldframe
description: How to design in a Meldframe file through its MCP tools without breaking a person's work. Use it whenever you read or change a Meldframe file: read first, change in small top-down atomic batches guarded by ifSeq, name every layer, check after a timeout, and look at the result.
---

# Working in a Meldframe file

A Meldframe file is shared: a person may be editing it in the browser while you work, and every change you make is
shown to them as it lands. Work so that each step is small, safe to repeat, and easy for the person to follow.

## The loop

1. **Know your limits.** Call `whoami` once per session: it says your workspace and actor, the batch limit (100
   commands per call) and the limits of a file.
2. **Read before you write.** Call `get_tree` on the file. Keep its `seq`: you pass it as `ifSeq` with your next
   change. Read the full data of the nodes you will change with `get_nodes`. Read the `warnings` too: default layer
   names and layout problems (`overflow`, `fill_under_hug`, `zero_fill`, `smaller_than_padding`) are yours to fix.
3. **Change in small, top-down batches.** Call `apply_commands` with one logical step at a time: a section frame, then
   its children, then their content. A batch that adds a frame and its children in one call is fine; a batch that
   rebuilds a whole page is not. Small batches show the person what you are doing and are cheap to undo.
4. **Keep each call atomic.** Leave `atomic` at its default (true) for a call of several commands: it is checked as a
   whole, and if one command would fail, none is written and the error names it by index. Fix that command and send
   the call again. In two rare cases (the store fails part way, or `ifSeq` is older than the snapshot the file was
   opened from) the first commands can still be written: check with `batch_status` and send the same call again with
   the same key.
5. **Guard edits of existing nodes with `ifSeq`.** Pass the `seq` of your last read. If the person (or another agent)
   changed a node your call touches since then, the call is refused with `conflict` and the error lists what changed;
   read again and redo your change on what is there now, instead of overwriting their work. An `ifSeq` more than 2000
   changes behind the file is refused as `invalid`: read again first.
6. **Give every call an `idempotencyKey`.** Use a new key for each new call and the same key when you send the same
   call again after a timeout: nothing is written twice. If you do not know whether a call landed, ask
   `batch_status` with its key (and `count`, the number of commands it had) before you send it again.
7. **Look at the result.** Call `capture` with the `file` and the `nodeId` of the frame you changed and check the image
   against what you meant; it needs an editor open on the file. Your session is bound to the first file you capture:
   capturing another file later is refused as `room_mismatch`. Then `get_tree` again: the new `seq` and any new
   warnings.

## Names

Give every layer a semantic name that says what it is: "Hero heading", "Pricing card", "Primary button label". A
default name ("Frame 3", "Rectangle", "Text", the node's id) does not fail a call, but it comes back under `warnings`
and makes the layers panel useless to the person. Rename such layers when you see them.

## Before a large rewrite

Do not rewrite a frame the person made in place. Keep the original and build your take as a variant beside it, so the
two can be compared: a new frame next to the original, made with `addNode` commands (top-down, as above), with a name
that says what it is. A frame delete needs the owner's approval and is refused as `forbidden`, so leave the original
for the person to keep or remove.

## When a call is refused

Read the error code; it says what to fix (`docs/error-codes.md` lists them all):

- `conflict`: someone changed a node you touch since your read. Read again and redo your change.
- `quota_ops_per_minute`: you sent too many changes this minute. Wait for the time in the reply, then send the same
  call again; the refused call was not counted.
- `batch_too_large`: split the call into calls of at most the limit in the message.
- `forbidden`: the policy refused the call. Most often a delete above what you may do alone (a frame, or a large
  subtree): ask the person. It is also what a `removeNode` of an id that is not in the file gets: read the tree again
  and check the id.
- An `E_` code: one command is invalid (an unknown node, a value out of range); the error names the command by index.
