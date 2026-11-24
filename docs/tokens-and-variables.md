# Tokens and variables

See also: [tokens-and-code.md](tokens-and-code.md), the loop from a project's CSS to the canvas and back, with runnable examples.

Status: **draft for owner review** (13 Nov 2026). What E3.3 built of it, and where it chose differently, is in ADR-0017: the model, bindings (`fill`, `radius`, `opacity` only so far), resolution, the mode switch as view state, undo and storage. Sections 3 to 5 below stay the design; 6 onwards (canvas modes per frame, import, export) are not built. `[P]` = proposed decision, `[Q]` = open question, `[A]` = assumption. Related decisions: `decisions.md` #3, #6, #7, #15, #17, #19. This is a design document, not product code. The spike (`wonder-proto`, branch `spike/rust-core`) has no token support; its `Node` stores `fill` as a plain string (default `#3f3f46`), uniform `padding`/`radius`/`gap`, uniform `border`, up to 4 `shadows`, `font_weight`, optional `image`. No margin, opacity, gradient, "no fill" or per-side padding.

## 1. Goals and non-goals

Goals (concept v1, differentiator 2): tokens that are **rendered on the canvas** (not stored as literals), **exported as CSS variables** (and Tailwind), and **imported** from an existing Tailwind config or CSS variables so a real project's design system is the starting point (decision 7).

Concept section 12 records that, in the owner's Wonder session, the agent reported `var(--...)` not rendering on the Wonder canvas and fell back to literals. The screenshot (`20.png`) shows the Variables section collapsed in the right panel and plain numeric fields; it cannot confirm or refute this. Treat it as an owner-reported observation, not a verified fact. The Wonder changelog lists variables, Display P3/OKLCH and Tailwind/CSS/DTCG export (research doc `research-wonder-deep-pricing-mcp-acp-nov-2026`, source marked primary there).

Non-goals now: component tokens, theming per component variant, token animation, Figma variable sync, a token marketplace, runtime theming API for the exported site beyond CSS variables.

## 2. Reference: how others model it

Read from search-result summaries only (the sandbox blocked page fetches, see Sources). Details marked unverified need a check against the primary page before we copy anything.

- **Figma Variables**: collections hold variables; each collection has one or more **modes** and each variable stores a value per mode; **aliases** point a variable at another variable; types are Color, Number (FLOAT), String, Boolean; **scopes** restrict which property pickers list a variable (for example corner radius, width/height, gap, text content, fills, stroke, effect color; `ALL_SCOPES` means everywhere). Scopes only filter pickers: they do not prevent binding through the API. Common practice: a primitives collection with empty scopes (hidden) and semantic collections with light/dark modes aliasing into it. The number of modes per collection depends on plan (help article snippet).
- **Penpot design tokens**: **sets** (groups of tokens), **themes** (named selections of active sets), aliases between tokens, renaming prompts to remap references. Export is Tokens Studio style with `$metadata`, not strict DTCG 2025.10 (community post, unverified). A Penpot Hub plugin exports CSS/SCSS/Tailwind v4/TS/DTCG.
- **DTCG** (W3C Design Tokens Community Group): Format Module 2025.10, a Final Community Group Report dated 28 Oct 2025, called the first stable version in the group's announcement (search snippets). Tokens are JSON objects with `$value`, `$type`, `$description`; aliases are `"{group.token}"` curly-brace references in `$value`; the Color Module defines a structured color `$value`; the Resolver Module covers sets/modifiers (theming) separately. The format itself has no "modes". I could not confirm the exact `$extends`/`$ref` usage; treat as unverified.
- **Tailwind v4**: tokens live in CSS inside `@theme`; the variable name prefix (namespace) decides which utilities exist: `--color-*`, `--spacing`/`--spacing-*`, `--radius-*`, `--shadow-*`, `--text-*`, `--font-*`, and so on. `@theme inline` maps a utility to a variable without emitting a new CSS variable and is recommended when the value references another variable (so the utility keeps pointing at the right one). `@theme static` always emits the variables, even unused (search snippets disagree on details: unverified). A JS config can still be loaded in v4 for legacy setups, but plugins and `darkMode` in a JS config behave differently from v3; dark mode in v4 is declared with `@custom-variant` (conflicting secondary sources: unverified). The spike already proved Tailwind CLI 4.3.5 output equals plain CSS on 84 nodes x 26 computed properties (`s9-design-code.md`, A2).
- **Style Dictionary**: a build tool that reads token JSON (DTCG supported in v4) and emits platform files. We do not depend on it; exporting DTCG lets users run it (unverified detail, from the Penpot search result).

## 3. Model

### 3.1 Tiers: keep the model flat, make tiers a convention [P]

Primitive / semantic / component is a **naming and linting convention**, not three model concepts:

- **Primitive**: raw values (`color/blue/500 = #3b82f6`, `space/4 = 16`).
- **Semantic**: roles that alias primitives and usually have modes (`color/bg`, `color/text`, `color/brand` -> light: `color/blue/600`, dark: `color/blue/400`).
- **Component** tokens (`button/bg`) are deferred (Q1); they are just more semantic variables whose names start with the component.

The model has only **collections, modes, variables, aliases, scopes**. The importer and linter apply the convention: nodes should bind semantic variables, not primitives, and primitives get `hidden` scope. This is smaller than Figma's tiers and sufficient for Tailwind/shadcn-style projects (shadcn's palette is semantic variables with light and dark values).

### 3.2 Entities

```
Collection   { id, name, modes: [Mode], order }
Mode         { id, name, css: { selector?: string, media?: string } }   // e.g. dark -> ".dark"
Variable     { id, collectionId, name: "color/brand/500", type, scopes: [Scope],
               hidden: bool, description?, values: { modeId -> Value }, order }
Value        = Literal(typed value) | Alias { var: variableId }
```

- `id`: opaque stable id (like node ids). Never the name, so rename is cheap and safe.
- `name`: slash path, lowercase segments `[a-z0-9]+(-[a-z0-9]+)*` [P]. This maps 1:1 to CSS (`/` becomes `-`) and avoids Tailwind namespace surprises. Dots and uppercase are rejected at the command level (importer normalizes and reports). Names are unique per collection (not globally) [Q2].
- One **default mode** per collection (first mode). A mode named `dark` with `css.selector = ".dark"` or `css.media = "(prefers-color-scheme: dark)"` drives export (section 7).

