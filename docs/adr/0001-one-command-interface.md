---
status: accepted
---
# One Command interface for every change

Every change to a file, whether from the canvas, a WebSocket client, an MCP tool or an undo, is expressed as a `Command` and goes through one validation and apply path in the core. This keeps human and agent changes in one order, makes undo and conflict handling uniform, and means a rule is fixed in one place.

## Considered options

- Separate paths per transport (UI mutations, MCP-specific writes): rejected; the spikes showed behaviour drifting between paths and undo could not cover agent edits.

## Consequences

New capabilities (variables, components) are added as commands, not as side channels. Admission policy still has to be written once in front of the room, because the spike duplicated it per transport.
