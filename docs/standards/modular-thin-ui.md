# Standard: modular architecture with a thin UI

Version 1.3, 25 Nov 2026. Mirror: the source of truth is the CodeCora standard kept in the team knowledge rooms (with its evidence and source pointers). Edit the source first, then update this copy and its version.

Status: adopted for Meldframe (owner decision 13 Nov 2026, ADR-0007 to ADR-0010). Derived from Crastu (primary source, enforced in CI) and Tole (kernel, gate and event patterns). Keywords MUST, SHOULD and MAY are used in the usual sense.

Changes in 1.1: D7 allows replacing spikes with first vertical slices in the product repository (Meldframe does); adoption row and known gaps updated.

Changes in 1.2: D1 sets the crate line count as an ideal band of 1,500 to 2,500 lines with a hard ceiling of 4,000, in place of a flat ceiling of 2,000 (owner decision 20 Nov 2026, recorded in `docs/decisions.md` as decision 35); the file limit of 500 lines is unchanged.

Changes in 1.3 (owner decision 25 Nov 2026, recorded in `docs/decisions.md` as decision 42):
- D1 raises the crate ceiling from 4,000 to 50,000 lines and makes the modular monolith mandatory: one small module per page or capability, shared pieces in reusable modules of their own and never copied, a module directory of small files, and a new page as a new module with its own tests. The ideal band of 1,500 to 2,500 lines stays for a crate with a single seam; the file limit of 500 lines is unchanged.
- New rule U9: text that comes from a model, a server or a file is made safe to display, and capped, where it enters the app layer, in one reusable helper. A count that may be unknown does not show as "nothing waiting".
- D8 notes that an automated review times out on a diff near 2,500 lines.
- The checklists, the anti-patterns and the adoption table follow.

## 1. Purpose and scope

Give every CodeCora product with a user interface and an agent surface the same shape: logic in modules that can be built and tested alone, one thin layer between logic and any screen, one gate in front of every change, and one event model for live clients. Use it at project kickoff and in architecture review. It does not choose languages or UI toolkits.

Use it when the product has a UI and an MCP or API surface, or will get a second front end later (web, desktop, mobile, CLI). Do not use it for single-purpose libraries and scripts.

## 2. Vocabulary

- **Module:** anything with an interface and an implementation (function, crate, slice).
- **Feature module:** a user-visible capability (dictation, variables, export) that can be left out of a build.
- **Core:** shared types, the document or domain model, validation, history. Not detachable.
- **Engine / integration / os:** adapters to models, media, services, operating system.
- **App layer:** UI-neutral state, actions, effects and events. No UI toolkit.
- **Front end:** renders app state and dispatches actions (web UI, GPUI, TUI, mobile, CLI, test harness).
- **Host:** the process that owns modules, storage and jobs.
- **Gate:** the single decision point that authorises a change.
- **Seam, interface, depth, adapter:** as in the `codebase-design` skill. One adapter is a hypothetical seam; two is a real one.

## 3. Rules

### L Layering

- **L1** Every crate MUST declare its kind: `core`, `engine`, `integration`, `os`, `feature`, `app`, `ffi`, `ui`, `feature-ui`, `bin`, `tool`. Feature crates MUST also declare a `group` and, if allowed to use another group, `composite = [..]`.
- **L2** A CI tool MUST enforce this dependency matrix (a crate may depend only on the listed kinds):

| Kind | May depend on |
|---|---|
| core | core |
| engine, integration | core, engine |
| os | core |
| feature | core, engine, integration, os, feature (other group only if `composite`) |
| app | core, engine, integration, os, feature |
| ffi | core, app |
| ui, feature-ui | core, app, ui |
| bin, tool | everything |

- **L3** Domain crates and the app layer MUST NOT depend on a UI toolkit. Only `ui`, `feature-ui` and `bin` crates may.
- **L4** Feature modules MUST NOT import each other, except crates marked `composite`.
- **L5** Portable kinds (core, engine, integration, os, feature, app, ffi) MUST compile for every target platform the product plans, checked in CI (for example `wasm32` for web; iOS and Android where mobile is planned; include 32-bit targets to catch 64-bit assumptions).
- **L6** Operating-system code MUST sit behind a trait with a per-platform adapter and a null adapter for tests.

### M Modules and detachability

- **M1** A module MUST contribute everything through one registration call: commands, settings, job kinds, agent tools, panels or intents.
- **M2** There MUST be one registry. Every front end (UI, CLI, MCP, REST) reads it. Removing a module from a build removes its commands, tools and settings everywhere.
- **M3** "Detachable" means: CI builds the product with each feature alone and with none, and tests pass.
- **M4** Every feature module MUST ship its agent tools, each with a risk tier (Read, Change, Delete). A feature is not done until an agent can use it under the approval policy.
- **M5** Detaching SHOULD have three layers: build time (Cargo features), runtime (disable in settings, lazy load), and later third party (WASM components or command engines).
- **M6** The core is not a module. A design MUST name what is inner and not detachable, and say where a concept is split (for example: bindings in core, import, export and panels in a feature).

