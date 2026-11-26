// The rule of the view packages about text (U9, thin-UI audit finding T1, issue #319): no package of the web client carries
// a definition of hidden characters, because no screen cleans text: it is made safe in the app layer, once. A test and not an
// npm script: it runs with `npm test` (so with `npm run check` and the `checks (web)` job) and needs no step of its own, and a
// new `\p{Cf}`-style regex in a view, model or host package fails it. The allow-list names what is left, with its reason; an
// entry must equal its count, so entries only go down.
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { checkText, hiddenTextLines, isProductSource } from './text-rules-lib.mjs';

const root = fileURLToPath(new URL('../../..', import.meta.url));
const SKIP = new Set(['node_modules', 'dist', '.wasm', 'target', 'rust']);

/** What is allowed today: the check of a typed name, a rule of the input and not a cleaner (finding T5, #323 moves it to the app layer). */
const ALLOW = [
  { file: 'packages/web-variables/src/manage.ts', count: 1, reason: 'HIDDEN, the check of a name the person types; the name rules move to the app layer with T5 (#323)' },
];

function* walk(dir) {
  for (const e of readdirSync(join(root, dir), { withFileTypes: true })) {
    const path = `${dir}/${e.name}`;
    if (e.isDirectory()) {
      if (!SKIP.has(e.name)) yield* walk(path);
    } else yield path;
  }
}

const tree = () => [...['packages', 'apps/web'].flatMap((d) => [...walk(d)])].filter(isProductSource).map((path) => ({ path, text: readFileSync(join(root, path), 'utf8') }));

describe('hiddenTextLines', () => {
  it('finds a Unicode property class of controls, formats and separators, in a regex literal or in a string', () => {
    expect(hiddenTextLines('const a = s.replace(/[\\p{Cc}\\p{Cf}]/gu, "");')).toEqual([1]);
    expect(hiddenTextLines("new RegExp('\\\\p{Zs}+', 'u')")).toEqual([1]);
    expect(hiddenTextLines('/\\p{Cf}/u\nok\n/\\p{Zl}/u')).toEqual([1, 3]);
  });
  it('finds the escape of a hidden code point, and the character written out', () => {
    for (const bad of ['\\u200b', '\\u200B', '\\u202e', '\\u2066', '\\u00ad', '\\u2800', '\\u3164', '\\u17B4', '\\u{200b}', '\\u{E0001}', '\\xad']) {
      expect(hiddenTextLines(`x.replace(/${bad}/g, '')`), bad).toEqual([1]);
    }
    expect(hiddenTextLines(`const s = 'a${String.fromCodePoint(0x202e)}b';`)).toEqual([1]);
    expect(hiddenTextLines(`const s = 'a${String.fromCodePoint(0x200b)}b';`)).toEqual([1]);
  });
  it('lets pass what is not a hidden character: a letter, a visible escape, a property class of letters', () => {
    expect(hiddenTextLines("const a = '\\u00e9'; const b = /\\p{L}+/u; const c = '\\u{1F600}'; const d = '\\x41';")).toEqual([]);
  });
});

describe('isProductSource', () => {
  it('is the source of a package and of the app, and not a test, a script or the tooling', () => {
    expect(isProductSource('packages/web-activity/src/view.ts')).toBe(true);
    expect(isProductSource('packages/web-panels/src/LayersPanel.svelte')).toBe(true);
    expect(isProductSource('apps/web/src/Editor.svelte')).toBe(true);
    expect(isProductSource('packages/web-activity/src/view.test.ts')).toBe(false);
    expect(isProductSource('packages/web-panels/scripts/activity-e2e.mjs')).toBe(false);
    expect(isProductSource('packages/web-tools/scripts/text-rules-lib.mjs')).toBe(false);
    expect(isProductSource('packages/web-tools/src/proxy-guard.ts')).toBe(false);
  });
});

describe('checkText', () => {
  const file = (path, text) => ({ path, text });
  it('fails a new hidden-character regex in a view package', () => {
    const problems = checkText([file('packages/web-feedback/src/failure.ts', 'const plain = (s) => s.replace(/[\\p{Cc}\\p{Cf}]/gu, "");')], []);
    expect(problems).toHaveLength(1);
    expect(problems[0]).toMatch(/failure\.ts:1: a hidden-character class in a view package/);
  });
  it('passes a file with none, and an allowed file at its count', () => {
    expect(checkText([file('packages/web-a/src/a.ts', 'const x = 1;')], [])).toEqual([]);
    expect(checkText([file('packages/web-a/src/a.ts', '/\\p{Cf}/u')], [{ file: 'packages/web-a/src/a.ts', count: 1, reason: 'x' }])).toEqual([]);
  });
  it('fails an allowed file that has more lines than its entry says, or none: entries only go down', () => {
    const entry = [{ file: 'packages/web-a/src/a.ts', count: 1, reason: 'x' }];
    expect(checkText([file('packages/web-a/src/a.ts', '/\\p{Cf}/u\n/\\p{Cc}/u')], entry)[0]).toMatch(/says 1 and the file has 2/);
    expect(checkText([file('packages/web-a/src/a.ts', 'clean')], entry)[0]).toMatch(/allowed but clean/);
  });
});

describe('the web client', () => {
  it('has no definition of hidden characters in a view, model or host package but the one allowed', () => {
    expect(checkText(tree(), ALLOW)).toEqual([]);
  });
});
