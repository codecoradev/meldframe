// The boundary check for the web packages (the TypeScript side of `cargo xtask deps-check`; docs/standards/
// modular-thin-ui.md L1 to L4, docs/audits/2026-11-25-ts-thin-ui.md finding T18 and section 5). Pure logic over
// package data and file text; boundaries.mjs reads the tree and prints. Rules, each named in the output:
//   kind            a package declares a known kind in its package.json (`"meldframe": { "kind": ... }`)
//   group           a view declares the group it belongs to (L4)
//   composite       a composite group is a group some package declares
//   matrix          a package imports only the kinds that its kind may import (KINDS)
//   group           a view imports a view of another group only when its `composite` names that group
//   framework       a framework is imported only in the kinds that FRAMEWORKS lists for it (not in test files)
//   framework-file  in a kit or a view only a .svelte file imports a framework, not a plain .ts file
//   file-type       a .svelte file sits only in a package of a kind that holds views
//   cycle           the graph of packages has no cycle
// What exists today and is not yet fixed is named in boundaries-allow.json (entries only go down).

import { stripComments } from './ui-literals-lib.mjs';

const ANY = '*';

/**
 * The kinds of web package and what each may import (the audit's section 5, with `tool` and `bin` free):
 *   proto   generated wire types; imports nothing
 *   model   pure TypeScript: no framework, rules and data
 *   host    browser adapters (the engine link, transport, assets, input)
 *   kit     framework primitives with no feature knowledge
 *   view    feature screens and panels, one group each
 *   render  the canvas drawing module (none yet: the Pixi code is in the app)
 *   bin     the composition root (apps/web)
 *   tool    scripts, checks and browser tests
 */
export const KINDS = {
  proto: { imports: [] },
  model: { imports: ['proto', 'model'] },
  host: { imports: ['proto', 'model', 'host'] },
  kit: { imports: ['kit'] },
  view: { imports: ['proto', 'model', 'kit', 'view'] },
  render: { imports: ['proto', 'model', 'host', 'render'] },
  bin: { imports: [ANY] },
  tool: { imports: [ANY] },
};

/**
 * Frameworks and libraries that stay in some kinds. `imports` are module names (the name and its subpaths);
 * `kinds` where a non-test file may import it; `viewFiles` the extensions of its view files (a file with
 * one of them may sit only in a package of those kinds); `plainFileKinds` the kinds where a plain .ts file may also
 * import it (the composition root mounts the app). With no `viewFiles` any file of those kinds may import it.
 * To add a framework, add a row (React: `{ name: 'react', imports: ['react', 'react-dom'], kinds: ['kit', 'view',
 * 'bin'], viewFiles: ['.tsx'], plainFileKinds: ['bin'] }`).
 */
export const FRAMEWORKS = [
  { name: 'svelte', imports: ['svelte'], kinds: ['kit', 'view', 'bin'], viewFiles: ['.svelte'], plainFileKinds: ['bin'] },
  { name: 'bits-ui', imports: ['bits-ui'], kinds: ['kit', 'view', 'bin'], viewFiles: ['.svelte'], plainFileKinds: ['bin'] },
  { name: 'pixi.js', imports: ['pixi.js'], kinds: ['render', 'bin'] },
];

/** True for a test file and for a script, which may import test tooling and are checked only for the matrix. */
export const isTestFile = (path) => /\.test\.[cm]?[jt]sx?$/.test(path) || /(?:^|\/)scripts\//.test(path);

const blank = (s) => s.replace(/[^\n]/g, ' ');

/** The text of a file that holds code: a Svelte file keeps only its script blocks (markup blanked, lines kept). */
function codeOf(path, text) {
  if (!path.endsWith('.svelte')) return stripComments(text, path);
  let out = '';
  let last = 0;
  for (const m of text.matchAll(/(<script\b[^>]*>)([\s\S]*?)(<\/script>)/g)) {
    out += blank(text.slice(last, m.index + m[1].length)) + m[2];
    last = m.index + m[1].length + m[2].length;
  }
  return stripComments(out + blank(text.slice(last)), 'a.ts');
}

