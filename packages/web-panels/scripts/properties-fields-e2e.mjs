// Browser check of the fields and controls of the sections of the Properties panel against a real server (the second half of
// properties-e2e.mjs, which checks the panel as a whole):
//   MELDFRAME_WEB_FILE=<a new file name> node packages/web-panels/scripts/properties-fields-e2e.mjs <web-url> <server-url>
// Start the server and the page as the header of properties-e2e.mjs says (a file name that is new for every run, the page's token in
// .dev-token at the root and in the server's token list, loopback URLs, the WASM bridge built from this revision, no defaults). It adds a frame
// with a row frame, a rectangle, a gradient, a text and a node bound to two variables to the file and leaves them: development server only.
// Each step acts on a control with the pointer or the keyboard, reads the node back from the server, and takes one undo: the node has its
// old value again. Driven here: Gap, Padding (all sides and each side), Align and Justify, Width and Height sizing (one choice each), the margin
// of each side and Absolute position of a node in a row, the corner radius, the font size, the height and Y of a free node, the fill's type
// (Radial), the gradient's angle, a stop's colour and position, Add stop and Remove stop, the border's width, colour and position, the shadow's
// X, Y, spread, blur and colour, the bind buttons of the opacity and of the corner radius (a row of the picker), Detach on the opacity chip and on
// the radius chip.
// Not driven in a browser by any script: the other choices of Width and Height sizing, the other types of the fill switch (Solid, Linear, None),
// the angle and stops of a radial gradient, a second shadow, Detach and the bind button on a node that is not in the state above, and the
// controls of a shader node. Driven by other scripts: the weight and the alignment of a text (text-style-e2e.mjs), Remove border
// (border-remove-e2e.mjs), the fill's bind button and Detach on the fill chip (variables-e2e.mjs), the image fill's type, fit, Change image and
// picker (image-fill-e2e.mjs), Clip content, the name, X, W and the opacity (properties-e2e.mjs, text-style-e2e.mjs, editor-e2e.mjs).
// MELDFRAME_UI=react opens the page with `/?react=properties` and first asks that React drew the panel, so that a silent fallback to Svelte
// cannot pass; without it the Svelte copy runs and the same steps apply. The steps share the file; one that fails can fail the ones after it.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { isDeepStrictEqual } from 'node:util';

const { chromium } = createRequire(import.meta.url)('playwright');
const react = process.env.MELDFRAME_UI === 'react';
const [web, server] = process.argv.slice(2);
const root = new URL('../../../', import.meta.url).pathname;
const file = process.env.MELDFRAME_WEB_FILE;
if (!web || !server || !file) throw new Error('usage: MELDFRAME_WEB_FILE=<a new file name> node properties-fields-e2e.mjs <web-url> <server-url> (see the top of this file; no defaults)');
for (const u of [web, server]) {
  if (!['127.0.0.1', 'localhost', '[::1]'].includes(new URL(u).hostname)) throw new Error(`refusing to send the token to ${u}: the web and server URLs must be loopback (127.0.0.1, localhost or ::1)`);
}
const token = readFileSync(`${root}.dev-token`, 'utf8').trim();
const auth = { authorization: `Bearer ${token}` };
const run = Math.random().toString(36).slice(2, 7);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const pageErrors = [];
const results = [];
let n = 0;

async function post(command) {
  const res = await fetch(`${server}/v1/files/${file}/tools/apply_commands`, {
    method: 'POST',
    headers: { 'content-type': 'application/json', ...auth },
    body: JSON.stringify({ ops: [{ key: `fields-${run}-${++n}`, command, deps: [] }] }),
  });
  assert.equal(res.status, 200, `op refused: ${res.status} ${await res.text()}`);
}
// A file the server does not know yet is 404 and empty; any other answer that is not the file is an error (the guard cannot pass on it).
const snapshot = async () => {
  const res = await fetch(`${server}/v1/files/${file}`, { headers: auth });
  if (res.status === 200) return res.json();
  if (res.status === 404) return { seq: 0, nodes: [] };
  throw new Error(`reading the file: ${res.status} ${await res.text()}`);
};
const nodeOf = async (id) => (await snapshot()).nodes.find((x) => x.id === id);
async function until(what, fn, ms = 15000) {
  const t0 = Date.now();
  for (;;) {
    const v = await fn();
    if (v) return v;
    if (Date.now() - t0 > ms) throw new Error(`timed out waiting for: ${what}`);
    await sleep(150);
  }
}
async function check(name, fn) {
  try {
    await fn();
    results.push(true);
    console.log(`ok   ${name}`);
  } catch (e) {
    results.push(false);
    console.log(`FAIL ${name}: ${String(e.message ?? e).split('\n').slice(0, process.env.MELDFRAME_E2E_DEBUG ? 12 : 1).join(' | ')}`);
  }
}

