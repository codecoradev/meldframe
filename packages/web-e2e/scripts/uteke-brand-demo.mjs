// Brand memory from Uteke (docs/tokens-and-code.md, last section): read the hex colours a Uteke brand room holds and, when a
// meldframe server is also given, send them through import_tokens. A demo aid, not a test: it is not part of any npm script.
//   UTEKE_CLOUD_URL=<url> UTEKE_CLOUD_TOKEN=<token> [UTEKE_ROOM=brand:acme] node packages/web-e2e/scripts/uteke-brand-demo.mjs
//   ... plus MELDFRAME_TOKEN=<agent token> [MELDFRAME_SERVER=http://127.0.0.1:8123] [MELDFRAME_FILE=<new file>] to import them.
// The Uteke leg runs on its own. No defaults for the Uteke URL and token: unset, the script says so and exits 1. Uteke is
// called with `Authorization: Bearer` and a User-Agent (an X-API-Token header is refused). The meldframe server knows nothing of Uteke:
// this script, like an agent with a second MCP connection, is the only thing that talks to both.
import assert from 'node:assert/strict';

const { UTEKE_CLOUD_URL: base, UTEKE_CLOUD_TOKEN: utekeToken } = process.env;
if (!base || !utekeToken) {
  console.error('set UTEKE_CLOUD_URL and UTEKE_CLOUD_TOKEN (no defaults): this script reads a Uteke brand room and has nothing to read without them');
  process.exit(1);
}
const room = process.env.UTEKE_ROOM ?? 'brand:acme';

const res = await fetch(`${base.replace(/\/$/, '')}/api/v2/room/memories?room_id=${encodeURIComponent(room)}&limit=50`, {
  headers: { authorization: `Bearer ${utekeToken}`, 'user-agent': 'meldframe-uteke-brand-demo' },
});
if (!res.ok) {
  console.error(`Uteke answered ${res.status} for room ${room}: ${(await res.text()).slice(0, 300)}`);
  process.exit(1);
}
const body = await res.json();
const memories = Array.isArray(body) ? body : (body.memories ?? body.items ?? body.results ?? []);
const contents = memories.map((m) => (typeof m === 'string' ? m : (m.content ?? ''))).join('\n');
// A name next to a hex colour ("brand: #1d4ed8", "--brand: #1d4ed8"); a colour with no name gets a numbered one.
const found = new Map();
for (const m of contents.matchAll(/(?:--)?([a-z][\w-]*)\s*[:=]\s*(#(?:[0-9a-f]{8}|[0-9a-f]{6}|[0-9a-f]{3,4})\b)/gi)) found.set(m[1].toLowerCase(), m[2].toLowerCase());
for (const m of contents.matchAll(/#(?:[0-9a-f]{8}|[0-9a-f]{6}|[0-9a-f]{3,4})\b/gi)) {
  const hex = m[0].toLowerCase();
  if (![...found.values()].includes(hex)) found.set(`color-${found.size + 1}`, hex);
}
console.log(`room ${room}: ${memories.length} memories, ${found.size} colours`);
for (const [name, hex] of found) console.log(`  ${name}: ${hex}`);
if (!found.size) {
  console.error('no hex colour in the room, nothing to import');
  process.exit(1);
}

const { MELDFRAME_TOKEN: token, MELDFRAME_SERVER: server = 'http://127.0.0.1:8123', MELDFRAME_FILE: file = 'file_dev' } = process.env;
if (!token) {
  console.log('MELDFRAME_TOKEN is not set: stopping after the Uteke leg');
  process.exit(0);
}
const cssText = `:root {\n${[...found].map(([n, h]) => `  --${n}: ${h};`).join('\n')}\n}\n`;
const call = await fetch(`${server}/mcp`, {
  method: 'POST',
  headers: { 'content-type': 'application/json', accept: 'application/json, text/event-stream', authorization: `Bearer ${token}` },
  body: JSON.stringify({ jsonrpc: '2.0', id: 1, method: 'tools/call', params: { name: 'import_tokens', arguments: { file, css: cssText, dryRun: false, idempotencyKey: `uteke-${room}` } } }),
});
const out = JSON.parse((await call.json()).result?.content?.[0]?.text ?? '{}');
assert.ok(!out.error, `import_tokens failed: ${JSON.stringify(out).slice(0, 400)}`);
console.log(`import_tokens into ${file}: ${out.report.variables} variables from ${out.report.colors} colours (${out.replayed ? 'replayed' : 'written'})`);
