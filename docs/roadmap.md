# Roadmap

Planning map in the wayfinder shape: a destination, decisions so far, the frontier of open decisions, fog that cannot be specified yet, and what is out of scope. This map plans; it does not build.

## Destination

W1 built and through its gate (owner score 4/5 or better on 3 of 4 real screens), following the milestones and epics in `docs/epics.md`. First goal: milestone M1, backend, basic web editor and MCP connected end to end.

## Notes

- W1 means web editor plus MCP: model round 2, variables, export with oracle, agent review UX, V2 against Wonder.
- Carousel, slides, motion, desktop (ACP) and PPTX stay out until W1 passes its gate.
- Each decision ticket below is resolved with the owner, then recorded in `docs/decisions.md` or an ADR.

## Decisions so far

See `docs/decisions.md` (items 1 to 14 and 20 to 26), the working defaults in `docs/owner-questionnaire.md`, and accepted ADRs 0001, 0002, 0006, 0007, 0008, 0009, 0010.

## Frontier (open decisions, takeable now)

| Ticket | Type | Needs |
|---|---|---|
| Approve the milestones and epics in `docs/epics.md` | review (owner) | owner |
| Approve ADR-0003 fidelity contract and tolerances | grilling (owner) | owner; questionnaire section 2 |
| Approve ADR-0004 variable model | grilling (owner) | owner; questionnaire section 3 |
| Approve ADR-0005 review and revert model | grilling (owner) | owner; questionnaire section 4 |
| Decide headless capture | decision (in the repository, when a real workflow needs it) | no separate spike |
| Confirm Gatehouse client type and scopes for MCP | decision during E2.2 | Gatehouse access |
| Choose the editor UI direction (`DESIGN.md`) | prototype (owner) | draft written in `DESIGN.md` and `docs/ui-design.md`; owner review |
| Define the document model round 2 (components, variants, schema version) | grilling (owner) | after ADR-0004 |
| Whether an undone revert makes its batch revertable again (E5.2; by the last-writer rule the person's undo is a later write, so today the batch has nothing to revert) | decision (owner) | the E5.2 status note in `docs/epics.md` |
| The lane fold rule of the Activity panel: which lane opens by itself (today the first, the most recently active, and the person's choice is kept) | decision (owner) | `docs/collab-ux.md` 4.3 |
| The canvas fit under the right panel (the web client keeps the canvas between the open cards; the Activity tab is 360 wide) | decision (owner) | `docs/ui-design.md` section 2 |
| Checkpoints kept by the server per file (E5.3 open part): the questions of ADR-0023 (restore after a reload, which run starts, the caps, agents over MCP, delete, the replay bound) | decision (owner) | ADR-0023, section Open questions |
| Refused calls in a lane (ADR-0021 slice 17, optional): a read route over the call log for one file, with no content | decision (owner) | ADR-0021 owner question 5 |
| `docs/prd.md` user story 38 says "one lane per agent session"; it should say per actor in a file (owner text, ADR-0021) | edit (owner) | `docs/prd.md` line 81 |
| The real Claude Code run of the M5 exit, with two tokens, before E5.1, E5.2 and E5.3 are ticked | run, then review (owner) | `AGENTS.md` (the Activity panel, operator notes) |

## Not yet specified (fog)

- Component and instance model and overrides.
- Responsive export and breakpoints.
- Brand memory through Uteke: what is stored and how agents read it.
- Mobile app presets beyond frames.
- Desktop shell and local agent (ACP) boundaries.
- Carousel, slides and motion design system.
- Pricing, hosting and billing (deferred by the owner).

## Out of scope

Live multi-human collaboration; HTML import; Figma import; PPTX; Flutter and Dart; marketplace of variables or templates.

## Follow-ups from E1.1 (14 Nov 2026)

Tracked as issues; priorities are labels (`priority:P0` first). The alignment work (issues 108 to 111) comes before the server and client tracks (issue 115).

- Alignment: close-out of E1.1 (108), `meldframe-module` (109), `meldframe-host` (110), the `apps/server` composition root (111), repository hygiene (112), validation of a file at load (113), the core tests under WASM (114).
- Replica and history: undo of a reparent into a deleted parent (107), a pending undo against an earlier remote value (94), matching own ops by idempotency key, resync and an outbox cap (90), the dependency rule in the room (96). `rebalance` is linear in the siblings since 91 (undo and redo of it still replay one `Reorder` per child, so they stay quadratic; the first key past the `pos` limit now refuses the whole call).
- Found while building the alignment (15 Nov 2026): validation of a server snapshot in `Replica::new` (125), splitting `meldframe-history` before it reaches its ceiling (126), a size threshold that relaxes a small delete to Auto (127), an approval path for Ask (128), a snapshot-save failure after an accepted op (129), sealing `Admission` (131), compile-fail tests with trybuild (132), the room registry, activity log and a page size for `ops_after` (133). Handled in the same batch: the build-level omission test (130, done) and the per-worktree target directory (134, with its pull request).
- Owner decisions (116): the GitHub backup mirror and runner capacity are settled by the move to GitHub (decision 33); `Signed-off-by` in squash messages stays required.
- Issue numbers in this section refer to the previous tracker, kept as a backup; they have no direct mapping to GitHub numbers. Only the issues still open were copied to GitHub: each epic is its own issue, and the approval path for Ask (128) is now 19.

## Milestones

M0 Foundation, M1 Connected skeleton (first goal), M2 Platform and agent identity, M3 Model round 2 and variables, M4 Export and fidelity, M5 Agent review and design quality, M6 V2 and the W1 gate. Epics, acceptance criteria and exit criteria: `docs/epics.md`.

### Progress

Counted from the acceptance-criteria boxes in `docs/epics.md` (a box is ticked only when its whole text is proven). State on 18 Nov 2026.

| Milestone | Ticked | Of | Share | What is left |
|---|---|---|---|---|
| M0 Foundation | 15 | 15 | 100% | nothing: E0.4 is ticked since `just web` starts an editor that talks to the server |
| M1 Connected skeleton | 29 | 29 | 100% | nothing open in M1: the demonstration runs with Claude Code as the MCP client in the real browser editor (step 6 keeps its decision column for the M2 call log) |
| M2 Platform and agent identity | 4 | 13 | 31% | E2.3 (every table has its workspace test, quotas survive a restart) and E2.5 assets are in; E2.1, E2.2 and the grant, risk-tier answer and revocation parts of E2.4 wait for Gatehouse; the release rule for development tokens is partial (the override `MELDFRAME_ALLOW_DEV_TOKENS=1` remains) |
| M3 Model round 2 and variables | 12 | 13 | 92% | E3.1, E3.2, E3.3, E3.4 and E3.5 are in except one box: the real Uteke Cloud theme as the import corpus (the owner supplies the file) |
| M4 Export and fidelity | 5 | 6 | 83% | E4.1, E4.2 and E4.3 are in except one box: probes for gradients, variable-bound fields and no fill (E4.1, every M3 property); the oracle is green on pull requests and nightly but not yet a required check (owner action) |
| M5 Agent review and design quality | 3 | 8 | 38% | E5.4 and E5.5 are in; E5.1, E5.2 and E5.3 are built and wait for the real Claude Code run (owner, `m1-claude-demo.mjs` with two tokens); the server-kept checkpoints of ADR-0023 are the open part of E5.3 |
| M6 V2 and the W1 gate | 0 | 4 | 0% | not started |
| Total | 68 | 88 | 77% | |

M0 and M1 are complete. The M1 demonstration runs with Claude Code as the MCP client in the real browser editor (`packages/web-e2e/scripts/m1-claude-demo.mjs`); the backend, MCP surface, live events, capture relay, app layer and the web editor (layers, canvas, properties, tool bar, text editing, actor labels, touch rings, capture) are covered by tests and by the browser checks that are run by hand.

## Earlier build order (superseded by the milestones)

1. E0 Foundation: workspace layout by kind, CI checks (`deps-check`, `build-matrix`, `portable-check` for wasm32, `size-check`, `ui-literals`, `licences`), schema version, protocol pipeline.
2. E1 Core and render model: seed the core; build the render model; model round 2.
3. E2 Variables, E5 Export and oracle (in step, because export proves the model).
4. E3 Agent surface (admission gate, module registry, durable and ephemeral frames), then E4 Editor: the `meldframe-app` layer with the editor shell as the first vertical slice, then review and revert.
5. E6 Platform (Gatehouse, workspaces, quotas) alongside E3.
6. E7 V2 evaluation.
