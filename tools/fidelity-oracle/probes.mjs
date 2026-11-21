// Oracle B and D: paint. Pixel probes on the exported pages (border, shadow, image, opacity, gradient, fill bound to a variable, no fill; both forms, and the two forms must
// draw the same pixels), and the falloff of a blurred shadow against the blur the canvas draws. The expectations come from the
// model semantics of the contract (section 4), not from a canvas screenshot.
import fs from 'node:fs';
import path from 'node:path';
import { decodePng, diffPixels, fontsReady, openMeasurePage, OUT, solve, writePage } from './lib.mjs';
import { screenshot } from './screenshot.mjs';
import fixtures from './fixtures.mjs';

const C = {
  red: [255, 0, 0], green: [0, 255, 0], blue: [0, 0, 255], white: [255, 255, 255], pink: [255, 128, 128], mint: [128, 255, 128], lilac: [128, 128, 255],
  yellow: [255, 255, 0], black: [0, 0, 0], gray: [128, 128, 128], purple: [128, 0, 128], zinc: [63, 63, 70] /* DEFAULT_FILL, #3f3f46 */,
};
const near = (p, c, t = 40) => p.every((v, i) => Math.abs(v - c[i]) <= t);
/** The probes of one box: the offsets are from the box's top left corner, so a probe reads as "inside this box". */
const at = (x0, y0, list) => list.map(([what, dx, dy, colour]) => [what, x0 + dx, y0 + dy, colour]);
const gx = (i) => 40 + 180 * i;
/** f19: the boxes of fixtures.mjs, in the order of the gradients. */
const gradient = [
  ...at(gx(0), 40, [['linear 90deg: the first stop is on the left', 20, 40, 'red'], ['linear 90deg: the last stop is on the right', 100, 40, 'blue']]),
  ...at(gx(1), 40, [['linear with no angle is 180deg: first stop on top', 60, 15, 'red'], ['linear with no angle is 180deg: last stop at the bottom', 60, 65, 'blue']]),
  ...at(gx(2), 40, [['linear 0deg points up: the first stop is at the bottom', 60, 65, 'red'], ['linear 0deg points up: the last stop is on top', 60, 15, 'blue']]),
  ...at(gx(3), 40, [['linear 45deg: first stop at the bottom left', 10, 70, 'red'], ['linear 45deg: last stop at the top right', 110, 10, 'blue'], ['linear 45deg: the middle runs from top left to bottom right, top (not 90 or 135)', 45, 5, 'blue'], ['linear 45deg: and below it on the left (not 135 or 270)', 40, 70, 'red'], ['linear 45deg: and right of it, below the centre line (not 0)', 100, 50, 'blue']]),
  ...at(gx(4), 40, [['two stops: near the first colour', 6, 40, 'black'], ['two stops: the middle is the mix', 60, 40, 'gray'], ['two stops: near the last colour', 114, 40, 'white']]),
  ...at(gx(5), 40, [['stop positions: flat before the first position', 10, 40, 'red'], ['stop positions: the mix between 25% and 75%', 60, 40, 'purple'], ['stop positions: flat after the last position', 110, 40, 'blue']]),
  ...at(gx(0), 160, [['radial: the first stop is the centre', 50, 50, 'red'], ['radial: inside the hard stop', 75, 50, 'red'], ['radial: beyond the hard stop', 95, 50, 'blue'], ['radial: the corner is the last stop', 8, 8, 'blue']]),
  ...at(gx(1), 160, [['radial on a wide box: centre', 80, 40, 'red'], ['radial is a circle: inside the radius sideways', 110, 40, 'red'], ['radial is a circle, not an ellipse: beyond it sideways', 132, 40, 'blue'], ['radial is a circle, not an ellipse: inside it vertically', 80, 76, 'red']]),
  ...at(gx(2), 160, [['gradient over the fill: the gradient on the left', 20, 40, 'blue'], ['gradient over the fill: transparent stops show the fill', 100, 40, 'green']]),
  ...at(gx(3), 160, [['ellipse: first stop on the left', 30, 50, 'red'], ['ellipse: last stop on the right', 70, 50, 'blue'], ['ellipse: the corner is clipped', 4, 4, 'white']]),
  ...at(gx(4), 160, [['rounded: the corner is clipped', 2, 2, 'white'], ['rounded: first stop on the left', 20, 40, 'red'], ['rounded: last stop on the right', 100, 40, 'blue']]),
  ...at(gx(5), 160, [['under an image: the letterbox shows the gradient, top', 50, 10, 'green'], ['under an image: the letterbox shows the gradient, bottom', 50, 90, 'yellow'], ['under an image: the image is over the gradient, left', 25, 40, 'red'], ['under an image: the image is over the gradient, right', 75, 40, 'blue']]),
];
/** f20 and f21: what a fill bound to `brand/color/primary` (v1) and `brand/color/surface` (v2) paints in a mode, and the corner and the
 *  opacity that `brand/radius/round` (v4, 40 or 0) and `brand/opacity/muted` (v5, 0.5 or 1) give. */
