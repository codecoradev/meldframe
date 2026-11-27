// Tests of the freeze check on made-up input (frozen-lib.mjs) and of the command on a scratch repository (frozen.mjs).
// They use `node:assert` only and take describe and it from vitest when it is installed (`npm test` in apps/web picks this file
// up), otherwise from `node:test`, so `node --test packages/web-tools/scripts/frozen.test.mjs` runs them with no install.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  buildAllow,
  checkFrozen,
  compareToBase,
  countLines,
  isFrozenFile,
  newPilotFiles,
  packageDirOf,
  renderAllow,
} from './frozen-lib.mjs';

const { describe, it } = await import('vitest').catch(() => import('node:test'));

const here = dirname(fileURLToPath(import.meta.url));
const cli = join(here, 'frozen.mjs');

const entry = (dir, files) => ({ dir, lines: Object.values(files).reduce((a, b) => a + b, 0), files });
const allow = [entry('packages/web-panels', { 'src/A.svelte': 100, 'src/react/B.tsx': 50 }), entry('apps/web', { 'src/react-pilot/pilot.ts': 20 })];
const tree = (extra = {}) => ({
  'packages/web-panels/src/A.svelte': 100,
  'packages/web-panels/src/react/B.tsx': 50,
  'apps/web/src/react-pilot/pilot.ts': 20,
  ...extra,
});
const files = (map) => Object.entries(map).map(([path, lines]) => ({ path, lines }));
const run = (map, list = allow) => checkFrozen(files(map), list).map((x) => `${x.rule} ${x.path}`);

describe('what is counted', () => {
  it('counts .svelte and .tsx files and the code files of react and react-pilot directories', () => {
    for (const p of [
      'packages/web-panels/src/A.svelte',
      'packages/web-panels/src/react/B.tsx',
      'packages/web-panels/src/react/compare.ts',
      'apps/web/src/canvas/react/surface.ts',
      'apps/web/src/react-pilot/pilot.ts',
      'apps/web/src/Editor.svelte',
      'apps/web/src/Canvas.tsx',
    ]) assert.equal(isFrozenFile(p), true, p);
  });
  it('does not count tests, test helpers, scripts or plain code outside the react directories', () => {
    for (const p of [
      'packages/web-panels/src/react/B.test.tsx',
      'apps/web/src/react-pilot/pilot.test.ts',
      'packages/web-chrome/src/react/markup.testutil.ts',
      'packages/web-panels/scripts/layers-e2e.mjs',
      'packages/web-tools/scripts/frozen.mjs',
      'packages/web-model/src/camera.ts',
      'apps/web/src/main.ts',
      'packages/web-panels/src/react/notes.md',
    ]) assert.equal(isFrozenFile(p), false, p);
  });
  it('names the package of a path and counts lines like the size check', () => {
    assert.equal(packageDirOf('apps/web/src/A.svelte'), 'apps/web');
    assert.equal(packageDirOf('packages/web-panels/src/A.svelte'), 'packages/web-panels');
    assert.equal(packageDirOf('tools/x.tsx'), null);
    assert.deepEqual(['', 'a', 'a\n', 'a\nb', 'a\nb\n', '\n'].map(countLines), [0, 1, 1, 2, 2, 1]);
  });
});

