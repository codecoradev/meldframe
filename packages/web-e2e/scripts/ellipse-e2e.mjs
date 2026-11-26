// Browser check of the Ellipse tool (issue 252): the key E and the tool bar button, a drag that makes one node of type `ellipse` with the
// dragged box and the defaults a rectangle gets, the selection, one undo, the pixels the canvas draws (the corner of the box is the page, the
// middle of the edge is the fill: a pill, not a box), the button as a person with a keyboard or a screen reader meets it, and the palette.
//   MELDFRAME_WEB_FILE=<new file> node packages/web-e2e/scripts/ellipse-e2e.mjs <web-url> <server-url> <token>
// Start first: the server (`MELDFRAME_DEV_TOKENS="<token>:ws_dev:agent:dev" MELDFRAME_LISTEN=127.0.0.1:<port> meldframe-server`, in memory), the
// web client on a file that is new for each run (`MELDFRAME_SERVER=<server-url> MELDFRAME_WEB_TOKEN=<token> MELDFRAME_WEB_FILE=<file> npx vite
// --port <port> --strictPort` in apps/web, with the WASM bridge built from this revision), and the same file name in MELDFRAME_WEB_FILE here.
// MELDFRAME_UI=react opens the page with `/?react=properties` (the React copy of the Properties panel, E7.3 slice S6) and first asks that React drew it, so that a
// silent fallback to Svelte cannot pass; without it the Svelte copy runs and the same checks apply.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const { chromium } = createRequire(import.meta.url)('playwright');
const [web, server, token] = process.argv.slice(2);
const file = process.env.MELDFRAME_WEB_FILE;
const react = process.env.MELDFRAME_UI === 'react';
if (!web || !server || !token || !file) throw new Error('usage: MELDFRAME_WEB_FILE=<new file> node ellipse-e2e.mjs <web-url> <server-url> <token>');
const auth = { authorization: `Bearer ${token}` };
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const WHITE = [255, 255, 255], FILL = [0x3f, 0x3f, 0x46];
const pageErrors = [];

const res = await fetch(`${server}/v1/files/${file}/tools/apply_commands`, {
  method: 'POST', headers: { 'content-type': 'application/json', ...auth },
  body: JSON.stringify({ ops: [{ key: 'ellipse-page', deps: [], command: { type: 'addNode', node: { id: 'page', name: 'page', type: 'frame', x: 0, y: 0, w: 800, h: 500, fill: '#ffffff' } } }] }),
});
assert.equal(res.status, 200, `apply_commands: ${res.status} ${await res.clone().text()}`);

const nodesOnServer = async () => (await (await fetch(`${server}/v1/files/${file}`, { headers: auth })).json()).nodes;
async function until(what, fn, ms = 20000) {
  const t0 = Date.now();
  for (;;) {
    const v = await fn();
    if (v) return v;
    if (Date.now() - t0 > ms) throw new Error(`timed out waiting for: ${what}`);
    await sleep(200);
  }
}
const drag = async (page, from, to) => {
  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move((from.x + to.x) / 2, (from.y + to.y) / 2, { steps: 4 });
  await page.mouse.move(to.x, to.y, { steps: 4 });
  await page.mouse.up();
};
const toolOf = (page) => page.getAttribute('[data-testid=tool]', 'data-tool');
let made;
const ellipses = async () => (await nodesOnServer()).filter((n) => n.type === 'ellipse');

