// Runs the oracle and the gate. `node oracle.mjs [--seeds N] [--only name ...]`; exit 1 on a violation.
import fs from 'node:fs';
import path from 'node:path';
import { serve, launch, openMeasurePage, OUT } from './lib.mjs';
import { geometry } from './geometry.mjs';
import { evaluate, report } from './gate.mjs';

const args = process.argv.slice(2);
const at = args.indexOf('--seeds');
const seeds = at >= 0 ? Number(args[at + 1]) : 50;
if (!Number.isInteger(seeds) || seeds < 1) throw new Error('--seeds takes a whole number of 1 or more (the fuzz files are part of the oracle)');
const only = args.filter((a, i) => !a.startsWith('--') && args[i - 1] !== '--seeds');
const log = (m) => console.log(m);

fs.rmSync(path.join(OUT, 'pages'), { recursive: true, force: true });
const { base, close } = await serve();
const browser = await launch();
const results = { seeds, only: only.length > 0, version: await browser.version() };
try {
  const mp = await openMeasurePage(browser, base);
  results.geometry = await geometry({ browser, base, mp, seeds, only, log });
} finally {
  await browser.close();
  close();
}
fs.writeFileSync(path.join(OUT, 'results.json'), JSON.stringify(results));
const failures = evaluate(results);
report(results, failures, log);
process.exit(failures.length ? 1 : 0);