### 3.3 Types and scopes

Storage types [P]:

| Type | Literal | Notes |
|---|---|---|
| `color` | hex `#rgb/#rgba/#rrggbb/#rrggbbaa` in round 2; extended to a struct `{space: srgb|display-p3|oklch, ...}` later | Wonder lists Display P3 and OKLCH; Tailwind v4's default palette is OKLCH. Store what we can render exactly; see Q3. |
| `number` | f64, unitless (px on the canvas) | spacing, size, radius, opacity (0..1), font size, line height, font weight, letter spacing |
| `string` | text | font family, text content |
| `shadow` | list of `Shadow` (0..4) | composite, phase T6 |
| `gradient` | `{kind: linear|radial, angle, stops: [{pos, color: Value<color>}]}` | composite, phase T6; stops are bindable to color variables |
| `typography` | `{family, size, weight, lineHeight, letterSpacing}` each bindable | composite, phase T6 |

Booleans are not needed now (Figma has them for prototype/visibility logic) [P: skip].

**Scopes** decide where a variable is offered and how it is exported. They are a **UI and lint filter, not a security boundary**, as in Figma. The server validates binding by **type** (a color variable cannot bind a number field), and, when scope is set, warns (lint) rather than rejects on a scope mismatch so agents and imports are not blocked [P].

| Scope | Applies to | Tailwind v4 namespace on export |
|---|---|---|
| `fill`, `text-fill`, `stroke`, `effect-color`, `all-colors` | color | `--color-*` |
| `spacing` | number: gap, padding, margin | `--spacing-*` (see 7.3) |
| `size` | number: width, height | `--spacing-*` or arbitrary `--size-*` [Q4] |
| `radius` | number: corner radius | `--radius-*` |
| `opacity` | number 0..1 | `--opacity-*` (not a default Tailwind namespace: emitted as plain variable) |
| `font-size`, `line-height`, `letter-spacing`, `font-weight` | number | `--text-*`, `--leading-*`, `--tracking-*`, `--font-weight-*` |
| `font-family` | string | `--font-*` |
| `shadow` | shadow | `--shadow-*` |
| `gradient` | gradient | plain variable (Tailwind has no gradient namespace) |
| none / `hidden` | primitives not offered in pickers | emitted per rules in section 7 |

The namespace table is `[A]` from the Tailwind v4 default theme; it must be re-verified against the official page before implementation (not fetched, see Sources).

## 4. Binding: how a node references a token

### 4.1 Representation

Every bindable node field becomes `Bindable<T> = T | { "var": "<variableId>" }` [P]. A bare value stays valid (backward compatible with existing documents and agents). Options considered:

| Option | Example | Pro | Con |
|---|---|---|---|
| A. Sentinel string | `"fill": "var:color/brand"` | zero schema change, simplest for agents | stringly typed, collides with free text, rename breaks silently, cannot carry a type |
| **B. Typed ref object by id** | `"fill": {"var": "v_8f3a"}` | rename-safe, typed, validated, undo-friendly | agents must look ids up (mitigated at the MCP edge) |
| C. Name-based ref | `{"var": "color/brand"}` | readable | rename must rewrite every reference (Penpot offers remapping), non-deterministic across renames |

Recommendation [P]: **B in the document and on the wire between our own clients**, plus a **name shorthand at the MCP edge** that the MCP adapter resolves to an id against the current snapshot before submitting (`{"var": "color/brand"}` or `"$color/brand"`). If the name does not exist, the call fails with `variable_not_found` and lists near matches (like the structured errors in S15). The log therefore stores ids: replays are deterministic even after renames.

Fields that become bindable in round 2 (so the schema is designed once, decision 19): `fill`, `fill2`/gradient stop colors, `border.color`, `border.width`, `shadows[].color`, `shadows[].x/y/blur/spread`, `radius` (and per-corner later), `gap`, `padding` (per side), `margin` (per side), `width`/`height` (when fixed), `opacity`, `font_size`, `font_weight`, `text color`, `line_height`, `letter_spacing`, `font family`. Position (`x`,`y`) is **not** bindable [P].

Composite binding: a whole `shadows` list, a whole `fill` (gradient) or a whole text style may bind one composite variable: `{"var": id}` at the field level.

### 4.2 Modes on nodes

Resolution needs a mode per collection. Order of lookup [P]: the nearest ancestor frame with an explicit override `modes: {collectionId: modeId}`, else the page's, else the collection's default mode. Frame-level override is what lets one page show a light and a dark version of a screen side by side (and carousel/slide variants later). Override is a node field like any other, so patch/undo apply unchanged. Phase T6; before that only the default mode renders, plus a global mode switch in the toolbar (view state, not document state) [Q5].

### 4.3 Resolution

```
resolve(node, field):
  v = node[field]
  if v is Literal: return v
  var = variables[v.var]
  if var missing: return FALLBACK(field)          // see 9.1, never panic
  modeId = mode_for(node, var.collection)         // 4.2
  value = var.values[modeId] ?? var.values[defaultMode]   // 9.3
  if value is Alias: follow with the consumer's mode for the target's collection, depth <= 16
  return literal
```

One pure function in the shared Rust core used by: layout (spacing and size tokens change layout!), the canvas renderer (WASM), HTML/Tailwind export, capture, and tests. The TypeScript layout oracle in the spike (`src/lib/layout.js`) must learn it for the differential tests. Cache: resolved value per (variable, mode) invalidated by variable writes; a token edit marks only nodes bound to it dirty (reverse index variable -> nodes) [A].

Alias semantics follow Figma: an alias is resolved **using the consumer's current mode** for the target variable's collection, so `color/brand` (modes light/dark, in collection `theme`) can alias `color/blue/600` (single-mode collection `primitives`) and still switch.

## 5. How Command, patch, undo and MCP treat tokens

### 5.1 Commands [P]

Node commands do not change shape: a bound field is just a different field value inside `UpdateNode` patches (`fill: {"var": id}`). New variable commands, same family as node commands (one `Command` enum for UI, WS, MCP and undo is a stated principle):

