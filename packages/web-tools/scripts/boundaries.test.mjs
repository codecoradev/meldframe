import { describe, expect, it } from 'vitest';
import {
  FRAMEWORKS,
  KINDS,
  applyAllowList,
  checkBoundaries,
  compareToBase,
  findCycles,
  parseImports,
  resolveImport,
} from './boundaries-lib.mjs';

const pkg = (id, kind, extra = {}) => ({ id, name: `@meldframe/${id}`, dir: `packages/${id}`, kind, ...extra });
const file = (id, name, text) => ({ path: `packages/${id}/src/${name}`, text });
const rules = (vs) => vs.map((v) => `${v.rule}:${v.from}->${v.to}`);

const packages = [
  pkg('protocol', 'proto'),
  pkg('web-model', 'model'),
  pkg('web-model2', 'model'),
  pkg('web-host', 'host'),
  pkg('web-kit', 'kit'),
  pkg('web-view', 'view', { group: 'one' }),
  pkg('web-view2', 'view', { group: 'two' }),
  pkg('web-tool', 'tool'),
  { id: 'web', name: '@meldframe/web', dir: 'apps/web', kind: 'bin' },
];
const check = (files, pkgs = packages, options = {}) => checkBoundaries({ packages: pkgs, files, ...options });

describe('parseImports', () => {
  it('finds static, type, side-effect, re-export and dynamic imports, also over several lines', () => {
    const text = [
      "import a from 'one';",
      "import type { B } from 'two';",
      "import 'three';",
      "export { c } from 'four';",
      "export * from 'five';",
      'import {',
      '  d,',
      '  e,',
      "} from 'six';",
      "const x = await import('seven');",
    ].join('\n');
    expect(parseImports('a.ts', text)).toEqual([
      { line: 1, specifier: 'one' },
      { line: 2, specifier: 'two' },
      { line: 3, specifier: 'three' },
      { line: 4, specifier: 'four' },
      { line: 5, specifier: 'five' },
      { line: 6, specifier: 'six' },
      { line: 10, specifier: 'seven' },
    ]);
  });
  it('ignores comments and text inside strings that is not an import statement', () => {
    const text = ["// import a from 'one';", "/* import b from 'two'; */", "const s = \"import c from 'three'\";", "import d from 'four';"].join('\n');
    expect(parseImports('a.ts', text)).toEqual([{ line: 4, specifier: 'four' }]);
  });
  it('reads only the script blocks of a Svelte file and keeps the line numbers', () => {
    const text = ['<script lang="ts">', "  import { x } from 'svelte';", '</script>', "<p>import y from 'markup'</p>", "<script module>import z from 'bits-ui';</script>"].join('\n');
    expect(parseImports('A.svelte', text)).toEqual([
      { line: 2, specifier: 'svelte' },
      { line: 5, specifier: 'bits-ui' },
    ]);
  });
});

describe('resolveImport', () => {
  it('maps the package alias, with or without a subpath, to a package', () => {
    expect(resolveImport('packages/web-view/src/a.ts', '@meldframe/web-model/camera', packages)).toEqual({ to: 'web-model' });
    expect(resolveImport('packages/web-view/src/a.ts', '@meldframe/protocol', packages)).toEqual({ to: 'protocol' });
  });
  it('maps a relative path that leaves the package to the package it lands in, and ignores one that stays', () => {
    expect(resolveImport('packages/web-view/src/a.ts', '../../web-host/src/x', packages)).toEqual({ to: 'web-host' });
    expect(resolveImport('apps/web/vite.config.ts', '../../packages/web-tool/src/x', packages)).toEqual({ to: 'web-tool' });
    expect(resolveImport('packages/web-view/src/a.ts', './b', packages)).toBeNull();
    expect(resolveImport('packages/web-view/src/a.ts', '../scripts/b.mjs', packages)).toBeNull();
  });
  it('treats anything else as an external module named by its root', () => {
    expect(resolveImport('packages/web-view/src/a.ts', 'svelte/server', packages)).toEqual({ external: 'svelte' });
    expect(resolveImport('packages/web-view/src/a.ts', '@sveltejs/kit/x', packages)).toEqual({ external: '@sveltejs/kit' });
    expect(resolveImport('packages/web-view/src/a.ts', 'node:fs', packages)).toEqual({ external: 'node:fs' });
    expect(resolveImport('packages/web-view/src/a.ts', '@meldframe/wasm', packages)).toEqual({ external: '@meldframe/wasm' });
  });
});

