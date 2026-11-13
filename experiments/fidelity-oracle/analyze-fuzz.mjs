// Classify fuzz documents by whether content fits its containers, then report pass rates per class and variant.
import fs from 'node:fs';
import path from 'node:path';
import { RESULTS, OUT } from './lib.mjs';
const EPS = 0.01;
export function regime(nodes, rects) {
  const flags = { overflow: false, zeroFill: false, collapsedFill: false };
  const by = Object.fromEntries(nodes.map((n) => [n.id, n]));
  for (const n of nodes) {
    const p = n.parentId ? by[n.parentId] : null;
    if (!p || p.type !== 'frame' || p.layout === 'none') continue;
    const r = rects[n.id], pr = rects[p.id];
    const ix = pr.x + p.padding, iy = pr.y + p.padding, iw = pr.w - 2 * p.padding, ih = pr.h - 2 * p.padding;
    if (iw < -EPS || ih < -EPS) flags.collapsedFill = true;
    if (r.x < ix - EPS || r.y < iy - EPS || r.x + r.w > ix + iw + EPS || r.y + r.h > iy + ih + EPS) flags.overflow = true;
    if ((n.sw === 'fill' && r.w < 1) || (n.sh === 'fill' && r.h < 1)) flags.zeroFill = true;
  }
  return flags;
}
const rows = [];
for (const v of ['fuzz', 'fuzz.norm', 'fuzz.norm.safe', 'fuzz.norm.safe.ws']) {
  const res = JSON.parse(fs.readFileSync(path.join(OUT, `oracle-${v}.json`)));
  const agg = { fits: { docs: 0, ok: 0, nodes: 0, bad: 0 }, notfit: { docs: 0, ok: 0, nodes: 0, bad: 0 } };
  agg.errors = res.filter((r) => r.error).map((r) => ({ name: r.name, error: r.error.trim().slice(0, 160) }));
  for (const r of res.filter((r) => !r.error)) {
    const f = regime(r.nodesUsed, r.expRects);
    const k = f.overflow || f.zeroFill || f.collapsedFill ? 'notfit' : 'fits';
    const m = r.asExported.html['1280@1'];
    agg[k].docs++; agg[k].nodes += m.nodes; agg[k].bad += m.gt05; if (m.gt05 === 0) agg[k].ok++;
  }
  rows.push({ variant: v, ...agg });
  if (agg.errors.length) console.log('  rust table-miss errors:', agg.errors.length, agg.errors[0].error);
  console.log(v.padEnd(16), 'fit-in-container docs', `${agg.fits.ok}/${agg.fits.docs} ok, ${agg.fits.bad}/${agg.fits.nodes} nodes >0.5px;`, 'overflow/degenerate docs', `${agg.notfit.ok}/${agg.notfit.docs} ok, ${agg.notfit.bad}/${agg.notfit.nodes} nodes >0.5px`);
}
fs.writeFileSync(path.join(RESULTS, 'fuzz-regimes.json'), JSON.stringify(rows, null, 1));
