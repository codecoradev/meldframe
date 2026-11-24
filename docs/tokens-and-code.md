# Tokens and code: the full loop

How a project's CSS variables get onto the canvas, and how a design gets back into code, through the MCP surface of the server. Every step here is a tool call an agent (Claude Code, Codex, Cursor) makes; there is no command-line product and no plugin. The model behind it is in [tokens-and-variables.md](tokens-and-variables.md). The full text of each tool is in [mcp-tools.json](mcp-tools.json).

This page states only what a script in the repository runs:

- [`packages/web-e2e/scripts/token-loop-demo.mjs`](../packages/web-e2e/scripts/token-loop-demo.mjs) runs the loop once and prints each step.
- [`packages/web-e2e/scripts/token-loop-assert.mjs`](../packages/web-e2e/scripts/token-loop-assert.mjs) checks that the exported CSS variables hold the values `get_variables` reads back.
- [`packages/web-e2e/scripts/token-loop-canvas-e2e.mjs`](../packages/web-e2e/scripts/token-loop-canvas-e2e.mjs) checks, in a real browser, that the canvas and the exported page draw the same pixels for the bound values (see "Canvas and export draw the same values" below).

The first two need a running server and an agent token:

```sh
MELDFRAME_LISTEN=127.0.0.1:8123 MELDFRAME_DEV_TOKENS="<token>:ws_dev:agent:claude" ./target/debug/meldframe-server
MELDFRAME_TOKEN=<token> MELDFRAME_FILE=<a new file name> node packages/web-e2e/scripts/token-loop-demo.mjs
MELDFRAME_TOKEN=<token> MELDFRAME_FILE=<a new file name> node packages/web-e2e/scripts/token-loop-assert.mjs
```

The token is 16 bytes or more. `MELDFRAME_SERVER` defaults to `http://127.0.0.1:8123`. See the Development section of [AGENTS.md](../AGENTS.md) for `just dev-server`.

A statement below that no script runs is marked "not run by the scripts".

## 1. CSS to canvas

The agent reads the project's `tokens.css`, sends the text to `import_tokens`, checks what landed with `get_variables`, and renders.

Prompt to paste into your agent:

> Read `src/styles/tokens.css`. If it has `@import` lines, a `url()` or a `tailwind.config.js` that holds values, resolve them and join everything into one CSS text. Call `import_tokens` on the Meldframe file `<file>` with that text, `dryRun: true` first, and read the report. Tell me what was converted and what was skipped. If I agree, call it again with `dryRun: false` and the same `idempotencyKey`. Then call `get_variables` and list the collections, their modes and every variable. Do not bind any node until you have read the report.

Then:

> Add a frame `Preview` to the file and bind its fill to the variable `surface` and a text inside it to `ink`. Capture it and show me.

### What `import_tokens` accepts

- The custom properties of `:root`.
- The custom properties of a rule that picks the dark mode: `.dark`, `[data-theme=dark]` or a `prefers-color-scheme: dark` query.
- The custom properties of a Tailwind v4 `@theme` block.

Nothing is executed or fetched. The text is read. The agent resolves `@import`, `url()` and `tailwind.config.js` first. The limit is 256 KiB of text (`E_IMPORT_TOO_LARGE` above it).

The demo's fixture is a `:root` block with three colours and an alias, plus one `.dark` override. The other two dark forms and `@theme` are described in the tool text; the scripts do not run them.

### What lands in the file

- `@theme` variables go to a collection `theme` (id `col-theme`) with one mode, `default`.
- `:root` variables go to a collection `root` (id `col-root`) with the mode `light` and, when a dark rule sets any, `dark`. A variable the dark rule does not set keeps its `light` value in `dark`.
- A name is lowercase. A Tailwind namespace is split off with a slash: `--color-brand-500` is `color/brand-500`.
- A value that is only `var(--x)` becomes an alias of that variable.
- A hex colour is kept as written. `rgb`, `hsl`, `oklch` and `oklab` are converted to hex; a colour outside sRGB is clamped. `rem` is 16 px. Variables are `color` or `number`.

The demo's import of its fixture, as run:

```text
1. import_tokens wrote 5 commands (4 colours, 1 alias, 2 report entries)
2. get_variables: collection root (modes light, dark), 4 variables: brand, surface, ink, accent
```

### What is skipped

Shadows, font families, strings, and computed values (`calc`, `clamp`, `color-mix`) are skipped. The report lists each skip and each conversion, with a reason, under `entries`. Read it before binding. A skip is not an error: the call succeeds without that variable.

### `dryRun`, parts and `idempotencyKey`

- `dryRun: true` writes nothing and returns the report and the number of `parts`.
- A call writes at most 100 commands. A longer import comes in parts: send the same call again with `part: 2`, then `3`, with the same `idempotencyKey` and `css`, in order. Each part is atomic; the whole import is not.
- Every call needs an `idempotencyKey`. The same call again after a timeout writes nothing twice (`replayed: true`).
- The demo sends one part with `dryRun: false`. A dry run and several parts are not run by the scripts.

