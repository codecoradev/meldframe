// Pixel comparison of the Svelte and the React copy of a panel: node packages/web-panels/scripts/pixels-e2e.mjs [web-url] [server-url]
// Needs the development server (the server binary with MELDFRAME_DEV_TOKENS and no DATABASE_URL, token in .dev-token at the root) and
// Vite running, the WASM bridge built, and Playwright from the global module path; MELDFRAME_WEB_FILE names a file that is new for this run
// (the script adds a frame, four nodes, a collection and three variables to it, and refuses a file that has content already: development server only). Hand-run, not in CI.
// A second token among the server's development tokens, with another actor (MELDFRAME_E2E_TOKEN2, as editor-e2e.mjs has it), makes the last
// node come from another actor, so that the Layers panel's marker of who changed a node is imaged; without it that is not imaged and the
// run says SKIP. The image picker of the Properties panel is imaged open on a workspace that holds no image (a server that has just started,
// or one that no image was uploaded to; image-fill-e2e.mjs uploads some): the step waits for the note of an empty workspace and fails without it.
// For each panel it opens the editor twice on the same file, once with the Svelte copy and once with the React copy (`/?react=<panel>`), puts
// the page through the same steps, takes an image of the panel's element at 1440x900 (1440x1800 for the Properties panel, which is taller than the card) and a device scale factor of 2 after each step, and
// compares the two images pixel by pixel. The tolerance is 0: one differing pixel fails the run. Before it compares, each React page is
// asked that React drew the element and the Svelte page that it did not, so a silent fallback to Svelte cannot pass. A panel is a row of
// PANELS: the switch word, the element, and the steps (each step ends in a state to compare).
// Not covered: the other themes, other widths, a hover or focus ring that the steps do not leave on screen, text of other languages, the
// Layers panel's empty state (the file has nodes), the icon of a shader node and, in the Properties panel, an image fill with an image (the
// picker is imaged open on a workspace with no image; the image tiles are Svelte on both pages until S7), the shader node, the Text
// section's weight select open, the chip of a variable that was removed (a binding whose variable is gone) and Absolute position
// checked. A step only clicks and types: it never changes the file, so the second run starts from the file
// the first one saw.
// The Activity panel is imaged by pixels-activity-e2e.mjs: its rows are live (a batch the page watched arrive can be reverted, one that was
// there when the page loaded cannot) and its lanes change with the clock, so it opens both copies at the same time on one file.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { fibre, makeDiffer } from './pixels-helpers.mjs';

const { chromium } = createRequire(import.meta.url)('playwright');
const web = process.argv[2] ?? 'http://localhost:5173';
const server = process.argv[3] ?? 'http://127.0.0.1:8080';
const root = new URL('../../../', import.meta.url).pathname;
const token = readFileSync(`${root}.dev-token`, 'utf8').trim();
const file = process.env.MELDFRAME_WEB_FILE;
if (!file) throw new Error('set MELDFRAME_WEB_FILE to a file name that is new for this run (see the top of this file)');
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let n = 0;

const token2 = process.env.MELDFRAME_E2E_TOKEN2;
async function post(command, as = token) {
  const res = await fetch(`${server}/v1/files/${file}/tools/apply_commands`, {
    method: 'POST',
    headers: { 'content-type': 'application/json', authorization: `Bearer ${as}` },
    body: JSON.stringify({ ops: [{ key: `pixels-e2e-${++n}`, command, deps: [] }] }),
  });
  assert.equal(res.status, 200, `op refused: ${res.status} ${await res.text()}`);
}
const layer = (page, id) => page.locator(`[data-testid=layer][data-id=${id}]`);
/** Selects a node by its row of the Layers panel and waits for the Properties panel to show it. */
const pick = async (page, id) => {
  const name = (await layer(page, id).locator('button').last().innerText()).trim();
  await layer(page, id).locator('button').last().click();
  await page.waitForFunction((n) => document.querySelector('[data-testid=prop-name]')?.value === n, name);
};

