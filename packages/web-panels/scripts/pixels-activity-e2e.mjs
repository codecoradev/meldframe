// Pixel comparison of the Svelte and the React copy of the Activity panel (E7.3 slice S8):
//   node packages/web-panels/scripts/pixels-activity-e2e.mjs [web-url] [server-url]
// Needs a server (the server binary with MELDFRAME_DEV_TOKENS and no DATABASE_URL), Vite running on it, the WASM bridge built and Playwright
// from the global module path. Two tokens: the page's (token in .dev-token at the root) and the agent's (MELDFRAME_E2E_TOKEN2, an actor that
// starts with `agent:`, so the server takes a checkpoint when its run starts), and MELDFRAME_REVIEWERS naming the page's actor, or the review
// buttons are not imaged (the script says so). MELDFRAME_WEB_FILE names a file that is new for this run (it adds nodes and two batches, and
// refuses a file that has content already: development server only). Hand-run, not in CI.
// Unlike pixels-e2e.mjs, which opens the copies one after the other on a file that no step changes, this script opens both copies at the same
// time on one file: the Activity panel shows what the page watched arrive (a batch that came while the page was open can be reverted, one that
// was there when it loaded cannot) and the state of a lane follows the clock, so two passes would never meet the same data. Both pages are
// put through the same steps in lockstep, an image of the panel's element is taken from each at 1440x1800 and a device scale factor of 2
// after each step (the card is tall enough that no row is cut), and the two images are compared pixel by pixel with a tolerance of 0. The
// lanes' states must be the same on the two pages when an image is taken, or the run fails and says the clock moved one (they go from
// Working to Quiet 10 s after the last change, and to Idle after 60 s: the steps wait for Quiet and then take well under a minute).
// Before it compares, the React page is asked that React drew the panel and the Svelte page that it did not. A state that differs is
// reported with the box the differing pixels lie in, and MELDFRAME_E2E_SHOTS=<dir> saves the two images of each.
// Not covered: the other theme, other widths, the hover and focus ring that no step leaves on screen, a refusal of the server (the
// status line shows the app layer's own refusal of a name), Show older batches, a batch with no number, text of other languages.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { sleep, until } from './activity-helpers.mjs';
import { fibre, makeDiffer } from './pixels-helpers.mjs';

const { chromium } = createRequire(import.meta.url)('playwright');
const web = process.argv[2] ?? 'http://localhost:5173';
const server = process.argv[3] ?? 'http://127.0.0.1:8080';
const root = new URL('../../../', import.meta.url).pathname;
const token = readFileSync(`${root}.dev-token`, 'utf8').trim();
const agent = process.env.MELDFRAME_E2E_TOKEN2;
const file = process.env.MELDFRAME_WEB_FILE;
if (!agent) throw new Error('set MELDFRAME_E2E_TOKEN2 to the agent token (see the top of this file)');
if (!file) throw new Error('set MELDFRAME_WEB_FILE to a file name that is new for this run (see the top of this file)');
for (const u of [web, server]) {
  if (!['127.0.0.1', 'localhost', '[::1]'].includes(new URL(u).hostname)) throw new Error(`refusing to send tokens to ${u}: the web and server URLs must be loopback`);
}
const run = Math.random().toString(36).slice(2, 7);
let rpc = 0;
let n = 0;

async function mcp(name, args) {
  const res = await fetch(`${server}/mcp`, {
    method: 'POST',
    headers: { 'content-type': 'application/json', accept: 'application/json, text/event-stream', authorization: `Bearer ${agent}` },
    body: JSON.stringify({ jsonrpc: '2.0', id: ++rpc, method: 'tools/call', params: { name, arguments: { file, ...args } } }),
  });
  return JSON.parse((await res.json()).result?.content?.[0]?.text ?? '{}');
}
async function post(command) {
  const res = await fetch(`${server}/v1/files/${file}/tools/apply_commands`, {
    method: 'POST',
    headers: { 'content-type': 'application/json', authorization: `Bearer ${token}` },
    body: JSON.stringify({ ops: [{ key: `pixels-activity-${run}-${++n}`, command, deps: [] }] }),
  });
  assert.equal(res.status, 200, `op refused: ${res.status} ${await res.text()}`);
}