### One import per file

The collections are always `root` and `theme`, so a second import into the same file fails with `E_DUPLICATE_ID`. If you have several CSS files, join them into one text. To change what a file holds later, use `apply_commands` with variable commands (section 3), not a second import. To finish an import that stopped part way, send the same call again.

## 2. Canvas to code

Design with variables bound, then export the frame.

Prompt to paste:

> In the Meldframe file `<file>`, build the card as a frame with a text inside it. Bind the frame's fill to the variable `surface` and the text's colour to `ink` with `updateNode` or `addNode` and `{var: <id>}`; take the ids from `get_variables`. Do not write a hex colour where a variable exists. Then call `export_frame` for the frame with `format: "css"` and write the `<style>` content into `src/styles/card.css`. Tell me what the report says under `unsupported` and `warnings`.

A bound node field holds `{var: <id>}` instead of a literal. The scripts bind with `addNode`; `updateNode` with a patch is the other way, as the `get_variables` text says.

### What the export looks like

`export_frame` returns a whole HTML document. The variables the frame uses become CSS custom properties on `:root`, named `--<collection>-<variable>` with the slash written as a dash, and one extra rule per mode other than the default. A bound field is `var(--...)`. From a run of the demo (a frame and a text, both bound; the node ids carry a random suffix):

```css
:root{--root-surface:#ffffff;--root-ink:#111827}
:root[data-mode-root="dark"]{--root-surface:#0b1220;--root-ink:#111827}
[data-node="card_7l7lo"]{position:absolute;left:0px;top:0px;width:320px;height:120px;overflow:hidden;background-color:var(--root-surface)}
[data-node="label_7l7lo"]{position:absolute;left:16px;top:16px;width:200px;height:24px;font-size:16px;font-weight:400;line-height:20px;white-space:pre-wrap;word-break:keep-all;color:var(--root-ink)}
```

Only the variables a frame uses are exported (a variable no node binds is not in the output). The `:root[data-mode-<collection>="<mode>"]` rule is how the other modes are selected: set that attribute on the `html` element to see the mode. The `modes` argument (collection id to mode id) exports a mode other than the default.

The assertion script checks, for a literal, an `rgb()` that the import converted, and an alias, that the exported declaration equals the value `get_variables` reads (an alias is exported as `var(--target)`), and that each bound node's rule uses that `var(--...)`.

Binding a radius or an opacity to a `number` variable is described in the tool text (`var(--...)` in the rule). The first two scripts bind colours only; the canvas script below binds a radius and an opacity too. `format: "tailwind"` is not run by the scripts.

### Canvas and export draw the same values

The assertion script reads text. [`token-loop-canvas-e2e.mjs`](../packages/web-e2e/scripts/token-loop-canvas-e2e.mjs) reads pixels: it imports the same CSS (with a radius and an opacity added), binds a frame's fill, radius and opacity to the variables, takes a `capture` of the frame from an editor open in Chromium, loads the `export_frame` page in the same browser, and asserts that at six points the two pixels are equal within 4/255 per channel and are the colour the CSS gives. Then it sets three values with `setVariableValue` and asserts again, so the canvas and the export both follow. It starts its own in-memory server and page, so it needs no token; run it by hand from the repository root (the header lists what it needs: the server binary, the WASM bridge, `npm ci` in `apps/web` and Playwright):

```sh
node packages/web-e2e/scripts/token-loop-canvas-e2e.mjs
```

From a run (an excerpt of the 17 checks it prints):

```text
ok   imported: card fill at the bound opacity: canvas rgb(119,149,232), export rgb(119,149,232), the values give rgb(119,149,232) (canvas vs export 0/255)
ok   after setVariableValue: card fill at the bound opacity: canvas rgb(203,176,248), export rgb(203,176,248), the values give rgb(203,176,248) (canvas vs export 0/255)
ok   after setVariableValue: bar fill (an alias of brand): canvas rgb(124,58,237), export rgb(124,58,237), the values give rgb(124,58,237) (canvas vs export 0/255)
largest difference between a canvas pixel and an exported pixel: 0/255 per channel (tolerance 4)
TOKEN LOOP CANVAS OK
```

It does not check the dark mode on the canvas, text, or `format: "tailwind"`.

### Where to put it

- Take the `:root` rules (the variable block) into the project's global stylesheet, or a file it imports. They replace the hand-written block that `tokens.css` had, or sit beside it.
- Take the node rules into the component's stylesheet. The `data-node` selectors and the absolute positions are the canvas's boxes; adapt them to the component's markup.
- The font is `inter.woff2` next to the document. Images are referenced, not included (see the limits).

## 3. Two-way update

The loop is request-driven. Nothing watches your files and nothing pushes to them. Each direction runs when you ask your agent to run it.

