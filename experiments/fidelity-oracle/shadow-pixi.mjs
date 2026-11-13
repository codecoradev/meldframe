// Oracle D: blurred shadow, Pixi BlurFilter exactly as Canvas.svelte decorate() builds it (strength = blur/2 * zoom, quality 4,
// padding ceil(1.5*blur*zoom)+2) vs Chromium CSS box-shadow, same geometry. Samples the falloff to the left of the box.
import fs from 'node:fs';
import path from 'node:path';
import { serve, launch, OUT, RESULTS } from './lib.mjs';
const { base, close } = await serve();
const browser = await launch();
fs.writeFileSync(path.join(OUT, 'sh-pixi.html'), `<!doctype html><meta charset=utf-8><script src="/vendor/pixi.min.js"></script><body style="margin:0">`);
const decode = async (png) => { const p = await (await browser.newContext()).newPage(); const r = await p.evaluate(async (b64) => { const i = new Image(); i.src = 'data:image/png;base64,' + b64; await i.decode(); const c = document.createElement('canvas'); c.width = i.width; c.height = i.height; const g = c.getContext('2d'); g.drawImage(i, 0, 0); return { w: i.width, d: Array.from(g.getImageData(0, 0, i.width, i.height).data) }; }, png.toString('base64')); await p.context().close(); return (x, y) => r.d[(y * r.w + x) * 4]; };
const results = [];
for (const blur of [8, 20, 40]) {
  const pg = await (await browser.newContext({ viewport: { width: 400, height: 300 }, deviceScaleFactor: 1 })).newPage();
  await pg.goto(`${base}/out/sh-pixi.html`);
  await pg.evaluate(async (blur) => {
    const { Application, Graphics, BlurFilter } = window.PIXI;
    const app = new Application();
    await app.init({ width: 400, height: 300, background: '#ffffff', preference: 'webgl', antialias: true, resolution: 1 });
    document.body.appendChild(app.canvas);
    const g = new Graphics();
    g.roundRect(150, 100, 120, 80, 0).fill('#000000ff');
    g.filters = [new BlurFilter({ strength: blur / 2, quality: 4 })];
    g.filters[0].padding = Math.ceil(blur * 1.5) + 2;
    const cover = new Graphics(); cover.rect(150, 100, 120, 80).fill('#ffffff'); // hide the shape itself: only the shadow outside is compared
    app.stage.addChild(g); app.render(); window.__done = true;
  }, blur);
  await pg.waitForFunction(() => window.__done);
  const pixi = await decode(await pg.screenshot());
  const dg = await (await browser.newContext({ viewport: { width: 400, height: 300 }, deviceScaleFactor: 1 })).newPage();
  await dg.setContent(`<body style="margin:0;background:#fff"><div style="position:absolute;left:150px;top:100px;width:120px;height:80px;background:#000;box-shadow:0 0 ${blur}px 0 #000000ff"></div>`);
  const dom = await decode(await dg.screenshot());
  const rows = [];
  for (const d of [1, 3, 5, 8, 12, 16, 20, 30]) {
    if (150 - d < 0) continue;
    rows.push({ d, pixi: pixi(150 - d, 140), css: dom(150 - d, 140), delta: dom(150 - d, 140) - pixi(150 - d, 140) });
  }
  const edge = { pixiAtEdge: pixi(149, 140), cssAtEdge: dom(149, 140), pixiInside: pixi(160, 140), cssInside: dom(160, 140) };
  const maxAbs = Math.max(...rows.map((r) => Math.abs(r.delta)));
  console.log(`blur ${blur}: max |css - pixi| over falloff = ${maxAbs}/255`, JSON.stringify(rows.map((r) => [r.d, r.pixi, r.css])));
  results.push({ blur, maxAbs, rows, edge });
}
fs.writeFileSync(path.join(RESULTS, 'oracle-D-shadow-blur.json'), JSON.stringify(results, null, 1));
await browser.close(); close();