describe('kinds and groups', () => {
  it('fails a package with no kind, an unknown kind, or a view with no group', () => {
    const vs = check([], [pkg('a', undefined), pkg('b', 'widget'), pkg('c', 'view')]);
    expect(rules(vs)).toEqual(['kind:a->', 'kind:b->', 'group:c->']);
  });
  it('fails a composite group that no package declares', () => {
    const vs = check([], [pkg('a', 'view', { group: 'one', composite: ['nope'] })]);
    expect(rules(vs)).toEqual(['composite:a->nope']);
  });
  it('lists every kind of the audit and declares what each may import', () => {
    expect(Object.keys(KINDS).sort()).toEqual(['bin', 'host', 'kit', 'model', 'proto', 'render', 'tool', 'view']);
  });
});

describe('the matrix of imports between kinds', () => {
  const imp = (id, kind) => check([file(id, 'a.ts', `import x from '@meldframe/${kind}';`)]);
  it('lets each kind import what the matrix lists', () => {
    expect(check([file('web-model', 'a.ts', "import x from '@meldframe/protocol'; import y from '@meldframe/web-model2/b';")])).toEqual([]);
    expect(check([file('web-host', 'a.ts', "import x from '@meldframe/web-model/b'; import y from '@meldframe/protocol';")])).toEqual([]);
    expect(check([file('web-view', 'A.svelte', "<script>import x from '@meldframe/web-kit/B.svelte'; import y from '@meldframe/web-model/b';</script>")])).toEqual([]);
    expect(check([file('web', 'a.ts', "import a from '@meldframe/web-view/b'; import b from '@meldframe/web-host/b'; import c from '@meldframe/web-tool/b';")])).toEqual([]);
    expect(check([file('web-tool', 'a.mjs', "import a from '@meldframe/web-host/b'; import b from '@meldframe/web-view/b';")])).toEqual([]);
  });
  it('fails a model that imports a host, a kit or a view', () => {
    expect(rules(imp('web-model', 'web-host'))).toEqual(['matrix:web-model->web-host']);
    expect(rules(imp('web-model', 'web-kit'))).toEqual(['matrix:web-model->web-kit']);
    expect(rules(imp('web-model', 'web-view'))).toEqual(['matrix:web-model->web-view']);
  });
  it('fails a host that imports a view, a kit or a tool', () => {
    expect(rules(imp('web-host', 'web-view'))).toEqual(['matrix:web-host->web-view']);
    expect(rules(imp('web-host', 'web-tool'))).toEqual(['matrix:web-host->web-tool']);
  });
  it('fails a kit that imports anything but a kit, and a view that imports a host', () => {
    expect(rules(imp('web-kit', 'web-model'))).toEqual(['matrix:web-kit->web-model']);
    expect(rules(imp('web-view', 'web-host'))).toEqual(['matrix:web-view->web-host']);
  });
  it('fails the proto package when it imports a package, and anything importing the app', () => {
    expect(rules(imp('protocol', 'web-model'))).toEqual(['matrix:protocol->web-model']);
    expect(rules(check([file('web-tool', 'a.ts', "import a from '@meldframe/web';")]))).toEqual([]);
    expect(rules(imp('web-model', 'web'))).toEqual(['matrix:web-model->web']);
  });
  it('applies the matrix to test files and scripts, with the path and line of the import', () => {
    const vs = check([{ path: 'packages/web-model/src/a.test.ts', text: "import { it } from 'vitest';\nimport h from '@meldframe/web-host/b';" }]);
    expect(vs).toHaveLength(1);
    expect(vs[0]).toMatchObject({ rule: 'matrix', path: 'packages/web-model/src/a.test.ts', line: 2 });
    expect(rules(check([{ path: 'packages/web-model/scripts/a.mjs', text: "import h from '../../web-host/src/b';" }]))).toEqual(['matrix:web-model->web-host']);
  });
  it('ignores an import of the package by its own name', () => {
    expect(check([file('web-model', 'a.ts', "import x from '@meldframe/web-model/b';")])).toEqual([]);
  });
});

