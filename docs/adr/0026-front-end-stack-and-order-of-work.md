---
status: accepted
---
# Front end stack and order of work: a thin UI first, React for the web, GPUIX with React for the desktop after the web client is fixed

Accepted by owner decision on 25 Nov 2026 (decision 42 in `docs/decisions.md`; tracking issues #305 to #309). The date of the decision is the date of this record; nothing here was proposed first.

## Context

The web client is Svelte 5 with Bits UI (ADR-0008, decision 14). Decision 5 planned the desktop client as Tauri 2 with a local agent over ACP, after the web client is stable. Four things came together:

- The web chrome has grown to about 4,800 lines of Svelte (about 2,100 on 20 Nov 2026), beside about 10,000 lines of framework-free TypeScript (canvas, document interaction, transport, tokens). The rules U1 to U9 of the standard (`docs/standards/modular-thin-ui.md`) are enforced by tools for the Rust crates only; the web packages keep to them by convention, and nobody has audited whether a rule of the editor lives in them instead of in `meldframe-app`.
- The owner wants the ecosystem of React, and a desktop client on GPUIX with React, as Kybern does.
- Standard v1.3 (21 Nov 2026) raises the crate ceiling and makes the modular monolith mandatory.
- A spike on 20 Nov 2026 with stock GPUIX 0.11 gave facts about the desktop renderer, listed under Risks. It used its own scene, not Meldframe's fixtures.

## Decision

The owner decided four things on 25 Nov 2026:

1. **Direction A: the architecture discipline of the standard comes first, and the UI is thin, enforced by tools.** Every rule and every piece of state of the editor lives in portable Rust (`meldframe-app` and the core). A front end reads state, dispatches actions and carries out effects. Native Rust GPUI views, as Crastu has them (direction B), were not chosen.
2. **The web client moves from Svelte to React.** The reasons are the ecosystem and the desktop client of point 3.
3. **The desktop client is GPUIX + React, after the web client is fixed.** This supersedes decision 5 (Tauri 2) for the desktop. The rest of decision 5 stands: the web is purely visual, and a local agent over ACP belongs to the desktop phase.
4. **Standard v1.3 applies now, and the modular monolith is mandatory.** The hard ceiling of a crate or package is 50,000 lines (it was 4,000; the file limit of 500 is unchanged), and a crate that large is one small module per page or capability, shared pieces in reusable modules, small files, a new page as a new module with its own tests. `cargo xtask size-check` enforces the ceiling; the mirror is `docs/standards/modular-thin-ui.md` version 1.3.

## What stays

- The Rust app layer `meldframe-app`, its serialisable state, actions, effects and events (ADR-0008), and the WASM bridge `meldframe-wasm`.
- The framework-free TypeScript: canvas, paint, document interaction, transport and the tokens module.
- Tailwind CSS v4 and the tokens, with the rule that colours, font sizes and durations come from the tokens module (U4).
- The browser scripts under `packages/*/scripts`, which are the parity check of the migration.
- The gate, the registry and the events (ADR-0007, ADR-0009, ADR-0010).

## What changes

- The `.svelte` files (the shell, panels, dialogs and chrome in `packages/web-chrome`, `web-panels`, `web-shell`, `web-variables`, `web-variables-io`, `web-activity`, `web-export`, `web-feedback` and `apps/web`) become React components, panel by panel.
- Bits UI is replaced by a component library that the plan of #308 chooses. It is not chosen here.
- The `ui-literals` scan reads `.tsx` as it reads `.svelte` and `.ts` today (`cargo xtask ui-literals` and `npm run ui-literals`), and the Svelte checks, lint and component tests give way to those of React.
- React reads the state of the app layer through the bridge with a subscription (`useSyncExternalStore`) and holds no editor state.
- Desktop: GPUIX with React replaces Tauri 2. Decision 5 and the desktop paragraphs of the roadmap are marked accordingly.
- The web packages get an enforced boundary check, the TypeScript counterpart of `deps-check` (design in #307).

## Consequences

- The thin UI audit comes first because it names the surface that React has to render, so that no rule is rewritten twice. Rules found in the TypeScript move into `meldframe-app` with a failing test at the app layer first.
- The migration has no freeze of the editor. A browser script passes on the React version of a panel before the Svelte version is deleted, and no Svelte is left at the end.
- One shared helper makes text from a model, a server or a file safe to display, at the point it enters the app layer (U9).
- The desktop client can share the Rust app layer, the tokens and the React components that do not need the DOM. It cannot share Pixi or components that need the DOM. What is shared is decided in the spike plan.
- Crates and packages are no longer forced to split by a ceiling of 4,000 lines. The splits that exist stay; new work follows the modular monolith rule instead of filling a crate.

## Risks

Known facts about GPUIX and GPUI from the spike of 20 Nov 2026 (stock GPUIX 0.11 on Linux with software rendering, an own scene of 18 items):

- GPUI lays text out in whole pixels, and it wrapped differently in 2 of 12 widths, against a geometry contract of 0.05 px (`docs/fidelity-contract.md`, ADR-0003).
- The upstream addon does not clip to rounded corners. Kybern keeps its own forks for that and for backdrop blur, so Kybern runs on forks of gpuix and zed, and its builds for Linux and Windows are beta.
- There are no transforms, so pan and zoom rewrite every element: 2,000 elements took 4.5 ms and 8,000 took 26 ms a batch.
- `@gpuix/react` is official and pre-1.0.
- Not tested: macOS, Windows, a real GPU, IME text input, virtual lists.

The web migration costs a rewrite of the chrome for no change in what a person sees, and a second set of component tests. The desktop path depends on forks that this project does not control. A React web client does not by itself give a desktop client: the renderer decides.

## What is not decided

- The component library that replaces Bits UI.
- The shape of the migration (a rewrite on a branch, a strangler by panel with React islands in the Svelte shell or the reverse, or a new app directory with a parity list) and how the Pixi canvas is mounted. The plan of #308 chooses.
- Any commitment to build the desktop client. It waits for the spike with written exit criteria, written before any code, against the contract of `docs/fidelity-contract.md` (boxes within 0.05 px and identical line breaks, text ink within 1 px, rounded clip and shadows within the contract's limits, pan and zoom within a frame budget at a stated node count), and for the plan of what the desktop shares with the web client and which forks, pins and runtime it needs.
- Whether the desktop canvas paints from the core's own text measure and line breaks (the `TextMeasurer` seam) rather than from GPUI's text layout. The spike result says.
- The form of the check that makes the modular monolith rule checkable (#306 holds the design if it needs one).

## Order of work

1. The thin UI audit and the boundary check for the web packages (#307).
2. The React plan and a pilot, then the migration panel by panel (#308), after the audit.
3. The desktop spike, its exit criteria first (#309), after the web client is fixed. The exit criteria and the plan can be written earlier.

Standard v1.3 (#306) is adopted at the start of this order, and the epic that holds all of it is #305 (`docs/epics.md`, E7).