/** The panels this script compares: the word of `/?react=`, the element to image, the steps. Each step leaves the page in a state to compare. */
const PANELS = {
  layers: {
    word: 'layers',
    element: '[data-testid=layers]',
    steps: [
      { name: 'the tree', run: async () => {} },
      { name: 'a selected row', run: (p) => layer(p, 'r1').locator('button').last().click() },
      { name: 'a second row added with Shift', run: (p) => layer(p, 'e1').locator('button').last().click({ modifiers: ['Shift'] }) },
      { name: 'the parent folded', run: (p) => layer(p, 'f1').locator('button[aria-expanded]').click() },
    ],
  },
  variables: {
    word: 'variables',
    element: '[data-testid=variables]',
    steps: [
      { name: 'the list', run: async () => {} },
      { name: 'a node selected (the bind buttons)', run: (p) => layer(p, 'f1').locator('button').last().click() },
      { name: 'a value field focused', run: (p) => p.locator('[data-testid=var-row][data-id=v1] [data-testid=var-value]').focus() },
      { name: 'a search that narrows the list', run: (p) => p.fill('[data-testid=var-search]', 'ink') },
    ],
  },
  properties: {
    word: 'properties',
    element: '[data-testid=properties]',
    viewport: { width: 1440, height: 1800 },
    steps: [
      { name: 'nothing selected', run: async () => {} },
      { name: 'a frame', run: (p) => pick(p, 'f1') },
      { name: 'a rectangle', run: (p) => pick(p, 'r1') },
      { name: 'a text', run: (p) => pick(p, 't1') },
      { name: 'a frame with a row layout, padding, a border and a shadow', run: (p) => pick(p, 'f2') },
      { name: 'a rectangle in an auto layout parent (the margin section)', run: (p) => pick(p, 'r2') },
      { name: 'a linear gradient fill', run: (p) => pick(p, 'g1') },
      { name: 'a fill, an opacity and a corner radius bound to variables', run: (p) => pick(p, 'b1') },
      { name: 'a radial gradient', run: (p) => pick(p, 'g2') },
      { name: 'padding that differs by side (the Mixed placeholder) and four shadows (Add shadow off)', run: (p) => pick(p, 'f3') },
      { name: 'a field focused', run: (p) => p.locator('[data-testid=prop-x]').focus() },
      {
        name: 'the image picker open (a workspace with no image)',
        run: async (p) => {
          await pick(p, 'r1');
          await p.locator('[data-testid=prop-fill-type]').getByRole('radio', { name: 'Image', exact: true }).click();
          // The note of an empty workspace: the picker has read its list (a workspace with images would be imaged before its thumbnails load).
          await p.locator('[data-testid=image-picker-empty]').waitFor({ timeout: 20000 });
        },
      },
    ],
  },
  picker: {
    word: 'variables',
    element: '[data-testid=picker]',
    steps: [
      {
        name: 'open on Fill',
        run: async (p) => {
          await layer(p, 'f1').locator('button').last().click();
          await p.locator('[data-testid=prop-fill-bind]').click();
          await p.locator('[data-testid=picker-row]').first().waitFor();
        },
      },
      { name: 'a search', run: (p) => p.fill('[data-testid=picker-search]', 'ink') },
      { name: 'a search with no match', run: (p) => p.fill('[data-testid=picker-search]', 'zzz') },
    ],
  },
};

