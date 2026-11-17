// Browser check of the editor against a real server: node scripts/editor-e2e.mjs [web-url] [server-url]
// Needs `just dev` (server on 8080) and `just web` (Vite on 5173) running, the WASM bridge built (`just wasm-build`),
// and Playwright from the global module path. It restarts the server (bash scripts/dev.sh) for the reconnect check, so
// run it against the development server only. Not part of `npm test`: it needs a browser, a server and Postgres.
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { closeSync, openSync, readFileSync } from 'node:fs';
import { createRequire } from 'node:module';

const { chromium } = createRequire(import.meta.url)('playwright');
const web = process.argv[2] ?? 'http://localhost:5173';
const server = process.argv[3] ?? 'http://127.0.0.1:8080';
const root = new URL('../../../', import.meta.url).pathname;
const token = readFileSync(`${root}.dev-token`, 'utf8').trim();
const file = process.env.MELDFRAME_WEB_FILE ?? 'file_dev';
const run = Math.random().toString(36).slice(2, 8);
const pageErrors = [];
let browser;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let n = 0;

async function post(command) {
  const res = await fetch(`${server}/v1/files/${file}/tools/apply_commands`, {
    method: 'POST',
    headers: { 'content-type': 'application/json', authorization: `Bearer ${token}` },
    body: JSON.stringify({ ops: [{ key: `e2e-${run}-${++n}`, command, deps: [] }] }),
  });
  assert.equal(res.status, 200, `op refused: ${res.status} ${await res.text()}`);
}

const rgb = (hex) => [1, 3, 5].map((i) => Number.parseInt(hex.slice(i, i + 2), 16));
/** How many pixels of the screenshot are (nearly) this colour. */
async function count(page, hex) {
  const b64 = (await page.screenshot()).toString('base64');
  return page.evaluate(async ({ b64, want }) => {
    const img = new Image();
    img.src = `data:image/png;base64,${b64}`;
    await img.decode();
    const c = document.createElement('canvas');
    c.width = img.width; c.height = img.height;
    const g = c.getContext('2d');
    g.drawImage(img, 0, 0);
    const d = g.getImageData(0, 0, c.width, c.height).data;
    let k = 0;
    for (let i = 0; i < d.length; i += 4) if (Math.abs(d[i] - want[0]) < 6 && Math.abs(d[i + 1] - want[1]) < 6 && Math.abs(d[i + 2] - want[2]) < 6) k++;
    return k;
  }, { b64, want: rgb(hex) });
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
const state = (page) => page.getAttribute('[data-testid=connection]', 'data-state');

const results = [];
async function check(name, fn) {
  try { await fn(); results.push(true); console.log('ok  ', name); }
  catch (e) { results.push(false); console.log('FAIL', name, '\n    ', String(e.message ?? e).split('\n')[0]); }
}

const A = '#12ab34', B = '#ab1234', C = '#3412ab';
const id = `e2e_${run}`;
(async () => {
  await post({ type: 'addNode', node: { id, name: `Test ${run}`, type: 'frame', parentId: null, x: 0, y: 0, w: 300, h: 200, fill: A } });
  browser = await chromium.launch({ args: ['--use-gl=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: 1 });
  page.on('pageerror', (e) => { pageErrors.push(e.message); console.log('[pageerror]', e.message); });
  await page.goto(web);
  await page.evaluate(() => { window.__stay = true; });

  await check('the page connects and shows the file that was saved before it opened', async () => {
    await until('connected', async () => (await state(page)) === 'live');
    await until('the saved frame is drawn', async () => (await count(page, A)) > 300);
  });
  await check('a change of an agent appears without a reload', async () => {
    await post({ type: 'updateNode', id, patch: { fill: B } });
    await until('the new fill', async () => (await count(page, B)) > 300);
    assert.ok((await count(page, A)) < 50, 'the old fill is gone');
    assert.equal(await page.evaluate(() => window.__stay), true, 'the page was not reloaded');
  });
  await check('a click on the frame selects it (the canvas draws the selection)', async () => {
    const before = await page.screenshot();
    // The frame is the centre of the screen after the view fitted the file: click there.
    const at = await page.evaluate(async () => {
      const c = document.querySelector('canvas');
      const r = c.getBoundingClientRect();
      return { x: r.x + r.width / 2, y: r.y + r.height / 2 };
    });
    await page.mouse.click(at.x, at.y);
    await sleep(400);
    const after = await page.screenshot();
    assert.notDeepEqual(after, before, 'the selection changed what is drawn');
  });
  // The reconnect check stops the development server: only when the checks before it passed (otherwise leave it).
  if (results.every(Boolean)) await check('the server goes away: the page says so; it comes back: the page reconnects and shows what was done meanwhile', async () => {
    // Stop only what listens on the server's port (not every process that has the name).
    const port = new URL(server).port || '80';
    const pids = spawnSync('lsof', ['-ti', `tcp:${port}`, '-sTCP:LISTEN']).stdout.toString().split(/\s+/).filter(Boolean);
    assert.ok(pids.length, `nothing listens on port ${port}`);
    for (const pid of pids) process.kill(Number(pid));
    await until('not live', async () => (await state(page)) !== 'live', 20000);
    // The restart keeps this process's environment (CARGO_TARGET_DIR included) and logs to a file.
    const log = openSync('/tmp/editor-e2e-dev.log', 'w');
    const dev = spawn('bash', ['scripts/dev.sh'], { cwd: root, detached: true, stdio: ['ignore', log, log] });
    dev.unref();
    closeSync(log);
    await until('the server answers', async () => { try { return (await fetch(`${server}/v1/health`)).ok; } catch { return false; } }, 120000);
    await post({ type: 'updateNode', id, patch: { fill: C } });
    await until('live again', async () => (await state(page)) === 'live', 90000);
    await until('the change made while it reconnected', async () => (await count(page, C)) > 300, 30000);
    assert.equal(await page.evaluate(() => window.__stay), true, 'the page was not reloaded');
  });
  await check('no uncaught error in the page', async () => assert.deepEqual(pageErrors, []));
  await browser.close();
  const failed = results.filter((r) => !r).length;
  console.log(failed ? `${failed} of ${results.length} checks failed` : `all ${results.length} checks passed`);
  process.exit(failed ? 1 : 0);
})().catch(async (e) => {
  console.error(`editor-e2e: ${e.message}`);
  await browser?.close().catch(() => {});
  process.exit(1);
});