describe('frameworks', () => {
  it('keeps svelte, bits-ui and pixi.js out of a model, a host and a proto package', () => {
    expect(rules(check([file('web-model', 'a.ts', "import { mount } from 'svelte';")]))).toEqual(['framework:web-model->svelte']);
    expect(rules(check([file('web-host', 'a.ts', "import { Button } from 'bits-ui';")]))).toEqual(['framework:web-host->bits-ui']);
    expect(rules(check([file('protocol', 'a.ts', "import * as P from 'pixi.js';")]))).toEqual(['framework:protocol->pixi.js']);
  });
  it('keeps pixi.js out of a view and a kit, and svelte out of a tool package source', () => {
    expect(rules(check([file('web-view', 'a.ts', "import * as P from 'pixi.js';")]))).toEqual(['framework:web-view->pixi.js']);
    expect(rules(check([file('web-kit', 'a.ts', "import * as P from 'pixi.js';")]))).toEqual(['framework:web-kit->pixi.js']);
    expect(rules(check([file('web-tool', 'a.ts', "import { mount } from 'svelte';")]))).toEqual(['framework:web-tool->svelte']);
  });
  it('lets a Svelte file of a kit or a view import svelte and bits-ui, but not its plain TypeScript files', () => {
    expect(check([file('web-view', 'A.svelte', "<script>import { Dialog } from 'bits-ui'; import type { Snippet } from 'svelte';</script>")])).toEqual([]);
    expect(check([file('web-kit', 'A.svelte', "<script>import { Menu } from 'bits-ui';</script>")])).toEqual([]);
    expect(rules(check([file('web-view', 'a.ts', "import { tick } from 'svelte';")]))).toEqual(['framework-file:web-view->svelte']);
    expect(rules(check([file('web-kit', 'a.ts', "import { Menu } from 'bits-ui';")]))).toEqual(['framework-file:web-kit->bits-ui']);
  });
  it('lets the app mount svelte and draw with pixi.js from plain files, and a render package use pixi.js', () => {
    expect(check([file('web', 'main.ts', "import { mount } from 'svelte'; import * as P from 'pixi.js';")])).toEqual([]);
    expect(check([file('web-render', 'stage.ts', "import * as P from 'pixi.js';")], [...packages, pkg('web-render', 'render')])).toEqual([]);
  });
  it('lets a test file import a framework or test tooling, whatever the kind', () => {
    expect(check([file('web-model', 'a.test.ts', "import { render } from 'svelte/server'; import { it } from 'vitest'; import fs from 'node:fs';")])).toEqual([]);
    expect(check([{ path: 'packages/web-host/scripts/a.mjs', text: "import { chromium } from 'playwright';" }])).toEqual([]);
  });
  it('fails a Svelte file in a package of a kind that holds no views', () => {
    expect(rules(check([file('web-model', 'A.svelte', '<script>let a = 1;</script>')]))).toEqual(['file-type:web-model->.svelte']);
    expect(rules(check([file('web-host', 'A.svelte', '<script></script>')]))).toEqual(['file-type:web-host->.svelte']);
    expect(check([file('web-view', 'A.svelte', '<script></script>'), file('web-kit', 'B.svelte', '<script></script>')])).toEqual([]);
  });
  it('is a table: React, with its .tsx view files, is accepted in the view kinds and nowhere else', () => {
    const frameworks = FRAMEWORKS; // React is a row of the table since S0
    expect(check([file('web-view', 'A.tsx', "import { useState } from 'react'; import { createRoot } from 'react-dom/client';")], packages, { frameworks })).toEqual([]);
    expect(rules(check([file('web-model', 'a.ts', "import { useState } from 'react';")], packages, { frameworks }))).toEqual(['framework:web-model->react']);
    expect(rules(check([file('web-view', 'a.ts', "import { useState } from 'react';")], packages, { frameworks }))).toEqual(['framework-file:web-view->react']);
    expect(rules(check([file('web-model', 'A.tsx', 'export const a = 1;')], packages, { frameworks }))).toEqual(['file-type:web-model->.tsx']);
    // without the row, react is just another module
    const without = FRAMEWORKS.filter((f) => f.name !== 'react');
    expect(check([file('web-model', 'a.ts', "import { useState } from 'react';")], packages, { frameworks: without })).toEqual([]);
  });
});

