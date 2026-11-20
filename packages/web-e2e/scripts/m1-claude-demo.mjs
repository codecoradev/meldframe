// The M1 demonstration with a real MCP client: Claude Code (the `claude` CLI, `-p` mode) is the agent, connected to the
// server's MCP endpoint, and the person is the browser editor:
//   node packages/web-e2e/scripts/m1-claude-demo.mjs [web-url] [server-url]
// Start first (see m1-demo.mjs): the server with two tokens and MELDFRAME_REVIEWERS naming the editor's actor only (the agent
// must not be listed: the activity panel's review controls are for the person), `just web` on the same file, then run with
//   MELDFRAME_WEB_FILE=<file> MELDFRAME_DEMO_TOKEN2=<token 2> node packages/web-e2e/scripts/m1-claude-demo.mjs
// Use a fresh server and a fresh file for each run: a successful capture binds the agent's session to its first file, and a
// second run on another file would be refused with room_mismatch. `claude` must be installed and logged in. Claude Code gets only the Meldframe MCP servers (`--strict-mcp-config`) and its
// Meldframe tools are allowed without a prompt; it runs in an empty temporary directory. The script checks what Claude Code
// did from its own tool calls (stream-json) and from the file and the editor, and exits non-zero if a step fails.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createRequire } from 'node:module';

const { chromium } = createRequire(import.meta.url)('playwright');
const web = process.argv[2] ?? 'http://localhost:5173';
const server = process.argv[3] ?? 'http://127.0.0.1:8080';
const token2 = process.env.MELDFRAME_DEMO_TOKEN2;
const file = process.env.MELDFRAME_WEB_FILE ?? 'file_dev';
if (!token2) throw new Error('set MELDFRAME_DEMO_TOKEN2 to the second token (see the top of this file)');
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const dir = mkdtempSync(join(tmpdir(), 'm1-claude-'));
const mcpConfig = join(dir, 'mcp.json');
writeFileSync(mcpConfig, JSON.stringify({ mcpServers: { meldframe: { type: 'http', url: `${server}/mcp`, headers: { Authorization: `Bearer ${token2}` } } } }), { mode: 0o600 });

/** One `claude -p` turn (resumed when `session` is given): the tool calls it made and its final text. */
function claude(prompt, session) {
  const args = ['-p', prompt, '--mcp-config', mcpConfig, '--strict-mcp-config', '--allowedTools', 'mcp__meldframe__*', '--output-format', 'stream-json', '--verbose', '--max-turns', '12'];
  if (session) args.push('--resume', session);
  return new Promise((resolve, reject) => {
    const p = spawn('claude', args, { cwd: dir, stdio: ['ignore', 'pipe', 'pipe'] });
    let out = '', err = '';
    p.stdout.on('data', (d) => (out += d));
    p.stderr.on('data', (d) => (err += d));
    const timer = setTimeout(() => p.kill('SIGKILL'), 240_000); // a stuck turn must not hang the demonstration
    p.on('error', reject);
    p.on('close', (code) => {
      clearTimeout(timer);
      if (code !== 0) return reject(new Error(`claude exited with ${code}: ${err.slice(0, 300)}`));
      const calls = [], results = new Map();
      let sessionId = session, text = '';
      for (const line of out.split('\n')) {
        let m;
        try { m = JSON.parse(line); } catch { continue; }
        sessionId = m.session_id ?? sessionId;
        for (const c of m.message?.content ?? []) {
          if (c.type === 'tool_use') calls.push({ id: c.id, name: c.name.replace(/^mcp__meldframe__/, ''), input: c.input });
          if (c.type === 'tool_result') results.set(c.tool_use_id, c.content);
        }
        if (m.type === 'result') text = m.result ?? '';
      }
      resolve({ sessionId, text, err, calls: calls.map((c) => ({ ...c, result: results.get(c.id) })) });
    });
  });
}
const resultText = (r) => (Array.isArray(r) ? r.map((x) => x.text ?? '').join('') : String(r ?? ''));
const rest = (path) => fetch(`${server}${path}`, { headers: { authorization: `Bearer ${token2}` } }).then((r) => r.json());
const nodeOf = async (id) => (await rest(`/v1/files/${file}`)).nodes.find((n) => n.id === id);
async function until(what, fn, ms = 30000) {
  const t0 = Date.now();
  for (;;) {
    const v = await fn();
    if (v) return v;
    if (Date.now() - t0 > ms) throw new Error(`timed out waiting for: ${what}`);
    await sleep(250);
  }
}
const step = (n, t) => console.log(`${n}. ${t}`);

