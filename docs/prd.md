---
title: "PRD: Meldframe"
status: draft for owner review
created: 2026-11-13
inputs: ["docs/decisions.md", "docs/concept.md", "PRODUCT.md", "GLOSSARY.md", "spike docs (rust-core-spikes)", "docs/architecture/spike-survey.md", "docs/collab-ux.md", "docs/tokens-and-variables.md", "docs/fidelity-contract.md"]
method: "to-spec template (problem, solution, stories, implementation, testing, out of scope) plus numbered FR/NFR and journeys, following docs/prd.md of gatehouse"
---

# PRD: Meldframe

Vocabulary is defined in `GLOSSARY.md`. Status of each requirement: **P0** = first release (W1), **P1** = after W1, **Later** = not scheduled. **Proven** = demonstrated by a spike, **New** = needs design.

## 1. Problem Statement

A developer or designer working with an AI agent can already get a design out of the agent, but the result is unreliable in three ways. The agent cannot see or safely edit a real design file (calls fail, edits clobber each other, nothing can be undone per actor). The design system the agent is told to use does not survive: values end up as literals on the canvas and in the exported code. And the exported code does not match what the canvas shows, so the human cannot trust either one. The owner saw all three while designing Uteke Cloud with Wonder: the agent reported that `var(--...)` did not render on the canvas and fell back to literals.

## 2. Solution

Meldframe is a web design canvas with an MCP server. An agent connects, reads the file, and edits it through tools that are idempotent, can be atomic and return structured errors. Every agent change appears on the canvas live, labelled by actor, and the human can review the batch and keep it or revert it. Variables from the user's own design system (imported from a Tailwind config or CSS variables) are bound to nodes, rendered on the canvas, and exported as CSS variables. The export is a translation of the canvas geometry and is checked against a real browser in CI, to a stated tolerance.

## 3. Personas and journeys

**P1 Owner (dogfood).** Designs Uteke Cloud pages, Uteke Mobile screens and marketing frames with Claude Code, then refines by hand.
**P2 Agent developer.** Uses Claude Code, Cursor or Codex, owns a Tailwind project, wants the agent to design screens that already use the project's variables, and takes HTML/Tailwind out.

- **UJ-1. First screen from an agent.** P2 creates a workspace, connects Claude Code to the MCP endpoint with a Gatehouse login, the agent reads the file and builds a landing hero in one atomic batch; the canvas updates live; the agent captures the frame, notices an overflow and fixes it in a second batch.
- **UJ-2. Take over mid-run.** While the agent is working, the human nudges a heading by hand; the agent's later edit of the same node is rejected with a conflict that says what changed; the agent reads again and continues. Nothing is lost.
- **UJ-3. Review and revert.** The agent finishes a run of batches. The human opens the activity panel, sees each batch with its note, inspects before and after, reverts one batch; only fields still last written by that agent revert, as one undoable change.
- **UJ-4. Bring my design system.** P2 imports a Tailwind config; variables, modes and aliases appear in the Variables panel with an import report; the canvas shows them rendered; the agent is told the variable names and binds nodes to them.
- **UJ-5. Export and trust it.** P2 exports HTML/Tailwind with CSS variables; the output renders the same boxes as the canvas within the contract tolerance; the CI oracle proves it for the fixtures.
- **UJ-6. Compare takes.** The agent or human duplicates a frame as a variant; both stay in the file; a design review list ties numbered critiques to the batches that fix them.

## 4. User stories

Workspace and files
1. As a developer, I want to sign in with my existing account, so that I do not create another password.
2. As a developer, I want a workspace that holds my files and variables, so that my design system is shared by all files in it.
3. As a developer, I want to create, rename, duplicate and delete files, so that I can organise work.
4. As a developer, I want a file to have pages and frames from presets, so that I start from real screen sizes.
5. As a developer, I want to see which files my agent session is bound to, so that I know where it can write.
6. As a workspace owner, I want quotas per workspace, so that one agent run cannot exhaust shared resources.