describe('the group rule', () => {
  it('fails a view that imports a view of another group, and allows the same group and a composite group', () => {
    const imp = (from) => check([file('web-view', 'A.svelte', "<script>import x from '@meldframe/web-view2/B.svelte';</script>")], from);
    expect(rules(imp(packages))).toEqual(['group:web-view->web-view2']);
    expect(imp(packages.map((p) => (p.id === 'web-view' ? { ...p, composite: ['two'] } : p)))).toEqual([]);
    expect(imp(packages.map((p) => (p.id === 'web-view2' ? { ...p, group: 'one' } : p)))).toEqual([]);
  });
});

describe('cycles', () => {
  it('finds a cycle of two packages and one of three, each once, and none in a chain', () => {
    expect(findCycles([['a', 'b'], ['b', 'a']])).toEqual([['a', 'b']]);
    expect(findCycles([['a', 'b'], ['b', 'c'], ['c', 'a'], ['c', 'd']])).toEqual([['a', 'b', 'c']]);
    expect(findCycles([['a', 'b'], ['b', 'c'], ['a', 'c']])).toEqual([]);
  });
  it('reports a cycle of packages as a violation', () => {
    const vs = check(
      [file('web-model', 'a.ts', "import x from '@meldframe/web-model2/b';"), file('web-model2', 'b.ts', "import y from '@meldframe/web-model/a';")],
    );
    expect(rules(vs)).toEqual(['cycle:web-model > web-model2->']);
  });
});

describe('the allow-list', () => {
  const v = (path, line) => ({ rule: 'matrix', from: 'web-view', to: 'web-host', path, line, message: 'm' });
  const entry = (count, extra = {}) => ({ rule: 'matrix', from: 'web-view', to: 'web-host', count, reason: 'moves with the helper', ...extra });
  const two = [v('a.ts', 1), v('b.ts', 2)];

  it('lets an entry that equals the number of violations pass them', () => {
    expect(applyAllowList(two, [entry(2)])).toEqual([]);
  });
  it('reports every violation that has no entry', () => {
    expect(applyAllowList(two, [])).toHaveLength(2);
  });
  it('reports the violations of an entry that is too low, and asks for no raise', () => {
    const errors = applyAllowList(two, [entry(1)]);
    expect(errors).toHaveLength(3);
    expect(errors.join('\n')).toContain('has 2 now, the allow-list has 1');
  });
  it('fails an entry that is higher than the violations (entries only go down)', () => {
    const errors = applyAllowList([v('a.ts', 1)], [entry(2)]);
    expect(errors).toHaveLength(1);
    expect(errors[0]).toContain('lower it to 1');
  });
  it('fails an entry that is no longer needed', () => {
    const errors = applyAllowList([], [entry(1)]);
    expect(errors).toHaveLength(1);
    expect(errors[0]).toContain('no longer needed');
  });
  it('fails an entry with no reason, a bad count or a repeated key', () => {
    expect(applyAllowList(two, [entry(2, { reason: ' ' })]).join('\n')).toContain('reason');
    expect(applyAllowList(two, [entry(0)]).join('\n')).toContain('count');
    expect(applyAllowList(two, [entry(2), entry(2)]).join('\n')).toContain('twice');
  });
  it('rejects an entry that is new or higher than on the base branch', () => {
    expect(compareToBase([entry(2)], [entry(2)])).toEqual([]);
    expect(compareToBase([entry(1)], [entry(2)])).toEqual([]);
    expect(compareToBase([entry(3)], [entry(2)]).join('\n')).toContain('higher than on the base');
    expect(compareToBase([entry(1)], []).join('\n')).toContain('new');
  });
});