const browser = await chromium.launch({ args: ['--use-gl=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: 1 });
  page.on('pageerror', (e) => console.log('[pageerror]', e.message));
  await page.goto(web);
  await page.evaluate(() => { window.__stay = true; });
  await until('connected', async () => (await page.getAttribute('[data-testid=connection]', 'data-state')) === 'live');
  const rows = () => page.locator('[data-testid=layer]').count();
  const base = await rows();

  // 1. Claude Code builds the hero in one atomic batch.
  const a = await claude(
    `You are connected to Meldframe, a design canvas, through MCP. In the file "${file}", build a landing hero in ONE atomic apply_commands call: a frame named "Hero" (480 x 260, dark blue fill) with the id "hero", a heading text "Design with agents" (id "heading", font size 32, white), a text "One canvas for people and agents." (id "blurb", font size 16) and a button: a frame (id "button", 160 x 48, orange fill, radius 12) with a text "Get started" (id "button_label") inside it. Place the children inside the hero. Then call get_tree and tell me the seq of the file. Use the tool descriptions to find out how to call the tools.`,
  );
  const batch = a.calls.filter((c) => c.name === 'apply_commands');
  assert.equal(batch.length, 1, `expected one apply_commands call, got ${batch.length}: ${a.text.slice(0, 300)}`);
  assert.ok((batch[0].input.commands?.length ?? 0) >= 5, 'the batch has the five nodes');
  await until('the hero in the editor', async () => (await rows()) >= base + 5);
  assert.equal(await page.evaluate(() => window.__stay), true, 'the page was not reloaded');
  // The nodes are the ones that were asked for, in the places that were asked for.
  const want = { hero: null, heading: 'hero', blurb: 'hero', button: 'hero', button_label: 'button' };
  for (const [id, parent] of Object.entries(want)) {
    const n = await nodeOf(id);
    assert.ok(n, `the node ${id} exists`);
    assert.equal(n.parentId, parent, `${id} is inside ${parent}`);
  }
  const fills = Object.fromEntries(await Promise.all(['hero', 'button'].map(async (id) => [id, (await nodeOf(id)).fill])));
  const tree = a.calls.find((c) => c.name === 'get_tree');
  assert.ok(tree, 'Claude Code read the tree');
  const seq0 = JSON.parse(resultText(tree.result)).seq;
  step(1, `Claude Code built the hero in one apply_commands call (${batch[0].input.commands.length} commands) and read the tree at seq ${seq0}; the open editor shows it without a reload`);

  // 2. The person moves the heading; Claude Code's next read shows it; its edit made from the old read conflicts.
  const before = await nodeOf('heading');
  assert.ok(before, 'the heading exists');
  await page.click('[data-testid=layer][data-id="heading"] button >> nth=-1');
  const box = await page.locator('canvas').boundingBox();
  const zoom = Number.parseInt(await page.textContent('[data-testid=zoom]'), 10) / 100;
  const hero = await nodeOf('hero');
  const at = { x: box.x + box.width / 2 + (before.x + before.w / 2 - hero.w / 2) * zoom, y: box.y + box.height / 2 + (before.y + before.h / 2 - hero.h / 2) * zoom };
  await page.mouse.move(at.x, at.y);
  await page.mouse.down();
  await page.mouse.move(at.x + 40 * zoom, at.y + 30 * zoom, { steps: 6 });
  await page.mouse.up();
  const moved = await until('the heading moved', async () => { const n = await nodeOf('heading'); return n.x !== before.x ? n : null; });
  step(2, `the person dragged the heading from (${before.x}, ${before.y}) to (${moved.x.toFixed(0)}, ${moved.y.toFixed(0)})`);
  const b = await claude(
    `Someone may have changed the file since your last read. First, change the text of the heading to "Design faster" with apply_commands, passing ifSeq = ${seq0} (the seq you read before). Tell me exactly what the tool answered. Then read the heading with get_nodes and tell me its x and y.`,
    a.sessionId,
  );
  const stale = b.calls.find((c) => c.name === 'apply_commands');
  assert.ok(stale, 'Claude Code tried the edit');
  assert.match(resultText(stale.result), /conflict/, `expected a conflict: ${resultText(stale.result).slice(0, 300)}`);
  const read = b.calls.find((c) => c.name === 'get_nodes');
  assert.ok(read, 'Claude Code read the heading again');
  const seen = JSON.parse(resultText(read.result)).nodes[0];
  assert.equal(Math.round(seen.x), Math.round(moved.x), 'its next read shows the new position');
  assert.notEqual((await nodeOf('heading')).text, 'Design faster', 'the stale edit changed nothing');
  step(2, `Claude Code's next read shows the new position (${seen.x}, ${seen.y}); its edit with the old seq was rejected with conflict`);

  // 3. The person presses undo: only their move is undone; Claude Code's work stays.
  await page.locator('canvas').focus();
  await page.keyboard.press('Control+z');
  await until('the move undone', async () => { const n = await nodeOf('heading'); return n.x === before.x && n.y === before.y; });
  assert.ok((await rows()) >= base + 5, "Claude Code's nodes stay");
  for (const [id, parent] of Object.entries(want)) assert.equal((await nodeOf(id))?.parentId, parent, `${id} is untouched by the undo`);
  for (const [id, fill] of Object.entries(fills)) assert.equal((await nodeOf(id)).fill, fill, `${id} keeps its fill`);
  step(3, "the person pressed undo: the heading is back where Claude Code put it, and its other nodes stay");

  // 5. Claude Code captures the frame and receives a PNG from the open editor.
  const c = await claude(`In the file "${file}", capture the Hero frame as an image with the capture tool (scale 1) and tell me the pixel size of the image you got. If a call fails, read the error and call again.`, a.sessionId);
  // Claude Code may need a second try when its first call misses an argument; what counts is that a capture call got an image.
  const caps = c.calls.filter((x) => x.name === 'capture');
  assert.ok(caps.length, 'Claude Code called capture');
  const withImage = caps.map((x) => ({ x, img: Array.isArray(x.result) ? x.result.find((p) => p.type === 'image') : null })).find((y) => y.img);
  assert.ok(withImage, `no image in the results of capture: ${JSON.stringify(caps.map((x) => [x.input, JSON.stringify(x.result).slice(0, 160)]))}`);
  const img = withImage.img;
  const png = Buffer.from(img.source?.data ?? img.data ?? '', 'base64');
  assert.equal(png.subarray(1, 4).toString(), 'PNG');
  step(5, `Claude Code captured the hero through the open editor and got a PNG (${png.readUInt32BE(16)} x ${png.readUInt32BE(20)} px)`);
  console.log('Steps 1 to 3 and 5 passed with Claude Code as the MCP client (step 4, the dropped connection, has no agent in it: m1-demo.mjs runs it).');
} catch (e) {
  console.log('FAIL', e.message ?? e);
  process.exitCode = 1;
} finally {
  await browser.close();
  rmSync(dir, { recursive: true, force: true }); // the token is in the config file of this directory
}
