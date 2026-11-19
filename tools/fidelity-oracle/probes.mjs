// Oracle B and D: paint. Pixel probes on the exported pages (border, shadow, image, opacity; both forms, and the two forms must
// draw the same pixels), and the falloff of a blurred shadow against the blur the canvas draws. The expectations come from the
// model semantics of the contract (section 4), not from a canvas screenshot.
import fs from 'node:fs';
import path from 'node:path';
import { decodePng, diffPixels, fontsReady, openMeasurePage, OUT, solve, writePage } from './lib.mjs';
import fixtures from './fixtures.mjs';

const C = { red: [255, 0, 0], green: [0, 255, 0], blue: [0, 0, 255], white: [255, 255, 255], pink: [255, 128, 128], mint: [128, 255, 128], lilac: [128, 128, 255] };
const near = (p, c, t = 40) => p.every((v, i) => Math.abs(v - c[i]) <= t);

/** [fixture, viewport, probes]; a probe is [what, x, y, colour]. */
const CASES = [
  ['f07-decor', { width: 1000, height: 520 }, [
    ['border inside: stripe', 43, 80, 'red'], ['border inside: below the stripe is the fill', 52, 80, 'green'], ['border inside: never outside the box', 37, 80, 'white'],
    ['border center: outer half', 237, 80, 'red'], ['border center: inner half', 241, 80, 'red'], ['border center: fill after the stripe', 247, 80, 'green'], ['border center: beyond the outer edge', 233, 80, 'white'],
    ['border outside: stripe', 435, 80, 'red'], ['border outside: the box itself is the fill', 443, 80, 'green'], ['border outside: beyond', 429, 80, 'white'],
    ['rounded corner is clipped', 641, 41, 'white'], ['rounded border on the straight edge', 642, 80, 'red'],
    ['hard shadow x+20: visible right of the box', 170, 240, 'red'], ['hard shadow: absent beyond the offset', 185, 240, 'white'], ['hard shadow: absent on the opposite side', 30, 240, 'white'], ['hard shadow: the box is above its shadow', 100, 240, 'green'],
    ['spread 10: right', 365, 240, 'blue'], ['spread 10: top', 300, 195, 'blue'], ['spread 10: beyond', 375, 240, 'white'],
    ['two shadows at one offset: the last of the list is on top', 770, 240, 'blue'],
  ]],
  ['f08-image', { width: 1000, height: 200 }, [
    ['cover: left half red', 60, 90, 'red'], ['cover: right half blue', 120, 90, 'blue'],
    ['cover: no letterbox, top left', 60, 50, 'red'], ['cover: no letterbox, bottom right', 120, 130, 'blue'],
    ['contain: the letterbox shows the fill, top', 260, 50, 'green'], ['contain: image row, left', 260, 90, 'red'], ['contain: image row, right', 320, 90, 'blue'], ['contain: the letterbox shows the fill, bottom', 260, 130, 'green'],
    ['fill: stretched, top left', 460, 50, 'red'], ['fill: stretched, top right', 520, 50, 'blue'], ['fill: stretched, bottom left', 460, 130, 'red'],
    ['cover on a 160x100 box: red side', 680, 90, 'red'], ['cover on a 160x100 box: blue side', 760, 90, 'blue'],
    ['a radius clips the image corner', 641, 41, 'white'], ['an ellipse clips the image corner', 841, 41, 'white'], ['ellipse: centre left is red', 880, 90, 'red'],
  ]],
  ['f18-opacity', { width: 300, height: 100 }, [
    ['opacity 0.5 on a red rect over white', 60, 50, 'pink'], ['opacity 0.5 on a frame: the fill is half', 170, 70, 'mint'], ['opacity 0.5 on a frame: its child is half too, not opaque', 200, 40, 'lilac'],
  ]],
];

