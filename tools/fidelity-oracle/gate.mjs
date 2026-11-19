// The gate: reads out/results.json (or the file given) and exits 1 on a contract violation or on a known gap that closed.
import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { KNOWN_GAPS } from './known-gaps.mjs';
import { OUT } from './lib.mjs';

export const LIMITS = { geometry: 0.05, tailwind: 0.001 };
/** A run that checks less than this is not a run of the oracle: the hand-written fixtures, and the share of the seeds the gate must hold. */
export const FLOORS = { fixtures: 16, gatedShare: 0.3 };

/** The violations in a results object, as lines of text. */
export function evaluate(results) {
  const fail = [];
  const files = results.geometry?.files ?? [];
  const names = new Set(files.map((d) => d.name));
  if (!(results.seeds >= 1)) fail.push('the run has no fuzz seeds (--seeds must be 1 or more)');
  if (!results.only) {
    const fixtures = files.filter((d) => !d.fuzz), fuzz = files.filter((d) => d.fuzz);
    if (fixtures.length < FLOORS.fixtures) fail.push(`only ${fixtures.length} fixtures ran (at least ${FLOORS.fixtures})`);
    if (fuzz.length !== results.seeds) fail.push(`${fuzz.length} fuzz files ran for ${results.seeds} seeds`);
    const gated = fuzz.filter((d) => !d.unspecified).length;
    if (gated < Math.floor(results.seeds * FLOORS.gatedShare)) fail.push(`only ${gated} of ${results.seeds} fuzz files are inside the contract and gated (at least ${FLOORS.gatedShare * 100}%): the generator or rule G3 changed`);
    for (const gap of Object.keys(KNOWN_GAPS)) if (!names.has(gap)) fail.push(`known gap ${gap} is not a fixture any more: remove it from known-gaps.mjs`);
  }
  for (const d of files) {
    // each problem has a kind: 'error' and 'missing' and 'font' are never excused, 'box' and 'tailwind' are the geometry ones
    const problems = [];
    for (const e of d.errors) problems.push({ kind: 'error', text: `export error ${e}` });
    for (const [key, r] of Object.entries(d.runs)) {
      if (r.missing) problems.push({ kind: 'missing', text: `${key}: ${r.missing} node(s) missing from the page` });
      if (!r.fontOk && d.textNodes > 0) problems.push({ kind: 'font', text: `${key}: the font did not load` });
      if (r.max > LIMITS.geometry) problems.push({ kind: 'box', text: `${key}: box off by ${r.max.toFixed(3)} px at ${r.worst} (limit ${LIMITS.geometry})`, nodes: r.bad ?? [] });
    }
    if (d.twVsCss && d.twVsCss.max > LIMITS.tailwind) problems.push({ kind: 'tailwind', text: `tailwind differs from css by ${d.twVsCss.max.toFixed(4)} px at ${d.twVsCss.worst}` });
    if (d.fuzz && d.fillUnderHug) problems.push({ kind: 'g1', text: 'the export reports fill_under_hug on a fuzz file that rule G1 was applied to (fuzz.mjs normaliseG1 missed a case)' });
    const gap = KNOWN_GAPS[d.name];
    if (gap) {
      const why = [];
      if (!problems.length) why.push('closed');
      for (const p of problems) {
        if (p.kind !== 'box') why.push(`fails another way: ${p.text}`);
        else for (const n of p.nodes) if (!gap.nodes.includes(n)) why.push(`${p.text.split(':')[0]}: node ${n} is off and is not part of the stated cause`);
      }
      if (!d.warnings.some((w) => w.startsWith(gap.warning))) why.push(`the export no longer reports ${gap.warning}`);
      if (why.includes('closed')) fail.push(`${d.name}: known gap closed (${gap.reason}); remove it from known-gaps.mjs and update docs/fidelity-contract.md`);
      else for (const w of [...new Set(why)]) fail.push(`${d.name}: known gap no longer as stated: ${w}`);
      continue;
    }
    // rule G3: the contract promises no box there, so box deltas are not gated; an export error, a missing node and a font are
    const gated = d.fuzz && d.unspecified ? problems.filter((p) => p.kind !== 'box') : problems;
    for (const p of gated) fail.push(`${d.name}: ${p.text}${d.nodesJson ? `\n  file (add it as a fixture): ${JSON.stringify(d.nodesJson)}` : ''}`);
  }
  return fail;
}

/** The numbers of a run. */
export function report(results, failures, log) {
  const docs = results.geometry?.files ?? [];
  const max = (list, f) => list.reduce((m, d) => Math.max(m, ...Object.values(d.runs).map(f)), 0);
  const fixtures = docs.filter((d) => !d.fuzz), fuzz = docs.filter((d) => d.fuzz);
  const gated = fuzz.filter((d) => !d.unspecified);
  log(`fixtures ${fixtures.length} (${fixtures.reduce((n, d) => n + d.nodes, 0)} nodes), known gaps ${fixtures.filter((d) => KNOWN_GAPS[d.name]).length}`);
  log(`  max box delta outside the known gaps: ${max(fixtures.filter((d) => !KNOWN_GAPS[d.name]), (r) => r.max).toFixed(4)} px`);
  log(`fuzz ${fuzz.length} seeds (${fuzz.reduce((n, d) => n + d.nodes, 0)} nodes), gated ${gated.length}, outside the contract (rule G3) ${fuzz.length - gated.length}, of which the export did not warn ${fuzz.filter((d) => d.silentG3).length}`);
  log(`  max box delta in the gated seeds: ${max(gated, (r) => r.max).toFixed(4)} px`);
  log(`  max tailwind vs css delta: ${Math.max(0, ...docs.filter((d) => !KNOWN_GAPS[d.name] && d.twVsCss).map((d) => d.twVsCss.max)).toFixed(4)} px`);
  log(failures.length ? `FAIL\n${failures.join('\n')}` : 'fidelity gate: PASS');
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  const file = process.argv[2] ?? path.join(OUT, 'results.json');
  const results = JSON.parse(fs.readFileSync(file, 'utf8'));
  const failures = evaluate(results);
  report(results, failures, console.log);
  process.exit(failures.length ? 1 : 0);
}
