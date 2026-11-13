---
status: accepted
---
# Layered crates with enforced dependency rules, and detachable feature modules

Every crate declares a kind (core, engine, integration, os, feature, app, ffi, ui, feature-ui, bin, tool). A CI tool enforces which kinds may depend on which, forbids feature modules from importing each other except crates marked composite, keeps UI toolkits out of everything except ui and bin crates, and builds the product with each feature alone and with none. Features register their commands, settings, jobs, agent tools and panels in one registry that the editor, MCP and REST all read, so leaving a feature out of a build removes it everywhere. The rules and rationale are the CodeCora standard `modular-thin-ui` (`docs/standards/modular-thin-ui.md`).

The inner core is not detachable: the document model, the `Command` interface and the render model. Variables are split: bindings live in the core; import, export and the panel are a feature.

## Considered options

- Few crates, internal modules only (the spike survey's first layout): rejected for features; it cannot be checked by CI and nothing proves a feature can be left out. Server internals may stay modules inside one crate until a second binary needs them.
- Convention without a checker: rejected; the spike drifted without one.

## Consequences

CI work moves into E0: the checker, the per-feature build matrix, a portable-build check (wasm32; iOS and Android only if mobile is planned), file and crate size ceilings, a UI literal lint and the licence gate. The layout in `docs/architecture.md` replaces the layout suggested in the survey.
