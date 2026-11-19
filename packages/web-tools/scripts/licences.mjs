// Usage: node ../../packages/web-tools/scripts/licences.mjs  (from apps/web). Lists the npm licences and fails on one outside the allowed set.
import { readFileSync } from 'node:fs';
import { checkLock } from './licences-lib.mjs';

const lock = JSON.parse(readFileSync('package-lock.json', 'utf8'));
const counts = {};
for (const [path, p] of Object.entries(lock.packages)) if (path) counts[p.license ?? 'none'] = (counts[p.license ?? 'none'] ?? 0) + 1;
console.log('npm licences:', JSON.stringify(counts));
const bad = checkLock(lock);
for (const v of bad) console.error(`${v.name}: licence ${v.license} is not in the allowed set`);
if (bad.length) process.exit(1);
console.log('licences (web): ok');
