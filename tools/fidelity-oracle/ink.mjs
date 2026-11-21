// Oracle C: where the glyphs are. The same text node is drawn by Pixi the way the canvas draws it (apps/web/src/canvas/stage.ts: Inter
// Variable, the size and fill of the node, word wrap at the box's width) and by the exported page; the bounding boxes of the ink
// are compared. Weight and alignment are not drawn by the product canvas yet (docs/fidelity-contract.md, coverage table): they
// are drawn as the contract says the canvas will (the weight of the node; the text shifted by the slack of the line), so
// those rows prove the export against the contract, not against the canvas.
import fs from 'node:fs';
import path from 'node:path';
import { decodePng, fontsReady, openMeasurePage, OUT, solve, writePage } from './lib.mjs';
import { screenshot } from './screenshot.mjs';
import { frame, text } from './fixtures.mjs';

const P = 'The quick brown fox jumps over the lazy dog while the export keeps every line break where the canvas put it.';
const CASES = [
  ['hug400', 'Hello world, Hamburgefonstiv', { fontSize: 16 }],
  ['hug700', 'Hello world, Hamburgefonstiv', { fontSize: 16, fontWeight: 700 }],
  ['hug900', 'Hello world, Hamburgefonstiv', { fontSize: 16, fontWeight: 900 }],
  ['hug600-24', 'Semibold heading 24', { fontSize: 24, fontWeight: 600 }],
  ['hug13', 'Small caption text at 13px', { fontSize: 13 }],
  ['hug40', 'Display 40', { fontSize: 40 }],
  ['hug9', 'Tiny nine pixel text here', { fontSize: 9 }],
  ['wrapL', P, { sw: 'fixed', w: 300, fontSize: 16, textAlign: 'left' }],
  ['wrapC', P, { sw: 'fixed', w: 300, fontSize: 16, textAlign: 'center' }],
  ['wrapR', P, { sw: 'fixed', w: 300, fontSize: 16, textAlign: 'right' }],
  ['wrapC700', P, { sw: 'fixed', w: 300, fontSize: 16, textAlign: 'center', fontWeight: 700 }],
  ['oneC', 'Centered one line', { sw: 'fixed', w: 300, fontSize: 16, textAlign: 'center' }],
  ['oneR', 'Right one line', { sw: 'fixed', w: 300, fontSize: 16, textAlign: 'right' }],
  ['nlL', 'Line one\nLine two is longer\nThree', { fontSize: 16, textAlign: 'left' }],
  ['nlC', 'Line one\nLine two is longer\nThree', { sw: 'fixed', w: 300, fontSize: 16, textAlign: 'center' }],
  ['nlR', 'Line one\nLine two is longer\nThree', { sw: 'fixed', w: 300, fontSize: 16, textAlign: 'right' }],
  ['cjk', '日本語のテキスト and Latin mixed café', { sw: 'fixed', w: 300, fontSize: 16 }],
];
const H = 40 + CASES.length * 100;
const nodes = [frame('free', null, { x: 0, y: 0, w: 800, h: H, layout: 'none', fill: '#ffffff' })];
CASES.forEach(([id, t, p], i) => nodes.push(text(id, 'free', t, { x: 20, y: 20 + i * 100, fill: '#000000', ...p })));

async function pixiShot(browser, base, rects) {
  const page = await (await browser.newContext({ viewport: { width: 800, height: H }, deviceScaleFactor: 1 })).newPage();
  await page.goto(`${base}/out/ink.html`);
  await page.evaluate(async ([nodes, rects, H]) => {
    for (const w of [400, 600, 700, 900]) await document.fonts.load(`${w} 16px "Inter Variable"`);
    await document.fonts.ready;
    const { Application, Text } = window.PIXI;
    const app = new Application();
    await app.init({ width: 800, height: H, background: '#ffffff', preference: 'webgl', antialias: true, resolution: 1, autoDensity: false });
    document.body.appendChild(app.canvas);
    for (const n of nodes) {
      if (n.type !== 'text') continue;
      const r = rects[n.id];
      const t = new Text({ text: n.text, style: { fontFamily: "'Inter Variable', Inter, system-ui, sans-serif", fontSize: Math.max(1, n.fontSize), fill: n.fill, wordWrap: r.w > 0, wordWrapWidth: Math.max(1, r.w), fontWeight: String(n.fontWeight ?? 400), align: n.textAlign ?? 'left' } });
      t.resolution = 1;
      const slack = Math.max(0, r.w - t.width);
      t.position.set(r.x + (n.textAlign === 'center' ? slack / 2 : n.textAlign === 'right' ? slack : 0), r.y);
      app.stage.addChild(t);
    }
    app.render();
    window.__done = true;
  }, [nodes, rects, H]);
  await page.waitForFunction(() => window.__done);
  await page.waitForTimeout(200);
  const png = await screenshot(page);
  await page.context().close();
  return decodePng(browser, png);
}

const ink = (D, x0, y0, x1, y1) => {
  let minx = 1e9, miny = 1e9, maxx = -1, maxy = -1;
  for (let y = y0; y < y1; y++) for (let x = x0; x < x1; x++) {
    const o = (y * D.width + x) * 4;
    if (255 - (D.data[o] + D.data[o + 1] + D.data[o + 2]) / 3 > 30) { minx = Math.min(minx, x); miny = Math.min(miny, y); maxx = Math.max(maxx, x); maxy = Math.max(maxy, y); }
  }
  return { minx, miny, maxx, maxy };
};

export async function textInk({ browser, base, log = () => {} }) {
  const mp = await openMeasurePage(browser, base);
  const { out } = await solve([{ name: 'ink', nodes }], mp);
  const ans = out.fixtures[0];
  const rects = ans.rects;
  fs.writeFileSync(path.join(OUT, 'ink.html'), `<!doctype html><meta charset=utf-8><style>@font-face{font-family:'Inter Variable';src:url('/inter.woff2') format('woff2');font-weight:100 900;font-display:block}body{margin:0}</style><script src="/vendor/pixi.min.js"></script><body>`);
  const pixi = await pixiShot(browser, base, rects);
  const rows = [];
  for (const form of ['css', 'tailwind']) {
    const page = await (await browser.newContext({ viewport: { width: 800, height: H }, deviceScaleFactor: 1 })).newPage();
    await page.goto(await writePage(base, `ink/free.${form}`, form, ans.frames[0][form]));
    await fontsReady(page);
    const dom = await decodePng(browser, await screenshot(page));
    await page.context().close();
    for (const [id] of CASES) {
      const r = rects[id];
      const box = [Math.max(0, Math.floor(r.x) - 12), Math.max(0, Math.floor(r.y) - 12), Math.min(800, Math.ceil(r.x + r.w) + 12), Math.min(H, Math.ceil(r.y + r.h) + 12)];
      const a = ink(pixi, ...box), b = ink(dom, ...box);
      // no ink on a side gives the sentinels of `ink` and a delta of 0: say so, the gate fails on it
      const noInk = a.maxx < 0 ? 'the canvas text' : b.maxx < 0 ? 'the exported page' : undefined;
      rows.push({ id, form, dLeft: b.minx - a.minx, dRight: b.maxx - a.maxx, dTop: b.miny - a.miny, dBottom: b.maxy - a.maxy, ...(noInk ? { noInk } : {}) });
    }
  }
  const worst = Math.max(...rows.filter((r) => !r.noInk).flatMap((r) => [r.dLeft, r.dRight, r.dTop, r.dBottom].map(Math.abs)));
  log(`text ink: ${CASES.length} cases x 2 forms, largest edge difference ${worst} px`);
  return { rows, worst };
}