/** `mutate(form, exported)` as in geometry.mjs: the sensitivity check loads a wrong export. */
export async function paint({ browser, base, log = () => {}, mutate = (_form, e) => e }) {
  const docs = CASES.map(([name]) => fixtures.find((f) => f.name === name));
  const { out } = await solve(docs.map(({ name, nodes }) => ({ name, nodes })), await openMeasurePage(browser, base));
  const probes = [], forms = [];
  for (const [i, [name, viewport, list]] of CASES.entries()) {
    const shots = {};
    for (const form of ['css', 'tailwind']) {
      const fr = out.fixtures[i].frames[0];
      const page = await (await browser.newContext({ viewport, deviceScaleFactor: 1 })).newPage();
      await page.goto(await writePage(base, `${name}/paint.${form}`, form, mutate(form, fr[form])));
      await fontsReady(page);
      shots[form] = await decodePng(browser, await page.screenshot());
      await page.context().close();
      for (const [what, x, y, want] of list) {
        const got = shots[form](x, y);
        const ok = near(got, C[want]);
        probes.push({ id: `${name} ${form}: ${what}`, at: [x, y], want, got, ok });
        if (!ok) log(`FAIL ${name} ${form}: ${what} (${x},${y}) want ${want} got rgb(${got.join(',')})`);
      }
    }
    forms.push({ fixture: name, differing: diffPixels(shots.css, shots.tailwind) });
  }
  log(`paint probes: ${probes.filter((p) => p.ok).length}/${probes.length} pass; pixels that differ between the forms: ${forms.map((f) => `${f.fixture} ${f.differing}`).join(', ')}`);
  return { probes, forms };
}

/** The CSS blur model against the canvas blur, NOT the export: a blurred shadow of Pixi (BlurFilter, strength blur/2, quality 4)
 *  against a hand-written CSS `box-shadow` page of the same geometry, the falloff to the left of a 120x80 box at 1, 3, 5, 8, 12,
 *  16, 20 and 30 px, in 0..255. It says how close the CSS blur the export writes is to the canvas's, not that the export wrote
 *  it; the shadows the export does write are probed on pixels in `paint` (f07), but not their blur. */
export async function blur({ browser, base, log = () => {} }) {
  fs.writeFileSync(path.join(OUT, 'blur.html'), '<!doctype html><meta charset=utf-8><script src="/vendor/pixi.min.js"></script><body style="margin:0">');
  const results = [];
  for (const radius of [8, 20, 40]) {
    const pg = await (await browser.newContext({ viewport: { width: 400, height: 300 }, deviceScaleFactor: 1 })).newPage();
    await pg.goto(`${base}/out/blur.html`);
    await pg.evaluate(async (radius) => {
      const { Application, Graphics, BlurFilter } = window.PIXI;
      const app = new Application();
      await app.init({ width: 400, height: 300, background: '#ffffff', preference: 'webgl', antialias: true, resolution: 1 });
      document.body.appendChild(app.canvas);
      const g = new Graphics();
      g.roundRect(150, 100, 120, 80, 0).fill('#000000ff');
      g.filters = [new BlurFilter({ strength: radius / 2, quality: 4 })];
      g.filters[0].padding = Math.ceil(radius * 1.5) + 2;
      app.stage.addChild(g); app.render(); window.__done = true;
    }, radius);
    await pg.waitForFunction(() => window.__done);
    const pixi = await decodePng(browser, await pg.screenshot());
    const dg = await (await browser.newContext({ viewport: { width: 400, height: 300 }, deviceScaleFactor: 1 })).newPage();
    await dg.setContent(`<body style="margin:0;background:#fff"><div style="position:absolute;left:150px;top:100px;width:120px;height:80px;background:#000;box-shadow:0 0 ${radius}px 0 #000000ff"></div>`);
    const dom = await decodePng(browser, await dg.screenshot());
    const rows = [1, 3, 5, 8, 12, 16, 20, 30].map((d) => ({ d, pixi: pixi(150 - d, 140)[0], css: dom(150 - d, 140)[0] }));
    const maxAbs = Math.max(...rows.map((r) => Math.abs(r.css - r.pixi)));
    results.push({ blur: radius, maxAbs, rows });
    await pg.context().close(); await dg.context().close();
  }
  log(`CSS blur model against Pixi (not the export), max |css - canvas| over the falloff: ${results.map((r) => `${r.blur}px ${r.maxAbs}/255`).join(', ')}`);
  return results;
}
