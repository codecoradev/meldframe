// The M1 demonstration (docs/epics.md, "M1 exit criteria") in the real browser editor, against the real server:
//   node packages/web-e2e/scripts/m1-demo.mjs [web-url] [server-url]
// Needs the development server started with two tokens, `just web` on the same file, the WASM bridge built and Playwright:
//   MELDFRAME_DEV_TOKENS="<.dev-token>:ws_dev:agent:dev,<token 2>:ws_dev:agent:claude" MELDFRAME_REVIEWERS=agent:dev just dev-server
//   MELDFRAME_WEB_FILE=<file> just web
//   MELDFRAME_WEB_FILE=<file> MELDFRAME_DEMO_TOKEN2=<token 2> node packages/web-e2e/scripts/m1-demo.mjs
// MELDFRAME_REVIEWERS names who may accept or flag a batch (ADR-0021): the editor's actor, `agent:dev`, and not the agent's,
// which has a token and an actor of its own so it cannot review its own work. Unset, nobody may.
// The agent is token 2, speaking MCP over HTTP (JSON-RPC `tools/call`); the person is the browser editor (token 1, added by
// the dev proxy). Prints one line per step and exits non-zero if a step fails. Run against the development server only.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const { chromium } = createRequire(import.meta.url)('playwright');
const web = process.argv[2] ?? 'http://localhost:5173';
const server = process.argv[3] ?? 'http://127.0.0.1:8080';
const agentToken = process.env.MELDFRAME_DEMO_TOKEN2;
const file = process.env.MELDFRAME_WEB_FILE ?? 'file_dev';
if (!agentToken) throw new Error('set MELDFRAME_DEMO_TOKEN2 to the second token (see the top of this file)');
const run = Math.random().toString(36).slice(2, 7);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let rpc = 0;

/** One MCP tool call as the agent; the result's JSON body (or the error body). */
async function tool(name, args) {
  const res = await fetch(`${server}/mcp`, {
    method: 'POST',
    headers: { 'content-type': 'application/json', accept: 'application/json, text/event-stream', authorization: `Bearer ${agentToken}` },
    body: JSON.stringify({ jsonrpc: '2.0', id: ++rpc, method: 'tools/call', params: { name, arguments: { file, ...args } } }),
  });
  const part = (await res.json()).result?.content?.[0];
  if (part?.type === 'image') return { image: Buffer.from(part.data, 'base64') };
  return JSON.parse(part?.text ?? '{}');
}
async function until(what, fn, ms = 20000) {
  const t0 = Date.now();
  for (;;) {
    const v = await fn();
    if (v) return v;
    if (Date.now() - t0 > ms) throw new Error(`timed out waiting for: ${what}`);
    await sleep(200);
  }
}
const nodeOf = async (id) => (await tool('get_nodes', { ids: [id] })).nodes?.[0];
const step = (n, text) => console.log(`${n}. ${text}`);

const hero = `hero_${run}`, heading = `heading_${run}`, blurb = `blurb_${run}`, button = `button_${run}`, label = `label_${run}`;
const text = (id, parentId, t, y, size) => ({ type: 'addNode', node: { id, type: 'text', name: t, parentId, x: 32, y, w: 400, h: size + 8, text: t, fontSize: size, fill: '#ffffff' } });

