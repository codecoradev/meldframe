// Browser check of the checkpoints (E5.3, ADR-0023) against a real server with Postgres:
//   node packages/web-panels/scripts/checkpoints-e2e.mjs [web-url] [server-url]
// The person is the browser editor (the token in .dev-token at the root, actor person:ada); the agent is a second token that
// speaks MCP over HTTP (`apply_commands`). Start, on a file name that is new (the script refuses a file that has ops):
//   MELDFRAME_DEV_TOKENS="<.dev-token>:ws_dev:person:ada,<token 2>:ws_dev:agent:claude" MELDFRAME_REVIEWERS=person:ada \
//     DATABASE_URL=postgres://postgres:postgres@127.0.0.1:5433/meldframe_dev just dev-server
//   MELDFRAME_WEB_FILE=<file> MELDFRAME_WEB_ACTOR=person:ada just web           (the person's pages, port 5173)
//   MELDFRAME_WEB_FILE=<file> MELDFRAME_DEMO_TOKEN2=<token 2> node packages/web-panels/scripts/checkpoints-e2e.mjs
// `just dev-server` sources .env after the environment is set: a .env that sets MELDFRAME_DEV_TOKENS or MELDFRAME_REVIEWERS
// overrides the lines above, so leave them out of it. The web and server URLs must be loopback (both tokens are sent there).
// What it checks: a run start taken while no editor is open, the list read on load, a manual checkpoint (the name checked, the
// field emptied), a reload that keeps the list, a restore after the reload (the file at the checkpoint comes from the server),
// a revert that waits for its row (a held post changes nothing; a failed one refuses it) and two editors that see the same list
// (a row made in one arrives in the other without a reload). Needs the WASM bridge built (`just wasm-build`) and Playwright from the
// global module path. Run it against a development server only: it adds nodes and checkpoints to the file and leaves them.
// MELDFRAME_E2E_SHOTS=<dir> saves screenshots (and one of the page when a check fails); MELDFRAME_E2E_DEBUG=1 prints more.
// With MELDFRAME_UI=react the pages open with /?react=activity (the React copy of the panel, E7.3 slice S8), the first check of the
// page asks that React drew the panel (a silent fallback to Svelte would pass every other check), and a last check blocks the chunks
// of the React copy and asks that the Svelte panel is drawn and still lists the checkpoints (dev server only: the block matches the
// paths of its modules).
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { fibre, openWithChunksBlocked, pageUrl, react, until } from './activity-helpers.mjs';

const { chromium } = createRequire(import.meta.url)('playwright');
const die = (m) => { console.error(`checkpoints-e2e: ${m}`); process.exit(1); };
const web = process.argv[2] ?? 'http://localhost:5173';
const server = process.argv[3] ?? 'http://127.0.0.1:8080';
const root = new URL('../../../', import.meta.url).pathname;
const person = readFileSync(`${root}.dev-token`, 'utf8').trim();
const agent = process.env.MELDFRAME_DEMO_TOKEN2;
const file = process.env.MELDFRAME_WEB_FILE;
const shots = process.env.MELDFRAME_E2E_SHOTS;
if (!agent) die('set MELDFRAME_DEMO_TOKEN2 to the second token (see the top of this file)');
if (!file) die('set MELDFRAME_WEB_FILE to a file name that has no ops yet (see the top of this file)');
for (const u of [web, server]) {
  if (!['127.0.0.1', 'localhost', '[::1]'].includes(new URL(u).hostname)) die(`refusing to send tokens to ${u}: the web and server URLs must be loopback (127.0.0.1, localhost or ::1)`);
}
const run = Math.random().toString(36).slice(2, 7);
const pageErrors = [];
let rpc = 0;
let mainPage;