Canvas editing
7. As a designer, I want to draw frames, text, shapes and images, so that I can build screens by hand.
8. As a designer, I want auto layout (direction, gap, padding, alignment, hug and fill), so that layouts adapt like the code will.
9. As a designer, I want margin and per-side padding, so that spacing matches the CSS I will ship.
10. As a designer, I want fills, gradients, opacity, borders, shadows and radius, so that I can style nodes fully.
11. As a designer, I want text with font size, weight, alignment and wrapping, so that typography is real.
12. As a designer, I want to place an image from an upload, so that mock-ups use real pictures.
13. As a designer, I want absolute children inside auto layout, so that badges and overlays work.
14. As a designer, I want semantic layer names to be encouraged, so that the exported code and the agent's reads are readable.
15. As a designer, I want undo and redo that only touch my own changes, so that an agent's work does not vanish when I press undo.
16. As a designer, I want zoom, pan and fast handling of very tall pages, so that long landing pages stay usable.

Variables
17. As a developer, I want to import variables from a Tailwind config or CSS variables, so that my real design system is the starting point.
18. As a developer, I want an import report listing what was mapped and what was not, so that I can trust the result.
19. As a designer, I want collections with modes such as light and dark, so that I can preview both.
20. As a designer, I want to bind a node property to a variable, so that changing the variable changes every use.
21. As a designer, I want aliases between variables, so that semantic roles point at primitives.
22. As a designer, I want to detach a binding into a literal on purpose, so that I can break away deliberately.
23. As a designer, I want a clear error when a variable is missing or an alias loops, so that nothing silently renders wrong.
24. As a developer, I want the canvas to render variables, not literals, so that the canvas shows what the code will show.

Agent surface (MCP)
25. As an agent, I want tools to read the file tree and node details, so that I understand the design before editing.
26. As an agent, I want to apply commands with an idempotency key, so that a retry does not duplicate work.
27. As an agent, I want to apply a batch atomically, so that a failed step leaves nothing half done.
28. As an agent, I want `ifSeq` on edits of existing nodes, so that I do not overwrite something I have not seen.
29. As an agent, I want structured errors with stable codes and English messages, so that I can recover without guessing.
30. As an agent, I want to learn limits (batch size, quota) from one tool, so that I plan within them.
31. As an agent, I want to capture a node or frame as an image, so that I can check my own work.
32. As an agent, I want to attach a short note to each batch, so that the human knows my intent.
33. As an agent, I want to list recent batches and who wrote a field last, so that I can see what changed since I looked.
34. As an agent, I want to revert my own recent batch, so that I can back out of a bad step.
35. As an agent, I want a design review tool and checklist, so that iteration follows numbered critiques.
36. As an agent, I want to import variables from text I read locally, so that no code executes on the server.

Collaboration and review
37. As a human, I want agent changes to appear live and be labelled by actor, so that I always know who did what.
38. As a human, I want an activity panel with one lane per agent session, so that I can follow several agents.
39. As a human, I want to see which nodes an agent is touching, so that I avoid colliding with it.
40. As a human, I want to review an agent's batches and keep or revert each, so that I stay in control without staging.
41. As a human, I want a revert to be a new change that I can undo, so that history is never rewritten.
42. As a human, I want automatic checkpoints at the start of an agent run and before a revert, so that I can restore a known state.
43. As a human, I want to stop an agent by revoking its session, so that a runaway agent ends at once.
44. As a human, I want conflicts explained in plain words, so that I know why an edit was rejected.

Export and output
45. As a developer, I want HTML/Tailwind and CSS variables exported, so that I can paste or commit the result.
46. As a developer, I want Tailwind v4 `@theme` output, so that it matches modern projects.
47. As a developer, I want PNG and PDF export per frame, so that I can share designs.
48. As a developer, I want the export to match the canvas within a stated tolerance, so that I can rely on it.
49. As a developer, I want unsupported properties to be reported, not dropped silently, so that I know what to fix.
50. As a developer, I want images referenced from assets in a stable way, so that exports can be reproduced.

Operations and trust
51. As a workspace owner, I want agents to authenticate through the same login as people, so that access is revocable in one place.
52. As a workspace owner, I want every change attributed in an audit trail, so that I can answer who changed what.
53. As an operator, I want rate limits before parsing and per-workspace quotas, so that abuse is cheap to stop.
54. As an operator, I want uploads limited to safe image types, so that user content cannot execute.
55. As the owner, I want the V2 comparison against Wonder run on real screens, so that quality is judged by evidence.

