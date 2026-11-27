// The freeze check for the legacy web clients (the Svelte page and the React copies of its panels): pure logic over
// file paths and line counts; frozen.mjs reads the tree and git and prints. The two clients stay in the repository and
// stay the default page until the native front end replaces them, so they must not grow: bug and security fixes are
// possible inside the allow-list, new web UI is written in Rust.
//
// What is counted (a path from the repository root), in `apps/web` and in every directory of `packages/`:
//   - every `.svelte` and `.tsx` file;
//   - every `.ts`, `.mts`, `.js`, `.mjs` file inside a directory named `react` or `react-pilot` (the React islands and
//     their glue: `pilot.ts`, `surface.ts`, ...).
// What is not counted: a test (`*.test.*`), a test helper (`*.testutil.*`), anything under a `scripts/` directory (the
// browser checks and the checks of the repository), and `node_modules`, `dist`, `.wasm`, `target`, `rust`.
// A frozen package is a directory that holds at least one counted file: the list comes from the files, not from a table.
//
// Rules, each named in the output as `path: [rule] message`:
//   new-package   a counted file in a directory that has no entry in the allow-list
//   new-file      a counted file that has no entry in the allow-list
//   react-pilot   a new file under apps/web/src/react-pilot/ (any file, a test too, when --base is given)
//   grew          a counted file has more lines than its entry
//   stale         a counted file has fewer lines than its entry: lower the entry (`--init` rewrites the list)
//   gone          an entry for a file that is not counted any more: delete the entry
//   allow-list    the allow-list is malformed (a bad count, a total that is not the sum of its files)
//   base          an entry that is new or higher than on the base ref (the allow-list only goes down)

export const ALLOW_PATH = 'packages/web-tools/frozen-allow.json';
export const PILOT_DIR = 'apps/web/src/react-pilot';
export const SKIP_DIRS = new Set(['node_modules', 'dist', '.wasm', 'target', 'rust']);

const SOURCE = /\.(?:[cm]?[jt]s|tsx)$/;

/** True for a test, a test helper and anything under a `scripts/` directory: not view code, not counted. */
export const isTestOrScript = (path) => /\.(?:test|testutil)\.[^/]*$/.test(path) || /(?:^|\/)scripts\//.test(path);

/** True when `path` (from the repository root) is a counted file of a frozen client. */
export function isFrozenFile(path) {
  if (isTestOrScript(path)) return false;
  if (path.endsWith('.svelte') || path.endsWith('.tsx')) return true;
  return SOURCE.test(path) && path.split('/').slice(0, -1).some((d) => d === 'react' || d === 'react-pilot');
}

/** The package directory that holds `path`: `apps/web` or `packages/<name>`, or null for any other path. */
export function packageDirOf(path) {
  const parts = path.split('/');
  if (parts[0] === 'apps' && parts[1] === 'web') return 'apps/web';
  if (parts[0] === 'packages' && parts.length > 2) return `packages/${parts[1]}`;
  return null;
}

/** The number of lines of `text`, as `cargo xtask size-check` counts them (a last line without a newline counts). */
export function countLines(text) {
  if (!text) return 0;
  return text.split('\n').length - (text.endsWith('\n') ? 1 : 0);
}

/** The allow-list entries (`{ dir, lines, files }`, sorted) for `files` (`{ path, lines }`, counted files only). */
export function buildAllow(files) {
  const byDir = new Map();
  for (const { path, lines } of files) {
    const dir = packageDirOf(path);
    if (!dir) continue;
    if (!byDir.has(dir)) byDir.set(dir, {});
    byDir.get(dir)[path.slice(dir.length + 1)] = lines;
  }
  return [...byDir.keys()].sort().map((dir) => {
    const map = byDir.get(dir);
    const files = Object.fromEntries(Object.keys(map).sort().map((k) => [k, map[k]]));
    return { dir, lines: Object.values(files).reduce((a, b) => a + b, 0), files };
  });
}

/** The text of the allow-list file for `packages` (one line for each file, so a diff shows one file for one line). */
export function renderAllow(packages, comment) {
  const body = packages
    .map((p) => {
      const rows = Object.entries(p.files).map(([f, n]) => `        ${JSON.stringify(f)}: ${n}`);
      return `    { "dir": ${JSON.stringify(p.dir)}, "lines": ${p.lines}, "files": {\n${rows.join(',\n')}\n    } }`;
    })
    .join(',\n');
  return `{\n  "comment": ${JSON.stringify(comment)},\n  "packages": [\n${body}\n  ]\n}\n`;
}

