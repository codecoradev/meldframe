// The token loop (docs/tokens-and-code.md) end to end over MCP, as an agent would run it:
//   MELDFRAME_TOKEN=<agent token> node packages/web-e2e/scripts/token-loop-demo.mjs
// Needs a running server with an agent token (`just dev-server`, or the binary with MELDFRAME_LISTEN and MELDFRAME_DEV_TOKENS set).
// No browser, no dependencies: raw JSON-RPC to `/mcp`. Env: MELDFRAME_SERVER (default http://127.0.0.1:8123), MELDFRAME_TOKEN,
// MELDFRAME_FILE (default file_dev). Ids carry a random run suffix, so a second run on the same file adds its own nodes. The import
// is once per file: the same call again is a replay (the key is fixed), and a file that holds other variables answers
// E_DUPLICATE_ID, which step 1 reports and goes on. Use a new file name for a clean run.
// Prints one line per step and exits non-zero if a step fails.
import assert from 'node:assert/strict';

const server = process.env.MELDFRAME_SERVER ?? 'http://127.0.0.1:8123';
const token = process.env.MELDFRAME_TOKEN;
const file = process.env.MELDFRAME_FILE ?? 'file_dev';
if (!token) throw new Error('set MELDFRAME_TOKEN to an agent token (see the top of this file)');
const run = Math.random().toString(36).slice(2, 7);
let rpc = 0;

/** One MCP tool call; the result's JSON body (or the error body). */
async function tool(name, args) {
  const res = await fetch(`${server}/mcp`, {
    method: 'POST',
    headers: { 'content-type': 'application/json', accept: 'application/json, text/event-stream', authorization: `Bearer ${token}` },
    body: JSON.stringify({ jsonrpc: '2.0', id: ++rpc, method: 'tools/call', params: { name, arguments: { file, ...args } } }),
  });
  const part = (await res.json()).result?.content?.[0];
  return JSON.parse(part?.text ?? '{}');
}
const step = (n, text) => console.log(`${n}. ${text}`);
const ok = (what, r) => assert.ok(!r.error, `${what} failed: ${JSON.stringify(r).slice(0, 400)}`);

// The project's tokens.css, as the agent read it: three colours, a dark override and an alias.
const css = `:root {
  --brand: #1d4ed8;
  --surface: #ffffff;
  --ink: #111827;
  --accent: var(--brand);
}
.dark {
  --surface: #0b1220;
}
`;

// 1. Import. Once per file: a second import is E_DUPLICATE_ID, so a repeat run reads what the first one wrote.
const imported = await tool('import_tokens', { css, dryRun: false, idempotencyKey: `tokens-${file}` });
if (imported.error?.code === 'E_DUPLICATE_ID') step(1, 'the file already holds these tokens (an earlier run); reading them back');
else {
  ok('import_tokens', imported);
  const r = imported.report;
  step(1, `import_tokens ${imported.replayed ? 'replayed (nothing written again)' : 'wrote'} ${imported.commands} commands (${r.colors} colours, ${r.aliases} alias, ${r.entries.length} report entries)`);
}

// 2. Read back.
const vars = await tool('get_variables', {});
ok('get_variables', vars);
const byName = Object.fromEntries(vars.variables.map((v) => [v.name, v]));
for (const n of ['brand', 'surface', 'ink', 'accent']) assert.ok(byName[n], `variable ${n} is missing: ${Object.keys(byName)}`);
const root = vars.collections.find((c) => c.id === 'col-root');
assert.deepEqual(root.modes.map((m) => m.name), ['light', 'dark']);
step(2, `get_variables: collection ${root.name} (modes ${root.modes.map((m) => m.name).join(', ')}), ${vars.variables.length} variables: ${vars.variables.map((v) => v.name).join(', ')}`);

// 3. Bind a frame and a text to variables with {var: id}.
const frame = `card_${run}`, label = `label_${run}`;
const built = await tool('apply_commands', {
  idempotencyKey: `card-${run}`,
  commands: [
    { type: 'addNode', node: { id: frame, type: 'frame', name: 'Card', parentId: null, x: 0, y: 0, w: 320, h: 120, fill: { var: byName.surface.id } } },
    { type: 'addNode', node: { id: label, type: 'text', name: 'Title', parentId: frame, x: 16, y: 16, w: 200, h: 24, text: 'Token loop', fontSize: 16, fill: { var: byName.ink.id } } },
  ],
});
ok('apply_commands', built);
step(3, `a frame bound to ${byName.surface.id} and a text bound to ${byName.ink.id} (seq ${built.seq})`);

// 4. Export; the variables come out as custom properties and the bound fills as var(--...).
const first = await tool('export_frame', { frame, format: 'css' });
ok('export_frame', first);
const block = (html) => html.match(/:root[^{]*\{[^}]*\}/g)?.join('\n') ?? '';
assert.ok(first.html.includes('var(--'), 'the export has no var(--...)');
step(4, `export_frame (css) has var(--...); the variable rules:\n${block(first.html)}`);

// 5. Two-way: change one value with a variable command, export again, the value moved.
// surface and ink are bound by the frame; only variables a frame uses are exported. A repeat run flips the value.
const next = byName.surface.values.light.toLowerCase() === '#fef3c7' ? '#ffffff' : '#fef3c7';
const lightId = root.modes.find((m) => m.name === 'light').id;
const changed = await tool('apply_commands', {
  idempotencyKey: `surface-${run}`,
  commands: [{ type: 'setVariableValue', id: byName.surface.id, mode: lightId, value: next }],
});
ok('setVariableValue', changed);
const second = await tool('export_frame', { frame, format: 'css' });
ok('export_frame', second);
assert.ok(second.html.includes(next), `the export does not carry ${next}`);
assert.ok(!first.html.includes(next), 'the first export already had the new value');
step(5, `setVariableValue surface (light) -> ${next}; the second export carries it:\n${block(second.html)}`);

console.log('TOKEN LOOP OK');
