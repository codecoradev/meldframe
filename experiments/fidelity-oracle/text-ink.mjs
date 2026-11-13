// Oracle C: text glyph placement. The SAME text node is drawn (1) by Pixi exactly as src/lib/Canvas.svelte does it
// (Text + style + slack-based text-align, resolution 1, WebGL) and (2) by the exported HTML (+ candidate overlay for weight/align).
// Compares ink bounding boxes and pixel difference per node.
import fs from 'node:fs';
import path from 'node:path';
import { serve, launch, openMeasurePage, cli, OUT, RESULTS } from './lib.mjs';
import { overlayHtml } from './overlay.mjs';
import { frame, text } from './fixtures.mjs';

const P = 'The quick brown fox jumps over the lazy dog while the export keeps every line break where the canvas put it.';
const cases = [
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
const nodes = [frame('free', null, { x: 0, y: 0, w: 800, h: 40 + cases.length * 100, layout: 'none', fill: '#ffffff' })];
cases.forEach(([id, t, p], i) => nodes.push(text(id, 'free', t, { x: 20, y: 20 + i * 100, fill: '#000000', ...p })));
// as in the app: a wrapped text node's multi-line height must be hug; fixed-width nodes above are sw fixed + sh hug.

const { base, close } = await serve();
const browser = await launch();
const mp = await openMeasurePage(browser, base);
const ts = await mp.layout(nodes);
const np = path.join(OUT, 'f16-ink.nodes.json'), tp = path.join(OUT, 'f16-ink.measure.json');
fs.writeFileSync(np, JSON.stringify(nodes)); fs.writeFileSync(tp, JSON.stringify(ts.table));
const ex = cli(np, tp, 'html');
if (ex.status !== 0) throw new Error(ex.err);
let html = ex.out.replace("url('inter.woff2')", "url('/font/inter.woff2')");
const asExported = html;
// candidate fixes found in oracle A: Pixi white-space semantics ('pre') and per-line line-height for explicit newlines
html = html.replace(/white-space:nowrap/g, 'white-space:pre').replace(/white-space:pre-line/g, 'white-space:pre-wrap;word-break:keep-all').replace('</style>', 'p{white-space:pre-wrap;word-break:keep-all}\n</style>');
for (const n of nodes.filter((q) => q.text.includes('\n'))) {
  const lh = await mp.lineHeight(n.fontSize, n.fontWeight);
  html = html.replace(new RegExp(`(\\.n-${n.id}\\{[^}]*?)line-height:[0-9.]+px`), `$1line-height:${lh}px`);
}
html = await overlayHtml(html, nodes);
fs.writeFileSync(path.join(OUT, 'f16-ink.dom.html'), html);
fs.writeFileSync(path.join(OUT, 'f16-ink.dom-asexported.html'), asExported);
const H = 40 + cases.length * 100;

// Pixi page: same drawing logic as Canvas.svelte
fs.writeFileSync(path.join(OUT, 'f16-ink.pixi.html'), `<!doctype html><meta charset=utf-8><style>@font-face{font-family:'Inter Variable';src:url('/font/inter.woff2') format('woff2');font-weight:100 900;font-display:block}body{margin:0}</style><script src="/vendor/pixi.min.js"></script><body>`);
async function shotPixi() {
  const ctx = await browser.newContext({ viewport: { width: 800, height: H }, deviceScaleFactor: 1 });
  const page = await ctx.newPage();
  await page.goto(`${base}/out/f16-ink.pixi.html`);
  await page.evaluate(async ([nodes, rects, H]) => {
    for (const w of [400, 600, 700, 900]) await document.fonts.load(`${w} 16px "Inter Variable"`);
    await document.fonts.ready;
    const { Application, Text } = window.PIXI;
    const app = new Application();
    await app.init({ width: 800, height: H, background: '#ffffff', preference: 'webgl', antialias: true, resolution: 1, autoDensity: false });
    document.body.appendChild(app.canvas);
    const FONT = 'Inter Variable, system-ui, sans-serif';
    for (const n of nodes) {
      if (n.type !== 'text') continue;
      const r = rects[n.id];
      const t = new Text({ text: '', style: { fontFamily: FONT, fontSize: 16, fill: '#111', wordWrap: true } });
      t.text = n.text; t.style.fontSize = n.fontSize; t.style.fill = n.fill; t.style.fontWeight = String(n.fontWeight ?? 400);
      t.style.wordWrap = n.sw !== 'hug'; t.style.wordWrapWidth = Math.max(8, r.w); t.style.align = n.textAlign ?? 'left'; t.resolution = 1;
      const slack = Math.max(0, r.w - t.width);
      t.position.set(r.x + (n.textAlign === 'center' ? slack / 2 : n.textAlign === 'right' ? slack : 0), r.y);
      app.stage.addChild(t);
    }
    app.render();
    window.__done = true;
  }, [nodes, ts.rects, H]);
  await page.waitForFunction(() => window.__done);
  await page.waitForTimeout(200);
  const png = await page.screenshot();
  await ctx.close();
  return png;
}
async function shotDom(file) {
  const ctx = await browser.newContext({ viewport: { width: 800, height: H }, deviceScaleFactor: 1 });
  const page = await ctx.newPage();
  await page.goto(`${base}/out/${file}`);
  await page.evaluate(() => document.fonts.ready);
  const png = await page.screenshot();
  await ctx.close();
  return png;
}
const pixiPng = await shotPixi();
const domPng = await shotDom('f16-ink.dom.html');
const domAsPng = await shotDom('f16-ink.dom-asexported.html');
fs.writeFileSync(path.join(OUT, 'ink.pixi.png'), pixiPng); fs.writeFileSync(path.join(OUT, 'ink.dom.png'), domPng);

const dec = await (await browser.newContext()).newPage();
const analyse = (pngs) => dec.evaluate(async ([pngs, rects, ids]) => {
  const load = async (b64) => { const i = new Image(); i.src = 'data:image/png;base64,' + b64; await i.decode(); const c = document.createElement('canvas'); c.width = i.width; c.height = i.height; const g = c.getContext('2d'); g.drawImage(i, 0, 0); return g.getImageData(0, 0, i.width, i.height); };
  const [A, B] = await Promise.all(pngs.map(load));
  const ink = (D, x0, y0, x1, y1) => { let minx = 1e9, miny = 1e9, maxx = -1, maxy = -1, mass = 0; for (let y = y0; y < y1; y++) for (let x = x0; x < x1; x++) { const o = (y * D.width + x) * 4; const dark = 255 - (D.data[o] + D.data[o + 1] + D.data[o + 2]) / 3; if (dark > 30) { minx = Math.min(minx, x); miny = Math.min(miny, y); maxx = Math.max(maxx, x); maxy = Math.max(maxy, y); } mass += dark; } return { minx, miny, maxx, maxy, mass }; };
  const out = {};
  for (const id of ids) {
    const r = rects[id]; const x0 = Math.max(0, Math.floor(r.x) - 12), y0 = Math.max(0, Math.floor(r.y) - 12), x1 = Math.min(A.width, Math.ceil(r.x + r.w) + 12), y1 = Math.min(A.height, Math.ceil(r.y + r.h) + 12);
    const a = ink(A, x0, y0, x1, y1), b = ink(B, x0, y0, x1, y1);
    let sad = 0, bad = 0, n = 0;
    for (let y = y0; y < y1; y++) for (let x = x0; x < x1; x++) { const o = (y * A.width + x) * 4; const d = Math.abs(A.data[o] - B.data[o]); sad += d; if (d > 64) bad++; n++; }
    out[id] = { pixi: a, dom: b, dLeft: b.minx - a.minx, dRight: b.maxx - a.maxx, dTop: b.miny - a.miny, dBottom: b.maxy - a.maxy, massRatio: +(b.mass / a.mass).toFixed(3), mad: +(sad / n).toFixed(2), badPct: +(100 * bad / n).toFixed(2) };
  }
  return out;
}, [pngs.map((p) => p.toString('base64')), ts.rects, cases.map((c) => c[0])]);
const withOverlay = await analyse([pixiPng, domPng]);
const asExp = await analyse([pixiPng, domAsPng]);
const rows = cases.map(([id]) => ({ id, withOverlay: withOverlay[id], asExported: { dLeft: asExp[id].dLeft, dRight: asExp[id].dRight, dTop: asExp[id].dTop, dBottom: asExp[id].dBottom, mad: asExp[id].mad } }));
for (const r of rows) {
  const o = r.withOverlay;
  console.log(`${r.id.padEnd(10)} ink delta L/R/T/B (DOM-Pixi) = ${o.dLeft}/${o.dRight}/${o.dTop}/${o.dBottom}  massRatio=${o.massRatio}  MAD=${o.mad}  badPx=${o.badPct}%   | as-exported L/R/T/B=${r.asExported.dLeft}/${r.asExported.dRight}/${r.asExported.dTop}/${r.asExported.dBottom}`);
}
fs.writeFileSync(path.join(RESULTS, 'oracle-C-text-ink.json'), JSON.stringify({ rows, rects: ts.rects }, null, 1));
await browser.close(); close();
