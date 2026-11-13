// Classify the topmost failing node of every failing fuzz doc in a variant into a root-cause bucket.
import fs from 'node:fs';
import path from 'node:path';
import { OUT, RESULTS } from './lib.mjs';
const variant = process.argv[2] ?? 'fuzz.norm.safe';
const res = JSON.parse(fs.readFileSync(path.join(OUT, `oracle-${variant}.json`))).filter((r) => !r.error);
const buckets = {};
const examples = {};
for (const r of res) {
  const bad = (r.rows ?? []).filter((x) => x.max > 0.5 || x.missing);
  if (!bad.length) continue;
  const by = Object.fromEntries(r.nodesUsed.map((n) => [n.id, n]));
  const order = Object.fromEntries(r.nodesUsed.map((n, i) => [n.id, i]));
  bad.sort((a, b) => order[a.id] - order[b.id]);
  const f = bad[0], n = by[f.id], p = n.parentId ? by[n.parentId] : null;
  const e = r.expRects[f.id];
  const pr = p && r.expRects[p.id];
  let b;
  const padFloor = n.type === 'frame' && n.layout !== 'none' && (e.w < 2 * n.padding - 0.01 || e.h < 2 * n.padding - 0.01);
  if (n.type === 'text' && n.sw === 'fill' && e.w < 1) b = 'text fill resolved to ~0 width (zero free space): wrap behaviour at width 0 differs';
  else if (padFloor) b = 'core box smaller than its own padding (CSS floors a box at padding+border)';
  else if (p && p.layout !== 'none' && pr && (() => { const ik = (p.layout === 'row' ? pr.w : pr.h) - 2 * p.padding; const kids = r.nodesUsed.filter((k) => k.parentId === p.id).map((k) => r.expRects[k.id]); const used = kids.reduce((a, k) => a + (p.layout === 'row' ? k.w : k.h), 0) + p.gap * (kids.length - 1); return used > ik + 0.01; })()) b = 'main-axis overflow in the parent (children + gaps exceed the content box)';
  else if (p && p.layout !== 'none') b = 'other, parent is an auto layout frame';
  else b = 'other';
  buckets[b] = (buckets[b] ?? 0) + 1;
  (examples[b] ??= []).push(r.name + ':' + f.id);
}
console.log(variant, buckets);
fs.writeFileSync(path.join(RESULTS, `fuzz-residual-${variant}.json`), JSON.stringify({ buckets, examples: Object.fromEntries(Object.entries(examples).map(([k, v]) => [k, v.slice(0, 5)])) }, null, 1));
