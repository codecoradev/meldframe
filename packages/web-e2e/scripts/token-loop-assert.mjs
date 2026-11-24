// Import to render agreement (docs/tokens-and-code.md): after import_tokens, every variable an exported frame uses has, in the
// exported CSS, the value get_variables reads back, and a bound node's rule uses var(--<collection>-<name>).
//   MELDFRAME_TOKEN=<agent token> MELDFRAME_FILE=<a file name that is new> node packages/web-e2e/scripts/token-loop-assert.mjs
// Needs a running server with an agent token (see token-loop-demo.mjs). Env: MELDFRAME_SERVER (default http://127.0.0.1:8123),
// MELDFRAME_TOKEN, MELDFRAME_FILE (default token_loop_assert). The fixture and the ids are fixed, so the file must be new:
// a file that already holds them is refused (E_DUPLICATE_ID). Prints one PASS line per variable and exits non-zero on a mismatch.
import assert from 'node:assert/strict';

const server = process.env.MELDFRAME_SERVER ?? 'http://127.0.0.1:8123';
const token = process.env.MELDFRAME_TOKEN;
const file = process.env.MELDFRAME_FILE ?? 'token_loop_assert';
if (!token) throw new Error('set MELDFRAME_TOKEN to an agent token (see the top of this file)');
let rpc = 0;

async function tool(name, args) {
  const res = await fetch(`${server}/mcp`, {
    method: 'POST',
    headers: { 'content-type': 'application/json', accept: 'application/json, text/event-stream', authorization: `Bearer ${token}` },
    body: JSON.stringify({ jsonrpc: '2.0', id: ++rpc, method: 'tools/call', params: { name, arguments: { file, ...args } } }),
  });
  return JSON.parse((await res.json()).result?.content?.[0]?.text ?? '{}');
}
const ok = (what, r) => assert.ok(!r.error, `${what} failed: ${JSON.stringify(r).slice(0, 400)}`);

const css = `:root {
  --brand: #1D4ED8;
  --surface: #ffffff;
  --ink: rgb(17 24 39);
  --accent: var(--brand);
}
.dark {
  --surface: #0b1220;
}
`;
ok('import_tokens', await tool('import_tokens', { css, dryRun: false, idempotencyKey: 'assert-import' }));

const vars = (await tool('get_variables', {}));
ok('get_variables', vars);
const byName = Object.fromEntries(vars.variables.map((v) => [v.name, v]));
const collectionName = Object.fromEntries(vars.collections.map((c) => [c.id, c.name]));
const defaultMode = Object.fromEntries(vars.collections.map((c) => [c.id, c.modes[0].id]));
const cssName = (v) => `--${collectionName[v.collection]}-${v.name.replaceAll('/', '-')}`;

// A frame whose fill, and three texts whose colour, are bound: a literal, an rgb() that was converted, and an alias.
const ids = { frame: 'assert_card', surface: 'assert_surface_text', ink: 'assert_ink_text', accent: 'assert_accent_text' };
const text = (id, y, v) => ({ type: 'addNode', node: { id, type: 'text', name: id, parentId: ids.frame, x: 16, y, w: 200, h: 24, text: id, fontSize: 16, fill: { var: v.id } } });
ok('apply_commands', await tool('apply_commands', {
  idempotencyKey: 'assert-build',
  commands: [
    { type: 'addNode', node: { id: ids.frame, type: 'frame', name: 'Card', parentId: null, x: 0, y: 0, w: 320, h: 160, fill: { var: byName.surface.id } } },
    text(ids.surface, 16, byName.surface),
    text(ids.ink, 48, byName.ink),
    text(ids.accent, 80, byName.accent),
  ],
}));

const exported = await tool('export_frame', { frame: ids.frame, format: 'css' });
ok('export_frame', exported);
const html = exported.html;
const rule = (selector) => html.match(new RegExp(`${selector.replace(/[[\]"=.]/g, '\\$&')}\\{([^}]*)\\}`))?.[1];
const rootDecls = Object.fromEntries([...(rule(':root') ?? '').matchAll(/(--[\w-]+):([^;]+)/g)].map((m) => [m[1], m[2]]));

// Every variable the frame uses: those named by a var(--...) in a node rule.
const used = new Set([...html.matchAll(/(?:background-color|color):var\((--[\w-]+)\)/g)].map((m) => m[1]));
assert.ok(used.size > 0, 'the export binds no variable');
let failed = 0;
const check = (label, fn) => {
  try {
    fn();
    console.log(`PASS ${label}`);
  } catch (e) {
    failed++;
    console.log(`FAIL ${label}: ${e.message.split('\n')[0]}`);
  }
};
for (const name of ['surface', 'ink', 'accent']) {
  const v = byName[name];
  const prop = cssName(v);
  check(`${prop} is used by a bound rule`, () => assert.ok(used.has(prop), `no rule uses var(${prop})`));
  check(`${prop} equals get_variables`, () => {
    const read = v.values[defaultMode[v.collection]];
    const want = typeof read === 'string' ? read.toLowerCase() : `var(${cssName(vars.variables.find((x) => x.id === read.var))})`;
    assert.equal(rootDecls[prop]?.toLowerCase(), want);
  });
}
for (const [node, name] of [[ids.frame, 'surface'], [ids.surface, 'surface'], [ids.ink, 'ink'], [ids.accent, 'accent']]) {
  check(`node ${node} rule uses var(${cssName(byName[name])})`, () => assert.ok(rule(`[data-node="${node}"]`)?.includes(`var(${cssName(byName[name])})`)));
}
// The alias pulls its target into the export, so the target is checked too.
check(`${cssName(byName.brand)} (target of the alias) equals get_variables`, () => assert.equal(rootDecls[cssName(byName.brand)]?.toLowerCase(), byName.brand.values.light.toLowerCase()));

if (failed) {
  console.log(`${failed} check(s) failed`);
  process.exit(1);
}
console.log('TOKEN LOOP ASSERT OK');