### U Thin UI

- **U1** The app layer MUST hold screen state, rules, actions, effects and events as plain serialisable data. A front end does three things only: read state, dispatch actions, carry out effects.
- **U2** UI crates MUST hold no business state and no rules.
- **U3** A complex screen SHOULD split into a `*-model` crate (plain data and button rules) and a `*-ui` crate (drawing only).
- **U4** UI MUST read colours, fonts, sizes and durations from one tokens module. A lint MUST reject literals. A test MUST compare the tokens module with the design file.
- **U5** Replies from host to UI MUST be data. User-visible wording belongs to the UI, so a remote adapter writes no text.
- **U6** Input to the app layer MUST be UI-neutral (a key press with modifiers, a pasted block), not a toolkit event.
- **U7** A pending approval is source-agnostic UI state. A request that is dropped unanswered (turn ended, replaced, UI gone) MUST be treated as denied. The rule "never allow always for a Delete-tier call" lives in one place.
- **U8** The test of thinness: a second front end (a CLI or a test harness) can drive the same state machine without moving logic.
- **U9** Text that comes from a model, a server or a file MUST be made safe to display (control and bidirectional characters removed, markup not interpreted) and capped in length at the point where it enters the app layer, in one reusable helper that every screen shares. A screen MUST NOT clean text again, and MUST NOT show raw text from those sources. A count that cannot be known (a list that failed to load, a request that did not answer) MUST NOT be shown as "nothing waiting" or as zero; it can say Unknown.

### G Gate

- **G1** Every change, from every transport (WebSocket, MCP, REST, UI), MUST pass one authorisation decision point. Policy MUST NOT be re-implemented per transport.
- **G2** The enforcement point SHOULD prove passage with a value only the gate can construct (a permit with a private constructor), so executing a change without the gate does not compile.
- **G3** Calls have risk tiers (Read, Change, Delete). Policy is per tier, and MAY be overridden per agent. Read is always allowed; Change is Ask or Auto; Delete is Ask, Auto or Deny. Deleted data goes to trash first.
- **G4** Agents connect through an OAuth grant through Gatehouse (ADR-0012). Access tokens are short-lived and the host stores no long-lived secret for an agent. A caller without a valid grant is refused, never downgraded.
- **G5** An approval queue entry expires to denied. If policy cannot be read or a row is not understood, only Read calls proceed.
- **G6** Every action is logged with its actor, tool, risk, decision and outcome. Logs carry structured fields, not content, and redaction is a safety net only.

### E Events for live clients

