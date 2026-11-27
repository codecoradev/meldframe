// Measures what the hover costs when it asks the wasm bridge what is under the pointer (T9, decision 43, question 4): the
// milliseconds of one `hit(x, y)` call through the real wasm glue under Node, for a stated number of nodes.
//
// Usage: node tools/hit-perf.mjs <dir with the nodejs-target glue> [node counts...]   (default 1000 5000 20000)
//   PROFILE=release WASM_TARGET=nodejs bash tools/build-wasm.sh <dir>   builds the glue (needs wasm-bindgen-cli, see that script).
// `MELDFRAME_HIT_CALLS=n` sets the calls of each row (default 4000). It prints one table, and exits 1 when a row is over the
// budget it states (a frame at 60 Hz is 16.7 ms; the budget is a quarter of that for the hover, because the page draws too).
//
// The file: one page frame that does not clip, `n / 101` frames that clip (cards), 100 rectangles in each, on a grid. A "hit" point is over a rectangle (the scan stops at the first box from the front); a "miss" point is over nothing (the scan
// reads every box, the worst case); "rebuild" is the first call after an edit, which works the scene out and indexes it again.
// The rows are for the editor's default (the layout on the canvas) and for the stored boxes.
import { createRequire } from 'node:module';
import { resolve } from 'node:path';

const dir = resolve(process.argv[2] ?? '');
const counts = process.argv.slice(3).map(Number).filter(Boolean);
const sizes = counts.length ? counts : [1000, 5000, 20000];
const calls = Number(process.env.MELDFRAME_HIT_CALLS ?? 4000);
const BUDGET_MS = 16.7 / 4;
const { WasmApp } = createRequire(import.meta.url)(`${dir}/meldframe_wasm.js`);

function file(n) {
  const nodes = [{ id: 'page', type: 'frame', pos: 'a0', x: 0, y: 0, w: 100000, h: 100000, clip: false }];
  const groups = Math.max(1, Math.floor((n - 1) / 101)); // a file holds at most 20,000 nodes
  for (let g = 0; g < groups; g++) {
    const gx = (g % 20) * 1100;
    const gy = Math.floor(g / 20) * 1100;
    nodes.push({ id: `g${g}`, type: 'frame', parentId: 'page', pos: `a${String(g).padStart(6, '0')}`, x: gx, y: gy, w: 1000, h: 1000, clip: true });
    for (let i = 0; i < 100; i++) {
      nodes.push({ id: `r${g}_${i}`, type: 'rect', parentId: `g${g}`, pos: `a${String(i).padStart(3, '0')}`, x: (i % 10) * 100 + 10, y: Math.floor(i / 10) * 100 + 10, w: 80, h: 80 });
    }
  }
  return { nodes, groups };
}

const ms = (f) => {
  const t = process.hrtime.bigint();
  const r = f();
  return [Number(process.hrtime.bigint() - t) / 1e6, r];
};
const stat = (a) => {
  const s = [...a].sort((x, y) => x - y);
  return { mean: a.reduce((x, y) => x + y, 0) / a.length, p99: s[Math.floor(s.length * 0.99)] };
};
const rng = (seed) => () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 2 ** 32);

let over = false;
const rows = [];
for (const n of sizes) {
  const { nodes, groups } = file(n);
  for (const layout of [true, false]) {
    const app = new WasmApp('usr_me', 'perf', 1, JSON.stringify(nodes), 0);
    app.setLayoutOnCanvas(layout);
    const [first] = ms(() => app.hit(5, 5));
    const rand = rng(7);
    const over_a_box = () => {
      const g = Math.floor(rand() * groups);
      const i = Math.floor(rand() * 100);
      return [(g % 20) * 1100 + (i % 10) * 100 + 40, Math.floor(g / 20) * 1100 + Math.floor(i / 10) * 100 + 40];
    };
    const over_nothing = () => [-100 - rand() * 100, -100 - rand() * 100];
    const run = (point) => {
      const t = [];
      for (let k = 0; k < calls; k++) {
        const [x, y] = point();
        t.push(ms(() => app.hit(x, y))[0]);
      }
      return stat(t);
    };
    const hit = run(over_a_box);
    const miss = run(over_nothing);
    // An edit changes the revision: the next call builds the scene and the index again.
    app.dispatch(JSON.stringify({ type: 'move', id: 'r0_0', x: 20, y: 20 }));
    const [rebuild] = ms(() => app.hit(5, 5));
    rows.push({ nodes: nodes.length, layout, first_ms: first, hit_mean_ms: hit.mean, hit_p99_ms: hit.p99, miss_mean_ms: miss.mean, miss_p99_ms: miss.p99, rebuild_ms: rebuild });
    if (hit.p99 > BUDGET_MS || miss.p99 > BUDGET_MS) over = true;
  }
}
const f = (v) => (typeof v === 'number' ? v.toFixed(4) : String(v));
console.log(`hit(x, y) through the wasm glue, ${calls} calls a row, budget for the hover ${BUDGET_MS.toFixed(2)} ms (p99)`);
console.table(rows.map((r) => Object.fromEntries(Object.entries(r).map(([k, v]) => [k, f(v)]))));
if (over) {
  console.error('hit-perf: a row is over the budget');
  process.exit(1);
}
