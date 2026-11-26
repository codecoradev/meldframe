// Browser check of rule U9 in the web client (#319) and of the one actor view (#320), against a real server: an agent writes
// node names that hold a right-to-left override and a zero width space, and the check asks the screens that draw them.
//   node packages/web-panels/scripts/hostile-names-e2e.mjs [web-url] [server-url]
// The person is the browser editor (its token is the one Vite was given, actor person:ada); the agent is a second token whose
// actor holds a character outside the BMP (U+1F300 then an x: it made the layers panel and the canvas ring disagree on its
// colour). Write that actor with the bytes of the character, not with its code point as text (a text would be an ASCII actor):
//   AGENT="agent:$(printf '\xf0\x9f\x8c\x80')x"
// Start, on ports of your own, with a file name that is new for each run of a server without Postgres:
//   MELDFRAME_LISTEN=127.0.0.1:8084 MELDFRAME_DEV_TOKENS="<.dev-token>:ws_dev:person:ada,<token 2>:ws_dev:$AGENT" \
//     MELDFRAME_REVIEWERS=person:ada MELDFRAME_ALLOW_DEV_TOKENS=1 meldframe-server
//   (in apps/web) MELDFRAME_WEB_TOKEN=<.dev-token> MELDFRAME_SERVER=http://127.0.0.1:8084 MELDFRAME_WEB_FILE=<file> \
//     MELDFRAME_WEB_ACTOR=person:ada npx vite --port 5185
//   MELDFRAME_WEB_FILE=<file> MELDFRAME_DEMO_TOKEN2=<token 2> node packages/web-panels/scripts/hostile-names-e2e.mjs http://localhost:5185 http://127.0.0.1:8084
// With MELDFRAME_UI=react the script opens the pages that have a React copy with their switch (/?react=layers,variables,properties,activity,manage,io
// for the editor, so the Layers panel, the Variables panel, the Properties panel, the Activity panel with its lanes, the manage view and the Import and Export variables dialogs are the React copies;
// /?react=export for the Export dialog; /?react=feedback for the error page) and first asks that React drew them.
// MELDFRAME_E2E_SHOTS=<dir> saves page images (2x, 1440x900).
// Needs the WASM bridge built (`just wasm-build`) and Playwright from the global module path. The web and server URLs must be loopback.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { rgbCount, until } from './activity-helpers.mjs';

const { chromium } = createRequire(import.meta.url)('playwright');
const react = process.env.MELDFRAME_UI === 'react';
const web = (process.argv[2] ?? 'http://localhost:5185').replace(/\/?$/, '/');
const server = process.argv[3] ?? 'http://127.0.0.1:8084';
const agent = process.env.MELDFRAME_DEMO_TOKEN2;
const file = process.env.MELDFRAME_WEB_FILE;
const shots = process.env.MELDFRAME_E2E_SHOTS;
if (!agent) throw new Error('set MELDFRAME_DEMO_TOKEN2 to the second token (see the top of this file)');
if (!file) throw new Error('set MELDFRAME_WEB_FILE to the file the page opens (see the top of this file)');
for (const u of [web, server]) {
  if (!['127.0.0.1', 'localhost', '[::1]'].includes(new URL(u).hostname)) throw new Error(`refusing to send tokens to ${u}: the web and server URLs must be loopback`);
}
const run = Math.random().toString(36).slice(2, 7);
const RLO = '\u202e', ZWSP = '\u200b';
// What a screen must never show: the controls and the bidirectional and zero-width characters of the hostile names.
const HIDDEN = /[\u00ad\u200b-\u200f\u202a-\u202e\u2060-\u206f\u2800\u3164\ufeff]/;
const results = [];
let rpc = 0;