- **E1** Durable frames carry a monotonically increasing sequence number and can be replayed from a cursor (`after=seq`, or `Last-Event-ID`; the explicit parameter wins).
- **E2** Ephemeral frames (progress deltas, touch hints, capture requests) carry no id, are best-effort and are never replayed.
- **E3** Each stream has a bounded, non-blocking queue. When full: drop the frame, send a resync marker, and never slow the producer.
- **E4** The number of concurrent streams is capped and counted in the connection cap.
- **E5** Heartbeats MUST be tested against a real clock. (Tole's first heartbeat never fired because the timer reset on every loop; a pure Heartbeat type plus an end-to-end test fixed it.)

### D Discipline

- **D1** A file stays under 500 lines. A crate (or package) MUST stay under 50,000 lines. The ceiling is a backstop, not a target: the product is a modular monolith, and that is mandatory. Each page or capability is one small module with its own tests and its own directory of small files; a new page is a new module, not more code in an existing one. A piece that two modules need (a control, a helper, a rule) lives in a reusable module of its own and is never copied. A crate with a single seam aims for 1,500 to 2,500 lines; above 2,500 the tool prints a note naming it as a candidate for a split, which is a planned cleanup and never a failure. An allow-list records what is over a ceiling today; an entry equals the current size and only goes down.
- **D2** Everything shipped or downloaded passes a licence gate (code and model weights). New model entries carry a licence id and a commercial flag.
- **D3** No telemetry unless the owner decides otherwise. All outbound requests go through one gate with an Offline switch; refusals name what and the host, never the full URL.
- **D4** `unsafe` is denied workspace-wide. A named crate may opt in with one commented module.
- **D5** Keep a local flight recorder per run (events, performance, panic and fatal-signal reports), with caps and retention, free of secrets.
- **D6** Secrets never appear in arguments, logs or tool output.
- **D7** Design before code. An unknown that could change the architecture is resolved by a time-boxed spike with a written result in `docs/spikes/`, or, when the owner rules spikes out, by building the first vertical slice in the product repository with exit criteria written first (Meldframe, 13 Nov 2026). Phases end on exit criteria, not dates.
- **D8** Pull requests stay small (15 files or fewer, ceiling 20; under 1,000 changed lines, ceiling 1,500; see CONTRIBUTING.md). Use an independent review and CI together; each has found real defects the other missed. An automated review times out on a diff near 2,500 lines, so a pull request that large is split before it is opened, not after the review fails.
- **D9** Status syncs are dated, and code is authoritative over docs. A parked design carries Status, un-park triggers and an anti-goal ("do not build from this note alone").

## 4. Reference structure

```
repo/
├─ apps/            bin: composition roots; Cargo features pick modules
├─ crates/
│  ├─ core/         shared types, module registry, host, store, settings, jobs
│  ├─ engines/      models, media, text, rasterisers (traits + adapters)
│  ├─ integrations/ MCP, memory, sync, auth providers
│  ├─ os/           hotkeys, permissions, crash, process memory (unsafe only here)
│  ├─ features/<g>/ <g> (domain), <g>-ui (view), optional composite
│  ├─ app/          UI-neutral view models: state, actions, effects, events
│  └─ ui/           tokens, components, shell; plain-data *-model beside *-ui
└─ xtask/           deps-check, build-matrix, portable-check, size-check, ui-literals, licences
```

```
apps -> features/*-ui -> ui/* -> toolkit
features/*-ui -> app
app -> features/* (domain) -> engines | integrations | os -> core
```

## 5. Enforcement tooling

Write the layering rules as a pure function over package data so they are unit-testable. Provide these commands and run them in CI and before commits:

| Command | Checks |
|---|---|
| `deps-check` | Kind and group rules (L1 to L4), UI toolkit usage (L3) |
| `build-matrix` | Product builds with no features and with each feature alone (M3) |
| `portable-check` | Portable kinds compile for each planned target (L5) |
| `size-check` | File ceiling of 500 lines and crate ceiling of 50,000 against the allow-list, and a note on crates above the ideal band (D1) |
| `ui-literals` | No colour, font, size or duration literals in UI crates (U4) |
| `licences --check` | Dependency and model licences (D2) |

## 6. Checklists

**Kickoff:** declare kinds; write the matrix; name the core and what is inner; define the registry; define the gate and risk tiers; define the app layer state, actions, effects; pick the event model; add the six commands to CI; write the first slice or spike plan with exit criteria; write milestones with epics, acceptance criteria and exit criteria before building (Meldframe: `docs/epics.md`).

**Is it a module?** Deletion test: delete it; if complexity vanishes it was a pass-through, if it reappears in many callers it earns its keep. Detach test: does the product build and pass tests without it? If not, it is core or it is wrongly coupled.

**Review:** any new path that changes data without the gate? any UI crate with rules or state? any feature importing another? any literal in UI? any host trait with many default-unsupported methods (a sign the seam is too wide)? any seam with one adapter introduced early? any secret in an argument? any new page added to an existing module instead of a module of its own, or any piece copied between modules instead of shared (D1)? any text from a model, a server or a file shown without passing the one helper of U9, or a count that can be unknown shown as zero?

## 7. Anti-patterns seen in this ecosystem

- Admission policy copied per transport (Meldframe spike: WebSocket, MCP and a third partial copy).
- A UI that holds the rules (Meldframe spike: a 787-line canvas component and three document modes in the client).
- A wide host trait with most methods defaulting to unsupported (Tole TUI audit, 10 Nov 2026), later narrowed to three data-only methods.
- Session management hidden in a CLI crate instead of a reusable module (Tole audit; extraction tracked).
- A seam with one adapter introduced before a second consumer exists.
- A heartbeat clock reset on every loop.
- A token passed on a command line.
- A page added to an existing module, or a control or helper copied from one module to another, because the ceiling left room (D1).
- Text from a model, a server or a file cleaned in each screen, or not at all, instead of once where it enters the app layer (U9).
- A count that failed to load drawn as "nothing waiting" (U9).
- A pull request or diff large enough that the automated review times out (D8).

## 8. Adoption

| Project | State |
|---|---|
| Crastu | Full and enforced: kinds, deps-check, build-matrix, mobile-check, size-check, ui-literals, licences, vendor-check |
| Tole | Partial: platform-agnostic kernel, host traits, single gate with permit, durable/ephemeral SSE; no crate-kind lint; app layer extraction tracked (epic #523) |
| Meldframe | Adopted 13 Nov 2026 as ADR-0007 to ADR-0010 (layers and registry; UI-neutral app layer built directly in the repository, starting with the editor shell, no separate spike; durable and ephemeral events; single admission gate). Version 1.3 applies from 25 Nov 2026 (decision 42, ADR-0026): the crate ceiling is 50,000 lines, `cargo xtask size-check` enforces it, and the thin-client audit of the web client against U1, U2 and U9 is tracked in #307 |
| Others | Not assessed |

## 9. Known gaps and exceptions

- Web front ends are written in TypeScript. Whether the app layer is Rust compiled to WASM or TypeScript is a decision per project (Meldframe: Rust and WASM, revisited if the first slice misses its criteria).
- This standard comes from desktop and CLI products. Where a rule fits a web service differently, record the deviation as an ADR.
- Discord's approach (shared Rust core, thin native UI per platform) is reported by secondary sources only.

## 10. Changing this standard

A change needs an owner decision recorded in the team knowledge rooms, next to the standard. Bump the version and note what changed and why. Projects record their own deviations in their ADRs.