async function tool(token, name, args) {
  const res = await fetch(`${server}/mcp`, {
    method: 'POST',
    headers: { 'content-type': 'application/json', accept: 'application/json, text/event-stream', authorization: `Bearer ${token}` },
    body: JSON.stringify({ jsonrpc: '2.0', id: ++rpc, method: 'tools/call', params: { name, arguments: { file, ...args } } }),
  });
  return JSON.parse((await res.json()).result?.content?.[0]?.text ?? '{}');
}
async function http(token, method, path, body) {
  const res = await fetch(`${server}${path}`, { method, headers: { authorization: `Bearer ${token}`, ...(body ? { 'content-type': 'application/json' } : {}) }, body: body && JSON.stringify(body) });
  return { status: res.status, body: await res.json().catch(() => null) };
}
const rows = async (token = person) => (await http(token, 'GET', `/v1/files/${file}/checkpoints`)).body;
const nodes = async () => Object.fromEntries((await http(person, 'GET', `/v1/files/${file}`)).body.nodes.map((n) => [n.id, n]));

// The file must be new: a second run on the same file would count from where the first stopped.
const existing = await http(person, 'GET', `/v1/files/${file}`);
if (existing.status !== 404) die(`the file ${file} already has content (status ${existing.status}): start the server and the pages again on a file name that is new`);
if ((await rows()).checkpoints?.length) die(`the file ${file} already has checkpoints: use a file name that is new`);

const id = (s) => `${s}_${run}`;
const [hero, heading] = ['hero', 'heading'].map(id);
const T0 = 'Design with agents', T1 = 'Design faster', T2 = 'Design together';
const build = { idempotencyKey: `cp-${run}-a`, note: 'Builds the hero.', commands: [
  { type: 'addNode', node: { id: hero, type: 'frame', name: 'Hero', parentId: null, x: 0, y: 0, w: 480, h: 260, fill: '#1d3b6e' } },
  { type: 'addNode', node: { id: heading, type: 'text', name: T0, parentId: hero, x: 32, y: 32, w: 400, h: 40, text: T0, fontSize: 32, fill: '#ffffff' } }] };
const retitle = (key, text) => ({ idempotencyKey: `cp-${run}-${key}`, note: `Retitles the hero to ${text}.`, commands: [{ type: 'updateNode', id: heading, patch: { text } }] });

const results = [];
async function check(name, fn) {
  try { await fn(); results.push(true); console.log(`ok   ${name}`); } catch (e) {
    results.push(false);
    if (shots && mainPage) await mainPage.screenshot({ path: `${shots}/cp-fail-${results.length}.png` }).catch(() => {});
    console.log(`FAIL ${name}: ${String(e.message ?? e).split('\n').slice(0, process.env.MELDFRAME_E2E_DEBUG ? 12 : 1).join(' | ')}`);
  }
}