const id = (k) => `${k}_${run}`;
const names = { [id('root')]: `Root ${run}`, [id('row')]: `Row ${run}`, [id('kid')]: `Kid ${run}`, [id('grad')]: `Grad ${run}`, [id('txt')]: `Text ${run}`, [id('bnd')]: `Bound ${run}` };
const stops = [{ pos: 0, color: '#112233' }, { pos: 0.5, color: '#445566' }, { pos: 1, color: '#ffffff' }];
let page;
const field_ = (testid) => page.locator(`[data-testid=${testid}]`).first();
const field = (testid, i = 0) => page.locator(`[data-testid=${testid}]`).nth(i);
const radio = (group, name) => page.locator(`[data-testid=${group}]`).getByRole('radio', { name, exact: true });
// The panel shows the node once its name is in the name field (the panel of the node selected before stays up until then).
const select = async (k) => {
  await page.locator(`[data-testid=layer][data-id="${id(k)}"] button`).last().click();
  await until('the properties panel shows the node', async () => (await field('prop-name').inputValue().catch(() => '')) === names[id(k)]);
};
const type = async (testid, text, i = 0) => {
  const f = field(testid, i);
  await f.fill(text);
  await f.press('Enter');
};
/** Acts on a control, waits for the server to have `want` (read from the node by `read`), takes one undo and waits for the old value. */
async function step(k, read, want, act) {
  const before = read(await nodeOf(id(k)));
  assert.ok(!isDeepStrictEqual(before, want), 'the step must change the value');
  await act();
  await until(`the server has ${JSON.stringify(want)}`, async () => isDeepStrictEqual(read(await nodeOf(id(k))), want));
  await page.locator('[data-testid=undo]').click();
  await until(`one undo brings back ${JSON.stringify(before)}`, async () => isDeepStrictEqual(read(await nodeOf(id(k))), before));
}

