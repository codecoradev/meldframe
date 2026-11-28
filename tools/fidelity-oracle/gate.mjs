// The gate: reads out/results.json (or the file given) and exits 1 on a contract violation or on a known gap that closed.
import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { IMPORT_GAPS, IMPORT_LOSSES, KNOWN_GAPS } from './known-gaps.mjs';
import { pageFailures, roundTripFailures } from './import-gate.mjs';
import { OUT } from './lib.mjs';

export const LIMITS = { geometry: 0.05, tailwind: 0.001, ink: 1, blur: 20 };
/** A run that checks less than this is not a run of the oracle: the hand-written fixtures, and the share of the seeds the gate must hold. */
export const FLOORS = { fixtures: 22, gatedShare: 0.3 };
/** The fixtures the paint oracle must probe (probes.mjs): a run without a probe in each form for one, or without the pixel comparison of its two
 *  forms, is not a run of the oracle. The fills of the contract: solid and image (f07, f08), opacity (f18), gradients (f19), a fill
 *  bound to a variable in the default mode and in a picked one (f20, f21), no fill (f22) and the alpha of a fill's colour (f23). */
export const PAINT_FIXTURES = ['f07-decor', 'f08-image', 'f18-opacity', 'f19-gradient', 'f20-variable-fill', 'f21-variable-modes', 'f22-no-fill', 'f23-fill-alpha'];

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
    for (const name of Object.keys(IMPORT_LOSSES)) if (!names.has(name)) fail.push(`known import loss ${name} is not a fixture any more: remove it from known-gaps.mjs`);
  }
  if (!results.only && !results.geometryOnly) {
    // a missing or empty section is a run that checked nothing
    if (!results.paint?.probes?.length) fail.push('the paint oracle ran no probes');
    if (!results.paint?.forms?.length) fail.push('the paint oracle compared no pages of the two forms');
    for (const name of PAINT_FIXTURES) {
      for (const form of ['css', 'tailwind']) {
        if (!(results.paint?.probes ?? []).some((p) => p.id.startsWith(`${name} ${form}: `))) fail.push(`no ${form} paint probe for ${name}`);
      }
      if (!(results.paint?.forms ?? []).some((f) => f.fixture === name)) fail.push(`the paint oracle did not compare the two forms of ${name}`);
    }
    if (!results.ink?.rows?.length) fail.push('the text ink oracle ran no case');
    if (!results.blur?.length) fail.push('the blur check ran no case');
  }
  for (const p of results.paint?.probes ?? []) if (!p.ok) fail.push(`paint probe failed: ${p.id} at ${p.at}, wanted ${p.want}, got rgb(${p.got})`);
  for (const f of results.paint?.forms ?? []) if (!(f.differing === 0)) fail.push(`${f.fixture}: the Tailwind page and the CSS page differ in ${f.differing} pixels`);
  for (const r of results.ink?.rows ?? []) {
    if (r.noInk) fail.push(`text ink ${r.id} (${r.form}): no ink found on ${r.noInk}, so nothing was compared`);
    const worst = Math.max(...['dLeft', 'dRight', 'dTop', 'dBottom'].map((k) => Math.abs(r[k])));
    if (worst > LIMITS.ink) fail.push(`text ink ${r.id} (${r.form}): an edge is ${worst} px from the canvas text (limit ${LIMITS.ink})`);
  }
  for (const b of results.blur ?? []) if (b.maxAbs > LIMITS.blur) fail.push(`shadow blur ${b.blur}px: the CSS falloff differs from the Pixi blur by ${b.maxAbs}/255 (limit ${LIMITS.blur})`);
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
    fail.push(...roundTripFailures(d));
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
  fail.push(...pageFailures(results));
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
  // the numbers of the files the gate judges: not the known gaps, and no page of a fuzz file under rule G3
  const judged = docs.filter((d) => !KNOWN_GAPS[d.name]);
  const trips = judged.flatMap((d) => Object.values(d.roundTrip ?? {}).map((r) => ({ r, page: d.fuzz && d.unspecified ? null : r.page })));
  if (trips.length) log(`import round trip: ${trips.length} exports imported back, largest box delta ${Math.max(0, ...trips.map((t) => t.r.max ?? 0)).toFixed(4)} px, largest delta on the page of the imported file ${Math.max(0, ...trips.map((t) => t.page?.max ?? 0)).toFixed(4)} px`);
  if (results.import) log(`hand-written pages ${results.import.files.length}, known import gaps ${results.import.files.filter((f) => IMPORT_GAPS[f.name]).length}, elements compared ${results.import.files.reduce((n, f) => n + f.nodes - f.anonymous, 0)} (anonymous runs not compared: ${results.import.files.reduce((n, f) => n + f.anonymous, 0)})`);
  if (results.paint) log(`paint probes ${results.paint.probes.filter((p) => p.ok).length}/${results.paint.probes.length}, pixels differing between the forms ${results.paint.forms.reduce((n, f) => n + f.differing, 0)}`);
  if (results.ink) log(`text ink: ${new Set(results.ink.rows.map((r) => r.id)).size} cases, largest edge difference ${results.ink.worst} px (limit ${LIMITS.ink})`);
  if (results.blur) log(`CSS blur model against Pixi (not the export): largest falloff difference ${Math.max(...results.blur.map((b) => b.maxAbs))}/255 (limit ${LIMITS.blur})`);
  log(failures.length ? `FAIL\n${failures.join('\n')}` : 'fidelity gate: PASS');
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  const file = process.argv[2] ?? path.join(OUT, 'results.json');
  const results = JSON.parse(fs.readFileSync(file, 'utf8'));
  const failures = evaluate(results);
  report(results, failures, console.log);
  process.exit(failures.length ? 1 : 0);
}
