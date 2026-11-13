---
status: accepted
---
# UI-neutral app layer in Rust (compiled to WASM); front ends only render

Editor behaviour lives in a `meldframe-app` layer: state, actions, effects and events as plain serialisable data, with no UI toolkit. The web client (Svelte and PixiJS) reads state, dispatches actions and carries out effects. Selection, tools, panel state, the agent activity panel, review and revert, and the variables panel keep their rules here, so they are written and tested once and a later desktop or mobile front end reuses them. Complex screens split into a plain-data model and a view that only draws.

The owner accepted this on 13 Nov 2026 and ruled out separate spikes: the layer is built directly in the product repository, starting with one real vertical slice (the agent activity panel), with exit criteria written before the work starts.

## Considered options

- Keep the rules in the Svelte client: rejected; that is how the spike client grew a 787-line canvas component and three document modes.
- A TypeScript app layer: kept as the fallback if the WASM boundary proves too costly in the first slice.

## Consequences

Every UI state change crosses the WASM boundary, so state is exchanged as coarse snapshots plus events. If the first slice shows the cost is not acceptable, this ADR is revisited before more panels are built.