async function tool(token, name, args) {
  const res = await fetch(`${server}/mcp`, {
    method: 'POST',
    headers: { 'content-type': 'application/json', accept: 'application/json, text/event-stream', authorization: `Bearer ${token}` },
    body: JSON.stringify({ jsonrpc: '2.0', id: ++rpc, method: 'tools/call', params: { name, arguments: { file, ...args } } }),
  });
  return JSON.parse((await res.json()).result?.content?.[0]?.text ?? '{}');
}
async function check(name, fn) {
  try { await fn(); results.push(true); console.log(`ok   ${name}`); } catch (e) {
    results.push(false);
    console.log(`FAIL ${name}: ${String(e.message ?? e).split('\n').slice(0, process.env.MELDFRAME_E2E_DEBUG ? 12 : 1).join(' | ')}`);
  }
}

const hero = `hero_${run}`, title = `title_${run}`, col = `col_${run}`, bg = `bg_${run}`;
const HERO = `Hero ${run}`, TITLE = `Title ${run}`;
// The three colours of an agent in both themes, by tone (packages/web-tokens/src/tokens.ts): the ring is drawn in one of them.
const TONES = { agent1: ['#FB923C', '#C2410C'], agent2: ['#2DD4BF', '#0E7490'], agent3: ['#C084FC', '#7E22CE'] };
const toneOf = (cls) => ({ 'bg-agent-1': 'agent1', 'bg-agent-2': 'agent2', 'bg-agent-3': 'agent3', 'bg-person': 'person' })[cls];