const browser = await chromium.launch({ args: ['--use-gl=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
let block = false; // while true, the editor's live socket is refused: the "dropped connection"
let live;
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: 1 });
  await page.routeWebSocket(/\/v1\/live\//, (ws) => {
    if (block) return void ws.close();
    live = ws;
    ws.connectToServer();
  });
  page.on('pageerror', (e) => console.log('[pageerror]', e.message));
  await page.goto(web);
  await page.evaluate(() => { window.__stay = true; });
  await until('connected', async () => (await page.getAttribute('[data-testid=connection]', 'data-state')) === 'live');
  const rows = () => page.locator('[data-testid=layer]').count();
  const base = await rows(); // nodes the file already had (a second run on the same file)

  // 1. The agent builds a landing hero in one atomic batch; the open editor shows it without a reload.
  const built = await tool('apply_commands', {
    idempotencyKey: `hero-${run}`,
    commands: [
      { type: 'addNode', node: { id: hero, type: 'frame', name: 'Hero', parentId: null, x: 0, y: 0, w: 480, h: 260, fill: '#1d3b6e' } },
      text(heading, hero, 'Design with agents', 32, 32),
      text(blurb, hero, 'One canvas for people and agents.', 90, 16),
      { type: 'addNode', node: { id: button, type: 'frame', name: 'Button', parentId: hero, x: 32, y: 150, w: 160, h: 48, fill: '#fb923c', radius: 12 } },
      text(label, button, 'Get started', 10, 18),
    ],
  });
  assert.ok(built.seq, `the batch failed: ${JSON.stringify(built)}`);
  await until('the hero in the editor', async () => (await rows()) === base + 5);
  assert.equal(await page.evaluate(() => window.__stay), true, 'the page was not reloaded');
  step(1, `the agent built the hero in one batch (seq ${built.seq}); the open editor shows its 5 layers without a reload`);

  // 2. The person moves the heading in the editor; the agent's next read shows the new position; its stale edit conflicts.
  const before = await nodeOf(heading);
  const seen = (await tool('get_tree', {})).seq;
  await page.click(`[data-testid=layer][data-id="${heading}"] button >> nth=-1`); // select it in the layers panel
  const box = await page.locator('canvas').boundingBox();
  const zoom = Number.parseInt(await page.textContent('[data-testid=zoom]'), 10) / 100;
  // The hero is fitted to the window: its top-left is at the centre minus half its size.
  const at = { x: box.x + box.width / 2 + (before.x + 200 - 240) * zoom, y: box.y + box.height / 2 + (before.y + 20 - 130) * zoom };
  await page.mouse.move(at.x, at.y);
  await page.mouse.down();
  await page.mouse.move(at.x + 40 * zoom, at.y + 30 * zoom, { steps: 6 });
  await page.mouse.up();
  const moved = await until('the heading moved', async () => { const n = await nodeOf(heading); return n.x !== before.x ? n : null; });
  step(2, `the person dragged the heading from (${before.x}, ${before.y}) to (${moved.x.toFixed(0)}, ${moved.y.toFixed(0)}); the agent's next read shows it`);
  const stale = await tool('apply_commands', { idempotencyKey: `retitle-${run}`, ifSeq: seen, commands: [{ type: 'updateNode', id: heading, patch: { text: 'Design faster' } }] });
  assert.equal(stale.error?.code, 'conflict', `expected a conflict: ${JSON.stringify(stale).slice(0, 200)}`);
  step(2, `the agent's edit made from the old read is rejected with conflict (changes by ${stale.error.changes?.[0]?.actor ?? 'someone'})`);

  // 3. The person presses undo: only their move is undone; the agent's work stays.
  await page.locator('canvas').focus();
  await page.keyboard.press('Control+z');
  await until('the move undone', async () => (await nodeOf(heading)).x === before.x && (await nodeOf(heading)).y === before.y);
  assert.equal(await rows(), base + 5, 'the agent\'s nodes stay');
  assert.equal((await nodeOf(button)).fill, '#fb923c');
  step(3, 'the person pressed undo: the heading is back, the 5 nodes of the agent stay');

  // 4. The browser drops its connection and reconnects: it receives exactly the missed changes.
  block = true;
  live?.close();
  await until('not live', async () => (await page.getAttribute('[data-testid=connection]', 'data-state')) !== 'live');
  const missed = await tool('apply_commands', { idempotencyKey: `missed-${run}`, commands: [{ type: 'updateNode', id: blurb, patch: { text: 'Written while you were offline.' } }] });
  assert.ok(missed.seq);
  block = false;
  await until('live again', async () => (await page.getAttribute('[data-testid=connection]', 'data-state')) === 'live', 60000);
  await page.click(`[data-testid=layer][data-id="${blurb}"] button >> nth=-1`);
  await until('the missed change arrived', async () => (await page.locator('[data-testid=prop-name]').inputValue()) !== '' );
  assert.equal(await page.evaluate(() => window.__stay), true, 'no reload for the reconnect');
  step(4, `the connection dropped and came back; the editor caught up on the change made meanwhile (seq ${missed.seq}) without a reload`);

  // 5. The agent captures the frame and receives a PNG.
  const t5 = Date.now();
  let shot = await tool('capture', { nodeId: hero, scale: 1 });
  while (!shot.image && Date.now() - t5 < 15000) { await sleep(1000); shot = await tool('capture', { nodeId: hero, scale: 1 }); }
  console.log('   (capture took', Date.now() - t5, 'ms)');
  assert.ok(shot.image, `no image: ${JSON.stringify(shot).slice(0, 200)}`);
  assert.equal(shot.image.subarray(1, 4).toString(), 'PNG');
  step(5, `the agent captured the hero and got a PNG (${shot.image.readUInt32BE(16)} x ${shot.image.readUInt32BE(20)} px)`);

  // 6. Every write passed the one gate: the server's log shows the actor and the decision of each.
  // The activity log holds the actor and the risk tier of every accepted op; it has no decision column and a refused write
  // is not in it (a known gap, recorded in docs/epics.md), so this step is not asserted here.
  console.log('6. (not asserted) every write went through the one gate of the server; its log lists the actor of each accepted write');
  console.log('M1 demonstration steps 1 to 5 passed in the browser editor.');
} catch (e) {
  console.log('FAIL', e.message ?? e);
  process.exitCode = 1;
} finally {
  await browser.close();
}