## 5. Functional requirements

| ID | Requirement | Priority | State |
|---|---|---|---|
| FR-1 | Sign in through Gatehouse (OIDC); workspaces as tenants; MCP uses OAuth through the same provider | P0 | New |
| FR-2 | Files, pages, frames from presets (Desktop, Tablet, Mobile) | P0 | New (frames Proven) |
| FR-3 | Single `Command` path for UI, WebSocket, MCP and undo, with one total order per file | P0 | Proven |
| FR-4 | Auto layout: direction, gap, alignment, hug, fill, fixed; padding per side; margin; absolute children | P0 | Partly proven (no margin, per-side padding, absolute children) |
| FR-5 | Paint: solid fill, gradient, opacity, no-fill, border, shadows, radius, image fill | P0 | Partly proven (no gradient, opacity, no-fill) |
| FR-6 | Text: size, weight, alignment, wrapping, measured in a browser table | P0 | Proven |
| FR-7 | Per-actor selective undo and redo under concurrency | P0 | Proven |
| FR-8 | Idempotent ops, atomic batches, `ifSeq`, positive op dependencies, structured errors | P0 | Proven |
| FR-9 | Variables: collections, modes, aliases, binding, resolution in the core | P0 | New |
| FR-10 | Variables rendered on the canvas | P0 | New |
| FR-11 | Import from Tailwind config and CSS variables, with an import report | P0 | New |
| FR-12 | Export HTML/Tailwind with CSS variables (Tailwind v4 first) | P0 | Partly proven (spike export lacks M3 properties) |
| FR-13 | Fidelity contract and a browser oracle as a CI gate | P0 | Proven as experiment |
| FR-14 | MCP tools: read, apply, capture, whoami/limits, batch status, assets, design review | P0 | Proven (design review New) |
| FR-15 | Capture through the open editor (relay) | P0 | Proven |
| FR-16 | Actor profiles, per-batch notes, query of changes and last writer | P0 | New |
| FR-17 | Activity panel, attribution in layers and history, review and revert of batches | P0 | New |
| FR-18 | Checkpoints with restore | P1 | New |
| FR-19 | Asset upload on local disk, content-addressed, safe image types only | P0 | Proven (Postgres-backed in spike) |
| FR-20 | Quotas per workspace, rate limit before parsing | P0 | Proven per actor; New per workspace |
| FR-21 | Layer-name lint, frame duplication as variant | P1 | New |
| FR-22 | PNG and PDF export per frame | P1 | Partly proven |
| FR-23 | Headless capture (render without an open editor) | P1 | Open (decision 18) |
| FR-24 | Brand memory through Uteke | P1 | New (optional) |
| FR-25 | Components and instances with overrides | P1 | New (decision 19) |
| FR-26 | Carousel, slides, motion | Later | Not scheduled |
| FR-27 | Feature modules register commands, settings, jobs, agent tools (with risk tiers) and panels in one registry read by editor, MCP and REST | P0 | New |
| FR-28 | Risk tiers (Read, Change, Delete) with policy per tier and per agent; agents connect through an OAuth grant, revocable at once (decision 31, ADR-0012) | P0 | Partly proven (session tokens, quotas) |
| FR-29 | Durable and ephemeral live frames with replay from a cursor and a resync marker | P0 | Partly proven (ops with sequence numbers; ephemeral frames New) |

## 6. Non-functional requirements