(async () => {
  assert.equal((await snapshot()).seq, 0, 'the file has content already: start the page and the check with a file name that is new');
  const add = (k, type, parent, rest) => post({ type: 'addNode', node: { id: id(k), name: names[id(k)], type, parentId: parent && id(parent), ...rest } });
  await add('root', 'frame', null, { x: 0, y: 0, w: 600, h: 400, fill: '#ffffff' });
  await add('row', 'frame', 'root', { x: 10, y: 10, w: 300, h: 100, fill: '#eeeeee' });
  await post({ type: 'updateNode', id: id('row'), patch: { layout: 'row', gap: 8, padding: 8, border: { width: 2, color: '#112233', position: 'inside' }, shadows: [{ x: 1, y: 2, blur: 3, spread: 0, color: '#00000040' }] } });
  await add('kid', 'rect', 'row', { x: 0, y: 0, w: 60, h: 40, fill: '#335577' });
  await add('grad', 'rect', 'root', { x: 10, y: 150, w: 80, h: 60, fill: '#335577' });
  await post({ type: 'updateNode', id: id('grad'), patch: { gradient: { kind: 'linear', angle: 90, stops } } });
  await add('txt', 'text', 'root', { x: 10, y: 250, w: 200, h: 40, fill: '#000000', text: 'Hello', fontSize: 20 });
  await add('bnd', 'rect', 'root', { x: 200, y: 150, w: 80, h: 60, fill: '#335577' });
  await post({ type: 'addCollection', collection: { id: id('c'), name: 'values', modes: [{ id: id('m'), name: 'one' }] } });
  await post({ type: 'addVariable', variable: { id: id('vo'), collection: id('c'), name: 'opacity/half', type: 'number', values: { [id('m')]: 0.5 } } });
  await post({ type: 'addVariable', variable: { id: id('vr'), collection: id('c'), name: 'radius/six', type: 'number', values: { [id('m')]: 6 } } });
  await post({ type: 'updateNode', id: id('bnd'), patch: { opacity: { var: id('vo') }, radius: { var: id('vr') } } });
  const browser = await chromium.launch({ args: ['--use-gl=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
  page = await (await browser.newContext({ viewport: { width: 1280, height: 1000 } })).newPage();
  page.on('pageerror', (e) => { pageErrors.push(e.message); console.log('[pageerror]', e.message); });
  await page.goto(react ? `${web.replace(/\/?$/, '/')}?react=properties` : web);
  await until('the editor is live', async () => (await page.getAttribute('[data-testid=connection]', 'data-state')) === 'live');
  await until('the nodes are listed', async () => (await page.locator(`[data-testid=layer][data-id="${id('bnd')}"]`).count()) === 1);
  await page.waitForSelector('[data-testid=properties]', { timeout: 20000 }); // the React chunk loads on demand
  if (react) assert.equal(await page.evaluate(() => Object.keys(document.querySelector('[data-testid=properties]') ?? {}).some((k) => k.startsWith('__reactFiber$'))), true, 'the Properties panel has no React fibre: the page fell back to Svelte');

  await check('a row frame: Gap, Padding for all sides and for one side, Align and Justify', async () => {
    await select('row');
    await step('row', (x) => x.gap, 20, () => type('prop-gap', '20'));
    await step('row', (x) => x.padding, 12, () => type('prop-padding', '12'));
    for (const [side, key] of [['top', 'padT'], ['right', 'padR'], ['bottom', 'padB'], ['left', 'padL']]) await step('row', (x) => x[key] ?? null, 30, () => type(`prop-padding-${side}`, '30'));
    await step('row', (x) => x.align, 'center', () => radio('prop-align', 'Center').click());
    await step('row', (x) => x.justify, 'between', () => radio('prop-justify', 'Between').click());
  });

  await check('a node in a row: Width sizing Hug, the margin of two sides, Absolute position, the corner radius', async () => {
    await select('kid');
    await step('kid', (x) => x.sw, 'hug', () => radio('prop-sw', 'Hug').click());
    await step('kid', (x) => x.sh, 'fill', () => radio('prop-sh', 'Fill').click());
    for (const [side, key] of [['top', 'marT'], ['right', 'marR'], ['bottom', 'marB'], ['left', 'marL']]) await step('kid', (x) => x[key], 9, () => type(`prop-margin-${side}`, '9'));
    await step('kid', (x) => x.absolute, true, () => field('prop-absolute').click());
    await step('kid', (x) => x.radius, 12, () => type('prop-radius', '12'));
  });

  await check('a gradient: the angle, the colour and the position of a stop, Add stop and Remove stop', async () => {
    await select('grad');
    await step('grad', (x) => x.gradient.kind, 'radial', () => radio('prop-fill-type', 'Radial').click());
    await step('grad', (x) => x.gradient.angle, 45, () => type('prop-gradient-angle', '45'));
    await step('grad', (x) => x.gradient.stops[0].color, '#abcdef', () => type('prop-gradient-color', '#abcdef', 0));
    await step('grad', (x) => x.gradient.stops[1].pos, 0.25, () => type('prop-gradient-pos', '25', 1));
    await step('grad', (x) => x.gradient.stops.length, 4, () => field('prop-gradient-add').click());
    await until('three stop rows are drawn again after the undo', async () => (await page.locator('[data-testid=prop-gradient-stop]').count()) === 3);
    await step('grad', (x) => x.gradient.stops.length, 2, () => field('prop-gradient-remove', 0).click());
  });

  await check('the border: the width, the colour and where it sits; the shadow: X, blur and colour', async () => {
    await select('row');
    await step('row', (x) => x.border.width, 3, () => type('prop-border-width', '3'));
    await step('row', (x) => x.border.color, '#ff0000', () => type('prop-border-color', '#ff0000'));
    await step('row', (x) => x.border.position, 'outside', () => radio('prop-border-position', 'Outside').click());
    await step('row', (x) => x.shadows[0].x, 7, () => type('prop-shadow-x', '7'));
    await step('row', (x) => x.shadows[0].y, 6, () => type('prop-shadow-y', '6'));
    await step('row', (x) => x.shadows[0].spread, 2, () => type('prop-shadow-spread', '2'));
    await step('row', (x) => x.shadows[0].blur, 9, () => type('prop-shadow-blur', '9'));
    await step('row', (x) => x.shadows[0].color, '#00ff0080', () => type('prop-shadow-color', '#00ff0080'));
  });

  await check('a text: the font size, and the height and Y of a node that is free', async () => {
    await select('txt');
    await step('txt', (x) => x.h, 50, () => type('prop-h', '50'));
    await step('txt', (x) => x.y, 260, () => type('prop-y', '260'));
    await step('txt', (x) => x.fontSize, 30, () => type('prop-fontSize', '30'));
  });

  await check('the bind buttons of the opacity and of the corner radius: a row of the picker binds the field, and one undo frees it', async () => {
    await select('kid');
    for (const [field, key, row] of [['opacity', 'opacity', 'opacity/half'], ['radius', 'radius', 'radius/six']]) {
      await step('kid', (x) => typeof x[key], 'object', async () => {
        await field_(`prop-${field}-bind`).click();
        await page.locator('[data-testid=picker-row]', { hasText: row }).click();
      });
    }
  });

  await check('a node bound to two variables: Detach on the opacity chip and on the radius chip makes the value a literal, and one undo binds it again', async () => {
    await select('bnd');
    assert.equal(await field('prop-opacity-bound').count(), 1, 'the opacity is a chip');
    assert.equal(await field('prop-radius-bound').count(), 1, 'the corner radius is a chip');
    await step('bnd', (x) => typeof x.opacity, 'number', () => field('prop-opacity-detach').click());
    await step('bnd', (x) => typeof x.radius, 'number', () => field('prop-radius-detach').click());
  });

  await check('no uncaught error in the page', async () => assert.deepEqual(pageErrors, []));
  await browser.close();
  const failed = results.filter((r) => !r).length;
  console.log(failed ? `${failed} of ${results.length} checks failed` : `all ${results.length} checks passed`);
  process.exit(failed ? 1 : 0);
})().catch((e) => {
  console.error(`properties-fields-e2e: ${e.stack ?? e.message}`);
  process.exit(1);
});
