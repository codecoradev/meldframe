// The rule of the view packages about text (U9, thin-UI audit finding T1, issue #319): no package of the web client carries
// a definition of hidden characters, because no screen cleans text: it is made safe in the app layer, once. A test and not an
// npm script: it runs with `npm test` (so with `npm run check` and the `checks (web)` job) and needs no step of its own, and a
// new `\p{Cf}`-style regex in a view, model or host package fails it. The allow-list names what is left, with its reason; an
// entry must equal its count, so entries only go down.
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { HIDDEN_RANGES, checkText, hiddenTextLines, isProductSource } from './text-rules-lib.mjs';

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
  it('finds the controls and the rest of the set of the Rust function, so a range of controls cannot slip by', () => {
    for (const bad of ['[\\u0000-\\u001f\\u007f]', '\\x00', '\\x7f', '\\x9f', '\\u0890', '\\u08E2', '\\u{110bd}', '\\u{13430}', '\\u{1bca0}', '\\u{1d173}']) {
      expect(hiddenTextLines(`x.replace(/${bad}/g, '')`), bad).toEqual([1]);
    }
  });
  it('finds the General_Category form of a property class', () => {
    expect(hiddenTextLines('/\\p{gc=Cf}/u')).toEqual([1]);
    expect(hiddenTextLines('/\\p{General_Category=Format}/u')).toEqual([1]);
    expect(hiddenTextLines("new RegExp('\\\\p{General_Category=Cc}', 'u')")).toEqual([1]);
  });
  it('finds a character built from its code and an escape split in two strings', () => {
    expect(hiddenTextLines('const z = String.fromCharCode(0x200b);')).toEqual([1]);
    expect(hiddenTextLines('const z = String.fromCodePoint(8238);')).toEqual([1]);
    expect(hiddenTextLines('const z = String.fromCodePoint(code);')).toEqual([1]); // a code that is not a literal can be any
    expect(hiddenTextLines("const z = '\\u{20' + '0b}';")).toEqual([1]);
    expect(hiddenTextLines("const z = '\\u' + '200b';")).toEqual([1]);
    expect(hiddenTextLines('const z = String.fromCodePoint(0x41);')).toEqual([]);
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
    // A tree that is empty (a moved directory, a wrong root) would pass this for nothing.
    expect(tree().length).toBeGreaterThan(0);
    expect(checkText(tree(), ALLOW)).toEqual([]);
  });
});

describe('the table of hidden code points', () => {
  it('is the set of meldframe_text::is_hidden_text: the Rust function and this copy are read and compared', () => {
    const rust = readFileSync(join(root, 'crates/core/meldframe-text/src/charset.rs'), 'utf8');
    const body = rust.slice(rust.indexOf('pub fn is_hidden_text'));
    const list = body.slice(body.indexOf('matches!(c as u32,'), body.indexOf('\n}\n'));
    const ranges = [...list.matchAll(/0x([0-9A-Fa-f]+)(?:\.\.=0x([0-9A-Fa-f]+))?/g)].map((m) => [parseInt(m[1], 16), parseInt(m[2] ?? m[1], 16)]);
    // The Rust function names the controls as `c.is_control()` (U+0000 to U+001F and U+007F to U+009F): they are in the table.
    const table = HIDDEN_RANGES.filter(([a]) => !(a === 0x0000 || a === 0x007f));
    expect(table).toEqual(ranges);
    expect(HIDDEN_RANGES.slice(0, 2)).toEqual([[0x0000, 0x001f], [0x007f, 0x009f]]);
  });
});