Decide the source of truth first, per change:

- **The CSS is the source.** Change `tokens.css`, then change the file with variable commands. Do not import again.
- **The canvas is the source.** Change the variable in the file, then export and copy the rules into the project.

Prompt for CSS to canvas:

> `tokens.css` changed. Read it and call `get_variables` on the Meldframe file `<file>`. For each variable whose value differs, call `apply_commands` with a `setVariableValue` command (`id`, `mode`, `value`). For a variable the file does not hold, say so and use an `addVariable` command. For a variable the file holds and the CSS no longer has, tell me and do not remove it. Do not call `import_tokens`: the file already holds an import. List every value you could not map.

Prompt for canvas to CSS:

> In the Meldframe file `<file>` I changed variables on the canvas. Call `get_variables`, compare each value with `src/styles/tokens.css`, and update the CSS file for the differences. Name the source of truth in your answer. List every variable that exists on one side only, and every value you could not write as CSS.

The demo's step 5 runs the change on the canvas side and shows the value move:

```text
5. setVariableValue surface (light) -> #fef3c7; the second export carries it:
:root{--root-surface:#fef3c7;--root-ink:#111827}
:root[data-mode-root="dark"]{--root-surface:#0b1220;--root-ink:#111827}
```

Before that step (step 4) the export held `--root-surface:#ffffff`. The command is `{ type: 'setVariableValue', id, mode, value }`; `mode` is a mode id (`light`, `dark`), as `get_variables` lists them. `apply_commands` is atomic, takes an `idempotencyKey`, and goes into the undo history of its actor.

What the agent must report, because the two sides do not map one to one:

- Imports that were skipped (shadows, fonts, computed values): they exist in the CSS and not in the file.
- Variables the file has and the CSS does not.
- Values that were converted, such as `oklch` to hex: writing them back gives the hex, not the original text.

## Honest limits

- Variable types are `color` and `number` only. A shadow, a font family or a string has no variable.
- Export text sizes come from the server's stub measurer (half an em per character, no weight), not from the browser. A text whose box the stub decides is named in one `text_size_estimated` warning. Its absence does not make a line height exact. Do not hand the sizes on as exact. The editor's Export dialog measures with the real font; see [fidelity-contract.md](fidelity-contract.md) for what the export promises and what the oracle checks.
- `export_frame` reports what it could not carry under `unsupported`: a shader is approximated by a gradient, a border or an image on text is dropped, and a border on a frame that clips is clipped.
- Images are referenced by id under `assets`; the files are not inlined or included.
- A frame whose export is over the size limit is refused as `E_EXPORT_TOO_LARGE`. Export a smaller frame.
- Import skips what the model cannot hold (see "What is skipped"), converts colours outside sRGB by clamping, and takes one import per file.
- Sync is on request. No script here watches a file.
- Not run by the scripts: `dryRun` and multi-part imports, `@theme`, `[data-theme=dark]` and `prefers-color-scheme` input, `format: "tailwind"`, and the `modes` argument of `export_frame`.

## Brand memory from Uteke (optional)

Meldframe does not know Uteke and does not call it. The pattern lives in your agent's configuration: the agent holds a second MCP connection to your Uteke instance, reads your brand room there, and feeds the colours it finds into step 1. The Meldframe server only sees the CSS text the agent sends to `import_tokens`.

Prompt to paste:

> You have two MCP connections: Meldframe and Uteke. Read the Uteke room `brand:acme` and list every colour with its name. Write them as a `:root` block of custom properties, show it to me, and after I agree call `import_tokens` on the Meldframe file `<file>` as in step 1. Do not write to Uteke.

Recording tokens into the brand room (done by you, once). Uteke takes `Authorization: Bearer`; an `X-API-Token` header is refused. Set a `User-Agent`.

```sh
curl -X POST "$UTEKE_CLOUD_URL/api/v2/room/remember" \
  -H "Authorization: Bearer $UTEKE_CLOUD_TOKEN" \
  -H "User-Agent: brand-notes" \
  -H "Content-Type: application/json" \
  -d '{"room_id": "brand:acme", "content": "brand: #1d4ed8, surface: #ffffff, ink: #111827", "tags": ["brand", "colour"]}'
```

[`packages/web-e2e/scripts/uteke-brand-demo.mjs`](../packages/web-e2e/scripts/uteke-brand-demo.mjs) does the reading half without an agent: it reads the room (`GET /api/v2/room/memories?room_id=brand:acme&limit=50`), picks out hex colours with a regular expression, prints them, and, when `MELDFRAME_TOKEN` is also set, sends them to `import_tokens`. It needs `UTEKE_CLOUD_URL` and `UTEKE_CLOUD_TOKEN` and has no defaults. It is a demo aid and is not part of any test run. The `curl` call and the shape of the room's answer are not run by the scripts of this repository against a live Uteke.