const flat = (packages) =>
  new Map(packages.flatMap((p) => Object.entries(p.files).map(([f, n]) => [`${p.dir}/${f}`, { dir: p.dir, lines: n }])));

const v = (rule, path, message) => ({ rule, path, message });
export const describeViolation = (x) => `${x.path}: [${x.rule}] ${x.message}`;

/** The problems of the allow-list itself: a count that is not a whole number of 1 or more, a total that is not the sum. */
export function checkAllowShape(packages) {
  const out = [];
  const dirs = new Set();
  for (const p of packages) {
    if (dirs.has(p.dir)) out.push(v('allow-list', p.dir, 'is listed twice'));
    dirs.add(p.dir);
    let sum = 0;
    for (const [f, n] of Object.entries(p.files ?? {})) {
      if (!Number.isInteger(n) || n < 1) out.push(v('allow-list', `${p.dir}/${f}`, `needs a count of 1 or more, not ${n}`));
      else sum += n;
    }
    if (p.lines !== sum) out.push(v('allow-list', p.dir, `says ${p.lines} lines but its files add up to ${sum}`));
  }
  return out;
}

/**
 * The violations of the freeze for `files` (`{ path, lines }`, the counted files of the tree) against the allow-list
 * `packages`. A count must equal its entry: a file that grew or is new fails, a file that shrank makes its entry stale.
 */
export function checkFrozen(files, packages) {
  const out = checkAllowShape(packages);
  const allowed = flat(packages);
  const listed = new Set(packages.map((p) => p.dir));
  const seen = new Set();
  for (const { path, lines } of [...files].sort((a, b) => a.path.localeCompare(b.path))) {
    seen.add(path);
    const entry = allowed.get(path);
    const dir = packageDirOf(path);
    if (!entry) {
      if (path.startsWith(`${PILOT_DIR}/`)) out.push(v('react-pilot', path, `a new file under ${PILOT_DIR}/: the React pilot is frozen, new web UI is written in Rust`));
      else if (!listed.has(dir)) out.push(v('new-package', path, `${dir} is not a frozen package yet and gets no new view code (${lines} lines): new web UI is written in Rust`));
      else out.push(v('new-file', path, `a new view file (${lines} lines); the web clients are frozen, new web UI is written in Rust`));
    } else if (lines > entry.lines) {
      out.push(v('grew', path, `has ${lines} lines, the allow-list has ${entry.lines}; a frozen file does not grow`));
    } else if (lines < entry.lines) {
      out.push(v('stale', path, `has ${lines} lines, the allow-list has ${entry.lines}; lower the entry to ${lines} (node packages/web-tools/scripts/frozen.mjs --init)`));
    }
  }
  for (const [path] of allowed) {
    if (!seen.has(path)) out.push(v('gone', path, 'is in the allow-list but is not a counted file any more; delete the entry'));
  }
  return out;
}

/** The violations for entries that are new or higher than in `base` (the allow-list of the base ref), files and totals. */
export function compareToBase(packages, base) {
  const out = [];
  const before = flat(base);
  const totals = new Map(base.map((p) => [p.dir, p.lines]));
  for (const [path, { lines }] of flat(packages)) {
    const was = before.get(path)?.lines;
    if (was === undefined) out.push(v('base', path, 'is a new entry; the allow-list only goes down, so a new view file is not allowed'));
    else if (lines > was) out.push(v('base', path, `is ${lines} in the allow-list, higher than on the base (${was})`));
  }
  for (const p of packages) {
    const was = totals.get(p.dir);
    if (was === undefined) out.push(v('base', p.dir, 'is a new package in the allow-list; the allow-list only goes down'));
    else if (p.lines > was) out.push(v('base', p.dir, `is ${p.lines} lines in the allow-list, higher than on the base (${was})`));
  }
  return out;
}

/** A file under the pilot directory (a test or a helper too) that is not on the base ref: `paths` and `basePaths` as git lists them. */
export function newPilotFiles(paths, basePaths) {
  const old = new Set(basePaths);
  return paths
    .filter((p) => p.startsWith(`${PILOT_DIR}/`) && !old.has(p))
    .sort()
    .map((p) => v('react-pilot', p, `a new file under ${PILOT_DIR}/ (not on the base): the React pilot is frozen, new web UI is written in Rust`));
}
