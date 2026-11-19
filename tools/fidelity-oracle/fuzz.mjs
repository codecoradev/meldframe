import { frame, rect, ell, text } from './fixtures.mjs';

// Seeded fuzz files: random trees over the supported subset (auto layout, hug/fill/fixed, text, rect, ellipse).
function rng(seed) { let a = seed >>> 0; return () => { a = (a + 0x6d2b79f5) >>> 0; let t = a; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; }; }
const WORDS = 'layout canvas export agent design token frame hug fill align gap padding render pixel browser engine system card button title subtitle paragraph caption'.split(' ');
export function fuzz(seed) {
  const R = rng(seed);
  const pick = (a) => a[Math.floor(R() * a.length)];
  const int = (lo, hi) => lo + Math.floor(R() * (hi - lo + 1));
  const nodes = [];
  let id = 0;
  const gen = (parent, depth, parentLayout, parentSw, parentSh) => {
    const kids = depth >= 4 ? 0 : int(1, 4);
    for (let i = 0; i < kids; i++) {
      const nid = `n${id++}`;
      const r = R();
      const inFlex = parentLayout !== 'none';
      // fill only where the core honours it (parent is a layout frame); hug only for text / layout frames
      const mode = () => pick(inFlex ? ['fixed', 'fixed', 'hug', 'fill', 'fill'] : ['fixed']);
      if (r < 0.34 && depth < 4) {
        const layout = pick(['row', 'column', 'row', 'column']);
        const sw = mode(), sh = mode();
        nodes.push(frame(nid, parent, { w: int(60, 300), h: int(40, 200), sw, sh, layout, padding: int(0, 4) * 4 + (R() < 0.2 ? 0.5 : 0), gap: int(0, 4) * 4, align: pick(['start', 'center', 'end']), justify: pick(['start', 'center', 'end', 'between']), fill: pick(['#e4e4e7', '#f4f4f5', '#dbeafe', '#fee2e2']), radius: pick([0, 8]), clip: R() < 0.5 }));
        gen(nid, depth + 1, layout, sw, sh);
      } else if (r < 0.7) {
        const words = Array.from({ length: int(1, 14) }, () => pick(WORDS)).join(' ');
        const sw = pick(inFlex ? ['hug', 'fill', 'fill', 'fixed'] : ['hug', 'fixed']);
        nodes.push(text(nid, parent, words, { fontSize: pick([11, 12, 13, 14, 15, 16, 18, 20, 24]), sw, sh: pick(['hug', 'hug', 'hug', 'fixed']), w: int(60, 220), h: int(16, 60) }));
      } else {
        nodes.push((R() < 0.7 ? rect : ell)(nid, parent, { w: int(10, 120), h: int(10, 100), sw: mode(), sh: mode(), fill: pick(['#f87171', '#60a5fa', '#34d399', '#fbbf24']) }));
      }
    }
  };
  const lay = pick(['row', 'column']);
  const sw = pick(['fixed', 'hug']), sh = pick(['fixed', 'hug']);
  nodes.push(frame('root', null, { x: 10, y: 10, w: int(300, 700), h: int(200, 600), sw, sh, layout: lay, padding: int(0, 5) * 4, gap: int(0, 3) * 4, fill: '#ffffff' }));
  gen('root', 1, lay, sw, sh);
  return nodes;
}
/** Rule G1 (docs/fidelity-contract.md section 5): `fill` on an axis where the parent hugs, or where the parent is not a layout frame,
 *  is `fixed`. The product has no write-time validator for it yet, so the fuzz files are normalised here; the hand-written
 *  fixtures are not (f17 keeps the case as a known gap). */
export function normaliseG1(nodes) {
  const by = Object.fromEntries(nodes.map((n) => [n.id, n]));
  return nodes.map((n) => {
    const p = n.parentId ? by[n.parentId] : null;
    if (!p) return n;
    const flex = p.type === 'frame' && p.layout !== 'none';
    const sw = n.sw === 'fill' && (!flex || p.sw === 'hug') ? 'fixed' : n.sw;
    const sh = n.sh === 'fill' && (!flex || p.sh === 'hug') ? 'fixed' : n.sh;
    return sw === n.sw && sh === n.sh ? n : { ...n, sw, sh };
  });
}

/** `count` seeded files; seed k is `1000 + k`, and a failing one is printed under that name. */
export const fuzzFixtures = (count) => Array.from({ length: count }, (_, i) => ({ name: `fz-${String(i).padStart(3, '0')}`, desc: `seeded fuzz ${i}`, nodes: normaliseG1(fuzz(1000 + i)), fuzz: true }));