const browser = await chromium.launch({ args: ['--use-gl=swiftshader', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist'] });
let failed = 0;
let passed = 0;
const check = async (name, fn) => {
  try { await fn(); passed++; console.log('ok  ', name); } catch (e) { failed++; console.log('FAIL', name, '\n    ', e.message); }
};
try {
  const page = await browser.newPage({ viewport: { width: 1440, height: 860 }, deviceScaleFactor: 1 });
  page.on('pageerror', (e) => { pageErrors.push(e.message); console.log('[pageerror]', e.message); });
  await page.goto(react ? `${web.replace(/\/?$/, '/')}?react=properties` : web);
  await page.waitForSelector('[data-testid=connection][data-state=live]', { timeout: 60_000 });
  await page.waitForSelector('[data-testid=layer][data-id=page]', { timeout: 20_000 });
  // MELDFRAME_UI=react: the page must really be the React copy of the Properties panel (a fallback to Svelte would pass every other check).
  if (react) await page.waitForSelector('[data-testid=properties]', { timeout: 20_000 }); // the React chunk loads on demand: the panel is there a moment after the editor is live
  if (react) assert.equal(await page.evaluate(() => Object.keys(document.querySelector('[data-testid=properties]') ?? {}).some((k) => k.startsWith('__reactFiber$'))), true, 'the Properties panel has no React fibre: the page fell back to Svelte');
  await page.waitForTimeout(2000);

  /** The pixels of the screen at world points of the page frame: the frame is the one pure white area, which gives the zoom and the origin. */
  async function probe(points) {
    const b64 = (await page.screenshot()).toString('base64');
    return page.evaluate(async ({ b64, points }) => {
      const img = new Image();
      img.src = `data:image/png;base64,${b64}`;
      await img.decode();
      const c = document.createElement('canvas');
      c.width = img.width; c.height = img.height;
      const g = c.getContext('2d');
      g.drawImage(img, 0, 0);
      const d = g.getImageData(0, 0, c.width, c.height).data;
      let x0 = 1e9, y0 = 1e9, x1 = 0, y1 = 0;
      for (let y = 60; y < c.height - 60; y++) for (let x = 290; x < 1140; x++) {
        const i = (y * c.width + x) * 4;
        if (d[i] === 255 && d[i + 1] === 255 && d[i + 2] === 255) { x0 = Math.min(x0, x); y0 = Math.min(y0, y); x1 = Math.max(x1, x + 1); y1 = Math.max(y1, y + 1); }
      }
      const s = (x1 - x0) / 800;
      return { s, x0, y0, w: x1 - x0, h: y1 - y0, rows: points.map(([wx, wy]) => { const i = (Math.round(y0 + wy * s) * c.width + Math.round(x0 + wx * s)) * 4; return [d[i], d[i + 1], d[i + 2]]; }) };
    }, { b64, points });
  }
  const frame = await probe([]);
  assert.ok(frame.s > 0.3 && frame.s < 3, `the page frame was not found on the canvas (scale ${frame.s})`);
  // The frame is 800 x 500 in the file: a white area of another shape is not the frame (a panel, a dialog), and every probe would be wrong.
  assert.ok(Math.abs(frame.h / frame.w - 500 / 800) < 0.01, `the white area found is ${frame.w} x ${frame.h}, not the shape of the 800 x 500 page frame`);
  const at = (wx, wy) => ({ x: frame.x0 + wx * frame.s, y: frame.y0 + wy * frame.s });

  await check('the tool bar lists Select, Frame, Rectangle, Ellipse, Text in that order, each a named button', async () => {
    const names = await page.locator('[data-testid=toolbar] [data-testid^=tool-]').evaluateAll((els) => els.map((e) => e.getAttribute('aria-label')));
    assert.deepEqual(names, ['Select', 'Frame', 'Rectangle', 'Ellipse', 'Text']);
  });

  await check('the Ellipse button is as large as its neighbours (at least 24 px) and names itself to a screen reader', async () => {
    const size = async (id) => page.locator(`[data-testid=${id}]`).evaluate((e) => { const r = e.getBoundingClientRect(); return [Math.round(r.width), Math.round(r.height)]; });
    const ellipse = await size('tool-ellipse');
    assert.ok(ellipse[0] >= 24 && ellipse[1] >= 24, `hit target ${ellipse}`);
    assert.deepEqual(ellipse, await size('tool-rectangle'));
    assert.equal(await page.getAttribute('[data-testid=tool-ellipse]', 'aria-label'), 'Ellipse');
    assert.equal(await page.locator('[data-testid=tool-ellipse] svg').count(), 1, 'the button shows an icon');
    assert.equal(await page.getAttribute('[data-testid=tool-ellipse]', 'aria-pressed'), 'false');
  });

  await check('hovering the button shows its name and key in a tooltip', async () => {
    await page.hover('[data-testid=tool-ellipse]');
    const tip = await until('the tooltip', async () => { const t = page.locator('[role=tooltip]'); return (await t.count()) ? (await t.first().innerText()) : null; });
    assert.match(tip, /Ellipse/);
    assert.match(tip, /\bE\b/);
    await page.mouse.move(5, 5);
  });

  await check('E selects the tool; the button is pressed; a drag makes one ellipse with the dragged box, inside the frame, selected; the tool goes back to select', async () => {
    const before = (await nodesOnServer()).length;
    await page.evaluate(() => document.activeElement?.blur());
    await page.keyboard.press('e');
    assert.equal(await toolOf(page), 'ellipse');
    assert.equal(await page.getAttribute('[data-testid=tool-ellipse]', 'aria-pressed'), 'true');
    assert.equal(await page.getAttribute('[data-testid=tool-rectangle]', 'aria-pressed'), 'false');
    await drag(page, at(100, 100), at(300, 220));
    const all = await until('the new node', async () => { const a = await nodesOnServer(); return a.length === before + 1 ? a : null; });
    made = all.find((n) => n.type === 'ellipse');
    assert.ok(made, `no node of type ellipse: ${JSON.stringify(all.map((n) => n.type))}`);
    assert.equal(made.parentId, 'page');
    for (const [k, want] of [['x', 100], ['y', 100], ['w', 200], ['h', 120]]) assert.ok(Math.abs(made[k] - want) <= 3, `${k} is ${made[k]}, want ${want}`);
    assert.equal(await toolOf(page), 'select');
    const row = page.locator(`[data-testid=layer][data-id="${made.id}"]`);
    await row.waitFor();
    assert.equal(await row.getAttribute('data-selected'), 'true', 'the new ellipse is selected in the layers panel');
  });

  await check('the canvas draws a pill: the middle of the edge and the centre are the fill, the corner of the box is the page', async () => {
    const m = made;
    assert.ok(m, 'the earlier check made no ellipse');
    // Deselect (a click on the canvas outside every node) and move the pointer away, so no handle or outline covers the probes.
    await page.mouse.click(frame.x0 - 8, frame.y0 + 100);
    await page.mouse.move(5, 5);
    await page.waitForTimeout(500);
    const pts = [[m.x + m.w / 2, m.y + m.h / 2], [m.x + 3, m.y + m.h / 2], [m.x + m.w - 3, m.y + m.h / 2], [m.x + 4, m.y + 4], [m.x + m.w - 4, m.y + m.h - 4], [m.x + 4, m.y + m.h - 4], [m.x + m.w + 4, m.y + m.h / 2]];
    const got = (await probe(pts)).rows;
    const near = (px, want) => px.every((v, k) => Math.abs(v - want[k]) <= 12);
    const want = [FILL, FILL, FILL, WHITE, WHITE, WHITE, WHITE];
    const names = ['centre', 'left edge', 'right edge', 'top-left corner', 'bottom-right corner', 'bottom-left corner', 'outside the box'];
    got.forEach((px, i) => assert.ok(near(px, want[i]), `${names[i]}: ${px} want ${want[i]}`));
  });

  await check('one undo removes the ellipse; redo brings it back', async () => {
    await page.keyboard.press('Control+z');
    await until('the ellipse removed', async () => (await ellipses()).length === 0);
    await page.keyboard.press('Control+Shift+z');
    await until('the ellipse back', async () => (await ellipses()).length === 1);
    await page.keyboard.press('Control+z');
    await until('the ellipse removed again', async () => (await ellipses()).length === 0);
  });

  await check('a click on the Ellipse button then a drag makes an ellipse with the same fields as the Rectangle tool gives, except the type', async () => {
    const drawn = async (button) => {
      const before = new Set((await nodesOnServer()).map((n) => n.id));
      await page.click(`[data-testid=${button}]`);
      await drag(page, at(400, 100), at(560, 200));
      const n = await until('the new node', async () => (await nodesOnServer()).find((x) => !before.has(x.id)));
      await page.keyboard.press('Control+z');
      await until('the node removed', async () => !(await nodesOnServer()).some((x) => x.id === n.id));
      // The sequence number of the last change differs by the undo steps in between, not by the tool.
      const rest = { ...n };
      for (const k of ['id', 'type', 'pos', 'lastSeq']) delete rest[k];
      return { type: n.type, rest };
    };
    const e = await drawn('tool-ellipse');
    const r = await drawn('tool-rectangle');
    assert.equal(e.type, 'ellipse');
    assert.equal(r.type, 'rect');
    for (const k of ['x', 'y', 'w', 'h']) r.rest[k] = e.rest[k] = Math.round(e.rest[k]);
    assert.deepEqual(e.rest, r.rest);
  });

  await check('a click without a drag makes an ellipse of the default size', async () => {
    await page.click('[data-testid=tool-ellipse]');
    await page.mouse.click(at(400, 300).x, at(400, 300).y);
    const n = (await until('the new ellipse', async () => (await ellipses())[0]));
    assert.equal(n.w, 100);
    assert.equal(n.h, 100);
    await page.keyboard.press('Control+z');
    await until('removed', async () => (await ellipses()).length === 0);
  });

  await check('the Ellipse button from the keyboard: Enter and Space choose the tool, Tab reaches it between Rectangle and Text with a 2 px focus ring', async () => {
    await page.locator('[data-testid=tool-ellipse]').focus();
    await page.keyboard.press('Enter');
    assert.equal(await toolOf(page), 'ellipse');
    await page.click('[data-testid=tool-select]');
    await page.locator('[data-testid=tool-ellipse]').focus();
    await page.keyboard.press('Space');
    assert.equal(await toolOf(page), 'ellipse');
    await page.click('[data-testid=tool-select]');
    // Tab order follows the visual order: Rectangle, Ellipse, Text.
    await page.locator('[data-testid=tool-rectangle]').focus();
    await page.keyboard.press('Tab');
    assert.equal(await page.evaluate(() => document.activeElement?.getAttribute('aria-label')), 'Ellipse');
    assert.equal(await page.evaluate(() => { const o = getComputedStyle(document.activeElement); return [o.outlineStyle, o.outlineWidth].join(' '); }), 'solid 2px', 'the focus ring');
    await page.keyboard.press('Tab');
    assert.equal(await page.evaluate(() => document.activeElement?.getAttribute('aria-label')), 'Text');
    await page.evaluate(() => document.activeElement?.blur());
  });

  await check('E typed into an open menu does not change the tool', async () => {
    await page.click('[data-testid=frame-presets]');
    await page.locator('[role=menu]').waitFor();
    await page.keyboard.press('e');
    assert.equal(await toolOf(page), 'select');
    await page.keyboard.press('Escape');
    await page.evaluate(() => document.activeElement?.blur());
  });

  await check('E typed into the Name field of the properties panel goes into the field and does not change the tool', async () => {
    await page.locator('[data-testid=layer][data-id=page] button').last().click();
    const field = page.locator('[data-testid=prop-name]');
    await field.waitFor();
    await field.focus();
    await page.keyboard.press('End');
    await page.keyboard.press('e');
    assert.equal(await field.inputValue(), 'pagee');
    assert.equal(await toolOf(page), 'select');
    await page.keyboard.press('Backspace');
    assert.equal(await field.inputValue(), 'page');
    await page.evaluate(() => document.activeElement?.blur());
  });

  await check('the command palette lists the Ellipse tool with its key and runs it', async () => {
    await page.keyboard.press('Control+k');
    await page.keyboard.type('ellipse');
    const item = page.locator('[data-testid=palette-option][data-id=tool-ellipse]');
    await item.waitFor();
    const text = await item.innerText();
    assert.match(text, /Ellipse tool/);
    assert.match(text, /\bE\b/);
    await page.keyboard.press('Enter');
    await until('the tool', async () => (await toolOf(page)) === 'ellipse');
    await page.click('[data-testid=tool-select]');
  });

  await check('the shortcut sheet lists Ellipse with the key E', async () => {
    await page.evaluate(() => document.activeElement?.blur());
    await page.keyboard.press('Shift+Slash');
    const hint = page.locator('[data-testid=shortcut-hint]');
    await hint.waitFor();
    assert.match((await hint.innerText()).replace(/\s+/g, ' '), /Ellipse\s*E/);
    await page.keyboard.press('Escape');
  });
} finally {
  await browser.close();
}
if (pageErrors.length) failed++;
if (failed) { console.log(`${failed} failed, ${passed} passed`); process.exit(1); }
console.log(`${passed} checks pass`);
