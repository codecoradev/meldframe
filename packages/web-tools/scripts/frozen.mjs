// Usage: node ../../packages/web-tools/scripts/frozen.mjs [--base <git-ref>] [--init] [--root <dir>]  (npm run frozen in apps/web).
// The freeze check of the legacy web clients (what is counted and the rules: frozen-lib.mjs, the allow-list: frozen-allow.json).
// Prints each violation as `path: [rule] message` and exits non-zero on any.
// With --base <ref> an allow-list entry that is new or higher than on that ref also fails, and so does a new file under
// apps/web/src/react-pilot/ (use origin/develop in CI; the ref must resolve). Without it only the working tree is checked.
// --init writes the allow-list from the tree as it is now: use it to lower entries after a file shrank or went away;
// growth is stopped by --base, so a bigger entry never gets through review's CI.
// --root <dir> checks another tree than this repository (the tests of this check use it).
import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  ALLOW_PATH,
  SKIP_DIRS,
  buildAllow,
  checkFrozen,
  compareToBase,
  countLines,
  describeViolation,
  isFrozenFile,
  newPilotFiles,
  renderAllow,
} from './frozen-lib.mjs';

const COMMENT =
  'The legacy web clients (the Svelte page and the React copies of its panels) are frozen: view files (.svelte, .tsx and the files of react and react-pilot directories, see frozen-lib.mjs) and their line counts, by package. An entry must equal the current number, so entries only go down: lower an entry when a file shrinks, delete it when a file goes away. Do not add entries or raise one to make a check pass: new web UI is written in Rust.';

const fail = (message) => {
  console.error(`frozen: ${message}`);
  process.exit(2);
};

const args = process.argv.slice(2);
let base = null;
let init = false;
let root = fileURLToPath(new URL('../../..', import.meta.url));
for (let i = 0; i < args.length; i++) {
  const a = args[i];
  const value = (flag) => (a.startsWith(`${flag}=`) ? a.slice(flag.length + 1) : args[++i]);
  if (a === '--init') init = true;
  else if (a === '--base' || a.startsWith('--base=')) base = value('--base');
  else if (a === '--root' || a.startsWith('--root=')) root = value('--root');
  else fail(`unknown argument ${a} (the options are --base <ref>, --init and --root <dir>)`);
  if ((a === '--base' || a === '--root') && !args[i]) fail(`${a} needs a value`);
}
if (base === '') fail('--base needs a git ref');

function* walk(dir) {
  for (const e of readdirSync(join(root, dir), { withFileTypes: true })) {
    const path = `${dir}/${e.name}`;
    if (e.isDirectory()) {
      if (!SKIP_DIRS.has(e.name)) yield* walk(path);
    } else if (isFrozenFile(path)) yield path;
  }
}

const dirs = [];
if (existsSync(join(root, 'apps/web'))) dirs.push('apps/web');
if (existsSync(join(root, 'packages'))) dirs.push(...readdirSync(join(root, 'packages')).sort().map((d) => `packages/${d}`));
const files = dirs
  .filter((d) => existsSync(join(root, d)))
  .flatMap((d) => [...walk(d)])
  .map((path) => ({ path, lines: countLines(readFileSync(join(root, path), 'utf8')) }));

if (init) {
  mkdirSync(dirname(join(root, ALLOW_PATH)), { recursive: true });
  writeFileSync(join(root, ALLOW_PATH), renderAllow(buildAllow(files), COMMENT));
  console.log(`frozen: wrote ${ALLOW_PATH}, ${files.length} files`);
  process.exit(0);
}

if (!existsSync(join(root, ALLOW_PATH))) fail(`${ALLOW_PATH} does not exist; create it with --init`);
const readAllow = (text) => JSON.parse(text).packages;
const allow = readAllow(readFileSync(join(root, ALLOW_PATH), 'utf8'));
const violations = checkFrozen(files, allow);

if (base) {
  const git = (...a) => execFileSync('git', a, { cwd: root, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
  try {
    git('rev-parse', '--verify', `${base}^{commit}`);
  } catch {
    fail(`${base} is not a git ref that resolves`);
  }
  let before = null;
  try {
    before = git('show', `${base}:${ALLOW_PATH}`);
  } catch {
    console.log(`frozen: ${base} has no allow-list yet, the comparison of its entries is skipped`);
  }
  if (before) violations.push(...compareToBase(allow, readAllow(before)));
  // Any file under the pilot directory, a test too, that the base does not have (a new counted file is reported already).
  const listed = (ref) => git('ls-tree', '-r', '--name-only', ref, '--', 'apps/web/src/react-pilot').split('\n').filter(Boolean);
  const onDisk = existsSync(join(root, 'apps/web/src/react-pilot')) ? [...walkAll('apps/web/src/react-pilot')] : [];
  const reported = new Set(violations.filter((x) => x.rule === 'react-pilot').map((x) => x.path));
  violations.push(...newPilotFiles(onDisk, listed(base)).filter((x) => !reported.has(x.path)));
}

function* walkAll(dir) {
  for (const e of readdirSync(join(root, dir), { withFileTypes: true })) {
    if (e.isDirectory()) yield* walkAll(`${dir}/${e.name}`);
    else yield `${dir}/${e.name}`;
  }
}

for (const x of violations) console.error(describeViolation(x));
if (violations.length) process.exit(1);
const total = allow.reduce((n, p) => n + p.lines, 0);
console.log(`frozen (web): ok, ${allow.length} packages, ${files.length} files, ${total} lines`);