const bound = ({ v1, v2, corner, muted }) => [
  ...at(40, 40, [['bound fill', 50, 40, v1]]),
  ...at(220, 40, [['alias of a variable', 50, 40, v1]]),
  ...at(400, 40, [['bound fill with a radius', 50, 40, v2], ['bound fill with a radius: the corner is clipped', 2, 2, 'white']]),
  ...at(580, 40, [['bound fill on an ellipse', 40, 40, v1], ['bound fill on an ellipse: the corner is clipped', 4, 4, 'white']]),
  ...at(760, 40, [['bound fill on a frame', 80, 40, v2], ['bound fill on a child of a bound frame', 30, 40, v1]]),
  ...at(40, 170, [['alias of a variable of another collection', 50, 40, v1]]),
  ...at(220, 170, [['a binding that does not resolve paints the default fill', 50, 40, 'zinc']]),
  ...at(400, 170, [['bound fill under a gradient: the gradient', 20, 40, 'blue'], ['bound fill under a gradient: transparent stops show the bound fill', 80, 40, v2]]),
  ...at(580, 170, [['bound fill under an image: the letterbox, top', 50, 6, v2], ['bound fill under an image: the letterbox, bottom', 50, 74, v2], ['bound fill under an image: the image, left', 25, 40, 'red'], ['bound fill under an image: the image, right', 75, 40, 'blue']]),
  ...at(760, 170, [['bound radius: the corner', 2, 2, corner], ['bound radius: the middle is the fill', 50, 40, 'green']]),
  ...at(40, 300, [['bound opacity', 50, 40, muted]]),
  ['the bound fill of the stage', 20, 20, 'white'],
];
/** f22: no fill. */
const noFill = [
  ['fill none on a rect shows what is behind', 90, 80, 'white'],
  ['a frame with fill none shows the box behind it', 300, 90, 'green'], ['a frame with fill none keeps its border', 243, 90, 'red'],   ['a layout frame with fill none paints its child', 480, 80, 'blue'], ['a layout frame with fill none paints nothing of its own', 570, 80, 'white'],
  ['a node with no fill at all takes the default fill', 690, 80, 'zinc'],
  ['fill none under a gradient: the gradient', 800, 80, 'red'], ['fill none under a gradient: transparent stops show what is behind', 920, 80, 'white'],
  ['an ellipse with fill none: the ring', 44, 220, 'red'], ['an ellipse with fill none: the inside shows what is behind', 90, 220, 'white'], ['an ellipse with fill none: outside the ring', 42, 172, 'white'],
  ['fill none under an image: the letterbox shows what is behind', 250, 180, 'white'], ['fill none under an image: the image, left', 225, 220, 'red'], ['fill none under an image: the image, right', 275, 220, 'blue'],
];

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
  ['f19-gradient', { width: 1160, height: 300 }, gradient],
  ['f20-variable-fill', { width: 1000, height: 400 }, bound({ v1: 'red', v2: 'green', corner: 'white', muted: 'pink' })],
  ['f21-variable-modes', { width: 1000, height: 400 }, bound({ v1: 'blue', v2: 'yellow', corner: 'green', muted: 'red' })],
  ['f22-no-fill', { width: 1000, height: 300 }, noFill],
  ['f18-opacity', { width: 300, height: 100 }, [
    ['opacity 0.5 on a red rect over white', 60, 50, 'pink'], ['opacity 0.5 on a frame: the fill is half', 170, 70, 'mint'], ['opacity 0.5 on a frame: its child is half too, not opaque', 200, 40, 'lilac'],
  ]],
];

/** `mutate(form, exported)` as in geometry.mjs: the sensitivity check loads a wrong export. */
export async function paint({ browser, base, log = () => {}, mutate = (_form, e) => e }) {
  const docs = CASES.map(([name]) => fixtures.find((f) => f.name === name));
  const { out } = await solve(docs.map(({ name, nodes, variables, modes }) => ({ name, nodes, variables, modes })), await openMeasurePage(browser, base));
  const probes = [], forms = [];
  for (const [i, [name, viewport, list]] of CASES.entries()) {
    const shots = {};
    for (const form of ['css', 'tailwind']) {
      const fr = out.fixtures[i].frames[0];
      const page = await (await browser.newContext({ viewport, deviceScaleFactor: 1 })).newPage();
      await page.goto(await writePage(base, `${name}/paint.${form}`, form, mutate(form, fr[form])));
      await fontsReady(page);
      shots[form] = await decodePng(browser, await screenshot(page));
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
    const pixi = await decodePng(browser, await screenshot(pg));
    const dg = await (await browser.newContext({ viewport: { width: 400, height: 300 }, deviceScaleFactor: 1 })).newPage();
    await dg.setContent(`<body style="margin:0;background:#fff"><div style="position:absolute;left:150px;top:100px;width:120px;height:80px;background:#000;box-shadow:0 0 ${radius}px 0 #000000ff"></div>`);
    const dom = await decodePng(browser, await screenshot(dg));
    const rows = [1, 3, 5, 8, 12, 16, 20, 30].map((d) => ({ d, pixi: pixi(150 - d, 140)[0], css: dom(150 - d, 140)[0] }));
    const maxAbs = Math.max(...rows.map((r) => Math.abs(r.css - r.pixi)));
    results.push({ blur: radius, maxAbs, rows });
    await pg.context().close(); await dg.context().close();
  }
  log(`CSS blur model against Pixi (not the export), max |css - canvas| over the falloff: ${results.map((r) => `${r.blur}px ${r.maxAbs}/255`).join(', ')}`);
  return results;
}