(async () => {
  let made;
  await check('a run start is taken by the server while no editor is open', async () => {
    made = (await tool(agent, 'apply_commands', build)).batch;
    assert.ok(made?.n >= 1, `batch: ${JSON.stringify(made)}`);
    const { checkpoints, canCreate } = await rows();
    assert.equal(canCreate, true, 'the person may make a checkpoint');
    assert.equal((await rows(agent)).canCreate, false, 'the agent may not');
    assert.deepEqual(checkpoints.map((c) => [c.kind, c.about, c.seq, c.by]), [['runStart', 'agent:claude', 0, undefined]]);
  });

  const browser = await chromium.launch({ args: ['--use-gl=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
  const open = async () => {
    const page = await browser.newPage({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1 });
    page.on('pageerror', (e) => { pageErrors.push(e.message); console.log('[pageerror]', e.message); });
    await page.goto(pageUrl(web));
    await until('live', async () => (await page.getAttribute('[data-testid=connection]', 'data-state')) === 'live');
    await page.locator('#right-tab-activity').click();
    await page.locator('[data-testid=activity]').waitFor({ timeout: 10000 });
    return page;
  };
  const shot = (page, name) => (shots ? page.screenshot({ path: `${shots}/cp-${name}.png` }) : null);
  const ids = (p) => p.locator('[data-testid=checkpoint]').evaluateAll((l) => l.map((x) => x.dataset.checkpoint));
  const words = (p) => p.locator('[data-testid=checkpoint]').evaluateAll((l) => l.map((x) => x.textContent));
  const message = (p) => p.locator('[data-testid=activity-message]').innerText();
  const page = await open();
  mainPage = page;
  await page.evaluate(() => { window.__stay = true; });
  const create = async (p, name) => { await p.locator('[data-testid=checkpoint-name]').fill(name); await p.locator('[data-testid=checkpoint-create]').click(); };

  await check('the Activity panel is drawn by the copy that was asked for', async () => {
    assert.equal(await page.locator('[data-testid=activity]').count(), 1);
    assert.equal(await fibre(page, '[data-testid=activity]'), react, react ? 'the Activity panel has no React fibre: the page fell back to Svelte' : 'the Activity panel has a React fibre on the Svelte run');
  });

  await check("the list is read on load: the agent's run start shows in words, and the control is enabled", async () => {
    await until('the run start', async () => (await words(page)).some((t) => /Before claude's run/.test(t)));
    assert.equal(await page.locator('[data-testid=checkpoint-create]').isEnabled(), true);
    assert.equal(await page.locator('[data-testid=checkpoint-name]').getAttribute('maxlength'), '128');
    await shot(page, 'list');
  });

  let after;
  await check('Create checkpoint makes a manual row with the name, shows who made it, and empties the field', async () => {
    await create(page, '  after the hero  ');
    await until('the row', async () => (await words(page)).some((t) => /after the hero/.test(t)));
    const rowsNow = (await rows()).checkpoints;
    after = rowsNow.find((c) => c.kind === 'manual');
    assert.deepEqual([after.label, after.by, after.seq], ['after the hero', 'person:ada', (await http(person, 'GET', `/v1/files/${file}`)).body.seq]);
    assert.match(await page.locator('[data-testid=checkpoint][data-checkpoint="' + after.n + '"]').innerText(), /Made by ada/);
    await until('the field is empty', async () => (await page.locator('[data-testid=checkpoint-name]').inputValue()) === '');
    await until('the live region says so', async () => /Checkpoint made/.test(await message(page)));
  });

  await check('a name with a hidden character is refused in words, nothing is made, and the text stays to be corrected', async () => {
    const before = (await rows()).checkpoints.length;
    await create(page, 'pay\u202enow');
    await until('the refusal', async () => /cannot be used/.test(await message(page)));
    assert.equal((await rows()).checkpoints.length, before);
    assert.equal(await page.locator('[data-testid=checkpoint-name]').inputValue(), 'pay\u202enow');
    await page.locator('[data-testid=checkpoint-name]').fill('x'.repeat(200));
    assert.equal((await page.locator('[data-testid=checkpoint-name]').inputValue()).length, 128, 'the field takes 128 characters');
    await page.locator('[data-testid=checkpoint-name]').fill('');
  });

  let shownBefore;
  await check('a reload keeps the list, in the same order', async () => {
    shownBefore = await ids(page);
    await page.reload();
    await until('live', async () => (await page.getAttribute('[data-testid=connection]', 'data-state')) === 'live');
    await page.locator('#right-tab-activity').click();
    await until('the rows', async () => (await ids(page)).length === shownBefore.length);
    assert.deepEqual(await ids(page), shownBefore);
    assert.equal(shownBefore[0], String((await rows()).checkpoints[0].n), 'newest first');
    assert.equal(await page.evaluate(() => window.__stay), undefined, 'the page was reloaded');
  });

  await check('a restore after the reload works from the file the server keeps at the checkpoint, and one undo takes it back', async () => {
    await tool(agent, 'apply_commands', retitle('b', T1));
    await until('the heading on the server', async () => (await nodes())[heading].text === T1);
    // This page loaded the file before that batch: the app has no record of the file at the checkpoint, the server has.
    const target = page.locator(`[data-testid=checkpoint][data-checkpoint="${after.n}"]`);
    await target.locator('[data-testid=checkpoint-restore] button').click();
    await page.locator('[data-testid=restore-yes]').click();
    await until('restored', async () => (await nodes())[heading].text === T0);
    assert.match(await message(page), /Restored/);
    await until('a row before the restore', async () => (await words(page)).some((t) => /Before a restore/.test(t)));
    await page.locator('canvas').first().focus();
    await page.keyboard.press('Control+z');
    await until('the restore undone', async () => (await nodes())[heading].text === T1);
  });

  const posts = '**/v1/files/*/checkpoints';
  await check('a revert waits for its row: a held post changes nothing, and the revert goes ahead once the row is made', async () => {
    await tool(agent, 'apply_commands', retitle('c', T2));
    await until('the batch live in the page', async () => (await page.locator('[data-testid=batch]').count()) >= 1 && (await nodes())[heading].text === T2);
    let held;
    await page.route(posts, (route) => (route.request().method() === 'POST' ? (held = route) : route.continue()));
    for (const b of await page.locator('[data-testid=lane] > button[aria-expanded=false]').all()) await b.click();
    await page.locator('[data-testid=batch-revert]').first().click();
    await page.locator('[data-testid=revert-confirm-yes]').click();
    await until('the post is held', async () => held !== undefined);
    await new Promise((r) => setTimeout(r, 1500));
    assert.equal((await nodes())[heading].text, T2, 'nothing changed before the row');
    const before = (await rows()).checkpoints.length;
    await held.continue();
    await page.unroute(posts);
    await until('the revert reached the server', async () => (await nodes())[heading].text !== T2);
    assert.equal((await rows()).checkpoints.length, before + 1, 'the row was made first');
    assert.match(await words(page).then((w) => w.join('|')), /Before reverting claude's batch/);
  });

  await check('a post that fails refuses the revert in words and changes nothing; trying again works', async () => {
    await tool(agent, 'apply_commands', retitle('d', T2));
    await until('the heading', async () => (await nodes())[heading].text === T2);
    await page.route(posts, (route) => (route.request().method() === 'POST' ? route.abort() : route.continue()));
    await page.locator('[data-testid=batch-revert]').first().click();
    await page.locator('[data-testid=revert-confirm-yes]').click();
    await until('the refusal', async () => /nothing was changed/.test(await message(page)));
    assert.equal((await nodes())[heading].text, T2);
    await page.unroute(posts);
    await page.locator('[data-testid=batch-revert]').first().click();
    await page.locator('[data-testid=revert-confirm-yes]').click();
    await until('the revert', async () => (await nodes())[heading].text !== T2);
  });

  await check('two editors see the same list, and a row made in one arrives in the other without a reload', async () => {
    const other = await open();
    await other.evaluate(() => { window.__stay = true; });
    await until('the same list', async () => JSON.stringify(await ids(other)) === JSON.stringify(await ids(page)));
    await create(page, `shared ${run}`);
    await until('the row in the other editor', async () => (await words(other)).some((t) => t.includes(`shared ${run}`)));
    assert.deepEqual(await ids(other), await ids(page));
    assert.equal(await other.evaluate(() => window.__stay), true, 'no reload');
    assert.deepEqual(await ids(page), (await rows()).checkpoints.map((c) => String(c.n)));
    await shot(other, 'second-editor');
    await other.close();
  });

  if (react) {
    await check('with the chunks of the React copy blocked, the Svelte panel is drawn and lists the checkpoints', async () => {
      const { ctx, page: blocked } = await openWithChunksBlocked(browser, pageUrl(web), pageErrors);
      await blocked.locator('#right-tab-activity').click();
      await blocked.locator('[data-testid=activity]').waitFor({ timeout: 10000 });
      assert.equal(await fibre(blocked, '[data-testid=activity]'), false, 'the panel is the Svelte copy');
      await until('the same list', async () => JSON.stringify(await ids(blocked)) === JSON.stringify((await rows()).checkpoints.map((c) => String(c.n))));
      // It works: Create checkpoint makes a row on the server.
      const before = (await rows()).checkpoints.length;
      await create(blocked, `blocked ${run}`);
      await until('the row on the server', async () => (await rows()).checkpoints.length === before + 1);
      await ctx.close();
    });
  }

  await check('no uncaught error in the pages', async () => assert.deepEqual(pageErrors, []));
  await browser.close();
  const failed = results.filter((r) => !r).length;
  console.log(failed ? `${failed} of ${results.length} checks failed` : `all ${results.length} checks passed`);
  process.exit(failed ? 1 : 0);
})().catch((e) => { console.error(`checkpoints-e2e: ${e.message}`); process.exit(1); });
