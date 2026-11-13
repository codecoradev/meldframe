---
status: proposed
---
# Flat variable model, resolved in the core

Variables are organised as collections with modes, variables and aliases. Primitive, semantic and component tiers are a naming and lint convention, not model concepts. Variables have stable opaque ids; names are slash paths. A node property is either a literal or a binding to a variable. Resolution is one function in the core, used by layout, canvas, export and tests; the canvas never uses CSS variables itself. Export keeps aliases as `var(--...)`; Tailwind v4 output uses `@theme`.

## Considered options

- Three-tier model in the data (as in some design tools): rejected for now; more model for no tested benefit.
- Store names as identity: rejected; renames would rewrite every binding.
- Run JavaScript Tailwind configs on the server for import: rejected; no user code executes on the server.

## Consequences

Colour validation changes from "hex only" to "hex or binding". Component tokens and workspace-level variables shared across files are deferred. Details: `docs/tokens-and-variables.md`.
