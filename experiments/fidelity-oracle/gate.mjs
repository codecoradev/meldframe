// CI gate: reads the result files written by run.mjs / pixel.mjs / text-ink.mjs / shadow-pixi.mjs and exits 1 on a contract violation.
// Fixtures listed in KNOWN_GAPS are tracked, not gated: their expected failure must stay a failure (so a fix is noticed) .
import fs from 'node:fs';
import path from 'node:path';
import { RESULTS, OUT } from './lib.mjs';
const j = (p) => JSON.parse(fs.readFileSync(p, 'utf8'));
const fail = [];
const A = j(path.join(RESULTS, 'oracle-A.safe.ws.json'));
const KNOWN_GAPS = new Set(['f14-text-edge']); // URL/slash breaks, CJK break, tab width: Pixi and CSS disagree (contract: unsupported)
for (const r of A) {
  const runs = r.asExported.html;
  for (const [label, m] of Object.entries(runs)) {
    if (!m.fontOk && r.textNodes > 0) fail.push(`${r.name} ${label}: font did not load`);
    if (KNOWN_GAPS.has(r.name)) { if (m.maxDelta <= 1) fail.push(`${r.name}: known gap closed, update the contract`); continue; }
    if (m.maxDelta > 0.05) fail.push(`${r.name} ${label}: html max box delta ${m.maxDelta.toFixed(3)}px > 0.05`);
  }
  if (r.twVsCss && r.twVsCss.maxDelta > 0.001 && !KNOWN_GAPS.has(r.name) && r.name !== 'f05-text-wrap') fail.push(`${r.name}: tailwind differs from css (${r.twVsCss.maxDelta})`);
  if (r.rustVsTs.maxDelta > 1e-6) fail.push(`${r.name}: Rust core != TS oracle (${r.rustVsTs.maxDelta})`);
  if (r.withOverlay && r.withOverlay.maxDelta > 0.05) fail.push(`${r.name}: overlay changes layout boxes (${r.withOverlay.maxDelta})`);
}
const fuzz = j(path.join(RESULTS, 'fuzz-regimes.json')).find((v) => v.variant === 'fuzz.norm.safe.ws');
if (fuzz.fits.bad > 0) fail.push(`fuzz: ${fuzz.fits.bad} nodes > 0.5px in fit-in-container docs`);
const px = j(path.join(RESULTS, 'oracle-B-pixels.json'));
for (const p of px.filter((p) => !p.ok)) fail.push(`pixel probe failed: ${p.id}`);
const ink = j(path.join(RESULTS, 'oracle-C-text-ink.json')).rows;
for (const r of ink) { const o = r.withOverlay; if (Math.max(Math.abs(o.dLeft), Math.abs(o.dRight), Math.abs(o.dTop), Math.abs(o.dBottom)) > 1) fail.push(`text ink ${r.id}: bbox delta > 1px`); }
const blur = j(path.join(RESULTS, 'oracle-D-shadow-blur.json'));
for (const b of blur) if (b.maxAbs > 20) fail.push(`shadow blur ${b.blur}: falloff differs ${b.maxAbs}/255 from the canvas`);
console.log(fail.length ? 'FAIL\n' + fail.join('\n') : 'fidelity gate: PASS');
process.exit(fail.length ? 1 : 0);
