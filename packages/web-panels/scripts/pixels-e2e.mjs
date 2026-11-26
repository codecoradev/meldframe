// Pixel comparison of the Svelte and the React copy of a panel: node packages/web-panels/scripts/pixels-e2e.mjs [web-url] [server-url]
// Needs the development server (the server binary with MELDFRAME_DEV_TOKENS and no DATABASE_URL, token in .dev-token at the root) and
// Vite running, the WASM bridge built, and Playwright from the global module path; MELDFRAME_WEB_FILE names a file that is new for this run
// (the script adds a frame and four nodes to it, and refuses a file that has content already: development server only). Hand-run, not in CI.
// A second token among the server's development tokens, with another actor (MELDFRAME_E2E_TOKEN2, as editor-e2e.mjs has it), makes the last
// node come from another actor, so that the Layers panel's marker of who changed a node is imaged; without it that is not imaged and the
// run says SKIP.
// For each panel it opens the editor twice on the same file, once with the Svelte copy and once with the React copy (`/?react=<panel>`), puts
// the page through the same steps, takes an image of the panel's element at 1440x900 and a device scale factor of 2 after each step, and
// compares the two images pixel by pixel. The tolerance is 0: one differing pixel fails the run. Before it compares, each React page is
// asked that React drew the element and the Svelte page that it did not, so a silent fallback to Svelte cannot pass. A panel is a row of
// PANELS: the switch word, the element, and the steps (each step ends in a state to compare).
// Not covered: the other themes, other widths, a hover or focus ring that the steps do not leave on screen, text of other languages, the
// Layers panel's empty state (the file has nodes) and the icon of a shader node.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';

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

/** The panels this script compares: the word of `/?react=`, the element to image, the steps. Each step leaves the page in a state to compare. */
const PANELS = {
  layers: {
    element: '[data-testid=layers]',
    steps: [
      { name: 'the tree', run: async () => {} },
      { name: 'a selected row', run: (p) => layer(p, 'r1').locator('button').last().click() },
      { name: 'a second row added with Shift', run: (p) => layer(p, 'e1').locator('button').last().click({ modifiers: ['Shift'] }) },
      { name: 'the parent folded', run: (p) => layer(p, 'f1').locator('button[aria-expanded]').click() },
    ],
  },
};

const browser = await chromium.launch({ args: ['--use-gl=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
// Whether React drew the element: React puts a fibre on every DOM node it makes; a Svelte page has none.
const fibre = (page, sel) => page.evaluate((s) => Object.keys(document.querySelector(s) ?? {}).some((k) => k.startsWith('__reactFiber$')), sel);

async function images(word, panel, react) {
  const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2, reducedMotion: 'reduce' });
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.goto(react ? `${web.replace(/\/?$/, '/')}?react=${word}` : web);
  await page.waitForSelector('[data-testid=connection][data-state=live]', { timeout: 20000 });
  await page.waitForSelector(panel.element, { timeout: 20000 });
  await sleep(600);
  assert.equal(await fibre(page, panel.element), react, `${word}: ${react ? 'React did not draw the panel (a fallback to Svelte)' : 'React drew the panel on the Svelte page'}`);
  const out = [];
  for (const step of panel.steps) {
    await step.run(page);
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
await post({ type: 'addNode', node: { id: 't1', name: 'A text', type: 'text', parentId: 'f1', x: 10, y: 100, w: 120, h: 30, text: 'Hello', fontSize: 16, fill: '#ffffff' } });

if (token2) await post({ type: 'addNode', node: { id: 'x1', name: 'From another actor', type: 'rect', parentId: 'f1', x: 60, y: 140, w: 40, h: 20, fill: '#775533' } }, token2);
else console.log('SKIP the marker of another actor in the Layers panel: MELDFRAME_E2E_TOKEN2 is not set (a second token with another actor among the server tokens)');

const cmp = await (await browser.newContext()).newPage();
await cmp.setContent('<canvas></canvas>');
const differ = (a, b) =>
  cmp.evaluate(async ({ a, b }) => {
    const load = async (b64) => {
      const im = new Image();
      im.src = `data:image/png;base64,${b64}`;
      await im.decode();
      const c = document.createElement('canvas');
      c.width = im.width;
      c.height = im.height;
      const g = c.getContext('2d');
      g.drawImage(im, 0, 0);
      return g.getImageData(0, 0, c.width, c.height);
    };
    const [p, q] = [await load(a), await load(b)];
    if (p.width !== q.width || p.height !== q.height) return { size: [p.width, p.height, q.width, q.height] };
    let k = 0;
    for (let i = 0; i < p.data.length; i += 4) if (p.data[i] !== q.data[i] || p.data[i + 1] !== q.data[i + 1] || p.data[i + 2] !== q.data[i + 2] || p.data[i + 3] !== q.data[i + 3]) k++;
    return { differing: k, total: p.width * p.height };
  }, { a, b });

let bad = 0;
for (const [word, panel] of Object.entries(PANELS)) {
  const svelte = await images(word, panel, false);
  const react = await images(word, panel, true);
  for (let i = 0; i < panel.steps.length; i++) {
    const d = await differ(svelte[i], react[i]);
    const ok = !d.size && d.differing === 0;
    if (!ok) bad++;
    console.log(`${ok ? 'ok  ' : 'FAIL'} ${word}: ${panel.steps[i].name}: ${d.size ? `sizes differ ${d.size}` : `${d.differing} of ${d.total} pixels differ`}`);
  }
}
await browser.close();
console.log(bad ? `${bad} states differ` : 'pixels-e2e: ok');
process.exit(bad ? 1 : 0);