const browser = await chromium.launch({ args: ['--use-gl=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });

async function images(word, panel, react) {
  const ctx = await browser.newContext({ viewport: panel.viewport ?? { width: 1440, height: 900 }, deviceScaleFactor: 2, reducedMotion: 'reduce' });
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.goto(react ? `${web.replace(/\/?$/, '/')}?react=${word}` : web);
  await page.waitForSelector('[data-testid=connection][data-state=live]', { timeout: 20000 });
  await page.waitForSelector('[data-testid=layers]', { timeout: 20000 });
  await sleep(600);
  const out = [];
  for (const [i, step] of panel.steps.entries()) {
    await step.run(page);
    if (i === 0) {
      // The element may be there only after the first step (a popover): then ask who drew it.
      await page.waitForSelector(panel.element, { timeout: 20000 });
      assert.equal(await fibre(page, panel.element), react, `${word}: ${react ? 'React did not draw the panel (a fallback to Svelte)' : 'React drew the panel on the Svelte page'}`);
    }
    await sleep(500);
    await page.mouse.move(5, 5);
    await sleep(200);
    out.push((await page.locator(panel.element).first().screenshot()).toString('base64'));
  }
  await ctx.close();
  assert.deepEqual(errors, [], `${word}: page errors`);
  return out;
}

{
  // A file the server does not know yet is 404 and empty; any other answer that is not the file is an error.
  const res = await fetch(`${server}/v1/files/${file}`, { headers: { authorization: `Bearer ${token}` } });
  if (res.status !== 404) {
    assert.equal(res.status, 200, `reading the file: ${res.status}`);
    assert.equal((await res.json()).seq, 0, 'the file has content already: start the page and the check with a file name that is new');
  }
}
await post({ type: 'addNode', node: { id: 'f1', name: 'Frame one', type: 'frame', parentId: null, x: 0, y: 0, w: 400, h: 300, fill: '#ffffff' } });
await post({ type: 'addNode', node: { id: 'r1', name: 'Alpha', type: 'rect', parentId: 'f1', x: 10, y: 10, w: 80, h: 60, fill: '#335577' } });
await post({ type: 'addNode', node: { id: 'e1', name: 'Beta', type: 'ellipse', parentId: 'f1', x: 100, y: 10, w: 80, h: 60, fill: '#775533' } });
await post({ type: 'addNode', node: { id: 'l1', name: 'A layer whose name is far too long to fit in the row of the panel', type: 'rect', parentId: 'f1', x: 10, y: 140, w: 40, h: 20, fill: '#335577' } });
await post({ type: 'addCollection', collection: { id: 'c1', name: 'appearance', modes: [{ id: 'light', name: 'light' }, { id: 'dark', name: 'dark' }] } });
await post({ type: 'addVariable', variable: { id: 'v1', collection: 'c1', name: 'color/bg', type: 'color', values: { light: '#00aa55', dark: '#aa0055' } } });
await post({ type: 'addVariable', variable: { id: 'v2', collection: 'c1', name: 'color/ink', type: 'color', values: { light: '#111111', dark: '#eeeeee' } } });
await post({ type: 'addVariable', variable: { id: 'v3', collection: 'c1', name: 'space/gap', type: 'number', values: { light: 8, dark: 12 } } });
await post({ type: 'addNode', node: { id: 't1', name: 'A text', type: 'text', parentId: 'f1', x: 10, y: 100, w: 120, h: 30, text: 'Hello', fontSize: 16, fill: '#ffffff' } });

// The nodes of the Properties panel's states: a frame with a row layout, padding, a border and a shadow, a rectangle in it (the margin
// section), a rectangle with a gradient and one whose fill, opacity and corner radius are bound to variables.
await post({ type: 'addVariable', variable: { id: 'v4', collection: 'c1', name: 'opacity/half', type: 'number', values: { light: 0.5, dark: 0.75 } } });
await post({ type: 'addNode', node: { id: 'f2', name: 'Frame two', type: 'frame', parentId: null, x: 500, y: 0, w: 300, h: 200, fill: '#eeeeee' } });
await post({ type: 'updateNode', id: 'f2', patch: { layout: 'row', gap: 12, padding: 8, border: { width: 2, color: '#cc3333', position: 'outside' }, shadows: [{ x: 0, y: 4, blur: 8, spread: 0, color: '#00000040' }] } });
await post({ type: 'addNode', node: { id: 'r2', name: 'In the row', type: 'rect', parentId: 'f2', x: 0, y: 0, w: 60, h: 40, fill: '#335577' } });
await post({ type: 'updateNode', id: 'r2', patch: { marT: 6 } });
await post({ type: 'addNode', node: { id: 'g1', name: 'Gradient', type: 'rect', parentId: 'f1', x: 200, y: 140, w: 80, h: 60, fill: '#335577' } });
await post({ type: 'updateNode', id: 'g1', patch: { gradient: { kind: 'linear', angle: 90, stops: [{ pos: 0, color: '#335577' }, { pos: 0.5, color: '#aa5533' }, { pos: 1, color: '#ffffff' }] } } });
await post({ type: 'addNode', node: { id: 'g2', name: 'Radial', type: 'rect', parentId: 'f1', x: 100, y: 220, w: 80, h: 60, fill: '#335577' } });
await post({ type: 'updateNode', id: 'g2', patch: { gradient: { kind: 'radial', angle: 90, stops: [{ pos: 0, color: '#335577' }, { pos: 1, color: '#ffffff' }] } } });
await post({ type: 'addNode', node: { id: 'f3', name: 'Frame three', type: 'frame', parentId: null, x: 500, y: 300, w: 300, h: 200, fill: '#eeeeee' } });
const sh = (y) => ({ x: 0, y, blur: 4, spread: 0, color: '#00000040' });
await post({ type: 'updateNode', id: 'f3', patch: { layout: 'column', padding: 4, padL: 12, shadows: [sh(1), sh(2), sh(3), sh(4)] } });
await post({ type: 'addNode', node: { id: 'b1', name: 'Bound', type: 'rect', parentId: 'f1', x: 300, y: 140, w: 80, h: 60, fill: '#335577' } });
await post({ type: 'updateNode', id: 'b1', patch: { fill: { var: 'v1' }, opacity: { var: 'v4' }, radius: { var: 'v3' } } });

if (token2) await post({ type: 'addNode', node: { id: 'x1', name: 'From another actor', type: 'rect', parentId: 'f1', x: 60, y: 140, w: 40, h: 20, fill: '#775533' } }, token2);
else console.log('SKIP the marker of another actor in the Layers panel: MELDFRAME_E2E_TOKEN2 is not set (a second token with another actor among the server tokens)');

const differ = await makeDiffer(browser);

let bad = 0;
// MELDFRAME_PIXELS_ONLY=properties,picker compares those panels only (the file still gets every node, so the images are the same as in a full run).
const only = process.env.MELDFRAME_PIXELS_ONLY?.split(',');
for (const [name, panel] of Object.entries(PANELS)) {
  if (only && !only.includes(name)) continue;
  const word = panel.word;
  const svelte = await images(word, panel, false);
  const react = await images(word, panel, true);
  for (let i = 0; i < panel.steps.length; i++) {
    const d = await differ(svelte[i], react[i]);
    const ok = !d.size && d.differing === 0;
    if (!ok) bad++;
    console.log(`${ok ? 'ok  ' : 'FAIL'} ${name}: ${panel.steps[i].name}: ${d.size ? `sizes differ ${d.size}` : `${d.differing} of ${d.total} pixels differ`}`);
  }
}
await browser.close();
console.log(bad ? `${bad} states differ` : 'pixels-e2e: ok');
process.exit(bad ? 1 : 0);
