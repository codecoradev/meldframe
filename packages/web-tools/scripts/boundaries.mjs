// Usage: node ../../packages/web-tools/scripts/boundaries.mjs [--base <git-ref>]  (npm run boundaries in apps/web).
// The boundary check of the web packages (rules and kinds: boundaries-lib.mjs, the allow-list: boundaries-allow.json).
// Every package under packages/ and apps/web declares `"meldframe": { "kind": ..., "group": ..., "composite": [...] }`
// in its package.json. Prints each violation as `path:line: [rule] message` and exits non-zero on any.
// With --base <ref> an allow-list entry that is new or higher than on that ref also fails (use origin/develop in CI).
import { execFileSync } from 'node:child_process';
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { applyAllowList, checkBoundaries, compareToBase } from './boundaries-lib.mjs';

const root = fileURLToPath(new URL('../../..', import.meta.url));
const ALLOW = 'packages/web-tools/boundaries-allow.json';
const SKIP = new Set(['node_modules', 'dist', '.wasm', 'target', 'rust']);

function loadPackages() {
  const dirs = [...readdirSync(join(root, 'packages')).sort().map((d) => `packages/${d}`), 'apps/web'];
  return dirs
    .filter((dir) => existsSync(join(root, dir, 'package.json')))
    .map((dir) => {
      const json = JSON.parse(readFileSync(join(root, dir, 'package.json'), 'utf8'));
      const { kind, group, composite } = json.meldframe ?? {};
      return { id: json.name.replace(/^@meldframe\//, ''), name: json.name, dir, kind, group, composite };
    });
}

function* walk(dir) {
  for (const e of readdirSync(join(root, dir), { withFileTypes: true })) {
    const path = `${dir}/${e.name}`;
    if (e.isDirectory()) {
      if (!SKIP.has(e.name)) yield* walk(path);
    } else if (/\.(?:ts|tsx|mts|cts|js|mjs|cjs|svelte)$/.test(e.name)) yield path;
  }
}

const readAllow = (text) => JSON.parse(text).entries;

const args = process.argv.slice(2);
const baseAt = args.findIndex((a) => a === '--base' || a.startsWith('--base='));
const unknown = args.filter((a, i) => !(a === '--base' || a.startsWith('--base=') || (i > 0 && args[i - 1] === '--base')));
if (unknown.length) {
  console.error(`boundaries: unknown argument ${unknown.join(' ')} (the only option is --base <ref>)`);
  process.exit(2);
}
const base = baseAt < 0 ? null : args[baseAt].includes('=') ? args[baseAt].slice(7) : args[baseAt + 1];
if (baseAt >= 0 && !base) {
  console.error('boundaries: --base needs a git ref');
  process.exit(2);
}

const packages = loadPackages();
const files = packages.flatMap((p) => [...walk(p.dir)]).map((path) => ({ path, text: readFileSync(join(root, path), 'utf8') }));
const allow = readAllow(readFileSync(join(root, ALLOW), 'utf8'));
const errors = applyAllowList(checkBoundaries({ packages, files }), allow);
if (base) {
  const git = (...a) => execFileSync('git', a, { cwd: root, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
  try {
    git('rev-parse', '--verify', `${base}^{commit}`);
  } catch {
    console.error(`boundaries: ${base} is not a git ref that resolves`);
    process.exit(2);
  }
  let before = null;
  try {
    before = git('show', `${base}:${ALLOW}`);
  } catch {
    console.log(`boundaries: ${base} has no allow-list yet, the comparison with it is skipped`);
  }
  if (before) errors.push(...compareToBase(allow, readAllow(before)));
}

for (const e of errors) console.error(e);
if (errors.length) process.exit(1);
console.log(`boundaries (web): ok, ${packages.length} packages, ${files.length} files, ${allow.reduce((n, e) => n + e.count, 0)} allowed`);
