// Browser end-to-end test: the real server (in-memory files), the Vite dev server with the dev-token proxy,
// and Chromium through Playwright. Run it with `npm run e2e` from apps/web.
//
// Needs `playwright` resolvable from Node (NODE_PATH, or `npm i --no-save playwright`) and a Chromium
// (`PLAYWRIGHT_CHROMIUM_PATH` to use a specific one). The server binary is built with cargo first.
//
// The test sits a TCP forwarder between the dev server's proxy and the real server, so it can drop every
// connection and refuse new ones, then let them back in: the "dropped connection" of the criterion.
const { spawn, execFileSync } = require('node:child_process');
const net = require('node:net');
const path = require('node:path');
const assert = require('node:assert/strict');

const root = path.resolve(__dirname, '../../..');
const HUMAN = { token: 'e2e-human-token-0001', actor: 'usr_human' };
const AGENT = { token: 'e2e-agent-token-0002', actor: 'agent:alpha' };
const WS = 'ws_e2e';
const FILE = 'file_e2e';

const freePort = () =>
  new Promise((ok) => {
    const s = net.createServer().listen(0, '127.0.0.1', () => {
      const { port } = s.address();
      s.close(() => ok(port));
    });
  });

function waitFor(child, pattern, what) {
  return new Promise((ok, fail) => {
    let out = '';
    const on = (d) => {
      out += d;
      if (pattern.test(out)) ok(out);
    };
    child.stdout.on('data', on);
    child.stderr.on('data', on);
    child.on('exit', (c) => fail(new Error(`${what} exited (${c}): ${out}`)));
    setTimeout(() => fail(new Error(`${what} did not start: ${out}`)), 60_000);
  });
}

/** Forwards 127.0.0.1:port to the server; `cut()` drops every connection and refuses new ones. */
function forwarder(port, target) {
  const sockets = new Set();
  let server;
  const open = () =>
    new Promise((ok) => {
      server = net.createServer((client) => {
        const up = net.connect(target, '127.0.0.1');
        for (const s of [client, up]) {
          sockets.add(s);
          s.on('close', () => sockets.delete(s));
          s.on('error', () => s.destroy());
        }
        client.pipe(up);
        up.pipe(client);
      });
      server.listen(port, '127.0.0.1', ok);
    });
  return {
    open,
    cut: () => new Promise((ok) => {
      for (const s of sockets) s.destroy();
      server.close(ok);
    }),
  };
}

async function api(port, who, body, method = 'POST', file = FILE, tool = 'apply_commands') {
  const url = method === 'GET' ? `http://127.0.0.1:${port}/v1/files/${file}` : `http://127.0.0.1:${port}/v1/files/${file}/tools/${tool}`;
  const res = await fetch(url, {
    method,
    headers: { authorization: `Bearer ${who.token}`, 'content-type': 'application/json' },
    body: method === 'GET' ? undefined : JSON.stringify(body),
  });
  return { status: res.status, body: await res.json() };
}

