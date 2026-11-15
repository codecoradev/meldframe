---
status: accepted
---
# The core is several small crates

The file model, its checks, its layout and its sibling order live in separate core crates: `meldframe-order`, `meldframe-check`, `meldframe-layout` and `meldframe-core`, with `meldframe-history` and `meldframe-sync` built on them (and two tool crates that hold their property tests). Each has one reason to change and stays under the 2,000-line crate ceiling with its tests counted. `meldframe-core` re-exports the check API, so callers keep one import path.

## Why

The first vertical slice of the apply path did not fit in one crate: with its tests it was close to the ceiling before decoration and image checks were added. The ceiling only goes down (rule D1), so the fix is a split along real seams, not an allow-list entry.

## Considered options

- One `meldframe-core` crate with an allow-list entry above the ceiling: rejected; the allow-list is for debt that shrinks, and this would grow on every feature.
- Move tests out of the count: rejected; tests are code that has to be read and kept.
- Split by seam (chosen): order depends only on protocol, check depends on order and protocol, core depends on check, order and protocol, layout depends on protocol. Helpers that need `Limits` and `ApplyError`, such as `resolve_pos`, stay in core because moving them into order would create a cycle.

## Consequences

A new crate means a new kind declaration and a place in `docs/architecture.md`; `cargo xtask deps-check` enforces the edges. Callers of the apply path depend on `meldframe-core` only.

The same reasoning applied later to the host: `meldframe-host` (room, store seam, gate) reached its ceiling, so the activity log and the room registry live in `meldframe-rooms`, a core crate that depends on the host.

`meldframe-history` reached its ceiling too, so the bookkeeping of undo and redo (steps, recs, runs, the rules that rewrite them against a remote command) lives in `meldframe-undo`, a core crate that depends only on the protocol. The seam is the tree: `FileState` stays in history and drives the stacks that `meldframe-undo` owns, and history re-exports nothing that callers did not already use.
