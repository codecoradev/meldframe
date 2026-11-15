# Meldframe

A design canvas that AI agents and humans edit together; the result is exported as HTML/Tailwind with the design system's variables intact.

## Language

### Structure

**Workspace**:
The tenant: a set of people, files, variables, assets and quotas that belong together.
_Avoid_: Team, organization, account

**File**:
One design document inside a workspace. It is the unit that is opened, shared with an agent, exported and edited live.
_Avoid_: Project, document, board

**FileState**:
The in-memory tree of one **File** that commands are applied to. It is the planned code name (in `meldframe-history`) for what a File holds while it is open; it is not a second kind of file.
_Avoid_: Document, model

**Page**:
A named canvas inside a file that holds frames.
_Avoid_: Sheet, screen

**Frame**:
A top-level container on a page with a size, usually a screen or a slide. Frames come from presets (Desktop, Tablet, Mobile; later Carousel, Story, Slide).
_Avoid_: Artboard, screen

**Node**:
Any element in a frame: a frame, text, image, shape or component instance. Nodes form a tree and keep a stable id.
_Avoid_: Layer (a layer is how a node is shown in the layers panel), element, object

**Component**:
A reusable node tree with instances that can override chosen properties. Planned, not yet designed.
_Avoid_: Symbol, widget

**Variant**:
A duplicated frame kept beside the original so two takes can be compared.
_Avoid_: Version, branch

### Design system

**Variable**:
A named, typed value that nodes can bind to instead of holding a literal (color, number or string; shadow, gradient and typography later).
_Avoid_: Token, style, theme value

**Collection**:
A group of variables that share modes.
_Avoid_: Library, set

**Mode**:
One alternative set of values for a collection, such as light and dark.
_Avoid_: Theme, skin

**Alias**:
A variable whose value points at another variable.
_Avoid_: Reference, link

**Binding**:
The link between a node property and a variable.
_Avoid_: Reference

**Brand memory**:
Optional, portable brand rules and design decisions for a workspace, stored outside the file.
_Avoid_: Brand kit, style guide

### Change model

**Command**:
The single vocabulary of change: add, update, remove, reparent, reorder (variable commands to follow). The canvas, the agent tools and undo all use it.
_Avoid_: Mutation, action, edit event

**Op**:
A command submitted by one actor, with an idempotency key, accepted or rejected as a whole and given a place in the file's total order.
_Avoid_: Operation, request

**Replica**:
One actor's working copy of a file: the last state the server confirmed plus that actor's pending ops, so an edit shows at once and the copy still ends equal to the server's order.
_Avoid_: Mirror, local copy, cache

**Pending op**:
An op that its actor has applied and sent but the server has not confirmed or refused yet. A remote op that is ordered before it makes the replica apply it again on the new state.
_Avoid_: Queued op, optimistic op

**Stale op**:
An undo or redo op that was already sent when another actor's op was ordered before the edit it reverses: the values it carries were worked out before that op, and the server will apply them because an op on its way cannot be recalled.
_Avoid_: Old op, outdated op

**Correction**:
An `UpdateNode` that the replica sends after a **Stale op** to set right the fields where that op would leave a value older than the other actor's. It is a pending op that is neither an edit nor part of an undo or redo step, and it falls with the ops it depends on. Only node fields are corrected, not a stale move, reorder, add or delete.
_Avoid_: Compensation, fix, repair op

**Dependency**:
A pending op of the same actor that another op was built on, such as the edit an undo reverses. The server accepts the op only if every dependency was applied, so an undo never lands on a premise the server refused.
_Avoid_: Prerequisite, parent op

**Batch**:
A group of ops submitted together, optionally all-or-nothing. A batch is the unit of review and revert.
_Avoid_: Transaction, changeset

**Actor**:
Whoever submits ops: a human editor or an agent session. Every change is attributed to one actor.
_Avoid_: User (a user is a person; an agent session is also an actor), author

**Agent**:
An external AI client, such as Claude Code, Cursor or Codex, that drives the canvas through MCP.
_Avoid_: Bot, assistant

**Room**:
The live channel of one file: every open client and agent sees the same ordered changes.
_Avoid_: Session, channel

**Review**:
Looking at an agent's batches after they were applied, then keeping or reverting them.
_Avoid_: Approval, staging

**Revert**:
A new batch, authored by the human, that restores what an agent's batch changed; it never rewrites history.
_Avoid_: Rollback, undo (undo is only for one's own changes)

**Checkpoint**:
A named point in a file's history that can be restored.
_Avoid_: Snapshot, save

### Structure of the product

**Feature module**:
A user-visible capability that can be left out of a build (variables, export, review, capture, assets). It registers its commands, settings, jobs, agent tools and panels in one place.
_Avoid_: Plugin, extension, package

**Registry**:
The single list of everything feature modules contribute, read by the editor, MCP and REST.
_Avoid_: Catalogue, manifest

**App layer**:
The UI-neutral part of the editor: state, actions, effects and events as plain data. Front ends only render it.
_Avoid_: View model, store, controller

**Front end**:
Anything that renders app state and dispatches actions: the web editor now; a desktop or mobile app, or a test harness, later.
_Avoid_: Client, shell

**Gate**:
The single step that decides whether a change from any transport is allowed.
_Avoid_: Middleware, guard

**Risk tier**:
How much a call can change user data: Read, Change or Delete. Policy is set per tier and may be overridden per agent.
_Avoid_: Permission level, scope

**Policy**:
What the gate does with a call of a given risk tier: Auto (let it through), Ask (the owner must confirm) or Deny (refuse). The defaults are Read Auto, Change Auto and Delete Ask.
_Avoid_: Permission, rule

**Approval**:
The owner's answer to a call whose policy is Ask: it is pending, then approved, denied or expired, and an approved one admits the same call once. It is not a Review, which looks at changes after they were applied.
_Avoid_: Permission, sign-off

**Durable event**:
A live message that is an accepted change, numbered and replayable after a reconnect.
_Avoid_: Persistent message

**Ephemeral event**:
A live message that is only a hint (agent touch, capture request, progress); it is best-effort and never replayed.
_Avoid_: Transient message, notification

### Output and verification

**Export**:
The translation of a file into HTML/Tailwind with CSS variables, or into PNG or PDF.
_Avoid_: Build, publish

**Fidelity**:
How closely an export matches the canvas, defined in pixels by the fidelity contract.
_Avoid_: Accuracy, parity

**Oracle**:
The automated check that renders an export in a real browser and compares it with the core's result.
_Avoid_: Visual test, snapshot test

**Capture**:
An image of a node or frame, requested by an agent to see its own work.
_Avoid_: Screenshot, render

**Design review**:
A numbered critique list for a file, produced by an agent or a person and tied to the batches that address each item.
_Avoid_: Audit, feedback

## Relationships

- A **Workspace** has many **Files**; a **File** has **Pages**; a **Page** has **Frames**; a **Frame** is a tree of **Nodes**.
- A **Collection** has **Modes** and **Variables**; a **Binding** ties a **Node** property to a **Variable**.
- An **Actor** submits **Ops**; **Ops** travel in **Batches**; a **Batch** can be reviewed and **Reverted**.
- An **Actor** edits through a **Replica**; its **Pending ops** wait for the **Room**, and an **Op** names its **Dependencies**.
- A **File** has one **Room**; the **Room** gives every **Op** one place in a single order.