{
  // A file the server does not know yet is 404 and empty; any other answer that is not the file is an error.
  const res = await fetch(`${server}/v1/files/${file}`, { headers: { authorization: `Bearer ${token}` } });
  if (res.status !== 404) {
    assert.equal(res.status, 200, `reading the file: ${res.status}`);
    assert.equal((await res.json()).seq, 0, 'the file has content already: start the page and the check with a file name that is new');
  }
}

// The nodes of the person, written before the pages open: the page of the person would otherwise show a lane of runs whose end is the time
// each page saw the last change arrive, which is a second apart on two pages and says nothing of the copies (the runs are in the unit tests).
await post({ type: 'addNode', node: { id: `f_${run}`, name: 'Frame one', type: 'frame', parentId: null, x: 0, y: 0, w: 400, h: 300, fill: '#ffffff' } });
await post({ type: 'addNode', node: { id: `r_${run}`, name: 'Alpha', type: 'rect', parentId: `f_${run}`, x: 10, y: 10, w: 80, h: 60, fill: '#335577' } });

const browser = await chromium.launch({ args: ['--use-gl=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
const errors = [];
const open = async (react) => {
  const ctx = await browser.newContext({ viewport: { width: 1440, height: 1800 }, deviceScaleFactor: 2, reducedMotion: 'reduce' });
  const page = await ctx.newPage();
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.goto(react ? `${web.replace(/\/?$/, '/')}?react=activity` : web);
  await page.waitForSelector('[data-testid=connection][data-state=live]', { timeout: 20000 });
  return { ctx, page };
};
const [svelte, react] = [await open(false), await open(true)];
const pages = [svelte.page, react.page];

// Two batches of the agent with notes (one holds markup, a quotation mark and a line break), written while both pages are open, so
// that both watch them arrive and can revert them.
const build = { idempotencyKey: `pa-${run}-a`, note: 'Builds the hero: a frame and a title.', commands: [
  { type: 'addNode', node: { id: `h_${run}`, type: 'frame', name: 'Hero', parentId: null, x: 500, y: 0, w: 480, h: 260, fill: '#1d3b6e' } },
  { type: 'addNode', node: { id: `t_${run}`, type: 'text', name: 'Title', parentId: `h_${run}`, x: 32, y: 32, w: 400, h: 40, text: 'Design with agents', fontSize: 32, fill: '#ffffff' } }] };
assert.ok((await mcp('apply_commands', build)).batch?.n >= 1, 'the first batch of the agent');
const retitle = { idempotencyKey: `pa-${run}-b`, note: 'Retitle.\n<b>not bold</b> & "quoted"', commands: [{ type: 'updateNode', id: `t_${run}`, patch: { text: 'Design faster' } }] };
assert.ok((await mcp('apply_commands', retitle)).batch?.n >= 2, 'the second batch of the agent');

const CANCEL = '[data-testid=revert-cancel]';
const NAME = '[data-testid=checkpoint-name]';
const unfold = async (p) => {
  for (let i = 0; i < 8; i++) { // one at a time, asking again: a click can move the lanes
    const b = p.locator('[data-testid=lane] > button[aria-expanded=false]').first();
    if ((await b.count()) === 0) return;
    await b.click();
  }
};
const STEPS = [
  { name: 'every lane unfolded', run: async (p) => { await p.click('#right-tab-activity'); await p.locator('[data-testid=activity]').waitFor(); await unfold(p); await p.locator('[data-testid=batch]').nth(1).waitFor(); } },
  { name: 'the question of a revert', run: async (p) => { await p.locator('[data-testid=batch-revert]').first().click(); await p.locator('[data-testid=revert-confirm]').waitFor(); } },
  { name: 'the question closed with Cancel', run: async (p) => { await p.click(CANCEL); await p.locator('[data-testid=revert-confirm]').waitFor({ state: 'detached' }); } },
  { name: 'the question of a restore', run: async (p) => { await p.locator('[data-testid=checkpoint-restore] button').first().click(); await p.locator('[data-testid=restore-confirm]').waitFor(); } },
  { name: 'a name typed in Create checkpoint', run: async (p) => { await p.locator('[data-testid=restore-cancel] button').click(); await p.fill(NAME, 'Before the pitch'); } },
  { name: 'a name refused, in the status line', run: async (p) => { await p.fill(NAME, 'pay‮now'); await p.click('[data-testid=checkpoint-create]'); await p.locator('[data-testid=activity-message] p').waitFor(); } },
];

const lanes = (p) => p.locator('[data-testid=lane]').evaluateAll((l) => l.map((x) => `${x.dataset.actor}=${x.dataset.state}`));
const differ = await makeDiffer(browser);
let bad = 0;
const fail = (what) => { bad++; console.log(`FAIL ${what}`); };

// The tab counts the two batches on both pages, whichever tab is open.
for (const p of pages) await until('two batches to review on the tab', async () => /\b2\b/.test(await p.locator('#right-tab-activity').innerText()));
const canReview = (await (await fetch(`${server}/v1/files/${file}/batches`, { headers: { authorization: `Bearer ${token}` } })).json()).canReview;
if (!canReview) console.log('NOTE the page\'s actor is not in MELDFRAME_REVIEWERS: the review buttons are not imaged');

for (const [i, step] of STEPS.entries()) {
  await Promise.all(pages.map((p) => step.run(p)));
  if (i === 0) {
    assert.equal(await fibre(svelte.page, '[data-testid=activity]'), false, 'React drew the panel on the Svelte page');
    assert.equal(await fibre(react.page, '[data-testid=activity]'), true, 'React did not draw the panel (a fallback to Svelte)');
    // Wait for every lane to be Quiet (a lane is Working for 10 s after its last change): from here the states do not move for 50 s.
    for (const p of pages) await until('every lane quiet', async () => (await lanes(p)).every((l) => l.endsWith('=quiet')), 40000);
  }
  await sleep(400);
  await Promise.all(pages.map((p) => p.mouse.move(5, 5)));
  await sleep(150);
  const [a, b] = await Promise.all(pages.map(async (p) => (await p.locator('[data-testid=activity]').screenshot()).toString('base64')));
  const states = await Promise.all(pages.map(lanes));
  if (JSON.stringify(states[0]) !== JSON.stringify(states[1])) { fail(`${step.name}: the lanes are not in the same state on the two pages (${states[0]} against ${states[1]}): the clock moved one, run again`); continue; }
  const d = await differ(a, b);
  const ok = !d.size && d.differing === 0;
  if (!ok) bad++;
  console.log(`${ok ? 'ok  ' : 'FAIL'} activity: ${step.name}: ${d.size ? `sizes differ ${d.size}` : `${d.differing} of ${d.total} pixels differ${d.box ? ` (in the box ${d.box} of the image)` : ''}`}`);
  if (!ok && process.env.MELDFRAME_E2E_SHOTS) {
    writeFileSync(`${process.env.MELDFRAME_E2E_SHOTS}/activity-${i}-svelte.png`, Buffer.from(a, 'base64'));
    writeFileSync(`${process.env.MELDFRAME_E2E_SHOTS}/activity-${i}-react.png`, Buffer.from(b, 'base64'));
  }
}
assert.deepEqual(errors, [], 'page errors');
await browser.close();
console.log(bad ? `${bad} states differ` : 'pixels-activity-e2e: ok');
process.exit(bad ? 1 : 0);