const browser = await chromium.launch({ args: ['--use-gl=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
const pageErrors = [];
const open = async (url, { dsf = 1, downloads = false } = {}) => {
  const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: dsf, acceptDownloads: downloads });
  const page = await ctx.newPage();
  page.on('pageerror', (e) => { pageErrors.push(e.message); console.log('[pageerror]', e.message); });
  await page.goto(url);
  return page;
};
const live = (page) => until('live', async () => (await page.getAttribute('[data-testid=connection]', 'data-state')) === 'live');
const shot = (page, name) => (shots ? page.screenshot({ path: `${shots}/${name}.png` }) : null);
const hasHidden = (s) => HIDDEN.test(s);

// The agent writes a frame and a text whose names hold a right-to-left override and a zero width space, and says so in a note.
const who = await tool(agent, 'whoami', {});
assert.equal(who.actor, 'agent:\u{1F300}x', `the agent token must belong to agent:U+1F300x, not ${who.actor}`);
const batch = await tool(agent, 'apply_commands', { idempotencyKey: `hostile-${run}-a`, note: 'Hostile names.', commands: [
  { type: 'addNode', node: { id: hero, type: 'frame', name: `${HERO.slice(0, 2)}${RLO}${HERO.slice(2)}${ZWSP}`, parentId: null, x: 0, y: 0, w: 480, h: 260, fill: '#1d3b6e' } },
  { type: 'addNode', node: { id: title, type: 'text', name: `Ti${ZWSP}tle ${run}${RLO}`, parentId: hero, x: 32, y: 32, w: 300, h: 40, text: 'Hello', fontSize: 24, fill: '#ffffff' } },
  { type: 'addCollection', collection: { id: col, name: `appearance-${run}`, modes: [{ id: 'light', name: `Li${ZWSP}ght${RLO}` }, { id: 'dark', name: `Da${RLO}rk ${ZWSP}` }] } },
  { type: 'addVariable', variable: { id: bg, collection: col, name: 'color/bg', type: 'color', values: { light: '#ffffff', dark: '#111111' } } },
] });
assert.equal(batch.ok, true, `the agent's write: ${JSON.stringify(batch)}`);

const page = await open(react ? `${web}?react=layers,variables,properties,activity,manage,io` : web, { dsf: 2 });
await live(page);
const layer = (id) => page.locator(`[data-testid=layer][data-id="${id}"]`);
const fibre = (sel) => page.evaluate((s) => Object.keys(document.querySelector(s) ?? {}).some((k) => k.startsWith('__reactFiber$')), sel);
let layerTone;

await check('the Layers, the Variables and the Properties panels are drawn by the copy that was asked for', async () => {
  await layer(hero).waitFor({ timeout: 15000 });
  await page.waitForSelector('[data-testid=variables]', { timeout: 15000 }); // the React side loads on demand
  assert.equal(await fibre('[data-testid=layers]'), react, react ? 'the Layers panel has no React fibre: the page fell back to Svelte' : 'the Layers panel has a React fibre on the Svelte run');
  assert.equal(await fibre('[data-testid=variables]'), react, react ? 'the Variables panel has no React fibre: the page fell back to Svelte' : 'the Variables panel has a React fibre on the Svelte run');
  await page.waitForSelector('[data-testid=properties]', { timeout: 15000 });
  assert.equal(await fibre('[data-testid=properties]'), react, react ? 'the Properties panel has no React fibre: the page fell back to Svelte' : 'the Properties panel has a React fibre on the Svelte run');
});

await check('Variables panel: a mode name with U+202E and U+200B is drawn clean, in the switch and in the name of the value field', async () => {
  const modes = page.locator(`[data-testid=var-modes][data-id="${col}"] [role=radio]`);
  await until('the two modes', async () => (await modes.count()) === 2);
  assert.deepEqual((await modes.allInnerTexts()).map((t) => t.trim()), ['Light', 'Dark']);
  const field = page.locator(`[data-testid=var-row][data-id="${bg}"] [data-testid=var-value]`);
  assert.equal(await field.getAttribute('aria-label'), 'color/bg in Light');
  assert.equal(hasHidden(await page.locator('[data-testid=variables]').evaluate((e) => e.outerHTML)), false, 'the markup of the Variables panel holds a hidden character');
  await shot(page, react ? 'u9web-variables-react' : 'u9web-variables-svelte');
});

await check('Manage view: two modes whose names hold U+202E and U+200B are drawn clean (a collection or variable name cannot hold them: the core refuses it), by the copy that was asked for', async () => {
  await page.click('[data-testid=var-manage]');
  const dlg = page.locator('[data-testid=manage-dialog]');
  await dlg.waitFor();
  assert.equal(await fibre('[data-testid=manage-dialog]'), react, react ? 'the manage view has no React fibre: the page fell back to Svelte' : 'the manage view has a React fibre on the Svelte run');
  const NAME = `appearance-${run}`;
  await until('the collection is listed', async () => (await dlg.locator(`[data-testid=manage-collection][data-id="${col}"]`).count()) === 1);
  assert.equal((await dlg.locator(`[data-testid=manage-collection][data-id="${col}"] span.truncate`).innerText()).trim(), NAME, 'the collection in the list');
  assert.equal((await dlg.locator('[data-testid=manage-title]').innerText()).trim(), NAME, 'the title of the collection shown');
  const modes = dlg.locator('[data-testid=manage-mode] span.truncate');
  assert.deepEqual((await modes.allInnerTexts()).map((t) => t.trim()), ['Light', 'Dark']);
  assert.equal(await dlg.locator('[data-testid=manage-rename-mode]').first().getAttribute('aria-label'), 'Rename mode Light');
  assert.equal(hasHidden(await dlg.evaluate((e) => e.outerHTML)), false, 'the markup of the manage view holds a hidden character');
  await shot(page, react ? 'u9web-manage-react' : 'u9web-manage-svelte');
  await page.keyboard.press('Escape');
  await dlg.waitFor({ state: 'detached' });
});

await check('Export and Import variables: the text of the export and the report of an import carry clean names, by the copy that was asked for; the file name in the note is a known gap', async () => {
  // The markup of a dialog without what the person typed (the field) and the exported text, which is file content: the first is looked at as markup, the second as text.
  const chrome = (sel) => page.locator(sel).evaluate((e) => { const c = e.cloneNode(true); c.querySelectorAll('textarea,[data-testid=export-vars-code]').forEach((n) => n.remove()); return c.outerHTML; });
  await page.click('[data-testid=var-export]');
  const ex = page.locator('[data-testid=export-vars-dialog]');
  await ex.waitFor();
  assert.equal(await fibre('[data-testid=export-vars-dialog]'), react, react ? 'the Export variables dialog has no React fibre: the page fell back to Svelte' : 'the Export variables dialog has a React fibre on the Svelte run');
  // The name of a mode is in a rule of the export (the default mode of a collection is in :root): its slug has no hidden character, and nor does the collection's name.
  const code = await ex.locator('[data-testid=export-vars-code]').innerText();
  assert.equal(hasHidden(code), false, 'the exported text holds a hidden character');
  assert.ok(code.includes(`--appearance-${run}-color-bg: #ffffff;`), code);
  assert.ok(code.includes(`:root[data-mode-appearance-${run}="da-rk"] {`), code);
  assert.equal(hasHidden(await chrome('[data-testid=export-vars-dialog]')), false, 'the markup of the Export variables dialog holds a hidden character');
  await shot(page, react ? 'u9web-export-variables-react' : 'u9web-export-variables-svelte');
  await page.keyboard.press('Escape');
  await ex.waitFor({ state: 'detached' });
  await page.click('[data-testid=var-import]');
  const im = page.locator('[data-testid=import-vars-dialog]');
  await im.waitFor();
  assert.equal(await fibre('[data-testid=import-vars-dialog]'), react, react ? 'the Import variables dialog has no React fibre: the page fell back to Svelte' : 'the Import variables dialog has a React fibre on the Svelte run');
  // Names with the overrides in rules that are not the root: nothing is imported (the file is not changed) and the report names what was skipped.
  await page.fill('[data-testid=import-vars-text]', `.card${RLO} { --local${ZWSP}: #ffffff; }\n.x { --other${RLO}: 1; }`);
  await page.click('[data-testid=import-vars-import]');
  await page.locator('[data-testid=import-vars-result][data-status=nothing]').waitFor();
  const rows = await page.locator('[data-testid=import-vars-report] li').allInnerTexts();
  assert.ok(rows.length >= 2, `the report has its lines: ${JSON.stringify(rows)}`);
  assert.ok(rows.some((r) => r.includes('.card')) && rows.some((r) => r.includes('.x')), JSON.stringify(rows));
  assert.equal(hasHidden(await chrome('[data-testid=import-vars-dialog]')), false, 'the markup of the Import variables dialog holds a hidden character');
  await shot(page, react ? 'u9web-import-variables-react' : 'u9web-import-variables-svelte');
  // A KNOWN GAP of rule U9, the same on both copies: the note about a file the person chose is made in the page from the name the browser gives, which the
  // app layer never sees, so it is drawn as it came. This asserts the gap, so that whoever closes it (the note worded by the app layer) is sent here to change it.
  const [chooser] = await Promise.all([page.waitForEvent('filechooser'), page.click('[data-testid=import-vars-choose]')]);
  await chooser.setFiles({ name: `th${RLO}eme${ZWSP}.css`, mimeType: 'text/css', buffer: Buffer.from(':root { --a: 1; }') });
  await until('the note', async () => (await page.locator('[data-testid=import-vars-note]').innerText()) !== '');
  assert.equal(await page.locator('[data-testid=import-vars-note]').innerText(), `Read th${RLO}eme${ZWSP}.css (17 bytes). Check it, then import.`, 'the file name in the note is clean now: the gap is closed, so change this check to assert it');
  await page.keyboard.press('Escape');
  await im.waitFor({ state: 'detached' });
});

await check('Layers panel: a node name with U+202E and U+200B is drawn clean, in the row, the label and the title', async () => {
  await layer(hero).waitFor({ timeout: 15000 });
  const name = (await layer(hero).locator('button.truncate, button >> span.truncate').first().innerText()).trim();
  assert.equal(name, HERO, `the name shown: ${JSON.stringify(name)}`);
  const html = await page.locator('[data-testid=layers]').evaluate((e) => e.outerHTML);
  assert.equal(hasHidden(html), false, 'the markup of the Layers panel holds a hidden character');
  assert.equal(await layer(title).locator('span.truncate').first().innerText(), TITLE);
  await shot(page, 'u9web-layers-panel');
});

await check('Layers panel: the actor of a layer is the clean name of the agent, with the tone of the app layer', async () => {
  const actor = layer(hero).locator('[data-testid=actor]');
  await actor.waitFor();
  assert.equal(await actor.getAttribute('title'), 'Last changed by \u{1F300}x');
  const dot = actor.locator('span.rounded-pill');
  layerTone = toneOf((await dot.getAttribute('class')).split(/\s+/).find((c) => c.startsWith('bg-')));
  assert.ok(['agent1', 'agent2', 'agent3'].includes(layerTone), `the dot of an agent: ${layerTone}`);
  assert.equal(await actor.getAttribute('data-tone'), layerTone, 'the row says the same tone as its dot');
});

await check('Properties: the name field starts from the name as it is shown', async () => {
  await layer(hero).locator('button[aria-current], button.truncate').first().click();
  const field = page.locator('[data-testid=prop-name]');
  await field.waitFor();
  assert.equal(await field.inputValue(), HERO);
  await shot(page, 'u9web-properties');
});

await check('Canvas: the touch ring of the agent is drawn in the same colour as its layer row and its lane', async () => {
  // A ring stays while the lane works (10 s after a write): write again, then count the pixels of each tone on the canvas.
  const box = { x: 300, y: 60, width: 740, height: 740 };
  const count = (tone) => rgbCount(page, box, TONES[tone]);
  const before = Object.fromEntries(await Promise.all(Object.keys(TONES).map(async (t) => [t, await count(t)])));
  const again = await tool(agent, 'apply_commands', { idempotencyKey: `hostile-${run}-b`, commands: [
    { type: 'updateNode', id: hero, patch: { x: 40 } },
    { type: 'addNode', node: { id: `card_${run}`, type: 'rect', name: `Ca${RLO}rd ${run}${ZWSP}`, parentId: hero, x: 20, y: 120, w: 100, h: 60, fill: '#ffffff' } },
  ] });
  assert.equal(again.ok, true, JSON.stringify(again));
  const seen = await until('the ring of the agent', async () => {
    const now = Object.fromEntries(await Promise.all(Object.keys(TONES).map(async (t) => [t, (await count(t)) - before[t]])));
    return now[layerTone] > 20 ? now : null;
  }, 12000);
  for (const t of Object.keys(TONES)) if (t !== layerTone) assert.ok(seen[t] < seen[layerTone] / 2, `the ring is not drawn in ${t}: ${JSON.stringify(seen)}`);
  await shot(page, 'u9web-ring');
});

await check('Activity: the lane is named by the clean actor name and wears the colour of its layer rows', async () => {
  await page.locator('#right-tab-activity').click();
  const lane = page.locator('[data-testid=lane][data-actor="agent:\u{1F300}x"]');
  await lane.waitFor({ timeout: 15000 });
  assert.equal(await fibre('[data-testid=activity]'), react, react ? 'the Activity panel has no React fibre: the page fell back to Svelte' : 'the Activity panel has a React fibre on the Svelte run');
  assert.equal((await lane.locator('bdi').first().innerText()).trim(), '\u{1F300}x');
  const dot = lane.locator('[data-tone]').first();
  assert.equal(await dot.getAttribute('data-tone'), layerTone, 'the lane and the layer row carry one tone');
  // The same colour on the screen, not only the same word: the computed colour of the two dots.
  const colour = (loc) => loc.evaluate((e) => getComputedStyle(e).backgroundColor);
  const laneColour = await colour(dot);
  await page.locator('#right-tab-design').click();
  assert.equal(await colour(layer(hero).locator('[data-testid=actor] span.rounded-pill')), laneColour, 'one actor, one colour');
  await page.locator('#right-tab-activity').click();
  assert.equal(hasHidden(await page.locator('[data-testid=activity]').evaluate((e) => e.outerHTML)), false, 'the Activity panel holds a hidden character');
  // The second batch was written while the page was open, so the app holds its ops and has a summary with the node names.
  const summary = await page.locator('[data-testid=batch]').first().innerText();
  assert.match(summary, new RegExp(`1 added, 1 changed: .*Card ${run}`), `the batch summary names the node: ${summary}`);
  assert.equal(hasHidden(summary), false);
  await shot(page, 'u9web-activity-lane');
  await page.locator('#right-tab-design').click();
});

await check('Export: the dialog and the saved files carry a clean name, on the Svelte and the React copies alike', async () => {
  const ex = await open(react ? `${web}?react=export` : web, { dsf: 2, downloads: true });
  await live(ex);
  await ex.locator(`[data-testid=layer][data-id="${hero}"] button.truncate`).first().click();
  await ex.waitForSelector('[data-testid=export-section]');
  await ex.click('[data-testid=export-section-html]');
  const dialog = ex.locator('[data-testid=export-dialog]');
  await dialog.waitFor();
  if (react) {
    const kind = await dialog.evaluate((e) => Object.keys(e).some((k) => k.startsWith('__reactFiber$')));
    assert.equal(kind, true, 'the dialog is not drawn by React: a fallback to Svelte would pass the rest');
  }
  const labelled = await dialog.getAttribute('aria-labelledby');
  assert.equal((await ex.locator(`[id="${labelled}"]`).innerText()).trim(), `Export ${HERO}`);
  // The code preview shows the exported document, which carries the node names as written (data-name, title): file content, not a name drawn by a screen.
  const chrome = await dialog.evaluate((e) => { const c = e.cloneNode(true); c.querySelector('[data-testid=export-code]')?.remove(); return c.outerHTML; });
  assert.equal(hasHidden(chrome), false, 'the dialog holds a hidden character');
  const [html] = await Promise.all([ex.waitForEvent('download'), dialog.locator('[data-testid=export-download]').click()]);
  assert.equal(html.suggestedFilename(), `${HERO}.html`, 'the HTML is saved under a name with no hidden character');
  await shot(ex, react ? 'u9web-export-react' : 'u9web-export-svelte');
  await dialog.locator('[role=tab]', { hasText: 'PNG' }).click();
  const [png] = await Promise.all([ex.waitForEvent('download'), dialog.locator('[data-testid=export-png]').click()]);
  assert.equal(png.suggestedFilename(), `${HERO}.png`, 'the PNG is saved under the same name');
  await ex.context().close();
});

await check('Error page: what a server said is made safe by the app layer, on the Svelte and the React copies alike', async () => {
  const bad = await open('about:blank', { dsf: 2 });
  await bad.route('**/v1/files/**', (route) => route.fulfill({ status: 500, contentType: 'text/plain', body: `bad${RLO}${ZWSP} body\u0007!` }));
  await bad.goto(react ? `${web}?react=feedback` : web);
  const detail = bad.locator('[data-testid=error-detail]');
  await detail.waitFor({ timeout: 15000 });
  if (react) assert.equal(await bad.locator('[data-testid=error-page]').evaluate((e) => Object.keys(e).some((k) => k.startsWith('__reactFiber$'))), true, 'the error page is not drawn by React');
  assert.equal((await detail.innerText()).trim(), 'The server answered 500: bad body!');
  assert.equal(hasHidden(await bad.locator('[data-testid=error-page]').evaluate((e) => e.outerHTML)), false);
  await shot(bad, react ? 'u9web-error-react' : 'u9web-error-svelte');
  await bad.context().close();
});

await page.context().close();
await browser.close();
const failed = results.filter((r) => !r).length;
console.log(`\n${results.length - failed}/${results.length} checks passed${pageErrors.length ? `, ${pageErrors.length} page error(s)` : ''}`);
process.exit(failed || pageErrors.length ? 1 : 0);