(async () => {
  const children = [];
  const stop = () => children.forEach((c) => c.kill());
  try {
    const target = process.env.CARGO_TARGET_DIR || path.join(root, 'target');
    execFileSync('cargo', ['build', '-p', 'meldframe-server'], { cwd: root, stdio: 'inherit' });
    const [serverPort, proxyPort, webPort] = [await freePort(), await freePort(), await freePort()];
    const server = spawn(path.join(target, 'debug/meldframe-server'), [], {
      env: {
        ...process.env,
        MELDFRAME_LISTEN: `127.0.0.1:${serverPort}`,
        MELDFRAME_DEV_TOKENS: `${HUMAN.token}:${WS}:${HUMAN.actor},${AGENT.token}:${WS}:${AGENT.actor}`,
      },
    });
    children.push(server);
    await waitFor(server, /listening on/, 'server');
    const fwd = forwarder(proxyPort, serverPort);
    await fwd.open();
    const vite = spawn(path.resolve(__dirname, '../node_modules/.bin/vite'), ['--port', String(webPort), '--strictPort', '--host', '127.0.0.1'], {
      cwd: path.resolve(__dirname, '..'),
      env: {
        ...process.env,
        MELDFRAME_SERVER: `http://127.0.0.1:${proxyPort}`,
        MELDFRAME_WEB_TOKEN: HUMAN.token,
        MELDFRAME_WEB_ACTOR: HUMAN.actor,
        MELDFRAME_WEB_FILE: FILE,
      },
    });
    children.push(vite);
    await waitFor(vite, /Local:/, 'vite');

    const { chromium } = require('playwright');
    const browser = await chromium.launch({ executablePath: process.env.PLAYWRIGHT_CHROMIUM_PATH || undefined });
    const page = await browser.newPage({ viewport: { width: 1100, height: 700 } });
    page.on('pageerror', (e) => console.log('pageerror:', e.message));
    await page.goto(`http://127.0.0.1:${webPort}/`);
    const t = (id) => page.locator(`[data-testid=${id}]`);
    const layers = () => page.locator('[data-testid^=layer-]').count();
    const text = (id) => t(id).innerText();
    await t('canvas').locator('canvas').waitFor({ timeout: 20_000 });
    await page.waitForFunction(() => document.querySelector('[data-testid=connection]')?.textContent === 'live', null, { timeout: 15_000 });
    const box = await t('canvas').boundingBox();
    const at = (x, y) => [box.x + x, box.y + y];

    // 1. Create a frame, a rectangle inside it, and a text node.
    await t('tool-frame').click();
    await page.mouse.click(...at(50, 50));
    await t('tool-rectangle').click();
    await page.mouse.click(...at(70, 70));
    await t('tool-text').click();
    await page.mouse.click(...at(400, 300));
    await page.waitForFunction(() => document.querySelectorAll('[data-testid^=layer-]').length === 3);
    await t('prop-text').fill('hello');
    await t('prop-text').dispatchEvent('change');

    // 2. Move the rectangle by dragging it, then undo the move.
    await page.locator('[data-testid^=layer-] .name').filter({ hasText: 'rect' }).click();
    const x0 = Number(await t('prop-x').inputValue());
    await page.mouse.move(...at(120, 120));
    await page.mouse.down();
    await page.mouse.move(...at(220, 170), { steps: 6 });
    await page.mouse.up();
    const x1 = Number(await t('prop-x').inputValue());
    assert.equal(x1, x0 + 100, 'the drag moved the rectangle by 100');
    await t('undo').click();
    assert.equal(Number(await t('prop-x').inputValue()), x0, 'undo put it back');
    await page.waitForFunction(() => document.querySelector('[data-testid=pending]')?.textContent === '0 pending', null, { timeout: 15_000 });
    const mine = await api(serverPort, HUMAN, null, 'GET');
    assert.equal(mine.body.nodes.length, 3, 'the server holds the three nodes');
    assert.ok(mine.body.nodes.some((n) => n.text === 'hello'), 'the text edit reached the server');

    // 3. An agent's change, over REST as another actor, appears without a reload and is labelled.
    const addAgent = (id, key, x) => ({ ops: [{ key, command: { type: 'addNode', node: { id, name: id, type: 'rect', parentId: null, x, y: 20, w: 60, h: 40 } } }] });
    const sent = await api(serverPort, AGENT, addAgent('agent_box', 'agent-k1', 600));
    assert.equal(sent.status, 200, JSON.stringify(sent.body));
    await t('layer-agent_box').waitFor({ timeout: 10_000 });
    assert.equal(await text('actor-agent_box'), AGENT.actor);
    assert.equal(await layers(), 4);

    // 4. Drop the connection. The agent keeps working; the person edits offline.
    await fwd.cut();
    await page.waitForFunction(() => document.querySelector('[data-testid=connection]')?.textContent !== 'live', null, { timeout: 15_000 });
    const sent2 = await api(serverPort, AGENT, addAgent('agent_late', 'agent-k2', 700));
    assert.equal(sent2.status, 200);
    await t('tool-rectangle').click();
    await page.mouse.click(...at(300, 400));
    await page.waitForFunction(() => document.querySelectorAll('[data-testid^=layer-]').length === 5);
    assert.notEqual(await text('pending'), '0 pending', 'the offline edit waits');

    // 5. The connection comes back: it catches up with the missed change and sends the waiting edit.
    await fwd.open();
    await t('layer-agent_late').waitFor({ timeout: 60_000 });
    await page.waitForFunction(() => document.querySelector('[data-testid=connection]')?.textContent === 'live', null, { timeout: 60_000 });
    await page.waitForFunction(() => document.querySelector('[data-testid=pending]')?.textContent === '0 pending', null, { timeout: 30_000 });
    assert.equal(await layers(), 6, 'frame, rectangle, text, two agent nodes and the offline rectangle');
    const after = await api(serverPort, HUMAN, null, 'GET');
    assert.equal(after.body.nodes.length, 6, 'the server holds all six');
    const reload = await browser.newPage({ viewport: { width: 1100, height: 700 } });
    await reload.goto(`http://127.0.0.1:${webPort}/`);
    await reload.locator('[data-testid^=layer-]').first().waitFor({ timeout: 20_000 });
    assert.equal(await reload.locator('[data-testid^=layer-]').count(), 6, 'a fresh page loads the snapshot');

    await browser.close();
    console.log('e2e: ok');
    stop();
    process.exit(0);
  } catch (e) {
    console.error('e2e: FAILED', e);
    stop();
    process.exit(1);
  }
})();