```
AddCollection{collection}                 RemoveCollection{id}        RenameCollection{id,name}
AddMode{collectionId, mode, copyFrom}     RemoveMode{...}             RenameMode / SetModeCss
AddVariable{variable}                     RemoveVariable{id, detach}  UpdateVariable{id, patch}
SetVariableValue{id, modeId, value}       // the field-level unit for LWW and undo
```

Status: the rename commands are in the core as `renameCollection {id, name}`, `renameMode {collection, mode, name}` and `renameVariable {id, name}` (ADR-0017, note of 20 Nov 2026). `UpdateVariable{id, patch}` is not taken: a variable has no field to patch but its name. `SetModeCss` is not built. `RemoveVariable` has no `detach` flag as a Command (removal never detaches, ADR-0017); the app layer has `Action::RemoveVariable {id, detach}` since #156.

Design question [Q6]: implement variables as a **second entity table that reuses the node patch/undo machinery** (same `ifSeq` trace, same field-level LWW, same S12 inverse logic, same fractional index for order) versus adding bespoke commands. Recommendation: generalize the store (entity kind + id + field), because S12 took significant effort to get node undo right and variables need the same properties. Needs a spike (T1).

### 5.2 Validation (deterministic function of state at seq, as `check_command` is today)

- `SetVariableValue`: value type matches variable type; alias target exists, has the same type, and **does not create a cycle** (reject `alias_cycle`, naming the chain); alias depth <= 16.
- `AddVariable`/`RenameVariable`: name syntax (3.2) and uniqueness.
- Node patch with `{"var": id}`: variable exists and its type matches the field; scope mismatch is a lint warning only.
- `RemoveVariable{id, detach}`: with `detach: true` the same command rewrites every node field and every alias that referenced it to a literal. As built (the Variables manage actions of the app layer, `Action::RemoveVariable` and `Action::RemoveCollection`; ADR-0017 keeps `removeVariable` itself a single command that never detaches): a node field takes the **default-mode resolved literal**, as written here; an alias takes the literal it has in **its own mode** (the other collections in their default mode). In the default mode the two readings are the same. In another mode, baking the default-mode literal into a `dark` cell would flip what every node bound through that alias paints in dark, so the app keeps what each cell shows. This is a reading of the note and is **flagged for the owner**, who may prefer the strict default-mode literal for aliases too (one line in `meldframe_app_parts::manage`). Detach is a **group of ops in one undo step**, not one atomic op: the app sends the `updateNode` unbinds, the `setVariableValue` literals and the `removeVariable`s as separate ops, so the server can accept some and refuse others. If another actor binds the variable first, only the removal is dropped (the variable stays and the unbinds stay), which is the "the caller unbinds first" rule of ADR-0017. With `detach: false` it is rejected `variable_in_use` with a count and the first 20 node ids [P default: `detach` required to be explicit; agents must choose].
- `RemoveMode`: values for that mode are dropped; frame overrides pointing at it are cleared (same command).
- `AddMode{copyFrom}`: copies every variable value from the source mode so the collection stays **complete** (9.3).

### 5.3 Undo and concurrency (S12 rules extended)

- The unit of last-writer-wins is **(variable, mode)**, like (node, field).
- Undo of a binding change restores the previous value (literal or ref). If the previous ref points to a variable another actor deleted meanwhile, the inverse is **dropped**, exactly like an inverse on a node deleted by others (S12 rule 4). `[A]`: needs new tests.
- Undo of `RemoveVariable{detach}` restores the variable and **rebinds the detached fields only where the field still holds the baked literal** (otherwise someone changed it; keep theirs).
- An agent re-skinning (changing many variable values in one atomic batch) is a normal batch for review/revert (see `collab-ux.md`); the activity summary counts "affects N nodes" using the reverse index.
- `ifSeq` is node-granular today; add the variable as a tracked entity so `ifSeq` on `SetVariableValue` protects a value another actor changed.
- Known S12 gaps carry over unchanged (concurrency window, z-index not transformed); variables add no new class except dangling-ref inverses.

### 5.4 MCP surface [P]

| Tool | Purpose |
|---|---|
| `list_variables` | collections, modes, variables with values per mode and usage counts; filter by type/scope/name prefix |
| `get_document` | node fields return `{"var": id, "name": "color/brand", "resolved": "#2563eb"}` for bound fields so the agent sees both |
| `apply_commands` | accepts the commands above; name shorthand resolved at the edge |
| `import_tokens` | takes normalized CSS text / JSON (section 8); returns an import report |
| `export_tokens` | returns CSS / Tailwind v4 / v3 config / DTCG text |
| `bind_tokens` | bulk: for the given nodes or whole page, replace literals that exactly (or within a tolerance) match a variable; returns what changed |
| `design_review` | includes token rules (hard-coded literals matching a token, unused variables, scope misuse) |

V1 lesson: agents silently work around missing model features instead of erroring (`v1-agent-design-eval.md`). So `describe_schema`/`whoami` must list the token catalog and say "bind variables; literals are linted", and the shipped agent guidance says to prefer variables. V1 also found `fill` accepts any 64-char string (for example `linear-gradient(...)`) and the Pixi client threw and stopped rendering: with typed `Paint`/`Bindable` the server rejects invalid values and the renderer must never abort on a bad node.

## 6. Rendering on the canvas

- The renderer receives **resolved** values only (resolve in core, see 4.3). A token edit re-resolves bound nodes and repaints; no CSS variable machinery exists on the canvas (PixiJS), and none should.
- Layout uses resolved numbers (spacing, size tokens). Because layout is computed in the shared Rust core on server and WASM, token changes reflow identically in capture and export.
- Mode switching is a view-state toggle (global) plus optional frame overrides (4.2).
- Visual affordance: bound fields show a chip with the variable name in the inspector; on canvas nothing differs (the point is that it renders like any other value).
- Never fail rendering: missing variable -> fallback (9.1), cycle can never be stored (validated), type mismatch can never be stored.

## 7. Export

