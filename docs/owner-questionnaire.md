# Owner questionnaire

**Purpose:** collect the decisions only the owner can make so the PRD, architecture and ADRs can move from draft to approved.
**From:** the drafting agent. **To:** the owner. **How your answers will be used:** each answer is recorded in `docs/decisions.md` or an ADR; unanswered items stay open and block only the parts noted.

## Context

PRD, architecture and six ADRs are drafted. Three research documents (`docs/architecture/spike-survey.md`, `docs/collab-ux.md` with `docs/tokens-and-variables.md`, `docs/fidelity-contract.md`) raised the questions below. They are ordered by how much they block.

## Status

On 13 Nov 2026 the owner set the first goal (backend, basic web editor and MCP connected end to end) and accepted the recommendations below as working defaults. Some items were marked as not yet certain: any answer can still be changed by writing over it; the plan in `docs/epics.md` follows these defaults until then.

## How to answer

Short answers are enough ("yes", "no", a number, "later"). Say "not sure" where you are unsure; that is useful. About 20 minutes.

## 1. Scope and priorities

### Is the P0 list in `docs/prd.md` section 5 the right first release?
> **Working default (recommendation, 13 Nov 2026):** Yes as the W1 scope. The first goal is narrower: backend, basic web editor and MCP connected end to end (milestone M1 in `docs/epics.md`).

### Which feature modules are detachable in W1 (variables, export, review, capture, assets, import-tokens)?
_Why this matters: it fixes the first crate layout and the build matrix._
> **Working default (recommendation, 13 Nov 2026):** All six, as feature modules. The document model, `Command` and the render model stay inner.

### Is the agent activity panel the right first vertical slice for the app layer, or should the variables panel come first?
> **Working default (recommendation, 13 Nov 2026):** The editor shell (layers, canvas, properties) comes first, because the first goal is a connected basic editor (M1). The activity panel follows in M5, the variables panel in M3.

### Is "one human editor per file at the start" acceptable?
_Why this matters: it removes presence and merge-of-people work from W1._
> **Working default (recommendation, 13 Nov 2026):** Yes.

### Should components and instances be in W1 or after it?
_Why this matters: decision 19; components change the document model._
> **Working default (recommendation, 13 Nov 2026):** After W1.

### Is the V2 gate (4/5 on 3 of 4 real screens) still right, and which four screens?
> **Working default (recommendation, 13 Nov 2026):** Keep the gate. Proposed screens: Uteke Cloud landing, pricing, compare page, and one Uteke Mobile screen.

## 2. Fidelity (ADR-0003)

### Is 0.05 px for geometry and 1 px for text ink acceptable, with anti-aliasing identity not promised?
> **Working default (recommendation, 13 Nov 2026):** Yes.

### May CI depend on headless Chromium (Playwright)?
> **Working default (recommendation, 13 Nov 2026):** Yes, for CI. Runtime use waits for decision 18.

### For degenerate documents (fill under hug, overflow, zero-width fill), should the validator normalise or warn, or should the core change to match CSS?
_Why this matters: the contract currently picks the validator._
> **Working default (recommendation, 13 Nov 2026):** The validator normalises fill under hug and warns on overflow and zero-width fills; the core does not change.

### Is text outside Latin (CJK, long URLs, tabs) in scope for W1?
> **Working default (recommendation, 13 Nov 2026):** No. Latin text with Inter in W1.

### After Inter, how should other fonts be registered?
> **Working default (recommendation, 13 Nov 2026):** Not in W1. Inter only; a font registry is designed after W1.

### Should Tailwind output be idiomatic (`p-4`, variable classes) or exact arbitrary values?
_Why this matters: idiomatic output needs the variable mapping and changes what the oracle compares._
> **Working default (recommendation, 13 Nov 2026):** Exact arbitrary values for literals, variable-based classes for bound fields. Fully idiomatic output after W1.

### How should exported images be referenced: relative file, data URI or hosted URL?
> **Working default (recommendation, 13 Nov 2026):** Relative files next to the exported HTML.

## 3. Variables (ADR-0004)

### Are collections, modes and aliases enough, with component variables deferred?
> **Working default (recommendation, 13 Nov 2026):** Yes.

### Must variable names be unique per collection or across the workspace?
> **Working default (recommendation, 13 Nov 2026):** Per collection.

### Store colours as hex sRGB first, or support OKLCH and Display P3 from the start?
> **Working default (recommendation, 13 Nov 2026):** Hex sRGB first; OKLCH and P3 after W1.

### Should variables shared across files in a workspace be in W1?
_Why this matters: this is where a real design system lives._
> **Working default (recommendation, 13 Nov 2026):** Yes: collections belong to the workspace (as in `docs/architecture.md` section 5).

### Default export unit for spacing: px or rem?
> **Working default (recommendation, 13 Nov 2026):** px.

### Import Tailwind screens as frame presets?
> **Working default (recommendation, 13 Nov 2026):** Not in W1.

## 4. Agent collaboration (ADR-0005)

### Accept "apply live, review afterwards" with no staged mode in W1?
> **Working default (recommendation, 13 Nov 2026):** Yes.

### Should an agent session need approval to delete a whole frame or a large subtree, or is live apply plus revert enough?
_Why this matters: ADR-0010 gives calls risk tiers (Read, Change, Delete) while ADR-0005 applies agent changes live; the two must agree on what Delete means._
> **Working default (recommendation, 13 Nov 2026):** Ask before an agent deletes a whole frame or a subtree above a size threshold; other changes apply live and can be reverted. Read is Auto, Change is Auto.

### Should a partial revert (some fields of a batch) be allowed, or always the whole batch?
> **Working default (recommendation, 13 Nov 2026):** The whole batch, atomic; fields that someone else wrote later are skipped and reported.

### Should agent sessions get an implicit `ifSeq` on edits of existing nodes?
_Why this matters: it prevents overwrites but increases rejections._
> **Working default (recommendation, 13 Nov 2026):** No implicit `ifSeq` at first; the agent guidance asks for it explicitly. Measure conflicts on real runs in M5 and revisit.

### Should a session bind to a frame or page, not only a file?
> **Working default (recommendation, 13 Nov 2026):** File only in W1.

### Where should the activity panel live, given the right panel is already dense?
> **Working default (recommendation, 13 Nov 2026):** A tab in the left sidebar next to Layers.

### Should "follow the agent" and before/after compare be in W1?
> **Working default (recommendation, 13 Nov 2026):** Not in W1. Touch hints are in W1.

### Should the agent be asked to duplicate a frame before a large rewrite?
> **Working default (recommendation, 13 Nov 2026):** Yes, through the agent guidance (not enforced).

## 5. Platform and process

### Should headless capture wait until a real agent workflow without a browser needs it (no separate spike)?
> **Working default (recommendation, 13 Nov 2026):** Yes.

### Who may merge pull requests in the product repository: the agent after your approval, or only you?
> **Working default (recommendation, 13 Nov 2026):** The agent, after your approval.

### Which Gatehouse client type and scopes should MCP clients use?
> **Working default (recommendation, 13 Nov 2026):** Proposed: a public client with PKCE and dynamic client registration, scopes `openid profile` plus Meldframe read and write scopes. To confirm against Gatehouse in E2.2.

## Anything else?
> 
