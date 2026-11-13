// Oracle B: pixel probes on the overlay pages (border / shadow / image / text-align), expectations derived from the M3A/M3B model
// semantics (docs/m3a, docs/m3b in the spike), NOT from a canvas screenshot. Run run.mjs first (it writes out/*.overlay.html).
import fs from 'node:fs';
import path from 'node:path';
import { serve, launch, OUT, RESULTS } from './lib.mjs';

const { base, close } = await serve();
const browser = await launch();
const results = [];
const C = { red: [255, 0, 0], green: [0, 255, 0], blue: [0, 0, 255], white: [255, 255, 255] };
const near = (p, c, t = 40) => Math.abs(p[0] - c[0]) <= t && Math.abs(p[1] - c[1]) <= t && Math.abs(p[2] - c[2]) <= t;

async function shot(name, w, h) {
  const ctx = await browser.newContext({ viewport: { width: w, height: h }, deviceScaleFactor: 1 });
  const page = await ctx.newPage();
  await page.goto(`${base}/out/${name}`);
  await page.evaluate(() => document.fonts.ready);
  await page.waitForTimeout(100);
  const png = await page.screenshot();
  await ctx.close();
  const dec = await (await browser.newContext()).newPage();
  const pix = await dec.evaluate(async (b64) => {
    const img = new Image(); img.src = 'data:image/png;base64,' + b64; await img.decode();
    const c = document.createElement('canvas'); c.width = img.width; c.height = img.height;
    const g = c.getContext('2d'); g.drawImage(img, 0, 0);
    return { w: img.width, data: Array.from(g.getImageData(0, 0, img.width, img.height).data) };
  }, png.toString('base64'));
  await dec.context().close();
  return (x, y) => { const o = (y * pix.w + x) * 4; return [pix.data[o], pix.data[o + 1], pix.data[o + 2]]; };
}
const probe = (get, id, x, y, want, note) => {
  const p = get(x, y);
  const ok = near(p, C[want]);
  results.push({ id, at: [x, y], want, got: p, ok, note });
  console.log(`${ok ? 'PASS' : 'FAIL'} ${id} (${x},${y}) want ${want} got rgb(${p.join(',')}) ${note ?? ''}`);
};

// ---- f07: border + shadow ----
let g = await shot('f07-decor.overlay.html', 1000, 520);
probe(g, 'border inside: stripe', 43, 80, 'red', 'x0..x0+8 red');
probe(g, 'border inside: below stripe is fill', 52, 80, 'green');
probe(g, 'border inside: never outside the box', 37, 80, 'white');
probe(g, 'border center: outer half', 237, 80, 'red');
probe(g, 'border center: inner half', 241, 80, 'red');
probe(g, 'border center: fill after stripe', 247, 80, 'green');
probe(g, 'border center: beyond outer edge', 233, 80, 'white');
probe(g, 'border outside: stripe', 435, 80, 'red');
probe(g, 'border outside: box itself is fill', 443, 80, 'green');
probe(g, 'border outside: beyond', 429, 80, 'white');
probe(g, 'rounded corner clipped', 641, 41, 'white');
probe(g, 'rounded border on the straight edge', 642, 80, 'red');
probe(g, 'hard shadow x+20: visible right of box', 170, 240, 'red');
probe(g, 'hard shadow: absent beyond offset', 185, 240, 'white');
probe(g, 'hard shadow: absent opposite side', 30, 240, 'white');
probe(g, 'hard shadow: box fill above shadow', 100, 240, 'green');
probe(g, 'spread 10: right', 365, 240, 'blue');
probe(g, 'spread 10: top', 300, 195, 'blue');
probe(g, 'spread 10: beyond', 375, 240, 'white');
probe(g, 'two shadows same offset: last in list is on top', 770, 240, 'blue', 'model: first = bottom; CSS list is reversed by the overlay');
// blur: measure the falloff outside the left edge of s3 (x=440), y=240, against the Gaussian with sigma = blur/2 = 10
const erfc = (x) => { const t = 1 / (1 + 0.3275911 * Math.abs(x)); const y = 1 - (((((1.061405429 * t - 1.453152027) * t) + 1.421413741) * t - 0.284496736) * t + 0.254829592) * t * Math.exp(-x * x); return x >= 0 ? 1 - y : 1 + y; };
const falloff = [];
for (const d of [1, 5, 10, 20, 30]) {
  const got = g(440 - d, 240)[0];
  const alpha = 0.5 * erfc(d / (10 * Math.SQRT2));
  const want = Math.round(255 * (1 - alpha));
  falloff.push({ distance: d, got, gaussian: want, delta: got - want });
}
console.log('blur falloff (sigma=10):', JSON.stringify(falloff));
results.push({ id: 'shadow blur falloff vs Gaussian sigma=blur/2', falloff, ok: falloff.every((f) => Math.abs(f.delta) <= 8) });

// ---- f08: image fits ----
g = await shot('f08-image.overlay.html', 1000, 200);
probe(g, 'cover: left half red', 60, 90, 'red');
probe(g, 'cover: right half blue', 120, 90, 'blue');
probe(g, 'contain: letterbox top shows fill', 260, 50, 'green');
probe(g, 'contain: image row left', 260, 90, 'red');
probe(g, 'contain: image row right', 320, 90, 'blue');
probe(g, 'contain: letterbox bottom shows fill', 260, 130, 'green');
probe(g, 'fill: stretched top-left', 460, 50, 'red');
probe(g, 'fill: stretched top-right', 520, 50, 'blue');
probe(g, 'fill: stretched bottom-left', 460, 130, 'red');
probe(g, 'cover on 160x100 box: crop edges, red side', 680, 90, 'red');
probe(g, 'cover on 160x100 box: blue side', 760, 90, 'blue');
probe(g, 'radius clips image corner', 641, 41, 'white');
probe(g, 'ellipse clips image corner', 841, 41, 'white');
probe(g, 'ellipse image centre-left is red', 880, 90, 'red');

// ---- f09: text ink boxes under text-align (DOM side only; see text-ink.mjs for the Pixi side) ----
fs.writeFileSync(path.join(RESULTS, 'oracle-B-pixels.json'), JSON.stringify(results, null, 1));
console.log(`\n${results.filter((r) => r.ok).length}/${results.length} probes pass`);
await browser.close();
close();
