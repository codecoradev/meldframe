# Owner questionnaire

**Purpose:** collect the decisions only the owner can make so the PRD, architecture and ADRs can move from draft to approved.
**From:** the drafting agent. **To:** the owner. **How your answers will be used:** each answer is recorded in `docs/decisions.md` or an ADR; unanswered items stay open and block only the parts noted.

## Context

PRD, architecture and six ADRs are drafted. Three research documents (`docs/architecture/spike-survey.md`, `docs/collab-ux.md` with `docs/tokens-and-variables.md`, `docs/fidelity-contract.md`) raised the questions below. They are ordered by how much they block.

## How to answer

Short answers are enough ("yes", "no", a number, "later"). Say "not sure" where you are unsure; that is useful. About 20 minutes.

## 1. Scope and priorities

### Is the P0 list in `docs/prd.md` section 5 the right first release?
> 

### Which feature modules are detachable in W1 (variables, export, review, capture, assets, import-tokens)?
_Why this matters: it fixes the first crate layout and the build matrix._
> 

### Is the agent activity panel the right first vertical slice for the app layer, or should the variables panel come first?
> 

### Is "one human editor per file at the start" acceptable?
_Why this matters: it removes presence and merge-of-people work from W1._
> 

### Should components and instances be in W1 or after it?
_Why this matters: decision 19; components change the document model._
> 

### Is the V2 gate (4/5 on 3 of 4 real screens) still right, and which four screens?
> 

## 2. Fidelity (ADR-0003)

### Is 0.05 px for geometry and 1 px for text ink acceptable, with anti-aliasing identity not promised?
> 

### May CI depend on headless Chromium (Playwright)?
> 

### For degenerate documents (fill under hug, overflow, zero-width fill), should the validator normalise or warn, or should the core change to match CSS?
_Why this matters: the contract currently picks the validator._
> 

### Is text outside Latin (CJK, long URLs, tabs) in scope for W1?
> 

### After Inter, how should other fonts be registered?
> 

### Should Tailwind output be idiomatic (`p-4`, variable classes) or exact arbitrary values?
_Why this matters: idiomatic output needs the variable mapping and changes what the oracle compares._
> 

### How should exported images be referenced: relative file, data URI or hosted URL?
> 

## 3. Variables (ADR-0004)

### Are collections, modes and aliases enough, with component variables deferred?
> 

### Must variable names be unique per collection or across the workspace?
> 

### Store colours as hex sRGB first, or support OKLCH and Display P3 from the start?
> 

### Should variables shared across files in a workspace be in W1?
_Why this matters: this is where a real design system lives._
> 

### Default export unit for spacing: px or rem?
> 

### Import Tailwind screens as frame presets?
> 

## 4. Agent collaboration (ADR-0005)

### Accept "apply live, review afterwards" with no staged mode in W1?
> 

### Should an agent session need approval to delete a whole frame or a large subtree, or is live apply plus revert enough?
_Why this matters: ADR-0010 gives calls risk tiers (Read, Change, Delete) while ADR-0005 applies agent changes live; the two must agree on what Delete means._
> 

### Should a partial revert (some fields of a batch) be allowed, or always the whole batch?
> 

### Should agent sessions get an implicit `ifSeq` on edits of existing nodes?
_Why this matters: it prevents overwrites but increases rejections._
> 

### Should a session bind to a frame or page, not only a file?
> 

### Where should the activity panel live, given the right panel is already dense?
> 

### Should "follow the agent" and before/after compare be in W1?
> 

### Should the agent be asked to duplicate a frame before a large rewrite?
> 

## 5. Platform and process

### Should headless capture wait until a real agent workflow without a browser needs it (no separate spike)?
> 

### Who may merge pull requests in the product repository: the agent after your approval, or only you?
> 

### Which Gatehouse client type and scopes should MCP clients use?
> 

## Anything else?
> 