The oracle for fidelity is already decided (#15): render the export in Chromium and compare with the canvas. Tokens extend it: same screen in each mode, canvas vs exported page with that mode's selector active.

### 7.1 CSS variables [P]

```css
/* tokens.css (generated) */
:root {
  --color-blue-600: #2563eb;          /* primitives */
  --space-4: 16px;
  --color-brand: var(--color-blue-600);   /* semantic, default mode */
}
.dark {                                  /* mode.css.selector */
  --color-brand: var(--color-blue-400);
}
@media (prefers-color-scheme: dark) {    /* mode.css.media, optional */
  :root:not(.light) { --color-brand: var(--color-blue-400); }
}
```

Nodes export as `var(--color-brand)` in generated CSS and `bg-brand` utilities in Tailwind output, never as the literal. Aliases export as `var(--target)` chains (kept, not flattened) so changing a primitive in code keeps working. Names: `/` to `-`, prefixed per namespace (`color/brand` -> `--color-brand`) [A]. Units: px by default; option `rem` with a 16 px root [Q7].

### 7.2 Tailwind v4 [P]

Namespaced variables go in `@theme`. Semantic variables with modes use the common pattern: define the raw variable per mode in plain CSS and map the utility with `@theme inline` so utilities follow the mode:

```css
@import "tailwindcss";
@custom-variant dark (&:where(.dark, .dark *));

:root { --brand: var(--color-blue-600); }
.dark { --brand: var(--color-blue-400); }

@theme {
  --color-blue-600: #2563eb;
  --color-blue-400: #60a5fa;
  --radius-card: 12px;
}
@theme inline {
  --color-brand: var(--brand);
}
```

This is the pattern used by shadcn/ui with Tailwind v4 (own knowledge, not re-verified here); `@theme inline` for variables that reference other variables is also what secondary sources recommend. Spacing: if every spacing token is a multiple of one base (commonly 4 px), emit `--spacing: 0.25rem` and let numeric utilities (`p-4`) work; otherwise emit named `--spacing-<name>` tokens and note the loss of numeric scale [A]. Oracle: compile the export with the Tailwind CLI (4.3.5 is already in the spike tooling) and assert the generated utilities' declarations equal the expected `var(--...)` references; run Chromium comparison on top.

### 7.3 Tailwind v3 config [P, lower priority]

`tailwind.config.js` with `theme.extend.colors: { brand: 'var(--color-brand)' }` plus the CSS file above. Known limit (own knowledge, to verify): opacity modifiers (`bg-brand/50`) do not work with a hex-valued CSS variable in v3; they need the channel form `rgb(var(--brand-rgb) / <alpha-value>)`. Option: emit channel variables for colors only when the user asks for v3. Dark mode: `darkMode: 'class'` with the same selectors.

### 7.4 DTCG (optional) [P]

Export our model as DTCG 2025.10: groups from the slash path, `$type`, `$value`, aliases as `"{color.blue.600}"`. Not representable in DTCG proper: modes, scopes, ids, `hidden`. Options: one file per mode (`tokens.light.json`, `tokens.dark.json`) as the resolver-style convention; scopes and `hidden` in `$extensions.meldframe`. The **document is the source of truth**; DTCG is an interchange artifact, never the storage format (it cannot carry ids or modes losslessly). Import of DTCG is optional and best effort. The compatibility target is "Style Dictionary and Tokens Studio can read it", checked by validating against the format's JSON shape in a test (not by trusting a tool).

### 7.5 Fidelity contract additions

Add to the fidelity contract (#15): tokens export as variables, not literals; computed style in Chromium equals canvas per mode within the existing pixel/rect tolerances; `color-mix`, OKLCH and P3 colors are only exported when the canvas can render them exactly (Q3).

## 8. Import (decision 7: first design system comes from Tailwind config or CSS variables)

### 8.1 Where parsing happens [P]

The importer takes **text, never executes code on the server**. A `tailwind.config.js` can run arbitrary JavaScript (functions, requires, plugins); executing user JS server-side is a security and reliability problem. Paths:

1. **Agent-assisted (primary)**: the user's agent runs in their repo, resolves the config locally (for example Tailwind's resolved-config output as JSON) and calls the MCP `import_tokens` tool with normalized JSON. Fits decision 5 (web has no agent runtime; external agents have the repo).
2. **Paste/upload CSS** containing `:root`, `.dark`, `@theme` blocks (and the v4 `@import`): parsed server-side by a CSS parser (candidate: a Rust CSS crate such as `lightningcss`; availability and fit unverified) or a minimal hand-written declaration scanner restricted to custom properties.
3. **Upload a resolved-config JSON** produced by the user's own tooling.

### 8.2 What maps

| Source | Becomes |
|---|---|
| `--name: <color>` in `:root` (hex, `rgb()`, `hsl()`, `oklch()`, named) | color variable in a collection, default mode |
| same name in `.dark` / `[data-theme=dark]` / `@media (prefers-color-scheme: dark)` | value of the `dark` mode, selector recorded in `mode.css` |
| `--x: var(--y)` pure reference | alias |
| shadcn v3 style `--background: 0 0% 100%` consumed as `hsl(var(--background))` | channel triplet detected by pattern, converted to color; export can re-emit in the same style if `import.style = channels` is remembered [A] |
| v4 `@theme { --color-*, --spacing, --radius-*, --font-*, --text-*, --shadow-* }` | variables typed and scoped by namespace; `--spacing` base creates a `space/*` scale |
| v3 `theme.extend.colors/spacing/borderRadius/fontSize/boxShadow/fontFamily` (resolved) | variables; nested color objects become `color/blue/500` |
| `fontSize: [size, {lineHeight}]` tuples | font size plus line height variables |
| `screens` breakpoints | not tokens: offered as **frame presets** (Tablet/Desktop widths) [Q8] |
| `darkMode: 'class'` or `'media'` (or `@custom-variant`) | default `css.selector`/`css.media` of the dark mode |

### 8.3 What cannot be imported (and what we say)

The import returns a **report**: imported (count by type), approximated, skipped, with reasons. Cannot or will not import:

- Arbitrary values in markup (`bg-[#123456]`): no config entry; not tokens (a future "extract literals from a design" is a separate feature, `bind_tokens`).
- Plugins (`addUtilities`, `addComponents`, `addBase`, typography/forms plugins), `@apply`, `@layer` components, `theme()` calls inside plugins.
- Dynamic config: functions as values, `require`, conditionals; only available if the agent resolved them locally first.
- Computed CSS values: `calc()`, `clamp()`, `min()/max()`, `color-mix()`, `light-dark()` as token values: imported as **unresolved literals of type string with a warning**, not evaluated [P]; pure `var()` chains become aliases.
- Keyframes/animations (motion is on hold), container queries, `content` globs, `prefix`, `important`, `presets` (flattened if resolved).
- OKLCH/P3 colors outside what the canvas renders exactly: converted with a warning, or kept as a string literal [Q3].
- Names that do not fit the slug rule: normalized (`1.5` -> `1-5`, camelCase -> kebab), original kept in `description` for round-trip, collisions reported.

Re-import: matches by name path within the collection; updates values, adds new variables, **never deletes** variables absent from the source unless asked (`prune: true`) [P]. All of it is one atomic batch (one review/revert unit).

### 8.4 Corpus (also the test input)

shadcn/ui default theme (Tailwind v3 channel style and v4 OKLCH style), Tailwind default theme, the owner's `uteke-cloud` project (dogfood), a config with a plugin and a function value (expected: report skips), a v4 file with `@theme inline` and a `.dark` block.

## 9. Edge cases

### 9.1 Missing token
Reference to a variable that does not exist (deleted by another actor, bad import): never reaches storage via commands (validation), but can arrive by race (variable removed between validation and apply is impossible in a total order, so the only source is old data/bugs). Resolve falls back to a **visible fallback** per type (magenta `#ff00ff` for color, `0` for number, the node's previous literal if known) and raises a lint error "Broken variable reference". `RemoveVariable` has `detach` precisely to avoid this.

### 9.2 Alias cycle
Rejected at command time with the chain in the message (`alias_cycle: color/a -> color/b -> color/a`). Cross-collection and cross-mode aliases are checked on the mode-expanded graph (a cycle in any mode is a cycle). Defense in depth: resolve has a depth cap of 16 and returns the fallback.

### 9.3 Incomplete modes
Invariant: **every variable has a value for every mode of its collection** [P], enforced by commands: `AddMode` copies from a source mode; `AddVariable` requires the default-mode value and fills the other modes with a copy of it (marked `inherited` so the UI can show it dimmed). Resolve still falls back to the default mode value for robustness (imports, old data). Import of a variable present only in `:root` gets inherited values in `dark`.

### 9.4 Type or scope mismatch
Type mismatch: rejected. Scope mismatch: lint warning.

### 9.5 Deleting a collection or mode
`RemoveCollection` follows the `detach` rule for all its variables. Cannot remove the last mode.

### 9.6 Rename
Free: ids are stable. Export names change; the next export reflects the new name; a rename is reported in the activity summary because it is a breaking change for code that uses the CSS variable [A].

### 9.7 Huge systems
Cap variables per file (for example 5,000) and modes per collection (for example 12) in validation, like the node-count caps added in hardening [A].

### 9.8 Concurrent edits
Two actors set the same (variable, mode): last write wins; an agent re-skin and a user tweak on different variables merge. `ifSeq` protects.

## 10. UI: Variables and Brand panels

### 10.1 What the Wonder screenshot shows (`20.png`)

Right panel, top to bottom: **Variables** (collapsed row with search and add icons), **Icons** (collapsed, with grid, search and add icons), then an expanded **Properties** block:

- **Position**: align buttons row, X and Y fields, rotation field, horizontal and vertical constraint dropdowns (Left, Top), Z-index field ("Auto"), constraint widget.
- **Layout**: a **Flow** row with four mode icons (the fourth is selected; the screenshot does not name them; the research doc lists freeform/flex/block/grid/inline).
- **Size**: W and H with a unit dropdown (px) and a sizing-mode icon.
- **Spacing** (gap) and **Alignment** (three buttons).
- **Padding**: two fields (horizontal, vertical) with an icon at the right to expand to per-side.
- **Margin**: four fields shown (0, 10, 0, 0) with the expand toggle active (per-side).
- **Appearance** begins below.

Left panel: **Pages** (Page 1) and **Layers** with semantic names (`Security Card`, `Security Card Title`, `Security Item`, `Security Bullet`, `Security Text`) and type icons (frame with row icon, dashed square, text T). The selected layer is a 4x4 `Security Bullet` with a 10 px top margin. The canvas shows two frames side by side ("Landing Page Optimized V2" next to the previous variant).

Not visible in the screenshot: whether fields show variable chips, what the Variables panel contains when expanded, any Brand panel.

### 10.2 What we copy and what differs

| Wonder (observed) | Meldframe [P] |
|---|---|
| Variables is a **collapsible section at the top of the right panel** with search and add | Same placement: quick access, search, add. Expands into the full table (below). |
| Properties grouped Position / Layout / Size / Spacing / Alignment / Padding / Margin | Same grouping and order (familiar to Wonder users, matches the round 2 model: margin, per-side padding). Padding/margin collapse to one value or expand to four. |
| Numeric fields are plain | Every bindable field has a **token affordance**: a small token icon at the field end; click opens a picker filtered by scope and type; a bound field shows a chip with the variable name and the resolved value on hover; detach icon unbinds to the literal. Dropdown also accepts typing `$` to search. |
| Layers use semantic names by habit | Same, plus **lint** (section 11). |
| Frames side by side as variants | Mode override per frame (4.2) lets a variant be the same frame in dark mode, without duplicating nodes. |
| (unknown) Brand | A **Brand panel**: a curated, guided view over the `brand` collections (10.4). |

### 10.3 Variables panel (expanded)

```
+-------------------------------------------------------+
| Variables                     [search] [+] [Import v]  |
| Collection: [Theme v]   Mode: ( Light | Dark ) [+ mode]|
+-------------------------------------------------------+
| color                                                  |
|  Name              Light             Dark       Scope  |
|  brand        (#) -> blue/600    (#) -> blue/400   fill|
|  bg           (#)  #ffffff       (#)  #0a0a0a      fill|
|  text         (#)  #0a0a0a       (#)  #fafafa     text |
| radius                                                 |
|  card              12                12           radius|
| space                                                  |
|  4                 16                16          spacing|
+-------------------------------------------------------+
| Collection: [Primitives v]   (hidden from pickers)     |
|  blue/600   #2563eb  ...                               |
+-------------------------------------------------------+
| 42 nodes use brand       [Find usages] [Detach all]    |
+-------------------------------------------------------+
```

Behaviors: create/rename/reorder (drag), group by slash prefix, alias picker (a chip `->` means alias), per-mode cells editable, a mode column shows `inherited` values dimmed, usage count from the reverse index, rename warns about export names, delete asks "Detach to literals" or cancel (the `detach` choice).

#### 10.3.1 Import variables and Export variables dialogs (D8)

Two buttons in the header of the Variables panel, **Import variables** and **Export variables**, each open a dialog. They are the owner's defaults of 21 Nov 2026 (`decisions.md` 38), reversible one by one.

- **Export variables** shows every variable of the file, not only the ones a frame uses, as CSS custom properties, one declaration per line: `:root { ... }` for the default mode of each collection and `:root[data-mode-<collection>="<mode>"] { ... }` for every other mode, with the names of ADR-0019 (`--<collection>-<name>`). A colour is the stored hex, lowercased, alpha included. The dialog copies the text or saves it as `variables.css`; nothing leaves the browser. CSS is the only format in v1: DTCG and `@theme` output are later, and the dialog says so. The text is built by `export_variables` of the `export` crate, by the app layer in WASM from the person's own view of the file (pending ops included), and it is built again only when the variables change.
- **Import variables** takes CSS text pasted into a field or read from a file the person chooses (read in the browser, never uploaded), and shows the result and its report: how many collections, modes, variables and aliases it made, and what was skipped and why. The text goes to the app layer as `Action::ImportVariables`; the same parser as the `import_tokens` tool reads it (ADR-0020: `:root`, a dark rule, `@theme`; nothing is run or fetched) and the commands are applied as **the person's own undoable step**, one undo takes the whole import back. No server route, no MCP and no change of the proxy guard come into it: the commands go out as ops, as any edit does.
- **It only adds.** Ids come from names (`col-root`, `col-theme`, `var-<collection>-<name>`), so a second import of names the file already holds is refused whole as `E_DUPLICATE_ID` and the result names the first, with nothing applied. Text with no variable the model can take says so (with the report) and applies nothing; empty text, text over 256 KiB (`E_IMPORT_TOO_LARGE`) and a list that does not fit the queue of ops (`outbox_full`) are refused whole. **The dialog takes at most 50 variables** (in at most 55 commands, `MAX_VARIABLES` and `MAX_COMMANDS` of `variables-import-dialog`): a text with more is refused whole as `E_IMPORT_TOO_MANY`, with a pointer to the agent tool `import_tokens`, which reads the same CSS in atomic parts of 100 commands. The ceilings of the import and of the file (256 KiB of text, 5,000 variables, 32 collections, 16 modes) hold on top of it.
- **Known limits, shown in the dialogs' help text.** (1) The round trip is not exact (ADR-0020): the export writes `--<collection>-<name>` and the import reads `--<name>`, so exported text imported again gives names with the collection in front (`appearance-color-brand`, not `color/brand`), one `root` collection, and only a mode that a root rule reads as light or dark (a mode named `Compact` is skipped with a line in the report). (2) A collection or variable the person made by hand with the name of an imported one is not a duplicate, because duplicates are found by the ids the import gives: two collections named `root` can exist. (3) The cap of 50 is there because the browser sends the ops of an import to the server one request each, and the server gives a client a burst of 60 requests and 30 a second after it (`meldframe-http`); an op it refuses is tried again 3 times and then rolled back with the ones that depend on it, so a longer import could be rolled back after the dialog had said it was done. "Imported" means applied in this page and sent: the server confirms it a moment later, and a refusal there takes it back as it does for any edit. The cost of confirming is the other reason (measured with the server's echoes, in a debug build): 1,000 variables imported in 0.37 s but the undo took 12.8 s, 2,000 variables took 65 s to undo, and 5,000 took 10.7 s to import and more than 7 minutes of CPU to undo. Follow-ups, not done: (i) the replica's cost of confirming ops grows about with the square of their number for an import and with its 2.5th to 3rd power for an undo; (ii) the dialog could send an import as one atomic call instead of one request for each op, and then the cap could rise to the 100 commands of a call. (4) The result of an import goes when the file no longer holds the variables it added (an undo, or someone else removing one), so an open dialog does not say "Imported" about a file that no longer has it.

### 10.4 Brand panel

A guided, opinionated page over the same collections, for people who do not want a table:

- **Palette**: roles (primary, secondary, accent, background, surface, text, muted, border, danger, success) with swatches per mode; "Generate scale" creates primitives (for example 50..950) from a base color [A: algorithm to decide].
- **Typography**: font families, a type scale (size/line-height/weight), live specimen.
- **Shape and space**: radius scale, spacing scale, shadow set.
- **Health**: unbound literals matching tokens (button "Bind all"), unused variables, contrast checks for text vs background roles in each mode.
- **Import / Export**: "Import from Tailwind config / CSS" and "Export CSS / Tailwind / DTCG".
- **Brand memory (Uteke)**: optional. If enabled, brand rules (concept section 8) are offered to agents as context; storage and sync are not designed here [Q9].

Brand = a presentation layer. It owns no extra data model; all state is in collections and variables, so MCP agents and the table view see the same truth.

## 11. Layer-name lint

Concept section 12: semantic names are a lint. V1 measured default-named nodes at 39% to 72% in unguided runs and 0% in guided runs, so guidance works but must be backed by a check. Rules (severity warn; `design_review` returns them; ruleset versioned) [P]:

| Rule | Flags | Suggestion |
|---|---|---|
| `name-default` | matches regex `^(Frame/Rectangle/Ellipse/Text/Group/Image/Shape/Layer)( \d+)?$` (alternatives separated by `/` here for table rendering) or empty | derive from role: first words of a text, parent name + role (`Pricing Card Title`), image alt |
| `name-numeric` | only digits or ends with a copy counter (`Card 2 copy`) | strip counter, suggest by content |
| `name-generic` | `container`, `wrapper`, `div`, `box`, `item` alone | parent name + role |
| `name-duplicate-sibling` | two siblings with the same name where they differ in content | add distinguishing word (allowed when they are intended list items) |
| `name-case` | mixed conventions in one frame (Title Case vs kebab) | normalize to the file's dominant style; Wonder's visible style is Title Case words |
| `name-length` | longer than 64 chars | truncate |

Provide `rename_layers` bulk (a batch of `UpdateNode{name}`) with suggestions. Open: require a non-default `name` on `addNode` from MCP (strict mode, [Q10]).

## 12. Impact on the round 2 model

Design every new field as `Bindable` from the start so tokens do not force a second migration (decision 19: schema versioning).

| Round 2 item | Token-related design |
|---|---|
| Margin (per side, per child) | `margin: Sides<Bindable<f64>>`; scope `spacing`; export as `margin-*` utilities or CSS; interplay with parent `gap` follows the portable layout subset (fidelity contract) |
| Padding per side | replaces uniform `padding: f64`; serialize as number (all sides), `[t,r,b,l]`, or `{top,right,bottom,left}`; each side bindable; migration: uniform value becomes four equal sides |
| Absolute children in auto layout | `ignoreAutoLayout` + constraints; offsets numeric, **not** bindable (position is not tokenized) |
| Gradients | `Paint` enum: `solid(Bindable<color>)`, `linear/radial{angle, stops:[{pos, color: Bindable<color>}]}`, `none`; V1 showed `fill` accepting arbitrary strings breaks rendering, so replace the string with `Paint` (bare hex accepted as shorthand for `solid` for compatibility); gradient composite tokens bind the whole paint |
| Opacity | `opacity: Bindable<f64>` 0..1, scope `opacity`; applies to node and subtree (document the compositing rule) |
| No fill | `Paint::None` (V1: default frame had an opaque `#3f3f46` and "no fill" could not be expressed) |
| Border, shadow | color and numbers bindable; `shadows` as a composite token |
| Typography | `font_size`, `font_weight`, `line_height`, `letter_spacing`, `font family` bindable; composite `typography` token |
| Node ids / names | unchanged; `name` is linted |
| Document | new top-level `variables` (collections, modes, variables) next to `nodes`; schema version bump with a migration that adds an empty set |

The export crate (`crates/export` in the spike: `to_html`, `to_tailwind`, `style.rs`) must emit `var(--...)` for bound fields; the spike export currently writes literals.

## 13. Tests and oracles

1. **Resolve oracle (differential)**: an independent TypeScript reference of `resolve` and layout-with-tokens versus the Rust core, as the spike did for layout (fixtures, random documents with aliases and modes). Include depth limit and fallbacks.
2. **Property tests (extend S12 harness)**: random variable commands interleaved with node commands across 3 to 5 replicas; invariants: convergence; no stored alias cycle; no stored dangling reference; undo/redo identity; undo never overwrites a later (variable, mode) write; `RemoveVariable{detach}` undo restores bindings only where unchanged.
3. **Export oracle (Chromium)**: for each mode, canvas rect/pixels versus exported page with the mode selector active; computed style uses `var(--...)` (assert the cascade, not just equal pixels, otherwise a literal export would pass).
4. **Tailwind compile oracle**: compile the exported v4 CSS with the Tailwind CLI and assert generated utility declarations; v3 config export compiled with a v3 CLI (not available in the spike tooling; to add).
5. **Import golden tests**: the corpus in 8.4, expected report and variables; fuzz the CSS scanner (never panics, bounded time and memory on hostile input); no code execution (a config containing `require('child_process')` is rejected as text, not run).
6. **Round trip**: export then import equals the original (names, values, aliases, modes), within documented lossy cases.
7. **DTCG shape test**: validate our output against the published structure (a JSON-shape check written from the spec, since a conformance suite was not verified to exist).
8. **Mutation checks**: remove the cycle check, the type check or the `detach` rewrite and assert tests fail.
9. **Perf**: 10,000 nodes, 500 variables, edit one primitive: re-resolve and repaint time budget [A: under 16 ms for 1,000 affected nodes].
10. **MCP e2e with a real agent** (like V1): "re-skin the page in dark mode by changing variables only" and "bind literals to tokens"; measure whether agents use `$name` binding without being told.

## 14. Implementation order

| Phase | Content | Depends on |
|---|---|---|
| T0 | Round 2 model fields as `Bindable`-ready (`Paint`, per-side padding/margin, opacity) with literals only; migration; renderer and export updated | none |
| T1 | Variables in core: collections, modes, variables, aliases, validation, `resolve`, commands, undo, property tests (decide Q6 here) | T0 |
| T2 | Binding in commands and layout/render; inspector token picker; Variables panel (table); mode switch in the toolbar | T1 |
| T3 | Export: CSS variables, Tailwind v4 (`@theme`, `@theme inline`), oracles | T1, T2 |
| T4 | Import: CSS variables and v4 `@theme`, shadcn corpus, report; MCP `import_tokens`/`list_variables`/`bind_tokens` | T1 |
| T5 | MCP surface and agent guidance; lint (layer names, token rules) in `design_review`; V2 against Wonder uses tokens | T2, T4 |
| T6 | Composites (shadow, gradient, typography); frame mode override; Brand panel | T2 |
| T7 | Tailwind v3 config export, resolved-config import, DTCG export/import | T3, T4 |

T0 to T3 should be inside W1 (concept spike order: model round 2, token import, HTML/Tailwind + CSS variables export). T4 corresponds to "token import" in W1.

## 15. Decisions

**Proposed [P]**

1. Flat model: collections, modes, variables, aliases, scopes. Primitive/semantic/component is convention plus lint.
2. Variable ids are opaque and stable; names are lowercase slash paths, unique per collection.
3. Types: color, number, string now; shadow, gradient, typography as composite later. Scopes are filters/lint, not security.
4. Bindable fields are `T | {var: id}`; MCP accepts a name shorthand resolved at the edge; the log stores ids.
5. Every variable has a value for every mode (inherited copy), alias resolution uses the consumer's mode, depth cap 16, cycles rejected at command time.
6. Variable edits are ordinary commands under the same undo, `ifSeq`, atomic batch and review rules; field-level unit is (variable, mode).
7. Deleting a used variable requires an explicit `detach`.
8. Resolution is one core function used by layout, render, export and tests; the canvas never uses CSS variables.
9. Export keeps aliases and emits `var(--...)` for bound fields; Tailwind v4 via `@theme` and `@theme inline`; v3 and DTCG later; document is the source of truth.
10. Import takes text only (no server-side code execution); agent-assisted resolution for JS configs; always returns a report; re-import never deletes.
11. Panels: Variables (table, Wonder-like placement), Brand (guided view, no extra data), token affordance on every bindable field.
12. Layer-name lint ruleset as in section 11.

**Open [Q]**

- Q1 Component tokens: later, or never (just naming)?
- Q2 Name uniqueness per collection or global (affects CSS name collisions across collections)?
- Q3 Color space: sRGB hex only now, or store OKLCH/P3 from the start (Tailwind v4 default palette is OKLCH, Wonder lists P3/OKLCH)? Needs a canvas rendering decision with PixiJS. The display default is OKLCH (decision 28); storage and rendering are still open.
- Q4 Does `size` get its own namespace or share `spacing`?
- Q5 Global mode switch as view state only, or persisted per file?
- Q6 Generalize the entity store (variables reuse node machinery) or bespoke commands?
- Q7 Default export unit: px or rem?
- Q8 Import Tailwind `screens` as frame presets?
- Q9 Brand memory (Uteke) scope and storage.
- Q10 Require non-default `name` on `addNode` from MCP (strict)?
- Q11 Per-file variable limits and whether workspace-level shared tokens (one design system across files) are needed. Likely yes for a brand ("workspace is the tenant"), but it changes where variables live (workspace-level collection referenced by files). Not designed here.
- Q12 Should detach-on-delete bake the default mode value or the current view mode value? Built (flagged for the owner): a node field bakes the default-mode value, an alias the value of its own mode (see 5.2); the mode on view is never used, so a detach does not depend on what the person happens to be looking at.

## 16. Sources

Spike and project (read 13 Nov 2026): `wonder-proto` branch `spike/rust-core`: `crates/protocol/src/lib.rs` (Node fields), `docs/s9-design-code.md` (export fidelity, model gaps), `docs/s12-multiplayer-undo.md`, `docs/v1-agent-design-eval.md`; project notes (team knowledge rooms): concept v1 section 12, and the research notes on Wonder pricing, MCP and ACP and on design-tool competitors; owner screenshot `20.png`.

Web (accessed 13 Nov 2026). **Direct fetches were blocked in this environment** (`help.figma.com`, `developers.figma.com`, `tailwindcss.com`, `www.designtokens.org`, `www.w3.org`, `penpot.app`, `www.mintlify.com` did not resolve), so everything below is from **search-result summaries, not the pages themselves**, and anything not marked otherwise is **unverified at the source**:

- Figma variables, modes, aliases, scopes: https://help.figma.com/hc/en-us/articles/15339657135383-Guide-to-variables-in-Figma (title only, stub), https://developers.figma.com/docs/rest-api/variables-types, https://developers.figma.com/docs/plugins/api/VariableScope (official docs, scope enum details from snippets; BOOLEAN scope list uncertain), forum report that imported variables report `ALL_SCOPES`: https://forum.figma.com/t/variables-imported-with-importvariablebykeyasync-always-have-scopes-set-to-all-scopes/56906 (unverified).
- Tailwind v4 `@theme`: https://tailwindcss.com/docs/theme (title only), secondary: https://www.mintlify.com/tailwindlabs/tailwindcss/api/css/at-theme, https://noqta.tn/en/tutorials/tailwind-css-v4-complete-guide-2026; upgrade guide https://tailwindcss.com/docs/upgrade-guide and https://mintlify.com/tailwindlabs/tailwindcss/upgrading/upgrade-guide. Conflicts between sources on `@theme static` and on JS-config plugin support are open.
- DTCG Format Module 2025.10 (Final Community Group Report, 28 Oct 2025): https://www.w3.org/community/reports/design-tokens/CG-FINAL-format-20251028/ and Resolver Module https://www.w3.org/community/reports/design-tokens/CG-FINAL-resolver-20251028/ and announcement https://www.w3.org/community/design-tokens/2025/10/28/design-tokens-specification-reaches-first-stable-version/ (status and date from search snippets). `$ref`/`$extends` usage unverified.
- Penpot tokens: https://help.penpot.app/user-guide/design-tokens/ and community post on export format https://community.penpot.app/t/token-export-is-tokens-studio-format-not-dtcg-2025-10/10544 (unverified claim that export is Tokens Studio format).
- Own knowledge (not checked in this session): shadcn/ui Tailwind v4 pattern (`:root`/`.dark` variables + `@theme inline`), Tailwind v3 `<alpha-value>` channel form, Tailwind default namespaces. Verify before implementation.

## 11. Alignment with architecture decisions (13 Nov 2026)

Decisions made after this draft (`docs/decisions.md` items 20 to 24; ADR-0004, ADR-0007, ADR-0008):

- **Split by layer.** Variable entities, bindings and the `resolve` function belong to the core and are not detachable. Import (Tailwind config, CSS variables), export (CSS variables, Tailwind) and the Variables and Brand panels are a `variables` feature module that registers its commands, agent tools (for example `import_tokens`) and panels in the module registry (ADR-0007).
- **Panel state in the app layer.** The Variables panel table, the mode switch and the token picker keep their rules in `meldframe-app` (ADR-0008); the web client only renders them.
- **UI uses tokens, not literals.** The editor's own colours, fonts and sizes come from a tokens module with a literal lint; this is separate from the user's design variables.
- **Export consumes the render model.** Export emits `var(--...)` from resolved bindings; the oracle in `docs/fidelity-contract.md` compares resolved values.
- **No separate spikes.** The prototype of variables as an entity table that reuses the node patch machinery (Q6) is decided while building the first variables slice in the repository, with written exit criteria.