describe('the freeze against the allow-list', () => {
  it('passes the tree that the allow-list was made from', () => {
    assert.deepEqual(run(tree()), []);
    assert.deepEqual(buildAllow(files(tree())).map((p) => p.dir), ['apps/web', 'packages/web-panels']);
  });
  it('fails a new .tsx file', () => {
    assert.deepEqual(run(tree({ 'packages/web-panels/src/New.tsx': 10 })), ['new-file packages/web-panels/src/New.tsx']);
  });
  it('fails a new .svelte file', () => {
    assert.deepEqual(run(tree({ 'packages/web-panels/src/New.svelte': 10 })), ['new-file packages/web-panels/src/New.svelte']);
  });
  it('fails a new file in a package that has no entry, as a new package', () => {
    assert.deepEqual(run(tree({ 'packages/web-other/src/New.svelte': 3 })), ['new-package packages/web-other/src/New.svelte']);
  });
  it('fails a new file under react-pilot', () => {
    assert.deepEqual(run(tree({ 'apps/web/src/react-pilot/More.svelte': 3 })), ['react-pilot apps/web/src/react-pilot/More.svelte']);
  });
  it('fails a file that grew', () => {
    assert.deepEqual(run(tree({ 'packages/web-panels/src/A.svelte': 101 })), ['grew packages/web-panels/src/A.svelte']);
  });
  it('makes the entry of a file that shrank stale until it is lowered', () => {
    const shrunk = tree({ 'packages/web-panels/src/A.svelte': 90 });
    assert.deepEqual(run(shrunk), ['stale packages/web-panels/src/A.svelte']);
    assert.deepEqual(run(shrunk, buildAllow(files(shrunk))), []);
  });
  it('makes the entry of a deleted file stale until it is removed', () => {
    const rest = tree();
    delete rest['packages/web-panels/src/react/B.tsx'];
    assert.deepEqual(run(rest), ['gone packages/web-panels/src/react/B.tsx']);
    assert.deepEqual(run(rest, buildAllow(files(rest))), []);
  });
  it('lets a fix that keeps or lowers every size pass once the entries are lowered', () => {
    assert.deepEqual(run(tree()), []);
    const lower = tree({ 'packages/web-panels/src/react/B.tsx': 48 });
    assert.deepEqual(run(lower, buildAllow(files(lower))), []);
  });
  it('rejects a malformed allow-list: a bad count, a total that is not the sum', () => {
    const bad = [{ dir: 'apps/web', lines: 5, files: { 'src/a.svelte': 0, 'src/b.svelte': 2 } }];
    const out = checkFrozen([], bad).map((x) => x.rule);
    assert.deepEqual(out.filter((r) => r === 'allow-list').length, 2);
  });
  it('writes the allow-list as JSON that reads back the same', () => {
    const built = buildAllow(files(tree()));
    assert.deepEqual(JSON.parse(renderAllow(built, 'c')).packages, built);
  });
});

describe('the allow-list against the base', () => {
  const same = JSON.parse(JSON.stringify(allow));
  it('passes the same allow-list and a lower one', () => {
    assert.deepEqual(compareToBase(allow, same), []);
    const lower = buildAllow(files(tree({ 'packages/web-panels/src/A.svelte': 90 })));
    assert.deepEqual(compareToBase(lower, same), []);
  });
  it('fails an entry that is higher than on the base, and the total of its package', () => {
    const higher = buildAllow(files(tree({ 'packages/web-panels/src/A.svelte': 130 })));
    assert.deepEqual(compareToBase(higher, same).map((x) => x.path), ['packages/web-panels/src/A.svelte', 'packages/web-panels']);
  });
  it('fails an entry that is new, and a new package', () => {
    const added = buildAllow(files(tree({ 'packages/web-other/src/N.svelte': 4, 'packages/web-panels/src/N.tsx': 4 })));
    assert.deepEqual(
      compareToBase(added, same).map((x) => x.path).sort(),
      ['packages/web-other', 'packages/web-other/src/N.svelte', 'packages/web-panels', 'packages/web-panels/src/N.tsx'],
    );
  });
  it('finds a new file under react-pilot that is only a test', () => {
    const now = ['apps/web/src/react-pilot/pilot.ts', 'apps/web/src/react-pilot/new.test.ts'];
    assert.deepEqual(newPilotFiles(now, ['apps/web/src/react-pilot/pilot.ts']).map((x) => x.path), ['apps/web/src/react-pilot/new.test.ts']);
    assert.deepEqual(newPilotFiles(now, now), []);
  });
});

