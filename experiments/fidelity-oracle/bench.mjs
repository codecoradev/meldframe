// Approach comparison, speed: core layout (Rust, via the spike CLI: wall time minus a tiny-doc baseline) vs browser CSS layout of the exported
// HTML (Chromium, forced full relayout + read of every box). Plain wall-clock, medians; NOT a rigorous benchmark.
import fs from 'node:fs';
import path from 'node:path';
import { performance } from 'node:perf_hooks';
import { serve, launch, openMeasurePage, cli, OUT, RESULTS } from './lib.mjs';
import { frame, rect, text } from './fixtures.mjs';

const POOL = ['Fast', 'Faithful and predictable', 'Agent-friendly with structured errors', 'Design tools should produce layouts a browser can reproduce', 'Short', 'A somewhat longer caption that wraps in a narrow card', 'Pricing', 'Ship it'];
function bigDoc(target) {
  const nodes = [frame('root', null, { x: 0, y: 0, w: 1200, sh: 'hug', layout: 'column', padding: 16, gap: 16, fill: '#fff' })];
  let i = 0;
  while (nodes.length < target) {
    const r = `r${i}`;
    nodes.push(frame(r, 'root', { sw: 'fill', sh: 'hug', layout: 'row', padding: 0, gap: 16, fill: '#f4f4f5' }));
    for (let k = 0; k < 3; k++) {
      const c = `${r}c${k}`;
      nodes.push(frame(c, r, { sw: 'fill', sh: 'hug', layout: 'column', padding: 12, gap: 8, radius: 8, fill: '#fff' }));
      nodes.push(rect(`${c}i`, c, { sw: 'fill', h: 60 + (i % 4) * 10, fill: '#60a5fa' }));
      nodes.push(text(`${c}a`, c, POOL[(i + k) % 8], { sw: 'fill', fontSize: 14 }));
      nodes.push(text(`${c}b`, c, POOL[(i + k + 3) % 8], { sw: 'fill', fontSize: 12 }));
    }
    i++;
  }
  return nodes;
}
const med = (a) => [...a].sort((x, y) => x - y)[Math.floor(a.length / 2)];
const { base, close } = await serve();
const browser = await launch();
const mp = await openMeasurePage(browser, base);
const out = [];
const timeCli = (np, tp, fmt) => { const t = []; for (let k = 0; k < 7; k++) { const a = performance.now(); const r = cli(np, tp, fmt); t.push(performance.now() - a); if (r.status !== 0) throw new Error(r.err); } return med(t); };
const tiny = bigDoc(14);
{ const ts = await mp.layout(tiny); fs.writeFileSync(path.join(OUT, 'b-tiny.nodes.json'), JSON.stringify(tiny)); fs.writeFileSync(path.join(OUT, 'b-tiny.table.json'), JSON.stringify(ts.table)); }
const baseline = timeCli(path.join(OUT, 'b-tiny.nodes.json'), path.join(OUT, 'b-tiny.table.json'), 'rects');
for (const n of [1000, 5000, 20000]) {
  const nodes = bigDoc(n);
  const t0 = performance.now();
  const ts = await mp.layout(nodes);
  const tsMs = performance.now() - t0; // includes Pixi measuring + page round trip
  const np = path.join(OUT, `b-${n}.nodes.json`), tp = path.join(OUT, `b-${n}.table.json`);
  fs.writeFileSync(np, JSON.stringify(nodes)); fs.writeFileSync(tp, JSON.stringify(ts.table));
  const rustWall = timeCli(np, tp, 'rects');
  const ex = cli(np, tp, 'html');
  const htmlFile = `b-${n}.html`;
  fs.writeFileSync(path.join(OUT, htmlFile), ex.out.replace("url('inter.woff2')", "url('/font/inter.woff2')"));
  const page = await (await browser.newContext({ viewport: { width: 1400, height: 900 } })).newPage();
  const t1 = performance.now();
  await page.goto(`${base}/out/${htmlFile}`);
  await page.evaluate(() => document.fonts.ready);
  const loadMs = performance.now() - t1;
  const dom = await page.evaluate(() => {
    const els = [...document.querySelectorAll('[data-node]')];
    const readAll = () => { let s = 0; for (const e of els) s += e.getBoundingClientRect().width; return s; };
    const root = document.querySelector('[data-node="root"]');
    const times = [];
    for (let k = 0; k < 7; k++) { root.style.gap = (16 + (k % 2)) + 'px'; const a = performance.now(); readAll(); times.push(performance.now() - a); }
    times.sort((a, b) => a - b);
    return { relayoutReadMs: times[3], elements: els.length };
  });
  out.push({ nodes: nodes.length, tableEntries: ts.table.length, rustCliWallMs: +rustWall.toFixed(1), rustCliMinusBaselineMs: +(rustWall - baseline).toFixed(1), baselineMs: +baseline.toFixed(1), domLoadMs: +loadMs.toFixed(0), domForcedFullRelayoutPlusReadMs: +dom.relayoutReadMs.toFixed(1), tsOraclePlusPixiMeasureMs: +tsMs.toFixed(0) });
  console.log(JSON.stringify(out.at(-1)));
  await page.context().close();
}
fs.writeFileSync(path.join(RESULTS, 'bench.json'), JSON.stringify(out, null, 1));
await browser.close(); close();
