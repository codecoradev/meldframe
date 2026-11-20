# Design boards and epics

Which board of the design canvas "Meldframe UI/UX" shows which epic of `docs/epics.md`, and what each board does not yet cover. Board IDs are the ones in the canvas titles (M0.1 and so on, D.n for desktop, P.n for parked). A board is a picture of intent; the acceptance criteria of the epic stay the test. `docs/ui-design.md` holds the layout decisions and `docs/design-qa.md` the check of the boards themselves.

The "Shows" and "Not on a board yet" columns come from the board titles and the first design pass, not from a line-by-line comparison with each epic; the first step before an epic is to confirm them.

How to use it: before an epic that has a UI part starts, open its boards, list what the boards show beyond the criteria (and send that to the owner) and what the criteria need that no board shows (and draw it first).

## M0 Foundation

| Board | Epic | Shows | Not on a board yet |
|---|---|---|---|
| M0.1 Foundations and components | E1.6 (tokens criterion), E0.2 | colour roles, type, space, base components | none; values live in `DESIGN.md` |
| M0.2 Accessibility and keyboard | E1.6 | focus order, keyboard map, reduced motion | the real tab order (check once built) |
| M0.3 Editor, light appearance | E1.6 | the editor in the light appearance | owner review of the light values |
| M0.4 Component state matrix | E1.6 | six states of nine controls | hover and pressed values are proposals, not in `DESIGN.md` |

## M1 Connected skeleton

| Board | Epic | Shows | Not on a board yet |
|---|---|---|---|
| M1.1 Editor, panels closed | E1.6 | canvas as the whole window | none |
| M1.2 Editor, panels open | E1.6 | layers, canvas, right panel; agent change labelled by actor | touch ring on an agent's node is drawn only in M1.4 |
| M1.3 Properties in full | E1.6 | basic properties of a frame, rectangle and text | none |
| M1.4 Canvas interactions | E1.6 | move, resize, edit text, selection, touch ring | none |
| M1.5 New file and frames | E1.6, E3.2 (frame presets, done) | new file, Desktop, Desktop HD, Tablet, Mobile, Phone small, A4, Letter presets | Carousel, Story, Slide presets (later) |
| M1.6 Files home | E1.2, E1.6 | file list, empty state, new file | sharing a file with an agent |
| M1.7 Shortcuts and palette | E1.6 | shortcut sheet, command palette | none |
| M1.8 Connection and errors | E1.4, E1.6 | connection bar, agent messages per error code, empty states, dialogs | the `Halted` reload flow of issue 90 |

## M2 Platform and agent identity

| Board | Epic | Shows | Not on a board yet |
|---|---|---|---|
| M2.1 First-run flow | E2.1, E2.2, E2.4 | sign in, workspace, authorize the computer, connect an agent (desktop ACP, web MCP) | none; OAuth only (decision 31, ADR-0012) |
| M2.2 Workspace settings | E2.3 | members, quotas | quota-exceeded messages beyond the state rows |
| M2.3 Agents settings | E2.4 | grants, tiers, policy per tier, revoke | the consent screen itself is drawn only as a step of M2.1 |
| M2.4 Preferences | E1.6 | appearance, shortcuts, panels | none |
| M2.5 Error pages | E2.1 | sign-in and access errors | none |
| M2.6 Assets and images | E2.5 | upload, image fills, missing asset | none |

## M3 Model round 2 and variables

| Board | Epic | Shows | Not on a board yet |
|---|---|---|---|
| M3.1 Variable modes | E3.3 | collections, modes, light and dark switch | none |
| M3.2 Inspector and Variables states | E3.4 | binding in the inspector, missing and unbound states | none |
| M3.3 Variables table | E3.4 | the Variables panel: list, search, edit | none |
| M3.4 Import variables | E3.4 | import report from CSS and Tailwind text | `import_tokens` agent call (no UI) |
| M3.5 Manage variables | E3.3, E3.4 | rename, move, delete, aliases | none |
| M3.6 Fill and layout round 2 | E3.2 | fills, strokes, effects, auto layout | none |

## M4 Export and fidelity

| Board | Epic | Shows | Not on a board yet |
|---|---|---|---|
| M4.1 Export a frame | E4.1, E4.3 | HTML and Tailwind export, unsupported-property report, PNG | none |
| M4.2 Export variables | E4.1 | CSS variables output | none |

E4.2 (the oracle gate) has no UI.

## M5 Agent review and design quality

| Board | Epic | Shows | Not on a board yet |
|---|---|---|---|
| M5.1 Activity and review | E5.1 | lanes per actor in a file, batches, notes | none |
| M5.2 Approvals | E2.4 (Ask tier), E5.1 | an agent asks before a large delete | none |
| M5.3 Before and after | E5.2 | per-batch before and after | whether it is in v1 (open) |
| M5.4 Checkpoints | E5.3 | automatic checkpoints, restore | none |
| M5.5 Design review and variants | E5.4 | numbered critique, variants | none |

E5.2 (revert a batch) is drawn inside M5.1 and M5.3.

## M6 V2 and the W1 gate

| Board | Epic | Shows | Not on a board yet |
|---|---|---|---|
| M6.1 V2 blind comparison | E6.1 | two options, source hidden, locked scores | none |
| M6.2 W1 gate scorecard | E6.1, E6.2 | four screens, gate progress, agent usability check, release | triage list is an empty frame |

## After W1 and parked

| Board | Plan | Note |
|---|---|---|
| D.1 to D.7 desktop | phase D (after W1) | layout A, flows, first launch, options B and C; decision 30 and 32 |
| P.1, P.2 Comments | parked (decision 29) | web comments are not built; the boards stay as reference |

## Epics with no board

E0.1 to E0.4, E1.1, E1.3, E1.5, E1.7, E2.2 (apart from the consent step), E4.2, E5.5: no UI of their own. E3.5 (layer names) shows only as names in the layers panel.
