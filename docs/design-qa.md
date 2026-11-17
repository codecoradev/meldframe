# Design QA

How the editor and desktop design boards were checked, what the check found, and what a person still has to look at. The boards live in the design canvas "Meldframe UI/UX"; the values they follow are in `DESIGN.md`, the layout decisions in `docs/ui-design.md`. This file is a record and a checklist, not a rule: the rules are in `DESIGN.md` (contrast, sizes) and `docs/standards/modular-thin-ui.md` (U-rules).

Last run: 16 Nov 2026, 41 boards.

## Method

Each board file was rendered in headless Chromium at its canvas size, with Inter and JetBrains Mono loaded, and a script measured the page. The script and screenshots were not committed (the boards are design files, not product code). The same checks are meant to run again on the built web client once it renders the showcase, as a test.

Limit: the boards were rendered outside the canvas runtime, so the check covers markup and CSS, not the way the canvas displays them. Hierarchy, taste and flow need a person.

## Automated checks

| # | Check | Pass rule | Result after fixes |
|---|---|---|---|
| 1 | Root size | the root equals the board's width and height | 41 of 41 |
| 2 | Content outside the root | nothing painted beyond the root's edge | 0 (16 elements on 3 boards before) |
| 3 | Clipped content | no `overflow: hidden` box hides content | 2 benign (two code viewers clip 4 and 9 px and scroll) |
| 4 | Text spilling out of a no-wrap box | none | 0 |
| 5 | Text over text | no overlap that is not meant to overlay | 23 flagged, all menus, dialogs, the split slider or inline runs (by design) |
| 6 | Contrast | 4.5:1 for text, 3:1 for large text, against the real background | 209 flagged before, 57 after; the rest are line numbers, separators and disabled items |
| 7 | Text size | at least 11 px | 78 before, 5 after (avatar initials of 9 to 10 px) |
| 8 | Button target | at least 24 px in both directions | 8 small buttons before, 0 after |
| 9 | Accessible names | every button and input has one | 0 missing |
| 10 | Glossary | no `_Avoid_` word of `GLOSSARY.md` in a rendered label | none; "Projects" in the desktop sidebar is the working name still under review |
| 11 | Emoji | none | none |

## Fixed in the pass

- Boards cut off at the bottom (Foundations, Preferences, the desktop layout options) are now tall enough.
- Muted text used `#8b8b94`; it is `#a1a1aa` in 299 places, which passes 4.5:1 on fields and menus.
- The person colour is `#6e60ea` everywhere (white initials 3.99 became 4.64); the primary button is `#2563eb` (white text 3.68 became 5.17).
- Text of 10 px became 11 px in 29 places; six small text buttons got a 24 px target.
- A second look at full resolution found two defects the script did not flag as errors: the focus-order wireframe of the accessibility board was cut off on its right side, and the helper note under the comment composer was unreadable over the canvas frame. Both are fixed. Lesson: after the script, look at every board at full resolution.

## Boards with notes

The boards with the most flags. The rest carry a few flags of the same kinds (line numbers, separators, menus) or none.

| Board | Note |
|---|---|
| D.2 Layout A, Code mode | code viewer clips a few px (it scrolls); menus and dialogs overlay by design |
| M1.6 Files home | 3 texts under 11 px (avatar initials) |
| M2.1 First-run flow | 2 texts under 11 px (avatar initials) |
| M2.3 Agents settings | 9 overlaps, all menus and dialogs |
| M3.1 Variable modes | 5 overlaps, all menus and dialogs |
| M4.1 Export a frame | code viewer clips a few px (it scrolls); 13 low-contrast texts, all line numbers and separators |
| M4.2 Export variables | 16 low-contrast texts, all line numbers and separators |
| M5.3 Before and after | 4 overlaps, the split slider and its handle |
| P.1 Comments (parked for web) | the composer note now sits in its own dark card |

## Manual checklist

To do by a person; tick when done.

- [ ] Every board reads in the right order for a keyboard user; the focus order drawn on the accessibility board matches the real tab order once the client exists.
- [ ] Copy: each message says what happened, whether the file changed and what happens next, with no blame.
- [ ] Words: a final read for `_Avoid_` words in prose, not only in rendered labels.
- [ ] Numbers on the boards are examples (batch counts, node counts, scores) or `[LIMIT]` placeholders; none is presented as a fact.
- [ ] Each board matches the acceptance criteria of its epic in `docs/epics.md` (board IDs to epics are not mapped yet).
- [ ] The light appearance reviewed by the owner; `DESIGN.md` marks it as less reviewed.
- [ ] Behaviour of a narrow window (under 1280 px) decided and drawn; not drawn yet.
- [ ] Component state matrix (hover, focus, active, disabled, error) drawn; not drawn yet.
- [ ] Screen reader pass on the built showcase, not on the boards.
- [ ] The desktop sidebar word (Projects or Repositories) decided against `GLOSSARY.md`.