describe('the command on a scratch repository', () => {
  const git = (cwd, ...a) => {
    const r = spawnSync('git', ['-c', 'user.name=t', '-c', 'user.email=t@example.invalid', '-c', 'commit.gpgsign=false', ...a], { cwd, encoding: 'utf8' });
    assert.equal(r.status, 0, r.stderr);
  };
  const put = (dir, path, text) => {
    mkdirSync(dirname(join(dir, path)), { recursive: true });
    writeFileSync(join(dir, path), text);
  };
  const lines = (n) => 'x\n'.repeat(n);
  const frozen = (dir, ...args) => spawnSync('node', [cli, '--root', dir, ...args], { encoding: 'utf8' });
  const scratch = (body) => {
    const dir = mkdtempSync(join(tmpdir(), 'frozen-'));
    try {
      put(dir, 'packages/web-panels/src/A.svelte', lines(10));
      put(dir, 'packages/web-panels/src/react/B.tsx', lines(5));
      put(dir, 'apps/web/src/react-pilot/pilot.ts', lines(4));
      put(dir, 'apps/web/src/react-pilot/pilot.test.ts', lines(4));
      put(dir, 'packages/web-panels/scripts/skipped.mjs', lines(99));
      put(dir, 'packages/web-model/src/plain.ts', lines(99));
      assert.equal(frozen(dir, '--init').status, 0);
      git(dir, 'init', '-q', '-b', 'main');
      git(dir, 'add', '-A');
      git(dir, 'commit', '-q', '-m', 'base');
      body(dir);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  };
  it('writes the allow-list with --init and passes on the tree it came from, with and without --base', () => {
    scratch((dir) => {
      const list = JSON.parse(readFileSync(join(dir, 'packages/web-tools/frozen-allow.json'), 'utf8')).packages;
      assert.deepEqual(list.map((p) => [p.dir, p.lines]), [['apps/web', 4], ['packages/web-panels', 15]]);
      assert.equal(frozen(dir).status, 0);
      assert.equal(frozen(dir, '--base', 'HEAD').status, 0);
    });
  });
  it('fails a new .tsx file and prints path, rule and message', () => {
    scratch((dir) => {
      put(dir, 'packages/web-panels/src/New.tsx', lines(3));
      const r = frozen(dir);
      assert.equal(r.status, 1);
      assert.match(r.stderr, /^packages\/web-panels\/src\/New\.tsx: \[new-file\] /m);
    });
  });
  it('fails a file that grew and, after --init blesses it, fails against the base', () => {
    scratch((dir) => {
      put(dir, 'packages/web-panels/src/A.svelte', lines(12));
      const grown = frozen(dir);
      assert.equal(grown.status, 1);
      assert.match(grown.stderr, /A\.svelte: \[grew\] /);
      assert.equal(frozen(dir, '--init').status, 0);
      assert.equal(frozen(dir).status, 0);
      const base = frozen(dir, '--base', 'HEAD');
      assert.equal(base.status, 1);
      assert.match(base.stderr, /A\.svelte: \[base\] .*higher than on the base \(10\)/);
    });
  });
  it('passes a lowered entry against the base', () => {
    scratch((dir) => {
      put(dir, 'packages/web-panels/src/A.svelte', lines(8));
      assert.match(frozen(dir).stderr, /\[stale\]/);
      assert.equal(frozen(dir, '--init').status, 0);
      assert.equal(frozen(dir, '--base', 'HEAD').status, 0);
    });
  });
  it('fails a new test file under react-pilot against the base, though a test is not counted', () => {
    scratch((dir) => {
      put(dir, 'apps/web/src/react-pilot/other.test.ts', lines(2));
      assert.equal(frozen(dir).status, 0);
      const r = frozen(dir, '--base', 'HEAD');
      assert.equal(r.status, 1);
      assert.match(r.stderr, /react-pilot\/other\.test\.ts: \[react-pilot\] /);
    });
  });
  it('errors when the base does not resolve, and on an unknown option', () => {
    scratch((dir) => {
      const r = frozen(dir, '--base', 'no-such-ref');
      assert.equal(r.status, 2);
      assert.match(r.stderr, /not a git ref that resolves/);
      assert.equal(frozen(dir, '--wat').status, 2);
    });
  });
});