| ID | Requirement | Basis |
|---|---|---|
| NFR-1 | Layout of 5,000 nodes recomputes in single-digit milliseconds in the browser | Spike measurement, to be re-measured on the product model |
| NFR-2 | Export geometry within 0.05 px of the core; text ink within 1 px; hard fail line 1 px | `docs/fidelity-contract.md` |
| NFR-3 | All clients converge to the same document under any order of delivery; property and mutation tests hold | Spike property tests (600 to 10,000 seeds) |
| NFR-4 | An agent retry with the same key never duplicates a change; a rejected op never writes | Spike server tests |
| NFR-5 | Rate limiting happens before parsing; quotas are per workspace | H1 hardening |
| NFR-6 | No credentials in process arguments or logs; session tokens are short-lived and revocable | S15 |
| NFR-7 | Uploads accept only png, jpeg, webp and gif; SVG is rejected until sanitisation exists | M3B |
| NFR-8 | Every error shown to a human or agent has a stable code and an English message | Q1, `docs/collab-ux.md` |
| NFR-9 | Editor UI meets WCAG 2.2 AA for focus, contrast and keyboard operation of panels | Proposed |
| NFR-10 | Every change is attributed to an actor and kept in an append-only log | Spike design |
| NFR-11 | CI builds the product with each feature module alone and with none; layering rules are enforced by a checker | ADR-0007 |
| NFR-12 | Every change from every transport passes one admission gate; a path that skips it does not compile | ADR-0010 |
| NFR-13 | UI crates hold no business state; colours, fonts and sizes come from tokens with a literal lint | ADR-0008 |
| NFR-14 | A slow or disconnected client never slows the room; durable frames replay from a cursor, ephemeral frames do not | ADR-0009 |

## 7. Implementation decisions

Modules (names are from `docs/architecture.md`; no file paths on purpose):
- **Document core** (deep): holds nodes, variables, history, validation, fractional order, layout and the render-model resolution. One small interface: submit a command, read resolved state. Shared by server and WASM client.
- **Render model**: a single resolution step from nodes and variables to painted boxes, consumed by the canvas and by export. It does not exist in the spike and is the first thing to build.
- **Admission gate and room**: one gate (authorise, risk tier, quota, timeout, error mapping) in front of the room actor, used by WebSocket, MCP and REST alike; the room accepts a write only with a permit the gate creates.
- **Module registry**: feature modules register commands, settings, jobs, agent tools and panels; the editor, MCP and REST read it.
- **App layer** (`meldframe-app`, compiled to WASM): state, actions, effects and events as plain data; the web client only renders it.
- **Agent surface**: MCP tools as a thin adapter over admission; tool text is English and is part of the contract.
- **Capture**: relay through the open editor first; a second adapter only when decision 18 is made.
- **Export**: consumes the render model; emits HTML, Tailwind and CSS variables; reports unsupported properties.
- **Persistence**: Postgres; documents carry a schema version from the first release; assets on local disk behind a blob interface.
- **Web client**: Svelte 5 + PixiJS v8 as a view of the app layer; one connected mode; panels for layers, properties, variables and agent activity; tokens only, no literals.

Decisions recorded as ADRs (status in each file): one Command seam, Rust core shared by server and WASM, geometry source of truth and oracle, variable model, live apply with post-hoc review, workspace tenancy with Gatehouse, layered detachable modules, UI-neutral app layer, durable and ephemeral events, one admission gate.

## 8. Testing decisions

- Test external behaviour only. The highest seam is the **submit-a-command** interface of the document core and, above it, the MCP tool surface exercised by a real MCP client against a real Postgres. Prefer these two over new seams.
- Properties first for the core: convergence, undo, rebase under random concurrent edits, with a mutation check that disabling a fix fails a test.
- The fidelity oracle renders exports in Chromium and compares boxes, text ink and paint probes with the core; each new model property enters the supported table together with a fixture, a probe and a place in the fuzz generator.
- Browser end-to-end tests cover the editor with a real WebSocket, including a dropped connection.
- A build matrix proves each feature module builds and tests alone and with none; a second driver (a test harness) exercises the app layer without the web client.
- No separate spikes: unknowns are resolved by building the first vertical slice in the repository, with exit criteria written first.
- Prior art: the spike's property harness, server integration suite and oracle harness carry over.
- Agent quality is measured by V2: blind comparison on four real screens, owner score of 4/5 or better on three.

## 9. Out of scope

Live multi-human collaboration (presence, comments, multi-editor); HTML import; Figma import; repo sync; desktop shell and local agent runtime (ACP); carousel, slides and motion; PPTX; Flutter and Dart; pricing, billing and hosting design; responsive export with breakpoints.

## 10. Further notes

- Wonder is the comparison point for V2. Claims about Wonder in this repo come from the owner's own use and public research; none were verified against Wonder's code.
- Open questions that block parts of this PRD are collected in `docs/owner-questionnaire.md`.
- Language: English for all repository artifacts.
