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
- A **File** has one **Room**; the **Room** gives every **Op** one place in a single order.