const STATEMENT = /^[ \t]*(?:import|export)\b(?:[^'"`;]*?\bfrom\b)?\s*(['"])([^'"\n]+)\1/gm;
const DYNAMIC = /\bimport\(\s*(['"])([^'"\n]+)\1\s*\)/g;

/**
 * The module specifiers a file imports, as `{ line, specifier }` in file order: `import`, `import type`, a side-effect
 * `import 'x'`, `export ... from` and a dynamic `import('x')`. Comments are skipped; a statement must start its line.
 * Not seen: `require`, `new URL(..., import.meta.url)` and `vi.mock`.
 */
export function parseImports(path, text) {
  const code = codeOf(path, text);
  const found = [];
  for (const re of [STATEMENT, DYNAMIC]) {
    for (const m of code.matchAll(re)) found.push({ at: m.index, specifier: m[2] });
  }
  return found
    .sort((a, b) => a.at - b.at)
    .map(({ at, specifier }) => ({ line: code.slice(0, at).split('\n').length, specifier }));
}

/** The package that holds `path` (its dir is a prefix), or undefined. */
const packageOf = (path, packages) => packages.find((p) => path.startsWith(`${p.dir}/`));

const normalise = (parts) => {
  const out = [];
  for (const part of parts) {
    if (part === '..') out.pop();
    else if (part !== '.' && part !== '') out.push(part);
  }
  return out.join('/');
};

/**
 * Where an import from `from` (a path from the repository root) goes: `{ to }` another package (by its alias
 * `@meldframe/<id>` or by a relative path that leaves the package), `{ external }` a module of node_modules or the
 * platform, or null for a path inside the same package.
 */
export function resolveImport(from, specifier, packages) {
  const here = packageOf(from, packages);
  if (specifier.startsWith('.')) {
    const target = normalise([...from.split('/').slice(0, -1), ...specifier.split('/')]);
    const there = packageOf(`${target}/`, packages);
    return there && there !== here ? { to: there.id } : null;
  }
  const root = specifier.split('/').slice(0, specifier.startsWith('@') ? 2 : 1).join('/');
  const alias = packages.find((p) => p.name === root);
  if (alias) return alias === here ? null : { to: alias.id };
  return { external: root };
}

const frameworkOf = (root, frameworks) => frameworks.find((f) => f.imports.includes(root));

/** Strongly connected sets of two or more nodes in a graph of `[from, to]` pairs, each sorted, in sorted order. */
export function findCycles(edges) {
  const next = new Map();
  for (const [a, b] of edges) {
    if (a === b) continue;
    if (!next.has(a)) next.set(a, new Set());
    next.get(a).add(b);
  }
  const index = new Map();
  const low = new Map();
  const stack = [];
  const onStack = new Set();
  const found = [];
  const visit = (v) => {
    index.set(v, index.size);
    low.set(v, index.get(v));
    stack.push(v);
    onStack.add(v);
    for (const w of next.get(v) ?? []) {
      if (!index.has(w)) {
        visit(w);
        low.set(v, Math.min(low.get(v), low.get(w)));
      } else if (onStack.has(w)) low.set(v, Math.min(low.get(v), index.get(w)));
    }
    if (low.get(v) !== index.get(v)) return;
    const set = [];
    for (let w = null; w !== v; ) {
      w = stack.pop();
      onStack.delete(w);
      set.push(w);
    }
    if (set.length > 1) found.push(set.sort());
  };
  for (const v of [...next.keys()].sort()) if (!index.has(v)) visit(v);
  return found.sort((a, b) => a.join().localeCompare(b.join()));
}

/**
 * The violations of the rules in `packages` (`{ id, name, dir, kind, group?, composite? }`, `dir` from the repository
 * root) and `files` (`{ path, text }`). Each is `{ rule, from, to, path, line, message }`; `from` and `to` name the
 * packages, the framework or the file type that the allow-list keys on.
 */
export function checkBoundaries({ packages, files, kinds = KINDS, frameworks = FRAMEWORKS }) {
  const out = [];
  const add = (rule, from, to, path, line, message) => out.push({ rule, from, to, path, line, message });
  const manifest = (p) => `${p.dir}/package.json`;
  const groups = new Set(packages.map((p) => p.group).filter(Boolean));
  const known = (p) => Object.hasOwn(kinds, p.kind);

  for (const p of packages) {
    if (!p.kind) add('kind', p.id, '', manifest(p), 1, `${p.id} declares no kind: add "meldframe": { "kind": ... } (one of ${Object.keys(kinds).join(', ')})`);
    else if (!known(p)) add('kind', p.id, '', manifest(p), 1, `${p.id} declares the kind "${p.kind}", which is not one of ${Object.keys(kinds).join(', ')}`);
    else if (p.kind === 'view' && !p.group) add('group', p.id, '', manifest(p), 1, `${p.id} is a view and declares no group`);
    for (const g of p.composite ?? []) {
      if (!groups.has(g)) add('composite', p.id, g, manifest(p), 1, `${p.id} lists the composite group "${g}", which no package declares`);
    }
  }

  const edges = [];
  for (const { path, text } of files) {
    const from = packageOf(path, packages);
    if (!from) continue;
    const test = isTestFile(path);
    for (const ext of new Set(frameworks.flatMap((f) => f.viewFiles ?? []))) {
      const holders = frameworks.filter((f) => f.viewFiles?.includes(ext)).flatMap((f) => f.kinds);
      if (path.endsWith(ext) && known(from) && !holders.includes(from.kind)) {
        add('file-type', from.id, ext, path, 1, `${path} is a ${ext} file in a package of kind ${from.kind}, which holds no views`);
      }
    }
    for (const { line, specifier } of parseImports(path, text)) {
      const target = resolveImport(path, specifier, packages);
      if (!target) continue;
      if (target.to) {
        const to = packages.find((p) => p.id === target.to);
        edges.push([from.id, to.id]);
        if (!known(from) || !known(to)) continue;
        const allowed = kinds[from.kind].imports;
        if (!allowed.includes(ANY) && !allowed.includes(to.kind)) {
          add('matrix', from.id, to.id, path, line, `${from.id} (${from.kind}) may not import ${to.id} (${to.kind}): ${specifier}`);
        } else if (from.kind === 'view' && to.kind === 'view' && from.group !== to.group && !(from.composite ?? []).includes(to.group)) {
          add('group', from.id, to.id, path, line, `${from.id} (group ${from.group}) imports ${to.id} (group ${to.group}); name the group in "composite" or share the code through a model or kit package: ${specifier}`);
        }
        continue;
      }
      const framework = frameworkOf(target.external, frameworks);
      if (!framework || test || !known(from)) continue;
      const isViewFile = (framework.viewFiles ?? []).some((ext) => path.endsWith(ext));
      if (!framework.kinds.includes(from.kind)) {
        add('framework', from.id, framework.name, path, line, `${from.id} (${from.kind}) may not import ${framework.name}; it stays in ${framework.kinds.join(', ')}: ${specifier}`);
      } else if (framework.viewFiles && !isViewFile && !(framework.plainFileKinds ?? []).includes(from.kind)) {
        add('framework-file', from.id, framework.name, path, line, `${path} is not a view file (${framework.viewFiles.join(', ')}) and imports ${framework.name}: ${specifier}`);
      }
    }
  }
  for (const cycle of findCycles(edges)) {
    add('cycle', cycle.join(' > '), '', manifest(packages.find((p) => p.id === cycle[0])), 1, `the packages ${cycle.join(', ')} import each other in a cycle`);
  }
  return out;
}

const keyOf = (e) => `${e.rule} ${e.from} -> ${e.to}`;
const describeViolation = (v) => `${v.path}:${v.line}: [${v.rule}] ${v.message}`;

/**
 * The errors left when the allow-list (`{ rule, from, to, count, reason }`) is applied to `violations`, as lines of
 * text. An entry must equal the number of violations it covers, so entries only go down: one that is higher says
 * what to lower it to, one that covers nothing is not needed, and a violation with no entry (or past its count) is
 * reported with its path and line.
 */
export function applyAllowList(violations, allow) {
  const errors = [];
  const seen = new Set();
  const entries = new Map();
  for (const e of allow) {
    const key = keyOf(e);
    if (seen.has(key)) errors.push(`allow-list: ${key} is listed twice`);
    seen.add(key);
    if (!Number.isInteger(e.count) || e.count < 1) errors.push(`allow-list: ${key} needs a count of 1 or more`);
    if (typeof e.reason !== 'string' || !e.reason.trim()) errors.push(`allow-list: ${key} needs a reason`);
    entries.set(key, e);
  }
  const groups = new Map();
  for (const v of violations) groups.set(keyOf(v), [...(groups.get(keyOf(v)) ?? []), v]);
  for (const [key, list] of groups) {
    const allowed = entries.get(key)?.count ?? 0;
    if (list.length === allowed) continue;
    if (list.length < allowed) errors.push(`allow-list: ${key} allows ${allowed} but there are ${list.length}; lower it to ${list.length}`);
    else {
      if (allowed) errors.push(`${key} has ${list.length} now, the allow-list has ${allowed}`);
      errors.push(...list.map(describeViolation));
    }
  }
  for (const [key] of entries) if (!groups.has(key)) errors.push(`allow-list: ${key} is no longer needed; delete the entry`);
  return errors;
}

/** The errors for entries that are new or higher than in `base` (the allow-list of the base branch). */
export function compareToBase(allow, base) {
  const before = new Map(base.map((e) => [keyOf(e), e.count]));
  return allow.flatMap((e) => {
    const was = before.get(keyOf(e));
    if (was === undefined) return [`allow-list: ${keyOf(e)} is new; fix the import instead of adding an entry`];
    return e.count > was ? [`allow-list: ${keyOf(e)} is ${e.count}, higher than on the base (${was})`] : [];
  });
}
